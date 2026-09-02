//! Line tokenizers for the C-family-ish languages that don't need Rust's
//! special cases: a table-driven one for JavaScript/TypeScript and SQL,
//! and a small dedicated one for CSS/SCSS. They reuse the code `Kind`s
//! (and so the theme keys) Rust introduced.

use super::{is_ident_char, is_ident_start, push_span, scan_string, Kind, SpanTok};

/// What carries across lines: a block comment, an unterminated string
/// (with its quote), and — for CSS — how deep inside `{}` we are.
#[derive(Default)]
pub struct CodeState {
    in_block: bool,
    in_string: Option<char>,
    brace_depth: usize,
}

/// A language for `generic_line`.
pub struct Lang {
    /// prefixes that start a comment to the end of the line
    line_comments: &'static [&'static str],
    /// block comment open / close (no nesting)
    block: Option<(&'static str, &'static str)>,
    /// string delimiters
    quotes: &'static [char],
    /// quotes whose strings may span lines
    multiline_quotes: &'static [char],
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    /// match keywords and types case-insensitively (SQL)
    case_insensitive: bool,
    /// `sigil` + identifier is a `Macro` (decorators, SQL variables)
    sigil: Option<char>,
    /// a capitalised identifier is a type name
    uppercase_is_type: bool,
}

pub const JS: Lang = Lang {
    line_comments: &["//"],
    block: Some(("/*", "*/")),
    quotes: &['"', '\'', '`'],
    multiline_quotes: &['`'],
    keywords: &[
        "abstract",
        "as",
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "declare",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "from",
        "function",
        "get",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "is",
        "keyof",
        "let",
        "namespace",
        "new",
        "null",
        "of",
        "override",
        "private",
        "protected",
        "public",
        "readonly",
        "return",
        "satisfies",
        "set",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "type",
        "typeof",
        "undefined",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ],
    types: &[
        "any", "bigint", "boolean", "never", "number", "object", "string", "symbol", "unknown",
    ],
    case_insensitive: false,
    sigil: Some('@'),
    uppercase_is_type: true,
};

pub const SQL: Lang = Lang {
    line_comments: &["--"],
    block: Some(("/*", "*/")),
    quotes: &['\'', '"'],
    multiline_quotes: &['\'', '"'],
    keywords: &[
        "add",
        "all",
        "alter",
        "and",
        "any",
        "as",
        "asc",
        "begin",
        "between",
        "by",
        "case",
        "cascade",
        "check",
        "column",
        "commit",
        "constraint",
        "create",
        "cross",
        "database",
        "default",
        "delete",
        "desc",
        "distinct",
        "drop",
        "else",
        "end",
        "except",
        "exists",
        "false",
        "foreign",
        "from",
        "full",
        "group",
        "having",
        "if",
        "in",
        "index",
        "inner",
        "insert",
        "intersect",
        "into",
        "is",
        "join",
        "key",
        "left",
        "like",
        "limit",
        "not",
        "null",
        "offset",
        "on",
        "or",
        "order",
        "outer",
        "primary",
        "references",
        "returning",
        "right",
        "rollback",
        "select",
        "set",
        "table",
        "then",
        "transaction",
        "true",
        "union",
        "unique",
        "update",
        "using",
        "values",
        "view",
        "when",
        "where",
        "with",
    ],
    types: &[
        "bigint",
        "blob",
        "bool",
        "boolean",
        "bytea",
        "char",
        "date",
        "decimal",
        "double",
        "float",
        "int",
        "integer",
        "interval",
        "json",
        "jsonb",
        "numeric",
        "real",
        "serial",
        "smallint",
        "text",
        "time",
        "timestamp",
        "timestamptz",
        "uuid",
        "varchar",
    ],
    case_insensitive: true,
    sigil: Some('@'),
    uppercase_is_type: false,
};

fn starts_with(chars: &[char], at: usize, s: &str) -> bool {
    s.chars()
        .enumerate()
        .all(|(k, c)| chars.get(at + k) == Some(&c))
}

/// Index just past the next `close` at or after `from`, if on this line.
fn find_str(chars: &[char], from: usize, close: &str) -> Option<usize> {
    (from..chars.len())
        .find(|&j| starts_with(chars, j, close))
        .map(|j| j + close.chars().count())
}

/// Scan a number from `i` (a digit): digits, `_`, one `.` followed by a
/// digit, exponents, and any trailing letters (suffixes, units).
fn scan_number(chars: &[char], i: usize) -> usize {
    let n = chars.len();
    let mut j = i + 1;
    while j < n {
        let c = chars[j];
        let dot =
            c == '.' && chars.get(j + 1).is_some_and(|d| d.is_ascii_digit()) && chars[j - 1] != '.';
        if is_ident_char(c) || dot || c == '%' {
            j += 1;
        } else {
            break;
        }
    }
    j
}

