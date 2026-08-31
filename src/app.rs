use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::{CursorMove, DataCursor, Input};

use crate::editor::{Editor, SaveOutcome};
use crate::tree::Tree;

const IDLE_AUTOSAVE: Duration = Duration::from_secs(2);
const TREE_REFRESH: Duration = Duration::from_secs(2);

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
}

pub struct App {
    pub tree: Tree,
    pub editor: Editor,
    pub focus: Focus,
    pub tree_visible: bool,
    pub editor_visible: bool,
    pub prompt: Prompt,
    pub status: Option<String>,
    pub tree_scroll: usize,
    pub should_quit: bool,
    pub last_edit: Option<Instant>,
    pub last_tree_refresh: Instant,
    pending_quit: bool,
    force_next_save: bool,
    last_search: String,
}

impl App {
    pub fn new(root: PathBuf) -> io::Result<App> {
        Ok(App {
            tree: Tree::new(root)?,
            editor: Editor::new(),
            focus: Focus::Tree,
            tree_visible: true,
            editor_visible: true,
            prompt: Prompt::None,
            status: None,
            tree_scroll: 0,
            should_quit: false,
            last_edit: None,
            last_tree_refresh: Instant::now(),
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
            (true, KeyCode::Char('e')) => {
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
        if self.editor.dirty && self.last_edit.is_some_and(|t| t.elapsed() >= IDLE_AUTOSAVE) {
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
        if self.last_tree_refresh.elapsed() >= TREE_REFRESH {
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
            KeyCode::Char('X') => self.confirm_delete(),
            KeyCode::Char('m') => self.start_move(),
            KeyCode::Char('r') => {
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
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (ctrl, key.code) {
            // Shift+Tab arrives as BackTab; treat it like Esc
            (false, KeyCode::Esc) | (_, KeyCode::BackTab) => self.focus = Focus::Tree,
            (true, KeyCode::Char('s')) => self.do_save(),
            (true, KeyCode::Char('f')) => self.prompt = Prompt::Search(String::new()),
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
            _ => {
                if self.editor.textarea.input(Input::from(key)) {
                    self.note_edit();
                }
            }
        }
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
        if key.modifiers.contains(KeyModifiers::CONTROL) {
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
                // Shift+X again = confirm the delete
                KeyCode::Char('X') => {
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

    /// Literal, case-sensitive, wraps around; starts one char after the cursor.
    fn search_next(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let DataCursor(crow, ccol) = self.editor.textarea.cursor();
        let lines: Vec<String> = self.editor.textarea.lines().to_vec();
        let n = lines.len();
        for i in 0..=n {
            let row = (crow + i) % n;
            let hay = &lines[row];
            let from_char = if i == 0 { ccol + 1 } else { 0 };
            let from_byte = hay
                .char_indices()
                .nth(from_char)
                .map(|(b, _)| b)
                .unwrap_or(hay.len());
            if let Some(b) = hay[from_byte..].find(query) {
                let cpos = hay[..from_byte + b].chars().count();
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
        let app = App::new(fixture("start")).unwrap();
        assert!(matches!(app.focus, Focus::Tree));
    }

    #[test]
    fn enter_opens_file_and_focuses_editor() {
        let mut app = App::new(fixture("open")).unwrap();
        app.handle_key(key(KeyCode::Enter)); // a.md selected first
        assert!(matches!(app.focus, Focus::Editor));
        assert_eq!(app.editor.textarea.lines(), ["hello", "world"]);
    }

    #[test]
    fn esc_returns_to_tree() {
        let mut app = App::new(fixture("esc")).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.focus, Focus::Tree));
    }

    #[test]
    fn typing_marks_dirty_and_switching_files_autosaves() {
        let root = fixture("autosave");
        let mut app = App::new(root.clone()).unwrap();
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
        let mut app = App::new(fixture("undo")).unwrap();
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
        let mut app = App::new(fixture("toggle")).unwrap();
        app.handle_key(ctrl('b'));
        assert!(!app.tree_visible);
        assert!(matches!(app.focus, Focus::Editor));
        app.handle_key(ctrl('b'));
        assert!(app.tree_visible);
    }

    #[test]
    fn ctrl_e_hides_editor_and_opening_a_file_reshows_it() {
        let mut app = App::new(fixture("epane")).unwrap();
        app.handle_key(ctrl('e'));
        assert!(!app.editor_visible);
        assert!(app.tree_visible);
        assert!(matches!(app.focus, Focus::Tree));
        app.handle_key(key(KeyCode::Enter)); // open a.md
        assert!(app.editor_visible);
        assert!(matches!(app.focus, Focus::Editor));
    }

    #[test]
    fn panes_can_never_both_be_hidden() {
        let mut app = App::new(fixture("panes")).unwrap();
        app.handle_key(ctrl('e')); // editor hidden
        app.handle_key(ctrl('b')); // hide tree -> editor must come back
        assert!(app.editor_visible);
        assert!(!app.tree_visible);
        app.handle_key(ctrl('e')); // hide editor -> tree must come back
        assert!(app.tree_visible);
        assert!(!app.editor_visible);
    }

    #[test]
    fn ctrl_q_saves_and_quits() {
        let root = fixture("quit");
        let mut app = App::new(root.clone()).unwrap();
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
        let mut app = App::new(root.clone()).unwrap();
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
        let mut app = App::new(fixture("search")).unwrap();
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
        let mut app = App::new(fixture("repeat")).unwrap();
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
    fn dash_reroots_tree_at_parent() {
        let root = fixture("ascend");
        let mut app = App::new(root.clone()).unwrap();
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
        let mut app = App::new(fixture("motion")).unwrap();
        app.handle_key(key(KeyCode::Enter)); // hello / world, cursor (0,0)
        app.handle_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        app.handle_key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
        assert_eq!(app.editor.textarea.lines()[0], "JKhello"); // typed, not moved
    }

    #[test]
    fn alt_jk_jumps_by_paragraph() {
        let root = fixture("para");
        fs::write(root.join("a.md"), "one\n\ntwo\n\nthree\n").unwrap();
        let mut app = App::new(root).unwrap();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT));
        let DataCursor(row, _) = app.editor.textarea.cursor();
        assert!(row >= 1); // moved past the blank line
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT));
        assert_eq!(app.editor.textarea.cursor(), (0, 0));
    }

    #[test]
    fn super_jk_jumps_to_line_end_and_start() {
        let mut app = App::new(fixture("linejump")).unwrap();
        app.handle_key(key(KeyCode::Enter)); // hello
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SUPER));
        assert_eq!(app.editor.textarea.cursor(), (0, 5)); // end of "hello"
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::SUPER));
        assert_eq!(app.editor.textarea.cursor(), (0, 0));
    }

    #[test]
    fn shift_tab_in_editor_returns_to_tree_without_typing() {
        let mut app = App::new(fixture("backtab")).unwrap();
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
        let mut app = App::new(root.clone()).unwrap();
        // docs/ sorts first (dirs before files), so it's already selected
        app.handle_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::SHIFT));
        assert_eq!(app.tree.root(), root.canonicalize().unwrap().join("docs"));
    }

