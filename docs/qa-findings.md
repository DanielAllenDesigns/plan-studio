# QA findings: Chief-parity scenario pass

Written by the scenario tests in `crates/plan-app/src/scenarios/` (run with
`cargo test -p plan-app scenarios`). Each scenario drives the real tools
through pointer and key events on a `PlanApp` without a window (the `Sim`
harness in `scenarios/mod.rs`), then checks the model, the undo stack, the
dialog requests, the 3D scene and the documents. A scenario that exposed a bug
is marked `#[ignore = "QA-nn"]` so the gate stays green; run
`cargo test -p plan-app scenarios -- --include-ignored` to see them fail.
When a bug is fixed, delete the `#[ignore]` line of its test. Round 8 fixed
QA-01, QA-02, QA-03, QA-05, QA-06 and QA-07 (their `#[ignore]` lines are gone);
QA-04 (Auto Stairwell hole) is fixed too: its `#[ignore]` is gone.

Round 7 changed nothing outside `scenarios/`, the one `mod scenarios;` line in
`main.rs` and this file.

### Round 8 fixes

* QA-01: `OpeningTool` derives `swing_flipped` from the pointer side of the wall
  and `hinge_at_end` from the nearer wall end (`plan_core::openings::door_defaults_for_pointer`);
  windows are untouched; the ghost follows the same rule.
* QA-02 (+R-23): `plan_3d::add_floor` builds floor and ceiling platforms per
  room, with each named room's `ceiling_height` and `floor_height_offset`;
  `project_hash` includes `room_names`.
* QA-03: the room schedule's area is the Interior Area and its ceiling column
  uses the room override. The row also carries a `standard_area` (centerline)
  cell; the column itself is not offered yet because the column list
  (`ROOM_FIELDS` in `plan-core/src/schedules.rs`) is outside this round's files.
* QA-05 / QA-06: `build_view_scene` meshes placed cabinets (`plan_cabinets::meshes`)
  and stairs, ramps and landings (`plan_stairs::tagged_meshes`); `project_hash`
  includes cabinets and stairs. Cabinet countertops map to Stone and handles to
  Metal; stair treads, landings and ramps map to Framing, risers, stringers and
  handrails to Trim, so `Floor` stays the room slabs.
* QA-07: each roof mode's `name()` is its toolbar entry name.

## Findings

