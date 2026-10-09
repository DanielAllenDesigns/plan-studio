# Chief X18 manual coverage audit: README

Daniel's request (Round 14): review Chief Architect's own documents and find the features Plan Studio still needs. Seven builders read the whole Reference Manual (1,497 pages) and the whole Tutorial Guide (517 pages) in Round 15, each against the code, and this folder holds their results plus the consolidation (Round 15, docs only).

## Contents of this folder

| File | What it is |
|---|---|
| `part1-program-files-defaults-editing.md` ... `part6-terrain-plants-materials-lists-layout-printing-schedules.md` | The Reference Manual audits: one row per feature (section, printed page, feature in our words, status, evidence), a Dialog panels comparison, a ranked Gaps to build list, counts and the places where the manual contradicts a DECISIONS.md row. |
| `part7-tutorial-workflows.md` | The Tutorial Guide replayed lesson by lesson (28 lessons): steps, assessment-question checks, ranked workflow breaks, a Defaults that differ table. |
| `scenario-proposals.md` | One headless scenario test proposal per tutorial lesson (consumed by Round 16 briefs 34 and 35). |
| `master-gaps.md` | The consolidation: one deduplicated, ranked list (215 entries) of every Missing and Partial finding plus the earlier Top 40. |
| `round16-plan.md` | The 35 Round 16 builder briefs with strict file ownership, waves and the Round 17 leftovers. The same briefs are in `~/plan-studio-dev/briefs/r16/`. |
| `decisions-corrections.md` | Every DECISIONS.md row the audits say contradicts the manual, with a recommendation. |
| `defaults-that-differ.md` | Chief Residential Template vs Daniel's working template vs Plan Studio today, flagged for Daniel's decision. |

## Copyright and what is stored

