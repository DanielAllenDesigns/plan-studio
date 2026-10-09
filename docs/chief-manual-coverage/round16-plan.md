# Round 16 plan: 35 builder briefs from the manual audits

Written 2026-10-08 (Round 15 consolidation, docs only). Source of the ranking: `master-gaps.md` in this folder (215 deduplicated entries). Each brief below is also a standalone file in the orchestrator's brief folder, `~/plan-studio-dev/briefs/r16/NN-slug.md`, in the same style as `~/plan-studio-dev/briefs/r15/*.md`, with the standard first line telling the builder to read `preamble.md`.

## Rules these briefs follow

- **Sized for one builder agent, 2 to 5 hours.** Larger entries in the master list are cut at an explicit stop line inside the brief ("stop after item N and queue the rest in docs/integration-queue.md"); what is cut lands in the Round 17 candidate list at the end.
- **Strictly disjoint file ownership.** Every file a brief lists under "Files you own" belongs to that brief alone; the script check at the end of this file reports no path owned by two briefs (a directory pattern `x/**` conflicts with anything under it). Files that other briefs also need are listed as "shared, targeted": small edits with the Edit tool after re-reading the file, never a whole-file rewrite, exactly as `preamble.md` says. Where a shared file has an owner, the owner is named.
- **Round 15 hand-offs.** A brief that touches files a Round 15 builder is editing starts only after that builder reports (column "Starts when"); it reads that builder's notes in `docs/integration-queue.md` first and builds only what was left.
- **The Round 15 gate comes first.** `~/plan-studio-dev/briefs/r14c/integration2.md` and the gate pass apply one-line hooks across owners (layout hooks, terrain handles, roof handle mapping, stale menu rows). Wave A briefs touch some of those files, so each builder re-reads the current file before every edit and checks `docs/integration-queue.md` for items the gate has not closed yet.
- **Scenario numbers.** Round 15 used s42 to s58. Round 16 uses s60 to s92 (one per feature brief, in brief order, plus two tutorial folders). Registration is one `mod` line in `scenarios/mod.rs`, which every brief lists as shared.
- **Every brief updates** the matching `docs/parity/*.md` rows (status plus evidence), appends leftovers to `docs/integration-queue.md`, and writes a DECISIONS.md row for each manual contradiction it resolves (the list is in `decisions-corrections.md`). Nobody edits `docs/parity-status.md` totals (the gate recounts).

## Summary

