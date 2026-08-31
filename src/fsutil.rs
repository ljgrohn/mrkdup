use std::fs::File;
use std::io::Read;
use std::path::Path;

/// A file is "text" if its first 8KB contain no NUL byte.
/// Unreadable/missing files are not text.
pub fn is_text_file(path: &Path) -> bool {
    let Ok(mut f) = File::open(path) else { return false };
    let mut buf = [0u8; 8192];
    let Ok(n) = f.read(&mut buf) else { return false };
    !buf[..n].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("markdup-test-{name}"));
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
}
