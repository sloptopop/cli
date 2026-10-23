//! Unit tests for [`super`]: the catalogue's invariants.

use std::collections::HashSet;
use std::path::Path;

use super::*;
use crate::collect_convert::{Defect, Rule};

/// Every licence the catalogue may claim. A claim is only allowed with the
/// licence files it was read from (`licence_files`), checked below.
const LICENCES: [&str; 12] = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "MIT OR Apache-2.0",
    "Apache-2.0 OR MIT",
    "LGPL-2.1-only",
    "GPL-2.0-or-later",
    "GPL-3.0-only",
    "GPL-3.0-or-later",
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
        "cppcheck",
        "shellcheck",
        "clippy",
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
                    "COPYING",
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
    assert_eq!(entry("eslint-sonarjs").kind, Kind::Run(ESLINT_SONARJS));
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
        // Clippy's entry names the converter for the JSON it prints when
        // it is run elsewhere (LX-CAR-4); collect itself never runs it.
        assert_eq!(
            tool.convert,
            (tool.name == "clippy").then_some((Converter::RustcJson, "clippy.json")),
            "{}",
            tool.name
        );
    }
}

#[test]
fn only_the_go_analyzers_and_the_dotnet_build_need_a_prepare_step() {
    for tool in CATALOGUE {
        let expected = match tool.name {
            "gosec" | "govulncheck" | "staticcheck" | "golangci-lint" | "golangci-lint-defects" => {
                Some(Prepare::GoModules)
            }
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
            "--json",
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
    for name in ["eslint-sonarjs", "eslint-typescript"] {
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
    assert!(
        lint_defects::PMD_RULESETS
            .split(',')
            .any(|rule| rule == "category/java/errorprone.xml/EmptyCatchBlock"),
        "the crown-jewel PMD rule"
    );
    assert!(has(
        entry("cppcheck"),
        &[
            "--enable=warning,performance,portability",
            "--library=googletest",
            "--library=/usr/share/sloptopop-cppcheck/tests.cfg"
        ]
    ));
}

#[test]
fn the_defect_runs_keep_the_crown_jewel_rules_and_their_own_configuration() {
    let ruff = entry("ruff").argv(&[], false);
    let select = ruff
        .iter()
        .skip_while(|arg| *arg != "--extend-select")
        .nth(1)
        .unwrap();
    for crown_jewel in ["S110", "S501", "N999"] {
        assert!(
            select.split(',').any(|code| code == crown_jewel),
            "crown-jewel rule {crown_jewel}"
        );
    }
    assert!(
        has(
            entry("golangci-lint-defects"),
            &["run", "--config=/opt/golangci-lint/defects.yml"]
        ),
        "the image's own configuration, never the project's"
    );
    assert!(
        has(entry("swiftlint-defects"), &["--no-cache"]),
        "swiftlint-defects writes nothing"
    );
}

/// The C and C++ analyzers and `ShellCheck` report exactly their listed
/// defect rules.
fn rules_of(name: &str) -> &'static [Rule] {
    match entry(name).convert {
        Some((
            Converter::ClangTidy(rules)
            | Converter::Cppcheck(rules)
            | Converter::ShellcheckJson1(rules),
            _,
        )) => rules,
        other => panic!("{name}: {other:?}"),
    }
}

#[test]
fn clang_tidy_runs_exactly_its_reported_checks() {
    let reported: Vec<&str> = rules_of("clang-tidy")
        .iter()
        .filter(|rule| rule.off.is_none())
        .map(|rule| rule.id)
        .collect();
    let checks = format!("-*,{}", reported.join(","));
    assert_eq!(
        entry("clang-tidy").args,
        [Fixed(defects::CLANG_TIDY_CHECKS)]
    );
    assert_eq!(defects::CLANG_TIDY_CHECKS, checks);
    assert!(
        reported.contains(&"clang-analyzer-core.NullDereference")
            && reported.contains(&"bugprone-empty-catch"),
        "the null-dereference case and the swallowed-errors check"
    );
    for rule in rules_of("clang-tidy") {
        assert!(
            [
                "bugprone-",
                "clang-analyzer-core.",
                "clang-analyzer-cplusplus.",
                "clang-analyzer-deadcode.",
                "clang-analyzer-unix.",
            ]
            .iter()
            .any(|family| rule.id.starts_with(family)),
            "{}: one of the enabled families",
            rule.id
        );
    }
}

#[test]
fn every_defect_rule_is_listed_once_with_its_reasons() {
    let tagged: Vec<&str> = CATALOGUE
        .iter()
        .filter(|tool| !tool.defects.is_empty())
        .map(|tool| tool.name)
        .collect();
    assert_eq!(
        tagged,
        [
            "ruff",
            "biome",
            "eslint-sonarjs",
            "eslint-typescript",
            "golangci-lint-defects",
            "swiftlint-defects",
            "sonaranalyzer-csharp",
            "pmd",
            "detekt",
            "clang-tidy",
            "cppcheck",
            "shellcheck"
        ]
    );
    for name in tagged {
        let rules = entry(name).defects;
        let ids: Vec<&str> = rules.iter().map(|rule| rule.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids, sorted, "{name}: sorted, each once");
        for rule in rules {
            assert!(
                !rule.id.is_empty() && !rule.id.contains([',', ' ']),
                "{name}: {}",
                rule.id
            );
            assert!(rule.reason.len() > 10, "{name}: {}: what it finds", rule.id);
            assert!(
                rule.off.is_none_or(|why| why.len() > 10),
                "{name}: {}: why it is off",
                rule.id
            );
        }
    }
    assert_eq!(
        Rule::reported(
            rules_of("clang-tidy"),
            "clang-analyzer-core.NullDereference"
        )
        .map(|rule| rule.class),
        Some(Defect::NullDereference),
        "the owner's case"
    );
    let redundant = rules_of("cppcheck")
        .iter()
        .find(|rule| rule.id == "nullPointerRedundantCheck")
        .unwrap();
    assert_eq!(redundant.class, Defect::NullDereference);
    assert!(
        redundant
            .off
            .is_some_and(|why| why.starts_with("precision 1/19")),
        "listed, but below the precision budget"
    );
    for name in ["clang-tidy", "cppcheck", "shellcheck"] {
        assert_eq!(entry(name).defects, rules_of(name), "{name}: one table");
    }
}

#[test]
fn shellcheck_reports_its_codes_with_their_classes() {
    for rule in rules_of("shellcheck") {
        assert!(
            rule.id
                .strip_prefix("SC")
                .is_some_and(|code| code.parse::<u16>().is_ok()),
            "{}",
            rule.id
        );
    }
    let class = |id: &str| {
        rules_of("shellcheck")
            .iter()
            .find(|rule| rule.id == id)
            .map(|rule| rule.class)
    };
    assert_eq!(
        ["SC2006", "SC2086", "SC2164"].map(class),
        [Some(Defect::ShellExpansion); 3],
        "the three rules with reviewed fixes are listed"
    );
    assert_eq!(
        ["SC2006", "SC2086", "SC2164"]
            .map(|id| Rule::reported(rules_of("shellcheck"), id).is_some()),
        [false, false, true],
        "only SC2164 is above the precision budget"
    );
}

/// The ids of the rules of `rules`, in order.
fn ids(rules: &[Rule]) -> Vec<&'static str> {
    rules.iter().map(|rule| rule.id).collect()
}

#[test]
fn the_linters_run_exactly_their_listed_defect_rules() {
    let mut select: Vec<&str> = ids(lint_defects::RUFF_RULES);
    select.extend(["N999", "S501"]);
    select.sort_unstable();
    assert_eq!(lint_defects::RUFF_SELECT, select.join(","));
    assert!(has(
        entry("ruff"),
        &["--extend-select", lint_defects::RUFF_SELECT]
    ));
    let rulesets: Vec<&str> = lint_defects::PMD_RULESETS.split(',').collect();
    let mut named: Vec<&str> = rulesets
        .iter()
        .map(|path| path.rsplit('/').next().unwrap())
        .collect();
    named.sort_unstable();
    assert_eq!(named, ids(lint_defects::PMD_RULES));
    for path in rulesets {
        assert!(
            path.starts_with("category/java/errorprone.xml/")
                || path.starts_with("category/java/multithreading.xml/"),
            "{path}"
        );
    }
    assert!(has(entry("pmd"), &["-R", lint_defects::PMD_RULESETS]));
    let argv = entry("swiftlint-defects").argv(&[], false);
    let only: Vec<&str> = argv
        .windows(2)
        .filter(|pair| pair[0] == "--only-rule")
        .map(|pair| pair[1].as_str())
        .collect();
    assert_eq!(only, ids(lint_defects::SWIFTLINT_DEFECT_RULES));
    assert_eq!(
        Rule::reported(lint_defects::GOLANGCI_DEFECT_RULES, "govet").map(|rule| rule.class),
        Some(Defect::NullDereference),
        "govet runs only nilness"
    );
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
    for name in ["clang-tidy", "cppcheck"] {
        assert!(
            entry(name).applies(&BTreeSet::from([Ecosystem::Cpp])),
            "{name}"
        );
        assert!(!entry(name).applies(&BTreeSet::new()), "{name}");
    }
    let shell = BTreeSet::from([Ecosystem::Shell]);
    assert!(entry("shellcheck").applies(&shell));
    assert!(!entry("shellcheck").applies(&python));
    assert!(entry("clippy").applies(&BTreeSet::from([Ecosystem::Rust])));
    assert!(!entry("clippy").applies(&shell));
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

/// WP-21: every tool whose fixes only another format carries runs in that
/// format, and collect converts it, keeping the fixes.
#[test]
fn fix_carrying_tools_run_in_their_fix_carrying_format() {
    let expected = [
        (
            "ruff",
            Converter::RuffJson,
            &["--output-format", "json"][..],
        ),
        (
            "eslint-sonarjs",
            Converter::EslintJson,
            &["--format", "json-with-metadata"][..],
        ),
        (
            "eslint-typescript",
            Converter::EslintJson,
            &["--format", "json-with-metadata"][..],
        ),
        (
            "biome",
            Converter::BiomeRdjson,
            &["--reporter=rdjson", "--max-diagnostics=none"][..],
        ),
        ("opengrep", Converter::SemgrepJson, &["scan", "--json"][..]),
        ("semgrep", Converter::SemgrepJson, &["scan", "--json"][..]),
        // The wrappers ask for json1 and the exported fixes themselves.
        (
            "shellcheck",
            Converter::ShellcheckJson1(defects::SHELLCHECK_RULES),
            &[][..],
        ),
        (
            "clang-tidy",
            Converter::ClangTidy(defects::CLANG_TIDY_RULES),
            &[][..],
        ),
        (
            "clippy",
            Converter::RustcJson,
            &["clippy", "--message-format=json"][..],
        ),
    ];
    for (name, converter, flags) in expected {
        let tool = entry(name);
        assert_eq!(tool.convert.map(|(c, _)| c), Some(converter), "{name}");
        assert!(flags.is_empty() || has(tool, flags), "{name}: {flags:?}");
    }
    for tool in CATALOGUE {
        assert_eq!(
            tool.sarif_fixes,
            (tool.name == "staticcheck").then_some(Columns::Utf8Bytes),
            "{}: only staticcheck's own SARIF fixes are kept",
            tool.name
        );
    }
}

#[test]
fn only_the_c_and_cpp_analyzers_learn_the_host_path_of_the_repository() {
    for tool in CATALOGUE {
        assert_eq!(
            tool.root_variable,
            ["clang-tidy", "cppcheck"]
                .contains(&tool.name)
                .then_some("SLOPTOPOP_HOST_ROOT"),
            "{}",
            tool.name
        );
    }
}

#[test]
fn golangci_lint_names_the_configuration_it_refuses() {
    for tool in CATALOGUE {
        assert_eq!(
            tool.refusals.is_empty(),
            tool.name != "golangci-lint",
            "{}",
            tool.name
        );
    }
    let [(needle, reason)] = entry("golangci-lint").refusals else {
        panic!("one refusal");
    };
    assert_eq!(*needle, "unsupported version of the configuration");
    assert!(reason.contains("golangci-lint migrate") && reason.contains("no report"));
}

/// CPP-1: what the pinned clang-tidy and Cppcheck images printed for the
/// null dereference after a null check (`tests/fixtures/cpp-probe/`), run
/// through the catalogue's converters and the normalization, is the
/// golden report, byte for byte, as `collect` writes it.
#[test]
fn the_cpp_probe_converts_to_the_golden_reports() {
    let probe = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cpp-probe");
    for name in ["clang-tidy", "cppcheck"] {
        let tool = entry(name);
        let (converter, raw_file) = tool.convert.unwrap();
        let image = image(tool).unwrap();
        let bytes = std::fs::read(probe.join(raw_file)).unwrap();
        let from_raw =
            crate::collect_convert::convert(converter, &bytes, name, image.version, &probe)
                .unwrap();
        let written = crate::collect_convert::normalize(
            from_raw.as_bytes(),
            name,
            image.version,
            tool.sarif_fixes,
            tool.defects,
            false,
            &probe,
        )
        .unwrap();
        let golden = std::fs::read_to_string(probe.join("expected").join(tool.output)).unwrap();
        assert_eq!(written, golden, "{name}");
        assert!(
            written.contains("\"confidence\": \"lower\""),
            "{name}: {written}"
        );
    }
    let clang_tidy = std::fs::read_to_string(probe.join("expected/clang-tidy.sarif")).unwrap();
    assert!(
        clang_tidy.contains("clang-analyzer-core.NullDereference")
            && clang_tidy.contains("\"DC01\"")
    );
    // Cppcheck finds the owner's case too, but its rule is off by default.
    let raw = std::fs::read_to_string(probe.join("cppcheck.txt")).unwrap();
    let cppcheck = std::fs::read_to_string(probe.join("expected/cppcheck.sarif")).unwrap();
    assert!(
        raw.contains("nullPointerRedundantCheck")
            && !cppcheck.contains("nullPointerRedundantCheck")
    );
    assert!(
        cppcheck.contains("\"DC06\""),
        "the out-of-bounds write is reported: {cppcheck}"
    );
}
