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
    // Same shape but the multi-byte char sits earlier in the hay:
    // "café" is 4 chars / 5 bytes, so a byte-index bug in the
    // fallback would report 6 here instead of the correct 5.
    assert_eq!(find_ci("café Meeting", "MEETING", 0), Some(5));
}
