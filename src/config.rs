use std::path::PathBuf;
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
    /// Color theme name: `default`, `light`, or `mono` (unrecognized
    /// names fall back to `default` at lookup time, not here).
    pub theme_name: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            tree_width: 30,
            side_margin_percent: 5,
            top_margin_percent: 3,
            autosave_seconds: 2,
            tree_refresh_seconds: 2,
            theme_name: "default".to_string(),
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
            | "tree_refresh_seconds" => {
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
            _ => warnings.push(format!("line {n}: unknown option: {key}")),
        }
    }
    (cfg, warnings)
}

/// `^[a-z][a-z0-9_-]{0,31}$`, hand-checked (no regex dependency).
fn valid_theme_name(name: &str) -> bool {
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

/// The config file path: `$XDG_CONFIG_HOME/mrkdup/config`, falling back
/// to `~/.config/mrkdup/config`.
fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("mrkdup").join("config"))
}

/// Load the config file. Absent or unreadable file = all defaults; this
/// never errors.
pub fn load() -> (Config, Vec<String>) {
    match config_path().map(std::fs::read_to_string) {
        Some(Ok(text)) => parse(&text),
        _ => (Config::default(), Vec::new()),
    }
}

#[cfg(test)]
mod tests;
