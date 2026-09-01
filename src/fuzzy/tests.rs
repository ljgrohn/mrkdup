use super::*;
use std::fs;

#[test]
fn fuzzy_score_matches_subsequences_and_rejects_non_matches() {
    assert!(fuzzy_score("anm", "app/notes.md").is_some());
    assert!(fuzzy_score("zzz", "app/notes.md").is_none());
    // subsequence order matters
    assert!(fuzzy_score("mn", "notes.md").is_none());
    // empty query matches everything
    assert!(fuzzy_score("", "notes.md").is_some());
}

#[test]
fn fuzzy_score_is_case_insensitive() {
    assert!(fuzzy_score("RM", "readme.md").is_some());
    assert_eq!(
        fuzzy_score("RM", "readme.md"),
        fuzzy_score("rm", "README.md")
    );
}

#[test]
fn fuzzy_score_ranks_consecutive_runs_first() {
    let tight = fuzzy_score("abc", "abc.md").unwrap();
    let scattered = fuzzy_score("abc", "a1b2c.md").unwrap();
    assert!(tight < scattered);
}

#[test]
fn fuzzy_score_prefers_earlier_matches_when_runs_tie() {
    let early = fuzzy_score("ab", "ab_xxx.md").unwrap();
    let late = fuzzy_score("ab", "xxx_ab.md").unwrap();
    assert!(early < late);
}

#[test]
fn fuzzy_score_breaks_remaining_ties_by_shorter_path() {
    let short = fuzzy_score("ab", "ab.md").unwrap();
    let long = fuzzy_score("ab", "ab-longer.md").unwrap();
    assert!(short < long);
}

#[test]
fn collect_candidates_honors_root_gitignore_for_nested_files() {
    let root = std::env::temp_dir().join("mrkdup-app-rootignore");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.join("notes/debug.log"), "d\n").unwrap();
    fs::write(root.join("notes/keep.md"), "k\n").unwrap();

    let candidates = collect_candidates(&root, false);
    let rels: Vec<&str> = candidates.iter().map(|(rel, _)| rel.as_str()).collect();
    assert!(rels.contains(&"notes/keep.md"));
    assert!(!rels.contains(&"notes/debug.log"));
}
