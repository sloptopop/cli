//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use serde_json::Value;

use super::*;
use crate::engine_run::{ENGINE_ERROR, NOT_FOUND};
use crate::render_report;
use crate::tests::support::{FAKE_LOCAL, TempDir, explicit, fake_local};

struct Output {
    code: u8,
    stdout: String,
    stderr: String,
}

fn run_with(args: &[&str], stdin: &[u8]) -> Output {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let code = run(&args, &mut &stdin[..], &mut stdout, &mut stderr);
    Output {
        code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    }
}

#[test]
fn version_flags_print_version() {
    for flag in ["version", "--version", "-V"] {
        let out = run_with(&[flag], b"");
        assert_eq!(out.code, 0, "{flag} exit code");
        assert_eq!(
            out.stdout,
            format!("sloptopop {SERVER_VERSION} (protocol {PROTOCOL_VERSION})\n"),
            "{flag} output"
        );
    }
}

#[test]
fn help_flags_print_usage() {
    for flag in ["help", "--help", "-h"] {
        let out = run_with(&[flag], b"");
        assert_eq!((out.code, out.stdout.as_str()), (0, USAGE), "{flag}");
    }
}

#[test]
fn schema_prints_the_document() {
    let out = run_with(&["schema"], b"");
    assert_eq!(out.code, 0, "exit code");
    let printed: Value = serde_json::from_str(&out.stdout).unwrap_or_default();
    assert_eq!(printed, schema::document(), "schema output");
}

#[test]
fn usage_errors_exit_2_with_usage_on_stderr() {
    for args in [&[][..], &["serve"], &["bogus", "x"]] {
        let out = run_with(args, b"");
        assert_eq!(out.code, USAGE_ERROR, "{args:?} exit code");
        assert!(out.stdout.is_empty(), "{args:?} stdout");
        assert!(out.stderr.starts_with("error: "), "{args:?} message");
        assert!(out.stderr.ends_with(USAGE), "{args:?} usage");
    }
}

#[test]
fn non_utf8_arguments_are_rejected() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    #[cfg(unix)]
    let arg = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![0xff])
    };
    #[cfg(not(unix))]
    let arg = OsString::from("\u{fffd}");
    let code = run(&[arg], &mut &b""[..], &mut out, &mut err);
    assert_eq!(code, USAGE_ERROR, "exit code");
    assert!(
        String::from_utf8_lossy(&err).contains('\u{fffd}'),
        "shown lossily"
    );
}

#[test]
fn serve_runs_the_server() {
    let out = run_with(&["serve", "--stdio"], b"");
    assert_eq!(out.code, 1, "EOF without shutdown");
    let bad = run_with(&["serve", "--stdio"], b"junk\r\n\r\n");
    assert_eq!(bad.code, 1, "framing error");
    assert!(
        bad.stderr.starts_with("error: invalid header line"),
        "reported"
    );
}

struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn broken_stdout_exits_1() {
    let mut err = Vec::new();
    let code = run(
        &[OsString::from("version")],
        &mut &b""[..],
        &mut Broken,
        &mut err,
    );
    assert_eq!(code, 1, "write failure");
}

#[test]
fn broken_stderr_still_exits_with_the_code() {
    let mut out = Vec::new();
    let code = run(
        &[OsString::from("bogus")],
        &mut &b""[..],
        &mut out,
        &mut Broken,
    );
    assert_eq!(code, USAGE_ERROR, "usage error despite broken stderr");
}

fn run_engine(locator: &Locator, args: &[&str]) -> Output {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let code = super::run_with(&args, locator, &mut &b""[..], &mut stdout, &mut stderr);
    Output {
        code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    }
}

