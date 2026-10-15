//! The checked-in catalogue of analyzers `sloptopop collect` runs: each
//! tool with its licence, the ecosystem it applies to, its fixed flags,
//! the report file it writes and the exit codes that mean "findings
//! found" rather than failure. `docs/collect.md` explains every entry.
//!
//! The flags are fixed so that output is complete (no caps on the number
//! of issues), offline where the tool allows it, and stable from run to
//! run. Copyleft tools (Opengrep, Semgrep, golangci-lint, the LGPL Sonar
//! analyzers) are only ever started as separate processes; nothing here
//! links or bundles a tool. Every licence was read from the tool's licence
//! file, whose address each entry records, never only from registry
//! metadata.

use std::collections::BTreeSet;

use Arg::{Eslint, Fixed, Output, ReportsDir, Rules, Tools};

use crate::collect_detect::Ecosystem;
use crate::collect_managed::Managed;

/// The directory, relative to the repository root, that reports go to.
pub(crate) const REPORTS_DIR: &str = ".sloptopop/reports";

/// One element of a tool's argument list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arg {
    /// A fixed flag or value.
    Fixed(&'static str),
    /// The report file's path relative to the root, after this prefix.
    Output(&'static str),
    /// The reports directory relative to the root, after this prefix.
    ReportsDir(&'static str),
    /// The flag, then one rule directory, repeated for each directory
    /// (`--config <dir> --config <dir>`).
    Rules(&'static str),
    /// After the prefix (first), the absolute path of this file (second)
    /// in the managed tools directory.
    Tools(&'static str, &'static str),
    /// The `ESLint` command-line script (the project's or the managed one).
    Eslint,
}

/// What an argument list is rendered with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Inputs<'a> {
    /// The rule directories, for [`Rules`].
    pub(crate) rules: &'a [String],
    /// The managed tools directory, for [`Tools`].
    pub(crate) tools: &'a str,
    /// The `ESLint` script, for [`Eslint`].
    pub(crate) eslint: &'a str,
}

/// Where a tool's report goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sink {
    /// The tool writes the file named by its [`Output`] argument.
    PathArgument,
    /// The tool prints the report; `collect` writes stdout to the file.
    Stdout,
    /// The files `collect` writes to the tool's managed directory name the
    /// report files; the output names them with one `*`.
    Config,
}

/// When an entry runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Whenever the tool is installed and applies.
    Run,
    /// Only in a git repository (the tool reads the history).
    NeedsGit,
    /// Never run: the command needs the project's own test run, so it is
    /// only documented.
    Documented,
    /// Whenever it applies and its pinned packages are installed in its
    /// managed directory.
    Managed(Managed),
}

/// Which repositories an entry applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Every repository.
    CrossCutting,
    /// Repositories using this ecosystem.
    Only(Ecosystem),
}

/// What network a containerised run gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Network {
    /// None at all (`--network none`).
    Offline,
    /// The default network, only to download a vulnerability database;
    /// the scan itself stays offline and sends nothing about the code.
    DatabaseDownload,
}

/// A container image for `--container`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Image {
    /// The image name without tag or digest.
    pub(crate) repository: &'static str,
    /// The pinned `sha256:` digest; an entry without one never runs.
    pub(crate) digest: Option<&'static str>,
}

impl Image {
    /// `repository@sha256:<digest>`, if the digest is a well-formed
    /// SHA-256 digest.
    pub(crate) fn reference(&self) -> Option<String> {
        let digest = self.digest?;
        let hex = digest.strip_prefix("sha256:")?;
        let well_formed =
            hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        well_formed.then(|| format!("{}@{digest}", self.repository))
    }
}

/// One catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tool {
    /// The entry's unique name, as `--only` takes it.
    pub(crate) name: &'static str,
    /// The executable.
    pub(crate) command: &'static str,
    /// The tool's licence (SPDX expression), as its licence files state.
    pub(crate) licence: &'static str,
    /// The licence files read, at a released tag or in the released
    /// archive.
    pub(crate) licence_files: &'static [&'static str],
    /// Which repositories it applies to.
    pub(crate) scope: Scope,
    /// When it runs.
    pub(crate) kind: Kind,
    /// The arguments after the command.
    pub(crate) args: &'static [Arg],
    /// The report's file name in [`REPORTS_DIR`]; with [`Sink::Config`] a
    /// pattern with one `*`.
    pub(crate) output: &'static str,
    /// Where the report goes.
    pub(crate) sink: Sink,
    /// Non-zero exit codes meaning "findings found", not failure.
    pub(crate) findings: &'static [i32],
    /// Network for a containerised run.
    pub(crate) network: Network,
    /// The image for `--container`, if one is known.
    pub(crate) image: Option<Image>,
}

