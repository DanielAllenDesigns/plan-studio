#!/usr/bin/env python3
"""Weighted parity score for docs/parity-status.md.

Reads the per-id status tables of the parity document and reports how close
Plan Studio is to the reference product, weighted toward daily residential work.

Weighting rule
  credit per id:  Works 1.0, Partial 0.5, Missing 0.
  Differs-by-design ids are excluded from the denominator (and listed apart).
  weight per id:  area weight (scripts/parity-weights.json) x next25_bonus when the
                  id is cited in the "Next 25 gaps for daily residential work" table.
  score:          sum(weight x credit) / sum(weight) over counted ids, in percent.
  The unweighted Works % and Works+Partial % (also over counted ids) are shown too.
  An area with no weight entry gets weight 1 and a warning. A status word that is
  not Works / Partial / Missing / Differs-by-design is warned about and counted
  as Missing.

Output: a per-area table, the overall line, then a build order: Missing ids and
then Partial ids, sorted by weight (high first) then id, capped by --limit.

Exit codes
  0  ok
  1  --fail-under N given and the weighted score is below N
  2  --check-totals given and the Totals table disagrees with the per-id rows
     (takes precedence over 1)
  3  error: doc not found, no per-id rows parsed, duplicate ids, unreadable weights
     file, or nothing countable

Examples
  python3 scripts/parity-score.py
  python3 scripts/parity-score.py --check-totals --limit 0
  python3 scripts/parity-score.py --json --fail-under 70 > parity.json
  python3 scripts/parity-score.py --markdown --doc docs/parity-status.md
"""
import argparse
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
ID_RE = re.compile(r"^[A-Z]+-\d+$")
ANY_ID_RE = re.compile(r"\b[A-Z]+-\d+\b")
STATUSES = {"works": "W", "partial": "P", "missing": "M", "differs-by-design": "D"}
CREDIT = {"W": 1.0, "P": 0.5, "M": 0.0}
LABEL = {"W": "Works", "P": "Partial", "M": "Missing", "D": "Differs-by-design"}


class ParityError(Exception):
    pass


def split_row(line):
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|"):
        s = s[:-1]
    return [c.strip() for c in s.split("|")]


def is_separator(cells):
    return all(re.fullmatch(r":?-{2,}:?", c) for c in cells if c) and any(cells)


def section_lines(lines, heading_prefix):
    """Lines under the first '## ' heading that starts with heading_prefix."""
    out, on = [], False
    for ln in lines:
        if ln.startswith("## "):
            if on:
                break
            on = ln[3:].strip().startswith(heading_prefix)
            continue
        if on:
            out.append(ln)
    return out


def id_key(i):
    p, n = i.rsplit("-", 1)
    return (p, int(n))


def parse_rows(lines, warnings):
    rows, seen, dups, unknown = [], {}, [], []
    for ln in _per_id_lines(lines):
        if not ln.lstrip().startswith("|"):
            continue
        cells = split_row(ln)
        if len(cells) < 4 or is_separator(cells) or not ID_RE.match(cells[0]):
            continue
        pid, area, behavior, raw = cells[0], cells[1], cells[2], cells[3]
        code = STATUSES.get(raw.strip().lower())
        if code is None:
            unknown.append((pid, raw))
            code = "M"
        if pid in seen:
            dups.append(pid)
        seen[pid] = True
        rows.append({"id": pid, "area": area, "behavior": behavior, "status": code,
                     "raw_status": raw})
    if dups:
        raise ParityError("duplicate ids in per-id tables: " + ", ".join(sorted(set(dups), key=id_key)))
    if not rows:
        raise ParityError("no per-id rows parsed (is there a '## Per-id status' heading with tables?)")
    for pid, raw in unknown:
        warnings.append("%s: unknown status %r, counted as Missing" % (pid, raw))
    return rows


