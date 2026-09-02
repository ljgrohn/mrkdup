# Settings popup + `firmitas`/`tokyonight` themes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Press `s` in the tree pane to open a settings popup whose first row cycles the theme live and writes the choice back to the config file; ship two more builtin palettes.

**Architecture:** A new `Prompt::Settings` variant holds a list of `SettingRow`s (name, choices, index). `h`/`l` step the selected row and call `App::apply_setting`, which loads the theme through the existing `theme::load_from`, assigns `app.theme`, and persists via a new pure-then-atomic config writer. `App` gains `config_dir: Option<PathBuf>` as the single disk seam so tests never touch `XDG_CONFIG_HOME`.

**Tech Stack:** Rust, ratatui 0.30, crossterm; no new crates.

**Spec:** `docs/superpowers/specs/2026-09-01-settings-popup-design.md`

## Global Constraints

- `Color::` appears only in `src/theme.rs` (gate: `rg 'Color::' src --glob '!src/theme.rs' --glob '!src/theme/**'` prints nothing).
- No builtin paints a full-pane background; `firmitas`/`tokyonight` set foregrounds only, except `status_bar`, `tree_open`, `selection`, `search_match`, which use the palette's own fg/bg pairs listed in the spec.
- Config stays `key = value`, never fails, unknown keys warn. The writer never loses comments, blank lines, or other keys.
- The overlay file `dir/theme` still applies on top of whatever the popup selects (use `theme::load_from`, never `Theme::named`, when a config dir exists).
- `theme::BUILTINS` is `["default", "light", "mono", "firmitas", "tokyonight"]` in that order; `Theme::named`, `is_builtin`, and the popup all consult it.
- Tests live in `src/<module>/tests.rs`. Theme/config/app tests must not set `XDG_CONFIG_HOME` (one existing config test owns it) and must never write to the real config dir: `App::new` (test-only) sets `config_dir: None`.
- Before every commit: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt`.
- Commit trailers (every commit):
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`
  `Claude-Session: https://claude.ai/code/session_01JTx4MwEJXhbdvUCbgPz1Jw`

---

## File map

| file | change |
|---|---|
| `src/theme.rs` | `BUILTINS`, `rgb()`, `Theme::firmitas()`, `Theme::tokyonight()`, `named` arms, `is_builtin` via `BUILTINS`, `list_user_themes(dir)` |
| `src/theme/tests.rs` | palette tables, BUILTINS round-trip, `list_user_themes` fixture tests |
| `src/config.rs` | `valid_theme_name` → `pub(crate)`, `rewrite_theme_line`, `save_theme_name_to` |
| `src/config/tests.rs` | rewrite table, save round-trip |
| `src/app.rs` | `SettingRow`, `Prompt::Settings`, `config_dir` field, `open_settings`, `apply_setting`, key handling |
| `src/app/tests.rs` | popup open/cycle/close tests |
| `src/ui.rs` | draw the popup, status hint, `s` in the cheat sheet |
| `src/render/tests.rs` | TestBackend dump with the popup open |
| `README.md` | Keys row, Configuration prose, five theme names, sample fix |

---

### Task 1: Two new builtins + `BUILTINS` + user theme discovery

**Files:**
- Modify: `src/theme.rs` (`named` at ~158, `is_builtin` at ~189, add `BUILTINS`, `rgb`, two palette fns, `list_user_themes` near `load_from`)
- Modify: `src/config.rs:97` (`fn valid_theme_name` → `pub(crate) fn valid_theme_name`)
- Test: `src/theme/tests.rs`
- Modify: `README.md` (config table row at line ~101 and the Themes paragraph at ~105 list all five names)

**Interfaces:**
- Consumes: `Theme` struct and `Theme::light()` style (existing).
- Produces: `pub const BUILTINS: &[&str]`, `Theme::firmitas()`, `Theme::tokyonight()`, `Theme::named("firmitas"|"tokyonight")`, `pub fn list_user_themes(dir: &Path) -> Vec<String>`, `pub(crate) fn config::valid_theme_name(&str) -> bool`.

- [ ] **Step 1: Write the failing palette + BUILTINS tests**

Append to `src/theme/tests.rs`:

