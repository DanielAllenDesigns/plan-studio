# Plan Studio Reference Manual

Plan Studio is an open-source residential design CAD application written in
Rust. It follows the structure and vocabulary of the Chief Architect X18
Reference Manual: you draw walls on a floor plan and the rooms, 3D views,
elevations, schedules and documents are generated from that one model.

This manual describes the program as it exists in the repository on
2026-10-08 (version 0.1.0, pre-alpha, through the Round 7 commit `8112e1c`; Round 8 is in flight). It is written to be honest about what
works today and what is still on the roadmap. If you know Chief Architect, the
chapter and section layout will look familiar; if you do not, start with
chapter 1 and the glossary in chapter 15.

## Table of contents

| Chapter | File | What it covers |
|---|---|---|
| 1 | [01-getting-started.md](01-getting-started.md) | Installing from source, first launch, templates, Daniel's Chief defaults and seeding from your own Chief templates, the window layout, toolbars and docks, a first look at the layout view, low-glare themes and UI brightness |
| 2 | [02-walls.md](02-walls.md) | Wall tools (straight, curved, foundation, pony, glass, half, divider, railing, deck, fencing), snapping, joins, Select Objects editing, the Wall Specification dialog (including Bottom Height), wall types |
| 3 | [03-doors-windows.md](03-doors-windows.md) | Door and window placement, the Door and Window Specification dialogs |
| 4 | [04-rooms-floors.md](04-rooms-floors.md) | Room detection, Room Specification, floors, foundations, Space Planning Assistant, Plan Check (slabs and platform holes are in chapter 16) |
| 5 | [05-dimensions-text-cad.md](05-dimensions-text-cad.md) | Dimension tools (including elevation and story pole strings), text, rich text, macros and note types, CAD drawing and edit tools, CAD blocks, hatch, the CAD, Text and Dimension dialogs, layers |
| 6 | [06-cabinets-library.md](06-cabinets-library.md) | Cabinets (fillers, corner and blind cabinets, custom countertops, Generate Countertop, the Cabinet Specification), the Library Browser, placed symbols, the Chief Architect catalogs read from your install |
| 7 | [07-stairs.md](07-stairs.md) | Stair tools, the Stair Specification, stairwells, ramps |
| 8 | [08-roofs.md](08-roofs.md) | Build Roof, roof planes, Gable/Roof Line, Join Roof Planes, roof holes and skylights, ceiling planes (drawn and built), dormers (auto, floating, exploded), roof returns |
| 9 | [09-electrical-terrain.md](09-electrical-terrain.md) | Electrical devices and circuits, terrain, roads and site tools |
| 10 | [10-3d-views-rendering.md](10-3d-views-rendering.md) | 3D views, cameras, rendering techniques, the Sun Angle, the ray tracer, vector elevations, Auto Elevations and Wall Elevation cameras, walkthroughs, lights |
| 11 | [11-layout-schedules-print.md](11-layout-schedules-print.md) | Schedules (windows and tables placed in the plan), the layout view (pages, boxes, Send to Layout, Page Setup, print to PDF), Project Information and the title block, materials list, construction sets, Build Framing, the manual framing tools and Framing Takeoff |
| 12 | [12-import-export.md](12-import-export.md) | The plan file and its typed slots, DXF export and import, CAD to Walls, glTF, PDF (construction set and layout), CSV, Chief catalogs (.calib), Chief templates and automatic seeding |
| 13 | [13-hotkeys.md](13-hotkeys.md) | The full hotkey table, Daniel's Chief hotkeys, the Customize Hotkeys dialog |
| 14 | [14-architecture-for-contributors.md](14-architecture-for-contributors.md) | Crate map, typed slots and object references, adding a tool, testing, the scenario harness and QA findings |
| 15 | [15-glossary.md](15-glossary.md) | Residential design terms in plain language |
| 16 | [16-foundation-slabs.md](16-foundation-slabs.md) | Slabs, slab holes, square pads, round piers, and holes in floor and ceiling platforms, with corner handles |
| 17 | [17-exterior-details.md](17-exterior-details.md) | Corner boards, quoins, moldings, floor and wall material regions, wall hatching, polygon decks and the 3D solids |

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

