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
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn short_line_is_one_row() {
        let rows = layout(&lines(&["hello"]), 10);
        assert_eq!(
            rows,
            vec![VisualRow {
                line: 0,
                start: 0,
                end: 5
            }]
        );
    }

    #[test]
    fn empty_line_is_one_empty_row() {
        let rows = layout(&lines(&[""]), 10);
        assert_eq!(
            rows,
            vec![VisualRow {
                line: 0,
                start: 0,
                end: 0
            }]
        );
    }

    #[test]
    fn exact_width_line_does_not_wrap() {
        let rows = layout(&lines(&["abcde"]), 5);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn long_word_breaks_at_width() {
        let rows = layout(&lines(&["abcdefgh"]), 5);
        assert_eq!(
            rows,
            vec![
                VisualRow {
                    line: 0,
                    start: 0,
                    end: 5
                },
                VisualRow {
                    line: 0,
                    start: 5,
                    end: 8
                },
            ]
        );
    }

    #[test]
    fn wraps_after_last_space_when_possible() {
        // "hello world" width 8 -> "hello " / "world"
        let rows = layout(&lines(&["hello world"]), 8);
        assert_eq!(
            rows,
            vec![
                VisualRow {
                    line: 0,
                    start: 0,
                    end: 6
                },
                VisualRow {
                    line: 0,
                    start: 6,
                    end: 11
                },
            ]
        );
    }

    #[test]
    fn wide_chars_count_two_cells() {
        // 你=2 cells; width 4 fits two chars per row
        let rows = layout(&lines(&["你好世界"]), 4);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            VisualRow {
                line: 0,
                start: 0,
                end: 2
            }
        );
    }

    #[test]
    fn char_wider_than_width_still_makes_progress() {
        let rows = layout(&lines(&["你你"]), 1);
        assert_eq!(rows.len(), 2); // one over-wide char per row; no infinite loop
    }

    #[test]
    fn multiple_lines_stack() {
        let rows = layout(&lines(&["ab", "", "cd"]), 10);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[1],
            VisualRow {
                line: 1,
                start: 0,
                end: 0
            }
        );
        assert_eq!(rows[2].line, 2);
    }

    #[test]
    fn cursor_in_first_row() {
        let ls = lines(&["hello world"]);
        let rows = layout(&ls, 8);
        assert_eq!(cursor_position(&rows, &ls, (0, 2)), (0, 2));
    }

    #[test]
    fn cursor_at_wrap_boundary_goes_to_next_row_start() {
        let ls = lines(&["hello world"]); // rows: [0,6) [6,11)
        let rows = layout(&ls, 8);
        assert_eq!(cursor_position(&rows, &ls, (0, 6)), (1, 0));
    }

    #[test]
    fn cursor_at_line_end_stays_on_last_row() {
        let ls = lines(&["hello world"]);
        let rows = layout(&ls, 8);
        assert_eq!(cursor_position(&rows, &ls, (0, 11)), (1, 5));
    }

    #[test]
    fn cursor_x_accounts_for_wide_chars() {
        let ls = lines(&["你好"]);
        let rows = layout(&ls, 10);
        assert_eq!(cursor_position(&rows, &ls, (0, 1)), (0, 2));
    }

    #[test]
    fn cursor_on_second_line() {
        let ls = lines(&["ab", "cd"]);
        let rows = layout(&ls, 10);
        assert_eq!(cursor_position(&rows, &ls, (1, 1)), (1, 1));
    }

    #[test]
    fn scroll_keeps_cursor_visible() {
        assert_eq!(scroll_top(0, 5, 10), 0); // already visible
        assert_eq!(scroll_top(0, 12, 10), 3); // cursor below -> scroll down
        assert_eq!(scroll_top(8, 3, 10), 3); // cursor above -> scroll up
        assert_eq!(scroll_top(3, 3, 10), 3); // at top edge -> unchanged
    }
}
