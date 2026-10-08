# Chapter 12: Import and Export

What Plan Studio can read and write today, how each format is handled, and what is
engine-only (written and tested in a crate, but with no menu command yet). File > Export
(DXF, Elevation DXF, Construction Set PDF, glTF), File > Import (Import Drawing (DXF)), File > Templates >
Import Chief Template, File > Print (Print Layout, Export Layout PDF) and CAD > CAD to Walls work; the
Library menu's catalog import is still dimmed. Chief catalogs are read in place by the Library Browser (chapter 6.6), not imported.

## 12.1 Formats at a glance

| Format | Direction | In the editor | Where it lives |
|---|---|---|---|
| `.psplan` (Plan Studio JSON) | Read and write | Yes: File > Open Plan, Save, Save As | `plan-core` |
| glTF 2.0 (`.gltf` + `.bin`) | Write | Yes: 3D > Export > glTF... | `plan-3d` |
| PDF (construction set) | Write | Yes: Tools > Schedules > Create Construction Set... | `plan-layout`, `plan-docs` |
| PDF (the plan's layout) | Write | Yes: File > Print > Print Layout... and Export Layout PDF... (chapter 11.3) | `plan-layout`, `plan-docs` |
| PNG (ray-traced stills) | Write | Yes: Ray Trace > Save PNG... | `plan-render` |
| CSV (schedules, materials list) | Write | Yes: Export CSV... buttons | `plan-docs` |
| Markdown (Plan Check report) | Write | Yes: Save Report... | `plan-check` |
| DXF (ASCII, R12) | Write | Yes: File > Export > DXF..., Elevation DXF... | `plan-core::export::dxf`, `plan-elevation` |
| DXF import, CAD to Walls | Read | Yes: File > Import > Import Drawing (DXF)..., CAD > CAD to Walls... | `plan-import` |
| DWG | Neither | (planned) | |
| Chief catalogs `.calib`, `.calibz` | Read in place | Yes: the Library Browser's Chief nodes (chapter 6.6); no import command | `plan-calib` |
| Chief templates `.plan`, `.tpl`, `.layout` | Read names and some values | Yes: File > Templates > Import Chief Template... | `plan-chiefplan` |
| Chief hotkeys, toolbars, preferences | Read | Hotkeys (chapter 13), and the default plan and layout template names from the preferences INI (chapter 1.7.1) | `plan-config` |
| IFC, SketchUp, Revit, OBJ | Neither | (planned) | |

### Tools: the commands that read and write files

| Menu command | Hotkey | Does | Today |
|---|---|---|---|
| File > Open Plan... | `Cmd+O` | Opens a `.psplan`. | Works. |
| File > Save, Save As... | `Cmd+S` | Writes a `.psplan`. | Works. |
| File > Export > DXF... | | Writes the active floor as an ASCII R12 DXF (12.3). | Works. |
| File > Export > Elevation DXF... (also 3D > Create Orthographic View > Export Elevations (DXF)...) | | Writes the four elevations as one DXF (12.3). | Works. |
| File > Export > Construction Set PDF... | | The same as Create Construction Set. | Works. |
| File > Export > glTF... | | The same as 3D > Export > glTF.... | Works. |
| File > Export > (image, DWG) | | Not in the menu. | (planned) |
| File > Import > Import Drawing (DXF)... | | Adds a DXF drawing to the active floor as CAD objects (12.4). | Works. |
| File > Import > (DWG, image underlay) | | Not in the menu. | (planned) |
| File > Templates > Import Chief Template... | | Seeds your defaults from a Chief `.plan`, `.tpl` or `.layout` (12.8). | Works. |
| File > New Layout | | Makes the plan's layout and shows the layout view (11.3). | Works. |
| File > Open Layout... | | Shows the layout view. | Works. |
| File > Print > Print Layout... | | Saves the layout's printed pages (all, or a range) as a PDF (11.3). | Works. |
| File > Print > Export Layout PDF... | | Saves every printed page as a PDF. | Works. |
| Row 1 Print button | `Cmd+P` | Prints the active view. | (planned) dimmed |
| 3D > Export > glTF... | | Writes `.gltf` and `.bin`. | Works. |
| Tools > Schedules > Create Construction Set... | | Writes a PDF set. | Works. |
| Door/Window/Room/Wall Schedule > Export CSV... | | Writes a CSV. | Works. |
| Schedule Specification > Export CSV... (a schedule placed in the plan) | | Writes a CSV of the placed schedule (11.2). | Works. |
| Materials List... > Export CSV... | | Writes a CSV. | Works. |
| Checks window > Save Report... | | Writes Markdown. | Works. |
| Ray Trace... > Save PNG... | | Writes a PNG. | Works. |
| Library > Import Library (.calib, .calibz)... | | Adds a Chief catalog. | (planned) dimmed |
| CAD > CAD to Walls... | | Converts pairs of parallel CAD lines to walls (12.4). | Works. |

## 12.2 Plan files (`.psplan`)

A plan is one JSON document: a `Project` with its name, floors (each with walls, openings,
dimensions, CAD objects, room names, symbols, cabinets, stairs, groups, roofs, electrical devices,
framing and slab objects, exterior details and placed schedules), the layer set, camera objects, the lights, the terrain, the Project Information, the
layout and the wall types stored in the plan. Lengths are inches.

- File > **Open Plan...** and **Save As...** show a file dialog filtered to `.psplan`. Save As
  adds the extension if you leave it off. **Save** writes to the current file, or asks for one.
- Newer fields all have defaults, so older files load cleanly. Opening a file resets the undo
  history (open is not undoable).
- Objects owned by an engine crate ride in **typed slots**: opaque JSON the editor reads and writes through
  typed accessors in `plan-core`, saved and undone with the plan.

  | Slot | Holds |
  |---|---|
  | `Floor.cabinets`, `Floor.stairs` | Cabinets, and stairs, ramps and landings with their plan-only settings (chapter 7) |
  | `Floor.roofs` | Roof planes, the Build Roof settings, ceiling planes and dormers (chapter 8) |
  | `Floor.electrical` | The floor's electrical devices and connections (chapter 9) |
  | `Floor.framing` | Built framing members, manual members and the framing layout lines (chapter 11.11) |
  | `Floor.foundation` | Slabs, slab holes, pads, piers and platform holes (chapter 16) |
  | `Floor.details` | Corner boards, quoins, moldings, floor and wall material regions, wall hatches, polygon decks and 3D solids (chapter 17) |
  | `Floor.schedules` | The schedules placed in the plan: kind, columns, sort, filter, labels, style, position (chapter 11.2) |
  | `Floor.cad_attrs`, `Floor.cad_blocks` | Own style (color, weight, dash, fill, arrows, rich text) of CAD objects, and the names and insertion points of CAD blocks (chapter 5.5). Typed lists, not JSON |
  | `Project.text_macros`, `Project.note_types` | The plan's user text macros and note types (chapter 5.3). Typed values |
  | `Project.terrain` | The terrain, its contour interval, whether it is built, and the terrain walls, breaks and landscape objects (chapter 9) |
  | `Project.layout` | The plan's one layout: pages, boxes, title block and page setup (chapter 11.3) |
  | `Project.info` | Project Information: client, designer, job number, date, revisions, custom fields (chapter 11.4) |
  | `Project.lights`, `Project.light_options` | The lights Add Lights places, and whether electrical light fixtures emit light (chapter 10.13) |

  Typed, serde-default **extras** hold what dialogs used to keep per session: a wall's label switch, specified label text and last
  picked wall type; an opening's style name, thickness, swing angle, jamb or frame width and Show Open in 2D; a room's
  conditioned setting, stem wall height, base and crown moldings, fill and label options. A wall also stores its class and
  curve, a camera object its section line (with the back clip) and its elevation rendering options (hatch, shadows, sun, line weight by distance, labels).
- **Migration.** Files from before these slots kept the roofs, electrical devices, terrain and slabs as hidden, locked text records on
  reserved layers (`Roof Planes, Data`, `Electrical, Data`, `Terrain, Data`, `Foundation, Data`, `Lights, Data` for lights, and, until Round 8, `CAD, Data` for CAD styles, CAD blocks, text macros and note types). When you open such a file the editor moves each
  record into its slot and deletes the legacy items and the hidden layers; it happens as the file loads, so it is not an undo step. A slot that is already filled
  is left alone, every new field has a default, and old files otherwise load unchanged.
- Per-session settings (the "session only" fields that remain in the dialogs) are not in the file.
- The file is readable and diff-friendly; keep it under version control if you like.

Autosave, backups and an Open Recent list are (planned).

## 12.3 DXF export (works)

**File > Export > DXF...** asks for a file name (default `<Project Name> - <Floor Name>.dxf`) and writes the
**active floor** as an **ASCII DXF, release 12 (AC1009)** (`plan_core::write_dxf(project, floor, rooms)`). The
status bar says "Saved <path>", or "Export cancelled" if you dismiss the file dialog. Units are inches
(`$INSUNITS` 1). It writes:

| Plan object | DXF entities |
|---|---|
| Walls | One closed POLYLINE per wall, using the joined (mitered) outlines, on the wall's layer (`Walls, Normal` ...) |
| Openings | Two jamb LINEs; a door adds its leaf LINE, a swing ARC and the closed-leaf LINE; a window adds a glass LINE. On layers `Doors` and `Windows` |
| Dimensions | Extension LINEs, the dimension LINE and a TEXT for the value, on `Dimensions, Manual` or `Dimensions, Automatic` (exploded; there is no DIMENSION entity, so every program shows them the same) |
| CAD objects | LINE, ARC, CIRCLE, POLYLINE and TEXT on their layers |
| Room labels | TEXT for the name and the area |

The layer table carries every plan layer with an AutoCAD color index, negative when the layer is
hidden. It also writes:

| Plan object | DXF entities |
|---|---|
| Roof planes | The closed plan outline of every roof plane stored on the floor, on layer `Roof Planes` |
| Framing placed by hand | The plan outline of each manual or layout-built member and each post footing, and each truss base, as closed polylines, and the Joist Direction, Roof Truss Direction and Bearing lines as open polylines, all on layer `Framing` |

The members Build Framing makes from the walls, floors and roof are not written, and slabs, pads and piers are not written either.

**File > Export > Elevation DXF...** (the same command is 3D > Create Orthographic View > Export Elevations
(DXF)...) writes the four exterior elevations (Front, Back, Left, Right) as the hidden-line drawings of
`plan-elevation`, side by side with a 120" gap and a caption under each ("Front Elevation" ...), into one DXF
(default `<Project Name> Elevations.dxf`). A side with nothing to draw is skipped; a plan with nothing to draw
says "There is nothing to draw an elevation of".

The same floor export from a small Rust program or test:

```rust
let rooms = plan_core::detect_rooms(&project.floors[0].walls, 0.5);
let dxf = plan_core::write_dxf(&project, 0, &rooms);
std::fs::write("first-floor.dxf", dxf)?;
```

## 12.4 DXF import and CAD to Walls (work)

`plan-import` is the counterpart of Chief's Import Drawing and CAD to Walls.

### File > Import > Import Drawing (DXF)...

Pick a `.dxf` file; the **Import Drawing (DXF)** window opens before anything is added.

- It shows the file name, "<n> entities on <m> layers; the file's units: <units>" and, when the reader
  skipped entity kinds it does not support, "Not imported: <count> <kind>, ...". A file that cannot be read
  ends with "Import failed: <reason>" in the status bar.
- **Units**: As the file says (default), Inches, Feet, Millimeters, Centimeters or Meters. A file with no
  units reads as inches. **Layer name prefix** is put in front of every imported layer name (blank by default).
  "Size in the plan" shows the drawing's declared extents in the current units, updating as you change Units.
- **Import** adds the drawing to the active floor as CAD objects, as one undo step ("Import Drawing"). Layers the
  plan does not have are created, hidden when the DXF layer was off. The status bar says "Imported n objects (k
  new layers)", or "The drawing had nothing to import".

