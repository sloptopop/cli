# `sloptopop collect`

`sloptopop collect` runs the open-source analyzers that apply to your
repository, each from a container image that sloptopop built and pinned
by its SHA-256 digest, with fixed flags, and writes their reports to
`.sloptopop/reports/`. The engine never runs a tool itself: `sloptopop
check` reads the SARIF and LCOV files it finds there (see the `analysis`
section of the [Report format](report-format.md)).

The versions are the ones this release of sloptopop was tested with,
whatever is installed on your machine. They change only when you update
sloptopop.

```
sloptopop collect [root] [--only <tool,...>] [--rules <dir>] [--dry-run] [--offline]
                  [--registry <host[/path]>] [--runtime docker|podman]
                  [--timeout <seconds>] [--vuln-db <YYYYMMDD>] [--host-tools]
sloptopop check [root]
```

| Option | Meaning |
|---|---|
| `root` | The repository root (default: the current directory) |
| `--only <tool,...>` | Only these catalogue entries, by the names below; repeatable. It narrows the applicable entries and never adds one whose ecosystem was not detected. It is also how an entry that is off by default (`semgrep`) runs. An unknown name is a usage error. |
| `--rules <dir>` | Rules for Opengrep and Semgrep, used instead of the defaults (default: sloptopop's own [rule pack](../rules/README.md), plus `.sloptopop/rules/` if it exists) |
| `--dry-run` | Print the plan (each image with its version and size, each command line, each output) and fetch and run nothing; nothing is written, and no container runtime is needed |
| `--offline` | Fetch nothing: use only images and vulnerability snapshots already on this machine |
| `--registry <host[/path]>` | Pull from this registry (a mirror or proxy of the component library) instead of the default; also `SLOPTOPOP_REGISTRY`. The images are the same: they are pulled by digest. |
| `--runtime docker\|podman` | Use this container runtime only; also `SLOPTOPOP_CONTAINER_RUNTIME` (default: `docker`, then `podman`) |
| `--timeout <seconds>` | The time limit of each tool (default 1800) |
| `--vuln-db <YYYYMMDD>` | Use the vulnerability snapshot of that date instead of the newest |
| `--host-tools` | **Unsupported.** Run the tools found on your `PATH` instead of the images (see [below](#--host-tools-unsupported)) |
| `--container` | Accepted for compatibility: containers are the default; it only prints a note |

## What it does

1. Detects the ecosystems from the manifests in the root directory (not
   subdirectories). The engine uses the same signals, except `pom.xml`,
   `requirements*.txt` and `*.sh`, which only `collect` uses:

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

   Cross-cutting entries (secrets, dependencies, SAST, licence, duplicate
   code) always apply.
2. Writes sloptopop's own rule pack (built into the binary from
   [`rules/`](../rules/README.md)) to `.sloptopop/reports/.rules/`, unless
   `--rules` is given.
3. Finds the container runtime (see [Container runtimes](#container-runtimes)).
   If none answers, nothing runs: every tool is recorded as `missing`, the
   error says what was tried and what to install, and `collect` exits 3.
   It never falls back to the tools on your `PATH`.
4. For every image the plan needs and that is not on this machine yet, it
   prints one line, then pulls the image **by its digest**:

   ```
   fetch    ruff             0.17.1-1               10.9 MB  components.sloptopop.dev/analyzers/ruff@sha256:9bc0…  MIT
   ```

   It then checks the image (see [Verification](#verification)).
5. For OSV-Scanner and govulncheck it fetches the newest vulnerability
   snapshot of each ecosystem it needs (see
   [Vulnerability data](#vulnerability-data)), again after a printed line.
6. For the Go analyzers and the C# build it runs a **prepare** step first,
   once: `go mod download` or `dotnet restore`, the only containers that
   get a network, so the project's own dependencies are in a cache the
   analysis can read offline (see [Prepare steps](#prepare-steps)).
7. Runs each tool in its own hardened container (see
   [How a tool runs](#how-a-tool-runs)), with the repository mounted
   read-only at `/src` and only `.sloptopop/reports/` writable.
   - A report an earlier run left is removed first, so a tool that fails
     leaves no stale report; a tool that fails leaves no report at all.
   - Exit 0 and the tool's "findings found" codes count as `ran`. Any other
     code, a signal, a tool that cannot start, or one past its time limit
     counts as `failed`, with the first line of its stderr. A failure never
     stops the other tools.
   - Tools that write no SARIF write their own format to
     `.sloptopop/reports/.raw/`, and `collect` converts it (see
     [Conversions](#conversions)).
   - Every SARIF report is then normalized, so the same repository gives
     the same bytes on every machine (see [Determinism](#determinism)).
8. Writes `.sloptopop/reports/.collect.json` (see below) and makes sure git
   ignores the reports: unless the root `.gitignore` already ignores
   `.sloptopop/` (or `.sloptopop/reports/`), it adds a `*` line to
   `.sloptopop/reports/.gitignore`. Reports hold secret locations (gitleaks
   redacts the secrets themselves), dependency lists and machine paths.
9. Prints one line per tool:

   ```
   sloptopop collect: 9 ran, 0 failed, 0 missing, 1 skipped (ecosystems: python)
     ran      gitleaks             exit 0  .sloptopop/reports/gitleaks.sarif
     ran      osv-scanner          exit 1  .sloptopop/reports/osv-scanner.sarif
     ...
     skipped  coverage.py          documented only: run it after your tests
   ```

`collect` exits 0 whatever the tools did. It exits 2 for invalid arguments
or settings (an unknown tool, a root or rules directory that does not
exist, a malformed `SLOPTOPOP_*` value), 3 when no container runtime
answers, and 1 only if it cannot write the reports directory, the rule
pack, `.collect.json` or its output.

## The catalogue

The catalogue is a table in the source (`src/collect_catalogue.rs`); the
command lines below are exactly what runs inside the container, from
`/src`. `<out>` is `.sloptopop/reports/<output>` (or, for a converted tool,
`.sloptopop/reports/.raw/<raw file>`), and "stdout" means the tool prints
its output and `collect` writes it to the file. Every image is
`<registry>/analyzers/<image>@sha256:<digest>`; the digests and sizes are in
the source and in the `--dry-run` plan.

| Name | Image | Licence | Applies to | Command (fixed flags) | Output | Findings exit codes |
|---|---|---|---|---|---|---|
| `gitleaks` | gitleaks 8.30.1 | MIT | all | `gitleaks dir . --redact --report-format sarif --report-path <out> --exit-code 0` | `gitleaks.sarif` | |
| `gitleaks-history` | gitleaks 8.30.1 | MIT | all git repositories | `gitleaks git . --redact --report-format sarif --report-path <out> --exit-code 0` | `gitleaks.history.sarif` | |
| `osv-scanner` | osv-scanner 2.7.0 | Apache-2.0 | all | `osv-scanner scan source -r --offline --format sarif .` (stdout) | `osv-scanner.sarif` | 1, 128 |
| `opengrep` | opengrep 1.31.0 | LGPL-2.1-only | all | `opengrep scan --sarif --no-rewrite-rule-ids --config /rules/0 [--config /rules/1] .` (stdout) | `opengrep.sarif` | |
| `semgrep` (only with `--only`) | semgrep 1.181.0 | LGPL-2.1-only | all | `semgrep scan --sarif --metrics off --disable-version-check --no-rewrite-rule-ids --config /rules/0 [--config /rules/1] .` (stdout) | `semgrep.sarif` | |
| `bandit` | bandit 1.9.4 | Apache-2.0 | python | `bandit -r . -f sarif -o <out>` | `bandit.sarif` | 1 |
| `ruff` | ruff 0.17.1 | MIT | python | `ruff check --output-format sarif --output-file <out> --no-cache --extend-select S110,S501,N999 .` | `ruff.sarif` | 1 |
| `pylint-cyclic-import` | pylint 4.1.3 | GPL-2.0-or-later | python | `pylint --disable=all --enable=cyclic-import --output-format=json2 --recursive=y --exit-zero .` (stdout, converted) | `pylint-cyclic-import.sarif` | |
| `gosec` | gosec 2.29.0 | Apache-2.0 | go | `gosec -fmt sarif -out <out> ./...` | `gosec.sarif` | 1 |
| `govulncheck` | govulncheck 1.1.4 | BSD-3-Clause | go | `govulncheck -db file:///db/govulndb -format sarif ./...` (stdout) | `govulncheck.sarif` | 3 |
| `staticcheck` | staticcheck 2026.2.2 | MIT | go | `staticcheck -f sarif ./...` (stdout) | `staticcheck.sarif` | 1 |
| `golangci-lint` | golangci-lint 2.14.0 | GPL-3.0-only | go | `golangci-lint run --max-issues-per-linter=0 --max-same-issues=0 --uniq-by-line=false --output.sarif.path=<out> ./...` | `golangci.sarif` | 1 |
| `oxlint` | oxlint 1.88.0 | MIT | node | `oxlint --format sarif .` (stdout) | `oxlint.sarif` | 1 |
| `biome` | biome 2.5.15 | MIT OR Apache-2.0 | node | `biome lint --reporter=sarif .` (stdout) | `biome.sarif` | 1 |
| `eslint-sonarjs` | eslint-sonarjs 2.0.4 | LGPL-3.0-only | node | `eslint --config /opt/eslint-sonarjs/eslint.config.mjs --no-inline-config --no-error-on-unmatched-pattern --format /opt/eslint-sonarjs/node_modules/@microsoft/eslint-formatter-sarif/sarif.js --output-file <out> .` | `eslint-sonarjs.sarif` | 1 |
| `eslint-sonarjs-json` | eslint-sonarjs 2.0.4 | LGPL-3.0-only | node | the same with `--format json-with-metadata` | `eslint-sonarjs.json` | 1 |
| `eslint-typescript` | eslint-typescript 8.71.2 | MIT | node | `eslint --config /opt/eslint-typescript/eslint.config.mjs --no-inline-config --no-error-on-unmatched-pattern --format …/sarif.js --output-file <out> .` | `eslint-typescript.sarif` | 1 |
| `dependency-cruiser` | dependency-cruiser 18.6.0 | MIT | node | `depcruise --no-progress --config /opt/dependency-cruiser/depcruise.cjs --output-type json .` (stdout, converted) | `dependency-cruiser.sarif` | |
| `swiftlint` | swiftlint 0.65.1 | MIT | swift | `swiftlint lint --quiet --no-cache --reporter sarif` (stdout) | `swiftlint.sarif` | 2 |
| `sonaranalyzer-csharp` | sonaranalyzer-csharp 9.32.0.97167 | LGPL-3.0-only | dotnet | `dotnet build --no-restore --no-incremental -p:TreatWarningsAsErrors=false -p:DirectoryBuildPropsPath=/opt/sonaranalyzer-csharp/Directory.Build.props` | `sonaranalyzer-csharp.<project>[.<framework>].sarif` | |
| `scorecard-license` | scorecard 5.6.0 | Apache-2.0 | all | `scorecard --local=. --checks=License --format=json` (stdout, converted) | `scorecard-license.sarif` | |
| `jscpd` | jscpd 5.4.1 | MIT | all | `jscpd --reporters sarif --output=.sloptopop/reports --report-name jscpd --min-lines 5 --workers 1 --silent --no-tips --ignore **/.sloptopop/**,**/node_modules/** .` | `jscpd.sarif` | |
| `phpcs` | phpcs 4.0.4 | BSD-3-Clause | php | `phpcs --standard=Squiz,Generic --sniffs=Squiz.Classes.ClassFileName,Generic.CodeAnalysis.EmptyStatement --extensions=php --report=json --no-cache -q .` (stdout, converted) | `phpcs.sarif` | 1, 2, 3 |
| `pmd` | pmd 7.28.0 | BSD-3-Clause | jvm | `pmd check --no-cache --no-progress -d . -R category/java/errorprone.xml/EmptyCatchBlock -f sarif -r <out>` | `pmd.sarif` | 4 |
| `detekt` | detekt 1.23.8 | Apache-2.0 | jvm | `detekt --input . --config /opt/detekt/sloptopop.yml --report sarif:<out>` | `detekt.sarif` | 2 |
| `clang-tidy` | clang-tidy 19.1.0 | Apache-2.0 WITH LLVM-exception | cpp | `sloptopop-clang-tidy '-*,bugprone-empty-catch'` (stdout, converted; the image's wrapper runs clang-tidy over every C++ source in sorted order, with `compile_commands.json` from the root, `build/` or `out/` if there is one, else as C++17) | `clang-tidy.sarif` | |

`<rules>` mounts: the rule directories are mounted read-only at `/rules/0`
(and `/rules/1`). The pack and `.sloptopop/rules/` work as before; with
`--rules <dir>` only that directory is mounted.

The crown-jewel entries (`pylint-cyclic-import`, `eslint-typescript`,
`dependency-cruiser`, `scorecard-license`, `jscpd`, `phpcs`, `pmd`,
`detekt`, `clang-tidy` and Ruff's `S110,S501,N999`) are the open-source
checks that sloptopop's native rules of the same purpose were retired to.

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

## The images

Every image is built by the open-source
[`sloptopop/analyzer-images`](https://github.com/sloptopop/analyzer-images)
repository from the upstream release of the tool, pinned by checksum, for
linux/amd64 and linux/arm64, reproducibly (the same inputs give the same
digest on any build host). On macOS and Windows they run in your runtime's
Linux virtual machine. Each image holds:

- the tool, at the version in its label `org.opencontainers.image.version`
  (the upstream version and sloptopop's build revision, e.g. `0.17.1-1`);
- its runtime where it needs one (the Go toolchain for the Go analyzers,
  CPython 3.13, Node.js 24, a Java 21 runtime, PHP 8.4, the Swift runtime,
  the .NET 10 SDK), from the official image, pinned by digest;
- the licence texts and notices of everything in it, in `/licenses/`, and
  the labels `org.opencontainers.image.licenses` and `io.sloptopop.tool`.

sloptopop's own configuration for a tool (the ESLint and detekt
configurations, the dependency-cruiser rules, the C# `Directory.Build.props`)
is part of its image, so the project's own configuration of the same tool is
never used.

The images come from sloptopop's **component library**, an OCI registry
served next to `sloptopop-engine` (`components.sloptopop.dev`, or your
self-hosted engine), never from the tools' upstream registries. The
catalogue compiled into `sloptopop` is the index of that library: every
image by digest, version, size and licence, signed as part of the release.

## Verification

Before a tool runs from an image, `collect` checks:

1. the catalogue names the image by a SHA-256 digest (every entry does; the
   tests refuse one that does not);
2. the runtime pulled it by that digest: the runtime verifies the manifest
   and every layer against it, so a mirror or a tampered registry can
   withhold an image but never substitute another one;
3. the runtime lists the image under that digest (or, for an image loaded
   from an archive, its ID is that digest);
4. the image's labels `io.sloptopop.tool` and
   `org.opencontainers.image.version` are the catalogue's.

Otherwise the tool is `failed` with the reason and nothing runs from that
image. Signatures (cosign) and build provenance (SLSA) are checked where an
image enters the library and in sloptopop's release checks, not by
`collect`: the digest in the signed release already names exactly the image
that was tested. You can check them yourself:

```
cosign verify \
  --certificate-identity-regexp '^https://github.com/sloptopop/analyzer-images/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  components.sloptopop.dev/analyzers/ruff@sha256:<digest>
```

## How a tool runs

```
<runtime> run --rm --pull never --name sloptopop-collect-<key>-<n>
  --network none --read-only --tmpfs /tmp:rw,exec,nosuid,nodev,size=1g
  --cap-drop ALL --security-opt no-new-privileges
  --user <uid>:<gid>                      (rootless Docker: --user 0:0; rootless Podman: --userns keep-id)
  --pids-limit 1024 --memory 4g --memory-swap 4g [--cpus <n>]
  --hostname sloptopop --env HOME=/tmp --env TZ=UTC --env LANG=C.UTF-8 --env LC_ALL=C.UTF-8
  --mount type=bind,source=<root>,target=/src,readonly
  --mount type=bind,source=<root>/.sloptopop/reports,target=/src/.sloptopop/reports
  [--mount …,target=/rules/0,readonly] [--mount …,target=/db/…,readonly] [--mount type=volume,…]
  --workdir /src --entrypoint <command> <registry>/analyzers/<image>@sha256:<digest> <arguments>
```

- **No network** for any tool. Only the prepare steps get one.
- **Read-only:** the image's filesystem and the repository. The only
  writable places are the reports directory and `/tmp` (a 1 GB in-memory
  filesystem, which some tools unpack themselves into and run from).
- **No privileges:** every capability is dropped, privileges cannot be
  gained (setuid), the runtime's default seccomp and AppArmor or SELinux
  profiles stay on, and the tool runs as you, so the reports belong to you.
- **Limits:** 1024 processes and 4 GB of memory per tool (no swap);
  `SLOPTOPOP_COLLECT_MEMORY` and `SLOPTOPOP_COLLECT_CPUS` change the memory
  and add a CPU cap. A runtime that cannot apply limits (rootless Podman
  without the memory and pids cgroup controllers) runs without them.
- **Time:** each tool is stopped after 30 minutes (`--timeout`), and
  recorded as `failed`.
- Never a published port, a device, a host socket (no `docker.sock`) or the
  host's environment.

## Container runtimes

Any of these works; Docker Desktop is never required:

| Runtime | Licence | Notes |
|---|---|---|
| Docker Engine (Linux), rootful or rootless | Apache-2.0 | rootless is detected (`docker info`), and reports still belong to you |
| Podman (Linux, macOS, Windows), rootful or rootless | Apache-2.0 | on macOS and Windows, start the machine first: `podman machine start` |
| Colima (macOS) | MIT | provides `docker` |
| Rancher Desktop (dockerd mode) | Apache-2.0 | provides `docker` |
| Docker Desktop | proprietary | needs a paid subscription for professional use in larger organisations |

`collect` uses `--runtime` or `SLOPTOPOP_CONTAINER_RUNTIME` if set, otherwise
the first of `docker` and `podman` on your `PATH` whose `info` answers.
Windows support is best effort.

## Prepare steps

- **Go** (`gosec`, `govulncheck`, `staticcheck`, `golangci-lint`): if the
  root has a `go.mod` and no `vendor/modules.txt`, one `go mod download`
  runs first, with network, into the volume `sloptopop-gomodcache` (shared
  by your repositories; `go.sum` and the Go checksum database verify every
  module). The analyzers then read it read-only, with `GOPROXY=off` and
  `GOTOOLCHAIN=local`. Your `GOPROXY`, `GOPRIVATE`, `GONOPROXY`,
  `GONOSUMDB`, `GONOSUMCHECK` and `GOINSECURE` are passed on; credentials
  are not, so private modules that need them fail with the reason.
- **.NET** (`sonaranalyzer-csharp`): one `dotnet restore` runs first, with
  network, into the volume `sloptopop-dotnet-<key>` of this repository; the
  build then runs offline with `--no-restore`. `obj/` and `bin/` go to that
  volume, never into your source tree. Private feeds that need credentials
  fail with the reason.

These steps fetch your project's own dependencies from the registries your
project uses; they are not sloptopop components. Remove the volumes with
`docker volume rm` (or `podman volume rm`) when you no longer need them.

## Vulnerability data

OSV-Scanner and govulncheck read their data from a snapshot mounted
read-only at `/db`. The component library serves a snapshot per day:
`db-osv-<ecosystem>` (PyPI, npm, Go, crates.io, Maven, Packagist, NuGet,
SwiftURL, for the ecosystems the repository uses) and `db-govulndb`.

- Each run uses the newest snapshot (the library's `latest` tag), or the one
  of `--vuln-db <YYYYMMDD>`; with `--offline`, the newest one already
  cached.
- It is recorded by digest in `.collect.json`, so the same data can be used
  again.
- It is unpacked once into your cache (`SLOPTOPOP_CACHE_DIR`, else
  `$XDG_CACHE_HOME/sloptopop`, `~/.cache/sloptopop` or
  `%LOCALAPPDATA%\sloptopop\cache`).
- A snapshot older than 7 days gets a warning.

This is the one input that is **not pinned per sloptopop release**: a
vulnerability database that is months old would miss what matters.

## Mirrors, proxies and air-gapped machines

- **A mirror or proxy:** `--registry` or `SLOPTOPOP_REGISTRY` replaces the
  registry (host, optional port and path); the digests do not change, so the
  verification does not either. It works with any OCI registry: a Harbor
  proxy-cache project, Zot, Artifactory or Nexus. Credentials stay in your
  runtime (`docker login`); `sloptopop` never handles them. With Podman you
  can also configure a `mirror-by-digest-only` mirror in `registries.conf`.
- **A self-hosted engine** serves the whole library itself: point
  `SLOPTOPOP_REGISTRY` at it.
- **No network at all:** each release ships the library as an OCI image
  layout archive per architecture. Copy it into your registry
  (`skopeo copy --all --preserve-digests oci-archive:… docker://your-registry/…`),
  then set `SLOPTOPOP_REGISTRY`. On a single machine with Docker's
  containerd image store (the default since Docker 29) `docker load -i`
  also works: `collect` accepts an image whose ID is the pinned digest.
  Then run with `--offline`.

## Conversions

Five tools write no SARIF. They write their own format to
`.sloptopop/reports/.raw/` (hidden, never read by the engine), and
`collect` converts it, keeping each tool's own rule id and message:

| Tool | Format | Results |
|---|---|---|
| `scorecard-license` | Scorecard JSON | one result, about the repository, when the License check finds no licence file (score 0) |
| `phpcs` | PHP_CodeSniffer JSON | every message, `ERROR` as error, others as warning |
| `pylint-cyclic-import` | Pylint `json2` | every message; fatal and error as error, warning, refactor and convention as warning |
| `dependency-cruiser` | dependency-cruiser JSON | every rule violation, at its `from` module, with the cycle |
| `clang-tidy` | clang-tidy diagnostics | every diagnostic of the requested check (compiler diagnostics are not findings) |

Output that cannot be read is a `failed` tool, never an empty report.

## Determinism

The same repository, the same images and the same vulnerability snapshot
give byte-identical reports on any machine (amd64 or arm64, any runtime),
and so on the server, which may re-run the same images:

- the repository is always at `/src`, the environment and hostname are
  fixed;
- every SARIF report is normalized: paths under `/src` become relative to
  the root, the invocations' start and end times are dropped, the rules are
  sorted by id (with the results' indexes following), the rules' help
  texts are dropped (some tools print their tables in a random order; the
  engine reads only the short descriptions), and so are the C# compiler's
  analyzer timings;
- `jscpd` runs with one worker, and Pylint walks the tree in sorted order;
- `.collect.json` has no timestamps and no durations.

gitleaks' results on the working tree and its history depend on files the
server never sees as they are (secret values are masked in the upload, and
the history is not uploaded), so those two stay evidence from your machine.

## `--host-tools` (unsupported)

`--host-tools` runs the catalogue's commands with the tools on your `PATH`,
whatever their versions, outside any container. It exists for machines
where no container runtime can run (CI jobs that are themselves containers
without Docker-in-Docker, policies that forbid container runtimes). It is
never chosen automatically:

- every run prints `warning: --host-tools runs unpinned tool versions from
  PATH; results are not reproducible and not supported`;
- `.collect.json` records `"mode": "host-unpinned"` and each tool's own
  `--version` line;
- the entries that exist only as images (`eslint-sonarjs`,
  `eslint-sonarjs-json`, `eslint-typescript`, `dependency-cruiser`,
  `sonaranalyzer-csharp`, `jscpd`, `detekt`, `clang-tidy`) are `missing`;
- OSV-Scanner downloads its own database (`--offline-vulnerabilities
  --download-offline-databases`) and govulncheck asks vuln.go.dev;
- there is no time limit.

## Why these flags

- **Complete:** issue caps are turned off (golangci-lint
  `--max-issues-per-linter=0 --max-same-issues=0 --uniq-by-line=false`), so
  a report is never silently truncated. gitleaks runs in both `dir` mode
  (the working tree) and `git` mode (the whole history), because a secret
  removed from the tree is still in the history until it is rotated.
- **Offline:** no tool has network. OSV-Scanner runs with `--offline` on the
  mounted snapshot, so the dependency list never leaves the machine;
  govulncheck reads the mounted Go database. Semgrep runs with `--metrics
  off --disable-version-check`; Ruff and SwiftLint with `--no-cache`, so
  they write nothing into the repository.
- **Own rules, never registry rules:** Opengrep and Semgrep always get an
  explicit rules directory: sloptopop's own rule pack (MIT OR Apache-2.0,
  written for sloptopop; see [`rules/README.md`](../rules/README.md)) plus
  `.sloptopop/rules/` if you have one, or only the directory you pass with
  `--rules`. They never run without `--config`, which would fetch rules
  from the registry. The Semgrep registry rules are not under an OSI
  licence and are never used. `--no-rewrite-rule-ids` keeps each rule id
  as written (`sloptopop.errors.empty-catch`).
- **Opengrep by default, Semgrep on request:** both run the same rule pack;
  running both doubles the findings and the download.
- **Secrets stay redacted:** gitleaks always runs with `--redact`, and the
  reports directory is ignored by git.
- **"Findings" is not failure:** each tool's "findings found" exit codes are
  listed, so a tool that found problems counts as `ran` and only genuine
  failures are reported as `failed`.
- **Pinned, not "latest":** an analyzer upgrade changes findings, so it
  happens with a sloptopop release, after sloptopop's regression tests ran
  on the new image, never behind your back.

## Licences

Every tool in the catalogue is under an OSI-approved licence. `collect`
only starts them as separate programs in their own containers and reads the
files they write: nothing is linked into `sloptopop`. Their output carries no
licence obligation for the code that reads it, and `sloptopop` stays MIT OR
Apache-2.0, like the rule pack built into it.

The images themselves carry each tool, its runtime and its base system
under their own licences (GPL-3.0 for golangci-lint, GPL-2.0-or-later for
Pylint, LGPL for Opengrep, Semgrep and the Sonar analyzers, GPL-2.0 for parts
of the Debian or Alpine base). Each image lists them in its
`org.opencontainers.image.licenses` label and SBOM and holds their texts and
notices in `/licenses/`; the corresponding source of each image's copyleft
components is published next to it in the component library.

Every licence in the table below was read from the tool's licence file, at a
release tag or inside the released archive, never only from registry
metadata. The catalogue records the files it was read from
(`licence_files`, also in `.collect.json`), and its tests refuse an entry
without them.

| Entry | Licence | Licence file read |
|---|---|---|
| `gitleaks`, `gitleaks-history` | MIT | [gitleaks v8.30.1 LICENSE](https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE) |
| `osv-scanner` | Apache-2.0 | [osv-scanner v2.7.0 LICENSE](https://github.com/google/osv-scanner/blob/v2.7.0/LICENSE) |
| `opengrep` | LGPL-2.1-only | [opengrep v1.31.0 LICENSE](https://github.com/opengrep/opengrep/blob/v1.31.0/LICENSE) |
| `semgrep` | LGPL-2.1-only | [semgrep v1.181.0 LICENSE](https://github.com/semgrep/semgrep/blob/v1.181.0/LICENSE) |
| `bandit` | Apache-2.0 | [bandit 1.9.4 LICENSE](https://github.com/PyCQA/bandit/blob/1.9.4/LICENSE) |
| `ruff` | MIT | [ruff 0.17.1 LICENSE](https://github.com/astral-sh/ruff/blob/0.17.1/LICENSE) |
| `pylint-cyclic-import` | GPL-2.0-or-later | [pylint v4.1.3 LICENSE](https://github.com/pylint-dev/pylint/blob/v4.1.3/LICENSE) |
| `gosec` | Apache-2.0 | [gosec v2.29.0 LICENSE.txt](https://github.com/securego/gosec/blob/v2.29.0/LICENSE.txt) |
| `govulncheck` | BSD-3-Clause | [golang/vuln v1.1.4 LICENSE](https://github.com/golang/vuln/blob/v1.1.4/LICENSE) |
| `staticcheck` | MIT | [go-tools 2026.2.2 LICENSE](https://github.com/dominikh/go-tools/blob/2026.2.2/LICENSE) |
| `golangci-lint` | GPL-3.0-only | [golangci-lint v2.14.0 LICENSE](https://github.com/golangci/golangci-lint/blob/v2.14.0/LICENSE) |
| `oxlint` | MIT | [oxlint v1.88.0 LICENSE](https://github.com/oxc-project/oxc/blob/oxlint_v1.88.0/LICENSE) |
| `biome` | MIT OR Apache-2.0 | [LICENSE-MIT](https://github.com/biomejs/biome/blob/@biomejs/biome@2.5.15/LICENSE-MIT) and [LICENSE-APACHE](https://github.com/biomejs/biome/blob/@biomejs/biome@2.5.15/LICENSE-APACHE) at Biome 2.5.15 |
| `eslint-sonarjs`, `eslint-sonarjs-json` | LGPL-3.0-only | `package/LICENSE` in [eslint-plugin-sonarjs-2.0.4.tgz](https://registry.npmjs.org/eslint-plugin-sonarjs/-/eslint-plugin-sonarjs-2.0.4.tgz) |
| `eslint-typescript` | MIT | [ESLint v9.39.5 LICENSE](https://github.com/eslint/eslint/blob/v9.39.5/LICENSE) and [typescript-eslint v8.71.2 LICENSE](https://github.com/typescript-eslint/typescript-eslint/blob/v8.71.2/LICENSE) |
| `dependency-cruiser` | MIT | [dependency-cruiser v18.6.0 LICENSE](https://github.com/sverweij/dependency-cruiser/blob/v18.6.0/LICENSE) |
| `swiftlint` | MIT | [SwiftLint 0.65.1 LICENSE](https://github.com/realm/SwiftLint/blob/0.65.1/LICENSE) |
| `sonaranalyzer-csharp` | LGPL-3.0-only | [sonar-dotnet 9.32.0.97167 LICENSE.txt](https://github.com/SonarSource/sonar-dotnet/blob/9.32.0.97167/LICENSE.txt), and the nuspec in [the package](https://api.nuget.org/v3-flatcontainer/sonaranalyzer.csharp/9.32.0.97167/sonaranalyzer.csharp.9.32.0.97167.nupkg) |
| `scorecard-license` | Apache-2.0 | [scorecard v5.6.0 LICENSE](https://github.com/ossf/scorecard/blob/v5.6.0/LICENSE) |
| `jscpd` | MIT | [jscpd v5.4.1 LICENSE](https://github.com/kucherenko/jscpd/blob/v5.4.1/LICENSE) |
| `phpcs` | BSD-3-Clause | [PHP_CodeSniffer 4.0.4 licence.txt](https://github.com/PHPCSStandards/PHP_CodeSniffer/blob/4.0.4/licence.txt) |
| `pmd` | BSD-3-Clause (PMD's BSD-style licence) | [PMD 7.28.0 LICENSE](https://github.com/pmd/pmd/blob/pmd_releases/7.28.0/LICENSE) |
| `detekt` | Apache-2.0 | [detekt v1.23.8 LICENSE](https://github.com/detekt/detekt/blob/v1.23.8/LICENSE) |
| `clang-tidy` | Apache-2.0 WITH LLVM-exception | [clang-tools-extra 19.1.0 LICENSE.TXT](https://github.com/llvm/llvm-project/blob/llvmorg-19.1.0/clang-tools-extra/LICENSE.TXT) |
| `cargo-llvm-cov` | Apache-2.0 OR MIT | [LICENSE-APACHE](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/LICENSE-APACHE) and [LICENSE-MIT](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/LICENSE-MIT) at v0.9.1 |
| `coverage.py` | Apache-2.0 | [coverage.py 7.16.2 LICENSE.txt](https://github.com/nedbat/coveragepy/blob/7.16.2/LICENSE.txt) |
| `c8` | ISC | [c8 v12.0.0 LICENSE.txt](https://github.com/bcoe/c8/blob/v12.0.0/LICENSE.txt) |
| `coverlet` | MIT | [coverlet v10.1.0 LICENSE](https://github.com/coverlet-coverage/coverlet/blob/v10.1.0/LICENSE) |
| `gcovr` | BSD-3-Clause | [gcovr 8.6 LICENSE.txt](https://github.com/gcovr/gcovr/blob/8.6/LICENSE.txt) |

## `.collect.json`

`.sloptopop/reports/.collect.json` records the run. The leading dot hides
it from the engine, which reads only visible files as reports. It is
deterministic: tools in catalogue order, no timestamps, no durations.

```json
{
  "databases": [
    { "name": "db-osv-pypi", "reference": "components.sloptopop.dev/analyzers/db-osv-pypi@sha256:65fc…", "version": "20261021" }
  ],
  "ecosystems": ["python"],
  "mode": "container",
  "registry": "components.sloptopop.dev",
  "runtime": { "name": "docker", "rootless": false, "version": "29.1.3" },
  "schemaVersion": 2,
  "tools": [
    {
      "argv": ["docker", "run", "--rm", "--pull", "never", "…", "--entrypoint", "gitleaks",
               "components.sloptopop.dev/analyzers/gitleaks@sha256:464c…", "dir", ".", "--redact", "…"],
      "exitCode": 0,
      "image": { "reference": "components.sloptopop.dev/analyzers/gitleaks@sha256:464c…", "version": "8.30.1-1" },
      "licence": "MIT",
      "licenceFiles": ["https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE"],
      "name": "gitleaks",
      "output": ".sloptopop/reports/gitleaks.sarif",
      "status": "ran",
      "version": "8.30.1-1"
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `schemaVersion` | `2` (`1` before images became the default) |
| `mode` | `container`, or `host-unpinned` under `--host-tools` |
| `registry`, `runtime` | Where the images came from, and the runtime that ran them (absent if none answered) |
| `ecosystems` | The detected ecosystems, sorted |
| `databases[]` | The vulnerability snapshots used: name, reference by digest, date |
| `tools[].name`, `licence`, `licenceFiles` | The catalogue entry and the licence files its licence was read from |
| `tools[].image` | The image: reference by digest and version |
| `tools[].version` | The image's version (under `--host-tools`, the tool's first `--version` line) |
| `tools[].argv` | The command line as run (the runtime's `run` command line) |
| `tools[].output` | The report path relative to the root (for `sonaranalyzer-csharp`, a pattern with `*`) |
| `tools[].status` | `ran`, `failed`, `missing` or `skipped` |
| `tools[].exitCode` | The tool's exit code; absent if it did not run or was ended by a signal |
| `tools[].reason` | Why it failed (first stderr line), is missing or was skipped |
