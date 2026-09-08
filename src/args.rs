use std::path::PathBuf;

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Print the usage text and exit successfully.
    Help,
    /// Print the version and exit successfully.
    Version,
    /// Open the tree here (`None` means the current directory).
    Run { root: Option<PathBuf> },
}

/// Parse CLI arguments (without `argv[0]`).
///
/// `--help`/`-h` and `--version`/`-V` are recognized in any position and
/// take precedence over directories; anything else counts as a directory,
/// and more than one directory is an error.
pub fn parse<I, S>(args: I) -> Result<Action, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut saw_help = false;
    let mut saw_version = false;
    let mut roots: Vec<PathBuf> = Vec::new();
    for arg in args {
        match arg.as_ref() {
            "--help" | "-h" => saw_help = true,
            "--version" | "-V" => saw_version = true,
            dir => roots.push(PathBuf::from(dir)),
        }
    }
    if saw_help {
        return Ok(Action::Help);
    }
    if saw_version {
        return Ok(Action::Version);
    }
    if roots.len() > 1 {
        return Err(format!(
            "expected at most one directory, got {}",
            roots.len()
        ));
    }
    Ok(Action::Run {
        root: roots.into_iter().next(),
    })
}

#[cfg(test)]
mod tests;
