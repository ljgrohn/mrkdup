use super::*;
use crate::search::find_ci;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}
/// Nested one level inside `mrkdup-app-{tag}` (a directory this test
/// owns) rather than returning that directory itself: `dash_reroots_
/// tree_at_parent` reroots the tree at the fixture's *parent*, and if
/// the fixture root were the top-level temp dir entry, that parent
/// would be the real shared system temp dir -- which hangs on
/// GitHub's Ubuntu/macOS runners (see the `is_text_file` fix; they
/// leave FIFOs sitting in temp). Keeping the parent owned by the test
/// avoids that regardless of what's in the real temp dir.
fn fixture(tag: &str) -> std::path::PathBuf {
    let owned = std::env::temp_dir().join(format!("mrkdup-app-{tag}"));
    let _ = fs::remove_dir_all(&owned);
    let root = owned.join("root");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "hello\nworld\n").unwrap();
    fs::write(root.join("b.md"), "bee\n").unwrap();
    root
}

#[test]
fn starts_focused_on_tree() {
    let app = App::new(fixture("start"), Config::default()).unwrap();
    assert!(matches!(app.focus, Focus::Tree));
}

#[test]
fn enter_opens_file_and_focuses_editor() {
    let mut app = App::new(fixture("open"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // a.md selected first
    assert!(matches!(app.focus, Focus::Editor));
    assert_eq!(app.editor.lines(), ["hello", "world"]);
}

#[test]
fn esc_returns_to_tree() {
    let mut app = App::new(fixture("esc"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.focus, Focus::Tree));
}

#[test]
fn typing_marks_dirty_and_switching_files_autosaves() {
    let root = fixture("autosave");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(key(KeyCode::Char('X')));
    assert!(app.editor.dirty);
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('j'))); // b.md
    app.handle_key(key(KeyCode::Enter)); // open b.md -> autosaves a.md
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Xhello\nworld\n"
    );
    assert_eq!(app.editor.lines(), ["bee"]);
}

#[test]
fn ctrl_z_undoes_and_ctrl_y_redoes() {
    let mut app = App::new(fixture("undo"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('X')));
    assert_eq!(app.editor.lines()[0], "Xhello");
    app.handle_key(ctrl('z'));
    assert_eq!(app.editor.lines()[0], "hello");
    app.handle_key(ctrl('y'));
    assert_eq!(app.editor.lines()[0], "Xhello");
}

#[test]
fn ctrl_b_toggles_tree_and_fixes_focus() {
    let mut app = App::new(fixture("toggle"), Config::default()).unwrap();
    app.handle_key(ctrl('b'));
    assert!(!app.tree_visible);
    assert!(matches!(app.focus, Focus::Editor));
    app.handle_key(ctrl('b'));
    assert!(app.tree_visible);
}

#[test]
fn ctrl_t_hides_editor_and_opening_a_file_reshows_it() {
    let mut app = App::new(fixture("epane"), Config::default()).unwrap();
    app.handle_key(ctrl('t'));
    assert!(!app.editor_visible);
    assert!(app.tree_visible);
    assert!(matches!(app.focus, Focus::Tree));
    app.handle_key(key(KeyCode::Enter)); // open a.md
    assert!(app.editor_visible);
    assert!(matches!(app.focus, Focus::Editor));
}

#[test]
fn panes_can_never_both_be_hidden() {
    let mut app = App::new(fixture("panes"), Config::default()).unwrap();
    app.handle_key(ctrl('t')); // editor hidden
    app.handle_key(ctrl('b')); // hide tree -> editor must come back
    assert!(app.editor_visible);
    assert!(!app.tree_visible);
    app.handle_key(ctrl('t')); // hide editor -> tree must come back
    assert!(app.tree_visible);
    assert!(!app.editor_visible);
}

#[test]
fn ctrl_q_saves_and_quits() {
    let root = fixture("quit");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('Q')));
    app.handle_key(ctrl('q'));
    assert!(app.should_quit);
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Qhello\nworld\n"
    );
}

#[test]
fn new_file_prompt_creates_and_opens() {
    let root = fixture("newfile");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('n')));
    for c in "notes.md".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert!(root.join("notes.md").exists());
    assert!(matches!(app.focus, Focus::Editor));
}

