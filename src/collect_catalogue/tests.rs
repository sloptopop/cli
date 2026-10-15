//! Unit tests for [`super`]: the catalogue's invariants.

use std::collections::HashSet;

use super::*;
use crate::collect_managed::Managed;
use crate::tests::support::inputs;

/// Every licence the catalogue may claim. A claim is only allowed with the
/// licence files it was read from (`licence_files`), checked below.
const LICENCES: [&str; 9] = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "MIT OR Apache-2.0",
    "Apache-2.0 OR MIT",
    "LGPL-2.1-only",
    "GPL-3.0-only",
    "LGPL-3.0-only",
];

fn entry(name: &str) -> &'static Tool {
    CATALOGUE
        .iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("{name} is catalogued"))
}

fn has(tool: &Tool, flags: &[&str]) -> bool {
    let argv = tool.argv(&inputs(&["RULES".to_owned()]));
    argv.windows(flags.len()).any(|window| window == flags)
}

#[test]
fn the_required_tier_one_tools_are_catalogued() {
    for name in [
        "gitleaks",
        "gitleaks-history",
        "osv-scanner",
        "opengrep",
        "semgrep",
        "bandit",
        "gosec",
        "govulncheck",
        "ruff",
        "oxlint",
        "biome",
        "staticcheck",
        "golangci-lint",
        "swiftlint",
        "eslint-sonarjs",
        "eslint-sonarjs-json",
        "sonaranalyzer-csharp",
        "cargo-llvm-cov",
        "coverage.py",
        "c8",
        "coverlet",
        "gcovr",
    ] {
        let _found = entry(name);
    }
}

#[test]
fn names_and_outputs_are_unique_and_plain() {
    let mut names = HashSet::new();
    let mut outputs = HashSet::new();
    for tool in CATALOGUE {
        assert!(names.insert(tool.name), "{}: duplicate name", tool.name);
        assert!(
            outputs.insert(tool.output),
            "{}: duplicate output",
            tool.name
        );
        assert!(
            !tool.output.starts_with('.') && !tool.output.contains('/'),
            "{}: output must be a visible file name in the reports directory",
            tool.name
        );
        assert_eq!(
            tool.output.matches('*').count(),
            usize::from(tool.sink == Sink::Config),
            "{}: only a configured report is a pattern, with one *",
            tool.name
        );
        assert!(
            !tool.name.is_empty() && !tool.name.contains(',') && !tool.name.starts_with('-'),
            "{}: usable with --only",
            tool.name
        );
    }
}

#[test]
fn every_entry_has_a_known_licence() {
    for tool in CATALOGUE {
        assert!(
            LICENCES.contains(&tool.licence),
            "{}: {}",
            tool.name,
            tool.licence
        );
    }
}

#[test]
fn every_licence_was_read_from_a_licence_file_at_a_release() {
    for tool in CATALOGUE {
        assert!(
            !tool.licence_files.is_empty(),
            "{}: no licence file",
            tool.name
        );
        for file in tool.licence_files {
            let name = file.rsplit(['/', '#']).next().unwrap_or_default();
            assert!(
                file.starts_with("https://") && !file.contains(char::is_whitespace),
                "{}: {file}",
                tool.name
            );
            assert!(
                ["LICENSE", "LICENSE.txt", "LICENSE-MIT", "LICENSE-APACHE"].contains(&name)
                    || name.ends_with(".nuspec"),
                "{}: {file} is a licence file",
                tool.name
            );
            assert!(
                ["/main/", "/master/", "/HEAD/", "/develop/", "/latest/"]
                    .iter()
                    .all(|branch| !file.contains(branch)),
                "{}: {file} is pinned to a release, not a branch",
                tool.name
            );
        }
    }
}

