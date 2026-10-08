# Chapter 9: Electrical and Terrain

Two site-and-systems families share this chapter: the electrical plan (outlets,
switches, lights, connections) and the terrain (lot, elevations, hills, roads,
driveways). Both store their data in typed fields of the plan (a floor's `electrical`
slot and the project's `terrain` slot), so they save and undo with the plan like any other
object. Older files kept them as hidden records on data layers; the program moves those into the
typed fields once, when it opens such a file.

# Part A: Electrical

## 9.1 How electrical works

An electrical **device** has a kind, a position, a height above the floor, a
facing, an optional label and circuit number, and the switches that control
it. Devices live on the `Electrical` layer. The editor stores a floor's devices in that
floor's typed `electrical` slot, so the plan file carries them and undo restores them. (Files
from before this change kept them as one hidden record on an `Electrical, Data` layer; opening
such a file converts it and removes the layer.)

Device kinds known to the engine (`plan-electrical`, 18 kinds): 110V Outlet, 220V Outlet,
GFCI Outlet, Floor Outlet, Switch, 3-Way Switch, Dimmer Switch, Ceiling Light, Recessed Can,
Pendant Light, Wall Sconce, Ceiling Fan, Smoke Detector, CO Detector, Thermostat, Doorbell,
Panel and Rope Light.

Default heights above the floor:

| Device | Height |
|---|---|
| Outlets (110V, 220V, GFCI) | 12" |
| Floor outlet | 0" |
| Switches, doorbell | 48" |
| Thermostat | 52" |
| Wall sconce | 66" |
| Panel, CO detector | 60" |
| Pendant light, rope light | 84" |
| Ceiling light, recessed can, ceiling fan, smoke detector | at the ceiling |

Wall devices sit on the wall face (half the wall thickness off the centerline), facing into
the room. Ceiling devices are placed freely.

## 9.2 Tools

### Electrical Tools (row 2, Electrical flyout; Build > Electrical)

| Variant | Hotkey | What a click does |
|---|---|---|
| 110V Outlet | `E, O` | Places an outlet on the nearest wall within 12", on the face nearest the cursor. The status bar shows `Height: 12"`. |
| 220V Outlet | `Ctrl+Alt+Cmd+7` | Same, 220V symbol. |
| GFCI Outlet | `Ctrl+Alt+Shift+Cmd+Y` | Same, GFCI symbol. |
| Light | `E, L` | Places a ceiling light where you click; it snaps to a room's center within 12". |
| Rope Light | `Ctrl+Alt+Shift+Cmd+A` | Places a strip light. |
| Switch | `E, S` | Places a switch on a wall at 48". |
| 3-Way Switch | | Switch variant. |
| Ceiling Fan | | Ceiling device. |
| Smoke Detector | | Ceiling device. |
| Electrical Connection | `E, C` | Click a switch (or outlet), then the light it controls. A dashed arc is drawn between them and stored. |
| Auto Place Outlets | `E, A, O` | One click places outlets for every room of the floor, from the room names and types. |

All of them start the one Electrical tool, and the Electrical flyout does pass your choice
through to it.

### Placing and editing

- Move the pointer: a ghost shows where the device will go. Wall devices snap to the nearest
  wall within 12" and flip to the face nearest the cursor.
- Click an existing device (with the Electrical tool or Select Objects) to select it. **Drag** to move it:
  wall devices slide along their wall; free devices move freely. `Tab` flips a wall device to the other side of
  the wall. `Left` and `Right` turn a free device (hold `Shift` for 90 degrees). `Delete` removes it.
  Double-click or `Enter` opens the Electrical Service Specification. With a device selected the Edit toolbar
  adds **Flip Side** and **Rotate Device**.

### Auto Place Outlets rules

- No point along a wall is more than 6' from an outlet.
- Outlets stay clear of door jambs.
- Kitchen counters get GFCI outlets at 44".
- Wet rooms and garages get GFCI outlets.

A related engine function places a room light and a switch 6" past the latch jamb at 48";
they are not offered as buttons yet (planned).

## 9.3 Dialog: Electrical Service Specification

Double-click a device with the Electrical tool.

| Tab | Fields |
|---|---|
| General | Type (read-only), Height, Label, Circuit (a number, or blank) |
| Options | **Switched By**: check boxes for the switches on this floor that control this device (a switch is not switched by another switch) |
| Layer | The layer (disabled, `Electrical`) |
| Label | Show label in plan, Text height (disabled) |

## 9.4 Circuits, schedules and legends (engine)

The engine groups devices into circuits (`assign_circuits`), counts devices by kind for a
schedule, and lists a legend of symbols. No menu uses the circuit and legend functions yet: circuit assignment and the electrical legend
are (planned; engine in `plan-electrical`). The **Electrical Schedule** from the Schedule flyout (chapter 11.2) lists the plan's devices (mark, type, label, mount height, circuit) as a live table placed in the plan.
Light fixtures also emit light in the ray tracer (chapter 10.13).
Electrical devices are also not drawn in the 3D view yet (planned).

# Part B: Terrain

## 9.5 How terrain works

A terrain is one object per plan: a **perimeter** (the lot), **elevation data** (points, lines,
regions), **modifiers** (hills, valleys, raised and lowered and flat regions), **features**
(terrain holes under buildings) and **road strips** (roads, driveways, sidewalks).

**Build Terrain** (Chief's command of the same name):

1. Samples a regular grid over the perimeter and interpolates elevations from your data
   (inverse-distance weighting).
2. Applies the modifiers.
3. Triangulates the grid with a Delaunay triangulation, clips to the perimeter and the holes.
4. Smooths the result (Laplacian passes).
5. Extracts **contours** by marching triangles. Every fifth is drawn heavier.

```
   Terrain Perimeter (lot)          After Build Terrain
   +-----------------------+        +-----------------------+
   |   . 102'   . 101'     |        |  ~~ 102 ~~~~           |   contour lines,
   |            hill       |        |    ( ~~ 101 ~~ )       |   labeled in the
   |   . 100'              |        |      ~~ 100 ~~~~       |   plan
   +-----------------------+        +-----------------------+
