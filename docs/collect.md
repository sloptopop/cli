# `sloptopop collect`

`sloptopop collect` runs the analyzers you already have installed, each
with fixed flags, and writes their reports to `.sloptopop/reports/`. The
engine never runs a tool itself: `sloptopop check` reads the SARIF and LCOV
files it finds there (see the `analysis` section of the
[Report format](report-format.md)).

```
sloptopop collect [root] [--container] [--only <tool,...>] [--rules <dir>] [--dry-run]
sloptopop check [root]
```

| Option | Meaning |
|---|---|
| `root` | The repository root (default: the current directory) |
| `--only <tool,...>` | Only these catalogue entries, by the names below; repeatable. It narrows the applicable entries and never adds one whose ecosystem was not detected. An unknown name is a usage error. |
| `--rules <dir>` | Rules for Opengrep and Semgrep, used instead of the defaults (default: sloptopop's own [rule pack](../rules/README.md), plus `.sloptopop/rules/` if it exists) |
| `--container` | Run each tool from a digest-pinned container image instead of `PATH` (see below) |
| `--dry-run` | Print the plan (tools, command lines, outputs) and run nothing; nothing is written |

## What it does

1. Detects the ecosystems from the manifests in the root directory (not
   subdirectories), the same signals the engine uses:

   | File in the root | Ecosystem |
   |---|---|
   | `Cargo.toml` | rust |
   | `package.json` | node |
   | `pyproject.toml`, `requirements*.txt` | python |
   | `go.mod` | go |
   | `*.csproj` | dotnet |
   | `pom.xml`, `build.gradle`, `build.gradle.kts` | jvm |
   | `composer.json` | php |
   | `Package.swift` | swift |
   | `CMakeLists.txt` | cpp |
   | `*.psd1` | powershell |
   | `*.sh` | shell |

   Cross-cutting entries (secrets, dependencies, SAST) always apply.
2. For every entry that applies, looks the executable up on `PATH`. A tool
   that is not there is reported as `missing`. Nothing is ever installed or
   downloaded. The [managed tools](#managed-tools-sloptopoptools) also
   need their pinned packages in `.sloptopop/tools/`. `collect` writes
   their manifests there and reports them as `missing`, with the one
   command that installs them, until you have run it.
3. Unless `--rules` is given, writes sloptopop's own rule pack (built
   into the binary from [`rules/`](../rules/README.md)) to
   `.sloptopop/reports/.rules/`, replacing what an earlier run left
   there. Opengrep and Semgrep get that directory and then
   `.sloptopop/rules/` if it exists; with `--rules <dir>` they get only
   `<dir>`.
4. Runs each installed tool with the repository root as its working
   directory and its report going to `.sloptopop/reports/<output>`: either
   through the tool's own output flag or by writing its stdout to the file.
   The exit code is recorded. Exit 0 and the tool's "findings found" codes
   count as `ran`. Any other code, a signal, or a tool that cannot start
   counts as `failed`, with the first line of its stderr. A failure never
   stops the other tools.
5. Writes `.sloptopop/reports/.collect.json` (see below).
6. Makes sure git ignores the reports. Unless the root `.gitignore`
   already ignores `.sloptopop/` (or `.sloptopop/reports/`), it adds a `*`
   line to `.sloptopop/reports/.gitignore`. Reports hold secret locations
   (gitleaks redacts the secrets themselves), dependency lists and machine
   paths, so they do not belong in version control. The managed tools
   directory, `.sloptopop/tools/`, is kept out of git the same way.
7. Prints one line per tool:

   ```
   sloptopop collect: 4 ran, 0 failed, 1 missing, 2 skipped (ecosystems: python)
     ran      gitleaks         exit 0  .sloptopop/reports/gitleaks.sarif
     skipped  gitleaks-history not a git repository
     ran      osv-scanner      exit 1  .sloptopop/reports/osv-scanner.sarif
     missing  opengrep         not found on PATH
     ran      semgrep          exit 0  .sloptopop/reports/semgrep.sarif
     ...
   ```

`collect` exits 0 whatever the tools did. It exits 2 for invalid arguments
(an unknown tool, a root or rules directory that does not exist) and 1 only
if it cannot write the reports directory, the rule pack, `.collect.json`
or its output.

## The catalogue

The catalogue is a table in the source (`src/collect_catalogue.rs`); the
command lines below are exactly what runs. `<out>` is
`.sloptopop/reports/<output>`, and "stdout" means the tool prints the report
and `collect` writes it to the file.

| Name | Licence | Applies to | Command (fixed flags) | Output | Findings exit codes |
|---|---|---|---|---|---|
| `gitleaks` | MIT | all | `gitleaks dir . --redact --report-format sarif --report-path <out> --exit-code 0` | `gitleaks.sarif` | (none needed) |
| `gitleaks-history` | MIT | all git repositories | `gitleaks git . --redact --report-format sarif --report-path <out> --exit-code 0` | `gitleaks.history.sarif` | (none needed) |
| `osv-scanner` | Apache-2.0 | all | `osv-scanner scan source -r --offline-vulnerabilities --download-offline-databases --format sarif .` (stdout) | `osv-scanner.sarif` | 1, 128 |
| `opengrep` | LGPL-2.1-only | all | `opengrep scan --sarif --no-rewrite-rule-ids --config <rules> [--config <rules>] .` (stdout) | `opengrep.sarif` | |
| `semgrep` | LGPL-2.1-only | all | `semgrep scan --sarif --metrics off --disable-version-check --no-rewrite-rule-ids --config <rules> [--config <rules>] .` (stdout) | `semgrep.sarif` | |
| `bandit` | Apache-2.0 | python | `bandit -r . -f sarif -o <out>` | `bandit.sarif` | 1 |
| `gosec` | Apache-2.0 | go | `gosec -fmt sarif -out <out> ./...` | `gosec.sarif` | 1 |
| `govulncheck` | BSD-3-Clause | go | `govulncheck -format sarif ./...` (stdout) | `govulncheck.sarif` | 3 |
| `ruff` | MIT | python | `ruff check --output-format sarif --output-file <out> --no-cache .` | `ruff.sarif` | 1 |
| `oxlint` | MIT | node | `oxlint --format sarif .` (stdout) | `oxlint.sarif` | 1 |
| `biome` | MIT OR Apache-2.0 | node | `biome lint --reporter=sarif .` (stdout) | `biome.sarif` | 1 |
| `staticcheck` | MIT | go | `staticcheck -f sarif ./...` (stdout) | `staticcheck.sarif` | 1 |
| `golangci-lint` | GPL-3.0-only | go | `golangci-lint run --max-issues-per-linter=0 --max-same-issues=0 --uniq-by-line=false --output.sarif.path=<out> ./...` | `golangci.sarif` | 1 |
| `swiftlint` | MIT | swift | `swiftlint lint --quiet --reporter sarif` (stdout) | `swiftlint.sarif` | 2 |
| `eslint-sonarjs` | LGPL-3.0-only | node | `node <eslint> --config <tools>/eslint-sonarjs/eslint.config.mjs --no-inline-config --no-error-on-unmatched-pattern --format <tools>/eslint-sonarjs/node_modules/@microsoft/eslint-formatter-sarif/sarif.js --output-file <out> .` | `eslint-sonarjs.sarif` | 1 |
| `eslint-sonarjs-json` | LGPL-3.0-only | node | the same with `--format json-with-metadata` | `eslint-sonarjs.json` | 1 |
| `sonaranalyzer-csharp` | LGPL-3.0-only | dotnet | `dotnet build --no-restore --no-incremental -p:TreatWarningsAsErrors=false -p:DirectoryBuildPropsPath=<tools>/sonaranalyzer-csharp/Directory.Build.props` | `sonaranalyzer-csharp.<project>[.<framework>].sarif` | |

`<rules>` is each rule directory in turn (see step 3 above). `<tools>` is
the absolute path of `.sloptopop/tools`, and `<eslint>` is ESLint's
command-line script (see [Managed tools](#managed-tools-sloptopoptools)).
The version recorded for each tool is the first line of
`<command> --version`; for the ESLint entries it is ESLint's
(`node <eslint> --version`), and for `sonaranalyzer-csharp` the .NET SDK's.

### Coverage producers (documented, never run)

Coverage needs the project's own test run, which `collect` cannot know, so
these entries are listed as `skipped` with the command to run yourself after
(or as part of) your tests. The engine reads the LCOV files they write.

| Name | Licence | Applies to | Command | Output |
|---|---|---|---|---|
| `cargo-llvm-cov` | Apache-2.0 OR MIT | rust | `cargo llvm-cov --lcov --output-path .sloptopop/reports/cargo.lcov` | `cargo.lcov` |
| `coverage.py` | Apache-2.0 | python | `coverage lcov -o .sloptopop/reports/python.lcov` (after `coverage run -m pytest`) | `python.lcov` |
| `c8` | ISC | node | `c8 --reporter=lcov --reports-dir=.sloptopop/reports npm test` (Vitest and Jest: their `lcov` reporter, same directory) | `lcov.info` |
| `coverlet` | MIT | dotnet | `dotnet test /p:CollectCoverage=true /p:CoverletOutputFormat=lcov /p:CoverletOutput=.sloptopop/reports/dotnet.lcov` | `dotnet.lcov` |
| `gcovr` | BSD-3-Clause | cpp | `gcovr --lcov .sloptopop/reports/cpp.lcov` (after building with coverage and running the tests) | `cpp.lcov` |

## Managed tools (`.sloptopop/tools/`)

Two analyzers from SonarSource are used at their last releases under
LGPL-3.0-only. Every later release is under the SONAR Source-Available
License, which is not an open-source licence. Neither analyzer is a
standalone executable, so `collect` cannot simply find it on `PATH`, and
neither is ever installed into your project's own dependencies. Each gets
a directory of its own, `.sloptopop/tools/<tool>/`. On every run (not
`--dry-run`) where the tool applies, `collect` writes its manifest files
there. They are built into the binary from [`tools/`](../tools/). Then you
install the pinned packages once, from the repository root:

| Directory | Install command | Pinned by |
|---|---|---|
| `eslint-sonarjs` | `npm ci --ignore-scripts --prefix .sloptopop/tools/eslint-sonarjs` | `package-lock.json`: every one of its 460 packages by exact version and SHA-512 integrity, all from registry.npmjs.org, none with an install script |
| `sonaranalyzer-csharp` | `dotnet restore .sloptopop/tools/sonaranalyzer-csharp/fetch.proj` | the exact version; NuGet checks the package signature, and `collect` compares the SHA-512 NuGet records with the pinned one |

Until the pinned versions are there, the entries are `missing` with that
command as the reason. `collect` itself still downloads nothing, and under
`--container` these entries are skipped (no image).

| Package | Version | Integrity (SHA-512 of the archive) | Licence | Licence file read |
|---|---|---|---|---|
| eslint-plugin-sonarjs | 2.0.4 | `sha512-XVVAB/t0WSgHitHNajIcIDmviCO8kB9VSsrjy+4WUEVM3eieY9SDHEtCDaOMTjj6XMtcAr8BFDXCFaP005s+tg==` | LGPL-3.0-only | `package/LICENSE` in [the archive](https://registry.npmjs.org/eslint-plugin-sonarjs/-/eslint-plugin-sonarjs-2.0.4.tgz) |
| eslint | 9.14.0 | `sha512-c2FHsVBr87lnUtjP4Yhvk4yEhKrQavGafRA/Se1ouse8PfbfC/Qh9Mxa00yWsZRlqeUB9raXip0aiiUZkgnr9g==` | MIT | `package/LICENSE` in [the archive](https://registry.npmjs.org/eslint/-/eslint-9.14.0.tgz) |
| @microsoft/eslint-formatter-sarif | 3.1.0 | `sha512-/mn4UXziHzGXnKCg+r8HGgPy+w4RzpgdoqFuqaKOqUVBT5x2CygGefIrO4SusaY7t0C4gyIWMNu6YQT6Jw64Cw==` | MIT | the archive has none: [LICENSE](https://github.com/microsoft/sarif-js-sdk/blob/0ff663e507e989ae751e2e9d94a3360291e3e4e5/LICENSE) at the commit whose `package.json` is 3.1.0 and whose `sarif.js` is the archive's, byte for byte |
| @typescript-eslint/parser | 7.16.1 | `sha512-u+1Qx86jfGQ5i4JjK33/FnawZRpsLxRnKzGE6EABZ40KxVT/vWsiZFEBBHjFOljmmV3MBYOHEKi0Jm9hbAOClA==` | BSD-2-Clause | `package/LICENSE` in [the archive](https://registry.npmjs.org/@typescript-eslint/parser/-/parser-7.16.1.tgz) |
| typescript | 5.5.4 | `sha512-Mtq29sKDAEYP7aljRgtPOpTvOfbwRWlS6dPRzwjdE+C0R4brX/GUyhHSecbHMFLNBLcJIPt9nl9yG5TZ1weH+Q==` | Apache-2.0 | `package/LICENSE.txt` in [the archive](https://registry.npmjs.org/typescript/-/typescript-5.5.4.tgz) |
| SonarAnalyzer.CSharp | 9.32.0.97167 | `sha512-NK5VSMCd8IXdKPSZruJY+Lm7aICuIitnAe5Wrmmgc+7x8ue78ee3pdjYib39t/gFBGTy8A5RiocJiNhEatUvWg==` | LGPL-3.0-only | the package has no licence file, only the nuspec's `LGPL-3.0-only` expression; the text is [LICENSE.txt](https://github.com/SonarSource/sonar-dotnet/blob/9.32.0.97167/LICENSE.txt) at the release tag |

Do not upgrade these by registry metadata. npm lists eslint-plugin-sonarjs
as `LGPL-3.0-only` for every version up to 4.x, but the LICENSE file
inside the package is the source-available licence from 3.0.0 onwards.

### `eslint-sonarjs` and `eslint-sonarjs-json`

- **What runs.** ESLint with the plugin's `recommended` configuration on
  `**/*.{js,mjs,cjs,jsx,ts,tsx,mts,cts}`, through sloptopop's own
  [`eslint.config.mjs`](../tools/eslint-sonarjs/eslint.config.mjs), passed
  with `--config` so the project's own configuration is not used. The
  configuration's imports resolve from the tool directory, never from the
  project's `node_modules`. TypeScript is parsed by
  `@typescript-eslint/parser` without type information, so nothing of the
  project is built or run, and the plugin's type-aware rules turn
  themselves off. `.sloptopop/`, `.next/`, `dist/`, `coverage/` and
  `*.min.js` are not linted.
- **Which ESLint.** The project's own ESLint (`node_modules/eslint`) if its
  version is 9.0 to 9.14, otherwise the pinned 9.14.0 in the tool
  directory. From ESLint 9.15 on, core rules read their defaults from
  `meta.defaultOptions`. The plugin's typescript-eslint 7 wrappers do not
  pass those defaults on, so linting stops with a crash. `node` itself
  must be on `PATH`.
- **Why `--no-inline-config`.** The project's `eslint-disable` comments
  name rules of plugins this run does not load, and ESLint would report
  each of them as an error ("Definition for rule … was not found").
  Ignoring inline configuration keeps the reports to the plugin's own
  rules. The cost is that a project's comments suppressing a sonarjs rule
  are ignored too.
- **Two runs, two reports.** ESLint writes one format per run.
  `eslint-sonarjs.sarif` (SARIF 2.1.0, through the formatter) is what
  `sloptopop check` reads. `eslint-sonarjs.json` is ESLint's
  `json-with-metadata` output. It keeps each finding's `fix` (a byte range
  and its replacement text) and `suggestions`, plus the rules' metadata,
  for tools that apply fixes. The engine lists it as an unrecognised input
  and reads nothing from it. `--no-error-on-unmatched-pattern` makes a
  project without matching files a plain empty run.

### `sonaranalyzer-csharp`

- **What runs.** The project's own `dotnet build`, with the analyzer
  added as Roslyn analyzers by a generated
  [`Directory.Build.props`](../tools/sonaranalyzer-csharp/Directory.Build.props)
  in the tool directory, passed with `-p:DirectoryBuildPropsPath=`. That
  property replaces the repository's own `Directory.Build.props`, so the
  generated file imports the repository's file first, if there is one
  (found the same way MSBuild finds it). No project file is changed, and
  the analyzer never enters the project's restore or lock files.
- **Reports.** Each project's compiler writes its SARIF v2.1 log
  (`ErrorLog` with `version=2.1`) to
  `.sloptopop/reports/sonaranalyzer-csharp.<project>.sarif`. A project
  built for several target frameworks writes
  `sonaranalyzer-csharp.<project>.<framework>.sarif`. The logs hold every
  compiler diagnostic, the analyzer's rules (`S…`) among them. Before
  each run, `collect` removes the logs of earlier runs, so a project that
  is gone leaves no stale report.
- **Flags.** `--no-restore` keeps `collect` offline, so restore the
  project first, as you would for any build. `--no-incremental` compiles
  every file, so every file is analysed. `-p:TreatWarningsAsErrors=false`
  keeps the analyzer's warnings from failing the build, so exit 1 means a
  real build failure. The build runs the project's own MSBuild logic, like
  any build of it. That is why this entry runs only after you have
  installed the analyzer.

## Why these flags

- **Deterministic and complete.** Every flag is fixed in the catalogue, so
  the same repository and tool versions give the same reports. Issue caps
  are turned off (golangci-lint `--max-issues-per-linter=0
  --max-same-issues=0 --uniq-by-line=false`), so a report is never
  silently truncated. gitleaks runs in both `dir` mode (the working tree)
  and `git` mode (the whole history), because a secret removed from the
  tree is still in the history until it is rotated.
- **Offline.** OSV-Scanner matches dependencies against a local copy of
  the OSV database (`--offline-vulnerabilities`), so the dependency list
  never leaves the machine. `--download-offline-databases` fetches that
  database; it is the only download any entry makes. Semgrep runs with
  `--metrics off --disable-version-check`, so it sends no usage data and
  never asks for the latest version; Opengrep does neither in the first
  place and has no such flags. Ruff runs with `--no-cache`, so
  it writes nothing into the repository. govulncheck is the exception: it
  looks up the module paths it finds in the Go vulnerability database
  (vuln.go.dev) and needs network to do so.
- **Own rules, never registry rules.** Opengrep and Semgrep always get an
  explicit rules directory: sloptopop's own rule pack (MIT OR Apache-2.0,
  written for sloptopop; see [`rules/README.md`](../rules/README.md)) plus
  `.sloptopop/rules/` if you have one, or only the directory you pass with
  `--rules`. They never run without `--config`, which would fetch rules
  from the registry. The Semgrep registry rules are not under an OSI
  licence and are never used. `--no-rewrite-rule-ids` keeps each rule id
  as written (`sloptopop.errors.empty-catch`) instead of prefixing it with
  the path of the rules directory, so the ids in the reports are stable.
- **Secrets stay redacted.** gitleaks always runs with `--redact`, and the
  reports directory is ignored by git.
- **"Findings" is not failure.** Each tool's "findings found" exit codes are
  listed, so a tool that found problems counts as `ran` and only genuine
  failures are reported as `failed`. gitleaks gets `--exit-code 0` for the
  same reason.

## Licences

Every tool in the catalogue is under an OSI-approved licence. `collect`
only starts them as separate processes and reads the files they write:
nothing is linked into `sloptopop` or bundled with it, and nothing is
downloaded on your behalf. The copyleft tools are used the way their
licences allow any program to be used: Opengrep and Semgrep under
LGPL-2.1, golangci-lint under GPL-3.0, and the two Sonar analyzers under
LGPL-3.0, which ESLint and the C# compiler load in their own processes.
Their output carries no licence obligation for the code that reads it. The
`sloptopop` binary stays MIT OR Apache-2.0. So do the rule pack and the
`tools/` files built into it: every rule was written for sloptopop, and
none comes from the Semgrep registry.

Every licence above was read from the tool's licence file, at a release
tag or inside the released archive, never only from registry metadata.
The catalogue records the files it was read from (`licence_files`, also in
`.collect.json`), and its tests refuse an entry without them.

| Entry | Licence | Licence file read |
|---|---|---|
| `gitleaks`, `gitleaks-history` | MIT | [gitleaks v8.30.1 LICENSE](https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE) |
| `osv-scanner` | Apache-2.0 | [osv-scanner v2.7.0 LICENSE](https://github.com/google/osv-scanner/blob/v2.7.0/LICENSE) |
| `opengrep` | LGPL-2.1-only | [opengrep v1.31.0 LICENSE](https://github.com/opengrep/opengrep/blob/v1.31.0/LICENSE) |
| `semgrep` | LGPL-2.1-only | [semgrep v1.181.0 LICENSE](https://github.com/semgrep/semgrep/blob/v1.181.0/LICENSE) |
| `bandit` | Apache-2.0 | [bandit 1.9.4 LICENSE](https://github.com/PyCQA/bandit/blob/1.9.4/LICENSE) |
| `gosec` | Apache-2.0 | [gosec v2.29.0 LICENSE.txt](https://github.com/securego/gosec/blob/v2.29.0/LICENSE.txt) |
| `govulncheck` | BSD-3-Clause | [golang/vuln v1.1.4 LICENSE](https://github.com/golang/vuln/blob/v1.1.4/LICENSE) |
| `ruff` | MIT | [ruff 0.17.1 LICENSE](https://github.com/astral-sh/ruff/blob/0.17.1/LICENSE) |
| `oxlint` | MIT | [oxlint v1.88.0 LICENSE](https://github.com/oxc-project/oxc/blob/oxlint_v1.88.0/LICENSE) |
| `biome` | MIT OR Apache-2.0 | [LICENSE-MIT](https://github.com/biomejs/biome/blob/@biomejs/biome@2.5.15/LICENSE-MIT) and [LICENSE-APACHE](https://github.com/biomejs/biome/blob/@biomejs/biome@2.5.15/LICENSE-APACHE) at Biome 2.5.15 |
| `staticcheck` | MIT | [go-tools 2026.2.2 LICENSE](https://github.com/dominikh/go-tools/blob/2026.2.2/LICENSE) |
| `golangci-lint` | GPL-3.0-only | [golangci-lint v2.14.0 LICENSE](https://github.com/golangci/golangci-lint/blob/v2.14.0/LICENSE) |
| `swiftlint` | MIT | [SwiftLint 0.65.1 LICENSE](https://github.com/realm/SwiftLint/blob/0.65.1/LICENSE) |
| `eslint-sonarjs`, `eslint-sonarjs-json` | LGPL-3.0-only | `package/LICENSE` in [eslint-plugin-sonarjs-2.0.4.tgz](https://registry.npmjs.org/eslint-plugin-sonarjs/-/eslint-plugin-sonarjs-2.0.4.tgz) (the other pinned packages: see [Managed tools](#managed-tools-sloptopoptools)) |
| `sonaranalyzer-csharp` | LGPL-3.0-only | [sonar-dotnet 9.32.0.97167 LICENSE.txt](https://github.com/SonarSource/sonar-dotnet/blob/9.32.0.97167/LICENSE.txt), and the nuspec in [the package](https://api.nuget.org/v3-flatcontainer/sonaranalyzer.csharp/9.32.0.97167/sonaranalyzer.csharp.9.32.0.97167.nupkg) |
| `cargo-llvm-cov` | Apache-2.0 OR MIT | [LICENSE-APACHE](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/LICENSE-APACHE) and [LICENSE-MIT](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/LICENSE-MIT) at v0.9.1 |
| `coverage.py` | Apache-2.0 | [coverage.py 7.16.2 LICENSE.txt](https://github.com/nedbat/coveragepy/blob/7.16.2/LICENSE.txt) |
| `c8` | ISC | [c8 v12.0.0 LICENSE.txt](https://github.com/bcoe/c8/blob/v12.0.0/LICENSE.txt) |
| `coverlet` | MIT | [coverlet v10.1.0 LICENSE](https://github.com/coverlet-coverage/coverlet/blob/v10.1.0/LICENSE) |
| `gcovr` | BSD-3-Clause | [gcovr 8.6 LICENSE.txt](https://github.com/gcovr/gcovr/blob/8.6/LICENSE.txt) |

## `--container`

With `--container`, each tool runs from a container image instead of
`PATH`:

```
docker run --rm --network none --user <uid>:<gid> --cap-drop ALL \
  --security-opt no-new-privileges --env HOME=/tmp \
  --mount type=bind,source=<root>,target=/src,readonly \
  --mount type=bind,source=<root>/.sloptopop/reports,target=/src/.sloptopop/reports \
  [--mount type=bind,source=<rules>,target=/rules/0,readonly \
   [--mount type=bind,source=<rules>,target=/rules/1,readonly]] \
  --workdir /src --entrypoint <command> <image>@sha256:<digest> <arguments>
```

- The root is mounted read-only. Only the reports directory is writable.
  For Opengrep and Semgrep, the rule directories are mounted read-only at
  `/rules/0` (and `/rules/1`) and passed as `--config /rules/0`. The tool runs
  as the owner of the root directory, so the reports belong to you.
- No network (`--network none`). The one exception is OSV-Scanner, which
  gets the default bridge network for its database download, its own
  documented step; the scan itself stays offline.
- No ports are ever published.
- Images come from the catalogue as `image@sha256:<digest>`. An entry
  without a pinned, well-formed SHA-256 digest is skipped ("container
  image digest not pinned"), and so is an entry without an image. A tag is
  never trusted on its own. The digests in the catalogue are marked
  `TODO(owner)` until they are pinned. The image must have the command on
  its `PATH`, because it is passed as `--entrypoint`.
- `docker` itself is looked up on `PATH`; if it is not there, every
  pinned entry is reported as `missing`.
- The version is read with
  `docker run --rm --network none --entrypoint <command> <image> --version`.

## `.collect.json`

`.sloptopop/reports/.collect.json` records the run. The leading dot hides
it from the engine, which reads only visible files as reports. It is
deterministic: tools in catalogue order, no timestamps, no durations.

```json
{
  "ecosystems": ["python"],
  "mode": "host",
  "schemaVersion": 1,
  "tools": [
    {
      "argv": ["gitleaks", "dir", ".", "--redact", "--report-format", "sarif",
               "--report-path", ".sloptopop/reports/gitleaks.sarif", "--exit-code", "0"],
      "exitCode": 0,
      "licence": "MIT",
      "licenceFiles": ["https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE"],
      "name": "gitleaks",
      "output": ".sloptopop/reports/gitleaks.sarif",
      "status": "ran",
      "version": "8.28.0"
    },
    {
      "argv": ["bandit", "-r", ".", "-f", "sarif", "-o", ".sloptopop/reports/bandit.sarif"],
      "licence": "Apache-2.0",
      "licenceFiles": ["https://github.com/PyCQA/bandit/blob/1.9.4/LICENSE"],
      "name": "bandit",
      "output": ".sloptopop/reports/bandit.sarif",
      "reason": "not found on PATH",
      "status": "missing"
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `schemaVersion` | `1` |
| `mode` | `host` or `container` |
| `ecosystems` | The detected ecosystems, sorted |
| `tools[].name`, `licence` | The catalogue entry |
| `tools[].licenceFiles` | The licence files the licence was read from |
| `tools[].version` | First line of `--version`; absent if it printed nothing or failed |
| `tools[].argv` | The command line as run (under `--container`, the `docker` command line) |
| `tools[].output` | The report path relative to the root (for `sonaranalyzer-csharp`, a pattern with `*`) |
| `tools[].status` | `ran`, `failed`, `missing` or `skipped` |
| `tools[].exitCode` | The tool's exit code; absent if it did not run or was ended by a signal |
| `tools[].reason` | Why it failed (first stderr line), is missing or was skipped |
