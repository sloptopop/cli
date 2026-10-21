//! `sloptopop transform` (preview): the open-source half of a transform.
//!
//! 1. `sloptopop-engine transform-request <root>` checks the repository,
//!    masks secret values and writes the request and its manifest;
//! 2. this command filters the request to source-code files, updates its
//!    digest-bound run and manifest, then shows the manifest and asks
//!    before anything is sent (`--yes` skips the question, for CI);
//! 3. it posts the filtered request to the backend with `curl` (the only network
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
use crate::engine_run::{ENGINE_ERROR, invoke};
use crate::json_fields::text;
use crate::render_manifest::manifest;

/// Exit code when the upload was not confirmed or did not go through.
pub(crate) const UPLOAD_ERROR: u8 = 4;

/// Source-code extensions allowed in an upload.
const CODE_EXTENSIONS: [&str; 28] = [
    "c", "cc", "cjs", "cpp", "cs", "cts", "cxx", "go", "h", "hpp", "java", "js", "jsx", "kt",
    "kts", "mjs", "mts", "php", "ps1", "psm1", "py", "rb", "rs", "scala", "sql", "swift", "ts",
    "tsx",
];

/// The reason listed for files filtered by this command.
const EXTENSION_REASON: &str = "not a code file extension";

/// How many excluded paths the manifest lists by name.
const LISTED_EXCLUDED: usize = 100;

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
        Ok(mut document) => {
            if let Err(why) = filter_request(&mut document, Path::new(&options.root)) {
                return report(
                    stderr,
                    &format!("error: cannot prepare code-only transform request: {why}\n"),
                    ENGINE_ERROR,
                );
            }
            document
        }
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

/// Filters the engine's request to code files and binds the rewritten
/// request, manifest and response path to its new digest-derived run id.
fn filter_request(document: &mut Value, root: &Path) -> Result<(), String> {
    filter_request_with_removal(document, root, remove_original_request)
}

/// Filters a request, allowing the cleanup result to be tested independently.
fn filter_request_with_removal(
    document: &mut Value,
    root: &Path,
    remove: fn(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let old_run = text(document, "run").to_owned();
    if old_run.len() != 16 || !old_run.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("the engine returned an invalid run id".to_owned());
    }
    let old_request = Path::new(text(document, "request")).to_path_buf();
    let expected = Path::new(".sloptopop")
        .join("transform")
        .join(old_run)
        .join("request.json");
    if old_request != expected {
        return Err("the engine returned an invalid request path".to_owned());
    }

    let request_path = root.join(&old_request);
    let bytes = fs::read(&request_path)
        .map_err(|error| format!("cannot read `{}`: {error}", request_path.display()))?;
    let mut request: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("`{}` is not valid JSON: {error}", request_path.display()))?;
    let files = request
        .get_mut("files")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "the engine's request has no files array".to_owned())?;
    let mut kept = Vec::with_capacity(files.len());
    let mut filtered = Vec::new();
    for file in std::mem::take(files) {
        let path = file
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "the engine's request has a file without a path".to_owned())?;
        if code_file(path) {
            kept.push(file);
        } else {
            filtered.push(path.to_owned());
        }
    }
    *files = kept;

    let request_bytes = canonical_bytes(&request);
    let digest = blake3::Hash::from(blake3::derive_key(
        "sloptopop transform request v1",
        &request_bytes,
    ));
    let digest = digest.to_hex().to_string();
    let run = digest.chars().take(16).collect::<String>();
    let request_relative = Path::new(".sloptopop")
        .join("transform")
        .join(&run)
        .join("request.json");
    let response_relative = request_relative.with_file_name("response.json");

    update_manifest(document, &request, request_bytes.len(), &filtered)?;
    let destination = root.join(&request_relative);
    let directory = root.join(".sloptopop").join("transform").join(&run);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create `{}`: {error}", directory.display()))?;
    fs::write(&destination, &request_bytes)
        .map_err(|error| format!("cannot write `{}`: {error}", destination.display()))?;
    if request_relative != old_request {
        remove(&request_path)?;
    }
    document["run"] = Value::String(run);
    document["request"] = Value::String(request_relative.to_string_lossy().into_owned());
    document["response"] = Value::String(response_relative.to_string_lossy().into_owned());
    Ok(())
}

/// Removes the engine's now-obsolete request after the filtered request is saved.
fn remove_original_request(path: &Path) -> Result<(), String> {
    fs::remove_file(path).map_err(|error| format!("cannot remove `{}`: {error}", path.display()))
}

/// Whether `path` has one of the allowed source-code extensions.
fn code_file(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        CODE_EXTENSIONS
            .iter()
            .any(|code_extension| extension.eq_ignore_ascii_case(code_extension))
    })
}

/// Serializes a JSON value with the same stable encoding as the engine.
fn canonical_bytes(value: &Value) -> Vec<u8> {
    let mut output = Vec::new();
    canonical_value(value, &mut output);
    output
}

