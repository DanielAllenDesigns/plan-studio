# Master gap list: every Missing and Partial finding from the seven manual audits, deduplicated and ranked

Written 2026-10-08 (Round 15 consolidation, docs only). Inputs: `part1` to `part7` and `scenario-proposals.md` in this folder, the earlier `docs/chief-feature-coverage.md` (Top 40), the appended sections of `docs/parity-status.md`, `DECISIONS.md` and the Round 15 briefs in `~/plan-studio-dev/briefs/r15`. Findings are in our own words; nothing of Chief's manual text is stored here (see README.md).

## How to read this list

- **Raw input:** 2589 feature rows from parts 1 to 6 whose status is Missing, Partial or In progress (Round 15), plus 698 Missing/Partial/In-progress step rows from the part 7 tutorial replays, plus the 194 ranked gaps the seven parts printed (25+35+33+25+24+22 for parts 1 to 6 and 30 workflow breaks for part 7) and the earlier audit's Top 40. Many of them describe the same feature from different pages of the manual (a Fill Style panel appears in more than 30 dialogs) or from the tutorial's point of view.
- **Deduplicated result:** 215 entries. Each row was assigned to exactly one entry by parity id first (cross-cutting ids such as R-89 Elevation Reference, DS-26 dynamic defaults, S-116 painters) and then by manual page range; a row that touches two topics is counted once, in the entry whose parity id it cites. All 2589 part 1 to 6 rows are assigned (the assignment check left none over). Part 7 added no new entry: of its 698 Missing/Partial/In-progress step rows, 498 cite parity ids that sit in existing entries, 173 matched by keyword and the last 27 were placed by hand (listed in the Part 7 section below).
- **Rank** is a priority score for Daniel's practice (custom homes and remodels in metro Atlanta, construction documents on layout sheets, kitchens and baths, site plans, code compliance) from 0 to 100, ties broken by the number of underlying rows. It is a judgment, not a formula; the score is printed so it can be argued with. Rows in his own Chief template (34 layer sets, 14 dimension default sets, 13 rich text default sets, 20 saved plan views, 108 wall types) raised the entries that must import and switch those things.
- **Size:** S under a day, M a few days, L a week or more of one builder (the same scale the parts use).
- **Rows** counts the Missing, Partial and In-progress feature rows folded into the entry (they are indicators of breadth, not effort). **Parts** lists which audit parts cited it; an entry cited by several parts is the cross-part deduplication at work.
- **Round 15 coverage** names the in-flight brief that already builds part of it (briefs are `~/plan-studio-dev/briefs/r15/*.md`). Those are not re-planned: the Round 16 brief starts after that builder reports and only takes what is left. **Round 16 brief** is the numbered brief in `round16-plan.md`; "Round 17 candidate" means ranked in the top 100 but not briefed in Round 16.
- Parity ids shown are the ids cited by the entry's rows (`docs/parity/*.md`); rows added by the audits carry the new ids.

## Round 16 coverage at a glance

Of the top 100 entries, 84 are taken by a Round 16 brief, 12 are Round 15 work still landing, and 4 are Round 17 candidates (listed in `round16-plan.md`).

## The top 100

### 1. Page Information on layout pages

- **Scope:** Page Information on layout pages: Label with # numbering (A-#, A0.#), Title, Description, Comments, Include in Layout Table, page macros (%page%, %numpages%, %layout.label%), Page Templates assigned per page, Page Revisions with Revised By and Date, Layout Revision Table, Layout Page Table
- **Size:** M | **Score:** 86 | **Rows:** 28 (6 Missing, 22 Partial) | **Parts:** 6
- **Parity ids:** L-1, L-5, L-7..9, L-11, L-15, L-16, L-54, L-138, L-139, L-141..144, L-146, L-185..189, L-191..196
- **Area / owner files:** plan-layout template.rs, titleblock.rs, model.rs, dialogs/layout.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 01 layout-pages
- **Why it matters for CDs:** Every CD set needs sheet numbering, an index and a revision table that follow page order (DECISIONS 35, 62).
- **Merged from:** source gaps P6#2, P6#3, P7#1

### 2. Survey-style entry

- **Scope:** Survey-style entry: bearings and azimuths in Input Line/Arc fields, New CAD Point/Line/Arc dialogs with Next, Current Point, Move Point, Show Length / Show Angle labels, plot plan by bearing
- **Size:** L | **Score:** 82 | **Rows:** 17 (10 Missing, 7 Partial) | **Parts:** 2, 6
- **Parity ids:** CAD-2, CAD-5, CAD-8, CAD-22, CAD-39, CAD-40, CAD-108, CAD-109, CAD-112..114, CAD-118, CAD-134, CB-44, CB-543, DIM-6, L-38
- **Area / owner files:** tools/cad/arcs.rs, new tools/cad/survey.rs, new dialogs/input_line.rs, move_point.rs, plan-core units/bearing
- **Round 15 coverage:** cad2 (CAD tools, partial)
- **Round 16 brief:** 06 survey-entry
- **Why it matters for CDs:** The lot cannot be entered from a survey today; every site plan and plot plan for a metro-Atlanta lot needs bearings and distances.
- **Merged from:** source gaps P2#2, P2#22, P7#2

### 3. Roof eave alignment and plane heights

- **Scope:** Roof eave alignment and plane heights: Same Roof Height at Exterior Walls, Same Height Eaves, Allow Low Roof Planes, pivot locks on Ridge Top/Baseline/Fascia Top, Heel Height, birdsmouth fields, Top of Plate read-out, Move to be Coplanar
- **Size:** M | **Score:** 80 | **Rows:** 23 (17 Missing, 6 Partial) | **Parts:** 4
- **Parity ids:** RF-41, RF-73..78, RF-80..82, RF-103, RF-104, RF-111..121
- **Area / owner files:** plan-roof spec.rs, geom.rs, join.rs, dialogs/roof.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 18 roof-eaves-heights
- **Why it matters for CDs:** Mixed-pitch roofs are the normal case on a custom home; eaves must meet and ridge heights must be controllable.
- **Merged from:** source gaps P4#1; earlier Top 40 #20

### 4. Wall Type Definitions and Wall Layer Specification depth

- **Scope:** Wall Type Definitions and Wall Layer Specification depth: per-layer fill/pattern/color/role/material, multiple main layers, Dimension Layer, foundation/platform alignment layers, Brick Ledge Depth, Delete All Unused, Partition and Room Divider types, library walls
- **Size:** L | **Score:** 78 | **Rows:** 16 (8 Missing, 8 Partial) | **Parts:** 2, 5
- **Parity ids:** C-104, CAD-35, CB-36, W-46, W-47, W-49, W-50, W-53, W-57, W-79, W-94, W-122, W-125, W-129, W-137, W-138
- **Area / owner files:** dialogs/wall_types.rs, new dialogs/wall_layer.rs, plan-core defaults.rs (WallLayer/WallTypeDef)
- **Round 15 coverage:** none
- **Round 16 brief:** 12 wall-types-layers
- **Why it matters for CDs:** Wall assemblies drive plan poché, dimensions, framing and Auto Detail on every set; today plan fills come from wall kind only.
- **Merged from:** source gaps P2#1, P2#23, P2#30, P2#31, P7#6

### 5. Modifier keys as Chief defines them

- **Scope:** Modifier keys as Chief defines them: Alt = Alternate behavior, Ctrl/Cmd = override snaps and restrictions, S suspends object snaps, Shift restricts to 90/45 and slows the pointer, 1 clears indicators
- **Size:** M | **Score:** 78 | **Rows:** 7 (3 Missing, 4 Partial) | **Parts:** 1
- **Parity ids:** S-22, S-74, S-100, S-121, S-123, S-124, S-127, S-130, S-131, S-142
- **Area / owner files:** editor/behaviors.rs, snap.rs, tools/select.rs, tools/wall.rs, tools/cad.rs, tools/stairs.rs, tools/cabinet.rs + scenario tests that pin the old roles
- **Round 15 coverage:** none
- **Round 16 brief:** 10 edit-behaviours-keys
- **Why it matters for CDs:** Muscle memory: Alt and Ctrl do the opposite of Chief today, so every drag feels wrong to a Chief user (DECISIONS 53, 70, 77).
- **Merged from:** source gaps P1#7

### 6. Layered floor / ceiling / roof structure and finish definitions

- **Scope:** Layered floor / ceiling / roof structure and finish definitions: Platform Defaults, Ceiling Structure, dropped ceilings, tile over backerboard, I-joist vs lumber; Material Layers Definition dialogs with Role, Fill, energy values and framing/purlin options
- **Size:** L | **Score:** 76 | **Rows:** 17 (5 Missing, 12 Partial) | **Parts:** 2, 4
- **Parity ids:** CB-462..469, R-25, R-27..29, R-56, R-122..124, RF-36, W-46
- **Area / owner files:** new plan-core assemblies.rs, dialogs/floor_defaults.rs, new dialogs/assembly_def.rs, plan-3d slab.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 13 layered-assemblies
- **Why it matters for CDs:** Wall and roof section details, Auto Detail and the energy submittal read from these assemblies; today a finish is a thickness and a name (DECISIONS 305).
- **Merged from:** source gaps P4#9, P7#7; earlier Top 40 #10

### 7. Draw text, CAD lines and dimensions on a cross section, elevation or camera view and save them with the view

- **Scope:** Draw text, CAD lines and dimensions on a cross section, elevation or camera view and save them with the view (Draw Mode buttons, Dimension Selected Edge, annotation layers)
- **Size:** L | **Score:** 76 | **Rows:** 7 (7 Missing, 0 Partial) | **Parts:** 5
- **Parity ids:** C-47, C-129, C-130, C-142..144
- **Area / owner files:** plan-elevation, tools/text.rs/dimension.rs hooks, shell/view3d_panel
- **Round 15 coverage:** none
- **Round 16 brief:** 32 camera-section-annotation
- **Why it matters for CDs:** Section and elevation sheets are annotated by hand; today the only route is CAD Detail from View, which stops updating.
- **Merged from:** source gaps P2#29, P5#1, P5#13, P7#11

### 8. Construction Lines

- **Scope:** Construction Lines (tool, infinite lines with callouts, ordering rule sets, specification, snap/dimension/center targets)
- **Size:** L | **Score:** 76 | **Rows:** 6 (6 Missing, 0 Partial) | **Parts:** 1, 2
- **Parity ids:** CAD-62..66
- **Area / owner files:** new plan-core construction.rs, tools/construction_line.rs, dialogs/construction_line.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 07 construction-lines-reference
- **Why it matters for CDs:** The standard way to hold gridlines, setbacks and floor-to-floor alignment, and section/elevation reference lines.
- **Merged from:** source gaps P1#1

### 9. Callouts as detail and section references

- **Scope:** Callouts as detail and section references: ten shapes, cross-section line, arrows, Callout Specification; linked callouts that report the target view name and layout sheet
- **Size:** L | **Score:** 74 | **Rows:** 18 (10 Missing, 8 Partial) | **Parts:** 3, 6
- **Parity ids:** CB-13, DW-62, L-26, L-73, L-150, TXT-7, TXT-49..51, TXT-56
- **Area / owner files:** tools/text.rs, dialogs/text/callout.rs, plan-core callout.rs
- **Round 15 coverage:** callouts (in progress)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Section and detail bubbles are how drawings point at each other and stay right when sheets are renumbered.
- **Merged from:** source gaps P3#2, P3#3, P7#17

### 10. Schedule table quality

- **Scope:** Schedule table quality: column and row totals, Sum Similar Rows, area columns, fractional-inch formats, minimum rows, wrapping onto the sheet, preview/picture columns, custom object fields, Swap Rows/Columns
- **Size:** L | **Score:** 74 | **Rows:** 11 (5 Missing, 6 Partial) | **Parts:** 3
- **Parity ids:** L-28, L-64, L-71, L-72
- **Area / owner files:** plan-docs schedule.rs, xlsx export, plan-layout render
- **Round 15 coverage:** none
- **Round 16 brief:** 04 schedules
- **Why it matters for CDs:** Per-column totals and wrapping decide whether a schedule fits a permit sheet.
- **Merged from:** source gaps P3#1, P7#14; earlier Top 40 #13

### 11. A dimension line as one object

- **Scope:** A dimension line as one object: Segments panel (leading/trailing text), add/move/resize/fixed-proximity extension lines, Mark as Centerline, label move/rotate handles and leader lines, Move Edge buttons, Dimension Line Specification panels, rounded-value indicators
- **Size:** L | **Score:** 72 | **Rows:** 25 (10 Missing, 15 Partial) | **Parts:** 1, 2, 6
- **Parity ids:** DIM-7..10, DIM-19, DIM-30..33, DIM-39, DIM-41, DIM-48, DIM-59, DIM-63, DIM-65, DIM-66, DIM-68, L-115
- **Area / owner files:** plan-core dimension.rs, tools/dimension.rs, dialogs/dimension.rs
- **Round 15 coverage:** dims2 (string-as-one-object option)
- **Round 16 brief:** 28 dimension-segments
- **Why it matters for CDs:** Edited dimension text and leaders appear on every set; strings are separate objects today (DECISIONS 81, 82).
- **Merged from:** source gaps P2#3, P2#28, P2#32, P2#35; earlier Top 40 #12

### 12. The six edit behaviors

- **Scope:** The six edit behaviors (Default, Alternate, Move, Resize, Concentric, Fillet) with temporary key/mouse summons and Connect CAD Segments
- **Size:** M | **Score:** 72 | **Rows:** 19 (4 Missing, 15 Partial) | **Parts:** 1, 2, 3
- **Parity ids:** CAD-3, CAD-4, CAD-68, S-14, S-16, S-65..67, S-94, S-135, S-161..167, S-169, S-176, TXT-40
- **Area / owner files:** editor/behaviors.rs, tools/select.rs, tools/cad.rs, dialogs/edit_behaviors.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 10 edit-behaviours-keys
- **Why it matters for CDs:** Alternate is each tool's other mode in Chief (continuous drawing, angle-keeping reshape); ours is a dominant-axis move (DECISIONS 13).
- **Merged from:** source gaps P1#7

### 13. Scene clipping for sections

- **Scope:** Scene clipping for sections: Clip Sides, Clip Elevation, Clip Lines, stepped cutting planes, Clip to Room options, Framing Back Clip, Poché switch, Depth Cue, Cross Section Slider, Cross Section Lines and Point Markers
- **Size:** L | **Score:** 72 | **Rows:** 19 (10 Missing, 9 Partial) | **Parts:** 5
- **Parity ids:** C-18..21, C-23, C-28, C-47, C-131, C-133, C-136..138, C-140, C-141, C-152, C-157, L-5, L-39
- **Area / owner files:** plan-elevation view.rs, plan-view3d, dialogs/camera.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 32 camera-section-annotation
- **Why it matters for CDs:** Wall sections, kitchen and bath elevations and framing sections need these to show one wall or one bay.
- **Merged from:** source gaps P5#2, P5#17

### 14. Layer management

- **Scope:** Layer management: New/Copy/Merge/Delete/Delete Unused/Reset Names, Layer Set Defaults per view kind, Object Layer Properties, Layer Hider, Select Layer dialog, primary vs secondary layers
- **Size:** M | **Score:** 72 | **Rows:** 16 (5 Missing, 11 Partial) | **Parts:** 1
- **Parity ids:** C-50, LAY-3, LAY-6, LAY-11, LAY-18, LAY-62..65, LAY-67..73
- **Area / owner files:** plan-core layers.rs, layer_sets.rs, dialogs/layer_display.rs, layer_sets.rs, new dialogs/select_layer.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 09 layers-drawing-groups
- **Why it matters for CDs:** Layer discipline is how CD sheets are produced; today the layer list is fixed.
- **Merged from:** source gaps P1#4

### 15. Staircase engine

- **Scope:** Staircase engine: sections and subsections, Complete Break, Disconnect Subsection, merging, curving a middle subsection, landing creation five ways, section table
- **Size:** L | **Score:** 72 | **Rows:** 16 (9 Missing, 7 Partial) | **Parts:** 4
- **Parity ids:** CB-27..29, CB-131..142
- **Area / owner files:** plan-stairs layout.rs, landing.rs, lib.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 21 stairs-engine
- **Why it matters for CDs:** Stairs with a landing, a width change or a curved bottom flight are in nearly every custom plan.
- **Merged from:** source gaps P4#2

### 16. Saved Plan Views

