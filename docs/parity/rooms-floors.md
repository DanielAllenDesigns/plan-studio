# Parity spec: Rooms and Floors (Chief Architect X18)

> Status (2026-10-08, re-audited against HEAD 4c11b69): 71 ids: 48 Works, 21 Partial, 2 Missing, 0 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.
>
> Since that count (Round 13): R-40 a Courtyard has no ceiling as well as no floor; layers and plan views (LAY-6, LAY-8): the Project Browser's jumps to a schedule or a CAD detail pan the plan there, the plan-view tab strip is a top panel of its own, the Active Layer by Tool table is read by the Text tools, the Angular Dimension and the wall tools (not yet by doors, windows, cabinets, devices, stairs, roofs, framing or Dimension objects, which take their layer from their kind), text styles can be renamed or removed with every layer, override, plan view, CAD text, dimension and schedule following (`Project::rename_text_style`; the Text Styles editor does not call it yet), and the 20 template plan views are checked by name against the template inventory (a tab switch is navigation, not an undo step: DECISIONS).

> Round 14 (rooms, floors and foundations): R-18, R-25, R-27, R-31, R-34, R-38, R-46, R-61, R-62 now Works and R-36, R-52, R-68 are further along (statuses and evidence in [../parity-status.md](../parity-status.md); decisions 300 to 311 in `DECISIONS.md`). Build Foundation builds Walls with Footings (footing under the walls, a basement or crawl space room on floor 0 with its slab and a ceiling taken from the first floor's platform), Monolithic Slab (slab thickness and stem wall height) and Grade Beams on Piers (piers and grade beams); a room has a Monolithic Slab Foundation flag, a Rough Ceiling and ceiling finish thickness that lift the ceiling platform in 3D, Moldings from a library (base, chair rail, crown; mitered, broken at doors), Materials for floor, ceiling and walls (per room, else the floor's Floor Defaults), a label text style and a label position; the Room Types list has Copy, Select All, Clear All and an In Use column; an Open Below room opens only the rooms wholly under it; the attic floor is built from Build New Floor ("Also build an attic floor") or kept current by Build Roof. Still open: the Room Specification's Deck tab, the platform-edge and underside materials, the "No Room Moldings" wall options, a permanent living-area readout (the status line shows it after a room is specified and after Plan Footprint) and a Build Roof checkbox that creates the attic floor.

Scope: automatic room detection, Room Specification, room types, room labels,
living area, floors (Build New Floor, Foundation, Reference Display, Exchange,
Floor Defaults, Attic). Walls, doors and windows are specified elsewhere
(`walls.md`, `doors-windows.md`); this file only states how they affect rooms.

Sources: Chief X18 Reference Manual (Rooms, Floors, Foundations chapters) as
remembered, plus `docs/chief-x18-dialogs.md` (Room Types / Room Specification
captured 2026-10-07). Lines marked "(verify in Chief)" are from memory and need
a check against the running application. Inches are the internal unit.

Code read: `crates/plan-core/src/rooms.rs`, `model.rs`, `layers.rs`,
`crates/plan-3d/src/{lib,slab}.rs`, `crates/plan-docs/src/schedule.rs`,
`crates/plan-app/src/main.rs` (floor switching, room drawing).

## 1. Room definition (auto-detection)

R-1. A room is any closed region bounded by walls. Chief detects rooms
continuously: drawing the last wall that closes a loop creates the room at once;
deleting or moving a wall re-detects the affected rooms immediately.

R-2. The room polygon follows the **interior surfaces** of the bounding walls
(the inside face of the finished wall layers), not the wall centerlines. All
areas, interior dimensions and the floor/ceiling platform footprints derive from
that inner-surface polygon.

R-3. Walls that count as room boundaries: Exterior, Interior, Half, Pony,
Foundation, Attic and Railing walls ... except walls with Options "No Room
Definition" (Wall Specification > General > Options; `chief-x18-dialogs.md`) and
railings that are not walls (verify in Chief for Railing-wall behavior: railings
drawn with the Railing tool do not close rooms; a wall switched to "Railing" in
its dialog does).

R-4. **Room Divider** walls (Straight Wall Tools > Room Divider) are invisible,
zero-height walls whose only job is to close a room. They draw as a thin dashed
line on layer "Walls, Invisible" (see layer table in `plan-core/layers.rs`),
never appear in 3D, elevations or the wall schedule, and have no framing.

