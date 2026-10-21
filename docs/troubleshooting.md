# Troubleshooting

Find the message you see, then follow the steps.

## Exit codes at a glance

| Code | Meaning | Section |
|---|---|---|
| 1 | `check`: the repository did not pass. Not an error: see the findings. | [rules.md](rules.md) |
| 2 | Usage error. | [Usage errors](#usage-errors-exit-code-2) |
| 3 | The engine is missing, could not start, failed, or printed invalid output. | [Engine problems](#engine-problems-exit-code-3) |

## Engine problems (exit code 3)

### `error: sloptopop-engine not found`

`sloptopop` could not find the engine. It looks, in order, at:

1. the `SLOPTOPOP_ENGINE` environment variable;
2. the directory of the `sloptopop` executable;
3. your `PATH`.

Fix it in one of these ways:

- install `sloptopop-engine` next to `sloptopop` (see
  [install.md](install.md));
- put its directory on your `PATH`;
- set `SLOPTOPOP_ENGINE` to its full path.

Check with `command -v sloptopop sloptopop-engine`.

### `error: cannot run <path>: …`

The engine was found but could not start. Common causes:

- the file is not executable: `chmod +x <path>`;
- it was built for another platform or CPU (for example an x86_64 binary
  on arm64 Linux): install the right build;
- on macOS, the system blocked a downloaded binary: check the file's
  signature and quarantine status before allowing it.

If `SLOPTOPOP_ENGINE` is set, it is used as given, even if the file does
not exist. Unset it or correct it.

### `error: sloptopop-engine failed (…)`

The engine exited with an unexpected status. Its own message is printed
just above this line (`error: …`). The usual cause is a path that does not
exist or cannot be read. Check the path you passed to `check` or `fix`.

### `error: sloptopop-engine printed invalid JSON: …`

`sloptopop` and `sloptopop-engine` probably come from different releases.
Install matching versions of both. `sloptopop --version` shows the CLI's
version.

## Usage errors (exit code 2)

The message is followed by the usage text. Common ones:

| Message | Fix |
|---|---|
| `no command given` | Add a command: `sloptopop check`. |
| `unrecognised arguments: …` | An option the command does not take, or options in the wrong place. See [commands.md](commands.md). |
| `unrecognised option: --x` | A path cannot start with `-`. Use `./-x` for such a path. |
| `unknown format: … (expected text or json)` | Use `--format text` or `--format json`. |
| `serve needs a transport: sloptopop serve --stdio` | Add `--stdio`. |
| `--only needs a value`, `--rules needs a value` | Give the value after the option, as a separate argument. |
| `--only has an empty tool name` | Remove the extra comma in the list. |
| `unknown tool: … (docs/collect.md lists them)` | Use a name from the catalogue in [collect.md](collect.md#the-catalogue). |

## `check` reports something unexpected

### A finding is a false positive

Check what the rule really looks for in [rules.md](rules.md). Some common
cases:

- **A committed `.env` file is reported although `.gitignore` lists it.**
  git keeps tracking a file once it is committed, whatever `.gitignore`
  says. Untrack it with `git rm --cached <file>`, keep it ignored, and
  rotate its credentials.
- **A test or fixture is reported.** Most source rules skip test code and
  fixtures. Layout rules skip directories named `testdata`, `fixtures`,
  `__fixtures__`, `test-fixtures` and `snapshots`.

If the finding is still wrong, report it with a small synthetic example.
Never include a real secret.

### An analyzer's results do not appear

- Run `sloptopop collect` before `sloptopop check`. `check` only reads
  what is in `.sloptopop/reports/`.
- Run `sloptopop collect --dry-run` and look at the tool's line:
  `missing` means it is not on your `PATH`; `skipped` gives the reason.
- Results about files that git ignores, that are generated, or that are
  outside the repository are kept as metadata in the Report, not as
  findings.
- Results at SARIF level `note`, and suppressed results, are not findings.
  They are counted in the Report's `analysis.observations`.

### `analyzer.unreadable-report`

A file in `.sloptopop/reports/` has a report name (`*.sarif`, `*.lcov`, …)
but cannot be read: too large (over 32 MiB), not UTF-8, or not a valid
report. Regenerate it with the tool's documented command, or delete it.

### `analyzer.reports-unignored`

Git does not ignore `.sloptopop/reports/`. Run `sloptopop collect` once
(it adds the ignore file), or add `.sloptopop/` to your root `.gitignore`.

## `collect` problems

### `collect` exits 3: no container runtime

Every analyzer runs in a pinned container image, so `collect` needs Docker
Engine, Podman, Colima, Rancher Desktop or Docker Desktop. The error lists
what it tried:

- `docker (not on PATH)`: install one of them (see
  [install.md](install.md#analyzers-a-container-runtime)).
- `on PATH, but … info failed: Cannot connect to the Docker daemon`: start
  it (Colima: `colima start`; Podman on macOS and Windows: `podman machine
  start`; Linux: `systemctl start docker`, or rootless `systemctl --user
  start docker`).
- To use only one runtime, set `SLOPTOPOP_CONTAINER_RUNTIME=podman` (or
  pass `--runtime podman`).

`collect` never falls back to the tools on your `PATH`. Where no runtime
can run, `--host-tools` uses your own tools instead (unpinned,
unsupported).

### A tool `failed`

The reason is the first line of the tool's error output, or one of these:

- **`pull failed: …`:** the image could not be fetched. Check your network
  or proxy; behind a company mirror set `SLOPTOPOP_REGISTRY`; `docker login`
  to the mirror if it needs credentials.
- **`image not in the local store, and --offline forbids pulling it`:** run
  once without `--offline`, or load the release archive (see
  [collect.md](collect.md#mirrors-proxies-and-air-gapped-machines)).
- **`image digest mismatch` or `image labels do not match the catalogue`:**
  the registry or store gave something other than the pinned image; nothing
  ran from it. Check the mirror.
- **`cannot fetch …db-osv-…`:** the vulnerability snapshot could not be
  fetched; OSV-Scanner did not run. `--offline` uses the newest one cached.
- **`prepare step (go mod download) failed`** or **`(dotnet restore)`:** the
  project's dependencies could not be downloaded; usually a private module
  or feed that needs credentials, which `collect` never passes into a
  container.
- **`stopped after 1800 s`:** the tool ran past its time limit; raise it
  with `--timeout`.
- A tool killed for memory: raise `SLOPTOPOP_COLLECT_MEMORY` (default
  `4g`).

A failed tool leaves no report, never stops the others, and `collect`
still exits 0.

### A tool is `missing` under `--host-tools`

It is not on your `PATH`, or it exists only as an image. Without
`--host-tools` it runs from its image.

### `collect` exits 1

It could not write `.sloptopop/reports/`, the rule pack or
`.sloptopop/reports/.collect.json`. Check that the repository directory is
writable.

### A warning that a snapshot is old

The component library (or your mirror) has not been updated for more than
a week, so vulnerabilities published since are not found. Update the
mirror, or pass `--vuln-db <YYYYMMDD>` deliberately.

## `fix` problems

### A file is listed as `exists`

`fix` never overwrites an existing file. Edit that file by hand, using the
finding's advice.

### The created hook or workflow fails

Both run `scripts/gate.sh`, your project's own quality gate. Create that
script. Enable the hook with `git config core.hooksPath .githooks`.

## `transform` problems (preview)

`transform` is in preview, so this section is short and may change.

- **It refuses to start on a dirty tree.** Commit or stash your changes
  first, untracked files included. It never works on top of uncommitted
  changes. Keep `.sloptopop/` in `.gitignore` so analyzer reports and
  `.sloptopop/context.json` do not count.
- **Exit code 4.** You did not confirm the upload (pass `--yes` in CI),
  `curl` is not on `PATH`, or the backend could not be reached or refused
  the request; the message says which.
- **Steps are `pending`.** A declared behaviour change waits for a
  person: confirm it with `--confirm <id> --by <name>`, as `SUMMARY.md`
  shows. `--by` must be listed in `.sloptopop/context.json` `owners`.
- **"HEAD moved since the request was made".** You committed between the
  upload and the delivery. Run `sloptopop transform` again.
- **"branch … exists".** A previous run made it. Delete it or pass
  `--branch <name>`.
- **A step was refused.** The transformation report names the check that
  failed. Fix that finding by hand, or report the refusal if you think it
  is wrong. See [safety.md](safety.md#refusals).
- **A finding was left manual.** The reason code is explained in
  [transformations.md](transformations.md).
- **After merging, the program stops with `<KEY> is not set`.** A secrets
  transformation now reads the value from the environment. Set the
  variable in every environment that runs the program.
- **After merging, a request that used to succeed now fails.** An
  unchecked write or a swallowed error is now reported instead of
  ignored. The failure was there before; it was silent. Look at the logged
  or thrown error.
