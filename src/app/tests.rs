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
    assert_eq!(app.editor().lines(), ["hello", "world"]);
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
    assert!(app.editor().dirty);
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('j'))); // b.md
    app.handle_key(key(KeyCode::Enter)); // open b.md -> autosaves a.md
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Xhello\nworld\n"
    );
    assert_eq!(app.editor().lines(), ["bee"]);
}

#[test]
fn ctrl_z_undoes_and_ctrl_y_redoes() {
    let mut app = App::new(fixture("undo"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('X')));
    assert_eq!(app.editor().lines()[0], "Xhello");
    app.handle_key(ctrl('z'));
    assert_eq!(app.editor().lines()[0], "hello");
    app.handle_key(ctrl('y'));
    assert_eq!(app.editor().lines()[0], "Xhello");
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
    assert_eq!(app.editor().cursor(), (1, 0));
}

#[test]
fn empty_search_repeats_last_search() {
    let mut app = App::new(fixture("repeat"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // hello / world: two 'l'-runs
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Char('l')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.editor().cursor(), (0, 2));
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Enter)); // empty -> repeat "l"
    assert_eq!(app.editor().cursor(), (0, 3));
}

#[test]
fn ctrl_g_jumps_to_the_next_match_of_the_last_search() {
    let mut app = App::new(fixture("next"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // hello / world
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Char('l')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.editor().cursor(), (0, 2));
    app.handle_key(ctrl('g'));
    assert_eq!(app.editor().cursor(), (0, 3));
    app.handle_key(ctrl('g'));
    assert_eq!(app.editor().cursor(), (1, 3)); // "world"
}

#[test]
fn ctrl_g_without_a_previous_search_shows_a_status_message() {
    let mut app = App::new(fixture("next-none"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('g'));
    assert_eq!(app.editor().cursor(), (0, 0)); // didn't move
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
    assert_eq!(app.tab().unwrap().search_highlight.as_deref(), Some("wor"));
}

#[test]
fn opening_a_file_clears_the_search_highlight() {
    let mut app = App::new(fixture("hl-clear"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // a.md
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Char('l')));
    app.handle_key(key(KeyCode::Enter));
    assert!(app.tab().unwrap().search_highlight.is_some());
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('j'))); // b.md
    app.handle_key(key(KeyCode::Enter));
    assert!(app.tab().unwrap().search_highlight.is_none());
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
    assert_eq!(app.editor().cursor(), (0, 6));
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
    assert_eq!(app.editor().lines()[0], "JKhello"); // typed, not moved
}

#[test]
fn alt_jk_jumps_by_paragraph() {
    let root = fixture("para");
    fs::write(root.join("a.md"), "one\n\ntwo\n\nthree\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT));
    let (row, _) = app.editor().cursor();
    assert!(row >= 1); // moved past the blank line
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT));
    assert_eq!(app.editor().cursor(), (0, 0));
}

#[test]
fn super_jk_jumps_to_line_end_and_start() {
    let mut app = App::new(fixture("linejump"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // hello
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SUPER));
    assert_eq!(app.editor().cursor(), (0, 5)); // end of "hello"
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::SUPER));
    assert_eq!(app.editor().cursor(), (0, 0));
}

