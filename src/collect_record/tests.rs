//! Unit tests for [`super`].

use std::path::PathBuf;

use serde_json::json;

use super::*;
use crate::collect_catalogue::{CATALOGUE, Kind, Tool};
use crate::collect_data::Snapshot;
use crate::collect_run::Used;
use crate::collect_runtime::Engine;

fn named(name: &str) -> &'static Tool {
    CATALOGUE.iter().find(|tool| tool.name == name).unwrap()
}

fn image_of(name: &str) -> Image {
    match named(name).kind {
        Kind::Run(image) | Kind::NeedsGit(image) => image,
        Kind::Documented => panic!("{name}"),
    }
}

fn outcomes() -> Vec<Outcome<'static>> {
    let outcome = |name, version: Option<&str>, image: Option<Used>, status| Outcome {
        tool: named(name),
        argv: named(name).argv(&[], false),
        version: version.map(str::to_owned),
        image,
        status,
    };
    let used = |name: &str| {
        let image = image_of(name);
        Some(Used {
            reference: image.reference("reg"),
            version: image.version,
        })
    };
    vec![
        outcome(
            "gitleaks",
            Some("8.30.1-1"),
            used("gitleaks"),
            Status::Ran(0),
        ),
        outcome(
            "ruff",
            None,
            None,
            Status::Failed {
                code: Some(2),
                detail: "bad config".to_owned(),
            },
        ),
        outcome(
            "staticcheck",
            Some("2025.1"),
            None,
            Status::Failed {
                code: None,
                detail: "signal: 9".to_owned(),
            },
        ),
        outcome(
            "bandit",
            None,
            None,
            Status::Missing("no container runtime".to_owned()),
        ),
        outcome(
            "semgrep",
            None,
            None,
            Status::Skipped("not a git repository".to_owned()),
        ),
    ]
}

fn runtime() -> Runtime {
    Runtime {
        engine: Engine::Podman,
        program: PathBuf::from("/usr/bin/podman"),
        version: "5.6.0".to_owned(),
        rootless: true,
        limits: true,
    }
}

#[test]
fn the_record_lists_every_tool_with_its_image_in_order() {
    let ecosystems = BTreeSet::from([Ecosystem::Python, Ecosystem::Go]);
    let runtime = runtime();
    let databases = [Resolved {
        snapshot: Snapshot {
            name: "db-osv-pypi",
            path: "osv-scalibr/PyPI",
        },
        reference: "reg/analyzers/db-osv-pypi@sha256:ab".to_owned(),
        version: "20261021".to_owned(),
        dir: PathBuf::from("/cache"),
    }];
    let text = record(
        Mode::Images {
            runtime: Some(&runtime),
            registry: "reg",
        },
        &ecosystems,
        &databases,
        &outcomes(),
    );
    assert!(text.ends_with("}\n"), "final newline");
    let parsed: Value = serde_json::from_str(&text).unwrap();
    let argv = |name| named(name).argv(&[], false);
    let files = |name| named(name).licence_files.to_vec();
    let gitleaks = image_of("gitleaks");
    assert_eq!(
        parsed,
        json!({
            "schemaVersion": 2,
            "mode": "container",
            "registry": "reg",
            "runtime": { "name": "podman", "version": "5.6.0", "rootless": true },
            "ecosystems": ["go", "python"],
            "databases": [
                { "name": "db-osv-pypi", "reference": "reg/analyzers/db-osv-pypi@sha256:ab", "version": "20261021" }
            ],
            "tools": [
                { "name": "gitleaks", "licence": "MIT", "licenceFiles": files("gitleaks"), "version": "8.30.1-1",
                  "image": { "reference": gitleaks.reference("reg"), "version": "8.30.1-1" },
                  "argv": argv("gitleaks"), "output": ".sloptopop/reports/gitleaks.sarif", "status": "ran", "exitCode": 0 },
                { "name": "ruff", "licence": "MIT", "licenceFiles": files("ruff"), "argv": argv("ruff"),
                  "output": ".sloptopop/reports/ruff.sarif", "status": "failed", "exitCode": 2,
                  "reason": "bad config" },
                { "name": "staticcheck", "licence": "MIT", "licenceFiles": files("staticcheck"), "version": "2025.1", "argv": argv("staticcheck"),
                  "output": ".sloptopop/reports/staticcheck.sarif", "status": "failed",
                  "reason": "signal: 9" },
                { "name": "bandit", "licence": "Apache-2.0", "licenceFiles": files("bandit"), "argv": argv("bandit"),
                  "output": ".sloptopop/reports/bandit.sarif", "status": "missing",
                  "reason": "no container runtime" },
                { "name": "semgrep", "licence": "LGPL-2.1-only", "licenceFiles": files("semgrep"), "argv": argv("semgrep"),
                  "output": ".sloptopop/reports/semgrep.sarif", "status": "skipped",
                  "reason": "not a git repository" }
            ]
        })
    );
}

