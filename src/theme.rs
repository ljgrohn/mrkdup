//! Color theme: every `Color::`/modifier literal that used to be
//! scattered across `highlight.rs`, `render.rs`, and `ui.rs` lives here,
//! in one struct. `Theme::default()` is pixel-identical to the old
//! hardcoded look — this module is purely a composability refactor.
//!
//! Users can override slots at startup via an overlay file
//! (`$XDG_CONFIG_HOME/mrkdup/theme`) and/or a named full theme file
//! (`$XDG_CONFIG_HOME/mrkdup/themes/<name>`) — see `load` and
//! `parse_overlay`.

use std::path::Path;

use ratatui::style::{Color, Modifier, Style};

use crate::highlight::Kind;

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    // chrome
    pub border_focused: Style,
    pub border_unfocused: Style,
    pub popup_border: Style,
    pub status_bar: Style,
    pub selection: Style,     // overlay; default = REVERSED
    pub prompt_cursor: Style, // the reversed " " in input popups
    pub welcome: Style,
    pub tree_open: Style, // currently White on Blue
    pub tab_active: Style,
    pub tab_inactive: Style,
    // syntax
    pub text: Style,
    pub mark: Style, // also LinkUrl, HtmlComment
    pub heading1: Style,
    pub heading2: Style,
    pub heading: Style, // h3+
    pub bold: Style,
    pub italic: Style,
    pub code: Style, // inline, fence, HtmlString
    pub checkbox: Style,
    pub done: Style,
    pub quote: Style,
    pub link: Style,
    pub bullet: Style,
    pub html_tag: Style,
    pub html_attr: Style,    // also FmKey
    pub search_match: Style, // overlay; default = Black on Yellow
}

/// The shipped palettes, in the order the settings popup lists them.
/// `Theme::named` and `is_builtin` both consult this list.
pub const BUILTINS: &[&str] = &["default", "light", "mono", "firmitas", "tokyonight"];

/// `#rrggbb` as a truecolor `Color`, for the builtin truecolor palettes.
const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        (hex >> 16) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            name: "default".to_string(),
            border_focused: Style::default().fg(Color::Cyan),
            border_unfocused: Style::default().add_modifier(Modifier::DIM),
            popup_border: Style::default().fg(Color::Cyan),
            status_bar: Style::default().add_modifier(Modifier::REVERSED),
            selection: Style::default().add_modifier(Modifier::REVERSED),
            prompt_cursor: Style::default().add_modifier(Modifier::REVERSED),
            welcome: Style::default().add_modifier(Modifier::DIM),
            tree_open: Style::default().fg(Color::White).bg(Color::Blue),
            tab_active: Style::default().fg(Color::White).bg(Color::Blue),
            tab_inactive: Style::default().add_modifier(Modifier::DIM),
            text: Style::default(),
            mark: Style::default().add_modifier(Modifier::DIM),
            heading1: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            heading2: Style::default().fg(Color::Cyan),
            heading: Style::default().fg(Color::Blue),
            bold: Style::default().add_modifier(Modifier::BOLD),
            italic: Style::default().add_modifier(Modifier::ITALIC),
            code: Style::default().fg(Color::Green),
            checkbox: Style::default().fg(Color::Magenta),
            done: Style::default().add_modifier(Modifier::DIM),
            quote: Style::default().fg(Color::Yellow),
            link: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            bullet: Style::default().fg(Color::Cyan),
            html_tag: Style::default().fg(Color::Magenta),
            html_attr: Style::default().fg(Color::Cyan),
            search_match: Style::default().fg(Color::Black).bg(Color::Yellow),
        }
    }
}

