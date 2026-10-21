//! The checked-in catalogue of analyzers `sloptopop collect` runs: each
//! tool with its licence, the ecosystem it applies to, its fixed flags,
//! the report file it writes, the exit codes that mean "findings found"
//! rather than failure, and the container image, pinned by digest, it runs
//! from. `docs/collect.md` explains every entry.
//!
//! The flags are fixed so that output is complete (no caps on the number
//! of issues), offline (the vulnerability data is a mounted snapshot), and
//! stable from run to run. Copyleft tools (Opengrep, Semgrep,
//! golangci-lint, Pylint, the LGPL Sonar analyzers) are only ever started
//! as separate processes; nothing here links a tool. Every licence was read
//! from the tool's licence file, whose address each entry records, never
//! only from registry metadata.
//!
//! The images are built by the open-source `sloptopop/analyzer-images`
//! repository from the upstream releases, reproducibly, so the digests
//! below are the digests the component library serves (D-0184). The
//! catalogue is the signed index: it ships inside the signed release.
//!
//! Standards: SCF VPM-12 (Vulnerability Scanning), CFG-13 (Open
//! Source Software: every analyzer with its licence), CFG-11 (Software
//! Usage Restrictions: fixed, offline flags), TDA-06.5 (Software Release
//! Integrity Verification: every image pinned by its SHA-256 digest).

use std::collections::BTreeSet;

use Arg::{Fixed, Output, ReportsDir, Rules};

use crate::collect_convert::Converter;
use crate::collect_detect::Ecosystem;

/// The directory, relative to the repository root, that reports go to.
pub(crate) const REPORTS_DIR: &str = ".sloptopop/reports";

/// Where the repository is mounted (read-only) inside a container.
pub(crate) const CONTAINER_ROOT: &str = "/src";

/// Where the rule directories are mounted (read-only) inside a
/// container: the first at `/rules/0`, the second at `/rules/1`.
pub(crate) const CONTAINER_RULES: &str = "/rules";

/// Where tools whose output `collect` converts to SARIF write it, relative
/// to the root: hidden, so the engine never reads it as a report.
pub(crate) const RAW_DIR: &str = ".sloptopop/reports/.raw";

/// One element of a tool's argument list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arg {
    /// A fixed flag or value.
    Fixed(&'static str),
    /// After this prefix, the path relative to the root of the file the
    /// tool writes: the report, or its raw output if `collect` converts it.
    Output(&'static str),
    /// The reports directory relative to the root, after this prefix.
    ReportsDir(&'static str),
    /// The flag, then one rule directory, repeated for each directory
    /// (`--config <dir> --config <dir>`).
    Rules(&'static str),
}

/// Where a tool's output goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sink {
    /// The tool writes the file its arguments name ([`Output`], or a
    /// fixed name in [`ReportsDir`]).
    PathArgument,
    /// The tool prints it; `collect` writes stdout to the file.
    Stdout,
    /// The tool's configuration names the report files; the output names
    /// them with one `*`.
    Config,
}

/// A container image, pinned by the digest of its OCI image index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Image {
    /// The repository below the registry, e.g. `analyzers/gitleaks`.
    pub(crate) name: &'static str,
    /// The image's `org.opencontainers.image.version`: the upstream
    /// version and sloptopop's build revision.
    pub(crate) version: &'static str,
    /// `sha256:` and 64 lowercase hex digits (checked by the tests).
    pub(crate) digest: &'static str,
    /// Compressed bytes a pull downloads for linux/amd64 and linux/arm64.
    pub(crate) bytes: [u64; 2],
}

impl Image {
    /// The reference to pull and run: `<registry>/<name>@<digest>`.
    pub(crate) fn reference(&self, registry: &str) -> String {
        format!("{registry}/{}@{}", self.name, self.digest)
    }

    /// The value its `io.sloptopop.tool` label must have.
    pub(crate) fn tool_label(&self) -> &'static str {
        self.name.rsplit('/').next().unwrap_or(self.name)
    }

    /// The bytes a pull downloads on a host of architecture `arch` (a value
    /// of `std::env::consts::ARCH`): arm64 on `aarch64`, amd64 otherwise.
    pub(crate) fn pull_bytes(&self, arch: &str) -> u64 {
        let [amd64, arm64] = self.bytes;
        if arch == "aarch64" { arm64 } else { amd64 }
    }
}

/// When an entry runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Whenever it applies.
    Run(Image),
    /// Only in a git repository (the tool reads the history).
    NeedsGit(Image),
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

