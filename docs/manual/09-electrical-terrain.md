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

Device kinds known to the engine (`plan-electrical`, 23 kinds): 110V Outlet, Quad Outlet, 220V Outlet,
GFCI Outlet, Floor Outlet, Switch, 3-Way Switch, 4-Way Switch, Dimmer Switch, Ceiling Light, Recessed Can,
Pendant Light, Wall Sconce, Ceiling Fan, Smoke Detector, CO Detector, Thermostat, Doorbell,
Data, Phone and TV Jacks, Panel and Rope Light. Every kind has its own plan symbol (the legend lists them all).

Default heights above the floor:

| Device | Height |
|---|---|
| Outlets (110V, quad, 220V, GFCI), data and phone jacks | 12" |
| Kitchen counter outlets (Auto Place Outlets) | 42" |
| Floor outlet | 0" |
| Switches (all kinds), doorbell, TV jack | 48" |
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
| 3-Way, 4-Way and Dimmer Switch | | Switch variants. |
| Quad Outlet, Floor Outlet | | Receptacle variants. |
| Recessed, Pendant and Wall Light | | Light variants (ceiling devices, the wall light on a wall). |
| Ceiling Fan, Smoke and CO Detector | | Ceiling and wall devices. |
| Thermostat, Doorbell, Data, Phone and TV Jack, Electrical Panel | | Wall devices. |
| Auto Place Switches | | One click: a switch 6" past the latch jamb of every door of every room, a ceiling light for a room without one, and the connections. Two doors in a room make a 3-way pair. |
| Electrical Connection | `E, C` | Click a switch (or outlet), then every light it controls (Esc ends the run). A dashed arc is drawn to each and stored. Clicking a second 3-way or 4-way switch wires the pair, and both then control the lights of either. Drag the square handle at the middle of an arc to bend it. |
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
- Kitchen counters get GFCI outlets at 42", at most 4' apart.
- Wet rooms and garages get GFCI outlets.

Auto Place Switches (above) places the room light and the switches.

## 9.3 Dialog: Electrical Service Specification

Double-click a device with the Electrical tool.

| Tab | Fields |
|---|---|
| General | Type (any kind of the same family, e.g. a duplex outlet becomes a GFCI), Height, Label, Circuit (a number, or blank) |
| Switches | For a switch: **Connected Lights and Outlets**, check boxes that add and remove connection arcs. For other devices: **Switched By**, the switches that control it |
| Materials | Plate or fixture finish (White, Ivory, Light Almond, Brown, Black, Stainless Steel), shown on the 3D plate or fixture |
| Label | Show label in plan, Label text, Text height (disabled) |
| Layer | The layer (disabled, `Electrical`) |

## 9.4 Circuits, schedules and legends (engine)

The engine groups devices into circuits (`assign_circuits`), counts devices by kind for a
schedule, and lists a legend of symbols. No menu uses the circuit and legend functions yet: circuit assignment and the electrical legend
are (planned; engine in `plan-electrical`). The **Electrical Schedule** from the Schedule flyout (chapter 11.2) lists the plan's devices (mark, type, label, mount height, circuit) as a live table placed in the plan.
Light fixtures also emit light in the ray tracer (chapter 10.13).
The schedule's Count column sums grouped rows.

### 9.4a Devices in 3D, connections and switches

Electrical devices are in the 3D view (`plan_electrical::electrical_meshes`, called by the 3D scene build, and the scene rebuilds when a device changes). They honor the
Electrical layer's display and each device's finish: cover **plates** with slots and toggles (outlets, switches, dimmers, jacks), **trim rings and dark apertures** for recessed cans,
flush ceiling fixtures, **pendants** with a cord and a shade, **ceiling fans**, **sconces**, **smoke and CO detectors**, the **panel box**, floor boxes and **rope-light** strips.
The plate or fixture takes the finish chosen on the Materials tab (White by default).

**Connections** (the dashed arcs from a switch or outlet to the lights it controls) have a handle at the middle of the arc: drag it to **bend** the arc (it is stored as the arc's
bulge, so a bent connection saves and undoes with the plan). **3-way and 4-way** switches are wired as pairs: click one, then the other, and both then control the lights of either.
**Auto Place Switches** puts a switch 6" past the latch jamb of every door of every room, a ceiling light in a room that has none, and the connections; a room with two doors gets a
3-way pair. The **Switches** tab of the Electrical Service Specification adds and removes connection arcs by tick box, also when the dialog is opened with Select Objects (it
opens from either tool the same way).

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

The three shaped features are **cut and fill pads**. The terrain under the outline is levelled to a flat top at
the mean ground under it plus the feature's height (4" by default; a negative height cuts the pad into the
ground), and the pad's sides slope back to the existing ground at the **side slope** (1 rise to 2 run by default).
The build finds the daylight line where each side meets the ground. Untick "Grade the terrain" in the feature's
dialog for the old behavior, a slab over the ground. A Terrain Hole cuts the surface away. A kidney or spline
outline keeps its control points: select it and drag them, and the curve follows (a smooth closed spline).
The cut and fill in cubic yards is listed in the Terrain Specification (Building Pad page), in the plan-docs
cut/fill table and in the Materials List under Landscaping.