```rust
fn hex(h: u32) -> Color {
    Color::Rgb((h >> 16) as u8, ((h >> 8) & 0xff) as u8, (h & 0xff) as u8)
}
fn fg(h: u32) -> Style {
    Style::default().fg(hex(h))
}
fn fg_mod(h: u32, m: Modifier) -> Style {
    Style::default().fg(hex(h)).add_modifier(m)
}
fn fg_bg(f: u32, b: u32) -> Style {
    Style::default().fg(hex(f)).bg(hex(b))
}

#[test]
fn firmitas_matches_the_spec_table() {
    let t = Theme::firmitas();
    assert_eq!(t.name, "firmitas");
    let expected = [
        ("text", t.text, fg(0xafaaa2)),
        ("bold", t.bold, fg_mod(0xd1ccc4, Modifier::BOLD)),
        ("italic", t.italic, fg_mod(0xd1ccc4, Modifier::ITALIC)),
        ("mark", t.mark, fg(0x6f6b63)),
        ("done", t.done, fg(0x6f6b63)),
        ("welcome", t.welcome, fg(0x6f6b63)),
        ("heading1", t.heading1, fg_mod(0xe3bf79, Modifier::BOLD)),
        ("heading2", t.heading2, fg(0xcea462)),
        ("heading", t.heading, fg(0xd4bda2)),
        ("code", t.code, fg(0xb49d80)),
        ("quote", t.quote, fg(0xad936d)),
        ("link", t.link, fg_mod(0xd4bda2, Modifier::UNDERLINED)),
        ("bullet", t.bullet, fg(0xcea462)),
        ("checkbox", t.checkbox, fg(0xe3bf79)),
        ("html_tag", t.html_tag, fg(0xe3bf79)),
        ("html_attr", t.html_attr, fg(0xb49d80)),
        ("border_focused", t.border_focused, fg(0xd4bda2)),
        ("popup_border", t.popup_border, fg(0xd4bda2)),
        ("border_unfocused", t.border_unfocused, fg(0x514d46)),
        ("status_bar", t.status_bar, fg_bg(0x0c1928, 0xd4bda2)),
        ("tree_open", t.tree_open, fg_bg(0x0c1928, 0xd4bda2)),
        ("selection", t.selection, fg_bg(0xfeeecd, 0x514d46)),
        ("search_match", t.search_match, fg_bg(0x0c1928, 0xe3bf79)),
        (
            "prompt_cursor",
            t.prompt_cursor,
            Style::default().add_modifier(Modifier::REVERSED),
        ),
    ];
    for (slot, got, want) in expected {
        assert_eq!(got, want, "firmitas slot {slot}");
    }
}

#[test]
fn tokyonight_matches_the_spec_table() {
    let t = Theme::tokyonight();
    assert_eq!(t.name, "tokyonight");
    let expected = [
        ("text", t.text, fg(0xc0caf5)),
        ("bold", t.bold, fg_mod(0xc0caf5, Modifier::BOLD)),
        ("italic", t.italic, fg_mod(0xc0caf5, Modifier::ITALIC)),
        ("mark", t.mark, fg(0x565f89)),
        ("done", t.done, fg(0x565f89)),
        ("welcome", t.welcome, fg(0x565f89)),
        ("heading1", t.heading1, fg_mod(0x7aa2f7, Modifier::BOLD)),
        ("heading2", t.heading2, fg(0x7aa2f7)),
        ("heading", t.heading, fg(0xbb9af7)),
        ("code", t.code, fg(0x9ece6a)),
        ("quote", t.quote, fg(0xe0af68)),
        ("link", t.link, fg_mod(0x7aa2f7, Modifier::UNDERLINED)),
        ("bullet", t.bullet, fg(0x7dcfff)),
        ("checkbox", t.checkbox, fg(0xbb9af7)),
        ("html_tag", t.html_tag, fg(0xbb9af7)),
        ("html_attr", t.html_attr, fg(0x7dcfff)),
        ("border_focused", t.border_focused, fg(0x7aa2f7)),
        ("popup_border", t.popup_border, fg(0x7aa2f7)),
        ("border_unfocused", t.border_unfocused, fg(0x3b4261)),
        ("status_bar", t.status_bar, fg_bg(0x1a1b26, 0x7aa2f7)),
        ("tree_open", t.tree_open, fg_bg(0x1a1b26, 0x7aa2f7)),
        ("selection", t.selection, fg_bg(0xc0caf5, 0x33467c)),
        ("search_match", t.search_match, fg_bg(0x1a1b26, 0xe0af68)),
        (
            "prompt_cursor",
            t.prompt_cursor,
            Style::default().add_modifier(Modifier::REVERSED),
        ),
    ];
    for (slot, got, want) in expected {
        assert_eq!(got, want, "tokyonight slot {slot}");
    }
}

#[test]
fn builtins_round_trip_through_named_and_are_valid_names() {
    assert_eq!(
        BUILTINS,
        &["default", "light", "mono", "firmitas", "tokyonight"]
    );
    for name in BUILTINS {
        assert_eq!(Theme::named(name).name, *name, "named({name})");
        assert!(crate::config::valid_theme_name(name), "{name} must be a valid theme name");
    }
    assert_eq!(Theme::named("nope").name, "default");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test theme::tests::firmitas theme::tests::tokyonight theme::tests::builtins_round_trip 2>&1 | tail -20`
Expected: compile errors — `no function or associated item named firmitas`, `cannot find value BUILTINS`, `function valid_theme_name is private`.

- [ ] **Step 3: Implement `BUILTINS`, `rgb`, the two palettes, `named`, `is_builtin`**

In `src/config.rs`, change the signature at line ~97:

```rust
pub(crate) fn valid_theme_name(name: &str) -> bool {
```

In `src/theme.rs`, above `impl Default for Theme` add:

