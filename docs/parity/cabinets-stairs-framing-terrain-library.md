# Parity spec: Cabinets, Stairs, Framing, Terrain, Library, Electrical (Chief X18)

> Status (2026-10-08): 68 ids: 38 Works, 27 Partial, 2 Missing, 1 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.

Scope: the six "object families" that share the Library Browser and the
Default Settings tree. Cabinet fields are quoted from `docs/chief-x18-dialogs.md`
(Cabinet Specification, captured 2026-10-07). Stairs, framing, terrain, library
and electrical behavior is from the Chief X18 Reference Manual as remembered;
the dialogs for these were not captured ("Still to capture" in the dialogs doc).
Lines marked "(verify in Chief)" need checking in the application.

Code read: `plan-cabinets`, `plan-stairs`, `plan-framing` (each a 1-line
placeholder crate, `Cargo.toml` only), `plan-library/src/*` (catalog, search,
tree, 1,057-line starter catalog), `plan-docs/src/materials.rs`, `plan-core`
layers. `plan-app` depends only on `plan-core` and does not use `plan-library`
yet. There is no terrain code anywhere.

## A. Cabinets

CB-1. Cabinet tools (Build > Cabinet, flyout): Base Cabinet (Shift+T), Wall
Cabinet (Cmd+T), Full Height, Soffit (T), Shelf, Partition, Base/Wall/Full-
Height Filler, Custom Countertop, Custom Backsplash, Custom Counter Hole.

CB-2. **Placement**: select the tool, move the cursor; a ghost cabinet follows
at the default width/depth, then **click** to place. The tool stays active for
repeated placement until Select Objects or Esc.

CB-3. **Auto-rotation against walls**: when the ghost is near a wall its back
rotates to face that wall and the cabinet snaps flush to the wall's interior
surface; near a corner it picks the wall the cursor is closer to. Cabinets away
from walls keep the angle set by the tool (0 degrees).

CB-4. **Neighbor alignment ("bumping")**: a cabinet moved or placed next to
another cabinet snaps to the neighbor's side (zero gap) and aligns its back/
front line with it. Edit > Snap Settings > Bumping/Pushing controls whether a
moved cabinet is stopped by walls and other cabinets or pushes them.

CB-5. **Fill between**: placing a base cabinet in a gap bounded by walls or other
cabinets can auto-size its width to fill the gap (width stays at the nearest
standard increment unless auto-fill is chosen). (verify in Chief for the
modifier and Preferences > Behaviors name)

CB-6. Default sizes: Base 24" W x 24" D x 36" H (36" includes countertop);
Wall cabinet 12" D x 30" H with bottom at 54" above finished floor; Full Height
24" D x 84" H; Elevation reference "From Finished Floor" (`chief-x18-dialogs.md`).

CB-7. **Cabinet Specification** tabs: General, Box Construction, Front/Sides/
Back, Door/Drawer, Accessories, Opening Indicators, Moldings, Layer, Fill Style,
Materials, Label, Components, Object Information, Schedule. Preview with plan/
front/side/perspective buttons. Field list as captured: Cabinet Style Type
(Standard) and Treat As Filler; Size and Position; Countertop; Backsplash; Toe
Kick; Box Construction (Framed/Frameless, overlay type); Door/Drawer; Accessories;
Moldings.

CB-8. **Resize handles** on a selected cabinet: left and right handles change
width (the cabinet grows from the dragged side), front handle changes depth,
corner handles resize both, a centre move handle, and a rotate handle. Resize
snaps to adjacent cabinets/walls. Width changes can be typed as a temporary
dimension. Edit Behaviors > Resize chooses whether adjacent cabinets follow.

CB-9. Moving a cabinet by dragging keeps its rotation until it bumps a wall
(CB-3), where it re-rotates; Ctrl while dragging disables the auto-rotation.
(verify in Chief for the modifier)

