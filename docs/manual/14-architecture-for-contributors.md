# Chapter 14: Architecture for Contributors

This chapter is for people who want to read or change the code: the crate map,
how data flows, how to add a tool, and how to test. It complements
`docs/architecture-tools.md` (the original tool-plugin plan), which this chapter
brings up to date with the code.

## 14.1 Principles

1. **The plan is the model.** `plan-core` holds the data; every view (2D, 3D, elevations,
   schedules, documents) is derived from it. If you find yourself storing the same fact in two
   places, one of them should be computed.
2. **No GUI in the libraries.** Only `plan-app` and `plan-view3d` touch `egui`/OpenGL. Every other
   crate is plain Rust that can be tested headlessly and reused (exporters, batch tools).
3. **One crate per engine, one module per tool.** Engines (roofs, stairs, cabinets ...) are
   libraries with their own tests; the editor wires them to tools and dialogs.
4. **Inches everywhere**, as `f64`. Convert only at the edge (display, parse, export).
5. **Chief's words.** Names, status hints and tooltips use Chief Architect's vocabulary, and code
   comments cite the parity ids (`W-21`, `DW-8`, `CB-29`) from `docs/parity/*.md`.
6. **Honest stubs.** An unbuilt feature appears dimmed with Chief's name and says "not
   implemented yet"; it is never hidden, so the target is visible.

## 14.2 The crate map

Twenty-two crates in one Cargo workspace (`Cargo.toml`). Arrows point from a crate to what it
depends on.

```
plan-app ---> plan-core  plan-3d  plan-view3d  plan-roof  plan-cabinets  plan-stairs
          \-> plan-docs  plan-layout  plan-elevation  plan-library  plan-config
          \-> plan-terrain  plan-materials  plan-electrical  plan-spaceplan  plan-check
          \-> plan-render  plan-chiefplan  plan-framing  plan-import
plan-3d <--- plan-cabinets, plan-stairs, plan-electrical, plan-terrain, plan-materials,
             plan-framing, plan-elevation, plan-spaceplan, plan-render, plan-check
plan-layout -> plan-docs, plan-elevation        plan-view3d -> plan-render
plan-calib  -> plan-library (not yet a dependency of plan-app)
plan-chiefplan -> plan-core      plan-config -> plan-core
```

### Model and geometry

**plan-core.** The data model and the pure geometry, with no GUI. `Project` holds floors; a `Floor`
holds walls, openings, dimensions, CAD objects, room names, symbols, cabinets and stairs (the last
two as opaque JSON), and groups. It also has: `rooms` (planar-graph room detection from wall
centerlines, with interior and standard areas), `joins` (mitered wall outlines and per-layer bands),
`walls` (flags, curves, roof directives, connection and split helpers), `openings` (styles, casing,
labels), `floors` (build, insert, delete, exchange, foundations), `layers`, `layer_sets` (named layer sets
and saved plan views), `dimension`, `cad`,
`camera`, `symbols`, `groups`, `history` (whole-project snapshot undo, 100 steps), `defaults`
(`PlanDefaults` and the Chief X18 Daniel template), `units` (feet-inches formatting and parsing, metric)
and `export::dxf` (ASCII DXF R12). It is the foundation; keep it free of GUI code and add a unit test
with every geometry change.

**plan-3d.** Turns a `Project` into renderable triangle meshes (`build_scene`, or `build_scene_with` and
`SceneOptions` for open doors, casing and lite grids) and exports glTF 2.0 (`gltf::export_gltf`, `write_gltf_files`):
walls with openings and wall-type materials, half, pony and railing walls, door leaves and window units in every
opening style, and floor and ceiling slabs. It includes an ear-clipping triangulator and the `Material` enum other
crates' meshes use. Output is plain vertex and index buffers: X right, Y up, Z = -plan y, UVs in feet.

### Building elements

**plan-roof.** Automatic roofs: `build_roof(footprint, edges, baseline)` computes a weighted straight skeleton
and returns watertight `RoofPlane`s (hip, gable and shed edges, per-edge pitch and overhang);
`footprint_from_walls` traces the outer boundary. It falls back to uniform pitch or a bounding-box hip roof
when the exact solution fails and sets `Roof::approximate`. No fascia, gutters or Dutch gables.

**plan-stairs.** The parametric stair engine: `solve` rounds the rise to whole risers and checks IRC R311.7;
`Stair` and `StairShape` (straight, L, U, winder, ramp); `plan_symbol` (outline, riser lines, UP arrow, break
line), `meshes` (treads, risers, stringers, landings, handrail), `footprint`, `top_point`, plus rail, newel and
baluster geometry for stairs and deck edges that no tool uses yet.

