//! The one network call of `sloptopop transform`: the request file posted
//! to the backend's `/v1/transform` with the system's `curl` (no TLS stack
//! of our own, and the exact command is visible), the answer written to
//! the response file. Nothing else is ever sent: no telemetry, no update
//! check, a generic user agent (decision D-0179).
//!
//! Standards: SCF NET-14 (encrypted transmission: https, or http inside the
//! customer's own network), PRI-05; OWASP ASVS 5.0 V12.1.1 (TLS for data
//! in transit); GDPR Art. 5(1)(c).

use std::path::Path;
use std::process::{Command, Stdio};

/// The user agent of the request: the tool and its version, nothing else.
pub(crate) const USER_AGENT: &str = concat!("sloptopop/", env!("CARGO_PKG_VERSION"));

/// The transform route under a backend's base URL.
pub(crate) fn endpoint(backend: &str) -> String {
    format!("{}/v1/transform", backend.trim_end_matches('/'))
}

/// Posts `request` to `url` with `curl`, writing the answer to
/// `response`.
///
/// # Errors
///
/// Why it failed: curl could not run, could not connect, or the backend
/// answered with an error (whose body is quoted).
pub(crate) fn post(curl: &Path, url: &str, request: &Path, response: &Path) -> Result<(), String> {
    let data = format!("@{}", request.display());
    let output = Command::new(curl)
        .args([
            "--silent",
            "--show-error",
            "--fail-with-body",
            "--proto",
            "=https,http",
            "--header",
            "Content-Type: application/json",
            "--user-agent",
            USER_AGENT,
            "--data-binary",
            &data,
            "--output",
        ])
        .arg(response)
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run {}: {error}", curl.display()))?;
    if output.status.success() {
        return Ok(());
    }
    let body = std::fs::read_to_string(response).unwrap_or_default();
    let answer = if body.trim().is_empty() {
        String::new()
    } else {
        format!(" ({})", body.trim())
    };
    Err(format!(
        "the backend at {url} did not answer: {}{answer}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

#[cfg(test)]
mod tests;