#[test]
fn dash_dash_zero_expands_to_checkbox() {
    let mut app = App::new(fixture("expand0"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // "hello", cursor at (0,0)
    app.handle_key(key(KeyCode::Char('-')));
    app.handle_key(key(KeyCode::Char('-')));
    app.handle_key(key(KeyCode::Char('0')));
    assert_eq!(app.editor().lines()[0], "- [ ] hello");
    assert_eq!(app.editor().cursor(), (0, 6)); // ready to type the item
    assert!(app.editor().dirty);
}

#[test]
fn plain_zero_still_types_zero() {
    let mut app = App::new(fixture("zero"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('0')));
    assert_eq!(app.editor().lines()[0], "0hello");
}

#[test]
fn triple_dash_zero_does_not_expand() {
    let mut app = App::new(fixture("dashes"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    for c in "---0".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    assert_eq!(app.editor().lines()[0], "---0hello");
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
    assert_eq!(app.editor().lines()[0], "qphello");
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
    assert_eq!(app.editor().cursor(), (1, 7));
    // ...and Ctrl+G wraps around to the capitalized one
    app.handle_key(ctrl('g'));
    assert_eq!(app.editor().cursor(), (0, 0));
}

#[test]
fn ctrl_jk_move_by_word_without_deleting() {
    let root = fixture("word-motion");
    fs::write(root.join("a.md"), "alpha bravo charlie\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('j'));
    let (_, col1) = app.editor().cursor();
    assert!(col1 > 0); // advanced
    app.handle_key(ctrl('j'));
    let (_, col2) = app.editor().cursor();
    assert!(col2 > col1);
    app.handle_key(ctrl('k'));
    assert_eq!(app.editor().cursor(), (0, col1));
    // nothing was deleted (Ctrl+K used to be kill-to-end-of-line)
    assert_eq!(app.editor().lines()[0], "alpha bravo charlie");
    assert!(!app.editor().dirty);
}

#[test]
fn ctrl_d_checks_an_unchecked_checkbox() {
    let root = fixture("cb-check");
    fs::write(root.join("a.md"), "- [ ] milk\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor().lines()[0], "- [x] milk");
    assert_eq!(app.editor().cursor(), (0, 0)); // same width: cursor stays
    assert!(app.editor().dirty);
}

#[test]
fn ctrl_d_unchecks_a_checked_checkbox() {
    let root = fixture("cb-uncheck");
    fs::write(root.join("a.md"), "- [x] milk\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor().lines()[0], "- [ ] milk");
}

#[test]
fn ctrl_d_unchecks_uppercase_checked_checkbox() {
    let root = fixture("cb-uncheck-upper");
    fs::write(root.join("a.md"), "- [X] milk\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor().lines()[0], "- [ ] milk");
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
    assert_eq!(app.editor().lines(), ["alpha", "bravo", "- [ ] charlie"]);
}

#[test]
fn typing_with_no_file_open_is_ignored() {
    let root = fixture("no-file-typing");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(ctrl('b')); // hide tree -> editor focus, no file
    app.handle_key(key(KeyCode::Char('x')));
    assert!(app.tabs.is_empty()); // nothing typed into a phantom buffer
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
    assert_eq!(app.editor().lines()[0], "- [ ] milk");
}

#[test]
fn ctrl_d_prefixes_a_plain_line_and_shifts_the_cursor() {
    let root = fixture("cb-plain");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // "hello", cursor (0,0)
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor().lines()[0], "- [ ] hello");
    assert_eq!(app.editor().cursor(), (0, 6)); // still on the 'h'
}

#[test]
fn ctrl_d_preserves_indentation() {
    let root = fixture("cb-indent");
    fs::write(root.join("a.md"), "  - [ ] a\n    plain\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor().lines()[0], "  - [x] a");
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor().lines()[1], "    - [ ] plain");
}

#[test]
fn ctrl_d_on_an_empty_line_does_not_join_the_next_line() {
    let root = fixture("cb-empty");
    fs::write(root.join("a.md"), "\nworld\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor().lines(), ["- [ ] ", "world"]);
}

#[test]
fn ctrl_d_is_undoable() {
    let root = fixture("cb-undo");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // "hello"
    app.handle_key(ctrl('d'));
    assert_eq!(app.editor().lines()[0], "- [ ] hello");
    // the toggle is a delete + an insert, so two undo steps
    app.handle_key(ctrl('z'));
    app.handle_key(ctrl('z'));
    assert_eq!(app.editor().lines()[0], "hello");
}

#[test]
fn shift_tab_in_editor_returns_to_tree_without_typing() {
    let mut app = App::new(fixture("backtab"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md
    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert!(matches!(app.focus, Focus::Tree));
    assert_eq!(app.editor().lines(), ["hello", "world"]); // unchanged
    assert!(!app.editor().dirty);
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
    assert!(app.tabs.is_empty());
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
        app.editor().path.as_deref(),
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
        app.editor().path.as_deref(),
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
        app.editor().path.as_deref(),
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
        .editor()
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
    assert_eq!(app.editor().lines(), ["bee"]);
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
    assert_eq!(app.editor().lines()[0], "?hello");
}

#[test]
fn esc_closes_go_to_file_without_opening() {
    let mut app = App::new(fixture("gtf-esc"), Config::default()).unwrap();
    app.handle_key(ctrl('p'));
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(app.tabs.is_empty());
}

#[test]
fn ctrl_p_does_not_fire_inside_another_prompt() {
    let mut app = App::new(fixture("gtf-nested"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('n'))); // NewFile prompt
    app.handle_key(ctrl('p'));
    assert!(matches!(app.prompt, Prompt::NewFile { .. }));
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
    assert!(app.tabs.is_empty());
}

#[test]
fn tick_honors_the_configured_autosave_delay() {
    let root = fixture("cfg-autosave");
    let (cfg, _) = crate::config::parse("autosave_seconds = 300\n");
    let mut app = App::new(root, cfg).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('Y')));
    app.tab_mut().unwrap().last_edit =
        Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
    app.tick();
    assert!(app.editor().dirty); // 5s idle < the configured 300s
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
    app.tab_mut().unwrap().last_edit =
        Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
    app.tick();
    assert!(!app.editor().dirty);
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Yhello\nworld\n"
    );
}

#[test]
fn ctrl_q_quits_inside_newfile_prompt() {
    let mut app = App::new(fixture("prompt-quit-clean"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('n'))); // open NewFile prompt
    assert!(matches!(app.prompt, Prompt::NewFile { .. }));
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
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].name, "theme");
    assert_eq!(rows[0].value(), "mono");
    assert_eq!(
        rows[0].choices,
        vec![
            "default",
            "light",
            "mono",
            "firmitas",
            "tokyonight",
            "ember"
        ]
    );
}

#[test]
fn ctrl_s_in_tree_does_not_open_settings() {
    let mut app = App::new(fixture("settings-ctrl-s"), Config::default()).unwrap();
    app.handle_key(ctrl('s'));
    assert!(matches!(app.prompt, Prompt::None));
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

    // wrap backwards from index 0 (ember is last in BUILTINS order)
    app.handle_key(key(KeyCode::Char('h')));
    app.handle_key(key(KeyCode::Left));
    app.handle_key(key(KeyCode::Char('h')));
    assert_eq!(app.theme.name, "ember");
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
        app.editor().lines()[0].starts_with('s'),
        "{:?}",
        app.editor().lines()[0]
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

#[test]
fn settings_second_row_cycles_side_padding_and_persists() {
    let root = fixture("settings-padding");
    let cfg_dir = root.parent().unwrap().join("xdg");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(cfg_dir.join("config"), "theme = default\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.config_dir = Some(cfg_dir.clone());

    app.handle_key(key(KeyCode::Char('s')));
    let Prompt::Settings { rows, selected } = &app.prompt else {
        panic!("expected Settings prompt");
    };
    assert_eq!(*selected, 0);
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[1].name, "side_padding");
    assert_eq!(rows[1].value(), "1");
    assert_eq!(rows[1].choices.len(), 21);

    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('l')));
    assert_eq!(app.config.side_padding, 2);
    assert_eq!(app.status.as_deref(), Some("side_padding: 2"));
    assert_eq!(
        std::fs::read_to_string(cfg_dir.join("config")).unwrap(),
        "theme = default\nside_padding = 2\n"
    );

    // h wraps 0 → 20
    app.handle_key(key(KeyCode::Char('h')));
    app.handle_key(key(KeyCode::Char('h')));
    app.handle_key(key(KeyCode::Char('h')));
    assert_eq!(app.config.side_padding, 20);
}

#[test]
fn settings_cursor_rows_cycle_shape_blink_and_color_and_persist() {
    use crate::cursor::Shape;
    let root = fixture("settings-cursor");
    let cfg_dir = root.parent().unwrap().join("xdg");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(cfg_dir.join("config"), "cursor_color = #123abc\n").unwrap();
    let cfg = Config {
        cursor_color: "#123abc".to_string(),
        ..Config::default()
    };
    let mut app = App::new(root, cfg).unwrap();
    app.config_dir = Some(cfg_dir.clone());

    app.handle_key(key(KeyCode::Char('s')));
    let Prompt::Settings { rows, .. } = &app.prompt else {
        panic!("expected Settings prompt");
    };
    assert_eq!(rows[2].name, "cursor_shape");
    assert_eq!(rows[2].value(), "default");
    assert_eq!(rows[2].choices, ["default", "block", "bar", "underline"]);
    assert_eq!(rows[3].name, "cursor_blink");
    assert_eq!(rows[3].value(), "on");
    assert_eq!(rows[4].name, "cursor_color");
    // a hex from the file isn't in the named list: appended so the row
    // starts on it
    assert_eq!(rows[4].value(), "#123abc");
    assert_eq!(rows[4].choices.len(), crate::cursor::COLOR_NAMES.len() + 1);

    // row 2: default → block
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('l')));
    assert_eq!(app.config.cursor_shape, Shape::Block);
    assert_eq!(app.status.as_deref(), Some("cursor_shape: block"));
    // row 3: on → off
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('l')));
    assert!(!app.config.cursor_blink);
    // row 4 (the last; j clamps there): #123abc wraps → default
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('l')));
    assert_eq!(app.config.cursor_color, "default");
    app.handle_key(key(KeyCode::Char('l')));
    assert_eq!(app.config.cursor_color, "white");
    assert_eq!(
        std::fs::read_to_string(cfg_dir.join("config")).unwrap(),
        "cursor_color = white\ncursor_shape = block\ncursor_blink = off\n"
    );

    // the file round-trips through the parser
    let (reparsed, warnings) =
        crate::config::parse(&std::fs::read_to_string(cfg_dir.join("config")).unwrap());
    assert!(warnings.is_empty());
    assert_eq!(reparsed.cursor_shape, Shape::Block);
    assert!(!reparsed.cursor_blink);
    assert_eq!(reparsed.cursor_color, "white");
}