### CAD > CAD to Walls...

Opens the **CAD to Walls** window ("Pairs of parallel lines become walls.") for the active floor. With no CAD
lines on the floor the status bar says "CAD to Walls: there are no CAD lines on this floor".

- **Lines from**: "Selected CAD lines (n)" or "All lines on layer" with a layer combo that lists each layer
  holding lines or polylines with its segment count. Polyline segments count as lines.
- **Options** (inches): Thinnest wall, Thickest wall, Shortest wall, Snap corners within, and Exterior from
  thickness (the thickness from which a proposed wall counts as exterior).
- **Preview** reads "<n> lines give <w> walls (<e> exterior, <i> interior); <p> line pieces left over." The
  preview refreshes when you change a setting.
- **Create <n> Walls** adds the proposed walls at the default height of their kind, as one undo step ("CAD to
  Walls"); the status bar says "Created n walls from CAD lines". The CAD lines stay where they are.

### The engine

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


## 12.5 glTF export (works)

3D > Export > **glTF...** asks for a file name and writes `<name>.gltf` (JSON) and `<name>.bin`
(buffers), glTF 2.0, of the same scene the 3D view shows (walls of every class with openings, doors,
windows, floor and ceiling platforms, slabs, pads and piers, roof planes with their holes, skylights, ceiling planes and dormers,
manual framing, and placed symbols). Coordinates: X right, Y up, Z = negative plan y, inches; UVs are in feet.
The status bar reports "Exported <path>.gltf" or the error; "Nothing to export: the 3D model is
empty" if the plan has no geometry. Open the file in Blender, a glTF viewer or a web viewer.

## 12.6 PDF, PNG, CSV and Markdown (work)

- **PDF**: Tools > Schedules > Create Construction Set... (chapter 11.5) writes the automatic set; File > Print > Print Layout... and
  Export Layout PDF... write the plan's own layout (chapter 11.3), and a vector elevation or section view has a Layout PDF... button (chapter 10.7).
  The PDF writer is the same dependency-free PDF 1.4 one for all of them; Print means "save a PDF", not a printer dialog.
- **PNG**: the Ray Trace window's Save PNG... (chapter 10.6), uncompressed 8-bit RGBA.
- **CSV**: every schedule window and the Materials List window have Export CSV.... Fields are
  quoted as needed; open them in any spreadsheet.
- **Markdown**: the Plan Check window's Save Report... writes findings grouped by severity.

## 12.7 Chief catalogs (`.calib`, `.calibz`)

`plan-calib` reads the user's own Chief library catalogs in place, and the Library Browser shows them (chapter 6.6). The Library menu's Import
Library (.calib, .calibz)... is dimmed: catalogs are read where Chief keeps them, never imported.

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
- Textures: the placed object's 3D mesh is drawn untextured (the editor fits the decoded meshes to the symbol, and shows a box when the
  geometry is partial; chapter 6.6). `LibraryObjects.Type`, texture link tables and `Content/<hash>-01` image resolution are not decoded.

