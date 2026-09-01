//! File-operation glue: create, rename, move, and delete files on disk
//! while keeping `Tree` and `Editor` (its `path`) in sync. `App` owns
//! the prompt/focus/status bookkeeping around these calls — these
//! functions never see `&mut App`.

use std::path::{Path, PathBuf};

use crate::editor::Editor;
use crate::tree::Tree;

/// Create an empty file named `name`, resolved against the tree's
/// current selection (inside a selected directory, or alongside a
/// selected file; at the root with nothing selected). Refreshes `tree`
/// on success; the caller is responsible for opening the new file.
pub fn create(tree: &mut Tree, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('/') || name.split('/').any(|part| part == "..") {
        return Err("invalid file name".into());
    }
    let base = match tree.selected_row() {
        Some(r) if r.is_dir => r.path.clone(),
        Some(r) => r.path.parent().unwrap_or(tree.root()).to_path_buf(),
        None => tree.root().to_path_buf(),
    };
    let path = base.join(name);
    if path.exists() {
        return Err("file already exists".into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create failed: {e}"))?;
    }
    crate::fsutil::atomic_write(&path, b"").map_err(|e| format!("create failed: {e}"))?;
    tree.refresh();
    Ok(path)
}

/// Rename `src` to `name` within its own directory, redirecting
/// `editor`'s open path if it pointed at `src`. `Ok(None)` means the
/// name was unchanged (or `src` had no parent — not expected for a
/// tree-selected file) and nothing happened.
pub fn rename(
    tree: &mut Tree,
    editor: &mut Editor,
    src: &Path,
    name: &str,
) -> Result<Option<String>, String> {
    if name.is_empty() || name.contains('/') || name == ".." {
        return Err("invalid file name".into());
    }
    let Some(dir) = src.parent() else {
        return Ok(None);
    };
    let target = dir.join(name);
    if target == src {
        return Ok(None); // unchanged
    }
    // on case-insensitive filesystems (macOS default) a case-only
    // rename makes target "exist" — but it's the same file, allow it
    let same_file =
        target.exists() && std::fs::canonicalize(&target).ok() == std::fs::canonicalize(src).ok();
    if target.exists() && !same_file {
        return Err("a file with that name already exists".into());
    }
    std::fs::rename(src, &target).map_err(|e| format!("rename failed: {e}"))?;
    if editor.path.as_deref() == Some(src) {
        editor.path = Some(target.clone());
    }
    // refresh tracks selection by the old (gone) path, so reselect
    tree.refresh();
    tree.select_path(&target);
    Ok(Some(format!("renamed to {name}")))
}

/// Move `src` into `dest_dir`, redirecting `editor`'s open path if it
/// pointed at `src`. `Ok(None)` means `src` had no file name — not
/// expected for a tree-selected file — and nothing happened.
pub fn move_to(
    tree: &mut Tree,
    editor: &mut Editor,
    src: &Path,
    dest_dir: &Path,
) -> Result<Option<String>, String> {
    let Some(name) = src.file_name() else {
        return Ok(None);
    };
    let target = dest_dir.join(name);
    if target == src {
        return Err("already there".into());
    }
    if target.exists() {
        return Err("a file with that name is already there".into());
    }
    std::fs::rename(src, &target).map_err(|e| format!("move failed: {e}"))?;
    if editor.path.as_deref() == Some(src) {
        editor.path = Some(target.clone());
    }
    tree.refresh();
    let shown = crate::fuzzy::rel_display(tree.root(), &target);
    Ok(Some(format!("moved to {shown}")))
}

/// Delete `path`, resetting `editor` to empty if it had the file open.
/// The caller is responsible for its own bookkeeping (e.g. an
/// `App::last_edit` timestamp) tied to that reset.
pub fn delete(tree: &mut Tree, editor: &mut Editor, path: &Path) -> Result<String, String> {
    std::fs::remove_file(path).map_err(|e| format!("delete failed: {e}"))?;
    if editor.path.as_deref() == Some(path) {
        *editor = Editor::new();
    }
    tree.refresh();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    Ok(format!("deleted {name}"))
}

#[cfg(test)]
mod tests;
