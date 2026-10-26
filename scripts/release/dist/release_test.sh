#!/usr/bin/env bash
# release_test.sh: runs release.sh with a fake `gh` in PATH and checks the calls it makes.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fail() { echo "release_test: FAIL: $*" >&2; exit 1; }

mkdir -p "$tmp/bin" "$tmp/assets"
cat > "$tmp/bin/gh" <<'G'
#!/usr/bin/env bash
# Fake gh: logs argv (and stdin of --input -); answers by FAKE_REF, FAKE_RELEASE, FAKE_FAIL_CREATE (1 = fail/missing).
printf '%s\n' "$*" >> "$FAKE_LOG"
case "$*" in *"--input -"*) printf 'STDIN %s\n' "$(cat)" >> "$FAKE_LOG" ;; esac
case "$*" in
  "api repos/"*"/git/ref/heads/"*) [ "${FAKE_REF:-1}" = 1 ] || exit 1 ;;
  "release view "*) [ "${FAKE_RELEASE:-0}" = 1 ] || exit 1 ;;
  "release create "*) [ "${FAKE_FAIL_CREATE:-0}" = 0 ] || exit 1 ;;
  "api "*) echo deadbeef ;;
esac
G
chmod +x "$tmp/bin/gh"
printf 'x\n' > "$tmp/assets/a-0.3.9.zip"
printf 'y\n' > "$tmp/assets/b-0.3.9.tar.gz"
printf 'z\n' > "$tmp/assets/SHA256SUMS"
mkdir "$tmp/nosums"
printf 'x\n' > "$tmp/nosums/a.zip"

export PATH="$tmp/bin:$PATH"
export FAKE_LOG="$tmp/log"
unset GITHUB_REPOSITORY GITHUB_OUTPUT MESHTOWN_DIST_REPO MESHTOWN_DIST_ANCHOR FAKE_REF FAKE_RELEASE FAKE_FAIL_CREATE

# run <args...>: release.sh with a token and a fresh log; sets rc and out.
run() {
  : > "$FAKE_LOG"
  : > "$tmp/output"
  rc=0
  out="$(GH_TOKEN=t GITHUB_OUTPUT="$tmp/output" "$here/release.sh" "$@" 2>&1)" || rc=$?
}
has() { grep -qF -- "$1" "$FAKE_LOG" || fail "$2: no '$1' in the gh calls: $(cat "$FAKE_LOG")"; }
hasnt() { if grep -qF -- "$1" "$FAKE_LOG"; then fail "$2: unexpected '$1'"; fi; }
expect_usage() { # expect_usage <what> <args...>
  local what="$1"
  shift
  run "$@"
  [ "$rc" = 2 ] || fail "$what: exit $rc, want 2"
  [ ! -s "$FAKE_LOG" ] || fail "$what: gh was called"
}

# usage errors
expect_usage "no arguments"
expect_usage "version 1.2" "$tmp/assets" 1.2
expect_usage "version v1.2.3" "$tmp/assets" v1.2.3
expect_usage "no SHA256SUMS" "$tmp/nosums" 0.3.9

# no token: nothing is called, the output names the current repository
: > "$FAKE_LOG"
: > "$tmp/output"
rc=0
out="$(env -u GH_TOKEN GITHUB_REPOSITORY=a/m GITHUB_OUTPUT="$tmp/output" "$here/release.sh" "$tmp/assets" 0.3.9)" || rc=$?
[ "$rc" = 0 ] || fail "no token: exit $rc"
[ ! -s "$FAKE_LOG" ] || fail "no token: gh was called"
case "$out" in *"::notice::"*) ;; *) fail "no token: no notice: $out" ;; esac
grep -qx 'repo=a/m' "$tmp/output" || fail "no token: output is $(cat "$tmp/output")"

# the anchor branch is missing: blob, tree, commit and ref are created
FAKE_REF=0 run "$tmp/assets" 0.3.9
[ "$rc" = 0 ] || fail "new anchor: exit $rc: $out"
has "repos/mesh-town/meshtown-dist/git/blobs" "new anchor"
has "repos/mesh-town/meshtown-dist/git/trees" "new anchor"
has "repos/mesh-town/meshtown-dist/git/commits" "new anchor"
has "refs/heads/releases" "new anchor"
commit_line="$(grep '^STDIN .*"parents"' "$FAKE_LOG")" || fail "new anchor: no commit body"
case "$commit_line" in *'"parents":[]'*) ;; *) fail "new anchor: the commit has parents" ;; esac
mail='340663027+mesh-town@users.noreply.github.com'
case "$commit_line" in *'"author":{"name":"Mesh Town","email":"'$mail'"}'*) ;; *) fail "new anchor: author: $commit_line" ;; esac
case "$commit_line" in *'"committer":{"name":"Mesh Town","email":"'$mail'"}'*) ;; *) fail "new anchor: committer" ;; esac

# the anchor branch exists: nothing is created
run "$tmp/assets" 0.3.9
hasnt "git/blobs" "existing anchor"
hasnt "git/commits" "existing anchor"
hasnt "git/refs" "existing anchor"

# a new release
run "$tmp/assets" 0.3.9
[ "$rc" = 0 ] || fail "new release: exit $rc: $out"
create="$(grep '^release create ' "$FAKE_LOG")" || fail "new release: no release create"
for want in "release create v0.3.9" "$tmp/assets/a-0.3.9.zip" "$tmp/assets/b-0.3.9.tar.gz" "$tmp/assets/SHA256SUMS" \
  "--target releases" "--repo mesh-town/meshtown-dist" "--latest"; do
  case "$create" in *"$want"*) ;; *) fail "new release: no '$want' in: $create" ;; esac
done
case "$create" in *--generate-notes*) fail "new release: --generate-notes" ;; esac
grep -qx 'repo=mesh-town/meshtown-dist' "$tmp/output" || fail "new release: output is $(cat "$tmp/output")"

# an existing release: upload with --clobber, then publish the draft
FAKE_RELEASE=1 run "$tmp/assets" 0.3.9
[ "$rc" = 0 ] || fail "existing release: exit $rc: $out"
has "release upload v0.3.9" "existing release"
has "--clobber" "existing release"
has "release edit v0.3.9" "existing release"
has "--draft=false" "existing release"
hasnt "release create" "existing release"

# another repository and anchor
MESHTOWN_DIST_REPO=a/b MESHTOWN_DIST_ANCHOR=anc FAKE_REF=0 run "$tmp/assets" 0.3.9
[ "$rc" = 0 ] || fail "other repository: exit $rc: $out"
while IFS= read -r line; do
  case "$line" in
    STDIN*) ;;
    *"repos/a/b/"*|*"--repo a/b"*) ;;
    *) fail "other repository: call without a/b: $line" ;;
  esac
done < "$FAKE_LOG"
has "--target anc" "other repository"
grep -qx 'repo=a/b' "$tmp/output" || fail "other repository: output"

# a failing gh makes the script fail
FAKE_FAIL_CREATE=1 run "$tmp/assets" 0.3.9
[ "$rc" != 0 ] || fail "failing create: exit 0"

echo "release_test: ok"
