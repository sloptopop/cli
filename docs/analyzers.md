# Analyzers

sloptopop's own rules find many problems. Open-source analyzers find many
more. `sloptopop collect` runs the analyzers that apply to your repository
and saves their reports, and `sloptopop check` reads those reports as
evidence.

This page is the overview: which analyzers exist for your stack and under
which licence. The full reference, with every image, command line, flag
and licence source, is [collect.md](collect.md).

## How it works

```
sloptopop collect      # runs the analyzers from pinned images → .sloptopop/reports/
sloptopop check        # reads .sloptopop/reports/ with sloptopop's own rules
```

- `collect` detects your ecosystems from the manifests at the repository
  root (`package.json`, `pyproject.toml`, `go.mod`, …).
- Each analyzer that applies runs from **its own container image, built
  and pinned by sloptopop**: the version sloptopop tested, whatever is
  installed on your machine. You need a container runtime (Docker Engine,
  Podman, Colima, Rancher Desktop or Docker Desktop); nothing else.
- An image that is not on your machine yet is announced (tool, version,
  size, source, licence), then pulled by its SHA-256 digest and checked.
  The next run uses it without fetching it again.
- Each tool runs without network, with your repository read-only and only
  `.sloptopop/reports/` writable, without privileges, with memory, process
  and time limits.
- A tool that finds problems counts as `ran`. Only a real failure counts as
  `failed`. One failure never stops the other tools.
- `collect` makes sure git ignores the reports, because they can hold
  secret locations and machine paths.
- The engine never runs a tool itself. It only reads SARIF 2.1.0 and LCOV
  files.

