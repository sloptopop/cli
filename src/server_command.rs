//! The one exchange with the sloptopop server that `check`, `fix` and
//! `transform` share (decision D-0183: sloptopop detects only on its
//! server, `sloptopop-engine`, hosted or self-hosted):
//!
//! 1. `sloptopop-local request <root> --mode <mode>` checks the
//!    repository, masks every secret value the secret scanner reported and
//!    writes the request and its manifest;
//! 2. this module shows the manifest and asks before anything is sent
//!    (`--yes` skips the question, for CI);
//! 3. it posts the request to the server with `curl` (the only network
//!    call sloptopop makes, and the only code that makes one is this open
//!    code) and the answer is written next to the request.
//!
//! Without a server there is no check: when none answers, the error says
//! so and names the hosted service and the self-hosted option.
//!
//! Standards: SCF PRI-02 (notice before upload), TDA-12.1; GDPR Art. 25;
//! decisions D-0008, D-0174, D-0179, D-0183.

use std::io::{BufRead, Write};
use std::path::Path;

use serde_json::Value;

use crate::backend_post::{endpoint, post};
use crate::collect_lookup::find;
use crate::command_io::parse_document;
use crate::diagnostic::report;
use crate::engine_locate::Locator;
use crate::engine_run::invoke;
use crate::json_fields::text;
use crate::render_manifest::manifest;

/// Exit code when the upload was not confirmed or did not go through.
pub(crate) const UPLOAD_ERROR: u8 = 4;

/// The hosted server (to be confirmed with the hosting decision, D-0173).
pub(crate) const DEFAULT_BACKEND: &str = "https://api.sloptopop.dev";

/// What to send.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Asked<'a> {
    /// The repository root.
    pub(crate) root: &'a str,
    /// `check`, `fix` or `transform`.
    pub(crate) mode: &'a str,
    /// The server's base URL (default: the hosted one).
    pub(crate) backend: Option<&'a str>,
    /// Upload without asking.
    pub(crate) yes: bool,
}

/// The run the server answered: its id and the response's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answered {
    /// The run id.
    pub(crate) run: String,
    /// Where the response is.
    pub(crate) response: std::path::PathBuf,
}

/// The hint every failed exchange ends with.
fn hint() -> String {
    format!(
        "hint: sloptopop detects only on its server: use the hosted service ({DEFAULT_BACKEND}, the default) or your self-hosted sloptopop-engine with --backend <url>\n"
    )
}

/// Asks on `stderr` whether to upload; whether `stdin` answered yes.
fn confirmed(stdin: &mut dyn BufRead, stderr: &mut dyn Write) -> bool {
    let _asked = stderr
        .write_all(b"Upload? [y/N] ")
        .and_then(|()| stderr.flush());
    let mut answer = String::new();
    let _read = stdin.read_line(&mut answer);
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Prepares the request `asked` describes, shows its manifest, asks, and
/// posts it; the run the server answered, or the exit code to stop with.
pub(crate) fn exchange(
    locator: &Locator,
    asked: Asked<'_>,
    stdin: &mut dyn BufRead,
    stderr: &mut dyn Write,
) -> Result<Answered, u8> {
    let (prepared, _) = invoke(
        locator,
        &["request", asked.root, "--mode", asked.mode],
        stderr,
    )?;
    let document = parse_document(&prepared.stdout, stderr)?;
    let url = endpoint(asked.backend.unwrap_or(DEFAULT_BACKEND));
    let root = Path::new(asked.root);
    let request = root.join(text(&document, "request"));
    let response = root.join(text(&document, "response"));
    let _shown = stderr
        .write_all(manifest(document.get("manifest").unwrap_or(&Value::Null), &url).as_bytes());
    if !asked.yes && !confirmed(stdin, stderr) {
        return Err(report(
            stderr,
            "error: the upload was not confirmed (pass --yes to upload without asking)\n",
            UPLOAD_ERROR,
        ));
    }
    let Some(curl) = find(locator.path.as_deref(), "curl") else {
        return Err(report(
            stderr,
            "error: curl not found on PATH: sloptopop sends its request with curl\n",
            UPLOAD_ERROR,
        ));
    };
    if let Err(why) = post(&curl, &url, &request, &response) {
        return Err(report(
            stderr,
            &format!("error: {why}\n{}", hint()),
            UPLOAD_ERROR,
        ));
    }
    Ok(Answered {
        run: text(&document, "run").to_owned(),
        response,
    })
}

#[cfg(test)]
mod tests;
