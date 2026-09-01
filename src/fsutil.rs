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
mod tests;