#[test]
fn managed_entries_claim_the_licence_of_their_pinned_analyzer() {
    for (name, managed) in [
        ("eslint-sonarjs", Managed::EslintSonarjs),
        ("eslint-sonarjs-json", Managed::EslintSonarjs),
        ("sonaranalyzer-csharp", Managed::SonarCsharp),
    ] {
        let tool = entry(name);
        let analyzer = managed.pins().first().unwrap();
        assert_eq!(tool.kind, Kind::Managed(managed), "{name}");
        assert_eq!(tool.licence, analyzer.licence, "{name}");
        assert_eq!(tool.licence_files, analyzer.licence_files, "{name}");
        assert_eq!(tool.image, None, "{name}: no container image");
    }
}

#[test]
fn flags_are_fixed_and_the_sink_matches_the_arguments() {
    for tool in CATALOGUE {
        let outputs = tool
            .args
            .iter()
            .filter(|arg| matches!(arg, Output(_) | ReportsDir(_)))
            .count();
        let expected = match tool.sink {
            Sink::PathArgument => 1,
            Sink::Stdout | Sink::Config => 0,
        };
        assert_eq!(outputs, expected, "{}: output argument", tool.name);
        assert_eq!(
            tool.takes_rules(),
            matches!(tool.name, "opengrep" | "semgrep"),
            "{}: only Opengrep and Semgrep take rules",
            tool.name
        );
        for arg in tool.args {
            if let Fixed(value) = arg {
                assert!(
                    !value.is_empty() && !value.contains(char::is_whitespace),
                    "{}: {value:?} is one plain argument",
                    tool.name
                );
            }
        }
        assert!(
            !tool.findings.contains(&0),
            "{}: 0 is plain success",
            tool.name
        );
    }
}

#[test]
fn documented_entries_are_never_containerised() {
    for tool in CATALOGUE
        .iter()
        .filter(|tool| tool.kind == Kind::Documented)
    {
        assert_eq!(tool.image, None, "{}", tool.name);
        assert_eq!(tool.network, Network::Offline, "{}", tool.name);
    }
}

#[test]
fn pinned_digests_are_well_formed() {
    for tool in CATALOGUE {
        if let Some(image) = tool.image {
            assert!(
                !image.repository.contains(['@', ':']),
                "{}: no tag",
                tool.name
            );
            if image.digest.is_some() {
                assert!(
                    image.reference().is_some(),
                    "{}: malformed digest",
                    tool.name
                );
            }
        }
    }
}

#[test]
fn only_osv_scanner_downloads_a_database() {
    for tool in CATALOGUE {
        assert_eq!(
            tool.network == Network::DatabaseDownload,
            tool.name == "osv-scanner",
            "{}",
            tool.name
        );
    }
}

#[test]
fn security_relevant_flags_are_present() {
    for name in ["gitleaks", "gitleaks-history"] {
        let tool = entry(name);
        assert!(has(tool, &["--redact"]), "{name} redacts");
        assert!(has(tool, &["--report-format", "sarif"]), "{name} SARIF");
        assert!(has(tool, &["--exit-code", "0"]), "{name} exit code");
    }
    assert!(has(entry("gitleaks"), &["gitleaks", "dir"]), "working tree");
    assert_eq!(entry("gitleaks-history").kind, Kind::NeedsGit, "history");
    assert!(
        has(
            entry("osv-scanner"),
            &[
                "scan",
                "source",
                "-r",
                "--offline-vulnerabilities",
                "--download-offline-databases",
                "--format",
                "sarif"
            ]
        ),
        "osv-scanner offline"
    );
    for name in ["opengrep", "semgrep"] {
        assert!(
            has(entry(name), &["--config", "RULES"]),
            "{name}: the given rules only"
        );
        assert!(
            has(entry(name), &["--no-rewrite-rule-ids"]),
            "{name}: rule ids as written"
        );
    }
    assert!(has(entry("semgrep"), &["--metrics", "off"]), "no metrics");
    assert!(
        has(
            entry("golangci-lint"),
            &[
                "--max-issues-per-linter=0",
                "--max-same-issues=0",
                "--uniq-by-line=false"
            ]
        ),
        "golangci-lint uncapped"
    );
    assert!(has(entry("ruff"), &["--no-cache"]), "ruff writes no cache");
}

