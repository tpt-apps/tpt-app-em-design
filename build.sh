#!/usr/bin/env sh
# TPT EM Design - build pipeline (macOS/Linux).
#
# Free edition (default):  ./build.sh
# Pro desktop edition:     ./build.sh --pro
#   (adds trunk build --release --features pro + the native webview exe)
#
# Env:
#   TPT_WEB_REPO  path to the web repo; the bundle is copied to
#                 $TPT_WEB_REPO/public/apps/em-design/ (glue named
#                 em-design.js, wasm keeps its _bg.wasm name),
#                 otherwise dist/web in this repo.

set -eu
cd "$(dirname "$0")"

CRATE=tpt-app-em-design
DESKTOP_CRATE=tpt-em-design-desktop
TARGET=wasm32-unknown-unknown
OUT_NAME=em-design

PRO=0
for arg in "$@"; do
  case "$arg" in
    --pro) PRO=1 ;;
    *) echo "usage: build.sh [--pro]" >&2 ; exit 2 ;;
  esac
done

echo "== TPT EM Design build =="

if [ "$PRO" = 1 ]; then
  echo "-- trunk build --release --features pro (standalone bundle)"
  trunk build --release --features pro
  echo "-- cargo build --release -p $DESKTOP_CRATE (webview exe)"
  cargo build --release -p "$DESKTOP_CRATE"
  echo "-- done (public/apps/em-design/ untouched)"
  exit 0
fi

if [ -n "${TPT_WEB_REPO:-}" ]; then
  DEST="$TPT_WEB_REPO/public/apps/em-design"
else
  DEST="$PWD/dist/web"
fi
echo " dest: $DEST"

echo "-- cargo build --release --target $TARGET"
cargo build --release --target "$TARGET" -p "$CRATE"

WASM="target/$TARGET/release/tpt_app_em_design.wasm"
[ -f "$WASM" ] || { echo "missing $WASM" >&2; exit 1; }

mkdir -p "$DEST"
wasm-bindgen --target web --out-name "$OUT_NAME" --out-dir "$DEST" "$WASM"

RAW=$(wc -c < "$DEST/${OUT_NAME}_bg.wasm" | tr -d ' ')
GZ=$(gzip -9 -c "$DEST/${OUT_NAME}_bg.wasm" | wc -c | tr -d ' ')
echo "-- ${OUT_NAME}_bg.wasm: $RAW bytes raw, $GZ bytes gzipped"

echo "-- done"
