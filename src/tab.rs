//! One open document and the per-document state that used to live on
//! `App` when there was only ever one: the editor itself, its scroll,
//! the idle-autosave timer, the disk-conflict flag, and the search
//! highlight. Plus the pure tab-bar layout the editor pane draws and
//! the mouse hit-tests against.

use std::time::Instant;

use unicode_width::UnicodeWidthStr;

use crate::editor::Editor;

pub struct Tab {
    pub editor: Editor,
    /// vertical scroll of the editor renderer, in visual rows
    pub scroll: usize,
    pub last_edit: Option<Instant>,
    /// after a disk-conflict warning, the next Ctrl+S overwrites
    pub force_next_save: bool,
    /// the query whose matches the renderer highlights (cleared on the
    /// next edit)
    pub search_highlight: Option<String>,
}

impl Tab {
    pub fn new(editor: Editor) -> Tab {
        Tab {
            editor,
            scroll: 0,
            last_edit: None,
            force_next_save: false,
            search_highlight: None,
        }
    }

    /// The file name alone, for status messages.
    pub fn name(&self) -> String {
        self.editor
            .path
            .as_deref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "?".into())
    }

    /// The tab-bar label: the file name, `*` when dirty.
    pub fn title(&self) -> String {
        let name = self.name();
        if self.editor.dirty {
            format!("{name}*")
        } else {
            name
        }
    }
}

/// Which tab is active once tab `closed` is removed from a bar that had
/// `active` selected and now holds `len` tabs: the left neighbour of a
/// closed active tab (the new first tab when it was the first), the
/// same tab shifted down when one to its left closed, unchanged
/// otherwise.
pub fn after_close(active: usize, closed: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let next = if closed <= active {
        active.saturating_sub(1)
    } else {
        active
    };
    next.min(len - 1)
}

/// One painted piece of the tab bar: the cells `[x0, x1)` (relative to
/// the bar's left edge) showing `text`, belonging to tab `index`. A
/// `close` piece is the `×` that closes the tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub x0: u16,
    pub x1: u16,
    pub text: String,
    pub index: usize,
    pub active: bool,
    pub close: bool,
}

impl Segment {
    pub fn hit(&self, x: u16) -> bool {
        x >= self.x0 && x < self.x1
    }
}

/// The close glyph and its trailing space, as painted.
const CLOSE: &str = "× ";

/// Lay `titles` out left to right in `width` cells as ` title × ` runs,
/// scrolled so the active tab is fully visible (tabs before it drop off
/// the left first); the last tab that doesn't fit is cut at the edge.
pub fn layout_bar(titles: &[String], active: usize, width: u16) -> Vec<Segment> {
    let width = width as usize;
    let tab_w = |t: &String| t.width() + 2 + CLOSE.width();
    let mut start = 0;
    while start < active && titles[start..=active].iter().map(tab_w).sum::<usize>() > width {
        start += 1;
    }
    let mut out = Vec::new();
    let mut x = 0usize;
    for (i, title) in titles.iter().enumerate().skip(start) {
        if x >= width {
            break;
        }
        let label = format!(" {title} ");
        let label = clip(&label, width - x);
        let lw = label.width();
        out.push(Segment {
            x0: x as u16,
            x1: (x + lw) as u16,
            text: label,
            index: i,
            active: i == active,
            close: false,
        });
        x += lw;
        if x >= width {
            break;
        }
        let glyph = clip(CLOSE, width - x);
        let gw = glyph.width();
        out.push(Segment {
            x0: x as u16,
            x1: (x + gw) as u16,
            text: glyph,
            index: i,
            active: i == active,
            close: true,
        });
        x += gw;
    }
    out
}

/// The longest prefix of `s` at most `cells` wide.
fn clip(s: &str, cells: usize) -> String {
    let mut out = String::new();
    let mut w = 0;
    for ch in s.chars() {
        let cw = crate::wrap::ch_width(ch);
        if w + cw > cells {
            break;
        }
        w += cw;
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests;