#[test]
fn managed_tools_run_with_sloptopops_files_only() {
    for (name, format) in [
        (
            "eslint-sonarjs",
            "TOOLS/eslint-sonarjs/node_modules/@microsoft/eslint-formatter-sarif/sarif.js",
        ),
        ("eslint-sonarjs-json", "json-with-metadata"),
    ] {
        let tool = entry(name);
        assert!(
            has(
                tool,
                &[
                    "node",
                    "ESLINT",
                    "--config",
                    "TOOLS/eslint-sonarjs/eslint.config.mjs",
                    "--no-inline-config",
                    "--no-error-on-unmatched-pattern",
                    "--format",
                    format
                ]
            ),
            "{name}: sloptopop's configuration only, offline"
        );
    }
    assert!(
        has(
            entry("sonaranalyzer-csharp"),
            &[
                "dotnet",
                "build",
                "--no-restore",
                "--no-incremental",
                "-p:TreatWarningsAsErrors=false",
                "-p:DirectoryBuildPropsPath=TOOLS/sonaranalyzer-csharp/Directory.Build.props"
            ]
        ),
        "no download, every file compiled, the generated props"
    );
}

#[test]
fn argv_renders_every_argument_kind() {
    let tool = Tool {
        name: "t",
        command: "tool",
        licence: "MIT",
        licence_files: &[],
        scope: Scope::CrossCutting,
        kind: Kind::Run,
        args: &[
            Fixed("-x"),
            Output(""),
            Output("--out="),
            ReportsDir("--dir="),
            Rules("--config"),
            Tools("--props=", "t/p"),
            Eslint,
        ],
        output: "t.sarif",
        sink: Sink::PathArgument,
        findings: &[],
        network: Network::Offline,
        image: None,
    };
    assert_eq!(
        tool.argv(&inputs(&["/a".to_owned(), "/b".to_owned()])),
        [
            "tool",
            "-x",
            ".sloptopop/reports/t.sarif",
            "--out=.sloptopop/reports/t.sarif",
            "--dir=.sloptopop/reports",
            "--config",
            "/a",
            "--config",
            "/b",
            "--props=TOOLS/t/p",
            "ESLINT"
        ],
        "argv"
    );
    assert_eq!(
        tool.version_args(&inputs(&[])),
        ["--version"],
        "the tool prints its version"
    );
    assert_eq!(
        entry("eslint-sonarjs").version_args(&inputs(&[])),
        ["ESLINT", "--version"],
        "ESLint's script prints ESLint's version"
    );
    assert!(tool.takes_rules(), "takes rules");
    assert!(!entry("ruff").takes_rules(), "ruff takes none");
    assert_eq!(
        tool.output_path(),
        ".sloptopop/reports/t.sarif",
        "output path"
    );
}

#[test]
fn scope_decides_applicability() {
    let python = BTreeSet::from([Ecosystem::Python]);
    assert!(entry("gitleaks").applies(&BTreeSet::new()), "cross-cutting");
    assert!(entry("ruff").applies(&python), "python tool on python");
    assert!(!entry("gosec").applies(&python), "go tool on python");
}

#[test]
fn only_well_formed_digests_make_a_reference() {
    let leak = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
    let hex = "0123456789abcdef".repeat(4);
    let image = |digest| Image {
        repository: "example/tool",
        digest,
    };
    assert_eq!(
        image(Some(leak(format!("sha256:{hex}")))).reference(),
        Some(format!("example/tool@sha256:{hex}")),
        "pinned"
    );
    for digest in [
        None,
        Some(leak(format!("sha256:{}", hex.to_uppercase()))),
        Some(leak(format!("sha256:{}", "a".repeat(63)))),
        Some(leak(format!("sha256:{hex}0"))),
        Some(leak(format!("sha512:{hex}"))),
        Some(leak(format!("sha256:{}g", "a".repeat(63)))),
    ] {
        assert_eq!(image(digest).reference(), None, "{digest:?}");
    }
}
