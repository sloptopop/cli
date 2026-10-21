//! Running the planned tools, each in the repository root, and recording
//! what happened: from its pinned image in a hardened container (or, under
//! `--host-tools`, from `PATH`), with a wall-clock limit, after removing
//! the report an earlier run left, and converting the output to SARIF
//! where the tool writes another format. A tool that fails is recorded; it
//! never stops the others.
//!
//! Standards: SCF VPM-12 (Vulnerability Scanning), MON-09 (Content of
//! Event Logs: each tool's outcome is recorded), SEA-04 (Process
//! Isolation), CAP-01 (Resource Management: time limits).

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::collect_catalogue::{Image, REPORTS_DIR, Tool};
use crate::collect_container::{Limits, Mount, Spec, run_argv};
use crate::collect_convert::{convert, normalize};
use crate::collect_plan::{NotRun, Planned, Step};
use crate::collect_runtime::{Runtime, first_line};

/// What happened to one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Status {
    /// It ran and exited with 0 or one of its "findings found" codes.
    Ran(i32),
    /// It ran and failed: another exit code (none if a signal ended it),
    /// or it could not be started or finished.
    Failed {
        /// The exit code, if it exited.
        code: Option<i32>,
        /// The first line it wrote to stderr, or what went wrong.
        detail: String,
    },
    /// The executable is not installed (`--host-tools`), or there is no
    /// container runtime.
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

    /// A failure without an exit code.
    pub(crate) fn failure(detail: impl Into<String>) -> Self {
        Self::Failed {
            code: None,
            detail: detail.into(),
        }
    }
}

impl From<NotRun> for Status {
    fn from(not_run: NotRun) -> Self {
        match not_run {
            NotRun::Missing(reason) => Self::Missing(reason),
            NotRun::Skipped(reason) => Self::Skipped(reason),
        }
    }
}

/// The image an entry ran from, as recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Used {
    /// The reference pulled: `<registry>/<name>@<digest>`.
    pub(crate) reference: String,
    /// The image's version label.
    pub(crate) version: &'static str,
}

/// The outcome of one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outcome<'t> {
    /// The catalogue entry.
    pub(crate) tool: &'t Tool,
    /// The command line as run, program name first.
    pub(crate) argv: Vec<String>,
    /// The tool's version: the image's label, or under `--host-tools` the
    /// first line of its `--version`.
    pub(crate) version: Option<String>,
    /// The image it ran from.
    pub(crate) image: Option<Used>,
    /// What happened.
    pub(crate) status: Status,
}

impl<'t> Outcome<'t> {
    /// An entry that did not run (or failed before it could).
    pub(crate) fn without_run<R>(
        planned: Planned<'t, R>,
        image: Option<Used>,
        status: Status,
    ) -> Self {
        Self {
            tool: planned.tool,
            argv: planned.argv,
            version: image.as_ref().map(|used| used.version.to_owned()),
            image,
            status,
        }
    }
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

/// What every container of a run shares.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Shared<'a> {
    /// The runtime.
    pub(crate) runtime: &'a Runtime,
    /// The resource limits.
    pub(crate) limits: &'a Limits,
    /// The repository owner's `uid:gid`.
    pub(crate) owner: Option<&'a str>,
    /// The repository root, absolute.
    pub(crate) root: &'a Path,
    /// The wall-clock limit of each container.
    pub(crate) timeout: Duration,
}

/// One container to run, apart from what [`Shared`] holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Job<'a> {
    /// Its unique name.
    pub(crate) name: String,
    /// Whether it gets network (a prepare step).
    pub(crate) network: bool,
    /// Its environment beyond the fixed variables.
    pub(crate) env: Vec<(String, String)>,
    /// Its mounts.
    pub(crate) mounts: Vec<Mount>,
    /// The executable in the image.
    pub(crate) entrypoint: &'a str,
    /// The image reference or ID to run.
    pub(crate) image: &'a str,
    /// The arguments after the image.
    pub(crate) args: Vec<String>,
}

impl Job<'_> {
    /// The full command line.
    pub(crate) fn argv(&self, shared: &Shared<'_>) -> Vec<String> {
        run_argv(&Spec {
            runtime: shared.runtime,
            name: self.name.clone(),
            network: self.network,
            owner: shared.owner.map(str::to_owned),
            limits: shared.limits,
            env: self.env.clone(),
            mounts: self.mounts.clone(),
            entrypoint: self.entrypoint,
            image: self.image,
            args: self.args.clone(),
        })
    }
}

