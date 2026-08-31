//! Pure live-syntax tokenizer for markdown and HTML. Produces styled
//! char-range spans per logical line; all characters stay visible (marks
//! are dimmed, never hidden), so layout is untouched.

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    /// dimmed syntax characters: **, *, backticks, [, ], #, >, ---
    Mark,
    Heading(u8),
    Bold,
    Italic,
    CodeInline,
    CodeBlock,
    CheckboxOpen,
    DoneText,
    Quote,
    LinkText,
    LinkUrl,
    Bullet,
    HtmlTag,
    HtmlAttr,
    HtmlString,
    HtmlComment,
    FmKey,
}

/// `[start, end)` in CHAR indices of the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanTok {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

fn tok(start: usize, end: usize, kind: Kind) -> SpanTok {
    SpanTok { start, end, kind }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Markdown,
    Html,
}

pub fn file_kind(path: Option<&std::path::Path>) -> FileKind {
    match path.and_then(|p| p.extension()).and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm") => {
            FileKind::Html
        }
        _ => FileKind::Markdown,
    }
}

pub fn style(kind: Kind) -> Style {
    match kind {
        Kind::Text => Style::default(),
        Kind::Mark | Kind::LinkUrl | Kind::HtmlComment => {
            Style::default().add_modifier(Modifier::DIM)
        }
        Kind::Heading(1) => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        Kind::Heading(2) => Style::default().fg(Color::Cyan),
        Kind::Heading(_) => Style::default().fg(Color::Blue),
        Kind::Bold => Style::default().add_modifier(Modifier::BOLD),
        Kind::Italic => Style::default().add_modifier(Modifier::ITALIC),
        Kind::CodeInline | Kind::CodeBlock | Kind::HtmlString => Style::default().fg(Color::Green),
        Kind::CheckboxOpen => Style::default().fg(Color::Magenta),
        Kind::DoneText => Style::default().add_modifier(Modifier::DIM),
        Kind::Quote => Style::default().fg(Color::Yellow),
        Kind::LinkText => Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED),
        Kind::Bullet => Style::default().fg(Color::Cyan),
        Kind::HtmlTag => Style::default().fg(Color::Magenta),
        Kind::HtmlAttr | Kind::FmKey => Style::default().fg(Color::Cyan),
    }
}

/// Highlight a whole document (state — fences, frontmatter, comments —
/// must run from the top). One `Vec<SpanTok>` per logical line; spans
/// are sorted, non-overlapping, and cover the whole line.
pub fn highlight(lines: &[String], kind: FileKind) -> Vec<Vec<SpanTok>> {
    let mut state = State::default();
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| highlight_line(line, i, &mut state, kind))
        .collect()
}

#[derive(Default)]
struct State {
    in_fence: bool,
    in_frontmatter: bool,
    in_comment: bool,
}

