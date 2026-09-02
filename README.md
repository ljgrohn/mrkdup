# mrkdup

A terminal markdown editor: a collapsible file tree on the left, a
soft-wrapping raw-text editor on the right. A thin, fast wrapper around
plain files — it autosaves, writes atomically, and picks up external
changes without clobbering yours.

The editor styles markdown live as you type — headings by level,
bold/italic, inline and fenced code, checkboxes (done items dimmed),
blockquotes, links, YAML frontmatter — plus HTML tag/attribute/string
coloring in `.html` files and for inline HTML in markdown, and code
syntax — keywords, types, functions, strings, comments, numbers — for
Rust (`.rs`), JavaScript/TypeScript (`.js`, `.mjs`, `.cjs`, `.jsx`,
`.ts`, `.tsx`), CSS/SCSS, and SQL, both as files and inside fenced
blocks tagged with the language. Every
character stays visible; syntax marks are dimmed, never hidden, so the
layout never shifts under your cursor. Tabs display as 4 spaces, not
tab-stops.

Until you open a file, the editor pane shows a short key cheat sheet;
it disappears the moment a file opens.

The tree shows directories and any text-based file (detected by content,
not extension), honors `.gitignore`, and lazy-loads as you expand. Press
`-` to climb above the directory you launched in. The focused pane has a
cyan border, and the status bar shows `TREE` or `EDIT`, the cursor
position, and a live word count for the open file.

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
| global | Ctrl+T | show/hide editor pane (tree goes full width) |
| global | Ctrl+P (tree: p) | go to file (fuzzy finder popup: type to filter, ↑/↓ or Ctrl+J/K choose, Enter opens) |
| global | Ctrl+Q / Ctrl+C (tree: q) | quit (works even in prompts; autosaves; if disk changed, warns — press again to discard) |
| global | Ctrl+W | close the active tab (autosaves; a disk conflict keeps it open) — the tab to its left takes over, or the launch page with the tree focused when it was the last |
| global | Opt+H / Opt+L | previous / next tab (wraps) |
| global | Opt+1 … Opt+9 | jump to that tab |
| editor | Esc / Shift+Tab | focus tree |
| tree | Enter / Tab | open file in a new tab right of the current one — or switch to its tab if it's already open (a dir expands/collapses) |
| tree | j/k, ↑/↓ | move selection |
| tree | h/l, ←/→ | collapse / expand |
| tree | g / G | jump to top / bottom |
| tree | n | new file (popup; `dir/name.md` paths allowed) |
| tree | m | move the selected file (popup lists directories) |
| tree | r | rename the selected file (popup prefilled with the current name) |
| tree | x | delete the selected file (confirm popup, No by default; x again confirms) |
| tree | u | refresh the tree (it also auto-refreshes, every ~2s by default) |
| tree | - | go up: re-root the tree at the parent directory |
| tree | + | zoom in: make the selected folder the tree root (a file: its folder) |
| tree | . | toggle hidden files (dotfiles and anything matched by `.gitignore`) |
| tree | ? | show the key cheat sheet (the launch page) over the editor; any key closes it |
| tree | s | settings popup: h/l or ←/→ cycle the selected setting — theme, side padding (applies live and is written to the config file); j/k pick the row; Esc, Enter, or s closes. Mouse: click a row to pick it, click its ‹ / › to cycle, click outside the popup to close |
| editor | Ctrl+S | save (after a disk-conflict warning, a second Ctrl+S overwrites) |
| editor | Ctrl+Z / Ctrl+Y | undo / redo |
| editor | Ctrl+D | toggle checkbox on the line (`- [ ]` ↔ `- [x]`, `- [X]` → `- [ ]`; other lines gain a `- [ ] ` prefix; Ctrl+Z twice undoes) |
| editor | Ctrl+F | search in file (popup; case-insensitive; Enter jumps and highlights all matches; empty search repeats the last one) |
| editor | Ctrl+G | jump to the next match of the last search |
| editor | Ctrl+J / Ctrl+K | next / previous word |
| editor | Opt+J / Opt+K | next / previous paragraph |
| editor | Cmd+J / Cmd+K | end / start of line |