```rust
/// The shipped palettes, in the order the settings popup lists them.
/// `Theme::named` and `is_builtin` both consult this list.
pub const BUILTINS: &[&str] = &["default", "light", "mono", "firmitas", "tokyonight"];

/// `#rrggbb` as a truecolor `Color`, for the builtin truecolor palettes.
const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}
```

Replace `is_builtin`:

```rust
/// One of the shipped palettes (`BUILTINS`). Anything else is a
/// candidate for `themes/<name>` on disk.
fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}
```

Extend `named`:

```rust
    pub fn named(name: &str) -> Theme {
        match name {
            "light" => Theme::light(),
            "mono" => Theme::mono(),
            "firmitas" => Theme::firmitas(),
            "tokyonight" => Theme::tokyonight(),
            _ => Theme::default(),
        }
    }
```

Add the two palettes after `mono()` inside `impl Theme`:

```rust
    /// Omarchy "Firmitas Utilitas Venustas" (navy / limestone / bronze /
    /// gold), foregrounds only — expects the theme's `#0c1928` terminal
    /// background. Source: github.com/OldJobobo/omarchy-firmitas-utilitas-venustas-theme.
    pub fn firmitas() -> Theme {
        let navy = rgb(0x0c1928);
        let graphite = rgb(0x514d46);
        let muted = rgb(0x6f6b63);
        let limestone = rgb(0xafaaa2);
        let marble = rgb(0xd1ccc4);
        let parchment = rgb(0xfeeecd);
        let gold = rgb(0xcea462);
        let sandstone = rgb(0xb49d80);
        let travertine = rgb(0xd4bda2);
        let gilded = rgb(0xe3bf79);
        let bronze = rgb(0xad936d);
        Theme {
            name: "firmitas".to_string(),
            border_focused: Style::default().fg(travertine),
            border_unfocused: Style::default().fg(graphite),
            popup_border: Style::default().fg(travertine),
            status_bar: Style::default().fg(navy).bg(travertine),
            selection: Style::default().fg(parchment).bg(graphite),
            prompt_cursor: Style::default().add_modifier(Modifier::REVERSED),
            welcome: Style::default().fg(muted),
            tree_open: Style::default().fg(navy).bg(travertine),
            text: Style::default().fg(limestone),
            mark: Style::default().fg(muted),
            heading1: Style::default().fg(gilded).add_modifier(Modifier::BOLD),
            heading2: Style::default().fg(gold),
            heading: Style::default().fg(travertine),
            bold: Style::default().fg(marble).add_modifier(Modifier::BOLD),
            italic: Style::default().fg(marble).add_modifier(Modifier::ITALIC),
            code: Style::default().fg(sandstone),
            checkbox: Style::default().fg(gilded),
            done: Style::default().fg(muted),
            quote: Style::default().fg(bronze),
            link: Style::default().fg(travertine).add_modifier(Modifier::UNDERLINED),
            bullet: Style::default().fg(gold),
            html_tag: Style::default().fg(gilded),
            html_attr: Style::default().fg(sandstone),
            search_match: Style::default().fg(navy).bg(gilded),
        }
    }

    /// Tokyo Night ("night" variant), foregrounds only — expects the
    /// theme's `#1a1b26` terminal background.
    pub fn tokyonight() -> Theme {
        let bg = rgb(0x1a1b26);
        let fg = rgb(0xc0caf5);
        let comment = rgb(0x565f89);
        let gutter = rgb(0x3b4261);
        let visual = rgb(0x33467c);
        let blue = rgb(0x7aa2f7);
        let cyan = rgb(0x7dcfff);
        let green = rgb(0x9ece6a);
        let magenta = rgb(0xbb9af7);
        let yellow = rgb(0xe0af68);
        Theme {
            name: "tokyonight".to_string(),
            border_focused: Style::default().fg(blue),
            border_unfocused: Style::default().fg(gutter),
            popup_border: Style::default().fg(blue),
            status_bar: Style::default().fg(bg).bg(blue),
            selection: Style::default().fg(fg).bg(visual),
            prompt_cursor: Style::default().add_modifier(Modifier::REVERSED),
            welcome: Style::default().fg(comment),
            tree_open: Style::default().fg(bg).bg(blue),
            text: Style::default().fg(fg),
            mark: Style::default().fg(comment),
            heading1: Style::default().fg(blue).add_modifier(Modifier::BOLD),
            heading2: Style::default().fg(blue),
            heading: Style::default().fg(magenta),
            bold: Style::default().fg(fg).add_modifier(Modifier::BOLD),
            italic: Style::default().fg(fg).add_modifier(Modifier::ITALIC),
            code: Style::default().fg(green),
            checkbox: Style::default().fg(magenta),
            done: Style::default().fg(comment),
            quote: Style::default().fg(yellow),
            link: Style::default().fg(blue).add_modifier(Modifier::UNDERLINED),
            bullet: Style::default().fg(cyan),
            html_tag: Style::default().fg(magenta),
            html_attr: Style::default().fg(cyan),
            search_match: Style::default().fg(bg).bg(yellow),
        }
    }
