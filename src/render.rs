//! The editor pane renderer: draws the document with soft wrap and live
//! syntax styling. ratatui-textarea remains the editing engine; this
//! module owns everything visual — wrap layout, scroll, the terminal
//! cursor, selection, and search-match highlighting.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::layout_cache::LayoutCache;
use crate::search::find_ci;
use crate::{highlight, wrap};

/// Everything `render_editor` needs to draw one frame of the editor pane.
/// `ui::draw_editor` gathers this from `Editor` and `App` state, so this
/// module never has to depend on `App` itself.
pub struct EditorView<'a> {
    pub lines: &'a [String],
    pub cursor: (usize, usize),
    pub selection: Option<((usize, usize), (usize, usize))>,
    pub search: Option<&'a str>,
    pub file_kind: highlight::FileKind,
    pub scroll: &'a mut usize,
    /// Wrap + highlight cache, owned by `Editor`. Recomputed here only
    /// when it's stale, the width changed, or the file kind changed.
    pub cache: &'a mut LayoutCache,
}

pub fn render_editor(f: &mut Frame, view: EditorView, inner: Rect, focused: bool) {
    let width = inner.width as usize;
    let height = inner.height as usize;
    if width == 0 || height == 0 {
        return;
    }
    let EditorView {
        lines,
        cursor,
        selection,
        search,
        file_kind,
        scroll,
        cache,
    } = view;
    let (rows, spans) = cache.ensure(lines, width, file_kind);
    let (cvrow, cx) = wrap::cursor_position(rows, lines, cursor);
    *scroll = wrap::scroll_top((*scroll).min(rows.len().saturating_sub(1)), cvrow, height);

    let search: Option<(String, usize)> = search
        .filter(|q| !q.is_empty())
        .map(|q| (q.to_string(), q.chars().count()));

    // Span cursor state, carried across rows: within the visible window
    // `row.line` only ever increases (rows are generated in line order,
    // and wrapped rows of one line run start..end contiguously), so a
    // single advancing index per line gives O(1) amortized lookup per
    // painted character instead of an O(spans) scan per character.
    let mut span_cursor_line: Option<usize> = None;
    let mut span_i: usize = 0;

    let out: Vec<Line> = rows
        .iter()
        .skip(*scroll)
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
            let line_spans = &spans[row.line];
            if span_cursor_line != Some(row.line) {
                span_cursor_line = Some(row.line);
                span_i = 0;
            }
            let mut runs: Vec<(String, Style)> = Vec::new();
            for (ci, &ch) in chars.iter().enumerate().take(row.end).skip(row.start) {
                while span_i < line_spans.len() && ci >= line_spans[span_i].end {
                    span_i += 1;
                }
                let mut st = line_spans
                    .get(span_i)
                    .filter(|s| ci >= s.start && ci < s.end)
                    .map(|s| highlight::style(s.kind))
                    .unwrap_or_default();
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
        let y = inner.y + (cvrow - *scroll) as u16;
        f.set_cursor_position((x, y));
    }
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
}
