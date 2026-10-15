//! End-to-end checks for the `sloptopop` binary: the CLI commands, and
//! every case in `conformance/cases` run against `serve --stdio`.
//!
//! `check` and `fix` are tested against the committed fake engine in
//! `tests/fake-engine/`, which speaks the public Report contract. The real
//! engine is proprietary and is never needed here.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use serde_json::Value;
use support::{FAKE_ENGINE, FAKE_ENGINE_DIR, frame, matches, run, run_engine, scratch, unframe};

/// Helpers shared by the tests below.
#[cfg(test)]
mod support {
    use std::ffi::OsStr;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};

    use serde_json::Value;

    const BIN: &str = env!("CARGO_BIN_EXE_sloptopop");

    /// Runs `sloptopop` with `SLOPTOPOP_ENGINE` set to `engine` (or unset)
    /// and `PATH` set to `path`.
    pub(crate) fn run_engine(args: &[&str], engine: Option<&OsStr>, path: &OsStr) -> Output {
        let mut command = Command::new(BIN);
        command
            .args(args)
            .env("PATH", path)
            .env_remove("SLOPTOPOP_ENGINE");
        if let Some(engine) = engine {
            command.env("SLOPTOPOP_ENGINE", engine);
        }
        command.output().expect("binary should run")
    }

    /// Directory holding the committed fake engine, named like the real one.
    pub(crate) const FAKE_ENGINE_DIR: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-engine");

    /// The committed fake engine.
    pub(crate) const FAKE_ENGINE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fake-engine/sloptopop-engine"
    );

    /// A fresh directory under Cargo's per-target temporary directory.
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        let _stale = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch directory");
        dir
    }

    pub(crate) fn run(args: &[&str], stdin: &[u8]) -> Output {
        let mut child = Command::new(BIN)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("binary should start");
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(stdin)
            .expect("stdin accepts input");
        child.wait_with_output().expect("binary should finish")
    }

    pub(crate) fn frame(body: &[u8]) -> Vec<u8> {
        let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        out.extend_from_slice(body);
        out
    }

    /// Splits stdout into frame bodies, parsed as JSON.
    pub(crate) fn unframe(mut bytes: &[u8]) -> Vec<Value> {
        let mut messages = Vec::new();
        while !bytes.is_empty() {
            let text = String::from_utf8_lossy(bytes);
            let (header, _) = text.split_once("\r\n\r\n").expect("complete header");
            let len: usize = header
                .strip_prefix("Content-Length: ")
                .expect("Content-Length header")
                .parse()
                .expect("numeric length");
            let start = header.len() + 4;
            let body = bytes.get(start..start + len).expect("complete body");
            messages.push(serde_json::from_slice(body).expect("JSON body"));
            bytes = bytes.get(start + len..).expect("rest of stream");
        }
        messages
    }

    /// Applies the conformance placeholders to an expected reply.
    pub(crate) fn matches(expected: &Value, actual: &Value) -> bool {
        match (expected, actual) {
            (Value::String(s), Value::String(a)) if s == "$SERVER_VERSION" => {
                a == env!("CARGO_PKG_VERSION")
            }
            (Value::Object(e), Value::Object(a)) => {
                if let Some(prefix) = e.get("messagePrefix").and_then(Value::as_str) {
                    let message = a.get("message").and_then(Value::as_str).unwrap_or("");
                    return message.starts_with(prefix)
                        && e.iter()
                            .filter(|(k, _)| *k != "messagePrefix")
                            .all(|(k, v)| a.get(k).is_some_and(|av| matches(v, av)))
                        && a.len() == e.len();
                }
                e.len() == a.len()
                    && e.iter()
                        .all(|(k, v)| a.get(k).is_some_and(|av| matches(v, av)))
            }
            (Value::Array(e), Value::Array(a)) => {
                e.len() == a.len() && e.iter().zip(a).all(|(ev, av)| matches(ev, av))
            }
            _ => expected == actual,
        }
    }
}

#[test]
fn conformance_cases_pass() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/cases");
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .expect("cases directory")
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no conformance cases found");
    for path in paths {
        let case: Value =
            serde_json::from_slice(&fs::read(&path).expect("readable case")).expect("JSON case");
        let mut stdin = Vec::new();
        for message in case["input"].as_array().expect("input array") {
            let body = match message.get("$raw").and_then(Value::as_str) {
                Some(raw) => raw.as_bytes().to_vec(),
                None => message.to_string().into_bytes(),
            };
            stdin.extend(frame(&body));
        }
        let out = run(&["serve", "--stdio"], &stdin);
        let replies = Value::Array(unframe(&out.stdout));
        let name = path.display();
        assert!(
            matches(&case["output"], &replies),
            "{name}: got {replies:#}"
        );
        assert_eq!(
            out.status.code().map(i64::from),
            case["exitCode"].as_i64(),
            "{name}: exit code"
        );
    }
}

#[test]
fn version_prints_version() {
    let out = run(&["--version"], b"");
    assert!(out.status.success(), "exit 0");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("sloptopop {} (protocol 1)\n", env!("CARGO_PKG_VERSION")),
        "version line"
    );
}

