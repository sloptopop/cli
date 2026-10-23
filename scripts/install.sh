#!/bin/sh
# Installs sloptopop and sloptopop-local from a sloptopop release.
#
#   curl -fsSL https://github.com/sloptopop/cli/releases/latest/download/install.sh | sh
#
# It picks the archive for this machine (Linux or macOS, x86_64 or arm64),
# downloads it and the release's SHA256SUMS, refuses to install unless the
# archive's SHA-256 matches, and installs both programs into one directory.
# When cosign is on PATH it also verifies SHA256SUMS against its Sigstore
# bundle (keyless, signed by the release workflow of github.com/sloptopop/cli).
#
# Settings (environment variables):
#   SLOPTOPOP_VERSION        a release such as 0.1.0 (default: the latest)
#   SLOPTOPOP_INSTALL_DIR    where to install (default: $HOME/.local/bin)
#   SLOPTOPOP_DOWNLOAD_URL   the release directory to download from (https://
#                            or file://; default: the GitHub release)
#   SLOPTOPOP_REQUIRE_SIGNATURE=1   fail unless cosign verifies the signature
#
# Exit codes: 0 installed; 1 a download, checksum or signature failed;
# 2 a usage or platform problem.
set -eu

repo=https://github.com/sloptopop/cli
identity="^https://github\.com/sloptopop/cli/\.github/workflows/release\.yml@refs/tags/v"
issuer=https://token.actions.githubusercontent.com

say() { printf 'sloptopop install: %s\n' "$*" >&2; }
fail() { say "error: $1"; exit "${2:-1}"; }

case $(uname -s) in
  Linux) os=unknown-linux-musl ;;
  Darwin) os=apple-darwin ;;
  *) fail "unsupported system $(uname -s) (Linux and macOS only)" 2 ;;
esac
case $(uname -m) in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *) fail "unsupported architecture $(uname -m) (x86_64 and arm64 only)" 2 ;;
esac
target=$arch-$os
archive=sloptopop-$target.tar.gz

if [ -n "${SLOPTOPOP_DOWNLOAD_URL:-}" ]; then
  base=${SLOPTOPOP_DOWNLOAD_URL%/}
elif [ -n "${SLOPTOPOP_VERSION:-}" ]; then
  base=$repo/releases/download/v${SLOPTOPOP_VERSION#v}
else
  base=$repo/releases/latest/download
fi
case $base in
  https://* | file://*) ;;
  *) fail "SLOPTOPOP_DOWNLOAD_URL must start with https:// or file://" 2 ;;
esac
dir=${SLOPTOPOP_INSTALL_DIR:-${HOME:?set HOME or SLOPTOPOP_INSTALL_DIR}/.local/bin}

command -v curl > /dev/null 2>&1 || fail "curl not found (sloptopop also needs it to talk to its server)" 2
if command -v sha256sum > /dev/null 2>&1; then
  sha() { sha256sum "$1" | cut -d ' ' -f 1; }
elif command -v shasum > /dev/null 2>&1; then
  sha() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
else
  fail "neither sha256sum nor shasum found: cannot verify the download" 2
fi

tmp=$(mktemp -d 2> /dev/null || mktemp -d -t sloptopop)
trap 'rm -rf "$tmp"' EXIT INT TERM
fetch() { curl -fsSL --proto '=https,file' --tlsv1.2 --retry 3 -o "$tmp/$1" "$base/$1" || fail "could not download $base/$1"; }

say "downloading $archive from $base"
fetch SHA256SUMS
fetch "$archive"

if command -v cosign > /dev/null 2>&1; then
  fetch SHA256SUMS.sigstore.json
  cosign verify-blob --bundle "$tmp/SHA256SUMS.sigstore.json" \
    --certificate-identity-regexp "$identity" --certificate-oidc-issuer "$issuer" \
    "$tmp/SHA256SUMS" > /dev/null 2>&1 || fail "the signature of SHA256SUMS does not verify"
  say "signature of SHA256SUMS verified (Sigstore, $repo release workflow)"
elif [ "${SLOPTOPOP_REQUIRE_SIGNATURE:-0}" = 1 ]; then
  fail "SLOPTOPOP_REQUIRE_SIGNATURE=1 but cosign is not on PATH" 2
else
  say "cosign not found: checking the SHA-256 only (see docs/install.md to verify the signature)"
fi

want=$(awk -v f="$archive" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")
[ -n "$want" ] || fail "$archive is not listed in SHA256SUMS"
got=$(sha "$tmp/$archive")
[ "$got" = "$want" ] || fail "checksum mismatch for $archive (expected $want, got $got): nothing installed"
say "checksum ok: $got"

mkdir -p "$tmp/x"
tar -xzf "$tmp/$archive" -C "$tmp/x"
src=$(find "$tmp/x" -type f -name sloptopop-local | head -n 1)
[ -n "$src" ] || fail "the archive holds no sloptopop-local"
src=$(dirname "$src")
[ -f "$src/sloptopop" ] || fail "the archive holds no sloptopop"
mkdir -p "$dir"
for program in sloptopop sloptopop-local; do
  cp "$src/$program" "$dir/$program.new"
  chmod 0755 "$dir/$program.new"
  mv -f "$dir/$program.new" "$dir/$program"
done
say "installed $("$dir/sloptopop" --version) and $("$dir/sloptopop-local" --version) in $dir"
case :${PATH:-}: in
  *:"$dir":*) ;;
  *) say "add $dir to your PATH, e.g.: export PATH=\"$dir:\$PATH\"" ;;
esac
