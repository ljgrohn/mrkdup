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
mod tests;