CB-10. **Front/Sides/Back face editing**: each of the six faces (Front, Left,
Right, Back, Top, Bottom) has a Side Type (Custom Face, Door, Drawer, Panel,
Open, ...) and a tree of Face Items (Layout Vertical/Horizontal, Door, Drawer,
Separation, Shelf, Appliance, Empty) with Add New, Delete, Move Up/Down, Split
Vertical/Horizontal, Equalize, plus Item Height/Width/Reveal, Lock from Auto-
Resize, Shelves and Appliance specify fields (captured).

CB-11. Splitting a face item recomputes sibling heights/widths automatically
unless an item is locked; "Equalize" divides evenly.

CB-12. **Door/Drawer** styles, handle, hinge and drawer front come from the
Library (Main Style with Library... and Edit...); Door Panel thickness 3/4";
handle offsets from top/edge (captured).

CB-13. **Auto labels**: each cabinet carries a Label (Label tab) shown in plan,
composed from cabinet type and size (for example B24 for base 24", W3030 for wall
30x30; verify format in Chief) and used as the schedule number. Labels are text-
style driven and rotate with the cabinet; they can be hidden per cabinet.

CB-14. **Countertops auto-join**: base cabinets with ☑ Countertop whose sides
touch merge into one continuous countertop slab; overhangs apply only to free
edges; Corner Treatment (None/Clipped/Rounded) applies at outside corners. Gaps
between cabinets break the joined top.

CB-15. **Custom Countertop**: draw a polyline countertop (island, peninsula,
vanity) independent of cabinets; has thickness, overhang, edge, and holes for
sinks/cooktops. Custom Backsplash and Counter Hole are siblings.

CB-16. **Appliances and fixtures in cabinets**: dragging or placing a library
appliance (range, dishwasher, sink, cooktop, microwave) over a cabinet inserts it
into the cabinet: the face item becomes an Appliance item (Reverse Appliance
option), a cut-out is made in the countertop for sinks/cooktops, and the
appliance bottom aligns to the cabinet.

CB-17. **Soffit tool**: draws a soffit box above cabinets or along a wall (click
two corners / polyline); height set by dialog; fills the gap between wall
cabinets and ceiling.

CB-18. **Shelf** and **Partition** tools: place a horizontal shelf or vertical
partition panel as cabinet-like objects with thickness and elevation; they snap
to walls.