| ID | Area | Parity id | Severity | Repro | Expected (Chief) | Observed | Test | Status |
|---|---|---|---|---|---|---|---|---|
| QA-01 | Doors: swing and hinge defaults | DW-8, DW-76 | Medium | Draw a 40x30 shell. Door tool. Click 3" inside the south wall near its start (60, 3), then 3" outside it near its end (420, -3). Read `swing_flipped` and `hinge_at_end` of both doors. | The swing side follows the side of the wall the pointer is on, and the hinge goes to the jamb nearer a wall end, so the two doors differ in both. | Both doors are `swing_flipped=false, hinge_at_end=false`: `place_from_template` copies the template, the click side and position are ignored. The user must Reverse Swing / flip the hinge on every door. | `s02_openings::door_swing_follows_the_pointer_side_and_hinge_the_nearer_end` | fixed in Round 8 |
| QA-02 | Room Specification: ceiling height reaches 3D | R-24, R-33 | Medium | Shell plus two partitions (3 rooms). Select tool, double-click the west room, set Ceiling Height 144" (`RoomName.ceiling_height = Some(144)`), OK. Build the 3D scene (`build_view_scene`) and look at the Ceiling-material meshes over that room. | The ceiling platform over that room is at 144" ("Room Structure edits propagate to the 3D platforms immediately"). | The ceiling tops out at 110.125" (the floor's 109.125" + slab): `plan_3d::add_floor` builds one ceiling slab from `floor.ceiling_height` and never reads `room_names`. `project_hash` does not include `room_names` either, so the 3D view would not even rebuild. Floor height offset (R-23) has the same gap. | `s03_interior::a_room_ceiling_height_override_moves_that_rooms_3d_ceiling` | fixed in Round 8 |
| QA-03 | Room schedule area | R-49, R-2 (manual 4.1: schedules use the Interior Area) | Medium | Shell plus a partition, Room Schedule (`build_tools::schedule_for(SchedKind::Room)`), compare "Area sq ft" with the room label / Room Specification. | The schedule shows the Interior Area (to the inside wall surfaces), the same number as the plan label. | Schedule row shows 601.7 sq ft (centerline area); the label shows 574.1 sq ft (interior). `plan_docs::room_schedule` uses `Room::area_sq_ft()`. The ceiling column also reads `floor.ceiling_height`, not the room's override (see QA-02). | `s10_documents::the_room_schedule_reports_the_interior_area_like_the_plan_label` | fixed in Round 8 |
| QA-04 | Stairs: Auto Stairwell hole | CB-29, CB-30 | High | 40x30 shell, Build New Floor (derive). Back on the 1st floor draw a stair (Draw Stairs, drag (100,100) to (250,100)). Run Auto Stairwell (Edit toolbar command). Build the 3D scene and look at Floor-material triangles of the 2nd floor over the stair footprint. | Chief cuts a stairwell hole in the upper floor platform where the stair passes through. | Auto Stairwell only adds invisible room-divider walls and a room named "Stairwell" on the floor above. 6 triangles of the 2nd-floor slab still cover the stairwell, so the 3D floor is closed over the stair. The foundation layer's platform holes (used by "Hole in Floor Platform") are not created. | `s05_stairs_floors::the_stairwell_cuts_a_hole_in_the_upper_floor_slab_in_3d` | fixed in Round 8 |
| QA-05 | 3D scene: cabinets | parity `3d-views-cameras.md` "3D scene contents" (documented gap), CB-6 | High | Finished house (walls, door, window, slab, roof). Base Cabinet tool, click (200, 10). Compare `build_view_scene` triangle count and `project_hash` before and after. | Placed cabinets are meshed (`plan_cabinets::meshes`) and the 3D view rebuilds. | 278 triangles before and after, same hash: `build_view_scene` never calls `plan_cabinets::meshes`, and `project_hash` ignores cabinets. | `s09_scene_3d::placed_cabinets_appear_in_the_3d_scene_and_change_the_hash` | fixed in Round 8 |
| QA-06 | 3D scene: stairs | parity `3d-views-cameras.md` "3D scene contents" (documented gap), CB-22 | High | Same house, Draw Stairs drag (100,100) to (250,100). Compare scene triangles and hash. | Stairs are meshed (`plan_stairs::model3d::meshes`) and the view rebuilds. | 278 triangles before and after, same hash: stairs are not in `build_view_scene` or `project_hash`. | `s09_scene_3d::stairs_appear_in_the_3d_scene_and_change_the_hash` | fixed in Round 8 |
| QA-07 | Roof tool name | architecture-tools.md (`Tool::name` is Chief's name) | Low | Activate each `ToolId::RoofVariant(mode)` and read `tools.active().name()`. | "Roof Plane", "Build Roof", "Auto Dormer", ... as every other multi-mode tool does ("Draw Line", "Auto Exterior Dimensions"). | All twelve modes answer "Roof". | `s06_roof::each_roof_mode_names_itself_like_its_toolbar_entry` | fixed in Round 8 |

QA-05 and QA-06 are already listed as gaps in `docs/parity/3d-views-cameras.md`;
they are recorded here with a test so the day a mesh builder is wired in the
test flips to green.

## Behaviors the scenarios confirmed (no finding)

* Walls: four click-drag walls with ends 1 to 2" off close into a 4-wall loop
  with shared endpoints, mitered outlines, one room, interior area within 1% of
  (W - t)(H - t); one undo step per wall; Esc keeps finished walls (W-3..W-8, R-2).
* Doors and windows: centered on the click at 1" snap, template sizes (30x96
  interior, exterior door, 32x72/24" sill window), overlap refused without an undo
  step, Door/Window Specification opens on double-click, a width change through
  the dialog is exactly one "Opening Specification" step, Cancel changes nothing.
* Known design deviation, not a bug here: a partition ending on a wall splits the
  through wall in two (`split_on_tee`, W-35 says Chief keeps it whole). The room
  count is right; wall counts are not asserted.
* Dimension points locate to main-layer faces (DIM-4), so a manual dimension
  between the south and north walls is 356.6" (clear span between finished
  faces is 353.4").
* Roof: Build Roof dialog builds nothing until OK, then 4 hip planes at 8:12 in one
  undo step; ridge = eave + span/2 x 8/12 to within 1"; holes, skylights, Auto
  Dormer (dialog then record), Explode Dormer and plane edits all undo.
* Hotkeys: `D, H` hinged door, `-` zoom in, `2/3/4` wall/door/window aliases,
  `1` select, Cmd+Z undo, Cmd+Y redo, Ctrl+Z / Ctrl+A floor down / up (macOS
  chords), 1.5 s sequence timeout and Esc, also through egui's key events.

## Test-access limits (not product bugs)

The Dimension, Text and CAD specification dialogs now expose a
`#[cfg(test)] draft_mut()` (reached through `SpecDialogs::{dimension,text,cad}_draft_mut`),
so `s07_dimensions_text_cad::editing_the_dimension_text_and_cad_dialogs_is_one_undo_step_each`
changes a value, presses OK and checks one undo step each. The Electrical,
Terrain and Roof dialogs are owned by their tools and are driven through the
real overlay frame (open, canvas blocked, OK applied on the next event).

## Tools that create nothing on a click-click

From `s11_every_tool` (every tool id reachable from the registry, the toolbars,
flyouts and menus, 213 ids): none panics, all have a name and a hint. 105 change
the plan on two clicks in the room or beside the south wall. These 108 do not;
most need a drag, a third click, a typed value, a selection, or an existing
object under the pointer, so this is a list to read, not a defect list:

* Select Objects, Pan Window, Library Symbol (needs a chosen item).
* Roof (all modes: Roof Plane, Build Roof (opens a dialog), Ceiling Plane, Hole,
  Skylight, Auto Dormer, Auto Floating Dormer, Explode Dormer, Roof Return, Edit).
* Dimensions needing 2 to 3 points or a pair of objects: Manual, End to End,
  Point to Point, Running, Angular, Centerline, Tape Measure.
* Text modes needing typing or a drag: Text, Rich Text, Leader Line, Text Line
  with Arrow, Callout, Note, Note Type Management, Text Macro Management.
* CAD edit tools needing a selected object: Make / Edit / Explode / Insert CAD
  Block, CAD Block Management, Fillet, Chamfer, Offset, Trim, Extend, Break Line,
  Reverse Direction, Make Parallel / Perpendicular, Convert to Polyline / Spline,
  Convert Polyline to Lines, Hatch, CAD Detail From View, Add Insertion Point,
  Add Arrow Backoff Point, Delete Temporary Points, Input Point, Input Arc.
* CAD drawing tools needing more than two clicks: Polyline, Draw Arc, Arc With
  Arrow, Ellipse, Box, Cross Box, Blocking Box, Insulation, Spline, Revision Cloud.
* Walls: all curved variants (a third click sets the arc).
* Foundation / details: Slab, Slab with Footing, Slab Hole (and with Footing),
  Hole in Floor / Ceiling Platform, Corner Boards, Quoins, Slab Footing,
  Molding Polyline, Polygon Shaped Deck, Floor / Wall Material Region, 3D Solid,
  Face, Pyramid, Truss Base.
* Cabinets: Custom Countertop, Custom Backsplash, Custom Counter Hole.
* Electrical: Electrical Connection (switch then light).
* Terrain: Perimeter, Elevation Point / Line / Region, Hill, Valley, Raised /
  Lowered / Flat Region, Terrain Hole, Road, Driveway, Sidewalk, Build Terrain
  (needs a perimeter first).

The scenarios 2 to 10 drive the ones that matter for "create it, then adjust it
in its dialog" with the right gesture (drag, third click, Enter, typed value).
`s11_every_tool::every_object_the_tools_make_opens_a_specification_dialog_that_closes_again`
places one of each object family and opens, draws and closes the dialog of every
object (wall, opening, cabinet, stair, roof plane, dimension, text, CAD, device,
terrain, room, slab, camera).
