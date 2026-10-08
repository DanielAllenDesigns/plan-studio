# Chapter 2: Walls

Walls are the backbone of a Plan Studio plan. Rooms, 3D, roofs, schedules and
dimensions are all derived from them, so almost everything you do starts here.
This chapter covers the wall tools (straight and curved, and the foundation, pony,
glass, half, divider, railing, deck and fencing variants), snapping and automatic joins,
editing walls with Select Objects, and the Wall Specification dialog.

## 2.1 How a wall is stored

A wall is one straight segment (or one arc, see Curve below), drawn along its
**centerline**, with a thickness and a height. Around that line sit:

```
        exterior face                      Plan view of a wall cross section
   +-----------------------+
   | stucco  | sheathing | framing (MAIN) | insulation | drywall |
   +-----------------------+
   ^                       ^
   outer surface           inner surface        centerline = drawn line
```

- **Kind**: Exterior or Interior.
- **Bottom height**: where the wall starts above the floor it is drawn on, in inches (0 by default, like Chief's wall "Bottom" value). A wall spans from its bottom height to its bottom
  height plus its height. The 3D view honors it for standard walls and every straight wall class; the plan view, room detection, elevations, schedules and wall framing do not know it yet (2.9).
- **Wall type**: a named stack of layers, for example `Stucco-6` (stucco,
  sheathing, 5 1/2" framing, drywall). Exactly one layer is the **Main layer**.
  The type decides the wall's thickness. See 2.8.
- **Resize About**: the reference line a thickness change keeps fixed. Exterior
  walls default to Main Layer Outside; interior walls to Wall Center.
- **Exterior side**: which side of start-to-end faces the outdoors, so layers
  are laid out in the right order.
- **Layer**: the plan layer the wall lives on, `Walls, Normal` by default.
- **Class**: which flyout wall it is. Standard (an ordinary exterior or interior
  wall), Foundation, Pony Wall, Glass Wall, Glass Pony Wall, Half-Wall, Room Divider,
  Railing, Deck Railing, Deck Edge or Fencing (2.2, 2.6). The class is stored with the
  wall. A Foundation wall also stores how far it reaches below the floor it is drawn on
  (`foundation_height`, 48" by default).
- **Flags**: invisible, no room definition, no locate, room divider, railing,
  half-wall, pony wall, foundation, attic. The model stores these and room
  detection honors them (see 2.9 for what the dialog can set today).
- **Curve**: a curved wall is stored as a true arc through its start, an apex and
  its end, with a signed **bulge** (the distance from the chord midpoint to the apex;
  positive bulges toward the left of start-to-end). Curved variants of the flyout walls
  draw this way (2.2). Opening offsets on a curved wall are measured along the arc.
- Lengths are in inches. Openings (doors and windows) belong to exactly one
  host wall and travel with it.

## 2.2 Tools

All wall tools are on the Straight Wall, Curved Wall, Railing and Deck, Fencing
buttons in row 2 and under Build > Wall, Build > Railing and Deck, Build >
Fencing. Every one of them draws the same way (see Drawing walls below) and differs
only in the wall it creates.

### Straight Wall Tools

| Button | Hotkey | Today |
|---|---|---|
| Straight Exterior Wall | `Shift+Q` (alias `2` while it is the flyout's face) | Works. Uses the Default Settings exterior wall type and height. |
| Straight Interior Wall | `Ctrl+Alt+Cmd+6` | Works. Uses the interior wall defaults. |
| Straight Foundation Wall | | Works. |
| Straight Pony Wall | | Works. |
| Straight Glass Wall | | Works. |
| Straight Glass Pony Wall | | Works. |
| Straight Half-Wall | | Works. |
| Room Divider | | Works. |
| Slab Footing | | Works: draw a polygon and it makes a foundation slab with a footing (chapters 16 and 17). The slab tools are also in their own flyout. |
| Wall Hatching | | Works: click a wall and pick the pattern (chapter 17). |
| Wall Material Region | | Works: click a wall, or press and drag along it for part of its length (chapter 17). |

The button's face shows the last variant you used, and alias key `2` starts
whichever wall variant is on the face. Switching between Exterior and Interior
in the middle of a chain keeps the chain and uses the new kind for the next wall.

What each variant creates (class, kind, wall type, height and layer):

| Variant | Kind | Wall type and height | Notes |
|---|---|---|---|
| Straight Foundation Wall | Exterior | The Foundation Wall default (`Foundation-8`, 48" in Daniel's template) | Class Foundation. Reaches its Foundation Height below the floor. |
| Straight Pony Wall | Exterior | Upper `Stucco-6`, lower `Foundation-8`, split at 36"; full exterior wall height | Two wall types stacked (2.6). |
| Straight Glass Wall | Exterior | `Glass-1`; exterior wall height | A glass panel in a frame. |
| Straight Glass Pony Wall | Exterior | A `Foundation-8` lower part up to 36", glass above | |
| Straight Half-Wall | Interior | The interior wall type, topped at 36" | |
| Room Divider | Interior | No wall type; 1/8" thick | Draws on the `Walls, Invisible` layer; closes rooms but has no body (2.9). |

The pony, glass, half-wall, railing, deck and fencing defaults (types, split and top
heights) are typical values, not captured from Chief. They are stored in the defaults, so a
template you save carries them, but no Default Settings page edits them yet; only the
Foundation Wall page (Default Settings > Walls) does.

### Curved Wall Tools

| Button | Today |
|---|---|
| Curved Exterior Wall | Works. |
| Curved Interior Wall | Works. |
| Curved Foundation Wall | Works. |
| Curved Pony Wall | Works. |
| Curved Half-Wall | Works. |

A curved variant takes **three clicks**: click the start, click the end of the chord, then
click to set the arc. The arc's height (the bulge) is the distance of that third click from
the chord, rounded to the snap unit; a ghost previews the arc while you move, and the
status bar says "Click to set the curve". A third click within 1/2" of the chord makes a
straight wall instead. A press-drag-release sets the chord of one wall (the chain then ends
with it) and the next click sets the arc. A chain of clicks continues from the arc's end. The
end of a curved wall snaps onto the nearest end of another wall within the connect distance.

Limits of curved walls today:

- **No mitered joins.** A curved wall's end only moves to meet a neighbor's end; there is no
  corner solving, the neighbors do not move, and the bulge stays.
- **No opening cuts in 3D.** The 3D view builds a curved wall as a run of straight facets
  (one every 7.5 degrees) and does not cut doors and windows through it.
- The Curved Wall section of the Wall Specification stays disabled, so the arc cannot be
  edited by number; there are no Select Objects handles for the bulge.
- There is no Curved Glass Wall or Curved Glass Pony Wall tool.

### Railing and Deck, Fencing

| Button | Today |
|---|---|
| Straight Railing (`Cmd+Q`) | Works from the toolbar and menu. The key is shown but not bound: Command-Q is the macOS Quit shortcut. |
| Curved Railing | Works. |
| Straight Deck Railing, Curved Deck Railing | Work. |
| Straight Deck Edge, Curved Deck Edge | Work. |
| Polygon Shaped Deck | (planned) |
| Straight Fencing, Curved Fencing | Work. |
| Straight and Curved Terrain Wall and Curb | (planned) |

| Variant | Kind | Wall type and height | Layer |
|---|---|---|---|
| Railing | Interior | `Railing-4`, 36" | `Walls, Normal` |
| Deck Railing | Exterior | `Deck Railing-4`, 36" | `Deck Railing` |
| Deck Edge | Exterior | `Deck Edge-2`, 9 1/4" (a rim board, no railing) | `Deck Railing` |
| Fencing | Exterior | `Fence-Wood-2`, 72" (Picket style) | `Fencing` |

The first use of a variant adds its layer to the plan if it is missing (and registers its wall
types, so the Wall Types tab lists them).

### Drawing walls

With a wall tool active:

| Gesture | Result |
|---|---|
| Click, click, click ... | Each click after the first ends one wall and starts the next at the same point: a continuous chain. |
| Press, drag, release | Draws exactly one wall from press to release and ends the chain. |
| Click the start of the first wall (after at least two walls) | Closes the loop and ends the chain. |
| `Esc` | Cancels the wall in progress, keeps the walls already placed, and leaves the chain. A second `Esc` returns to Select Objects. |
| Right-click | Ends the chain without leaving the tool. |
| Hold `Alt` | Suspends the angle snap only (free angle). Object and grid snaps stay on. |
| Move the pointer | The status bar shows `Length: 12'-6"` and the snap in use (with Temporary Dimensions on). A ghost wall previews the real thickness. |

Walls shorter than a small threshold (1") are discarded. A wall started or ended on
another wall splits it there (a T-junction). Every wall you draw is one undo step
named "Draw Wall". The new wall becomes the selection.

Typing a length while drawing (Chief's Tab and Enter entry) is (planned). To place
a wall at an exact length today, draw it and then type a value into its temporary
dimension (2.5) or open its specification (2.6).

## 2.3 Snapping while drawing

In priority order, the pointer snaps to:

1. The start of the chain's first wall (closes the loop).
2. Another wall's **endpoint**, then an **intersection**, then a **midpoint**,
   then a **perpendicular** foot, then a point **on** another wall's centerline.
   The status bar names the snap, for example `Snap: Midpoint`.
3. The axes through the chain's first point (square-up alignment).
4. **Collinear** with, or **perpendicular** to, the previous wall.
5. The **15 degree angle** snap from the previous point (with the length rounded
   to the snap unit).
6. The **grid** snap, 1" in Daniel's template.

`Alt` suspends the angle snap only; object snaps and the grid stay active.
Snap spacing and grid spacing are set in the Properties panel (Grid spacing,
Snap spacing). Angle snaps follow the defaults' 15 degrees.

Not built: dashed alignment guides that extend from other walls' endpoints,
Shift to constrain to 0/90 degrees, a separate on/off for each object snap
(Edit > Snap Settings is dimmed), and Alt suspending every snap.

## 2.4 Automatic wall connections

Chief joins walls as you draw them and as you edit them; Plan Studio does too.
After a wall is created, or after one of its ends is moved or released:

```
 Corner (L)            T-junction             Crossing (X)
 +------+              ------+------          |   |
 |      |                     |                ---+---
 |      |                     |                   |
 mitered, no overlap   butting wall trimmed;   both walls cut at the
 or gap                through wall split      crossing so rooms form
```

- An end near another wall's end becomes a **corner**: both outlines meet at the
  miter, inner to inner and outer to outer, layer by layer. Very sharp angles
  fall back to butt joins (miter limit 4 x thickness).
- An end near another wall's centerline becomes a **T**. The butting wall's layers
  stop at the through wall's faces. The through wall is split at the T so room
  detection can use the pieces. (Chief keeps one object; Plan Studio splits by
  default and the split option is not yet a saved default.)
- **Crossing** walls are cut at the crossing.
- Overlapping duplicate collinear walls are merged.

Every connection function is idempotent: running it twice changes nothing.
The whole edit is one undo step. The Edit toolbar shows a **Fix Wall
Connections** button when a wall is selected. It is (planned): the function that
re-solves every wall of the floor exists and is tested (`editor/connect.rs`), but the
button is not wired to it yet and reports "not implemented yet".

## 2.5 Editing walls with Select Objects

Press `Space`. Select Objects picks and edits every kind of object: walls, openings, dimensions, CAD
and text, cabinets, library symbols, stairs, roof planes, electrical devices, camera objects, rooms,
the terrain, slab objects, placed framing and the exterior details (each kind's handles and dialog are in its own chapter; a schedule placed in the plan is picked with the Schedule tool, chapter 11.2). This section is about walls: click
a wall to select it (anywhere inside its footprint). The selection shows **handles** and **temporary
dimensions**.

### Handles

```
        end handle                     middle (move) handle        end handle
   o------------------[ ]------------------o
   |<---- 14'-0" --->|                       temporary dimensions:
                                              own length, distance to the
                                              nearest parallel walls on
                                              each side, distance from each
                                              end to a perpendicular wall
```

| Drag | Effect |
|---|---|
| The wall body or middle handle | Moves the wall **perpendicular to its length only**. Connected walls keep their directions and stretch so corners stay joined. Openings travel with the wall. This is Chief's signature behavior. |
| The same, holding `Alt` | Free move in any direction; connected ends follow. |
| An end handle | Moves that end, with snapping. The other end stays put. Walls joined at the dragged end follow it. Dropping the end on another wall's centerline makes a T and splits that wall. |
| A door or window | Slides it along its wall, or onto another wall (chapter 3). |
| Empty space | Marquee: left to right selects what is enclosed, right to left what is touched. |

Every drag is one undo step. `Esc` during a drag cancels it and restores the
geometry. A drag begins only after the pointer moves a few pixels.

### Typed dimensions

With Temporary Dimensions on, click the value on a temporary dimension, type a
length (`5'-0"`, `60`) and press `Enter`. The selected object moves so the
dimension takes that value; the object at the other end stays fixed. A wall's own
length dimension follows the Lock setting of its specification (Start by default).
`Tab` moves between a selected object's dimensions, `Esc` cancels.

### Selection and clipboard keys

| Key | Does |
|---|---|
| Click, `Shift`+click | Select, add or remove. |
| `Tab` | Cycle through objects under the pointer. |
| `Delete`, `Backspace` | Delete the selection (a deleted wall takes its openings). One undo step. |
| Double-click, `Enter` | Open the specification dialog. |
| Edit toolbar: Open Object, Delete Objects, Copy Selected Objects, Paste in Place | The common buttons for any selection. Paste in Place duplicates the selection in the same position (the copy is selected, with new ids). Move it afterwards; pasted walls are not auto-connected. Some object kinds add their own buttons (Reverse Swing, Auto Stairwell, Join Roof Planes, Flip Side ...). |
| `Cmd+C`, `Cmd+X`, `Cmd+V`, `Cmd+A` | (planned) The Edit menu items are dimmed. Paste Hold Position is (planned). |

Objects on a locked layer can be selected but not moved or deleted; the status
bar says so. Objects on a hidden layer cannot be selected.

Not built: group/ungroup in the Edit toolbar (the model supports groups), Transform/
Replicate, Reflect, Align/Distribute, Break Wall, Reverse Layers, Make Parallel/
Perpendicular, Change Line/Arc (all planned).

## 2.6 Dialog: Wall Specification

Open it by double-clicking a wall with Select Objects, with Enter on the
selection, from "Open Specification..." in the Properties panel, or from the Edit
toolbar's Open Object. Edit > Default Settings > Walls opens the same dialog for
the defaults new walls start from.

The dialog uses Chief's frame: a 150 px vertical tab list, the tab's panel, a
220 px preview (plan view above, layer section below) and Help / Cancel / OK.
It edits a copy: OK (or Enter) applies all tabs as one undo step, Cancel or Escape
discards. Length fields show feet-inches and turn red, blocking OK, when the text
does not parse.

| Tab | Status |
|---|---|
| General | Works |
| Structure | Works (first part) |
| Roof | (disabled) |
| Foundation | (disabled) |
| Wall Types | Works |
| Rail Style | Works |
| Wall Cap, Wall Covering, Newels/Balusters, Rails | (disabled) |
| Layer | Works |
| Materials | (disabled) |
| Label | Works (the plan-label switch and a specified label text are stored; the rest is session only) |
| Components, Object Information, Schedule | (disabled) |

### General

- **General**: check boxes **Foundation Wall** and **Railing** (they switch the wall's
  class; disabled in the Default Settings dialogs), Terrain Retaining Wall and Attic Wall
  (disabled); **Wall Class**; **Thickness**; **Bottom Height**; **Wall Length**; **Wall Angle**; **Lock**
  Start / Center / End.
  - **Bottom Height** is the distance from the floor to the bottom of the wall (0 by default). A negative value is refused. It is disabled in the Default Settings dialogs;
    "Default Wall Bottom Height" in the Structure group stays disabled. A wall with a bottom height shows its full height in 3D starting at that level;
    the openings keep their sill heights measured from the floor, so only the part of an opening between the wall's bottom and top is cut. Explode Dormer (chapter 8) uses it to stand the dormer's walls on the roof.
  - **Wall Class** (not in the Default Settings dialogs) is a list of Standard, Foundation,
    Pony Wall, Glass Wall, Glass Pony Wall, Half-Wall, Room Divider, Railing, Deck Railing,
    Deck Edge and Fencing, in flyout order. Picking a class moves the wall to that class's
    layer, swaps in a wall type that goes with it (a glass type for a glass wall, `Foundation-8`
    for a foundation wall, an ordinary type when leaving a special one) and takes the new
    thickness from that type. Three classes add a field right below the list: **Foundation
    Height** (a Foundation wall: how far it reaches below the floor), **Half-Wall Height** (the
    wall's top; it also sets the wall height) and **Fence Style** (Picket, Privacy or Rail).
  - Changing Wall Length moves the end point (Lock Start), the start point (Lock
    End) or both equally (Lock Center). Openings keep their distance from the
    locked point. OK is blocked with "Wall is too short for its openings" if an
    opening would no longer fit.
  - Wall Angle rotates the wall about its start point. Joined walls are not moved.
  - Length, angle and lock are disabled in the Default Settings dialog.
- **Options**: Invisible, No Room Definition, No Locate (stored on the wall, see 2.9; disabled in the Default Settings dialog);
  Lock Center, No Room Moldings Exterior, No Room Moldings Interior, Automatically
  Generated Wall, Ignored by Hide Exterior Walls (disabled).
- **Curved Wall** (disabled): Radius to Outer Surface / Main Layer Outside; Lock
  Arc Center / Ends; Automatic Facet Angle.

### Structure

- **Default Wall Heights**: Default Wall Top Height (checked: the wall follows the
  floor's ceiling height and platform), Wall Height, Default Wall Bottom Height (disabled).
- **Platform Intersections**, **Wall Intersections**, **Rim Joist**, **Double Wall**,
  **Stud Layout**, **Framing** (all disabled): Chief's framing-related options such as
  Through Wall At Start/End, Bearing Wall, Stud Rollout.

### Wall Types

- **Wall Type**: a list of the plan's wall types with their thicknesses (an exterior
  default dialog lists exterior types; an interior one, interior types). Picking
  one sets the thickness, kind and layer stack at once and keeps the Resize About
  reference fixed. "Custom (7")" appears for a wall whose thickness matches no type.
- **Define...** opens Wall Type Definitions (2.7). **Library...** is disabled.
- A strip shows the layer stack, and a note says how far in from the exterior face
  the main layer starts.
- The Wall Type list offers the types that go with the wall's class: ordinary types for
  standard, pony and half walls, concrete types for a foundation wall, and the glass, railing,
  deck railing, deck edge and fencing types (named `Glass...`, `Railing...`, `Deck Railing...`,
  `Deck Edge...`, `Fence...`) for those classes.
- **Pony Wall**: the **Pony Wall** check box turns the wall into a pony wall (or back to a standard
  one). For a Pony Wall, **Upper Wall Type** and **Lower Wall Type** are lists, **Elevation of
  Lower Wall Top** is the split height (clamped to the wall height; 36" by default) and **Display in
  Plan View** picks Upper or Lower, the type whose layers the plan draws. For a Glass Pony Wall the
  upper type is fixed to Glass. On any other class these fields are disabled.

### Rail Style

For a Railing or Deck Railing wall: **Railing Height** and a note of what 3D builds (posts at most 8'
apart and one at each end, a top rail, a bottom rail and 3/4" balusters about 4" apart). On other
walls the tab says to draw a Railing or Deck Railing wall, or tick Railing on the General tab.

### Layer

**Layer** (with a Default check box) picks the plan layer: Walls, Normal; Walls,
Invisible; and the other wall layers. **Drawing Group** is disabled.

### Label

Display Options (Suppress Label in All Views, Display in Plan View) and Label
Content (Automatic Label shows `Wall - <type> - <length>`; Specify Label takes your
own text). **Display in Plan View**, the Specify Label choice and its text are saved with the
wall; Suppress Label in All Views is (session only). Appearance (Display Border, Text Style,
Alignment, Auto Adjust Text Direction) and Label Layer are disabled.

## 2.7 Dialog: Wall Type Definitions

From Wall Types > Define.... A table of the selected wall type's layers from the
exterior face to the interior face.

| Column or control | Meaning |
|---|---|
| Name | The layer's name (Stucco, Sheathing, Framing, Drywall ...). |
| Thickness | Edited as feet-inches. |
| Material | The layer's material name. |
| Main layer radio | Exactly one layer is the Main layer. |
| Insert, Delete, Move Up, Move Down | Edit the stack. |
| Resize About | The reference a thickness change holds fixed: Main Layer Outside, Main Layer Inside, Wall Center, Outer Surface, Inner Surface. |
| Preview | A live layer-stack drawing. |

Changing a type re-flows the walls that use it. A new wall copies the default
type of its kind at draw time, so editing the default later does not change
walls already drawn.

## 2.8 Wall types and Default Settings

Daniel's template ships the common wall types (Stucco-6, Siding-6, Brick-6, Interior-4, Foundation-8 ...); his
Chief working template lists 108 wall type names (inventoried in `docs/daniel-template-inventory.md`), 103 of which have a decoded layer stack that Import Chief Template uses (chapter 12.8). Edit > Default Settings...
> Walls > Exterior Wall, Interior Wall or Foundation Wall edits the wall type, height and a custom thickness
for each (a custom thickness adds a `Custom-<n>` wall type). The Properties panel's "Default walls" section
edits the same values.

## 2.9 What the dialog stores and what it does not

- **Stored with the plan**: thickness, height, kind, wall type, layer, the
  position of the wall, and the **Invisible**, **No Room Definition** and **No Locate**
  options (the wall's `flags`). Room detection, drawing and dimensions honor them, and
  the Auto Stairwell sets the same flags on the walls it creates.
- **Stored in the wall's extras**: the Display in Plan View switch, a specified label text, and the
  wall type last picked in the Wall Types tab. They are saved with the plan and migrate on their own
  from older files (which simply load with the defaults).
- **Stored with the wall**: the Bottom Height (`bottom_offset`, 0 unless set). Limits: Railing-class (`flags.railing`) walls ignore it in 3D, and the 2D plan, room detection, elevations,
  schedules and wall framing do not read it yet.
- **Stored with the class**: the Wall Class and the values that go with it (pony upper and lower
  types, split height and plan display; glass pony lower type and split; half-wall height; fence
  style), the Foundation Height, and the curve of a curved wall.
- **Session only**: Suppress Label in All Views and the other dimmed-or-kept-in-memory controls.
- **Model ready, no UI**: the attic flag and per-wall roof directives other than Hip, Full Gable
  and High Shed/Gable (the Gable/Roof Line tool sets the first two, chapter 8).

## 2.10 Known differences from Chief

- Rooms are traced from centerlines, then offset to the interior faces for areas and labels (chapter 4).
- Walls T-split at junctions; Chief keeps the through wall whole.
- No typed length or angle readout while drawing, and connections are inferred from coordinates each time.
- Curved walls have no mitered joins, no 3D opening cuts, no bulge handle and no editable Curved Wall
  section; a curved wall is a run of 7.5 degree facets in 3D.
- Pony, glass, half-wall, railing, deck and fencing default types and heights are typical values, not
  Chief's, and are not editable from Default Settings. Terrain walls and curbs are (planned); Wall Hatching, Wall Material
  Region and the Polygon Shaped Deck are in chapter 17.
- The Roof, Foundation, Wall Cap, Wall Covering, Newels/Balusters and Rails tabs of the Wall
  Specification are still (disabled).
