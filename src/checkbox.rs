//! Pure markdown-checkbox helpers: toggling a line's checkbox state and
//! detecting the `--0` expansion trigger. `App` owns the textarea-rewriting
//! glue that drives these from Ctrl+D and typed input.

/// The checkbox-toggled form of `line`, indentation preserved.
pub(crate) fn toggle_checkbox_line(line: &str) -> String {
    let indent_end = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_end);
    if let Some(tail) = rest.strip_prefix("- [ ]") {
        format!("{indent}- [x]{tail}")
    } else if let Some(tail) = rest.strip_prefix("- [x]") {
        format!("{indent}- [ ]{tail}")
    } else if let Some(tail) = rest.strip_prefix("- [X]") {
        format!("{indent}- [ ]{tail}")
    } else if let Some(tail) = rest.strip_prefix("- ") {
        format!("{indent}- [ ] {tail}")
    } else {
        format!("{indent}- [ ] {rest}")
    }
}

/// True when the two chars before `col` in `line` are exactly "--"
/// (not part of a longer dash run).
pub(crate) fn trigger_armed(line: &str, col: usize) -> bool {
    let before: Vec<char> = line.chars().take(col).collect();
    col >= 2
        && before[col - 1] == '-'
        && before[col - 2] == '-'
        && (col == 2 || before[col - 3] != '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_checkbox_line_checks_an_unchecked_box() {
        assert_eq!(toggle_checkbox_line("- [ ] milk"), "- [x] milk");
    }

    #[test]
    fn toggle_checkbox_line_unchecks_a_checked_box() {
        assert_eq!(toggle_checkbox_line("- [x] milk"), "- [ ] milk");
    }

    #[test]
    fn toggle_checkbox_line_unchecks_an_uppercase_checked_box() {
        assert_eq!(toggle_checkbox_line("- [X] milk"), "- [ ] milk");
    }

    #[test]
    fn toggle_checkbox_line_turns_a_bullet_into_a_checkbox() {
        assert_eq!(toggle_checkbox_line("- milk"), "- [ ] milk");
    }

    #[test]
    fn toggle_checkbox_line_prefixes_a_plain_line() {
        assert_eq!(toggle_checkbox_line("hello"), "- [ ] hello");
    }

    #[test]
    fn toggle_checkbox_line_preserves_indentation() {
        assert_eq!(toggle_checkbox_line("  - [ ] a"), "  - [x] a");
        assert_eq!(toggle_checkbox_line("    plain"), "    - [ ] plain");
    }

    #[test]
    fn trigger_armed_after_exactly_two_dashes() {
        assert!(trigger_armed("--", 2));
        assert!(trigger_armed("x --", 4));
    }

    #[test]
    fn trigger_armed_rejects_a_longer_dash_run() {
        assert!(!trigger_armed("---", 3));
    }

    #[test]
    fn trigger_armed_rejects_fewer_than_two_dashes() {
        assert!(!trigger_armed("-", 1));
        assert!(!trigger_armed("", 0));
    }
}
