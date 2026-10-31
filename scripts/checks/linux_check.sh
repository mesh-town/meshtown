#!/bin/sh
# Build the Linux side of the workspace (debug, no packaging) in an Apple `container` (Docker elsewhere): the
# node without tray and the Linux platform crate. A build, not `cargo check`: compiler recursion-limit errors
# ("queries overflow the depth limit") and other Linux-only failures appear in code generation, which
# `cargo check` skips.
#   scripts/checks/linux_check.sh
# Environment:
#   LINUX_CHECK_DIR — persistent cache directory (target, target-tray, cargo registry and git);
#                     default $BUILD_ROOT/linux-check. A warm directory makes a pass seconds, not minutes.
#   LINUX_CHECK_MAX_GB — cache size limit, default 50. Cargo never drops artifacts of older source states, so the
#                     directory only grows; under the lock, before the container starts, a directory over the limit
#                     loses the cheapest-to-rebuild parts first (incremental data, then compiled artifacts, then
#                     everything but the lock), one stderr line per step.
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
MAX_GB="${LINUX_CHECK_MAX_GB:-50}"
size_gb() { du -sk "$DIR" | awk '{ printf "%d", ($1 + 1048575) / 1048576 }'; }
# Remove the given paths (relative to $DIR) if the cache is still over the limit.
shrink() {
  what="$1"; shift
  gb="$(size_gb)"
  [ "$gb" -gt "$MAX_GB" ] || return 0
  echo "linux-check: cache $gb GB over $MAX_GB GB, removed $what" >&2
  for p in "$@"; do rm -rf "$DIR"/$p; done
}
shrink "incremental data" 'target*/debug/incremental'
shrink "compiled artifacts" 'target*/debug/deps' 'target*/debug/examples' 'target*/debug/.fingerprint' 'target*/debug/build'
shrink "everything but the lock" 'target' 'target-tray' 'cargo-registry' 'cargo-git'
"$CONTAINER" run --rm --platform "linux/$ARCH" --memory 8g --cpus 4 \
  -v "$ROOT:/src:ro" -w /src \
  -v "$DIR/cargo-registry:/usr/local/cargo/registry" -v "$DIR/cargo-git:/usr/local/cargo/git" \
  -v "$DIR/target:/target" -v "$DIR/target-tray:/target-tray" \
  -e CARGO_TERM_COLOR=never -e CARGO_INCREMENTAL=0 "$IMAGE" sh -c '
    set -e
    CARGO_TARGET_DIR=/target cargo build --locked --all-targets -p meshtown-core --no-default-features --features net-hyper
    CARGO_TARGET_DIR=/target-tray cargo build --locked --all-targets -p meshtown-linux
  '
