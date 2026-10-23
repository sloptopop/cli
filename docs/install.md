# Install

> **Skeleton.** The release artifacts are not published yet. The sections
> marked `TODO(L3)` are filled in by the packaging and release work. Until
> then, nothing on this page is a download link.

sloptopop comes as two programs:

| Program | What it does | Licence |
|---|---|---|
| `sloptopop` | The command-line tool you run: `check`, `fix`, `collect`, `transform`. Also the JSON-RPC server the language libraries talk to. | MIT OR Apache-2.0 (open source, this repository) |
| `sloptopop-local` | The open-source half on your machine: it prepares the masked snapshot, applies the server's verified changes and runs your checks. `sloptopop` starts it as a separate process for `check`, `fix`, `transform` and `local`. | MIT OR Apache-2.0 (open source) |

sloptopop detects on its server, `sloptopop-engine` (hosted, or
self-hosted from the same image); nothing proprietary is installed on your
machine. `sloptopop collect` works without `sloptopop-local`. `check`,
`fix` and `transform` need it and a server.

## Supported platforms

Planned release targets:

- macOS on Apple silicon (arm64) and Intel (x86_64);
- Linux on x86_64 and arm64.

`TODO(L3)`: confirm the list and the minimum OS versions.

## Install script

`TODO(L3)`: the command, the URL, and what the script does. It must:

- be POSIX `sh`;
- verify the checksum of every file it downloads before installing it;
- install `sloptopop` and `sloptopop-local` into the same directory.

```sh
# TODO(L3): install command
```

## Homebrew

`TODO(L3)`: the tap and the formula name.

```sh
# TODO(L3): brew tap … && brew install …
```

## `cargo install` (CLI only)

The open-source CLI builds from source with Cargo. This installs only
`sloptopop`, not `sloptopop-local`.

```sh
# TODO(L3): cargo install command (package name and version)
```

## Where `sloptopop-local` must be

`sloptopop` looks for `sloptopop-local` in this order and uses the first
match:

1. the path in the `SLOPTOPOP_LOCAL` environment variable;
2. `sloptopop-local` in the same directory as the `sloptopop` executable;
3. `sloptopop-local` in a directory on your `PATH`.

If none is found, `check`, `fix` and `transform` stop with exit code 3 and
this message:

```
error: sloptopop-local not found
hint: install sloptopop-local next to the sloptopop executable or on your PATH,
      or set SLOPTOPOP_LOCAL to its full path
```

## Verify the download

`TODO(L3)`: where the checksums, the signatures and the SBOM are published,
the public signing key, and the exact verification commands.

## Check the installation

```sh
sloptopop --version          # prints: sloptopop <version> (protocol 1)
sloptopop check .            # asks the server (hosted by default) for the Report
```

`sloptopop check` exits 0 when the repository passes, 1 when it does not,
and 3 when `sloptopop-local` is missing or fails. See
[troubleshooting.md](troubleshooting.md) if it exits 3.

## Analyzers: a container runtime

`sloptopop collect` runs every analyzer from a container image that
sloptopop built and pinned by digest; you do not install the analyzers.
You need one container runtime:

| Platform | Runtime (any one) |
|---|---|
| Linux | Docker Engine (rootful or rootless) or Podman |
| macOS | Colima, Podman (`podman machine start`), Rancher Desktop, or Docker Desktop |
| Windows | Podman, Rancher Desktop, or Docker Desktop (best effort) |

Docker Desktop needs a paid subscription for professional use in larger
organisations; the others are open source. `collect` finds `docker`, then
`podman`, on your `PATH` (`--runtime` or `SLOPTOPOP_CONTAINER_RUNTIME` picks
one). The first run fetches the images it needs (it prints each one, with
its size, before fetching it); later runs reuse them. Behind a company
mirror or proxy, set `SLOPTOPOP_REGISTRY`. See
[analyzers.md](analyzers.md) and [collect.md](collect.md).

## Updates

The current `sloptopop` and `sloptopop-local` contain no update check;
`sloptopop` makes one network call, to the sloptopop server, when you run
`check`, `fix` or `transform`. How updates will be delivered is to be
confirmed.

`TODO(L3)`: how to update with each install path above.

## Uninstall

Delete `sloptopop` and `sloptopop-local` from the directory you installed
them into. In each repository you ran sloptopop on, you can also delete
these generated directories:

- `.sloptopop/reports/` (analyzer reports);
- `.sloptopop/tools/` (left by sloptopop versions before the images; no
  longer used);
- `.sloptopop/transform/` (transformation reports).

The analyzer images, the vulnerability snapshots in your cache
(`~/.cache/sloptopop`) and the volumes `sloptopop-gomodcache` and
`sloptopop-dotnet-*` can be removed with your runtime
(`docker image rm`, `docker volume rm`).

Keep `.sloptopop/rules/` if you wrote your own Opengrep or Semgrep rules
there.

`TODO(L3)`: the uninstall command for each install path.
