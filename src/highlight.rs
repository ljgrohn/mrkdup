//! Pure live-syntax tokenizer for markdown, HTML, and code (Rust here;
//! JavaScript/TypeScript, CSS, and SQL in `code.rs`). Produces styled
//! char-range spans per logical line; all characters stay visible (marks
//! are dimmed, never hidden), so layout is untouched.

mod code;

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
    // code (Rust files and ```rust fences)
    Keyword,
    /// a capitalised or primitive type name, or a `'lifetime`
    TypeName,
    /// string, raw string, byte string, or char literal
    Str,
    Comment,
    Number,
    /// a `name!` macro invocation or a `#[attribute]`
    Macro,
    /// a function name: after `fn`, or any plain identifier called with `(`
    Function,
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
    Rust,
    JavaScript,
    Css,
    Sql,
}

pub fn file_kind(path: Option<&std::path::Path>) -> FileKind {
    let ext = path
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("html" | "htm") => FileKind::Html,
        Some(e) => fence_kind(e).unwrap_or(FileKind::Markdown),
        None => FileKind::Markdown,
    }
}

/// The code kind a fence tag (or file extension) names, if any.
fn fence_kind(lang: &str) -> Option<FileKind> {
    Some(match lang {
        "rust" | "rs" => FileKind::Rust,
        "js" | "javascript" | "mjs" | "cjs" | "jsx" | "ts" | "typescript" | "tsx" => {
            FileKind::JavaScript
        }
        "css" | "scss" => FileKind::Css,
        "sql" => FileKind::Sql,
        _ => return None,
    })
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
    /// the open fence names a code language: highlight its body as that
    fence_code: Option<FileKind>,
    in_frontmatter: bool,
    in_comment: bool,
    rust: RustState,
    code: code::CodeState,
}

/// Rust constructs that carry across lines.
#[derive(Default)]
struct RustState {
    /// nesting depth inside `/* */` (Rust block comments nest)
    block_depth: usize,
    /// inside a `"..."` that hasn't closed yet
    in_string: bool,
    /// inside a raw string, with this many `#`s after its closing quote
    raw_hashes: Option<usize>,
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
    if let Some(spans) = code_line(&chars, state, kind, Kind::Text) {
        return spans;
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
        let lang = trimmed.trim_start_matches(['`', '~']).trim();
        state.fence_code = if state.in_fence {
            fence_kind(&lang.to_ascii_lowercase())
        } else {
            None
        };
        state.rust = RustState::default();
        state.code = code::CodeState::default();
        return vec![tok(0, n, Kind::Mark)];
    }
    if state.in_fence {
        if let Some(kind) = state.fence_code {
            if let Some(spans) = code_line(&chars, state, kind, Kind::CodeBlock) {
                return spans;
            }
        }
        return vec![tok(0, n, Kind::CodeBlock)];
    }

