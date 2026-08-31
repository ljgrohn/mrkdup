use ratatui_textarea::{TextArea, WrapMode};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::fsutil;

pub enum SaveOutcome {
    Saved,
    Clean,
    Conflict,
    NoFile,
}

pub struct Editor {
    pub textarea: TextArea<'static>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    mtime: Option<SystemTime>,
}

fn make_textarea(lines: Vec<String>) -> TextArea<'static> {
    let mut ta = TextArea::from(lines);
    ta.set_wrap_mode(WrapMode::WordOrGlyph);
    ta
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            textarea: make_textarea(Vec::new()),
            path: None,
            dirty: false,
            mtime: None,
        }
    }

    pub fn open(&mut self, path: &Path) -> io::Result<()> {
        let text = fs::read_to_string(path)?;
        self.textarea = make_textarea(text.lines().map(String::from).collect());
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        self.mtime = disk_mtime(path);
        Ok(())
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn content(&self) -> String {
        let mut s = self.textarea.lines().join("\n");
        s.push('\n');
        s
    }

    pub fn save(&mut self, force: bool) -> io::Result<SaveOutcome> {
        let Some(path) = self.path.clone() else {
            return Ok(SaveOutcome::NoFile);
        };
        if !self.dirty {
            return Ok(SaveOutcome::Clean);
        }
        if !force && self.disk_changed(&path) {
            return Ok(SaveOutcome::Conflict);
        }
        fsutil::atomic_write(&path, self.content().as_bytes())?;
        self.mtime = disk_mtime(&path);
        self.dirty = false;
        Ok(SaveOutcome::Saved)
    }

    /// Clean buffer + changed disk => reload; returns true if reloaded.
    /// A dirty buffer is never reloaded (save() reports the conflict).
    pub fn check_external(&mut self) -> io::Result<bool> {
        let Some(path) = self.path.clone() else {
            return Ok(false);
        };
        if !self.disk_changed(&path) || self.dirty {
            return Ok(false);
        }
        self.open(&path)?;
        Ok(true)
    }

    fn disk_changed(&self, path: &Path) -> bool {
        match (self.mtime, disk_mtime(path)) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        }
    }
}

fn disk_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpfile(tag: &str, content: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("mrkdup-ed-{tag}.md"));
        fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn open_loads_lines() {
        let p = tmpfile("open", "# a\n\nb\n");
        let mut ed = Editor::new();
        ed.open(&p).unwrap();
        assert_eq!(ed.textarea.lines(), ["# a", "", "b"]);
        assert!(!ed.dirty);
    }

    #[test]
    fn save_when_clean_is_noop() {
        let p = tmpfile("clean", "x\n");
        let mut ed = Editor::new();
        ed.open(&p).unwrap();
        assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Clean));
    }

    #[test]
    fn save_writes_with_trailing_newline() {
        let p = tmpfile("save", "x\n");
        let mut ed = Editor::new();
        ed.open(&p).unwrap();
        ed.textarea.insert_str("hi ");
        ed.mark_dirty();
        assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Saved));
        assert!(!ed.dirty);
        assert_eq!(fs::read_to_string(&p).unwrap(), "hi x\n");
    }

    #[test]
    fn save_with_no_file_is_nofile() {
        let mut ed = Editor::new();
        assert!(matches!(ed.save(false).unwrap(), SaveOutcome::NoFile));
    }

    #[test]
    fn external_change_while_dirty_blocks_save_then_force_wins() {
        let p = tmpfile("conflict", "x\n");
        let mut ed = Editor::new();
        ed.open(&p).unwrap();
        ed.textarea.insert_str("mine ");
        ed.mark_dirty();
        // simulate an external writer (bump mtime meaningfully)
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&p, "theirs\n").unwrap();
        assert!(matches!(ed.save(false).unwrap(), SaveOutcome::Conflict));
        assert_eq!(fs::read_to_string(&p).unwrap(), "theirs\n"); // untouched
        assert!(matches!(ed.save(true).unwrap(), SaveOutcome::Saved));
        assert_eq!(fs::read_to_string(&p).unwrap(), "mine x\n");
    }

    #[test]
    fn external_change_while_clean_reloads() {
        let p = tmpfile("reload", "x\n");
        let mut ed = Editor::new();
        ed.open(&p).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&p, "fresh\n").unwrap();
        assert!(ed.check_external().unwrap());
        assert_eq!(ed.textarea.lines(), ["fresh"]);
    }

    #[test]
    fn external_change_while_dirty_does_not_reload() {
        let p = tmpfile("noreload", "x\n");
        let mut ed = Editor::new();
        ed.open(&p).unwrap();
        ed.textarea.insert_str("mine");
        ed.mark_dirty();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&p, "theirs\n").unwrap();
        assert!(!ed.check_external().unwrap());
        assert_eq!(ed.textarea.lines()[0], "minex");
    }
}
