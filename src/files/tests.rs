use super::*;
use std::fs;

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
    let path = create(&mut tree, "new.md").unwrap();
    assert_eq!(path, root.join("new.md"));
    assert_eq!(fs::read(&path).unwrap(), b"");
}

#[test]
fn create_rejects_invalid_names() {
    let root = fixture("create-invalid");
    let mut tree = Tree::new(root).unwrap();
    for name in ["", "..", "/etc/passwd", "docs/../x.md"] {
        assert!(create(&mut tree, name).is_err(), "accepted {name:?}");
    }
}

#[test]
fn create_rejects_existing_file() {
    let root = fixture("create-exists");
    let mut tree = Tree::new(root).unwrap();
    assert!(create(&mut tree, "a.md").is_err());
}

#[test]
fn rename_moves_the_file_and_redirects_the_open_editor() {
    let root = fixture("rename");
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    editor.path = Some(root.join("a.md"));
    let status = rename(&mut tree, &mut editor, &root.join("a.md"), "z.md")
        .unwrap()
        .unwrap();
    assert_eq!(status, "renamed to z.md");
    assert!(root.join("z.md").exists());
    assert!(!root.join("a.md").exists());
    assert_eq!(editor.path.as_deref(), Some(root.join("z.md").as_path()));
}

#[test]
fn rename_to_same_name_is_a_no_op() {
    let root = fixture("rename-same");
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    assert_eq!(
        rename(&mut tree, &mut editor, &root.join("a.md"), "a.md").unwrap(),
        None
    );
    assert!(root.join("a.md").exists());
}

#[test]
fn rename_rejects_invalid_names() {
    let root = fixture("rename-invalid");
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    for name in ["docs/x.md", "..", ""] {
        assert!(rename(&mut tree, &mut editor, &root.join("a.md"), name).is_err());
    }
}

#[test]
fn move_to_relocates_the_file_and_redirects_the_open_editor() {
    let root = fixture("move");
    fs::create_dir_all(root.join("docs")).unwrap();
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    editor.path = Some(root.join("a.md"));
    let status = move_to(
        &mut tree,
        &mut editor,
        &root.join("a.md"),
        &root.join("docs"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(status, "moved to docs/a.md");
    assert!(root.join("docs/a.md").exists());
    assert_eq!(
        editor.path.as_deref(),
        Some(root.join("docs/a.md").as_path())
    );
}

#[test]
fn move_to_same_directory_is_rejected() {
    let root = fixture("move-same");
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    assert!(move_to(&mut tree, &mut editor, &root.join("a.md"), &root).is_err());
    assert!(root.join("a.md").exists());
}

#[test]
fn delete_removes_the_file_and_resets_a_matching_editor() {
    let root = fixture("delete");
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    editor.path = Some(root.join("a.md"));
    let status = delete(&mut tree, &mut editor, &root.join("a.md")).unwrap();
    assert_eq!(status, "deleted a.md");
    assert!(!root.join("a.md").exists());
    assert!(editor.path.is_none());
}

#[test]
fn delete_leaves_an_unrelated_open_editor_alone() {
    let root = fixture("delete-unrelated");
    fs::write(root.join("b.md"), "bee\n").unwrap();
    let mut tree = Tree::new(root.clone()).unwrap();
    let mut editor = Editor::new();
    editor.path = Some(root.join("b.md"));
    delete(&mut tree, &mut editor, &root.join("a.md")).unwrap();
    assert_eq!(editor.path.as_deref(), Some(root.join("b.md").as_path()));
}