def _per_id_lines(lines):
    """Everything from the '## Per-id status' heading to the end (sub-headings are '###')."""
    out, on = [], False
    for ln in lines:
        if ln.startswith("## "):
            on = ln[3:].strip().startswith("Per-id status")
            continue
        if on:
            out.append(ln)
    return out


def parse_next25(lines):
    ids = set()
    for ln in section_lines(lines, "Next 25"):
        if not ln.lstrip().startswith("|"):
            continue
        cells = split_row(ln)
        if len(cells) < 2 or not cells[0].isdigit():
            continue
        for grp in re.findall(r"\(([^()]*)\)", cells[1]):
            ids.update(ANY_ID_RE.findall(grp))
    return ids


def parse_totals(lines):
    """Return {area: (ids, (w,p,m,d))} for the 'New W/P/M/D' column, plus the Overall row."""
    out = {}
    for ln in section_lines(lines, "Totals by area"):
        if not ln.lstrip().startswith("|"):
            continue
        cells = split_row(ln)
        if len(cells) < 4 or is_separator(cells) or cells[0].lower() == "area":
            continue
        name = cells[0].replace("*", "").strip()
        m = re.fullmatch(r"(\d+)/(\d+)/(\d+)/(\d+)", cells[3].replace("*", "").strip())
        if not m:
            continue
        ids = cells[1].replace("*", "").strip()
        out[name] = (int(ids) if ids.isdigit() else None, tuple(int(x) for x in m.groups()))
    return out


def load_weights(path):
    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
    except (OSError, ValueError) as e:
        raise ParityError("cannot read weights file %s: %s" % (path, e))
    bonus = data.get("next25_bonus", 1)
    weights = {k: v for k, v in data.items()
               if not k.startswith("_") and k != "next25_bonus"}
    return weights, bonus


def pct(a, b):
    return round(100.0 * a / b, 1) if b else 0.0


def compute(rows, weights, bonus, next25, warnings):
    areas, order = {}, []
    ids_present = {r["id"] for r in rows}
    for pid in sorted(next25 - ids_present, key=id_key):
        warnings.append("Next 25 table cites %s, which has no per-id row" % pid)
    warned = set()
    for r in rows:
        a = r["area"]
        if a not in areas:
            areas[a] = {"area": a, "W": 0, "P": 0, "M": 0, "D": 0, "wsum": 0.0, "wcredit": 0.0}
            order.append(a)
            if a not in weights and a not in warned:
                warned.add(a)
                warnings.append("area %r has no weight in the weights file, using 1" % a)
        aw = weights.get(a, 1)
        w = aw * (bonus if r["id"] in next25 else 1)
        r["weight"] = w
        st = r["status"]
        ar = areas[a]
        ar[st] += 1
        if st != "D":
            ar["wsum"] += w
            ar["wcredit"] += w * CREDIT[st]
    out = []
    for a in order:
        ar = areas[a]
        counted = ar["W"] + ar["P"] + ar["M"]
        out.append({
            "area": a, "counted": counted, "W": ar["W"], "P": ar["P"], "M": ar["M"], "D": ar["D"],
            "weight": weights.get(a, 1),
            "works_pct": pct(ar["W"], counted),
            "works_partial_pct": pct(ar["W"] + ar["P"], counted),
            "weighted_pct": pct(ar["wcredit"], ar["wsum"]),
            "_wsum": ar["wsum"], "_wcredit": ar["wcredit"],
        })
    tw = sum(x["_wsum"] for x in out)
    tc = sum(x["_wcredit"] for x in out)
    W = sum(x["W"] for x in out); P = sum(x["P"] for x in out)
    M = sum(x["M"] for x in out); D = sum(x["D"] for x in out)
    counted = W + P + M
    if tw <= 0:
        raise ParityError("nothing countable: every parsed id is Differs-by-design or has weight 0")
    overall = {"ids": counted + D, "counted": counted, "W": W, "P": P, "M": M, "D": D,
               "works_pct": pct(W, counted), "works_partial_pct": pct(W + P, counted),
               "weighted_pct": pct(tc, tw)}
    for x in out:
        x.pop("_wsum"); x.pop("_wcredit")
    return out, overall


