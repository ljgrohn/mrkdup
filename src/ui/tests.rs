use super::*;
use crate::app::App;
use crate::config::Config;
use ratatui::{backend::TestBackend, Terminal};
use std::fs;

#[test]
fn draws_tree_and_wrapped_editor() {
    let root = std::env::temp_dir().join("mrkdup-ui-1");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "one two three four five six seven\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("a.md")); // tree row
    assert!(text.contains("one two")); // editor content
    assert!(text.contains("1:1")); // status bar cursor
    assert!(text.contains("EDIT")); // focus tag after opening a file

    // back to the tree: tag flips
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    ));
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("TREE"));
}

#[test]
fn new_file_prompt_renders_as_popup() {
    let root = std::env::temp_dir().join("mrkdup-ui-2");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "x\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('n'),
        crossterm::event::KeyModifiers::NONE,
    ));
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('z'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("New file")); // popup title
    assert!(text.contains("z")); // typed input shown in the popup
}

#[test]
fn search_prompt_renders_as_popup_with_hint_in_status_bar() {
    let root = std::env::temp_dir().join("mrkdup-ui-search");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "hello world\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    let key = |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
    app.handle_key(key(crossterm::event::KeyCode::Enter)); // open a.md
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('f'),
        crossterm::event::KeyModifiers::CONTROL,
    ));
    app.handle_key(key(crossterm::event::KeyCode::Char('w')));
    app.handle_key(key(crossterm::event::KeyCode::Char('o')));
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("Search")); // popup title
    assert!(text.contains("wo")); // typed query shown in the popup
    assert!(text.contains("Enter jump")); // status bar shows hints…
    assert!(!text.contains("search: wo")); // …not the old inline query
}

#[test]
fn rename_prompt_renders_as_prefilled_popup_with_hint_in_status_bar() {
    let root = std::env::temp_dir().join("mrkdup-ui-rename");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "x\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('r'),
        crossterm::event::KeyModifiers::SHIFT,
    ));
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("Rename")); // popup title
    assert!(text.contains("Enter rename")); // status bar hint
}

#[test]
fn go_to_file_popup_lists_filtered_results() {
    let root = std::env::temp_dir().join("mrkdup-ui-gtf");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("apple.md"), "a\n").unwrap();
    fs::write(root.join("banana.md"), "b\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    let key = |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
    let ctrl = |c| {
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(c),
            crossterm::event::KeyModifiers::CONTROL,
        )
    };
    app.handle_key(ctrl('b')); // hide the tree so its rows don't alias
    app.handle_key(ctrl('p'));
    for c in "ban".chars() {
        app.handle_key(key(crossterm::event::KeyCode::Char(c)));
    }
    // +2 over the tight 60 columns the status line needs: the default
    // 1-column side padding would otherwise clip "Enter open" off the
    // right edge.
    let backend = TestBackend::new(62, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("Go to file")); // popup title
    assert!(text.contains("ban")); // typed query
    assert!(text.contains("banana.md")); // matching result listed
    assert!(!text.contains("apple.md")); // filtered out
    assert!(text.contains("Enter open")); // status bar hint
}

#[test]
fn status_bar_shows_word_count_only_when_a_file_is_open() {
    let root = std::env::temp_dir().join("mrkdup-ui-wc");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "one two three\nfour\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();

    // no file open yet: no word count
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(!text.contains("words"));

    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("4 words"));
}

#[test]
fn welcome_pane_shows_key_cheat_sheet_until_a_file_opens() {
    // root deliberately not named "mrkdup-…" so the app-name assert
    // can't be satisfied by the tree title
    let root = std::env::temp_dir().join("welcome-pane-fx");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "hello\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    // at launch (no file open): the cheat sheet is showing
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("mrkdup")); // app name
    assert!(text.contains("go to file"));
    assert!(text.contains("rename"));
    assert!(text.contains("quit"));

    // open a file: the cheat sheet is gone, content shows
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(!text.contains("go to file"));
    assert!(!text.contains("quit"));
    assert!(text.contains("hello"));
}

