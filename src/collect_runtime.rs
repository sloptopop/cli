//! The container runtime `collect` runs the analyzers in: which one is
//! there (Docker Engine, rootless or not, through Docker Desktop, Colima or
//! Rancher Desktop; or Podman, rootless or not), how to start its CLI, and
//! the flags that make the reports belong to the user. Nothing here falls
//! back to running a tool from `PATH`.
//!
//! Standards: SCF CFG-03 (Least Functionality: only the named runtime CLI
//! is started), TDA-12.1 (Error Handling: a missing runtime is a clear,
//! actionable error, never a silent fallback).

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::collect_lookup::find;

/// A container runtime CLI `collect` knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Engine {
    /// `docker`: Docker Engine, Docker Desktop, Colima, Rancher Desktop.
    Docker,
    /// `podman`.
    Podman,
}

impl Engine {
    /// The CLI's name, as `--runtime` takes it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }

    /// The engine `name` stands for, if any.
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "docker" => Some(Self::Docker),
            "podman" => Some(Self::Podman),
            _ => None,
        }
    }

    /// The arguments that print `<version>|<rootless>|<limits>`, where the
    /// last two are whatever the engine reports about them.
    fn probe(self) -> &'static [&'static str] {
        match self {
            Self::Docker => &[
                "info",
                "--format",
                "{{.ServerVersion}}|{{json .SecurityOptions}}|true",
            ],
            Self::Podman => &[
                "info",
                "--format",
                "{{.Version.Version}}|{{.Host.Security.Rootless}}|{{json .Host.CgroupControllers}}",
            ],
        }
    }
}

/// A runtime that answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Runtime {
    /// Which CLI.
    pub(crate) engine: Engine,
    /// The CLI's executable.
    pub(crate) program: PathBuf,
    /// The server's version.
    pub(crate) version: String,
    /// Whether containers run in the user's own user namespace.
    pub(crate) rootless: bool,
    /// Whether memory, CPU and process limits can be applied.
    pub(crate) limits: bool,
}

impl Runtime {
    /// Runs the CLI with `args` in `cwd`, with no input, and returns what
    /// it printed.
    pub(crate) fn output<S: AsRef<OsStr>>(&self, args: &[S], cwd: &Path) -> io::Result<Output> {
        Command::new(&self.program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .output()
    }

    /// The flags that run a container as the owner of the repository
    /// (`owner`, a `uid:gid`, where the platform has one), so the reports
    /// it writes belong to the user: rootless Docker maps the container's
    /// root to the user, rootless Podman keeps the user's own ids.
    pub(crate) fn user(&self, owner: Option<&str>) -> Vec<String> {
        match (self.engine, self.rootless, owner) {
            (Engine::Docker, true, _) => vec!["--user".to_owned(), "0:0".to_owned()],
            (Engine::Podman, true, Some(owner)) => ["--userns", "keep-id", "--user", owner]
                .map(str::to_owned)
                .into(),
            (Engine::Podman, true, None) => vec!["--userns".to_owned(), "keep-id".to_owned()],
            (_, false, Some(owner)) => vec!["--user".to_owned(), owner.to_owned()],
            (_, false, None) => Vec::new(),
        }
    }
}

/// Finds a runtime: `wanted` only, if given, otherwise `docker`, then
/// `podman`, on `path`. The error says what was tried and why each failed.
pub(crate) fn detect(
    path: Option<&OsStr>,
    wanted: Option<Engine>,
    cwd: &Path,
) -> Result<Runtime, String> {
    let candidates: Vec<Engine> = match wanted {
        Some(engine) => vec![engine],
        None => vec![Engine::Docker, Engine::Podman],
    };
    let mut tried = Vec::new();
    for engine in candidates {
        match probe(path, engine, cwd) {
            Ok(runtime) => return Ok(runtime),
            Err(reason) => tried.push(format!("{} ({reason})", engine.name())),
        }
    }
    Err(tried.join(", "))
}

fn probe(path: Option<&OsStr>, engine: Engine, cwd: &Path) -> Result<Runtime, String> {
    let program = find(path, engine.name()).ok_or_else(|| "not on PATH".to_owned())?;
    let failed = |detail: String| format!("on PATH, but `{} info` failed: {detail}", engine.name());
    let output = Command::new(&program)
        .args(engine.probe())
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| failed(e.to_string()))?;
    if !output.status.success() {
        return Err(failed(reason(&output)));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut fields = text.trim().splitn(3, '|');
    let version = fields.next().unwrap_or_default().trim().to_owned();
    let rootless = fields.next().unwrap_or_default();
    let limits = fields.next().unwrap_or_default();
    if version.is_empty() {
        return Err(failed("it printed no server version".to_owned()));
    }
    Ok(Runtime {
        engine,
        program,
        version,
        rootless: rootless.contains("rootless") || rootless.trim() == "true",
        limits: limits.trim() == "true" || (limits.contains("memory") && limits.contains("pids")),
    })
}

/// The first non-empty line of `bytes`, trimmed.
/// Why a runtime command failed: the first line it printed on stderr, or
/// else its exit status.
pub(crate) fn reason(output: &Output) -> String {
    first_line(&output.stderr).unwrap_or_else(|| output.status.to_string())
}

pub(crate) fn first_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests;
