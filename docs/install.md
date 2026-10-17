# Install

> **Skeleton.** The release artifacts are not published yet. The sections
> marked `TODO(L3)` are filled in by the packaging and release work. Until
> then, nothing on this page is a download link.

sloptopop comes as two programs:

| Program | What it does | Licence |
|---|---|---|
| `sloptopop` | The command-line tool you run: `check`, `fix`, `collect`, `transform`. Also the JSON-RPC server the language libraries talk to. | MIT OR Apache-2.0 (open source, this repository) |
| `sloptopop-engine` | The analysis engine. `sloptopop` starts it as a separate process for `check`, `fix` and `transform`. | Proprietary binary. Its licence terms are to be confirmed. |

`sloptopop collect` works without the engine. `check`, `fix` and
`transform` need it.

## Supported platforms

Planned release targets:

- macOS on Apple silicon (arm64) and Intel (x86_64);
- Linux on x86_64 and arm64.

`TODO(L3)`: confirm the list and the minimum OS versions.

## Install script

`TODO(L3)`: the command, the URL, and what the script does. It must:

- be POSIX `sh`;
- verify the checksum of every file it downloads before installing it;
- install `sloptopop` and `sloptopop-engine` into the same directory.

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
`sloptopop`, not the engine.

```sh
# TODO(L3): cargo install command (package name and version)
```

## Where the engine must be

`sloptopop` looks for `sloptopop-engine` in this order and uses the first
match:

1. the path in the `SLOPTOPOP_ENGINE` environment variable;
2. `sloptopop-engine` in the same directory as the `sloptopop` executable;
3. `sloptopop-engine` in a directory on your `PATH`.

If none is found, `check`, `fix` and `transform` stop with exit code 3 and
this message:

```
error: sloptopop-engine not found
hint: install sloptopop-engine next to the sloptopop executable or on your PATH,
      or set SLOPTOPOP_ENGINE to its full path
```

## Verify the download

`TODO(L3)`: where the checksums, the signatures and the SBOM are published,
the public signing key, and the exact verification commands.

## Check the installation

```sh
sloptopop --version          # prints: sloptopop <version> (protocol 1)
sloptopop check .            # runs the engine on the current directory
```

`sloptopop check` exits 0 when the repository passes, 1 when it does not,
and 3 when the engine is missing or fails. See
[troubleshooting.md](troubleshooting.md) if it exits 3.

## Analyzers (optional)

`sloptopop collect` runs open-source analyzers that you install yourself.
sloptopop never installs or downloads them for you. See
[analyzers.md](analyzers.md) for the list and their licences.

## Updates

The current `sloptopop` and `sloptopop-engine` contain no update check and
make no network call to sloptopop. How updates will be delivered is to be
confirmed.

`TODO(L3)`: how to update with each install path above.

## Uninstall

Delete `sloptopop` and `sloptopop-engine` from the directory you installed
them into. In each repository you ran sloptopop on, you can also delete
these generated directories:

- `.sloptopop/reports/` (analyzer reports);
- `.sloptopop/tools/` (managed analyzer packages);
- `.sloptopop/transform/` (transformation reports).

Keep `.sloptopop/rules/` if you wrote your own Opengrep or Semgrep rules
there.

`TODO(L3)`: the uninstall command for each install path.
