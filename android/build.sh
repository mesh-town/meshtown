#!/usr/bin/env bash
# Builds the node, the tun2proxy bridge and the APK; copies the APK to dist/android/.
# Signing: with MESHTOWN_ANDROID_KEYSTORE, MESHTOWN_ANDROID_KEYSTORE_PASS and MESHTOWN_ANDROID_KEY_ALIAS set (and
# optionally MESHTOWN_ANDROID_KEY_PASS) a release APK is built and signed with that key; otherwise the debug APK.
# Needs: cargo-ndk, rustup target aarch64-linux-android, Android SDK + NDK, JDK 17+, git.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}"
if [ -z "${JAVA_HOME:-}" ] && [ -d "/Applications/Android Studio.app/Contents/jbr/Contents/Home" ]; then
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
fi
BUILD="${ANDROID_BUILD_DIR:-$ROOT/.tmp/build}"
DIST="${ANDROID_DIST_DIR:-$ROOT/dist/android}"
T2P_REF="${TUN2PROXY_REF:-657a263ecc1fe11be7ebb2ba9b360456df74b77d}" # tun2proxy 0.8.6
T2P_SRC="$BUILD/tun2proxy-src"
JNI="$HERE/app/src/main/jniLibs/arm64-v8a"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"

mkdir -p "$BUILD" "$JNI" "$DIST"

# Source paths must not leak into the libraries (panic messages, debug info): the last matching prefix wins,
# so the most specific ones come last.
RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo --remap-path-prefix=$ROOT=/src"
RUSTFLAGS="$RUSTFLAGS --remap-path-prefix=$T2P_SRC=/tun2proxy"
export RUSTFLAGS

# Fails when a library still holds the home directory of the build machine.
check_no_home() {
  if grep -a -q -F "$HOME/" "$1"; then
    echo "error: $1 contains build-machine paths" >&2
    exit 1
  fi
}

echo "== node"
(cd "$ROOT" && CARGO_TARGET_DIR="$BUILD/android" CARGO_PROFILE_RELEASE_PANIC=unwind \
  cargo ndk -t arm64-v8a -P 24 build --release --locked --no-default-features --features net-hyper)
cp "$BUILD/android/aarch64-linux-android/release/meshtown" "$JNI/libmeshtown.so"
check_no_home "$JNI/libmeshtown.so"

echo "== tun2proxy"
if [ ! -d "$T2P_SRC/.git" ]; then
  git clone -q https://github.com/tun2proxy/tun2proxy "$T2P_SRC"
fi
git -C "$T2P_SRC" fetch -q origin "$T2P_REF" 2>/dev/null || true
git -C "$T2P_SRC" checkout -q "$T2P_REF"
(cd "$T2P_SRC" && CARGO_TARGET_DIR="$BUILD/android-t2p" cargo ndk -t arm64-v8a -P 24 build --release --lib)
cp "$BUILD/android-t2p/aarch64-linux-android/release/libtun2proxy.so" "$JNI/libtun2proxy.so"
check_no_home "$JNI/libtun2proxy.so"

echo "== apk"
if [ -n "${MESHTOWN_ANDROID_KEYSTORE:-}" ] && [ -n "${MESHTOWN_ANDROID_KEYSTORE_PASS:-}" ] && [ -n "${MESHTOWN_ANDROID_KEY_ALIAS:-}" ]; then
  TASK=assembleRelease; OUT="$HERE/app/build/outputs/apk/release/app-release.apk"
else
  TASK=assembleDebug; OUT="$HERE/app/build/outputs/apk/debug/app-debug.apk"
fi
(cd "$HERE" && ./gradlew --no-daemon -q -PnodeVersion="$VERSION" "$TASK")
cp "$OUT" "$DIST/meshtown-$VERSION.apk"
echo "built $DIST/meshtown-$VERSION.apk"
