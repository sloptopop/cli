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
- An optional hosted service can turn a Report into a remediation plan.

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