#[test]
fn search_jumps_to_match() {
    let mut app = App::new(fixture("search"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // a.md: hello / world
    app.handle_key(ctrl('f'));
    for c in "wor".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.editor.cursor(), (1, 0));
}

#[test]
fn empty_search_repeats_last_search() {
    let mut app = App::new(fixture("repeat"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // hello / world: two 'l'-runs
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Char('l')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.editor.cursor(), (0, 2));
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Enter)); // empty -> repeat "l"
    assert_eq!(app.editor.cursor(), (0, 3));
}

#[test]
fn ctrl_g_jumps_to_the_next_match_of_the_last_search() {
    let mut app = App::new(fixture("next"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // hello / world
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Char('l')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.editor.cursor(), (0, 2));
    app.handle_key(ctrl('g'));
    assert_eq!(app.editor.cursor(), (0, 3));
    app.handle_key(ctrl('g'));
    assert_eq!(app.editor.cursor(), (1, 3)); // "world"
}

#[test]
fn ctrl_g_without_a_previous_search_shows_a_status_message() {
    let mut app = App::new(fixture("next-none"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('g'));
    assert_eq!(app.editor.cursor(), (0, 0)); // didn't move
    assert!(app.status.as_deref().is_some_and(|s| s.contains("search")));
}

#[test]
fn search_submit_arms_the_renderer_highlight() {
    let mut app = App::new(fixture("hl"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('f'));
    for c in "wor".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.search_highlight.as_deref(), Some("wor"));
}

#[test]
fn opening_a_file_clears_the_search_highlight() {
    let mut app = App::new(fixture("hl-clear"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // a.md
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Char('l')));
    app.handle_key(key(KeyCode::Enter));
    assert!(app.search_highlight.is_some());
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('j'))); // b.md
    app.handle_key(key(KeyCode::Enter));
    assert!(app.search_highlight.is_none());
}

#[test]
fn search_query_with_regex_metachars_matches_literally() {
    let root = fixture("meta");
    fs::write(root.join("a.md"), "price (a.b) here\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('f'));
    for c in "(a.b)".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.editor.cursor(), (0, 6));
    // and the renderer's matcher is literal too: "axb" is no match
    assert_eq!(find_ci("price axb here", "(a.b)", 0), None);
}

#[test]
fn dash_reroots_tree_at_parent() {
    let root = fixture("ascend");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('-')));
    assert_eq!(
        app.tree.root(),
        root.canonicalize().unwrap().parent().unwrap()
    );
    assert_eq!(
        app.tree.selected_row().unwrap().path,
        root.canonicalize().unwrap()
    );
}

#[test]
fn shift_jk_types_capital_letters() {
    let mut app = App::new(fixture("motion"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // hello / world, cursor (0,0)
    app.handle_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
    app.handle_key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
    assert_eq!(app.editor.lines()[0], "JKhello"); // typed, not moved
}

#[test]
fn alt_jk_jumps_by_paragraph() {
    let root = fixture("para");
    fs::write(root.join("a.md"), "one\n\ntwo\n\nthree\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT));
    let (row, _) = app.editor.cursor();
    assert!(row >= 1); // moved past the blank line
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT));
    assert_eq!(app.editor.cursor(), (0, 0));
}

#[test]
fn super_jk_jumps_to_line_end_and_start() {
    let mut app = App::new(fixture("linejump"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // hello
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SUPER));
    assert_eq!(app.editor.cursor(), (0, 5)); // end of "hello"
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::SUPER));
    assert_eq!(app.editor.cursor(), (0, 0));
}

#[test]
fn dash_dash_zero_expands_to_checkbox() {
    let mut app = App::new(fixture("expand0"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // "hello", cursor at (0,0)
    app.handle_key(key(KeyCode::Char('-')));
    app.handle_key(key(KeyCode::Char('-')));
    app.handle_key(key(KeyCode::Char('0')));
    assert_eq!(app.editor.lines()[0], "- [ ] hello");
    assert_eq!(app.editor.cursor(), (0, 6)); // ready to type the item
    assert!(app.editor.dirty);
}

#[test]
fn plain_zero_still_types_zero() {
    let mut app = App::new(fixture("zero"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('0')));
    assert_eq!(app.editor.lines()[0], "0hello");
}

#[test]
fn triple_dash_zero_does_not_expand() {
    let mut app = App::new(fixture("dashes"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    for c in "---0".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    assert_eq!(app.editor.lines()[0], "---0hello");
}

#[test]
fn plain_p_and_q_work_in_tree() {
    let root = fixture("plain-keys");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('p')));
    assert!(matches!(app.prompt, Prompt::GoToFile { .. }));
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('q')));
    assert!(app.should_quit);
}

