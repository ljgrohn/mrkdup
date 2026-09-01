# mrkdup review follow-ups — plan for v0.1 hardening

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> (or subagent-driven-development) to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. Work the packages
> **in order**. Do not start Package E (ship) until A–D are green on CI
> for ubuntu, macos, **and** windows.

**Date:** 2026-08-31
**Status:** approved next work (blocks crates.io / v0.1.0)
**Source:** full review of `main` at `b1e35f1` — composability, simplicity,
speed, and bug-freedom. 161 tests pass locally; CI is red.

**Goal:** Make mrkdup fast and boringly correct on real notes, without
adding features. The app is already feature-complete for v0.1. The risk
is “fast and doesn’t eat my files,” which is the whole pitch.

**Do not do in this plan:** mark-hiding on non-cursor lines, new
keybindings, themes, vim mode, rendered preview, extra config keys,
new dependencies. Those stay deferred.

---

## What’s already right (do not regress)

These modules are the template. New code should look like them, not like
`app.rs`.

- **Document engine vs renderer is the correct split.**
  `ratatui-textarea` is the buffer/undo engine; `render.rs` owns paint.
  Never go back to rendering the textarea widget.
- **Writes are careful.** Atomic temp+rename, dirty-never-clobbers-disk,
  idle autosave. Matches CONTRIBUTING: “plain files are the only state.”
- **Tests actually pin behavior.** Checkbox + selection, case-only
  rename, search wrapping, welcome-pane typing, conflict/force-save.
- **Pure cores are good.** `wrap.rs`, `highlight.rs`, `config.rs`,
  `tree.rs`, `editor.rs` save/conflict logic, `fsutil.rs`.

`CONTRIBUTING.md` already says: each module has one job; logic goes in a
testable module; `ui.rs` and `main.rs` stay thin glue. This plan is
enforcing that, not inventing a new architecture.

---

## Architecture after this plan

```
main.rs          terminal + event loop (unchanged)
app.rs           App state, focus, Prompt enum, key dispatch only
fuzzy.rs         collect_candidates, fuzzy_score, fuzzy_filter
search.rs        find_ci, search_next (the jump; highlight stays in render)
checkbox.rs      toggle_checkbox_line, checkbox_trigger_armed
files.rs         new / rename / move / delete (fs + tree refresh + editor.path)
editor.rs        buffer, save/conflict, **line-ending preserved**
                 textarea field private; thin ops for insert/jump/undo/line
render.rs        takes a view struct, not &mut App; caches wrap + highlight
tree.rs          gitignore from the *tree root* down; sniff cache
fsutil.rs        atomic_write cleans up tmp on error; is_text_file cacheable
wrap.rs / highlight.rs / config.rs   unchanged APIs (highlight may grow
                                     a reusable State for incremental use)
```

No new crates. CI matrix stays ubuntu / macos / windows.

---

## Current CI failure (the first thing to unred)

Latest run on `main` (`b1e35f1`):
https://github.com/ljgrohn/mrkdup/actions/runs/33452428322

- **Windows `test` failed.** Ubuntu/mac were then **cancelled** (`fail-fast`).
- Clippy and rustfmt were green.
- The failing test:

```
app::tests::ctrl_p_opens_go_to_file_with_text_files_from_the_whole_root
assertion failed: rels.contains(&"docs/deep.md")
```

On Windows the relative path is `docs\deep.md`. Fuzzy find will also
require backslashes. Display paths must be normalized to `/`.

`atomic_write_replaces_existing` **passed** on Windows in that run — do
not “fix” Windows rename-over-existing unless a test actually fails.

---

## Conventions (every task)

- TDD: failing test → implement → green. `cargo test`, `cargo clippy
  --all-targets -- -D warnings`, `cargo fmt` before every commit.
