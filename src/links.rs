//! Obsidian-style local note linking: pure wikilink / markdown-link
//! parsing, target resolution, and backlink scanning.
//!
//! Everything here works over plain strings and paths — the only
//! filesystem touch is a caller-supplied existence predicate (or, for
//! [`backlinks`], the file walk and the notes it reads), so tests
//! never touch disk outside temp dirs.

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

/// Cheap pre-check before `resolve`: a link can only reach `target` if
/// its last path segment is `target`'s file name, with or without the
/// `.md`, ignoring ASCII case (case-insensitive disks resolve `[[B]]`
/// to `b.md`). No filesystem access.
///
/// Only ever a pre-filter, so it errs permissive: a false positive
/// costs one wasted `resolve` (which, with the path equality, still
/// decides the answer) while a false negative would silently lose a
/// real backlink. So the segment compared is the last *non-empty* one
/// — `[[b/]]` reaches `b.md`, since `candidates` normalizes the empty
/// trailing component away — and a `..` tail, which names a directory
/// this function cannot put a name to, is let through.
fn may_name(link_target: &str, target: &Path) -> bool {
    let Some(name) = target.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(last) = link_target
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    last == ".."
        || last.eq_ignore_ascii_case(name)
        || name
            .strip_suffix(".md")
            .is_some_and(|stem| last.eq_ignore_ascii_case(stem))
}

/// The notes under `root` linking to `target`: every `[[...]]` in every
/// text file (`wikilinks`) is resolved from that file's directory
/// exactly as Ctrl+O would (`resolve`, then the on-disk spelling), so
/// `[[b]]`, `[[notes/b]]`, `[[b.md]]`, `[[/b]]` and `[[../b]]` all
/// count and a `[[b]]` that reaches a different `b.md` does not. Walks
/// the same files as the go-to-file picker — `fuzzy::collect_candidates`,
/// same ignore rules, same 5000-file cap — skips `target` itself, and
/// returns (root-relative display, absolute path) in the picker's order.
/// Files that fail to read as UTF-8 are skipped.
pub(crate) fn backlinks(root: &Path, show_hidden: bool, target: &Path) -> Vec<(String, PathBuf)> {
    let exists = |p: &Path| p.is_file();
    crate::fuzzy::collect_candidates(root, show_hidden)
        .into_iter()
        .filter(|(_, path)| path != target)
        .filter(|(_, path)| {
            let Ok(content) = std::fs::read_to_string(path) else {
                return false;
            };
            if !content.contains("[[") {
                return false;
            }
            let dir = path.parent().unwrap_or(root);
            content.lines().flat_map(wikilinks).any(|w| {
                may_name(&w.target, target)
                    && resolve(&w.target, dir, root, &exists)
                        .map(crate::fsutil::canonical)
                        .is_some_and(|p| p == target)
            })
        })
        .collect()
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

/// True for anything with a scheme (`://` anywhere) or a `mailto:`
/// prefix, case-insensitively. Everything else is treated as a local
/// path.
pub fn is_remote_url(url: &str) -> bool {
    url.contains("://") || url.to_ascii_lowercase().starts_with("mailto:")
}

/// Split a markdown link url into (percent-decoded path, fragment). The
/// fragment is the text after the first `#`, without the `#`; `None`
/// when there is none or it is empty. `[t](#heading)` yields an empty
/// path.
pub fn split_md_url(url: &str) -> (String, Option<String>) {
    let (path, fragment) = match url.split_once('#') {
        Some((p, f)) => (p, Some(f).filter(|f| !f.is_empty())),
        None => (url, None),
    };
    (percent_decode(path), fragment.map(str::to_string))
}

/// `%XX` escapes decoded as bytes, then read as UTF-8; anything that is
/// not two hex digits after a `%`, or does not form valid UTF-8, is
/// kept as written.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .filter(|h| bytes[i] == b'%' && h.iter().all(u8::is_ascii_hexdigit))
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(b) => {
                out.push(b);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests;