#[test]
fn plain_p_and_q_still_type_in_editor() {
    let root = fixture("plain-keys-editor");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md ("hello"...)
    app.handle_key(key(KeyCode::Char('q')));
    app.handle_key(key(KeyCode::Char('p')));
    assert_eq!(app.editor.lines()[0], "qphello");
    assert!(!app.should_quit);
}

#[test]
fn search_is_case_insensitive_both_ways() {
    let root = fixture("search-ci");
    fs::write(root.join("a.md"), "Ship it\nfriend ship\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    // all-caps query finds the lowercase occurrence first (search
    // starts one char after the cursor, skipping "Ship" at 0:0)...
    app.handle_key(ctrl('f'));
    for c in "SHIP".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.editor.cursor(), (1, 7));
    // ...and Ctrl+G wraps around to the capitalized one
    app.handle_key(ctrl('g'));
    assert_eq!(app.editor.cursor(), (0, 0));
}

#[test]
fn ctrl_jk_move_by_word_without_deleting() {
    let root = fixture("word-motion");
    fs::write(root.join("a.md"), "alpha bravo charlie\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('j'));
    let (_, col1) = app.editor.cursor();
    assert!(col1 > 0); // advanced
    app.handle_key(ctrl('j'));
    let (_, col2) = app.editor.cursor();
    assert!(col2 > col1);
    app.handle_key(ctrl('k'));
    assert_eq!(app.editor.cursor(), (0, col1));
    // nothing was deleted (Ctrl+K used to be kill-to-end-of-line)
    assert_eq!(app.editor.lines()[0], "alpha bravo charlie");
    assert!(!app.editor.dirty);
}

#[test]
fn ctrl_d_checks_an_unchecked_checkbox() {
    let root = fixture("cb-check");
    fs::write(root.join("a.md"), "- [ ] milk\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines()[0], "- [x] milk");
    assert_eq!(app.editor.cursor(), (0, 0)); // same width: cursor stays
    assert!(app.editor.dirty);
}

#[test]
fn ctrl_d_unchecks_a_checked_checkbox() {
    let root = fixture("cb-uncheck");
    fs::write(root.join("a.md"), "- [x] milk\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines()[0], "- [ ] milk");
}

#[test]
fn ctrl_d_unchecks_uppercase_checked_checkbox() {
    let root = fixture("cb-uncheck-upper");
    fs::write(root.join("a.md"), "- [X] milk\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines()[0], "- [ ] milk");
}

#[test]
fn ctrl_d_with_active_selection_only_touches_cursor_line() {
    let root = fixture("cb-selection");
    fs::write(root.join("a.md"), "alpha\nbravo\ncharlie\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    // select two lines with Shift+Down, then toggle
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines(), ["alpha", "bravo", "- [ ] charlie"]);
}

#[test]
fn typing_with_no_file_open_is_ignored() {
    let root = fixture("no-file-typing");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(ctrl('b')); // hide tree -> editor focus, no file
    app.handle_key(key(KeyCode::Char('x')));
    assert_eq!(app.editor.lines(), [""]); // nothing typed
    assert!(!app.editor.dirty);
    assert!(app.status.is_some()); // told the user why
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.focus, Focus::Tree)); // Esc still escapes
}

#[test]
fn ctrl_d_turns_a_bullet_into_a_checkbox() {
    let root = fixture("cb-bullet");
    fs::write(root.join("a.md"), "- milk\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines()[0], "- [ ] milk");
}

#[test]
fn ctrl_d_prefixes_a_plain_line_and_shifts_the_cursor() {
    let root = fixture("cb-plain");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // "hello", cursor (0,0)
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines()[0], "- [ ] hello");
    assert_eq!(app.editor.cursor(), (0, 6)); // still on the 'h'
}

#[test]
fn ctrl_d_preserves_indentation() {
    let root = fixture("cb-indent");
    fs::write(root.join("a.md"), "  - [ ] a\n    plain\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines()[0], "  - [x] a");
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines()[1], "    - [ ] plain");
}

#[test]
fn ctrl_d_on_an_empty_line_does_not_join_the_next_line() {
    let root = fixture("cb-empty");
    fs::write(root.join("a.md"), "\nworld\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines(), ["- [ ] ", "world"]);
}

#[test]
fn ctrl_d_is_undoable() {
    let root = fixture("cb-undo");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // "hello"
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor.lines()[0], "- [ ] hello");
    // the toggle is a delete + an insert, so two undo steps
    app.handle_key(ctrl('z'));
    app.handle_key(ctrl('z'));
    assert_eq!(app.editor.lines()[0], "hello");
}

#[test]
fn shift_tab_in_editor_returns_to_tree_without_typing() {
    let mut app = App::new(fixture("backtab"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert!(matches!(app.focus, Focus::Tree));
    assert_eq!(app.editor.lines(), ["hello", "world"]); // unchanged
    assert!(!app.editor.dirty);
}

#[test]
fn plus_makes_selected_folder_the_root() {
    let root = fixture("mkroot");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/x.md"), "x\n").unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    // docs/ sorts first (dirs before files), so it's already selected
    app.handle_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::SHIFT));
    assert_eq!(app.tree.root(), root.canonicalize().unwrap().join("docs"));
}

#[test]
fn shift_x_confirm_no_by_default_keeps_file() {
    let root = fixture("del-no");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('x')));
    assert!(matches!(
        app.prompt,
        Prompt::ConfirmDelete { yes: false, .. }
    ));
    app.handle_key(key(KeyCode::Enter)); // No selected -> just closes
    assert!(matches!(app.prompt, Prompt::None));
    assert!(root.join("a.md").exists());
}

