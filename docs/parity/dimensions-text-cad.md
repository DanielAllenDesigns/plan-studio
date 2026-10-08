# Parity spec: dimensions, text, CAD drawing, layers

Reference: Chief Architect X18 dimension, text and CAD tools, layers and layer sets, Reference Display. Source:
Chief's Reference Manual and documented behavior, plus the tool names and hotkeys captured in
`docs/chief-x18-subtools.md` and the toolbars in `docs/chief-x18-toolbars.md`. **(verify in Chief)** marks
recalled-but-unconfirmed detail. Ids: `DIM-n`, `TXT-n`, `CAD-n`, `LAY-n`.

## 0. Plan Studio today (code snapshot)

Dimensions, text and CAD are fully in the editor (`tools/dimension.rs`, `text.rs`, `cad.rs`, with `dialogs/{dimension,text,cad}.rs`). Dimension tools: manual, end to end, interior, point to point, running, baseline, centerline, angular, tape measure, plus Auto Exterior and Interior; text tools: text, rich text, leader line, arrow line, callout, marker, note; CAD: points, lines, polylines, arcs, circles, boxes, polygons, splines, revision clouds and CAD blocks. All draw on their layers, respect layer display and lock, select, move, open their specification dialogs and undo. The layer set, layer sets and saved plan views are live in the Active Layer Display Options dock; Default Settings > Dimensions opens the saved dimension defaults (Primary Format, Setup Automatic, Extensions, Arrow, Text Style) and Default Settings > Text Styles edits the text styles. Reference Display, Line Weights and Color toggles draw. Still open: associative dimensions, printed-size text and extension offsets, Fillet and Chamfer, Find/Replace Text and Replace Fonts, Auto Elevation and Story Pole dimensions. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## 1. Dimension tools: general rules

