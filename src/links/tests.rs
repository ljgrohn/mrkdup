use super::*;
use std::fs;
use std::path::Path;

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
