#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="$ROOT/dist"
SKIP_BUILD=0

usage() {
  cat <<'EOF'
Build the native Linux CLI and GTK application as a Debian package.

Usage: scripts/build_linux_app.sh [--output DIR] [--skip-build]

Environment:
  CARGO                       Cargo executable (default: cargo from PATH)
  BFD_LINUX_GUI_FEATURES      Cargo features (default: linux-gui)
  BFD_BUILD_COMMIT            Commit recorded by build.rs
EOF
}

while (($#)); do
  case "$1" in
    --output)
      OUTPUT="${2:?--output requires a directory}"
      shift 2
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$(uname -s)" != Linux ]]; then
  printf 'The Linux application package must be built on Linux.\n' >&2
  exit 1
fi

for command in cargo dpkg-deb python3 sha256sum; do
  command -v "$command" >/dev/null || {
    printf 'Missing prerequisite: %s\n' "$command" >&2
    exit 1
  }
done

if command -v magick >/dev/null; then
  IMAGE_TOOL=(magick)
elif command -v convert >/dev/null; then
  IMAGE_TOOL=(convert)
else
  printf 'ImageMagick is required to prepare the desktop icon.\n' >&2
  exit 1
fi

CARGO_BIN="${CARGO:-cargo}"
FEATURES="${BFD_LINUX_GUI_FEATURES:-linux-gui}"
if [[ "$SKIP_BUILD" == 0 ]]; then
  "$CARGO_BIN" build \
    --manifest-path "$ROOT/Cargo.toml" \
    --release \
    --features "$FEATURES" \
    --bin burst-frame-deduplicator \
    --bin burst-frame-deduplicator-gtk
fi

VERSION="$($CARGO_BIN metadata --manifest-path "$ROOT/Cargo.toml" --no-deps --format-version 1 \
  | python3 -c 'import json,sys; data=json.load(sys.stdin); print(next(p["version"] for p in data["packages"] if p["name"] == "burst-frame-deduplicator"))')"
TARGET_DIR="$($CARGO_BIN metadata --manifest-path "$ROOT/Cargo.toml" --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
case "$(uname -m)" in
  x86_64) ARCH=amd64 ;;
  aarch64|arm64) ARCH=arm64 ;;
  *)
    printf 'Unsupported Debian architecture: %s\n' "$(uname -m)" >&2
    exit 1
    ;;
esac

WORK="$(mktemp -d "${TMPDIR:-/tmp}/bfd-linux-package.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
ROOTFS="$WORK/root"
PACKAGE="burst-frame-deduplicator_${VERSION}_${ARCH}.deb"
mkdir -p \
  "$ROOTFS/DEBIAN" \
  "$ROOTFS/usr/bin" \
  "$ROOTFS/usr/lib/burst-frame-deduplicator" \
  "$ROOTFS/usr/share/applications" \
  "$ROOTFS/usr/share/doc/burst-frame-deduplicator" \
  "$ROOTFS/usr/share/icons/hicolor/512x512/apps" \
  "$ROOTFS/usr/share/metainfo" \
  "$OUTPUT"

install -m 0755 "$TARGET_DIR/release/burst-frame-deduplicator" "$ROOTFS/usr/bin/"
install -m 0755 "$TARGET_DIR/release/burst-frame-deduplicator-gtk" "$ROOTFS/usr/bin/"
install -m 0755 "$ROOT/scripts/install_linux_ml_models.sh" \
  "$ROOTFS/usr/lib/burst-frame-deduplicator/"
install -m 0644 "$ROOT/packaging/linux/org.burstframe.Deduplicator.desktop" \
  "$ROOTFS/usr/share/applications/"
install -m 0644 "$ROOT/README.md" "$ROOT/LICENSE" "$ROOT/docs/LINUX_ML_MODELS.md" \
  "$ROOTFS/usr/share/doc/burst-frame-deduplicator/"
"${IMAGE_TOOL[@]}" "$ROOT/assets/app-icon.png" -resize 512x512 \
  "$ROOTFS/usr/share/icons/hicolor/512x512/apps/org.burstframe.Deduplicator.png"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@DATE@/$(date -u +%F)/g" \
  "$ROOT/packaging/linux/org.burstframe.Deduplicator.metainfo.xml.in" \
  > "$ROOTFS/usr/share/metainfo/org.burstframe.Deduplicator.metainfo.xml"

INSTALLED_SIZE="$(du -sk "$ROOTFS/usr" | awk '{print $1}')"
cat > "$ROOTFS/DEBIAN/control" <<EOF
Package: burst-frame-deduplicator
Version: $VERSION
Section: graphics
Priority: optional
Architecture: $ARCH
Installed-Size: $INSTALLED_SIZE
Maintainer: Burst Frame Deduplicator contributors
Depends: libc6 (>= 2.39), libgcc-s1, libgtk-4-1 (>= 4.14), libadwaita-1-0 (>= 1.5), libgdk-pixbuf-2.0-0, libraw23t64, imagemagick
Recommends: desktop-file-utils, hicolor-icon-theme, xdg-desktop-portal-gtk
Description: local burst-photo culling with recoverable review decisions
 Scores RAW and compressed burst frames, presents native GTK review controls,
 and keeps reject moves explicit and restorable.
EOF

cat > "$ROOTFS/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
EOF
cat > "$ROOTFS/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
EOF
chmod 0755 "$ROOTFS/DEBIAN/postinst" "$ROOTFS/DEBIAN/postrm"

dpkg-deb --root-owner-group --build "$ROOTFS" "$OUTPUT/$PACKAGE"
(
  cd "$OUTPUT"
  sha256sum "$PACKAGE" > "$PACKAGE.sha256"
  sha256sum --check "$PACKAGE.sha256"
)
printf 'Built %s\n' "$OUTPUT/$PACKAGE"
