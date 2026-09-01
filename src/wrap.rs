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