### Tabs

Every file you open gets a tab in the bar across the top of the editor pane: `name ×`, with a `*` after the name while the buffer has unsaved changes. The active tab is highlighted (theme keys `tab_active` / `tab_inactive`). Names longer than 20 cells are cut with `…`. When there are more tabs than fit, the bar scrolls so the active one stays visible, with `‹` / `›` at the edges for the tabs hidden that way. Idle autosave and reload-on-external-change run for every tab, not just the one on screen; switching away from a tab saves it; quitting saves them all (a disk conflict in any tab stops the quit with the usual warning, and Ctrl+Q again discards). Renaming or moving a file in the tree follows it into its tab; deleting one closes its tab.

### Mouse

mrkdup takes over the mouse, so a click and a drag act on the pane they land in instead of the terminal painting one selection across the tree and the editor together.

| Where | Action |
|---|---|
| editor | click lands the cursor (and focuses the editor); drag selects; the selection is confined to the editor pane — drag over the tree or past the top/bottom edge and it keeps extending (scrolling a row at a time) |
| editor | releasing a drag copies the selection to the system clipboard: through OSC 52 (iTerm2, kitty, WezTerm, Ghostty, Alacritty, foot, tmux with `set-clipboard on`) and, when one is installed, a local command — `pbcopy` on macOS (so Terminal.app works too), `wl-copy` under Wayland, `xclip` or `xsel` under X11 |
| editor | wheel scrolls the view three rows without moving the cursor; the next key or click brings the view back to the cursor |
| tab bar | click a name to switch to that tab; click its × to close it (same as Ctrl+W); when tabs are hidden off either edge, `‹` / `›` appear there and step one tab |
| tree | click selects the row and opens it (a folder toggles), like Enter; wheel moves the selection three rows |

Hold Shift while dragging to get the terminal's own selection back (Option in some macOS terminals).

The Cmd motions need a terminal that supports the kitty keyboard
protocol (Ghostty, kitty, WezTerm, recent iTerm2) — mrkdup enables it
automatically where available. In terminals without it, Cmd never reaches
the app, and Option needs "Use Option as Meta/Esc+" turned on.

## Configuration

mrkdup reads `$XDG_CONFIG_HOME/mrkdup/config` at startup
(`~/.config/mrkdup/config` when `XDG_CONFIG_HOME` is unset). The file is
optional — no file means all defaults — and it never causes a crash:
lines that can't be parsed (or name an unknown option) are ignored, with
a one-line warning in the status bar.

`theme` and `side_padding` can also be changed from inside mrkdup: press
`s` in the tree pane; the choice applies immediately and is written back
to this file.

The format is plain `key = value` lines; a line starting with `#` is a
comment (inline comments after a value are not supported) and blank
lines are fine. Numeric values are whole numbers; out-of-range values are
clamped into the ranges below.

```ini
# ~/.config/mrkdup/config
tree_width = 30
side_margin_percent = 5
autosave_seconds = 10
side_padding = 1
# default | light | mono | firmitas | tokyonight
theme = default
```

| Option | Default | Range | Meaning |
|---|---|---|---|
| `tree_width` | 30 | 10–120 | tree pane width, in columns |
| `side_margin_percent` | 5 | 0–40 | editor breathing room: % of pane width trimmed off each side |
| `top_margin_percent` | 3 | 0–40 | editor breathing room: % of pane height trimmed off the top |
| `autosave_seconds` | 2 | 1–600 | idle seconds before a dirty buffer autosaves |
| `tree_refresh_seconds` | 2 | 1–600 | seconds between automatic tree refreshes |
| `side_padding` | 1 | 0–20 | columns of empty space between the terminal edges and the panes (padding outside the borders; `side_margin_percent` is the text margin inside the editor) |
| `theme` | `default` | see below | `default` (current look), `light` (dark fg for light terminals), `mono` (no color, modifiers only), `firmitas` (Omarchy navy/bronze/gold, truecolor), `tokyonight` (truecolor), or any other name — see Themes |

### Themes

