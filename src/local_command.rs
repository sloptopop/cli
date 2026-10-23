//! `sloptopop local <arguments>`: runs `sloptopop-local` with the
//! arguments as they are, its output passed through and its exit code
//! kept (decision D-0183: the cli dispatches `local` subcommands to
//! `sloptopop local`, and everything else to the server).
//!
//! Standards: SCF CFG-03 (Least Functionality: one documented binary),
//! TDA-12.1 (a missing or killed `sloptopop-local` exits 3).

use std::io::Write;
use std::process::{Command, Stdio};

use crate::diagnostic::report;
use crate::engine_locate::Locator;
use crate::engine_run::{ENGINE_ERROR, NOT_FOUND};

/// Runs `sloptopop-local` with `args`; its exit code.
pub(crate) fn local(
    locator: &Locator,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let Some(binary) = locator.locate() else {
        return report(stderr, NOT_FOUND, ENGINE_ERROR);
    };
    match Command::new(&binary)
        .args(args)
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => {
            let _out = stdout
                .write_all(&output.stdout)
                .and_then(|()| stdout.flush());
            let _err = stderr.write_all(&output.stderr);
            output
                .status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(ENGINE_ERROR)
        }
        Err(e) => report(
            stderr,
            &format!("error: cannot run {}: {e}\n", binary.display()),
            ENGINE_ERROR,
        ),
    }
}

#[cfg(test)]
mod tests;
