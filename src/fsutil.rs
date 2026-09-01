use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

/// Write via a temp file in the same directory + rename, so a crash
/// mid-write can never truncate the destination.
pub fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let tmp = dir.join(format!(".{}.mrkdup-tmp", name.to_string_lossy()));
    {
        let mut f = match File::create(&tmp) {
            Ok(f) => f,
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                return Err(e);
            }
        };
        if let Err(e) = f.write_all(contents) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        if let Err(e) = f.sync_all() {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    }
    match std::fs::rename(&tmp, path) {
        Ok(_) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

// Test-only call counter so tests can observe whether a sniff actually
// happened (vs. being served from a cache) without touching real I/O
// timing. Thread-local: libtest reuses worker threads across tests but
// never runs two test bodies on one thread at once, so a before/after
// delta taken within a single test is never perturbed by other tests.
#[cfg(test)]
thread_local! {
    static SNIFF_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn sniff_call_count() -> usize {
    SNIFF_CALLS.with(|c| c.get())
}

/// A file is "text" if its first 8KB contain no NUL byte.
/// Unreadable/missing files are not text.
///
/// Anything that isn't a regular file — FIFO, socket, device, etc. — is
/// "not text" and, critically, is never `open`ed to find that out: opening
/// a FIFO for reading blocks until a writer shows up (forever, if none
/// ever does), which used to hang the tree walk and fuzzy search whenever
/// one turned up under the notes root. `fs::metadata` (which follows
/// symlinks) never blocks on a FIFO, so we consult it first and bail
/// before touching `File::open` for anything non-regular. A symlink that
/// resolves to a regular file is unaffected: `metadata` reports the
/// target's type, so it's sniffed exactly as before. A broken symlink (or
/// any other path `metadata` can't stat) falls through to "not text",
/// matching the missing-file case.
pub fn is_text_file(path: &Path) -> bool {
    #[cfg(test)]
    SNIFF_CALLS.with(|c| c.set(c.get() + 1));
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 8192];
    let Ok(n) = f.read(&mut buf) else {
        return false;
    };
    !buf[..n].contains(&0)
}

#[cfg(test)]
mod tests {
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

        let tmp_path = dir.join(".out.md.mrkdup-tmp");
        // Call atomic_write, which should fail because it can't rename over a directory
        let result = atomic_write(&out_path, b"content\n");
        assert!(
            result.is_err(),
            "atomic_write should fail when target is a directory"
        );

        // The key assertion: temp file must be cleaned up
        assert!(
            !tmp_path.exists(),
            "temp file should be deleted after atomic_write fails"
        );
    }
}
