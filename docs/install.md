# Install

sloptopop comes as two programs:

| Program | What it does | Licence |
|---|---|---|
| `sloptopop` | The command-line tool you run: `check`, `fix`, `collect`, `transform`. Also the JSON-RPC server the language libraries talk to. | MIT OR Apache-2.0 (open source, this repository) |
| `sloptopop-local` | The open-source half on your machine: it prepares the masked snapshot, applies the server's verified changes and runs your checks. `sloptopop` starts it as a separate process for `check`, `fix`, `transform` and `local`. | MIT OR Apache-2.0 (open source, [sloptopop/local-engine](https://github.com/sloptopop/local-engine)) |

sloptopop detects on its server, `sloptopop-engine` (hosted at
`https://api.sloptopop.dev`, or self-hosted from the same image); nothing
proprietary is installed on your machine. `sloptopop collect` works
without `sloptopop-local`. `check`, `fix` and `transform` need it, `curl`
and a server.

Every release archive holds both programs, built from the commits the
release names, with their licences and third-party notices.

## Supported platforms

| System | Architectures | Archive |
|---|---|---|
| Linux (any distribution; static musl binaries) | x86_64, arm64 | `sloptopop-x86_64-unknown-linux-musl.tar.gz`, `sloptopop-aarch64-unknown-linux-musl.tar.gz` |
| macOS 11 or later | Apple silicon (arm64), Intel (x86_64) | `sloptopop-aarch64-apple-darwin.tar.gz`, `sloptopop-x86_64-apple-darwin.tar.gz` |

Windows has no release archive yet.

## Install script

```sh
curl -fsSL https://github.com/sloptopop/cli/releases/latest/download/install.sh | sh
```

The script (POSIX `sh`, [scripts/install.sh](../scripts/install.sh)):

1. picks the archive for your system and architecture;
2. downloads it and the release's `SHA256SUMS`;
3. refuses to install unless the archive's SHA-256 matches its line in
   `SHA256SUMS`;
4. when `cosign` is on your `PATH`, also verifies the signature of
   `SHA256SUMS` (see [Verify the download](#verify-the-download));
5. installs `sloptopop` and `sloptopop-local` into the same directory,
   `~/.local/bin` by default, and says if that directory is not on your
   `PATH`.

| Variable | Meaning |
|---|---|
| `SLOPTOPOP_VERSION` | Install this release (e.g. `0.1.0`) instead of the latest. |
| `SLOPTOPOP_INSTALL_DIR` | Install into this directory. |
| `SLOPTOPOP_DOWNLOAD_URL` | Download from this release directory (`https://` or `file://`), e.g. a mirror. |
| `SLOPTOPOP_REQUIRE_SIGNATURE=1` | Fail unless `cosign` verifies the signature. |

To read the script before running it:

```sh
curl -fsSLO https://github.com/sloptopop/cli/releases/latest/download/install.sh
less install.sh
sh install.sh
```

## Homebrew (macOS and Linux)

```sh
brew install sloptopop/tap/sloptopop
```

The formula ([sloptopop/homebrew-tap](https://github.com/sloptopop/homebrew-tap))
installs both programs from the same release archives and checks their
SHA-256.

## `cargo install` (CLI only): not available yet

Installing from source with Cargo (`cargo install --git
https://github.com/sloptopop/cli`, or from crates.io) does not work yet:
every sloptopop crate's build script runs its quality gate (all tests with
100% coverage) on every build, and that includes a `cargo install`. A
change that skips the gate for sources Cargo unpacked itself is waiting on
a policy decision. Until then, use the install script or Homebrew, or
build a checkout as a developer does (see [CONTRIBUTING.md](../CONTRIBUTING.md)).
`cargo install` would install only `sloptopop`, not `sloptopop-local`.

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

Every release publishes, next to the archives:

| File | What it is |
|---|---|
| `SHA256SUMS` | The SHA-256 of every archive, `install.sh` and both SBOMs. |
| `SHA256SUMS.sigstore.json` | A [Sigstore](https://www.sigstore.dev/) bundle: the keyless signature of `SHA256SUMS` by the release workflow of `github.com/sloptopop/cli`, with its certificate and transparency-log entry. There is no long-lived signing key. |
| `sloptopop.cdx.json`, `sloptopop-local.cdx.json` | The CycloneDX SBOMs of both programs (every crate and its version). |

By hand, for release `v0.1.0`:

```sh
v=v0.1.0; base=https://github.com/sloptopop/cli/releases/download/$v
curl -fsSLO "$base/SHA256SUMS" -fsSLO "$base/SHA256SUMS.sigstore.json" \
     -fsSLO "$base/sloptopop-aarch64-apple-darwin.tar.gz"
cosign verify-blob --bundle SHA256SUMS.sigstore.json \
  --certificate-identity "https://github.com/sloptopop/cli/.github/workflows/release.yml@refs/tags/$v" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com SHA256SUMS
shasum -a 256 --ignore-missing -c SHA256SUMS    # Linux: sha256sum --ignore-missing -c SHA256SUMS
```

`cosign` is open source (Apache-2.0). The Linux archives are reproducible:
building the release's commits with `release/build.sh` on any x86_64 or
arm64 Docker host gives the same bytes as the published archive.

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

`sloptopop` and `sloptopop-local` contain no update check and no
telemetry. Only `check`, `fix` and `transform` go online, and only to the
sloptopop server (the hosted one or your `--backend`), with `curl`:

- one `POST /v1/jobs` with the request you confirmed (the files and
  reports the manifest lists);
- then `GET /v1/jobs/<id>` for the job's state, every 2 seconds at first,
  backing off to every 5, until the answer is there; a poll that fails is
  tried again up to 5 times, waiting 2 to 20 seconds;
- one `DELETE /v1/jobs/<id>` once the answer is fetched, or when you stop
  `sloptopop` with Ctrl-C.

The polls and the `DELETE` send nothing but the job's id, which the
server drew at random ([commands.md](commands.md#waiting-in-the-queue)).
To update:

| Installed with | Update |
|---|---|
| the install script | run it again (it replaces both programs) |
| Homebrew | `brew upgrade sloptopop` |

## Uninstall

| Installed with | Uninstall |
|---|---|
| the install script | `rm ~/.local/bin/sloptopop ~/.local/bin/sloptopop-local` (or your `SLOPTOPOP_INSTALL_DIR`) |
| Homebrew | `brew uninstall sloptopop` (and `brew untap sloptopop/tap`) |
 In each repository you ran sloptopop on, you can also delete
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