#[test]
fn settings_side_padding_without_config_dir_applies_but_reports_unsaved() {
    let mut app = App::new(fixture("settings-padding-nodir"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('s')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.config.side_padding, 0);
    assert_eq!(
        app.status.as_deref(),
        Some("side_padding: 0 (not saved: no config dir)")
    );
}

// ---- mouse ----------------------------------------------------------

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

fn mouse(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }
}
fn down(x: u16, y: u16) -> MouseEvent {
    mouse(MouseEventKind::Down(MouseButton::Left), x, y)
}
fn drag(x: u16, y: u16) -> MouseEvent {
    mouse(MouseEventKind::Drag(MouseButton::Left), x, y)
}
fn up(x: u16, y: u16) -> MouseEvent {
    mouse(MouseEventKind::Up(MouseButton::Left), x, y)
}

/// An app with a.md open and the pane rects a draw would have recorded:
/// tree rows at x 1..29, editor text at x 32..72, both on rows 1..9.
fn mouse_app(tag: &str) -> App {
    let mut app = App::new(fixture(tag), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // open a.md: "hello" / "world"
    app.tree_area = Some(Rect::new(1, 1, 28, 8));
    app.editor_area = Some(Rect::new(32, 1, 40, 8));
    app
}

#[test]
fn click_in_editor_lands_the_cursor() {
    let mut app = mouse_app("click");
    app.handle_key(key(KeyCode::Esc)); // focus the tree first
    app.handle_mouse(down(35, 2)); // "world", col 3
    app.handle_mouse(up(35, 2));
    assert!(matches!(app.focus, Focus::Editor));
    assert_eq!(app.editor().cursor(), (1, 3));
    // a plain click leaves no selection behind
    assert!(app.editor().selection_range().is_none());
    assert!(app.clipboard.is_none());
}

#[test]
fn click_past_the_end_of_a_line_snaps_to_its_end() {
    let mut app = mouse_app("click-end");
    app.handle_mouse(down(70, 1));
    app.handle_mouse(up(70, 1));
    assert_eq!(app.editor().cursor(), (0, 5));
}

#[test]
fn drag_selects_and_release_copies() {
    let mut app = mouse_app("drag");
    app.handle_mouse(down(33, 1)); // "hello" col 1
    app.handle_mouse(drag(34, 2)); // "world" col 2
    assert_eq!(app.editor().selection_range(), Some(((0, 1), (1, 2))));
    app.handle_mouse(up(34, 2));
    // the selection stays visible and its text is queued for the clipboard
    assert_eq!(app.editor().selection_range(), Some(((0, 1), (1, 2))));
    assert_eq!(app.clipboard.as_deref(), Some("ello\nwo"));
    assert_eq!(app.status.as_deref(), Some("copied to clipboard"));
}

#[test]
fn drag_into_the_tree_pane_keeps_selecting_editor_text() {
    let mut app = mouse_app("drag-tree");
    app.handle_mouse(down(36, 2)); // "world" col 4
                                   // wander left across the border into the tree pane, on row 1
    app.handle_mouse(drag(5, 1));
    // x clamps to the editor's left edge, so the anchor..cursor range is
    // (0,0)..(1,4) — the tree is untouched
    assert_eq!(app.editor().selection_range(), Some(((0, 0), (1, 4))));
    assert_eq!(app.tree.selected(), 0);
    assert!(matches!(app.focus, Focus::Editor));
    app.handle_mouse(up(5, 1));
    assert_eq!(app.clipboard.as_deref(), Some("hello\nworl"));
}

#[test]
fn drag_above_the_pane_scrolls_one_row_per_event() {
    let root = fixture("drag-scroll");
    let body: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    fs::write(root.join("a.md"), body).unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.tree_area = Some(Rect::new(1, 1, 28, 8));
    app.editor_area = Some(Rect::new(32, 1, 40, 8));
    app.tab_mut().unwrap().scroll = 10; // rows 10..18 visible
    app.handle_mouse(down(32, 4)); // line 13
    app.handle_mouse(drag(32, 0)); // above the pane → row 9
    assert_eq!(app.editor().cursor(), (9, 0));
    app.tab_mut().unwrap().scroll = 9;
    app.handle_mouse(drag(32, 0));
    assert_eq!(app.editor().cursor(), (8, 0));
    // and below it → the row just under the window
    app.handle_mouse(drag(32, 20));
    assert_eq!(app.editor().cursor(), (17, 0));
}

#[test]
fn click_in_the_tree_selects_and_opens_that_row() {
    let mut app = mouse_app("tree-click");
    assert_eq!(app.editor().lines(), ["hello", "world"]);
    app.handle_mouse(down(3, 2)); // second row: b.md
    app.handle_mouse(up(3, 2));
    assert_eq!(app.tree.selected(), 1);
    assert_eq!(app.editor().lines(), ["bee"]);
    assert!(matches!(app.focus, Focus::Editor));
}

#[test]
fn click_on_empty_tree_space_only_focuses_the_tree() {
    let mut app = mouse_app("tree-blank");
    app.handle_mouse(down(3, 7)); // below the two rows
    assert!(matches!(app.focus, Focus::Tree));
    assert_eq!(app.tree.selected(), 0);
    assert_eq!(app.editor().lines(), ["hello", "world"]);
}

#[test]
fn wheel_scrolls_the_view_three_rows_and_leaves_the_cursor() {
    let root = fixture("wheel");
    let body: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    fs::write(root.join("a.md"), body).unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.tree_area = Some(Rect::new(1, 1, 28, 8));
    app.editor_area = Some(Rect::new(32, 1, 40, 8));
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 40, 3));
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 40, 3));
    assert_eq!(app.editor().cursor(), (0, 0));
    assert_eq!(app.tab().unwrap().scroll, 6);
    assert!(!app.tab().unwrap().follow_cursor);
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 40, 3));
    assert_eq!(app.tab().unwrap().scroll, 3);
    // a key brings the view back to the cursor
    app.handle_key(key(KeyCode::Right));
    assert!(app.tab().unwrap().follow_cursor);
    // over the tree the wheel moves the tree selection instead
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 5, 3));
    assert_eq!(app.tree.selected(), 1); // clamped: only two rows
    assert_eq!(app.tab().unwrap().scroll, 3);
}

