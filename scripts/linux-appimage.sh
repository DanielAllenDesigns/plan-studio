#!/bin/bash
# Package the Linux build: a tarball always, an AppImage when appimagetool is available.
#
# Usage: scripts/linux-appimage.sh [binary] [output-dir] [label]
#   binary      the built plan-studio (default: $CARGO_TARGET_DIR/release/plan-studio,
#               else target/release/plan-studio)
#   output-dir  where the packages go (default: dist)
#   label       file-name stem (default: plan-studio-v<version>-linux-<arch>)
#
# Produces <label>.tar.gz (the binary, .desktop file, 256 px icon, MIME definition
# and install.sh) and, if appimagetool is on PATH (or named by $APPIMAGETOOL),
# <label>.AppImage. Without appimagetool the AppImage step is skipped with a
# message and the script still succeeds: this repo's sandbox may not download it,
# the release workflow does (and treats that step as best effort).
#
# The AppImage is not self-contained: like the tarball it uses the system GTK 3,
# xkbcommon, X11/Wayland and OpenGL libraries (the app opens them at run time).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BIN="${1:-${CARGO_TARGET_DIR:-$ROOT/target}/release/plan-studio}"
OUT="${2:-dist}"
VERSION="$(sh "$HERE/version.sh")"
ARCH="$(uname -m)"
LABEL="${3:-plan-studio-v$VERSION-linux-$ARCH}"
[ -f "$BIN" ] || { echo "binary not found: $BIN (run cargo build --release -p plan-app first)" >&2; exit 1; }
ICON="$ROOT/crates/plan-app/assets/icons/app/icon-256.png"
[ -f "$ICON" ] || { echo "icon not found: $ICON" >&2; exit 1; }
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

# --- tarball ---------------------------------------------------------------
PKG="$STAGE/$LABEL"
mkdir -p "$PKG"
install -m755 "$BIN" "$PKG/plan-studio"
install -m644 "$HERE/linux/plan-studio.desktop" "$PKG/plan-studio.desktop"
install -m644 "$HERE/linux/plan-studio-psplan.xml" "$PKG/plan-studio-psplan.xml"
install -m644 "$ICON" "$PKG/plan-studio.png"
install -m755 "$HERE/linux/install.sh" "$PKG/install.sh"
install -m644 "$ROOT/LICENSE" "$PKG/LICENSE"
tar -C "$STAGE" -czf "$OUT/$LABEL.tar.gz" "$LABEL"
echo "created: $OUT/$LABEL.tar.gz"

# --- AppImage (optional) ---------------------------------------------------
TOOL="${APPIMAGETOOL:-}"
[ -n "$TOOL" ] || TOOL="$(command -v appimagetool 2>/dev/null || true)"
if [ -z "$TOOL" ]; then
  echo "appimagetool not found: skipping the AppImage (set APPIMAGETOOL or put it on PATH)"
  exit 0
fi
APPDIR="$STAGE/Plan Studio.AppDir"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps" "$APPDIR/usr/share/mime/packages"
install -m755 "$BIN" "$APPDIR/usr/bin/plan-studio"
install -m644 "$HERE/linux/plan-studio.desktop" "$APPDIR/plan-studio.desktop"
install -m644 "$HERE/linux/plan-studio.desktop" "$APPDIR/usr/share/applications/plan-studio.desktop"
install -m644 "$ICON" "$APPDIR/plan-studio.png"
install -m644 "$ICON" "$APPDIR/.DirIcon"
install -m644 "$ICON" "$APPDIR/usr/share/icons/hicolor/256x256/apps/plan-studio.png"
install -m644 "$HERE/linux/plan-studio-psplan.xml" "$APPDIR/usr/share/mime/packages/plan-studio-psplan.xml"
cat > "$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/plan-studio" "$@"
APPRUN
chmod 755 "$APPDIR/AppRun"
# --appimage-extract-and-run lets the tool itself run where FUSE is missing (CI runners).
ARCH="$ARCH" "$TOOL" --appimage-extract-and-run "$APPDIR" "$OUT/$LABEL.AppImage" >/dev/null 2>&1 \
  || ARCH="$ARCH" "$TOOL" "$APPDIR" "$OUT/$LABEL.AppImage"
echo "created: $OUT/$LABEL.AppImage"
