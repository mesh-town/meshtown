#!/usr/bin/env bash
# Runs the JVM unit tests of the Android shell (no NDK, Rust or emulator needed).
# Needs: Android SDK (for android.jar), JDK 17+, network access to Maven Central on the first run.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
if [ -z "${JAVA_HOME:-}" ] && [ -d "/Applications/Android Studio.app/Contents/jbr/Contents/Home" ]; then
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
fi

cd "$HERE"
./gradlew --no-daemon -q testDebugUnitTest
echo "Android unit tests passed"
