# mrkdup

A terminal markdown editor: a collapsible file tree on the left, a
soft-wrapping raw-text editor on the right. A thin, fast wrapper around
plain files — it autosaves, writes atomically, and picks up external
changes without clobbering yours.

The tree shows directories and any text-based file (detected by content,
not extension), honors `.gitignore`, and lazy-loads as you expand. Press
`-` to climb above the directory you launched in. The focused pane has a
cyan border, and the status bar shows `TREE` or `EDIT`.

## Install

```sh
cargo install --path .
```

## Use

```sh
mrkdup [directory]   # defaults to the current directory
```

## Keys

| Context | Key | Action |
|---|---|---|
| global | Ctrl+B | show/hide tree pane |
| global | Ctrl+Q | quit (autosaves; if disk changed, warns — Ctrl+Q again discards) |
| editor | Esc / Shift+Tab | focus tree |
| tree | Enter / Tab | open file (or expand/collapse dir) |
| tree | j/k, ↑/↓ | move selection |
| tree | h/l, ←/→ | collapse / expand |
| tree | g / G | jump to top / bottom |
| tree | n | new file (popup; `dir/name.md` paths allowed) |
| tree | m | move the selected file (popup lists directories) |
| tree | Shift+X | delete the selected file (confirm popup, No by default; Shift+X again confirms) |
| tree | r | refresh the tree (it also auto-refreshes every ~2s) |
| tree | - | go up: re-root the tree at the parent directory |
| tree | + | zoom in: make the selected folder the tree root (a file: its folder) |
| tree | . | toggle hidden files |
| editor | Ctrl+S | save (after a disk-conflict warning, a second Ctrl+S overwrites) |
| editor | Ctrl+Z / Ctrl+Y | undo / redo |
| editor | Ctrl+F | search in file (Enter jumps; empty search repeats the last one) |
| editor | Opt+J / Opt+K | next / previous paragraph |
| editor | Cmd+J / Cmd+K | end / start of line |

The Cmd motions need a terminal that supports the kitty keyboard
protocol (Ghostty, kitty, WezTerm, recent iTerm2) — mrkdup enables it
automatically where available. In terminals without it, Cmd never reaches
the app, and Option needs "Use Option as Meta/Esc+" turned on.

## Saving model

- Edits autosave on file switch, on quit, and after ~2s idle.
- Writes are atomic (temp file + rename) — a crash never truncates a file.
- If a file changed on disk while your buffer is clean, it reloads
  silently. If your buffer is dirty, mrkdup refuses to clobber the disk
  and asks you to confirm with a second Ctrl+S.
