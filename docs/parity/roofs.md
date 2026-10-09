# Parity spec: Roofs (Chief Architect X18)

> Status (2026-10-08, re-audited against HEAD 4c11b69): 60 ids: 42 Works, 15 Partial, 3 Missing, 0 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.
>
> Builder update (2026-10-08, roofs round): built since the audit, each with tests: **dormer overhang** (`DormerSpec::overhang`, eaves and rakes pushed out per edge with hips and valleys kept on their lines, fascia/rake boards/soffit from the eave detail, the hole stays the wall footprint); **gable triangle** above an exploded gable dormer's front wall (Full Gable wall, rises to the dormer roof); a **roofless room across planes** gets a hole piece in every plane it reaches (`plan_roof::hole_pieces`); the **flat roof overhangs** its exterior edges (`flat_roof_plane_with_overhang`); the **Dutch gable's vertical face** (`build_roof_with_faces`, a wall-like face stored with the roof, not a plane); **ceiling planes mitre** where they meet (`ceiling_plane_meshes_joined`); roof plane **edge, pitch-arrow and rotate handles** in Edit Roof Planes (RF-38). Still open: a dormer that spans two planes, a break per edge on a staged roof, curved roofs, Build Roof per group (framing).

> Builder update (2026-10-08, roofs round 14): **Roof Styles** in Build Roof (Hip, Gable, Shed, Gambrel, Dutch Gable, Half Hip) write the exterior walls' roof directives before the build (`roof_view::{RoofStyle, apply_style}`, one undo step with the build), and the Edit toolbar sets Hip / Full Gable / High Shed / Knee / Dutch Gable Wall on a selection of exterior walls (`set_walls_roof_kind`, `run_wall_command`); a **half hip** is a Full Gable wall with an Upper Pitch (its Starts at Height is where the small hip begins; `plan-roof/src/halfhip.rs`). **Wings** (RF-9): rooms standing at different plate heights, and first-floor rooms the floor above does not cover, each get their own roof at their own plate (`floor_levels`, `wing_regions`, `region_roof`, `make_wing_planes`); an edge against a taller wall or an upper floor rises to it as a high shed with no overhang; Auto Rebuild watches the walls of the floors below (`project_signature`). **Extend Slope Downward** reaches down to the wall below when the wall gives no drop (`wall_below_drop`). **Roof plane editing**: pitch, edge and rotate handles listed under Select (`HandleKind::{Pitch, EdgeMove}`, `roof_view::apply_handle_drag`), **Edit All Roof Planes** (`AllPlanesDialog`, `AllPlanesEdit`), polygon roof holes (click corners; `add_hole_polygon`, pieces across a ridge), **Structure > Define** on the plane (`RoofStructure`; thickness and rafter size reach 3D through `EaveOverrides`). **Dormers**: the Auto Dormer tools outline the dormer under the pointer and a dragged dormer moves onto the plane it is carried over (`slide_dormer`). **Bay, box and bow windows** have a hip or shed roof (`plan_3d::bay_roof_into`). Still open: a dormer spanning two planes, a break per edge on a staged roof, curved roofs, Build Roof per group (framing), the Select tool's drag of the new plane handles (select.rs), pitch / overhang / none options for the bay roof (fields on the opening).

Scope: Build Roof dialog, auto roof generation rules, per-wall roof directives,
manual roof planes and their edit handles, Join Roof Planes, Roof Hole, Skylight,
Gable/Roof Line, dormers, roof returns, ceiling planes, attic walls, and roof
framing (rafters, trusses, truss base). Wall dialog fields are quoted from
`docs/chief-x18-dialogs.md` (Wall Specification > Roof tab, captured
2026-10-07); everything else is from the Chief X18 Reference Manual as
remembered. Lines marked "(verify in Chief)" need checking in the application.
Lengths in inches, pitch as rise per 12 of run unless stated.

Code read: `crates/plan-roof/src/{lib,footprint,skeleton,tests}.rs` (1,420 lines,
straight-skeleton roof generator, no UI), `plan-core/src/model.rs` (Wall has no
roof fields), `plan-3d/src/mesh.rs` (a `Material::Roof` exists but no roof mesh
builder), `plan-core/layers.rs` (layer "Roof Planes" exists). `plan-framing` is a
placeholder crate.

## 1. Build Roof dialog

RF-1. Build > Roof > Build Roof (Ctrl+Opt+Shift+Cmd+N) opens the Build Roof
dialog. Pressing OK builds roof planes over the exterior walls of the highest
floor(s) and generates the result as editable roof plane objects on layer
"Roof Planes".

RF-2. Dialog options (verify exact labels in Chief):
- Build Roof Planes (on by default).
- Build Ceiling Planes (vaulted/cathedral ceilings under sloped roof planes).
- Build Framing (also generate rafters or trusses).
- Auto Rebuild Roofs (keep roof synced with wall edits, see RF-6).
- Ignore Top Floor / "Roof Over Floor": build the roof over a chosen floor
  rather than the topmost; walls above are ignored for the roof footprint.
- Default pitch and overhang fields (override Floor/Wall defaults for this
  build) and a roof style preset.

RF-3. **Roof style presets** in the dialog or Roof Defaults: Gable, Hip, Shed
(single slope), Gambrel, Mansard, Dutch Gable, Flat. Choosing a preset sets the
roof directive of exterior walls for this build (Hip -> every wall Hip; Gable ->
the two short walls Full Gable; Shed -> one wall High Shed). (verify list in
Chief)

RF-4. Build Roof always builds from **exterior wall roof directives** (RF-18),
so the dialog preset is a shortcut that writes those wall settings.

