# Shared part of the build/<system>/build.sh and build/dev.sh scripts: sourced with `.`,
# not run directly. Expects the calling script to have already set ROOT (the repository root) —
# the depth of the script itself differs between build/dev.sh (one level) and
# build/<system>/build.sh (two levels), so each caller computes it itself.
#
# Provides:
#   PROTO, DIST         — from ROOT.
#   VERSION             — version from Cargo.toml; part of the dist/ file names.
#   build_keep_two      — dist/ cleanup: the two newest versions of one name stay.
#   BUILD_ROOT, TARGET_DIR — defaults (shared by host builds of main).
#   CONTAINER           — `container` on Darwin, `docker` on other OSes; the default lives only
#                          here (the Makefile has no default for CONTAINER: `?= container` would override
#                          this choice when exported).
#   CONTAINER_RUN_FLAGS — shared part of the container command line
#                          (`--platform linux/<host arch> --memory 8g --cpus 4`).
#   build_cargo_git_prefetch <dir> — fills <dir>/git (and <dir>/registry) on the host: `cargo fetch --locked`
#                          with CARGO_HOME=<dir>; if TOWNJUMP_RS (a local townjump checkout) is set, the
#                          GitHub URL is substituted by that checkout. A non-zero exit code refuses the build.
#   build_townjump_rev  — townjump revision (hash) pinned in Cargo.lock, for the dist/ summary.
#   build_die_unknown_kind — refuse an unknown build kind: exit code 2 and the list of allowed kinds.
#
# Defaults use `${VAR:-default}` (empty or unset → default): an empty value
# that arrived through `export` in the root Makefile must not override the OS-based choice or the substituted
# default.
: "${ROOT:?build/common/lib.sh: the calling script must set ROOT before sourcing}"

PROTO="$ROOT"
DIST="${DIST:-$ROOT/dist}"

BUILD_ROOT="${BUILD_ROOT:-$ROOT/.tmp/build}"
TARGET_DIR="${TARGET_DIR:-$BUILD_ROOT/host}"

if [ -z "$CONTAINER" ]; then
  if [ "$(uname -s)" = Darwin ]; then
    CONTAINER=container
  else
    CONTAINER=docker
  fi
fi

# build_cargo_git_prefetch <directory> — git dependencies (townjump) are fetched on the host, the container
# only mounts <directory>/git next to <directory>/registry. TOWNJUMP_RS (optional): a local checkout that stands in for
# the GitHub repository during the fetch (the git CLI is required: libgit2 ignores the environment config).
build_cargo_git_prefetch() {
  mkdir -p "$1"
  if [ -n "${TOWNJUMP_RS:-}" ]; then
    CARGO_HOME="$1" CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=1 \
      GIT_CONFIG_KEY_0="url.file://$TOWNJUMP_RS.insteadOf" GIT_CONFIG_VALUE_0=https://github.com/mesh-town/townjump \
      cargo fetch --locked --manifest-path "$PROTO/Cargo.toml"
  else
    CARGO_HOME="$1" cargo fetch --locked --manifest-path "$PROTO/Cargo.toml"
  fi || {
    echo "cargo fetch --locked failed: git dependencies are not available, cannot build" >&2
    exit 1
  }
}

# build_townjump_rev — townjump revision hash from the Cargo.lock `source` line (...townjump?rev=<H>#<H>).
build_townjump_rev() {
  sed -n 's|^source = "git+[^"]*townjump?rev=[0-9a-f]*#\([0-9a-f]*\)"$|\1|p' "$PROTO/Cargo.lock" | head -1
}

CONTAINER_ARCH="$(uname -m | sed 's/x86_64/amd64/; s/aarch64/arm64/')"
CONTAINER_RUN_FLAGS="--rm --platform linux/$CONTAINER_ARCH --memory 8g --cpus 4"

# build_die_unknown_kind <received kind> <allowed1> [allowed2 ...]
build_die_unknown_kind() {
  got="$1"
  shift
  echo "unknown build kind: $got (expected one of: $*)" >&2
  exit 2
}

# build_manifest_no_dylib <directory> — creates the directory and puts a Cargo.toml in it: a copy
# of $PROTO/Cargo.toml without the `crate-type = ` line (the dylib is needed only by the fast make loop on macOS,
# for a rollout it adds an extra ~40 MB and link time). Prints the file path; over /src/Cargo.toml
# in the container the copy is mounted with a second -v, after the directory.
build_manifest_no_dylib() {
  mkdir -p "$1"
  sed '/^crate-type = /d' "$PROTO/Cargo.toml" > "$1/Cargo.toml"
  echo "$1/Cargo.toml"
}

# build_farm_no_dylib <directory> — for a build on the host without a container: a directory with a manifest without
# dylib and symlinks to $PROTO/src, web, vendor, Cargo.lock (nothing else is needed for `cargo build --release`).
# Earlier links are removed with `rm -f` (the links themselves, not their targets). Prints the path to Cargo.toml;
# call with `--manifest-path` from the $PROTO directory.
build_farm_no_dylib() {
  farm_manifest="$(build_manifest_no_dylib "$1")"
  for farm_item in src web vendor Cargo.lock; do
    rm -f "$1/$farm_item"
    ln -s "$PROTO/$farm_item" "$1/$farm_item"
  done
  echo "$farm_manifest"
}

# VERSION — `version` from the [package] section of $PROTO/Cargo.toml: it goes into the dist/ file names.
VERSION="$(sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' "$PROTO/Cargo.toml" | head -1)"
if [ -z "$VERSION" ]; then
  echo "version not found in [package] $PROTO/Cargo.toml" >&2
  exit 1
fi

# build_keep_two <directory> <name> [extension] — keeps the two newest versions (sort -V) of files
# <name>-X.Y.Z<extension>, deletes the rest (rm -rf: a .app bundle is a directory). The name matches strictly,
# so `meshtown-macos-gui` does not touch `meshtown-macos-gui-x86_64-*`. The extension comes with its dot.
build_keep_two() {
  keep_name="$(printf '%s' "$2" | sed 's/[.]/\\./g')"
  keep_ext="$(printf '%s' "$3" | sed 's/[.]/\\./g')"
  ls -1 "$1" | grep -E "^$keep_name-[0-9]+\.[0-9]+\.[0-9]+$keep_ext\$" | sort -V | sed '$d' | sed '$d' |
    while read -r keep_old; do
      rm -rf "${1:?}/$keep_old"
    done
}
