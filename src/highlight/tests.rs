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
fn heading_with_inline_bold() {
    let s = hl(&["# **hi**"]);
    let k = kinds(&s[0]);
    assert!(k.contains(&Kind::Bold));
    assert_covers(&s[0], 8);
}

#[test]
fn quote_with_inline_bold() {
    let s = hl(&["> **bold**"]);
    let k = kinds(&s[0]);
    assert!(k.contains(&Kind::Bold));
    assert_covers(&s[0], 10);
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
    let s = hl(&["```python", "let x = 1;", "```", "after"]);
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

// ---- rust -------------------------------------------------------------

fn rs(lines: &[&str]) -> Vec<Vec<SpanTok>> {
    let v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    highlight(&v, FileKind::Rust)
}

/// The kind painted over the first occurrence of `needle` in `line`.
fn kind_of(line: &str, spans: &[SpanTok], needle: &str) -> Kind {
    let at = line.find(needle).expect("needle present");
    let at = line[..at].chars().count();
    spans
        .iter()
        .find(|s| at >= s.start && at < s.end)
        .map(|s| s.kind)
        .expect("covered")
}

/// `(kind, text)` per span, for readable assertions.
fn pieces(line: &str, spans: &[SpanTok]) -> Vec<(Kind, String)> {
    let chars: Vec<char> = line.chars().collect();
    spans
        .iter()
        .map(|s| (s.kind, chars[s.start..s.end].iter().collect()))
        .collect()
}

#[test]
fn rs_extension_is_rust() {
    assert_eq!(
        file_kind(Some(std::path::Path::new("src/main.rs"))),
        FileKind::Rust
    );
    assert_eq!(
        file_kind(Some(std::path::Path::new("notes.md"))),
        FileKind::Markdown
    );
}

#[test]
fn rust_keywords_types_and_macros() {
    let line = "pub fn main() -> Option<u8> { println!(\"hi\"); }";
    let s = rs(&[line]);
    assert_covers(&s[0], line.chars().count());
    let p = pieces(line, &s[0]);
    assert!(p.contains(&(Kind::Keyword, "pub".into())));
    assert!(p.contains(&(Kind::Keyword, "fn".into())));
    assert_eq!(kind_of(line, &s[0], "main"), Kind::Function);
    assert!(p.contains(&(Kind::TypeName, "Option".into())));
    assert!(p.contains(&(Kind::TypeName, "u8".into())));
    assert!(p.contains(&(Kind::Macro, "println!".into())));
    assert!(p.contains(&(Kind::Str, "\"hi\"".into())));
}

#[test]
fn rust_not_equal_is_not_a_macro() {
    let line = "if a != b {}";
    let s = rs(&[line]);
    assert_eq!(kind_of(line, &s[0], "a"), Kind::Text);
    assert!(!s[0].iter().any(|t| t.kind == Kind::Macro));
}

#[test]
fn rust_line_comment_runs_to_the_end() {
    let line = "let x = 1; // one \"not a string\"";
    let p = pieces(line, &rs(&[line])[0]);
    assert_eq!(
        p.last().unwrap(),
        &(Kind::Comment, "// one \"not a string\"".into())
    );
    assert!(p.contains(&(Kind::Number, "1".into())));
}

#[test]
fn rust_block_comments_nest_and_span_lines() {
    let s = rs(&["a /* one /* two */", "still */ b"]);
    assert_eq!(kinds(&s[0]), vec![Kind::Text, Kind::Comment]);
    assert_eq!(
        pieces("still */ b", &s[1])[0],
        (Kind::Comment, "still */".into())
    );
    assert_eq!(kinds(&s[1]), vec![Kind::Comment, Kind::Text]);
}

#[test]
fn rust_strings_escape_and_span_lines() {
    let line = "let s = \"a \\\" b\"; x";
    let p = pieces(line, &rs(&[line])[0]);
    assert!(p.contains(&(Kind::Str, "\"a \\\" b\"".into())));
    let s = rs(&["let s = \"open", "close\"; y"]);
    assert_eq!(kinds(&s[0]), vec![Kind::Keyword, Kind::Text, Kind::Str]);
    assert_eq!(
        pieces("close\"; y", &s[1])[0],
        (Kind::Str, "close\"".into())
    );
}

#[test]
fn rust_raw_and_byte_strings() {
    let line = "r#\"a \" b\"# b\"x\" br\"y\"";
    let p = pieces(line, &rs(&[line])[0]);
    assert!(p.contains(&(Kind::Str, "r#\"a \" b\"#".into())));
    assert!(p.contains(&(Kind::Str, "b\"x\"".into())));
    assert!(p.contains(&(Kind::Str, "br\"y\"".into())));
    // a raw string carries across lines until its matching "#
    let s = rs(&["r##\"one", "two\"# not yet", "three\"## done"]);
    assert_eq!(kinds(&s[1]), vec![Kind::Str]);
    assert_eq!(kinds(&s[2]), vec![Kind::Str, Kind::Text]);
}

#[test]
fn rust_char_literals_versus_lifetimes() {
    let line = "fn f<'a>(x: &'a str) -> char { '\\n' } // 'x'";
    let p = pieces(line, &rs(&[line])[0]);
    assert!(p.contains(&(Kind::TypeName, "'a".into())), "{p:?}");
    assert!(p.contains(&(Kind::Str, "'\\n'".into())), "{p:?}");
    let line = "let c = 'x';";
    let p = pieces(line, &rs(&[line])[0]);
    assert!(p.contains(&(Kind::Str, "'x'".into())));
}

#[test]
fn rust_attributes_and_numbers() {
    let line = "#[derive(Debug, Clone)] const N: usize = 0xff + 1_000 + 3.14 + 2u8;";
    let p = pieces(line, &rs(&[line])[0]);
    assert_eq!(p[0], (Kind::Macro, "#[derive(Debug, Clone)]".into()));
    for num in ["0xff", "1_000", "3.14", "2u8"] {
        assert!(p.contains(&(Kind::Number, num.into())), "{num}: {p:?}");
    }
    let line = "#![allow(dead_code)]";
    assert_eq!(kinds(&rs(&[line])[0]), vec![Kind::Macro]);
}

#[test]
fn rust_range_is_not_a_float() {
    let line = "for i in 0..10 {}";
    let p = pieces(line, &rs(&[line])[0]);
    assert!(p.contains(&(Kind::Number, "0".into())));
    assert!(p.contains(&(Kind::Number, "10".into())));
    assert!(p.contains(&(Kind::Text, "..".into())));
}

#[test]
fn rust_fence_in_markdown_gets_rust_styling_on_a_code_base() {
    let s = hl(&["```rust", "let x = \"s\"; // c", "```", "let y"]);
    assert_eq!(kinds(&s[0]), vec![Kind::Mark]);
    assert_eq!(
        kinds(&s[1]),
        vec![
            Kind::Keyword,
            Kind::CodeBlock,
            Kind::Str,
            Kind::CodeBlock,
            Kind::Comment
        ]
    );
    assert_eq!(kinds(&s[2]), vec![Kind::Mark]);
    assert_eq!(kinds(&s[3]), vec![Kind::Text]); // back to markdown
                                                // other fences stay flat code
    let s = hl(&["```python", "let x = 1", "```"]);
    assert_eq!(kinds(&s[1]), vec![Kind::CodeBlock]);
}

#[test]
fn rust_fence_state_resets_at_the_closing_fence() {
    let s = hl(&["```rs", "/* open", "```", "plain"]);
    assert_eq!(kinds(&s[1]), vec![Kind::Comment]);
    assert_eq!(kinds(&s[2]), vec![Kind::Mark]);
    assert_eq!(kinds(&s[3]), vec![Kind::Text]);
}

#[test]
fn rust_every_line_is_fully_covered() {
    let src = [
        "use std::io;",
        "",
        "/// doc",
        "#[derive(Default)]",
        "struct S<'a> { name: &'a str, n: u32 }",
        "impl<'a> S<'a> {",
        "    fn go(&self) -> Result<(), io::Error> { Ok(()) }",
        "}",
    ];
    let s = rs(&src);
    for (line, spans) in src.iter().zip(&s) {
        assert_covers(spans, line.chars().count());
    }
    assert_eq!(kinds(&s[2]), vec![Kind::Comment]);
}

#[test]
fn rust_function_names_and_raw_identifiers() {
    let line = "fn r#type(x: u8) -> u8 { helper(x) + count + r#match }";
    let s = rs(&[line]);
    assert_eq!(kind_of(line, &s[0], "r#type"), Kind::Function); // after fn
    assert_eq!(kind_of(line, &s[0], "helper"), Kind::Function); // called
    assert_eq!(kind_of(line, &s[0], "count"), Kind::Text);
    assert_eq!(kind_of(line, &s[0], "r#match"), Kind::Text); // not a keyword
    assert_eq!(kind_of(line, &s[0], "x:"), Kind::Text);
    // keywords and types before `(` keep their own kinds
    let line = "if Some(v) = f() { while (x) {} }";
    let s = rs(&[line]);
    assert_eq!(kind_of(line, &s[0], "Some"), Kind::TypeName);
    assert_eq!(kind_of(line, &s[0], "while"), Kind::Keyword);
    assert_eq!(kind_of(line, &s[0], "f()"), Kind::Function);
}

// ---- javascript / css / sql -----------------------------------------

fn code(lines: &[&str], kind: FileKind) -> Vec<Vec<SpanTok>> {
    let v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    highlight(&v, kind)
}

#[test]
fn extensions_map_to_code_kinds() {
    use std::path::Path;
    for (name, kind) in [
        ("app.js", FileKind::JavaScript),
        ("app.mjs", FileKind::JavaScript),
        ("App.TSX", FileKind::JavaScript),
        ("types.ts", FileKind::JavaScript),
        ("site.css", FileKind::Css),
        ("site.scss", FileKind::Css),
        ("schema.sql", FileKind::Sql),
        ("lib.rs", FileKind::Rust),
        ("index.htm", FileKind::Html),
        ("notes.txt", FileKind::Markdown),
    ] {
        assert_eq!(file_kind(Some(Path::new(name))), kind, "{name}");
    }
}

#[test]
fn js_keywords_types_functions_strings_and_comments() {
    let line = "const total = compute(items.length) + Math.max(1, 2); // sum";
    let s = code(&[line], FileKind::JavaScript);
    assert_covers(&s[0], line.chars().count());
    assert_eq!(kind_of(line, &s[0], "const"), Kind::Keyword);
    assert_eq!(kind_of(line, &s[0], "total"), Kind::Text);
    assert_eq!(kind_of(line, &s[0], "compute"), Kind::Function);
    assert_eq!(kind_of(line, &s[0], "items"), Kind::Text);
    assert_eq!(kind_of(line, &s[0], "Math"), Kind::TypeName);
    assert_eq!(kind_of(line, &s[0], "max"), Kind::Function);
    assert_eq!(kind_of(line, &s[0], "2"), Kind::Number);
    assert_eq!(kind_of(line, &s[0], "// sum"), Kind::Comment);
    let line = "let s: string = 'it\\'s' + \"x\"; @decorator class Foo extends Bar {}";
    let s = code(&[line], FileKind::JavaScript);
    assert_eq!(kind_of(line, &s[0], "string"), Kind::TypeName);
    assert_eq!(kind_of(line, &s[0], "'it"), Kind::Str);
    assert_eq!(kind_of(line, &s[0], "\"x\""), Kind::Str);
    assert_eq!(kind_of(line, &s[0], "@decorator"), Kind::Macro);
    assert_eq!(kind_of(line, &s[0], "class"), Kind::Keyword);
    assert_eq!(kind_of(line, &s[0], "Foo"), Kind::TypeName);
}

#[test]
fn js_template_literals_and_block_comments_span_lines() {
    let s = code(
        &[
            "const t = `line one",
            "line two`; /* open",
            "close */ let x = 1;",
        ],
        FileKind::JavaScript,
    );
    assert_eq!(kinds(&s[0]), vec![Kind::Keyword, Kind::Text, Kind::Str]);
    assert_eq!(kinds(&s[1]), vec![Kind::Str, Kind::Text, Kind::Comment]);
    assert_eq!(kinds(&s[2])[0], Kind::Comment);
    assert_eq!(kind_of("close */ let x = 1;", &s[2], "let"), Kind::Keyword);
    // a plain quote does not carry across lines
    let s = code(&["a = 'open", "b = 1"], FileKind::JavaScript);
    assert_eq!(kind_of("b = 1", &s[1], "b"), Kind::Text);
}

#[test]
fn css_selectors_properties_values_and_at_rules() {
    let src = [
        "@media (max-width: 600px) {",
        "  .card > a:hover, #main { color: #fff; margin: -2px 1.5em !important; }",
        "}",
        "body { background: url(http://x/y.png) rgb(0, 0, 0); }",
        "/* multi",
        "line */ h1 { }",
    ];
    let s = code(&src, FileKind::Css);
    for (line, spans) in src.iter().zip(&s) {
        assert_covers(spans, line.chars().count());
    }
    assert_eq!(kind_of(src[0], &s[0], "@media"), Kind::Macro);
    assert_eq!(kind_of(src[0], &s[0], "600px"), Kind::Number);
    assert_eq!(kind_of(src[1], &s[1], ".card"), Kind::TypeName);
    assert_eq!(kind_of(src[1], &s[1], "#main"), Kind::TypeName);
    assert_eq!(kind_of(src[1], &s[1], "color"), Kind::Keyword);
    assert_eq!(kind_of(src[1], &s[1], "#fff"), Kind::Number);
    assert_eq!(kind_of(src[1], &s[1], "margin"), Kind::Keyword);
    assert_eq!(kind_of(src[1], &s[1], "-2px"), Kind::Number);
    assert_eq!(kind_of(src[1], &s[1], "1.5em"), Kind::Number);
    assert_eq!(kind_of(src[1], &s[1], "!important"), Kind::Keyword);
    assert_eq!(kind_of(src[3], &s[3], "body"), Kind::TypeName);
    assert_eq!(kind_of(src[3], &s[3], "url"), Kind::Function);
    assert_eq!(kind_of(src[3], &s[3], "http"), Kind::Str); // the URL, not a // comment
    assert_eq!(kind_of(src[3], &s[3], "rgb"), Kind::Function);
    assert_eq!(kinds(&s[4]), vec![Kind::Comment]);
    assert_eq!(kind_of(src[5], &s[5], "line */"), Kind::Comment);
    assert_eq!(kind_of(src[5], &s[5], "h1"), Kind::TypeName);
}

#[test]
fn scss_nesting_variables_and_line_comments() {
    let src = [
        "$gap: 4px;",
        ".a {",
        "  // note",
        "  &:hover { padding: $gap; }",
        "  .b { x: y }",
        "}",
    ];
    let s = code(&src, FileKind::Css);
    assert_eq!(kind_of(src[0], &s[0], "$gap"), Kind::Text);
    assert_eq!(kind_of(src[0], &s[0], "4px"), Kind::Number);
    assert_eq!(kinds(&s[2]), vec![Kind::Text, Kind::Comment]);
    assert_eq!(kind_of(src[3], &s[3], "&:hover"), Kind::TypeName);
    assert_eq!(kind_of(src[3], &s[3], "padding"), Kind::Keyword);
    assert_eq!(kind_of(src[3], &s[3], "$gap"), Kind::Text);
    assert_eq!(kind_of(src[4], &s[4], ".b"), Kind::TypeName);
    assert_eq!(kind_of(src[4], &s[4], "x:"), Kind::Keyword);
}

#[test]
fn sql_is_case_insensitive_with_types_functions_and_comments() {
    let line = "SELECT count(*), name FROM users u WHERE id > 10 AND note = 'it''s' -- tail";
    let s = code(&[line], FileKind::Sql);
    assert_covers(&s[0], line.chars().count());
    assert_eq!(kind_of(line, &s[0], "SELECT"), Kind::Keyword);
    assert_eq!(kind_of(line, &s[0], "count"), Kind::Function);
    assert_eq!(kind_of(line, &s[0], "name"), Kind::Text);
    assert_eq!(kind_of(line, &s[0], "FROM"), Kind::Keyword);
    assert_eq!(kind_of(line, &s[0], "users"), Kind::Text);
    assert_eq!(kind_of(line, &s[0], "10"), Kind::Number);
    assert_eq!(kind_of(line, &s[0], "'it"), Kind::Str);
    assert_eq!(kind_of(line, &s[0], "s'"), Kind::Str);
    assert_eq!(kind_of(line, &s[0], "-- tail"), Kind::Comment);
    let line =
        "create table t (id serial primary key, body text, n numeric(10,2), when timestamptz);";
    let s = code(&[line], FileKind::Sql);
    assert_eq!(kind_of(line, &s[0], "create"), Kind::Keyword);
    assert_eq!(kind_of(line, &s[0], "serial"), Kind::TypeName);
    assert_eq!(kind_of(line, &s[0], "text"), Kind::TypeName);
    assert_eq!(kind_of(line, &s[0], "numeric"), Kind::TypeName);
    assert_eq!(kind_of(line, &s[0], "timestamptz"), Kind::TypeName);
    let s = code(&["/* a", "b */ SET @x = 1;"], FileKind::Sql);
    assert_eq!(kinds(&s[0]), vec![Kind::Comment]);
    assert_eq!(kind_of("b */ SET @x = 1;", &s[1], "@x"), Kind::Macro);
}

#[test]
fn fence_tags_pick_the_language() {
    let s = hl(&[
        "```js",
        "const a = 1;",
        "```",
        "```CSS",
        "a { color: red }",
        "```",
        "```sql",
        "select 1",
        "```",
    ]);
    assert_eq!(
        kinds(&s[1]),
        vec![
            Kind::Keyword,
            Kind::CodeBlock,
            Kind::Number,
            Kind::CodeBlock
        ]
    );
    assert_eq!(kind_of("a { color: red }", &s[4], "a"), Kind::TypeName);
    assert_eq!(kind_of("a { color: red }", &s[4], "color"), Kind::Keyword);
    assert_eq!(kind_of("a { color: red }", &s[4], "red"), Kind::CodeBlock);
    assert_eq!(
        kinds(&s[7]),
        vec![Kind::Keyword, Kind::CodeBlock, Kind::Number]
    );
}

#[test]
fn fence_state_resets_between_code_fences() {
    // an unterminated JS template literal must not leak into a later fence
    let s = hl(&[
        "```js",
        "const t = `open",
        "```",
        "```sql",
        "select 1",
        "```",
    ]);
    assert_eq!(
        kinds(&s[4]),
        vec![Kind::Keyword, Kind::CodeBlock, Kind::Number]
    );
}
