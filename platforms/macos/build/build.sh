#!/bin/sh
# Build of meshtown for macOS (the native architecture of this Mac, no cross-compilation).
# Usage: platforms/macos/build/build.sh cli|app|app_x86|app_universal
# The dist/ file names carry the version from Cargo.toml (<v> below); after copying to dist/
# the two newest versions of each name stay, older ones are removed (build_keep_two, scripts/release/lib.sh).
#   cli            — binary without tray (--no-default-features --features net-hyper), no bundle: dist/meshtown-macos-cli-<v>
#   app            — binary with tray (arm64) + .app + zip: dist/meshtown-macos-gui-<v>,
#                    dist/macos-arm64/MeshTown-<v>.app, dist/macos-arm64/MeshTown-macos-arm64-<v>.zip
#   app_x86        — the same for x86_64 (rustup target add x86_64-apple-darwin, ld64 from Xcode CLT):
#                    dist/meshtown-macos-gui-x86_64-<v>, dist/macos-x86_64/
#   app_universal  — universal binary (lipo, arm64+x86_64): dist/meshtown-macos-gui-universal-<v>,
#                    dist/macos-universal/
# The bundle is built by platforms/macos/build/app.sh in $BUILD_ROOT/app-stage/<arch>/ (MeshTown.app and an archive with no
# version in the name inside), build.sh copies it to dist/ under a versioned name; inside the archive the bundle
# stays MeshTown.app — it is installed into "Applications".
# Each kind builds its own binary, there are no prerequisites between kinds; app_universal builds
# both halves itself (an already built half is a cargo no-op).
# Environment variables (defaults in scripts/release/lib.sh):
#   BUILD_ROOT — build directory, default .tmp/build/ at the tree root.
#   TARGET_DIR — CARGO_TARGET_DIR for the native (arm64) build.
#   PROFILE — for app (arm64) only: PROFILE=dev builds without --release from $TARGET_DIR/debug and puts
#     files with a -dev suffix into dist/ (Mesh-<v>-dev.app, meshtown-macos-gui-<v>-dev); build_keep_two
#     does not clean them. For a quick look, a debug bundle is enough; release builds are for distribution.
#   APP_NAME, APP_TITLE, BUNDLE_ID — bundle file name, human-readable name and identifier (platforms/macos/build/app.sh).
# Build rules: one cargo build per host at a time.
set -e

if [ "$(uname -s)" != Darwin ]; then
  echo "build_macos builds on a Mac only, this is $(uname -s)" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
# shellcheck source=../../../scripts/release/lib.sh
. "$SCRIPT_DIR/../../../scripts/release/lib.sh"

KIND="$1"
APP_NAME="${APP_NAME:-MeshTown}"

# PROFILE=dev — for app (arm64) only; release by default, as before.
CARGO_REL="--release"
PROFILE_DIR=release
SUF=""
if [ "${PROFILE:-}" = dev ]; then
  if [ "$KIND" != app ]; then
    echo "PROFILE=dev is supported only for the app kind (arm64), got $KIND" >&2
    exit 2
  fi
  CARGO_REL=""
  PROFILE_DIR=debug
  SUF="-dev"
fi

# build_publish_app <binary in dist/> <arch> — builds the bundle in $BUILD_ROOT/app-stage/<arch>/ and
# puts the versioned MeshTown-<v>.app and MeshTown-macos-<arch>-<v>.zip into dist/macos-<arch>/; the signature after
# copying is verified; old versions of the bundle and archive are cleaned.
build_publish_app() {
  stage="$BUILD_ROOT/app-stage/$2"
  out="$DIST/macos-$2"
  rm -rf "$stage"
  mkdir -p "$stage" "$out"
  "$SCRIPT_DIR/app.sh" "$1" "$stage" "$2"
  rm -rf "$out/$APP_NAME-$VERSION$SUF.app"
  ditto "$stage/$APP_NAME.app" "$out/$APP_NAME-$VERSION$SUF.app"
  codesign --verify --deep "$out/$APP_NAME-$VERSION$SUF.app"
  cp "$stage/$APP_NAME-macos-$2.zip" "$out/$APP_NAME-macos-$2-$VERSION$SUF.zip"
  echo "bundle: $out/$APP_NAME-$VERSION$SUF.app, archive: $out/$APP_NAME-macos-$2-$VERSION$SUF.zip"
  build_keep_two "$out" "$APP_NAME" .app
  build_keep_two "$out" "$APP_NAME-macos-$2" .zip
}

