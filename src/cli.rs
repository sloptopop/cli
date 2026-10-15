//! Command-line dispatch, kept free of process globals so every path can
//! be tested with in-memory streams.

use std::ffi::OsString;
use std::io::{self, BufRead, Write};

use crate::args::{Command, USAGE, parse};
use crate::check_command::check;
use crate::diagnostic::report;
use crate::engine_locate::Locator;
use crate::fix_command::fix;
use crate::handshake::{PROTOCOL_VERSION, SERVER_VERSION};
use crate::schema;
use crate::server;

/// Exit code for a usage error, matching common CLI convention.
pub const USAGE_ERROR: u8 = 2;

/// Runs the CLI with explicit streams and returns the process exit code.
/// `check` and `fix` look for the engine in this process's environment.
pub fn run(
    args: &[OsString],
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    run_with(args, &Locator::from_process(), stdin, stdout, stderr)
}

/// [`run`], with the engine looked up through `locator`.
pub fn run_with(
    args: &[OsString],
    locator: &Locator,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let command = match parse(args) {
        Ok(command) => command,
        Err(message) => {
            return report(stderr, &format!("error: {message}\n\n{USAGE}"), USAGE_ERROR);
        }
    };
    let written = match command {
        Command::Check(path, format) => return check(locator, &path, format, stdout, stderr),
        Command::Fix(path, apply) => return fix(locator, &path, apply, stdout, stderr),
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

fn exit_code(result: &io::Result<()>) -> u8 {
    u8::from(result.is_err())
}

#[cfg(test)]
mod tests;