- **Scope:** Saved Plan Views: specification (remember zoom/rotation, show color, watermark, link to layout, save options, selected defaults, reference display) and duplicate/open flows
- **Size:** L | **Score:** 72 | **Rows:** 11 (1 Missing, 10 Partial) | **Parts:** 1, 2
- **Parity ids:** APP-122, L-2, L-24, L-25, LAY-1, LAY-2, LAY-46, LAY-47, LAY-49..51, LAY-54, LAY-56, LAY-57
- **Area / owner files:** plan-core views/model, dialogs/plan_views.rs, shell/docks.rs browser rows
- **Round 15 coverage:** none
- **Round 16 brief:** 26 saved-defaults-views
- **Why it matters for CDs:** Every sheet is a saved view with its own layer set, defaults and scale; the tutorials open one before each lesson.
- **Merged from:** source gaps P1#20, P7#9

### 17. Schedule Specification

- **Scope:** Schedule Specification: Categories to Include tree, room scope (Include Objects from Room), Create Schedule from Room, custom categories, multiple schedules per kind, Number Formatting, Schedule Defaults dialogs
- **Size:** M | **Score:** 72 | **Rows:** 10 (3 Missing, 7 Partial) | **Parts:** 3
- **Parity ids:** L-23, L-25, L-41, L-58, L-59, L-61, L-64, TXT-30
- **Area / owner files:** plan-docs schedule_kinds.rs, plan-core schedules.rs, dialogs/schedule_spec.rs
- **Round 15 coverage:** excel_roundtrip (custom property columns only)
- **Round 16 brief:** 04 schedules
- **Why it matters for CDs:** Every set carries door, window and room-finish schedules; they must be correct and tidy.
- **Merged from:** source gaps P3#1, P7#14; earlier Top 40 #13

### 18. Elevation Reference

- **Scope:** Elevation Reference (absolute, from floor, finished floor, terrain, ceiling, roof) with To Top/To Bottom on every object
- **Size:** L | **Score:** 72 | **Rows:** 9 (3 Missing, 6 Partial) | **Parts:** 1, 3, 4
- **Parity ids:** DW-49, LAY-36, R-89, R-90
- **Area / owner files:** plan-core model slots on Opening, Cabinet, Device, Symbol, Stair, Slab, Soffit; every spec dialog with a height field; plan-3d placement
- **Round 15 coverage:** none
- **Round 16 brief:** 05 common-object-pages
- **Why it matters for CDs:** Heights of fixtures, windows, soffits and cabinets stay right on remodels with changing floors and split levels.
- **Merged from:** source gaps P1#12

### 19. Arithmetic in every number box

- **Scope:** Arithmetic in every number box (+ - * /), with or without units
- **Size:** S | **Score:** 72 | **Rows:** 1 (1 Missing, 0 Partial) | **Parts:** 1
- **Parity ids:** APP-84
- **Area / owner files:** plan-core units.rs parse_length (hook), editor/typed_input.rs, one shared length-field widget
- **Round 15 coverage:** none
- **Round 16 brief:** 10 edit-behaviours-keys
- **Why it matters for CDs:** Daily time saver in cabinet, framing and dimension dialogs; Chief users type sums constantly.
- **Merged from:** source gaps P1#9, P7#28

### 20. Keeping layout views current and editing them

- **Scope:** Keeping layout views current and editing them: dynamic / semi-dynamic update, Missing Layout Views, Pan/Scale, Recenter, Scale to Fit, Rescale dialog, non-scaled corner handle, Layout Box Specification (Linked View, Box Scale, Fill, Label, Layer Set panels)
- **Size:** L | **Score:** 70 | **Rows:** 26 (11 Missing, 15 Partial) | **Parts:** 6
- **Parity ids:** L-4..6, L-12, L-16, L-158..160, L-162..170, L-177..184, L-199, L-200, L-204, L-207
- **Area / owner files:** plan-layout extent.rs, canvas.rs, shell/layout_window.rs, new dialogs/layout_box.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 02 layout-boxes
- **Why it matters for CDs:** Detail callouts and drawing titles under every view; keeping 40+ views current without redrawing.
- **Merged from:** source gaps P6#8

### 21. Saved Defaults for annotation tools

- **Scope:** Saved Defaults for annotation tools (dimensions, clouds, rich text, text, callouts, markers, arrows) with a Saved Defaults dialog and one active at a time
- **Size:** M | **Score:** 70 | **Rows:** 22 (6 Missing, 16 Partial) | **Parts:** 1, 2, 3
- **Parity ids:** CB-20, CB-36, DIM-4, DIM-6, DIM-40, DIM-51, DIM-53, DIM-54, DS-4, DS-6, DS-18, DS-22..24, DS-28, DS-30, DS-31, DW-124, +6 more
- **Area / owner files:** plan-core defaults.rs, new dialogs/saved_defaults.rs, tools double-click hooks
- **Round 15 coverage:** dims2 (dimension saves only)
- **Round 16 brief:** 26 saved-defaults-views
- **Why it matters for CDs:** Switching text and dimension setups per sheet scale is how offices keep annotations consistent.
- **Merged from:** source gaps P1#15, P2#15, P3#23, P7#9; earlier Top 40 #35

### 22. Automatic mulling, Minimum Separation, Mulled Unit dialog and defaults, window Levels, ...

- **Scope:** Automatic mulling, Minimum Separation, Mulled Unit dialog and defaults, window Levels, stacked-opening cleanup
- **Size:** M | **Score:** 70 | **Rows:** 17 (7 Missing, 10 Partial) | **Parts:** 3
- **Parity ids:** DW-51, DW-52, DW-74, DW-89, DW-122, DW-143, DW-155, DW-157..161, L-74
- **Area / owner files:** plan-core openings.rs, tools/opening/place.rs, dialogs/opening.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 22 openings-mulled-bay
- **Why it matters for CDs:** Window groups, transoms and sidelights are routine in custom homes and must schedule as units (DECISIONS 42).
- **Merged from:** source gaps P3#7

### 23. Poché on cut walls, platforms and roofs, plus custom hatch patterns

- **Scope:** Poché on cut walls, platforms and roofs, plus custom hatch patterns (Create Pattern from CAD, Pattern window, tile groups, infinite pattern lines, .pat import)
- **Size:** L | **Score:** 70 | **Rows:** 8 (6 Missing, 2 Partial) | **Parts:** 1, 2, 4
- **Parity ids:** CAD-71, CAD-78..81, LAY-55, RF-100
- **Area / owner files:** plan-layout/plan-elevation fills, plan-core wall fills, new pattern editor
- **Round 15 coverage:** none
- **Round 16 brief:** 08 line-fill-styles-poche
- **Why it matters for CDs:** Wall poché is on every plan and section of a CD set; plans and details read as drawings, not outlines.
- **Merged from:** source gaps P1#5

### 24. Thermal Envelope Data (CSV) and REScheck (.rxl) export

- **Scope:** Thermal Envelope Data (CSV) and REScheck (.rxl) export: orientation, assemblies, conditioned rooms
- **Size:** M | **Score:** 70 | **Rows:** 3 (3 Missing, 0 Partial) | **Parts:** 5
- **Parity ids:** L-83
- **Area / owner files:** new plan-docs rescheck.rs, new dialogs/rescheck.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 27 text-macros-rescheck
- **Why it matters for CDs:** Energy-code submittals for Georgia permits; U-factor, SHGC and conditioned-room data already exist.
- **Merged from:** source gaps P5#6; earlier Top 40 #38

### 25. Clear Terrain erases the perimeter, data and features instead of only the built surface

- **Scope:** Clear Terrain erases the perimeter, data and features instead of only the built surface
- **Size:** S | **Score:** 70 | **Rows:** 0 (0 Missing, 0 Partial) | **Parts:** part 6 row 1309 (status Differs)
- **Parity ids:** NO SPEC ids (page-level rows)
- **Area / owner files:** plan-app main.rs C::Clear, plan-terrain
- **Round 15 coverage:** none
- **Round 16 brief:** 33 terrain-site
- **Why it matters for CDs:** The command contradicts the manual and destroys a site model on a mis-click.
- **Merged from:** source gaps P6#1

### 26. Auto dimensions

- **Scope:** Auto dimensions: Exterior three-row sets, Interior, Room, Elevation and Story Pole with elevation markers and grade reference, Auto Refresh, dimensions in camera views and overviews
- **Size:** L | **Score:** 68 | **Rows:** 21 (9 Missing, 9 Partial, 3 In progress) | **Parts:** 2, 3, 5
- **Parity ids:** C-130, C-142, DIM-15, DIM-18, DIM-24, DIM-26..29, DIM-32, DIM-38, DIM-47, DIM-52, DIM-61, DIM-62, DIM-64, DIM-67
- **Area / owner files:** tools/dimension.rs, editor/dim_assoc.rs, plan-elevation dims.rs
- **Round 15 coverage:** dims2 (Auto Elevation/Story Pole)
- **Round 16 brief:** 28 dimension-segments
- **Why it matters for CDs:** Exterior elevations and sections need height strings; Chief's template builds exterior dimensions at room definition.
- **Merged from:** source gaps P2#5, P2#29, P2#32, P2#35, P7#5

### 27. Send to Layout options

- **Scope:** Send to Layout options: site-plan scales (1 in = 30/40/50/100 ft or typed ratio), too-large warning, Link Saved Plan View, camera view options (Live View, Plot Lines, Color Fill), Snap to CAD point, Send All Remaining Views
- **Size:** M | **Score:** 68 | **Rows:** 12 (7 Missing, 5 Partial) | **Parts:** 6
- **Parity ids:** L-3..5, L-12, L-22, L-147..149, L-152..157, L-161
- **Area / owner files:** plan-layout send.rs, shell/layout_window.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 02 layout-boxes
- **Why it matters for CDs:** Site plans, elevations and details have to land on the sheet at the right scale.
- **Merged from:** source gaps P7#3, P7#16

### 28. Wall roof directives

- **Scope:** Wall roof directives: Dutch gable, high shed, knee wall, Roof Cuts Wall at Bottom, auto roof return, lower wall type, frieze, end truss, combine; Roof panel for several walls at once and for interior-kind walls
- **Size:** M | **Score:** 68 | **Rows:** 8 (1 Missing, 7 Partial) | **Parts:** 2, 4
- **Parity ids:** R-68, RF-3, RF-4, RF-18, RF-24, RF-26, RF-31, W-147, W-148
- **Area / owner files:** dialogs/wall/tabs.rs (Roof tab), plan-roof spec.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 18 roof-eaves-heights
- **Why it matters for CDs:** Gambrel, mansard, half-hip and knee-wall recipes cannot be followed without it.
- **Merged from:** source gaps P2#16, P7#4, P7#5

### 29. Number Style and Angle Style dialogs

- **Scope:** Number Style and Angle Style dialogs (feet-inches formats, fractions, degrees/minutes/seconds, quadrant and azimuth bearings, pitch styles) and custom unit conversions
- **Size:** M | **Score:** 68 | **Rows:** 3 (1 Missing, 2 Partial) | **Parts:** 1
- **Parity ids:** PR-16, PR-26, PR-30, PR-31
- **Area / owner files:** plan-core units.rs, new dialogs/number_style.rs, dialogs/preferences/pages.rs (hook)
- **Round 15 coverage:** none
- **Round 16 brief:** 06 survey-entry
- **Why it matters for CDs:** Site plans need bearings and consistent distance formats; the survey workflow cannot start without them.
- **Merged from:** source gaps P1#19, P7#30

### 30. Terrain Specification parity

- **Scope:** Terrain Specification parity: Absolute Elevation (Automatic, Retain Surface at Reference Point or Contour 0), Elevation Reference Point, Hide Terrain Intersected by Building, Skirt, smoothing levels and triangle count, Polyline/Spline/Line Style/Fill/Materials/Label/Schedule panels
- **Size:** M | **Score:** 66 | **Rows:** 29 (16 Missing, 13 Partial) | **Parts:** 6
- **Parity ids:** CB-43, CB-47, CB-50, CB-51, CB-505..509, CB-511, CB-512, CB-530..542, CB-544..546
- **Area / owner files:** plan-terrain surface.rs, mesh.rs, dialogs/terrain.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 33 terrain-site
- **Why it matters for CDs:** Decides how the lot meets Floor 1; needed for daylight and walkout basements and sloped sites (DECISIONS 106).
- **Merged from:** source gaps P6#5, P7#20

### 31. Cabinet runs

- **Scope:** Cabinet runs: automatic fillers, filler tools, merging within 3 in, snapping/aligning rules, minimum cabinet size, appliance-aware countertops
- **Size:** M | **Score:** 66 | **Rows:** 22 (7 Missing, 15 Partial) | **Parts:** 3
- **Parity ids:** CB-1..6, CB-8, CB-14..19, CB-60, CB-83, CB-478, CB-480..483, CB-491
- **Area / owner files:** plan-cabinets filler.rs, top.rs, tools/cabinet.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 23 cabinet-runs-labels
- **Why it matters for CDs:** Kitchen elevations and the cabinet schedule depend on continuous runs and fillers (DECISIONS 53).
- **Merged from:** source gaps P3#10

### 32. Bay, box and bow windows as wall-section units

- **Scope:** Bay, box and bow windows as wall-section units: dialogs, depth handle, roof options, foundation under, explode, component windows
- **Size:** L | **Score:** 66 | **Rows:** 14 (11 Missing, 3 Partial) | **Parts:** 3, 4
- **Parity ids:** DW-48, DW-64, DW-155, RF-29
- **Area / owner files:** plan-core openings.rs, plan-3d windows.rs, dialogs/opening.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 22 openings-mulled-bay
- **Why it matters for CDs:** Bays are common in custom homes and drive roof and foundation (DECISIONS 125).
- **Merged from:** source gaps P3#9; earlier Top 40 #19

### 33. Watermark

- **Scope:** Watermark: View toggle, Watermark Defaults (text or image, tile / border / fit, angle, transparency, margins), Include Watermark in Print
- **Size:** M | **Score:** 66 | **Rows:** 11 (10 Missing, 1 Partial) | **Parts:** 1, 6
- **Parity ids:** L-53, L-209..211, L-214..217
- **Area / owner files:** new dialogs/watermark.rs, plan-layout print.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 03 print-sheet-watermark
- **Why it matters for CDs:** DRAFT and NOT FOR CONSTRUCTION marks on review sets.
- **Merged from:** source gaps P6#4, P6#19

### 34. Enter Coordinates dialog on Tab/Enter during drawing and moving; move/rotate/resize/ref...

- **Scope:** Enter Coordinates dialog on Tab/Enter during drawing and moving; move/rotate/resize/reflect by typed numbers
- **Size:** M | **Score:** 66 | **Rows:** 8 (0 Missing, 8 Partial) | **Parts:** 1, 2
- **Parity ids:** S-28, S-48, S-105, S-132, S-146, S-156, S-157, S-159, S-160, W-15, W-16
- **Area / owner files:** editor/typed_input.rs, dialogs/transform.rs, new dialogs/enter_coordinates.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 10 edit-behaviours-keys
- **Why it matters for CDs:** Exact positions without grid tricks: absolute, relative and polar entry used for every precise move.
- **Merged from:** source gaps P7#28

### 35. Line styles as a library

- **Scope:** Line styles as a library: Line Style Management and Specification (dash, dot and text components), Used/System marks, Import .lin
- **Size:** L | **Score:** 66 | **Rows:** 7 (6 Missing, 1 Partial) | **Parts:** 1, 2
- **Parity ids:** CAD-82..86
- **Area / owner files:** new plan-core line_styles.rs, dialogs/line_style.rs, render stroke hook
- **Round 15 coverage:** none
- **Round 16 brief:** 08 line-fill-styles-poche
- **Why it matters for CDs:** Property lines, centerlines, hidden lines and 'existing to remain' lines carry the CD set; fixed dashes are not enough.
- **Merged from:** source gaps P1#3

### 36. Default Sets

- **Scope:** Default Sets (named bundles of saved defaults, layer set and current CAD layer), Active Defaults dialog and toolbar controls
- **Size:** M | **Score:** 66 | **Rows:** 4 (1 Missing, 3 Partial) | **Parts:** 1
- **Parity ids:** APP-24, DS-6, DS-32, TB-6
- **Area / owner files:** plan-core defaults.rs, new dialogs/default_sets.rs, toolbar controls
- **Round 15 coverage:** none
- **Round 16 brief:** 26 saved-defaults-views
- **Why it matters for CDs:** One click swaps the whole annotation and layer setup for plan, electrical, framing or presentation views.
- **Merged from:** source gaps P1#15, P7#9

### 37. Wall drawing gestures

- **Scope:** Wall drawing gestures: chain drawing, dashed alignment guides, angle snapping, typed length+angle, Spacebar/Esc rules
- **Size:** M | **Score:** 66 | **Rows:** 3 (0 Missing, 3 Partial) | **Parts:** 2
- **Parity ids:** W-3..5, W-11, W-14, W-17, W-18
- **Area / owner files:** tools/wall.rs, editor/tempdim.rs, snap.rs
- **Round 15 coverage:** walls2
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** The first thing done in every plan.
- **Merged from:** earlier Top 40 #5

### 38. Moldings system

