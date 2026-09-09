use super::*;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[test]
fn wikilink_plain_alias_and_heading() {
    let w = parse_wikilink_at("see [[notes/plan]] done", 7).unwrap();
    assert_eq!(
        (w.target, w.alias, w.heading),
        ("notes/plan".into(), None, None)
    );
    let w = parse_wikilink_at("see [[plan|the plan]] done", 8).unwrap();
    assert_eq!(w.alias.as_deref(), Some("the plan"));
    assert_eq!(w.target, "plan");
    let w = parse_wikilink_at("see [[plan#next steps]] done", 8).unwrap();
    assert_eq!(w.heading.as_deref(), Some("next steps"));
}

#[test]
fn wikilink_col_must_be_inside_brackets() {
    let line = "a [[plan]] b";
    assert!(parse_wikilink_at(line, 0).is_none()); // on 'a'
    assert!(parse_wikilink_at(line, 2).is_some()); // on '['
    assert!(parse_wikilink_at(line, 9).is_some()); // on ']'
    assert!(parse_wikilink_at(line, 11).is_none()); // past it
}

#[test]
fn wikilink_rejects_unclosed_and_empty() {
    assert!(parse_wikilink_at("a [[oops b", 4).is_none());
    assert!(parse_wikilink_at("a [[]] b", 4).is_none());
    assert!(parse_wikilink_at("a [[|alias]] b", 5).is_none());
}

#[test]
fn md_link_parses_url_span() {
    let m = parse_md_link_at("see [t](docs/a.md) ok", 9).unwrap();
    assert_eq!(m.url, "docs/a.md");
    assert!(parse_md_link_at("see [t](docs/a.md) ok", 0).is_none());
}

#[test]
fn wikilink_hash_after_pipe_belongs_to_alias() {
    let w = parse_wikilink_at("see [[plan#sec|see #2]] done", 8).unwrap();
    assert_eq!(w.target, "plan");
    assert_eq!(w.heading.as_deref(), Some("sec"));
    assert_eq!(w.alias.as_deref(), Some("see #2"));
}

#[test]
fn resolve_prefers_sibling_dir_then_root_and_adds_md() {
    let root = std::path::Path::new("/vault");
    let dir = std::path::Path::new("/vault/notes");
    let exists = |p: &std::path::Path| {
        p == Path::new("/vault/notes/plan.md") || p == Path::new("/vault/shared.md")
    };
    assert_eq!(
        resolve("plan", dir, root, &exists).unwrap(),
        Path::new("/vault/notes/plan.md")
    );
    assert_eq!(
        resolve("shared", dir, root, &exists).unwrap(),
        Path::new("/vault/shared.md")
    );
    assert_eq!(resolve("readme.md", dir, root, &exists), None);
    assert!(resolve("", dir, root, &exists).is_none());
}

#[test]
fn normalize_lexical_resolves_dots_without_touching_disk() {
    assert_eq!(
        normalize_lexical(Path::new("/vault/notes/../sib/c.md")),
        Path::new("/vault/sib/c.md")
    );
    assert_eq!(
        normalize_lexical(Path::new("/vault/./notes/b.md")),
        Path::new("/vault/notes/b.md")
    );
    assert_eq!(
        normalize_lexical(Path::new("/vault/notes")),
        Path::new("/vault/notes")
    );
    // nothing left to pop: kept, so escaping paths stay recognizable
    assert_eq!(
        normalize_lexical(Path::new("/vault/../../x.md")),
        Path::new("/../x.md")
    );
}

#[test]
fn resolve_anchors_leading_slash_at_root() {
    let root = std::path::Path::new("/vault");
    let dir = std::path::Path::new("/vault/notes");
    let exists = |p: &std::path::Path| p == Path::new("/vault/abs.md");
    // never the filesystem absolute, even when it exists there
    assert_eq!(
        resolve("/abs", dir, root, &exists).unwrap(),
        Path::new("/vault/abs.md")
    );
    assert!(resolve("/", dir, root, &exists).is_none());
}

