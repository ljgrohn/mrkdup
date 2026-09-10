# Wikilinks + backlinks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Obsidian-style local note linking with zero stored state: `[[target]]` / `[[target|alias]]` / `[[target#heading]]` highlight with the existing link style, `Ctrl+O` in the editor follows the link under the cursor (missing target offers creation), `Ctrl+O` on a `[text](path)` URL follows local relative paths, and `Ctrl+L` lists backlinks ("what links here") in a popup.

**Architecture:** A new pure module `src/links.rs` owns parsing (`parse_wikilink_at`, `parse_md_link_at`) and resolution (`resolve`, `backlink_matches`) over plain strings and paths — no fs reads except a caller-supplied existence predicate, so tests never touch disk. `highlight.rs` reuses the existing `Kind::LinkText` / `Kind::Mark` spans for wikilinks (no theme change). `app.rs` adds two editor key arms that reuse `open_file` (open-or-switch-tab), `Prompt::NewFile` (create-on-follow, prefilled), and a new `Prompt::Backlinks` list popup drawn in `ui.rs`. Backlink discovery walks the tree root with the same `ignore::WalkBuilder` flags as `fuzzy::collect_candidates`, capped the same way.

**Tech Stack:** Rust, ratatui 0.30, crossterm; no new crates.

**Decisions (locked):**
- Syntax is `[[target]]`, `[[target|alias]]`, `[[target#heading]]`. Display text is alias or target; `[[`, `]]`, `|` render dimmed (`Mark`), text uses the existing `link` style. No broken-link distinction in v1 (highlight is pure text, no fs access) — missing targets reveal themselves via the create offer on follow.
- Resolution order: current file's parent dir, then tree root. No extension → append `.md`. Case-sensitive, exact filesystem truth. `http(s):`, `mailto:`, `#`-only, and absolute-path URLs are never followed (status message instead). No network, no shell-out, ever.
- Heading anchors jump via the existing `search::find_ci` + `Editor::set_cursor`; anchor miss opens the file top with a status note.
- Keyboard-only v1. No mouse follow, no graph view, no embeds (`![[]]`), no rename-rewrites-links (that fights "never lose user data" — stays deferred).
- No index files, no config keys, no new settings. What's on disk stays the truth.

## Global Constraints

- `Color::` appears only in `src/theme.rs` (gate: `rg 'Color::' src --glob '!src/theme.rs' --glob '!src/theme/**'` prints nothing). This plan adds no colors.
- New files go through `files::create` / `open_file` only; every unresolved-target creation honors the atomic-write path. Dirty buffers keep the existing conflict behavior — follow autosaves the tab being left exactly like `open_file` does today.
- Tests live in `src/<module>/tests.rs`, use temp directories and synthetic key events, never touch the real config dir or HOME.
- Before every commit: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` (the CI gates in `.github/workflows/ci.yml`).
- README is user docs: Keys-table rows for `Ctrl+O` / `Ctrl+L` plus one wikilink sentence in the styling intro paragraph land in the same commit as the behavior.

---

## File map

| file | change |
|---|---|
| `src/links.rs` (new) | `parse_wikilink_at`, `parse_md_link_at`, `resolve`, `backlink_matches`, `scan_backlinks` |
| `src/links/tests.rs` (new) | parser/resolver/backlink tables, all pure or temp-dir |
| `src/main.rs` (or module root) | declare `mod links;` wherever sibling modules are declared |
| `src/highlight.rs` | `[[...]]` arm in `inline_based` reusing `Mark` + `LinkText` |
| `src/highlight/tests.rs` | wikilink span tables |
| `src/app.rs` | `Ctrl+O` / `Ctrl+L` editor arms, `follow_link_under_cursor`, `Prompt::Backlinks`, popup key handling |
| `src/app/tests.rs` | follow/create/backlink dispatch tests on temp vaults |
| `src/ui.rs` | draw the backlinks popup, `Ctrl+O` / `Ctrl+L` cheat-sheet lines |
| `README.md` | two Keys rows + styling-paragraph sentence |
| `docs/ROADMAP.md` | replace "No pending items" with the wikilinks entry while proposed; mark done on land |

---

### Task 1: `src/links.rs` — pure parse + resolve

**Files:**
- New: `src/links.rs`, `src/links/tests.rs`
- Modify: module declaration for `links` (mirror `fuzzy`/`search`)

**Interfaces:**
- Produces:
  - `pub struct WikiLink { pub target: String, pub alias: Option<String>, pub heading: Option<String>, pub start: usize, pub end: usize }` — `start`/`end` are char indices of the whole `[[...]]` in the line, `target` is the raw text before `|`/`#`.
  - `pub fn parse_wikilink_at(line: &str, col: usize) -> Option<WikiLink>` — the `[[...]]` spanning char column `col`, or `None`. Unclosed `[[` → `None`. Empty target → `None`.
  - `pub struct MdLink { pub url: String, pub start: usize, pub end: usize }` — span covers the `(url)` part.
  - `pub fn parse_md_link_at(line: &str, col: usize) -> Option<MdLink>` — mirrors the `[text](url)` shape `highlight.rs` already tokenizes. Must agree with the highlighter on what counts (add a cross-test in Task 2).
  - `pub fn resolve(target: &str, file_dir: &Path, root: &Path, exists: &dyn Fn(&Path) -> bool) -> Option<PathBuf>` — try `file_dir.join(p)`, then `root.join(p)`, where `p` is `target` verbatim, plus again with `.md` appended when the target has no extension. First path where `exists` holds wins.
  - `pub fn backlink_matches(content: &str, stem: &str) -> bool` — true when `content` contains `[[<stem>]]`, `[[<stem>|`, or `[[<stem>#`. Case-sensitive.
  - `pub(crate) fn scan_backlinks(root: &Path, show_hidden: bool, stem: &str) -> Vec<PathBuf>` — `ignore::WalkBuilder` with the same flags as `fuzzy::collect_candidates` (hidden/git_ignore/git_exclude toggles, `require_git(false)`, `git_global(false)`, `parents(false)`, skip `.git`), same 5000 cap, `fsutil::is_text_file` gate, `fs::read_to_string` (skip unreadable), keep files where `backlink_matches` holds, sorted.

