//! LSP-style message framing: `Content-Length: N\r\n\r\n` followed by an
//! N-byte UTF-8 JSON body. Header names are case-insensitive; headers other
//! than `Content-Length` (e.g. `Content-Type`) are accepted and ignored.
//!
//! Standards: SCF NET-35 (Application Programming Interface Security:
//! a frame needs a usable `Content-Length`), TDA-12.

use std::fmt;
use std::io::{self, BufRead, Read, Write};

/// Largest body accepted, so a hostile or broken client can't make the
/// server allocate unbounded memory.
pub const MAX_CONTENT_LENGTH: usize = 64 * 1024 * 1024;

/// Longest header line accepted, for the same reason.
const MAX_HEADER_LINE: u64 = 1024;

/// Why a frame could not be read. Every variant leaves the stream out of
/// sync, so the server stops after reporting it.
#[derive(Debug)]
pub enum FrameError {
    /// Reading from the stream failed, or it ended mid-frame.
    Io(io::Error),
    /// A header line is not `Name: value` terminated by `\r\n`.
    InvalidHeader(String),
    /// The headers ended without a usable `Content-Length`.
    MissingContentLength,
    /// `Content-Length` exceeds [`MAX_CONTENT_LENGTH`].
    TooLarge(usize),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::InvalidHeader(line) => write!(f, "invalid header line: {line:?}"),
            Self::MissingContentLength => f.write_str("missing or invalid Content-Length header"),
            Self::TooLarge(n) => write!(
                f,
                "Content-Length {n} exceeds the {MAX_CONTENT_LENGTH}-byte limit"
            ),
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Reads one frame and returns its body, or `None` on a clean end of
/// stream (EOF before the first header byte).
///
/// # Errors
///
/// Returns a [`FrameError`] if the stream fails, ends mid-frame, or the
/// headers are malformed.
pub fn read_frame(reader: &mut dyn BufRead) -> Result<Option<Vec<u8>>, FrameError> {
    let mut content_length = None;
    let mut first = true;
    loop {
        let mut line = Vec::new();
        let n = (&mut *reader)
            .take(MAX_HEADER_LINE)
            .read_until(b'\n', &mut line)?;
        if n == 0 && first {
            return Ok(None);
        }
        first = false;
        let Some(text) = line.strip_suffix(b"\r\n") else {
            if n == 0 {
                return Err(FrameError::Io(io::ErrorKind::UnexpectedEof.into()));
            }
            return Err(FrameError::InvalidHeader(
                String::from_utf8_lossy(&line).into_owned(),
            ));
        };
        if text.is_empty() {
            break;
        }
        let text = String::from_utf8_lossy(text);
        let Some((name, value)) = text.split_once(':') else {
            return Err(FrameError::InvalidHeader(text.into_owned()));
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    let len = content_length.ok_or(FrameError::MissingContentLength)?;
    if len > MAX_CONTENT_LENGTH {
        return Err(FrameError::TooLarge(len));
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// Writes `body` as one frame and flushes.
///
/// # Errors
///
/// Returns the underlying I/O error if writing or flushing fails.
pub fn write_frame(writer: &mut dyn Write, body: &[u8]) -> io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(body)?;
    writer.flush()
}

#[cfg(test)]
mod tests;
