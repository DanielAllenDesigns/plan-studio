# Parity spec: Rooms and Floors (Chief Architect X18)

> Status (2026-10-08, re-audited against HEAD 4c11b69): 71 ids: 48 Works, 21 Partial, 2 Missing, 0 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.
>
> Since that count (Round 13): R-40 a Courtyard has no ceiling as well as no floor; layers and plan views (LAY-6, LAY-8): the Project Browser's jumps to a schedule or a CAD detail pan the plan there, the plan-view tab strip is a top panel of its own, the Active Layer by Tool table is read by the Text tools, the Angular Dimension and the wall tools (not yet by doors, windows, cabinets, devices, stairs, roofs, framing or Dimension objects, which take their layer from their kind), text styles can be renamed or removed with every layer, override, plan view, CAD text, dimension and schedule following (`Project::rename_text_style`; the Text Styles editor does not call it yet), and the 20 template plan views are checked by name against the template inventory (a tab switch is navigation, not an undo step: DECISIONS).

> Round 14 (rooms, floors and foundations): R-18, R-25, R-27, R-31, R-34, R-38, R-46, R-61, R-62 now Works and R-36, R-52, R-68 are further along (statuses and evidence in [../parity-status.md](../parity-status.md); decisions 300 to 311 in `DECISIONS.md`). Build Foundation builds Walls with Footings (footing under the walls, a basement or crawl space room on floor 0 with its slab and a ceiling taken from the first floor's platform), Monolithic Slab (slab thickness and stem wall height) and Grade Beams on Piers (piers and grade beams); a room has a Monolithic Slab Foundation flag, a Rough Ceiling and ceiling finish thickness that lift the ceiling platform in 3D, Moldings from a library (base, chair rail, crown; mitered, broken at doors), Materials for floor, ceiling and walls (per room, else the floor's Floor Defaults), a label text style and a label position; the Room Types list has Copy, Select All, Clear All and an In Use column; an Open Below room opens only the rooms wholly under it; the attic floor is built from Build New Floor ("Also build an attic floor") or kept current by Build Roof. Still open: the Room Specification's Deck tab, the platform-edge and underside materials, the "No Room Moldings" wall options, a permanent living-area readout (the status line shows it after a room is specified and after Plan Footprint) and a Build Roof checkbox that creates the attic floor.
> Round 16 (brief 17, floors, foundation and fireplace foundation): R-18 and R-61..R-63 (Build Foundation rebuilds in place, basement tiers, garage and slab rooms, stepped walls, piers, fireplace blocks; Delete Foundation refuses under Auto Rebuild), R-55 and R-59 (Move Roof Up and Step Elevations, 30 floors, Insert New Floor below, Floor Defaults opens after a new floor) and R-68 (the Attic floor has no rooms and warns) are Works; R-83, R-85, R-116..R-121 and R-125..R-142 below carry the new status and evidence (decisions FF1 to FF16 in `DECISIONS.md`).