def order_key(r):
    return (-r["weight"], id_key(r["id"]))


def check_totals(rows, areas, overall, totals):
    mism = []
    if not totals:
        return ["no parseable rows in the '## Totals by area' table"]
    for a in areas:
        name = a["area"]
        mine = (a["W"], a["P"], a["M"], a["D"])
        if name not in totals:
            mism.append("%s: not in the Totals table, rows count %d/%d/%d/%d" % ((name,) + mine))
            continue
        tids, theirs = totals[name]
        if theirs != mine:
            mism.append("%s: table says %d/%d/%d/%d, rows count %d/%d/%d/%d" % ((name,) + theirs + mine))
        elif tids is not None and tids != sum(mine):
            mism.append("%s: table Ids says %d, rows count %d" % (name, tids, sum(mine)))
    names = {a["area"] for a in areas}
    for name, (tids, theirs) in totals.items():
        if name.lower() == "overall" or name in names:
            continue
        mism.append("%s: table says %d/%d/%d/%d, rows count 0/0/0/0" % ((name,) + theirs))
    key = next((k for k in totals if k.lower() == "overall"), None)
    if key is None:
        mism.append("Overall: no Overall row in the Totals table")
    else:
        tids, theirs = totals[key]
        mine = (overall["W"], overall["P"], overall["M"], overall["D"])
        if theirs != mine:
            mism.append("Overall: table says %d/%d/%d/%d, rows count %d/%d/%d/%d" % (theirs + mine))
        elif tids is not None and tids != overall["ids"]:
            mism.append("Overall: table Ids says %d, rows count %d" % (tids, overall["ids"]))
    return mism


def clip(s, n=80):
    return s if len(s) <= n else s[:n]


def render(fmt, areas, overall, missing, partial, differs, warnings, unknown_rows,
           limit, mism, did_check, bonus):
    md = fmt == "markdown"
    out = []

    def esc(s):
        return s.replace("|", "\\|") if md else s

    def table(headers, data):
        if md:
            out.append("| " + " | ".join(headers) + " |")
            out.append("|" + "|".join("---" for _ in headers) + "|")
            for d in data:
                out.append("| " + " | ".join(esc(str(c)) for c in d) + " |")
        else:
            widths = [max(len(str(x)) for x in [h] + [d[i] for d in data]) for i, h in enumerate(headers)]
            out.append("  ".join(h.ljust(widths[i]) for i, h in enumerate(headers)))
            for d in data:
                out.append("  ".join(str(c).ljust(widths[i]) for i, c in enumerate(d)))

    def h(title):
        out.append("")
        out.append(("## " if md else "") + title)
        if not md:
            out.append("-" * len(title))
        out.append("")

    out.append(("# " if md else "") + "Parity score")
    h("Per area (Differs-by-design excluded from counts and percentages)")
    table(["Area", "Counted", "W", "P", "M", "D", "Weight", "Works %", "Weighted %"],
          [[a["area"], a["counted"], a["W"], a["P"], a["M"], a["D"], a["weight"],
            "%.1f" % a["works_pct"], "%.1f" % a["weighted_pct"]] for a in areas])
    out.append("")
    o = overall
    out.append(("**Overall**: " if md else "Overall: ") +
               "weighted %.1f%%; unweighted Works %.1f%%, Works+Partial %.1f%%; "
               "%d counted ids (W %d, P %d, M %d); %d Differs-by-design excluded; "
               "Next 25 bonus x%s." % (o["weighted_pct"], o["works_pct"], o["works_partial_pct"],
                                       o["counted"], o["W"], o["P"], o["M"], o["D"], bonus))
    if differs:
        out.append("Differs-by-design (%d): %s" % (len(differs), ", ".join(r["id"] for r in differs)))

    def listing(title, lst):
        shown = lst if limit == 0 else lst[:limit]
        h("%s (%d%s)" % (title, len(lst), "" if len(shown) == len(lst) else ", showing %d" % len(shown)))
        if not shown:
            out.append("none")
            return
        table(["Id", "Weight", "Area", "Chief behavior"],
              [[r["id"], r["weight"], r["area"], clip(r["behavior"])] for r in shown])

    listing("Build order: Missing", missing)
    listing("Build order: Partial", partial)

    if did_check:
        h("Totals check")
        if mism:
            for m in mism:
                out.append(("- " if md else "") + m)
        else:
            out.append("ok: the Totals table matches the per-id rows")
    if unknown_rows or warnings:
        h("Warnings")
        for w in warnings:
            out.append(("- " if md else "") + w)
    return "\n".join(out) + "\n"


