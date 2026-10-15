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
  collect [root] [--container] [--only <tool,...>] [--rules <dir>] [--dry-run]
                  Run the installed analyzers with fixed flags and write
                  their reports to .sloptopop/reports/ (see docs/collect.md);
                  --dry-run prints the plan without running anything
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

/// The options of `collect`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CollectOptions {
    /// The repository root (default: the current directory).
    pub(crate) root: String,
    /// Run each tool from its pinned container image instead of `PATH`.
    pub(crate) container: bool,
    /// Only these catalogue entries (empty: every applicable entry).
    pub(crate) only: Vec<String>,
    /// The local rules directory for Opengrep and Semgrep.
    pub(crate) rules: Option<String>,
    /// Print the plan without running anything.
    pub(crate) dry_run: bool,
}

/// A parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    /// `check [path] [--format text|json]`.
    Check(String, Format),
    /// `fix [path] [--apply]`.
    Fix(String, bool),
    /// `collect [root] [--container] [--only <tool,...>] [--rules <dir>] [--dry-run]`.
    Collect(CollectOptions),
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
        ["collect", rest @ ..] => parse_collect(rest),
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

fn parse_collect(rest: &[&str]) -> Result<Command, String> {
    let mut options = CollectOptions::default();
    let mut root = None;
    let mut rest = rest.iter();
    while let Some(&arg) = rest.next() {
        match arg {
            "--container" => options.container = true,
            "--dry-run" => options.dry_run = true,
            "--only" => {
                for name in value(arg, rest.next())?.split(',') {
                    if name.is_empty() {
                        return Err("--only has an empty tool name".into());
                    }
                    options.only.push(name.to_owned());
                }
            }
            "--rules" => options.rules = Some(value(arg, rest.next())?.to_owned()),
            path if root.is_none() => root = Some(checked_path(path)?),
            _ => return Err(format!("unrecognised option or argument: {arg}")),
        }
    }
    options.root = root.unwrap_or_else(|| ".".to_owned());
    Ok(Command::Collect(options))
}

/// The value after `option`, which must be present and not an option.
fn value<'a>(option: &str, next: Option<&&'a str>) -> Result<&'a str, String> {
    match next {
        Some(value) if !value.starts_with('-') => Ok(value),
        _ => Err(format!("{option} needs a value")),
    }
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
