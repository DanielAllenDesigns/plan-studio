# Chapter 8: Roofs

Plan Studio builds roofs two ways, like Chief: automatically from the exterior
walls (Build Roof), or one plane at a time (Roof Plane). Both produce editable
**roof planes**, and both show in plan and in the 3D view.

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
- Plane data is stored on the floor the roof sits on as hidden `Roof Planes, Data` records,
  with a visible outline polyline on the `Roof Planes` layer, so the roof saves, loads, undoes
  and exports to DXF with the plan. (A dedicated roof field in the plan file is a planned clean-up.)
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
mansard) breaks are not modeled.

## 8.2 Tools

### Roof Tools (row 2, Roof flyout; Build > Roof)

Each flyout entry starts the Roof tool in its mode. The tool also draws a small **palette** of mode
buttons in the top-left corner of the canvas (including Delete Roof Planes and Rebuild Roofs), so you
can change mode without going back to the flyout.

| Flyout entry | Hotkey | Mode | Today |
|---|---|---|---|
| Roof Plane | `Q` | Roof Plane | Works. |
| Build Roof | `Ctrl+Alt+Shift+Cmd+N` | Build Roof | Works: opens the Build Roof dialog. |
| Gable/Roof Line | `Ctrl+Alt+Shift+Cmd+O` | Gable/Roof Line | Works. |
| Roof Hole | `Ctrl+Alt+Shift+Cmd+T` | Roof Hole | Works. |
| Skylight | `Ctrl+Alt+Shift+Cmd+S` | Skylight | Works. |
| Edit All Roof Planes | `Ctrl+Alt+Shift+Cmd+P` | Edit | Works (select, move, reshape planes). |
| Delete Roof Planes | `Ctrl+Alt+Shift+Cmd+W` | (command) | Works: removes every roof plane in the plan, on every floor, in one undo step. "There are no roof planes to delete" if none. |
| Auto Dormer | `Ctrl+Alt+Shift+Cmd+Z` | Auto Dormer | (planned) Opens the tool and says "not implemented yet". |
| Join Roof Planes | (Edit toolbar) | Join | (planned) Asks for two planes, then says "not implemented yet". |
| Ceiling Plane | `Ctrl+Alt+Shift+Cmd+U` | | (planned) |
| Auto Floating Dormer | `Ctrl+Alt+Shift+Cmd+R` | | (planned) |
| Delete Ceiling Planes | `Ctrl+Alt+Shift+Cmd+X` | | (planned) |

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

Click an exterior wall to flip it between **Hip** and **Full Gable**. A gable wall rises to the
ridge instead of carrying a plane, and its neighbors extend to the rake. The tool flips every wall
along the same footprint edge. Other wall directives (Dutch Gable, High Shed/Gable, Knee Wall,
Extend Slope Downward) exist in the model; only High Shed/Gable is honored by the builder today
(as a shed edge). The Roof tab of the Wall Specification is (disabled).

### Roof Hole and Skylight

- **Roof Hole**: press and drag a rectangle inside a plane to cut a hole.
- **Skylight**: click inside a plane to place a skylight (a size and center stored on the plane).

## 8.3 What the plan and 3D show

- In plan, each plane draws as an outline on the `Roof Planes` layer with its pitch shown next to
  its label (for example `8:12`).
- The 3D view shows roof planes as slabs. The 3D builder (`roof_meshes`) is the only non-wall
  object the 3D view draws besides floors, so you can check a roof in Perspective, Doll House and
  elevation views (chapter 10).
- Ridge caps, gutters, fascia and frieze, soffits and rafter tails are (planned).
- Roof framing (Rafter, Roof Beam, Roof Truss ...) is (planned; engine in `plan-framing`).

## 8.4 Dialog: Build Roof

Opened from the Build Roof mode. Three tabs.

| Tab | Fields |
|---|---|
| Roof | **Roof**: Build Roof Planes, Auto Rebuild Roofs, Ignore Top Floor (build over the floor below the top one), Build Ceiling Planes (stored; vaulted ceilings are not generated yet). **Defaults for walls without their own roof settings**: Pitch, Overhang, Raise Roof Off Plate. A note says which floor the roof goes over. |
| Options | **Framing**: Build Framing (stored; framing is not generated yet), Rafters (disabled, on), Trusses (disabled, off). |
| Materials | **Roofing**: Material (Asphalt Shingles, Concrete Tile, Standing Seam Metal, Wood Shakes, Slate). |

OK is refused with "Pitch must be between 0.5 and 24 in 12" for an out-of-range pitch. The
preview draws a hip outline with the pitch label.

## 8.5 Dialog: Roof Plane Specification

Open by double-clicking a plane.

| Tab | Fields |
|---|---|
| General | Pitch (changing it re-solves the plane from the baseline), Baseline Height, Overhang (read-only), Surface Area (true sloped area), Origin (Automatic or Manual), and a table of the plane's vertices (number, X, Y, elevation). |
| Options | **Eaves and Ridge**: Include Ridge Caps, Include Gutter (both stored, not modeled yet). |
| Materials | Roofing material. |
| Layer | The layer. |
| Label | The label text; the pitch is always appended. |

Editing a plane by hand marks it Manual, so a later Build Roof leaves it alone.

With a plane selected, the Edit toolbar offers **Join Roof Planes** (planned: it asks for two planes, then
says "not implemented yet") and **Rebuild Roofs** (re-runs the last Build Roof settings; "No roof has been
built yet: use Build Roof" if there are none).

## 8.6 Wall directives

Each exterior wall carries a roof directive in the model: kind (Hip, Full Gable, Dutch Gable,
High Shed/Gable, Knee Wall, Extend Slope Downward), pitch, overhang, an optional upper pitch with
its start height, and flags (Include Frieze, Roof Cuts Wall at Bottom). Daniel's template sets
exterior walls to Hip, 8:12, 16" overhang. Today you change a wall's directive with
Gable/Roof Line (Hip or Full Gable) and the Build Roof defaults; the per-wall Roof tab is planned.

## 8.7 Differences from Chief

- No Join Roof Planes, dormers, ceiling planes, roof returns or cathedral ceilings.
- No automatic attic floor from Build Roof.
- Walls do not yet extend to meet the roof plane above them, so gable ends do not get their triangle in 3D.
- Fascia, soffit, gutters and rafter tails are not modeled.