| # | Brief | Master ranks | Est. hours | Starts when | Notes |
|---|---|---|---|---|---|
| 01 | [layout-pages](#brief-01-layout-pages) | 1 | 4 | Wave A: now |  |
| 02 | [layout-boxes](#brief-02-layout-boxes) | 20, 27, 82, 101 | 5 | After R15: callouts |  |
| 03 | [print-sheet-watermark](#brief-03-print-sheet-watermark) | 33, 39, 56, 171 | 4 | Wave A: now |  |
| 04 | [schedules](#brief-04-schedules) | 10, 17, 80, 100 | 5 | After R15: excel_roundtrip, materials_list |  |
| 05 | [common-object-pages](#brief-05-common-object-pages) | 18, 43 | 4 | After R15: excel_roundtrip, materials_list |  |
| 06 | [survey-entry](#brief-06-survey-entry) | 2, 29, 152 | 5 | After R15: cad2 |  |
| 07 | [construction-lines-reference](#brief-07-construction-lines-reference) | 8, 44 | 5 | Wave A: now |  |
| 08 | [line-fill-styles-poche](#brief-08-line-fill-styles-poche) | 23, 35, 40 | 5 | Wave A: now |  |
| 09 | [layers-drawing-groups](#brief-09-layers-drawing-groups) | 14, 104, 165 | 4 | After R15: cad2 |  |
| 10 | [edit-behaviours-keys](#brief-10-edit-behaviours-keys) | 5, 12, 19, 34, 102, 148 | 5 | After R15: walls2, cad2 |  |
| 11 | [cad-edit-tools-area](#brief-11-cad-edit-tools-area) | 66, 69, 89, 111, 118, 135, 146, 156, 161, 177, 179 | 5 | After R15: cad2 | soft order after 06 |
| 12 | [wall-types-layers](#brief-12-wall-types-layers) | 4, 128 | 5 | After R15: walls2 |  |
| 13 | [layered-assemblies](#brief-13-layered-assemblies) | 6 | 5 | After R15: defaults_pages |  |
| 14 | [wall-edit-tools](#brief-14-wall-edit-tools) | 49, 72, 132, 137, 144 | 5 | After R15: walls2 |  |
| 15 | [rooms-living-area](#brief-15-rooms-living-area) | 42, 65, 67, 112 | 5 | After R15: decks_chimneys, defaults_pages |  |
| 16 | [tray-ceilings](#brief-16-tray-ceilings) | 95 | 4 | Wave A: now | soft order after 15 |
| 17 | [floors-foundation-fireplace](#brief-17-floors-foundation-fireplace) | 63, 85, 105, 120, 180 | 5 | After R15: decks_chimneys |  |
| 18 | [roof-eaves-heights](#brief-18-roof-eaves-heights) | 3, 28, 103, 119 | 5 | After R15: walls2 | soft order after 12 |
| 19 | [roof-baselines-groups-curved](#brief-19-roof-baselines-groups-curved) | 76, 81, 163 | 5 | Wave A: now | soft order after 15, 18 |
| 20 | [roof-trim-dormers-skylights](#brief-20-roof-trim-dormers-skylights) | 75, 83, 141, 149 | 5 | Wave A: now | soft order after 31, 18 |
| 21 | [stairs-engine](#brief-21-stairs-engine) | 15, 48, 61, 62, 91, 99, 138, 164, 185 | 5 | After R15: stairs2 |  |
| 22 | [openings-mulled-bay](#brief-22-openings-mulled-bay) | 22, 32, 51, 116, 133 | 5 | After R15: opening_tabs |  |
| 23 | [cabinet-runs-labels](#brief-23-cabinet-runs-labels) | 31, 50, 129 | 4 | Wave A: now |  |
| 24 | [cabinet-faces-specials](#brief-24-cabinet-faces-specials) | 54, 55, 77, 113, 181 | 5 | Wave A: now | soft order after 23 |
| 25 | [electrical-defaults-connections](#brief-25-electrical-defaults-connections) | 57, 110, 126 | 4 | After R15: defaults_pages |  |
| 26 | [saved-defaults-views](#brief-26-saved-defaults-views) | 16, 21, 36, 41, 58, 74, 131 | 5 | After R15: defaults_pages |  |
| 27 | [text-macros-rescheck](#brief-27-text-macros-rescheck) | 24, 68, 115, 162, 192 | 5 | After R15: callouts, painters_spell |  |
| 28 | [dimension-segments](#brief-28-dimension-segments) | 11, 26, 73 | 5 | After R15: dims2 | soft order after 10 |
| 29 | [framing-members-reporting](#brief-29-framing-members-reporting) | 70, 78 | 5 | After R15: decks_chimneys | soft order after 30 |
| 30 | [framing-layout-trusses](#brief-30-framing-layout-trusses) | 46, 47, 60, 71, 147 | 5 | After R15: decks_chimneys |  |
| 31 | [moldings-trim](#brief-31-moldings-trim) | 38, 159 | 5 | Wave A: now | soft order after 15 |
| 32 | [camera-section-annotation](#brief-32-camera-section-annotation) | 7, 13, 59, 117, 125 | 5 | After R15: cameras2 | soft order after 28 |
| 33 | [terrain-site](#brief-33-terrain-site) | 25, 30, 45, 64, 140, 151 | 5 | Wave A: now |  |
| 34 | [tutorial-replay-a](#brief-34-tutorial-replay-a) | QA | 4 | Wave A: now |  |
| 35 | [tutorial-replay-b](#brief-35-tutorial-replay-b) | QA | 4 | After brief 34 reports |  |

Total estimate: 166 builder-hours across 35 briefs (33 feature briefs, 2 test-only tutorial briefs).

## What can run concurrently

Because owned files are disjoint, **any subset of the briefs can run at the same time**. The only things that limit concurrency are (a) Round 15 hand-offs, (b) the hot shared files below, where simultaneous targeted edits are safe but review is easier if the edits arrive from fewer builders, and (c) one real dependency: brief 35 needs the support file brief 34 writes.

- **Wave A, start as soon as the Round 15 gate is green or earlier (12 briefs, no Round 15 prerequisite):** 01 layout-pages, 03 print-sheet-watermark, 07 construction-lines-reference, 08 line-fill-styles-poche, 16 tray-ceilings, 19 roof-baselines-groups-curved, 20 roof-trim-dormers-skylights, 23 cabinet-runs-labels, 24 cabinet-faces-specials, 31 moldings-trim, 33 terrain-site, 34 tutorial-replay-a.
- **Wave B, start when the named Round 15 builders have reported (22 briefs):**

| Brief | Waits for | Why |
|---|---|---|
| 02 layout-boxes | callouts | they are editing tools/text.rs, dialogs/text/**, plan-core text/callout/note types, plan-layout render.rs hooks |
| 04 schedules | excel_roundtrip, materials_list | they are editing plan-docs xlsx.rs and xlsx_read.rs, props_exchange.rs, plan-core props.rs, dialogs/property_manager.rs, import_review.rs; plan-docs materials/**, master_list.rs, the Materials List window |
| 05 common-object-pages | excel_roundtrip, materials_list | they are editing plan-docs xlsx.rs and xlsx_read.rs, props_exchange.rs, plan-core props.rs, dialogs/property_manager.rs, import_review.rs; plan-docs materials/**, master_list.rs, the Materials List window |
| 06 survey-entry | cad2 | they are editing plan-core cad.rs and clip.rs, tools/cad/**, tools/select.rs, editor/edit_commands.rs, dialogs/multiple_copy.rs, dxf_options.rs, drawing groups |
| 09 layers-drawing-groups | cad2 | they are editing plan-core cad.rs and clip.rs, tools/cad/**, tools/select.rs, editor/edit_commands.rs, dialogs/multiple_copy.rs, dxf_options.rs, drawing groups |
| 10 edit-behaviours-keys | walls2, cad2 | they are editing plan-core walls.rs, wall_spec.rs, joins.rs, tools/wall.rs, dialogs/wall.rs and wall/tabs.rs, editor/wall_edit.rs, connect.rs, plan-3d wall.rs; plan-core cad.rs and clip.rs, tools/cad/**, tools/select.rs, editor/edit_commands.rs, dialogs/multiple_copy.rs, dxf_options.rs, drawing groups |
| 11 cad-edit-tools-area | cad2 | they are editing plan-core cad.rs and clip.rs, tools/cad/**, tools/select.rs, editor/edit_commands.rs, dialogs/multiple_copy.rs, dxf_options.rs, drawing groups |
| 12 wall-types-layers | walls2 | they are editing plan-core walls.rs, wall_spec.rs, joins.rs, tools/wall.rs, dialogs/wall.rs and wall/tabs.rs, editor/wall_edit.rs, connect.rs, plan-3d wall.rs |
| 13 layered-assemblies | defaults_pages | they are editing dialogs/defaults.rs, default_lists.rs, default_settings_*.rs, default_pages/**, floor_defaults.rs, plan-core defaults.rs, shell/view_commands.rs |
| 14 wall-edit-tools | walls2 | they are editing plan-core walls.rs, wall_spec.rs, joins.rs, tools/wall.rs, dialogs/wall.rs and wall/tabs.rs, editor/wall_edit.rs, connect.rs, plan-3d wall.rs |
| 15 rooms-living-area | decks_chimneys, defaults_pages | they are editing tools/fireplace.rs, dialogs/fireplace.rs, plan-3d fireplace.rs and deck.rs, plan-core fireplace.rs, plan-framing deck module, rooms.rs deck fields; dialogs/defaults.rs, default_lists.rs, default_settings_*.rs, default_pages/**, floor_defaults.rs, plan-core defaults.rs, shell/view_commands.rs |
| 17 floors-foundation-fireplace | decks_chimneys | they are editing tools/fireplace.rs, dialogs/fireplace.rs, plan-3d fireplace.rs and deck.rs, plan-core fireplace.rs, plan-framing deck module, rooms.rs deck fields |
| 18 roof-eaves-heights | walls2 | they are editing plan-core walls.rs, wall_spec.rs, joins.rs, tools/wall.rs, dialogs/wall.rs and wall/tabs.rs, editor/wall_edit.rs, connect.rs, plan-3d wall.rs |
| 21 stairs-engine | stairs2 | they are editing plan-stairs/**, tools/stairs.rs, dialogs/stairs.rs, editor/stairs_view.rs, plan-3d railing.rs |
| 22 openings-mulled-bay | opening_tabs | they are editing dialogs/opening.rs, plan-core openings/**, opening_symbol.rs, plan-3d windows.rs, doors.rs, casing.rs, opening.rs, tools/opening/** |
| 25 electrical-defaults-connections | defaults_pages | they are editing dialogs/defaults.rs, default_lists.rs, default_settings_*.rs, default_pages/**, floor_defaults.rs, plan-core defaults.rs, shell/view_commands.rs |
| 26 saved-defaults-views | defaults_pages | they are editing dialogs/defaults.rs, default_lists.rs, default_settings_*.rs, default_pages/**, floor_defaults.rs, plan-core defaults.rs, shell/view_commands.rs |
| 27 text-macros-rescheck | callouts, painters_spell | they are editing tools/text.rs, dialogs/text/**, plan-core text/callout/note types, plan-layout render.rs hooks; tools/painters.rs, dialogs/painters.rs, spell.rs, dialogs/spell_check.rs |
| 28 dimension-segments | dims2 | they are editing plan-core dimension.rs, tools/dimension.rs, editor/dim_assoc.rs, dialogs/dimension.rs |
| 29 framing-members-reporting | decks_chimneys | they are editing tools/fireplace.rs, dialogs/fireplace.rs, plan-3d fireplace.rs and deck.rs, plan-core fireplace.rs, plan-framing deck module, rooms.rs deck fields |
| 30 framing-layout-trusses | decks_chimneys | they are editing tools/fireplace.rs, dialogs/fireplace.rs, plan-3d fireplace.rs and deck.rs, plan-core fireplace.rs, plan-framing deck module, rooms.rs deck fields |
| 32 camera-section-annotation | cameras2 | they are editing plan-view3d/**, plan-render encoders and panorama, dialogs/camera.rs, tools/camera.rs, shell/view3d_panel/** |

- **Brief 35** starts after brief 34 reports (it reuses `scenarios/tutorials_support.rs`).
- Suggested load: no more than 12 builders at a time, taking Wave A first in the order of the table, then Wave B in table order as Round 15 reports arrive; briefs 01, 06, 10, 12, 18 and 04 are the highest-ranked and should not wait behind the others.

### Hot shared files (edited in a targeted way by three or more briefs)

| File | Owner brief | Briefs that also touch it |
|---|---|---|
| `crates/plan-app/src/scenarios/mod.rs` | none | 01, 02, 03, 04, 05, 06, 07, 08, 09, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35 |
| `crates/plan-app/src/menus.rs` | none | 01, 02, 03, 04, 06, 07, 08, 09, 11, 12, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33 |
| `crates/plan-app/src/toolbar.rs` | none | 01, 02, 04, 06, 07, 09, 11, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 28, 30, 31, 32, 33 |
| `crates/plan-core/src/model.rs` | none | 03, 04, 05, 07, 08, 09, 11, 14, 15, 16, 17, 18, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 32, 33 |
| `crates/plan-core/src/lib.rs` | none | 03, 05, 06, 07, 08, 10, 11, 12, 13, 14, 15, 16, 22, 27, 31 |
| `crates/plan-core/src/defaults.rs` | 26 | 03, 12, 13, 15, 17, 22, 23, 28, 29 |
| `crates/plan-core/src/rooms.rs` | 15 | 13, 16, 17, 19, 21, 30, 31 |
| `crates/plan-app/src/editor/render.rs` | none | 07, 08, 09, 11, 26, 28 |
| `crates/plan-app/src/tools/select.rs` | 11 | 02, 10, 14, 18, 28 |
| `crates/plan-app/src/tools/mod.rs` | none | 07, 16, 19, 20, 31 |
| `crates/plan-layout/src/render.rs` | 02 | 01, 04, 09, 28 |
| `crates/plan-app/src/dispatch.rs` | none | 01, 02, 07, 11 |
| `crates/plan-app/src/shell/docks.rs` | none | 01, 04, 26, 30 |
| `crates/plan-app/src/main.rs` | none | 03, 10, 26, 33 |
| `crates/plan-docs/src/schedule_kinds.rs` | 04 | 22, 23, 25, 33 |
| `crates/plan-layout/src/lib.rs` | none | 01, 02, 03 |
| `crates/plan-app/src/editor/snap.rs` | 10 | 07, 14, 23 |
| `crates/plan-core/src/walls.rs` | 14 | 12, 18, 30 |
| `crates/plan-app/src/dialogs/default_pages/architectural.rs` | none | 12, 15, 22 |
| `crates/plan-3d/src/lib.rs` | none | 16, 20, 31 |

The unowned ones (menus.rs, toolbar.rs, dispatch.rs, main.rs, model.rs, lib.rs files, scenarios/mod.rs) are the places every feature brief touches; each edit is a few lines, so merge friction is expected to be small.

## The briefs

### Brief 01 layout-pages

**Layout page management: Page Information, # numbering, page templates per page, revisions, page tables**

- **Goal:** Page Information dialog with Label patterns (A0.#), per-page templates, page revisions and placeable Layout Revision / Layout Page tables.
- **Master gaps covered:** 1 Page Information on layout pages (M)
- **Parity ids:** L-1, L-5, L-7..9, L-11, L-15, L-16, L-54, L-138, L-139, L-141..144, L-146, L-185..189, L-191..196
- **Manual page refs (printed page numbers):** 1393-1396, 1417-1422; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite.
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-layout/src/model.rs`
  - `crates/plan-layout/src/template.rs`
  - `crates/plan-layout/src/titleblock.rs`
  - `crates/plan-layout/src/arrange.rs`
  - `crates/plan-app/src/dialogs/layout.rs`
  - `crates/plan-app/src/dialogs/page_info.rs`
  - `crates/plan-app/src/dialogs/layout_revisions.rs`
  - `crates/plan-app/src/scenarios/s60_layout_pages.rs`
- **Shared files (targeted edits only):**
  - crates/plan-layout/src/lib.rs
  - crates/plan-layout/src/render.rs (macro evaluation hook; owner is brief 02)
  - crates/plan-docs/src/pdf/sheet.rs (macro hook)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/dispatch.rs
  - crates/plan-app/src/shell/layout_window.rs (page navigation hook; owner is brief 02)
  - crates/plan-app/src/shell/docks.rs (page rows in the Project Browser)
  - crates/plan-app/src/scenarios/mod.rs (one mod line)
- **Tests required:** Unit tests in plan-layout for the # numbering engine (next free number, reorder renumber, mixed prefixes), macro evaluation, revision table rows, page table rows; serde round trip of the new page fields (all serde-default so older plans open); scenario s60_layout_pages.rs: create three pages with A0.#/A1.# patterns, assign a template, add revisions, place both tables, reorder pages and assert labels, undo/redo as one step each, PDF text contains the evaluated macros.
- **Docs to update:** docs/parity/documentation-layout.md (L-188..L-192, L-230, L-236 rows), docs/manual/11-layout-schedules-print.md (Page Information section, append)

### Brief 02 layout-boxes

**Layout boxes and Send to Layout: site-plan scales, linked views, box specification, rescale tools, plot lines**

- **Goal:** Send to Layout options, Layout Box Specification panels, Pan/Scale/Rescale tools, view update commands and Edit Layout Lines.
- **Master gaps covered:** 27 Send to Layout options (M); 20 Keeping layout views current and editing them (L); 101 Edit Layout Lines and Plot Lines (L); 82 CAD and text on layout pages (M)
- **Parity ids:** L-3..6, L-12, L-13, L-16, L-22, L-145, L-147..149, L-152..184, L-199, L-200, L-204, L-207
- **Manual page refs (printed page numbers):** 1396-1415, 1425-1429; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: callouts (they edit callouts: tools/text.rs, dialogs/text/**, plan-core text/callout/note types, plan-layout render.rs hooks).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-layout/src/send.rs`
  - `crates/plan-layout/src/extent.rs`
  - `crates/plan-layout/src/canvas.rs`
  - `crates/plan-layout/src/clip.rs`
  - `crates/plan-layout/src/hatch.rs`
  - `crates/plan-layout/src/overlay.rs`
  - `crates/plan-layout/src/annot.rs`
  - `crates/plan-layout/src/cadattrs.rs`
  - `crates/plan-layout/src/layers.rs`
  - `crates/plan-layout/src/render.rs`
  - `crates/plan-app/src/shell/layout_window.rs`
  - `crates/plan-app/src/dialogs/layout_box.rs`
  - `crates/plan-app/src/dialogs/send_to_layout.rs`
  - `crates/plan-app/src/scenarios/s61_layout_boxes.rs`
- **Shared files (targeted edits only):**
  - crates/plan-layout/src/lib.rs
  - crates/plan-layout/src/model.rs (LayoutBox fields, serde-default; owner is brief 01)
  - crates/plan-app/src/tools/select.rs (layout context hooks; owner is brief 11)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/dispatch.rs
  - crates/plan-app/src/shell/view3d_panel.rs (Send to Layout button hook)
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-layout unit tests for scale math (1 in = 100 ft is exactly 1:1200 paper-to-model), too-large detection, Box Scale modes, crop/pan geometry, plot-line extraction on a synthetic box; scenario s61_layout_boxes.rs: send a plan at 1 ft = 100 ft and a section as Plot Lines to two pages, rescale, pan/scale, link a saved view and change the view then update, assert counts and one undo step each.
- **Docs to update:** docs/parity/documentation-layout.md (L-145, L-150, L-155, L-156, L-161, L-199..L-207, L-229, L-231, L-232), docs/manual/11-layout-schedules-print.md

### Brief 03 print-sheet-watermark

**Print dialog, Drawing Sheet Setup per view, program-wide sheet sizes, line weights, Watermark**

- **Goal:** Print Source, Check Plot, Collate, Scale to Fit, per-view sheet setup with drawing scale, program-wide sheet sizes and Watermark.
- **Master gaps covered:** 39 Print dialog (M); 56 Drawing Sheet Setup per view with Drawing Scale and per-edge margins feeding Send to La... (M); 171 Line weight printing (S); 33 Watermark (M)
- **Parity ids:** L-8, L-12, L-18, L-19, L-22, L-53, L-197, L-201..203, L-205, L-206, L-208..227
- **Manual page refs (printed page numbers):** 184, 220, 1423-1444; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite.
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-layout/src/print.rs`
  - `crates/plan-docs/src/pdf/mod.rs`
  - `crates/plan-docs/src/pdf/sheet.rs`
  - `crates/plan-docs/src/pdf/scale.rs`
  - `crates/plan-docs/src/pdf/truetype.rs`
  - `crates/plan-docs/src/pdf/font_tests.rs`
  - `crates/plan-docs/src/pdf/mode_tests.rs`
  - `crates/plan-app/src/dialogs/print.rs`
  - `crates/plan-app/src/dialogs/watermark.rs`
  - `crates/plan-app/src/dialogs/drawing_sheet.rs`
  - `crates/plan-core/src/watermark.rs`
  - `crates/plan-app/src/scenarios/s62_print_watermark.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/lib.rs (mod line)
  - crates/plan-core/src/model.rs (watermark and drawing-sheet slots, serde-default)
  - crates/plan-core/src/defaults.rs (Watermark defaults group; owner is brief 26)
  - crates/plan-layout/src/lib.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/main.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: check-plot scale math, scale-to-fit selection on a known plan extent, margin application, watermark placement for tile/border/fit (positions and rotation), sheet-size file round trip, old per-layout sizes still load; scenario s62_print_watermark.rs: print preview of a layout page with watermark on and off, drawing scale feeding the Send to Layout default, PDF page count and size at ARCH D and Letter with Check Plot 1/2.
- **Docs to update:** docs/parity/documentation-layout.md (L-19, L-53, L-192, L-199..L-211), docs/manual/11-layout-schedules-print.md

### Brief 04 schedules

**Schedules at construction-document quality**

- **Goal:** Categories tree, room scope, totals, wrapping, number formatting, preview columns, edit tools and Find Object in Plan.
- **Master gaps covered:** 17 Schedule Specification (M); 10 Schedule table quality (L); 100 Schedule handles and edit tools (M); 80 Find Object in Plan from schedule/materials rows, Find Schedule(s) from Object, Find Wa... (M)
- **Parity ids:** CB-68, L-23, L-25, L-27, L-28, L-41, L-58, L-59, L-61, L-63..66, L-68..72, S-118..120, TXT-30
- **Manual page refs (printed page numbers):** 92-93, 699, 708-726; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: excel_roundtrip, materials_list (they edit excel_roundtrip: plan-docs xlsx.rs and xlsx_read.rs, props_exchange.rs, plan-core props.rs, dialogs/property_manager.rs, import_review.rs; materials_list: plan-docs materials/**, master_list.rs, the Materials List window).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-docs/src/schedule.rs`
  - `crates/plan-docs/src/schedule_kinds.rs`
  - `crates/plan-core/src/schedules.rs`
  - `crates/plan-app/src/dialogs/schedule_spec.rs`
  - `crates/plan-app/src/dialogs/schedule_categories.rs`
  - `crates/plan-app/src/editor/schedule_view.rs`
  - `crates/plan-app/src/tools/schedule.rs`
  - `crates/plan-app/src/dialogs/select_location.rs`
  - `crates/plan-app/src/scenarios/s63_schedules.rs`
- **Shared files (targeted edits only):**
  - crates/plan-docs/src/lib.rs
  - crates/plan-docs/src/xlsx.rs (public writer only)
  - crates/plan-layout/src/render.rs (schedule table drawing hook; owner is brief 02)
  - crates/plan-app/src/editor/selection.rs (Find in Plan; owner is brief 11)
  - crates/plan-app/src/shell/docks.rs (schedule rows)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-docs for totals, sum-similar, minimum rows, wrapping break positions, fractional-inch formatting, swap rows/columns, category filtering by tree selection, room scope; scenario s63_schedules.rs: door/window/room-finish schedules on a small house, wrap at a max table size, Open Row Object selects the object, Find in Plan from a row, renumber closes gaps, export to XLSX keeps the totals row; one undo step per edit.
- **Docs to update:** docs/parity/documentation-layout.md (L-57..L-75, L-233..L-235), docs/parity/select-and-edit.md (S-118..S-120)

### Brief 05 common-object-pages

**Common object pages: Object Information, Components, Manufacturer, Schedule, Label and the Elevation Reference widget**

- **Goal:** One shared host for the Object Information / Components / Manufacturer / Schedule / Label panels and an Elevation Reference selector used by every height field.
- **Master gaps covered:** 43 Object labels in plan, camera and section views (L); 18 Elevation Reference (L)
- **Parity ids:** DW-49, DW-63, DW-104, L-34, L-74, L-135, LAY-36, R-89, R-90
- **Manual page refs (printed page numbers):** 21, 587, 619, 669, 728-734, 758, 1028-1030, 1071, +1 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: excel_roundtrip, materials_list (they edit excel_roundtrip: plan-docs xlsx.rs and xlsx_read.rs, props_exchange.rs, plan-core props.rs, dialogs/property_manager.rs, import_review.rs; materials_list: plan-docs materials/**, master_list.rs, the Materials List window).
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-app/src/shell/spec_dialogs.rs`
  - `crates/plan-app/src/dialogs/object_info.rs`
  - `crates/plan-app/src/dialogs/property_manager.rs`
  - `crates/plan-app/src/dialogs/components_panel.rs`
  - `crates/plan-app/src/dialogs/elevation_ref.rs`
  - `crates/plan-core/src/props.rs`
  - `crates/plan-core/src/elevation_ref.rs`
  - `crates/plan-app/src/scenarios/s64_common_pages.rs`
- **Shared files (targeted edits only):**
  - crates/plan-app/src/dialogs/opening.rs (brief 22)
  - crates/plan-app/src/dialogs/cabinet.rs (brief 24)
  - crates/plan-app/src/dialogs/electrical.rs (brief 25)
  - crates/plan-app/src/dialogs/symbol.rs
  - crates/plan-app/src/dialogs/wall.rs (brief 12)
  - crates/plan-app/src/dialogs/room.rs (brief 15)
  - crates/plan-app/src/dialogs/stairs.rs (brief 21)
  - crates/plan-core/src/model.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: ElevationRef resolver for every reference (including a split-level floor and sloped terrain), props round trip, label macro expansion; scenario s64_common_pages.rs: open each opted-in dialog, edit the Object Information fields, see the value in a schedule column and the label on the plan, change a cabinet's elevation reference from Floor to Ceiling and see the 3D position follow a ceiling height edit, one undo step each.
- **Docs to update:** docs/parity/documentation-layout.md (L-29, L-30, L-35), docs/parity/doors-windows.md (DW-118), docs/parity/walls.md (W-118), docs/parity/rooms-floors.md (R-89, R-90)

### Brief 06 survey-entry

**Survey-style drafting: bearings, Number and Angle Style, Input Line/Arc/Point dialogs, Current Point, Show Length and Angle**

- **Goal:** Enter a lot from a survey: bearings, Input dialogs with Next, Current Point, Move Point, length/angle labels, five arc modes.
- **Master gaps covered:** 2 Survey-style entry (L); 29 Number Style and Angle Style dialogs (M); 152 Line and Arc Specification (M)
- **Parity ids:** CAD-2, CAD-5, CAD-7, CAD-8, CAD-22, CAD-39, CAD-40, CAD-60, CAD-108, CAD-109, CAD-112..114, CAD-116..119, CAD-121, CAD-134, CB-44, CB-543, DIM-6, L-38, PR-16, PR-26, PR-30, PR-31, W-64, W-67
- **Manual page refs (printed page numbers):** 139, 156, 189, 320-344, 355, 367, 387, 1308, +2 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: cad2 (they edit cad2: plan-core cad.rs and clip.rs, tools/cad/**, tools/select.rs, editor/edit_commands.rs, dialogs/multiple_copy.rs, dxf_options.rs, drawing groups).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/units.rs`
  - `crates/plan-core/src/bearing.rs`
  - `crates/plan-app/src/tools/cad/arcs.rs`
  - `crates/plan-app/src/tools/cad/survey.rs`
  - `crates/plan-app/src/dialogs/cad.rs`
  - `crates/plan-app/src/dialogs/input_line.rs`
  - `crates/plan-app/src/dialogs/input_arc.rs`
  - `crates/plan-app/src/dialogs/move_point.rs`
  - `crates/plan-app/src/dialogs/number_style.rs`
  - `crates/plan-app/src/scenarios/s65_survey_entry.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/cad.rs (CadAttrs show-length/angle fields; owner is brief 11)
  - crates/plan-app/src/tools/cad.rs (tool hooks; owner is brief 11)
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/dialogs/preferences/pages.rs (Number Style button)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-core for bearing parse/format round trips (all four quadrants, azimuth, DMS with seconds, invalid input), Number/Angle Style formatting, arc construction for each of the five modes, traverse closure; scenario s65_survey_entry.rs: the six-course plot plan above, Next chaining, Show Length/Angle labels equal the typed values, undo steps one per entered line, the lot converts to a Terrain Perimeter.
- **Docs to update:** docs/parity/dimensions-text-cad.md (CAD rows above), docs/parity/preferences-hotkeys-toolbars.md (PR-16, PR-26, PR-30, PR-31, DS-47)

### Brief 07 construction-lines-reference

**Construction Lines and the Reference Display**

- **Goal:** Construction Line tool with ordering rules and callouts; Reference Display with several rows, other plan files, offsets and floor swapping.
- **Master gaps covered:** 8 Construction Lines (L); 44 Reference Display with several rows (L)
- **Parity ids:** CAD-62..66, LAY-9, LAY-39..45, R-96
- **Manual page refs (printed page numbers):** 81-91, 246, 331; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite.
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/construction.rs`
  - `crates/plan-app/src/tools/construction_line.rs`
  - `crates/plan-app/src/dialogs/construction_line.rs`
  - `crates/plan-app/src/dialogs/construction_order.rs`
  - `crates/plan-app/src/dialogs/reference_display.rs`
  - `crates/plan-app/src/editor/ref_overlay.rs`
  - `crates/plan-app/src/scenarios/s66_construction_reference.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/lib.rs
  - crates/plan-core/src/model.rs (floor slots, serde-default)
  - crates/plan-app/src/editor/snap.rs (construction snaps; owner is brief 10)
  - crates/plan-app/src/editor/render.rs (draw hooks)
  - crates/plan-app/src/tools/mod.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/dispatch.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: construction-line intersection/snap math (infinite lines, parallel, perpendicular), ordering rule sets producing callout labels, reference-row transform (offset + rotation) mapping; scenario s66_construction_reference.rs: draw two construction lines, snap a wall corner to their intersection, dimension to one, set a reference row from a second plan file with an offset and assert the overlay geometry, swap reference floor, undo steps.
- **Docs to update:** docs/parity/dimensions-text-cad.md (CAD-62..CAD-67, LAY-9, LAY-39..LAY-45), docs/parity/select-and-edit.md (S-194)

### Brief 08 line-fill-styles-poche

**Line style library, Fill Style panel and library, Poché and custom hatch patterns**

- **Goal:** Line Style Management and editor, one Fill Style panel with library, wall/platform/roof poché in plan and section, pattern creation.
- **Master gaps covered:** 35 Line styles as a library (L); 40 Fill Style panel everywhere (L); 23 Poché on cut walls, platforms and roofs, plus custom hatch patterns (L)
- **Parity ids:** C-55, C-61, CAD-56, CAD-69..86, CB-46, LAY-18, LAY-55, RF-100, S-116, S-174
- **Manual page refs (printed page numbers):** 180, 205, 221-232, 305, 329-332, 392, 838, 1320, +1 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite.
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/line_styles.rs`
  - `crates/plan-core/src/fill_styles.rs`
  - `crates/plan-core/src/patterns.rs`
  - `crates/plan-materials/src/pattern.rs`
  - `crates/plan-app/src/dialogs/line_style.rs`
  - `crates/plan-app/src/dialogs/fill_style.rs`
  - `crates/plan-app/src/dialogs/pattern_editor.rs`
  - `crates/plan-elevation/src/hatch.rs`
  - `crates/plan-app/src/scenarios/s67_line_fill_poche.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/lib.rs
  - crates/plan-core/src/model.rs
  - crates/plan-core/src/cad.rs (style field type; owner is brief 11)
  - crates/plan-app/src/editor/render.rs (stroker hook)
  - crates/plan-layout/src/hatch.rs (owner is brief 02: call the shared stroker only)
  - crates/plan-app/src/tools/painters.rs (Fill Style Painter hook)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-core for line-style stroking (dash phase continuity round corners, text component spacing, closed shapes), .lin parser on synthetic files, fill-style scale/offset/angle tile math, poché region selection (cut walls only); scenario s67_line_fill_poche.rs: create a custom dash style and a text style, apply to a line and a property line, set a wall type fill and see poché in a plan view and a cross section, create a pattern from CAD and use it on a slab, one undo step per action.
- **Docs to update:** docs/parity/dimensions-text-cad.md (CAD-56, CAD-69..CAD-86, LAY-55), docs/parity/roofs.md (RF-100)

### Brief 09 layers-drawing-groups

**Layer management and drawing groups**

- **Goal:** New/Copy/Merge/Delete layers, Layer Set Defaults, Object Layer Properties, Select Layer, drawing group tools and Chief's default numbers.
- **Master gaps covered:** 14 Layer management (M); 104 Drawing groups (M); 165 Wall system display layers (M)
- **Parity ids:** C-50, CAD-139, CB-60, CB-269, DW-116, LAY-3, LAY-6, LAY-11, LAY-18, LAY-36, LAY-62..65, LAY-67..74, LAY-76, S-113, TXT-35, W-80
- **Manual page refs (printed page numbers):** 206-218, 334, 354, 390, 436, 533, 601, 653, +5 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: cad2 (they edit cad2: plan-core cad.rs and clip.rs, tools/cad/**, tools/select.rs, editor/edit_commands.rs, dialogs/multiple_copy.rs, dxf_options.rs, drawing groups).
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/layers.rs`
  - `crates/plan-core/src/layer_sets.rs`
  - `crates/plan-core/src/drawing_group.rs`
  - `crates/plan-app/src/dialogs/layer_display.rs`
  - `crates/plan-app/src/dialogs/layer_sets.rs`
  - `crates/plan-app/src/dialogs/drawing_groups.rs`
  - `crates/plan-app/src/dialogs/send_to_layer.rs`
  - `crates/plan-app/src/dialogs/select_layer.rs`
  - `crates/plan-app/src/scenarios/s68_layers.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/editor/render.rs (draw order hook)
  - crates/plan-layout/src/render.rs (sort hook; owner is brief 02)
  - crates/plan-app/src/tools/painters.rs (Layer Painter uses Select Layer)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: merge moves objects and defaults, delete refuses system or used layers, layer-set initial selection per view kind, drawing-group ordering, 'No Change' semantics; scenario s68_layers.rs: create a custom layer, move walls to it, hide it in one set only, merge it into another, undo restores objects and defaults in one step each, drawing-group send-to-back changes the draw order in plan and layout.
- **Docs to update:** docs/parity/dimensions-text-cad.md (LAY-3..LAY-6, LAY-11, LAY-36, LAY-62..LAY-76), docs/parity/select-and-edit.md (S-113)

### Brief 10 edit-behaviours-keys

**Edit behaviors and modifier keys the way Chief defines them, Enter Coordinates, arithmetic in number boxes**

- **Goal:** Swap Alt/Ctrl roles, real Alternate/Move/Resize/Concentric/Fillet behaviors with summon keys, Enter Coordinates dialog, + - * / in number fields.
- **Master gaps covered:** 5 Modifier keys as Chief defines them (M); 12 The six edit behaviors (M); 19 Arithmetic in every number box (S); 34 Enter Coordinates dialog on Tab/Enter during drawing and moving; move/rotate/resize/ref... (M); 148 Allowed angles and snap grid settings (M); 102 Snap aids (M)
- **Parity ids:** APP-84, CAD-3, CAD-4, CAD-68, DS-13, DS-47, DS-48, LAY-76, PR-3, PR-12, S-14, S-16, S-22, S-28, S-48, S-65..67, S-71, S-74, S-92, S-94, S-100, S-105, S-121..132, S-135, S-142, S-146, S-156, S-157, S-159..167, S-169, S-176, TXT-40, W-15, W-16, W-34, W-36, W-37, W-45
- **Manual page refs (printed page numbers):** 20, 122, 149, 188-199, 236-258, 288, 329-331, 389, +1 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: walls2, cad2 (they edit walls2: plan-core walls.rs, wall_spec.rs, joins.rs, tools/wall.rs, dialogs/wall.rs and wall/tabs.rs, editor/wall_edit.rs, connect.rs, plan-3d wall.rs; cad2: plan-core cad.rs and clip.rs, tools/cad/**, tools/select.rs, editor/edit_commands.rs, dialogs/multiple_copy.rs, dxf_options.rs, drawing groups).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-app/src/editor/behaviors.rs`
  - `crates/plan-app/src/editor/snap.rs`
  - `crates/plan-app/src/editor/typed_input.rs`
  - `crates/plan-app/src/dialogs/edit_behaviors.rs`
  - `crates/plan-app/src/dialogs/snap_settings.rs`
  - `crates/plan-app/src/dialogs/enter_coordinates.rs`
  - `crates/plan-core/src/calc.rs`
  - `crates/plan-app/src/scenarios/s69_edit_behaviours.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/units.rs (parse_length hook; owner is brief 06)
  - crates/plan-app/src/tools/select.rs (owner is brief 11)
  - crates/plan-app/src/tools/wall.rs (owner is brief 14)
  - crates/plan-app/src/tools/cad.rs (owner is brief 11)
  - crates/plan-app/src/tools/stairs.rs (owner is brief 21)
  - crates/plan-app/src/tools/cabinet.rs (owner is brief 23)
  - crates/plan-app/src/main.rs (input routing)
  - crates/plan-app/src/scenarios/s12_hotkeys.rs, s14_wall_edit_and_snaps.rs, s30_select_r14.rs (expectation updates only)
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: expression evaluation (precedence, units, fractions, errors), allowed-angle sets with opposing angles, behavior state machine; scenarios s69_edit_behaviours.rs for each behavior on a wall, a polyline and a box (Default vs Alternate vs Concentric outcomes asserted numerically), Ctrl override of a snap, Alt-drag no longer starting a marquee, Enter Coordinates drawing a wall by polar entry; update s12/s14/s30 expectations that pinned the old Alt behavior in the same commit.
- **Docs to update:** docs/parity/select-and-edit.md (S-14..S-16, S-22, S-28, S-65..S-74, S-100, S-121..S-131, S-161..S-169, S-195), docs/parity/preferences-hotkeys-toolbars.md (APP-84, APP-145, PR-12, DS-48), docs/manual/13-hotkeys.md

### Brief 11 cad-edit-tools-area

**CAD edit tools, Edit Area, Center/Align/Distribute, copy and paste modes, Delete Objects scopes**

- **Goal:** The Intersect/Join/Close/Complete Break/Fillet All family, Edit Area all floors, align and distribute dialogs, paste modes, locked-layer rules.
- **Master gaps covered:** 66 CAD line/arc/polyline edit tools (L); 135 Convert tools and polyline types (M); 69 Edit Area (M); 89 Copy/paste modes (M); 111 Center Object variants, Align/Distribute dialogs, Align To Line, Framing Reference plac... (M); 156 Make Parallel/Perpendicular with Rotate Edge/Polyline, Set Angular Dimension, Reflect A... (M); 118 Selection behaviors (M); 161 Edit handle sets and Edit-toolbar buttons per object family (M); 177 Delete Objects scopes (S); 146 Trim and Extend to CAD cutters or framing, Sticky Mode, drawn cutting lines (M); 179 Move rules (M)
- **Parity ids:** APP-73, APP-79, C-43, CAD-9, CAD-15, CAD-16, CAD-20, CAD-21, CAD-24, CAD-26, CAD-28, CAD-41, CAD-54, CAD-89..106, CAD-111, CB-46, CB-75, CB-516, CB-576, DIM-60, DS-1, DW-24, L-43, L-51, L-78, LAY-26, LAY-60, R-14, S-8, S-16..18, S-24, S-32, S-34, S-45, S-50, S-53, S-54, S-59, S-66, S-73, S-80..83, S-85, S-87, S-88, +14 more
- **Manual page refs (printed page numbers):** 14-25, 185, 197-203, 236-302, 313-317, 332-334, 343-344, 397-398, +12 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: cad2 (they edit cad2: plan-core cad.rs and clip.rs, tools/cad/**, tools/select.rs, editor/edit_commands.rs, dialogs/multiple_copy.rs, dxf_options.rs, drawing groups); preferably after Round 16 brief(s) 06 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/cad.rs`
  - `crates/plan-core/src/clip.rs`
  - `crates/plan-core/src/groups.rs`
  - `crates/plan-core/src/transform.rs`
  - `crates/plan-app/src/tools/select.rs`
  - `crates/plan-app/src/tools/select/area.rs`
  - `crates/plan-app/src/tools/select/describe.rs`
  - `crates/plan-app/src/tools/cad.rs`
  - `crates/plan-app/src/tools/cad_ops.rs`
  - `crates/plan-app/src/tools/cad/edit.rs`
  - `crates/plan-app/src/tools/cad/edit_tests.rs`
  - `crates/plan-app/src/tools/cad/style.rs`
  - `crates/plan-app/src/editor/edit_commands.rs`
  - `crates/plan-app/src/editor/ops.rs`
  - `crates/plan-app/src/editor/transform.rs`
  - `crates/plan-app/src/editor/clipboard.rs`
  - `crates/plan-app/src/editor/selection.rs`
  - `crates/plan-app/src/editor/handles.rs`
  - `crates/plan-app/src/dialogs/transform.rs`
  - `crates/plan-app/src/dialogs/delete_objects.rs`
  - `crates/plan-app/src/dialogs/multiple_copy.rs`
  - `crates/plan-app/src/dialogs/align_distribute.rs`
  - `crates/plan-app/src/scenarios/s70_cad_edit_tools.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/model.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/dispatch.rs
  - crates/plan-app/src/editor/render.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-core for each geometric op (join/trim cases, simplify tolerance, fillet-all radius, chamfer distances, arc-to-polyline side counts), Edit Area transforms, align/distribute math; scenario s70_cad_edit_tools.rs covering each tool through the Sim harness with one undo step per action, Edit Area across two floors moving walls and openings together, the locked-layer rule.
- **Docs to update:** docs/parity/select-and-edit.md and docs/parity/dimensions-text-cad.md rows above

### Brief 12 wall-types-layers

**Wall Type Definitions and Wall Layer Specification depth**

- **Goal:** Per-layer fill, role, material and alignment properties; multiple main layers; Dimension Layer; Delete Unused; library walls; Wall Layer Specification dialog.
- **Master gaps covered:** 4 Wall Type Definitions and Wall Layer Specification depth (L); 128 Wall Defaults dialogs per wall kind (M)
- **Parity ids:** C-104, CAD-35, CB-36, R-56, R-75, R-76, R-82, R-83, W-46, W-47, W-49, W-50, W-53, W-57, W-79, W-94, W-119, W-120, W-122, W-125, W-129, W-137, W-138
- **Manual page refs (printed page numbers):** 370-380, 389-419, 432, 737, 762, 1114, 1140; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: walls2 (they edit walls2: plan-core walls.rs, wall_spec.rs, joins.rs, tools/wall.rs, dialogs/wall.rs and wall/tabs.rs, editor/wall_edit.rs, connect.rs, plan-3d wall.rs).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-app/src/dialogs/wall_types.rs`
  - `crates/plan-app/src/dialogs/wall_layer.rs`
  - `crates/plan-app/src/dialogs/wall.rs`
  - `crates/plan-core/src/wall_types.rs`
  - `crates/plan-core/src/wall_spec.rs`
  - `crates/plan-core/src/wall_spec_tabs.rs`
  - `crates/plan-app/src/scenarios/s71_wall_types.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/defaults.rs (WallLayer/WallTypeDef fields; owner is brief 26)
  - crates/plan-app/src/dialogs/wall/tabs.rs (Wall Types and Foundation panels; owner is brief 18)
  - crates/plan-core/src/walls.rs (owner is brief 14)
  - crates/plan-3d/src/wall.rs (layer materials; owner is brief 14)
  - crates/plan-app/src/dialogs/default_pages/architectural.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: LayerTable operations with the new columns, multiple main layers and the resize rule, role validation, import/merge of wall types, migration of old types; scenario s71_wall_types.rs: define a Siding-6 variant with a furred layer, apply, see the plan fill and the Auto Detail layers follow, make a Room Divider and a Partition wall, library round trip, undo one step per dialog OK.
- **Docs to update:** docs/parity/walls.md (rows above), docs/manual/02-walls.md

### Brief 13 layered-assemblies

**Layered floor, ceiling and roof structure and finish definitions**

- **Goal:** Material Layers Definition model and dialogs for Floor/Ceiling Platform Defaults, finishes, dropped ceilings and roof surface/structure/ceiling layers.
- **Master gaps covered:** 6 Layered floor / ceiling / roof structure and finish definitions (L)
- **Parity ids:** CB-462..469, R-25, R-27..29, R-56, R-122..124, RF-36, W-46
- **Manual page refs (printed page numbers):** 460, 762-770, 882, 1087-1092; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: defaults_pages (they edit defaults_pages: dialogs/defaults.rs, default_lists.rs, default_settings_*.rs, default_pages/**, floor_defaults.rs, plan-core defaults.rs, shell/view_commands.rs).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/assemblies.rs`
  - `crates/plan-app/src/dialogs/assembly_def.rs`
  - `crates/plan-app/src/dialogs/floor_defaults.rs`
  - `crates/plan-3d/src/slab.rs`
  - `crates/plan-app/src/scenarios/s72_layered_assemblies.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/floors.rs (thickness slots; owner is brief 17)
  - crates/plan-core/src/rooms.rs (room finish slots; owner is brief 15)
  - crates/plan-core/src/defaults.rs (owner is brief 26)
  - crates/plan-roof/src/spec.rs (EaveOverrides; owner is brief 18)
  - crates/plan-docs/src/materials/** (category hook)
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/dialogs/room.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-core for layer totals, default chain resolution, migration of old thickness fields (round trip byte-stable), dropped-ceiling height math; plan-3d mesh tests for a two-layer floor; scenario s72_layered_assemblies.rs: define a 12 in floor of 3/4 OSB + 2x12, a hat-channel dropped ceiling and a tile-over-backer finish, apply to rooms, assert platform heights and the Materials List lines, undo one step per dialog OK.
- **Docs to update:** docs/parity/rooms-floors.md (R-25, R-27..R-29, R-56, R-122..R-124, R-143, R-144), docs/parity/roofs.md (RF-36)

### Brief 14 wall-edit-tools

**Wall connection repair and wall edit tools**

- **Goal:** Off-angle and unconnected caution icons with Fix dialogs, Edit Wall Intersections, Align With Wall Above/Below, Generate Between Platforms, stepped and double walls.
- **Master gaps covered:** 49 Wall connection repair (M); 72 Wall editing tools (M); 144 Generate Between Platforms short walls, railings across platform gaps, stairs snapping ... (M); 132 Stepped, raked and double walls (L); 137 Wall, railing, deck and fence tool set (M)
- **Parity ids:** CB-86, CB-98, CB-150, DIM-5, R-3, R-31, R-65, R-88, R-138, W-18, W-22..24, W-27, W-29, W-30, W-32, W-37, W-39, W-41, W-45, W-54..56, W-58..60, W-62, W-63, W-66..68, W-75, W-89, W-93, W-107, W-108, W-116, W-117, W-120, W-123, W-124, W-126, W-127, W-130..136, W-139, W-141..146, W-149
- **Manual page refs (printed page numbers):** 373-413, 422-424, 437-441, 450, 747, 767-771, 796; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: walls2 (they edit walls2: plan-core walls.rs, wall_spec.rs, joins.rs, tools/wall.rs, dialogs/wall.rs and wall/tabs.rs, editor/wall_edit.rs, connect.rs, plan-3d wall.rs).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/walls.rs`
  - `crates/plan-core/src/joins.rs`
  - `crates/plan-app/src/tools/wall.rs`
  - `crates/plan-app/src/editor/wall_edit.rs`
  - `crates/plan-app/src/editor/connect.rs`
  - `crates/plan-3d/src/wall.rs`
  - `crates/plan-3d/src/wall/arc.rs`
  - `crates/plan-3d/src/wall_kinds.rs`
  - `crates/plan-app/src/dialogs/fix_connections.rs`
  - `crates/plan-app/src/dialogs/edit_intersections.rs`
  - `crates/plan-app/src/dialogs/polygon_room.rs`
  - `crates/plan-app/src/scenarios/s73_wall_edit_tools.rs`
- **Shared files (targeted edits only):**
  - crates/plan-app/src/editor/tempdim.rs (owner is brief 28)
  - crates/plan-app/src/tools/select.rs (handle mapping; owner is brief 11)
  - crates/plan-app/src/editor/snap.rs (owner is brief 10)
  - crates/plan-core/src/model.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-core for off-angle detection thresholds, acute-junction geometry, alignment above/below, platform-gap wall generation, intersection reset; scenario s73_wall_edit_tools.rs: draw a wall 2 degrees off-angle and fix it through the dialog, leave a gap and use Connect Walls, align a wall to the floor below, generate platform walls for a two-height floor and assert they appear in 3D, one undo step per action.
- **Docs to update:** docs/parity/walls.md (rows above), docs/manual/02-walls.md

### Brief 15 rooms-living-area

**Room types and functions, Exterior Room, Living Area labels, room polylines and room commands**

- **Goal:** Room Type Defaults dialog, room functions, Exterior Room object, Living Area rules and polylines, Calculate Materials / Schedule / Elevation Views from a room.
- **Master gaps covered:** 67 Room Type Defaults dialog with the full Room Specification panels per type, Room Label defaults, room functions (M); 65 Exterior Room object and specification, Calculate Materials in Room, Create Schedule fr... (M); 42 Room labels and Living Area (M); 112 Room Specification panels (M)
- **Parity ids:** CB-13, CB-30, CB-36, DIM-27, DW-59, DW-115, DW-117, DW-155, L-60, L-74, L-83, R-4, R-5, R-15, R-16, R-18, R-19, R-21, R-30, R-32, R-36..38, R-40, R-42..44, R-46, R-47, R-49..51, R-56, R-98..101, R-103..109, R-114, R-115, S-107, TXT-17
- **Manual page refs (printed page numbers):** 443-474, 501, 537, 605, 709, 728, 740, 768, +5 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: decks_chimneys, defaults_pages (they edit decks_chimneys: tools/fireplace.rs, dialogs/fireplace.rs, plan-3d fireplace.rs and deck.rs, plan-core fireplace.rs, plan-framing deck module, rooms.rs deck fields; defaults_pages: dialogs/defaults.rs, default_lists.rs, default_settings_*.rs, default_pages/**, floor_defaults.rs, plan-core defaults.rs, shell/view_commands.rs).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/rooms.rs`
  - `crates/plan-app/src/dialogs/room.rs`
  - `crates/plan-app/src/dialogs/default_lists.rs`
  - `crates/plan-app/src/dialogs/room_types.rs`
  - `crates/plan-app/src/dialogs/exterior_room.rs`
  - `crates/plan-app/src/editor/rooms_edit.rs`
  - `crates/plan-app/src/scenarios/s74_rooms.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/defaults.rs (room type defaults; owner is brief 26)
  - crates/plan-check/src/minimums.rs (function hooks)
  - crates/plan-app/src/dialogs/default_pages/architectural.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: function property sets, living area rules at the 48 in boundary, per-structure area sums, standard-area rounding, polylines from room outlines; scenario s74_rooms.rs: add Court and Balcony rooms, an Open Below stairwell, an Exterior Room selection, living-area labels on a two-structure plan, Calculate Materials in Room and Create Schedule from Room outputs, undo one step per action.
- **Docs to update:** docs/parity/rooms-floors.md (rows above), docs/manual/04-rooms-floors.md

### Brief 16 tray-ceilings

**Tray and coffered ceilings, ceiling planes and cathedral behavior**

- **Goal:** Tray Ceiling Polyline tool and specification with nesting, rope lights and framing; cathedral and shelf ceilings.
- **Master gaps covered:** 95 Tray and coffered ceilings (L)
- **Parity ids:** R-32, R-111, R-112
- **Manual page refs (printed page numbers):** 457-458, 824, 917; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite; preferably after Round 16 brief(s) 15 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-roof/src/ceiling.rs`
  - `crates/plan-core/src/tray.rs`
  - `crates/plan-3d/src/tray.rs`
  - `crates/plan-app/src/tools/tray_ceiling.rs`
  - `crates/plan-app/src/dialogs/tray_ceiling.rs`
  - `crates/plan-app/src/scenarios/s75_tray_ceilings.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/rooms.rs (flat_ceiling flag; owner is brief 15)
  - crates/plan-framing/src/floor.rs (side members; owner is brief 30)
  - crates/plan-3d/src/lib.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/tools/mod.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: tray step geometry (nested, coffered grid of 3x2, sloped side), ceiling follow-roof height sampling, framing side members; plan-3d mesh test for a nested tray; scenario s75_tray_ceilings.rs: make a tray in a living room, nest one, add rope lights, switch the room to cathedral, see heights change, one undo step per action.
- **Docs to update:** docs/parity/rooms-floors.md (R-27, R-29, R-32, R-111, R-112, R-146)

### Brief 17 floors-foundation-fireplace

**Floor-level options, Foundation Defaults and build behavior, fireplace foundation**

- **Goal:** Build New Floor options, foundation options (rebar, foam, termite, garage), Auto Rebuild, S step markers, basement living area, fireplace base and facing rules.
- **Master gaps covered:** 105 Floor-level essentials (S); 85 Foundation Defaults and Options (M); 63 Foundation build behavior (M); 180 Slab Specification, Create Hole, piers and pads specification (S); 120 Fireplace foundation, dimensions in plan, rough opening, depth handle, chimney chase an... (M)
- **Parity ids:** CB-51, CB-60, CB-87, CB-232, CB-500, CB-501, CB-503, DW-125, L-39, L-61, R-18, R-26, R-31, R-34, R-55, R-59, R-61..63, R-68, R-85, R-116..121, R-125..137, R-140..142
- **Manual page refs (printed page numbers):** 737-773, 896; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: decks_chimneys (they edit decks_chimneys: tools/fireplace.rs, dialogs/fireplace.rs, plan-3d fireplace.rs and deck.rs, plan-core fireplace.rs, plan-framing deck module, rooms.rs deck fields).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/floors.rs`
  - `crates/plan-core/src/foundation.rs`
  - `crates/plan-core/src/split_level.rs`
  - `crates/plan-core/src/fireplace.rs`
  - `crates/plan-app/src/dialogs/floor.rs`
  - `crates/plan-app/src/dialogs/foundation.rs`
  - `crates/plan-app/src/dialogs/fireplace.rs`
  - `crates/plan-app/src/editor/foundation_view.rs`
  - `crates/plan-app/src/editor/fireplace_view.rs`
  - `crates/plan-app/src/editor/fireplace_view/deck.rs`
  - `crates/plan-app/src/tools/foundation.rs`
  - `crates/plan-app/src/tools/fireplace.rs`
  - `crates/plan-3d/src/foundation.rs`
  - `crates/plan-3d/src/split_level.rs`
  - `crates/plan-3d/src/fireplace.rs`
  - `crates/plan-app/src/scenarios/s76_floors_foundation.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/rooms.rs (owner is brief 15)
  - crates/plan-core/src/defaults.rs
  - crates/plan-app/src/dialogs/floor_defaults.rs (owner is brief 13)
  - crates/plan-framing/src/deck.rs (owner is brief 30)
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: stepped stem wall step detection and S marker positions, pier spacing, garage curb cut width from a door, stem wall height math, fireplace base footprint; scenario s76_floors_foundation.rs: new floor with step-elevation option, insert floor below, build a stepped foundation on a sloped test terrain, garage curb cutout under an overhead door, fireplace in an exterior wall faces in, one undo step per action.
- **Docs to update:** docs/parity/rooms-floors.md and docs/parity/cabinets-stairs-framing-terrain-library.md rows above, docs/manual/16-foundation-slabs.md

### Brief 18 roof-eaves-heights

**Roof eave alignment, plane height controls, plane specifications and the wall Roof panel**

- **Goal:** Same Roof Height at Exterior Walls / Same Height Eaves, ridge/baseline/fascia height locks, birdsmouth and heel fields, roof plane panels, Roof panel for multi-wall and interior-kind walls.
- **Master gaps covered:** 3 Roof eave alignment and plane heights (M); 119 Roof/Ceiling Plane and Skylight specifications (M); 103 Roof plane drawing and editing (M); 28 Wall roof directives (M)
- **Parity ids:** CB-37, R-68, RF-3, RF-4, RF-6, RF-18, RF-24, RF-26, RF-31, RF-35..38, RF-41, RF-59, RF-73..78, RF-80..82, RF-88..99, RF-101..121, RF-123, RF-124, W-147, W-148
- **Manual page refs (printed page numbers):** 407-409, 427-429, 822-849, 898; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: walls2 (they edit walls2: plan-core walls.rs, wall_spec.rs, joins.rs, tools/wall.rs, dialogs/wall.rs and wall/tabs.rs, editor/wall_edit.rs, connect.rs, plan-3d wall.rs); preferably after Round 16 brief(s) 12 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-roof/src/spec.rs`
  - `crates/plan-roof/src/geom.rs`
  - `crates/plan-roof/src/join.rs`
  - `crates/plan-roof/src/edges.rs`
  - `crates/plan-roof/src/lib.rs`
  - `crates/plan-roof/src/tests.rs`
  - `crates/plan-app/src/dialogs/roof.rs`
  - `crates/plan-app/src/dialogs/wall/tabs.rs`
  - `crates/plan-app/src/editor/roof_view.rs`
  - `crates/plan-app/src/tools/roof.rs`
  - `crates/plan-3d/src/roof.rs`
  - `crates/plan-app/src/scenarios/s77_roof_eaves.rs`
- **Shared files (targeted edits only):**
  - crates/plan-app/src/dialogs/wall.rs (owner is brief 12)
  - crates/plan-core/src/walls.rs (directive fields; owner is brief 14)
  - crates/plan-app/src/tools/select.rs (handle mapping; owner is brief 11)
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-roof for eave alignment (two wings with different pitches meet at one eave line), pivot-lock math for each lock, birdsmouth depth/seat, projected vs actual lengths; scenario s77_roof_eaves.rs: mixed-pitch roof with Same Height Eaves on/off, lock the ridge and change pitch, multi-wall directive entry building a gambrel, panel edits undone in one step.
- **Docs to update:** docs/parity/roofs.md (rows above), docs/manual/08-roofs.md

### Brief 19 roof-baselines-groups-curved

**Roof Baseline Polylines, Roof Groups, Build Roof retain switches and curved roof planes**

- **Goal:** Roofs for additions and remodels: baseline polylines with specification, roof groups from rooms, retain-manual/edited switches, curved and barrel planes.
- **Master gaps covered:** 76 Build Roof options (M); 81 Roof Baseline Polylines with specification, directive letters on edges, Use Existing Ro... (L); 163 Curved roof planes (L)
- **Parity ids:** R-114, RF-9, RF-61, RF-62, RF-66..72, RF-125..127
- **Manual page refs (printed page numbers):** 822-829, 838, 847-857; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite; preferably after Round 16 brief(s) 15, 18 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-roof/src/staged.rs`
  - `crates/plan-roof/src/footprint.rs`
  - `crates/plan-roof/src/skeleton.rs`
  - `crates/plan-roof/src/halfhip.rs`
  - `crates/plan-roof/src/baseline.rs`
  - `crates/plan-roof/src/curved.rs`
  - `crates/plan-app/src/tools/roof_baseline.rs`
  - `crates/plan-app/src/dialogs/roof_baseline.rs`
  - `crates/plan-app/src/scenarios/s78_roof_baselines.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/rooms.rs (roof_group field; owner is brief 15)
  - crates/plan-app/src/dialogs/roof.rs (Build Roof switches; owner is brief 18)
  - crates/plan-app/src/tools/roof.rs (owner is brief 18)
  - crates/plan-3d/src/roof.rs (curved plane meshes; owner is brief 18)
  - crates/plan-roof/src/lib.rs (owner is brief 18: mod lines only)
  - crates/plan-app/src/tools/mod.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-roof for baseline-polyline roofing of an L-shaped addition over an existing gable, roof group separation of two buildings, retained edited plane after rebuild, barrel plane section radius and facet count; scenario s78_roof_baselines.rs: add a porch roof from a baseline polyline, group the garage separately, rebuild while retaining a manual plane, one undo step each.
- **Docs to update:** docs/parity/roofs.md (RF-6, RF-9, RF-37, RF-61, RF-62, RF-66..RF-72), docs/parity/rooms-floors.md (R-114)

### Brief 20 roof-trim-dormers-skylights

**Roof trim parts, gable lines, skylights and dormers**

- **Goal:** Profile-driven rafter tails, ridge caps, gutters, frieze and shadow boards; Gable Over Door/Window lines; skylight specification; dormer types and crickets.
- **Master gaps covered:** 83 Roof trim as profile-driven parts (L); 149 Gable/Roof Line object (S); 141 Skylight tool and specification (M); 75 Dormers: roof types (M)
- **Parity ids:** RF-15, RF-27, RF-42, RF-43, RF-45, RF-48..52, RF-54, RF-60, RF-79, RF-83..87, RF-122, RF-133..160
- **Manual page refs (printed page numbers):** 831-838, 848, 858-884, 988; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite; preferably after Round 16 brief(s) 31, 18 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-roof/src/dormer.rs`
  - `crates/plan-roof/src/hole.rs`
  - `crates/plan-roof/src/gable.rs`
  - `crates/plan-3d/src/eave.rs`
  - `crates/plan-3d/src/dormer.rs`
  - `crates/plan-app/src/dialogs/dormer.rs`
  - `crates/plan-app/src/dialogs/skylight.rs`
  - `crates/plan-app/src/dialogs/roof_trim.rs`
  - `crates/plan-app/src/scenarios/s79_roof_trim.rs`
- **Shared files (targeted edits only):**
  - crates/plan-roof/src/spec.rs (trim fields; owner is brief 18)
  - crates/plan-app/src/dialogs/roof.rs (owner is brief 18)
  - crates/plan-3d/src/lib.rs
  - crates/plan-3d/src/molding.rs (profile sweep; owner is brief 31)
  - crates/plan-app/src/tools/mod.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Plan-roof and plan-3d unit tests for each trim profile sweep along an eave and rake (lengths, miters at hips), gable line pitch intersection, skylight hole rim shapes, cricket plane geometry; scenario s79_roof_trim.rs: Build Roof with ridge caps and gutters, edit one eave's frieze, add a Gable Over Door, add a skylight, one undo step per action.
- **Docs to update:** docs/parity/roofs.md (rows above), docs/parity/doors-windows.md (DW-145), docs/manual/08-roofs.md

### Brief 21 stairs-engine

**Staircase engine, landings, specification read-outs and drawing behaviors**

- **Goal:** Sections and subsections, Complete Break, merging, landing options, Best Fit and Lock Top/Bottom, New Shaped Staircase dialog, Open Below stairwell, rail panels.
- **Master gaps covered:** 15 Staircase engine (L); 62 Stair drawing behavior (M); 48 Staircase Specification (M); 99 Landing and ramp specification (M); 91 Tread geometry (M); 61 Stair railings and construction recipes (L); 138 Stair plan display (S); 164 Stairwell as an Open Below room, Auto Stairwell, Max Tread Contraction for rooms under ... (S); 185 Stair library round trip (S)
- **Parity ids:** CB-24, CB-26..34, CB-58, CB-99..127, CB-129..149, CB-151..200, CB-450
- **Manual page refs (printed page numbers):** 772-821, 995, 1005-1015, 1053, 1068; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: stairs2 (they edit stairs2: plan-stairs/**, tools/stairs.rs, dialogs/stairs.rs, editor/stairs_view.rs, plan-3d railing.rs).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-stairs/src/layout.rs`
  - `crates/plan-stairs/src/landing.rs`
  - `crates/plan-stairs/src/plan.rs`
  - `crates/plan-stairs/src/railing.rs`
  - `crates/plan-stairs/src/model3d.rs`
  - `crates/plan-stairs/src/deck.rs`
  - `crates/plan-stairs/src/lib.rs`
  - `crates/plan-stairs/src/tests.rs`
  - `crates/plan-app/src/tools/stairs.rs`
  - `crates/plan-app/src/dialogs/stairs.rs`
  - `crates/plan-app/src/dialogs/new_shaped_stair.rs`
  - `crates/plan-app/src/editor/stairs_view.rs`
  - `crates/plan-3d/src/railing.rs`
  - `crates/plan-app/src/scenarios/s80_stairs_engine.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/model.rs
  - crates/plan-core/src/rooms.rs (Open Below stairwell; owner is brief 15)
  - crates/plan-check/src/rules_irc.rs (stair rule hooks; read the NKBA/code owner's notes)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-stairs unit tests: section/subsection graphs, merge rules, landing height/thickness auto-adjust, Best Fit for several rises, tread locking, walkline lengths, stringer geometry counts; scenario s80_stairs_engine.rs: build an L-stair with a landing and a width change, Make Best Fit, Lock Top, merge two sections, Auto Stairwell, assert the stair table and the 3D bounds, one undo step per action.
- **Docs to update:** docs/parity/cabinets-stairs-framing-terrain-library.md (CB rows above), docs/manual/07-stairs.md

### Brief 22 openings-mulled-bay

**Door and window defaults per type, automatic mulling, Mulled Unit and bay/box/bow windows**

- **Goal:** Per-type door/window defaults with Use Default, auto-mull with Minimum Separation, Mulled Unit dialog, window Levels, bay/box/bow units with roofs.
- **Master gaps covered:** 51 Door and window defaults per type (M); 22 Automatic mulling, Minimum Separation, Mulled Unit dialog and defaults, window Levels, ... (M); 32 Bay, box and bow windows as wall-section units (L); 116 Door and window plan/3D display rules (M); 133 Door and window placement and edit rules (M)
- **Parity ids:** CB-60, DW-1, DW-3, DW-5, DW-8, DW-13, DW-15, DW-16, DW-19, DW-26, DW-30, DW-32, DW-33, DW-36, DW-39, DW-40, DW-43, DW-46, DW-48, DW-51..53, DW-55, DW-57..61, DW-64, DW-74, DW-81..83, DW-89..92, DW-103, DW-122, DW-125..140, DW-142..146, DW-154..164, L-23, L-74, R-26, R-133, RF-29
- **Manual page refs (printed page numbers):** 572-642, 654, 731, 743-748, 822; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: opening_tabs (they edit opening_tabs: dialogs/opening.rs, plan-core openings/**, opening_symbol.rs, plan-3d windows.rs, doors.rs, casing.rs, opening.rs, tools/opening/**).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/openings.rs`
  - `crates/plan-core/src/openings/spec.rs`
  - `crates/plan-core/src/openings/spec/shape.rs`
  - `crates/plan-core/src/openings/spec/tabs.rs`
  - `crates/plan-core/src/opening_symbol.rs`
  - `crates/plan-app/src/dialogs/opening.rs`
  - `crates/plan-app/src/dialogs/opening/tabs.rs`
  - `crates/plan-app/src/dialogs/mulled_unit.rs`
  - `crates/plan-app/src/dialogs/bay_window.rs`
  - `crates/plan-app/src/tools/opening.rs`
  - `crates/plan-app/src/tools/opening/place.rs`
  - `crates/plan-app/src/editor/opening_view.rs`
  - `crates/plan-app/src/editor/opening_edit.rs`
  - `crates/plan-3d/src/windows.rs`
  - `crates/plan-3d/src/doors.rs`
  - `crates/plan-3d/src/casing.rs`
  - `crates/plan-3d/src/opening.rs`
  - `crates/plan-3d/src/opening/shape.rs`
  - `crates/plan-3d/src/opening/treatments.rs`
  - `crates/plan-3d/src/leaf.rs`
  - `crates/plan-app/src/scenarios/s81_openings_mulled_bay.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/defaults.rs (opening defaults; owner is brief 26)
  - crates/plan-app/src/dialogs/default_pages/architectural.rs
  - crates/plan-roof/src/gable.rs (Gable Over hook; owner is brief 20)
  - crates/plan-docs/src/schedule_kinds.rs (mulled unit rows; owner is brief 04)
  - crates/plan-core/src/model.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: casing-touch detection and shared-casing width, mulled unit hole bounds, level pick order, bay depth/angle geometry and roof plane count, dynamic-default propagation; plan-3d tests for a bow window mesh and a mulled pair; scenario s81_openings_mulled_bay.rs: drag two windows together and see auto-mull, change a window type default and see Use-Default windows follow, add a bay window with roof, explode it, undo one step each.
- **Docs to update:** docs/parity/doors-windows.md (rows above), docs/manual/03-doors-windows.md

### Brief 23 cabinet-runs-labels

**Cabinet runs: automatic fillers, merging, labels, module lines and General Cabinet Defaults**

- **Goal:** Automatic fillers and filler tools, merge within 3 in, B24/3DB24 label formats, module lines layer, plan display options, General Cabinet Defaults.
- **Master gaps covered:** 31 Cabinet runs (M); 50 Cabinet labels (M); 129 General Cabinet Defaults (M)
- **Parity ids:** CB-1..6, CB-8, CB-10, CB-13..19, CB-21, CB-60, CB-71, CB-83, CB-476, CB-478, CB-480..483, CB-485, CB-487, CB-491, L-57
- **Manual page refs (printed page numbers):** 643-655, 667; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite.
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-cabinets/src/filler.rs`
  - `crates/plan-cabinets/src/top.rs`
  - `crates/plan-cabinets/src/geom.rs`
  - `crates/plan-cabinets/src/push.rs`
  - `crates/plan-cabinets/src/lib.rs`
  - `crates/plan-app/src/tools/cabinet.rs`
  - `crates/plan-app/src/dialogs/cabinet_defaults.rs`
  - `crates/plan-app/src/scenarios/s82_cabinet_runs.rs`
- **Shared files (targeted edits only):**
  - crates/plan-cabinets/src/symbol.rs (label codes; owner is brief 24)
  - crates/plan-app/src/dialogs/cabinet.rs (owner is brief 24)
  - crates/plan-docs/src/schedule_kinds.rs (owner is brief 04)
  - crates/plan-core/src/defaults.rs (owner is brief 26)
  - crates/plan-app/src/editor/snap.rs (owner is brief 10)
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-cabinets unit tests: filler insertion and widths (gaps of 2, 3, 6 in), merge detection within 3 in at angles, label code generation table (20 cases), minimum size, run extents; scenario s82_cabinet_runs.rs: a U-kitchen drawn with gaps, fillers appear and labels read the contractor codes, module lines layer toggle, schedule categories count, one undo step per action.
- **Docs to update:** docs/parity/cabinets-stairs-framing-terrain-library.md (rows above), docs/manual/06-cabinets-library.md

### Brief 24 cabinet-faces-specials

**Cabinet face items, special cabinet shapes, countertops and Cabinet Specification panels**

- **Goal:** 18 face item types with per-item dialogs, shelves, end/radius/angled/bow/peninsula cabinets, Custom Countertop Specification, waterfalls, backsplash.
- **Master gaps covered:** 54 Cabinet face items (M); 77 Special cabinets (M); 55 Custom Countertop Specification, waterfall add/remove, backsplash, molding polylines; countertops as cabinet components (M); 113 Cabinet Specification panels (M); 181 Cabinet editing (S)
- **Parity ids:** CB-8, CB-10..12, CB-14, CB-70, CB-478, CB-479, CB-481, CB-484, CB-486, CB-487, CB-490..498
- **Manual page refs (printed page numbers):** 648-689; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite; preferably after Round 16 brief(s) 23 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-cabinets/src/face.rs`
  - `crates/plan-cabinets/src/dress.rs`
  - `crates/plan-cabinets/src/cabinet.rs`
  - `crates/plan-cabinets/src/mesh3d.rs`
  - `crates/plan-cabinets/src/mesh_extra.rs`
  - `crates/plan-cabinets/src/symbol.rs`
  - `crates/plan-cabinets/src/extras_tests.rs`
  - `crates/plan-app/src/dialogs/cabinet.rs`
  - `crates/plan-app/src/dialogs/cabinet_face.rs`
  - `crates/plan-app/src/dialogs/cabinet_shelf.rs`
  - `crates/plan-app/src/dialogs/custom_countertop.rs`
  - `crates/plan-app/src/scenarios/s83_cabinet_faces.rs`
- **Shared files (targeted edits only):**
  - crates/plan-cabinets/src/top.rs (waterfall hooks; owner is brief 23)
  - crates/plan-cabinets/src/lib.rs (owner is brief 23: mod lines)
  - crates/plan-app/src/tools/cabinet.rs (owner is brief 23)
  - crates/plan-app/src/shell/spec_dialogs.rs (multi-open; owner is brief 05)
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-cabinets unit tests: face-item layout solver for each item combination, bow/angled/radius geometry and mesh bounds, blind-corner depth, waterfall end panel geometry; scenario s83_cabinet_faces.rs: build a kitchen island with a bow-front end and a waterfall top, give it false drawers and rollouts, open three cabinets together with No Change, undo one step per OK.
- **Docs to update:** docs/parity/cabinets-stairs-framing-terrain-library.md (rows above), docs/manual/06-cabinets-library.md

### Brief 25 electrical-defaults-connections

**Electrical Defaults, outlet tools, connection splines and rope lights**

- **Goal:** Electrical Defaults dialog with library objects and four height groups, three outlet tools, editable connection splines, rope lights as paths.
- **Master gaps covered:** 57 Electrical Defaults (M); 110 Electrical connections as editable splines (M); 126 Electrical tool behaviors (M)
- **Parity ids:** CB-60, CB-61, CB-66, DW-63, E-1, E-3, E-5..12, E-15, E-18..22, E-24, E-26..29
- **Manual page refs (printed page numbers):** 447, 648, 692-707; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: defaults_pages (they edit defaults_pages: dialogs/defaults.rs, default_lists.rs, default_settings_*.rs, default_pages/**, floor_defaults.rs, plan-core defaults.rs, shell/view_commands.rs).
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-electrical/src/circuit.rs`
  - `crates/plan-electrical/src/defaults.rs`
  - `crates/plan-electrical/src/device.rs`
  - `crates/plan-electrical/src/layer.rs`
  - `crates/plan-electrical/src/lib.rs`
  - `crates/plan-electrical/src/mesh3d.rs`
  - `crates/plan-electrical/src/place.rs`
  - `crates/plan-electrical/src/symbol.rs`
  - `crates/plan-electrical/src/tests.rs`
  - `crates/plan-app/src/tools/electrical.rs`
  - `crates/plan-app/src/dialogs/electrical.rs`
  - `crates/plan-app/src/dialogs/default_pages/electrical.rs`
  - `crates/plan-app/src/dialogs/rope_light.rs`
  - `crates/plan-app/src/scenarios/s84_electrical.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/model.rs (electrical slots)
  - crates/plan-core/src/layers.rs (Electrical Connection layer; owner is brief 09)
  - crates/plan-docs/src/schedule_kinds.rs (electrical schedule; owner is brief 04)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-electrical unit tests: height group resolution above base cabinets, weatherproof decision by wall side, spline curvature and reset, circuit numbering with 3-way and 4-way switches, rope light point spacing; scenario s84_electrical.rs: draw outlets inside and outside, edit defaults and see Use-Default devices follow, connect a switch to two lights with a curved spline, add a rope light under a wall cabinet, one undo step per action.
- **Docs to update:** docs/parity/electrical.md (E rows above), docs/manual/09-electrical-terrain.md

### Brief 26 saved-defaults-views

**Dynamic and saved defaults, Default Sets, Saved Plan Views, templates and Import Settings**

- **Goal:** 'Use Default' plus Set as Default everywhere, Saved Defaults and Default Sets, Saved Plan View specification, template chooser, Import Settings, typed plan rotation.
- **Master gaps covered:** 41 Dynamic defaults (M); 21 Saved Defaults for annotation tools (M); 36 Default Sets (M); 16 Saved Plan Views (L); 131 Typed Rotate Plan View angle saved with the view, CAD detail/footprint inheriting it, R... (S); 58 Import Settings from a plan or layout, plus .layers, .cadefs, wall-definition .dat and ... (M); 74 Templates (M)
- **Parity ids:** APP-1, APP-2, APP-13, APP-24, APP-60, APP-106, APP-122, APP-126, CAD-67, CB-20, CB-36, CB-475, DIM-4, DIM-6, DIM-40, DIM-51, DIM-53, DIM-54, DS-4, DS-6, DS-18, DS-22..24, DS-26..40, DW-124, DW-154, E-19, L-2, L-5, L-7, L-10, L-24, L-25, L-46, L-140, L-151, L-190, LAY-1, LAY-2, LAY-8, LAY-17, LAY-46, LAY-47, LAY-49..54, LAY-56, LAY-57, RF-25, +6 more
- **Manual page refs (printed page numbers):** 48-49, 62, 71, 102-118, 133, 159, 175-181, 265, +15 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: defaults_pages (they edit defaults_pages: dialogs/defaults.rs, default_lists.rs, default_settings_*.rs, default_pages/**, floor_defaults.rs, plan-core defaults.rs, shell/view_commands.rs).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/defaults.rs`
  - `crates/plan-app/src/dialogs/defaults.rs`
  - `crates/plan-app/src/dialogs/plan_views.rs`
  - `crates/plan-app/src/dialogs/saved_defaults.rs`
  - `crates/plan-app/src/dialogs/default_sets.rs`
  - `crates/plan-app/src/dialogs/import_settings.rs`
  - `crates/plan-app/src/dialogs/template_chooser.rs`
  - `crates/plan-app/src/plan_defaults.rs`
  - `crates/plan-app/src/templates.rs`
  - `crates/plan-app/src/shell/view_commands.rs`
  - `crates/plan-app/src/scenarios/s85_saved_defaults_views.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/model.rs (views slots)
  - crates/plan-app/src/dialogs/default_pages/mod.rs and page.rs (page registration)
  - crates/plan-app/src/editor/edit_commands.rs (Set as Default button; owner is brief 11)
  - crates/plan-app/src/editor/render.rs (rotation hook)
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/main.rs
  - crates/plan-app/src/shell/docks.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: use_default propagation, saved default CRUD with the in-use rule, default set switching restores every member, saved view round trip with rotation, template purge categories, import categories applied once; scenario s85_saved_defaults_views.rs: import a synthetic template with Daniel's inventory shape (many layer sets and default sets), switch the active default set and see new dimensions pick up the 1/8 in set, change a door default and watch a Use-Default door follow while an overridden one does not, saved view with rotation restores on reopen, Reverse Plan twice is the identity, undo one step each.
- **Docs to update:** docs/parity/preferences-hotkeys-toolbars.md (DS rows above), docs/parity/dimensions-text-cad.md (LAY-2, LAY-8, LAY-17, LAY-46..LAY-57)

### Brief 27 text-macros-rescheck

**Text tooling (find/replace, macros, Project Information) and Thermal Envelope / REScheck export**

- **Goal:** Find/Replace scopes and options, Insert Macro with object macros, Project Information as macros, text anchor rule, REScheck .rxl and thermal envelope CSV.
- **Master gaps covered:** 162 Find/Replace Text scopes and options, Replace Fonts prompt (S); 68 Text macros (M); 115 Rich Text editing odds and ends (M); 192 Text Style Defaults and the Text Style panel in object dialogs (S); 24 Thermal Envelope Data (CSV) and REScheck (.rxl) export (M)
- **Parity ids:** CB-60, L-52, L-54, L-62, L-83, TXT-1, TXT-2, TXT-4, TXT-10, TXT-12, TXT-13, TXT-16..21, TXT-23..33, TXT-35..38, TXT-41..46, TXT-57..64
- **Manual page refs (printed page numbers):** 521-544, 559-570, 710, 1304-1306, 1326, 1446-1457; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: callouts, painters_spell (they edit callouts: tools/text.rs, dialogs/text/**, plan-core text/callout/note types, plan-layout render.rs hooks; painters_spell: tools/painters.rs, dialogs/painters.rs, spell.rs, dialogs/spell_check.rs).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-app/src/dialogs/find_replace.rs`
  - `crates/plan-app/src/dialogs/project_info.rs`
  - `crates/plan-app/src/dialogs/text.rs`
  - `crates/plan-app/src/dialogs/text/annot.rs`
  - `crates/plan-app/src/dialogs/text/callout.rs`
  - `crates/plan-app/src/dialogs/text/defaults.rs`
  - `crates/plan-app/src/dialogs/text/editbar.rs`
  - `crates/plan-app/src/dialogs/text/manage.rs`
  - `crates/plan-app/src/dialogs/text/marker.rs`
  - `crates/plan-app/src/dialogs/text/note.rs`
  - `crates/plan-app/src/tools/text.rs`
  - `crates/plan-app/src/dialogs/macro_manager.rs`
  - `crates/plan-core/src/find_text.rs`
  - `crates/plan-core/src/text_styles.rs`
  - `crates/plan-core/src/text_box.rs`
  - `crates/plan-core/src/macros.rs`
  - `crates/plan-docs/src/rescheck.rs`
  - `crates/plan-app/src/dialogs/rescheck.rs`
  - `crates/plan-app/src/scenarios/s86_text_macros_rescheck.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/callout.rs and note.rs (callouts r15 types; add fields only)
  - crates/plan-layout/src/titleblock.rs (macro evaluation hook; owner is brief 01)
  - crates/plan-app/src/spell.rs (read only)
  - crates/plan-core/src/model.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-docs/src/lib.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: macro expansion and nesting (object-referencing macros resolve against the pointed-at object), find/replace over a synthetic plan with all scopes and options, thermal envelope grouping and orientation buckets, rxl XML well-formedness against a golden file generated in the test; scenario s86_text_macros_rescheck.rs: replace a word across plan text and layout text in one undo step, a room label macro, a title block filled from Project Information, an exported .csv and .rxl for a two-story plan whose totals match the walls and windows.
- **Docs to update:** docs/parity/dimensions-text-cad.md (TXT rows above), docs/parity/documentation-layout.md (L-47, L-52, L-83), docs/manual/05-dimensions-text-cad.md and 18-plan-check.md (energy export section)

### Brief 28 dimension-segments

**A dimension line as one object: Segments panel, extension lines, label handles, auto dimension refresh**

- **Goal:** Dimension string as one object with Segments panel and extension-line editing, label/leader handles, auto dimension refresh, rounding indicators, Delete Dimensions.
- **Master gaps covered:** 11 A dimension line as one object (L); 26 Auto dimensions (L); 73 Dimension Defaults dialog structure (M)
- **Parity ids:** C-130, C-142, DIM-6..10, DIM-15, DIM-18, DIM-19, DIM-24, DIM-26..33, DIM-38, DIM-39, DIM-41, DIM-46..53, DIM-55..59, DIM-61..68, L-115, S-63
- **Manual page refs (printed page numbers):** 267, 325, 396, 452, 476-520, 556, 579, 700, +3 more; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: dims2 (they edit dims2: plan-core dimension.rs, tools/dimension.rs, editor/dim_assoc.rs, dialogs/dimension.rs); preferably after Round 16 brief(s) 10 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/dimension.rs`
  - `crates/plan-core/src/dim_assoc.rs`
  - `crates/plan-app/src/tools/dimension.rs`
  - `crates/plan-app/src/dialogs/dimension.rs`
  - `crates/plan-app/src/dialogs/dimension_segments.rs`
  - `crates/plan-app/src/editor/dim_assoc.rs`
  - `crates/plan-app/src/editor/tempdim.rs`
  - `crates/plan-app/src/dialogs/default_pages/dimension.rs`
  - `crates/plan-app/src/scenarios/s87_dimension_segments.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/defaults.rs (owner is brief 26)
  - crates/plan-app/src/editor/render.rs (dimension drawing)
  - crates/plan-layout/src/render.rs (paper; owner is brief 02)
  - crates/plan-docs/src/pdf/sheet.rs (owner is brief 03)
  - crates/plan-app/src/tools/select.rs (owner is brief 11)
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests: string-as-one-object migration (old strings load into one object, segments equal), segment text overrides, extension-line insertion preserves segment values, rounding indicator logic, auto-refresh idempotence; scenario s87_dimension_segments.rs: place an exterior string, add leading text to one segment, add and delete an extension line, move a wall by editing a dimension value and watch associated dimensions follow, undo one step each.
- **Docs to update:** docs/parity/dimensions-text-cad.md (DIM rows above), docs/manual/05-dimensions-text-cad.md

### Brief 29 framing-members-reporting

**Framing Member Defaults, Framing Types, Automatic Framing Defaults panels and Structural Member Reporting**

- **Goal:** Framing member/type management with roles, Automatic Framing Defaults panels (Chief's real Build Framing structure), Buy/Cut/Linear/Mixed reporting.
- **Master gaps covered:** 70 Framing Member Defaults and Framing Types management (L); 78 Structural Member Reporting (L)
- **Parity ids:** CB-35, CB-38, CB-41, CB-202..218, CB-246..253, CB-287
- **Manual page refs (printed page numbers):** 881-889, 901-908, 932; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: decks_chimneys (they edit decks_chimneys: tools/fireplace.rs, dialogs/fireplace.rs, plan-3d fireplace.rs and deck.rs, plan-core fireplace.rs, plan-framing deck module, rooms.rs deck fields); preferably after Round 16 brief(s) 30 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-framing/src/defaults.rs`
  - `crates/plan-framing/src/member.rs`
  - `crates/plan-framing/src/lumber.rs`
  - `crates/plan-framing/src/takeoff.rs`
  - `crates/plan-framing/src/span.rs`
  - `crates/plan-framing/src/lib.rs`
  - `crates/plan-app/src/dialogs/framing_defaults.rs`
  - `crates/plan-app/src/dialogs/framing_types.rs`
  - `crates/plan-app/src/dialogs/member_reporting.rs`
  - `crates/plan-app/src/scenarios/s88_framing_members.rs`
- **Shared files (targeted edits only):**
  - crates/plan-app/src/dialogs/framing.rs (Build Framing dialog; owner is brief 30)
  - crates/plan-core/src/defaults.rs (owner is brief 26)
  - crates/plan-docs/src/materials/** (framing categories)
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-framing unit tests: reporting for a known floor (board counts under Buy List vs Cut List vs Mixed), type/role resolution, header size by opening width, default member application keeps size; scenario s88_framing_members.rs: edit a Framing Type from lumber to I-joist and see floor members change shape and the takeoff, apply a default member to selection, reporting dialog totals equal the takeoff.
- **Docs to update:** docs/parity/cabinets-stairs-framing-terrain-library.md (CB-36..CB-41, CB-256, CB-261, CB-638..CB-645), docs/manual/19-framing.md

### Brief 30 framing-layout-trusses

**Build Framing semantics, framing detail options, Wall Detail window and truss layout for the manufacturer**

- **Goal:** Build for Selected/Parent, Build Once, framing groups and reference markers, Wall Detail window, lap/butt/blocking/rim options, TR-X truss labels and Truss Detail.
- **Master gaps covered:** 47 Build Framing semantics (L); 60 Framing detail options (M); 71 Framing display and editing (M); 46 Roof truss layout for the truss manufacturer (L); 147 Truss Specification panels, member sizing and framing schedule (M)
- **Parity ids:** CB-35, CB-36, CB-38..41, CB-219..231, CB-233..245, CB-254..268, CB-270..286, CB-291..313, CB-328..331, RF-54, RF-161, W-134
- **Manual page refs (printed page numbers):** 388, 890-900, 909-943, 958-963; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: decks_chimneys (they edit decks_chimneys: tools/fireplace.rs, dialogs/fireplace.rs, plan-3d fireplace.rs and deck.rs, plan-core fireplace.rs, plan-framing deck module, rooms.rs deck fields).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-framing/src/build.rs`
  - `crates/plan-framing/src/layout.rs`
  - `crates/plan-framing/src/wall.rs`
  - `crates/plan-framing/src/floor.rs`
  - `crates/plan-framing/src/roof.rs`
  - `crates/plan-framing/src/manual.rs`
  - `crates/plan-framing/src/detail.rs`
  - `crates/plan-framing/src/deck.rs`
  - `crates/plan-framing/src/truss.rs`
  - `crates/plan-app/src/dialogs/framing.rs`
  - `crates/plan-app/src/dialogs/truss.rs`
  - `crates/plan-app/src/dialogs/truss_detail.rs`
  - `crates/plan-app/src/tools/framing.rs`
  - `crates/plan-app/src/editor/framing_view.rs`
  - `crates/plan-app/src/scenarios/s89_framing_layout.rs`
- **Shared files (targeted edits only):**
  - crates/plan-framing/src/lib.rs (owner is brief 29: mod lines)
  - crates/plan-core/src/rooms.rs (framing_group field; owner is brief 15)
  - crates/plan-core/src/walls.rs (bearing wall flag; owner is brief 14)
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/shell/docks.rs (Wall Details folder rows)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-framing unit tests: Build Once for one floor, Build for Selected for a wall vs roof plane, framing group separation, lap vs butt member counts, wall detail member list for a wall with a window, TR label grouping (three identical trusses share one label), truss schedule rows; scenario s89_framing_layout.rs: build floor framing for floor 2 only, edit a member then rebuild with Retain on and off, open a Wall Detail, truss labels on a gable roof, one undo step per action.
- **Docs to update:** docs/parity/cabinets-stairs-framing-terrain-library.md (rows above), docs/manual/19-framing.md

### Brief 31 moldings-trim

**Moldings system, molding polylines, Replace Moldings, corner boards and quoins**

- **Goal:** One Moldings/Profiles/Rails panel model with offsets and stacking, Molding Specification, molding polylines and 3D moldings along paths, corner boards and quoins field sets.
- **Master gaps covered:** 38 Moldings system (L); 159 Corner Board and Quoin field sets (S)
- **Parity ids:** CB-7, CB-57, CB-332..355, CB-357..363, R-21, R-34, R-84, R-113, RF-162, RF-163, W-109..111
- **Manual page refs (printed page numbers):** 462, 474, 671, 964-987; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite; preferably after Round 16 brief(s) 15 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-core/src/moldings.rs`
  - `crates/plan-core/src/extras.rs`
  - `crates/plan-core/src/details.rs`
  - `crates/plan-3d/src/molding.rs`
  - `crates/plan-3d/src/cover.rs`
  - `crates/plan-3d/src/details.rs`
  - `crates/plan-app/src/dialogs/molding.rs`
  - `crates/plan-app/src/dialogs/details.rs`
  - `crates/plan-app/src/tools/molding.rs`
  - `crates/plan-app/src/tools/details.rs`
  - `crates/plan-app/src/editor/details_view.rs`
  - `crates/plan-app/src/scenarios/s90_moldings.rs`
- **Shared files (targeted edits only):**
  - crates/plan-core/src/rooms.rs (MoldingDef; owner is brief 15)
  - crates/plan-cabinets/src/cabinet.rs (Molding type; owner is brief 24)
  - crates/plan-app/src/dialogs/room.rs, dialogs/cabinet.rs, dialogs/wall/tabs.rs (shared widget hook only)
  - crates/plan-3d/src/lib.rs
  - crates/plan-core/src/lib.rs
  - crates/plan-app/src/tools/mod.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** Unit tests in plan-core and plan-3d: profile sweep mesh along a rectangle with mitered corners and along an arc, stacked offsets, Reverse Direction, Repeat Distance placement, corner board / quoin placement counts; scenario s90_moldings.rs: define a crown profile from a polyline, apply to a room, convert to a Molding Polyline, edit one edge, replace from library, materials list length, undo one step each.
- **Docs to update:** docs/parity/rooms-floors.md and docs/parity/cabinets-stairs-framing-terrain-library.md molding rows, docs/manual/17-exterior-details.md

### Brief 32 camera-section-annotation

**Section and elevation clipping, annotations saved with the view, camera specification panels**

- **Goal:** Clip Sides/Elevation/Lines and stepped planes, text/CAD/dimensions on sections and elevations saved with the view, camera Plan Display/Layer/Selected Defaults/Below Grade panels, per-camera step sizes.
- **Master gaps covered:** 13 Scene clipping for sections (L); 7 Draw text, CAD lines and dimensions on a cross section, elevation or camera view and save them with the view (L); 59 Camera and section specification panels (M); 125 Camera navigation (S); 117 Orthographic Full/Floor/Framing and Isometric overviews, Auto Elevation tools one side ... (M)
- **Parity ids:** C-3..5, C-7, C-10, C-12, C-15, C-18..21, C-23, C-24, C-28, C-30, C-34..39, C-41, C-47, C-51, C-65, C-67, C-70, C-109, C-114, C-115, C-117, C-119, C-121..127, C-129..131, C-133..144, C-146..153, C-156..158, L-5, L-39
- **Manual page refs (printed page numbers):** 1146-1205; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** Round 15 builder(s) must have reported: cameras2 (they edit cameras2: plan-view3d/**, plan-render encoders and panorama, dialogs/camera.rs, tools/camera.rs, shell/view3d_panel/**); preferably after Round 16 brief(s) 28 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-view3d/src/**`
  - `crates/plan-elevation/src/dims.rs`
  - `crates/plan-elevation/src/drawing.rs`
  - `crates/plan-elevation/src/dxf.rs`
  - `crates/plan-elevation/src/hlr.rs`
  - `crates/plan-elevation/src/labels.rs`
  - `crates/plan-elevation/src/lib.rs`
  - `crates/plan-elevation/src/mlabels.rs`
  - `crates/plan-elevation/src/projection.rs`
  - `crates/plan-elevation/src/regions.rs`
  - `crates/plan-elevation/src/shadow.rs`
  - `crates/plan-elevation/src/styles.rs`
  - `crates/plan-elevation/src/view.rs`
  - `crates/plan-core/src/camera.rs`
  - `crates/plan-core/src/camera_view.rs`
  - `crates/plan-app/src/dialogs/camera.rs`
  - `crates/plan-app/src/dialogs/section_clip.rs`
  - `crates/plan-app/src/dialogs/cross_section_slider.rs`
  - `crates/plan-app/src/tools/camera.rs`
  - `crates/plan-app/src/shell/view3d_panel.rs`
  - `crates/plan-app/src/shell/view3d_panel/**`
  - `crates/plan-app/src/scenarios/s91_camera_sections.rs`
- **Shared files (targeted edits only):**
  - crates/plan-elevation/src/hatch.rs (poché; owner is brief 08)
  - crates/plan-app/src/tools/dimension.rs and text.rs (annotation hosting; owners are briefs 28 and 27)
  - crates/plan-layout/src/send.rs (annotated sections to layout; owner is brief 02)
  - crates/plan-core/src/model.rs
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-elevation and plan-view3d unit tests: clip volume math for sides/elevation/stepped planes, section-line cut marker positions, depth cue ramps, annotation persistence round trip with the saved view; scenario s91_camera_sections.rs: make a wall section with a stepped cut, clip to one bay, add a dimension and a note, save, close and reopen the view, send to layout and see the annotations, change a camera's incremental distance and step, undo one step each.
- **Docs to update:** docs/parity/3d-views-cameras.md (C rows above), docs/manual/10-3d-views-rendering.md

### Brief 33 terrain-site

**Terrain: Clear Terrain fix, Terrain Specification parity, retaining walls, contour presentation and site schedules**

- **Goal:** Fix Clear Terrain, Absolute Elevation and reference point, retaining walls from terrain breaks, terrain walls that follow the ground, contour labels and layers, terrain schedules.
- **Master gaps covered:** 25 Clear Terrain erases the perimeter, data and features instead of only the built surface (S); 30 Terrain Specification parity (M); 45 Retaining Wall tools (M); 64 Contour presentation and terrain labels (M); 140 Terrain elevation data tools and modifiers (M); 151 Terrain feature shapes and click-once placement (M)
- **Parity ids:** CB-43..51, CB-53, CB-60, CB-73..76, CB-78, CB-505..509, CB-511..515, CB-517..542, CB-544..568, CB-604, CB-608
- **Manual page refs (printed page numbers):** 1307-1341, 1358-1361; read with `python3 ~/plan-studio-dev/chief-docs/pages.py ref <first> <last>`
- **Starts when:** no Round 15 prerequisite.
- **Estimate:** 5 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-terrain/src/**`
  - `crates/plan-docs/src/terrain_report.rs`
  - `crates/plan-app/src/dialogs/terrain.rs`
  - `crates/plan-app/src/dialogs/terrain/**`
  - `crates/plan-app/src/dialogs/default_settings_terrain.rs`
  - `crates/plan-app/src/tools/terrain.rs`
  - `crates/plan-app/src/tools/terrain/**`
  - `crates/plan-app/src/editor/site_view.rs`
  - `crates/plan-app/src/editor/site_view/**`
  - `crates/plan-app/src/scenarios/s92_terrain_site.rs`
- **Shared files (targeted edits only):**
  - crates/plan-app/src/main.rs (TerrainCommand::Clear)
  - crates/plan-core/src/model.rs
  - crates/plan-docs/src/schedule_kinds.rs (terrain categories; owner is brief 04)
  - crates/plan-app/src/editor/handles.rs (terrain handles; owner is brief 11)
  - crates/plan-app/src/menus.rs
  - crates/plan-app/src/toolbar.rs
  - crates/plan-app/src/scenarios/mod.rs
- **Tests required:** plan-terrain unit tests: Clear Terrain keeps data, Absolute Elevation retain-at-reference math, retaining wall height from two-sided terrain, contour offset/interval labeling, negative highlight; scenario s92_terrain_site.rs: a sloped lot with a pad, Clear Terrain then rebuild, retaining wall along a break, labels visible on the plot plan, schedule counts, one undo step per action.
- **Docs to update:** docs/parity/cabinets-stairs-framing-terrain-library.md (terrain rows), docs/manual/09-electrical-terrain.md

### Brief 34 tutorial-replay-a

**Tutorial replays, lessons 1 to 14 (walls through cabinet styles) as headless scenarios**

- **Goal:** One scenario test per Chief tutorial lesson 1 to 14 with shared Chic Cottage support, breaks recorded as named ignored tests.
- **Master gaps covered:** none (QA). Source: `scenario-proposals.md` and part 7 of the audit.
- **Parity ids:** the break ids named in `scenario-proposals.md` (W-, R-, RF-, DW-, CB-, E-, L-, DIM-, TXT- series from part 7).
- **Manual page refs:** Tutorial Guide pages via `pages.py tut <first> <last>` (lessons 1-14: pp. 3-260; lessons 15-28: pp. 260-500).
- **Starts when:** no Round 15 prerequisite.
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief except that brief 35 waits for it.
- **Files owned:**
  - `crates/plan-app/src/scenarios/tutorials_support.rs`
  - `crates/plan-app/src/scenarios/tutorials_a/**`
- **Shared files (targeted edits only):**
  - crates/plan-app/src/scenarios/mod.rs (one mod line)
- **Tests required:** The deliverable is tests. Report: how many lessons run green, how many tests are ignored with which break ids, and any defects found. `cargo test -p plan-app scenarios::tutorials_a` must be green and `-- --ignored` must list the open work in lesson order.
- **Docs to update:** docs/chief-manual-coverage/scenario-proposals.md (append a 'Status' column note per lesson)

### Brief 35 tutorial-replay-b

**Tutorial replays, lessons 15 to 28 (cabinet layout through sending views to layout) as headless scenarios**

- **Goal:** One scenario test per Chief tutorial lesson 15 to 28 using the shared support file, breaks recorded as named ignored tests.
- **Master gaps covered:** none (QA). Source: `scenario-proposals.md` and part 7 of the audit.
- **Parity ids:** the break ids named in `scenario-proposals.md` (W-, R-, RF-, DW-, CB-, E-, L-, DIM-, TXT- series from part 7).
- **Manual page refs:** Tutorial Guide pages via `pages.py tut <first> <last>` (lessons 1-14: pp. 3-260; lessons 15-28: pp. 260-500).
- **Starts when:** no Round 15 prerequisite; preferably after Round 16 brief(s) 34 (they own files this brief edits in a targeted way; the order is a courtesy, not a block).
- **Estimate:** 4 hours. **Concurrency:** disjoint from every other brief.
- **Files owned:**
  - `crates/plan-app/src/scenarios/tutorials_b/**`
- **Shared files (targeted edits only):**
  - crates/plan-app/src/scenarios/tutorials_support.rs (read only; owner is brief 34)
  - crates/plan-app/src/scenarios/mod.rs (one mod line)
- **Tests required:** `cargo test -p plan-app scenarios::tutorials_b` green; `-- --ignored` lists open work in lesson order; report counts and defects as in brief 34.
- **Docs to update:** docs/chief-manual-coverage/scenario-proposals.md

## Ownership verification

Checked by script on 2026-10-08 against the working tree: 380 owned paths across 35 briefs, **0 overlaps** (a path owned twice, or a `dir/**` owned alongside a file or directory inside it). 261 owned paths exist today; 119 are new files or folders the briefs create (new dialogs, new plan-core modules, the s60 to s92 scenarios, the tutorial folders). Re-run before launching: the checker is `~/plan-studio-dev/briefs/r15/manual_audit/tools/check_ownership.py`.

## Round 17 candidates (ranked in the top 100 or left on the floor by a Round 16 stop line)

Ranked entries with no Round 16 brief, highest first. Items marked R15 are still landing from Round 15 and need a gate check before anyone plans a follow-up.

| Master rank | Gap | Size | Round 15 | Note |
|---|---|---|---|---|
| 9 | Callouts as detail and section references | L | callouts (in progress) | tools/text.rs |
| 37 | Wall drawing gestures | M | walls2 | tools/wall.rs |
| 52 | Materials List scopes, specification, columns, export formats and Master List | L | materials_list (in progress) | plan-docs materials/** |
| 53 | Material Painter scoping modes, Plan Materials dialog, Select Material dialog | L | lightbeans (import only) | plan-materials library.rs |
| 79 | Rough Opening and Framing panels | M | opening_tabs | plan-core openings/spec.rs |
| 84 | Import Drawing | L | dxf (in progress) | plan-import dxf/** |
| 86 | CAD Detail Specification, Plan Footprint object and specification, Automatic Truss/Wall... | M | cad2 (Plan Footprint) | tools/details/** |
| 87 | Door and window specification panels | M | opening_tabs (most tabs) | dialogs/opening/tabs.rs |
| 88 | Picture Box and PDF Box objects | L | none | tools/underlay/** |
| 90 | Deck rooms | M | decks_chimneys | plan-framing deck.rs |
| 92 | Import Terrain Data and Import GPS Data assistants | M | none | plan-terrain import.rs |
| 93 | Wall Specification panels | M | walls2 (most tabs) | dialogs/wall/tabs.rs |
| 94 | Notes linked to Note Schedules | M | callouts (in progress) | dialogs/text/note.rs |
| 96 | Symbol Specification depth | L | none | dialogs/symbol.rs |
| 97 | Define Material depth | M | lightbeans (PBR maps) | plan-materials material.rs |
| 98 | Architectural Blocks and Ganged Electrical Blocks | M | none | new plan-core blocks.rs |
| 106 | Library Browser search and organisation | M | library (in progress) | plan-library browse.rs |
| 107 | Lighting | M | cameras2 (light sets) | plan-render lighting.rs |
| 108 | Painter options | L | painters_spell (basic painters, Match Properties) | tools/painters.rs |
| 109 | Library placement | M | library (in progress) | plan-library |
| 114 | Marker types | M | callouts (in progress) | tools/text.rs |
| 121 | Text attached by arrows | M | callouts (Add Arrow handle) | dialogs/text |
| 122 | General Plan Defaults gaps | M | defaults_pages | plan-core defaults.rs |
| 123 | Import 3D Symbol dialog details | M | import3d (in progress) | dialogs/symbol/import3d.rs |
| 124 | Foundation wall footings | M | none | plan-core foundation.rs |
| 127 | Jack, hip, girder, drop-hip, subgirder, scissors, attic and energy-heel trusses; Truss ... | L | none | plan-framing truss.rs |
| 130 | Missing Files dialog | M | none | files.rs |
| 134 | Road geometry and specs | L | none | plan-terrain landscape.rs |
| 136 | Rendering Technique Options and Defaults dialogs | L | cameras2 (techniques) | plan-view3d |
| 139 | Sun Angle objects | M | none | plan-core camera.rs |

Stop-line leftovers expected from Round 16 (cut inside the briefs, queue them as Round 17 briefs): brief 02 plot-line editing and layout CAD parity; brief 11 copy/paste modes and selection extras; brief 13 roof layered definitions; brief 14 stepped/raked/double walls; brief 15 none; brief 17 fireplace chase and cap details; brief 20 dormers; brief 21 stair railing recipes; brief 22 bay/box/bow roofs and display rules; brief 24 special cabinet shapes; brief 25 rope lights; brief 26 templates and Import Settings; brief 27 energy export; brief 28 auto dimensions; brief 29 reporting; brief 30 truss layout; brief 31 molding polylines and corner trim; brief 32 camera panels and overviews; brief 33 terrain object specs.

## Decisions Daniel should make before launch

1. **Swapping Alt and Ctrl/Cmd** (brief 10) changes muscle memory the other way round from what has been built since Round 2. The manual is clear; confirm before the old tests are rewritten.
2. **Duplicate page labels** (brief 01) reverse DECISIONS 62; they are how `A0.#` numbering works in Chief.
3. **Defaults that differ** (see `defaults-that-differ.md`): whether new plans follow Chief's Residential Template behaviours (roof and exterior dimensions at room definition) or Daniel's working template; the briefs add switches defaulting to off.
4. **Wall Type Definitions** (brief 12) will change how the shipped template's 12 wall types load; confirm the template regeneration (`PlanDefaults::chief_x18_daniel().to_json()`) is acceptable.
5. **Program-wide sheet sizes** (brief 03) move storage out of the plan file.
