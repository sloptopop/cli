//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.

use std::ffi::OsString;
use std::fs;

use crate::cli::run_with;
use crate::tests::support::{TempDir, fake_local};

struct Output {
    code: u8,
    stdout: String,
    stderr: String,
}

/// Runs `sloptopop transform <root> <args>` with the fake `sloptopop-local`
/// and the fake curl.
fn transform(root: &str, args: &[&str]) -> Output {
    let mut all: Vec<OsString> = vec!["transform".into(), root.into()];
    all.extend(args.iter().map(OsString::from));
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = run_with(&all, &fake_local(), &mut &b""[..], &mut stdout, &mut stderr);
    Output {
        code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    }
}

fn root(dir: &TempDir, name: &str) -> String {
    let path = dir.0.join(name);
    fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn a_transform_applies_what_the_server_answered() {
    let dir = TempDir::new();
    let repo = root(&dir, "plain");
    let out = transform(
        &repo,
        &[
            "--apply",
            "--branch",
            "b",
            "--confirm",
            "att-1,att-2",
            "--by",
            "lead",
            "--backend",
            "http://ok.test",
            "--yes",
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(out.stdout, "# summary of 0123456789abcdef\n", "SUMMARY.md");
    assert!(out.stderr.contains(" --mode transform\n"), "the mode");
    assert!(
        out.stderr.contains(&format!(
            "fake-local: apply {repo} --run 0123456789abcdef --apply --branch b --confirm att-1,att-2 --by lead\n"
        )),
        "{}",
        out.stderr
    );
    let json = transform(
        &repo,
        &["--format", "json", "--backend", "http://ok.test", "--yes"],
    );
    assert_eq!(json.code, 1, "a dry run");
    assert!(
        json.stdout
            .starts_with("{\"schemaVersion\":2,\"args\":\"apply "),
        "report.json: {}",
        json.stdout
    );
    let bare = transform(
        &root(&dir, "nosummary"),
        &["--backend", "http://ok.test", "--yes"],
    );
    assert!(
        bare.stdout.starts_with("{\"schemaVersion\":2"),
        "no summary: the report"
    );
}

#[test]
fn failures_stop_the_transform() {
    let dir = TempDir::new();
    let declined = transform(&root(&dir, "plain"), &["--backend", "http://ok.test"]);
    assert_eq!(declined.code, 4, "not confirmed");
    let failing = transform(
        &root(&dir, "failing"),
        &["--backend", "http://ok.test", "--yes"],
    );
    assert_eq!(failing.code, 3, "apply fails");
    assert!(failing.stderr.contains("HEAD moved"), "{}", failing.stderr);
}
