use super::*;
use std::fs;
use std::path::PathBuf;

/// tmp fixture:
///   root/
///     b-dir/inner.md
///     a.md
///     zz.md
///     bin.dat        (binary, hidden from tree)
///     .hidden.md     (dotfile)
/// Nested one level inside `mrkdup-tree-{tag}` (a directory this test
/// owns) rather than returning that directory itself: `ascend`/`-`
/// reroots the tree at the fixture's *parent*, and if the fixture
/// root were the top-level temp dir entry, that parent would be the
/// real shared system temp dir. Walking that for real hangs on
/// GitHub's Ubuntu/macOS runners, which leave FIFOs sitting in temp
/// (see the `is_text_file` fix) -- so every reroot/ascend test needs
/// its fixture's parent to still be something this test created and
/// controls.
fn fixture(tag: &str) -> PathBuf {
    let owned = std::env::temp_dir().join(format!("mrkdup-tree-{tag}"));
    let _ = fs::remove_dir_all(&owned);
    let root = owned.join("root");
    fs::create_dir_all(root.join("b-dir")).unwrap();
    fs::write(root.join("b-dir/inner.md"), "x\n").unwrap();
    fs::write(root.join("a.md"), "a\n").unwrap();
    fs::write(root.join("zz.md"), "z\n").unwrap();
    fs::write(root.join("bin.dat"), b"\x00\x01").unwrap();
    fs::write(root.join(".hidden.md"), "h\n").unwrap();
    root
}

fn names(t: &Tree) -> Vec<String> {
    t.rows().iter().map(|r| r.name.clone()).collect()
}

#[test]
fn lists_dirs_first_then_files_sorted_no_binary_no_hidden() {
    let t = Tree::new(fixture("list")).unwrap();
    assert_eq!(names(&t), vec!["b-dir", "a.md", "zz.md"]);
}

/// A FIFO in the tree is listed like the existing binary-file case
/// (`lists_dirs_first_then_files_sorted_no_binary_no_hidden` above):
/// `is_text_file` reports it as non-text, so `list_dir` omits it —
/// same semantics as `bin.dat`, just via a different `is_text_file`
/// path (metadata type check vs. NUL-byte sniff). The real point of
/// this test is that building the tree doesn't hang: opening a FIFO
/// for reading blocks forever absent a writer, and pre-fix this walk
/// would never return.
#[cfg(unix)]
#[test]
fn dir_with_fifo_lists_fine_and_does_not_hang() {
    let root = std::env::temp_dir().join("mrkdup-tree-fifo");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "a\n").unwrap();
    let fifo = root.join("pipe");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo must be on PATH for this test");
    assert!(status.success(), "mkfifo failed");

    let t = Tree::new(root).unwrap();
    assert_eq!(names(&t), vec!["a.md"]);
}

#[test]
fn expand_inserts_children_below() {
    let mut t = Tree::new(fixture("expand")).unwrap();
    t.expand(); // selection starts at 0 = b-dir
    assert_eq!(names(&t), vec!["b-dir", "inner.md", "a.md", "zz.md"]);
    assert_eq!(t.rows()[1].depth, 1);
}

#[test]
fn collapse_removes_children() {
    let mut t = Tree::new(fixture("collapse")).unwrap();
    t.expand();
    t.collapse();
    assert_eq!(names(&t), vec!["b-dir", "a.md", "zz.md"]);
}

#[test]
fn collapse_on_file_selects_parent_dir() {
    let mut t = Tree::new(fixture("parent")).unwrap();
    t.expand();
    t.move_down(); // inner.md
    t.collapse();
    assert_eq!(t.selected_row().unwrap().name, "b-dir");
}

#[test]
fn movement_clamps_at_edges() {
    let mut t = Tree::new(fixture("mv")).unwrap();
    t.move_up();
    assert_eq!(t.selected(), 0);
    t.move_bottom();
    assert_eq!(t.selected_row().unwrap().name, "zz.md");
    t.move_down();
    assert_eq!(t.selected_row().unwrap().name, "zz.md");
    t.move_top();
    assert_eq!(t.selected(), 0);
}

