//! Command-line dispatch, kept free of process globals so every path can
//! be tested with in-memory streams.
//!
//! Standards: SCF TDA-12 (Input Data Validation), TDA-12.1 (Error
//! Handling: fixed exit codes, no process globals, never a panic).

use std::ffi::OsString;
use std::io::{self, BufRead, Write};

use crate::args::{Command, USAGE, parse};
use crate::check_command::check;
use crate::collect_command::collect;
use crate::diagnostic::report;
use crate::engine_locate::Locator;
use crate::fix_command::fix;
use crate::handshake::{PROTOCOL_VERSION, SERVER_VERSION};
use crate::schema;
use crate::server;
use crate::transform_command::transform;

/// Exit code for a usage error, matching common CLI convention.
pub const USAGE_ERROR: u8 = 2;

/// Runs the CLI with explicit streams and returns the process exit code.
/// `check` and `fix` look for the engine, and `collect` for the analyzers,
/// in this process's environment.
pub fn run(
    args: &[OsString],
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    run_with(args, &Locator::from_process(), stdin, stdout, stderr)
}

/// [`run`], with the engine looked up through `locator`; `collect` looks
/// for the analyzers on `locator.path`.
pub fn run_with(
    args: &[OsString],
    locator: &Locator,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let command = match parse(args) {
        Ok(command) => command,
        Err(message) => return usage_error(stderr, &message),
    };
    let written = match command {
        Command::Check(path, format) => return check(locator, &path, format, stdout, stderr),
        Command::Fix(path, apply) => return fix(locator, &path, apply, stdout, stderr),
        Command::Transform(options) => return transform(locator, &options, stdin, stdout, stderr),
        Command::Collect(options) => {
            return collect(&options, locator.path.as_deref(), stdout, stderr)
                .unwrap_or_else(|message| usage_error(stderr, &message));
        }
        Command::Serve => {
            return match server::serve(stdin, stdout) {
                Ok(code) => code,
                Err(e) => report(stderr, &format!("error: {e}\n"), 1),
            };
        }
        Command::Schema => writeln!(stdout, "{:#}", schema::document()),
        Command::Version => writeln!(
            stdout,
            "sloptopop {SERVER_VERSION} (protocol {PROTOCOL_VERSION})"
        ),
        Command::Help => stdout.write_all(USAGE.as_bytes()),
    };
    exit_code(&written.and_then(|()| stdout.flush()))
}

fn usage_error(stderr: &mut dyn Write, message: &str) -> u8 {
    report(stderr, &format!("error: {message}\n\n{USAGE}"), USAGE_ERROR)
}

fn exit_code(result: &io::Result<()>) -> u8 {
    u8::from(result.is_err())
}

#[cfg(test)]
mod tests;
