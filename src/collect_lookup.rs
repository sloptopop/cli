//! Finding an installed tool on `PATH`. Nothing is ever installed or
//! downloaded: a tool that is not there is reported as missing.
//!
//! Standards: SCF CFG-03 (Least Functionality: nothing is installed
//! or downloaded).

use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;

/// The first file named `command` (with the platform's executable suffix)
/// in a directory of `path`, the value of `PATH`.
pub(crate) fn find(path: Option<&OsStr>, command: &str) -> Option<PathBuf> {
    let file = format!("{command}{}", env::consts::EXE_SUFFIX);
    path.into_iter()
        .flat_map(env::split_paths)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(&file))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests;
