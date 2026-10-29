//! The one exchange with the sloptopop server that `check`, `fix` and
//! `transform` share (decision D-0183: sloptopop detects only on its
//! server, `sloptopop-engine`, hosted or self-hosted):
//!
//! 1. `sloptopop-local request <root> --mode <mode>` checks the
//!    repository, masks every secret value the secret scanner reported and
//!    writes the request and its manifest;
//! 2. unless `--no-history` is given, this module adds the snapshot's
//!    masked commit range to the request (`history_upload`, D-0915);
//!    it shows the manifest, with the range, and asks before anything is
//!    sent (`--yes` skips the question, for CI);
//! 3. it submits the request to the server's job queue with `curl` (the
//!    only network calls sloptopop makes, and the only code that makes them
//!    is this open code), shows its place in line on stderr until the job
//!    runs, and writes the answer next to the request; the job is then
//!    deleted on the server, also when `sloptopop` is stopped (Ctrl-C,
//!    exit 130) while it waits.
//!
//! Without a server there is no check: when none answers, the error says
//! so and names the hosted service and the self-hosted option.
//!
//! Standards: SCF PRI-02 (notice before upload), TDA-12.1; GDPR Art. 25;
//! decisions D-0008, D-0174, D-0179, D-0183.

use std::io::{BufRead, Write};
use std::path::Path;

use serde_json::Value;

use crate::backend_jobs::{Cancel, Progress, endpoint, pause, submit, wait};
use crate::collect_lookup::find;
use crate::command_io::parse_document;
use crate::diagnostic::report;
use crate::engine_locate::Locator;
use crate::engine_run::invoke;
use crate::history_upload::{attach, line};
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
    /// Add the commit range.
    pub(crate) history: bool,
}

/// The run the server answered: its id and the response's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answered {
    /// The run id.
    pub(crate) run: String,
    /// Where the response is.
    pub(crate) response: std::path::PathBuf,
}

/// The hint every exchange the server did not take ends with.
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
/// submits it and waits for the answer; the run the server answered, or
/// the exit code to stop with.
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
    let backend = asked.backend.unwrap_or(DEFAULT_BACKEND);
    let url = endpoint(backend);
    let root = Path::new(asked.root);
    let request = root.join(text(&document, "request"));
    let response = root.join(text(&document, "response"));
    let attached = asked
        .history
        .then(|| attach(locator.path.as_deref(), root, &request))
        .flatten();
    let shown = manifest(document.get("manifest").unwrap_or(&Value::Null), &url)
        + &attached.as_ref().map(line).unwrap_or_default();
    let _shown = stderr.write_all(shown.as_bytes());
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
    let answer = response.with_file_name("job.json");
    let status = match submit(&curl, backend, &request, &answer) {
        Ok(status) => status,
        Err(why) => {
            return Err(report(
                stderr,
                &format!("error: {why}\n{}", hint()),
                UPLOAD_ERROR,
            ));
        }
    };
    let cancel = Cancel::start(&curl, &status, &answer);
    let waited = wait(
        &curl,
        &status,
        (&answer, &response),
        locator.poll,
        &mut Progress::new(stderr, locator.terminal),
        &mut pause,
    );
    cancel.now();
    if let Err(why) = waited {
        return Err(report(stderr, &format!("error: {why}\n"), UPLOAD_ERROR));
    }
    Ok(Answered {
        run: text(&document, "run").to_owned(),
        response,
    })
}

#[cfg(test)]
mod tests;
