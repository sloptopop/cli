//! The command-line grammar: the usage text and parsing arguments into a
//! [`Command`].
//!
//! Standards: SCF TDA-12 (Input Data Validation: only the documented
//! grammar is accepted; anything else is a usage error).

use std::ffi::OsString;

/// The usage text, printed by `help` and after usage errors.
pub(crate) const USAGE: &str = "\
Usage: sloptopop <command>

Commands:
  check [path] [--format text|json] [--backend <url>] [--yes]
                  Check a repository (default: the current directory) with
                  the sloptopop server; exit 0 if it passes, 1 if not
  fix [path] [--apply] [--backend <url>] [--yes]
                  Show the files the server's mechanical fixes would
                  create; --apply commits them on a new branch (existing
                  files are never touched)
  collect [root] [--only <tool,...>] [--rules <dir>] [--dry-run] [--offline]
          [--registry <host[/path]>] [--runtime docker|podman]
          [--timeout <seconds>] [--vuln-db <YYYYMMDD>] [--host-tools]
                  Run the applicable analyzers from sloptopop's pinned
                  container images, with fixed flags, and write their
                  reports to .sloptopop/reports/ (see docs/collect.md);
                  --dry-run prints the plan without fetching or running
                  anything; --host-tools runs unpinned tools from PATH
                  instead (unsupported). Exit code 3: no container runtime
  transform [root] [--apply] [--branch <name>] [--backend <url>] [--yes]
            [--confirm <id,...> --by <name>] [--format text|json]
                  Plan verified transformations with the sloptopop server
                  and apply them on a new branch (preview; see
                  docs/commands.md#transform-preview); without --apply a
                  dry run
  local <arguments>
                  Run sloptopop-local (the open-source half on this
                  machine) with these arguments, e.g. local evidence . --run <id>
  serve --stdio   Run the JSON-RPC server on stdin/stdout (used by the
                  language libraries)
  schema          Print the protocol description as JSON
  version         Print the version

Options:
  -h, --help      Print this help
  -V, --version   Print the version

sloptopop detects only on its server (sloptopop-engine): the hosted
service (the default) or your self-hosted one (--backend <url>). check,
fix and transform prepare the upload with sloptopop-local (open source),
found via the SLOPTOPOP_LOCAL environment variable, next to this
executable, or on PATH; it masks every secret value gitleaks reports, so
run `sloptopop collect` first. They show what they upload and ask first
(--yes in CI), then send one request with curl. Exit code 3 means
sloptopop-local is missing or failed; 4 means the upload was not confirmed
or the server could not be reached or refused it.
";

/// How `check` prints the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    /// Rendered as readable text.
    Text,
    /// The engine's JSON, unchanged.
    Json,
}

/// Where `collect` takes the tools from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Source {
    /// The pinned images (the default).
    #[default]
    Images,
    /// The pinned images, with the old `--container` flag given (it has
    /// no effect; only a note is printed).
    ContainerFlag,
    /// `--host-tools`: unpinned tools from `PATH` (unsupported).
    HostTools,
}

/// The options of `collect`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CollectOptions {
    /// The repository root (default: the current directory).
    pub(crate) root: String,
    /// Where the tools come from.
    pub(crate) source: Source,
    /// Only these catalogue entries (empty: every applicable entry).
    pub(crate) only: Vec<String>,
    /// The local rules directory for Opengrep and Semgrep.
    pub(crate) rules: Option<String>,
    /// Print the plan without fetching or running anything.
    pub(crate) dry_run: bool,
    /// Never fetch: use only images and data already present.
    pub(crate) offline: bool,
    /// The registry to pull from instead of the default.
    pub(crate) registry: Option<String>,
    /// The container runtime to use: `docker` or `podman`.
    pub(crate) runtime: Option<String>,
    /// The time limit of each tool, in seconds.
    pub(crate) timeout: Option<u64>,
    /// The vulnerability snapshot to use instead of the newest, `YYYYMMDD`.
    pub(crate) vuln_db: Option<String>,
}