impl Tool {
    /// Whether the entry applies to a repository using `ecosystems`.
    pub(crate) fn applies(&self, ecosystems: &BTreeSet<Ecosystem>) -> bool {
        match self.scope {
            Scope::CrossCutting => true,
            Scope::Only(ecosystem) => ecosystems.contains(&ecosystem),
        }
    }

    /// The report's path relative to the root.
    pub(crate) fn output_path(&self) -> String {
        format!("{REPORTS_DIR}/{}", self.output)
    }

    /// Whether the tool takes rule directories ([`Rules`]).
    pub(crate) fn takes_rules(&self) -> bool {
        self.args.iter().any(|arg| matches!(arg, Rules(_)))
    }

    /// The command and its arguments.
    pub(crate) fn argv(&self, inputs: &Inputs<'_>) -> Vec<String> {
        std::iter::once(self.command.to_owned())
            .chain(self.args.iter().flat_map(|arg| self.render(*arg, inputs)))
            .collect()
    }

    /// The arguments after the command that print the tool's version: the
    /// script a [`Eslint`] entry starts with, then `--version`.
    pub(crate) fn version_args(&self, inputs: &Inputs<'_>) -> Vec<String> {
        self.args
            .iter()
            .take_while(|arg| matches!(arg, Eslint))
            .flat_map(|arg| self.render(*arg, inputs))
            .chain(std::iter::once("--version".to_owned()))
            .collect()
    }

    fn render(&self, arg: Arg, inputs: &Inputs<'_>) -> Vec<String> {
        match arg {
            Fixed(value) => vec![value.to_owned()],
            Output(prefix) => vec![format!("{prefix}{}", self.output_path())],
            ReportsDir(prefix) => vec![format!("{prefix}{REPORTS_DIR}")],
            Rules(flag) => inputs
                .rules
                .iter()
                .flat_map(|dir| [flag.to_owned(), dir.clone()])
                .collect(),
            Tools(prefix, file) => vec![format!("{prefix}{}/{file}", inputs.tools)],
            Eslint => vec![inputs.eslint.to_owned()],
        }
    }
}

