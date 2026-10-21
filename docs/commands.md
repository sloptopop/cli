# Commands

Every command and option of `sloptopop`, and of the `sloptopop-engine`
binary it starts. The grammar is strict: anything not listed here is a
usage error (exit code 2).

```
sloptopop check [path] [--format text|json]
sloptopop fix [path] [--apply]
sloptopop collect [root] [--only <tool,...>] [--rules <dir>] [--dry-run] [--offline] …
sloptopop transform [root] [--apply] [--branch <name>] [--backend <url>] [--yes]
                    [--confirm <id,...> --by <name>] [--format text|json]   (preview)
sloptopop serve --stdio
sloptopop schema
sloptopop version | --version | -V
sloptopop help | --help | -h
```

`path` and `root` default to the current directory. A path may not start
with `-`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. For `check`: the repository passed. For `transform`: every automated step was applied (with `--apply`) or can be (dry run). |
| 1 | For `check`: the repository did not pass. For `transform`: a step is waiting for a confirmation, was refused, or is blocked by one that was. For `collect`: it could not write the reports directory, the rule pack, `.collect.json` or its output. For `serve`: the session ended without `shutdown`, or a framing error. |
| 2 | Usage error: an unknown command or option, a missing value, an unknown format or tool, a root or rules directory that does not exist. The message and the usage text go to stderr. |
| 3 | `collect`: no container runtime answered (nothing ran). `check`, `fix` and `transform`: the engine is missing, could not be started, failed, or printed something that is not JSON. For `transform` this includes the engine refusing to start: not the top of a git repository, uncommitted or untracked changes, `HEAD` moved since the request, a branch that exists, a confirmation that is not allowed. |
| 4 | `transform`: the upload was not confirmed, `curl` is not on `PATH`, or the backend could not be reached or refused the request. |

## `check`

```
sloptopop check [path] [--format text|json]
```

Checks the repository at `path` and prints the result.

- It starts `sloptopop-engine check <path>` and reads the
  [Report](report-format.md) it prints.
