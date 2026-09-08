use super::*;
use std::path::PathBuf;

fn parsed(args: &[&str]) -> Result<Action, String> {
    parse(args.iter().copied())
}

#[test]
fn no_args_runs_with_current_dir() {
    assert_eq!(parsed(&[]), Ok(Action::Run { root: None }));
}

#[test]
fn one_dir_runs_with_that_root() {
    assert_eq!(
        parsed(&["notes"]),
        Ok(Action::Run {
            root: Some(PathBuf::from("notes")),
        })
    );
}

#[test]
fn extra_trailing_args_are_an_error() {
    assert!(parsed(&["notes", "extra"]).is_err());
    assert!(parsed(&["a", "b", "c"]).is_err());
}

#[test]
fn help_after_directory_shows_help() {
    assert_eq!(parsed(&["notes", "--help"]), Ok(Action::Help));
    assert_eq!(parsed(&["notes", "-h"]), Ok(Action::Help));
}

#[test]
fn help_before_directory_shows_help() {
    assert_eq!(parsed(&["--help", "notes"]), Ok(Action::Help));
}

#[test]
fn version_is_recognized_in_any_position() {
    assert_eq!(parsed(&["--version"]), Ok(Action::Version));
    assert_eq!(parsed(&["-V"]), Ok(Action::Version));
    assert_eq!(parsed(&["notes", "--version"]), Ok(Action::Version));
    assert_eq!(parsed(&["notes", "-V"]), Ok(Action::Version));
}

#[test]
fn flags_take_precedence_over_extra_positionals() {
    assert_eq!(parsed(&["a", "b", "--help"]), Ok(Action::Help));
    assert_eq!(parsed(&["a", "b", "--version"]), Ok(Action::Version));
}
