//! One open document and the per-document state that used to live on
//! `App` when there was only ever one: the editor itself, its scroll,
//! the idle-autosave timer, the disk-conflict flag, and the search
//! highlight. Plus the pure tab-bar layout the editor pane draws and
//! the mouse hit-tests against.

use std::time::Instant;

use unicode_width::UnicodeWidthStr;

use crate::editor::Editor;
use crate::wrap;

pub struct Tab {
    pub editor: Editor,
    /// vertical scroll of the editor renderer, in visual rows
    pub scroll: usize,
    /// `false` after a wheel scroll: the view stays where it was put,
    /// even with the cursor off screen, until the next key or click
    pub follow_cursor: bool,
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
            follow_cursor: true,
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

/// What a painted piece of the tab bar is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// the tab's name; a click switches to it
    Title,
    /// the `×`; a click closes it
    Close,
    /// the `‹` at the left edge when tabs are hidden that way; a click
    /// goes one tab left
    Prev,
    /// the `›` at the right edge; a click goes one tab right
    Next,
}

/// One painted piece of the tab bar: the cells `[x0, x1)` (relative to
/// the bar's left edge) showing `text`, belonging to tab `index` (for
/// `Prev`/`Next`, the tab a click would go to).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub x0: u16,
    pub x1: u16,
    pub text: String,
    pub index: usize,
    pub active: bool,
    pub part: Part,
}

impl Segment {
    pub fn hit(&self, x: u16) -> bool {
        x >= self.x0 && x < self.x1
    }
}

/// The close glyph and its trailing space, as painted.
const CLOSE: &str = "× ";
/// Longer names are cut with `…` so one tab can't hog the bar.
pub const MAX_TITLE: usize = 20;

/// Lay `titles` out left to right in `width` cells as ` title × ` runs.
/// When they don't all fit, one cell at each edge shows `‹` / `›` for
/// the tabs hidden that way (blank when nothing is), and the run is
/// scrolled so the active tab is fully visible (tabs before it drop off
/// the left first); the last tab that doesn't fit is cut at the edge.
pub fn layout_bar(titles: &[String], active: usize, width: u16) -> Vec<Segment> {
    let n = titles.len();
    let width = width as usize;
    let mut out = Vec::new();
    if n == 0 || width == 0 {
        return out;
    }
    let labels: Vec<String> = titles
        .iter()
        .map(|t| format!(" {} ", wrap::ellipsize(t, MAX_TITLE)))
        .collect();
    let tab_w = |i: usize| labels[i].width() + CLOSE.width();
    let total: usize = (0..n).map(tab_w).sum();
    let overflow = total > width;
    let edges = if overflow && width >= 3 { 1 } else { 0 };
    let avail = width - 2 * edges;
    let mut start = 0;
    while start < active && (start..=active).map(tab_w).sum::<usize>() > avail {
        start += 1;
    }
    let mut x = edges;
    if edges == 1 {
        out.push(Segment {
            x0: 0,
            x1: 1,
            text: if start > 0 { "‹" } else { " " }.into(),
            index: active.saturating_sub(1),
            active: false,
            part: Part::Prev,
        });
    }
    let end_x = x + avail;
    let mut last_shown_fully = None;
    for (i, label) in labels.iter().enumerate().skip(start) {
        if x >= end_x {
            break;
        }
        let text = wrap::clip(label, end_x - x);
        let w = text.width();
        out.push(Segment {
            x0: x as u16,
            x1: (x + w) as u16,
            text,
            index: i,
            active: i == active,
            part: Part::Title,
        });
        x += w;
        if x >= end_x {
            break;
        }
        let glyph = wrap::clip(CLOSE, end_x - x);
        let w = glyph.width();
        let whole = glyph == CLOSE;
        out.push(Segment {
            x0: x as u16,
            x1: (x + w) as u16,
            text: glyph,
            index: i,
            active: i == active,
            part: Part::Close,
        });
        x += w;
        if whole {
            last_shown_fully = Some(i);
        }
    }
    if edges == 1 {
        let more_right = last_shown_fully.is_none_or(|i| i + 1 < n);
        out.push(Segment {
            x0: end_x as u16,
            x1: (end_x + 1) as u16,
            text: if more_right { "›" } else { " " }.into(),
            index: (active + 1).min(n - 1),
            active: false,
            part: Part::Next,
        });
    }
    out
}

#[cfg(test)]
mod tests;