- No new dependencies.
- `textarea.cursor()` is `DataCursor(row, col)` (tuple struct).
- All user-file writes go through `fsutil::atomic_write`.
- Commit messages: `fix:` / `refactor:` / `perf:` as appropriate.
  Copy the repo’s existing Co-Authored-By / session trailer style from
  `git log` if the working session uses it; otherwise omit. Do **not**
  invent a session URL. Do **not** push until the user asks, except the
  commit that added this plan (already on the remote).
- Update README in the same commit when user-visible behavior changes
  (line endings, gitignore scope, Ctrl+Q in prompts).
- One concern per commit. Packages below list the commits.

---

## Package A — unred CI + don’t mutate files

These are ship-blockers. No refactors yet.

### A1. Normalize relative paths to `/`  (fix, one commit)

**Why:** Windows CI is red; Ctrl+P matching is OS-dependent.

**Files:** `src/app.rs` (`collect_candidates`; later `src/fuzzy.rs`),
the test at `ctrl_p_opens_go_to_file_with_text_files_from_the_whole_root`.

**Behavior:**
- When stripping `root` off an absolute path for display / fuzzy input,
  join components with `/` regardless of OS. Never `Path::to_string_lossy()`
  on a relative `Path` for UI strings.
- Opening still uses the absolute `PathBuf` (OS-native). Only the
  *display* string is canonical.
- Test: assert the candidate list contains `"docs/deep.md"` on every OS.
  Also assert it does **not** contain a backslash form.

```rust
fn rel_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
```

Put this next to `collect_candidates` (moves to `fuzzy.rs` in C1; write
it once, don’t duplicate).

Also in this commit: `.github/workflows/ci.yml` — set
`strategy.fail-fast: false` so a Windows failure still reports Linux/mac.

- [ ] **Step 1:** Change the existing test to also `assert!(!rels.iter().any(|s| s.contains('\\')))`.
- [ ] **Step 2:** Implement `rel_display`; use it in `collect_candidates`.
- [ ] **Step 3:** `fail-fast: false` in CI.
- [ ] **Step 4:** `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt`
- [ ] **Step 5:** Commit `fix: normalize go-to-file paths to slash-separated`

### A2. Preserve original line endings  (fix, one commit)

**Why:** `fs::read_to_string` + `str::lines()` + `join("\n")` converts
CRLF files to LF on first save. That violates “plain files are the
truth.” A Windows notes folder would churn every file in git.

**Files:** `src/editor.rs` (and tests). README “Saving model” if it
implies LF-only.

**Behavior:**
- On `open`, detect the file’s newline:
  - If the file contains `"\r\n"` (and not a lone `\n` mixed — see
    mixed below) → `Newline::CrLf`
  - Else → `Newline::Lf`
  - Empty file → `Newline::Lf` (new content is Unix; fine)
- Mixed endings: pick the majority, or first occurrence. Document the
  choice in the test name. Do **not** try to preserve mixed per-line —
  that’s a second rewrite. Majority/first is enough.
- Store `newline: Newline` on `Editor`.
- `content()` joins with that ending and still ensures the file ends
  with exactly one trailing newline of that kind (existing trailing-NL
  guarantee, now encoding-aware).
- `check_external` reload must re-detect (the external writer may have
  changed endings).
- `Editor::new()` (no file) has no newline yet; opening sets it.

**Tests (write first):**
- Open a file whose bytes are `b"hello\r\nworld\r\n"`, edit, save,
  assert the file still contains `\r\n` and no bare `\n`.
- Open an LF file, save, still LF.
- Empty file create-via-new-file stays LF (`atomic_write(&path, b"")`
  is already empty; first save after typing writes LF).

Do **not** change `str::lines()` loading — stripping `\r` for the in-
memory buffer is correct. Only the write-back encoding changes.

This **overrides** v1 plan Global Constraint “Saved files always end
with exactly one trailing newline” in the LF-only sense: still exactly
one trailing newline, of the file’s original kind.

