# mrkdup settings popup + `firmitas` and `tokyonight` themes

**Date:** 2026-09-01
**Status:** approved in chat, awaiting spec review
**Builds on:** [`2026-08-31-mrkdup-themes.md`](../plans/2026-08-31-mrkdup-themes.md)
(merged as `9285103..e643a35`)

## Goal

Switch themes from inside mrkdup instead of editing the config file and
restarting. Press `s` in the tree pane, cycle the theme, see it apply
live, and have the choice written back to the config file. The popup is
a generic *settings list* so later options slot in as new rows; today it
has exactly one row.

Also ship two more builtins: the Omarchy *Firmitas Utilitas Venustas*
palette as `firmitas`, and *Tokyo Night* (the standard "night" variant)
as `tokyonight`.

This reverses two lines of the themes plan on purpose: "no theme picker
popup" and "no live reload". Everything else in that plan still holds
(no per-project config, no new crates, no `editor_bg`, no keybind
remapping).

## What stays true

- Terminal default background is the canvas. No builtin paints a
  full-pane background; `firmitas` and `tokyonight` set foregrounds only
  and expect their own terminal background (`#0c1928` / `#1a1b26`)
  underneath.
- `Color::` lives only in `src/theme.rs`. The popup uses theme slots.
- Config is `key = value`, never fails, unknown keys warn. The writer
  must not lose comments, blank lines, or other keys.
- The overlay file `~/.config/mrkdup/theme` still applies on top of
  whatever the popup selects.
- `Theme` stays a plain struct of `Style`s; the paint cache stores
  `Kind` spans and styles at paint, so a theme swap is a repaint, not a
  cache invalidation.

## Design

### Popup

`Prompt::Settings { rows, selected }`. Opened by `s` in the tree pane
(same family as `?`; it renders over the editor when one is open). In
the editor pane `s` still types and Ctrl+S still saves.

Rendering (`ui.rs`, `draw_popup`): a `popup_block(" Settings ", theme)`
centered like the other popups, one line per row:

```
 theme        ‹ firmitas ›
```

Name left-aligned, value right-aligned inside `‹ ›`. The selected row is
REVERSED (same literal modifier the pickers use). Width: longest
`name + value` plus padding, min 40. Height: rows + 2, capped to the
area. Footer hint via the existing status-bar mode text:
`| h/l or ←/→ change · j/k move · Esc close`.

Keys inside the popup (`App::prompt_key`):

| key | action |
|---|---|
| `h`, `Left` | previous value (wraps) |
| `l`, `Right` | next value (wraps) |
| `j`/`k`, `Down`/`Up` | move between rows (one row today; still implemented so a second row needs no key work) |
| `Esc`, `Enter`, `s` | close |
| Ctrl+Q / Ctrl+C | quit, as everywhere |

Any other key is ignored.

### Rows

```rust
pub struct SettingRow {
    pub name: &'static str,     // "theme"
    pub choices: Vec<String>,   // ["default","light","mono","firmitas","tokyonight", <files…>]
    pub index: usize,           // current choice
}
```

`App::open_settings()` builds the rows. The theme row's choices are
`theme::BUILTINS` (`default`, `light`, `mono`, `firmitas`, `tokyonight`,
in that order) followed by `theme::list_user_themes(&dir)`: file names in
`dir/themes/` that pass `config::valid_theme_name`, sorted, excluding
names that collide with a builtin (the builtin wins, same as the
loader). `index` is the position of `config.theme_name`; if it isn't in
the list (name from config was invalid and fell back), index is 0
(`default`).

`list_user_themes` takes an explicit `&Path` so tests use a temp dir
and never touch `XDG_CONFIG_HOME`. The popup calls it with
`config::config_dir()`; `None` (no HOME) means builtins only.

### Live apply

On every `h`/`l`:

1. `index` moves (wrapping).
2. `let (theme, warnings) = theme::load_from(name, &dir)` with
   `dir = app.config_dir` — the same loader `main.rs` uses, so
   `themes/<name>` and the overlay both apply. With no config dir,
   `Theme::named(name)`.
