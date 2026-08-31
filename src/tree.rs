use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
}

pub struct Tree {
    root: PathBuf,
    expanded: HashSet<PathBuf>,
    show_hidden: bool,
    selected: usize,
    rows: Vec<Row>,
}

impl Tree {
    pub fn new(root: PathBuf) -> io::Result<Tree> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "not a directory",
            ));
        }
        let mut tree = Tree {
            root,
            expanded: HashSet::new(),
            show_hidden: false,
            selected: 0,
            rows: Vec::new(),
        };
        tree.rebuild();
        Ok(tree)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_row(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1));
    }

    pub fn move_top(&mut self) {
        self.selected = 0;
    }

    pub fn move_bottom(&mut self) {
        self.selected = self.rows.len().saturating_sub(1);
    }

    pub fn expand(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if row.is_dir {
            self.expanded.insert(row.path.clone());
            self.rebuild_keeping_selection();
        }
    }

    /// Collapse an expanded dir; on a file or collapsed dir, move
    /// selection to the parent row if it is in the tree.
    pub fn collapse(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if row.is_dir && row.expanded {
            let path = row.path.clone();
            self.expanded.retain(|p| !p.starts_with(&path));
            self.rebuild_keeping_selection();
        } else if let Some(parent) = row.path.parent() {
            if let Some(idx) = self.rows.iter().position(|r| r.path == parent) {
                self.selected = idx;
            }
        }
    }

    pub fn toggle(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if row.is_dir && row.expanded {
            self.collapse();
        } else {
            self.expand();
        }
    }

    /// Re-root the tree at the parent directory. The old root stays
    /// visible as an expanded, selected row so you keep your place.
    /// No-op at the filesystem root.
    pub fn ascend(&mut self) {
        let Some(parent) = self.root.parent().map(|p| p.to_path_buf()) else {
            return;
        };
        let old_root = std::mem::replace(&mut self.root, parent);
        self.expanded.insert(old_root.clone());
        self.rebuild();
        self.selected = self
            .rows
            .iter()
            .position(|r| r.path == old_root)
            .unwrap_or(0);
    }

    /// Re-root the tree at the selected directory (or, for a file, its
    /// parent directory). Inverse of `ascend`.
    pub fn make_root(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        let new_root = if row.is_dir {
            row.path.clone()
        } else {
            match row.path.parent() {
                Some(p) => p.to_path_buf(),
                None => return,
            }
        };
        if new_root == self.root {
            return;
        }
        self.root = new_root;
        self.selected = 0;
        self.rebuild();
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.rebuild_keeping_selection();
    }

    pub fn refresh(&mut self) {
        self.rebuild_keeping_selection();
    }

    fn rebuild_keeping_selection(&mut self) {
        let selected_path = self.selected_row().map(|r| r.path.clone());
        self.rebuild();
        self.selected = selected_path
            .and_then(|p| self.rows.iter().position(|r| r.path == p))
            .unwrap_or(self.selected)
            .min(self.rows.len().saturating_sub(1));
    }

    fn rebuild(&mut self) {
        self.rows.clear();
        let root = self.root.clone();
        self.push_children(&root, 0);
    }

    fn push_children(&mut self, dir: &Path, depth: usize) {
        for (path, is_dir) in list_dir(dir, self.show_hidden) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let expanded = is_dir && self.expanded.contains(&path);
            self.rows.push(Row {
                path: path.clone(),
                name,
                depth,
                is_dir,
                expanded,
            });
            if expanded {
                self.push_children(&path, depth + 1);
            }
        }
    }
}

