# plan-app (Plan Studio)

A desktop 2D floor-plan editor built with egui/eframe on top of `plan-core`.
Draw walls, drop in doors and windows, and see rooms and areas update live.

Run it with `cargo run -p plan-app` (binary name: `plan-studio`).

Controls:
- Toolbars (Chief Architect style, dark theme): row 1 (file, view, floors, 3D and material tools), row 2 (Build tools) and a vertical view bar on the right edge. Hover any button for its name and hotkey. Flyout buttons (small arrow on the right) open a variant list; clicking the icon half activates the variant shown. Dimmed buttons are not implemented yet. The Library, Project and Layer Display buttons open a placeholder dock panel.
- Tools: see Hotkeys below. Esc cancels a wall in progress, otherwise returns to Select.
- View bar: Zoom In/Out, Undo Zoom, Fill Window, Pan Window (left-drag pans; Esc or Select returns), and display toggles. Crosshairs draws cursor crosshairs; Temporary Dimensions (on by default) shows the live length while drawing walls. The floor arrows in row 1 switch floors.
- Wall tool: click to place points (continuous chain), or press-drag-release to draw a single wall. Snaps to endpoints, midpoints, intersections, wall centerlines, the snap grid and 15 degree angles (marker and name in the status bar); hold Alt to skip angle snap. A wall started or ended on another wall splits it there (T-junction). Esc or right-click ends the chain.
- Door/Window: hover a wall to see the opening and its distances to both wall ends; click to place it centered on the pointer (snapped to 1"). New openings take their size from the Default Settings door/window templates.
- Select: click an object (Shift adds), Tab cycles objects under the pointer, drag empty space for a marquee (left-to-right encloses, right-to-left touches). Dragging a wall moves it perpendicular with its connected walls stretching (Alt: free move); the end handles stretch it and dropping an end on another wall splits that wall; dragging an opening slides it along the wall. Click a temporary dimension value, type a length and press Enter to move the object. Delete/Backspace removes the selection, Esc cancels a drag.
- Edit toolbar (bottom left of the canvas, when something is selected): Open Object, Delete, Copy, Paste in Place, Reverse Swing (doors), Fix Wall Connections (not built yet).
- Undo/Redo: Edit menu, the row-1 buttons, `Cmd+Z`, `Shift+Cmd+Z`, `Cmd+Y`; the menu names the step ("Undo Move Wall").
- Specification dialogs: double-click a wall (Wall Specification) or an opening (Door/Window Specification) with the Select tool, or use "Open Specification..." / the "..." button in the Properties panel. Canvas clicks and hotkeys are ignored while a dialog is open.
- View: scroll or pinch to zoom at the cursor; middle- or right-drag to pan.
- Files are `.psplan` JSON.

## Menus

The menu bar follows Chief Architect X18: File, Edit, Build, Terrain, Library, 3D, CAD, Tools, View, Window, Help (see `docs/chief-x18-menus.md`). The Build, Terrain and CAD submenus are generated from the same flyout tables as the toolbar (`toolbar.rs`, one function per group, lists from `docs/chief-x18-subtools.md`), so each entry shows its icon, name and hotkey. Working today: File New/Open/Save/Save As/Quit, Edit Select Objects, the View toggles (Library Browser, Project Browser, Active Layer Display Options, Color, Crosshairs, Reference Grid, Temporary Dimensions, Arc Centers and Ends, Line Weights, Drawing Sheet), and Window Zoom In/Out, Undo Zoom, Fill Window, Pan Window. Everything else is listed but disabled. Help > About Plan Studio shows the version and license.

## Hotkeys

Active whenever no text field has focus. The whole table lives in `toolbar::BINDINGS`.

| Key | Command |
|---|---|
| `Space` (alias `1`) | Select Objects |
| `Shift+Q` (alias `2`, the wall flyout's current pick) | Straight Exterior Wall |
| `Shift+W` (alias `4`) | Window |
| `D, H` (alias `3`) | Hinged Door |
| `Ctrl+F` | Fill Window |
| `H` | Pan Window |
| `F8` | Color flag |
| `Shift+F9` | Reference Grid flag |
| `Cmd+L` | Library Browser dock |
| `Cmd+N`, `Cmd+O`, `Cmd+S` | New, Open, Save |
| `Cmd+Z`, `Shift+Cmd+Z`, `Cmd+Y` | Undo, Redo, Redo |
| `Shift+Y`, `Shift+T`, `Shift+A`, `Q`, `Y`, `K`, `Shift+P` | Draw Stairs, Base Cabinet, Auto Exterior Dimensions, Roof Plane, Text, Circle, Rectangular Polyline |
| `Shift+J`, `Shift+K` | Full Camera, Perspective Full Overview |

`D, H` is a two-key sequence: after `D` the status bar shows `D, ...` for 1.5 seconds. Four-modifier Chief chords (such as Straight Interior Wall, Ctrl+Alt+Cmd+6) appear in tooltips and menus but are not bound.

## Themes and brightness

The canvas has four themes, chosen in View > Canvas Theme or the Theme box in the Properties panel: Low Glare (default, a warm mid-gray with no pure whites, for light-sensitive users), Paper, Dark and High Contrast. View > UI Brightness (0.6 to 1.0) dims the panels, text and toolbar icons without touching the OS display. Both settings are saved to `~/.plan-studio/settings.json`.

## Specification dialogs

`dialogs.rs` is the Chief-style dialog frame (spec: `docs/chief-x18-dialogs.md`): a 150 px vertical tab list, the active tab's panel, a 220 px preview drawn with the painter, and Help / Cancel / OK along the bottom. Section headings are a label plus a rule. Length fields are text boxes that show feet-inches and parse as you type (red and OK blocked when the text does not parse); angles are degrees. The dialog edits a cloned draft: OK (or Enter) writes it back and marks rooms dirty, Cancel or Escape discards it. Tabs and controls the model cannot store yet are listed but disabled.

- Wall Specification (`dialogs/wall.rs`): General (thickness, length, angle, lock Start/Center/End, options), Structure (default top height, wall height), Wall Types (the plan defaults' wall types, each with its layer stack drawn from the definition), Layer, Label. Changing the length moves the end point (Start lock), the start point (End lock) or both (Center lock); openings keep their place on the wall and OK is blocked if the wall becomes too short for them. The angle rotates about the start point; walls joined at the ends are not moved.
- Door and Window Specification (`dialogs/opening.rs`): General (style/type, width, height, floor to top/bottom, position along the wall with "Center on wall", clamped like `Project::add_opening`), Options (door swing hinge side, egress, tempered glass), Casing, Jamb or Frame, Lites, Label, with an elevation sketch and a plan sketch.
- Default Settings (Edit > Default Settings... or the toolbar button, `dialogs/defaults.rs`): a searchable tree. Exterior, Interior and Foundation Wall edit the three wall defaults (wall type and height; a custom thickness adds a `Custom-<n>` wall type); Interior Door, Exterior Door and Window edit the templates new openings are placed from; the other leaves say "Coming in a later phase".

Wall and opening settings the model has no fields for (invisible, no room definition, no locate, door style, window type, lites, casing, jamb, label options) live in memory for the session only; those controls say so in a tooltip.

## Templates and defaults

Plan Studio starts the way Chief Architect starts from a template plan. The defaults live in `plan_core::PlanDefaults` (`crates/plan-core/src/defaults.rs`): wall types and the exterior/interior/foundation wall defaults, interior and exterior door, window and cabinet defaults, the 1/4" scale dimension defaults, room types, the layer set, text, grid and units. `PlanDefaults::chief_x18_daniel()` holds the values captured from Daniel's Chief X18 template (`docs/chief-x18-dialogs.md`); it ships as `assets/templates/chief-x18-daniel.json`, embedded in the binary (a unit test keeps the file and the code identical).

- On startup the app loads `~/.plan-studio/defaults.json` if it exists, else the embedded template. A file that cannot be read is reported in the status bar and the template is used. Keys missing from the file fall back to the template's values.
- File > New Plan creates `Project::from_defaults`: the template's ceiling height and layer set.
- New walls take the exterior or interior wall type thickness and height. New doors take the interior door defaults, or the exterior door defaults when the host wall is an exterior wall; new windows take the window defaults. The Properties panel, the Default Settings dialog (Edit > Default Settings...) and the Wall Types tab edit these same values.
- Grid spacing, snap and the angle snap come from the defaults. Dimension text (the live wall length, status bar coordinates, the Properties panel) is rounded to the dimension defaults' smallest fraction (1/8" in the Chief template).
- File > Templates > Save Current Defaults as My Template... writes the current defaults to `~/.plan-studio/defaults.json`. File > Templates > Reset to Chief X18 Template restores the embedded values and removes that file.

Not stored in the defaults yet: muntin width, label options and the door's second casing (an exterior door keeps the exterior casing values, an interior door the interior ones); the sash width is kept as read from the template.


## Architecture

Tools live in `src/tools/` (one file each, behind the `Tool` trait) and share
the services in `src/editor/` through an `EditorContext`; `main.rs` is the
window, panels and dialogs. See `../../docs/architecture-tools.md`.

### Tools

Every flyout entry selects its exact sub-tool through a `ToolId` payload
(`StairsVariant`, `RoofVariant`, `CabinetVariant`, `DimensionVariant`,
`TextVariant`, `CadVariant`, `CameraVariant`, `ElectricalVariant`,
`TerrainVariant`); `Tool::set_variant` receives it. Tools with dialogs or
palettes also get a per-frame `Tool::frame` hook.

| Tool | File | Variants (flyout entries) |
|---|---|---|
| Select Objects | `select.rs` | picks, moves and edits every object kind (walls, openings, dimensions, CAD/text, cabinets, symbols, stairs, roof planes, devices, cameras, rooms, terrain) |
| Walls, Doors, Windows | `wall.rs`, `opening.rs` | wall flavors, hinged door, window |
| Dimensions | `dimension.rs` | Manual, End to End, Interior, Point to Point, Running, Baseline, Centerline, Angular, Tape Measure, Auto Exterior/Interior |
| Text | `text.rs` | Text, Rich Text, Leader Line, Text Line with Arrow, Callout, Marker, Note |
| CAD | `cad.rs` | points, lines, polyline, arcs, circles, ellipse/oval, boxes, polygon, spline, revision cloud, CAD blocks |
| Cabinets, Library | `cabinet.rs`, `library.rs` | Base, Wall, Full Height, Soffit, Shelf, Partition; library symbols |
| Stairs | `stairs.rs` | Draw, Straight, L-Shaped, U-Shaped, Curve Left/Right, Landing, Ramp |
| Roofs | `roof.rs` | Roof Plane, Edit, Build Roof, Gable/Roof Line, Roof Hole, Skylight, Join, Auto Dormer |
| Electrical | `electrical.rs` | outlets, switches, lights, connection, Auto Place Outlets |
| Terrain | `terrain.rs` | perimeter, elevation data, modifiers, features, roads |
| Cameras | `camera.rs` | Full Camera, overviews, Doll House, cross sections |

Still dimmed in the toolbar: framing, slabs, trim, curved walls and the other
tools whose flyouts are not built (see the `NotImplemented` list printed by
`cargo test -p plan-app flyout_entries -- --nocapture`).

The Edit toolbar is open-ended (`EditActionKind::Custom`): stairs add Auto
Stairwell, Flare/Curve, Add/Remove Breakline and Make Railing; roof planes Join
Roof Planes and Rebuild Roofs; symbols Replace From Library; devices Flip Side
and Rotate. Specification dialogs of every kind open from `open_spec`
(double-click or Enter in Select Objects). File > Templates > Import Chief
Template... seeds the defaults from a Chief `.plan` file.