#[test]
fn resolve_returns_normalized_paths() {
    let root = std::path::Path::new("/vault");
    let dir = std::path::Path::new("/vault/notes");
    let exists = |p: &std::path::Path| p == Path::new("/vault/sib/c.md");
    assert_eq!(
        resolve("../sib/c", dir, root, &exists).unwrap(),
        Path::new("/vault/sib/c.md")
    );
}

#[test]
fn backlink_matches_all_three_forms_only() {
    assert!(backlink_matches("see [[plan]]", "plan"));
    assert!(backlink_matches("see [[plan|P]]", "plan"));
    assert!(backlink_matches("see [[plan#H]]", "plan"));
    assert!(!backlink_matches("see [[planet]]", "plan"));
    assert!(!backlink_matches("see [[my plan]]", "plan"));
}

#[test]
fn scan_backlinks_finds_only_linkers_sorted() {
    let root = std::env::temp_dir().join("mrkdup-links-scan");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("a.md"), "see [[b]]\n").unwrap();
    fs::write(root.join("sub/c.md"), "see [[b#H]]\n").unwrap();
    fs::write(root.join("d.md"), "see [[other]]\n").unwrap();
    fs::write(root.join("bin.dat"), [0u8, 1, 2, 3]).unwrap();
    let found = scan_backlinks(&root, false, "b");
    assert_eq!(found, vec![root.join("a.md"), root.join("sub/c.md")]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn candidates_lists_dir_then_root_each_raw_then_md() {
    let root = Path::new("/vault");
    let dir = Path::new("/vault/notes");
    assert_eq!(
        candidates("plan", dir, root),
        vec![
            PathBuf::from("/vault/notes/plan"),
            PathBuf::from("/vault/notes/plan.md"),
            PathBuf::from("/vault/plan"),
            PathBuf::from("/vault/plan.md"),
        ]
    );
    // a leading `/` anchors at the root only; `..` is normalized away
    assert_eq!(
        candidates("/abs", dir, root),
        vec![PathBuf::from("/vault/abs"), PathBuf::from("/vault/abs.md")]
    );
    assert_eq!(
        candidates("../sib/c", dir, root)[0],
        PathBuf::from("/vault/sib/c")
    );
    assert!(candidates("", dir, root).is_empty());
    assert!(candidates("/", dir, root).is_empty());
}

#[test]
fn create_target_is_the_md_candidate_resolve_would_have_found() {
    let root = Path::new("/vault");
    let dir = Path::new("/vault/notes");
    assert_eq!(
        create_target("new", dir, root),
        Some(PathBuf::from("/vault/notes/new.md"))
    );
    assert_eq!(
        create_target("/new", dir, root),
        Some(PathBuf::from("/vault/new.md"))
    );
    assert_eq!(
        create_target("../sib/new", dir, root),
        Some(PathBuf::from("/vault/sib/new.md"))
    );
    // no `.md` yet: creation appends one, like Obsidian
    assert_eq!(
        create_target("notes.txt", dir, root),
        Some(PathBuf::from("/vault/notes/notes.txt.md"))
    );
    assert_eq!(create_target("/", dir, root), None);
}

#[test]
fn nested_open_brackets_make_the_inner_link_win() {
    // the outer `[[` is plain text; only `[[b]]` is a link
    let line = "[[a [[b]]";
    let all = wikilinks(line);
    assert_eq!(all.len(), 1);
    assert_eq!(
        (all[0].target.as_str(), all[0].start, all[0].end),
        ("b", 4, 9)
    );
    assert!(parse_wikilink_at(line, 2).is_none()); // on `a`
    assert_eq!(parse_wikilink_at(line, 6).unwrap().target, "b");
    // `[[[[b]]`: the first accepted open is index 1 (`[[[b]]`, target `[`)
    let all = wikilinks("[[[[b]]");
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].start, all[0].end), (1, 7));
}

#[test]
fn wikilinks_lists_every_link_in_a_line_in_order() {
    let all = wikilinks("[[a]] and [[b|B]] and [[c#H]] and [[oops");
    let targets: Vec<&str> = all.iter().map(|w| w.target.as_str()).collect();
    assert_eq!(targets, ["a", "b", "c"]);
    assert_eq!(all[1].alias.as_deref(), Some("B"));
    assert_eq!(all[2].heading.as_deref(), Some("H"));
    assert!(wikilinks("no links here").is_empty());
}