fn highlight_line(line: &str, idx: usize, state: &mut State, kind: FileKind) -> Vec<SpanTok> {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    if n == 0 {
        return vec![];
    }
    if kind == FileKind::Html {
        return html_line(&chars, state);
    }

    // frontmatter: opened by --- on the very first line only
    if idx == 0 && line.trim() == "---" && !state.in_frontmatter {
        state.in_frontmatter = true;
        return vec![tok(0, n, Kind::Mark)];
    }
    if state.in_frontmatter {
        if line.trim() == "---" {
            state.in_frontmatter = false;
            return vec![tok(0, n, Kind::Mark)];
        }
        if let Some(colon) = chars.iter().position(|&c| c == ':') {
            let mut spans = vec![tok(0, colon + 1, Kind::FmKey)];
            if colon + 1 < n {
                spans.push(tok(colon + 1, n, Kind::Text));
            }
            return spans;
        }
        return vec![tok(0, n, Kind::Text)];
    }

    // fenced code blocks
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        state.in_fence = !state.in_fence;
        return vec![tok(0, n, Kind::Mark)];
    }
    if state.in_fence {
        return vec![tok(0, n, Kind::CodeBlock)];
    }

    // headings
    let hashes = chars.iter().take_while(|&&c| c == '#').count();
    if (1..=6).contains(&hashes) && chars.get(hashes) == Some(&' ') {
        let mut spans = vec![tok(0, hashes + 1, Kind::Mark)];
        spans.push(tok(hashes + 1, n, Kind::Heading(hashes as u8)));
        return spans;
    }

    // horizontal rule (---, ***, ___, 3+ chars, nothing else)
    let t = line.trim();
    if t.len() >= 3
        && (t.chars().all(|c| c == '-')
            || t.chars().all(|c| c == '*')
            || t.chars().all(|c| c == '_'))
    {
        return vec![tok(0, n, Kind::Mark)];
    }

    let indent = chars.iter().take_while(|&&c| c == ' ').count();

    // blockquote
    if chars.get(indent) == Some(&'>') {
        let mut spans = vec![tok(0, indent + 1, Kind::Mark)];
        if indent + 1 < n {
            spans.push(tok(indent + 1, n, Kind::Quote));
        }
        return spans;
    }

    // checkboxes: "- [ ] " / "- [x] "
    let rest: String = chars[indent..].iter().collect();
    if rest.starts_with("- [ ] ") || rest.starts_with("- [x] ") || rest.starts_with("- [X] ") {
        let done = !rest.starts_with("- [ ] ");
        let box_end = indent + 6; // "- [x] "
        let mut spans = vec![
            tok(indent, indent + 2, Kind::Bullet),
            tok(indent + 2, box_end, Kind::CheckboxOpen),
        ];
        if indent > 0 {
            spans.insert(0, tok(0, indent, Kind::Text));
        }
        if box_end < n {
            if done {
                spans.push(tok(box_end, n, Kind::DoneText));
            } else {
                spans.extend(inline(&chars, box_end, n));
            }
        }
        return spans;
    }

    // plain bullets and ordered lists
    let bullet_len = if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        Some(2)
    } else {
        let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && rest[digits..].starts_with(". ") {
            Some(digits + 2)
        } else {
            None
        }
    };
    if let Some(blen) = bullet_len {
        let mut spans = Vec::new();
        if indent > 0 {
            spans.push(tok(0, indent, Kind::Text));
        }
        spans.push(tok(indent, indent + blen, Kind::Bullet));
        spans.extend(inline(&chars, indent + blen, n));
        return spans;
    }

    inline(&chars, 0, n)
}

