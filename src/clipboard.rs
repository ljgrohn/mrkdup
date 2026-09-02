//! System clipboard via the OSC 52 escape sequence. Once the app owns the
//! mouse (see `main.rs`), the terminal no longer copies a drag selection
//! itself, so the app has to hand the selected text over. OSC 52 is the
//! one channel that works over SSH and in tmux without a helper binary;
//! most modern terminals (iTerm2, kitty, WezTerm, Ghostty, Alacritty,
//! foot) honour it. Terminal.app does not.

use std::io::{self, Write};

/// The full `ESC ] 52 ; c ; <base64> BEL` sequence for `text`.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Write `text` to the terminal clipboard through `out` and flush.
pub fn copy<W: Write>(out: &mut W, text: &str) -> io::Result<()> {
    out.write_all(osc52(text).as_bytes())?;
    out.flush()
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
    fn copy_writes_the_sequence_to_the_sink() {
        let mut sink: Vec<u8> = Vec::new();
        copy(&mut sink, "hi").unwrap();
        assert_eq!(sink, b"\x1b]52;c;aGk=\x07");
    }
}
