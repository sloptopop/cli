//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use serde_json::Value;

use super::*;
use crate::engine_run::{ENGINE_ERROR, NOT_FOUND};
use crate::render_report;
use crate::tests::support::{FAKE_ENGINE, explicit};

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

fn fake() -> Locator {
    explicit(std::path::Path::new(FAKE_ENGINE))
}

#[test]
fn check_passes_json_through_unchanged() {
    let out = run_engine(&fake(), &["check", "fail", "--format", "json"]);
    assert_eq!(out.code, 1, "engine exit code");
    let report: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(report["passed"], false, "the engine's document");
    assert!(
        out.stdout.ends_with("}\n"),
        "verbatim, with the engine's newline"
    );
    let passed = run_engine(&fake(), &["check", "pass", "--format", "json"]);
    assert_eq!(passed.code, 0, "passed exit code");
}

#[test]
fn check_renders_text_by_default() {
    let out = run_engine(&fake(), &["check", "fail"]);
    assert_eq!(out.code, 1, "engine exit code; stderr: {}", out.stderr);
    assert!(
        out.stderr.starts_with("fake-engine: check fail\n"),
        "path passed to the engine"
    );
    let json = run_engine(&fake(), &["check", "fail", "--format", "json"]);
    assert_eq!(
        out.stdout,
        render_report::report(&serde_json::from_str(&json.stdout).unwrap()),
        "text"
    );
}

#[test]
fn fix_passes_apply_and_renders() {
    let applied = run_engine(&fake(), &["fix", "fail", "--apply"]);
    assert!(
        applied
            .stderr
            .starts_with("fake-engine: fix fail --apply\n"),
        "--apply passed to the engine"
    );
    assert_eq!(
        (applied.code, applied.stdout.as_str()),
        (
            0,
            "sloptopop fix: applied\n  created  rust-toolchain.toml  (template rust-toolchain)\n"
        ),
        "apply"
    );
    let dry = run_engine(&fake(), &["fix", "pass"]);
    assert_eq!(
        (dry.code, dry.stdout.as_str()),
        (
            0,
            "sloptopop fix: dry run, nothing written\n  nothing to fix\n"
        ),
        "dry run"
    );
}

#[test]
fn engine_problems_exit_3() {
    for args in [&["check", "garbage"][..], &["fix", "garbage"]] {
        let out = run_engine(&fake(), args);
        assert_eq!(out.code, ENGINE_ERROR, "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?} stdout");
        assert!(
            out.stderr
                .contains("\nerror: sloptopop-engine printed invalid JSON: "),
            "{args:?}: {}",
            out.stderr
        );
    }
    for args in [&["check", "broken"][..], &["fix", "broken", "--apply"]] {
        let out = run_engine(&fake(), args);
        assert_eq!(out.code, ENGINE_ERROR, "{args:?}");
        assert!(
            out.stderr
                .contains("\nerror: boom\nerror: sloptopop-engine failed"),
            "{args:?}: {}",
            out.stderr
        );
    }
    let missing = run_engine(&Locator::default(), &["check"]);
    assert_eq!(
        (missing.code, missing.stderr.as_str()),
        (ENGINE_ERROR, NOT_FOUND),
        "missing"
    );
}

#[test]
fn broken_stdout_during_check_exits_3() {
    let mut err = Vec::new();
    let code = super::run_with(
        &[OsString::from("check"), OsString::from("pass")],
        &fake(),
        &mut &b""[..],
        &mut Broken,
        &mut err,
    );
    assert_eq!(code, ENGINE_ERROR, "write failure");
    assert!(
        String::from_utf8_lossy(&err).contains("\nerror: cannot write output: "),
        "message"
    );
}

#[test]
fn collect_is_dispatched_with_the_locator_settings() {
    let root = crate::tests::support::TempDir::new();
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