## Status at a glance (2026-10-08, through Round 7)

| Area | Works today | Not yet |
|---|---|---|
| Walls (ch. 2) | Straight and curved exterior and interior walls; foundation, pony, glass, glass pony, half-wall, room divider, railing, deck railing, deck edge and fencing walls (curved variants take three clicks); click chains and drags, snapping, automatic corner/T/crossing joins, Select Objects editing, typed temporary dimensions, Wall Specification (Wall Class, pony types, foundation and half-wall height, fence style, rail style, Bottom Height; Invisible, No Room Definition and No Locate saved), Wall Type Definitions, Wall Hatching, Wall Material Region and Slab Footing (ch. 17) | Mitered joins and 3D opening cuts on curved walls, an editable arc, terrain walls and curbs, typed length while drawing, Break Wall, Reverse Layers, Fix Wall Connections button; Bottom Height in the plan, rooms, elevations and wall framing |
| Doors and windows (ch. 3) | Hinged Door, Window, ghost placement, sliding along walls, Reverse Swing, both specification dialogs (a door's Swing side and Hinge side are separate settings; style, thickness, swing angle, jamb or frame width saved with the opening), `D01` / `W01` callouts from a placed schedule | All other door and window styles, opening plan labels, mulling, resize handles, swing and hinge from the click side (QA-01, Round 8) |
| Rooms and floors (ch. 4) | Automatic rooms, interior and standard areas, Room Specification (conditioned, stem wall, moldings, fill and label options saved), Room Types list, new/insert/delete/exchange floors, foundations, Reference Display (floor below in gray), Space Planning Assistant, Plan Check, Floor Material Region (ch. 17) | Floor Defaults, function-driven room behavior, nested-room holes; a room's own ceiling height in 3D (QA-02, Round 8) |
| Slabs and platform holes (ch. 16) | Slab, Slab with Footing, Slab Hole(s), Square Pad, Round Pier, Hole in Floor and Ceiling Platform: polygon drawing, select/move/delete/undo, corner handles on slabs and holes, specification dialogs, plan display, 3D | Concrete in the Materials List |
| Exterior details (ch. 17) | Corner boards, quoins (manual and auto), moldings, floor and wall material regions, wall hatching, polygon decks, 3D solids (3D Solid, Face, Cone, Cylinder, Pyramid, Sphere) with dialogs, selection with handles, plan drawing and 3D; corner boards and quoins follow dragged walls | Molding miters, a Custom molding profile editor, 3D Solid Feature, a stored line style |
| Dimensions, text, CAD (ch. 5) | Nine dimension tools plus four automatic ones (exterior, interior, elevation and story pole); text, rich text with markup and B / I / U, leader, arrow line, callouts (circle, hexagon, square), markers, notes with note types, text macros; points, lines, arcs, circles, boxes, cross, blocking and insulation boxes, splines, revision clouds, CAD blocks and the block manager; Fillet, Chamfer, Offset, Trim, Extend, Break, Reverse, Make Parallel / Perpendicular, converts, Hatch, CAD Detail From View; own line style, fill and arrows in the CAD dialog; layers, layer sets, saved plan views, Saved Dimension Defaults, Text Styles | Associative dimensions, printed-size text, Find/Replace Text, Replace Fonts, Edit toolbar buttons for the CAD edit tools, the Revision Cloud toolbar toggle, Current CAD Layer |
| Cabinets and library (ch. 6) | Sixteen cabinet kinds (base, wall, full height, soffit, shelf, partition, three fillers, corner and blind cabinets, custom countertop, backsplash and counter hole), Generate Countertop (`G`), appliance openings, sink and cooktop cutouts, Cabinet Specification with a draggable face editor, door and drawer styles, Opening Indicators, moldings, per-part materials, labels, Cabinet Schedule; Library Browser with about 145 built-in 2D symbols and the Chief Architect Core, Bonus, Manufacturer and User catalogs read from your install, symbol placement, Replace From Library | Editing the Sides and Back faces, library door styles, countertop corner treatments, Add to User Library, Import Library; cabinets in 3D (QA-05, Round 8) |
| Stairs (ch. 7) | Straight, L, U, winder, ramp, landing, IRC solver, Stair Specification, Auto Stairwell | Railings, 3D view (QA-06), stairwell cut in the floor (QA-04) - the stairs builder has these in Round 8 |
| Roofs (ch. 8) | Build Roof (skeleton), Roof Plane, edit, Gable/Roof Line, Join Roof Planes, roof holes, skylights, ceiling planes (drawn, built for vaulted rooms, Ceiling Plane Specification), Auto Dormer, Auto Floating Dormer, Explode Dormer (keeps its walls), Roof Return, Auto Roof Return and Extend Slope Downward from data, Roof Plane Specification with Holes and Build Roof Edge tabs, 3D, roof framing through Build Framing | The Wall Roof tab, Dutch gable, knee wall, upper pitch, gutters, fascia |
| Framing (ch. 11.11) | Build Framing and Build All Framing (walls, floors, roofs, honoring direction and bearing lines, markers and truss bases), 19 manual framing tools, Select Objects picking with handles, Framing Member Specification, Framing Takeoff with Export CSV and Export Material List, manual framing in 3D and DXF | Framing defaults dialog, corner and T backing, combined headers |
| Electrical and terrain (ch. 9) | Outlets, lights, switches, connections, Auto Place Outlets; perimeter, elevations, hills, roads, Build Terrain with contours | Circuits UI, electrical schedule, 3D terrain |
| 3D (ch. 10) | Overviews, Doll House, Full Camera, sections, elevations, nine technique looks, vector elevations (Vector View and Technical Illustration) with Refresh, pan and zoom, Auto Elevations, Auto Back-Clipped Elevations, Wall Elevation cameras, walkthroughs (play and record), Add Lights and Adjust Lights, Sun Angle (date or angles), ray tracer, glTF export, 3D View Defaults (`Cmd+1`), cameras in the Project Browser, walls of every class, slabs, roofs with holes and dormers, manual framing, exterior details and placed symbols in 3D | Materials painting, cabinets and stairs in 3D (QA-05, QA-06), terrain and electrical in 3D, opening cuts in curved walls; vector elevations cut square to the nearest axis; walkthrough recording is low-quality path-traced frames, not a video |
| Layout and documents (ch. 11) | The layout view (File > New Layout, page tabs, boxes with handles, Send to Layout, Layout Box Specification, Page Setup, Page Table, Print Layout and Export Layout PDF), Project Information and title block macros, schedules placed in the plan with callout labels and the Schedule Specification, door, window, room, wall schedule windows, Framing Takeoff, materials list, construction set PDF, Drawing Sheet and Print Preview | One layout per plan, box rotation, text / image / schedule boxes from the editor, a printer dialog, grouping and totals in schedules, extra Project Information macros in the title block |
| Import/export (ch. 12) | `.psplan` with typed slots and automatic migration of older files, glTF, PDF (construction set and layout), PNG, CSV, Markdown, DXF export (plan, roof planes, framing; elevations), DXF import, CAD to Walls, Import Chief Template, automatic seeding from your Chief default templates | DWG, Import Library |
| Defaults and docks (ch. 1) | Default Settings (walls, doors, windows, Dimensions, Text Styles, Room Types, Preferences > Templates), File > Templates including Import Chief Template with Set as default, seeding from your Chief plan and layout templates, Project Browser floors, cameras, Saved Views and Layout, View toggles Color, Line Weights, Reference Display, Drawing Sheet and Print Preview | Defaults groups for cabinets, roofs, stairs and the rest, File > New Plan From Template |
| Hotkeys (ch. 13) | Base table, Daniel's Chief hotkeys (100 of 143 named bindings work), Customize Hotkeys, four-modifier chords as `Ctrl+Alt+...` off macOS (Command folds into Control there), Window menu labels that follow the live map | Distinguishing Control from Command off macOS (they are one key there) |
| QA (ch. 14.8) | Twelve scenario test files drive the tools headlessly; seven findings are in `docs/qa-findings.md` | Round 8 is fixing QA-01 to QA-03 and QA-05 to QA-07; the stairs builder has QA-04 |

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
