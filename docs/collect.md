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
   | `*.csproj`, `*.sln`, `*.slnx` | dotnet |
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
| `opengrep` | opengrep 1.31.0 | LGPL-2.1-only | all | `opengrep scan --json --no-rewrite-rule-ids --config /rules/0 [--config /rules/1] .` (stdout, converted) | `opengrep.sarif` | |
| `semgrep` (only with `--only`) | semgrep 1.181.0 | LGPL-2.1-only | all | `semgrep scan --json --metrics off --disable-version-check --no-rewrite-rule-ids --config /rules/0 [--config /rules/1] .` (stdout, converted) | `semgrep.sarif` | |
| `bandit` | bandit 1.9.4 | Apache-2.0 | python | `bandit -r . -f sarif -o <out>` | `bandit.sarif` | 1 |
| `ruff` | ruff 0.17.1 | MIT | python | `ruff check --output-format json --output-file <out> --no-cache --extend-select <the Python defect rules>,N999,S501 .` (see [Defect rules](#python-ruff)) (converted) | `ruff.sarif` | 1 |
| `pylint-cyclic-import` | pylint 4.1.3 | GPL-2.0-or-later | python | `pylint --disable=all --enable=cyclic-import --output-format=json2 --recursive=y --exit-zero .` (stdout, converted) | `pylint-cyclic-import.sarif` | |
| `gosec` | gosec 2.29.0 | Apache-2.0 | go | `gosec -fmt sarif -out <out> ./...` | `gosec.sarif` | 1 |
| `govulncheck` | govulncheck 1.1.4 | BSD-3-Clause | go | `govulncheck -db file:///db/govulndb -format sarif ./...` (stdout) | `govulncheck.sarif` | 3 |
| `staticcheck` | staticcheck 2026.2.2 | MIT | go | `staticcheck -f sarif ./...` (stdout) | `staticcheck.sarif` | 1 |
| `golangci-lint` | golangci-lint 2.14.0 | GPL-3.0-only | go | `golangci-lint run --max-issues-per-linter=0 --max-same-issues=0 --uniq-by-line=false --output.sarif.path=<out> ./...` (a version-1 configuration file is refused: see [below](#when-a-tool-refuses-your-configuration)) | `golangci.sarif` | 1 |
| `golangci-lint-defects` | golangci-lint 2.14.0 | GPL-3.0-only | go | `golangci-lint run --config=/opt/golangci-lint/defects.yml --max-issues-per-linter=0 --max-same-issues=0 --uniq-by-line=false --output.sarif.path=<out> ./...` (the image's configuration, never the repository's; see [Defect rules](#go-golangci-lint)) | `golangci-defects.sarif` | 1 |
| `oxlint` | oxlint 1.88.0 | MIT | node | `oxlint --format sarif .` (stdout) | `oxlint.sarif` | 1 |
| `biome` | biome 2.5.15 | MIT OR Apache-2.0 | node | `biome lint --reporter=rdjson --max-diagnostics=none .` (stdout, converted) | `biome.sarif` | 1 |
| `eslint-sonarjs` | eslint-sonarjs 2.0.4 | LGPL-3.0-only | node | `eslint --config /opt/eslint-sonarjs/eslint.config.mjs --no-inline-config --no-error-on-unmatched-pattern --format json-with-metadata --output-file <out> .` (converted) | `eslint-sonarjs.sarif` | 1 |
| `eslint-typescript` | eslint-typescript 8.71.2 | MIT | node | `eslint --config /opt/eslint-typescript/eslint.config.mjs --no-inline-config --no-error-on-unmatched-pattern --format json-with-metadata --output-file <out> .` (converted) | `eslint-typescript.sarif` | 1 |
| `dependency-cruiser` | dependency-cruiser 18.6.0 | MIT | node | `depcruise --no-progress --config /opt/dependency-cruiser/depcruise.cjs --output-type json .` (stdout, converted) | `dependency-cruiser.sarif` | |
| `swiftlint` | swiftlint 0.65.1 | MIT | swift | `swiftlint lint --quiet --no-cache --reporter sarif` (stdout) | `swiftlint.sarif` | 2 |
| `swiftlint-defects` | swiftlint 0.65.1 | MIT | swift | `swiftlint lint --quiet --no-cache --reporter sarif --only-rule <rule>...` (stdout; one `--only-rule` per Swift defect rule, see [Defect rules](#swift-swiftlint)) | `swiftlint-defects.sarif` | 2 |
| `sonaranalyzer-csharp` | sonaranalyzer-csharp 9.32.0.97167 | LGPL-3.0-only | dotnet | `dotnet build --no-restore --no-incremental -maxcpucount:1 -p:TreatWarningsAsErrors=false -p:DirectoryBuildPropsPath=/opt/sonaranalyzer-csharp/Directory.Build.props` | `sonaranalyzer-csharp.<project>[.<framework>].sarif` | |
| `scorecard-license` | scorecard 5.6.0 | Apache-2.0 | all | `scorecard --local=. --checks=License --format=json` (stdout, converted) | `scorecard-license.sarif` | |
| `jscpd` | jscpd 5.4.1 | MIT | all | `jscpd --reporters sarif --output=.sloptopop/reports --report-name jscpd --min-lines 5 --workers 1 --silent --no-tips --ignore **/.sloptopop/**,**/node_modules/** .` | `jscpd.sarif` | |
| `phpcs` | phpcs 4.0.4 | BSD-3-Clause | php | `phpcs --standard=Squiz,Generic --sniffs=Squiz.Classes.ClassFileName,Generic.CodeAnalysis.EmptyStatement --extensions=php --report=json --no-cache -q .` (stdout, converted) | `phpcs.sarif` | 1, 2, 3 |
| `pmd` | pmd 7.28.0 | BSD-3-Clause | jvm | `pmd check --no-cache --no-progress -d . -R <the Java defect rules> -f sarif -r <out>` (see [Defect rules](#java-pmd)) | `pmd.sarif` | 4 |
| `detekt` | detekt 1.23.8 | Apache-2.0 | jvm | `detekt --input . --config /opt/detekt/sloptopop.yml --report sarif:<out>` | `detekt.sarif` | 2 |
| `clang-tidy` | clang-tidy 19.1.0 | Apache-2.0 WITH LLVM-exception | cpp | `sloptopop-clang-tidy '-*,<the checks that are on>'` (stdout, converted; the image's wrapper runs clang-tidy over the C and C++ files `compile_commands.json` lists, from the root, `build/` or `out/`, else over every C file as C17 and every C++ file as C++17, always for `x86_64-linux-gnu`; see [C and C++ defect rules](#c-and-c-defect-rules)) | `clang-tidy.sarif` | |
| `cppcheck` | cppcheck 2.17.1 | GPL-3.0-or-later | cpp | `sloptopop-cppcheck --enable=warning,performance,portability --library=googletest --library=/usr/share/sloptopop-cppcheck/tests.cfg` (stdout, converted; the image's wrapper runs Cppcheck over the C and C++ sources, from `compile_commands.json` if there is one, else as C17 and C++17 with no defines, for the `unix64` platform; see [C and C++ defect rules](#c-and-c-defect-rules)) | `cppcheck.sarif` | |

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

## Defect rules

Some tools report defect rules: rules that find a class of bug (a null
dereference, a leak, a swallowed error) rather than style or complexity.
`collect` knows these rules by id, from a table in the source
(`src/collect_catalogue/defects.rs` for C and C++,
`src/collect_catalogue/lint_defects.rs` for the others), and tags each one
in the report with its defect class: the rule's descriptor gets
`properties.defectClass`. A tool that writes results without rule
descriptors (golangci-lint) gets one for each listed rule it reported.
Rules a tool reports that the table does not list pass through as the
tool wrote them, untagged.

Each listed rule belongs to exactly one defect class:

| Class | Defect | CWE |
|---|---|---|
| DC01 | null or absent-value dereference | 476, 690 |
| DC02 | missing error or result check | 252, 253 |
| DC03 | resource leak (memory, file, handle) | 401, 772, 775 |
| DC04 | a lock not released on some path | 667, 832 |
| DC05 | use after free, double free, dangling reference | 416, 415, 825, 562 |
| DC06 | out-of-bounds access | 125, 787, 129 |
| DC07 | off-by-one, wrong bound comparison | 193 |
| DC08 | integer overflow, wrap, truncation, sign error | 190, 191, 197, 681 |
| DC09 | division or remainder by zero | 369 |
| DC10 | read of an uninitialised value | 457, 908 |
| DC11 | data race, missing synchronisation, check-then-act | 362, 366, 367 |
| DC12 | deadlock, blocking while holding a lock | 833 |
| DC13 | injection: SQL, OS command, path traversal, format string | 89, 78, 88, 22, 134 |
| DC14 | an asynchronous result nobody observes (floating promise, unawaited task) | 252, 754 |
| DC15 | a closure that captures a loop variable late | 665 (closest) |
| DC16 | wrong operator, condition or call | 480, 481, 570, 571, 563 |
| DC17 | swallowed error (empty handler) | 390 |
| DC19 | a language trap: mutable default argument, `==` coercion, `finally` overriding `return` | 1188, 597 |
| DC20 | a forced unwrap, cast or `try!` that crashes when it fails | 617, 248 |

**Which rules are on.** A listed rule is reported by default only if its
sample precision is at least 0.80 on sloptopop's public precision corpus
for its language (public repositories at pinned commits; up to 20 findings
per rule, sampled at random and read by hand), for a rule with five
findings or more there. A rule below that stays listed, `off`, with the
reason. A rule with one to four findings keeps the default it had before it
was measured (a rule new to the table was not reported, so it is off), and
a rule with no findings is on until it is measured. Some rules stay on
below the budget because the tool reported them before the table existed:
the swallowed-errors rules (`S110`, PMD's and detekt's `EmptyCatchBlock`,
ESLint's `no-empty`, `bugprone-empty-catch`) and the recommended sets of
eslint-plugin-sonarjs and Biome, which are tagged but not filtered. Each
table below gives every rule's default and, for one that is off, why.

**How a rule is off.** clang-tidy does not run it. Every other tool runs
it and its results are dropped from the report (they stay in the raw
output, if the tool has one).

**Rules that need types.** Some defect rules need the project's type
information: eslint-plugin-sonarjs `null-dereference`, typescript-eslint
`strict-boolean-expressions` and `no-unnecessary-condition`, and detekt's
type-resolution rules. `collect` never installs a project's dependencies,
so it has no types to give them: `null-dereference` turns itself off
without types, the typescript-eslint rules are not configured, and detekt's
type-resolution rules report nothing.

### C and C++ defect rules

`clang-tidy` and `cppcheck` report the defect rules listed below, and only
those, each tagged with its defect class. Both run from their images'
wrappers over the repository's C and C++ sources in sorted order:

- **Compile flags.** With a `compile_commands.json` at the root, in
  `build/` or in `out/` (in that order), the files analyzed are the C and
  C++ files the database lists, each compiled as the project's build
  compiles it. The file is only read. A build writes the paths of your
  checkout, so `collect` gives both containers your checkout's path
  (`SLOPTOPOP_HOST_ROOT`) and their wrappers map those paths to `/src` in
  a copy. A database naming files outside the repository makes Cppcheck
  fail with its own message (`failed`, no report); clang-tidy reports each
  such file as an error (below) and goes on with the rest. Without a
  database, every C file is analyzed as C17 and every C++ file (`.cc`,
  `.cpp`, `.cxx`, `.c++`) as C++17, with the repository root and
  `include/` as include directories and no defines (Cppcheck checks only
  that one configuration). `collect` never runs your build to make a
  database.
- **Confidence.** Each report records how its files were compiled
  (`runs[0].properties.compileFlags`, the wrapper's own words). Without the
  project's compile database every result is marked
  `"properties": {"confidence": "lower"}`.
- **Same everywhere.** clang-tidy always compiles for `x86_64-linux-gnu`
  and Cppcheck always analyzes for its `unix64` platform, whatever the
  host, so amd64 and arm64 machines report the same bytes.
- **System headers.** The clang-tidy image holds the C and C++ system
  headers of Debian trixie for `x86_64-linux-gnu` (glibc 2.41, Linux 6.12
  and libstdc++ 14, pinned by checksum), so `<stdio.h>` and `<string>`
  resolve. Headers of other libraries resolve only through the compile
  database's include paths.
- **Errors stay in the report.** A file clang-tidy cannot compile, or a
  file the database lists that is not there, is in the clang-tidy report
  as a tool execution notification
  (`runs[0].invocations[0].toolExecutionNotifications`), never as a
  finding and never silently dropped.
- **Test assertions.** Cppcheck runs with `--library=googletest` and the
  image's `tests.cfg`, which tells it that Unity's `TEST_ASSERT*`, Catch2's
  `REQUIRE*` and `assert()` end a test when they fail, so a pointer a test
  asserted is not null is not reported as one that may be.

#### clang-tidy 19.1.0

The `clang-analyzer-core`, `-cplusplus`, `-deadcode` and `-unix` families
(without the Objective-C `core.NonnilStringConstants` and the modeling
checkers, which report nothing and run with the checkers that need them),
and the bugprone checks for the same classes. The command line is
`sloptopop-clang-tidy '-*,<every check that is on>'`.

<!-- rules: CLANG_TIDY_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `bugprone-branch-clone` | DC16 | identical branches of an if/else chain or switch: a copy-paste or wrong-branch error | off: precision 0/20 on the C/C++ precision corpus (29 findings, with the image's system headers): the shared branch bodies are deliberate, and three "identical" verdicts come from bodies clang could not type-check |
| `bugprone-dangling-handle` | DC05 | a string_view or other non-owning handle that outlives the object it points into | on |
| `bugprone-empty-catch` | DC17 | an empty catch handler hides the error (the swallowed-errors transformation) | on |
| `bugprone-implicit-widening-of-multiplication-result` | DC08 | a multiplication done in the narrow type, then widened: the product can overflow first | off: precision 1/20 on the C/C++ precision corpus (22 findings, with the image's system headers): products of small constants or bounded values that fit in int |
| `bugprone-infinite-loop` | DC16 | a loop whose condition no statement in its body can change | off: one finding on the C/C++ precision corpus with the image's system headers (0/1: both loop variables change in the body), too few to overturn the earlier 0/14 |
| `bugprone-misplaced-widening-cast` | DC08 | a cast that widens the result of a calculation that already overflowed | on |
| `bugprone-narrowing-conversions` | DC08 | an implicit conversion that can lose value (integer or floating point) | off: precision 0/20 on the C/C++ precision corpus (246 findings, with the image's system headers): values that fit the target type in every reachable case |
| `bugprone-signed-char-misuse` | DC08 | a signed char converted to a wider integer: negative values sign-extend | off: precision 1/10 on the C/C++ precision corpus (10 findings, with the image's system headers): the converted values are ASCII, small table entries or bytes read as signed on purpose |
| `bugprone-too-small-loop-variable` | DC07 | a loop variable whose type cannot reach the loop bound | on |
| `bugprone-unchecked-optional-access` | DC01 | std::optional (and absl/folly) value read without checking it holds one | on |
| `bugprone-unused-return-value` | DC02 | the result of a function whose result must be used is dropped | on |
| `bugprone-use-after-move` | DC05 | an object used after it was moved from | on |
| `clang-analyzer-core.BitwiseShift` | DC08 | a shift by a negative amount or by at least the operand's width (undefined behaviour) | on |
| `clang-analyzer-core.CallAndMessage` | DC01 | a call through a null function pointer or on a null object, or with an undefined argument | on |
| `clang-analyzer-core.DivideZero` | DC09 | a division or remainder whose divisor is zero on some path | on |
| `clang-analyzer-core.NonNullParamChecker` | DC01 | null passed for a parameter declared nonnull | on |
| `clang-analyzer-core.NullDereference` | DC01 | a pointer that is null on some path is dereferenced | on |
| `clang-analyzer-core.StackAddressEscape` | DC05 | the address of a local variable escapes its function | on |
| `clang-analyzer-core.UndefinedBinaryOperatorResult` | DC10 | an operator with an undefined (uninitialised) operand | on |
| `clang-analyzer-core.VLASize` | DC06 | a variable-length array of zero, negative or undefined size | on |
| `clang-analyzer-core.uninitialized.ArraySubscript` | DC10 | an array indexed with an uninitialised value | on |
| `clang-analyzer-core.uninitialized.Assign` | DC10 | an uninitialised value assigned | on |
| `clang-analyzer-core.uninitialized.Branch` | DC10 | a branch on an uninitialised value | on |
| `clang-analyzer-core.uninitialized.CapturedBlockVariable` | DC10 | an uninitialised variable captured by a block | on |
| `clang-analyzer-core.uninitialized.NewArraySize` | DC10 | new[] with an uninitialised size | on |
| `clang-analyzer-core.uninitialized.UndefReturn` | DC10 | an uninitialised value returned | on |
| `clang-analyzer-cplusplus.ArrayDelete` | DC05 | an array of derived objects deleted through a base-class pointer | on |
| `clang-analyzer-cplusplus.InnerPointer` | DC05 | a pointer into a std::string's buffer used after the string changed or died | on |
| `clang-analyzer-cplusplus.Move` | DC05 | an object used after it was moved from (path-sensitive) | on |
| `clang-analyzer-cplusplus.NewDelete` | DC05 | use after delete, double delete, delete of memory new did not allocate | on |
| `clang-analyzer-cplusplus.NewDeleteLeaks` | DC03 | memory from new never deleted on some path | on |
| `clang-analyzer-cplusplus.PlacementNew` | DC06 | placement new into storage too small for the object | on |
| `clang-analyzer-cplusplus.PureVirtualCall` | DC05 | a pure virtual call during construction or destruction: the object is used outside its lifetime as that type | on |
| `clang-analyzer-cplusplus.StringChecker` | DC01 | a std::string constructed from a null pointer | on |
| `clang-analyzer-deadcode.DeadStores` | DC16 | a value stored and never read: a lost assignment or the wrong variable | off: precision 6/13 (0.46) on the C/C++ precision corpus (13 findings, with the image's system headers): the hits are overwritten error codes, the misses harmless last increments and results discarded on purpose |
| `clang-analyzer-unix.API` | DC16 | a POSIX call made wrongly: open with O_CREAT and no mode, an allocation of 0 bytes, pthread_once on a local | on |
| `clang-analyzer-unix.BlockInCriticalSection` | DC12 | a blocking call made while a lock is held | on |
| `clang-analyzer-unix.Errno` | DC02 | errno read when the last call did not set it, or not checked when it did | on |
| `clang-analyzer-unix.Malloc` | DC03 | malloc'd memory leaked on some path (the same id also reports use after free and double free) | off: precision 2/7 on the C/C++ precision corpus (7 findings, with the image's system headers): most are tests whose allocator fails on purpose |
| `clang-analyzer-unix.MallocSizeof` | DC06 | an allocation sized with sizeof of the wrong type | on |
| `clang-analyzer-unix.MismatchedDeallocator` | DC05 | memory released by a deallocator that does not match its allocator | on |
| `clang-analyzer-unix.StdCLibraryFunctions` | DC01 | a C library call that breaks the function's preconditions (mostly a null argument) | on |
| `clang-analyzer-unix.Stream` | DC03 | a FILE stream leaked, used after fclose, or used when fopen failed | off: precision 1/6 on the C/C++ precision corpus (6 findings, with the image's system headers): idiomatic fread loops that stop at end of file |
| `clang-analyzer-unix.Vfork` | DC16 | a vfork child that does more than exec or _exit | on |
| `clang-analyzer-unix.cstring.BadSizeArg` | DC06 | a strncat-style size argument that can overflow the destination | on |
| `clang-analyzer-unix.cstring.NullArg` | DC01 | a null pointer passed to a C string function | on |

#### Cppcheck 2.17.1

`sloptopop-cppcheck --enable=warning,performance,portability
--library=googletest --library=/usr/share/sloptopop-cppcheck/tests.cfg`; the
wrapper adds `--platform=unix64 --xml --xml-version=2` and either
`--project=<database>` or `--std=c17 --std=c++17 --max-configs=1
--file-list=<sorted sources>`. Cppcheck's other findings (performance and
portability advice, configuration and syntax diagnostics) are not defects of
these classes and are dropped.

<!-- rules: CPPCHECK_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `accessForwarded` | DC05 | a variable used after it was forwarded | on |
| `accessMoved` | DC05 | a variable used after it was moved from | on |
| `argumentSize` | DC06 | a buffer smaller than the size the called function expects | on |
| `arrayIndexOutOfBounds` | DC06 | an array indexed past its end | on |
| `arrayIndexOutOfBoundsCond` | DC07 | an array indexed past its end when a condition (usually the loop bound) holds | on |
| `autoVariables` | DC05 | the address of a local variable stored where it outlives the function | on |
| `autovarInvalidDeallocation` | DC05 | free or delete of a local (stack) variable | on |
| `badBitmaskCheck` | DC16 | a bitmask test that is always true or always false (| for &) | on |
| `bufferAccessOutOfBounds` | DC06 | a buffer read or written past its end | on |
| `charLiteralWithCharPtrCompare` | DC16 | a char literal compared with a pointer: a missing dereference | on |
| `checkCastIntToCharAndBack` | DC08 | a function's int result (EOF) stored in a char, then compared with EOF | on |
| `compareBoolExpressionWithInt` | DC16 | a boolean expression compared with an integer other than 0 or 1 | on |
| `comparePointers` | DC16 | pointers to different objects compared | on |
| `comparisonOfBoolWithInvalidComparator` | DC16 | a boolean compared with <, >, <= or >= | on |
| `containerOutOfBounds` | DC06 | a container indexed past its end | on |
| `ctuArrayIndex` | DC06 | an array indexed past its end, across functions | on |
| `ctuPointerArith` | DC06 | pointer arithmetic past an object's end, across functions | on |
| `ctunullpointer` | DC01 | a null pointer dereferenced, across functions | off: precision 0/3 on the C/C++ precision corpus: defensive null checks in callees and test assertions |
| `ctuuninitvar` | DC10 | an uninitialised value read, across functions | on |
| `danglingLifetime` | DC05 | a non-local variable left pointing at an object that dies | off: precision 0/1 on the C/C++ precision corpus: a pointer to a heap object owned by a moved unique_ptr |
| `danglingReference` | DC05 | a non-local reference bound to a local variable | on |
| `danglingTempReference` | DC05 | a reference to a temporary used after the temporary died | on |
| `danglingTemporaryLifetime` | DC05 | an object that points into a temporary used after the temporary died | on |
| `deallocret` | DC05 | a pointer returned or dereferenced after it was released | on |
| `deallocuse` | DC05 | a pointer dereferenced after it was released | on |
| `derefInvalidIterator` | DC05 | an iterator dereferenced when it can be invalid (end) | on |
| `doubleFree` | DC05 | memory released twice | on |
| `eraseDereference` | DC05 | an iterator used after erase invalidated it | on |
| `eraseIteratorOutOfBounds` | DC06 | erase called on an iterator past the container's end | on |
| `eraseIteratorOutOfBoundsCond` | DC06 | erase called on an iterator past the end when a condition holds | on |
| `floatConversionOverflow` | DC08 | a floating-point value converted to an integer type it does not fit | on |
| `identicalConditionAfterEarlyExit` | DC16 | the same condition tested again after an early exit: the second test is always false | on |
| `identicalInnerCondition` | DC16 | an inner if with the outer if's condition: always true | off: precision 0/1 on the C/C++ precision corpus: true only in the one (no-define) configuration checked |
| `ignoredReturnValue` | DC02 | the result of a function whose result must be used is dropped | on |
| `incompleteArrayFill` | DC16 | memset/memcpy sized in elements, not bytes | on |
| `incorrectCharBooleanError` | DC16 | a char literal used as a boolean: always true | on |
| `incorrectLogicOperator` | DC16 | a logical expression that is always true or always false (&& for ||) | on |
| `incorrectStringBooleanError` | DC16 | a string literal used as a boolean: always true | on |
| `incorrectStringCompare` | DC16 | a substring compared with a literal of a different length | on |
| `integerOverflow` | DC08 | signed integer overflow | on |
| `integerOverflowCond` | DC08 | signed integer overflow when a condition holds | on |
| `invalidContainer` | DC05 | a reference, pointer or iterator into a container used after the container reallocated | on |
| `invalidContainerLoop` | DC05 | a container changed while a loop iterates over it | on |
| `invalidFree` | DC05 | free of an address malloc did not return | on |
| `invalidIterator1` | DC05 | an invalid iterator used | on |
| `invalidLifetime` | DC05 | an object used after its scope ended | on |
| `invalidScanfFormatWidth` | DC06 | a scanf field width larger than the destination buffer | on |
| `invalidTestForOverflow` | DC08 | an overflow test that relies on signed overflow (undefined, optimised away) | on |
| `invalidscanf` | DC06 | scanf %s without a field width | on |
| `leakNoVarFunctionCall` | DC03 | an allocation passed to a function that does not release it | on |
| `leakReturnValNotUsed` | DC03 | the result of an allocation function not stored | on |
| `leakUnsafeArgAlloc` | DC03 | an allocation in an argument list that leaks if another argument throws | on |
| `legacyUninitvar` | DC10 | a variable read before it was initialised (the older checker) | on |
| `literalWithCharPtrCompare` | DC16 | a string literal compared with a pointer: strcmp was meant | on |
| `memleak` | DC03 | allocated memory never released | off: precision 0/1 on the C/C++ precision corpus: a block returned with an offset and freed by its matching function |
| `memleakOnRealloc` | DC03 | p = realloc(p, n): the old block leaks when realloc fails | on |
| `memsetValueOutOfRange` | DC16 | a memset fill value that does not fit an unsigned char | on |
| `memsetZeroBytes` | DC16 | memset of 0 bytes: swapped arguments | on |
| `mismatchAllocDealloc` | DC05 | memory released by a deallocator that does not match its allocator | on |
| `missingReturn` | DC16 | a path out of a non-void function without a return value | on |
| `moduloAlwaysTrueFalse` | DC16 | a comparison of a modulo result that is always true or false | on |
| `multiplySizeof` | DC16 | sizeof multiplied by sizeof | on |
| `negativeArraySize` | DC06 | an array declared with a negative size | on |
| `negativeContainerIndex` | DC06 | a container indexed with a negative value | on |
| `negativeIndex` | DC06 | an array indexed with a negative value | on |
| `negativeMemoryAllocationSize` | DC06 | an allocation of a negative size | on |
| `nullPointer` | DC01 | a null pointer dereferenced | on |
| `nullPointerArithmetic` | DC01 | pointer arithmetic on a null pointer | on |
| `nullPointerArithmeticRedundantCheck` | DC01 | pointer arithmetic on a pointer an earlier check says can be null | on |
| `nullPointerDefaultArg` | DC01 | a pointer parameter whose default is null dereferenced | on |
| `nullPointerOutOfMemory` | DC01 | an allocation's result dereferenced without a check for failure | on |
| `nullPointerOutOfResources` | DC01 | a resource function's result (fopen) dereferenced without a check for failure | on |
| `nullPointerRedundantCheck` | DC01 | a pointer dereferenced where an earlier or later check says it can be null | off: precision 1/19 on the C/C++ precision corpus (19 findings, with the test-assertion libraries; 46 without, 0/20): defensive checks in helpers no caller reaches, re-checks after reassignment; without the test directories 0/17 |
| `objectIndex` | DC06 | the address of a single object indexed past it | on |
| `oppositeInnerCondition` | DC16 | an inner if whose condition contradicts the outer one: dead code | on |
| `overlappingStrcmp` | DC16 | two strcmp comparisons that overlap: one of them is redundant or wrong | on |
| `pointerAdditionResultNotNull` | DC16 | ptr + n compared with null: never null unless it overflowed | on |
| `pointerOutOfBounds` | DC06 | pointer arithmetic past an object's end | on |
| `pointerOutOfBoundsCond` | DC06 | pointer arithmetic past an object's end when a condition holds | on |
| `pointerSize` | DC16 | the size of a pointer used where the size of its data was meant | on |
| `resourceLeak` | DC03 | a file or other resource never closed | on |
| `returnDanglingLifetime` | DC05 | an object returned that points at a local that dies | off: precision 0/2 on the C/C++ precision corpus: it took the address of a function for a dangling local |
| `returnReference` | DC05 | a reference to a local variable returned | on |
| `returnTempReference` | DC05 | a reference to a temporary returned | on |
| `selfInitialization` | DC10 | a member initialised with itself | off: precision 0/6 on the C/C++ precision corpus: it reads m(m) with a constructor parameter of the same name, which is correct C++ |
| `shiftNegative` | DC08 | a shift by a negative amount | on |
| `shiftNegativeLHS` | DC08 | a negative value shifted (undefined before C++20) | on |
| `shiftTooManyBits` | DC08 | a shift by at least the operand's width | on |
| `shiftTooManyBitsSigned` | DC08 | a signed value shifted into or past its sign bit | on |
| `signConversion` | DC08 | a value that can be negative converted to an unsigned type | on |
| `signConversionCond` | DC08 | a value that can be negative when a condition holds converted to an unsigned type | on |
| `signedCharArrayIndex` | DC06 | a signed char used as an array index: a negative index | on |
| `sizeofCalculation` | DC16 | a calculation inside sizeof, which is never evaluated | on |
| `sizeofDivisionMemfunc` | DC16 | a memset/memcpy size divided by sizeof where multiplying was meant | on |
| `sizeofwithsilentarraypointer` | DC16 | sizeof of an array parameter: the size of a pointer | on |
| `stlOutOfBounds` | DC07 | a container indexed with a value that can equal its size | on |
| `stlcstr` | DC05 | the result of c_str() used after the string died | on |
| `stringCompare` | DC16 | a string compared with itself | on |
| `suspiciousCase` | DC16 | a case label with || or &&: matches one value only | on |
| `suspiciousSemicolon` | DC16 | a ; right after if, for or while: the body is empty | on |
| `thisUseAfterFree` | DC05 | a member used after this may have been deleted | on |
| `uninitDerivedMemberVar` | DC10 | a member a derived class's constructor leaves uninitialised | on |
| `uninitDerivedMemberVarPrivate` | DC10 | a private member a derived class's constructor leaves uninitialised | on |
| `uninitMemberVar` | DC10 | a member the constructor leaves uninitialised | off: precision 1/18 on the C/C++ precision corpus (18 findings, with the test-assertion libraries): storage buffers written before they are read, members set by an init method, intentionally trivial constructors |
| `uninitMemberVarPrivate` | DC10 | a private member the constructor leaves uninitialised | on |
| `uninitStructMember` | DC10 | a struct member read before it was initialised | on |
| `uninitdata` | DC10 | allocated memory read before it was initialised | on |
| `uninitvar` | DC10 | a variable read before it was initialised | off: precision 0/2 on the C/C++ precision corpus: reads guarded by a short-circuit or a zero length |
| `unknownEvaluationOrder` | DC16 | an expression whose result depends on unspecified evaluation order | on |
| `useClosedFile` | DC05 | a file used after it was closed | on |
| `uselessAssignmentPtrArg` | DC16 | an assignment to a pointer parameter that has no effect: a missing dereference | on |
| `uselessCallsRemove` | DC02 | the result of std::remove ignored: the elements stay in the container | on |
| `va_list_usedBeforeStarted` | DC10 | a va_list used before va_start | on |
| `zerodiv` | DC09 | a division by zero | on |
| `zerodivcond` | DC09 | a division by a value an earlier or later check says can be zero | on |

### Python: Ruff

`ruff` selects the defect rules beyond Ruff's defaults with
`--extend-select` (with `N999` and `S501`, which are crown-jewel checks,
not defect rules). Precision corpus: the Python repositories of the
evaluation corpus.

<!-- rules: RUFF_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `B006` | DC19 | a mutable default argument, shared by every call | on |
| `B008` | DC19 | a function call in a default argument, evaluated once at definition | off: precision 0/20 on the Python precision corpus (105 findings): FastAPI's dependency markers (Depends, Body) in defaults, evaluated once by design |
| `B012` | DC19 | return, break or continue in finally: it swallows the exception in flight | on |
| `B015` | DC16 | a comparison whose result is thrown away (an assert or assignment was meant) | on |
| `B018` | DC16 | an expression statement with no effect | on |
| `B020` | DC16 | a loop variable that overwrites the iterable it loops over | on |
| `B023` | DC15 | a function defined in a loop that reads the loop variable when called, not when defined | on |
| `B033` | DC16 | a set literal with the same item twice | on |
| `PLR0124` | DC16 | a value compared with itself | on |
| `RUF006` | DC14 | an asyncio task whose reference is dropped: it can be garbage-collected before it finishes, and its exception is never seen | on |
| `RUF012` | DC19 | a mutable class attribute default, shared by every instance | off: 4 findings on the Python precision corpus, too few to sample (0/4: pydantic fields and read-only class configuration); new to the list, so it stays off |
| `S110` | DC17 | try/except/pass: the error is dropped (kept on: the swallowed-errors transformation reads it) | on |
| `S307` | DC13 | eval of a value that may come from outside | on |
| `S602` | DC13 | subprocess with shell=True: the command line goes through the shell | on |
| `S605` | DC13 | a process started through the shell (os.system, os.popen) | off: precision 1/5 on the Python precision corpus (5 findings): constant command lines that nothing from outside reaches |
| `S608` | DC13 | SQL built by string formatting or concatenation | on |
| `SIM115` | DC03 | a file opened without a context manager: it stays open when an exception is raised | on |

### Java: PMD

`pmd` runs exactly these rules of PMD's `errorprone` and `multithreading`
categories (`-R category/java/<category>.xml/<rule>,...`). Precision
corpus: Apache Commons CLI, Gson and jsoup at pinned commits.

<!-- rules: PMD_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `AssignmentInOperand` | DC16 | an assignment inside a condition or other operand | off: precision 0/20 on the Java precision corpus (104 findings): read loops, offset += n and x++ in arguments, written on purpose |
| `AvoidBranchingStatementAsLastInLoop` | DC16 | a loop whose last statement always leaves it: it runs at most once | off: one finding on the Java precision corpus, too few to sample (0/1: a search loop that returns on its first match); new to the list, so it stays off |
| `AvoidDecimalLiteralsInBigDecimalConstructor` | DC08 | new BigDecimal(0.1): the binary double's error is kept | off: one finding on the Java precision corpus, too few to sample (0/1: 5.5, exact in binary); new to the list, so it stays off |
| `AvoidLosingExceptionInformation` | DC17 | an exception's getMessage() or similar called and the result thrown away | on |
| `BrokenNullCheck` | DC01 | a null check that dereferences the value when it is null (|| for &&, or the reverse) | on |
| `CheckSkipResult` | DC02 | the count InputStream.skip returns ignored: fewer bytes may have been skipped | on |
| `CloseResource` | DC03 | a stream, connection or other Closeable not closed on every path | off: precision 1/20 on the Java precision corpus (434 findings): readers over in-memory strings, wrapped caller streams and the saved System.out, which closing would break |
| `CompareObjectsWithEquals` | DC16 | objects compared with == or != instead of equals() | off: precision 0/20 on the Java precision corpus (109 findings): intended identity comparisons (tree nodes, sentinels, this) |
| `ComparisonWithNaN` | DC16 | a comparison with NaN, always false | on |
| `ConstructorCallsOverridableMethod` | DC10 | a constructor that calls an overridable method, which sees a half-built object | off: precision 0/9 on the Java precision corpus (9 findings): no subclass in the project overrides the method |
| `DoNotThrowExceptionInFinally` | DC19 | a throw in finally: it replaces the exception in flight | on |
| `DoubleCheckedLocking` | DC11 | double-checked locking without volatile: another thread can see a half-built object | on |
| `EmptyCatchBlock` | DC17 | an empty catch block (kept on: the swallowed-errors transformation reads it) | on |
| `EqualsNull` | DC16 | x.equals(null), always false or a NullPointerException | off: one finding on the Java precision corpus, too few to sample (0/1: a test of the equals contract); new to the list, so it stays off |
| `IdempotentOperations` | DC16 | an assignment of a variable to itself | on |
| `ImplicitSwitchFallThrough` | DC16 | a switch case that falls through to the next without a comment saying so | off: one finding on the Java precision corpus, too few to sample (0/1: a commented fall-through); new to the list, so it stays off |
| `InvalidLogMessageFormat` | DC16 | a log message whose placeholders do not match its arguments | on |
| `JumbledIncrementer` | DC16 | an inner loop that increments the outer loop's variable | on |
| `MisplacedNullCheck` | DC01 | a null check after the value was already dereferenced in the same condition | on |
| `NonThreadSafeSingleton` | DC11 | a lazily created singleton without synchronisation | on |
| `OverrideBothEqualsAndHashcode` | DC16 | equals() without hashCode() or the reverse: hash collections misbehave | off: precision 0/5 on the Java precision corpus (5 findings): test-only value classes never put in a hash collection |
| `ReturnFromFinallyBlock` | DC19 | return in finally: it discards the exception in flight | on |
| `SuspiciousEqualsMethodName` | DC16 | an equals or hashCode lookalike that overrides nothing | off: one finding on the Java precision corpus, too few to sample (0/1: a JUnit test named equals); new to the list, so it stays off |
| `UnconditionalIfStatement` | DC16 | an if whose condition is the constant true or false | on |
| `UnsynchronizedStaticFormatter` | DC11 | a static DateFormat or similar used from several threads without a lock | on |
| `UnusedNullCheckInEquals` | DC01 | equals() called on the argument after the null check of the receiver it was meant for | off: one finding on the Java precision corpus, too few to sample (0/1: the receiver can never be null); new to the list, so it stays off |
| `UselessOperationOnImmutable` | DC16 | the result of an operation on an immutable BigDecimal or BigInteger thrown away | on |

### Kotlin: detekt

The image's `sloptopop.yml` turns on `EmptyCatchBlock` and these rules of
the `potential-bugs` and `exceptions` rule sets; every other rule set is
off. detekt runs without type resolution, so the rules that need it report
nothing. Precision corpus: Coil, KotlinPoet and Okio at pinned commits.

<!-- rules: DETEKT_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `detekt.empty-blocks.EmptyCatchBlock` | DC17 | an empty catch block (kept on: the swallowed-errors transformation reads it) | on |
| `detekt.exceptions.ReturnFromFinally` | DC19 | return in finally: it discards the exception in flight | on |
| `detekt.exceptions.ThrowingExceptionFromFinally` | DC19 | a throw in finally: it replaces the exception in flight | off: one finding on the Kotlin precision corpus, too few to sample (1/1); new to the list, so it stays off |
| `detekt.potential-bugs.AvoidReferentialEquality` | DC16 | === or !== on a type whose instances should be compared with == | on |
| `detekt.potential-bugs.CastNullableToNonNullableType` | DC01 | a nullable value cast to a non-nullable type, which throws on null | on |
| `detekt.potential-bugs.EqualsAlwaysReturnsTrueOrFalse` | DC16 | an equals() that always returns the same value | on |
| `detekt.potential-bugs.IgnoredReturnValue` | DC02 | the result of a function annotated @CheckReturnValue thrown away | on |
| `detekt.potential-bugs.InvalidRange` | DC07 | a range that is empty because its bounds are the wrong way round | on |
| `detekt.potential-bugs.IteratorHasNextCallsNextMethod` | DC16 | an Iterator.hasNext() that calls next() and so moves the iterator | on |
| `detekt.potential-bugs.LateinitUsage` | DC10 | a lateinit property, which throws when read before it is set | off: precision 0/20 on the Kotlin precision corpus (58 findings): test fixtures and lifecycle properties set before any read |
| `detekt.potential-bugs.MapGetWithNotNullAssertionOperator` | DC01 | map[key]!!, which throws when the key is missing | on |
| `detekt.potential-bugs.NullCheckOnMutableProperty` | DC11 | a null check on a var property that another thread can change before the use | on |
| `detekt.potential-bugs.PropertyUsedBeforeDeclaration` | DC10 | a property read in an initializer before the property is declared | on |
| `detekt.potential-bugs.UnconditionalJumpStatementInLoop` | DC16 | a loop whose body always returns or breaks: it runs at most once | off: 3 findings on the Kotlin precision corpus, too few to sample (0/3: the jumps are conditional); new to the list, so it stays off |
| `detekt.potential-bugs.UnreachableCatchBlock` | DC16 | a catch block an earlier catch of a wider type always takes first | on |
| `detekt.potential-bugs.UnreachableCode` | DC16 | code after a return, throw, break or continue | on |
| `detekt.potential-bugs.UnsafeCallOnNullableType` | DC01 | !! on a nullable value, which throws on null | on |
| `detekt.potential-bugs.UnsafeCast` | DC20 | a cast that can never succeed, so it always throws | on |
| `detekt.potential-bugs.UnusedUnaryOperator` | DC16 | a unary + or - whose result is not used (a broken line continuation) | on |
| `detekt.potential-bugs.UselessPostfixExpression` | DC16 | a postfix ++ or -- whose effect is lost (return x++) | on |
| `detekt.potential-bugs.WrongEqualsTypeParameter` | DC16 | an equals whose parameter is not Any?, so it overloads instead of overriding | on |

### Go: golangci-lint

`golangci-lint-defects` is a second golangci-lint run with the image's own
configuration, `/opt/golangci-lint/defects.yml` (version 2, paths relative
to `/src`), never the repository's: only `govet` with its `nilness`
analyzer and the defect linters below. It therefore also runs on a
repository whose version-1 `.golangci.yml` the `golangci-lint` entry
refuses ([below](#when-a-tool-refuses-your-configuration)). golangci-lint's
SARIF reports the linter, not the analyzer, as the rule. Precision corpus:
the Go repositories of the evaluation corpus.

<!-- rules: GOLANGCI_DEFECT_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `bodyclose` | DC03 | an HTTP response body not closed | on |
| `durationcheck` | DC08 | two time.Duration values multiplied together | on |
| `govet` | DC01 | go vet's nilness analyzer (the only one this run enables): a nil pointer dereferenced, or compared where it is always or never nil | on |
| `makezero` | DC16 | append to a slice made with a non-zero length, which keeps the zero values in front | on |
| `nilerr` | DC17 | nil returned where the error just checked is not nil, or the reverse | on |
| `nilnesserr` | DC17 | a different, nil error returned after an error check | on |
| `rowserrcheck` | DC02 | sql.Rows.Err not checked after the loop | off: one finding on the Go precision corpus, too few to sample (1/1: rows.Err unchecked after the loop); new to the list, so it stays off |
| `spancheck` | DC03 | an OpenTelemetry or OpenCensus span not ended | on |
| `sqlclosecheck` | DC03 | sql.Rows or sql.Stmt not closed | off: 4 findings on the Go precision corpus, too few to sample (4/4: prepared statements never closed); new to the list, so it stays off |

### Swift: SwiftLint

`swiftlint-defects` is a second SwiftLint run with `--only-rule` for each
rule below, opt-in rules included, whatever the repository's
`.swiftlint.yml` turns on or off. Precision corpus: Alamofire, Kingfisher
and SwiftyJSON at pinned commits.

<!-- rules: SWIFTLINT_DEFECT_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `discarded_notification_center_observer` | DC03 | a block-based observer token thrown away: it can never be removed | on |
| `duplicate_conditions` | DC16 | an if/else-if chain or switch that tests the same condition twice | on |
| `force_cast` | DC20 | as!, which crashes when the cast fails | off: precision 0/20 on the Swift precision corpus (60 findings): test and demo code casting a value whose type it just created |
| `force_try` | DC20 | try!, which crashes when the call throws | off: precision 0/20 on the Swift precision corpus (115 findings): test setup calls, where a throw is a test failure |
| `force_unwrapping` | DC01 | a forced unwrap (!) of an optional, which crashes on nil | off: precision 0/20 on the Swift precision corpus (575 findings): test code, where a crash is a failure, and unwraps the code has just made safe |
| `identical_operands` | DC16 | the same operand on both sides of a comparison | on |
| `implicitly_unwrapped_optional` | DC01 | an implicitly unwrapped optional (Type!), which crashes when read while nil | off: precision 0/20 on the Swift precision corpus (40 findings): fixtures and outlets set before any read |
| `unhandled_throwing_task` | DC14 | a Task whose thrown error nobody reads | on |
| `unowned_variable_capture` | DC05 | an unowned capture, which crashes if the object is gone when the closure runs | off: precision 0/20 on the Swift precision corpus (39 findings): closures owned by the object they capture |
| `unused_setter_value` | DC16 | a setter that ignores newValue | off: precision 0/5 on the Swift precision corpus (5 findings): extension points whose setter is empty on purpose |
| `weak_delegate` | DC03 | a strong delegate property: a retain cycle that leaks both objects | off: precision 0/8 on the Swift precision corpus (8 findings): owned delegate objects and closure wrappers, no cycle |

### C#: SonarAnalyzer and the .NET analyzers

The image's `Directory.Build.props` turns on the .NET SDK's own analyzers
(`EnableNETAnalyzers`) beside SonarAnalyzer, and its
`sloptopop.globalconfig` (a global analyzer configuration at level -1, so
the repository's own `.editorconfig` and analyzer configurations win)
makes these rules warnings. The build is serial (`-maxcpucount:1`): in a
parallel build one project can be compiled twice at once, and the two
compilers then write its SARIF log over each other. Precision corpus: FluentValidation at a pinned
commit (Polly and Humanizer, also tried, do not build in the image), on
which none of these rules reported anything, so all are on until
measured.

<!-- rules: SONAR_CSHARP_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `CA1806` | DC02 | the result of a call (HRESULT, TryParse, a new object) thrown away | on |
| `CA2000` | DC03 | an IDisposable not disposed before it goes out of scope | on |
| `CA2011` | DC16 | a property setter that assigns the property itself: infinite recursion | on |
| `CA2012` | DC14 | a ValueTask awaited twice, never, or read before it completed | on |
| `CA2013` | DC16 | ReferenceEquals on value types, always false | on |
| `CA2017` | DC16 | a logging message template whose placeholders do not match its arguments | on |
| `CA2018` | DC06 | Buffer.BlockCopy given an element count instead of a byte count | on |
| `CA2021` | DC16 | Cast<T> or OfType<T> with a type the elements can never be | on |
| `CA2213` | DC03 | a disposable field the type's Dispose does not dispose | on |
| `CA2241` | DC16 | a format string whose placeholders do not match its arguments | on |
| `CA2242` | DC16 | a comparison with NaN, always false | on |
| `CA2245` | DC16 | a property assigned to itself | on |
| `CA2248` | DC16 | Enum.HasFlag with a value of another enum type | on |
| `S1764` | DC16 | the same expression on both sides of a binary operator | on |
| `S2077` | DC13 | SQL built by string formatting or concatenation | on |
| `S2201` | DC02 | the result of a call without side effects thrown away | on |
| `S2222` | DC04 | a lock acquired and not released on every path | on |
| `S2259` | DC01 | a null reference dereferenced on some path | on |
| `S2583` | DC16 | a condition that is always true or always false, so a branch is dead | on |
| `S2589` | DC16 | a boolean expression whose value is always the same | on |
| `S2930` | DC03 | an IDisposable created and never disposed | on |
| `S3655` | DC01 | Nullable<T>.Value read where it can be empty | on |
| `S3966` | DC05 | an object disposed twice | on |

### JavaScript and TypeScript

`eslint-typescript` adds ESLint's core defect rules that need no type
information to its configuration; `eslint-sonarjs` and `biome` run their
recommended sets as before, and their defect rules are tagged. Precision
corpus: the JavaScript and TypeScript repositories of the evaluation
corpus.

ESLint core rules (`eslint-typescript`):

<!-- rules: ESLINT_TYPESCRIPT_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `array-callback-return` | DC16 | an array method callback (map, filter, reduce…) that does not return | off: 4 findings on the JavaScript/TypeScript precision corpus, too few to sample (0/4: the missing return is the intended undefined); new to the list, so it stays off |
| `for-direction` | DC07 | a for loop whose counter moves away from its end condition | on |
| `getter-return` | DC16 | a getter that does not return | on |
| `no-async-promise-executor` | DC14 | an async Promise executor, whose thrown errors the promise never sees | on |
| `no-compare-neg-zero` | DC16 | a comparison with -0, which equals 0 | on |
| `no-cond-assign` | DC16 | an assignment in a condition | on |
| `no-constant-binary-expression` | DC16 | a comparison or logical expression whose result never changes | off: 4 findings on the JavaScript/TypeScript precision corpus, too few to sample (2/4); new to the list, so it stays off |
| `no-constant-condition` | DC16 | a condition that is a constant | on |
| `no-dupe-else-if` | DC16 | an else-if that repeats an earlier condition of the chain | on |
| `no-duplicate-case` | DC16 | a switch with the same case twice | on |
| `no-empty` | DC17 | an empty block, an empty catch among them (kept on: the swallowed-errors transformation reads it) | on |
| `no-fallthrough` | DC16 | a switch case that falls through without a comment saying so | on |
| `no-loop-func` | DC15 | a function created in a loop that captures a variable the loop changes | off: 4 findings on the JavaScript/TypeScript precision corpus, too few to sample (0/4: closures run within the same iteration); new to the list, so it stays off |
| `no-loss-of-precision` | DC08 | a number literal that cannot be stored exactly as a double | on |
| `no-promise-executor-return` | DC14 | a value returned from a Promise executor, which is ignored | off: precision 0/20 on the JavaScript/TypeScript precision corpus (26 findings): arrow executors that return setTimeout's handle by syntax only |
| `no-self-assign` | DC16 | a variable assigned to itself | on |
| `no-self-compare` | DC16 | a value compared with itself | on |
| `no-unmodified-loop-condition` | DC16 | a loop condition whose variables the body never changes | on |
| `no-unsafe-finally` | DC19 | return, throw, break or continue in finally: it overrides the try's outcome | on |
| `no-unsafe-negation` | DC16 | ! applied to the left operand of in or instanceof | on |
| `no-unsafe-optional-chaining` | DC01 | an optional chain whose undefined result is then used where it throws | on |
| `require-atomic-updates` | DC11 | a variable updated from a value read before an await: another update in between is lost | off: precision 0/20 on the JavaScript/TypeScript precision corpus (33 findings): writes to per-request or per-test objects and restores of saved values |
| `use-isnan` | DC16 | a comparison with NaN, always false | on |
| `valid-typeof` | DC16 | typeof compared with a string no type has | on |

eslint-plugin-sonarjs (`eslint-sonarjs`):

<!-- rules: SONARJS_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `sonarjs/bitwise-operators` | DC16 | a bitwise & or | where the logical && or || was meant | on |
| `sonarjs/misplaced-loop-counter` | DC16 | a for loop that updates a counter other than the one its condition tests | on |
| `sonarjs/no-all-duplicated-branches` | DC16 | an if/else or switch whose branches are all the same | on |
| `sonarjs/no-case-label-in-switch` | DC16 | a label inside a switch that looks like a case | on |
| `sonarjs/no-dead-store` | DC16 | a value assigned and overwritten or dropped before anyone reads it | on |
| `sonarjs/no-duplicated-branches` | DC16 | two branches of an if chain or switch with the same body | on |
| `sonarjs/no-element-overwrite` | DC16 | an array element or map entry written twice in a row | on |
| `sonarjs/no-equals-in-for-termination` | DC07 | a for loop that ends on == or !=, which a step past the end never meets | on |
| `sonarjs/no-gratuitous-expressions` | DC16 | a condition whose value is already known at that point | on |
| `sonarjs/no-identical-conditions` | DC16 | an if chain that tests the same condition twice | on |
| `sonarjs/no-identical-expressions` | DC16 | the same expression on both sides of an operator | on |
| `sonarjs/no-ignored-exceptions` | DC17 | an empty catch block that drops the exception | on |
| `sonarjs/no-invariant-returns` | DC16 | a function that returns the same value on every path | on |
| `sonarjs/no-os-command-from-path` | DC13 | an OS command run by a name looked up in PATH, which the environment controls | on |
| `sonarjs/no-redundant-assignments` | DC16 | an assignment of the value the variable already holds | on |
| `sonarjs/no-same-line-conditional` | DC16 | an if on the same line as the closing brace of another, a missing else | on |
| `sonarjs/no-self-compare` | DC16 | a value compared with itself | on |
| `sonarjs/no-unenclosed-multiline-block` | DC16 | indented lines after an if without braces, of which only the first is conditional | on |
| `sonarjs/no-unthrown-error` | DC16 | an Error created and never thrown | on |
| `sonarjs/no-unused-collection` | DC16 | a collection filled and never read | on |
| `sonarjs/no-unused-expressions` | DC16 | an expression statement with no effect | on |
| `sonarjs/no-use-of-empty-return-value` | DC16 | the result of a function that returns nothing used as a value | on |
| `sonarjs/no-useless-increment` | DC16 | x = x++, whose increment is lost | on |
| `sonarjs/non-existent-operator` | DC16 | =+, =- or =! where +=, -= or != was meant | on |
| `sonarjs/os-command` | DC13 | an OS command built from values that may come from outside | on |
| `sonarjs/sql-queries` | DC13 | SQL built by string formatting or concatenation | on |
| `sonarjs/updated-loop-counter` | DC16 | a for loop counter changed inside the body | on |

Biome (`biome`):

<!-- rules: BIOME_RULES -->

| Rule | Class | Finds | Default |
|---|---|---|---|
| `lint/correctness/noConstantCondition` | DC16 | a condition that is a constant | on |
| `lint/correctness/noInvalidUseBeforeDeclaration` | DC10 | a variable or parameter used before it is declared | on |
| `lint/correctness/noPrecisionLoss` | DC08 | a number literal that cannot be stored exactly as a double | on |
| `lint/correctness/noSelfAssign` | DC16 | a variable assigned to itself | on |
| `lint/correctness/noSwitchDeclarations` | DC16 | a declaration in a switch case that the other cases can see uninitialised | on |
| `lint/correctness/noUnreachable` | DC16 | code that can never run | on |
| `lint/correctness/noUnsafeFinally` | DC19 | return, throw, break or continue in finally: it overrides the try's outcome | on |
| `lint/correctness/noUnsafeOptionalChaining` | DC01 | an optional chain whose undefined result is then used where it throws | on |
| `lint/correctness/useIsNan` | DC16 | a comparison with NaN, always false | on |
| `lint/correctness/useValidTypeof` | DC16 | typeof compared with a string no type has | on |
| `lint/suspicious/noAssignInExpressions` | DC16 | an assignment inside an expression, often a mistyped comparison | on |
| `lint/suspicious/noAsyncPromiseExecutor` | DC14 | an async Promise executor, whose thrown errors the promise never sees | on |
| `lint/suspicious/noCompareNegZero` | DC16 | a comparison with -0, which equals 0 | on |
| `lint/suspicious/noDoubleEquals` | DC19 | == or != with its type coercion | on |
| `lint/suspicious/noDuplicateCase` | DC16 | a switch with the same case twice | on |
| `lint/suspicious/noDuplicateElseIf` | DC16 | an else-if that repeats an earlier condition of the chain | on |
| `lint/suspicious/noDuplicateObjectKeys` | DC16 | an object literal with the same key twice: the first value is lost | on |
| `lint/suspicious/noFallthroughSwitchClause` | DC16 | a switch case that falls through to the next | on |
| `lint/suspicious/noGlobalIsNan` | DC19 | the global isNaN, which coerces its argument | on |
| `lint/suspicious/noSelfCompare` | DC16 | a value compared with itself | on |
| `lint/suspicious/noSparseArray` | DC16 | an array literal with a hole (two commas in a row) | on |
| `lint/suspicious/noUnsafeNegation` | DC16 | ! applied to the left operand of in or instanceof | on |
| `lint/suspicious/useIterableCallbackReturn` | DC16 | an array method callback (map, filter, every…) that does not return a value | on |

## Conversions

Six tools write no SARIF, and five more keep their fixes only in another
format. They write that format to `.sloptopop/reports/.raw/` (hidden, never
read by the engine), and `collect` converts it, keeping each tool's own
rule id and message, and its fixes (see [Fixes](#fixes)):

| Tool | Format | Results |
|---|---|---|
| `ruff` | Ruff JSON | every diagnostic, with its fix and Ruff's applicability (`safe`, `unsafe`, `displayonly`), which Ruff's SARIF leaves out |
| `eslint-sonarjs`, `eslint-typescript` | ESLint `json-with-metadata` | every message with a rule (ESLint's own errors are not findings; 2 as error, 1 as warning), with its autofix and its suggestion |
| `biome` | Biome rdjson | every diagnostic, with its code suggestion (`ERROR` as error, `WARNING` as warning, others as note) |
| `opengrep`, `semgrep` | Opengrep and Semgrep JSON | every result, with its rule's fix (`ERROR`, `CRITICAL`, `HIGH` as error; `WARNING`, `MEDIUM` as warning; others as note) |
| `scorecard-license` | Scorecard JSON | one result, about the repository, when the License check finds no licence file (score 0) |
| `phpcs` | PHP_CodeSniffer JSON | every message, `ERROR` as error, others as warning |
| `pylint-cyclic-import` | Pylint `json2` | every message; fatal and error as error, warning, refactor and convention as warning |
| `dependency-cruiser` | dependency-cruiser JSON | every rule violation, at its `from` module, with the cycle |
| `clang-tidy` | clang-tidy diagnostics | every diagnostic of a check that is on (compiler diagnostics are not findings), with its defect class, how the files were compiled, and lower confidence without a compile database |
| `cppcheck` | Cppcheck XML version 2 | every error of an id that is on, at its first location (`error` as error; `warning`, `portability`, `performance` as warning; others as note), with its defect class, how the files were compiled, and lower confidence without a compile database |

Output that cannot be read is a `failed` tool, never an empty report.

## Fixes

Most analyzers know how to fix some of what they report. `collect` keeps
each such fix with its finding, exactly as the tool gave it, so the
server can later turn the safe ones into verified changes (WP-21,
`APPLY_ANALYZER_FIX`). `collect` never applies a fix, never runs a tool's
own fix mode (`--fix`, `--write`), and never changes a fix: not its extent,
its order, its text or its alternatives. It only translates where the
fix is.

**The carrier.** A kept fix is a SARIF `fixes` entry (SARIF 2.1.0 §3.55–§3.57)
whose regions are **byte offsets only** (`byteOffset`, `byteLength`, §3.30.3)
into the file as committed, byte order mark included:

```json
"fixes": [{
  "description": { "text": "Convert to `X | None`" },
  "artifactChanges": [{ "artifactLocation": { "uri": "app/models.py" },
    "replacements": [{ "deletedRegion": { "byteOffset": 1180, "byteLength": 14 },
                       "insertedContent": { "text": "str | None" } }] }],
  "properties": { "sloptopop/applicability": "safe", "sloptopop/toolApplicability": "safe",
                  "sloptopop/source": "ruff-json", "sloptopop/converter": 1,
                  "sloptopop/reviewed": true }
}]
```

The run lists every file a kept fix names in `artifacts`, with its length
and the BLAKE3 digest of the bytes the tool read (`hashes.blake3`), and
every fix that was not kept in `properties["sloptopop/droppedFixes"]`
(`{rule, path, reason}`). A finding always stays; only its fix is dropped.

**Applicability**, normalised from the tool's own word:

| `sloptopop/applicability` | Means | From |
|---|---|---|
| `safe` | the tool applies it in its default fix mode, with no opt-in | Ruff `safe`; ESLint's autofix (`fix`); Biome's safe fix |
| `unsafe` | the tool applies it only with an opt-in | Ruff `unsafe`; Biome's unsafe fix |
| `suggestion` | the tool never applies it by itself | Ruff `displayonly`; ESLint `suggestions`; Opengrep and Semgrep rule fixes; fixes written in a tool's own SARIF (staticcheck) |

**Where a fix is.** Every tool counts positions its own way; `collect`
applies each one's rule, as its source at the pinned version and a
probe with non-ASCII text, CRLF line ends and a byte order mark show:

| Tool | Positions | Rule |
|---|---|---|
| Ruff | 1-based rows and columns, columns in code points | the byte order mark is not a column |
| Biome | 1-based lines and columns, columns in code points | the byte order mark is column 1 of line 1 |
| ESLint | `range`, UTF-16 code units | into the text without its byte order mark |
| staticcheck (its SARIF) | 1-based lines and columns, columns in UTF-8 bytes | whatever its SARIF `columnKind` says |
| Opengrep, Semgrep | byte offsets | into the raw file |

The SARIF fixes other tools write are dropped (`fix.unconverted`): their
text regions do not say their unit reliably (Ruff's, staticcheck's and
golangci-lint's SARIF have no `columnKind`, and Opengrep's SARIF fixes put
whole lines into a region that is part of a line). golangci-lint's own fixes
travel only in its JSON output, which `collect` does not run yet; they are
suggestions (go/analysis: "at most one may be applied").

**Dropped, with the reason**: a file that is not UTF-8 (`fix.not-utf8`) or
has a carriage return without a line feed (`fix.lone-cr`); a position past
its line or file (`fix.out-of-range`), inside a character or between `\r`
and `\n` (`fix.boundary`), inside a UTF-16 surrogate pair
(`fix.surrogate`); an edit touching the byte order mark (`fix.bom`); a fix
in a notebook cell (`fix.notebook`); edits out of order, overlapping, or
two insertions at one point (`fix.overlap-within`); several fixes of one
level for one finding (`fix.alternatives`); a fix in several files
(`fix.multi-file`); a fix without a file or an edit (`fix.placeholder`);
an ESLint fix for text other than the file's (`fix.stale`); a path that is
not a regular file below the root (`fix.path`); a level or format the
converter does not know (`fix.unconverted`); and a safe fix whose
safety the project's configuration can change (`fix.config`, see below).

**Reviewed.** `sloptopop/reviewed` is `true` only when the reviewed fix
table (`src/collect_convert/reviewed-fixes.tsv`) lists the tool, its
upstream version, the rule and the level, with the row switched `on`. The
level the tool reports must equal the table's: a promoted fix, a lying
report or a newer tool version makes the fix unreviewed (it is still
carried, never planned). The table holds the 35 safe-level rules the
pinned tools fixed on sloptopop's public corpus. Three are owner defaults
and **switchable** (set their state to `off` to exclude them):
Biome `useImportType` (can break decorator metadata and imports kept for
their side effects), Biome `noTsIgnore` (`@ts-expect-error` fails, TS2578,
where nothing is suppressed) and Ruff `I001` (moves comments with the
import block).

**Your configuration.** Ruff's `extend-safe-fixes` can make an unsafe fix
report as safe, and a Biome rule's `fix` option can set its kind. When a
`*.toml` file below the root mentions `extend-safe-fixes` (Ruff), or a
`biome.json`/`biome.jsonc` mentions `"fix"` (Biome), that tool's safe fixes
in the repository are dropped (`fix.config`); unsafe and suggestion fixes
stay. Biome's own fix kinds come from `biome explain` at the pinned
version (`src/collect_convert/biome-fix-kinds.tsv`); with any other Biome
version its fixes are dropped.

What the server does with the kept fixes (re-running the same tools,
checking digests, planning only reviewed safe fixes, never on `info`
findings unless the owner switches that on, verifying each step) is
described in [safety.md](safety.md#analyzer-fixes).

## When a tool refuses your configuration

golangci-lint 2 refuses a version-1 `.golangci.yml` (no `version: "2"`).
`collect` records it as `failed` with the reason
`golangci-lint refused the repository's configuration: it is a version-1
file, and golangci-lint 2 reads only version: "2" (convert it with
golangci-lint migrate); no report`, and writes no report. It never runs
golangci-lint without your configuration instead: that would change the
rules your project chose. (`golangci-lint-defects` still runs: it reads only the image's
configuration.) Run `golangci-lint migrate` in your repository
(golangci-lint's own command) to convert the file.

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
  analyzer timings; the C# compiler's results, which it writes in the
  order its concurrent analyzers finish, are sorted by file, line, column,
  rule and message; fixes are kept only in the byte-region carrier (see
  [Fixes](#fixes)), and the files and dropped fixes are listed in sorted
  order;
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
  `eslint-typescript`, `dependency-cruiser`,
  `sonaranalyzer-csharp`, `jscpd`, `detekt`, `clang-tidy`, `cppcheck`) are `missing`;
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
under their own licences (GPL-3.0 for golangci-lint, GPL-3.0-or-later for
Cppcheck, GPL-2.0-or-later for Pylint, LGPL for Opengrep, Semgrep and the
Sonar analyzers, GPL-2.0 for parts
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
| `golangci-lint-defects` | GPL-3.0-only | [golangci-lint v2.14.0 LICENSE](https://github.com/golangci/golangci-lint/blob/v2.14.0/LICENSE) |
| `oxlint` | MIT | [oxlint v1.88.0 LICENSE](https://github.com/oxc-project/oxc/blob/oxlint_v1.88.0/LICENSE) |
| `biome` | MIT OR Apache-2.0 | [LICENSE-MIT](https://github.com/biomejs/biome/blob/@biomejs/biome@2.5.15/LICENSE-MIT) and [LICENSE-APACHE](https://github.com/biomejs/biome/blob/@biomejs/biome@2.5.15/LICENSE-APACHE) at Biome 2.5.15 |
| `eslint-sonarjs` | LGPL-3.0-only | `package/LICENSE` in [eslint-plugin-sonarjs-2.0.4.tgz](https://registry.npmjs.org/eslint-plugin-sonarjs/-/eslint-plugin-sonarjs-2.0.4.tgz) |
| `eslint-typescript` | MIT | [ESLint v9.39.5 LICENSE](https://github.com/eslint/eslint/blob/v9.39.5/LICENSE) and [typescript-eslint v8.71.2 LICENSE](https://github.com/typescript-eslint/typescript-eslint/blob/v8.71.2/LICENSE) |
| `dependency-cruiser` | MIT | [dependency-cruiser v18.6.0 LICENSE](https://github.com/sverweij/dependency-cruiser/blob/v18.6.0/LICENSE) |
| `swiftlint` | MIT | [SwiftLint 0.65.1 LICENSE](https://github.com/realm/SwiftLint/blob/0.65.1/LICENSE) |
| `swiftlint-defects` | MIT | [SwiftLint 0.65.1 LICENSE](https://github.com/realm/SwiftLint/blob/0.65.1/LICENSE) |
| `sonaranalyzer-csharp` | LGPL-3.0-only | [sonar-dotnet 9.32.0.97167 LICENSE.txt](https://github.com/SonarSource/sonar-dotnet/blob/9.32.0.97167/LICENSE.txt), and the nuspec in [the package](https://api.nuget.org/v3-flatcontainer/sonaranalyzer.csharp/9.32.0.97167/sonaranalyzer.csharp.9.32.0.97167.nupkg) |
| `scorecard-license` | Apache-2.0 | [scorecard v5.6.0 LICENSE](https://github.com/ossf/scorecard/blob/v5.6.0/LICENSE) |
| `jscpd` | MIT | [jscpd v5.4.1 LICENSE](https://github.com/kucherenko/jscpd/blob/v5.4.1/LICENSE) |
| `phpcs` | BSD-3-Clause | [PHP_CodeSniffer 4.0.4 licence.txt](https://github.com/PHPCSStandards/PHP_CodeSniffer/blob/4.0.4/licence.txt) |
| `pmd` | BSD-3-Clause (PMD's BSD-style licence) | [PMD 7.28.0 LICENSE](https://github.com/pmd/pmd/blob/pmd_releases/7.28.0/LICENSE) |
| `detekt` | Apache-2.0 | [detekt v1.23.8 LICENSE](https://github.com/detekt/detekt/blob/v1.23.8/LICENSE) |
| `clang-tidy` | Apache-2.0 WITH LLVM-exception | [clang-tools-extra 19.1.0 LICENSE.TXT](https://github.com/llvm/llvm-project/blob/llvmorg-19.1.0/clang-tools-extra/LICENSE.TXT) |
| `cppcheck` | GPL-3.0-or-later (the image also holds its tinyxml2 library, Zlib) | [Cppcheck 2.17.1 COPYING](https://github.com/danmar/cppcheck/blob/2.17.1/COPYING), and the source headers' "version 3 of the License, or (at your option) any later version" |
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
