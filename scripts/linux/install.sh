#!/bin/sh
# Per-user install from the Plan Studio tarball (no root needed).
# Usage: ./install.sh [prefix]     prefix defaults to ~/.local
#        ./install.sh --uninstall  removes what was installed under ~/.local
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${1:-$HOME/.local}"
if [ "${1:-}" = "--uninstall" ]; then
  PREFIX="$HOME/.local"
  rm -f "$PREFIX/bin/plan-studio" \
        "$PREFIX/share/applications/plan-studio.desktop" \
        "$PREFIX/share/icons/hicolor/256x256/apps/plan-studio.png" \
        "$PREFIX/share/mime/packages/plan-studio-psplan.xml"
  command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$PREFIX/share/mime" || true
  echo "removed Plan Studio from $PREFIX"
  exit 0
fi
put() { mkdir -p "$(dirname "$3")"; install -m "$1" "$2" "$3"; }
put 755 "$HERE/plan-studio" "$PREFIX/bin/plan-studio"
put 644 "$HERE/plan-studio.desktop" "$PREFIX/share/applications/plan-studio.desktop"
put 644 "$HERE/plan-studio.png" "$PREFIX/share/icons/hicolor/256x256/apps/plan-studio.png"
put 644 "$HERE/plan-studio-psplan.xml" "$PREFIX/share/mime/packages/plan-studio-psplan.xml"
command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$PREFIX/share/mime" || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$PREFIX/share/applications" || true
echo "installed Plan Studio under $PREFIX (make sure $PREFIX/bin is on your PATH)"
