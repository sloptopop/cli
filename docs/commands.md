# Commands

Every command and option of `sloptopop`, and of the open-source
`sloptopop-local` binary it starts. The grammar is strict: anything not
listed here is a usage error (exit code 2).

sloptopop detects only on its server, `sloptopop-engine`: the hosted
service, or the same image self-hosted (`--backend <url>`). There is no
offline check. `check`, `fix` and `transform` all go the same way: run
[`collect`](#collect) first (its gitleaks report is what masks your
secrets), then each command prepares the masked snapshot with
`sloptopop-local`, shows what it uploads and asks, posts it, and prints or
applies the server's answer.

```
sloptopop check [path] [--format text|json] [--backend <url>] [--yes]
sloptopop fix [path] [--apply] [--backend <url>] [--yes]
sloptopop collect [root] [--only <tool,...>] [--rules <dir>] [--dry-run] [--offline] …
sloptopop transform [root] [--apply] [--branch <name>] [--backend <url>] [--yes]
                    [--confirm <id,...> --by <name>] [--format text|json]   (preview)
sloptopop local <arguments>
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
| 3 | `collect`: no container runtime answered (nothing ran). `check`, `fix`, `transform` and `local`: `sloptopop-local` is missing, could not be started, failed, or printed something that is not JSON. This includes it refusing: not the top of a git repository, uncommitted or untracked changes, no usable gitleaks report (run `collect`), `HEAD` moved since the request, a branch that exists, a confirmation that is not allowed. |
| 4 | `check`, `fix`, `transform`: the upload was not confirmed, `curl` is not on `PATH`, or no server answered or it refused the request (the message names the hosted service and the self-hosted option). |

## `check`

```
sloptopop check [path] [--format text|json] [--backend <url>] [--yes]
```

Checks the repository at `path` on the sloptopop server and prints the
server's [Report](report-format.md).

1. `sloptopop-local request <path> --mode check` takes the files of the
   commit `HEAD` points to (the tree must be clean), masks every secret
   value the gitleaks report in `.sloptopop/reports/` names (it refuses
   without a usable one: run `sloptopop collect`), and writes the request
   with the analyzer reports `collect` wrote.
2. It shows what it will upload and asks (`--yes` skips the question), and
   posts it to `<backend>/v1/transform` with `curl`.
3. The server checks that no secret arrived in the clear, re-runs the
   analyzers it can from the same pinned images, runs its Detector and
   answers the Report v2. `sloptopop-local report` prints it.

It changes nothing in the repository.

| Option | Meaning |
|---|---|
| `--format text` | The default. A summary: the verdict, the findings grouped by severity (each with its message, its control, its mechanical fix if any, and how to fix it), then every control with its status. |
| `--format json` | The server's Report, unchanged. See [report-format.md](report-format.md). |
| `--backend <url>` | The server (default the hosted `https://api.sloptopop.dev`); your self-hosted `sloptopop-engine` otherwise. |
| `--yes` | Upload without asking. |

Exit code: 0 if the repository passed, 1 if not, 3 if `sloptopop-local`
failed, 4 if nothing was uploaded or no server answered. The profile is
`strict`, so every finding fails the repository. The rules are listed in
[rules.md](rules.md).

## `fix`

```
sloptopop fix [path] [--apply] [--backend <url>] [--yes]
```

Creates missing files from templates: the mechanical fixes some findings
carry (see [rules.md](rules.md)). The server detects and returns one step
per missing file; `sloptopop-local apply` checks each against the server's
digests.

- Without `--apply` it is a dry run. It lists the files it would create
  and writes nothing.
- With `--apply` it commits each file as one commit on a new branch
  (`sloptopop/transform-<run>`); the checkout is never touched.
- It never overwrites or edits a file that already exists.

The templates today:

| Template | File created | For |
|---|---|---|
| `rust-toolchain` | `rust-toolchain.toml` | `toolchain.unpinned`, Rust only, when no pin file exists |
| `pre-push` | `.githooks/pre-push` | `hook.missing` |
| `gate-workflow` | `.github/workflows/gate.yml` | `ci.missing`, when the repository has no CI file at all |
| `gitignore` | `.gitignore` | `secrets.committed` on an environment file, when there is no `.gitignore` |

The hook and the workflow run `scripts/gate.sh`. You write that script
yourself: it is your project's quality gate.

It prints the run's `SUMMARY.md`. Exit code 0 when every fix was
delivered (or, in a dry run, can be), 1 when not, 3 or 4 as for `check`.

## `local`

```
sloptopop local <arguments>
```

Runs `sloptopop-local` with the arguments as they are, for what the other
commands do not cover, such as the evidence of your project's own checks
after a transform:

```
sloptopop local evidence . --run <id>
```

It runs your type check, build and tests on the base commit and on the
applied branch (from `.sloptopop/checks.json`, or what `package.json`,
Cargo or Go declare) and writes the evidence request; only exit codes,
durations and output digests go to the server, whose Guider says whether
the delivered steps keep your checks passing.

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
> hosted server is not open yet (use `--backend` with a server you run).

```
sloptopop transform [root] [--apply] [--branch <name>] [--backend <url>] [--yes]
                    [--confirm <id,...> --by <name>] [--format text|json]
```

Detects problems, plans automatic transformations for them with the
sloptopop backend, applies the ones that are verified on a new branch, and
writes a report. The transformations are listed in
[transformations.md](transformations.md).

What it does, in order:

1. **Prepare.** `sloptopop-local request <root> --mode transform` checks
   that `root` is the top of a git repository with a clean tree, takes
   the files of the commit `HEAD` points to, masks every secret value the
   gitleaks report names (it refuses without a usable one), and writes the
   request, with the analyzer reports, to
   `.sloptopop/transform/<run>/request.json`.
2. **Show and ask.** It prints what it will upload and where to stderr
   (the upload manifest: files, bytes, analyzer reports, files left out
   and why, secret values masked) and asks `Upload? [y/N]`. `--yes`
   uploads without asking (for CI). Without a yes, nothing is sent (exit
   code 4).
3. **Upload.** It posts the request to `<backend>/v1/transform` with
   `curl`, the only network call sloptopop makes. The server checks that no
   secret arrived in the clear, detects, describes the code, plans, and
   verifies every automated step on the snapshot.
4. **Deliver.** `sloptopop-local apply` checks that the answer belongs to
   the request and to `HEAD`, checks every change against the server's
   whole-file digests (a step is used only if its bytes are exactly the
   ones the server verified), puts your real secret values back, and with
   `--apply` commits it: one commit per step, on top of the previous one,
   on a new branch.
5. **Report.** It writes `report.json`, `SUMMARY.md` and `PR.md` to
   `.sloptopop/transform/<run>/` (with the server's re-detection) and
   prints `SUMMARY.md` (or, with `--format json`, `report.json`).

| Option | Meaning |
|---|---|
| (none) | Dry run: plans and checks every step, commits nothing. |
| `--apply` | Commits the delivered steps on a new branch. |
| `--branch <name>` | The branch to create (default `sloptopop/transform-<run>`). It must not exist. |
| `--backend <url>` | The server's base URL (default `https://api.sloptopop.dev`, not open yet). `http://` is accepted for a server inside your own network. |
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

`report.json` (schema version 2, written by `sloptopop-local`):

| Member | Content |
|---|---|
| `schemaVersion`, `tool` | `2`; `{name, version}` (`sloptopop-local`) |
| `run`, `plan`, `base` | the run id, the plan id, the base commit |
| `applied` | whether the steps were committed (`--apply`) or only checked (dry run) |
| `branch`, `head` | the branch made and its last commit, or `null` |
| `complete` | whether no step is pending, refused or blocked |
| `steps[]` | `{ordinal, kind, class, title, status, commit, files, reason}`; `status` is `applied`, `planned` (dry run), `manual`, `pending` (waits for a confirmation), `refused` or `blocked` |
| `redetected` | the server's re-detection, `{rules[] {rule, before, after}}` (the rules whose counts differ, the Detector's own rules only), or `null` |

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
| `SLOPTOPOP_LOCAL` | `check`, `fix`, `transform`, `local` | Full path of the `sloptopop-local` binary. When set and not empty, it is used as given. |
| `PATH` | `check`, `fix`, `transform`, `local`, `collect` | Searched for `sloptopop-local` (after the directory of `sloptopop`), for `curl`, by `collect` for `docker` and `podman` (and, under `--host-tools`, the analyzers). |
| `SLOPTOPOP_REGISTRY` | `collect` | The registry the analyzer images and vulnerability snapshots are pulled from (default `components.sloptopop.dev`); `--registry` wins. |
| `SLOPTOPOP_CONTAINER_RUNTIME` | `collect` | `docker` or `podman`: the only runtime to use; `--runtime` wins. |
| `SLOPTOPOP_COLLECT_MEMORY`, `SLOPTOPOP_COLLECT_CPUS` | `collect` | Each tool's memory limit (default `4g`) and an optional CPU cap. |
| `SLOPTOPOP_CACHE_DIR` | `collect` | Where vulnerability snapshots are unpacked (default `$XDG_CACHE_HOME/sloptopop`, `~/.cache/sloptopop` or `%LOCALAPPDATA%\\sloptopop\\cache`). |
| `GOPROXY`, `GOPRIVATE`, `GONOPROXY`, `GONOSUMDB`, `GONOSUMCHECK`, `GOINSECURE` | `collect` | Passed to the `go mod download` prepare step of Go projects. |
| `SLOPTOPOP_BIN` | language libraries | Path of the `sloptopop` binary a library starts (see [PROTOCOL.md](../PROTOCOL.md)). |

## `sloptopop-local`

You do not normally run `sloptopop-local` yourself (see [`local`](#local)).
`sloptopop` starts it with these arguments and reads the JSON it prints on
stdout. It is open source (MIT OR Apache-2.0) and holds no detection logic.

```
sloptopop-local request <root> --mode check|fix|transform
sloptopop-local report <root> --run <id>
sloptopop-local apply <root> --run <id> [--apply] [--branch <name>] [--confirm <id,...> --by <name>]
sloptopop-local evidence <root> --run <id>
sloptopop-local version
```

| Command | Prints | Exit code |
|---|---|---|
| `request` | `{schemaVersion, run, base, request, response, manifest}`: the run, the request and response paths (relative to the root), the upload manifest. | 0 |
| `report` | The server's [Report](report-format.md). | 0 passed, 1 not passed |
| `apply` | `report.json`. | 0 every automated step delivered, 1 not |
| `evidence` | `{schemaVersion, run, base, request, response, checks}`: the evidence request and response paths. | 0 |

It has no network code: `sloptopop` posts the request it wrote and saves
the answer where it reads it. A usage error exits 2. Any other failure
exits 3 with `error: …` on stderr.
