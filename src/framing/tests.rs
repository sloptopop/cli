//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.

use super::*;

fn read(input: &[u8]) -> Result<Option<Vec<u8>>, FrameError> {
    read_frame(&mut &input[..])
}

#[test]
fn reads_a_frame_and_ignores_other_headers() {
    let input = b"content-length: 2\r\nContent-Type: x\r\n\r\n{}";
    assert_eq!(read(input).ok().flatten(), Some(b"{}".to_vec()), "body");
}

#[test]
fn reads_consecutive_frames() {
    let mut input = &b"Content-Length: 1\r\n\r\naContent-Length: 1\r\n\r\nb"[..];
    assert_eq!(
        read_frame(&mut input).ok().flatten(),
        Some(b"a".to_vec()),
        "first"
    );
    assert_eq!(
        read_frame(&mut input).ok().flatten(),
        Some(b"b".to_vec()),
        "second"
    );
    assert!(matches!(read_frame(&mut input), Ok(None)), "then clean EOF");
}

#[test]
fn clean_eof_is_none() {
    assert!(matches!(read(b""), Ok(None)), "empty stream");
}

#[test]
fn eof_inside_headers_is_an_error() {
    assert!(
        matches!(read(b"Content-Length: 2\r\n"), Err(FrameError::Io(e)) if e.kind() == io::ErrorKind::UnexpectedEof),
        "EOF after a header"
    );
}

#[test]
fn eof_inside_body_is_an_error() {
    assert!(
        matches!(read(b"Content-Length: 5\r\n\r\nab"), Err(FrameError::Io(_))),
        "short body"
    );
}

#[test]
fn bare_newline_is_rejected() {
    assert!(
        matches!(
            read(b"Content-Length: 2\n\n{}"),
            Err(FrameError::InvalidHeader(_))
        ),
        "LF without CR"
    );
}

#[test]
fn overlong_header_line_is_rejected() {
    let long = vec![b'x'; 2048];
    assert!(
        matches!(read(&long), Err(FrameError::InvalidHeader(_))),
        "header longer than the limit"
    );
}

#[test]
fn header_without_colon_is_rejected() {
    assert!(
        matches!(read(b"nonsense\r\n\r\n"), Err(FrameError::InvalidHeader(l)) if l == "nonsense"),
        "no colon"
    );
}

#[test]
fn missing_or_bad_length_is_rejected() {
    assert!(
        matches!(
            read(b"Content-Type: x\r\n\r\n"),
            Err(FrameError::MissingContentLength)
        ),
        "no Content-Length"
    );
    assert!(
        matches!(
            read(b"Content-Length: -1\r\n\r\n"),
            Err(FrameError::MissingContentLength)
        ),
        "unparsable Content-Length"
    );
}

#[test]
fn oversized_body_is_rejected_before_allocating() {
    let input = format!("Content-Length: {}\r\n\r\n", MAX_CONTENT_LENGTH + 1);
    assert!(
        matches!(read(input.as_bytes()), Err(FrameError::TooLarge(n)) if n == MAX_CONTENT_LENGTH + 1),
        "too large"
    );
}

#[test]
fn errors_describe_themselves() {
    let cases = [
        (
            FrameError::Io(io::ErrorKind::UnexpectedEof.into()),
            "i/o error",
        ),
        (
            FrameError::InvalidHeader("h".into()),
            "invalid header line: \"h\"",
        ),
        (
            FrameError::MissingContentLength,
            "missing or invalid Content-Length",
        ),
        (FrameError::TooLarge(9), "Content-Length 9 exceeds"),
    ];
    for (error, expected) in cases {
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn writes_a_frame() {
    let mut out = Vec::new();
    assert!(write_frame(&mut out, b"{}").is_ok(), "write succeeds");
    assert_eq!(out, b"Content-Length: 2\r\n\r\n{}", "framed bytes");
}

/// A reader that fails, to cover read errors.
struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::ConnectionReset.into())
    }
}

#[test]
fn read_errors_are_reported() {
    let mut reader = io::BufReader::new(FailingReader);
    assert!(
        matches!(read_frame(&mut reader), Err(FrameError::Io(e)) if e.kind() == io::ErrorKind::ConnectionReset),
        "read error"
    );
}

/// A writer that accepts `budget` bytes, then fails.
struct FailAfter {
    budget: usize,
    flush_fails: bool,
}

impl Write for FailAfter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.budget {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.budget -= buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.flush_fails {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn write_errors_are_reported_at_every_step() {
    let header = b"Content-Length: 2\r\n\r\n".len();
    let cases = [
        (0, false, "header write fails"),
        (header, false, "body write fails"),
        (header + 2, true, "flush fails"),
    ];
    for (budget, flush_fails, what) in cases {
        let mut writer = FailAfter {
            budget,
            flush_fails,
        };
        assert!(write_frame(&mut writer, b"{}").is_err(), "{what}");
    }
    let mut ok = FailAfter {
        budget: header + 2,
        flush_fails: false,
    };
    assert!(write_frame(&mut ok, b"{}").is_ok(), "fits exactly");
}

#[test]
fn body_limit_is_64_mib() {
    assert_eq!(MAX_CONTENT_LENGTH, 67_108_864, "documented limit");
}

#[test]
fn body_of_exactly_the_limit_is_allowed() {
    let input = format!("Content-Length: {MAX_CONTENT_LENGTH}\r\n\r\n");
    assert!(
        matches!(read(input.as_bytes()), Err(FrameError::Io(e)) if e.kind() == io::ErrorKind::UnexpectedEof),
        "at the limit the body is read (and here runs short), not rejected"
    );
}
