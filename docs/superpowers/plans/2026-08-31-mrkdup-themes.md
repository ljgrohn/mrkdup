# mrkdup themes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> (or subagent-driven-development) to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. Work the tasks **in
> order**. One concern per commit.

**Date:** 2026-08-31
**Status:** planned (post-harden, not a v0.1 ship blocker)
**Depends on:** review follow-ups Package C
([`2026-08-31-mrkdup-review-followups.md`](./2026-08-31-mrkdup-review-followups.md))
so `render.rs` takes a view, not `&mut App`. T1 can land before C6 if
needed — C6 then threads `&Theme` through `EditorView` instead of
reading `app.theme`. Do **not** start this during Package A (CI is red).

**Goal:** Users pick a color palette without mrkdup growing a design
system. One `Theme` struct is the only place `Color::` appears. Three
builtins (`default`, `light`, `mono`) plus a dumb overlay file. No new
crates, no new keys, no per-project config, no live reload.

**Why this exists:** every color today is a literal in three files.
`highlight::style` couples the tokenizer to ratatui. Light terminals
make Yellow quotes and Cyan headings unreadable. There is no way to
turn the colors off. Themes fix that **and** are the composable place
for chrome (borders, search match, tree “open” marker).

---

## What’s already true (do not regress)

- Terminal default background is the canvas. mrkdup does **not** paint
  a full-pane editor background. Themes do not add one. A user who
  wants a dark canvas sets it in the terminal (or Ghostty/kitty theme).
- `Kind` in `highlight.rs` is semantic. Spans stay `Kind`, never `Style`.
- Config is `$XDG_CONFIG_HOME/mrkdup/config`, `key = value`, never
  fails, unknown keys warn. Themes follow that exact contract.
- CONTRIBUTING: no per-project config, no sidecar next to notes.
- Render tests assert on TestBackend `Debug` (`"Cyan"`, `"Yellow"`,
  `"REVERSED"`). Default theme must keep those strings.
- Search-match and selection are **overlays** on syntax style, not
  replacements of `Kind`. Selection stays `REVERSED` in every builtin
  (it adapts to the terminal). Search-match is Yellow/Black in color
  themes and `REVERSED` in `mono`.

---

## Architecture

```
highlight.rs     Kind + tokenizer. NO ratatui::style. NO Color.
theme.rs         Theme struct, builtins, parse_style, file overlay
config.rs        one new string key: theme = default|light|mono|<name>
main.rs          load config, load theme (builtin + files), pass in
app.rs           App.theme: Theme          (until C6)
render.rs        style_at(..., theme) ; search overlay from theme
ui.rs            every Style::default().fg(Color::…) goes through theme
```

After follow-ups C6:

```
EditorView { …, theme: &'a Theme }
render.rs must not import App
```

`Theme` is a plain struct of `Style` fields, not a `HashMap`. Unknown
overlay keys warn (same as unknown config keys). Adding a slot is a
compile change, which is what we want.

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    // chrome
    pub border_focused: Style,
    pub border_unfocused: Style,
    pub popup_border: Style,
    pub status_bar: Style,
    pub selection: Style,       // overlay; default = REVERSED
    pub prompt_cursor: Style,   // the reversed “ “ in input popups
    pub welcome: Style,
    pub tree_open: Style,       // currently White on Blue
    // syntax
    pub text: Style,
    pub mark: Style,            // also LinkUrl, HtmlComment
    pub heading1: Style,
    pub heading2: Style,
    pub heading: Style,         // h3+
    pub bold: Style,
    pub italic: Style,
    pub code: Style,            // inline, fence, HtmlString
    pub checkbox: Style,
    pub done: Style,
    pub quote: Style,
    pub link: Style,
    pub bullet: Style,
    pub html_tag: Style,
    pub html_attr: Style,       // also FmKey
    pub search_match: Style,    // overlay; default = Black on Yellow
}