/// The options of `check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckOptions {
    /// The repository root (default: the current directory).
    pub(crate) root: String,
    /// How to print the Report.
    pub(crate) format: Format,
    /// The server's base URL.
    pub(crate) backend: Option<String>,
    /// Upload without asking.
    pub(crate) yes: bool,
}

/// The options of `fix`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixOptions {
    /// The repository root (default: the current directory).
    pub(crate) root: String,
    /// Commit the fixes on a branch (otherwise a dry run).
    pub(crate) apply: bool,
    /// The server's base URL.
    pub(crate) backend: Option<String>,
    /// Upload without asking.
    pub(crate) yes: bool,
}

/// The options of `transform`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransformOptions {
    /// The repository root (default: the current directory).
    pub(crate) root: String,
    /// Commit the verified steps on a branch (otherwise a dry run).
    pub(crate) apply: bool,
    /// The branch to make.
    pub(crate) branch: Option<String>,
    /// The backend's base URL.
    pub(crate) backend: Option<String>,
    /// Upload without asking.
    pub(crate) yes: bool,
    /// Attestations to confirm.
    pub(crate) confirm: Vec<String>,
    /// Who confirms them.
    pub(crate) by: Option<String>,
    /// How to print the result.
    pub(crate) format: Format,
}

/// A parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    /// `check [path] [options]`.
    Check(CheckOptions),
    /// `fix [path] [options]`.
    Fix(FixOptions),
    /// `local <arguments>`.
    Local(Vec<String>),
    /// `collect [root] [options]`.
    Collect(CollectOptions),
    /// `transform [root] …`.
    Transform(TransformOptions),
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
        ["transform", rest @ ..] => parse_transform(rest),
        ["local", rest @ ..] if !rest.is_empty() => Ok(Command::Local(
            rest.iter().map(|arg| (*arg).to_owned()).collect(),
        )),
        ["local"] => {
            Err("local needs arguments for sloptopop-local (try: sloptopop local version)".into())
        }
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
    let mut options = CheckOptions {
        root: ".".to_owned(),
        format: Format::Text,
        backend: None,
        yes: false,
    };
    let mut root = None;
    let mut rest = rest.iter();
    while let Some(&arg) = rest.next() {
        match arg {
            "--yes" => options.yes = true,
            "--backend" => options.backend = Some(value(arg, rest.next())?.to_owned()),
            "--format" => options.format = format(value(arg, rest.next())?)?,
            path if root.is_none() => root = Some(checked_path(path)?),
            _ => return Err(format!("unrecognised arguments: check {arg}")),
        }
    }
    options.root = root.unwrap_or_else(|| ".".to_owned());
    Ok(Command::Check(options))
}

fn parse_fix(rest: &[&str]) -> Result<Command, String> {
    let mut options = FixOptions {
        root: ".".to_owned(),
        apply: false,
        backend: None,
        yes: false,
    };
    let mut root = None;
    let mut rest = rest.iter();
    while let Some(&arg) = rest.next() {
        match arg {
            "--apply" => options.apply = true,
            "--yes" => options.yes = true,
            "--backend" => options.backend = Some(value(arg, rest.next())?.to_owned()),
            path if root.is_none() => root = Some(checked_path(path)?),
            _ => return Err(format!("unrecognised arguments: fix {arg}")),
        }
    }
    options.root = root.unwrap_or_else(|| ".".to_owned());
    Ok(Command::Fix(options))
}

/// The output format `name` names.
fn format(name: &str) -> Result<Format, String> {
    match name {
        "text" => Ok(Format::Text),
        "json" => Ok(Format::Json),
        other => Err(format!("unknown format: {other} (expected text or json)")),
    }
}

