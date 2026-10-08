# Chapter 8: Roofs

Plan Studio builds roofs two ways, like Chief: automatically from the exterior
walls (Build Roof), or one plane at a time (Roof Plane). Both produce editable
**roof planes**, and both show in plan and in the 3D view. On top of the planes sit roof
holes, skylights, vaulted ceiling planes, dormers and roof returns.

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
  `settings`, `ceiling` and `dormer`), so the roof saves, loads and undoes with the plan and its
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
building; fascia is a fixed nominal 6"; Dutch gable, knee wall and upper-pitch (gambrel and
mansard) breaks are not modeled. A rare case (two parallel same-facing planes of different pitch,
or a hip next to a gable, on either side of a short jog) also falls back to the approximation; the
`plan-roof` README gives the sweep results.

## 8.2 Tools

### Roof Tools (row 2, Roof flyout; Build > Roof)

Each flyout entry starts the Roof tool in its mode. The tool also draws a small **palette** of mode
buttons in the top-left corner of the canvas (including Delete Roof Planes and Rebuild Roofs), so you
can change mode without going back to the flyout.

| Flyout entry | Hotkey | Mode | Today |
|---|---|---|---|
| Roof Plane | `Q` | Roof Plane | Works. |
| Build Roof | `Ctrl+Alt+Shift+Cmd+N` | Build Roof | Works: opens the Build Roof dialog. |
| Ceiling Plane | `Ctrl+Alt+Shift+Cmd+U` | Ceiling Plane | Works (8.2). |
| Gable/Roof Line | `Ctrl+Alt+Shift+Cmd+O` | Gable/Roof Line | Works. |
| Roof Hole | `Ctrl+Alt+Shift+Cmd+T` | Roof Hole | Works. |
| Skylight | `Ctrl+Alt+Shift+Cmd+S` | Skylight | Works. |
| Auto Dormer | `Ctrl+Alt+Shift+Cmd+Z` | Auto Dormer | Works: click a plane, then the Dormer Specification (8.6). |
| Auto Floating Dormer | `Ctrl+Alt+Shift+Cmd+R` | | (planned) |
| Explode Dormer | | Explode Dormer | Works. |
| Roof Return | | Roof Return | Works. |
| Edit All Roof Planes | `Ctrl+Alt+Shift+Cmd+P` | Edit | Works (select, move, reshape planes). |
| Delete Roof Planes | `Ctrl+Alt+Shift+Cmd+W` | (command) | Works: removes every roof plane in the plan, on every floor, in one undo step. "There are no roof planes to delete" if none. |
| Delete Ceiling Planes | `Ctrl+Alt+Shift+Cmd+X` | (command) | Works: removes the ceiling planes of every floor in one undo step. "There are no ceiling planes to delete" if none. |
| Join Roof Planes | (Edit toolbar) | Join | (planned) Asks for two planes, then says "not implemented yet". |

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
| Double-click, `Enter` | Opens the Roof Plane Specification. |
| `Delete` | Removes the plane. |

### Build Roof mode

Click once to open the **Build Roof** dialog (8.4). OK builds the automatic planes from the
exterior walls of the floor, using each wall's roof directive (Hip by default) and writes
them as editable planes. With **Auto Rebuild Roofs** on, a change to the walls that
affects the roof (a moved or added wall, a changed directive or height) rebuilds the automatic
planes; manual planes are not touched. A message tells you if no exterior outline exists.

### Gable/Roof Line mode

Click near an **eave** to make that edge a gable end. For a plane made by Build Roof, the edge becomes a
gable end in the Build Roof edge settings and the roof is rebuilt ("That edge is now a gable end"). For a
manual plane, the roof is rebuilt from the planes' eave baselines (`plan_roof::apply_gable_line`); that needs the
planes to form one closed ring or one chain with a single straight gap, otherwise the status bar says
"Gable/Roof Line: <reason>". The new gable wall stands on the old eave line and the edge's overhang becomes the
rake.

