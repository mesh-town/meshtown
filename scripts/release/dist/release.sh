#!/usr/bin/env bash
# release.sh <assets-dir> <version>: publishes the files of <assets-dir> (with SHA256SUMS) as the GitHub release
# v<version> of the distribution repository. The tag points to a tiny orphan commit on an anchor branch, so the git
# history of the repository does not grow with every release.
# Env: GH_TOKEN (a token with write access to the repository; empty: nothing is published and the release stays in
# $GITHUB_REPOSITORY), MESHTOWN_DIST_REPO (default mesh-town/meshtown-dist), MESHTOWN_DIST_ANCHOR (default releases),
# GITHUB_OUTPUT (if set, receives "repo=<repository holding the release>").
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
. "$here/lib.sh"

usage() {
  echo "usage: release.sh <assets-dir> <version>   (version like 1.2.3; the directory must contain SHA256SUMS)" >&2
  exit 2
}

if [ "$#" -ne 2 ] || ! [[ "$2" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || [ ! -f "$1/SHA256SUMS" ]; then
  usage
fi
dir="$1"
version="$2"
tag="v$version"
repo="${MESHTOWN_DIST_REPO:-mesh-town/meshtown-dist}"
anchor="${MESHTOWN_DIST_ANCHOR:-releases}"
ident='{"name":"Mesh Town","email":"340663027+mesh-town@users.noreply.github.com"}'

emit_repo() {
  if [ -n "${GITHUB_OUTPUT:-}" ]; then
    printf 'repo=%s\n' "$1" >> "$GITHUB_OUTPUT"
  fi
}

if [ -z "${GH_TOKEN:-}" ]; then
  echo "::notice::DIST_TOKEN is not set; the release stays in ${GITHUB_REPOSITORY:-this repository}"
  emit_repo "${GITHUB_REPOSITORY:-}"
  exit 0
fi

# create_anchor: an orphan commit with one README.md and the branch $anchor pointing to it.
create_anchor() {
  local blob tree commit
  blob="$(gh api "repos/$repo/git/blobs" -f encoding=utf-8 \
    -f content='This branch only anchors the tags of the releases; the files are attached to the releases.' --jq .sha)"
  tree="$(printf '{"tree":[{"path":"README.md","mode":"100644","type":"blob","sha":"%s"}]}' "$blob" |
    gh api "repos/$repo/git/trees" --input - --jq .sha)"
  commit="$(printf '{"message":"Anchor of the release tags","tree":"%s","parents":[],"author":%s,"committer":%s}' \
    "$tree" "$ident" "$ident" | gh api "repos/$repo/git/commits" --input - --jq .sha)"
  gh api "repos/$repo/git/refs" -f ref="refs/heads/$anchor" -f sha="$commit" > /dev/null
}

if ! gh api "repos/$repo/git/ref/heads/$anchor" > /dev/null 2>&1; then
  create_anchor
fi

files=()
for f in "$dir"/*; do
  if [ -f "$f" ]; then
    files+=("$f")
  fi
done

if gh release view "$tag" --repo "$repo" > /dev/null 2>&1; then
  gh release upload "$tag" "${files[@]}" --repo "$repo" --clobber
  gh release edit "$tag" --repo "$repo" --draft=false
else
  notes="$(mktemp)"
  trap 'rm -f "$notes"' EXIT
  render "$here/templates/release.md.in" "$version" /dev/null > "$notes"
  gh release create "$tag" "${files[@]}" --repo "$repo" --target "$anchor" --title "$tag" \
    --notes-file "$notes" --latest
fi
emit_repo "$repo"
