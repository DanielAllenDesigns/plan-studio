# Chapter 12: Import and Export

What Plan Studio can read and write today, how each format is handled, and what is
engine-only (written and tested in a crate, but with no menu command yet). The File >
Export, Import and Print submenus are dimmed in the current build, so several formats
below are reached from Rust code or the command line rather than from the menus.

## 12.1 Formats at a glance

| Format | Direction | In the editor | Where it lives |
|---|---|---|---|
| `.psplan` (Plan Studio JSON) | Read and write | Yes: File > Open Plan, Save, Save As | `plan-core` |
| glTF 2.0 (`.gltf` + `.bin`) | Write | Yes: 3D > Export > glTF... | `plan-3d` |
| PDF (construction set) | Write | Yes: Tools > Schedules > Create Construction Set... | `plan-layout`, `plan-docs` |
| PNG (ray-traced stills) | Write | Yes: Ray Trace > Save PNG... | `plan-render` |
| CSV (schedules, materials list) | Write | Yes: Export CSV... buttons | `plan-docs` |
| Markdown (Plan Check report) | Write | Yes: Save Report... | `plan-check` |
| DXF (ASCII, R12) | Write | (planned; engine in `plan-core`) | `plan-core::export::dxf` |
| DXF import, CAD to Walls | Read | (planned; engine in `plan-import`) | `plan-import` |
| DWG | Neither | (planned) | |
| Chief catalogs `.calib`, `.calibz` | Read | (planned; engine in `plan-calib`) | `plan-calib` |
| Chief templates `.plan`, `.layout` | Read names and some values | (planned; engine in `plan-chiefplan`) | `plan-chiefplan` |
| Chief hotkeys, toolbars, preferences | Read | Hotkeys only (chapter 13) | `plan-config` |
| IFC, SketchUp, Revit, OBJ | Neither | (planned) | |

### Tools: the commands that read and write files

| Menu command | Hotkey | Does | Today |
|---|---|---|---|
| File > Open Plan... | `Cmd+O` | Opens a `.psplan`. | Works. |
| File > Save, Save As... | `Cmd+S` | Writes a `.psplan`. | Works. |
| File > Export > (DXF, PDF, Image) | | Chief's export submenu. | (planned) dimmed |
| File > Import > (DXF/DWG, image underlay) | | Chief's import submenu. | (planned) dimmed |
| File > Print > Print... | `Cmd+P` | Prints the active view. | (planned) dimmed |
| 3D > Export > glTF... | | Writes `.gltf` and `.bin`. | Works. |
| Tools > Schedules > Create Construction Set... | | Writes a PDF set. | Works. |
| Door/Window/Room/Wall Schedule > Export CSV... | | Writes a CSV. | Works. |
| Materials List... > Export CSV... | | Writes a CSV. | Works. |
| Checks window > Save Report... | | Writes Markdown. | Works. |
| Ray Trace... > Save PNG... | | Writes a PNG. | Works. |
| Library > Import Library (.calib, .calibz)... | | Adds a Chief catalog. | (planned) dimmed |
| CAD > CAD to Walls... | | Converts lines to walls. | (planned) dimmed |

## 12.2 Plan files (`.psplan`)

A plan is one JSON document: a `Project` with its name, floors (each with walls, openings,
dimensions, CAD objects, room names, symbols, cabinets, stairs and groups), the layer set,
camera objects and the wall types stored in the plan. Lengths are inches.

- File > **Open Plan...** and **Save As...** show a file dialog filtered to `.psplan`. Save As
  adds the extension if you leave it off. **Save** writes to the current file, or asks for one.
- Newer fields all have defaults, so older files load cleanly. Opening a file resets the undo
  history (open is not undoable).
- Objects that do not yet have their own field are stored as hidden, locked data on reserved
  layers so they save and undo with the plan: roof planes (`Roof Planes, Data`), electrical
  devices (`Electrical, Data`) and the terrain (`Terrain, Data`). Cabinets and stairs ride in
  opaque JSON slots on the floor. A planned clean-up moves these into real fields.
- Per-session settings (the "session only" fields in the dialogs) are not in the file.
- The file is readable and diff-friendly; keep it under version control if you like.

Autosave, backups and an Open Recent list are (planned).

## 12.3 DXF export (planned in the editor; engine ready)

`plan_core::write_dxf(project, floor, rooms)` returns an **ASCII DXF, release 12 (AC1009)**
string for one floor. Units are inches (`$INSUNITS` 1). It writes:

| Plan object | DXF entities |
|---|---|
| Walls | One closed POLYLINE per wall, using the joined (mitered) outlines, on the wall's layer (`Walls, Normal` ...) |
| Openings | Two jamb LINEs; a door adds its leaf LINE, a swing ARC and the closed-leaf LINE; a window adds a glass LINE. On layers `Doors` and `Windows` |
| Dimensions | Extension LINEs, the dimension LINE and a TEXT for the value, on `Dimensions, Manual` or `Dimensions, Automatic` (exploded; there is no DIMENSION entity, so every program shows them the same) |
| CAD objects | LINE, ARC, CIRCLE, POLYLINE and TEXT on their layers |
| Room labels | TEXT for the name and the area |

The layer table carries every plan layer with an AutoCAD color index, negative when the layer is
hidden. Roof planes show up as the closed outline polylines on `Roof Planes`.

To use it before a menu command exists, call it from a small Rust program or test:

```rust
let rooms = plan_core::detect_rooms(&project.floors[0].walls, 0.5);
let dxf = plan_core::write_dxf(&project, 0, &rooms);
std::fs::write("first-floor.dxf", dxf)?;
```

## 12.4 DXF import and CAD to Walls (planned in the editor; engine ready)

`plan-import` is the counterpart of Chief's Import Drawing and CAD to Walls.

- `parse_dxf(text)` reads **ASCII DXF** tolerantly (CRLF or LF, padded or bare group codes) into a
  `DxfDrawing` with layers, units, extents, blocks and entities. Supported entities: LINE,
  LWPOLYLINE, POLYLINE/VERTEX, CIRCLE, ARC, TEXT, MTEXT and INSERT. Anything else is counted in
  `skipped`.
- `DxfDrawing::explode_inserts` expands block references with scale, rotation, base point and nesting.
- `to_inches_factor(units, override)` and `to_cad_objects(drawing, factor, layer_prefix)` turn
  entities into plan CAD objects in inches; bulged polyline segments become sampled arcs.
- `cad_to_walls(lines, options)` pairs **parallel lines** into wall proposals, measures the
  thickness, and closes corners and T-junctions; `apply_walls` and `apply_cad` add the results to a
  `Project` with fresh ids.
- **DWG and binary DXF are not supported.** Convert to ASCII DXF first. A file that is not ASCII DXF
  fails with "not an ASCII DXF file" or "binary DXF files are not supported".
- The reader round-trips what `write_dxf` writes.

The planned menu commands are File > Import > DXF/DWG... and CAD > CAD to Walls....

## 12.5 glTF export (works)

3D > Export > **glTF...** asks for a file name and writes `<name>.gltf` (JSON) and `<name>.bin`
(buffers), glTF 2.0, of the same scene the 3D view shows (walls with openings, doors, windows,
slabs and roof planes). Coordinates: X right, Y up, Z = negative plan y, inches; UVs are in feet.
The status bar reports "Exported <path>.gltf" or the error; "Nothing to export: the 3D model is
empty" if the plan has no geometry. Open the file in Blender, a glTF viewer or a web viewer.

## 12.6 PDF, PNG, CSV and Markdown (work)

- **PDF**: Tools > Schedules > Create Construction Set... (chapter 11.4).
- **PNG**: the Ray Trace window's Save PNG... (chapter 10.6), uncompressed 8-bit RGBA.
- **CSV**: every schedule window and the Materials List window have Export CSV.... Fields are
  quoted as needed; open them in any spreadsheet.
- **Markdown**: the Plan Check window's Save Report... writes findings grouped by severity.

## 12.7 Chief catalogs (`.calib`, `.calibz`), planned in the editor

`plan-calib` reads the user's own Chief library catalogs in place. The Library menu's Import Library
(.calib, .calibz)... is dimmed, and the Library Browser shows a "Chief catalogs" node that says "coming".

What the engine does:

- A `.calib` file is a **SQLite 3 database**. A `.calibz` is a zip (deflate) of textures plus one
  `.calib`. The crate has its own minimal read-only SQLite reader and its own inflate and zip code, so
  it uses only `std` and `serde_json`, and it reads multi-gigabyte files with bounded memory (one page at a
  time plus a small cache).
- `Chief Library.json` is the registry listing the installed catalogs by UUID (category 1 Core, 2
  Manufacturer, 4 Bonus). `ChiefLibrary::discover()` reads it, caches the path index in
  `~/.plan-studio/chief-catalog-index.json` (file paths, sizes, modification times and catalog UUIDs only),
  and finds catalogs such as `CoreArchitectural.calib` in the Core, Bonus and Manufacturer folders of the
  Chief install (`/Library/Application Support/Chief Architect Premier X18/` on macOS).