#[test]
fn shift_x_then_yes_deletes_file() {
    let root = fixture("del-yes");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('x')));
    app.handle_key(key(KeyCode::Char('j'))); // move highlight to Yes
    app.handle_key(key(KeyCode::Enter));
    assert!(!root.join("a.md").exists());
    assert!(!app.tree.rows().iter().any(|r| r.name == "a.md"));
}

#[test]
fn shift_x_inside_popup_deletes_immediately() {
    let root = fixture("del-xx");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('x')));
    app.handle_key(key(KeyCode::Char('x')));
    assert!(!root.join("a.md").exists());
}

#[test]
fn esc_closes_delete_popup_without_deleting() {
    let root = fixture("del-esc");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('x')));
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(root.join("a.md").exists());
}

#[test]
fn deleting_the_open_file_clears_the_editor() {
    let root = fixture("del-open");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('x')));
    app.handle_key(key(KeyCode::Char('k'))); // k also toggles to Yes
    app.handle_key(key(KeyCode::Enter));
    assert!(app.editor.path.is_none());
    assert_eq!(app.editor.lines(), [""]);
}

#[test]
fn shift_x_on_directory_is_refused() {
    let root = fixture("del-dir");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/x.md"), "x\n").unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    // docs/ sorts first, so it's selected
    app.handle_key(key(KeyCode::Char('x')));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(app.status.is_some());
    assert!(root.join("docs").exists());
}

#[test]
fn m_moves_file_into_chosen_directory() {
    let root = fixture("move");
    fs::create_dir_all(root.join("docs")).unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('j'))); // docs(0) -> a.md(1)
    app.handle_key(key(KeyCode::Char('m')));
    assert!(matches!(app.prompt, Prompt::MoveFile { .. }));
    app.handle_key(key(KeyCode::Char('j'))); // root -> docs
    app.handle_key(key(KeyCode::Enter));
    assert!(root.join("docs/a.md").exists());
    assert!(!root.join("a.md").exists());
}

#[test]
fn m_on_directory_is_refused() {
    let root = fixture("move-dir");
    fs::create_dir_all(root.join("docs")).unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('m'))); // docs selected
    assert!(matches!(app.prompt, Prompt::None));
    assert!(app.status.is_some());
}

