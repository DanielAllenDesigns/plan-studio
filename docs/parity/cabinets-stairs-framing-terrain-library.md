# Parity spec: Cabinets, Stairs, Framing, Terrain, Library, Electrical (Chief X18)

> Status (2026-10-08, re-audited against HEAD 4c11b69): 68 ids: 50 Works, 16 Partial, 1 Missing, 1 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.

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

**Status of section A, round 14 (`plan-cabinets`, `dialogs/cabinet.rs`, `tools/cabinet.rs`, `editor/placed.rs`, `tools/library/door_styles.rs`):**

| ID | Status |
|---|---|
| CB-4 | Done. Edit toolbar (Cabinet tool) cycles Bump (default: stop butted against the run, back lines aligned), Push (the overlapped cabinets of the run move ahead of the dragged one, chained; a wall or another run in the way makes it bump instead; dragging back lets them return; one undo step) and Pass Through. `plan_cabinets::push_run`, `tools::cabinet::{BumpMode, apply_edit_mode}`; tests `push::tests::*`, `pushing_moves_the_run_ahead_of_a_dragged_cabinet_and_lets_it_back`, `a_wall_that_stops_the_pushed_run_makes_the_cabinet_bump_instead`, `bump_mode_stops_and_off_passes_through`, `the_edit_toolbar_cycles_the_bumping_mode`, scenario `s31_cab_stairs_r14`. A width or corner handle dragged within 4" of a wall or the next cabinet snaps to it (the cabinet fills the gap): `a_resized_edge_dragged_near_a_wall_or_cabinet_snaps_to_it`. Verify in Chief: default mode, where the control lives (Edit > Snap Settings has no cabinet row yet), snap reach. The Select tool still bumps. |
| CB-7 | Done. All 14 tabs are live. Fill Style (pattern, colour, opacity, spacing; plan only, `placed::draw_cabinet_fill`), Components (`plan_cabinets::components`: parts, counts, sizes, materials), Object Information (facts plus manufacturer, model, description, notes), Schedule (include in the Cabinet Schedule; `schedule_kinds` skips a cabinet that is out), Accessories (front pilasters, feet, finished end panels; 3D in `mesh_extra.rs`). Tests `every_tab_of_the_specification_is_live_and_the_new_ones_show_their_content`, `components_follow_the_accessories_and_the_face`, `dress::tests::*`, `a_cabinet_switched_out_of_the_schedule_is_not_listed`. Verify in Chief: the Accessories lists (Chief's are longer) and the Object Information fields. |
| CB-12 | Done for the library hookup. Door Panel and Drawer Panel Main Style list the built-in styles, then the library objects under "Cabinet Doors" / "Cabinet Drawers" (built-in, user library, and Chief catalogs on request: "Load Chief Library Styles", names and categories only). The pick copies its name, look (Slab / Shaker / Raised, glass) and library id into the cabinet (`DoorStyle::library`, `DrawerStyle::library`); 3D builds from the look, not from the object's own geometry. `tools::library::door_styles`; tests `door_styles::tests::*`, `a_library_style_pick_copies_its_look_and_a_built_in_pick_clears_it`. Verify in Chief: category names and what Library... / Edit... open. |
| CB-17 | Done. The Soffit Polygon tool draws one by clicks (earlier rounds); Convert Polyline to Soffit (Edit toolbar of the Cabinet tool, `cabinet.soffit_from_polyline`) turns a selected closed CAD polyline into a polygon soffit with the Soffit tool's height and elevation, replacing the polyline, one undo step: `a_closed_polyline_becomes_a_polygon_soffit_in_one_undo_step`, scenario `a_closed_polyline_becomes_a_soffit_from_the_edit_toolbar`. The Select tool's toolbar does not offer it yet (integration queue). |

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
| CB-26 | Done for curved stairs: Curved Stairs takes a centre and a walking radius (`StairShape::Curved`), turns left or right, wedge treads, stepped stringers, rails along the arc, 6" inside-tread check. **Spiral Stairs** (flyout entry): click the centre, drag to the outside radius; wedge treads round a centre pole (`StairParams::spiral`), judged by the spiral code (9 1/2" risers, 6 3/4" treads at the walking line, 26" clear width, 78" headroom; plan-check rule R311.7.10.1). **Flared bottom tread** (`StairParams::flare`, Style tab): the bottom tread reaches past the stair in half-round ends, in plan and in 3D. **Bullnose bottom tread** (round 14, `StairParams::bullnose`, Style tab: None, Left End, Right End, Both Ends): the chosen end is a half-round of (tread + nosing) / 2 and wins over the flare on that end, in plan, 3D and the Components tab (`StairParams::apron_reach`, `layout::apron_outline`; test `a_bullnose_rounds_the_chosen_ends_of_the_bottom_tread`). The Flare/Curve Stairs command still toggles winders. Verify in Chief: its apron shapes. |
| CB-27 | Done. Landings are rectangles (drag) or polygons (click, click, double-click); a landing takes the height of the stair section that arrives on it and a section that starts on it begins there (`stairs_view::connect`). Ramps over 30" of rise get 60" landings between runs. |
| CB-28 | Done for move, rotate, run (a curved stair's run handle turns it further round) and width handles. A polygon landing only moves. No per-landing corner handles. |
| CB-29 | Done. Auto Stairwell cuts a `PlatformHole { kind: Floor, owner: stair }` in the floor above (the footprint grown by 1/20") and adds the divider walls; the hole and the walls follow a moved or reshaped stair and go with the stair on delete or undo. The guard railing around the opening (every side but the one the stair arrives at) is the Line Style tab check box, made as railing walls on the floor above. Round 14: each stair side is None, Wall, Railing (a guard with newels and balusters), Half Wall or **Handrail** (a wall rail only, `SideKind::Handrail`, `is_guard()` false), and the Newels/Balusters and Rails tabs edit **both sides or one side alone** (`left_railing` / `right_railing`, `StairParams::railing_for`; plan, 3D, Components and landings follow); tests `a_handrail_side_is_a_rail_on_the_wall_and_not_a_guard`, `each_side_can_have_its_own_newels_and_balusters`, `the_rails_tabs_edit_one_side_without_touching_the_other`. Plan-check's handrail rule does not know the Handrail side (integration queue). |
| CB-30 | Done as a room named "Stairwell" (type Stairwell, no floor under it) bounded by the invisible dividers. `RoomFunction::OpenBelow` is not used. |
| CB-31 | Done. Each side is None, Wall, Railing or Half Wall; newels, balusters (spacing from the clear-opening rule), top rail and bottom rail in 3D (`StairParams::{left_side, right_side, railing}`). The rail carries across the landing or turn of an L, U or winder stair in plan (double line, newel squares at the corners), and a Landing object guards its open left and right sides with a railing or half wall (3D and plan; the arrival ends stay open; a full wall is not drawn on a landing). |
| CB-32 | Done: Staircase Specification tabs General (with the solved result and the spiral check box), Style (with the flared and bullnose bottom tread), Newels/Balusters and Rails (with the Applies To side row), Line Style (with the break line position), Fill Style, Materials, Components (treads, risers, stringers, landings, newels, balusters, rails, handrails, with counts, sizes and materials), **Schedule** (the stair's row of the Stair Schedule: type, treads, risers, riser height, tread depth, total rise, total run, width, headroom), Label; Landing Specification: General, Rails, Line Style, Fill Style, Materials, Label. The Stair Schedule (`ScheduleKind::Stair`) has Chief's columns Treads, Risers, Riser height, Tread depth, Total rise, Total run, Width and Headroom (`STAIR_FIELDS`; tests `the_stair_schedule_lists_stairs_but_not_landings`, `the_schedule_tab_shows_the_row_the_stair_has_in_the_stair_schedule`). |
| CB-33 | Done. Tread lines, the UP arrow with Chief's label (`UP 15R @ 7 3/4"`, riser count and height to the eighth), break line at two thirds of the run by default and movable per stair (Line Style tab), DN arrow and the part beyond the break on the floor above (dashed), the treads beyond the break dashed on the stair's own floor; round 14: a stair that opens into the floor above (its stairwell hole, or an Open Below room over it) also shows the treads below the break there, dashed and lighter (`stairs_view::{open_to_floor_above, well_strokes}`; tests `the_treads_below_the_break_show_dashed_on_the_floor_above_through_the_stairwell`, `an_open_below_room_above_the_stair_counts_as_an_opening_too`). Curved stairs and ramp landings have their own symbols; railings, walls, half-walls and handrails are drawn along the flights and across the landings. Stairs are meshed into the 3D view scene, so sections cut through them (the cut treads show as Cut edges and regions). |
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

**Status, round 14 (framing).** CB-35 Works. CB-36 Partial: Build > Framing > Build Framing... opens the Build Framing dialog (tabs Floor, Ceiling, Roof, Wall, Posts, Trusses, Framing Defaults; Build, Auto rebuild and Retain existing per group; OK saves the options and builds as one undo step); the flyout item and the hotkey still build directly. CB-37 Partial: `BuildOptions` keeps frozen groups and retained walls through a rebuild and `framing_view::auto_rebuild` rebuilds the changed automatic groups; the per-frame call and the wall dialog's Retain Wall Framing check box are queued. CB-38 Works (Retain Wall Framing per wall). CB-39 Works (single or double rim, joist direction, exterior-and-Bearing-Line bearing, ceiling joists, Open Below). CB-40 Partial (`wall_detail_dims` and `paint_wall_detail`; no window hosts them yet; `overview_scene` for the FramingOverview camera). CB-41 Works. CB-42 Partial (joist and rafter span check only). Evidence is in `docs/parity-status.md` and `docs/manual/19-framing.md`; the dash patterns of the plan line styles and the dialog layout are marked verify in Chief (DECISIONS 112..115).

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

All five areas now exist as engine crates with editor tools. Cabinets (`plan-cabinets`): six kinds with a Cabinet Specification and face tree; fillers, custom countertops and 3D library models are open. Stairs (`plan-stairs`): straight, click, L, U, winder, curved, landing (rectangle or polygon) and ramp tools, an IRC solver, the Staircase and Landing Specifications, railings, walls and half-walls on the stair sides, open/closed risers and stringer styles, landings that join stair sections, and Auto Stairwell with the hole in the floor platform above (Round 8; see the status table in section B); guard railings around the stairwell opening and the flared apron are open. Framing (`plan-framing`): Build > Framing builds wall, floor and roof framing for the active floor or all floors, draws it on the Framing layer and offers a Framing Takeoff with CSV export; the manual framing tools and framing in 3D are open. Terrain (`plan-terrain`): perimeter, elevation data (drag to move, spline tension), modifiers, features as cut/fill pads, terrain walls and curbs that cut the surface, the building pad under the house, a cut/fill report in cubic yards, roads with crowns and curbs, landscape objects (editable kidney and spline outlines, water ripple), contours with labels along the major lines, North Pointer and Scale Bar, and Build Terrain with contours, in plan and in 3D; sprinkler supply connections and the Materials page of the Terrain Specification are open. Library: the Library Browser searches the built-in catalog (about 145 2D symbols) and places symbols; `plan-calib` reads Chief `.calib` catalogs but the browser does not show them yet, and there is no user library, Replace From Library or 3D symbol model. Electrical (`plan-electrical`) has devices and Auto Place Outlets; the circuits UI is open. `materials_list` still takes off studs, plates, drywall, sheathing, siding, flooring, ceiling drywall, doors and windows. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

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
| CB-26 Curved stairs, flare, winders | Curved Stairs, winders, Spiral Stairs and the flared bottom tread done | Med | Arc run generator after straight/L/U |
| CB-27, CB-28 Landings and edit handles | Done (rectangle and polygon landings join sections; move/rotate/run/width handles) | High | Landing object; handle set for width/length/rotate |
| CB-29, CB-30 Auto stairwell hole, guard rails, stairwell room | Hole in the floor platform, Stairwell room and the guard rails around the opening done (QA-04) | High | Cut floor platform on upper floor; generate railing; link with `RoomFunction::OpenBelow` (rooms-floors.md R-40) |
| CB-31 Railings from Rail Style; baluster/newel | Done on stairs (none/wall/railing/half wall per side); decks use `deck_edge_railing` | High | Railing object shared by stairs, decks, balconies |
| CB-34 Ramps with slope check | Done, with landings every 30" of rise | Low | Slope check against 1:12 |
| CB-36, CB-37 Build Framing dialog, auto rebuild, manual retention | Round 14: dialog, per-group Build/Auto/Retain, Retain Wall Framing, auto rebuild function; the per-frame call and the wall dialog check box are queued | Low | Shell hook (main.rs) and Wall Specification Framing tab |
| CB-38 Stud layout from Framing Reference Marker, rollout | Studs counted by formula only | High | Layout generator keyed to reference marker; kings/jacks per opening |
| CB-39 Floor framing, joist direction, rim joist | Round 14: done (direction, double rim, bearing mode, ceiling joists, Open Below); directed floors skip stairwell holes | Low | Holes in `frame_floor_directed` |
| CB-40 Wall detail views | Round 14: dimensions and painter exist; no window or command hosts them | Med | A Wall Detail window from the wall's context menu |
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

<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| CB-69 | Elevator (listed in the toolbar capture): Elevator cab and shaft that cuts the floor platforms and opens on landings. | Missing | no elevator tool or symbol (grep Elevator in crates finds nothing) |
| CB-70 | Corner Base Cabinet: L-shaped corner units with lazy-susan or blind options. Also covers: Corner Wall Cabinet. (Not captured; verify in Chief.) | Works | toolbar.rs cabinet() "Corner Base Cabinet", "Corner Wall Cabinet"; plan-cabinets kinds |
| CB-71 | Blind Base Cabinet: Cabinets with a blind corner section. Also covers: Blind Wall Cabinet. (Not captured; verify in Chief.) | Works | toolbar.rs cabinet() "Blind Base Cabinet", "Blind Wall Cabinet" |
| CB-72 | 3D Solid Feature (custom molding profile): Draw a 2D profile and extrude or sweep it into a 3D solid or molding. | Partial | tools/images.rs ImageMode::SolidFeature; no profile editor (parity-status "Details" open items) |
| CB-73 | Plant Chooser (Plant Chooser dialog): Browse plants by name, type, size, hardiness zone and sun; place with a drawn canopy. (Not captured; verify in Chief.) | Partial | dialogs/terrain/object.rs "Plant Chooser" window over plan-library plant catalog (catalog_plants.rs); no hardiness zone, sun or water filter |
| CB-74 | Hardiness zones / plant growth size by age (not captured): Plant data carries USDA zone and growth size over years, used by the Plant Chooser filter. (Not captured; verify in Chief.) | Missing | no hardiness or growth-age data (grep finds nothing) |
| CB-75 | Import terrain data (DXF points, text, GPX): Load survey points from a DXF/CSV/text file into elevation points. Also covers: Import terrain data (survey points). (Not captured; verify in Chief.) | Works | plan-terrain import.rs; toolbar.rs site_objects() "Import Terrain Data…" |
| CB-76 | Terrain Cut and Fill Report: Cut and fill volumes between existing and finished grade. (Not captured; verify in Chief.) | Works | plan-terrain; dialogs/terrain.rs REPORT_TABS; plan-docs terrain_report.rs |
| CB-77 | Building Pad: Flat pad under the building at the first-floor elevation. (Not captured; verify in Chief.) | Works | toolbar.rs site_objects() "Building Pad"; dialogs/terrain.rs Building Pad tab |
| CB-78 | Get Additional Content…: Download extra library catalogs, bonus catalogs and manufacturer content from Chief's servers. | Missing | not in menus.rs (needs Chief's online content service) |
| CB-79 | Install Core Content: Install the Core Library catalogs from the installer. | Differs-by-design | Plan Studio reads the user's installed Chief catalogs at runtime and ships its own starter catalog; it never installs or bundles Chief content (plan-calib, plan-library starter.rs) |
| CB-80 | Update Library Catalogs: Refresh the catalog index and pull catalog updates. | Partial | Library > Catalog Settings… rescans the folder; no automatic update check (CB-53) |
| CB-81 | Import 3D Model (OBJ, glTF; also STL, 3DS, DAE, see CB-82): Bring an outside 3D model in as a library symbol (Chief imports 3DS, OBJ, SKP, DAE, STL via Import 3D Symbol). Also covers: Import 3D symbols OBJ / glTF. (Not captured; verify in Chief.) | Works | menus.rs; tools/library/make.rs; plan-import obj.rs, gltf.rs |
| CB-82 | Import 3D Model (3DS, SKP, DAE, STL formats) (not captured): Import 3DS, SketchUp SKP, COLLADA DAE and STL meshes into the library. Also covers: Import 3DS / SketchUp SKP / COLLADA DAE / STL. (Not captured; verify in Chief.) | Partial | STL, 3DS and COLLADA work through File > Import > 3D Symbol and the Library Browser import: plan-import stl.rs, tds.rs, dae.rs, xml.rs, formats.rs (units/up-axis guess), shape.rs (facing, resize); dialogs/symbol/import3d.rs (Name, Category, Scale units, Up axis, Symbol faces direction, size, Placement, Elevation, Layer, Materials mapped to plan-materials); tests stl::tests, tds::tests, dae::tests, formats::tests, shape::tests, import3d::tests, scenarios s43 (an_stl_in_millimeters_is_imported_and_placed, a_3ds_keeps_its_object_and_maps_the_texture_to_a_plan_material, a_collada_file_brings_its_own_units_axis_and_transform, sketchup_files_say_how_to_export). SketchUp .skp is not readable without SketchUp's SDK: the dialog says "Export from SketchUp as COLLADA (.dae) or OBJ". Verify in Chief: dialog layout and wording, texture images (only a color per part is kept), origin point choices |
| CB-83 | Manufacturer catalogs, 3D Warehouse, bonus catalogs (not captured): Product catalogs from manufacturers and the SketchUp 3D Warehouse inside the Library Browser. (Not captured; verify in Chief.) | Missing | no manufacturer content service; 3D Warehouse login is out of scope |
| CB-84 | North Pointer: Plan symbol that points north and sets the sun's compass bearing. | Works | toolbar.rs site_objects() "North Pointer"; plan-terrain site_symbols.rs; dialogs/terrain.rs North angle |
| CB-85 | 3D Solid defaults: Default material, size and layer for 3D solids. | Missing | not in the Default Settings tree (dialogs/defaults.rs TREE); a placed solid has its own dialog (dialogs/symbol.rs) |
| CB-86 | Deck framing and decking boards (planking): Deck surface boards, joists, beams, ledger and footings built from the deck outline. Also covers: Build Deck Framing. (Not captured; verify in Chief.) | Works (verify in Chief) | a deck room carries a Deck Specification (`plan_core::deck::DeckSpec`, Deck tab of dialogs/room.rs): planking (`deck::plank_layout`: board width, gap, direction, picture frame border; plan-3d `deck.rs deck_meshes`, plan lines in `editor/fireplace_view/deck.rs draw`), framing (`plan_framing::deck::build_deck_framing`: joists, rim joists, ledger at the house wall, beams, posts with footings; Build Framing > Deck, `fireplace_view::deck::build_framing`) and stairs to grade; tests `deck.rs` (plan-core, plan-3d, plan-framing, editor), scenarios s44 `build_deck_framing_makes_the_members_and_replaces_the_skirt`, `a_deck_room_has_planking_in_3d_and_in_the_plan`. Gap: railing balusters follow the wall's own rail style; no structural calculator (CB-42); a deck on an upper floor has no posts below it |
| CB-87 | Chimneys and fireplaces: Chimney and fireplace tools that build the firebox, flue, chase and cap through roof and floors. (Not captured; verify in Chief.) | Works (verify in Chief) | Fireplace tools (Build > Fireplace: Fireplace, Fireplace in Wall, Prefab Fireplace, Chimney; `tools/fireplace.rs`), a Fireplace Specification with General, Hearth, Mantel, Chimney, Materials, Label and Layer tabs (`dialogs/fireplace.rs`), the record in `plan_core::fireplace`, 3D body, firebox recess, hearth, mantel, shaft, cap and flashing (`plan_3d::fireplace`), the 3-2-10 height rule, the roof cut around the shaft (`roof_view floor_roof_meshes_in`), a chase through the floors above (plan and platform holes), the wall cut of an in-wall fireplace; tests in `plan-core/src/fireplace.rs`, `plan-3d/src/fireplace.rs`, `tools/fireplace.rs`, `dialogs/fireplace.rs`, scenarios s44 `a_chimney_tops_out_three_feet_over_the_roof_and_cuts_a_hole_in_it`, `the_chimney_chase_runs_up_through_the_floors_above_and_cuts_their_platforms`. Gap: no cricket; the chase does not cut walls; Select-tool double-click still opens the Symbol dialog (integration queue) |
| CB-88 | Plumbing and HVAC symbols and connections: Plumbing fixtures with pipe connections, HVAC registers and duct runs on their own layers. (Not captured; verify in Chief.) | Partial | plumbing and HVAC symbols come from the library; there are no supply/drain/duct connection objects (only electrical connections exist) |
| CB-89 | Kitchen and Bath design checks (NKBA clearances): Check kitchen work triangle, clearances and counter lengths against NKBA guidelines. (Not captured; verify in Chief.) | Partial | Plan Check covers IRC bath clearances (R307) and NKBA kitchen aisle width and counter depth (plan-check rules_fixtures.rs); work triangle, landing space and ventilation are not checked |
<!-- coverage-audit:end -->