#[test]
fn toggle_hidden_shows_dotfiles() {
    let mut t = Tree::new(fixture("hidden")).unwrap();
    assert!(!t.show_hidden());
    t.toggle_hidden();
    assert!(t.show_hidden());
    assert!(names(&t).contains(&".hidden.md".to_string()));
    t.toggle_hidden();
    assert!(!names(&t).contains(&".hidden.md".to_string()));
}

#[test]
fn refresh_preserves_expansion_and_sees_new_files() {
    let root = fixture("refresh");
    let mut t = Tree::new(root.clone()).unwrap();
    t.expand();
    fs::write(root.join("b-dir/new.md"), "n\n").unwrap();
    t.refresh();
    assert_eq!(
        names(&t),
        vec!["b-dir", "inner.md", "new.md", "a.md", "zz.md"]
    );
}

#[test]
fn ascend_reroots_at_parent_keeping_place() {
    let root = fixture("asc");
    let mut t = Tree::new(root.clone()).unwrap();
    t.expand(); // expand b-dir so we can check state survives
    t.ascend();
    let canon = root.canonicalize().unwrap();
    // old root is now a visible, expanded, selected row...
    let sel = t.selected_row().unwrap();
    assert_eq!(sel.path, canon);
    assert!(sel.is_dir && sel.expanded);
    // ...its children (and the previously expanded subdir's) still show
    assert!(t.rows().iter().any(|r| r.path == canon.join("a.md")));
    assert!(t
        .rows()
        .iter()
        .any(|r| r.path == canon.join("b-dir/inner.md")));
    // and the tree is rooted one level up now
    assert_eq!(t.root(), canon.parent().unwrap());
}

#[test]
fn make_root_on_dir_reroots_there() {
    let root = fixture("mkroot");
    let mut t = Tree::new(root.clone()).unwrap();
    t.make_root(); // selection starts on b-dir
    assert_eq!(t.root(), root.canonicalize().unwrap().join("b-dir"));
    assert_eq!(names(&t), vec!["inner.md"]);
    assert_eq!(t.selected(), 0);
}

#[test]
fn make_root_on_file_reroots_at_its_parent() {
    let root = fixture("mkroot-file");
    let mut t = Tree::new(root.clone()).unwrap();
    t.expand();
    t.move_down(); // inner.md
    t.make_root();
    assert_eq!(t.root(), root.canonicalize().unwrap().join("b-dir"));
    assert_eq!(names(&t), vec!["inner.md"]);
}

#[test]
fn ascend_at_filesystem_root_is_noop() {
    let mut t = Tree::new(PathBuf::from("/")).unwrap();
    let before = t.root().to_path_buf();
    t.ascend();
    assert_eq!(t.root(), before);
}

#[test]
fn select_path_selects_the_matching_row() {
    let mut t = Tree::new(fixture("selpath")).unwrap();
    let target = t.root().join("zz.md");
    assert!(t.select_path(&target));
    assert_eq!(t.selected_row().unwrap().name, "zz.md");
    // a missing path leaves the selection where it was
    assert!(!t.select_path(Path::new("/nonexistent/nope.md")));
    assert_eq!(t.selected_row().unwrap().name, "zz.md");
}

#[test]
fn expand_honors_root_gitignore_for_nested_files() {
    let root = std::env::temp_dir().join("mrkdup-tree-rootignore");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.join("notes/debug.log"), "d\n").unwrap();
    fs::write(root.join("notes/keep.md"), "k\n").unwrap();

    let mut t = Tree::new(root).unwrap();
    t.expand(); // selection starts at 0 = notes
    let rows = names(&t);
    assert!(rows.contains(&"keep.md".to_string()));
    assert!(!rows.contains(&"debug.log".to_string()));
}