**plan-cabinets.** The parametric cabinet engine: `Cabinet` (kind, countertop, backsplash, toe kick), `FaceLayout`
(the face-item tree that `resolve()` turns into rectangles), `plan_symbol`, `meshes`, `auto_label` (`B24`,
`W3030`) and `run_along_wall`.

**plan-electrical.** Devices (18 kinds), plan symbols, `place_on_wall` and `place_free`, `auto_place_outlets`
(6' rule, door-jamb clearance, GFCI rules), `auto_place_room_light` and `auto_place_switch`, connections as
dashed arcs, `circuits` and `assign_circuits`, a schedule and legend, and 3D stand-in meshes.

**plan-terrain.** Chief-style terrain: `Terrain` (perimeter, elevation points/lines/regions, modifiers,
features, road strips), `build_terrain` (grid, inverse-distance interpolation, Bowyer-Watson Delaunay, clipping,
smoothing), `elevation_at`, `contours` (marching triangles), `terrain_mesh` and `road_meshes`, `plan_symbols`,
and `auto_hole_for_building`.

**plan-framing.** Chief's Build Framing as a library: `frame_wall` (plates, studs at 16" on center, king and
trimmer studs, plied headers, cripples, sills), `frame_floor` (joists, rim, blocking), `wall_detail` (the 2D
framing elevation), `takeoff` (counts, board feet, linear feet) and `FramingDefaults`. The editor calls it from
Build > Framing (`editor/framing_view.rs`, chapter 11.11); no corner or T backing, no combined headers.

### Documents and output

**plan-docs.** Construction-document outputs: `schedule` (door, window, room and wall schedules to CSV or
Markdown), `materials` (framing, drywall, sheathing, siding, flooring and openings take-off), and a minimal
dependency-free PDF 1.4 writer (`PdfDoc`) with `plan_sheet`, a scaled plan sheet with a title block.

**plan-layout.** Chief's Layout, headless: `Layout`, `LayoutPage`, `LayoutBox`, `BoxSource`, title block
templates with macros, `send_to_layout` (shelf packing), `default_construction_set`, and `render_pdf`.

**plan-elevation.** Hidden-line vector drawings from a `plan-3d` scene: elevations, cross sections and the plan
overhead, as weighted `Line2` lists with `to_cad` and `svg`. Accuracy is about one pixel of the depth buffer;
no hatching, curves or text.

**plan-import.** Brings outside drawings in: an ASCII DXF reader, unit conversion, `to_cad_objects` and
`cad_to_walls` (parallel-line pairing). The editor calls it from File > Import and CAD > CAD to Walls
(`dialogs/exchange.rs`, chapter 12.4). No DWG, no binary DXF.

### Views and rendering

**plan-view3d.** An egui widget (`Viewport3d`) that draws a `plan_3d::Scene` with OpenGL through eframe's glow backend:
orbit, doll house, full camera (66" eye), elevation and plan-overhead cameras, opaque-then-translucent draw order,
an edge overlay. Camera math and edge extraction are plain Rust and unit tested; all GL is in a private module.

**plan-render.** A dependency-free CPU path tracer: BVH, Lambert plus GGX, glass, soft sun shadows, sky, Russian
roulette, tone mapping, optional denoise, deterministic multithreaded output and its own PNG writer.

**plan-materials.** The material system as data: 45 materials, 2D hatch patterns, procedural textures, default
assignments, the nine rendering techniques, and sun position from date, time and latitude.

### Library, checks, planning

**plan-library.** The catalog system: JSON catalog format, in-memory index with ranked search and a category
tree, and the built-in 2D symbol catalogs (starter, plants, bath and kitchen, lighting and electrical, furniture and
exterior).

**plan-check.** Chief's Tools > Checks as a rule engine: twelve IRC-based rules, `Finding`s with severity, location,
rule text and fix, `plan_footprint`, `report_markdown`; limits in `CheckOptions`.

**plan-spaceplan.** The Space Planning Assistant: `Questionnaire`, `generate_boxes` (greedy affinity packer),
`bump`, `validate`, `build_house` and `plan_symbols`.

### Reading Chief's files (read-only)

**plan-calib.** Read-only access to `.calib` and `.calibz` catalogs: its own SQLite reader, inflate and zip code,
`ChiefCatalog`, `ChiefRegistry`, `ChiefLibrary::discover/search`, a `decode` module that recovers object sizes and
plan-view symbols from Chief's binary blobs (reverse-engineered, with measured coverage in its README), and a
bridge to `plan-library` items.

**plan-chiefplan.** Read-only reader for `.plan` and `.layout` templates: scan, classify, decode per-layer color, line
weight and flags, and seed `PlanDefaults` (wall types, layers, layer sets, text styles, dimension sets). The editor uses it
for File > Templates > Import Chief Template....

**plan-config.** Chief's user configuration: `UserHotkeys.xml` (208 of 2,284 commands carry keys in Daniel's file),
the `.toolbar` files and the preferences INI, with Daniel's files embedded at compile time.

