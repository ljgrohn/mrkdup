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

#[test]
fn hit_test_lands_on_the_clicked_char() {
    let ls = lines(&["hello world"]);
    let rows = layout(&ls, 20);
    assert_eq!(hit_test(&rows, &ls, 0, 0), (0, 0));
    assert_eq!(hit_test(&rows, &ls, 0, 4), (0, 4));
    assert_eq!(hit_test(&rows, &ls, 0, 10), (0, 10));
}

#[test]
fn hit_test_past_the_end_snaps_to_line_end() {
    let ls = lines(&["hello"]);
    let rows = layout(&ls, 20);
    assert_eq!(hit_test(&rows, &ls, 0, 17), (0, 5));
}

#[test]
fn hit_test_below_the_last_row_snaps_to_the_last_row() {
    let ls = lines(&["one", "two"]);
    let rows = layout(&ls, 20);
    assert_eq!(hit_test(&rows, &ls, 9, 1), (1, 1));
}

#[test]
fn hit_test_on_a_wrapped_row_stays_on_that_row() {
    // "aaaa bbbb" at width 5 wraps to "aaaa " | "bbbb"
    let ls = lines(&["aaaa bbbb"]);
    let rows = layout(&ls, 5);
    assert_eq!(rows.len(), 2);
    // past the end of the first row: col 4 (the space), not 5 which
    // cursor_position would draw at the start of the second row
    assert_eq!(hit_test(&rows, &ls, 0, 9), (0, 4));
    assert_eq!(cursor_position(&rows, &ls, (0, 4)).0, 0);
    // second row maps from its own start
    assert_eq!(hit_test(&rows, &ls, 1, 2), (0, 7));
    assert_eq!(hit_test(&rows, &ls, 1, 9), (0, 9));
}

#[test]
fn hit_test_counts_wide_chars_as_two_cells() {
    let ls = lines(&["日本x"]);
    let rows = layout(&ls, 20);
    assert_eq!(hit_test(&rows, &ls, 0, 1), (0, 0)); // second cell of 日
    assert_eq!(hit_test(&rows, &ls, 0, 2), (0, 1));
    assert_eq!(hit_test(&rows, &ls, 0, 4), (0, 2));
}

#[test]
fn hit_test_on_empty_document_is_origin() {
    let ls = lines(&[""]);
    let rows = layout(&ls, 20);
    assert_eq!(hit_test(&rows, &ls, 3, 3), (0, 0));
    assert_eq!(hit_test(&[], &ls, 0, 0), (0, 0));
}