Opening a `.calibz` writes its embedded `.calib` to `<temp dir>/plan-studio/` as a cache, a derived copy
of your own file outside the repository; delete it any time. Details: `docs/chief-library-format.md`.

### Licensing rule

Chief catalogs, their thumbnails and textures are Chief Architect and manufacturer licensed content.
Plan Studio **reads them in place** from your installation; it never ships, copies or commits any of it.
`.gitignore` excludes `*.calib`, `*.calibz`, `*.calib_error`, `*.plan` and `*.layout`. Tests use synthetic
fixtures built at test time; tests against a real install are marked `#[ignore]`. This is decision 3 in
`DECISIONS.md`.

## 12.8 Chief templates (`.plan`, `.tpl`, `.layout`)

**File > Templates > Import Chief Template...** picks a Chief `.plan`, `.tpl` or `.layout` file, seeds your
defaults from it (chapter 1.7) and saves the result as your template in `~/.plan-studio/defaults.json`. The status
bar reports "Imported <path>: n wall types, m layers added", or "Import failed: <reason>"; if your template could not
be saved it says so. Reset to Chief X18 Template undoes it. The engine behind it:

`plan-chiefplan` is a **read-only** scanner for Chief template files. It lists the names stored in a template (layer sets,
layers, text styles, dimension defaults, wall types, saved plan views, sheet sizes, layout pages, schedule
names) and decodes per-layer color, line weight and display/lock flags where the record layout is understood. It
`seed_defaults` turns an inventory into Plan Studio defaults. It never writes or copies a template, and it
redacts client-specific strings (project file names, network paths, address-like text) before any inventory is
written. Daniel's 26 templates are inventoried in `docs/daniel-template-inventory.md`: for example the working
template `x17 Working Template 2025-08-20.plan` has 34 layer sets, 356 layers, 20 saved plan views and (from the name scan) 108 wall types.