impl Theme {
    pub fn syntax(&self, kind: highlight::Kind) -> Style { /* match */ }
}
```

`highlight::style` is **deleted**. `render::style_at` becomes
`theme.syntax(span.kind)`.

No `editor_bg` / `tree_bg` field. If someone later wants a painted
canvas, that is a separate decision (it costs a bg on every cell).

---

## Color value grammar

One parser, used by the overlay file. Keep it tiny and tested.

```
style  := part ( '+' part )* [ ' on ' part ( '+' part )* ]
part   := color | modifier
color  := named | bright-named | '#' hex6 | 'default'
named  := black | red | green | yellow | blue | magenta | cyan | white | gray | grey
modifier := reverse | reversed | dim | bold | italic | underline | underlined
```

Examples:

| value | meaning |
|---|---|
| `cyan` | fg Cyan |
| `cyan+bold` | fg Cyan, BOLD |
| `black on yellow` | fg Black, bg Yellow (search) |
| `white on blue` | tree-open chip |
| `dim` | DIM only |
| `reverse` | REVERSED only |
| `#89b4fa` | fg RGB (truecolor terminals) |
| `#cdd6f4 on #1e1e2e` | truecolor fg on bg |
| `default` | `Style::default()` / Color::Reset |

Rules:

- First color token in a side is fg; `on` starts the bg side.
- Two color tokens on the same side without `on` → warning, ignore line.
- `gray`/`grey` → `Color::Gray`. `bright-cyan` → `Color::LightCyan`
  (ratatui’s `Light*` names). No `Color::Indexed` — `#rrggbb` is the
  escape hatch, not `196`.
- Hex must be exactly 6 digits. `#fff` is a warning, not CSS expansion.
- Case-insensitive tokens. Hex digits hex-case-insensitive.
- No spaces except around `on`. `cyan + bold` warns (unknown). Trim the
  whole value, then split.

This is the entire language. No TOML, no JSON, no nested tables.

---

## Files on disk

Startup only (same as today’s config). No watch, no `:theme`, no key
to cycle. Restart to apply.

```
$XDG_CONFIG_HOME/mrkdup/config     already exists
    theme = default                  # new key; default if absent

$XDG_CONFIG_HOME/mrkdup/theme      optional overlay, applied on top
                                   of the named builtin / named file

$XDG_CONFIG_HOME/mrkdup/themes/<name>   optional full overlay used when
                                        config `theme = <name>` is not
                                        a builtin
```

Load order:

1. `Theme::named(name)` where `name` is `config.theme` (`default` if
   unset). Builtins: `default`, `light`, `mono`.
2. If `name` is **not** a builtin, read `themes/<name>`. Missing file
   → warning, fall back to `default`. File is an overlay on `default`
   (not a required-complete palette).
3. If `theme` (the overlay file) exists, apply it last.

Never-fail: bad lines warn, rest apply. Warnings join the existing
config-warning status bar (`config: ignored N line(s)` — extend the
message to `config/theme: …` or keep one list).

`theme` overlay keys are the **field names** above:
`heading1`, `quote`, `search_match`, `border_focused`, `tree_open`, …
Unknown key → warning. `name` is not settable from the file.

Valid `theme =` names: `^[a-z0-9][a-z0-9_-]{0,31}$`. Anything else
warns and uses `default`.

No per-notes-directory theme. No `mrkdup.toml` next to files.

---

## Builtins (exact palettes)

Implement these as `Theme::default()`, `Theme::light()`, `Theme::mono()`.
`Theme::named("default")` == `Theme::default()`. Tests compare field
equality, not screenshots.

### `default` — current look, pixel-identical

Must keep render tests (`headings_render_in_color` looks for `"Cyan"`,
`search_matches_render_with_yellow_background` looks for `"Yellow"`).

| slot | style |
|---|---|
| border_focused, popup_border | fg Cyan |
| border_unfocused | DIM |
| status_bar, selection, prompt_cursor | REVERSED |
| welcome, mark, done | DIM |
| tree_open | fg White, bg Blue |
| text | default |
| heading1 | fg Cyan, BOLD |
| heading2 | fg Cyan |
| heading | fg Blue |
| bold | BOLD |
| italic | ITALIC |
| code | fg Green |
| checkbox, html_tag | fg Magenta |
| quote | fg Yellow |
| link | fg Blue, UNDERLINED |
| bullet, html_attr | fg Cyan |
| search_match | fg Black, bg Yellow |

`syntax()` mapping:

