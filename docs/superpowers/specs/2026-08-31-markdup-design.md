# markdup — design spec

Date: 2026-08-31
Status: approved

## What it is

A terminal markdown/text editor in Rust: a collapsible left-pane file tree
plus a soft-wrapping raw-text editor. Optimized for fast traversal and quick
edits of markdown and other text files. Same stack and philosophy as locdo:
a thin, fast wrapper around plain files.

## Core decisions

- **Raw text editing.** No inline styling or rendered preview in v1.
- **Modeless keys with a focus toggle.** No vim modes; key handling is
  structured so a vim mode could bolt on later.
- **Autosave.** Edits save on file switch, on quit, and after ~2s idle.
  No dirty-buffer juggling; only one open buffer at a time.
- **Soft wrap from day one.** Long lines wrap visually at the pane edge.
- **Editor architecture (revised post-review):** `ratatui-textarea` is
  both document engine and renderer. The original plan for a custom
  wrapped renderer assumed the crate could not soft-wrap; review found
  ratatui-textarea 0.9.2 has native soft wrap
  (`set_wrap_mode(WrapMode::WordOrGlyph)`, already used by locdo), so
  the widget renders directly and manages its own viewport/scroll.

## Layout

- Left pane: file tree rooted at the launch directory (`markdup [path]`,
  default cwd). Toggleable.
- Right pane: editor.
- Bottom: one-line status bar — relative file path, dirty dot, cursor
  line:col, transient messages ("saved", conflict warnings).

## File tree

- Shows directories and any *text* file, determined by a binary sniff
  (null-byte check on the first 8KB), not an extension allowlist.
- Respects `.gitignore` via the `ignore` crate; hidden files off by
  default, toggled with `.`.
- Directories load their children lazily on first expand.
- v1 file ops: `n` creates a new file in the selected directory (or the
  selected file's parent). Rename/delete are out of scope for v1.

## Keys

| Context | Key | Action |
|---|---|---|
| global | Ctrl+B | show/hide tree pane |
| global | Ctrl+Q | quit (autosaves first) |
| editor | Esc | focus tree |
| tree | Enter / Tab | open selected file, focus editor |
| tree | j/k, ↑/↓ | move selection |
| tree | h/l, ←/→ | collapse / expand directory |
| tree | g / G | jump to top / bottom |
| tree | n | new file |
| tree | . | toggle hidden files |
| editor | (typing) | insert text; Tab inserts indentation |
| editor | Ctrl+S | save (second press forces past a conflict) |
| editor | Ctrl+Z / Ctrl+Y | undo / redo |
| editor | Ctrl+F | in-file search (Enter next, Esc cancel) |

## Editor internals

The event loop feeds key events into `TextArea::input()` for editing;
the `TextArea` widget renders itself with
`set_wrap_mode(WrapMode::WordOrGlyph)` (wrap at word boundaries, split
words wider than the pane) and keeps the cursor visible in its own
viewport. Notes discovered in review of crate 0.9.2:

- Default keymap binds Ctrl+U=undo, Ctrl+R=redo, Ctrl+Y=paste. The app
  intercepts Ctrl+Z / Ctrl+Y before `input()` and calls
  `undo()` / `redo()` so the spec's keys hold.
- `cursor()` returns `DataCursor(pub usize, pub usize)` (row, char col),
  a tuple struct comparable to `(usize, usize)`.

## Saving and external changes

- Atomic writes: write to a temp file in the same directory, then rename.
- mtime recorded at load; re-checked before every save and on the event
  tick (no `notify` crate in v1):
  - Disk changed + buffer clean → silently reload.
  - Disk changed + buffer dirty → do **not** write; show a status-bar
    warning. An explicit second Ctrl+S forces the overwrite.

## Modules

| File | Responsibility |
|---|---|
| `main.rs` | terminal setup/teardown, event loop |
| `app.rs` | App state, focus enum, key dispatch |
| `tree.rs` | tree model, lazy loading, expand state, flatten-to-rows |
| `editor.rs` | TextArea wrapper, save/autosave/conflict logic |
| `ui.rs` | pane layout, tree + editor + status bar rendering |
| `fsutil.rs` | binary sniff, atomic write |

Dependencies: `ratatui 0.30`, `crossterm 0.29`, `ratatui-textarea 0.9`,
`ignore`, `unicode-width`.

## Testing

- Unit tests on the pure cores: tree flatten/expand, binary sniff,
  atomic write, editor save/conflict logic, app key dispatch.
- Render smoke test via ratatui `TestBackend`.
- Interactive behavior verified by running the app.

## Out of scope for v1

Tabs/splits, rendered markdown preview, cross-file search, themes, vim
mode, tree rename/delete, file watching via `notify`.
