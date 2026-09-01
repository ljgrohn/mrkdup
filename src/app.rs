use std::io;
use std::path::PathBuf;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::{CursorMove, DataCursor, Input};

use crate::config::Config;
use crate::editor::{Editor, SaveOutcome};
use crate::tree::Tree;

pub enum Focus {
    Tree,
    Editor,
}

pub enum Prompt {
    None,
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
}

pub struct App {
    pub tree: Tree,
    pub editor: Editor,
    pub config: Config,
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
    pub fn new(root: PathBuf, config: Config) -> io::Result<App> {
        Ok(App {
            tree: Tree::new(root)?,
            editor: Editor::new(),
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
        let Some(name) = src.file_name() else { return };
        let target = dest_dir.join(name);
        if target == src {
            self.status = Some("already there".into());
            return;
        }
        if target.exists() {
            self.status = Some("a file with that name is already there".into());
            return;
        }
        if let Err(e) = std::fs::rename(&src, &target) {
            self.status = Some(format!("move failed: {e}"));
            return;
        }
        if self.editor.path.as_deref() == Some(src.as_path()) {
            self.editor.path = Some(target.clone());
        }
        self.tree.refresh();
        let shown = target
            .strip_prefix(self.tree.root())
            .unwrap_or(&target)
            .to_string_lossy()
            .into_owned();
        self.status = Some(format!("moved to {shown}"));
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
        if name.is_empty() || name.contains('/') || name == ".." {
            self.status = Some("invalid file name".into());
            return;
        }
        let Some(dir) = src.parent() else { return };
        let target = dir.join(name);
        if target == src {
            return; // unchanged
        }
        // on case-insensitive filesystems (macOS default) a case-only
        // rename makes target "exist" — but it's the same file, allow it
        let same_file = target.exists()
            && std::fs::canonicalize(&target).ok() == std::fs::canonicalize(src).ok();
        if target.exists() && !same_file {
            self.status = Some("a file with that name already exists".into());
            return;
        }
        if let Err(e) = std::fs::rename(src, &target) {
            self.status = Some(format!("rename failed: {e}"));
            return;
        }
        if self.editor.path.as_deref() == Some(src) {
            self.editor.path = Some(target.clone());
        }
        // refresh tracks selection by the old (gone) path, so reselect
        self.tree.refresh();
        self.tree.select_path(&target);
        self.status = Some(format!("renamed to {name}"));
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
        if let Err(e) = std::fs::remove_file(&path) {
            self.status = Some(format!("delete failed: {e}"));
            return;
        }
        if self.editor.path.as_deref() == Some(path.as_path()) {
            self.editor = Editor::new();
            self.last_edit = None;
        }
        self.tree.refresh();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        self.status = Some(format!("deleted {name}"));
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
                self.editor.textarea.move_cursor(CursorMove::WordForward);
            }
            (true, KeyCode::Char('k')) => {
                self.editor.textarea.move_cursor(CursorMove::WordBack);
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
                if self.editor.textarea.undo() {
                    self.note_edit();
                }
            }
            (true, KeyCode::Char('y')) => {
                if self.editor.textarea.redo() {
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
                self.editor.textarea.move_cursor(mv);
            }
            // typing "--0" expands to a markdown checkbox "- [ ] "
            (false, KeyCode::Char('0'))
                if !key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
                    && self.checkbox_trigger_armed() =>
            {
                self.editor.textarea.delete_char(); // the two dashes
                self.editor.textarea.delete_char();
                self.editor.textarea.insert_str("- [ ] ");
                self.note_edit();
            }
            _ => {
                if self.editor.textarea.input(Input::from(key)) {
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
        self.editor.textarea.cancel_selection();
        let DataCursor(row, col) = self.editor.textarea.cursor();
        let Some(old) = self.editor.textarea.lines().get(row).cloned() else {
            return;
        };
        let new = toggle_checkbox_line(&old);
        // Head, not Jump(row as u16, _): u16 would truncate past line 65535
        self.editor.textarea.move_cursor(CursorMove::Head);
        if !old.is_empty() {
            // an empty line would delete the newline instead — skip
            self.editor.textarea.delete_line_by_end();
        }
        self.editor.textarea.insert_str(&new);
        let delta = new.chars().count() - old.chars().count();
        let new_col = (col + delta).min(new.chars().count());
        if row <= u16::MAX as usize && new_col <= u16::MAX as usize {
            self.editor
                .textarea
                .move_cursor(CursorMove::Jump(row as u16, new_col as u16));
        }
        self.note_edit();
    }

    /// True when the two chars before the cursor are exactly "--"
    /// (not part of a longer dash run).
    fn checkbox_trigger_armed(&self) -> bool {
        let DataCursor(row, col) = self.editor.textarea.cursor();
        let Some(line) = self.editor.textarea.lines().get(row) else {
            return false;
        };
        let before: Vec<char> = line.chars().take(col).collect();
        col >= 2
            && before[col - 1] == '-'
            && before[col - 2] == '-'
            && (col == 2 || before[col - 3] != '-')
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
        let name = name.trim();
        if name.is_empty() || name.starts_with('/') || name.split('/').any(|part| part == "..") {
            self.status = Some("invalid file name".into());
            return;
        }
        let base = match self.tree.selected_row() {
            Some(r) if r.is_dir => r.path.clone(),
            Some(r) => r.path.parent().unwrap_or(self.tree.root()).to_path_buf(),
            None => self.tree.root().to_path_buf(),
        };
        let path = base.join(name);
        if path.exists() {
            self.status = Some("file already exists".into());
            return;
        }
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                self.status = Some(format!("create failed: {e}"));
                return;
            }
        }
        if let Err(e) = crate::fsutil::atomic_write(&path, b"") {
            self.status = Some(format!("create failed: {e}"));
            return;
        }
        self.tree.refresh();
        self.open_file(path);
    }

    /// Literal, case-insensitive, wraps around; starts one char after the cursor.
    fn search_next(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        // the renderer highlights every (case-insensitive) match of this
        self.search_highlight = Some(query.to_string());
        let DataCursor(crow, ccol) = self.editor.textarea.cursor();
        let lines: Vec<String> = self.editor.textarea.lines().to_vec();
        let n = lines.len();
        for i in 0..=n {
            let row = (crow + i) % n;
            let hay = &lines[row];
            let from_char = if i == 0 { ccol + 1 } else { 0 };
            if let Some(cpos) = find_ci(hay, query, from_char) {
                if row > u16::MAX as usize || cpos > u16::MAX as usize {
                    // Jump takes u16; truncating would land on the wrong line
                    self.status = Some("match is beyond line 65535 — cannot jump".into());
                    self.last_search = query.to_string();
                    return;
                }
                self.editor.textarea.cancel_selection();
                self.editor
                    .textarea
                    .move_cursor(CursorMove::Jump(row as u16, cpos as u16));
                self.last_search = query.to_string();
                return;
            }
        }
        self.status = Some(format!("not found: {query}"));
        self.last_search = query.to_string();
    }
}

/// Case-insensitive literal find: the char index of the first match of
/// `query` in `hay` at or after char index `from_char`.
pub(crate) fn find_ci(hay: &str, query: &str, from_char: usize) -> Option<usize> {
    let h: Vec<char> = hay.chars().collect();
    let q: Vec<char> = query.chars().collect();
    if q.is_empty() || h.len() < q.len() {
        return None;
    }
    let ci_eq = |a: &char, b: &char| a.to_lowercase().eq(b.to_lowercase());
    (from_char..=h.len() - q.len()).find(|&start| {
        h[start..start + q.len()]
            .iter()
            .zip(&q)
            .all(|(a, b)| ci_eq(a, b))
    })
}

/// The checkbox-toggled form of `line`, indentation preserved.
fn toggle_checkbox_line(line: &str) -> String {
    let indent_end = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_end);
    if let Some(tail) = rest.strip_prefix("- [ ]") {
        format!("{indent}- [x]{tail}")
    } else if let Some(tail) = rest.strip_prefix("- [x]") {
        format!("{indent}- [ ]{tail}")
    } else if let Some(tail) = rest.strip_prefix("- [X]") {
        format!("{indent}- [ ]{tail}")
    } else if let Some(tail) = rest.strip_prefix("- ") {
        format!("{indent}- [ ] {tail}")
    } else {
        format!("{indent}- [ ] {rest}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }
    fn fixture(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("mrkdup-app-{tag}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.md"), "hello\nworld\n").unwrap();
        fs::write(root.join("b.md"), "bee\n").unwrap();
        root
    }

    #[test]
    fn starts_focused_on_tree() {
        let app = App::new(fixture("start"), Config::default()).unwrap();
        assert!(matches!(app.focus, Focus::Tree));
    }

    #[test]
    fn enter_opens_file_and_focuses_editor() {
        let mut app = App::new(fixture("open"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // a.md selected first
        assert!(matches!(app.focus, Focus::Editor));
        assert_eq!(app.editor.textarea.lines(), ["hello", "world"]);
    }

    #[test]
    fn esc_returns_to_tree() {
        let mut app = App::new(fixture("esc"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.focus, Focus::Tree));
    }

    #[test]
    fn typing_marks_dirty_and_switching_files_autosaves() {
        let root = fixture("autosave");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // open a.md
        app.handle_key(key(KeyCode::Char('X')));
        assert!(app.editor.dirty);
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('j'))); // b.md
        app.handle_key(key(KeyCode::Enter)); // open b.md -> autosaves a.md
        assert_eq!(
            fs::read_to_string(root.join("a.md")).unwrap(),
            "Xhello\nworld\n"
        );
        assert_eq!(app.editor.textarea.lines(), ["bee"]);
    }

    #[test]
    fn ctrl_z_undoes_and_ctrl_y_redoes() {
        let mut app = App::new(fixture("undo"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('X')));
        assert_eq!(app.editor.textarea.lines()[0], "Xhello");
        app.handle_key(ctrl('z'));
        assert_eq!(app.editor.textarea.lines()[0], "hello");
        app.handle_key(ctrl('y'));
        assert_eq!(app.editor.textarea.lines()[0], "Xhello");
    }

    #[test]
    fn ctrl_b_toggles_tree_and_fixes_focus() {
        let mut app = App::new(fixture("toggle"), Config::default()).unwrap();
        app.handle_key(ctrl('b'));
        assert!(!app.tree_visible);
        assert!(matches!(app.focus, Focus::Editor));
        app.handle_key(ctrl('b'));
        assert!(app.tree_visible);
    }

    #[test]
    fn ctrl_t_hides_editor_and_opening_a_file_reshows_it() {
        let mut app = App::new(fixture("epane"), Config::default()).unwrap();
        app.handle_key(ctrl('t'));
        assert!(!app.editor_visible);
        assert!(app.tree_visible);
        assert!(matches!(app.focus, Focus::Tree));
        app.handle_key(key(KeyCode::Enter)); // open a.md
        assert!(app.editor_visible);
        assert!(matches!(app.focus, Focus::Editor));
    }

    #[test]
    fn panes_can_never_both_be_hidden() {
        let mut app = App::new(fixture("panes"), Config::default()).unwrap();
        app.handle_key(ctrl('t')); // editor hidden
        app.handle_key(ctrl('b')); // hide tree -> editor must come back
        assert!(app.editor_visible);
        assert!(!app.tree_visible);
        app.handle_key(ctrl('t')); // hide editor -> tree must come back
        assert!(app.tree_visible);
        assert!(!app.editor_visible);
    }

    #[test]
    fn ctrl_q_saves_and_quits() {
        let root = fixture("quit");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('Q')));
        app.handle_key(ctrl('q'));
        assert!(app.should_quit);
        assert_eq!(
            fs::read_to_string(root.join("a.md")).unwrap(),
            "Qhello\nworld\n"
        );
    }

