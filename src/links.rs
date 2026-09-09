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

/// The shape of one accepted `[[...]]`: `close` indexes its `]]`, `pipe`
/// the first `|` inside, `hash` the first `#` before that `|` (a `#`
/// after the `|` belongs to the alias).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WikiSpan {
    pub close: usize,
    pub pipe: Option<usize>,
    pub hash: Option<usize>,
}

/// The `[[...]]` opening at `open` in `chars[..to]`, or `None` when
/// `open` is not on a `[[`, no `]]` follows, another `[[` opens inside
/// (the innermost pair wins: `[[a [[b]]` is text plus `[[b]]`), or the
/// target — the inner text before any `|`/`#` — is empty. This is the
/// one acceptance rule: the highlighter paints exactly the spans this
/// accepts and `wikilinks` lists exactly the same, so what looks like a
/// link is what Ctrl+O follows. `[[a](b)]]` is therefore a wikilink
/// with target `a](b)` on both sides, not a markdown link.
pub fn wikilink_span(chars: &[char], open: usize, to: usize) -> Option<WikiSpan> {
    let to = to.min(chars.len());
    if open + 1 >= to || chars[open] != '[' || chars[open + 1] != '[' {
        return None;
    }
    let inner = open + 2;
    let close = (inner..to.saturating_sub(1)).find(|&j| chars[j] == ']' && chars[j + 1] == ']')?;
    if (inner..close.saturating_sub(1)).any(|j| chars[j] == '[' && chars[j + 1] == '[') {
        return None;
    }
    let pipe = (inner..close).find(|&j| chars[j] == '|');
    let pre_end = pipe.unwrap_or(close);
    let hash = (inner..pre_end).find(|&j| chars[j] == '#');
    if hash.unwrap_or(pre_end) == inner {
        return None; // empty target, e.g. `[[]]`, `[[|alias]]`, `[[#h]]`
    }
    Some(WikiSpan { close, pipe, hash })
}

/// Every wikilink in `line`, left to right, scanning the way the
/// highlighter does: after an accepted span continue past its `]]`,
/// otherwise advance one char. `start`/`end` are char indices of the
/// whole `[[...]]`.
pub fn wikilinks(line: &str) -> Vec<WikiLink> {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let text = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let Some(s) = wikilink_span(&chars, i, n) else {
            i += 1;
            continue;
        };
        let inner = i + 2;
        let pre_end = s.pipe.unwrap_or(s.close);
        let target_end = s.hash.unwrap_or(pre_end);
        out.push(WikiLink {
            target: text(inner, target_end),
            alias: s.pipe.map(|p| text(p + 1, s.close)),
            heading: s
                .hash
                .map(|h| text(h + 1, pre_end))
                .filter(|h| !h.is_empty()),
            start: i,
            end: s.close + 2,
        });
        i = s.close + 2;
    }
    out
}

/// The `[[...]]` spanning char column `col`, or `None`.
pub fn parse_wikilink_at(line: &str, col: usize) -> Option<WikiLink> {
    wikilinks(line)
        .into_iter()
        .find(|w| w.start <= col && col < w.end)
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
/// appended unless the target already ends in `.md` (a dot inside a
/// note name is not an extension: `[[v1.2]]` reaches `v1.2.md`). Every
/// entry is run through `normalize_lexical`, so no `..` survives.
/// `resolve` and `create_target` both read from this list, which is
/// what makes the path Ctrl+O opens and the path the create offer
/// prefills agree — and with it tab dedup (`tab_index`) and backlink
/// self-exclusion. Empty and bare-`/` targets name nothing.
pub fn candidates(target: &str, file_dir: &Path, root: &Path) -> Vec<PathBuf> {
    let (p, bases): (&Path, &[&Path]) = match target.strip_prefix('/') {
        Some("") => return Vec::new(),
        Some(rest) => (Path::new(rest), &[root]),
        None if target.is_empty() => return Vec::new(),
        None => (Path::new(target), &[file_dir, root]),
    };
    let add_md = !p.extension().is_some_and(|e| e == "md");
    let mut out = Vec::new();
    for base in bases {
        let joined = normalize_lexical(&base.join(p));
        if add_md {
            let mut with_md = joined.clone().into_os_string();
            with_md.push(".md");
            out.push(joined);
            out.push(PathBuf::from(with_md));
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
/// first candidate carrying the `.md` extension — the one `resolve`
/// would have found had the note existed. The caller decides whether
/// that path is inside the vault.
pub fn create_target(target: &str, file_dir: &Path, root: &Path) -> Option<PathBuf> {
    candidates(target, file_dir, root)
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "md"))
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

/// `s` as a heading anchor: trimmed, lower-cased, spaces as `-`, only
/// letters, digits and `-` kept — so `Next Steps` and the GitHub-style
/// `next-steps` compare equal.
fn slug(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// The row of the ATX heading (`#` .. `######` then a space) whose text
/// slugs equal to `heading`, or `None` — including for an empty
/// `heading`. Only heading lines count, so `[[note#Intro]]` never lands
/// on a body sentence that mentions the intro.
pub fn heading_row(lines: &[String], heading: &str) -> Option<usize> {
    let want = slug(heading);
    if want.is_empty() {
        return None;
    }
    lines.iter().position(|l| {
        let hashes = l.chars().take_while(|&c| c == '#').count();
        (1..=6).contains(&hashes) && l[hashes..].starts_with(' ') && slug(&l[hashes..]) == want
    })
}

#[cfg(test)]
mod tests;
