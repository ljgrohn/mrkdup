//! Literal, case-insensitive search matching: the pure matcher `render.rs`
//! uses for highlighting, and the wrap-around next-match search `App`
//! drives from Ctrl+F / Ctrl+G.

/// Case-insensitive literal find: the char index of the first match of
/// `query` in `hay` at or after char index `from_char`.
pub(crate) fn find_ci(hay: &str, query: &str, from_char: usize) -> Option<usize> {
    let h: Vec<char> = hay.chars().collect();
    let q: Vec<char> = query.chars().collect();
    if q.is_empty() || h.len() < q.len() {
        return None;
    }
    let ci_eq = |a: &char, b: &char| a.to_lowercase().eq(b.to_lowercase());
    (from_char..=h.len() - q.len()).find(|&start| {
        h[start..start + q.len()]
            .iter()
            .zip(&q)
            .all(|(a, b)| ci_eq(a, b))
    })
}

/// Literal, case-insensitive, wraps around; starts one char after `cursor`.
/// Returns the (row, char col) of the next match of `query`, if any.
pub(crate) fn next(
    lines: &[String],
    cursor: (usize, usize),
    query: &str,
) -> Option<(usize, usize)> {
    if query.is_empty() {
        return None;
    }
    let (crow, ccol) = cursor;
    let n = lines.len();
    for i in 0..=n {
        let row = (crow + i) % n;
        let hay = &lines[row];
        let from_char = if i == 0 { ccol + 1 } else { 0 };
        if let Some(cpos) = find_ci(hay, query, from_char) {
            return Some((row, cpos));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_ci_handles_unicode_and_offsets() {
        assert_eq!(find_ci("héLLo héllo", "Éllo", 0), Some(1));
        assert_eq!(find_ci("héLLo héllo", "Éllo", 2), Some(7));
        assert_eq!(find_ci("abc", "zzz", 0), None);
        assert_eq!(find_ci("abc", "", 0), None);
        assert_eq!(find_ci("ab", "abc", 0), None);
    }
}
