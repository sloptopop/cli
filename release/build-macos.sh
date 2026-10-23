#!/bin/sh
# Builds the macOS release archives (arm64 and x86_64) on a macOS host with
# rustup: the release workflow's GitHub-hosted macOS runner, never a laptop.
#   release/build-macos.sh <out-dir> [<local-engine checkout>]
# Same archive layout as release/build.sh (sloptopop-<target>.tar.gz). The
# minimum macOS is 11.0 on both architectures.
set -eu
out=${1:?usage: release/build-macos.sh <out-dir> [local-engine checkout]}
here=$(cd "$(dirname "$0")/.." && pwd)
le=$(cd "${2:-$here/../local-engine}" && pwd)
ref=$(cat "$here/release/local-engine.ref")
[ "$(git -C "$le" rev-parse HEAD)" = "$ref" ] || { echo "build-macos: $le is not at $ref" >&2; exit 65; }
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$here/Cargo.toml" | head -1)
mkdir -p "$out"; out=$(cd "$out" && pwd)
export CARGO_LLVM_COV=1 SOURCE_DATE_EPOCH=0 MACOSX_DEPLOYMENT_TARGET=11.0 CARGO_INCREMENTAL=0 \
  CARGO_PROFILE_RELEASE_LTO=true CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  CARGO_PROFILE_RELEASE_STRIP=symbols CARGO_PROFILE_RELEASE_PANIC=abort
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  for crate in "$here" "$le"; do
    (cd "$crate" && rustup target add "$target" > /dev/null \
      && RUSTFLAGS="--remap-path-prefix=$crate=/sloptopop --remap-path-prefix=$HOME/.cargo=/cargo" \
         cargo build --release --locked --target "$target")
  done
  name=sloptopop-$version-$target
  stage=$(mktemp -d)/$name
  mkdir -p "$stage"
  cp "$here/target/$target/release/sloptopop" "$le/target/$target/release/sloptopop-local" "$stage/"
  cp "$here/LICENSE-MIT" "$here/LICENSE-APACHE" "$here/README.md" "$stage/"
  cp "$here/THIRD-PARTY-NOTICES.md" "$stage/THIRD-PARTY-NOTICES-sloptopop.md"
  cp "$le/THIRD-PARTY-NOTICES.md" "$stage/THIRD-PARTY-NOTICES-sloptopop-local.md"
  asset=sloptopop-$target.tar.gz
  (cd "$(dirname "$stage")" && COPYFILE_DISABLE=1 tar --uid 0 --gid 0 -cf - "$name") | gzip -n -9 > "$out/$asset"
  (cd "$out" && shasum -a 256 "$asset" > "$asset.sha256" && cat "$asset.sha256")
done
