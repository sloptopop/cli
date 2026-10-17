//! `sloptopop transform` (preview): the open-source half of a transform.
//!
//! 1. `sloptopop-engine transform-request <root>` checks the repository,
//!    masks secret values and writes the request and its manifest;
//! 2. this command shows the manifest on stderr and asks before anything
//!    is sent (`--yes` skips the question, for CI);
//! 3. it posts the request to the backend with `curl` (the only network
//!    call sloptopop makes, and the only code that makes one is this open
//!    code);
//! 4. `sloptopop-engine transform <root> --run <id> …` checks the answer,
//!    delivers the verified steps (on a new branch with `--apply`) and
//!    writes the report; this command prints `SUMMARY.md` (or, with
//!    `--format json`, `report.json`).
//!
//! Exit code: 0 every automated step delivered (or, in a dry run,
//! deliverable); 1 a step is pending a confirmation, refused or blocked;
//! 3 the engine is missing or failed; 4 the upload was not confirmed,
//! `curl` is missing, or the backend could not be reached or refused it.
//!
//! Standards: SCF PRI-02 (notice before upload), CHG-03 (changes only on
//! a new branch), TDA-12.1; GDPR Art. 25; decisions D-0008, D-0174,
//! D-0179.

use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;

use serde_json::Value;

use crate::args::{Format, TransformOptions};
use crate::backend_post::{endpoint, post};
use crate::collect_lookup::find;
use crate::command_io::{parse_document, write_output};
use crate::diagnostic::report;
use crate::engine_locate::Locator;
use crate::engine_run::invoke;
use crate::json_fields::text;
use crate::render_manifest::manifest;

/// Exit code when the upload was not confirmed or did not go through.
pub(crate) const UPLOAD_ERROR: u8 = 4;

/// The hosted backend (to be confirmed with the hosting decision, D-0173).
pub(crate) const DEFAULT_BACKEND: &str = "https://api.sloptopop.dev";

/// Runs `sloptopop transform` as `options` ask.
pub(crate) fn transform(
    locator: &Locator,
    options: &TransformOptions,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let prepared = match invoke(locator, &["transform-request", &options.root], stderr) {
        Ok((output, _)) => output,
        Err(failure) => return failure,
    };
    let document = match parse_document(&prepared.stdout, stderr) {
        Ok(document) => document,
        Err(failure) => return failure,
    };
    let url = endpoint(options.backend.as_deref().unwrap_or(DEFAULT_BACKEND));
    let root = Path::new(&options.root);
    let request = root.join(text(&document, "request"));
    let response = root.join(text(&document, "response"));
    let run = text(&document, "run");
    let _shown = stderr
        .write_all(manifest(document.get("manifest").unwrap_or(&Value::Null), &url).as_bytes());
    if !options.yes && !confirmed(stdin, stderr) {
        return report(
            stderr,
            "error: the upload was not confirmed (pass --yes to upload without asking)\n",
            UPLOAD_ERROR,
        );
    }
    let Some(curl) = find(locator.path.as_deref(), "curl") else {
        return report(
            stderr,
            "error: curl not found on PATH: transform sends its request with curl\n",
            UPLOAD_ERROR,
        );
    };
    if let Err(why) = post(&curl, &url, &request, &response) {
        return report(stderr, &format!("error: {why}\n"), UPLOAD_ERROR);
    }
    let confirm = options.confirm.join(",");
    let mut args = vec!["transform", &options.root, "--run", run];
    if options.apply {
        args.push("--apply");
    }
    if let Some(branch) = &options.branch {
        args.extend(["--branch", branch]);
    }
    if let Some(by) = &options.by {
        args.extend(["--confirm", &confirm, "--by", by]);
    }
    let (output, code) = match invoke(locator, &args, stderr) {
        Ok(result) => result,
        Err(failure) => return failure,
    };
    let printed = match options.format {
        Format::Json => output.stdout,
        Format::Text => {
            let summary = response.with_file_name("SUMMARY.md");
            fs::read(summary).unwrap_or(output.stdout)
        }
    };
    write_output(stdout, stderr, &printed, code)
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

#[cfg(test)]
mod tests;