The reader also **decodes the template's objects** (the "object stream", `docs/chief-template-format.md` section 7): in that template 103 wall
types with their real layer stacks (name, material, thickness, main layer, framing; the 7 names with no definition fall back to the name-number guess),
15 text styles (font, style, height; `1/4" Text Style` is 4.5 plan inches, Avenir Book), 14 dimension default sets (arrow, extension, offset,
separation, smallest fraction, number format), 550 materials (146 named), the default room-type ceiling height (109 1/8") and the sheet size. Seeding
uses the decoded wall stacks first, then a stack found by the older name scan, then the name-number guess. What did not decode: per-floor defaults (floor, foundation,
rough ceiling and stem wall heights), roof and floor finish materials, arrow style, text colour, and the layout template's pages, boxes and title block
(the template stores no page or box objects the reader could identify). Chief's own `Default Text Style` is 6" Arial in the stock template but 4.5"
Avenir in Daniel's; the seed keeps the names Plan Studio already ships at their current values.

**Automatic seeding.** You do not have to import to use these values. Plan Studio finds your default plan and layout templates (from Chief's preferences INI,
else by their stock names), decodes them once into `~/.plan-studio/template-seed.json`, and lays the decoded wall types, text styles, dimension sets and default height over its
defaults when no saved `defaults.json` exists. **Edit > Default Settings > Preferences > Templates** shows and changes the two paths and the seeding switch, and the Import Chief Template window's
**Set as default plan / layout template** button writes a path there (chapter 1.7.1 has the whole story). The module is `plan-app/src/templates.rs`, with the Chief INI reader in
`plan-config/src/templates.rs`.

## 12.9 Chief hotkeys, toolbars and preferences

`plan-config` reads Chief's `UserHotkeys.xml`, the `.toolbar` files and the preferences INI. The app uses the
hotkeys (chapter 13). Daniel's own files are embedded at compile time from `docs/chief-config-raw/`. The one thing read from the live Chief
INI (`~/.config/Chief Architect Inc/Chief Architect Premier X18.ini`, then X17's) is the names of the default plan and layout templates (chapter 1.7.1); reading its other values
and rebuilding the toolbars from his four toolbar sets (Default, Extended Tool, Space Planning, Terrain) are (planned).

## 12.10 Notes

- Opening or exporting never touches your Chief files. Everything is read-only against Chief data.
- Large exports (glTF of a big model, 1920 x 1080 ray traces at 1024 samples) are slow in a debug build; use
  `cargo run --release -p plan-app`.