```

Update the doc comment on `named` and the module doc's "three builtins" wording to say `BUILTINS`.

- [ ] **Step 4: Run the three tests to verify they pass**

Run: `cargo test theme::tests:: 2>&1 | tail -5`
Expected: all theme tests pass, including the three new ones.

- [ ] **Step 5: Write the failing `list_user_themes` tests**

Append to `src/theme/tests.rs`:

```rust
fn list_fixture(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mrkdup-theme-list-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("themes").join("subdir")).unwrap();
    std::fs::write(dir.join("themes/forest"), "heading1 = green\n").unwrap();
    std::fs::write(dir.join("themes/aurora"), "quote = red\n").unwrap();
    std::fs::write(dir.join("themes/Bad!"), "quote = red\n").unwrap();
    std::fs::write(dir.join("themes/mono"), "quote = red\n").unwrap();
    dir
}

#[test]
fn list_user_themes_is_sorted_and_skips_invalid_shadowed_and_dirs() {
    let dir = list_fixture("basic");
    assert_eq!(list_user_themes(&dir), vec!["aurora".to_string(), "forest".to_string()]);
}

#[test]
fn list_user_themes_is_empty_without_a_themes_dir() {
    let dir = std::env::temp_dir().join("mrkdup-theme-list-missing");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(list_user_themes(&dir).is_empty());
}
```

- [ ] **Step 6: Run them to verify they fail**

Run: `cargo test theme::tests::list_user_themes 2>&1 | tail -5`
Expected: compile error `cannot find function list_user_themes`.

- [ ] **Step 7: Implement `list_user_themes`**

In `src/theme.rs`, before `load_from`:

```rust
/// Names of the user's theme files in `dir/themes/`, sorted. Skips
/// names that fail `config::valid_theme_name`, names that shadow a
/// builtin (the loader would pick the builtin anyway), and anything
/// that isn't a regular file. A missing or unreadable directory is
/// simply empty. Used by the settings popup to build its choice list.
pub fn list_user_themes(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir.join("themes")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| crate::config::valid_theme_name(n) && !is_builtin(n))
        .collect();
    names.sort();
    names
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test theme::tests::list_user_themes 2>&1 | tail -5`
Expected: 2 passed.

- [ ] **Step 9: README names**

In `README.md`, the config-table row for `theme` (line ~101) becomes:

```
| `theme` | `default` | see below | `default` (current look), `light` (dark fg for light terminals), `mono` (no color, modifiers only), `firmitas` (Omarchy navy/bronze/gold, truecolor), `tokyonight` (truecolor), or any other name — see Themes |
```

In the Themes paragraph (line ~105) replace "one of the three builtins (`default`, `light`, `mono`)" with "one of the five builtins (`default`, `light`, `mono`, `firmitas`, `tokyonight`)". Add one sentence after it: "`firmitas` and `tokyonight` are truecolor and set foregrounds only — they assume the matching terminal background (`#0c1928` / `#1a1b26`)."

- [ ] **Step 10: Full suite, clippy, fmt, grep gate**

Run: `cargo test 2>&1 | tail -3 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo fmt && rg 'Color::' src --glob '!src/theme.rs' --glob '!src/theme/**'`
Expected: all tests pass, clippy clean, grep prints nothing.

- [ ] **Step 11: Commit**

```bash
git add src/theme.rs src/theme/tests.rs src/config.rs README.md
git commit -m "feat: firmitas and tokyonight builtin themes"
```

---

### Task 2: Config writer for the `theme` key

**Files:**
- Modify: `src/config.rs` (add `use std::io;` and `use std::path::Path;`, two new fns after `load`)
- Test: `src/config/tests.rs`
- Modify: `README.md` config sample (line ~91: move the inline comment to its own line)

**Interfaces:**
- Consumes: `crate::fsutil::atomic_write(path: &Path, contents: &[u8]) -> io::Result<()>` (existing).
- Produces: `pub fn rewrite_theme_line(text: &str, name: &str) -> String`, `pub fn save_theme_name_to(path: &Path, name: &str) -> io::Result<()>`.

- [ ] **Step 1: Write the failing rewrite tests**

Append to `src/config/tests.rs`:

```rust
#[test]
fn rewrite_theme_line_replaces_in_place_and_keeps_everything_else() {
    let text = "# my config\ntree_width = 40\n  theme = light\nautosave_seconds = 5\n";
    assert_eq!(
        rewrite_theme_line(text, "mono"),
        "# my config\ntree_width = 40\n  theme = mono\nautosave_seconds = 5\n"
    );
}

#[test]
fn rewrite_theme_line_ignores_commented_out_and_lookalike_keys() {
    let text = "# theme = light\ntheme_name = x\nthemes = y\n";
    assert_eq!(
        rewrite_theme_line(text, "mono"),
        "# theme = light\ntheme_name = x\nthemes = y\ntheme = mono\n"
    );
}

#[test]
fn rewrite_theme_line_only_rewrites_the_first_duplicate() {
    let text = "theme=light\ntheme = mono\n";
    assert_eq!(rewrite_theme_line(text, "firmitas"), "theme = firmitas\ntheme = mono\n");
}

#[test]
fn rewrite_theme_line_appends_with_and_without_trailing_newline() {
    assert_eq!(rewrite_theme_line("tree_width = 40\n", "light"), "tree_width = 40\ntheme = light\n");
    assert_eq!(rewrite_theme_line("tree_width = 40", "light"), "tree_width = 40\ntheme = light\n");
    assert_eq!(rewrite_theme_line("", "light"), "theme = light\n");
}

#[test]
fn rewrite_theme_line_output_reparses_to_the_new_name() {
    let out = rewrite_theme_line("tree_width = 40\ntheme = light\n", "tokyonight");
    let (cfg, warnings) = parse(&out);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(cfg.theme_name, "tokyonight");
    assert_eq!(cfg.tree_width, 40);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test config::tests::rewrite_theme_line 2>&1 | tail -5`
