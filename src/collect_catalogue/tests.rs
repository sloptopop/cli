//! Unit tests for [`super`]: the catalogue's invariants.

use std::collections::HashSet;
use std::path::Path;

use super::*;

/// Every licence the catalogue may claim. A claim is only allowed with the
/// licence files it was read from (`licence_files`), checked below.
const LICENCES: [&str; 11] = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "MIT OR Apache-2.0",
    "Apache-2.0 OR MIT",
    "LGPL-2.1-only",
    "GPL-2.0-or-later",
    "GPL-3.0-only",
    "LGPL-3.0-only",
    "Apache-2.0 WITH LLVM-exception",
];

fn entry(name: &str) -> &'static Tool {
    CATALOGUE
        .iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("{name} is catalogued"))
}

fn has(tool: &Tool, flags: &[&str]) -> bool {
    let argv = tool.argv(&["RULES".to_owned()], false);
    argv.windows(flags.len()).any(|window| window == flags)
}

fn image(tool: &Tool) -> Option<Image> {
    match tool.kind {
        Kind::Run(image) | Kind::NeedsGit(image) => Some(image),
        Kind::Documented => None,
    }
}

#[test]
fn the_required_tools_are_catalogued() {
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
        "pylint-cyclic-import",
        "oxlint",
        "biome",
        "eslint-sonarjs",
        "eslint-sonarjs-json",
        "eslint-typescript",
        "dependency-cruiser",
        "staticcheck",
        "golangci-lint",
        "swiftlint",
        "sonaranalyzer-csharp",
        "scorecard-license",
        "jscpd",
        "phpcs",
        "pmd",
        "detekt",
        "clang-tidy",
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
    let mut raws = HashSet::new();
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
        if let Some((_, raw)) = tool.convert {
            assert!(raws.insert(raw), "{}: duplicate raw file", tool.name);
            assert!(
                !raw.contains('/') && Path::new(raw).extension().is_none_or(|e| e != "sarif"),
                "{}: {raw}",
                tool.name
            );
            assert!(
                Path::new(tool.output)
                    .extension()
                    .is_some_and(|e| e == "sarif"),
                "{}: converted to SARIF",
                tool.name
            );
            assert_eq!(
                tool.written_path(),
                format!("{RAW_DIR}/{raw}"),
                "{}",
                tool.name
            );
        } else {
            assert_eq!(tool.written_path(), tool.output_path(), "{}", tool.name);
        }
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
                [
                    "LICENSE",
                    "LICENSE.txt",
                    "LICENSE.TXT",
                    "LICENSE-MIT",
                    "LICENSE-APACHE",
                    "licence.txt",
                ]
                .contains(&name)
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
fn every_runnable_entry_has_a_well_formed_pinned_image() {
    for tool in CATALOGUE {
        let Some(image) = image(tool) else {
            continue;
        };
        let hex = image.digest.strip_prefix("sha256:").unwrap_or_default();
        assert!(
            hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "{}: digest {}",
            tool.name,
            image.digest
        );
        assert!(
            image.name.starts_with("analyzers/") && !image.name.contains(['@', ':']),
            "{}: {}",
            tool.name,
            image.name
        );
        assert!(
            image.version.contains('-') && !image.version.contains(char::is_whitespace),
            "{}: version {} has a build revision",
            tool.name,
            image.version
        );
        assert!(
            image.bytes.iter().all(|&bytes| bytes > 1_000_000),
            "{}: sizes {:?}",
            tool.name,
            image.bytes
        );
    }
}

#[test]
fn pull_sizes_follow_the_host_architecture() {
    let image = Image {
        name: "analyzers/x",
        version: "1-1",
        digest: "sha256:00",
        bytes: [10, 20],
    };
    assert_eq!(image.pull_bytes("x86_64"), 10);
    assert_eq!(image.pull_bytes("aarch64"), 20);
}

#[test]
fn entries_sharing_an_image_share_its_digest() {
    let mut seen: Vec<Image> = Vec::new();
    for tool in CATALOGUE {
        if let Some(image) = image(tool) {
            if let Some(other) = seen.iter().find(|other| other.name == image.name) {
                assert_eq!(*other, image, "{}: one pin per image", tool.name);
            }
            seen.push(image);
        }
    }
    assert_eq!(entry("gitleaks").kind, Kind::Run(GITLEAKS));
    assert_eq!(
        entry("gitleaks-history").kind,
        Kind::NeedsGit(GITLEAKS),
        "history needs git"
    );
    assert_eq!(entry("eslint-sonarjs-json").kind, Kind::Run(ESLINT_SONARJS));
}

#[test]
fn references_and_labels() {
    let image = GITLEAKS;
    assert_eq!(
        image.reference("registry.example/mirror"),
        format!(
            "registry.example/mirror/analyzers/gitleaks@{}",
            image.digest
        )
    );
    assert_eq!(image.tool_label(), "gitleaks");
    let flat = Image {
        name: "flat",
        ..image
    };
    assert_eq!(flat.tool_label(), "flat");
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
fn only_semgrep_needs_naming() {
    for tool in CATALOGUE {
        assert_eq!(!tool.default, tool.name == "semgrep", "{}", tool.name);
    }
}

#[test]
fn documented_entries_run_nowhere() {
    for tool in CATALOGUE
        .iter()
        .filter(|tool| tool.kind == Kind::Documented)
    {
        assert_eq!(tool.prepare, None, "{}", tool.name);
        assert_eq!(tool.data, None, "{}", tool.name);
        assert_eq!(tool.convert, None, "{}", tool.name);
    }
}

#[test]
fn only_the_go_analyzers_and_the_dotnet_build_need_a_prepare_step() {
    for tool in CATALOGUE {
        let expected = match tool.name {
            "gosec" | "govulncheck" | "staticcheck" | "golangci-lint" => Some(Prepare::GoModules),
            "sonaranalyzer-csharp" => Some(Prepare::DotnetRestore),
            _ => None,
        };
        assert_eq!(tool.prepare, expected, "{}", tool.name);
    }
}

#[test]
fn the_scanners_read_mounted_vulnerability_data() {
    for tool in CATALOGUE {
        let expected = match tool.name {
            "osv-scanner" => Some(Data::Osv),
            "govulncheck" => Some(Data::GoVuln),
            _ => None,
        };
        assert_eq!(tool.data, expected, "{}", tool.name);
    }
    assert!(
        has(entry("osv-scanner"), &["--offline", "--format", "sarif"]),
        "fully offline"
    );
    assert!(
        has(entry("govulncheck"), &["-db", "file:///db/govulndb"]),
        "local database"
    );
}

#[test]
fn host_arguments_replace_only_the_mounts() {
    let osv = entry("osv-scanner");
    let host = osv.argv(&[], true);
    assert!(
        host.windows(2)
            .any(|w| w == ["--offline-vulnerabilities", "--download-offline-databases"]),
        "{host:?}"
    );
    assert!(
        !osv.argv(&[], false)
            .contains(&"--download-offline-databases".to_owned())
    );
    let govulncheck = entry("govulncheck").argv(&[], true);
    assert_eq!(govulncheck, ["govulncheck", "-format", "sarif", "./..."]);
    let ruff = entry("ruff");
    assert_eq!(ruff.argv(&[], true), ruff.argv(&[], false), "Host::Same");
    let eslint = entry("eslint-sonarjs");
    assert_eq!(eslint.host, Host::ImageOnly);
    assert_eq!(eslint.argv(&[], true), eslint.argv(&[], false));
}

#[test]
fn arguments_render_outputs_rules_and_directories() {
    let rules = ["/rules/0".to_owned(), "/rules/1".to_owned()];
    assert_eq!(
        entry("opengrep").argv(&rules, false),
        [
            "opengrep",
            "scan",
            "--sarif",
            "--no-rewrite-rule-ids",
            "--config",
            "/rules/0",
            "--config",
            "/rules/1",
            "."
        ]
    );
    assert!(has(
        entry("golangci-lint"),
        &["--output.sarif.path=.sloptopop/reports/golangci.sarif"]
    ));
    assert!(has(
        entry("detekt"),
        &["--report", "sarif:.sloptopop/reports/detekt.sarif"]
    ));
    assert!(has(
        entry("jscpd"),
        &["--output=.sloptopop/reports", "--report-name", "jscpd"]
    ));
    assert!(has(entry("c8"), &["--reports-dir=.sloptopop/reports"]));
    assert_eq!(
        entry("jscpd").output_path(),
        ".sloptopop/reports/jscpd.sarif"
    );
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
        has(entry("semgrep"), &["--disable-version-check"]),
        "no version check"
    );
    assert!(has(entry("ruff"), &["--no-cache"]), "ruff writes nothing");
    assert!(
        has(entry("ruff"), &["--extend-select", "S110,S501,N999"]),
        "crown-jewel rules"
    );
    assert!(
        has(entry("swiftlint"), &["--no-cache"]),
        "swiftlint writes nothing"
    );
    assert!(
        has(
            entry("golangci-lint"),
            &[
                "--max-issues-per-linter=0",
                "--max-same-issues=0",
                "--uniq-by-line=false"
            ]
        ),
        "no caps"
    );
    for name in ["eslint-sonarjs", "eslint-sonarjs-json", "eslint-typescript"] {
        assert!(has(entry(name), &["--no-inline-config"]), "{name}");
        assert!(
            has(entry(name), &["--no-error-on-unmatched-pattern"]),
            "{name}"
        );
    }
    assert!(has(
        entry("pylint-cyclic-import"),
        &["--disable=all", "--enable=cyclic-import"]
    ));
    assert!(has(
        entry("scorecard-license"),
        &["--local=.", "--checks=License"]
    ));
    assert_eq!(
        entry("scorecard-license").env,
        [("GITHUB_AUTH_TOKEN", "unused-in-local-mode")]
    );
    assert!(has(
        entry("pmd"),
        &["-R", "category/java/errorprone.xml/EmptyCatchBlock"]
    ));
    assert!(has(entry("clang-tidy"), &["-*,bugprone-empty-catch"]));
}

#[test]
fn scopes_follow_the_ecosystems() {
    let python = BTreeSet::from([Ecosystem::Python]);
    assert!(entry("gitleaks").applies(&python), "cross-cutting");
    assert!(entry("ruff").applies(&python));
    assert!(!entry("oxlint").applies(&python));
    assert!(entry("oxlint").applies(&BTreeSet::from([Ecosystem::Node])));
    assert!(entry("detekt").applies(&BTreeSet::from([Ecosystem::Jvm])));
    assert!(entry("phpcs").applies(&BTreeSet::from([Ecosystem::Php])));
    assert!(entry("clang-tidy").applies(&BTreeSet::from([Ecosystem::Cpp])));
    assert!(!entry("clang-tidy").applies(&BTreeSet::new()));
}

/// The whole catalogue, every field of every entry, as reviewed: a change
/// to an entry (a flag, an output, a digest) must change this file too, so
/// it shows in review. `catalogue.txt` is `format!("{CATALOGUE:#?}\n")`.
#[test]
fn the_catalogue_is_the_reviewed_one() {
    let shown = format!("{CATALOGUE:#?}\n");
    let reviewed = include_str!("catalogue.txt");
    let first = shown
        .lines()
        .zip(reviewed.lines())
        .position(|(a, b)| a != b);
    assert!(
        shown == reviewed,
        "the catalogue differs from src/collect_catalogue/catalogue.txt at line {}",
        first.map_or(shown.lines().count().min(reviewed.lines().count()), |n| n
            + 1)
    );
}
