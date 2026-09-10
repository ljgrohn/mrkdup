# Wikilinks Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the review findings on PR #2 (`wikilinks` branch) as one small, independently reviewable commit per fix, consolidating the duplicated glue first so the correctness fixes build on shared pieces.

**Architecture:** `src/links.rs` stays the one pure home for link parsing and resolution and grows three small shared pieces: a `candidates` list that both `resolve` and the create-prefill read from, a `wikilink_span` acceptance rule that both the highlighter and the Ctrl+O parser build on, and a `backlinks` function built on `fuzzy::collect_candidates` plus parse-and-resolve. `app.rs` glue shrinks: the backlinks popup becomes the existing go-to-file picker, and the second new-file prompt variant folds back into the first. Every task is TDD: failing test, minimal code, checks green, commit.

**Tech Stack:** Rust, `ratatui` + `crossterm` + `ratatui-textarea`, `ignore` crate for walking. No new crates.

**Spec:** The review findings for PR #2, listed under `## Findings (the spec)` below. There is no separate design document; the findings are the authority.

## Global Constraints

- **No new crates.** `Cargo.toml` dependencies do not change.
- **Colors live in `src/theme.rs` only.** No `Color::` literals in `ui.rs`, `render.rs`, or `highlight.rs`.
- **Logic goes in testable modules; `src/ui.rs` and `src/main.rs` stay thin glue.** `src/app.rs` dispatches keys.
- **Tests live beside the module** in `src/<module>/tests.rs`, declared `#[cfg(test)] mod tests;` at the bottom of `src/<module>.rs`.
- **README in the same commit.** Any change that alters observable behavior (a key's behavior, a message, resolution rules) updates `README.md` in that same commit. Re-read the touched README section before committing.
- **Checks before every commit:** `cargo fmt --check && cargo clippy -- -D warnings && cargo test` must all pass. Run them from the repository root of the worktree you are in.
- **One commit per task.** Message format: `<type>: <short summary>` where type is `fix`, `refactor`, or `docs`, then a blank line, then exactly this trailer: `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
- **Never lose user data; modeless keys.** No new modes, no new state files.
- **Keep messages user-facing and unchanged unless a task says otherwise.** Existing status strings are asserted by tests; when a task changes one, it lists the new exact string.
- **Do not dispatch subagents.** Implementers work alone; review comes from the controller.

## Findings (the spec)

Design (simplicity / composability):

- D1. `scan_backlinks` is a verbatim copy of the walker in `fuzzy::collect_candidates`.
- D2. `Prompt::Backlinks` is a third hand-rolled list popup; it is a go-to-file picker with pre-filtered candidates.
- D3. `Prompt::NewFileAt` duplicates `Prompt::NewFile`; `files::create` and `files::create_in` duplicate each other.
- D4. `follow_wikilink` rebuilds `resolve`'s candidate path by hand to compute the create prefill.
- D5. `follow_md_url` strips a leading `/` that `resolve` already handles.
- D6. Stale `#![allow(dead_code)]` and "Task 1, no callers yet" comment in `links.rs`.
- D7. `highlight.rs` re-implements the wikilink validity rule; `WikiLink.start`/`end` exist for this and are unused.
- D8. `docs/ROADMAP.md` has a "Proposed" section whose only item is checked off.

Correctness:

- C1. Following `[[Note]]` when `note.md` is open opens a second tab on the same file on case-insensitive filesystems (and via symlinks); two buffers then race on autosave. Canonicalize before opening.
- C2. `open_file` returns `()`, so a failed open (non-UTF-8, unreadable) still runs the heading jump in the source tab and overwrites the "open failed" status.
- C3. `.md` fallback is gated on `Path::extension().is_none()`, so `[[v1.2]]` never tries `v1.2.md` and offers to create an extension-less file.
- C4. The backlink scan is unbounded (cap counts matches, not files) and runs on the event loop.
- C5. Backlink matching by stem misses `[[notes/b]]`, `[[b.md]]`, `[[/b]]`, `[[../notes/b]]` and gives false positives when two notes share a stem.
- C6. The "no note 'X' — Enter creates Y, Esc cancels" status never renders: the status-bar arm for the new-file prompt ignores `app.status`.
- C7. The heading jump is a substring search over every line, not a heading match; `[[b#]]` produces "heading '' not found".
- C8. The heading jump does not set `follow_cursor` and ignores `set_cursor`'s bool, so a scrolled target keeps a stale viewport.
- C9. `[text](spec.md#goals)` and `[x](my%20note.md)` never resolve: no fragment split, no percent-decoding. The remote-scheme check misses `ftp:`, `file:`, uppercase.
- C10. Highlighter anchors at the first `[[`, parser at the last `[[` before the cursor; on `[[a [[b]]` they disagree about what the painted span follows.
- C11. README does not say the cursor must be on the path part of a `[text](path)` link.

Deferred (ruled out of scope, recorded here): Windows backslash / drive-letter validation in `files::create` (pre-existing, moved verbatim); `[[b]]` inside fenced code counting as a backlink; markdown `[t](b.md)` links counting as backlinks.

---

### Task 1: One candidate list behind `resolve` and the create prefill (D4, D5, D6)

**Files:**
- Modify: `src/links.rs` (remove the `#![allow(dead_code)]` block and its two comment lines; replace `resolve`; add `candidates`, `create_target`)
- Modify: `src/app.rs` (`follow_wikilink` miss branch, `follow_md_url`)
- Test: `src/links/tests.rs`

**Interfaces:**
- Consumes: `normalize_lexical(&Path) -> PathBuf` (exists), `crate::fuzzy::rel_display(root, path) -> String` (exists).
- Produces: `pub fn candidates(target: &str, file_dir: &Path, root: &Path) -> Vec<PathBuf>`; `pub fn resolve(target: &str, file_dir: &Path, root: &Path, exists: &dyn Fn(&Path) -> bool) -> Option<PathBuf>` (same signature as today); `pub fn create_target(target: &str, file_dir: &Path, root: &Path) -> Option<PathBuf>`. Task 2 changes the `.md` rule inside `candidates`; Tasks 8 and 12 call `resolve`.

No user-visible behavior changes in this task. Every existing test must still pass unchanged.

- [ ] **Step 1: Write the failing tests** (append to `src/links/tests.rs`)

```rust
#[test]
fn candidates_lists_dir_then_root_each_raw_then_md() {
    let root = Path::new("/vault");
    let dir = Path::new("/vault/notes");
    assert_eq!(
        candidates("plan", dir, root),
        vec![
            PathBuf::from("/vault/notes/plan"),
            PathBuf::from("/vault/notes/plan.md"),
            PathBuf::from("/vault/plan"),
            PathBuf::from("/vault/plan.md"),
        ]
    );
    // a leading `/` anchors at the root only; `..` is normalized away
    assert_eq!(
        candidates("/abs", dir, root),
        vec![PathBuf::from("/vault/abs"), PathBuf::from("/vault/abs.md")]
    );
    assert_eq!(
        candidates("../sib/c", dir, root)[0],
        PathBuf::from("/vault/sib/c")
    );
    assert!(candidates("", dir, root).is_empty());
    assert!(candidates("/", dir, root).is_empty());
}

#[test]
fn create_target_is_the_md_candidate_resolve_would_have_found() {
    let root = Path::new("/vault");
    let dir = Path::new("/vault/notes");
    assert_eq!(
        create_target("new", dir, root),
        Some(PathBuf::from("/vault/notes/new.md"))
    );
    assert_eq!(
        create_target("/new", dir, root),
        Some(PathBuf::from("/vault/new.md"))
    );
    assert_eq!(
        create_target("../sib/new", dir, root),
        Some(PathBuf::from("/vault/sib/new.md"))
    );
    // an explicit extension is kept as written
    assert_eq!(
        create_target("notes.txt", dir, root),
        Some(PathBuf::from("/vault/notes/notes.txt"))
    );
    assert_eq!(create_target("/", dir, root), None);
}
```

Add `use std::path::PathBuf;` at the top of `src/links/tests.rs` next to the existing `use std::path::Path;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test links::tests::candidates_ links::tests::create_target_`
Expected: compile error, `candidates` and `create_target` not found.

- [ ] **Step 3: Implement**

In `src/links.rs`, delete these three lines near the top:

```rust
// Task 1 of the wikilinks plan: no callers yet (highlight/app wire up
// in later tasks), so nothing here is reachable from `main`.
#![allow(dead_code)]
```

Replace the whole `resolve` function (doc comment included) with:

```rust
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
```

In `src/app.rs`, replace the `else` branch of `follow_wikilink` (everything from `} else {` after the `if let Some(path) = crate::links::resolve(...)` block, through its closing brace) with:

```rust
        } else {
            // creation is anchored at the vault root via `NewFileAt`, so
            // the file lands where the prefill says whatever the tree
            // selection is
            match crate::links::create_target(target, file_dir, root) {
                Some(would_be) if would_be.starts_with(root) => {
                    let prefill = crate::fuzzy::rel_display(root, &would_be);
                    self.status = Some(format!(
                        "no note '{target}' — Enter creates {prefill}, Esc cancels"
                    ));
                    self.prompt = Prompt::NewFileAt {
                        input: prefill,
                        dir: root.to_path_buf(),
                    };
                }
                Some(_) => {
                    self.status = Some(format!("can't create '{target}' outside the vault"));
                }
                None => self.status = Some(format!("can't create '{target}'")),
            }
        }
```

In `follow_md_url`, replace the `let hit = if let Some(stripped) = url.strip_prefix('/') { ... } else { ... };` statement with:

```rust
        let hit = crate::links::resolve(url, file_dir, root, &exists);
```

and update its doc comment's second sentence to: `absolute `/...` paths anchor at the tree root (handled inside `resolve`).`

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass. If clippy now reports dead code in `links.rs`, delete the dead item rather than re-adding the allow, and say so in your report.

- [ ] **Step 5: Commit**

```bash
git add src/links.rs src/links/tests.rs src/app.rs
git commit -m "refactor: one candidate list behind resolve and the create prefill

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Try `.md` whenever the target does not already end in `.md` (C3)

**Files:**
- Modify: `src/links.rs` (`candidates`, `create_target`)
- Modify: `README.md` (intro sentence about wikilinks, line ~24)
- Test: `src/links/tests.rs`, `src/app/tests.rs`

**Interfaces:**
- Consumes: `candidates`, `create_target` from Task 1.
- Produces: same signatures; `candidates` now always yields a raw and a `.md` entry per base unless the target already has the `md` extension. `create_target` becomes "first candidate with the `md` extension".

- [ ] **Step 1: Write the failing tests**

Append to `src/links/tests.rs`:

```rust
#[test]
fn dots_in_a_note_name_are_not_an_extension() {
    let root = Path::new("/vault");
    let dir = Path::new("/vault/notes");
    let exists = |p: &Path| p == Path::new("/vault/notes/v1.2.md");
    assert_eq!(
        resolve("v1.2", dir, root, &exists).unwrap(),
        Path::new("/vault/notes/v1.2.md")
    );
    assert_eq!(
        create_target("v1.2", dir, root),
        Some(PathBuf::from("/vault/notes/v1.2.md"))
    );
    // an explicit `.md` is never doubled
    assert_eq!(
        candidates("plan.md", dir, root),
        vec![
            PathBuf::from("/vault/notes/plan.md"),
            PathBuf::from("/vault/plan.md"),
        ]
    );
    // a real file with another extension still resolves as written
    let exists = |p: &Path| p == Path::new("/vault/notes/photo.png");
    assert_eq!(
        resolve("photo.png", dir, root, &exists).unwrap(),
        Path::new("/vault/notes/photo.png")
    );
}
```

Update the earlier `create_target_is_the_md_candidate_resolve_would_have_found` test: the `notes.txt` assertion becomes

```rust
    // no `.md` yet: creation appends one, like Obsidian
    assert_eq!(
        create_target("notes.txt", dir, root),
        Some(PathBuf::from("/vault/notes/notes.txt.md"))
    );
```

Append to `src/app/tests.rs` after `ctrl_o_root_fallback_link_opens_root_file`:

```rust
#[test]
fn ctrl_o_note_name_with_a_dot_opens_its_md_file() {
    let root = link_vault("dotname", "see [[v1.2]]\n");
    fs::write(root.join("notes/v1.2.md"), "release\n").unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/v1.2.md").as_path())
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test dots_in_a_note_name ctrl_o_note_name_with_a_dot create_target_is_the_md`
Expected: FAIL (`v1.2` is probed raw only; the app test offers creation instead of opening).

- [ ] **Step 3: Implement**

In `candidates`, replace `let add_md = p.extension().is_none();` with

```rust
    let add_md = !p.extension().is_some_and(|e| e == "md");
```

and replace the `set_extension` pair with an append (so `v1.2` becomes `v1.2.md`, not `v1.md`):

```rust
        if add_md {
            let mut with_md = joined.clone().into_os_string();
            with_md.push(".md");
            out.push(joined);
            out.push(PathBuf::from(with_md));
        } else {
            out.push(joined);
        }
```

Update the `candidates` doc comment: "each as written and then with `.md` appended unless the target already ends in `.md` (a dot inside a note name is not an extension: `[[v1.2]]` reaches `v1.2.md`)".

Simplify `create_target` to:

```rust
/// Where following `target` creates a note when nothing resolves: the
/// first candidate carrying the `.md` extension — the one `resolve`
/// would have found had the note existed. The caller decides whether
/// that path is inside the vault.
pub fn create_target(target: &str, file_dir: &Path, root: &Path) -> Option<PathBuf> {
    candidates(target, file_dir, root)
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "md"))
}
```

In `README.md`, replace the line

```
`[[wikilinks]]` highlight like links; `Ctrl+O` follows, `Ctrl+L` shows backlinks.
```

with

```
`[[wikilinks]]` highlight like links. `Ctrl+O` follows one: `[[note]]` opens
`note.md` beside the current file, else under the root (a leading `/` anchors
at the root; `.md` is assumed unless the name already ends in it). `Ctrl+L`
shows backlinks.
```

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/links.rs src/links/tests.rs src/app/tests.rs README.md
git commit -m "fix: resolve [[v1.2]] to v1.2.md — a dot in a note name is not an extension

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: One wikilink tokenizer shared by the highlighter and the parser (C10, D7)

**Files:**
- Modify: `src/links.rs` (add `WikiSpan`, `wikilink_span`, `wikilinks`; rewrite `parse_wikilink_at` on top of them)
- Modify: `src/highlight.rs` (the `// [[wikilink]]` block inside `inline_based`)
- Test: `src/links/tests.rs`, `src/highlight/tests.rs`

