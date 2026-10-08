# Chapter 2: Walls

Walls are the backbone of a Plan Studio plan. Rooms, 3D, roofs, schedules and
dimensions are all derived from them, so almost everything you do starts here.
This chapter covers the wall tools, snapping and automatic joins, editing walls
with Select Objects, and the Wall Specification dialog.

## 2.1 How a wall is stored

A wall is one straight segment, drawn along its **centerline**, with a
thickness and a height. Around that line sit:

```
        exterior face                      Plan view of a wall cross section
   +-----------------------+
   | stucco  | sheathing | framing (MAIN) | insulation | drywall |
   +-----------------------+
   ^                       ^
   outer surface           inner surface        centerline = drawn line
```

- **Kind**: Exterior or Interior.
- **Wall type**: a named stack of layers, for example `Stucco-6` (stucco,
  sheathing, 5 1/2" framing, drywall). Exactly one layer is the **Main layer**.
  The type decides the wall's thickness. See 2.8.
- **Resize About**: the reference line a thickness change keeps fixed. Exterior
  walls default to Main Layer Outside; interior walls to Wall Center.
- **Exterior side**: which side of start-to-end faces the outdoors, so layers
  are laid out in the right order.
- **Layer**: the plan layer the wall lives on, `Walls, Normal` by default.
- **Flags**: invisible, no room definition, no locate, room divider, railing,
  half-wall, pony wall, foundation, attic. The model stores these and room
  detection honors them (see 2.9 for what the dialog can set today).
- **Curve**: curved walls are stored as a true arc with a signed bulge
  (planned in the editor; see 2.2).
- Lengths are in inches. Openings (doors and windows) belong to exactly one
  host wall and travel with it.

## 2.2 Tools

All wall tools are on the Straight Wall, Curved Wall, Railing and Deck, Fencing
buttons in row 2 and under Build > Wall, Build > Railing and Deck, Build >
Fencing. Only the first two straight walls are built.

### Straight Wall Tools

| Button | Hotkey | Today |
|---|---|---|
| Straight Exterior Wall | `Shift+Q` (alias `2` while it is the flyout's face) | Works. Uses the Default Settings exterior wall type and height. |
| Straight Interior Wall | `Ctrl+Alt+Cmd+6` | Works. Uses the interior wall defaults. |
| Straight Foundation Wall | | (planned) |
| Straight Pony Wall | | (planned) |
| Straight Glass Wall | | (planned) |
| Straight Glass Pony Wall | | (planned) |
| Straight Half-Wall | | (planned) |
| Room Divider | | (planned) |
| Slab Footing | | (planned) |
| Wall Hatching | | (planned) |
| Wall Material Region | | (planned) |

The button's face shows the last variant you used, and alias key `2` starts
whichever wall variant is on the face. Switching between Exterior and Interior
in the middle of a chain keeps the chain and uses the new kind for the next wall.

### Curved Wall Tools

Curved Exterior Wall, Curved Interior Wall, Curved Foundation Wall, Curved Pony
Wall, Curved Half-Wall: all (planned). The model already has `WallCurve` and the
Wall Specification already shows a (disabled) Curved Wall section.

### Railing and Deck, Fencing, Terrain Wall and Curb

Straight Railing (`Cmd+Q`), Curved Railing, Straight and Curved Deck Railing,
Straight and Curved Deck Edge, Polygon Shaped Deck, Straight and Curved
Fencing, Straight and Curved Terrain Wall and Curb: all (planned).

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

Walls shorter than a small threshold are discarded. A wall started or ended on
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
and text, cabinets, library symbols, stairs, roof planes, electrical devices, camera objects, rooms and
the terrain (each kind's handles and dialog are in its own chapter). This section is about walls: click
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
| Wall Cap, Wall Covering, Rail Style, Newels/Balusters, Rails | (disabled) |
| Layer | Works |
| Materials | (disabled) |
| Label | Works (session only) |
| Components, Object Information, Schedule | (disabled) |

### General

- **General**: check boxes Foundation Wall, Railing, Terrain Retaining Wall, Attic
  Wall (all disabled); **Thickness**; **Wall Length**; **Wall Angle**; **Lock**
  Start / Center / End.
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
- **Pony Wall** (disabled): Lower Wall Type, Elevation of Lower Wall Top.

### Layer

**Layer** (with a Default check box) picks the plan layer: Walls, Normal; Walls,
Invisible; and the other wall layers. **Drawing Group** is disabled.

### Label

Display Options (Suppress Label in All Views, Display in Plan View) and Label
Content (Automatic Label shows `Wall - <type> - <length>`; Specify Label takes your
own text). All of these are (session only). Appearance (Display Border, Text Style,
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
Chief working template has 108, inventoried in `docs/daniel-template-inventory.md`. Edit > Default Settings...
> Walls > Exterior Wall, Interior Wall or Foundation Wall edits the wall type, height and a custom thickness
for each (a custom thickness adds a `Custom-<n>` wall type). The Properties panel's "Default walls" section
edits the same values.

## 2.9 What the dialog stores and what it does not

- **Stored with the plan**: thickness, height, kind, wall type, layer, the
  position of the wall, and the **Invisible**, **No Room Definition** and **No Locate**
  options (the wall's `flags`). Room detection, drawing and dimensions honor them, and
  the Auto Stairwell sets the same flags on the walls it creates.
- **Session only**: the Label tab.
- **Model ready, no UI**: pony walls, curved walls, railing, half-wall and
  foundation flags, per-wall roof directives (set with the Gable/Roof Line tool,
  chapter 8).

## 2.10 Known differences from Chief

- Rooms are traced from centerlines, then offset to the interior faces for areas and labels (chapter 4).
- Walls T-split at junctions; Chief keeps the through wall whole.
- No typed length or angle readout while drawing, and connections are inferred from coordinates each time.
