# Plan Studio

An open-source residential design CAD application written in Rust, in the
spirit of Chief Architect: you draw walls on a floor plan, and rooms, 3D,
elevations, schedules and construction documents are generated from that one
model.

**Status: pre-alpha, but a working editor.** Plan Studio opens, edits and saves
plans, draws in 2D and 3D, and writes schedules, DXF, glTF and PDF. See
[ROADMAP.md](ROADMAP.md) for what is done and what is left, and the
[reference manual](docs/manual/00-index.md) for how everything works today.
[docs/chief-x18-ui-notes.md](docs/chief-x18-ui-notes.md) is the UI study the
design is based on.

![Plan Studio with Chief-style toolbars, menus and the Low Glare theme](docs/screenshot-chief-toolbars.jpg)

Earlier milestone: [phase 0 screenshot](docs/screenshot-phase0.jpg).

## Why

Chief Architect is the reference tool for residential designers, but it is
closed, Windows/macOS only, and expensive. Plan Studio aims at the core
residential workflow, open source, with a modern Rust codebase that others can
extend.

## What it does today

- **Tools** (Chief's names and hotkeys): walls, doors and windows; dimensions
  (manual, end to end, interior, point to point, running, baseline, centerline,
  angular, tape measure, automatic exterior and interior); text (text, rich
  text, leader line, arrow line, callout, marker, note); CAD (points, lines,
  polylines, arcs, circles, boxes, polygons, splines, revision clouds, CAD
  blocks); cabinets (base, wall, full height, soffit, shelf, partition);
  library symbols; stairs (straight, L, U, winder, curved, landing, ramp);
  roofs (Build Roof, roof planes, gable lines, holes, skylights); electrical
  devices with Auto Place Outlets; terrain (perimeter, elevation data,
  modifiers, features, roads, Build Terrain); framing (walls, floor platforms
  and roofs, with a lumber takeoff); cameras.
- **Select Objects** picks, moves, stretches and edits every object kind, with
  temporary dimensions, snaps, an Edit toolbar, copy and paste, and whole-plan
  undo and redo that names each step.
- **Dialogs**: Chief-style specification dialogs for walls (with Wall Type
  Definitions), doors, windows, rooms, floors and foundations, dimensions,
  text, CAD, cabinets, symbols, stairs, roofs, electrical devices, terrain and
  cameras; Default Settings (walls, doors, windows, saved dimension defaults,
  room types, text styles); Customize Hotkeys; Layer Display Options; the Space
  Planning Assistant; Plan Check and Door/Window Check; schedules and the
  Materials List.
- **Docks**: Active Layer Display Options (layer sets and saved plan views),
  Project Browser (floors, cameras, saved views, layout sheet) and the Library
  Browser (about 145 built-in 2D symbols).
- **3D**: overview, floor overview, doll house, Full Camera, cross sections and
  the four elevations in an orbitable view; nine rendering techniques, Sun
  Angle, a CPU path tracer with PNG output, and glTF export.
- **Documents**: door, window, room and wall schedules, materials list, framing
  takeoff (CSV), hidden-line elevations, construction set PDF, DXF export of a
  floor or the four elevations, DXF import and CAD to Walls.
- **Chief data**: Daniel's Chief X18 template is built in; File > Templates >
  Import Chief Template reads a Chief `.plan` for its layer sets, wall types,
  text styles and dimension defaults; his customized hotkeys are loaded on top of
  Chief's, and you can edit them. `plan-calib` reads the `.calib` catalogs of
  your own Chief install (not yet shown in the Library Browser).
- **Look**: four canvas themes (Low Glare is the default) and a UI brightness
  dimmer. View toggles for Color, Line Weights, Drawing Sheet, Print Preview and
  Reference Display change what the plan shows.
- **Samples**: three plans in [`samples/`](samples/README.md) (ranch, two-story
  colonial, studio ADU) open from File > Open Plan.

## Architecture

Twenty-two crates in one Cargo workspace. Only `plan-app` and `plan-view3d`
touch the GUI; everything else is plain Rust, tested headlessly.

| Crate | What it is |
|---|---|
| `plan-core` | The model (project, floors, walls, openings, rooms, layers, defaults, units), geometry, snapshot undo, ASCII DXF export |
| `plan-app` | The desktop editor (egui/eframe): tools, dialogs, docks, menus, hotkeys, themes |
| `plan-3d` | Plan to triangle meshes and glTF 2.0 export (no GPU code) |
| `plan-view3d` | The egui/OpenGL 3D viewport widget and camera views |
| `plan-render` | CPU path tracer for the Physically Based and Clay techniques |
| `plan-roof` | Automatic roofs from a footprint (weighted straight skeleton) |
| `plan-stairs` | Parametric stair engine with IRC checks, plan symbols and meshes |
| `plan-cabinets` | Parametric cabinet engine (face layouts, countertops, labels) |
| `plan-electrical` | Devices, plan symbols, Auto Place Outlets, connections, circuits |
| `plan-terrain` | Terrain perimeter, elevation data, modifiers, roads, contours |
| `plan-framing` | Wall, floor and roof framing members and lumber takeoff |
| `plan-materials` | Material definitions, 2D hatches, textures, rendering technique presets, sun |
| `plan-library` | The catalog system behind the Library Browser |
| `plan-calib` | Read-only reader for Chief `.calib` / `.calibz` catalogs |
| `plan-chiefplan` | Read-only scanner for Chief `.plan` / `.layout` templates |
| `plan-config` | Reads Chief's hotkeys, toolbars and preferences |
| `plan-docs` | Schedules, materials list and the scaled plan-sheet PDF |
| `plan-layout` | Headless layouts: pages, boxes, title blocks, construction set PDF |
| `plan-elevation` | Hidden-line elevations, sections and plan overhead drawings |
| `plan-import` | DXF reader, CAD objects from drawings, CAD to Walls |
| `plan-check` | Plan Check and Door/Window Check rule engine |
| `plan-spaceplan` | The Space Planning Assistant (room boxes to a first plan) |

Inside `plan-app`, every Chief tool is one module behind a shared `Tool` trait
(`tools/`), the services they share (selection, snapping, handles, temporary
dimensions, undo, rendering) live in `editor/`, the dialogs in `dialogs/`, and
the docks, hotkeys and 3D panel in `shell/`. `main.rs` owns the window and the
wiring. See [docs/architecture-tools.md](docs/architecture-tools.md) and
chapter 14 of the manual.

- **The plan is the model.** Every view is derived from `plan-core`.
- Lengths are stored in inches as `f64`; `plan_core::units` formats and parses
  architectural feet-and-inches (`12'-6 1/2"`).
- Room detection builds a planar graph from wall centerlines, splits at
  T-junctions and crossings, and traces the bounded faces.

## Building

Install Rust via [rustup](https://rustup.rs), then:

```bash
cargo test --workspace       # unit tests for every crate
cargo run -p plan-app        # launch the editor (binary: plan-studio)
```

Release builds and per-OS packages come from the Release workflow
(`.github/workflows/release.yml`); CI runs fmt, clippy and the tests on macOS,
Linux and Windows.

## Controls (plan-app)

| Action | Input |
|---|---|
| Tools | `Space` (or `1`) Select, `2` current wall, `3` Hinged Door, `4` Window; every other tool has its Chief hotkey (see the manual, chapter 13) |
| Draw walls | click start, click end, keep clicking to chain, or press-drag-release for one wall; `Esc` to stop |
| Select / move | click, `Tab` to cycle, drag a wall to move it perpendicular, drag handles to stretch, drag empty space to marquee |
| Undo / redo | `Cmd+Z`, `Shift+Cmd+Z` or `Cmd+Y` (`Ctrl` instead of `Cmd` on Windows and Linux) |
| Angle snap | automatic at 15 degrees; hold `Alt` for free angle |
| Zoom / pan | scroll wheel or pinch / middle- or right-drag |
| Delete | select, press `Delete` |
| Hotkeys | Tools > Toolbars and Hotkeys > Customize Hotkeys |

## Documentation

- [Reference manual](docs/manual/00-index.md): 15 chapters from first launch to
  contributor notes, with an honest status mark on every feature.
- [docs/parity/](docs/parity/): the Chief behavior specifications the code cites
  (ids such as `W-21`), each with a "Plan Studio today" snapshot.
- [docs/chief-x18-*.md](docs/chief-x18-menus.md): captures of Chief's menus,
  toolbars, sub-tools and dialogs.
- [DECISIONS.md](DECISIONS.md): open questions for the project owner.

## Contributing

Issues and pull requests are welcome. Keep `plan-core` free of GUI code and
add a unit test with every geometry change. Run `cargo fmt`,
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` before opening a PR.

## License

MIT. See [LICENSE](LICENSE). No Chief Architect code, assets or catalog content
ships in this repository.
