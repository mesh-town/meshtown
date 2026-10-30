#!/usr/bin/env bash
# winget_test.sh: runs winget.sh render/submit (submit with a fake `komac` in PATH) and checks the output.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fail() { echo "winget_test: FAIL: $*" >&2; exit 1; }

mkdir -p "$tmp/bin"
cat > "$tmp/bin/komac" <<'K'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$FAKE_LOG"
K
chmod +x "$tmp/bin/komac"
printf 'installer bytes\n' > "$tmp/setup.exe"
want_sha="$(shasum -a 256 "$tmp/setup.exe" | cut -d' ' -f1 | tr 'a-f' 'A-F')"
url=https://github.com/mesh-town/meshtown-dist/releases/download/v1.2.3/meshtown-setup-1.2.3.exe

unset GITHUB_TOKEN WINGET_DRY_RUN MESHTOWN_DIST_REPO
export FAKE_LOG="$tmp/log"
: > "$FAKE_LOG"
base_path="$PATH"

dir="$tmp/out/manifests/m/MeshTown/MeshTown/1.2.3"
"$here/winget.sh" render 1.2.3 "$tmp/setup.exe" "$tmp/out" || fail "render failed"
[ "$(ls "$dir" | wc -l | tr -d ' ')" = 3 ] || fail "want three files in $dir"
for f in MeshTown.MeshTown.yaml MeshTown.MeshTown.locale.en-US.yaml MeshTown.MeshTown.installer.yaml; do
  [ -f "$dir/$f" ] || fail "no $f"
  grep -qx 'PackageVersion: 1.2.3' "$dir/$f" || fail "$f: PackageVersion"
  grep -q '@[A-Z_]*@' "$dir/$f" && fail "$f: placeholder left"
done
[ "$(cat "$dir"/*.yaml | grep -h '^ManifestVersion:' | sort -u | wc -l | tr -d ' ')" = 1 ] || fail "ManifestVersion differs"
# winget validate warns about a manifest without the schema header of its type and manifest version.
mv_="$(sed -n 's/^ManifestVersion: //p' "$dir/MeshTown.MeshTown.yaml")"
for pair in MeshTown.MeshTown.yaml:version MeshTown.MeshTown.locale.en-US.yaml:defaultLocale \
  MeshTown.MeshTown.installer.yaml:installer; do
  grep -qxF "# yaml-language-server: \$schema=https://aka.ms/winget-manifest.${pair#*:}.$mv_.schema.json" \
    "$dir/${pair%%:*}" || fail "${pair%%:*}: schema header"
done
inst="$dir/MeshTown.MeshTown.installer.yaml"
grep -qx "  InstallerUrl: $url" "$inst" || fail "InstallerUrl"
grep -qx "  InstallerSha256: $want_sha" "$inst" || fail "InstallerSha256, want $want_sha"
grep -qx '  ProductCode: MeshTown' "$inst" || fail "ProductCode"
grep -qx 'Scope: machine' "$inst" || fail "Scope"
grep -qx 'InstallerType: nullsoft' "$inst" || fail "InstallerType"
grep -qx 'ElevationRequirement: elevatesSelf' "$inst" || fail "ElevationRequirement"
grep -qx 'PackageIdentifier: MeshTown.MeshTown' "$dir/MeshTown.MeshTown.yaml" || fail "PackageIdentifier"
loc="$dir/MeshTown.MeshTown.locale.en-US.yaml"
desc="$(sed -n 's/^description = "\(.*\)"$/\1/p' "$here/../../../core/Cargo.toml" | head -n 1)"
grep -qxF "ShortDescription: '$desc'" "$loc" || fail "ShortDescription differs from core/Cargo.toml"

# --base-url
"$here/winget.sh" render 1.2.3 "$tmp/setup.exe" "$tmp/out2" --base-url https://example.test/dl
grep -qx '  InstallerUrl: https://example.test/dl/meshtown-setup-1.2.3.exe' \
  "$tmp/out2/manifests/m/MeshTown/MeshTown/1.2.3/MeshTown.MeshTown.installer.yaml" || fail "--base-url"

# refusals: exit != 0 and nothing written
refuse() { # refuse <what> <args...>
  local what="$1"
  shift
  mkdir -p "$tmp/bad"
  if "$here/winget.sh" render "$@" "$tmp/bad" 2> /dev/null; then fail "$what: exit 0"; fi
  [ -z "$(ls -A "$tmp/bad")" ] || fail "$what: files written"
}
refuse "version 1.2" 1.2 "$tmp/setup.exe"
refuse "no installer" 1.2.3 "$tmp/missing.exe"

# submit
sub() { rc=0; out="$(PATH="$tmp/bin:$base_path" "$here/winget.sh" submit "$@" 2>&1)" || rc=$?; }
: > "$FAKE_LOG"
sub 1.2.3
[ "$rc" = 0 ] || fail "no token: exit $rc"
[ ! -s "$FAKE_LOG" ] || fail "no token: komac was called"
export GITHUB_TOKEN=t
: > "$FAKE_LOG"
sub 1.2.3
[ "$rc" = 0 ] || fail "token: exit $rc"
[ "$(cat "$FAKE_LOG")" = "update MeshTown.MeshTown --version 1.2.3 --urls $url --submit" ] || fail "komac argv: $(cat "$FAKE_LOG")"
: > "$FAKE_LOG"
WINGET_DRY_RUN=1 sub 1.2.3
[ "$(cat "$FAKE_LOG")" = "update MeshTown.MeshTown --version 1.2.3 --urls $url --dry-run" ] || fail "dry run argv: $(cat "$FAKE_LOG")"
rc=0
PATH="/usr/bin:/bin" "$here/winget.sh" submit 1.2.3 > /dev/null 2>&1 || rc=$?
[ "$rc" != 0 ] || fail "no komac: exit 0"

echo "winget_test: ok"