- [ ] **Step 1:** Failing tests for CRLF round-trip and LF round-trip.
- [ ] **Step 2:** `Newline` enum + detect + `content()` uses it.
- [ ] **Step 3:** README one-liner under Saving model.
- [ ] **Step 4:** test / clippy / fmt
- [ ] **Step 5:** Commit `fix: preserve original CRLF vs LF on save`

### A3. Gitignore from the tree root  (fix, one commit)

**Why:** `list_dir` and `collect_candidates` both set `parents(false)`.
That was a test-hermeticity choice in the v1 plan (don’t read the
developer’s *global* gitignore). It leaked into production: expanding
`notes/` does not apply `root/.gitignore`. A `*.log` rule at the repo
root will not hide `notes/debug.log`. README claims “honors `.gitignore`.”

**Keep:** `git_global(false)` — still don’t read `~/.config/git/ignore`.
**Change:** when walking a subdirectory, still honor gitignores from
the **tree root** down (the launch dir / current `Tree::root()`), not
from `/`.

**Files:** `src/tree.rs` (`list_dir`), `src/app.rs` `collect_candidates`
(later `fuzzy.rs`). Possibly a shared helper in `fsutil.rs` or `tree.rs`:

```rust
pub fn walk_dir(dir: &Path, root: &Path, show_hidden: bool) -> WalkBuilder
```

`WalkBuilder::new(dir)` + something that applies ignore files from
`root` to `dir`. Options:

- Build one `ignore::gitignore::Gitignore` for `root` and match each
  entry (simplest to test).
- Or `WalkBuilder::new(root).max_depth(...)` filtered to `dir` — easy
  to get wrong with lazy expand.

Prefer a `Gitignore` matcher built from `root` (and nested
`.gitignore` files under `root` as the walker already does per-dir).
The ignore crate’s `WalkBuilder` on `dir` with `parents(true)` would
walk *above* `root` toward `/` — too far. So: **parents(false) relative
to `root`**, not relative to `dir`.

Concrete approach that stays small:

1. Add `Tree` method / helper `fn ignores(root, show_hidden) -> Gitignore`.
2. `list_dir(dir, root, show_hidden)` uses WalkBuilder on `dir`
   (`max_depth 1`, `git_global(false)`, `parents(false)`) **plus**
   explicitly skip entries whose path relative to `root` is ignored by
   a `Gitignore` built from `root/.gitignore` (and, if present, nested
   gitignores between `root` and `dir`).

If wiring a full nested matcher is getting large, **minimum viable
fix**: build `Gitignore` from `{root}/.gitignore` only and match
root-relative paths. Nested per-directory `.gitignore` already works
today when that directory is the `WalkBuilder` root. The bug is only
the *parent* file not applying to children.

**Tests:**
```
root/
  .gitignore     containing "*.log"
  notes/debug.log
  notes/keep.md
```
- `Tree::new(root)`, expand `notes` → rows include `keep.md`, not `debug.log`.
- `collect_candidates` likewise omits `notes/debug.log`.
- A `*.log` in the **user’s global** gitignore still must not affect
  tests (`git_global(false)` stays).

- [ ] **Step 1:** Failing tree + go-to-file tests with a root `.gitignore`.
- [ ] **Step 2:** Matcher from tree root; thread `root` into `list_dir`.
- [ ] **Step 3:** Same matcher in `collect_candidates`.
- [ ] **Step 4:** test / clippy / fmt
- [ ] **Step 5:** Commit `fix: apply root .gitignore to nested tree listings`

### A4. Ctrl+Q / Ctrl+C work inside prompts  (fix, one commit)

**Why:** `prompt_key` returns immediately on any Control modifier.
Stuck in Search/Rename/GoToFile, the user cannot quit. Only Esc works,
which is not what Ctrl+Q is documented as.

**Files:** `src/app.rs` `prompt_key`.

**Behavior:** Before the “any Ctrl → handle j/k in GoToFile else
return” block, handle Ctrl+Q and Ctrl+C via `do_quit()` (same as
global). Then the existing Ctrl+J/K for go-to-file. Other Ctrl keys
still ignored (don’t type into the prompt).

