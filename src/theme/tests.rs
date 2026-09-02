use super::*;
use crate::highlight::Kind;
use ratatui::style::{Color, Modifier, Style};

/// The `highlight::style` match arms, frozen here as the oracle for
/// `Theme::default()`. Copied verbatim before `highlight::style` is
/// deleted by this task — this is the ONLY place outside `theme.rs`
/// that `Color::` may appear.
fn legacy_style(kind: Kind) -> Style {
    match kind {
        Kind::Text => Style::default(),
        Kind::Mark | Kind::LinkUrl | Kind::HtmlComment => {
            Style::default().add_modifier(Modifier::DIM)
        }
        Kind::Heading(1) => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        Kind::Heading(2) => Style::default().fg(Color::Cyan),
        Kind::Heading(_) => Style::default().fg(Color::Blue),
        Kind::Bold => Style::default().add_modifier(Modifier::BOLD),
        Kind::Italic => Style::default().add_modifier(Modifier::ITALIC),
        Kind::CodeInline | Kind::CodeBlock | Kind::HtmlString => Style::default().fg(Color::Green),
        Kind::CheckboxOpen => Style::default().fg(Color::Magenta),
        Kind::DoneText => Style::default().add_modifier(Modifier::DIM),
        Kind::Quote => Style::default().fg(Color::Yellow),
        Kind::LinkText => Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED),
        Kind::Bullet => Style::default().fg(Color::Cyan),
        Kind::HtmlTag => Style::default().fg(Color::Magenta),
        Kind::HtmlAttr | Kind::FmKey => Style::default().fg(Color::Cyan),
        // code kinds postdate the legacy table; these are their defaults
        Kind::Keyword => Style::default().fg(Color::Magenta),
        Kind::TypeName => Style::default().fg(Color::Yellow),
        Kind::Str => Style::default().fg(Color::Green),
        Kind::Comment => Style::default().add_modifier(Modifier::DIM),
        Kind::Number => Style::default().fg(Color::Cyan),
        Kind::Macro => Style::default().fg(Color::Blue),
    }
}

/// Every `Kind` variant, including a representative sample of each
/// `Heading` level (1, 2, and 3+ collapse to the same match arm).
const ALL_KINDS: &[Kind] = &[
    Kind::Text,
    Kind::Mark,
    Kind::Heading(1),
    Kind::Heading(2),
    Kind::Heading(3),
    Kind::Heading(6),
    Kind::Bold,
    Kind::Italic,
    Kind::CodeInline,
    Kind::CodeBlock,
    Kind::CheckboxOpen,
    Kind::DoneText,
    Kind::Quote,
    Kind::LinkText,
    Kind::LinkUrl,
    Kind::Bullet,
    Kind::HtmlTag,
    Kind::HtmlAttr,
    Kind::HtmlString,
    Kind::HtmlComment,
    Kind::FmKey,
    Kind::Keyword,
    Kind::TypeName,
    Kind::Str,
    Kind::Comment,
    Kind::Number,
    Kind::Macro,
];

#[test]
fn default_matches_legacy_highlight_style() {
    let theme = Theme::default();
    for &kind in ALL_KINDS {
        assert_eq!(
            theme.syntax(kind),
            legacy_style(kind),
            "Theme::default().syntax({kind:?}) diverged from the legacy highlight::style match"
        );
    }
}

#[test]
fn default_theme_is_named_default() {
    assert_eq!(Theme::default().name, "default");
}

#[test]
fn named_looks_up_builtins_and_falls_back_to_default() {
    assert_eq!(Theme::named("default"), Theme::default());
    assert_eq!(Theme::named("light"), Theme::light());
    assert_eq!(Theme::named("mono"), Theme::mono());
    assert_eq!(Theme::named("nonexistent"), Theme::default());
}

