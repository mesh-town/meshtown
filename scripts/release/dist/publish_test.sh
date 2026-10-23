#!/usr/bin/env bash
# publish_test.sh: runs publish.sh against a local bare repository and checks the result.
# Publishes 0.3.8 ... 0.3.13, 0.3.13 again, then only android/ 0.3.14.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export MESHTOWN_DIST_PUBLISH_URL="file://$tmp/remote.git"
export GIT_CONFIG_GLOBAL=/dev/null
git init -q --bare -b main "$tmp/remote.git"

fail() { echo "publish_test: FAIL: $*" >&2; exit 1; }

# kinds: folder/stem/ext
kinds=(
  "macos/MeshTown-macos-arm64/zip" "macos/MeshTown-macos-x86_64/zip"
  "windows/meshtown/exe" "windows/meshtown-cli/exe"
  "linux/meshtown-linux-x86_64/tar.gz" "linux/meshtown-linux-aarch64/tar.gz"
  "linux/meshtown-cli-linux-x86_64/tar.gz" "linux/meshtown-cli-linux-aarch64/tar.gz"
  "android/meshtown/apk"
)

# make_src <dir> <version> [folder...]: fake files of the given folders (all by default).
make_src() {
  local dir="$1" v="$2" k folder rest stem ext
  shift 2
  for k in "${kinds[@]}"; do
    folder="${k%%/*}" rest="${k#*/}" stem="${rest%/*}" ext="${rest##*/}"
    if [ "$#" -eq 0 ] || [[ " $* " == *" $folder "* ]]; then
      mkdir -p "$dir/$folder"
      printf 'fake %s %s\n' "$stem" "$v" > "$dir/$folder/$stem-$v.$ext"
    fi
  done
}

publish() { # publish <version> [folder...]
  local v="$1"
  shift
  rm -rf "$tmp/src"
  make_src "$tmp/src" "$v" "$@"
  "$here/publish.sh" "$tmp/src" "$v" > /dev/null
}

sums_check() { # in <dir>
  if command -v sha256sum > /dev/null 2>&1; then
    (cd "$1" && sha256sum -c SHA256SUMS > /dev/null)
  else
    (cd "$1" && shasum -a 256 -c SHA256SUMS > /dev/null)
  fi
}

for v in 0.3.8 0.3.9 0.3.10 0.3.11 0.3.12 0.3.13; do publish "$v"; done
publish 0.3.13
publish 0.3.14 android

clone="$tmp/clone"
git clone -q "$MESHTOWN_DIST_PUBLISH_URL" "$clone"
[ "$(git -C "$clone" rev-list --count HEAD)" = 1 ] || fail "the history must be one commit"
[ "$(git -C "$clone" log -1 --format=%s)" = "meshtown 0.3.14" ] || fail "commit message"
[ "$(git -C "$clone" log -1 --format=%an)" = "Mesh Town" ] || fail "author"
[ "$(cat "$clone/version.txt")" = 0.3.14 ] || fail "version.txt"
[ -f "$clone/README.md" ] || fail "root README.md"

for k in "${kinds[@]}"; do
  folder="${k%%/*}" rest="${k#*/}" stem="${rest%/*}" ext="${rest##*/}"
  d="$clone/$folder"
  [ -f "$d/README.md" ] || fail "$folder/README.md"
  if [ "$folder" = android ]; then newest=0.3.14 expect="0.3.14 0.3.13 0.3.12 0.3.11"; else
    newest=0.3.13 expect="0.3.13 0.3.12 0.3.11 0.3.10"
  fi
  got="$(cd "$d" && ls -- "$stem"-[0-9]*."$ext" | sed -e "s/^$stem-//" -e "s/\\.$ext\$//" | sort -t. -k2,2nr -k3,3nr | tr '\n' ' ')"
  [ "$got" = "$expect " ] || fail "$folder/$stem versions: '$got', want '$expect'"
  cmp "$d/$stem.$ext" "$d/$stem-$newest.$ext" || fail "$folder/$stem.$ext is not the newest"
  grep -q "$stem-$newest.$ext" "$d/README.md" || fail "$folder/README.md lacks $stem-$newest.$ext"
done

for folder in macos windows linux android; do
  sums_check "$clone/$folder" || fail "SHA256SUMS in $folder"
  [ "$(grep -c . "$clone/$folder/SHA256SUMS")" = "$(find "$clone/$folder" -type f ! -name SHA256SUMS ! -name README.md | wc -l | tr -d ' ')" ] \
    || fail "SHA256SUMS in $folder does not list every file"
done

# The folders left out of the partial publication are the ones of the 0.3.13 commit.
prev_commit="$(git -C "$clone" rev-parse HEAD)"
publish 0.3.13 macos windows linux
git clone -q "$MESHTOWN_DIST_PUBLISH_URL" "$tmp/again"
for folder in macos windows linux; do
  diff -r "$clone/$folder" "$tmp/again/$folder" > /dev/null || fail "$folder changed by a partial publication"
done
[ "$prev_commit" != "$(git -C "$tmp/again" rev-parse HEAD)" ] || fail "no new commit"
[ -f "$tmp/again/android/meshtown-0.3.14.apk" ] || fail "android 0.3.14 lost"

# A partial publication leaves the other folders as they were: android has to survive the one above, too.
publish 0.3.15 android
git clone -q "$MESHTOWN_DIST_PUBLISH_URL" "$tmp/last"
diff -r "$tmp/again/macos" "$tmp/last/macos" > /dev/null || fail "macos changed by android 0.3.15"
[ ! -f "$tmp/last/android/meshtown-0.3.11.apk" ] || fail "android 0.3.11 must be pruned"
[ -f "$tmp/last/android/meshtown-0.3.15.apk" ] || fail "android 0.3.15 missing"

echo "publish_test: ok"
