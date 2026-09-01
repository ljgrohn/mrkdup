use ratatui_textarea::{CursorMove, DataCursor, Input, TextArea};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::fsutil;
use crate::layout_cache::LayoutCache;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Newline {
    Lf,
    CrLf,
}

pub enum SaveOutcome {
    Saved,
    Clean,
    Conflict,
    NoFile,
}

pub struct Editor {
    textarea: TextArea<'static>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    mtime: Option<SystemTime>,
    newline: Newline,
    layout_cache: LayoutCache,
}

// rendering (wrap, styling, cursor, search highlight) lives in render.rs;
// the textarea here is purely the editing engine
fn make_textarea(lines: Vec<String>) -> TextArea<'static> {
    TextArea::from(lines)
}

/// Detect the newline style from raw file bytes.
/// Scans for the first line terminator (\n):
/// - If preceded by \r, return CRLF
/// - Otherwise return LF
/// - If no line terminator found (empty file), default to LF
fn detect_newline(bytes: &[u8]) -> Newline {
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' {
            if i > 0 && bytes[i - 1] == b'\r' {
                return Newline::CrLf;
            } else {
                return Newline::Lf;
            }
        }
    }
    // No line terminator found, default to LF.
    Newline::Lf
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            textarea: make_textarea(Vec::new()),
            path: None,
            dirty: false,
            mtime: None,
            newline: Newline::Lf,
            layout_cache: LayoutCache::new(),
        }
    }

    pub fn open(&mut self, path: &Path) -> io::Result<()> {
        let bytes = fs::read(path)?;
        let newline = detect_newline(&bytes);
        let text =
            String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        // Nothing above mutates `self` — a failed read or a failed UTF-8
        // validation leaves the currently-open document untouched. Once
        // we're past validation the rest can't fail, so assign here.
        self.newline = newline;
        self.textarea = make_textarea(text.lines().map(String::from).collect());
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        self.mtime = disk_mtime(path);
        self.layout_cache.invalidate();
        Ok(())
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.layout_cache.invalidate();
    }

    /// The buffer's lines, one `String` per line (no line terminators).
    pub fn lines(&self) -> &[String] {
        self.textarea.lines()
    }

    /// The document lines and the mutable wrap/highlight cache, borrowed
    /// together from one `&mut self` call so `render.rs` can hold both
    /// at once without fighting the borrow checker over separate
    /// accessor calls.
    pub fn render_parts(&mut self) -> (&[String], &mut LayoutCache) {
        (self.textarea.lines(), &mut self.layout_cache)
    }

    /// How many times the wrap/highlight cache has actually recomputed.
    /// Test-only: lets render tests assert that painting twice without
    /// an edit doesn't redo the work.
    #[cfg(test)]
    pub fn layout_recomputes(&self) -> usize {
        self.layout_cache.recomputes
    }

    /// Cursor position as (row, col), both 0-based.
    pub fn cursor(&self) -> (usize, usize) {
        let DataCursor(row, col) = self.textarea.cursor();
        (row, col)
    }

    /// The line the cursor is currently on, if any.
    pub fn current_line(&self) -> Option<&str> {
        let (row, _) = self.cursor();
        self.textarea.lines().get(row).map(String::as_str)
    }

    /// Move the cursor to an exact (row, col) position. `CursorMove::Jump`
    /// only takes `u16` coordinates, so this is the one place that guards
    /// against a position beyond `u16::MAX`; returns `false` (leaving the
    /// cursor untouched) when either coordinate doesn't fit.
    pub fn set_cursor(&mut self, row: usize, col: usize) -> bool {
        if row > u16::MAX as usize || col > u16::MAX as usize {
            return false;
        }
        self.textarea
            .move_cursor(CursorMove::Jump(row as u16, col as u16));
        true
    }

    /// Move the cursor with any other `CursorMove` variant (word/paragraph/
    /// line motions, etc). Positions beyond `u16::MAX` are handled by
    /// `set_cursor`, not this method.
    pub fn move_cursor(&mut self, m: CursorMove) {
        self.textarea.move_cursor(m);
    }

    pub fn insert_str<S: AsRef<str>>(&mut self, s: S) -> bool {
        self.textarea.insert_str(s)
    }

    pub fn undo(&mut self) -> bool {
        self.textarea.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.textarea.redo()
    }

    pub fn input(&mut self, input: Input) -> bool {
        self.textarea.input(input)
    }

    pub fn cancel_selection(&mut self) {
        self.textarea.cancel_selection();
    }

    pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        self.textarea.selection_range()
    }

    /// Delete the character before the cursor. Used by the "--0" checkbox
    /// expansion to remove the two dashes it was triggered on.
    pub fn delete_char(&mut self) -> bool {
        self.textarea.delete_char()
    }

    /// Delete from the cursor to the end of the current line. Used by
    /// checkbox toggling to rewrite a line in place.
    pub fn delete_line_by_end(&mut self) -> bool {
        self.textarea.delete_line_by_end()
    }

    fn content(&self) -> String {
        let newline_str = match self.newline {
            Newline::Lf => "\n",
            Newline::CrLf => "\r\n",
        };
        let mut s = self.textarea.lines().join(newline_str);
        s.push_str(newline_str);
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
mod tests;
