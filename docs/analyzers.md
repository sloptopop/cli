# Analyzers

sloptopop's own rules find many problems. Open-source analyzers find many
more. `sloptopop collect` runs the analyzers you have installed and saves
their reports, and `sloptopop check` reads those reports as evidence.

This page is the overview: which analyzers exist for your stack and under
which licence. The full reference, with every command line, flag and
licence source, is [collect.md](collect.md).

## How it works

```
sloptopop collect      # runs the installed analyzers → .sloptopop/reports/
sloptopop check        # reads .sloptopop/reports/ with sloptopop's own rules
```

- `collect` detects your ecosystems from the manifests at the repository
  root (`package.json`, `pyproject.toml`, `go.mod`, …).
- For each analyzer that applies, it looks the executable up on your
  `PATH`. A tool that is not installed is reported as `missing`.
  **Nothing is ever installed or downloaded for you.**
- Each tool runs as a separate process, with fixed flags, from the
  repository root. Its report goes to `.sloptopop/reports/`.
- A tool that finds problems counts as `ran`. Only a real failure counts
  as `failed`. One failure never stops the other tools.
- `collect` makes sure git ignores the reports, because they can hold
  secret locations and machine paths.
- The engine never runs a tool itself. It only reads SARIF 2.1.0 and LCOV
  files.

In the Report, an analyzer's result appears as `analyzer.finding` (with
the tool and its own rule id in `origin`), a secret scanner's as
`secrets.committed` or `secrets.in-history`, and coverage gaps as
`coverage.uncovered`. See [rules.md](rules.md#analyzer-reports).

Try `sloptopop collect --dry-run` first: it prints what would run, and
what is missing, without running anything.

## Supported analyzers

### Every repository

| Tool | Finds | Licence |
|---|---|---|
| gitleaks | secrets in the working tree (`gitleaks`) and in the whole git history (`gitleaks-history`) | MIT |
| OSV-Scanner | dependencies with known vulnerabilities, matched offline | Apache-2.0 |
| Opengrep | code patterns, with sloptopop's own rule pack | LGPL-2.1-only |
| Semgrep | code patterns, with sloptopop's own rule pack | LGPL-2.1-only |

Opengrep and Semgrep always run with sloptopop's own rules
([rules/](../rules/README.md), MIT OR Apache-2.0) plus your
`.sloptopop/rules/` if it exists. They never fetch registry rules. You
need only one of the two.

### By language

| Ecosystem | Tool | Licence |
|---|---|---|
| JavaScript, TypeScript | oxlint | MIT |
| JavaScript, TypeScript | Biome | MIT OR Apache-2.0 |
| JavaScript, TypeScript | eslint-plugin-sonarjs 2.0.4 (managed, see below) | LGPL-3.0-only |
| Python | Ruff | MIT |
| Python | Bandit | Apache-2.0 |
| Go | staticcheck | MIT |
| Go | golangci-lint | GPL-3.0-only |
| Go | gosec | Apache-2.0 |
| Go | govulncheck | BSD-3-Clause |
| Swift | SwiftLint | MIT |
| C# (.NET) | SonarAnalyzer.CSharp 9.32.0.97167 (managed, see below) | LGPL-3.0-only |

### Coverage

Coverage needs your own test run, so `collect` never runs these. It lists
them as `skipped`, with the command to run after your tests. `check` then
reads the LCOV files they write.

| Ecosystem | Tool | Licence | Command |
|---|---|---|---|
| Rust | cargo-llvm-cov | Apache-2.0 OR MIT | `cargo llvm-cov --lcov --output-path .sloptopop/reports/cargo.lcov` |
| Python | coverage.py | Apache-2.0 | `coverage lcov -o .sloptopop/reports/python.lcov` (after `coverage run -m pytest`) |
| JavaScript, TypeScript | c8 | ISC | `c8 --reporter=lcov --reports-dir=.sloptopop/reports npm test` |
| .NET | coverlet | MIT | `dotnet test /p:CollectCoverage=true /p:CoverletOutputFormat=lcov /p:CoverletOutput=.sloptopop/reports/dotnet.lcov` |
| C, C++ | gcovr | BSD-3-Clause | `gcovr --lcov .sloptopop/reports/cpp.lcov` |

Vitest and Jest can write LCOV themselves: point their `lcov` reporter at
`.sloptopop/reports/`.

## Managed analyzers

Two SonarSource analyzers are used at their **last releases under
LGPL-3.0-only**. Every later release is under a source-available licence
that is not open source, so sloptopop pins these exact versions.

They are not standalone executables, so `collect` gives each its own
directory under `.sloptopop/tools/`. It writes the pinned manifest there
and reports the tool as `missing` until you install it once, from the
repository root:

| Tool | Install command |
|---|---|
| `eslint-sonarjs` | `npm ci --ignore-scripts --prefix .sloptopop/tools/eslint-sonarjs` |
| `sonaranalyzer-csharp` | `dotnet restore .sloptopop/tools/sonaranalyzer-csharp/fetch.proj` |

They never enter your project's own dependencies or lock files. Do not
upgrade them: see [collect.md](collect.md#managed-tools-sloptopoptools).

## Running in containers

`sloptopop collect --container` runs each tool from a container image
pinned by digest, with no network, a read-only source mount and all
capabilities dropped. Only the reports directory is writable. An entry
whose image digest is not pinned yet is skipped, and so are the managed
analyzers. See [collect.md](collect.md#--container).

## Licences

Every analyzer above is under an OSI-approved licence. sloptopop only
starts them as separate processes and reads the files they write. Nothing
is linked into `sloptopop` or bundled with it, so the copyleft licences
(LGPL, GPL) put no obligation on your code or on sloptopop. Each licence
was read from the tool's own licence file at a release tag. The sources
are listed in [collect.md](collect.md#licences).

## Network use

`collect` sends nothing to sloptopop. Two analyzers use the network
themselves:

- OSV-Scanner downloads its vulnerability database. Your dependency list
  stays on your machine.
- govulncheck looks up the module paths it finds in the Go vulnerability
  database (vuln.go.dev).

Semgrep runs with `--metrics off --disable-version-check`, so it sends no
usage data.

## Your own reports

You can also put reports there yourself. Any `*.sarif` or `*.sarif.json`
file (SARIF 2.1.0) and any `*.lcov`, `lcov.info` or `*.lcov.info` file in
`.sloptopop/reports/` is read by the next `check`. A report that cannot be
read is reported as `analyzer.unreadable-report`, never silently ignored.
