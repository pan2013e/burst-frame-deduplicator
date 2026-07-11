#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGE="$ROOT/macos/BurstFrameDeduplicatorApp"
APP="$ROOT/target/macos/Burst Frame Deduplicator.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
FRAMEWORKS="$CONTENTS/Frameworks"
RESOURCES="$CONTENTS/Resources"
CARGO="${CARGO:-$(command -v cargo || true)}"

if [[ -z "$CARGO" && -x "$HOME/.cargo/bin/cargo" ]]; then
  CARGO="$HOME/.cargo/bin/cargo"
fi
if [[ -z "$CARGO" ]]; then
  printf 'cargo was not found; set CARGO to its absolute path\n' >&2
  exit 1
fi

cd "$ROOT"
"$CARGO" build --release --lib
install_name_tool -id @rpath/libburst_frame_deduplicator.dylib \
  "$ROOT/target/release/libburst_frame_deduplicator.dylib"

swift build -c release --package-path "$PACKAGE"
SWIFT_BIN="$(swift build -c release --show-bin-path --package-path "$PACKAGE")"

rm -rf "$APP"
mkdir -p "$MACOS" "$FRAMEWORKS" "$RESOURCES/locales"
cp "$SWIFT_BIN/BurstFrameDeduplicator" "$MACOS/BurstFrameDeduplicator"
cp "$ROOT/target/release/libburst_frame_deduplicator.dylib" "$FRAMEWORKS/"
cp "$PACKAGE/Info.plist" "$CONTENTS/Info.plist"
cp "$PACKAGE/Resources/AppIcon.icns" "$RESOURCES/AppIcon.icns"
cp "$ROOT/locales/en.json" "$ROOT/locales/zh-CN.json" "$RESOURCES/locales/"

codesign --force --sign - "$FRAMEWORKS/libburst_frame_deduplicator.dylib"
codesign --force --sign - "$MACOS/BurstFrameDeduplicator"
codesign --force --sign - "$APP"

printf 'Built %s\n' "$APP"