Expected: compile error `cannot find function rewrite_theme_line`.

- [ ] **Step 3: Implement `rewrite_theme_line`**

In `src/config.rs` after `load`:

```rust
/// `text` with the first `theme = …` line replaced by `theme = <name>`
/// (leading indentation kept; the parser has no inline comments, so the
/// whole rest of the line is the value), or with `theme = <name>`
/// appended when no such line exists. Commented-out lines and other
/// keys that merely start with `theme` (`theme_name`) don't count.
/// Other lines are copied verbatim; the output always ends in `\n`.
/// Pure, so the settings popup's write-back is testable without disk.
pub fn rewrite_theme_line(text: &str, name: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut replaced = false;
    for line in text.lines() {
        if !replaced && is_theme_line(line) {
            let indent = &line[..line.len() - line.trim_start().len()];
            out.push_str(indent);
            out.push_str("theme = ");
            out.push_str(name);
            replaced = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !replaced {
        out.push_str("theme = ");
        out.push_str(name);
        out.push('\n');
    }
    out
}

/// `theme` followed by optional spaces and `=`, after any indentation.
fn is_theme_line(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("theme")
        .map(|rest| rest.trim_start().starts_with('='))
        .unwrap_or(false)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test config::tests::rewrite_theme_line 2>&1 | tail -5`
Expected: 5 passed.

- [ ] **Step 5: Write the failing save round-trip test**

Append to `src/config/tests.rs`:

```rust
#[test]
fn save_theme_name_to_creates_the_file_and_preserves_other_keys() {
    let dir = std::env::temp_dir().join("mrkdup-config-save");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("mrkdup").join("config");

    // missing dir + file: created with just the theme line
    save_theme_name_to(&path, "mono").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "theme = mono\n");

    // existing file: other keys and comments survive, theme rewritten
    std::fs::write(&path, "# keep me\ntree_width = 42\ntheme = mono\n").unwrap();
    save_theme_name_to(&path, "light").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "# keep me\ntree_width = 42\ntheme = light\n");
    let (cfg, warnings) = parse(&text);
    assert!(warnings.is_empty());
    assert_eq!(cfg.tree_width, 42);
    assert_eq!(cfg.theme_name, "light");
}
```

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test config::tests::save_theme_name_to 2>&1 | tail -5`
Expected: compile error `cannot find function save_theme_name_to`.

- [ ] **Step 7: Implement `save_theme_name_to`**

In `src/config.rs`, add `use std::io;` and `use std::path::{Path, PathBuf};` at the top (replacing the existing `use std::path::PathBuf;`), then after `rewrite_theme_line`:

```rust
/// Persist `name` as the `theme` key of the config file at `path`:
/// read it (missing = empty), rewrite the line, write atomically.
/// Creates the parent directory if needed. Only the settings popup
/// calls this — startup never writes the config.
pub fn save_theme_name_to(path: &Path, name: &str) -> io::Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::fsutil::atomic_write(path, rewrite_theme_line(&text, name).as_bytes())
}
```

- [ ] **Step 8: Run it to verify it passes**

Run: `cargo test config::tests::save_theme_name_to 2>&1 | tail -5`
Expected: 1 passed.

- [ ] **Step 9: README sample fix**

In `README.md`, the config sample block (line ~86-92) becomes:

```ini
# ~/.config/mrkdup/config
tree_width = 30
side_margin_percent = 5
autosave_seconds = 10
# default | light | mono | firmitas | tokyonight
theme = default
```

(The old sample put the comment on the `theme` line, which the parser rejects as an invalid name.)

- [ ] **Step 10: Full suite, clippy, fmt**

Run: `cargo test 2>&1 | tail -3 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo fmt`
Expected: all pass, clippy clean.

- [ ] **Step 11: Commit**

```bash
git add src/config.rs src/config/tests.rs README.md
git commit -m "feat: config writer for the theme key"
```

---

### Task 3: `Prompt::Settings` — open, cycle live, persist, close

**Files:**
- Modify: `src/app.rs` (`Prompt` enum ~18, `App` struct ~50, `new` ~62, `new_with_theme` ~71, `tree_key` ~173, `prompt_key` ~505)
- Test: `src/app/tests.rs`

**Interfaces:**
- Consumes: `theme::BUILTINS`, `theme::list_user_themes(&Path)`, `theme::load_from(&str, &Path)`, `Theme::named(&str)`, `config::config_dir() -> Option<PathBuf>`, `config::save_theme_name_to(&Path, &str)`.
- Produces: `pub struct SettingRow { name: &'static str, choices: Vec<String>, index: usize }` with `value()` and `step(delta)`, `Prompt::Settings { rows: Vec<SettingRow>, selected: usize }`, `App.config_dir: Option<PathBuf>`. Task 4 draws these.

- [ ] **Step 1: Write the failing tests**

Append to `src/app/tests.rs`:

```rust
#[test]
fn s_in_tree_opens_settings_on_the_current_theme() {
    let cfg = Config {
        theme_name: "mono".into(),
        ..Config::default()
    };
    let mut app = App::new(fixture("settings-open"), cfg).unwrap();
    app.handle_key(key(KeyCode::Char('s')));
    let Prompt::Settings { rows, selected } = &app.prompt else {
        panic!("expected Settings prompt");
    };
    assert_eq!(*selected, 0);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "theme");
    assert_eq!(rows[0].value(), "mono");
    assert_eq!(
        rows[0].choices,
        vec!["default", "light", "mono", "firmitas", "tokyonight"]
    );
}

#[test]
fn settings_l_and_h_cycle_the_theme_live_and_wrap() {
    let mut app = App::new(fixture("settings-cycle"), Config::default()).unwrap();
    assert!(app.config_dir.is_none(), "tests must never write the real config");
    app.handle_key(key(KeyCode::Char('s')));
    app.handle_key(key(KeyCode::Char('l')));
    assert_eq!(app.theme, Theme::light());
    assert_eq!(app.config.theme_name, "light");
    assert_eq!(app.status.as_deref(), Some("theme: light (not saved: no config dir)"));

    app.handle_key(key(KeyCode::Right));
    assert_eq!(app.theme, Theme::mono());

    // wrap backwards from index 0
    app.handle_key(key(KeyCode::Char('h')));
    app.handle_key(key(KeyCode::Left));
    app.handle_key(key(KeyCode::Char('h')));
    assert_eq!(app.theme.name, "tokyonight");
    assert!(matches!(app.prompt, Prompt::Settings { .. }), "popup stays open while cycling");
}

#[test]
fn settings_close_keys_and_editor_s_still_types() {
    let root = fixture("settings-close");
    let mut app = App::new(root, Config::default()).unwrap();
    for close in [KeyCode::Esc, KeyCode::Enter, KeyCode::Char('s')] {
        app.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(app.prompt, Prompt::Settings { .. }));
        app.handle_key(key(close));
        assert!(matches!(app.prompt, Prompt::None), "{close:?} should close");
    }
    // in the editor, s is a character
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(key(KeyCode::Char('s')));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(app.editor.lines()[0].starts_with('s'), "{:?}", app.editor.lines()[0]);
}

#[test]
fn settings_persists_the_choice_and_lists_user_themes_from_config_dir() {
    let root = fixture("settings-persist");
    let cfg_dir = root.parent().unwrap().join("xdg");
    std::fs::create_dir_all(cfg_dir.join("themes")).unwrap();
    std::fs::write(cfg_dir.join("themes/forest"), "heading1 = green+bold\n").unwrap();
    std::fs::write(cfg_dir.join("config"), "tree_width = 33\ntheme = default\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.config_dir = Some(cfg_dir.clone());

    app.handle_key(key(KeyCode::Char('s')));
    let Prompt::Settings { rows, .. } = &app.prompt else {
        panic!("expected Settings prompt");
    };
    assert_eq!(rows[0].choices.last().map(String::as_str), Some("forest"));

    app.handle_key(key(KeyCode::Char('h'))); // wraps to the last choice: forest
    assert_eq!(app.config.theme_name, "forest");
    assert_eq!(app.status.as_deref(), Some("theme: forest"));
    // the named file was applied (no `Color::` here — keep the grep gate clean)
    let mut expected = Theme::default();
    crate::theme::parse_overlay("heading1 = green+bold\n", &mut expected);
    assert_eq!(app.theme.heading1, expected.heading1);
    assert_eq!(
        std::fs::read_to_string(cfg_dir.join("config")).unwrap(),
        "tree_width = 33\ntheme = forest\n"
    );
}
```

Do **not** write `Color::` anywhere in `src/app/tests.rs`. `Theme` is in scope via `use super::*` (app.rs imports it); add `use crate::theme::Theme;` only if the compiler complains.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test app::tests::settings app::tests::s_in_tree 2>&1 | tail -8`
Expected: compile errors — `no variant named Settings`, `no field config_dir`.

- [ ] **Step 3: Implement the row type, the variant, the field, the seam**

In `src/app.rs`, after the `Prompt` enum:

```rust
/// One row of the settings popup: a named option with a fixed list of
/// choices and the index of the current one. `h`/`l` step it; the
/// popup shows `name ‹ value ›`.
pub struct SettingRow {
    pub name: &'static str,
    pub choices: Vec<String>,
    pub index: usize,
}

impl SettingRow {
    pub fn value(&self) -> &str {
        &self.choices[self.index]
    }

    /// Move `delta` choices (±1), wrapping, and return the new value.
    pub fn step(&mut self, delta: isize) -> String {
        let n = self.choices.len() as isize;
        self.index = (self.index as isize + delta).rem_euclid(n) as usize;
        self.choices[self.index].clone()
    }
}
```

Add the variant to `Prompt` (after `GoToFile`):

```rust
    /// The settings list (`s` in the tree): one row per option, `h`/`l`
    /// cycle the selected row's value and apply it immediately.
    Settings {
        rows: Vec<SettingRow>,
        selected: usize,
    },
```

Add the field to `App` (after `pub theme: Theme,`):

```rust
    /// `$XDG_CONFIG_HOME/mrkdup`: where the settings popup reads
    /// `themes/` and writes `config`. `None` = no HOME; tests set it
    /// explicitly so they never touch the real config.
    pub config_dir: Option<PathBuf>,
```

In `new_with_theme`, initialise it: `config_dir: crate::config::config_dir(),`.

In the test-only `new`, force it off after construction:

```rust
    #[cfg(test)]
    pub fn new(root: PathBuf, config: Config) -> io::Result<App> {
        let theme = Theme::named(&config.theme_name);
        let mut app = App::new_with_theme(root, config, theme)?;
        app.config_dir = None; // tests must never write the user's real config
        Ok(app)
    }
```

- [ ] **Step 4: Open, apply, and key handling**

In `tree_key`, after the `'?'` arm:

```rust
            KeyCode::Char('s') => self.open_settings(),
```

Add to `impl App` (near `open_go_to_file`):

```rust
    /// Build the settings rows. The theme row lists the builtins, then
    /// the user's `themes/` files; it starts on the configured name (or
    /// `default` if that name isn't in the list).
    fn open_settings(&mut self) {
        let mut choices: Vec<String> = crate::theme::BUILTINS
            .iter()
            .map(|s| s.to_string())
            .collect();
        if let Some(dir) = &self.config_dir {
            choices.extend(crate::theme::list_user_themes(dir));
        }
        let index = choices
            .iter()
            .position(|c| *c == self.config.theme_name)
            .unwrap_or(0);
        self.prompt = Prompt::Settings {
            rows: vec![SettingRow {
                name: "theme",
                choices,
                index,
            }],
            selected: 0,
        };
    }

    /// A settings row changed: apply the new value live and persist it.
    /// Only `theme` exists today. The theme goes through `load_from` so
    /// `themes/<name>` and the overlay file both apply, exactly as at
    /// startup; the config file is rewritten in place.
    fn apply_setting(&mut self, row: &str, value: &str) {
        if row != "theme" {
            return;
        }
        let (theme, warnings, saved) = match &self.config_dir {
            Some(dir) => {
                let (theme, warnings) = crate::theme::load_from(value, dir);
                let saved = crate::config::save_theme_name_to(&dir.join("config"), value)
                    .map_err(|e| e.to_string());
                (theme, warnings, saved)
            }
            None => (Theme::named(value), Vec::new(), Err("no config dir".to_string())),
        };
        self.theme = theme;
        self.config.theme_name = value.to_string();
        self.status = Some(match (saved, warnings.first()) {
            (Ok(()), None) => format!("theme: {value}"),
            (Ok(()), Some(w)) => format!("theme: {value} — {w}"),
            (Err(e), _) => format!("theme: {value} (not saved: {e})"),
        });
    }
```

In `prompt_key`, the `match &mut self.prompt` gains an arm (Esc is already handled above the match; add it before the `GoToFile` arm or after `ConfirmDelete` — anywhere the compiler is happy):

```rust
            Prompt::Settings { rows, selected } => {
                let last = rows.len().saturating_sub(1);
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => *selected = (*selected + 1).min(last),
                    KeyCode::Char('k') | KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('l') | KeyCode::Right => {
                        let delta = if matches!(key.code, KeyCode::Char('l') | KeyCode::Right) {
                            1
                        } else {
                            -1
                        };
                        let row = &mut rows[*selected];
                        let name = row.name;
                        let value = row.step(delta);
                        self.apply_setting(name, &value);
                    }
                    KeyCode::Enter | KeyCode::Char('s') => self.prompt = Prompt::None,
                    _ => {}
                }
            }
