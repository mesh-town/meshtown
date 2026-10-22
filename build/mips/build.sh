#!/bin/sh
# Console build of meshtown for 32-bit MIPS routers (static musl, soft-float), on Linux x86_64.
# Usage: build/mips/build.sh mipsel-unknown-linux-musl|mips-unknown-linux-musl
# Both targets are tier 3: there is no prebuilt std, so the standard library is built from source
# (nightly + rust-src, -Zbuild-std). musl and the C compiler (needed by `ring`) come from a
# soft-float musl cross toolchain (cross-tools/musl-cross release, pinned by version and sha256), which is
# downloaded into MIPS_TOOLCHAIN_DIR when missing; the archive is unpacked with xz.
# Environment: MIPS_TOOLCHAIN_DIR (default: .tmp/mips-toolchain), CARGO_TARGET_DIR (default: target),
#   RUSTFLAGS (extra flags, appended to +crt-static), RUST_NIGHTLY (default: nightly-2026-10-18),
#   MIPS_TOOLCHAIN_CACHE (directory keeping the downloaded toolchain archive, default: MIPS_TOOLCHAIN_DIR),
#   CARGO_LOCKED=0 (drop --locked, for a checkout whose dependencies are path-patched).
# Result: $CARGO_TARGET_DIR/<triple>/release/meshtown, stripped and checked.
set -eu

TC_RELEASE=20261001
TC_URL_BASE=https://github.com/cross-tools/musl-cross/releases/download/$TC_RELEASE

TARGET="${1:-}"
case "$TARGET" in
  mipsel-unknown-linux-musl) CROSS=mipsel-unknown-linux-muslsf; ENDIAN="little endian"; EXPECT_DATA=LSB
    TC_SHA256=93fcfc5e8d28096941df04c3c5bc56b653d8aa81f8634671f236e0f443a451b6 ;;
  mips-unknown-linux-musl)   CROSS=mips-unknown-linux-muslsf; ENDIAN="big endian";    EXPECT_DATA=MSB
    TC_SHA256=4da371158b22950376fe03e6494b451fdfed7ed473dcc363ecdac6d21126cbb9 ;;
  *) echo "usage: $0 mipsel-unknown-linux-musl|mips-unknown-linux-musl" >&2; exit 2 ;;
esac

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
TC_DIR="${MIPS_TOOLCHAIN_DIR:-$ROOT/.tmp/mips-toolchain}"
NIGHTLY="${RUST_NIGHTLY:-nightly-2026-10-18}"
CARGO_LOCKED_FLAG=--locked
[ "${CARGO_LOCKED:-1}" = 0 ] && CARGO_LOCKED_FLAG=""
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

if [ ! -x "$TC_DIR/$CROSS/bin/$CROSS-gcc" ]; then
  mkdir -p "$TC_DIR"
  echo "fetching $CROSS toolchain into $TC_DIR"
  TGZ="${MIPS_TOOLCHAIN_CACHE:-$TC_DIR}/$CROSS-$TC_RELEASE.tar.xz"
  mkdir -p "$(dirname "$TGZ")"
  if command -v sha256sum >/dev/null 2>&1; then SHA="sha256sum"; else SHA="shasum -a 256"; fi
  # A bounded number of attempts: an unreachable host must fail the build, not hang it.
  TRY=0
  while :; do
    GOT=""
    [ -f "$TGZ" ] && GOT="$($SHA "$TGZ" | cut -d' ' -f1)"
    [ "$GOT" = "$TC_SHA256" ] && break
    TRY=$((TRY + 1))
    if [ "$TRY" -gt 3 ]; then
      echo "cannot fetch the $CROSS toolchain from $TC_URL_BASE (expected sha256 $TC_SHA256, have '${GOT:-none}')" >&2
      exit 1
    fi
    rm -f "$TGZ"
    curl -fsSL --retry 3 --retry-all-errors --connect-timeout 30 --max-time 900 -o "$TGZ" \
      "$TC_URL_BASE/$CROSS.tar.xz" || echo "download attempt $TRY failed" >&2
  done
  tar -xJf "$TGZ" -C "$TC_DIR"
fi
BIN="$TC_DIR/$CROSS/bin"

if command -v rustup >/dev/null 2>&1; then
  rustup toolchain install "$NIGHTLY" --profile minimal --component rust-src
fi

UNDERSCORED="$(echo "$TARGET" | tr '-' '_')"
UPPER="$(echo "$UNDERSCORED" | tr 'a-z' 'A-Z')"
export "CC_$UNDERSCORED=$BIN/$CROSS-gcc"
export "AR_$UNDERSCORED=$BIN/$CROSS-ar"
export "CARGO_TARGET_${UPPER}_LINKER=$BIN/$CROSS-gcc"
export CARGO_TARGET_DIR="$TARGET_DIR"
# The toolchain, not rustc, supplies the C runtime objects (rustc ships none for tier 3 targets). std links a static
# `unwind` library; the toolchain provides the same symbols in libgcc_eh.a, so it is exposed under that name.
SHIM="$TC_DIR/$CROSS-shim"
mkdir -p "$SHIM"
cp "$("$BIN/$CROSS-gcc" -print-file-name=libgcc_eh.a)" "$SHIM/libunwind.a"
export RUSTFLAGS="-C target-feature=+crt-static -C link-self-contained=no -L native=$SHIM ${RUSTFLAGS:-}"
# sccache would look for the compiler by name in PATH; the toolchain is addressed by full path.
unset RUSTC_WRAPPER CARGO_BUILD_RUSTC_WRAPPER

cargo "+$NIGHTLY" build -Zbuild-std=std,panic_abort --release ${CARGO_LOCKED_FLAG} --target "$TARGET" \
  --no-default-features --features net-hyper

OUT="$TARGET_DIR/$TARGET/release/meshtown"
"$BIN/$CROSS-strip" "$OUT"

# Sanity checks: a static 32-bit MIPS executable with the expected byte order.
HDR="$("$BIN/$CROSS-readelf" -h "$OUT")"
echo "$HDR" | grep -q 'Machine:.*MIPS' || { echo "not a MIPS binary" >&2; exit 1; }
echo "$HDR" | grep -q "Class:.*ELF32" || { echo "not ELF32" >&2; exit 1; }
echo "$HDR" | grep -q "2's complement, $ENDIAN" || { echo "wrong byte order, expected $ENDIAN" >&2; exit 1; }
if "$BIN/$CROSS-readelf" -l "$OUT" | grep -q 'INTERP'; then echo "binary is dynamically linked" >&2; exit 1; fi
echo "ok: $OUT ($(wc -c < "$OUT") bytes, $EXPECT_DATA)"