> Round 16 brief 15 (room functions, Exterior Room, Living Area, room commands): R-101, R-104, R-105, R-108, R-109 and R-146 now Works; R-99, R-102, R-103, R-106, R-107, R-110, R-115 and R-145 are further along (evidence on their rows below; decisions RM1 to RM11 in `DECISIONS.md`). Also touched, with evidence in the same code: R-15 and R-16 (the Exterior Room joins the click and Tab selection), R-19 and R-21 (Function and Room Information in the General panel, type changes hand on the type's spec), R-30 (Roof Over, Flat Ceiling and Build Foundation Below follow the function), R-36 to R-38 (Room Types list rows, Room Type Defaults), R-40 (functions), R-42 to R-52 (Living Area per structure, Standard Area and macros `%dimensions%`, `%internal_area%`, `%standard_area%`), R-56 (the Exterior Room's grips set the level defaults).

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
| R-25 Rough Ceiling | Missing (Round 16: with a layered Ceiling Finish the rough ceiling is still the platform's underside, the gap above the finish is solid and a dropped ceiling's plenum sits under it, `slab::room_levels`, test `a_rough_ceiling_keeps_a_solid_filler_between_a_layered_finish_and_the_platform`) | High | `rough_ceiling: Option<f64>`; slab builder uses it as ceiling platform bottom |
| R-26 Stem wall for dropped rooms | Missing | Med | Generate foundation-wall solid along room edge adjoining higher floor when drop > 0 |
| R-27..R-29 Finish thickness, Floor/Ceiling Structure Define | Constants `FLOOR_FINISH` 0.75" / `SLAB_THICKNESS` 1" (Round 16: a tray ceiling carries its own ceiling layers, `TrayCeiling.structure`, and its dropped or raised ceiling slab is that thick in 3D) (Round 16: Works. Floor Structure, Floor Finish, Ceiling Structure and Ceiling Finish are layered `Assembly` definitions with Role, Fill, framing options and energy values in the Material Layers Definition dialog, `crates/plan-core/src/assemblies.rs`, `dialogs/assembly_def.rs`; older single thicknesses read as one-layer definitions, DECISIONS LA1, LA5; tests in `assemblies::tests`, s84) | High | `Structure { layers: Vec<(Material, thickness)> }` shared floor/ceiling; Define... dialog |
| R-30 Floor/Ceiling/Roof Over This Room toggles | Missing; ceilings always built | High | Booleans on `RoomSpec`; skip meshes accordingly; roof builder uses `roof_over` |
| R-32 Flat vs sloped ceiling, ceiling planes | Flat only; Round 16 adds the cathedral switch (R-146), shelf ceilings (`plan_roof::Shelf`, `cathedral_ceiling_planes`), `ceiling_height_at`, `room_ceiling_height_at` and `cathedral_height_at` sampling | High | Tie to `plan-roof` ceiling planes (see roofs.md RF-45) |
| R-34..R-36 Moldings, Fill Style, Materials per room | Missing | Med | Phase 2-3; start with Fill Style in plan draw, Materials with Material Painter data model |
| R-37..R-39 Room Types list (Edit/Copy/Rename/Delete) | Stub dialog entry | High | `RoomType` records (name, function, defaults) in `Project`; seed built-ins; list dialog |
| R-40..R-42 Function (Garage drop, Porch/Deck no ceiling, Open Below) | Missing | Critical | `enum RoomFunction`; apply defaults table (R-40/R-41) when room type chosen; Open Below cuts floor and ceiling holes in adjacent floors |
| R-44, R-45 Draggable label with interior dims/area/standard area toggles | Fixed centroid label "Room N / area" | High | `RoomLabel { offset, show_dims, show_area, show_standard }`; draw interior dims from bounding rectangle |
| R-47 Text macros for labels | None | Med | Macro registry keyed by name; expand in label and schedule rendering |
| R-49 Standard Area | Missing | Low | Compute from outside-face polygon; second readout |
| R-51..R-54 Living area total with per-room include/exclude | Missing | High | `living_area_total(project)` honoring Function/override; show in status/plan info |
| R-55 Floor numbering with 0 = foundation, Attic | Index 0 = "1st Floor" | High | Add `Floor.number` and `FloorKind {Foundation,Normal,Attic}`; keep names separate; migration for old JSON |
| R-56..R-58 Floor Defaults dialog; heights cascade | Constants and `Floor.ceiling_height` only (Round 16: Floor Defaults has the four platform rows with Edit and Use Default, `dialogs/floor_defaults.rs`; tests `a_layered_floor_structure_sets_the_floor_height`, `use_default_follows_the_plan_wide_definition_and_off_keeps_a_copy`, s84) | High | `FloorDefaults` on `Floor` with structure thickness; recompute elevations of upper floors on change |
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
| R-83 | Slab defaults: Slab thickness, footing and fill defaults. | Partial | Foundation Defaults dialog (dialogs/foundation.rs FoundationForm: Slab Thickness, Slab at top of Stem Wall, chamfer width and height, DECISIONS FF8); the Slab and Slab with Footing Defaults dialogs of the Slab tools: still missing; verify in Chief. |
| R-84 | Wall Covering tab (room wall coverings): Wall coverings applied to every wall of a room (wainscot, tile). (Not captured; verify in Chief.) | Partial | dialogs/room.rs "Wall Covering" tab live; walls cannot yet carry their own coverings (see Wall dialog) |
| R-85 | Slab / Footing / Pad / Pier specification: Slab specification (General, Fill Style, Line Style, Layer). (Not captured; verify in Chief.) | Works | dialogs/foundation.rs: Slab Specification with Hole in Slab, Elevation Reference, Top/Bottom, Has Footing, Footing Offset; Pier/Pad Specification with Type, Top Height, Bottom Height, Width (DECISIONS FF9, FF16); tests dialogs/foundation.rs, scenarios/s92_floors_foundation.rs the_depth..; verify in Chief. |
| R-86 | Split-level floors (floors at different heights on one level): Two or more floor platforms on one story at different elevations joined by stairs. (Not captured; verify in Chief.) | Partial (verify in Chief) | rooms carry their own Floor Height (R-23); `plan_core::split_level::level_steps` finds where two rooms meet at different heights, the plan marks the step (`fireplace_view::deck::draw`), 3D closes the raised platform with a riser where no solid wall stands (`plan_3d::split_level`), Add Steps at Level Changes builds a stair per step (`deck::add_steps`); tests `split_level.rs`, scenarios s44 `rooms_at_different_heights_get_a_marked_level_change_and_a_riser`, `add_steps_puts_a_stair_between_the_levels_in_one_undo_step`. Gap: walls that step with the floor (short walls) are not built |
| R-87 | Room Planner import: Import a room-planner layout and build walls from its room outlines. (Not captured; verify in Chief.) | Missing | no Room Planner or Space Planning import beyond the Assistant |
| R-88 | Stepped footings (footing steps down a sloping lot): Footings that step in height along a foundation wall on a sloping lot, with the step shown in plan and 3D. (Not captured; verify in Chief.) | Missing | no footing steps in plan-core rooms.rs / dialogs/foundation.rs (grep stepped footing finds nothing) |
<!-- coverage-audit:end -->

## Manual audit additions (part 1)

