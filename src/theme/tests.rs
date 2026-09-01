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
