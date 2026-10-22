#!/bin/sh
# Cross-build of meshtown for Linux x86_64 (static, musl), in a container.
# Usage: platforms/linux/build/build.sh cli|gui|arm64_cli|armv7_cli
# The dist/ file name carries the version from Cargo.toml (<v>); after copying the two
# newest versions of each name stay (build_keep_two, release/lib.sh).
#   cli       — without tray (--no-default-features --features net-hyper): dist/meshtown-linux-cli-<v>
#   gui       — with tray (default features): dist/meshtown-linux-gui-<v>
#   arm64_cli — Linux arm64 (OpenWrt router, static musl) without tray, image
#               messense/rust-musl-cross:aarch64-musl: dist/meshtown-linux-arm64-cli-<v>
#   armv7_cli — Linux ARMv7 (OpenWrt router, static musl, soft-float musleabi: no FPU instructions, runs on cores with
#               and without an FPU) without tray, image messense/rust-musl-cross:armv7-musleabi:
#               dist/meshtown-linux-armv7-cli-<v>
# townjump (hyper transport, the net-hyper feature in default) is a git dependency: it is fetched on the host
# (build_cargo_git_prefetch, release/lib.sh) into $BUILD_ROOT/make-linux*/git, mounted at /root/.cargo/git.
# One shared build directory for cli and gui ($BUILD_ROOT/make-linux/out) — cargo keeps dependencies with
# different features side by side, only the crate itself is rebuilt; the binary is copied to
# dist/ right after its own build, there are no prerequisites between kinds. arm64_cli has its own directory
# ($BUILD_ROOT/make-linux-arm64/{out,registry}); armv7_cli likewise ($BUILD_ROOT/make-linux-armv7): a different image and a
# different target. These kinds are not part of the build_linux (x86_64) group — only the explicit
# `make build_linux_arm64_cli` / `make build_linux_armv7_cli`.
# HEAP_PROF=1 (cli, gui) — heap profile for the memory watchdog: --features heap-prof --profile release-sym
#   (jemalloc with profiling, symbols for jeprof; the binary comes from <triple>/release-sym/); by default — the usual build.
# Environment variables (defaults in release/lib.sh): BUILD_ROOT, CONTAINER.
# Build rules: --memory 8g -c 4 are mandatory, emulation is for running only.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
# shellcheck source=../../../release/lib.sh
. "$SCRIPT_DIR/../../../release/lib.sh"

KIND="$1"
IMAGE="messense/rust-musl-cross:x86_64-musl"
TRIPLE="x86_64-unknown-linux-musl"
LINUX_DIR="$BUILD_ROOT/make-linux"

case "$KIND" in
  cli)
    FEATURE_FLAG="--no-default-features --features net-hyper"
    OUT_NAME="meshtown-linux-cli"
    ;;
  gui)
    FEATURE_FLAG=""
    OUT_NAME="meshtown-linux-gui"
    ;;
  arm64_cli)
    IMAGE="messense/rust-musl-cross:aarch64-musl"
    TRIPLE="aarch64-unknown-linux-musl"
    LINUX_DIR="$BUILD_ROOT/make-linux-arm64"
    FEATURE_FLAG="--no-default-features --features net-hyper"
    OUT_NAME="meshtown-linux-arm64-cli"
    ;;
  armv7_cli)
    IMAGE="messense/rust-musl-cross:armv7-musleabi"
    TRIPLE="armv7-unknown-linux-musleabi"
    LINUX_DIR="$BUILD_ROOT/make-linux-armv7"
    FEATURE_FLAG="--no-default-features --features net-hyper"
    OUT_NAME="meshtown-linux-armv7-cli"
    ;;
  *)
    build_die_unknown_kind "$KIND" cli gui arm64_cli armv7_cli
    ;;
esac

PROFILE="release"
PROF_FLAGS=""
if [ "${HEAP_PROF:-}" = "1" ]; then
  PROFILE="release-sym"
  PROF_FLAGS="--features heap-prof --profile release-sym"
else
  PROF_FLAGS="--release"
fi

mkdir -p "$LINUX_DIR/out" "$LINUX_DIR/registry"
build_cargo_git_prefetch "$LINUX_DIR"
# Manifest without `crate-type` (no dylib), over /src/core/Cargo.toml only in the container; the order
# of the mounts matters — the directory first, then the file.
MANIFEST="$(build_manifest_no_dylib "$LINUX_DIR")"
"$CONTAINER" run $CONTAINER_RUN_FLAGS \
  -v "$PROTO:/src" -v "$MANIFEST:/src/core/Cargo.toml" -w /src -e CARGO_TARGET_DIR=/target \
  -v "$LINUX_DIR/out:/target" -v "$LINUX_DIR/registry:/root/.cargo/registry" -v "$LINUX_DIR/git:/root/.cargo/git" \
  "$IMAGE" \
  cargo build --locked $PROF_FLAGS --target "$TRIPLE" $FEATURE_FLAG
mkdir -p "$DIST"
cp "$LINUX_DIR/out/$TRIPLE/$PROFILE/meshtown" "$DIST/$OUT_NAME-$VERSION"
echo "binary: $DIST/$OUT_NAME-$VERSION"
build_keep_two "$DIST" "$OUT_NAME"
