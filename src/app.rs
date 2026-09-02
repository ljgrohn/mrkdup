use std::io;
use std::path::PathBuf;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui_textarea::{CursorMove, Input};

use crate::config::Config;
use crate::editor::{Editor, SaveOutcome};
use crate::tab::{self, Part, Segment, Tab};
use crate::theme::Theme;
use crate::tree::Tree;

pub enum Focus {
    Tree,
    Editor,
}

pub enum Prompt {
    None,
    /// The key cheat sheet, drawn over the editor pane; any key closes it.
    Help,
    NewFile(String),
    Search(String),
    ConfirmDelete {
        path: PathBuf,
        yes: bool,
    },
    MoveFile {
        src: PathBuf,
        dests: Vec<PathBuf>,
        selected: usize,
    },
    Rename {
        path: PathBuf,
        input: String,
    },
    GoToFile {
        input: String,
        /// (root-relative display path, absolute path), collected once
        /// when the popup opens.
        candidates: Vec<(String, PathBuf)>,
        /// Index into the current filtered result list.
        selected: usize,
    },
    /// The settings list (`s` in the tree): one row per option, `h`/`l`
    /// cycle the selected row's value and apply it immediately.
    Settings {
        rows: Vec<SettingRow>,
        selected: usize,
    },
}

/// One row of the settings popup: a named option with a fixed list of
/// choices and the index of the current one. `h`/`l` step it; the
/// popup shows `name ‹ value ›`.
pub struct SettingRow {
    pub name: &'static str,
    pub choices: Vec<String>,
    pub index: usize,
}

impl SettingRow {
    pub fn value(&self) -> &str {
        &self.choices[self.index]
    }

    /// Move `delta` choices (±1), wrapping, and return the new value.
    pub fn step(&mut self, delta: isize) -> String {
        let n = self.choices.len() as isize;
        self.index = (self.index as isize + delta).rem_euclid(n) as usize;
        self.choices[self.index].clone()
    }
}

pub struct App {
    pub tree: Tree,
    /// The open documents, in bar order. Empty = the welcome page.
    pub tabs: Vec<Tab>,
    /// Index into `tabs` of the one being edited (0 when there are none).
    pub active: usize,
    pub config: Config,
    pub theme: Theme,
    /// `$XDG_CONFIG_HOME/mrkdup`: where the settings popup reads
    /// `themes/` and writes `config`. `None` = no HOME; tests set it
    /// explicitly so they never touch the real config.
    pub config_dir: Option<PathBuf>,
    pub focus: Focus,
    pub tree_visible: bool,
    pub editor_visible: bool,
    pub prompt: Prompt,
    pub status: Option<String>,
    pub tree_scroll: usize,
    pub should_quit: bool,
    pub last_tree_refresh: Instant,
    /// The editor pane's text rect as of the last frame — `None` until
    /// the first draw, while the pane is hidden, or while the welcome
    /// page covers it. Mouse events hit-test against it.
    pub editor_area: Option<Rect>,
    /// The tree pane's rows rect as of the last frame (`None` when the
    /// pane is hidden).
    pub tree_area: Option<Rect>,
    /// The tab bar as of the last frame: its rect and the painted
    /// segments (x-relative to the rect), for click-to-switch/close.
    pub tab_bar: Option<(Rect, Vec<Segment>)>,
    /// Text a mouse selection wants on the system clipboard; `main.rs`
    /// drains it into an OSC 52 write after each event.
    pub clipboard: Option<String>,
    /// A left-button drag that began in the editor pane is in progress.
    dragging: bool,
    pending_quit: bool,
    last_search: String,
}

impl App {
    /// `new_with_theme` looking the theme up by name alone, with no
    /// overlay files applied. Since `main.rs` resolves the theme via
    /// `theme::load` (which does apply overlays) and calls
    /// `new_with_theme` directly, this shorthand is only exercised by
    /// tests that don't care about theme overlays.
    #[cfg(test)]
    pub fn new(root: PathBuf, config: Config) -> io::Result<App> {
        let theme = Theme::named(&config.theme_name);
        // no config dir: tests must never write the user's real config
        App::new_with_theme(root, config, theme, None)
    }

