//! Output handling shared by the commands that run `sloptopop-local`:
//! reading the JSON document it printed and writing the result to stdout.
//!
//! Standards: SCF TDA-12 (Input Data Validation: its output must be
//! JSON), TDA-12.1 (Error Handling).

use std::io::Write;

use serde_json::Value;

use crate::diagnostic::report;
use crate::engine_locate::ENGINE_NAME;
use crate::engine_run::ENGINE_ERROR;

/// Parses `sloptopop-local`'s stdout as JSON; if it isn't, reports that and
/// returns [`ENGINE_ERROR`].
pub(crate) fn parse_document(bytes: &[u8], stderr: &mut dyn Write) -> Result<Value, u8> {
    serde_json::from_slice(bytes).map_err(|e| {
        report(
            stderr,
            &format!("error: {ENGINE_NAME} printed invalid JSON: {e}\n"),
            ENGINE_ERROR,
        )
    })
}

/// Writes `bytes` to stdout and returns `code`, or [`ENGINE_ERROR`] if
/// stdout is broken.
pub(crate) fn write_output(
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    bytes: &[u8],
    code: u8,
) -> u8 {
    match stdout.write_all(bytes).and_then(|()| stdout.flush()) {
        Ok(()) => code,
        Err(e) => report(
            stderr,
            &format!("error: cannot write output: {e}\n"),
            ENGINE_ERROR,
        ),
    }
}
