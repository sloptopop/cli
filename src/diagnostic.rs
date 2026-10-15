//! Reporting a failure on stderr and turning it into an exit code.

use std::io::Write;

/// Writes `message` to stderr and returns `code`. If stderr itself is
/// broken there is nowhere left to report to; the exit code still says it.
pub(crate) fn report(stderr: &mut dyn Write, message: &str, code: u8) -> u8 {
    let _ignored = stderr.write_all(message.as_bytes());
    code
}
