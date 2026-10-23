//! End-to-end checks for the `sloptopop` binary: the CLI commands, and
//! every case in `conformance/cases` run against `serve --stdio`.
//!
//! `check`, `fix` and `transform` are tested against the committed fake
//! `sloptopop-local` in `tests/fake-local/` and the fake curl in
//! `tests/fake-curl/`: no server is ever needed here. `collect` is tested
//! against the committed fake container runtime in `tests/fake-runtime/`
//! (and, for `--host-tools`, the fake analyzers in `tests/fake-tools/`).

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use serde_json::Value;
use support::{
    FAKE_LOCAL, FAKE_LOCAL_DIR, FAKE_TOOLS, checked_inputs, collect, collect_fixture, collect_with,
    frame, matches, run, run_engine, scratch, unframe,
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

    /// Runs `sloptopop` with `SLOPTOPOP_LOCAL` set to `engine` (or unset)
    /// and `PATH` set to `path`.
    pub(crate) fn run_engine(args: &[&str], engine: Option<&OsStr>, path: &OsStr) -> Output {
        let mut command = Command::new(BIN);
        command
            .args(args)
            .env("PATH", path)
            .env_remove("SLOPTOPOP_LOCAL");
        if let Some(engine) = engine {
            command.env("SLOPTOPOP_LOCAL", engine);
        }
        command.output().expect("binary should run")
    }

    /// Directory holding the committed fake `sloptopop-local`, named like
    /// the real one.
    pub(crate) const FAKE_LOCAL_DIR: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-local");

    /// `PATH` with the fake curl first, then the system's tools.
    pub(crate) const CURL_PATH: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-curl:/bin:/usr/bin");

    /// Directory of the committed fake analyzers that report findings.
    pub(crate) const FAKE_TOOLS: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-tools/findings");

    /// The committed fake `sloptopop-local`.
    pub(crate) const FAKE_LOCAL: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fake-local/sloptopop-local"
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
            .env_remove("SLOPTOPOP_LOCAL");
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

    /// The report paths the fake `sloptopop-local`'s Report lists for `root`.
    pub(crate) fn checked_inputs(root: &str) -> Vec<String> {
        let check = run_engine(
            &[
                "check",
                root,
                "--format",
                "json",
                "--backend",
                "http://ok.test",
                "--yes",
            ],
            Some(OsStr::new(FAKE_LOCAL)),
            OsStr::new(CURL_PATH),
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

/// `sloptopop <args> --backend http://ok.test --yes` with the fake
/// `sloptopop-local` named by the environment and the fake curl on `PATH`.
fn fake(args: &[&str]) -> std::process::Output {
    let mut all = args.to_vec();
    all.extend(["--backend", "http://ok.test", "--yes"]);
    run_engine(
        &all,
        Some(OsStr::new(FAKE_LOCAL)),
        OsStr::new(support::CURL_PATH),
    )
}

#[test]
fn check_runs_sloptopop_local_named_by_the_environment() {
    let root = scratch("check-fail").join("fail");
    fs::create_dir_all(&root).expect("root");
    let root = root.to_str().expect("UTF-8");
    let json = fake(&["check", root, "--format", "json"]);
    assert_eq!(json.status.code(), Some(1), "exit code");
    let report: Value = serde_json::from_slice(&json.stdout).expect("Report");
    assert_eq!(
        report["passed"], false,
        "the server's Report, passed through"
    );
    assert!(
        String::from_utf8_lossy(&json.stderr)
            .starts_with(&format!("fake-local: request {root} --mode check\n")),
        "stderr forwarded"
    );
    let text = fake(&["check", root]);
    assert_eq!(text.status.code(), Some(1), "text exit code");
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(
        text.starts_with("sloptopop check: FAILED (3 findings; ecosystems: rust)"),
        "{text}"
    );
    assert!(
        text.contains("rust-toolchain.toml  toolchain.unpinned  [STD-1]"),
        "{text}"
    );
    assert!(text.contains("src/a.ts:9  errors.swallowed"), "{text}");
    let passed = scratch("check-pass");
    let out = fake(&["check", passed.to_str().expect("UTF-8")]);
    assert_eq!(out.status.code(), Some(0), "passed exit code");
    assert!(
        String::from_utf8_lossy(&out.stdout).starts_with("sloptopop check: passed"),
        "passed text"
    );
}

#[test]
fn fix_prints_the_summary_and_passes_apply() {
    let root = scratch("fix-plain");
    let root = root.to_str().expect("UTF-8");
    let applied = fake(&["fix", root, "--apply"]);
    assert!(
        String::from_utf8_lossy(&applied.stderr).contains(&format!(
            "fake-local: apply {root} --run 0123456789abcdef --apply\n"
        )),
        "--apply passed on"
    );
    assert_eq!(
        (
            applied.status.code(),
            String::from_utf8_lossy(&applied.stdout).into_owned()
        ),
        (Some(0), "# summary of 0123456789abcdef\n".to_owned()),
        "the summary"
    );
}

#[test]
fn local_on_path_is_found() {
    let mut path = std::ffi::OsString::from(FAKE_LOCAL_DIR);
    path.push(":");
    path.push(support::CURL_PATH);
    let out = run_engine(&["local", "version"], None, &path);
    assert_eq!(out.status.code(), Some(0), "exit 0");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "sloptopop-local 0.1.0\n",
        "passed through"
    );
}

#[test]
fn local_failures_exit_3() {
    let dir = scratch("local-failures");
    for (mode, message) in [
        ("garbage", "error: sloptopop-local printed invalid JSON: "),
        (
            "broken",
            "error: the working tree has uncommitted or untracked changes\nerror: sloptopop-local failed (exit status: 3)",
        ),
    ] {
        let root = dir.join(mode);
        fs::create_dir_all(&root).expect("root");
        let out = fake(&["check", root.to_str().expect("UTF-8")]);
        assert_eq!(out.status.code(), Some(3), "{mode}: exit 3");
        assert!(out.stdout.is_empty(), "{mode}: nothing on stdout");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(message), "{mode}: {stderr}");
    }
}

#[test]
fn missing_local_exits_3_with_a_hint() {
    let empty = scratch("no-local");
    let out = run_engine(&["check"], None, empty.as_os_str());
    assert_eq!(out.status.code(), Some(3), "exit 3");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("error: sloptopop-local not found"),
        "{stderr}"
    );
    assert!(stderr.contains("SLOPTOPOP_LOCAL"), "hint");
}