- [ ] **Step 1: Write the failing parser tests**

Append to `src/links/tests.rs` (create both files, wire `#[cfg(test)] mod tests;`):

```rust
#[test]
fn wikilink_plain_alias_and_heading() {
    let w = parse_wikilink_at("see [[notes/plan]] done", 7).unwrap();
    assert_eq!((w.target, w.alias, w.heading), ("notes/plan".into(), None, None));
    let w = parse_wikilink_at("see [[plan|the plan]] done", 8).unwrap();
    assert_eq!(w.alias.as_deref(), Some("the plan"));
    assert_eq!(w.target, "plan");
    let w = parse_wikilink_at("see [[plan#next steps]] done", 8).unwrap();
    assert_eq!(w.heading.as_deref(), Some("next steps"));
}

#[test]
fn wikilink_col_must_be_inside_brackets() {
    let line = "a [[plan]] b";
    assert!(parse_wikilink_at(line, 0).is_none()); // on 'a'
    assert!(parse_wikilink_at(line, 2).is_some()); // on '['
    assert!(parse_wikilink_at(line, 9).is_some()); // on ']'
    assert!(parse_wikilink_at(line, 11).is_none()); // past it
}

#[test]
fn wikilink_rejects_unclosed_and_empty() {
    assert!(parse_wikilink_at("a [[oops b", 4).is_none());
    assert!(parse_wikilink_at("a [[]] b", 4).is_none());
    assert!(parse_wikilink_at("a [[|alias]] b", 5).is_none());
}

#[test]
fn md_link_parses_url_span() {
    let m = parse_md_link_at("see [t](docs/a.md) ok", 9).unwrap();
    assert_eq!(m.url, "docs/a.md");
    assert!(parse_md_link_at("see [t](docs/a.md) ok", 0).is_none());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test links:: 2>&1 | tail -5`
Expected: compile errors (`cannot find function parse_wikilink_at`, etc.).

- [ ] **Step 3: Implement the parsers + `resolve` + `backlink_matches`**

Char-indexed scans over `line.chars().collect::<Vec<_>>()` (cols are char columns). For `parse_wikilink_at`: find the last `[[` at/before `col`, then the first `]]` after it; `col` must be within `open..close+2`. Split inner on first `|` (alias) and first `#` (heading; `#` inside alias text after `|` belongs to the alias — split `#` off the target part only, before `|` handling... define: heading splits off the pre-`|` segment; document with a test).