    #[test]
    fn new_file_prompt_creates_and_opens() {
        let root = fixture("newfile");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('n')));
        for c in "notes.md".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert!(root.join("notes.md").exists());
        assert!(matches!(app.focus, Focus::Editor));
    }

    #[test]
    fn search_jumps_to_match() {
        let mut app = App::new(fixture("search"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // a.md: hello / world
        app.handle_key(ctrl('f'));
        for c in "wor".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.editor.textarea.cursor(), (1, 0));
    }

    #[test]
    fn empty_search_repeats_last_search() {
        let mut app = App::new(fixture("repeat"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // hello / world: two 'l'-runs
        app.handle_key(ctrl('f'));
        app.handle_key(key(KeyCode::Char('l')));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.editor.textarea.cursor(), (0, 2));
        app.handle_key(ctrl('f'));
        app.handle_key(key(KeyCode::Enter)); // empty -> repeat "l"
        assert_eq!(app.editor.textarea.cursor(), (0, 3));
    }

    #[test]
    fn ctrl_g_jumps_to_the_next_match_of_the_last_search() {
        let mut app = App::new(fixture("next"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // hello / world
        app.handle_key(ctrl('f'));
        app.handle_key(key(KeyCode::Char('l')));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.editor.textarea.cursor(), (0, 2));
        app.handle_key(ctrl('g'));
        assert_eq!(app.editor.textarea.cursor(), (0, 3));
        app.handle_key(ctrl('g'));
        assert_eq!(app.editor.textarea.cursor(), (1, 3)); // "world"
    }

    #[test]
    fn ctrl_g_without_a_previous_search_shows_a_status_message() {
        let mut app = App::new(fixture("next-none"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('g'));
        assert_eq!(app.editor.textarea.cursor(), (0, 0)); // didn't move
        assert!(app.status.as_deref().is_some_and(|s| s.contains("search")));
    }

    #[test]
    fn search_submit_arms_the_renderer_highlight() {
        let mut app = App::new(fixture("hl"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('f'));
        for c in "wor".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.search_highlight.as_deref(), Some("wor"));
    }

    #[test]
    fn opening_a_file_clears_the_search_highlight() {
        let mut app = App::new(fixture("hl-clear"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // a.md
        app.handle_key(ctrl('f'));
        app.handle_key(key(KeyCode::Char('l')));
        app.handle_key(key(KeyCode::Enter));
        assert!(app.search_highlight.is_some());
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('j'))); // b.md
        app.handle_key(key(KeyCode::Enter));
        assert!(app.search_highlight.is_none());
    }

    #[test]
    fn search_query_with_regex_metachars_matches_literally() {
        let root = fixture("meta");
        fs::write(root.join("a.md"), "price (a.b) here\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('f'));
        for c in "(a.b)".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.editor.textarea.cursor(), (0, 6));
        // and the renderer's matcher is literal too: "axb" is no match
        assert_eq!(find_ci("price axb here", "(a.b)", 0), None);
    }

    #[test]
    fn dash_reroots_tree_at_parent() {
        let root = fixture("ascend");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('-')));
        assert_eq!(
            app.tree.root(),
            root.canonicalize().unwrap().parent().unwrap()
        );
        assert_eq!(
            app.tree.selected_row().unwrap().path,
            root.canonicalize().unwrap()
        );
    }

    #[test]
    fn shift_jk_types_capital_letters() {
        let mut app = App::new(fixture("motion"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // hello / world, cursor (0,0)
        app.handle_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        app.handle_key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
        assert_eq!(app.editor.textarea.lines()[0], "JKhello"); // typed, not moved
    }

    #[test]
    fn alt_jk_jumps_by_paragraph() {
        let root = fixture("para");
        fs::write(root.join("a.md"), "one\n\ntwo\n\nthree\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT));
        let DataCursor(row, _) = app.editor.textarea.cursor();
        assert!(row >= 1); // moved past the blank line
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT));
        assert_eq!(app.editor.textarea.cursor(), (0, 0));
    }

    #[test]
    fn super_jk_jumps_to_line_end_and_start() {
        let mut app = App::new(fixture("linejump"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // hello
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SUPER));
        assert_eq!(app.editor.textarea.cursor(), (0, 5)); // end of "hello"
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::SUPER));
        assert_eq!(app.editor.textarea.cursor(), (0, 0));
    }

    #[test]
    fn dash_dash_zero_expands_to_checkbox() {
        let mut app = App::new(fixture("expand0"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // "hello", cursor at (0,0)
        app.handle_key(key(KeyCode::Char('-')));
        app.handle_key(key(KeyCode::Char('-')));
        app.handle_key(key(KeyCode::Char('0')));
        assert_eq!(app.editor.textarea.lines()[0], "- [ ] hello");
        assert_eq!(app.editor.textarea.cursor(), (0, 6)); // ready to type the item
        assert!(app.editor.dirty);
    }

    #[test]
    fn plain_zero_still_types_zero() {
        let mut app = App::new(fixture("zero"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('0')));
        assert_eq!(app.editor.textarea.lines()[0], "0hello");
    }

    #[test]
    fn triple_dash_zero_does_not_expand() {
        let mut app = App::new(fixture("dashes"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        for c in "---0".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.editor.textarea.lines()[0], "---0hello");
    }

    #[test]
    fn plain_p_and_q_work_in_tree() {
        let root = fixture("plain-keys");
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('p')));
        assert!(matches!(app.prompt, Prompt::GoToFile { .. }));
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn plain_p_and_q_still_type_in_editor() {
        let root = fixture("plain-keys-editor");
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // open a.md ("hello"...)
        app.handle_key(key(KeyCode::Char('q')));
        app.handle_key(key(KeyCode::Char('p')));
        assert_eq!(app.editor.textarea.lines()[0], "qphello");
        assert!(!app.should_quit);
    }

    #[test]
    fn search_is_case_insensitive_both_ways() {
        let root = fixture("search-ci");
        fs::write(root.join("a.md"), "Ship it\nfriend ship\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        // all-caps query finds the lowercase occurrence first (search
        // starts one char after the cursor, skipping "Ship" at 0:0)...
        app.handle_key(ctrl('f'));
        for c in "SHIP".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.editor.textarea.cursor(), (1, 7));
        // ...and Ctrl+G wraps around to the capitalized one
        app.handle_key(ctrl('g'));
        assert_eq!(app.editor.textarea.cursor(), (0, 0));
    }

    #[test]
    fn find_ci_handles_unicode_and_offsets() {
        assert_eq!(find_ci("héLLo héllo", "Éllo", 0), Some(1));
        assert_eq!(find_ci("héLLo héllo", "Éllo", 2), Some(7));
        assert_eq!(find_ci("abc", "zzz", 0), None);
        assert_eq!(find_ci("abc", "", 0), None);
        assert_eq!(find_ci("ab", "abc", 0), None);
    }

    #[test]
    fn ctrl_jk_move_by_word_without_deleting() {
        let root = fixture("word-motion");
        fs::write(root.join("a.md"), "alpha bravo charlie\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('j'));
        let DataCursor(_, col1) = app.editor.textarea.cursor();
        assert!(col1 > 0); // advanced
        app.handle_key(ctrl('j'));
        let DataCursor(_, col2) = app.editor.textarea.cursor();
        assert!(col2 > col1);
        app.handle_key(ctrl('k'));
        assert_eq!(app.editor.textarea.cursor(), (0, col1));
        // nothing was deleted (Ctrl+K used to be kill-to-end-of-line)
        assert_eq!(app.editor.textarea.lines()[0], "alpha bravo charlie");
        assert!(!app.editor.dirty);
    }

    #[test]
    fn ctrl_d_checks_an_unchecked_checkbox() {
        let root = fixture("cb-check");
        fs::write(root.join("a.md"), "- [ ] milk\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines()[0], "- [x] milk");
        assert_eq!(app.editor.textarea.cursor(), (0, 0)); // same width: cursor stays
        assert!(app.editor.dirty);
    }

    #[test]
    fn ctrl_d_unchecks_a_checked_checkbox() {
        let root = fixture("cb-uncheck");
        fs::write(root.join("a.md"), "- [x] milk\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines()[0], "- [ ] milk");
    }

    #[test]
    fn ctrl_d_unchecks_uppercase_checked_checkbox() {
        let root = fixture("cb-uncheck-upper");
        fs::write(root.join("a.md"), "- [X] milk\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines()[0], "- [ ] milk");
    }

    #[test]
    fn ctrl_d_with_active_selection_only_touches_cursor_line() {
        let root = fixture("cb-selection");
        fs::write(root.join("a.md"), "alpha\nbravo\ncharlie\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        // select two lines with Shift+Down, then toggle
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT));
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT));
        app.handle_key(ctrl('d'));
        assert_eq!(
            app.editor.textarea.lines(),
            ["alpha", "bravo", "- [ ] charlie"]
        );
    }

    #[test]
    fn typing_with_no_file_open_is_ignored() {
        let root = fixture("no-file-typing");
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(ctrl('b')); // hide tree -> editor focus, no file
        app.handle_key(key(KeyCode::Char('x')));
        assert_eq!(app.editor.textarea.lines(), [""]); // nothing typed
        assert!(!app.editor.dirty);
        assert!(app.status.is_some()); // told the user why
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.focus, Focus::Tree)); // Esc still escapes
    }

    #[test]
    fn ctrl_d_turns_a_bullet_into_a_checkbox() {
        let root = fixture("cb-bullet");
        fs::write(root.join("a.md"), "- milk\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines()[0], "- [ ] milk");
    }

    #[test]
    fn ctrl_d_prefixes_a_plain_line_and_shifts_the_cursor() {
        let root = fixture("cb-plain");
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // "hello", cursor (0,0)
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines()[0], "- [ ] hello");
        assert_eq!(app.editor.textarea.cursor(), (0, 6)); // still on the 'h'
    }

    #[test]
    fn ctrl_d_preserves_indentation() {
        let root = fixture("cb-indent");
        fs::write(root.join("a.md"), "  - [ ] a\n    plain\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines()[0], "  - [x] a");
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines()[1], "    - [ ] plain");
    }

    #[test]
    fn ctrl_d_on_an_empty_line_does_not_join_the_next_line() {
        let root = fixture("cb-empty");
        fs::write(root.join("a.md"), "\nworld\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines(), ["- [ ] ", "world"]);
    }

    #[test]
    fn ctrl_d_is_undoable() {
        let root = fixture("cb-undo");
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // "hello"
        app.handle_key(ctrl('d'));
        assert_eq!(app.editor.textarea.lines()[0], "- [ ] hello");
        // the toggle is a delete + an insert, so two undo steps
        app.handle_key(ctrl('z'));
        app.handle_key(ctrl('z'));
        assert_eq!(app.editor.textarea.lines()[0], "hello");
    }

    #[test]
    fn shift_tab_in_editor_returns_to_tree_without_typing() {
        let mut app = App::new(fixture("backtab"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // open a.md
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert!(matches!(app.focus, Focus::Tree));
        assert_eq!(app.editor.textarea.lines(), ["hello", "world"]); // unchanged
        assert!(!app.editor.dirty);
    }

    #[test]
    fn plus_makes_selected_folder_the_root() {
        let root = fixture("mkroot");
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/x.md"), "x\n").unwrap();
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        // docs/ sorts first (dirs before files), so it's already selected
        app.handle_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::SHIFT));
        assert_eq!(app.tree.root(), root.canonicalize().unwrap().join("docs"));
    }

    #[test]
    fn shift_x_confirm_no_by_default_keeps_file() {
        let root = fixture("del-no");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('x')));
        assert!(matches!(
            app.prompt,
            Prompt::ConfirmDelete { yes: false, .. }
        ));
        app.handle_key(key(KeyCode::Enter)); // No selected -> just closes
        assert!(matches!(app.prompt, Prompt::None));
        assert!(root.join("a.md").exists());
    }

    #[test]
    fn shift_x_then_yes_deletes_file() {
        let root = fixture("del-yes");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Char('j'))); // move highlight to Yes
        app.handle_key(key(KeyCode::Enter));
        assert!(!root.join("a.md").exists());
        assert!(!app.tree.rows().iter().any(|r| r.name == "a.md"));
    }

    #[test]
    fn shift_x_inside_popup_deletes_immediately() {
        let root = fixture("del-xx");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Char('x')));
        assert!(!root.join("a.md").exists());
    }

    #[test]
    fn esc_closes_delete_popup_without_deleting() {
        let root = fixture("del-esc");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(root.join("a.md").exists());
    }

    #[test]
    fn deleting_the_open_file_clears_the_editor() {
        let root = fixture("del-open");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // open a.md
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Char('k'))); // k also toggles to Yes
        app.handle_key(key(KeyCode::Enter));
        assert!(app.editor.path.is_none());
        assert_eq!(app.editor.textarea.lines(), [""]);
    }

    #[test]
    fn shift_x_on_directory_is_refused() {
        let root = fixture("del-dir");
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/x.md"), "x\n").unwrap();
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        // docs/ sorts first, so it's selected
        app.handle_key(key(KeyCode::Char('x')));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(app.status.is_some());
        assert!(root.join("docs").exists());
    }

    #[test]
    fn m_moves_file_into_chosen_directory() {
        let root = fixture("move");
        fs::create_dir_all(root.join("docs")).unwrap();
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('j'))); // docs(0) -> a.md(1)
        app.handle_key(key(KeyCode::Char('m')));
        assert!(matches!(app.prompt, Prompt::MoveFile { .. }));
        app.handle_key(key(KeyCode::Char('j'))); // root -> docs
        app.handle_key(key(KeyCode::Enter));
        assert!(root.join("docs/a.md").exists());
        assert!(!root.join("a.md").exists());
    }

    #[test]
    fn m_on_directory_is_refused() {
        let root = fixture("move-dir");
        fs::create_dir_all(root.join("docs")).unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('m'))); // docs selected
        assert!(matches!(app.prompt, Prompt::None));
        assert!(app.status.is_some());
    }

    #[test]
    fn moving_the_open_file_keeps_editing_it_at_the_new_path() {
        let root = fixture("move-open");
        fs::create_dir_all(root.join("docs")).unwrap();
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Enter)); // open a.md
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('m')));
        app.handle_key(key(KeyCode::Char('j'))); // docs
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            app.editor.path.as_deref(),
            Some(root.canonicalize().unwrap().join("docs/a.md").as_path())
        );
        // edits still save to the new location
        app.focus = Focus::Editor;
        app.handle_key(key(KeyCode::Char('Z')));
        app.handle_key(ctrl('s'));
        assert!(fs::read_to_string(root.join("docs/a.md"))
            .unwrap()
            .starts_with('Z'));
    }

    #[test]
    fn move_to_same_directory_is_rejected() {
        let root = fixture("move-same");
        fs::create_dir_all(root.join("docs")).unwrap();
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('j'))); // a.md
        app.handle_key(key(KeyCode::Char('m')));
        app.handle_key(key(KeyCode::Enter)); // first dest is root = current dir
        assert!(root.join("a.md").exists());
        assert!(app.status.is_some());
    }

    #[test]
    fn u_refreshes_tree_to_pick_up_external_files() {
        let root = fixture("refresh-key");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        fs::write(root.join("new.md"), "n\n").unwrap();
        assert!(!app.tree.rows().iter().any(|r| r.name == "new.md"));
        app.handle_key(key(KeyCode::Char('u')));
        assert!(app.tree.rows().iter().any(|r| r.name == "new.md"));
    }

    #[test]
    fn tick_auto_refreshes_tree_periodically() {
        let root = fixture("refresh-tick");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        fs::write(root.join("new.md"), "n\n").unwrap();
        app.last_tree_refresh = std::time::Instant::now() - std::time::Duration::from_secs(3);
        app.tick();
        assert!(app.tree.rows().iter().any(|r| r.name == "new.md"));
    }

    /// Open the rename popup, erase `erase` chars of the prefill, type
    /// `name`, and submit.
    fn rename_to(app: &mut App, erase: usize, name: &str) {
        app.handle_key(key(KeyCode::Char('r')));
        for _ in 0..erase {
            app.handle_key(key(KeyCode::Backspace));
        }
        for c in name.chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
    }

    #[test]
    fn shift_r_opens_rename_popup_prefilled_with_the_file_name() {
        let mut app = App::new(fixture("ren-open"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('r')));
        match &app.prompt {
            Prompt::Rename { input, .. } => assert_eq!(input, "a.md"),
            _ => panic!("expected rename prompt"),
        }
    }

    #[test]
    fn rename_renames_the_file_and_keeps_it_selected() {
        let root = fixture("ren-do");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        rename_to(&mut app, 4, "z.md"); // a.md -> z.md (sorts after b.md)
        assert!(root.join("z.md").exists());
        assert!(!root.join("a.md").exists());
        assert_eq!(app.tree.selected_row().unwrap().name, "z.md");
    }

    #[test]
    fn rename_to_case_variant_of_itself_works() {
        // on case-insensitive filesystems (macOS default) A.md "exists"
        // when a.md does — a case-only rename must still go through
        let root = fixture("ren-case");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        rename_to(&mut app, 4, "A.md");
        assert!(root.join("A.md").exists());
        let names: Vec<String> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"A.md".to_string()));
        assert!(!names.contains(&"a.md".to_string()));
    }

    #[test]
    fn rename_to_existing_name_is_rejected() {
        let root = fixture("ren-exists");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        rename_to(&mut app, 4, "b.md");
        assert!(root.join("a.md").exists());
        assert!(root.join("b.md").exists());
        assert!(app.status.as_deref().is_some_and(|s| s.contains("exists")));
    }

    #[test]
    fn rename_to_invalid_names_is_rejected() {
        let root = fixture("ren-invalid");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        for name in ["docs/x.md", "..", ""] {
            rename_to(&mut app, 4, name);
            assert!(root.join("a.md").exists(), "rejected rename to {name:?}");
            assert!(
                app.status.as_deref().is_some_and(|s| s.contains("invalid")),
                "status for {name:?}"
            );
        }
    }

    #[test]
    fn shift_r_on_directory_is_refused() {
        let root = fixture("ren-dir");
        fs::create_dir_all(root.join("docs")).unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        // docs/ sorts first, so it's selected
        app.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(app.status.as_deref().is_some_and(|s| s.contains("rename")));
    }

    #[test]
    fn renaming_the_open_file_keeps_editing_it_at_the_new_path() {
        let root = fixture("ren-open-file");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // open a.md
        app.handle_key(key(KeyCode::Esc));
        rename_to(&mut app, 4, "z.md");
        assert_eq!(
            app.editor.path.as_deref(),
            Some(root.canonicalize().unwrap().join("z.md").as_path())
        );
        // edits still save to the new name
        app.focus = Focus::Editor;
        app.handle_key(key(KeyCode::Char('Z')));
        app.handle_key(ctrl('s'));
        assert!(fs::read_to_string(root.join("z.md"))
            .unwrap()
            .starts_with('Z'));
    }

    #[test]
    fn esc_closes_rename_popup_without_renaming() {
        let root = fixture("ren-esc");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('r')));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(root.join("a.md").exists());
    }

    #[test]
    fn ctrl_p_opens_go_to_file_with_text_files_from_the_whole_root() {
        let root = fixture("gtf-open");
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/deep.md"), "d\n").unwrap();
        fs::write(root.join("bin.dat"), b"\x00\x01").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(ctrl('p'));
        match &app.prompt {
            Prompt::GoToFile { candidates, .. } => {
                let rels: Vec<&str> = candidates.iter().map(|c| c.0.as_str()).collect();
                assert!(rels.contains(&"a.md"));
                assert!(rels.contains(&"docs/deep.md")); // walks subdirs, root-relative
                assert!(!rels.iter().any(|r| r.contains("bin.dat"))); // text files only
                assert!(!rels.iter().any(|s| s.contains('\\'))); // no backslashes on any OS
            }
            _ => panic!("expected go-to-file prompt"),
        }
    }

    #[test]
    fn ctrl_p_typing_filters_and_enter_opens_the_top_match() {
        let root = fixture("gtf-enter");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(ctrl('p'));
        app.handle_key(key(KeyCode::Char('b')));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(matches!(app.focus, Focus::Editor));
        assert_eq!(
            app.editor.path.as_deref(),
            Some(root.canonicalize().unwrap().join("b.md").as_path())
        );
    }

    #[test]
    fn ctrl_p_selection_moves_with_arrows_and_ctrl_jk() {
        let mut app = App::new(fixture("gtf-move"), Config::default()).unwrap();
        app.handle_key(ctrl('p')); // empty query: a.md, b.md in order
        app.handle_key(key(KeyCode::Down));
        match &app.prompt {
            Prompt::GoToFile { selected, .. } => assert_eq!(*selected, 1),
            _ => panic!("expected go-to-file prompt"),
        }
        app.handle_key(ctrl('k'));
        match &app.prompt {
            Prompt::GoToFile { selected, .. } => assert_eq!(*selected, 0),
            _ => panic!("expected go-to-file prompt"),
        }
        app.handle_key(ctrl('j'));
        app.handle_key(key(KeyCode::Enter)); // second result = b.md
        assert!(app
            .editor
            .path
            .as_deref()
            .is_some_and(|p| p.ends_with("b.md")));
    }

    #[test]
    fn ctrl_p_from_the_editor_autosaves_before_opening() {
        let root = fixture("gtf-autosave");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // open a.md
        app.handle_key(key(KeyCode::Char('X')));
        app.handle_key(ctrl('p')); // global: works from editor focus too
        app.handle_key(key(KeyCode::Char('b')));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            fs::read_to_string(root.join("a.md")).unwrap(),
            "Xhello\nworld\n"
        );
        assert_eq!(app.editor.textarea.lines(), ["bee"]);
    }

    #[test]
    fn ctrl_p_honors_the_tree_hidden_setting() {
        let root = fixture("gtf-hidden");
        fs::write(root.join(".secret.md"), "s\n").unwrap();
        let mut app = App::new(root, Config::default()).unwrap();
        app.handle_key(ctrl('p'));
        match &app.prompt {
            Prompt::GoToFile { candidates, .. } => {
                assert!(!candidates.iter().any(|c| c.0.contains(".secret.md")));
            }
            _ => panic!("expected go-to-file prompt"),
        }
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('.'))); // tree: show hidden
        app.handle_key(ctrl('p'));
        match &app.prompt {
            Prompt::GoToFile { candidates, .. } => {
                assert!(candidates.iter().any(|c| c.0.contains(".secret.md")));
            }
            _ => panic!("expected go-to-file prompt"),
        }
    }

    #[test]
    fn esc_closes_go_to_file_without_opening() {
        let mut app = App::new(fixture("gtf-esc"), Config::default()).unwrap();
        app.handle_key(ctrl('p'));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(app.editor.path.is_none());
    }

    #[test]
    fn ctrl_p_does_not_fire_inside_another_prompt() {
        let mut app = App::new(fixture("gtf-nested"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('n'))); // NewFile prompt
        app.handle_key(ctrl('p'));
        assert!(matches!(app.prompt, Prompt::NewFile(_)));
    }

    #[test]
    fn enter_with_no_go_to_file_match_just_closes() {
        let mut app = App::new(fixture("gtf-nomatch"), Config::default()).unwrap();
        app.handle_key(ctrl('p'));
        for c in "qqq".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(app.editor.path.is_none());
    }

    #[test]
    fn tick_honors_the_configured_autosave_delay() {
        let root = fixture("cfg-autosave");
        let (cfg, _) = crate::config::parse("autosave_seconds = 300\n");
        let mut app = App::new(root, cfg).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('Y')));
        app.last_edit = Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
        app.tick();
        assert!(app.editor.dirty); // 5s idle < the configured 300s
    }

    #[test]
    fn tick_honors_the_configured_tree_refresh_interval() {
        let root = fixture("cfg-refresh");
        let (cfg, _) = crate::config::parse("tree_refresh_seconds = 300\n");
        let mut app = App::new(root.clone(), cfg).unwrap();
        fs::write(root.join("new.md"), "n\n").unwrap();
        app.last_tree_refresh = std::time::Instant::now() - std::time::Duration::from_secs(5);
        app.tick();
        // 5s < the configured 300s: no refresh yet
        assert!(!app.tree.rows().iter().any(|r| r.name == "new.md"));
    }

    #[test]
    fn tick_idle_autosaves() {
        let root = fixture("tick");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('Y')));
        app.last_edit = Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
        app.tick();
        assert!(!app.editor.dirty);
        assert_eq!(
            fs::read_to_string(root.join("a.md")).unwrap(),
            "Yhello\nworld\n"
        );
    }

    #[test]
    fn ctrl_q_quits_inside_newfile_prompt() {
        let mut app = App::new(fixture("prompt-quit-clean"), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Char('n'))); // open NewFile prompt
        assert!(matches!(app.prompt, Prompt::NewFile(_)));
        app.handle_key(ctrl('q')); // Ctrl+Q should quit even inside prompt
        assert!(app.should_quit);
    }

    #[test]
    fn ctrl_q_saves_and_quits_inside_search_prompt() {
        let root = fixture("prompt-quit-dirty");
        let mut app = App::new(root.clone(), Config::default()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // open a.md
        app.handle_key(key(KeyCode::Char('Q'))); // make it dirty
        app.handle_key(ctrl('f')); // open Search prompt (from editor)
        assert!(matches!(app.prompt, Prompt::Search(_)));
        app.handle_key(ctrl('q')); // Ctrl+Q should save and quit
        assert!(app.should_quit);
        assert_eq!(
            fs::read_to_string(root.join("a.md")).unwrap(),
            "Qhello\nworld\n"
        );
    }
}