#[test]
fn help_overlay_shows_the_full_key_list_over_an_open_file() {
    let root = std::env::temp_dir().join("help-overlay-fx");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "hello\n").unwrap();
    let mut app = App::new(root, Config::default()).unwrap();
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    // the launch page lists the hidden-files toggle and the help key
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("hidden files"));
    assert!(text.contains("help"));

    let press = |app: &mut App, code: crossterm::event::KeyCode| {
        app.handle_key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    };
    press(&mut app, crossterm::event::KeyCode::Enter); // open a.md
    press(&mut app, crossterm::event::KeyCode::Esc); // focus tree
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(!text.contains("hidden files"));

    press(&mut app, crossterm::event::KeyCode::Char('?'));
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("hidden files"));
    assert!(text.contains("go to file"));
    assert!(text.contains("any key closes"));

    press(&mut app, crossterm::event::KeyCode::Esc);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(!text.contains("hidden files"));
}

#[test]
fn open_marker_falls_back_to_collapsed_ancestor() {
    use crate::tree::Row;
    use std::path::PathBuf;
    let row = |path: &str, is_dir: bool| Row {
        path: PathBuf::from(path),
        name: String::new(),
        depth: 0,
        is_dir,
        expanded: false,
    };
    let open = PathBuf::from("/r/docs/sub/a.md");
    // file visible -> its own row wins
    let rows = vec![row("/r/docs", true), row("/r/docs/sub/a.md", false)];
    assert_eq!(open_marker_index(&rows, Some(&open)), Some(1));
    // file hidden -> deepest visible ancestor dir
    let rows = vec![
        row("/r/docs", true),
        row("/r/docs/sub", true),
        row("/r/other", true),
    ];
    assert_eq!(open_marker_index(&rows, Some(&open)), Some(1));
    // nothing open -> no marker
    assert_eq!(open_marker_index(&rows, None), None);
}

#[test]
fn editor_text_gets_side_and_top_margins() {
    let r = with_side_margins(ratatui::layout::Rect::new(10, 0, 100, 100), 5, 3);
    assert_eq!(r.x, 15); // 5% of 100 = 5 cols in
    assert_eq!(r.width, 90); // 5 off each side
    assert_eq!(r.y, 3); // 3% of 100 = 3 rows down
    assert_eq!(r.height, 97); // trimmed from the top only
    let tiny = with_side_margins(ratatui::layout::Rect::new(0, 0, 3, 5), 5, 3);
    assert_eq!(tiny.width, 3); // tiny pane: percentages round to 0, no underflow
    assert_eq!(tiny.height, 5);
    // configured percentages apply; 0 means no margin at all
    let wide = with_side_margins(ratatui::layout::Rect::new(0, 0, 100, 100), 20, 10);
    assert_eq!((wide.x, wide.width, wide.y, wide.height), (20, 60, 10, 90));
    let none = with_side_margins(ratatui::layout::Rect::new(0, 0, 100, 100), 0, 0);
    assert_eq!(none, ratatui::layout::Rect::new(0, 0, 100, 100));
}

#[test]
fn tree_pane_width_comes_from_config() {
    let root = std::env::temp_dir().join("mrkdup-ui-cfgwidth");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "x\n").unwrap();
    let (cfg, _) = crate::config::parse("tree_width = 20\n");
    let mut app = App::new(root, cfg).unwrap();
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let buf = terminal.backend().buffer();
    // tree occupies columns 0..20 inside the default 1-column side
    // padding: its top-right corner sits at x=20 and the editor block
    // starts at x=21 (default tree_width would put these at 29/30)
    assert_eq!(buf.cell((20u16, 0u16)).unwrap().symbol(), "┐");
    assert_eq!(buf.cell((21u16, 0u16)).unwrap().symbol(), "┌");
}

#[test]
fn draw_records_the_pane_rects_for_mouse_hit_testing() {
    let root = std::env::temp_dir().join("mrkdup-ui-rects");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "x\n").unwrap();
    let config = Config {
        side_padding: 0,
        side_margin_percent: 0,
        top_margin_percent: 0,
        ..Config::default()
    };
    let mut app = App::new(root, config).unwrap();
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();

    // welcome page: the tree rect is known, the editor's is not
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    assert_eq!(app.tree_area, Some(Rect::new(1, 1, 28, 9)));
    assert_eq!(app.editor_area, None);

    // a file open: the editor's text rect sits inside its border
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    assert_eq!(app.tree_area, Some(Rect::new(1, 1, 28, 9)));
    assert_eq!(app.editor_area, Some(Rect::new(31, 1, 28, 9)));

    // hiding the tree drops its rect and widens the editor's
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('b'),
        crossterm::event::KeyModifiers::CONTROL,
    ));
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    assert_eq!(app.tree_area, None);
    assert_eq!(app.editor_area, Some(Rect::new(1, 1, 58, 9)));
}