    /// Same as `new`, but with an already-resolved `Theme` (e.g. one
    /// loaded from disk via `theme::load`, overlay files and all)
    /// instead of looking one up by `config.theme_name` again, and the
    /// config directory the settings popup may write to.
    pub fn new_with_theme(
        root: PathBuf,
        config: Config,
        theme: Theme,
        config_dir: Option<PathBuf>,
    ) -> io::Result<App> {
        Ok(App {
            tree: Tree::new(root)?,
            tabs: Vec::new(),
            active: 0,
            theme,
            config_dir,
            config,
            focus: Focus::Tree,
            tree_visible: true,
            editor_visible: true,
            prompt: Prompt::None,
            status: None,
            tree_scroll: 0,
            should_quit: false,
            last_tree_refresh: Instant::now(),
            editor_area: None,
            tree_area: None,
            tab_bar: None,
            clipboard: None,
            dragging: false,
            pending_quit: false,
            last_search: String::new(),
        })
    }

    /// The active tab, if any file is open.
    pub fn tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    #[cfg(test)]
    pub fn tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    /// The active tab's editor. Only for paths that have already checked
    /// a file is open (`editor_key` and friends); panics otherwise.
    fn ed(&mut self) -> &mut Editor {
        &mut self.tabs[self.active].editor
    }

    fn ed_ref(&self) -> &Editor {
        &self.tabs[self.active].editor
    }

    /// Test shorthand for the active tab's editor.
    #[cfg(test)]
    pub fn editor(&self) -> &Editor {
        &self.tab().expect("a file is open").editor
    }

    /// Bar titles, in order, for the tab-bar layout.
    pub fn tab_titles(&self) -> Vec<String> {
        self.tabs.iter().map(Tab::title).collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.status = None;
        if !matches!(self.prompt, Prompt::None) {
            self.prompt_key(key);
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (ctrl, key.code) {
            (true, KeyCode::Char('q') | KeyCode::Char('c')) => self.do_quit(),
            (true, KeyCode::Char('b')) => {
                self.tree_visible = !self.tree_visible;
                if !self.tree_visible {
                    self.editor_visible = true; // never hide both panes
                    self.focus = Focus::Editor;
                }
            }
            (true, KeyCode::Char('p')) => self.open_go_to_file(),
            (true, KeyCode::Char('w')) => self.close_tab(self.active),
            // Opt+H / Opt+L cycle tabs, Opt+1..9 jump (Cmd+H would hide
            // the terminal on macOS, so only Option)
            (false, KeyCode::Char(c @ ('h' | 'l' | '1'..='9')))
                if key.modifiers.contains(KeyModifiers::ALT) =>
            {
                match c {
                    'h' => self.cycle_tab(-1),
                    'l' => self.cycle_tab(1),
                    n => {
                        let i = n as usize - '1' as usize;
                        if i < self.tabs.len() {
                            self.activate_tab(i);
                        }
                    }
                }
            }
            // Ctrl+T, not Ctrl+E: macOS terminals send Ctrl+E for Cmd+Right
            (true, KeyCode::Char('t')) => {
                self.editor_visible = !self.editor_visible;
                if !self.editor_visible {
                    self.tree_visible = true; // never hide both panes
                    self.focus = Focus::Tree;
                }
            }
            _ => match self.focus {
                Focus::Tree => self.tree_key(key),
                Focus::Editor => self.editor_key(key),
            },
        }
    }

    /// Idle autosave + external-change pickup; called when input is quiet.
    pub fn tick(&mut self) {
        let autosave = self.config.autosave();
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            let name = tab.name();
            if tab.editor.dirty && tab.last_edit.is_some_and(|t| t.elapsed() >= autosave) {
                match tab.editor.save(false) {
                    Ok(SaveOutcome::Saved) => self.status = Some(format!("saved {name}")),
                    Ok(SaveOutcome::Conflict) => {
                        self.status =
                            Some(format!("{name}: disk changed — Ctrl+S again to overwrite"));
                        tab.force_next_save = true;
                        tab.last_edit = None; // don't retry until the next edit
                    }
                    Ok(_) => {}
                    Err(e) => self.status = Some(format!("save failed: {e}")),
                }
            }
            match tab.editor.check_external() {
                Ok(true) => {
                    self.status = Some(format!("reloaded {name} (changed on disk)"));
                    if i == self.active {
                        // nothing about the old text applies any more
                        tab.search_highlight = None;
                    }
                }
                Ok(false) => {}
                Err(e) => self.status = Some(format!("reload failed: {e}")),
            }
        }
        // pick up files created/removed outside the app; the rebuild
        // only walks expanded directories, so this stays cheap
        if self.last_tree_refresh.elapsed() >= self.config.tree_refresh() {
            self.tree.refresh();
            self.last_tree_refresh = Instant::now();
        }
    }

