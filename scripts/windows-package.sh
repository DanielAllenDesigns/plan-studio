#!/bin/bash
# Package the Windows build as "Plan Studio-<version>-windows-x64.zip".
#
# Usage: scripts/windows-package.sh [exe] [output-dir]
#   exe         the built binary (default: $CARGO_TARGET_DIR/release/plan-studio.exe,
#               else target/release/plan-studio.exe)
#   output-dir  where the zip goes (default: dist)
#
# The zip holds one folder, "Plan Studio-<version>-windows-x64", with
# plan-studio.exe, plan-studio.ico (packed from the PNG sizes in
# crates/plan-app/assets/icons/app by scripts/make-ico.py), LICENSE and a short
# README.txt. Runs under Git Bash on the windows-latest runner, and anywhere
# bash and python3 exist (the CI packaging test runs it on Linux with a stand-in
# exe). Python's zipfile writes the zip, so no zip/7z tool is needed.
#
# Not done: the .ico is shipped next to the exe but not embedded in it. Embedding
# a Windows resource needs the winres crate (or rc.exe in a build.rs), which would
# add a crate to the workspace graph; until then Explorer shows the generic icon.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
EXE="${1:-${CARGO_TARGET_DIR:-$ROOT/target}/release/plan-studio.exe}"
OUT="${2:-dist}"
[ -f "$EXE" ] || { echo "binary not found: $EXE (run cargo build --release -p plan-app first)" >&2; exit 1; }
VERSION="$(sh "$HERE/version.sh")"
PY=""
for c in python3 python py; do command -v "$c" >/dev/null 2>&1 && PY="$c" && break; done
[ -n "$PY" ] || { echo "python3 not found" >&2; exit 1; }

NAME="Plan Studio-$VERSION-windows-x64"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
DIR="$STAGE/$NAME"
mkdir -p "$DIR" "$OUT"
cp "$EXE" "$DIR/plan-studio.exe"
cp "$ROOT/LICENSE" "$DIR/LICENSE"

ICONS="$ROOT/crates/plan-app/assets/icons/app"
PNGS=()
for s in 16 32 64 128 256; do
  if [ -f "$ICONS/icon-$s.png" ]; then PNGS+=("$ICONS/icon-$s.png"); fi
done
[ "${#PNGS[@]}" -gt 0 ] || { echo "no icon-<size>.png files in $ICONS" >&2; exit 1; }
"$PY" "$HERE/make-ico.py" "$DIR/plan-studio.ico" "${PNGS[@]}" >/dev/null

cat > "$DIR/README.txt" <<README
Plan Studio $VERSION for Windows (64-bit)

Run plan-studio.exe. There is no installer: keep the folder anywhere you like.
Windows SmartScreen may warn about an unsigned program the first time (More info,
then Run anyway).

Plan Studio is an open-source floor-plan editor. Its plan files end in .psplan.
To open them by double-click: right-click a .psplan file, Open with, Choose
another app, pick plan-studio.exe, and tick "Always use this app".

plan-studio.ico is the program icon (for shortcuts: right-click a shortcut,
Properties, Change Icon).

Source and manual: https://github.com/DanielAllenDesigns/plan-studio
README

ZIP="$OUT/$NAME.zip"
rm -f "$ZIP"
ZIP_ABS="$(cd "$OUT" && pwd)/$NAME.zip"
(cd "$STAGE" && "$PY" - "$ZIP_ABS" "$NAME" <<'PYEOF'
import os, sys, zipfile
zip_path, top = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as z:
    for root, _, files in os.walk(top):
        for f in sorted(files):
            p = os.path.join(root, f)
            z.write(p, p.replace(os.sep, "/"))
PYEOF
)
echo "created: $ZIP"
