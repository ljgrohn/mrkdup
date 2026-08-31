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
}

impl Default for Config {
    fn default() -> Config {
        Config {
            tree_width: 30,
            side_margin_percent: 5,
            top_margin_percent: 3,
            autosave_seconds: 2,
            tree_refresh_seconds: 2,
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
            _ => warnings.push(format!("line {n}: unknown option: {key}")),
        }
    }
    (cfg, warnings)
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
mod tests {
    use super::*;

    #[test]
    fn empty_text_gives_all_defaults_and_no_warnings() {
        let (cfg, warnings) = parse("");
        assert_eq!(cfg, Config::default());
        assert!(warnings.is_empty());
        assert_eq!(cfg.tree_width, 30);
        assert_eq!(cfg.side_margin_percent, 5);
        assert_eq!(cfg.top_margin_percent, 3);
        assert_eq!(cfg.autosave_seconds, 2);
        assert_eq!(cfg.tree_refresh_seconds, 2);
    }

    #[test]
    fn comments_and_blank_lines_are_skipped_silently() {
        let (cfg, warnings) = parse("# a comment\n\n   \n  # indented comment\n");
        assert_eq!(cfg, Config::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn valid_overrides_apply_with_flexible_whitespace() {
        let text = "tree_width = 42\nside_margin_percent=10\n  top_margin_percent =  8\n\
                    autosave_seconds= 30\ntree_refresh_seconds =5\n";
        let (cfg, warnings) = parse(text);
        assert!(warnings.is_empty());
        assert_eq!(cfg.tree_width, 42);
        assert_eq!(cfg.side_margin_percent, 10);
        assert_eq!(cfg.top_margin_percent, 8);
        assert_eq!(cfg.autosave_seconds, 30);
        assert_eq!(cfg.tree_refresh_seconds, 5);
    }

    #[test]
    fn junk_lines_warn_and_leave_defaults() {
        let (cfg, warnings) = parse("this is not a setting\ntree_width 40\n");
        assert_eq!(cfg, Config::default());
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("line 1"));
        assert!(warnings[1].contains("line 2"));
    }

    #[test]
    fn unknown_keys_warn_and_leave_defaults() {
        let (cfg, warnings) = parse("colour = 7\ntree_width = 50\n");
        assert_eq!(cfg.tree_width, 50); // the good line still applies
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unknown option"));
        assert!(warnings[0].contains("colour"));
    }

    #[test]
    fn non_numeric_values_warn_and_leave_defaults() {
        let (cfg, warnings) = parse("tree_width = wide\nautosave_seconds = 2s\n");
        assert_eq!(cfg, Config::default());
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("not a number"));
    }

    #[test]
    fn out_of_range_values_are_clamped_not_ignored() {
        let text = "tree_width = 500\nside_margin_percent = 90\ntop_margin_percent = -3\n\
                    autosave_seconds = 0\ntree_refresh_seconds = 100000\n";
        let (cfg, warnings) = parse(text);
        assert!(warnings.is_empty());
        assert_eq!(cfg.tree_width, 120);
        assert_eq!(cfg.side_margin_percent, 40);
        assert_eq!(cfg.top_margin_percent, 0);
        assert_eq!(cfg.autosave_seconds, 1);
        assert_eq!(cfg.tree_refresh_seconds, 600);
    }

    #[test]
    fn margins_may_be_zero_and_tree_width_floors_at_10() {
        let (cfg, warnings) = parse("side_margin_percent = 0\ntree_width = 0\n");
        assert!(warnings.is_empty());
        assert_eq!(cfg.side_margin_percent, 0);
        assert_eq!(cfg.tree_width, 10);
    }

    #[test]
    fn later_lines_override_earlier_ones() {
        let (cfg, _) = parse("tree_width = 40\ntree_width = 50\n");
        assert_eq!(cfg.tree_width, 50);
    }

    #[test]
    fn duration_helpers_reflect_the_configured_seconds() {
        let (cfg, _) = parse("autosave_seconds = 7\ntree_refresh_seconds = 9\n");
        assert_eq!(cfg.autosave(), Duration::from_secs(7));
        assert_eq!(cfg.tree_refresh(), Duration::from_secs(9));
    }

    #[test]
    fn load_reads_xdg_config_home_and_defaults_when_absent() {
        // one test covers both cases so the env var is only touched here
        let dir = std::env::temp_dir().join("mrkdup-config-load");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mrkdup")).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        // no config file yet: defaults, no warnings
        let (cfg, warnings) = load();
        assert_eq!(cfg, Config::default());
        assert!(warnings.is_empty());

        std::fs::write(
            dir.join("mrkdup/config"),
            "# my settings\ntree_width = 44\nbogus line\n",
        )
        .unwrap();
        let (cfg, warnings) = load();
        assert_eq!(cfg.tree_width, 44);
        assert_eq!(warnings.len(), 1);
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