### The application

**plan-app.** The desktop editor (binary `plan-studio`). Layout of `src/`:

```
main.rs         window, menu/toolbar/panel wiring, file operations, dialogs host
toolbar.rs      the three bars, flyout tables, BINDINGS, Action enum
menus.rs        the menu bar (Build and CAD menus generated from the flyout tables)
theme.rs        canvas themes, UI brightness, ~/.plan-studio/settings.json
plan_defaults.rs  defaults loading and the helpers that turn defaults into objects
icons.rs        embedded SVG icons
tools/          one module per Chief tool behind the Tool trait (select, wall, opening, pan,
                dimension, text, cad, cabinet, stairs, roof, electrical, library, camera, terrain)
editor/         services shared by tools: EditorContext, selection, snap engine, handles,
                temporary dimensions, undo history, plan rendering, wall connections, edit actions
                and their dispatch, and per-object views (roof_view, stairs_view, site_view, placed,
                rooms_edit, framing_view); restyle (View > Color and Line Weights) and sheet (the
                drawing sheet)
shell/          docks, library browser, the 3D panel, the runtime hotkey map, and spec_dialogs (the
                one place that maps an object kind to its specification dialog)
dialogs/        Chief-style specification dialogs on the shared frame, Default Settings and its lists
                (default_lists: dimension sets, room types, text styles), Customize Hotkeys, Layer
                Display Options, file exchange and framing windows (exchange), the Build and Tools windows
```

## 14.3 How data flows in the editor

- `EditorContext` (in `editor/mod.rs`) owns the `Project`, the active floor, the selection, snap
  settings, defaults, view flags, the status text, the clipboard, temporary dimensions, session-only extras
  and cached derived data (`rooms`, `outlines`, `layer_outlines`).
- A tool changes the model like this: `cx.begin_change("Move Wall")` (snapshots the project for undo), mutate
  `cx.project`, `cx.mark_dirty()`, and return `ToolResult::committed("Move Wall")`. The shell calls `cx.refresh()`
  once a frame, which recomputes rooms and outlines when dirty.
- Pointer events arrive as `PointerEvent { world, snapped, snap, screen, modifiers, button, down, drag_delta }`
  already mapped to world inches; key events as `KeyEvent`. Activation hotkeys are never read by tools; they
  come from the runtime `HotkeyMap` (chapter 13).
- Objects are addressed by `ObjectRef` (Wall, Opening, Dimension, Cad, Cabinet, Stair, RoofPlane, Symbol,
  Camera, Text, Room, Device, Terrain). A second, smaller `plan_core::ObjectRef` (Wall, Opening, Dimension, Cad,
  Symbol, Camera) is used by object groups and the core clipboard; the two types are not the same, so convert
  deliberately.
- Things the model has no field for yet go in `SessionExtras` (per session), in the opaque `cabinets` and
  `stairs` JSON slots, or as hidden data records on reserved layers (roofs, electrical, terrain). Each of these
  is marked `TODO: plan-core field` in the code.

## 14.4 How to add a tool

Example: a hypothetical "Soffit Line" tool. Follow the same steps for any Chief tool.

1. **Read the spec.** Find the tool's section in `docs/parity/*.md` and its flyout in
   `docs/chief-x18-subtools.md`. Note the ids you will implement (for example `CB-17`).
2. **Create the module.** `crates/plan-app/src/tools/soffit_line.rs` with a struct that implements `Tool`:
   - `id()` returns the `ToolId`; `name()` returns Chief's name (for example "Soffit"); `hint()` is the status-bar
     text.
   - `activate`/`deactivate` reset state; `pointer_down/move/up`, `double_click` and `key` drive a state machine;
     `draw_overlay` draws the ghost; `edit_toolbar` returns the Edit toolbar buttons for the selection.
   - `frame()` runs once per frame if the tool owns dialogs or palettes. `set_variant()` receives the
     `ToolId::...Variant(..)` payload when a flyout entry (or its hotkey) is picked; the tool object
     is shared by all variants of its family, so state survives a switch.
3. **Register it.** Add `pub mod soffit_line;` and one line in `registry()` in `tools/mod.rs`. If the tool has
   flyout variants, add a `ToolId::SoffitVariant(Mode)` and map it in `ToolId::base()`.
