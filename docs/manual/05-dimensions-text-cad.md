# Chapter 5: Dimensions, Text and CAD

This chapter covers everything you draw on top of the model: dimensions,
text and annotation, 2D CAD primitives and the CAD edit tools, and the layers they live on.

## 5.1 Choosing a variant

Chief puts each family of tools (Dimensions, Text, Lines, Arcs, Circles, Boxes) on a flyout
button. Picking a flyout entry starts the tool already in that variant, so the Dimensions
flyout's "Running Dimension" really starts a running dimension. The hotkey of a variant does the
same.

Inside the tool you can also switch variants with the **option strip**: a row of small buttons that
the Dimension, Text and CAD tools draw along the top edge of the canvas while they are active.
Click a button to change mode without leaving the tool. The strip reaches a few modes that have no
flyout entry of their own, for example Spline, Revision Cloud, Hatch and the Regular Polygon sides.
The Stairs, Electrical, Terrain, Roof and Cabinet tools take the flyout choice the same way
(chapters 6 to 9).

## 5.2 Dimension tools

Dimensions are objects on the `Dimensions, Manual` or `Dimensions, Automatic` layer.
Each stores two measured points, a signed offset for the dimension line, a kind and an
optional text override. A dimension with a number like `12'-6 1/2"` is drawn with
extension lines, a dimension line with ticks, and the value above.

```
   |<------------ 12'-6 1/2" ------------>|     dimension line + ticks
   |                                       |
   |                                       |     extension lines
 ==+===                                 ===+==   walls (located to the main layer)
```

Points **locate** to the objects under the pointer by the Locate Objects settings of the active
Dimension Defaults (5.9): a wall's surfaces, main layer or centerline, an opening's sides or center (or the
wall behind it), the sides of a cabinet or of a placed fixture, otherwise the snap point. Walls marked No Locate
and hidden walls are skipped. Hold `Alt` to suspend the locating. Every completed dimension is one undo
step. `Esc` cancels one in progress.

### Dimension Tools (row 2, Dimensions flyout; CAD > Dimensions)

| Variant | Hotkey | Click | Notes |
|---|---|---|---|
| Manual Dimension | `Ctrl+Alt+Cmd+A` | Click two points (or press-drag-release), then click to place the line. | Measuring direction (horizontal, vertical or aligned) follows the placement click. |
| End to End Dimension | `D, E` | Click a wall or line, then click to place. | Measures its full length. |
| Interior Dimension | `D, I` | Click inside a room, then click to place. | Interior surface to interior surface. |
| Point to Point Dimension | `Ctrl+Alt+Cmd+B` | Two points, then place. | Aligned, straight distance. |
| Running Dimension | `Ctrl+Alt+Cmd+C` | Click the points; `Enter` or double-click places the string. | Cumulative distances from the first point. |
| Baseline Dimension | `Ctrl+Alt+Cmd+D` | Click the origin, then each point. | Each dimension stacks outward at a fixed spacing. |
| Angular Dimension | `Ctrl+Alt+Cmd+F` | Click the vertex and two arm points, then the arc radius. | Degrees. |
| Centerline Dimension | `Ctrl+Alt+Cmd+G` | Two centers, then place. | Locates to the centers of openings and objects. |
| Tape Measure | `D, T, M` | Two points. | Reads the distance; adds nothing to the plan. `Esc` clears. |

### Automatic Dimension Tools

