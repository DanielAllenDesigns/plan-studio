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
  cell; the column is `standard_area` in `ROOM_FIELDS` (`plan-core/src/schedules.rs`), hidden by default.
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
| QA-08 | Auto Exterior Dimensions on a hand-drawn shell | DIM-24, DIM-26 | Medium | `draw_shell` 40x30 (four click-drag walls; the closing west wall ends 2" off plumb), a window on each side, Auto Exterior Dimensions, click inside. Look at the strings left of the west wall. | Three strings (openings, wall to wall, overall) on every side, all outside the walls, nearest string first. | The west side gets its set twice: three strings outside at x = -36 / -72 and a second full set of three inside the house at x = +37 / +72 (36" and 72" inside the west wall). Without windows the west wall's wall-to-wall string is missing outside and lands inside. The other three sides are right; on a plumb west wall (`exact_house`, `auto_exterior_puts_three_strings_...`) all 12 strings are right. | `s17_dimensions::auto_exterior_on_a_hand_drawn_shell_keeps_every_string_outside_the_house` | fixed |
| QA-09 | File menu rows do nothing | S-1, files.rs (File > Close / Revert to Saved / Save a Copy / Backup Entire Plan / Open Recent > Clear Menu / Manage Auto Archives) | High | Save a plan (`Action::FileSave`), then `Action::Custom(files::CLEAR_RECENT)`; or `files::REVERT`, `files::CLOSE`. | Clear Menu empties the Open Recent list, Revert reloads the saved plan (after the prompt), Close empties the window. | Nothing happens. `PlanApp::apply` sends every `Action::Custom` to `EditorContext::run_custom`, which does not know the `file.*` ids; `PlanApp::file_command` (and `save_a_copy`) have no caller (rustc: "methods `file_command` and `save_a_copy` are never used"). Looks like the `files::is_command` branch was lost when `main.rs` was rebuilt after it was overwritten with the Action History file at 09:05 on 2026-10-08. Open Recent rows (`recent.N`) still work, they go through `app_info`. | `s22_files::the_file_menu_rows_clear_recent_close_and_revert_reach_the_files_module` | fixed |
| QA-10 | Archive rotation can delete the newest copy | files.rs archive_previous / rotate_archives | Low | Archive limit 3, save five or six times inside one second (stamps are per second). | The newest copy stays, the oldest go. | Names are `maple-<stamp>`, `-2`, `-3` ...; once the unsuffixed first copy has been rotated away, the next save of that second reuses the plain name, which sorts lowest and is rotated away at once. After six quick saves the newest archive is the version from three saves ago. Needs 4+ saves in one second, so it only shows with scripted or very fast saving. | `s22_files::rapid_saves_over_the_archive_limit_never_rotate_away_the_newest_copy` | fixed |
| QA-11 | Billboard picture colored like a roof vanishes in the Doll House | C-43, Create Billboard Image | Low | Place a billboard whose average color is [100, 120, 90] (it maps to the Roof material) inside the shell; Doll House view; click it. | Pictures stay visible and pickable in every view. | `image_mesh` gives the picture the nearest opaque scene material; the Doll House and overview modes hide every mesh of the Ceiling and Roof material (`pick::is_drawn`, the viewport), so a green-ish tree billboard disappears with the roof and cannot be clicked. A brick-colored one works. | `s24_view3d_picking_textures::a_green_billboard_is_not_hidden_with_the_roofs_in_the_doll_house` | fixed |

QA-05 and QA-06 are already listed as gaps in `docs/parity/3d-views-cameras.md`;
they are recorded here with a test so the day a mesh builder is wired in the
test flips to green.

## Round 12: scenarios s15 to s24

Ten more scenario files (95 tests, 4 of them `#[ignore]`d for QA-08 to QA-11)
cover the Edit menu, typed wall input and wall edits, dimensions, door and
window 3D parts, rooms and floors, roofs in 3D, the layout and print path,
files, cabinets/underlays/preferences and the 3D view. `s21_layout_print::isolate_home`
points `$HOME` at a temp folder once per test process, so nothing reads or
writes `~/.plan-studio`; the Open Recent list is in memory under `cfg(test)`.
The s01 to s14 scenarios all still pass unchanged (no expectation needed
updating).

Notes that are by design, not findings: a partition ending on an exterior wall
splits it, so Build New Floor "exterior only" copies one wall more per tee;
the Arch tab leaves the hole in the wall square and adds a spandrel to the
opening's own mesh; unnamed rooms share one floor slab, per-room platforms
start once a room is named; the CAD Line tool drags the handle of the selected
line when a new line starts on that handle; walls drawn after Floor Defaults
keep their own tool height (W-6).

## Behaviors the scenarios confirmed (no finding)

* Round 12: cursor-attached Paste lands the copy's center on the click, Esc
  cancels with no undo step, Paste Hold Position and Duplicate (12", -12") are
  one step each, Select All skips locked and hidden layers, Transform/Replicate
  3 copies step by the offset, a radial copy turns 90 degrees each, Reflect
  About Object mirrors the swing and exterior side, Point to Point, Center
  Object (room and opening), Align/Distribute, Group, Delete Objects and the
  Action History all undo in one step.
* Round 12: typed 12' at 90 degrees, Shift (15 degrees) and Alt (raw point),
  Break Wall (both halves connected, refused inside a window), Remove Break,
  Reverse Layers (plan and 3D), Change Line/Arc with the bulge handle and the
  Arc section round trip, Snap Settings toggles and Edit Behaviors Resize.
* Round 12: Locate walls surfaces < main layer < centers, strings that follow a
  wall and a window, 12 Auto Exterior strings on a plumb shell, Auto Interior,
  Auto NKBA, printed-size text at 1/4" and 1/8".
* Round 12: every door and window flavor builds its 3D parts, the new opening
  is selected, labels go size then mark, Sash/Arch/Shutters change the 3D
  triangles, a door and its sidelite mull.
* Round 12: garage drops 24" with a slab and stem walls, Open Below removes the
  floor and the ceiling under it, Floor Defaults cascade, Reference Display
  snaps, a nested closet, the dragged room label and the Floor Structure
  Define thickness reach 3D.
* Round 12: gable walls reach the ridge, the baseline at the plate closes the
  gap, eave cut changes the fascia geometry, Roof Cuts Wall at Bottom and Auto
  Attic Walls switch from the Roof Defaults.
* Round 12: New Layout, Send to Layout (plan and camera), text box, page CAD
  moves, layout layers, Page Setup, Print with tiling (page counts), Materials
  List with the Master List, construction set (10 pages with a sheet index).
* Round 12: atomic save, archive rotation, autosave only while dirty, startup
  and open-time recovery, Open Recent, the unsaved prompt (Save / Don't Save /
  Cancel) and the dirty dot in the title.
* Round 12: cabinet depth and corner handles, fit to gap, auto-joined
  countertops, underlay calibration, DXF layer map, Preferences round trip,
  Material Painter pick hook, 3D pick/open/delete, terrain pick, Textures toggle
  (store prefetch and viewport flag), billboards facing the eye.


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

## Round 14: robustness sweep (s40_roundtrips, plan-core/tests/roundtrip.rs)

Tests only; no feature code was changed. `crates/plan-core/tests/roundtrip.rs`
(19 tests, 4 `#[ignore]`d) builds a plan with one of every slot by hand and
checks save, load, save is byte-identical, old and odd files, and numbers that
do not survive JSON. `crates/plan-app/src/scenarios/s40_roundtrips.rs` builds
the same kind of plan with the real tools (every tool id gets the gestures of
`CANDS` on one house, plus direct inserts for the kinds no gesture reaches,
`required_kinds` lists what must be there), then checks Save and Open through
the application, one undo step per tool gesture and per Edit command
(do, undo, redo, undo gives back the same plan), legacy files (old hidden-layer
records, the sample plans and their earlier versions in git), and 2000 seeded
random gestures. Run the gate with `cargo test -p plan-core --test roundtrip`
and `cargo test -p plan-app s40_`; QA-20 to QA-29 are fixed and their tests run in the normal gate (no `#[ignore]` is left).

| ID | Area | Parity id | Severity | Repro | Expected | Observed | Test | Status |
|---|---|---|---|---|---|---|---|---|
| QA-20 | Forward compatibility: unknown keys are dropped on the next save | S-1 (files) | Medium | Take a saved plan, add `"from_the_future": {...}` at the top level, `"future_slot"` on a floor and `"future_wall_field"` on a wall, `Project::from_json`, `to_json`. | Opening a newer build's plan and saving it keeps the newer build's data (at least at the top level and in floors), or Open warns that it would lose data. | Loading succeeds (no `deny_unknown_fields`), but nothing keeps the extra keys: the saved JSON has none of them. Saving a plan from a newer Plan Studio silently strips whatever this build does not know. | `plan-core/tests/roundtrip.rs::unknown_extra_keys_are_kept_when_the_plan_is_saved_again` | fixed (round 15) |
| QA-21 | `{}` is not a loadable plan | S-1 | Low | `Project::from_json("{}")`. | An empty plan with one floor (every other slot is `#[serde(default)]`). | Error: `name`, `floors` and `next_id` have no serde default, so an empty or braces-only file is refused. Also relevant to hand-written plans and to generated test files. | `plan-core/tests/roundtrip.rs::an_empty_json_object_loads_as_an_empty_plan` | fixed (round 15) |
| QA-22 | A stale `next_id` hands out ids that are already in use | model.rs `Project::alloc_id` | High | Load a plan whose `next_id` is 1 (an old file, or a hand edit), then `alloc_id()` repeatedly. | New ids start above every id in the plan (Project::from_json clamps `next_id` to max id + 1). | `alloc_id` returns ids of existing walls; two objects then share an id and selection, undo and dimensions attach to the wrong one. | `plan-core/tests/roundtrip.rs::a_stale_next_id_does_not_hand_out_an_id_that_is_in_use` | fixed (round 15) |
| QA-23 | One NaN or infinity makes the saved plan unloadable | model.rs `to_json` / `io::save_project` | High | `add_wall` with a NaN or infinite coordinate (a zero-length division in a tool), `to_json`, `from_json` on the result. | Saving refuses (Err) or writes a plan that loads again. | `serde_json` writes non-finite floats as `null`; `null` is not an `f64` on the way back in, so the whole file is unreadable, and the atomic save has already replaced the good file with it. | `plan-core/tests/roundtrip.rs::a_non_finite_number_never_makes_the_saved_plan_unreadable` | fixed (round 15) |
| QA-24 | Delete and Reverse Swing on a mixed selection push several undo steps | edit_commands.rs `edit.delete`, `edit.reverse_swing` | High | House with a door, a base cabinet and a stair; Select All (walls, door, cabinet, stair), Edit > Delete. Or select a door and a cabinet and Reverse Swing. | One user action is one undo step (architecture-tools.md), so one Ctrl+Z restores everything. | Each object family pushes its own undo step: Delete of walls, a door, a cabinet and a stair is 3 undo steps, Reverse Swing of a door and a cabinet is 2, so Ctrl+Z brings back a half-restored plan in between. | `s40_roundtrips::deleting_a_mixed_selection_is_one_undo_step`, `::reversing_the_swing_of_a_door_and_a_cabinet_is_one_undo_step` | fixed (round 15) |
| QA-25 | Rebuild All changes the plan without an undo step | Action::RebuildAll | Medium | Shell plus a second floor pushed directly into the project (a stale plan), Rebuild All, compare digest and undo depth. | A command that changes the plan is one undo step. | The plan changes (a stale floor elevation is recomputed, e.g. floors[1].elevation 240.0 becomes 119.375) and the undo stack does not grow; Ctrl+Z then undoes the action before it. The 2000-gesture fuzz hits it too (gesture 95, menu RebuildAll). | `s40_roundtrips::rebuild_all_leaves_an_undo_step_when_it_changes_the_plan` | fixed (round 15) |
| QA-26 | Commands that change nothing still leave an undo step | edit_commands.rs `edit.unlock`, `edit.lock` | Low | Select an unlocked wall, Unlock Selection. Select a terrain elevation point, Lock Selection. | A command with nothing to do leaves no undo step (an empty step makes Ctrl+Z look broken); Lock on an object that cannot be locked says so. | Unlock on an unlocked wall pushes a step with an identical plan. Lock on a terrain elevation point changes nothing and pushes a step (the sweep counts 29 such steps in the fuzz run, QA-26 and QA-27 together). | `s40_roundtrips::a_command_that_changes_nothing_leaves_no_undo_step`, `::locking_a_terrain_object_locks_it_or_says_it_cannot` | fixed (round 15) |
| QA-27 | Undo after a foundation pad does not give back the plan | tools/foundation.rs | Low | Small house, Foundation > Square Pad, click once, Ctrl+Z, compare the saved JSON. | The plan is byte-equal to the one before the click. | Only `next_id` differs after the undo (`/next_id: 5 became 6`, the id the pad took is not given back). Harmless by itself, but it makes do, undo, redo, undo checks and "no unsaved changes" disagree. | `s40_roundtrips::undo_after_a_foundation_tool_gives_back_exactly_the_plan` | fixed (round 15) |
| QA-28 | One unreadable electrical device takes the readable ones with it | tools/electrical.rs, plan-electrical layer load | High | Two outlets and a switch, then push a device `{"kind":"FutureDevice"}` into `floor.electrical.devices` (a plan from a newer build), reopen, place one more device. | The unknown device is kept as-is; the others stay. | After the next electrical edit a readable device is gone (outlets with ids 1 and 2 plus a foreign device: after one more tool click only id 1 is left). The layer is read as a whole and a record that does not parse takes its neighbours with it. | `s40_roundtrips::one_unreadable_electrical_device_does_not_take_the_readable_ones_with_it` | fixed (round 15) |
| QA-29 | Roofs drop a record this build cannot read; cabinets refuse silently | roof_view.rs `load`/`store`, placed.rs `edit_cabinets` (the untyped `Vec<Value>` slots) | Medium | Push a record this build cannot parse into `floor.roofs` (`{"kind":"future","id":880003}`), reopen, place a skylight on a roof plane. Same with `floor.cabinets` (`{"id":880001,"kind":"FutureKind"}`) and a base cabinet click. | The unreadable record is written back untouched next to the edited ones; if an edit has to be refused the user is told. | Roofs: `load` skips records with an unknown `kind` and `store` rewrites the slot from the parsed records, so the foreign record is deleted by the first roof edit. Cabinets: `edit_cabinets` refuses to edit the family while any record is unreadable (data safe), but the click does nothing and says nothing. Stairs and framing keep the foreign record (checked). | `s40_roundtrips::records_this_build_cannot_read_survive_an_edit_of_their_family` (roofs; the cabinet refusal is printed by `the_foreign_record_checks_work`) | fixed (round 15) |

Notes from the sweep, no finding: the sample plans under `samples/` and every
earlier version of them in git load, migrate once (a second load changes
nothing) and save byte-stable; the oldest wall-only file loads with every
newer slot defaulted; a truncated or damaged file is an error with a message
and the open plan stays; `save_project` leaves the previous file untouched
when the target cannot be written. Every tool gesture and every Edit command
is exactly one undo step and do, undo, redo, undo
returns the same plan; 2000 seeded random gestures never panicked and every
undo unwind returned to the start plan.

## Round 14: tool and dialog sweep (s35_tool_dialog_sweep)

`crates/plan-app/src/scenarios/s35_tool_dialog_sweep.rs` activates every tool id the toolbars, flyouts and menus can pick (360 ids after round 16: construction lines, tray ceilings, roof baseline polylines, moldings and the 3D solids included), performs its headless gesture, and checks one object, one undo step, Open Object with Chief's title and tabs, an edit through the dialog, and Delete. The full table, the missing-tabs table and the "OK changed nothing" list are in `docs/tool-dialog-sweep.md` (regenerate with `S35_WRITE_DOC=1 cargo test -p plan-app s35`). The rows below are what the sweep found that is a bug rather than a missing tab; the sweep lists them in `KNOWN_GAPS` so the gate stays green, and a new defect outside that list fails it. Delete the `KNOWN_GAPS` entry when a row is fixed.

| ID | Area | Parity id | Severity | Repro | Expected | Observed | Test | Status |
|---|---|---|---|---|---|---|---|---|
| QA-12 | Bay Window Specification panics in debug builds | OP-bay (dialogs/bay_window.rs:46, dialogs.rs `Fields::degrees`) | High | Window flyout > Bay Window, click on a wall, select it, Open Object. | The dialog opens. | `Fields::degrees` has `debug_assert!(key.starts_with("deg_"))`; the Bay Angle row passes the key `bay_angle`, so the first paint panics (`assertion failed: key.starts_with("deg_")`). A release build skips the assert and parses the angle with the Length parser. Rename the key to `deg_bay_angle`. | `s35_tool_dialog_sweep::every_creating_tool_makes_an_object_with_a_chief_dialog_that_edits_and_deletes` (KNOWN_GAPS) | open |
| QA-13 | Rope Light tool makes no device | tools/electrical.rs `ElecVariant::RopeLight` | Medium | Electrical flyout > Rope Light, click once inside a room. | A rope light device, or the start of a polyline that Enter finishes. | The single click makes nothing and the status line gives no hint. If the tool is a polyline tool the sweep needs its gesture (two clicks and Enter); if a click should place one, it does not. | same | open |
| QA-14 | Open Object on a fireplace opens no dialog | CB-501 to CB-503 (manual pp. 759 to 760) | Medium | Fireplace, Fireplace in Wall, Prefab Fireplace or Chimney tool, place one, Select, Open Object (or double-click). | Fireplace Specification (Layer, Materials, Components). | Open Object offers nothing for the Symbol. The extra Edit toolbar button `Fireplace Specification` (`editor/fireplace_view.rs` `edit_actions`, command `OPEN_SPEC`) does open it, so only the Open Object / double-click routing is missing. | same | open |
| QA-15 | Five framing tools make objects that cannot be opened | tools/framing.rs `ReferenceMarker`, `JoistDirection`, `BearingLine`, `RoofTrussDirection`, `TrussBase` | Medium | Framing flyouts: place a Reference Marker, Joist Direction, Bearing Line, Roof Truss Direction or Truss Base, Select, Open Object. | A specification dialog (Chief has General, Line Style and Layer for these). | `no specification dialog opens for Framing Object`; the member and layout-line tools (Joist, Rafter, ...) do open Framing Member Specification. | same | open |
| QA-16 | CAD Detail From View object cannot be opened | tools/cad.rs `CadMode::DetailFromView` | Low | CAD > Detail From View, click, OK, Select the result, Open Object. | A CAD detail specification (Chief opens the CAD Detail dialog). | `no specification dialog opens for CAD Object`. | same | open |
| QA-17 | Thirty tools whose dialog edit the sweep cannot verify | Details 3D solids, Wall Material Region, Landing, Dormers, Slab Holes, Terrain elevation, region, grass, stone, retaining wall and sprinkler tools | Medium | Make the object, Open Object, Tab to any of the first 48 fields, type 7, Enter. | The plan changes in one undo step. | The dialog opens with the right title and tabs but the plan is byte-identical after OK for every Tab position tried. Either the dialog drops the edit or its fields are not reachable by Tab (a table, a canvas, a combo first). The dialogs expose no draft accessor like the wall, opening, dimension, text and CAD ones, so the sweep cannot tell which. Roof Plane and the 110V Outlet, on the round 14 list, now take edits. | same | open |
| QA-18 | Construction Line, Tray Ceiling and Roof Baseline polylines open the generic CAD dialog | CAD-62 (manual pp. 83 to 91), RF baseline, tray ceilings | Medium | Tools > Construction Line (or Tray Ceiling Polyline, Roof Baseline Polyline), draw one, Select, Open Object. | The object's own dialog: Construction Line, Tray Ceiling or Roof Baseline Specification. | Open Object opens `CAD Line Specification` or `CAD Polyline Specification` (General, Line Style, Arrow, Layer). The dedicated dialogs open only from the extra Edit toolbar buttons of the same name. | `s35_tool_dialog_sweep::construction_tray_and_baseline_polylines_have_their_own_specification_dialog` proves the dedicated dialogs open from the Edit toolbar | open (Open Object routing) |
