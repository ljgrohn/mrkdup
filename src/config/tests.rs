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
fn theme_key_sets_theme_name() {
    let (cfg, warnings) = parse("theme = light\n");
    assert_eq!(cfg.theme_name, "light");
    assert!(warnings.is_empty());
}

#[test]
fn theme_key_rejects_invalid_name_and_keeps_default() {
    let (cfg, warnings) = parse("theme = nope!\n");
    assert_eq!(cfg.theme_name, "default");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("theme"));
}

#[test]
fn theme_key_rejects_uppercase_name() {
    let (cfg, warnings) = parse("theme = Default\n");
    assert_eq!(cfg.theme_name, "default");
    assert_eq!(warnings.len(), 1);
}

#[test]
fn theme_key_rejects_a_bare_number() {
    let (cfg, warnings) = parse("theme = 7\n");
    assert_eq!(cfg.theme_name, "default");
    assert_eq!(warnings.len(), 1);
}

#[test]
fn numeric_keys_still_reject_non_numeric_values_after_key_first_match() {
    let (cfg, warnings) = parse("tree_width = wide\n");
    assert_eq!(cfg.tree_width, 30);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("not a number"));
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