Clicking an exterior wall (away from any eave) still flips it between **Hip** and **Full Gable**. A gable wall
rises to the ridge instead of carrying a plane, and its neighbors extend to the rake. The tool flips every wall
along the same footprint edge. Other wall directives (Dutch Gable, High Shed/Gable, Knee Wall,
Extend Slope Downward) exist in the model; only High Shed/Gable is honored by the builder today
(as a shed edge). The Roof tab of the Wall Specification is (disabled).

### Roof Hole and Skylight

- **Roof Hole**: press and drag a rectangle inside a plane to cut a hole (at least 6" on each side).
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
**Delete Ceiling Planes** clears them all. The Build Roof dialog's Build Ceiling Planes box is stored but does not make
planes yet; you draw each ceiling plane by hand. In Edit All Roof Planes a ceiling plane can be selected, moved and
deleted like a roof plane; it has no specification dialog yet.

### Auto Dormer and Explode Dormer

- **Auto Dormer**: click inside a roof plane. The click sets the dormer's place (centered at the click along the eave;
  the distance up the slope is the setback, at least 12") and the **Dormer Specification** opens (8.6); OK builds the
  dormer. "Auto Dormer: <reason>" is shown when it does not fit: too big, too close to the eave, past the ridge, a
  hip ridge that would collapse, or a flat plane. A dormer across two planes is not supported. Double-click a dormer with
  Edit All Roof Planes to change it; it follows its plane when Build Roof rebuilds the roof.
- A dormer has a front wall (parallel to the eave), two triangular cheek walls, its own roof planes (gable 2, shed 1,
  hip 3), a window in the front wall if you ask for one, and a hole in the main roof plane under it. It has no overhang,
  soffit or fascia.
- **Explode Dormer**: click a dormer (or use the Edit toolbar button with a dormer selected). Its roof planes become
  ordinary planes and its footprint a plain hole in the main plane; **the dormer's walls are dropped**. "Exploded the
  dormer into n roof planes".

### Roof Return mode

Click an eave corner to add a **roof return** 24" long at that corner. A plain click makes a full return (a quad that
continues the main plane around the corner), `Shift` a half return (its triangle) and `Alt` a boxed return (a level boxed
return). It becomes a new roof plane ("Roof return made"; "A roof return does not fit at that corner" when it cannot).
Chief's slope, extend, shadow-board, ridge-cap, frieze and gutter options are not modeled, and the length is fixed.

## 8.3 What the plan and 3D show

- In plan, each plane draws as an outline on the `Roof Planes` layer with its pitch shown next to
  its label (for example `8:12`); vaulted ceiling planes draw on `Ceiling Planes`.
- The 3D view shows each plane as a slab with its roof holes cut out, a skylight's curb, frame and glass on top, the
  ceiling planes, and the dormers (walls and roof planes; their footprint is cut from the main plane). You can check a
  roof in Perspective, Doll House and elevation views (chapter 10).
- Ridge caps, gutters, fascia and frieze, soffits and rafter tails are (planned).
- Build > Framing > Build Framing frames the roof planes stored on the floor (rafters from `plan-framing`;
  chapter 11.11), and the manual roof framing tools (Rafter, Roof Beam, Roof Truss, Roof Truss Direction, Truss Base ...)
  are in the Roof Framing flyout (11.11).

## 8.4 Dialog: Build Roof

Opened from the Build Roof mode. Three tabs.

| Tab | Fields |
|---|---|
| Roof | **Roof**: Build Roof Planes, Auto Rebuild Roofs, Ignore Top Floor (build over the floor below the top one), Build Ceiling Planes (stored; Build Roof does not generate ceiling planes yet, use the Ceiling Plane tool). **Defaults for walls without their own roof settings**: Pitch, Overhang, Raise Roof Off Plate. A note says which floor the roof goes over. |
| Options | **Framing**: Build Framing (stored; the Build > Framing commands frame the stored roof planes whatever it says, 11.11), Rafters (disabled, on), Trusses (disabled, off). |
| Materials | **Roofing**: Material (Asphalt Shingles, Concrete Tile, Standing Seam Metal, Wood Shakes, Slate). |

OK is refused with "Pitch must be between 0.5 and 24 in 12" for an out-of-range pitch. The
preview draws a hip outline with the pitch label.

