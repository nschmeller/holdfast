#!/usr/bin/env bash
# Build the itch.io bundle.
#
#   tools/build-web.sh          release build into dist/
#   tools/build-web.sh --serve  ...then serve it on :8080
#
# Produces dist/ containing index.html, holdfast.js and holdfast_bg.wasm, plus
# a holdfast-web.zip ready to drag onto itch.io. Nothing else: every mesh,
# material, sound and glyph in this game is generated at runtime, so there are
# no assets to ship.
set -euo pipefail

cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

TARGET=wasm32-unknown-unknown
PROFILE=web-release
OUT=dist

if ! rustup target list --installed | grep -q "$TARGET"; then
  echo "adding $TARGET"
  rustup target add "$TARGET"
fi
if ! command -v wasm-bindgen >/dev/null; then
  echo "wasm-bindgen-cli is required: cargo install wasm-bindgen-cli" >&2
  exit 1
fi

echo "==> building $PROFILE for $TARGET"
# getrandom never enters the tree - the PRNG is hand-rolled precisely so the
# web build needs no JS shim - so there is nothing to configure here beyond
# the target itself.
cargo build --profile "$PROFILE" --target "$TARGET"

WASM="target/$TARGET/$PROFILE/holdfast.wasm"
echo "==> binding ($(du -h "$WASM" | cut -f1) raw)"
rm -rf "$OUT"
mkdir -p "$OUT"
wasm-bindgen --no-typescript --target web --out-dir "$OUT" --out-name holdfast "$WASM"

cp web/index.html "$OUT/index.html"

# wasm-opt shrinks the bundle by a third or so when it is available. It is a
# nice-to-have, not a requirement, so a missing binaryen is a note rather than
# a failure.
if command -v wasm-opt >/dev/null; then
  echo "==> wasm-opt"
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    "$OUT/holdfast_bg.wasm" -o "$OUT/holdfast_bg.wasm"
else
  echo "    (wasm-opt not found; install binaryen for a smaller bundle)"
fi

( cd "$OUT" && zip -q -r ../holdfast-web.zip . )

echo
echo "==> done"
du -h "$OUT"/* | sed 's/^/    /'
echo "    $(du -h holdfast-web.zip | cut -f1)  holdfast-web.zip  <- upload this"
echo
echo "itch.io: 'This file will be played in the browser', viewport 1280x720,"
echo "         and tick 'Mobile friendly' off - this game is keyboard-only."

if [[ "${1:-}" == "--serve" ]]; then
  echo
  echo "==> http://localhost:8080"
  # Cross-origin isolation headers, in case a future build wants threads.
  cd "$OUT" && python3 -c "
import http.server, socketserver
class H(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        super().end_headers()
socketserver.TCPServer(('', 8080), H).serve_forever()
"
fi