#[test]
fn clicking_the_bar_edge_markers_steps_one_tab() {
    let mut app = two_tabs("tabs-edges");
    fs::write(app.tree.root().join("c.md"), "sea\n").unwrap();
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('u')));
    app.handle_key(key(KeyCode::Char('G')));
    app.handle_key(key(KeyCode::Enter)); // a, b, c open; c active
    assert_eq!(app.active, 2);
    // a bar too narrow for all three: "‹ b.md ×  c.md ×  "
    let segs = crate::tab::layout_bar(&app.tab_titles(), app.active, 18);
    app.tab_bar = Some((Rect::new(31, 1, 18, 1), segs));
    app.handle_mouse(down(31, 1)); // ‹
    app.handle_mouse(up(31, 1));
    assert_eq!(app.active, 1);
    let segs = crate::tab::layout_bar(&app.tab_titles(), app.active, 18);
    app.tab_bar = Some((Rect::new(31, 1, 18, 1), segs));
    app.handle_mouse(down(31 + 17, 1)); // ›
    app.handle_mouse(up(31 + 17, 1));
    assert_eq!(app.active, 2);
}

#[test]
fn mouse_is_ignored_while_a_prompt_is_open() {
    let mut app = mouse_app("mouse-prompt");
    app.handle_key(ctrl('f'));
    app.handle_mouse(down(35, 2));
    app.handle_mouse(up(35, 2));
    assert!(matches!(app.prompt, Prompt::Search(_)));
    assert_eq!(app.editor().cursor(), (0, 0));
}

#[test]
fn mouse_does_nothing_without_recorded_pane_rects() {
    let mut app = App::new(fixture("mouse-norect"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_mouse(down(35, 2));
    app.handle_mouse(drag(36, 2));
    app.handle_mouse(up(36, 2));
    assert_eq!(app.editor().cursor(), (0, 0));
    assert!(app.editor().selection_range().is_none());
}

// ---- search highlight / Ctrl+W --------------------------------------

#[test]
fn typing_clears_the_search_highlight() {
    let mut app = App::new(fixture("hl-clear"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(ctrl('f'));
    for c in "wor".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.tab().unwrap().search_highlight.as_deref(), Some("wor"));
    // cursor motion keeps it
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.tab().unwrap().search_highlight.as_deref(), Some("wor"));
    // an edit drops it
    app.handle_key(key(KeyCode::Char('x')));
    assert!(app.tab().unwrap().search_highlight.is_none());
    // Ctrl+G brings it back for the next match
    app.handle_key(ctrl('g'));
    assert_eq!(app.tab().unwrap().search_highlight.as_deref(), Some("wor"));
}

#[test]
fn ctrl_w_saves_and_closes_the_file() {
    let root = fixture("close");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('X')));
    app.handle_key(ctrl('w'));
    assert!(app.tabs.is_empty());
    assert!(matches!(app.focus, Focus::Tree));
    assert_eq!(app.status.as_deref(), Some("closed a.md"));
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Xhello\nworld\n"
    );
    // typing now hits the no-file guard rather than a phantom buffer
    app.handle_key(key(KeyCode::Tab)); // reopen via the tree
    assert_eq!(app.editor().lines(), ["Xhello", "world"]);
}

#[test]
fn ctrl_w_with_a_disk_conflict_keeps_the_file_open() {
    let root = fixture("close-conflict");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Char('X')));
    // someone else writes the file with a newer mtime
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(root.join("a.md"), "other\n").unwrap();
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    // a write handle: Windows refuses set_modified on a read-only one
    fs::OpenOptions::new()
        .write(true)
        .open(root.join("a.md"))
        .unwrap()
        .set_modified(future)
        .unwrap();
    app.handle_key(ctrl('w'));
    assert!(!app.tabs.is_empty());
    assert!(app.editor().dirty);
    assert!(matches!(app.focus, Focus::Editor));
    assert!(app.status.as_deref().unwrap().contains("conflict"));
    assert_eq!(fs::read_to_string(root.join("a.md")).unwrap(), "other\n");
}

#[test]
fn ctrl_w_with_no_file_open_is_a_noop() {
    let mut app = App::new(fixture("close-none"), Config::default()).unwrap();
    app.handle_key(ctrl('w'));
    assert!(app.status.is_none());
    assert!(matches!(app.focus, Focus::Tree));
}

// ---- tabs -----------------------------------------------------------

fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

/// a.md and b.md open, in that order, b.md active.
fn two_tabs(tag: &str) -> App {
    let mut app = App::new(fixture(tag), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // a.md
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Enter)); // b.md
    app
}

#[test]
fn opening_a_second_file_adds_a_tab_and_keeps_the_first() {
    let app = two_tabs("tabs-open");
    assert_eq!(app.tab_titles(), ["a.md", "b.md"]);
    assert_eq!(app.active, 1);
    assert_eq!(app.editor().lines(), ["bee"]);
    assert_eq!(app.tabs[0].editor.lines(), ["hello", "world"]);
}

#[test]
fn reopening_an_open_file_switches_to_its_tab() {
    let mut app = two_tabs("tabs-reopen");
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('k')));
    app.handle_key(key(KeyCode::Enter)); // a.md again
    assert_eq!(app.tab_titles(), ["a.md", "b.md"]);
    assert_eq!(app.active, 0);
    assert!(matches!(app.focus, Focus::Editor));
}

#[test]
fn switching_tabs_autosaves_the_one_being_left() {
    let root = fixture("tabs-autosave");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // a.md
    app.handle_key(key(KeyCode::Char('X')));
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Enter)); // b.md
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Xhello\nworld\n"
    );
    assert!(!app.tabs[0].editor.dirty);
}

