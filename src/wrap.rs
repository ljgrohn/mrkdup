//! Pure soft-wrap layout math for the editor renderer: splits logical
//! lines into display rows, maps the logical cursor to screen space,
//! and keeps the cursor visible when scrolling.

use unicode_width::UnicodeWidthChar;

/// One display row: chars `[start, end)` of logical line `line`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualRow {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

pub fn ch_width(c: char) -> usize {
    if c == '\t' {
        4
    } else {
        UnicodeWidthChar::width(c).unwrap_or(0)
    }
}

/// Word-aware wrap: break at the last space that fits, fall back to a
/// hard break for words wider than the pane. Every line yields at least
/// one row (empty lines yield `(0, 0)`).
pub fn layout(lines: &[String], width: usize) -> Vec<VisualRow> {
    let width = width.max(1);
    let mut rows = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            rows.push(VisualRow {
                line: li,
                start: 0,
                end: 0,
            });
            continue;
        }
        let mut start = 0;
        while start < chars.len() {
            let mut col = 0;
            let mut end = start;
            let mut last_space = None;
            while end < chars.len() {
                let w = ch_width(chars[end]);
                if col + w > width && end > start {
                    break;
                }
                if chars[end] == ' ' {
                    last_space = Some(end);
                }
                col += w;
                end += 1;
            }
            if end < chars.len() {
                // mid-line break: prefer just after the last space in this row
                if let Some(sp) = last_space {
                    if sp + 1 > start {
                        end = sp + 1;
                    }
                }
            }
            rows.push(VisualRow {
                line: li,
                start,
                end,
            });
            start = end;
        }
    }
    rows
}

/// Map a logical (line, char col) cursor to (visual row index, x cells).
/// A cursor at a row's `end` belongs to the NEXT row of the same line,
/// except at the very end of the line.
pub fn cursor_position(
    rows: &[VisualRow],
    lines: &[String],
    cursor: (usize, usize),
) -> (usize, usize) {
    let (cline, ccol) = cursor;
    let mut result = 0;
    for (ri, row) in rows.iter().enumerate() {
        if row.line != cline {
            continue;
        }
        result = ri;
        let is_last_row_of_line = rows.get(ri + 1).is_none_or(|next| next.line != cline);
        if ccol < row.end || (is_last_row_of_line && ccol >= row.end) {
            let x = lines[cline]
                .chars()
                .skip(row.start)
                .take(ccol.saturating_sub(row.start))
                .map(ch_width)
                .sum();
            return (ri, x);
        }
    }
    (result, 0)
}

/// Map a screen hit — visual row index `vrow` and `x` cells from the
/// pane's left edge — back to a logical (line, char col). The inverse of
/// `cursor_position`, used to land the cursor on a mouse click.
///
/// `vrow` past the last row snaps to the last row. `x` past the row's
/// text snaps to the row's end — or, on a row that wraps into another,
/// one before it, so the cursor stays on the clicked row instead of
/// rendering at the start of the next one (`cursor_position` puts a
/// cursor at `end` on the following row). A row that's a single char
/// wide can't do that and lands on its start instead.
pub fn hit_test(rows: &[VisualRow], lines: &[String], vrow: usize, x: usize) -> (usize, usize) {
    let ri = vrow.min(rows.len().saturating_sub(1));
    let Some(row) = rows.get(ri) else {
        return (0, 0);
    };
    let is_last_row_of_line = rows.get(ri + 1).is_none_or(|next| next.line != row.line);
    let mut cells = 0;
    for (ci, ch) in lines[row.line]
        .chars()
        .enumerate()
        .take(row.end)
        .skip(row.start)
    {
        let w = ch_width(ch).max(1);
        if x < cells + w {
            return (row.line, ci);
        }
        cells += w;
    }
    let col = if is_last_row_of_line {
        row.end
    } else {
        row.end.saturating_sub(1).max(row.start)
    };
    (row.line, col)
}

/// The longest prefix of `s` at most `cells` wide.
pub fn clip(s: &str, cells: usize) -> String {
    let mut out = String::new();
    let mut w = 0;
    for ch in s.chars() {
        let cw = ch_width(ch);
        if w + cw > cells {
            break;
        }
        w += cw;
        out.push(ch);
    }
    out
}

/// `s` if it fits in `cells`, else its prefix plus `…` to fit. A budget
/// of 0 yields the empty string, 1 yields `…` alone.
pub fn ellipsize(s: &str, cells: usize) -> String {
    let width: usize = s.chars().map(ch_width).sum();
    if width <= cells {
        return s.to_string();
    }
    if cells == 0 {
        return String::new();
    }
    let mut out = clip(s, cells - 1);
    out.push('…');
    out
}

/// Adjust vertical scroll (in visual rows) so `cursor_row` is visible.
pub fn scroll_top(current_top: usize, cursor_row: usize, height: usize) -> usize {
    let height = height.max(1);
    if cursor_row < current_top {
        cursor_row
    } else if cursor_row >= current_top + height {
        cursor_row + 1 - height
    } else {
        current_top
    }
}

#[cfg(test)]
mod tests;
