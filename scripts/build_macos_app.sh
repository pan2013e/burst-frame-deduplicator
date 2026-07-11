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
RUSTC_BIN="${RUSTC:-$(command -v rustc || true)}"
CODE_SIGN_IDENTITY="${CODE_SIGN_IDENTITY:--}"

if [[ "$(uname -m)" != "arm64" ]]; then
  printf 'The macOS app is distributed for Apple Silicon only.\n' >&2
  exit 1
fi

if [[ -z "$CARGO" && -x "$HOME/.cargo/bin/cargo" ]]; then
  CARGO="$HOME/.cargo/bin/cargo"
fi
if [[ -z "$CARGO" ]]; then
  printf 'cargo was not found; set CARGO to its absolute path\n' >&2
  exit 1
fi
if [[ -z "$RUSTC_BIN" && -x "$HOME/.cargo/bin/rustc" ]]; then
  RUSTC_BIN="$HOME/.cargo/bin/rustc"
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
cp -R "$PACKAGE/Resources/en.lproj" "$PACKAGE/Resources/zh-Hans.lproj" "$RESOURCES/"

GIT_COMMIT="$(git -C "$ROOT" rev-parse --short=12 HEAD 2>/dev/null || printf unknown)"
if [[ -n "$(git -C "$ROOT" status --short 2>/dev/null || true)" ]]; then
  GIT_COMMIT="${GIT_COMMIT}-dirty"
fi
if [[ -n "$RUSTC_BIN" ]]; then
  RUST_VERSION="$("$RUSTC_BIN" --version 2>/dev/null || printf unknown)"
else
  RUST_VERSION="unknown"
fi
SWIFT_VERSION="$(swift --version 2>/dev/null | head -n 1 || printf unknown)"
if xcodebuild -version >/dev/null 2>&1; then
  CLT_VERSION="$(xcodebuild -version 2>/dev/null | tr '\n' ' ' | sed 's/[[:space:]]*$//')"
elif pkgutil --pkg-info=com.apple.pkg.CLTools_Executables >/dev/null 2>&1; then
  CLT_VERSION="Command Line Tools $(pkgutil --pkg-info=com.apple.pkg.CLTools_Executables | awk '/^version:/ { print $2 }')"
else
  CLT_VERSION="$(xcrun clang --version 2>/dev/null | head -n 1 || printf unknown)"
fi

/usr/libexec/PlistBuddy -c "Add :BFDGitCommit string $GIT_COMMIT" "$CONTENTS/Info.plist"
/usr/libexec/PlistBuddy -c "Add :BFDRustVersion string $RUST_VERSION" "$CONTENTS/Info.plist"
/usr/libexec/PlistBuddy -c "Add :BFDSwiftVersion string $SWIFT_VERSION" "$CONTENTS/Info.plist"
/usr/libexec/PlistBuddy -c "Add :BFDCLTVersion string $CLT_VERSION" "$CONTENTS/Info.plist"

SIGN_ARGS=(--force --sign "$CODE_SIGN_IDENTITY")
if [[ "$CODE_SIGN_IDENTITY" != "-" ]]; then
  SIGN_ARGS+=(--options runtime --timestamp)
fi
codesign "${SIGN_ARGS[@]}" "$FRAMEWORKS/libburst_frame_deduplicator.dylib"
codesign "${SIGN_ARGS[@]}" "$MACOS/BurstFrameDeduplicator"
codesign "${SIGN_ARGS[@]}" "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

printf 'Built %s\n' "$APP"
if [[ "$CODE_SIGN_IDENTITY" == "-" ]]; then
  printf 'Signing: ad hoc (local testing only; use CODE_SIGN_IDENTITY for distribution)\n'
else
  printf 'Signing: %s with hardened runtime and secure timestamp\n' "$CODE_SIGN_IDENTITY"
fi
printf 'RAW decoding: Apple Camera RAW/ImageIO through /usr/bin/sips; ImageMagick is not bundled.\n'