- DIM-1: Dimensions are plan objects on a dimension layer (`Dimensions, Manual` for manual, `Dimensions, Automatic` for automatic in Plan Studio; Chief's equivalents are in the layer set) with a primary format, extension lines, a dimension line, end marks (ticks/arrows) and text.
- DIM-2: A dimension is built from an ordered list of **measured points** (2 or more) along one **measuring direction**; the dimension line sits at a signed offset from the measured points; each pair of adjacent measured points yields one segment with its own value text.
- DIM-3: Dimensions are **associative**: a measured point located on a wall, opening or cabinet stays tied to that object. Moving the object updates the dimension; deleting the object leaves the point at its last position (verify in Chief for deletion).
- DIM-4: What is measured is set by Dimension Defaults > Locate: for walls the surface (Outer Surface, Main Layer Outside, Wall Center, Main Layer Inside, Inner Surface; default dimension to Main Layer Outside for exteriors and Wall Center for interiors, verify in Chief); for doors and windows (Opening edges, Center, Casing edge, Rough Opening); for cabinets, fixtures, columns (edges or center); for rooms (finished surfaces).
- DIM-5: "No Locate" walls (Wall Specification option) are skipped by the locate logic.
- DIM-6: The Dimension Defaults dialog controls: Primary Format (units: feet and inches, decimal feet, inches, millimetres, etc.; smallest fraction 1/16 or 1/8 or 1/32; Show Unit Indicators; Round Off Dimension Values), Secondary format, Text Style/Size, Arrows (tick, slash, arrow, dot), Extension Line settings (offset from object about 1/16", extend past dimension line about 1/8" to 1/4", Line weight), Dimension Line weight, Locate settings, Text position (above/centered/inline), Auto Dimension strings (Exterior).
- DIM-7: Text sizes and extension offsets are specified in **printed** units (paper inches at the view's print scale), so a 1/8" text is 1/8" tall on paper at any plan scale; extension line offsets scale with the plan scale likewise (verify in Chief).
- DIM-8: Value text is the measured distance formatted by the primary format, e.g. `12'-6 1/2"` (Plan Studio already matches this format in `DimFormat::fmt_len`); text is placed above the dimension line, centered between extension lines, rotated parallel to the line and readable (flipped so it never reads upside-down).
- DIM-9: If the text does not fit between extension lines, it moves outside the extension lines with a short leader and the end marks flip to outside arrows.
- DIM-10: A dimension segment's value can be overridden with custom text (Dimension Specification > Text); overrides show in a different editing color and survive moves (matches `Dimension.text_override`).

## 2. Manual-family tools

- DIM-11: **Manual Dimension** (Ctrl+Opt+Cmd+A): click the first point (object surface, edge, endpoint or free point with snaps), click the second point, then move the pointer perpendicular to set the dimension line offset and click to place. Further clicks before leaving the tool continue the same string with additional measured points (verify in Chief). The measuring direction (horizontal, vertical or aligned with the two points) follows the placement click's position relative to the two points (verify in Chief).
- DIM-12: Object highlight and snap markers preview where each click will locate (wall main-layer-outside, door edge, cabinet edge, center) and the status bar names the located feature.
- DIM-13: **End to End Dimension** (`D, E`): click a wall or CAD line and the tool measures its full end-to-end length along its reference line, with extension lines at the ends; clicking more segments extends a collinear string; sets the dimension line offset by a final click.
- DIM-14: **Interior Dimension** (`D, I`): click inside a room (or on an interior surface); the tool dimensions the clear distance between opposing interior wall surfaces along the horizontal or vertical axis nearest the pointer's direction, ignoring wall thickness (inner surface to inner surface). A second click places the dimension line.
- DIM-15: **Point to Point Dimension**: click two arbitrary points, the dimension is measured along the straight line between them (aligned, not horizontal/vertical), value is the Euclidean distance; the dimension line is parallel to the line joining the points at the placement offset.
- DIM-16: **Running Dimension**: one dimension line with extension lines at each clicked point; each tick labels the cumulative distance from the first (zero) point (verify in Chief).
- DIM-17: **Baseline Dimension**: first click sets the baseline origin; each subsequent click adds a dimension from the origin to that point, stacked outward at a fixed row spacing so lines do not overlap (verify in Chief).
- DIM-18: **Angular Dimension**: click two walls/lines (or three points: vertex, arm 1, arm 2), then move to set the arc radius and click. Text is the angle in degrees with the format's angle precision (0.1 degree default).
- DIM-19: **Centerline Dimension**: dimension locates to the center of clicked doors, windows, columns, cabinets or other objects (a Locate override that forces center) (verify in Chief).
- DIM-20: **Tape Measure** (`D, T, M`): click two points to show a temporary measurement (distance, deltas X/Y, angle) in the status bar and as a temporary dimension; nothing is added to the plan; Esc clears it.
- DIM-21: All dimension tools honor Object Snaps and Angle Snaps (select-and-edit S-68..S-71). Alt suspends snaps.
- DIM-22: Esc cancels the dimension in progress; double-click or choosing Select Objects ends a string.
- DIM-23: Each completed dimension is one undo step.

## 3. Automatic dimensions

- DIM-24: **Auto Exterior Dimensions** (Shift+A): requires a closed exterior; the tool runs on the active floor, building a set of dimension strings around the outside. Defaults produce, from outside inwards: an overall dimension (outer surface to outer surface), a wall-to-wall string at wall intersections, and an openings string (door/window centers or edges, per Locate), each offset by the dimension string spacing (24" typical) (verify in Chief). Plan Studio emits overall + one breakpoint string only.
- DIM-25: Re-running Auto Exterior Dimensions replaces the previous automatic dimensions (those on the automatic layer that have not been edited) and keeps manual ones; edited automatic dimensions become manual.
- DIM-26: Exterior dimension positions use Dimension Defaults > Locate for the exterior wall surface; dimensions to walls with the No Locate option are skipped.
- DIM-27: **Auto Interior Dimensions**: placed inside each room wall to wall (listed in `chief-x18-toolbars.md` but absent from the hotkey capture in `chief-x18-subtools.md`; verify in Chief for X18).
- DIM-28: **Auto Elevation Dimensions / Auto Story Pole Dimensions** apply only in elevation/cross-section views (out of scope for plan).
- DIM-29: Auto exterior dimensions are linked to the walls; moving a wall updates them until they are edited.

## 4. Selecting and editing dimensions

- DIM-30: Click a dimension line, extension line or its text to select it. A selected dimension shows: a **Move** handle on the dimension line (drag moves the whole dimension line perpendicular, extension lines stretch to follow), a handle at each extension line end (drag to relocate that measured point; the extension point snaps to objects and becomes associated with the new target), a **text** handle (drag text along the line or off it, with a leader), and Add/Delete extension handles (click on the line to add a measured point; selecting an extension line and pressing Delete removes it and merges segments).
- DIM-31: Double-click a dimension opens Dimension Specification: Dimension Line (offset, weight, style, extension line offset), Text (style, size, position, override), Primary and Secondary format, Locate, Layer, and a read-only list of measured points.
- DIM-32: Dimension value editing: with a dimension segment selected, clicking its text makes it editable; entering a value moves the object(s) at the dimension's **moveable** end so the segment equals the new value: the end that is farther from the dimension's origin (the start of the click sequence) moves; if that end is tied to a wall, the wall moves perpendicular to its length and connected walls stretch (select-and-edit S-21); the referenced end stays fixed (verify in Chief for which end moves).
- DIM-33: The same rule applies to temporary dimensions (select-and-edit S-59..S-61). Typing a dimension on an **automatic** dimension moves the object and converts the dimension to manual (verify in Chief).
- DIM-34: A dimension whose value edit would move objects on locked layers is refused with a message.
- DIM-35: Moving the object that a dimension measures does not move the dimension line if "Dimension line stays" is the default (line stays in place; value changes); with the Move handle you move the line.
- DIM-36: Selecting multiple dimensions allows Align/Distribute of dimension lines (align lines at a common offset).
- DIM-37: Reverse Dimension (flips the side of the dimension line) is available from the Edit toolbar (verify in Chief).
- DIM-38: Dimensions are included in Copy/Paste with the objects they reference when all referenced objects are copied; otherwise they paste as free dimensions (verify in Chief).

## 5. Text, Rich Text and annotation tools

- TXT-1: **Text** (`Y`): click to place the text anchor; a text entry field/dialog opens; typing and clicking outside (or OK) commits. Click-drag instead defines a text box with a fixed wrap width. Esc cancels.
- TXT-2: Text style comes from Text Defaults (Text Style: font, size, bold/italic/underline, alignment, color, background fill, border, angle). The size is a **printed** size (e.g. 1/8" cap height at paper) and scales with the plan scale (verify in Chief). Plan Studio's `CadItem::Text.height` is plan inches: needs a scale conversion.
- TXT-3: Selected text shows: Move handle (corner/anchor), a width Resize handle at the right edge (changes wrap width, reflows text), a Rotate handle; double-click opens Text Specification (Text, Font, Size, Alignment, Border, Layer, Angle, Background Fill).
- TXT-4: **Rich Text** (Ctrl+Opt+Cmd+J): a box with a rich-text editor (fonts, sizes, bold/italic/underline, bullets, colors, tables, images) that resizes with corner handles; the text wraps to the box; double-click edits inline.
- TXT-5: **Leader Line** (Opt+L): click the arrow tip, then click subsequent bend points, double-click to end; an arrowhead is drawn at the first point; used standalone to point at things. The line can be segmented or curved (spline leader) via the Edit toolbar (verify in Chief).
- TXT-6: **Text Line with Arrow** (Opt+A): like Leader Line but ends with a text anchor; after the last click the text dialog opens and the text attaches to the end of the line; moving text moves the leader end; dragging the arrow tip keeps text.
- TXT-7: **Callout** (Ctrl+Opt+Cmd+K): a framed text box (border/fill) with a leader to an object. Click the target, click the callout position, type text; the callout's border and leader style come from Callout defaults.
- TXT-8: **Marker** (Ctrl+Opt+Cmd+M): places a graphic marker (circle/bubble with text or number, such as detail/section/elevation reference) at a point; markers can link to views or details (verify in Chief).
- TXT-9: **Note** (Ctrl+Opt+Cmd+N): places a Note object chosen from Note Types (Note Type Management); the note text and number come from the note type; leader optional; notes appear in the Note Schedule; text macros (Text Macro Management) fill fields such as `%automatic_description%`.
- TXT-10: Text snaps: the anchor snaps with Object Snaps and Grid Snaps; text can be aligned with the left edge of nearby text (smart alignment) (verify in Chief).
- TXT-11: Text objects live on the active CAD or text layer (default `Text` in Plan Studio; Chief default text layer set by the Text defaults). Layers locked or hidden hide/disable editing.
- TXT-12: Find/Replace Text (Edit menu) edits text across the plan; Replace Fonts replaces font families.
- TXT-13: Text boxes auto-grow with content in height; the Resize handle changes width only (S-25).
- TXT-14: Rotation snaps to angle snaps (15 degrees) and to the angle of the object under the pointer (a wall) so text can be rotated parallel to a wall.
- TXT-15: Text and leader moves are one undo step; entering text then clicking away is one step including the placement.

## 6. CAD drawing tools

- CAD-1: CAD tools draw on the **current CAD layer** (Current CAD Layer button on the toolbar, CAD menu). New objects take the layer's line style/weight/color unless overridden in their specification. Default current layer in Plan Studio: `CAD, Default`.
- CAD-2: **Place Point / Input Point / Point Marker**: points are snap anchors and markers; Place Point drops a marker symbol at the click; Input Point opens a dialog for X, Y; Delete Temporary Points clears.
- CAD-3: **Draw Line** (Line tools): click start, click end; the tool continues from the end (chain) when "Connect CAD Segments" is on (right bar toggle); double-click or Esc ends. A line is its own object unless connected (CAD-4).
- CAD-4: Connect CAD Segments (toggle): consecutive lines/arcs sharing endpoints behave as a single path: moving a shared vertex moves both, and Convert to Polyline merges them.
- CAD-5: **Input Line**: after the first click a dialog asks for Length and Angle (and line style); OK places the end at that polar offset; the tool repeats from the new end.
- CAD-6: **Line With Arrow**: line with an arrowhead at its end; arrow backoff points (CAD Block tools) trim the line to the arrowhead.
- CAD-7: **Draw Arc** uses the current **Arc Creation Mode** (Edit > Arc Creation Modes): Three-point (start, end, then a point the arc passes through), Center-Start-End (center, start, sweep to end), Start-End-Radius, and Tangent (continues tangent from the previous segment) (verify in Chief for exact mode list and order).
- CAD-8: **Input Arc**: dialog entering radius and sweep angle (and start angle) before placement (verify in Chief).
- CAD-9: Arc editing: Move (midpoint), Resize at both ends (changes sweep with radius kept, or chord with radius changing per modifier), center/radius handle at the arc midpoint (drag to change radius).
- CAD-10: **Arc Centers and Ends** (right-bar toggle) shows the center marker and end points of arcs and circles for snapping.
- CAD-11: **Circle** (`K`): click center, drag or click for radius; a circle is a single object with center and radius. **Circle About Center** draws from center; **Ellipse** takes center and two radii; **Oval** takes a bounding box (verify in Chief for exactly which of these takes a diameter).
- CAD-12: **Rectangular Polyline** (Shift+P): click opposite corners; creates a closed 4-point polyline with fill option. **Box**: the same but as a Box primitive; **Cross Box** draws diagonals; **Blocking Box**, **Insulation** draw patterned boxes; **Regular Polygon** takes center, radius and side count.
- CAD-13: Fill and line style: closed polylines/boxes/circles can carry a fill style (pattern/solid/color) and each item has line style (solid, dashed), weight and color; defaults come from CAD Default Settings.
- CAD-14: All CAD tools use Object Snaps, Angle Snaps and Grid Snaps (select-and-edit S-68..S-72). Holding Shift constrains line direction to 0/45/90 (verify in Chief; Chief may rely on Angle Snaps only).

## 7. Polylines and splines (CAD-15 to CAD-30 editing rules)

- CAD-15: **Polyline** (Boxes group, also Wall Hatching and Material Region use the same): click vertices; double-click or Esc ends; clicking on the first vertex closes it. Right-click opens the end menu (verify in Chief).
- CAD-16: A polyline with N vertices shows N vertex handles plus a Move handle; closed polylines show no free ends.
- CAD-20: Dragging a vertex handle moves only that vertex; with Edit Behavior Fillet, clicking two segments rounds the corner with the prompted radius; with Chamfer it cuts the corner.
- CAD-21: **Add Break** inserts a new vertex at the clicked point on a segment (no geometry change); **Delete Break** removes a vertex; **Break Line** splits an open polyline into two at the click.
- CAD-22: **Change Line/Arc** toggles a segment between straight and arc; dragging the arc handle sets the bulge (polyline segments can be arcs).
- CAD-23: **Convert to Polyline** converts selected lines/arcs/connected segments (and walls' centerlines) into one polyline; **Convert to Spline** and **Convert Polyline to Lines** are the inverses on the Edit toolbar (verify in Chief).
- CAD-24: **Make Arc Tangent** adjusts an arc to be tangent to the adjacent segment at the shared end.
- CAD-25: Offset/Concentric behavior (Edit Behavior Concentric): dragging a polyline's move handle with Concentric creates an offset copy at the dragged distance (verify in Chief).
- CAD-26: Closing a polyline sets `closed = true`; the area can be queried and fill applied; Plan Studio's `CadItem::Polyline { points, closed }` matches this but has no arc segments (gap).
- CAD-27: Self-intersecting polylines are allowed; fill uses non-zero or even-odd per fill style (verify in Chief).
- CAD-28: Polylines can be turned into walls (CAD to Walls), slabs (Slab from polyline), roof planes, rooms (custom), and 3D solids; their Specification has an "Object" type conversion section.
- CAD-29: **Spline**: click control/fit points; double-click ends; clicking the first point closes the spline. Each point has a handle; dragging reshapes smoothly; Edit toolbar offers Add/Delete Break, Change Tension, Convert to Polyline (verify in Chief).
- CAD-30: Splines are rendered as smooth curves; dimension snaps use fit points and midpoints.

## 8. CAD blocks and details

- CAD-31: **Make CAD Block**: select CAD objects; the command asks for a name, the insertion point (click) and optional symbol categories; the objects become one block in the CAD Block library (Library Browser) and in the plan as a block instance.
- CAD-32: **Edit CAD Block** opens the block in a CAD detail window; changes update all instances; **Explode CAD Block** turns an instance back into individual CAD objects.
- CAD-33: Block instances have Move, Resize (uniform), Rotate handles and an insertion point marker; Add Insertion Point and Add Arrow Backoff Point are tools to adjust a block's anchors.
- CAD-34: CAD Block Management dialog (`V`): list, rename, delete, export blocks.
- CAD-35: **Auto Detail** (toolbar) creates a 2D detail view from a selected model portion; **CAD Detail From View** creates a CAD detail window; **CAD Detail Management** lists details.
- CAD-36: Revision Cloud tool draws a cloud-shaped polyline by clicking points; arc size set in defaults.
- CAD-37: Layers of CAD block contents are preserved inside the block; instances honor the layer display of the block's own layers.

## 9. Layers, layer sets and display

- LAY-1: Every object belongs to exactly one **layer**; layers hold display, lock, color, line style, line weight and plot behaviors; objects can use the layer's values (ByLayer) or their own.
- LAY-2: A **Layer Set** is a named collection of layer display/lock states; each saved plan view carries a layer set; changing the view changes visible layers (captured: saved views each carry their own layer set).
- LAY-3: **Active Layer Display Options** (dock, View menu toggle, right-bar button) shows a table with columns Display (eye), Lock, Layer name, Color, Line weight; a Layer Set dropdown at the top; New/Delete/Rename; Select by layer; changes apply immediately to the view.
- LAY-4: Turning a layer off hides its objects (not selectable, not snapped to), but they still exist and still participate in the model (e.g. wall hidden but still defines a room, DW-93/W-93).
- LAY-5: Locked layers: objects are visible and snappable but cannot be selected for edit (select-and-edit S-5), moved or deleted.
- LAY-6: The **Current CAD Layer** (toolbar button, CAD menu) chooses where new CAD objects go. Toolbar shows the layer name. Placing walls, doors, etc. always uses their default layers, not the current CAD layer.
- LAY-7: Layer default colors/weights drive plan printing; the Line Weights toggle (right bar) switches between weights and uniform thin lines.
- LAY-8: Layer sets can be created from the current display (Save Layer Set As) and applied to other views.
- LAY-9: **Reference Display** shows the floor below (and optionally above, or any floor) as gray ghosted linework behind the active floor, using layers marked for reference. Reference items are not selectable and not snappable by default (Object Snaps may snap to reference objects when Snap To Reference is on, verify in Chief).
- LAY-10: Reference Display options (Tools > Floor/Reference Display): choose which floors; "Show only walls/layers selected for reference"; color of ghost linework; per-layer Reference checkbox in Layer Display Options.
- LAY-11: Floor Plan layer set defaults seen in the capture: wall layer `Walls, Normal` (Default checkbox in wall Layer tab); label layers; the Plan Studio default set (`default_floor_plan`) names 16 layers.
- LAY-12: Layer changes (display, lock, color) are view settings and are undoable in the same stack (verify in Chief).
- LAY-13: Per-object "Layer" command (Edit toolbar) moves selected objects to another layer; objects on locked layers cannot be moved.

## 10. Dialogs and fields (reference for specification dialogs)

- DIM-39: Dimension Specification tabs (Chief): General (Dimension Line offset, Extension Line offset/extend, Line weight, Dimension Arrows), Text (Text Style, Size, Position relative to line, Rotation), Format (Primary and Secondary: units, fractions, Round Off, Show unit indicators, Tolerance), Locate, Layer, Object Information. Plan Studio's Default Settings > Dimensions entry currently shows "Coming in a later phase".
- DIM-40: Dimension Defaults has separate Manual, Automatic, Temporary and Elevation groups; Temporary Dimensions sub-settings choose which references temp dimensions locate to (walls, openings, cabinets) and whether they show for each selected type (verify in Chief).
- TXT-16: Text Specification fields: Text (multi-line box with Insert macro), Font, Size, Bold/Italic/Underline, Alignment (left/center/right), Text Angle, Border (on/off, weight), Background Fill (none/color), Lock Position, Layer, Label Layer.
- TXT-17: Text Style Management (Define...) defines named text styles; "Use Layer Text Style" uses the object's layer style (seen in the wall Label tab capture). Style changes update all text using that style.
- CAD-38: Line Specification: Start X/Y, End X/Y (or Length/Angle), Line Style, Line Weight, Color, Arrow at Start/End, Layer. Arc Specification: Center, Radius, Start Angle, End Angle, Sweep, Line Style. Circle Specification: Center, Radius/Diameter, Line Style, Fill Style. Polyline Specification: vertices, Closed, Fill, Line Style, "Convert to" (wall, slab, room).
- CAD-39: Input Line dialog fields: Length (feet-inches parse), Angle (degrees, positive counter-clockwise from +X), Line style; Enter accepts, Tab moves between fields; after OK the new line's end becomes the next start (verify in Chief).
- CAD-40: Snap to CAD geometry uses all Object Snap types: line endpoint/midpoint/intersection/perpendicular/tangent (to circle/arc)/center/quadrant (circle/arc)/On Object/Extension.

## 11. Edit toolbar buttons for dimension, text and CAD objects (see select-and-edit S-39..S-55)

- DIM-41: Dimension: Open Object, Copy, Delete, Add Extension Line, Delete Extension Line, Reverse Dimension, Align/Distribute, Convert to Manual (automatic), Layer.
- TXT-18: Text: Open Object, Copy, Delete, Rotate, Transform/Replicate Object, Convert to Rich Text, Layer.
- CAD-41: Line/Arc/Polyline: Open Object, Copy, Delete, Transform/Replicate Object, Reflect About Object, Make Parallel/Perpendicular, Break Line, Add Break, Delete Break, Change Line/Arc, Convert to Polyline, Convert to Spline, Make Arc Tangent, Fillet/Chamfer via Edit Behaviors, Layer.
- CAD-42: Circle/Ellipse: Open Object, Copy, Delete, Transform/Replicate, Reflect, Convert to Polyline, Layer.
- CAD-43: Edit toolbar commands operate on the selection and are one undo step each.

## 12. Acceptance scenarios for builders

- DIM-42: Draw a 20' x 12' exterior rectangle; run Auto Exterior Dimensions: bottom and top show overall 20'-0" plus a wall-to-wall string; left and right show 12'-0"; text is horizontal on horizontal lines and rotated 90 degrees (reading bottom to top) on vertical lines; moving the north wall by 1' updates the top and side dimensions.
- DIM-43: Draw a Manual Dimension between two interior wall faces 10'-0" apart, select it, type 10'-6" in the text: the wall tied to the second point moves perpendicular by 6", rooms update, one undo step restores everything.
- DIM-44: Interior Dimension click in a 12' x 14' room with 4 1/2" interior walls and 6 1/2" exterior walls: dimension reads clear surface-to-surface (e.g. 11'-5 1/2") not centerline.
- DIM-45: Angular Dimension between a wall at 0 degrees and a wall at 90 degrees reads 90.0 degrees; at 135 degrees between walls reads 135.0 degrees (use interior angle by click side).
- TXT-19: Place text "KITCHEN" at 1/8" printed height at 1/4"=1'-0" scale: the text is 6" tall in plan inches; at 1/8"=1'-0" scale it is 12" tall; rotating by dragging the handle snaps to 15 degrees and to nearby wall angles.
- CAD-44: Draw a line with Input Line: Length 12', Angle 30: end point = start + (12' cos 30, 12' sin 30) = (125.73", 72") from a (0,0) start (in inches); the line's temporary dimension shows 12'-0" and 30.0 degrees.
- CAD-45: Draw a closed polyline, add a vertex with Add Break, drag the vertex, convert to walls with CAD to Walls: a loop of connected walls appears with joined corners.
- LAY-14: Turn off the `Doors` layer: door symbols disappear and cannot be picked; turn on and lock `Text`: text remains visible but cannot be dragged; Undo does not change layer display.
- LAY-15: With Reference Display on and the active floor 2, floor 1's walls show in gray under floor 2's plan and cannot be selected.

## 13. Gap list

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| DIM-1..DIM-10 render dimensions with extension lines, ticks, text | Model + geometry in core; nothing drawn; no Dimension objects in `draw()` | Critical | New `draw_dimensions()` iterating `floor.dimensions` using `line_points()`, `extension_lines()`, `label(&fmt)`; add tick/arrow style, text rotation/flip rules |
| DIM-11..DIM-20 manual-family tools | All `NotImplemented` | Critical (Manual, End to End, Interior, Point to Point); High (Angular, Baseline, Running, Centerline); Low (Tape) | `Tool::Dimension(kind)` with click state machine; locate logic in core (`locate.rs`: nearest wall surface/opening edge/center from point + settings) |
| DIM-4 Locate settings | Auto exterior uses centerlines only | High | `LocateSettings` in `Project`; `wall_surface_point(wall, side)`; update `auto_exterior_dimensions` |
| DIM-3 associative dimensions | Dimensions store raw points | High | Store `Anchor { object: Id, feature }` per measured point; recompute on wall edits |
| DIM-24..DIM-26 Auto Exterior with 3 strings | 1 overall + 1 breakpoint string; axis-aligned sides only; not wired | High | Wire Shift+A and toolbar to `auto_exterior_dimensions`; add openings string and wall-to-wall string; replace previous automatic dims (DIM-25) |
| DIM-27 Auto Interior | None | Low | After room face polygons exist |
| DIM-30..DIM-33 dimension edit handles, edit value to move objects | None | Critical | Handles via the shared handle system; value edit calls `move_wall_perpendicular`/opening move |
| DIM-6/DIM-7 Dimension defaults, printed-size text | `DimFormat` only (fraction, indicators) | Medium | Add `DimensionDefaults` struct and dialog (Default Settings > Dimension is already a stub entry) |
| DIM-8/DIM-9 text flip and outside placement | N/A | Medium | Implement in `draw_dimensions` |
| TXT-1..TXT-15 text/rich text/leader/callout/marker/note tools | `CadItem::Text` model only | High (Text); Medium (Leader, Text Line with Arrow, Callout); Low (Rich Text, Marker, Note) | Text tool + `Text` spec dialog first; leader as a `CadItem::Leader { points, text }`; Notes need note-type library |
| TXT-2 printed text size scaled by plan scale | `height` is plan inches | Medium | Add `print_height` and view scale to the model; convert at draw time |
| CAD-1..CAD-14 line/arc/circle/box/polyline tools and Input dialogs | `CadItem` model + bounds only; no tools, no draw | Critical | Add draw code for each `CadItem`; tools: Line, Circle, Rect Polyline, Polyline, Arc (3-pt first); Input Line dialog |
| CAD-4 Connect CAD Segments | Flag exists, no behavior | Medium | Store `connected` links or merge on end |
| CAD-7/CAD-8 arc creation modes | Arc as center/radius/angles; no modes | Medium | Add arc constructors from 3 points / center-start-end in core |
| CAD-15..CAD-30 polyline editing (vertices, breaks, arc segments, fillet/chamfer, convert) | `Polyline{points, closed}` without arc segments | High | Extend to `Vec<Vertex { p, bulge }>` (bulge arcs) and write edit ops in core |
| CAD-29..CAD-30 spline | None | Low | Add `CadItem::Spline` with Catmull-Rom/B-spline; polyline-based first |
| CAD-31..CAD-35 CAD blocks, details | None | Low (defer) | `CadBlock { name, items, insertion }` library after tools exist |
| LAY-1..LAY-8 layers consulted by the app; Layer Display Options panel | Layers exist in core, ignored in UI; dock says "Coming in Phase 1" | High | Panel with eye/lock per layer; `draw()` filters by `is_visible`; hit tests honor lock/visibility; save layer sets per view |
| LAY-6 current CAD layer | Not selectable; default string `CAD, Default` | Medium | `PlanApp.current_cad_layer` and toolbar button/dialog |
| LAY-9..LAY-10 Reference Display | Flag only | Medium | When on, draw floor below's walls in gray under the active floor; not pickable |
| LAY-12 layer changes undoable | N/A (no undo yet) | Low | Part of history wiring |
| DIM-39..DIM-41 dimension specification dialog, Edit toolbar entries | None | Medium | Dialog in `dialogs/dimension.rs` reusing `SpecDialog`; Edit toolbar entries via the shared contextual toolbar |
| TXT-16..TXT-17 Text Specification and named text styles | None | Medium | `TextStyle` list in `Project`; dialog; `Use Layer Text Style` option |
| CAD-38..CAD-40 CAD specification dialogs and snaps to CAD geometry | None | Medium | Dialogs per type; extend `snap()` with CAD candidates via `CadItem` geometry helpers (extend `cad.rs`) |
| DIM-42..LAY-15 acceptance scenarios | Not runnable | n/a | Turn into `plan-core` tests as each feature lands |