/// The Tier-1 catalogue, in the order `collect` runs and reports it.
///
/// Image digests are left for the owner to pin (`TODO(owner)` below): an
/// entry without a digest is skipped under `--container`. Pick images
/// whose `PATH` holds the command; it is passed as `--entrypoint`.
pub(crate) const CATALOGUE: &[Tool] = &[
    // Secrets in the working tree.
    Tool {
        name: "gitleaks",
        command: "gitleaks",
        licence: "MIT",
        licence_files: &["https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE"],
        scope: Scope::CrossCutting,
        kind: Kind::Run,
        args: &[
            Fixed("dir"),
            Fixed("."),
            Fixed("--redact"),
            Fixed("--report-format"),
            Fixed("sarif"),
            Fixed("--report-path"),
            Output(""),
            Fixed("--exit-code"),
            Fixed("0"),
        ],
        output: "gitleaks.sarif",
        sink: Sink::PathArgument,
        findings: &[],
        network: Network::Offline,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "ghcr.io/gitleaks/gitleaks",
            digest: None,
        }),
    },
    // Secrets anywhere in the git history.
    Tool {
        name: "gitleaks-history",
        command: "gitleaks",
        licence: "MIT",
        licence_files: &["https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE"],
        scope: Scope::CrossCutting,
        kind: Kind::NeedsGit,
        args: &[
            Fixed("git"),
            Fixed("."),
            Fixed("--redact"),
            Fixed("--report-format"),
            Fixed("sarif"),
            Fixed("--report-path"),
            Output(""),
            Fixed("--exit-code"),
            Fixed("0"),
        ],
        output: "gitleaks.history.sarif",
        sink: Sink::PathArgument,
        findings: &[],
        network: Network::Offline,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "ghcr.io/gitleaks/gitleaks",
            digest: None,
        }),
    },
    // Known-vulnerable dependencies, matched against a local database.
    Tool {
        name: "osv-scanner",
        command: "osv-scanner",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/google/osv-scanner/blob/v2.7.0/LICENSE"],
        scope: Scope::CrossCutting,
        kind: Kind::Run,
        args: &[
            Fixed("scan"),
            Fixed("source"),
            Fixed("-r"),
            Fixed("--offline-vulnerabilities"),
            Fixed("--download-offline-databases"),
            Fixed("--format"),
            Fixed("sarif"),
            Fixed("."),
        ],
        output: "osv-scanner.sarif",
        sink: Sink::Stdout,
        findings: &[1, 128],
        network: Network::DatabaseDownload,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "ghcr.io/google/osv-scanner",
            digest: None,
        }),
    },
    // SAST with sloptopop's own rule pack or the given rules, never
    // registry rules. Rule ids are kept as written (no path prefix).
    Tool {
        name: "opengrep",
        command: "opengrep",
        licence: "LGPL-2.1-only",
        licence_files: &["https://github.com/opengrep/opengrep/blob/v1.31.0/LICENSE"],
        scope: Scope::CrossCutting,
        kind: Kind::Run,
        args: &[
            Fixed("scan"),
            Fixed("--sarif"),
            Fixed("--no-rewrite-rule-ids"),
            Rules("--config"),
            Fixed("."),
        ],
        output: "opengrep.sarif",
        sink: Sink::Stdout,
        findings: &[],
        network: Network::Offline,
        // TODO(owner): choose an image and pin its digest.
        image: None,
    },
    Tool {
        name: "semgrep",
        command: "semgrep",
        licence: "LGPL-2.1-only",
        licence_files: &["https://github.com/semgrep/semgrep/blob/v1.181.0/LICENSE"],
        scope: Scope::CrossCutting,
        kind: Kind::Run,
        args: &[
            Fixed("scan"),
            Fixed("--sarif"),
            Fixed("--metrics"),
            Fixed("off"),
            Fixed("--disable-version-check"),
            Fixed("--no-rewrite-rule-ids"),
            Rules("--config"),
            Fixed("."),
        ],
        output: "semgrep.sarif",
        sink: Sink::Stdout,
        findings: &[],
        network: Network::Offline,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "semgrep/semgrep",
            digest: None,
        }),
    },
    Tool {
        name: "bandit",
        command: "bandit",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/PyCQA/bandit/blob/1.9.4/LICENSE"],
        scope: Scope::Only(Ecosystem::Python),
        kind: Kind::Run,
        args: &[
            Fixed("-r"),
            Fixed("."),
            Fixed("-f"),
            Fixed("sarif"),
            Fixed("-o"),
            Output(""),
        ],
        output: "bandit.sarif",
        sink: Sink::PathArgument,
        findings: &[1],
        network: Network::Offline,
        // TODO(owner): choose an image and pin its digest.
        image: None,
    },
    Tool {
        name: "gosec",
        command: "gosec",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/securego/gosec/blob/v2.29.0/LICENSE.txt"],
        scope: Scope::Only(Ecosystem::Go),
        kind: Kind::Run,
        args: &[
            Fixed("-fmt"),
            Fixed("sarif"),
            Fixed("-out"),
            Output(""),
            Fixed("./..."),
        ],
        output: "gosec.sarif",
        sink: Sink::PathArgument,
        findings: &[1],
        network: Network::Offline,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "securego/gosec",
            digest: None,
        }),
    },
    Tool {
        name: "govulncheck",
        command: "govulncheck",
        licence: "BSD-3-Clause",
        licence_files: &["https://github.com/golang/vuln/blob/v1.1.4/LICENSE"],
        scope: Scope::Only(Ecosystem::Go),
        kind: Kind::Run,
        args: &[Fixed("-format"), Fixed("sarif"), Fixed("./...")],
        output: "govulncheck.sarif",
        sink: Sink::Stdout,
        findings: &[3],
        network: Network::Offline,
        // TODO(owner): choose an image and pin its digest.
        image: None,
    },
    Tool {
        name: "ruff",
        command: "ruff",
        licence: "MIT",
        licence_files: &["https://github.com/astral-sh/ruff/blob/0.17.1/LICENSE"],
        scope: Scope::Only(Ecosystem::Python),
        kind: Kind::Run,
        args: &[
            Fixed("check"),
            Fixed("--output-format"),
            Fixed("sarif"),
            Fixed("--output-file"),
            Output(""),
            Fixed("--no-cache"),
            Fixed("."),
        ],
        output: "ruff.sarif",
        sink: Sink::PathArgument,
        findings: &[1],
        network: Network::Offline,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "ghcr.io/astral-sh/ruff",
            digest: None,
        }),
    },
    Tool {
        name: "oxlint",
        command: "oxlint",
        licence: "MIT",
        licence_files: &["https://github.com/oxc-project/oxc/blob/oxlint_v1.88.0/LICENSE"],
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Run,
        args: &[Fixed("--format"), Fixed("sarif"), Fixed(".")],
        output: "oxlint.sarif",
        sink: Sink::Stdout,
        findings: &[1],
        network: Network::Offline,
        // TODO(owner): choose an image and pin its digest.
        image: None,
    },
    Tool {
        name: "biome",
        command: "biome",
        licence: "MIT OR Apache-2.0",
        licence_files: &[
            "https://github.com/biomejs/biome/blob/@biomejs/biome@2.5.15/LICENSE-MIT",
            "https://github.com/biomejs/biome/blob/@biomejs/biome@2.5.15/LICENSE-APACHE",
        ],
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Run,
        args: &[Fixed("lint"), Fixed("--reporter=sarif"), Fixed(".")],
        output: "biome.sarif",
        sink: Sink::Stdout,
        findings: &[1],
        network: Network::Offline,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "ghcr.io/biomejs/biome",
            digest: None,
        }),
    },
    Tool {
        name: "staticcheck",
        command: "staticcheck",
        licence: "MIT",
        licence_files: &["https://github.com/dominikh/go-tools/blob/2026.2.2/LICENSE"],
        scope: Scope::Only(Ecosystem::Go),
        kind: Kind::Run,
        args: &[Fixed("-f"), Fixed("sarif"), Fixed("./...")],
        output: "staticcheck.sarif",
        sink: Sink::Stdout,
        findings: &[1],
        network: Network::Offline,
        // TODO(owner): choose an image and pin its digest.
        image: None,
    },
    Tool {
        name: "golangci-lint",
        command: "golangci-lint",
        licence: "GPL-3.0-only",
        licence_files: &["https://github.com/golangci/golangci-lint/blob/v2.14.0/LICENSE"],
        scope: Scope::Only(Ecosystem::Go),
        kind: Kind::Run,
        args: &[
            Fixed("run"),
            Fixed("--max-issues-per-linter=0"),
            Fixed("--max-same-issues=0"),
            Fixed("--uniq-by-line=false"),
            Output("--output.sarif.path="),
            Fixed("./..."),
        ],
        output: "golangci.sarif",
        sink: Sink::PathArgument,
        findings: &[1],
        network: Network::Offline,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "golangci/golangci-lint",
            digest: None,
        }),
    },
    Tool {
        name: "swiftlint",
        command: "swiftlint",
        licence: "MIT",
        licence_files: &["https://github.com/realm/SwiftLint/blob/0.65.1/LICENSE"],
        scope: Scope::Only(Ecosystem::Swift),
        kind: Kind::Run,
        args: &[
            Fixed("lint"),
            Fixed("--quiet"),
            Fixed("--reporter"),
            Fixed("sarif"),
        ],
        output: "swiftlint.sarif",
        sink: Sink::Stdout,
        findings: &[2],
        network: Network::Offline,
        // TODO(owner): pin the digest.
        image: Some(Image {
            repository: "ghcr.io/realm/swiftlint",
            digest: None,
        }),
    },
    // eslint-plugin-sonarjs 2.0.4 (LGPL-3.0-only, the last release before
    // SonarSource's source-available licence) with its recommended rules,
    // run by ESLint from the managed directory with sloptopop's own
    // configuration (never the project's). Two runs, because ESLint writes
    // one format per run: SARIF for the engine, and `json-with-metadata`,
    // which keeps each finding's fix and suggestions.
    Tool {
        name: "eslint-sonarjs",
        command: "node",
        licence: "LGPL-3.0-only",
        licence_files: &[
            "https://registry.npmjs.org/eslint-plugin-sonarjs/-/eslint-plugin-sonarjs-2.0.4.tgz#package/LICENSE",
        ],
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Managed(Managed::EslintSonarjs),
        args: &[
            Eslint,
            Fixed("--config"),
            Tools("", "eslint-sonarjs/eslint.config.mjs"),
            Fixed("--no-inline-config"),
            Fixed("--no-error-on-unmatched-pattern"),
            Fixed("--format"),
            Tools(
                "",
                "eslint-sonarjs/node_modules/@microsoft/eslint-formatter-sarif/sarif.js",
            ),
            Fixed("--output-file"),
            Output(""),
            Fixed("."),
        ],
        output: "eslint-sonarjs.sarif",
        sink: Sink::PathArgument,
        findings: &[1],
        network: Network::Offline,
        image: None,
    },
    Tool {
        name: "eslint-sonarjs-json",
        command: "node",
        licence: "LGPL-3.0-only",
        licence_files: &[
            "https://registry.npmjs.org/eslint-plugin-sonarjs/-/eslint-plugin-sonarjs-2.0.4.tgz#package/LICENSE",
        ],
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Managed(Managed::EslintSonarjs),
        args: &[
            Eslint,
            Fixed("--config"),
            Tools("", "eslint-sonarjs/eslint.config.mjs"),
            Fixed("--no-inline-config"),
            Fixed("--no-error-on-unmatched-pattern"),
            Fixed("--format"),
            Fixed("json-with-metadata"),
            Fixed("--output-file"),
            Output(""),
            Fixed("."),
        ],
        output: "eslint-sonarjs.json",
        sink: Sink::PathArgument,
        findings: &[1],
        network: Network::Offline,
        image: None,
    },
    // SonarAnalyzer.CSharp 9.32.0.97167 (LGPL-3.0-only, the last release
    // before the licence change) as Roslyn analyzers in the project's own
    // build, added by a generated Directory.Build.props in the managed
    // directory, so no project file changes. Each project's compiler
    // writes its SARIF v2.1 log to the reports directory.
    Tool {
        name: "sonaranalyzer-csharp",
        command: "dotnet",
        licence: "LGPL-3.0-only",
        licence_files: &[
            "https://github.com/SonarSource/sonar-dotnet/blob/9.32.0.97167/LICENSE.txt",
            "https://api.nuget.org/v3-flatcontainer/sonaranalyzer.csharp/9.32.0.97167/sonaranalyzer.csharp.9.32.0.97167.nupkg#SonarAnalyzer.CSharp.nuspec",
        ],
        scope: Scope::Only(Ecosystem::Dotnet),
        kind: Kind::Managed(Managed::SonarCsharp),
        args: &[
            Fixed("build"),
            Fixed("--no-restore"),
            Fixed("--no-incremental"),
            Fixed("-p:TreatWarningsAsErrors=false"),
            Tools(
                "-p:DirectoryBuildPropsPath=",
                "sonaranalyzer-csharp/Directory.Build.props",
            ),
        ],
        output: "sonaranalyzer-csharp.*.sarif",
        sink: Sink::Config,
        findings: &[],
        network: Network::Offline,
        image: None,
    },
    // Coverage producers: documented only, never run.
    Tool {
        name: "cargo-llvm-cov",
        command: "cargo",
        licence: "Apache-2.0 OR MIT",
        licence_files: &[
            "https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/LICENSE-APACHE",
            "https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/LICENSE-MIT",
        ],
        scope: Scope::Only(Ecosystem::Rust),
        kind: Kind::Documented,
        args: &[
            Fixed("llvm-cov"),
            Fixed("--lcov"),
            Fixed("--output-path"),
            Output(""),
        ],
        output: "cargo.lcov",
        sink: Sink::PathArgument,
        findings: &[],
        network: Network::Offline,
        image: None,
    },
    Tool {
        name: "coverage.py",
        command: "coverage",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/nedbat/coveragepy/blob/7.16.2/LICENSE.txt"],
        scope: Scope::Only(Ecosystem::Python),
        kind: Kind::Documented,
        args: &[Fixed("lcov"), Fixed("-o"), Output("")],
        output: "python.lcov",
        sink: Sink::PathArgument,
        findings: &[],
        network: Network::Offline,
        image: None,
    },
    Tool {
        name: "c8",
        command: "c8",
        licence: "ISC",
        licence_files: &["https://github.com/bcoe/c8/blob/v12.0.0/LICENSE.txt"],
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Documented,
        args: &[
            Fixed("--reporter=lcov"),
            ReportsDir("--reports-dir="),
            Fixed("npm"),
            Fixed("test"),
        ],
        output: "lcov.info",
        sink: Sink::PathArgument,
        findings: &[],
        network: Network::Offline,
        image: None,
    },
    Tool {
        name: "coverlet",
        command: "dotnet",
        licence: "MIT",
        licence_files: &["https://github.com/coverlet-coverage/coverlet/blob/v10.1.0/LICENSE"],
        scope: Scope::Only(Ecosystem::Dotnet),
        kind: Kind::Documented,
        args: &[
            Fixed("test"),
            Fixed("/p:CollectCoverage=true"),
            Fixed("/p:CoverletOutputFormat=lcov"),
            Output("/p:CoverletOutput="),
        ],
        output: "dotnet.lcov",
        sink: Sink::PathArgument,
        findings: &[],
        network: Network::Offline,
        image: None,
    },
    Tool {
        name: "gcovr",
        command: "gcovr",
        licence: "BSD-3-Clause",
        licence_files: &["https://github.com/gcovr/gcovr/blob/8.6/LICENSE.txt"],
        scope: Scope::Only(Ecosystem::Cpp),
        kind: Kind::Documented,
        args: &[Fixed("--lcov"), Output("")],
        output: "cpp.lcov",
        sink: Sink::PathArgument,
        findings: &[],
        network: Network::Offline,
        image: None,
    },
];

#[cfg(test)]
mod tests;
