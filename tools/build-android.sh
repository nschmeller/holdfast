#!/usr/bin/env bash
# Build HOLDFAST as an Android shared library.
#
# Requires the Android NDK and cargo-ndk:
#   brew install --cask android-ndk        (or Android Studio > SDK > NDK)
#   cargo install cargo-ndk
#   export ANDROID_NDK_HOME=/path/to/ndk/<version>
set -euo pipefail

cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  echo "ANDROID_NDK_HOME is not set." >&2
  echo "The NDK supplies the linker and the C toolchain that blake3 and wgpu" >&2
  echo "need; without it the build fails in a dependency's build script and" >&2
  echo "the error does not mention Android at all." >&2
  exit 1
fi
command -v cargo-ndk >/dev/null || { echo "cargo install cargo-ndk" >&2; exit 1; }

# arm64 covers every Android device worth shipping a 3D game to; add
# armeabi-v7a here if you need to support hardware from before 2017.
ABIS=(arm64-v8a)
OUT=mobile/android/app/src/main/jniLibs

echo "==> building for ${ABIS[*]}"
cargo ndk -o "$OUT" $(printf -- '-t %s ' "${ABIS[@]}") build --lib --release

echo
echo "==> $OUT"
echo "    Wrap it with a GameActivity project; see mobile/README.md."
