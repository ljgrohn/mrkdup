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
