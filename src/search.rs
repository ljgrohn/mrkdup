//! Literal, case-insensitive search matching: the pure matcher `render.rs`
//! uses for highlighting, and the wrap-around next-match search `App`
//! drives from Ctrl+F / Ctrl+G.

/// Case-insensitive literal find: the char index of the first match of
/// `query` in `hay` at or after char index `from_char`.
///
/// Markdown source is overwhelmingly ASCII, so the common case scans
/// bytes directly with `eq_ignore_ascii_case` and allocates nothing.
/// Byte index == char index for an all-ASCII string, so offsets line up
/// with the general API. Anything with non-ASCII bytes (in either `hay`
/// or `query`) falls back to the char-by-char unicode-aware path.
pub(crate) fn find_ci(hay: &str, query: &str, from_char: usize) -> Option<usize> {
    if hay.is_ascii() && query.is_ascii() {
        find_ci_ascii(hay.as_bytes(), query.as_bytes(), from_char)
    } else {
        find_ci_unicode(hay, query, from_char)
    }
}

fn find_ci_ascii(hay: &[u8], query: &[u8], from_char: usize) -> Option<usize> {
    if query.is_empty() || hay.len() < query.len() {
        return None;
    }
    (from_char..=hay.len() - query.len())
        .find(|&start| hay[start..start + query.len()].eq_ignore_ascii_case(query))
}

fn find_ci_unicode(hay: &str, query: &str, from_char: usize) -> Option<usize> {
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

    #[test]
    fn find_ci_ascii_fast_path_is_case_insensitive() {
        assert_eq!(find_ci("Hello World", "world", 0), Some(6));
        assert_eq!(find_ci("aaa", "A", 1), Some(1));
        assert_eq!(find_ci("abcabc", "ABC", 1), Some(3));
        assert_eq!(find_ci("abc", "", 0), None);
        assert_eq!(find_ci("ab", "abc", 0), None);
    }

    #[test]
    fn find_ci_ascii_fast_path_and_unicode_fallback_agree_on_char_offsets() {
        // Non-ASCII hay routes through the unicode fallback; the match
        // index must still be a char offset, not a byte offset (é is a
        // 2-byte char, so a byte-index bug would shift this by one).
        assert_eq!(find_ci("héLLo alpha", "ALPHA", 0), Some(6));
        // ASCII hay with a non-ASCII query: no match, must not panic.
        assert_eq!(find_ci("abc", "é", 0), None);
    }
}