#[test]
fn new_tab_opens_right_of_the_active_one() {
    let mut app = two_tabs("tabs-insert");
    fs::write(app.tree.root().join("c.md"), "sea\n").unwrap();
    app.handle_key(alt('h')); // back to a.md
    assert_eq!(app.active, 0);
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('u'))); // pick up c.md
    app.handle_key(key(KeyCode::Char('G')));
    app.handle_key(key(KeyCode::Enter)); // c.md
    assert_eq!(app.tab_titles(), ["a.md", "c.md", "b.md"]);
    assert_eq!(app.active, 1);
}

#[test]
fn opt_h_and_l_cycle_tabs_and_wrap() {
    let mut app = two_tabs("tabs-cycle");
    app.handle_key(alt('l'));
    assert_eq!(app.active, 0); // wrapped
    app.handle_key(alt('l'));
    assert_eq!(app.active, 1);
    app.handle_key(alt('h'));
    assert_eq!(app.active, 0);
    // from the tree too
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(alt('l'));
    assert_eq!(app.active, 1);
    assert!(matches!(app.focus, Focus::Editor));
}

#[test]
fn opt_digit_jumps_to_that_tab() {
    let mut app = two_tabs("tabs-digit");
    app.handle_key(alt('1'));
    assert_eq!(app.active, 0);
    app.handle_key(alt('2'));
    assert_eq!(app.active, 1);
    app.handle_key(alt('5')); // no such tab
    assert_eq!(app.active, 1);
}

#[test]
fn plain_h_and_l_still_type_in_the_editor() {
    let mut app = two_tabs("tabs-plain");
    app.handle_key(key(KeyCode::Char('h')));
    app.handle_key(key(KeyCode::Char('l')));
    assert_eq!(app.editor().lines(), ["hlbee"]);
    assert_eq!(app.active, 1);
}

#[test]
fn ctrl_w_closes_the_active_tab_and_activates_the_left_neighbour() {
    let mut app = two_tabs("tabs-close");
    app.handle_key(ctrl('w'));
    assert_eq!(app.tab_titles(), ["a.md"]);
    assert_eq!(app.active, 0);
    assert!(matches!(app.focus, Focus::Editor));
    assert_eq!(app.status.as_deref(), Some("closed b.md"));
    app.handle_key(ctrl('w'));
    assert!(app.tabs.is_empty());
    assert!(matches!(app.focus, Focus::Tree));
}

#[test]
fn closing_the_first_tab_activates_the_new_first() {
    let mut app = two_tabs("tabs-close-first");
    app.handle_key(alt('1'));
    app.handle_key(ctrl('w'));
    assert_eq!(app.tab_titles(), ["b.md"]);
    assert_eq!(app.active, 0);
}

#[test]
fn clicking_a_tab_title_switches_and_the_cross_closes() {
    let mut app = two_tabs("tabs-click");
    // bar at x 31.., " a.md ×  b.md × ": a.md title 0..6, × 6..8, b.md 8..14, × 14..16
    let segs = crate::tab::layout_bar(&app.tab_titles(), app.active, 28);
    app.tab_bar = Some((Rect::new(31, 1, 28, 1), segs));
    app.editor_area = Some(Rect::new(31, 2, 28, 8));
    app.handle_mouse(down(33, 1));
    app.handle_mouse(up(33, 1));
    assert_eq!(app.active, 0);
    app.handle_mouse(down(31 + 14, 1)); // b.md's ×
    app.handle_mouse(up(31 + 14, 1));
    assert_eq!(app.tab_titles(), ["a.md"]);
    assert_eq!(app.active, 0);
    // a click on the bar never reaches the editor text below it
    assert!(app.editor().selection_range().is_none());
}

#[test]
fn deleting_a_file_open_in_another_tab_drops_that_tab() {
    let mut app = two_tabs("tabs-delete");
    // tree selection is on b.md; move to a.md and delete it
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('k')));
    app.handle_key(key(KeyCode::Char('x')));
    app.handle_key(key(KeyCode::Char('x')));
    assert_eq!(app.tab_titles(), ["b.md"]);
    assert_eq!(app.active, 0);
    assert_eq!(app.editor().lines(), ["bee"]);
}

#[test]
fn renaming_a_file_updates_its_tab_title() {
    let mut app = two_tabs("tabs-rename");
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('k'))); // a.md in the tree
    app.handle_key(key(KeyCode::Char('r')));
    for _ in 0..4 {
        app.handle_key(key(KeyCode::Backspace));
    }
    for c in "z.md".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.tab_titles(), ["z.md", "b.md"]);
}

#[test]
fn tick_autosaves_an_inactive_tab() {
    let root = fixture("tabs-tick");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter)); // a.md
    app.handle_key(key(KeyCode::Char('X')));
    // switch without the autosave: fake a stale timer on the tab
    app.tabs[0].editor.mark_dirty();
    app.tabs[0].last_edit = Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
    // an unrelated second tab, active
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Enter));
    // (the switch already saved a.md; dirty it again behind the scenes)
    app.tabs[0].editor.insert_str("Y");
    app.tabs[0].editor.mark_dirty();
    app.tabs[0].last_edit = Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
    app.tick();
    assert!(!app.tabs[0].editor.dirty);
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "XYhello\nworld\n"
    );
    assert_eq!(app.status.as_deref(), Some("saved a.md"));
}

#[test]
fn quit_saves_every_tab() {
    let root = fixture("tabs-quit");
    let mut app = two_tabs("tabs-quit");
    app.handle_key(key(KeyCode::Char('B'))); // b.md dirty
    app.tabs[0].editor.insert_str("A");
    app.tabs[0].editor.mark_dirty(); // a.md dirty behind the scenes
    app.handle_key(ctrl('q'));
    assert!(app.should_quit);
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "Ahello\nworld\n"
    );
    assert_eq!(fs::read_to_string(root.join("b.md")).unwrap(), "Bbee\n");
}

#[test]
fn search_highlight_is_per_tab() {
    let mut app = two_tabs("tabs-search");
    app.handle_key(ctrl('f'));
    app.handle_key(key(KeyCode::Char('e')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.tabs[1].search_highlight.as_deref(), Some("e"));
    assert!(app.tabs[0].search_highlight.is_none());
}

// ---- settings popup mouse -------------------------------------------

/// The settings popup open, with the geometry a draw would record:
/// popup at (10,3) 40x4, rows on y 4 and 5, ‹ at x 30, › at x 47.
fn settings_mouse_app(tag: &str) -> App {
    let mut app = App::new(fixture(tag), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('s')));
    app.settings_hits = Some(SettingsHits {
        popup: Rect::new(10, 3, 40, 4),
        rows: vec![
            SettingsRowHit {
                y: 4,
                prev_x: 30,
                next_x: 47,
            },
            SettingsRowHit {
                y: 5,
                prev_x: 30,
                next_x: 47,
            },
        ],
    });
    app
}