**Test:** Open NewFile prompt, Ctrl+Q → `should_quit` (and autosave
rules unchanged: no dirty buffer in this fixture). Second test: dirty
editor, open Search, Ctrl+Q → save+quit like normal.

- [ ] **Step 1:** Failing test: `n` then Ctrl+Q quits.
- [ ] **Step 2:** Dispatch Ctrl+Q/C in `prompt_key`.
- [ ] **Step 3:** README if the keys table needs “works during prompts.”
- [ ] **Step 4:** test / clippy / fmt
- [ ] **Step 5:** Commit `fix: Ctrl+Q/C quit even while a prompt is open`

### A5. Checkbox `[X]` + single-undo toggle  (fix, one commit)

**Why two bugs in one function:**

1. Highlight treats `- [X]` as done; `toggle_checkbox_line` only knows
   `- [x]`. Ctrl+D on an uppercase box **prefixes** another `- [ ]`.
2. Toggle is `delete_line_by_end` + `insert_str` = two undo steps.
   The existing test `ctrl_d_is_undoable` even documents “two undo
   steps.” Users will think undo is broken.

**Files:** `src/app.rs` (`toggle_checkbox_line`, `toggle_checkbox`;
later `checkbox.rs`).

**Behavior:**
- `- [ ]` ↔ `- [x]` as today.
- `- [X]` → `- [ ]` (same as `[x]`). Writing back always uses
  lowercase `[x]` (one canonical checked form).
- Undo: **one** Ctrl+Z restores the original line. How, without a new
  crate:
  - Prefer a single textarea operation if the crate has one (replace
    current line). Check `ratatui-textarea` 0.9 APIs before inventing.
  - If not: after the two ops, if the crate exposes undo-group begin/end,
    use that.
  - If not: keep the two ops but call `undo()` once internally after
    reconstructing… **no, that would undo the toggle.** Don’t.
  - Fallback that stays simple: `move_cursor(Head)`,
    `delete_line_by_end()`, `insert_str(new)` is two history entries.
    Acceptable fallback **only if** the crate truly has no grouping:
    then change the test to `undo` twice and document in the README
    (“Ctrl+Z twice”). Prefer grouping.
- Keep the selection-cancel and empty-line special cases.
- Keep cursor-column shift by prefix delta; keep the u16 Jump guard.

**Tests:**
- `- [X] milk` + Ctrl+D → `- [ ] milk`.
- Existing check/uncheck tests still pass.
- `ctrl_d_is_undoable` becomes **one** Ctrl+Z (update the test; that
  is the point of this commit).

- [ ] **Step 1:** Failing tests for `[X]` and single undo.
- [ ] **Step 2:** Strip-prefix `[X]`; group the edit.
- [ ] **Step 3:** test / clippy / fmt
- [ ] **Step 4:** Commit `fix: Ctrl+D handles [X] and undoes in one step`

---

## Package B — smaller correctness cuts

Do these before the split so the split doesn’t move buggy code.

### B1. `atomic_write` deletes tmp on failure  (fix)

**Files:** `src/fsutil.rs`.

On any error after `File::create(&tmp)`, `let _ = fs::remove_file(&tmp)`
before returning the error (including `write_all`, `sync_all`, `rename`).
Success path already has no tmp (rename consumed it).

**Test:** Simulate failure? Hard without injecting. Minimum: a unit
test that if `rename` target is an existing **directory** with the
destination name (so rename fails), the `.{name}.mrkdup-tmp` is gone.
Or chmod the dest dir 0555 on Unix — skip on Windows.

- [ ] Test + cleanup + commit `fix: atomic_write removes temp file on error`

### B2. Tab width is 4 columns, not tab-stops — document, don’t “fix”

`wrap::ch_width('\t')` returns 4. Tabs do not align to 4/8/12.
Changing to tab-stops would shift every wrapped line and every test.