/// A repository directory named `name` (its name picks the fake's
/// behaviour), in `dir`.
fn repo(dir: &TempDir, name: &str) -> String {
    let path = dir.0.join(name);
    std::fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// `sloptopop <command> <root> <rest> --backend http://ok.test --yes`
/// with the fake `sloptopop-local` and the fake curl.
fn served(command: &str, root: &str, rest: &[&str]) -> Output {
    let mut args = vec![command, root];
    args.extend_from_slice(rest);
    args.extend(["--backend", "http://ok.test", "--yes"]);
    run_engine(&fake_local(), &args)
}

#[test]
fn check_passes_json_through_unchanged() {
    let dir = TempDir::new();
    let out = served("check", &repo(&dir, "fail"), &["--format", "json"]);
    assert_eq!(out.code, 1, "exit code: {}", out.stderr);
    let report: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(report["passed"], false, "the server's Report");
    assert!(out.stdout.ends_with("}\n"), "verbatim");
    assert!(
        out.stderr.contains(" --mode check\n"),
        "the mode: {}",
        out.stderr
    );
    assert!(
        out.stderr.contains("fake-local: report "),
        "then the Report: {}",
        out.stderr
    );
    let passed = served("check", &repo(&dir, "pass"), &["--format", "json"]);
    assert_eq!(passed.code, 0, "passed exit code");
}

#[test]
fn check_renders_text_by_default() {
    let dir = TempDir::new();
    let root = repo(&dir, "fail");
    let out = served("check", &root, &[]);
    assert_eq!(out.code, 1, "exit code; stderr: {}", out.stderr);
    let json = served("check", &root, &["--format", "json"]);
    assert_eq!(
        out.stdout,
        render_report::report(&serde_json::from_str(&json.stdout).unwrap()),
        "text"
    );
    assert!(
        out.stderr
            .contains("sloptopop check will upload to http://ok.test/v1/jobs:"),
        "the manifest first: {}",
        out.stderr
    );
}

#[test]
fn fix_passes_apply_and_prints_the_summary() {
    let dir = TempDir::new();
    let root = repo(&dir, "plain");
    let applied = served("fix", &root, &["--apply"]);
    assert!(applied.stderr.contains(" --mode fix\n"), "the mode");
    assert!(
        applied.stderr.contains("fake-local: apply ") && applied.stderr.contains(" --apply\n"),
        "--apply passed on: {}",
        applied.stderr
    );
    assert_eq!(
        (applied.code, applied.stdout.as_str()),
        (0, "# summary of 0123456789abcdef\n"),
        "the summary"
    );
    let dry = served("fix", &repo(&dir, "nosummary"), &[]);
    assert_eq!(dry.code, 1, "a dry run is not complete in the fake");
    assert!(
        dry.stdout
            .starts_with("{\"schemaVersion\":2,\"args\":\"apply "),
        "report.json without a summary: {}",
        dry.stdout
    );
}

#[test]
fn local_problems_exit_3() {
    let dir = TempDir::new();
    for command in ["check", "fix"] {
        let garbage = served(command, &repo(&dir, "garbage"), &[]);
        assert_eq!(garbage.code, ENGINE_ERROR, "{command}");
        assert!(garbage.stdout.is_empty(), "{command} stdout");
        assert!(
            garbage
                .stderr
                .contains("error: sloptopop-local printed invalid JSON: "),
            "{command}: {}",
            garbage.stderr
        );
        let broken = served(command, &repo(&dir, "broken"), &[]);
        assert_eq!(broken.code, ENGINE_ERROR, "{command}");
        assert!(
            broken.stderr.contains("error: sloptopop-local failed"),
            "{command}: {}",
            broken.stderr
        );
    }
    let failing = served("fix", &repo(&dir, "failing"), &[]);
    assert_eq!(failing.code, ENGINE_ERROR, "apply fails");
    let bad_report = served("check", &repo(&dir, "badreport"), &[]);
    assert!(
        bad_report.code == ENGINE_ERROR && bad_report.stderr.contains("printed invalid JSON"),
        "a Report that is not JSON: {}",
        bad_report.stderr
    );
    let bad_answer = served("check", &repo(&dir, "badanswer"), &["--format", "json"]);
    assert_eq!(bad_answer.code, ENGINE_ERROR, "report fails");
    let missing = run_engine(&Locator::default(), &["check"]);
    assert_eq!(
        (missing.code, missing.stderr.as_str()),
        (ENGINE_ERROR, NOT_FOUND),
        "missing"
    );
}

#[test]
fn check_without_a_server_says_how_to_get_one() {
    let dir = TempDir::new();
    let out = run_engine(
        &fake_local(),
        &[
            "check",
            &repo(&dir, "pass"),
            "--backend",
            "http://down.test",
            "--yes",
        ],
    );
    assert_eq!(out.code, 4, "no server: no check");
    assert!(
        out.stderr.contains("hint: sloptopop detects only on its server: use the hosted service (https://api.sloptopop.dev, the default) or your self-hosted sloptopop-engine with --backend <url>"),
        "{}",
        out.stderr
    );
}

#[test]
fn broken_stdout_during_check_exits_3() {
    let dir = TempDir::new();
    let root = repo(&dir, "pass");
    let mut err = Vec::new();
    let args: Vec<OsString> = [
        "check",
        root.as_str(),
        "--backend",
        "http://ok.test",
        "--yes",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    let code = super::run_with(&args, &fake_local(), &mut &b""[..], &mut Broken, &mut err);
    assert_eq!(code, ENGINE_ERROR, "write failure");
    assert!(
        String::from_utf8_lossy(&err).contains("\nerror: cannot write output: "),
        "message"
    );
}

#[test]
fn local_runs_sloptopop_local_with_the_arguments() {
    let out = run_engine(
        &explicit(std::path::Path::new(FAKE_LOCAL)),
        &["local", "version"],
    );
    assert_eq!(
        (out.code, out.stdout.as_str()),
        (0, "sloptopop-local 0.1.0\n"),
        "passed through"
    );
    let other = run_engine(
        &explicit(std::path::Path::new(FAKE_LOCAL)),
        &["local", "other"],
    );
    assert_eq!(other.code, 2, "its exit code");
}

#[test]
fn collect_is_dispatched_with_the_locator_settings() {
    let root = TempDir::new();
    let locator = Locator {
        path: Some(crate::tests::support::fake_tools("findings").into_os_string()),
        vars: vec![("SLOPTOPOP_REGISTRY".to_owned(), "reg.example".into())],
        ..Locator::default()
    };
    let root_arg = root.0.to_string_lossy().into_owned();
    let dry = run_engine(&locator, &["collect", &root_arg, "--dry-run"]);
    assert_eq!(dry.code, 0, "exit 0: {}", dry.stderr);
    assert!(
        dry.stdout
            .contains("  run      gitleaks             reg.example/analyzers/gitleaks@sha256:"),
        "the locator's registry: {}",
        dry.stdout
    );
    let host = run_engine(
        &locator,
        &["collect", &root_arg, "--dry-run", "--host-tools"],
    );
    assert!(
        host.stdout
            .contains("  run      gitleaks             gitleaks dir . --redact"),
        "found on the locator's PATH: {}",
        host.stdout
    );
    let unknown = run_engine(&locator, &["collect", &root_arg, "--only", "nope"]);
    assert_eq!(unknown.code, USAGE_ERROR, "unknown tool");
    assert!(unknown.stdout.is_empty(), "nothing on stdout");
    assert!(
        unknown.stderr.starts_with("error: unknown tool: nope"),
        "{}",
        unknown.stderr
    );
    assert!(unknown.stderr.ends_with(USAGE), "usage");
}

/// Scripts the fake server's answers to the polls of `root`'s run.
fn script(root: &str, answers: &str) -> std::path::PathBuf {
    let run = std::path::Path::new(root).join(".sloptopop/transform/0123456789abcdef");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::write(run.join("fake-curl.script"), answers).unwrap();
    run
}

#[test]
fn check_waits_in_the_queue_with_progress_on_stderr_only() {
    let dir = TempDir::new();
    let root = repo(&dir, "pass");
    let run = script(
        &root,
        "{\"state\":\"queued\",\"position\":2,\"running\":1}\n{\"state\":\"queued\",\"position\":1,\"running\":1}\n{\"state\":\"running\"}\n{\"state\":\"done\",\"response\":{\"answered\":true}}\n",
    );
    let out = served("check", &root, &["--format", "json"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let report: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(report["passed"], true, "stdout holds only the Report");
    assert!(
        out.stderr.contains(
            "waiting: 2 projects ahead of you\nwaiting: the server is busy, you are next\nrunning…\n"
        ),
        "{}",
        out.stderr
    );
    assert!(!out.stdout.contains("waiting"), "no progress on stdout");
    let log = std::fs::read_to_string(run.join("fake-curl.log")).unwrap();
    assert_eq!(
        log,
        "POST http://ok.test/v1/jobs\nGET http://ok.test/v1/jobs/j1\nGET http://ok.test/v1/jobs/j1\nGET http://ok.test/v1/jobs/j1\nGET http://ok.test/v1/jobs/j1\nDELETE http://ok.test/v1/jobs/j1\n",
        "submitted, polled, deleted"
    );
}

#[test]
fn a_failed_job_exits_4_and_is_deleted() {
    let dir = TempDir::new();
    let root = repo(&dir, "pass");
    let run = script(
        &root,
        "{\"state\":\"failed\",\"error\":{\"code\":\"secret.unmasked\",\"message\":\"`a` line 1\"}}\n",
    );
    let out = served("transform", &root, &[]);
    assert_eq!(out.code, 4, "{}", out.stderr);
    assert!(
        out.stderr
            .contains("error: the server refused the request: secret.unmasked: `a` line 1\n"),
        "{}",
        out.stderr
    );
    assert!(!out.stderr.contains("hint:"), "the server answered");
    let log = std::fs::read_to_string(run.join("fake-curl.log")).unwrap();
    assert!(log.ends_with("DELETE http://ok.test/v1/jobs/j1\n"), "{log}");
}

#[test]
fn a_failed_poll_does_not_end_the_wait_but_failing_ones_do() {
    let dir = TempDir::new();
    let root = repo(&dir, "pass");
    let run = script(
        &root,
        "x\n~502 -\n{\"state\":\"running\"}\n{\"state\":\"done\",\"response\":{\"answered\":true}}\n",
    );
    let out = served("check", &root, &[]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stderr.contains(
            "waiting: no answer from the server, try 1 of 5 in 0 s\nwaiting: no answer from the server, try 2 of 5 in 0 s\nrunning…\n"
        ),
        "{}",
        out.stderr
    );
    let log = std::fs::read_to_string(run.join("fake-curl.log")).unwrap();
    assert!(log.ends_with("DELETE http://ok.test/v1/jobs/j1\n"), "{log}");
    let lost = repo(&dir, "fail");
    let gone = script(&lost, "x\n");
    let given_up = served("check", &lost, &[]);
    assert_eq!(given_up.code, 4, "{}", given_up.stderr);
    assert!(
        given_up.stderr.contains(
            "error: gave up waiting for the job after 5 retries (the backend at http://ok.test/v1/jobs/j1 did not answer: "
        ) && given_up
            .stderr
            .contains("curl: (7) Failed to connect); run the command again\n"),
        "{}",
        given_up.stderr
    );
    let calls = std::fs::read_to_string(gone.join("fake-curl.log")).unwrap();
    assert!(
        calls.ends_with("DELETE http://ok.test/v1/jobs/j1\n"),
        "deleted: {calls}"
    );
}
