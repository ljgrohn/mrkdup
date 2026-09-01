use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
    /// `is_text_file` results keyed by path, valid as long as the file's
    /// mtime matches what's stored. Sniffing a file opens and reads up to
    /// 8KB of it, which is wasteful to redo on every ~2s tree refresh when
    /// nothing changed. See `cached_is_text_file`.
    ///
    /// Staleness window: validity is judged purely by mtime equality, with
    /// no separate expiry. On a filesystem with coarse mtime resolution
    /// (FAT32/exFAT ~2s ticks, some network mounts), a file that's
    /// rewritten across the text/binary boundary within the same mtime
    /// tick as the cached read will keep serving the stale verdict — not
    /// just until the next refresh, but indefinitely, until some *later*
    /// write produces a mtime that actually differs from what's cached.
    /// Accepted as inherent to the mandated mtime-cache design rather than
    /// worked around.
    text_cache: HashMap<PathBuf, (SystemTime, bool)>,
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
            text_cache: HashMap::new(),
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

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    pub fn refresh(&mut self) {
        self.rebuild_keeping_selection();
    }

    /// Select the visible row at `path`; returns whether it was found.
    /// A miss leaves the selection unchanged.
    pub fn select_path(&mut self, path: &Path) -> bool {
        match self.rows.iter().position(|r| r.path == path) {
            Some(i) => {
                self.selected = i;
                true
            }
            None => false,
        }
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
        let root = self.root.clone();
        let entries = list_dir(dir, &root, self.show_hidden, &mut self.text_cache);
        for (path, is_dir) in entries {
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

    #[cfg(test)]
    pub(crate) fn text_cache_len(&self) -> usize {
        self.text_cache.len()
    }
}

/// Builds a `Gitignore` matcher from `{root}/.gitignore` only (not the
/// user's global gitignore — callers keep `git_global(false)` so tests
/// stay hermetic, and not nested `.gitignore` files below `root`, which
/// the walker in `list_dir` already applies on its own when a directory
/// containing one is itself the walk root). Matches are made against
/// paths relative to `root`, so a rule at the tree root also applies to
/// files listed several directories below it.
pub(crate) fn root_gitignore(root: &Path) -> ignore::gitignore::Gitignore {
    let (matcher, _err) = ignore::gitignore::Gitignore::new(root.join(".gitignore"));
    matcher
}

/// Whether `path` (rooted under `root`) is ignored by `ignores`, a
/// matcher built by `root_gitignore`.
pub(crate) fn is_root_ignored(
    ignores: &ignore::gitignore::Gitignore,
    root: &Path,
    path: &Path,
    is_dir: bool,
) -> bool {
    match path.strip_prefix(root) {
        Ok(rel) if !rel.as_os_str().is_empty() => {
            ignores.matched_path_or_any_parents(rel, is_dir).is_ignore()
        }
        _ => false,
    }
}

/// Sniffs `path` via `fsutil::is_text_file`, caching the verdict against
/// the file's mtime so an unchanged file is never re-read. A cache hit
/// requires both the path to be present and its stored mtime to match
/// the file's current mtime; anything else (miss, stale mtime, or a
/// failed `metadata` call) falls through to a real sniff. A failed
/// `metadata` call also skips the cache write, since there's no mtime to
/// key it on — such a path (e.g. a race with deletion) will simply be
/// re-sniffed (and just as unreadable) next time.
///
/// See the staleness-window note on `Tree::text_cache`: mtime equality is
/// the only validity check, so a coarse-resolution filesystem can serve a
/// stale verdict indefinitely if a text/binary swap lands within one
/// mtime tick of the cached read.
fn cached_is_text_file(path: &Path, cache: &mut HashMap<PathBuf, (SystemTime, bool)>) -> bool {
    let Ok(mtime) = std::fs::metadata(path).and_then(|m| m.modified()) else {
        return crate::fsutil::is_text_file(path);
    };
    if let Some(&(cached_mtime, is_text)) = cache.get(path) {
        if cached_mtime == mtime {
            return is_text;
        }
    }
    let is_text = crate::fsutil::is_text_file(path);
    cache.insert(path.to_path_buf(), (mtime, is_text));
    is_text
}

/// List one directory: dirs and text files, dirs first, each group
/// sorted case-insensitively. Honors per-directory .gitignore, plus the
/// tree root's own .gitignore (which a plain `WalkBuilder::new(dir)`
/// with `parents(false)` would otherwise miss for `dir != root`).
///
/// `cache` is `Tree::text_cache`: a (path, mtime) -> is_text cache so a
/// tree refresh doesn't re-sniff every file's contents every ~2s. After
/// listing, entries in `cache` for files directly inside `dir` that no
/// longer appear in the walk (deleted, renamed, or now gitignored) are
/// dropped, so the cache stays bounded by what's actually on disk in
/// directories the tree has visited rather than growing unboundedly.
fn list_dir(
    dir: &Path,
    root: &Path,
    show_hidden: bool,
    cache: &mut HashMap<PathBuf, (SystemTime, bool)>,
) -> Vec<(PathBuf, bool)> {
    let mut out = Vec::new();
    // Every non-dir path the walk actually considered (sniffed or served
    // from cache), text or not. This is deliberately broader than `out`
    // below, which drops binary files from the tree display — a binary
    // file that's still on disk must stay in `walked_files` so the
    // eviction pass doesn't treat its "not text" cache entry as stale
    // and force a pointless re-sniff of it on every refresh.
    let mut walked_files: HashSet<PathBuf> = HashSet::new();
    let ignores = root_gitignore(root);
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
        if is_root_ignored(&ignores, root, &path, is_dir) {
            continue;
        }
        if !is_dir {
            walked_files.insert(path.clone());
        }
        if is_dir || cached_is_text_file(&path, cache) {
            out.push((path, is_dir));
        }
    }
    cache.retain(|p, _| p.parent() != Some(dir) || walked_files.contains(p));
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
    /// Nested one level inside `mrkdup-tree-{tag}` (a directory this test
    /// owns) rather than returning that directory itself: `ascend`/`-`
    /// reroots the tree at the fixture's *parent*, and if the fixture
    /// root were the top-level temp dir entry, that parent would be the
    /// real shared system temp dir. Walking that for real hangs on
    /// GitHub's Ubuntu/macOS runners, which leave FIFOs sitting in temp
    /// (see the `is_text_file` fix) -- so every reroot/ascend test needs
    /// its fixture's parent to still be something this test created and
    /// controls.
    fn fixture(tag: &str) -> PathBuf {
        let owned = std::env::temp_dir().join(format!("mrkdup-tree-{tag}"));
        let _ = fs::remove_dir_all(&owned);
        let root = owned.join("root");
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

    /// A FIFO in the tree is listed like the existing binary-file case
    /// (`lists_dirs_first_then_files_sorted_no_binary_no_hidden` above):
    /// `is_text_file` reports it as non-text, so `list_dir` omits it —
    /// same semantics as `bin.dat`, just via a different `is_text_file`
    /// path (metadata type check vs. NUL-byte sniff). The real point of
    /// this test is that building the tree doesn't hang: opening a FIFO
    /// for reading blocks forever absent a writer, and pre-fix this walk
    /// would never return.
    #[cfg(unix)]
    #[test]
    fn dir_with_fifo_lists_fine_and_does_not_hang() {
        let root = std::env::temp_dir().join("mrkdup-tree-fifo");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.md"), "a\n").unwrap();
        let fifo = root.join("pipe");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo must be on PATH for this test");
        assert!(status.success(), "mkfifo failed");

        let t = Tree::new(root).unwrap();
        assert_eq!(names(&t), vec!["a.md"]);
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
        assert!(!t.show_hidden());
        t.toggle_hidden();
        assert!(t.show_hidden());
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
    fn select_path_selects_the_matching_row() {
        let mut t = Tree::new(fixture("selpath")).unwrap();
        let target = t.root().join("zz.md");
        assert!(t.select_path(&target));
        assert_eq!(t.selected_row().unwrap().name, "zz.md");
        // a missing path leaves the selection where it was
        assert!(!t.select_path(Path::new("/nonexistent/nope.md")));
        assert_eq!(t.selected_row().unwrap().name, "zz.md");
    }

    #[test]
    fn expand_honors_root_gitignore_for_nested_files() {
        let root = std::env::temp_dir().join("mrkdup-tree-rootignore");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::write(root.join("notes/debug.log"), "d\n").unwrap();
        fs::write(root.join("notes/keep.md"), "k\n").unwrap();

        let mut t = Tree::new(root).unwrap();
        t.expand(); // selection starts at 0 = notes
        let rows = names(&t);
        assert!(rows.contains(&"keep.md".to_string()));
        assert!(!rows.contains(&"debug.log".to_string()));
    }

    #[test]
    fn refresh_without_changes_does_not_resniff_cached_files() {
        let root = fixture("cache-hit");
        let mut t = Tree::new(root).unwrap();
        // First build already sniffed a.md, zz.md, and bin.dat (b-dir
        // isn't expanded, so its contents were never walked).
        let before = crate::fsutil::sniff_call_count();
        t.refresh();
        let after = crate::fsutil::sniff_call_count();
        assert_eq!(
            after, before,
            "unchanged files should be served from the cache, not re-sniffed"
        );
    }

    #[test]
    fn mtime_bump_forces_a_resniff() {
        let root = fixture("cache-bump");
        let mut t = Tree::new(root.clone()).unwrap();
        // Rewrite a.md's contents without changing its name, forcing a
        // new mtime. 20ms is plenty of margin on the sub-millisecond
        // mtime resolution of APFS/ext4-class filesystems, which is what
        // this test (and CI) actually runs on — it is not a guarantee on
        // coarser-resolution filesystems (FAT32/exFAT ~2s ticks, some
        // network mounts), where this same tick could in principle still
        // land within the old mtime. See the staleness-window note on
        // `Tree::text_cache` for that known limit of the cache itself.
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(root.join("a.md"), "changed\n").unwrap();

        let before = crate::fsutil::sniff_call_count();
        t.refresh();
        let after = crate::fsutil::sniff_call_count();
        assert!(
            after > before,
            "a changed mtime should invalidate the cache entry and trigger a re-sniff"
        );
    }

    #[test]
    fn deleted_file_is_evicted_from_cache() {
        let root = fixture("evict");
        let mut t = Tree::new(root.clone()).unwrap();
        // a.md, zz.md, bin.dat were sniffed and cached by the initial build.
        let before = t.text_cache_len();

        fs::remove_file(root.join("a.md")).unwrap();
        t.refresh();

        assert!(
            t.text_cache_len() < before,
            "a deleted file's cache entry should be dropped, not retained forever"
        );
        assert!(!names(&t).contains(&"a.md".to_string()));
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
