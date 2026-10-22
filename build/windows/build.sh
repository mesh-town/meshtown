#!/bin/sh
# Cross-build of meshtown for Windows x86_64 (.exe), in a container via cargo-zigbuild.
# Usage: build/windows/build.sh cli|gui
# The dist/ file name carries the version from Cargo.toml (<v>); after copying the two
# newest versions of each name stay (build_keep_two, build/common/lib.sh).
#   cli — without tray (--no-default-features --features net-hyper): dist/meshtown-windows-cli-<v>.exe
#   gui — with tray (default features): dist/meshtown-windows-gui-<v>.exe
# One shared build directory for both kinds ($BUILD_ROOT/make-windows/out), the binary
# is copied to dist/ right after its own build, there are no prerequisites between kinds.
# The target manifest is a copy without `crate-type` (see below), laid over /src/Cargo.toml in the container.
# Environment variables (defaults in build/common/lib.sh): BUILD_ROOT, CONTAINER.
# townjump is a git dependency: fetched on the host (build_cargo_git_prefetch) into $BUILD_ROOT/make-windows/git, mounted at /usr/local/cargo/git.
# The image's rustc (1.93) is older than townjump's rust-version 1.94: rustup inside the container installs stable
# (minimal profile), the toolchain lives in the parent's registry volume no longer than the container (--rm), and is reinstalled each time.
# sh -c, not sh -lc: a login shell resets the image's PATH.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
# shellcheck source=../common/lib.sh
. "$SCRIPT_DIR/../common/lib.sh"

KIND="$1"
WINDOWS_DIR="$BUILD_ROOT/make-windows"

case "$KIND" in
  cli)
    FEATURE_FLAG="--no-default-features --features net-hyper"
    OUT_NAME="meshtown-windows-cli"
    ;;
  gui)
    FEATURE_FLAG=""
    OUT_NAME="meshtown-windows-gui"
    ;;
  *)
    build_die_unknown_kind "$KIND" cli gui
    ;;
esac

mkdir -p "$WINDOWS_DIR/out" "$WINDOWS_DIR/registry"
build_cargo_git_prefetch "$WINDOWS_DIR"

# The Windows target manifest is a copy of Cargo.toml without the `crate-type` line. Dynamic linking
# of the library (a fast-loop measure) makes the PE format ask for an export table of 82,330 public symbols,
# and the field holding their count in that table is 16-bit — the ceiling is 65,535, and linking meshtown.dll fails with
# exit code 101. Windows does not need dynamic linking: `-C prefer-dynamic` lives only in the Makefile targets on
# macOS, the rollout calls cargo directly and links statically from rlib. cargo builds the lib in one rustc
# call with all declared crate-types at once, so `--bin` does not cancel the dylib — the only way is to remove
# it from the manifest. The copy is laid over /src/Cargo.toml only inside the container: the tree is not
# changed and parallel builds do not see the manifest flicker. The order of the mounts matters — the directory
# first, then the file over it.
WIN_MANIFEST="$(build_manifest_no_dylib "$WINDOWS_DIR")"

"$CONTAINER" run $CONTAINER_RUN_FLAGS \
  -v "$PROTO:/src" -v "$WIN_MANIFEST:/src/Cargo.toml" -w /src -e CARGO_TARGET_DIR=/target \
  -v "$WINDOWS_DIR/out:/target" -v "$WINDOWS_DIR/registry:/usr/local/cargo/registry" -v "$WINDOWS_DIR/git:/usr/local/cargo/git" \
  ghcr.io/rust-cross/cargo-zigbuild:latest \
  sh -c "rustup toolchain install stable --profile minimal && rustup default stable && rustup target add x86_64-pc-windows-gnu && cargo zigbuild --locked --release --target x86_64-pc-windows-gnu $FEATURE_FLAG"
mkdir -p "$DIST"
cp "$WINDOWS_DIR/out/x86_64-pc-windows-gnu/release/meshtown.exe" "$DIST/$OUT_NAME-$VERSION.exe"
echo "binary: $DIST/$OUT_NAME-$VERSION.exe"
build_keep_two "$DIST" "$OUT_NAME" .exe
