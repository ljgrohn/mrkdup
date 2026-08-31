use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
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
    let inner = block.inner(area);
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

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let mode = match app.focus {
        Focus::Tree => " TREE ",
        Focus::Editor => " EDIT ",
    };
    let text = match &app.prompt {
        Prompt::NewFile(s) => format!("{mode}| new file: {s}"),
        Prompt::Search(s) => format!("{mode}| search: {s}"),
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
        let root = std::env::temp_dir().join("markdup-ui-1");
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
}
