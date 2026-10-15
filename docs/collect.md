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
   downloaded.
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
   paths, so they do not belong in version control.
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

`<rules>` is each rule directory in turn (see step 3 above). The version
recorded for each tool is the first line of `<command> --version`.

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
downloaded on your behalf. The copyleft tools (Opengrep and Semgrep under
LGPL-2.1, golangci-lint under GPL-3.0) are therefore used the way their
licences allow any program to be used. Their output carries no licence
obligation for the code that reads it. The `sloptopop` binary stays MIT OR
Apache-2.0, and so does the rule pack built into it: every rule was written
for sloptopop and none comes from the Semgrep registry.

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
      "name": "gitleaks",
      "output": ".sloptopop/reports/gitleaks.sarif",
      "status": "ran",
      "version": "8.28.0"
    },
    {
      "argv": ["bandit", "-r", ".", "-f", "sarif", "-o", ".sloptopop/reports/bandit.sarif"],
      "licence": "Apache-2.0",
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
| `tools[].version` | First line of `--version`; absent if it printed nothing or failed |
| `tools[].argv` | The command line as run (under `--container`, the `docker` command line) |
| `tools[].output` | The report path relative to the root |
| `tools[].status` | `ran`, `failed`, `missing` or `skipped` |
| `tools[].exitCode` | The tool's exit code; absent if it did not run or was ended by a signal |
| `tools[].reason` | Why it failed (first stderr line), is missing or was skipped |
