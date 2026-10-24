#!/usr/bin/env bash
# build_debs.sh <assets-dir> <out-dir> <version>
# Builds meshtown_<version>_<arch>.deb (amd64, arm64) into <out-dir> from the release archives
# <assets-dir>/meshtown-<version>-<x86_64|aarch64>-unknown-linux-musl.tar.gz. The keyring packed into the
# packages is MESHTOWN_APT_PUBKEY, if set, else the one committed next to deb.py.
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: build_debs.sh <assets-dir> <out-dir> <version>" >&2
  exit 2
fi
assets="$1" out="$2" version="$3"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
deb_py="$here/../../../platforms/linux/build/debian/deb.py"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$out"
for pair in x86_64:amd64 aarch64:arm64; do
  rust="${pair%:*}" arch="${pair#*:}"
  archive="$assets/meshtown-$version-$rust-unknown-linux-musl.tar.gz"
  if [ ! -f "$archive" ]; then
    echo "build_debs.sh: missing $archive" >&2
    exit 1
  fi
  mkdir -p "$tmp/$arch"
  tar xzf "$archive" -C "$tmp/$arch"
  binary="$(find "$tmp/$arch" -type f -name meshtown | head -n 1)"
  license="$(find "$tmp/$arch" -type f -name LICENSE | head -n 1)"
  if [ -z "$binary" ]; then
    echo "build_debs.sh: no meshtown binary in $archive" >&2
    exit 1
  fi
  args=(--binary "$binary" --arch "$arch" --version "$version" --out "$out")
  if [ -n "$license" ]; then args+=(--license "$license"); fi
  if [ -n "${MESHTOWN_APT_PUBKEY:-}" ]; then args+=(--keyring "$MESHTOWN_APT_PUBKEY"); fi
  python3 "$deb_py" "${args[@]}"
done