/// How a process ended.
#[derive(Debug)]
enum Ended {
    /// It exited (or was ended by a signal: no code).
    Exited(ExitStatus, Vec<u8>),
    /// It ran past the time limit and was stopped.
    TimedOut,
}

/// Runs `command` (stdin closed, stdout to `stdout` or nowhere, stderr
/// collected) to its end. With `limit`, a time limit and how to stop the
/// process once it has passed: the wait for it is bounded, the stop is
/// asked for once, and its end is then awaited.
fn supervise(
    command: &mut Command,
    stdout: Option<&Path>,
    limit: Option<(Duration, &dyn Fn())>,
) -> io::Result<Ended> {
    let sink = match stdout {
        Some(path) => Stdio::from(File::create(path)?),
        None => Stdio::null(),
    };
    let child = command
        .stdin(Stdio::null())
        .stdout(sink)
        .stderr(Stdio::piped())
        .spawn()?;
    let (done, ended) = mpsc::channel();
    // The waiter owns the child: it collects stderr while waiting, so a
    // tool that writes a lot to stderr never blocks on a full pipe.
    let waiter = thread::spawn(move || {
        let _sent = done.send(child.wait_with_output());
    });
    let finished = match limit {
        None => ended.recv().ok(),
        Some((timeout, stop)) => {
            if let Ok(output) = ended.recv_timeout(timeout) {
                Some(output)
            } else {
                stop();
                let _drained = ended.recv();
                None
            }
        }
    };
    let _joined = waiter.join();
    match finished {
        Some(output) => output.map(|out| Ended::Exited(out.status, out.stderr)),
        None => Ok(Ended::TimedOut),
    }
}

/// The status of a finished process.
fn ended(result: io::Result<Ended>, what: &str, findings: &[i32], timeout: Duration) -> Status {
    match result {
        Err(e) => Status::failure(format!("cannot run {what}: {e}")),
        Ok(Ended::TimedOut) => Status::failure(format!(
            "stopped after {} s (the time limit; --timeout changes it)",
            timeout.as_secs()
        )),
        Ok(Ended::Exited(exit, stderr)) => status(exit.code(), findings, || {
            first_line(&stderr).unwrap_or_else(|| exit.to_string())
        }),
    }
}

/// Runs one container to its end, or until the time limit stops it, and
/// returns its status.
pub(crate) fn run_job(
    shared: &Shared<'_>,
    job: &Job<'_>,
    stdout: Option<&Path>,
    findings: &[i32],
) -> Status {
    let argv = job.argv(shared);
    let mut command = Command::new(&shared.runtime.program);
    command.args(argv.iter().skip(1)).current_dir(shared.root);
    let stop = || {
        let _stopped = shared
            .runtime
            .output(&["kill", job.name.as_str()], shared.root);
    };
    ended(
        supervise(&mut command, stdout, Some((shared.timeout, &stop))),
        shared.runtime.engine.name(),
        findings,
        shared.timeout,
    )
}

/// Removes what an earlier run of `tool` left in the reports: its raw
/// output and its report(s), so a tool that fails leaves no stale report
/// behind.
fn clear(root: &Path, tool: &Tool) -> io::Result<()> {
    remove(&root.join(tool.written_path()))?;
    remove_reports(root, tool)
}

/// Removes the file `path` if there is one; anything else there (a
/// directory) is an error, since the tool could not write its output.
fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
}

/// Removes `tool`'s report: the file, or every file matching it if it is
/// a pattern (one `*`).
fn remove_reports(root: &Path, tool: &Tool) -> io::Result<()> {
    let reports = root.join(REPORTS_DIR);
    match tool.output.split_once('*') {
        None => remove(&reports.join(tool.output)),
        Some((prefix, suffix)) => matching(&reports, prefix, suffix)?
            .iter()
            .try_for_each(fs::remove_file),
    }
}

/// The files in `reports` whose names are `prefix`, something, `suffix`,
/// sorted.
fn matching(reports: &Path, prefix: &str, suffix: &str) -> io::Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = fs::read_dir(reports)?
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.len() >= prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
        })
        .map(|entry| entry.path())
        .collect();
    found.sort();
    Ok(found)
}

/// The final status of a run: after a success, the raw output converted
/// to the SARIF report where the tool writes another format; after a
/// failure, no report at all (what a failed run wrote is not evidence).
fn settle(root: &Path, tool: &Tool, version: &str, status: Status) -> Status {
    let status = finish(root, tool, version, status);
    if !matches!(status, Status::Ran(_)) {
        // Best effort: the failure is what gets recorded either way.
        let _removed = remove_reports(root, tool);
    }
    status
}