#[test]
fn clicking_a_settings_row_selects_it() {
    let mut app = settings_mouse_app("settings-click-row");
    app.handle_mouse(down(15, 5));
    match &app.prompt {
        Prompt::Settings { selected, .. } => assert_eq!(*selected, 1),
        _ => panic!("popup closed"),
    }
    // the theme is untouched by a plain row click
    assert_eq!(app.config.theme_name, "default");
}

#[test]
fn clicking_the_arrows_steps_the_value() {
    let mut app = settings_mouse_app("settings-click-arrows");
    app.handle_mouse(down(47, 4)); // › on the theme row
    assert_eq!(app.config.theme_name, "light");
    app.handle_mouse(down(30, 4)); // ‹
    assert_eq!(app.config.theme_name, "default");
    // › on the side_padding row selects it and steps it
    app.handle_mouse(down(47, 5));
    assert_eq!(app.config.side_padding, 2);
    match &app.prompt {
        Prompt::Settings { selected, .. } => assert_eq!(*selected, 1),
        _ => panic!("popup closed"),
    }
}

#[test]
fn clicking_outside_the_settings_popup_closes_it() {
    let mut app = settings_mouse_app("settings-click-outside");
    app.handle_mouse(down(2, 2));
    assert!(matches!(app.prompt, Prompt::None));
    // and that click does not fall through to the tree/editor
    assert_eq!(app.tree.selected(), 0);
}

#[test]
fn settings_popup_ignores_clicks_before_its_first_draw() {
    let mut app = App::new(fixture("settings-click-nodraw"), Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('s')));
    app.handle_mouse(down(2, 2));
    assert!(matches!(app.prompt, Prompt::Settings { .. }));
}

// ---- no file open: status-bar errors, never panics ------------------
//
// `ed` / `ed_ref` and the tab helpers used to index
// `tabs[active]` directly; with no file open that panicked. Now every
// such path reports `NO_FILE_OPEN` in the status bar instead.

/// The user-visible error every no-tab path must show.
const NO_FILE: &str = "no file open — pick one in the tree (Esc)";

#[test]
fn editor_helpers_report_no_tab_instead_of_panicking() {
    let mut app = App::new(fixture("no-tab-helpers"), Config::default()).unwrap();
    assert!(app.tabs.is_empty());
    assert!(app.ed().is_none());
    assert!(app.ed_ref().is_none());
    // the checkbox predicate answers instead of panicking
    assert!(!app.checkbox_trigger_armed());
    assert!(app.status.is_none());
}

#[test]
fn toggle_checkbox_with_no_file_open_errors() {
    let mut app = App::new(fixture("no-tab-checkbox"), Config::default()).unwrap();
    app.toggle_checkbox();
    assert!(app.tabs.is_empty());
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
}

#[test]
fn save_with_no_file_open_errors() {
    let mut app = App::new(fixture("no-tab-save"), Config::default()).unwrap();
    app.do_save();
    assert!(app.tabs.is_empty());
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
}

#[test]
fn search_with_no_file_open_errors_and_leaves_no_trace() {
    let mut app = App::new(fixture("no-tab-search"), Config::default()).unwrap();
    app.search_next("hello");
    assert!(app.tabs.is_empty());
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
    assert!(app.last_search.is_empty());
}

#[test]
fn note_edit_with_no_file_open_errors() {
    let mut app = App::new(fixture("no-tab-note"), Config::default()).unwrap();
    app.note_edit();
    assert!(app.tabs.is_empty());
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
}

#[test]
fn editor_hit_with_no_file_open_is_none() {
    let mut app = App::new(fixture("no-tab-hit"), Config::default()).unwrap();
    assert!(app.editor_hit(Rect::new(32, 1, 40, 8), 35, 2).is_none());
    assert!(app.status.is_none());
}

/// A click landing in a stale editor rect (recorded before the last
/// tab closed) errors instead of panicking.
#[test]
fn click_in_a_stale_editor_rect_with_no_file_open_errors() {
    let mut app = App::new(fixture("no-tab-click"), Config::default()).unwrap();
    app.editor_area = Some(Rect::new(32, 1, 40, 8));
    app.handle_mouse(down(35, 2));
    assert!(app.tabs.is_empty());
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
    // the release finds no drag in progress and does nothing
    app.handle_mouse(up(35, 2));
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
    assert!(app.clipboard.is_none());
}

/// A drag begun before the last tab closed errors on move and on
/// release instead of panicking.
#[test]
fn drag_and_release_with_no_file_open_error() {
    let mut app = App::new(fixture("no-tab-drag"), Config::default()).unwrap();
    app.editor_area = Some(Rect::new(32, 1, 40, 8));
    app.dragging = true;
    app.handle_mouse(drag(35, 2));
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
    app.dragging = true;
    app.handle_mouse(up(35, 2));
    assert!(!app.dragging);
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
    assert!(app.clipboard.is_none());
}

#[test]
fn wheel_over_a_stale_editor_rect_with_no_file_open_errors() {
    let mut app = App::new(fixture("no-tab-wheel"), Config::default()).unwrap();
    app.editor_area = Some(Rect::new(32, 1, 40, 8));
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 40, 3));
    assert!(app.tabs.is_empty());
    assert_eq!(app.status.as_deref(), Some(NO_FILE));
}

/// Task 3 (Ctrl+O follow-link) fixtures: a vault with a subdirectory,
/// so sibling-dir vs root fallback resolution is exercisable.
fn link_vault(tag: &str, a_content: &str) -> std::path::PathBuf {
    let owned = std::env::temp_dir().join(format!("mrkdup-link-{tag}"));
    let _ = fs::remove_dir_all(&owned);
    let root = owned.join("root");
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::create_dir_all(root.join("sib")).unwrap();
    fs::write(root.join("notes/a.md"), a_content).unwrap();
    fs::write(root.join("notes/b.md"), "# Target Head\nbody\n").unwrap();
    fs::write(root.join("shared.md"), "shared\n").unwrap();
    fs::write(root.join("sib/c.md"), "c\n").unwrap();
    // canonicalize: the tree (and hence resolution) sees the real path,
    // which on macOS differs from `temp_dir()` (`/var` → `/private/var`)
    std::fs::canonicalize(&root).unwrap_or(root)
}

