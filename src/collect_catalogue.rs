//! The checked-in catalogue of analyzers `sloptopop collect` runs: each
//! tool with its licence, the ecosystem it applies to, its fixed flags,
//! the report file it writes and the exit codes that mean "findings
//! found" rather than failure. `docs/collect.md` explains every entry.
//!
//! The flags are fixed so that output is complete (no caps on the number
//! of issues), offline where the tool allows it, and stable from run to
//! run. Copyleft tools (Opengrep, Semgrep, golangci-lint) are only ever
//! started as separate processes; nothing here links or bundles a tool.

use std::collections::BTreeSet;

use Arg::{Fixed, Output, ReportsDir, Rules};

use crate::collect_detect::Ecosystem;

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
    /// The local rules directory.
    Rules,
}

/// Where a tool's report goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sink {
    /// The tool writes the file named by its [`Output`] argument.
    PathArgument,
    /// The tool prints the report; `collect` writes stdout to the file.
    Stdout,
}

/// When an entry runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Whenever the tool is installed and applies.
    Run,
    /// Only in a git repository (the tool reads the history).
    NeedsGit,
    /// Only with a local rules directory (never registry rules).
    NeedsRules,
    /// Never run: the command needs the project's own test run, so it is
    /// only documented.
    Documented,
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
    /// The tool's licence (SPDX expression).
    pub(crate) licence: &'static str,
    /// Which repositories it applies to.
    pub(crate) scope: Scope,
    /// When it runs.
    pub(crate) kind: Kind,
    /// The arguments after the command.
    pub(crate) args: &'static [Arg],
    /// The report's file name in [`REPORTS_DIR`].
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

    /// The command and its arguments, with `rules` as the rules directory.
    pub(crate) fn argv(&self, rules: &str) -> Vec<String> {
        let output = self.output_path();
        std::iter::once(self.command.to_owned())
            .chain(self.args.iter().map(|arg| match arg {
                Fixed(value) => (*value).to_owned(),
                Output(prefix) => format!("{prefix}{output}"),
                ReportsDir(prefix) => format!("{prefix}{REPORTS_DIR}"),
                Rules => rules.to_owned(),
            }))
            .collect()
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
    // SAST and taint with local rules only.
    Tool {
        name: "opengrep",
        command: "opengrep",
        licence: "LGPL-2.1-only",
        scope: Scope::CrossCutting,
        kind: Kind::NeedsRules,
        args: &[
            Fixed("scan"),
            Fixed("--sarif"),
            Fixed("--config"),
            Rules,
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
        scope: Scope::CrossCutting,
        kind: Kind::NeedsRules,
        args: &[
            Fixed("scan"),
            Fixed("--sarif"),
            Fixed("--metrics"),
            Fixed("off"),
            Fixed("--disable-version-check"),
            Fixed("--config"),
            Rules,
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
    // Coverage producers: documented only, never run.
    Tool {
        name: "cargo-llvm-cov",
        command: "cargo",
        licence: "Apache-2.0 OR MIT",
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
