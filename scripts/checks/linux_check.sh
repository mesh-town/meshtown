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
# One pass at a time per cache directory: two containers building into one target directory remove each
# other's artifacts. A second pass waits for the lock `$DIR/.lock`; a lock whose process is gone is taken over.
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
LOCK="$DIR/.lock"
until mkdir "$LOCK" 2>/dev/null; do
  holder="$(cat "$LOCK/pid" 2>/dev/null || true)"
  if [ -n "$holder" ] && ! kill -0 "$holder" 2>/dev/null; then rm -rf "$LOCK"; continue; fi
  [ -n "${waiting:-}" ] || echo "linux-check: $DIR is in use by pid ${holder:-?}, waiting" >&2
  waiting=1
  sleep 5
done
echo $$ >"$LOCK/pid"
trap 'rm -rf "$LOCK"' EXIT
trap 'exit 130' INT TERM
"$CONTAINER" run --rm --platform "linux/$ARCH" --memory 8g --cpus 4 \
  -v "$ROOT:/src:ro" -w /src \
  -v "$DIR/cargo-registry:/usr/local/cargo/registry" -v "$DIR/cargo-git:/usr/local/cargo/git" \
  -v "$DIR/target:/target" -v "$DIR/target-tray:/target-tray" \
  -e CARGO_TERM_COLOR=never "$IMAGE" sh -c '
    set -e
    CARGO_TARGET_DIR=/target cargo build --locked --all-targets -p meshtown-core --no-default-features --features net-hyper
    CARGO_TARGET_DIR=/target-tray cargo build --locked --all-targets -p meshtown-linux
  '