- `ChiefCatalog::open(path)` gives the catalog name, the category tree (`Tags`), objects (lazily, touching
  only the leading columns of each row), thumbnails (PNG blobs) and keywords. `ChiefLibrary::search("shaker
  door", 50)` streams catalog by catalog and stops at the limit.
- `to_plan_library(&catalog, limit)` converts objects into `plan_library::CatalogItem`s with id
  `chief.<catalog-uuid>.<object-id>`, the category path, keywords as tags and free-standing placement. The
  **size** comes from the object's blobs (a `w d h` record followed by three 1.0 scale factors) and the **plan
  symbol** from the object's geometry: Chief stores no separate 2D symbol, so the `decode` module projects the 3D
  triangle meshes (or the polygon face stream in `LibrarySymbolData.symDxf`) straight down with hidden-line
  removal, rotates them so the front faces +Y, and keeps the result only when its footprint is within 10% of the
  decoded size. Otherwise the item gets a **placeholder symbol** (a rectangle, an X and the first letter of the
  name). Items with no size record are 24 x 24 x 24 in and tagged `size-unknown`.
- Measured on a local X18 install (Core Architectural, Interiors and MEP, 3,295 objects, per the crate README at
  the time of writing): size decoded for about 89%, a plan symbol for about 89%, and symbol and size agreeing
  within 10% for about 74% of the objects that have both. The decoders are still being refined, so check the
  README for current figures.

What is not decoded yet (so some items stay placeholders):

- About a quarter of the geometry is partial: the decoded meshes cover only part of an object (a chair's seat
  but not its legs). Parametric solids and some wrapper records are not decoded, and the bridge falls back to
  the placeholder when the footprint disagrees with the size.
- Parametric cabinets have no geometry in the catalog (Chief generates them); door swing arcs and window
  symbols are absent from the blobs; windows with no size record fall back to mesh bounds.
- Elevation (floor to bottom) is always 0; plant spread and height are inferred, and a plant's plan symbol is a
  canopy circle where Chief draws a textured image.
- 3D models for the editor (the meshes are decoded only to project a plan view), `LibraryObjects.Type`,
  texture link tables and `Content/<hash>-01` image resolution.

Opening a `.calibz` writes its embedded `.calib` to `<temp dir>/plan-studio/` as a cache, a derived copy
of your own file outside the repository; delete it any time. Details: `docs/chief-library-format.md`.

### Licensing rule

Chief catalogs, their thumbnails and textures are Chief Architect and manufacturer licensed content.
Plan Studio **reads them in place** from your installation; it never ships, copies or commits any of it.
`.gitignore` excludes `*.calib`, `*.calibz`, `*.calib_error`, `*.plan` and `*.layout`. Tests use synthetic
fixtures built at test time; tests against a real install are marked `#[ignore]`. This is decision 3 in
`DECISIONS.md`.

## 12.8 Chief templates (`.plan`, `.layout`), planned in the editor

`plan-chiefplan` is a **read-only** scanner for Chief template files. It lists the names stored in a template (layer sets,
layers, text styles, dimension defaults, wall types, saved plan views, sheet sizes, layout pages, schedule
names) and decodes per-layer color, line weight and display/lock flags where the record layout is understood. It
`seed_defaults` can turn an inventory into Plan Studio defaults. It never writes or copies a template, and it
redacts client-specific strings (project file names, network paths, address-like text) before any inventory is
written. Daniel's 26 templates are inventoried in `docs/daniel-template-inventory.md`: for example the working
template `x17 Working Template 2025-08-20.plan` has 34 layer sets, 356 layers, 12 text styles, 14 dimension
default sets, 108 wall types and 20 saved plan views.

## 12.9 Chief hotkeys, toolbars and preferences

`plan-config` reads Chief's `UserHotkeys.xml`, the `.toolbar` files and the preferences INI. The app uses the
hotkeys (chapter 13). Daniel's own files are embedded at compile time from `docs/chief-config-raw/`; reading
the live Chief INI at startup and rebuilding the toolbars from his four toolbar sets (Default, Extended Tool,
Space Planning, Terrain) are (planned).

## 12.10 Notes

- Opening or exporting never touches your Chief files. Everything is read-only against Chief data.
- Large exports (glTF of a big model, 1920 x 1080 ray traces at 1024 samples) are slow in a debug build; use
  `cargo run --release -p plan-app`.