- The engine also reads the analyzer reports in `.sloptopop/reports/`, if
  there are any (see [`collect`](#collect)).
- It changes nothing in the repository.

| Option | Meaning |
|---|---|
| `--format text` | The default. A summary: the verdict, the findings grouped by severity (each with its message, its control, its mechanical fix if any, and how to fix it), then every control with its status. |
| `--format json` | The engine's Report, unchanged. See [report-format.md](report-format.md). |

Exit code: 0 if the repository passed, 1 if not, 3 if the engine failed.
The profile is `strict`, so every finding fails the repository. The rules
are listed in [rules.md](rules.md).

## `fix`

```
sloptopop fix [path] [--apply]
```

Creates missing files from templates: the mechanical fixes some findings
carry (see [rules.md](rules.md)).

- Without `--apply` it is a dry run. It lists the files it would create
  and writes nothing.
- With `--apply` it creates them.
- It never overwrites or edits a file that already exists. Such a file is
  listed as `exists`.

The templates today:

| Template | File created | For |
|---|---|---|
| `rust-toolchain` | `rust-toolchain.toml` | `toolchain.unpinned`, Rust only, when no pin file exists |
| `pre-push` | `.githooks/pre-push` | `hook.missing` |
| `gate-workflow` | `.github/workflows/gate.yml` | `ci.missing`, when the repository has no CI file at all |
| `gitignore` | `.gitignore` | `secrets.committed` on an environment file, when there is no `.gitignore` |

The hook and the workflow run `scripts/gate.sh`. You write that script
yourself: it is your project's quality gate.

Output, one line per file: `planned` (dry run), `created` (with
`--apply`) or `exists` (left alone). Exit code 0, or 3 if the engine
failed. The JSON the engine prints is described in
[report-format.md](report-format.md#fix-result).

## `collect`

```
sloptopop collect [root] [--only <tool,...>] [--rules <dir>] [--dry-run] [--offline]
                  [--registry <host[/path]>] [--runtime docker|podman]
                  [--timeout <seconds>] [--vuln-db <YYYYMMDD>] [--host-tools]
```

Runs the open-source analyzers that apply, each from the container image
sloptopop built and pinned by digest for this release, with fixed flags,
and writes their reports to `.sloptopop/reports/`. The next `check` reads
them. Every image (and every vulnerability snapshot) is announced before
it is fetched, then verified by digest. It needs a container runtime and
never falls back to the tools on your `PATH`.

| Option | Meaning |
|---|---|
| `--only <tool,...>` | Run only these catalogue entries. Repeatable. It never adds an entry whose ecosystem was not detected. An unknown name is a usage error. |
| `--rules <dir>` | Use only this directory of rules for Opengrep and Semgrep, instead of sloptopop's own rule pack plus `.sloptopop/rules/`. |
| `--dry-run` | Print the plan (images with version and size, command lines, outputs); fetch and run nothing. Nothing is written. |
| `--offline` | Fetch nothing; use the images and vulnerability snapshots already on this machine. |
| `--registry <host[/path]>` | Pull from this mirror or proxy of the component library (also `SLOPTOPOP_REGISTRY`). |
| `--runtime docker\|podman` | Use only this container runtime (also `SLOPTOPOP_CONTAINER_RUNTIME`). |
| `--timeout <seconds>` | The time limit of each tool (default 1800). |
| `--vuln-db <YYYYMMDD>` | Use that day's vulnerability snapshot instead of the newest. |
| `--host-tools` | Unsupported: run the tools on your `PATH`, unpinned, with a warning on every run. |
| `--container` | Accepted for compatibility (containers are the default); prints a note. |

Exit code 0 whatever the tools found or did; 3 if no container runtime
answers (then nothing runs, and the message says what to install). A tool
that fails is reported, and the others still run. The full reference, with
every tool, flag and licence, is [collect.md](collect.md). An overview by
language is in [analyzers.md](analyzers.md).

## `transform` (preview)

> **Preview.** `transform` works end to end and is tested, but names,
> options and file layouts can still change before release, and the
> hosted backend is not open yet (use `--backend` with a backend you run).

```
sloptopop transform [root] [--apply] [--branch <name>] [--backend <url>] [--yes]
                    [--confirm <id,...> --by <name>] [--format text|json]
```

Detects problems, plans automatic transformations for them with the
sloptopop backend, applies the ones that are verified on a new branch, and
writes a report. The transformations are listed in
[transformations.md](transformations.md).

What it does, in order:

1. **Prepare.** `sloptopop-engine transform-request` checks that `root` is
   the top of a git repository with a clean tree, runs the Detector (it
   also reads analyzer reports in `.sloptopop/reports/`), takes the files
   of the commit `HEAD` points to, masks secret values, and writes the
   request to `.sloptopop/transform/<run>/request.json`.
2. **Show and ask.** It prints what it will upload and where to stderr
   (the upload manifest: files, bytes, files left out and why, secret
   values masked) and asks `Upload? [y/N]`. `--yes` uploads without asking
   (for CI). Without a yes, nothing is sent (exit code 4).
3. **Upload.** It posts the request to `<backend>/v1/transform` with
   `curl`, the only network call sloptopop makes. The backend describes
   the code, plans, and verifies every automated step on the snapshot.
4. **Deliver.** `sloptopop-engine transform` checks that the answer
   belongs to the request and to `HEAD`, lowers each verified step again
   on this machine (the Guider checks it here too, and the step is used
   only if its bytes are exactly the ones the backend verified), and with
   `--apply` commits it: one commit per step, on top of the previous one,
   on a new branch. Then it runs the Detector again on the result.
5. **Report.** It writes `report.json`, `SUMMARY.md` and `PR.md` to
   `.sloptopop/transform/<run>/` and prints `SUMMARY.md` (or, with
   `--format json`, `report.json`).

| Option | Meaning |
|---|---|
| (none) | Dry run: plans and checks every step, commits nothing. |
| `--apply` | Commits the delivered steps on a new branch. |
| `--branch <name>` | The branch to create (default `sloptopop/transform-<run>`). It must not exist. |
| `--backend <url>` | The backend's base URL (default `https://api.sloptopop.dev`, not open yet). `http://` is accepted for a backend inside your own network. |
| `--yes` | Upload without asking. |
| `--confirm <id,...>` | Confirm these declared behaviour changes (`att-1,att-2`, as `SUMMARY.md` lists them) before delivering. Repeatable. Needs `--by`. |
| `--by <name>` | Who confirms. The name must be listed in `.sloptopop/context.json` (see below). |
| `--format text\|json` | Print `SUMMARY.md` (default) or `report.json`. |

### Declared behaviour changes need a person

Most transformations change behaviour on purpose (a failed write now
throws instead of passing silently). Each such step carries a statement,
and it is not committed until a person confirms it. A first run (dry run
or `--apply`) stops at the first such step and lists what to confirm:

```
## Waiting for confirmation
- `att-1` (step 2, approver): a failed write to orders now aborts the operation instead of continuing
Then run:
    sloptopop transform --apply --confirm att-1 --by <your name>
```

Who may confirm is listed in `.sloptopop/context.json`, which you write
(keep it out of the commit with `.sloptopop/` in `.gitignore`, or commit
it):

```json
{ "owners": { "approvers": ["lead"], "operators": ["ops"], "data": ["owner"] } }
```

An approver may confirm anything; an operator confirms deployment facts,
a data owner confirms facts about the data's consumers. A confirmation is
kept in `.sloptopop/transform/attestations.json` (never uploaded) and is
bound to the exact statement, the plan and the commit, so it never carries
over to a different plan or commit. Running again on the same commit
gives the same plan, so confirmations are reused.

### Safety rules

- It never touches your checkout: no change to your files, your index or
  `HEAD`. Commits are made with git's plumbing in a private index and put
  on the new branch only.
- It refuses to start if your working tree has uncommitted or untracked
  changes, or if `root` is not the top of the repository.
- It never merges, pushes or deploys. You review the branch and merge it
  yourself.
- A step that is not verified, here and in the backend, is not applied,
  and the steps that stack on it are not applied either. See
  [safety.md](safety.md).
- Commits carry the base commit's date and the author `sloptopop`, so the
  same run gives the same commits.

### The files it writes

`.sloptopop/transform/` holds its own `.gitignore` (`*`), so nothing in it
is committed or makes your tree dirty.

| File | Content |
|---|---|
| `<run>/request.json` | Exactly what was uploaded (secret values masked). |
| `<run>/response.json` | The backend's answer. |
| `<run>/report.json` | The machine-readable result, below. |
| `<run>/SUMMARY.md` | What changed and why, with the standard controls; every refusal with its reason; confirmations still needed; every manual task as a checklist; the re-detection. |
| `<run>/PR.md` | A ready pull-request description for the branch. |
| `attestations.json` | The confirmations you recorded. |

`report.json` (schema version 1):

| Member | Content |
|---|---|
| `schemaVersion`, `tool` | `1`; `{name, version}` |
| `run`, `mode`, `base` | the run id, `apply` or `dry-run`, the base commit |
| `branch`, `head` | the branch made and its last commit, or `null` |
| `plan` | `{id, steps, rounds, birAfter}` |
| `steps[]` | `{ordinal, kind, class, status, commit, files, reason, problems, attestations[] {id, text, who, confirmedBy}, manual[] {text, checklist}}`; `status` is `applied`, `planned` (dry run), `manual`, `pending` (waits for a confirmation), `refused` or `blocked` |
| `problems[]` | `{id, goal, control, trace, method, findings[] {rule, path, line}, step, status, redetection}`; `redetection` is `resolved`, `still detected`, `not applied` or `not re-checked` |
| `refused[]` | problems the backend refused and planned without: `{round, step, problems, findings, reason}` |
| `pending[]` | confirmations still needed: `{attestation, step, text, who}` |
| `redetection` | `{before, after, notRechecked, changes[] {rule, path, before, after}}` or `null`: the Detector's own findings on uploaded files, before and after; analyzer findings are counted in `notRechecked` (run `sloptopop collect` again) |
| `summary` | how many steps ended in each status |

## `serve`

```
sloptopop serve --stdio
```

Runs the JSON-RPC 2.0 server on stdin and stdout. The language libraries
start it; you do not normally run it yourself. The wire protocol is in
[PROTOCOL.md](../PROTOCOL.md). `serve` without `--stdio` is a usage
error.

## `schema`

Prints the protocol description (methods, error codes, JSON Schemas) as
JSON. The same document is [schema/protocol.json](../schema/protocol.json).

## `version`

`sloptopop version`, `sloptopop --version` and `sloptopop -V` print
`sloptopop <version> (protocol <n>)` and exit 0.

## `help`

`sloptopop help`, `sloptopop --help` and `sloptopop -h` print the usage
text and exit 0.

## Environment variables

| Variable | Used by | Meaning |
|---|---|---|
| `SLOPTOPOP_ENGINE` | `check`, `fix`, `transform` | Full path of the `sloptopop-engine` binary. When set and not empty, it is used as given. |
| `PATH` | `check`, `fix`, `transform`, `collect` | Searched for `sloptopop-engine` (after the directory of `sloptopop`), by `transform` for `curl`, by `collect` for `docker` and `podman` (and, under `--host-tools`, the analyzers). |
| `SLOPTOPOP_REGISTRY` | `collect` | The registry the analyzer images and vulnerability snapshots are pulled from (default `components.sloptopop.dev`); `--registry` wins. |
| `SLOPTOPOP_CONTAINER_RUNTIME` | `collect` | `docker` or `podman`: the only runtime to use; `--runtime` wins. |
| `SLOPTOPOP_COLLECT_MEMORY`, `SLOPTOPOP_COLLECT_CPUS` | `collect` | Each tool's memory limit (default `4g`) and an optional CPU cap. |
| `SLOPTOPOP_CACHE_DIR` | `collect` | Where vulnerability snapshots are unpacked (default `$XDG_CACHE_HOME/sloptopop`, `~/.cache/sloptopop` or `%LOCALAPPDATA%\\sloptopop\\cache`). |
| `GOPROXY`, `GOPRIVATE`, `GONOPROXY`, `GONOSUMDB`, `GONOSUMCHECK`, `GOINSECURE` | `collect` | Passed to the `go mod download` prepare step of Go projects. |
| `SLOPTOPOP_BIN` | language libraries | Path of the `sloptopop` binary a library starts (see [PROTOCOL.md](../PROTOCOL.md)). |

## The engine: `sloptopop-engine`

You do not normally run the engine yourself. `sloptopop` starts it with
these arguments and reads the JSON it prints on stdout.

```
sloptopop-engine check <path> [--profile strict]
sloptopop-engine fix <path> [--apply]
sloptopop-engine transform-request <root>                                          (preview)
sloptopop-engine transform <root> --run <id> [--apply] [--branch <name>]
                           [--confirm <id,...> --by <name>]                        (preview)
```

| Command | Prints | Exit code |
|---|---|---|
| `check <path>` | The [Report](report-format.md). `--profile` takes only `strict`, the default. | 0 passed, 1 not passed |
| `fix <path>` | The [fix result](report-format.md#fix-result). A dry run unless `--apply`. | 0 |
| `transform-request <root>` | `{schemaVersion, run, base, request, response, manifest}`: the run, the request and response paths (relative to the root), the upload manifest. | 0 |
| `transform <root> --run <id>` | `report.json`. | 0 every automated step delivered, 1 not |

The engine has no network code: `sloptopop transform` posts the request
the engine wrote and saves the answer where the engine reads it.

The path is required and may not start with `-`. A usage error exits 2.
Any other failure exits 3 with `error: …` on stderr.