/// Inline markdown within `chars[from..to]`: `code`, **bold**, *italic*,
/// _italic_, [text](url), and inline <html>.
fn inline(chars: &[char], from: usize, to: usize) -> Vec<SpanTok> {
    let mut spans = Vec::new();
    let mut text_start = from;
    let mut i = from;
    let flush = |spans: &mut Vec<SpanTok>, text_start: usize, upto: usize| {
        if upto > text_start {
            spans.push(tok(text_start, upto, Kind::Text));
        }
    };
    while i < to {
        let c = chars[i];
        // `code`
        if c == '`' {
            if let Some(close) = find(chars, i + 1, to, '`') {
                flush(&mut spans, text_start, i);
                spans.push(tok(i, close + 1, Kind::CodeInline));
                i = close + 1;
                text_start = i;
                continue;
            }
        }
        // **bold**
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(close) = find_pair(chars, i + 2, to, '*') {
                flush(&mut spans, text_start, i);
                spans.push(tok(i, i + 2, Kind::Mark));
                spans.push(tok(i + 2, close, Kind::Bold));
                spans.push(tok(close, close + 2, Kind::Mark));
                i = close + 2;
                text_start = i;
                continue;
            }
        }
        // *italic* / _italic_
        if (c == '*' || c == '_') && i + 1 < to && chars[i + 1] != ' ' && chars[i + 1] != c {
            if let Some(close) = find(chars, i + 1, to, c) {
                if close > i + 1 && chars[close - 1] != ' ' {
                    flush(&mut spans, text_start, i);
                    spans.push(tok(i, i + 1, Kind::Mark));
                    spans.push(tok(i + 1, close, Kind::Italic));
                    spans.push(tok(close, close + 1, Kind::Mark));
                    i = close + 1;
                    text_start = i;
                    continue;
                }
            }
        }
        // [text](url)
        if c == '[' {
            if let Some(rb) = find(chars, i + 1, to, ']') {
                if chars.get(rb + 1) == Some(&'(') {
                    if let Some(rp) = find(chars, rb + 2, to, ')') {
                        flush(&mut spans, text_start, i);
                        spans.push(tok(i, i + 1, Kind::Mark));
                        spans.push(tok(i + 1, rb, Kind::LinkText));
                        spans.push(tok(rb, rb + 2, Kind::Mark));
                        spans.push(tok(rb + 2, rp, Kind::LinkUrl));
                        spans.push(tok(rp, rp + 1, Kind::Mark));
                        i = rp + 1;
                        text_start = i;
                        continue;
                    }
                }
            }
        }
        // inline <tag ...>
        if c == '<' {
            if let Some(gt) = find(chars, i + 1, to, '>') {
                if gt > i + 1 {
                    flush(&mut spans, text_start, i);
                    spans.extend(tag_spans(chars, i, gt + 1));
                    i = gt + 1;
                    text_start = i;
                    continue;
                }
            }
        }
        i += 1;
    }
    flush(&mut spans, text_start, to);
    spans
}

fn find(chars: &[char], from: usize, to: usize, needle: char) -> Option<usize> {
    (from..to).find(|&j| chars[j] == needle)
}

