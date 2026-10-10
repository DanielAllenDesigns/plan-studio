# Chapter 8: Roofs

Plan Studio builds roofs two ways, like Chief: automatically from the exterior
walls (Build Roof), or one plane at a time (Roof Plane). Both produce editable
**roof planes**, and both show in plan and in the 3D view. On top of the planes sit roof
holes, skylights, vaulted ceiling planes, dormers (including floating dormers) and roof returns, and you can join planes along their meeting line.

## 8.1 How roofs work

A roof plane is a flat polygon in 3D. Its first edge is the **eave baseline**, at the
top of a wall, and the plane rises from there at a pitch measured as rise per 12 of run
(8:12 is Daniel's default).

```
   Hip roof, plan view                Gable roof, plan view
   +----------------------+           +----------------------+
   |\\                    /|           |                      |
   | \\__________________/ |           |======== ridge =======|
   | /                  \\ |           |                      |
   |/____________________\\|           +----------------------+
    hips meet at the ridge             gable ends: the wall rises to the ridge
```

- **Pitch**: rise in 12. Allowed range 0.5 to 24.
- **Overhang**: how far the eave projects past the wall face; 16" by default.
- **Baseline height**: the elevation of the eave (the wall top, optionally raised off the plate).
- Roof data is stored in the floor's own typed `roofs` slot (a list of records tagged `plane`,
  `settings`, `ceiling`, `dormer` and `face`), so the roof saves, loads and undoes with the plan and its
  outlines export to DXF. Planes are drawn straight from the records on the `Roof Planes` layer; there
  are no outline polylines in the CAD objects. Files from before this change kept the roof as hidden
  `Roof Planes, Data` text records plus an outline polyline per plane; the program converts them
  once when it opens such a file (and removes the hidden layer), with no undo entry.
- Each plane remembers whether it is **automatic** (rebuilt when the walls change) or
  **manual** (kept when the roof is rebuilt).

### What the automatic roof does

`plan-roof` computes a roof from the building outline with a **weighted straight skeleton**:

1. The outline of the exterior walls is offset outward by each edge's overhang to find the eave.
2. Each edge moves inward as height grows, at a speed set by its pitch. Hip edges move;
   gable and shed edges stay vertical.
3. Events (an edge shrinks to nothing, a reflex corner hits an edge, a loop collapses) are
   processed in order, which produces hips, valleys and ridges.
4. Neighboring planes share their hip, ridge and valley vertices exactly, so the roof is
   watertight, and the sum of plane areas is checked against the eave polygon.

It handles rectangles, L, T, U, H, plus-shaped and stepped footprints, general simple polygons,
per-edge pitch and overhang, and gable or shed edges. When the exact solution fails (about 1% of
unusual pitch mixtures) the roof degrades to one uniform pitch, and if that fails too, to a hip roof
on the bounding rectangle. Limits: no holes in the footprint; one `build_roof` call per detached
building; fascia is 6" by default (a Roof Defaults size, 8.4a); an upper-pitch (gambrel, mansard) or Dutch gable roof has one break height for the whole roof and is built in two stages (below). A rare case (two parallel same-facing planes of different pitch,
or a hip next to a gable, on either side of a short jog) also falls back to the approximation; the
`plan-roof` README gives the sweep results.

**Rooms steer the roof** (Room Specification, chapter 4.4). A room with **Roof Over This Room** off is left out of the footprint: its exterior walls stop shaping the roof and a partition between it and a roofed room becomes the roof's edge (when that leaves no closed outline, the whole exterior is used); a roofless room that lies inside a plane gets a hole cut instead. A room with **Flat Roof Over This Room** on gets a level plane at its ceiling (`plan_roof::flat_roof_plane`) in place of the pitched roof, and the Flat Roof room type has no ceiling and a membrane deck. Auto Rebuild Roofs reruns when these flags change.

**Wings** (Round 14). When the pitched rooms of the floor you build over stand at different plate heights, or first-floor rooms are not covered by the floor above (a one-story garage beside a two-story house), each group gets its own roof at its own plate. An edge against a taller wall or an upper floor rises to it as a high shed with no overhang. Auto Rebuild Roofs also watches the walls of the floors below. A floor with wings and a Flat Roof or Roof Over This Room off room keeps one footprint at its tallest wall instead.