**This plan: document in README** (Configuration or a one-liner under
the editor description): “Tabs display as 4 spaces (not tab-stops).”
No code change unless you later decide tab-stops are worth a dedicated
task.

- [ ] README line + commit `docs: tabs display as 4 spaces, not tab-stops`

### B3. Inline styles inside headings and quotes  (optional, one commit)

`> **bold**` and `# **hi**` are currently one span (Quote / Heading).
Cheap win for the highlighter feeling finished.

**Only if it stays small:** after the block prefix span, run `inline()`
on the remainder instead of one Quote/Heading span. Headings can keep
the heading color as a base and let inline marks overlay — or just
run `inline()` and drop the solid heading color on the tail. Pick one,
test coverage via `assert_covers` + a kind-contains Bold.

If this fights `Kind` (can’t be both Heading and Bold): **skip this
task.** Don’t invent span stacking in this plan. Simplicity wins.

- [ ] Either a small highlight.rs commit, or skip with a note in the
      PR/commit that stacking isn’t worth it.

---

## Package C — split `app.rs` (behavior-identical)

`app.rs` is ~1800 lines and owns dispatch, file ops, search, fuzzy,
and checkbox. CONTRIBUTING forbids this. Extract **without** changing
behavior. Tests move with the functions they cover.

Do **not** parallelize these; they all touch `app.rs`.

### C1. `src/fuzzy.rs`

Move `collect_candidates`, `fuzzy_score`, `fuzzy_filter`, `rel_display`.
`fuzzy_filter` stays `pub(crate)` — `ui.rs` draw of GoToFile uses it.

`collect_candidates` needs `tree.root()`, `show_hidden()`, and the
gitignore matcher from A3. Pass those in. Don’t take `&App`.

Move the fuzzy_* unit tests into `fuzzy.rs`. App tests that open Ctrl+P
stay in `app.rs`.

Commit `refactor: extract fuzzy file finder into fuzzy.rs`

### C2. `src/search.rs`

Move `find_ci`. `search_next` uses `&mut Editor` + `last_search` +
`search_highlight` — either:

- Keep `search_next` as an `App` method that calls `find_ci`, or
- `search::next(lines, cursor, query) -> Option<(row, col)>` (pure),
  App applies the Jump.

Prefer the pure function. `render.rs` already imports `find_ci` — it
should import from `search`, not `app`.

Commit `refactor: extract search matching into search.rs`

### C3. `src/checkbox.rs`

Move `toggle_checkbox_line` and `checkbox_trigger_armed` (the latter
needs the current line + col, so make it
`fn trigger_armed(line: &str, col: usize) -> bool`).
App’s `toggle_checkbox` stays as the textarea-rewriting glue.

Commit `refactor: extract checkbox helpers into checkbox.rs`

### C4. `src/files.rs` (or fold into `fsutil.rs` if it stays <150 lines)

Move the fs+tree+editor.path glue: `submit_new_file`, `submit_rename`,
`move_file`, `delete_file`. They need `&mut Tree`, `&mut Editor`,
status string out. Shape:

```rust
pub fn rename(tree: &mut Tree, editor: &mut Editor, src: &Path, name: &str)
    -> Result<String /* status */, String /* error status */>
```

Don’t pass `&mut App`. Status/focus stay in App.

If this wants a 400-line `files.rs` of thin wrappers, **stop** and
leave the methods on App — the win is fuzzy/search/checkbox, not
ceremony. Bias toward extract when the function is already pure-ish
(rename validation, checkbox line, fuzzy score).

Commit only if the extract is obviously thinner.

### C5. Hide `Editor.textarea`

`pub textarea: TextArea` lets App poke the engine 40+ times. Add
narrow methods used by dispatch:

- `lines() -> &[String]`
- `cursor() -> (usize, usize)`
- `set_cursor(row, col)` / Jump with the u16 guard in one place
- `insert_str`, `undo`, `redo`, `input(Input) -> bool`
- `move_cursor(CursorMove)`
- `cancel_selection`, `selection_range`
- `current_line() -> Option<&str>`

App and render go through those. Field becomes `pub(crate)` or private.

This is the commit that will touch the most call sites. Keep it
mechanical. Tests should not change behavior.

Commit `refactor: stop exposing Editor.textarea`

### C6. Renderer takes a view, not `&mut App`

Today `render::render_editor(f, app: &mut App, inner, focused)` clones
the whole document, wraps, highlights, and mutates `app.editor_scroll`.

Introduce something like:

```rust
pub struct EditorView<'a> {
    pub lines: &'a [String],
    pub cursor: (usize, usize),
    pub selection: Option<((usize, usize), (usize, usize))>,
    pub search: Option<&'a str>,
    pub file_kind: highlight::FileKind,
    pub scroll: &'a mut usize,
}
```

`ui::draw_editor` builds it. `render.rs` must not `use crate::app::App`.
It may `use crate::search::find_ci`.

Tests in `render.rs` that drive `App::handle_key` can stay as
integration tests, or keep using App to set up state then draw via
`ui::draw`. Don’t rewrite those unless they break.

Commit `refactor: render the editor from a view struct, not App`

---

## Package D — speed (the thing that will hurt on real notes)

Every editor frame currently:

1. Clones every line (`lines().to_vec()`)
2. Re-wraps the **whole file**
3. Re-tokenizes the **whole file** (required: fence/frontmatter state
   runs from line 0)
4. For each visible character, linearly scans that line’s spans
   (`style_at`)
5. Re-runs case-insensitive search, allocating `Vec<char>` per row

Fine at 200 lines. Painful at 5–10k.

Tree refresh has the same smell: every ~2s, every visible file is
opened and the first 8KB sniffed.

### D1. Cache wrap + highlight; invalidate from the edited line

**Files:** `src/render.rs` and/or a small `src/layout_cache.rs`.
Owned by whoever holds editor paint state — likely `Editor` or a
field on `App` that render updates.

**Cache contents:**
- `width: usize` (wrap depends on it)
- `gen` or dirty flag
- `rows: Vec<VisualRow>`
- `spans: Vec<Vec<SpanTok>>`

**Invalidate:**
- On any edit (`note_edit`), file open, or width change: dirty.
- **Correct simple version:** recompute wrap+highlight for the whole
  file on dirty. That’s already a win vs doing it on **every frame**
  (cursor move, tick, tree refresh paint).
- **Better version (if simple stays tiny):** re-highlight from the
  edited line downward (a fence toggle changes later lines). Wrap is
  per-line and independent — rewrap dirty lines only, then rebuild
  the `VisualRow` vec from that line to EOF.

Do the **simple version first** (skip recompute when `!dirty && width
unchanged && same line-count+hash`). Only if that’s <50 lines of
cache logic. Don’t build an incremental highlighter in this task
unless the simple cache is already landed.

`highlight::highlight` can stay as the full-doc API. Optional follow:
expose `State` + `highlight_line` as `pub(crate)` for incremental.
Not required for D1.

**Do not clone the document to render.** `textarea.lines()` already
borrows. Cache keys off that slice.

**Test:** existing render TestBackend tests still pass. Add one that
paints twice without an edit and (if you expose a counter in
`#[cfg(test)]`) asserts highlight wasn’t recomputed. If a counter is
too cute, skip — the point is not to regress paint.

Commit `perf: cache wrap and highlight across frames`

### D2. Walk highlight spans with a cursor, not `find` per character

`style_at` is O(spans) per character. For a long line that’s
O(n * spans).

In the visible-row loop, keep `span_i` and advance when `ci >= span.end`.
Spans are already sorted, non-overlapping, covering the line
(`assert_covers` in highlight tests).