#[test]
fn schema_matches_committed_file() {
    let out = run(&["schema"], b"");
    let printed: Value = serde_json::from_slice(&out.stdout).expect("schema is JSON");
    let committed: Value =
        serde_json::from_str(include_str!("../schema/protocol.json")).expect("committed JSON");
    assert_eq!(printed, committed, "schema output");
}

#[test]
fn usage_error_exits_2() {
    let out = run(&["frobnicate"], b"");
    assert_eq!(out.status.code(), Some(2), "exit 2");
    assert!(out.stdout.is_empty(), "nothing on stdout");
    assert!(
        String::from_utf8_lossy(&out.stderr).starts_with("error: unrecognised arguments"),
        "message on stderr"
    );
}

#[test]
fn framing_error_exits_1() {
    let out = run(&["serve", "--stdio"], b"garbage\r\n\r\n");
    assert_eq!(out.status.code(), Some(1), "exit 1");
    assert!(
        String::from_utf8_lossy(&out.stderr).starts_with("error: invalid header line"),
        "message on stderr"
    );
}

fn fake(args: &[&str]) -> std::process::Output {
    run_engine(args, Some(OsStr::new(FAKE_ENGINE)), OsStr::new(""))
}

#[test]
fn check_runs_the_engine_named_by_the_environment() {
    let json = fake(&["check", "fail", "--format", "json"]);
    assert_eq!(json.status.code(), Some(1), "engine exit code");
    let expected = std::process::Command::new(FAKE_ENGINE)
        .args(["check", "fail"])
        .output()
        .expect("fake engine runs");
    assert_eq!(
        json.stdout, expected.stdout,
        "Report passed through unchanged"
    );
    assert!(
        String::from_utf8_lossy(&json.stderr).starts_with("fake-engine: check fail\n"),
        "engine stderr forwarded"
    );
    let text = fake(&["check", "fail"]);
    assert_eq!(text.status.code(), Some(1), "text exit code");
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(
        text.starts_with("sloptopop check: FAILED (3 findings; ecosystems: rust)"),
        "{text}"
    );
    assert!(
        text.contains("src/a.py:1  analyzer.finding\n    ruff F401: `os` imported but unused"),
        "a finding with an origin and a report with an analysis render: {text}"
    );
    assert!(
        text.contains("rust-toolchain.toml  toolchain.unpinned  [STD-1]"),
        "{text}"
    );
    assert!(text.contains("src/a.py:3  errors.swallowed"), "{text}");
    assert!(text.contains("STD-2   notChecked  Formatting"), "{text}");
    let passed = fake(&["check", "pass"]);
    assert_eq!(passed.status.code(), Some(0), "passed exit code");
    assert!(
        String::from_utf8_lossy(&passed.stdout).starts_with("sloptopop check: passed"),
        "passed text"
    );
}

#[test]
fn fix_renders_the_plan_and_passes_apply() {
    let dry = fake(&["fix", "fail"]);
    assert_eq!(dry.status.code(), Some(0), "dry run exit code");
    assert_eq!(
        String::from_utf8_lossy(&dry.stdout),
        "sloptopop fix: dry run, nothing written\n  exists   .gitignore  (template gitignore)\n  planned  rust-toolchain.toml  (template rust-toolchain)\nRun `sloptopop fix --apply` to create the planned files.\n",
        "dry run text"
    );
    let applied = fake(&["fix", "fail", "--apply"]);
    assert!(
        String::from_utf8_lossy(&applied.stderr).starts_with("fake-engine: fix fail --apply\n"),
        "--apply passed to the engine"
    );
    assert_eq!(
        String::from_utf8_lossy(&applied.stdout),
        "sloptopop fix: applied\n  created  rust-toolchain.toml  (template rust-toolchain)\n",
        "applied text"
    );
}

#[test]
fn engine_on_path_is_found() {
    let out = run_engine(&["fix", "pass"], None, OsStr::new(FAKE_ENGINE_DIR));
    assert_eq!(out.status.code(), Some(0), "exit 0");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "sloptopop fix: dry run, nothing written\n  nothing to fix\n",
        "rendered"
    );
}

#[test]
fn engine_failures_exit_3() {
    for (mode, message) in [
        ("garbage", "error: sloptopop-engine printed invalid JSON: "),
        (
            "broken",
            "error: boom\nerror: sloptopop-engine failed (exit status: 3)",
        ),
        ("killed", "error: sloptopop-engine failed (signal: 9"),
        ("other", "error: sloptopop-engine failed (exit status: 2)"),
    ] {
        let out = fake(&["check", mode]);
        assert_eq!(out.status.code(), Some(3), "{mode}: exit 3");
        assert!(out.stdout.is_empty(), "{mode}: nothing on stdout");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.starts_with(&format!("fake-engine: check {mode}\n")),
            "{mode}: engine stderr forwarded first"
        );
        assert!(stderr.contains(message), "{mode}: {stderr}");
    }
}

#[test]
fn missing_engine_exits_3_with_a_hint() {
    let empty = scratch("no-engine");
    let out = run_engine(&["check"], None, empty.as_os_str());
    assert_eq!(out.status.code(), Some(3), "exit 3");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("error: sloptopop-engine not found"),
        "{stderr}"
    );
    assert!(stderr.contains("SLOPTOPOP_ENGINE"), "hint");
}