**Half hip** (Round 14). A Full Gable wall with an **Upper Pitch** is built as a half hip (a jerkinhead): the end wall rises vertically to the clip height (the wall's **Starts at Height**) and a short hip plane slopes up from there to the ridge. The Roof Styles button **Half Hip** in the Build Roof dialog sets this up.

## 8.2 Tools

### Roof Tools (row 2, Roof flyout; Build > Roof)

Each flyout entry starts the Roof tool in its mode. The tool also draws a small **palette** of mode
buttons in the top-left corner of the canvas (including Delete Roof Planes and Rebuild Roofs), so you
can change mode without going back to the flyout. Since Round 8 (QA-07) the active tool names itself after the mode ("Roof Plane",
"Build Roof", "Auto Dormer" ... as the flyout entry reads), so anything that shows the tool's name says which mode you are in; before that all twelve answered "Roof".

| Flyout entry | Hotkey | Mode | Today |
|---|---|---|---|
| Roof Plane | `Q` | Roof Plane | Works. |
| Build Roof | `Ctrl+Alt+Shift+Cmd+N` | Build Roof | Works: opens the Build Roof dialog. |
| Ceiling Plane | `Ctrl+Alt+Shift+Cmd+U` | Ceiling Plane | Works (8.2). |
| Gable/Roof Line | `Ctrl+Alt+Shift+Cmd+O` | Gable/Roof Line | Works. |
| Roof Hole | `Ctrl+Alt+Shift+Cmd+T` | Roof Hole | Works. |
| Skylight | `Ctrl+Alt+Shift+Cmd+S` | Skylight | Works. |
| Auto Dormer | `Ctrl+Alt+Shift+Cmd+Z` | Auto Dormer | Works: click a plane, then the Dormer Specification (8.6). |
| Auto Floating Dormer | `Ctrl+Alt+Shift+Cmd+R` | Auto Floating Dormer | Works, see 8.2. |
| Explode Dormer | | Explode Dormer | Works; the dormer's walls stay as real walls (8.2). |
| Roof Return | | Roof Return | Works. |
| Edit All Roof Planes | `Ctrl+Alt+Shift+Cmd+P` | Edit | Works (select, move, reshape planes). The flyout entry of this name starts Edit mode; the palette has **Edit Roof Planes** (Edit mode) and **Edit All Roof Planes** (the one-dialog mode, below) as two separate buttons. |
| Delete Roof Planes | `Ctrl+Alt+Shift+Cmd+W` | (command) | Works: removes every roof plane in the plan, on every floor, in one undo step. "There are no roof planes to delete" if none. |
| Delete Ceiling Planes | `Ctrl+Alt+Shift+Cmd+X` | (command) | Works: removes the ceiling planes of every floor in one undo step. "There are no ceiling planes to delete" if none. |
| Join Roof Planes | (Edit toolbar) | Join | Works, see 8.2. It is on the Edit toolbar (with a plane selected) and in the tool's palette, as in Chief, not in the flyout. |

### Roof Plane mode

1. Press and drag the **eave baseline** along the edge where the roof should start.
2. Click on the side the plane should **rise toward**.

A rectangular plane is made at the default pitch (8:12). Each plane is one undo step.

### Edit mode (select and reshape planes)

| Gesture | Result |
|---|---|
| Click a plane | Selects it. |
| Drag the plane | Moves it. |
| Drag a vertex | Reshapes it; the plane stays planar (the other vertices re-solve). |
| Drag an **edge handle** (a hollow square at the middle of an edge) | Moves that edge square to itself; the plane stays planar and keeps its pitch. Moving the baseline edge moves the eave. One undo step, "Move Roof Edge". |
| Drag the **pitch arrow** (the round handle on an arrow up the slope from the plane's center; the pitch is printed beside it) | Steepens the plane as you drag up the slope and flattens it as you drag down, 1/4 in 12 per inch of drag, in quarter steps between 1/4:12 and 24:12. One undo step, "Change Roof Pitch". |
| Drag the **rotate knob** (a ring below the middle of the eave) | Turns the whole plane about its center: outline, eave baseline, holes and heights. One undo step, "Rotate Roof Plane". |
| Double-click, `Enter` | Opens the Roof Plane Specification. |
| `Delete` | Removes the plane. |

Any of these marks the plane **manual**, so a later Build Roof leaves it alone. The handles belong to Edit All
Roof Planes; Select Objects shows the corner and move handles of a roof plane only.

### Edit All Roof Planes (the dialog mode, Round 14)

Pick **Edit All Roof Planes** in the palette in the top-left corner of the canvas and one dialog opens for every plane of the floor ("Edit All Roof Planes (n planes)"; "There are no roof planes to edit" when there are none). Every setting has a **Change** check box: only the settings you tick are written, to all planes at once, in one undo step ("Edit All Roof Planes"). Tabs: **General** (Pitch, Overhang; the automatic planes are rebuilt so hips and ridges follow), **Options** (Include Ridge Caps, Eave Cut, Rafter Tails, Fascia, Soffit, Frieze, Gutters), **Structure** (the Define Roof Structure window, below) and **Materials** (roofing material and layer). OK is refused with "Choose what to change" when nothing is ticked.

### Build Roof mode

Click once to open the **Build Roof** dialog (8.4). OK builds the automatic planes from the
exterior walls of the floor, using each wall's roof directive (Hip by default) and writes
them as editable planes. With **Auto Rebuild Roofs** on, a change to the walls that
affects the roof (a moved or added wall, a changed directive or height) rebuilds the automatic
planes; manual planes are not touched. A message tells you if no exterior outline exists.

A room with **Roof Over This Room** off (chapter 4.4) gets a hole in the roof. When the room sits across a ridge, hip or valley
and no single plane encloses it (Round 13), Build Roof cuts one hole piece per plane, each sitting 0.04" inside the
shared joint so the planes still meet. A **Flat Roof Over This Room** now has an overhang: the exterior edges overhang
by half the wall plus the wall's (or the roof settings') overhang, the partition edges none.

### Gable/Roof Line mode

Click near an **eave** to make that edge a gable end. For a plane made by Build Roof, the edge becomes a
gable end in the Build Roof edge settings and the roof is rebuilt ("That edge is now a gable end"). For a
manual plane, the roof is rebuilt from the planes' eave baselines (`plan_roof::apply_gable_line`); that needs the
planes to form one closed ring or one chain with a single straight gap, otherwise the status bar says
"Gable/Roof Line: <reason>". The new gable wall stands on the old eave line and the edge's overhang becomes the
rake.

Clicking an exterior wall (away from any eave) still flips it between **Hip** and **Full Gable**. A gable wall
rises to the ridge instead of carrying a plane, and its neighbors extend to the rake. The tool flips every wall
along the same footprint edge. The other wall directives (Dutch Gable, High Shed/Gable, Knee Wall,
Extend Slope Downward) are set on the Roof tab of the Wall Specification (8.7); High Shed/Gable is built as a shed edge.
Since Round 14 the Edit toolbar also sets the directive for a selection of walls (at least one exterior): **Hip Wall**, **Full Gable Wall**, **High Shed/Gable Wall**, **Knee Wall** and **Dutch Gable Wall**. The whole footprint edge the wall lies on follows (a Knee Wall is the wall itself only), it is one undo step named after the button, and with Auto Rebuild Roofs on the roof is rebuilt.

### Roof Hole and Skylight

- **Roof Hole**: press and drag a rectangle inside a plane to cut a hole (at least 6" on each side), or (Round 14) click the corners of any outline that does not cross itself and double-click to close it. A polygon that straddles a ridge, hip or valley is cut into one piece per plane.
- **Skylight**: press and drag a rectangle inside a plane, or just click to place a 24" x 48" one. The
  skylight gets a curb, a frame ring and a glass pane.

Each hole belongs to the plane under the center of its rectangle and must lie completely inside that
plane, away from its boundary and clear of earlier holes; otherwise the status bar says why. A feature that spans a ridge or hip (a
chimney) needs one hole per plane. Holes and skylights are listed on the Holes tab of the Roof Plane
Specification (8.5). The 3D view cuts the holes out of the plane slab and builds the skylight on top (8.3).

### Ceiling Plane mode

Draws a **vaulted ceiling plane** the way Roof Plane draws a roof plane: press and drag the baseline, then click
toward the high side. The plane starts at the floor's ceiling height and at the pitch in this floor's roof settings (4:12
when it has none), and sits on the `Ceiling Planes` layer. Its thickness is 9" below the surface. Each plane is one undo step ("Draw Ceiling Plane");
**Delete Ceiling Planes** clears them all. A ceiling plane is selected, moved and deleted like a roof plane (in Select Objects or Edit All Roof Planes), and
double-click or `Enter` opens the **Ceiling Plane Specification** (8.5).

**Build Ceiling Planes.** The Build Roof dialog's check box *Build ceiling planes for vaulted rooms* (8.4) makes ceiling planes for you: Build Roof adds one that follows the
roof (same pitch and eave baseline) over every room whose **Ceiling Over This Room** is turned off in the Room Specification, on the `Ceiling Planes` layer, and
replaces the ones it made before. Ceiling planes you drew by hand stay. Auto Rebuild reruns only when walls change, so after you change a room's ceiling flag, run Build Roof again.

### Auto Dormer and Explode Dormer

- **Auto Dormer**: click inside a roof plane. The click sets the dormer's place (centered at the click along the eave;
  the distance up the slope is the setback, at least 12") and the **Dormer Specification** opens (8.6); OK builds the
  dormer. "Auto Dormer: <reason>" is shown when it does not fit: too big, too close to the eave, past the ridge, a
  hip ridge that would collapse, or a flat plane. A dormer across two planes is not supported (a dormer you drag moves onto the plane the pointer is carried over, centered under it, if it still fits). Double-click a dormer with
  Edit All Roof Planes to change it; it follows its plane when Build Roof rebuilds the roof.
- A dormer has a front wall (parallel to the eave), two triangular cheek walls, its own roof planes (gable 2, shed 1,
  hip 3), a window in the front wall if you ask for one, and a hole in the main roof plane under it. Since Round 13
  its roof has an **overhang** (12" for a new dormer; the Roof tab of the Dormer Specification): the eaves and rakes
  project past the walls edge by edge while the ridges, hips and valleys keep their lines, and the 3D view builds
  **fascia, rake boards, soffit and gutters** on them from the Roof Defaults. The ghost that follows the pointer in
  Auto Dormer and Auto Floating Dormer outlines the dormer's overhanging planes before you click.
- **Explode Dormer**: click a dormer (or use the Edit toolbar button with a dormer selected). Its roof planes become
  ordinary planes and its footprint a plain hole in the main plane (none for a floating dormer), and **the front and cheek walls become real walls**: they take the default exterior wall type
  and thickness, sit with their outer face on the footprint, and stand on the roof through a **Bottom Height** (chapter 2.9); a dormer window becomes a window opening in the front wall. "Exploded the
  dormer into n roof planes". The front wall of a gable dormer is made as tall as the ridge and as a Full Gable wall, so it rises to the underside of the dormer roof and fills the gable triangle (Round 13); the exploded planes keep their overhang. Limits: room detection does not know Bottom Height, so a dormer wall
  is a wall of the roof's floor and, being open at the back, makes no room (a raised wall of 48" or more is drawn dashed, chapter 2.1); and Auto Rebuild after wall changes sees the dormer walls as exterior walls of that floor.

### Roof Return mode

Click an eave corner to add a **roof return** 24" long at that corner. A plain click makes a full return (a quad that
continues the main plane around the corner), `Shift` a half return (its triangle) and `Alt` a boxed return (a level boxed
return). It becomes a new roof plane ("Roof return made"; "A roof return does not fit at that corner" when it cannot).
In Roof Return mode the palette has a **Roof Return Settings...** button: the Roof Return dialog sets the **Type** (Full, Half or Boxed) and the **Length** (at least 2"; 24" until you change it) that the next clicks make; `Shift` and `Alt` still make a half or a boxed return. Chief's slope, extend, shadow-board, ridge-cap, frieze and gutter options for a return are not modeled.

**Auto Roof Return.** A gable-end wall (Full Gable) whose roof directive has *auto roof return* set makes a full 24" return on the planes at both of its corners when Build Roof runs. These returns
are automatic planes with no source edge and are replaced by every rebuild. The flag and the return length are on the **Roof** tab of the Wall Specification of an exterior wall (8.7).

### Join Roof Planes

Joins two planes along the line where they meet: the first plane's edge is extended or trimmed to that line.

1. Click an **edge of the first plane**, or select the plane and press the Edit toolbar's **Join Roof Planes** (which then asks for the edge).
2. Click the **second plane**.

"Roof planes joined" is the result, in one undo step ("Join Roof Planes"); the joined plane becomes **manual** (a later Build Roof leaves it alone). It refuses, with the reason in the status bar, for planes that are parallel,
for an edge whose neighbors run parallel to the meeting line, or when the result would fold over. `Esc` drops the first pick.

### Auto Floating Dormer

Works exactly like Auto Dormer (click a plane, then the Dormer Specification), but the dormer is marked **floating**: it cuts no hole in the roof plane under it, in 3D or when you explode it. Editing it later keeps the flag.
Use it for a dormer that sits on top of the roof surface instead of breaking through it.

## 8.3 What the plan and 3D show

- In plan, each plane draws as an outline on the `Roof Planes` layer with its pitch shown next to
  its label (for example `8:12`); vaulted ceiling planes draw on `Ceiling Planes`.
- The 3D view shows each plane as a slab with its roof holes cut out, a skylight's curb, frame and glass on top, the
  ceiling planes (their neighbors meet in mitres), and the dormers (walls, roof planes and their eave detail; their footprint is cut from the main plane).
  The roofs of every floor are drawn, whichever floor is active; in the plan a roof draws only while the floor that holds it is active.
  Bay and bow windows carry a 6:12 hip roof and a box window a 6:12 shed roof, flush with the unit (Round 14; the pitch and overhang are not editable yet). You can check a
  roof in Perspective, Doll House and elevation views (chapter 10).
- **Walls follow the roof in 3D.** The scene builder reads the roof and ceiling planes stored on the floors and shapes the
  top of every ordinary wall to them, so a roof no longer floats over flat-topped walls:
  - A wall along a **gable end** (a wall whose directive is Full Gable, Dutch Gable, High Shed/Gable or Knee Wall, or an exterior wall on an edge
    that Gable/Roof Line made a gable) rises to the underside of the roof and ends in the gable **triangle**; the wall face has vertices along the
    slopes. An eave wall stops at its plate.
  - Under a **hip** or shed roof every wall is clipped to the plane above it: no wall vertex ends up above the roof's underside, and openings
    in the wall are still cut.
  - An **interior** wall rises to the ceiling planes over it (a vaulted ceiling), and is cut by the roof above.
  - A wall with no plane above it keeps its flat top. A wall's own **Bottom Height** and the planes of other floors are honored: a plane that is not at least 1" above the wall's bottom does not shape it (a dormer wall stands on the roof surface).
  - **Half, pony and foundation walls** are cut by the roof like a standard wall but are never raised to a gable: a half wall stays a half wall under the gable. A **curved** exterior or interior wall is cut by the roof facet by facet and (Round 12) is raised to the gable like a straight wall, so a gable end on an arc follows the roof; a curved wall of another class (glass, pony ...) is cut but not raised.
  - **Roof Cuts Wall at Bottom** (Roof Defaults, 8.4a; on by default): the bottom of a wall that stands over a lower roof (a second-floor wall on a first-floor roof) is cut along the top surface of that roof instead of ending in a flat line.
  - **Attic and lower wall types.** The part of a wall above its plate (a gable above the top plate) takes the **Attic Wall Type** of Roof Defaults, and the part below a butting roof's line takes the **Lower Wall Type**; empty keeps the wall's own type.
  - **Butting roofs.** Where a lower roof meets a wall that rises well above it (more than 1" above the plane and running at least 6" through it), the
    plane is **trimmed** a half inch short of the wall face instead of the wall being cut, and a **flashing** strip follows the butt line. The
    wall of the upper floor over that lower roof gets an **attic wall** that fills the gap between the roof and the wall's bottom (Auto Attic Walls, on by default; none is made where the roof clears the wall).
- **Eave detail**, drawn with each roof plane and picked with it: the **eave cut** (the end of the roof structure at the eave: Plumb, a vertical end with the fascia hanging plumb; Level, a horizontal end; or Square to the rafter, the fascia perpendicular to the plane), **fascia** along the eaves (6" high, 1 1/2" thick by default), an eave **soffit** under the overhang (level, or sloped with the roof), or
  exposed **rafter tails** in its place, **rake** fascia and rake soffit on gable ends, a **frieze** board on the wall (off by default), **gutters** (off by default; 5"), and **ridge and hip caps** on planes
  that have Include Ridge Caps ticked in the Roof Plane Specification (8.5). The roof slab is 6" thick along the plane normal by default.
  The sizes and switches are the **Roof Defaults** (8.4a), copied into the roof when Build Roof runs; each plane can override the eave options (8.5). Roofs stored before Roof Defaults existed keep the older eave-tip baseline.
- **Rafter tails** are boxes `Rafter Width` wide every `Spacing` inches on center (24" by default; the first flush with the end) from the eave tip back to the wall face, hanging `Rafter Depth` below the underside of the roof (1 1/2" x 5 1/2"); where tails are on, the soffit is left out under that eave.
- **Build Roof baseline.** With **Roof baseline at top plate** on (the default), Build Roof seats the underside of the roof structure on the top plate at the wall, so gable corners meet the plate with no gap and no overlap; older plans, and the option off, put the eave tip at plate height instead, which stands gable corners about 6" above the plate.
- Build > Framing > Build Framing frames the roof planes stored on the floor (rafters from `plan-framing`;
  chapter 11.11), and the manual roof framing tools (Rafter, Roof Beam, Roof Truss, Roof Truss Direction, Truss Base ...)
  are in the Roof Framing flyout (11.11).

## 8.4 Dialog: Build Roof

Opened from the Build Roof mode. Four tabs.

| Tab | Fields |
|---|---|
| Roof | **Roof Styles** (Round 14): Hip, Gable, Shed, Gambrel, Dutch Gable and Half Hip. Pick one (click it again to clear it) and OK writes the roof directives of the exterior walls before it builds, in the same undo step as the build: Hip makes every wall a Hip Wall; Gable makes the walls across the ridge Full Gable Walls; Shed makes one long wall the High Shed/Gable Wall and the ends Full Gable Walls; Gambrel gives the gable ends and a steep lower and a shallow upper pitch on the long walls; Dutch Gable makes the walls across the ridge Dutch Gable Walls; Half Hip makes gable ends whose peak is clipped by a small hip (Starts at Height). With no style the roof follows each wall's own Roof tab. The preview draws the style. **Roof**: Build Roof Planes, Auto Rebuild Roofs, Ignore Top Floor (build over the floor below the top one), **Build ceiling planes for vaulted rooms** (Build Roof makes the ceiling planes of 8.2). **Defaults for walls without their own roof settings**: Pitch, Overhang, Raise Roof Off Plate. A note says which floor the roof goes over. |
| Options | **Framing**: Build Framing (stored; the Build > Framing commands frame the stored roof planes whatever it says, 11.11), Framing Method Rafters or Trusses (Round 16: picks which Roof Height fields apply). |
| Materials | **Roofing**: Material (Asphalt Shingles, Concrete Tile, Standing Seam Metal, Wood Shakes, Slate). |
| Detail | The Roof Defaults form (8.4a), for this roof: it is copied into the roof settings of the floor when Build Roof runs. |

**Roof Height group (Round 16, brief 18)** on the Roof tab, below Raise Roof Off Plate. With Rafters:
Automatic Birdsmouth Cut (on by default; with it off, Raise Off Plate / Birdsmouth Cut is typed, a
positive value lifts the roof for attic knee walls and a negative value sinks it into a birdsmouth, and
the Birdsmouth Seat read-out follows from the pitch). With Trusses: Heel Height, which lifts the roof off
the plates. Vertical Structure Depth is a read-out. Three eave switches follow. **Same Roof Height at
Exterior Walls** (on by default) keeps the bearing walls at one height and changes the overhang of each
plane whose pitch differs from the default so its fascia drops as far as the default plane's; the wall
overhangs are ignored, and a plane that meets no plane of another pitch keeps its own overhang.
**Same Height Eaves** puts every eave at the height of a plane with the default pitch and overhang and
honors the wall overhangs; with both on, independent planes also take the adjusted overhang. **Allow Low
Roof Planes** is stored (see DECISIONS RH4). The group is kept with the roof's settings.

OK is refused with "Pitch must be between 0.5 and 24 in 12" for an out-of-range pitch. The
preview draws a hip outline with the pitch label.

## 8.4a Default Settings: Roof Defaults

**Edit > Default Settings > Roofs > Roof Defaults** opens a page (the same form as Build Roof's Detail tab, where the two wall type fields are text boxes instead of lists). Its values are saved with your plan defaults (`PlanDefaults::roof_detail`), Build Roof copies them into the roof settings of its floor, and the 3D view draws from them. **OK** also gives the roofs already in this plan the new detail, as one undo step named "Roof Defaults", when **Also use for the roofs in this plan** is ticked (it is, by default); Cancel changes nothing. OK is blocked for a zero roof thickness, a negative fascia size, or rafter tails with a spacing under 1" or a zero width or depth.

| Section | Fields (defaults) |
|---|---|
| Roof Structure | **Thickness** (6"), **Roof baseline at top plate** (on). |
| Eaves | **Eave Cut** (Plumb, Level, Square; Plumb), **Fascia** (on) with **Fascia Height** (6") and **Fascia Thickness** (1 1/2"), **Rake Fascia** (on), **Soffit** (on) and **Sloped Soffit** (off; needs Soffit), **Frieze Board** (off), **Ridge and Hip Caps** (on; drawn only on planes with Include Ridge Caps), **Gutters** (off) with **Gutter Size** (5"), **Flashing at Butting Roofs** (on). |
| Rafter Tails | **Exposed Rafter Tails** (off; they replace the soffit) with **Spacing (on center)** (24"), **Rafter Width** (1 1/2") and **Rafter Depth** (5 1/2"). |
| Walls Under the Roof | **Auto Attic Walls** (on: an attic wall fills the gap between a lower roof and the bottom of the wall above it), **Attic Wall Type** and **Lower Wall Type if Split by Butting Roof** (each a list of the plan's wall types; empty keeps the wall's own type), **Roof Cuts Wall at Bottom** (on). |

Limits: Roof Cuts Wall at Bottom follows the centerline of straight walls (a curved wall's bottom stays flat); a wall gets one material split (a butting roof wins over the plate split); an unsplit exterior wall that spans a roofless room and a roofed one keeps its roof; a changed Roof Defaults reaches the roofs of a plan only through the OK check box or the next Build Roof.

## 8.5 Dialog: Roof Plane Specification

Open by double-clicking a plane.

| Tab | Fields |
|---|---|
| General | Pitch (changing it re-solves the plane from the baseline), Baseline Height, Overhang (read-only), Surface Area (true sloped area), Origin (Automatic or Manual), and a table of the plane's vertices (number, X, Y, elevation). |
| Holes | Lists the plane's roof holes and skylights, each with its size and a **Delete** button. A skylight also has **Curb Height** (up to 48"), **Glass Thickness** (up to 6") and **Frame Width** (up to 12"). "This plane has no holes. Use the Roof Hole and Skylight tools." when empty. |
| Build Roof Edge | For planes made by Build Roof: the settings of the wall edge the plane rises from. **Pitch** and **Overhang from wall face** are optional overrides (tick to use), and **Gable end (no plane rises from this edge)** turns the edge into a gable. OK rebuilds the automatic roof with these edge settings (the plane disappears if you made it a gable). Other planes show "Only planes made by Build Roof rise from a wall edge." |
| Options | **Eaves and Ridge**: Include Ridge Caps (draws ridge and hip caps on this plane in 3D, 8.3), and this plane's own eave choices, each "Roof Default" until you change it: **Eave Cut** (Roof Default, Plumb, Level, Square) and **Rafter Tails**, **Fascia**, **Soffit**, **Frieze** and **Gutters** (Roof Default, On or Off, as a three-way choice). Only this plane's eaves change; the rest of the roof follows Roof Defaults (8.4a). |
| Structure | **Structure > Define** (Round 14): the Source (This plane or Roof Defaults), the framing, member size and spacing, and thickness, with **Define...** and **Use Roof Defaults**. **Define Roof Structure** has Roof Framing (Rafters or Trusses; Member Width, Member Depth, Spacing On Center), Layers Over the Framing (Sheathing, Roofing thickness) and Ceiling Framing (Ceiling Joists, Vaulted (rafters show), Truss Bottom Chords), with the total thickness shown. The thickness and the rafter size reach the 3D roof. |
| Materials | Roofing material. |
| Layer | The layer. |
| Label | The label text; the pitch is always appended. |

Editing a plane's own fields by hand marks it Manual, so a later Build Roof leaves it alone. The per-edge overrides on the Build Roof
Edge tab are different: they are remembered with the roof's settings and used by the next Build Roof.

With a plane selected, the Edit toolbar offers **Join Roof Planes** (8.2) and **Rebuild Roofs** (re-runs the last Build Roof settings; "No roof has been
built yet: use Build Roof" if there are none). With a dormer selected it offers **Explode Dormer** instead.

### Dialog: Ceiling Plane Specification

Open by double-click or `Enter` on a ceiling plane (it opens in Select Objects and in Edit All Roof Planes). Three tabs on the shared frame.

| Tab | Fields |
|---|---|
| General | **Height at Baseline** (the plane's height at its baseline), **Pitch** (0 to 24 in 12), **Thickness** (not negative), and **Origin**: built by Build Roof (replaced when the roof is rebuilt) or manual (kept). |
| Line Style | The line style of the plane's outline. |
| Layer | The layer, `Ceiling Planes` by default. |

OK writes the change as one undo step, and a plane you edit is no longer replaced by Build Roof.

## 8.6 Dialog: Dormer Specification

Auto Dormer opens it after your click, and a double-click on a dormer in Edit mode reopens it. Three tabs.

| Tab | Fields |
|---|---|
| General | **Type**: Gable, Shed or Hip. **Width** (48" by default) and **Wall Height** (36"). **Position on the roof plane**: Along the Eave (the distance from the plane's first eave vertex to the dormer's center) and Setback from Eave (36" by default; a click sets both). |
| Roof | **Pitch** (8:12 by default), **Height to Ridge** and **Overhang** (12" by default; 0 for none). The overhang pushes the eaves and rakes out and builds the dormer's fascia, rake boards, soffit and gutters. A ridge higher than the walls sets the pitch (the dormer's planes then report the pitch they got); 0 uses the pitch above. A shed dormer uses the pitch and halves it when it is not flatter than the main roof. |
| Window | A check box "Window in the front wall" with its Width and Height. |

OK is refused for an out-of-range pitch or an invalid length. The preview draws the dormer's front elevation.

## 8.7 Wall directives

Each exterior wall carries a roof directive in the model: kind (Hip, Full Gable, Dutch Gable,
High Shed/Gable, Knee Wall, Extend Slope Downward), pitch, overhang, an optional upper pitch with
its start height, and flags (Include Frieze, auto roof return; Roof Cuts Wall at Bottom is a Roof Defaults setting, 8.4a). Daniel's template sets
exterior walls to Hip, 8:12, 16" overhang. You change a wall's directive on the **Roof** tab of the Wall Specification (exterior walls only; it is dimmed on other walls),
with Gable/Roof Line (Hip or Full Gable), and in the Build Roof defaults. Build Roof reads them; a change to a wall directive rebuilds the automatic planes when Auto Rebuild Roofs is on.

| Roof tab section | Fields |
|---|---|
| Roof Options | Hip Wall, Full Gable Wall, Dutch Gable Wall, High Shed/Gable Wall, Knee Wall, Extend Slope Downward (with **Drop below Eave**, 24" by default). |
| Pitch Options | **Specify Pitch** (8:12 until you change it) for this wall's edge; **Upper Pitch** with the second pitch (12:12 by default) and **Starts at Height** above the wall's floor (the wall's height plus 48" by default) or, for a Dutch gable wall, **Starts Dutch Gable at Height**. |
| Overhang | **Specify Overhang** and its Length (16" by default). |
| Auto Roof Return | **Auto Roof Return** with its Length (24" by default): a full return at both corners of a gable end. |

What Build Roof does with them:

- **Full Gable** and **High Shed/Gable**: the edge becomes a gable (or shed) end; the wall rises to the roof in 3D (8.3).
- **Extend Slope Downward**: the edge's plane continues down past the eave. It reaches down to the top of the wall below (the nearest lower floor's exterior wall within 6' of the edge) when there is one, else it drops the **Drop below Eave** length you typed (Round 14).
- **Knee Wall**: the wall stands under a roof plane and makes no plane of its own; Build Roof leaves it out of the footprint unless that would leave no outline.
- **Upper Pitch** (a gambrel or mansard) and **Dutch Gable**: the roof is built in two stages. The lower roof is the plain hip roof at the lower pitches, cut by a level plane at the break (the edges' Starts-at heights give the break above the eave; the smallest wins, and
  with none it is 60% of the roof's height). A second roof is built on the cut, with each edge at its upper pitch; a Dutch gable edge is a vertical gable end there, so the hip below it ends in a short gable. Edges that keep their pitch across the break become one plane again.
  If the two-stage roof cannot be built (the skeleton had to approximate, or the break is at or above the peak) Build Roof makes the plain roof.

## 8.7a Roof trim, Gable/Roof Lines and skylights

**Roof Trim.** Build > Roof > Roof Trim opens a dialog with a tab for each trim part: Rafter Tails, Ridge Caps, Gutters, Frieze, Shadow Boards, Subfascia, Lookouts and Soffits. Each part has a switch (nothing is made until it is on), a profile from the molding library (or a plain board) and its width and height. Build Roof then makes the parts as molding polylines on the layer Roofs, Trim, Automatically Generated; they select and edit like any molding polyline, they are listed under Exterior Trim in the Materials List, and a piece you edit stays when the trim is made again.

| Part | What it does |
|---|---|
| Rafter Tails | Recipe Exposed (tails show, no soffit), Hidden (no tails, a soffit) or Partially Exposed (only the last few inches show past the soffit); Stretch to Fit Rafter or an own width and height; Extend Past Subfascia. |
| Ridge Caps | Bend to Roof Pitch makes a strip on each plane of a ridge or hip; off makes one level strip. The Ridge Cap setting of a single edge is Automatic (ridges and hips), On or Off. |
| Gutters | Along eaves that do not slope. |
| Frieze | Against the wall under the eaves and under the gable overhangs. |
| Shadow Boards | On the face of the fascia, eaves and rakes; needs a fascia. |
| Subfascia, Lookouts | A board behind the fascia; blocks every 24" under the rake overhangs. |
| Soffits | Boxed (horizontal) or Flush (follows the rafters), Higher Eaves Boxed, Trim Framing To Soffits. |

**Gable/Roof Line objects.** A line drawn exactly parallel to an exterior wall and within 10 feet of the wall's Main Layer stays in the plan until you delete it. Each Build Roof turns it into a gable: two planes of the line's own pitch and overhang, running from the line into the roof, with valleys where they meet the old roof planes. The Gable Line Specification has the Gable Line (pitch, overhang), Line, Line Style and Arrow panels. With doors or windows on an exterior wall selected, **Gable Over Door/Window** draws a line 12" past each side of each one (openings within 30" of each other share a line); **Delete Gable Over Opening** takes those lines away again.

**Skylights.** A click with the Skylight tool makes a 2 by 2 foot skylight, a drag a rectangle. The Skylight Specification: General (Shape Rectangle, Circle, Ellipse, Oval or Custom, Width and Length, Frame Width and Height, Display in Plan View, Edit Skylight Shape), Inside Hole Rim (Square, Plumb, Plumb/Square) and Ceiling Hole (Automatically Generate, Use Manual Polyline, Do Not Cut). Moving a corner of the opening makes the shape Custom.

**Dormers.** The pure geometry for a gambrel dormer (second pitch, In from Eave), the Auto Roof Return of a gable dormer, the Dormer Room options of a floating dormer (shaft to the room below, Set to Existing Ceiling) and crickets behind up-slope walls is in `plan-roof` and tested; the Dormer Specification does not offer them yet (docs/integration-queue.md).

## 8.8 Differences from Chief

- A roof with an upper pitch or a Dutch gable has one break height for the whole roof. The vertical face of a Dutch gable is not a roof plane: Build Roof stores it as a **face** record that 3D meshes like wall, which every rebuild and Delete Roof Planes replaces and which you cannot select (Round 13).
- Extend Slope Downward drops the typed length when there is no wall below to reach, and Auto Roof Return applies only at gable ends. Include Frieze on the Wall Roof tab is stored but not edited (the frieze is a Roof Defaults switch and a per-plane choice); Roof Cuts Wall at Bottom is a Roof Defaults switch for the whole roof, not a per-wall one.
- Explode Dormer keeps the walls (the gable dormer's front wall now reaches the ridge), and room detection does not read their Bottom Height. A dormer cannot straddle two planes.
- Not built: a break line per edge on a staged roof, Build Roof per framing group. Round 16 brief 20 added skylight shapes, roof trim molding polylines and Gable/Roof Line objects (8.7a); Mansard, Barrel, Curved Eave and Eyebrow dormers and automatic crickets are still open.
- A Roof Return is a full, half or boxed return of a length you set; its other options (slope, extend, shadow boards, ridge cap, frieze, gutter) are not modeled.
- Build Ceiling Planes follows the roof planes only. In 3D an interior wall rises to the ceiling planes over it (8.3).
- Roof holes must sit wholly inside one plane; Chief's holes across a ridge need one hole per plane here.
- No automatic attic floor from Build Roof.
- Half, pony and foundation walls (and curved walls of those classes) are cut by the roof but never raised to a gable; railing, glass, fencing, deck and the other special classes keep a flat top. With the baseline-at-plate rule off (and in plans built before it existed) the gable triangle's corners stand about 6" above the plate because the
  roof slab is 6" thick (the wall rises to the roof's underside).
- The eave cut is Plumb, Level or Square (no other angle); gutters are a plain board-shaped box hung along the eave (no profile, downspouts or slope), and rafter tails are rectangular boxes (no bird's-mouth or decorative end cut). The flat roof plane is a room's **Flat Roof Over This Room** (chapter 4.4).