#[test]
fn toggle_hidden_also_shows_gitignored_files() {
    let root = std::env::temp_dir().join("mrkdup-tree-showignored");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.join("notes/debug.log"), "d\n").unwrap();
    fs::write(root.join("notes/.gitignore"), "local.md\n").unwrap();
    fs::write(root.join("notes/local.md"), "l\n").unwrap();
    fs::write(root.join("notes/keep.md"), "k\n").unwrap();

    let mut t = Tree::new(root).unwrap();
    t.expand(); // selection starts at 0 = notes
    assert!(!names(&t).contains(&"debug.log".to_string()));
    assert!(!names(&t).contains(&"local.md".to_string()));

    t.toggle_hidden();
    let rows = names(&t);
    assert!(rows.contains(&"keep.md".to_string()));
    assert!(
        rows.contains(&"debug.log".to_string()),
        "root .gitignore rule"
    );
    assert!(
        rows.contains(&"local.md".to_string()),
        "nested .gitignore rule"
    );
    assert!(
        rows.contains(&".gitignore".to_string()),
        "dotfiles still shown"
    );

    t.toggle_hidden();
    assert!(!names(&t).contains(&"debug.log".to_string()));
    assert!(!names(&t).contains(&"local.md".to_string()));
}

#[test]
fn refresh_without_changes_does_not_resniff_cached_files() {
    let root = fixture("cache-hit");
    let mut t = Tree::new(root).unwrap();
    // First build already sniffed a.md, zz.md, and bin.dat (b-dir
    // isn't expanded, so its contents were never walked).
    let before = crate::fsutil::sniff_call_count();
    t.refresh();
    let after = crate::fsutil::sniff_call_count();
    assert_eq!(
        after, before,
        "unchanged files should be served from the cache, not re-sniffed"
    );
}

#[test]
fn mtime_bump_forces_a_resniff() {
    let root = fixture("cache-bump");
    let mut t = Tree::new(root.clone()).unwrap();
    // Rewrite a.md's contents without changing its name, forcing a
    // new mtime. 20ms is plenty of margin on the sub-millisecond
    // mtime resolution of APFS/ext4-class filesystems, which is what
    // this test (and CI) actually runs on — it is not a guarantee on
    // coarser-resolution filesystems (FAT32/exFAT ~2s ticks, some
    // network mounts), where this same tick could in principle still
    // land within the old mtime. See the staleness-window note on
    // `Tree::text_cache` for that known limit of the cache itself.
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(root.join("a.md"), "changed\n").unwrap();

    let before = crate::fsutil::sniff_call_count();
    t.refresh();
    let after = crate::fsutil::sniff_call_count();
    assert!(
        after > before,
        "a changed mtime should invalidate the cache entry and trigger a re-sniff"
    );
}

#[test]
fn deleted_file_is_evicted_from_cache() {
    let root = fixture("evict");
    let mut t = Tree::new(root.clone()).unwrap();
    // a.md, zz.md, bin.dat were sniffed and cached by the initial build.
    let before = t.text_cache_len();

    fs::remove_file(root.join("a.md")).unwrap();
    t.refresh();

    assert!(
        t.text_cache_len() < before,
        "a deleted file's cache entry should be dropped, not retained forever"
    );
    assert!(!names(&t).contains(&"a.md".to_string()));
}

#[test]
fn empty_directory_yields_no_rows_and_no_panic() {
    let root = std::env::temp_dir().join("mrkdup-tree-empty");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let mut t = Tree::new(root).unwrap();
    assert!(t.rows().is_empty());
    assert!(t.selected_row().is_none());
    t.move_down();
    t.expand();
    t.collapse();
}

#[test]
fn select_takes_an_index_and_rejects_out_of_range() {
    let root = std::env::temp_dir().join("mrkdup-tree-select");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "").unwrap();
    fs::write(root.join("b.md"), "").unwrap();
    let mut t = Tree::new(root).unwrap();
    assert!(t.select(1));
    assert_eq!(t.selected(), 1);
    assert!(!t.select(2));
    assert_eq!(t.selected(), 1);
}
