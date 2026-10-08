#!/bin/sh
# Wrap the plan-studio binary in a minimal macOS .app bundle.
# Usage: scripts/macos-bundle.sh [debug|release] [output-dir]
set -eu
PROFILE="${1:-debug}"
OUT="${2:-${CARGO_TARGET_DIR:-target}}"
BIN="${CARGO_TARGET_DIR:-target}/$PROFILE/plan-studio"
APP="$OUT/Plan Studio.app"
[ -x "$BIN" ] || { echo "binary not found: $BIN (run cargo build -p plan-app first)"; exit 1; }
# The version comes from the workspace Cargo.toml so a bump ships in the plist.
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$(dirname "$0")/../Cargo.toml" | head -1)"
VERSION="${VERSION:-0.1.0}"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$BIN" "$APP/Contents/MacOS/plan-studio"
# The app icon: crates/plan-app/assets/icons/app/AppIcon.icns (made by
# `cargo run -p plan-app --example gen_app_icon`) becomes both the application
# icon and the icon of .psplan documents. A scripts/AppIcon.icns overrides it.
# With only the PNG sizes next to it and iconutil on the machine, the .icns is
# built here; with neither, the bundle has no icon.
ROOT="$(dirname "$0")/.."
ICONS="$ROOT/crates/plan-app/assets/icons/app"
ICON=""
for c in "$(dirname "$0")/AppIcon.icns" "$ICONS/AppIcon.icns" "$ROOT/crates/plan-app/assets/AppIcon.icns"; do
  [ -f "$c" ] && ICON="$c" && break
done
if [ -z "$ICON" ] && [ -f "$ICONS/icon-1024.png" ] && command -v iconutil >/dev/null 2>&1; then
  SET="$(mktemp -d)/AppIcon.iconset"
  mkdir -p "$SET"
  for pair in 16:icon_16x16 32:icon_16x16@2x 32:icon_32x32 64:icon_32x32@2x \
              128:icon_128x128 256:icon_128x128@2x 256:icon_256x256 512:icon_256x256@2x \
              512:icon_512x512 1024:icon_512x512@2x; do
    cp "$ICONS/icon-${pair%%:*}.png" "$SET/${pair#*:}.png"
  done
  if iconutil -c icns -o "$SET/../AppIcon.icns" "$SET"; then ICON="$SET/../AppIcon.icns"; fi
fi
ICON_KEYS=""
DOC_ICON_KEY=""
if [ -n "$ICON" ]; then
  mkdir -p "$APP/Contents/Resources"
  cp "$ICON" "$APP/Contents/Resources/AppIcon.icns"
  ICON_KEYS="<key>CFBundleIconFile</key><string>AppIcon</string>"
  DOC_ICON_KEY="<key>CFBundleTypeIconFile</key><string>AppIcon</string>"
fi
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Plan Studio</string>
  <key>CFBundleDisplayName</key><string>Plan Studio</string>
  <key>CFBundleIdentifier</key><string>com.danielallendesigns.plan-studio</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>plan-studio</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHighResolutionCapable</key><true/>
  $ICON_KEYS
  <!-- .psplan documents: double-clicking one in Finder opens it in Plan Studio -->
  <key>CFBundleDocumentTypes</key>
  <array><dict>
    <key>CFBundleTypeName</key><string>Plan Studio Plan</string>
    <key>CFBundleTypeRole</key><string>Editor</string>
    <key>LSHandlerRank</key><string>Owner</string>
    <key>LSItemContentTypes</key><array><string>com.danielallendesigns.plan-studio.psplan</string></array>
    <key>CFBundleTypeExtensions</key><array><string>psplan</string></array>
    $DOC_ICON_KEY
  </dict></array>
  <key>UTExportedTypeDeclarations</key>
  <array><dict>
    <key>UTTypeIdentifier</key><string>com.danielallendesigns.plan-studio.psplan</string>
    <key>UTTypeDescription</key><string>Plan Studio Plan</string>
    <key>UTTypeConformsTo</key><array><string>public.json</string><string>public.data</string></array>
    <key>UTTypeTagSpecification</key>
    <dict><key>public.filename-extension</key><array><string>psplan</string></array></dict>
  </dict></array>
</dict></plist>
PLIST
echo "created: $APP"
