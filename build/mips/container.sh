#!/bin/sh
# Runs build/mips/build.sh inside an amd64 Linux container (Apple `container`, or any engine given by CONTAINER).
# Usage: build/mips/container.sh mipsel-unknown-linux-musl|mips-unknown-linux-musl
# The toolchain is unpacked inside the container (host-mounted volumes cannot hold its symlinks); cargo and rustup
# homes persist in MIPS_HOME (default: .tmp/mips-toolchain/home). Environment: CONTAINER (default: container),
# CARGO_TARGET_DIR as a host path (default: .tmp/build/mips-<triple> under the checkout), MIPS_EXTRA_MOUNTS
# (extra `-v host:guest` options, e.g. a path-patched dependency checkout).
set -eu
TARGET="${1:?usage: $0 <triple>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TC="${MIPS_HOME:-$ROOT/.tmp/mips-toolchain}"
OUT="${CARGO_TARGET_DIR:-$ROOT/.tmp/build/mips-$TARGET}"
mkdir -p "$TC/home" "$TC/dl" "$OUT"
# shellcheck disable=SC2086
exec "${CONTAINER:-container}" run --rm --platform linux/amd64 --memory 8g -c 4 \
  -v "$ROOT:$ROOT" -v "$OUT:$OUT" ${MIPS_EXTRA_MOUNTS:-} \
  -v "$TC/dl:/opt/mips-dl" -e MIPS_TOOLCHAIN_CACHE=/opt/mips-dl -e MIPS_TOOLCHAIN_DIR=/opt/mips-toolchain -e CARGO_TARGET_DIR="$OUT" -e RUSTFLAGS="${RUSTFLAGS:-}" \
  -v "$TC/home:/work" -e CARGO_LOCKED="${CARGO_LOCKED:-1}" -e CARGO_HOME=/work/cargo -e RUSTUP_HOME=/work/rustup -e PATH=/work/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  -w "$ROOT" ubuntu:24.04 sh -c '
    set -eu
    command -v curl >/dev/null || { apt-get update -qq && apt-get install -y -qq curl ca-certificates gcc git xz-utils >/dev/null; }
    command -v rustup >/dev/null || curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none >/dev/null
    exec build/mips/build.sh "$1"
  ' sh "$TARGET"
