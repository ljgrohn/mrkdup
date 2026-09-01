//! Caches the wrap layout and syntax-highlight spans for the editor pane
//! across frames. `render_editor` used to redo both on every single
//! frame — a cursor blink, a tick, a tree-pane repaint — even though the
//! document text hadn't changed. This cache recomputes only when
//! something that could actually change the result has changed: an
//! edit, a file open, or a pane resize; every other frame reuses the
//! stored result.
//!
//! Owned by `Editor`, one cache per open document. Whole-file recompute
//! only (no incremental re-highlight from the edited line) — see the D1
//! task brief for why: simple first, incremental only if this stayed
//! tiny.

use crate::highlight::{self, FileKind, SpanTok};
use crate::wrap::{self, VisualRow};

pub struct LayoutCache {
    width: usize,
    file_kind: Option<FileKind>,
    stale: bool,
    rows: Vec<VisualRow>,
    spans: Vec<Vec<SpanTok>>,
    #[cfg(test)]
    pub recomputes: usize,
}

impl LayoutCache {
    pub fn new() -> Self {
        LayoutCache {
            width: 0,
            file_kind: None,
            stale: true,
            rows: Vec::new(),
            spans: Vec::new(),
            #[cfg(test)]
            recomputes: 0,
        }
    }

    /// Force the next `ensure` call to recompute, regardless of width or
    /// file kind. Called on edit and on file open/reload.
    pub fn invalidate(&mut self) {
        self.stale = true;
    }

    /// Recompute wrap rows and highlight spans for `lines` if the cache
    /// is stale, the pane width changed, or the file kind changed;
    /// otherwise reuse the stored result.
    pub fn ensure(
        &mut self,
        lines: &[String],
        width: usize,
        file_kind: FileKind,
    ) -> (&[VisualRow], &[Vec<SpanTok>]) {
        if self.stale || self.width != width || self.file_kind != Some(file_kind) {
            self.rows = wrap::layout(lines, width);
            self.spans = highlight::highlight(lines, file_kind);
            self.width = width;
            self.file_kind = Some(file_kind);
            self.stale = false;
            #[cfg(test)]
            {
                self.recomputes += 1;
            }
        }
        (&self.rows, &self.spans)
    }
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn recomputes_on_first_call_only() {
        let mut cache = LayoutCache::new();
        let ls = lines(&["hello world"]);
        cache.ensure(&ls, 20, FileKind::Markdown);
        assert_eq!(cache.recomputes, 1);
        cache.ensure(&ls, 20, FileKind::Markdown);
        cache.ensure(&ls, 20, FileKind::Markdown);
        assert_eq!(cache.recomputes, 1, "unchanged inputs should reuse cache");
    }

    #[test]
    fn width_change_forces_recompute() {
        let mut cache = LayoutCache::new();
        let ls = lines(&["hello world"]);
        cache.ensure(&ls, 20, FileKind::Markdown);
        cache.ensure(&ls, 10, FileKind::Markdown);
        assert_eq!(cache.recomputes, 2);
    }

    #[test]
    fn file_kind_change_forces_recompute() {
        let mut cache = LayoutCache::new();
        let ls = lines(&["<p>hi</p>"]);
        cache.ensure(&ls, 20, FileKind::Markdown);
        cache.ensure(&ls, 20, FileKind::Html);
        assert_eq!(cache.recomputes, 2);
    }

    #[test]
    fn invalidate_forces_recompute() {
        let mut cache = LayoutCache::new();
        let ls = lines(&["hello world"]);
        cache.ensure(&ls, 20, FileKind::Markdown);
        cache.invalidate();
        cache.ensure(&ls, 20, FileKind::Markdown);
        assert_eq!(cache.recomputes, 2);
    }
}