In the Report, an analyzer's result appears as `analyzer.finding` (with
the tool and its own rule id in `origin`), a secret scanner's as
`secrets.committed` or `secrets.in-history`, and coverage gaps as
`coverage.uncovered`. See [rules.md](rules.md#analyzer-reports).

Try `sloptopop collect --dry-run` first: it prints every image (with its
version and download size) and every command line, without fetching or
running anything.

## Supported analyzers

### Every repository

| Tool | Finds | Licence |
|---|---|---|
| gitleaks | secrets in the working tree (`gitleaks`) and in the whole git history (`gitleaks-history`) | MIT |
| OSV-Scanner | dependencies with known vulnerabilities, matched offline against a daily snapshot of the OSV data | Apache-2.0 |
| Opengrep | code patterns, with sloptopop's own rule pack | LGPL-2.1-only |
| Semgrep (only with `--only semgrep`) | the same rule pack | LGPL-2.1-only |
| OpenSSF Scorecard, its License check | a repository without a licence file | Apache-2.0 |
| jscpd | duplicated code | MIT |

Opengrep and Semgrep always run with sloptopop's own rules
([rules/](../rules/README.md), MIT OR Apache-2.0) plus your
`.sloptopop/rules/` if it exists. They never fetch registry rules.

### By language

| Ecosystem | Tool | Licence |
|---|---|---|
| JavaScript, TypeScript | oxlint | MIT |
| JavaScript, TypeScript | Biome | MIT OR Apache-2.0 |
| JavaScript, TypeScript | eslint-plugin-sonarjs 2.0.4 | LGPL-3.0-only |
| JavaScript, TypeScript | ESLint and typescript-eslint (`ban-ts-comment`, `no-empty`, `no-empty-function`, and ESLint's defect rules that need no types; see [collect.md](collect.md#javascript-and-typescript)) | MIT |
| JavaScript, TypeScript | dependency-cruiser (`no-circular`) | MIT |
| Python | Ruff (its defaults plus `S501`, `N999` and its defect rules; see [collect.md](collect.md#python-ruff)) | MIT |
| Python | Bandit | Apache-2.0 |
| Python | Pylint (`cyclic-import`) | GPL-2.0-or-later |
| Go | staticcheck | MIT |
| Go | golangci-lint (your configuration; and a second run with only `govet` nilness and the defect linters; see [collect.md](collect.md#go-golangci-lint)) | GPL-3.0-only |
| Go | gosec | Apache-2.0 |
| Go | govulncheck | BSD-3-Clause |
| Java | PMD (`EmptyCatchBlock` and its defect rules; see [collect.md](collect.md#java-pmd)) | BSD-3-Clause |
| Kotlin | detekt (`EmptyCatchBlock` and the `potential-bugs` and `exceptions` defect rules; see [collect.md](collect.md#kotlin-detekt)) | Apache-2.0 |
| PHP | PHP_CodeSniffer (`Squiz.Classes.ClassFileName`, `Generic.CodeAnalysis.EmptyStatement`) | BSD-3-Clause |
| C, C++ | clang-tidy (Clang Static Analyzer core, C++, dead-code and Unix checks; bugprone checks; see [collect.md](collect.md#c-and-c-defect-rules)) | Apache-2.0 WITH LLVM-exception |
| C, C++ | Cppcheck (null, leak, use-after-free, bounds, overflow, division-by-zero, uninitialised and condition defects; see [collect.md](collect.md#c-and-c-defect-rules)) | GPL-3.0-or-later |
| Swift | SwiftLint (your configuration; and a second run with only its defect rules; see [collect.md](collect.md#swift-swiftlint)) | MIT |
| C# (.NET) | SonarAnalyzer.CSharp 9.32.0.97167 and the .NET SDK analyzers, in your project's own build (see [collect.md](collect.md#c-sonaranalyzer-and-the-net-analyzers)) | LGPL-3.0-only |

eslint-plugin-sonarjs and SonarAnalyzer.CSharp are used at their **last
releases under LGPL-3.0-only**: every later release is under a
source-available licence that is not open source. Do not upgrade them.

Most of these tools also know how to fix some of what they report. Their
fixes are kept with their findings, unchanged, in byte offsets; nothing
on your machine applies them (see [collect.md](collect.md#fixes)). On
sloptopop's public corpus the pinned tools offered 893 fixes they consider
safe: Biome 710, Ruff 168, eslint-plugin-sonarjs 15. oxlint's fixes are not
kept: none of its report formats carries them.

The Go analyzers and the C# build need your project's dependencies: one
`go mod download` or `dotnet restore` runs first, with network, into a
cache volume, and the analysis then runs offline (see
[collect.md](collect.md#prepare-steps)).

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

## Where the images come from

The images are built by the open-source
[`sloptopop/analyzer-images`](https://github.com/sloptopop/analyzer-images)
repository from each tool's upstream release, reproducibly, for
linux/amd64 and linux/arm64, and served by sloptopop's component library
(`components.sloptopop.dev`, or your self-hosted engine). A company mirror
or proxy works with `--registry` or `SLOPTOPOP_REGISTRY`, and machines
without network work with a release archive and `--offline`. See
[collect.md](collect.md#mirrors-proxies-and-air-gapped-machines).

## Without a container runtime

`collect` stops with exit code 3 and says what it tried and what to
install. It never falls back to the tools on your `PATH`. Where no
container runtime can run, `--host-tools` runs your own installed tools
instead: unpinned, unsupported, with a warning on every run (see
[collect.md](collect.md#--host-tools-unsupported)).

## Licences

Every analyzer above is under an OSI-approved licence. sloptopop only
starts them as separate programs in their own containers and reads the
files they write. Nothing is linked into `sloptopop`, so the copyleft
licences (LGPL, GPL) put no obligation on your code or on sloptopop's. Each
image carries the licence texts and notices of everything inside it. Each
licence was read from the tool's own licence file at a release tag; the
sources are listed in [collect.md](collect.md#licences).

## Network use

`collect` sends nothing about your code anywhere. It contacts the
component library (or your mirror) to fetch images and the daily
vulnerability snapshots, each announced before it is fetched, and, for Go
and .NET projects, your project's own module or package registries in the
prepare step. No analyzer has network.

## Your own reports

You can also put reports there yourself. Any `*.sarif` or `*.sarif.json`
file (SARIF 2.1.0) and any `*.lcov`, `lcov.info` or `*.lcov.info` file in
`.sloptopop/reports/` is read by the next `check`. A report that cannot be
read is reported as `analyzer.unreadable-report`, never silently ignored.