3. `app.theme = theme; app.config.theme_name = name.into()`.
4. `config::save_theme_name_to(dir.join("config"), name)` persists (below). Its `Err` and any
   loader warnings go to `app.status` as one line, e.g.
   `theme: firmitas — config write failed: …`. On success, status is
   `theme: firmitas`.

The next frame repaints in the new theme. Nothing else is invalidated.

### Persistence

`config.rs` gains:

```rust
/// Pure: return `text` with the first `theme = …` line replaced (the
/// value only — comments after `#` on that line are kept), or with
/// `theme = <name>` appended if no such line exists. Never touches
/// other lines.
pub fn rewrite_theme_line(text: &str, name: &str) -> String;

/// Read `path` (missing = empty text), rewrite, `fsutil::atomic_write`.
/// Creates the parent directory if needed.
pub fn save_theme_name_to(path: &Path, name: &str) -> io::Result<()>;
```

Rules for `rewrite_theme_line`:

- Match a line whose trimmed form starts with `theme` followed by
  optional spaces and `=`; only the first match is rewritten, later
  duplicates are left alone (the parser also honours the first).
- Preserve the line's leading indentation and any trailing `# comment`
  (split the value at the first `#`, keep from there).
- Commented-out lines (`# theme = light`) do not match.
- Appending: if `text` is non-empty and doesn't end in `\n`, add one
  before the new line. Output always ends in `\n`.
- `name` is already validated (it came from the choice list); the
  function does not re-validate.

`save_theme_name` only errors on I/O failure. It is called from the
popup only, never at startup.

### `firmitas` builtin

`Theme::firmitas()`, name `firmitas`, truecolor. Source palette:
<https://github.com/OldJobobo/omarchy-firmitas-utilitas-venustas-theme>
(`firmitas-utilitas-venustas-base24.yaml` / `colors.toml`). Foregrounds
only except the four chip/overlay slots, which use the theme's own
selection and accent pairs.

| slot | style | palette role |
|---|---|---|
| text | fg `#afaaa2` | fg / Limestone Text |
| bold | fg `#d1ccc4`, BOLD | light_fg |
| italic | fg `#d1ccc4`, ITALIC | light_fg |
| mark | fg `#6f6b63` | muted / Weathered Stone |
| done | fg `#6f6b63` | muted |
| welcome | fg `#6f6b63` | muted |
| heading1 | fg `#e3bf79`, BOLD | magenta / Gilded Light |
| heading2 | fg `#cea462` | green / Column Gold |
| heading | fg `#d4bda2` | blue / Travertine |
| code | fg `#b49d80` | cyan / Sandstone |
| quote | fg `#ad936d` | bright_yellow / Raised Bronze |
| link | fg `#d4bda2`, UNDERLINED | accent |
| bullet | fg `#cea462` | Column Gold |
| checkbox | fg `#e3bf79` | Gilded Light |
| html_tag | fg `#e3bf79` | Gilded Light |
| html_attr | fg `#b49d80` | Sandstone |
| border_focused | fg `#d4bda2` | active_border_color |
| popup_border | fg `#d4bda2` | active_border_color |
| border_unfocused | fg `#514d46` | Drafting Graphite |
| status_bar | fg `#0c1928` on `#d4bda2` | active_tab |
| tree_open | fg `#0c1928` on `#d4bda2` | accent chip |
| selection | fg `#feeecd` on `#514d46` | selection_fg / selection_bg |
| search_match | fg `#0c1928` on `#e3bf79` | Gilded Light |
| prompt_cursor | REVERSED | — |

The mapping mirrors `default`'s roles (h1 = the "magenta" accent
because gold is the theme's headline color; quote = the theme's
"yellow"; code = "cyan"). Tweak by overlay if a slot reads wrong.

### `tokyonight` builtin

`Theme::tokyonight()`, name `tokyonight`, truecolor. Source: the
canonical Tokyo Night "night" palette (folke/tokyonight.nvim). Same
role mapping as `firmitas` and `default`: blue headings, green code,
yellow quotes, magenta for checkbox/html tags, cyan for bullets/attrs.