| Variant | Hotkey | Today |
|---|---|---|
| Auto Exterior Dimensions | `Shift+A` | Click once: up to **three strings** on each side of the building, nearest the wall first. Chief's order is **Openings** (door and window sides, or centers), **Wall to Wall** (the corners and the walls that meet that side) and **Overall** (corner to corner). The first string sits the Exterior Offset (32" in Daniel's set) from the wall's outer surface and the strings are a Line Separation apart; Setup Automatic in the Dimension Defaults chooses and orders the strings. The exterior walls (all walls when none is Exterior; room dividers and No Locate walls are left out) are grouped by direction, so a building or wing turned off the axes gets strings parallel to its walls. A string that would repeat another or has nothing to show (no openings on that side) is left out. **Curved walls** take part: a curved exterior wall is dimensioned by its chord and tangent points (it joins the frame its chord lies in but never starts one), and its bulge pushes the strings of that side outward so they clear the arc. Re-running replaces the previous exterior run and keeps manual dimensions. |
| Auto Interior Dimensions | | Click once: for every room larger than 10 sq ft, one horizontal and one vertical clear dimension (interior surface to interior surface, or by the Walls setting when "Interior dimensions locate interior surfaces" is off) through the room, and an **openings string** along each wall of the room with doors or windows (the corners and the openings' sides or centers; none when Locate Openings is None). Re-running replaces the previous interior run. |
| Auto Elevation Dimensions | `Ctrl+Alt+Cmd+H` | Click once where the string should go: a vertical string of level heights (floor platforms, ceiling heights, heights above the first floor) with each level named beside it. |
| Auto Story Pole Dimensions | `Ctrl+Alt+Cmd+I` | Click once: a story pole, the same kind of vertical string of level heights. |
| Auto NKBA Dimensions | (option strip only: **Auto NKBA**; no flyout entry or hotkey) | Click once: for every run of kitchen and bath cabinets (base and tall cabinets, and free-standing appliances, that touch side by side with their fronts on one line) up to three strings on the front side, nearest the cabinets first: **cabinet faces** (every cabinet, from the wall at one end to the wall at the other when a wall stands within reach of the run's end), the **centers of the sinks, cooktops and appliances** over the same extent (left out when the run has none), and the **overall** length (left out when it would repeat the faces string). Spacing and reach come from the "NKBA" dimension set when the plan has one, else the active set; its text style is used. Walls marked No Locate and room dividers are ignored. Re-running replaces the previous NKBA run, "No base cabinet runs to dimension" says there was nothing to measure, and the result is one undo step. |

The plan has no elevation view, so the Y axis of an elevation or story pole string is the height: they are strings in plan coordinates, not dimensions drawn on an elevation.

### Editing dimensions

Select a dimension by clicking its line, extension or text. Its handles:

- The **dimension line** handle moves the line; the extension lines stretch to follow.
- The **end** handles relocate either measured point.
- Click the **value text** to edit it inline. Type a length and press `Enter`: the
  object at the end nearer the pointer moves so the segment equals the new value (a
  wall moves perpendicular, or lengthens if the dimension runs along it; openings
  slide). A locked layer refuses the change.
- Double-click opens the Dimension Specification (5.7).
- **Add Extension Line** and **Delete Extension Line** (Round 14) edit an existing string: click a dimension line where a new measured point goes (or on a hidden extension line to bring it back); click an extension line to delete it, which merges the strings on both sides into one or hides an end's line. Both tools are built and reachable through the option strip of the Dimension tools; the Dimension flyout has no button for them yet (planned). Copying a dimension keeps its ties to the walls and openings it measures.
- The **Edit toolbar** of a selected dimension offers four commands, each one undo step (a locked layer refuses them):
  - **Reverse Dimension** swaps the two measured points (with their ties and extension-line switches), which puts the dimension line on the other side of what it measures, the same distance away.
  - **Convert to Manual Dimension** turns an automatic dimension (any string of Auto Exterior, Interior, Elevation, Story Pole or NKBA) into an ordinary manual one that a later Auto run no longer replaces. Enabled only when a selected dimension is automatic.
  - **Align Dimensions** (two or more selected) moves the dimension lines of the dimensions parallel to the first one selected onto its line.
  - **Distribute Dimensions** (three or more) spaces the dimension lines of the dimensions parallel to the first one evenly between the two outermost; the two ends stay.
  The status bar says how many changed, or "nothing to change" (for example when none of the others is parallel).

### Associative dimensions

A measured point that was located on a wall (its start, end or a place along it, with the distance to the side
and the outer-corner extension), an opening (an edge or the center), a cabinet or a placed fixture is **tied** to it.
When the object moves, stretches or is reshaped, the point follows (the editor re-resolves every tie whenever the plan
changes), so dimensions, including the automatic strings, stay on the walls and openings they measure. Dragging a
point by hand, or typing a new value, drops the tie of the end you moved and turns an automatic dimension into a manual one;
a point dragged onto another object is tied there. The Dimension Specification (5.7) names the object each end is tied to.

Not built: ties to other kinds of objects (stairs, roof planes, framing).

**Printed-size text and picking.** A text or dimension in a Printed Size style is drawn at its size on paper at the sheet's scale. Clicking it with Select Objects picks it by the box it is drawn in at that scale (not by its stored plan height), so a 1/8" label at 1/8" scale is as easy to hit as it looks. The DXF export (12.3) and the construction set PDF honor the same sizes and the same hidden extension lines: a dimension whose Show Extension Line switch is off at an end writes and prints no extension line there, and text takes the height of its text style (a printed-size style is converted to plan inches at the sheet's scale).

### Temporary dimensions

Temporary Dimensions (view bar, on by default) show a live readout while drawing
and dimensions to nearby objects for the selected one. They are never saved or
printed. Clicking one turns it into an edit field (see chapter 2.5). A selected **CAD object** shows its own (Round 14): a line its length and angle, a box its width and height, a circle its radius and diameter and an arc its radius; typing a value resizes the object (a line keeps its start, a box its lower-left corner, a circle its center). A **padlock** beside a measuring value locks it: the temporary dimension becomes a permanent manual dimension on the Dimensions layer, tied to the objects it measures, and clicking the padlock again takes it away. Which part of a wall or opening they measure to is the **Temporary** group of Locate Objects in the Dimension Defaults (5.9); the **Elevation** group is stored for the level dimensions but no tool reads it yet. Both groups default to wall surfaces and opening sides.

## 5.3 Text tools

All text and annotation tools are in the Text flyout (row 2; CAD > Text). Every
annotation is a CAD item on the `Text` layer: text is a text item; leaders and
callout frames are polylines and circles grouped with their text so they select
together.

| Variant | Hotkey | How it works |
|---|---|---|
| Text | `Y` | Click to set the text's lower-left anchor, type, `Enter` to commit. A click elsewhere commits and starts the next text. `Esc` cancels. Clicking existing text edits it in place. **Press and drag** instead of clicking (Round 14) to draw a **text box**: its width wraps the text, its drag height is a minimum box height, and a ghost shows the box; a drag narrower than about three characters is an ordinary click. Rich Text takes the same drag. |
| Rich Text | `Ctrl+Alt+Cmd+J` | Click, type; `Enter` adds a line; `Tab` finishes. Bold, italic, underline, size and color are stored with the text as runs (below). |
| Leader Line | `Alt+L` | Click the arrow tip and the bends; double-click or `Enter` ends. |
| Text Line with Arrow | `Alt+A` | As Leader Line, then asks for the text at the end. |
| Callout | `Ctrl+Alt+Cmd+K` | Click the target, click the callout position, type. A circle, hexagon or **square** frames the text (pick the shape in the option strip). |
| Marker | `Ctrl+Alt+Cmd+M` | Click to place the next numbered marker (a numbered circle). |
| Note | `Ctrl+Alt+Cmd+N` | Click, type; the text reads "Note n: ..." with the next free number of the active note type. |
| Note Type Management | | Opens the dialog below. |
| Text Macro Management | | Opens the dialog below. |
| Text Style Management | | The **Text Style Management** window (Round 13): pick a style, then **Rename** it or **Remove** it. A rename follows the style everywhere it is used (the plan's layers, layer-set overrides, saved plan views, CAD text, dimensions and placed schedules); a remove sends its users back to the layer's style. The Default Text Style can be neither renamed nor removed. OK applies the changes. The window is built (the Text tool's Styles mode) but the Text flyout has no entry for it yet (planned); Default Settings > Text > Text Styles renames and removes styles in the meantime (5.9). A note type does not follow a style rename. |

Text height defaults to the template's 6" plan height. A text style can instead be a **Printed Size** style
(Default Settings > Text Styles, 5.9): its text keeps its size on paper (a 1/8" label stays 1/8" at 1/4", 1/8" or 1/2" scale)
because its plan height is recomputed from the sheet's scale; a new text on such a layer is placed at the style's own height, and the
Text Specification says how big it is on paper. A **Character Height** style keeps the plan height you give it.
Edit > Find/Replace Text finds a string in the text objects of the floor (or every floor) and replaces it, Replace All as one undo step. **Replace Fonts** (Default Settings > Text > Text Styles, 5.9) swaps one font family for another in every style at once; the Edit > Replace Fonts... menu line itself stays dimmed.

### Rich text

Rich Text keeps formatting as **runs** stored with the text object. Type the formatting inline as markup, or use the **B**, **I** and **U** buttons in the option strip:

| Markup | Effect |
|---|---|
| `<b>...</b>`, `<i>...</i>`, `<u>...</u>` | Bold, italic, underline. |
| `<size=1.5>...</size>` | The run at 1.5 times the text height. |
| `<color=#RRGGBB>...</color>` | The run in that color. |

The Text Specification (5.8) shows the markup in its Text tab (tick *Rich text*). Limits: italic is stored but not drawn, because the on-screen font family has no italic face.

### Text macros

A `%macro%` in text is replaced when the text is placed or edited. The built-in macros:

| Macro | Gives |
|---|---|
| `%room.name%`, `%room.number%`, `%room.area%` | The name, number and floor area of the room under the text |
| `%plan.name%`, `%plan.date%` | The plan's name; today's date as `YYYY-MM-DD` |
| `%floor%`, `%floor.number%`, `%floor.count%`, `%floor.height%` | The floor's name; its number (1 is the lowest); the number of floors; its ceiling height |

**Text Macro Management** adds your own: a name (letters, digits, `.`, `_` or `-`; not a built-in's name; unique) and the text it expands to, used as `%name%`. They are saved with the plan.

### Note types

**Note Type Management** lists the kinds of note, each with a label prefix and a text style: General Note (`Note`), Construction Note (`C`), Framing Note (`F`) and Electrical Note (`E`) to start with. The Note tool writes the active type's prefix and numbers each type on
its own: `Note 3:`, `E 1:`. Add a type with a name and a prefix of letters and digits. Types are saved with the plan.

## 5.4 CAD drawing tools

CAD objects are drawn on the **current CAD layer**, `CAD, Default`. The Current
CAD Layer button is (planned), so new CAD objects always land on that layer. Object
snaps, angle snaps and the grid apply to every CAD tool. In the tools that take a typed value (a length and angle, a radius, a distance)
`Enter` starts the typed fields.

### Points, Lines, Arcs, Circles, Boxes

| Flyout | Variant | Hotkey | Gesture |
|---|---|---|---|
| Points | Place Point | | Click to drop a point. |
| | Input Point | | Type X, `Tab`, Y, `Enter`. |
| | Point Marker | | Click to drop a marked point. |
| | Delete Temporary Points | | A command: removes every point Place Point and Input Point dropped (they sit on the layer `CAD, Temporary Points`; Point Marker points stay). |
| Lines | Draw Line | | Click start, click end, or press-drag-release. With Connect CAD Segments on (`Shift+F8`) the next line starts where the last ended. `Enter` after the first click types a length and angle. Hold `Shift` to hold the line to 15-degree steps. |
| | Input Line | | Click start, type length, `Tab`, angle, `Enter`. |
| | Line With Arrow | | Click the start, click the arrow tip. |
| | Polyline | | Click vertices, click the first to close, `Enter` or double-click ends. |
| Arcs | Draw Arc | | Click the points of the arc. Four **Arc Creation Modes** (Edit > Arc Creation Modes, or the option strip): **Three-Point** (start, end, then a point the arc passes through), **Center-Start-End** (center, start, end), **Start-End-Radius** (start, end, then a point on the side the arc bulges to; its distance from the middle of the chord sets the radius) and **Tangent** (start, end, then a point giving the tangent direction at the start; two clicks when the start is the end of the line or arc just drawn, which the new arc continues without a corner). |
| | Input Arc | | Click the center, type radius, start angle and sweep. |
| | Arc With Arrow | | Three-point arc ending in an arrowhead. |
| Circles | Circle | `K` | Click the center, click a point on the circle. |
| | Circle About Center | | Click the center, click or type the radius. |
| | Ellipse | | Center, an axis end, then a point for the other radius. |
| | Oval | | Two opposite corners of the bounding box. |
| Boxes | Rectangular Polyline | `Shift+P` | Click two opposite corners. |
| | Regular Polygon | | Click the center, then a vertex (sides in the option strip). |
| | Box | | Click the ends of one edge, then click the depth. |
| | Cross Box | | The same gesture; drawn as a box with an X through it. |
| | Blocking Box | | The same gesture; drawn as a box with one diagonal. |
| | Insulation | | Click the ends of one edge, then the thickness; drawn as a box holding a wave. |

(Cross Box, Blocking Box and Insulation follow Chief's usual symbols; check them against Chief before you rely on the exact look.)

Other CAD tools: `Backspace` drops the last vertex of a polyline. **Spline** places points and draws a smooth curve through them (a **Fit** or **Bezier** toggle and a tension setting in the option strip; a click on the first point closes it; `Enter` or
a double-click ends). **Revision Cloud** works like a polyline: click the outline, click the first point to close. The CAD menu lists both; on the toolbar the Revision Cloud button is a dimmed toggle, but both modes are reachable from the CAD menu and the CAD option strip while a CAD
tool is active. An ellipse is stored as a 48-segment closed polyline.

### CAD Blocks

CAD blocks are named groups of CAD objects you can insert again. The **CAD Blocks** flyout and the CAD menu hold:

| Command | What it does |
|---|---|
| Make CAD Block | Groups the selected CAD objects into a block (they select and move together). |
| Explode CAD Block | Ungroups the selected block. |
| Add Insertion Point | Click a block, then click the point by which it will be placed. |
| Add Arrow Backoff Point | Click a block, then click where arrows stop. |
| Edit CAD Block | Select a block to edit its name and points. |
| CAD Block Management (`V`) | A dialog listing the plan's blocks: rename, insert, edit or delete. |
| Insert CAD Block | From the manager: click where the block's insertion point goes. |

The commands that act on a selection run on the first frame after you pick them and return to Select Objects. Copy and paste carries a block whole when every object of the block was copied (its name, insertion point and the objects' styles come with it); copy only some of its objects and you get plain objects.

### Editing CAD objects

With Select Objects a selected CAD object shows handles: line ends and midpoint, polyline
vertices with "add vertex" handles at edge midpoints, circle center and radius. Drag to
edit. Double-click opens the CAD Specification (5.6).

The **CAD edit tools** are in the CAD menu (CAD > Edit CAD, CAD > Patterns) as tool modes; each is one undo step:

| Tool | Gesture |
|---|---|
| Fillet | Click two lines (or a polyline corner); `Enter` types the radius first. |
| Chamfer | Click two lines (or a polyline corner); `Enter` types the distances first. |
| Offset | Click an object, then the side; `Enter` types a distance (0 means through the click). |
| Trim Line | Click the part of a line to remove at its nearest cutters. |
| Extend Line | Click the end of a line to extend it to the next object. |
| Break Line | Click the point where a line or polyline splits. |
| Change Line/Arc | Click a line, an arc or one edge of a polyline: it becomes curved (bulging toward your click) or straight. A selected polyline shows a **diamond handle** on each arc edge; drag it to set the bulge. Arc edges are stored as sample points every 7.5 degrees, so DXF and the layout see a smooth curve. |
| Delete Break | Click a polyline vertex to remove it; the two edges around it become one straight edge. |
| Make Arc Tangent | Click an arc edge (or an Arc object): it turns so it leaves its neighbor at the shared end without a corner. |
| Reverse Direction | Click a line or polyline to reverse it. |
| Make Parallel | Click the end of a line to turn, then the line to match. |
| Make Perpendicular | Click the end of a line to turn, then the line to square to. |
| Convert to Polyline | Select connected lines, then use the command. |
| Convert to Spline | Select polylines, then use the command. |
| Convert Polyline to Lines | Select polylines, then use the command. |
| Hatch (CAD > Patterns > Hatch Closed Shape) | Click inside a closed polyline or circle; pick the pattern in the option strip. The pattern is drawn as real lines grouped with the outline. The Fill Style tab of the CAD Specification does the same. |
| CAD Detail From View (CAD menu) | Copies the view's wall outlines, CAD items and dimensions into a new floor named "CAD Detail" above the current one and makes it current. |

CAD Detail From View makes a normal floor, which takes part in the 3D stack; Chief's detail windows and CAD Detail Management do not exist here. With a drawn CAD object selected the Edit toolbar has a button for each of these tools (Fillet, Chamfer, Offset, Trim, Extend, Break, Reverse Direction, Make Parallel, Make Perpendicular and the three converts): the button switches to that CAD tool mode with the selection kept, and the converts act on the selection at once. Separately, Edit > **Make Parallel** and **Make Perpendicular** (and the Edit toolbar's buttons for a selection of walls and CAD lines) work from a selection: with the selection made, click the wall or line to match and each selected one keeps its start and length and swings its far end to be parallel or perpendicular to it (chapter 2.5).

The Edit menu's selection commands (Cut, Copy, Paste, Duplicate, Select All, Select Same Type, Group, Transform/Replicate, Reflect About Object, Point to Point Move, Align/Distribute, Move to Front / Back, Lock, Send to Layer) all work on CAD objects, text and dimensions as well as on walls (chapter 2.5). **Move to Front / Back** is for CAD objects and text: CAD objects are drawn in list order, so it moves the selection to the end (top) or the start (bottom) of the list. **Edit > Edit Behaviors > Resize, Concentric, Fillet, Alternate and Replicate** change what dragging a CAD selection does (chapter 2.5).

## 5.5 Layers and layer display

Every object belongs to one layer. Daniel's template starts with 16 layers (Walls,
Normal; Walls, Invisible; Doors; Windows; Rooms; Room Labels; Dimensions, Manual;
Dimensions, Automatic; Text; CAD, Default; Cabinets, Base; Cabinets, Wall; Electrical;
Stairs; Roof Planes; Framing), each with a color and line weight. The tools add layers as they
need them: `Ceiling Planes`, `Slabs`, `Piers/Pads`, `Floors, Holes`, `Ceilings, Holes`, `Deck Railing`,
`Fencing` and the manual framing layers (`Framing, Floor Joists`, `Framing, Rafters`, `Framing, Posts`,
`Framing, Beams`, `Framing, Trusses`). Roof, electrical, terrain, slab and CAD-style data no longer ride on
hidden `... , Data` layers: they live in typed fields of the plan, as does framing (chapter 12.2), and files
that still carry those layers are converted when opened.

**Active Layer Display Options** (view bar, View menu, Tools > Layer Settings > Display
Options...) shows the plan's layers as a table. The dock and the window are the same widget.

- **Columns**: Name, Used (object count), **Disp**, **Lock**, **Ref** (does the layer show on the Reference Display floor, 4.5), Color,
  **Weight**, **Line Style** and **Text Style**. A cell edits the layer in the layer set that is shown. A name filter narrows the rows,
  and the table can be sorted by name, Used, Disp, Lock, Ref or weight.
- **Disp** off hides the layer: its objects are not drawn, picked or snapped to, but they
  still exist and still form rooms.
- **Lock** on makes objects visible and snappable but not selectable for editing; moving
  or deleting them is refused with a status message.
- **Selecting rows.** Click a name to select one row, Cmd/Ctrl-click to toggle a row, Shift-click for a range; **Select All** and
  **Select None** act on the rows the filter shows. When the edited row is part of a multi-selection the edit goes to every selected
  row (turn Disp off on twelve layers with one click). Right-click a name for **Select All on Layer** (selects the layer's objects
  in the plan) and **Reset to Defaults**.
- **Layer set buttons** above the table: the set drop-down and **New**, **Copy Set**, **Rename**, **Delete** and **Manage...**.
  **Modify All Layer Sets** sends every edit to all sets at once instead of only the shown one.
- **Properties for Selected Layer**: Display, Lock, Color, Line Weight, Line Style, Text Style
  and Fill Style, and **Copy To Other Sets** (tick the sets, then copy the selected layers' look to them).
- The window version adds **New Layer...**, **Layer Set Management...** and **Active Layers by Tool...**. All edits are undoable
  (one step per edit; a drag shares one).
- **Layer sets and saved plan views.** A layer set is a named table of per-layer overrides (display, lock,
  color, line weight, line style, text style) laid over the plan's base layers. A saved plan view carries a
  layer set, a floor, a reference-display setting and a zoom. The **Layer Set** drop-down in the dock
  switches the active set; the saved-view selector in row 1 activates a view (its layer set, floor and zoom;
  one undo step). A new plan starts with "Default Set" and "Floor Plan View". Daniel's 34 layer sets
  (Presentation, Working, Electrical, Foundation, Roof Plan ...) come in through File > Templates > Import
  Chief Template....

**Tools > Layer Settings > Layer Set Management...** lists the plan's layer sets with **Make Active**, **New...** (a set whose
layers all take the base layers' look), **Copy...**, **Rename...** (the plan views that show the set follow) and **Delete**, and
**Import From Plan File...**: pick another `.psplan`, tick the layer sets to bring in and they arrive with the layers they need
(one undo step).

**Tools > Layer Settings > Active Layers by Tool...** lists the layer each tool draws on, with a drop-down per tool (walls, doors,
cabinets, dimensions and so on) and the **Current CAD Layer** new CAD objects go on; **Reset to Default Layers** puts them back.

**Tools > Plan Views** manages the saved plan views:

- **Plan View Specification...** edits the active view: its name, layer set, floor, Reference Display on or off and which floor is
  the reference (relative to the floor viewed), the default dimension set and text style, and the zoom; it also makes New,
  Duplicate and Delete.
- **Save Plan View** stores the floor, reference display, zoom and pan the plan shows now in the active view (one undo step; the row 1
  button Save Active View does the same). **Reset Plan View** shows the active view as it was saved.
- **Add Template Plan Views** adds Daniel's 20 template plan views (each with its layer set) that the plan lacks, in one undo step; it
  says so when they are already there. A new plan that holds only the starting view gets them too.
- Views open as **tabs** above the canvas (1.3): switching tabs keeps each view's floor, reference display, zoom and pan. The strip shows when two or more views are open: click a tab to switch, drag to reorder, click x or middle-click to close, **+** to open another saved view.

- Line weights are stored in hundredths of a millimeter; View > Line Weights scales the on-screen strokes by them (chapter 1.4). A CAD object can carry a weight, color and dash of its own (5.6).
- **Typed storage** (Round 8). A CAD object's own style (color, weight, dash, fill, arrows, rich text runs), the name and insertion point of a CAD block, the plan's text macros and its note types are no longer small hidden records on a locked `CAD, Data` layer. They are typed, serde-default fields of the plan: `Floor.cad_attrs` and `Floor.cad_blocks` for each floor, `Project.text_macros` and `Project.note_types` for the project (chapter 12.2). They save, undo and copy with the plan. A file from before this change is converted when it opens (the old records move into the fields once and the `CAD, Data` layer is removed; it is not an undo step). Deleting a CAD object deletes its style, and a block goes when none of its objects is left. Copy carries the styles of the copied objects and every CAD block whose objects were all copied, and Paste in Place gives each pasted block a group, name and points of its own, so the earlier remark that a pasted block loses its name and attributes no longer applies to the clipboard. A deleted block's empty object group stays in the group list (harmless).

## 5.6 Dialog: CAD Line / Polyline Specification

Double-click a line, polyline, circle or arc. One dialog serves all of them. OK applies the geometry and the look as one undo step.

| Tab | Fields |
|---|---|
| General | **Line**: Start X, Start Y, End X, End Y, Length, Angle. **Circle**: Center X/Y, Radius, Diameter. **Arc**: Center X/Y, Radius, Start Angle, End Angle, Sweep, Arc Length. **Polyline**: Vertices, Perimeter, Area, Vertex List, Closed. |
| Line Style | **Line Weight**: *Use a weight of its own* and the weight (else the layer's). **Color**: *Use a color of its own* and the color (else the layer's). **Line Style**: By layer, Solid, Dashed, Dotted or Dash-Dot. |
| Fill Style | Closed shapes only: **No Fill**, **Solid** or **Pattern**; a Color and an Opacity (lower it to see through a solid fill); for a pattern, the pattern and its Spacing. A pattern is drawn as lines grouped with the shape when you press OK. |
| Arrow | Open shapes (lines, arcs, open polylines) only: **Start** and **End** arrow style (None, Open, Filled, Tick or Dot) and the **Arrow Size**. These add arrowheads to the line itself; Line With Arrow makes separate shapes. |
| Layer | The layer, editable. |

The plan draws the object's own color, weight, dash, solid fill and arrow ends. Not drawn: a concave solid fill paints as a convex polygon.

## 5.7 Dialog: Dimension Specification

| Tab | Fields |
|---|---|
| General | Type and Value; the Offset From Measured Line; the measured points (Start X/Y, End X/Y); **Located Objects**: for each end what it is tied to (a wall, an opening, a cabinet, a fixture, or "Free point") and a **Show Extension Line** check box per end that hides or shows that end's extension line (stored with the dimension). |
| Primary Format | Round 14: the check box **Use the Dimension Defaults' format** (on: the format of the active Dimension Defaults, shown read-only). Clear it and this one dimension has its own **Units** (Feet and Inches, Inches, Decimal Feet, Millimeters, Centimeters, Meters), **Smallest Fraction** (1/2 to 1/64) or **Decimal Places** and **Show Trailing Zeroes**, **Show Unit Indicators** and **Suppress Zero Feet**; a line shows how the value then reads. |
| Arrow | Round 14: **Use the Dimension Defaults' arrows**, or this dimension's own **Style** (Tick, Arrow, Dot, None), **Size** and **Filled**; and **Extension Lines**: **Use the Dimension Defaults' extension lines**, or its own **Gap From Marked Object**, **Length Past Dimension Line** and **Fixed Extension Line Length** with the **Length From Dimension Line**. |
| Text Style | **Style**: "From Dimension Defaults" or any text style of the plan (stored with the dimension). Below it the style's Font, whether the Size is a Character Height or a Printed Size, the size **on paper** at the sheet's scale (inches and points) and the height **in the plan**. Position (Centered On / Above / Below Dimension Line) is shown disabled. |
| Layer | Manual or Automatic dimension layer. |
| Label | Value Text: **Specify the dimension text** and type a replacement for the measured value (the measured value is shown beneath). |

The Dimension Defaults sets (1/4" Scale, 1/8" Scale, Electrical, Framing ...) are kept in the plan
defaults and edited from Edit > Default Settings > Dimension > Dimensions (5.9). The set marked
Currently Active (the template's 1/4" Scale set in Daniel's: smallest fraction 1/8, diagonal
fractions) is what dimensions use.

## 5.8 Dialog: Text Specification

| Tab | Fields |
|---|---|
| Text | The text, with a *Rich text* check box that shows it as markup (`<b>`, `<i>`, `<u>`, `<size=1.5>`, 5.3), Angle, Position (lower left X and Y). |
| Text Style | **Style**: a named text style of the plan, or "(layer's style)"; the font it gives; **Format** check boxes Bold, Italic and Underline for the whole text (mixed formats are typed as markup on the Text tab). |
| Appearance | **Size**: Text Height. **Alignment** (Left, Center, Right; Top, Middle, Bottom when the box is taller than the text). **Text Box** (Round 14): **Wrap Text at Box Width** with the **Box Width**, and the **Minimum Box Height** (the box grows taller to hold the text; 0 fits it). **Border** with its **Margin** and **Line Weight** (0 follows the layer). **Background Fill** and its **Fill Color**. The same box is drawn in the plan, on layout pages and in the PDF; the DXF export writes a boxed text as one text entity without its wrapping, border or fill. |
| Layer | The layer, editable. |

## 5.9 Default Settings: Dimensions and Text Styles

### Saved Dimension Defaults

Edit > Default Settings... > Dimension > **Dimensions** opens **Saved Dimension Defaults**: the dimension sets
of the template.

- **Currently Active** is a combo of the set names; the active set is the one new dimensions and the auto
  dimension tools use, and the list marks it "(active)".
- Click a row to select it. **Edit...** (or double-click) opens the set, **Copy** adds "<name> Copy" (made unique)
  below it, **Rename...** asks for a name, **Delete** removes it. Names must be filled in and unique. The active
  set cannot be deleted ("Make another set active before deleting this one") and the last set stays.
- OK saves the draft into the defaults ("Saved the dimension defaults"); Cancel or Escape drops it.

**Dimension Defaults - <name>** is a six-tab editor with a preview that draws a sample dimension (12'-6 1/2")
in the set's format. A length field turns red when it does not parse and OK is blocked ("Fix the highlighted field").

| Tab | Fields |
|---|---|
| Primary Format | Units (Feet and Inches, Inches, Decimal Feet, Millimeters, Centimeters, Meters), Smallest Fraction (1/2 to 1/64), Fraction Style (Diagonal, Horizontal, Stacked), Decimal Places (0 to 6), Unit Indicators, Trailing Zeroes |
| Setup Automatic | Exterior Offset, Line Separation, and **Exterior Strings**: String 1, 2 and 3, each Openings, Wall to Wall, Overall or None, nearest the wall first (Openings, Wall to Wall, Overall by default; all None keeps Overall) |
| Extensions | Gap from Object, Extend Past Dimension Line |
| Arrow | Arrow Size, Leader Style (Square Corner, Round Corner, Diagonal) |
| Text Style | Text Style (the name of a text style; its font and size are used), **Printed Size** (the dimension text and arrows keep their size on paper at any scale), Text Above Dimension Line, Fraction Text Size (25 to 100 %) |
| Locate Objects | Three groups, picked by the buttons at the top of the tab: **Manual and Automatic** (the set's own), **Temporary** (the temporary dimensions, which read it) and **Elevation** (stored for the level dimensions; nothing reads it yet). Each has **Walls**: Surfaces (full thickness), Main Layer (default for the first group) or Centers; **Openings**: Sides, Centers or None (the wall behind is located); **Cabinets** and **Fixtures**: Sides or None. The first group also has "Interior dimensions locate interior surfaces". A dimension set saved before these settings loads with the defaults; Daniel's template keeps locating opening centers. |

### Text Styles

Edit > Default Settings... > Text > **Text Styles** edits named text styles for two places, chosen by the
**Edit styles of** radios: **This plan** (the open plan's styles, one undo step, "Text Styles") and **New-plan
defaults** (what plans start from).

- The list is on the left. The form on the right has Name, **Font** (a picker of the fonts installed on this
  computer, with a search box; the Chief names Avenir, Arial, Arial Narrow and Chief Blueprint are listed too and
  marked when this machine does not have them; a preview line under it is set in the face the style will use,
  Bold and Italic choosing the real bold and italic faces), Height (0.25" to 96"), **Size by** (Character Height or Printed Size), Style (Bold, Italic, Underline) and Color.
  **Character Height** keeps the plan height; a character-height style prints at its height times the sheet scale (a 6" style is 1/8" at 1/4" scale).
  **Printed Size** shows a size in inches on paper (0.02" to 2") that holds at any scale, so the plan height changes with the
  sheet's scale. **New** adds a style, **Copy** duplicates the selected one, **Delete** removes it.
- Names must be filled in and unique. "Default Text Style" can be neither renamed nor deleted.
- Renaming a style in the plan renames it on every layer, layer-set override, saved plan view, CAD text,
  dimension and placed schedule that used it (Round 13). OK applies both lists ("Saved the text
  styles"); Cancel or Escape drops the changes.
- **Replace Fonts** (Chief's TXT-12), in the same dialog: choose the family to replace (the list offers the
  families the styles use now), choose the family to use instead with the same picker, and press **Replace in all
  styles**. The note under the button says how many styles changed. It edits the list on the screen, so it
  takes effect with the dialog's OK, as one undo step ("Text Styles").
- **Fonts on screen and on paper.** A style's font is drawn in the installed font of that name: plan text objects,
  dimension numbers, room labels and the layout window's box text on screen, and the same fonts embedded in the PDFs
  you make here (chapter 12.6). A font family that is not installed falls back to a close stand-in (Arial to Helvetica
  Neue, Avenir to Avenir Next), else the bundled font on screen and Helvetica on paper. Preferences > Fonts switches
  all this off or on (chapter 1.9a).
- The styles are stored with the plan, and a layer's Text Style property refers to one by name. The Text
  Specification's Appearance tab holds the text box settings (5.8).
