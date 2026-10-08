# plan-app (Plan Studio)

A desktop 2D floor-plan editor built with egui/eframe on top of `plan-core`.
Draw walls, drop in doors and windows, and see rooms and areas update live.

Run it with `cargo run -p plan-app` (binary name: `plan-studio`).

Controls:
- Toolbars (Chief Architect style, dark theme): row 1 (file, view, floors, 3D and material tools), row 2 (Build tools) and a vertical view bar on the right edge. Hover any button for its name and hotkey. Flyout buttons (small arrow on the right) open a variant list; clicking the icon half activates the variant shown. Dimmed buttons are not implemented yet. The Library, Project and Layer Display buttons open a placeholder dock panel.
- Tools: see Hotkeys below. Esc cancels a wall in progress, otherwise returns to Select.
- View bar: Zoom In/Out, Undo Zoom, Fill Window, Pan Window (left-drag pans; Esc or Select returns), and display toggles. Crosshairs draws cursor crosshairs; Temporary Dimensions (on by default) shows the live length while drawing walls. The floor arrows in row 1 switch floors.
- Wall tool: click to place points (continuous chain). Snaps to endpoints, the snap grid and 15 degree angles; hold Alt to skip angle snap. Esc or right-click ends the chain.
- Door/Window: click on a wall. Select: click a wall; Delete/Backspace removes it.
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
| `Shift+Y`, `Shift+T`, `Shift+A`, `Q`, `Y`, `K`, `Shift+P` | Draw Stairs, Base Cabinet, Auto Exterior Dimensions, Roof Plane, Text, Circle, Rectangular Polyline (not built yet: show a status message) |

`D, H` is a two-key sequence: after `D` the status bar shows `D, ...` for 1.5 seconds. Four-modifier Chief chords (such as Straight Interior Wall, Ctrl+Alt+Cmd+6) appear in tooltips and menus but are not bound.

## Themes and brightness

The canvas has four themes, chosen in View > Canvas Theme or the Theme box in the Properties panel: Low Glare (default, a warm mid-gray with no pure whites, for light-sensitive users), Paper, Dark and High Contrast. View > UI Brightness (0.6 to 1.0) dims the panels, text and toolbar icons without touching the OS display. Both settings are saved to `~/.plan-studio/settings.json`.
