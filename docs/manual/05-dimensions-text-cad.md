# Chapter 5: Dimensions, Text and CAD

This chapter covers everything you draw on top of the model: dimensions,
text and annotation, 2D CAD primitives, and the layers they live on.

## 5.1 Choosing a variant

Chief puts each family of tools (Dimensions, Text, Lines, Arcs, Circles, Boxes) on a flyout
button. Picking a flyout entry starts the tool already in that variant, so the Dimensions
flyout's "Running Dimension" really starts a running dimension. The hotkey of a variant does the
same.

Inside the tool you can also switch variants with the **option strip**: a row of small buttons that
the Dimension, Text and CAD tools draw along the top edge of the canvas while they are active.
Click a button to change mode without leaving the tool. The strip reaches a few modes that have no
flyout entry of their own, for example Spline, Revision Cloud and the Regular Polygon sides.
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
| Auto Elevation Dimensions | `Ctrl+Alt+Cmd+H` | (planned) |
| Auto Story Pole Dimensions | `Ctrl+Alt+Cmd+I` | (planned) |

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
| Rich Text | `Ctrl+Alt+Cmd+J` | Click, type; `Enter` adds a line; `Tab` finishes. The size scale is stored; bold, italic and underline are (session only). |
| Leader Line | `Alt+L` | Click the arrow tip and the bends; double-click or `Enter` ends. |
| Text Line with Arrow | `Alt+A` | As Leader Line, then asks for the text at the end. |
| Callout | `Ctrl+Alt+Cmd+K` | Click the target, click the callout position, type. A circle or hexagon frames the text. |
| Marker | `Ctrl+Alt+Cmd+M` | Click to place the next numbered marker (a numbered circle). |
| Note | `Ctrl+Alt+Cmd+N` | Click, type; the text reads "Note n: ..." with the next free note number. |
| Note Type Management, Text Macro Management | | (planned) |

Text height defaults to the template's 6" plan height. Printed-size scaling (a 1/8"
text that stays 1/8" at any plan scale) is (planned). Find/Replace Text and
Replace Fonts are (planned).

## 5.4 CAD drawing tools

CAD objects are drawn on the **current CAD layer**, `CAD, Default`. The Current
CAD Layer button is (planned), so new CAD objects always land on that layer. Object
snaps, angle snaps and the grid apply to every CAD tool.

### Points, Lines, Arcs, Circles, Boxes

| Flyout | Variant | Hotkey | Gesture |
|---|---|---|---|
| Points | Place Point | | Click to drop a point. |
| | Input Point | | Type X, `Tab`, Y, `Enter`. |
| | Point Marker | | Click to drop a marked point. |
| | Delete Temporary Points | | (planned) |
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
| | Box, Cross Box, Blocking Box, Insulation | | (planned) |

Other CAD tools: `Backspace` drops the last vertex of a polyline, **Spline** and **Revision
Cloud** work like polylines. On the toolbar, the Spline and Revision Cloud buttons are
(planned) dimmed toggles, but both modes are reachable from the CAD option strip while a CAD
tool is active. An ellipse is stored as a 48-segment closed polyline.

### CAD Blocks

Make CAD Block groups the selected CAD objects so they select and move together; Explode
CAD Block ungroups them. Add Insertion Point, Add Arrow Backoff Point, Edit CAD Block and
CAD Block Management (`V`) are (planned).

### Editing CAD objects

With Select Objects a selected CAD object shows handles: line ends and midpoint, polyline
vertices with "add vertex" handles at edge midpoints, circle center and radius. Drag to
edit. Double-click opens the CAD Specification (5.6). Edit toolbar commands Break Line,
Change Line/Arc, Convert to Polyline, Make Arc Tangent, Fillet and Chamfer are (planned).

## 5.5 Layers and layer display

Every object belongs to one layer. Daniel's template starts with 16 layers (Walls,
Normal; Walls, Invisible; Doors; Windows; Rooms; Room Labels; Dimensions, Manual;
Dimensions, Automatic; Text; CAD, Default; Cabinets, Base; Cabinets, Wall; Electrical;
Stairs; Roof Planes; Framing), each with a color and line weight. Data layers such as
`Roof Planes, Data`, `Electrical, Data` and `Terrain, Data` are hidden and locked by the
program; they store roof, electrical and terrain records (see chapters 8 and 9).

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
- Line weights are stored (in hundredths of a millimeter) but the Line Weights toggle does
  not change the drawing yet (planned).

## 5.6 Dialog: CAD Line / Polyline Specification

Double-click a line, polyline, circle or arc. One dialog serves all of them.

| Tab | Fields |
|---|---|
| General | **Line**: Start X, Start Y, End X, End Y, Length, Angle. **Circle**: Center X/Y, Radius, Diameter. **Arc**: Center X/Y, Radius, Start Angle, End Angle, Sweep, Arc Length. **Polyline**: Vertices, Perimeter, Area, Vertex List, Closed. |
| Line Style | Shows the object's layer line weight and color (read-only) and Line Style options (Solid, Dashed, Dotted, Dash-Dot) which are disabled: the model keeps no per-object style. |
| Fill Style | Closed shapes only: No Fill, Solid, Pattern, Fill Is Transparent (disabled). |
| Layer | The layer, editable. |

## 5.7 Dialog: Dimension Specification

| Tab | Fields |
|---|---|
| General | Measured points (Start X/Y, End X/Y), dimension Type and Value. Offset of the line. |
| Primary Format | Units, Show Unit Indicators, Smallest Fraction, Show Denominator, Reduce Fractions. Shown from the active Dimension Defaults (disabled). |
| Arrow | Style (Tick) and Size (disabled). |
| Text Style | Font and Height, Position (Centered On / Above / Below Dimension Line) (disabled). |
| Layer | Manual or Automatic dimension layer. |
| Label | Value Text: type a replacement for the measured value. |

The Dimension Defaults sets (1/4" Scale, 1/8" Scale, Electrical, Framing ...) are
(planned); the template's 1/4" Scale set (smallest fraction 1/8, diagonal fractions) is
what every dimension uses. Edit > Default Settings > Dimension is listed but not
openable yet.

## 5.8 Dialog: Text Specification

| Tab | Fields |
|---|---|
| Text | The text, Angle, Position (lower left X and Y), Text Height. |
| Appearance | Font family, Bold, Italic, Underline, Alignment (Left, Center, Right), Border, Background Fill. All disabled until the model stores them. |
| Layer | The layer, editable. |
