# Architecture

```
your code ─► language library ─JSON-RPC─► sloptopop (open-source CLI) ─Report JSON─► sloptopop-engine
             (Python, Node, Go, JVM,        stdio, see PROTOCOL.md          process     (proprietary,
              .NET, C++, PHP, PowerShell,                                   boundary     runs locally,
              shell, Rust, Swift)                                                        offline)
```

- **Language libraries** (open source, MIT OR Apache-2.0) are thin clients.
  Each starts the `sloptopop` binary as a child process and speaks JSON-RPC
  2.0 to it. They contain no analysis logic.
- **`sloptopop`** (open source, MIT OR Apache-2.0) is the command-line tool
  and the JSON-RPC server. For `check` and `fix` it starts the
  `sloptopop-engine` binary as a separate process and exchanges the
  [Report](report-format.md) with it. It never links the engine.
- **`sloptopop-engine`** is proprietary and distributed as a binary. It runs
  locally and offline, and is light enough for small machines and CI
  containers.
- **The sloptopop backend** (proprietary) describes the code and plans the
  transformations for `sloptopop transform` (preview). `check`, `fix` and
  `collect` never use it. Where it runs is to be confirmed: see
  [What leaves your machine](safety.md#what-leaves-your-machine).

Every component is deterministic: the same input produces the same output.

## `check` and `fix`

```
sloptopop check [path] [--format text|json]
sloptopop fix [path] [--apply]
```

`path` defaults to the current directory. Both commands start
`sloptopop-engine` as a child process (`sloptopop-engine check <path>` or
`sloptopop-engine fix <path> [--apply]`), read the JSON document it prints
on stdout, and forward anything it writes to stderr.

- `check --format json` prints the engine's [Report](report-format.md)
  unchanged; `--format text` (the default) prints a summary: the verdict,
  findings grouped by severity with their remediation, and the controls.
  The exit code is the engine's: 0 if the repository passed, 1 if not.
- `fix` shows the files that mechanical fixes would create; with
  `--apply` the engine creates them. Existing files are never touched.

The engine is located in this order:

1. the `SLOPTOPOP_ENGINE` environment variable (a path to the binary);
2. `sloptopop-engine` next to the `sloptopop` executable;
3. `sloptopop-engine` in a directory on `PATH`.

Exit code 3 means the engine is missing (the message says how to install
it), could not be started, failed, or printed something that is not
JSON. Usage errors exit 2.

## `collect`

```
sloptopop collect [root] [--container] [--only <tool,...>] [--rules <dir>] [--dry-run]
```

The engine never runs an analyzer. `collect` runs the installed ones from
a checked-in catalogue with fixed flags, as separate processes, and writes
their SARIF reports to `.sloptopop/reports/`, which `check` hands to the
engine to read. See [collect.md](collect.md).

## `transform` (preview)

```
sloptopop transform [root] [--apply] [--branch <name>] [--backend <url>] [--yes]
                    [--confirm <id,...> --by <name>] [--format text|json]
```

`transform` starts the engine twice. `sloptopop-engine transform-request`
runs the Detector, takes the snapshot of `HEAD` with secret values masked
and writes the request; the CLI shows the upload manifest, asks, and
posts the request to the backend with `curl` (the only network call, made
by this open code); `sloptopop-engine transform` checks the answer,
delivers the verified steps (one commit each, on a new branch, with
`--apply`) and writes the report. See
[commands.md](commands.md#transform-preview),
[transformations.md](transformations.md) and [safety.md](safety.md).
