//! Finding `sloptopop-engine`: through the `SLOPTOPOP_ENGINE` environment
//! variable, then next to the `sloptopop` executable, then on `PATH`.
//!
//! Standards: SCF CFG-03 (Least Functionality: only the binary named
//! `sloptopop-engine` is started, from three documented places in a
//! fixed order).

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// File name of the engine binary (without the platform's suffix).
pub const ENGINE_NAME: &str = "sloptopop-engine";

/// Environment variable naming the engine binary explicitly.
pub const ENGINE_ENV: &str = "SLOPTOPOP_ENGINE";

/// Where to look for the engine, captured once so lookups are testable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Locator {
    /// The value of `SLOPTOPOP_ENGINE`, if set and not empty.
    pub explicit: Option<OsString>,
    /// The directory holding the running `sloptopop` executable.
    pub exe_dir: Option<PathBuf>,
    /// The value of `PATH`.
    pub path: Option<OsString>,
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
        }
    }

    /// The engine to run: `SLOPTOPOP_ENGINE` as given, otherwise the first
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
