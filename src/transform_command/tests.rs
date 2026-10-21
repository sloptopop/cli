//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use super::{
    CODE_EXTENSIONS, DEFAULT_BACKEND, UPLOAD_ERROR, canonical_bytes, checked_upload_size,
    code_file, filter_request, filter_request_with_removal, increment_count,
    remove_original_request, update_exclusions, update_file_counts, update_manifest,
};
use crate::cli::run_with;
use crate::engine_locate::Locator;
use crate::tests::support::{FAKE_ENGINE, TempDir};
use serde_json::Value;

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

fn posted_run(repo: &Path) -> std::path::PathBuf {
    fs::read_dir(repo.join(".sloptopop/transform"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.join("request.json.posted").is_file())
        .expect("the filtered request was posted")
}

fn assert_filtered_request(run_dir: &Path, stderr: &str) {
    assert!(
        stderr.contains(
            "1 file(s) not uploaded: 1 not a code file extension\n    README.md (not a code file extension)"
        ),
        "{stderr}"
    );
    let posted: Value =
        serde_json::from_slice(&fs::read(run_dir.join("request.json.posted")).unwrap()).unwrap();
    assert_eq!(posted["files"].as_array().unwrap().len(), 1, "{posted}");
    assert_eq!(posted["files"][0]["path"], "src/app.ts", "{posted}");
    assert_eq!(
        fs::read(run_dir.join("request.json")).unwrap(),
        fs::read(run_dir.join("request.json.posted")).unwrap(),
        "the engine later verifies the exact filtered request"
    );
    let request = fs::read(run_dir.join("request.json")).unwrap();
    let digest = blake3::Hash::from(blake3::derive_key(
        "sloptopop transform request v1",
        &request,
    ));
    assert_eq!(
        digest.to_hex().as_str().get(..16),
        run_dir.file_name().and_then(|name| name.to_str()),
        "the run id is bound to the exact canonical request"
    );
    assert_eq!(
        posted["files"][0]["text"], "const app = 1;\n",
        "only source text was posted"
    );
}

#[test]
fn code_file_matches_the_upload_extension_allowlist() {
    for extension in CODE_EXTENSIONS {
        let path = format!("src/file.{extension}");
        assert!(code_file(&path), "{path}");
    }
    assert!(code_file("src/file.TS"), "case insensitive");
    assert!(!code_file("README.md"), "documentation");
    assert!(!code_file("src/no-extension"), "no extension");
}

#[test]
fn canonical_json_encoding_matches_engine_format() {
    let value = serde_json::json!([
        null,
        true,
        false,
        7,
        "é\"\\\n\r\t\u{1} ",
        {"z": "last", "a": "first"}
    ]);
    assert_eq!(
        canonical_bytes(&value),
        r#"[null,true,false,7,"é\"\\\n\r\t\u0001 ",{"a":"first","z":"last"}]"#.as_bytes(),
        "JSON is compact, sorted, and escaped like the engine's digest input"
    );
}

#[test]
fn filtering_rejects_invalid_run_ids_paths_and_missing_requests() {
    let mut invalid_run = serde_json::json!({"run": "bad"});
    assert_eq!(
        filter_request(&mut invalid_run, Path::new(".")),
        Err("the engine returned an invalid run id".to_owned())
    );
    let mut invalid_path = serde_json::json!({
        "run": "0123456789abcdef",
        "request": ".sloptopop/transform/wrong/request.json"
    });
    assert_eq!(
        filter_request(&mut invalid_path, Path::new(".")),
        Err("the engine returned an invalid request path".to_owned())
    );
    let mut missing_request = serde_json::json!({
        "run": "0123456789abcdef",
        "request": ".sloptopop/transform/0123456789abcdef/request.json"
    });
    assert!(filter_request(&mut missing_request, Path::new(".")).is_err());
}

#[test]
fn an_unfiltered_request_keeps_its_digest_bound_run() {
    let dir = TempDir::new();
    let repo = root(&dir, "repo");
    let request = serde_json::json!({"files":[{"path":"src/app.rs","text":"fn main() {}\n"}]});
    let request_bytes = canonical_bytes(&request);
    let digest = blake3::Hash::from(blake3::derive_key(
        "sloptopop transform request v1",
        &request_bytes,
    ));
    let run: String = digest.to_hex().to_string().chars().take(16).collect();
    let relative = Path::new(".sloptopop")
        .join("transform")
        .join(&run)
        .join("request.json");
    let original = repo.join(&relative);
    fs::create_dir_all(original.parent().unwrap()).unwrap();
    fs::write(&original, &request_bytes).unwrap();
    let mut document = serde_json::json!({
        "run": run,
        "request": relative.to_string_lossy(),
        "manifest": {
            "excluded": {"count": 0, "byReason": {}, "paths": []}
        }
    });

    filter_request(&mut document, &repo).unwrap();

    assert_eq!(document["run"], run, "the content digest did not change");
    assert!(original.is_file(), "an unchanged request is not moved");
    assert_eq!(
        document["request"],
        relative.to_string_lossy().as_ref(),
        "the request path remains digest-bound"
    );
    assert_eq!(document["manifest"]["files"], 1);
    assert_eq!(document["manifest"]["excluded"]["count"], 0);
}

#[test]
fn filtered_exclusions_preserve_existing_paths_and_sort_them() {
    let request = serde_json::json!({"files":[]});
    let mut document = serde_json::json!({
        "manifest": {
            "excluded": {
                "count": 1,
                "byReason": {"existing reason": 1},
                "paths": [{"path":"z.txt","reason":"existing reason"}]
            }
        }
    });

    update_manifest(&mut document, &request, 11, &["README.md".to_owned()]).unwrap();

    assert_eq!(document["manifest"]["excluded"]["count"], 2);
    assert_eq!(
        document["manifest"]["excluded"]["paths"][0]["path"],
        "README.md"
    );
    assert_eq!(
        document["manifest"]["excluded"]["paths"][1]["path"],
        "z.txt"
    );
}

fn filtering_error(request_bytes: &[u8]) -> String {
    let dir = TempDir::new();
    let repo = root(&dir, "repo");
    let relative = Path::new(".sloptopop/transform/0123456789abcdef/request.json");
    let path = repo.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, request_bytes).unwrap();
    let mut document = serde_json::json!({
        "run": "0123456789abcdef",
        "request": relative.to_string_lossy()
    });
    filter_request(&mut document, &repo).unwrap_err()
}

