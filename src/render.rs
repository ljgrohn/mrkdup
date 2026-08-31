//! The editor pane renderer: draws the document with soft wrap and live
//! syntax styling. ratatui-textarea remains the editing engine; this
//! module owns everything visual — wrap layout, scroll, the terminal
//! cursor, selection, and search-match highlighting.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui_textarea::DataCursor;

use crate::app::{find_ci, App};
use crate::{highlight, wrap};

pub fn render_editor(f: &mut Frame, app: &mut App, inner: Rect, focused: bool) {
    let width = inner.width as usize;
    let height = inner.height as usize;
    if width == 0 || height == 0 {
        return;
    }
    let lines: Vec<String> = app.editor.textarea.lines().to_vec();
    let rows = wrap::layout(&lines, width);
    let DataCursor(crow, ccol) = app.editor.textarea.cursor();
    let (cvrow, cx) = wrap::cursor_position(&rows, &lines, (crow, ccol));
    app.editor_scroll = wrap::scroll_top(
        app.editor_scroll.min(rows.len().saturating_sub(1)),
        cvrow,
        height,
    );

    let kind = highlight::file_kind(app.editor.path.as_deref());
    let spans = highlight::highlight(&lines, kind);
    let selection = app.editor.textarea.selection_range();
    let search: Option<(String, usize)> = app
        .search_highlight
        .as_ref()
        .filter(|q| !q.is_empty())
        .map(|q| (q.clone(), q.chars().count()));

    let out: Vec<Line> = rows
        .iter()
        .skip(app.editor_scroll)
        .take(height)
        .map(|row| {
            let line = &lines[row.line];
            let chars: Vec<char> = line.chars().collect();
            let mut match_ranges: Vec<(usize, usize)> = Vec::new();
            if let Some((q, qlen)) = &search {
                let mut from = 0;
                while let Some(p) = find_ci(line, q, from) {
                    match_ranges.push((p, p + qlen));
                    from = p + 1;
                }
            }
            let mut runs: Vec<(String, Style)> = Vec::new();
            for (ci, &ch) in chars.iter().enumerate().take(row.end).skip(row.start) {
                let mut st = style_at(&spans[row.line], ci);
                if match_ranges.iter().any(|&(a, b)| ci >= a && ci < b) {
                    st = st.bg(Color::Yellow).fg(Color::Black);
                }
                if in_selection(selection, row.line, ci) {
                    st = st.add_modifier(Modifier::REVERSED);
                }
                let piece = if ch == '\t' {
                    "    ".to_string()
                } else {
                    ch.to_string()
                };
                match runs.last_mut() {
                    Some((run, rst)) if *rst == st => run.push_str(&piece),
                    _ => runs.push((piece, st)),
                }
            }
            Line::from(
                runs.into_iter()
                    .map(|(t, st)| Span::styled(t, st))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    f.render_widget(Paragraph::new(out), inner);
    if focused {
        let x = inner.x + cx.min(width.saturating_sub(1)) as u16;
        let y = inner.y + (cvrow - app.editor_scroll) as u16;
        f.set_cursor_position((x, y));
    }
}

fn style_at(spans: &[highlight::SpanTok], ci: usize) -> Style {
    spans
        .iter()
        .find(|s| ci >= s.start && ci < s.end)
        .map(|s| highlight::style(s.kind))
        .unwrap_or_default()
}

fn in_selection(sel: Option<((usize, usize), (usize, usize))>, line: usize, ci: usize) -> bool {
    // selection_range is documented ordered: start <= end
    let Some((s, e)) = sel else { return false };
    (line, ci) >= s && (line, ci) < e
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Focus};
    use crate::config::Config;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
}
