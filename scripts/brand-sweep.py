#!/usr/bin/env python3
"""List every place the original product's names appear, for a pre-release review.

Purpose: before a public release, find the user-facing places that name the
product Plan Studio is modeled on, so each can be checked for trademark and
nominative-use hygiene (naming a file format or an import source is fine; a
marketing-style comparison is not). It reports; it is a review aid, not a CI
gate and not legal advice.

Terms: scripts/brand-terms.txt, one Python regex per line, matched case-insensitively.
Allow list: scripts/brand-sweep-allow.txt, one regex per line. A hit whose matched
line matches any allow pattern is counted as "allowed" (nominative phrases such as
"Import ... Chief Plan") and shown only with --show-allowed. The rule is per line, so
a line that mixes an allowed phrase with a comparison is allowed as a whole.

Classes (each line counts once per class):
  literal     match inside a Rust string literal ("...", r"...", r#"..."#) in
              crates/**/src/**/*.rs and crates/**/build.rs. Lines of #[ignore = ".."]
              attributes and anything under a tests/ directory are skipped. Test code
              heuristics: once a #[cfg(test)] line is followed (past any other
              attributes) by an inline block "mod name {", the rest of the file counts
              as test code and is skipped; a #[cfg(test)] item of any other kind (a
              fn, a use, an impl) skips nothing, so real strings after it are still
              seen. An out-of-line "#[cfg(test)] mod name;" skips that module's file
              (name.rs, name/mod.rs, name/**). Text inside comments is never a literal.
  comment     match in a Rust comment (//, ///, //!, /* */) in the same files.
  doc         README.md, CHANGELOG.md, ROADMAP.md, DECISIONS.md, docs/manual/**/*.md,
              scripts/release-footer.md, and each description = "..." line of any
              Cargo.toml.
  identifier  a file or directory name under crates/, docs/, scripts/ containing
              "chief" (informational; reported as the path up to that component).
  internal    docs/*.md (except the manual) and docs/parity/**: reverse-engineering
              notes, counted only unless --show-internal.
target/, .git/ and other build dirs are skipped; files that are not UTF-8 are skipped.

Exit codes: 0 normally; 1 with --strict when a non-allowed literal hit exists;
3 on a usage or file error (missing root, terms or allow file, bad regex).

Examples
  python3 scripts/brand-sweep.py
  python3 scripts/brand-sweep.py --strict --limit 0
  python3 scripts/brand-sweep.py --json --root /path/to/checkout > sweep.json
  python3 scripts/brand-sweep.py --show-allowed --show-comments
"""
import argparse
import json
import os
import re
import sys
from collections import OrderedDict

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SKIP_DIRS = {"target", ".git", "node_modules", ".claude", ".venv", "venv", "__pycache__",
             "build", "dist", ".idea", ".vscode"}