RF-5. Roof defaults (pitch 8 in 12, overhang 16", fascia, frieze, soffit) live
in Default Settings > Roofs, and each wall's Roof tab can override them
(pitch use-default wrench, `chief-x18-dialogs.md`).

RF-6. **Auto Rebuild Roofs**: when on, any change to the walls that influence the
roof (move, add, delete, directive change, height change) rebuilds the *auto*
roof planes. Planes the user edited by hand (RF-37) are not rebuilt (they stay as
they are). Turning it off requires explicit Build Roof; "Delete Roof Planes"
removes planes without rebuilding.

RF-7. Build Roof when planes already exist asks whether to replace the existing
roof (all auto planes are rebuilt, manual planes are kept unless the user
chooses Delete). (verify in Chief)

RF-8. Rebuild Walls/Floors/Ceilings (F12) does not rebuild roofs; Build Roof
(or Auto Rebuild) does.

## 2. Automatic roof generation rules

RF-9. A roof is generated for each **roof level**: any group of exterior walls
whose top is exposed (no wall above it on the next floor). A two-story house
with a one-story garage gets a main roof over floor 2 walls and a separate roof
over the garage walls, joined where they meet.

RF-10. Roof planes spring from a **baseline** along the top outside edge of each
exterior wall, at wall-top height. Overhang is measured horizontally from the
outside surface of the wall (the wall's outer layer face), not its centerline
(verify in Chief for sheathing vs framing reference).

RF-11. Each plane rises toward the building centre at the wall's pitch. Where
adjacent planes meet they form hips (convex corners), valleys (reflex corners)
and ridges (opposite planes), with different pitches meeting at the correct
skewed hip.

RF-12. L, T, U and any rectilinear footprint produce valleys at interior
corners and a ridge on each wing; the roof is watertight (planes share edges).

RF-13. Walls at a different height (a lower wing butting an upper wall) produce
a roof that stops at the higher wall ("butting roof"): the lower roof plane ends
in a roof/wall intersection, and the wall may split ("Lower Wall Type if Split by
Butting Roof").

RF-14. Plane thickness is the roof structure thickness (rafter + sheathing +
roofing); the 3D plane has a top surface, an underside (soffit/ceiling) and a
fascia edge of the fascia height (default 6" nominal).

RF-15. Eave detail per plane: Fascia height, Eave Cut (plumb, level, square),
Soffit (Include Frieze) and rafter tails. Eaves and rakes use the plane's
Overhang value; rakes use the gable overhang.

RF-16. Roof planes cut the walls beneath them: wall tops intersect roof planes
so gable-end walls reach to the roof surface; exterior wall siding stops at the
plane. "Roof Cuts Wall at Bottom" cuts the wall's lower end instead of its top
(wall hung below a sloping roof).

RF-17. Rebuilding is deterministic: the same walls and settings produce the same
planes, with a stable plane ordering so schedules and callouts do not shuffle.

## 3. Per-wall roof directives (Wall Specification > Roof)

RF-18. Roof Options radios: **Hip Wall** (default), **Full Gable Wall**,
**Dutch Gable Wall**, **High Shed/Gable Wall**, **Knee Wall**, **Extend Slope
Downward**; plus ☐ Roof Cuts Wall at Bottom, ☑ Include Frieze, ☐ Include
Automatic End Truss Above.

RF-19. **Hip Wall**: a roof plane rises from the wall at the wall's pitch; the
plane is hipped at both ends where it meets neighbouring planes.

RF-20. **Full Gable Wall**: no plane rises from the wall; the wall extends upward
to the roof ridge as a triangular gable, and the neighbouring planes extend to
the gable wall's outer face plus the gable overhang (rake). The gable is part of
the wall (it gets siding, framing and a Gable Vent location).

RF-21. **Dutch Gable Wall**: a hip roof with a small gable at the top: the roof
hips at the wall, but above a break height the end is a short vertical gable. The
gable height is set by Pitch Options (Starts at Height, In from Baseline).
(verify in Chief for which fields drive the gable)

RF-22. **High Shed/Gable Wall**: the wall is the high side of a shed or the
high gable of a monoslope roof; the plane rises *toward* the wall and the wall
extends up to meet it (no overhang plane on that side).

RF-23. **Knee Wall**: the wall carries a roof plane over it without cutting it;
the wall rises to meet the underside of an existing plane (storage knee walls
under a roof slope, attic knee walls).

RF-24. **Extend Slope Downward**: the roof plane keeps sloping below the wall
top (an eyebrow or low eave).

RF-25. **Pitch Options**: Pitch (rise in 12, with "use default" wrench);
☐ Upper Pitch (second pitch) with "Starts at Height" (above floor) and
"In from Baseline" (horizontal). The pair describes a **break**: the plane
changes slope at that height or distance. Together they build gambrel and
mansard roofs.

RF-26. **Overhang**: Length (default 16"), measured as in RF-10.

RF-27. **Auto Roof Return**: ☐ with Length (36"), Extend (0"), Roof Type
(Gable / Hip / Full), Slope (Sloping / Flat), ☑ Include Shadow Boards,
☐ Include Ridge Caps, ☐ Include Frieze, ☐ Include Gutter. Creates a **roof
return** at a gable end: a short eave that wraps the gable corner for a given
length so the rake meets the eave in a boxed return.

RF-28. **Lower Wall Type if Split by Butting Roof** [Interior-6 ▾]: wall type to
use for the portion of a wall shielded from the weather after a roof butts it.

RF-29. **Treat As Part Of Bay/Box/Bow Window**: ☐ Use Existing Roof,
☐ Extend Existing Roof Over: lets a bay window's walls attach to the roof of the
parent wall instead of generating their own.

RF-30. Interior walls and railings have no roof directive; only exterior
walls (including attic walls) feed Build Roof.

## 4. Attic walls and attic floor

RF-31. **Attic walls** are walls on an Attic floor (wall option "Attic Wall" in
the General tab) or generated automatically under sloped roof planes to define
the attic space. They are marked "Automatically Generated Wall" and are not
editable except through the roof. (verify in Chief for exact naming)

RF-32. When the roof is built over a floor whose rooms have ceilings, the attic
space between the ceiling platform and the roof is *not* a room; an Attic floor
only exists if the user builds one (Build New Floor, "Attic"; or by enabling
Build Attic in the roof dialog; verify in Chief).

RF-33. Attic floor walls follow the roof underside: the walls are cut by roof
planes at their top and rest on the attic floor platform below.

RF-34. Dormers and gable walls are generated as walls; the Attic floor may
contain their back walls.

## 5. Manual roof planes

RF-35. **Roof Plane tool** (Build > Roof > Roof Plane, hotkey `Q`): in plan,
click and drag to draw the plane's **baseline** (the eave line, the lowest edge),
then click to set the opposite edge (the **ridge** or the plane's far boundary);
the plane forms a quadrilateral on the baseline with the slope rising away from
the baseline. Polyline planes (click each vertex, double-click to close) are also
supported; the first edge is the baseline. (verify in Chief for the exact
gesture; the baseline-first behavior is documented)

RF-36. A roof plane's properties: Pitch (rise in 12 or degrees), Baseline
height/elevation (above floor), Overhang, Thickness, Eave cut, Fascia height,
Layer ("Roof Planes"), Material (roofing), and Structure (Define...). Roof
Plane Specification opens on double-click: General, Structure, Roof (rafter
spacing, truss), Eave, Layer, Materials, Label, Components.

RF-37. Manually drawn planes, and auto planes the user has moved, resized or
re-pitched by hand, become **manual** and are skipped by Auto Rebuild Roofs
(RF-6). A plane's context menu "Reset to automatic" is not offered; deleting the
plane and rebuilding is the way back. (verify in Chief)

RF-38. **Edit handles** on a selected plane: vertex handles on the outline
(drag moves a vertex along the plane; the plane stays planar), edge handles
(move an edge; baseline edge drag changes baseline position), a **pitch/slope
arrow** at the centre (drag changes pitch; typed pitch in the temporary
dimension), a **move** handle (translate in plan), and a **rotate** handle.
Edge drags keep the plane's pitch unless the pitch handle is used.

RF-39. **Edit All Roof Planes** (Ctrl+Opt+Shift+Cmd+P): selects all auto planes
and opens the multiple-selection dialog so pitch/overhang can be changed
together.

RF-40. **Delete Roof Planes** / **Delete Ceiling Planes**: remove all roof (or
ceiling) planes of the active floor/roof level in one step.

RF-41. **Join Roof Planes**: click plane A, then plane B; the planes are trimmed
or extended along their intersection so they meet cleanly in a ridge, hip or
valley. Works for planes of different pitch, and extends a plane past its edge
to meet the other. (verify in Chief for the trim vs extend rule)

RF-42. **Roof Hole** (Ctrl+Opt+Shift+Cmd+T): draw a closed polygon (rectangle by
two corners, or polyline); every roof plane under it is cut by the hole, used
for chimneys, skylight wells and roof access. The hole has Specification fields:
Edge treatment and Layer.

RF-43. **Skylight** (Ctrl+Opt+Shift+Cmd+S): click on a roof plane to place a
skylight (Library or Skylight dialog) of default size; it cuts the roof
automatically, sits at the plane's pitch, and has an optional shaft down to the
ceiling. Skylights move only along their roof plane.

RF-44. **Gable/Roof Line tool** (Ctrl+Opt+Shift+Cmd+O): draws a line on the
roof that becomes a gable edge: planes are trimmed along the line and the
triangular gable wall is generated. Also used to define a roof/eave line cut in
plans. (verify in Chief)

## 6. Ceiling planes

RF-45. **Ceiling Plane tool** (Ctrl+Opt+Shift+Cmd+U): draws a sloped ceiling
plane over a room region (vaulted/cathedral/tray). Properties: Pitch, Height at
low edge, Direction, Thickness. Ceiling planes override the room's flat ceiling
(`rooms-floors.md` R-32).

RF-46. **Build Ceiling Planes** (Build Roof dialog): for each room under a
sloped roof plane, creates a ceiling plane parallel to the roof plane with the
roof structure thickness removed, so vaulted ceilings follow the roof. Rooms
with a flat ceiling set on the Room Specification are kept flat.

RF-47. Ceiling planes show in 3D as the underside of the roof and are included in
Materials List ceiling area. They appear in plan as dashed outlines when the layer
is on. (verify in Chief)

> Round 16 (tray ceilings): ceiling plane height sampling (`plan_roof::ceiling_height_at`, `cathedral_height_at`, `room_ceiling_height_at`), cathedral planes by the room's Flat Ceiling Over This Room switch with shelf rooms cut out (`cathedral_ceiling_planes`, `subtract_polygon`), and the ceiling planes of a tray (`tray_ceiling_planes`, used by Explode Tray Ceiling) are in `plan-roof/src/ceiling.rs` with unit tests; rows RF-45 to RF-47 keep their statuses.

## 7. Dormers and skylight features

RF-48. **Auto Dormer** (Ctrl+Opt+Shift+Cmd+Z): click a roof plane; Chief builds
a dormer there: front wall, two cheek walls (side walls), a gable or hip or shed
roof, windows, and the cut in the main roof, using Dormer Defaults (width, wall
height, roof type and pitch, overhang, window). The dormer's roof auto-joins the
main roof with valleys.

RF-49. **Auto Floating Dormer**: a dormer whose back is not supported by the
floor below (a roof dormer over a vaulted ceiling); generated as a freestanding
roof structure with its own walls over the roof (verify in Chief).

RF-50. **Manual dormers**: build dormer walls on the floor (or Attic floor), set
their roof directives, and draw/join roof planes with the Roof Plane tool and
Join Roof Planes. A hole is cut in the main roof with Roof Hole or by the dormer
itself.

RF-51. Dormers are objects with a specification dialog (Dormer Specification);
resizing the dormer rebuilds its walls and roof. They can be moved along the
roof plane.

## 8. Roof framing

RF-52. **Build Framing > Roof Framing** generates framing for the roof planes:
rafters (default 2x8 at 16" o.c.), ridge board/beam, hip and valley rafters,
jack rafters, collar ties, purlins, and blocking, according to Framing Defaults.
(verify defaults in Chief)

RF-53. **Rafter** tool (single member, drawn along a plane) and **Roof Beam**,
**Roof Blocking**, **Roof Purlin** tools draw individual framing parallel to the
roof plane; each is a Framing object with a spec dialog.

RF-54. **Roof Truss** tool: click-drag a rectangular or polygon region to place
a truss bay; trusses are placed at the truss spacing (default 24" o.c.), run
perpendicular to the **Roof Truss Direction** (a marker that sets orientation),
and bear on the **Truss Base** (a plane/loop marking the bearing height, normally
the top of walls). **Girder Truss** doubles the first truss at changes in
direction.

RF-55. Truss types: common, scissor, hip, girder, mono, attic; each spec has Heel
Height, Overhang, Top/Bottom chord pitch and Tail type; trusses are generated
as 3D objects with web layout (verify depth of web generation in Chief).

RF-56. Roof Framing respects Floor/Ceiling Framing: ceiling joists or bottom
chords create the ceiling platform; Build All Framing builds floors, walls and
roof together.

RF-57. Roof framing, once built, is a set of editable objects. It is rebuilt
when "Build Framing" is run again; manual members are kept (framing has its own
"Retain Framing" setting). (verify in Chief)

## 9. Selection, display and output

RF-58. Plan display: roof planes show in plan with a dashed outline and a
hash/arrow indicating slope direction, a pitch triangle, hip/ridge/valley lines,
and eave outline on the "Roof Planes" layer. The roof is visible only when the
layer is on (and in the floor above the walls it covers, when the "Roof" Display
option is on).

RF-59. Roof Plane label: pitch (e.g. 8/12), optional area and direction; Text
Macros.

RF-60. Roof area and ridge lengths flow to the Materials List (roofing,
underlayment, ridge cap, gutters, fascia) and the roof pitch diagram.

## 10. Plan Studio today

`plan-roof` builds roofs and the editor drives it: Build Roof (with auto rebuild), Roof Plane, edit, Gable/Roof Line, Roof Hole and Skylight tools, a Roof Specification dialog, the Edit toolbar's Rebuild Roofs, and roof planes in the 3D view. Walls carry roof directives in the model; Build Roof takes its default pitch and overhang from the exterior wall defaults. Roof records (planes, ceiling planes, dormers and the Build Roof settings) are stored per floor in the typed `Floor.roofs` slot, so they save and undo with the plan. Roof holes and skylights are cut in 3D (the skylight has a curb, frame and glass), ceiling planes (layer `Ceiling Planes`), Auto Dormer / Explode Dormer, Gable/Roof Line on an eave and Roof Return are live, and the Roof Plane Specification has a Holes tab and per-edge pitch / overhang / gable fields that feed Build Roof. Roof framing is available from Build > Framing (rafters, ridge, hips, valleys, collar ties, ceiling joists or trusses). Join Roof Planes (`plan_roof::join_planes`), Auto Floating Dormer, Build Ceiling Planes (Build Roof dialog), the Ceiling Plane Specification, Extend Slope Downward and the wall's Auto Roof Return flag are live too. Walls now build up to the roof in 3D (gable triangles to the ridge, tops cut to the roof underside, interior walls up to vaulted ceiling planes, knee walls to the plane), a lower roof is trimmed at a taller wall with a flashing line and an attic wall above it, and planes carry 6 in of structure with fascia, soffit, rake boards, optional frieze and ridge caps (`plan-3d` `cover.rs`, `eave.rs`). Missing: Dutch gable, dormer walls as wall objects (Explode Dormer keeps only the planes and the hole), gutters, eave cut and Roof Cuts Wall at Bottom, Lower Wall Type split, and Default Settings fields for the new sizes. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## 11. Gap table

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| RF-1, RF-2 Build Roof dialog (planes, ceiling planes, framing, auto rebuild, ignore top floor) | No dialog or command | Critical | Dialog on shared frame; command calls `footprint_from_walls` + `build_roof`, stores planes in model (`Floor.roof_planes`) |
| RF-3, RF-5 Roof style presets and Roof Defaults | `EdgeRoof::default()` constants only | High | `RoofDefaults` in project; presets write wall directives |
| RF-6, RF-37 Auto Rebuild vs manual-plane protection | Not present | Critical | `RoofPlane.origin: Auto|Manual`; rebuild replaces only Auto; hook into wall edit events |
| RF-9 One roof per roof level (garage + main) | Single outer footprint; multiple buildings pick largest only | Critical | Compute exposed exterior wall groups per height level; call `build_roof` per group; merge butts via Join logic |
| RF-10 Overhang/baseline from wall outer surface | From wall centerline (`footprint_from_walls`) | High | Offset footprint outward by half wall thickness (or main-layer reference) before skeleton |
| RF-13 Butting roofs and split walls | Not present | High | Planes stop at higher wall; generate `Lower Wall Type` split in wall mesh |
| RF-14, RF-15 Roof thickness, fascia, soffit, eave cuts (plumb/level/square) | `fascia_height` constant, flat polygons | High | Thickened plane solid in `plan-3d`; eave cut enum; soffit underside |
| RF-16 Roof cuts walls (gable walls to roof, Roof Cuts Wall at Bottom) | Walls are boxes of fixed height | Critical | Clip wall solid against roof planes in `plan-3d::wall`; triangle gable infill |
| RF-18 Wall roof directives stored per wall | None on `Wall` | Critical | Add `Wall.roof: RoofDirective {kind, pitch, upper_pitch, start_height, in_from_baseline, overhang, return..}`; map to `EdgeRoof` |
| RF-19, RF-20 Hip, Full Gable | Hip, Gable implemented in skeleton | Done (core) | Keep; add Chief rake overhang for gables (rake uses gable overhang) |
| RF-21 Dutch Gable | Done: a hip below the break, a short gable above it, and the vertical face standing on the cut (`plan_roof::build_roof_with_faces`, stored as `"face"` records, meshed as wall) | Done | One break height per roof; a break per edge |
| RF-22 High Shed/Gable | `Shed` treated like Gable | Med | Distinct handling: rise toward wall, wall extends to plane, no overhang on that side |
| RF-23, RF-24 Knee Wall, Extend Slope Downward | Extend Slope Downward done (fixed 24" drop below the eave, `EXTEND_SLOPE_DROP`); Knee Wall missing | Partly done | Knee wall flag consumed by wall mesh clip; drop that reaches the wall below |
| RF-25 Upper pitch / break at height / in from baseline (gambrel, mansard) | Single pitch per edge | High | Extend skeleton to two-stage speed (second wavefront pass at break height) |
| RF-27 Auto Roof Return | Roof Return tool: click an eave corner (full, half with Shift, boxed with Alt, 24") | Done (flag) | Length / type dialog; the wall's `auto_roof_return` flag makes full 24" returns at both corners of a gable end |
| RF-28, RF-29 Lower wall type, bay/box/bow roof attach | Missing | Low | Depends on wall type system and bay window objects |
| RF-31..RF-34 Attic walls auto-generation and attic floor | Missing | High | Generate wall objects flagged `auto_generated` from roof underside; Attic `FloorKind` (see rooms-floors.md R-68) |
| RF-35, RF-36 Manual Roof Plane tool (baseline then ridge/polygon), Roof Plane spec | Missing | Critical | Tool in `plan-app`; `RoofPlane` as editable object with pitch/baseline height; dialog |
| RF-38 Edit handles (vertex, edge, pitch arrow, move, rotate) | Done in Edit Roof Planes: corner handles, edge-middle handles (move the edge square to itself), a pitch arrow up the slope (quarter steps) and a rotate knob below the eave; each drag is one undo step and keeps the plane planar | Done | The same handles under the Select tool (`editor/handles.rs`) |
| RF-39, RF-40 Edit All Roof Planes, Delete Roof/Ceiling Planes | Missing | Med | Multi-select dialog; delete commands |
| RF-41 Join Roof Planes | Done: `plan_roof::join_planes`, Join mode (pick an edge, then the other plane) and the Edit toolbar button | Done | Join more than one edge at once |
| RF-42 Roof Hole | Done: rectangle drag, dashed in plan, cut in 3D | Done (rectangles) | Polygon holes |
| RF-43 Skylight | Done: drag or click, curb/frame/glass in 3D, spec in the plane dialog | Done | Library skylights |
| RF-44 Gable/Roof Line | Done: click an eave (automatic planes rebuild, manual planes use `apply_gable_line`) | Done | Free line trimming |
| RF-45..RF-47 Ceiling planes and Build Ceiling Planes (vaulted) | Ceiling Plane tool, Ceiling Plane Specification, Delete Ceiling Planes, Build Ceiling Planes (rooms with Ceiling Over This Room off); planes that meet at a ridge, hip or valley are mitred in 3D | Done | Room-level ceiling height for flat ceilings |
| RF-48..RF-51 Auto/floating/manual dormers | Auto Dormer with its dialog (now with Overhang: fascia, rake boards and soffit) and Explode Dormer (real walls, the gable triangle above the front wall) | Partly done | Auto Floating Dormer is live; manual dormers; a dormer across two planes |
| RF-52..RF-57 Roof framing: rafters, trusses, truss base/direction | `plan-framing` empty | High | See CB-framing items in cabinets-stairs-framing-terrain-library.md |
| RF-58, RF-59 Plan display of roof (dashed, slope arrows, pitch triangle, labels) | No drawing | High | Draw in `draw_*` layer "Roof Planes"; label with pitch text |
| RF-60 Roof quantities to Materials List | `materials_list` has no roof lines | Med | Add roof area, ridge/hip/valley length, fascia length from planes |
| 3D roof mesh with correct materials, thickness | `Material::Roof` unused | Critical | `plan-3d::roof` module producing top, underside, edges per plane |
| Test coverage vs Chief for tricky footprints (concave, 45-degree walls, disjoint) | Tests cover rect, L, T, U, mixed pitch | Med | Add curved-wall footprints (facets), 45-degree hips, multi-building |

<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| RF-61 | Curved roofs / curved roof planes: Barrel and bell-curve roof planes on curved baselines. (Not captured; verify in Chief.) | Missing | roof planes are planar; curved walls give faceted eaves only (grep curved roof finds nothing) |
<!-- coverage-audit:end -->

## Manual audit additions (part 4)

Rows added by the Chief X18 Reference Manual audit, part 4 (pages 762 to 1098; `docs/chief-manual-coverage/part4-floors-stairs-roofs-framing-library-symbols.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the application.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| RF-62 | Auto vs manual (manual p. 822): Roofs generated from a Roof Baseline Polyline with its own specification dialog. | Missing | no baseline polylines (grep finds none); manual audit part 4; verify in Chief. |
| RF-63 | Roof Defaults (manual p. 823): Default Settings > Roofs and Custom Ceilings lists Roof Defaults, Skylight Defaults, Ceiling Plane Defaults, Tray Ceiling Defaults, Dormer Defaults, plus roof framing defaults. | Partial | Default Settings tree has Roof Defaults and Dormer pages; no Skylight, Ceiling Plane or Tray Ceiling Defaults (dialogs/defaults.rs TREE); manual audit part 4; verify in Chief. |
| RF-64 | Roof Defaults (manual p. 823): Skylight Defaults dialog (also by double-click on the Skylight tool). | Missing | no Skylight defaults page; manual audit part 4; verify in Chief. |
| RF-65 | Roof Defaults (manual p. 823): Ceiling Plane Defaults and Tray Ceiling Defaults dialogs (double-click the tools). | Missing | no pages in the Default Settings tree; manual audit part 4; verify in Chief. |
| RF-66 | Auto roofs (manual p. 826): Curved walls roof at a segment angle from 6 to 90 degrees chosen in Build Roof. | Missing | curved walls give faceted eaves with a fixed step; no angle field (grep finds none); manual audit part 4; verify in Chief. |
| RF-67 | Auto roofs (manual p. 826): Concave curved walls roofed when the base is longer than the Minimum Alcove Size, otherwise simplified. | Missing | no field; manual audit part 4; verify in Chief. |
| RF-68 | Rebuilding (manual p. 827): Rebuild deletes and replaces every roof plane; Retain Manually Drawn Roof Planes and Retain Edited Roof Planes keep them. | Partial | manual planes always kept, automatic planes always replaced; no retain switches and no Mark as Edited flag (roof_view.rs origin); manual audit part 4; verify in Chief. |
| RF-69 | Rebuilding (manual p. 827): A question message appears when editing or drawing a plane while Auto Rebuild Roofs is on. | Missing | editing is allowed and the plane becomes manual; manual audit part 4; verify in Chief. |
| RF-70 | Build Roof Roof: Build (manual p. 828): Build Roof Planes; Auto Rebuild Roofs; Make Roof Baseline Polylines (alternative to building planes). | Partial | Build Roof Planes and Auto Rebuild Roofs present; baseline polylines missing; manual audit part 4; verify in Chief. |
| RF-71 | Build Roof Roof: Build (manual p. 829): Retain Manually Drawn Roof Planes; Retain Edited Automatic Roof Planes (new plane coplanar with a retained one is dropped); Use Existing Roof Baselines. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-72 | Build Roof Roof: Specs (manual p. 829): Pitch as rise over 12 (metric: degrees) and Pitch in Degrees for dialogs and labels (-89 to 89). | Partial | Pitch in 12 only (pitch_row); degrees not offered (grep finds none); manual audit part 4; verify in Chief. |
| RF-73 | Build Roof Roof: Height (manual p. 829): Heel Height (trusses only). | Missing | no roof heel field (trusses have heel_height in TrussSpec); manual audit part 4; verify in Chief. |
| RF-74 | Build Roof Roof: Height (manual p. 829): Automatic Birdsmouth Cut off to set Raise Off Plate, Birdsmouth Cut and Birdsmouth Seat for rafters. | Partial | Raise Roof Off Plate present; birdsmouth depth and seat are in Framing Defaults (Birdsmouth seat) only; manual audit part 4; verify in Chief. |
| RF-75 | Build Roof Roof: Height (manual p. 829): Vertical Structure Depth read-out. | Missing | not shown; manual audit part 4; verify in Chief. |
| RF-76 | Build Roof Roof: Height (manual p. 830): Eave alignment: Same Roof Height at Exterior Walls (changes overhangs so eaves meet) and Same Height Eaves. | Missing | no switches (grep finds none); eaves of mixed pitch are built as the straight skeleton gives them; manual audit part 4; verify in Chief. |
| RF-77 | Build Roof Roof: Height (manual p. 830): Allow Low Roof Planes (uncheck only when an upper floor overhangs lower roofs). | Missing | no switch; manual audit part 4; verify in Chief. |
| RF-78 | Build Roof Roof: Options (manual p. 830): Segment Angle at Curved Wall and Minimum Alcove Size. | Missing | see Auto roofs; manual audit part 4; verify in Chief. |
| RF-79 | Build Roof Options (manual p. 831): Boxed Eave (horizontal soffit), Flush Eave, Higher Eaves Boxed, Default to Overhang with Length. | Missing | soffit is level or sloped (sloped_soffit) with no boxed-eave flush options; manual audit part 4; verify in Chief. |
| RF-80 | Build Roof Options (manual p. 832): Ceiling Break Lines: display at finish or framing intersection. | Missing | no break lines on the plan (grep finds none); manual audit part 4; verify in Chief. |
| RF-81 | Build Roof Options (manual p. 832): Use Room Ceiling Finish, Has Ceiling, Ceiling Thickness for the underside of roof planes. | Missing | ceiling finish comes from the room only; manual audit part 4; verify in Chief. |
| RF-82 | Build Roof Options (manual p. 832): Show All Ridges (hip lines over a conical roof on a curved wall in vector views). | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-83 | Build Roof Rafter Tails (manual p. 833): One rafter tail profile, Stretch to Fit Rafter or own height and width, Extend past subfascia. | Partial | Exposed Rafter Tails switch with width and depth (RoofDetailDefaults); no profile pick; manual audit part 4; verify in Chief. |
| RF-84 | Build Roof Ridge Caps (manual p. 833): Ridge cap profile list, size, Bend to Roof Pitch. | Partial | Ridge and Hip Caps on/off (RoofDetailDefaults.ridge_caps); no profile or size; manual audit part 4; verify in Chief. |
| RF-85 | Build Roof Gutter (manual p. 833): Gutter profile list for eaves. | Partial | Gutters switch with a size (gutter_size); no profile pick; manual audit part 4; verify in Chief. |
| RF-86 | Build Roof Frieze (manual p. 833): Frieze molding profiles for eaves and gable overhangs. | Partial | Frieze Board switch only; manual audit part 4; verify in Chief. |
| RF-87 | Build Roof Shadow Boards (manual p. 833): One or more shadow board profiles that follow fascia on eaves, gables or both (needs fascia). | Missing | no shadow boards (grep finds none); manual audit part 4; verify in Chief. |
| RF-88 | Build Roof Line Style (manual p. 834): Line Style panel for roof planes in plan. | Missing | no Line Style tab on the plane dialog; manual audit part 4; verify in Chief. |
| RF-89 | Build Roof Fill Style (manual p. 834): Fill Style panel for roof planes in plan. | Missing | no Fill Style tab on the plane dialog; manual audit part 4; verify in Chief. |
| RF-90 | Build Roof Materials (manual p. 834): Materials panel with roof component materials (surface, fascia, soffit and so on). | Partial | one Roofing material combo (Materials tab); components not separate; manual audit part 4; verify in Chief. |
| RF-91 | Build Roof Arrow (manual p. 834): Slope arrow appearance. | Missing | no Arrow tab; manual audit part 4; verify in Chief. |
| RF-92 | Build Roof Components (manual p. 834): Components, Object Information and Schedule panels. | Missing | not on the Roof Plane dialog; manual audit part 4; verify in Chief. |
| RF-93 | Roof planes (manual p. 835): Baseline: height = top plate elevation plus structure depth minus birdsmouth; shown as a separate line on the Roofs, Baselines layer with an upslope tick. | Missing | baseline height field in General tab; no baseline line or layer shown in plan; manual audit part 4; verify in Chief. |
| RF-94 | Roof planes (manual p. 836): Baseline drawn over another roof plane takes that plane's height at the start point. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-95 | Displaying roofs (manual p. 837): Roofs, Baselines, Gable Lines, Ceiling Break Lines, Baseline Polylines show in plan only. | Partial | Gable lines draw; the others have no layers; manual audit part 4; verify in Chief. |
| RF-96 | Displaying roofs (manual p. 837): Plane display moves to another floor with Display on Floor Above / Below edit buttons (skylights move with it). | Missing | no such buttons (grep finds none); manual audit part 4; verify in Chief. |
| RF-97 | Displaying roofs (manual p. 837): Projected vs actual size: perimeter and area both reported; edge length entered as projected or at pitch. | Partial | Surface Area read-out on the plane dialog; no projected/actual choice; manual audit part 4; verify in Chief. |
| RF-98 | Displaying roofs (manual p. 838): Ceiling plane and skylight labels have a blank automatic label and no slope indicator. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-99 | Displaying roofs (manual p. 838): Section views show roof surface, sheathing and ceiling layers; framing on if its layer is on; flat ceiling not generated over rooms with Flat Ceiling Over This Room. | Partial | section scene shows plane thickness; per-layer display not confirmed; manual audit part 4; verify in Chief. |
| RF-100 | Displaying roofs (manual p. 838): Poche fill on clipped roof edges in sections. | Partial | Section cut regions of roof planes (and floor / ceiling platforms) are gray poché regions already; `plan_elevation::poche_hatch_lines` hatches them from a Fill Style and `without_poche` drops them with the view's switch (`PocheSettings`, default on in sections); tests a_fill_style_hatches_the_cut_faces_only, the_poche_switch_off_drops_the_cut_fill_but_not_the_faces; the camera and layout callers are queued (integration-queue brief 08 item 3). |
| RF-101 | Displaying roofs (manual p. 839): Roof Planes, Ceiling Planes, Roof Holes, Skylights and roof trim items appear in schedules (skylights and holes under Windows, trim under Roof Trim). | Missing | no roof schedule category (grep finds none); manual audit part 4; verify in Chief. |
| RF-102 | Editing planes (manual p. 839): Select Next Object picks the Baseline, editing it apart from the plane (handles at ends and midpoint). | Missing | no baseline object; manual audit part 4; verify in Chief. |
| RF-103 | Editing planes (manual p. 840): Raise or lower a plane: lock the pitch and change a height; or Transform/Replicate. | Partial | Baseline Height field; pivot locks missing; Transform skips roof planes (DECISIONS 16); manual audit part 4; verify in Chief. |
| RF-104 | Editing planes (manual p. 840): Pitch change pivots about the locked height; with Top of Plate locked the pivot depends on Automatic Birdsmouth Cut. | Missing | no lock radio buttons; manual audit part 4; verify in Chief. |
| RF-105 | Editing planes (manual p. 841): Birdsmouth cut: notch on a rafter at the top plate; no birdsmouth with Trusses or with a Raise/Lower of at least 1/16 inch. | Partial | Birdsmouth seat in the rafter cut (plan-framing member.rs Birdsmouth); not tied to roof height; manual audit part 4; verify in Chief. |
| RF-106 | Editing planes (manual p. 841): Baseline edits: moving it changes the plane height not the baseline height; angle changes the pitch direction (dialog tilts about an axis, Rotate handle changes direction). | Partial | Rotate handle on a plane (RF-38); baseline angle field missing; manual audit part 4; verify in Chief. |
| RF-107 | Editing planes (manual p. 841): Plane edges snap to the outside surface of a parallel nearby wall (Use Special Snapping on the plane). | Missing | no wall snapping for plane edges; manual audit part 4; verify in Chief. |
| RF-108 | Editing planes (manual p. 841): Make Parallel/Perpendicular aligns a plane edge with a wall or another straight edge. | Partial | CAD edit tool exists (S/CAD rows); not wired to roof planes; manual audit part 4; verify in Chief. |
| RF-109 | Editing planes (manual p. 842): Bumping/Pushing: a plane bumps CAD objects when CAD Stops Move is checked. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-110 | Editing planes (manual p. 842): Components of a plane can be edited in a live Materials List (with prompt when Auto Rebuild is on). | Missing | no live materials list editing of planes; manual audit part 4; verify in Chief. |
| RF-111 | Joining planes (manual p. 843): Place Roof Plane Intersection Point edit button: temporary CAD point where an edge would meet the selected plane. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-112 | Aligning eaves (manual p. 844): Same Roof Height at Exterior Walls vs Same Eave Heights; Independent planes keep their overhangs. | Missing | see Build Roof Height; manual audit part 4; verify in Chief. |
| RF-113 | Move to be coplanar (manual p. 844): Move to be Coplanar edit button: move one plane into the plane of the next (parallel baselines). | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-114 | Set Baseline Height (manual p. 844): Dialog when a new plane's baseline lies on an existing plane: Over Wall Top (full height dormer) or Over the Existing Roof Plane (dormer vent, cricket). | Missing | no dialog; manual audit part 4; verify in Chief. |
| RF-115 | Roof Plane Spec: General (manual p. 845): Four 3D orientation values (Ridge Top Height, Baseline Height, Fascia Top Height, Pitch) each lockable as the pivot; heights measured from the Floor 1 default elevation. | Partial | Pitch and Baseline Height only; no ridge or fascia heights and no pivot locks; manual audit part 4; verify in Chief. |
| RF-116 | Roof Plane Spec: General (manual p. 846): Top of Plate reference, Heel Height (trusses only), Shadow Board Top height. | Missing | not shown; manual audit part 4; verify in Chief. |
| RF-117 | Roof Plane Spec: General (manual p. 847): Diagram for rafters or trusses showing the lock point. | Missing | not shown; manual audit part 4; verify in Chief. |
| RF-118 | Roof Plane Spec: General (manual p. 847): Measurements read-outs: Structure Thickness, Birdsmouth Depth and Seat, Vertical Structure Depth, Overhang from Baseline. | Partial | Thickness and Surface Area read-outs; others missing; manual audit part 4; verify in Chief. |
| RF-119 | Roof Plane Spec: General (manual p. 847): Use Special Snapping to walls. | Missing | not present; manual audit part 4; verify in Chief. |
| RF-120 | Roof Plane Spec: General (manual p. 847): Baseline angle (relative to the XY axis) and which end keeps the baseline height. | Missing | not present; manual audit part 4; verify in Chief. |
| RF-121 | Roof Plane Spec: Options (manual p. 847): Options panel equals the Build Roof Options panel for the selected plane. | Partial | Options tab with Ridge Caps and Eave Cut (dialogs/roof.rs); boxed eave, supply flags missing; manual audit part 4; verify in Chief. |
| RF-122 | Roof Plane Spec: profile panels (manual p. 848): Rafter Tails, Ridge Caps (with On Selected Edge Automatic/On/Off), Gutter, Frieze, Shadow Boards panels per plane. | Partial | per-plane Default/On/Off for rafter tails, ridge caps, gutters on the Options tab (RF-15); no profile tables or per-edge choice; manual audit part 4; verify in Chief. |
| RF-123 | Roof Plane Spec: Polyline (manual p. 849): Polyline panel: Perimeter, Framing Area, Projected Area, Roof Surface Area, Overhang Area, Projected Overhang Area. | Partial | Surface Area only; manual audit part 4; verify in Chief. |
| RF-124 | Roof Plane Spec: other (manual p. 849): Selected Line, Line Style, Fill Style, Arrow, Materials, Label, Components, Object Information, Schedule panels. | Partial | Materials, Label, Layer present; the rest absent; manual audit part 4; verify in Chief. |
| RF-125 | Roof Baseline Polylines (manual p. 850): Make Roof Baseline Polylines creates closed polylines along the outside of the exterior walls (one per height), editable and reusable by Use Existing Roof Baselines. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-126 | Roof Baseline Polylines (manual p. 851): Directive text V, G, K, L on each edge with pitch or (vert); edit with polyline handles and Intersect/Join Two Lines; straight sides only. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-127 | Roof Baseline Specification (manual p. 852): Roof Baseline panel: Baseline Height, Roof Options (Hip, Full Gable, Dutch Gable, High Shed/Gable, Knee, Extend Slope Downward, Against Wall), Pitch Options; Polyline, Selected Line, Line Style, Fill Style panels. | Missing | not supported (the Build Roof Edge tab edits per-edge pitch and overhang of the automatic footprint); manual audit part 4; verify in Chief. |
| RF-128 | Roof framing (manual p. 853): Framing for manually drawn Ceiling Planes generated with roof framing from Build Roof defaults. | Missing | ceiling planes have no framing (grep finds none); manual audit part 4; verify in Chief. |
| RF-129 | Roof framing (manual p. 853): Purlins generated in the Surface and Ceiling layers of planes (not drawn by hand). | Missing | purlins are manual members only (RoofPurlin tool); manual audit part 4; verify in Chief. |
| RF-130 | Ceiling Plane Spec (manual p. 859): General: Ridge Height, Height Inside Wall, Height Outside Wall, Pitch (lockable pivots), Elevation Reference, Pitch in Degrees. | Partial | General tab with pitch and height fields (dialogs/roof.rs CeilingDialog); no pivot locks or elevation reference; manual audit part 4; verify in Chief. |
| RF-131 | Ceiling Plane Spec (manual p. 860): Measurements: Structure Thickness, Vertical Rafter Depth, Top of Plate, Overhang from Wall Inside, Clip End; Curved Ceiling. | Missing | not shown; manual audit part 4; verify in Chief. |
| RF-132 | Ceiling Plane Spec (manual p. 860): Structure, Polyline, Selected Line, Line Style, Fill Style, Materials, Label, Components, Object Information, Schedule panels. | Partial | Line Style and Layer tabs only; manual audit part 4; verify in Chief. |
| RF-133 | Gable/Roof Lines (manual p. 862): Gable Over Door/Window edit button: a small gable line over selected openings (12 inches each side, merged when within 30 inches) at the next roof build. | Missing | no button (grep finds none); manual audit part 4; verify in Chief. |
| RF-134 | Gable/Roof Lines (manual p. 863): Delete Gable Over Opening; remove by deleting the line or Reset to Defaults. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-135 | Gable/Roof Lines (manual p. 863): Cover an alcove with a line aligned to the exterior main layer, or a bay with a line extending past the bay side walls so the gable is built over the bay. | Missing | needs the line object; manual audit part 4; verify in Chief. |
| RF-136 | Gable Line Spec (manual p. 864): Gable Line Specification: Gable Line panel (pitch and overhang of the two planes), Line panel, Line Style panel, Arrow panel. | Missing | no gable line object, so no dialog; manual audit part 4; verify in Chief. |
| RF-137 | Skylights and holes (manual p. 866): Skylights and holes live on Roofs, Openings and show in plan even when their roof does not; true shape only at zero pitch. | Partial | holes draw with their plane; separate layer absent; manual audit part 4; verify in Chief. |
| RF-138 | Skylights and holes (manual p. 866): Skylights resize only in the dialog or with Edit Skylight Shape (detail window with a closed polyline). | Missing | no Edit Skylight Shape (grep finds none); manual audit part 4; verify in Chief. |
| RF-139 | Skylight Spec (manual p. 867): Skylight General: Shape (Circle, Ellipse, Oval, Rectangle, Custom), Width and Height or Diameter, Lock Top/Center/Bottom. | Missing | skylights are rectangles only; spec has curb height, glass thickness, frame width; manual audit part 4; verify in Chief. |
| RF-140 | Skylight Spec (manual p. 867): Frame: Display in Plan View, frame Width and Height. | Partial | Frame Width field (dialogs/roof.rs Holes tab); manual audit part 4; verify in Chief. |
| RF-141 | Skylight Spec (manual p. 867): Inside Hole Rim: Square Sides, Plumb Sides, Plumb/Square. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-142 | Skylight Spec (manual p. 868): Ceiling Hole: Automatically Generate, Manually Edit Ceiling Hole Polyline, Do Not Generate; Generate Shaft to Ceiling Hole. | Missing | no ceiling hole choices (grep finds none); manual audit part 4; verify in Chief. |
| RF-143 | Skylight Spec (manual p. 868): Polyline, Selected Line, Line Style, Fill Style, Materials, Label, Components, Object Information, Schedule panels. | Missing | listed under the plane's Holes tab only; manual audit part 4; verify in Chief. |
| RF-144 | Dormers (manual p. 869): Dormers on their own layer Roofs, Dormers. | Missing | no layer (grep finds none); manual audit part 4; verify in Chief. |
| RF-145 | Dormers (manual p. 871): Crickets: automatic cricket against walls that face up slope of an adjacent roof, or drawn by hand with Roof Plane and Place Roof Plane Intersection Point. | Missing | no cricket (DECISIONS 204 says none built at chimneys); manual audit part 4; verify in Chief. |
| RF-146 | Dormer Spec: Roof (manual p. 873): Roof Type: Hip, Gable, Shed, Gambrel, Mansard, Barrel, Curved Eave, Hip Curved Eave, Eyebrow. | Partial | Gable, Shed and Hip only (plan-roof dormer.rs DormerKind); manual audit part 4; verify in Chief. |
| RF-147 | Dormer Spec: Roof (manual p. 873): Pitch with Pitch in Degrees; Second Pitch and In from Eave for upper-roof types; shed default 3 in 12. | Partial | Pitch in 12 (default 8); no second pitch, no degrees; manual audit part 4; verify in Chief. |
| RF-148 | Dormer Spec: Roof (manual p. 873): Roof Overhang: Eave and Gable. | Partial | one Overhang field; manual audit part 4; verify in Chief. |
| RF-149 | Dormer Spec: Roof (manual p. 873): Framing: Rafters or Trusses, Include Ridges, Ridge Depth, Fascia Depth, Eave Fascia Depth, Rafter Depth, Boxed Eaves, Gutters, Frieze, Shadow Boards, Plumb or Square cut. | Missing | none of these dormer framing fields; manual audit part 4; verify in Chief. |
| RF-150 | Dormer Spec: Roof (manual p. 873): Auto Roof Return on gable dormers: length, extend, Gable/Hip/Full, Sloping/Flat, shadow boards, ridge caps, frieze. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-151 | Dormer Spec: Walls (manual p. 874): Wall Type with Define, Height, Set to Existing Ceiling, Height to Reach Existing, Width. | Partial | Width, wall height, setback and position fields; no wall type pick or Set to Existing Ceiling; manual audit part 4; verify in Chief. |
| RF-152 | Dormer Spec: Walls (manual p. 874): Dormer Room options for floating dormers: Create Shaft to Room Below, Form Room Inside Dormer, Neither. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-153 | Dormer Spec: Walls (manual p. 874): Set Inside Window Trim Width (defaults only) and Auto Resize Windows. | Missing | not supported; manual audit part 4; verify in Chief. |
| RF-154 | Dormer Spec: Line Style (manual p. 875): Line Style panel for the dormer. | Missing | tabs are General, Roof, Window; manual audit part 4; verify in Chief. |
| RF-155 | Roof details (manual p. 876): Boxed eaves with flat soffit reaching the gable fascia or flush with the wall. | Missing | see Build Roof Options; manual audit part 4; verify in Chief. |
| RF-156 | Roof details (manual p. 877): Shadow boards are molding polylines on Roofs, Trim, per eave or gable eave, require fascia; editing a polyline removes the Automatic mark. | Missing | no shadow boards; manual audit part 4; verify in Chief. |
| RF-157 | Roof details (manual p. 877): Soffits follow rafter ends and butt the fascia; Flat Under Eave Subfascia; Gable Subfascia Depth sets soffit height. | Partial | Soffit and Sloped Soffit (RF-15) ; subfascia depths missing; manual audit part 4; verify in Chief. |
| RF-158 | Roof details (manual p. 878): Rafter tail recipes: exposed, hidden trimmed, partially exposed (Soffits, Eave Subfascia, Eave Fascia, Trim Framing To Soffits, Gable Subfascia Depth). | Partial | Exposed Rafter Tails vs Soffit; no Trim Framing to Soffits or subfascia depth; manual audit part 4; verify in Chief. |
| RF-159 | Roof details (manual p. 879): Decorative rafter tail profile from molding profiles (own profiles allowed). | Missing | no profile pick; manual audit part 4; verify in Chief. |
| RF-160 | Roof pitches in degrees (manual p. 879): Conversion table from rise over 12 to degrees and Pitch in Degrees entry anywhere. | Missing | no degree entry (grep finds none); manual audit part 4; verify in Chief. |
| RF-161 | Framing tools (manual p. 913): Roof Purlin needs a Purlins layer in the Roof Surface Definition and runs perpendicular to the pitch. | Partial | RoofPurlin member without a plane layer check; manual audit part 4; verify in Chief. |
| RF-162 | Moldings panel (manual p. 971): On Selected Edge Automatic/On/Off for ridge caps, gutters, shadow boards on a roof plane edge. | Missing | per-plane only; manual audit part 4; verify in Chief. |
| RF-163 | Molding polylines (manual p. 979): Automatically generated polylines (gutters, shadow boards, ridge caps) marked Automatic. | Missing | roof trim is generated in 3D, not as polyline objects; manual audit part 4; verify in Chief. |

## Manual audit additions (part 7)

Rows added by the Chief X18 Tutorial Guide audit, part 7 (pages 1 to 517; `docs/chief-manual-coverage/part7-tutorial-workflows.md`). Each is a workflow step the tutorials rely on that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the application.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| RF-164 | Residential Template behaviour on room definition (tutorial pp. 6, 15, 129): as soon as walls enclose a room, a roof is built and Auto Exterior Dimensions appear, and both then follow wall edits (Auto Rebuild Roofs, Auto Refresh). | Missing | roof_view::auto_rebuild only follows a roof that was built once (settings exist); Auto Exterior Dimensions run from the tool only and have no Auto Refresh switch (DIM-52, DIM-60); nothing fires when a room closes; tutorial audit (part 7). |
| RF-165 | In From Baseline: the upper-pitch break entered as a distance in from the baseline, linked to Starts at Height (Tab updates the other) (tutorial pp. 133-136, 149). | Missing | dialogs/wall.rs Roof tab offers Starts at Height only (line ~1971); tutorial audit (part 7). |
| RF-166 | Roof panel for several walls at once and for interior-class walls (tutorial pp. 128-135, 145, 148-149, 163-164): roof directives typed once for a Shift-selection (gambrel, mansard, half hip, Dutch gable), and on Half Walls, Railings and interior Knee Walls. | Missing | dialogs/wall.rs WALL_TABS_MULTI has no Roof tab; WALL_TABS has off("Roof") for Interior-kind walls, and Half Wall and Railing are Interior-kind (tools/wall.rs); the edit-toolbar roof commands (roof_view.rs wall_edit_actions) appear only when an Exterior wall is selected; tutorial audit (part 7). |
