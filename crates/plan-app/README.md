# plan-app (Plan Studio)

The Plan Studio desktop editor, built with egui/eframe on top of `plan-core` and
the engine crates. Draw walls, doors, windows, cabinets, stairs, roofs and
terrain; see rooms and areas update live; look at the plan in 3D; and write
schedules, DXF, glTF and PDF. The project overview is the repository `README.md`.

Run it with `cargo run -p plan-app` (binary name: `plan-studio`).

Controls:
- Toolbars (Chief Architect style, dark theme): row 1 (file, view, floors, 3D and material tools), row 2 (Build tools) and a vertical view bar on the right edge. Hover any button for its name and hotkey. Flyout buttons (small arrow on the right) open a variant list; clicking the icon half activates the variant shown. Dimmed buttons are not implemented yet.
- Tools: every Chief tool in `src/tools/` (table below); hotkeys are listed under Hotkeys. Esc cancels the operation in progress, otherwise returns to Select.
- View bar: Zoom In/Out, Undo Zoom, Fill Window, Pan Window (left-drag pans; Esc or Select returns), and the display toggles (see View toggles). The floor arrows in row 1 switch floors.
- Wall tool: click to place points (continuous chain), or press-drag-release to draw a single wall. Snaps to endpoints, midpoints, intersections, wall centerlines, the snap grid and 15 degree angles (marker and name in the status bar); hold Alt to skip angle snap. A wall started or ended on another wall splits it there (T-junction). Esc or right-click ends the chain.
- Door/Window: hover a wall to see the opening and its distances to both wall ends; click to place it centered on the pointer (snapped to 1"). New openings take their size from the Default Settings door/window templates.
- Select: click an object (Shift adds), Tab cycles objects under the pointer, drag empty space for a marquee (left-to-right encloses, right-to-left touches). Dragging a wall moves it perpendicular with its connected walls stretching (Alt: free move); the end handles stretch it and dropping an end on another wall splits that wall; dragging an opening slides it along the wall. Click a temporary dimension value, type a length and press Enter to move the object. Delete/Backspace removes the selection, Esc cancels a drag.
- Edit toolbar (bottom left of the canvas, when something is selected): Open Object, Delete, Copy, Paste in Place, Reverse Swing (doors), plus the commands of the selected kind (stairs, roof planes, symbols, devices).
- Undo/Redo: Edit menu, the row-1 buttons, `Cmd+Z`, `Shift+Cmd+Z`, `Cmd+Y`; the menu names the step ("Undo Move Wall").
- Specification dialogs: double-click an object (or press Enter) with the Select tool, use "Open Specification..." / the "..." button in the Properties panel, or the Edit toolbar's Open Object.
- Docks (view bar): Library Browser, Project Browser, Active Layer Display Options.
- View: scroll or pinch to zoom at the cursor; middle- or right-drag to pan.
- Files are `.psplan` JSON. On Windows and Linux the Command key is the Control key (see Platforms).

## Menus

The menu bar follows Chief Architect X18: File, Edit, Build, Terrain, Library, 3D, CAD, Tools, View, Window, Help (see `docs/chief-x18-menus.md`). The Build, Terrain and CAD submenus are generated from the same flyout tables as the toolbar (`toolbar.rs`, one function per group, lists from `docs/chief-x18-subtools.md`), so each entry shows its icon, name and hotkey. Shortcuts shown in the Window menu and in 3D View Defaults come from the live hotkey map, so Daniel's customized keys (his `-` is Zoom In) and your own edits show up.

- **File**: New Plan, Open Plan, Save, Save As, Templates (Save Current Defaults as My Template, Import Chief Template, Reset to Chief X18 Template), Export (DXF of the active floor, Elevation DXF of the four sides, Construction Set PDF, glTF), Import (Import Drawing (DXF) with a units choice), Quit.
- **Edit**: Undo/Redo, Select Objects, Default Settings (see below).
- **Build / Terrain**: every working tool and command, including Build > Framing (Build Framing `Shift+Cmd+S`, Build All Framing, Delete Framing) and the Floor, Roof, Stairs, Cabinet and Electrical flyouts.
- **3D**: Create Orthographic View (the elevations, plan overhead, sections, Export Elevations (DXF)), Create Perspective View (Full Camera, overviews, Doll House, Ray Trace), Rendering Techniques, Rebuild 3D, Export glTF, 3D View Defaults (`Cmd+1`).
- **CAD**: the point, line, arc, circle, box, dimension, text and CAD block tools, and CAD to Walls.
- **Tools**: Layer Settings, Checks (Plan Check, Door/Window Check, Plan Footprint), Toolbars and Hotkeys, Space Planning, Schedules (door, window, room, wall, Framing Takeoff, Create Construction Set), Materials List.
- **View**: the docks, the display toggles, Canvas Theme and UI Brightness. **Window**: zoom, Fill Window, Pan Window. **Help**: About.

Items for features that are not built are listed disabled with Chief's name.

## Hotkeys

Active whenever no text field has focus. The fixed base table is `toolbar::BINDINGS`; `shell/hotkeys.rs` turns it into a runtime map keyed by command name, overrides it with Daniel's Chief hotkeys (`plan-config`), then with your edits from Tools > Toolbars and Hotkeys > Customize Hotkeys (`~/.plan-studio/hotkeys.json`). The full list is chapter 13 of the manual; the fixed ones:

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
| `Cmd+1` | 3D View Defaults |
| `Shift+Cmd+S` | Build Framing |
| `Shift+Y`, `Shift+T`, `Shift+A`, `Q`, `Y`, `K`, `Shift+P` | Draw Stairs, Base Cabinet, Auto Exterior Dimensions, Roof Plane, Text, Circle, Rectangular Polyline |
| `Shift+J`, `Shift+K` | Full Camera, Perspective Full Overview |

`D, H` is a two-key sequence: after `D` the status bar shows `D, ...` for 1.5 seconds. Sequences can be up to four chords.

Platforms: Chief's "Ctrl" is the Command key on a Mac. Where there is no Command key (Windows, Linux) it is the Control key, and the Mac Control modifier folds into it (`shell::hotkeys::platform_modifiers`), so Chief's four-modifier chords become Ctrl+Alt+... chords. Menus and tooltips then say `Ctrl+`. Settings, your template and your hotkey edits live in `~/.plan-studio/` (`paths.rs`: `HOME`, else `USERPROFILE`, else `HOMEDRIVE` + `HOMEPATH`; no `dirs` crate).

## Themes and brightness

The canvas has four themes, chosen in View > Canvas Theme or the Theme box in the Properties panel: Low Glare (default, a warm mid-gray with no pure whites, for light-sensitive users), Paper, Dark and High Contrast. View > UI Brightness (0.6 to 1.0) dims the panels, text and toolbar icons without touching the OS display. Both settings are saved to `~/.plan-studio/settings.json`.

## View toggles

The view bar and the View menu share one flag set (`toolbar::ViewFlag`). What each does (`editor/render.rs`, `editor/restyle.rs`, `editor/sheet.rs`):

| Toggle | Effect |
|---|---|
| Color (`F8`, on by default) | Off: the plan draws in grays (every color to its luminance); highlights stay colored |
| Line Weights | On: stroke widths scale with each layer's line weight (0.25 mm is the base width) |
| Reference Display | Draws the walls of the floor below in gray (nothing on the lowest floor); a saved plan view that has it set turns it on |
| Drawing Sheet | Outlines the active layout's sheet (size and scale set in the Project Browser's Layout section, Arch D at 1/4" by default) centered on the plan |
| Print Preview | Grays out everything outside that sheet |
| Reference Grid, Crosshairs, Temporary Dimensions, Arc Centers and Ends | As named |

## Specification dialogs

`dialogs.rs` is the Chief-style dialog frame (spec: `docs/chief-x18-dialogs.md`): a 150 px vertical tab list, the active tab's panel, a 220 px preview drawn with the painter, and Help / Cancel / OK along the bottom. Section headings are a label plus a rule. Length fields are text boxes that show feet-inches and parse as you type (red and OK blocked when the text does not parse); angles are degrees. The dialog edits a cloned draft: OK (or Enter) writes it back and marks rooms dirty, Cancel or Escape discards it. Tabs and controls the model cannot store yet are listed but disabled. There is a dialog for every object kind (`shell/spec_dialogs.rs` hosts all but walls and openings).

- Wall Specification (`dialogs/wall.rs`): General (thickness, length, angle, lock Start/Center/End, options), Structure, Wall Types (the plan defaults' wall types, with Wall Type Definitions), Layer, Label. Invisible, No Room Definition and No Locate are stored on the wall (`Wall.flags`), so room detection, drawing and dimensions honor them. Changing the length moves the end point (Start lock), the start point (End lock) or both (Center lock); openings keep their place on the wall.
- Door and Window Specification (`dialogs/opening.rs`): General, Options (Swing side and Hinge side are independent: `swing_flipped` and `hinge_at_end`), Casing, Jamb or Frame, Lites, Label, with an elevation sketch and a plan sketch.
- Default Settings (Edit > Default Settings..., `dialogs/defaults.rs`): a searchable tree. Exterior, Interior and Foundation Wall edit the wall defaults (a custom thickness adds a `Custom-<n>` wall type); Interior Door, Exterior Door and Window edit the templates new openings are placed from. **Dimensions** opens Saved Dimension Defaults (`dialogs/default_lists.rs`): the template's dimension sets with Edit, Copy, Rename, Delete and a Currently Active combo; Edit opens Dimension Defaults (Primary Format, Setup Automatic, Extensions, Arrow, Text Style). **Room Types** lists name, function, living area and conditioned with Add, Edit, Rename and Delete (a rename follows into the plan's named rooms, one undo step). **Text Styles** edits name, font, height, bold, italic, underline and color, for the open plan (undoable) and for the defaults new plans start from.

Door style, window type, lites, casing, jamb, label options and the like that the model has no fields for yet live in memory for the session only; those controls say so in a tooltip.

## Docks

- Active Layer Display Options (`shell/docks.rs`): the layer table with a name filter and layer properties; every edit is undoable. The view selector in row 1 switches saved plan views (each carries a layer set).
- Project Browser: Floors (click to switch), Cameras (click selects the camera, shows its floor and pans the plan to it), Saved Views (click activates the view), and Layout (the active layout's sheet size and scale, Create Construction Set).
- Library Browser (`shell/library_browser.rs`): search and category tree over the built-in catalog; clicking an item makes it the Library tool's active symbol. A Chief catalogs node is a placeholder until `plan-calib` is wired in.

## Templates and defaults

Plan Studio starts the way Chief Architect starts from a template plan. The defaults live in `plan_core::PlanDefaults` (`crates/plan-core/src/defaults.rs`): wall types and the exterior/interior/foundation wall defaults, interior and exterior door, window and cabinet defaults, the saved dimension sets, room types, text styles, the layer set and layer sets, grid and units. `PlanDefaults::chief_x18_daniel()` holds the values captured from Daniel's Chief X18 template (`docs/chief-x18-dialogs.md`); it ships as `assets/templates/chief-x18-daniel.json`, embedded in the binary (a unit test keeps the file and the code identical).

- On startup the app loads `~/.plan-studio/defaults.json` if it exists, else the embedded template. A file that cannot be read is reported in the status bar and the template is used. Keys missing from the file fall back to the template's values.
- File > New Plan creates `Project::from_defaults`: the template's ceiling height and layer set.
- New walls take the exterior or interior wall type thickness and height. New doors take the interior door defaults, or the exterior door defaults when the host wall is an exterior wall; new windows take the window defaults.
- Grid spacing, snap and the angle snap come from the defaults. Dimension text is rounded to the active dimension set's smallest fraction (1/8" in the Chief template) and uses its units when it picks inches or metric.
- File > Templates > Save Current Defaults as My Template... writes the current defaults to `~/.plan-studio/defaults.json`. File > Templates > Import Chief Template... seeds the defaults from a Chief `.plan` / `.tpl` / `.layout` and keeps them as your template. File > Templates > Reset to Chief X18 Template restores the embedded values and removes that file.

## Samples

`samples/` at the repository root holds three plans (ranch, two-story colonial, studio ADU) made by `cargo run -p plan-core --example make_samples`; open them with File > Open Plan.

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

Framing is built from Build > Framing (Build Framing, Build All Framing,
Delete Framing: `editor/framing_view.rs`, stored on the floor, drawn on the
Framing layer, with a Framing Takeoff window and CSV export). Still dimmed in the
toolbar: the manual framing tools (General Framing, Post, Joist, Rafter ...),
slabs, trim, curved walls and the other tools whose flyouts are not built (see
the `NotImplemented` list printed by `cargo test -p plan-app flyout_entries --
--nocapture`).

The Edit toolbar is open-ended (`EditActionKind::Custom`): stairs add Auto
Stairwell, Flare/Curve, Add/Remove Breakline and Make Railing; roof planes Join
Roof Planes and Rebuild Roofs; symbols Replace From Library; devices Flip Side
and Rotate. Specification dialogs of every kind open from `open_spec`
(double-click or Enter in Select Objects).

File exchange, CAD to Walls and the framing windows live in `dialogs/exchange.rs`
(`Action::File(..)` and `Action::Framing(..)`); each command that changes the
plan is one undo step.
