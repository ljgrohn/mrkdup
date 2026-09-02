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

    /// Select row `i` directly (a mouse click). Returns `false`, leaving
    /// the selection alone, when there is no such row.
    pub fn select(&mut self, i: usize) -> bool {
        if i < self.rows.len() {
            self.selected = i;
            true
        } else {
            false
        }
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
/// `show_hidden` is the tree's one "show everything" switch: it reveals
/// dotfiles *and* gitignored files together.
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
        .git_ignore(!show_hidden)
        .git_exclude(!show_hidden)
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
        if !show_hidden && is_root_ignored(&ignores, root, &path, is_dir) {
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
mod tests;
