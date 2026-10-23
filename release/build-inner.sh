#!/bin/sh
# Runs inside the pinned Rust image (release/build.sh starts it): installs
# the musl C toolchain from a dated Debian snapshot, builds `sloptopop` and
# `sloptopop-local` as static musl binaries for this machine's architecture
# with remapped paths and fixed timestamps, and writes them to /build/bin.
# Everything that reaches the binaries is pinned: the image by digest, the
# Debian packages by snapshot date, the toolchain by rust-toolchain.toml, the
# crates by Cargo.lock (--locked); two builds give the same bytes.
set -eu
snapshot=${DEBIAN_SNAPSHOT:-20261020T000000Z}
printf 'deb [check-valid-until=no] http://snapshot.debian.org/archive/debian/%s bookworm main\n' "$snapshot" > /etc/apt/sources.list
rm -f /etc/apt/sources.list.d/*
apt-get update -q > /dev/null
apt-get install -q -y --no-install-recommends musl-tools > /dev/null
dpkg-query -W musl musl-tools musl-dev
target="$(uname -m)-unknown-linux-musl"
# Every sloptopop crate's build.rs runs the coverage gate unless
# CARGO_LLVM_COV is set; the gate runs on the gate runners and in CI.
export CARGO_LLVM_COV=1 SOURCE_DATE_EPOCH=0 CARGO_INCREMENTAL=0 \
  CARGO_PROFILE_RELEASE_LTO=true CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  CARGO_PROFILE_RELEASE_STRIP=symbols CARGO_PROFILE_RELEASE_PANIC=abort \
  RUSTFLAGS="--remap-path-prefix=/build=/sloptopop --remap-path-prefix=/usr/local/cargo=/cargo -C target-feature=+crt-static"
for crate in cli local-engine; do
  (cd "/build/$crate" && rustup target add "$target" > /dev/null && cargo build -q --release --locked --target "$target")
done
mkdir -p /build/bin
install -m 0755 "/build/cli/target/$target/release/sloptopop" /build/bin/sloptopop
install -m 0755 "/build/local-engine/target/$target/release/sloptopop-local" /build/bin/sloptopop-local
echo "$target" > /build/bin/TARGET
rustc --version
