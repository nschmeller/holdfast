#!/usr/bin/env bash
# Build HOLDFAST as an iOS xcframework.
#
#   tools/build-ios.sh              device + simulator
#   tools/build-ios.sh --device     device only (faster)
#
# Produces mobile/HoldfastCore.xcframework, which an Xcode app target links
# against. The Rust side exposes one symbol, `holdfast_main`; see mobile/README.
set -euo pipefail

cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

DEVICE=aarch64-apple-ios
SIM=aarch64-apple-ios-sim
OUT=mobile/HoldfastCore.xcframework

for target in "$DEVICE" "$SIM"; do
  rustup target list --installed | grep -q "$target" || rustup target add "$target"
done

echo "==> $DEVICE"
cargo build --lib --release --target "$DEVICE"

ARGS=(-library "target/$DEVICE/release/libholdfast.a")
if [[ "${1:-}" != "--device" ]]; then
  echo "==> $SIM"
  cargo build --lib --release --target "$SIM"
  ARGS+=(-library "target/$SIM/release/libholdfast.a")
fi

rm -rf "$OUT"
xcodebuild -create-xcframework "${ARGS[@]}" -output "$OUT"

echo
echo "==> $OUT"
echo "    Link it from an Xcode app target and call holdfast_main() from main."
echo "    See mobile/README.md."
