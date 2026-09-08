use super::*;
use std::fs;

fn tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("mrkdup-test-{name}"));
    fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn text_file_is_text() {
    assert!(is_text_file(&tmp("a.md", b"# hello\nworld\n")));
}

#[test]
fn empty_file_is_text() {
    assert!(is_text_file(&tmp("empty.txt", b"")));
}

#[test]
fn binary_file_is_not_text() {
    assert!(!is_text_file(&tmp("bin.dat", b"\x89PNG\x00\x01\x02")));
}

#[test]
fn missing_file_is_not_text() {
    assert!(!is_text_file(std::path::Path::new("/nonexistent/nope")));
}

#[test]
fn utf8_content_is_text() {
    assert!(is_text_file(&tmp("uni.md", "héllo — 你好 🎉\n".as_bytes())));
}

/// A FIFO must never be `open`ed by `is_text_file`: opening a FIFO for
/// reading blocks until a writer connects, which (absent a writer)
/// blocks forever. The meaningful assertion here is that this test
/// *returns at all* — pre-fix, it hangs. Run the RED under a timeout
/// (e.g. `timeout 30 cargo test`) rather than letting it block CI.
#[cfg(unix)]
#[test]
fn fifo_is_not_text_and_is_not_opened() {
    let dir = std::env::temp_dir().join("mrkdup-test-fifo");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let fifo = dir.join("pipe");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo must be on PATH for this test");
    assert!(status.success(), "mkfifo failed");

    assert!(!is_text_file(&fifo));
}

#[test]
fn atomic_write_creates_file() {
    let dir = std::env::temp_dir().join("mrkdup-test-aw1");
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("out.md");
    atomic_write(&p, b"content\n").unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"content\n");
}

#[test]
fn atomic_write_replaces_existing() {
    let dir = std::env::temp_dir().join("mrkdup-test-aw2");
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("out.md");
    fs::write(&p, b"old").unwrap();
    atomic_write(&p, b"new\n").unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"new\n");
}

#[test]
fn atomic_write_leaves_no_temp_file() {
    let dir = std::env::temp_dir().join("mrkdup-test-aw3");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    atomic_write(&dir.join("out.md"), b"x\n").unwrap();
    let names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["out.md"]);
}

#[test]
fn atomic_write_deletes_temp_file_on_rename_failure() {
    let dir = std::env::temp_dir().join("mrkdup-test-aw-cleanup");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let out_path = dir.join("out.md");
    // Create a directory with the target name so rename will fail
    fs::create_dir(&out_path).unwrap();

    // Call atomic_write, which should fail because it can't rename over a directory
    let result = atomic_write(&out_path, b"content\n");
    assert!(
        result.is_err(),
        "atomic_write should fail when target is a directory"
    );

    // The key assertion: no stray temp file may be left behind. Temp
    // names are unique per write, so scan for the marker instead of a
    // fixed name.
    assert!(
        !has_stray_tmps(&dir),
        "temp file should be deleted after atomic_write fails"
    );
}

/// True if the directory holds any leftover `atomic_write` temp file.
fn has_stray_tmps(dir: &std::path::Path) -> bool {
    fs::read_dir(dir).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".mrkdup-tmp")
    })
}

#[test]
fn atomic_write_leaves_no_stray_tmps_across_writes() {
    let dir = std::env::temp_dir().join("mrkdup-test-aw-unique");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("out.md");
    // Sequential writes must each use a fresh temp file and leave none
    // behind; the last write wins.
    atomic_write(&p, b"one\n").unwrap();
    atomic_write(&p, b"two\n").unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"two\n");
    assert!(
        !has_stray_tmps(&dir),
        "no temp files may remain after successful writes"
    );
}

#[test]
fn atomic_write_concurrent_writes_to_same_path() {
    let dir = std::env::temp_dir().join("mrkdup-test-aw-concurrent");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("out.md");
    fs::write(&p, b"base\n").unwrap();

    // With the old fixed temp name these threads raced on one temp file;
    // unique names let every write land without error.
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let p = p.clone();
            std::thread::spawn(move || {
                atomic_write(&p, format!("writer {i}\n").as_bytes()).unwrap();
            })
        })
        .collect();
    for h in handles {
        h.join().expect("writer thread panicked");
    }

    let body = fs::read_to_string(&p).unwrap();
    assert!(
        (0..8).any(|i| body == format!("writer {i}\n")),
        "destination must hold exactly one complete write, got {body:?}"
    );
    assert!(
        !has_stray_tmps(&dir),
        "no temp files may remain after concurrent writes"
    );
}
