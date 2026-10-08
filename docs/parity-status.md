# Parity status: Plan Studio vs Chief Architect X18

Snapshot of 2026-10-08 against HEAD 6f6a7b9. Written by reading every spec in `docs/parity/*.md`, `docs/architecture-tools.md`, `docs/integration-queue.md` and `docs/qa-findings.md`, then locating the code for each id (ids cited in `crates/` first, otherwise the feature by name in `tools/`, `dialogs/`, `editor/` and the engine crates). No Rust file was edited. `cargo test -p plan-app scenarios` passes (95 tests). The full workspace run has one failure, `editor::render::perf_bench::a_door_added_changes_the_schedule_that_is_drawn`, in `crates/plan-app/src/editor/perf_bench.rs`, a file another builder was editing during this pass (its line numbers moved between runs); it is not a parity gap.

**Status words.** Works: behavior is present and exercised (a test, a scenario or the tool itself). Partial: present with a stated limit. Missing: no code path or a dimmed/stub control. Differs-by-design: deliberately different (a DECISIONS.md call, the manual's "Known differences" or a spec note) so it is not counted as a gap to close. Statuses come from code reading plus the manual chapters; "verify in Chief" items are not settled by this table (see the list below).

There are 765 parity ids: W 105, S 112, DW 110, R 71, RF 60, DIM 45, TXT 19, CAD 42 (CAD-17..19 do not exist in the spec), LAY 15, C 71, CB 68, L 47. Stairs, framing, terrain, library and electrical are CB-22..CB-68; no separate parity spec exists for exterior details, slabs, images or schedules beyond L-23..L-32 (their status lives in manual chapters 16 and 17).

## Totals by area

| Area | Ids | Works | Partial | Missing | Differs | Works % |
|---|---|---|---|---|---|---|
| Walls | 105 | 64 | 25 | 14 | 2 | 61% |
| Select | 112 | 51 | 32 | 28 | 1 | 46% |
| Doors/Windows | 110 | 42 | 34 | 33 | 1 | 38% |
| Rooms/Floors | 71 | 37 | 28 | 6 | 0 | 52% |
| Roofs | 60 | 28 | 19 | 13 | 0 | 47% |
| Dimensions | 45 | 27 | 11 | 6 | 1 | 60% |
| Text | 19 | 8 | 10 | 1 | 0 | 42% |
| CAD | 42 | 23 | 17 | 2 | 0 | 55% |
| Layers | 15 | 10 | 3 | 2 | 0 | 67% |
| 3D/Cameras | 71 | 31 | 25 | 15 | 0 | 44% |
| Cabinets | 21 | 11 | 10 | 0 | 0 | 52% |
| Stairs | 13 | 8 | 5 | 0 | 0 | 62% |
| Framing | 8 | 3 | 3 | 2 | 0 | 38% |
| Terrain | 10 | 7 | 3 | 0 | 0 | 70% |
| Library | 9 | 4 | 4 | 0 | 1 | 44% |
| Electrical | 7 | 5 | 2 | 0 | 0 | 71% |
| Layout/Docs | 47 | 8 | 30 | 8 | 1 | 17% |
| **Overall** | 765 | 367 | 261 | 130 | 7 | 48% |

Weighted view: Works plus Partial covers 628 of 765 ids (82%); 130 ids are Missing.

Reading the table: the engines are strong (walls, joins, roofs, stairs, terrain, 3D, layout engine). The gaps cluster in everyday editing commands (select, clipboard, transform), door and window variants and plan labels, associative dimensions, printing output, schedules interaction and 3D picking/materials.

## Next 25 gaps for daily residential work

Ranked by value to a residential designer (walls, openings, rooms, dimensions, cabinets, roofs, layout and printing first). Size: S under a day, M a few days, L a week or more of one builder.

| # | Gap (parity ids) | Why it matters daily | Files a builder would touch | Size |
|---|---|---|---|---|
| 1 | Clipboard and selection basics: Cut, Copy, cursor-attached Paste, Cmd+A Select All, Cmd+D duplicate, bind the `C, P, P` chord (S-33, S-81..S-84, S-93) | Every plan session; today only the Edit-toolbar Copy and Paste in Place buttons exist and the Edit menu rows are inert | `editor/actions.rs`, `editor/dispatch.rs`, `shell/hotkeys.rs`, `toolbar.rs` (BINDINGS), `menus.rs` (rows near line 273-295), `main.rs` (send_key), `editor/selection.rs` | M |
| 2 | Transform/Replicate Object, Reflect About Object, Point to Point Move, Center Object, multi-object Rotate (S-47, S-48, S-52, S-53, S-101..S-106, DW-23) | Mirror a kitchen, array windows, rotate a wing; no way to do any of it today | new `editor/transform.rs`, pure transform fns in `plan-core` (over `ObjectRef`), new `dialogs/transform.rs`, `editor/actions.rs`, `editor/dispatch.rs`, `tools/select.rs` | L |
| 3 | Door and window variants with plan symbols: Doorway, Sliding, Pocket, Bifold, Garage, Barn, Fixed, Shower; Bay, Bow, Box windows, Pass-Through, Wall Niche; double-door leaves (DW-38..DW-49, DW-107) | 13 flyout entries are dimmed stubs; the model and 3D already know all 15 styles, only the tools and 2D symbols are missing | `toolbar.rs` (lines 704-725), `tools/opening.rs`, `editor/render.rs` (draw_opening), `plan-core/src/openings.rs`, `dialogs/opening.rs`, `dialogs/default_lists.rs`, `shell/hotkeys.rs` | L (S for Doorway and Sliding first) |
| 4 | Opening labels in plan (size shorthand 3068, schedule mark in the label) and automatic mark assignment on placement (DW-59..DW-63, DW-77, DW-94) | Door and window tags are on every permit set; `Opening::auto_label` exists but nothing draws it | `editor/render.rs`, `editor/schedule_view.rs`, `plan-core/src/openings.rs`, `dialogs/opening.rs` (persist Label tab), `tools/opening.rs` | M |
| 5 | Opening resize handles, width temporary dimension, Center Object, Mull/Unmull windows (DW-12, DW-24, DW-26..DW-28, DW-51, DW-52, DW-98) | Resizing a window means opening a dialog; adjacent windows cannot be mulled | `editor/handles.rs`, `tools/select.rs` (new Op), `editor/tempdim.rs`, `plan-core/src/openings.rs` (mull group), `editor/actions.rs` | M |
| 6 | Typed length and angle while drawing walls, angle readout, Shift orthogonal, Alt suspends all snaps (W-15, W-16, W-17, W-18, S-74) | Drawing to exact dimensions is the core wall workflow; today you draw then retype in a temp dimension | `tools/wall.rs` (key handling), `editor/mod.rs` (capture_typing, queue item), `editor/tempdim.rs`, `editor/snap.rs` | M |
| 7 | Wall edit commands: Break Wall, Reverse Layers, Change Line/Arc with a bulge handle, Make Arc Tangent, Convert to Polyline, numeric Curved Wall section (W-23, W-43, W-44, W-66..W-68, W-90, S-41, S-42, S-55) | Layer reversal and breaking a wall are routine edits; curved walls cannot be edited by number | `editor/actions.rs`, `editor/dispatch.rs`, `plan-core/src/walls.rs` (flip layers, set curve), `editor/handles.rs`, `tools/select.rs`, `dialogs/wall.rs` (Curved section) | M |
| 8 | Associative dimensions plus Locate settings (DIM-3, DIM-4, DIM-26, DIM-29, DIM-40) | Dimensions that do not follow a moved wall are the top trust problem on a drawing set | `plan-core/src/dimension.rs` (anchors), `tools/dimension.rs`, `editor/ops.rs`, `editor/tempdim.rs`, `dialogs/default_lists.rs` | L |
| 9 | Auto Exterior Dimensions completeness: openings string and wall-to-wall string, non-orthogonal sides, edited-auto becomes manual (DIM-24, DIM-25, DIM-33) | First thing run on every floor plan; today only overall plus breakpoint strings | `tools/dimension.rs` (AutoExterior), `plan-core/src/dimension.rs`, `plan-core/src/defaults.rs` | M |
| 10 | Printed-size text and dimension text tied to plan scale; upright vertical dimension text, outside-text flip (DIM-7, DIM-8, DIM-9, TXT-2, TXT-19, L-15) | Text that stays 1/8 in at any scale is how every sheet is annotated | `editor/render.rs`, `tools/text.rs`, `plan-core/src/text_styles.rs`, `plan-layout` (render_box_lines) | M |
| 11 | Roof directives UI: Wall Specification Roof tab, roof style presets (Gable, Hip, Shed, Gambrel, Mansard, Dutch), Dutch gable, knee wall, upper pitch break (RF-3, RF-5, RF-18, RF-21, RF-23, RF-25) | Gambrel and Dutch gable roofs are common on residential jobs; the tab is disabled and gable/hip is a click-toggle only | `dialogs/wall.rs`, `plan-core/src/walls.rs` (RoofDirective), `plan-roof/src/skeleton.rs`, `plan-roof/src/spec.rs`, `editor/roof_view.rs`, `dialogs/roof.rs` | L |
| 12 | Roofs cut walls: gable triangles and walls clipped to roof planes in 3D, eave detail (soffit, fascia, eave cut), butting roofs (RF-13..RF-16, RF-20) | 3D and elevations show boxy wall tops under the roof, which clients and building departments notice | `plan-3d/src/wall.rs`, `plan-3d/src/roof.rs`, `plan-roof` (rake, fascia), `editor/roof_view.rs` | L |
| 13 | Materials List: roofing, cabinets, waste factors, stock-length rounding, prices, categories, layout/PDF output (RF-60, CB-41, L-33..L-37) | Estimating quantities is a daily deliverable; today a simple wall-only take-off | `plan-docs/src/materials.rs`, `dialogs/build_tools.rs`, `plan-roof`, `plan-cabinets`, `plan-layout` | M |
| 14 | Layout boxes beyond plan views: text, schedule, perspective/3D raster and layout CAD on pages; box rotation (L-4, L-5, L-16, L-24) | Sheets need notes, schedules and renderings on the page; the editor can only create plan and elevation boxes | `shell/layout_window.rs`, `dialogs/layout.rs`, `plan-layout` (model, render), `editor/schedule_view.rs` | L |
| 15 | Printing: printer dialog with scale and tiling, B&W and grayscale, PDF embedded fonts, bookmarks, custom scale ratio (L-18..L-20, L-22) | PDF is the only output and uses Helvetica with no bookmarks | `plan-docs/src/pdf/`, `shell/layout_window.rs`, `dialogs/layout.rs`, `toolbar.rs` (Print button) | M |
| 16 | Schedule interaction: click row selects the object, grouping and totals, Room Finish and Note kinds, Wall schedule placeable, Schedule tab data on objects (L-23, L-27..L-30) | Schedules are read-only pictures today | `editor/schedule_view.rs`, `plan-docs/src/schedule_kinds.rs`, `plan-core/src/schedules.rs`, `dialogs/schedule_spec.rs` | M |
| 17 | Right-click context menu on objects and empty space (S-8, DW-109) | Right-click currently only ends a drawing chain | `main.rs` (canvas secondary click), `editor/actions.rs`, `tools/select.rs` | M |
| 18 | Select objects in the 3D view (C-43, C-39 orbit center pick) | The plan-side half is done (`pick_for_mesh_id`); the panel has no pick hook | `shell/view3d_panel.rs`, `editor/selection.rs`, `plan-view3d/src/` | M |
| 19 | Room Function behavior and 3D honoring Floor/Ceiling Over This Room: garage drop, deck/porch no ceiling, Open Below cut-outs, function-driven defaults (R-30, R-40, R-41, CB-29, CB-30) | Garage floor drop and open-to-below foyers are standard residential rooms | `plan-3d/src/slab.rs`, `plan-3d/src/lib.rs`, `editor/rooms_edit.rs`, `dialogs/room.rs`, `plan-core/src/model.rs` | M |
| 20 | Floor Defaults dialog, fuller Build New Floor options (interior walls, rooms, below), Reference Display dialog and floor above (R-56, R-58, R-59, R-65, LAY-10) | Multi-story set-up and tracing the floor below | `toolbar.rs` (line 1869 stub), new `dialogs/floor_defaults.rs`, `plan-core/src/floors.rs`, `editor/render.rs` (draw_reference_floor) | M |
| 21 | Nested rooms (ring polygon and platform hole), draggable room labels, Floor/Ceiling Structure Define (R-11, R-28, R-29, R-44) | Islands such as chimney chases and wet-room pods get the wrong area and floor | `plan-core/src/rooms.rs`, `plan-3d/src/slab.rs`, `editor/render.rs`, `editor/rooms_edit.rs`, `dialogs/room.rs` | M |
| 22 | Cabinet editing: depth and corner resize handles, Sides and Back face editing, automatic countertop join, auto-fit to gap (CB-5, CB-8, CB-10, CB-14) | Kitchen layout is the highest-value interior task; resizing needs the dialog | `dialogs/cabinet.rs`, `editor/placed.rs`, `editor/handles.rs`, `tools/cabinet.rs`, `plan-cabinets` | M |
| 23 | PDF and image underlay with two-point calibration; DXF import layer mapping and block explode; DWG later (L-43, L-46) | Tracing a survey or an existing-home PDF starts most remodel jobs | new underlay object in `plan-core`, `dialogs/exchange.rs`, `editor/render.rs`, `plan-import` | M |
| 24 | Snap Settings dialog (per-snap on/off, Center, Quadrant, Tangent) and Edit Behaviors (S-65, S-68, S-69, S-74, CAD-40) | The engine has per-type flags but the dialog is inert | `editor/snap.rs`, `menus.rs` (lines 296-297), new `dialogs/snap_settings.rs`, `editor/mod.rs` | S |
| 25 | Edit-toolbar completeness: Group/Ungroup buttons and group-aware pick, Select Same Type, Align/Distribute, Layer button, Delete Objects dialog, Action History panel, Find/Replace Text (S-32, S-35, S-36, S-54, S-79, S-88, TXT-12, LAY-13) | Many small commands; the core group model already exists | `editor/actions.rs`, `tools/select.rs`, `plan-core/src/groups.rs`, `editor/selection.rs`, `menus.rs`, `shell/docks.rs` | M |

Not in the 25 but cheap and worth a sweep: wall Structure tab options (Through Wall At Start/End, platform intersections: W-39, W-62), multi-wall Open Object (W-83), Roof/Foundation/Wall Cap tabs of the Wall Specification, the Current CAD Layer button (CAD-1, LAY-6), Dimension Specification Format/Arrow/Text tabs (DIM-31, DIM-39), the Room Types Copy/Select All/Clear All buttons (R-38).

## "Verify in Chief" items for one live session

Counts exclude the nine "Lines marked (verify in Chief) ..." preamble sentences (one per parity file). Totals: 211 in `docs/parity`, 1 in `docs/integration-queue.md`, 1 in `DECISIONS.md` (#6) = **213**. Where one id carries two markers, both are listed.

### A. Plan view: wall tools, snapping, joins (24)
- W-1 interior wall default thickness; W-3 right-click does not end chain, double-click does; W-4 does drag-release continue the chain; W-5 closing on first point is a proper corner.
- W-14 alignment guide set; W-15 angle readout origin; W-16 Tab behavior in length/angle entry; W-18 Shift constrains to 0/90 or not; W-19 typed length rounding to Dimension Defaults.
- W-21 exterior side is right or left of drawing direction; W-22 no auto-flip of layer stack after a room forms.
- W-34 3+ walls at a point pick the through wall; W-35 T-junction keeps the through wall whole (we split it); W-37 stepped faces when main layers differ in thickness; W-105 dragging the through wall: does the butting end follow.
- W-42 Fix Wall Connections also removes duplicates and zero-length walls; W-43 Break Wall refusal when an opening straddles; W-44 Add Break equals Break Wall on walls; W-45 Merge/Join collinear walls exists in X18.
- W-54 half-wall height and plan style (42 in? dashed?); W-59 Wall Hatching and Material Region behavior; W-64 curved wall radius readout and gestures; W-93 hidden-layer wall still defines rooms; W-96 duplicate-wall merge or warning.

### B. Wall Specification dialog and wall Edit toolbar (6)
- W-26 default Resize About per exterior/interior; W-29 thickness edit changes main layer only; W-78 Lock Center: openings keep which distance; W-83 multi-wall Open Object shows indeterminate fields.
- W-85 opening clamp/delete when a wall edit leaves too little room; W-90 Convert to Polyline: delete or keep the wall.

### C. Select Objects: picking, handles, drag (21)
- Picking: S-4 CAD/text on active layer win; S-5 locked-layer objects selectable for viewing only; S-6 hover status text; S-8 right-click menu vs pan; S-29 window vs crossing marquee direction; S-30 Alt to marquee over objects; S-31 Ctrl/Cmd-click means nothing; S-32 Select Same Type UI; S-34 Tab/Shift+Tab candidate highlight; S-35 group click and double-click; S-36 Select Group Member; S-37 multi-select drag with walls is a plain translate; S-112 CAD layer order pick.
- Handles: S-11 handles with several objects selected; S-14 Shift keeps proportions; S-16 mid-edge handle on polylines; S-18 no rotate on straight walls; S-22 Alt free move vs Point to Point Move; S-25 text handles; S-28 Tab or typing during a drag; S-99 auto-scroll when dragging off screen.

### D. Edit toolbar, Edit menu, undo, clipboard (14)
- S-40 common button order; S-41 straight-wall button list; S-48 Reflect About Line flow; S-66 Edit Behavior indicator and reset; S-67 fillet/chamfer not offered for wall corners.
- S-76 chain walls are one step each; S-78 undo level default (20?); S-80 default-settings edits not undoable; S-85 paste across files keeps layers; S-88 Delete Objects dialog; S-90 Edit Area menu names; S-93 Cmd+D; S-104 radial array via Rotate + copies; S-105 reflected walls keep exterior outward.

### E. Snap Settings, modifiers, temporary dimensions (7)
- S-57 temp-dim width entry for doors/windows; S-63 locked temp dimensions become permanent.
- S-69 snap priority order; S-73 pushing behavior; S-74 Alt suspends all snaps; S-92 Shift+arrow 10 units and wall nudge; S-94 modifier table (Ctrl+drag copy).

### F. Door and Window placement, Door/Window Specification (28)
- Placement: DW-4 adjacent windows may touch; DW-5 drag after click; DW-8 swing/hinge rules (both); DW-10 alignment candidates and clearances; DW-12 width-dimension resize pivot; DW-13 end clearance; DW-19 drag through wall flips swing; DW-70 double-wall host; DW-71 host picking at junctions; DW-88 opening on curved wall measure; DW-92 openings on room dividers; DW-110 Alt disables alignment snaps and Shift constrains nothing.
- Specification: DW-6 exterior vs interior defaults; DW-7 exterior door defaults; DW-14 General tab position fields; DW-27 library width list snapping; DW-50 window head default (80 in vs our 84/96).
- Swing and Edit toolbar: DW-32 Reverse Swing / Flip Hinge names; DW-33 diagonal swing handle; DW-37 no Swing Toward Exterior toggle.
- Symbols: DW-38 double-door threshold (about 40 in?); DW-41 pocket door symbol; DW-43 garage door default size.
- Mulling: DW-51 auto vs manual mull; DW-52 sidelite/transom mull.
- Label tab: DW-59 fraction handling in 3068 shorthand; DW-60 schedule number style; DW-63 label layer and label handle.

### G. Dimension, Text, CAD tools and dialogs (31)
- Dimensions: DIM-3 deleting a located object; DIM-4 default Locate per exterior/interior; DIM-7 printed-size extension offsets; DIM-11 string continuation and measuring-direction rule (two markers); DIM-16 Running Dimension tick labels; DIM-17 Baseline row spacing; DIM-19 Centerline Dimension scope; DIM-24 Auto Exterior strings and 24 in spacing; DIM-27 Auto Interior in X18; DIM-32 which end moves on value edit; DIM-33 automatic becomes manual on edit; DIM-37 Reverse Dimension on Edit toolbar; DIM-38 copy with referenced objects; DIM-40 temporary-dimension locate settings in Dimension Defaults.
- Text: TXT-2 printed text size; TXT-5 spline leader; TXT-8 markers linking to views; TXT-10 smart text alignment.
- CAD: CAD-7 arc creation mode list; CAD-8 Input Arc fields; CAD-11 which circle variants exist; CAD-14 Shift constrains lines; CAD-15 right-click end menu; CAD-23 Convert to Polyline/Spline; CAD-25 Concentric behavior; CAD-27 self-intersecting fill rule; CAD-29 spline Edit toolbar; CAD-39 Input Line dialog Tab order.
- Layers: LAY-9 snapping to reference objects; LAY-12 layer changes in the undo stack.

### H. Rooms, Room Specification, Floors (20)
- Detection: R-3 railing-wall behavior; R-13 default room name ("ROOM"/"Room"); R-15 split and join identity (two markers).
- Room Specification: R-19 tab list; R-24 Absolute/Relative toggle wording; R-26 stem wall enable conditions; R-32 sloped-ceiling height meaning; R-40 Function list; R-26 (garage bullet) default garage drop (about 4 in); R-42 Utility living-area default; R-47 label macro tokens.
- Labels and areas: R-49 Standard Area definition; R-51 living area uses Interior or Standard; R-52 menu path of the living-area readout.
- Floors: R-56 Floor Defaults field layout; R-57 template floor structure/ceiling numbers; R-59 Build New Floor wording; R-61 Build Foundation type names; R-68 Attic via Build Attic.

### I. Roofs: Build Roof dialog, wall Roof tab, roof tools, framing (16)
- Build Roof: RF-2 dialog labels; RF-3 style preset list; RF-7 replace-existing prompt.
- Geometry and wall Roof tab: RF-10 overhang reference (sheathing vs framing); RF-21 Dutch gable fields.
- Attic: RF-31 Automatically Generated Wall naming; RF-32 Attic via Build New Floor or roof dialog.
- Tools: RF-35 Roof Plane gesture (baseline-first); RF-37 no "Reset to automatic"; RF-41 Join Roof Planes trim vs extend; RF-44 Gable/Roof Line; RF-47 ceiling planes dashed in plan; RF-49 floating dormer.
- Framing: RF-52 default rafter size and spacing; RF-55 truss web depth; RF-57 Retain Framing setting.

### J. 3D menu, camera tools, Camera Specification and View Options (25)
- Camera tools: C-5 default eye height 66 in and FOV 60 (two markers); C-7 FOV is horizontal; C-13 Doll House upper floors; C-14 overview camera symbol; C-16 ortho scale field; C-17 section side by drag direction; C-20 Wall Elevation gesture; C-23 Cross Section Slider; C-25 FOV and distance handles; C-27 Shift constrains aim.
- Dialogs: C-30 Camera Specification field list; C-31 Camera View Options tab list; C-33 Locked Camera; C-68 3D View Defaults grouping.
- Navigation: C-34 Move Camera with Mouse tool names; C-38 keyboard keys and step sizes; C-39 3D Center gesture; C-41 View Direction.
- Rendering, materials, lights: C-45 technique list; C-55 Delete Surface restore; C-56 Material Painter modes; C-58 Material Definition options; C-65 light set names; C-71 walkthrough keyframe editing.

### K. Cabinets, Stairs, Terrain, Library, Electrical (12, plus DECISIONS #6)
- Cabinets: CB-5 auto-fill modifier and Preference; CB-9 Ctrl disables auto-rotation; CB-13 label format (B24, W3030).
- Stairs: CB-23 default riser/tread/width; CB-29 stairwell option names; CB-30 stairwell room label ("STAIRWELL" vs "Open to Below"); CB-34 ramp slope check in dialog; plus DECISIONS #6 Click Stairs heading rule.
- Other: CB-42 structural calculators scope; CB-51 Terrain Specification fields; CB-54 Library search filter list; CB-57 Replace From Library keeps cabinet inserts; CB-64 Auto Place Outlets clearance (2 ft or 1 ft).

### L. Layout, Print, Schedules (7) and the integration queue (1)
- Send to Layout dialog: L-2 field names; L-3 box refresh mode names. Page Setup: L-8 sheet list and Customize Sheet Sizes; L-22 scale lists. Layout Box: L-12 line-weight multiplier name. Site plan: L-14 scale bar object. CAD menu: L-39 Auto Detail.
- Integration queue (docs/integration-queue.md, CAD section item 4): Cross Box, Blocking Box and Insulation symbols are drawn as an X box, a one-diagonal box and a boxed wave; compare with Chief.

## Open items from the queue and QA files not tied to one id

- 3D: electrical devices have a builder but are not in the 3D view; Build Framing's wall, floor and roof members draw in plan only; billboards keep their stored angle; landscape uses stand-in materials (no green); curved walls get no door/window cuts in 3D (manual 10.1, queue).
- Walls: `Wall.bottom_offset` is honored in 3D only, not in the 2D plan, room detection, elevations, schedules or wall framing (queue, manual 2.9); curved walls have no mitered joins.
- Stairs: guard railings around the stairwell opening, flared apron of Flare/Curve Stairs, railing across a landing in plan, a Stair Schedule, dashed hidden treads on other floors, and stair side railings are not clipped by the floor above in 3D (queue, Round 8 notes).
- Details (chapter 17): Line Style pages are disabled, moldings do not miter at polyline corners, "3D Solid Feature" custom molding profile has no editor; trim does not follow walls edited through the dialog or Delete All.
- Terrain: Select Objects now has `ObjectRef::TerrainObject` (editor/selection.rs), so the manual's statement that single terrain objects cannot be picked looks stale; terrain walls and curbs do not cut the TIN.
- Schedules: the placed tables are not in the DXF export or the construction-set PDF; the table is rebuilt every frame; Cabinet/Plant/Fixture rows come from placed symbols.
- Doc drift to correct in the manual: chapter 2.4 says the Fix Wall Connections button is not wired (it is: `editor/actions.rs`, `editor/connect.rs`); chapter 3 says pointer-derived swing/hinge is planned (fixed as QA-01); chapter 10.10 says 3D picking is planned for Round 9 (plan-side `pick_for_mesh_id` exists, no caller).

## Per-id status

### Walls (`docs/parity/walls.md`)

105 ids: 64 Works, 25 Partial, 14 Missing, 2 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| W-1 | Walls | Exterior and interior wall tools, own defaults (ext 7 5/8) | Works | tools/wall.rs WallVariant::spec; template-seeded defaults (plan-core defaults.rs) | Built-in fallback is 6.5 in; template seeding supplies Chief values. |
| W-2 | Walls | Toolbar face shows last wall variant | Works | toolbar.rs Straight Wall flyout; manual 2.2 | - |
| W-3 | Walls | Click chain; double-click/Esc ends; right-click keeps chain | Partial | tools/wall.rs advance_chain | Right-click ends the chain here (Chief keeps it). |
| W-4 | Walls | Click-drag draws one wall, chain continues | Partial | tools/wall.rs test drag_draws_one_wall_and_releases | Drag-release ends the chain (spec says verify). |
| W-5 | Walls | Click first point closes loop | Works | wall.rs test sloppy_chain_closes_on_the_start_point_with_exact_corners | - |
| W-6 | Walls | New wall layer/floor/height from defaults | Works | wall.rs create(), class_layer | - |
| W-7 | Walls | Tiny walls discarded | Works | wall.rs MIN_LENGTH | Threshold is 1 in not 1/16 in. |
| W-8 | Walls | Esc cancels current wall, keeps chain | Works | wall.rs key(); manual 2.2 | - |
| W-9 | Walls | Wall tool never selects | Works | wall.rs pointer_down | - |
| W-10 | Walls | Switch variant mid-chain | Works | wall.rs set_variant | - |
| W-11 | Walls | Snap order object, angle, grid | Works | editor/snap.rs, wall.rs snap() | - |
| W-12 | Walls | End on a wall makes T | Works | wall.rs test a_wall_ending_near_a_centerline_makes_a_tee_and_splits_it | Through wall is split (see W-35). |
| W-13 | Walls | Snap markers and status names snap | Works | render.rs draw_snap_marker; SnapKind::label | - |
| W-14 | Walls | Dashed alignment guides to other endpoints | Partial | wall.rs snap() axes/collinear/perpendicular snaps | Snaps exist but no dashed guide lines to arbitrary existing endpoints. |
| W-15 | Walls | Length and angle readouts while drawing | Partial | wall.rs update_readout (Length only) | No angle readout. |
| W-16 | Walls | Type length/angle while drawing (Tab, Enter) | Missing | manual 2.2 says planned | No typed entry during wall drawing. |
| W-17 | Walls | Alt suspends all snaps | Partial | snap.rs header: Alt suspends angle snap only | Object and grid snaps stay on. |
| W-18 | Walls | Shift constrains to 0/90 | Missing | none | Not bound. |
| W-19 | Walls | Lengths in project dimension format | Works | EditorContext::fmt_dim | - |
| W-20 | Walls | Ghost wall with real footprint | Works | wall.rs draw_overlay | Ghost does not run through join solver. |
| W-21 | Walls | Wall direction and exterior side | Works | plan-core walls.rs Wall.exterior_side, Side | - |
| W-22 | Walls | No auto interior/exterior detection | Works | tool chosen sets type; no auto flip | - |
| W-23 | Walls | Reverse Layers command | Missing | grep finds no command | Add Edit-toolbar Reverse Layers flipping exterior_side. |
| W-24 | Walls | General Options flags | Partial | dialogs/wall.rs; flags in walls.rs | Invisible/No Room/No Locate/Foundation/Railing work; Lock Center, Terrain Retaining, Attic, Auto-Generated disabled. |
| W-25 | Walls | Automatically Generated wall flag | Missing | dialogs/wall.rs disabled control | Flag not stored or warned on delete. |
| W-26 | Walls | Baseline reference (Resize About) | Works | walls.rs ResizeAbout::default_for | - |
| W-27 | Walls | Thickness change holds reference | Works | walls.rs set_wall_thickness_about + test | - |
| W-28 | Walls | Dimension-driven resize holds other face | Partial | tempdim.rs WallGap moves wall | Moves wall, no face-to-face resize of thickness. |
| W-29 | Walls | Thickness edits main layer | Works | dialogs/wall.rs custom thickness | - |
| W-30 | Walls | Min thickness, no zero thickness | Partial | dialogs/wall.rs validation | Minimum not enforced at 1/8 in everywhere. |
| W-31 | Walls | Persisted connections | Differs-by-design | manual 2.10: inferred from coordinates each time | Same behavior, no stored connection graph. |
| W-32 | Walls | Mitered L joins, miter limit | Works | plan-core joins.rs MITER_LIMIT; tests | - |
| W-33 | Walls | Collinear continuation seamless | Works | joins.rs square ends, layer bands | - |
| W-34 | Walls | 3+ walls at a point choose through wall | Partial | joins.rs header: 3+ keep square ends | No through-wall selection at multi junctions. |
| W-35 | Walls | T keeps through wall whole | Differs-by-design | manual 2.10, qa-findings: split_on_tee | Plan Studio splits the through wall. |
| W-36 | Walls | Crossing walls unbroken, cleaned | Partial | connect.rs cut at crossing | Crossing walls are cut into pieces. |
| W-37 | Walls | Per-layer corner joins | Works | joins.rs wall_layer_outlines | - |
| W-38 | Walls | Any-angle miter | Works | joins.rs | - |
| W-39 | Walls | Through Wall At Start/End | Missing | dialogs/wall.rs Structure tab disabled | Not modelled. |
| W-40 | Walls | Join cleanup in same undo step | Works | connect.rs auto_connect; begin_change | - |
| W-41 | Walls | Fix Wall Connections | Works | editor/actions.rs FixWallConnections; connect.rs fix_wall_connections_action | Edit-toolbar button only, not a flyout tool. |
| W-42 | Walls | Repair duplicates and zero-length | Works | connect.rs test duplicate_wall_inside_another_is_dropped | - |
| W-43 | Walls | Break Wall at a point | Partial | plan-core walls.rs split_wall_at (openings follow) | No Break Wall tool or button. |
| W-44 | Walls | Add Break on walls | Missing | none | Same as W-43. |
| W-45 | Walls | Join collinear walls | Missing | none | Not offered (rarely used). |
| W-46 | Walls | Wall type layer stack with main layer | Works | plan-core defaults WallTypeDef; dialogs/wall_types.rs | - |
| W-47 | Walls | Types shared and editable; Library button | Partial | dialogs/wall_types.rs | Wall Types Library button disabled. |
| W-48 | Walls | Thickness = sum of layers | Works | defaults.rs thickness_of | - |
| W-49 | Walls | Editing a type re-flows walls | Works | walls.rs set_wall_type | - |
| W-50 | Walls | Layer lines in plan and toggle | Partial | render.rs wall layer bands | Check Display Options Wall Layers toggle coverage. |
| W-51 | Walls | Defaults copied at draw time | Works | wall.rs create() | - |
| W-52 | Walls | Foundation wall | Partial | WallClass::Foundation; tools/wall.rs | Footing/slab/sill options (Foundation tab) disabled. |
| W-53 | Walls | Pony wall | Works | PonyWall in walls.rs; dialogs/wall.rs | - |
| W-54 | Walls | Half-wall | Works | WallClass::Half | Distinct dashed plan style unverified. |
| W-55 | Walls | Room divider / invisible wall | Works | wall.rs test room_dividers_close_rooms | - |
| W-56 | Walls | Railing wall | Works | rail style tab; plan-3d | - |
| W-57 | Walls | Glass wall and glass pony wall | Works | wall.rs tests; Glass-1 type | Curved glass variants absent. |
| W-58 | Walls | Slab footing | Works | tools/details Slab Footing | - |
| W-59 | Walls | Wall hatching and material region | Works | tools/details.rs | - |
| W-60 | Walls | Heights follow platform defaults | Partial | dialogs/wall.rs Structure Default Wall Top Height | Bottom height default disabled. |
| W-61 | Walls | Default ceiling 109.125 in | Works | DEFAULT_CEILING_HEIGHT | - |
| W-62 | Walls | Platform intersection options | Missing | dialogs/wall.rs disabled | Framing-only option, disabled. |
| W-63 | Walls | Generate invisible walls between platforms | Missing | disabled | Not built. |
| W-64 | Walls | Curved wall three-click draw | Works | wall.rs tests curved_variants_take_a_third_click_for_the_arc | No radius readout. |
| W-65 | Walls | True arcs faceted by facet angle | Works | WallCurve; 7.5 deg facets in plan-3d | - |
| W-66 | Walls | Curved wall dialog radius/lock | Missing | dialogs/wall.rs Curved section disabled | No numeric arc edit. |
| W-67 | Walls | Change Line/Arc on walls | Missing | none | No bulge handle either. |
| W-68 | Walls | Make Arc Tangent | Missing | none | Not built. |
| W-69 | Walls | Room from inside faces | Works | rooms.rs interior polygon; QA-03 fix | - |
| W-70 | Walls | Label area finished-floor | Works | QA-03 fixed, interior area | - |
| W-71 | Walls | Room labels name and area | Works | rooms_edit.rs | - |
| W-72 | Walls | Rooms update live | Works | EditorContext::refresh | - |
| W-73 | Walls | Auto exterior dims locate per defaults | Partial | tools/dimension.rs AutoExterior | Locate options partly honoured (see DIM). |
| W-74 | Walls | Wall temp dim along baseline | Works | tempdim.rs WallLength | - |
| W-75 | Walls | Length edit with Lock | Works | dialogs/wall.rs tests length_lock_* | - |
| W-76 | Walls | Angle edit re-solves joined walls | Partial | manual 2.6 | Rotates about start; joined walls not moved. |
| W-77 | Walls | OK blocked if openings no longer fit | Works | dialogs/wall.rs too_short | - |
| W-78 | Walls | Openings keep distance from locked end | Works | dialogs/wall.rs adjusted_openings | - |
| W-79 | Walls | Wall Types tab switch | Works | dialogs/wall.rs | Library button disabled. |
| W-80 | Walls | Layer tab | Partial | dialogs/wall.rs | Drawing Group disabled. |
| W-81 | Walls | Label tab | Partial | dialogs/wall.rs Label | Border/text style/alignment disabled. |
| W-82 | Walls | OK one undo step, preview pane | Works | dialogs/wall.rs; spec_dialogs | - |
| W-83 | Walls | Multi-wall Open Object | Missing | spec_dialogs.rs single-object only | No indeterminate multi-edit. |
| W-84 | Walls | Openings hosted by wall | Works | Project::remove_wall | - |
| W-85 | Walls | Opening clamped after wall edit | Partial | select.rs end drag | Clamp/delete warning not systematic. |
| W-86 | Walls | Break splits openings; merge re-hosts | Partial | split_wall_at | No merge. |
| W-87 | Walls | Thickness change keeps opening centers | Works | openings.rs depth follows wall | - |
| W-88 | Walls | Opening cutout across layers in plan | Works | render.rs draw_openings | - |
| W-89 | Walls | CAD to Walls | Works | menus.rs CAD to Walls; plan_import::cad_to_walls | - |
| W-90 | Walls | Convert wall to polyline | Missing | none | Not built. |
| W-91 | Walls | Rebuild Walls/Floors/Ceilings | Works | Action::RebuildAll (toolbar.rs:974) | - |
| W-92 | Walls | Hatch/region use CAD polyline rules | Works | details_view.rs | - |
| W-93 | Walls | Hidden layer wall not drawn or snapped, still defines rooms | Partial | layers.is_visible in render | Verify rooms still use hidden walls. |
| W-94 | Walls | Exterior vs interior fills | Works | render.rs palette | - |
| W-95 | Walls | Plot line weights | Works | restyle.rs, View Line Weights | - |
| W-96 | Walls | Duplicate walls merged | Works | connect.rs | - |
| W-97 | Walls | Acceptance: rectangle of 4 walls, 1 room | Works | scenarios/s01_house_shell.rs | - |
| W-98 | Walls | Acceptance: interior wall T both ends, 2 rooms | Partial | scenarios/s03_interior.rs | Exterior walls split by Ts. |
| W-99 | Walls | Acceptance: typed distance moves wall | Works | select.rs test typing_a_temporary_dimension_moves_the_wall | - |
| W-100 | Walls | Acceptance: type change keeps framing fixed | Works | walls.rs test set_wall_type_holds_reference_line | - |
| W-101 | Walls | Free ends square | Works | joins.rs | - |
| W-102 | Walls | Different thickness L corners | Works | joins.rs wall_layer_outlines | - |
| W-103 | Walls | End on wall end gives 3-way | Partial | joins.rs | Square ends at 3-way (see W-34). |
| W-104 | Walls | Delete one wall of pair leaves free end | Works | ops delete_objects | - |
| W-105 | Walls | Dragging through wall breaks T | Partial | select.rs end follows | Butting end not auto-following through wall move verified. |

### Select and edit (`docs/parity/select-and-edit.md`)

112 ids: 51 Works, 32 Partial, 28 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| S-1 | Select | Click selects, shows handles and Edit toolbar | Works | tools/select.rs test click_selects_and_empty_click_deselects | - |
| S-2 | Select | Empty click deselects, selected click keeps | Works | select.rs same test | - |
| S-3 | Select | Pick by drawn geometry | Works | editor/selection.rs hit_test_cx | Text/door swing-arc tolerance not individually verified. |
| S-4 | Select | Overlap order, openings beat walls | Works | selection.rs hit_test_cx tiers | - |
| S-5 | Select | Hidden layer unselectable; locked layer viewable not editable | Works | editor/actions.rs check_unlocked, is_locked | - |
| S-6 | Select | Hover highlight and status description | Partial | select.rs update_hover; cx.hover | Hover highlight yes; status line does not name the object. |
| S-7 | Select | Double-click or Open Object opens spec | Works | select.rs double_click; shell/spec_dialogs.rs | - |
| S-8 | Select | Right-click context menu | Missing | main.rs:692 right-click only ends a tool chain | No context menu on objects or empty space. |
| S-9 | Select | Esc clears selection when idle | Works | select.rs key() | - |
| S-10 | Select | Space toggles Select; Esc returns to Select | Works | toolbar.rs bindings; scenarios s12_hotkeys | - |
| S-11 | Select | Handles only for single object | Works | handles.rs handles_for | - |
| S-12 | Select | Fixed-size handles, cursors, handle priority | Works | handles.rs cursor icons; hit_handle first | - |
| S-13 | Select | Move handle with snaps; opening re-hosts | Works | select.rs Op::OpeningSlide, Op::Group | - |
| S-14 | Select | Resize handles, opposite anchored | Partial | handles.rs ResizeStart/End | Ends of lines and walls only; no box corner and side handles or Shift proportions. |
| S-15 | Select | Rotate handle with 15 deg snap and readout | Partial | select.rs Op::CadRotate | CAD items only; no rotate for other objects, no angle readout verified. |
| S-16 | Select | Reshape vertex handles, mid-edge handle | Partial | handles.rs Reshape(i) | Vertex handles yes; mid-edge move handle absent. |
| S-17 | Select | Line, arc, circle handles | Works | handles.rs CAD handles; select.rs CadVertex | - |
| S-18 | Select | Wall handles: move, ends, curve | Partial | handles.rs:63; manual 2.3 | No bulge handle on curved walls. |
| S-19 | Select | End drag with joined walls following | Works | select.rs test end_handle_stretches_with_the_connected_wall_and_splits_on_drop | - |
| S-20 | Select | End onto wall makes T; onto end makes corner | Works | same test | - |
| S-21 | Select | Perpendicular-only wall move, neighbors stretch | Works | select.rs test perpendicular_drag_moves_a_wall_and_keeps_neighbours_attached | - |
| S-22 | Select | Alt free move | Works | select.rs module doc | - |
| S-23 | Select | Live temp dims and snap to faces during move | Partial | tempdim.rs; select.rs apply | Snapping to neighbor faces not confirmed. |
| S-24 | Select | Door/window resize handles, swing handle | Partial | handles.rs; select.rs Op::Swing | Swing flip and slide yes; width resize handles missing (DW). |
| S-25 | Select | Text move, wrap-width resize, rotate | Partial | handles.rs Cad/Text | Move and rotate; no wrap-width resize handle. |
| S-26 | Select | Drag threshold | Works | select.rs DRAG_THRESHOLD_PX | - |
| S-27 | Select | One undo step per drag, Esc cancels | Works | select.rs test escape_cancels_a_drag | - |
| S-28 | Select | Tab or typing during drag opens entry | Missing | none | Typed entry only via temp dim click after selection. |
| S-29 | Select | Marquee window vs crossing | Works | select.rs test marquee_window_and_crossing | - |
| S-30 | Select | Marquee from over an object with Alt | Partial | select.rs | Body drag wins; Alt marquee not seen. |
| S-31 | Select | Shift add and remove | Works | select.rs pointer_down | - |
| S-32 | Select | Select Same Type | Missing | grep finds none | Not built. |
| S-33 | Select | Select All (Cmd+A) | Missing | menus.rs:294 inert | No Select All. |
| S-34 | Select | Tab/Shift+Tab cycling | Works | select.rs test tab_cycles_through_the_objects_under_the_pointer | - |
| S-35 | Select | Group, ungroup | Partial | plan-core groups.rs make_group/explode_group | Model only; no Edit-toolbar buttons or group-aware click. |
| S-36 | Select | Select group member | Missing | none | Depends on S-35 UI. |
| S-37 | Select | Multi-select drag moves all | Works | select.rs Op::Group | - |
| S-38 | Select | Edit toolbar shows common commands for multi | Works | actions.rs common_edit_actions | Small command set. |
| S-39 | Select | Floating contextual Edit toolbar | Works | editor/actions.rs; tools/select.rs edit_toolbar | Fixed set per type. |
| S-40 | Select | Common buttons Transform, Reflect, Center, P2P, Layer, Same Type | Partial | actions.rs: Open, Delete, Copy, Paste in Place only | Everything beyond four buttons is missing. |
| S-41 | Select | Straight wall buttons list | Partial | select.rs edit_toolbar | Only Fix Wall Connections extra; Break, Change Line/Arc, Reverse Layers, Parallel absent. |
| S-42 | Select | Curved wall buttons | Missing | none | Change Line/Arc and Make Arc Tangent absent. |
| S-43 | Select | Door/window buttons | Partial | EditActionKind::ReverseSwing | Reverse swing yes; Center, mulling absent. |
| S-44 | Select | Text buttons | Partial | actions.rs | Open/Copy/Delete only. |
| S-45 | Select | CAD line buttons | Partial | tools/cad/edit.rs modes (via CAD menu) | Parallel/Perp/Break/Convert are CAD-menu tools not Edit-toolbar buttons. |
| S-46 | Select | Each command one undo step | Works | begin_change in actions | - |
| S-47 | Select | Transform/Replicate Object | Missing | grep finds none | No dialog; Move/Copy/Rotate/Resize arrays absent. |
| S-48 | Select | Reflect About Object/Line | Missing | none | Not built. |
| S-49 | Select | Make Parallel/Perpendicular | Partial | tools/cad/edit.rs for CAD lines | Not for walls. |
| S-50 | Select | Break Line/Wall | Partial | cad edit Break Line; walls: core split_wall_at only | No wall Break button. |
| S-51 | Select | Fix Wall Connections on selection | Works | connect.rs fix_wall_connections_action | - |
| S-52 | Select | Point to Point Move | Missing | none | Not built. |
| S-53 | Select | Center Object | Missing | none | Not built. |
| S-54 | Select | Align/Distribute | Missing | none | Not built. |
| S-55 | Select | Reverse Layers | Missing | none | Not built. |
| S-56 | Select | Temp dims for selected wall | Works | editor/tempdim.rs | - |
| S-57 | Select | Door/window jamb dims | Works | tempdim.rs OpeningToStart/End | Opening width entry absent. |
| S-58 | Select | CAD line/box/circle temp dims | Missing | tempdim.rs only wall and opening kinds | None for CAD. |
| S-59 | Select | Click value, type, Enter, Tab, Esc | Works | select.rs key_while_editing | - |
| S-60 | Select | Referenced fixed, selected moves | Works | tempdim.rs apply | - |
| S-61 | Select | Wall length follows Lock | Works | tempdim.rs WallLength | - |
| S-62 | Select | Face-to-face to surfaces per Locate | Partial | tempdim.rs WallGap | Face-based; center-to-center option not offered. |
| S-63 | Select | Lockable temp dimensions | Missing | none | Not built. |
| S-64 | Select | Temp dims never printed or saved | Works | tempdim.rs | - |
| S-65 | Select | Edit Behaviors (Replicate, Resize, Concentric) | Missing | menus.rs:297 inert | Not built. |
| S-66 | Select | Behavior indicator, reset | Missing | none | Depends on S-65. |
| S-67 | Select | Fillet/chamfer on CAD, not walls | Works | tools/cad/edit.rs | - |
| S-68 | Select | Object snap types with on/off | Partial | editor/snap.rs SnapSettings | Engine has per-type flags; Snap Settings dialog is inert. |
| S-69 | Select | Snap priority | Works | snap.rs header | Chief order has Center/Quadrant/Tangent; ours lacks them. |
| S-70 | Select | Snap distance screen-space | Works | EditorContext::snap_tol | - |
| S-71 | Select | Angle snaps incl. parallel/perpendicular | Works | snap.rs angle_snap; wall.rs | Angle Snap Grid guide not drawn. |
| S-72 | Select | Grid snap unit | Works | defaults grid.snap | - |
| S-73 | Select | Bumping/pushing | Partial | tools/cabinet.rs neighbor alignment | Cabinets bump; no pushing. |
| S-74 | Select | Alt suspends all snaps | Partial | snap.rs header | Angle snap only. |
| S-75 | Select | Cmd+Z/Cmd+Y undo and redo | Works | plan-core history.rs; scenarios s12_hotkeys | - |
| S-76 | Select | One gesture one step | Works | begin_change | - |
| S-77 | Select | Undo restores selection | Partial | selection.retain_existing | Selection kept only if object still exists. |
| S-78 | Select | Undo levels | Differs-by-design | history.rs cap 100 | Chief default 20; we keep 100. |
| S-79 | Select | Action History panel | Missing | menus.rs:822 inert | Undo labels in menu only. |
| S-80 | Select | Spec edits undoable | Works | spec_dialogs one step | - |
| S-81 | Select | Copy/Cut/Paste with reference point | Partial | actions.rs copy_selection, paste_in_place | Copy and Paste in Place only; no Cut, no Cmd+C/V, no cursor-attached paste. |
| S-82 | Select | Paste attaches to cursor | Missing | none | Not built. |
| S-83 | Select | Paste Hold Position | Missing | toolbar.rs:1927 stub button | Paste in Place covers same-position paste. |
| S-84 | Select | Copy and Paste in Place (C,P,P) | Partial | Edit toolbar Copy + Paste in Place (actions.rs); dispatch.rs test | The C,P,P chord is only a menu label (menus.rs:275, inert); two button clicks instead. |
| S-85 | Select | Paste across files keeps layers | Partial | clipboard in memory only | No cross-file clipboard. |
| S-86 | Select | Rooms not copied | Works | groups.rs Clipboard | - |
| S-87 | Select | Delete multi, hosted openings | Works | select.rs test delete_removes_the_selection_with_its_openings_and_undoes | - |
| S-88 | Select | Delete Objects category dialog | Missing | menus.rs:279 inert | Not built. |
| S-89 | Select | Locked layer delete refused | Works | actions.rs delete_selection | - |
| S-90 | Select | Edit Area commands | Missing | menus.rs:300 inert | Not built. |
| S-91 | Select | Stretch CAD | Missing | menus.rs:301 inert | Not built. |
| S-92 | Select | Arrow-key nudge | Works | select.rs nudge() | - |
| S-93 | Select | Enter opens spec; Delete deletes | Works | select.rs key() | Cmd+D duplicate absent. |
| S-94 | Select | Modifier table | Partial | select.rs | Shift and Alt only; Ctrl-drag copy absent. |
| S-95 | Select | Cursor shapes | Works | handles.rs CursorIcon | - |
| S-96 | Select | Fill Window Selected Objects | Partial | main.rs:155 fill_window; toolbar.rs:1971 | Fits whole plan; selected variant stub. |
| S-97 | Select | Selection highlight color | Works | render.rs | - |
| S-98 | Select | Status bar selection count and XYZ | Partial | main.rs:975 status_bar | X/Y only; no selection description or Z. |
| S-99 | Select | Auto-scroll on edge drag | Missing | none | Not built. |
| S-100 | Select | Esc cancels drag or deselects | Works | select.rs | - |
| S-101 | Select | Rotate about center / chosen center | Partial | select.rs Op::CadRotate | CAD objects only. |
| S-102 | Select | Rotate multi-selection | Missing | none | Not built. |
| S-103 | Select | Transform/Replicate dialog fields | Missing | none | See S-47. |
| S-104 | Select | Linear/radial arrays | Missing | none | See S-47. |
| S-105 | Select | Reflect with copy | Missing | none | See S-48. |
| S-106 | Select | Reflected doors flip | Missing | none | See S-48. |
| S-107 | Select | Rooms selectable, spec on double-click | Works | select.rs test clicking_inside_a_room_selects_the_room_and_double_click_opens_it | - |
| S-108 | Select | Dimension handles, text move, add/remove point | Partial | handles.rs:106 | Move handle; extension-end resize and add/delete extension point limited. |
| S-109 | Select | Wall in loop selects alone | Works | select.rs | - |
| S-110 | Select | Hosted opening highlights host wall | Partial | selection | Not confirmed. |
| S-111 | Select | Reference display unselectable | Works | render.rs reference floor drawn separately | - |
| S-112 | Select | CAD layer order pick | Partial | selection.rs tiers | Active-layer favoring not confirmed. |

### Doors and windows (`docs/parity/doors-windows.md`)

110 ids: 42 Works, 34 Partial, 33 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| DW-1 | Doors/Windows | Placement tools place per click, tool stays active | Works | tools/opening.rs pointer_down; scenarios s02_openings | - |
| DW-2 | Doors/Windows | Ghost follows pointer with swing arc | Works | tools/opening.rs draw_overlay -> render::draw_opening(ghost) | - |
| DW-3 | Doors/Windows | No ghost off wall, no placement | Partial | opening.rs target() | No ghost; no "no" cursor glyph. |
| DW-4 | Doors/Windows | Refuse short wall/overlap; windows may touch and mull | Partial | s02_openings refused placement; opening.rs test refused_placement_leaves_no_undo_step | Touching windows refused (2 in gap); no mulling. |
| DW-5 | Doors/Windows | Click places; drag after click slides new opening | Missing | opening.rs pointer_down only | No placement-drag. |
| DW-6 | Doors/Windows | Defaults by type, exterior vs interior wall | Works | opening.rs test doors_and_windows_come_from_the_templates | - |
| DW-7 | Doors/Windows | Captured interior door defaults | Works | defaults.rs; opening.rs test interior_walls_get_the_interior_door | - |
| DW-8 | Doors/Windows | Swing side from pointer, hinge nearer end | Works | plan-core openings.rs door_defaults_for_pointer; QA-01 fixed | - |
| DW-9 | Doors/Windows | Center under pointer, 1 in snap | Works | opening.rs target() | Alt skips grid. |
| DW-10 | Doors/Windows | Alignment candidates (midpoint, equal spacing) | Missing | opening.rs target() grid only | No midpoint or equal-spacing snaps. |
| DW-11 | Doors/Windows | Temp dims to wall ends, neighbours, width | Partial | tempdim.rs jamb_dims (OpeningToStart/End) | No neighbour or width dimension. |
| DW-12 | Doors/Windows | Type into jamb dims moves; width dim resizes | Partial | tempdim.rs | Jamb dims only; width not editable. |
| DW-13 | Doors/Windows | End clearance, 0 at intersecting wall | Differs-by-design | openings.rs OPENING_MARGIN 2 in | Flat 2 in; spec says verify in Chief. |
| DW-14 | Doors/Windows | Distance-from-wall-start field | Works | dialogs/opening.rs | - |
| DW-15 | Doors/Windows | Not placed if no fit, never shrunk | Works | Project::add_opening | - |
| DW-16 | Doors/Windows | Move handle slides along wall | Works | select.rs test dragging_an_opening_slides_it | - |
| DW-17 | Doors/Windows | Body drag moves | Works | select.rs Op::OpeningSlide | - |
| DW-18 | Doors/Windows | Re-host onto another wall | Works | select.rs OpeningSlide (manual 3.2) | - |
| DW-19 | Doors/Windows | Drag through wall flips swing | Missing | none | Not built. |
| DW-20 | Doors/Windows | Stops at clearance | Works | Project::slide_opening | - |
| DW-21 | Doors/Windows | Arrow nudge along wall | Works | select.rs nudge() slide_opening_by | - |
| DW-22 | Doors/Windows | Copy/paste opening onto a wall | Partial | actions.rs copy_selection (Opening) | Paste in Place only; not cursor-attached. |
| DW-23 | Doors/Windows | Reflect mirrors openings | Missing | none | No Reflect. |
| DW-24 | Doors/Windows | Center Object | Partial | dialogs/opening.rs Center on wall button | No Edit-toolbar Center Object. |
| DW-25 | Doors/Windows | One undo step per drag | Works | select.rs finish | - |
| DW-26 | Doors/Windows | Jamb resize handles | Missing | manual 3.2 "not built" | No resize handles. |
| DW-27 | Doors/Windows | Resize snaps to grid/library widths | Missing | none | Depends on DW-26. |
| DW-28 | Doors/Windows | Resize clamps | Missing | none | Depends on DW-26. |
| DW-29 | Doors/Windows | Height/sill via dialog | Works | dialogs/opening.rs Floor to Top/Bottom | - |
| DW-30 | Doors/Windows | Door panels calculated from width | Missing | dialogs/opening.rs Door Panels disabled | Not built. |
| DW-31 | Doors/Windows | Swing and hinge as four combinations | Works | Opening.swing_flipped + hinge_at_end | - |
| DW-32 | Doors/Windows | Reverse Swing and Flip Hinge buttons | Partial | EditActionKind::ReverseSwing; Project::flip_hinge | Hinge flip only in dialog, no button. |
| DW-33 | Doors/Windows | Swing handle at leaf end | Partial | select.rs Op::Swing | Click flips swing; hinge toggle handle absent. |
| DW-34 | Doors/Windows | Swing Angle drives plan arc | Partial | dialogs/opening.rs swing_angle stored; render.rs draw_opening fixed 90 deg | Angle not drawn. |
| DW-35 | Doors/Windows | Both-direction swing arcs | Missing | none | Not drawn. |
| DW-36 | Doors/Windows | Show Open in 2D | Partial | extras.rs show_open_in_plan stored | Render ignores the flag. |
| DW-37 | Doors/Windows | Swing Toward Exterior toggle absent in Chief | Works | n/a | No gap. |
| DW-38 | Doors/Windows | Hinged symbol single and double | Partial | render.rs draw_opening | Single leaf only; no double doors. |
| DW-39 | Doors/Windows | Doorway symbol | Missing | toolbar.rs:704 todo | Tool stub; no plan symbol. |
| DW-40 | Doors/Windows | Sliding symbol | Missing | toolbar.rs:705 todo | Tool stub; 3D only (plan-3d doors.rs). |
| DW-41 | Doors/Windows | Pocket symbol | Missing | toolbar.rs:706 todo | Tool stub. |
| DW-42 | Doors/Windows | Bifold symbol | Missing | toolbar.rs:707 todo | Tool stub; 3D only. |
| DW-43 | Doors/Windows | Garage symbol | Missing | toolbar.rs:710 todo | Tool stub. |
| DW-44 | Doors/Windows | Barn door | Missing | toolbar.rs:708 todo | Tool stub. |
| DW-45 | Doors/Windows | Shower and Fixed door | Missing | toolbar.rs:709,711 todo | Tool stubs. |
| DW-46 | Doors/Windows | Change door type in dialog updates symbol | Partial | dialogs/opening.rs Door Style list | Style kept per session; plan symbol unchanged. |
| DW-47 | Doors/Windows | Window symbol, types | Partial | render.rs three-line window | Window type affects nothing in plan. |
| DW-48 | Doors/Windows | Bay, bow, box windows | Missing | toolbar.rs:721-723 todo | Tool stubs; 3D supports styles (plan-3d opening.rs:81). |
| DW-49 | Doors/Windows | Pass-through, wall niche | Missing | toolbar.rs:724-725 todo | Tool stubs; niche depth in plan-3d wall.rs. |
| DW-50 | Doors/Windows | Window sill default | Works | defaults template 24+72 | Manual: head 96 in. |
| DW-51 | Doors/Windows | Mulled windows | Missing | none | No Mull command. |
| DW-52 | Doors/Windows | Door plus sidelite mull | Missing | none | Same as DW-51. |
| DW-53 | Doors/Windows | Window cannot span a junction | Works | Project::add_opening bounds; split_wall_at refuses | - |
| DW-54 | Doors/Windows | Size fields recompute | Works | dialogs/opening.rs | Elevation Reference disabled. |
| DW-55 | Doors/Windows | Library door sets style; Components | Partial | Library Style stored | Components tab disabled. |
| DW-56 | Doors/Windows | Clearance gaps | Missing | dialogs disabled | 3D only in Chief. |
| DW-57 | Doors/Windows | Depth follows wall; Recessed into Wall | Partial | Options disabled sections | Recessed not supported. |
| DW-58 | Doors/Windows | Openings in curved walls | Partial | manual 2.2 offsets along arc | No 3D cut, no radial symbol. |
| DW-59 | Doors/Windows | Plan label with size shorthand | Partial | plan-core openings.rs auto_label | Not drawn on plan except schedule callouts. |
| DW-60 | Doors/Windows | Schedule number in label | Partial | schedule_view.rs D01 callouts | Callouts from a placed schedule, not label option. |
| DW-61 | Doors/Windows | Schedule numbering/renumber | Partial | Opening.schedule_number | No automatic assignment on placement; no Renumber. |
| DW-62 | Doors/Windows | Specify Label with macros | Partial | dialogs/opening.rs Label | Session only, not drawn. |
| DW-63 | Doors/Windows | Label placement/layer/handle | Missing | none | Not drawn. |
| DW-64 | Doors/Windows | Tab cycling and temp dim Tab | Works | select.rs cycle(); tempdim next_field | - |
| DW-65 | Doors/Windows | Hover description and size | Missing | select.rs hover | Status does not describe object. |
| DW-66 | Doors/Windows | Cut across all layers, jambs drawn | Works | render.rs draw_opening | - |
| DW-67 | Doors/Windows | Openings do not break rooms | Works | rooms.rs | - |
| DW-68 | Doors/Windows | Openings follow wall | Works | ops.rs wall move | - |
| DW-69 | Doors/Windows | Break splits openings; straddle refused | Partial | walls.rs split_wall_at | No UI for Break. |
| DW-70 | Doors/Windows | Single host | Works | Opening.wall_id | - |
| DW-71 | Doors/Windows | Host picking | Partial | opening.rs target(): nearest centerline within pick tol | Not footprint-based. |
| DW-72 | Doors/Windows | Project and grid snap | Works | opening.rs target | - |
| DW-73 | Doors/Windows | Allowed range | Works | Project::add_opening | - |
| DW-74 | Doors/Windows | Neighbor blocked intervals | Partial | add_opening overlap rule | No window mull gap rule. |
| DW-75 | Doors/Windows | Alignment candidates | Missing | none | See DW-10. |
| DW-76 | Doors/Windows | Swing/hinge derivation | Works | door_defaults_for_pointer | - |
| DW-77 | Doors/Windows | Create, number, select, one undo | Partial | opening.rs | No schedule number assigned. |
| DW-78 | Doors/Windows | Door on window refused | Works | overlap rule | - |
| DW-79 | Doors/Windows | Casing drawn in plan | Missing | casing 3D only (plan-3d casing.rs) | Plan draws no casing rectangles. |
| DW-80 | Doors/Windows | Plinth blocks elevation | Missing | disabled | Not built. |
| DW-81 | Doors/Windows | Threshold line | Missing | none | Not drawn. |
| DW-82 | Doors/Windows | Jamb thickness / size includes jamb | Partial | Jamb tab stores width | Not drawn in plan. |
| DW-83 | Doors/Windows | Opening Indicators | Missing | tab disabled | Not built. |
| DW-84 | Doors/Windows | Shutters in plan | Missing | tab disabled | Not built. |
| DW-85 | Doors/Windows | Sill line past exterior face | Missing | none | Not drawn. |
| DW-86 | Doors/Windows | Egress flag metadata | Works | extras egress; plan-check bedroom_egress | - |
| DW-87 | Doors/Windows | Refused across junction | Partial | add_opening vs wall ends | T through-wall face clearance not modelled. |
| DW-88 | Doors/Windows | Curved wall opening symbol radial | Missing | manual 2.2 | Not drawn radially. |
| DW-89 | Doors/Windows | Shared segment windows | Partial | none | No mulling exemption. |
| DW-90 | Doors/Windows | Garage door too wide refused | Works | add_opening | - |
| DW-91 | Doors/Windows | Foundation wall openings | Partial | same tool | No foundation-specific defaults. |
| DW-92 | Doors/Windows | No openings on invisible walls | Missing | none | Not restricted. |
| DW-93 | Doors/Windows | Delete opening re-closes wall | Works | select.rs delete | - |
| DW-94 | Doors/Windows | Undo releases number | Partial | undo works | Moot until numbers assigned. |
| DW-95 | Doors/Windows | Acceptance: centered door, swing, dims | Works | scenarios/s02_openings | - |
| DW-96 | Doors/Windows | Acceptance: slide with snap | Works | select.rs test dragging_an_opening_slides_it | - |
| DW-97 | Doors/Windows | Acceptance: Reverse Swing, Flip Hinge | Partial | Reverse Swing button | Flip Hinge button missing. |
| DW-98 | Doors/Windows | Acceptance: mulled windows | Missing | none | Not built. |
| DW-99 | Doors/Windows | Acceptance: Lock End keeps distances | Works | dialogs/wall.rs test length_lock_end_keeps_openings_in_place | - |
| DW-100 | Doors/Windows | Acceptance: save and reopen preserves | Partial | plan-core extras.rs round-trips | Label/casing options session-only. |
| DW-101 | Doors/Windows | Spec dialog with preview | Works | dialogs/opening.rs | - |
| DW-102 | Doors/Windows | Width edit clamps with warning | Works | dialogs/opening.rs clamp_center | - |
| DW-103 | Doors/Windows | Style swaps defaults | Partial | dialogs/opening.rs | Style change keeps size. |
| DW-104 | Doors/Windows | Label tab live preview | Works | dialogs/opening.rs | - |
| DW-105 | Doors/Windows | Defaults never alter placed | Works | cx.opening_template | - |
| DW-106 | Doors/Windows | Cancel/OK one undo | Works | scenarios s02 | - |
| DW-107 | Doors/Windows | Door/window hotkeys | Partial | toolbar.rs bindings | D,H and Shift+W live; other chords belong to stub tools. |
| DW-108 | Doors/Windows | Flyout face and hint | Works | toolbar.rs; opening.rs hint | - |
| DW-109 | Doors/Windows | Esc returns to Select; right-click menu | Partial | main.rs:692 | No context menu. |
| DW-110 | Doors/Windows | Alt disables alignment snaps | Works | opening.rs target(alt) | - |

### Rooms and floors (`docs/parity/rooms-floors.md`)

71 ids: 37 Works, 28 Partial, 6 Missing, 0 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| R-1 | Rooms/Floors | Rooms auto-detected from closed wall loops | Works | plan-core rooms.rs detect_rooms; scenarios s01 | - |
| R-2 | Rooms/Floors | Room polygon at interior surfaces | Works | rooms.rs interior polygon; QA-03 fixed | - |
| R-3 | Rooms/Floors | Which walls bound rooms, railing rule | Works | rooms.rs WallFlags::defines_rooms | Railing-tool vs railing-wall distinction unverified. |
| R-4 | Rooms/Floors | Room Divider invisible wall | Works | wall.rs test room_dividers_close_rooms | - |
| R-5 | Rooms/Floors | Invisible walls still bound rooms | Works | WallFlags | - |
| R-6 | Rooms/Floors | Per-floor detection | Works | rooms.rs | - |
| R-7 | Rooms/Floors | Dangling wall excluded from area | Partial | manual 4.1: dead-end does not split | Area includes the dangling wall body. |
| R-8 | Rooms/Floors | Openings do not break rooms | Works | rooms.rs | - |
| R-9 | Rooms/Floors | T splits, crossing makes four | Works | rooms.rs; connect.rs | - |
| R-10 | Rooms/Floors | Gaps leave no room | Works | rooms.rs tolerance | - |
| R-11 | Rooms/Floors | Nested rooms ring polygon | Partial | manual 4.9: no hole cut | Island is own room; outer area not reduced, no platform hole. |
| R-12 | Rooms/Floors | Curved walls bound rooms | Works | rooms.rs expand_curves | - |
| R-13 | Rooms/Floors | Default name from room type | Works | rooms_edit.rs "Room N" | - |
| R-14 | Rooms/Floors | Stable room identity | Partial | RoomName.anchor point | Name tied to an interior point, not a room id. |
| R-15 | Rooms/Floors | Split/join keeps larger identity | Partial | anchor logic | Piece holding the anchor keeps the name. |
| R-16 | Rooms/Floors | Click selects room | Works | select.rs test clicking_inside_a_room_selects_the_room | Handles on inner polygon not confirmed. |
| R-17 | Rooms/Floors | Rooms only by walls | Works | design | - |
| R-18 | Rooms/Floors | Foundation rooms | Partial | Foundation floor kind | Crawl/Basement function behavior absent. |
| R-19 | Rooms/Floors | Room Specification dialog tabs | Works | dialogs/room.rs | Deck tab disabled. |
| R-20 | Rooms/Floors | General fields | Partial | dialogs/room.rs | Function shown read-only (follows type). |
| R-21 | Rooms/Floors | Use Default follows type | Works | RoomName Option fields | - |
| R-22 | Rooms/Floors | Structure tab | Works | dialogs/room.rs | Thickness fields session only. |
| R-23 | Rooms/Floors | Floor height absolute/relative | Works | RoomName.floor_height_offset; plan-3d slab.rs room_levels | - |
| R-24 | Rooms/Floors | Ceiling height override | Works | QA-02 fixed; slab.rs | Absolute/Relative toggle session only. |
| R-25 | Rooms/Floors | Rough ceiling | Partial | RoomName.rough_ceiling stored | 3D ignores it. |
| R-26 | Rooms/Floors | Stem wall at dropped room | Partial | RoomName.stem_wall_height stored | No stem wall geometry in 3D. |
| R-27 | Rooms/Floors | Floor and ceiling finish thickness | Partial | RoomName finish names | Thickness not stored. |
| R-28 | Rooms/Floors | Floor Structure Define dialog | Missing | none; Components read-only | Not built. |
| R-29 | Rooms/Floors | Ceiling Structure Define | Missing | none | Not built. |
| R-30 | Rooms/Floors | Floor/Ceiling/Roof Over This Room | Partial | RoomName.has_floor/has_ceiling; roof_view uses has_ceiling | 3D slab builder ignores has_floor/has_ceiling; Roof Over session only. |
| R-31 | Rooms/Floors | Monolithic slab flag per room | Missing | none | Foundation tool covers slabs separately. |
| R-32 | Rooms/Floors | Sloped ceiling via ceiling planes | Partial | roof_view Build Ceiling Planes | Room Ceiling Height not tied to plane. |
| R-33 | Rooms/Floors | Structure edits update 3D at once | Works | QA-02 fix; project_hash includes room_names | - |
| R-34 | Rooms/Floors | Room moldings | Partial | RoomName.moldings | Base/Crown names stored; 3D molding geometry not built. |
| R-35 | Rooms/Floors | Fill Style | Works | RoomName.fill_style; render | - |
| R-36 | Rooms/Floors | Materials per room | Partial | floor_finish/ceiling_finish | Other surfaces planned. |
| R-37 | Rooms/Floors | Room Types list | Works | dialogs/default_lists.rs | - |
| R-38 | Rooms/Floors | Room Types buttons | Partial | manual 4.10: Add, Edit, Rename, Delete | No Copy, Select All, Clear All. |
| R-39 | Rooms/Floors | Type feeds dropdown and schedules | Works | rooms_edit.rs | - |
| R-40 | Rooms/Floors | Function built-in behavior | Partial | Function stored in room types | No garage drop, deck/porch rules or Open Below cut-outs. |
| R-41 | Rooms/Floors | Function sets checkbox defaults | Missing | none | Not built. |
| R-42 | Rooms/Floors | Living Area override | Works | rooms.rs living_area_sq_ft | - |
| R-43 | Rooms/Floors | Conditioned override | Partial | RoomName.conditioned | Not used by energy/finish schedule. |
| R-44 | Rooms/Floors | Draggable room label | Partial | label at centroid | No label drag handle. |
| R-45 | Rooms/Floors | Label content toggles | Works | extras.rs RoomLabelOptions | - |
| R-46 | Rooms/Floors | Label text style | Partial | Label tab | Text Style disabled. |
| R-47 | Rooms/Floors | Text macros in labels | Partial | tools/text.rs %room.name% | Room label itself not macro-driven. |
| R-48 | Rooms/Floors | Interior dimensions in label | Works | rooms.rs bounding rect | - |
| R-49 | Rooms/Floors | Interior and Standard Area | Works | rooms.rs standard_area_sq_in | - |
| R-50 | Rooms/Floors | Hide label without deleting room | Works | label options | - |
| R-51 | Rooms/Floors | Total Living Area | Works | rooms.rs living_area_sq_ft | - |
| R-52 | Rooms/Floors | Total shown in readouts | Partial | Room dialog shows total | No plan-info readout or schedule footer. |
| R-53 | Rooms/Floors | Garage/deck/porch excluded by default | Works | room types | - |
| R-54 | Rooms/Floors | Area units | Works | units.rs | - |
| R-55 | Rooms/Floors | Floor numbering, foundation, attic | Works | floors.rs FloorKind | Numbering display differs. |
| R-56 | Rooms/Floors | Floor Defaults dialog | Missing | toolbar.rs:1869 stub button | Not built. |
| R-57 | Rooms/Floors | Template floor defaults | Works | FLOOR_PLATFORM_THICKNESS, DEFAULT_CEILING_HEIGHT | - |
| R-58 | Rooms/Floors | Floor height change cascades | Partial | floors.rs restack_floors | Via Rebuild; wall Default Top follows. |
| R-59 | Rooms/Floors | Build New Floor dialog | Partial | floors.rs build_new_floor(copy_exterior) | Two options only; no interior-wall/room copy checkboxes or below. |
| R-60 | Rooms/Floors | Insert and Delete Floor | Works | floors.rs insert_floor_above, delete_floor | Insert only above. |
| R-61 | Rooms/Floors | Build Foundation types | Partial | floors.rs build_foundation; FoundationKind | Walls with footings, monolithic, piers; no basement. |
| R-62 | Rooms/Floors | Pier and monolithic detail | Partial | tools/foundation.rs slabs/piers | Not tied to Build Foundation. |
| R-63 | Rooms/Floors | Delete Foundation | Works | floors.rs | - |
| R-64 | Rooms/Floors | Exchange with floor above/below | Works | floors.rs exchange_floors | - |
| R-65 | Rooms/Floors | Reference Display dialog, dimmed, snappable | Partial | render.rs draw_reference_floor | Floor below walls only; no dialog, floor above, or snapping. |
| R-66 | Rooms/Floors | Reference respects layer visibility | Works | render.rs:495 | - |
| R-67 | Rooms/Floors | Switching floors keeps view/tool | Works | main.rs | - |
| R-68 | Rooms/Floors | Attic floor | Partial | FloorKind::Attic | Not auto-created by Build Roof. |
| R-69 | Rooms/Floors | Platform intersections | Missing | wall Structure tab disabled | Not built. |
| R-70 | Rooms/Floors | Saved views pin a floor | Partial | Project.plan_views | Layer views yes; floor pinning unverified. |
| R-71 | Rooms/Floors | Floor elevation cascade | Partial | floors.rs restack_floors | Stored elevation recomputed on rebuild only. |

### Roofs (`docs/parity/roofs.md`)

60 ids: 28 Works, 19 Partial, 13 Missing, 0 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| RF-1 | Roofs | Build Roof dialog builds editable planes | Works | dialogs/roof.rs; tools/roof.rs Build Roof; scenarios/s06_roof.rs | - |
| RF-2 | Roofs | Dialog options (planes, ceiling planes, framing, auto rebuild, ignore top floor) | Works | dialogs/roof.rs tabs Roof/Options/Materials | Build Framing stored only; Rafters/Trusses radios disabled. |
| RF-3 | Roofs | Roof style presets (Gable, Hip, Shed, Gambrel, Mansard, Dutch, Flat) | Missing | no preset control in dialogs/roof.rs | Gable/Hip only via Gable/Roof Line wall flip. |
| RF-4 | Roofs | Presets write wall directives | Missing | none | Depends on RF-3. |
| RF-5 | Roofs | Roof defaults (pitch, overhang) and per-wall override | Partial | dialogs/roof.rs defaults; walls.rs roof directive | Wall Roof tab disabled; no Default Settings > Roofs page. |
| RF-6 | Roofs | Auto Rebuild protects manual planes | Works | roof_view.rs auto_rebuild | - |
| RF-7 | Roofs | Rebuild replaces auto planes, keeps manual | Works | roof_view.rs build_roof | No confirmation prompt. |
| RF-8 | Roofs | F12 does not rebuild roofs | Works | roof_view.rs RF-8 comment; s06_roof | - |
| RF-9 | Roofs | One roof per roof level (garage + main) | Partial | roof_view.rs one build per floor | Footprint-based; separate wing heights not modelled (no butting). |
| RF-10 | Roofs | Overhang from wall outer face | Works | roof_view.rs:1077 overhang + thick/2 | - |
| RF-11 | Roofs | Hips, valleys, ridges from straight skeleton | Works | plan-roof skeleton.rs; tests | ~1% fallback cases. |
| RF-12 | Roofs | L, T, U footprints watertight | Works | plan-roof tests | - |
| RF-13 | Roofs | Butting roofs and split walls | Missing | none | Not modelled. |
| RF-14 | Roofs | Plane thickness, underside, fascia | Partial | plan-3d roof.rs slab | Fascia fixed nominal 6 in; manual says fascia/soffit planned. |
| RF-15 | Roofs | Eave detail: cut, soffit, tails | Missing | manual 8.3 planned | Not built. |
| RF-16 | Roofs | Roof cuts walls; gable walls reach roof | Missing | plan-3d wall.rs does not clip to roof planes | Walls stay boxes. |
| RF-17 | Roofs | Deterministic rebuild | Works | plan-roof tests | - |
| RF-18 | Roofs | Per-wall roof directives UI | Partial | Wall.roof in walls.rs; tools/roof.rs flip | Wall Roof tab disabled; only Hip/Full Gable via tool. |
| RF-19 | Roofs | Hip wall | Works | roof_view.rs RoofWallKind::Hip | - |
| RF-20 | Roofs | Full gable wall | Partial | EdgeRoofSpec.full_gable_wall; gable rake | Roof honors gable; triangular wall gable not built in 3D. |
| RF-21 | Roofs | Dutch gable | Missing | manual 8.1 not modelled | - |
| RF-22 | Roofs | High shed/gable | Partial | EdgeRoofSpec.high_shed_gable | Treated as shed edge; wall extension absent. |
| RF-23 | Roofs | Knee wall | Missing | manual: not modelled | - |
| RF-24 | Roofs | Extend slope downward | Partial | roof_view.rs EXTEND_SLOPE_DROP | Fixed 24 in drop, not to the wall below. |
| RF-25 | Roofs | Upper pitch break (gambrel, mansard) | Missing | manual 8.1 | Fields in model, not read by builder. |
| RF-26 | Roofs | Overhang length | Works | plan-roof spec.rs | - |
| RF-27 | Roofs | Auto Roof Return | Partial | roof_view.rs RF-27; Roof Return tool | Fixed 24 in; no slope/extend/shadow-board options; flag set from data only. |
| RF-28 | Roofs | Lower wall type if split by butting roof | Missing | none | - |
| RF-29 | Roofs | Bay/box/bow roof attach | Missing | none | Depends on bay windows. |
| RF-30 | Roofs | Only exterior walls feed roof | Works | roof_view.rs | - |
| RF-31 | Roofs | Attic walls auto-generated | Missing | none | Attic flag in model only. |
| RF-32 | Roofs | Attic floor only when built | Works | FloorKind::Attic | - |
| RF-33 | Roofs | Attic walls cut by roof | Missing | none | - |
| RF-34 | Roofs | Dormer walls as walls | Partial | Explode Dormer makes real walls | Auto dormer keeps walls inside the record. |
| RF-35 | Roofs | Roof Plane tool | Works | tools/roof.rs RoofMode::Plane | Rectangular only; no polyline planes. |
| RF-36 | Roofs | Roof Plane Specification | Partial | dialogs/roof.rs | No Structure Define, rafter spacing, eave tabs. |
| RF-37 | Roofs | Manual planes skipped by rebuild | Works | roof_view.rs origin | - |
| RF-38 | Roofs | Edit handles: vertex, move, pitch arrow, rotate | Partial | roof_view.rs vertex/move | No pitch arrow or rotate handle. |
| RF-39 | Roofs | Edit All Roof Planes | Partial | RoofMode::Edit | Single-plane edit; no multi-plane dialog. |
| RF-40 | Roofs | Delete Roof/Ceiling Planes | Works | tools/roof.rs | - |
| RF-41 | Roofs | Join Roof Planes | Works | plan-roof join.rs; roof_view join_planes_record | One edge at a time. |
| RF-42 | Roofs | Roof Hole | Partial | plan-roof hole.rs; s06_roof | Rectangles only. |
| RF-43 | Roofs | Skylight | Works | roof_view.rs skylight; 3D curb/frame/glass | Library skylights not used. |
| RF-44 | Roofs | Gable/Roof Line | Works | plan-roof gable.rs; tools/roof.rs | - |
| RF-45 | Roofs | Ceiling Plane tool | Works | plan-roof ceiling.rs; dialogs/roof.rs CeilingDialog | - |
| RF-46 | Roofs | Build Ceiling Planes | Works | roof_view.rs build_ceiling_planes | Reruns only on Build Roof. |
| RF-47 | Roofs | Ceiling planes in 3D and plan | Partial | plan-3d ceiling_plane_meshes | Not in material list. |
| RF-48 | Roofs | Auto Dormer | Works | plan-roof dormer.rs; dialogs/roof.rs; s06_roof | Not across two planes. |
| RF-49 | Roofs | Auto Floating Dormer | Works | roof_view.rs DormerRecord::floating | - |
| RF-50 | Roofs | Manual dormers | Partial | Explode Dormer then edit | No guided manual workflow. |
| RF-51 | Roofs | Dormer specification and move | Partial | dialogs/roof.rs Dormer tabs | Move along plane via dialog only. |
| RF-52 | Roofs | Roof framing generation | Works | plan-framing roof.rs; Build Framing | - |
| RF-53 | Roofs | Manual rafter, beam, blocking, purlin | Works | tools/framing.rs 19 tools | - |
| RF-54 | Roofs | Roof Truss, direction, truss base, girder | Partial | tools/framing.rs; plan-framing truss.rs | Girder auto-doubling not confirmed. |
| RF-55 | Roofs | Truss types and web layout | Partial | plan-framing truss.rs | Type coverage not verified. |
| RF-56 | Roofs | Ceiling framing with Build All Framing | Works | plan-framing floor.rs; Build All Framing | - |
| RF-57 | Roofs | Framing rebuild keeps manual members | Works | framing_view.rs manual records | - |
| RF-58 | Roofs | Plan display of roof | Partial | roof_view.rs draw (outline, pitch label) | No slope arrows or hip/ridge lines confirmed. |
| RF-59 | Roofs | Roof plane label with pitch | Works | roof_view.rs label | - |
| RF-60 | Roofs | Roof quantities in Materials List | Missing | plan-docs materials.rs has no roof lines | Not built. |

### Dimensions, text, CAD, layers (`docs/parity/dimensions-text-cad.md`)

121 ids: 68 Works, 41 Partial, 11 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| DIM-1 | Dimensions | Dimension objects on manual/automatic layers | Works | render.rs draw_dimension; plan-core dimension.rs | - |
| DIM-2 | Dimensions | Multi-point string with offset | Partial | Dimension stores two measured points; Running/Baseline emit several | Strings are separate dimension objects. |
| DIM-3 | Dimensions | Associative to objects | Missing | manual 5.2, ROADMAP open | Dimensions store raw points. |
| DIM-4 | Dimensions | Locate to wall surfaces per Locate setting | Partial | tools/dimension.rs Located; qa-findings (main-layer faces) | Fixed to main layer faces; no Locate settings page. |
| DIM-5 | Dimensions | No Locate walls skipped | Works | WallFlags.no_locate; dimension.rs | - |
| DIM-6 | Dimensions | Dimension Defaults formats | Works | dialogs/default_lists.rs Saved Dimension Defaults; 14 template sets | Secondary format absent. |
| DIM-7 | Dimensions | Printed-size text and offsets | Missing | manual 5.3 planned | Size in plan inches. |
| DIM-8 | Dimensions | Value text format and placement | Works | render.rs draw_dimension; DimFormat::fmt_len | Text not rotated to read upright on verticals. |
| DIM-9 | Dimensions | Text outside when tight | Missing | render.rs draw_dimension | No flip/leader. |
| DIM-10 | Dimensions | Text override | Works | dialogs/dimension.rs override_text | - |
| DIM-11 | Dimensions | Manual Dimension tool | Works | tools/dimension.rs DimMode::Manual | - |
| DIM-12 | Dimensions | Locate preview and status | Partial | dimension.rs Located | Preview marker; status names snap. |
| DIM-13 | Dimensions | End to End | Works | DimMode::EndToEnd | - |
| DIM-14 | Dimensions | Interior Dimension | Works | DimMode::Interior | - |
| DIM-15 | Dimensions | Point to Point | Works | DimMode::PointToPoint | - |
| DIM-16 | Dimensions | Running | Works | DimMode::Running | - |
| DIM-17 | Dimensions | Baseline | Works | DimMode::Baseline | - |
| DIM-18 | Dimensions | Angular | Works | DimMode::Angular | - |
| DIM-19 | Dimensions | Centerline | Works | DimMode::Centerline | - |
| DIM-20 | Dimensions | Tape Measure | Works | DimMode::TapeMeasure | - |
| DIM-21 | Dimensions | Snaps apply | Works | cx.snap_at | - |
| DIM-22 | Dimensions | Esc cancels, double-click ends | Works | dimension.rs key | - |
| DIM-23 | Dimensions | One undo step | Works | begin_change | - |
| DIM-24 | Dimensions | Auto Exterior Dimensions strings | Partial | dimension.rs AutoExterior | Overall and breakpoint strings; door/window string planned; axis-aligned sides only. |
| DIM-25 | Dimensions | Re-run replaces autos, keeps manual | Works | manual 5.2 | - |
| DIM-26 | Dimensions | Uses Locate; skips No Locate | Partial | dimension.rs | Main-layer faces fixed. |
| DIM-27 | Dimensions | Auto Interior | Works | DimMode::AutoInterior | One horizontal and vertical per room. |
| DIM-28 | Dimensions | Auto Elevation / Story Pole (elevation views) | Differs-by-design | dimension.rs plan-axis strings | Plan strings, not elevation view objects. |
| DIM-29 | Dimensions | Auto dims linked to walls | Missing | no associativity | Replace by re-running tool. |
| DIM-30 | Dimensions | Dimension handles | Works | editor/handles.rs:106 | Add/delete extension point limited. |
| DIM-31 | Dimensions | Dimension Specification | Partial | dialogs/dimension.rs | Format/Arrow/Text tabs disabled (display from defaults). |
| DIM-32 | Dimensions | Edit value moves object | Works | dimension.rs inline edit | - |
| DIM-33 | Dimensions | Same for temp; auto becomes manual | Partial | tempdim.rs | Auto-to-manual conversion not confirmed. |
| DIM-34 | Dimensions | Locked layer refuses | Works | check_unlocked | - |
| DIM-35 | Dimensions | Line stays when object moves | Works | raw points | - |
| DIM-36 | Dimensions | Align/Distribute dims | Missing | none | - |
| DIM-37 | Dimensions | Reverse Dimension | Missing | none | - |
| DIM-38 | Dimensions | Copy with referenced objects | Partial | actions.rs copy dimensions | Copied as free dimensions. |
| DIM-39 | Dimensions | Dimension Specification tabs | Partial | dialogs/dimension.rs | Primary Format/Arrow/Text Style tabs disabled. |
| DIM-40 | Dimensions | Dimension Defaults groups (manual, auto, temp, elevation) | Partial | Saved Dimension Defaults sets (14 from template) | No separate temp/elevation groups. |
| DIM-41 | Dimensions | Edit toolbar for dimensions | Partial | actions.rs | Add/Delete Extension, Reverse, Convert to Manual absent. |
| DIM-42 | Dimensions | Acceptance: auto exterior 20x12 | Works | scenarios/s07_dimensions_text_cad.rs | - |
| DIM-43 | Dimensions | Acceptance: edit dimension text moves wall | Works | s07 / dimension.rs inline edit | - |
| DIM-44 | Dimensions | Acceptance: interior dimension clear span | Works | dimension.rs Interior | - |
| DIM-45 | Dimensions | Acceptance: angular dimension 90 and 135 | Works | dimension.rs Angular | - |
| TXT-1 | Text | Text tool, click-drag box | Partial | tools/text.rs TextMode::Text | No wrap-width drag box. |
| TXT-2 | Text | Printed size scaling | Partial | plan-core text_styles.rs plan_height_for_printed; manual 5.3 | Model supports it; default text height is plan inches. |
| TXT-3 | Text | Text handles and spec | Partial | handles.rs; dialogs/text.rs | No wrap-width handle. |
| TXT-4 | Text | Rich Text box editor | Partial | tools/text.rs RichRun markup | Markup runs not a WYSIWYG box; italic not drawn. |
| TXT-5 | Text | Leader Line | Works | TextMode::LeaderLine | Spline leader absent. |
| TXT-6 | Text | Text Line with Arrow | Works | TextMode::ArrowLine | - |
| TXT-7 | Text | Callout | Works | TextMode::Callout (circle, hexagon, square) | - |
| TXT-8 | Text | Marker | Works | TextMode::Marker | Not linked to views. |
| TXT-9 | Text | Note and Note Types | Works | TextMode::Note; dialogs/text/manage.rs | Note Schedule separate kind open (queue). |
| TXT-10 | Text | Text snaps and smart alignment | Partial | cx.snap_at | No left-edge text alignment. |
| TXT-11 | Text | Text layer, lock/hide | Works | selection layer_of | - |
| TXT-12 | Text | Find/Replace Text, Replace Fonts | Missing | menus.rs:303-304 inert | - |
| TXT-13 | Text | Auto-grow height, width handle | Partial | text.rs | No width handle. |
| TXT-14 | Text | Rotation snaps to 15 and walls | Partial | select.rs CadRotate snaps angle | Wall-angle snap not confirmed. |
| TXT-15 | Text | One undo step | Works | text.rs | - |
| TXT-16 | Text | Text Specification fields | Partial | dialogs/text.rs | Alignment, Border, Background disabled. |
| TXT-17 | Text | Named text styles | Works | plan-core text_styles.rs; Default Settings Text Styles | - |
| TXT-18 | Text | Edit toolbar buttons for text | Partial | actions.rs | Open/Copy/Delete only; no Rotate/Convert/Transform buttons. |
| TXT-19 | Text | Acceptance: printed 1/8 in text | Partial | text_styles.rs plan_height_for_printed | Placement path not using it by default. |
| CAD-1 | CAD | Current CAD layer | Partial | cx defaults CAD, Default | Current CAD Layer button planned. |
| CAD-2 | CAD | Place/Input/Marker points, delete temps | Works | CadMode::PlacePoint etc. | - |
| CAD-3 | CAD | Draw Line with Connect CAD Segments | Works | CadMode::Line; Shift+F8 toggle | - |
| CAD-4 | CAD | Connected segments behave as path | Partial | Connect flag; Convert to Polyline | Moving shared vertex moves both: not confirmed. |
| CAD-5 | CAD | Input Line | Works | CadMode::InputLine | - |
| CAD-6 | CAD | Line With Arrow, backoff points | Works | CadMode::LineArrow; Add Arrow Backoff Point | - |
| CAD-7 | CAD | Draw Arc with creation modes | Partial | CadMode::Arc option strip (three-point, center-start-end, tangent) | Start-end-radius mode absent; Edit > Arc Creation Modes inert. |
| CAD-8 | CAD | Input Arc | Works | CadMode::InputArc | - |
| CAD-9 | CAD | Arc editing handles | Works | handles.rs arc handles | - |
| CAD-10 | CAD | Arc Centers and Ends toggle | Works | menus.rs:874 ViewFlag::ArcCenters | - |
| CAD-11 | CAD | Circle, Circle About Center, Ellipse, Oval | Works | CadMode variants | Ellipse stored as 48-segment polyline. |
| CAD-12 | CAD | Rect polyline, Box, Cross, Blocking, Insulation, Polygon | Works | CadMode; edit_tests | Symbol look to verify in Chief. |
| CAD-13 | CAD | Fill and line style per item | Works | dialogs/cad.rs Line/Fill Style tabs | Concave solid fills draw convex. |
| CAD-14 | CAD | Snaps; Shift constrains | Partial | cx.snap_at | Shift constraint absent. |
| CAD-15 | CAD | Polyline draw/close | Works | CadMode::Polyline | Right-click end menu absent. |
| CAD-16 | CAD | Polyline vertex handles | Works | handles.rs Reshape | - |
| CAD-20 | CAD | Vertex drag; fillet/chamfer behaviors | Partial | Fillet and Chamfer tools | Edit Behaviors absent; tools via CAD menu. |
| CAD-21 | CAD | Add/Delete Break, Break Line | Partial | CadMode::BreakLine; add-vertex edge handles | No Delete Break command. |
| CAD-22 | CAD | Change Line/Arc | Missing | manual: planned | Polyline has no arc segments. |
| CAD-23 | CAD | Convert to polyline/spline/lines | Works | CadMode converts | Wall-to-polyline absent. |
| CAD-24 | CAD | Make Arc Tangent | Missing | manual: planned | - |
| CAD-25 | CAD | Concentric behavior | Partial | CadMode::Offset | Offset tool covers; behavior drag absent. |
| CAD-26 | CAD | Closed polyline area, fill | Works | dialogs/cad.rs Perimeter/Area | No arc segments. |
| CAD-27 | CAD | Self-intersect fill rule | Partial | render fill | Not verified. |
| CAD-28 | CAD | Convert polylines to walls/slabs | Partial | CAD to Walls; slab tool polygon | No roof plane/room/solid conversion. |
| CAD-29 | CAD | Spline fit/Bezier, tension | Works | CadMode::Spline | - |
| CAD-30 | CAD | Spline rendering and snaps | Works | tools/cad.rs catmull_rom | - |
| CAD-31 | CAD | Make CAD Block into library | Partial | CadMode::MakeBlock | Block is a named group in the plan, not in the Library Browser. |
| CAD-32 | CAD | Edit/Explode block | Partial | Edit CAD Block edits name/points | No detail window. |
| CAD-33 | CAD | Block instance handles, insertion points | Partial | Add Insertion Point tool | Instances are groups; no uniform resize. |
| CAD-34 | CAD | CAD Block Management | Works | dialogs/cad/blocks.rs | No export. |
| CAD-35 | CAD | Auto Detail, Detail From View, Detail Management | Partial | CadMode::DetailFromView | Makes a floor; no Auto Detail or management. |
| CAD-36 | CAD | Revision Cloud | Works | CadMode::RevisionCloud | Toolbar button dimmed; menu works. |
| CAD-37 | CAD | Block layers preserved | Partial | blocks keep object layers | Instance layer display not verified. |
| CAD-38 | CAD | Line/Arc/Circle specification | Works | dialogs/cad.rs | - |
| CAD-39 | CAD | Input Line dialog fields | Works | CadMode::InputLine typed fields | - |
| CAD-40 | CAD | Snap to CAD geometry (tangent, quadrant) | Partial | snap.rs endpoint/mid/intersection | Tangent, quadrant, center absent. |
| CAD-41 | CAD | Edit toolbar for line/arc/polyline | Partial | actions.rs | Edit tools live in CAD menu not toolbar. |
| CAD-42 | CAD | Edit toolbar for circle/ellipse | Partial | actions.rs | Open/Copy/Delete only. |
| CAD-43 | CAD | Edit commands one undo step | Works | edit.rs | - |
| CAD-44 | CAD | Acceptance: Input Line 12 ft at 30 deg | Works | tools/cad edit_tests / s07 | - |
| CAD-45 | CAD | Acceptance: polyline to walls | Works | plan_import::cad_to_walls | - |
| LAY-1 | Layers | Layer holds display, lock, color, weight; ByLayer | Works | plan-core layers.rs | - |
| LAY-2 | Layers | Layer sets and views | Works | plan-core layer_sets.rs; Project.plan_views | - |
| LAY-3 | Layers | Active Layer Display Options dock | Works | shell/docks.rs LayerDisplay | Copy Layer Set planned. |
| LAY-4 | Layers | Hidden layer still in model | Works | render filters only | - |
| LAY-5 | Layers | Locked layers | Works | actions.rs is_locked | - |
| LAY-6 | Layers | Current CAD layer button | Missing | manual 5.4 planned | - |
| LAY-7 | Layers | Line weights toggle | Works | restyle.rs | - |
| LAY-8 | Layers | Save layer set | Partial | layer_sets.rs | Save Active View As planned. |
| LAY-9 | Layers | Reference Display | Partial | render.rs draw_reference_floor | Floor below walls only. |
| LAY-10 | Layers | Reference options | Missing | none | No dialog or color pref. |
| LAY-11 | Layers | Default layer set names | Works | defaults default_floor_plan | - |
| LAY-12 | Layers | Layer changes undoable | Works | manual 5.5 | - |
| LAY-13 | Layers | Per-object Layer command | Partial | spec dialogs have Layer tab | No Edit-toolbar Layer button. |
| LAY-14 | Layers | Acceptance: layer off/lock | Works | s10/s12 scenarios | - |
| LAY-15 | Layers | Acceptance: reference display floor 1 under floor 2 | Works | render.rs:495 | - |

### 3D views and cameras (`docs/parity/3d-views-cameras.md`)

71 ids: 31 Works, 25 Partial, 15 Missing, 0 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| C-1 | 3D/Cameras | Row 1 3D controls | Partial | toolbar.rs 3D View/Full Camera/Orbit/Slider/Walkthrough/Lights flyouts | Material Painter, eyedroppers, Delete Surface, Adjust/Interactive Material buttons are stubs. |
| C-2 | 3D/Cameras | 3D menu structure | Partial | menus.rs three_d_menu | Move/Orbit/Tilt/View Direction/Isometric/Materials/Lighting/Camera View Options are inert. |
| C-3 | 3D/Cameras | Camera view as tab/window with saved view options | Partial | shell/view3d_panel.rs | Single 3D panel; per-view options limited to technique and elevation settings. |
| C-4 | 3D/Cameras | Full Camera click + drag | Works | tools/camera.rs | - |
| C-5 | 3D/Cameras | Eye height 66 in, FOV 60 | Works | manual 10.2 | - |
| C-6 | 3D/Cameras | Camera floor-relative, floors displayed | Partial | camera.rs floor field | Floors Displayed option absent. |
| C-7 | 3D/Cameras | Angle of view 5 to 170 | Works | dialogs/camera.rs | - |
| C-8 | 3D/Cameras | Height change keeps target | Partial | dialogs/camera.rs | Target height editing limited. |
| C-9 | 3D/Cameras | No roll | Works | plan-view3d camera.rs | - |
| C-10 | 3D/Cameras | Perspective Full Overview | Works | toolbar.rs V::FullOverview (Shift+K) | - |
| C-11 | 3D/Cameras | Perspective Floor Overview | Works | toolbar.rs:1720 | - |
| C-12 | 3D/Cameras | Perspective Framing Overview | Missing | no entry in toolbar.rs | Framing members only via manual framing in 3D. |
| C-13 | 3D/Cameras | Doll House View | Works | toolbar.rs DollHouse | - |
| C-14 | 3D/Cameras | Overviews generated, not placed | Works | manual 10.2: no camera object | - |
| C-15 | 3D/Cameras | Orthographic overviews and isometrics | Partial | Orthographic Full Overview + 4 elevations | No floor/framing ortho overview, no isometric SW/SE/NE/NW. |
| C-16 | 3D/Cameras | Ortho zoom and scale | Partial | view3d_panel.rs | Camera View Options scale absent. |
| C-17 | 3D/Cameras | Cross Section/Elevation camera | Works | tools/camera.rs; s09 | Drawn cut square to nearest axis. |
| C-18 | 3D/Cameras | Section poche vs elevation | Works | plan-elevation; manual 10.7 | - |
| C-19 | 3D/Cameras | Back-Clipped Cross Section | Works | tools/camera.rs; clip handle | - |
| C-20 | 3D/Cameras | Wall Elevation Camera | Works | manual 10.11 | - |
| C-21 | 3D/Cameras | Create Auto Elevations | Works | manual 10.11; tools/camera.rs | - |
| C-22 | 3D/Cameras | Hidden-line vector output | Works | plan-elevation; Vector View panel | Curves are facets. |
| C-23 | 3D/Cameras | Cross Section Slider | Works | toolbar.rs cross_section_slider | - |
| C-24 | 3D/Cameras | Camera symbol in plan, layer Cameras | Works | tools/camera.rs draw_camera_symbols | Integration queue item 1: confirm drawn in Select. |
| C-25 | 3D/Cameras | Full Camera handles | Partial | camera.rs CamHandle (move, aim, clip) | FOV cone-edge handles absent. |
| C-26 | 3D/Cameras | Drag moves camera, live 3D | Works | camera.rs | - |
| C-27 | 3D/Cameras | Aim handle with Shift 15 deg | Works | manual 10.3 | - |
| C-28 | 3D/Cameras | Section handles incl. clip | Works | manual 10.3 | - |
| C-29 | 3D/Cameras | Delete camera; copy/paste | Partial | Delete works | No copy/paste of cameras confirmed. |
| C-30 | 3D/Cameras | Camera Specification dialog | Works | dialogs/camera.rs | - |
| C-31 | 3D/Cameras | Camera View Options per view | Partial | dialogs/camera.rs Rendering tab, elevation options | No Lighting set, backdrop, sky, fog, ground, layer set. |
| C-32 | 3D/Cameras | View options vs defaults | Partial | 3D View Defaults dialog | Defaults: eye height, angle, technique only. |
| C-33 | 3D/Cameras | Locked camera | Missing | dialogs/camera.rs:464 disabled | - |
| C-34 | 3D/Cameras | Orbit vs Move Camera modes | Partial | view3d_panel.rs gestures | No explicit Move Camera tool choices. |
| C-35 | 3D/Cameras | Orbit with pitch clamp | Works | plan-view3d camera.rs | - |
| C-36 | 3D/Cameras | Pan | Works | view3d_panel.rs | - |
| C-37 | 3D/Cameras | Zoom, rubber-band, Fill Window | Partial | wheel dolly; fit | No rubber-band zoom or Fill Window Selected. |
| C-38 | 3D/Cameras | Move Camera with Keyboard | Partial | WASD/arrows/PgUp/PgDn in Full Camera | Menu entries inert. |
| C-39 | 3D/Cameras | 3D Center pick | Missing | none | Orbit centre fixed to bounds. |
| C-40 | 3D/Cameras | Tilt Camera | Partial | drag pitch; spec dialog | No Tilt tool. |
| C-41 | 3D/Cameras | View Direction snaps | Missing | menus.rs:500 inert | - |
| C-42 | 3D/Cameras | Undo Zoom in 3D | Missing | none | - |
| C-43 | 3D/Cameras | Click selects object in 3D | Missing | selection.rs pick_for_mesh_id exists, no caller | Panel has no pick hook (integration queue). |
| C-44 | 3D/Cameras | Floors displayed / ceiling visibility | Partial | Floor Overview and Doll House | Per-camera Floors Displayed absent. |
| C-45 | 3D/Cameras | Technique list | Partial | plan-materials nine techniques; manual 10.4 | Many are approximations of Chief looks. |
| C-46 | 3D/Cameras | Standard shaded view | Works | plan-view3d | No textures. |
| C-47 | 3D/Cameras | Vector View | Works | plan-elevation; Vector View panel | - |
| C-48 | 3D/Cameras | Technical Illustration | Works | manual 10.7 | - |
| C-49 | 3D/Cameras | Watercolor, Line Drawing, Duotone | Partial | flat shading approximations | Not bitmap filters; no parameter dialogs. |
| C-50 | 3D/Cameras | Glass House | Works | manual 10.4 | - |
| C-51 | 3D/Cameras | Physically Based ray trace | Works | plan-render; Ray Trace window | CPU progressive, PNG out. |
| C-52 | 3D/Cameras | Clay | Works | manual 10.4 | - |
| C-53 | 3D/Cameras | Technique never changes model | Works | view state only | - |
| C-54 | 3D/Cameras | Rebuild 3D | Works | menus.rs:547 | - |
| C-55 | 3D/Cameras | Delete Surface | Missing | toolbar.rs:1888 stub | - |
| C-56 | 3D/Cameras | Material Painter modes | Missing | toolbar.rs:1885 stub | - |
| C-57 | 3D/Cameras | Eyedroppers | Missing | stub | - |
| C-58 | 3D/Cameras | Adjust Material Definition | Missing | stub | - |
| C-59 | 3D/Cameras | Interactive Material Editor | Missing | stub | - |
| C-60 | 3D/Cameras | Painter overrides wall-type materials | Partial | walls take material from type layers | No per-surface override. |
| C-61 | 3D/Cameras | Material Builder, textures | Missing | menus.rs inert | plan-materials has data; GL view flat colors. |
| C-62 | 3D/Cameras | Default lighting and shadows option | Partial | key light + ambient | Shadows only in vector elevations and ray tracer. |
| C-63 | 3D/Cameras | Sun Angle | Partial | Sun Angle window (date, time, latitude, azimuth/altitude) | No longitude, DST, north direction; no Move Sun/Moon. |
| C-64 | 3D/Cameras | Add Lights, auto-place, Adjust Lights | Partial | Project.lights; Adjust Lights dialog | Point lights only; no auto-place or color temperature. |
| C-65 | 3D/Cameras | Light sets | Missing | manual 10.6: not built | - |
| C-66 | 3D/Cameras | Lights as selectable objects | Partial | lights drawn in plan | Selectable in Add Lights tool; layer name differs. |
| C-67 | 3D/Cameras | Shadows toggle in Standard | Missing | none | Cast shadows disabled in dialog. |
| C-68 | 3D/Cameras | 3D View Defaults dialog | Partial | dialogs/camera.rs | Eye height, angle, technique only. |
| C-69 | 3D/Cameras | Edge display and line weights | Partial | edge overlay; vector line weights | No weight setting for Standard edges. |
| C-70 | 3D/Cameras | Sky, ground, backdrop | Missing | single background | Terrain renders as ground. |
| C-71 | 3D/Cameras | Walkthrough path and preview | Works | tools/camera.rs; Play/Record | PNG sequence, 640x480, no video. |

### Cabinets, stairs, framing, terrain, library, electrical (`docs/parity/cabinets-stairs-framing-terrain-library.md`)

68 ids: 38 Works, 27 Partial, 2 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| CB-1 | Cabinets | Cabinet tool inventory | Works | toolbar.rs Cabinet flyout (16 kinds); tools/cabinet.rs | - |
| CB-2 | Cabinets | Ghost placement, tool stays active | Works | tools/cabinet.rs; scenarios/s04_cabinets.rs | - |
| CB-3 | Cabinets | Auto-rotate flush to nearest wall | Works | tools/cabinet.rs (12 in rule) | - |
| CB-4 | Cabinets | Neighbor bumping/pushing | Partial | cabinet.rs bump to neighbor | No pushing; no Snap Settings control. |
| CB-5 | Cabinets | Fill between walls/cabinets | Partial | Base/Wall/Full Height Filler fill gap | Standard cabinets do not auto-size to the gap. |
| CB-6 | Cabinets | Default sizes | Works | plan-cabinets defaults; manual 6.1 | - |
| CB-7 | Cabinets | Cabinet Specification tabs | Partial | dialogs/cabinet.rs | Fill Style, Components, Object Info, Schedule disabled; Sides/Back not editable. |
| CB-8 | Cabinets | Resize, rotate handles, temp dims | Partial | handles via placed.rs | No depth or corner handles; no cabinet temp dims. |
| CB-9 | Cabinets | Drag keeps rotation until bump; Ctrl disables | Works | manual 6.2 | - |
| CB-10 | Cabinets | Face editing on all six faces | Partial | dialogs/cabinet.rs Front/Sides/Back | Front only. |
| CB-11 | Cabinets | Split, equalize, locks | Works | plan-cabinets face solver | - |
| CB-12 | Cabinets | Door/drawer styles from Library | Partial | dialogs/cabinet.rs Door/Drawer | Built-in styles only; library styles planned. |
| CB-13 | Cabinets | Auto labels B24, W3030 | Works | plan-cabinets auto_label | - |
| CB-14 | Cabinets | Countertops auto-join | Partial | Generate Countertop (G) | Manual command, not automatic; corner treatment None only. |
| CB-15 | Cabinets | Custom Countertop, Backsplash, Counter Hole | Works | tools/cabinet.rs | - |
| CB-16 | Cabinets | Appliances inserted into cabinets, cut-outs | Partial | Appliance Opening; sink/cooktop cutouts | No drag-over insertion of library appliances. |
| CB-17 | Cabinets | Soffit tool | Partial | Soffit kind | Cabinet-like box, not polyline soffit. |
| CB-18 | Cabinets | Shelf and Partition | Works | CabinetKind | - |
| CB-19 | Cabinets | Fillers | Works | tools/cabinet.rs | - |
| CB-20 | Cabinets | Cabinet defaults per kind | Partial | defaults.rs cabinet door/drawer style | No Default Settings > Cabinets page per kind confirmed. |
| CB-21 | Cabinets | Cabinet Schedule | Works | schedule_kinds.rs; C-01 labels | - |
| CB-22 | Stairs | Stair tool inventory | Works | tools/stairs.rs; toolbar Stairs flyout | - |
| CB-23 | Stairs | Draw Stairs by drag, riser count | Works | tools/stairs.rs; plan-stairs solver | - |
| CB-24 | Stairs | Riser/tread/rise interrelation | Works | stairs_view.rs set_total_rise, locks | - |
| CB-25 | Stairs | Straight, L, U | Works | StairShape | - |
| CB-26 | Stairs | Curved stairs, flare, winders | Partial | StairShape::Curved; winders | Flared apron of Flare/Curve open. |
| CB-27 | Stairs | Landings | Works | stairs_view.rs connect | - |
| CB-28 | Stairs | Stair edit handles | Partial | stairs_view.rs StairHandleKind | No per-landing corner handles. |
| CB-29 | Stairs | Auto Stairwell hole and guard rails | Partial | QA-04 fixed; PlatformHole | Guard railings around opening open. |
| CB-30 | Stairs | Stairwell room | Works | room "Stairwell", has_floor false | Not RoomFunction::OpenBelow. |
| CB-31 | Stairs | Railings and balusters | Works | StairParams left_side/right_side | No railing across landing in plan. |
| CB-32 | Stairs | Stair Specification tabs | Partial | dialogs/stairs.rs | Components and Schedule tabs open. |
| CB-33 | Stairs | Plan symbol with break, UP/DN | Partial | stairs_view.rs | Dashed hidden treads on other floors open. |
| CB-34 | Stairs | Ramps slope check | Works | plan-stairs | - |
| CB-35 | Framing | Framing tool inventory | Works | tools/framing.rs 19 tools | - |
| CB-36 | Framing | Build Framing dialog with per-group auto rebuild | Partial | Build Framing / Build All Framing commands | No dialog; no auto-rebuild groups. |
| CB-37 | Framing | Auto rebuild keeps manual members | Partial | rebuild replaces built; manual kept | No auto-rebuild on edits; no Retain option. |
| CB-38 | Framing | Wall framing and reference marker | Works | plan-framing wall.rs; manual 11.11 | Rollout options absent. |
| CB-39 | Framing | Floor framing, joist direction, rim | Works | plan-framing floor.rs | - |
| CB-40 | Framing | Wall detail views | Missing | none | - |
| CB-41 | Framing | Framing schedule and Materials List | Partial | Framing Takeoff + CSV; plan-docs materials.rs | No waste/cost, no roofing lines, no PDF. |
| CB-42 | Framing | Structural calcs | Missing | none | Out of scope. |
| CB-43 | Terrain | Perimeter, data, Build Terrain | Works | tools/terrain.rs; plan-terrain | - |
| CB-44 | Terrain | Elevation data tools and Break | Works | TerrainVariant | - |
| CB-45 | Terrain | Modifiers | Works | Hill, Valley, Raised, Lowered, Flat | - |
| CB-46 | Terrain | Features and Terrain Hole | Works | plan-terrain landscape | Features are slabs, not cut and fill. |
| CB-47 | Terrain | Terrain hole around building, reference point | Partial | site_view::auto_building_hole | No elevation reference point. |
| CB-48 | Terrain | Roads, driveways, sidewalks | Works | tools/terrain.rs scape.rs | - |
| CB-49 | Terrain | Garden beds, grass, water | Works | landscape.rs | - |
| CB-50 | Terrain | Plants and sprinklers | Partial | plant runs; sprinkler runs | Plants not library symbols; not in Plant Schedule. |
| CB-51 | Terrain | Terrain Specification | Partial | dialogs/terrain.rs | Materials tab disabled. |
| CB-52 | Terrain | Terrain on Terrain layer, in 3D | Works | site_view terrain_feature_meshes; manual 9.7 | - |
| CB-53 | Library | Library Browser panel | Works | shell/library_browser.rs | - |
| CB-54 | Library | Search and filters | Partial | plan-library search | No Type/Style/Manufacturer filters or in-plan filter. |
| CB-55 | Library | Click to place symbol | Works | tools/library.rs | - |
| CB-56 | Library | Auto-rotate wall-mounted | Works | library.rs 6 in rule | - |
| CB-57 | Library | Replace From Library | Works | EditActionKind via placed.rs | Does not keep cabinet inserts. |
| CB-58 | Library | User Library, Add to Library | Partial | ~/.plan-studio/user-library.json (images) | Add to User Library menu item still coming. |
| CB-59 | Library | Import .calib/.calibz | Differs-by-design | DECISIONS.md #3: Chief catalogs read in place via plan-calib | Library > Import menu inert by design. |
| CB-60 | Library | Library items with 3D models | Partial | plan-calib decoded meshes in 3D | Built-in symbols are boxes; Symbol 3D tab disabled. |
| CB-61 | Library | List/thumbnail, favorites | Partial | thumbnails in Chief rows | No view toggle or Favorites. |
| CB-62 | Electrical | Electrical tools | Works | tools/electrical.rs | - |
| CB-63 | Electrical | Outlet placement on wall, heights | Works | site_view.rs; manual 9.2 | - |
| CB-64 | Electrical | Auto Place Outlets rules | Partial | plan-electrical auto place | 6 ft rule and GFCI done; kitchen 4 ft counter rule uses 44 in GFCI. |
| CB-65 | Electrical | Switch placement near door | Partial | engine function only | Not offered as a button. |
| CB-66 | Electrical | Lights | Works | Light, Recessed, Pendant etc. | - |
| CB-67 | Electrical | Electrical Connection arcs | Works | tools/electrical.rs | Midpoint bend handle not confirmed. |
| CB-68 | Electrical | Electrical Schedule | Works | schedule_kinds.rs | Circuits/legend not in UI. |

### Documentation and layout (`docs/parity/documentation-layout.md`)

47 ids: 8 Works, 30 Partial, 8 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| L-1 | Layout/Docs | Send to Layout from active view | Works | shell/layout_window.rs; dialogs/layout.rs; hotkey S,L | Sources: plan views and elevation/section cameras; perspective, framing, schedule views not sendable. |
| L-2 | Layout/Docs | Send to Layout dialog fields | Partial | dialogs/layout.rs | One layout per plan; no layout-file choice or link toggle. |
| L-3 | Layout/Docs | Linked layout boxes refresh | Works | layout_window.rs live redraw; Update Layout Views | - |
| L-4 | Layout/Docs | Layout Box Specification | Partial | dialogs/layout.rs General/Source/Line Style | Rotation disabled; per-box layer/dimension display options absent. |
| L-5 | Layout/Docs | Vector vs raster boxes | Partial | plan-layout vector boxes; image boxes | Shaded 3D views cannot be sent as raster boxes. |
| L-6 | Layout/Docs | Move, resize, copy, align boxes | Partial | layout_window.rs 8 handles, nudge | No align or copy between pages; no Open Source View. |
| L-7 | Layout/Docs | Pages with names and numbers | Partial | Layout Page Table, A-n numbering | No Page Specification dialog (border, page text). |
| L-8 | Layout/Docs | Sheet sizes list | Partial | Page Setup lists Arch, ANSI, ISO | Landscape only; no Customize Sheet Sizes. |
| L-9 | Layout/Docs | Title block macros | Partial | plan-layout macros; Project Information | %client.phone%, %company%, %custom.x% not expanded by layout. |
| L-10 | Layout/Docs | Layout templates | Partial | Page Template page; Daniel 18x24 block modeled | No Save As Template for layouts; template sheet size only. |
| L-11 | Layout/Docs | Sheet index | Works | Page Setup Sheet index on first page | - |
| L-12 | Layout/Docs | Pen weights in paper units, scaling multiplier | Works | plan-layout render; Line weight scaling 0.1-5x | - |
| L-13 | Layout/Docs | Fill/pattern scaling to paper | Partial | plan-materials patterns | Elevation hatch fixed at 1/4 in scale. |
| L-14 | Layout/Docs | Plot/site plan layout | Partial | plan view box with terrain layers | North pointer and scale bar objects missing; no site plan layout box. |
| L-15 | Layout/Docs | Dimensions/text scale with box | Partial | plan-layout renders dimensions | Text height not in paper units by default. |
| L-16 | Layout/Docs | Layout CAD and text on pages | Missing | engine supports; manual 11.9 | No editor way to draw CAD/text on pages. |
| L-17 | Layout/Docs | Boxes keep link by id | Works | layout stored in plan | - |
| L-18 | Layout/Docs | Print dialog, scale, tiling | Partial | Print Layout writes PDF | No printer dialog, margins, tiling; Row-1 Print button dimmed. |
| L-19 | Layout/Docs | Print Preview, Drawing Sheet, B/W modes | Partial | View Print Preview, Drawing Sheet (Alt+F2/F3) | No B&W or grayscale modes. |
| L-20 | Layout/Docs | Multi-page PDF, bookmarks, fonts, raster DPI | Partial | Export Layout PDF; PdfDoc | Helvetica only, no bookmarks, no embedded fonts, no raster at DPI. |
| L-21 | Layout/Docs | Printed scale accuracy | Works | plan-docs pdf; scale caption | - |
| L-22 | Layout/Docs | Drawing scale lists | Partial | architectural and metric scale lists | Custom ratio not confirmed. |
| L-23 | Layout/Docs | Schedule set | Partial | Schedule flyout (10 kinds) | Wall is window-only; Note and Room Finish not kinds; no Manage Custom Schedules. |
| L-24 | Layout/Docs | Live schedules, floor filter, send to layout | Partial | schedule_view.rs live table; Floors option | Schedule layout box kind absent. |
| L-25 | Layout/Docs | Column editor | Works | dialogs/schedule_spec.rs | - |
| L-26 | Layout/Docs | Callout markers linking plan to schedule | Partial | schedule_view.rs D01/W03/C-01/F-01 | Text labels not circle/hexagon markers; no renumber command. |
| L-27 | Layout/Docs | Edit from schedule | Missing | integration-queue item 4 open | No click-row-selects-object. |
| L-28 | Layout/Docs | Grouping and totals | Missing | integration-queue open | - |
| L-29 | Layout/Docs | Schedule tab data on objects | Missing | spec dialogs Schedule tab disabled | No manufacturer/model/cost fields. |
| L-30 | Layout/Docs | Room Finish Schedule | Partial | Room schedule Floor/Ceiling Finish columns (hidden) | No Wall Finish, Base, Number columns; not a separate kind. |
| L-31 | Layout/Docs | Custom schedules | Partial | Create Schedule with filter text | No object-type/field builder or management. |
| L-32 | Layout/Docs | Schedules export CSV/print | Partial | Export CSV | No Excel/PDF; tables absent from DXF and construction set PDF. |
| L-33 | Layout/Docs | Materials List by category | Partial | Tools > Materials List (plan-docs materials.rs) | Active floor only; framing/sheathing/drywall/doors/windows; no roofing/cabinets. |
| L-34 | Layout/Docs | Materials columns | Partial | Category, Item, Quantity, Unit | No price, waste, supplier. |
| L-35 | Layout/Docs | Quantities from component layers | Partial | formula take-off | Not driven by Components tabs. |
| L-36 | Layout/Docs | Waste and stock lengths | Missing | manual 11.5 | - |
| L-37 | Layout/Docs | Materials List to layout, PDF, XLS | Partial | CSV export | No layout box or PDF/XLS. |
| L-38 | Layout/Docs | Plan Footprint | Partial | Tools > Checks > Plan Footprint (CAD polyline + area) | Traces room boundary, not outer wall faces. |
| L-39 | Layout/Docs | Auto Detail | Missing | menus.rs inert | - |
| L-40 | Layout/Docs | CAD Detail From View | Partial | CadMode::DetailFromView (new floor) | Copies plan only, not section/elevation; floor not detail library. |
| L-41 | Layout/Docs | CAD Detail Management | Missing | menus.rs:637 inert | - |
| L-42 | Layout/Docs | CAD to Walls | Works | plan_import::cad_to_walls; dialogs/exchange.rs | - |
| L-43 | Layout/Docs | DXF import with units, layers | Partial | dialogs/exchange.rs; plan-import | Units and prefix only; no layer map, explode blocks, DWG. |
| L-44 | Layout/Docs | DXF/DWG export options | Partial | plan-core export/dxf.rs R12 ASCII; elevation DXF | No version, units, selection options; no DWG. |
| L-45 | Layout/Docs | Walls as polylines, dims, symbols | Partial | dxf.rs | Dimension/symbol options absent. |
| L-46 | Layout/Docs | PDF/image underlay with calibration | Missing | manual 12: planned | - |
| L-47 | Layout/Docs | Other exports (SketchUp, IFC, image, animation) | Differs-by-design | glTF and ray-trace PNG, walkthrough PNG sequence exist | Out of scope per spec. |

