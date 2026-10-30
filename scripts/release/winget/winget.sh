#!/usr/bin/env bash
# winget.sh: manifests of the Windows installer for the winget community repository.
#   winget.sh render <version> <installer> <out> [--base-url URL]
#       writes <out>/manifests/m/MeshTown/MeshTown/<version>/ (three files) from templates/*.yaml.in.
#       The installer URL is <base>/meshtown-setup-<version>.exe; the base is
#       https://github.com/<MESHTOWN_DIST_REPO>/releases/download/v<version> unless --base-url is given.
#       A wrong version or a missing installer: exit 2, nothing written.
#   winget.sh submit <version>
#       komac update MeshTown.MeshTown --version <version> --urls <URL> --submit (--dry-run when WINGET_DRY_RUN=1).
#       Empty GITHUB_TOKEN: a notice, exit 0, komac is not run. komac not in PATH: exit 1.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
id=MeshTown.MeshTown
manifest_version=1.10.0
repo="${MESHTOWN_DIST_REPO:-mesh-town/meshtown-dist}"

usage() {
  echo "usage: winget.sh render <version> <installer> <out> [--base-url URL] | submit <version>" >&2
  exit 2
}

check_version() {
  [[ "${1:-}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || usage
}

sha256_upper() {
  if command -v sha256sum > /dev/null 2>&1; then
    sha256sum -- "$1" | cut -d' ' -f1 | tr 'a-f' 'A-F'
  else
    shasum -a 256 -- "$1" | cut -d' ' -f1 | tr 'a-f' 'A-F'
  fi
}

# short_description: `description` of core/Cargo.toml, quoted for a YAML single-quoted scalar.
short_description() {
  local d
  d="$(sed -n 's/^description = "\(.*\)"$/\1/p' "$here/../../../core/Cargo.toml" | head -n 1)"
  [ -n "$d" ] || { echo "winget.sh: no description in core/Cargo.toml" >&2; exit 1; }
  printf '%s' "${d//\'/\'\'}"
}

# fill <template> <version> <url> <sha> <short>: the template with the placeholders replaced.
fill() {
  local line
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line//@MANIFEST_VERSION@/$manifest_version}"
    line="${line//@VERSION@/$2}"
    line="${line//@INSTALLER_URL@/$3}"
    line="${line//@INSTALLER_SHA256@/$4}"
    line="${line//@SHORT_DESCRIPTION@/$5}"
    printf '%s\n' "$line"
  done < "$1"
}

render() {
  [ "$#" -ge 3 ] || usage
  local version="$1" installer="$2" out="$3" base=""
  shift 3
  check_version "$version"
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --base-url) [ "$#" -ge 2 ] || usage; base="$2"; shift 2 ;;
      *) usage ;;
    esac
  done
  [ -f "$installer" ] || { echo "winget.sh: no installer $installer" >&2; exit 2; }
  base="${base:-https://github.com/$repo/releases/download/v$version}"
  local url="${base%/}/meshtown-setup-$version.exe" sha short dir name
  sha="$(sha256_upper "$installer")"
  short="$(short_description)"
  dir="$out/manifests/m/MeshTown/MeshTown/$version"
  mkdir -p "$dir"
  for name in "$id" "$id.locale.en-US" "$id.installer"; do
    fill "$here/templates/$name.yaml.in" "$version" "$url" "$sha" "$short" > "$dir/$name.yaml"
  done
}

submit() {
  [ "$#" -eq 1 ] || usage
  check_version "$1"
  if [ -z "${GITHUB_TOKEN:-}" ]; then
    echo "::notice::GITHUB_TOKEN is not set; no winget pull request is made"
    return 0
  fi
  command -v komac > /dev/null 2>&1 || { echo "winget.sh: komac not found in PATH" >&2; exit 1; }
  local mode=--submit
  [ "${WINGET_DRY_RUN:-0}" = 1 ] && mode=--dry-run
  komac update "$id" --version "$1" --urls "https://github.com/$repo/releases/download/v$1/meshtown-setup-$1.exe" "$mode"
}

case "${1:-}" in
  render) shift; render "$@" ;;
  submit) shift; submit "$@" ;;
  *) usage ;;
esac
