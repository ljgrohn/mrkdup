use super::*;
use crate::config::Config;

#[test]
fn shape_round_trips_through_parse_and_as_str() {
    for shape in Shape::ALL {
        assert_eq!(Shape::parse(shape.as_str()), Some(shape));
    }
    assert_eq!(Shape::parse("Block"), None);
    assert_eq!(Shape::parse("beam"), None);
}

#[test]
fn color_hex_accepts_names_and_hex_and_rejects_junk() {
    assert_eq!(color_hex("red").as_deref(), Some("#ff0000"));
    assert_eq!(color_hex("grey").as_deref(), Some("#808080"));
    assert_eq!(color_hex("#ABCDEF").as_deref(), Some("#abcdef"));
    assert_eq!(color_hex("abcdef"), None);
    assert_eq!(color_hex("#abc"), None);
    assert_eq!(color_hex("#gggggg"), None);
    assert_eq!(color_hex("default"), None);
    assert!(valid_color("default"));
    assert!(valid_color("#123456"));
    assert!(!valid_color("mauve"));
    for name in COLOR_NAMES {
        assert!(valid_color(name), "{name}");
    }
}

#[test]
fn default_config_leaves_the_terminal_cursor_alone() {
    let cursor = Cursor::from_config(&Config::default());
    assert_eq!(
        cursor,
        Cursor {
            shape: Shape::Default,
            blink: true,
            color: None
        }
    );
    assert_eq!(cursor.sequence(), "\x1b[0 q\x1b]112\x07");
    assert_eq!(RESET, "\x1b[0 q\x1b]112\x07");
}

#[test]
fn decscusr_numbers_follow_the_shape_and_blink() {
    let cursor = |shape, blink| Cursor {
        shape,
        blink,
        color: None,
    };
    assert!(cursor(Shape::Block, true)
        .sequence()
        .starts_with("\x1b[1 q"));
    assert!(cursor(Shape::Block, false)
        .sequence()
        .starts_with("\x1b[2 q"));
    assert!(cursor(Shape::Underline, true)
        .sequence()
        .starts_with("\x1b[3 q"));
    assert!(cursor(Shape::Underline, false)
        .sequence()
        .starts_with("\x1b[4 q"));
    assert!(cursor(Shape::Bar, true).sequence().starts_with("\x1b[5 q"));
    assert!(cursor(Shape::Bar, false).sequence().starts_with("\x1b[6 q"));
    // the default shape ignores blink
    assert!(cursor(Shape::Default, false)
        .sequence()
        .starts_with("\x1b[0 q"));
}

#[test]
fn a_color_is_sent_as_osc_12_and_default_as_osc_112() {
    let mut cfg = Config {
        cursor_shape: Shape::Block,
        cursor_blink: false,
        cursor_color: "orange".to_string(),
        ..Config::default()
    };
    let cursor = Cursor::from_config(&cfg);
    assert_eq!(cursor.color.as_deref(), Some("#ffa500"));
    assert_eq!(cursor.sequence(), "\x1b[2 q\x1b]12;#ffa500\x07");
    cfg.cursor_color = "#0a0B0c".to_string();
    assert_eq!(
        Cursor::from_config(&cfg).sequence(),
        "\x1b[2 q\x1b]12;#0a0b0c\x07"
    );
}

#[test]
fn apply_and_reset_write_and_flush() {
    let mut out: Vec<u8> = Vec::new();
    let cursor = Cursor {
        shape: Shape::Bar,
        blink: true,
        color: Some("#ff0000".to_string()),
    };
    apply(&mut out, &cursor).unwrap();
    assert_eq!(out, b"\x1b[5 q\x1b]12;#ff0000\x07");
    out.clear();
    reset(&mut out).unwrap();
    assert_eq!(out, RESET.as_bytes());
}
