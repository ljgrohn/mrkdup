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
