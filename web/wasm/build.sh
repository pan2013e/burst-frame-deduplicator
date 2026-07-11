#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIST="$ROOT/web/dist"
WASM_PACK="${WASM_PACK:-wasm-pack}"

rm -rf "$DIST"
mkdir -p "$DIST/vendor/libraw-wasm"

(
  cd "$ROOT/web/wasm"
  "$WASM_PACK" build --target web --release --out-dir ../dist/pkg
)

rsync -a "$ROOT/web/wasm/static/" "$DIST/"
rsync -a "$ROOT/web/vendor/libraw-wasm/" "$DIST/vendor/libraw-wasm/"

printf 'Built static app at %s\n' "$DIST"
