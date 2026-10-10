#!/bin/sh
# Print the workspace version from the root Cargo.toml ([workspace.package] version).
# Usage: scripts/version.sh
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
V="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
[ -n "$V" ] || { echo "no version found in $ROOT/Cargo.toml" >&2; exit 1; }
printf '%s\n' "$V"