## 8.5 Dialog: Roof Plane Specification

Open by double-clicking a plane.

| Tab | Fields |
|---|---|
| General | Pitch (changing it re-solves the plane from the baseline), Baseline Height, Overhang (read-only), Surface Area (true sloped area), Origin (Automatic or Manual), and a table of the plane's vertices (number, X, Y, elevation). |
| Holes | Lists the plane's roof holes and skylights, each with its size and a **Delete** button. A skylight also has **Curb Height** (up to 48"), **Glass Thickness** (up to 6") and **Frame Width** (up to 12"). "This plane has no holes. Use the Roof Hole and Skylight tools." when empty. |
| Build Roof Edge | For planes made by Build Roof: the settings of the wall edge the plane rises from. **Pitch** and **Overhang from wall face** are optional overrides (tick to use), and **Gable end (no plane rises from this edge)** turns the edge into a gable. OK rebuilds the automatic roof with these edge settings (the plane disappears if you made it a gable). Other planes show "Only planes made by Build Roof rise from a wall edge." |
| Options | **Eaves and Ridge**: Include Ridge Caps, Include Gutter (both stored, not modeled yet). |
| Materials | Roofing material. |
| Layer | The layer. |
| Label | The label text; the pitch is always appended. |

Editing a plane's own fields by hand marks it Manual, so a later Build Roof leaves it alone. The per-edge overrides on the Build Roof
Edge tab are different: they are remembered with the roof's settings and used by the next Build Roof.

With a plane selected, the Edit toolbar offers **Join Roof Planes** (planned: it asks for two planes, then
says "not implemented yet") and **Rebuild Roofs** (re-runs the last Build Roof settings; "No roof has been
built yet: use Build Roof" if there are none). With a dormer selected it offers **Explode Dormer** instead.

## 8.6 Dialog: Dormer Specification

Auto Dormer opens it after your click, and a double-click on a dormer in Edit mode reopens it. Three tabs.

| Tab | Fields |
|---|---|
| General | **Type**: Gable, Shed or Hip. **Width** (48" by default) and **Wall Height** (36"). **Position on the roof plane**: Along the Eave (the distance from the plane's first eave vertex to the dormer's center) and Setback from Eave (36" by default; a click sets both). |
| Roof | **Pitch** (8:12 by default) and **Height to Ridge**. A ridge higher than the walls sets the pitch (the dormer's planes then report the pitch they got); 0 uses the pitch above. A shed dormer uses the pitch and halves it when it is not flatter than the main roof. |
| Window | A check box "Window in the front wall" with its Width and Height. |

OK is refused for an out-of-range pitch or an invalid length. The preview draws the dormer's front elevation.

## 8.7 Wall directives

Each exterior wall carries a roof directive in the model: kind (Hip, Full Gable, Dutch Gable,
High Shed/Gable, Knee Wall, Extend Slope Downward), pitch, overhang, an optional upper pitch with
its start height, and flags (Include Frieze, Roof Cuts Wall at Bottom). Daniel's template sets
exterior walls to Hip, 8:12, 16" overhang. Today you change a wall's directive with
Gable/Roof Line (Hip or Full Gable) and the Build Roof defaults; the per-wall Roof tab is planned.

## 8.8 Differences from Chief

- No Join Roof Planes, Auto Floating Dormer or Dutch gable, knee wall and upper-pitch breaks.
- Explode Dormer drops the dormer's walls. Dormers have no overhang, soffit or fascia, and a dormer cannot straddle two planes.
- Roof Return is a fixed 24" full, half or boxed return; its options (slope, extend, shadow boards, ridge cap, frieze, gutter) are not modeled.
- Ceiling planes are drawn by hand, one at a time (Build Roof does not generate them), and walls are not cut by them.
- Roof holes must sit wholly inside one plane; Chief's holes across a ridge need one hole per plane here.
- No automatic attic floor from Build Roof.
- Walls do not yet extend to meet the roof plane above them, so gable ends do not get their triangle in 3D.
- Fascia, soffit, gutters and rafter tails are not modeled.