#[test]
fn moving_the_open_file_keeps_editing_it_at_the_new_path() {
    let root = fixture("move-open");
    fs::create_dir_all(root.join("docs")).unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('m')));
    app.handle_key(key(KeyCode::Char('j'))); // docs
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.editor.path.as_deref(),
        Some(root.canonicalize().unwrap().join("docs/a.md").as_path())
    );
    // edits still save to the new location
    app.focus = Focus::Editor;
    app.handle_key(key(KeyCode::Char('Z')));
    app.handle_key(ctrl('s'));
    assert!(fs::read_to_string(root.join("docs/a.md"))
        .unwrap()
        .starts_with('Z'));
}

#[test]
fn move_to_same_directory_is_rejected() {
    let root = fixture("move-same");
    fs::create_dir_all(root.join("docs")).unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('j'))); // a.md
    app.handle_key(key(KeyCode::Char('m')));
    app.handle_key(key(KeyCode::Enter)); // first dest is root = current dir
    assert!(root.join("a.md").exists());
    assert!(app.status.is_some());
}

#[test]
fn u_refreshes_tree_to_pick_up_external_files() {
    let root = fixture("refresh-key");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    fs::write(root.join("new.md"), "n\n").unwrap();
    assert!(!app.tree.rows().iter().any(|r| r.name == "new.md"));
    app.handle_key(key(KeyCode::Char('u')));
    assert!(app.tree.rows().iter().any(|r| r.name == "new.md"));
}

#[test]
fn tick_auto_refreshes_tree_periodically() {
    let root = fixture("refresh-tick");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    fs::write(root.join("new.md"), "n\n").unwrap();
    app.last_tree_refresh = std::time::Instant::now() - std::time::Duration::from_secs(3);
    app.tick();
    assert!(app.tree.rows().iter().any(|r| r.name == "new.md"));
}

/// Open the rename popup, erase `erase` chars of the prefill, type
/// `name`, and submit.
fn rename_to(app: &mut App, erase: usize, name: &str) {
    app.handle_key(key(KeyCode::Char('r')));
    for _ in 0..erase {
        app.handle_key(key(KeyCode::Backspace));
    }
    for c in name.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
}

#[test]
fn shift_r_opens_rename_popup_prefilled_with_the_file_name() {
    let mut app = App::new(fixture("ren-open"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('r')));
    match &app.prompt {
        Prompt::Rename { input, .. } => assert_eq!(input, "a.md"),
        _ => panic!("expected rename prompt"),
    }
}

#[test]
fn rename_renames_the_file_and_keeps_it_selected() {
    let root = fixture("ren-do");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    rename_to(&mut app, 4, "z.md"); // a.md -> z.md (sorts after b.md)
    assert!(root.join("z.md").exists());
    assert!(!root.join("a.md").exists());
    assert_eq!(app.tree.selected_row().unwrap().name, "z.md");
}

#[test]
fn rename_to_case_variant_of_itself_works() {
    // on case-insensitive filesystems (macOS default) A.md "exists"
    // when a.md does — a case-only rename must still go through
    let root = fixture("ren-case");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    rename_to(&mut app, 4, "A.md");
    assert!(root.join("A.md").exists());
    let names: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"A.md".to_string()));
    assert!(!names.contains(&"a.md".to_string()));
}

#[test]
fn rename_to_existing_name_is_rejected() {
    let root = fixture("ren-exists");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    rename_to(&mut app, 4, "b.md");
    assert!(root.join("a.md").exists());
    assert!(root.join("b.md").exists());
    assert!(app.status.as_deref().is_some_and(|s| s.contains("exists")));
}

#[test]
fn rename_to_invalid_names_is_rejected() {
    let root = fixture("ren-invalid");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    for name in ["docs/x.md", "..", ""] {
        rename_to(&mut app, 4, name);
        assert!(root.join("a.md").exists(), "rejected rename to {name:?}");
        assert!(
            app.status.as_deref().is_some_and(|s| s.contains("invalid")),
            "status for {name:?}"
        );
    }
}

