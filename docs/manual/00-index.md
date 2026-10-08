# Plan Studio Reference Manual

Plan Studio is an open-source residential design CAD application written in
Rust. It follows the structure and vocabulary of the Chief Architect X18
Reference Manual: you draw walls on a floor plan and the rooms, 3D views,
elevations, schedules and documents are generated from that one model.

This manual describes the program as it exists in the repository on
2026-10-08 (version 0.1.0, pre-alpha). It is written to be honest about what
works today and what is still on the roadmap. If you know Chief Architect, the
chapter and section layout will look familiar; if you do not, start with
chapter 1 and the glossary in chapter 15.

## Table of contents

| Chapter | File | What it covers |
|---|---|---|
| 1 | [01-getting-started.md](01-getting-started.md) | Installing from source, first launch, templates and Daniel's Chief defaults, the window layout, toolbars and docks, low-glare themes and UI brightness |
| 2 | [02-walls.md](02-walls.md) | Wall tools, snapping, joins, Select Objects editing, the Wall Specification dialog, wall types |
| 3 | [03-doors-windows.md](03-doors-windows.md) | Door and window placement, the Door and Window Specification dialogs |
| 4 | [04-rooms-floors.md](04-rooms-floors.md) | Room detection, Room Specification, floors, foundations, Space Planning Assistant, Plan Check |
| 5 | [05-dimensions-text-cad.md](05-dimensions-text-cad.md) | Dimension tools, text and annotation, CAD drawing tools, layers |
| 6 | [06-cabinets-library.md](06-cabinets-library.md) | Cabinets, the Library Browser, placed symbols, Chief catalogs |
| 7 | [07-stairs.md](07-stairs.md) | Stair tools, the Stair Specification, stairwells, ramps |
| 8 | [08-roofs.md](08-roofs.md) | Build Roof, roof planes, Gable/Roof Line, roof holes and skylights |
| 9 | [09-electrical-terrain.md](09-electrical-terrain.md) | Electrical devices and circuits, terrain, roads and site tools |
| 10 | [10-3d-views-rendering.md](10-3d-views-rendering.md) | 3D views, cameras, rendering techniques, the ray tracer |
| 11 | [11-layout-schedules-print.md](11-layout-schedules-print.md) | Schedules, Framing Build and Takeoff, materials list, construction sets, the drawing sheet and Print Preview, layout, printing |
| 12 | [12-import-export.md](12-import-export.md) | DXF export and import, CAD to Walls, glTF, PDF, CSV, Chief catalogs (.calib) and Chief templates |
| 13 | [13-hotkeys.md](13-hotkeys.md) | The full hotkey table, Daniel's Chief hotkeys, the Customize Hotkeys dialog |
| 14 | [14-architecture-for-contributors.md](14-architecture-for-contributors.md) | Crate map, adding a tool, testing |
| 15 | [15-glossary.md](15-glossary.md) | Residential design terms in plain language |

## Conventions

### Status marks

Every feature in this manual is described as it behaves in the program today.
Where it does not work yet, the manual says so.

| Mark | Meaning |
|---|---|
| (no mark) | Works in the application today. |
| (planned) | Not usable yet. The toolbar button or menu item is dimmed, reports "not implemented yet" in the status bar, or does not exist. Described so you know the target. |
| (planned; engine in `plan-xxx`) | The calculation is written and unit-tested in a library crate, but no screen in the editor calls it yet. |
| (session only) | The dialog accepts the setting, but the plan file does not store it. It is forgotten when you close the program. |
| (disabled) | The control is drawn but grayed out, as Chief does for options you cannot change. |

A section titled **Tools** lists the toolbar button, its hotkey, and what a
click, a drag and the other keys do. A section titled **Dialog** lists the
tabs of the object's specification dialog and the fields on each tab.

### Hotkeys

Hotkeys are written the way the program's tooltips write them in text form.

- `Shift+Q` is a chord: hold the modifier and press the key.
- `D, H` is a sequence: press `D`, release, then press `H` within 1.5 seconds.
  The status bar shows `D, ...` while the program waits.
- `Ctrl` is the Control key and `Cmd` is the Command key on macOS. On Windows
  and Linux there is no Command key: `Cmd` stands for the Control key and the Mac
  Control modifier folds into it, so the four-modifier chord `Ctrl+Alt+Cmd+6` is
  typed Ctrl+Alt+6 and menus and tooltips write `Ctrl+` (see chapter 13).
- `Alt` is Option on macOS.

The hotkeys in this manual are Daniel's Chief Architect X18 bindings. Several
repository documents call the same keys "Chief defaults"; they were captured
from Daniel's customized Customize Hotkeys dialog, and the factory file
differs (for example, factory Straight Exterior Wall is `W`, factory Hinged
Door is `Shift+E`). Chapter 13 explains how they are loaded.

### Units and typing lengths