Same commit or a tiny follow: `find_ci` should not allocate two
`Vec<char>` per visible row per frame. Either cache the query’s
lowercase chars on `App.search_highlight`, or scan with
`eq_ignore_ascii_case` for ASCII-only (markdown) and keep unicode
path for the rare case. Don’t regress `find_ci_handles_unicode`.

Commit `perf: O(1) span lookup per character while painting`

### D3. Don’t re-sniff every file every 2s

`is_text_file` opens and reads 8KB per non-dir entry on every
`list_dir` (tree refresh default: 2s) and on every Ctrl+P walk.

**Minimum:** cache `(path, mtime) -> bool` on `Tree` (or a static
isn’t acceptable). Invalidate a path when mtime changes; drop entries
for paths no longer in the walk.

**Even cheaper first filter:** if the extension is a known text type
(`md`, `txt`, `html`, `htm`, `json`, `toml`, `rs`, `py`, …) skip the
sniff. If a known binary type (`png`, `jpg`, `pdf`, `zip`, …) skip
and return false. **Unknown / no extension:** sniff. This is a
behavior change for “misnamed” binaries with `.md` — acceptable;
document in CONTRIBUTING as “extension hint, sniff as fallback.”

Prefer extension hint + mtime cache, not a 40-type enum with politics.
Keep the list short (the ones in *this repo* plus markdown/html).

**Test:** a `.md` file that starts with a NUL is still a text file if
you trust the extension — **or** still sniffed. Pick trust-extension
and test it; or keep sniff-always and only add the mtime cache.
Sniff-always + cache is the smaller behavior change. Prefer that.

Commit `perf: cache is_text_file by path+mtime`

---

## Package E — ship (only after A–D)

Unchanged from `todo`. Do not start this package until:

- CI is green on ubuntu, macos, **and** windows
- Line endings round-trip
- Root gitignore applies to nested dirs
- `app.rs` is dispatch-sized
- Paint is cached

Then:

- [ ] Publish `mrkdup` to crates.io (grabs the name) — **needs the
      owner’s crates.io token; don’t guess**
- [ ] Tag `v0.1.0`
- [ ] CI badge on the README
- [ ] cargo-dist (or a release workflow) so tagged releases ship
      prebuilt binaries for mac/linux/windows

Separate plan if the release plumbing is more than one commit. This
file does not specify cargo-dist config.

---

## Explicitly out of scope (do not sneak in)

- Mark-hiding on non-cursor lines (deferred on purpose; layout must
  not shift).
- Prompt mini-editor (left/right/home/end in rename). Append/backspace
  is fine for v0.1.
- Keybind remapping, themes, vim mode, tabs/splits, `notify` watcher.
- Replacing `ratatui-textarea` as the engine.
- Stacking highlight styles (Heading+Bold) unless B3 stays trivial.
- Changing tab width to tab-stops.
- Reading the user’s *global* gitignore.

---

## Suggested execution order (checklist)

Copy this into the session log and tick it.

- [ ] A1 slash-separated paths + CI `fail-fast: false`
- [ ] A2 CRLF/LF preserved on save
- [ ] A3 root `.gitignore` applies to nested listings
- [ ] A4 Ctrl+Q/C inside prompts
- [ ] A5 `[X]` checkbox + single undo
- [ ] B1 atomic_write tmp cleanup
- [ ] B2 docs: tabs = 4 spaces
- [ ] B3 (optional) inline styles in quotes/headings
- [ ] C1 fuzzy.rs
- [ ] C2 search.rs
- [ ] C3 checkbox.rs
- [ ] C4 files.rs only if it’s actually thinner
- [ ] C5 Editor.textarea private
- [ ] C6 EditorView; render.rs does not import App
- [ ] D1 wrap/highlight frame cache
- [ ] D2 span cursor + cheaper find_ci
- [ ] D3 is_text_file mtime cache
- [ ] E ship v0.1.0

Each box is one commit (B3 skippable, C4 skippable, E is several).