/// After a successful run, converts the raw output to the SARIF report
/// where the tool writes another format, then normalizes every SARIF
/// report it wrote (see `collect_convert::normalize`).
fn finish(root: &Path, tool: &Tool, version: &str, status: Status) -> Status {
    let Status::Ran(code) = status else {
        return status;
    };
    let read =
        |path: &Path, name: &str| fs::read(path).map_err(|e| format!("cannot read {name}: {e}"));
    let write = |path: &Path, name: &str, text: String| {
        fs::write(path, text).map_err(|e| format!("cannot write {name}: {e}"))
    };
    let mut done = Ok(());
    if let Some((format, _)) = tool.convert {
        done = read(&root.join(tool.written_path()), &tool.written_path())
            .and_then(|bytes| convert(format, &bytes, tool.name, version))
            .and_then(|sarif| write(&root.join(tool.output_path()), &tool.output_path(), sarif));
    }
    let reports = done.and_then(|()| {
        sarif_reports(root, tool).map_err(|e| format!("cannot list the reports: {e}"))
    });
    let normalized = reports.and_then(|paths| {
        paths.iter().try_for_each(|path| {
            let name = path
                .strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string();
            read(path, &name)
                .and_then(|bytes| normalize(&bytes, tool.name, version))
                .and_then(|sarif| write(path, &name, sarif))
        })
    });
    match normalized {
        Ok(()) => Status::Ran(code),
        Err(detail) => Status::Failed {
            code: Some(code),
            detail,
        },
    }
}

/// The SARIF reports `tool` wrote: its report, or every file matching it
/// if it is a pattern; none if it is not SARIF or was not written.
fn sarif_reports(root: &Path, tool: &Tool) -> io::Result<Vec<PathBuf>> {
    let reports = root.join(REPORTS_DIR);
    if Path::new(tool.output)
        .extension()
        .is_none_or(|e| e != "sarif")
    {
        return Ok(Vec::new());
    }
    match tool.output.split_once('*') {
        None => {
            let report = reports.join(tool.output);
            Ok(report.is_file().then_some(report).into_iter().collect())
        }
        Some((prefix, suffix)) => matching(&reports, prefix, suffix),
    }
}

/// Runs a planned entry from its image, as `job` describes (the
/// reference or ID that passed verification, and the mounts its data and
/// volumes need), and records `used` as the image it ran from.
pub(crate) fn run_image<'t>(
    planned: &Planned<'t, Image>,
    image: &Image,
    used: Used,
    job: &Job<'_>,
    shared: &Shared<'_>,
) -> Outcome<'t> {
    let tool = planned.tool;
    let status = match clear(shared.root, tool) {
        Err(e) => Status::failure(format!("cannot remove an earlier report: {e}")),
        Ok(()) => settle(
            shared.root,
            tool,
            image.version,
            run_job(shared, job, planned.stdout.as_deref(), tool.findings),
        ),
    };
    Outcome {
        tool,
        argv: job.argv(shared),
        version: Some(image.version.to_owned()),
        image: Some(used),
        status,
    }
}

/// Runs a planned `--host-tools` entry with `root` as the working
/// directory, to its end: unsupported, so without a time limit.
pub(crate) fn run_host<'t>(planned: Planned<'t, PathBuf>, root: &Path) -> Outcome<'t> {
    let Planned {
        tool,
        argv,
        stdout,
        step,
    } = planned;
    let program = match step {
        Step::Run(program) => program,
        Step::NotRun(not_run) => {
            return Outcome {
                tool,
                argv,
                version: None,
                image: None,
                status: Status::from(not_run),
            };
        }
    };
    let version = query_version(&program, root);
    let status = if let Err(e) = clear(root, tool) {
        Status::failure(format!("cannot remove an earlier report: {e}"))
    } else {
        let mut command = Command::new(&program);
        command.args(argv.iter().skip(1)).current_dir(root);
        let ran = ended(
            supervise(&mut command, stdout.as_deref(), None),
            &program.display().to_string(),
            tool.findings,
            Duration::ZERO,
        );
        settle(root, tool, version.as_deref().unwrap_or("unknown"), ran)
    };
    Outcome {
        tool,
        argv,
        version,
        image: None,
        status,
    }
}

/// The first non-empty line `program --version` prints, if it succeeds.
fn query_version(program: &Path, root: &Path) -> Option<String> {
    let output = Command::new(program)
        .arg("--version")
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

#[cfg(test)]
mod tests;