Lengths are stored in inches. The program shows and accepts architectural
feet-and-inches. Accepted input includes `12'`, `12' 6"`, `12'-6 1/2"`, `6"`,
`1/2"`, `12.5'` and a bare number, which means inches (`96` is 8 feet).
Displayed values round to the smallest fraction in the dimension defaults
(1/8" in Daniel's template).

Angles are degrees, counter-clockwise from the +X axis (east) in plan.

### Names

Plan Studio uses Chief Architect's names for tools, dialogs and menu items
(Select Objects, Straight Exterior Wall, Wall Specification, Fill Window,
Reference Display and so on). The in-program text, the status bar hints and
this manual all use those names. "Plan Studio" is a working title; see
`DECISIONS.md` in the repository root.

### File names

- `.psplan` is Plan Studio's plan file (JSON).
- `~/.plan-studio/` holds your settings, saved template defaults and hotkey edits.
- Paths in this manual are relative to the repository root unless they start with `~`.

## Status at a glance (2026-10-08)

| Area | Works today | Not yet |
|---|---|---|
| Walls (ch. 2) | Straight exterior and interior walls, click chains and drags, snapping, automatic corner/T/crossing joins, Select Objects editing, typed temporary dimensions, Wall Specification (Invisible, No Room Definition and No Locate are saved) and Wall Type Definitions | Curved, foundation, pony, half, glass walls and room dividers as tools; typed length while drawing; Break Wall, Reverse Layers, Fix Wall Connections button |
| Doors and windows (ch. 3) | Hinged Door, Window, ghost placement, sliding along walls, Reverse Swing, both specification dialogs (a door's Swing side and Hinge side are separate settings) | All other door and window styles, plan labels, mulling, resize handles |
| Rooms and floors (ch. 4) | Automatic rooms, interior and standard areas, Room Specification, Room Types list, new/insert/delete/exchange floors, foundations, Reference Display (floor below in gray), Space Planning Assistant, Plan Check | Floor Defaults, function-driven room behavior, nested-room holes |
| Dimensions, text, CAD (ch. 5) | Nine dimension tools plus auto exterior and interior, seven text tools, points/lines/arcs/circles/boxes/polylines/splines, layers, layer sets and saved plan views, line weights on screen, Saved Dimension Defaults and the Text Styles editor (Edit > Default Settings) | Associative dimensions, printed-size text, Fillet and Chamfer |
| Cabinets and library (ch. 6) | Six cabinet kinds, Cabinet Specification with face tree, Library Browser with about 145 built-in 2D symbols, symbol placement | Fillers, custom countertops, 3D symbols, Chief catalogs in the editor |
| Stairs (ch. 7) | Straight, L, U, winder, ramp, landing, IRC solver, Stair Specification, Auto Stairwell | Railings, 3D view, stairwell cut in the floor |
| Roofs (ch. 8) | Build Roof (skeleton), Roof Plane, edit, Gable/Roof Line, hole, skylight, 3D, roof framing through Build Framing | Dormers, join planes, ceiling planes, gutters, manual rafter and truss tools |
| Electrical and terrain (ch. 9) | Outlets, lights, switches, connections, Auto Place Outlets; perimeter, elevations, hills, roads, Build Terrain with contours | Circuits UI, electrical schedule, 3D terrain |
| 3D (ch. 10) | Overviews, Doll House, Full Camera, sections, elevations, nine technique looks, Sun Angle, ray tracer, glTF export, 3D View Defaults (`Cmd+1`), cameras in the Project Browser | Walkthroughs, lights, materials painting, stairs/cabinets/terrain in 3D |
| Documents (ch. 11) | Door, window, room, wall schedules, Build Framing and Framing Takeoff (CSV), materials list, construction set PDF, Drawing Sheet and Print Preview on the plan (sheet size and scale in Project Browser > Layout) | Print, interactive layout, other schedules, manual framing tools |
| Import/export (ch. 12) | `.psplan`, glTF, PDF, PNG, CSV, Markdown, DXF export (plan and elevations), DXF import, CAD to Walls, Import Chief Template | DWG, Chief catalogs in the editor |
| Defaults and docks (ch. 1) | Default Settings (walls, doors, windows, Dimensions, Text Styles, Room Types), File > Templates including Import Chief Template, Project Browser floors, cameras, Saved Views and Layout, View toggles Color, Line Weights, Reference Display, Drawing Sheet and Print Preview | Defaults groups for cabinets, roofs, stairs and the rest |
| Hotkeys (ch. 13) | Base table, Daniel's Chief hotkeys, Customize Hotkeys, four-modifier chords as `Ctrl+Alt+...` off macOS, Window menu labels that follow the live map | Distinguishing Control from Command off macOS (they are one key there) |

## Where this manual comes from

The tool names, tab lists and field names come from captures of Chief
Architect X18 stored in `docs/chief-x18-toolbars.md`, `docs/chief-x18-menus.md`,
`docs/chief-x18-subtools.md` and `docs/chief-x18-dialogs.md`. The behavior
descriptions come from the parity specifications in `docs/parity/` (ids such as
`W-21` or `DW-8` appear in the source code comments), checked against the code
as it stands. Where a parity document says "verify in Chief", the manual
describes what Plan Studio does, not what Chief does.

Icons in Plan Studio are original artwork. No Chief Architect code, assets or
catalog content ships in this repository.

## Reporting problems

Open an issue in the project repository. Chapter 14 explains how to build and
test the program and how to add a tool, in case you want to fix it yourself.
