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
mkdir -p "$DIST/models"

(
  cd "$ROOT/web/wasm"
  "$WASM_PACK" build --target web --release --out-dir ../dist/pkg
)

(
  cd "$ROOT/web/ml-wasm"
  "$WASM_PACK" build --target web --release --out-dir ../dist/ml-pkg
)

rsync -a "$ROOT/web/wasm/static/" "$DIST/"
cp "$ROOT/web/shared/tutorial-progress.mjs" "$DIST/tutorial-progress.mjs"
rsync -a "$ROOT/web/vendor/libraw-wasm/" "$DIST/vendor/libraw-wasm/"
rsync -a "$ROOT/locales/" "$DIST/locales/"
cp "$ROOT/web/ml-wasm/assets/u2netp.bpk" "$DIST/models/u2netp.bpk"
cp "$ROOT/web/ml-wasm/NOTICE.md" "$DIST/models/U2NET-P-NOTICE.md"
cp "$ROOT/web/ml-wasm/LICENSE-U2NET.txt" "$DIST/models/U2NET-P-LICENSE.txt"

APP_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
COMMIT="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || printf unknown)"
RUSTC_VERSION="$(rustc --version 2>/dev/null || printf unknown)"
CARGO_VERSION="$(cargo --version 2>/dev/null || printf unknown)"
WASM_PACK_VERSION="$("$WASM_PACK" --version 2>/dev/null || printf unknown)"
cat > "$DIST/build-info.json" <<EOF
{
  "mode": "wasm",
  "app_version": "$APP_VERSION",
  "commit": "$COMMIT",
  "rustc": "$RUSTC_VERSION",
  "cargo": "$CARGO_VERSION",
  "wasm_pack": "$WASM_PACK_VERSION",
  "build_target": "wasm32-unknown-unknown"
}
EOF

printf 'Built static app at %s\n' "$DIST"