#[test]
fn wikilink_span_accepts_exactly_what_the_parser_accepts() {
    for line in ["a [[plan]] b", "a [[plan|P]] b", "a [[p#H]] b", "[[a](b)]]"] {
        let chars: Vec<char> = line.chars().collect();
        let open = chars.windows(2).position(|w| w == ['[', '[']).unwrap();
        let span = wikilink_span(&chars, open, chars.len()).unwrap();
        let link = parse_wikilink_at(line, open).unwrap();
        assert_eq!((link.start, link.end), (open, span.close + 2), "{line}");
    }
    for line in ["a [[oops b", "a [[]] b", "a [[|alias]] b", "a [[#h]] b"] {
        let chars: Vec<char> = line.chars().collect();
        let open = chars.windows(2).position(|w| w == ['[', '[']).unwrap();
        assert!(wikilink_span(&chars, open, chars.len()).is_none(), "{line}");
    }
}

#[test]
fn dots_in_a_note_name_are_not_an_extension() {
    let root = Path::new("/vault");
    let dir = Path::new("/vault/notes");
    let exists = |p: &Path| p == Path::new("/vault/notes/v1.2.md");
    assert_eq!(
        resolve("v1.2", dir, root, &exists).unwrap(),
        Path::new("/vault/notes/v1.2.md")
    );
    assert_eq!(
        create_target("v1.2", dir, root),
        Some(PathBuf::from("/vault/notes/v1.2.md"))
    );
    // an explicit `.md` is never doubled
    assert_eq!(
        candidates("plan.md", dir, root),
        vec![
            PathBuf::from("/vault/notes/plan.md"),
            PathBuf::from("/vault/plan.md"),
        ]
    );
    // a real file with another extension still resolves as written
    let exists = |p: &Path| p == Path::new("/vault/notes/photo.png");
    assert_eq!(
        resolve("photo.png", dir, root, &exists).unwrap(),
        Path::new("/vault/notes/photo.png")
    );
}

#[test]
fn heading_row_matches_headings_only_by_slug() {
    let lines: Vec<String> = [
        "see the introduction below",
        "",
        "## Introduction",
        "### Next Steps",
        "#not a heading",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(heading_row(&lines, "Introduction"), Some(2));
    assert_eq!(heading_row(&lines, "introduction"), Some(2));
    assert_eq!(heading_row(&lines, "next-steps"), Some(3));
    assert_eq!(heading_row(&lines, " Next Steps "), Some(3));
    assert_eq!(heading_row(&lines, "not a heading"), None);
    assert_eq!(heading_row(&lines, "missing"), None);
    assert_eq!(heading_row(&lines, ""), None);
}

#[test]
fn an_empty_heading_is_no_heading() {
    let w = parse_wikilink_at("see [[b#]] x", 6).unwrap();
    assert_eq!(w.target, "b");
    assert_eq!(w.heading, None);
}

#[test]
fn md_url_splits_fragment_and_decodes_percent_escapes() {
    assert_eq!(
        split_md_url("spec.md#goals"),
        ("spec.md".into(), Some("goals".into()))
    );
    assert_eq!(split_md_url("spec.md#"), ("spec.md".into(), None));
    assert_eq!(split_md_url("#goals"), ("".into(), Some("goals".into())));
    assert_eq!(split_md_url("my%20note.md"), ("my note.md".into(), None));
    assert_eq!(percent_decode("caf%C3%A9.md"), "café.md");
    // malformed escapes stay literal
    assert_eq!(percent_decode("100%.md"), "100%.md");
    assert_eq!(percent_decode("a%zzb"), "a%zzb");
}

#[test]
fn remote_urls_are_any_scheme_or_mailto() {
    for url in [
        "https://x",
        "http://x",
        "ftp://x",
        "file:///x",
        "MAILTO:a@b",
        "mailto:a@b",
    ] {
        assert!(is_remote_url(url), "{url}");
    }
    for url in ["notes/a.md", "/a.md", "../a.md", "#h", "a:b.md"] {
        assert!(!is_remote_url(url), "{url}");
    }
}