/// Open `rel` (root-relative) in the app and park the cursor at
/// (row, char col of `needle` + `delta`), e.g. just inside a link.
fn open_at_link(app: &mut App, root: &std::path::Path, rel: &str, needle: &str, delta: usize) {
    app.open_file(root.join(rel));
    let line = app.editor().lines()[0].clone();
    let byte = line
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {line:?}"));
    let col = line[..byte].chars().count() + delta;
    assert!(app.tab_mut().unwrap().editor.set_cursor(0, col));
}

#[test]
fn ctrl_o_with_no_link_under_cursor_sets_status_and_keeps_text() {
    // Step 2 probe: Ctrl+O must reach app dispatch (textarea 0.9 binds
    // nothing to it), so with plain text under the cursor it reports
    // instead of typing or silently doing nothing.
    let root = link_vault("nolink", "see [[b]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("notes/a.md"));
    assert!(app.tab_mut().unwrap().editor.set_cursor(0, 0)); // on 's'
    app.handle_key(ctrl('o'));
    assert_eq!(app.status.as_deref(), Some("no link under cursor"));
    assert_eq!(app.editor().lines(), ["see [[b]]"]);
    assert_eq!(app.tabs.len(), 1); // opened nothing
}

#[test]
fn ctrl_o_follow_link_opens_sibling() {
    let root = link_vault("sibling", "see [[b]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/b.md").as_path())
    );
}

#[test]
fn ctrl_o_missing_link_offers_create_and_submit_creates() {
    let root = link_vault("missing", "see [[new]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert!(
        matches!(&app.prompt, Prompt::NewFile{input, ..} if input == "notes/new.md"),
        "unexpected prompt: {:?}",
        match &app.prompt {
            Prompt::NewFile { input, .. } => input.clone(),
            _ => "<not a NewFile prompt>".into(),
        }
    );
    assert!(app
        .status
        .as_deref()
        .is_some_and(|s| s.contains("no note 'new'") && s.contains("notes/new.md")));
    // the create offer is anchored at the vault root, not the tree
    // selection: park the selection on the `sib/` dir and the file
    // must still land in `notes/` where the prefill says
    assert!(app.tree.select_path(&root.join("sib")));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(root.join("notes/new.md").exists());
    assert!(!root.join("sib/notes/new.md").exists());
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/new.md").as_path())
    );
}

#[test]
fn ctrl_o_dotdot_link_offers_creatable_sibling_prefill() {
    // `[[../sib/new]]` from `notes/a.md`: the prefill is normalized to
    // `sib/new.md` (no `..` for `files::create` to reject) and the
    // submit creates exactly there
    let root = link_vault("dotdot", "see [[../sib/new]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert!(
        matches!(&app.prompt, Prompt::NewFile{input, ..} if input == "sib/new.md"),
        "unexpected prompt: {:?}",
        match &app.prompt {
            Prompt::NewFile { input, .. } => input.clone(),
            _ => "<not a NewFile prompt>".into(),
        }
    );
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.prompt, Prompt::None));
    assert!(root.join("sib/new.md").exists());
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sib/new.md").as_path())
    );
}

#[test]
fn ctrl_o_escaping_create_is_refused_without_a_prompt() {
    // `[[../../outside]]` from `notes/a.md` would land above the vault:
    // no prompt, just a status (creating files outside the vault behind
    // a link-follow would be a surprise)
    let root = link_vault("escape", "see [[../../outside]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert!(matches!(app.prompt, Prompt::None));
    assert_eq!(
        app.status.as_deref(),
        Some("can't create '../../outside' outside the vault")
    );
    assert_eq!(app.tabs.len(), 1);
}

#[test]
fn ctrl_o_absolute_target_anchors_at_root() {
    // `[[/shared]]` opens the vault-root note, never the filesystem
    // absolute (resolving it outside the vault would break the
    // vault-is-the-truth model `follow_md_url` already honors)
    let root = link_vault("abswiki", "see [[/shared]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("shared.md").as_path())
    );
}

#[test]
fn ctrl_o_dotdot_follow_stores_one_normalized_tab() {
    // following `[[../sib/c]]` twice must switch to the same tab, not
    // stack `notes/../sib/c.md` next to `sib/c.md`
    let root = link_vault("dedup", "see [[../sib/c]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sib/c.md").as_path())
    );
    app.open_file(root.join("notes/a.md")); // already open: switches back
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sib/c.md").as_path())
    );
}

#[test]
fn ctrl_o_alias_link_opens_target() {
    let root = link_vault("alias", "see [[b|Bee]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "Bee", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/b.md").as_path())
    );
}

#[test]
fn ctrl_o_heading_link_opens_target_at_heading_row() {
    let root = link_vault("heading", "see [[b#Target Head]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/b.md").as_path())
    );
    assert_eq!(app.editor().cursor(), (0, 0)); // the `# Target Head` line
}

#[test]
fn ctrl_o_md_link_follows_relative_and_refuses_remote() {
    let root = link_vault("mdlink", "see [t](../sib/c.md)\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "../sib/c.md", 1);
    app.handle_key(ctrl('o'));
    // `..` resolves to a normalized path, so it compares directly
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sib/c.md").as_path())
    );
    // remote urls report instead of opening anything
    let root = link_vault("mdremote", "see [t](https://x)\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "https://x", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(app.status.as_deref(), Some("not a local file — https://x"));
    assert_eq!(app.tabs.len(), 1);
}

#[test]
fn ctrl_o_root_fallback_link_opens_root_file() {
    let root = link_vault("rootfb", "see [[shared]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("shared.md").as_path())
    );
}

#[test]
fn ctrl_o_note_name_with_a_dot_opens_its_md_file() {
    let root = link_vault("dotname", "see [[v1.2]]\n");
    fs::write(root.join("notes/v1.2.md"), "release\n").unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/v1.2.md").as_path())
    );
}

/// Task 4 (Ctrl+L backlinks) fixture: `a.md` and `sub/c.md` link
/// `[[b]]`, `d.md` links elsewhere (so it is lonely), and `b.md`
/// self-links (so the popup must exclude the current file).
fn backlink_vault(tag: &str) -> std::path::PathBuf {
    let owned = std::env::temp_dir().join(format!("mrkdup-backlink-{tag}"));
    let _ = fs::remove_dir_all(&owned);
    let root = owned.join("root");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("a.md"), "see [[b]]\n").unwrap();
    fs::write(root.join("sub/c.md"), "see [[b#H]]\n").unwrap();
    fs::write(root.join("d.md"), "see [[other]]\n").unwrap();
    fs::write(root.join("b.md"), "self [[b]]\n").unwrap();
    // canonicalize: the tree (and hence the scan) sees the real path,
    // which on macOS differs from `temp_dir()` (`/var` → `/private/var`)
    std::fs::canonicalize(&root).unwrap_or(root)
}