    #[test]
    fn shift_x_confirm_no_by_default_keeps_file() {
        let root = fixture("del-no");
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT));
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
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT));
        app.handle_key(key(KeyCode::Char('j'))); // move highlight to Yes
        app.handle_key(key(KeyCode::Enter));
        assert!(!root.join("a.md").exists());
        assert!(!app.tree.rows().iter().any(|r| r.name == "a.md"));
    }

    #[test]
    fn shift_x_inside_popup_deletes_immediately() {
        let root = fixture("del-xx");
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT));
        app.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT));
        assert!(!root.join("a.md").exists());
    }

    #[test]
    fn esc_closes_delete_popup_without_deleting() {
        let root = fixture("del-esc");
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(root.join("a.md").exists());
    }

    #[test]
    fn deleting_the_open_file_clears_the_editor() {
        let root = fixture("del-open");
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(key(KeyCode::Enter)); // open a.md
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT));
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
        let mut app = App::new(root.clone()).unwrap();
        // docs/ sorts first, so it's selected
        app.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT));
        assert!(matches!(app.prompt, Prompt::None));
        assert!(app.status.is_some());
        assert!(root.join("docs").exists());
    }

    #[test]
    fn m_moves_file_into_chosen_directory() {
        let root = fixture("move");
        fs::create_dir_all(root.join("docs")).unwrap();
        let mut app = App::new(root.clone()).unwrap();
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
        let mut app = App::new(root).unwrap();
        app.handle_key(key(KeyCode::Char('m'))); // docs selected
        assert!(matches!(app.prompt, Prompt::None));
        assert!(app.status.is_some());
    }

    #[test]
    fn moving_the_open_file_keeps_editing_it_at_the_new_path() {
        let root = fixture("move-open");
        fs::create_dir_all(root.join("docs")).unwrap();
        let mut app = App::new(root.clone()).unwrap();
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
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(key(KeyCode::Char('j'))); // a.md
        app.handle_key(key(KeyCode::Char('m')));
        app.handle_key(key(KeyCode::Enter)); // first dest is root = current dir
        assert!(root.join("a.md").exists());
        assert!(app.status.is_some());
    }

    #[test]
    fn r_refreshes_tree_to_pick_up_external_files() {
        let root = fixture("refresh-key");
        let mut app = App::new(root.clone()).unwrap();
        fs::write(root.join("new.md"), "n\n").unwrap();
        assert!(!app.tree.rows().iter().any(|r| r.name == "new.md"));
        app.handle_key(key(KeyCode::Char('r')));
        assert!(app.tree.rows().iter().any(|r| r.name == "new.md"));
    }

    #[test]
    fn tick_auto_refreshes_tree_periodically() {
        let root = fixture("refresh-tick");
        let mut app = App::new(root.clone()).unwrap();
        fs::write(root.join("new.md"), "n\n").unwrap();
        app.last_tree_refresh = std::time::Instant::now() - std::time::Duration::from_secs(3);
        app.tick();
        assert!(app.tree.rows().iter().any(|r| r.name == "new.md"));
    }

    #[test]
    fn tick_idle_autosaves() {
        let root = fixture("tick");
        let mut app = App::new(root.clone()).unwrap();
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
}
