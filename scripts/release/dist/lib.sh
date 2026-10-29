#!/usr/bin/env bash
# Helpers of publish.sh: file names, checksums and README tables of the distribution repository.
# A build is published as "<stem>.<ext>" (no version in the name); "<stem>-<version>.<ext>" is the name of a build as
# it comes from the release, and the extension is one of tar.gz, zip, exe, apk.

dist_base_url="https://github.com/mesh-town/meshtown-dist/raw/main"
dist_pages_url="https://mesh-town.github.io/meshtown-dist"
dist_name_re='^(.+)-([0-9]+\.[0-9]+\.[0-9]+)\.(tar\.gz|zip|exe|apk)$'

# sha256_of <file>: the SHA-256 digest of a file (GNU sha256sum, or shasum on macOS).
sha256_of() {
  if command -v sha256sum > /dev/null 2>&1; then
    sha256sum -- "$1" | cut -d' ' -f1
  else
    shasum -a 256 -- "$1" | cut -d' ' -f1
  fi
}

# strip_versions <dir>: removes the versioned files (<stem>-<version>.<ext>) at the top level of <dir>.
strip_versions() {
  local f name
  for f in "$1"/*; do
    name="${f##*/}"
    if [[ -f "$f" && "$name" =~ $dist_name_re ]]; then
      rm -f -- "$f"
    fi
  done
}

# write_sums <dir>: SHA256SUMS of every file of <dir> except SHA256SUMS and README.md.
write_sums() {
  local f name
  : > "$1/SHA256SUMS"
  for f in "$1"/*; do
    name="${f##*/}"
    if [[ -f "$f" && "$name" != SHA256SUMS && "$name" != README.md ]]; then
      printf '%s  %s\n' "$(sha256_of "$f")" "$name" >> "$1/SHA256SUMS"
    fi
  done
}

# files_table <dir> <folder>: a markdown table "file -> link" of the files of <dir> (not SHA256SUMS and README.md).
files_table() {
  local f name
  printf '| File | Link |\n|---|---|\n'
  for f in "$1"/*; do
    name="${f##*/}"
    if [[ -f "$f" && "$name" != SHA256SUMS && "$name" != README.md ]]; then
      printf '%s\n' "$name"
    fi
  done | LC_ALL=C sort | while IFS= read -r name; do
    printf '| `%s` | %s/%s/%s |\n' "$name" "$dist_base_url" "$2" "$name"
  done
}

# render <template> <version> <table-file>: the template with @VERSION@ replaced and the line @TABLE@ replaced by the
# contents of <table-file>.
render() {
  local line
  while IFS= read -r line; do
    if [ "$line" = "@TABLE@" ]; then
      cat "$3"
    else
      printf '%s\n' "${line//@VERSION@/$2}"
    fi
  done < "$1"
}

# deb_table <dir> <url>: a markdown table "file -> link" of the apt repository folder <dir> published at <url>: the
# direct links to the newest packages, then the packages of the pool by version, newest first.
deb_table() {
  local arch v f name
  printf '| File | Link |\n|---|---|\n'
  for arch in amd64 arm64; do
    if [ -f "$1/meshtown_$arch.deb" ]; then
      printf '| `meshtown_%s.deb` (latest) | %s/meshtown_%s.deb |\n' "$arch" "$2" "$arch"
    fi
  done
  for arch in amd64 arm64; do
    for f in "$1"/pool/main/m/meshtown/meshtown_*_"$arch".deb; do
      [ -f "$f" ] || continue
      name="${f##*/}"
      v="${name#meshtown_}"
      printf '%s %s\n' "${v%%_*}" "$name"
    done | sort -t. -k1,1nr -k2,2nr -k3,3nr | while read -r _ name; do
      printf '| `%s` | %s/pool/main/m/meshtown/%s |\n' "$name" "$2" "$name"
    done
  done
}
