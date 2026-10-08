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

Points locate to wall surfaces (the main layer faces), opening centers or the snap
point. Hold `Alt` to suspend the locating. Every completed dimension is one undo
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
| Auto Exterior Dimensions | `Shift+A` | Click once: puts an overall dimension on each axis-aligned side of the exterior walls plus a breakpoint string, set out by the Dimension Defaults offset (32"). Re-running replaces the previous exterior strings. Door and window strings are (planned). |
| Auto Interior Dimensions | | Click once: adds one horizontal and one vertical clear dimension (interior surface to interior surface) through every room larger than 10 sq ft. Re-running replaces them. |
| Auto Elevation Dimensions | `Ctrl+Alt+Cmd+H` | Click once where the string should go: a vertical string of level heights (floor platforms, ceiling heights, heights above the first floor) with each level named beside it. |
| Auto Story Pole Dimensions | `Ctrl+Alt+Cmd+I` | Click once: a story pole, the same kind of vertical string of level heights. |

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

The model keeps no link between a dimension and the objects it measured. If you move a
wall afterwards the dimension line stays where it is (planned: associative
dimensions).

### Temporary dimensions

Temporary Dimensions (view bar, on by default) show a live readout while drawing
and dimensions to nearby objects for the selected one. They are never saved or
printed. Clicking one turns it into an edit field (see chapter 2.5).

## 5.3 Text tools

All text and annotation tools are in the Text flyout (row 2; CAD > Text). Every
annotation is a CAD item on the `Text` layer: text is a text item; leaders and
callout frames are polylines and circles grouped with their text so they select
together.

| Variant | Hotkey | How it works |
|---|---|---|
| Text | `Y` | Click to set the text's lower-left anchor, type, `Enter` to commit. A click elsewhere commits and starts the next text. `Esc` cancels. Clicking existing text edits it in place. |
| Rich Text | `Ctrl+Alt+Cmd+J` | Click, type; `Enter` adds a line; `Tab` finishes. Bold, italic, underline, size and color are stored with the text as runs (below). |
| Leader Line | `Alt+L` | Click the arrow tip and the bends; double-click or `Enter` ends. |
| Text Line with Arrow | `Alt+A` | As Leader Line, then asks for the text at the end. |
| Callout | `Ctrl+Alt+Cmd+K` | Click the target, click the callout position, type. A circle, hexagon or **square** frames the text (pick the shape in the option strip). |
| Marker | `Ctrl+Alt+Cmd+M` | Click to place the next numbered marker (a numbered circle). |
| Note | `Ctrl+Alt+Cmd+N` | Click, type; the text reads "Note n: ..." with the next free number of the active note type. |
| Note Type Management | | Opens the dialog below. |
| Text Macro Management | | Opens the dialog below. |

Text height defaults to the template's 6" plan height. Printed-size scaling (a 1/8"
text that stays 1/8" at any plan scale) is (planned). Find/Replace Text and
Replace Fonts are (planned).

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
| Lines | Draw Line | | Click start, click end, or press-drag-release. With Connect CAD Segments on (`Shift+F8`) the next line starts where the last ended. `Enter` after the first click types a length and angle. |
| | Input Line | | Click start, type length, `Tab`, angle, `Enter`. |
| | Line With Arrow | | Click the start, click the arrow tip. |
| | Polyline | | Click vertices, click the first to close, `Enter` or double-click ends. |
| Arcs | Draw Arc | | Click the points of the arc. Modes in the option strip: three-point, center-start-end, start-end-tangent. |
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
| Reverse Direction | Click a line or polyline to reverse it. |
| Make Parallel | Click the end of a line to turn, then the line to match. |
| Make Perpendicular | Click the end of a line to turn, then the line to square to. |
| Convert to Polyline | Select connected lines, then use the command. |
| Convert to Spline | Select polylines, then use the command. |
| Convert Polyline to Lines | Select polylines, then use the command. |
| Hatch (CAD > Patterns > Hatch Closed Shape) | Click inside a closed polyline or circle; pick the pattern in the option strip. The pattern is drawn as real lines grouped with the outline. The Fill Style tab of the CAD Specification does the same. |
| CAD Detail From View (CAD menu) | Copies the view's wall outlines, CAD items and dimensions into a new floor named "CAD Detail" above the current one and makes it current. |

CAD Detail From View makes a normal floor, which takes part in the 3D stack; Chief's detail windows and CAD Detail Management do not exist here. The Edit toolbar does not have buttons for these tools yet: use the CAD menu. Change Line/Arc and Make Arc Tangent are (planned).

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
Options...) shows a table with Name, Used (object count), Disp and Lock, plus Color.