R-5. A wall with the **Invisible** option stays a boundary for room detection
(unless "No Room Definition" is also set) but is not drawn in plan or 3D.

R-6. Rooms are detected per floor and independently of other floors. A wall
only closes a room on the floor it lives on; reference-displayed walls from other
floors never participate.

R-7. A room needs a closed loop of wall surfaces. A wall that dead-ends inside a
room (partial partition, peninsula) does not split it; the room polygon walks
around the dangling wall's outline so the area excludes the wall body.

R-8. Doorways, doors and windows do not break room boundaries: a room is closed
across a wall opening (the opening sits in a wall that continues).

R-9. A wall ending on another wall (T-junction) splits the larger region; a
crossing splits it into four. Overlapping, collinear or duplicated walls are
treated as one boundary, no sliver rooms are created.

R-10. Walls with zero thickness-equivalent gaps smaller than the wall snap
tolerance are treated as closed; a visible gap (a missing wall) leaves the
region unbounded and **no room** exists (no area label appears).

R-11. Nested rooms: a closed loop of walls wholly inside another room (a
closet pod, a free-standing chimney box) creates its own room, and the enclosing
room's polygon becomes a ring: area is outer minus the island, and the floor and
ceiling platforms have a hole under the island. Both rooms keep their labels.

R-12. Curved walls bound rooms like straight walls; the room polygon follows the
wall's facets (Facet Angle on the wall dialog).

R-13. Each detected room gets a default name from its room type; a brand-new
room is named "ROOM" and typed by the "Default Room Type" in the plan Defaults
(verify in Chief: default may be "Room" with Function Standard).

R-14. Room identity is stable across edits: renaming, changing type, finishes
and structure overrides stay attached to the room as walls move, as long as the
room still exists. Chief keys this to the room object, not to a coordinate.

R-15. Splitting a room with a new wall gives the original room identity to the
larger piece (verify in Chief) and a fresh default room to the other. Joining two
rooms by deleting the wall keeps the identity of the larger (verify in Chief).

R-16. Selecting a room: click inside it with Select Objects (the room selects on
click in empty floor area when "Select Room" is not blocked by other objects) or
click its label. The room shows its outline and handles on the inner polygon.

R-17. A room can be given a **manual boundary** only by walls; there is no
free-form room polygon (use Floor/Ceiling Platform holes or a Slab for odd
platforms).

R-18. Rooms on a Foundation floor (floor 0) are created the same way and carry
Function "Crawl Space"/"Basement" style types (see 3).

## 2. Room Specification dialog

R-19. Double-click a room (or Open Object) opens "Room Specification" with the
shared dialog frame: vertical tab list, centre panel, live preview with view
buttons. Tabs: General, Structure, Moldings, Layer, Fill Style, Materials,
Label, Components (plus Object Information in later builds; verify in Chief).

R-20. **General**: Room Name (free text, drives label and schedules), Room Type
(dropdown of the Room Types list), Function (drop-down, see 4), Living Area
radios (Include / Exclude / Use Default) and Conditioned Room radios
(Conditioned / Unconditioned / Use Default).

R-21. Changing Room Type does not reset a room whose fields were hand-edited:
only fields still set to "Use Default" follow the new type's defaults.

R-22. **Structure** tab: floor and ceiling heights, finish thicknesses,
platforms and rough-ceiling controls (R-23 to R-33).

R-23. *Floor Height*: absolute (relative to the floor's finished-floor datum) or
relative to the surrounding floor level; a room can sit lower (sunken living
room, garage) or higher than its neighbours. A negative relative value lowers
the room's floor platform.

R-24. *Ceiling Height*: measured from the room's own finished floor to the
finished ceiling, with an Absolute/Relative toggle. Default comes from Floor
Defaults > Ceiling Height. Changing the room's floor height does **not** change
its ceiling height when Absolute is on; it does when Relative is on (verify in
Chief for the exact toggle wording).

R-25. *Rough Ceiling*: a second ceiling height for the framed (rough) ceiling
used where the finished ceiling is lower, for example a dropped soffit, tray or
furred ceiling. Rough Ceiling height sets the bottom of the ceiling platform
structure; finish ceiling sits below it. When unchecked the rough ceiling equals
the finished ceiling minus finish thickness.

R-26. *Stem Wall*: when a room's floor is lowered below the wall base, "Stem
Wall" generates the stem/foundation wall under the room edge (the lowered garage
slab edge); height equals the drop. Applies on slab foundations (verify in
Chief for the precise enable conditions).

