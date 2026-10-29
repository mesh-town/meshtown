#!/bin/sh
# Runs the Linux tray end-to-end test (a late StatusNotifierWatcher) inside a container with a private session bus.
set -eu

ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
CLI="${CONTAINER_CLI:-container}"
IMAGE="${BUILD_IMAGE:-rust:1-bookworm}"
CACHE="$ROOT/.tmp/build/tray-e2e"
mkdir -p "$CACHE/target" "$CACHE/cargo-registry" "$CACHE/cargo-git"

"$CLI" run --rm --platform linux/arm64 --memory 8g --cpus 4 \
  -v "$ROOT:/src" -w /src \
  -v "$CACHE/target:/target" \
  -v "$CACHE/cargo-registry:/usr/local/cargo/registry" \
  -v "$CACHE/cargo-git:/usr/local/cargo/git" \
  -e CARGO_TARGET_DIR=/target \
  "$IMAGE" sh -c '
    set -eu
    apt-get update -qq && apt-get install -y -qq dbus
    rustup component add clippy
    cargo clippy -p meshtown-linux --all-targets --locked -- -D warnings
    MESHTOWN_TRAY_E2E=1 dbus-run-session -- cargo test --locked -p meshtown-linux --test tray_late_watcher -- --nocapture
  '
