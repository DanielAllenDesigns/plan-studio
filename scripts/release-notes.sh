#!/bin/sh
# Print the CHANGELOG.md section for a version (the text under "## [X.Y.Z]" up to
# the next "## " heading), for use as GitHub Release notes.
#
# Usage: scripts/release-notes.sh X.Y.Z [changelog]     (a leading "v" is ignored)
#
# A prerelease tag such as 0.2.0-rc1 also matches the "## [0.2.0-rc1]" heading,
# and falls back to the "## [Unreleased]" section if there is no heading of that
# name (so a release candidate can be cut before the changelog is renamed). A
# final release has no fallback: the checklist renames [Unreleased] first, and a
# missing section is an error. Link-reference lines ("[0.1.0]: https://...") are
# dropped. Exits 1 with a message on stderr when nothing is found.
set -eu
V="${1:?usage: release-notes.sh X.Y.Z [changelog]}"
V="${V#v}"
FILE="${2:-$(dirname "$0")/../CHANGELOG.md}"
[ -f "$FILE" ] || { echo "changelog not found: $FILE" >&2; exit 1; }
extract() {
  awk -v want="$1" '
    /^## / { if (on) exit; on = (index($0, "## [" want "]") == 1); next }
    on && /^\[[^]]+\]: / { next }
    on { print }
  ' "$FILE"
}
BODY="$(extract "$V")"
case "$V" in
  *-*) [ -n "$(printf '%s' "$BODY" | tr -d '[:space:]')" ] || BODY="$(extract Unreleased)" ;;
esac
if [ -z "$(printf '%s' "$BODY" | tr -d '[:space:]')" ]; then
  echo "no CHANGELOG section for $V in $FILE (rename [Unreleased] first, see docs/release-checklist.md)" >&2
  exit 1
fi
printf '%s\n' "$BODY" | sed -e :a -e '/^[[:space:]]*$/{$d;N;ba' -e '}' | sed '/./,$!d'
