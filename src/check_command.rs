//! `sloptopop check`: runs `sloptopop-engine check <path>` and prints its
//! report.

use std::io::Write;

use crate::args::Format;
use crate::command_io::{parse_document, write_output};
use crate::engine_locate::Locator;
use crate::engine_run::invoke;
use crate::render_report::report;

/// Runs `sloptopop-engine check <path>` and prints its report as JSON
/// (unchanged) or as text. Exits like the engine (0 passed, 1 not passed)
/// or with [`crate::engine_run::ENGINE_ERROR`].
pub(crate) fn check(
    locator: &Locator,
    path: &str,
    format: Format,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let (output, code) = match invoke(locator, &["check", path], stderr) {
        Ok(result) => result,
        Err(failure) => return failure,
    };
    let printed = match format {
        Format::Json => output.stdout,
        Format::Text => match parse_document(&output.stdout, stderr) {
            Ok(document) => report(&document).into_bytes(),
            Err(failure) => return failure,
        },
    };
    write_output(stdout, stderr, &printed, code)
}
