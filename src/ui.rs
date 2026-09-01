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
    crate::render::render_editor(f, app, inner, focused);
}

/// The keys most useful before any file is open, shown centered and dim
/// in the editor pane until the first file opens.
fn draw_welcome(f: &mut Frame, area: Rect) {
    let keys = [
        ("Enter", "open file"),
        ("n", "new file"),
        ("p", "go to file"),
        ("m", "move"),
        ("r", "rename"),
        ("x", "delete"),
        ("Ctrl+B/Ctrl+T", "panes"),
        ("q", "quit"),
    ];
    let mut lines = vec![
        Line::from("mrkdup").alignment(Alignment::Center),
        Line::from(""),
    ];
    lines.extend(
        keys.iter()
            .map(|(k, v)| Line::from(format!("{k:>13}  {v}"))),
    );
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
                .map(|p| {
                    p.strip_prefix(app.tree.root())
                        .unwrap_or(p)
                        .to_string_lossy()
                        .into_owned()
                })
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
mod tests {
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
        let key =
            |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
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
        let key =
            |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
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
        let backend = TestBackend::new(60, 20);
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
        // tree occupies columns 0..20: its top-right corner sits at x=19
        // and the editor block starts at x=20 (default would be 29/30)
        assert_eq!(buf.cell((19u16, 0u16)).unwrap().symbol(), "┐");
        assert_eq!(buf.cell((20u16, 0u16)).unwrap().symbol(), "┌");
    }
}
