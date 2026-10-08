#!/bin/sh
# Wrap the plan-studio binary in a minimal macOS .app bundle.
# Usage: scripts/macos-bundle.sh [debug|release] [output-dir]
set -eu
PROFILE="${1:-debug}"
OUT="${2:-${CARGO_TARGET_DIR:-target}}"
BIN="${CARGO_TARGET_DIR:-target}/$PROFILE/plan-studio"
APP="$OUT/Plan Studio.app"
[ -x "$BIN" ] || { echo "binary not found: $BIN (run cargo build -p plan-app first)"; exit 1; }
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$BIN" "$APP/Contents/MacOS/plan-studio"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Plan Studio</string>
  <key>CFBundleDisplayName</key><string>Plan Studio</string>
  <key>CFBundleIdentifier</key><string>com.danielallendesigns.plan-studio</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleExecutable</key><string>plan-studio</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
echo "created: $APP"