fn parse_collect(rest: &[&str]) -> Result<Command, String> {
    let mut options = CollectOptions::default();
    let mut root = None;
    let mut rest = rest.iter();
    while let Some(&arg) = rest.next() {
        match arg {
            "--container" | "--host-tools" => {
                let wanted = if arg == "--container" {
                    Source::ContainerFlag
                } else {
                    Source::HostTools
                };
                if options.source != Source::Images && options.source != wanted {
                    return Err("--host-tools and --container exclude each other".into());
                }
                options.source = wanted;
            }
            "--dry-run" => options.dry_run = true,
            "--offline" => options.offline = true,
            "--only" => {
                for name in value(arg, rest.next())?.split(',') {
                    if name.is_empty() {
                        return Err("--only has an empty tool name".into());
                    }
                    options.only.push(name.to_owned());
                }
            }
            "--rules" => options.rules = Some(value(arg, rest.next())?.to_owned()),
            "--registry" => options.registry = Some(registry(value(arg, rest.next())?)?),
            "--runtime" => {
                options.runtime = Some(match value(arg, rest.next())? {
                    name @ ("docker" | "podman") => name.to_owned(),
                    other => {
                        return Err(format!(
                            "unknown runtime: {other} (expected docker or podman)"
                        ));
                    }
                });
            }
            "--timeout" => {
                let seconds = value(arg, rest.next())?;
                options.timeout = Some(seconds.parse().ok().filter(|&s: &u64| s > 0).ok_or_else(
                    || format!("--timeout needs a whole number of seconds, not {seconds}"),
                )?);
            }
            "--vuln-db" => {
                let date = value(arg, rest.next())?;
                if date.len() != 8 || !date.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(format!("--vuln-db needs a date as YYYYMMDD, not {date}"));
                }
                options.vuln_db = Some(date.to_owned());
            }
            path if root.is_none() => root = Some(checked_path(path)?),
            _ => return Err(format!("unrecognised option or argument: {arg}")),
        }
    }
    options.root = root.unwrap_or_else(|| ".".to_owned());
    Ok(Command::Collect(options))
}

/// A registry given with `--registry` (or `SLOPTOPOP_REGISTRY`): a host,
/// optionally with a port and a path, without scheme or trailing slash.
pub(crate) fn registry(value: &str) -> Result<String, String> {
    let trimmed = value.trim_end_matches('/');
    let valid = !trimmed.is_empty()
        && !trimmed.contains("://")
        && !trimmed.starts_with('/')
        && trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':' | b'/'));
    if valid {
        Ok(trimmed.to_owned())
    } else {
        Err(format!(
            "not a registry: {value} (expected a host, optionally with a port and a path, e.g. registry.example.com/mirror)"
        ))
    }
}

fn parse_transform(rest: &[&str]) -> Result<Command, String> {
    let mut options = TransformOptions {
        root: ".".to_owned(),
        apply: false,
        branch: None,
        backend: None,
        yes: false,
        confirm: Vec::new(),
        by: None,
        format: Format::Text,
    };
    let mut root = None;
    let mut rest = rest.iter();
    while let Some(&arg) = rest.next() {
        match arg {
            "--apply" => options.apply = true,
            "--yes" => options.yes = true,
            "--branch" => options.branch = Some(value(arg, rest.next())?.to_owned()),
            "--backend" => options.backend = Some(value(arg, rest.next())?.to_owned()),
            "--by" => options.by = Some(value(arg, rest.next())?.to_owned()),
            "--confirm" => options.confirm.extend(
                value(arg, rest.next())?
                    .split(',')
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned),
            ),
            "--format" => options.format = format(value(arg, rest.next())?)?,
            path if root.is_none() => root = Some(checked_path(path)?),
            _ => return Err(format!("unrecognised option or argument: {arg}")),
        }
    }
    if options.confirm.is_empty() != options.by.is_none() {
        return Err("--confirm and --by go together".into());
    }
    options.root = root.unwrap_or_else(|| ".".to_owned());
    Ok(Command::Transform(options))
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