# Manifest without dylib (release does not use it): cargo runs from $PROTO with --manifest-path to the farm.
FARM=""
case "$KIND" in
  cli|app|app_x86|app_universal) FARM="$(build_farm_no_dylib "$BUILD_ROOT/release-farm")" ;;
esac

# Source paths must not leak into the binaries (panic messages, debug info): the last matching prefix wins,
# so the most specific ones come last.
RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo --remap-path-prefix=$ROOT=/src --remap-path-prefix=$BUILD_ROOT/release-farm=/src"
if [ -n "${TOWNJUMP_RS:-}" ]; then
  RUSTFLAGS="$RUSTFLAGS --remap-path-prefix=$TOWNJUMP_RS=/townjump"
fi
export RUSTFLAGS

case "$KIND" in
  cli)
    (cd "$PROTO" && CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release --manifest-path "$FARM" --locked --no-default-features --features net-hyper)
    mkdir -p "$DIST"
    cp "$TARGET_DIR/release/meshtown" "$DIST/meshtown-macos-cli-$VERSION"
    echo "binary: $DIST/meshtown-macos-cli-$VERSION"
    build_keep_two "$DIST" meshtown-macos-cli
    ;;
  app)
    (cd "$PROTO" && CARGO_TARGET_DIR="$TARGET_DIR" cargo build $CARGO_REL --manifest-path "$FARM" --locked -p meshtown-macos)
    mkdir -p "$DIST"
    cp "$TARGET_DIR/$PROFILE_DIR/meshtown-macos" "$DIST/meshtown-macos-gui-$VERSION$SUF"
    echo "binary: $DIST/meshtown-macos-gui-$VERSION$SUF"
    build_keep_two "$DIST" meshtown-macos-gui
    build_publish_app "$DIST/meshtown-macos-gui-$VERSION$SUF" arm64
    ;;
  app_x86)
    rustup target add x86_64-apple-darwin
    APP_UNIVERSAL_DIR="$BUILD_ROOT/app-universal"
    (cd "$PROTO" && CARGO_TARGET_DIR="$APP_UNIVERSAL_DIR" cargo build --release --manifest-path "$FARM" --locked -p meshtown-macos --target x86_64-apple-darwin)
    mkdir -p "$DIST"
    cp "$APP_UNIVERSAL_DIR/x86_64-apple-darwin/release/meshtown-macos" "$DIST/meshtown-macos-gui-x86_64-$VERSION"
    echo "binary: $DIST/meshtown-macos-gui-x86_64-$VERSION"
    build_keep_two "$DIST" meshtown-macos-gui-x86_64
    build_publish_app "$DIST/meshtown-macos-gui-x86_64-$VERSION" x86_64
    ;;
  app_universal)
    rustup target add x86_64-apple-darwin
    APP_UNIVERSAL_DIR="$BUILD_ROOT/app-universal"
    (cd "$PROTO" && CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release --manifest-path "$FARM" --locked -p meshtown-macos)
    (cd "$PROTO" && CARGO_TARGET_DIR="$APP_UNIVERSAL_DIR" cargo build --release --manifest-path "$FARM" --locked -p meshtown-macos --target x86_64-apple-darwin)
    mkdir -p "$DIST"
    lipo -create -output "$DIST/meshtown-macos-gui-universal-$VERSION" \
      "$TARGET_DIR/release/meshtown-macos" \
      "$APP_UNIVERSAL_DIR/x86_64-apple-darwin/release/meshtown-macos"
    echo "binary: $DIST/meshtown-macos-gui-universal-$VERSION"
    lipo -info "$DIST/meshtown-macos-gui-universal-$VERSION"
    build_keep_two "$DIST" meshtown-macos-gui-universal
    build_publish_app "$DIST/meshtown-macos-gui-universal-$VERSION" universal
    ;;
  *)
    build_die_unknown_kind "$KIND" cli app app_x86 app_universal
    ;;
esac