/// How an entry runs under `--host-tools` (unpinned tools from `PATH`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Host {
    /// With the same arguments.
    Same,
    /// With these arguments instead (the container ones name mounts).
    Args(&'static [Arg]),
    /// Not at all: the entry exists only as its image.
    ImageOnly,
}

/// A step that must run, with network, before the tool can run offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Prepare {
    /// `go mod download` into the shared module-cache volume.
    GoModules,
    /// `dotnet restore` into the repository's work volume.
    DotnetRestore,
}

/// Vulnerability data the tool reads from a mounted daily snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Data {
    /// The OSV data of the repository's ecosystems.
    Osv,
    /// The Go vulnerability database.
    GoVuln,
}

/// One catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tool {
    /// The entry's unique name, as `--only` takes it.
    pub(crate) name: &'static str,
    /// The executable, on the image's `PATH` (passed as `--entrypoint`).
    pub(crate) command: &'static str,
    /// The tool's licence (SPDX expression), as its licence files state.
    pub(crate) licence: &'static str,
    /// The licence files read, at a released tag or in the released
    /// archive.
    pub(crate) licence_files: &'static [&'static str],
    /// Which repositories it applies to.
    pub(crate) scope: Scope,
    /// When it runs, and from which image.
    pub(crate) kind: Kind,
    /// Whether it runs without being named by `--only`.
    pub(crate) default: bool,
    /// The arguments after the command.
    pub(crate) args: &'static [Arg],
    /// How it runs under `--host-tools`.
    pub(crate) host: Host,
    /// The report's file name in [`REPORTS_DIR`]; with [`Sink::Config`] a
    /// pattern with one `*`.
    pub(crate) output: &'static str,
    /// Where the output goes.
    pub(crate) sink: Sink,
    /// Non-zero exit codes meaning "findings found", not failure.
    pub(crate) findings: &'static [i32],
    /// The format `collect` converts the output from, and the raw file's
    /// name in [`RAW_DIR`]; `None` for a tool that writes SARIF.
    pub(crate) convert: Option<(Converter, &'static str)>,
    /// Extra environment variables in the container.
    pub(crate) env: &'static [(&'static str, &'static str)],
    /// The step that must run first, with network.
    pub(crate) prepare: Option<Prepare>,
    /// The vulnerability data it needs.
    pub(crate) data: Option<Data>,
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

    /// The path, relative to the root, of the file the tool itself writes:
    /// its raw output if `collect` converts it, otherwise the report.
    pub(crate) fn written_path(&self) -> String {
        match self.convert {
            Some((_, raw)) => format!("{RAW_DIR}/{raw}"),
            None => self.output_path(),
        }
    }

    /// Whether the tool takes rule directories ([`Rules`]).
    pub(crate) fn takes_rules(&self) -> bool {
        self.args.iter().any(|arg| matches!(arg, Rules(_)))
    }

    /// The command and its arguments, with `rules` as the rule
    /// directories; `host` selects the `--host-tools` arguments.
    pub(crate) fn argv(&self, rules: &[String], host: bool) -> Vec<String> {
        let args = match self.host {
            Host::Args(args) if host => args,
            Host::Args(_) | Host::Same | Host::ImageOnly => self.args,
        };
        std::iter::once(self.command.to_owned())
            .chain(args.iter().flat_map(|arg| self.render(*arg, rules)))
            .collect()
    }

    fn render(&self, arg: Arg, rules: &[String]) -> Vec<String> {
        match arg {
            Fixed(value) => vec![value.to_owned()],
            Output(prefix) => vec![format!("{prefix}{}", self.written_path())],
            ReportsDir(prefix) => vec![format!("{prefix}{REPORTS_DIR}")],
            Rules(flag) => rules
                .iter()
                .flat_map(|dir| [flag.to_owned(), dir.clone()])
                .collect(),
        }
    }
}

/// The value of [`Tool::findings`] and the other optional fields for a
/// plain SARIF-writing entry, so the table below stays readable.
const PLAIN: Tool = Tool {
    name: "",
    command: "",
    licence: "",
    licence_files: &[],
    scope: Scope::CrossCutting,
    kind: Kind::Documented,
    default: true,
    args: &[],
    host: Host::Same,
    output: "",
    sink: Sink::PathArgument,
    findings: &[],
    convert: None,
    env: &[],
    prepare: None,
    data: None,
};

const GITLEAKS: Image = Image {
    name: "analyzers/gitleaks",
    version: "8.30.1-1",
    digest: "sha256:464c5f5a13aa05ae98c6add4062d1a51deb4dbd1be2572fada97b2d625549510",
    bytes: [40_148_139, 39_880_487],
};

