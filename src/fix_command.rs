//! `sloptopop fix`: runs `sloptopop-engine fix <path> [--apply]` and
//! prints what was, or would be, created.

use std::io::Write;

use crate::command_io::{parse_document, write_output};
use crate::engine_locate::Locator;
use crate::engine_run::invoke;
use crate::render_fix::fix as render;

/// Runs `sloptopop-engine fix <path> [--apply]` and prints the rendered
/// result. Exits like the engine or with
/// [`crate::engine_run::ENGINE_ERROR`].
pub(crate) fn fix(
    locator: &Locator,
    path: &str,
    apply: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let args: &[&str] = if apply {
        &["fix", path, "--apply"]
    } else {
        &["fix", path]
    };
    let (output, code) = match invoke(locator, args, stderr) {
        Ok(result) => result,
        Err(failure) => return failure,
    };
    match parse_document(&output.stdout, stderr) {
        Ok(result) => write_output(stdout, stderr, render(&result).as_bytes(), code),
        Err(failure) => failure,
    }
}
