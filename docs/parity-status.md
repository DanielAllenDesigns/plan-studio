# Parity status: Plan Studio vs Chief Architect X18

Snapshot of 2026-10-08 against HEAD 6f6a7b9. Written by reading every spec in `docs/parity/*.md`, `docs/architecture-tools.md`, `docs/integration-queue.md` and `docs/qa-findings.md`, then locating the code for each id (ids cited in `crates/` first, otherwise the feature by name in `tools/`, `dialogs/`, `editor/` and the engine crates). No Rust file was edited. `cargo test -p plan-app scenarios` passes (95 tests). The full workspace run has one failure, `editor::render::perf_bench::a_door_added_changes_the_schedule_that_is_drawn`, in `crates/plan-app/src/editor/perf_bench.rs`, a file another builder was editing during this pass (its line numbers moved between runs); it is not a parity gap.

**Status words.** Works: behavior is present and exercised (a test, a scenario or the tool itself). Partial: present with a stated limit. Missing: no code path or a dimmed/stub control. Differs-by-design: deliberately different (a DECISIONS.md call, the manual's "Known differences" or a spec note) so it is not counted as a gap to close. Statuses come from code reading plus the manual chapters; "verify in Chief" items are not settled by this table (see the list below).

There are 765 parity ids: W 105, S 112, DW 110, R 71, RF 60, DIM 45, TXT 19, CAD 42 (CAD-17..19 do not exist in the spec), LAY 15, C 71, CB 68, L 47. Stairs, framing, terrain, library and electrical are CB-22..CB-68; no separate parity spec exists for exterior details, slabs, images or schedules beyond L-23..L-32 (their status lives in manual chapters 16 and 17).

## Totals by area

| Area | Ids | Works | Partial | Missing | Differs | Works % |
|---|---|---|---|---|---|---|
| Walls | 105 | 74 | 23 | 6 | 2 | 70% |
| Select | 112 | 76 | 28 | 7 | 1 | 68% |
| Doors/Windows | 110 | 76 | 20 | 13 | 1 | 69% |
| Rooms/Floors | 71 | 48 | 21 | 2 | 0 | 68% |
| Roofs | 60 | 36 | 19 | 5 | 0 | 60% |
| Dimensions | 45 | 35 | 9 | 0 | 1 | 78% |
| Text | 19 | 10 | 8 | 1 | 0 | 53% |
| CAD | 42 | 24 | 16 | 2 | 0 | 57% |
| Layers | 15 | 13 | 2 | 0 | 0 | 87% |
| 3D/Cameras | 71 | 32 | 27 | 12 | 0 | 45% |
| Cabinets | 21 | 15 | 6 | 0 | 0 | 71% |
| Stairs | 13 | 8 | 5 | 0 | 0 | 62% |
| Framing | 8 | 4 | 3 | 1 | 0 | 50% |
| Terrain | 10 | 7 | 3 | 0 | 0 | 70% |
| Library | 9 | 7 | 1 | 0 | 1 | 78% |
| Electrical | 7 | 7 | 0 | 0 | 0 | 100% |
| Layout/Docs | 47 | 17 | 24 | 5 | 1 | 36% |
| **Overall** | 765 | 489 | 215 | 54 | 7 | 64% |

Weighted view: Works plus Partial covers 704 of 765 ids (92%); 54 ids are Missing.

Reading the table: the engines are strong (walls, joins, roofs, stairs, terrain, 3D, layout engine). The gaps cluster in everyday editing commands (select, clipboard, transform), door and window variants and plan labels, associative dimensions, printing output, schedules interaction and 3D picking/materials.

## Next 25 gaps for daily residential work

Ranked by value to a residential designer (walls, openings, rooms, dimensions, cabinets, roofs, layout and printing first). Size: S under a day, M a few days, L a week or more of one builder.

| # | Gap (parity ids) | Why it matters daily | Files a builder would touch | Size |
|---|---|---|---|---|
| 1 | ~~Clipboard and selection basics: Cut, Copy, cursor-attached Paste, Cmd+A Select All, Cmd+D duplicate, bind the `C, P, P` chord (S-33, S-81..S-84, S-93)~~ **Done 2026-10-08** (S-33, S-81..S-84, S-93) | Every plan session; today only the Edit-toolbar Copy and Paste in Place buttons exist and the Edit menu rows are inert | `editor/actions.rs`, `editor/dispatch.rs`, `shell/hotkeys.rs`, `toolbar.rs` (BINDINGS), `menus.rs` (rows near line 273-295), `main.rs` (send_key), `editor/selection.rs` | M |
| 2 | ~~Transform/Replicate Object, Reflect About Object, Point to Point Move, Center Object, multi-object Rotate (S-47, S-48, S-52, S-53, S-101..S-106, DW-23)~~ **Done 2026-10-08** (S-47, S-48, S-52..S-54, S-101..S-106, DW-23) | Mirror a kitchen, array windows, rotate a wing; no way to do any of it today | new `editor/transform.rs`, pure transform fns in `plan-core` (over `ObjectRef`), new `dialogs/transform.rs`, `editor/actions.rs`, `editor/dispatch.rs`, `tools/select.rs` | L |
| 3 | **Done 2026-10-08 (the Layout pages draw every symbol through `plan_core::opening_symbol::plan_symbol` since 2026-10-08; Ctrl+Alt+Cmd chords come from the hotkey file).** Door and window variants with plan symbols: Doorway, Sliding, Pocket, Bifold, Garage, Barn, Fixed, Shower; Bay, Bow, Box windows, Pass-Through, Wall Niche; double-door leaves (DW-38..DW-49, DW-107) | 13 flyout entries are dimmed stubs; the model and 3D already know all 15 styles, only the tools and 2D symbols are missing | `toolbar.rs` (lines 704-725), `tools/opening.rs`, `editor/render.rs` (draw_opening), `plan-core/src/openings.rs`, `dialogs/opening.rs`, `dialogs/default_lists.rs`, `shell/hotkeys.rs` | L (S for Doorway and Sliding first) |
| 4 | **Done 2026-10-08 (the Doors, Labels / Windows, Labels layers and the draggable label handle are done too).** Opening labels in plan (size shorthand 3068, schedule mark in the label) and automatic mark assignment on placement (DW-59..DW-63, DW-77, DW-94) | Door and window tags are on every permit set; `Opening::auto_label` exists but nothing draws it | `editor/render.rs`, `editor/schedule_view.rs`, `plan-core/src/openings.rs`, `dialogs/opening.rs` (persist Label tab), `tools/opening.rs` | M |
| 5 | **Done 2026-10-08 (library width snapping and door-plus-sidelite mulling are done; a transom over a door is open).** Opening resize handles, width temporary dimension, Center Object, Mull/Unmull windows (DW-12, DW-24, DW-26..DW-28, DW-51, DW-52, DW-98) | Resizing a window means opening a dialog; adjacent windows cannot be mulled | `editor/handles.rs`, `tools/select.rs` (new Op), `editor/tempdim.rs`, `plan-core/src/openings.rs` (mull group), `editor/actions.rs` | M |
| 6 | Typed length and angle while drawing walls, angle readout, Shift orthogonal, Alt suspends all snaps (W-15, W-16, W-17, W-18, S-74) | Drawing to exact dimensions is the core wall workflow; today you draw then retype in a temp dimension | `tools/wall.rs` (key handling), `editor/mod.rs` (capture_typing, queue item), `editor/tempdim.rs`, `editor/snap.rs` | M Built 2026-10-08: typed length/angle, angle readout, Shift and Alt (editor/typed_input.rs, tools/wall.rs). |
| 7 | Wall edit commands: Break Wall, Reverse Layers, Change Line/Arc with a bulge handle, Make Arc Tangent, Convert to Polyline, numeric Curved Wall section (W-23, W-43, W-44, W-66..W-68, W-90, S-41, S-42, S-55) | Layer reversal and breaking a wall are routine edits; curved walls cannot be edited by number | `editor/actions.rs`, `editor/dispatch.rs`, `plan-core/src/walls.rs` (flip layers, set curve), `editor/handles.rs`, `tools/select.rs`, `dialogs/wall.rs` (Curved section) | M Built 2026-10-08: all but the Radius-to/Lock options (editor/wall_edit.rs, dialogs/wall.rs). |
| 8 | **Done 2026-10-08 (the Elevation locate group has nothing to drive yet; temporary dimensions have their own group).** Associative dimensions plus Locate settings (DIM-3, DIM-4, DIM-26, DIM-29, DIM-40) | Dimensions that do not follow a moved wall are the top trust problem on a drawing set | `plan-core/src/dimension.rs` (anchors), `tools/dimension.rs`, `editor/ops.rs`, `editor/tempdim.rs`, `dialogs/default_lists.rs` | L |
| 9 | **Done 2026-10-08 (Auto NKBA Dimensions built; curved exterior walls string by their chord).** Auto Exterior Dimensions completeness: openings string and wall-to-wall string, non-orthogonal sides, edited-auto becomes manual (DIM-24, DIM-25, DIM-33) | First thing run on every floor plan; today only overall plus breakpoint strings | `tools/dimension.rs` (AutoExterior), `plan-core/src/dimension.rs`, `plan-core/src/defaults.rs` | M |
| 10 | **Done 2026-10-08 (placement, picking, DXF and sheet PDF follow the printed size).** Printed-size text and dimension text tied to plan scale; upright vertical dimension text, outside-text flip (DIM-7, DIM-8, DIM-9, TXT-2, TXT-19, L-15) | Text that stays 1/8 in at any scale is how every sheet is annotated | `editor/render.rs`, `tools/text.rs`, `plan-core/src/text_styles.rs`, `plan-layout` (render_box_lines) | M |
| 11 | Roof directives UI: Wall Specification Roof tab, roof style presets (Gable, Hip, Shed, Gambrel, Mansard, Dutch), Dutch gable, knee wall, upper pitch break (RF-3, RF-5, RF-18, RF-21, RF-23, RF-25) | Gambrel and Dutch gable roofs are common on residential jobs; the tab is disabled and gable/hip is a click-toggle only | `dialogs/wall.rs`, `plan-core/src/walls.rs` (RoofDirective), `plan-roof/src/skeleton.rs`, `plan-roof/src/spec.rs`, `editor/roof_view.rs`, `dialogs/roof.rs` | L |
| 12 | Roofs cut walls: gable triangles and walls clipped to roof planes in 3D, eave detail (soffit, fascia, eave cut), butting roofs (RF-13..RF-16, RF-20). **Done 2026-10-08** (eave cut plumb/level/square, rafter tails, gutters, Roof Defaults page, Roof Cuts Wall at Bottom, lower wall type split, half/pony/foundation/curved walls clipped, baseline at top plate; the break of a wall's bottom follows a lower roof only straight and by its centerline) | 3D and elevations show boxy wall tops under the roof, which clients and building departments notice | `plan-3d/src/wall.rs`, `plan-3d/src/roof.rs`, `plan-roof` (rake, fascia), `editor/roof_view.rs` | L |
| 13 | Materials List: roofing, cabinets, waste factors, stock-length rounding, prices, categories, layout/PDF output (RF-60, CB-41, L-33..L-37) | Estimating quantities is a daily deliverable; today a simple wall-only take-off (done in round 10: categories, roofing, cabinets, waste, stock lengths, Master List, window, CSV, PDF page, layout box) | `plan-docs/src/materials.rs`, `dialogs/build_tools.rs`, `plan-roof`, `plan-cabinets`, `plan-layout` | M |
| 14 | Layout boxes beyond plan views: text, schedule, perspective/3D raster and layout CAD on pages; box rotation (L-4, L-5, L-16, L-24) | Sheets need notes, schedules and renderings on the page; the editor can only create plan and elevation boxes (**done 2026-10-08**: text boxes that wrap, clip and shrink to fit, perspective boxes with their own DPI and samples, picture boxes (PNG and JPEG), Materials List and sheet index boxes, layout CAD tools with circles, arcs, leaders and revision clouds, move and resize handles on every page drawing, a layout layer set, rotation knob) | `shell/layout_window.rs`, `dialogs/layout.rs`, `plan-layout` (model, render), `editor/schedule_view.rs` | L |
| 15 | Printing: printer dialog with scale and tiling, B&W and grayscale, PDF embedded fonts, bookmarks, custom scale ratio (L-18..L-20, L-22) | PDF is the only output and uses Helvetica with no bookmarks (done except embedded font programs: File > Print dialog with a printer list from `lpstat -p`, tiling, color modes, bookmarks, Print Model at a chosen DPI, Print Image of the 3D view) | `plan-docs/src/pdf/`, `shell/layout_window.rs`, `dialogs/layout.rs`, `toolbar.rs` (Print button) | M |
| 16 | Schedule interaction: click row selects the object, grouping and totals, Room Finish and Note kinds, Wall schedule placeable, Schedule tab data on objects (L-23, L-27..L-30) | Schedules are read-only pictures today | `editor/schedule_view.rs`, `plan-docs/src/schedule_kinds.rs`, `plan-core/src/schedules.rs`, `dialogs/schedule_spec.rs` | M |
| 17 | ~~Right-click context menu on objects and empty space (S-8, DW-109)~~ **Done 2026-10-08** (S-8, DW-109) | Right-click currently only ends a drawing chain | `main.rs` (canvas secondary click), `editor/actions.rs`, `tools/select.rs` | M |
| 18 | Select objects in the 3D view (C-43, C-39 orbit center pick) | The plan-side half is done (`pick_for_mesh_id`); the panel has no pick hook | `shell/view3d_panel.rs`, `editor/selection.rs`, `plan-view3d/src/` | M |
| 19 | **Done 2026-10-08.** Garage and Stem Wall rooms get concrete stem walls in 3D; the Flat Roof directive and the Flat Roof function are built. Room Function behavior and 3D honoring Floor/Ceiling Over This Room: garage drop, deck/porch no ceiling, Open Below cut-outs, function-driven defaults (R-30, R-40, R-41, CB-29, CB-30) | Garage floor drop and open-to-below foyers are standard residential rooms | `plan-3d/src/slab.rs`, `plan-3d/src/lib.rs`, `editor/rooms_edit.rs`, `dialogs/room.rs`, `plan-core/src/model.rs` | M |
| 20 | **Done 2026-10-08.** Reference walls snap (ends and crossings) and the Ref column of Layer Display Options chooses their layers; Shift+Cmd+Y opens Floor Defaults. Floor Defaults dialog, fuller Build New Floor options (interior walls, rooms, below), Reference Display dialog and floor above (R-56, R-58, R-59, R-65, LAY-10) | Multi-story set-up and tracing the floor below | `toolbar.rs` (line 1869 stub), new `dialogs/floor_defaults.rs`, `plan-core/src/floors.rs`, `editor/render.rs` (draw_reference_floor) | M |
| 21 | **Done 2026-10-08.** Nested rooms (ring polygon and platform hole), draggable room labels, Floor/Ceiling Structure Define (R-11, R-28, R-29, R-44) | Islands such as chimney chases and wet-room pods get the wrong area and floor | `plan-core/src/rooms.rs`, `plan-3d/src/slab.rs`, `editor/render.rs`, `editor/rooms_edit.rs`, `dialogs/room.rs` | M |
| 22 | **Done 2026-10-08** (Sides and Back faces were already there): depth and corner resize handles, automatic countertop join, fit to gap (CB-5, CB-8, CB-10, CB-14) | Kitchen layout is the highest-value interior task; resizing needs the dialog | `dialogs/cabinet.rs`, `editor/placed.rs`, `editor/handles.rs`, `tools/cabinet.rs`, `plan-cabinets` | M |
| 23 | **Done 2026-10-08** except vector PDF and DWG: PNG, baseline JPEG and scanned-PDF underlays with two-point calibration; DXF import layer map, units, scale, rotation, insertion point, Convert to walls (L-43, L-46) | Tracing a survey or an existing-home PDF starts most remodel jobs | new underlay object in `plan-core`, `dialogs/exchange.rs`, `editor/render.rs`, `plan-import` | M |
| 24 | Snap Settings dialog (per-snap on/off, Center, Quadrant, Tangent) and Edit Behaviors (S-65, S-68, S-69, S-74, CAD-40) | The engine has per-type flags but the dialog is inert | `editor/snap.rs`, `menus.rs` (lines 296-297), new `dialogs/snap_settings.rs`, `editor/mod.rs` | S Built 2026-10-08: dialogs/snap_settings.rs and dialogs/edit_behaviors.rs. |
| 25 | ~~Edit-toolbar completeness: Group/Ungroup buttons and group-aware pick, Select Same Type, Align/Distribute, Layer button, Delete Objects dialog, Action History panel, Find/Replace Text (S-32, S-35, S-36, S-54, S-79, S-88, TXT-12, LAY-13)~~ **Done 2026-10-08** (S-32 (partly), S-35, S-36, S-54, S-79, S-88) | Many small commands; the core group model already exists | `editor/actions.rs`, `tools/select.rs`, `plan-core/src/groups.rs`, `editor/selection.rs`, `menus.rs`, `shell/docks.rs` | M |

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
- Layers: LAY-12 layer changes in the undo stack.

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
- Other: CB-42 structural calculators scope; CB-51 Terrain Specification fields; CB-64 Auto Place Outlets clearance (2 ft or 1 ft).

### L. Layout, Print, Schedules (7) and the integration queue (1)
- Send to Layout dialog: L-2 field names; L-3 box refresh mode names. Page Setup: L-8 sheet list and Customize Sheet Sizes; L-22 scale lists. Layout Box: L-12 line-weight multiplier name. Site plan: L-14 scale bar object. CAD menu: L-39 Auto Detail.
- Integration queue (docs/integration-queue.md, CAD section item 4): Cross Box, Blocking Box and Insulation symbols are drawn as an X box, a one-diagonal box and a boxed wave; compare with Chief.

## Open items from the queue and QA files not tied to one id

- 3D: electrical devices have a builder but are not in the 3D view; Build Framing's wall, floor and roof members draw in plan only; billboards keep their stored angle; landscape uses stand-in materials (no green); curved-wall detail that stays open is listed under Walls below.
- Walls: `Wall.bottom_offset` is honored in 3D only, not in the 2D plan, room detection, elevations, schedules or wall framing (queue, manual 2.9); curved walls are mitered in plan and in 3D to the walls joined to them (straight, curved and tee), but see the open items that follow.
- Curved walls, still open: a straight wall is built square-ended in 3D (`plan_3d::wall_kinds::EndCuts::from_outline` and `build_class_cut` are ready, `plan-3d/src/lib.rs` add_wall does not pass them yet), so a straight wall meeting an arc shows a small notch or overlap at the corner; `interior_sign` in `lib.rs` probes from the chord's middle, not the arc's, so the exterior face of a strongly bowed exterior wall can flip; `Project::add_opening` clamps the center offset to the chord, not the arc length; door and window units are flat, so on a tight radius their ends sit up to about 1 1/2" off the arc on a 10' radius (inside the wall thickness); railings on an arc are built a straight piece per facet; `wall_outlines()` (the cache plan-layout reads) still gives a curved wall its chord rectangle; the pony wall's lower layers in plan are bands without mitered ends.
- Stairs: guard railings around the stairwell opening, flared apron of Flare/Curve Stairs, railing across a landing in plan, a Stair Schedule, dashed hidden treads on other floors, and stair side railings are not clipped by the floor above in 3D (queue, Round 8 notes).
- Details (chapter 17): Line Style pages are disabled, moldings do not miter at polyline corners, "3D Solid Feature" custom molding profile has no editor; trim does not follow walls edited through the dialog or Delete All.
- Terrain: Select Objects now has `ObjectRef::TerrainObject` (editor/selection.rs), so the manual's statement that single terrain objects cannot be picked looks stale; terrain walls and curbs now cut the TIN (done 2026-10-08, see CB-46).
- Schedules: the placed tables are not in the DXF export or the construction-set PDF; the table is rebuilt every frame; Cabinet/Plant/Fixture rows come from placed symbols.
- Doc drift to correct in the manual: chapter 2.4 says the Fix Wall Connections button is not wired (it is: `editor/actions.rs`, `editor/connect.rs`); chapter 3 says pointer-derived swing/hinge is planned (fixed as QA-01); chapter 10.10 says 3D picking is planned for Round 9 (plan-side `pick_for_mesh_id` exists, no caller).

## Per-id status

### Walls (`docs/parity/walls.md`)

105 ids: 74 Works, 23 Partial, 6 Missing, 2 Differs-by-design.

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
| W-15 | Walls | Length and angle readouts while drawing | Works | tools/wall.rs update_readout + draw_overlay angle label; tools/wall.rs tests | Status bar and ghost show Length and Angle (degrees counter-clockwise from east, to 0.1); angle origin still a verify-in-Chief item. |
| W-16 | Walls | Type length/angle while drawing (Tab, Enter) | Works | editor/typed_input.rs, tools/wall.rs key; main.rs forwards text while armed | Digits fill the length (feet-inches, `12-6` too), Tab swaps Length/Angle, Enter draws and continues the chain, Esc drops the text first. Tab behavior still verify-in-Chief. |
| W-17 | Walls | Alt suspends all snaps | Works | snap.rs `suspend_all`; EditorContext::snap_at | Alt returns the raw point: object, angle and grid snaps all off, also for Select drags. |
| W-18 | Walls | Shift constrains to 0/90 | Works | tools/wall.rs snap (Shift hold), wall_edit::drag_end | Shift holds the Angle Snap increment (`editing.angle_snap_deg`, 15 by default, 45/90 from Snap Settings) even with angle snaps off; not a fixed 0/90 (verify in Chief). |
| W-19 | Walls | Lengths in project dimension format | Works | EditorContext::fmt_dim | - |
| W-20 | Walls | Ghost wall with real footprint | Works | wall.rs draw_overlay | Ghost does not run through join solver. |
| W-21 | Walls | Wall direction and exterior side | Works | plan-core walls.rs Wall.exterior_side, Side | - |
| W-22 | Walls | No auto interior/exterior detection | Works | tool chosen sets type; no auto flip | - |
| W-23 | Walls | Reverse Layers command | Works | editor/wall_edit.rs, plan-core walls.rs `reverse_layers` | Edit-toolbar button on any number of walls; flips `exterior_side` (the stack mirrors across the centerline), one undo step. |
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
| W-37 | Walls | Per-layer corner joins | Works | joins.rs wall_layer_outlines, curved_layer_outlines | A curved wall's layers are arcs (radial offsets) mitered at both ends to a straight or curved neighbour, tangent arcs join square, and a tee on an arc is cut to its face along the tangent. |
| W-38 | Walls | Any-angle miter | Works | joins.rs | - |
| W-39 | Walls | Through Wall At Start/End | Missing | dialogs/wall.rs Structure tab disabled | Not modelled. |
| W-40 | Walls | Join cleanup in same undo step | Works | connect.rs auto_connect; begin_change | - |
| W-41 | Walls | Fix Wall Connections | Works | editor/actions.rs FixWallConnections; connect.rs fix_wall_connections_action | Edit-toolbar button only, not a flyout tool. |
| W-42 | Walls | Repair duplicates and zero-length | Works | connect.rs test duplicate_wall_inside_another_is_dropped | - |
| W-43 | Walls | Break Wall at a point | Works | editor/wall_edit.rs break_wall_at/break_click; plan-core split_wall_at | Edit-toolbar Break Wall, then click the wall; openings go to the half that holds them; a break inside an opening or at an end is refused with a message. |
| W-44 | Walls | Add Break on walls | Works | editor/wall_edit.rs | Same command as Break Wall (as the spec says); Remove Break merges a straight continuation back. |
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
| W-57 | Walls | Glass wall and glass pony wall | Works | wall.rs tests; Glass-1 type; plan-3d tests/curved_walls.rs | No curved glass flyout entry; a glass wall bent with Change Line/Arc builds along the arc in 3D (panel cut by openings, one frame). |
| W-58 | Walls | Slab footing | Works | tools/details Slab Footing | - |
| W-59 | Walls | Wall hatching and material region | Works | tools/details.rs | - |
| W-60 | Walls | Heights follow platform defaults | Partial | dialogs/wall.rs Structure Default Wall Top Height | Bottom height default disabled. |
| W-61 | Walls | Default ceiling 109.125 in | Works | DEFAULT_CEILING_HEIGHT | - |
| W-62 | Walls | Platform intersection options | Missing | dialogs/wall.rs disabled | Framing-only option, disabled. |
| W-63 | Walls | Generate invisible walls between platforms | Missing | disabled | Not built. |
| W-64 | Walls | Curved wall three-click draw | Works | wall.rs tests curved_variants_take_a_third_click_for_the_arc, the_arc_passes_through_the_third_point, the_arc_readout_gives_radius_arc_length_and_chord | The third click is a point the arc passes through (not only the apex); the status line reads radius, arc length and chord. |
| W-65 | Walls | True arcs faceted by facet angle | Works | WallCurve; 7.5 deg facets in plan-3d; render.rs arc_facets | Plan drawing adds facets for a large radius at a high zoom (under 0.4 px off the curve); 3D shells use exact radial offsets. |
| W-66 | Walls | Curved wall dialog radius/lock | Partial | dialogs/wall.rs arc_section | General tab Arc section: Curved Wall check box, Radius, Arc Angle, Arc Rise, side, arc length and center readouts, all round-tripping; Radius-to and Lock options stay dimmed. |
| W-67 | Walls | Change Line/Arc on walls | Works | editor/wall_edit.rs change_line_arc; handles.rs HandleKind::Bulge; select.rs Op::WallBulge | Straight to arc (rise = chord/4) and back; the apex handle drags the bulge (grid unless Alt, up to a semicircle, near zero straightens); openings keep their proportion along the arc, and in 3D they are cut through the arc (jambs and head follow it, the door leaf or window unit stands square to the tangent at its center). |
| W-68 | Walls | Make Arc Tangent | Works | plan-core walls.rs make_arc_tangent; editor/wall_edit.rs | Refits the curved wall tangent to the connected wall at its start (else its end), straight or curved neighbour. |
| W-69 | Walls | Room from inside faces | Works | rooms.rs interior polygon; QA-03 fix | - |
| W-70 | Walls | Label area finished-floor | Works | QA-03 fixed, interior area | - |
| W-71 | Walls | Room labels name and area | Works | rooms_edit.rs | - |
| W-72 | Walls | Rooms update live | Works | EditorContext::refresh | - |
| W-73 | Walls | Auto exterior dims locate per defaults | Partial | tools/dimension.rs AutoExterior | Locate options partly honoured (see DIM). |
| W-74 | Walls | Wall temp dim along baseline | Works | tempdim.rs WallLength, WallRadius, WallArcLength | A curved wall shows its chord, radius and arc length (labels; no parallel-wall gaps along an arc). |
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
| W-90 | Walls | Convert wall to polyline | Works | editor/wall_edit.rs convert_to_polyline | Edit-toolbar Convert to Polyline: connected straight walls chain into one CAD polyline, an arc becomes 24 segments; the walls and their openings are removed (one undo step; no keep-the-wall prompt). |
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

112 ids: 76 Works, 28 Partial, 7 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| S-1 | Select | Click selects, shows handles and Edit toolbar | Works | tools/select.rs test click_selects_and_empty_click_deselects | - |
| S-2 | Select | Empty click deselects, selected click keeps | Works | select.rs same test | - |
| S-3 | Select | Pick by drawn geometry | Works | editor/selection.rs hit_test_cx | Text/door swing-arc tolerance not individually verified. |
| S-4 | Select | Overlap order, openings beat walls | Works | selection.rs hit_test_cx tiers | - |
| S-5 | Select | Hidden layer unselectable; locked layer viewable not editable | Works | editor/actions.rs check_unlocked, is_locked | - |
| S-6 | Select | Hover highlight and status description | Partial | select.rs update_hover; cx.hover | Hover highlight yes; status line does not name the object. |
| S-7 | Select | Double-click or Open Object opens spec | Works | select.rs double_click; shell/spec_dialogs.rs | - |
| S-8 | Select | Right-click context menu | Works | main.rs canvas_context_menu; edit_commands.rs context_entries | Right-click selects the object (a group member selects its group) and opens its menu: Open Object, the type's buttons, Cut/Copy/Paste/Delete, Select Same Type, Group/Ungroup, Lock, Send to Layer, Transform/Replicate. Empty space shows Paste, Paste Hold Position, Select All, Undo/Redo, zoom. Right-drag still pans. |
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
| S-32 | Select | Select Same Type | Partial | edit_commands.rs select_same_type | Select Same Type selects every object of the same type at once (Edit menu, toolbar, context menu); the version that waits for a marquee is not built. |
| S-33 | Select | Select All (Cmd+A) | Works | selection.rs select_all; edit_tests.rs | Cmd+A selects every object of the floor on a displayed, unlocked layer; the status bar gives the count. |
| S-34 | Select | Tab/Shift+Tab cycling | Works | select.rs test tab_cycles_through_the_objects_under_the_pointer | - |
| S-35 | Select | Group, ungroup | Works | edit_commands.rs group_selection/ungroup_selection; select.rs pointer_down; plan-core groups.rs | Group and Ungroup on the Edit toolbar, Edit menu, Cmd+G and the context menu; a click on a member selects the whole group; a group left with under two members dissolves. |
| S-36 | Select | Select group member | Works | select.rs cycle (Tab) | Tab cycling picks one member of a group. |
| S-37 | Select | Multi-select drag moves all | Works | select.rs Op::Group | - |
| S-38 | Select | Edit toolbar shows common commands for multi | Works | actions.rs common_edit_actions | Small command set. |
| S-39 | Select | Floating contextual Edit toolbar | Works | editor/actions.rs; tools/select.rs edit_toolbar | Fixed set per type. |
| S-40 | Select | Common buttons Transform, Reflect, Center, P2P, Layer, Same Type | Partial | actions.rs: Open, Delete, Copy, Paste in Place only | Everything beyond four buttons is missing. |
| S-41 | Select | Straight wall buttons list | Partial | wall_edit::edit_actions, select.rs edit_toolbar | Fix Wall Connections, Reverse Layers, Break Wall, Remove Break, Change Line/Arc, Convert to Polyline, Transform/Replicate, Reflect About Object, Point to Point Move, Center Object, Make Parallel/Perpendicular and Layer are on the Edit toolbar. |
| S-42 | Select | Curved wall buttons | Works | wall_edit::edit_actions | Change Line/Arc turns it straight again; Make Arc Tangent is enabled for curved walls. |
| S-43 | Select | Door/window buttons | Partial | EditActionKind::ReverseSwing | Reverse swing yes; Center, mulling absent. |
| S-44 | Select | Text buttons | Partial | actions.rs | Open/Copy/Delete only. |
| S-45 | Select | CAD line buttons | Partial | tools/cad/edit.rs modes (via CAD menu) | Parallel/Perp/Break/Convert are CAD-menu tools not Edit-toolbar buttons. |
| S-46 | Select | Each command one undo step | Works | begin_change in actions | - |
| S-47 | Select | Transform/Replicate Object | Works | dialogs/transform.rs; transform.rs transform_replicate | Transform/Replicate Object: copies, move (X,Y or distance and angle), rotate about the center or a point, resize %, reflect about an X or Y line; one undo step. |
| S-48 | Select | Reflect About Object/Line | Works | transform.rs Mode::Reflect; plan-core transform.rs | Reflect About Object: click a wall or CAD line; Reflect Copy keeps the originals. Reflect About Line by two typed points is not built. |
| S-49 | Select | Make Parallel/Perpendicular | Partial | tools/cad/edit.rs for CAD lines | Not for walls. |
| S-50 | Select | Break Line/Wall | Partial | cad edit Break Line; walls: core split_wall_at only | No wall Break button. |
| S-51 | Select | Fix Wall Connections on selection | Works | connect.rs fix_wall_connections_action | - |
| S-52 | Select | Point to Point Move | Works | transform.rs Mode::PointToPoint | Click the from point, then the to point; object snaps apply; walls move freely. |
| S-53 | Select | Center Object | Works | transform.rs center_opening_in_wall/center_between_walls/center_in_room | A lone door or window centers on its wall; other objects center in a clicked room or between two clicked walls. |
| S-54 | Select | Align/Distribute | Works | dialogs/transform.rs AlignDialog; plan-core transform.rs | Align Left/Center/Right/Top/Middle/Bottom and Distribute Horizontally/Vertically with equal gaps or a typed gap. |
| S-55 | Select | Reverse Layers | Works | editor/wall_edit.rs | See W-23. |
| S-56 | Select | Temp dims for selected wall | Works | editor/tempdim.rs | - |
| S-57 | Select | Door/window jamb dims | Works | tempdim.rs OpeningToStart/End | Opening width entry absent. |
| S-58 | Select | CAD line/box/circle temp dims | Missing | tempdim.rs only wall and opening kinds | None for CAD. |
| S-59 | Select | Click value, type, Enter, Tab, Esc | Works | select.rs key_while_editing | - |
| S-60 | Select | Referenced fixed, selected moves | Works | tempdim.rs apply | - |
| S-61 | Select | Wall length follows Lock | Works | tempdim.rs WallLength | - |
| S-62 | Select | Face-to-face to surfaces per Locate | Partial | tempdim.rs WallGap | Face-based; center-to-center option not offered. |
| S-63 | Select | Lockable temp dimensions | Missing | none | Not built. |
| S-64 | Select | Temp dims never printed or saved | Works | tempdim.rs | - |
| S-65 | Select | Edit Behaviors (Replicate, Resize, Concentric) | Partial | dialogs/edit_behaviors.rs, editor/behaviors.rs, select.rs apply hooks | Default, Resize (CAD, from the opposite corner, Shift keeps proportions), Concentric (polyline, line, circle, arc), Fillet (polyline corner drag), Alternate (axis lock), Replicate (CAD and walls, N copies); Chamfer mode and Replicate dialog hand-off absent. |
| S-66 | Select | Behavior indicator, reset | Missing | none | Depends on S-65. |
| S-67 | Select | Fillet/chamfer on CAD, not walls | Works | tools/cad/edit.rs | - |
| S-68 | Select | Object snap types with on/off | Works | dialogs/snap_settings.rs, snap.rs SnapSettings, `editing` defaults | Edit > Snap Settings: object snaps (master and Endpoint, Midpoint, Intersection, Perpendicular, Tangent, Center, Quadrant, On Object, Extension, Points/Markers), grid, angle snaps with the allowed-angle list, bumping, tolerance in pixels. Extension is off by default (a point near the line of any wall would jump onto it); Points/Markers is on. |
| S-69 | Select | Snap priority | Works | snap.rs header | Endpoint, Intersection, Midpoint, Center, Quadrant, Perpendicular, Tangent, On Object, then Angle and Grid. |
| S-70 | Select | Snap distance screen-space | Works | EditorContext::snap_tol | - |
| S-71 | Select | Angle snaps incl. parallel/perpendicular | Works | snap.rs angle_snap; wall.rs | Angle Snap Grid guide not drawn. |
| S-72 | Select | Grid snap unit | Works | defaults grid.snap | - |
| S-73 | Select | Bumping/pushing | Partial | tools/cabinet.rs neighbor alignment | Cabinets bump; no pushing. |
| S-74 | Select | Alt suspends all snaps | Works | snap.rs `suspend_all` | See W-17. |
| S-75 | Select | Cmd+Z/Cmd+Y undo and redo | Works | plan-core history.rs; scenarios s12_hotkeys | - |
| S-76 | Select | One gesture one step | Works | begin_change | - |
| S-77 | Select | Undo restores selection | Partial | selection.retain_existing | Selection kept only if object still exists. |
| S-78 | Select | Undo levels | Differs-by-design | history.rs cap 100 | Chief default 20; we keep 100. |
| S-79 | Select | Action History panel | Works | dialogs/action_history.rs | Edit/View > Action History lists the undo and redo steps; clicking one goes to it. |
| S-80 | Select | Spec edits undoable | Works | spec_dialogs one step | - |
| S-81 | Select | Copy/Cut/Paste with reference point | Works | clipboard.rs; edit_commands.rs | Cmd+C/Cmd+X take walls with openings, dimensions, CAD, text, cameras, cabinets, symbols, stairs, devices, framing, foundation objects, moldings, decks, floor regions, 3D solids and schedules; the reference point is the center of the copied box. Roof planes, terrain elements and wall trim are not copied (Copy says so, Cut refuses). |
| S-82 | Select | Paste attaches to cursor | Works | transform.rs Mode::Paste; edit_tests.rs | Cmd+V hangs the copy on the pointer; a click drops it and joins walls that touch; Esc or right click cancels. Paste As Group drops one group. |
| S-83 | Select | Paste Hold Position | Works | edit_commands.rs ids::PASTE_HOLD; toolbar.rs button | Paste Hold Position (button, Edit > Paste, Alt+Cmd+V) pastes at the original coordinates on the current floor. |
| S-84 | Select | Copy and Paste in Place (C,P,P) | Works | shell/hotkeys.rs edit_defaults | C, P, P copies and pastes in place in one step; the copy is selected. |
| S-85 | Select | Paste across files keeps layers | Partial | clipboard in memory only | No cross-file clipboard. |
| S-86 | Select | Rooms not copied | Works | groups.rs Clipboard | - |
| S-87 | Select | Delete multi, hosted openings | Works | select.rs test delete_removes_the_selection_with_its_openings_and_undoes | - |
| S-88 | Select | Delete Objects category dialog | Works | dialogs/delete_objects.rs | Shift+Space opens Delete Objects: tick types, this floor or all floors; hidden and locked layers stay; one undo step. |
| S-89 | Select | Locked layer delete refused | Works | actions.rs delete_selection | - |
| S-90 | Select | Edit Area commands | Missing | menus.rs:300 inert | Not built. |
| S-91 | Select | Stretch CAD | Missing | menus.rs:301 inert | Not built. |
| S-92 | Select | Arrow-key nudge | Works | select.rs nudge() | - |
| S-93 | Select | Enter opens spec; Delete deletes | Works | select.rs key(); edit_commands.rs duplicate_selection | Enter opens the specification, Delete deletes, Cmd+D duplicates 12" right and down. |
| S-94 | Select | Modifier table | Partial | select.rs | Shift and Alt only; Ctrl-drag copy absent. |
| S-95 | Select | Cursor shapes | Works | handles.rs CursorIcon | - |
| S-96 | Select | Fill Window Selected Objects | Partial | main.rs:155 fill_window; toolbar.rs:1971 | Fits whole plan; selected variant stub. |
| S-97 | Select | Selection highlight color | Works | render.rs | - |
| S-98 | Select | Status bar selection count and XYZ | Partial | main.rs:975 status_bar | X/Y only; no selection description or Z. |
| S-99 | Select | Auto-scroll on edge drag | Missing | none | Not built. |
| S-100 | Select | Esc cancels drag or deselects | Works | select.rs | - |
| S-101 | Select | Rotate about center / chosen center | Works | transform.rs rotate_selection, group_rotate_handle; dialogs/transform.rs | Rotate about the selection center or a typed point for walls, CAD, text, symbols, cameras, cabinets, stairs and devices. |
| S-102 | Select | Rotate multi-selection | Works | select.rs Op::GroupRotate | A Rotate handle above a multi-object selection turns every object about the box center; openings follow their walls. |
| S-103 | Select | Transform/Replicate dialog fields | Works | dialogs/transform.rs | Move, Rotate (About), Copy count, Resize %, Reflect axis. |
| S-104 | Select | Linear/radial arrays | Works | transform.rs transform_replicate | Copy k is the step applied k times: a move gives a linear array, a rotation a radial one. |
| S-105 | Select | Reflect with copy | Works | plan-core transform.rs xform_wall | A mirror flips each wall's exterior side so the exterior still faces outward; Copy leaves the originals. |
| S-106 | Select | Reflected doors flip | Works | plan-core transform.rs transform_objects | A mirrored door flips its swing. |
| S-107 | Select | Rooms selectable, spec on double-click | Works | select.rs test clicking_inside_a_room_selects_the_room_and_double_click_opens_it | - |
| S-108 | Select | Dimension handles, text move, add/remove point | Partial | handles.rs:106 | Move handle; extension-end resize and add/delete extension point limited. |
| S-109 | Select | Wall in loop selects alone | Works | select.rs | - |
| S-110 | Select | Hosted opening highlights host wall | Partial | selection | Not confirmed. |
| S-111 | Select | Reference display unselectable | Works | render.rs reference floor drawn separately | - |
| S-112 | Select | CAD layer order pick | Partial | selection.rs tiers | Active-layer favoring not confirmed. |

### Doors and windows (`docs/parity/doors-windows.md`)

110 ids: 70 Works, 23 Partial, 16 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| DW-1 | Doors/Windows | Placement tools place per click, tool stays active | Works | tools/opening.rs pointer_down; scenarios s02_openings | - |
| DW-2 | Doors/Windows | Ghost follows pointer with swing arc | Works | tools/opening.rs draw_overlay -> opening_view::draw_opening(ghost); scenarios s13 | - |
| DW-3 | Doors/Windows | No ghost off wall, no placement | Partial | opening.rs target() | No ghost; no "no" cursor glyph. |
| DW-4 | Doors/Windows | Refuse short wall/overlap; windows may touch and mull | Partial | s02_openings refused placement; Project::mull_openings | Touching windows still refused at placement (2 in gap); Mull closes gaps up to 12 in. |
| DW-5 | Doors/Windows | Click places; drag after click slides new opening | Missing | opening.rs pointer_down only | No placement-drag. |
| DW-6 | Doors/Windows | Defaults by type, exterior vs interior wall | Works | opening.rs test doors_and_windows_come_from_the_templates | - |
| DW-7 | Doors/Windows | Captured interior door defaults | Works | defaults.rs; opening.rs test interior_walls_get_the_interior_door | - |
| DW-8 | Doors/Windows | Swing side from pointer, hinge nearer end | Works | plan-core openings.rs door_defaults_for_pointer; QA-01 fixed | - |
| DW-9 | Doors/Windows | Center under pointer, 1 in snap | Works | opening.rs target() | Alt skips grid. |
| DW-10 | Doors/Windows | Alignment candidates (midpoint, equal spacing) | Missing | opening.rs target() grid only | No midpoint or equal-spacing snaps. |
| DW-11 | Doors/Windows | Temp dims to wall ends, neighbours, width | Works | tempdim.rs opening_temp_dims (width, jamb to neighbour or wall end); scenarios s13 | - |
| DW-12 | Doors/Windows | Type into jamb dims moves; width dim resizes | Works | tempdim.rs apply (OpeningWidth about center, else the jamb with room); select.rs typed length while a jamb or move handle is dragged; scenarios s13 | Which pivot Chief uses for a typed width is still verify-in-Chief. |
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
| DW-23 | Doors/Windows | Reflect mirrors openings | Works | plan-core transform.rs transform_objects | Mirroring a wall keeps its openings' offsets and flips the door swing. |
| DW-24 | Doors/Windows | Center Object | Works | editor/opening_edit.rs Center on Wall Segment -> Project::center_opening | - |
| DW-25 | Doors/Windows | One undo step per drag | Works | select.rs finish | - |
| DW-26 | Doors/Windows | Jamb resize handles | Works | handles.rs ResizeStart/ResizeEnd on openings; select.rs Op::OpeningResize; scenarios s13 | - |
| DW-27 | Doors/Windows | Resize snaps to grid/library widths | Works | plan-core openings/spec.rs StandardWidths (per-style lists, door and window fallbacks) + `snap_edge`; select.rs OpeningResize; dialogs/opening.rs General > Standard Widths (Default Settings dialogs); tests a_jamb_drag_lands_on_a_standard_width_when_snapping_is_on, standard_widths_snap_the_dragged_jamb | The option and lists set in the Default Settings dialog hold for the session; persisting them into the plan defaults needs the main.rs hook `OpeningDialog::apply_to_variants` (integration-queue). Alt skips the snap. |
| DW-28 | Doors/Windows | Resize clamps | Works | Project::resize_opening clamps (6 in min, 2 in clearance, neighbours); scenarios s13 | - |
| DW-29 | Doors/Windows | Height/sill via dialog | Works | dialogs/opening.rs Floor to Top/Bottom | - |
| DW-30 | Doors/Windows | Door panels calculated from width | Works | spec.rs door_panel_count / Opening::effective_style; dialogs/opening.rs Door Panels radios Single, Double, Calculate from Width; sliding_panels / bifold_panels; tests calculated_door_panels_draw_a_double_door_when_wide, a_calculated_door_is_double_when_wide | Hinged becomes double from 40 in (verify in Chief). Custom left/right counts not built. |
| DW-31 | Doors/Windows | Swing and hinge as four combinations | Works | Opening.swing_flipped + hinge_at_end | - |
| DW-32 | Doors/Windows | Reverse Swing and Flip Hinge buttons | Works | EditActionKind::ReverseSwing; Edit toolbar Flip Hinge (opening_edit.rs -> edit.flip_hinge) | - |
| DW-33 | Doors/Windows | Swing handle at leaf end | Works | handles.rs Swing handle; select.rs click reverses swing, Shift-click flips hinge | - |
| DW-34 | Doors/Windows | Swing Angle drives plan arc | Works | opening_symbol.rs plan_symbol uses extras.swing_angle_deg | - |
| DW-35 | Doors/Windows | Both-direction swing arcs | Missing | none | Not drawn. |
| DW-36 | Doors/Windows | Show Open in 2D | Works | opening_symbol.rs plan_symbol uses extras.show_open_in_plan (closed leaf, no arc) | - |
| DW-37 | Doors/Windows | Swing Toward Exterior toggle absent in Chief | Works | n/a | No gap. |
| DW-38 | Doors/Windows | Hinged symbol single and double | Works | opening_symbol.rs Hinged and DoubleDoor; tools/opening.rs Double Door entry | Double-door threshold width not modelled (own flyout entry instead). |
| DW-39 | Doors/Windows | Doorway symbol | Works | opening_symbol.rs Doorway; toolbar.rs door() D, W | - |
| DW-40 | Doors/Windows | Sliding symbol | Works | opening_symbol.rs Sliding (2 to 4 panels, arrow); plan-3d doors.rs; toolbar.rs S, D | - |
| DW-41 | Doors/Windows | Pocket symbol | Works | opening_symbol.rs Pocket (dashed pocket past a jamb); toolbar.rs D, P | Wall pocket framing not drawn. |
| DW-42 | Doors/Windows | Bifold symbol | Works | opening_symbol.rs Bifold (V per pair); plan-3d doors.rs | - |
| DW-43 | Doors/Windows | Garage symbol | Works | opening_symbol.rs Garage (panel and dashed overhead path); plan-3d doors.rs; toolbar.rs G, D | Default 108 x 96 in (variant defaults). |
| DW-44 | Doors/Windows | Barn door | Works | opening_symbol.rs Barn (panel off the face, track); plan-3d doors.rs | Overhang values not editable. |
| DW-45 | Doors/Windows | Shower and Fixed door | Works | opening_symbol.rs Shower (glass leaf and arc) and Fixed door (glass pane) | - |
| DW-46 | Doors/Windows | Change door type in dialog updates symbol | Works | dialogs/opening.rs Door Style / Window Style bound to Opening.style; plan, 3D and sketch follow | - |
| DW-47 | Doors/Windows | Window symbol, types | Works | opening_symbol.rs Window, Casement (swing arcs), Fixed, SlidingWindow, Awning, Hopper | Opening indicators (X, arrows tab) not built. |
| DW-48 | Doors/Windows | Bay, bow, box windows | Partial | opening_symbol.rs projection_footprint (bay, bow, box); plan-3d windows.rs shares it | Projects 18 in; no Bay/Bow tab for depth, seat or roof options. |
| DW-49 | Doors/Windows | Pass-through, wall niche | Works | opening_symbol.rs PassThrough and WallNiche (cut band); Opening::niche_depth (spec.niche_depth, 3 1/2 in default); dialogs/opening.rs Wall Niche > Niche Depth; plan-3d wall.rs hole_for; tests the_niche_depth_is_editable, the_niche_depth_is_read_from_the_opening | The niche always leaves 1 in of wall behind it. |
| DW-50 | Doors/Windows | Window sill default | Works | defaults template 24+72 | Manual: head 96 in. |
| DW-51 | Doors/Windows | Mulled windows | Works | Project::mull_openings / unmull_openings; opening_edit.rs Mull and Unmull; scenarios s13 | Auto-mull on touching placement not built (manual Mull). |
| DW-52 | Doors/Windows | Door plus sidelite mull | Partial | Project::mull_openings accepts one door (hinged, double, doorway, fixed) plus windows; opening_edit.rs Mull from the door or a sidelite; plan-3d Unit (one frame post, one casing loop); opening_symbol.rs casing_parts (one pair per unit); tests a_door_and_its_sidelite_share_one_casing_loop, a_door_mulls_with_the_sidelite_beside_it_and_not_with_a_second_door | Sidelites beside a door only: a transom over a door needs openings that overlap in plan, which the placement rules refuse. |
| DW-53 | Doors/Windows | Window cannot span a junction | Works | Project::add_opening bounds; split_wall_at refuses | - |
| DW-54 | Doors/Windows | Size fields recompute | Works | dialogs/opening.rs | Elevation Reference disabled. |
| DW-55 | Doors/Windows | Library door sets style; Components | Partial | Library Style stored | Components tab disabled. |
| DW-56 | Doors/Windows | Clearance gaps | Missing | dialogs disabled | 3D only in Chief. |
| DW-57 | Doors/Windows | Depth follows wall; Recessed into Wall | Partial | Options disabled sections | Recessed not supported. |
| DW-58 | Doors/Windows | Openings in curved walls | Partial | manual 2.2 offsets along arc | No 3D cut, no radial symbol. |
| DW-59 | Doors/Windows | Plan label with size shorthand | Works | openings.rs size_text / plan_label; opening_view.rs draws the label; scenarios s13 | Fractions round down to the next-lower inch (verify in Chief). |
| DW-60 | Doors/Windows | Schedule number in label | Works | opening_view.rs shows the schedule mark in its bubble when a schedule numbers the opening | - |
| DW-61 | Doors/Windows | Schedule numbering/renumber | Partial | plan-docs schedule_kinds.rs reading-order marks; Opening.schedule_number override | A new opening takes the next mark automatically (computed); no Renumber command. |
| DW-62 | Doors/Windows | Specify Label with macros | Works | dialogs/opening.rs Label tab -> Opening.label_override / extras.label; macros in Opening::plan_label | - |
| DW-63 | Doors/Windows | Label placement/layer/handle | Works | LayerSet default "Doors, Labels" / "Windows, Labels" (ensure_opening_label_layers on first placement, layer sets follow Doors/Windows); opening_view.rs labels_visible, drag_label; spec.label_offset; handles.rs HandleKind::Label; select.rs Op::OpeningLabel; Reset Label Position; tests labels_have_their_own_layers_that_hide_them_alone, the_label_handle_drags_the_label_as_one_undo_step, the_schedule_mark_bubble_follows_the_dragged_label | Label angle not editable. The embedded template JSON predates the layers (a new plan gets them with its first opening). |
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
| DW-77 | Doors/Windows | Create, number, select, one undo | Works | tools/opening.rs pointer_down selects the new opening, edit_toolbar shows its Edit toolbar, Esc lets go (a second Esc ends the tool); marks computed by schedule order; tests the_new_opening_is_selected_with_its_edit_toolbar_and_esc_lets_go, scenarios s02_openings | - |
| DW-78 | Doors/Windows | Door on window refused | Works | overlap rule | - |
| DW-79 | Doors/Windows | Casing drawn in plan | Partial | opening_symbol.rs casing_parts; opening_view.rs draw_casing; Casing tab > Show Casing in Plan (spec.casing_in_plan); honors Use Interior / Exterior Casing; a mulled unit has one pair | Off by default; the PDF sheet and the layout do not draw it yet. |
| DW-80 | Doors/Windows | Plinth blocks elevation | Missing | disabled | Not built. |
| DW-81 | Doors/Windows | Threshold line | Missing | none | Not drawn. |
| DW-82 | Doors/Windows | Jamb thickness / size includes jamb | Partial | Frame tab width stored (frame_width) and drawn in plan as jamb blocks (opening_symbol.rs add_frame_blocks) and as the 3D frame; sash widths on the Sash tab | Door jambs are not drawn in plan; Size Includes Frame is not applied. |
| DW-83 | Doors/Windows | Opening Indicators | Missing | tab disabled | Not built. |
| DW-84 | Doors/Windows | Shutters in plan | Works | spec.shutters (Shutters tab: style, sides, width, color, closed, outside casing); opening_symbol.rs add_shutters; plan-3d casing.rs add_shutters (also in the elevations, which are cut from the 3D scene); tests shutters_are_rectangles_outside_the_wall, shutters_stand_beside_or_over_the_opening_on_the_outside | The color is stored; the plan draws the outline and 3D the trim material (no per-mesh color yet). Exterior walls only. |
| DW-85 | Doors/Windows | Sill line past exterior face | Missing | none | Not drawn. |
| DW-86 | Doors/Windows | Egress flag metadata | Works | extras egress; plan-check bedroom_egress | - |
| DW-87 | Doors/Windows | Refused across junction | Partial | add_opening vs wall ends | T through-wall face clearance not modelled. |
| DW-88 | Doors/Windows | Curved wall opening symbol radial | Missing | manual 2.2 | Not drawn radially. |
| DW-89 | Doors/Windows | Shared segment windows | Partial | Project::mull_openings (zero gap inside a unit) | Touching only through Mull. |
| DW-90 | Doors/Windows | Garage door too wide refused | Works | add_opening | - |
| DW-91 | Doors/Windows | Foundation wall openings | Partial | same tool | No foundation-specific defaults. |
| DW-92 | Doors/Windows | No openings on invisible walls | Missing | none | Not restricted. |
| DW-93 | Doors/Windows | Delete opening re-closes wall | Works | select.rs delete | - |
| DW-94 | Doors/Windows | Undo releases number | Works | marks are computed from the plan, so undo frees them; scenarios s13 | - |
| DW-95 | Doors/Windows | Acceptance: centered door, swing, dims | Works | scenarios/s02_openings | - |
| DW-96 | Doors/Windows | Acceptance: slide with snap | Works | select.rs test dragging_an_opening_slides_it | - |
| DW-97 | Doors/Windows | Acceptance: Reverse Swing, Flip Hinge | Partial | Reverse Swing button | Flip Hinge button missing. |
| DW-98 | Doors/Windows | Acceptance: mulled windows | Works | opening_edit.rs Mull / Unmull; scenarios s13 two_adjacent_windows_mull_into_one_unit | - |
| DW-99 | Doors/Windows | Acceptance: Lock End keeps distances | Works | dialogs/wall.rs test length_lock_end_keeps_openings_in_place | - |
| DW-100 | Doors/Windows | Acceptance: save and reopen preserves | Partial | extras.rs round-trips; Opening.style, label settings, mull group, lites, interior casing width/depth/reveal and the whole Specification spec (sash, lintel, arch, hardware, shutters, niche depth, label offset) saved | Exterior casing widths are still session-only (one casing per opening). |
| DW-101 | Doors/Windows | Spec dialog with preview | Works | dialogs/opening.rs | - |
| DW-102 | Doors/Windows | Width edit clamps with warning | Works | dialogs/opening.rs clamp_center | - |
| DW-103 | Doors/Windows | Style swaps defaults | Works | dialogs/opening.rs style combo keeps the size (spec) | - |
| DW-104 | Doors/Windows | Label tab live preview | Works | dialogs/opening.rs | - |
| DW-105 | Doors/Windows | Defaults never alter placed | Works | cx.opening_template | - |
| DW-106 | Doors/Windows | Cancel/OK one undo | Works | scenarios s02 | - |
| DW-107 | Doors/Windows | Door/window hotkeys | Partial | toolbar.rs BINDINGS: D,H D,W D,P S,D G,D Shift+W | The Ctrl+Alt+Cmd chords come from Daniel's hotkey file only. |
| DW-108 | Doors/Windows | Flyout face and hint | Works | toolbar.rs; opening.rs hint | - |
| DW-109 | Doors/Windows | Esc returns to Select; right-click menu | Works | main.rs has_canvas_menu | Door and window tools show the context menu with Select Objects at the top; the other drawing tools keep right click as Esc. |
| DW-110 | Doors/Windows | Alt disables alignment snaps | Works | opening.rs target(alt) | - |

### Rooms and floors (`docs/parity/rooms-floors.md`)

71 ids: 45 Works, 24 Partial, 2 Missing, 0 Differs-by-design.

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
| R-11 | Rooms/Floors | Nested rooms ring polygon | Works | rooms.rs nest_rooms (Room.holes); slab holes in plan-3d lib.rs add_floor; rooms_edit.rs tests | An island loop is its own room; the room around it excludes its area (centerline, interior, standard), has a platform hole under it, and names/picks inside the island go to the island. |
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
| R-26 | Rooms/Floors | Stem wall at dropped room | Works | RoomName.stem_wall_height; plan-3d slab.rs stem_walls; tests/roof_detail.rs | A dropped room (garage) and a room with a Stem Wall get concrete stem walls along their exterior walls from the underside of the floor platform up to the floor datum; they stop at garage doors. The thickness is the first Foundation wall type's. |
| R-27 | Rooms/Floors | Floor and ceiling finish thickness | Partial | RoomName finish names | Thickness not stored. |
| R-28 | Rooms/Floors | Floor Structure Define dialog | Works | dialogs/room.rs define_editor; RoomMisc.floor_structure | Layer stack (material, thickness, order) per room, stored; the total is the 3D floor platform thickness. Components tab still shows finishes only. |
| R-29 | Rooms/Floors | Ceiling Structure Define | Works | dialogs/room.rs define_editor; RoomMisc.ceiling_structure | Same editor for the ceiling platform thickness. |
| R-30 | Rooms/Floors | Floor/Ceiling/Roof Over This Room | Works | RoomName.has_floor/has_ceiling and RoomMisc.roof_over/flat_roof (saved with the plan); plan-3d slab.rs room_levels; roof_view.rs room_roof | Floor Under and Ceiling Over regenerate the 3D platforms. Build Roof leaves a room with Roof Over off out of the footprint (its partition becomes the roof edge), cuts a hole when the room lies inside one plane, and builds a level plane at the ceiling of a Flat Roof room. A wall that spans a skipped room and a roofed one keeps its roof (the editor splits walls at tees). |
| R-31 | Rooms/Floors | Monolithic slab flag per room | Missing | none | Foundation tool covers slabs separately. |
| R-32 | Rooms/Floors | Sloped ceiling via ceiling planes | Partial | roof_view Build Ceiling Planes | Room Ceiling Height not tied to plane. |
| R-33 | Rooms/Floors | Structure edits update 3D at once | Works | QA-02 fix; project_hash includes room_names | - |
| R-34 | Rooms/Floors | Room moldings | Partial | RoomName.moldings | Base/Crown names stored; 3D molding geometry not built. |
| R-35 | Rooms/Floors | Fill Style | Works | RoomName.fill_style; render | - |
| R-36 | Rooms/Floors | Materials per room | Partial | floor_finish/ceiling_finish | Other surfaces planned. |
| R-37 | Rooms/Floors | Room Types list | Works | dialogs/default_lists.rs | - |
| R-38 | Rooms/Floors | Room Types buttons | Partial | manual 4.10: Add, Edit, Rename, Delete | No Copy, Select All, Clear All. |
| R-39 | Rooms/Floors | Type feeds dropdown and schedules | Works | rooms_edit.rs | - |
| R-40 | Rooms/Floors | Function built-in behavior | Partial | rooms.rs function_defaults; plan-3d room_function tests | Garage drops 24" on a 4" slab with no finish; Deck and Porch have no ceiling and a deck or slab platform; Open Below, Attic and Courtyard have no floor and Open Below opens the ceiling below. Garage stem walls are built (R-26); the Flat Roof function has no ceiling and a membrane deck; the Flat Roof Over This Room directive puts a level roof plane over the room. |
| R-41 | Rooms/Floors | Function sets checkbox defaults | Works | dialogs/room.rs apply_function_defaults on type change; rooms_edit.rs new_room_draft | Changing the room type refreshes the Structure switches, floor height, finish and Floor Structure; each stays editable. |
| R-42 | Rooms/Floors | Living Area override | Works | rooms.rs living_area_sq_ft | - |
| R-43 | Rooms/Floors | Conditioned override | Partial | RoomName.conditioned | Not used by energy/finish schedule. |
| R-44 | Rooms/Floors | Draggable room label | Works | rooms_edit.rs label_pointer_*; RoomLabelOptions.offset; select.rs | Press on a label picks its room and drags it; the offset is stored with the room (Reset Position on the Label tab). Label macros (`<name>`, `<area>`, `<dims>`, `<ceiling>`...) in the Label tab. |
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
| R-56 | Rooms/Floors | Floor Defaults dialog | Works | dialogs/floor_defaults.rs; Floor.settings; toolbar button, Build > Floor, Default Settings | Ceiling height, floor and ceiling structure and finish thickness, default room type and materials per floor, and the plan defaults new floors start from. |
| R-57 | Rooms/Floors | Template floor defaults | Works | FLOOR_PLATFORM_THICKNESS, DEFAULT_CEILING_HEIGHT | - |
| R-58 | Rooms/Floors | Floor height change cascades | Works | floors.rs apply_floor_settings, restack_floors | Ceiling height or structure change moves the floors above; walls at the old ceiling height follow. |
| R-59 | Rooms/Floors | Build New Floor dialog | Works | floors.rs build_new_floor_with; dialogs/floor.rs | Derive exterior walls or all walls or blank; copy rooms and slab data; above or below; heights from Floor Defaults or the source; builds a foundation when missing. Insert New Floor Below added. |
| R-60 | Rooms/Floors | Insert and Delete Floor | Works | floors.rs insert_floor_above, delete_floor | Insert only above. |
| R-61 | Rooms/Floors | Build Foundation types | Partial | floors.rs build_foundation; FoundationKind | Walls with footings, monolithic, piers; no basement. |
| R-62 | Rooms/Floors | Pier and monolithic detail | Partial | tools/foundation.rs slabs/piers | Not tied to Build Foundation. |
| R-63 | Rooms/Floors | Delete Foundation | Works | floors.rs | - |
| R-64 | Rooms/Floors | Exchange with floor above/below | Works | floors.rs exchange_floors | - |
| R-65 | Rooms/Floors | Reference Display dialog, dimmed, snappable | Works | dialogs/reference_display.rs (reference_walls); render.rs draw_reference_floor; snap.rs snap_with_reference | Dialog picks the floor (below, above, any), layer set and color; the reference floor's walls draw dimmed and their wall ends and centerline crossings snap (the active floor's own ends win). Walls only; reference objects cannot be picked. |
| R-66 | Rooms/Floors | Reference respects layer visibility | Works | render.rs:495 | - |
| R-67 | Rooms/Floors | Switching floors keeps view/tool | Works | main.rs | - |
| R-68 | Rooms/Floors | Attic floor | Partial | FloorKind::Attic | Not auto-created by Build Roof. |
| R-69 | Rooms/Floors | Platform intersections | Missing | wall Structure tab disabled | Not built. |
| R-70 | Rooms/Floors | Saved views pin a floor | Partial | Project.plan_views | Layer views yes; floor pinning unverified. |
| R-71 | Rooms/Floors | Floor elevation cascade | Partial | floors.rs restack_floors | Stored elevation recomputed on rebuild only. |

### Roofs (`docs/parity/roofs.md`)

60 ids: 32 Works, 22 Partial, 6 Missing, 0 Differs-by-design.

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
| RF-13 | Roofs | Butting roofs and split walls | Works | plan-3d cover.rs trim_plane, Butt, wall_split; wall.rs Split | A lower plane is trimmed at the face of a taller wall that rises above it, a flashing line marks the butt, and the part of the wall under the butting roof takes the Lower Wall Type (RF-28). |
| RF-14 | Roofs | Plane thickness, underside, fascia | Works | plan-3d cover.rs RoofDetail; plan-core RoofDetailDefaults; Roof Defaults page and the Build Roof Detail tab | Thickness and the fascia sizes come from the Roof Defaults, stored with the roof (settings record `detail`). |
| RF-15 | Roofs | Eave detail: cut, soffit, tails | Works | plan-3d eave.rs (eave cut plumb/level/square, rafter tails, gutters, soffit level/sloped, fascia, rake fascia and soffit, frieze, ridge caps); dialogs/roof.rs plane Options tab | Eave cut and rafter tails are Roof Defaults and per-plane choices (Default/On/Off). Tails replace the soffit on the eaves they stand under. A level cut lays the fascia flat under the structure, a square cut stands it square to the plane. |
| RF-16 | Roofs | Roof cuts walls; gable walls reach roof | Works | plan-3d cover.rs wall_top, wall_top_clipped, bottom_cut; wall.rs build_wall_shaped; wall_kinds.rs class_top; tests/roof_walls.rs, tests/roof_detail.rs | Wall tops are cut to the roof underside and gable walls reach the ridge; half, pony, foundation and curved walls are cut like standard walls but never raised; Roof Cuts Wall at Bottom cuts a wall's bottom along the roof under it (straight walls, centerline; it reaches down to the roof only where Auto Attic Walls is off, since an attic wall fills that gap otherwise). Build Roof's baseline-at-plate rule puts the underside at the plate so gable corners meet it. |
| RF-17 | Roofs | Deterministic rebuild | Works | plan-roof tests | - |
| RF-18 | Roofs | Per-wall roof directives UI | Partial | Wall.roof in walls.rs; tools/roof.rs flip | Wall Roof tab disabled; only Hip/Full Gable via tool. |
| RF-19 | Roofs | Hip wall | Works | roof_view.rs RoofWallKind::Hip | - |
| RF-20 | Roofs | Full gable wall | Works | cover.rs Drive::Roof (FullGable wall or gable edge override); tests/roof_walls.rs | - |
| RF-21 | Roofs | Dutch gable | Missing | manual 8.1 not modelled | - |
| RF-22 | Roofs | High shed/gable | Works | cover.rs Drive::Roof (HighShedGable) | - |
| RF-23 | Roofs | Knee wall | Partial | cover.rs Drive::Roof (KneeWall) | The wall rises to the plane above it; Build Roof already leaves knee walls out of the footprint; no Wall Specification control. |
| RF-24 | Roofs | Extend slope downward | Partial | roof_view.rs EXTEND_SLOPE_DROP | Fixed 24 in drop, not to the wall below. |
| RF-25 | Roofs | Upper pitch break (gambrel, mansard) | Missing | manual 8.1 | Fields in model, not read by builder. |
| RF-26 | Roofs | Overhang length | Works | plan-roof spec.rs | - |
| RF-27 | Roofs | Auto Roof Return | Partial | roof_view.rs RF-27; Roof Return tool | Fixed 24 in; no slope/extend/shadow-board options; flag set from data only. |
| RF-28 | Roofs | Lower wall type if split by butting roof | Works | plan-3d cover.rs wall_split; lib.rs split_for; Roof Defaults "Lower Wall Type" | The exterior face under a butting roof's line takes the lower wall type; where no roof butts a wall that rises above its plate, the part above the plate takes the attic wall type (RF-31). One split per wall: the butting roof wins. |
| RF-29 | Roofs | Bay/box/bow roof attach | Missing | none | Depends on bay windows. |
| RF-30 | Roofs | Only exterior walls feed roof | Works | roof_view.rs | - |
| RF-31 | Roofs | Attic walls auto-generated | Partial | plan-3d cover.rs attic panels, RoofDetail::auto_attic_walls, RoofTypes::attic | Generated at scene time above a lower roof beside a taller wall (not stored, not selectable); the switch and the attic wall type are Roof Defaults now. |
| RF-32 | Roofs | Attic floor only when built | Works | FloorKind::Attic | - |
| RF-33 | Roofs | Attic walls cut by roof | Partial | wall_top applies to every standard wall incl. attic-floor walls | No attic floor platform of its own beyond FloorKind::Attic. |
| RF-34 | Roofs | Dormer walls as walls | Partial | Explode Dormer makes real walls | Auto dormer keeps walls inside the record. |
| RF-35 | Roofs | Roof Plane tool | Works | tools/roof.rs RoofMode::Plane | Rectangular only; no polyline planes. |
| RF-36 | Roofs | Roof Plane Specification | Partial | dialogs/roof.rs | No Structure Define; the Options tab holds the eave cut, rafter tails, fascia, soffit, frieze and gutters. |
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
| RF-60 | Roofs | Roof quantities in Materials List | Works | plan-docs materials.rs roofing lines from the stored roof planes (area, squares, shingle bundles, underlayment, drip edge, gutters; waste from the Master List); test for the 40x30 house | Reads `Floor.roofs` plane records; no ridge cap or flashing lines. |

### Dimensions, text, CAD, layers (`docs/parity/dimensions-text-cad.md`)

121 ids: 79 Works, 37 Partial, 4 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| DIM-1 | Dimensions | Dimension objects on manual/automatic layers | Works | render.rs draw_dimension; plan-core dimension.rs | - |
| DIM-2 | Dimensions | Multi-point string with offset | Partial | Dimension stores two measured points; Running/Baseline emit several | Strings are separate dimension objects. |
| DIM-3 | Dimensions | Associative to objects | Works | plan-core dim_assoc.rs (DimAnchor on walls, openings, cabinets, fixtures; Floor::sync_dimension_anchors from EditorContext::refresh); tools/dimension.rs commit/finish_edit | A point dragged onto a new object ties there. Deleting the object (wall, opening or cabinet) frees the point where it was (dim_assoc.rs tests); Chief's own behavior still to verify. |
| DIM-4 | Dimensions | Locate to wall surfaces per Locate setting | Works | DimensionDefaults locate_walls/openings/cabinets/fixtures/interior flag; default_lists.rs Locate Objects tab; tools/dimension.rs locate() | Settings are per saved set; no separate exterior/interior defaults (verify in Chief). |
| DIM-5 | Dimensions | No Locate walls skipped | Works | WallFlags.no_locate; dimension.rs | - |
| DIM-6 | Dimensions | Dimension Defaults formats | Works | dialogs/default_lists.rs Saved Dimension Defaults; 14 template sets | Secondary format absent. |
| DIM-7 | Dimensions | Printed-size text and offsets | Works | render.rs DimLook; DimensionDefaults.printed_size; TextStyle printed size | Text follows the set's text style; arrows and extension gaps hold their size on paper when the set's Printed Size is on. |
| DIM-8 | Dimensions | Value text format and placement | Works | render.rs draw_dimension_look, upright_angle; DimFormat::fmt_len | Text runs along the line and reads upright (verticals bottom to top). |
| DIM-9 | Dimensions | Text outside when tight | Works | render.rs draw_dimension_look | Number moves beyond the line end with a short leader; end ticks stay. |
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
| DIM-24 | Dimensions | Auto Exterior Dimensions strings | Works | plan-core dimension.rs auto_exterior_set; DimensionDefaults.auto_strings | Openings, wall to wall and overall per side, configurable; every wall direction (frames modulo 90 degrees). A curved exterior wall joins by its chord (its tangent points are breaks of the wall-to-wall string, its bulge pushes the strings out and counts toward the overall); a curve whose chord has no straight wall frame is skipped, and openings on the arc are not strung. |
| DIM-25 | Dimensions | Re-run replaces autos, keeps manual | Works | tools/dimension.rs clear_run; Dimension.auto_group | Edited automatic strings become manual and stay. |
| DIM-26 | Dimensions | Uses Locate; skips No Locate | Works | auto_exterior_set ExteriorSetup; tools/dimension.rs auto_exterior | Surfaces, main layer or centers; opening sides or centers. |
| DIM-27 | Dimensions | Auto Interior | Works | DimMode::AutoInterior | Clear spans plus an openings string along each room wall. |
| DIM-28 | Dimensions | Auto Elevation / Story Pole (elevation views) | Differs-by-design | dimension.rs plan-axis strings | Plan strings, not elevation view objects. |
| DIM-29 | Dimensions | Auto dims linked to walls | Works | dim_assoc.rs; tools/dimension.rs add_run | Strings follow their walls and openings until edited. |
| DIM-30 | Dimensions | Dimension handles | Works | editor/handles.rs:106 | Add/delete extension point limited. |
| DIM-31 | Dimensions | Dimension Specification | Partial | dialogs/dimension.rs | Format/Arrow/Text tabs disabled (display from defaults). |
| DIM-32 | Dimensions | Edit value moves object | Works | dimension.rs inline edit | - |
| DIM-33 | Dimensions | Same for temp; auto becomes manual | Works | tools/dimension.rs finish_edit; dim_assoc.rs sync; dialogs/dimension.rs | Handle drags, typed values, dialog edits and hand-moved ends make an automatic string manual. Temp-dimension typing is in tempdim.rs. |
| DIM-34 | Dimensions | Locked layer refuses | Works | check_unlocked | - |
| DIM-35 | Dimensions | Line stays when object moves | Works | dim_assoc.rs sync_dimension | The measured points follow the object and the manual dimension line stays put (only the value changes); automatic strings keep their distance from the wall. |
| DIM-36 | Dimensions | Align/Distribute dims | Partial | plan-core dimension.rs align_dimensions/distribute_dimensions; tools/dimension.rs edit_actions, run_command (`dim.align`, `dim.distribute`) | Align Dimensions puts the parallel dimension lines on the first selected one's line; Distribute spaces three or more evenly. Edit toolbar buttons show while the Dimension tool is active; the Select tool's toolbar needs the one-line hook (`tools::dimension::edit_actions` in `dispatch.rs::extra_edit_actions`). |
| DIM-37 | Dimensions | Reverse Dimension | Partial | plan-core Dimension::reverse; tools/dimension.rs run_command (`dim.reverse`) | The dimension line lands on the other side of the objects it measures. Same Edit toolbar limit as DIM-36. |
| DIM-38 | Dimensions | Copy with referenced objects | Partial | actions.rs copy dimensions | Copied as free dimensions. |
| DIM-39 | Dimensions | Dimension Specification tabs | Partial | dialogs/dimension.rs | Primary Format/Arrow/Text Style tabs disabled. |
| DIM-40 | Dimensions | Dimension Defaults groups (manual, auto, temp, elevation) | Partial | DimensionDefaults.temp_locate/elevation_locate (LocateGroup); default_lists.rs Locate Objects tab (Manual and Automatic / Temporary / Elevation); tempdim.rs TempLocate | The temporary dimensions locate by their own group: wall gaps between faces, main layers or centerlines, opening distances jamb to jamb or center to center; typing a value still moves the object. The Elevation group is stored and editable, but the Auto Elevation and Story Pole strings measure levels, not located walls or openings, so it has nothing to drive yet. |
| DIM-41 | Dimensions | Edit toolbar for dimensions | Partial | actions.rs; tools/dimension.rs edit_actions | Reverse Dimension, Convert to Manual, Align and Distribute Dimensions (Dimension tool active). Add/Delete Extension absent (extension lines switch per point in the Dimension Specification). |
| DIM-42 | Dimensions | Acceptance: auto exterior 20x12 | Works | scenarios/s07_dimensions_text_cad.rs | - |
| DIM-43 | Dimensions | Acceptance: edit dimension text moves wall | Works | s07 / dimension.rs inline edit | - |
| DIM-44 | Dimensions | Acceptance: interior dimension clear span | Works | dimension.rs Interior | - |
| DIM-45 | Dimensions | Acceptance: angular dimension 90 and 135 | Works | dimension.rs Angular | - |
| TXT-1 | Text | Text tool, click-drag box | Partial | tools/text.rs TextMode::Text | No wrap-width drag box. |
| TXT-2 | Text | Printed size scaling | Works | text_styles.rs TextStyle::text_height/drawn_height/placed_height; render.rs printed_text_object, settle_text_hits; default_lists.rs Size by | A style set to Printed Size draws, picks and exports at its size on paper for the sheet scale; the template styles stay Character Height. Callout and marker shapes are sized for the text as drawn at placement. |
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
| TXT-19 | Text | Acceptance: printed 1/8 in text | Works | render.rs printed_size_text_and_dimension_numbers_follow_the_sheet_scale; tools/text.rs printed_size_text_is_placed_at_the_style_height_and_picked_by_its_drawn_box; plan-docs sheet.rs printed_size_text_prints_the_same_size_at_any_sheet_scale; dxf.rs text_heights_come_from_the_styles_at_the_sheet_scale | Text on a printed-size style is placed at the style's character height, drawn, picked (Text and Select tools) and exported (DXF at the export scale, sheet PDF at the sheet scale) at 1/8 in on paper. |
| CAD-1 | CAD | Current CAD layer | Partial | plan-core layers.rs `current_cad_layer`; tools/cad.rs `draw_layer`; Tools > Layer Settings > Active Layers by Tool | Drawing follows the chosen layer; the toolbar button is still a stub. |
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
| CAD-40 | CAD | Snap to CAD geometry (tangent, quadrant) | Works | snap.rs cad_rounds/cad_segments/cad_intersections/on_extension/cad_marker_points | Endpoint, Midpoint, On Object of CAD lines and polylines; Center, Quadrant, Tangent of circles and arcs; Intersection of CAD with CAD (lines, polylines, circles, arcs) and with walls; Extension of a wall or CAD line beyond its end (off by default); Points/Markers (Place Point, Point Marker, numbered markers). |
| CAD-41 | CAD | Edit toolbar for line/arc/polyline | Partial | actions.rs | Edit tools live in CAD menu not toolbar. |
| CAD-42 | CAD | Edit toolbar for circle/ellipse | Partial | actions.rs | Open/Copy/Delete only. |
| CAD-43 | CAD | Edit commands one undo step | Works | edit.rs | - |
| CAD-44 | CAD | Acceptance: Input Line 12 ft at 30 deg | Works | tools/cad edit_tests / s07 | - |
| CAD-45 | CAD | Acceptance: polyline to walls | Works | plan_import::cad_to_walls | - |
| LAY-1 | Layers | Layer holds display, lock, color, weight; ByLayer | Works | plan-core layers.rs | - |
| LAY-2 | Layers | Layer sets and views | Works | plan-core layer_sets.rs; Project.plan_views | - |
| LAY-3 | Layers | Active Layer Display Options dock | Works | dialogs/layer_display.rs (dock and modal); shell/docks.rs `edit_layers` | Columns Name, Used, Disp, Lock, Ref, Color, Weight, Line Style, Text Style; every cell edits the shown layer set (or every set with Modify All Layer Sets) for the whole row selection; name filter, sort, Select All/None, Reset, Select Objects, Copy To Other Sets; New, Copy Set, Rename, Delete beside the set selector. |
| LAY-4 | Layers | Hidden layer still in model | Works | render filters only | - |
| LAY-5 | Layers | Locked layers | Works | actions.rs is_locked | - |
| LAY-6 | Layers | Current CAD layer button | Partial | dialogs/layer_sets.rs (Active Layers by Tool, Tools > Layer Settings); plan-core layers.rs `tool_layer`; tools/cad.rs `draw_layer` | New CAD objects go on the Current CAD Layer, saved in the plan; the table also stores a layer for 13 other tools, but only the CAD tools read it. The toolbar's Current CAD Layer button (toolbar.rs) is still a stub. |
| LAY-7 | Layers | Line weights toggle | Works | restyle.rs | - |
| LAY-8 | Layers | Save layer set | Works | dialogs/layer_sets.rs (Layer Set Management: New, Copy, Rename, Delete, Make Active, Import From Plan File); dialogs/plan_views.rs (Save Plan View, Reset Plan View, Plan View Specification) | Plan view tabs (editor/plan_tabs.rs) keep each view's floor, layer set, reference display, zoom and pan; the Project Browser lists Plan Views (double-click opens a tab, drag reorders), Cameras (rename, delete), Schedules and CAD Details (jump). Pan and zoom restore needs the shell's two-line hook (see plan_tabs.rs). |
| LAY-9 | Layers | Reference Display | Works | render.rs draw_reference_floor; snap.rs snap_with_reference | The reference floor's walls draw dimmed and snap (ends and crossings). Walls only. |
| LAY-10 | Layers | Reference options | Works | dialogs/reference_display.rs; Layer.reference; the Ref column in the Layer Display Options table (shell/docks.rs) | The Reference Display dialog picks the floor, layer set and color; a layer's Ref box decides whether its objects draw and snap on the reference floor. |
| LAY-11 | Layers | Default layer set names | Works | defaults default_floor_plan | - |
| LAY-12 | Layers | Layer changes undoable | Works | manual 5.5 | - |
| LAY-13 | Layers | Per-object Layer command | Partial | spec dialogs have Layer tab | No Edit-toolbar Layer button. |
| LAY-14 | Layers | Acceptance: layer off/lock | Works | s10/s12 scenarios | - |
| LAY-15 | Layers | Acceptance: reference display floor 1 under floor 2 | Works | render.rs:495 | - |

### 3D views and cameras (`docs/parity/3d-views-cameras.md`)

71 ids: 31 Works, 26 Partial, 14 Missing, 0 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| C-1 | 3D/Cameras | Row 1 3D controls | Partial | toolbar.rs 3D View/Full Camera/Orbit/Slider/Walkthrough/Lights flyouts | Material Painter, Material Eyedropper, Delete Surface, Adjust Material Definition (Material Builder) and Interactive Material Editor (the Materials list) are live (`tools/materials.rs`). |
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
| C-17 | 3D/Cameras | Cross Section/Elevation camera | Works | tools/camera.rs; `plan_elevation::section_free` (free-angle `FreeView`); dialogs/camera.rs `free_view` | The drawing follows the cut line at any angle: the scene is turned into the camera frame, the camera line is the cut plane, the drawing is cut off at the line's ends. Test: a 30 degree wall elevation's face region is the wall's length by its height. |
| C-18 | 3D/Cameras | Section poche vs elevation | Works | plan-elevation; manual 10.7 | - |
| C-19 | 3D/Cameras | Back-Clipped Cross Section | Works | tools/camera.rs; clip handle | - |
| C-20 | 3D/Cameras | Wall Elevation Camera | Works | manual 10.11; `wall_elevation` + `free_view` | Matches a wall at any angle exactly (the camera line is the wall's own); the back clip reaches through the wall. |
| C-21 | 3D/Cameras | Create Auto Elevations | Works | manual 10.11; tools/camera.rs | Also `CameraVariant::AutoInterior` (Auto Interior Elevations): click a room, four Wall Elevation cameras named "{room} North/East/South/West Wall", back-clipped to the room (`Project::auto_interior_elevations`). Toolbar/menu entry not wired yet (toolbar.rs and menus.rs belong to another owner); reachable through `ToolId::CameraVariant(AutoInterior)`. |
| C-22 | 3D/Cameras | Hidden-line vector output | Works | plan-elevation; Vector View panel | Curves are facets. Export as DXF (`Drawing::to_dxf`, lines on layers "{name}, Heavy/Medium/Light/Hidden/Hatch/Annotation") from the Vector View toolbar and the Camera Specification. |
| C-23 | 3D/Cameras | Cross Section Slider | Works | toolbar.rs cross_section_slider; view3d_panel `slider_camera` | The depth slider also shows in the Vector View, which redraws the cut live. |
| C-24 | 3D/Cameras | Camera symbol in plan, layer Cameras | Works | tools/camera.rs draw_camera_symbols, `callouts_on` | Section and elevation cameras carry a callout bubble (view number; the sheet reference under a dividing line once the camera is on a layout; shape, size and name in 3D View Defaults). The layout caption reads "1 - SOUTH ELEVATION". Integration queue item 1: confirm drawn in Select. |
| C-25 | 3D/Cameras | Full Camera handles | Partial | camera.rs CamHandle (move, aim, clip) | FOV cone-edge handles absent. |
| C-26 | 3D/Cameras | Drag moves camera, live 3D | Works | camera.rs | - |
| C-27 | 3D/Cameras | Aim handle with Shift 15 deg | Works | manual 10.3 | - |
| C-28 | 3D/Cameras | Section handles incl. clip | Works | manual 10.3 | - |
| C-29 | 3D/Cameras | Delete camera; copy/paste | Partial | Delete works | No copy/paste of cameras confirmed. |
| C-30 | 3D/Cameras | Camera Specification dialog | Works | dialogs/camera.rs | - |
| C-31 | 3D/Cameras | Camera View Options per view | Partial | dialogs/camera.rs Rendering tab, elevation options, `CameraObject.vector` | Per camera: hatch, shadows, depth weights, labels, level callouts, automatic dimensions, material labels, line weights from layers, dashed hidden lines, callout number. No Lighting set, backdrop, sky, fog, ground, layer set. |
| C-32 | 3D/Cameras | View options vs defaults | Partial | 3D View Defaults dialog | Defaults: eye height, angle, technique and the callout style (not saved with the plan). |
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
| C-45 | 3D/Cameras | Technique list | Partial | plan-materials nine techniques mapped to `plan_view3d::Look`; manual 10.4 | All nine draw in GL (Technical Illustration and Line Drawing edges, Watercolor wash and Duotone in the composite pass); they approximate Chief's looks and have no parameter dialogs. |
| C-46 | 3D/Cameras | Standard shaded view | Works | plan-view3d `gpu`/`pipeline`/`quality` | Textures, sun shadow map (PCF, 1024 to 4096 by quality), half-res SSAO, FXAA, sky gradient with ground fade, GGX and Fresnel shading with roughness and metalness from `plan_materials::scene_surface`, soft-shoulder tone curve, up to 8 nearest plan point lights. |
| C-47 | 3D/Cameras | Vector View | Works | plan-elevation; Vector View panel | Line weights per layer (walls heavy, trim lighter, from the layer pens; opt-in per camera), automatic elevation dimensions (floor-to-floor, opening sill/head strings, overall), optional level labels and material labels. |
| C-48 | 3D/Cameras | Technical Illustration | Works | manual 10.7 | - |
| C-49 | 3D/Cameras | Watercolor, Line Drawing, Duotone | Partial | composite-pass filters in `pipeline.rs` (wash, wobbling edge darkening, paper grain; flat lines; two-tone) | Screen-space filters, not Chief's bitmap generators; no parameter dialogs. |
| C-50 | 3D/Cameras | Glass House | Works | manual 10.4; `Look::GlassHouse` | Fresnel-weighted panes; every edge line drawn about 2 px wide over the glass, hidden ones included. |
| C-51 | 3D/Cameras | Physically Based ray trace | Works | plan-render; Ray Trace window | CPU progressive with a block preview, NEE for sun, point and area lights, Russian roulette, albedo/normal-guided denoiser, Preetham clear sky, depth of field, exposure, Save Image at 1x/2x/4x as PNG. No MIS, no path-traced caustics. |
| C-52 | 3D/Cameras | Clay | Works | manual 10.4; `Look::Clay` | Matte one-material fill with full-strength ambient occlusion and soft shadows in GL. |
| C-53 | 3D/Cameras | Technique never changes model | Works | view state only | - |
| C-54 | 3D/Cameras | Rebuild 3D | Works | menus.rs:547 | - |
| C-55 | 3D/Cameras | Delete Surface | Missing | toolbar.rs:1888 stub | - |
| C-56 | 3D/Cameras | Material Painter modes | Partial | `tools/materials.rs`: Paint, Eyedropper and Delete Surface modes; the 3D pick hook (`pick.rs::apply_pick`) paints the object under the click; Adjust Materials per part; Material Builder | The viewport shades with 23 scene materials, so a paint shows the closest one (`plan_materials::scene_material`), not the exact color. |
| C-57 | 3D/Cameras | Eyedroppers | Missing | stub | - |
| C-58 | 3D/Cameras | Adjust Material Definition | Missing | stub | - |
| C-59 | 3D/Cameras | Interactive Material Editor | Missing | stub | - |
| C-60 | 3D/Cameras | Painter overrides wall-type materials | Partial | walls take material from type layers | No per-surface override. |
| C-61 | 3D/Cameras | Material Builder, textures | Missing | menus.rs inert | plan-materials has data; GL view flat colors. |
| C-62 | 3D/Cameras | Default lighting and shadows option | Works | sun/key + fill + sky ambient; Shading menu in the 3D bar | Shadows are on by default (Chief's default is off). The key light follows Sun Angle. |
| C-63 | 3D/Cameras | Sun Angle | Partial | Sun Angle window (date, time, latitude, azimuth/altitude) | No longitude, DST, north direction; no Move Sun/Moon. |
| C-64 | 3D/Cameras | Add Lights, auto-place, Adjust Lights | Partial | Project.lights; Adjust Lights dialog | Point lights only; no auto-place or color temperature. |
| C-65 | 3D/Cameras | Light sets | Missing | manual 10.6: not built | - |
| C-66 | 3D/Cameras | Lights as selectable objects | Partial | lights drawn in plan | Selectable in Add Lights tool; layer name differs. |
| C-67 | 3D/Cameras | Shadows toggle in Standard | Partial | Shading menu: Shadows, Ambient occlusion, Quality, Exposure (`ViewSettings`) | Sun shadows only (no light shadows); not in Camera View Options > Display and not saved with the view. |
| C-68 | 3D/Cameras | 3D View Defaults dialog | Partial | dialogs/camera.rs | Eye height, angle, technique only. |
| C-69 | 3D/Cameras | Edge display and line weights | Partial | edge overlay; vector line weights from layer pens (`dialogs::camera::layer_weights`) | No weight setting for Standard edges. |
| C-70 | 3D/Cameras | Sky, ground, backdrop | Partial | sky gradient and ground fade behind Standard and Physically Based (`quality::sky_colors`) | Colours derive from the technique's background; no sky colour/image or ground option. Terrain renders as ground. |
| C-71 | 3D/Cameras | Walkthrough path and preview | Works | tools/camera.rs; Play/Record | PNG sequence, 640x480, no video. |

### Cabinets, stairs, framing, terrain, library, electrical (`docs/parity/cabinets-stairs-framing-terrain-library.md`)

68 ids: 42 Works, 23 Partial, 2 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| CB-1 | Cabinets | Cabinet tool inventory | Works | toolbar.rs Cabinet flyout (16 kinds); tools/cabinet.rs | - |
| CB-2 | Cabinets | Ghost placement, tool stays active | Works | tools/cabinet.rs; scenarios/s04_cabinets.rs | - |
| CB-3 | Cabinets | Auto-rotate flush to nearest wall | Works | tools/cabinet.rs (12 in rule) | - |
| CB-4 | Cabinets | Neighbor bumping/pushing | Partial | cabinet.rs bump to neighbor | No pushing; no Snap Settings control. |
| CB-5 | Cabinets | Fill between walls/cabinets | Works | Fillers fill the gap; a cabinet dragged into a gap within 2" of its width takes the gap (`plan_cabinets::fit_to_gap`, `tools/cabinet.rs::fit_gap`, Preferences > Architectural) | A new standard cabinet (Base, Wall, Full Height, the library types) clicked into a gap within the tolerance takes the gap too (`CabinetTool::placed_at`; Alt places it as it is); the tolerance is `tools::cabinet::set_fit_tolerance` (2" until Preferences sets it). |
| CB-6 | Cabinets | Default sizes | Works | plan-cabinets defaults; manual 6.1 | - |
| CB-7 | Cabinets | Cabinet Specification tabs | Partial | dialogs/cabinet.rs | Fill Style, Components, Object Info, Schedule disabled; Sides/Back not editable. |
| CB-8 | Cabinets | Resize, rotate handles, temp dims | Partial | Width from either end, depth from the front and the back, four corner handles (`Reshape(1..=6)` in `placed.rs`), rotate, a label handle (`HandleKind::Label`), and temporary dimensions while placing, dragging and selected (`tempdim::cabinet_temp_dims`: width, the gap to the nearest wall or cabinet on each side measured across openings, and the gap to the near jamb of an opening in the wall behind; click a value and type, Enter slides or resizes, in the Select and Cabinet tools) | Height by the dialog (Chief changes it in elevation views). |
| CB-9 | Cabinets | Drag keeps rotation until bump; Ctrl disables | Works | manual 6.2 | - |
| CB-10 | Cabinets | Face editing on all six faces | Works | dialogs/cabinet.rs Front/Sides/Back (Left, Right and Back faces: Plain, Finished Panel, Open, Custom Face) | Height and the top/bottom faces are not face-edited. |
| CB-11 | Cabinets | Split, equalize, locks | Works | plan-cabinets face solver | - |
| CB-12 | Cabinets | Door/drawer styles from Library | Partial | dialogs/cabinet.rs Door/Drawer | Built-in styles only; library styles planned. Hardware is complete: knob, bar pull, cup pull, edge pull (`HandleStyle`), pull length, drawer pull distance, bar pulls measured to their near end, tall doors hung at 38". |
| CB-13 | Cabinets | Auto labels B24, W3030 | Works | plan-cabinets auto_label / expand_label (`<L> <T> <W> <D> <H> <WxD> <WxH> <WxDxH> <N> <S> <F> <HW> <A>`); labels draw on the layer "Cabinets, Labels" (created with the first cabinet) and drag by their handle (`Cabinet::label_offset`) | - |
| CB-14 | Cabinets | Countertops auto-join | Works | Touching base cabinets join into one generated top that regenerates on a move, resize, add or delete (`placed::rejoin_countertops`, Preferences > Architectural); Generate Countertop (G) is the manual form | Corner treatment is still applied from the cabinets' tops. |
| CB-15 | Cabinets | Custom Countertop, Backsplash, Counter Hole | Works | tools/cabinet.rs | - |
| CB-16 | Cabinets | Appliances inserted into cabinets, cut-outs | Partial | Appliance Opening; sink/cooktop cutouts; `placed::snap_symbol_to_bay` snaps a dropped dishwasher, range, oven, microwave or refrigerator symbol into a matching bay (appliance openings, the tall oven and refrigerator cabinets' `Opening` bays) | The Library tool does not call the snap yet (integration queue); Replace From Library still drops cabinet inserts. |
| CB-17 | Cabinets | Soffit tool | Partial | Soffit kind | Cabinet-like box, not polyline soffit. |
| CB-18 | Cabinets | Shelf and Partition | Works | CabinetKind | - |
| CB-19 | Cabinets | Fillers | Works | tools/cabinet.rs | - |
| CB-20 | Cabinets | Cabinet defaults per kind | Partial | plan-core `CabinetDefaults` (Base, Wall, Full Height, Soffit, Shelf, Partition, Vanity, Pantry, Tall Oven, Refrigerator, Countertop, Backsplash, fillers and corners); `dialogs::cabinet::CabinetDefaultsDialog` with a page per kind; `tools::cabinet::default_cabinet` / `default_preset_cabinet` read them | The dialog has no menu item yet (integration queue). |
| CB-21 | Cabinets | Cabinet Schedule | Works | schedule_kinds.rs; C-01 labels; columns Width, Depth, Height, Elevation, Countertop, Door Style, Drawer Style, Finish, Hardware (the new ones off until chosen in Schedule Specification) | - |
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
| CB-36 | Framing | Build Framing dialog with per-group auto rebuild | Partial | Build Framing / Build All Framing commands; Framing Defaults page (`dialogs/framing.rs` FramingDefaultsDialog, `framing_view::settings`) | No Build Framing dialog with per-group switches; no auto-rebuild groups; the Defaults page has no menu item yet (integration queue). |
| CB-37 | Framing | Auto rebuild keeps manual members | Partial | rebuild replaces built; manual kept | No auto-rebuild on edits; no Retain option. |
| CB-38 | Framing | Wall framing and reference marker | Works | plan-framing wall.rs: header table, kings/trimmers/cripples/sills, double top plates, corner and tee backing, 48" blocking | Rollout options absent. |
| CB-39 | Framing | Floor framing, joist direction, rim | Works | plan-framing floor.rs: rim joists, stairwell headers and trimmers (`frame_floor_holes`) | Directed (Joist Direction) floors do not frame stairwell holes. |
| CB-40 | Framing | Wall detail views | Partial | Framing Overview plan view; `framing_view::elevation_scene` swaps the wall skins for studs; `plan_framing::wall_detail` | Needs the one-line call in dialogs/camera.rs and a menu item (integration queue); members are not dimensioned. |
| CB-41 | Framing | Framing schedule and Materials List | Works | Framing members counted in stock lengths with board feet, waste and unit prices; roofing lines; CSV, PDF page in the construction set, layout table box; Framing Schedule by member type, size and cut length (Linear ft, Board ft columns); Framing Takeoff window "By member type" | - |
| CB-42 | Framing | Structural calcs | Missing | none | Out of scope. |
| CB-43 | Terrain | Perimeter, data, Build Terrain | Works | tools/terrain.rs; plan-terrain `build_terrain_with_progress`; site_view `build_surface_with_progress` | Build Terrain reports its stages and ends with the triangle count and time; Terrain Specification has the auto-rebuild switch (off keeps the built surface, marked stale, until Build Terrain runs again), subdivision, smoothing, contour interval, major-every and label spacing. The progress is stage callbacks and a final summary, not a live bar (the build runs on the UI thread). |
| CB-44 | Terrain | Elevation data tools and Break | Works | TerrainVariant; `ElevationLine::{spline, reflatten}` | Elevation Point/Line/Spline: typed elevation on place, drag an existing point or line to move it (one undo step), spline tension 0 to 1 in the Elevation Line Specification (control points kept), handles on the spline's control points; contour elevation labels every N feet along the major contours, upright. |
| CB-45 | Terrain | Modifiers | Works | Hill, Valley, Raised, Lowered, Flat | - |
| CB-46 | Terrain | Features and Terrain Hole | Works | plan-terrain `grading.rs`, `landscape.rs`, `surface.rs`; plan-docs `terrain_report` | Features are cut/fill pads (flat top at the mean ground plus or minus a height, sides at a slope ratio, daylight line found by the build); terrain walls and curbs cut the TIN (gap with vertical faces, the right side lowered by the grade step, contours stop at the wall); kidney and spline outlines keep control points and are edited as smooth closed curves; cut/fill cubic yards per pad and in total (Terrain Specification, plan-docs table, Materials List Landscaping lines). |
| CB-47 | Terrain | Terrain hole around building, reference point | Partial | site_view::auto_building_hole; `auto_building_pad`; `TerrainVariant::BuildingPad` | The elevation reference is done: the building pad is levelled at the first floor less the terrain-to-first-floor distance. The Terrain menu has no Building Pad entry yet (menus.rs/toolbar.rs); the command is reachable as `SetTool(ToolId::TerrainVariant(BuildingPad))`. |
| CB-48 | Terrain | Roads, driveways, sidewalks | Works | tools/terrain.rs scape.rs; mesh.rs | Roads have a crown (centerline raised over the edges) and a curb height; curbs can be set on any strip. |
| CB-49 | Terrain | Garden beds, grass, water | Works | landscape.rs; landscape_plan.rs `ripple_lines` | Water draws a ripple fill (rows of waves clipped to the outline) with its depth in plan; the basin depth is in 3D. The 3D water surface is flat. |
| CB-50 | Terrain | Plants and sprinklers | Partial | plant runs; sprinkler runs | Plants are terrain-owned runs, not library symbols (they are in the Plant Schedule); sprinkler heads are not connected to a supply. |
| CB-51 | Terrain | Terrain Specification | Partial | dialogs/terrain.rs | Pages General, Contours, Building Pad (with the cut/fill table), Layer; Materials tab disabled. The Default Settings > Terrain page is not wired (defaults.rs). |
| CB-52 | Terrain | Terrain on Terrain layer, in 3D | Works | site_view terrain_feature_meshes; manual 9.7 | - |
| CB-53 | Library | Library Browser panel | Works | shell/library_browser.rs | - |
| CB-54 | Library | Search and filters | Works | plan-library browse.rs, shell/library_browser/user_ui.rs | Type, catalog, style/manufacturer, size range, favorites, category and sort. No filter of the objects already in the plan. |
| CB-55 | Library | Click to place symbol | Works | tools/library.rs | - |
| CB-56 | Library | Auto-rotate wall-mounted | Works | library.rs 6 in rule | - |
| CB-57 | Library | Replace From Library | Works | tools/library/user.rs replace_other | Cabinets swap for a saved cabinet item and keep cutouts, appliance, moldings and label; devices become the active library symbol. |
| CB-58 | Library | User Library, Add to Library | Works | tools/library/user.rs, make.rs; ~/.plan-studio/user-library.json | User Catalog: folders, rename, duplicate, move (menu or drag), delete, Object Information, Add to Library from a symbol, cabinet, CAD block, text or material (Library menu, Symbol Specification). Chief objects are never copied (licence). |
| CB-59 | Library | Import .calib/.calibz | Differs-by-design | DECISIONS.md #3: Chief catalogs read in place via plan-calib | Chief files are read in place, not imported. Library > Export/Import Library handle Plan Studio's own `.calibz` zip (JSON + .psm models), which Chief cannot read. |
| CB-60 | Library | Library items with 3D models | Partial | plan-calib decoded meshes; user models (OBJ, glTF) in tools/library/user.rs placed_meshes | Imported models carry a mesh, unit/up-axis options and a generated plan symbol, and preview in 3D. The 3D view does not yet call `user::placed_meshes` (view3d_panel.rs), built-in symbols are boxes, Symbol 3D tab disabled. |
| CB-61 | Library | List/thumbnail, favorites | Works | library_browser.rs result_row/result_cell | List and Grid toggle, Favorites and Recently Used lists (persisted in user-library-meta.json), preview pane with 2D/3D and rotate. |
| CB-62 | Electrical | Electrical tools | Works | tools/electrical.rs | - |
| CB-63 | Electrical | Outlet placement on wall, heights | Works | site_view.rs; manual 9.2 | - |
| CB-64 | Electrical | Auto Place Outlets rules | Works | plan-electrical auto place: 12 ft rule, 6 in clear of jambs, kitchen counter GFCI at 42 in and 4 ft, bath/laundry/garage GFCI; test on a 40x30 shell | Clearance from openings (2 ft or 1 ft) still to verify in Chief. |
| CB-65 | Electrical | Switch placement near door | Works | Auto Place Switches (tools/electrical.rs): switch 6 in past the latch jamb of each door, room light, connections, 3-way pair for two doors | - |
| CB-66 | Electrical | Lights | Works | Light, Recessed, Pendant, Wall, Rope, fans, detectors, thermostat, doorbell, jacks; 3D fixture meshes (`plan_electrical::electrical_meshes`); light fixtures feed the ray tracer (verified, `dialogs/camera.rs electrical_lights`) | 3D meshes need the call in view3d_panel.rs (integration queue). |
| CB-67 | Electrical | Electrical Connection arcs | Works | tools/electrical.rs: runs of lights from one switch, 3-way/4-way traveler pairs, midpoint bend handle | - |
| CB-68 | Electrical | Electrical Schedule | Works | schedule_kinds.rs: Mark, Type, Count, Label, Mount Height, Circuit; grouped rows sum Count | Circuit assignment and legend not in UI. |

### Documentation and layout (`docs/parity/documentation-layout.md`)

47 ids: 15 Works, 26 Partial, 5 Missing, 1 Differs-by-design.

| id | area | Chief behavior | Plan Studio status | evidence | gap note |
|---|---|---|---|---|---|
| L-1 | Layout/Docs | Send to Layout from active view | Works | shell/layout_window.rs; dialogs/layout.rs; hotkey S,L; Project Browser camera list (right-click > Send to Layout, `LayoutCommand::SendCamera`) | Sources: plan views, elevation/section cameras and perspective cameras; schedules and the Materials List are added as boxes from the layout toolbar. |
| L-2 | Layout/Docs | Send to Layout dialog fields | Partial | dialogs/layout.rs | One layout per plan; no layout-file choice or link toggle. |
| L-3 | Layout/Docs | Linked layout boxes refresh | Works | layout_window.rs live redraw; Update Layout Views | - |
| L-4 | Layout/Docs | Layout Box Specification | Partial | dialogs/layout.rs General/Source/Line Style; quarter-turn Rotation, rotate knob on the selected box, hit test and outline use the turned content | Quarter turns only; per-box layer/dimension display options absent. Perspective boxes carry their own DPI and sample count; text boxes a wrap / shrink-to-fit / as-typed fit. |
| L-5 | Layout/Docs | Vector vs raster boxes | Partial | Perspective camera boxes (`BoxSource::Perspective`): ray traced at the box size times its own DPI (default 80 dpi: a 6x4.5 in box is 480x360) and sample count, capped at 8 MP and 4096 px, cached per camera, model and request, rendered by Update Views on a thread with a progress bar; picture-file boxes (PNG and JPEG, baseline or progressive, through the shared `plan_library::image` decoder); Print Model and the Print dialog's perspective DPI override | Default sun and sky and no point lights in renders; no GL snapshot (`plan_view3d` has no offscreen target), so Print Image of the 3D view is ray traced from the viewport camera. |
| L-6 | Layout/Docs | Move, resize, copy, align boxes | Partial | layout_window.rs 8 handles, nudge | No align or copy between pages; no Open Source View. |
| L-7 | Layout/Docs | Pages with names and numbers | Partial | Layout Page Table, A-n numbering | No Page Specification dialog (border, page text). |
| L-8 | Layout/Docs | Sheet sizes list | Partial | Page Setup lists Arch, ANSI, ISO | Landscape only; no Customize Sheet Sizes. |
| L-9 | Layout/Docs | Title block macros | Partial | plan-layout macros; Project Information | %client.phone%, %company%, %custom.x% not expanded by layout. |
| L-10 | Layout/Docs | Layout templates | Partial | Page Template page; Daniel 18x24 block modeled; Create Construction Set adds Daniel's sheet set (Cover with the sheet index, Site, Floor Plans, Elevations, Sections, Details, Schedules, Materials, Framing) to the live layout in one undo step (`append_construction_set`) | No Save As Template for layouts; the set is built for the layout's own sheet and title block, not chosen from several templates. |
| L-11 | Layout/Docs | Sheet index | Works | Sheet index table box (`BoxSource::SheetIndex`) that follows page titles, on the cover of the construction set and from the layout toolbar; the old Page Setup option still prints the page-1 list | - |
| L-12 | Layout/Docs | Pen weights in paper units, scaling multiplier | Works | plan-layout render; Line weight scaling 0.1-5x | - |
| L-13 | Layout/Docs | Fill/pattern scaling to paper | Partial | plan-materials patterns | Elevation hatch fixed at 1/4 in scale. |
| L-14 | Layout/Docs | Plot/site plan layout | Partial | plan view box with terrain layers; the construction set has a Site Plan sheet (first floor plan at up to 1/8 in) | North Pointer and Scale Bar exist as CAD objects on the Site Plan layer (Terrain tool; the north angle feeds `site_view::plan_sun_azimuth`); the site sheet shows the building plan layer, not a surveyed site plan. |
| L-15 | Layout/Docs | Dimensions/text scale with box | Works | plan-layout render.rs draw_cad_item_styled, dimension_text_pt; tests printed_size_text_prints_the_same_size_at_any_box_scale | Printed-size styles print the same size on paper at every box scale; character-height ones scale with the box. |
| L-16 | Layout/Docs | Layout CAD and text on pages | Works | Layout view tools: Line, Box, Polyline, Circle, Arc, Text, Text Box, Leader (text with arrowhead), Revision Cloud (scalloped, tagged with a revision mark); select, move, resize by 8 handles and nudge any page drawing with one undo step each; text boxes wrap at the box width, clip overflow and can shrink to fit; layout layer set (Layout Box Borders, Layout CAD, Text, Title Block, Revision Clouds) with show/hide, line weight and colour in Layer Display Options | Page CAD has no dimensions, fillets or trim; a leader is one straight segment. |
| L-17 | Layout/Docs | Boxes keep link by id | Works | layout stored in plan | - |
| L-18 | Layout/Docs | Print dialog, scale, tiling | Works | File > Print dialog (dialogs/print.rs): PDF file, system printer picked from `lpstat -p` (default printer otherwise) through `lp -d`, or viewer; paper list and custom size, orientation, margin, fit / 100% / percentage / drawing scale / custom ratio, tiling with overlap and tile marks, range, copies, perspective DPI override; plan views and layouts; toolbar Print button; Print Model | No operating-system print panel (duplex, tray, color profile); the printer list is macOS and Linux CUPS only. |
| L-19 | Layout/Docs | Print Preview, Drawing Sheet, B/W modes | Partial | View Print Preview, Drawing Sheet (Alt+F2/F3); the Print dialog's Print Preview button sets the sheet to the chosen paper and scale; color, grayscale and black and white output | The preview is the sheet outline, not a picture of the chosen color mode and line weights. |
| L-20 | Layout/Docs | Multi-page PDF, bookmarks, fonts, raster DPI | Partial | Export Layout PDF; PdfDoc bookmarks per sheet, standard fonts with metrics and descriptors, print modes | The two standard fonts are referenced with their metrics, not embedded as font programs; perspective boxes are rendered at their DPI but pictures are only downscaled, never resampled up. |
| L-21 | Layout/Docs | Printed scale accuracy | Works | plan-docs pdf; scale caption | - |
| L-22 | Layout/Docs | Drawing scale lists | Works | architectural and metric scale lists; custom ratio 1:n in the Print dialog and `Scale::Ratio` | - |
| L-23 | Layout/Docs | Schedule set | Partial | Schedule flyout (10 kinds) | Wall is window-only; Note and Room Finish not kinds; no Manage Custom Schedules. |
| L-24 | Layout/Docs | Live schedules, floor filter, send to layout | Works | schedule_view.rs live table; Floors option; Schedule, Placed Schedule and Materials List layout boxes, refreshed by Update Views | - |
| L-25 | Layout/Docs | Column editor | Works | dialogs/schedule_spec.rs | - |
| L-26 | Layout/Docs | Callout markers linking plan to schedule | Partial | schedule_view.rs D01/W03/C-01/F-01 | Text labels not circle/hexagon markers; no renumber command. |
| L-27 | Layout/Docs | Edit from schedule | Missing | integration-queue item 4 open | No click-row-selects-object. |
| L-28 | Layout/Docs | Grouping and totals | Missing | integration-queue open | - |
| L-29 | Layout/Docs | Schedule tab data on objects | Missing | spec dialogs Schedule tab disabled | No manufacturer/model/cost fields. |
| L-30 | Layout/Docs | Room Finish Schedule | Partial | Room schedule Floor/Ceiling Finish columns (hidden) | No Wall Finish, Base, Number columns; not a separate kind. |
| L-31 | Layout/Docs | Custom schedules | Partial | Create Schedule with filter text | No object-type/field builder or management. |
| L-32 | Layout/Docs | Schedules export CSV/print | Partial | Export CSV | No Excel/PDF; tables absent from DXF and construction set PDF. |
| L-33 | Layout/Docs | Materials List by category | Works | Tools > Materials List window: the eleven categories (Foundation ... Landscaping), category filter, active floor or all floors | - |
| L-34 | Layout/Docs | Materials columns | Works | Category, ID, Description, Size, Count, Unit, Unit Price, Price; supplier in the Master List; total | - |
| L-35 | Layout/Docs | Quantities from component layers | Partial | formula take-off plus stored framing members and roof planes | Not driven by Components tabs. |
| L-36 | Layout/Docs | Waste and stock lengths | Works | Master List (`~/.plan-studio/master-list.json`): waste per category, stock lengths 8-16 ft, counts round up | - |
| L-37 | Layout/Docs | Materials List to layout, PDF, XLS | Partial | CSV export; Send to Layout table box; PDF sheet and construction-set page | No XLS (the CSV opens in a spreadsheet). |
| L-38 | Layout/Docs | Plan Footprint | Partial | Tools > Checks > Plan Footprint (CAD polyline + area) | Traces room boundary, not outer wall faces. |
| L-39 | Layout/Docs | Auto Detail | Missing | menus.rs inert | - |
| L-40 | Layout/Docs | CAD Detail From View | Partial | CadMode::DetailFromView (new floor) | Copies plan only, not section/elevation; floor not detail library. |
| L-41 | Layout/Docs | CAD Detail Management | Missing | menus.rs:637 inert | - |
| L-42 | Layout/Docs | CAD to Walls | Works | plan_import::cad_to_walls; dialogs/exchange.rs | - |
| L-43 | Layout/Docs | DXF import with units, layers | Works | dialogs/exchange.rs import window: units, scale, rotation, base and insertion point, per-layer map (keep, skip, plan layer, new name), Convert to walls; plan-import `ImportOptions` | Blocks are always exploded; no DWG. |
| L-44 | Layout/Docs | DXF/DWG export options | Partial | plan-core export/dxf.rs R12 ASCII; elevation DXF | No version, units, selection options; no DWG. |
| L-45 | Layout/Docs | Walls as polylines, dims, symbols | Partial | dxf.rs | Dimension/symbol options absent. |
| L-46 | Layout/Docs | PDF/image underlay with calibration | Partial | `plan_core::underlay`, `tools/underlay.rs` (+ `jpeg.rs`, `pdf.rs`), `dialogs/underlay.rs`: PNG, baseline JPEG and scanned-PDF (JPEG page) underlays on the Underlays layer; opacity, rotation, lock, two-point calibration, real-width entry | Vector PDF pages are not rasterized (print to PNG); progressive JPEG is refused. |
| L-47 | Layout/Docs | Other exports (SketchUp, IFC, image, animation) | Differs-by-design | glTF and ray-trace PNG, walkthrough PNG sequence exist | Out of scope per spec. |

