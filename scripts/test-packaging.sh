#!/bin/bash
# Self-test of the packaging scripts with a stand-in binary (no cargo build).
# Run from anywhere: scripts/test-packaging.sh. CI runs it on every pull request.
# Checks the Windows zip, the ICO, the Linux tarball (and the AppImage staging
# when appimagetool exists), the changelog extraction and the version helper.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
PY=python3; command -v python3 >/dev/null 2>&1 || PY=python

VERSION="$(sh "$HERE/version.sh")"
case "$VERSION" in [0-9]*.[0-9]*.[0-9]*) ;; *) fail "version.sh gave '$VERSION'" ;; esac

# --- ICO ---
"$PY" "$HERE/make-ico.py" "$T/a.ico" "$ROOT"/crates/plan-app/assets/icons/app/icon-{16,32,256}.png >/dev/null
"$PY" - "$T/a.ico" <<'PYEOF' || fail "ico header"
import struct, sys
d = open(sys.argv[1], "rb").read()
assert struct.unpack("<HHH", d[:6]) == (0, 1, 3), "header"
sizes = [d[6 + 16 * i] or 256 for i in range(3)]
assert sizes == [16, 32, 256], sizes
off = struct.unpack("<I", d[6 + 12:6 + 16])[0]
assert d[off:off + 8] == b"\x89PNG\r\n\x1a\n", "png payload"
PYEOF
echo "ok: ico"

# --- Windows zip ---
printf 'MZ stand-in' > "$T/plan-studio.exe"
bash "$HERE/windows-package.sh" "$T/plan-studio.exe" "$T/out" >/dev/null
Z="$T/out/Plan Studio-$VERSION-windows-x64.zip"
[ -f "$Z" ] || fail "no windows zip at $Z"
"$PY" - "$Z" "$VERSION" <<'PYEOF' || fail "zip contents"
import sys, zipfile
z = zipfile.ZipFile(sys.argv[1]); top = f"Plan Studio-{sys.argv[2]}-windows-x64/"
names = set(z.namelist())
for want in ("plan-studio.exe", "plan-studio.ico", "LICENSE", "README.txt"):
    assert top + want in names, (want, sorted(names))
assert z.testzip() is None
PYEOF
echo "ok: windows zip"

# --- Linux tarball ---
linux_tarball_checks() {
printf '#!/bin/sh\n' > "$T/plan-studio"; chmod +x "$T/plan-studio"
bash "$HERE/linux-appimage.sh" "$T/plan-studio" "$T/out" test-label >/dev/null
tar -tzf "$T/out/test-label.tar.gz" > "$T/list"
for f in plan-studio plan-studio.desktop plan-studio.png plan-studio-psplan.xml install.sh LICENSE; do
  grep -qx "test-label/$f" "$T/list" || fail "tarball is missing $f"
done
grep -q '^Icon=plan-studio$' "$HERE/linux/plan-studio.desktop" || fail "desktop file has no Icon"
if command -v appimagetool >/dev/null 2>&1 || [ -n "${APPIMAGETOOL:-}" ]; then
  [ -f "$T/out/test-label.AppImage" ] || fail "appimagetool present but no AppImage"
fi
# AppImage staging with a stand-in appimagetool that checks the AppDir layout.
cat > "$T/fake-appimagetool" <<'FAKE'
#!/bin/sh
for a; do SRC="$DST"; DST="$a"; done
for f in AppRun plan-studio.desktop plan-studio.png .DirIcon usr/bin/plan-studio; do
  [ -e "$SRC/$f" ] || { echo "AppDir lacks $f" >&2; exit 1; }
done
echo appimage > "$DST"
FAKE
chmod +x "$T/fake-appimagetool"
APPIMAGETOOL="$T/fake-appimagetool" bash "$HERE/linux-appimage.sh" "$T/plan-studio" "$T/out2" ai-label >/dev/null
[ -f "$T/out2/ai-label.AppImage" ] || fail "AppImage step did not run"
# install.sh into a scratch prefix, then uninstall.
tar -xzf "$T/out/test-label.tar.gz" -C "$T"
sh "$T/test-label/install.sh" "$T/prefix" >/dev/null
[ -x "$T/prefix/bin/plan-studio" ] && [ -f "$T/prefix/share/applications/plan-studio.desktop" ] || fail "install.sh"
echo "ok: linux tarball"
}
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) echo "skip: linux tarball (Windows runner; the Linux job covers it)" ;;
  *) linux_tarball_checks ;;
esac

# --- changelog extraction ---
cat > "$T/CL.md" <<'MD'
# Changelog

## [Unreleased]

Next thing.

## [1.2.3] - 2026-01-01

### Added

- Thing one.

[1.2.3]: https://example.invalid/1.2.3

## [1.2.2] - 2025-12-01

- Old.
MD
OUT="$(sh "$HERE/release-notes.sh" v1.2.3 "$T/CL.md")"
printf '%s' "$OUT" | grep -q 'Thing one' || fail "notes missing body"
printf '%s' "$OUT" | grep -q 'Old\.' && fail "notes leaked the next section"
printf '%s' "$OUT" | grep -q 'example.invalid' && fail "notes kept a link reference"
sh "$HERE/release-notes.sh" 1.2.9 "$T/CL.md" >/dev/null 2>&1 && fail "missing final section should fail"
sh "$HERE/release-notes.sh" 1.3.0-rc1 "$T/CL.md" | grep -q 'Next thing' || fail "rc should fall back to Unreleased"
echo "ok: release notes"

# --- parity-score ---
cat > "$T/par.md" <<'MD'
# Parity fixture

## Totals by area

