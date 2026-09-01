use super::*;
use std::fs;

fn tmpfile(tag: &str, content: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("mrkdup-ed-{tag}.md"));
    fs::write(&p, content).unwrap();
    p
}

#[test]
fn open_loads_lines() {
    let p = tmpfile("open", "# a\n\nb\n");
    let mut ed = Editor::new();
    ed.open(&p).unwrap();
    assert_eq!(ed.textarea.lines(), ["# a", "", "b"]);
    assert!(!ed.dirty);
}

#[test]
fn failed_open_does_not_mutate_the_still_open_document() {
    // an LF document is already open...
    let good = tmpfile("txn-good", "line1\nline2\n");
    let mut ed = Editor::new();
    ed.open(&good).unwrap();
    assert_eq!(ed.newline, Newline::Lf);

    // ...then an open of an invalid-UTF-8 (e.g. Latin-1) file fails —
    // and its bytes are CRLF, so a buggy detect-before-validate order
    // would stamp CRLF onto the still-open LF document below...
    let bad = std::env::temp_dir().join("mrkdup-ed-txn-bad.md");
    fs::write(&bad, [b'x', b'\r', b'\n', 0xff, b'y']).unwrap();
    assert!(ed.open(&bad).is_err());

    // ...and the still-open LF document is untouched: same path, same
    // newline style, not marked dirty by the failed attempt.
    assert_eq!(ed.path.as_deref(), Some(good.as_path()));
    assert_eq!(ed.newline, Newline::Lf);
    assert!(!ed.dirty);

    // editing and saving the still-open document preserves LF endings
    // and its content, rather than picking up CRLF from the failed
    // open's (never-validated) bytes.
    ed.textarea.insert_str("hi ");
    ed.mark_dirty();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Saved));
    assert_eq!(fs::read_to_string(&good).unwrap(), "hi line1\nline2\n");
}

#[test]
fn save_when_clean_is_noop() {
    let p = tmpfile("clean", "x\n");
    let mut ed = Editor::new();
    ed.open(&p).unwrap();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Clean));
}

#[test]
fn save_writes_with_trailing_newline() {
    let p = tmpfile("save", "x\n");
    let mut ed = Editor::new();
    ed.open(&p).unwrap();
    ed.textarea.insert_str("hi ");
    ed.mark_dirty();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Saved));
    assert!(!ed.dirty);
    assert_eq!(fs::read_to_string(&p).unwrap(), "hi x\n");
}

#[test]
fn save_with_no_file_is_nofile() {
    let mut ed = Editor::new();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::NoFile));
}

#[test]
fn external_change_while_dirty_blocks_save_then_force_wins() {
    let p = tmpfile("conflict", "x\n");
    let mut ed = Editor::new();
    ed.open(&p).unwrap();
    ed.textarea.insert_str("mine ");
    ed.mark_dirty();
    // simulate an external writer (bump mtime meaningfully)
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&p, "theirs\n").unwrap();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Conflict));
    assert_eq!(fs::read_to_string(&p).unwrap(), "theirs\n"); // untouched
    assert!(matches!(ed.save(true).unwrap(), SaveOutcome::Saved));
    assert_eq!(fs::read_to_string(&p).unwrap(), "mine x\n");
}

#[test]
fn external_change_while_clean_reloads() {
    let p = tmpfile("reload", "x\n");
    let mut ed = Editor::new();
    ed.open(&p).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&p, "fresh\n").unwrap();
    assert!(ed.check_external().unwrap());
    assert_eq!(ed.textarea.lines(), ["fresh"]);
}

#[test]
fn external_change_while_dirty_does_not_reload() {
    let p = tmpfile("noreload", "x\n");
    let mut ed = Editor::new();
    ed.open(&p).unwrap();
    ed.textarea.insert_str("mine");
    ed.mark_dirty();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&p, "theirs\n").unwrap();
    assert!(!ed.check_external().unwrap());
    assert_eq!(ed.textarea.lines()[0], "minex");
}

