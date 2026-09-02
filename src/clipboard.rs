//! System clipboard for a mouse selection. Once the app owns the mouse
//! (see `main.rs`), the terminal no longer copies a drag selection
//! itself, so the app hands the text over two ways, best effort:
//!
//! - the OSC 52 escape sequence, which works over SSH and inside tmux
//!   (with `set-clipboard on`) in most modern terminals — iTerm2, kitty,
//!   WezTerm, Ghostty, Alacritty, foot. Terminal.app ignores it.
//! - a local clipboard command when one exists: `pbcopy` on macOS,
//!   `wl-copy` under Wayland, `xclip` then `xsel` under X11. Covers
//!   Terminal.app and anything else that doesn't speak OSC 52.

use std::io::{self, Write};
use std::process::{Command, Stdio};

/// The full `ESC ] 52 ; c ; <base64> BEL` sequence for `text`.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Write `text` to the terminal clipboard through `out` (OSC 52) and
/// flush.
pub fn write_osc52<W: Write>(out: &mut W, text: &str) -> io::Result<()> {
    out.write_all(osc52(text).as_bytes())?;
    out.flush()
}

/// Both channels: OSC 52 through `out`, then the first local clipboard
/// command that starts. Only the write to `out` can fail; a missing or
/// failing command is silently skipped.
pub fn copy<W: Write>(out: &mut W, text: &str) -> io::Result<()> {
    write_osc52(out, text)?;
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
    for (cmd, args) in system_commands(std::env::consts::OS, wayland) {
        if pipe_to(cmd, args, text).is_ok() {
            break;
        }
    }
    Ok(())
}

/// The clipboard commands to try, in order, for `os` (Rust's
/// `std::env::consts::OS` names) and whether a Wayland display is set.
pub(crate) fn system_commands(
    os: &str,
    wayland: bool,
) -> Vec<(&'static str, &'static [&'static str])> {
    match os {
        "macos" => vec![("pbcopy", &[])],
        "windows" => vec![("clip", &[])],
        _ if wayland => vec![("wl-copy", &[])],
        _ => vec![
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ],
    }
}

/// Run `cmd args` with `text` on its stdin. `Err` if it couldn't be
/// started (not installed) or exited non-zero.
fn pipe_to(cmd: &str, args: &[&str], text: &str) -> io::Result<()> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{cmd} exited with {status}")))
    }
}

/// Standard base64 with `=` padding — small enough to hand-roll rather
/// than pull in a crate for one escape sequence.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let v = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(TABLE[(v >> 18) as usize & 63] as char);
        out.push(TABLE[(v >> 12) as usize & 63] as char);
        out.push(if n > 1 {
            TABLE[(v >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if n > 2 {
            TABLE[v as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn osc52_wraps_the_payload() {
        assert_eq!(osc52("hello"), "\x1b]52;c;aGVsbG8=\x07");
    }

    #[test]
    fn write_osc52_writes_the_sequence_to_the_sink() {
        let mut sink: Vec<u8> = Vec::new();
        write_osc52(&mut sink, "hi").unwrap();
        assert_eq!(sink, b"\x1b]52;c;aGk=\x07");
    }

    #[test]
    fn system_commands_per_platform() {
        assert_eq!(system_commands("macos", false), [("pbcopy", &[][..])]);
        assert_eq!(system_commands("linux", true), [("wl-copy", &[][..])]);
        let x11 = system_commands("linux", false);
        assert_eq!(x11[0].0, "xclip");
        assert_eq!(x11[1].0, "xsel");
        assert_eq!(system_commands("windows", false)[0].0, "clip");
    }

    #[test]
    fn a_missing_command_is_an_error_not_a_panic() {
        assert!(pipe_to("mrkdup-no-such-clipboard-tool", &[], "x").is_err());
    }
}