/// One line of `lang`. `base` is what unstyled code is painted as.
pub fn generic_line(
    chars: &[char],
    state: &mut CodeState,
    base: Kind,
    lang: &Lang,
) -> Vec<SpanTok> {
    let n = chars.len();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < n {
        if state.in_block {
            let close = lang.block.map(|b| b.1).unwrap_or("*/");
            match find_str(chars, i, close) {
                Some(end) => {
                    push_span(&mut spans, i, end, Kind::Comment);
                    state.in_block = false;
                    i = end;
                }
                None => {
                    push_span(&mut spans, i, n, Kind::Comment);
                    return spans;
                }
            }
            continue;
        }
        if let Some(q) = state.in_string {
            let (end, closed) = scan_quoted(chars, i, q);
            push_span(&mut spans, i, end, Kind::Str);
            if closed {
                state.in_string = None;
            }
            i = end;
            continue;
        }
        let c = chars[i];
        if lang.line_comments.iter().any(|p| starts_with(chars, i, p)) {
            push_span(&mut spans, i, n, Kind::Comment);
            return spans;
        }
        if let Some((open, close)) = lang.block {
            if starts_with(chars, i, open) {
                match find_str(chars, i + open.chars().count(), close) {
                    Some(end) => {
                        push_span(&mut spans, i, end, Kind::Comment);
                        i = end;
                    }
                    None => {
                        push_span(&mut spans, i, n, Kind::Comment);
                        state.in_block = true;
                        return spans;
                    }
                }
                continue;
            }
        }
        if lang.quotes.contains(&c) {
            let (end, closed) = scan_quoted(chars, i + 1, c);
            push_span(&mut spans, i, end, Kind::Str);
            if !closed && lang.multiline_quotes.contains(&c) {
                state.in_string = Some(c);
            }
            i = end;
            continue;
        }
        if Some(c) == lang.sigil && chars.get(i + 1).is_some_and(|&d| is_ident_start(d)) {
            let mut j = i + 1;
            while j < n && is_ident_char(chars[j]) {
                j += 1;
            }
            push_span(&mut spans, i, j, Kind::Macro);
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let j = scan_number(chars, i);
            push_span(&mut spans, i, j, Kind::Number);
            i = j;
            continue;
        }
        if is_ident_start(c) || c == '$' {
            let mut j = i + 1;
            while j < n && (is_ident_char(chars[j]) || chars[j] == '$') {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect();
            let key = if lang.case_insensitive {
                word.to_ascii_lowercase()
            } else {
                word.clone()
            };
            let kind = if lang.keywords.contains(&key.as_str()) {
                Kind::Keyword
            } else if lang.types.contains(&key.as_str()) {
                Kind::TypeName
            } else if chars.get(j) == Some(&'(') {
                Kind::Function
            } else if lang.uppercase_is_type && c.is_uppercase() {
                Kind::TypeName
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

/// Like `scan_string` but for an arbitrary quote char; a doubled quote
/// (`''` in SQL) is an escape too, which for our purposes just reads as
/// two adjacent strings and merges into one span.
fn scan_quoted(chars: &[char], from: usize, q: char) -> (usize, bool) {
    if q == '"' {
        return scan_string(chars, from);
    }
    let n = chars.len();
    let mut j = from;
    while j < n {
        match chars[j] {
            '\\' => j += 2,
            c if c == q => return (j + 1, true),
            _ => j += 1,
        }
    }
    (n, false)
}

/// One line of CSS / SCSS. Outside `{}` everything is a selector
/// (`TypeName`); inside, a name before `:` is a property (`Keyword`),
/// `.class` / `#id` / `&` runs are nested selectors, `name(` is a
/// function, `#hex` and numbers-with-units are `Number`s, `@rules` are
/// `Macro`s, `!important` a `Keyword`. Comments and brace depth carry
/// across lines.
pub fn css_line(chars: &[char], state: &mut CodeState, base: Kind) -> Vec<SpanTok> {
    let n = chars.len();
    let mut spans = Vec::new();
    let mut i = 0;
    let is_sel_char = |c: char| {
        is_ident_char(c)
            || matches!(
                c,
                '-' | '.' | '#' | '&' | ':' | '*' | '>' | '+' | '~' | '[' | ']' | '=' | '"' | '\''
            )
    };
    // inside `(...)` on this line — `@media (max-width: 600px)`, `:not(.x)`
    // — names and numbers read like declarations, not selectors
    let mut paren = 0usize;
    // after a top-level `$var:` (SCSS) the rest of the line is a value
    let mut decl = false;
    while i < n {
        if state.in_block {
            match find_str(chars, i, "*/") {
                Some(end) => {
                    push_span(&mut spans, i, end, Kind::Comment);
                    state.in_block = false;
                    i = end;
                }
                None => {
                    push_span(&mut spans, i, n, Kind::Comment);
                    return spans;
                }
            }
            continue;
        }
        let c = chars[i];
        if starts_with(chars, i, "/*") {
            match find_str(chars, i + 2, "*/") {
                Some(end) => {
                    push_span(&mut spans, i, end, Kind::Comment);
                    i = end;
                }
                None => {
                    push_span(&mut spans, i, n, Kind::Comment);
                    state.in_block = true;
                    return spans;
                }
            }
            continue;
        }
        // SCSS line comment, only at the start of the line so `url(http://…)` survives
        if starts_with(chars, i, "//") && chars[..i].iter().all(|c| c.is_whitespace()) {
            push_span(&mut spans, i, n, Kind::Comment);
            return spans;
        }
        if c == '"' || c == '\'' {
            let (end, _) = scan_quoted(chars, i + 1, c);
            push_span(&mut spans, i, end, Kind::Str);
            i = end;
            continue;
        }
        if c == '{' {
            state.brace_depth += 1;
            push_span(&mut spans, i, i + 1, base);
            i += 1;
            continue;
        }
        if c == '}' {
            state.brace_depth = state.brace_depth.saturating_sub(1);
            push_span(&mut spans, i, i + 1, base);
            i += 1;
            continue;
        }
        if c == '@' && chars.get(i + 1).is_some_and(|&d| is_ident_start(d)) {
            let mut j = i + 1;
            while j < n && (is_ident_char(chars[j]) || chars[j] == '-') {
                j += 1;
            }
            push_span(&mut spans, i, j, Kind::Macro);
            i = j;
            continue;
        }
        if c == '!' && starts_with(chars, i + 1, "important") {
            push_span(&mut spans, i, i + 10, Kind::Keyword);
            i += 10;
            continue;
        }
        if c == '(' || c == ')' {
            paren = if c == '(' {
                paren + 1
            } else {
                paren.saturating_sub(1)
            };
            push_span(&mut spans, i, i + 1, base);
            i += 1;
            continue;
        }
        let in_rule = state.brace_depth > 0 || paren > 0 || decl;
        // #hex colour inside a rule; a selector outside
        if c == '#' && in_rule {
            let mut j = i + 1;
            while j < n && chars[j].is_ascii_hexdigit() {
                j += 1;
            }
            if j > i + 1 && !chars.get(j).is_some_and(|&d| is_ident_char(d)) {
                push_span(&mut spans, i, j, Kind::Number);
                i = j;
                continue;
            }
        }
        // numbers with units: 12px, 1.5em, 100%, .5s, -2px
        let num_start = c.is_ascii_digit()
            || ((c == '.' || c == '-')
                && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())
                && in_rule);
        if num_start {
            let j = scan_number(chars, i);
            push_span(&mut spans, i, j, Kind::Number);
            i = j;
            continue;
        }
        if !in_rule {
            // `$var: value` at top level (SCSS): the name is plain
            if c == '$' {
                let mut j = i + 1;
                while j < n && (is_ident_char(chars[j]) || chars[j] == '-') {
                    j += 1;
                }
                push_span(&mut spans, i, j, base);
                i = j;
                decl = true;
                continue;
            }
            // selector run: up to `{`, `,`, `(`, a comment, or a string
            if is_sel_char(c) {
                let mut j = i + 1;
                while j < n
                    && (is_sel_char(chars[j]) || chars[j] == ' ')
                    && !starts_with(chars, j, "/*")
                {
                    j += 1;
                }
                // trailing spaces belong to the base
                while j > i + 1 && chars[j - 1] == ' ' {
                    j -= 1;
                }
                push_span(&mut spans, i, j, Kind::TypeName);
                i = j;
                continue;
            }
        } else if is_ident_start(c) || c == '-' || c == '$' || c == '.' || c == '#' || c == '&' {
            let nested = matches!(c, '.' | '#' | '&');
            let mut j = i + 1;
            while j < n
                && (is_ident_char(chars[j])
                    || chars[j] == '-'
                    || (nested && matches!(chars[j], '.' | ':' | '&' | ' ' | '>')))
            {
                j += 1;
            }
            while nested && j > i + 1 && chars[j - 1] == ' ' {
                j -= 1;
            }
            let mut k = j;
            while k < n && chars[k] == ' ' {
                k += 1;
            }
            let kind = if nested || chars.get(k) == Some(&'{') {
                Kind::TypeName
            } else if chars.get(k) == Some(&':') && !matches!(c, '$') {
                Kind::Keyword
            } else if chars.get(j) == Some(&'(') {
                Kind::Function
            } else {
                base
            };
            push_span(&mut spans, i, j, kind);
            i = j;
            // url(...) holds a bare URL: a string, not code
            if kind == Kind::Function && chars[i - 3..i] == ['u', 'r', 'l'] {
                let close = chars[i..].iter().position(|&d| d == ')').map(|p| i + p);
                let end = close.unwrap_or(n);
                push_span(&mut spans, i, i + 1, base);
                push_span(&mut spans, i + 1, end, Kind::Str);
                i = end;
            }
            continue;
        }
        push_span(&mut spans, i, i + 1, base);
        i += 1;
    }
    spans
}