impl Theme {
    /// Dark foreground palette for light terminal backgrounds. Same
    /// slots as `default`, ANSI colors only — no invented pastel scheme.
    pub fn light() -> Theme {
        Theme {
            name: "light".to_string(),
            border_focused: Style::default().fg(Color::Blue),
            border_unfocused: Style::default().add_modifier(Modifier::DIM),
            popup_border: Style::default().fg(Color::Blue),
            status_bar: Style::default().add_modifier(Modifier::REVERSED),
            selection: Style::default().add_modifier(Modifier::REVERSED),
            prompt_cursor: Style::default().add_modifier(Modifier::REVERSED),
            welcome: Style::default().add_modifier(Modifier::DIM),
            tree_open: Style::default().fg(Color::White).bg(Color::Blue),
            tab_active: Style::default().fg(Color::White).bg(Color::Blue),
            tab_inactive: Style::default().add_modifier(Modifier::DIM),
            text: Style::default(),
            mark: Style::default().add_modifier(Modifier::DIM),
            heading1: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            heading2: Style::default().fg(Color::Blue),
            heading: Style::default().fg(Color::Magenta),
            bold: Style::default().add_modifier(Modifier::BOLD),
            italic: Style::default().add_modifier(Modifier::ITALIC),
            code: Style::default().fg(Color::Green),
            checkbox: Style::default().fg(Color::Magenta),
            done: Style::default().add_modifier(Modifier::DIM),
            quote: Style::default().fg(Color::Red),
            link: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            bullet: Style::default().fg(Color::Blue),
            html_tag: Style::default().fg(Color::Magenta),
            html_attr: Style::default().fg(Color::Blue),
            search_match: Style::default().fg(Color::Black).bg(Color::Yellow),
        }
    }

    /// Modifiers only, no `Color::` at all. For 8-color terminals,
    /// screenshots, and "just the text."
    pub fn mono() -> Theme {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let reversed = Style::default().add_modifier(Modifier::REVERSED);
        Theme {
            name: "mono".to_string(),
            border_focused: bold,
            border_unfocused: dim,
            popup_border: bold,
            status_bar: reversed,
            selection: reversed,
            prompt_cursor: reversed,
            welcome: dim,
            tree_open: reversed,
            tab_active: reversed,
            tab_inactive: Style::default().add_modifier(Modifier::DIM),
            text: Style::default(),
            mark: dim,
            heading1: bold,
            heading2: Style::default(),
            heading: Style::default(),
            bold,
            italic: Style::default().add_modifier(Modifier::ITALIC),
            code: dim,
            checkbox: Style::default(),
            done: dim,
            quote: dim,
            link: Style::default().add_modifier(Modifier::UNDERLINED),
            bullet: Style::default(),
            html_tag: Style::default(),
            html_attr: Style::default(),
            search_match: reversed,
        }
    }

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
            tab_active: Style::default().fg(navy).bg(travertine),
            tab_inactive: Style::default().add_modifier(Modifier::DIM),
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
            link: Style::default()
                .fg(travertine)
                .add_modifier(Modifier::UNDERLINED),
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
            tab_active: Style::default().fg(bg).bg(blue),
            tab_inactive: Style::default().add_modifier(Modifier::DIM),
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

    /// The builtin named `name`, or `Theme::default()` if `name` isn't
    /// one of the builtins.
    pub fn named(name: &str) -> Theme {
        match name {
            "light" => Theme::light(),
            "mono" => Theme::mono(),
            "firmitas" => Theme::firmitas(),
            "tokyonight" => Theme::tokyonight(),
            _ => Theme::default(),
        }
    }

    /// Style for one syntax `Kind`, per this theme's palette.
    pub fn syntax(&self, kind: Kind) -> Style {
        match kind {
            Kind::Text => self.text,
            Kind::Mark | Kind::LinkUrl | Kind::HtmlComment => self.mark,
            Kind::Heading(1) => self.heading1,
            Kind::Heading(2) => self.heading2,
            Kind::Heading(_) => self.heading,
            Kind::Bold => self.bold,
            Kind::Italic => self.italic,
            Kind::CodeInline | Kind::CodeBlock | Kind::HtmlString => self.code,
            Kind::CheckboxOpen => self.checkbox,
            Kind::DoneText => self.done,
            Kind::Quote => self.quote,
            Kind::LinkText => self.link,
            Kind::Bullet => self.bullet,
            Kind::HtmlTag => self.html_tag,
            Kind::HtmlAttr | Kind::FmKey => self.html_attr,
        }
    }
}

/// One of the shipped palettes (`BUILTINS`). Anything else is a
/// candidate for `themes/<name>` on disk.
fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}

