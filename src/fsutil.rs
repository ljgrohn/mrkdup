use std::fs::File;
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
        let mut f = File::create(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// A file is "text" if its first 8KB contain no NUL byte.
/// Unreadable/missing files are not text.
pub fn is_text_file(path: &Path) -> bool {
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
}