/// Appends a JSON value with object members in key order.
fn canonical_value(item: &Value, output: &mut Vec<u8>) {
    match item {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        Value::String(text) => canonical_string(text, output),
        Value::Array(values) => {
            output.push(b'[');
            for (index, child) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                canonical_value(child, output);
            }
            output.push(b']');
        }
        Value::Object(members) => {
            output.push(b'{');
            for (index, (key, child)) in members.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                canonical_string(key, output);
                output.push(b':');
                canonical_value(child, output);
            }
            output.push(b'}');
        }
    }
}

/// Appends a JSON string using the engine's canonical escaping.
fn canonical_string(text: &str, output: &mut Vec<u8>) {
    output.push(b'"');
    for character in text.chars() {
        match character {
            '"' => output.extend_from_slice(b"\\\""),
            '\\' => output.extend_from_slice(b"\\\\"),
            '\n' => output.extend_from_slice(b"\\n"),
            '\r' => output.extend_from_slice(b"\\r"),
            '\t' => output.extend_from_slice(b"\\t"),
            character if u32::from(character) < 0x20 => {
                output.extend_from_slice(format!("\\u{:04x}", u32::from(character)).as_bytes());
            }
            character => {
                let mut buffer = [0; 4];
                output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    output.push(b'"');
}

/// Updates the manifest to describe the exact request that will be posted.
fn update_manifest(
    document: &mut Value,
    request: &Value,
    request_bytes: usize,
    filtered: &[String],
) -> Result<(), String> {
    let manifest = document
        .get_mut("manifest")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "the engine's manifest is not an object".to_owned())?;
    update_file_counts(manifest, request, request_bytes)?;
    if !filtered.is_empty() {
        update_exclusions(manifest, filtered)?;
    }
    Ok(())
}

/// Updates file counts and byte totals in the manifest.
fn update_file_counts(
    manifest: &mut serde_json::Map<String, Value>,
    request: &Value,
    request_bytes: usize,
) -> Result<(), String> {
    let files = request
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| "the filtered request has no files array".to_owned())?;
    let bytes = files.iter().try_fold(0_usize, |total, file| {
        let text = file
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| "the engine's request has a file without text".to_owned())?;
        checked_upload_size(total, text.len())
    })?;
    manifest.insert("files".to_owned(), Value::from(files.len()));
    manifest.insert("bytes".to_owned(), Value::from(bytes));
    manifest.insert("requestBytes".to_owned(), Value::from(request_bytes));
    Ok(())
}

/// Adds file-text byte lengths while detecting impossible size overflow.
fn checked_upload_size(total: usize, additional: usize) -> Result<usize, String> {
    total
        .checked_add(additional)
        .ok_or_else(|| "the upload size does not fit in memory".to_owned())
}

/// Adds CLI-filtered paths to the exclusion section of the manifest.
fn update_exclusions(
    manifest: &mut serde_json::Map<String, Value>,
    filtered: &[String],
) -> Result<(), String> {
    let excluded = manifest
        .get_mut("excluded")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "the engine's exclusion manifest is not an object".to_owned())?;
    let current_count = excluded
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "the exclusion count is missing or invalid".to_owned())?;
    let count = increment_count(
        current_count,
        filtered.len(),
        "the exclusion count is too large",
    )?;
    excluded.insert("count".to_owned(), Value::Number(count.into()));

    let by_reason = excluded
        .get_mut("byReason")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "the exclusion reasons are missing or invalid".to_owned())?;
    let current_reason_count = by_reason
        .get(EXTENSION_REASON)
        .map_or(Some(0), Value::as_u64)
        .ok_or_else(|| "the extension exclusion count is invalid".to_owned())?;
    let reason_count = increment_count(
        current_reason_count,
        filtered.len(),
        "the extension exclusion count is too large",
    )?;
    by_reason.insert(
        EXTENSION_REASON.to_owned(),
        Value::Number(reason_count.into()),
    );

    let paths = excluded
        .get_mut("paths")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "the excluded paths are missing or invalid".to_owned())?;
    let mut listed: Vec<(String, Value)> = paths
        .iter()
        .map(|path| {
            let name = path
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "an excluded path is missing its name".to_owned())?;
            Ok((name.to_owned(), path.clone()))
        })
        .collect::<Result<_, String>>()?;
    listed.extend(filtered.iter().map(|path| {
        (
            path.clone(),
            serde_json::json!({"path": path, "reason": EXTENSION_REASON}),
        )
    }));
    listed.sort_by(|left, right| left.0.cmp(&right.0));
    *paths = listed
        .into_iter()
        .take(LISTED_EXCLUDED)
        .map(|(_, path)| path)
        .collect();
    Ok(())
}

/// Adds one for each path and reports arithmetic overflow.
fn increment_count(mut count: u64, additions: usize, overflow_error: &str) -> Result<u64, String> {
    for _ in 0..additions {
        count = count
            .checked_add(1)
            .ok_or_else(|| overflow_error.to_owned())?;
    }
    Ok(count)
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
