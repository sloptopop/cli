#!/bin/sh
# Builds the Linux release archive for this machine's architecture.
#   release/build.sh <out-dir> [<local-engine checkout>]
#   release/build.sh --context <dir> <out-dir>
# Run on a Docker host (a gate runner or the release workflow): the build
# runs in the Rust image pinned below (release/build-inner.sh). The first form takes `git archive HEAD` of this repository
# and of the local-engine commit in release/local-engine.ref (from the
# checkout given, default ../local-engine); the second a directory that
# already holds cli/ and local-engine/ exported that way. Writes to
# <out-dir>:
#   sloptopop-<target>.tar.gz          sloptopop-<version>-<target>/ with both
#                                      programs, the licences and the notices
#   sloptopop-<target>.tar.gz.sha256
set -eu
usage="usage: release/build.sh <out-dir> [local-engine checkout] | --context <dir> <out-dir>"
here=$(cd "$(dirname "$0")/.." && pwd)
ctx=$(mktemp -d)
trap 'rm -rf "$ctx"' EXIT
if [ "${1:-}" = --context ]; then
  given=$(cd "${2:?$usage}" && pwd)
  out=${3:?$usage}
  cp -R "$given/cli" "$given/local-engine" "$ctx/"
else
  out=${1:?$usage}
  le=${2:-$here/../local-engine}
  mkdir -p "$ctx/cli" "$ctx/local-engine"
  git -C "$here" archive HEAD | tar -xf - -C "$ctx/cli"
  git -C "$le" archive "$(cat "$here/release/local-engine.ref")" | tar -xf - -C "$ctx/local-engine"
fi
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ctx/cli/Cargo.toml" | head -1)
mkdir -p "$out"; out=$(cd "$out" && pwd)
image=rust:1.99.0-slim-bookworm@sha256:2c3a22f0a5533ea2dd5a16627bc841228151faa2d4de2644ac9987e4a2f1f2fa
cp "$here/release/build-inner.sh" "$ctx/build-inner.sh"
docker run --rm -v "$ctx:/build" -w /build "$image" \
  sh -c 'sh /build/build-inner.sh; s=$?; chown -R "$0" /build; exit $s' "$(id -u):$(id -g)"
target=$(cat "$ctx/bin/TARGET")
name=sloptopop-$version-$target
stage=$ctx/$name
mkdir -p "$stage"
cp "$ctx/bin/sloptopop" "$ctx/bin/sloptopop-local" "$stage/"
cp "$ctx/cli/LICENSE-MIT" "$ctx/cli/LICENSE-APACHE" "$ctx/cli/README.md" "$stage/"
cp "$ctx/cli/THIRD-PARTY-NOTICES.md" "$stage/THIRD-PARTY-NOTICES-sloptopop.md"
cp "$ctx/local-engine/THIRD-PARTY-NOTICES.md" "$stage/THIRD-PARTY-NOTICES-sloptopop-local.md"
touch -h -d @0 "$stage" "$stage"/*
asset=sloptopop-$target.tar.gz
(cd "$ctx" && tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner --format=ustar -cf - "$name") | gzip -n -9 > "$out/$asset"
(cd "$out" && sha256sum "$asset" > "$asset.sha256" && cat "$asset.sha256")
(cd "$ctx/bin" && sha256sum sloptopop sloptopop-local)