#[test]
fn the_record_says_when_there_was_no_runtime_or_tools_were_unpinned() {
    let ecosystems = BTreeSet::new();
    let none = record(
        Mode::Images {
            runtime: None,
            registry: "reg",
        },
        &ecosystems,
        &[],
        &[],
    );
    let parsed: Value = serde_json::from_str(&none).unwrap();
    assert_eq!(parsed["mode"], "container");
    assert!(parsed.get("runtime").is_none(), "{parsed}");
    let host = record(Mode::Host, &ecosystems, &[], &[]);
    let unpinned: Value = serde_json::from_str(&host).unwrap();
    assert_eq!(unpinned["mode"], "host-unpinned");
    assert!(unpinned.get("registry").is_none(), "{unpinned}");
}

#[test]
fn the_summary_counts_and_explains() {
    let ecosystems = BTreeSet::from([Ecosystem::Python]);
    assert_eq!(
        summary(&ecosystems, &outcomes()),
        "sloptopop collect: 1 ran, 2 failed, 1 missing, 1 skipped (ecosystems: python)\n\
         \x20 ran      gitleaks             exit 0  .sloptopop/reports/gitleaks.sarif\n\
         \x20 failed   ruff                 exit 2  bad config\n\
         \x20 failed   staticcheck          signal: 9\n\
         \x20 missing  bandit               no container runtime\n\
         \x20 skipped  semgrep              not a git repository\n\
         Details: .sloptopop/reports/.collect.json; next, `sloptopop check` reads the reports.\n"
    );
    assert!(summary(&BTreeSet::new(), &[]).contains("(ecosystems: none)"));
}

#[test]
fn the_dry_run_shows_images_or_commands() {
    let ecosystems = BTreeSet::from([Ecosystem::Python]);
    let image = image_of("ruff");
    let plan_of = |step| Planned {
        tool: named("ruff"),
        argv: vec!["ruff".to_owned(), "check".to_owned()],
        stdout: None,
        step,
    };
    let osv = Planned {
        tool: named("osv-scanner"),
        argv: vec!["osv-scanner".to_owned()],
        stdout: Some(PathBuf::from("/r/.sloptopop/reports/osv-scanner.sarif")),
        step: Step::NotRun(NotRun::Missing("not found on PATH".to_owned())),
    };
    let planned = vec![
        plan_of(Step::Run(image)),
        plan_of(Step::NotRun(NotRun::Skipped("why".to_owned()))),
    ];
    let text = plan(&ecosystems, &planned, |pinned, target| {
        image_line(pinned, "reg", "aarch64", target)
    });
    assert_eq!(
        text,
        format!(
            "sloptopop collect: dry run, nothing fetched or run (ecosystems: python)\n\
             \x20 run      ruff                 {} ({}, {}): ruff check\n\
             \x20 skipped  ruff                 ruff check  (why)\n",
            image.reference("reg"),
            image.version,
            megabytes(image.bytes[1])
        )
    );
    let host = plan(&ecosystems, &[osv], |_: &PathBuf, target| target.to_owned());
    assert!(
        host.ends_with(
            "  missing  osv-scanner          osv-scanner > .sloptopop/reports/osv-scanner.sarif  (not found on PATH)\n"
        ),
        "{host}"
    );
    let run = Planned {
        tool: named("ruff"),
        argv: vec!["ruff".to_owned()],
        stdout: None,
        step: Step::Run(PathBuf::from("/bin/ruff")),
    };
    let runs = plan(&ecosystems, &[run], |_, target| format!("<{target}>"));
    assert!(
        runs.ends_with("  run      ruff                 <ruff>\n"),
        "{runs}"
    );
    assert_eq!(
        image_line(&image, "reg", "x86_64", "t"),
        format!(
            "{} ({}, {}): t",
            image.reference("reg"),
            image.version,
            megabytes(image.bytes[0])
        )
    );
}
