use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use ratatui_textarea::DataCursor;

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

    if app.tree_visible {
        let [tree_area, editor_area] =
            Layout::horizontal([Constraint::Length(30), Constraint::Min(1)]).areas(main);
        draw_tree(f, app, tree_area);
        draw_editor(f, app, editor_area);
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

fn draw_popup(f: &mut Frame, app: &App, area: Rect) {
    if let Prompt::NewFile(input) = &app.prompt {
        let width = (input.len() as u16 + 8).max(40);
        let popup = centered_rect(width, 3, area);
        f.render_widget(Clear, popup);
        let block = popup_block(" New file ");
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        let line = Line::from(vec![
            ratatui::text::Span::raw(format!(" {input}")),
            ratatui::text::Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)),
        ]);
        f.render_widget(Paragraph::new(line), inner);
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
            if i == selected {
                let style = if tree_focused {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().add_modifier(Modifier::REVERSED | Modifier::DIM)
                };
                line = line.style(style);
            }
            line
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_editor(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Editor);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(focused));
    let inner = with_side_margins(block.inner(area));
    f.render_widget(block, area);
    // hide the block cursor while the tree has focus
    let cursor_style = if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    app.editor.textarea.set_cursor_style(cursor_style);
    f.render_widget(&app.editor.textarea, inner);
}

/// Inset a rect by 5% of its width on each side and 3% of its height on
/// top (breathing room for text).
fn with_side_margins(r: Rect) -> Rect {
    let pad = (r.width as u32 * 5 / 100) as u16;
    let top = (r.height as u32 * 3 / 100) as u16;
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
        Prompt::Search(s) => format!("{mode}| search: {s}"),
        Prompt::ConfirmDelete { .. } | Prompt::MoveFile { .. } => {
            format!("{mode}| j/k choose · Enter confirm · Esc cancel")
        }
        Prompt::None => {
            let path = app
                .editor
                .path
                .as_deref()
                .map(|p| {
                    p.strip_prefix(app.tree.root())
                        .unwrap_or(p)
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_else(|| "[no file]".into());
            let dirty = if app.editor.dirty { "*" } else { "" };
            let DataCursor(row, col) = app.editor.textarea.cursor();
            let mut s = format!("{mode}| {path}{dirty}  {}:{}", row + 1, col + 1);
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
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::{backend::TestBackend, Terminal};
    use std::fs;

    #[test]
    fn draws_tree_and_wrapped_editor() {
        let root = std::env::temp_dir().join("mrkdup-ui-1");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.md"), "one two three four five six seven\n").unwrap();
        let mut app = App::new(root).unwrap();
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
        let mut app = App::new(root).unwrap();
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
    fn editor_text_gets_side_and_top_margins() {
        let r = with_side_margins(ratatui::layout::Rect::new(10, 0, 100, 100));
        assert_eq!(r.x, 15); // 5% of 100 = 5 cols in
        assert_eq!(r.width, 90); // 5 off each side
        assert_eq!(r.y, 3); // 3% of 100 = 3 rows down
        assert_eq!(r.height, 97); // trimmed from the top only
        let tiny = with_side_margins(ratatui::layout::Rect::new(0, 0, 3, 5));
        assert_eq!(tiny.width, 3); // tiny pane: percentages round to 0, no underflow
        assert_eq!(tiny.height, 5);
    }
}