ROOT_DOCS = ("README.md", "CHANGELOG.md", "ROADMAP.md", "DECISIONS.md")
CFG_TEST_RE = re.compile(r"^\s*#\[cfg\(test\)\]")
MOD_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*([{;])")
CARGO_DESC_RE = re.compile(r'^\s*description\s*=\s*"')


class SweepError(Exception):
    pass


def load_patterns(path, what):
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except OSError as e:
        raise SweepError("cannot read %s file %s: %s" % (what, path, e))
    out = []
    for ln in lines:
        s = ln.strip()
        if not s or s.startswith("#"):
            continue
        try:
            out.append(re.compile(s, re.IGNORECASE))
        except re.error as e:
            raise SweepError("bad regex in %s (%s): %r" % (path, e, s))
    return out


def lex_rust(text):
    """Return (literals, comments): dicts of 1-based line number -> list of text segments."""
    lits, coms = {}, {}
    n = len(text)
    i = 0
    line = 1
    buf = []
    mode = None  # None, 'lc', 'bc', 'str', 'raw'
    depth = hashes = 0

    def flush(kind):
        s = "".join(buf)
        del buf[:]
        if s.strip():
            (lits if kind == "lit" else coms).setdefault(line, []).append(s)

    while i < n:
        c = text[i]
        nxt = text[i + 1] if i + 1 < n else ""
        if mode is None:
            if c == "/" and nxt == "/":
                mode = "lc"; i += 2; continue
            if c == "/" and nxt == "*":
                mode = "bc"; depth = 1; i += 2; continue
            if c == '"':
                mode = "str"; i += 1; continue
            if c in "rb" and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
                j = i
                if c == "b" and nxt == "r":
                    j = i + 1
                if text[j] == "r":
                    k = j + 1
                    while k < n and text[k] == "#":
                        k += 1
                    if k < n and text[k] == '"':
                        mode = "raw"; hashes = k - j - 1; i = k + 1; continue
            if c == "'":
                if nxt == "\\":
                    j = i + 3
                    while j < n and text[j] != "'" and text[j] != "\n":
                        j += 1
                    i = j + 1
                elif i + 2 < n and text[i + 2] == "'":
                    i += 3
                else:
                    i += 1
                continue
            if c == "\n":
                line += 1
            i += 1
        elif mode == "lc":
            if c == "\n":
                flush("com"); mode = None; line += 1
            else:
                buf.append(c)
            i += 1
        elif mode == "bc":
            if c == "/" and nxt == "*":
                depth += 1; i += 2
            elif c == "*" and nxt == "/":
                depth -= 1; i += 2
                if depth == 0:
                    flush("com"); mode = None
            else:
                if c == "\n":
                    flush("com"); line += 1
                else:
                    buf.append(c)
                i += 1
        elif mode == "str":
            if c == "\\":
                if nxt == "\n":
                    flush("lit"); line += 1
                else:
                    buf.append(nxt)
                i += 2
            elif c == '"':
                flush("lit"); mode = None; i += 1
            elif c == "\n":
                flush("lit"); line += 1; i += 1
            else:
                buf.append(c); i += 1
        else:  # raw string
            if c == '"' and text[i + 1:i + 1 + hashes] == "#" * hashes:
                flush("lit"); mode = None; i += 1 + hashes
            elif c == "\n":
                flush("lit"); line += 1; i += 1
            else:
                buf.append(c); i += 1
    if mode in ("lc", "bc"):
        flush("com")
    elif mode in ("str", "raw"):
        flush("lit")
    return lits, coms


def walk(root, top):
    base = os.path.join(root, top)
    if not os.path.isdir(base):
        return
    for dp, dns, fns in os.walk(base):
        dns[:] = sorted(d for d in dns if d not in SKIP_DIRS)
        for fn in sorted(fns):
            yield os.path.join(dp, fn)


def rel(root, p):
    return os.path.relpath(p, root).replace(os.sep, "/")


def read_text(path):
    try:
        with open(path, "rb") as f:
            data = f.read()
        if b"\x00" in data:
            return None
        return data.decode("utf-8")
    except (OSError, UnicodeDecodeError):
        return None


def is_rust_target(r):
    parts = r.split("/")
    if parts[0] != "crates" or not r.endswith(".rs") or "tests" in parts[:-1]:
        return False
    return "src" in parts[:-1] or parts[-1] == "build.rs"


def doc_kind(r):
    """'doc', 'internal', or None for a non-rust relative path."""
    parts = r.split("/")
    if r in ROOT_DOCS or r == "scripts/release-footer.md":
        return "doc"
    if parts[0] == "docs":
        if len(parts) >= 3 and parts[1] == "manual" and r.endswith(".md"):
            return "doc"
        if len(parts) == 2 and r.endswith(".md"):
            return "internal"
        if len(parts) >= 3 and parts[1] == "parity":
            return "internal"
    return None


def cfg_test_items(lines):
    """Yield (line_number, module_name or None, inline) for each #[cfg(test)] line.

    module_name is set when the attribute is followed (past other attributes, blank
    and comment lines) by "mod name {" (inline=True) or "mod name;" (inline=False).
    """
    for idx, ln in enumerate(lines):
        if not CFG_TEST_RE.match(ln):
            continue
        j = idx + 1
        while j < len(lines):
            t = lines[j].strip()
            if t == "" or t.startswith("#[") or t.startswith("//"):
                j += 1
                continue
            break
        m = MOD_RE.match(lines[j]) if j < len(lines) else None
        if m:
            yield idx + 1, m.group(1), m.group(2) == "{"
        else:
            yield idx + 1, None, False


def test_module_paths(root, texts):
    """Relative paths (files and dir prefixes) of out-of-line #[cfg(test)] modules."""
    files, dirs = set(), set()
    for r, text in texts.items():
        d = os.path.dirname(r)
        base = os.path.basename(r)
        if base not in ("lib.rs", "main.rs", "mod.rs", "build.rs"):
            d = d + "/" + base[:-3]
        for _, name, inline in cfg_test_items(text.split("\n")):
            if name and not inline:
                files.add(d + "/" + name + ".rs")
                dirs.add(d + "/" + name + "/")
    return files, dirs


def scan(root, terms, allow):
    hits = {"literal": [], "comment": [], "doc": [], "internal": []}

    def matched(text):
        return [t.pattern for t in terms if t.search(text)]

    def add(cls, path, lineno, line, found):
        hits[cls].append({
            "path": path, "line": lineno, "text": line.strip(),
            "terms": found, "allowed": any(a.search(line) for a in allow),
        })

    texts = OrderedDict()
    for p in walk(root, "crates"):
        r = rel(root, p)
        if not is_rust_target(r):
            continue
        text = read_text(p)
        if text is not None:
            texts[r] = text
    test_files, test_dirs = test_module_paths(root, texts)
    for r, text in texts.items():
        if r in test_files or any(r.startswith(d) for d in test_dirs):
            continue
        lines = text.split("\n")
        cutoff = len(lines) + 1
        for ln_no, name, inline in cfg_test_items(lines):
            if name and inline:
                cutoff = ln_no
                break
        lits, coms = lex_rust(text)
        for cls, segs in (("literal", lits), ("comment", coms)):
            for lineno in sorted(segs):
                if lineno >= cutoff:
                    continue
                raw = lines[lineno - 1]
                if raw.lstrip().startswith("#[ignore"):
                    continue
                found = []
                for seg in segs[lineno]:
                    for t in matched(seg):
                        if t not in found:
                            found.append(t)
                if found:
                    add(cls, r, lineno, raw, found)

    def scan_lines(path, r, cls, only=None):
        text = read_text(path)
        if text is None:
            return
        for lineno, ln in enumerate(text.split("\n"), 1):
            if only is not None and not only.match(ln):
                continue
            found = matched(ln)
            if found:
                add(cls, r, lineno, ln, found)

    seen = set()
    for top in ("", "docs", "scripts", "crates"):
        base = root if top == "" else os.path.join(root, top)
        if top == "":
            cands = [os.path.join(root, f) for f in ROOT_DOCS]
        else:
            cands = list(walk(root, top))
        for p in cands:
            if not os.path.isfile(p) or p in seen:
                continue
            seen.add(p)
            r = rel(root, p)
            if os.path.basename(p) == "Cargo.toml":
                scan_lines(p, r, "doc", CARGO_DESC_RE)
                continue
            k = doc_kind(r)
            if k:
                scan_lines(p, r, k)
    cargo_root = os.path.join(root, "Cargo.toml")
    if os.path.isfile(cargo_root) and cargo_root not in seen:
        scan_lines(cargo_root, "Cargo.toml", "doc", CARGO_DESC_RE)

    idents = []
    for top in ("crates", "docs", "scripts"):
        base = os.path.join(root, top)
        if not os.path.isdir(base):
            continue
        for dp, dns, fns in os.walk(base):
            dns[:] = sorted(d for d in dns if d not in SKIP_DIRS)
            for name in sorted(dns):
                if "chief" in name.lower():
                    idents.append(rel(root, os.path.join(dp, name)))
            for name in sorted(fns):
                if "chief" in name.lower():
                    idents.append(rel(root, os.path.join(dp, name)))
    # Report a directory once, not each file under it.
    idents = sorted(set(idents))
    idents = [p for p in idents if not any(p.startswith(q + "/") for q in idents)]

    for lst in hits.values():
        lst.sort(key=lambda h: (h["path"], h["line"]))
    return hits, idents


def group(hs):
    g = OrderedDict()
    for h in hs:
        g.setdefault(h["path"], []).append(h)
    return g


def clip(s, n=160):
    return s if len(s) <= n else s[:n - 3] + "..."


def main(argv=None):
    ap = argparse.ArgumentParser(description="List the places the original product's names appear.")
    ap.add_argument("--root", default=ROOT)
    ap.add_argument("--terms", default=os.path.join(HERE, "brand-terms.txt"))
    ap.add_argument("--allow", default=os.path.join(HERE, "brand-sweep-allow.txt"))
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--strict", action="store_true",
                    help="exit 1 if any non-allowed literal hit exists")
    ap.add_argument("--show-allowed", action="store_true")
    ap.add_argument("--show-comments", action="store_true")
    ap.add_argument("--show-internal", action="store_true")
    ap.add_argument("--limit", type=int, default=5,
                    help="doc lines listed per file (0 = all, default 5)")
    args = ap.parse_args(argv)
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass
    try:
        if not os.path.isdir(args.root):
            raise SweepError("root not found: %s" % args.root)
        terms = load_patterns(args.terms, "terms")
        allow = load_patterns(args.allow, "allow")
        if not terms:
            raise SweepError("no terms in %s" % args.terms)
        hits, idents = scan(args.root, terms, allow)
    except SweepError as e:
        print("brand-sweep: error: %s" % e, file=sys.stderr)
        return 3

    counts = {c: sum(1 for h in hits[c] if not h["allowed"]) for c in hits}
    counts["identifier"] = len(idents)
    allowed_by_class = {c: sum(1 for h in hits[c] if h["allowed"]) for c in hits}
    allowed = sum(allowed_by_class.values())
    strict_fail = args.strict and counts["literal"] > 0

    if args.json:
        print(json.dumps({
            "root": args.root,
            "counts": counts,
            "allowed": allowed,
            "allowed_by_class": allowed_by_class,
            "hits": hits,
            "identifiers": idents,
        }, indent=2))
        return 1 if strict_fail else 0

    out = []
    out.append("Brand sweep of %s" % args.root)
    out.append("")
    out.append("Summary (non-allowed hits; allowed in brackets)")
    for c in ("literal", "doc", "identifier", "comment", "internal"):
        extra = ""
        if c in allowed_by_class:
            extra = "  [%d allowed]" % allowed_by_class[c]
        out.append("  %-10s %5d%s" % (c, counts[c], extra))
    out.append("  %-10s %5d" % ("allowed", allowed))

    def visible(c):
        return [h for h in hits[c] if args.show_allowed or not h["allowed"]]

    def fmt(h):
        tag = " [allowed]" if h["allowed"] else ""
        return "  %s:%d:%s %s" % (h["path"], h["line"], tag, clip(h["text"]))

    out.append("")
    out.append("Literals in Rust strings (%d)" % counts["literal"])
    for path, hs in group(visible("literal")).items():
        for h in hs:
            out.append(fmt(h))
    if not visible("literal"):
        out.append("  none")

    out.append("")
    out.append("Docs (%d)" % counts["doc"])
    for path, hs in group(visible("doc")).items():
        out.append("  %s: %d" % (path, len(hs)))
        shown = hs if args.limit == 0 else hs[:args.limit]
        for h in shown:
            tag = " [allowed]" if h["allowed"] else ""
            out.append("    %d:%s %s" % (h["line"], tag, clip(h["text"])))
        if len(shown) < len(hs):
            out.append("    ... %d more (--limit 0 for all)" % (len(hs) - len(shown)))
    if not visible("doc"):
        out.append("  none")

    out.append("")
    out.append("Identifiers (path names containing 'chief', informational) (%d)" % len(idents))
    for p in idents:
        out.append("  " + p)

    for cls, flag, title in (("comment", args.show_comments, "Rust comments"),
                             ("internal", args.show_internal, "Internal notes")):
        out.append("")
        if flag:
            out.append("%s (%d)" % (title, counts[cls]))
            for path, hs in group(visible(cls)).items():
                for h in hs:
                    out.append(fmt(h))
        else:
            out.append("%s: %d hits in %d files (use --show-%s to list)" %
                       (title, counts[cls], len(group(visible(cls))), cls if cls == "internal" else "comments"))
    print("\n".join(out))
    return 1 if strict_fail else 0


if __name__ == "__main__":
    sys.exit(main())
