#!/usr/bin/env bash
# lib_test.sh: checks the shared build helpers in release/lib.sh against a throwaway workspace tree.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
lib="$here/lib.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() { echo "lib_test: FAIL: $*" >&2; exit 1; }

# make_tree <dir>: a minimal virtual workspace with a core member.
make_tree() {
  mkdir -p "$1/core/node" "$1/core/web_gui/ui"
  printf '[workspace]\nmembers = ["core"]\n\n[workspace.package]\nversion = "1.2.3"\n' > "$1/Cargo.toml"
  printf '# lock\n' > "$1/Cargo.lock"
  printf '[package]\nname = "x"\nversion.workspace = true\n\n[lib]\ncrate-type = ["lib", "dylib"]\n' > "$1/core/Cargo.toml"
  printf 'pub fn f() {}\n' > "$1/core/lib.rs"
  printf 'fn x() {}\n' > "$1/core/node/x.rs"
  printf '<html></html>\n' > "$1/core/web_gui/ui/index.html"
}

# in_lib <root> <script>: runs <script> in a shell that sources lib.sh the way the platform scripts do.
in_lib() {
  local root="$1"
  shift
  ROOT="$root" sh -c ". '$lib'; $*"
}

root="$tmp/tree"
make_tree "$root"

version_from_workspace_package() {
  local got
  got="$(in_lib "$root" 'echo "$VERSION"')"
  [ "$got" = "1.2.3" ] || fail "version_from_workspace_package: got '$got'"
  mkdir -p "$tmp/plain"
  printf '[package]\nname = "p"\nversion = "9.9.9"\n' > "$tmp/plain/Cargo.toml"
  if err="$(in_lib "$tmp/plain" 'echo "$VERSION"' 2>&1)"; then
    fail "version_from_workspace_package: a [package] version was accepted"
  fi
  case "$err" in *'[workspace.package]'*) ;; *) fail "version_from_workspace_package: message: $err" ;; esac
}

root_is_required() {
  if err="$(env -u ROOT sh -c ". '$lib'" 2>&1)"; then fail "root_is_required: sourcing without ROOT passed"; fi
  case "$err" in *release/lib.sh*) ;; *) fail "root_is_required: message: $err" ;; esac
}

manifest_copy_has_no_crate_type() {
  local out="$tmp/m1" before
  before="$(cat "$root/core/Cargo.toml")"
  in_lib "$root" "build_manifest_no_dylib '$out' >/dev/null"
  ! grep -q 'crate-type' "$out/Cargo.toml" || fail "manifest_copy: crate-type kept"
  [ "$(diff "$root/core/Cargo.toml" "$out/Cargo.toml" | grep -c '^<')" = 1 ] || fail "manifest_copy: not one removed line"
  [ "$(cat "$root/core/Cargo.toml")" = "$before" ] || fail "manifest_copy: source changed"
  [ "$(ls -A "$out")" = Cargo.toml ] || fail "manifest_copy: extra files in the directory"
  mkdir -p "$tmp/bare"
  printf '[workspace]\n[workspace.package]\nversion = "1.0.0"\n' > "$tmp/bare/Cargo.toml"
  if in_lib "$tmp/bare" "build_manifest_no_dylib '$tmp/m2'" 2>/dev/null; then fail "manifest_copy: missing source passed"; fi
}

farm_layout() {
  local farm="$tmp/farm"
  in_lib "$root" "build_farm_no_dylib '$farm' >/dev/null"
  cmp "$root/Cargo.toml" "$farm/Cargo.toml" || fail "farm_layout: root manifest differs"
  [ -L "$farm/Cargo.lock" ] || fail "farm_layout: Cargo.lock is not a link"
  [ -f "$farm/core/Cargo.toml" ] && [ ! -L "$farm/core/Cargo.toml" ] || fail "farm_layout: core manifest is not a plain file"
  ! grep -q 'crate-type' "$farm/core/Cargo.toml" || fail "farm_layout: crate-type kept"
  local item
  for item in lib.rs node web_gui; do
    [ -L "$farm/core/$item" ] || fail "farm_layout: core/$item is not a link"
    [ "$(readlink "$farm/core/$item")" = "$root/core/$item" ] || fail "farm_layout: core/$item points elsewhere"
  done
  for item in src web vendor; do
    [ ! -e "$farm/$item" ] && [ ! -L "$farm/$item" ] || fail "farm_layout: unexpected $item"
  done
}

farm_rerun_drops_stale_entries() {
  local farm="$tmp/farm2"
  in_lib "$root" "build_farm_no_dylib '$farm' >/dev/null"
  ln -s "$root/core/node" "$farm/core/old"
  in_lib "$root" "build_farm_no_dylib '$farm' >/dev/null"
  [ ! -e "$farm/core/old" ] && [ ! -L "$farm/core/old" ] || fail "farm_rerun: stale entry kept"
  [ -f "$root/core/node/x.rs" ] || fail "farm_rerun: source damaged"
}

farm_needs_core() {
  mkdir -p "$tmp/nocore"
  printf '[workspace]\n[workspace.package]\nversion = "1.0.0"\n' > "$tmp/nocore/Cargo.toml"
  if in_lib "$tmp/nocore" "build_farm_no_dylib '$tmp/farm3'" 2>/dev/null; then fail "farm_needs_core: passed without core"; fi
}

for t in version_from_workspace_package root_is_required manifest_copy_has_no_crate_type farm_layout \
  farm_rerun_drops_stale_entries farm_needs_core; do
  "$t"
  echo "ok: $t"
done