#[test]
fn shift_r_on_directory_is_refused() {
    let root = fixture("ren-dir");
    fs::create_dir_all(root.join("docs")).unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    // docs/ sorts first, so it's selected
    app.handle_key(key(KeyCode::Char('r')));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(app.status.as_deref().is_some_and(|s| s.contains("rename")));
}

#[test]
fn renaming_the_open_file_keeps_editing_it_at_the_new_path() {
    let root = fixture("ren-open-file");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(key(KeyCode::Esc));
    rename_to(&mut app, 4, "z.md");
    assert_eq!(
        app.editor.path.as_deref(),
        Some(root.canonicalize().unwrap().join("z.md").as_path())
    );
    // edits still save to the new name
    app.focus = Focus::Editor;
    app.handle_key(key(KeyCode::Char('Z')));
    app.handle_key(ctrl('s'));
    assert!(fs::read_to_string(root.join("z.md"))
        .unwrap()
        .starts_with('Z'));
}

#[test]
fn esc_closes_rename_popup_without_renaming() {
    let root = fixture("ren-esc");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('r')));
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(root.join("a.md").exists());
}

#[test]
fn ctrl_p_opens_go_to_file_with_text_files_from_the_whole_root() {
    let root = fixture("gtf-open");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/deep.md"), "d\n").unwrap();
    fs::write(root.join("bin.dat"), b"\x00\x01").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(ctrl('p'));
    match &app.prompt {
        Prompt::GoToFile { candidates, .. } => {
            let rels: Vec<&str> = candidates.iter().map(|c| c.0.as_str()).collect();
            assert!(rels.contains(&"a.md"));
            assert!(rels.contains(&"docs/deep.md")); // walks subdirs, root-relative
            assert!(!rels.iter().any(|r| r.contains("bin.dat"))); // text files only
            assert!(!rels.iter().any(|s| s.contains('\\'))); // no backslashes on any OS
        }
        _ => panic!("expected go-to-file prompt"),
    }
}

#[test]
fn ctrl_p_typing_filters_and_enter_opens_the_top_match() {
    let root = fixture("gtf-enter");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(ctrl('p'));
    app.handle_key(key(KeyCode::Char('b')));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(matches!(app.focus, Focus::Editor));
    assert_eq!(
        app.editor.path.as_deref(),
        Some(root.canonicalize().unwrap().join("b.md").as_path())
    );
}

#[test]
fn ctrl_p_selection_moves_with_arrows_and_ctrl_jk() {
    let mut app = App::new(fixture("gtf-move"), Config::default()).unwrap();
    app.handle_key(ctrl('p')); // empty query: a.md, b.md in order
    app.handle_key(key(KeyCode::Down));
    match &app.prompt {
        Prompt::GoToFile { selected, .. } => assert_eq!(*selected, 1),
        _ => panic!("expected go-to-file prompt"),
    }
    app.handle_key(ctrl('k'));
    match &app.prompt {
        Prompt::GoToFile { selected, .. } => assert_eq!(*selected, 0),
        _ => panic!("expected go-to-file prompt"),
    }
    app.handle_key(ctrl('j'));
    app.handle_key(key(KeyCode::Enter)); // second result = b.md
    assert!(app
        .editor
        .path
        .as_deref()
        .is_some_and(|p| p.ends_with("b.md")));
}

#[test]
fn ctrl_p_from_the_editor_autosaves_before_opening() {
    let root = fixture("gtf-autosave");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(key(KeyCode::Char('X')));
    app.handle_key(ctrl('p')); // global: works from editor focus too
    app.handle_key(key(KeyCode::Char('b')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Xhello\nworld\n"
    );
    assert_eq!(app.editor.lines(), ["bee"]);
}

#[test]
fn ctrl_p_honors_the_tree_hidden_setting() {
    let root = fixture("gtf-hidden");
    fs::write(root.join(".secret.md"), "s\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(ctrl('p'));
    match &app.prompt {
        Prompt::GoToFile { candidates, .. } => {
            assert!(!candidates.iter().any(|c| c.0.contains(".secret.md")));
        }
        _ => panic!("expected go-to-file prompt"),
    }
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('.'))); // tree: show hidden
    app.handle_key(ctrl('p'));
    match &app.prompt {
        Prompt::GoToFile { candidates, .. } => {
            assert!(candidates.iter().any(|c| c.0.contains(".secret.md")));
        }
        _ => panic!("expected go-to-file prompt"),
    }
}

