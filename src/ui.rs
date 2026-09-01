use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Focus, Prompt};

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let [main, status] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(f.area());

    if app.tree_visible && app.editor_visible {
        let [tree_area, editor_area] = Layout::horizontal([
            Constraint::Length(app.config.tree_width),
            Constraint::Min(1),
        ])
        .areas(main);
        draw_tree(f, app, tree_area);
        draw_editor(f, app, editor_area);
    } else if app.tree_visible {
        draw_tree(f, app, main);
    } else {
        draw_editor(f, app, main);
    }

    draw_status(f, app, status);
    draw_popup(f, app, main);
}

/// A `width` x `height` rect centered in `area` (clamped to fit).
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn popup_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title)
}

/// A one-line text-input popup: the typed input followed by a block cursor.
fn draw_input_popup(f: &mut Frame, area: Rect, title: &str, input: &str) {
    let width = (input.len() as u16 + 8).max(40);
    let popup = centered_rect(width, 3, area);
    f.render_widget(Clear, popup);
    let block = popup_block(title);
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let line = Line::from(vec![
        ratatui::text::Span::raw(format!(" {input}")),
        ratatui::text::Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn draw_popup(f: &mut Frame, app: &App, area: Rect) {
    if let Prompt::Help = &app.prompt {
        let lines = key_lines();
        let width = lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16 + 4;
        let popup = centered_rect(width, lines.len() as u16 + 2, area);
        f.render_widget(Clear, popup);
        let block = popup_block(" Keys ");
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        // left-aligned so the key column lines up; one cell of padding
        let padded = Rect {
            x: inner.x + 1,
            width: inner.width.saturating_sub(2),
            ..inner
        };
        f.render_widget(Paragraph::new(lines), padded);
    }
    if let Prompt::NewFile(input) = &app.prompt {
        draw_input_popup(f, area, " New file ", input);
    }
    if let Prompt::Search(input) = &app.prompt {
        draw_input_popup(f, area, " Search ", input);
    }
    if let Prompt::Rename { input, .. } = &app.prompt {
        draw_input_popup(f, area, " Rename ", input);
    }
    if let Prompt::GoToFile {
        input,
        candidates,
        selected,
    } = &app.prompt
    {
        let matches = crate::fuzzy::fuzzy_filter(input, candidates);
        let sel = (*selected).min(matches.len().saturating_sub(1));
        let visible = matches.len().min(10);
        let width = matches
            .iter()
            .take(visible.max(1))
            .map(|c| c.0.len())
            .max()
            .unwrap_or(0)
            .max(input.len() + 8)
            .max(40) as u16
            + 4;
        let height = (visible as u16 + 3).min(area.height); // borders + input line
        let popup = centered_rect(width, height, area);
        f.render_widget(Clear, popup);
        let block = popup_block(" Go to file ");
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        let mut lines = vec![Line::from(vec![
            ratatui::text::Span::raw(format!(" {input}")),
            ratatui::text::Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)),
        ])];
        // keep the selection visible if it scrolls past the shown window
        let top = sel.saturating_sub(visible.saturating_sub(1));
        for (i, c) in matches.iter().enumerate().skip(top).take(visible) {
            let mut line = Line::from(format!(" {} ", c.0));
            if i == sel {
                line = line.style(Style::default().add_modifier(Modifier::REVERSED));
            }
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), inner);
    }
    if let Prompt::MoveFile {
        src,
        dests,
        selected,
    } = &app.prompt
    {
        let name = src.file_name().unwrap_or_default().to_string_lossy();
        let title = format!(" Move {name} to… ");
        let names: Vec<String> = dests
            .iter()
            .map(|d| {
                if d == app.tree.root() {
                    "./".into()
                } else {
                    let rel = d.strip_prefix(app.tree.root()).unwrap_or(d);
                    format!("{}/", rel.to_string_lossy())
                }
            })
            .collect();
        let width = names
            .iter()
            .map(|n| n.len())
            .max()
            .unwrap_or(10)
            .max(title.len()) as u16
            + 6;
        let height = (names.len() as u16 + 2).min(area.height);
        let popup = centered_rect(width.max(30), height, area);
        f.render_widget(Clear, popup);
        let block = popup_block(&title);
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        // keep the selection visible if the list is taller than the popup
        let visible = inner.height as usize;
        let top = selected.saturating_sub(visible.saturating_sub(1));
        let lines: Vec<Line> = names
            .iter()
            .enumerate()
            .skip(top)
            .take(visible)
            .map(|(i, n)| {
                let mut line = Line::from(format!(" {n} "));
                if i == *selected {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }
    if let Prompt::ConfirmDelete { path, yes } = &app.prompt {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let popup = centered_rect((name.len() as u16 + 16).max(30), 5, area);
        f.render_widget(Clear, popup);
        let block = popup_block(" Delete file? ");
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        let on = Style::default().add_modifier(Modifier::REVERSED);
        let off = Style::default();
        let lines = vec![
            Line::from(format!("Delete {name}?")).alignment(Alignment::Center),
            Line::from(""),
            Line::from(vec![
                ratatui::text::Span::styled("  Yes  ", if *yes { on } else { off }),
                ratatui::text::Span::raw("   "),
                ratatui::text::Span::styled("  No  ", if *yes { off } else { on }),
            ])
            .alignment(Alignment::Center),
        ];
        f.render_widget(Paragraph::new(lines), inner);
    }
}

fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let root_name = app
        .tree
        .root()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".into());
    let tree_focused = matches!(app.focus, Focus::Tree);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(tree_focused))
        .title(root_name);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    let selected = app.tree.selected();
    if selected < app.tree_scroll {
        app.tree_scroll = selected;
    } else if height > 0 && selected >= app.tree_scroll + height {
        app.tree_scroll = selected + 1 - height;
    }

    let open_marker = open_marker_index(app.tree.rows(), app.editor.path.as_deref());
    let lines: Vec<Line> = app
        .tree
        .rows()
        .iter()
        .enumerate()
        .skip(app.tree_scroll)
        .take(height)
        .map(|(i, row)| {
            let marker = if row.is_dir {
                if row.expanded {
                    "▾ "
                } else {
                    "▸ "
                }
            } else {
                "  "
            };
            let text = format!("{}{}{}", "  ".repeat(row.depth), marker, row.name);
            let mut line = Line::from(text);
            // blue = the open document (or the folder hiding it)
            let mut style = if Some(i) == open_marker {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            if i == selected {
                style = style.add_modifier(Modifier::REVERSED);
                if !tree_focused {
                    style = style.add_modifier(Modifier::DIM);
                }
            }
            line = line.style(style);
            line
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

/// Which tree row carries the blue "open document" marker: the open
/// file's own row, or — when a collapsed folder hides it — the deepest
/// visible ancestor directory.
fn open_marker_index(rows: &[crate::tree::Row], open: Option<&std::path::Path>) -> Option<usize> {
    let open = open?;
    if let Some(i) = rows.iter().position(|r| r.path == open) {
        return Some(i);
    }
    rows.iter()
        .enumerate()
        .filter(|(_, r)| r.is_dir && open.starts_with(&r.path))
        .max_by_key(|(_, r)| r.path.components().count())
        .map(|(i, _)| i)
}

fn draw_editor(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Editor);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(focused));
    let inner = with_side_margins(
        block.inner(area),
        app.config.side_margin_percent,
        app.config.top_margin_percent,
    );
    f.render_widget(block, area);
    if app.editor.path.is_none() {
        draw_welcome(f, inner);
        return;
    }
    // our renderer: soft wrap + live syntax styling + terminal cursor
    // (only drawn when the editor has focus)
    let cursor = app.editor.cursor();
    let selection = app.editor.selection_range();
    let file_kind = crate::highlight::file_kind(app.editor.path.as_deref());
    let (lines, cache) = app.editor.render_parts();
    let view = crate::render::EditorView {
        lines,
        cursor,
        selection,
        search: app.search_highlight.as_deref(),
        file_kind,
        scroll: &mut app.editor_scroll,
        cache,
    };
    crate::render::render_editor(f, view, inner, focused);
}

/// The key cheat sheet, one `key  action` line per row, shared by the
/// launch page and the `?` help overlay so the two can't drift. Tree keys
/// first (that's where focus starts), then the globals.
fn key_lines() -> Vec<Line<'static>> {
    const KEYS: &[(&str, &str)] = &[
        ("Enter", "open file"),
        ("n", "new file"),
        ("p", "go to file"),
        ("m", "move"),
        ("r", "rename"),
        ("x", "delete"),
        (".", "hidden files (dotfiles + gitignored)"),
        ("- / +", "go up / zoom in"),
        ("u", "refresh"),
        ("Ctrl+B/Ctrl+T", "panes"),
        ("?", "help"),
        ("q", "quit"),
    ];
    KEYS.iter()
        .map(|(k, v)| Line::from(format!("{k:>13}  {v}")))
        .collect()
}

/// The cheat sheet, shown centered and dim in the editor pane until the
/// first file opens.
fn draw_welcome(f: &mut Frame, area: Rect) {
    let mut lines = vec![
        Line::from("mrkdup").alignment(Alignment::Center),
        Line::from(""),
    ];
    lines.extend(key_lines());
    let width = lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16;
    let rect = centered_rect(width, lines.len() as u16, area);
    f.render_widget(
        Paragraph::new(lines).style(Style::default().add_modifier(Modifier::DIM)),
        rect,
    );
}

/// Inset a rect by `side_pct`% of its width on each side and `top_pct`%
/// of its height on top (breathing room for text).
fn with_side_margins(r: Rect, side_pct: u16, top_pct: u16) -> Rect {
    let pad = (r.width as u32 * side_pct as u32 / 100) as u16;
    let top = (r.height as u32 * top_pct as u32 / 100) as u16;
    Rect {
        x: r.x + pad,
        y: r.y + top,
        width: r.width.saturating_sub(pad * 2),
        height: r.height.saturating_sub(top),
    }
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let mode = match app.focus {
        Focus::Tree => " TREE ",
        Focus::Editor => " EDIT ",
    };
    let text = match &app.prompt {
        Prompt::NewFile(_) => {
            format!("{mode}| type a name (dir/name.md works) · Enter create · Esc cancel")
        }
        Prompt::Help => format!("{mode}| any key closes"),
        Prompt::Search(_) => format!("{mode}| Enter jump · Esc cancel"),
        Prompt::Rename { .. } => format!("{mode}| type the new name · Enter rename · Esc cancel"),
        Prompt::GoToFile { .. } => {
            format!("{mode}| type to filter · ↑/↓ or Ctrl+J/K choose · Enter open · Esc cancel")
        }
        Prompt::ConfirmDelete { .. } | Prompt::MoveFile { .. } => {
            format!("{mode}| j/k choose · Enter confirm · Esc cancel")
        }
        Prompt::None => {
            let path = app
                .editor
                .path
                .as_deref()
                .map(|p| crate::fuzzy::rel_display(app.tree.root(), p))
                .unwrap_or_else(|| "[no file]".into());
            let dirty = if app.editor.dirty { "*" } else { "" };
            let (row, col) = app.editor.cursor();
            let mut s = format!("{mode}| {path}{dirty}  {}:{}", row + 1, col + 1);
            if app.editor.path.is_some() {
                let words: usize = app
                    .editor
                    .lines()
                    .iter()
                    .map(|l| l.split_whitespace().count())
                    .sum();
                s.push_str(&format!(" · {words} words"));
            }
            if let Some(msg) = &app.status {
                s.push_str("  —  ");
                s.push_str(msg);
            }
            s
        }
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().add_modifier(Modifier::REVERSED)),
        area,
    );
}

#[cfg(test)]
mod tests;
