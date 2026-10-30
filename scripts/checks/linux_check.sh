#!/bin/sh
# Build the Linux side of the workspace (debug, no packaging) in an Apple `container` (Docker elsewhere): the
# node without tray and the Linux platform crate. A build, not `cargo check`: compiler recursion-limit errors
# ("queries overflow the depth limit") and other Linux-only failures appear in code generation, which
# `cargo check` skips.
#   scripts/checks/linux_check.sh
# Environment:
#   LINUX_CHECK_DIR — persistent cache directory (target, target-tray, cargo registry and git);
#                     default $BUILD_ROOT/linux-check. A warm directory makes a pass seconds, not minutes.
#   LINUX_CHECK_IMAGE — Rust image, default rust:1.98-bookworm.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="${LINUX_CHECK_DIR:-${BUILD_ROOT:-$ROOT/.tmp/build}/linux-check}"
IMAGE="${LINUX_CHECK_IMAGE:-rust:1.98-bookworm}"
if [ -z "${CONTAINER:-}" ]; then
  if [ "$(uname -s)" = Darwin ]; then CONTAINER=container; else CONTAINER=docker; fi
fi
case "$(uname -m)" in
  arm64 | aarch64) ARCH=arm64 ;;
  *) ARCH=amd64 ;;
esac
mkdir -p "$DIR/target" "$DIR/target-tray" "$DIR/cargo-registry" "$DIR/cargo-git"
exec "$CONTAINER" run --rm --platform "linux/$ARCH" --memory 8g --cpus 4 \
  -v "$ROOT:/src:ro" -w /src \
  -v "$DIR/cargo-registry:/usr/local/cargo/registry" -v "$DIR/cargo-git:/usr/local/cargo/git" \
  -v "$DIR/target:/target" -v "$DIR/target-tray:/target-tray" \
  -e CARGO_TERM_COLOR=never "$IMAGE" sh -c '
    set -e
    CARGO_TARGET_DIR=/target cargo build --locked --all-targets -p meshtown-core --no-default-features --features net-hyper
    CARGO_TARGET_DIR=/target-tray cargo build --locked --all-targets -p meshtown-linux
  '