- **Disp** off hides the layer: its objects are not drawn, picked or snapped to, but they
  still exist and still form rooms.
- **Lock** on makes objects visible and snappable but not selectable for editing; moving
  or deleting them is refused with a status message.
- **Properties for Selected Layer**: Display, Lock, Color, Line Weight, Line Style, Text Style
  and Fill Style.
- The window version adds New Layer... and Copy Layer Set... (Copy Layer Set is
  (planned)). All edits are undoable.
- **Layer sets and saved plan views.** A layer set is a named table of per-layer overrides (display, lock,
  color, line weight, line style, text style) laid over the plan's base layers. A saved plan view carries a
  layer set, a floor, a reference-display setting and a zoom. The **Layer Set** drop-down in the dock
  switches the active set; the saved-view selector in row 1 activates a view (its layer set, floor and zoom;
  one undo step). A new plan starts with "Default Set" and "Floor Plan View". Daniel's 34 layer sets
  (Presentation, Working, Electrical, Foundation, Roof Plan ...) come in through File > Templates > Import
  Chief Template.... Save Active View, Save Active View As, Edit Active View and creating views are (planned).
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
| General | Measured points (Start X/Y, End X/Y), dimension Type and Value. Offset of the line. |
| Primary Format | Units, Show Unit Indicators, Smallest Fraction, Show Denominator, Reduce Fractions. Shown from the active Dimension Defaults (disabled). |
| Arrow | Style (Tick) and Size (disabled). |
| Text Style | Font and Height, Position (Centered On / Above / Below Dimension Line) (disabled). |
| Layer | Manual or Automatic dimension layer. |
| Label | Value Text: type a replacement for the measured value. |

The Dimension Defaults sets (1/4" Scale, 1/8" Scale, Electrical, Framing ...) are kept in the plan
defaults and edited from Edit > Default Settings > Dimension > Dimensions (5.9). The set marked
Currently Active (the template's 1/4" Scale set in Daniel's: smallest fraction 1/8, diagonal
fractions) is what dimensions use.

## 5.8 Dialog: Text Specification

| Tab | Fields |
|---|---|
| Text | The text, with a *Rich text* check box that shows it as markup (`<b>`, `<i>`, `<u>`, `<size=1.5>`, 5.3), Angle, Position (lower left X and Y). |
| Text Style | **Style**: a named text style of the plan, or "(layer's style)"; the font it gives; **Format** check boxes Bold, Italic and Underline for the whole text (mixed formats are typed as markup on the Text tab). |
| Appearance | **Size**: Text Height. Alignment (Left, Center, Right), Border and Background Fill are disabled until the model stores them. |
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

**Dimension Defaults - <name>** is a five-tab editor with a preview that draws a sample dimension (12'-6 1/2")
in the set's format. A length field turns red when it does not parse and OK is blocked ("Fix the highlighted field").

| Tab | Fields |
|---|---|
| Primary Format | Units (Feet and Inches, Inches, Decimal Feet, Millimeters, Centimeters, Meters), Smallest Fraction (1/2 to 1/64), Fraction Style (Diagonal, Horizontal, Stacked), Decimal Places (0 to 6), Unit Indicators, Trailing Zeroes |
| Setup Automatic | Exterior Offset, Line Separation, Locate Openings at Centers |
| Extensions | Gap from Object, Extend Past Dimension Line |
| Arrow | Arrow Size, Leader Style (Square Corner, Round Corner, Diagonal) |
| Text Style | Text Above Dimension Line, Fraction Text Size (25 to 100 %) |

### Text Styles

Edit > Default Settings... > Text > **Text Styles** edits named text styles for two places, chosen by the
**Edit styles of** radios: **This plan** (the open plan's styles, one undo step, "Text Styles") and **New-plan
defaults** (what plans start from).

- The list is on the left. The form on the right has Name, Font (Arial, Helvetica, Times New Roman, Courier
  New, Verdana, Georgia, Calibri), Height (0.25" to 96"), Style (Bold, Italic, Underline), Color, and Size
  ("Follows the drawing scale"). **New** adds a style, **Copy** duplicates the selected one, **Delete** removes it.
- Names must be filled in and unique. "Default Text Style" can be neither renamed nor deleted.
- Renaming a style in the plan renames it on the layers that used it. OK applies both lists ("Saved the text
  styles"); Cancel or Escape drops the changes.
- The styles are stored with the plan, and a layer's Text Style property refers to one by name. The Text
  Specification's Appearance tab is still disabled (5.8).
