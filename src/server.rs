//! The JSON-RPC session: reads frames, dispatches methods, writes replies.
//!
//! Lifecycle (same as LSP): `initialize` → any requests → `shutdown` →
//! `exit`. Requests before `initialize` get `SERVER_NOT_INITIALIZED`;
//! requests after `shutdown` get `INVALID_REQUEST`. The process exits with
//! 0 after `exit` if `shutdown` came first, otherwise 1. Notifications other
//! than `exit` are ignored. Batches are not supported.
//!
//! Standards: SCF NET-35 (Application Programming Interface Security),
//! TDA-12.1 (Error Handling).

use std::io::{BufRead, Write};

use crate::framing::{self, FrameError};
use crate::session::Session;

/// Runs a session until `exit` or end of input and returns the process
/// exit code: 0 after `shutdown` + `exit`, 1 if the client exited or
/// disconnected without `shutdown`.
///
/// # Errors
///
/// Returns a [`FrameError`] if the input is not validly framed or reading
/// or writing fails; the stream can't be resynchronised after that.
pub fn serve(input: &mut dyn BufRead, output: &mut dyn Write) -> Result<u8, FrameError> {
    let mut session = Session::new();
    while let Some(body) = framing::read_frame(input)? {
        let step = session.handle(&body);
        if let Some(reply) = step.reply {
            framing::write_frame(output, reply.to_string().as_bytes())?;
        }
        if let Some(code) = step.exit_code {
            return Ok(code);
        }
    }
    Ok(session.exit_code())
}

#[cfg(test)]
mod tests;
