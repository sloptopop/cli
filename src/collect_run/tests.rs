//! Unit tests for [`super`], with the committed fake analyzers in
//! `tests/fake-tools/` standing in for the real ones.

use std::fs;
use std::path::PathBuf;

use super::*;
use crate::collect_catalogue::CATALOGUE;
use crate::tests::support::{TempDir, fake_tools};

fn named(name: &str) -> &'static Tool {
    CATALOGUE.iter().find(|tool| tool.name == name).unwrap()
}

fn root() -> TempDir {
    let root = TempDir::new();
    fs::create_dir_all(root.0.join(".sloptopop/reports")).unwrap();
    root
}

fn planned(tool: &'static Tool, program: PathBuf, stdout: Option<PathBuf>) -> Planned<'static> {
    Planned {
        tool,
        argv: tool.argv("<rules>"),
        step: Step::Run {
            program,
            version: vec!["--version".to_owned()],
            stdout,
        },
    }
}

fn failed(status: Status) -> (Option<i32>, String) {
    let Status::Failed { code, detail } = status else {
        panic!("{status:?}");
    };
    (code, detail)
}

fn calls(root: &TempDir) -> String {
    fs::read_to_string(root.0.join(".fake-calls")).unwrap_or_default()
}

#[test]
fn exit_codes_map_to_statuses() {
    let unused = || -> String { panic!("no detail for success") };
    assert_eq!(status(Some(0), &[], unused), Status::Ran(0), "success");
    assert_eq!(status(Some(1), &[2, 1], unused), Status::Ran(1), "findings");
    let detail = || "boom".to_owned();
    assert_eq!(
        status(Some(2), &[1], detail),
        Status::Failed {
            code: Some(2),
            detail: "boom".to_owned()
        },
        "other code"
    );
    assert_eq!(
        status(None, &[1], detail),
        Status::Failed {
            code: None,
            detail: "boom".to_owned()
        },
        "signal"
    );
}

#[test]
fn statuses_expose_name_code_and_reason() {
    let failed = Status::Failed {
        code: Some(70),
        detail: "boom".to_owned(),
    };
    for (status, name, code, reason) in [
        (Status::Ran(1), "ran", Some(1), None),
        (failed, "failed", Some(70), Some("boom")),
        (Status::Missing("m".to_owned()), "missing", None, Some("m")),
        (Status::Skipped("s".to_owned()), "skipped", None, Some("s")),
    ] {
        assert_eq!(
            (status.name(), status.exit_code(), status.reason()),
            (name, code, reason),
            "{status:?}"
        );
    }
}

#[test]
fn a_tool_writing_its_own_report_runs_in_the_root() {
    let root = root();
    let outcome = run(
        planned(named("ruff"), fake_tools("findings").join("ruff"), None),
        &root.0,
    );
    assert_eq!(outcome.version.as_deref(), Some("ruff 1.0.0"), "first line");
    assert_eq!(outcome.status, Status::Ran(1), "findings are success");
    assert_eq!(outcome.argv, named("ruff").argv("<rules>"), "argv kept");
    assert_eq!(
        calls(&root),
        "ruff --version\nruff check --output-format sarif --output-file .sloptopop/reports/ruff.sarif --no-cache .\n",
        "argv, run in the root"
    );
    assert!(
        fs::read_to_string(root.0.join(".sloptopop/reports/ruff.sarif"))
            .unwrap()
            .contains("\"name\":\"ruff\""),
        "report written"
    );
}

#[test]
fn stdout_is_captured_into_the_report() {
    let root = root();
    let report = root.0.join(".sloptopop/reports/osv-scanner.sarif");
    let outcome = run(
        planned(
            named("osv-scanner"),
            fake_tools("findings").join("osv-scanner"),
            Some(report.clone()),
        ),
        &root.0,
    );
    assert_eq!(outcome.status, Status::Ran(1), "vulnerabilities found");
    assert_eq!(
        fs::read_to_string(report).unwrap(),
        "{\"version\":\"2.1.0\",\"runs\":[{\"tool\":{\"driver\":{\"name\":\"osv-scanner\"}},\"results\":[]}]}\n",
        "stdout"
    );
}

#[test]
fn failures_are_recorded() {
    let root = root();
    let broken = run(
        planned(named("ruff"), fake_tools("broken").join("ruff"), None),
        &root.0,
    );
    assert_eq!(broken.version, None, "failed --version");
    assert_eq!(
        broken.status,
        Status::Failed {
            code: Some(70),
            detail: "fake failure".to_owned()
        },
        "first stderr line"
    );
    let killed = run(
        planned(
            named("staticcheck"),
            fake_tools("killed").join("staticcheck"),
            Some(root.0.join(".sloptopop/reports/staticcheck.sarif")),
        ),
        &root.0,
    );
    assert_eq!(
        killed.version.as_deref(),
        Some("staticcheck 1.0.0"),
        "version"
    );
    let (code, detail) = failed(killed.status);
    assert_eq!(code, None, "no exit code");
    assert!(detail.contains("signal"), "{detail}");
}

#[test]
fn unwritable_reports_and_unstartable_tools_fail() {
    let root = root();
    let unwritable = run(
        planned(
            named("osv-scanner"),
            fake_tools("findings").join("osv-scanner"),
            Some(root.0.join("missing/osv-scanner.sarif")),
        ),
        &root.0,
    );
    let (code, detail) = failed(unwritable.status);
    assert_eq!(code, None, "not run");
    assert!(detail.starts_with("cannot write the report: "), "{detail}");
    assert_eq!(
        calls(&root),
        "osv-scanner --version\n",
        "the scan never started"
    );
    let plain = root.0.join("not-executable");
    fs::write(&plain, "").unwrap();
    let unstartable = run(planned(named("ruff"), plain, None), &root.0);
    assert_eq!(unstartable.version, None, "no version");
    let (start_code, start_detail) = failed(unstartable.status);
    assert_eq!(start_code, None, "not started");
    assert!(start_detail.starts_with("cannot run "), "{start_detail}");
}

#[test]
fn missing_and_skipped_entries_pass_through() {
    let root = root();
    for (step, status) in [
        (
            Step::Missing("m".to_owned()),
            Status::Missing("m".to_owned()),
        ),
        (
            Step::Skipped("s".to_owned()),
            Status::Skipped("s".to_owned()),
        ),
    ] {
        let outcome = run(
            Planned {
                tool: named("bandit"),
                argv: vec!["bandit".to_owned()],
                step,
            },
            &root.0,
        );
        assert_eq!(
            outcome,
            Outcome {
                tool: named("bandit"),
                argv: vec!["bandit".to_owned()],
                version: None,
                status,
            },
            "nothing run"
        );
    }
    assert_eq!(calls(&root), "", "nothing ran");
}