R-27. *Floor Finish thickness* and *Ceiling Finish thickness*: thickness of the
visible finish layers; they define the top of the floor platform and bottom of
the ceiling platform and feed the Components tree.

R-28. *Floor Structure* [Define...]: opens the Floor Structure dialog (layer
stack: finish, underlayment, subfloor, joist depth, ceiling below). Shared by
all rooms on the floor unless the room overrides it ("Use Default" wrench).

R-29. *Ceiling Structure* [Define...]: same for the ceiling platform (drywall,
joist or truss depth, insulation). Preview in the dialog shows the live
cross-section (CEILING / FLOOR dimension strings as captured in
`chief-x18-dialogs.md`).

R-30. Checkboxes "Floor Under This Room", "Ceiling Over This Room",
"Roof Over This Room": turn off the floor platform, ceiling platform or
automatic roof coverage for just this room (porch, open-to-below, deck).

R-31. *Monolithic Slab Foundation* flag on a Floor 1 room: the room's floor
becomes a thickened-edge slab and the foundation floor is not required under it.

R-32. *Flat Ceiling / Sloped*: a room's ceiling is Flat at the specified height
by default. A Sloped (vaulted) ceiling follows ceiling planes created by Build
Roof "Build Ceiling Planes" or the Ceiling Plane tool; the room's Ceiling Height
then means the lowest wall-top height (verify in Chief for wording).

R-33. Room Structure edits propagate to the 3D platforms immediately; the
Rebuild Walls/Floors/Ceilings command (F12) is needed only when Auto Rebuild
Floors/Ceilings is off in Preferences.

R-34. **Moldings**: base, chair-rail, crown, ceiling-shadow profiles per room
with the same Profiles table as cabinets (`chief-x18-dialogs.md`), plus wall
option "No Room Moldings Interior/Exterior" on walls.

R-35. **Fill Style**: pattern used to fill the room in plan view (hatching for
tile, wood), independent of the 3D floor material.

R-36. **Materials**: Floor Surface, Ceiling Surface, Floor Platform Edge, Ceiling
Platform Edge, Floor Underside, etc. Each is a Library material choice that
Material Painter can also change.

## 3. Room types and Function

R-37. Edit > Default Settings > Floors and Rooms > Room Types lists the built-in
types (Bedroom, Bonus Room, Closet, Courtyard, Crawl Space, Deck, Dinette,
Dining, Dining Room, Dressing Room, Entry, Family Room, Flat Roof, Foyer,
Garage, Kitchen, Living, Master Bath, Porch, Utility, ...; full list in the dialog
capture). "In Use" ticks the types the plan actually has.

R-38. Room Types buttons: Edit..., Copy..., Rename..., Delete, Select All, Clear
All. Edit opens Room Type Defaults (same tabs as the Room Specification minus
the per-room name).

R-39. The type list also feeds the Room dropdown, schedules and labels. A new
room created in the plan takes the default room type; changing the room's type
in its dialog is the usual way to name it.