fn filtered_request_fixture(repo: &Path, manifest: &Value) -> Value {
    let relative = Path::new(".sloptopop/transform/ffffffffffffffff/request.json");
    let path = repo.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, br#"{"files":[{"path":"README.md","text":"notes"}]}"#).unwrap();
    serde_json::json!({
        "run": "ffffffffffffffff",
        "request": relative.to_string_lossy(),
        "manifest": manifest
    })
}

fn filtered_request_run() -> String {
    let bytes = canonical_bytes(&serde_json::json!({"files":[]}));
    blake3::Hash::from(blake3::derive_key("sloptopop transform request v1", &bytes))
        .to_hex()
        .to_string()
        .chars()
        .take(16)
        .collect()
}

#[test]
fn filtering_reports_malformed_requests() {
    assert!(filtering_error(b"{").contains("is not valid JSON"));
    assert!(filtering_error(br#"{"files":null}"#).contains("no files array"));
    assert!(filtering_error(br#"{"files":[{}]}"#).contains("without a path"));
}

#[test]
fn filtering_reports_manifest_and_filesystem_failures() {
    let dir = TempDir::new();
    let repo = root(&dir, "manifest");
    let mut invalid_manifest = filtered_request_fixture(&repo, &serde_json::json!({}));
    assert!(filter_request(&mut invalid_manifest, &repo).is_err());

    let run = filtered_request_run();
    let blocked_dir = root(&dir, "blocked-dir");
    let mut create_error = filtered_request_fixture(
        &blocked_dir,
        &serde_json::json!({"excluded":{
            "count":0,"byReason":{},"paths":[]
        }}),
    );
    let blocked_directory = blocked_dir.join(".sloptopop/transform").join(&run);
    fs::write(&blocked_directory, b"not a directory").unwrap();
    assert!(filter_request(&mut create_error, &blocked_dir).is_err());

    let blocked_file = root(&dir, "blocked-file");
    let mut write_error = filtered_request_fixture(
        &blocked_file,
        &serde_json::json!({"excluded":{
            "count":0,"byReason":{},"paths":[]
        }}),
    );
    let blocked_request = blocked_file.join(".sloptopop/transform").join(&run);
    fs::create_dir_all(blocked_request.join("request.json")).unwrap();
    assert!(filter_request(&mut write_error, &blocked_file).is_err());
}

#[test]
fn manifest_update_reports_invalid_shapes_and_overflow() {
    let request_without_files = serde_json::json!({});
    let request_without_text = serde_json::json!({"files":[{}]});
    let mut invalid_manifest = serde_json::json!({});
    assert!(
        update_manifest(
            &mut invalid_manifest,
            &serde_json::json!({"files":[]}),
            0,
            &[]
        )
        .is_err()
    );

    let mut missing_files_manifest = serde_json::json!({"manifest":{}});
    assert!(update_manifest(&mut missing_files_manifest, &request_without_files, 0, &[]).is_err());
    let mut missing_text_manifest = serde_json::json!({"manifest":{}});
    assert!(
        update_file_counts(
            missing_text_manifest["manifest"].as_object_mut().unwrap(),
            &request_without_text,
            0
        )
        .is_err()
    );
    assert_eq!(checked_upload_size(10, 5).unwrap(), 15);
    assert!(checked_upload_size(usize::MAX, 1).is_err());
}

#[test]
fn exclusion_update_reports_invalid_shapes_and_overflow() {
    let cases = [
        serde_json::json!({}),
        serde_json::json!({"excluded":{}}),
        serde_json::json!({"excluded":{"count":u64::MAX,"byReason":{},"paths":[]}}),
        serde_json::json!({"excluded":{"count":0,"byReason":"invalid","paths":[]}}),
        serde_json::json!({"excluded":{"count":0,"byReason":{"not a code file extension":"invalid"},"paths":[]}}),
        serde_json::json!({"excluded":{"count":0,"byReason":{"not a code file extension":u64::MAX},"paths":[]}}),
        serde_json::json!({"excluded":{"count":0,"byReason":{},"paths":{}}}),
        serde_json::json!({"excluded":{"count":0,"byReason":{},"paths":[{}]}}),
    ];
    for mut manifest in cases {
        assert!(
            update_exclusions(manifest.as_object_mut().unwrap(), &["README.md".to_owned()])
                .is_err(),
            "{manifest}"
        );
    }
    let mut manifest = serde_json::json!({"excluded":{
        "count":0,
        "byReason":{},
        "paths":[{"path":"README.md","reason":"old"}]
    }});
    assert!(
        update_exclusions(manifest.as_object_mut().unwrap(), &["README.md".to_owned()]).is_ok()
    );
}

#[test]
fn obsolete_request_removal_reports_filesystem_errors() {
    let dir = TempDir::new();
    assert!(
        remove_original_request(&dir.0.join("missing.json"))
            .unwrap_err()
            .contains("cannot remove")
    );
}

#[test]
fn filtering_reports_failure_to_remove_the_obsolete_request() {
    let dir = TempDir::new();
    let repo = root(&dir, "repo");
    let mut document = filtered_request_fixture(
        &repo,
        &serde_json::json!({"excluded":{
            "count":0,"byReason":{},"paths":[]
        }}),
    );
    let error = filter_request_with_removal(&mut document, &repo, |_| {
        Err("cannot remove test request".to_owned())
    })
    .unwrap_err();
    assert_eq!(error, "cannot remove test request");
}

#[test]
fn exclusion_counter_increment_handles_empty_additions_and_overflow() {
    assert_eq!(increment_count(4, 0, "overflow").unwrap(), 4);
    assert_eq!(increment_count(1, 2, "overflow").unwrap(), 3);
    assert_eq!(
        increment_count(u64::MAX, 1, "overflow"),
        Err("overflow".to_owned())
    );
}

#[test]
fn a_confirmed_upload_runs_the_transform_and_prints_the_summary() {
    let dir = TempDir::new();
    let repo = root(&dir, "repo");
    let out = transform(&repo, &["--backend", "http://ok:8080/"], b"yes\n", true);
    assert_eq!(out.code, 1, "a dry run with pending steps: {}", out.stderr);
    let run_dir = posted_run(&repo);
    let run = run_dir.file_name().unwrap().to_string_lossy();
    assert_eq!(out.stdout, format!("# summary of {run}\n"), "the summary");
    assert_filtered_request(&run_dir, &out.stderr);
    assert!(
        out.stderr
            .contains("sloptopop transform will upload to http://ok:8080/v1/transform:\n  1 files"),
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("Upload? [y/N] "), "asked");
    assert!(
        out.stderr.contains("fake-engine: transform ")
            && out.stderr.contains(&format!(" --run {run}")),
        "{}",
        out.stderr
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
    let json_run = posted_run(&repo)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        json.stdout.contains(&format!(
            "--run {json_run} --apply --branch fixes --confirm att-1,att-2 --by lead"
        )),
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
    let invalid_run = transform(&root(&dir, "invalidrun"), &["--yes"], b"", true);
    assert_eq!(invalid_run.code, 3, "invalid run id");
    assert!(
        invalid_run
            .stderr
            .contains("cannot prepare code-only transform request"),
        "{}",
        invalid_run.stderr
    );
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
    let run_dir = posted_run(&repo);
    fs::remove_file(run_dir.join("response.json")).unwrap();
    let lost = transform(&repo, &["--backend", "http://down", "--yes"], b"", true);
    assert_eq!(lost.code, UPLOAD_ERROR, "the backend is down");
}
