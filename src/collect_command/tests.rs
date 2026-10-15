//! Unit tests for [`super`], with the committed fake analyzers in
//! `tests/fake-tools/` on the `PATH` handed to `collect`.

use std::ffi::OsString;
use std::io;

use serde_json::Value;

use super::*;
use crate::tests::support::{TempDir, fake_tools};

struct Run {
    code: Result<u8, String>,
    stdout: String,
    stderr: String,
}

fn options(root: &Path) -> CollectOptions {
    CollectOptions {
        root: root.to_string_lossy().into_owned(),
        ..CollectOptions::default()
    }
}

fn run_with(options: &CollectOptions, path: &OsStr) -> Run {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = collect(options, Some(path), &mut stdout, &mut stderr);
    Run {
        code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    }
}

fn findings() -> OsString {
    fake_tools("findings").into_os_string()
}

fn python_repo() -> TempDir {
    let root = TempDir::new();
    fs::write(root.0.join("pyproject.toml"), "").unwrap();
    root
}

fn record_of(root: &TempDir) -> Value {
    serde_json::from_str(
        &fs::read_to_string(root.0.join(".sloptopop/reports/.collect.json")).unwrap(),
    )
    .unwrap()
}

fn statuses(record: &Value) -> Vec<(String, String)> {
    record["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            (
                tool["name"].as_str().unwrap().to_owned(),
                tool["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn installed_tools_run_and_are_recorded() {
    let root = python_repo();
    fs::create_dir_all(root.0.join(RULES_DIR)).unwrap();
    let out = run_with(&options(&root.0), &findings());
    assert_eq!(out.code, Ok(0), "stderr: {}", out.stderr);
    assert!(
        out.stdout.starts_with(
            "sloptopop collect: 6 ran, 0 failed, 0 missing, 2 skipped (ecosystems: python)\n"
        ),
        "{}",
        out.stdout
    );
    let record = record_of(&root);
    assert_eq!(
        statuses(&record),
        pairs(&[
            ("gitleaks", "ran"),
            ("gitleaks-history", "skipped"),
            ("osv-scanner", "ran"),
            ("opengrep", "ran"),
            ("semgrep", "ran"),
            ("bandit", "ran"),
            ("ruff", "ran"),
            ("coverage.py", "skipped"),
        ]),
        "statuses"
    );
    assert_eq!(
        record["ecosystems"],
        serde_json::json!(["python"]),
        "ecosystems"
    );
    let reports = root.0.join(REPORTS_DIR);
    for file in [
        "gitleaks",
        "osv-scanner",
        "opengrep",
        "semgrep",
        "bandit",
        "ruff",
    ] {
        assert!(
            reports.join(format!("{file}.sarif")).is_file(),
            "{file} report"
        );
    }
    assert_eq!(
        fs::read_to_string(reports.join(".gitignore")).unwrap(),
        "*\n",
        "ignored"
    );
    let pack = root.0.join(PACK_DIR);
    let rules = root.0.join(RULES_DIR);
    let argv = record["tools"][4]["argv"].as_array().unwrap();
    let config: Vec<&Value> = argv
        .windows(2)
        .filter(|pair| pair[0] == "--config")
        .map(|pair| &pair[1])
        .collect();
    assert_eq!(
        config,
        [
            &Value::from(pack.to_string_lossy().into_owned()),
            &Value::from(rules.to_string_lossy().into_owned())
        ],
        "the built-in pack, then the local rules"
    );
    for rule in collect_pack::PACK {
        assert_eq!(
            fs::read_to_string(pack.join(rule.area).join(rule.file)).unwrap(),
            rule.text,
            "pack written: {}",
            rule.file
        );
    }
}

#[test]
fn explicit_rules_and_git_are_used() {
    let root = python_repo();
    let rules = TempDir::new();
    fs::create_dir(root.0.join(".git")).unwrap();
    let mut options = options(&root.0);
    options.rules = Some(rules.0.to_string_lossy().into_owned());
    options.only = vec!["semgrep".to_owned(), "gitleaks-history".to_owned()];
    let out = run_with(&options, &findings());
    assert_eq!(out.code, Ok(0), "stderr: {}", out.stderr);
    let record = record_of(&root);
    assert_eq!(
        statuses(&record),
        pairs(&[("gitleaks-history", "ran"), ("semgrep", "ran")]),
        "only the selected entries"
    );
    let argv = record["tools"][1]["argv"].as_array().unwrap();
    assert!(
        argv.windows(2)
            .any(|pair| pair[0] == "--config" && pair[1] == rules.0.to_string_lossy().into_owned()),
        "--rules"
    );
    assert_eq!(
        argv.iter().filter(|arg| *arg == "--config").count(),
        1,
        "only the given rules"
    );
    assert!(!root.0.join(PACK_DIR).exists(), "the pack is not written");
}

#[test]
fn missing_tools_never_fail_the_run() {
    let root = python_repo();
    let out = run_with(&options(&root.0), OsStr::new(""));
    assert_eq!(out.code, Ok(0), "exit 0");
    assert!(
        out.stdout
            .starts_with("sloptopop collect: 0 ran, 0 failed, 6 missing, 2 skipped"),
        "{}",
        out.stdout
    );
}

#[test]
fn a_dry_run_writes_nothing() {
    let root = python_repo();
    let mut options = options(&root.0);
    options.dry_run = true;
    let out = run_with(&options, &findings());
    assert_eq!(out.code, Ok(0), "exit 0");
    assert!(
        out.stdout
            .starts_with("sloptopop collect: dry run, nothing run (ecosystems: python)\n"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("  run      ruff "), "{}", out.stdout);
    assert!(!root.0.join(".sloptopop").exists(), "nothing created");
    assert!(!root.0.join(".fake-calls").exists(), "nothing ran");
}

#[test]
fn containers_skip_unpinned_images() {
    let root = python_repo();
    let mut options = options(&root.0);
    options.container = true;
    options.only = vec!["gitleaks".to_owned(), "bandit".to_owned()];
    let out = run_with(&options, &findings());
    assert_eq!(out.code, Ok(0), "exit 0");
    let record = record_of(&root);
    assert_eq!(record["mode"], "container", "mode");
    assert_eq!(
        statuses(&record),
        pairs(&[("gitleaks", "skipped"), ("bandit", "skipped")]),
        "no digests are pinned yet"
    );
    assert!(!root.0.join(".fake-calls").exists(), "docker never ran");
}

#[test]
fn invalid_arguments_are_usage_errors() {
    let root = python_repo();
    let mut unknown = options(&root.0);
    unknown.only = vec!["ruff".to_owned(), "nope".to_owned()];
    let mut no_rules = options(&root.0);
    no_rules.rules = Some(root.0.join("missing").to_string_lossy().into_owned());
    let mut empty_rules = options(&root.0);
    empty_rules.rules = Some(String::new());
    let file = root.0.join("pyproject.toml").to_string_lossy().into_owned();
    for (options, message) in [
        (
            unknown,
            "unknown tool: nope (docs/collect.md lists them)".to_owned(),
        ),
        (
            options(Path::new(&file)),
            format!("not a directory: {file}"),
        ),
        (options(Path::new("")), "not a directory: ".to_owned()),
        (
            no_rules,
            format!(
                "rules directory not found: {}",
                root.0.join("missing").display()
            ),
        ),
        (empty_rules, "rules directory not found: ".to_owned()),
    ] {
        let out = run_with(&options, &findings());
        assert_eq!(out.code, Err(message), "{options:?}");
        assert!(
            out.stdout.is_empty() && out.stderr.is_empty(),
            "{options:?}"
        );
    }
}

#[test]
fn unwritable_reports_exit_1() {
    let blocked = python_repo();
    fs::write(blocked.0.join(".sloptopop"), "").unwrap();
    let out = run_with(&options(&blocked.0), &findings());
    assert_eq!(out.code, Ok(WRITE_ERROR), "reports directory");
    assert!(
        out.stderr.starts_with("error: cannot write "),
        "{}",
        out.stderr
    );
    assert!(out.stdout.is_empty(), "no summary");
    let pack = python_repo();
    fs::create_dir_all(pack.0.join(".sloptopop/reports")).unwrap();
    fs::write(pack.0.join(PACK_DIR), "").unwrap();
    let unpacked = run_with(&options(&pack.0), &findings());
    assert_eq!(unpacked.code, Ok(WRITE_ERROR), "pack");
    assert!(
        unpacked.stderr.starts_with(&format!(
            "error: cannot write {}: ",
            pack.0.join(PACK_DIR).display()
        )),
        "{}",
        unpacked.stderr
    );
    assert!(!pack.0.join(".fake-calls").exists(), "nothing ran");
    let record = python_repo();
    fs::create_dir_all(record.0.join(".sloptopop/reports/.collect.json")).unwrap();
    let unrecorded = run_with(&options(&record.0), &findings());
    assert_eq!(unrecorded.code, Ok(WRITE_ERROR), "record");
    assert!(
        unrecorded.stderr.contains(".collect.json: "),
        "{}",
        unrecorded.stderr
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
    let root = python_repo();
    let mut stderr = Vec::new();
    let code = collect(
        &options(&root.0),
        Some(&findings()),
        &mut Broken,
        &mut stderr,
    );
    assert_eq!(code, Ok(WRITE_ERROR), "exit 1");
    assert!(
        String::from_utf8_lossy(&stderr).starts_with("error: cannot write output: "),
        "message"
    );
}
