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

Device kinds known to the engine (`plan-electrical`, 28 kinds): 110V Outlet, Quad Outlet, 220V Outlet,
GFCI Outlet, Floor Outlet, WP Outlet, Dedicated Outlet, Switch, WP Switch, 3-Way Switch, 4-Way Switch, Dimmer Switch, Ceiling Light, Recessed Can,
Pendant Light, Wall Sconce, Exterior Wall Light, Path Light, Ceiling Fan, Smoke Detector, CO Detector, Thermostat, Doorbell,
Data, Phone and TV Jacks, Panel and the older straight-strip Rope Light. New rope lights are paths (9.2a), not devices. Every kind has its own plan symbol (the legend lists them all).

Receptacle symbols carry their flags as Chief draws them: the 110V duplex is a circle with two
blades; the **220V** outlet has three blades and a `220V` tag; the **GFCI** outlet has the `GFCI` tag; the
**WP** (weatherproof, exterior) outlet is a GFCI duplex with a `WP` tag; the **Dedicated** outlet is a single
blade with a solid hub and a `DED` tag. A 3-way switch is the S symbol with a `3` and a 4-way switch the S
symbol with a `4` (S3, S4). Schedules list the voltage and flags with the type (`110V, GFCI, WP`).

Default heights above the floor are the plan's **Electrical Defaults** (Edit > Default Settings > Electrical; double-click any
Electrical Tools button), in four groups with a **Use Default Heights** switch:

| Group | Applies to | Default |
|---|---|---|
| Outlet | every receptacle (WP too), phone, data and TV jacks | 12" |
| Switch | switches, doorbells, thermostats | 48" |
| Above Base Cabinet | switches and outlets placed on the wall over a base cabinet, measured up from the counter top | 8" (44" over a 36" counter) |
| On Cabinet Side | switches and outlets placed on the side of a cabinet or soffit, measured up from its bottom, kept on the box | 32" |

