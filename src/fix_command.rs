//! `sloptopop fix`: the files the server's mechanical fixes create (a
//! toolchain pin, the pre-push hook, the CI gate, a `.gitignore`), as one
//! commit each on a new branch with `--apply`; a dry run otherwise. The
//! server detects and returns the fixes; `sloptopop-local apply` checks
//! and commits them.
//!
//! Standards: SCF VPM-06 (Vulnerability Remediation Process), CHG-03
//! (Configuration Change Control: a dry run unless `--apply`, changes only
//! on a new branch).

use std::fs;
use std::io::{BufRead, Write};

use crate::args::FixOptions;
use crate::command_io::write_output;
use crate::engine_locate::Locator;
use crate::engine_run::invoke;
use crate::server_command::{Asked, exchange};

/// Runs `sloptopop fix` as `options` ask and prints the run's summary.
/// Exits 0 when every fix was delivered (or, in a dry run, would be), 1
/// when not, 3 or 4 on a failure.
pub(crate) fn fix(
    locator: &Locator,
    options: &FixOptions,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let asked = Asked {
        root: &options.root,
        mode: "fix",
        backend: options.backend.as_deref(),
        yes: options.yes,
        history: options.history,
    };
    let answered = match exchange(locator, asked, stdin, stderr) {
        Ok(answered) => answered,
        Err(failure) => return failure,
    };
    let mut args = vec!["apply", &options.root, "--run", &answered.run];
    if options.apply {
        args.push("--apply");
    }
    let (output, code) = match invoke(locator, &args, stderr) {
        Ok(result) => result,
        Err(failure) => return failure,
    };
    let summary = answered.response.with_file_name("SUMMARY.md");
    let printed = fs::read(summary).unwrap_or(output.stdout);
    write_output(stdout, stderr, &printed, code)
}
