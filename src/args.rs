//! The command-line grammar: the usage text and parsing arguments into a
//! [`Command`].

use std::ffi::OsString;

/// The usage text, printed by `help` and after usage errors.
pub(crate) const USAGE: &str = "\
Usage: sloptopop <command>

Commands:
  check [path] [--format text|json]
                  Check a repository (default: the current directory);
                  exit 0 if it passes, 1 if not
  fix [path] [--apply]
                  Show the files that mechanical fixes would create;
                  --apply creates them (existing files are never touched)
  serve --stdio   Run the JSON-RPC server on stdin/stdout (used by the
                  language libraries)
  schema          Print the protocol description as JSON
  version         Print the version

Options:
  -h, --help      Print this help
  -V, --version   Print the version

check and fix run the separately installed sloptopop-engine, found via
the SLOPTOPOP_ENGINE environment variable, next to this executable, or on
PATH. Exit code 3 means the engine is missing or failed.
";

/// How `check` prints the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    /// Rendered as readable text.
    Text,
    /// The engine's JSON, unchanged.
    Json,
}

/// A parsed command line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// `check [path] [--format text|json]`.
    Check(String, Format),
    /// `fix [path] [--apply]`.
    Fix(String, bool),
    /// `serve --stdio`.
    Serve,
    /// `schema`.
    Schema,
    /// `version`, `--version` or `-V`.
    Version,
    /// `help`, `--help` or `-h`.
    Help,
}

/// Parses the arguments after the program name; the error is the message
/// to show above the usage text.
pub(crate) fn parse(args: &[OsString]) -> Result<Command, String> {
    let args: Vec<&str> = args
        .iter()
        .map(|a| a.to_str().unwrap_or("\u{fffd}"))
        .collect();
    match args.as_slice() {
        ["check", rest @ ..] => parse_check(rest),
        ["fix", rest @ ..] => parse_fix(rest),
        ["serve", "--stdio"] => Ok(Command::Serve),
        ["serve"] => Err("serve needs a transport: sloptopop serve --stdio".into()),
        ["schema"] => Ok(Command::Schema),
        ["version" | "--version" | "-V"] => Ok(Command::Version),
        ["help" | "--help" | "-h"] => Ok(Command::Help),
        [] => Err("no command given".into()),
        other => Err(format!("unrecognised arguments: {}", other.join(" "))),
    }
}

fn parse_check(rest: &[&str]) -> Result<Command, String> {
    let (path, format) = match rest {
        [] => (".", "text"),
        ["--format", format] => (".", *format),
        [path] => (*path, "text"),
        [path, "--format", format] => (*path, *format),
        _ => return Err(format!("unrecognised arguments: check {}", rest.join(" "))),
    };
    let format = match format {
        "text" => Format::Text,
        "json" => Format::Json,
        other => return Err(format!("unknown format: {other} (expected text or json)")),
    };
    Ok(Command::Check(checked_path(path)?, format))
}

fn parse_fix(rest: &[&str]) -> Result<Command, String> {
    let (path, apply) = match rest {
        [] => (".", false),
        ["--apply"] => (".", true),
        [path] => (*path, false),
        [path, "--apply"] => (*path, true),
        _ => return Err(format!("unrecognised arguments: fix {}", rest.join(" "))),
    };
    Ok(Command::Fix(checked_path(path)?, apply))
}

fn checked_path(path: &str) -> Result<String, String> {
    if path.starts_with('-') {
        Err(format!("unrecognised option: {path}"))
    } else {
        Ok(path.to_owned())
    }
}

#[cfg(test)]
mod tests;
