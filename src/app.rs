use std::io;
use std::path::PathBuf;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::{CursorMove, Input};

use crate::config::Config;
use crate::editor::{Editor, SaveOutcome};
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
    pub editor: Editor,
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
    pub last_edit: Option<Instant>,
    pub last_tree_refresh: Instant,
    /// vertical scroll of the editor renderer, in visual rows
    pub editor_scroll: usize,
    /// the query whose matches the renderer highlights (cleared on open)
    pub search_highlight: Option<String>,
    pending_quit: bool,
    force_next_save: bool,
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
        let mut app = App::new_with_theme(root, config, theme)?;
        app.config_dir = None; // tests must never write the user's real config
        Ok(app)
    }

    /// Same as `new`, but with an already-resolved `Theme` (e.g. one
    /// loaded from disk via `theme::load`, overlay files and all)
    /// instead of looking one up by `config.theme_name` again.
    pub fn new_with_theme(root: PathBuf, config: Config, theme: Theme) -> io::Result<App> {
        Ok(App {
            tree: Tree::new(root)?,
            editor: Editor::new(),
            theme,
            config_dir: crate::config::config_dir(),
            config,
            focus: Focus::Tree,
            tree_visible: true,
            editor_visible: true,
            prompt: Prompt::None,
            status: None,
            tree_scroll: 0,
            should_quit: false,
            last_edit: None,
            last_tree_refresh: Instant::now(),
            editor_scroll: 0,
            search_highlight: None,
            pending_quit: false,
            force_next_save: false,
            last_search: String::new(),
        })
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
        if self.editor.dirty
            && self
                .last_edit
                .is_some_and(|t| t.elapsed() >= self.config.autosave())
        {
            match self.editor.save(false) {
                Ok(SaveOutcome::Saved) => self.status = Some("saved".into()),
                Ok(SaveOutcome::Conflict) => {
                    self.status = Some("disk changed — Ctrl+S again to overwrite".into());
                    self.force_next_save = true;
                    self.last_edit = None; // don't retry until the next edit
                }
                Ok(_) => {}
                Err(e) => self.status = Some(format!("save failed: {e}")),
            }
        }
        match self.editor.check_external() {
            Ok(true) => self.status = Some("reloaded (changed on disk)".into()),
            Ok(false) => {}
            Err(e) => self.status = Some(format!("reload failed: {e}")),
        }
        // pick up files created/removed outside the app; the rebuild
        // only walks expanded directories, so this stays cheap
        if self.last_tree_refresh.elapsed() >= self.config.tree_refresh() {
            self.tree.refresh();
            self.last_tree_refresh = Instant::now();
        }
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
        match crate::files::move_to(&mut self.tree, &mut self.editor, &src, &dest_dir) {
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
        match crate::files::rename(&mut self.tree, &mut self.editor, src, name) {
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
    /// `default` if that name isn't in the list).
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
        self.prompt = Prompt::Settings {
            rows: vec![SettingRow {
                name: "theme",
                choices,
                index,
            }],
            selected: 0,
        };
    }

    /// A settings row changed: apply the new value live and persist it.
    /// Only `theme` exists today. The theme goes through `load_from` so
    /// `themes/<name>` and the overlay file both apply, exactly as at
    /// startup; the config file is rewritten in place.
    fn apply_setting(&mut self, row: &str, value: &str) {
        if row != "theme" {
            return;
        }
        let (theme, warnings, saved) = match &self.config_dir {
            Some(dir) => {
                let (theme, warnings) = crate::theme::load_from(value, dir);
                let saved = crate::config::save_theme_name_to(&dir.join("config"), value)
                    .map_err(|e| e.to_string());
                (theme, warnings, saved)
            }
            None => (
                Theme::named(value),
                Vec::new(),
                Err("no config dir".to_string()),
            ),
        };
        self.theme = theme;
        self.config.theme_name = value.to_string();
        self.status = Some(match (saved, warnings.first()) {
            (Ok(()), None) => format!("theme: {value}"),
            (Ok(()), Some(w)) => format!("theme: {value} — {w}"),
            (Err(e), _) => format!("theme: {value} (not saved: {e})"),
        });
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
        let was_open = self.editor.path.as_deref() == Some(path.as_path());
        match crate::files::delete(&mut self.tree, &mut self.editor, &path) {
            Ok(status) => {
                self.status = Some(status);
                if was_open {
                    self.last_edit = None;
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

    /// Autosave the current buffer, then open `path` and focus the editor.
    fn open_file(&mut self, path: PathBuf) {
        match self.editor.save(false) {
            Ok(SaveOutcome::Conflict) => {
                self.status = Some("unsaved changes conflict with disk — Ctrl+S to resolve".into());
                self.force_next_save = true;
                self.focus = Focus::Editor;
                return;
            }
            Err(e) => {
                self.status = Some(format!("save failed: {e}"));
                return;
            }
            Ok(_) => {}
        }
        match self.editor.open(&path) {
            Ok(()) => {
                // no stale search highlight or scroll carries into the
                // newly opened file
                self.search_highlight = None;
                self.editor_scroll = 0;
                self.focus = Focus::Editor;
                self.editor_visible = true;
                self.force_next_save = false;
                self.pending_quit = false;
                self.last_edit = None;
            }
            Err(e) => self.status = Some(format!("open failed: {e}")),
        }
    }

    fn editor_key(&mut self, key: KeyEvent) {
        // no file open: the welcome pane covers the textarea, so typing
        // would silently go into a buffer that can never be saved
        if self.editor.path.is_none() {
            match key.code {
                KeyCode::Esc | KeyCode::BackTab => self.focus = Focus::Tree,
                _ => self.status = Some("no file open — pick one in the tree (Esc)".into()),
            }
            return;
        }
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
                self.editor.move_cursor(CursorMove::WordForward);
            }
            (true, KeyCode::Char('k')) => {
                self.editor.move_cursor(CursorMove::WordBack);
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
                if self.editor.undo() {
                    self.note_edit();
                }
            }
            (true, KeyCode::Char('y')) => {
                if self.editor.redo() {
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
                self.editor.move_cursor(mv);
            }
            // typing "--0" expands to a markdown checkbox "- [ ] "
            (false, KeyCode::Char('0'))
                if !key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
                    && self.checkbox_trigger_armed() =>
            {
                self.editor.delete_char(); // the two dashes
                self.editor.delete_char();
                self.editor.insert_str("- [ ] ");
                self.note_edit();
            }
            _ => {
                if self.editor.input(Input::from(key)) {
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
        self.editor.cancel_selection();
        let (row, col) = self.editor.cursor();
        let Some(old) = self.editor.current_line().map(str::to_string) else {
            return;
        };
        let new = crate::checkbox::toggle_checkbox_line(&old);
        // Head, not Jump(row as u16, _): u16 would truncate past line 65535
        self.editor.move_cursor(CursorMove::Head);
        if !old.is_empty() {
            // an empty line would delete the newline instead — skip
            self.editor.delete_line_by_end();
        }
        self.editor.insert_str(&new);
        let delta = new.chars().count() - old.chars().count();
        let new_col = (col + delta).min(new.chars().count());
        self.editor.set_cursor(row, new_col);
        self.note_edit();
    }

    /// True when the two chars before the cursor are exactly "--"
    /// (not part of a longer dash run).
    fn checkbox_trigger_armed(&self) -> bool {
        let (_, col) = self.editor.cursor();
        let Some(line) = self.editor.current_line() else {
            return false;
        };
        crate::checkbox::trigger_armed(line, col)
    }

    fn note_edit(&mut self) {
        self.editor.mark_dirty();
        self.last_edit = Some(Instant::now());
        self.force_next_save = false;
        self.pending_quit = false;
    }

    fn do_save(&mut self) {
        let force = self.force_next_save;
        match self.editor.save(force) {
            Ok(SaveOutcome::Saved) => {
                self.status = Some("saved".into());
                self.force_next_save = false;
                self.pending_quit = false;
            }
            Ok(SaveOutcome::Conflict) => {
                self.status = Some("disk changed — Ctrl+S again to overwrite".into());
                self.force_next_save = true;
            }
            Ok(_) => {}
            Err(e) => self.status = Some(format!("save failed: {e}")),
        }
    }

    fn do_quit(&mut self) {
        if self.pending_quit {
            self.should_quit = true;
            return;
        }
        match self.editor.save(false) {
            Ok(SaveOutcome::Conflict) => {
                self.status =
                    Some("disk changed — Ctrl+S to overwrite, Ctrl+Q again to discard".into());
                self.pending_quit = true;
            }
            Err(e) => {
                self.status = Some(format!("save failed: {e} — Ctrl+Q again to discard"));
                self.pending_quit = true;
            }
            Ok(_) => self.should_quit = true,
        }
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
        // the renderer highlights every (case-insensitive) match of this
        self.search_highlight = Some(query.to_string());
        let (crow, ccol) = self.editor.cursor();
        let lines: Vec<String> = self.editor.lines().to_vec();
        self.last_search = query.to_string();
        match crate::search::next(&lines, (crow, ccol), query) {
            Some((row, cpos)) => {
                // set_cursor guards the u16::MAX bound that Jump takes
                if !self.editor.set_cursor(row, cpos) {
                    self.status = Some("match is beyond line 65535 — cannot jump".into());
                    return;
                }
                self.editor.cancel_selection();
            }
            None => {
                self.status = Some(format!("not found: {query}"));
            }
        }
    }
}

#[cfg(test)]
mod tests;
