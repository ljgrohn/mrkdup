use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// User configuration, read from `$XDG_CONFIG_HOME/mrkdup/config`
/// (default `~/.config/mrkdup/config`). Every field has a default; a
/// missing or unreadable file just means all defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Tree pane width in columns (10..=120).
    pub tree_width: u16,
    /// Editor breathing room: % of pane width trimmed off each side (0..=40).
    pub side_margin_percent: u16,
    /// Editor breathing room: % of pane height trimmed off the top (0..=40).
    pub top_margin_percent: u16,
    /// Idle seconds before a dirty buffer autosaves (1..=600).
    pub autosave_seconds: u64,
    /// Seconds between automatic tree refreshes (1..=600).
    pub tree_refresh_seconds: u64,
    /// Columns of empty space between the terminal edges and the panes
    /// (0..=20). Padding *outside* the borders; `side_margin_percent`
    /// is the text margin inside the editor pane.
    pub side_padding: u16,
    /// Builtin color theme (`default`, `light`, `mono`), or the name of
    /// a file under `$XDG_CONFIG_HOME/mrkdup/themes/`; validated by `valid_theme_name`.
    pub theme_name: String,
    /// Terminal cursor shape (`default` = leave the terminal's own).
    pub cursor_shape: crate::cursor::Shape,
    /// Whether the cursor blinks; only applies with an explicit `cursor_shape`.
    pub cursor_blink: bool,
    /// Cursor color: `default`, a name from `cursor::COLOR_NAMES`, or `#rrggbb`.
    pub cursor_color: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            tree_width: 30,
            side_margin_percent: 5,
            top_margin_percent: 3,
            autosave_seconds: 2,
            tree_refresh_seconds: 2,
            side_padding: 1,
            theme_name: "default".to_string(),
            cursor_shape: crate::cursor::Shape::Default,
            cursor_blink: true,
            cursor_color: "default".to_string(),
        }
    }
}

impl Config {
    pub fn autosave(&self) -> Duration {
        Duration::from_secs(self.autosave_seconds)
    }
    pub fn tree_refresh(&self) -> Duration {
        Duration::from_secs(self.tree_refresh_seconds)
    }
}

/// Parse `key = value` lines (`#` comments and blank lines skipped).
/// Never fails: returns the config plus a warning per ignored line
/// (malformed, unknown key, or non-numeric value). Out-of-range values
/// are clamped into each option's valid range rather than ignored.
pub fn parse(text: &str) -> (Config, Vec<String>) {
    let mut cfg = Config::default();
    let mut warnings = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let n = i + 1;
        let Some((key, value)) = line.split_once('=') else {
            warnings.push(format!("line {n}: expected `key = value`"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "tree_width"
            | "side_margin_percent"
            | "top_margin_percent"
            | "autosave_seconds"
            | "tree_refresh_seconds"
            | "side_padding" => {
                let Ok(v) = value.parse::<i64>() else {
                    warnings.push(format!("line {n}: {key}: not a number: {value:?}"));
                    continue;
                };
                match key {
                    "tree_width" => cfg.tree_width = v.clamp(10, 120) as u16,
                    "side_margin_percent" => cfg.side_margin_percent = v.clamp(0, 40) as u16,
                    "top_margin_percent" => cfg.top_margin_percent = v.clamp(0, 40) as u16,
                    "autosave_seconds" => cfg.autosave_seconds = v.clamp(1, 600) as u64,
                    "tree_refresh_seconds" => cfg.tree_refresh_seconds = v.clamp(1, 600) as u64,
                    "side_padding" => cfg.side_padding = v.clamp(0, 20) as u16,
                    _ => unreachable!(),
                }
            }
            "theme" => {
                if valid_theme_name(value) {
                    cfg.theme_name = value.to_string();
                } else {
                    warnings.push(format!("line {n}: theme: invalid name {value:?}"));
                }
            }
            "cursor_shape" => match crate::cursor::Shape::parse(&value.to_ascii_lowercase()) {
                Some(shape) => cfg.cursor_shape = shape,
                None => warnings.push(format!(
                    "line {n}: cursor_shape: expected default | block | bar | underline, got {value:?}"
                )),
            },
            "cursor_blink" => match parse_bool(value) {
                Some(b) => cfg.cursor_blink = b,
                None => warnings.push(format!(
                    "line {n}: cursor_blink: expected on | off, got {value:?}"
                )),
            },
            "cursor_color" => {
                let value = value.to_ascii_lowercase();
                if crate::cursor::valid_color(&value) {
                    cfg.cursor_color = value;
                } else {
                    warnings.push(format!(
                        "line {n}: cursor_color: expected default, a color name, or #rrggbb, got {value:?}"
                    ));
                }
            }
            _ => warnings.push(format!("line {n}: unknown option: {key}")),
        }
    }
    (cfg, warnings)
}

/// `on`/`off` as written by the settings popup, plus the usual
/// spellings; case-insensitive.
fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Some(true),
        "off" | "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// `^[a-z][a-z0-9_-]{0,31}$`, hand-checked (no regex dependency).
pub(crate) fn valid_theme_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    name.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// The `$XDG_CONFIG_HOME/mrkdup` directory, falling back to
/// `~/.config/mrkdup`. Shared by `config_path` here and by
/// `theme::load`.
pub(crate) fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("mrkdup"))
}

/// The config file path: `$XDG_CONFIG_HOME/mrkdup/config`, falling back
/// to `~/.config/mrkdup/config`.
fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config"))
}

/// Load the config file. Absent or unreadable file = all defaults; this
/// never errors.
pub fn load() -> (Config, Vec<String>) {
    match config_path().map(std::fs::read_to_string) {
        Some(Ok(text)) => parse(&text),
        _ => (Config::default(), Vec::new()),
    }
}

/// `text` with the first `<key> = …` line replaced by `<key> = <value>`
/// (leading indentation kept; the parser has no inline comments, so the
/// whole rest of the line is the value), or with `<key> = <value>`
/// appended when no such line exists. Commented-out lines and keys that
/// merely start with `key` (`theme_name` for `theme`) don't count. Other
/// lines are copied verbatim; the output always ends in `\n`. Pure, so
/// the settings popup's write-back is testable without disk.
pub fn rewrite_key_line(text: &str, key: &str, value: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut replaced = false;
    for line in text.lines() {
        if !replaced && is_key_line(line, key) {
            let indent = &line[..line.len() - line.trim_start().len()];
            out.push_str(indent);
            out.push_str(key);
            out.push_str(" = ");
            out.push_str(value);
            replaced = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !replaced {
        out.push_str(key);
        out.push_str(" = ");
        out.push_str(value);
        out.push('\n');
    }
    out
}

/// `key` followed by optional spaces and `=`, after any indentation.
fn is_key_line(line: &str, key: &str) -> bool {
    line.trim_start()
        .strip_prefix(key)
        .map(|rest| rest.trim_start().starts_with('='))
        .unwrap_or(false)
}

/// Persist one `key = value` in the config file at `path`: read it
/// (missing = empty), rewrite the line, write atomically. Creates the
/// parent directory if needed. Only the settings popup calls this —
/// startup never writes the config.
pub fn save_key_to(path: &Path, key: &str, value: &str) -> io::Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::fsutil::atomic_write(path, rewrite_key_line(&text, key, value).as_bytes())
}

#[cfg(test)]
mod tests;