- `Kind::Mark \| LinkUrl \| HtmlComment` → `mark`
- `Kind::Heading(1)` → `heading1`, `(2)` → `heading2`, `_` → `heading`
- `Kind::CodeInline \| CodeBlock \| HtmlString` → `code`
- `Kind::FmKey` → `html_attr`
- others match the field of the same idea

### `light` — dark fg for light terminal backgrounds

Yellow and Cyan on white are the bug this palette exists to fix. Do
not invent a pastel truecolor scheme here — ANSI only, same slots.

| slot | style |
|---|---|
| border_focused, popup_border | fg Blue |
| border_unfocused | DIM |
| status_bar, selection, prompt_cursor | REVERSED |
| welcome, mark, done | DIM |
| tree_open | fg White, bg Blue |
| text | default |
| heading1 | fg Blue, BOLD |
| heading2 | fg Blue |
| heading | fg Magenta |
| bold / italic | BOLD / ITALIC |
| code | fg Green |
| checkbox, html_tag | fg Magenta |
| quote | fg Red |
| link | fg Blue, UNDERLINED |
| bullet, html_attr | fg Blue |
| search_match | fg Black, bg Yellow |

### `mono` — modifiers only, no `Color::`

For 8-color terminals, screenshots, and “just the text.” TestBackend
debug of an editor pane with `theme=mono` must **not** contain
`Cyan`, `Green`, `Yellow`, `Blue`, `Magenta`, `Red`.

| slot | style |
|---|---|
| border_focused, popup_border | BOLD |
| border_unfocused, welcome, mark, done, quote, code | DIM |
| status_bar, selection, prompt_cursor, tree_open, search_match | REVERSED |
| heading1, bold | BOLD |
| italic | ITALIC |
| link | UNDERLINED |
| everything else | default |

---

## Config parser change

Today `parse` does `value.parse::<i64>()` **before** matching the key.
That makes `theme = default` a “not a number” warning. Fix by matching
on key first:

```rust
match key {
    "tree_width" => { /* parse i64, clamp */ }
    // …existing numeric keys…
    "theme" => {
        if valid_theme_name(value) {
            cfg.theme_name = value.to_string();
        } else {
            warnings.push(format!("line {n}: theme: invalid name {value:?}"));
        }
    }
    _ => warnings.push(format!("line {n}: unknown option: {key}")),
}
```

`Config` gains `pub theme_name: String` (default `"default"`). Do not
put `Theme` on `Config` — config stays serializable data, theme stays
styles.

Existing tests that parse a full file stay green. Add:

- `theme = light` sets `theme_name`
- `theme = Default` is invalid (uppercase) — warn, keep `"default"`
- `theme = 7` is a name, not a number: invalid, warn
- numeric keys still reject non-numbers
- `colour = 7` still “unknown option” (existing test)

---

## Wiring

`App` gains `pub theme: Theme`. `App::new(root, config)` sets
`theme: Theme::named(&config.theme_name)` with **no file I/O** so tests
stay hermetic (`Config { theme_name: "light".into(), ..default() }` is
enough to paint light).

`main.rs`:

```rust
let (config, mut warnings) = config::load();
let (theme, tw) = theme::load(&config.theme_name); // builtin + files
warnings.extend(tw);
let mut app = app::App::new(root, config)?;
app.theme = theme;
```

If that `app.theme =` feels like a hole, add
`App::new_with_theme(root, config, theme)` and keep `new` as
`new_with_theme(..., Theme::named(&config.theme_name))`. Don’t churn
every test either way — pick one, use it everywhere new.

`ui.rs`: `border_style(focused, theme)`, `popup_block(title, theme)`,
tree_open / welcome / status / prompt_cursor all from `app.theme`.
Zero remaining `Color::` in `ui.rs`.

`render.rs`: `style_at(spans, ci, theme)`, search overlay
`st.patch(theme.search_match)` or assign fg+bg from it. Selection
overlay `st.patch(theme.selection)`. Zero remaining `Color::` in
`render.rs`.

`highlight.rs`: drop `use ratatui::style…` and `fn style`. Tokenizer
tests do not change.

Grep gate for the last commit: `Color::` lives only in `src/theme.rs`.

---

## What we are not doing