```

(`rows`/`selected` are last used before `self.apply_setting` / `self.prompt = …`, so the borrow of `self.prompt` has ended by then; if the borrow checker disagrees, compute `(name, value)` inside a block that ends before the call.)

Also confirm `handle_key`'s `self.status = None` at the top does not defeat the message: `apply_setting` sets `status` *after* that reset, so the message shows on the next frame (Task 4 renders it in the popup's hint line).

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test app::tests:: 2>&1 | tail -5`
Expected: all app tests pass including the four new ones.

- [ ] **Step 6: Full suite, clippy, fmt, grep gate**

Run: `cargo test 2>&1 | tail -3 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo fmt && rg 'Color::' src --glob '!src/theme.rs' --glob '!src/theme/**'`
Expected: all pass; clippy clean (a `match` on `&mut self.prompt` with an unused `Settings` arm in `ui.rs` will not warn — `draw_popup` uses `if let`); grep prints nothing.

- [ ] **Step 7: Commit**

```bash
git add src/app.rs src/app/tests.rs
git commit -m "feat: s opens a settings popup that switches the theme live"
```

---

### Task 4: Draw the popup, hints, cheat sheet, docs

**Files:**
- Modify: `src/ui.rs` (`draw_popup` ~74, `key_lines` ~325, `draw_status` ~370)
- Test: `src/render/tests.rs`
- Modify: `README.md` (Keys table ~line 56, Configuration prose ~75, Themes "read once" sentence ~115)
- Modify: `docs/superpowers/plans/2026-08-31-mrkdup-themes.md` ("What we are not doing": one-line note)

