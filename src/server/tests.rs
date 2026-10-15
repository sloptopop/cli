//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use serde_json::{Value, json};

use super::*;
use crate::tests::support::initialize;

fn frame(message: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    let _written = framing::write_frame(&mut out, message.to_string().as_bytes());
    out
}

#[test]
fn serve_runs_a_session_over_frames() {
    let mut input = Vec::new();
    input.extend(frame(&initialize(1)));
    input.extend(frame(
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
    ));
    input.extend(frame(&json!({ "jsonrpc": "2.0", "method": "exit" })));
    let mut output = Vec::new();
    let code = serve(&mut &input[..], &mut output);
    assert!(matches!(code, Ok(0)), "clean exit");
    let text = String::from_utf8_lossy(&output);
    assert_eq!(text.matches("Content-Length").count(), 2, "two replies");
}

#[test]
fn serve_reports_disconnect_without_shutdown() {
    let input = frame(&initialize(1));
    let mut output = Vec::new();
    assert!(
        matches!(serve(&mut &input[..], &mut output), Ok(1)),
        "unclean"
    );
    let mut empty = Vec::new();
    assert!(
        matches!(serve(&mut &b""[..], &mut empty), Ok(1)),
        "no session at all"
    );
}

#[test]
fn serve_stops_on_framing_errors() {
    let mut output = Vec::new();
    assert!(
        matches!(
            serve(&mut &b"garbage\r\n\r\n"[..], &mut output),
            Err(FrameError::InvalidHeader(_))
        ),
        "framing error"
    );
}

/// A writer that always fails, to cover reply-write errors.
struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn serve_stops_when_replies_cannot_be_written() {
    let input = frame(&initialize(1));
    assert!(
        matches!(serve(&mut &input[..], &mut Broken), Err(FrameError::Io(_))),
        "write error"
    );
}
