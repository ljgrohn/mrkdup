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
