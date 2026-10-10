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
  height plus its height. The 3D view honors it for standard walls and every straight wall class; room detection, elevations, schedules and wall framing do not know it yet (2.9). In plan a wall raised **48" or more** off the floor is drawn **dashed and unfilled** (a raised wall, such as a clerestory or a header wall), and it no longer closes a room.
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

While you set the arc, the status bar reads the **radius, the arc length and the chord**
("Radius: 12'-0"   Arc: 18'-10 1/2"   Chord: 20'-0""); a bulge near zero reads "Straight".

What a curved wall does (Round 12):

- **Joins.** Both outlines of a curved wall, and every layer of its wall type, are exact arcs
  (the plan band is a run of facets inscribed in the offset arcs, so each vertex is half the
  thickness off the centerline). Where a curved wall meets another wall at an end the two are
  **mitered**, straight or curved neighbor, and where its end meets the side of another wall
  (or an arc meets the side of a curved wall) it gets the **tee cut**, as straight walls do
  (`plan_core::joins::{curved_wall_polygon, curved_end_miters, curved_layer_outlines}`). The
  miter treats the arc as its tangent line at the joined end, so a wide arc meeting a steep
  corner shows a small kink on the end facet; past the miter limit (4 x thickness) the end
  stays square.
- **Doors and windows cut through it in 3D.** The jambs and the head follow the arc and the
  door or window unit (leaf, sash, casing, mullions) stands square to the arc's tangent at the
  opening's center, so a window in a curved wall is flat and plumb in a curve (`plan-3d`
  `opening::tangent_wall`). The elevations show them, since they are drawn from the same scene.
- **Every class curves.** Curved foundation, pony, half and glass walls, glass pony walls,
  railings, deck railings and fencing are built along the arc in 3D; the roof cuts a curved
  wall's top facet by facet, and a **curved gable end** rises to the roof like a straight one.
  Foundation, pony, half-wall and the other special classes keep their flat or roof-cut tops
  as in chapter 8.
- **The arc can be edited by number.** The Arc section of the Wall Specification gives radius,
  arc angle and rise (each recomputes the others), the arc length and the center point,
  and (Round 13) **Radius to** and **Lock** (2.6). Change Line/Arc and its bulge handle, Make Arc Tangent and the three-click tool
  (2.5) draw and edit the same arc.
- **Opening offsets** are measured along the arc, and Auto Exterior Dimensions include curved
  walls (chapter 5).

Limits of curved walls today:

- A **straight wall drawn to meet an arc** ends square until the editor's connection pass is
  extended to arcs (the plan outline and the 3D cut already miter the pair once the ends
  touch; the hook that makes the connection when the straight wall is created is wired at
  the gate, not by this chapter's author).
- An opening's **plan symbol** (jambs, leaf, swing arc) is drawn on the **chord** of the curved
  wall, not along the arc. The 3D opening follows the arc.
- Automatic Facet Angle stays dimmed. The facet angle is fixed at 7.5 degrees.
- There is no Curved Glass Wall or Curved Glass Pony Wall button on the Curved Wall flyout;
  draw a Curved Exterior Wall and set its Wall Class to Glass or Glass Pony in the Wall
  Specification (or use Change Line/Arc on a straight glass wall).

### Railing and Deck, Fencing

| Button | Today |
|---|---|
| Straight Railing (`Cmd+Q`) | Works from the toolbar and menu. The key is shown but not bound: Command-Q is the macOS Quit shortcut. |
| Curved Railing | Works. |
| Straight Deck Railing, Curved Deck Railing | Work. |
| Straight Deck Edge, Curved Deck Edge | Work. |
| Polygon Shaped Deck | Works: a polygon of decking (chapter 17). |
| Straight Fencing, Curved Fencing | Work. |
| Straight and Curved Terrain Wall and Curb | Work, from the Terrain menu: walls and curbs that follow the ground (chapter 9.6). |

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
| Hold `Alt` | Suspends every snap (object, angle and grid): the wall goes exactly where the pointer is. |
| Hold `Shift` | Holds the Angle Snap increment (15 degrees by default, set in Edit > Snap Settings) even where angle snaps are off. |
| Type a length, `Tab`, an angle, `Enter` | After the first click, digits fill the length (`12'6`, `12-6`, `150`), `Tab` switches to the angle (degrees counter-clockwise from east), `Enter` draws the wall at exactly that length and angle and continues the chain. With only a length typed the pointer still picks the direction. `Backspace` edits, `Esc` drops the typed text first. |
| Move the pointer | The status bar shows `Length: 12'-6"   Angle: 90.0°` and the snap in use (with Temporary Dimensions on). A ghost wall previews the real thickness. |

Walls shorter than a small threshold (1") are discarded. A wall started or ended on
another wall splits it there (a T-junction). Every wall you draw is one undo step
named "Draw Wall". The new wall becomes the selection.

The same typed length and angle work when you drag a selected wall's end handle
(`Enter` drops the end there), and a selected wall shows its angle next to its start:
click it to turn the wall about its start point.

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

`Alt` suspends every snap. Edit > Snap Settings sets which snaps are on: the object
snaps as a group and one by one (Endpoint, Midpoint, Intersection, Perpendicular,
Tangent, Center, Quadrant, On Object, Extension, Points/Markers), grid snaps and the grid snap unit, angle snaps
with the increment and an optional list of allowed angles, the bumping distance and
the snap distance in pixels. Center, Quadrant and Tangent snap to CAD circles and
arcs, and CAD lines and polylines give Endpoint, Midpoint and On Object points. The
priority is Points/Markers, Endpoint, Intersection, Midpoint, Center, Quadrant, Perpendicular,
Tangent, Extension, On Object, Angle, Grid.

The snaps work against CAD objects as well as walls (Round 11):

- **Intersection** snaps where a wall and a CAD line, two CAD lines or polylines, or a line and a circle or arc cross, as well as where two walls cross.
- **Extension** (off by default; tick it in Snap Settings) snaps to the line beyond the end of a wall or a CAD line, once the pointer is farther past the end than the snap distance (nearer than that, Endpoint and On Object win), so a new wall can start in line with an existing one.
- **Points/Markers** (on) snaps to the center of a CAD point, a Point Marker and a numbered Marker (the circle with a number from the Text flyout).
- **Reference Display.** While the reference floor is shown, the pointer also snaps to the ends and the crossings of its walls (chapter 4.5).

Not built: dashed alignment guides that extend from other walls' endpoints.

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
- A **curved wall** takes part in all three: its ends are mitered against a straight or curved neighbor, and a wall that ends on an arc's side is cut to the arc's face there (the arc is met along its tangent at that point; Round 12, 2.2).
- Overlapping duplicate collinear walls are merged.

Every connection function is idempotent: running it twice changes nothing.
The whole edit is one undo step. The Edit toolbar's **Fix Wall Connections** button runs this repair on the selected walls, or on every wall of the floor when none is selected (`fix_wall_connections_action` in `editor/connect.rs`).

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
| Empty space | Marquee: left to right selects what is enclosed, right to left what is touched (Edit > **Marquee Selection** chooses By Drag Direction, Enclosing or Touching). |

Every drag is one undo step. `Esc` during a drag cancels it and restores the
geometry. A drag begins only after the pointer moves a few pixels.

**Select feel (Round 14).**

- Hold `Alt` when you press to start a marquee even on top of an object. Hold `Ctrl` (`Cmd` on a Mac) when you
  start a move drag to **copy** the selection instead of moving it.
- While you drag a move or a rotate, **type a number**: digits give the distance (`Tab` then the angle) of the
  move or the degrees of the turn, `Enter` finishes, `Esc` cancels.
- A drag that reaches the edge of the canvas scrolls the view that way.
- The status bar names what is under the pointer and, once selected, describes the selection (kind, size, Z);
  it also shows the **Edit Behavior** in force (Edit > Edit Behaviors).
- **Edit > Edit Area** asks for a rubber band; the rectangle left behind is an *edit area*: drag inside it to move
  everything wholly inside (with `Ctrl` or `Cmd` held at the start, to copy), drag its Rotate handle to turn it, `Delete`
  removes the contents, `Esc` or a click outside ends the mode. A wall that crosses the edge stretches: the end
  inside moves, the end outside stays, and its doors and windows keep their place. **Edit Area Visible** takes only
  objects on displayed layers. **Edit > Stretch CAD** takes the same band and then one drag: every CAD vertex inside
  moves with the pointer. Each drag is one undo step.
- Copy also writes the objects to `~/.plan-studio/clipboard.json`, so a Copy in one plan (or one window) pastes into
  another; the layers the objects sit on travel by name and are made if the destination lacks them.
- The **Fill Window Selected Objects** button frames the selection (Window > Fill Window Selected Objects).

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
| Edit toolbar: Open Object, Delete Objects, Cut, Copy Selected Objects, Paste in Place | The common buttons for any selection. Paste in Place puts a copy in the same position (the copy is selected, with new ids). Then come Group / Ungroup, Select Same Type, Transform/Replicate, Reflect About Object, Point to Point Move, Center Object, Make Parallel / Perpendicular (walls and CAD lines), Align/Distribute (two or more objects), Layer and Lock / Unlock. Some object kinds add their own buttons (Reverse Swing, Auto Stairwell, Join Roof Planes, Flip Side ...). The bar wraps onto a second row when it is long. |
| `Cmd+C`, `Cmd+X` | Copy and Cut. The clipboard keeps walls with their doors and windows, dimensions, CAD and text (with styles and blocks), cameras, cabinets, symbols, stairs, devices, framing, slabs, pads, piers, platform holes, moldings, decks, floor regions, 3D solids, schedules and terrain elements. It does not take roof planes, the terrain perimeter, rooms, or the trim and hatching that belong to a wall; Copy says what it left out and Cut refuses so nothing is lost. |
| `Cmd+V` | Paste: the copy hangs on the pointer, centered on it. Click to drop it (walls join the walls they touch); `Esc` or a right click cancels. Edit > Paste > Paste Special > As Group drops it as one group. |
| `C, P, P`, `Alt+Cmd+V` | Copy and Paste in Place, and Paste Hold Position (paste at the original coordinates, on the current floor). |
| `Cmd+D` | Duplicate: a copy 12" right and 12" down, selected. Leaves the clipboard alone. |
| `Cmd+A` | Select All: every object of the floor on a displayed, unlocked layer. |
| `Shift+Space` | Delete Objects: tick object types (walls, doors, windows, cabinets, text ...), choose this floor or all floors, Delete. One undo step. |
| `Cmd+G` | Group. A click on a member then selects the whole group; `Tab` still picks one member. Deleting members dissolves a group left with fewer than two. |

Objects on a locked layer can be selected but not moved or deleted; the status
bar says so. Objects on a hidden layer cannot be selected.

### Transform, reflect, align and the right-click menu

- **Edit > Transform/Replicate Object...** moves, rotates, resizes and mirrors the
  selection, with optional copies. Copy k is the step applied k times, so a move makes
  a linear array and a rotation about a point a radial one. Move is X and Y or a
  distance and an angle; Rotate turns about the center of the selection or a point you
  type; Resize is a percentage about the center; Reflect mirrors about a vertical or
  horizontal line (through the center by default). Apply is one undo step; the copies
  are selected.
- Walls take their doors and windows along. A mirror flips the doors' swing so they
  still look mirrored and flips each wall's exterior side so the exterior stays outside.
  Walls, openings, dimensions, CAD, text, symbols, cameras, cabinets and devices turn
  and mirror; stairs turn and mirror; roof planes, framing, foundation objects, details,
  schedules and terrain elements can be moved and copied but not turned (the status bar says so).
- **Reflect About Object**: click a wall or CAD line and the selection mirrors about it
  (Reflect Copy leaves the originals). **Point to Point Move**: click where to move
  from, then where to move to (object snaps apply; walls move freely, joined walls
  follow). **Center Object**: a lone door or window centers on its wall at once; for
  other objects click a room, or click two walls to center between them.
- **Make Parallel / Perpendicular**: click a wall or CAD line; each selected wall keeps
  its start and length and swings its far end (joined walls follow).
- With two or more objects selected a **Rotate handle** sits above their box; drag it
  to turn all of them about the center of the box.
- **Align/Distribute...** (Edit toolbar or Edit > Align): Left, Center, Right, Top,
  Middle, Bottom, and Distribute Horizontally / Vertically with equal gaps or a typed
  gap. **Move to Front / Back** orders CAD objects.
- **Edit > Lock / Unlock** locks the layers the selection is on; **Send to Layer...**
  moves walls, CAD, text and symbols to a layer you pick. **View or Edit > Action
  History** lists the undo steps; click one to go back to it, click an undone one to
  redo up to it.
- **Right-click** an object (Select Objects) to select it and open its menu: Open
  Object, the buttons of its type (Reverse Swing and Flip Hinge for doors, Reverse
  Layers and Fix Wall Connections for walls, Rebuild Roofs for roof planes, Explode for
  CAD blocks), Cut, Copy, Paste, Delete, Select Same Type, Group / Ungroup, Lock,
  Send to Layer and Transform/Replicate. Right-click on empty space shows Paste, Paste
  Hold Position, Select All, Undo, Redo and the zoom commands. With a door or window
  tool the menu starts with Select Objects; the drawing tools keep right click as
  their `Esc`.

Wall buttons on the Edit toolbar: **Reverse Layers** (any number of walls; the layer
stack swaps faces and the **main layer stays where it was**: the centerline moves by twice the main
layer's offset, so the framing does not shift, and the ends of walls joined to it follow; a wall whose main layer is
centered does not move; arcs stay concentric), **Break Wall** (then click the wall where it should break; openings
go with the half that holds them), **Remove Break** (merges a straight continuation
back), **Change Line/Arc** (a straight wall becomes an arc with a bulge handle at its
apex; drag it to set the bulge, drag it flat to straighten; openings keep their
proportion along the arc) and **Make Arc Tangent** (refits a curved wall tangent to the
wall it is connected to). Edit > Edit Behaviors changes what dragging does: Default,
Resize (scales a CAD selection from the opposite corner), Concentric (offset copies of a
polyline, line, circle or arc), Fillet (drag a polyline corner to round it), **Chamfer** (drag a polyline corner to
cut it off; the dialog's Chamfer Distance, 0 follows the drag), Alternate (one axis only)
and Replicate (copies at the drag distance; tick **Open Transform/Replicate after the drag** and releasing a
drag opens Transform/Replicate Object with the move and the copy count already filled in).

Select two or more walls and press the Edit toolbar's Open Object (double-click and `Enter` still open one wall) and the
**Wall Specification (Multiple Walls)** opens: General (Thickness, Bottom Height, the Invisible, No Room Definition and
No Locate options), Structure, Foundation, Wall Types, Wall Cap and Layer. A field the walls disagree on is
blank or shows a dash; only the fields you edit are written, all in one undo step.

**Convert to Polyline** turns the selected walls' centerlines into CAD polylines and removes the walls and their openings.

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
| Structure | Works (Default Wall Heights, Platform Intersections, Wall Intersections; the framing groups are dimmed) |
| Roof | Works for exterior walls (roof kind, pitch, upper pitch, overhang, Auto Roof Return; chapter 8.7); dimmed for the other wall kinds |
| Foundation | Works on a placed wall (Footing, Slab chamfers, Sill Plate; 2.6) |
| Wall Types | Works |
| Wall Cap | Works on a placed wall (2.6) |
| Rail Style | Works |
| Wall Covering, Newels/Balusters, Rails | (disabled) |
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
- **Arc** (a curved wall): the **Curved Wall (Change Line/Arc)** check box, then Radius, Arc Angle and Rise
  (each recomputes the others; the ends stay put and Wall Length is the chord), a Bulges Left / Right choice, with
  the arc length and the center point shown below.
  - **Radius to** (Round 13) says which line the Radius number measures: Outer Surface, Main Layer Outside,
    Wall Center, Main Layer Inside or Inner Surface. Switch it and the Radius field shows the same arc to the new
    line; typing a radius sets the arc so that line has it.
  - **Lock** says what holds still when you type a Radius or an Arc Angle: **Ends** keeps the wall's ends
    (the center moves), **Arc Center** keeps the center (the ends move along their radii). Openings keep
    their proportion along the arc when the wall gets longer or shorter.
  - Both are available on a curved wall only. Automatic Facet Angle is disabled.

### Structure

- **Default Wall Heights**: Default Wall Top Height (checked: the wall follows the
  floor's ceiling height and platform), Wall Height, Default Wall Bottom Height (checked: the bottom is 0;
  not in the Default Settings dialogs).
- **Platform Intersections** (placed walls): **Ceiling Platform** (Automatic, Stop at Ceiling Above, Balloon Through
  Ceiling Above, Hang Floor Platform Above on Wall with Subflooring to Wall Interior and Include Ledger) and **Floor
  Platform** (Automatic, Stop at Floor Below, Balloon/Extend Through Floor Below) change how far the wall's top and
  bottom reach in 3D. **Invisible Walls and Railings: Generate Between Platforms** is stored, and the gap is
  measured, but nothing is created yet.
- **Wall Intersections**: Through Wall At Start / At End change the plan outline at that end (3D and framing
  still build the wall to its centerline ends).
- **Rim Joist**, **Double Wall**, **Stud Layout**, **Framing** (disabled): Chief's framing-related options such as
  Bearing Wall and Stud Rollout.

### Foundation and Wall Cap (placed walls; Round 14)

- **Foundation**: the Foundation Wall and Slab Footing check boxes, Wall Thickness; **Footing** (Width, Height,
  Automatic Footing Bottom Height or Footing Bottom, Vertical Footing, Footing Offset, Center Footing on Main
  Layer, Align Footing on Outside); **Slab** (Add Chamfer on Monolithic or Regular Slab, Chamfer Width and Height,
  Monolithic Slab Pour Number); **Sill Plate** (the check box; the Construction list is dimmed). The footing, the
  sill plate (foundation walls) and the cap are built in 3D on straight walls; the slab chamfer, the pour number and the sill
  construction are stored and not built yet.
- **Wall Cap**: Wall Cap, a table of profiles (name, width, height), Full Wall Width, Split Pony Wall, and the
  Horizontal Position (Inside Wall, Wall Center, Outside Wall). The profile table is a fixed list of three: Flat Cap (5 1/2" x 1 1/2"), Overhanging Cap (9 1/2" x 2") and Thick Coping (7 1/2" x 3").

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
Invisible; and the other wall layers. **Drawing Group** is disabled. A standard wall you draw goes on the
active layer of the exterior or interior wall tool (Tools > Layer Settings > Active Layers by Tool, chapter 5).

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
- **Model ready, no UI**: the attic flag on the wall (the Attic Wall check box stays dimmed; the per-wall roof directives are on the Roof tab, chapter 8.7). The attic walls that fill the gap between a lower roof and the wall above it are made in 3D by **Auto Attic Walls** in Roof Defaults (chapter 8.4a), which also sets their wall type and the type of the part of a wall below a butting roof.

## 2.10 Known differences from Chief

- Rooms are traced from centerlines, then offset to the interior faces for areas and labels (chapter 4).
- Walls T-split at junctions; Chief keeps the through wall whole.
- Connections are inferred from coordinates each time.
- Curved walls are mitered in plan and in 3D against the walls that join them by treating the
  arc as its tangent line at the joined end (a wide arc at a steep corner shows a small kink); a
  curved wall is a run of 7.5 degree facets in 3D; the plan symbols of its doors and windows sit
  on the chord (2.2, Curved Wall Tools).
- Pony, glass, half-wall, railing, deck and fencing default types and heights are typical values, not
  Chief's, and are not editable from Default Settings. Terrain walls and curbs are terrain objects, not wall classes (chapter 9.6); Wall Hatching, Wall Material
  Region and the Polygon Shaped Deck are in chapter 17.
- The Wall Covering, Newels/Balusters and Rails tabs of the Wall
  Specification are still (disabled), and the Roof tab is dimmed on interior and other non-exterior walls.
  Crossing walls with the T-split turned off overlap in plan (their fill and layer lines are not merged at the
  crossing).

## Wall Type Definitions (Round 16)

Build > Wall > Define Wall Types opens the dialog without a wall selected (it also opens from the Define button of a Wall Specification). The table runs from the exterior face to the interior face in three sections, Exterior, Main and Interior layers. Each row has a Main checkbox (several Main layers may sit together; the last one stays), Name, Thickness, Extension (Exterior layers only, the highest sets the Brick Ledge Depth), Role (Framing, Air Gap, Standard, Cladding, Finish), a Fill swatch (click to open the layer's Fill Style) and Material. Insert Above/Below, Delete, Move Up/Down and Edit Layer work on the selected row; Total Thickness is taken by the outermost Main layer, down to 1/16 in. Copy starts a new type from the current one, Room Divider adds a 0 in divider type, Delete All Unused removes the types no wall or default uses (the type on screen stays), Import brings in the types of another plan (a clash arrives as `Name_2`).

Edit Layer opens the Wall Layer Specification (General, Line Style, Fill Style, Materials). The Wall Properties tab sets Dimension to Exterior of Layer, Foundation to Exterior of Layer and its offset, Build Platform To This Line, the roof layer, Partition Wall, Room Divider and the Energy Values. The preview turns when dragged, or shows the plan view with the layer fills. OK is one undo step; the walls of an edited type take its new thickness.