#[test]
fn ctrl_p_shows_gitignored_files_only_when_hidden_is_toggled_on() {
    let root = fixture("gtf-ignored");
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("notes/debug.log"), "d\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(ctrl('p'));
    match &app.prompt {
        Prompt::GoToFile { candidates, .. } => {
            assert!(!candidates.iter().any(|c| c.0 == "notes/debug.log"));
        }
        _ => panic!("expected go-to-file prompt"),
    }
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('.'))); // tree: show hidden + ignored
    app.handle_key(ctrl('p'));
    match &app.prompt {
        Prompt::GoToFile { candidates, .. } => {
            assert!(candidates.iter().any(|c| c.0 == "notes/debug.log"));
        }
        _ => panic!("expected go-to-file prompt"),
    }
}

#[test]
fn question_mark_opens_help_and_any_key_closes_it() {
    let mut app = App::new(fixture("help"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('?')));
    assert!(matches!(app.prompt, Prompt::Help));
    // a tree key while help is open closes help; it is not dispatched
    app.handle_key(key(KeyCode::Char('j')));
    assert!(matches!(app.prompt, Prompt::None));
    assert_eq!(app.tree.selected_row().unwrap().name, "a.md");

    app.handle_key(key(KeyCode::Char('?')));
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.prompt, Prompt::None));
}

#[test]
fn question_mark_in_the_editor_types_a_character() {
    let mut app = App::new(fixture("help-editor"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md, focus editor
    app.handle_key(key(KeyCode::Char('?')));
    assert!(matches!(app.prompt, Prompt::None));
    assert_eq!(app.editor.lines()[0], "?hello");
}

#[test]
fn esc_closes_go_to_file_without_opening() {
    let mut app = App::new(fixture("gtf-esc"), Config::default()).unwrap();
    app.handle_key(ctrl('p'));
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(app.editor.path.is_none());
}

#[test]
fn ctrl_p_does_not_fire_inside_another_prompt() {
    let mut app = App::new(fixture("gtf-nested"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('n'))); // NewFile prompt
    app.handle_key(ctrl('p'));
    assert!(matches!(app.prompt, Prompt::NewFile(_)));
}

#[test]
fn enter_with_no_go_to_file_match_just_closes() {
    let mut app = App::new(fixture("gtf-nomatch"), Config::default()).unwrap();
    app.handle_key(ctrl('p'));
    for c in "qqq".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(app.editor.path.is_none());
}

#[test]
fn tick_honors_the_configured_autosave_delay() {
    let root = fixture("cfg-autosave");
    let (cfg, _) = crate::config::parse("autosave_seconds = 300\n");
    let mut app = App::new(root, cfg).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('Y')));
    app.last_edit = Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
    app.tick();
    assert!(app.editor.dirty); // 5s idle < the configured 300s
}

#[test]
fn tick_honors_the_configured_tree_refresh_interval() {
    let root = fixture("cfg-refresh");
    let (cfg, _) = crate::config::parse("tree_refresh_seconds = 300\n");
    let mut app = App::new(root.clone(), cfg).unwrap();
    fs::write(root.join("new.md"), "n\n").unwrap();
    app.last_tree_refresh = std::time::Instant::now() - std::time::Duration::from_secs(5);
    app.tick();
    // 5s < the configured 300s: no refresh yet
    assert!(!app.tree.rows().iter().any(|r| r.name == "new.md"));
}

#[test]
fn tick_idle_autosaves() {
    let root = fixture("tick");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('Y')));
    app.last_edit = Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
    app.tick();
    assert!(!app.editor.dirty);
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Yhello\nworld\n"
    );
}

#[test]
fn ctrl_q_quits_inside_newfile_prompt() {
    let mut app = App::new(fixture("prompt-quit-clean"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('n'))); // open NewFile prompt
    assert!(matches!(app.prompt, Prompt::NewFile(_)));
    app.handle_key(ctrl('q')); // Ctrl+Q should quit even inside prompt
    assert!(app.should_quit);
}