#[test]
fn ctrl_l_backlinks_popup_lists_sorted_linkers_excluding_current_file() {
    let root = backlink_vault("list");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("b.md"));
    app.handle_key(ctrl('l'));
    let Prompt::GoToFile {
        title,
        input,
        candidates,
        selected,
    } = &app.prompt
    else {
        panic!("expected the picker, got status {:?}", app.status);
    };
    assert_eq!(title, " links to b (2) ");
    assert_eq!(input, "");
    assert_eq!(*selected, 0);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].0, "a.md");
    assert_eq!(candidates[0].1, root.join("a.md"));
    assert_eq!(candidates[1].0, "sub/c.md");
    assert_eq!(candidates[1].1, root.join("sub/c.md"));
}

#[test]
fn ctrl_l_backlinks_enter_opens_candidate_and_esc_closes() {
    let root = backlink_vault("openesc");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    // Enter on the first candidate switches to its tab and clears it
    app.open_file(root.join("b.md"));
    app.handle_key(ctrl('l'));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.prompt, Prompt::None));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("a.md").as_path())
    );
    // ↓ moves down; Enter opens the second hit
    app.open_file(root.join("b.md")); // already open: just switches back
    app.handle_key(ctrl('l'));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.prompt, Prompt::None));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sub/c.md").as_path())
    );
    // Esc closes the popup without switching
    app.open_file(root.join("b.md"));
    app.handle_key(ctrl('l'));
    assert!(matches!(app.prompt, Prompt::GoToFile { .. }));
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.prompt, Prompt::None));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("b.md").as_path())
    );
}

#[test]
fn ctrl_l_backlinks_typing_filters_like_go_to_file() {
    let root = backlink_vault("filter");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("b.md"));
    app.handle_key(ctrl('l'));
    app.handle_key(key(KeyCode::Char('c')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sub/c.md").as_path())
    );
}

#[test]
fn ctrl_l_backlinks_lonely_file_sets_status_and_no_popup() {
    let root = backlink_vault("lonely");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("d.md"));
    app.handle_key(ctrl('l'));
    assert!(matches!(app.prompt, Prompt::None));
    assert_eq!(app.status.as_deref(), Some("no links to 'd' yet"));
}

#[test]
fn ctrl_l_backlinks_no_open_file_sets_status_without_panic() {
    let root = backlink_vault("nofile");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.focus = Focus::Editor; // welcome page covers the editor pane
    app.handle_key(ctrl('l'));
    assert!(matches!(app.prompt, Prompt::None));
    assert_eq!(app.status.as_deref(), Some(NO_FILE_OPEN));
}

#[cfg(unix)]
#[test]
fn ctrl_o_on_an_alias_of_an_open_file_switches_tab_instead_of_duplicating() {
    // `notes/alias.md` is a symlink to `notes/b.md`; following
    // `[[alias]]` while b.md is open must land in b.md's tab, not open a
    // second buffer of the same file (two buffers race on autosave)
    let root = link_vault("alias-tab", "see [[alias]]\n");
    std::os::unix::fs::symlink(root.join("notes/b.md"), root.join("notes/alias.md")).unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("notes/b.md"));
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    assert_eq!(app.tabs.len(), 2);
    app.handle_key(ctrl('o'));
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/b.md").as_path())
    );
}

#[test]
fn ctrl_o_heading_link_to_an_unopenable_file_keeps_the_source_cursor() {
    // `notes/bin` exists but is not UTF-8, so the open fails; the heading
    // jump must not then run against the still-active source file (which
    // mentions "intro" in its body) nor replace the open-failed status
    let root = link_vault("badopen", "intro here, see [[bin#Intro]]\n");
    fs::write(root.join("notes/bin"), [0xffu8, 0xfe, 0x00, 0x01]).unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    let before = app.editor().cursor();
    app.handle_key(ctrl('o'));
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.editor().cursor(), before);
    assert!(
        app.status
            .as_deref()
            .is_some_and(|s| s.starts_with("open failed")),
        "status: {:?}",
        app.status
    );
}

#[test]
fn ctrl_o_heading_link_skips_body_text_that_mentions_the_heading() {
    let root = link_vault("headbody", "see [[b#Second]]\n");
    fs::write(
        root.join("notes/b.md"),
        "# First\nthe second part is below\n## Second\nbody\n",
    )
    .unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(app.editor().cursor(), (2, 0)); // the `## Second` line
    assert_eq!(app.status, None);
}

#[test]
fn ctrl_o_heading_link_into_a_scrolled_tab_follows_the_cursor_again() {
    let root = link_vault("headscroll", "see [[b#Target Head]]\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    app.open_file(root.join("notes/b.md"));
    app.tab_mut().unwrap().follow_cursor = false; // as a wheel scroll leaves it
    open_at_link(&mut app, &root, "notes/a.md", "[[", 2);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/b.md").as_path())
    );
    assert!(app.tab().unwrap().follow_cursor);
    assert_eq!(app.editor().cursor(), (0, 0));
}

#[test]
fn ctrl_o_md_link_with_fragment_opens_and_jumps_to_the_heading() {
    let root = link_vault("mdfrag", "see [t](../sib/c.md#sea-side)\n");
    fs::write(root.join("sib/c.md"), "intro\n## Sea Side\nbody\n").unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "../sib", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("sib/c.md").as_path())
    );
    assert_eq!(app.editor().cursor(), (1, 0));
}

#[test]
fn ctrl_o_md_link_decodes_percent_escapes() {
    let root = link_vault("mdpct", "see [t](my%20note.md)\n");
    fs::write(root.join("notes/my note.md"), "spaced\n").unwrap();
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "my%20", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.tab().unwrap().editor.path.as_deref(),
        Some(root.join("notes/my note.md").as_path())
    );
}

#[test]
fn ctrl_o_fragment_only_link_jumps_within_the_file() {
    let root = link_vault("mdanchor", "see [t](#below)\nfiller\n## Below\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "#below", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.editor().cursor(), (2, 0));
}

#[test]
fn ctrl_o_refuses_any_remote_scheme() {
    let root = link_vault("mdftp", "see [t](ftp://x/y.md)\n");
    let mut app = App::new(root.clone(), Config::default()).unwrap();
    open_at_link(&mut app, &root, "notes/a.md", "ftp://", 1);
    app.handle_key(ctrl('o'));
    assert_eq!(
        app.status.as_deref(),
        Some("not a local file — ftp://x/y.md")
    );
    assert_eq!(app.tabs.len(), 1);
}
