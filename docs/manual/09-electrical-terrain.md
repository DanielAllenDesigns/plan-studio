# Chapter 9: Electrical and Terrain

Two site-and-systems families share this chapter: the electrical plan (outlets,
switches, lights, connections) and the terrain (lot, elevations, hills, roads,
driveways, walls, garden beds, plants). Both store their data in typed fields of the plan (a floor's `electrical`
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

# Part B: Terrain and landscaping

## 9.5 How terrain works

A terrain is one object per plan: a **perimeter** (the lot), **elevation data** (points, lines, splines,
regions and break lines), **modifiers** (hills, valleys, raised and lowered and flat regions), **features**
(terrain holes under buildings, and slab-like rectangular, kidney-shaped and spline features), **road strips**
(roads, driveways, sidewalks, straight or spline), **terrain walls and curbs**, and the **landscape objects**
(garden beds, grass regions, water features, stepping stones, plant runs and sprinkler runs).

**Build Terrain** (Chief's command of the same name):

1. Samples a regular grid over the perimeter and interpolates elevations from your data
   (inverse-distance weighting).
2. Applies the modifiers.
3. Triangulates the grid with a Delaunay triangulation, clips to the perimeter and the holes.
4. Smooths the result (Laplacian passes). The vertices of a **Terrain Break** are held fixed, so the
   crease survives the smoothing.
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
moves it into the slot and removes the layer. Files from before the landscape objects load unchanged (every
new list defaults to empty).

## 9.6 Terrain tools

All terrain tools are on the Terrain menu (Elevation Data, Modifier, Feature, Terrain Wall and Curb, Garden Bed,
Grass Region, Water Feature, Stepping Stone, Road, Driveway, Sidewalk, Plant and Sprinkler submenus) and in the
same flyouts of the toolbars. Every entry starts the one Terrain tool and the choice is passed through to it.
Daniel's Terrain Configuration toolbar is (planned).

Typed values go through an inline field: `Enter` accepts, an empty field takes the default shown, `Esc` cancels.
In every polyline gesture `Enter` or a double-click ends the line; in every polygon gesture `Enter` closes it.

### Elevation Data

| Variant | Gesture |
|---|---|
| Terrain Perimeter | Click the corners of the lot; `Enter`, double-click or a click on the first point closes it. One per project. |
| Elevation Point | Click, then type the elevation in the inline field and press `Enter`. |
| Elevation Line | Click a polyline; `Enter` or double-click ends it; then type the elevation. |
| Elevation Region | Click a polygon; `Enter` closes it; type the elevation. |
| Elevation Spline | Like Elevation Line, with the clicks as control points of a smooth curve. |
| Terrain Break | A polyline, then the elevation it is held at. The surface keeps a crease along it. |
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
| Rectangular Feature | Drag a rectangle, or click two corners. |
| Kidney Shaped Feature | Click both ends of the long axis, then a third click sets the width. |
| Spline Feature | Click control points of a closed spline; `Enter` closes it. |

The three shaped features are slabs 4" thick over the surface (set the height in the dialog). They are
not cut or fill: the ground under them does not change. Only Terrain Hole features cut the surface.

### Terrain Wall and Curb

| Variant | Gesture |
|---|---|
| Straight Terrain Wall, Straight Terrain Curb | Click a polyline. |
| Curved Terrain Wall, Curved Terrain Curb | Click the start and end, then a third click sets the bulge of the arc. |

A terrain wall follows the ground: its top is a set height above the terrain along the whole path (3'-0" for a
wall, 6" for a curb) and its bottom a footing depth below it (1'-0" and 4"). Thickness defaults to 8" and 6".

### Landscaping

| Variant | Gesture |
|---|---|
| Polyline Garden Bed, Polyline Grass Region, Polyline Water Feature | Click the corners of a polygon; `Enter` closes it. |
| Kidney Garden Bed, Kidney Grass Region | Click both ends of the long axis, then a third click sets the width. |
| Spline Garden Bed, Spline Grass Region, Spline Water Feature | Click control points of a closed spline. |
| Polyline Stepping Stone, Spline Stepping Stone | Click a path (control points for the spline); stones are laid along it. |
| Polyline Plant, Spline Plant | Click a path; plants of the chosen kind are laid along it at one canopy width apart. |
| Polyline Sprinkler, Spline Sprinkler | Click a path; sprinkler heads are laid along it. |

Objects take their sizes from defaults, and the specification (below) edits them. A **plant run** stores a catalog
plant (the built-in Plants catalog of the Library Browser, boxwood by default), its canopy width and its height;
the plan draws the canopies and 3D draws a shrub or, for a plant 8' tall or more, a tree on a trunk. Splines and
kidney shapes are stored flattened as polylines.

### Roads, driveways, sidewalks

| Variant | Gesture |
|---|---|
| Polyline Road | Click a polyline, `Enter` or double-click, type the width (default 20'-0"). |
| Polyline Driveway | Same, default width 12'-0". |
| Polyline Sidewalk | Same, default width 4'-0". |
| Spline Road, Spline Driveway, Spline Sidewalk | The same, with the clicks as control points of a curve. |

Roads drape 0.5" above the terrain surface.

### Terrain menu commands

At the top of the Terrain menu:

| Command | Does |
|---|---|
| Create Terrain Perimeter | Starts the Terrain Perimeter tool. |
| Terrain Specification... | Opens the Terrain Specification (9.8). |
| Build Terrain | Builds the surface and contours. |
| Clear Terrain | Resets the terrain record (one undo step). "There is no terrain to clear" if none. |
| Make Terrain Hole Around Building | Cuts a hole in the surface 12" outside the building footprint. |

### Editing

With a Terrain tool active: `Delete` removes the element under the pointer (one undo step), the arrow keys
nudge it, and a double-click on an element that has a specification opens it; a double-click outside any
element opens the Terrain Specification, which Select Objects can also open (there is one terrain, so Select
Objects picks the whole terrain). **Select Objects cannot pick or move one terrain object yet**, and the 3D
view cannot pick them either; both are planned (Round 9).

## 9.7 What the plan shows

The perimeter, elevation points and lines with their elevations, region and feature outlines,
road edges and, after Build Terrain, labeled contours. The landscape objects draw on layers of their own, which
the program adds to the plan when you draw the first object:

| Layer | Holds |
|---|---|
| `Terrain, Features` | Rectangular, kidney and spline features |
| `Terrain, Walls` | Terrain walls and curbs |
| `Terrain, Breaks` | Terrain breaks |
| `Landscaping, Garden Beds`, `Landscaping, Grass Regions`, `Landscaping, Water Features`, `Landscaping, Stepping Stones` | The matching regions and stones |
| `Plants`, `Sprinklers` | Plant runs and sprinkler runs |

Each object can name another layer in its specification. The surface is cached and rebuilt when the record
changes; the layer the terrain itself draws on is set in the Terrain Specification.

**In 3D** (Round 8): the terrain surface, the roads draped on it and the landscape objects are in the 3D
scene, and the scene rebuilds when the terrain changes. plan-3d has no grass, mulch or foliage material, so the
stand-ins are: grass, mulch and canopies the brown Floor material; trunks Framing; bed edging, stepping stones and
water basins Stone; water translucent window glass; walls Concrete, Stone or Brick by the object's material. A
green material is (planned; Round 9 landscape materials).

## 9.8 Dialog: Terrain Specification

| Tab | Fields |
|---|---|
| General | Subfloor height above terrain, Building pad elevation, Contour interval, Grid spacing, Smoothing passes. A line reports the Finished floor elevation. |
| Materials | Ground (Grass), Bare ground (Dirt): disabled. |
| Layer | The layer the terrain is drawn on. |

### Dialog: terrain object specifications

Double-click a feature, break, wall, curb, landscape object, road or elevation line with a Terrain tool. The title
names the object (Terrain Feature, Terrain Break, Terrain Wall, Terrain Curb, Garden Bed, Grass Region, Water Feature,
Stepping Stone, Plant, Sprinkler, Road or Elevation Line Specification). OK is one undo step.

| Tab | Fields |
|---|---|
| General | Feature: Material, Height above ground. Break and elevation line: Elevation. Wall or curb: Type, Material (Concrete, Stone, Brick, Grass), Top above terrain, Bottom below terrain, Thickness. Road: Type, Width, Curbs. Garden bed: Material (Mulch, Soil, Stone), Mulch depth, Edging and Edging height. Grass region: Height above ground. Water feature: Water level below grade, Depth of water, Stone edge and Edge width. Stepping stones: Stone size, Spacing, Thickness. Plant: the plant, Canopy width, Height, Spacing. Sprinkler: Spray radius, Head spacing, Spray angle, Riser height |
| Line Style | A color of its own, line weight, dashed (everything but roads and elevation lines) |
| Fill Style | A fill color of its own (regions, features and walls) |
| Layer | The layer name |

Roads and elevation lines show General only; a path object (break, stepping stones, plants, sprinklers) has General,
Line Style and Layer.

## 9.9 Differences from Chief

- Terrain walls and curbs do not cut the surface or stop contours; features are slabs over the surface, not cut and fill.
- Water has no ripple fill, sprinkler heads are not connected to a supply, and a kidney shape is one fixed blob
  (axis, width, notch) rather than a freely edited curve.
- Plants are terrain-owned runs, not placed library symbols: the Plant Schedule (chapter 11.2) does not list them and Replace
  From Library does not see them.
- No elevation reference points on the building.
- No site plan layout box for a plot plan yet (chapter 11).