CB-19. **Fillers**: thin cabinet-like strips (3" typical) auto-placed with Treat
As Filler; used to close gaps at corners.

CB-20. Cabinet defaults live in Default Settings > Cabinets with separate sets
for Base, Wall, Full Height, etc.; the Default Sets let a style (e.g., "Lincoln
Flat Panel") apply across all.

CB-21. Cabinet Schedule: columns number, type, width, depth, height, style,
count; appears via Tools > Schedules > Cabinet.

## B. Stairs

CB-22. Stair tools (Build > Stairs): Draw Stairs (Shift+Y), Straight Stairs,
L-Shaped, U-Shaped, Curve to Left, Curve to Right, Landing, Draw Ramp; related
Auto Stairwell, Flare/Curve Stairs, Add/Remove Stair Breakline.

CB-23. **Draw Stairs**: click-drag in plan from the bottom of the run to the top;
drag length plus floor height sets the number of risers (riser height tends to
7 3/4" default, tread 10"); an arrow shows the up direction and a number shows
the risers. Default width 36" (verify values in Chief).

CB-24. Stair width, riser height, tread depth, nosing, stringer sizes and
number of treads are interrelated: changing the number of risers recomputes riser
height from the floor-to-floor height; changing riser height recomputes risers.
Total rise equals the platform-to-platform height of the floors it connects.

CB-25. **Straight, L-shaped, U-shaped** stairs: the shape tool places the
correct run + landing (L: 90 degree turn on a landing; U: 180 degree with a landing
or winders). Landing size follows stair width.

CB-26. **Curve to Left/Right** and **Flare/Curve Stairs**: curved stairs bend
the run along an arc with a radius set in the dialog; Flare widens the bottom
tread(s) into an apron. Winder variants exist per corner.

CB-27. **Landings**: Landing tool draws a rectangular platform; landings join
runs; elevation of a landing is derived from the run risers below it.

CB-28. **Edit handles** on a selected stair: move, rotate, width handles at the
sides, length handle at the top end, handles at landings, and a "break line"
handle for the stair break symbol. Dragging the end changes the number of treads
while keeping riser height within code (7 3/4" max suggested).

CB-29. **Auto stairwell**: when a stair rises to an upper floor, Chief cuts a
stairwell hole in the upper floor platform where the stair passes through (ceiling
height of stairwell clear headroom ≥ 6'8" verified by the "Stairs" check) and
creates guard railings around the open edges. (verify in Chief for the exact
option names: "Auto Stairwell" menu command)

CB-30. The auto-generated stairwell is a **room** of function Stairwell/Open
Below with its own label ("STAIRWELL" or "Open to Below"). (verify in Chief)

CB-31. **Railings and balusters** are automatic on open stair sides (Left/Right
Railing options in the Stair dialog); newels, balusters and rail profile come
from Rail Style/Newels/Balusters tabs shared with railings and decks.

CB-32. **Stair Specification** tabs: General (width, risers/treads, stair type,
turn direction), Structure (stringers, treads, risers, nosing), Railing, Landing,
Materials, Label, Components, Schedule. (verify tab list)

CB-33. Stairs show in plan with tread lines, the up arrow with "UP"/"DN" text,
a break line where the cut plane occurs, and dashed hidden treads on other floors
when Reference Display is on. In 3D they are full geometry with railings.

CB-34. **Ramps**: Draw Ramp makes a sloped surface with slope check against the
1:12 ADA rule shown in the dialog (verify in Chief).

**Status of section B (Round 8, `plan-stairs` + `editor/stairs_view.rs`, `tools/stairs.rs`, `dialogs/stairs.rs`):**

| ID | Status |
|---|---|
| CB-22 | Done. Stairs flyout: Draw Stairs, Click Stairs, Straight Stairs, L-Shaped, U-Shaped, Curve to Left/Right (winders), Curved Stairs, Landing, Draw Ramp. Edit toolbar: Auto Stairwell, Flare/Curve Stairs, Add/Remove Stair Breakline, Make Railing. |
| CB-23, CB-24 | Done. Drag direction and length set the direction and the run; the riser count comes from the floor-to-floor rise (109 1/8" with the 7 3/4" maximum gives 15 risers). Lock Tread depth / Riser height / Number of treads and the Bottom and Top Height fields (`stairs_view::{set_total_rise, set_risers, set_bottom_height, set_top_height, fit_to_story}`). |
| CB-25 | Done. L and U stairs carry their landing; the winders option swaps the landing for pie treads (dialog check box). |
| CB-26 | Done for curved stairs: Curved Stairs takes a centre and a walking radius (`StairShape::Curved`), turns left or right, wedge treads, stepped stringers, rails along the arc, 6" inside-tread check. The flared apron of Flare/Curve Stairs is open (the command toggles winders). |
| CB-27 | Done. Landings are rectangles (drag) or polygons (click, click, double-click); a landing takes the height of the stair section that arrives on it and a section that starts on it begins there (`stairs_view::connect`). Ramps over 30" of rise get 60" landings between runs. |
| CB-28 | Done for move, rotate, run (a curved stair's run handle turns it further round) and width handles. A polygon landing only moves. No per-landing corner handles. |
| CB-29 | Done. Auto Stairwell cuts a `PlatformHole { kind: Floor, owner: stair }` in the floor above (the footprint grown by 1/20") and adds the divider walls; the hole and the walls follow a moved or reshaped stair and go with the stair on delete or undo. Guard railings around the opening are open. |
| CB-30 | Done as a room named "Stairwell" (type Stairwell, no floor under it) bounded by the invisible dividers. `RoomFunction::OpenBelow` is not used. |
| CB-31 | Done. Each side is None, Wall, Railing or Half Wall; newels, balusters (spacing from the clear-opening rule), top rail and bottom rail in 3D (`StairParams::{left_side, right_side, railing}`). A railing is not drawn across a landing in plan. |
| CB-32 | Done: Staircase Specification tabs General (with the solved result), Style, Newels/Balusters, Rails, Line Style, Fill Style, Materials, Label; Landing Specification: General, Line Style, Fill Style, Materials, Label. Components and Schedule tabs are open. |
| CB-33 | Done. Tread lines, UP arrow, riser count, break line at two thirds, DN arrow and the part beyond the break on the floor above; curved stairs and ramp landings have their own symbols; railings, walls and half-walls are drawn along the flights. Dashed hidden treads on other floors are open. |
| CB-34 | Done. Slope check against 1:12; rise over 30" gets landings. |

## C. Framing

CB-35. Framing tools: Build Framing (Shift+Cmd+S), Build All Framing, General
Framing, Post, Post with Footing, Blocking, Framing Reference Marker; floor/
ceiling framing (Joist, Joist Blocking, Joist Direction, Floor/Ceiling Beam,
Floor/Ceiling Truss, Bearing Line); roof framing (see `roofs.md` RF-52..57).

CB-36. **Build Framing dialog**: tabs/checkboxes for Floor Framing, Wall Framing,
Ceiling Framing, Roof Framing, Foundation framing and Structural Rebuild. Each
group has Auto Rebuild on/off. Framing is generated from the wall layers, floor
and ceiling platform definitions and the bearing lines.

CB-37. **Auto rebuild**: when on, any wall/floor/roof change that affects
framing rebuilds the *auto* members. Members edited manually are marked manual
and kept (Retain Wall Framing option in the wall dialog, captured).

CB-38. **Wall framing** produces studs, plates (bottom, double top), headers,
king/jack/trimmer studs, cripples, sills and corner/T-intersection backing by
the framing defaults (2x6 at 16" o.c. exterior, 2x4 interior). Stud layout starts
at a **Framing Reference Marker** or the wall start (Use Framing Reference
option, captured). Rollout offset and Reverse Rollout are wall options.

CB-39. **Floor framing**: joists at spacing (16" typical) spanning the shorter
direction between bearing lines, with rim joist (Automatic/Double/Single, wall
option), blocking and header at stair holes. **Joist Direction** tool overrides.

CB-40. **Wall detail views**: right-click a wall > Wall Detail (or View Wall
Detail from Exterior) opens an elevation-style framing view of the wall with
members dimensioned. **Cross Section** views also show framing.

CB-41. **Framing Schedule** lists framing members by size, length and count;
**Materials List** (Tools > Materials List > Create) aggregates framing, sheathing,
drywall, roofing, doors/windows, cabinets, with waste factors and optional costs;
exports to CSV/PDF.

CB-42. Structural calculation tools (Calculate Structural Materials for Deck)
size beams and joists to span tables; out of scope until Phase 3 (verify in
Chief).

## D. Terrain

CB-43. Workflow: **Create Terrain Perimeter** (draw closed polyline around the
lot; dialog sets name/elevation), add **elevation data**, **Build Terrain**
(Terrain menu) generates a surface mesh from the perimeter and data, with the
building's foundation fitted automatically.

CB-44. **Elevation Data tools**: Elevation Line (polyline contour with a height),
Elevation Point (spot height), Elevation Region (polygon at one height), Elevation
Spline, and Terrain Break (a crease line in the surface).

CB-45. **Terrain modifiers**: Hill, Valley, Raised Region, Lowered Region, Flat
Region (Cut/Fill) - polygonal or spline areas that add or remove height. The Flat
Region (Cut/Fill) around a building makes a level pad.

CB-46. **Terrain features**: Rectangular, Kidney, Spline features and Terrain
Hole - overlays with their own materials and heights (pond, patio, planter).

CB-47. **Make Terrain Hole Around Building(s)** cuts the terrain at the
foundation perimeter so the basement is visible; Place/Remove Terrain Elevation
Reference Point sets the datum.

CB-48. **Roads, Driveways, Sidewalks**: polyline/spline surfaces that follow the
terrain with width, thickness, material and curb options; Driveway polyline and
spline variants; Stepping Stone. They project onto terrain height in 3D.

CB-49. **Garden Beds, Grass Regions, Water Features**: polygonal/spline regions
with fill and 3D material; Water Feature has depth.

CB-50. **Plants and Sprinklers**: Plant tool places a library plant (symbol +
3D model) at a click with auto elevation from terrain and a growth size; Sprinkler
tool places heads with radius/arc spray shown in plan.

CB-51. **Terrain Specification** (Terrain > Terrain Specification): elevation
(sea level or relative), terrain type (Contour, Elevation Point), grid spacing,
contour interval, materials (grass, dirt), display of contours in plan.
(verify in Chief)

CB-52. Terrain is a view attribute: contours/elevation data are drawn in plan on
the Terrain layer; the 3D view shows the surface when the camera's Display
options include terrain.

## E. Library Browser

CB-53. **Library Browser** (Cmd+L, right-edge toggle) docks in a side panel: a
category **tree**, a search field, a results grid with previews, and a preview
pane; panel remembers expanded nodes. Core Catalog, Bonus catalogs, Manufacturer
catalogs and User Library are separate roots.

CB-54. **Search** matches names and keywords (and tags) with all search terms,
ranks by relevance, and supports filters: by Type (Cabinets, Doors, Windows,
Furniture, Fixtures, Plants...), Style, Manufacturer, and "Show only library
items in the current plan". (verify filter list in Chief)

CB-55. **Placing a symbol**: select an item, then **click in plan** to place a
copy (or drag from the browser into plan). The tool stays active for repeats.
The item is created on its default layer, with its default elevation and size.

CB-56. **Auto-rotate to walls**: wall-mounted items (toilet, sink, wall-hung
fixtures, appliances) rotate to face away from the nearest wall and snap flush
when within a snap distance; free-standing items keep the angle; ceiling items
center on cursor. Placement type is part of the catalog item.

CB-57. **Replace From Library**: select one or more placed objects, pick a
library item, then Replace: the new object takes the old one's position,
rotation and (optionally) size, and keeps connections (electrical, cabinet
inserts). (verify in Chief)

CB-58. **User Library / User Catalog**: any selected object can be added to the
user library ("Add to Library"); user items persist across plans and can be
shared as `.calib` files.

CB-59. **Import** (Library > Import Library, .calib/.calibz): adds a catalog
archive to the Library Browser (Core, Bonus, Manufacturer). Get Additional
Content downloads catalogs; Update Library Catalogs refreshes. Installing
core content requires an internet connection. (X18 menu items, captured)

CB-60. Library items carry: 2D plan symbol, 3D model (with materials), default
size/elevation, placement type, layer, manufacturer data, keywords; edited via
Symbol Specification (Tools > Symbol).

CB-61. **Library Browser display**: toggle between list and thumbnail views,
"Favorites/Recents"; double-click on a library item in the browser may open
its preview/edit dialog.

## F. Electrical

CB-62. Electrical tools: 110V Outlet (E,O), 220V Outlet, GFCI Outlet, Light
(E,L), Rope Light, Switch (E,S), Electrical Connection (E,C), Auto Place Outlets
(E,A,O).

CB-63. **Outlet placement**: click on a wall; the outlet snaps to the nearest
wall and rotates its symbol to face into the room; default height 12" above
finished floor (outlets) and 48" (switches), Custom heights from Electrical
Defaults.

CB-64. **Auto Place Outlets**: places outlets along walls using the electrical
code rule of thumb: no point along the wall is more than **6 feet** from an
outlet, so outlets are at most **12 feet** apart; every wall wider than **2 feet**
gets an outlet; outlets are kept at least **2 feet** (verify in Chief; may be
1 foot) clear of door/window openings. Kitchen counters get an outlet at least
every **4 feet** of counter, bathrooms get GFCI outlets near the sink,
garages/exterior doors get exterior/GFCI outlets.

CB-65. **Switch placement**: click near a door's latch side to place a switch
on the wall about **48"** above the floor; Chief can auto-connect the switch to
the room's lights when placed in the same room.

CB-66. **Light**: ceiling lights placed by click (ceiling, recessed, pendant,
wall sconce from the library); lights have lumens and a fixture type. Add Lights
in 3D uses these fixtures.

CB-67. **Connect Electrical**: click-drag from a switch (or outlet) to a light
(or fixture) creates a **connection arc** (dashed curved line); click-drag the
arc midpoint bends it; the arc is an object on layer "Electrical Connection".
Connections define switch-controlled loads and are listed in the Electrical
Schedule.

CB-68. **Electrical Schedule** counts outlets, switches and lights by type per
room/floor; Auto Place Outlets results are editable afterward (outlets are normal
objects).

## G. Plan Studio today

All five areas now exist as engine crates with editor tools. Cabinets (`plan-cabinets`): six kinds with a Cabinet Specification and face tree; fillers, custom countertops and 3D library models are open. Stairs (`plan-stairs`): straight, click, L, U, winder, curved, landing (rectangle or polygon) and ramp tools, an IRC solver, the Staircase and Landing Specifications, railings, walls and half-walls on the stair sides, open/closed risers and stringer styles, landings that join stair sections, and Auto Stairwell with the hole in the floor platform above (Round 8; see the status table in section B); guard railings around the stairwell opening and the flared apron are open. Framing (`plan-framing`): Build > Framing builds wall, floor and roof framing for the active floor or all floors, draws it on the Framing layer and offers a Framing Takeoff with CSV export; the manual framing tools and framing in 3D are open. Terrain (`plan-terrain`): perimeter, elevation data, modifiers, features, roads and Build Terrain with contours; terrain in 3D is open. Library: the Library Browser searches the built-in catalog (about 145 2D symbols) and places symbols; `plan-calib` reads Chief `.calib` catalogs but the browser does not show them yet, and there is no user library, Replace From Library or 3D symbol model. Electrical (`plan-electrical`) has devices and Auto Place Outlets; the circuits UI is open. `materials_list` still takes off studs, plates, drywall, sheathing, siding, flooring, ceiling drywall, doors and windows. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## H. Gap table

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| CB-2, CB-3 Click-to-place cabinets with auto-rotation to nearest wall | No cabinet object or tool | Critical | `Cabinet` struct in plan-cabinets (pos, rotation, w/d/h, type, elevation, faces); tool using `joins` wall faces for nearest-wall snap |
| CB-4 Bumping/Pushing to neighbors | No snap system for objects | High | Neighbor-edge snapping + Bumping/Pushing setting in snap settings |
| CB-5 Auto-fill width between walls/cabinets | Missing | Med | Compute gap by ray to walls/cabinets on both sides; offer on click with modifier |
| CB-7 Cabinet Specification dialog (14 tabs) | Dialogs doc only | High | Build on shared frame; start with General, Box, Front/Sides/Back |
| CB-8, CB-9 Resize/rotate handles and temp dimensions | Missing | High | Reuse handle framework from wall editing; width/depth snaps |
| CB-10, CB-11 Face item tree (layout/door/drawer/shelf/appliance), Equalize | Missing | Critical | Recursive `FaceItem` enum with auto-resize solver; 3D generator for doors/drawers |
| CB-13 Auto labels (B24, W3030...) | None | Med | Label formatter from type and size; schedule number |
| CB-14, CB-15 Countertop auto-join and Custom Countertop | None | High | Merge touching top rectangles into polygon with `i_overlay`; offset overhang; polyline tool |
| CB-16 Appliances inserted into cabinets, counter cut-outs | Library has Countertop placement flag only | High | Drop-on-cabinet logic converting face to Appliance item; boolean cut on counter |
| CB-17..CB-19 Soffit, Shelf, Partition, Fillers | Toolbar placeholders | Med | Simple extruded objects with spec dialogs |
| CB-21 Cabinet Schedule | Schedules for door/window/room/wall only | Med | Add `cabinet_schedule` in plan-docs |
| CB-22..CB-25 Draw Stairs + riser/tread solver, L/U shapes | Done (Round 8: solver, locks, heights, Click Stairs) | Critical | `Stair` object, rise/run solver (7 3/4" / 10" defaults), run builders; plan symbol |
| CB-26 Curved stairs, flare, winders | Curved Stairs and winders done; flare apron open | Med | Arc run generator after straight/L/U |
| CB-27, CB-28 Landings and edit handles | Done (rectangle and polygon landings join sections; move/rotate/run/width handles) | High | Landing object; handle set for width/length/rotate |
| CB-29, CB-30 Auto stairwell hole, guard rails, stairwell room | Hole in the floor platform and Stairwell room done (QA-04); guard rails around the opening open | High | Cut floor platform on upper floor; generate railing; link with `RoomFunction::OpenBelow` (rooms-floors.md R-40) |
| CB-31 Railings from Rail Style; baluster/newel | Done on stairs (none/wall/railing/half wall per side); decks use `deck_edge_railing` | High | Railing object shared by stairs, decks, balconies |
| CB-34 Ramps with slope check | Done, with landings every 30" of rise | Low | Slope check against 1:12 |
| CB-36, CB-37 Build Framing dialog, auto rebuild, manual retention | plan-framing empty; `materials_list` approximates studs | High | Real member model: plates, studs, headers; auto/manual flag |
| CB-38 Stud layout from Framing Reference Marker, rollout | Studs counted by formula only | High | Layout generator keyed to reference marker; kings/jacks per opening |
| CB-39 Floor framing, joist direction, rim joist | Missing | High | Joist generator per room/bearing lines |
| CB-40 Wall detail views | Missing | Med | 2D elevation generator from wall framing; depends on plan-elevation |
| CB-41 Framing schedule and full Materials List with waste, cost, roofing | Basic take-off, CSV export | Med | Extend `MaterialLine` with waste factor, unit price, roof/framing/cabinet lines; PDF export |
| CB-43..CB-47 Terrain perimeter, elevation data, Build Terrain, modifiers, hole around building | No terrain code | Critical | New `plan-terrain` module: contour/point interpolation (TIN via Delaunay), mesh builder, building pad cut/fill |
| CB-48, CB-49 Roads, driveways, sidewalks, garden beds, grass | Missing | Med | Polyline/spline surface objects draped on terrain mesh |
| CB-50 Plants and sprinklers | Library has no plants | Med | Add plants category and terrain-elevation placement |
| CB-53, CB-54 Library Browser panel with tree/search/filters/preview | Crate API only (tree, search); no panel | Critical | egui side panel using `Library::tree` and `search`; add Type/Manufacturer filter |
| CB-55, CB-56 Click-to-place symbols with auto-rotate to walls | `Placement` enum exists; no placement code | Critical | Tool using nearest wall normal; origin convention already defined (back-center) |
| CB-57 Replace From Library | Missing | Med | Command preserving transform and linked objects |
| CB-58 User library, Add to Library | Missing | High | Writeable user `Catalog` JSON in app data dir |
| CB-59 Import .calib/.calibz, Get Additional Content | `plan-import` empty; format is JSON | High | `.calib` is a proprietary archive: implement clean-room reader only if format documented; otherwise support Plan Studio JSON catalogs and glTF |
| CB-60 3D models in library items | `model3d` field reserved, unused | High | glTF loading via plan-3d; render in 3D and in plan symbol |
| CB-62, CB-63 Outlet/switch/light tools with wall snap, heights | Toolbar stubs | High | Electrical objects (wall-hosted) with height defaults |
| CB-64 Auto Place Outlets (12' spacing, 2' from openings, kitchen 4', GFCI wet rooms) | Missing | High | Rule engine over walls, openings and room types; unit tests per rule |
| CB-65 Switch placement near door, auto-connect to room lights | Missing | Med | Latch-side detection from door swing; connect to lights in same room |
| CB-67 Electrical Connection arcs | Missing | Med | Connection object with Bezier arc; electrical schedule |
