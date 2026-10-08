# Parity spec: Roofs (Chief Architect X18)

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

`plan-roof` builds roofs and the editor drives it: Build Roof (with auto rebuild), Roof Plane, edit, Gable/Roof Line, Roof Hole and Skylight tools, a Roof Specification dialog, the Edit toolbar's Rebuild Roofs, and roof planes in the 3D view. Walls carry roof directives in the model; Build Roof takes its default pitch and overhang from the exterior wall defaults. Roof records (planes, ceiling planes, dormers and the Build Roof settings) are stored per floor in the typed `Floor.roofs` slot, so they save and undo with the plan. Roof holes and skylights are cut in 3D (the skylight has a curb, frame and glass), ceiling planes (layer `Ceiling Planes`), Auto Dormer / Explode Dormer, Gable/Roof Line on an eave and Roof Return are live, and the Roof Plane Specification has a Holes tab and per-edge pitch / overhang / gable fields that feed Build Roof. Roof framing is available from Build > Framing (rafters, ridge, hips, valleys, collar ties, ceiling joists or trusses). Missing: Dutch gable, knee wall, Auto Floating Dormer, Join Roof Planes, Build Ceiling Planes, a Ceiling Plane dialog, dormer walls as wall objects (Explode Dormer keeps only the planes and the hole), gutters, fascia and soffit. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

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
| RF-21 Dutch Gable | Missing | High | Skeleton with zero-speed segment above break height; or post-process hip into Dutch gable |
| RF-22 High Shed/Gable | `Shed` treated like Gable | Med | Distinct handling: rise toward wall, wall extends to plane, no overhang on that side |
| RF-23, RF-24 Knee Wall, Extend Slope Downward | Missing | Med | Per-wall flags consumed by wall mesh clip and eave extent |
| RF-25 Upper pitch / break at height / in from baseline (gambrel, mansard) | Single pitch per edge | High | Extend skeleton to two-stage speed (second wavefront pass at break height) |
| RF-27 Auto Roof Return | Roof Return tool: click an eave corner (full, half with Shift, boxed with Alt, 24") | Partly done | Length / type dialog and the wall's `auto_roof_return` flag |
| RF-28, RF-29 Lower wall type, bay/box/bow roof attach | Missing | Low | Depends on wall type system and bay window objects |
| RF-31..RF-34 Attic walls auto-generation and attic floor | Missing | High | Generate wall objects flagged `auto_generated` from roof underside; Attic `FloorKind` (see rooms-floors.md R-68) |
| RF-35, RF-36 Manual Roof Plane tool (baseline then ridge/polygon), Roof Plane spec | Missing | Critical | Tool in `plan-app`; `RoofPlane` as editable object with pitch/baseline height; dialog |
| RF-38 Edit handles (vertex, edge, pitch arrow, move, rotate) | Missing | Critical | Handle set in plan hit-testing; keep plane planar by solving z from pitch/baseline |
| RF-39, RF-40 Edit All Roof Planes, Delete Roof/Ceiling Planes | Missing | Med | Multi-select dialog; delete commands |
| RF-41 Join Roof Planes | Missing | High | Plane-plane intersection trim/extend operation |
| RF-42 Roof Hole | Done: rectangle drag, dashed in plan, cut in 3D | Done (rectangles) | Polygon holes |
| RF-43 Skylight | Done: drag or click, curb/frame/glass in 3D, spec in the plane dialog | Done | Library skylights |
| RF-44 Gable/Roof Line | Done: click an eave (automatic planes rebuild, manual planes use `apply_gable_line`) | Done | Free line trimming |
| RF-45..RF-47 Ceiling planes and Build Ceiling Planes (vaulted) | Ceiling Plane tool and Delete Ceiling Planes (Build Ceiling Planes not yet) | Partly done | `ceiling_planes_for_vaulted_room` from the Build Roof dialog |
| RF-48..RF-51 Auto/floating/manual dormers | Auto Dormer with its dialog and Explode Dormer | Partly done | Floating and manual dormers, dormer walls as wall objects |
| RF-52..RF-57 Roof framing: rafters, trusses, truss base/direction | `plan-framing` empty | High | See CB-framing items in cabinets-stairs-framing-terrain-library.md |
| RF-58, RF-59 Plan display of roof (dashed, slope arrows, pitch triangle, labels) | No drawing | High | Draw in `draw_*` layer "Roof Planes"; label with pitch text |
| RF-60 Roof quantities to Materials List | `materials_list` has no roof lines | Med | Add roof area, ridge/hip/valley length, fascia length from planes |
| 3D roof mesh with correct materials, thickness | `Material::Roof` unused | Critical | `plan-3d::roof` module producing top, underside, edges per plane |
| Test coverage vs Chief for tricky footprints (concave, 45-degree walls, disjoint) | Tests cover rect, L, T, U, mixed pitch | Med | Add curved-wall footprints (facets), 45-degree hips, multi-building |