Rows added by the Chief X18 Reference Manual audit, part 1 (pages 11 to 318; `docs/chief-manual-coverage/part1-program-files-defaults-editing.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the running program.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| R-89 | Elevation references (manual p. 21): Per-object height reference: Absolute, From Floor, From Finished Floor, From Terrain, From Ceiling, From Roof. | Works | plan-core `elevation_ref.rs` (`ElevationRef` base + To Top/To Bottom edge, `Resolver`, `resolve`, tests for a split-level floor and sloped terrain), `PropTable::pages[key].elevation`; widget `dialogs/elevation_ref.rs` (`row`, one line in the opening, cabinet, symbol and electrical dialogs); applied by `elevation_ref::effective_project` in the 3D scene build; s64 `a_cabinet_measured_from_the_ceiling_follows_the_ceiling_height`. Slabs and soffit dialog not yet wired (integration-queue). Verify in Chief. |
| R-90 | Elevation references (manual p. 21): Default drawing group of a ceiling-hung or roof-hung symbol is 29 Soffits, otherwise 31 Fixtures/Furniture. | Partial | LAY-36 drawing groups (Round 15, `drawing_groups.rs`); the reference is now stored per symbol (`props.pages[symbol:id].elevation`, Round 16 brief 05) but the drawing-group default that reads it is not wired (integration-queue); verify in Chief. |
| R-91 | Space Planning (manual p. 80): Room Box tools, one per room type (bedroom, kitchen and so on): click to drop a default-size box or drag any size; repeats until another tool. | Missing | R-79 covers the Assistant only; tools: no spec yet; manual audit part 1; verify in Chief. |
| R-92 | Space Planning (manual p. 80): Boxes live on the "Space Planning Boxes" layer, labelled with that layer's text style; show the wall extents that Build House will use. | Partial | `rooms_edit.rs` draws boxes and labels (plan_symbols); no layer, no wall-extent outline: no spec yet; manual audit part 1; verify in Chief. |
| R-93 | Space Planning (manual p. 80): Room Boxes are 2D polylines: move, resize, reshape like closed polylines; no curved edges; overlap by creation order, overlapped edges hidden. | Partial | boxes drag and bump (R-79); polyline handles and overlap order: no spec yet; manual audit part 1; verify in Chief. |
| R-94 | Space Planning (manual p. 80): Room Boxes saved with the plan (they are plan objects). | Missing | boxes are session state in `rooms_edit.rs` `State`, not in the Project: no spec yet (data loss risk); manual audit part 1; verify in Chief. |
| R-95 | Space Planning (manual p. 81): Edit tools Remove Overlapped Areas and Overlap Adjacent Room Box. | Missing | no spec yet; manual audit part 1; verify in Chief. |
| R-96 | Space Planning (manual p. 81): Space planning on floor 1 and up (not floor 0); align upper boxes using the Reference Display, snaps to Endpoint and On Object. | Partial | Reference Display works (LAY-9); upper-floor boxes: no spec yet; manual audit part 1; verify in Chief. |
| R-97 | Room Box Spec (manual p. 82): Room Box Specification dialog: General (room name, function), Line Style, Fill Style. | Missing | no spec yet; manual audit part 1; verify in Chief. |

## Manual audit additions (part 2)

Rows added by the Chief X18 Reference Manual audit, part 2 (pages 319 to 520; `docs/chief-manual-coverage/part2-cad-walls-rooms-dimensions.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the program.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| R-98 | Rooms split by an invisible wall break at the wall's centerline or lower/higher edge (manual p. 443): Two rooms separated by an invisible wall are measured from its centerline and materials break there; with different floor heights the break is at the edge facing the lower floor, with different ceiling heights at the edge facing the higher ceiling; a new room from subdividing inherits its parent's non-default settings. | Partial | rooms.rs detect_rooms uses invisible walls (R-4, R-5); the height-dependent break edge is not implemented; the split-off room keeps the parent's name only via RoomName anchor (R-15); manual audit part 2; verify in Chief. |
| R-99 | Room Type Defaults dialog panels (manual p. 445): Each Room Type carries its own name, function, living and conditioned inclusion, ceiling and floor structure and finish, deck framing and supports, layer and drawing group, fill style, moldings and components; several types can be edited together. | Partial | Round 16 brief 15: `RoomTypeDef::spec` (`plan-core rooms.rs RoomTypeSpec`: floor and ceiling Structure and Finish, Deck Specification, layer and drawing group, fill, moldings, label) handed to rooms by `apply_type_spec`; `dialogs/room_types.rs` Room Type Defaults (General, Structure, Deck, Moldings, Layer, Fill Style, Label) and Multiple Room Type Defaults, opened by the Room Types list (`default_lists.rs`); tests `room_types::tests::*`, `rooms.rs a_room_type_spec_overrides_the_room_it_is_given_to`; DECISIONS RM4, RM5. Open: the Components panel; verify in Chief. |
| R-100 | Room Label Defaults dialog (Label and Dimension Format panels) (manual p. 445): Room Label Defaults sets text style, size, color, border and the dimension number format used in the interior-dimension line of labels; changing it does not alter existing labels. | Partial | default_pages/plan.rs room_label page (name, area, dimensions, ceiling, text style, border, alignment) is stored only (no bound field); per-room Label tab works (R-44..R-50); manual audit part 2; verify in Chief. |
| R-101 | Room Function set (Interior, Exterior, Hybrid) and what each function does (manual p. 446): Functions group room types: interior (most), exterior (Balcony, Court, Deck) and hybrid (Attic, Garage, Open Below, Porch, Slab). They set living and conditioned defaults, ceiling and roof generation, floor and foundation, door and window facing, electrical and Plan Check behaviour; stairwells and crawl spaces use Open Below. | Works | Round 16 brief 15: `plan_core::rooms` `ROOM_FUNCTIONS`, `FunctionClass`, `function_defaults_with`, `electrical_rules`, `plan_check_rules` (Interior Standard/Utility, Exterior Balcony/Court/Deck, Hybrid Attic/Garage/Open Below/Porch/Slab; Basement and Crawl Space are not functions; Slab floor = Foundation Defaults slab thickness); tests `the_three_categories_hold_the_manuals_functions`, `living_and_conditioned_defaults_follow_the_function`, `ceilings_and_roofs_follow_the_function`, `floors_and_foundations_follow_the_function`, `electrical_rules_follow_the_function_and_the_type`, s74 `court_and_balcony_rooms_take_the_exterior_properties`, `an_open_below_stairwell_has_no_floor_and_is_conditioned_but_not_living`; DECISIONS RM1 to RM3. Auto Place Outlets and Plan Check read the rules through hooks queued in docs/integration-queue.md; verify in Chief. |
| R-102 | Room function drives door and window exterior/interior behaviour (manual p. 447): A window between an exterior-type and an interior-type room faces outward; a hinged or sliding door between them uses the exterior door defaults and shows a threshold; doors between interior rooms are interior. | Partial | Round 16 brief 15: `rooms::opening_faces_outside` and `faces_outside` carry the rule (exterior beside interior faces out; Open Below and other hybrids are interior), test `a_door_or_window_between_exterior_and_interior_rooms_faces_outside`; the opening tools still choose by wall kind (DECISIONS 42, 43, RM3); manual audit part 2; verify in Chief. |
| R-103 | Layer panel in the Room Specification (layer and Drawing Group) (manual p. 447): A room can be put on a custom layer (default layer Rooms) and a drawing group; Object Layer Properties shows its primary and secondary layers. | Partial | Round 16 brief 15: Layer tab in `dialogs/room.rs` (layer and Drawing Group stored in `RoomName::options`, handed on by Room Type Defaults); plan drawing still puts every room on the Rooms layer (DECISIONS RM11); s74 `the_structure_panel_options_are_kept_with_the_room`. |
| R-104 | Calculate Materials in Room / From Selection (manual p. 448): Calculate Materials for Room lists a room's contents (not walls); Calculate Materials From Selection lists finish materials of the selected rooms: moldings, wall, floor and ceiling finish, subflooring. | Works | Round 16 brief 15: the selected room's Edit toolbar offers Calculate Materials in Room (`dialogs/materials_list.rs edit_buttons`, `rooms_edit::edit_actions`); s74 `calculate_materials_in_room_and_create_schedule_from_room`. |
| R-105 | Create Schedule from Room (manual p. 448): An edit tool that builds a schedule of one object type located in the selected room. | Works | Round 16 brief 15: the Edit toolbar's Create Schedule from Room asks the type (`schedule_spec::ask_schedule_type`) and `schedule_view::create_from_room` places a schedule limited to the room (`Schedule::rooms`); `rooms_edit::create_schedule_from_room`; s74 `calculate_materials_in_room_and_create_schedule_from_room`. |
| R-106 | Exterior Room object and Exterior Room Specification (manual pp. 449-450): Each structure on each floor has an Exterior Room selected by clicking just outside an exterior wall (Select Next Object): its label reports the living area, its edit handles set the level's default floor and ceiling heights, and its specification dialog controls exterior wall coverings and materials. | Partial | Round 16 brief 15: `plan_core::living` (`structures`, `ExteriorRoom`, `Floor::exterior_rooms`), `editor/rooms_edit/exterior.rs` (click outside an exterior wall or on the Living Area label, Tab after the objects under the pointer, band and edge grips that set the default ceiling height and floor platform, spec applied to the exterior walls in one undo step), `dialogs/exterior_room.rs` (General, Wall Covering, Materials); s74 `a_click_outside_an_exterior_wall_selects_the_exterior_room`, `tab_reaches_the_exterior_room_after_the_wall_you_clicked`, `the_exterior_room_specification_covers_the_exterior_walls_in_one_step`, `dragging_the_exterior_rooms_top_grip_sets_the_default_ceiling_height`; DECISIONS RM8. Open: selecting it in 3D views (no 3D room handles exist); verify in Chief. |
| R-107 | Room Edit toolbar buttons (manual p. 452): A selected room offers Calculate Materials for Room, Turn Ceiling Off/On, Add to Style Palette, Build Framing for Selected, Make Room Polyline, Make Standard Area Polyline, Make Room Molding Polyline, Make Tray Ceiling in Room, Expand Room Polyline, Create Schedule from Room, Create Room Elevation Views and Auto Room Dimensions. | Partial | Round 16 brief 15: `rooms_edit::edit_actions` lists Open Object, Calculate Materials in Room, Turn Ceiling Off/On, Make Room Polyline, Make Standard Area Polyline, Expand Room Polyline, Create Schedule from Room, Create Room Elevation Views and Auto Room Dimensions (DECISIONS RM9); s74 `a_selected_room_offers_the_room_edit_buttons`. Add to Style Palette, Build Framing for Selected, Make Room Molding Polyline and Make Tray Ceiling in Room belong to other owners. |
| R-108 | Standard Area boundary choice and rounding (manual p. 453): Standard Area runs to the center of interior walls and to either the outside surface or the outside of the Main Layer of exterior walls, chosen by the 'Living Area to' setting in General Plan Defaults, is rounded to the nearest square foot and excludes bay, box and bow window areas. | Works | Round 16 brief 15: `plan_core::living::standard_polygon` / `standard_area_sq_in` (center of interior walls, outside surface or Main Layer outside per General Plan Defaults > Living Area to, bay/box/bow excluded, rounded in labels); tests `living::standard_area_runs_to_the_outside_surface_and_the_main_layer`, `a_bay_window_is_in_the_interior_area_but_not_the_standard_area`, s74 `living_area_to_the_main_layer_and_the_label_switch_come_from_general_plan_defaults`; DECISIONS RM6. |
| R-109 | Living Area label per structure and its rules (manual pp. 454-455): A Living Area label is created for every structure with living rooms, recalculated on every change, hidden by General Plan Defaults > Show Living Area Label, not deletable; rooms with rough ceiling under 48 in are excluded; Make Living Area Polyline draws the exact extent. | Works | Round 16 brief 15: `living::living_areas` (one Living Area label per structure, rounded to the nearest square foot, rules: interior in, exterior and hybrid out, rough ceiling under 48 in out, per-room 3-way override, basement of 48 in or more counts), drawn by `rooms_edit/exterior.rs draw`, hidden by Show Living Area Label; tests `living::rooms_rule_the_living_area_at_the_48_inch_boundary`, `a_basement_counts_at_48_inches_or_more`, `each_structure_has_its_own_rounded_living_area`, s74 `a_living_area_label_per_structure_rounds_to_the_nearest_square_foot`, `a_rough_ceiling_under_48_inches_keeps_a_room_out_and_48_brings_it_in`; DECISIONS RM6, RM7. |
| R-110 | Room polylines (room, standard area, living area, molding) (manual p. 455): Static CAD polylines made from a room: Make Room Polyline (surfaces), Make Standard Area Polyline, Make Living Area Polyline (Exterior Room) and Make Room Molding Polyline (dialog: molding to convert or blank, height) which removes that molding from the room; Expand Room Polyline selects an enlarged room that ignores invisible walls and railings. | Partial | Round 16 brief 15: Make Room Polyline, Make Standard Area Polyline, Make Living Area Polyline and Expand Room Polyline (`rooms_edit/commands.rs`, `living::room_polyline`, `standard_area_polyline`, `living_area_polylines`, `structure_polylines`, `expand_room`); s74 `room_polylines_follow_the_surfaces_the_standard_area_and_the_living_area`, `expand_room_polyline_ignores_an_invisible_wall`. Open: Make Room Molding Polyline from a room (tools/molding.rs has the Exterior Room one). |
| R-111 | Tray and coffered ceilings (Tray Ceiling Polyline tool) (manual pp. 457-458): Tray Ceiling Polyline tool and edit buttons (Make Tray Ceiling in Room, Make Nested Tray Ceiling, convert a closed CAD polyline, Explode Tray Ceiling, Build Framing for Selected): upper and lower ceiling plane with a ceiling hole, depth, recess into ceiling, vertical or sloped sides, nested polylines copied into a coffered ceiling, caution symbol when unsupported. | Partial | Round 16: `plan-core/src/tray.rs` (TrayCeiling record keyed by a closed CAD polyline on "Ceiling Planes", `resolve`, `ceiling_height_at`, `coffer_cells`, `make_tray_in_room`, `make_nested_tray`, `convert_polyline_to_tray`; tests in that file), `tools/tray_ceiling.rs` (Tray Ceiling Polyline tool: click in a room or drag a rectangle; Make Tray Ceiling in Room, Make Nested Tray Ceiling, Make Coffered Ceiling, Explode Tray Ceiling, Convert Polyline, Turn Off/On Ceiling), `plan-roof ceiling.rs tray_ceiling_planes`, `plan-3d tray.rs tray_meshes`, `plan-framing floor.rs frame_tray_ceiling`, scenario `s75_tray_ceilings.rs`. Open: Edit toolbar and Open Object routes need the dispatch hooks in docs/integration-queue.md; Convert Polyline dialog entry; Default Settings > Tray Ceiling Defaults (RF-65); Build Framing for Selected Object button (CB-256); verify in Chief. |
| R-112 | Tray Ceiling Specification dialog (manual pp. 458-460): General (width, depth, recess, pitch or vertical, pitch in degrees, ceiling layers and material use, retain framing, rafter spacing), Polyline, Moldings, Rope Lights (offsets, Rope Light Specification), Line Style, Fill Style, Materials, Label, Components. | Partial | Round 16: `dialogs/tray_ceiling.rs` has all nine panels (General with Width in the Make dialogs, Depth, Recess into Ceiling, Pitch/Vertical/Pitch in Degrees, ceiling layers, room material switches, Retain Framing, Rafter Spacing; Polyline; Moldings and Rope Lights, off while sloped; Line Style and Fill Style on the polyline's CAD attributes; Materials; Label; Components) and a section preview; rope light runs are `plan_core::tray::rope_light_paths` for the electrical layer to light. Open: Rope Light Specification Edit button, Material Layers Definition dialog for the ceiling layers (a simple list here), moldings use the Crown profiles of the room library; verify in Chief. |
| R-113 | Room moldings suppressed behind cabinets and per wall (manual p. 462): Room moldings stop behind cabinets, architectural blocks and symbols that have Suppress Adjacent Room Moldings, and along walls with No Room Molding Exterior/Interior; defaults per floor, per room type and per room; none by default for exterior or hybrid rooms. | Partial | plan_core::moldings `room_molding_lines`, `cabinet_obstacles` (moldings stop behind cabinets unless Cut Room Moldings is off), `DetailsLayer.molding_free_walls` and `tools::molding::set_wall_suppresses_moldings`, per-edge switch `set_room_edge`; tests `room_runs_stop_at_cabinets_and_skip_off_edges`, s90 `room_types_floors_and_rooms_set_moldings_in_that_order`. The Wall Specification No Room Molding check boxes and the Suppress switch on blocks and symbols are not wired (integration-queue); DECISIONS MO7; verify in Chief. |
| R-114 | Roof Group on the room (manual pp. 464-466): Roof Group number decides how the room's roof joins the rest of the building when automatic roofs are built (nearly always zero). | Works | Round 16 brief 19: `RoomName.roof_group` (plan-core model.rs), Roof Group drag value in the Room Specification General panel (`dialogs/room.rs`) applied in `editor/rooms_edit.rs`, `plan_roof::assign_roof_groups` (group 0 keeps the plate-height rule of DECISIONS 118; other groups are one building each), `editor/roof_view.rs::level_groups`; tests `group::tests::*`, s78 `a_roof_group_roofs_the_garage_as_its_own_building`; DECISIONS RB1. |
| R-115 | Room Structure panel fields not yet present (manual pp. 466-470): Absolute and relative readouts (Floor Above, Floor Below, SWT Below, Ceiling Below, Stem Wall Top to Ceiling, Floor to Stem Wall Top), Shelf Ceiling, Use Soffit Surface for Ceiling, Floor Supplied by the Foundation Room Below, Build Foundation Below, Raised Floor For Bump Out, Retain Floor/Ceiling Framing, Framing Group, On Structure Resize (lock floor top or bottom) and the hover-highlighting cross-section preview. | Partial | Round 16 brief 15: `dialogs/room.rs` Structure tab lists Absolute Elevations (Floor Above, Ceiling, Floor, Floor Below, SWT Below) and Relative Heights (Rough Ceiling, Finished Ceiling, Stem Wall Top to Ceiling, Floor to Stem Wall Top, Ceiling Below) from `living::room_heights`, a cross-section preview, and stores Shelf Ceiling, Use Soffit Surface for Ceiling, Floor Supplied by the Foundation Room Below, Build Foundation Below, Raised Floor For Bump Out, Retain Floor/Ceiling Framing, Framing Group, Slab Pour Number and On Structure Resize (`RoomOptions`); test `living::the_structure_panel_heights_read_the_floors_above_and_below`; DECISIONS RM10. Open: the effects of the switches in framing, foundation and attic walls; hover highlighting of the preview. |

## Manual audit additions (part 4)

Rows added by the Chief X18 Reference Manual audit, part 4 (pages 762 to 1098; `docs/chief-manual-coverage/part4-floors-stairs-roofs-framing-library-symbols.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the application.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| R-116 | Floor Level Defaults (manual p. 762): Floor Defaults also opens right after a new floor is added. | Works | dialogs/build_tools.rs opens FloorDialog::defaults_for_floor after a Build New Floor OK; test dialogs::floor tests; verify in Chief. |
| R-117 | Floor Level Defaults (manual p. 762): Moldings panel (room base, chair rail, crown defaults for the floor). | Works | dialogs/floor_defaults.rs Moldings panel (MoldingPanel PanelOptions::room) applied with the floor defaults in one undo step (dialogs/floor.rs apply, tools::molding::set_floor_molding_table); s90_moldings floor tables; verify in Chief. |
| R-118 | Floor Level Defaults (manual p. 762): Fill Style panel (default room fill for the floor). | Works | dialogs/floor_defaults.rs Fill Style panel -> FloorSettings::room_fill; rooms_edit::fill_style falls back to it; test dialogs::floor_defaults; verify in Chief. |
| R-119 | Adding floors (manual p. 764): Build New Floor option: Move Highest Floor's Roof Up when roof planes sit on the top floor. | Works | NewFloorOptions::move_roof_up (floors.rs build_new_floor_with, raise_roofs); dialogs/floor.rs RoofChoice (off with Auto Rebuild Roofs); tests crates/plan-core/tests/floors_foundation.rs the_highest_floors_roof_moves_up_with_a_new_floor, a_floor_inserted_below_lifts_the_roof_of_the_floors_above. |
| R-120 | Adding floors (manual p. 764): Build New Floor option: Step floor/ceiling elevations to match the existing floor so existing ceiling heights are kept. | Works | NewFloorOptions::step_elevations (floors.rs step_new_floor_to); tests crates/plan-core/tests/floors_foundation.rs stepping_a_new_floor_keeps_the_ceilings_of_the_floor_below, scenarios/s92_floors_foundation.rs build_new_floor_steps_its_elevations_to_the_floor_below_in_one_undo_step; DECISIONS FF12; verify in Chief. |
| R-121 | Adding floors (manual p. 764): Insert New Floor below the Current Floor, derived from the Current Floor walls, with the same two options and a blank choice. | Works | Action::InsertFloorBelow opens FloorDialog::insert_floor (Place Below, derive, the two options, blank); 30 living floors cap (Project::new_floor_blocker); tests crates/plan-core/tests/floors_foundation.rs inserting_below_steps_the_new_ceilings_to_the_floors_above, a_plan_has_at_most_thirty_living_floors, scenarios/s92_floors_foundation.rs insert_new_floor_goes_below_the_current_floor_derived_from_its_walls. |
| R-122 | Floor and ceiling heights (manual p. 768): Dropped (suspended) ceiling built from a Ceiling Finish Definition with a plenum, framing and drywall layers; raised floor (shower pan) the same way from the Floor Finish Definition. | Works | A Ceiling Finish with Air Gap and Framing layers hangs the finished ceiling lower and leaves the wall tops and the platform (`plan_core::assemblies::{Assembly::drop, ceiling_heights}`, `plan-3d` `slab::room_levels` / `platform_bands`; tests `dropped_ceiling_height_math_keeps_wall_tops_and_platform`, `a_dropped_ceiling_lowers_the_finished_ceiling_and_nothing_else`, scenario s84 `a_hat_channel_dropped_ceiling_lowers_the_finished_ceiling_and_keeps_the_wall_tops`). A raised floor is a Floor Finish of that thickness. DECISIONS LA4, LA6; verify in Chief how the height is entered. |
| R-123 | Platforms (manual p. 769): Floor and ceiling platform definitions are layer stacks (framing, subfloor, drywall, finish) in Floor/Ceiling Structure and Finish Definition dialogs. | Works | `dialogs/assembly_def.rs` `AssemblyDefDialog` (layer table with Material, Fill, Role, Thickness, Insert/Delete/Move, framing options, cross section) behind Edit buttons in Floor Defaults, Room Specification > Structure and the Default Settings page; layers built per slice in 3D and listed in the Materials List (`slab::platform_bands`, `materials/engine/platform_lines.rs`); tests `insert_delete_and_move_edit_the_table`, `a_two_layer_floor_stacks_its_layers_down_from_the_platform_datum`, s84 `tile_over_backerboard_is_a_floor_finish_and_the_materials_list_reads_the_layers`. DECISIONS LA3, LA5, LA6, LA10. |
| R-124 | Platforms (manual p. 770): Platform dynamic defaults chain: plan-wide, floor level (Floor Defaults and Framing Defaults), room type, room, then Joist Direction Line. | Partial | plan-wide (`Project::assemblies.plan_wide`) > floor level (`FloorSettings::platform`) > room (`RoomMisc::assemblies`) with Use Default boxes and the legacy single thickness as the last link (`plan_core::assemblies::resolve`; tests `the_default_chain_runs_room_then_floor_then_plan_wide_then_legacy`, s84 `a_floor_that_uses_the_default_follows_the_plan_wide_page_and_ok_is_one_step`). The room-type level and Joist Direction override of spacing/depth are not wired (DECISIONS LA2); verify in Chief. |
| R-125 | Split level plans (manual p. 772): Adding a floor 2 resets the lower room ceiling to the default unless Relative Rough Ceiling is re-set. | Works | floors.rs build_new_floor_with resets the ceiling heights of raised/lowered rooms unless Step is on (DECISIONS FF12); test crates/plan-core/tests/floors_foundation.rs a_floor_added_above_a_split_level_resets_its_ceiling_unless_stepped; verify in Chief. |
| R-126 | Attic floor (manual p. 773): A warning appears when walls or objects are drawn on the Attic floor. | Works | Project::attic_floor_warning, foundation_view::attic_warning (status line once per visit, called from PlanApp::update via foundation_view::frame); the Attic floor gets no rooms (refresh_attic_floor, DECISIONS FF13); tests crates/plan-core/tests/floors_foundation.rs drawing_on_the_attic_floor_warns_and_it_gets_no_rooms, scenarios/s92_floors_foundation.rs the_attic_floor_warns_when_walls_are_drawn_on_it_and_has_no_rooms. |

## Manual audit additions (part 3)

Rows added by the Chief X18 Reference Manual audit, part 3 (pages 521 to 761; `docs/chief-manual-coverage/part3-text-doors-windows-cabinets-electrical.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the program.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| R-127 | Foundation types and defaults (manual p. 737): Walls with Footings: floor platforms bear on the stem walls, or Hang Floor Above on Wall in the wall's Structure panel; Slab at Top of Stem Wall makes a slab floor platform; a basement ceiling height of 72 in or more gets a slab floor | Works | FoundationOptions::top_gap/floor_height, FoundationSettings::hang_platform, slab_at_stem_top (floors.rs build_foundation_classified; DECISIONS FF1); tests crates/plan-core/tests/floors_foundation.rs stem_wall_height_runs_from_the_footing_to_the_underside_of_the_platform; verify in Chief. |
| R-128 | Foundation types and defaults (manual p. 738): Monolithic Slab: one slab bounded by Slab Footings for each structure; the floor platform of Floor 1 is that slab; Monolithic Slab Foundation and Floor Supplied by the Foundation Room Below tick automatically; different pour numbers per room | Partial | Monolithic Slab: thickened edges with chamfer, a slab per room, curbs and lowered garage, Floor 1 rooms ticked Monolithic Slab Foundation + Floor Supplied by the Foundation and unticked by another type (floors.rs; test crates/plan-core/tests/floors_foundation.rs a_monolithic_slab_lowers_and_curbs_the_garage); Floor 1 Defaults has no such boxes and pour numbers are not split by room (integration queue). |
| R-129 | Foundation types and defaults (manual p. 738): Foundation Defaults dialog: panels Foundation and Options, no changes to the model on OK | Works | dialogs/foundation.rs FoundationForm (Foundation and Options panels), FloorDialog::foundation_defaults, Action::FoundationDefaults via Edit > Default Settings > Foundation (dialogs/defaults.rs DefaultsEntry::Foundation); PlanDefaults::foundation; tests dialogs::floor, dialogs::foundation; DECISIONS FF8. |
| R-130 | Foundation types and defaults (manual p. 739): Auto Rebuild Foundation: rebuild whenever Floor 1 changes affect the foundation (and floor 0 editing is then locked with a warning) | Partial | FoundationSettings::auto_rebuild, foundation_signature, Project::auto_rebuild_foundation, foundation_view::frame from PlanApp::update; Floor 0 cannot be deleted and the foundation tools refuse while on; the Wall tools and Room Specification on Floor 0 do not warn yet (integration queue); tests crates/plan-core/tests/floors_foundation.rs auto_rebuild_follows_floor_one_and_only_when_switched_on, floor_zero_stays_while_auto_rebuild_foundation_is_on, scenarios/s92_floors_foundation.rs floor_zero_cannot_be_deleted_while_auto_rebuild_foundation_is_on. |
| R-131 | Foundation types and defaults (manual p. 739): Foundation Type radio buttons; Hang 1st Floor Platform Inside Foundation Walls; Show S Markers on Step Foundation | Works | FoundationForm: type radios, Hang 1st Floor Platform Inside Foundation Walls (Walls with Footings), Show S Markers (not for piers); foundation_view::draw_step_markers on layer Footings, Step Markers; test crates/plan-core/tests/floors_foundation.rs rooms_at_different_floor_heights_get_a_stepped_foundation_with_s_markers, scenarios/s92_floors_foundation.rs a_foundation_on_a_sloped_terrain_steps_with_s_markers_and_sits_under_the_terrain. |
| R-132 | Foundation types and defaults (manual p. 740): Piers: width, depth, maximum separation, Round or Square | Works | FoundationSettings::pier_width/pier_shape, FoundationOptions::pier_height (Depth)/pier_spacing (Maximum Separation); foundation::pier_positions; Square Pads or Round Piers; test crates/plan-core/tests/floors_foundation.rs piers_are_round_or_square_and_no_further_apart_than_the_maximum; DECISIONS FF9. |
| R-133 | Foundation types and defaults (manual p. 740): Garage Options: Garage Floor to Stem Wall Top, Lower Garage Floor, Minimum Garage Height | Works | FoundationSettings::garage_floor/garage_floor_to_stem_top/lower_garage_floor/min_garage_height; the Build Garage Floor box is stored; tests crates/plan-core/tests/floors_foundation.rs a_garage_gets_a_lowered_slab_stem_walls_and_a_cutout_the_size_of_its_door, a_garage_stem_wall_is_never_shorter_than_the_minimum_garage_height; DECISIONS FF3. |
| R-134 | Foundation types and defaults (manual p. 741): Options panel: Rebar for footing, wall horizontal and vertical courses, pier, slab (bars per course, spacing, size, overlap, mesh), Foam Seal, Termite Flashing in the Materials List | Partial | FoundationSettings::rebar/foam_seal/termite_flashing stored and edited (Options panel); foundation::foundation_takeoff gives the Materials List rows; the Materials List engine does not call it yet (integration queue); test crates/plan-core/tests/floors_foundation.rs the_options_panel_feeds_the_materials_list. |
| R-135 | Building a foundation (manual p. 742): Foundation is generated under exterior walls of rooms with Build Foundation Below (never railings or invisible), interior walls flagged Create Wall/Footing Below, and interior walls between rooms of different floor heights; at least one such room is needed | Works | floors.rs plan_walls (exterior with Build Foundation Below, Create Wall/Footing Below, bearing, slab separation, height steps); WallFoundation::create_below; tests crates/plan-core/tests/floors_foundation.rs interior_walls_get_a_footing_only_when_they_ask_or_bear, a_room_stem_wall_height_overrides_the_default; DECISIONS FF4; the Wall Specification check box is in the integration queue. Round 16 finish: walls are cut at T-junctions so a step falls at the partition (`floors::t_junctions`, `plan_piece`; tests `a_long_wall_is_cut_where_a_partition_meets_it_so_the_foundation_steps_there`, `a_door_in_a_cut_wall_keeps_its_place_in_the_piece_it_stands_in`, scenarios `s92_floors_foundation::a_foundation_on_a_sloped_terrain_steps_with_s_markers_and_sits_under_the_terrain`, `a_garage_door_leaves_a_curb_cutout_as_wide_as_its_rough_opening_and_concrete_cutout`; DECISIONS FF17). |
| R-136 | Displaying foundations (manual p. 743): Layers: Walls, Foundation; Footings (also piers/pads); Footings, Post and Footings, Deck Post; Slabs, Custom; Walls, Labels and Polylines 3D, Labels; brick ledge lines; Foundations layer for 3D | Partial | Layer Footings, Step Markers added (foundation_view); the other layers as before; verify in Chief. |
| R-137 | Editing foundations (manual p. 746): Footing Width, Height and Offset come from the Foundation Wall and Slab Footing defaults, then per wall; Footing Width drags with handles in plan and 3D (each side separately), by dimensions for slab footings; stem wall heights drag in 3D | Partial | Slab Footing Offset and Width/Height in the Slab Specification (plan-3d slab_mesh draws the offset); footing handles in plan and 3D unchanged; verify in Chief. |
| R-138 | Editing foundations (manual p. 747): Stepped stem walls with vertical footings; chamfer width and height of monolithic slab footings (thickened slabs and edges) | Works | FoundationSettings::vertical_step_footings, chamfer_width/chamfer_height written to the walls (WallFoundation vertical_footing, chamfer_monolithic); steps found by foundation::step_markers; tests crates/plan-core/tests/floors_foundation.rs a_monolithic_slab_lowers_and_curbs_the_garage, rooms_at_different_floor_heights_get_a_stepped_foundation_with_s_markers; DECISIONS FF5. Round 16 finish: walls are cut at T-junctions so a step falls at the partition (`floors::t_junctions`, `plan_piece`; tests `a_long_wall_is_cut_where_a_partition_meets_it_so_the_foundation_steps_there`, `a_door_in_a_cut_wall_keeps_its_place_in_the_piece_it_stands_in`, scenarios `s92_floors_foundation::a_foundation_on_a_sloped_terrain_steps_with_s_markers_and_sits_under_the_terrain`, `a_garage_door_leaves_a_curb_cutout_as_wide_as_its_rough_opening_and_concrete_cutout`; DECISIONS FF17). |
| R-139 | Foundations and rooms, terrain (manual p. 749): Basement rooms: ceiling height 48 in or more counts toward living area without finishes, 72 in or more gets a 4 in slab and a floor finish like Floor 1, 76 in or more a ceiling finish | Works | floors.rs build_foundation_classified tiers (72 in finished, 48 in living, else crawl space; DECISIONS FF2) with the existing living::room_in_living_area; test crates/plan-core/tests/floors_foundation.rs a_basement_of_48_inches_counts_as_living_area_and_72_gets_a_slab. |
| R-140 | Foundations and rooms, terrain (manual p. 749): Terrain sits 6 in below the top of stem walls or grade beams, 8 in below the top of a monolithic slab; daylight and walkout basements by unticking Automatic in the Terrain Specification (Elevation At, Flatten Pad off); stepped foundations from several floor heights | Partial | foundation::terrain_elevation, site_view::auto_building_pad (Automatic elevation; 6 in below stem tops, 8 in below a monolithic slab; DECISIONS FF11); test scenarios/s92_floors_foundation.rs a_foundation_on_a_sloped_terrain_steps_with_s_markers_and_sits_under_the_terrain; daylight and walkout conditions stay hand set in the Terrain Specification. |
| R-141 | The Slab tools (manual p. 752): Slab Specification General: Hole in Slab, Thickness, Elevation Reference with Top and Bottom, Post Footing Square or Round; Footing: Has Footing, Height, Width, Footing Offset | Works | dialogs/foundation.rs Slab Specification: Hole in Slab, Thickness, Elevation Reference with Top and Bottom, Has Footing with Height, Width and Footing Offset; Draft::apply moves a slab to the holes and back; tests dialogs::foundation; DECISIONS FF16; verify in Chief. |
| R-142 | The Slab tools (manual p. 754): Piers and Pads: select in 2D and 3D, always under a wall, move only to another wall, three plan handles like a CAD line, 3D box handles; move with their wall or beam | Partial | Pier/Pad Specification Type, Top/Bottom Height, Width (R-85); the under-a-wall constraint and moving with the wall: not built. |

## Manual audit additions (part 7)

Rows added by the Chief X18 Tutorial Guide audit, part 7 (pages 1 to 517; `docs/chief-manual-coverage/part7-tutorial-workflows.md`). Each is a workflow step the tutorials rely on that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the application.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| R-143 | Floor/Ceiling Platform Defaults page (tutorial pp. 13, 178, 204): Floor Structure, Floor Finish, Ceiling Structure and Ceiling Finish Edit buttons that open layered definitions every floor and room type can follow. | Works | `PlatformDefaultsPage` in `dialogs/assembly_def.rs` (Default Settings > Floors and Rooms > Floor/Ceiling Platform, `Leaf::Platforms`) reports each total depth and opens the layers window; Floor Defaults shows the same four rows with Use Default (`floor_defaults.rs`); OK is one undo step (s84). Room-type defaults dialogs do not have the rows yet (see R-124). DECISIONS LA2, LA7. |
| R-144 | Structure definition fields (tutorial pp. 13, 111, 182): per layer Framing Method (joists), Construction (Lumber, I-Joist, Hat Channel), Width, Spacing and Role, so the joist layer drives the framing and a layer can be changed from I-Joist to 2x12 lumber. | Partial | The Framing layer carries method (Joists, Rafters, Trusses, Purlins), construction (Lumber, I-Joist, Hat Channel), width and spacing and the Role column has Standard, Framing, Sheathing, Finish, Air Gap and 3D Cladding (`FramingSpec`, `LayerRole`); the Materials List reads the spacing. `plan-framing` still takes joist size and spacing from Framing Defaults (CB-220), not from the structure: left in docs/integration-queue.md. |
| R-145 | Room Supplies Floor for the Room Above (tutorial p. 50): a foundation-level room whose slab and curbs become the floor of the room above (a garage on a slab). | Partial | Round 16 brief 15: Room Supplies Floor for the Room Above is a check box on the Structure tab stored in `RoomOptions::supplies_floor_above` (s74 `the_structure_panel_options_are_kept_with_the_room`); the foundation builder does not read it yet (docs/integration-queue.md). |
| R-146 | Flat Ceiling Over This Room (tutorial pp. 145, 184): unchecking it makes the room's ceiling follow the underside of the roof or ceiling plane above (a cathedral ceiling); checked gives the flat ceiling. | Works | Round 16 brief 15: the Structure tab's Flat Ceiling Over This Room check box is bound to `RoomName::flat_ceiling`, `apply_room_spec` copies it, and Turn Ceiling Off/On on the room's Edit toolbar call `tray_ceiling::set_flat_ceiling`; s74 `turn_ceiling_off_and_on_flips_the_flat_ceiling_one_step_each`, `the_structure_panel_options_are_kept_with_the_room`. Use Room Ceiling Finish on roof planes (p. 847) stays with the roof owner. |