- **Scope:** Moldings system: shared Moldings / Profiles / Rails panel with offsets and stacking, profiles from closed polylines, Molding Specification, 3D moldings along a path, Replace Moldings, molding polylines (sloped edges), room molding polylines, wall cap
- **Size:** L | **Score:** 64 | **Rows:** 37 (18 Missing, 18 Partial, 1 In progress) | **Parts:** 2, 3, 4
- **Parity ids:** CB-7, CB-57, CB-338..355, CB-357..363, R-21, R-34, R-84, R-113, RF-162, RF-163, W-111
- **Area / owner files:** new plan-core moldings.rs, plan-3d molding.rs, tools/molding.rs, plan-core extras.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 31 moldings-trim
- **Why it matters for CDs:** Crown and base details beyond the nine built-in profiles; kitchens and baths live on millwork.
- **Merged from:** source gaps P4#19, P7#27

### 39. Print dialog

- **Scope:** Print dialog: Print Source (Drawing Sheet or Current View), Check Plot at a fraction, Collate, Scale to Fit and Center Sheet, printer per view, Print Image options, layout print specifics
- **Size:** M | **Score:** 64 | **Rows:** 13 (6 Missing, 7 Partial) | **Parts:** 6
- **Parity ids:** L-18, L-19, L-22, L-197, L-205, L-206, L-218..227
- **Area / owner files:** dialogs/print.rs, plan-layout print.rs, plan-docs pdf/**
- **Round 15 coverage:** none
- **Round 16 brief:** 03 print-sheet-watermark
- **Why it matters for CDs:** Proofing 24x36 sets on 11x17 paper is daily work.
- **Merged from:** source gaps P6#7

### 40. Fill Style panel everywhere

- **Scope:** Fill Style panel everywhere (pattern list, scale, offsets, angle, color source, background, gradient), library fill styles, Fill Style Painter
- **Size:** L | **Score:** 64 | **Rows:** 12 (6 Missing, 6 Partial) | **Parts:** 1, 6
- **Parity ids:** C-55, C-61, CAD-56, CAD-69, CAD-70, CAD-72..77, CB-46, LAY-18, S-116, S-174
- **Area / owner files:** new plan-core fill_styles.rs, dialogs/fill_style.rs, plan-materials pattern.rs, editor/render.rs hatch
- **Round 15 coverage:** none
- **Round 16 brief:** 08 line-fill-styles-poche
- **Why it matters for CDs:** Material hatches in plans, sections and details; every object dialog has a Fill Style panel in Chief.
- **Merged from:** source gaps P1#5

### 41. Dynamic defaults

- **Scope:** Dynamic defaults ('Use Default' radio / wrench) and the Set as Default edit button for every object kind
- **Size:** M | **Score:** 64 | **Rows:** 11 (9 Missing, 2 Partial) | **Parts:** 1, 2, 3
- **Parity ids:** CAD-67, CB-475, DS-26, DS-27, DS-29, RF-25, S-85, W-121
- **Area / owner files:** plan-core defaults.rs (use_default flags), every spec dialog, edit_commands.rs
- **Round 15 coverage:** defaults_pages (tree only)
- **Round 16 brief:** 26 saved-defaults-views
- **Why it matters for CDs:** Change a default once and every object still on 'Use Default' follows; Chief templates depend on it.

### 42. Room labels and Living Area

- **Scope:** Room labels and Living Area: standard/interior area formats, rough-ceiling rule (<48 in out), per-room override, per-structure Living Area label, Room Polylines (Make Room Polyline, Standard Area Polyline), label macros
- **Size:** M | **Score:** 64 | **Rows:** 10 (3 Missing, 7 Partial) | **Parts:** 2, 3, 5
- **Parity ids:** CB-13, DW-59, DW-115, DW-155, L-74, L-83, R-19, R-21, R-42..44, R-46, R-47, R-49, R-50, R-108, R-114
- **Area / owner files:** plan-core rooms.rs, dialogs/room.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 15 rooms-living-area
- **Why it matters for CDs:** Living area and footprint areas go on every permit set (DECISIONS 300).
- **Merged from:** source gaps P2#9

### 43. Object labels in plan, camera and section views

- **Scope:** Object labels in plan, camera and section views: automatic, custom, size formats, position/orientation, label layers, Edit Label button, Suppress Label; Object Information, Components, Schedule and Manufacturer panels on every object
- **Size:** L | **Score:** 64 | **Rows:** 10 (1 Missing, 9 Partial) | **Parts:** 3, 6
- **Parity ids:** DW-63, DW-104, L-34, L-74, L-135
- **Area / owner files:** shell/spec_dialogs.rs (common pages host), dialogs/object_info.rs
- **Round 15 coverage:** excel_roundtrip (props), materials_list (Components)
- **Round 16 brief:** 05 common-object-pages
- **Why it matters for CDs:** Supplier/model data flow into schedules, labels and the materials list; missing on walls, doors, windows, symbols.
- **Merged from:** source gaps P6#14; earlier Top 40 #40

### 44. Reference Display with several rows

- **Scope:** Reference Display with several rows (another plan file, layer set per row, offsets/angle), Swap Floor/Reference, Reference Model in cameras
- **Size:** L | **Score:** 64 | **Rows:** 9 (4 Missing, 5 Partial) | **Parts:** 1
- **Parity ids:** LAY-9, LAY-39..45, R-96
- **Area / owner files:** dialogs/reference_display.rs, editor/render.rs overlay, plan-core reference slot
- **Round 15 coverage:** none
- **Round 16 brief:** 07 construction-lines-reference
- **Why it matters for CDs:** Remodels show the existing plan under the proposed plan; additions align floor to floor.
- **Merged from:** source gaps P1#2

### 45. Retaining Wall tools

- **Scope:** Retaining Wall tools (Terrain Break plus a wall sized from the terrain on both sides), terrain wall that follows the ground, stepping as an option, streams (DECISIONS 108)
- **Size:** M | **Score:** 64 | **Rows:** 3 (2 Missing, 1 Partial) | **Parts:** 6
- **Parity ids:** CB-46, CB-521, CB-522
- **Area / owner files:** plan-terrain grading.rs, tools/terrain.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 33 terrain-site
- **Why it matters for CDs:** Retaining walls are common on custom-home sites.
- **Merged from:** source gaps P6#6

### 46. Roof truss layout for the truss manufacturer

- **Scope:** Roof truss layout for the truss manufacturer: TR-X labels shared per configuration, Truss Detail, truss specification fields, Force Rebuild, Lock Envelope
- **Size:** L | **Score:** 62 | **Rows:** 28 (13 Missing, 15 Partial) | **Parts:** 4
- **Parity ids:** CB-39, CB-41, CB-291..313, RF-54
- **Area / owner files:** plan-framing truss.rs, new dialogs/truss.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 30 framing-layout-trusses
- **Why it matters for CDs:** Truss companies work from the truss plan with labels.
- **Merged from:** source gaps P4#6

### 47. Build Framing semantics

- **Scope:** Build Framing semantics: Build for Selected / Parent object, Build Once per floor, Framing Groups, bearing walls and beams, framing reference markers, Move to Framing Ref, joist direction lines
- **Size:** L | **Score:** 62 | **Rows:** 24 (9 Missing, 15 Partial) | **Parts:** 2, 4
- **Parity ids:** CB-35, CB-36, CB-38, CB-39, CB-254..267, RF-161, W-134
- **Area / owner files:** plan-framing build.rs, layout.rs, dialogs/framing.rs, tools/framing.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 30 framing-layout-trusses
- **Why it matters for CDs:** Controls what gets framed and where members start.
- **Merged from:** source gaps P4#13, P7#18

### 48. Staircase Specification

- **Scope:** Staircase Specification: Information read-outs, Best Fit / Make Best Fit, tread depth modes, Lock Top/Bottom, Radius Reference, specifications table, Open Risers/Underneath, runner, top landing nosing, stringers (custom, trim against wall, extend top, large base), plan display
- **Size:** M | **Score:** 62 | **Rows:** 21 (14 Missing, 5 Partial, 2 In progress) | **Parts:** 4
- **Parity ids:** CB-24, CB-32, CB-158..169, CB-171..176
- **Area / owner files:** dialogs/stairs.rs
- **Round 15 coverage:** stairs2 (remaining tabs)
- **Round 16 brief:** 21 stairs-engine
- **Why it matters for CDs:** Riser/tread compliance and the stair detail sheet.
- **Merged from:** source gaps P4#3, P7#19; earlier Top 40 #9

### 49. Wall connection repair

- **Scope:** Wall connection repair: off-angle and unconnected caution icons, Fix Off Angle dialog, Auto Connect lock, Fix Wall Connections, Connect Walls, acute junctions, Reset Notification Icons, siding-side-out auto reverse
- **Size:** M | **Score:** 62 | **Rows:** 11 (7 Missing, 4 Partial) | **Parts:** 2
- **Parity ids:** W-18, W-22, W-23, W-32, W-37, W-41, W-45, W-120, W-131..133, W-135, W-136, W-144
- **Area / owner files:** plan-core joins.rs, editor/connect.rs, wall_edit.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 14 wall-edit-tools
- **Why it matters for CDs:** Catches bad geometry before rooms or roofs fail.
- **Merged from:** source gaps P2#7, P2#33

### 50. Cabinet labels

- **Scope:** Cabinet labels (B24, 3DB24, SB24R), schedule categories, cabinet layers and plan display (closed doors/drawers, module lines layer, opening indicators, Suppress Label)
- **Size:** M | **Score:** 62 | **Rows:** 8 (2 Missing, 6 Partial) | **Parts:** 3
- **Parity ids:** CB-13, CB-21, CB-481, CB-485, CB-487, L-57
- **Area / owner files:** plan-cabinets symbol.rs, plan-docs schedule_kinds.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 23 cabinet-runs-labels
- **Why it matters for CDs:** Cabinet labels are what contractors read on the plan.
- **Merged from:** source gaps P3#11, P7#24

### 51. Door and window defaults per type

- **Scope:** Door and window defaults per type (Interior/Exterior Hinged and Sliding, Doorway, Pocket, Bifold, Garage; window types) with dynamic Use Default, plus Door/Window defaults tabs
- **Size:** M | **Score:** 62 | **Rows:** 5 (3 Missing, 2 Partial) | **Parts:** 3
- **Parity ids:** DW-103, DW-154, DW-164
- **Area / owner files:** plan-core openings.rs, dialogs/default_pages/architectural.rs, dialogs/opening.rs
- **Round 15 coverage:** opening_tabs (spec tabs only)
- **Round 16 brief:** 22 openings-mulled-bay
- **Why it matters for CDs:** Setting up a template once saves hours; today only interior, exterior, garage and one window set exist.
- **Merged from:** source gaps P3#8

### 52. Materials List scopes, specification, columns, export formats and Master List

- **Scope:** Materials List scopes, specification, columns, export formats and Master List (Calculate From Selection / Room / Polyline, Report vs Live list, 21 columns, TXT/XML/HTML/BuilderTREND, Master List update)
- **Size:** L | **Score:** 60 | **Rows:** 64 (33 Missing, 31 Partial) | **Parts:** 2, 3, 4, 6
- **Parity ids:** CB-356, L-23, L-27, L-29..37, L-56, L-75, L-84..87, L-89..114, L-116..134, L-136, L-137
- **Area / owner files:** plan-docs materials/**, master_list.rs, dialogs/materials_list.rs
- **Round 15 coverage:** materials_list (in progress)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Cost estimates and builder bids rest on these columns.
- **Merged from:** source gaps P6#11, P6#12, P6#13

### 53. Material Painter scoping modes, Plan Materials dialog, Select Material dialog

- **Scope:** Material Painter scoping modes, Plan Materials dialog, Select Material dialog (library / plan / defaults panels), plan-specific materials that travel with the plan, Purge, Convert Textures to Materials, Create Plan Materials Library
- **Size:** L | **Score:** 60 | **Rows:** 49 (25 Missing, 21 Partial, 3 In progress) | **Parts:** 2, 4, 5
- **Parity ids:** C-45, C-55, C-56, C-58..61, C-64, C-74, C-80..92, C-94, C-95
- **Area / owner files:** plan-materials library.rs, tools/materials/**, dialogs/materials.rs
- **Round 15 coverage:** lightbeans (import only)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Material selections are client decisions; a plan opened elsewhere loses custom materials.
- **Merged from:** source gaps P5#4

### 54. Cabinet face items

- **Scope:** Cabinet face items: item types (18), per-item dialogs, shelf specification, hardware, pilasters and feet, library doors/drawers, group-edit No Change
- **Size:** M | **Score:** 60 | **Rows:** 16 (6 Missing, 10 Partial) | **Parts:** 3
- **Parity ids:** CB-10..12, CB-479, CB-481, CB-487, CB-491..493, CB-497, CB-498
- **Area / owner files:** plan-cabinets face.rs, dress.rs, dialogs/cabinet.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 24 cabinet-faces-specials
- **Why it matters for CDs:** Kitchen design needs false drawers, double drawers, panels, rollouts and shelf counts.
- **Merged from:** source gaps P3#12, P7#24; earlier Top 40 #8

### 55. Custom Countertop Specification, waterfall add/remove, backsplash, molding polylines; countertops as cabinet components

- **Scope:** Custom Countertop Specification, waterfall add/remove, backsplash, molding polylines; countertops as cabinet components (DECISIONS 18)
- **Size:** M | **Score:** 60 | **Rows:** 9 (4 Missing, 5 Partial) | **Parts:** 3
- **Parity ids:** CB-14, CB-481, CB-484, CB-486
- **Area / owner files:** plan-cabinets top.rs, dialogs/cabinet.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 24 cabinet-faces-specials
- **Why it matters for CDs:** Countertop edges and waterfalls are on every kitchen sheet.
- **Merged from:** source gaps P3#14; earlier Top 40 #8

### 56. Drawing Sheet Setup per view with Drawing Scale and per-edge margins feeding Send to La...

- **Scope:** Drawing Sheet Setup per view with Drawing Scale and per-edge margins feeding Send to Layout; Customize Sheet Sizes program-wide; Drawing Sheet object
- **Size:** M | **Score:** 60 | **Rows:** 6 (2 Missing, 4 Partial) | **Parts:** 6
- **Parity ids:** L-8, L-12, L-18, L-22, L-201..203, L-208
- **Area / owner files:** dialogs/print.rs, new dialogs/drawing_sheet.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 03 print-sheet-watermark
- **Why it matters for CDs:** The scale chosen here is the scale Send to Layout offers (DECISIONS 63).
- **Merged from:** source gaps P6#7, P7#29

### 57. Electrical Defaults

- **Scope:** Electrical Defaults: library objects per tool, four height groups (Outlet, Switch, Above Base Cabinet, On Cabinet Side), Use Default Heights, Set as Default
- **Size:** M | **Score:** 60 | **Rows:** 5 (3 Missing, 2 Partial) | **Parts:** 3
- **Parity ids:** E-10, E-11, E-18..20
- **Area / owner files:** plan-electrical defaults.rs, dialogs/electrical.rs, default_pages/electrical.rs
- **Round 15 coverage:** defaults_pages (partial)
- **Round 16 brief:** 25 electrical-defaults-connections
- **Why it matters for CDs:** Electrical plans depend on the right symbols and heights (DECISIONS 87, 89).
- **Merged from:** source gaps P3#18

### 58. Import Settings from a plan or layout, plus .layers, .cadefs, wall-definition .dat and ...

- **Scope:** Import Settings from a plan or layout, plus .layers, .cadefs, wall-definition .dat and note-type imports
- **Size:** M | **Score:** 60 | **Rows:** 5 (4 Missing, 1 Partial) | **Parts:** 1
- **Parity ids:** APP-2, DS-36..40
- **Area / owner files:** new dialogs/import_settings.rs, plan-core defaults.rs, layer_sets.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 26 saved-defaults-views
- **Why it matters for CDs:** Carries the studio's wall types, layer sets and saved defaults from job to job without retyping.
- **Merged from:** source gaps P1#6

### 59. Camera and section specification panels

- **Scope:** Camera and section specification panels: Plan Display (placement, line style, arrow, callout), Layer and Drawing Group, Selected Defaults, Below Grade, rendering options (Reflections, Bloom, AO amount, Super Resolution, Depth of Field, Maximum Lights, Clip Surfaces Within), Backdrop panel; Hide Camera-Facing Exterior Walls
- **Size:** M | **Score:** 58 | **Rows:** 34 (13 Missing, 21 Partial) | **Parts:** 5
- **Parity ids:** C-7, C-24, C-30, C-47, C-51, C-65, C-67, C-70, C-109, C-119, C-139, C-146..151, C-153, C-156, +1 more
- **Area / owner files:** dialogs/camera.rs, plan-core camera.rs
- **Round 15 coverage:** cameras2 (partial)
- **Round 16 brief:** 32 camera-section-annotation
- **Why it matters for CDs:** Section marks and callouts on layout sheets are controlled here; Below Grade drives the grade-line look.
- **Merged from:** source gaps P5#5, P5#9, P5#15; earlier Top 40 #24

### 60. Framing detail options

- **Scope:** Framing detail options: lap/butt over supports, blocking styles, rim joist options, wall connection styles, plates, mitre ends, header sizes by opening width, hip girders, roof overframing, lookouts
- **Size:** M | **Score:** 58 | **Rows:** 28 (16 Missing, 12 Partial) | **Parts:** 4
- **Parity ids:** CB-38, CB-40, CB-219..231, CB-233..245, RF-54
- **Area / owner files:** plan-framing floor.rs, wall.rs, roof.rs, dialogs/framing.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 30 framing-layout-trusses
- **Why it matters for CDs:** Framing plans and wall details for permit sets in framed additions.
- **Merged from:** source gaps P4#13

### 61. Stair railings and construction recipes

- **Scope:** Stair railings and construction recipes: solid / inset / partial rails, housed stringer, concrete, masonry and steel stairs, transitions, handrail extensions and returns, brackets, newel/baluster/rail panels
- **Size:** L | **Score:** 58 | **Rows:** 23 (10 Missing, 11 Partial, 2 In progress) | **Parts:** 4
- **Parity ids:** CB-31, CB-34, CB-151..156, CB-170, CB-177..187
- **Area / owner files:** plan-stairs railing.rs, plan-3d railing.rs, dialogs/stairs.rs
- **Round 15 coverage:** stairs2 (newel/baluster library)
- **Round 16 brief:** 21 stairs-engine
- **Why it matters for CDs:** Finish carpentry and rail details on the stair sheet.
- **Merged from:** earlier Top 40 #9

### 62. Stair drawing behavior

- **Scope:** Stair drawing behavior: wall and Reference Display snaps with Ctrl override, Alt to reverse arrow or draw downward, stairs to terrain/deck, New Shaped Staircase dialog, wall-aware L/U preview, default direction, U gap and split landing
- **Size:** M | **Score:** 58 | **Rows:** 20 (11 Missing, 8 Partial, 1 In progress) | **Parts:** 4
- **Parity ids:** CB-24, CB-26, CB-29, CB-34, CB-99..101, CB-103..116
- **Area / owner files:** tools/stairs.rs, plan-stairs layout.rs, dialogs/stairs.rs
- **Round 15 coverage:** stairs2 (deck stairs)
- **Round 16 brief:** 21 stairs-engine
- **Why it matters for CDs:** Drawing speed and correctness on remodel stairs next to existing walls (DECISIONS 6).
- **Merged from:** source gaps P4#7

### 63. Foundation build behavior

- **Scope:** Foundation build behavior: Auto Rebuild, garage curb/door cutouts, S step markers and stepped footings, interior footings, chamfer, basement counts as living area at 48 in, stem wall height (DECISIONS 300, 301)
- **Size:** M | **Score:** 58 | **Rows:** 18 (3 Missing, 15 Partial) | **Parts:** 3
- **Parity ids:** CB-51, L-39, L-61, R-18, R-26, R-31, R-61, R-63, R-130, R-131, R-133, R-135..137, R-140
- **Area / owner files:** plan-core foundation.rs, editor/foundation_view.rs, plan-3d foundation.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 17 floors-foundation-fireplace
- **Why it matters for CDs:** Foundation plans on sloping lots need steps shown; rebuild saves rework.
- **Merged from:** source gaps P3#15, P7#22; earlier Top 40 #6

### 64. Contour presentation and terrain labels

- **Scope:** Contour presentation and terrain labels: primary/secondary layers, Offset, label units, Highlight Negative Elevations, 2D smoothing passes, Terrain Labels layer, terrain and road schedule categories, library terrain objects
- **Size:** M | **Score:** 58 | **Rows:** 15 (6 Missing, 9 Partial) | **Parts:** 6
- **Parity ids:** CB-43, CB-48, CB-50, CB-51, CB-53, CB-60, CB-73..76, CB-78, CB-523..529, CB-604, CB-608
- **Area / owner files:** plan-terrain contour.rs, plan-docs terrain_report.rs, editor/site_view
- **Round 15 coverage:** none
- **Round 16 brief:** 33 terrain-site
- **Why it matters for CDs:** Site-plan legibility and site quantities on the plot plan sheet.
- **Merged from:** source gaps P6#9, P6#10

### 65. Exterior Room object and specification, Calculate Materials in Room, Create Schedule fr...

- **Scope:** Exterior Room object and specification, Calculate Materials in Room, Create Schedule from Room, Create Room Elevation Views, Auto Room Dimensions button, Turn Ceiling On/Off
- **Size:** M | **Score:** 58 | **Rows:** 14 (6 Missing, 8 Partial) | **Parts:** 2, 3
- **Parity ids:** CB-36, DIM-27, L-60, R-16, R-30, R-32, R-42, R-44, R-51, R-56, R-104..107, R-109, S-107
- **Area / owner files:** plan-core rooms.rs, dialogs/room.rs, editor/rooms_edit.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 15 rooms-living-area
- **Why it matters for CDs:** Living-area labels, footprints and quantity takeoffs by room.
- **Merged from:** source gaps P2#9, P2#21

### 66. CAD line/arc/polyline edit tools

- **Scope:** CAD line/arc/polyline edit tools: Lock Center, Intersect/Join Two Lines, Complete Break, Fillet All Corners, Chamfer, Simplify, Concentric Resize with Sticky Mode, Close Polyline
- **Size:** L | **Score:** 58 | **Rows:** 13 (8 Missing, 5 Partial) | **Parts:** 1, 2
- **Parity ids:** CAD-15, CAD-16, CAD-21, CAD-26, CAD-89, CAD-90, CAD-93, CAD-94, CAD-97, CAD-99..102, CAD-111, S-59, S-191, W-66
- **Area / owner files:** plan-core cad.rs, tools/cad_ops.rs, tools/cad/edit.rs
- **Round 15 coverage:** cad2 (Boolean, Trim/Extend only)
- **Round 16 brief:** 11 cad-edit-tools-area
- **Why it matters for CDs:** Daily detail drafting: a group of 15 small tools that speed every detail and site outline.
- **Merged from:** source gaps P1#8, P2#25; earlier Top 40 #31

### 67. Room Type Defaults dialog with the full Room Specification panels per type, Room Label defaults, room functions

- **Scope:** Room Type Defaults dialog with the full Room Specification panels per type, Room Label defaults, room functions (Balcony, Court, Slab, Attic; Open Below for stairwells) and function-driven behavior
- **Size:** M | **Score:** 58 | **Rows:** 12 (1 Missing, 11 Partial) | **Parts:** 2, 3, 4
- **Parity ids:** CB-30, R-4, R-5, R-15, R-18, R-30, R-37, R-38, R-40, R-42..44, R-50, R-98..101, R-103, TXT-17
- **Area / owner files:** dialogs/default_lists.rs, plan-core rooms.rs
- **Round 15 coverage:** defaults_pages (list only)
- **Round 16 brief:** 15 rooms-living-area
- **Why it matters for CDs:** Room types set finishes, moldings and decks once (DECISIONS 300, 310).
- **Merged from:** source gaps P2#10, P2#12

### 68. Text macros

- **Scope:** Text macros: Insert Macro on every text field, global/object/user macros (%comment%, %description%, %page%), Project Information as macros, macro import/export
- **Size:** M | **Score:** 58 | **Rows:** 11 (3 Missing, 8 Partial) | **Parts:** 3, 6
- **Parity ids:** L-52, L-54, L-62, TXT-20, TXT-36, TXT-59..64
- **Area / owner files:** new plan-core macros.rs, dialogs/project_info.rs, dialogs/text
- **Round 15 coverage:** none
- **Round 16 brief:** 27 text-macros-rescheck
- **Why it matters for CDs:** Title-block data and notes that fill themselves.
- **Merged from:** source gaps P3#26, P6#21, P7#21

### 69. Edit Area

- **Scope:** Edit Area (all floors, visible only, drawn polyline marquee) and Place at Allowed Angles
- **Size:** M | **Score:** 58 | **Rows:** 6 (2 Missing, 4 Partial) | **Parts:** 1, 2, 4
- **Parity ids:** R-14, S-90, S-177..179
- **Area / owner files:** tools/select/area.rs, tools/select.rs, editor/edit_commands.rs
- **Round 15 coverage:** cad2 (select/CAD owner)
- **Round 16 brief:** 11 cad-edit-tools-area
- **Why it matters for CDs:** Moving or copying a whole wing or scheme across floors is a daily remodel move.
- **Merged from:** source gaps P1#18; earlier Top 40 #1

### 70. Framing Member Defaults and Framing Types management

- **Scope:** Framing Member Defaults and Framing Types management (composition, shape, steel/concrete), Automatic Framing Defaults panels (Foundation, floors, Deck, Wall, Openings, Fireplaces, Roof, Trusses), Role per member
- **Size:** L | **Score:** 56 | **Rows:** 26 (11 Missing, 15 Partial) | **Parts:** 4
- **Parity ids:** CB-35, CB-38, CB-202..218, CB-246..248
- **Area / owner files:** plan-framing defaults.rs, member.rs, new dialogs/framing_defaults.rs
- **Round 15 coverage:** framing round 14
- **Round 16 brief:** 29 framing-members-reporting
- **Why it matters for CDs:** Offices set up framing once; the Build Framing dialog is only three things in Chief (DECISIONS 112).
- **Merged from:** source gaps P4#17

### 71. Framing display and editing

- **Scope:** Framing display and editing: symbols, labels, section views, Wall Detail window and Open Wall Detail, S/E indicators, join and lap/mitre ends, end profiles, manual heights, multi-ply, Framing Specification panels
- **Size:** M | **Score:** 56 | **Rows:** 22 (8 Missing, 14 Partial) | **Parts:** 4
- **Parity ids:** CB-35, CB-40, CB-268, CB-270..286
- **Area / owner files:** editor/framing_view.rs, dialogs/framing.rs, plan-framing detail.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 30 framing-layout-trusses
- **Why it matters for CDs:** Wall details and framing sheets are CD sheets.

### 72. Wall editing tools

- **Scope:** Wall editing tools: Edit Wall Intersections, Same Wall Type handles, Align With Wall Above/Below, section top/bottom edge editing, Make Invisible/Visible, Add Wall to Library, attached cabinets move
- **Size:** M | **Score:** 56 | **Rows:** 17 (9 Missing, 8 Partial) | **Parts:** 2
- **Parity ids:** DIM-5, R-65, R-88, W-24, W-27, W-29, W-30, W-60, W-66..68, W-75, W-130, W-139, W-141..143, W-145
- **Area / owner files:** editor/wall_edit.rs, handles.rs, tools/select.rs
- **Round 15 coverage:** walls2 (partial)
- **Round 16 brief:** 14 wall-edit-tools
- **Why it matters for CDs:** Daily wall clean-up between floors and at odd corners.
- **Merged from:** source gaps P2#6

### 73. Dimension Defaults dialog structure

- **Scope:** Dimension Defaults dialog structure: per-tool Locate panels, Setup Automatic/Temporary, General (rounding, text position), Primary/Secondary Format, Extensions, Layer; Auto Story Pole defaults
- **Size:** M | **Score:** 56 | **Rows:** 15 (2 Missing, 10 Partial, 3 In progress) | **Parts:** 2, 3, 6
- **Parity ids:** DIM-6..8, DIM-46, DIM-49..51, DIM-53, DIM-55..58, S-63
- **Area / owner files:** dialogs/default_pages/dimension.rs, plan-core dimension.rs
- **Round 15 coverage:** dims2
- **Round 16 brief:** 28 dimension-segments
- **Why it matters for CDs:** Dimension setup is done once per office standard.
- **Merged from:** source gaps P2#4

### 74. Templates

- **Scope:** Templates: New Plan/Layout from Template chooser, Save as Template with purge list, per-unit-system defaults, missing-template prompt
- **Size:** M | **Score:** 56 | **Rows:** 14 (3 Missing, 11 Partial) | **Parts:** 1, 6
- **Parity ids:** APP-1, APP-2, APP-13, APP-60, APP-106, APP-126, DS-33..35, L-5, L-7, L-10, L-46, L-140, L-151, L-190
- **Area / owner files:** templates.rs, plan_defaults.rs, files.rs, new dialogs/template_chooser.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 26 saved-defaults-views
- **Why it matters for CDs:** Starting each job from the studio template with the right data purged is the first step of every project.
- **Merged from:** source gaps P1#6

### 75. Dormers: roof types

- **Scope:** Dormers: roof types (gambrel, mansard, barrel, curved eave, eyebrow), second pitch, dormer room options, wall type / Set to Existing Ceiling, framing fields, crickets, returns
- **Size:** M | **Score:** 56 | **Rows:** 8 (2 Missing, 6 Partial) | **Parts:** 4
- **Parity ids:** RF-48..52, RF-54, RF-144, RF-145
- **Area / owner files:** plan-roof dormer.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 20 roof-trim-dormers-skylights
- **Why it matters for CDs:** Cape-style and remodel dormers; crickets behind chimneys.
- **Merged from:** source gaps P4#12

### 76. Build Roof options

- **Scope:** Build Roof options: curved-wall segment angle, concave alcoves, Retain Manually Drawn/Edited planes, rebuild prompts, pitch in degrees, Roof Groups on rooms
- **Size:** M | **Score:** 56 | **Rows:** 8 (5 Missing, 3 Partial) | **Parts:** 4
- **Parity ids:** R-114, RF-9, RF-66..72
- **Area / owner files:** plan-roof staged.rs, footprint.rs, dialogs/roof.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 19 roof-baselines-groups-curved
- **Why it matters for CDs:** The roof of a remodel or addition is rarely generated from scratch (DECISIONS 118).

### 77. Special cabinets

- **Scope:** Special cabinets: end, radius end, angled front, bow front, peninsula, blind, corner conversion, islands
- **Size:** M | **Score:** 56 | **Rows:** 7 (4 Missing, 3 Partial) | **Parts:** 3
- **Parity ids:** CB-70, CB-481, CB-494, CB-495
- **Area / owner files:** plan-cabinets cabinet.rs, mesh3d.rs, tools/cabinet.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 24 cabinet-faces-specials
- **Why it matters for CDs:** Islands and peninsulas end in these shapes.
- **Merged from:** source gaps P3#13

### 78. Structural Member Reporting

- **Scope:** Structural Member Reporting (Buy List, Cut List, Linear Length, Mixed), board sizes and specifications, framing materials list categories
- **Size:** L | **Score:** 56 | **Rows:** 7 (6 Missing, 1 Partial) | **Parts:** 4
- **Parity ids:** CB-41, CB-249..253, CB-287
- **Area / owner files:** plan-framing takeoff.rs, lumber.rs, new dialogs/member_reporting.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 29 framing-members-reporting
- **Why it matters for CDs:** Lumber orders for the contractor.
- **Merged from:** source gaps P4#17

### 79. Rough Opening and Framing panels

- **Scope:** Rough Opening and Framing panels (header method, trimmers, sills, combine headers, supports)
- **Size:** M | **Score:** 56 | **Rows:** 5 (1 Missing, 4 Partial) | **Parts:** 3
- **Parity ids:** CB-502, DW-56, DW-114, DW-129, DW-134, DW-152
- **Area / owner files:** plan-core openings/spec.rs, plan-framing, dialogs/opening/tabs.rs
- **Round 15 coverage:** opening_tabs
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Rough openings drive schedules and wall-framing headers.
- **Merged from:** source gaps P3#17; earlier Top 40 #2

### 80. Find Object in Plan from schedule/materials rows, Find Schedule(s) from Object, Find Wa...

- **Scope:** Find Object in Plan from schedule/materials rows, Find Schedule(s) from Object, Find Wall from framing, Find Trusses
- **Size:** M | **Score:** 56 | **Rows:** 4 (3 Missing, 1 Partial) | **Parts:** 1
- **Parity ids:** S-118..120
- **Area / owner files:** editor/selection.rs, schedule_view.rs, framing_view.rs, new Select Location dialog
- **Round 15 coverage:** none
- **Round 16 brief:** 04 schedules
- **Why it matters for CDs:** Jump from a schedule row to the object on the plan (and back) on large sets.
- **Merged from:** source gaps P1#16, P7#14

### 81. Roof Baseline Polylines with specification, directive letters on edges, Use Existing Ro...

- **Scope:** Roof Baseline Polylines with specification, directive letters on edges, Use Existing Roof Baselines
- **Size:** L | **Score:** 56 | **Rows:** 4 (4 Missing, 0 Partial) | **Parts:** 4
- **Parity ids:** RF-62, RF-125..127
- **Area / owner files:** new plan-roof baseline.rs, tools/roof_baseline.rs, dialogs/roof_baseline.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 19 roof-baselines-groups-curved
- **Why it matters for CDs:** The tool for roofs that differ from the wall footprint (porches, additions, remodels over an existing roof).
- **Merged from:** source gaps P4#5

### 82. CAD and text on layout pages

- **Scope:** CAD and text on layout pages: Center Object, Point to Point Move, concentric copies, Selected Edge dimensions, object snaps, editable grid snap unit, text macros in view text, callout labels linked to pages
- **Size:** M | **Score:** 56 | **Rows:** 1 (1 Missing, 0 Partial) | **Parts:** 6
- **Parity ids:** L-145
- **Area / owner files:** plan-layout annot.rs, shell/layout_window.rs
- **Round 15 coverage:** callouts (linked callouts)
- **Round 16 brief:** 02 layout-boxes
- **Why it matters for CDs:** Title blocks and detail callouts are drawn on the layout.
- **Merged from:** source gaps P7#15, P7#17

### 83. Roof trim as profile-driven parts

- **Scope:** Roof trim as profile-driven parts: rafter tails, ridge caps, gutters, frieze, shadow boards, boxed and flush eaves, Trim Framing To Soffits, subfascia and lookouts
- **Size:** L | **Score:** 54 | **Rows:** 32 (11 Missing, 21 Partial) | **Parts:** 4
- **Parity ids:** RF-15, RF-27, RF-60, RF-79, RF-83..87, RF-122, RF-146..160
- **Area / owner files:** plan-3d eave.rs, plan-roof spec.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 20 roof-trim-dormers-skylights
- **Why it matters for CDs:** The eave and cornice section is a detail every set carries; today they are switches with fixed shapes.
- **Merged from:** source gaps P4#4, P7#26; earlier Top 40 #20

### 84. Import Drawing

- **Scope:** Import Drawing: DWG gap (binary files), Import Drawing Assistant pages, duplicate blocks, terrain conversion
- **Size:** L | **Score:** 54 | **Rows:** 11 (3 Missing, 8 Partial) | **Parts:** 5
- **Parity ids:** L-44, L-45, L-78..81
- **Area / owner files:** plan-import dxf/**, dialogs/import_drawing/**
- **Round 15 coverage:** dxf (in progress)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Surveyors and engineers send DWG; today only ASCII DXF moves.
- **Merged from:** source gaps P5#3

### 85. Foundation Defaults and Options

- **Scope:** Foundation Defaults and Options: type radios, hang platform, slab group, piers, garage options, rebar, foam seal, termite flashing
- **Size:** M | **Score:** 54 | **Rows:** 8 (1 Missing, 7 Partial) | **Parts:** 3
- **Parity ids:** R-31, R-62, R-127..129, R-131, R-134
- **Area / owner files:** plan-core foundation.rs, dialogs/foundation.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 17 floors-foundation-fireplace
- **Why it matters for CDs:** Engineers and builders read the materials takeoff.
- **Merged from:** source gaps P3#16

### 86. CAD Detail Specification, Plan Footprint object and specification, Automatic Truss/Wall...

- **Scope:** CAD Detail Specification, Plan Footprint object and specification, Automatic Truss/Wall Detail windows
- **Size:** M | **Score:** 54 | **Rows:** 7 (3 Missing, 4 Partial) | **Parts:** 2
- **Parity ids:** CAD-35, CAD-132, CAD-133, CAD-135, L-38, R-110
- **Area / owner files:** tools/details/**, dialogs/details/**, plan-core details.rs
- **Round 15 coverage:** cad2 (Plan Footprint)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Plot-plan building outline and typical details feed the site plan and detail sheets.
- **Merged from:** source gaps P2#19; earlier Top 40 #14

### 87. Door and window specification panels

- **Scope:** Door and window specification panels: Shape/Arch/Lites, Lintel/Sill, Treatments, Shutters, Energy Values, Manufacturer, Hardware, Panel/Louver options, Layer/Materials, Object Information
- **Size:** M | **Score:** 52 | **Rows:** 31 (6 Missing, 25 Partial) | **Parts:** 3
- **Parity ids:** CB-83, DW-30, DW-44, DW-55, DW-57, DW-79, DW-80, DW-83..86, DW-104, DW-111, DW-113, DW-115..117, DW-121..123, DW-127, DW-132, +7 more
- **Area / owner files:** dialogs/opening/tabs.rs, plan-3d windows.rs, doors.rs
- **Round 15 coverage:** opening_tabs (most tabs)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Elevation realism, energy submittals and ordering data come from these fields.
- **Merged from:** source gaps P3#28, P3#30; earlier Top 40 #18, #27, #39

### 88. Picture Box and PDF Box objects

- **Scope:** Picture Box and PDF Box objects (page range, crop, Save in Plan, Show Outline), vector PDF underlays, Export Picture options
- **Size:** L | **Score:** 52 | **Rows:** 16 (6 Missing, 10 Partial) | **Parts:** 5
- **Parity ids:** CAD-136..139, L-46, L-77
- **Area / owner files:** tools/underlay/**, dialogs/underlay.rs
- **Round 15 coverage:** none
- **Round 16 brief:** Round 17 candidate
- **Why it matters for CDs:** Client sketches, site photos, scanned plats and consultant PDFs go on the plan and on layout pages.
- **Merged from:** source gaps P5#10; earlier Top 40 #22

### 89. Copy/paste modes

- **Scope:** Copy/paste modes (paste edit buttons, Sticky Mode, Paste Hold Position), array with Alternate, copy intervals per kind, paste formats, layers and defaults recreated when pasting into another file
- **Size:** M | **Score:** 52 | **Rows:** 14 (4 Missing, 10 Partial) | **Parts:** 1, 5, 6
- **Parity ids:** CB-75, CB-576, L-43, L-51, L-78, S-81..83, S-85, S-105, S-133, S-134, S-136..139
- **Area / owner files:** editor/clipboard.rs, edit_commands.rs, dialogs/multiple_copy.rs
- **Round 15 coverage:** cad2 (Multiple Copy)
- **Round 16 brief:** 11 cad-edit-tools-area
- **Why it matters for CDs:** Reusing details, symbols and whole rooms between jobs is how remodel sets get built.
- **Merged from:** earlier Top 40 #15

### 90. Deck rooms

- **Scope:** Deck rooms: Deck and Deck Support panels, plank options, auto-regenerate framing, keep after delete, structural materials takeoff
- **Size:** M | **Score:** 52 | **Rows:** 12 (4 Missing, 3 Partial, 5 In progress) | **Parts:** 2, 4, 6
- **Parity ids:** CB-86, L-88, R-99, R-103
- **Area / owner files:** plan-framing deck.rs, plan-3d deck.rs, dialogs/room.rs
- **Round 15 coverage:** decks_chimneys
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Decks are a frequent remodel add-on.
- **Merged from:** source gaps P2#13; earlier Top 40 #16

### 91. Tread geometry

- **Scope:** Tread geometry: lock tread depth, ignore subsection boundaries, flare/curve handles, starter treads, winders rule, wrap around deck or landing
- **Size:** M | **Score:** 52 | **Rows:** 10 (6 Missing, 4 Partial) | **Parts:** 4
- **Parity ids:** CB-26, CB-143..149
- **Area / owner files:** plan-stairs layout.rs, model3d.rs
- **Round 15 coverage:** stairs2 (winders rule, curved refinements)
- **Round 16 brief:** 21 stairs-engine
- **Why it matters for CDs:** Flared and curved entry stairs on custom homes (DECISIONS 58).

### 92. Import Terrain Data and Import GPS Data assistants

- **Scope:** Import Terrain Data and Import GPS Data assistants (column order, filtering, scale, rotate north, GPX as marker/polyline/perimeter, drawing-layer conversion)
- **Size:** M | **Score:** 52 | **Rows:** 8 (6 Missing, 2 Partial) | **Parts:** 6
- **Parity ids:** CB-569..575, CB-577
- **Area / owner files:** plan-terrain import.rs, new dialogs/import_terrain.rs
- **Round 15 coverage:** none
- **Round 16 brief:** Round 17 candidate
- **Why it matters for CDs:** Survey data is how most lots begin (DECISIONS 107).
- **Merged from:** source gaps P6#15

### 93. Wall Specification panels

- **Scope:** Wall Specification panels: Structure, Foundation, Wall Types (pony/lower type), Wall Cap, Wall Covering, Rail Style / Newels / Balusters / Rails, Materials, Label, Components, Schedule
- **Size:** M | **Score:** 52 | **Rows:** 7 (1 Missing, 3 Partial, 3 In progress) | **Parts:** 2
- **Parity ids:** W-81, W-114..118, W-127, W-140
- **Area / owner files:** dialogs/wall/tabs.rs
- **Round 15 coverage:** walls2 (most tabs)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Railing and covering specs appear in permit drawings.
- **Merged from:** source gaps P2#17; earlier Top 40 #3, #17, #25

### 94. Notes linked to Note Schedules

- **Scope:** Notes linked to Note Schedules (types, renumbering, Convert Text to Note)
- **Size:** M | **Score:** 52 | **Rows:** 4 (2 Missing, 2 Partial) | **Parts:** 3
- **Parity ids:** TXT-9, TXT-54, TXT-55
- **Area / owner files:** dialogs/text/note.rs, plan-core note.rs
- **Round 15 coverage:** callouts (in progress)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** General-note sheets need numbered notes that live in a schedule.
- **Merged from:** source gaps P3#6

### 95. Tray and coffered ceilings

- **Scope:** Tray and coffered ceilings (polyline tools, nested, specification, rope lights, framing), lowered / cathedral / vaulted ceiling controls (Flat Ceiling Over This Room), shelf ceiling
- **Size:** L | **Score:** 52 | **Rows:** 4 (3 Missing, 1 Partial) | **Parts:** 2, 4
- **Parity ids:** R-32, R-111, R-112
- **Area / owner files:** plan-roof ceiling.rs, new tools/tray_ceiling.rs, dialogs/room.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 16 tray-ceilings
- **Why it matters for CDs:** Common feature in custom homes.
- **Merged from:** source gaps P2#11, P7#23

### 96. Symbol Specification depth

- **Scope:** Symbol Specification depth: 3D panel (origin, rotation), 2D Block panel, Advanced Sizing with stretch planes, Elevation References, Cut Referenced Surface, Opening and Plan View panels, Requirements, Light Data link
- **Size:** L | **Score:** 50 | **Rows:** 30 (17 Missing, 13 Partial) | **Parts:** 4
- **Parity ids:** CB-4, CB-16, CB-56, CB-60, CB-73, CB-83, CB-400..422
- **Area / owner files:** dialogs/symbol.rs, plan-library symbol.rs
- **Round 15 coverage:** none
- **Round 16 brief:** Round 17 candidate
- **Why it matters for CDs:** Custom millwork, appliances and manufacturer models.
- **Merged from:** source gaps P4#15

### 97. Define Material depth

- **Scope:** Define Material depth: Pattern tab line color/weight/contrast, ten material classes, Materials List structure type and calculation method, Stretch to Fit, bump/normal/AO maps with Invert, Clear Coat, Brushed, Water
- **Size:** M | **Score:** 50 | **Rows:** 20 (5 Missing, 15 Partial) | **Parts:** 5
- **Parity ids:** C-82, C-92, C-97..101
- **Area / owner files:** plan-materials material.rs, dialogs/materials.rs
- **Round 15 coverage:** lightbeans (PBR maps)
- **Round 16 brief:** Round 15 only
- **Why it matters for CDs:** Pattern lines set how siding and masonry read in elevations; calculation method drives takeoffs (DECISIONS 127).
- **Merged from:** source gaps P5#7

### 98. Architectural Blocks and Ganged Electrical Blocks

- **Scope:** Architectural Blocks and Ganged Electrical Blocks (make, explode, spec, library)
- **Size:** M | **Score:** 50 | **Rows:** 16 (15 Missing, 1 Partial) | **Parts:** 3, 4
- **Parity ids:** CB-426..438, E-25
- **Area / owner files:** new plan-core blocks.rs, tools, dialogs
- **Round 15 coverage:** none
- **Round 16 brief:** Round 17 candidate
- **Why it matters for CDs:** Kitchen islands and repeated unit groups move and schedule as one.
- **Merged from:** source gaps P3#21, P4#18, P7#12

### 99. Landing and ramp specification

- **Scope:** Landing and ramp specification: Auto Adjust Height/Thickness, per-edge railing, adjacent landings, Add Break, Selected Edge and Plan Display panels, Ramp Specification and slope entry
- **Size:** M | **Score:** 50 | **Rows:** 14 (6 Missing, 7 Partial, 1 In progress) | **Parts:** 4
- **Parity ids:** CB-34, CB-188..200
- **Area / owner files:** plan-stairs landing.rs, dialogs/stairs.rs
- **Round 15 coverage:** stairs2 (Ramp tool)
- **Round 16 brief:** 21 stairs-engine
- **Why it matters for CDs:** Landings and ramps in remodels (DECISIONS CB-34).
- **Merged from:** source gaps P4#8

### 100. Schedule handles and edit tools

- **Scope:** Schedule handles and edit tools: resize columns, move rows, sort prompt, Move Up/Down, Open Row Object(s), Schedule to Text, Reset Column Widths
- **Size:** M | **Score:** 50 | **Rows:** 14 (6 Missing, 8 Partial) | **Parts:** 3
- **Parity ids:** CB-68, L-25, L-27, L-28, L-63..66, L-68..70
- **Area / owner files:** editor/schedule_view.rs, tools/schedule.rs
- **Round 15 coverage:** none
- **Round 16 brief:** 04 schedules
- **Why it matters for CDs:** Fast schedule tidying on the sheet.
- **Merged from:** source gaps P3#27

## Ranks 101 to 215 (compact)

| Rank | Gap | Parity ids | Size | Area | Rows | Round 15 | Round 16 / next |
|---|---|---|---|---|---|---|---|
| 101 | Edit Layout Lines and Plot Lines | L-4, L-12, L-13, L-171..176 | L | plan-layout hatch.rs | 6 | none | 02 layout-boxes |
| 102 | Snap aids | DS-13, LAY-76, PR-3, S-92, S-122, S-125, +4 more | M | editor/snap.rs | 6 | walls2 (guides while drawing walls) | 10 edit-behaviours-keys |
| 103 | Roof plane drawing and editing | RF-6, RF-35, RF-37, RF-38, RF-59, RF-93..97, RF-99, +2 more | M | plan-roof geom.rs | 18 | roofs round 14 | 18 roof-eaves-heights |
| 104 | Drawing groups | CAD-139, CB-60, CB-269, DW-116, LAY-36, LAY-74, +3 more | M | plan-core drawing_group.rs | 12 | cad2 (Drawing Groups) | 09 layers-drawing-groups |
| 105 | Floor-level essentials | R-34, R-55, R-59, R-63, R-68, R-116..121, +1 more | S | plan-core floors.rs | 12 | none | 17 floors-foundation-fireplace |
| 106 | Library Browser search and organisation | C-60, CB-54, CB-61, CB-364..385 | M | plan-library browse.rs | 25 | library (in progress) | Round 15 only |
| 107 | Lighting | C-64..66, C-164, C-167..172 | M | plan-render lighting.rs | 19 | cameras2 (light sets) | Round 15 only |
| 108 | Painter options | C-56, CB-128, CB-489, S-38, S-116, S-183..189, +1 more | L | tools/painters.rs | 17 | painters_spell (basic painters, Match Properties) | Round 15 only |
| 109 | Library placement | CB-16, CB-55..57, CB-60, CB-393..398 | M | plan-library | 12 | library (in progress) | Round 15 only |
| 110 | Electrical connections as editable splines | E-21, E-22 | M | plan-electrical circuit.rs | 10 | none | 25 electrical-defaults-connections |
| 111 | Center Object variants, Align/Distribute dialogs, Align To Line, Framing Reference plac... | CAD-41, DW-24, S-45, S-53, S-54, S-145, S-147..150 | M | tools/cad_ops.rs | 8 | none | 11 cad-edit-tools-area |
| 112 | Room Specification panels | DW-117, R-36, R-115 | M | dialogs/room.rs | 8 | none | 15 rooms-living-area |
| 113 | Cabinet Specification panels | CB-10, CB-481, CB-496 | M | dialogs/cabinet.rs | 5 | none | 24 cabinet-faces-specials |
| 114 | Marker types | TXT-8, TXT-52, TXT-53 | M | tools/text.rs | 4 | callouts (in progress) | Round 15 only |
| 115 | Rich Text editing odds and ends | CB-60, TXT-1, TXT-2, TXT-4, TXT-10, TXT-12, TXT-13, TXT-16, +7 more | M | dialogs/text.rs | 39 | callouts (Rich Text Edit Bar) | 27 text-macros-rescheck |
| 116 | Door and window plan/3D display rules | DW-36, DW-57, DW-59, DW-81, DW-83, DW-91, +8 more | M | plan-3d casing.rs | 33 | opening_tabs (partial) | 22 openings-mulled-bay |
| 117 | Orthographic Full/Floor/Framing and Isometric overviews, Auto Elevation tools one side ... | C-3..5, C-10, C-12, C-15, C-21, C-115, +2 more | M | plan-view3d camera.rs | 17 | cameras2 (partial) | 32 camera-section-annotation |
| 118 | Selection behaviors | APP-79, C-43, CAD-16, CAD-20, S-8, S-16, +4 more | M | tools/select.rs | 17 | none | 11 cad-edit-tools-area |
| 119 | Roof/Ceiling Plane and Skylight specifications | CB-37, RF-35, RF-36, RF-88..92, RF-98, RF-101, RF-123, RF-124 | M | dialogs/roof.rs | 15 | none | 18 roof-eaves-heights |
| 120 | Fireplace foundation, dimensions in plan, rough opening, depth handle, chimney chase an... | CB-60, CB-87, CB-232, CB-500, CB-501, CB-503, DW-125 | M | plan-core fireplace.rs | 12 | decks_chimneys | 17 floors-foundation-fireplace |
| 121 | Text attached by arrows | TXT-3, TXT-5, TXT-6, TXT-39, TXT-47, TXT-48 | M | dialogs/text | 12 | callouts (Add Arrow handle) | Round 15 only |
| 122 | General Plan Defaults gaps | DS-13, DS-42..46, DS-49, DW-83, LAY-37, PR-22 | M | plan-core defaults.rs | 9 | defaults_pages | Round 15 only |
| 123 | Import 3D Symbol dialog details | CB-60, CB-82, CB-423..425 | M | dialogs/symbol/import3d.rs | 9 | import3d (in progress) | Round 15 only |
| 124 | Foundation wall footings | W-52 | M | plan-core foundation.rs | 3 | none | Round 17 candidate |
| 125 | Camera navigation | C-21, C-28, C-34..39, C-41, C-114, C-121..127, +1 more | S | shell/view3d_panel/nudge.rs | 24 | cameras2 (undo zoom) | 32 camera-section-annotation |
| 126 | Electrical tool behaviors | CB-60, CB-61, CB-66, DW-63, E-1, E-3, E-5..12, +3 more | M | plan-electrical place.rs | 21 | none | 25 electrical-defaults-connections |
| 127 | Jack, hip, girder, drop-hip, subgirder, scissors, attic and energy-heel trusses; Truss ... | CB-314..327, RF-54 | L | plan-framing truss.rs | 18 | none | Round 17 candidate |
| 128 | Wall Defaults dialogs per wall kind | R-56, R-75, R-76, R-82, R-83, W-119, W-120 | M | dialogs/default_pages/architectural.rs | 11 | defaults_pages (partial) | 12 wall-types-layers |
| 129 | General Cabinet Defaults | CB-10, CB-71, CB-476 | M | new dialogs/cabinet_defaults.rs | 6 | defaults_pages (tree) | 23 cabinet-runs-labels |
| 130 | Missing Files dialog | APP-107..109 | M | files.rs | 4 | none | Round 17 candidate |
| 131 | Typed Rotate Plan View angle saved with the view, CAD detail/footprint inheriting it, R... | LAY-17, LAY-52, LAY-53, TXT-16, TXT-34 | S | editor/render.rs rotation hook | 3 | defaults_pages (rotate/reverse shell commands) | 26 saved-defaults-views |
| 132 | Stepped, raked and double walls | R-88, R-138, W-89, W-149 | L | plan-core walls.rs | 3 | none | 14 wall-edit-tools |
| 133 | Door and window placement and edit rules | CB-60, DW-1, DW-3, DW-5, DW-8, DW-13, +22 more | M | tools/opening/** | 39 | none | 22 openings-mulled-bay |
| 134 | Road geometry and specs | CB-48, CB-578..600 | L | plan-terrain landscape.rs | 26 | none | Round 17 candidate |
| 135 | Convert tools and polyline types | CAD-28, CAD-91, CAD-92, CAD-103..106, CB-46, CB-516, DS-1 | M | tools/cad_ops.rs | 20 | none | 11 cad-edit-tools-area |
| 136 | Rendering Technique Options and Defaults dialogs | C-45, C-49, C-51, C-113, C-163, C-174 | L | plan-view3d | 20 | cameras2 (techniques) | Round 15 only |
| 137 | Wall, railing, deck and fence tool set | CB-86, CB-150, R-3, R-31, W-54..56, W-58, W-59, +5 more | M | tools/wall.rs | 20 | none | 14 wall-edit-tools |
| 138 | Stair plan display | CB-26, CB-31, CB-33, CB-34, CB-117..126 | S | plan-stairs plan.rs | 14 | stairs2 (UP/DN, break line) | 21 stairs-engine |
| 139 | Sun Angle objects | C-63, C-111, C-161, C-165, C-166, C-173 | M | plan-core camera.rs | 14 | none | Round 17 candidate |
| 140 | Terrain elevation data tools and modifiers | CB-44..46, CB-513..515, CB-547..554 | M | plan-terrain elevation.rs | 13 | none | 33 terrain-site |
| 141 | Skylight tool and specification | RF-42, RF-43, RF-45, RF-137..143 | M | plan-roof hole.rs | 10 | none | 20 roof-trim-dormers-skylights |
| 142 | User Catalog | CB-83, CB-386..392 | M | plan-library user.rs | 8 | library (in progress) | Round 15 only |
| 143 | Light Data panel | C-64, E-23 | L | plan-electrical | 6 | cameras2 (light sets) | Round 15 only |
| 144 | Generate Between Platforms short walls, railings across platform gaps, stairs snapping ... | CB-98, W-39, W-60, W-62, W-63, W-134, W-146 | M | plan-core walls.rs | 6 | walls2 (W-63) | 14 wall-edit-tools |
| 145 | 3D model export | L-18, L-20, L-55 | M | plan-3d gltf.rs + new exporters | 5 | none | Round 17 candidate |
| 146 | Trim and Extend to CAD cutters or framing, Sticky Mode, drawn cutting lines | CAD-21, CAD-95, CAD-96, S-50, S-180, S-181 | M | tools/cad_ops.rs | 5 | cad2 (Trim/Extend) | 11 cad-edit-tools-area |
| 147 | Truss Specification panels, member sizing and framing schedule | CB-41, CB-328..331 | M | dialogs/truss.rs | 5 | none | 30 framing-layout-trusses |
| 148 | Allowed angles and snap grid settings | DS-47, DS-48, PR-12, S-71, S-126 | M | dialogs/snap_settings.rs | 4 | none | 10 edit-behaviours-keys |
| 149 | Gable/Roof Line object | RF-133..136 | S | plan-roof gable.rs | 4 | none | 20 roof-trim-dormers-skylights |
| 150 | Export Drawing options dialog | L-44, L-198 | M | dialogs/dxf_options.rs | 3 | cad2 (DXF options) | Round 15 only |
| 151 | Terrain feature shapes and click-once placement | CB-44, CB-46, CB-48, CB-49, CB-517..520, CB-555..568 | M | plan-terrain landscape.rs | 23 | none | 33 terrain-site |
| 152 | Line and Arc Specification | CAD-7, CAD-60, CAD-116, CAD-117, CAD-119, CAD-121, W-64, +1 more | M | dialogs/cad.rs | 19 | cad2 (partial) | 06 survey-entry |
| 153 | Preferences panels | APP-39, PR-3..7, PR-10, PR-11, PR-13, PR-14, PR-17, PR-19..21, +4 more | L | dialogs/preferences/pages.rs | 19 | none | Round 17 candidate |
| 154 | 3D solid tools | CB-72, CB-439..449, CB-451, S-182 | L | tools/cad_ops.rs | 14 | none | Round 17 candidate |
| 155 | CAD blocks | CAD-31..34, CAD-37, CAD-128..131, DIM-69, L-44 | M | dialogs/cad/blocks.rs | 11 | none | Round 17 candidate |
| 156 | Make Parallel/Perpendicular with Rotate Edge/Polyline, Set Angular Dimension, Reflect A... | CAD-24, CAD-54, CAD-98, S-152..155, S-158 | M | editor/transform.rs | 8 | none | 11 cad-edit-tools-area |
| 157 | Walkthrough completeness | C-71, C-116, C-176..181 | M | plan-view3d walkthrough.rs | 30 | cameras2 (video) | Round 15 only |
| 158 | 3D Cladding | C-73, C-104..107 | L | plan-3d | 11 | none | Round 17 candidate |
| 159 | Corner Board and Quoin field sets | CB-332..337, W-109, W-110 | S | plan-core details.rs | 9 | none | 31 moldings-trim |
| 160 | Image objects and Image Specification | NO SPEC ids (page-level rows) | M | tools/images.rs | 7 | details round 14 | Round 15 only |
| 161 | Edit handle sets and Edit-toolbar buttons per object family | APP-73, CAD-9, CAD-16, S-17, S-18, S-24, S-59, +2 more | M | editor/handles.rs | 6 | none | 11 cad-edit-tools-area |
| 162 | Find/Replace Text scopes and options, Replace Fonts prompt | TXT-12, TXT-26, TXT-45, TXT-46 | S | dialogs/find_replace.rs | 4 | none | 27 text-macros-rescheck |
| 163 | Curved roof planes | RF-61 | L | plan-roof geom.rs | 4 | none | 19 roof-baselines-groups-curved |
| 164 | Stairwell as an Open Below room, Auto Stairwell, Max Tread Contraction for rooms under ... | CB-29, CB-30, CB-157 | S | plan-stairs | 4 | stairs2 | 21 stairs-engine |
| 165 | Wall system display layers | LAY-76 | M | plan-core layers | 1 | none | 09 layers-drawing-groups |
| 166 | Project Browser parity | APP-35, APP-110..113, APP-115..121, APP-123 | M | shell/docks.rs | 13 | none | Round 17 candidate |
| 167 | Ovals/ellipses, boxes, regular polygon, true splines, polyline specification panels, Re... | CAD-11, CAD-12, CAD-29, CAD-30, CAD-111, CAD-120, CAD-122..126 | M | dialogs/cad.rs | 12 | none | Round 17 candidate |
| 168 | Tool Search box and Child Tool Palette | APP-36, APP-81, TB-1, TB-2, TB-7, TB-8 | M | toolbar.rs | 9 | none | Round 17 candidate |
| 169 | Roof defaults dialogs | RF-1, RF-2, RF-5, RF-63..65 | S | dialogs/roof.rs | 6 | defaults_pages | Round 15 only |
| 170 | CAD Defaults dialog per view | CAD-1, CAD-107, CAD-110 | M | dialogs/default_pages/cad.rs | 4 | defaults_pages (CAD pages, 0 bound fields) | Round 15 only |
| 171 | Line weight printing | L-12, L-212, L-213 | S | plan-layout print.rs | 3 | none | 03 print-sheet-watermark |
| 172 | Soffit Specification fields and primitive solid specification panels | CB-7, CB-17, CB-60, CB-452..461 | S | dialogs | 14 | none | Round 17 candidate |
| 173 | Material regions and custom backsplash | CB-15, R-72 | S | tools | 3 | none | Round 17 candidate |
| 174 | Camera symbol display and 3D working | C-24..28, C-30, C-43, C-47, C-69, C-70, C-112, +6 more | M | shell/view3d_panel | 15 | none | Round 17 candidate |
| 175 | Dialog conventions | APP-82, APP-83, APP-85..89, PR-18, W-83 | M | dialogs.rs | 13 | none | Round 17 candidate |
| 176 | Backdrops | C-70, C-154, C-155, C-175 | M | plan-view3d backdrop.rs | 9 | none | Round 17 candidate |
| 177 | Delete Objects scopes | DIM-60, LAY-26, S-80, S-87, S-88, S-190, S-192, S-193 | S | dialogs/delete_objects.rs | 9 | none | 11 cad-edit-tools-area |
| 178 | Ray-traced view controls | C-51, C-65, C-148, C-159, C-160, C-162 | M | plan-render | 8 | cameras2 | Round 15 only |
| 179 | Move rules | LAY-60, S-73, S-92, S-140, S-141, S-143, S-144, S-151 | M | editor/ops.rs | 7 | none | 11 cad-edit-tools-area |
| 180 | Slab Specification, Create Hole, piers and pads specification | R-85, R-132, R-141, R-142 | S | plan-core foundation.rs | 7 | none | 17 floors-foundation-fireplace |
| 181 | Cabinet editing | CB-8, CB-14, CB-478, CB-490 | S | editor/handles.rs | 5 | none | 24 cabinet-faces-specials |
| 182 | Camera defaults dialogs | C-47, C-68, C-69, C-108, C-110, C-112 | S | dialogs/default_pages/camera.rs | 5 | defaults_pages (Camera Tools pages) | Round 15 only |
| 183 | Wall labels, wall schedule categories and display of wall layers in sections | W-93 | S | plan-core walls.rs | 2 | none | Round 17 candidate |
| 184 | Archive naming | APP-18, APP-127..130, PR-9 | M | files.rs | 5 | none | Round 17 candidate |
| 185 | Stair library round trip | CB-58, CB-102, CB-127, CB-129, CB-130, CB-450 | S | plan-stairs | 14 | none | 21 stairs-engine |
| 186 | Ceiling Planes join and display; mixed truss/stick framing order; purlins | CB-201, RF-54, RF-128..132 | S | plan-roof ceiling.rs | 7 | none | Round 17 candidate |
| 187 | Room Boxes | R-79, R-80, R-91..95, R-97 | M | plan-spaceplan | 7 | none | Round 17 candidate |
| 188 | Distributed Objects and path/region fills | CB-470..474 | S | tools | 5 | none | Round 17 candidate |
| 189 | Material Builder with Masonry and Stone, Tile and Wood builders; Pattern from Texture | C-96, C-102, C-103 | L | plan-materials | 9 | none | Round 17 candidate |
| 190 | Saving and exporting 3D views | C-77, C-145, L-49, L-76, L-82 | S | dialogs/export_picture.rs | 9 | import3d | Round 15 only |
| 191 | Arrowheads | CAD-6, CAD-115, CAD-127 | S | plan-core cad.rs | 5 | callouts (arrow handle) | Round 15 only |
| 192 | Text Style Defaults and the Text Style panel in object dialogs | TXT-17, TXT-57, TXT-58 | S | plan-core text_styles.rs | 3 | none | 27 text-macros-rescheck |
| 193 | Plants as Library images with Plant Image Specification, richer Plant Chooser | CB-49, CB-50, CB-73, CB-601..603, CB-605..607, CB-609..626, L-23 | L | plan-terrain symbols.rs | 25 | none | Round 17 candidate |
| 194 | Window system | APP-8, APP-37, APP-43, APP-66, APP-76..78, APP-80, +1 more | M | main.rs | 13 | none | Round 17 candidate |
| 195 | Mouse buttons, trackpad pinch/pan, five-button shortcuts, cursor badges and crosshair a... | APP-69..75, C-36, CB-43, LAY-38 | S | main.rs input | 9 | none | Round 17 candidate |
| 196 | Text, callout and marker miscellany | DIM-4 | S | dialogs/text | 5 | none | Round 17 candidate |
| 197 | Time Tracker with Time Log, CSV export, preferences | APP-29, APP-131..133 | S | new dialog | 5 | none | Round 17 candidate |
| 198 | Projects | APP-18, APP-19, APP-93..102 | M | files.rs | 10 | (Differs by design) | Round 15 only |
| 199 | Toolbar configuration management | TB-2..4, TB-7, TB-9..14 | M | dialogs/customize_toolbars.rs | 8 | none | Round 17 candidate |
| 200 | Start window | APP-6, APP-67, APP-68 | S | new dialogs/dashboard.rs | 3 | none | Round 17 candidate |
| 201 | Creating-object miscellany | NO SPEC ids (page-level rows) | S | main.rs | 1 | none | Round 17 candidate |
| 202 | Status bar content options, toast notifications, 'remember my choice' message boxes | APP-90, APP-91, PR-8, PR-29, S-6, S-98 | S | shell/status.rs | 7 | none | Round 17 candidate |
| 203 | Open dialog memory, recent-document pins, Save Entire Project, thumbnails | APP-1, APP-3, APP-5, APP-11, APP-14, APP-105, +3 more | S | files.rs | 6 | none | Round 17 candidate |
| 204 | Hotkey gaps | HK-6, HK-8..11, S-9, S-10 | S | shell/hotkeys.rs | 4 | none | Round 17 candidate |
| 205 | Angle Snap Grid display | LAY-23 | S | editor/render.rs | 2 | none | Round 17 candidate |
| 206 | Per-view Color toggle with grayscale / black-and-white 'Color Off Is' | LAY-19, LAY-75 | S | editor/render.rs | 1 | none | Round 17 candidate |
| 207 | Coordinate System Indicators | LAY-21, LAY-23, LAY-48 | S | editor/render.rs | 5 | none | Round 17 candidate |
| 208 | View navigation | APP-140, C-36, LAY-59, LAY-61, S-99 | S | main.rs | 3 | none | Round 17 candidate |
| 209 | Region-driven number formats | DS-25 | S | plan-core units.rs | 1 | none | Round 17 candidate |
| 210 | Color Chooser | APP-33, CAD-87, CAD-88 | S | dialogs | 2 | none | Round 17 candidate |
| 211 | Program overview, dialog and file-management miscellany | NO SPEC ids (page-level rows) | S | main.rs | 1 | none | Round 17 candidate |
| 212 | Legacy content migration and X18 what's-new rows | APP-143, APP-144, L-50, L-228 | S | plan-calib | 9 | none | Round 17 candidate |
| 213 | Convenience dialogs | APP-28, APP-30, APP-134..136, APP-141, APP-142 | S | new dialogs | 7 | (Deferred) | Round 15 only |
| 214 | Asset Management dialog and asset export | APP-103, APP-104, APP-114 | S | new dialog | 3 | (Differs by design) | Round 15 only |
| 215 | File association preference | APP-92 | S | packaging | 1 | none | Round 17 candidate |

## The earlier audit's Top 40, cross-referenced

`docs/chief-feature-coverage.md` ranked 40 features on 2026-10-08 before Round 14 landed. Each is mapped to the entries above; the third column says where it stands now (from the manual audits' reading of the tree, which supersedes the earlier statuses). Items marked Round 15 are in flight: confirm them at the gate rather than trusting this table.

| # | Earlier Top 40 feature | Master rank(s) | Where it stands |
|---|---|---|---|
| 1 | Edit Area and Stretch CAD: rubber-band a region and move, rotate or copy everything in it | 69 | Partial after Round 14; Edit Area (all floors) and Stretch rows remain |
| 2 | Door and window Rough Opening and Framing tabs: rough size, header height, clearance gaps, header and trimmer construction | 79 | Round 15 opening_tabs builds Rough Opening and Framing tabs |
| 3 | Wall Specification Structure and Foundation tabs: through walls, platform intersections, balloon walls, footing and stem wall | 93, 144 | Round 15 walls2 builds the tabs; Generate Between Platforms moves to brief 14 |
| 4 | Fill Window Building Only and rubber-band Zoom | 131 | Fill Window Building Only done (DECISIONS DS5); rubber-band Zoom and Zoom Previous in Round 15 defaults_pages; typed rotation in brief 26 |
| 5 | Wall drawing gestures: right-click and drag-release keep the chain, dashed alignment guides to other endpoints | 37 | Round 15 walls2 |
| 6 | Build Foundation types, basement and crawl space rooms, attic floor from Build Roof | 63, 105 | Round 14 landed Build Foundation types; step markers and options in brief 17 |
| 7 | Export Picture of any view (live 3D, layout page, plan) as PNG or JPEG | 190 | Done in Round 15 import3d (Export Picture); only option polish remains |
| 8 | Cabinet Specification depth: Front tree, Door/Drawer styles from the library, Moldings, soffit from a polyline | 54, 113, 55 | Round 14 built the Front tree; depth in briefs 23 and 24 |
| 9 | Stairs: guard rails around the stairwell, Stair Specification tabs, break line and UP/DN symbol, flared and curved runs | 61, 48, 138 | Round 14 built guard rails and tabs; Round 15 stairs2; remaining in brief 21 |
| 10 | Room finish surfaces: rough ceiling, finish thickness, moldings and materials per room, label style | 112, 6 | Round 14 rooms; layered finishes in brief 13 |
| 11 | Text box: wrap width, handles, alignment, border and background, rich text | 115 | Round 14 text builder done; leftovers in brief 27 |
| 12 | Dimension Specification: format tabs, multi-point strings as one object, Align/Distribute, default sets | 11 | Round 14 and Round 15 dims2; string-as-one-object in brief 28 |
| 13 | Schedules in the DXF and construction-set PDF, custom schedule builder, Schedule tab data and component quantities | 17, 10 | Round 14 layout builder; table quality in brief 04 |
| 14 | Auto Detail, CAD Detail From View and CAD Detail Management | 86 | Round 14 details builder; Plan Footprint in Round 15 cad2 |
| 15 | Open several plans at once and paste between plans (keeping layers) | 194, 89 | Round 14 added multi-plan; paste modes in brief 11, tab management in Round 17 |
| 16 | Deck framing and decking boards from the deck outline | 90 | Round 15 decks_chimneys |
| 17 | Wall Covering on walls and rooms (wainscot, tile, paneling) | 93 | Round 15 walls2 (Wall Covering tab) |
| 18 | Window Shape tab: raked sides, angled top corners, custom heights | 87 | Round 15 opening_tabs (Shape tab) |
| 19 | Bay, box and bow windows with their roofs, depth and seat | 32 | Brief 22 |
| 20 | Roof finishing: Auto Roof Return, plane handles, truss types and web layout | 3, 83 | Briefs 18 and 20 |
| 21 | Chimneys and fireplaces that run through floors, roof and cap | 120 | Round 15 decks_chimneys; foundation and facing rule in brief 17 |
| 22 | DXF/DWG export options (version, units, blocks, selection) and PDF underlay with calibration | 150, 88 | Round 15 cad2 (DXF options) and dxf; PDF/Picture Box in Round 17 |
| 23 | Select feedback: hover description, selection count and Z in the status bar, host wall highlight | 118, 202 | Brief 11 (selection) and Round 17 (status bar) |
| 24 | Camera View Options, View Direction snaps, light sets (Day, Evening, Interior) | 59, 107 | Round 15 cameras2 (light sets); camera panels in brief 32 |
| 25 | Railing walls: Wall Cap, Newels/Balusters and Rails tabs | 93 | Round 15 walls2 (Newels/Balusters/Rails tabs) |
| 26 | Import SketchUp, 3DS, COLLADA and STL models into the library | 123 | Done in Round 15 import3d (SKP stays out of scope) |
| 27 | Door and window Materials and Layer tabs | 87 | Round 15 opening_tabs (Materials and Layer tabs) |
| 28 | Layer Painter / Layer Eyedropper and Object Painter | 108 | Round 15 painters_spell; options and Style Palettes in Round 17 |
| 29 | Reverse Plan and Rotate Plan View | 131 | Round 15 defaults_pages (Reverse/Rotate); typed angle and saved rotation in brief 26 |
| 30 | Split-level floors on one story | 105 | Round 15 decks_chimneys (split level); Build New Floor options in brief 17 |
| 31 | Boolean polyline operations: union, subtract, intersect | 66 | Round 15 cad2 (Boolean) |
| 32 | Tile windows, swap views, tab cycling | 194 | Round 15 defaults_pages (Tile, Swap, Tab) |
| 33 | Drawing Groups: line-weight groups mapped to object types | 104 | Round 15 cad2; defaults and tools in brief 09 |
| 34 | Spell check for text boxes, notes and labels | 115 | Round 15 painters_spell (spell check) |
| 35 | Default Settings tree completed: CAD, Stairs, Slab, Foundation, Image, Corner Trim, 3D Solid pages | 122, 21 | Round 15 defaults_pages (tree); General Plan Defaults leftovers in Round 17 |
| 36 | 360 panorama and walkthrough video file export | 157, 136 | Round 15 cameras2 (panorama, video) |
| 37 | Adjust 3D Cladding: siding and brick start point and course offset | 158 | Round 17 |
| 38 | Structural calculators for beams, headers and deck materials | 24 | Round 15 nkba (calculators, NKBA rules); REScheck export in brief 27 |
| 39 | Window treatments: curtains, blinds and exterior millwork on windows | 87 | Round 15 opening_tabs (Treatments tab) |
| 40 | Object Information tabs (code, comment, manufacturer, supplier) on walls, doors and windows | 43 | Brief 05 |

## Where each part's ranked gaps landed

Every ranked gap that parts 1 to 7 printed is mapped to one or two entries above (an entry can absorb several gaps; a gap that spans two topics maps to both). This is the audit trail for the deduplication.

### Part 1

| Source # | Source gap (shortened) | Master rank(s) |
|---|---|---|
| 1 | Construction Lines (tool, infinite lines, callouts, ordering rule sets, specification dialog; usable | 8 |
| 2 | Reference Display with several rows, another plan file, layer set per row, Details and XOR, offsets/ | 44 |
| 3 | Line styles as a library (Line Style Management and Specification: dash, dot and text components; Im | 35 |
| 4 | Layer management: New, Copy, Merge, Delete, Delete Unused, Reset Names, Fill column; Layer Set Defau | 14 |
| 5 | Fill Styles: one Fill Style panel (pattern list, scale, offsets, angle, color source, background, gr | 40, 23 |
| 6 | Import Settings from Plan/Layout, Save as Template with purge list, New Plan from Template chooser,  | 58, 74 |
| 7 | Edit behaviors as Chief defines them: Alternate (continuous drawing, angle-keeping reshape), Move, t | 5, 12 |
| 8 | CAD edit tools: Intersect/Join Two Lines, Close Polyline, Simplify Polyline, Complete Break, Fillet  | 66, 135 |
| 9 | Math in number fields (+ - * /) in every length box | 19 |
| 10 | Preferences General, File Management, New Plans, Coordinate System, Dialogs/Side Windows, Project Br | 153 |
| 11 | General Plan Defaults: Warn Before Deleting, Ignore Casing, Show Pitch as Degrees, Arrow-Key Scroll  | 122 |
| 12 | Elevation References (Absolute, From Floor, Finished Floor, Terrain, Ceiling, Roof) on objects | 18 |
| 13 | Style Palettes and Style Painter | 108 |
| 14 | Room Boxes saved with the plan, Room Box tools per room type, overlap tools, Room Box Specification | 187 |
| 15 | Saved Defaults for all annotation kinds, Active Defaults dialog, activating a Default Set from a too | 21, 36 |
| 16 | Project Browser: filter and sort, Advanced Search, Details/Notes, previews, new view folders, Find i | 166, 80 |
| 17 | Missing Files dialog (with Replace, Search Directory) and Asset handling | 130, 214 |
| 18 | Edit Area (All Floors), Edit Area Polyline, Place at Allowed Angles | 69 |
| 19 | Number/Angle Style dialog with quadrant and azimuth bearings; custom unit conversions | 29 |
| 20 | Plan View Specification gaps (remember zoom/rotation, show color, watermark, link to layout, poché,  | 16, 131 |
| 21 | Time Tracker with Time Log (CSV export), preferences | 197 |
| 22 | Tool Search box, Tool Palette (child tools), Scrollbars, Coordinate System Indicators, Angle Snap Gr | 168, 207, 205 |
| 23 | Delete Objects: room scopes, grouped categories, layout version, locked-layer rule | 177 |
| 24 | Dashboard, Loan Calculator, Plan Database, Screen capture | 200, 213 |
| 25 | Project-store features (projects, folders, caproj export/import, Asset Management dialog) | 198, 214 |

### Part 2

| Source # | Source gap (shortened) | Master rank(s) |
|---|---|---|
| 1 | Wall Type Definitions depth: per-layer fill/pattern columns, multiple main layers, Dimension Layer,  | 4 |
| 2 | Show Length / Show Angle on CAD lines with bearing input and quadrant/azimuth display (CAD Defaults  | 2 |
| 3 | Dimension Line Specification and handles: Segments panel with leading/trailing text, label move/rota | 11 |
| 4 | Dimension Defaults panel structure: per-tool Locate panels, Setup Temporary, General (rounding, text | 73 |
| 5 | Auto Elevation and Auto Story Pole dimensions with elevation markers and a grade reference | 26 |
| 6 | Wall edit tools: Edit Wall Intersections, Auto Connect lock, Align With Wall Above/Below, Connect Wa | 72 |
| 7 | Off-angle and unconnected wall caution symbols with Fix Off Angle dialog and Reset Notification Icon | 49 |
| 8 | General Wall Defaults dialog (Resize About, Auto Reverse Layers, Auto Merge Collinear) and the missi | 128 |
| 9 | Exterior Room object with Living Area label per structure and Make Living Area / Room Polyline | 65, 42 |
| 10 | Room Type Defaults dialog with the full Room Specification panels per type | 67 |
| 11 | Tray and coffered ceiling polylines with specification and framing | 95 |
| 12 | Room function set (Balcony, Court, Slab, Attic) and function-driven behaviour | 67 |
| 13 | Deck Specification fidelity: Deck Support panel, plank options, auto-regenerate, keep-after-delete,  | 90 |
| 14 | Stepped and raked walls, edge breaks and height handles in sections and 3D | 132 |
| 15 | Revision Cloud specification, saved defaults and click-an-object clouds | 167, 21 |
| 16 | Wall roof directive extras: Include Frieze, auto end truss above, combine with above wall, lower typ | 28 |
| 17 | Railing Rail Style / Newels / Balusters / Rails fields, Move Newels, glass panel above half wall | 93 |
| 18 | Polygon Shaped Room and the regular polygon dialog; box and oval/ellipse as true objects with specif | 137, 167 |
| 19 | Plan Footprint object with specification and CAD detail insertion | 86 |
| 20 | CAD block specification (By Block/By Object, size factors), Used marks, Purge, library blocks with l | 155 |
| 21 | Calculate Materials in Room, Create Schedule from Room, Create Room Elevation Views buttons | 65 |
| 22 | Point tools: Current Point, Move Point dialog, New CAD Point/Line/Arc dialogs | 2 |
| 23 | Wall Layer Specification dialog (role, framing assembly, cladding) | 4 |
| 24 | Wall hatching: partial length, overlap warnings, Fill Style and Label tabs, Wall Hatching defaults | 137 |
| 25 | Polyline holes, Hide/Show Selected Edge, Disconnect Edges, Same Line Type handles (edit rules live i | 66, 152 |
| 26 | Double walls (Frame Through, Split Framing, Furred Wall) | 132 |
| 27 | Brick ledges under masonry veneer | 132 |
| 28 | Dimension rounding method and rounded-value indicators | 11 |
| 29 | Dimensions in camera views and overviews | 26, 7 |
| 30 | Library walls and Add to Library for walls | 4 |
| 31 | Partition Wall type flag for glass shower walls | 4 |
| 32 | Dimension Line Separation Snaps preference and Delete Dimensions tool | 11, 26 |
| 33 | Auto Reverse Wall Layers / interior-exterior by position | 49 |
| 34 | Hide/Show wall system layers (Walls, Layers; Main Layer Only; Through Wall Lines; Footings; Brick Le | 165 |
| 35 | Point to Point dimension point markers; Set Angular Dimension; Move Left/Right End buttons | 26, 11 |

### Part 3

| Source # | Source gap (shortened) | Master rank(s) |
|---|---|---|
| 1 | Door, window and room-finish schedules at construction-document quality | 17, 10 |
| 2 | Callouts as detail and section references (cross section line, arrows, ten shapes, Callout Specifica | 9 |
| 3 | Linked callouts that report the target view name and sheet | 9 |
| 4 | Marker types: Level Line, Test Boring, Point, Marker Specification | 114 |
| 5 | Rich Text editing: Edit Bar, bullets and numbering, hyperlinks | 115 |
| 6 | Notes linked to Note Schedules | 94 |
| 7 | Automatic mulling, Minimum Separation, Window Levels and a Mulled Unit dialog | 22 |
| 8 | Door and window defaults per type, with dynamic Use Default | 51 |
| 9 | Bay, box and bow windows as wall-section units (dialog, depth handle, roof options, foundation, expl | 32 |
| 10 | Cabinet automatic fillers, merging, module lines and the General Cabinet Defaults | 31 |
| 11 | Cabinet automatic label format (B24, 3DB24, SB24R) and schedule categories | 50 |
| 12 | Cabinet face item types, per-item dialogs and shelf specification | 54 |
| 13 | Cabinet special shapes: end, radius, angled front, bow front, peninsula | 77 |
| 14 | Custom Countertop Specification, Add/Remove Waterfall, molding polylines | 55 |
| 15 | Foundation drawing aids: S step markers, stepped footings, Auto Rebuild, garage curbs and door cutou | 63 |
| 16 | Foundation Options: rebar, foam seal, termite flashing and Foundation Defaults panels | 85 |
| 17 | Opening Framing panel: header method, placement, combine headers, supports, sills | 79 |
| 18 | Electrical Defaults: library objects per tool, cabinet and counter heights, Use Default Heights | 57 |
| 19 | Electrical connections as splines with vertices, handle and defaults; rope lights as paths | 110 |
| 20 | Light Data panel (point/spot, lumens, colour, offsets, shadows) | 143 |
| 21 | Ganged electrical blocks | 98 |
| 22 | Text attached by arrows (Add arrow handle, attachment, Auto Position) | 121 |
| 23 | Saved defaults for text tools wired to the tools, Text defaults page, double-click to edit defaults | 21 |
| 24 | Text editing odds and ends: resize behaviours, convert Text/Rich Text, edit toolbar, tab columns, pa | 115 |
| 25 | Find/Replace scopes and options; Replace Fonts prompt | 162 |
| 26 | Project Information owners and use of its values as macros in any text | 68 |
| 27 | Schedule handles and edit tools (resize columns, move rows, sort, Open Row Object, Schedule to Text, | 100 |
| 28 | Door and window hardware, arch options, sill and lintel library fields, shutter fields | 87 |
| 29 | Vector View opening indicators and Opaque Glass | 116 |
| 30 | Manufacturer panel on doors, windows and cabinets | 87 |
| 31 | Gable Over Door/Window | 149 |
| 32 | Custom muntins from CAD blocks | 116 |
| 33 | Fireplace foundation, dimensions, rough opening and depth handle | 120 |

### Part 4

| Source # | Source gap (shortened) | Master rank(s) |
|---|---|---|
| 1 | Roof eave alignment and plane height controls: Same Roof Height at Exterior Walls, Same Height Eaves | 3 |
| 2 | Staircase engine: sections and subsections, Complete Break, Disconnect Subsection, merging, the sect | 15 |
| 3 | Staircase Specification panels: Stringers panel (custom stringers, Trim Against Wall, Extend Stringe | 48 |
| 4 | Roof trim as profile-driven parts: Rafter Tails, Ridge Caps, Gutter, Frieze and Shadow Boards panels | 83 |
| 5 | Roof Baseline Polylines with their specification dialog, Use Existing Roof Baselines, Retain Manuall | 81 |
| 6 | Roof truss layout for the truss manufacturer: TR-X labels shared per configuration, Truss Detail, tr | 46 |
| 7 | Stair drawing behaviour: wall and Reference Display snaps with Ctrl override, Alt to reverse the arr | 62 |
| 8 | Landings: Auto Adjust Height and Thickness, per-edge railing choice, adjacent landings, short-edge r | 99 |
| 9 | Material Layers Definition dialogs for floor, ceiling, roof and backsplash assemblies (Role, Fill fo | 6 |
| 10 | Roof Plane, Ceiling Plane and Skylight specifications: Line Style, Fill Style, Arrow, Polyline areas | 119 |
| 11 | Floor-level essentials: Build New Floor options (Move Highest Floor's Roof Up, Step floor/ceiling el | 105 |
| 12 | Dormers: roof types (Gambrel, Mansard, Barrel, Curved Eave, Eyebrow), second pitch, Dormer Room opti | 75 |
| 13 | Framing details: lap or butt over supports, blocking styles, rim joist options, wall connection styl | 60, 47 |
| 14 | Library Browser search and organisation: tags, saved and advanced filters, Boolean and phrase search | 106 |
| 15 | Symbol Specification depth: 3D panel (origin, rotation, faces), 2D Block panel, Advanced Sizing with | 96 |
| 16 | 3D model export: 3DS, COLLADA, STL and DXF/DWG 3D | 145 |
| 17 | Framing Member Defaults, Framing Types and Structural Member Reporting (Buy List, Cut List, Linear L | 70, 78 |
| 18 | Architectural Blocks and Ganged Electrical Blocks | 98 |
| 19 | Moldings system: shared Moldings/Profiles/Rails panel with offsets and stacking, profiles from close | 38 |
| 20 | 3D solid edit tools: Union, Intersection, Subtract on solids, Extrude, Revolve, Fillet and Chamfer e | 154 |
| 21 | Soffit Specification fields (Sloped, Corner Type, Place Under Ceiling or Roof, Ignore Room Moldings, | 172 |
| 22 | Stair library round trip: add a stair or ramp to the library and draw from it; Match Properties for  | 185 |
| 23 | Corner Board and Quoin field sets (Set Top/Bottom, recess to sheathing, gap, uniform and mirrored st | 159 |
| 24 | Curved roof planes and curved walls roof controls (segment angle, minimum alcove, Show All Ridges) | 163 |
| 25 | Distributed Objects and Material Region polish (Auto Spacing, display switches, click-fill in elevat | 188, 173 |

### Part 5

| Source # | Source gap (shortened) | Master rank(s) |
|---|---|---|
| 1 | Draw text, CAD lines and dimensions on a cross section or elevation view and save them with the view | 7 |
| 2 | Scene clipping for sections: Clip Sides, Clip Elevation, Clip Lines, stepped cutting planes, Clip to | 13 |
| 3 | DWG import and export, AutoCAD versions, binary DXF, Import Drawing Assistant (joined lines, hatch,  | 84 |
| 4 | Plan Materials dialog, Select Material dialog and plan-specific material definitions that travel wit | 53 |
| 5 | Camera and section specification panels: Plan Display (placement, line style and weight, arrow, symb | 59 |
| 6 | Thermal Envelope Data (CSV) and REScheck (.rxl) export | 24 |
| 7 | Material definition depth: Pattern tab line color/weight/shading contrast, Materials List structure  | 97 |
| 8 | Rendering Technique Options and Defaults (Vector View shadow intensity and shading contrast, Technic | 136 |
| 9 | Hide Camera-Facing Exterior Walls | 59 |
| 10 | Picture Box and PDF Box objects (import with page range, cropping, Save in Plan, Show Outline) and v | 88 |
| 11 | Orthographic Full/Floor/Framing Overviews and Isometric Overviews | 117 |
| 12 | Sun Angle objects with date, time, place and plan shadow polylines, North Pointer driving the sun, S | 139 |
| 13 | Annotations in camera views (Text, Leader Line, Note, dimensions on a drawing surface) | 7 |
| 14 | Walkthrough completeness: Key Frame fields (time, speed, sun, pause, floor), paths across stairs, or | 157 |
| 15 | Camera specification rendering options: Reflections, Animate Water, Light Bloom, Ambient Occlusion a | 59 |
| 16 | Incremental Move Distance and Rotate Angle per camera; Mouse-Dolly, Mouse-Tilt, Focus on Object; Vie | 125 |
| 17 | Cross Section Lines and Point Markers; Depth Cue for sections and elevations | 13 |
| 18 | Spot lights, Light Specification, brightness per light set, Adjust Lights columns, Create/Edit Light | 107 |
| 19 | 3D Cladding (siding and roofing with real depth) and Adjust 3D Cladding | 158 |
| 20 | Material Builder with Masonry and Stone, Tile and Wood builders; Pattern from Texture; Convert Textu | 189 |
| 21 | Screen Capture tools and Capture View to Clipboard | 213 |
| 22 | Backdrop import, folder import, HDR backdrops, Generated Sky, spherical panoramic backdrops, Rotate  | 176 |
| 23 | Export Picture options (active window size, units, open in viewer, remembered settings), WebP and HD | 190 |
| 24 | 3D View Defaults options (camera bumps off walls, auto turn, display of active cameras and openings, | 182 |

### Part 6

| Source # | Source gap (shortened) | Master rank(s) |
|---|---|---|
| 1 | Clear Terrain erases the perimeter, data and features instead of only the generated contours (main.r | 25 |
| 2 | Page Information on layout pages: Label with a # pattern (A-#), Description, Comments, Include in La | 1 |
| 3 | Per-page revisions with Revised By and Date, Add Layout Revision, and a placeable page-specific Layo | 1 |
| 4 | Watermark: View > Watermark, Watermark Defaults (text or image, tile or border, angle, transparency, | 33 |
| 5 | Terrain Specification parity: Absolute Elevation (Automatic, Retain Surface At Reference Point or Co | 30 |
| 6 | Retaining Wall tools (break plus wall sized from the terrain on both sides), 5 ft default terrain wa | 45 |
| 7 | Print dialog: Print Source (Drawing Sheet or Current View), Check Plot at a fraction, Collate, Scale | 39, 56 |
| 8 | Layout Box Specification: Fill Style and Label panels, box labels with macros and callout or marker  | 20 |
| 9 | Contour presentation: primary and secondary layers, Offset, label units, Highlight Negative Elevatio | 64 |
| 10 | Terrain and road schedule categories (Terrain Perimeter, Roads, Driveways, Medians, Road Markings, T | 64 |
| 11 | Materials List columns and Specification: Floor, Label, Supplier, Manufacturer, Code, Extra, Markup, | 52 |
| 12 | Materials List export formats and options: TXT, XML spreadsheet, HTML, header and hidden-column opti | 52 |
| 13 | Materials List scopes: Calculate From Selection, in Room on the room's edit toolbar, Materials List  | 52 |
| 14 | Components and Object Information panels on every object that has them in Chief (Code, Comment, Manu | 43 |
| 15 | Import Terrain and GPS assistants: column order, point filtering, range limits, units, scale, rotate | 92 |
| 16 | Terrain feature shapes and click-once placement: Round Garden Bed, Grass Region, Pond and Stepping S | 151 |
| 17 | Road geometry: flare at intersections, Polyline Road, Driveway and Sidewalk shapes, Median, Cul-de-s | 134 |
| 18 | Edit Layout Lines and Plot Lines (camera views as vector lines with editable edge and pattern lines) | 101 |
| 19 | Default Settings groups for Roads, Sidewalks and Driveways, Watermark, Layout Box, Materials List Po | 134, 33 |
| 20 | Plants as Library images with Plant Image Specification, richer Plant Chooser (type, needs, zone, co | 193 |
| 21 | Text macro import and export between plans, Chief global macro names, Copy and evaluation errors | 68 |
| 22 | Export Logs and a program log; keep Program Paths and Ruby as out of scope | 212 |

### Part 7 (workflow breaks)

| Source # | Source gap (shortened) | Master rank(s) |
|---|---|---|
| 1 | Sheet numbering: no page Label with `#` prefix numbering (A0.#, A1.#, E1.#) and no per-page template | 1 |
| 2 | Site plan from a survey: no bearings (N 61 25 10 E) in Input Line or angle fields, no Current Point  | 2 |
| 3 | Send to Layout offers no site-plan scales (1 in = 30 / 40 / 50 / 100 ft) or typed ratio and gives no | 27 |
| 4 | Roof panel missing for a multi-wall selection and for Interior-kind walls (Half Wall, Railing, inter | 28 |
| 5 | Residential-template behaviours: no roof and no Auto Exterior Dimensions when a room closes, no Auto | 28, 26 |
| 6 | Wall Type Definitions: no layer Fill colour, no Role (framing, air gap, standard), no library materi | 4 |
| 7 | Layered floor / ceiling finish and structure definitions (Platform Defaults, hat-channel lowered cei | 6 |
| 8 | Project model: no Project, no Make a Copy list of archives, no folders, no Dashboard pin; every less | 198 |
| 9 | Saved Plan Views with Selected Defaults and Default Sets (text and dimension defaults, layers, scale | 16, 21, 36 |
| 10 | Window > Tile Vertically / Swap Views (plan beside 3D or section) used in 13 lessons | 194 |
| 11 | Dimensions and CAD in section / elevation views (headroom checks, stair section notes, exterior elev | 7 |
| 12 | Architectural Blocks (kitchen island, furniture groups, ganged switches, plant groups, library block | 98 |
| 13 | Library modal pickers (Select Library Object / Select Material): newels, hardware, door styles, mold | 109 |
| 14 | Schedules: Categories to Include tree, room scope, picture columns, wall-legend columns, Move Row an | 17, 10, 80 |
| 15 | Layout CAD editing for title blocks (Center Object, Point to Point Move, concentric copies, Selected | 82 |
| 16 | Exterior elevations sent as Plot Lines with Color Fill or as Live Views; Link Saved Plan View | 27, 101 |
| 17 | Page-linked callout labels on layout section / elevation boxes | 82, 9 |
| 18 | Wall framing workflow: Retain Wall Framing is a disabled check box, Build Framing for Selected Objec | 47 |
| 19 | Stairs: Make Best Fit, Lock Top / Lock Bottom, Staircase Information read-outs, DN stairs to grade w | 48 |
| 20 | Terrain: Absolute Elevation + Reference Point, Selected Line / Arc panels on elevation lines, road c | 30, 134, 193 |
| 21 | Rich Text: Uppercase button, Insert Macro with object-referencing macros (%comment%, %description%,  | 68, 115 |
| 22 | Garage foundation options (curb and lowered stem), "S" markers, Room Supplies Floor, deck footings a | 63 |
| 23 | Cathedral ceilings: no Flat Ceiling Over This Room switch; no Tray Ceiling tool | 95 |
| 24 | Cabinets: multi-select Open Object, Auto door item, per-item shelves, Suppress Label, module-lines l | 54, 50 |
| 25 | Electrical: context-sensitive Light and Outlet tools, interior vs exterior wall-light defaults, rece | 126 |
| 26 | Roofs: Curved Roof planes, Display On Floor Above, frieze profiles, Ceiling Break Lines | 163, 103, 83 |
| 27 | Moldings: Floor Defaults molding table, Exterior Room, Make Room Molding Polyline, per-edge molding | 38 |
| 28 | Arithmetic in dialog number fields; Enter Coordinates dialog | 19, 34 |
| 29 | Drawing Sheet Setup per view and its Drawing Scale feeding Send to Layout | 56 |
| 30 | Allowed Angles entry by bearing, DMS or decimal; Number Style / Angle Style dialog | 148, 29 |

158 of the 215 entries are named by at least one ranked gap above; the other 57 come only from row-level findings inside the parts' tables (ranks 37, 41, 61, 71, 76, 89, 91, 102, 104, 111, 112, 113, 118, 123, 124, 127, 129, 133, 138, 140, 141, 142, 144, 146, 147, 150, 156, 160, 161, 164, 169, 170, 171, 174, 175, 178, 179, 180, 181, 183, ...).

### Part 7 step rows placed by hand

These 27 Missing/Partial tutorial steps cited no parity id and matched no keyword rule; each belongs to an existing entry:

| Tutorial page | Step | Entry |
|---|---|---|
| pp. 29-30, 98 | Floor and Room Type defaults hold structure definitions; centering windows | assemblies, centeralign |
| p. 36 | room label delete and Show Room Label | livingarea |
| p. 47 | derived walls take first-floor exterior wall types | floorlevels |
| pp. 160, 189, 485-486 | Roof Plan View as a working view; Saved Plan View Control | savedviews |
| pp. 192-193, 232-233 | User Catalog folders and drag-in of items | libuser |
| p. 196 | Material Painter object mode from the Library Browser | planmaterials |
| pp. 199, 208 | Material Region layers, Thinset thickness | assemblies, regions |
| pp. 231, 313 | accessory inside furniture; lamps seek the table top | symbolspec |
| p. 280 | expression typed in a Partition height | mathfields |
| pp. 285, 291 | Fixture 3D panel (Reflect Geometry, Include Size) | symbolspec |
| p. 296 | Suppress Label in all views | objlabels |
| p. 310 | Multiple Copy of two lights | copypaste |
| p. 355 | Wall Detail named from the wall label, Open Wall Detail | framingdisplay |
| pp. 393-394 | dashed line style; Number Style back to fractional inches | linestyles, numstyle |
| p. 446 | planting border garden bed replace from library | plants |
| p. 478 | Point to Point Move of an image | layoutcad |
| p. 482 | Grid Snap Unit quiz | layoutpages |
| p. 492 | view too large for the sheet message | layoutsend |

## Deduplication statistics

- Entries cited by two or more audit parts: 68 of 215. The widest: 104 Drawing groups (parts 1,2,3,4,5, 12 rows); 52 Materials List scopes, specification, columns, export formats and Master List (parts 2,3,4,6, 64 rows); 135 Convert tools and polyline types (parts 1,2,4,6, 20 rows); 108 Painter options (parts 1,2,3,4, 17 rows); 175 Dialog conventions (parts 1,2,3,5, 13 rows); 177 Delete Objects scopes (parts 1,2,3,4, 9 rows); 53 Material Painter scoping modes, Plan Materials dialog, Select Material dialog (parts 2,4,5, 49 rows); 38 Moldings system (parts 2,3,4, 37 rows); 11 A dimension line as one object (parts 1,2,6, 25 rows); 21 Saved Defaults for annotation tools (parts 1,2,3, 22 rows).
- Distinct parity ids cited by the 2589 rows: 1976; ids cited by two or more parts: 143.
- Rows per entry: median 9, largest 64 (matlistscopes).
- One entry has no feature row in the Missing/Partial set because its underlying row is a Differs finding: Clear Terrain (part 6 row 1309), kept because it is a data-loss defect.

## Quick wins in the top 100 (size S)

- 19. Arithmetic in every number box (10 edit-behaviours-keys)
- 25. Clear Terrain erases the perimeter, data and features instead of only the built surface (33 terrain-site)

