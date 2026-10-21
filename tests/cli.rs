//! End-to-end checks for the `sloptopop` binary: the CLI commands, and
//! every case in `conformance/cases` run against `serve --stdio`.
//!
//! `check` and `fix` are tested against the committed fake engine in
//! `tests/fake-engine/`, which speaks the public Report contract. The real
//! engine is proprietary and is never needed here. `collect` is tested
//! against the committed fake container runtime in `tests/fake-runtime/`
//! (and, for `--host-tools`, the fake analyzers in `tests/fake-tools/`).

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use serde_json::Value;
use support::{
    FAKE_ENGINE, FAKE_ENGINE_DIR, FAKE_TOOLS, checked_inputs, collect, collect_fixture,
    collect_with, frame, matches, run, run_engine, scratch, unframe,
};

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

    /// Directory of the committed fake analyzers that report findings.
    pub(crate) const FAKE_TOOLS: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-tools/findings");

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

    /// The committed fake Docker (see `tests/fake-runtime/runtime`).
    pub(crate) const FAKE_RUNTIME: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-runtime/docker");

    /// A repository using Go, Node, Python and Swift, with git history and
    /// a local rules directory, so every non-coverage entry for those
    /// ecosystems applies and runs.
    pub(crate) fn collect_fixture(name: &str) -> PathBuf {
        let root = scratch(name);
        for manifest in ["go.mod", "package.json", "pyproject.toml", "Package.swift"] {
            fs::write(root.join(manifest), "").expect("manifest");
        }
        fs::create_dir_all(root.join(".git")).expect("git directory");
        fs::create_dir_all(root.join(".sloptopop/rules")).expect("rules directory");
        root
    }

    /// Runs `sloptopop` with `path` as `PATH`, the test registry and a
    /// cache in the scratch directory, and no other collect setting.
    pub(crate) fn collect_with(args: &[&str], path: &OsStr) -> Output {
        let cache = scratch("collect-cache");
        let mut command = Command::new(BIN);
        command
            .args(args)
            .env("PATH", path)
            .env("SLOPTOPOP_REGISTRY", "reg.example")
            .env("SLOPTOPOP_CACHE_DIR", cache)
            .env_remove("SLOPTOPOP_ENGINE");
        for name in [
            "SLOPTOPOP_CONTAINER_RUNTIME",
            "SLOPTOPOP_COLLECT_MEMORY",
            "SLOPTOPOP_COLLECT_CPUS",
            "GOPROXY",
        ] {
            command.env_remove(name);
        }
        command.output().expect("binary should run")
    }

    /// `PATH` with the fake Docker first.
    pub(crate) fn fake_docker() -> std::ffi::OsString {
        let mut path = std::ffi::OsString::from(FAKE_RUNTIME);
        path.push(":/bin:/usr/bin");
        path
    }

    /// Runs `sloptopop collect` against the fake Docker, after writing the
    /// images' versions (read from a dry run) where the fake finds them.
    pub(crate) fn collect(args: &[&str], root: &Path) -> Output {
        let mut dry: Vec<&str> = args.to_vec();
        dry.push("--dry-run");
        let plan = collect_with(&dry, &fake_docker());
        let versions: String = String::from_utf8_lossy(&plan.stdout)
            .lines()
            .filter_map(|line| {
                let label = line.split("/analyzers/").nth(1)?.split('@').next()?;
                let version = line.split(" (").nth(1)?.split(',').next()?;
                Some(format!("{label} {version}\n"))
            })
            .collect();
        fs::write(root.join(".fake-versions"), versions).expect("versions");
        collect_with(args, &fake_docker())
    }

    /// The report paths the fake engine's `check` read in `root`.
    pub(crate) fn checked_inputs(root: &str) -> Vec<String> {
        let check = run_engine(
            &["check", root, "--format", "json"],
            Some(OsStr::new(FAKE_ENGINE)),
            OsStr::new(""),
        );
        assert_eq!(check.status.code(), Some(0), "check exit code");
        let report: Value = serde_json::from_slice(&check.stdout).expect("Report");
        report["analysis"]["inputs"]
            .as_array()
            .expect("inputs")
            .iter()
            .map(|input| input["path"].as_str().expect("path").to_owned())
            .collect()
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

/// Every entry that runs on the fixture, with the exit code the fake
/// runtime's tool reports findings with and the report it writes.
const RAN: [(&str, i64, &str); 21] = [
    ("gitleaks", 0, "gitleaks.sarif"),
    ("gitleaks-history", 0, "gitleaks.history.sarif"),
    ("osv-scanner", 1, "osv-scanner.sarif"),
    ("opengrep", 0, "opengrep.sarif"),
    ("bandit", 1, "bandit.sarif"),
    ("gosec", 1, "gosec.sarif"),
    ("govulncheck", 0, "govulncheck.sarif"),
    ("ruff", 1, "ruff.sarif"),
    ("pylint-cyclic-import", 0, "pylint-cyclic-import.sarif"),
    ("oxlint", 1, "oxlint.sarif"),
    ("biome", 1, "biome.sarif"),
    ("eslint-sonarjs", 1, "eslint-sonarjs.sarif"),
    ("eslint-typescript", 1, "eslint-typescript.sarif"),
    ("dependency-cruiser", 0, "dependency-cruiser.sarif"),
    ("staticcheck", 1, "staticcheck.sarif"),
    ("golangci-lint", 1, "golangci.sarif"),
    ("swiftlint", 2, "swiftlint.sarif"),
    ("scorecard-license", 0, "scorecard-license.sarif"),
    ("jscpd", 0, "jscpd.sarif"),
    ("coverage.py", -1, ""),
    ("c8", -1, ""),
];

#[test]
fn collect_runs_pinned_images_and_check_reads_their_reports() {
    let root = collect_fixture("collect-full");
    let root_arg = root.to_str().expect("UTF-8 path");
    let out = collect(&["collect", root_arg], &root);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "exit 0: {stdout}");
    assert!(
        stdout.contains(
            "sloptopop collect: 19 ran, 0 failed, 0 missing, 2 skipped (ecosystems: go, node, python, swift)\n"
        ),
        "{stdout}"
    );
    assert!(
        stdout.starts_with("fetch    gitleaks "),
        "announced first: {stdout}"
    );
    let reports = root.join(".sloptopop/reports");
    let record: Value =
        serde_json::from_str(&fs::read_to_string(reports.join(".collect.json")).expect("record"))
            .expect("record is JSON");
    assert_eq!(record["mode"], "container");
    let tools = record["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), RAN.len(), "{record:#}");
    for ((name, code, file), tool) in RAN.iter().zip(tools) {
        assert_eq!(tool["name"], *name, "order");
        if *code < 0 {
            assert_eq!(tool["status"], "skipped", "{name}");
            continue;
        }
        assert_eq!(tool["status"], "ran", "{name}: {tool}");
        assert_eq!(tool["exitCode"], *code, "{name}");
        assert_eq!(
            tool["output"],
            format!(".sloptopop/reports/{file}"),
            "{name}"
        );
        assert!(reports.join(file).is_file(), "{name}: report written");
        assert!(
            tool["image"]["reference"]
                .as_str()
                .expect("image")
                .starts_with("reg.example/analyzers/"),
            "{name}"
        );
    }
    assert_eq!(
        fs::read_to_string(reports.join(".gitignore")).expect("ignored"),
        "*\n",
        "reports ignored"
    );
    let first = fs::read(reports.join(".collect.json")).expect("record");
    let again = collect(&["collect", root_arg], &root);
    assert_eq!(
        fs::read(reports.join(".collect.json")).expect("record"),
        first,
        "same record"
    );
    let again_text = String::from_utf8_lossy(&again.stdout);
    assert!(
        !again_text.contains("fetch    gitleaks"),
        "images cached: {again_text}"
    );
    assert!(
        again_text.contains("fetch    db-osv-pypi"),
        "the newest snapshot is checked: {again_text}"
    );
    let inputs = checked_inputs(root_arg);
    let mut expected: Vec<String> = RAN
        .iter()
        .filter(|(_, code, _)| *code >= 0)
        .map(|(_, _, file)| format!(".sloptopop/reports/{file}"))
        .collect();
    expected.sort();
    assert_eq!(
        inputs, expected,
        "check reads every report and no hidden file"
    );
}

#[test]
fn collect_mounts_the_built_in_pack_then_the_local_rules() {
    let root = collect_fixture("collect-pack");
    let out = collect(
        &[
            "collect",
            root.to_str().expect("UTF-8"),
            "--only",
            "semgrep",
        ],
        &root,
    );
    assert_eq!(out.status.code(), Some(0), "exit 0");
    let calls = fs::read_to_string(root.join(".fake-runtime-calls")).expect("semgrep ran");
    let pack = root.join(".sloptopop/reports/.rules");
    assert!(
        calls.contains(&format!(
            "--mount type=bind,source={},target=/rules/0,readonly --mount type=bind,source={},target=/rules/1,readonly",
            pack.display(),
            root.join(".sloptopop/rules").display()
        )),
        "{calls}"
    );
    assert!(calls.contains("scan --json --metrics off --disable-version-check --no-rewrite-rule-ids --config /rules/0 --config /rules/1 ."), "{calls}");
    assert!(
        pack.join("supabase/unchecked-write.yaml").is_file(),
        "the pack is written"
    );
}

#[test]
fn collect_leaves_ignoring_to_the_root_gitignore() {
    let root = collect_fixture("collect-ignored");
    fs::write(root.join(".gitignore"), "/.sloptopop/\n").expect("gitignore");
    let out = collect(
        &[
            "collect",
            root.to_str().expect("UTF-8"),
            "--only",
            "ruff,gitleaks",
        ],
        &root,
    );
    assert_eq!(out.status.code(), Some(0), "exit 0");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("sloptopop collect: 2 ran, "),
        "only the selected tools"
    );
    assert!(
        !root.join(".sloptopop/reports/.gitignore").exists(),
        "not created"
    );
}

#[test]
fn collect_dry_run_and_usage_errors() {
    let root = collect_fixture("collect-dry");
    let root_arg = root.to_str().expect("UTF-8");
    let dry = collect_with(
        &["collect", root_arg, "--dry-run", "--only", "osv-scanner"],
        OsStr::new(""),
    );
    assert_eq!(dry.status.code(), Some(0), "exit 0");
    let text = String::from_utf8_lossy(&dry.stdout);
    assert!(text.starts_with("sloptopop collect: dry run, nothing fetched or run (ecosystems: go, node, python, swift)\n  run      osv-scanner          reg.example/analyzers/osv-scanner@sha256:"), "{text}");
    assert!(text.ends_with("osv-scanner scan source -r --offline --format sarif . > .sloptopop/reports/osv-scanner.sarif\n"), "{text}");
    assert!(!root.join(".sloptopop/reports").exists(), "nothing written");
    for args in [
        &["collect", root_arg, "--only", "nope"][..],
        &["collect", root_arg, "--rules", "/nonexistent-rules"],
        &["collect", "--bogus"],
    ] {
        let out = collect_with(args, OsStr::new(""));
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn collect_without_a_runtime_exits_3() {
    let root = collect_fixture("collect-no-runtime");
    let out = collect_with(
        &["collect", root.to_str().expect("UTF-8")],
        OsStr::new("/nonexistent"),
    );
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("error: no container runtime found"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Tried: docker (not on PATH), podman (not on PATH)."),
        "{stderr}"
    );
    assert!(
        root.join(".sloptopop/reports/.collect.json").is_file(),
        "recorded"
    );
}

#[test]
fn collect_host_tools_runs_unpinned_tools_with_a_warning() {
    let root = collect_fixture("collect-host");
    let out = collect_with(
        &[
            "collect",
            root.to_str().expect("UTF-8"),
            "--host-tools",
            "--only",
            "ruff",
        ],
        OsStr::new(FAKE_TOOLS),
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .starts_with("warning: --host-tools runs unpinned tool versions")
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("  ran      ruff "),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}
