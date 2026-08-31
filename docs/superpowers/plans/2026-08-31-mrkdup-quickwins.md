# mrkdup quick wins + medium batch

Eight features, one commit each, executed by subagents in three sequential
work packages (they share files, so no parallel edits).

## Conventions (every task)

- TDD: failing test → implement → green. `cargo test`, `cargo clippy
  --all-targets` (zero warnings), `cargo fmt` before every commit.
- No new dependencies.
- Key gotchas: `textarea.cursor()` returns `DataCursor(row, col)` (tuple
  struct, re-exported at crate root). Popup infra exists in `src/ui.rs`
  (`centered_rect`, `popup_block`, `draw_popup`, `Clear`). Prompt state
  machine is `app::Prompt` + `App::prompt_key`. Tree ops in `src/tree.rs`.
  Terminal may or may not support the kitty keyboard protocol — don't
  bind bare letters in the editor, and don't rely on Shift.
- Update the README key table in the same commit as its feature.
- Commit messages: `feat: …`, ending with the repo's standard
  Co-Authored-By/Claude-Session trailers (copy from `git log`). Do NOT push.

## Package A — editor features (4 commits)

### A1. Checkbox toggle — Ctrl+D (editor)
On the cursor's line: `- [ ]` → `- [x]`; `- [x]` → `- [ ]`; a plain
`- item` bullet → `- [ ] item`; any other line gets `- [ ] ` prefixed
after its leading whitespace. Preserve indentation. Implement by
rewriting the line via textarea ops so undo works (e.g. Jump to line
head, `delete_line_by_end()`, `insert_str(new)`), then restore the
cursor to a sensible column (old column clamped/shifted by the prefix
delta). Marks dirty via `note_edit`.

### A2. Word count in status bar
Append `· N words` to the normal (non-prompt) status line when a file is
open. Words = `lines().iter().map(split_whitespace().count()).sum()`.
Computed per draw; fine at md scale.

### A3. Search next + match highlighting — Ctrl+G
First check the installed crate source
(`~/.cargo/registry/src/*/ratatui-textarea-0.9.2/Cargo.toml`) for a
`search` feature; if present, enable it for our dependency and on search
submit call `set_search_pattern` with the regex-escaped literal query so
matches highlight (pick a visible `set_search_style`), clearing the
pattern when a new file opens. If the feature doesn't exist, skip
highlighting (note it in the commit message) — do not hand-roll a
highlighting renderer.
Ctrl+G in the editor = jump to next match of the last search (reuse
`search_next`; status message if there's no previous search).

### A4. Search prompt becomes a popup
Render `Prompt::Search` as a centered popup titled ` Search ` exactly
like the NewFile popup (input + block cursor); the status bar shows
hints (`Enter jump · Esc cancel`) instead of the query.

## Package B — file operations (2 commits)

### B1. Rename — Shift+R (tree)
`R` on a file opens a popup titled ` Rename ` prefilled with the current
file name (editable input; Backspace works on the prefill). Enter
renames within the same directory (`fs::rename`); reject empty, `/`,
`..`, and existing-target names with a status message. If the renamed
file is open, update `editor.path`. Refresh the tree and keep the
renamed file selected. Dirs refused with a status message ("can only
rename files"), matching delete/move. New `Prompt::Rename { path, input }`
variant.

### B2. Fuzzy file finder — Ctrl+P (global, outside prompts)
Ctrl+P opens a popup titled ` Go to file `: an input plus a result list
(top ~10). Candidates: walk the whole root with the `ignore` crate
(respect .gitignore, skip `.git`, honor the tree's show_hidden), text
files only (`fsutil::is_text_file`), cap at ~5000 entries collected once
when the popup opens. Match: case-insensitive subsequence against the
root-relative path; rank by (consecutive-run bonus, earlier match start,
shorter path). Keep the scorer a pure function with unit tests.
Up/Down (and Ctrl+J/Ctrl+K) move the selection, typing refilters,
Enter opens the file (via the existing autosave-then-open path), Esc
cancels. New `Prompt::GoToFile { input, candidates, selected }` variant
(store relative+absolute paths; filter on each keystroke).

## Package C — shell polish (2 commits)

### C1. Welcome pane
When the editor pane is visible but no file is open, render a centered
key cheat-sheet instead of the empty textarea: app name + the most
useful keys (open, new, move, delete, rename, panes, quit). Dim styling.
Disappears the moment a file opens. TestBackend test asserts it shows at
launch and is gone after opening a file.

### C2. Config file
Read `$XDG_CONFIG_HOME/mrkdup/config` (default
`~/.config/mrkdup/config`) at startup; absent file = all defaults;
malformed lines are ignored with a status warning (never crash). Format:
plain `key = value` lines, `#` comments — hand-rolled parser in a new
`src/config.rs` (pure, unit-tested; no TOML dep). Options:
`tree_width` (default 30), `side_margin_percent` (5),
`top_margin_percent` (3), `autosave_seconds` (2),
`tree_refresh_seconds` (2). Thread a `Config` struct through `App` and
`ui::draw` (replaces today's hardcoded constants). Keybind remapping is
explicitly out of scope. Document the file in the README.
