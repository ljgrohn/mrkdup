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
mod tests;