**Building Pad** (command `TerrainVariant::BuildingPad`): levels the terrain under the building walls plus a
24" margin to the first floor's elevation less the *Terrain to first floor* distance of the Terrain
Specification, with sloped sides. The Building Pad page edits the margin, the slope and the first floor.

### Terrain Wall and Curb

| Variant | Gesture |
|---|---|
| Straight Terrain Wall, Straight Terrain Curb | Click a polyline. |
| Curved Terrain Wall, Curved Terrain Curb | Click the start and end, then a third click sets the bulge of the arc. |

A terrain wall follows the ground: its top is a set height above the terrain along the whole path (3'-0" for a
wall, 6" for a curb) and its bottom a footing depth below it (1'-0" and 4"). Thickness defaults to 8" and 6".

A wall also **cuts the terrain surface**. The strip under the wall is a gap with vertical faces, so contours stop
at the wall. The ground on the left of the drawing direction keeps its grade (the retained side) and the ground on
the right is lowered by the wall's *grade step* (2'-0" for a new wall, none for a curb), sloping back up to the
existing ground at 1:4. Untick "Cut the terrain along the wall" to leave the surface whole.

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

Roads drape 0.5" above the terrain surface. A road's specification has a **Crown** (how much higher the centerline sits than the edges, so the road sheds water to both sides) and **Curbs** with a **Curb height**; the roadway and its curb blocks are in the 3D scene.

### Site objects: North Pointer, Scale Bar, Building Pad

| Variant | Gesture |
|---|---|
| North Pointer | Click the center, then click toward true north; the distance between the clicks is the pointer's radius (12" to 100'). It draws a circle with an arrow toward north and an "N" beyond the tip, as CAD objects on the `Site Plan` layer, and **sets the plan's north angle** (to a tenth of a degree). Placing a second pointer replaces the first. One undo step. |
| Scale Bar | Click the start, then the end; the length is rounded to whole feet (at least one foot) and drawn as a bar in four parts (fewer when short) with a tick and a length label at each division and alternate parts hatched, on the `Site Plan` layer. |
| Building Pad | One click levels the terrain under the building walls (above). |

The north angle is also a field of the Terrain Specification (General). A true compass azimuth maps to a plan direction through it (`site_view::plan_sun_azimuth`), and the facing labels use the same angle.

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
Objects picks the whole terrain). **Select Objects picks single terrain objects** (features, breaks, walls, curbs, roads and landscape objects): click to select, drag to move, the arrow keys nudge, `Delete` removes, and a double-click opens the object's specification. An element with 40 points or fewer also shows vertex handles. The 3D view picks them too (chapter 10).

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
scene, and the scene rebuilds when the terrain changes. The 3D materials now include Grass, Mulch, Foliage, Water, Asphalt and Gravel, and the landscape uses them: the terrain surface and lawn or grass regions Grass, garden beds Mulch (or the bed's named material, such as Gravel), canopies Foliage, water Water (translucent), trunks Framing, edging, basins and stepping stones Stone, roads and driveways Asphalt, walls Concrete, Stone or Brick by the object's material.

## 9.8 Dialog: Terrain Specification

| Tab | Fields |
|---|---|
| General | Terrain to first floor (subfloor height above terrain), Building pad elevation, Grid spacing, Subdivision, Smoothing passes, North angle, and the "Rebuild the terrain after every edit" switch (off keeps the built surface until Build Terrain runs again; the plan then writes "Terrain out of date" at the perimeter). A line reports the Finished floor elevation. |
| Contours | Contour interval, Major contour every N, Label spacing (elevation text every N feet along the labeled contours, 0 = one per line), Label the major contours only. |
| Building Pad | Level the terrain under the building; margin, side slope and first floor elevation of the pad; the Cut and Fill table in cubic yards per pad with the total. |
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

- Sprinkler heads are not connected to a supply, and the 3D water surface is flat (the ripple is a plan fill).
- Build Terrain runs on the UI thread: it reports its stages (with a percentage) in the readout while it builds and ends with a triangle count and time, with no separate progress bar. The Materials page of the Terrain Specification is disabled, and an elevation point has no
  dialog of its own (delete and place it again, or drag it).
- Plants are terrain-owned runs, not placed library symbols: the Plant Schedule (chapter 11.2) does not list them and Replace
  From Library does not see them.
- North Pointer and Scale Bar are CAD objects on the "Site Plan" layer (Terrain tool); the pointer sets the plan's
  north angle, which the sun angle reads through `site_view::plan_sun_azimuth`.
- No site plan layout box for a plot plan yet (chapter 11).
