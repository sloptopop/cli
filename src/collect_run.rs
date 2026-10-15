//! Running the planned tools, each in the repository root, and recording
//! what happened. A tool that fails is recorded; it never stops the others.

use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::collect_catalogue::Tool;
use crate::collect_plan::{Planned, Step};

/// What happened to one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Status {
    /// It ran and exited with 0 or one of its "findings found" codes.
    Ran(i32),
    /// It ran and failed: another exit code (none if a signal ended it),
    /// or it could not be started.
    Failed {
        /// The exit code, if it exited.
        code: Option<i32>,
        /// The first line it wrote to stderr, or what went wrong.
        detail: String,
    },
    /// The executable is not installed.
    Missing(String),
    /// It was not run, for this reason.
    Skipped(String),
}

impl Status {
    /// `ran`, `failed`, `missing` or `skipped`.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Ran(_) => "ran",
            Self::Failed { .. } => "failed",
            Self::Missing(_) => "missing",
            Self::Skipped(_) => "skipped",
        }
    }

    /// The exit code, if the tool exited.
    pub(crate) fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Ran(code) => Some(*code),
            Self::Failed { code, .. } => *code,
            Self::Missing(_) | Self::Skipped(_) => None,
        }
    }

    /// Why the tool did not run or failed.
    pub(crate) fn reason(&self) -> Option<&str> {
        match self {
            Self::Ran(_) => None,
            Self::Failed { detail: reason, .. } | Self::Missing(reason) | Self::Skipped(reason) => {
                Some(reason)
            }
        }
    }
}

/// The outcome of one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outcome<'t> {
    /// The catalogue entry.
    pub(crate) tool: &'t Tool,
    /// The command line, program name first.
    pub(crate) argv: Vec<String>,
    /// The first line of the tool's `--version` output.
    pub(crate) version: Option<String>,
    /// What happened.
    pub(crate) status: Status,
}

/// Maps an exit code to a status: 0 and the tool's `findings` codes are
/// success; anything else, or no code at all, is a failure described by
/// `detail`.
pub(crate) fn status(
    code: Option<i32>,
    findings: &[i32],
    detail: impl FnOnce() -> String,
) -> Status {
    match code {
        Some(code) if code == 0 || findings.contains(&code) => Status::Ran(code),
        code => Status::Failed {
            code,
            detail: detail(),
        },
    }
}

/// Runs one planned entry with `root` as the working directory.
pub(crate) fn run<'t>(planned: Planned<'t>, root: &Path) -> Outcome<'t> {
    let Planned { tool, argv, step } = planned;
    let (version, status) = match step {
        Step::Missing(reason) => (None, Status::Missing(reason)),
        Step::Skipped(reason) => (None, Status::Skipped(reason)),
        Step::Run {
            program,
            version,
            stdout,
        } => (
            query_version(&program, &version, root),
            execute(tool, &program, &argv, stdout.as_deref(), root),
        ),
    };
    Outcome {
        tool,
        argv,
        version,
        status,
    }
}

/// The first non-empty line `program version...` prints, if it succeeds.
fn query_version(program: &Path, version: &[String], root: &Path) -> Option<String> {
    let output = Command::new(program)
        .args(version)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if output.status.success() {
        first_line(&output.stdout)
    } else {
        None
    }
}

fn execute(
    tool: &Tool,
    program: &Path,
    argv: &[String],
    stdout: Option<&Path>,
    root: &Path,
) -> Status {
    let sink = match stdout.map(File::create) {
        None => Stdio::null(),
        Some(Ok(file)) => Stdio::from(file),
        Some(Err(e)) => {
            return Status::Failed {
                code: None,
                detail: format!("cannot write the report: {e}"),
            };
        }
    };
    match Command::new(program)
        .args(argv.iter().skip(1))
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(sink)
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => status(output.status.code(), tool.findings, || {
            first_line(&output.stderr).unwrap_or_else(|| output.status.to_string())
        }),
        Err(e) => Status::Failed {
            code: None,
            detail: format!("cannot run {}: {e}", program.display()),
        },
    }
}

fn first_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests;
