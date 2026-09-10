//! File-operation glue: create, rename, move, and delete files on disk
//! while keeping `Tree` and the open tabs (their editors' `path`) in
//! sync. `App` owns the prompt/focus/status bookkeeping around these
//! calls — these functions never see `&mut App`.

use std::path::{Path, PathBuf};

use crate::tab::Tab;
use crate::tree::Tree;

/// Where `n` creates: inside the selected directory, beside the
/// selected file, or at the root with nothing selected.
pub fn selected_dir(tree: &Tree) -> PathBuf {
    match tree.selected_row() {
        Some(r) if r.is_dir => r.path.clone(),
        Some(r) => r.path.parent().unwrap_or(tree.root()).to_path_buf(),
        None => tree.root().to_path_buf(),
    }
}

/// Create an empty file named `name` (a relative path, no `..`) inside
/// `base` — `selected_dir(tree)` for the `n` key, the vault root for a
/// link-follow create offer — and refresh `tree`. The caller opens the
/// new file. Writes go through the atomic temp-file+rename path.
pub fn create(tree: &mut Tree, base: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('/') || name.split('/').any(|part| part == "..") {
        return Err("invalid file name".into());
    }
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

/// Rename `src` to `name` within its own directory, redirecting the
/// open path of any tab that pointed at `src`. `Ok(None)` means the
/// name was unchanged (or `src` had no parent — not expected for a
/// tree-selected file) and nothing happened.
pub fn rename(
    tree: &mut Tree,
    tabs: &mut [Tab],
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
    // the tab's spelling of `src`, captured while `src` still resolves
    let open_as = crate::fsutil::canonical(src.to_path_buf());
    std::fs::rename(src, &target).map_err(|e| format!("rename failed: {e}"))?;
    redirect(tabs, &open_as, &target);
    // refresh tracks selection by the old (gone) path, so reselect
    tree.refresh();
    tree.select_path(&target);
    Ok(Some(format!("renamed to {name}")))
}

/// Move `src` into `dest_dir`, redirecting the open path of any tab
/// that pointed at `src`. `Ok(None)` means `src` had no file name — not
/// expected for a tree-selected file — and nothing happened.
pub fn move_to(
    tree: &mut Tree,
    tabs: &mut [Tab],
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
    // The tab's spelling of `src`, captured while `src` still
    // resolves. `fs::rename` moves a symlink and leaves its target
    // alone, so for a symlink row the canonical path is the *wrong*
    // thing to match: it names the target, and redirecting on it would
    // drag the tab holding the real file onto the moved link (which,
    // if relative, now dangles). Match the raw spelling instead, so
    // only a tab opened at that exact link path is redirected. An
    // unreadable metadata is treated as "assume symlink" — the
    // conservative choice, since it claims no match with the target.
    let is_symlink = src
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(true);
    let open_as = if is_symlink {
        src.to_path_buf()
    } else {
        crate::fsutil::canonical(src.to_path_buf())
    };
    std::fs::rename(src, &target).map_err(|e| format!("move failed: {e}"))?;
    redirect(tabs, &open_as, &target);
    tree.refresh();
    let shown = crate::fuzzy::rel_display(tree.root(), &target);
    Ok(Some(format!("moved to {shown}")))
}

/// Delete `path`. The caller drops the tab that had it open (`App`
/// owns the active-tab bookkeeping that goes with that).
pub fn delete(tree: &mut Tree, path: &Path) -> Result<String, String> {
    std::fs::remove_file(path).map_err(|e| format!("delete failed: {e}"))?;
    tree.refresh();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    Ok(format!("deleted {name}"))
}

/// Point every tab that has `from` open at `to` instead.
///
/// Tab paths are canonical (`App::open_file` stores them that way), so
/// `to` is canonicalized here, keeping a redirected tab stored under
/// the spelling `tab_index` dedups on. `from` is whatever spelling the
/// caller captured *before* the move — afterwards the old path no
/// longer resolves — and the two callers capture it differently,
/// because `fs::rename` moves a symlink and leaves its target alone:
///
/// - `rename` always passes `canonical(src)`. The link stays in its
///   own directory, so for a symlink row that canonical path is the
///   target both before and after; the match is exact and the tab is
///   rewritten to the same value it already had. Renaming the real
///   file redirects the tab, whether it was opened through the link or
///   at the file.
/// - `move_to` passes `canonical(src)` only when `src` is not itself a
///   symlink, and the raw `src` when it is. Moving a symlink therefore
///   redirects only a tab opened at that exact link spelling (which
///   `open_file` never produces) and leaves a tab holding the real
///   file pointing at live content. Moving the real file redirects the
///   tab as usual.
fn redirect(tabs: &mut [Tab], from: &Path, to: &Path) {
    let to = crate::fsutil::canonical(to.to_path_buf());
    for tab in tabs {
        if tab.editor.path.as_deref() == Some(from) {
            tab.editor.path = Some(to.clone());
        }
    }
}

#[cfg(test)]
mod tests;