Chief Architect's Reference Manual and Tutorial Guide are Chief Architect, Inc.'s documents. They are not in this repository and must never be added to it. The audits read them from a private text extraction at `~/plan-studio-dev/chief-docs/` (outside the repo) and recorded every finding in our own words. What the files do contain are facts a user can see in the program: tool names, dialog names, panel names, field labels, default values quoted as numbers, and printed page numbers so a reader can find the passage in their own copy of the manual. No sentence is copied and none of the files reproduces the manual's structure beyond the chapter names that the program's menus use. The same rule governs the briefs: builders read pages with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>` (or `tut`), write code and docs in their own words, and never copy passages, PDFs or the extracted text into the repo or the scratchpad.

## Method

1. **Scope split.** The manual was split into seven page ranges, one audit part each (briefs in `~/plan-studio-dev/briefs/r15/manual_audit/`): part 1 pp. 11-318, part 2 pp. 319-520, part 3 pp. 521-761, part 4 pp. 762-1098, part 5 pp. 1099-1306, part 6 pp. 1307-1497, part 7 the Tutorial Guide pp. 1-517. Page numbers in every table are the **printed** page numbers (the numbers in the manual's own table of contents).
2. **Enumerate everything.** For each chapter the builder listed every tool and tool mode, each specification or defaults dialog with each of its panels and fields (units and defaults when stated), each menu command, edit handle, display option, keyboard or mouse behavior and Default Settings page. A row is one feature, written as one line.
3. **Status check, in a fixed order.** For each feature the builder looked in `docs/chief-feature-coverage.md` (the earlier audit), then in the `docs/parity/*.md` rows (including the "Coverage audit additions"), then in the code under `crates/` (grep for the Action, ToolId, dialog or field, and reading the path where the result mattered), then in `DECISIONS.md`, and finally in the Round 15 briefs to see whether the feature was already being built. Where the earlier audit and the working tree disagreed the tree won.
4. **Part 7 replays the tutorials.** Each of the 28 lessons was walked step by step against the code (tool, dialog, field, in the guide's order) and every assessment question was checked as a feature. A break means the guide's own steps cannot be followed to the end.
5. **New parity ids.** A feature that no parity row covered ("NO SPEC") got a new row appended to the matching `docs/parity/*.md` under a "Manual audit additions (part N)" heading and, with the same text, at the end of `docs/parity-status.md` under a dated heading. The totals table in `parity-status.md` was left alone on purpose: the gate recounts it.
6. **Consolidation (this round).** `master-gaps.md` merges all of it. Rows were assigned to one entry each by parity id (so a feature cited from several pages and several parts lands once) and then by page range; the assignment script is kept next to the briefs (see "Re-running" below), so the grouping is reproducible and every one of the 2,589 Missing/Partial/In-progress rows is accounted for.

### Status words

| Status | Meaning |
|---|---|
| Works | Present and behaving as the manual describes, with evidence (file, function, test or parity id). |
| Partial | Present with a stated gap: a missing field, mode, panel, edge case or a different gesture. |
| Missing | No implementation found. |
| Differs | Different by design or by Daniel's template or by a recorded decision; not counted as a gap unless the row says it is a defect. |
| Out-of-scope | Ruby macros, cloud and licensing, 3D Warehouse, VR and gamepad hardware, vendor services, Windows-only formats, the SketchUp SDK. |
| In progress (Round 15) | A brief in `~/plan-studio-dev/briefs/r15` covers it and the code was not finished when the row was written. |
| NO SPEC | An evidence tag, not a status: no parity id existed; the new id follows ("NO SPEC -> W-120"). |

## Counts

Rows are table rows in the parts' feature tables, counted by script from the files on 2026-10-08 (the status column of each row). Parts 1 to 6 are additive; part 7 replays the same ground from the user's side, so its rows are reported separately and are **not** added to the combined total.

| Part | Manual pages | Feature rows | Works | Partial | Missing | Differs | Out-of-scope | In progress | NO SPEC rows | New parity rows | Dialog-panel rows compared |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 11-318 | 588 | 158 | 218 | 180 | 22 | 9 | 1 | 297 | 297 | 97 |
| 2 | 319-520 | 395 | 83 | 191 | 93 | 4 | 1 | 23 | 171 | 103 | 29 |
| 3 | 521-761 | 585 | 71 | 324 | 168 | 16 | 5 | 1 | 303 | 160 | 41 |
| 4 | 762-1098 | 813 | 120 | 344 | 320 | 10 | 8 | 11 | 490 | 490 | 55 |
| 5 | 1099-1306 | 455 | 59 | 188 | 175 | 12 | 17 | 4 | 207 | 116 | 36 |
| 6 | 1307-1497 | 460 | 77 | 176 | 172 | 9 | 26 | 0 | 274 | 274 | 43 |
| **1 to 6** | **11-1497** | **3,296** | **568** | **1,441** | **1,108** | **73** | **66** | **40** | **1,742** | **1,440** | **301** |
| 7 (Tutorial Guide steps, not additive) | tut. 3-500 | 1,169 | 455 | 421 | 265 | 11 | 5 | 12 | 136 steps without a row | 56 | 17 |
| 7 (assessment questions, not additive) | tut. 3-500 | 198 | 72 | 75 | 46 | 2 | 1 | 2 | - | - | - |

- **Combined features enumerated (parts 1 to 6): 3,296.** Works 568 (17%), Partial 1,441 (44%), Missing 1,108 (34%), Differs 73 (2%), Out-of-scope 66 (2%), In progress 40 (1.2%). Works plus Partial is 61%; Missing plus Partial plus In progress is 2,589 rows.
- **NO SPEC rows added:** 1,742 feature rows had no parity id (parts 1 to 6) and were folded into **1,440 new parity rows**; part 7 added 56 more, **1,496 new parity rows in all**, appended to the parity files and to `docs/parity-status.md` without touching its totals. (Parts 2, 3 and 5 fold several rows into one id, so their new-row counts are below their NO SPEC counts.)
- **Part 1 summary note:** part 1's own counts section says 587 rows and 157 Works; its table now holds 588 rows and 158 Works (one Works row was added after the summary was written). The table figures are used here.
- **Ranked gaps printed by the parts:** 25 + 35 + 33 + 25 + 24 + 22 (parts 1 to 6) and 30 workflow breaks (part 7), 194 in all, all of them mapped in `master-gaps.md`.
- **Deduplicated result:** the 2,589 Missing/Partial/In-progress rows collapse into **215 gap entries** (`master-gaps.md`); 143 parity ids are cited by two or more parts.

## How the statuses should be read, and known caveats

- **A moving target.** The audits ran on 2026-10-08 on branch `wip/round-14-partial` (HEAD 129890a) while about 25 Round 14 and Round 15 builders were editing the tree. Rows marked In progress, and many Partial rows in the areas of the Round 15 briefs, may already be better (or may have been broken by a neighbour). Re-grep the named file before building from any row; the Round 16 briefs say so in their first paragraph.
- **"Not confirmed in code" rows (part 7).** 109 tutorial step rows say the builder read the surrounding dialog or tool but not the exact code path (for example whether a symbol's Label tab has an offset field). Their statuses are best readings, not facts; part 7 lists them in its own caveat section.
- **Stale earlier audit.** `docs/chief-feature-coverage.md` predates Rounds 14 and 15; the parts overrule it where they differ (for example its Wall Specification table, its 3D menu table, its Light Sets and Panorama rows). Its Top 40 is cross-referenced in `master-gaps.md`.
- **Verify in Chief.** Where the manual is ambiguous or silent the rows say "verify in Chief" (about 20 places, e.g. Auto Rebuild Roofs default, Material Painter room mode for walls, the manual contradicting itself on automatic roof trusses, pp. 853 vs 900/937). These should be checked in Daniel's installed Chief before the behavior is changed.
- **Page numbers and structure are Chief's.** A row's page is where the manual describes the feature, not where Plan Studio should put it.
- **The consolidation is mechanical plus judgment.** Row-to-entry assignment is by parity id and page range, so a row occasionally sits in a neighbouring entry (the outliers were reviewed and the worst ones moved); counts per entry are indicators of breadth. Priority scores, sizes and brief groupings are the consolidator's judgment for Daniel's practice (custom homes and remodels in metro Atlanta, construction documents on layout sheets, kitchens and baths, site plans, code compliance) and are meant to be argued with.
- **Sizes** are the parts' own S/M/L (under a day / a few days / a week or more of one builder); briefs cut larger entries at explicit stop lines.
- **Not features:** the Reference Manual's index (pp. 1479-1497), the Resources chapter's support links and error-message page, and the Ruby API (marked Out-of-scope; text macros that Chief offers for labels are covered).

## Where the manual disagrees with DECISIONS.md

About 60 DECISIONS rows and parity statements are contradicted or not supported by the manual. They are consolidated, with a recommendation each, in `decisions-corrections.md`. Wording corrections were applied to DECISIONS.md at the Round 15 gate (rows 6, 13, 19, 41, 47, 53, 58, 70, 76, 77, 112, 116, 118, 122, 208, 300, 305, 310, CD2, DS3, DS4, DS7, DS8 and DS11 already carry a "Manual correction" note or the manual page); behavior changes are assigned to Round 16 briefs.

## Re-running an audit

To refresh one part after the code changes (or to audit a new Chief version):

1. Extract the manual to text once, outside the repo, and keep `pages.py` beside it (`~/plan-studio-dev/chief-docs/`). Read a range with `python3 ~/plan-studio-dev/chief-docs/pages.py ref 521 570` (Reference Manual) or `... tut 82 105` (Tutorial Guide), 20 to 40 pages at a time.
2. Give a builder the common instructions and the part brief: `~/plan-studio-dev/briefs/r15/manual_audit/common.md` and `part1.md` ... `part7.md`. The builder rewrites the matching `partN-*.md` (same table layout: Section | Page | Feature | Status | Evidence), appends parity rows under "Manual audit additions (part N)" for every NO SPEC feature, and reports counts.
3. Recount a part from its file with the table parser in `~/plan-studio-dev/briefs/r15/manual_audit/tools/` (`ext.py` reads every feature table of every part and writes `rows.json`; `gen_readme.py` recomputes the per-part counts in this README from it). A quick check without the tools: count rows per status in the main table with `awk -F'|' '/^\| [A-Za-z]/ {print $5}' partN-*.md | sort | uniq -c` (column 5 holds the status in the Section/Page/Feature/Status/Evidence layout; column 4 for the Page/Feature/Status/Evidence layout).
4. Re-run the consolidation: `cd ~/plan-studio-dev/briefs/r15/manual_audit/tools && python3 ext.py && python3 dump.py && python3 srcgaps.py && cat briefs_a.py briefs_b.py briefs_c.py > briefs_all.py && python3 gen_master.py && python3 gen_plan.py && python3 gen_readme.py && python3 gen_decisions.py && python3 gen_defaults.py` regenerates `master-gaps.md`, `round16-plan.md`, the briefs in `~/plan-studio-dev/briefs/r16/` and the counts. Cluster definitions (`cl1.py` ... `cl5.py`), the brief definitions (`briefs_*.py`) and the source-gap mapping (`srcmap.py`) are plain Python lists: edit them, not the generated markdown. `check_ownership.py` verifies that no two briefs own the same file.
5. After any audit, recount `docs/parity-status.md` totals with `python3 scripts/parity-score.py --check-totals` at the gate.

## Related documents

`docs/chief-feature-coverage.md` (earlier audit and Top 40), `docs/parity-status.md`, `docs/parity/*.md`, `DECISIONS.md`, `docs/integration-queue.md`, `docs/daniel-template-inventory.md`, `docs/daniel-chief-setup.md`.
