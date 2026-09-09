//! Obsidian-style local note linking: pure wikilink / markdown-link
//! parsing, target resolution, and backlink scanning.
//!
//! Everything here works over plain strings and paths — the only
//! filesystem touch is a caller-supplied existence predicate (or, for
//! [`scan_backlinks`], directory reads), so tests never touch disk
//! outside temp dirs.

use std::fs;
use std::path::{Path, PathBuf};

/// One `[[target]]` / `[[target|alias]]` / `[[target#heading]]`
/// occurrence in a line.
///
/// `start`/`end` are char indices of the whole `[[...]]` in the line
/// (`start` points at the first `[`, `end` is one past the second
/// `]`). `target` is the raw text before any `|`/`#`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
    pub heading: Option<String>,
    pub start: usize,
    pub end: usize,
}

/// One `[text](url)` occurrence, mirroring the shape `highlight.rs`
/// tokenizes. `start`/`end` cover the `(url)` part (`start` points at
/// `(`, `end` is one past `)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdLink {
    pub url: String,
    pub start: usize,
    pub end: usize,
}

/// The `[[...]]` spanning char column `col`, or `None`.
///
/// Finds the last `[[` at/before `col`, then the first `]]` after it;
/// `col` must lie within `open..close+2`. Unclosed `[[` and empty
/// targets (including `[[|alias]]`) yield `None`.
///
/// Splitting rule: the inner text splits on the first `|` into target
/// part + alias, then the target part splits on the first `#` into
/// target + heading — so a `#` after the `|` belongs to the alias
/// (e.g. `[[plan#sec|see #2]]` has heading `sec` and alias `see #2`).
pub fn parse_wikilink_at(line: &str, col: usize) -> Option<WikiLink> {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    if col >= n {
        return None;
    }
    let mut open = None;
    for i in 0..n.saturating_sub(1) {
        if i > col {
            break;
        }
        if chars[i] == '[' && chars[i + 1] == '[' {
            open = Some(i);
        }
    }
    let open = open?;
    let mut close = None;
    let mut j = open + 2;
    while j + 1 < n {
        if chars[j] == ']' && chars[j + 1] == ']' {
            close = Some(j);
            break;
        }
        j += 1;
    }
    let close = close?;
    if col >= close + 2 {
        return None;
    }
    let inner: String = chars[open + 2..close].iter().collect();
    let (pre, alias) = match inner.split_once('|') {
        Some((a, b)) => (a, Some(b.to_string())),
        None => (inner.as_str(), None),
    };
    let (target, heading) = match pre.split_once('#') {
        Some((t, h)) => (t, Some(h.to_string())),
        None => (pre, None),
    };
    if target.is_empty() {
        return None;
    }
    Some(WikiLink {
        target: target.to_string(),
        alias,
        heading,
        start: open,
        end: close + 2,
    })
}

/// The `[text](url)` whose `(url)` span contains char column `col`, or
/// `None`.
///
/// Uses exactly the bracket rules `highlight.rs` tokenizes with: the
/// first `]` after a `[`, immediately followed by `(`, closed by the
/// first `)` after that. (In particular a `[[wikilink]]` never parses
/// as an md link, since `]` is never followed by `(` there.)
pub fn parse_md_link_at(line: &str, col: usize) -> Option<MdLink> {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    if col >= n {
        return None;
    }
    let mut i = 0;
    while i < n {
        if chars[i] != '[' {
            i += 1;
            continue;
        }
        let Some(rb) = (i + 1..n).find(|&k| chars[k] == ']') else {
            i += 1;
            continue;
        };
        if chars.get(rb + 1) != Some(&'(') {
            i += 1;
            continue;
        }
        let Some(rp) = (rb + 2..n).find(|&k| chars[k] == ')') else {
            i += 1;
            continue;
        };
        let start = rb + 1;
        let end = rp + 1;
        if col >= start && col < end {
            let url: String = chars[rb + 2..rp].iter().collect();
            return Some(MdLink { url, start, end });
        }
        i += 1;
    }
    None
}