    // headings
    let hashes = chars.iter().take_while(|&&c| c == '#').count();
    if (1..=6).contains(&hashes) && chars.get(hashes) == Some(&' ') {
        let mut spans = vec![tok(0, hashes + 1, Kind::Mark)];
        spans.extend(inline_based(
            &chars,
            hashes + 1,
            n,
            Kind::Heading(hashes as u8),
        ));
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
            spans.extend(inline_based(&chars, indent + 1, n, Kind::Quote));
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
    inline_based(chars, from, to, Kind::Text)
}

/// Like `inline`, but plain (unmarked) text runs get `base` instead of
/// `Kind::Text` — lets a heading/quote keep its color under inline marks
/// without stacking kinds on one span.
fn inline_based(chars: &[char], from: usize, to: usize, base: Kind) -> Vec<SpanTok> {
    let mut spans = Vec::new();
    let mut text_start = from;
    let mut i = from;
    let flush = |spans: &mut Vec<SpanTok>, text_start: usize, upto: usize| {
        if upto > text_start {
            spans.push(tok(text_start, upto, base));
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

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while",
];

const RUST_PRIMITIVES: &[&str] = &[
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
    "i128", "isize", "f32", "f64",
];

/// One line of a code language, or `None` when `kind` isn't one.
fn code_line(
    chars: &[char],
    state: &mut State,
    kind: FileKind,
    base: Kind,
) -> Option<Vec<SpanTok>> {
    Some(match kind {
        FileKind::Rust => rust_line(chars, &mut state.rust, base),
        FileKind::JavaScript => code::generic_line(chars, &mut state.code, base, &code::JS),
        FileKind::Sql => code::generic_line(chars, &mut state.code, base, &code::SQL),
        FileKind::Css => code::css_line(chars, &mut state.code, base),
        FileKind::Markdown | FileKind::Html => return None,
    })
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Append `[start, end)` as `kind`, merging into the previous span when
/// it's the same kind (keeps the output compact and easy to assert on).
fn push_span(spans: &mut Vec<SpanTok>, start: usize, end: usize, kind: Kind) {
    if end <= start {
        return;
    }
    match spans.last_mut() {
        Some(last) if last.kind == kind && last.end == start => last.end = end,
        _ => spans.push(tok(start, end, kind)),
    }
}

/// Scan a `"` string body from `from` (just after the opening quote):
/// the index just past the closing quote and whether it closed on this
/// line. Backslash escapes skip the next char.
fn scan_string(chars: &[char], from: usize) -> (usize, bool) {
    let n = chars.len();
    let mut j = from;
    while j < n {
        match chars[j] {
            '\\' => j += 2,
            '"' => return (j + 1, true),
            _ => j += 1,
        }
    }
    (n, false)
}

/// Scan a raw string body from `from`: the index just past `"` followed
/// by `hashes` `#`s, or `None` if it doesn't close on this line.
fn scan_raw(chars: &[char], from: usize, hashes: usize) -> Option<usize> {
    let n = chars.len();
    (from..n).find_map(|j| {
        let closes = chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#'));
        closes.then_some(j + 1 + hashes)
    })
}

/// Continue a block comment from `from` (which may be just after a `/*`
/// or the start of a line): the index where it ends, tracking nesting
/// in `depth`. `depth` is 0 afterwards iff it closed on this line.
fn scan_block_comment(chars: &[char], from: usize, depth: &mut usize) -> usize {
    let n = chars.len();
    let mut j = from;
    while j < n && *depth > 0 {
        if chars[j] == '/' && chars.get(j + 1) == Some(&'*') {
            *depth += 1;
            j += 2;
        } else if chars[j] == '*' && chars.get(j + 1) == Some(&'/') {
            *depth -= 1;
            j += 2;
        } else {
            j += 1;
        }
    }
    j.min(n)
}

/// One line of Rust. `base` is what unstyled code is painted as — `Text`
/// in a `.rs` file, `CodeBlock` inside a markdown fence. Block comments,
/// strings, and raw strings carry across lines via `state`.
fn rust_line(chars: &[char], state: &mut RustState, base: Kind) -> Vec<SpanTok> {
    let n = chars.len();
    let mut spans = Vec::new();
    let mut i = 0;
    // the previous identifier on this line was the `fn` keyword
    let mut after_fn = false;
    while i < n {
        if state.block_depth > 0 {
            let end = scan_block_comment(chars, i, &mut state.block_depth);
            push_span(&mut spans, i, end, Kind::Comment);
            i = end;
            continue;
        }
        if let Some(h) = state.raw_hashes {
            match scan_raw(chars, i, h) {
                Some(end) => {
                    push_span(&mut spans, i, end, Kind::Str);
                    state.raw_hashes = None;
                    i = end;
                }
                None => {
                    push_span(&mut spans, i, n, Kind::Str);
                    return spans;
                }
            }
            continue;
        }
        if state.in_string {
            let (end, closed) = scan_string(chars, i);
            push_span(&mut spans, i, end, Kind::Str);
            state.in_string = !closed;
            i = end;
            continue;
        }
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        // comments
        if c == '/' && next == Some('/') {
            push_span(&mut spans, i, n, Kind::Comment);
            return spans;
        }
        if c == '/' && next == Some('*') {
            state.block_depth = 1;
            let end = scan_block_comment(chars, i + 2, &mut state.block_depth);
            push_span(&mut spans, i, end, Kind::Comment);
            i = end;
            continue;
        }
        // strings: "..", b"..", r".." / r#".."#, br".."
        {
            let mut q = i;
            if chars.get(q) == Some(&'b') && matches!(chars.get(q + 1), Some('"') | Some('r')) {
                q += 1;
            }
            let raw = chars.get(q) == Some(&'r');
            let mut hashes = 0;
            if raw {
                q += 1;
                while chars.get(q) == Some(&'#') {
                    hashes += 1;
                    q += 1;
                }
            }
            if chars.get(q) == Some(&'"') && (raw || q == i || q == i + 1) {
                if raw {
                    match scan_raw(chars, q + 1, hashes) {
                        Some(end) => {
                            push_span(&mut spans, i, end, Kind::Str);
                            i = end;
                        }
                        None => {
                            push_span(&mut spans, i, n, Kind::Str);
                            state.raw_hashes = Some(hashes);
                            return spans;
                        }
                    }
                } else {
                    let (end, closed) = scan_string(chars, q + 1);
                    push_span(&mut spans, i, end, Kind::Str);
                    state.in_string = !closed;
                    i = end;
                }
                continue;
            }
        }
        // char literal or lifetime
        if c == '\'' {
            if next == Some('\\') {
                // '\n', '\u{1F600}', ...: up to the closing quote
                let end = find(chars, i + 2, n, '\'').map(|e| e + 1).unwrap_or(n);
                push_span(&mut spans, i, end, Kind::Str);
                i = end;
                continue;
            }
            if chars.get(i + 2) == Some(&'\'') {
                push_span(&mut spans, i, i + 3, Kind::Str);
                i += 3;
                continue;
            }
            if next.is_some_and(is_ident_start) {
                let mut j = i + 1;
                while j < n && is_ident_char(chars[j]) {
                    j += 1;
                }
                push_span(&mut spans, i, j, Kind::TypeName);
                i = j;
                continue;
            }
        }
        // #[attribute] / #![attribute]
        if c == '#' && (next == Some('[') || (next == Some('!') && chars.get(i + 2) == Some(&'[')))
        {
            let mut depth = 0;
            let mut j = i;
            let mut end = n;
            while j < n {
                match chars[j] {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            end = j + 1;
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            push_span(&mut spans, i, end, Kind::Macro);
            i = end;
            continue;
        }
        // numbers: 42, 1_000, 0xff, 3.14, 1e9, 2u8
        if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < n
                && (is_ident_char(chars[j])
                    || (chars[j] == '.'
                        && chars.get(j + 1).is_some_and(|d| d.is_ascii_digit())
                        && chars[j - 1] != '.'))
            {
                j += 1;
            }
            push_span(&mut spans, i, j, Kind::Number);
            i = j;
            continue;
        }
        // identifiers: keyword, macro!, Type / primitive, function, or plain
        if is_ident_start(c) {
            let mut j = i + 1;
            while j < n && is_ident_char(chars[j]) {
                j += 1;
            }
            // raw identifier `r#type`: a plain name, whatever it says
            let raw = j == i + 1
                && c == 'r'
                && chars.get(j) == Some(&'#')
                && chars.get(j + 1).is_some_and(|&d| is_ident_start(d));
            if raw {
                j += 2;
                while j < n && is_ident_char(chars[j]) {
                    j += 1;
                }
            }
            let word: String = chars[i..j].iter().collect();
            let was_after_fn = after_fn;
            after_fn = false;
            let kind = if raw {
                if chars.get(j) == Some(&'(') || was_after_fn {
                    Kind::Function
                } else {
                    base
                }
            } else if chars.get(j) == Some(&'!') && chars.get(j + 1) != Some(&'=') {
                j += 1;
                Kind::Macro
            } else if RUST_KEYWORDS.contains(&word.as_str()) {
                after_fn = word == "fn";
                Kind::Keyword
            } else if RUST_PRIMITIVES.contains(&word.as_str()) || c.is_uppercase() {
                Kind::TypeName
            } else if was_after_fn || chars.get(j) == Some(&'(') {
                Kind::Function
            } else {
                base
            };
            push_span(&mut spans, i, j, kind);
            i = j;
            continue;
        }
        push_span(&mut spans, i, i + 1, base);
        i += 1;
    }
    spans
}

#[cfg(test)]
mod tests;