/// One color-value token: a named color, `bright-<named>`, `#rrggbb`,
/// or `default` (== `Color::Reset` when used as one part among others;
/// the whole-value `default` special case is handled by `parse_style`).
/// `token` has already been lowercased; a token that ends up with
/// leftover whitespace (from `cyan + bold`-style spacing around `+`)
/// simply matches nothing here and is reported as unknown by the caller.
fn parse_color_token(token: &str) -> Option<Color> {
    if token == "default" {
        return Some(Color::Reset);
    }
    if let Some(hex) = token.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(name) = token.strip_prefix("bright-") {
        return bright_named_color(name);
    }
    named_color(token)
}

fn named_color(name: &str) -> Option<Color> {
    match name {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        _ => None,
    }
}

/// `bright-<named>` maps to ratatui's `Light*` variants where one
/// exists. `bright-black` is ratatui's `DarkGray` (its own doc comment
/// calls that "bright black"). There's no `LightWhite`/`LightGray` in
/// ratatui, so `bright-white`/`bright-gray`/`bright-grey` are unknown
/// tokens (a warning), not silently downgraded to plain white/gray.
fn bright_named_color(name: &str) -> Option<Color> {
    match name {
        "black" => Some(Color::DarkGray),
        "red" => Some(Color::LightRed),
        "green" => Some(Color::LightGreen),
        "yellow" => Some(Color::LightYellow),
        "blue" => Some(Color::LightBlue),
        "magenta" => Some(Color::LightMagenta),
        "cyan" => Some(Color::LightCyan),
        _ => None,
    }
}

/// Exactly 6 hex digits, case-insensitive (`token` is already
/// lowercased by the caller). No `#rgb` shorthand expansion.
fn parse_hex(hex: &str) -> Option<Color> {
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

fn parse_modifier(token: &str) -> Option<Modifier> {
    match token {
        "reverse" | "reversed" => Some(Modifier::REVERSED),
        "dim" => Some(Modifier::DIM),
        "bold" => Some(Modifier::BOLD),
        "italic" => Some(Modifier::ITALIC),
        "underline" | "underlined" => Some(Modifier::UNDERLINED),
        _ => None,
    }
}

/// One side of a `style` value (the fg side, or the bg side after
/// `on`): `part ( '+' part )*`, each part a color or a modifier, at
/// most one color per side.
fn parse_side(side: &str, is_fg: bool) -> Result<Style, String> {
    let mut style = Style::default();
    let mut color_set = false;
    for token in side.split('+') {
        let lower = token.to_ascii_lowercase();
        if let Some(color) = parse_color_token(&lower) {
            if color_set {
                return Err(format!("two colors on one side: {side:?}"));
            }
            style = if is_fg {
                style.fg(color)
            } else {
                style.bg(color)
            };
            color_set = true;
        } else if let Some(modifier) = parse_modifier(&lower) {
            style = style.add_modifier(modifier);
        } else {
            return Err(format!("unknown token: {token:?}"));
        }
    }
    Ok(style)
}

/// Parse one `style` value per the color-value grammar:
///
/// ```text
/// style  := part ( '+' part )* [ ' on ' part ( '+' part )* ]
/// part   := color | modifier
/// color  := named | bright-named | '#' hex6 | 'default'
/// named  := black | red | green | yellow | blue | magenta | cyan
///         | white | gray | grey
/// modifier := reverse | reversed | dim | bold | italic
///           | underline | underlined
/// ```
///
/// The whole value being exactly `default` (case-insensitive) is
/// `Style::default()`; `default` as one part among others is
/// `Color::Reset` on that side. No spaces except around the literal
/// `on` — `cyan + bold` is a warning, not `cyan+bold` with padding.
pub fn parse_style(value: &str) -> Result<Style, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("empty value".to_string());
    }
    if value.eq_ignore_ascii_case("default") {
        return Ok(Style::default());
    }
    match value.split_once(" on ") {
        Some((fg, bg)) => {
            if fg.is_empty() || bg.is_empty() {
                return Err(format!("empty side in {value:?}"));
            }
            let fg_style = parse_side(fg, true)?;
            let bg_style = parse_side(bg, false)?;
            Ok(fg_style.patch(bg_style))
        }
        None => parse_side(value, true),
    }
}