def main(argv=None):
    ap = argparse.ArgumentParser(description="Weighted parity score over docs/parity-status.md.")
    ap.add_argument("--doc", default=os.path.join(ROOT, "docs", "parity-status.md"))
    ap.add_argument("--weights", default=os.path.join(HERE, "parity-weights.json"))
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    ap.add_argument("--markdown", action="store_true", help="markdown tables")
    ap.add_argument("--fail-under", type=float, default=None, metavar="N",
                    help="exit 1 if the weighted score is below N")
    ap.add_argument("--check-totals", action="store_true",
                    help="compare per-id rows with the Totals table; exit 2 on mismatch")
    ap.add_argument("--limit", type=int, default=25,
                    help="rows per build-order list in text/markdown (0 = all, default 25)")
    args = ap.parse_args(argv)
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass
    warnings = []
    try:
        if not os.path.isfile(args.doc):
            raise ParityError("doc not found: %s" % args.doc)
        with open(args.doc, encoding="utf-8") as f:
            lines = f.read().splitlines()
        weights, bonus = load_weights(args.weights)
        rows = parse_rows(lines, warnings)
        next25 = parse_next25(lines)
        totals = parse_totals(lines)
        areas, overall = compute(rows, weights, bonus, next25, warnings)
    except ParityError as e:
        print("parity-score: error: %s" % e, file=sys.stderr)
        return 3

    missing = sorted((r for r in rows if r["status"] == "M"), key=order_key)
    partial = sorted((r for r in rows if r["status"] == "P"), key=order_key)
    differs = sorted((r for r in rows if r["status"] == "D"), key=lambda r: id_key(r["id"]))
    unknown_rows = [r for r in rows if r["raw_status"].strip().lower() not in STATUSES]
    mism = check_totals(rows, areas, overall, totals)

    if args.json:
        def lst(rs):
            return [{"id": r["id"], "area": r["area"], "weight": r["weight"],
                     "behavior": r["behavior"]} for r in rs]
        print(json.dumps({
            "doc": args.doc, "overall": overall, "areas": areas, "next25_bonus": bonus,
            "next25_ids": sorted(next25, key=id_key),
            "missing": lst(missing), "partial": lst(partial),
            "differs_by_design": [r["id"] for r in differs],
            "unknown_status": [{"id": r["id"], "status": r["raw_status"]} for r in unknown_rows],
            "warnings": warnings,
            "totals_check": {"checked": args.check_totals, "ok": not mism, "mismatches": mism},
        }, indent=2))
    else:
        print(render("markdown" if args.markdown else "text", areas, overall, missing, partial,
                     differs, warnings, unknown_rows, args.limit, mism, args.check_totals, bonus),
              end="")

    if args.check_totals and mism:
        return 2
    if args.fail_under is not None and overall["weighted_pct"] < args.fail_under:
        print("parity-score: weighted %.1f%% is below --fail-under %s" %
              (overall["weighted_pct"], args.fail_under), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
