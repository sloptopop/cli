# Architecture

```
your code ─► language library ─JSON-RPC─► sloptopop (open-source CLI) ──process──► sloptopop-local (open source)
             (Python, Node, Go, JVM,        stdio, see PROTOCOL.md       boundary      masked snapshot, applier,
              .NET, C++, PHP, PowerShell,          │                                     evidence of your checks
              shell, Rust, Swift)                  └──HTTPS (curl)──► sloptopop-engine (proprietary server:
                                                                       hosted, or self-hosted from the same image)
```

- **Language libraries** (open source, MIT OR Apache-2.0) are thin clients.
  Each starts the `sloptopop` binary as a child process and speaks JSON-RPC
  2.0 to it. They contain no analysis logic.
- **`sloptopop`** (open source, MIT OR Apache-2.0) is the command-line tool
  and the JSON-RPC server. It runs the open-source analyzers (`collect`),
  starts `sloptopop-local` as a separate process, and makes the HTTPS calls
  to the server (submit the job, poll its place in the queue, fetch and
  delete the answer). It never links either.
- **`sloptopop-local`** (open source, MIT OR Apache-2.0) prepares the
  snapshot the server analyses (the files of `HEAD`, every secret value
  gitleaks reports masked, the analyzer reports), applies the changes the
  server verified as commits on a new branch (checked against the server's
  digests, your real secret values put back), and runs your own type
  check, build and tests as evidence. It holds no detection logic.
- **`sloptopop-engine`** (proprietary) is the server: the Detector, the
  business-logic model, the Architect, the Implementor and the Guider run
  only there (decision D-0183), hosted by sloptopop or self-hosted from the
  same image. There is no detection without it. See
  [What leaves your machine](safety.md#what-leaves-your-machine).

Every component is deterministic: the same input produces the same output.

## `check`, `fix` and `transform`

```
sloptopop check [path] [--format text|json] [--backend <url>] [--yes] [--no-history]
sloptopop fix [path] [--apply] [--backend <url>] [--yes] [--no-history]
sloptopop transform [root] [--apply] [--branch <name>] [--backend <url>] [--yes] [--no-history] …
```

All three go the same way: `sloptopop-local request <root> --mode <mode>`
writes the request (refusing without a clean tree or a usable gitleaks
report), the CLI adds the branch's masked commit range read with `git`
(`history_git`, `history_mask`, `history_upload`; not with `--no-history`,
see [commands.md](commands.md#the-commit-range)), shows the upload
manifest, asks, and submits the request
to the server's job queue with `curl`, polls it while it waits and fetches
the answer (the only network calls, made by this open code);
then `sloptopop-local report` prints the server's
[Report](report-format.md) (`check`), or `sloptopop-local apply` delivers
its verified steps (`fix`: the files the mechanical fixes create;
`transform`: the planned transformations) and writes the run's report.
See [commands.md](commands.md).

`sloptopop-local` is located in this order:

1. the `SLOPTOPOP_LOCAL` environment variable (a path to the binary);
2. `sloptopop-local` next to the `sloptopop` executable;
3. `sloptopop-local` in a directory on `PATH`.

Exit code 3 means `sloptopop-local` is missing (the message says how to
install it), could not be started, failed, or printed something that is not
JSON; 4 that nothing was uploaded or no server answered. Usage errors exit 2.

## `collect`

```
sloptopop collect [root] [--only <tool,...>] [--rules <dir>] [--dry-run] [--offline] …
```

The server never runs an analyzer on your machine. `collect` runs the ones that apply from
a checked-in catalogue: each from its own container image (built by the
open-source `sloptopop/analyzer-images` repository and pinned by digest),
in a container runtime on your machine (Docker or Podman), with fixed
flags, without network, the repository read-only. It writes their SARIF
reports to `.sloptopop/reports/`, which `sloptopop-local` uploads with the
snapshot (without code snippets), and gitleaks' report is what masks your
secrets. The server re-runs the source-only tools from the same images and
its results win. See [collect.md](collect.md).