const ESLINT_SONARJS: Image = Image {
    name: "analyzers/eslint-sonarjs",
    version: "2.0.4-1",
    digest: "sha256:71327a7bcad1d2d6ba138f461f48c438852fa1b17511bddb166d23b83998ee13",
    bytes: [107_709_256, 108_049_408],
};

const GITLEAKS_LICENCE: &[&str] = &["https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE"];

const SONARJS_LICENCE: &[&str] = &[
    "https://registry.npmjs.org/eslint-plugin-sonarjs/-/eslint-plugin-sonarjs-2.0.4.tgz#package/LICENSE",
];

/// The catalogue, in the order `collect` runs and reports it.
pub(crate) const CATALOGUE: &[Tool] = &[
    // Secrets in the working tree.
    Tool {
        name: "gitleaks",
        command: "gitleaks",
        licence: "MIT",
        licence_files: GITLEAKS_LICENCE,
        kind: Kind::Run(GITLEAKS),
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
        ..PLAIN
    },
    // Secrets anywhere in the git history.
    Tool {
        name: "gitleaks-history",
        command: "gitleaks",
        licence: "MIT",
        licence_files: GITLEAKS_LICENCE,
        kind: Kind::NeedsGit(GITLEAKS),
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
        ..PLAIN
    },
    // Known-vulnerable dependencies, matched against the mounted snapshot
    // of the OSV data, fully offline.
    Tool {
        name: "osv-scanner",
        command: "osv-scanner",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/google/osv-scanner/blob/v2.7.0/LICENSE"],
        kind: Kind::Run(Image {
            name: "analyzers/osv-scanner",
            version: "2.7.0-1",
            digest: "sha256:2b5de71b5330d0cc587afe24121030ff05a377000147636661aa708d1bdf5c7f",
            bytes: [22_209_890, 20_377_564],
        }),
        args: &[
            Fixed("scan"),
            Fixed("source"),
            Fixed("-r"),
            Fixed("--offline"),
            Fixed("--format"),
            Fixed("sarif"),
            Fixed("."),
        ],
        host: Host::Args(&[
            Fixed("scan"),
            Fixed("source"),
            Fixed("-r"),
            Fixed("--offline-vulnerabilities"),
            Fixed("--download-offline-databases"),
            Fixed("--format"),
            Fixed("sarif"),
            Fixed("."),
        ]),
        output: "osv-scanner.sarif",
        sink: Sink::Stdout,
        findings: &[1, 128],
        env: &[("OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY", "/db")],
        data: Some(Data::Osv),
        ..PLAIN
    },
    // SAST with sloptopop's own rule pack or the given rules, never
    // registry rules. Rule ids are kept as written (no path prefix).
    Tool {
        name: "opengrep",
        command: "opengrep",
        licence: "LGPL-2.1-only",
        licence_files: &["https://github.com/opengrep/opengrep/blob/v1.31.0/LICENSE"],
        kind: Kind::Run(Image {
            name: "analyzers/opengrep",
            version: "1.31.0-1",
            digest: "sha256:ee955c93d35285d47ae77672516b4acaf998890ff56d4fb04f4ab9ea909c6614",
            bytes: [52_293_400, 53_856_303],
        }),
        args: &[
            Fixed("scan"),
            Fixed("--sarif"),
            Fixed("--no-rewrite-rule-ids"),
            Rules("--config"),
            Fixed("."),
        ],
        output: "opengrep.sarif",
        sink: Sink::Stdout,
        ..PLAIN
    },
    // The same rules through Semgrep CE; only with `--only semgrep`, since
    // Opengrep already runs them (decision O6, the owner may change it).
    Tool {
        name: "semgrep",
        command: "semgrep",
        licence: "LGPL-2.1-only",
        licence_files: &["https://github.com/semgrep/semgrep/blob/v1.181.0/LICENSE"],
        kind: Kind::Run(Image {
            name: "analyzers/semgrep",
            version: "1.181.0-1",
            digest: "sha256:f330febd84d4d97e1cc49821a2a71e04c3074d5af850d9e2bb520c98e03f735f",
            bytes: [128_896_632, 131_620_775],
        }),
        default: false,
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
        ..PLAIN
    },
    Tool {
        name: "bandit",
        command: "bandit",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/PyCQA/bandit/blob/1.9.4/LICENSE"],
        scope: Scope::Only(Ecosystem::Python),
        kind: Kind::Run(Image {
            name: "analyzers/bandit",
            version: "1.9.4-1",
            digest: "sha256:18e13cb5ac47c5ac2d5805cf125ef7132eb5d35cf79f2641c5300c61af7c5f9b",
            bytes: [46_386_282, 46_612_862],
        }),
        args: &[
            Fixed("-r"),
            Fixed("."),
            Fixed("-f"),
            Fixed("sarif"),
            Fixed("-o"),
            Output(""),
        ],
        output: "bandit.sarif",
        findings: &[1],
        ..PLAIN
    },
    Tool {
        name: "gosec",
        command: "gosec",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/securego/gosec/blob/v2.29.0/LICENSE.txt"],
        scope: Scope::Only(Ecosystem::Go),
        kind: Kind::Run(Image {
            name: "analyzers/gosec",
            version: "2.29.0-1",
            digest: "sha256:029df028c1d82cb14396077738d090cff2166f9eca97d172efcad21f86a7a4f5",
            bytes: [103_214_933, 98_601_993],
        }),
        args: &[
            Fixed("-fmt"),
            Fixed("sarif"),
            Fixed("-out"),
            Output(""),
            Fixed("./..."),
        ],
        output: "gosec.sarif",
        findings: &[1],
        prepare: Some(Prepare::GoModules),
        ..PLAIN
    },
    Tool {
        name: "govulncheck",
        command: "govulncheck",
        licence: "BSD-3-Clause",
        licence_files: &["https://github.com/golang/vuln/blob/v1.1.4/LICENSE"],
        scope: Scope::Only(Ecosystem::Go),
        kind: Kind::Run(Image {
            name: "analyzers/govulncheck",
            version: "1.1.4-1",
            digest: "sha256:d1fc9f1571046846b8a906ac84b793c0c67947d6cea8af56a78d6c674c75b551",
            bytes: [84_169_744, 80_293_194],
        }),
        args: &[
            Fixed("-db"),
            Fixed("file:///db/govulndb"),
            Fixed("-format"),
            Fixed("sarif"),
            Fixed("./..."),
        ],
        host: Host::Args(&[Fixed("-format"), Fixed("sarif"), Fixed("./...")]),
        output: "govulncheck.sarif",
        sink: Sink::Stdout,
        findings: &[3],
        prepare: Some(Prepare::GoModules),
        data: Some(Data::GoVuln),
        ..PLAIN
    },
    // S110 (try-except-pass), S501 (TLS verification off) and N999
    // (invalid module name) are not in Ruff's default selection; the
    // crown-jewel verdicts retire native rules to them.
    Tool {
        name: "ruff",
        command: "ruff",
        licence: "MIT",
        licence_files: &["https://github.com/astral-sh/ruff/blob/0.17.1/LICENSE"],
        scope: Scope::Only(Ecosystem::Python),
        kind: Kind::Run(Image {
            name: "analyzers/ruff",
            version: "0.17.1-1",
            digest: "sha256:9bc033618a3f3e9b3a16fd027a9ce3196e241ff5d20443f1a16675fce4305138",
            bytes: [11_570_832, 10_938_906],
        }),
        args: &[
            Fixed("check"),
            Fixed("--output-format"),
            Fixed("sarif"),
            Fixed("--output-file"),
            Output(""),
            Fixed("--no-cache"),
            Fixed("--extend-select"),
            Fixed("S110,S501,N999"),
            Fixed("."),
        ],
        output: "ruff.sarif",
        findings: &[1],
        ..PLAIN
    },
    Tool {
        name: "pylint-cyclic-import",
        command: "pylint",
        licence: "GPL-2.0-or-later",
        licence_files: &["https://github.com/pylint-dev/pylint/blob/v4.1.3/LICENSE"],
        scope: Scope::Only(Ecosystem::Python),
        kind: Kind::Run(Image {
            name: "analyzers/pylint",
            version: "4.1.3-2",
            digest: "sha256:3187804fb9f0f15017780a72d9a2f87d9c8dcb918b0933af4965d84cb22a424a",
            bytes: [45_570_074, 44_345_745],
        }),
        args: &[
            Fixed("--disable=all"),
            Fixed("--enable=cyclic-import"),
            Fixed("--output-format=json2"),
            Fixed("--recursive=y"),
            Fixed("--exit-zero"),
            Fixed("."),
        ],
        output: "pylint-cyclic-import.sarif",
        sink: Sink::Stdout,
        convert: Some((Converter::Pylint, "pylint-cyclic-import.json")),
        ..PLAIN
    },
    Tool {
        name: "oxlint",
        command: "oxlint",
        licence: "MIT",
        licence_files: &["https://github.com/oxc-project/oxc/blob/oxlint_v1.88.0/LICENSE"],
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Run(Image {
            name: "analyzers/oxlint",
            version: "1.88.0-1",
            digest: "sha256:dde16007e5c8e53707ff61899799fa6dca785d58b4bd1173dda62d15ff6ba278",
            bytes: [7_492_467, 7_020_202],
        }),
        args: &[Fixed("--format"), Fixed("sarif"), Fixed(".")],
        output: "oxlint.sarif",
        sink: Sink::Stdout,
        findings: &[1],
        ..PLAIN
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
        kind: Kind::Run(Image {
            name: "analyzers/biome",
            version: "2.5.15-1",
            digest: "sha256:0b298de6fcb4fab406a7b21390c53ae598eaa106c6e00361f5e6c390e4968c4c",
            bytes: [25_810_227, 24_210_701],
        }),
        args: &[Fixed("lint"), Fixed("--reporter=sarif"), Fixed(".")],
        output: "biome.sarif",
        sink: Sink::Stdout,
        findings: &[1],
        ..PLAIN
    },
    // eslint-plugin-sonarjs 2.0.4 (LGPL-3.0-only, the last release before
    // SonarSource's source-available licence) with its recommended rules,
    // run by the image's pinned ESLint 9.14.0 with sloptopop's own
    // configuration (never the project's). Two runs, because ESLint writes
    // one format per run: SARIF for the engine, and `json-with-metadata`,
    // which keeps each finding's fix and suggestions.
    Tool {
        name: "eslint-sonarjs",
        command: "eslint",
        licence: "LGPL-3.0-only",
        licence_files: SONARJS_LICENCE,
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Run(ESLINT_SONARJS),
        args: &[
            Fixed("--config"),
            Fixed("/opt/eslint-sonarjs/eslint.config.mjs"),
            Fixed("--no-inline-config"),
            Fixed("--no-error-on-unmatched-pattern"),
            Fixed("--format"),
            Fixed("/opt/eslint-sonarjs/node_modules/@microsoft/eslint-formatter-sarif/sarif.js"),
            Fixed("--output-file"),
            Output(""),
            Fixed("."),
        ],
        host: Host::ImageOnly,
        output: "eslint-sonarjs.sarif",
        findings: &[1],
        ..PLAIN
    },
    Tool {
        name: "eslint-sonarjs-json",
        command: "eslint",
        licence: "LGPL-3.0-only",
        licence_files: SONARJS_LICENCE,
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Run(ESLINT_SONARJS),
        args: &[
            Fixed("--config"),
            Fixed("/opt/eslint-sonarjs/eslint.config.mjs"),
            Fixed("--no-inline-config"),
            Fixed("--no-error-on-unmatched-pattern"),
            Fixed("--format"),
            Fixed("json-with-metadata"),
            Fixed("--output-file"),
            Output(""),
            Fixed("."),
        ],
        host: Host::ImageOnly,
        output: "eslint-sonarjs.json",
        findings: &[1],
        ..PLAIN
    },
    // ESLint core and typescript-eslint, with exactly the rules the
    // crown-jewel verdicts retire native rules to (decision O10).
    Tool {
        name: "eslint-typescript",
        command: "eslint",
        licence: "MIT",
        licence_files: &[
            "https://github.com/eslint/eslint/blob/v9.39.5/LICENSE",
            "https://github.com/typescript-eslint/typescript-eslint/blob/v8.71.2/LICENSE",
        ],
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Run(Image {
            name: "analyzers/eslint-typescript",
            version: "8.71.2-1",
            digest: "sha256:996e45f60992b17a1f0ec3dd85d8302d8b1ba86e1d80028985b1f46f18308c79",
            bytes: [95_199_267, 95_539_419],
        }),
        args: &[
            Fixed("--config"),
            Fixed("/opt/eslint-typescript/eslint.config.mjs"),
            Fixed("--no-inline-config"),
            Fixed("--no-error-on-unmatched-pattern"),
            Fixed("--format"),
            Fixed("/opt/eslint-typescript/node_modules/@microsoft/eslint-formatter-sarif/sarif.js"),
            Fixed("--output-file"),
            Output(""),
            Fixed("."),
        ],
        host: Host::ImageOnly,
        output: "eslint-typescript.sarif",
        findings: &[1],
        ..PLAIN
    },
    Tool {
        name: "dependency-cruiser",
        command: "depcruise",
        licence: "MIT",
        licence_files: &["https://github.com/sverweij/dependency-cruiser/blob/v18.6.0/LICENSE"],
        scope: Scope::Only(Ecosystem::Node),
        kind: Kind::Run(Image {
            name: "analyzers/dependency-cruiser",
            version: "18.6.0-1",
            digest: "sha256:09d194868d77131b245be1b5545464ff7ddec47a10d90acdd35610534d9a2f4f",
            bytes: [88_276_477, 88_616_628],
        }),
        args: &[
            Fixed("--no-progress"),
            Fixed("--config"),
            Fixed("/opt/dependency-cruiser/depcruise.cjs"),
            Fixed("--output-type"),
            Fixed("json"),
            Fixed("."),
        ],
        host: Host::ImageOnly,
        output: "dependency-cruiser.sarif",
        sink: Sink::Stdout,
        convert: Some((Converter::Depcruise, "dependency-cruiser.json")),
        ..PLAIN
    },
    Tool {
        name: "staticcheck",
        command: "staticcheck",
        licence: "MIT",
        licence_files: &["https://github.com/dominikh/go-tools/blob/2026.2.2/LICENSE"],
        scope: Scope::Only(Ecosystem::Go),
        kind: Kind::Run(Image {
            name: "analyzers/staticcheck",
            version: "2026.2.2-1",
            digest: "sha256:6ff246f23f3315ba4da366078cc83b0d5648e5f190fa538896e0f146231ff165",
            bytes: [84_786_385, 80_842_045],
        }),
        args: &[Fixed("-f"), Fixed("sarif"), Fixed("./...")],
        output: "staticcheck.sarif",
        sink: Sink::Stdout,
        findings: &[1],
        prepare: Some(Prepare::GoModules),
        ..PLAIN
    },
    Tool {
        name: "golangci-lint",
        command: "golangci-lint",
        licence: "GPL-3.0-only",
        licence_files: &["https://github.com/golangci/golangci-lint/blob/v2.14.0/LICENSE"],
        scope: Scope::Only(Ecosystem::Go),
        kind: Kind::Run(Image {
            name: "analyzers/golangci-lint",
            version: "2.14.0-1",
            digest: "sha256:9c1dce300769edcdc0feaa01508ad834e7a9d685b670f4d0bb1a259c38909350",
            bytes: [90_876_823, 86_373_864],
        }),
        args: &[
            Fixed("run"),
            Fixed("--max-issues-per-linter=0"),
            Fixed("--max-same-issues=0"),
            Fixed("--uniq-by-line=false"),
            Output("--output.sarif.path="),
            Fixed("./..."),
        ],
        output: "golangci.sarif",
        findings: &[1],
        prepare: Some(Prepare::GoModules),
        ..PLAIN
    },
    Tool {
        name: "swiftlint",
        command: "swiftlint",
        licence: "MIT",
        licence_files: &["https://github.com/realm/SwiftLint/blob/0.65.1/LICENSE"],
        scope: Scope::Only(Ecosystem::Swift),
        kind: Kind::Run(Image {
            name: "analyzers/swiftlint",
            version: "0.65.1-2",
            digest: "sha256:398cb703a29bfca494cb009040e1a7dd2a4227fb62aaf4f34e11ef1d7af42d4c",
            bytes: [276_061_429, 273_565_369],
        }),
        args: &[
            Fixed("lint"),
            Fixed("--quiet"),
            Fixed("--no-cache"),
            Fixed("--reporter"),
            Fixed("sarif"),
        ],
        output: "swiftlint.sarif",
        sink: Sink::Stdout,
        findings: &[2],
        ..PLAIN
    },
    // SonarAnalyzer.CSharp 9.32.0.97167 (LGPL-3.0-only, the last release
    // before the licence change) as Roslyn analyzers in the project's own
    // build, added by the image's Directory.Build.props, so no project file
    // changes. Each project's compiler writes its SARIF v2.1 log to the
    // reports directory.
    Tool {
        name: "sonaranalyzer-csharp",
        command: "dotnet",
        licence: "LGPL-3.0-only",
        licence_files: &[
            "https://github.com/SonarSource/sonar-dotnet/blob/9.32.0.97167/LICENSE.txt",
            "https://api.nuget.org/v3-flatcontainer/sonaranalyzer.csharp/9.32.0.97167/sonaranalyzer.csharp.9.32.0.97167.nupkg#SonarAnalyzer.CSharp.nuspec",
        ],
        scope: Scope::Only(Ecosystem::Dotnet),
        kind: Kind::Run(Image {
            name: "analyzers/sonaranalyzer-csharp",
            version: "9.32.0.97167-1",
            digest: "sha256:0bbbb5fc22d11bebc4e799b84463f3b1a28f4f2be7413d2f74203a80aabe6e36",
            bytes: [352_298_027, 344_174_207],
        }),
        args: &[
            Fixed("build"),
            Fixed("--no-restore"),
            Fixed("--no-incremental"),
            Fixed("-p:TreatWarningsAsErrors=false"),
            Fixed("-p:DirectoryBuildPropsPath=/opt/sonaranalyzer-csharp/Directory.Build.props"),
        ],
        host: Host::ImageOnly,
        output: "sonaranalyzer-csharp.*.sarif",
        sink: Sink::Config,
        prepare: Some(Prepare::DotnetRestore),
        ..PLAIN
    },
    // OpenSSF Scorecard, only its License check, on the local checkout.
    Tool {
        name: "scorecard-license",
        command: "scorecard",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/ossf/scorecard/blob/v5.6.0/LICENSE"],
        kind: Kind::Run(Image {
            name: "analyzers/scorecard",
            version: "5.6.0-1",
            digest: "sha256:7137e3ed0782e1c63e4d085e62e688a76272db9e7335d51ae4d913897c22265b",
            bytes: [27_996_640, 25_667_460],
        }),
        args: &[
            Fixed("--local=."),
            Fixed("--checks=License"),
            Fixed("--format=json"),
        ],
        output: "scorecard-license.sarif",
        sink: Sink::Stdout,
        convert: Some((Converter::Scorecard, "scorecard-license.json")),
        env: &[("GITHUB_AUTH_TOKEN", "unused-in-local-mode")],
        ..PLAIN
    },
    Tool {
        name: "jscpd",
        command: "jscpd",
        licence: "MIT",
        licence_files: &["https://github.com/kucherenko/jscpd/blob/v5.4.1/LICENSE"],
        kind: Kind::Run(Image {
            name: "analyzers/jscpd",
            version: "5.4.1-1",
            digest: "sha256:aff4ae3c4a8c9dd8318520bb44a1a8a8fb3c4876d2f8ad30770862e452b2cc7a",
            bytes: [92_318_757, 92_350_072],
        }),
        args: &[
            Fixed("--reporters"),
            Fixed("sarif"),
            ReportsDir("--output="),
            Fixed("--report-name"),
            Fixed("jscpd"),
            Fixed("--min-lines"),
            Fixed("5"),
            Fixed("--workers"),
            Fixed("1"),
            Fixed("--silent"),
            Fixed("--no-tips"),
            Fixed("--ignore"),
            Fixed("**/.sloptopop/**,**/node_modules/**"),
            Fixed("."),
        ],
        host: Host::ImageOnly,
        output: "jscpd.sarif",
        ..PLAIN
    },
    Tool {
        name: "phpcs",
        command: "phpcs",
        licence: "BSD-3-Clause",
        licence_files: &[
            "https://github.com/PHPCSStandards/PHP_CodeSniffer/blob/4.0.4/licence.txt",
        ],
        scope: Scope::Only(Ecosystem::Php),
        kind: Kind::Run(Image {
            name: "analyzers/phpcs",
            version: "4.0.4-1",
            digest: "sha256:028e8040f01f0f5d13b56f1c031620b1ede0a299638ed56bb97627f05160f121",
            bytes: [185_704_480, 177_865_741],
        }),
        args: &[
            Fixed("--standard=Squiz,Generic"),
            Fixed("--sniffs=Squiz.Classes.ClassFileName,Generic.CodeAnalysis.EmptyStatement"),
            Fixed("--extensions=php"),
            Fixed("--report=json"),
            Fixed("--no-cache"),
            Fixed("-q"),
            Fixed("."),
        ],
        output: "phpcs.sarif",
        sink: Sink::Stdout,
        findings: &[1, 2, 3],
        convert: Some((Converter::Phpcs, "phpcs.json")),
        ..PLAIN
    },
    Tool {
        name: "pmd",
        command: "pmd",
        licence: "BSD-3-Clause",
        licence_files: &["https://github.com/pmd/pmd/blob/pmd_releases/7.28.0/LICENSE"],
        scope: Scope::Only(Ecosystem::Jvm),
        kind: Kind::Run(Image {
            name: "analyzers/pmd",
            version: "7.28.0-1",
            digest: "sha256:2835550be91a2171b34961e01ed4274679b70cc84bc46f56a702bdf3d36b60d1",
            bytes: [248_410_133, 246_352_770],
        }),
        args: &[
            Fixed("check"),
            Fixed("--no-cache"),
            Fixed("--no-progress"),
            Fixed("-d"),
            Fixed("."),
            Fixed("-R"),
            Fixed("category/java/errorprone.xml/EmptyCatchBlock"),
            Fixed("-f"),
            Fixed("sarif"),
            Fixed("-r"),
            Output(""),
        ],
        output: "pmd.sarif",
        findings: &[4],
        ..PLAIN
    },
    Tool {
        name: "detekt",
        command: "detekt",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/detekt/detekt/blob/v1.23.8/LICENSE"],
        scope: Scope::Only(Ecosystem::Jvm),
        kind: Kind::Run(Image {
            name: "analyzers/detekt",
            version: "1.23.8-1",
            digest: "sha256:a5125bc6d9fae36e2508fd6f932c9ce97c393ced28bdda9a060b7f3b787edc89",
            bytes: [179_773_093, 177_715_729],
        }),
        args: &[
            Fixed("--input"),
            Fixed("."),
            Fixed("--config"),
            Fixed("/opt/detekt/sloptopop.yml"),
            Fixed("--report"),
            Output("sarif:"),
        ],
        host: Host::ImageOnly,
        output: "detekt.sarif",
        findings: &[2],
        ..PLAIN
    },
    Tool {
        name: "clang-tidy",
        command: "sloptopop-clang-tidy",
        licence: "Apache-2.0 WITH LLVM-exception",
        licence_files: &[
            "https://github.com/llvm/llvm-project/blob/llvmorg-19.1.0/clang-tools-extra/LICENSE.TXT",
        ],
        scope: Scope::Only(Ecosystem::Cpp),
        kind: Kind::Run(Image {
            name: "analyzers/clang-tidy",
            version: "19.1.0-1",
            digest: "sha256:6c12730b2a734cbe2430b8dcef41b2bd9c4a636dbdec716ca6f6a478db28b5d3",
            bytes: [68_524_962, 68_624_873],
        }),
        args: &[Fixed("-*,bugprone-empty-catch")],
        host: Host::ImageOnly,
        output: "clang-tidy.sarif",
        sink: Sink::Stdout,
        convert: Some((Converter::ClangTidy, "clang-tidy.txt")),
        ..PLAIN
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
        args: &[
            Fixed("llvm-cov"),
            Fixed("--lcov"),
            Fixed("--output-path"),
            Output(""),
        ],
        output: "cargo.lcov",
        ..PLAIN
    },
    Tool {
        name: "coverage.py",
        command: "coverage",
        licence: "Apache-2.0",
        licence_files: &["https://github.com/nedbat/coveragepy/blob/7.16.2/LICENSE.txt"],
        scope: Scope::Only(Ecosystem::Python),
        args: &[Fixed("lcov"), Fixed("-o"), Output("")],
        output: "python.lcov",
        ..PLAIN
    },
    Tool {
        name: "c8",
        command: "c8",
        licence: "ISC",
        licence_files: &["https://github.com/bcoe/c8/blob/v12.0.0/LICENSE.txt"],
        scope: Scope::Only(Ecosystem::Node),
        args: &[
            Fixed("--reporter=lcov"),
            ReportsDir("--reports-dir="),
            Fixed("npm"),
            Fixed("test"),
        ],
        output: "lcov.info",
        ..PLAIN
    },
    Tool {
        name: "coverlet",
        command: "dotnet",
        licence: "MIT",
        licence_files: &["https://github.com/coverlet-coverage/coverlet/blob/v10.1.0/LICENSE"],
        scope: Scope::Only(Ecosystem::Dotnet),
        args: &[
            Fixed("test"),
            Fixed("/p:CollectCoverage=true"),
            Fixed("/p:CoverletOutputFormat=lcov"),
            Output("/p:CoverletOutput="),
        ],
        output: "dotnet.lcov",
        ..PLAIN
    },
    Tool {
        name: "gcovr",
        command: "gcovr",
        licence: "BSD-3-Clause",
        licence_files: &["https://github.com/gcovr/gcovr/blob/8.6/LICENSE.txt"],
        scope: Scope::Only(Ecosystem::Cpp),
        args: &[Fixed("--lcov"), Output("")],
        output: "cpp.lcov",
        ..PLAIN
    },
];

#[cfg(test)]
mod tests;