R-40. **Function** values and their built-in behavior (verify the exact list
in Chief; names below are those documented plus the ones in the capture):
- Standard: ordinary room; floor, ceiling and roof coverage as normal.
- Living: heated living space counted in living area.
- Utility: counted as non-living by default (laundry, mechanical).
- Garage: floor platform dropped relative to the house floor (default
  roughly 4" lower; verify in Chief), slab floor, stem wall generated at the
  house edge (R-26), excluded from living area, unconditioned.
- Deck: no ceiling and no roof over the room; floor is a deck platform with
  decking/joist structure; excluded from living area; railing and deck-edge
  walls are expected around it.
- Porch: no ceiling by default (roof or ceiling plane can still cover it),
  exterior floor material, excluded from living area, unconditioned.
- Open Below: no floor under the room on the floor it sits on, and a matching
  hole is cut in the ceiling of the room below; railings are generated by the
  wall Platform Intersection options (Invisible Walls and Railings between
  platforms). Used for two-story foyers.
- Flat Roof: roof platform with no ceiling; floor is a membrane roof deck
  (Room type list includes "Flat Roof").

R-41. Function changes the **defaults** of the Structure checkboxes in R-30 but
every checkbox remains editable per room (R-21).

R-42. Living Area Include/Exclude overrides the Function default for one room
("Use Default" follows the Function: Included for Standard/Living, excluded for
Garage, Deck, Porch, Crawl Space, Flat Roof; verify in Chief for Utility).

R-43. Conditioned/Unconditioned feeds energy values and the room finish
schedule; defaults Conditioned.

## 4. Room labels

R-44. Each room carries a label placed at its centroid, draggable by a handle on
the label (label position is independent of the room geometry and is kept
relative to the room).

R-45. Label content comes from Room Specification > Label: Display in All Views
toggles "Interior Dimensions", "Interior Area", "Standard Area" and "Display in
Plan View" (captured). Default shows Room Name, then dimensions line
`W' x L'` (interior), then area when checked.

R-46. Text Style: "Use Layer Text Style" or Define... a custom style (font,
height, bold, colour). Labels live on layer "Room Labels".

R-47. Label text can be edited with text macros (CAD > Text > Text Macro
Management). Macros in X18 include the room's name, number, type,
interior dimensions, interior area, ceiling height and finishes; used in the
Room Finish Schedule and in custom label text. Exact macro tokens: (verify in
Chief).

R-48. Interior dimensions are measured between opposite interior surfaces of the
rectangle bounding the room (width x length), expressed in the plan's dimension
format (feet-inches with fractions per Dimension Defaults > Primary Format).

R-49. Interior Area is the polygon area of the interior-surface room polygon
(R-2). **Standard Area** measures to the outside of the surrounding walls and
the centre of shared walls (ANSI-style gross area; verify in Chief).

R-50. A room label can be deleted/hidden per room without deleting the room
(clear the Display checkboxes); a hidden label still exists in schedules.

## 5. Living area calculation

R-51. Total Living Area = sum of the Interior (or Standard, per Plan Defaults;
verify in Chief) area of every room whose Living Area setting resolves to
Included, across all floors. It updates live.

R-52. The total is displayed via Tools > Checks / plan information readouts and
in the area report; it appears on the Room Finish Schedule footer when added.
(verify in Chief for the exact menu path in X18.)

R-53. Rooms with Function Garage, Deck, Porch, Crawl Space and Flat Roof are
excluded by default, so an attached garage never inflates living area.

R-54. Area units follow the plan's unit setting (sq ft by default).

## 6. Floors

R-55. Chief floors are numbered: **0 = Foundation** (below first floor), 1 =
first floor, 2 = second floor ... and an optional **Attic** floor above the
top. Floor numbers are stable labels shown in the Floor toolbar (Down One Floor,
current number, Up One Floor).

R-56. Floor Defaults dialog (toolbar button next to the floor controls):
Floor Number, Description (e.g. "1st Floor"), Floor Structure thickness (floor
platform), Ceiling Height, Wall Heights, Default Ceiling Structure and the
Absolute/Relative height base. Each floor owns these. (field layout: verify in
Chief.)

R-57. Default values in an architectural template: floor structure 10 1/4",
ceiling height 108", wall framing height 109 1/8" to plate; the floor-to-floor
rise therefore equals ceiling height + floor platform thickness. (verify in Chief;
the numbers are template-dependent)

R-58. Changing a floor's Ceiling Height or Floor Structure thickness
re-heights walls whose top height is "Default Wall Top Height" and moves every
floor above upward by the delta.

R-59. **Build New Floor** (Build > Floor > Build New Floor, Shift+X): opens the
Build New Floor dialog. Options (verify in Chief for exact wording):
- Derive the new floor from: the current floor's exterior walls; the current
  floor's all walls; or an empty plan.
- Where to put it: above the current floor (default) or below.
- Checkbox: copy exterior walls with doors/windows; copy interior walls;
  copy room definitions.
New floors above get a default floor platform and the same default heights.

R-60. **Insert New Floor**: inserts an empty floor between existing floors and
renumbers everything above it. **Delete Current Floor**: removes the floor and
renumbers; confirmation required; Undo restores it.

R-61. **Build Foundation** (Cmd+F; the Build New Floor sibling): creates floor 0
with foundation walls under the exterior walls of floor 1 using the Foundation
dialog defaults. Foundation type options: Slab on grade (monolithic), Stem wall
slab/crawl space, Pier and beam, Basement (full height walls with footings)
(verify option names in Chief).
Stem wall and basement walls are Foundation-type walls with footings and sill
plates (Wall Specification > Foundation tab, `chief-x18-dialogs.md`).

R-62. Pier foundations place piers (Round Pier/Square Pad from the Slab tools) at
a spacing with a grade beam or girder; Monolithic slab foundation draws a slab
with thickened edge (Wall > Foundation > Slab: chamfer, pour number).

R-63. **Delete Foundation** removes floor 0 and resets floor 1 walls to bottom
at the floor platform.

R-64. **Exchange With Floor Above/Below**: swaps the contents (walls, rooms,
objects) of two adjacent floors including heights, and keeps both floors'
numbers. Typical use: swap garage/basement plans.

R-65. **Reference Display** (toolbar toggle and Tools > Floor/Reference
Display): shows another floor's objects in the current plan, dimmed and
unselectable but snappable. Default shows the floor below. Dialog columns: floor,
Display (full), Reference (dimmed), plus "Show All Floors Above/Below" shortcuts.

R-66. Reference floor objects respect layer visibility of the active view; they
are drawn in the reference colour set (Preferences > Reference Display Color).

R-67. Switching floors (Up/Down One Floor) keeps zoom/pan and active tool, and
selects nothing.

R-68. **Attic** floor: the Attic is auto-created by Build Roof when "Build
Attic" is requested (Roof dialog/Build New Floor > Attic; verify in Chief). It
holds Attic walls (kind "Attic Wall" in the wall dialog), has an automatic floor
platform on the ceiling platform of the floor below and the roof planes above.

R-69. Walls on the floor above that sit over walls below form stacked wall
platform intersections using the wall Platform Intersection options (Automatic /
Stop at / Balloon). A floor's wall tops intersect the next floor's floor
platform; roof planes and attic walls cut upward.

R-70. Floors have independent layer set views via Saved Views; "Floor Plan View"
saved views can pin a specific floor.

R-71. Floor elevation is the finished-floor height of floor n =
sum over floors below of (ceiling height + floor platform thickness); a room's
Floor Height (R-23) is an offset on top of that.

## 7. Plan Studio today (summary, details in gap table)

Rooms are detected from wall centerlines with interior, standard and centerline areas; walls carry Invisible, No Room Definition and Room Divider flags in the model (the first two are checkboxes in the Wall Specification) that the detection honors. A Room Specification dialog names rooms and sets their type, finishes, heights, living area, conditioned flag and label options; Default Settings > Room Types edits the types (name, function, living area, conditioned). Floors: Build New Floor, Insert Floor, Delete Floor, Exchange Floors, Build Foundation (slab, stem wall, pier, basement; hotkey `Cmd+F`) and Delete Foundation work, and the Space Planning Assistant and Plan Check are in. Reference Display draws the walls of the floor below in gray. Room and floor 3D slabs come from the room polygons. Missing: Floor Defaults, function-driven room behavior, nested-room holes, Floor Material Region, holes in floor and ceiling platforms, Attic floors as a tool. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## 8. Gap table

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| R-2 Room polygon = interior wall surfaces; areas from it | Centerline polygon; area overstated by half-wall thickness on every side | Critical | In `detect_rooms`, offset each face inward by the adjoining wall half-thickness (use `joins::wall_outlines` inner faces); keep centerline polygon only for graph building |
| R-3, R-4, R-5 Room Divider, Invisible, No Room Definition | None: `WallKind` {Exterior, Interior}; no flags | Critical | Add `Wall.flags` (invisible, no_room_definition) + `WallKind::RoomDivider`; filter in `detect_rooms`; draw dashed on "Walls, Invisible" |
| R-7 Dangling wall inside a room excluded from area | Dangling walls are ignored (graph prunes nothing); area includes wall body | Med | Trace face boundary around dangling edges (cut-out) when computing interior polygon |
| R-11 Nested rooms (island) with ring polygon | Island loop produces a separate room, but outer room keeps full area and no hole | High | Detect containment; store `holes: Vec<Vec<Point>>` on `Room`; subtract area; triangulate with holes |
| R-14, R-15 Stable room identity across edits | Room names keyed to anchor point; splits/joins lose properties silently | High | Add `Room.id`; match old and new faces by polygon overlap (max IoU) on re-detect; migrate properties |
| R-19..R-21 Room Specification dialog with per-room overrides | No dialog; only name+type string | Critical | New `RoomSpec` struct in plan-core + `plan-app/src/dialogs/room.rs` using the shared frame; "use default" inheritance from room type |
| R-23, R-24 Floor/ceiling height absolute/relative per room | Only `Floor.ceiling_height` | Critical | `RoomSpec.floor_height`, `ceiling_height` with `HeightBase::{Absolute,Relative}`; feed `plan_3d::slab` per room |
| R-25 Rough Ceiling | Missing | High | `rough_ceiling: Option<f64>`; slab builder uses it as ceiling platform bottom |
| R-26 Stem wall for dropped rooms | Missing | Med | Generate foundation-wall solid along room edge adjoining higher floor when drop > 0 |
| R-27..R-29 Finish thickness, Floor/Ceiling Structure Define | Constants `FLOOR_FINISH` 0.75" / `SLAB_THICKNESS` 1" | High | `Structure { layers: Vec<(Material, thickness)> }` shared floor/ceiling; Define... dialog |
| R-30 Floor/Ceiling/Roof Over This Room toggles | Missing; ceilings always built | High | Booleans on `RoomSpec`; skip meshes accordingly; roof builder uses `roof_over` |
| R-32 Flat vs sloped ceiling, ceiling planes | Flat only | High | Tie to `plan-roof` ceiling planes (see roofs.md RF-45) |
| R-34..R-36 Moldings, Fill Style, Materials per room | Missing | Med | Phase 2-3; start with Fill Style in plan draw, Materials with Material Painter data model |
| R-37..R-39 Room Types list (Edit/Copy/Rename/Delete) | Stub dialog entry | High | `RoomType` records (name, function, defaults) in `Project`; seed built-ins; list dialog |
| R-40..R-42 Function (Garage drop, Porch/Deck no ceiling, Open Below) | Missing | Critical | `enum RoomFunction`; apply defaults table (R-40/R-41) when room type chosen; Open Below cuts floor and ceiling holes in adjacent floors |
| R-44, R-45 Draggable label with interior dims/area/standard area toggles | Fixed centroid label "Room N / area" | High | `RoomLabel { offset, show_dims, show_area, show_standard }`; draw interior dims from bounding rectangle |
| R-47 Text macros for labels | None | Med | Macro registry keyed by name; expand in label and schedule rendering |
| R-49 Standard Area | Missing | Low | Compute from outside-face polygon; second readout |
| R-51..R-54 Living area total with per-room include/exclude | Missing | High | `living_area_total(project)` honoring Function/override; show in status/plan info |
| R-55 Floor numbering with 0 = foundation, Attic | Index 0 = "1st Floor" | High | Add `Floor.number` and `FloorKind {Foundation,Normal,Attic}`; keep names separate; migration for old JSON |
| R-56..R-58 Floor Defaults dialog; heights cascade | Constants and `Floor.ceiling_height` only | High | `FloorDefaults` on `Floor` with structure thickness; recompute elevations of upper floors on change |
| R-59, R-60 Build New Floor / Insert / Delete | Missing | Critical | Commands on `Project` + dialog; derive options copy walls/openings with id remap |
| R-61..R-63 Build Foundation (slab, stem wall, pier, basement), Delete Foundation | Missing | High | Foundation builder generating floor-0 foundation walls from floor-1 exterior outline; footing params from wall dialog |
| R-64 Exchange with floor above/below | Missing | Low | Swap `Floor` structs, fix elevations |
| R-65, R-66 Reference Display with dialog, dimmed snappable | Toolbar toggle is stub | High | Draw other floors' walls/rooms in dimmed colour; add to snap candidates; dialog of floors |
| R-68 Attic floor and attic walls | Missing | Med | Tie to Build Roof (roofs.md RF-31) |
| R-69 Platform intersections between floors | Walls end at their own height; no platforms | High | Floor/ceiling platform model in `plan-3d`, driven by wall Structure options |
| R-71 Elevation cascade from floor heights | `Floor.elevation` is stored, not derived | Med | Derive at load/edit from structure + ceiling heights; keep stored as cache |

<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| R-72 | Floor Material Region: Polygon on the floor with its own material and fill. | Works | tools/details.rs DetailsVariant::FloorMaterialRegion; toolbar.rs floor() |
| R-73 | Hole in Floor Platform: Cut a hole in a floor platform (stairwell, open to below). | Works | tools/foundation.rs FoundationVariant::FloorHole |
| R-74 | Hole in Ceiling Platform: Cut a hole in a ceiling platform (attic stair, chase). | Works | tools/foundation.rs FoundationVariant::CeilingHole |
| R-75 | Slab: Slab polyline with thickness, fill and a footing option (no parity spec: manual chapter 16 only). | Works | tools/foundation.rs FoundationVariant::Slab; docs/manual/16-foundation-slabs.md |
| R-76 | Slab with Footing: Slab with a perimeter footing. | Works | FoundationVariant::SlabFooting; dialogs/foundation.rs |
| R-77 | Slab Hole: Hole cut in a slab, optionally with its own footing. Also covers: Slab Hole with Footing. | Works | FoundationVariant::SlabHole, SlabHoleFooting |
| R-78 | Square Pad: Footing pad and pier placed under posts and beams. Also covers: Round Pier. | Works | FoundationVariant::SquarePad, RoundPier |
| R-79 | Space Planning ▸ Space Planning Assistant…: Questionnaire to colored room boxes, arrange, then build the house plan. Also covers: Space Planning Assistant. | Works | plan-spaceplan crate (questionnaire, room boxes, bump, validate, build_house); menus.rs tools_menu |
| R-80 | Space Planning ▸ Room Planner / Space Planning Configuration toolbar (not captured): Space Planning toolbar configuration; Room Planner to import a room-planner sketch. (Not captured; verify in Chief.) | Partial | room boxes exist; the Space Planning toolbar configuration toggle is a stub (toolbar.rs config_space_planning) |
| R-81 | Slab flyout: Slab and pier tools flyout. | Works | toolbar.rs slab() |
| R-82 | Foundation defaults: Footing size, slab thickness, stem wall height defaults used by Build Foundation. | Missing | not in the tree; Build Foundation dialog (dialogs/foundation.rs) takes the values per build |
| R-83 | Slab defaults: Slab thickness, footing and fill defaults. | Missing | not in the tree |
| R-84 | Wall Covering tab (room wall coverings): Wall coverings applied to every wall of a room (wainscot, tile). (Not captured; verify in Chief.) | Partial | dialogs/room.rs "Wall Covering" tab live; walls cannot yet carry their own coverings (see Wall dialog) |
| R-85 | Slab / Footing / Pad / Pier specification: Slab specification (General, Fill Style, Line Style, Layer). (Not captured; verify in Chief.) | Works | dialogs/foundation.rs SLAB_TABS, HOLE_TABS, PAD_TABS |
| R-86 | Split-level floors (floors at different heights on one level): Two or more floor platforms on one story at different elevations joined by stairs. (Not captured; verify in Chief.) | Partial (verify in Chief) | rooms carry their own Floor Height (R-23); `plan_core::split_level::level_steps` finds where two rooms meet at different heights, the plan marks the step (`fireplace_view::deck::draw`), 3D closes the raised platform with a riser where no solid wall stands (`plan_3d::split_level`), Add Steps at Level Changes builds a stair per step (`deck::add_steps`); tests `split_level.rs`, scenarios s44 `rooms_at_different_heights_get_a_marked_level_change_and_a_riser`, `add_steps_puts_a_stair_between_the_levels_in_one_undo_step`. Gap: walls that step with the floor (short walls) are not built |
| R-87 | Room Planner import: Import a room-planner layout and build walls from its room outlines. (Not captured; verify in Chief.) | Missing | no Room Planner or Space Planning import beyond the Assistant |
| R-88 | Stepped footings (footing steps down a sloping lot): Footings that step in height along a foundation wall on a sloping lot, with the step shown in plan and 3D. (Not captured; verify in Chief.) | Missing | no footing steps in plan-core rooms.rs / dialogs/foundation.rs (grep stepped footing finds nothing) |
<!-- coverage-audit:end -->
