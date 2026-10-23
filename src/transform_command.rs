//! `sloptopop transform` (preview): the open-source half of a transform.
//! After the exchange with the server (`server_command`: the masked
//! snapshot prepared by `sloptopop-local request`, the manifest shown and
//! confirmed, the request posted with `curl`), `sloptopop-local apply`
//! checks the answer, delivers the verified steps (on a new branch with
//! `--apply`, recording `--confirm --by` first) and writes the report;
//! this command prints `SUMMARY.md` (or, with `--format json`,
//! `report.json`).
//!
//! Exit code: 0 every automated step delivered (or, in a dry run,
//! deliverable); 1 a step is pending a confirmation, refused or blocked;
//! 3 `sloptopop-local` is missing or failed; 4 the upload was not
//! confirmed, `curl` is missing, or the server could not be reached or
//! refused it.
//!
//! Standards: SCF PRI-02 (notice before upload), CHG-03 (changes only on
//! a new branch), TDA-12.1; GDPR Art. 25; decisions D-0008, D-0174,
//! D-0179, D-0183.

use std::fs;
use std::io::{BufRead, Write};

use crate::args::{Format, TransformOptions};
use crate::command_io::write_output;
use crate::engine_locate::Locator;
use crate::engine_run::invoke;
use crate::server_command::{Asked, exchange};

/// Runs `sloptopop transform` as `options` ask.
pub(crate) fn transform(
    locator: &Locator,
    options: &TransformOptions,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let asked = Asked {
        root: &options.root,
        mode: "transform",
        backend: options.backend.as_deref(),
        yes: options.yes,
    };
    let answered = match exchange(locator, asked, stdin, stderr) {
        Ok(answered) => answered,
        Err(failure) => return failure,
    };
    let confirm = options.confirm.join(",");
    let mut args = vec!["apply", &options.root, "--run", &answered.run];
    if options.apply {
        args.push("--apply");
    }
    if let Some(branch) = &options.branch {
        args.extend(["--branch", branch]);
    }
    if let Some(by) = &options.by {
        args.extend(["--confirm", &confirm, "--by", by]);
    }
    let (output, code) = match invoke(locator, &args, stderr) {
        Ok(result) => result,
        Err(failure) => return failure,
    };
    let printed = match options.format {
        Format::Json => output.stdout,
        Format::Text => {
            let summary = answered.response.with_file_name("SUMMARY.md");
            fs::read(summary).unwrap_or(output.stdout)
        }
    };
    write_output(stdout, stderr, &printed, code)
}

#[cfg(test)]
mod tests;
