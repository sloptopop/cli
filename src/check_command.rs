//! `sloptopop check`: the server's Report of a repository. `sloptopop
//! local` prepares the masked snapshot, the server detects (there is no
//! detection without it, D-0183), and `sloptopop-local report` prints the
//! answer.
//!
//! Standards: SCF TDA-17.1 (Static Code Analysis, by the server),
//! TDA-12.1 (a `sloptopop local` that fails or prints no JSON exits 3).

use std::io::{BufRead, Write};

use crate::args::{CheckOptions, Format};
use crate::command_io::{parse_document, write_output};
use crate::engine_locate::Locator;
use crate::engine_run::invoke;
use crate::render_report::report;
use crate::server_command::{Asked, exchange};

/// Runs `sloptopop check` as `options` ask and prints the Report as JSON
/// (unchanged) or as text. Exits 0 passed, 1 not passed, 3 when `sloptopop
/// local` fails, 4 when the upload did not go through.
pub(crate) fn check(
    locator: &Locator,
    options: &CheckOptions,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let asked = Asked {
        root: &options.root,
        mode: "check",
        backend: options.backend.as_deref(),
        yes: options.yes,
        history: options.history,
    };
    let answered = match exchange(locator, asked, stdin, stderr) {
        Ok(answered) => answered,
        Err(failure) => return failure,
    };
    let (output, code) = match invoke(
        locator,
        &["report", &options.root, "--run", &answered.run],
        stderr,
    ) {
        Ok(result) => result,
        Err(failure) => return failure,
    };
    let printed = match options.format {
        Format::Json => output.stdout,
        Format::Text => match parse_document(&output.stdout, stderr) {
            Ok(document) => report(&document).into_bytes(),
            Err(failure) => return failure,
        },
    };
    write_output(stdout, stderr, &printed, code)
}
