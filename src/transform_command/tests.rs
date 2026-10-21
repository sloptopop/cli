//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use super::{DEFAULT_BACKEND, UPLOAD_ERROR};
use crate::cli::run_with;
use crate::engine_locate::Locator;
use crate::tests::support::{FAKE_ENGINE, TempDir};

const CURL_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-curl");

struct Output {
    code: u8,
    stdout: String,
    stderr: String,
}

/// Runs `sloptopop transform <root> <args>` with the fake engine, the fake
/// curl on `PATH` (unless `curl` is false) and `stdin`.
fn transform(root: &Path, args: &[&str], stdin: &[u8], curl: bool) -> Output {
    let locator = Locator {
        explicit: Some(OsString::from(FAKE_ENGINE)),
        exe_dir: None,
        path: curl.then(|| OsString::from(CURL_DIR)),
        vars: Vec::new(),
    };
    let mut all: Vec<OsString> = vec!["transform".into(), root.as_os_str().to_owned()];
    all.extend(args.iter().map(OsString::from));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run_with(&all, &locator, &mut &stdin[..], &mut stdout, &mut stderr);
    Output {
        code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    }
}

fn root(dir: &TempDir, name: &str) -> std::path::PathBuf {
    let path = dir.0.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn a_confirmed_upload_runs_the_transform_and_prints_the_summary() {
    let dir = TempDir::new();
    let repo = root(&dir, "repo");
    let out = transform(&repo, &["--backend", "http://ok:8080/"], b"yes\n", true);
    assert_eq!(out.code, 1, "a dry run with pending steps: {}", out.stderr);
    assert_eq!(out.stdout, "# summary of 0123456789abcdef\n", "the summary");
    assert!(
        out.stderr
            .contains("sloptopop transform will upload to http://ok:8080/v1/transform:\n  2 files"),
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("Upload? [y/N] "), "asked");
    assert!(
        out.stderr.contains("fake-engine: transform ")
            && out.stderr.contains(" --run 0123456789abcdef"),
        "{}",
        out.stderr
    );
    let posted = repo.join(".sloptopop/transform/0123456789abcdef/request.json.posted");
    assert_eq!(
        fs::read_to_string(posted).unwrap(),
        r#"{"schemaVersion":1}"#,
        "the request was posted"
    );
    let json = transform(
        &repo,
        &[
            "--backend",
            "https://ok",
            "--yes",
            "--apply",
            "--branch",
            "fixes",
            "--confirm",
            "att-1,att-2",
            "--by",
            "lead",
            "--format",
            "json",
        ],
        b"",
        true,
    );
    assert_eq!(json.code, 0, "{}", json.stderr);
    assert!(!json.stderr.contains("Upload?"), "--yes does not ask");
    assert!(
        json.stdout.contains(
            "--run 0123456789abcdef --apply --branch fixes --confirm att-1,att-2 --by lead"
        ),
        "{}",
        json.stdout
    );
    let fallback = transform(
        &root(&dir, "nosummary"),
        &["--backend", "http://ok", "--yes"],
        b"",
        true,
    );
    assert!(
        fallback.stdout.starts_with("{\"schemaVersion\":1"),
        "no summary: the report"
    );
}

#[test]
fn an_upload_needs_a_yes_curl_and_an_answer() {
    let dir = TempDir::new();
    let repo = root(&dir, "repo");
    for answer in [&b"\n"[..], b"n\n", b""] {
        let out = transform(&repo, &["--backend", "http://ok"], answer, true);
        assert_eq!(out.code, UPLOAD_ERROR, "{answer:?}");
        assert!(out.stderr.contains("not confirmed"), "{}", out.stderr);
    }
    assert!(
        transform(&repo, &["--backend", "http://ok"], b"Y\n", true).code < UPLOAD_ERROR,
        "Y"
    );
    let no_curl = transform(&repo, &["--yes"], b"", false);
    assert_eq!(no_curl.code, UPLOAD_ERROR, "no curl");
    assert!(
        no_curl.stderr.contains("curl not found"),
        "{}",
        no_curl.stderr
    );
    assert!(
        no_curl
            .stderr
            .contains(&format!("{DEFAULT_BACKEND}/v1/transform")),
        "the default backend"
    );
    let refused = transform(&repo, &["--backend", "http://refuses", "--yes"], b"", true);
    assert_eq!(refused.code, UPLOAD_ERROR, "refused");
    assert!(
        refused.stderr.contains("every slot is taken"),
        "{}",
        refused.stderr
    );
    let down = transform(&repo, &["--backend", "http://down", "--yes"], b"", true);
    assert!(down.stderr.contains("Failed to connect"), "{}", down.stderr);
}

#[test]
fn engine_failures_exit_3() {
    let dir = TempDir::new();
    let broken = transform(&root(&dir, "broken"), &["--yes"], b"", true);
    assert_eq!(broken.code, 3, "transform-request failed");
    assert!(
        broken.stderr.contains("uncommitted changes"),
        "{}",
        broken.stderr
    );
    let garbage = transform(&root(&dir, "garbage"), &["--yes"], b"", true);
    assert_eq!(garbage.code, 3, "not JSON");
    let failing = transform(
        &root(&dir, "failing"),
        &["--backend", "http://ok", "--yes"],
        b"",
        true,
    );
    assert_eq!(failing.code, 3, "the engine's transform failed");
    assert!(failing.stderr.contains("HEAD moved"), "{}", failing.stderr);
    let repo = root(&dir, "lost");
    let answered = transform(&repo, &["--backend", "http://ok", "--yes"], b"", true);
    assert_eq!(answered.code, 1, "{}", answered.stderr);
    fs::remove_file(repo.join(".sloptopop/transform/0123456789abcdef/response.json")).unwrap();
    let lost = transform(&repo, &["--backend", "http://down", "--yes"], b"", true);
    assert_eq!(lost.code, UPLOAD_ERROR, "the backend is down");
}
