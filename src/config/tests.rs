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

#[test]
fn rewrite_key_line_replaces_in_place_and_keeps_everything_else() {
    let text = "# my config\ntree_width = 40\n  theme = light\nautosave_seconds = 5\n";
    assert_eq!(
        rewrite_key_line(text, "theme", "mono"),
        "# my config\ntree_width = 40\n  theme = mono\nautosave_seconds = 5\n"
    );
}

#[test]
fn rewrite_key_line_ignores_commented_out_and_lookalike_keys() {
    let text = "# theme = light\ntheme_name = x\nthemes = y\n";
    assert_eq!(
        rewrite_key_line(text, "theme", "mono"),
        "# theme = light\ntheme_name = x\nthemes = y\ntheme = mono\n"
    );
}

#[test]
fn rewrite_key_line_only_rewrites_the_first_duplicate() {
    let text = "theme=light\ntheme = mono\n";
    assert_eq!(
        rewrite_key_line(text, "theme", "firmitas"),
        "theme = firmitas\ntheme = mono\n"
    );
}

#[test]
fn rewrite_key_line_appends_with_and_without_trailing_newline() {
    assert_eq!(
        rewrite_key_line("tree_width = 40\n", "theme", "light"),
        "tree_width = 40\ntheme = light\n"
    );
    assert_eq!(
        rewrite_key_line("tree_width = 40", "theme", "light"),
        "tree_width = 40\ntheme = light\n"
    );
    assert_eq!(rewrite_key_line("", "theme", "light"), "theme = light\n");
}

#[test]
fn rewrite_key_line_output_reparses_to_the_new_name() {
    let out = rewrite_key_line("tree_width = 40\ntheme = light\n", "theme", "tokyonight");
    let (cfg, warnings) = parse(&out);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(cfg.theme_name, "tokyonight");
    assert_eq!(cfg.tree_width, 40);
}

#[test]
fn save_key_to_creates_the_file_and_preserves_other_keys() {
    let dir = std::env::temp_dir().join("mrkdup-config-save");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("mrkdup").join("config");

    // missing dir + file: created with just the theme line
    save_key_to(&path, "theme", "mono").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "theme = mono\n");

    // existing file: other keys and comments survive, theme rewritten
    std::fs::write(&path, "# keep me\ntree_width = 42\ntheme = mono\n").unwrap();
    save_key_to(&path, "theme", "light").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "# keep me\ntree_width = 42\ntheme = light\n");
    let (cfg, warnings) = parse(&text);
    assert!(warnings.is_empty());
    assert_eq!(cfg.tree_width, 42);
    assert_eq!(cfg.theme_name, "light");
}

#[test]
fn side_padding_parses_clamps_and_defaults() {
    assert_eq!(Config::default().side_padding, 1);
    let (cfg, warnings) = parse("side_padding = 3\n");
    assert!(warnings.is_empty());
    assert_eq!(cfg.side_padding, 3);
    let (cfg, _) = parse("side_padding = 99\n");
    assert_eq!(cfg.side_padding, 20);
    let (cfg, _) = parse("side_padding = -4\n");
    assert_eq!(cfg.side_padding, 0);
    let (_, warnings) = parse("side_padding = wide\n");
    assert!(warnings[0].contains("not a number"));
}

#[test]
fn cursor_keys_parse_and_default_to_the_terminals_own_cursor() {
    use crate::cursor::Shape;
    let cfg = Config::default();
    assert_eq!(cfg.cursor_shape, Shape::Default);
    assert!(cfg.cursor_blink);
    assert_eq!(cfg.cursor_color, "default");

    let (cfg, warnings) =
        parse("cursor_shape = Block\ncursor_blink = off\ncursor_color = Orange\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(cfg.cursor_shape, Shape::Block);
    assert!(!cfg.cursor_blink);
    assert_eq!(cfg.cursor_color, "orange");

    let (cfg, _) = parse("cursor_blink = false\ncursor_color = #AABBCC\n");
    assert!(!cfg.cursor_blink);
    assert_eq!(cfg.cursor_color, "#aabbcc");
    let (cfg, _) = parse("cursor_blink = yes\ncursor_shape = underline\n");
    assert!(cfg.cursor_blink);
    assert_eq!(cfg.cursor_shape, Shape::Underline);
}

#[test]
fn cursor_keys_warn_on_bad_values_and_keep_defaults() {
    let (cfg, warnings) =
        parse("cursor_shape = beam\ncursor_blink = maybe\ncursor_color = mauve\n");
    assert_eq!(cfg, Config::default());
    assert_eq!(warnings.len(), 3);
    assert!(
        warnings[0].starts_with("line 1: cursor_shape:"),
        "{}",
        warnings[0]
    );
    assert!(
        warnings[1].starts_with("line 2: cursor_blink:"),
        "{}",
        warnings[1]
    );
    assert!(
        warnings[2].starts_with("line 3: cursor_color:"),
        "{}",
        warnings[2]
    );
    let (cfg, warnings) = parse("cursor_color = #abc\n");
    assert_eq!(cfg.cursor_color, "default");
    assert_eq!(warnings.len(), 1);
}

#[test]
fn rewrite_key_line_is_generic_over_the_key() {
    assert_eq!(
        rewrite_key_line("theme = light\n", "side_padding", "2"),
        "theme = light\nside_padding = 2\n"
    );
    assert_eq!(
        rewrite_key_line(
            "side_padding = 1\nside_padding_x = 9\n",
            "side_padding",
            "3"
        ),
        "side_padding = 3\nside_padding_x = 9\n"
    );
}
