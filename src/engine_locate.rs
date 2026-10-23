//! Finding `sloptopop-local` (the open-source half of sloptopop on this
//! machine, decision D-0183): through the `SLOPTOPOP_LOCAL` environment
//! variable, then next to the `sloptopop` executable, then on `PATH`.
//!
//! Standards: SCF CFG-03 (Least Functionality: only the binary named
//! `sloptopop-local` is started, from three documented places in a
//! fixed order).

use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// File name of the `sloptopop local` binary (without the platform's
/// suffix).
pub const ENGINE_NAME: &str = "sloptopop-local";

/// Environment variable naming the `sloptopop local` binary explicitly.
pub const ENGINE_ENV: &str = "SLOPTOPOP_LOCAL";

/// The environment variables `collect` reads (besides `PATH`): its own
/// settings, where the user's cache is, and the Go module settings its
/// prepare step passes on.
pub(crate) const COLLECT_VARS: [&str; 14] = [
    "SLOPTOPOP_REGISTRY",
    "SLOPTOPOP_CONTAINER_RUNTIME",
    "SLOPTOPOP_COLLECT_MEMORY",
    "SLOPTOPOP_COLLECT_CPUS",
    "SLOPTOPOP_CACHE_DIR",
    "XDG_CACHE_HOME",
    "HOME",
    "LOCALAPPDATA",
    "GOPROXY",
    "GOPRIVATE",
    "GONOPROXY",
    "GONOSUMDB",
    "GONOSUMCHECK",
    "GOINSECURE",
];

/// How often a queued job is polled at first (it backs off to two and a
/// half times this).
pub(crate) const POLL_EVERY: Duration = Duration::from_secs(2);

/// Where to look for the engine (and what `collect` needs from the
/// environment, and how the server's queue is shown), captured once so
/// lookups are testable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Locator {
    /// The value of `SLOPTOPOP_LOCAL`, if set and not empty.
    pub explicit: Option<OsString>,
    /// The directory holding the running `sloptopop` executable.
    pub exe_dir: Option<PathBuf>,
    /// The value of `PATH`.
    pub path: Option<OsString>,
    /// The set, non-empty environment variables `collect` reads: its own
    /// settings, the cache location and the Go module settings.
    pub vars: Vec<(String, OsString)>,
    /// Whether stderr is a terminal (the queue line is then rewritten in
    /// place).
    pub terminal: bool,
    /// How often a queued job is polled at first (2 s).
    pub poll: Duration,
}

impl Locator {
    /// Captures the locations from the current process.
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            explicit: env::var_os(ENGINE_ENV).filter(|value| !value.is_empty()),
            exe_dir: env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf)),
            path: env::var_os("PATH"),
            vars: COLLECT_VARS
                .iter()
                .filter_map(|name| {
                    env::var_os(name)
                        .filter(|value| !value.is_empty())
                        .map(|value| ((*name).to_owned(), value))
                })
                .collect(),
            terminal: io::stderr().is_terminal(),
            poll: POLL_EVERY,
        }
    }

    /// The value of the captured variable `name`, if set.
    #[must_use]
    pub fn var(&self, name: &str) -> Option<&OsString> {
        self.vars
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    /// `sloptopop-local`: `SLOPTOPOP_LOCAL` as given, otherwise the first
    /// existing file next to the executable or in a `PATH` directory.
    #[must_use]
    pub fn locate(&self) -> Option<PathBuf> {
        if let Some(explicit) = &self.explicit {
            return Some(PathBuf::from(explicit));
        }
        let file = format!("{ENGINE_NAME}{}", env::consts::EXE_SUFFIX);
        self.exe_dir
            .iter()
            .cloned()
            .chain(self.path.iter().flat_map(env::split_paths))
            .map(|dir| dir.join(&file))
            .find(|candidate| candidate.is_file())
    }
}

#[cfg(test)]
mod tests;