- No new keybinding to cycle themes. Config is startup-only already.
- No theme picker popup. Ctrl+P is files, not settings.
- No Helix/Alacritty/bat import. Overlay file is the interchange.
- No shipping Catppuccin/Nord/Solarized as builtins. Three palettes +
  `#rrggbb` is enough. A sample overlay may live in the README, not
  in `src/`.
- No `editor_bg` painting the pane.
- No `notify` / live reload of the theme file.
- No per-project `.mrkdup-theme`.
- No new dependencies (`toml`, `serde`, `syntect`).
- No keybind remapping sneaking in on the same branch.
- No stacking `Kind` (Heading+Bold). Themes don’t fix B3.

---

## Conventions (every task)

- TDD: failing test → implement → green.
- `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt`
  before every commit.
- No new dependencies.
- README updates in the same commit as the user-visible change
  (the `theme =` key, the overlay file, the three names).
- Commit messages: `refactor:` for T1 (no user-visible change),
  `feat:` after that.

---

### T1. Extract `Theme`; default palette is current colors

**Behavior-identical.** No config key yet. This is the composability
commit. After it, changing a color is one struct, not three files.

**Files:**
- Create: `src/theme.rs`
- Edit: `src/main.rs` (`mod theme;`), `src/highlight.rs` (delete
  `style`), `src/render.rs`, `src/ui.rs`, `src/app.rs`

**Tests first:**
- `Theme::default().syntax(Kind::Heading(1))` equals today’s
  `Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)`.
  Table-driven over every `Kind` variant vs the current match arms
  (copy them into the test before deleting `highlight::style`).
- Existing `headings_render_in_color` / search Yellow / selection
  REVERSED still pass (App uses `Theme::default()`).

**Steps:**

- [ ] **Step 1:** Write the table-driven `default_matches_legacy_highlight_style`
      test against a `Theme::default()` you haven’t implemented — fail.
- [ ] **Step 2:** `Theme` struct + `default()` + `syntax()`. Wire
      `App.theme = Theme::default()`. Point `style_at` and `ui.rs` at it.
- [ ] **Step 3:** Delete `highlight::style` and its ratatui import.
- [ ] **Step 4:** `rg 'Color::' src` — only `theme.rs` (and this test
      file’s expected values).
- [ ] **Step 5:** test / clippy / fmt
- [ ] **Step 6:** Commit `refactor: extract Theme; syntax colors live in one struct`

If follow-ups C6 has already landed, `EditorView` carries `theme: &'a Theme`
in this same commit instead of `render_editor` reading `app.theme`.

---

### T2. Config key + `light` + `mono`

**Files:** `src/config.rs`, `src/theme.rs`, `src/app.rs`, README
Configuration table.

**Tests first:**
- `parse("theme = light\n")` → `cfg.theme_name == "light"`, no warnings.
- `parse("theme = nope!\n")` → warning, name stays `default`.
- Existing numeric tests still pass (parser now matches key first).
- `App` with `theme_name = "light"`: heading TestBackend dump contains
  `"Blue"`, does **not** require `"Cyan"`.
- `App` with `theme_name = "mono"`: dump of a `# Title` file does not
  contain `Cyan`/`Green`/`Yellow`/`Blue`/`Magenta`/`Red`. Still contains
  `BOLD` (h1) and the text `Title`.
- Search under mono: dump contains `REVERSED` (match overlay), not
  `Yellow`.

`App::new` uses `Theme::named(&config.theme_name)` so tests don’t
touch disk.

**README** (same commit): add `theme` to the config table.

```ini
# ~/.config/mrkdup/config
theme = default    # default | light | mono
```

| Option | Default | Meaning |
|---|---|---|
| `theme` | `default` | `default` (current), `light` (dark fg for light terminals), `mono` (no color, modifiers only) |

- [ ] **Step 1:** Failing parser + render tests
- [ ] **Step 2:** Parser string key; `Theme::light` / `Theme::mono` / `named`
- [ ] **Step 3:** README
- [ ] **Step 4:** test / clippy / fmt
- [ ] **Step 5:** Commit `feat: theme = default|light|mono`

---

### T3. Overlay file + named user themes