```

A new project has no terrain. The default terrain is a flat 100' by 80' lot. The record (the terrain,
its contour interval and whether it has been built) is stored in the project's typed `terrain` slot.
Older files kept it on the first floor that held one, on a hidden `Terrain, Data` layer; opening such a file
moves it into the slot and removes the layer.

## 9.6 Terrain tools

All terrain tools are on the Terrain menu (Elevation Data, Modifier, Feature, Road, Driveway,
Sidewalk submenus). Daniel's Terrain Configuration toolbar is (planned). Every working entry starts the one Terrain tool and the choice is
passed through to it.

### Elevation Data

| Variant | Gesture |
|---|---|
| Terrain Perimeter | Click the corners of the lot; `Enter`, double-click or a click on the first point closes it. One per project. |
| Elevation Point | Click, then type the elevation in the inline field and press `Enter`. |
| Elevation Line | Click a polyline; `Enter` or double-click ends it; then type the elevation. |
| Elevation Region | Click a polygon; `Enter` closes it; type the elevation. |
| Elevation Spline, Terrain Break | (planned) |
| Build Terrain | One click builds the surface and shows the contours. |

### Modifier

| Variant | Gesture |
|---|---|
| Hill | Click a polygon, `Enter`, type the height (default 6'-0"). |
| Valley | Same, type the depth (default 6'-0"). |
| Raised Region | Type "Raise by" (default 2'-0"). |
| Lowered Region | Type "Lower by" (default 2'-0"). |
| Flat Region (Cut/Fill) | Click a polygon, `Enter`. No value needed. |

### Feature

| Variant | Gesture |
|---|---|
| Terrain Hole | Click a polygon, `Enter`. Cuts the surface (for a basement or building footprint). |
| Rectangular Feature, Kidney Shaped Feature, Spline Feature | (planned) |

### Roads, driveways, sidewalks

| Variant | Gesture |
|---|---|
| Polyline Road | Click a polyline, `Enter` or double-click, type the width (default 20'-0"). |
| Polyline Driveway | Same, default width 12'-0". |
| Polyline Sidewalk | Same, default width 4'-0". |
| Spline Road/Driveway/Sidewalk | (planned) |

Roads drape 0.5" above the terrain surface in the engine.

### Terrain menu commands

At the top of the Terrain menu:

| Command | Does |
|---|---|
| Create Terrain Perimeter | Starts the Terrain Perimeter tool. |
| Terrain Specification... | Opens the Terrain Specification (9.8). |
| Build Terrain | Builds the surface and contours. |
| Clear Terrain | Resets the terrain record (one undo step). "There is no terrain to clear" if none. |
| Make Terrain Hole Around Building | Cuts a hole in the surface 12" outside the building footprint. |

### Planned terrain groups

Garden Bed, Grass Region, Water Feature, Stepping Stone, Plant, Sprinkler, Terrain Wall and Curb
(Terrain menu items) are listed with Chief's names and dimmed (planned).

### Editing

With the Terrain tool active: `Delete` removes the element under the pointer. A double-click
outside a drawing opens the Terrain Specification, which Select Objects can also open. Typed values go through the inline field: `Enter`
accepts, an empty field takes the default shown, `Esc` cancels.

## 9.7 What the plan shows

The perimeter, elevation points and lines with their elevations, region and feature outlines,
road edges and, after Build Terrain, labeled contours. The surface is cached and rebuilt when the
record changes; the layer it draws on is set in the Terrain Specification.

The engine builds terrain and road meshes (`terrain_mesh`, `road_meshes`) with grass and
pavement stand-in materials, but the 3D view does not draw them yet (planned).

## 9.8 Dialog: Terrain Specification

| Tab | Fields |
|---|---|
| General | Subfloor height above terrain, Building pad elevation, Contour interval, Grid spacing, Smoothing passes. A line reports the Finished floor elevation. |
| Materials | Ground (Grass), Bare ground (Dirt): disabled. |
| Layer | The layer the terrain is drawn on. |

## 9.9 Differences from Chief

- Only Hole features change the surface; other features are listed in the engine but not placed.
- No Terrain Break lines, splines, or elevation reference points on the building.
- No site plan layout box for a plot plan yet (chapter 11).