**Interfaces:**
- Produces: `pub struct WikiSpan { pub close: usize, pub pipe: Option<usize>, pub hash: Option<usize> }`; `pub fn wikilink_span(chars: &[char], open: usize, to: usize) -> Option<WikiSpan>`; `pub fn wikilinks(line: &str) -> Vec<WikiLink>`; `parse_wikilink_at(line, col)` keeps its signature. Task 12 consumes `wikilinks`.

Rule (both sides): scan left to right; at each `[[` accept the span if a `]]` follows, no other `[[` opens inside it, and the target (inner text before any `|` or `#`) is non-empty; after an accepted span continue after its `]]`, otherwise advance one char. The innermost `[[` therefore wins on `[[a [[b]]`. `[[a](b)]]` is a wikilink with target `a](b)` on both sides — consistent, and documented.

- [ ] **Step 1: Write the failing tests**

Append to `src/links/tests.rs`:

```rust
#[test]
fn nested_open_brackets_make_the_inner_link_win() {
    // the outer `[[` is plain text; only `[[b]]` is a link
    let line = "[[a [[b]]";
    let all = wikilinks(line);
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].target.as_str(), all[0].start, all[0].end), ("b", 4, 9));
    assert!(parse_wikilink_at(line, 2).is_none()); // on `a`
    assert_eq!(parse_wikilink_at(line, 6).unwrap().target, "b");
    // `[[[[b]]`: the first accepted open is index 1 (`[[[b]]`, target `[`)
    let all = wikilinks("[[[[b]]");
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].start, all[0].end), (1, 7));
}

#[test]
fn wikilinks_lists_every_link_in_a_line_in_order() {
    let all = wikilinks("[[a]] and [[b|B]] and [[c#H]] and [[oops");
    let targets: Vec<&str> = all.iter().map(|w| w.target.as_str()).collect();
    assert_eq!(targets, ["a", "b", "c"]);
    assert_eq!(all[1].alias.as_deref(), Some("B"));
    assert_eq!(all[2].heading.as_deref(), Some("H"));
    assert!(wikilinks("no links here").is_empty());
}

#[test]
fn wikilink_span_accepts_exactly_what_the_parser_accepts() {
    for line in ["a [[plan]] b", "a [[plan|P]] b", "a [[p#H]] b", "[[a](b)]]"] {
        let chars: Vec<char> = line.chars().collect();
        let open = chars.windows(2).position(|w| w == ['[', '[']).unwrap();
        let span = wikilink_span(&chars, open, chars.len()).unwrap();
        let link = parse_wikilink_at(line, open).unwrap();
        assert_eq!((link.start, link.end), (open, span.close + 2), "{line}");
    }
    for line in ["a [[oops b", "a [[]] b", "a [[|alias]] b", "a [[#h]] b"] {
        let chars: Vec<char> = line.chars().collect();
        let open = chars.windows(2).position(|w| w == ['[', '[']).unwrap();
        assert!(wikilink_span(&chars, open, chars.len()).is_none(), "{line}");
    }
}
```

Replace the body of `wikilink_highlight_agrees_with_parser` in `src/highlight/tests.rs` with a structural comparison: every `Mark` span that is exactly a `[[` must start a link the parser lists, and vice versa.

```rust
fn wikilink_highlight_agrees_with_parser() {
    let lines = [
        "a [[plan]] b",
        "a [[plan|P]] b",
        "a [[p#H]] b",
        "[[a [[b]]",
        "[[[[b]]",
        "[[a](b)]]",
        "x [[a]] y [[b|B]] z",
        "a [[oops b",
        "a [[]] b",
        "a [[|alias]] b",
    ];
    for line in lines {
        let chars: Vec<char> = line.chars().collect();
        let spans = &hl(&[line])[0];
        let painted_opens: Vec<usize> = spans
            .iter()
            .filter(|t| {
                t.kind == Kind::Mark
                    && t.end == t.start + 2
                    && chars[t.start] == '['
                    && chars[t.start + 1] == '['
            })
            .map(|t| t.start)
            .collect();
        let parsed_opens: Vec<usize> = crate::links::wikilinks(line)
            .iter()
            .map(|w| w.start)
            .collect();
        assert_eq!(painted_opens, parsed_opens, "{line}");
    }
}
```

(`hl` and `Kind` are the helpers the existing highlight tests already use; keep the surrounding tests as they are.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test wikilink`
Expected: compile errors for `wikilinks` / `wikilink_span`.

- [ ] **Step 3: Implement**

In `src/links.rs`, replace `parse_wikilink_at` (doc comment included) with:

```rust
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
            heading: s.hash.map(|h| text(h + 1, pre_end)),
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
```

Keep the `WikiLink` struct and its doc comment as they are.

In `src/highlight.rs`, replace the whole `// [[wikilink]] / [[target|alias]] / [[target#heading]]` block (from that comment line through the closing `}` of its `if c == '[' && ...` statement) with:

```rust
        // [[wikilink]] / [[target|alias]] / [[target#heading]]: the
        // acceptance rule is links::wikilink_span, shared with the Ctrl+O
        // parser so painted spans are exactly what following acts on
        if c == '[' {
            if let Some(s) = crate::links::wikilink_span(chars, i, to) {
                flush(&mut spans, text_start, i);
                spans.push(tok(i, i + 2, Kind::Mark));
                match s.pipe {
                    Some(p) => {
                        spans.push(tok(i + 2, p, Kind::LinkText));
                        spans.push(tok(p, p + 1, Kind::Mark));
                        if p + 1 < s.close {
                            spans.push(tok(p + 1, s.close, Kind::LinkText));
                        }
                    }
                    None => spans.push(tok(i + 2, s.close, Kind::LinkText)),
                }
                spans.push(tok(s.close, s.close + 2, Kind::Mark));
                i = s.close + 2;
                text_start = i;
                continue;
            }
        }
```

(`p > i + 2` always holds because the target is non-empty, so the first `LinkText` span is never empty.)

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass. The editor test `cursor_col_is_a_char_index_the_wikilink_parser_accepts` and every existing `parse_wikilink_at` test must pass unchanged.

- [ ] **Step 5: Commit**

```bash
git add src/links.rs src/links/tests.rs src/highlight.rs src/highlight/tests.rs
git commit -m "fix: one wikilink tokenizer for highlighting and following

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Open the filesystem's own spelling of a followed link (C1)

**Files:**
- Modify: `src/fsutil.rs` (add `canonical`)
- Modify: `src/app.rs` (`follow_wikilink`, `follow_md_url`)
- Test: `src/app/tests.rs`

**Interfaces:**
- Produces: `pub(crate) fn canonical(path: PathBuf) -> PathBuf` in `fsutil.rs`. Tasks 8 and 12 use it.

- [ ] **Step 1: Write the failing test** (append to `src/app/tests.rs`; unix-only because it needs a symlink)

```rust
#[cfg(unix)]
#[test]
fn ctrl_o_on_an_alias_of_an_open_file_switches_tab_instead_of_duplicating() {
    // `notes/alias.md` is a symlink to `notes/b.md`; following
    // `[[alias]]` while b.md is open must land in b.md's tab, not open a
    // second buffer of the same file (two buffers race on autosave)
    let root = link_vault("alias-tab", "see [[alias]]\n");
    std::os::unix::fs::symlink(root.join("notes/b.md"), root.join("notes/alias.md")).unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("notes/b.md"));
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    assert_eq!(app.tabs.len(), 2);
    app.handle_key(ctrl('o'));
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/b.md").as_path())
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test ctrl_o_on_an_alias`
Expected: FAIL, `tabs.len()` is 3.

- [ ] **Step 3: Implement**

Append to `src/fsutil.rs`:

```rust
/// The filesystem's own spelling of `path`: symlinks followed, case as
/// stored on disk. Tabs are keyed by path, so a followed link must open
/// this spelling — otherwise `[[Note]]` on a case-insensitive disk, or a
/// symlinked `alias.md`, opens a second buffer of a file that is already
/// open and the two race on autosave. Falls back to `path` when the
/// lookup fails (the open then reports its own error).
pub(crate) fn canonical(path: std::path::PathBuf) -> std::path::PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}
```

In `src/app.rs` `follow_wikilink`, change

```rust
        if let Some(path) = crate::links::resolve(target, file_dir, root, &exists) {
            self.open_file(path);
```

to

```rust
        if let Some(path) = crate::links::resolve(target, file_dir, root, &exists) {
            self.open_file(crate::fsutil::canonical(path));
```

and in `follow_md_url` change `Some(path) => self.open_file(path),` to `Some(path) => self.open_file(crate::fsutil::canonical(path)),`.

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass (`ctrl_o_dotdot_follow_stores_one_normalized_tab` still passes: the fixture roots are already canonical).

- [ ] **Step 5: Commit**

```bash
git add src/fsutil.rs src/app.rs src/app/tests.rs
git commit -m "fix: follow links to the on-disk spelling so an open file never gets a second tab

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Only jump to a heading when the open succeeded (C2)

**Files:**
- Modify: `src/app.rs` (`open_file` returns `bool`; `follow_wikilink`)
- Test: `src/app/tests.rs`

**Interfaces:**
- Produces: `fn open_file(&mut self, path: PathBuf) -> bool` — true when `path` is now the active tab. Existing callers may ignore the result. Task 8 relies on it.

- [ ] **Step 1: Write the failing test** (append to `src/app/tests.rs`)

```rust
#[test]
fn ctrl_o_heading_link_to_an_unopenable_file_keeps_the_source_cursor() {
    // `notes/bin` exists but is not UTF-8, so the open fails; the heading
    // jump must not then run against the still-active source file (which
    // mentions "intro" in its body) nor replace the open-failed status
    let root = link_vault("badopen", "intro here, see [[bin#Intro]]\n");
    fs::write(root.join("notes/bin"), [0xffu8, 0xfe, 0x00, 0x01]).unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    let before = app.editor().cursor();
    app.handle_key(ctrl('o'));
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.editor().cursor(), before);
    assert!(
        app.status.as_deref().is_some_and(|s| s.starts_with("open failed")),
        "status: {:?}",
        app.status
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test ctrl_o_heading_link_to_an_unopenable`
Expected: FAIL (cursor moved to column 0 and/or status is "note opened; heading 'Intro' not found").

- [ ] **Step 3: Implement**

In `src/app.rs`, change `open_file`:

```rust
    /// Open `path` in a new tab right of the active one and focus the
    /// editor — or, if it's already open, just switch to that tab. The
    /// tab being left autosaves on the way out. Returns whether `path`
    /// is now the active tab (false when the read failed; `status` then
    /// says why).
    fn open_file(&mut self, path: PathBuf) -> bool {
        if let Some(i) = self.tab_index(&path) {
            self.activate_tab(i);
            return true;
        }
        let mut editor = Editor::new();
        if let Err(e) = editor.open(&path) {
            self.status = Some(format!("open failed: {e}"));
            return false;
        }
        // ... unchanged body ...
        self.pending_quit = false;
        true
    }
```

In `follow_wikilink`:

```rust
        if let Some(path) = crate::links::resolve(target, file_dir, root, &exists) {
            if self.open_file(crate::fsutil::canonical(path)) {
                if let Some(h) = heading {
                    self.jump_to_heading(h);
                }
            }
        } else {
```

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass. If clippy flags `collapsible_if`, keep the nested form only if the alternative reads worse; otherwise collapse as it suggests.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/app/tests.rs
git commit -m "fix: skip the heading jump when the linked file failed to open

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Heading links land on headings only (C7)

**Files:**
- Modify: `src/links.rs` (add `heading_row`, `slug`; `wikilinks` drops empty headings)
- Modify: `src/app.rs` (`jump_to_heading`)
- Modify: `README.md` (Ctrl+O row)
- Test: `src/links/tests.rs`, `src/app/tests.rs`

**Interfaces:**
- Produces: `pub fn heading_row(lines: &[String], heading: &str) -> Option<usize>`. Task 8 consumes it via `jump_to_heading`.

Rule: only ATX heading lines (`#` to `######` followed by a space) count; the heading text and the wanted text are compared as slugs — trimmed, lower-cased, spaces turned into `-`, everything except letters, digits and `-` dropped — so both `[[b#Next Steps]]` and `[t](b.md#next-steps)` reach `## Next Steps`. An empty heading (`[[b#]]`) is no heading at all.

- [ ] **Step 1: Write the failing tests**

Append to `src/links/tests.rs`:

```rust
#[test]
fn heading_row_matches_headings_only_by_slug() {
    let lines: Vec<String> = [
        "see the introduction below",
        "",
        "## Introduction",
        "### Next Steps",
        "#not a heading",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(heading_row(&lines, "Introduction"), Some(2));
    assert_eq!(heading_row(&lines, "introduction"), Some(2));
    assert_eq!(heading_row(&lines, "next-steps"), Some(3));
    assert_eq!(heading_row(&lines, " Next Steps "), Some(3));
    assert_eq!(heading_row(&lines, "not a heading"), None);
    assert_eq!(heading_row(&lines, "missing"), None);
    assert_eq!(heading_row(&lines, ""), None);
}

#[test]
fn an_empty_heading_is_no_heading() {
    let w = parse_wikilink_at("see [[b#]] x", 6).unwrap();
    assert_eq!(w.target, "b");
    assert_eq!(w.heading, None);
}
```

Append to `src/app/tests.rs`:

```rust
#[test]
fn ctrl_o_heading_link_skips_body_text_that_mentions_the_heading() {
    let root = link_vault("headbody", "see [[b#Second]]\n");
    fs::write(
        root.join("notes/b.md"),
        "# First\nthe second part is below\n## Second\nbody\n",
    )
    .unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(app.editor().cursor(), (2, 0)); // the `## Second` line
    assert_eq!(app.status, None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test heading_row an_empty_heading ctrl_o_heading_link_skips`
Expected: compile error for `heading_row`; the others fail on the substring behavior.

- [ ] **Step 3: Implement**

Append to `src/links.rs` (before `#[cfg(test)] mod tests;`):

```rust
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
        (1..=6).contains(&hashes)
            && l[hashes..].starts_with(' ')
            && slug(&l[hashes..]) == want
    })
}
```

In `wikilinks`, change the `heading:` field to drop empties:

```rust
            heading: s.hash.map(|h| text(h + 1, pre_end)).filter(|h| !h.is_empty()),
```

In `src/app.rs`, replace `jump_to_heading` with:

```rust
    /// After following a `[[t#H]]` link (or a `path#H` markdown link):
    /// land on the heading `H` (`links::heading_row`), or note the miss.
    fn jump_to_heading(&mut self, heading: &str) {
        let row = self
            .tabs
            .get(self.active)
            .and_then(|tab| crate::links::heading_row(tab.editor.lines(), heading));
        match row {
            Some(r) => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.editor.set_cursor(r, 0);
                    tab.editor.cancel_selection();
                }
            }
            None => {
                self.status = Some(format!("note opened; heading '{heading}' not found"));
            }
        }
    }
```

In `README.md`, change the Ctrl+O row to:

```
| editor | Ctrl+O | follow the link under the cursor (`[[wikilink]]` or `[text](path)`; `[[note#Heading]]` lands on that heading; offers to create a missing note) |
```

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass. `ctrl_o_heading_link_opens_target_at_heading_row` (`# Target Head`, row 0) still passes.

- [ ] **Step 5: Commit**

```bash
git add src/links.rs src/links/tests.rs src/app.rs src/app/tests.rs README.md
git commit -m "fix: heading links land on headings, not body text that mentions them

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: The heading jump scrolls into view like search does (C8)

**Files:**
- Modify: `src/app.rs` (`jump_to_heading`)
- Test: `src/app/tests.rs`

**Interfaces:**
- Consumes: `Tab.follow_cursor: bool` (exists; `render.rs` only scrolls to the cursor when it is true), `Editor::set_cursor -> bool` (exists).

- [ ] **Step 1: Write the failing test** (append to `src/app/tests.rs`)

```rust
#[test]
fn ctrl_o_heading_link_into_a_scrolled_tab_follows_the_cursor_again() {
    let root = link_vault("headscroll", "see [[b#Target Head]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("notes/b.md"));
    app.tab_mut().unwrap().follow_cursor = false; // as a wheel scroll leaves it
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/b.md").as_path())
    );
    assert!(app.tab().unwrap().follow_cursor);
    assert_eq!(app.editor().cursor(), (0, 0));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test ctrl_o_heading_link_into_a_scrolled`
Expected: FAIL on the `follow_cursor` assertion.

- [ ] **Step 3: Implement**

Replace `jump_to_heading` in `src/app.rs` with:

```rust
    /// After following a `[[t#H]]` link (or a `path#H` markdown link):
    /// land on the heading `H` (`links::heading_row`) with the view
    /// following the cursor again (a wheel scroll may have parked it),
    /// or note the miss.
    fn jump_to_heading(&mut self, heading: &str) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        match crate::links::heading_row(tab.editor.lines(), heading) {
            Some(r) => {
                tab.follow_cursor = true;
                // set_cursor guards the u16::MAX bound that Jump takes
                if !tab.editor.set_cursor(r, 0) {
                    self.status = Some("heading is beyond line 65535 — cannot jump".into());
                    return;
                }
                tab.editor.cancel_selection();
            }
            None => {
                self.status = Some(format!("note opened; heading '{heading}' not found"));
            }
        }
    }
```

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/app/tests.rs
git commit -m "fix: heading jump scrolls the target into view

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: Markdown links with fragments, percent escapes, and any remote scheme (C9, C11)

**Files:**
- Modify: `src/links.rs` (add `split_md_url`, `percent_decode`, `is_remote_url`)
- Modify: `src/app.rs` (`follow_md_url`)
- Modify: `README.md` (Ctrl+O row)
- Test: `src/links/tests.rs`, `src/app/tests.rs`

**Interfaces:**
- Consumes: `resolve` (Task 1), `fsutil::canonical` (Task 4), `open_file -> bool` (Task 5), `jump_to_heading` (Task 7).
- Produces: `pub fn split_md_url(url: &str) -> (String, Option<String>)` (percent-decoded path, fragment without `#`, `None` when absent or empty); `pub fn percent_decode(s: &str) -> String`; `pub fn is_remote_url(url: &str) -> bool`.

Behavior: a url with `://` anywhere or a `mailto:` prefix (any case) is refused with the existing `not a local file — {url}` status. Otherwise the path part resolves like a wikilink and the fragment jumps to that heading. A bare `#heading` jumps inside the current file. A bare `#` (no path, no fragment) reports `no file '#'`.

- [ ] **Step 1: Write the failing tests**

Append to `src/links/tests.rs`:

```rust
#[test]
fn md_url_splits_fragment_and_decodes_percent_escapes() {
    assert_eq!(split_md_url("spec.md#goals"), ("spec.md".into(), Some("goals".into())));
    assert_eq!(split_md_url("spec.md#"), ("spec.md".into(), None));
    assert_eq!(split_md_url("#goals"), ("".into(), Some("goals".into())));
    assert_eq!(split_md_url("my%20note.md"), ("my note.md".into(), None));
    assert_eq!(percent_decode("caf%C3%A9.md"), "café.md");
    // malformed escapes stay literal
    assert_eq!(percent_decode("100%.md"), "100%.md");
    assert_eq!(percent_decode("a%zzb"), "a%zzb");
}

#[test]
fn remote_urls_are_any_scheme_or_mailto() {
    for url in ["https://x", "http://x", "ftp://x", "file:///x", "MAILTO:a@b", "mailto:a@b"] {
        assert!(is_remote_url(url), "{url}");
    }
    for url in ["notes/a.md", "/a.md", "../a.md", "#h", "a:b.md"] {
        assert!(!is_remote_url(url), "{url}");
    }
}
```

Append to `src/app/tests.rs`:

```rust
#[test]
fn ctrl_o_md_link_with_fragment_opens_and_jumps_to_the_heading() {
    let root = link_vault("mdfrag", "see [t](../sib/c.md#sea-side)\n");
    fs::write(root.join("sib/c.md"), "intro\n## Sea Side\nbody\n").unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "../sib", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sib/c.md").as_path())
    );
    assert_eq!(app.editor().cursor(), (1, 0));
}

#[test]
fn ctrl_o_md_link_decodes_percent_escapes() {
    let root = link_vault("mdpct", "see [t](my%20note.md)\n");
    fs::write(root.join("notes/my note.md"), "spaced\n").unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "my%20", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/my note.md").as_path())
    );
}

#[test]
fn ctrl_o_fragment_only_link_jumps_within_the_file() {
    let root = link_vault("mdanchor", "see [t](#below)\nfiller\n## Below\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "#below", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.editor().cursor(), (2, 0));
}

#[test]
fn ctrl_o_refuses_any_remote_scheme() {
    let root = link_vault("mdftp", "see [t](ftp://x/y.md)\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "ftp://", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(app.status.as_deref(), Some("not a local file — ftp://x/y.md"));
    assert_eq!(app.tabs.len(), 1);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test md_url_splits remote_urls_are ctrl_o_md_link_with_fragment ctrl_o_md_link_decodes ctrl_o_fragment_only ctrl_o_refuses_any`
Expected: compile errors for the new functions; the app tests fail on literal paths.

- [ ] **Step 3: Implement**

Append to `src/links.rs`:

```rust
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
```

Replace `follow_md_url` in `src/app.rs` with:

```rust
    /// Open a local `[text](url)` target the same way wikilinks resolve
    /// (`resolve` handles the leading `/`), then jump to its `#fragment`
    /// heading if any; a bare `#fragment` jumps inside the current
    /// file. Remote urls are refused with a status message, and unlike
    /// wikilinks a miss never offers creation — it just reports.
    fn follow_md_url(&mut self, root: &std::path::Path, file_dir: &std::path::Path, url: &str) {
        if crate::links::is_remote_url(url) {
            self.status = Some(format!("not a local file — {url}"));
            return;
        }
        let (path, fragment) = crate::links::split_md_url(url);
        let opened = if path.is_empty() && fragment.is_some() {
            true // `#heading` alone: stay in this file
        } else {
            let exists = |p: &std::path::Path| p.is_file();
            match crate::links::resolve(&path, file_dir, root, &exists) {
                Some(p) => self.open_file(crate::fsutil::canonical(p)),
                None => {
                    self.status = Some(format!("no file '{url}'"));
                    false
                }
            }
        };
        if opened {
            if let Some(h) = fragment {
                self.jump_to_heading(&h);
            }
        }
    }
```

In `README.md`, change the Ctrl+O row to:

```
| editor | Ctrl+O | follow the link under the cursor: a `[[wikilink]]`, or a `[text](path)` with the cursor on the path (`path#heading` and `%20` escapes work; a leading `/` is the vault root). `[[note#Heading]]` lands on that heading. Offers to create a missing note |
```

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass, including `ctrl_o_md_link_follows_relative_and_refuses_remote` unchanged.

- [ ] **Step 5: Commit**

```bash
git add src/links.rs src/links/tests.rs src/app.rs src/app/tests.rs README.md
git commit -m "fix: follow [text](path#heading) and percent-encoded local links

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 9: One new-file prompt that knows its directory (D3)

**Files:**
- Modify: `src/files.rs` (`create` takes `base`; add `selected_dir`; delete `create_in`)
- Modify: `src/app.rs` (`Prompt::NewFile` becomes a struct variant; delete `Prompt::NewFileAt`, `submit_new_file_at`; update `n`, `prompt_key`, `follow_wikilink`, `submit_new_file`)
- Modify: `src/ui.rs` (one `NewFile` arm in `draw_popup` and in `draw_status`)
- Test: `src/files/tests.rs`, `src/app/tests.rs`, `src/ui/tests.rs` (only if a test matches on `Prompt::NewFile(`)

**Interfaces:**
- Produces: `pub fn selected_dir(tree: &Tree) -> PathBuf`; `pub fn create(tree: &mut Tree, base: &Path, name: &str) -> Result<PathBuf, String>`; `Prompt::NewFile { input: String, dir: PathBuf }`. Tasks 10 and 11 match on this variant.

No user-visible behavior change: `n` still creates relative to the tree selection; the link-follow offer still creates relative to the vault root.

- [ ] **Step 1: Update the tests first**

In `src/files/tests.rs`: `create(&mut tree, "new.md")` becomes `create(&mut tree, &root, "new.md")` (`create_writes_an_empty_file_at_root` has `root` in scope; in `create_rejects_invalid_names` and `create_rejects_existing_file` bind `let root = fixture(...)` and pass `&root`). Rename `create_in_ignores_the_tree_selection` to `create_ignores_the_tree_selection` and replace its two `create_in(` calls with `create(`. Add:

```rust
#[test]
fn selected_dir_is_the_selected_dir_or_the_selected_files_parent() {
    let root = fixture("selected-dir");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub/x.md"), "x\n").unwrap();
    let mut tree = Tree::new(root.clone()).unwrap();
    assert!(tree.select_path(&root.join("sub")));
    assert_eq!(selected_dir(&tree), root.join("sub"));
    assert!(tree.select_path(&root.join("a.md")));
    assert_eq!(selected_dir(&tree), root);
}
```

In `src/app/tests.rs`: every `Prompt::NewFileAt { input, .. }` pattern becomes `Prompt::NewFile { input, .. }`, and the `"<not a NewFileAt prompt>"` strings become `"<not a NewFile prompt>"`. Search the file for `Prompt::NewFile(` (tuple form) and rewrite each match to the struct form — e.g. `Prompt::NewFile(_)` becomes `Prompt::NewFile { .. }`, and `Prompt::NewFile(s)` becomes `Prompt::NewFile { input: s, .. }`. Do the same in `src/ui/tests.rs` if any pattern appears there. Do not weaken any assertion.

- [ ] **Step 2: Run the tests to verify they fail to compile**

Run: `cargo test files:: 2>&1 | head -20`
Expected: compile errors (`create` arity, `selected_dir` missing).

- [ ] **Step 3: Implement**

`src/files.rs`: replace `create` and `create_in` with:

```rust
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
    // ... the rest of the former `create_in` body, unchanged ...
}
```

`src/app.rs`:

- The `Prompt` enum: replace `NewFile(String),` and the whole `NewFileAt { .. }` variant (with its doc comment) by

```rust
    /// The new-file input; `dir` is where the typed (relative) name
    /// lands — the tree selection for `n`, the vault root for a
    /// link-follow create offer — fixed when the prompt opens so the
    /// file goes where the popup implied whatever the tree does later.
    NewFile {
        input: String,
        dir: PathBuf,
    },
```

- The `n` key: `KeyCode::Char('n') => self.prompt = Prompt::NewFile { input: String::new(), dir: crate::files::selected_dir(&self.tree) },`
- In `prompt_key`, the arm `Prompt::NewFile(s) | Prompt::Search(s) | Prompt::NewFileAt { input: s, .. } =>` becomes `Prompt::NewFile { input: s, .. } | Prompt::Search(s) =>`; inside the `Enter` match, replace the two `NewFile`/`NewFileAt` arms with `Prompt::NewFile { input, dir } => self.submit_new_file(&dir, &input),`.
- `submit_new_file` takes `(&mut self, dir: &std::path::Path, name: &str)` and calls `crate::files::create(&mut self.tree, dir, name)`; delete `submit_new_file_at`.
- In `follow_wikilink`, `Prompt::NewFileAt { input: prefill, dir: root.to_path_buf() }` becomes `Prompt::NewFile { input: prefill, dir: root.to_path_buf() }`; update the comment above it to say `NewFile`.
- Fix any other `Prompt::NewFile(` pattern in `app.rs` (search for it).

`src/ui.rs`: in `draw_popup`, delete the `NewFileAt` `if let` block and change the `NewFile` one to `if let Prompt::NewFile { input, .. } = &app.prompt {`. In `draw_status`, the arm `Prompt::NewFile(_) | Prompt::NewFileAt { .. } =>` becomes `Prompt::NewFile { .. } =>`.

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass. `grep -rn "NewFileAt\|create_in" src/` prints nothing.

- [ ] **Step 5: Commit**

```bash
git add src/files.rs src/files/tests.rs src/app.rs src/app/tests.rs src/ui.rs src/ui/tests.rs
git commit -m "refactor: one new-file prompt that carries its target directory

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

(Drop `src/ui/tests.rs` from the `git add` if it was not modified.)

---

### Task 10: Show the create offer in the status bar (C6)

**Files:**
- Modify: `src/ui.rs` (`draw_status`, `NewFile` arm)
- Test: `src/ui/tests.rs`

**Interfaces:**
- Consumes: `Prompt::NewFile { .. }` (Task 9), `app.status`.

- [ ] **Step 1: Write the failing test** (append to `src/ui/tests.rs`; mirror the setup of the existing `new_file_prompt_renders_as_popup` test for the terminal size and draw call)

```rust
#[test]
fn link_follow_create_offer_shows_its_message_in_the_status_bar() {
    let root = std::env::temp_dir().join("mrkdup-ui-create-offer");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("notes/a.md"), "see [[new]]\n").unwrap();
    let root = root.canonicalize().unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    // keys only (App's open_file is private): expand `notes`, select
    // a.md, open it, then walk the cursor inside `[[new]]`
    for code in [KeyCode::Char('l'), KeyCode::Char('j'), KeyCode::Enter] {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
    for _ in 0..6 {
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));
    let backend = TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("New file"), "popup title missing");
    assert!(text.contains("Enter creates notes/new.md"), "status message missing:\n{text}");
}
```

Add `use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};` at the top of `src/ui/tests.rs` if it is not already imported. If the tree's initial selection is not the `notes` row, adjust the key sequence so `notes/a.md` ends up open (and say so in the report); the assertion on the status text is the point of the test.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test link_follow_create_offer`
Expected: FAIL on the status message assertion.

- [ ] **Step 3: Implement**

In `src/ui.rs` `draw_status`, replace the `NewFile` arm with:

```rust
        Prompt::NewFile { .. } => match &app.status {
            // a link-follow create offer explains itself here
            Some(msg) => format!("{mode}| {msg}"),
            None => {
                format!("{mode}| type a name (dir/name.md works) · Enter create · Esc cancel")
            }
        },
```

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/ui.rs src/ui/tests.rs
git commit -m "fix: show the 'Enter creates …' offer while the new-file prompt is open

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 11: Backlinks open in the go-to-file picker (D2)

**Files:**
- Modify: `src/app.rs` (`Prompt::GoToFile` gains `title`; delete `Prompt::Backlinks` and its `prompt_key` arm; `open_go_to_file`, `show_backlinks`)
- Modify: `src/ui.rs` (delete the `Backlinks` block in `draw_popup` and its `draw_status` arm; `GoToFile` block uses `title`)
- Modify: `README.md` (Ctrl+L row)
- Test: `src/app/tests.rs`

**Interfaces:**
- Produces: `Prompt::GoToFile { title: String, input: String, candidates: Vec<(String, PathBuf)>, selected: usize }`. Task 12 keeps `show_backlinks`'s shape and swaps only the candidate source.

Behavior change (documented in README): the backlinks popup is now the go-to-file picker with a `links to <stem> (<n>)` title — typing filters, `↑`/`↓` or `Ctrl+J`/`Ctrl+K` move, `Enter` opens, `Esc` closes. `j`/`k` no longer move the selection there (they type).

- [ ] **Step 1: Update the tests first** (in `src/app/tests.rs`)

`ctrl_l_backlinks_popup_lists_sorted_linkers_excluding_current_file`: replace the `let Prompt::Backlinks { candidates, selected } = &app.prompt else { ... }` destructure with

```rust
    let Prompt::GoToFile {
        title,
        input,
        candidates,
        selected,
    } = &app.prompt
    else {
        panic!("expected the picker, got status {:?}", app.status);
    };
    assert_eq!(title, " links to b (2) ");
    assert_eq!(input, "");
```

(keep the remaining assertions). `ctrl_l_backlinks_enter_opens_candidate_and_esc_closes`: replace `app.handle_key(key(KeyCode::Char('j')));` with `app.handle_key(key(KeyCode::Down));`, and `assert!(matches!(app.prompt, Prompt::Backlinks { .. }));` with `assert!(matches!(app.prompt, Prompt::GoToFile { .. }));`. Add after it:

```rust
#[test]
fn ctrl_l_backlinks_typing_filters_like_go_to_file() {
    let root = backlink_vault("filter");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("b.md"));
    app.handle_key(ctrl('l'));
    app.handle_key(key(KeyCode::Char('c')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sub/c.md").as_path())
    );
}
```

Any test that destructures `Prompt::GoToFile { input, candidates, selected }` must add `title: _,` or `..`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test ctrl_l_ 2>&1 | head -20`
Expected: compile errors (`title` field unknown).

- [ ] **Step 3: Implement**

`src/app.rs`:

- `Prompt::GoToFile` gains a first field `/// Popup title, e.g. ` Go to file ` or ` links to plan (3) `.` `title: String,`. Delete the `Backlinks { .. }` variant and its doc comment.
- `open_go_to_file`: add `title: " Go to file ".to_string(),` to the constructed prompt.
- Every `Prompt::GoToFile { input, candidates, selected }` pattern in `app.rs` gains `..` (there are two in `prompt_key`).
- Delete the whole `Prompt::Backlinks { candidates, selected } => match key.code { ... },` arm in `prompt_key` and its comment.
- `show_backlinks`: replace the final `self.prompt = Prompt::Backlinks { candidates, selected: 0 };` with

```rust
        self.prompt = Prompt::GoToFile {
            title: format!(" links to {stem} ({}) ", candidates.len()),
            input: String::new(),
            candidates,
            selected: 0,
        };
```

and change its doc comment's first sentence to: "Ctrl+L in the editor: list the notes linking to the open file ("what links here") in the go-to-file picker, titled with the note and the count."

`src/ui.rs`:

- Delete the `if let Prompt::Backlinks { .. } = &app.prompt { ... }` block in `draw_popup`, and the `Prompt::Backlinks { .. } => ...` arm in `draw_status`.
- In the `GoToFile` block: destructure `title` too; replace `popup_block(" Go to file ", theme)` with `popup_block(title, theme)`; in the width computation add `.max(title.len())` next to `.max(input.len() + 8)`.

`README.md` Ctrl+L row:

```
| editor | Ctrl+L | list notes linking here (backlinks) in the go-to-file picker: type to filter, Enter opens |
```

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass. `grep -rn "Backlinks" src/` prints nothing.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/app/tests.rs src/ui.rs README.md
git commit -m "refactor: backlinks reuse the go-to-file picker

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 12: Backlinks by resolving every wikilink, over the picker's file walk (D1, C4, C5)

**Files:**
- Modify: `src/links.rs` (add `backlinks`, `may_name`; delete `backlink_matches`, `scan_backlinks`, and the now-unused `use std::fs;` if nothing else needs it)
- Modify: `src/app.rs` (`show_backlinks`)
- Modify: `README.md` (Ctrl+L row)
- Test: `src/links/tests.rs` (delete `backlink_matches_all_three_forms_only` and `scan_backlinks_finds_only_linkers_sorted`; add the tests below), `src/app/tests.rs` (fixture additions)

**Interfaces:**
- Consumes: `crate::fuzzy::collect_candidates(root, show_hidden) -> Vec<(String, PathBuf)>` (sorted by display, 5000-file cap, text files only), `wikilinks` (Task 3), `resolve` (Task 1), `crate::fsutil::canonical` (Task 4).
- Produces: `pub(crate) fn backlinks(root: &Path, show_hidden: bool, target: &Path) -> Vec<(String, PathBuf)>`.

- [ ] **Step 1: Write the failing tests**

Replace the two deleted tests in `src/links/tests.rs` with:

```rust
#[test]
fn backlinks_resolve_every_spelling_ctrl_o_follows() {
    let root = std::env::temp_dir().join("mrkdup-links-backlinks");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::create_dir_all(root.join("home")).unwrap();
    let root = root.canonicalize().unwrap();
    fs::write(root.join("b.md"), "self [[b]]\n").unwrap();
    fs::write(root.join("a.md"), "see [[b]]\n").unwrap(); // bare stem
    fs::write(root.join("sub/c.md"), "see [[b#H]] and [[../b|B]]\n").unwrap(); // root fallback, `..`
    fs::write(root.join("e.md"), "see [[/b]]\n").unwrap(); // root-anchored
    fs::write(root.join("f.md"), "see [[b.md]]\n").unwrap(); // explicit .md
    fs::write(root.join("d.md"), "see [[other]]\n").unwrap(); // lonely
    // a same-stem note elsewhere: `[[b]]` beside it resolves there, not to root/b.md
    fs::write(root.join("home/b.md"), "other b\n").unwrap();
    fs::write(root.join("home/todo.md"), "see [[b]]\n").unwrap();
    fs::write(root.join("bin.dat"), [0u8, 1, 2, 3]).unwrap();
    let found = backlinks(&root, false, &root.join("b.md"));
    let displays: Vec<&str> = found.iter().map(|(d, _)| d.as_str()).collect();
    assert_eq!(displays, ["a.md", "e.md", "f.md", "sub/c.md"]);
    assert_eq!(found[0].1, root.join("a.md"));
    // and home/b.md's backlinks are exactly its neighbour
    let found = backlinks(&root, false, &root.join("home/b.md"));
    let displays: Vec<&str> = found.iter().map(|(d, _)| d.as_str()).collect();
    assert_eq!(displays, ["home/todo.md"]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn may_name_is_a_cheap_filename_prefilter() {
    let t = Path::new("/vault/notes/b.md");
    for link in ["b", "b.md", "notes/b", "/b", "../notes/b", "B"] {
        assert!(may_name(link, t), "{link}");
    }
    for link in ["bb", "b.txt", "notes"] {
        assert!(!may_name(link, t), "{link}");
    }
    // a trailing slash is not a different note: `resolve` drops the empty
    // component, so Ctrl+O opens b.md from `[[b/]]` and the pre-filter
    // must not veto it
    assert!(may_name("b/", t));
    assert!(may_name("notes/b/", t));
    assert!(may_name("v1.2", Path::new("/vault/v1.2.md")));
}
```

In `src/app/tests.rs`, extend `backlink_vault` so the popup test proves a path-qualified link counts: add `fs::write(root.join("sub/e.md"), "see [[../b]]\n").unwrap();` and in `ctrl_l_backlinks_popup_lists_sorted_linkers_excluding_current_file` change the expectations to three candidates in this order: `a.md`, `sub/c.md`, `sub/e.md` (title ` links to b (3) `). In `ctrl_l_backlinks_enter_opens_candidate_and_esc_closes` the `Down` + `Enter` step still opens `sub/c.md` (second entry) — leave it.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test backlinks may_name ctrl_l_`
Expected: compile errors (`backlinks`, `may_name` missing).

- [ ] **Step 3: Implement**

In `src/links.rs`, delete `backlink_matches` and `scan_backlinks` (doc comments included) and add:

```rust
/// Cheap pre-check before `resolve`: a link can only reach `target` if
/// its last path segment is `target`'s file name, with or without the
/// `.md`, ignoring ASCII case (case-insensitive disks resolve `[[B]]`
/// to `b.md`).
fn may_name(link_target: &str, target: &Path) -> bool {
    let Some(name) = target.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(last) = link_target.rsplit('/').next().filter(|s| !s.is_empty()) else {
        return false;
    };
    last.eq_ignore_ascii_case(name)
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
```

Remove `use std::fs;` from the top of `links.rs` if it is now unused (the tests module has its own import).

In `src/app.rs` `show_backlinks`, replace the `stem` computation and the `candidates` collection (`let mut candidates ... collect(); candidates.sort();`) with:

```rust
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let root = self.tree.root().to_path_buf();
        let candidates = crate::links::backlinks(&root, self.tree.show_hidden(), &path);
```

Keep the empty-result status `no links to '{stem}' yet` and the picker construction from Task 11. Update the doc comment: "The scan runs per press over the same files the go-to-file picker lists, resolving each `[[link]]` the way Ctrl+O would."

`README.md` Ctrl+L row:

```
| editor | Ctrl+L | list notes linking here (backlinks) in the go-to-file picker: every `[[link]]` in the vault that Ctrl+O would resolve to this file counts; type to filter, Enter opens |
```

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass. `grep -rn "scan_backlinks\|backlink_matches\|WalkBuilder" src/links.rs` prints nothing.

- [ ] **Step 5: Commit**

```bash
git add src/links.rs src/links/tests.rs src/app.rs src/app/tests.rs README.md
git commit -m "fix: find backlinks by resolving every wikilink over the picker's file walk

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 13: Docs sweep (D8 and stale comments)

**Files:**
- Modify: `docs/ROADMAP.md`
- Add: `docs/superpowers/plans/2026-09-09-wikilinks-review-fixes.md` (this plan, already on disk; commit it here)
- Modify: `README.md` (re-read only; fix drift if any)
- Modify: `src/app.rs`, `src/links.rs` (doc comments only)
- Modify: `src/files/tests.rs` (the stale `"create-in"` fixture tag)

- [ ] **Step 1: ROADMAP**

In `docs/ROADMAP.md` rename the `## Proposed` heading to `## Done` (its only item is checked).

- [ ] **Step 2: Doc comments**

Search `src/app.rs` and `src/links.rs` for stale references and fix each: `Task 1`, `Task 3`, `Task 4` plan references in doc comments (drop the task numbers, keep the meaning); any mention of `NewFileAt`, `create_in`, `scan_backlinks`, `backlink_matches`, `Prompt::Backlinks`; the `follow_link_under_cursor` doc must say a missing target offers creation via a prefilled `Prompt::NewFile` anchored at the vault root. `grep -rn "Task [0-9]\|NewFileAt\|create_in\|scan_backlinks\|backlink_matches\|Prompt::Backlinks" src/` must print nothing afterwards (test fixture comments in `src/app/tests.rs` may keep "Task N" wording only if rewording them would be pure churn — drop them anyway if trivial).

- [ ] **Step 3: README re-read**

Carry these two review findings into this step, both in `README.md`:

1. The Ctrl+O row says "`path#heading` and `%20` escapes work". Percent-decoding applies to the link's path only, never to the `#fragment` (`links::split_md_url` decodes the path; the fragment is compared as a slug). Reword that clause to "`%20` escapes in the path".
2. The README says Ctrl+O "offers to create a missing note" but does not say where that offer appears. Decide whether one clause naming the status bar earns its place, and say which way you decided.
3. `src/files/tests.rs` has a fixture tag `"create-in"` naming the deleted `files::create_in`. Rename the tag to `"create-base"` (it only names a temp directory).
4. The Ctrl+L row says "type to filter, Enter opens" but never names what moves the selection now that `j`/`k` type into the filter. Add the movement keys (`↑`/`↓` or `Ctrl+J`/`Ctrl+K`) to that row, matching how the Ctrl+P row reads.
5. Nothing in the README states the nested-bracket rule. Add nothing for it unless the intro's wikilink sentence would read as covering `[[a [[b]]`; the innermost `[[` wins and `[[a](b)]]` is a wikilink, both of which are edge cases the README need not spell out. Say in your report which way you decided and why.

Read the README intro paragraph about wikilinks and the two Keys rows (Ctrl+O, Ctrl+L) against the code: resolution order, `.md` rule, leading `/`, heading jump, fragment and percent handling, picker behavior for backlinks. Fix any drift.

- [ ] **Step 4: Run the checks**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add docs/ROADMAP.md README.md src/app.rs src/links.rs src/app/tests.rs docs/superpowers/plans/2026-09-09-wikilinks-review-fixes.md
git commit -m "docs: wikilinks review sweep — roadmap, stale comments, README re-read

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

(Drop any path from `git add` that was not modified.)