| slot | style | palette role |
|---|---|---|
| text | fg `#c0caf5` | fg |
| bold | fg `#c0caf5`, BOLD | fg |
| italic | fg `#c0caf5`, ITALIC | fg |
| mark | fg `#565f89` | comment |
| done | fg `#565f89` | comment |
| welcome | fg `#565f89` | comment |
| heading1 | fg `#7aa2f7`, BOLD | blue |
| heading2 | fg `#7aa2f7` | blue |
| heading | fg `#bb9af7` | magenta |
| code | fg `#9ece6a` | green |
| quote | fg `#e0af68` | yellow |
| link | fg `#7aa2f7`, UNDERLINED | blue |
| bullet | fg `#7dcfff` | cyan |
| checkbox | fg `#bb9af7` | magenta |
| html_tag | fg `#bb9af7` | magenta |
| html_attr | fg `#7dcfff` | cyan |
| border_focused | fg `#7aa2f7` | blue |
| popup_border | fg `#7aa2f7` | blue |
| border_unfocused | fg `#3b4261` | fg_gutter |
| status_bar | fg `#1a1b26` on `#7aa2f7` | bg on blue |
| tree_open | fg `#1a1b26` on `#7aa2f7` | bg on blue |
| selection | fg `#c0caf5` on `#33467c` | fg on bg_visual |
| search_match | fg `#1a1b26` on `#e0af68` | bg on yellow |
| prompt_cursor | REVERSED | — |

`Theme::named("firmitas")` / `named("tokyonight")` return them;
`theme::BUILTINS` is the single list both `named` and the popup consult.
README config table and Themes section list all five names.

### Help and docs

- `?` cheat sheet gains `s  settings`.
- README: `s` in the key table; Configuration says the theme can also be
  changed with `s` and the choice is written back to the config file;
  "restart to apply" now applies only to the overlay/theme *files*, not
  to the `theme =` key.
- The themes plan's "What we are not doing" gets a one-line note that
  the picker and live switch landed here (or leave the plan as history
  and let the spec date speak — implementer's call; don't rewrite the
  plan).

## Error handling

| situation | behaviour |
|---|---|
| no config dir (no HOME) | popup lists builtins only; nothing is written; status says `theme: <name> (not saved: no config dir)`; theme still applies |
| `themes/` missing or unreadable | treated as empty |
| a `themes/` entry with an invalid name | skipped silently |
| `themes/<name>` fails to parse in part | loader warnings → status line (first warning), theme applied with the valid lines |
| config file unreadable | treated as empty text → written with just `theme = <name>` |
| write fails | status shows the io error; in-memory theme and `config.theme_name` already updated |

## Testing

- `config::rewrite_theme_line` table: replace-in-place keeps comment
  and indentation; commented-out line not matched; first of two
  duplicates rewritten; append with and without trailing newline;
  empty text.
- `config::save_theme_name` round-trip through a temp dir: needs a
  `save_theme_name_to(path, name)` inner fn taking the file path so the
  test never sets `XDG_CONFIG_HOME`.
- `theme::list_user_themes` on a temp dir: sorted, invalid names
  skipped, builtin collisions dropped, missing dir → empty.
- `Theme::firmitas()` and `Theme::tokyonight()` field tables vs the
  spec (every slot); `BUILTINS` names round-trip through `named` and
  `valid_theme_name`.
- App: `s` in tree opens `Prompt::Settings` with index on the current
  theme; `l` changes `app.theme` to `Theme::light()` when starting from
  default; `Esc` closes; `s` in the editor pane still types.
  Disk seam: `App` gains `config_dir: Option<PathBuf>`, set by
  `new_with_theme` from `config::config_dir()`. The popup uses
  `theme::load_from(name, dir)` and `config::save_theme_name_to(dir.join("config"), name)`
  when it is `Some`, and `Theme::named(name)` with no write when `None`.
  Tests construct `App` then set `config_dir` to `None` (pure) or to a
  temp dir (round-trip), never via `XDG_CONFIG_HOME`.
- TestBackend dump with the popup open contains `Settings`, `theme`,
  and the current name.

## Not doing

- No editing overlay slots from the UI.
- No numeric settings rows yet (the row struct allows them; nothing
  more).
- No theme preview swatches; the live repaint is the preview.
- No Ctrl+key global binding for the popup; tree-pane `s` only.
- No writing any key other than `theme`.