**Interfaces:**
- Consumes: `Prompt::Settings { rows, selected }`, `SettingRow { name, choices, index }` + `value()`, `app.status`, `popup_block(title, theme)`, `centered_rect`.

- [ ] **Step 1: Write the failing render test**

Append to `src/render/tests.rs`:

```rust
#[test]
fn settings_popup_renders_the_theme_row_and_status() {
    let root = fixture("settings-popup", "# Title\n");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('s')));
    let text = draw_to_string(&mut app);
    assert!(text.contains("Settings"), "title missing: {text}");
    assert!(text.contains("theme"), "row name missing: {text}");
    assert!(text.contains("‹ default ›"), "value missing: {text}");

    app.handle_key(key(KeyCode::Char('l')));
    let text = draw_to_string(&mut app);
    assert!(text.contains("‹ light ›"), "cycled value missing: {text}");
    assert!(
        text.contains("theme: light (not saved: no config dir)"),
        "status missing from hint line: {text}"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test render::tests::settings_popup 2>&1 | tail -5`
Expected: FAIL on `title missing` (the popup isn't drawn yet; keys already work from Task 3).

- [ ] **Step 3: Draw the popup**

In `src/ui.rs` `draw_popup`, after the `ConfirmDelete` block:

```rust
    if let Prompt::Settings { rows, selected } = &app.prompt {
        let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(0);
        let value_w = rows.iter().map(|r| r.value().len()).max().unwrap_or(0);
        let width = ((name_w + value_w + 12) as u16).max(40);
        let height = (rows.len() as u16 + 2).min(area.height);
        let popup = centered_rect(width, height, area);
        f.render_widget(Clear, popup);
        let block = popup_block(" Settings ", theme);
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        let lines: Vec<Line> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let value = format!("‹ {} ›", r.value());
                let gap = (inner.width as usize)
                    .saturating_sub(r.name.len() + value.len() + 2)
                    .max(1);
                let mut line = Line::from(format!(" {}{}{} ", r.name, " ".repeat(gap), value));
                if i == *selected {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }
```

- [ ] **Step 4: Hint line with status, and the cheat sheet**

In `draw_status`, add an arm before `Prompt::None`:

```rust
        Prompt::Settings { .. } => {
            let mut s = format!("{mode}| h/l or ←/→ change · j/k move · Esc close");
            if let Some(msg) = &app.status {
                s.push_str("  —  ");
                s.push_str(msg);
            }
            s
        }
```

In `key_lines`, add `("s", "settings (theme)"),` after the `("?", "help")` entry.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test render::tests::settings_popup 2>&1 | tail -5`
Expected: 1 passed.

- [ ] **Step 6: Docs**

`README.md`:

- Keys table: after the `?` row add
  `| tree | s | settings popup: h/l or ←/→ cycle the theme (applies live and is written to the config file); Esc closes |`
- Configuration intro (after "with a one-line warning in the status bar."): add a sentence: "The `theme` key can also be changed from inside mrkdup: press `s` in the tree pane; the choice applies immediately and is written back to this file."
- Themes section: change "both are read once at startup — restart mrkdup to apply a change; there's no live reload or in-app theme switcher." to "both files are read when the theme is (re)applied — at startup, or when you pick a theme with `s` — so edit a theme file, then re-select the theme to see it; there's no file watcher."

`docs/superpowers/plans/2026-08-31-mrkdup-themes.md`, under "What we are not doing", append one line:
`- (2026-09-01: the picker popup and live switch landed after all — see specs/2026-09-01-settings-popup-design.md.)`

- [ ] **Step 7: Full suite, clippy, fmt, grep gate**

Run: `cargo test 2>&1 | tail -3 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo fmt && rg 'Color::' src --glob '!src/theme.rs' --glob '!src/theme/**'`
Expected: all pass, clippy clean, grep prints nothing.

- [ ] **Step 8: Commit**

```bash
git add src/ui.rs src/render/tests.rs README.md docs/superpowers/plans/2026-08-31-mrkdup-themes.md
git commit -m "feat: settings popup rendering, s in the cheat sheet, docs"
```

---

## Execution order

- [ ] Task 1 builtins + discovery
- [ ] Task 2 config writer
- [ ] Task 3 popup logic
- [ ] Task 4 popup rendering + docs