#[test]
fn light_theme_uses_blue_for_headings_and_red_for_quote() {
    let theme = Theme::light();
    assert_eq!(theme.name, "light");
    assert_eq!(
        theme.heading1,
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(theme.heading2, Style::default().fg(Color::Blue));
    assert_eq!(theme.heading, Style::default().fg(Color::Magenta));
    assert_eq!(theme.quote, Style::default().fg(Color::Red));
    assert_eq!(theme.bullet, Style::default().fg(Color::Blue));
    assert_eq!(theme.html_attr, Style::default().fg(Color::Blue));
    assert_eq!(theme.border_focused, Style::default().fg(Color::Blue));
    // search_match stays Black on Yellow, same as default
    assert_eq!(
        theme.search_match,
        Style::default().fg(Color::Black).bg(Color::Yellow)
    );
}

#[test]
fn mono_theme_has_no_color_anywhere() {
    let theme = Theme::mono();
    assert_eq!(theme.name, "mono");
    let styles = [
        theme.border_focused,
        theme.border_unfocused,
        theme.popup_border,
        theme.status_bar,
        theme.selection,
        theme.prompt_cursor,
        theme.welcome,
        theme.tree_open,
        theme.text,
        theme.mark,
        theme.heading1,
        theme.heading2,
        theme.heading,
        theme.bold,
        theme.italic,
        theme.code,
        theme.checkbox,
        theme.done,
        theme.quote,
        theme.link,
        theme.bullet,
        theme.html_tag,
        theme.html_attr,
        theme.search_match,
        theme.keyword,
        theme.type_name,
        theme.string,
        theme.comment,
        theme.number,
        theme.macro_call,
    ];
    for style in styles {
        assert_eq!(
            style.fg, None,
            "mono style has a foreground color: {style:?}"
        );
        assert_eq!(
            style.bg, None,
            "mono style has a background color: {style:?}"
        );
    }
}

#[test]
fn mono_theme_reverses_selection_status_and_search() {
    let theme = Theme::mono();
    let reversed = Style::default().add_modifier(Modifier::REVERSED);
    assert_eq!(theme.status_bar, reversed);
    assert_eq!(theme.selection, reversed);
    assert_eq!(theme.prompt_cursor, reversed);
    assert_eq!(theme.tree_open, reversed);
    assert_eq!(theme.search_match, reversed);
}

// ---------------------------------------------------------------------
// parse_style: the color-value grammar table from the plan.
// ---------------------------------------------------------------------

#[test]
fn parse_style_grammar_table() {
    assert_eq!(
        parse_style("cyan").unwrap(),
        Style::default().fg(Color::Cyan)
    );
    assert_eq!(
        parse_style("cyan+bold").unwrap(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(
        parse_style("black on yellow").unwrap(),
        Style::default().fg(Color::Black).bg(Color::Yellow)
    );
    assert_eq!(
        parse_style("white on blue").unwrap(),
        Style::default().fg(Color::White).bg(Color::Blue)
    );
    assert_eq!(
        parse_style("dim").unwrap(),
        Style::default().add_modifier(Modifier::DIM)
    );
    assert_eq!(
        parse_style("reverse").unwrap(),
        Style::default().add_modifier(Modifier::REVERSED)
    );
    assert_eq!(
        parse_style("reversed").unwrap(),
        Style::default().add_modifier(Modifier::REVERSED)
    );
    assert_eq!(
        parse_style("#89b4fa").unwrap(),
        Style::default().fg(Color::Rgb(0x89, 0xb4, 0xfa))
    );
    assert_eq!(
        parse_style("#cdd6f4 on #1e1e2e").unwrap(),
        Style::default()
            .fg(Color::Rgb(0xcd, 0xd6, 0xf4))
            .bg(Color::Rgb(0x1e, 0x1e, 0x2e))
    );
    assert_eq!(parse_style("default").unwrap(), Style::default());
}

#[test]
fn parse_style_default_on_a_side_is_color_reset() {
    assert_eq!(
        parse_style("default on blue").unwrap(),
        Style::default().fg(Color::Reset).bg(Color::Blue)
    );
}

#[test]
fn parse_style_is_case_insensitive() {
    assert_eq!(
        parse_style("CYAN").unwrap(),
        Style::default().fg(Color::Cyan)
    );
    assert_eq!(
        parse_style("Cyan+Bold").unwrap(),
        parse_style("cyan+bold").unwrap()
    );
    assert_eq!(
        parse_style("#89B4FA").unwrap(),
        parse_style("#89b4fa").unwrap()
    );
    assert_eq!(parse_style("DEFAULT").unwrap(), Style::default());
    assert_eq!(
        parse_style("Black on Yellow").unwrap(),
        parse_style("black on yellow").unwrap()
    );
}

#[test]
fn parse_style_gray_and_grey_are_color_gray() {
    assert_eq!(
        parse_style("gray").unwrap(),
        Style::default().fg(Color::Gray)
    );
    assert_eq!(
        parse_style("grey").unwrap(),
        Style::default().fg(Color::Gray)
    );
}

#[test]
fn parse_style_bright_prefix_maps_to_light_variant() {
    assert_eq!(
        parse_style("bright-cyan").unwrap(),
        Style::default().fg(Color::LightCyan)
    );
    assert_eq!(
        parse_style("bright-red").unwrap(),
        Style::default().fg(Color::LightRed)
    );
}

#[test]
fn parse_style_two_colors_on_one_side_without_on_is_a_warning() {
    assert!(parse_style("red+blue").is_err());
}

#[test]
fn parse_style_hex_must_be_exactly_six_digits() {
    assert!(parse_style("#fff").is_err());
    assert!(parse_style("#ffffff").is_ok());
    assert!(parse_style("#fffffff").is_err());
}

#[test]
fn parse_style_invalid_hex_digits_is_a_warning() {
    assert!(parse_style("#gg0000").is_err());
}

#[test]
fn parse_style_no_color_indexed_escape_hatch() {
    // "196" is not a recognized color token at all (no bare numbers).
    assert!(parse_style("196").is_err());
}

#[test]
fn parse_style_spaces_around_plus_is_a_warning() {
    assert!(parse_style("cyan + bold").is_err());
}

#[test]
fn parse_style_unknown_token_is_a_warning() {
    assert!(parse_style("chartreuse").is_err());
}

// ---------------------------------------------------------------------
// parse_overlay: pure, no filesystem.
// ---------------------------------------------------------------------

#[test]
fn parse_overlay_sets_only_the_named_slot() {
    let mut theme = Theme::default();
    let warnings = parse_overlay("heading1 = red+bold", &mut theme);
    assert!(warnings.is_empty());
    assert_eq!(
        theme.heading1,
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    );
    // everything else stays at the default theme's values
    let expected = Theme {
        heading1: theme.heading1,
        ..Theme::default()
    };
    assert_eq!(theme, expected);
}

#[test]
fn parse_overlay_search_match_round_trips() {
    let mut theme = Theme::default();
    let before = theme.search_match;
    let warnings = parse_overlay("search_match = black on yellow", &mut theme);
    assert!(warnings.is_empty());
    assert_eq!(theme.search_match, before);
}

#[test]
fn parse_overlay_unknown_key_warns_and_leaves_theme_unchanged() {
    let mut theme = Theme::default();
    let before = theme.clone();
    let warnings = parse_overlay("nope = cyan", &mut theme);
    assert_eq!(warnings.len(), 1);
    assert_eq!(theme, before);
}

#[test]
fn parse_overlay_spaces_around_plus_warns() {
    let mut theme = Theme::default();
    let before = theme.clone();
    let warnings = parse_overlay("heading1 = cyan + bold", &mut theme);
    assert_eq!(warnings.len(), 1);
    assert_eq!(theme, before);
}

#[test]
fn parse_overlay_bad_hex_warns() {
    let mut theme = Theme::default();
    let before = theme.clone();
    let warnings = parse_overlay("heading1 = #gg0000", &mut theme);
    assert_eq!(warnings.len(), 1);
    assert_eq!(theme, before);
}

#[test]
fn parse_overlay_hex_sets_rgb_fg() {
    let mut theme = Theme::default();
    let warnings = parse_overlay("quote = #cc0000", &mut theme);
    assert!(warnings.is_empty());
    assert_eq!(
        theme.quote,
        Style::default().fg(Color::Rgb(0xcc, 0x00, 0x00))
    );
}

#[test]
fn parse_overlay_name_is_not_settable() {
    let mut theme = Theme::default();
    let before = theme.clone();
    let warnings = parse_overlay("name = evil", &mut theme);
    assert_eq!(warnings.len(), 1);
    assert_eq!(theme, before);
}

#[test]
fn parse_overlay_bad_lines_warn_and_the_rest_still_apply() {
    let mut theme = Theme::default();
    let warnings = parse_overlay("nope = cyan\nquote = red\n", &mut theme);
    assert_eq!(warnings.len(), 1);
    assert_eq!(theme.quote, Style::default().fg(Color::Red));
}

#[test]
fn parse_overlay_comments_and_blank_lines_are_skipped() {
    let mut theme = Theme::default();
    let warnings = parse_overlay("# a comment\n\n   \nquote = red\n", &mut theme);
    assert!(warnings.is_empty());
    assert_eq!(theme.quote, Style::default().fg(Color::Red));
}

// ---------------------------------------------------------------------
// load_from: filesystem, explicit dir (no env var).
// ---------------------------------------------------------------------

fn load_fixture(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mrkdup-theme-load-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("themes")).unwrap();
    std::fs::write(dir.join("theme"), "quote = red\n").unwrap();
    std::fs::write(dir.join("themes/forest"), "heading1 = green+bold\n").unwrap();
    dir
}

#[test]
fn load_from_default_applies_the_overlay_file() {
    let dir = load_fixture("default");
    let (theme, warnings) = load_from("default", &dir);
    assert!(warnings.is_empty());
    let expected = Theme {
        quote: Style::default().fg(Color::Red),
        ..Theme::default()
    };
    assert_eq!(theme, expected);
}

#[test]
fn load_from_light_applies_the_overlay_file_on_top_of_light() {
    let dir = load_fixture("light");
    let (theme, warnings) = load_from("light", &dir);
    assert!(warnings.is_empty());
    let expected = Theme {
        quote: Style::default().fg(Color::Red),
        ..Theme::light()
    };
    assert_eq!(theme, expected);
}

#[test]
fn load_from_named_file_then_overlay_file() {
    let dir = load_fixture("forest");
    let (theme, warnings) = load_from("forest", &dir);
    assert!(warnings.is_empty());
    let expected = Theme {
        heading1: Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        quote: Style::default().fg(Color::Red),
        ..Theme::default()
    };
    assert_eq!(theme, expected);
}

#[test]
fn load_from_missing_named_file_warns_once_and_falls_back_to_default() {
    let dir = load_fixture("missing");
    let (theme, warnings) = load_from("missing", &dir);
    assert_eq!(warnings.len(), 1);
    let expected = Theme {
        quote: Style::default().fg(Color::Red),
        ..Theme::default()
    };
    assert_eq!(theme, expected);
}

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
        assert!(
            crate::config::valid_theme_name(name),
            "{name} must be a valid theme name"
        );
    }
    assert_eq!(Theme::named("nope").name, "default");
}

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
    assert_eq!(
        list_user_themes(&dir),
        vec!["aurora".to_string(), "forest".to_string()]
    );
}

#[test]
fn list_user_themes_is_empty_without_a_themes_dir() {
    let dir = std::env::temp_dir().join("mrkdup-theme-list-missing");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(list_user_themes(&dir).is_empty());
}
