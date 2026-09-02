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

// Wider than `draw_to_string`'s fixed 80 columns: the Settings status
// line ("... — theme: light (not saved: no config dir)") runs past 80
// cols and draw_status doesn't wrap, so an 80-col buffer truncates it.
// A dedicated width keeps that assertion honest without touching the
// shared 80-col helper, which `long_lines_soft_wrap_in_the_renderer`
// depends on for its wrap-width assumption.
fn draw_to_string_wide(app: &mut App) -> String {
    let backend = TestBackend::new(110, 16);
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
    // from row 2: row 1 is the tab bar, whose file name has a '.'
    for y in 2..15u16 {
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
    assert_eq!(app.editor().layout_recomputes(), 1);
    // a second paint with nothing changed (no edit, no resize) must
    // not redo wrap+highlight
    draw_to_string(&mut app);
    assert_eq!(app.editor().layout_recomputes(), 1);
    // an actual edit does invalidate and recompute
    app.handle_key(key(KeyCode::Char('!')));
    draw_to_string(&mut app);
    assert_eq!(app.editor().layout_recomputes(), 2);
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
    assert!(app.tab().unwrap().scroll > 0);
}

#[test]
fn settings_popup_renders_the_theme_row_and_status() {
    let root = fixture("settings-popup", "# Title\n");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('s')));
    let text = draw_to_string(&mut app);
    assert!(text.contains("Settings"), "title missing: {text}");
    assert!(text.contains("theme"), "row name missing: {text}");
    assert!(text.contains("‹ default ›"), "value missing: {text}");

    app.handle_key(key(KeyCode::Char('l')));
    // wide: the status hint below runs past 80 cols once the
    // "(not saved: no config dir)" suffix is appended
    let text = draw_to_string_wide(&mut app);
    assert!(text.contains("‹ light ›"), "cycled value missing: {text}");
    assert!(
        text.contains("theme: light (not saved: no config dir)"),
        "status missing from hint line: {text}"
    );
}

fn draw_buffer(app: &mut App) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| crate::ui::draw(f, app)).unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn side_padding_insets_the_panes_from_the_terminal_edges() {
    let root = fixture("side-padding", "# Title\n");
    let cfg = Config {
        side_padding: 3,
        ..Config::default()
    };
    let mut app = App::new(root, cfg).unwrap();
    let buf = draw_buffer(&mut app);
    for x in 0..3u16 {
        assert_eq!(buf[(x, 0)].symbol(), " ", "left padding column {x}");
        assert_eq!(
            buf[(79 - x, 0)].symbol(),
            " ",
            "right padding column {}",
            79 - x
        );
    }
    assert_ne!(
        buf[(3, 0)].symbol(),
        " ",
        "tree border should start at column 3"
    );
    assert_ne!(
        buf[(76, 0)].symbol(),
        " ",
        "editor border should end at column 76"
    );

    // default is one column
    let mut app = App::new(
        fixture("side-padding-default", "# Title\n"),
        Config::default(),
    )
    .unwrap();
    let buf = draw_buffer(&mut app);
    assert_eq!(buf[(0, 0)].symbol(), " ");
    assert_ne!(buf[(1, 0)].symbol(), " ");
}

// Regression test for the popup row layout in `draw_popup` (Settings
// block, src/ui.rs): row width there is computed with `.chars().count()`
// so a multi-row popup's `‹ value ›` sits flush against the right
// border. `‹`/`›` are each 3 UTF-8 bytes but 1 char, so swapping in
// `.len()` (byte count) overcounts the value's width by 4 and shorts the
// gap by 4 columns, leaving the value floating away from the border —
// confirmed by temporarily reverting the three `.chars().count()` calls
// to `.len()` (see commit body for both runs' output). Reconstructing
// each row as a string (rather than asserting on individual `Buffer`
// cells by hand) is the robust option here: it survives incidental
// width/height changes to the popup and reads like the on-screen text.
#[test]
fn settings_popup_second_row_and_value_align_to_border() {
    let root = fixture("settings-rows", "# Title\n");
    let mut app = App::new(root, Config::default()).unwrap();
    app.handle_key(key(KeyCode::Char('s')));
    let buf = draw_buffer(&mut app);

    let width = buf.area.width;
    let height = buf.area.height;
    let rows: Vec<String> = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect();

    let theme_row = rows
        .iter()
        .find(|r| r.contains("theme"))
        .unwrap_or_else(|| panic!("no row with 'theme' found in popup: {rows:?}"));
    let theme_row_chars: Vec<char> = theme_row.chars().collect();
    let value_end = theme_row_chars
        .iter()
        .position(|&c| c == '›')
        .unwrap_or_else(|| panic!("no '›' found in theme row: {theme_row}"));
    // Row text is " theme<gap>‹ default › " followed immediately by the
    // popup's right border: `›`, one space, then `│`.
    let border = theme_row_chars.get(value_end + 2).copied();
    assert_eq!(
        border,
        Some('│'),
        "value not flush against right border two columns after '›': {theme_row}"
    );
    assert!(
        theme_row.contains("‹ default › │"),
        "theme row's value is not flush against the popup border: {theme_row}"
    );

    let side_padding_row = rows
        .iter()
        .find(|r| r.contains("side_padding"))
        .unwrap_or_else(|| panic!("no row with 'side_padding' found in popup: {rows:?}"));
    assert!(
        side_padding_row.contains("‹ 1 ›"),
        "second row missing its default value: {side_padding_row}"
    );
}