/// List one directory: dirs and text files, dirs first, each group
/// sorted case-insensitively. Honors per-directory .gitignore.
fn list_dir(dir: &Path, show_hidden: bool) -> Vec<(PathBuf, bool)> {
    let mut out = Vec::new();
    let walker = ignore::WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(!show_hidden)
        .require_git(false)
        .git_global(false)
        .parents(false)
        .build();
    for entry in walker.flatten() {
        if entry.depth() == 0 {
            continue; // the dir itself
        }
        if entry.file_name() == ".git" {
            continue;
        }
        let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
        let path = entry.into_path();
        if is_dir || crate::fsutil::is_text_file(&path) {
            out.push((path, is_dir));
        }
    }
    out.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            a.0.file_name()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .cmp(&b.0.file_name().unwrap_or_default().to_ascii_lowercase())
        })
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// tmp fixture:
    ///   root/
    ///     b-dir/inner.md
    ///     a.md
    ///     zz.md
    ///     bin.dat        (binary, hidden from tree)
    ///     .hidden.md     (dotfile)
    fn fixture(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("mrkdup-tree-{tag}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("b-dir")).unwrap();
        fs::write(root.join("b-dir/inner.md"), "x\n").unwrap();
        fs::write(root.join("a.md"), "a\n").unwrap();
        fs::write(root.join("zz.md"), "z\n").unwrap();
        fs::write(root.join("bin.dat"), b"\x00\x01").unwrap();
        fs::write(root.join(".hidden.md"), "h\n").unwrap();
        root
    }

    fn names(t: &Tree) -> Vec<String> {
        t.rows().iter().map(|r| r.name.clone()).collect()
    }

    #[test]
    fn lists_dirs_first_then_files_sorted_no_binary_no_hidden() {
        let t = Tree::new(fixture("list")).unwrap();
        assert_eq!(names(&t), vec!["b-dir", "a.md", "zz.md"]);
    }

    #[test]
    fn expand_inserts_children_below() {
        let mut t = Tree::new(fixture("expand")).unwrap();
        t.expand(); // selection starts at 0 = b-dir
        assert_eq!(names(&t), vec!["b-dir", "inner.md", "a.md", "zz.md"]);
        assert_eq!(t.rows()[1].depth, 1);
    }

    #[test]
    fn collapse_removes_children() {
        let mut t = Tree::new(fixture("collapse")).unwrap();
        t.expand();
        t.collapse();
        assert_eq!(names(&t), vec!["b-dir", "a.md", "zz.md"]);
    }

    #[test]
    fn collapse_on_file_selects_parent_dir() {
        let mut t = Tree::new(fixture("parent")).unwrap();
        t.expand();
        t.move_down(); // inner.md
        t.collapse();
        assert_eq!(t.selected_row().unwrap().name, "b-dir");
    }

    #[test]
    fn movement_clamps_at_edges() {
        let mut t = Tree::new(fixture("mv")).unwrap();
        t.move_up();
        assert_eq!(t.selected(), 0);
        t.move_bottom();
        assert_eq!(t.selected_row().unwrap().name, "zz.md");
        t.move_down();
        assert_eq!(t.selected_row().unwrap().name, "zz.md");
        t.move_top();
        assert_eq!(t.selected(), 0);
    }

    #[test]
    fn toggle_hidden_shows_dotfiles() {
        let mut t = Tree::new(fixture("hidden")).unwrap();
        t.toggle_hidden();
        assert!(names(&t).contains(&".hidden.md".to_string()));
        t.toggle_hidden();
        assert!(!names(&t).contains(&".hidden.md".to_string()));
    }

    #[test]
    fn refresh_preserves_expansion_and_sees_new_files() {
        let root = fixture("refresh");
        let mut t = Tree::new(root.clone()).unwrap();
        t.expand();
        fs::write(root.join("b-dir/new.md"), "n\n").unwrap();
        t.refresh();
        assert_eq!(
            names(&t),
            vec!["b-dir", "inner.md", "new.md", "a.md", "zz.md"]
        );
    }

    #[test]
    fn ascend_reroots_at_parent_keeping_place() {
        let root = fixture("asc");
        let mut t = Tree::new(root.clone()).unwrap();
        t.expand(); // expand b-dir so we can check state survives
        t.ascend();
        let canon = root.canonicalize().unwrap();
        // old root is now a visible, expanded, selected row...
        let sel = t.selected_row().unwrap();
        assert_eq!(sel.path, canon);
        assert!(sel.is_dir && sel.expanded);
        // ...its children (and the previously expanded subdir's) still show
        assert!(t.rows().iter().any(|r| r.path == canon.join("a.md")));
        assert!(t
            .rows()
            .iter()
            .any(|r| r.path == canon.join("b-dir/inner.md")));
        // and the tree is rooted one level up now
        assert_eq!(t.root(), canon.parent().unwrap());
    }

    #[test]
    fn make_root_on_dir_reroots_there() {
        let root = fixture("mkroot");
        let mut t = Tree::new(root.clone()).unwrap();
        t.make_root(); // selection starts on b-dir
        assert_eq!(t.root(), root.canonicalize().unwrap().join("b-dir"));
        assert_eq!(names(&t), vec!["inner.md"]);
        assert_eq!(t.selected(), 0);
    }

    #[test]
    fn make_root_on_file_reroots_at_its_parent() {
        let root = fixture("mkroot-file");
        let mut t = Tree::new(root.clone()).unwrap();
        t.expand();
        t.move_down(); // inner.md
        t.make_root();
        assert_eq!(t.root(), root.canonicalize().unwrap().join("b-dir"));
        assert_eq!(names(&t), vec!["inner.md"]);
    }

    #[test]
    fn ascend_at_filesystem_root_is_noop() {
        let mut t = Tree::new(PathBuf::from("/")).unwrap();
        let before = t.root().to_path_buf();
        t.ascend();
        assert_eq!(t.root(), before);
    }

    #[test]
    fn empty_directory_yields_no_rows_and_no_panic() {
        let root = std::env::temp_dir().join("mrkdup-tree-empty");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut t = Tree::new(root).unwrap();
        assert!(t.rows().is_empty());
        assert!(t.selected_row().is_none());
        t.move_down();
        t.expand();
        t.collapse();
    }
}