/// `path` with `.` popped and `..` resolved lexically (no filesystem
/// access, symlinks untouched). A `..` with nothing left to pop is kept,
/// so paths escaping the vault stay recognizable as such. Pure, so
/// resolution and creation-prefill agree on one spelling — which is what
/// makes tab dedup (`tab_index`) and backlink self-exclusion compare
/// equal paths.
pub fn normalize_lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            Component::CurDir => {}
            _ => out.push(c.as_os_str()),
        }
    }
    out
}

/// Every path `target` may name, in resolution order: under `file_dir`
/// then under `root` (under `root` only for a leading `/`, mirroring
/// `[text](/path)` links — a bare `Path::join` would discard the base
/// and reach outside the vault), each as written and then with `.md`
/// appended when the target has no extension. Every entry is run
/// through `normalize_lexical`, so no `..` survives. `resolve` and
/// `create_target` both read from this list, which is what makes the
/// path Ctrl+O opens and the path the create offer prefills agree —
/// and with it tab dedup (`tab_index`) and backlink self-exclusion.
/// Empty and bare-`/` targets name nothing.
pub fn candidates(target: &str, file_dir: &Path, root: &Path) -> Vec<PathBuf> {
    let (p, bases): (&Path, &[&Path]) = match target.strip_prefix('/') {
        Some("") => return Vec::new(),
        Some(rest) => (Path::new(rest), &[root]),
        None if target.is_empty() => return Vec::new(),
        None => (Path::new(target), &[file_dir, root]),
    };
    let add_md = p.extension().is_none();
    let mut out = Vec::new();
    for base in bases {
        let joined = normalize_lexical(&base.join(p));
        if add_md {
            let mut with_md = joined.clone();
            with_md.set_extension("md");
            out.push(joined);
            out.push(with_md);
        } else {
            out.push(joined);
        }
    }
    out
}

/// The first of `candidates` for which `exists` holds. No sandboxing
/// beyond the vault: a `..` that leaves `root` still resolves if the
/// file exists (same as the tree's `-` ascend ethos).
pub fn resolve(
    target: &str,
    file_dir: &Path,
    root: &Path,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    candidates(target, file_dir, root)
        .into_iter()
        .find(|p| exists(p))
}

/// Where following `target` creates a note when nothing resolves: the
/// first candidate carrying the `.md` extension (the one `resolve`
/// would have found had the note existed), or the first candidate at
/// all when the target spells its own extension. The caller decides
/// whether that path is inside the vault.
pub fn create_target(target: &str, file_dir: &Path, root: &Path) -> Option<PathBuf> {
    let cands = candidates(target, file_dir, root);
    cands
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "md"))
        .or(cands.first())
        .cloned()
}

/// True when `content` links to the note named `stem`: it contains
/// `[[<stem>]]`, `[[<stem>|`, or `[[<stem>#`. Case-sensitive.
pub fn backlink_matches(content: &str, stem: &str) -> bool {
    content.contains(&format!("[[{stem}]]"))
        || content.contains(&format!("[[{stem}|"))
        || content.contains(&format!("[[{stem}#"))
}

/// All text files under `root` linking to the note named `stem`,
/// sorted, capped at 5000.
///
/// Uses the same `ignore::WalkBuilder` flags as
/// `fuzzy::collect_candidates` (hidden/git_ignore/git_exclude
/// toggles, `require_git(false)`, `git_global(false)`,
/// `parents(false)`, skip `.git`), the same cap, the same
/// `fsutil::is_text_file` gate; unreadable files are skipped.
pub(crate) fn scan_backlinks(root: &Path, show_hidden: bool, stem: &str) -> Vec<PathBuf> {
    const CAP: usize = 5000;
    let mut out = Vec::new();
    let ignores = crate::tree::root_gitignore(root);
    let walker = ignore::WalkBuilder::new(root)
        .hidden(!show_hidden)
        .git_ignore(!show_hidden)
        .git_exclude(!show_hidden)
        .require_git(false)
        .git_global(false)
        .parents(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walker.flatten() {
        if out.len() >= CAP {
            break;
        }
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.into_path();
        if !show_hidden && crate::tree::is_root_ignored(&ignores, root, &path, false) {
            continue;
        }
        if !crate::fsutil::is_text_file(&path) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if backlink_matches(&content, stem) {
            out.push(path);
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests;
