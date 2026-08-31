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
- **Editor architecture:** `ratatui-textarea` is the document engine
  (buffer, cursor, edit ops, undo/redo, search). Its widget rendering is
  never used; a custom renderer draws the wrapped view.

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

`WrapView` (module `wrap.rs`) is pure layout math, fully unit-testable:

- Input: the document's logical lines, cursor (row, col), pane width,
  pane height, current scroll.
- Splits each logical line into display rows at the pane width using
  `unicode-width` (never splitting a grapheme's cells).
- Maps the logical cursor to a (display row, display col).
- Adjusts vertical scroll (in display rows) so the cursor stays visible.
- Output: the visible display rows + screen cursor position for ratatui.

The event loop feeds key events into `TextArea::input()` for editing and
reads `lines()` / `cursor()` for rendering. `TextArea`'s own viewport and
render are unused.

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
| `wrap.rs` | pure soft-wrap layout math |
| `fsutil.rs` | binary sniff, atomic write |

Dependencies: `ratatui 0.30`, `crossterm 0.29`, `ratatui-textarea 0.9`,
`ignore`, `unicode-width`.

## Testing

- Unit tests on the pure cores: wrap layout (unicode widths, empty lines,
  exact-width lines, cursor mapping at boundaries), tree flatten/expand,
  binary sniff, atomic write.
- Render snapshots via ratatui `TestBackend` for the wrapped view.
- Interactive behavior verified by running the app.

## Out of scope for v1

Tabs/splits, rendered markdown preview, cross-file search, themes, vim
mode, tree rename/delete, file watching via `notify`.
