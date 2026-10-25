# Android

A Gradle application that runs the node as a service behind a system VPN. It is not a Cargo workspace member: the
node is built from `core/` as a native library for `arm64-v8a`.

## Layout

- `gui/` — the Gradle project (`app/`, `gradlew`, `test.sh`).
- `build/build.sh` — builds the node, the `tun2proxy` bridge and the APK, and copies the APK to `dist/android/`.

## Build

Run `platforms/android/build/build.sh` from the repository root. It needs `cargo-ndk`, the Rust target
`aarch64-linux-android`, the Android SDK and NDK, JDK 17+ and git. Environment variables:

| Variable | Meaning |
|---|---|
| `ANDROID_HOME`, `ANDROID_NDK_HOME` | SDK and NDK locations |
| `ANDROID_BUILD_DIR` | build directory (default `.tmp/build`) |
| `ANDROID_DIST_DIR` | where the APK is copied (default `dist/android`) |
| `TUN2PROXY_SRC_DIR` | where the `tun2proxy` source is cloned (default `$XDG_CACHE_HOME/meshtown/tun2proxy-src`, i.e. `~/.cache/meshtown/tun2proxy-src`); keep it outside the repository tree, otherwise cargo treats it as part of the workspace |
| `TUN2PROXY_REF` | the `tun2proxy` revision to build |
| `MESHTOWN_ANDROID_KEYSTORE`, `MESHTOWN_ANDROID_KEYSTORE_PASS`, `MESHTOWN_ANDROID_KEY_ALIAS`, `MESHTOWN_ANDROID_KEY_PASS` | signing key; without the first three a debug APK is built |

## Tests

`make android-test` runs the JVM unit tests of the shell (`platforms/android/gui/test.sh`): no emulator, NDK or Rust
is needed, only the Android SDK (`ANDROID_HOME`) and JDK 17+.

## What is used outside this directory

Only `core/` (the node sources), `scripts/release/lib.sh`, `scripts/release/dist/` and the workspace root (`Cargo.toml`, `Cargo.lock`,
`.tmp/`, `dist/`). `make boundary` checks this.
