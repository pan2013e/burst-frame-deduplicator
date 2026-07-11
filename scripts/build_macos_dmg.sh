#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/target/macos/Burst Frame Deduplicator.app"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$ROOT/macos/BurstFrameDeduplicatorApp/Info.plist")"
DMG="${DMG_OUTPUT:-$ROOT/target/macos/Burst-Frame-Deduplicator-${VERSION}.dmg}"
VOLUME_NAME="Burst Frame Deduplicator"
CODE_SIGN_IDENTITY="${CODE_SIGN_IDENTITY:--}"

"$ROOT/scripts/build_macos_app.sh"

mkdir -p "$ROOT/target/macos"
STAGING="$(mktemp -d "$ROOT/target/macos/dmg-stage.XXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
rm -f "$DMG"
hdiutil create \
  -volname "$VOLUME_NAME" \
  -srcfolder "$STAGING" \
  -format UDZO \
  -ov \
  "$DMG"

if [[ "$CODE_SIGN_IDENTITY" != "-" ]]; then
  codesign --force --sign "$CODE_SIGN_IDENTITY" --timestamp "$DMG"
fi

if [[ -n "${NOTARY_PROFILE:-}" ]]; then
  if [[ "$CODE_SIGN_IDENTITY" == "-" ]]; then
    printf 'NOTARY_PROFILE requires a Developer ID CODE_SIGN_IDENTITY.\n' >&2
    exit 1
  fi
  xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG"
  xcrun stapler validate "$DMG"
fi

printf 'Built %s\n' "$DMG"
if [[ "$CODE_SIGN_IDENTITY" == "-" ]]; then
  printf 'This DMG is ad-hoc signed for local testing and is not ready for public distribution.\n'
elif [[ -z "${NOTARY_PROFILE:-}" ]]; then
  printf 'The DMG is signed but not notarized; set NOTARY_PROFILE for public distribution.\n'
else
  printf 'The DMG is signed, notarized, and stapled.\n'
fi