4. **Put it on the toolbar.** In `toolbar.rs`, replace the dimmed `todo("icon", "Name")` entry with
   `item("icon", "Name", Action::SetTool(ToolId::...))` (and `with_hotkey(...)` for the tooltip). The Build and CAD
   menus and the command list for the hotkey dialog pick it up automatically, because they are generated from
   the same flyout tables. Add an SVG to the icon set in `icons.rs` if none fits.
5. **Hotkeys.** Daniel's Chief bindings map by command name, so a tool whose name matches (for example
   "Soffit" -> `T`) is live as soon as the entry is no longer `NotImplemented`. For a Plan Studio default
   binding add a `bind(...)` row to `toolbar::BINDINGS`.
6. **Change the model safely.** `cx.begin_change(label)` before mutating, `cx.mark_dirty()` after. One user
   gesture is one undo step. Respect locked layers with `cx.check_unlocked(...)`. Use `cx.snap_at(...)` for snapping
   and `cx.fmt_dim(...)` for lengths.
7. **Dialog.** Implement `SpecPages` in `dialogs/<name>.rs` (tab list built with `on("Name")` and `off("Name")`,
   `page(ui, tab)`, `preview(painter, rect)`, `error()`), wrap it in `SpecDialog::new("Title", key)`, and add one
   arm to `SpecDialogs::open` in `shell/spec_dialogs.rs` so double-click and `Enter` open it for your `ObjectRef`. Controls the model cannot store yet are drawn with `dis_check`, `dis_radio` or
   `session_check`.
8. **Tests.** Drive the tool with synthetic `PointerEvent`s on a fresh `EditorContext::new(plan_defaults::embedded())`
   and assert on the `Project`; no GUI is needed. The registry test checks every `ToolId` is registered once.
9. **Document.** Update the matching chapter of this manual (remove "(planned)"), tick the parity doc if it has a gap
   table, and cite the ids in doc comments.

Engine first, editor second is the usual order: write the algorithm in its own crate with tests (as `plan-roof`,
`plan-stairs`, `plan-terrain` did), then add the tool that calls it.

## 14.5 Adding a rule, a catalog or a format

- **A Plan Check rule**: write one function in `plan-check/src/rules.rs` with a doc comment naming its IRC section,
  call it from `plan_check`, and test it.
- **A library catalog**: write a JSON file in the catalog format (`plan-library` README) and load it with
  `Catalog::from_json`, or add a module `catalog_*.rs` next to the starter ones. Ids are `core.<group>.<name>` and
  must be unique; symbols have at most 40 strokes and their bounds must match width and depth within 1".
- **A file format**: put the reader or writer in a library crate with `std` only if you can, take and return strings or
  bytes, and test round trips. The editor then calls it from a menu action.

## 14.6 Testing and quality gates

```bash
cargo test -p plan-core                  # one crate
cargo test --workspace                   # everything
cargo test -p plan-app                   # editor state machines, dialogs, hotkeys (no window needed)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

- Every geometry change in `plan-core` gets a unit test. Tool tests use synthetic events; dialog tests drive the
  pure form logic (`WallForm::set_length`, `LayerTable`, `FaceLayout` commands) rather than egui drawing.
- Tests that need a real Chief install (`plan-calib`, `plan-chiefplan`) are `#[ignore]`; run them with
  `cargo test -p plan-calib --release -- --ignored --nocapture --test-threads=1`. Everything else uses synthetic
  fixtures built at test time (some build SQLite and zip files with the `sqlite3` and `zip` command-line tools and
  skip if the tool is missing).
- Generated documents are checked: `docs/chief-hotkeys-resolved.md` is regenerated with
  `cargo run -p plan-config --example gen_hotkeys_md > docs/chief-hotkeys-resolved.md`, and a test fails when it is
  stale.
- CI (`.github/workflows/ci.yml`) runs `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` on Linux, macOS and Windows for every push to `main` and every pull request. The Linux job
  installs the GUI build packages (GTK 3, xkbcommon, Wayland, xcb, Mesa GL). `ROADMAP.md` still shows CI as unchecked.
- Keep `plan-core` free of GUI dependencies and Chief content out of the repository (see 12.7).

## 14.7 Where the decisions are

`DECISIONS.md` records open decisions (license, app name, Chief catalog handling) and the queued work.
`ROADMAP.md` is the feature inventory by phase. `docs/parity/` holds the per-area behavior specs with stable ids.
`docs/chief-x18-*.md` are the captures of Chief's UI the program is built to.
