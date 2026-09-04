//! The terminal cursor's look: shape, blink, and color, from the
//! `cursor_*` config keys. The editor uses the terminal's own cursor
//! (ratatui's `set_cursor_position`), so its look is whatever the
//! terminal profile says unless the app asks for something else. Two
//! escape sequences do that, in every modern emulator (Windows Terminal,
//! iTerm2, kitty, WezTerm, Ghostty, Alacritty, foot, xterm):
//!
//! - DECSCUSR `ESC [ n SP q`: shape + blink in one number; `0` is the
//!   terminal's own default.
//! - OSC 12 `ESC ] 12 ; #rrggbb BEL` sets the cursor color and OSC 112
//!   resets it to the terminal's default.
//!
//! Everything here is pure except `apply`/`reset`, so the sequences are
//! testable; `main.rs` re-applies them whenever the settings change and
//! resets both on exit so the shell gets its cursor back.

use std::io::{self, Write};

/// `cursor_shape`: `default` leaves the terminal's own shape (and
/// ignores `cursor_blink`, which DECSCUSR can't set on its own).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Default,
    Block,
    Bar,
    Underline,
}

impl Shape {
    /// In settings-popup order.
    pub const ALL: [Shape; 4] = [Shape::Default, Shape::Block, Shape::Bar, Shape::Underline];

    pub fn parse(value: &str) -> Option<Shape> {
        match value {
            "default" => Some(Shape::Default),
            "block" => Some(Shape::Block),
            "bar" => Some(Shape::Bar),
            "underline" => Some(Shape::Underline),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Shape::Default => "default",
            Shape::Block => "block",
            Shape::Bar => "bar",
            Shape::Underline => "underline",
        }
    }
}

/// `cursor_color` names offered by the settings popup, in order. The
/// config file also takes any `#rrggbb`.
pub const COLOR_NAMES: [&str; 11] = [
    "default", "white", "black", "gray", "red", "orange", "yellow", "green", "cyan", "blue",
    "magenta",
];

/// `#rrggbb` for a named color or a hex value, `None` for anything
/// else. `default` is not a color here: callers check it first.
/// Names use the bright xterm values so the cursor stands out; a
/// user who wants an exact shade writes the hex.
pub fn color_hex(value: &str) -> Option<String> {
    let hex = match value {
        "white" => "#ffffff",
        "black" => "#000000",
        "gray" | "grey" => "#808080",
        "red" => "#ff0000",
        "orange" => "#ffa500",
        "yellow" => "#ffff00",
        "green" => "#00ff00",
        "cyan" => "#00ffff",
        "blue" => "#5c5cff",
        "magenta" => "#ff00ff",
        _ => {
            let digits = value.strip_prefix('#')?;
            if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            return Some(format!("#{}", digits.to_ascii_lowercase()));
        }
    };
    Some(hex.to_string())
}

/// `default` or something `color_hex` accepts.
pub fn valid_color(value: &str) -> bool {
    value == "default" || color_hex(value).is_some()
}

/// The resolved look, compared frame to frame by `main.rs` so the
/// escapes are only re-sent when a setting changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub shape: Shape,
    pub blink: bool,
    /// `#rrggbb`, or `None` for the terminal's default color.
    pub color: Option<String>,
}

impl Cursor {
    pub fn from_config(cfg: &crate::config::Config) -> Cursor {
        Cursor {
            shape: cfg.cursor_shape,
            blink: cfg.cursor_blink,
            color: if cfg.cursor_color == "default" {
                None
            } else {
                color_hex(&cfg.cursor_color)
            },
        }
    }

    /// DECSCUSR's parameter: 0 default, 1/2 block, 3/4 underline,
    /// 5/6 bar; odd blinks, even is steady.
    fn decscusr(&self) -> u8 {
        let base = match self.shape {
            Shape::Default => return 0,
            Shape::Block => 1,
            Shape::Underline => 3,
            Shape::Bar => 5,
        };
        if self.blink {
            base
        } else {
            base + 1
        }
    }

    /// Both escapes: shape, then color (or the color reset, so switching
    /// back to `default` in the popup takes effect).
    pub fn sequence(&self) -> String {
        let mut s = format!("\x1b[{} q", self.decscusr());
        match &self.color {
            Some(hex) => s.push_str(&format!("\x1b]12;{hex}\x07")),
            None => s.push_str("\x1b]112\x07"),
        }
        s
    }
}

/// Everything back to the terminal's own defaults, for exit.
pub const RESET: &str = "\x1b[0 q\x1b]112\x07";

/// Write `cursor`'s escapes through `out` and flush.
pub fn apply<W: Write>(out: &mut W, cursor: &Cursor) -> io::Result<()> {
    out.write_all(cursor.sequence().as_bytes())?;
    out.flush()
}

/// Write `RESET` through `out` and flush.
pub fn reset<W: Write>(out: &mut W) -> io::Result<()> {
    out.write_all(RESET.as_bytes())?;
    out.flush()
}

#[cfg(test)]
mod tests;
