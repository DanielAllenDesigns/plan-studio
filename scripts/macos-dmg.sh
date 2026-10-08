#!/bin/sh
# Pack "Plan Studio.app" into a disk image with an Applications shortcut, so
# installing is "drag the app onto Applications".
#
# Usage: scripts/macos-dmg.sh [app-dir] [output-dir] [file-name]
#   app-dir     folder holding "Plan Studio.app" (default: $CARGO_TARGET_DIR, else
#               target, where scripts/macos-bundle.sh puts it)
#   output-dir  where the image goes (default: app-dir)
#   file-name   name of the image (default: "Plan Studio.dmg")
#
# Run scripts/macos-bundle.sh first, and sign the app before packing it (the
# release workflow does `codesign --force --deep -s -`). Needs macOS (hdiutil).
set -eu
APP_DIR="${1:-${CARGO_TARGET_DIR:-target}}"
OUT="${2:-$APP_DIR}"
NAME="${3:-Plan Studio.dmg}"
APP="$APP_DIR/Plan Studio.app"
DMG="$OUT/$NAME"
[ -d "$APP" ] || { echo "app bundle not found: $APP (run scripts/macos-bundle.sh first)"; exit 1; }
command -v hdiutil >/dev/null 2>&1 || { echo "hdiutil not found: the disk image can only be made on macOS"; exit 1; }
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
# ditto keeps the bundle's signature, symlinks and permissions intact.
ditto "$APP" "$STAGE/Plan Studio.app"
ln -s /Applications "$STAGE/Applications"
mkdir -p "$OUT"
rm -f "$DMG"
hdiutil create -volname "Plan Studio" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG" >/dev/null
echo "created: $DMG"