#[test]
fn ctrl_q_saves_and_quits_inside_search_prompt() {
    let root = fixture("prompt-quit-dirty");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(key(KeyCode::Char('Q'))); // make it dirty
    app.handle_key(ctrl('f')); // open Search prompt (from editor)
    assert!(matches!(app.prompt, Prompt::Search(_)));
    app.handle_key(ctrl('q')); // Ctrl+Q should save and quit
    assert!(app.should_quit);
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Qhello\nworld\n"
    );
}

#[test]
fn s_in_tree_opens_settings_on_the_current_theme() {
    let cfg = Config {
        theme_name: "mono".into(),
        ..Config::default()
    };
    let mut app = App::new(fixture("settings-open"), cfg).unwrap();
    app.handle_key(key(KeyCode::Char('s')));
    let Prompt::Settings { rows, selected } = &app.prompt else {
        panic!("expected Settings prompt");
    };
    assert_eq!(*selected, 0);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "theme");
    assert_eq!(rows[0].value(), "mono");
    assert_eq!(
        rows[0].choices,
        vec!["default", "light", "mono", "firmitas", "tokyonight"]
    );
}

#[test]
fn settings_l_and_h_cycle_the_theme_live_and_wrap() {
    let mut app = App::new(fixture("settings-cycle"), Config::default()).unwrap();
    assert!(
        app.config_dir.is_none(),
        "tests must never write the real config"
    );
    app.handle_key(key(KeyCode::Char('s')));
    app.handle_key(key(KeyCode::Char('l')));
    assert_eq!(app.theme, Theme::light());
    assert_eq!(app.config.theme_name, "light");
    assert_eq!(
        app.status.as_deref(),
        Some("theme: light (not saved: no config dir)")
    );

    app.handle_key(key(KeyCode::Right));
    assert_eq!(app.theme, Theme::mono());

    // wrap backwards from index 0
    app.handle_key(key(KeyCode::Char('h')));
    app.handle_key(key(KeyCode::Left));
    app.handle_key(key(KeyCode::Char('h')));
    assert_eq!(app.theme.name, "tokyonight");
    assert!(
        matches!(app.prompt, Prompt::Settings { .. }),
        "popup stays open while cycling"
    );
}

#[test]
fn settings_close_keys_and_editor_s_still_types() {
    let root = fixture("settings-close");
    let mut app = App::new(root, Config::default()).unwrap();
    for close in [KeyCode::Esc, KeyCode::Enter, KeyCode::Char('s')] {
        app.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(app.prompt, Prompt::Settings { .. }));
        app.handle_key(key(close));
        assert!(matches!(app.prompt, Prompt::None), "{close:?} should close");
    }
    // in the editor, s is a character
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(key(KeyCode::Char('s')));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(
        app.editor.lines()[0].starts_with('s'),
        "{:?}",
        app.editor.lines()[0]
    );
}

#[test]
fn settings_persists_the_choice_and_lists_user_themes_from_config_dir() {
    let root = fixture("settings-persist");
    let cfg_dir = root.parent().unwrap().join("xdg");
    std::fs::create_dir_all(cfg_dir.join("themes")).unwrap();
    std::fs::write(cfg_dir.join("themes/forest"), "heading1 = green+bold\n").unwrap();
    std::fs::write(cfg_dir.join("config"), "tree_width = 33\ntheme = default\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.config_dir = Some(cfg_dir.clone());

    app.handle_key(key(KeyCode::Char('s')));
    let Prompt::Settings { rows, .. } = &app.prompt else {
        panic!("expected Settings prompt");
    };
    assert_eq!(rows[0].choices.last().map(String::as_str), Some("forest"));

    app.handle_key(key(KeyCode::Char('h'))); // wraps to the last choice: forest
    assert_eq!(app.config.theme_name, "forest");
    assert_eq!(app.status.as_deref(), Some("theme: forest"));
    // the named file was applied (comparing struct fields keeps this file free of raw color literals)
    let mut expected = Theme::default();
    crate::theme::parse_overlay("heading1 = green+bold\n", &mut expected);
    assert_eq!(app.theme.heading1, expected.heading1);
    assert_eq!(
        std::fs::read_to_string(cfg_dir.join("config")).unwrap(),
        "tree_width = 33\ntheme = forest\n"
    );
}
