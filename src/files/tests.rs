use super::*;
use crate::editor::Editor;
use std::fs;
use std::path::PathBuf;

/// A temp dir with an `a.md` in it, canonicalized so it matches
/// what `Tree::new` (and thus `tree.root()`) returns.
fn fixture(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("mrkdup-files-{tag}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "hello\n").unwrap();
    root.canonicalize().unwrap()
}

#[test]
fn create_writes_an_empty_file_at_root() {
    let root = fixture("create");
    let mut tree = Tree::new(root.clone()).unwrap();
    let path = create(&mut tree, &root, "new.md").unwrap();
    assert_eq!(path, root.join("new.md"));
    assert_eq!(fs::read(&path).unwrap(), b"");
}

#[test]
fn create_rejects_invalid_names() {
    let root = fixture("create-invalid");
    let mut tree = Tree::new(root.clone()).unwrap();
    for name in ["", "..", "/etc/passwd", "docs/../x.md"] {
        assert!(create(&mut tree, &root, name).is_err(), "accepted {name:?}");
    }
}

#[test]
fn create_rejects_existing_file() {
    let root = fixture("create-exists");
    let mut tree = Tree::new(root.clone()).unwrap();
    assert!(create(&mut tree, &root, "a.md").is_err());
}

#[test]
fn create_ignores_the_tree_selection() {
    let root = fixture("create-base");
    fs::create_dir_all(root.join("sub")).unwrap();
    let mut tree = Tree::new(root.clone()).unwrap();
    // park the selection somewhere unrelated: the file must still land
    // in `base`, not alongside the selection (the link-follow case)
    assert!(tree.select_path(&root.join("sub")));
    let path = create(&mut tree, &root, "notes/new.md").unwrap();
    assert_eq!(path, root.join("notes/new.md"));
    assert!(path.is_file());
    for name in ["", "/x.md", "../x.md"] {
        assert!(create(&mut tree, &root, name).is_err(), "accepted {name:?}");
    }
}

#[test]
fn selected_dir_is_the_selected_dir_or_the_selected_files_parent() {
    let root = fixture("selected-dir");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub/x.md"), "x\n").unwrap();
    let mut tree = Tree::new(root.clone()).unwrap();
    assert!(tree.select_path(&root.join("sub")));
    assert_eq!(selected_dir(&tree), root.join("sub"));
    assert!(tree.select_path(&root.join("a.md")));
    assert_eq!(selected_dir(&tree), root);
}

#[test]
fn rename_moves_the_file_and_redirects_the_open_editor() {
    let root = fixture("rename");
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    editor.path = Some(root.join("a.md"));
    let mut tabs = vec![Tab::new(editor)];
    let status = rename(&mut tree, &mut tabs, &root.join("a.md"), "z.md")
        .unwrap()
        .unwrap();
    assert_eq!(status, "renamed to z.md");
    assert!(root.join("z.md").exists());
    assert!(!root.join("a.md").exists());
    assert_eq!(
        tabs[0].editor.path.as_deref(),
        Some(root.join("z.md").as_path())
    );
}

#[test]
fn rename_to_same_name_is_a_no_op() {
    let root = fixture("rename-same");
    let mut tree = Tree::new(root.clone()).unwrap();
    assert_eq!(
        rename(&mut tree, &mut [], &root.join("a.md"), "a.md").unwrap(),
        None
    );
    assert!(root.join("a.md").exists());
}

#[test]
fn rename_rejects_invalid_names() {
    let root = fixture("rename-invalid");
    let mut tree = Tree::new(root.clone()).unwrap();
    for name in ["docs/x.md", "..", ""] {
        assert!(rename(&mut tree, &mut [], &root.join("a.md"), name).is_err());
    }
}

#[test]
fn move_to_relocates_the_file_and_redirects_the_open_editor() {
    let root = fixture("move");
    fs::create_dir_all(root.join("docs")).unwrap();
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    editor.path = Some(root.join("a.md"));
    let mut tabs = vec![Tab::new(editor)];
    let status = move_to(&mut tree, &mut tabs, &root.join("a.md"), &root.join("docs"))
        .unwrap()
        .unwrap();
    assert_eq!(status, "moved to docs/a.md");
    assert!(root.join("docs/a.md").exists());
    assert_eq!(
        tabs[0].editor.path.as_deref(),
        Some(root.join("docs/a.md").as_path())
    );
}

#[test]
fn move_to_same_directory_is_rejected() {
    let root = fixture("move-same");
    let mut tree = Tree::new(root.clone()).unwrap();
    assert!(move_to(&mut tree, &mut [], &root.join("a.md"), &root).is_err());
    assert!(root.join("a.md").exists());
}

#[test]
fn delete_removes_the_file() {
    let root = fixture("delete");
    let mut tree = Tree::new(root.clone()).unwrap();
    let status = delete(&mut tree, &root.join("a.md")).unwrap();
    assert_eq!(status, "deleted a.md");
    assert!(!root.join("a.md").exists());
    assert!(tree.rows().iter().all(|r| r.name != "a.md"));
}

#[test]
fn redirect_only_touches_tabs_on_the_old_path() {
    let mut a = Editor::new();
    a.path = Some(PathBuf::from("/x/a.md"));
    let mut b = Editor::new();
    b.path = Some(PathBuf::from("/x/b.md"));
    let mut tabs = vec![Tab::new(a), Tab::new(b)];
    redirect(&mut tabs, Path::new("/x/a.md"), Path::new("/y/a.md"));
    assert_eq!(tabs[0].editor.path.as_deref(), Some(Path::new("/y/a.md")));
    assert_eq!(tabs[1].editor.path.as_deref(), Some(Path::new("/x/b.md")));
}

/// Tab paths are canonical, so a file opened through a symlink is
/// stored under the target's spelling: `redirect` must still find it
/// when the target is renamed, and must leave it alone (still pointing
/// at live content) when only the link is renamed.
#[test]
#[cfg(unix)]
fn rename_redirects_a_tab_opened_through_a_symlink() {
    let root = fixture("rename-symlink");
    std::os::unix::fs::symlink(root.join("a.md"), root.join("alias.md")).unwrap();
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    // what `App::open_file` stores for `alias.md`
    editor.path = Some(crate::fsutil::canonical(root.join("alias.md")));
    assert_eq!(editor.path.as_deref(), Some(root.join("a.md").as_path()));
    let mut tabs = vec![Tab::new(editor)];

    // renaming the link moves the link, not the open file: the tab
    // keeps pointing at content that is still there
    rename(&mut tree, &mut tabs, &root.join("alias.md"), "alias2.md")
        .unwrap()
        .unwrap();
    assert_eq!(
        tabs[0].editor.path.as_deref(),
        Some(root.join("a.md").as_path())
    );
    assert!(tabs[0].editor.path.as_deref().unwrap().is_file());

    // renaming the file the tab actually has open redirects it
    rename(&mut tree, &mut tabs, &root.join("a.md"), "z.md")
        .unwrap()
        .unwrap();
    assert_eq!(
        tabs[0].editor.path.as_deref(),
        Some(root.join("z.md").as_path())
    );
}

/// Moving a symlink moves the link and leaves its target alone, so a
/// tab holding the real file must not be repointed at the moved link —
/// with a relative link that path dangles, and the tab would be off
/// live content entirely.
#[test]
#[cfg(unix)]
fn move_to_a_symlink_leaves_the_tab_on_its_target() {
    let root = fixture("move-symlink");
    fs::create_dir_all(root.join("sub")).unwrap();
    // relative, so the link dangles once moved into `sub`
    std::os::unix::fs::symlink("a.md", root.join("alias.md")).unwrap();
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    // what `App::open_file` stores for either spelling
    editor.path = Some(crate::fsutil::canonical(root.join("alias.md")));
    assert_eq!(editor.path.as_deref(), Some(root.join("a.md").as_path()));
    let mut tabs = vec![Tab::new(editor)];

    move_to(
        &mut tree,
        &mut tabs,
        &root.join("alias.md"),
        &root.join("sub"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        tabs[0].editor.path.as_deref(),
        Some(root.join("a.md").as_path()),
        "moving the link must not repoint the tab holding its target"
    );
    assert!(root.join("a.md").is_file(), "the real file did not move");
}