`theme` picks one of the five builtins (`default`, `light`, `mono`, `firmitas`, `tokyonight`) by
name. Any other name (matching `^[a-z][a-z0-9_-]{0,31}$`) is looked up as
a file at `$XDG_CONFIG_HOME/mrkdup/themes/<name>` — an unknown name or a
missing file falls back to `default` with a warning in the status bar.
`firmitas` and `tokyonight` are truecolor and set foregrounds only — they assume the matching terminal background (`#0c1928` / `#1a1b26`).

On top of whichever theme that resolves to, mrkdup applies
`$XDG_CONFIG_HOME/mrkdup/theme` if it exists — an overlay file that
tweaks individual slots without redefining the whole palette. Both
files use the same `key = value` format as the config file (`#`
comments, blank lines, never fails — bad lines warn and are skipped,
the rest still apply), and both files are read when the theme is
(re)applied — at startup, or when you pick a theme with `s` — so edit a
theme file, then re-select the theme to see it; there's no file watcher.

Each `value` is one or more colors and modifiers:

```text
style  := part ( '+' part )* [ ' on ' part ( '+' part )* ]
part   := color | modifier
color  := named | bright-named | '#rrggbb' | 'default'
named  := black | red | green | yellow | blue | magenta | cyan
         | white | gray | grey
modifier := reverse | reversed | dim | bold | italic
           | underline | underlined
```

The first color on a side is the foreground; `on` starts the
background. `bright-cyan` is ratatui's `LightCyan`; `gray`/`grey` is
`Color::Gray`. Hex colors need exactly 6 digits (`#fff` is a warning,
not CSS-style expansion) and work on truecolor terminals. `default`
alone is `Style::default()`; `default` combined with other parts
(`default on blue`) means "reset that side to the terminal's own
color." Tokens are case-insensitive; there are no spaces except
around `on` (`cyan + bold` is a warning, not `cyan+bold` with padding).
`bright-` applies only to black, red, green, yellow, blue, magenta, and
cyan — there is no `bright-white`, `bright-gray`, or `bright-grey`.

Beware: `selection = default` or `search_match = default` disables that overlay entirely (it patches nothing), making the selection or search match invisible; use `reverse` for a colorless-but-visible overlay instead.

The settable keys are the `Theme` struct's field names — every pane
and syntax slot mrkdup paints:

`border_focused`, `border_unfocused`, `popup_border`, `status_bar`,
`selection`, `prompt_cursor`, `welcome`, `tree_open`, `tab_active`,
`tab_inactive`, `text`, `mark`,
`heading1`, `heading2`, `heading`, `bold`, `italic`, `code`,
`checkbox`, `done`, `quote`, `link`, `bullet`, `html_tag`,
`html_attr`, `search_match`, and for code (Rust, JS/TS, CSS, SQL):
`keyword` (also CSS properties and `!important`), `type_name` (also
Rust lifetimes and CSS selectors), `string` (also char literals),
`comment`, `number` (also CSS units and `#hex` colours), `macro`
(Rust `name!` and `#[attributes]`, JS `@decorators`, CSS `@rules`, SQL
`@variables`), `function` (a name called with `(`, or after `fn`).

`name` is not settable from a theme file — it's how the theme is
addressed, not part of it.

A dark truecolor overlay, saved to `~/.config/mrkdup/theme`:

```ini
# ~/.config/mrkdup/theme
text = #cdd6f4
heading1 = #89b4fa+bold
heading2 = #89b4fa
quote = #a6e3a1
code = #94e2d5
link = #89b4fa+underline
selection = reverse
search_match = #1e1e2e on #f9e2af
```

Slots are independent — `text` is not inherited by `bold`, `italic`, or `mark`; set those explicitly (e.g., `bold = #cdd6f4+bold`) if you want them to follow `text`.

Keybindings are not remappable.

## Saving model

- Edits autosave on file switch, on quit, and after ~2s idle
  (`autosave_seconds` in the config file).
- Writes are atomic (temp file + rename) — a crash never truncates a file.
- If a file changed on disk while your buffer is clean, it reloads
  silently. If your buffer is dirty, mrkdup refuses to clobber the disk
  and asks you to confirm with a second Ctrl+S.
- Line endings are preserved: CRLF files stay CRLF, LF files stay LF.