/// Position of the next `mark`+`mark` pair at or after `from`.
fn find_pair(chars: &[char], from: usize, to: usize, mark: char) -> Option<usize> {
    let mut j = from;
    while j + 1 < to {
        if chars[j] == mark && chars[j + 1] == mark {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Tokens for one `<...>` tag: brackets and `/` dim, tag name magenta,
/// attribute names cyan, quoted values green.
fn tag_spans(chars: &[char], start: usize, end: usize) -> Vec<SpanTok> {
    let mut spans = Vec::new();
    let mut i = start;
    // leading <, </
    let mut j = i + 1;
    if chars.get(j) == Some(&'/') {
        j += 1;
    }
    spans.push(tok(i, j, Kind::Mark));
    i = j;
    // tag name
    while j < end && (chars[j].is_ascii_alphanumeric() || chars[j] == '-' || chars[j] == '!') {
        j += 1;
    }
    if j > i {
        spans.push(tok(i, j, Kind::HtmlTag));
    }
    i = j;
    // attributes and strings
    while i < end {
        let c = chars[i];
        if c == '"' || c == '\'' {
            let close = find(chars, i + 1, end, c).unwrap_or(end - 1);
            spans.push(tok(i, (close + 1).min(end), Kind::HtmlString));
            i = (close + 1).min(end);
        } else if c.is_ascii_alphanumeric() {
            let mut k = i;
            while k < end && (chars[k].is_ascii_alphanumeric() || chars[k] == '-') {
                k += 1;
            }
            spans.push(tok(i, k, Kind::HtmlAttr));
            i = k;
        } else {
            spans.push(tok(i, i + 1, Kind::Mark));
            i += 1;
        }
    }
    spans
}

/// One line of an .html file. Comments carry across lines.
fn html_line(chars: &[char], state: &mut State) -> Vec<SpanTok> {
    let n = chars.len();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < n {
        if state.in_comment {
            // look for -->
            let mut close = None;
            for j in i..n.saturating_sub(2) {
                if chars[j] == '-' && chars[j + 1] == '-' && chars[j + 2] == '>' {
                    close = Some(j + 3);
                    break;
                }
            }
            match close {
                Some(c) => {
                    spans.push(tok(i, c, Kind::HtmlComment));
                    state.in_comment = false;
                    i = c;
                }
                None => {
                    spans.push(tok(i, n, Kind::HtmlComment));
                    return spans;
                }
            }
            continue;
        }
        if chars[i] == '<' {
            // comment open?
            let is_comment = chars.get(i + 1) == Some(&'!')
                && chars.get(i + 2) == Some(&'-')
                && chars.get(i + 3) == Some(&'-');
            if is_comment {
                state.in_comment = true;
                continue; // loop re-enters the in_comment branch at i
            }
            if let Some(gt) = find(chars, i + 1, n, '>') {
                spans.extend(tag_spans(chars, i, gt + 1));
                i = gt + 1;
                continue;
            }
        }
        // plain text run until the next '<'
        let next = find(chars, i, n, '<').unwrap_or(n);
        let next = next.max(i + 1);
        spans.push(tok(i, next, Kind::Text));
        i = next;
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hl(lines: &[&str]) -> Vec<Vec<SpanTok>> {
        let v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        highlight(&v, FileKind::Markdown)
    }

    fn kinds(spans: &[SpanTok]) -> Vec<Kind> {
        spans.iter().map(|s| s.kind).collect()
    }

    /// spans must be sorted, non-overlapping, and cover [0, len)
    fn assert_covers(spans: &[SpanTok], len: usize) {
        if len == 0 {
            assert!(spans.is_empty());
            return;
        }
        assert_eq!(spans.first().unwrap().start, 0);
        assert_eq!(spans.last().unwrap().end, len);
        for w in spans.windows(2) {
            assert_eq!(w[0].end, w[1].start, "gap/overlap in {spans:?}");
        }
    }

    #[test]
    fn heading_levels() {
        let s = hl(&["# Title", "### Sub"]);
        assert_eq!(kinds(&s[0]), vec![Kind::Mark, Kind::Heading(1)]);
        assert_eq!(kinds(&s[1]), vec![Kind::Mark, Kind::Heading(3)]);
        assert_covers(&s[0], 7);
    }

    #[test]
    fn not_a_heading_without_space() {
        let s = hl(&["#tag"]);
        assert_eq!(kinds(&s[0]), vec![Kind::Text]);
    }

    #[test]
    fn bold_italic_code_inline() {
        let s = hl(&["a **b** *c* `d` e"]);
        let k = kinds(&s[0]);
        assert!(k.contains(&Kind::Bold));
        assert!(k.contains(&Kind::Italic));
        assert!(k.contains(&Kind::CodeInline));
        assert_covers(&s[0], 17);
    }

    #[test]
    fn unclosed_marks_stay_text() {
        let s = hl(&["a **b and `c"]);
        assert_eq!(kinds(&s[0]), vec![Kind::Text]);
    }

    #[test]
    fn checkboxes_and_done_dimming() {
        let s = hl(&["- [ ] todo", "- [x] done *never styled*"]);
        assert_eq!(
            kinds(&s[0]),
            vec![Kind::Bullet, Kind::CheckboxOpen, Kind::Text]
        );
        assert_eq!(
            kinds(&s[1]),
            vec![Kind::Bullet, Kind::CheckboxOpen, Kind::DoneText]
        );
        assert_covers(&s[1], 25);
    }

    #[test]
    fn indented_checkbox_keeps_indent_as_text() {
        let s = hl(&["  - [ ] sub"]);
        assert_eq!(
            kinds(&s[0]),
            vec![Kind::Text, Kind::Bullet, Kind::CheckboxOpen, Kind::Text]
        );
    }

    #[test]
    fn quote_and_hr() {
        let s = hl(&["> wise words", "---"]);
        assert_eq!(kinds(&s[0]), vec![Kind::Mark, Kind::Quote]);
        assert_eq!(kinds(&s[1]), vec![Kind::Mark]); // hr, not frontmatter (has line above? no — idx 0!)
    }

    #[test]
    fn dashes_on_first_line_open_frontmatter() {
        let s = hl(&["---", "day: 1", "---", "# After"]);
        assert_eq!(kinds(&s[0]), vec![Kind::Mark]);
        assert_eq!(kinds(&s[1]), vec![Kind::FmKey, Kind::Text]);
        assert_eq!(kinds(&s[2]), vec![Kind::Mark]);
        assert_eq!(kinds(&s[3]), vec![Kind::Mark, Kind::Heading(1)]);
    }

    #[test]
    fn fenced_code_block() {
        let s = hl(&["```rust", "let x = 1;", "```", "after"]);
        assert_eq!(kinds(&s[0]), vec![Kind::Mark]);
        assert_eq!(kinds(&s[1]), vec![Kind::CodeBlock]);
        assert_eq!(kinds(&s[2]), vec![Kind::Mark]);
        assert_eq!(kinds(&s[3]), vec![Kind::Text]);
    }

    #[test]
    fn no_markdown_styling_inside_fence() {
        let s = hl(&["```", "# not a heading", "```"]);
        assert_eq!(kinds(&s[1]), vec![Kind::CodeBlock]);
    }

    #[test]
    fn links() {
        let s = hl(&["see [docs](http://x) ok"]);
        let k = kinds(&s[0]);
        assert!(k.contains(&Kind::LinkText));
        assert!(k.contains(&Kind::LinkUrl));
        assert_covers(&s[0], "see [docs](http://x) ok".chars().count());
    }

    #[test]
    fn bullets_and_ordered_lists() {
        let s = hl(&["- item", "12. item"]);
        assert_eq!(s[0][0].kind, Kind::Bullet);
        assert_eq!(s[1][0].kind, Kind::Bullet);
        assert_eq!(s[1][0].end, 4); // "12. "
    }

    #[test]
    fn inline_html_in_markdown() {
        let s = hl(&["text <b class=\"x\"> more"]);
        let k = kinds(&s[0]);
        assert!(k.contains(&Kind::HtmlTag));
        assert!(k.contains(&Kind::HtmlAttr));
        assert!(k.contains(&Kind::HtmlString));
        assert_covers(&s[0], 23);
    }

    #[test]
    fn html_file_mode_with_multiline_comment() {
        let v: Vec<String> = [
            "<div id=\"a\">hi</div>",
            "<!-- note",
            "still -->",
            "<p>x</p>",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let s = highlight(&v, FileKind::Html);
        assert!(kinds(&s[0]).contains(&Kind::HtmlTag));
        assert!(kinds(&s[0]).contains(&Kind::Text)); // "hi"
        assert_eq!(kinds(&s[1]), vec![Kind::HtmlComment]);
        assert!(kinds(&s[2]).contains(&Kind::HtmlComment));
        assert!(kinds(&s[3]).contains(&Kind::HtmlTag));
    }

    #[test]
    fn every_line_fully_covered_on_a_gnarly_doc() {
        let doc = [
            "---",
            "title: x",
            "---",
            "# H",
            "",
            "a **b** `c` [d](e) <i>f</i>",
            "> q",
            "- [x] done",
            "```",
            "code",
            "```",
        ];
        let s = hl(&doc);
        for (i, line) in doc.iter().enumerate() {
            assert_covers(&s[i], line.chars().count());
        }
    }

    #[test]
    fn file_kind_by_extension() {
        use std::path::Path;
        assert_eq!(file_kind(Some(Path::new("/a/b.HTML"))), FileKind::Html);
        assert_eq!(file_kind(Some(Path::new("/a/b.md"))), FileKind::Markdown);
        assert_eq!(file_kind(None), FileKind::Markdown);
    }

    #[test]
    fn unicode_char_indices() {
        // spans are char-indexed: emoji/wide chars must not break coverage
        let s = hl(&["héllo **wörld** 你好"]);
        assert_covers(&s[0], "héllo **wörld** 你好".chars().count());
    }
}