Everything else keeps the height saved with its symbol: wall sconce and exterior wall light 66", the panel and CO detector 60",
path light 18", pendant and the old rope strip 84", floor outlet 0", ceiling devices at the ceiling.
With Use Default Heights off every device takes its symbol height. A height can also be set per device
(Electrical Service Specification) and Set as Default copies a placed device's type and height back into the defaults. Plans saved with the
older per-kind heights read into the groups (110V Outlet becomes Outlet, Switch stays Switch, the Counter Outlet minus 36" becomes Above Base Cabinet).
The defaults are saved in the plan and undo with the dialog's OK.

Wall devices sit on the wall face (half the wall thickness off the centerline), facing into
the room (outside the house for WP outlets). Ceiling devices are placed freely.

**Default Library Objects** (the same page) choose which symbol each tool places: the Light tool a ceiling light,
recessed can or pendant; the 110V tool a duplex or quad outlet; the 220V tool a 220V or dedicated outlet; the Switch tool a single-pole or
dimmer switch; the Wall Light tool and its Exterior counterpart. The **Electrical Connection** tab sets the curvature ratio (0.2 by default:
the first arc sags a fifth of its length), line style, arrow and label of new splines; the **Rope Light** tab holds the Rope Light Specification fields.

## 9.2 Tools

### Electrical Tools (row 2, Electrical flyout; Build > Electrical)

| Variant | Hotkey | What a click does |
|---|---|---|
| 110V Outlet | `E, O` | Reads the click (9.2a): on a wall within 12", on the face nearest the cursor; over a base cabinet above the counter; on a cabinet side; weatherproof outdoors. The status bar shows the height the click would use. |
| 220V Outlet | `Ctrl+Alt+Cmd+7` | Same, 220V symbol (it stays 220V outdoors). |
| GFCI Outlet | `Ctrl+Alt+Shift+Cmd+Y` | Same, GFCI symbol. |
| Light | `E, L` | Near a wall (within 12") a wall light, inside a soffit a ceiling light on its bottom, away from walls a ceiling light that snaps to a room's center within 12", outside every room a path light. |
| Rope Light | `Ctrl+Alt+Shift+Cmd+A` | Click and drag to draw a rope light path (9.2b). |
| Switch | `E, S` | Places a switch on a wall at 48", over a counter or on a cabinet side by the height groups, a WP switch outdoors. |
| 3-Way, 4-Way and Dimmer Switch | | Switch variants. |
| Quad Outlet, Floor Outlet | | Receptacle variants. |
| Recessed, Pendant and Wall Light | | Light variants (ceiling devices, the wall light on a wall). |
| Ceiling Fan, Smoke and CO Detector | | Ceiling and wall devices. |
| Thermostat, Doorbell, Data, Phone and TV Jack, Electrical Panel | | Wall devices. |
| Auto Place Switches | | One click: a switch 6" past the latch jamb of every door of every room, a ceiling light for a room without one, and the connections. Two doors in a room make a 3-way pair. |
| WP Outlet, Dedicated Outlet | | Wall receptacles: weatherproof GFCI (18") and a single receptacle on its own circuit. |
| Electrical Connection | `E, C` | Click a switch (or outlet), then every light it controls (Esc ends the run), or press on any object and drag to the next (or to open ground for a free spline). A dashed spline is drawn to each and stored (9.2c). When a light is controlled by **two or more switches** they become multi-way: the first and last are 3-way (S3) and any in between 4-way (S4), and the symbols change as you wire; removing a connection or a switch turns them back. Clicking a second 3-way or 4-way switch wires the pair, and both then control the lights of either. |
| Auto Place Outlets | `E, A, O` | One click places outlets for every room of the floor, from the room names and types, and the exterior weatherproof outlets. One undo step. |

All of them start the one Electrical tool, and the Electrical flyout does pass your choice
through to it.

### 9.2a Where a click lands

The 110V, GFCI and 220V outlet tools, the Switch tool and the Light tool read the click (manual pp. 693-694):

- **On a wall** within 12": a wall device on the face nearest the cursor at the Outlet or Switch height. If the other side of that face is outdoors (an exterior wall with no room beyond it, or a room with an exterior type such as a Deck), a 110V or GFCI outlet is the **WP outlet**, a switch the **WP switch** and a wall light the **exterior wall light**; a 220V outlet stays a 220V outlet.
- **Behind a base cabinet**: the height is measured up from the counter top (Above Base Cabinet); a 110V outlet in a Kitchen or Bath room is a GFCI there. Behind a **sink base** the plain Outlet height.
- **On the side of a cabinet or soffit** (nearer than the wall): 32" up from its bottom, kept on the box; the device remembers its cabinet.
- **Away from walls in a room**: an outlet goes on the floor (a 110V outlet in a Garage or Slab room on the ceiling), a light on the ceiling. A switch needs a wall.
- **Outside any room, or in an exterior room, away from walls**: a weatherproof outlet on the floor, or a path light.

The other flavors (quad, floor, WP, dedicated, jacks, detectors, thermostat...) place their own kind: wall devices on the nearest wall, ceiling devices freely.

### 9.2b Rope lights

A rope light is a path with regularly spaced light sources, not a device. Click and drag to draw its first segment; it is then selected and edited like an
open polyline: drag a vertex, drag the small circle at the middle of a segment to add a vertex, double-click a vertex to remove it, `Delete` removes the rope light.
A rope light that starts under a wall cabinet hangs from the cabinet's bottom. Double-click opens the **Rope Light Specification**: General (Elevation
Reference, Height to the top of the profile, Distance Between Lights, Center Lights, Show Lights, Light Display Size, Treat as One Object), Polyline
(length, area, volume, lines), Strip Profile, Layer, Schedule. Rope lights set to Treat as One Object are a line of the Electrical Schedule and are listed
in the Materials List under Electrical by length (`lf`). The runs of a tray ceiling's Rope Lights panel are available as closed rope light paths
(`tools::electrical::tray_rope_lights`).

### 9.2c Electrical Connection splines

A connection is a spline: an arc of two segments through a middle vertex when first drawn, then any number of vertices (a smooth curve through them).
With the Electrical Connection tool, or with a connection selected:

- Square handles sit on the vertices; drag one to reshape (the other vertices stay), double-click the curve to add a vertex, double-click a vertex to remove it.
- Drag an end off its object to **detach** it: the spline stays with a free end and the light it controlled is no longer switched by it. Drop an end on an object to attach it.
- A selected switch, outlet or light has the diamond-shaped handle below its symbol: drag it to another object to draw a spline.
- **Reset Curvature** (Edit toolbar) removes the vertices and restores the original direction with the curvature ratio of the Electrical Connection Defaults.
- Wiring a switch to a light with a spline makes the light switched by it; two switches on one light become 3-way and three or more 3-way/4-way, unless the switch has Automatically Change Switch Type When Wiring turned off (Options tab).

### Placing and editing

- Move the pointer: a ghost shows where the device will go. Wall devices snap to the nearest
  wall within 12" and flip to the face nearest the cursor.
- Click an existing device (with the Electrical tool or Select Objects) to select it. **Drag** to move it:
  wall devices slide along their wall; free devices move freely. `Tab` flips a wall device to the other side of
  the wall. `Left` and `Right` turn a free device (hold `Shift` for 90 degrees). `Delete` removes it.
  Double-click or `Enter` opens the Electrical Service Specification. With a device selected the Edit toolbar
  adds **Flip Side** and **Rotate Device**.

### Auto Place Outlets rules

These follow NEC 210.52:

- Every wall space of 2' or more gets outlets; no point along a wall is more than 6' from one (outlets at most 12' apart, none farther than 6' from a doorway edge or a corner of its wall space).
- Outlets stay 6" clear of door jambs.
- Kitchens get GFCI outlets at the counter height (44") at most 4' apart.
- Baths, laundries and garages get GFCI outlets; a bath whose walls are all shorter than 2' still gets one, on its longest wall.
- Exterior: one WP GFCI outlet on the outside face of the longest exterior wall (the front) and one on the opposite side of the house (the back), in the widest stretch clear of doors, at the Outlet height.
- Heights come from the Electrical Defaults (Outlet 12", counter 44" over a 36" counter, exterior WP at the Outlet height).
- The room's type decides what it gets: exterior rooms (decks, balconies, courts), Porches and Open Below rooms get none; other hybrid rooms such as a garage or slab get half as many (the spacing doubles, a garage still has one per bay).
- Over base cabinets in kitchens and baths the outlets are GFCI at the Above Base Cabinet height. A kitchen also gets plain outlets at the Outlet height on the wall spaces the cabinets leave free; a bath with a vanity gets outlets only over it.
- Outlets already in place are not duplicated.

Auto Place Switches (above) places the room light and the switches.

## 9.3 Dialog: Electrical Service Specification

Double-click a device with the Electrical tool.

| Tab | Fields |
|---|---|
| General | Type (any kind of the same family, e.g. a duplex outlet becomes a GFCI), Voltage and Flags (110V / 220V, GFCI, WP, Dedicated; read-only, set by the type), Height to Center / Bottom / Top, Size (Width, Height, Retain Aspect Ratio), Use as default height for its group, Default Heights (the four groups and Use Default Heights), Label, Circuit (a number, or blank) |
| Options | Mounting (Wall, Floor, Ceiling, Cabinet Side), Recess (Distance from Wall, negative sets it into the wall; Cuts Floor, Ceiling, Wall; Cut Depth; Insert Depth), Automatically Change Switch Type When Wiring |
| Switches | For a switch: **Connected Lights and Outlets**, check boxes that add and remove connection arcs. For other devices: **Switched By**, the switches that control it |
| Materials | Plate or fixture finish (White, Ivory, Light Almond, Brown, Black, Stainless Steel), shown on the 3D plate or fixture |
| Label | Show label in plan, Label text, Text height (disabled) |
| Layer | The layer (disabled, `Electrical`) |

## 9.4 Circuits, schedules and legends (engine)

The engine groups devices into circuits (`assign_circuits`), counts devices by kind for a
schedule, and lists a legend of symbols. No menu uses the circuit and legend functions yet: circuit assignment and the electrical legend
are (planned; engine in `plan-electrical`). The **Electrical Schedule** from the Schedule flyout (chapter 11.2) lists the plan's devices (mark, type, label, mount height, circuit) as a live table placed in the plan. `schedule_rows` gives the same lines with the voltage and flags (`E-01`, `GFCI Outlet`, 12", circuit 4, `110V, GFCI`).
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
3-way pair (and the S3 / S4 symbols follow the wiring, see Electrical Connection above). WP outlets are modeled with a deeper in-use cover with a dark flap opening. Every wall device is built on the face of its host wall and faces out of it, so a plate follows the wall's thickness and side. The **Switches** tab of the Electrical Service Specification adds and removes connection arcs by tick box, also when the dialog is opened with Select Objects (it
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
| Polyline Feature | Click the corners of a polygon; `Enter` closes it. |
| Round Feature | Click the center, then a point on the circle (or drag from the center); the distance is the radius. |

The five shaped features (rectangular, kidney, spline, polyline and round) are **cut and fill pads**, each with its own height and material. The terrain under the outline is levelled to a flat top at
the mean ground under it plus the feature's height (4" by default; a negative height cuts the pad into the
ground), and the pad's sides slope back to the existing ground at the **side slope** (1 rise to 2 run by default).
The build finds the daylight line where each side meets the ground. Untick "Grade the terrain" in the feature's
dialog for the old behavior, a slab over the ground. A Terrain Hole cuts the surface away. A kidney or spline
outline keeps its control points: select it and drag them, and the curve follows (a smooth closed spline). A round feature keeps its center and radius: any of its handles drags the edge and sets the radius, and the Round Feature specification has a Radius field.
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

A terrain wall sits on top of the terrain and follows it: its top is a set height above the terrain along the whole path (5'-0" for a
wall, 6" for a curb) and its bottom a footing depth below it (1'-0" and 4"). Thickness defaults to 8" and 6".

A wall also **cuts the terrain surface**. The strip under the wall is a gap with vertical faces, so contours stop
at the wall. The ground on the left of the drawing direction keeps its grade (the retained side) and the ground on
the right is lowered by the wall's *grade step* (none for a new wall or curb; set it in the specification), sloping back up to the
existing ground at 1:4. Untick "Cut the terrain along the wall" to leave the surface whole.

**Stepped top** is an option of the specification: in 3D the top then holds level and drops in whole courses (8" by default; the *Course height*) as the ground falls, with a vertical riser at each step, instead of following every bump of the ground. A new wall and a curb are not stepped.

**Straight Retaining Wall and Curved Retaining Wall** (Terrain Wall and Curb flyout) make a **Terrain Break plus a wall** in one undo step. The program reads the ground 3' either side of the wall, turns the path round when needed so the **high side is on its left** (the retained side), puts the wall's top at the high side and its bottom a 1'-0" footing below the low side, and sets the wall's grade step to the drop between the two sides. The break follows the ground (it holds no elevation of its own), so the contours stay sharp at the wall. On flat ground a retaining wall is a concrete strip.

### Landscaping

| Variant | Gesture |
|---|---|
| Polyline Garden Bed, Polyline Grass Region, Polyline Water Feature | Click the corners of a polygon; `Enter` closes it. |
| Kidney Garden Bed, Kidney Grass Region | Click both ends of the long axis, then a third click sets the width. |
| Spline Garden Bed, Spline Grass Region, Spline Water Feature | Click control points of a closed spline. |
| Polyline Stepping Stone, Spline Stepping Stone | Click a path (control points for the spline); stones are laid along it. |
| Polyline Plant, Spline Plant | Click a path; plants of the chosen kind are laid along it at one canopy width apart. Pick the plant in the **Plant Chooser** of the Plant Specification. |
| Polyline Sprinkler, Spline Sprinkler | Click a path; sprinkler heads are laid along it. |

Objects take their sizes from defaults, and the specification (below) edits them. A **plant run** stores a catalog
plant (the built-in Plants catalog of the Library Browser, boxwood by default), its canopy width and its height;
the plan draws the canopies. In 3D a plant is built in the **3D form** the specification names: *Automatic* (a cone for a conifer
such as a pine, spruce, cedar or cypress, a round shrub otherwise, and a tree on a trunk when the plant is 8' tall or more), *Round canopy*, *Cone (evergreen)* or *Billboard*
(two crossed upright planes, cheap to draw). The **Plant Chooser** (the "Plant Chooser..." button of the Plant Specification) lists the Plants catalog of the Library Browser by category (Trees > Deciduous, Trees > Evergreen,
Shrubs, Grasses and Perennials, ...) with a search field; a click sets the plant, its canopy width, its height, the spacing and the form. The plants come from the built-in catalog; no catalog files are bundled. Splines and
kidney shapes are stored flattened as polylines.

### Roads, driveways, sidewalks

| Variant | Gesture |
|---|---|
| Straight Road | Click a polyline, `Enter` or double-click, type the width (default 20'-0"). |
| Straight Driveway | Same, default width 12'-0". |
| Straight Sidewalk | Same, default width 4'-0". |
| Spline Road, Spline Driveway, Spline Sidewalk | The same, with the clicks as control points of a curve. |
| Polyline Road, Polyline Driveway, Polyline Sidewalk, Median | Click the corners of an outline, `Enter` closes it (see Round 15 below). |
| Cul-de-sac, Auto Generate Sidewalk | Click on the end of a road; click a road and type the offset. |
| Polyline Road Marking, Spline Road Marking | A painted stripe: click a polyline, `Enter`, type the line width (default 4"). Dashed, color and layer are in the specification. |

A **road marking** (stripe) lies on whatever is under it: on the ground, or on the crown of a road it crosses (0.3" above the surface), in white paint in 3D; a dashed marking is cut into 10' dashes with 20' gaps. The plan draws it as a line of its true width in traffic yellow (or the color you pick). A road's *Material* (Asphalt, Concrete, Gravel, Brick, Stone) sets its 3D surface.

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
| Clear Terrain | Removes what Build Terrain generated (see Round 15 below); "There is no terrain to clear" if none. |
| Make Terrain Hole Around Building | Cuts a hole in the surface 12" outside the building footprint. |
| Site Objects > Import Terrain Data... , Import GPS Data... (also File > Import) | Open the Import Terrain Assistant and the Import GPS Data Assistant (9.8a). |
| Site Objects > Terrain Cut and Fill Report... | Opens the report of the soil every graded pad moves (9.8a). |

### Editing

With a Terrain tool active: `Delete` removes the element under the pointer (one undo step), the arrow keys
nudge it, and a double-click on an element that has a specification opens it; a double-click outside any
element opens the Terrain Specification, which Select Objects can also open (there is one terrain, so Select
Objects picks the whole terrain). **Select Objects picks single terrain objects** (features, breaks, walls, curbs, roads and landscape objects): click to select, drag to move, the arrow keys nudge, `Delete` removes, and a double-click opens the object's specification. An element with 40 points or fewer also shows vertex handles. The 3D view picks them too (chapter 10). **Every terrain object has a specification of its own**: the elevation point, region and modifier too (Hill, Valley, Raised Region, Lowered Region and Flat Region), and a Terrain Hole; the Terrain Perimeter opens the Terrain Specification. **Terrain Perimeter editing**: select the perimeter (`ObjectRef::TerrainObject(Perimeter)`) and drag a corner handle ("Reshape Terrain Element"), or nudge it with the arrow keys from a Terrain tool.

## 9.7 What the plan shows

The perimeter, elevation points and lines with their elevations, region and feature outlines,
road edges and, after Build Terrain, labeled contours. The landscape objects draw on layers of their own, which
the program adds to the plan when you draw the first object:

| Layer | Holds |
|---|---|
| `Terrain, Features` | Rectangular, kidney, spline, polyline and round features |
| `Terrain, Walls` | Terrain walls and curbs |
| `Terrain, Breaks` | Terrain breaks |
| `Landscaping, Garden Beds`, `Landscaping, Grass Regions`, `Landscaping, Water Features`, `Landscaping, Stepping Stones` | The matching regions and stones |
| `Plants`, `Sprinklers` | Plant runs and sprinkler runs |

Each object can name another layer in its specification. The surface is cached and rebuilt when the record
changes; the layer the terrain itself draws on is set in the Terrain Specification.

**In 3D** (Round 8): the terrain surface, the roads draped on it and the landscape objects are in the 3D
scene, and the scene rebuilds when the terrain changes. The 3D materials now include Grass, Mulch, Foliage, Water, Asphalt and Gravel, and the landscape uses them: the terrain surface and lawn or grass regions Grass, garden beds Mulch (or the bed's named material, such as Gravel), canopies Foliage, water Water (translucent), trunks Framing, edging, basins and stepping stones Stone, roads and driveways Asphalt, walls Concrete, Stone or Brick by the object's material.

**Round 14**: the terrain surface takes the *Ground* material of the Terrain Specification (Grass, Dirt, Gravel, Stone, Mulch, Concrete); road strips take their own material, road markings are paint (white) lying on the road's crown, retaining walls step, conifers are cones and billboards are crossed planes; the roads follow the built surface (TIN) at every sample.

## 9.8 Dialog: Terrain Specification

| Tab | Fields |
|---|---|
| General | Terrain to first floor (subfloor height above terrain), Building pad elevation, "Level the terrain under the building automatically", Contour interval, Grid spacing, Subdivision, Smoothing passes, North angle, and the "Rebuild the terrain after every edit" switch (off keeps the built surface until Build Terrain runs again; the plan then writes "Terrain out of date" at the perimeter). A line reports the Finished floor elevation. |
| Contours | Contour interval, Major contour every N, Label spacing (elevation text every N feet along the labeled contours, 0 = one per line). **Primary lines** (the majors) and **Secondary lines** (between them): a color of their own, line weight (0 takes the default of 1 pt and 0.35 pt), Dashed, with a sample of the line; the primary lines are always labeled, the secondary ones when "Labeled with their elevation" is ticked. |
| Building Pad | Level the terrain under the building; margin, side slope and first floor elevation of the pad; the Cut and Fill table in cubic yards per pad with the total. |
| Materials | Ground (Grass, Dirt, Gravel, Stone, Mulch, Concrete): the material of the terrain surface in 3D. Bare ground (Dirt, Gravel, Mulch, Stone): stored for the cut slopes of graded pads. |
| Layer | The layer the terrain is drawn on. |

### 9.8a Dialogs: Import Terrain Data and Terrain Cut and Fill Report

**Import Terrain Data** (Terrain > Site Objects) reads survey points as elevation points. *Choose File...* reads a DXF, GPX or text file, or paste the text into the box. Formats (parsed by Plan Studio itself):

| Format | Reads |
|---|---|
| DXF (ASCII) | `POINT`, `3DFACE`, the `VERTEX` records of a `POLYLINE` and the vertices of an `LWPOLYLINE` (at its elevation). The unit is the drawing's `$INSUNITS` when it has one (feet if not). |
| GPX | Waypoints, route points and track points with `lat`, `lon` and `<ele>` (meters), projected onto a flat local plan around the first point (east is plan x, north is plan y). |
| XYZ text | One point per line, `x y z` separated by spaces, commas, semicolons or tabs; a leading point number and header lines are skipped. |

*Coordinates in* overrides the unit (Feet, Inches, Meters, Millimeters). "Center the points on the terrain perimeter" moves the survey to the middle of the lot (handy for GPX and for surveys in a state-plane coordinate system) and "Make the lowest point elevation 0" lowers the survey to the plan's datum. *Add to terrain* reports how many points were read and added (points that fall on an existing point are skipped); OK stores them as one undo step "Terrain Specification", Cancel keeps nothing.

**Terrain Cut and Fill Report** lists every graded pad (the building pad and each feature that grades the terrain) with the elevation of its top, its area, and its cut and fill in cubic yards (the pad and its sloped sides together), the totals, and the soil to haul away or bring in. *Copy as CSV* puts the table on the clipboard. The report stores nothing.

### Dialog: terrain object specifications

Double-click any terrain object with a Terrain tool (or select it and press `Enter`). The title
names the object (Terrain Feature, Terrain Hole, Terrain Break, Terrain Wall, Terrain Curb, Garden Bed, Grass Region, Water Feature,
Stepping Stone, Plant, Sprinkler, Road, Driveway, Sidewalk, Road Marking, Elevation Line, Elevation Point, Elevation Region, Hill, Valley, Raised Region, Lowered Region or Flat Region Specification). OK is one undo step.

| Tab | Fields |
|---|---|
| General | Feature: Shape, Radius (round), Material, Height above ground or the graded pad's height and side slope. Terrain Hole: a note only. Break and elevation line: Elevation. Elevation point: Elevation and position. Elevation region: Elevation. Modifier: Hill height, Valley depth, Raise by or Lower by (none for a Flat Region). Wall or curb: Type, Material (Concrete, Stone, Brick, Grass), Top above terrain, Bottom below terrain, Thickness, Stepped top and Course height. Road: Type (Road, Driveway, Sidewalk, Marking), Material, Width, Crown, Curbs; Marking: Line width, Dashed, Color. Garden bed: Material (Mulch, Soil, Stone), Mulch depth, Edging and Edging height. Grass region: Height above ground. Water feature: Water level below grade, Depth of water, Stone edge and Edge width. Stepping stones: Stone size, Spacing, Thickness. Plant: the plant and the Plant Chooser, Canopy width, Height, Spacing, 3D form. Sprinkler: Spray radius, Head spacing, Spray angle, Riser height |
| Line Style | A color of its own, line weight, dashed (everything but roads and elevation lines) |
| Fill Style | A fill color of its own (regions, features and walls) |
| Layer | The layer name |

Elevation lines, points, regions, modifiers and holes show General only; a road shows General and Layer; a path object (break, stepping stones, plants, sprinklers) has General,
Line Style and Layer.

### Round 15: the rest of the Terrain Specification

The **General** page now follows Chief's Absolute Elevation. *Absolute elevation: automatic* (the default) lets the program set the distance between Floor 1 and the terrain (6"; the field shows it). Unticked, you choose **Retain surface elevation at: Reference Point or Contour 0**, type *Surface at Reference Point* (or *at Contour 0*, the vertical distance from the Floor 1 subfloor to the surface, normally negative) and the *Floor 1 subfloor elevation*. The whole terrain then moves vertically so that the surface at that place sits where you said, which lets real survey elevations (1,000 ft above sea level) sit under a Floor 1 that starts at 0. The **Terrain Elevation Reference Point** has its own tools in the Elevation Data flyout (*Terrain Elevation Reference Point* places it by a click, *Remove Terrain Elevation Reference Point* takes it away); the plan draws a ringed cross at it while the perimeter is selected, and the page has X and Y fields. Without a placed point the middle of the perimeter's box is used.

Also on General: *Hide terrain intersected by building* (no surface or contours inside the building footprint); **Skirt** (a wall hanging from the terrain edge in 3D: thickness, *Flat base* below the lowest point or *Follow terrain* at a constant distance under the surface; its material is on the Materials page); **Terrain surface smoothing** (*Linear*: flat triangles between the data points; *Low*, *Medium*, *High*; or a custom number of passes); **Triangle count** (*Low* 1000, *Medium* 2000, *High* 4000, *Custom*, or the grid spacing with a *Maximum triangle size*) with a readout of the estimate and of the triangles of the last build; and the *Season* plant images are shown in.

**Contours** adds *Offset* (lines fall at the offset plus whole intervals), *Smooth the contour lines* with its passes, *Label units* (feet and inches, inches, decimal feet) and *Highlight negative elevations* (labels below 0 in red), with separate switches to label the primary and the secondary contours. Primary and secondary contours and their labels draw on the layers **Terrain, Primary Contours** and **Terrain, Secondary Contours**, so Layer Display Options can hide either family.

The perimeter has **Polyline** (perimeter, area, number of lines), **Label**, **Object Information** and **Schedule** pages. **Clear Terrain** is meant to remove only what Build Terrain generated (the surface and the contours) and keep the perimeter, the elevation data and the objects; `TerrainRecord::clear_generated` does that; wiring the menu command to it is on the integration queue, so until then the command still resets the record.

### Round 15: object panels and labels

Every terrain object now has the panels Chief gives it. **Polyline** shows the perimeter or length, area, volume and number of lines (read only). **Label** switches a label on (the *Terrain Labels* layer), with custom text or the automatic one and an offset; the **Terrain Labels** tool (Elevation Data flyout) switches a label on or off with a click. **Object Information** holds manufacturer, supplier, code, comment and URL. **Schedule** moves the object to another category of the terrain schedule: *Terrain Perimeter*, *Driveways*, *Medians*, *Roads*, *Road Markings*, *Terrain Paths* (sidewalks, terrain walls and curbs) and *Terrain Features* (features, modifiers, garden beds, grass, water features, stepping stones); the General schedule lists them under those categories.

Elevation data: an **Elevation Point** has **Display** (a note beside the point with an *Insert Macro* button for `%elevation%`, and the marker radius) and Line Style; an **Elevation Region** has *Interior is flat* (unticked, only the outline is held) and *Interpolate tangent to the edge*; a **Terrain Break** has a *Transition distance* (how far its elevation reaches) and can *follow the ground*. One click and Enter with the Elevation Region tool make an 8 ft square, and with a modifier tool a 10 ft square. The Terrain Specification has a **Fill Style** page for the perimeter (no fill, solid, hatch, a color of its own). Modifiers (Hill, Valley, Raised, Lowered, Flat) have Polyline, Line Style, Fill Style, Label, Object Information and Schedule pages. A **Terrain Feature** has *Terrain to top* (negative sinks it), *Thickness* (a shell under the top, for a planter or a pool) and *Clip overlapping terrain features* (the part of a feature that a lower feature covers is left out). A **Grass Region** has **Blades** (density, minimum and maximum height, width and curve) and **Appearance** (blade colors, noise frequency, roughness and mowing); in 3D the region takes the average blade color. A **Garden Bed** has a **Distributed Plant** page: pick a plant and a spacing and copies of it are spread over the bed (staggered rows, an edge margin), drawn in plan and in 3D.

### Round 15: roads

Roads have **Polyline Road, Polyline Driveway, Polyline Sidewalk** (click the corners of an outline; the vertex handles reshape it), **Median** (a polyline inside a road: the terrain material over the road with its own curb when the road has one), **Cul-de-sac** (click on the end of a road: a round road end; a click away from any road end places a free one) and **Auto Generate Sidewalk** (click a road, type the offset from it: a sidewalk on each side of the road and of every road joined to it). The old Polyline tools are now called Straight Road, Straight Driveway and Straight Sidewalk. A road or path specification adds **Terrain to top**, **Thickness** (a slab with side faces), **Flare** (check Start or End and give a radius: the strip widens into a quarter-circle fillet where it meets another road) and **Curb** (height, width, and *Cut the curb for driveways and sidewalks* so a gap is left where they cross).

### Round 15: survey and GPS import, plants

**File > Import > Terrain Data** is the Import Terrain Assistant in three pages. *Select File* picks a DXF, GPX or text file (or paste text) and the **data organization**: detected, XYZ, #XYZ, #XYZ Description, YXZ, #YXZ or #YXZ Description, with the delimiter and the header lines to skip. *Filter Data* shows the point count and the extent of each axis, lets you limit X, Y and Z to a range and **reduce to a number of evenly spread points** (it warns above 2,000 points). *Scale Data* sets the units of each axis, maps a file point to the plan origin, scales the relief and **rotates north counterclockwise**; options center the survey on the perimeter, lower the lowest point to 0, and **create a perimeter around the data when there is none**.

**File > Import > GPS Data** is the Import GPS Data Assistant: *Select File* (a GPX 1.1 file; way points carry elevation, track points do not, route points are ignored), *Import As* (way points as Elevation Data, Marker, Polyline or Terrain Perimeter; track points as Marker, Polyline or Perimeter) and *Transform Coordinates* (lower the elevation data by an amount, rotate north, map a latitude and longitude to the plan origin). Markers and polylines become CAD objects on the Site Plan layer; the whole import is one undo step.

A plant run can use **plant images**: tick *Draw the plants as images* on the Plant Image page (image file, 2D symbol, size with an aspect-ratio lock, elevation to top or bottom, center point, reverse, always face the camera, copyright, transparent color). In 3D each plant is a billboard tinted for the plan's *Season* (spring, summer, autumn, winter, each with a tint and a foliage share; a bare winter tree has a thin crown). **Terrain > Plant > Grow All Plants** opens a 0 to 20 year slider that scales every plant run that has a mature height and age (the Plant Chooser sets them from the catalog); OK stores the sizes as one undo step. A **Sprinkler Line** (Polyline Sprinkler Line, Spline Sprinkler Line) is 2D irrigation pipe: a dashed line in plan, nothing in 3D.

## 9.9 Differences from Chief

- Sprinkler heads are not connected to a supply, and the 3D water surface is flat (the ripple is a plan fill).
- Build Terrain runs on the UI thread: it reports its stages (with a percentage) in the readout while it builds and ends with a triangle count and time, with no separate progress bar.
- Round 15 estimates that Chief does not publish and marks "verify in Chief": the automatic floor-to-terrain distance (6", not the foundation-dependent value), the skirt and Hide Terrain Intersected by Building being off by default, the Low, Medium and High smoothing passes (1, 3, 6), the median as a patch laid over the road instead of a hole cut in it, the flare as a quarter circle, the 3D grass being a tinted surface (no blades) and the plant-image seasons.
- The tab layout of the Terrain Specification, the stepped-wall course height, the road-marking defaults (4" wide, 10' dashes with 20' gaps, yellow in plan, white in 3D) and the Import Terrain Data dialog are modeled on Chief's but not captured from it (verify in Chief); see `DECISIONS.md`.
- Plants are terrain-owned runs, not placed library symbols: the Plant Schedule (chapter 11.2) does not list them and Replace
  From Library does not see them.
- North Pointer and Scale Bar are CAD objects on the "Site Plan" layer (Terrain tool); the pointer sets the plan's
  north angle, which the sun angle reads through `site_view::plan_sun_azimuth`.
- No site plan layout box for a plot plan yet (chapter 11).