#[test]
fn crlf_round_trip_preserved_on_edit_and_save() {
    // Create a file with CRLF endings using raw bytes.
    let p = std::env::temp_dir().join("mrkdup-crlf-rt.md");
    fs::write(&p, b"hello\r\nworld\r\n").unwrap();

    let mut ed = Editor::new();
    ed.open(&p).unwrap();

    // Verify it detected CRLF.
    assert_eq!(ed.newline, Newline::CrLf);
    assert_eq!(ed.textarea.lines(), ["hello", "world"]);

    // Edit and save.
    ed.textarea.insert_str("!");
    ed.mark_dirty();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Saved));

    // Verify file uses only CRLF line endings (no bare \n).
    let bytes = fs::read(&p).unwrap();
    let lf_count = bytes.iter().filter(|&&b| b == b'\n').count();
    let crlf_count = bytes.windows(2).filter(|w| w == b"\r\n").count();
    assert_eq!(
        lf_count, crlf_count,
        "All line endings should be \\r\\n; LF count {} != CRLF count {}",
        lf_count, crlf_count
    );
}

#[test]
fn lf_round_trip_preserved_on_edit_and_save() {
    // Create a file with LF endings.
    let p = std::env::temp_dir().join("mrkdup-lf-rt.md");
    fs::write(&p, b"hello\nworld\n").unwrap();

    let mut ed = Editor::new();
    ed.open(&p).unwrap();

    // Verify it detected LF.
    assert_eq!(ed.newline, Newline::Lf);
    assert_eq!(ed.textarea.lines(), ["hello", "world"]);

    // Edit and save.
    ed.textarea.insert_str("!");
    ed.mark_dirty();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Saved));

    // Verify file still contains only \n (not \r\n).
    let bytes = fs::read(&p).unwrap();
    assert!(
        !bytes.windows(2).any(|w| w == b"\r\n"),
        "File should not contain \\r\\n"
    );
    let content = String::from_utf8_lossy(&bytes);
    assert!(content.contains('\n'), "File should contain \\n");
}

#[test]
fn empty_file_defaults_to_lf() {
    // Create an empty file.
    let p = std::env::temp_dir().join("mrkdup-empty.md");
    fs::write(&p, b"").unwrap();

    let mut ed = Editor::new();
    ed.open(&p).unwrap();

    // Verify it defaults to LF for empty files.
    assert_eq!(ed.newline, Newline::Lf);

    // Type content and save.
    ed.textarea.insert_str("hello");
    ed.mark_dirty();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Saved));

    // Verify file has only \n.
    let bytes = fs::read(&p).unwrap();
    assert!(
        !bytes.windows(2).any(|w| w == b"\r\n"),
        "File should not contain \\r\\n"
    );
}

#[test]
fn mixed_endings_first_line_terminator_wins_lf() {
    // Create a file with mostly CRLF but the first line ends in bare LF.
    // Per the policy, the first line terminator (\n) determines the style.
    // Even though CRLF is the majority here, LF comes first.
    let p = std::env::temp_dir().join("mrkdup-mixed-lf-first.md");
    fs::write(&p, b"hello\nworld\r\nfoo\r\nbar\r\n").unwrap();

    let mut ed = Editor::new();
    ed.open(&p).unwrap();

    // Verify it detected LF (first terminator).
    assert_eq!(ed.newline, Newline::Lf);

    // Edit and save.
    ed.textarea.insert_str("!");
    ed.mark_dirty();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Saved));

    // Verify file is now all LF (mixed endings converted to LF).
    let bytes = fs::read(&p).unwrap();
    assert!(
        !bytes.windows(2).any(|w| w == b"\r\n"),
        "File should not contain \\r\\n"
    );
}

#[test]
fn mixed_endings_first_line_terminator_wins_crlf() {
    // Create a file with mostly LF but the first line ends in CRLF.
    // Per the policy, the first line terminator (\n) determines the style.
    // Even though LF is the majority, CRLF comes first.
    let p = std::env::temp_dir().join("mrkdup-mixed-crlf-first.md");
    fs::write(&p, b"hello\r\nworld\nfoo\nbar\n").unwrap();

    let mut ed = Editor::new();
    ed.open(&p).unwrap();

    // Verify it detected CRLF (first terminator).
    assert_eq!(ed.newline, Newline::CrLf);

    // Edit and save.
    ed.textarea.insert_str("!");
    ed.mark_dirty();
    assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Saved));

    // Verify file is now all CRLF (mixed endings converted to CRLF).
    let bytes = fs::read(&p).unwrap();
    let lf_count = bytes.iter().filter(|&&b| b == b'\n').count();
    let crlf_count = bytes.windows(2).filter(|w| w == b"\r\n").count();
    assert_eq!(
        lf_count, crlf_count,
        "All line endings should be \\r\\n; LF count {} != CRLF count {}",
        lf_count, crlf_count
    );
}
