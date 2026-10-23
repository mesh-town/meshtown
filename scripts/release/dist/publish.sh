#!/usr/bin/env bash
# publish.sh <src-dir> <version>
# Publishes builds as one orphan commit on branch main of the distribution repository (force push: the history is not
# kept). <src-dir> holds folders per system (macos, windows, linux, android) with files <stem>-<version>.<ext>. The
# folders present in <src-dir> are updated: each kind of file keeps its four (keep) newest versions plus <stem>.<ext>,
# a copy of the newest one. The other folders, and the older files, are taken from the current main of the repository
# unchanged. Every folder gets SHA256SUMS and README.md; the root gets README.md and version.txt.
# The ssh key, if any, comes from GIT_SSH_COMMAND of the caller; the repository URL can be overridden with
# MESHTOWN_DIST_PUBLISH_URL.
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: publish.sh <src-dir> <version>" >&2
  exit 2
fi
src="$1" version="$2"
url="${MESHTOWN_DIST_PUBLISH_URL:-git@github.com:mesh-town/meshtown-dist.git}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
keep=4
folders="macos windows linux android"
# shellcheck source=lib.sh
. "$here/lib.sh"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "publish.sh: bad version: $version" >&2
  exit 2
fi
for d in "$src"/*/; do
  d="${d%/}"
  case " $folders " in
    *" ${d##*/} "*) ;;
    *) echo "publish.sh: unknown folder in $src: ${d##*/}" >&2; exit 1 ;;
  esac
done

work="$(mktemp -d)"
prev="$(mktemp -d)"
trap 'rm -rf "$work" "$prev"' EXIT
# An empty repository has no main yet; any other failure of ls-remote or clone stops the script.
heads="$(git ls-remote --heads "$url" main)"
if [ -n "$heads" ]; then
  git clone -q --depth 1 --branch main "$url" "$prev/repo"
  rm -rf "$prev/repo/.git"
  cp -R "$prev/repo/." "$work/"
fi

published=0
for folder in $folders; do
  if [ -d "$src/$folder" ]; then
    mkdir -p "$work/$folder"
    count=0
    for f in "$src/$folder"/*-"$version".*; do
      if [ -f "$f" ]; then
        cp "$f" "$work/$folder/"
        count=$((count + 1))
      fi
    done
    if [ "$count" -eq 0 ]; then
      echo "publish.sh: no files for $version in $src/$folder" >&2
      exit 1
    fi
    published=$((published + 1))
    while IFS= read -r kind; do
      refresh_kind "$work/$folder" "${kind%|*}" "${kind#*|}" "$keep"
    done < <(kinds_in "$work/$folder")
    write_sums "$work/$folder"
    versions_table "$work/$folder" "$folder" > "$prev/table"
    render "$here/templates/$folder.md.in" "$version" "$prev/table" > "$work/$folder/README.md"
  fi
done
if [ "$published" -eq 0 ]; then
  echo "publish.sh: $src has no system folders" >&2
  exit 1
fi
printf '%s\n' "$version" > "$work/version.txt"
render "$here/templates/root.md.in" "$version" /dev/null > "$work/README.md"

cd "$work"
export TZ=UTC
export GIT_AUTHOR_NAME="Mesh Town" GIT_AUTHOR_EMAIL="340663027+mesh-town@users.noreply.github.com"
export GIT_COMMITTER_NAME="Mesh Town" GIT_COMMITTER_EMAIL="340663027+mesh-town@users.noreply.github.com"
git init -q -b main .
git add -A -f .
git -c commit.gpgsign=false commit -q -m "meshtown $version"
git push -q --force "$url" HEAD:main
echo "publish.sh: pushed meshtown $version"