    /// Mouse input. The app owns the mouse (see `main.rs`), so this is
    /// where clicks land the cursor, drags select, and the wheel scrolls;
    /// everything is confined to the pane it started in — a drag that
    /// wanders over the tree keeps selecting editor text. Ignored while
    /// a prompt is open.
    pub fn handle_mouse(&mut self, m: MouseEvent) {
        if !matches!(self.prompt, Prompt::None) {
            return;
        }
        let (x, y) = (m.column, m.row);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.status = None;
                if let Some((bar, segs)) = self.tab_bar.as_ref().filter(|(r, _)| contains(*r, x, y))
                {
                    let rel = x - bar.x;
                    if let Some(seg) = segs.iter().find(|s| s.hit(rel)) {
                        let (i, part) = (seg.index, seg.part);
                        match part {
                            Part::Close => self.close_tab(i),
                            Part::Title | Part::Prev | Part::Next => self.activate_tab(i),
                        }
                    }
                } else if let Some(area) = self.editor_area.filter(|a| contains(*a, x, y)) {
                    self.focus = Focus::Editor;
                    let (row, col) = self.editor_hit(area, x, y);
                    let tab = &mut self.tabs[self.active];
                    tab.follow_cursor = true;
                    tab.editor.cancel_selection();
                    tab.editor.set_cursor(row, col);
                    tab.editor.start_selection();
                    self.dragging = true;
                } else if let Some(area) = self.tree_area.filter(|a| contains(*a, x, y)) {
                    self.focus = Focus::Tree;
                    let i = self.tree_scroll + (y - area.y) as usize;
                    if self.tree.select(i) {
                        self.open_selected();
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) if self.dragging => {
                if let Some(area) = self.editor_area {
                    let (row, col) = self.editor_hit(area, x, y);
                    let tab = &mut self.tabs[self.active];
                    tab.follow_cursor = true;
                    tab.editor.set_cursor(row, col);
                }
            }
            MouseEventKind::Up(MouseButton::Left) if self.dragging => {
                self.dragging = false;
                match self.ed_ref().selected_text() {
                    Some(text) => {
                        self.status = Some("copied to clipboard".into());
                        self.clipboard = Some(text);
                    }
                    // a plain click: no selection to keep
                    None => self.ed().cancel_selection(),
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let up = matches!(m.kind, MouseEventKind::ScrollUp);
                if self.editor_area.is_some_and(|a| contains(a, x, y)) {
                    // scroll the view, not the cursor; the renderer clamps
                    // to the last row and stops following the cursor
                    // until the next key or click
                    let tab = &mut self.tabs[self.active];
                    tab.scroll = if up {
                        tab.scroll.saturating_sub(3)
                    } else {
                        tab.scroll + 3
                    };
                    tab.follow_cursor = false;
                } else if self.tree_area.is_some_and(|a| contains(a, x, y)) {
                    for _ in 0..3 {
                        if up {
                            self.tree.move_up();
                        } else {
                            self.tree.move_down();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The (line, col) under screen cell (`x`, `y`) given the editor's
    /// text rect. Points outside the rect clamp to it: above the pane
    /// resolves to the row just above the visible window and below it to
    /// the row just under, so a drag past either edge scrolls one row
    /// per event; left/right of it snap to the row's ends.
    fn editor_hit(&mut self, area: Rect, x: u16, y: u16) -> (usize, usize) {
        let tab = &mut self.tabs[self.active];
        let file_kind = crate::highlight::file_kind(tab.editor.path.as_deref());
        let scroll = tab.scroll;
        let height = area.height as usize;
        let vrow = if y < area.y {
            scroll.saturating_sub(1)
        } else if (y - area.y) as usize >= height {
            scroll + height
        } else {
            scroll + (y - area.y) as usize
        };
        let xcell = x.saturating_sub(area.x) as usize;
        let (lines, cache) = tab.editor.render_parts();
        let (rows, _) = cache.ensure(lines, area.width as usize, file_kind);
        crate::wrap::hit_test(rows, lines, vrow, xcell)
    }

    fn tree_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.tree.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.tree.move_up(),
            KeyCode::Char('l') | KeyCode::Right => self.tree.expand(),
            KeyCode::Char('h') | KeyCode::Left => self.tree.collapse(),
            KeyCode::Char('g') => self.tree.move_top(),
            KeyCode::Char('G') => self.tree.move_bottom(),
            KeyCode::Char('.') => self.tree.toggle_hidden(),
            KeyCode::Char('?') => self.prompt = Prompt::Help,
            KeyCode::Char('s') if key.modifiers.is_empty() => self.open_settings(),
            KeyCode::Char('-') => self.tree.ascend(),
            KeyCode::Char('+') => self.tree.make_root(),
            KeyCode::Char('n') => self.prompt = Prompt::NewFile(String::new()),
            KeyCode::Char('x' | 'X') => self.confirm_delete(),
            KeyCode::Char('m') => self.start_move(),
            KeyCode::Char('r') => self.start_rename(),
            KeyCode::Char('p') => self.open_go_to_file(),
            KeyCode::Char('q') => self.do_quit(),
            KeyCode::Char('u') => {
                self.tree.refresh();
                self.last_tree_refresh = Instant::now();
                self.status = Some("refreshed".into());
            }
            KeyCode::Enter | KeyCode::Tab => self.open_selected(),
            _ => {}
        }
    }

    fn start_move(&mut self) {
        match self.tree.selected_row() {
            Some(r) if !r.is_dir => {
                let src = r.path.clone();
                // destinations: the root plus every directory in the tree
                let mut dests = vec![self.tree.root().to_path_buf()];
                dests.extend(
                    self.tree
                        .rows()
                        .iter()
                        .filter(|row| row.is_dir)
                        .map(|row| row.path.clone()),
                );
                self.prompt = Prompt::MoveFile {
                    src,
                    dests,
                    selected: 0,
                };
            }
            Some(_) => self.status = Some("can only move files".into()),
            None => {}
        }
    }

    fn move_file(&mut self, src: PathBuf, dest_dir: PathBuf) {
        match crate::files::move_to(&mut self.tree, &mut self.tabs, &src, &dest_dir) {
            Ok(Some(status)) => self.status = Some(status),
            Ok(None) => {}
            Err(e) => self.status = Some(e),
        }
    }

    fn start_rename(&mut self) {
        match self.tree.selected_row() {
            Some(r) if !r.is_dir => {
                let input = r
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.prompt = Prompt::Rename {
                    path: r.path.clone(),
                    input,
                };
            }
            Some(_) => self.status = Some("can only rename files".into()),
            None => {}
        }
    }

    /// Rename `src` to `name` within its own directory.
    fn submit_rename(&mut self, src: &std::path::Path, name: &str) {
        match crate::files::rename(&mut self.tree, &mut self.tabs, src, name) {
            Ok(Some(status)) => self.status = Some(status),
            Ok(None) => {}
            Err(e) => self.status = Some(e),
        }
    }

    fn open_go_to_file(&mut self) {
        let candidates =
            crate::fuzzy::collect_candidates(self.tree.root(), self.tree.show_hidden());
        self.prompt = Prompt::GoToFile {
            input: String::new(),
            candidates,
            selected: 0,
        };
    }

    /// Build the settings rows. The theme row lists the builtins, then
    /// the user's `themes/` files; it starts on the configured name (or
    /// `default` if that name isn't in the list). The side-padding row
    /// lists every column 0..=20.
    fn open_settings(&mut self) {
        let mut choices: Vec<String> = crate::theme::BUILTINS
            .iter()
            .map(|s| s.to_string())
            .collect();
        if let Some(dir) = &self.config_dir {
            choices.extend(crate::theme::list_user_themes(dir));
        }
        let index = choices
            .iter()
            .position(|c| *c == self.config.theme_name)
            .unwrap_or(0);
        let padding_choices: Vec<String> = (0..=20u16).map(|n| n.to_string()).collect();
        let padding_index = (self.config.side_padding as usize).min(20);
        self.prompt = Prompt::Settings {
            rows: vec![
                SettingRow {
                    name: "theme",
                    choices,
                    index,
                },
                SettingRow {
                    name: "side_padding",
                    choices: padding_choices,
                    index: padding_index,
                },
            ],
            selected: 0,
        };
    }

    /// A settings row changed: apply the new value live and persist it
    /// as `<row> = <value>` in the config file. The theme goes through
    /// `load_from` so `themes/<name>` and the overlay file both apply,
    /// exactly as at startup.
    fn apply_setting(&mut self, row: &str, value: &str) {
        let mut warnings: Vec<String> = Vec::new();
        match row {
            "theme" => {
                self.theme = match &self.config_dir {
                    Some(dir) => {
                        let (theme, w) = crate::theme::load_from(value, dir);
                        warnings = w;
                        theme
                    }
                    None => Theme::named(value),
                };
                self.config.theme_name = value.to_string();
            }
            "side_padding" => {
                self.config.side_padding = value.parse::<u16>().unwrap_or(1).min(20);
            }
            _ => return,
        }
        let saved = self.persist_setting(row, value);
        self.status = Some(match (saved, warnings.first()) {
            (Ok(()), None) => format!("{row}: {value}"),
            (Ok(()), Some(w)) => format!("{row}: {value} — {w}"),
            (Err(e), _) => format!("{row}: {value} (not saved: {e})"),
        });
    }

    /// Write one setting back to `config_dir/config`; `Err` carries the
    /// reason for the status line.
    fn persist_setting(&self, key: &str, value: &str) -> Result<(), String> {
        match &self.config_dir {
            Some(dir) => crate::config::save_key_to(&dir.join("config"), key, value)
                .map_err(|e| e.to_string()),
            None => Err("no config dir".to_string()),
        }
    }

    fn confirm_delete(&mut self) {
        match self.tree.selected_row() {
            Some(r) if !r.is_dir => {
                self.prompt = Prompt::ConfirmDelete {
                    path: r.path.clone(),
                    yes: false,
                };
            }
            Some(_) => self.status = Some("can only delete files".into()),
            None => {}
        }
    }

    fn delete_file(&mut self, path: PathBuf) {
        match crate::files::delete(&mut self.tree, &path) {
            Ok(status) => {
                self.status = Some(status);
                // the file is gone: drop its tab without saving
                if let Some(i) = self.tab_index(&path) {
                    self.remove_tab(i);
                }
            }
            Err(e) => self.status = Some(e),
        }
    }

    fn open_selected(&mut self) {
        let Some(row) = self.tree.selected_row() else {
            return;
        };
        if row.is_dir {
            self.tree.toggle();
        } else {
            let path = row.path.clone();
            self.open_file(path);
        }
    }

    /// Open `path` in a new tab right of the active one and focus the
    /// editor — or, if it's already open, just switch to that tab. The
    /// tab being left autosaves on the way out.
    fn open_file(&mut self, path: PathBuf) {
        if let Some(i) = self.tab_index(&path) {
            self.activate_tab(i);
            return;
        }
        let mut editor = Editor::new();
        if let Err(e) = editor.open(&path) {
            self.status = Some(format!("open failed: {e}"));
            return;
        }
        self.autosave_active();
        let at = if self.tabs.is_empty() {
            0
        } else {
            self.active + 1
        };
        self.tabs.insert(at, Tab::new(editor));
        self.active = at;
        self.focus = Focus::Editor;
        self.editor_visible = true;
        self.pending_quit = false;
    }

    /// The tab showing `path`, if any.
    fn tab_index(&self, path: &std::path::Path) -> Option<usize> {
        self.tabs
            .iter()
            .position(|t| t.editor.path.as_deref() == Some(path))
    }

    /// Switch to tab `i` (a click, Opt+1..9, or reopening a file that's
    /// already in a tab); the tab being left autosaves on the way out.
    fn activate_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        if i != self.active {
            self.autosave_active();
            self.active = i;
            self.dragging = false;
        }
        self.focus = Focus::Editor;
        self.editor_visible = true;
    }

    /// Opt+H / Opt+L: the previous / next tab, wrapping.
    fn cycle_tab(&mut self, delta: isize) {
        let n = self.tabs.len() as isize;
        if n == 0 {
            return;
        }
        let i = (self.active as isize + delta).rem_euclid(n) as usize;
        self.activate_tab(i);
    }

    /// Save the active tab because we're leaving it. A disk conflict
    /// doesn't block the switch — the tab stays open with the warning
    /// and its `*`, and the next Ctrl+S there overwrites.
    fn autosave_active(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let name = tab.name();
        let status = match tab.editor.save(false) {
            Ok(SaveOutcome::Conflict) => {
                tab.force_next_save = true;
                Some(format!(
                    "{name}: unsaved changes conflict with disk — Ctrl+S there to resolve"
                ))
            }
            Err(e) => Some(format!("save failed: {e}")),
            Ok(_) => None,
        };
        if status.is_some() {
            self.status = status;
        }
    }

    /// Ctrl+W or the bar's `×`: autosave and close tab `i`. A disk
    /// conflict or a failed save keeps it open (and makes it active so
    /// the warning is about what's on screen).
    fn close_tab(&mut self, i: usize) {
        let Some(tab) = self.tabs.get_mut(i) else {
            return;
        };
        let name = tab.name();
        match tab.editor.save(false) {
            Ok(SaveOutcome::Conflict) => {
                self.status = Some("unsaved changes conflict with disk — Ctrl+S to resolve".into());
                tab.force_next_save = true;
                self.active = i;
                self.focus = Focus::Editor;
                return;
            }
            Err(e) => {
                self.status = Some(format!("save failed: {e}"));
                self.active = i;
                self.focus = Focus::Editor;
                return;
            }
            Ok(_) => {}
        }
        self.remove_tab(i);
        self.status = Some(format!("closed {name}"));
    }

    /// Drop tab `i` without saving; the neighbour takes over, or the
    /// welcome page and the tree when it was the last one.
    fn remove_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.tabs.remove(i);
        self.active = tab::after_close(self.active, i, self.tabs.len());
        self.dragging = false;
        self.pending_quit = false;
        if self.tabs.is_empty() {
            self.editor_area = None;
            self.tab_bar = None;
            self.focus = Focus::Tree;
        }
    }

    fn editor_key(&mut self, key: KeyEvent) {
        // no file open: the welcome pane covers the textarea, so typing
        // would silently go into a buffer that can never be saved
        if self.tabs.is_empty() {
            match key.code {
                KeyCode::Esc | KeyCode::BackTab => self.focus = Focus::Tree,
                _ => self.status = Some("no file open — pick one in the tree (Esc)".into()),
            }
            return;
        }
        // any key brings the view back to the cursor after a wheel scroll
        self.tabs[self.active].follow_cursor = true;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (ctrl, key.code) {
            // Shift+Tab arrives as BackTab; treat it like Esc
            (false, KeyCode::Esc) | (_, KeyCode::BackTab) => self.focus = Focus::Tree,
            (true, KeyCode::Char('s')) => self.do_save(),
            (true, KeyCode::Char('f')) => self.prompt = Prompt::Search(String::new()),
            (true, KeyCode::Char('d')) => self.toggle_checkbox(),
            // crate defaults make Ctrl+K kill-to-end-of-line; we use
            // Ctrl+J/Ctrl+K as word motions instead
            (true, KeyCode::Char('j')) => {
                self.ed().move_cursor(CursorMove::WordForward);
            }
            (true, KeyCode::Char('k')) => {
                self.ed().move_cursor(CursorMove::WordBack);
            }
            (true, KeyCode::Char('g')) => {
                if self.last_search.is_empty() {
                    self.status = Some("no previous search (Ctrl+F to search)".into());
                } else {
                    let q = self.last_search.clone();
                    self.search_next(&q);
                }
            }
            // crate defaults are Ctrl+U/Ctrl+R with Ctrl+Y = paste;
            // intercept so the advertised keys work
            (true, KeyCode::Char('z')) => {
                if self.ed().undo() {
                    self.note_edit();
                }
            }
            (true, KeyCode::Char('y')) => {
                if self.ed().redo() {
                    self.note_edit();
                }
            }
            // j/k motions with Option or Cmd held (plain/shifted j/k
            // still types): Option = paragraph, Cmd = line start/end
            (false, KeyCode::Char(c @ ('j' | 'k' | 'J' | 'K')))
                if key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SUPER) =>
            {
                let down = matches!(c, 'j' | 'J');
                let mv = if key.modifiers.contains(KeyModifiers::SUPER) {
                    if down {
                        CursorMove::End
                    } else {
                        CursorMove::Head
                    }
                } else if down {
                    CursorMove::ParagraphForward
                } else {
                    CursorMove::ParagraphBack
                };
                self.ed().move_cursor(mv);
            }
            // typing "--0" expands to a markdown checkbox "- [ ] "
            (false, KeyCode::Char('0'))
                if !key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
                    && self.checkbox_trigger_armed() =>
            {
                let ed = self.ed();
                ed.delete_char(); // the two dashes
                ed.delete_char();
                ed.insert_str("- [ ] ");
                self.note_edit();
            }
            _ => {
                if self.ed().input(Input::from(key)) {
                    self.note_edit();
                }
            }
        }
    }

    /// Toggle a markdown checkbox on the cursor's line (Ctrl+D):
    /// `- [ ]` <-> `- [x]`; a plain `- ` bullet or any other line gains a
    /// `- [ ] ` prefix after its leading whitespace. Rewrites the line via
    /// textarea edits so undo works, then keeps the cursor on the same
    /// character (old column shifted by the prefix growth).
    fn toggle_checkbox(&mut self) {
        // an active selection would make the moves below extend it and
        // delete_line_by_end would then eat the whole selection
        let ed = self.ed();
        ed.cancel_selection();
        let (row, col) = ed.cursor();
        let Some(old) = ed.current_line().map(str::to_string) else {
            return;
        };
        let new = crate::checkbox::toggle_checkbox_line(&old);
        // Head, not Jump(row as u16, _): u16 would truncate past line 65535
        ed.move_cursor(CursorMove::Head);
        if !old.is_empty() {
            // an empty line would delete the newline instead — skip
            ed.delete_line_by_end();
        }
        ed.insert_str(&new);
        let delta = new.chars().count() - old.chars().count();
        let new_col = (col + delta).min(new.chars().count());
        ed.set_cursor(row, new_col);
        self.note_edit();
    }

    /// True when the two chars before the cursor are exactly "--"
    /// (not part of a longer dash run).
    fn checkbox_trigger_armed(&self) -> bool {
        let (_, col) = self.ed_ref().cursor();
        let Some(line) = self.ed_ref().current_line() else {
            return false;
        };
        crate::checkbox::trigger_armed(line, col)
    }

    fn note_edit(&mut self) {
        let tab = &mut self.tabs[self.active];
        // the highlighted matches are stale once the text changes
        tab.search_highlight = None;
        tab.editor.mark_dirty();
        tab.last_edit = Some(Instant::now());
        tab.force_next_save = false;
        self.pending_quit = false;
    }

    fn do_save(&mut self) {
        let tab = &mut self.tabs[self.active];
        let force = tab.force_next_save;
        match tab.editor.save(force) {
            Ok(SaveOutcome::Saved) => {
                self.status = Some("saved".into());
                tab.force_next_save = false;
                self.pending_quit = false;
            }
            Ok(SaveOutcome::Conflict) => {
                self.status = Some("disk changed — Ctrl+S again to overwrite".into());
                tab.force_next_save = true;
            }
            Ok(_) => {}
            Err(e) => self.status = Some(format!("save failed: {e}")),
        }
    }

    /// Save every tab, then quit. The first tab that can't be saved
    /// becomes active with its warning; Ctrl+Q again discards it all.
    fn do_quit(&mut self) {
        if self.pending_quit {
            self.should_quit = true;
            return;
        }
        for i in 0..self.tabs.len() {
            let tab = &mut self.tabs[i];
            let name = tab.name();
            let problem = match tab.editor.save(false) {
                Ok(SaveOutcome::Conflict) => Some(format!(
                    "{name}: disk changed — Ctrl+S to overwrite, Ctrl+Q again to discard"
                )),
                Err(e) => Some(format!("save failed: {e} — Ctrl+Q again to discard")),
                Ok(_) => None,
            };
            if let Some(msg) = problem {
                self.status = Some(msg);
                self.pending_quit = true;
                self.active = i;
                self.focus = Focus::Editor;
                return;
            }
        }
        self.should_quit = true;
    }

    fn prompt_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        // Handle Ctrl+Q and Ctrl+C first - they should quit even in prompts
        if ctrl && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('c')) {
            self.do_quit();
            return;
        }

        if ctrl {
            // Ctrl+J/Ctrl+K move the go-to-file selection
            if let Prompt::GoToFile {
                input,
                candidates,
                selected,
            } = &mut self.prompt
            {
                let n = crate::fuzzy::fuzzy_filter(input, candidates).len();
                match key.code {
                    KeyCode::Char('j') => *selected = (*selected + 1).min(n.saturating_sub(1)),
                    KeyCode::Char('k') => *selected = selected.saturating_sub(1),
                    _ => {}
                }
            }
            return;
        }
        if key.code == KeyCode::Esc {
            self.prompt = Prompt::None;
            return;
        }
        match &mut self.prompt {
            // the cheat sheet: any key closes it and is otherwise consumed
            Prompt::Help => self.prompt = Prompt::None,
            Prompt::NewFile(s) | Prompt::Search(s) => match key.code {
                KeyCode::Backspace => {
                    s.pop();
                }
                KeyCode::Char(c) => s.push(c),
                KeyCode::Enter => {
                    match std::mem::replace(&mut self.prompt, Prompt::None) {
                        Prompt::NewFile(name) => self.submit_new_file(&name),
                        Prompt::Search(query) => {
                            // empty submit repeats the previous search
                            let q = if query.is_empty() {
                                self.last_search.clone()
                            } else {
                                query
                            };
                            self.search_next(&q);
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            Prompt::ConfirmDelete { path, yes } => match key.code {
                KeyCode::Char('j' | 'k') | KeyCode::Down | KeyCode::Up => *yes = !*yes,
                // x again = confirm the delete
                KeyCode::Char('x' | 'X') => {
                    let p = path.clone();
                    self.prompt = Prompt::None;
                    self.delete_file(p);
                }
                KeyCode::Enter => {
                    let (p, yes) = (path.clone(), *yes);
                    self.prompt = Prompt::None;
                    if yes {
                        self.delete_file(p);
                    }
                }
                _ => {}
            },
            Prompt::Rename { input, .. } => match key.code {
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) => input.push(c),
                KeyCode::Enter => {
                    if let Prompt::Rename { path, input } =
                        std::mem::replace(&mut self.prompt, Prompt::None)
                    {
                        self.submit_rename(&path, input.trim());
                    }
                }
                _ => {}
            },
            Prompt::GoToFile {
                input,
                candidates,
                selected,
            } => match key.code {
                KeyCode::Backspace => {
                    input.pop();
                    *selected = 0;
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    *selected = 0;
                }
                KeyCode::Down => {
                    let n = crate::fuzzy::fuzzy_filter(input, candidates).len();
                    *selected = (*selected + 1).min(n.saturating_sub(1));
                }
                KeyCode::Up => *selected = selected.saturating_sub(1),
                KeyCode::Enter => {
                    let target = crate::fuzzy::fuzzy_filter(input, candidates)
                        .get(*selected)
                        .map(|c| c.1.clone());
                    self.prompt = Prompt::None;
                    if let Some(path) = target {
                        self.open_file(path);
                    }
                }
                _ => {}
            },
            Prompt::Settings { rows, selected } => {
                let last = rows.len().saturating_sub(1);
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => *selected = (*selected + 1).min(last),
                    KeyCode::Char('k') | KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('l') | KeyCode::Right => {
                        let delta = if matches!(key.code, KeyCode::Char('l') | KeyCode::Right) {
                            1
                        } else {
                            -1
                        };
                        let row = &mut rows[*selected];
                        let name = row.name;
                        let value = row.step(delta);
                        self.apply_setting(name, &value);
                    }
                    KeyCode::Enter | KeyCode::Char('s') => self.prompt = Prompt::None,
                    _ => {}
                }
            }
            Prompt::MoveFile {
                src,
                dests,
                selected,
            } => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    *selected = (*selected + 1).min(dests.len().saturating_sub(1));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    let (s, d) = (src.clone(), dests[*selected].clone());
                    self.prompt = Prompt::None;
                    self.move_file(s, d);
                }
                _ => {}
            },
            Prompt::None => {}
        }
    }

    fn submit_new_file(&mut self, name: &str) {
        match crate::files::create(&mut self.tree, name) {
            Ok(path) => self.open_file(path),
            Err(e) => self.status = Some(e),
        }
    }

    /// Literal, case-insensitive, wraps around; starts one char after the cursor.
    fn search_next(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let tab = &mut self.tabs[self.active];
        tab.follow_cursor = true;
        // the renderer highlights every (case-insensitive) match of this
        tab.search_highlight = Some(query.to_string());
        let (crow, ccol) = tab.editor.cursor();
        let lines: Vec<String> = tab.editor.lines().to_vec();
        self.last_search = query.to_string();
        match crate::search::next(&lines, (crow, ccol), query) {
            Some((row, cpos)) => {
                // set_cursor guards the u16::MAX bound that Jump takes
                if !tab.editor.set_cursor(row, cpos) {
                    self.status = Some("match is beyond line 65535 — cannot jump".into());
                    return;
                }
                tab.editor.cancel_selection();
            }
            None => {
                self.status = Some(format!("not found: {query}"));
            }
        }
    }
}

/// Whether screen cell (`x`, `y`) lies inside `r`.
fn contains(r: Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

#[cfg(test)]
mod tests;