**Files:** `src/theme.rs` (`parse_overlay`, `load`), `src/config.rs`
(`config_path` is private — either `pub(crate)` the directory helper
or put path logic in `theme.rs` the same way: `XDG_CONFIG_HOME/mrkdup/`).
`src/main.rs` calls `theme::load`. README.

**Do not** share a process-wide env mutation with
`config::tests::load_reads_xdg_config_home` — that test is already
the one place `XDG_CONFIG_HOME` is touched. Theme file tests should
call `parse_overlay(text, &mut Theme)` **pure**, plus one `load_from`
that takes an explicit dir (so tests don’t need the env var).

```rust
pub fn parse_overlay(text: &str, theme: &mut Theme) -> Vec<String> { /* warnings */ }
pub fn load_from(name: &str, dir: &Path) -> (Theme, Vec<String>);
pub fn load(name: &str) -> (Theme, Vec<String>); // uses xdg dir
```

**Tests (pure overlay):**
- `heading1 = red+bold` on a default theme changes only that slot.
- `search_match = black on yellow` round-trips (default already that).
- `nope = cyan` → warning, theme unchanged.
- `heading1 = cyan + bold` (spaces) → warning.
- `heading1 = #gg0000` → warning.
- `quote = #cc0000` sets RGB fg.
- `parse_style` unit table: every grammar example in this plan.

**Tests (load_from):**
```
dir/
  theme              "quote = red"
  themes/forest      "heading1 = green+bold"
```
- `load_from("default", dir)` → default + quote red.
- `load_from("light", dir)` → light + quote red.
- `load_from("forest", dir)` → default + heading1 green+bold + quote red
  (named file, then overlay file).
- `load_from("missing", dir)` → default + quote red, one warning about
  unknown name / missing file.

**README:** overlay format, slot list, one copy-paste example
(e.g. a dark-truecolor snippet). Slot list is the struct field names,
not a second vocabulary.

- [ ] **Step 1:** `parse_style` table tests, then the function
- [ ] **Step 2:** `parse_overlay` + `load_from` tests, then the functions
- [ ] **Step 3:** `main.rs` uses `theme::load`; warnings in status
- [ ] **Step 4:** README
- [ ] **Step 5:** test / clippy / fmt
- [ ] **Step 6:** Commit `feat: user theme overlay and named theme files`

---

### T4. Grep gate + docs polish

Not a feature. Confirm the contract stuck.

- [ ] `rg 'Color::' src --glob '!theme.rs'` is empty
- [ ] `rg 'highlight::style' src` is empty
- [ ] README Configuration mentions restart-to-apply
- [ ] CONTRIBUTING one line: “colors live in `theme.rs`; don’t add
      `Color::` in ui/render/highlight”
- [ ] Commit `docs: theme color contract (only theme.rs paints)`

Skip T4 if T3’s README already says it and the grep is clean — don’t
make an empty commit.

---

## Suggested execution order

- [ ] T1 extract Theme (behavior-identical)
- [ ] T2 `theme = default|light|mono`
- [ ] T3 overlay file + `themes/<name>`
- [ ] T4 grep gate / CONTRIBUTING (optional if T3 is clean)

T1 is the one that matters if time is short. T2 is the user-visible
feature. T3 is the escape hatch so we never have to vendor Catppuccin.

---

## Interaction with the hardening plan

- **A–B (bugs):** themes must not start; don’t paint over red CI.
- **C6 EditorView:** if C6 lands first, T1 puts `theme: &'a Theme` on
  the view. If T1 lands first, C6 carries `theme` along with `lines`.
  Either order is fine; don’t pass `&mut App` back into `render.rs`.
- **D1 paint cache:** cache keys must include theme identity (`name` is
  enough; theme is immutable for the process). Don’t cache styled
  `Line`s across a theme we can’t change at runtime anyway — but don’t
  bake `Color::Cyan` into the cache either; store `Kind` spans, style
  at paint, same as today.
- **E ship v0.1.0:** does **not** wait on this plan. Themes are 0.2
  (or a fast-follow after the name is grabbed). Shipping `default`
  only is correct.

If an agent is mid-harden and tempted to “just add a Theme while
touching render.rs”: do T1 only, stop. Don’t sneak T2/T3 into a perf
commit.