/// Every entry that runs on the fixture, with the exit code the fake
/// runtime's tool reports findings with and the report it writes.
const RAN: [(&str, i64, &str); 23] = [
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
    ("golangci-lint-defects", 1, "golangci-defects.sarif"),
    ("swiftlint", 2, "swiftlint.sarif"),
    ("swiftlint-defects", 2, "swiftlint-defects.sarif"),
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
            "sloptopop collect: 21 ran, 0 failed, 0 missing, 2 skipped (ecosystems: go, node, python, swift)\n"
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

#[test]
fn ctrl_c_while_queued_deletes_the_job() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let root = scratch("ctrl-c").join("pass");
    let run = root.join(".sloptopop/transform/0123456789abcdef");
    fs::create_dir_all(&run).expect("run directory");
    fs::write(
        run.join("fake-curl.script"),
        "{\"state\":\"queued\",\"position\":3,\"running\":1}\n",
    )
    .expect("script");
    let mut child = Command::new(env!("CARGO_BIN_EXE_sloptopop"))
        .args([
            "check",
            root.to_str().expect("UTF-8"),
            "--backend",
            "http://ok.test",
            "--yes",
        ])
        .env("PATH", support::CURL_PATH)
        .env("SLOPTOPOP_LOCAL", FAKE_LOCAL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary should run");
    let mut stderr = BufReader::new(child.stderr.take().expect("stderr"));
    let mut line = String::new();
    while !line.starts_with("waiting:") {
        line.clear();
        assert!(
            stderr.read_line(&mut line).expect("stderr") > 0,
            "no queue line"
        );
    }
    assert_eq!(
        line, "waiting: 3 projects ahead of you\n",
        "the place in line"
    );
    let interrupted = Command::new("sh")
        .args(["-c", &format!("kill -INT {}", child.id())])
        .status()
        .expect("kill");
    assert!(interrupted.success(), "kill -INT");
    let status = child.wait().expect("exit");
    assert_eq!(
        status.signal(),
        Some(2),
        "stopped by SIGINT: the shell's 130"
    );
    let log = run.join("fake-curl.log");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let text = fs::read_to_string(&log).unwrap_or_default();
        if text.ends_with("DELETE http://ok.test/v1/jobs/j1\n") {
            break;
        }
        assert!(Instant::now() < deadline, "never deleted: {text}");
        std::thread::park_timeout(Duration::from_millis(20));
    }
}
