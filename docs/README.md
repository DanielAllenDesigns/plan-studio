# Plan Studio documentation

An index of everything under `docs/`. The repository front door is the [README](../README.md); how to contribute is in
[CONTRIBUTING.md](../CONTRIBUTING.md).

## Using Plan Studio

The [reference manual](manual/00-index.md) is the user documentation. It is also built into the program (Help > Launch
Help) and every feature carries an honest status mark. There are 20 chapters.

| Chapter | Covers |
|---|---|
| [1 Getting started](manual/01-getting-started.md) | Installing, first launch, window layout, toolbars, docks, menus, templates, themes, preferences, files |
| [2 Walls](manual/02-walls.md) | Wall tools, typed length and angle, snapping, joins, Select Objects, the Wall Specification |
| [3 Doors and windows](manual/03-doors-windows.md) | Placement, every style, labels, mulling, the Door and Window Specification |
| [4 Rooms and floors](manual/04-rooms-floors.md) | Room detection, Room Specification, floors, Floor Defaults |
| [5 Dimensions, text and CAD](manual/05-dimensions-text-cad.md) | Dimension tools, text and notes, CAD drawing and editing |
| [6 Cabinets and the library](manual/06-cabinets-library.md) | Cabinets, countertops, the Library Browser and the User Catalog |
| [7 Stairs](manual/07-stairs.md) | Stair tools, landings, ramps, railings |
| [8 Roofs](manual/08-roofs.md) | Build Roof, roof planes, holes, skylights, dormers, ceiling planes |
| [9 Electrical and terrain](manual/09-electrical-terrain.md) | Devices, connections, terrain, roads, landscaping |
| [10 3D views, cameras and rendering](manual/10-3d-views-rendering.md) | 3D views, cameras, rendering techniques, the path tracer |
| [11 Layout, schedules and printing](manual/11-layout-schedules-print.md) | Layouts, schedules, the Materials List, Print and PDF |
| [12 Import and export](manual/12-import-export.md) | The plan file, file safety, DXF, glTF, OBJ, Chief template and plan import |
| [13 Hotkeys](manual/13-hotkeys.md) | The full hotkey table and the Customize Hotkeys dialog |
| [14 Architecture for contributors](manual/14-architecture-for-contributors.md) | Crate map, data flow, how to add a tool, testing, scenario tests |
| [15 Glossary](manual/15-glossary.md) | Residential design terms in plain language |
| [16 Slabs, pads, piers and platform holes](manual/16-foundation-slabs.md) | Foundation slabs and holes in floor and ceiling platforms |
| [17 Exterior details](manual/17-exterior-details.md) | Trim, material regions, decks and 3D solids |
| [18 Plan Check](manual/18-plan-check.md) | The IRC-based Plan Check and Door/Window Check |
| [19 Framing](manual/19-framing.md) | Build Framing: the dialog, auto rebuild and retain, framing layers, the Framing Overview, the wall detail |
| [20 Pictures, underlays and CAD details](manual/20-cad-details.md) | Images, underlays to trace over, CAD details |

[Sample plans](../samples/README.md) open from File > Open Plan.

## How the code is organized

- [architecture-tools.md](architecture-tools.md): the `Tool` trait, `ToolId`, `EditorContext`, `ObjectRef`, and the rules for
  tool builders.
- [manual chapter 14](manual/14-architecture-for-contributors.md): the crate map, data flow, how to add a tool, a rule or a format, and the test layers.
- [performance.md](performance.md): the redraw and refresh budgets and the benchmark.
- [fonts.md](fonts.md): how text styles map to system fonts and what is embedded in PDFs.

## Parity with Chief Architect X18

The project's progress is measured against Chief's behavior, one stable id per behavior.

- [parity-status.md](parity-status.md): the status of every id (Works, Partial, Missing, Differs-by-design), the evidence, the
  next gaps, and the "verify in Chief" list. `python3 scripts/parity-score.py` recounts it.
- [chief-feature-coverage.md](chief-feature-coverage.md): the master inventory of Chief X18 features mapped to parity ids.
- The parity specifications (each has a "Plan Studio today" snapshot):
  [walls](parity/walls.md),
  [select and edit](parity/select-and-edit.md),
  [doors and windows](parity/doors-windows.md),
  [rooms and floors](parity/rooms-floors.md),
  [roofs](parity/roofs.md),
  [dimensions, text, CAD and layers](parity/dimensions-text-cad.md),
  [3D views and cameras](parity/3d-views-cameras.md),
  [cabinets, stairs, framing, terrain, library and electrical](parity/cabinets-stairs-framing-terrain-library.md),
  [electrical (Round 14)](parity/electrical.md),
  [documentation and layout](parity/documentation-layout.md),
  [preferences, hotkeys and toolbars](parity/preferences-hotkeys-toolbars.md).
- [qa-findings.md](qa-findings.md): what the headless scenario tests found.

## Captures and studies of Chief Architect X18

Written from the maintainer's own copy of Chief to say what the target looks like; no Chief library content is copied.

- [chief-x18-ui-notes.md](chief-x18-ui-notes.md), [menus](chief-x18-menus.md), [toolbars](chief-x18-toolbars.md),
  [sub-tools and hotkeys](chief-x18-subtools.md) and [dialogs](chief-x18-dialogs.md).
- [chief-hotkeys-resolved.md](chief-hotkeys-resolved.md): the resolved hotkey table (generated; a test fails when it is stale).
- File format notes for the read-only readers: [libraries (.calib)](chief-library-format.md),
  [projects (.plan)](chief-plan-format.md) and [templates](chief-template-format.md).
- [daniel-chief-setup.md](daniel-chief-setup.md) and [daniel-template-inventory.md](daniel-template-inventory.md): the
  maintainer's Chief setup and template names the defaults are seeded from.
- `chief-config-raw/`: the maintainer's own Chief toolbar and hotkey files, the source of the generated hotkey table.
- Two screenshots from early in the project: [phase 0](screenshot-phase0.jpg) and [Chief-style toolbars](screenshot-chief-toolbars.jpg).
  Current screenshots are expected under `docs/screenshots/` (see the [README](../README.md#screenshots)).

## Working notes and release

- [../DECISIONS.md](../DECISIONS.md): numbered decisions and open questions, each with the assumption built into the code.
- [integration-queue.md](integration-queue.md): wiring left for whoever owns a shared file.
- [release-checklist.md](release-checklist.md): the gate, cutting a tag, what the Release workflow produces
  (macOS `.dmg` and zip, Windows zip, Linux tarball and AppImage, checksums), licensing, and the manual QA pass still owed.
- [../CHANGELOG.md](../CHANGELOG.md) and [../ROADMAP.md](../ROADMAP.md): what each round added and what is left.
- [../CONTRIBUTING.md](../CONTRIBUTING.md), [../CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) and [../SECURITY.md](../SECURITY.md).
