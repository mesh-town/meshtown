#!/bin/sh
# Builds the Windows installer (NSIS): dist/meshtown-setup-<version>.exe.
# Usage: platforms/windows/build/installer.sh [<gui-exe>]
# The gui exe defaults to dist/meshtown-windows-gui-<version>.exe (platforms/windows/build/build.sh gui).
# A VERSION set in the environment wins over the Cargo.toml one: that is how an older installer is built
# for an upgrade check.
# makensis comes from PATH; without it, a container ($CONTAINER, debian + nsis) runs it.
# Environment variables (defaults in scripts/release/lib.sh): DIST, BUILD_ROOT, CONTAINER.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
WANT_VERSION="${VERSION:-}"
# shellcheck source=../../../scripts/release/lib.sh
. "$SCRIPT_DIR/../../../scripts/release/lib.sh"
[ -z "$WANT_VERSION" ] || VERSION="$WANT_VERSION"

EXE="${1:-$DIST/meshtown-windows-gui-$VERSION.exe}"
if [ ! -f "$EXE" ]; then
  echo "gui exe not found: $EXE (build it with: platforms/windows/build/build.sh gui)" >&2
  exit 1
fi
EXE="$(cd "$(dirname "$EXE")" && pwd)/$(basename "$EXE")"

mkdir -p "$DIST"
DIST="$(cd "$DIST" && pwd)"
OUT="$DIST/meshtown-setup-$VERSION.exe"

if command -v makensis >/dev/null 2>&1; then
  makensis -V2 -DVERSION="$VERSION" -DEXE="$EXE" -DOUTFILE="$OUT" "$SCRIPT_DIR/installer.nsi"
else
  # Everything the script needs is mounted under /in (read-only) and /out.
  "$CONTAINER" run $CONTAINER_RUN_FLAGS \
    -v "$SCRIPT_DIR:/in/nsi:ro" -v "$(dirname "$EXE"):/in/exe:ro" -v "$DIST:/out" \
    debian:stable-slim \
    sh -c "export DEBIAN_FRONTEND=noninteractive && apt-get update -qq && apt-get install -y -qq nsis >/dev/null && cd /in/nsi && makensis -V2 -DVERSION='$VERSION' -DEXE='/in/exe/$(basename "$EXE")' -DOUTFILE='/out/meshtown-setup-$VERSION.exe' installer.nsi"
fi
echo "installer: $OUT"
build_keep_two "$DIST" meshtown-setup .exe
