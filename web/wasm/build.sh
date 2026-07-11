#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIST="$ROOT/web/dist"
WASM_PACK="${WASM_PACK:-wasm-pack}"

if ! command -v cargo >/dev/null 2>&1 && [[ -x "$HOME/.cargo/bin/cargo" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

rm -rf "$DIST"
mkdir -p "$DIST/vendor/libraw-wasm"
mkdir -p "$DIST/locales"

(
  cd "$ROOT/web/wasm"
  "$WASM_PACK" build --target web --release --out-dir ../dist/pkg
)

rsync -a "$ROOT/web/wasm/static/" "$DIST/"
rsync -a "$ROOT/web/vendor/libraw-wasm/" "$DIST/vendor/libraw-wasm/"
rsync -a "$ROOT/locales/" "$DIST/locales/"

printf 'Built static app at %s\n' "$DIST"
