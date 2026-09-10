use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-process counter mixed into temp-file names so concurrent writes
/// to the same path never share a temp file.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_tmp_path(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(
        ".{}.mrkdup-tmp.{}.{}",
        name.to_string_lossy(),
        std::process::id(),
        n
    ))
}

/// Persist a directory entry change (the rename below) so it survives a
/// crash on filesystems that only make renames durable once the parent
/// directory is synced.
fn fsync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(dir)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        // No stable way to fsync a directory on this platform; the file
        // data itself was already synced before the rename.
        let _ = dir;
        Ok(())
    }
}

/// Write via a temp file in the same directory + rename, so a crash
/// mid-write can never truncate the destination.
///
/// The temp name is unique per write (pid + counter) and created with
/// `create_new`, so concurrent saves of the same path don't clobber each
/// other's temp file. The temp file is removed on every failure path
/// where it can still exist (after a successful rename there is nothing
/// left to clean up).
pub fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    // `create_new` fails rather than truncating if the name is taken, so
    // retry with a fresh counter value on the (essentially impossible)
    // collision instead of ever touching a file we didn't create.
    let mut attempts = 0;
    let (tmp, mut f) = loop {
        let tmp = unique_tmp_path(dir, name);
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => break (tmp, f),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempts < 10 => {
                attempts += 1;
            }
            Err(e) => return Err(e),
        }
    };
    if let Err(e) = f.write_all(contents).and_then(|()| f.sync_all()) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    drop(f);
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fsync_dir(dir)
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

/// The filesystem's own spelling of `path`: symlinks followed, case as
/// stored on disk. This is the one spelling tabs are keyed by:
/// `App::open_file` runs every path it is handed through here, whatever
/// entry point produced it, so `[[Note]]` on a case-insensitive disk
/// and a symlinked `alias.md` opened from the tree both land on the
/// tab that already has that file open instead of a second buffer of
/// one inode, racing on autosave. Anything that compares a path
/// against a tab's stored `editor.path` must therefore canonicalize
/// its own side too (`files::redirect`, `ui::open_marker_index`).
/// Falls back to `path` when the lookup fails (a missing file: the
/// open then reports its own error).
pub(crate) fn canonical(path: std::path::PathBuf) -> std::path::PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

#[cfg(test)]
mod tests;
