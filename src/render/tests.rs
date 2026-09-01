use crate::app::{App, Focus};
use crate::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Modifier;
use ratatui::{backend::TestBackend, Terminal};
use std::fs;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn draw_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| crate::ui::draw(f, app)).unwrap();
    format!("{:?}", terminal.backend().buffer())
}

fn fixture(tag: &str, content: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("mrkdup-render-{tag}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), content).unwrap();
    root
}

#[test]
fn headings_render_in_color() {
    let root = fixture("heading", "# Title\nplain text\n");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    let text = draw_to_string(&mut app);
    assert!(text.contains("Title"));
    assert!(text.contains("Cyan"), "heading color missing: {text}");
}

#[test]
fn search_matches_render_with_yellow_background() {
    let root = fixture("search", "alpha\nbravo alpha\n");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    for c in "alpha".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    let text = draw_to_string(&mut app);
    assert!(text.contains("Yellow"), "search bg missing: {text}");
}

#[test]
fn light_theme_headings_render_blue_not_cyan() {
    let root = fixture("heading-light", "# Title\nplain text\n");
    let cfg = Config {
        theme_name: "light".into(),
        ..Config::default()
    };
    let mut app = App::new(root, cfg).unwrap();
    app.handle_key(key(KeyCode::Enter));
    let text = draw_to_string(&mut app);
    assert!(text.contains("Title"));
    assert!(text.contains("Blue"), "heading color missing: {text}");
}

#[test]
fn mono_theme_headings_render_with_no_color_but_bold() {
    let root = fixture("heading-mono", "# Title\nplain text\n");
    let cfg = Config {
        theme_name: "mono".into(),
        ..Config::default()
    };
    let mut app = App::new(root, cfg).unwrap();
    app.handle_key(key(KeyCode::Enter));
    let text = draw_to_string(&mut app);
    assert!(text.contains("Title"));
    assert!(text.contains("BOLD"), "heading bold missing: {text}");
    for c in ["Cyan", "Green", "Yellow", "Blue", "Magenta", "Red"] {
        assert!(!text.contains(c), "mono theme leaked color {c}: {text}");
    }
}

#[test]
fn mono_theme_search_match_renders_reversed_not_yellow() {
    let root = fixture("search-mono", "alpha\nbravo alpha\n");
    let cfg = Config {
        theme_name: "mono".into(),
        ..Config::default()
    };
    let mut app = App::new(root, cfg).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    for c in "alpha".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    let text = draw_to_string(&mut app);
    assert!(text.contains("REVERSED"), "search overlay missing: {text}");
    assert!(
        !text.contains("Yellow"),
        "mono search leaked Yellow: {text}"
    );
}

#[test]
fn selection_renders_reversed() {
    let root = fixture("selection", "abcdef\n");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    let text = draw_to_string(&mut app);
    assert!(text.contains("REVERSED"), "selection missing: {text}");
}

#[test]
fn long_lines_soft_wrap_in_the_renderer() {
    // editor inner width ~ 42 cols after tree + margins on an 80-col
    // screen; a 60-char word-free line must produce a second row
    let long = "x".repeat(60);
    let root = fixture("wrap", &format!("{long}\n"));
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    let text = draw_to_string(&mut app);
    // the full line can't fit on one row, so some x-run appears twice
    let rows_with_x = text.lines().filter(|l| l.contains("xxxxx")).count();
    assert!(rows_with_x >= 2, "expected wrapped rows: {text}");
}

#[test]
fn styling_stays_correct_across_a_soft_wrap_boundary() {
    // One long logical line, no spaces, so wrap.rs hard-breaks it
    // into several rows of a fixed column count. The `**Z**` / `*W*`
    // / `.` pattern repeats on a period of 9 chars, which the pane's
    // wrap width (~42-44 cols) doesn't evenly divide, so at least
    // one row boundary is guaranteed to land inside a Bold or
    // Italic span -- exactly the case an off-by-one in the D2 span
    // cursor (reset-on-line-change, forward-only advance) would
    // paint with a stale or wrong style.
    let unit = "**Z***W*.";
    let content = unit.repeat(30);
    let root = fixture("wrap-styles", &format!("{content}\n"));
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));

    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
    let buf = terminal.backend().buffer();

    // Walk the editor pane (x >= tree_width, y above the status
    // line) in on-screen order: top-to-bottom, left-to-right. That
    // is the same order the sentinel characters appear in the
    // source line, across however many wrapped rows it takes, so
    // the extracted sequence can be compared directly against the
    // source without knowing the exact wrap width.
    let mut seen: Vec<(char, bool, bool)> = Vec::new(); // (char, bold, italic)
    let mut rows_seen = std::collections::HashSet::new();
    for y in 0..15u16 {
        for x in 30..80u16 {
            if let Some(cell) = buf.cell((x, y)) {
                let sym = cell.symbol();
                if sym == "Z" || sym == "W" || sym == "." {
                    let ch = sym.chars().next().unwrap();
                    let bold = cell.modifier.contains(Modifier::BOLD);
                    let italic = cell.modifier.contains(Modifier::ITALIC);
                    seen.push((ch, bold, italic));
                    rows_seen.insert(y);
                }
            }
        }
    }

    assert!(
        rows_seen.len() >= 2,
        "expected the line to wrap across multiple rows, saw rows: {rows_seen:?}"
    );

    let expected: Vec<(char, bool, bool)> = content
        .chars()
        .filter(|&c| c == 'Z' || c == 'W' || c == '.')
        .map(|c| match c {
            'Z' => ('Z', true, false),
            'W' => ('W', false, true),
            _ => ('.', false, false),
        })
        .collect();

    assert_eq!(
        seen, expected,
        "styling diverged from source order across a wrap boundary"
    );
}

#[test]
fn painting_twice_without_an_edit_reuses_the_layout_cache() {
    let root = fixture("cache", "# Title\nplain text\n");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    draw_to_string(&mut app);
    assert_eq!(app.editor.layout_recomputes(), 1);
    // a second paint with nothing changed (no edit, no resize) must
    // not redo wrap+highlight
    draw_to_string(&mut app);
    assert_eq!(app.editor.layout_recomputes(), 1);
    // an actual edit does invalidate and recompute
    app.handle_key(key(KeyCode::Char('!')));
    draw_to_string(&mut app);
    assert_eq!(app.editor.layout_recomputes(), 2);
}

#[test]
fn cursor_tracks_into_scrolled_view() {
    let content: String = (0..200).map(|i| format!("line {i}\n")).collect();
    let root = fixture("scroll", &content);
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Enter));
    // jump far down via repeated paragraph-ish moves
    for _ in 0..150 {
        app.handle_key(key(KeyCode::Down));
    }
    assert!(matches!(app.focus, Focus::Editor));
    let text = draw_to_string(&mut app);
    assert!(text.contains("line 150"), "cursor line not visible: {text}");
    assert!(app.editor_scroll > 0);
}
