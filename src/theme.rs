//! Color theme: every `Color::`/modifier literal that used to be
//! scattered across `highlight.rs`, `render.rs`, and `ui.rs` lives here,
//! in one struct. `Theme::default()` is pixel-identical to the old
//! hardcoded look — this module is purely a composability refactor.

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

    /// The builtin named `name`, or `Theme::default()` if `name` isn't
    /// one of the builtins. File-based lookup is a later task.
    pub fn named(name: &str) -> Theme {
        match name {
            "light" => Theme::light(),
            "mono" => Theme::mono(),
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

#[cfg(test)]
mod tests;
