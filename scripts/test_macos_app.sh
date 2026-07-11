#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGE="$ROOT/macos/BurstFrameDeduplicatorApp"
TESTING_FRAMEWORKS="/Library/Developer/CommandLineTools/Library/Developer/Frameworks"
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

arguments=(swift test --package-path "$PACKAGE")
if [[ -d "$TESTING_FRAMEWORKS/Testing.framework" ]]; then
  arguments+=(
    -Xswiftc "-F$TESTING_FRAMEWORKS"
    -Xlinker -rpath
    -Xlinker "$TESTING_FRAMEWORKS"
  )
fi
"${arguments[@]}"