/// Apply an overlay file's `key = value` lines onto `theme` in place.
/// Same never-fail contract as `config::parse`: bad lines (unknown
/// key, unparsable value) are skipped with a warning; the rest still
/// apply. `name` is not a settable slot — it identifies the theme, it
/// isn't part of it.
pub fn parse_overlay(text: &str, theme: &mut Theme) -> Vec<String> {
    let mut warnings = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let n = i + 1;
        let Some((key, value)) = line.split_once('=') else {
            warnings.push(format!("line {n}: expected `key = value`"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key == "name" {
            warnings.push(format!("line {n}: name is not settable from a theme file"));
            continue;
        }
        let style = match parse_style(value) {
            Ok(style) => style,
            Err(reason) => {
                warnings.push(format!("line {n}: {key}: {reason}"));
                continue;
            }
        };
        let slot = match key {
            "border_focused" => &mut theme.border_focused,
            "border_unfocused" => &mut theme.border_unfocused,
            "popup_border" => &mut theme.popup_border,
            "status_bar" => &mut theme.status_bar,
            "selection" => &mut theme.selection,
            "prompt_cursor" => &mut theme.prompt_cursor,
            "welcome" => &mut theme.welcome,
            "tree_open" => &mut theme.tree_open,
            "tab_active" => &mut theme.tab_active,
            "tab_inactive" => &mut theme.tab_inactive,
            "text" => &mut theme.text,
            "mark" => &mut theme.mark,
            "heading1" => &mut theme.heading1,
            "heading2" => &mut theme.heading2,
            "heading" => &mut theme.heading,
            "bold" => &mut theme.bold,
            "italic" => &mut theme.italic,
            "code" => &mut theme.code,
            "checkbox" => &mut theme.checkbox,
            "done" => &mut theme.done,
            "quote" => &mut theme.quote,
            "link" => &mut theme.link,
            "bullet" => &mut theme.bullet,
            "html_tag" => &mut theme.html_tag,
            "html_attr" => &mut theme.html_attr,
            "search_match" => &mut theme.search_match,
            _ => {
                warnings.push(format!("line {n}: unknown option: {key}"));
                continue;
            }
        };
        *slot = style;
    }
    warnings
}

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

/// Load the theme named `name` from `dir` (normally
/// `$XDG_CONFIG_HOME/mrkdup`), applying the load order:
///
/// 1. `Theme::named(name)` — one of the `BUILTINS`, or
///    `Theme::default()` if `name` isn't a builtin.
/// 2. If `name` isn't a builtin, `dir/themes/<name>` is read as an
///    overlay on top of `default`. A missing file is a warning
///    (unknown theme name), not a hard failure — the theme stays
///    `default`.
/// 3. If `dir/theme` exists, it's applied last, on top of whatever
///    came out of steps 1-2.
///
/// `name` is assumed syntactically valid (`config::parse` already
/// rejected anything else at config-load time).
pub fn load_from(name: &str, dir: &Path) -> (Theme, Vec<String>) {
    let mut theme = Theme::named(name);
    let mut warnings = Vec::new();

    if !is_builtin(name) {
        let path = dir.join("themes").join(name);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                warnings.extend(
                    parse_overlay(&text, &mut theme)
                        .into_iter()
                        .map(|w| format!("themes/{name}: {w}")),
                );
            }
            Err(_) => {
                warnings.push(format!(
                    "themes/{name}: unknown theme name (no such file); using default"
                ));
            }
        }
    }

    if let Ok(text) = std::fs::read_to_string(dir.join("theme")) {
        warnings.extend(
            parse_overlay(&text, &mut theme)
                .into_iter()
                .map(|w| format!("theme: {w}")),
        );
    }

    (theme, warnings)
}

/// `load_from` rooted at `$XDG_CONFIG_HOME/mrkdup` (falling back to
/// `~/.config/mrkdup`), same as `config::load`. No config directory at
/// all just means the named builtin (or `default`), no warnings.
pub fn load(name: &str) -> (Theme, Vec<String>) {
    match crate::config::config_dir() {
        Some(dir) => load_from(name, &dir),
        None => (Theme::named(name), Vec::new()),
    }
}

#[cfg(test)]
mod tests;
