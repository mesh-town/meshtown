#!/usr/bin/env bash
# publish_test.sh: runs publish.sh against a local bare repository and checks the result.
# Publishes 0.3.8 ... 0.3.13, 0.3.13 again, then only android/ 0.3.14; with gpg also an apt repository in linux/debian/.
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
[ -f "$clone/.nojekyll" ] || fail ".nojekyll in the root"

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
  [ "$(grep -c . "$clone/$folder/SHA256SUMS")" = "$(find "$clone/$folder" -maxdepth 1 -type f ! -name SHA256SUMS ! -name README.md | wc -l | tr -d ' ')" ] \
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

[ -f "$tmp/last/.nojekyll" ] || fail ".nojekyll lost after a partial publication"

# The apt repository: linux/debian/ built from fake archives, signed with a throwaway key.
debian_test() {
  local py="$here/../../../platforms/linux/build/debian" keys="$tmp/keys" v arch rust remote head dclone
  mkdir -p "$keys"
  python3 "$py/test_support.py" key "$keys"
  export MESHTOWN_APT_KEY="$keys/secret.asc" MESHTOWN_APT_PUBKEY="$keys/meshtown.gpg"
  remote="$tmp/remote-deb.git"
  git init -q --bare -b main "$remote"
  export MESHTOWN_DIST_PUBLISH_URL="file://$remote"
  deb_publish() { # deb_publish <version> [with-linux-only]
    rm -rf "$tmp/src" "$tmp/assets"
    mkdir -p "$tmp/assets" "$tmp/bin"
    if [ "${2:-}" = partial ]; then
      make_src "$tmp/src" "$1" android
    else
      make_src "$tmp/src" "$1" linux
      for arch in x86_64:x86_64 aarch64:aarch64; do
        rust="${arch%:*}"
        rm -rf "$tmp/bin/pkg"
        mkdir -p "$tmp/bin/pkg/meshtown-$1-$rust-unknown-linux-musl"
        printf '#!/bin/sh\necho fake %s\n' "$1" > "$tmp/bin/pkg/meshtown-$1-$rust-unknown-linux-musl/meshtown"
        chmod +x "$tmp/bin/pkg/meshtown-$1-$rust-unknown-linux-musl/meshtown"
        tar czf "$tmp/assets/meshtown-$1-$rust-unknown-linux-musl.tar.gz" -C "$tmp/bin/pkg" .
      done
      "$here/build_debs.sh" "$tmp/assets" "$tmp/src/linux/debian" "$1" > /dev/null
    fi
    "$here/publish.sh" "$tmp/src" "$1" > /dev/null
  }
  for v in 0.3.8 0.3.9 0.3.10 0.3.11 0.3.12 0.3.13; do deb_publish "$v"; done
  dclone="$tmp/dclone"
  git clone -q "$MESHTOWN_DIST_PUBLISH_URL" "$dclone"
  d="$dclone/linux/debian"
  for arch in amd64 arm64; do
    [ "$(ls "$d/pool/main/m/meshtown/"meshtown_*_"$arch".deb | wc -l | tr -d ' ')" = 4 ] || fail "pool has not 4 $arch packages"
    [ -f "$d/pool/main/m/meshtown/meshtown_0.3.13_$arch.deb" ] || fail "pool lacks 0.3.13 $arch"
    [ ! -f "$d/pool/main/m/meshtown/meshtown_0.3.9_$arch.deb" ] || fail "pool keeps 0.3.9 $arch"
    cmp "$d/meshtown_$arch.deb" "$d/pool/main/m/meshtown/meshtown_0.3.13_$arch.deb" || fail "meshtown_$arch.deb is not the newest"
  done
  gpgv --keyring "$keys/meshtown.gpg" "$d/dists/stable/InRelease" > /dev/null 2>&1 || fail "InRelease does not verify"
  grep -q "meshtown_0.3.13_amd64.deb" "$d/README.md" || fail "debian/README.md lacks 0.3.13"
  sums_check "$d" || fail "SHA256SUMS in linux/debian"
  sums_check "$dclone/linux" || fail "SHA256SUMS in linux with debian"
  [ -f "$dclone/.nojekyll" ] || fail ".nojekyll in the apt root"
  # A publication without linux/debian/ leaves it as it was.
  deb_publish 0.3.14 partial
  git clone -q "$MESHTOWN_DIST_PUBLISH_URL" "$tmp/dagain"
  diff -r "$d" "$tmp/dagain/linux/debian" > /dev/null || fail "linux/debian changed by a partial publication"
  # No key: refused, the remote does not move.
  head="$(git -C "$remote" rev-parse main)"
  unset MESHTOWN_APT_KEY
  rm -rf "$tmp/src"
  make_src "$tmp/src" 0.3.15 linux
  mkdir -p "$tmp/src/linux/debian"
  cp "$d"/meshtown_amd64.deb "$tmp/src/linux/debian/meshtown_0.3.15_amd64.deb"
  if "$here/publish.sh" "$tmp/src" 0.3.15 > /dev/null 2>&1; then fail "published linux/debian without a key"; fi
  [ "$head" = "$(git -C "$remote" rev-parse main)" ] || fail "the remote moved after a refused publication"
}

if command -v gpg > /dev/null 2>&1 && command -v gpgv > /dev/null 2>&1 && command -v gpgconf > /dev/null 2>&1; then
  debian_test
else
  echo "publish_test: skip debian (no gpg)"
fi

echo "publish_test: ok"
