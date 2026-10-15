//! Unit tests for [`super`].

use std::path::PathBuf;

use serde_json::json;

use super::*;
use crate::collect_catalogue::{CATALOGUE, Tool};

fn named(name: &str) -> &'static Tool {
    CATALOGUE.iter().find(|tool| tool.name == name).unwrap()
}

fn outcomes() -> Vec<Outcome<'static>> {
    let outcome = |name, version: Option<&str>, status| Outcome {
        tool: named(name),
        argv: named(name).argv(&[]),
        version: version.map(str::to_owned),
        status,
    };
    vec![
        outcome("gitleaks", Some("8.0.0"), Status::Ran(0)),
        outcome(
            "ruff",
            None,
            Status::Failed {
                code: Some(2),
                detail: "bad config".to_owned(),
            },
        ),
        outcome(
            "staticcheck",
            Some("2025.1"),
            Status::Failed {
                code: None,
                detail: "signal: 9".to_owned(),
            },
        ),
        outcome(
            "bandit",
            None,
            Status::Missing("not found on PATH".to_owned()),
        ),
        outcome(
            "semgrep",
            None,
            Status::Skipped("not a git repository".to_owned()),
        ),
    ]
}

#[test]
fn the_record_lists_every_tool_in_order() {
    let ecosystems = BTreeSet::from([Ecosystem::Python, Ecosystem::Go]);
    let text = record(false, &ecosystems, &outcomes());
    assert!(text.ends_with("}\n"), "final newline");
    let parsed: Value = serde_json::from_str(&text).unwrap();
    let argv = |name| named(name).argv(&[]);
    assert_eq!(
        parsed,
        json!({
            "schemaVersion": 1,
            "mode": "host",
            "ecosystems": ["go", "python"],
            "tools": [
                { "name": "gitleaks", "licence": "MIT", "version": "8.0.0", "argv": argv("gitleaks"),
                  "output": ".sloptopop/reports/gitleaks.sarif", "status": "ran", "exitCode": 0 },
                { "name": "ruff", "licence": "MIT", "argv": argv("ruff"),
                  "output": ".sloptopop/reports/ruff.sarif", "status": "failed", "exitCode": 2,
                  "reason": "bad config" },
                { "name": "staticcheck", "licence": "MIT", "version": "2025.1", "argv": argv("staticcheck"),
                  "output": ".sloptopop/reports/staticcheck.sarif", "status": "failed",
                  "reason": "signal: 9" },
                { "name": "bandit", "licence": "Apache-2.0", "argv": argv("bandit"),
                  "output": ".sloptopop/reports/bandit.sarif", "status": "missing",
                  "reason": "not found on PATH" },
                { "name": "semgrep", "licence": "LGPL-2.1-only", "argv": argv("semgrep"),
                  "output": ".sloptopop/reports/semgrep.sarif", "status": "skipped",
                  "reason": "not a git repository" },
            ],
        }),
        "content"
    );
    assert_eq!(
        text,
        record(false, &ecosystems, &outcomes()),
        "deterministic"
    );
    assert!(
        !text.contains("time") && !text.contains("duration"),
        "no clocks"
    );
    let container: Value = serde_json::from_str(&record(true, &BTreeSet::new(), &[])).unwrap();
    assert_eq!(
        container,
        json!({ "schemaVersion": 1, "mode": "container", "ecosystems": [], "tools": [] }),
        "container mode"
    );
}

#[test]
fn the_summary_has_one_line_per_tool() {
    assert_eq!(
        summary(&BTreeSet::from([Ecosystem::Python]), &outcomes()),
        "sloptopop collect: 1 ran, 2 failed, 1 missing, 1 skipped (ecosystems: python)\n\
         \x20 ran      gitleaks         exit 0  .sloptopop/reports/gitleaks.sarif\n\
         \x20 failed   ruff             exit 2  bad config\n\
         \x20 failed   staticcheck      signal: 9\n\
         \x20 missing  bandit           not found on PATH\n\
         \x20 skipped  semgrep          not a git repository\n\
         Details: .sloptopop/reports/.collect.json; next, `sloptopop check` reads the reports.\n",
        "summary"
    );
    assert!(
        summary(&BTreeSet::new(), &[]).starts_with(
            "sloptopop collect: 0 ran, 0 failed, 0 missing, 0 skipped (ecosystems: none)\n"
        ),
        "nothing applies"
    );
}

#[test]
fn the_plan_shows_commands_and_reasons() {
    let run = |stdout: Option<PathBuf>| Step::Run {
        program: PathBuf::from("/bin/tool"),
        version: Vec::new(),
        stdout,
    };
    let entry = |name, step| Planned {
        tool: named(name),
        argv: vec![name.to_owned(), "-x".to_owned()],
        step,
    };
    let planned = [
        entry("gitleaks", run(None)),
        entry("osv-scanner", run(Some(PathBuf::from("/r/o.sarif")))),
        entry("bandit", Step::Missing("not found on PATH".to_owned())),
        entry("semgrep", Step::Skipped("not a git repository".to_owned())),
    ];
    assert_eq!(
        plan(&BTreeSet::from([Ecosystem::Rust, Ecosystem::Go]), &planned),
        "sloptopop collect: dry run, nothing run (ecosystems: go, rust)\n\
         \x20 run      gitleaks         gitleaks -x\n\
         \x20 run      osv-scanner      osv-scanner -x > .sloptopop/reports/osv-scanner.sarif\n\
         \x20 missing  bandit           bandit -x  (not found on PATH)\n\
         \x20 skipped  semgrep          semgrep -x  (not a git repository)\n",
        "plan"
    );
}
