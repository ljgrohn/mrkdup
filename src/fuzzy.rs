//! The Ctrl+P "go to file" fuzzy finder: candidate collection, scoring,
//! and filtering, plus the root-relative display-path helper they share.

use std::path::{Path, PathBuf};

/// Renders a root-relative path as a forward-slash-separated string,
/// normalized for display on all OS. Never converts to `to_string_lossy()`
/// without joining with `/`.
pub(crate) fn rel_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// All text files under `root` as (root-relative display path, absolute
/// path), sorted by relative path, capped at 5000. Skips .git, and
/// respects .gitignore and omits dotfiles unless `show_hidden` is set
/// (the tree's one "show everything" switch).
///
/// Deliberately does not use `Tree`'s (path, mtime) -> is_text cache: this
/// walk runs once per Ctrl+P popup open (a user action), not on a ~2s
/// timer, so there's no repeated-sniff cost to amortize here. Threading
/// the cache in from `App` would mean exposing `Tree`'s private cache or
/// widening its API for a walk that already only sniffs each file once
/// per popup open.
pub(crate) fn collect_candidates(root: &Path, show_hidden: bool) -> Vec<(String, PathBuf)> {
    const CAP: usize = 5000;
    let mut out = Vec::new();
    let ignores = crate::tree::root_gitignore(root);
    let walker = ignore::WalkBuilder::new(root)
        .hidden(!show_hidden)
        .git_ignore(!show_hidden)
        .git_exclude(!show_hidden)
        .require_git(false)
        .git_global(false)
        .parents(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walker.flatten() {
        if out.len() >= CAP {
            break;
        }
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.into_path();
        if !show_hidden && crate::tree::is_root_ignored(&ignores, root, &path, false) {
            continue;
        }
        if !crate::fsutil::is_text_file(&path) {
            continue;
        }
        let rel = rel_display(root, &path);
        out.push((rel, path));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Case-insensitive subsequence match of `query` in `candidate` (greedy,
/// leftmost). `None` = no match; otherwise a sort key where smaller ranks
/// first: (query chars outside the longest consecutive matched run,
/// first match position, candidate length). An empty query matches
/// everything.
fn fuzzy_score(query: &str, candidate: &str) -> Option<(usize, usize, usize)> {
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let c: Vec<char> = candidate.to_lowercase().chars().collect();
    if q.is_empty() {
        // constant score: the stable sort keeps the alphabetical
        // candidate order for the just-opened, un-filtered popup
        return Some((0, 0, 0));
    }
    let mut positions = Vec::with_capacity(q.len());
    let mut from = 0;
    for &qc in &q {
        let found = from + c[from..].iter().position(|&cc| cc == qc)?;
        positions.push(found);
        from = found + 1;
    }
    let mut max_run = 1usize;
    let mut run = 1usize;
    for w in positions.windows(2) {
        if w[1] == w[0] + 1 {
            run += 1;
            max_run = max_run.max(run);
        } else {
            run = 1;
        }
    }
    Some((q.len() - max_run, positions[0], c.len()))
}

/// The candidates matching `query`, best first (stable: ties keep the
/// input order).
pub(crate) fn fuzzy_filter<'a>(
    query: &str,
    candidates: &'a [(String, PathBuf)],
) -> Vec<&'a (String, PathBuf)> {
    let mut scored: Vec<_> = candidates
        .iter()
        .filter_map(|c| fuzzy_score(query, &c.0).map(|s| (s, c)))
        .collect();
    scored.sort_by_key(|(s, _)| *s);
    scored.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
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
}
