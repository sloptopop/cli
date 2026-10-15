//! Running `sloptopop-engine` as a child process.
//!
//! `check` and `fix` never link the engine: they start it and read the
//! JSON document it prints (see `docs/report-format.md`).

use std::io::Write;
use std::process::{Command, Output, Stdio};

use crate::diagnostic::report;
use crate::engine_locate::{ENGINE_NAME, Locator};

/// Exit code when the engine is missing, fails, or its output is unusable.
pub const ENGINE_ERROR: u8 = 3;

/// Printed when the engine cannot be found.
pub const NOT_FOUND: &str = "\
error: sloptopop-engine not found
hint: install sloptopop-engine next to the sloptopop executable or on your PATH,
      or set SLOPTOPOP_ENGINE to its full path
";

/// Runs the engine with `args`, forwards what it writes to stderr, and
/// returns its output if it exited with 0 or 1 (its two normal outcomes).
/// Otherwise reports the problem on `stderr` and returns the exit code
/// to use.
pub(crate) fn invoke(
    locator: &Locator,
    args: &[&str],
    stderr: &mut dyn Write,
) -> Result<(Output, u8), u8> {
    let Some(engine) = locator.locate() else {
        return Err(report(stderr, NOT_FOUND, ENGINE_ERROR));
    };
    let output = Command::new(&engine)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            report(
                stderr,
                &format!("error: cannot run {}: {e}\n", engine.display()),
                ENGINE_ERROR,
            )
        })?;
    let _forwarded = stderr.write_all(&output.stderr);
    match output.status.code() {
        Some(code @ (0 | 1)) => Ok((output, u8::from(code == 1))),
        _ => Err(report(
            stderr,
            &format!("error: {ENGINE_NAME} failed ({})\n", output.status),
            ENGINE_ERROR,
        )),
    }
}

#[cfg(test)]
mod tests;