| Area | Ids | Old W/P/M/D | New W/P/M/D | Works % | Works+Partial % |
|---|---|---|---|---|---|
| Walls | 5 | 2/1/1/1 | 2/1/1/1 | 50% | 75% |
| Text | 3 | 1/1/1/0 | 1/1/1/0 | 33% | 67% |
| **Overall** | 8 | 3/2/2/1 | 3/2/2/1 | 43% | 71% |

## Next 25 gaps for daily residential work

| # | Gap (parity ids) | Why |
|---|---|---|
| 1 | Chain behavior (W-3) | Daily |

## Per-id status

### Walls

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| W-1 | Walls | Wall tool | Works | a.rs | - |
| W-2 | Walls | Wall face | works | a.rs | - |
| W-3 | Walls | Click chain | Partial | a.rs | Right-click ends it. |
| W-4 | Walls | Join cleanup | Missing | - | Not built. |
| W-5 | Walls | Odd one | Differs-by-design | - | By decision. |

### Text

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| T-1 | Text | Text tool | Works | b.rs | - |
| T-2 | Text | Text box wrap | Partial | b.rs | No wrap. |
| T-3 | Text | Replace fonts | Missing | - | Inert. |
MD
printf '{"_comment": "test", "next25_bonus": 2, "Walls": 3, "Text": 2}\n' > "$T/pw.json"
"$PY" "$HERE/parity-score.py" --doc "$T/par.md" --weights "$T/pw.json" --json > "$T/par.json" || fail "parity-score --json"
"$PY" - "$T/par.json" <<'PYEOF' || fail "parity-score values"
import json, sys
d = json.load(open(sys.argv[1]))
o = d["overall"]
# weights: W-1 3, W-2 3, W-3 6 (Next 25 bonus), W-4 3, T-1..T-3 2; credit 3+3+3+0+2+1+0 of 21
assert o["weighted_pct"] == 57.1, o
assert (o["W"], o["P"], o["M"], o["D"], o["counted"]) == (3, 2, 2, 1, 7), o
assert o["works_pct"] == 42.9 and o["works_partial_pct"] == 71.4, o
assert [m["id"] for m in d["missing"]] == ["W-4", "T-3"], d["missing"]
assert d["differs_by_design"] == ["W-5"] and d["warnings"] == [], d
assert d["totals_check"]["ok"] is True, d["totals_check"]
PYEOF
"$PY" "$HERE/parity-score.py" --doc "$T/par.md" --weights "$T/pw.json" --check-totals >/dev/null || fail "parity-score --check-totals should pass"
sed 's#| 3/2/2/1 | 43%#| 3/2/3/1 | 43%#' "$T/par.md" > "$T/par-bad.md"
grep -q '3/2/3/1' "$T/par-bad.md" || fail "parity fixture edit did not apply"
rc=0; "$PY" "$HERE/parity-score.py" --doc "$T/par-bad.md" --weights "$T/pw.json" --check-totals >"$T/par-bad.out" 2>&1 || rc=$?
[ "$rc" = 2 ] || fail "parity-score --check-totals should exit 2 on a wrong total, got $rc"
grep -q 'Overall: table says 3/2/3/1, rows count 3/2/2/1' "$T/par-bad.out" || fail "parity-score mismatch message"
rc=0; "$PY" "$HERE/parity-score.py" --doc "$T/par.md" --weights "$T/pw.json" --fail-under 90 >/dev/null 2>&1 || rc=$?
[ "$rc" = 1 ] || fail "parity-score --fail-under should exit 1, got $rc"
{ cat "$T/par.md"; echo '| W-1 | Walls | Again | Works | a.rs | - |'; } > "$T/par-dup.md"
rc=0; "$PY" "$HERE/parity-score.py" --doc "$T/par-dup.md" --weights "$T/pw.json" >/dev/null 2>&1 || rc=$?
[ "$rc" = 3 ] || fail "parity-score duplicate id should exit 3, got $rc"
rc=0; "$PY" "$HERE/parity-score.py" --doc "$T/nope.md" --weights "$T/pw.json" >/dev/null 2>&1 || rc=$?
[ "$rc" = 3 ] || fail "parity-score missing doc should exit 3, got $rc"
echo "ok: parity-score"

# --- brand-sweep ---
mkdir -p "$T/fixture/crates/x/src"
cat > "$T/fixture/crates/x/src/lib.rs" <<'RS'
pub fn a() -> &'static str { "Chief Architect" }
// Chief
pub fn b() -> &'static str { "Import Chief Plan" }
#[cfg(test)]
mod tests {
    fn t() -> &'static str { "Chief Architect" }
}
RS
printf '# Fixture\n\nPlan Studio reads Chief Architect files.\n' > "$T/fixture/README.md"
"$PY" "$HERE/brand-sweep.py" --root "$T/fixture" --json > "$T/sweep.json" || fail "brand-sweep --json"
"$PY" - "$T/sweep.json" <<'PYEOF' || fail "brand-sweep counts"
import json, sys
d = json.load(open(sys.argv[1]))
c = d["counts"]
assert (c["literal"], c["comment"], c["doc"]) == (1, 1, 1), c
assert c["identifier"] == 0 and c["internal"] == 0, c
assert d["allowed"] == 1, d["allowed"]
assert d["hits"]["literal"][0]["line"] == 1, d["hits"]["literal"]
PYEOF
rc=0; "$PY" "$HERE/brand-sweep.py" --root "$T/fixture" --strict >/dev/null 2>&1 || rc=$?
[ "$rc" = 1 ] || fail "brand-sweep --strict should exit 1, got $rc"
"$PY" "$HERE/brand-sweep.py" --root "$T/fixture" >/dev/null || fail "brand-sweep without --strict should exit 0"
echo "ok: brand-sweep"
echo "all packaging checks passed"