For `resolve`: helper `with_md(s) -> PathBuf`; candidates `[file_dir.join(p), root.join(p)]`, each tried raw then with `.md` appended if `Path::new(p).extension().is_none()`. Skip candidates that escape nothing — no sandboxing beyond the vault: `..` that leaves root still resolves if the file exists (same as the tree's `-` ascend ethos); document this.

- [ ] **Step 4: Write the failing resolve/backlink tests**

```rust
#[test]
fn resolve_prefers_sibling_dir_then_root_and_adds_md() {
    let root = std::path::Path::new("/vault");
    let dir = std::path::Path::new("/vault/notes");
    let exists = |p: &std::path::Path| p == Path::new("/vault/notes/plan.md") || p == Path::new("/vault/shared.md");
    assert_eq!(resolve("plan", dir, root, &exists).unwrap(), Path::new("/vault/notes/plan.md"));
    assert_eq!(resolve("shared", dir, root, &exists).unwrap(), Path::new("/vault/shared.md"));
    assert_eq!(resolve("readme.md", dir, root, &exists), None);
    assert!(resolve("", dir, root, &exists).is_none());
}

#[test]
fn backlink_matches_all_three_forms_only() {
    assert!(backlink_matches("see [[plan]]", "plan"));
    assert!(backlink_matches("see [[plan|P]]", "plan"));
    assert!(backlink_matches("see [[plan#H]]", "plan"));
    assert!(!backlink_matches("see [[planet]]", "plan"));
    assert!(!backlink_matches("see [[my plan]]", "plan"));
}
```

Plus a `scan_backlinks` temp-dir test: vault with `a.md` linking `[[b]]`, `sub/c.md` linking `[[b#H]]`, `d.md` linking `[[other]]`, and a binary file; assert result is exactly `{a.md, sub/c.md}` sorted.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test links:: 2>&1 | tail -3`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/links.rs src/links/tests.rs <module-decl-file> docs/ROADMAP.md
git commit -m "feat: wikilink parse/resolve/backlink-scan core (links.rs)"
```

(ROADMAP: replace the "No pending items." line with `## Proposed\n\n- [ ] Obsidian-style local linking (see docs/superpowers/plans/2026-09-09-wikilinks.md).` — flip to done when the feature lands.)

---

### Task 2: Highlight `[[...]]` with the existing link style

**Files:**
- Modify: `src/highlight.rs` (`inline_based`, ~line 318 next to the `[text](url)` arm)
- Test: `src/highlight/tests.rs`

**Interfaces:**
- Consumes: `links::parse_wikilink_at`-compatible shape (same bracket rules; add a cross-check test, not a call — the highlighter stays a pure char scan).
- Produces: no new `Kind`, no theme change. `[[` / `]]` / `|` → `Kind::Mark`, inner text → `Kind::LinkText` (so `Theme::link` paints it everywhere, all builtins included).

- [ ] **Step 1: Write the failing span tests**

Mirror the existing link-span test style in `src/highlight/tests.rs`: for input `a [[plan|P]] b`, assert the `[[` and `]]` and `|` spans are `Mark` and `plan|P`... precisely: `plan` and `P` spans are `LinkText`. Include: unclosed `[[` yields no link spans; `[[...]]` inside `` `code` `` stays `CodeInline` (code arm runs first — assert order); heading form `[[p#H]]` yields one `LinkText` span over `p#H` (no sub-splitting in paint).

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test highlight:: 2>&1 | tail -5`
Expected: failures on the new wikilink cases only.

- [ ] **Step 3: Implement the arm**

In `inline_based`, before the `[text](url)` arm (so `[[` isn't eaten by `[`): on `c == '[' && chars.get(i+1) == Some(&'[')`, find closing `]]` via a `find_pair`-style scan; on success flush and push `Mark([[])`, `LinkText(inner)`, `Mark([]])`, splitting `|` into `LinkText / Mark(|) / LinkText` when present. Advance past `]]`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test highlight:: 2>&1 | tail -3`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/highlight.rs src/highlight/tests.rs
git commit -m "feat: highlight [[wikilinks]] with the link style"
```

---

### Task 3: `Ctrl+O` follow-link (+ create on missing, + local `[text](path)`)

**Files:**
- Modify: `src/app.rs` (editor dispatch ~line 891, new `follow_link_under_cursor` helper, `Prompt::NewFile` prefill)
- Test: `src/app/tests.rs`
- Modify: `src/ui.rs` (cheat-sheet line), `README.md` (Keys row)

**Interfaces:**
- Consumes: `Editor::cursor() -> (row, col)` + `Editor::lines()`, `links::{parse_wikilink_at, parse_md_link_at, resolve}`, `App::open_file`, `Prompt::NewFile`, `fuzzy::rel_display`, `search::find_ci`, `Editor::set_cursor`.
- Behavior table (all set `self.status` on no-op so the key never silently does nothing):
  - cursor on `[[t]]`/`[[t|a]]` resolving to an existing file → `open_file` it (autosave-out included, already-open switches tab).
  - resolving to nothing → `prompt = NewFile(rel_display(root, would_be_path))` + status `no note '<t>' — Enter creates, Esc cancels`, where `would_be_path` is `file_dir.join(t with .md default)` (the first resolution candidate, even though it doesn't exist yet).
  - cursor on `[[t#H]]` → open then `find_ci(content, H)` → `set_cursor(row, 0)`; miss → status `note opened; heading '<H>' not found`.
  - cursor on `(url)` of `[text](url)` with a relative local url → same as wikilink (resolve against file dir then root; reuse `resolve`). Absolute `/...` paths resolve against root. `http(s)://`, `mailto:`, `#anchor`-only → status `not a local file — <url>`.
  - cursor on plain text → status `no link under cursor`.

- [ ] **Step 1: Pin the cursor-column contract**

`Editor::cursor()` returns `(row, col)` from `ratatui-textarea`. Before relying on `col` as a char index into `lines()[row]`, add a test in `src/editor/tests.rs`: type `héllo [[wörld]]` (multibyte), place the cursor over the wikilink, assert `cursor()` col lands inside the `[[...]]` char range the parser accepts. If textarea cols are display-width based instead, Task 1's parser takes the textarea col as-is and this test defines the conversion helper (implement it in `links.rs`, tested here). Do not proceed until this test passes — everything else keys off it.

- [ ] **Step 2: Verify `Ctrl+O` reaches app dispatch**

Confirm unmatched editor keys fall through to the textarea *after* the `(ctrl, Char(..))` arms (~line 891), and that textarea 0.9 binds nothing to `Ctrl+O` by default (synthetic-key test: open temp file, send `Ctrl+O` with no link under cursor, assert `status == Some("no link under cursor")` and buffer text unchanged). Place the new arms with the existing ctrl arms.

- [ ] **Step 3: Write the failing dispatch tests** (`src/app/tests.rs`, temp vaults + synthetic keys):

  - follow opens sibling: vault `notes/a.md` containing `[[b]]`, `notes/b.md` exists → `Ctrl+O` on the link switches to a `b.md` tab.
  - missing target offers create: `[[new]]` → prompt becomes `NewFile("notes/new.md")`-ish prefill; submitting creates via the existing new-file flow.
  - alias + heading: `[[b|Bee]]` opens `b.md`; `[[b#Target Head]]` opens `b.md` with cursor row on the `# Target Head` line.
  - md-link: `[t](../sib/c.md)` follows; `[t](https://x)` sets the not-local status and opens nothing.
  - root fallback: link from `notes/a.md` to `[[shared]]` with only `<root>/shared.md` present opens it.

- [ ] **Step 4: Run them to verify they fail**

Run: `cargo test app::tests::link 2>&1 | tail -8` (name tests `...link...`)
Expected: compile error (`follow_link_under_cursor` missing) or assertion failures.

- [ ] **Step 5: Implement `follow_link_under_cursor` + the `Ctrl+O` arm**

  Pseudocode: get active editor (guard: no open file → status, no crash — cf. the `ed()` hardening); `row/col` + line; try `parse_wikilink_at`, else `parse_md_link_at`; classify url (`http(s)://`, `mailto:`, leading `#` → not-local); `resolve(...)` with `|p| p.is_file()`; hit → `open_file` + optional heading jump; miss (wikilink only) → prefilled `NewFile` + status. `Ctrl+O` must also work when a popup is *not* open only (don't hijack prompt input).

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test app::tests:: 2>&1 | tail -3`
Expected: all pass.

- [ ] **Step 7: Cheat sheet + README Keys row**

`src/ui.rs` launch-page sheet: add `Ctrl+O follow link under cursor` to the editor keys. README Keys table: `| editor | Ctrl+O | follow the link under the cursor ([[wikilink]] or [text](path); offers to create a missing note) |`.

- [ ] **Step 8: Full gates + commit**

Run: `cargo test 2>&1 | tail -3 && cargo clippy -- -D warnings 2>&1 | tail -1 && cargo fmt --check && rg 'Color::' src --glob '!src/theme.rs' --glob '!src/theme/**'`
Expected: green, clean, nothing.

```bash
git add src/app.rs src/app/tests.rs src/editor/tests.rs src/ui.rs README.md
git commit -m "feat: Ctrl+O follows [[wikilinks]] and local [text](path) links"
```

---

### Task 4: `Ctrl+L` backlinks popup

**Files:**
- Modify: `src/app.rs` (`Prompt::Backlinks { candidates: Vec<(String, PathBuf)>, selected: usize }`, `Ctrl+L` arm, popup j/k/Enter/Esc handling mirroring `GoToFile` minus the filter input)
- Test: `src/app/tests.rs`
- Modify: `src/ui.rs` (draw popup reusing the go-to-file popup frame; empty state `no links to <name> yet`), `README.md` (Keys row)

**Interfaces:**
- Consumes: `links::scan_backlinks`, active tab's path stem (`path.file_stem()`), `App::open_file`.
- Behavior: `Ctrl+L` with an open file collects `scan_backlinks(root, show_hidden, stem)` excluding the current file itself; empty → status `no links to '<stem>' yet` (no popup); else popup lists root-relative paths, `j/k`/arrows move, `Enter` opens (`open_file`) and closes, `Esc` closes. No filter input in v1 (lists are small; the input row would duplicate GoToFile machinery).

- [ ] **Step 1: Verify `Ctrl+L` reaches app dispatch** (same synthetic-key probe as Task 3 Step 2; textarea 0.9 default check).

- [ ] **Step 2: Write the failing tests**

  - vault where `a.md` + `sub/c.md` link `[[b]]`, `d.md` doesn't → `Ctrl+L` in `b.md` pops 2 candidates sorted (`a.md`, `sub/c.md`); current file excluded even if it self-links.
  - `Enter` on first candidate switches to its tab and clears the prompt; `Esc` clears without switching.
  - lonely file → status message, `prompt` stays `None`.
  - no open file (welcome page) → status message, no panic.

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test app::tests::backlink 2>&1 | tail -8`
Expected: compile error (`Backlinks` variant / `scan_backlinks` wiring missing).

- [ ] **Step 4: Implement the variant, arm, scan call, popup draw + keys**

  Keep the popup's key handling next to GoToFile's (~lines 1227+): `j/k`/Up/Down move `selected` (clamp), `Enter` opens, `Esc` closes. Draw in `ui.rs` with the same popup border/padding helpers as GoToFile; title `links to <stem> (n)`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test app::tests:: 2>&1 | tail -3`
Expected: all pass.

- [ ] **Step 6: README Keys row + gates + commit**

README: `| editor | Ctrl+L | list notes linking here (backlinks) |`.

```bash
git add src/app.rs src/app/tests.rs src/ui.rs README.md
git commit -m "feat: Ctrl+L backlinks popup"
```

---

### Task 5: Docs close-out + merge checks

- [ ] **Step 1: Styling-paragraph sentence**

README intro (the "links" mention in the styling paragraph): append one sentence — `` `[[wikilinks]]` highlight like links; `Ctrl+O` follows, `Ctrl+L` shows backlinks. `` Verify the paragraph still reads true.

- [ ] **Step 2: Re-read every touched README section** (Keys table, intro) against the code: exact key names (`Ctrl+O`, `Ctrl+L`), exact behaviors (create offer, not-local refusal, no-popup empty state).

- [ ] **Step 3: ROADMAP flip + full gates**

Mark the roadmap item done. Run the CI trio exactly as configured:

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test 2>&1 | tail -3`
Expected: all green.

- [ ] **Step 4: Final commit (docs only if behavior is already committed, else squash into the feature commits)**

```bash
git add README.md docs/ROADMAP.md
git commit -m "docs: wikilinks keys and roadmap"
```

---

## Explicitly deferred (not this plan)

- Graph view, `![[embeds]]`, rename-rewrites-links, broken-link styling, mouse follow, `Ctrl+P` completion of `[[` targets while typing, daily-note special-casing, case-insensitive matching, watching + live backlink refresh (scan runs per `Ctrl+L` press — always fresh by construction).
