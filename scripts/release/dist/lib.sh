#!/usr/bin/env bash
# Helpers of publish.sh: file names, versions, checksums and README tables of the distribution repository.
# A file of a folder is "<stem>-<version>.<ext>" (versioned) or "<stem>.<ext>" (the newest copy). A "kind" is the pair
# "<stem>|<ext>"; the extension is one of tar.gz, zip, exe, apk.

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

# kinds_in <dir>: the kinds ("stem|ext") of the versioned files in <dir>, one per line, sorted.
kinds_in() {
  local f name
  for f in "$1"/*; do
    name="${f##*/}"
    if [[ -f "$f" && "$name" =~ $dist_name_re ]]; then
      printf '%s|%s\n' "${BASH_REMATCH[1]}" "${BASH_REMATCH[3]}"
    fi
  done | sort -u
}

# versions_of <dir> <stem> <ext>: the versions of <stem>-<version>.<ext> in <dir>, newest first (numeric order).
versions_of() {
  local f name
  for f in "$1"/"$2"-[0-9]*."$3"; do
    name="${f##*/}"
    if [[ -f "$f" && "$name" =~ $dist_name_re && "${BASH_REMATCH[1]}" == "$2" && "${BASH_REMATCH[3]}" == "$3" ]]; then
      printf '%s\n' "${BASH_REMATCH[2]}"
    fi
  done | sort -t. -k1,1nr -k2,2nr -k3,3nr
}

# refresh_kind <dir> <stem> <ext> <keep>: removes all but the <keep> newest versions and makes <stem>.<ext> a copy
# of the newest one.
refresh_kind() {
  local v
  versions_of "$1" "$2" "$3" | tail -n "+$(($4 + 1))" | while IFS= read -r v; do rm -f "$1/$2-$v.$3"; done
  v="$(versions_of "$1" "$2" "$3" | head -n 1)"
  cp "$1/$2-$v.$3" "$1/$2.$3"
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

# versions_table <dir> <folder>: a markdown table "file -> link" of the versioned files, newest first per kind.
versions_table() {
  local kind stem ext v
  printf '| File | Link |\n|---|---|\n'
  while IFS= read -r kind; do
    stem="${kind%|*}" ext="${kind#*|}"
    printf '| `%s.%s` (latest) | %s/%s/%s.%s |\n' "$stem" "$ext" "$dist_base_url" "$2" "$stem" "$ext"
    while IFS= read -r v; do
      printf '| `%s-%s.%s` | %s/%s/%s-%s.%s |\n' "$stem" "$v" "$ext" "$dist_base_url" "$2" "$stem" "$v" "$ext"
    done < <(versions_of "$1" "$stem" "$ext")
  done < <(kinds_in "$1")
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
