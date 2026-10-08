# Chapter 12: Import and Export

What Plan Studio can read and write today, how each format is handled, and what is
engine-only (written and tested in a crate, but with no menu command yet). File > Export
(DXF, Elevation DXF, Construction Set PDF, glTF), File > Import (Import Drawing (DXF)), File > Templates >
Import Chief Template, File > Print (Print Layout, Export Layout PDF) and CAD > CAD to Walls work; the
Library menu's catalog import is still dimmed. Chief catalogs are read in place by the Library Browser (chapter 6.6), not imported.

## 12.1 Formats at a glance

| Format | Direction | In the editor | Where it lives |
|---|---|---|---|
| `.psplan` (Plan Studio JSON) | Read and write | Yes: File > Open Plan, Open Recent, Save, Save As, Save a Copy, Revert to Saved, Close Plan (12.2a); a Finder double-click or a drop on the window opens one | `plan-core` (`io`), `plan-app` (`files`) |
| Backup zip (`<name>-backup-<time>.zip`) | Write | Yes: File > Backup Entire Plan... (12.2a) | `plan-app` (`files`) |
| glTF 2.0 (`.gltf` + `.bin`) | Write | Yes: 3D > Export > glTF... | `plan-3d` |
| PDF (construction set) | Write | Yes: Tools > Schedules > Create Construction Set... | `plan-layout`, `plan-docs` |
| PDF (the plan's layout, or a plan view) | Write | Yes: File > Print > Print..., Print Layout... and Export Layout PDF... (chapter 11.3) | `plan-layout`, `plan-docs` |
| PDF (the Materials List) | Write | Yes: Materials List > Export PDF... (chapter 11.5) | `plan-docs` |
| PNG (ray-traced stills) | Write | Yes: Ray Trace > Save Image... at the same size, 2x or 4x (chapter 10.6) | `plan-render` |
| PNG (the plan's lines, or a ray-traced picture of the open 3D view) | Write | Yes: File > Print > Print Image... (chapter 11.3) | `plan-app` |
| PNG, baseline JPEG, scanned PDF (underlay pictures for tracing) | Read | Yes: File > Import > Underlay Picture... (12.4a) | `plan-app` |
| PNG and JPEG (baseline and progressive) as textures and pictures | Read | Yes, in 3D: the textures of the 3D view and the ray tracer (Chief's own texture files when your install has them, else generated ones), the bitmaps of pictures and billboards, and picture boxes in a layout (10.8a) | `plan-library` (`image`), `plan-materials` (`textures`) |
| CSV (schedules, materials list, layout tables) | Write | Yes: Export CSV... buttons | `plan-docs` |
| XLSX (Excel workbook: schedules, materials list, layout tables) | Write | Yes: Export Excel... buttons and Layout > Export Table to Excel... (chapter 11.2, 11.3); the program writes the workbook itself | `plan-docs` (`xlsx`) |
| Fonts: TrueType and TrueType collections (`.ttf`, `.ttc`; `.otf` and `.otc` are listed but not embedded) | Read | Yes: installed fonts draw plan and layout text and are embedded (subset) in PDFs you make (12.6; chapter 5.9) | `plan-app` (`fonts`), `plan-docs` (`pdf::truetype`) |
| Layout JSON | Read and write | Yes: File > Export > Layout (JSON)... and File > Import > Layout (JSON)... | `plan-layout` |
| Markdown and PDF (Plan Check report) | Write | Yes: Save Report... and Report PDF... (chapter 18.4) | `plan-check` |
| DXF (ASCII, R12) | Write | Yes: File > Export > DXF..., Elevation DXF..., and Export DXF from a section or elevation's vector view and its Camera Specification (chapter 10.7) | `plan-core::export::dxf`, `plan-elevation` |
| DXF import, CAD to Walls | Read | Yes: File > Import > Import Drawing (DXF)..., CAD > CAD to Walls... | `plan-import` |
| DWG | Neither | (planned) | |
| Chief catalogs `.calib`, `.calibz` | Read in place | Yes: the Library Browser's Chief nodes (chapter 6.6); no import command | `plan-calib` |
| Chief templates `.plan`, `.tpl`, `.layout` | Read names and some values | Yes: File > Templates > Import Chief Template... | `plan-chiefplan` |
| Chief project `.plan` (the building itself) | Read | Yes: File > Import > Chief Plan... (12.8a): floors, walls, doors, windows, named rooms, dimensions, text, cabinets, placed library objects, electrical devices, stairs and roof planes | `plan-chiefplan` (`import`) |
| Chief hotkeys, toolbars, preferences | Read | Hotkeys: Customize Hotkeys > Import (chapter 13.7); toolbars: Customize Toolbars > Import Chief Toolbar File (chapter 1.4a); the default plan and layout template names from the preferences INI (chapter 1.7.1) | `plan-config` |
| OBJ (`.obj` + `.mtl`) and glTF 2.0 (`.gltf`, `.glb`) 3D models into the user library | Read | Yes: Library > Import 3D Model (OBJ, glTF)... (chapter 6.4a, 12.7a) | `plan-import` (`obj`, `gltf`), `plan-library` (`model`) |
| Plan Studio library zip (`.calibz` extension, stored zip of JSON and `.psm` models) | Read and write | Yes: Library > Export Library (Plan Studio only)... and Import Library... (12.7a); Chief cannot open it | `plan-library` (`archive`) |
| `.psm` (Plan Studio model, `PSM1`) | Read and write | Inside the library zip and in `~/.plan-studio/user-models/` | `plan-library` (`model`) |
| Toolbar configuration JSON | Read and write | Yes: Customize Toolbars > Export... and Load Exported File... (chapter 1.4a); `~/.plan-studio/toolbars.json` | `plan-app` (`toolbar::config`) |
| Hotkey list (JSON, CSV, PDF) | Write | Yes: Customize Hotkeys > Export and Print List (chapter 13.7) | `plan-app` |
| IFC, SketchUp, Revit | Neither | (planned) | |

### Tools: the commands that read and write files

| Menu command | Hotkey | Does | Today |
|---|---|---|---|
| File > Open Plan... | `Cmd+O` | Opens a `.psplan`; asks first when the open plan has unsaved changes (12.2a). | Works. |
| File > Open Recent Documents | | The last ten plans opened or saved, newest first; Clear Menu empties the list. | Works. |
| File > Close Plan | | Replaces the plan with an empty one, asking first about unsaved changes. | Works. |
| File > Save, Save As... | `Cmd+S` | Writes a `.psplan` safely: a temporary file renamed over the plan, the old version kept in `Archives/` (12.2a). | Works. |
| File > Save a Copy... | | Writes the plan to another file and goes on editing the original. | Works. |
| File > Revert to Saved | | Reloads the saved file after a confirmation. | Works. |
| File > Backup Entire Plan... | | A zip of the plan and the pictures it uses, in a folder you pick. | Works. |
| File > Manage Auto Archives... | | The autosave and archive settings and the archive copies of this plan. | Works. |
| File > Export > DXF... | | Writes the active floor as an ASCII R12 DXF (12.3). | Works. |
| File > Export > Elevation DXF... (also 3D > Create Orthographic View > Export Elevations (DXF)...) | | Writes the four elevations as one DXF (12.3). | Works. |
| File > Export > Construction Set PDF... | | The same as Create Construction Set. | Works. |
| File > Export > glTF... | | The same as 3D > Export > glTF.... | Works. |
| File > Export > (image, DWG) | | Not in the menu. | (planned) |
| File > Import > Import Drawing (DXF)... | | Adds a DXF drawing to the active floor as CAD objects (12.4). | Works. |
| File > Import > Underlay Picture (PNG, JPEG, PDF)... (also Tools > Underlays...) | | Places a picture under the plan for tracing (12.4a). | Works. |
| File > Import > (DWG) | | Not in the menu. | (planned) |
| File > Import > Chief Plan... | | Reads a Chief project `.plan` into a new plan in the window (12.8a); it asks about unsaved changes first. | Works. |
| File > Templates > Import Chief Template... | | Seeds your defaults from a Chief `.plan`, `.tpl` or `.layout` (12.8). | Works. |
| File > New Layout | | Makes the plan's layout and shows the layout view (11.3). | Works. |
| File > Open Layout... | | Shows the layout view. | Works. |
| File > Print > Print... (the row 1 Print button) | `Cmd+P` | The Print dialog: a PDF file, the system printer or the viewer (11.3). | Works. |
| File > Print > Print Layout... | | The same dialog for the layout's printed pages (all, or a range). | Works. |
| File > Print > Export Layout PDF... | | Saves every printed page as a PDF. | Works. |
| File > Print > Print Image... | | Saves the active floor's plan lines as a PNG. | Works. |
| File > Export > Layout (JSON)..., File > Import > Layout (JSON)... | | Writes or reads the plan's layout as JSON. | Works. |
| 3D > Export > glTF... | | Writes `.gltf` and `.bin`. | Works. |
| Tools > Schedules > Create Construction Set... | | Writes a PDF set. | Works. |
| Door/Window/Room/Wall Schedule > Export CSV... | | Writes a CSV. | Works. |
| Schedule Specification > Export CSV... (a schedule placed in the plan) | | Writes a CSV of the placed schedule (11.2). | Works. |
| Materials List... > Export CSV..., Export PDF... | | Writes the priced take-off as a CSV or a PDF (11.5). | Works. |
| Checks window > Save Report..., Report PDF..., Add to Layout | | Writes Markdown or a PDF, or adds a page to the layout (chapter 18.4). | Works. |
| Ray Trace... > Save Image... | | Writes a PNG at the render size, 2x or 4x. | Works. |
| Vector view > Export DXF..., Camera Specification > Export drawing as DXF... | | Writes the section or elevation drawing as a DXF (10.7). | Works. |
| Customize Hotkeys > Import, Export, Print List | | Reads a Chief `UserHotkeys.xml`; writes the keys as JSON or CSV; prints the list as a PDF (13.7). | Works. |
| Customize Toolbars > Import Chief Toolbar File..., Export..., Load Exported File... | | Reads a Chief `.toolbar`; writes or reads the toolbar configuration as JSON (1.4a). | Works. |
| Library > Export Library (Plan Studio only)..., Import Library... | | Writes or reads the User catalog as a `.calibz` zip (JSON, folders, favorites and recents, `.psm` models). Plan Studio only: Chief cannot open it, and Chief `.calib` files are read in place instead (12.7a, 12.7). | Works. |
| Library > Import 3D Model (OBJ, glTF)... | | Adds an OBJ, glTF or GLB model to the User catalog with unit and up-axis options (6.4a, 12.7a). | Works. |
| Library > Add Selection to Library, Add Active Material to Library | | Saves plan objects (a symbol, cabinet, CAD pieces, text) or the Material Painter's material into the User catalog (6.4a). | Works. |
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
  | `Floor.roofs` | Roof planes, the Build Roof settings, ceiling planes, dormers and the Dutch gable faces Build Roof makes (chapter 8) |
  | `Floor.electrical` | The floor's electrical devices and connections (chapter 9) |
  | `Floor.framing` | Built framing members, manual members and the framing layout lines (chapter 11.11) |
  | `Floor.foundation` | Slabs, slab holes, pads, piers and platform holes (chapter 16) |
  | `Floor.details` | Corner boards, quoins, moldings, floor and wall material regions, wall hatches, polygon decks and 3D solids (chapter 17) |
  | `Floor.schedules` | The schedules placed in the plan: kind, columns, sort, filter, labels, style, position (chapter 11.2) |
  | `Floor.cad_attrs`, `Floor.cad_blocks` | Own style (color, weight, dash, fill, arrows, rich text) of CAD objects, and the names and insertion points of CAD blocks (chapter 5.5). Typed lists, not JSON |
  | `Project.text_macros`, `Project.note_types` | The plan's user text macros and note types (chapter 5.3). Typed values |
  | `Project.terrain` | The terrain, its contour interval, whether it is built, and the terrain walls, breaks and landscape objects (chapter 9) |
  | `Floor.underlays` | Underlay pictures placed under the plan: file path, pixel size, placement, opacity, show and lock (12.4a). Typed list |
  | `Project.object_materials` | Per-object material overrides from the Material Painter and Adjust Materials (chapter 10.8). Typed values |
  | `Project.layout`, `Project.layout_files` | The plan's open layout (pages, boxes, title block and page setup) and the other layout files the plan holds (chapter 11.3) |
  | `Project.opening_display` | The 3D display of openings: casing, jambs and sills on or off, doors shown open and the open angle (chapter 3.9) |
  | `Project.info` | Project Information: client, designer, job number, date, revisions, custom fields (chapter 11.4) |
  | `Project.lights`, `Project.light_options` | The lights Add Lights places, and whether electrical light fixtures emit light (chapter 10.13) |
  | `Project.info.custom` (reserved keys) | `plancheck.settings` and `plancheck.ignored`: the Plan Check settings and the findings you ignored, as JSON text (chapter 18.2) |
  | `Floor.framing` (one reserved record) | The Framing Defaults of the plan, stored under the key `FramingSettings` among the framing members (chapter 11.11) |
  | `Project.plan_views`, `Project.layer_sets` | Saved plan views (floor, layer set, reference display, zoom and pan) and the named layer sets (chapter 5.5) |

  Typed, serde-default **extras** hold what dialogs used to keep per session: a wall's label switch, specified label text and last
  picked wall type; an opening's style name, thickness, swing angle, jamb or frame width and Show Open in 2D; a room's
  conditioned setting, stem wall height, base and crown moldings, fill and label options. A wall also stores its class and
  curve, a camera object its section line (with the back clip), its elevation rendering options (hatch, shadows, sun, line weight by distance, labels), its Vector View options and its plan callout (number and visibility), and a cabinet its library type, label offset and Opening Indicators in 3D. The terrain record also holds the plan's north angle.
- **Migration.** Files from before these slots kept the roofs, electrical devices, terrain and slabs as hidden, locked text records on
  reserved layers (`Roof Planes, Data`, `Electrical, Data`, `Terrain, Data`, `Foundation, Data`, `Lights, Data` for lights, and, until Round 8, `CAD, Data` for CAD styles, CAD blocks, text macros and note types). When you open such a file the editor moves each
  record into its slot and deletes the legacy items and the hidden layers; it happens as the file loads, so it is not an undo step. A slot that is already filled
  is left alone, every new field has a default, and old files otherwise load unchanged.
- Per-session settings (the "session only" fields that remain in the dialogs) are not in the file.
- The file is readable and diff-friendly; keep it under version control if you like.

File > Open Recent Documents lists the last ten plans (`recent_files` in `~/.plan-studio/settings.json`; a plan that has moved is left out). Saving is safe, there is an autosave and crash recovery, and the unsaved-changes prompts guard New, Open, Close and Quit: all of that is in 12.2a.

## 12.2a File management: safe saves, archives, autosave and recovery

Round 11 added `crates/plan-app/src/files.rs` (with `plan-core/src/io.rs` and the prompts in `dialogs/unsaved.rs`). It makes a crash, a full disk or a mistaken click lose as little as possible. All times in file names and prompts are **UTC**.

### Saving

- **Save** (`Cmd+S`) first copies the file now on disk into `Archives/<plan name>/` beside the plan, named `<plan name>-<yyyymmdd-hhmmss>.psplan`, then writes the new version to a temporary file next to the plan, flushes it to disk and **renames it over the plan**. The plan on disk is therefore always either the old version or the new one, never half of each.
  If the write fails (a full disk, a locked folder) the status bar says "Save failed: ...; the file on disk is unchanged" and nothing was replaced. If only the archive copy fails, the save still happens and the status bar adds "(could not archive the old version: ...)".
- The newest **20** archive copies of each plan stay; older ones are deleted when you save. Two saves inside one second both keep their copy (the second gets `-2`, `-3` ...). A first save (no file yet) has nothing to archive; Save As over an existing file archives the file it replaces, and Save As to a new name archives nothing.
- **Save As...** adds `.psplan` if you leave it off. **Save a Copy...** (suggested name `<project> copy.psplan`) writes the plan elsewhere with the same safe write, keeps the original as the open plan and does not archive or mark anything saved.
- The window title reads `Plan Studio — <file name>` (`Untitled` before the first save) and gets a dot (•) after the name while there are **unsaved changes**. The dot follows the plan's contents, not a flag: undo back to the saved state and it goes away. The status bar also shows how long ago you last saved, as "Saved just now", "Saved 2 min ago", "Saved 3 hr ago" and so on, with ", edited since" while there are unsaved changes (the time counts from the last save or open, or from when the new plan was made).

### File menu commands

| Command | What it does |
|---|---|
| **Close Plan** | Replaces the plan with an empty new one ("Closed the plan"). With unsaved changes it asks first. |
| **Revert to Saved** | Reloads the file on disk. Needs a saved plan ("This plan has not been saved yet; there is nothing to revert to" otherwise). With unsaved changes it asks "Revert “name” to the last saved version? Changes made since then are lost." Revert is the default button. |
| **Save a Copy...** | As above. |
| **Backup Entire Plan...** | Asks for a folder and writes `<plan name>-backup-<yyyymmdd-hhmmss>.zip` there: the plan as `<plan name>.psplan`, a copy of every picture file the plan refers to under `assets/` (numbered `01-name.png`, ...), and a `README.txt` that lists each copy and the original path it came from. The plan itself still refers to pictures by their original paths; the zip is a safe copy, not a relocated plan. It finds underlay pictures, placed images and library items (PNG, JPEG, PDF, GIF, BMP, TIFF paths in the plan) that exist on disk; the status bar reports "Backed up to <file> (n picture(s), m not found)". The zip is stored without compression. |
| **Manage Auto Archives...** | The window below. |
| **Open Recent Documents > Clear Menu** | Empties the recent list ("Cleared the recent plans list"). |

**Manage Auto Archives** (File menu) holds the settings and the archive copies of the open plan:

| Control | Meaning |
|---|---|
| Autosave while there are unsaved changes | On by default. |
| Every n minutes | 5 by default, 1 to 120. |
| Keep the last n archive copies of each plan | 20 by default, 1 to 500. |
| The archives list | The ten newest copies of this plan, each with an **Open** button (it asks first if the open plan has unsaved changes), **Show Archives Folder** (opens `Archives/<plan name>/` in the system file manager) and **Back Up Entire Plan...**. "None yet; a copy is kept each time you save over a saved plan." before the second save, and "Save the plan to start its archive." for an unsaved plan. |

The settings are the `files` key of `~/.plan-studio/settings.json` (`autosave`, `autosave_minutes`, `archive_keep`) and are written when you change them.

### Autosave and recovery

- **Autosave.** While the plan has unsaved changes and the interval has passed, the program writes `Archives/<plan name>/autosave.psplan` (for a plan never saved, `~/.plan-studio/recovery/untitled-autosave.psplan`). It **never touches the real file**. The autosave is deleted when you save, when you answer Don't Save, and when you undo back to the saved state.
- **Opening a plan whose autosave is newer than the file** (the program or the machine stopped after the last save) shows **Recover Unsaved Work**: "An autosave of “name” from <time> UTC is newer than the saved file." **Recover** (`Enter`) loads the autosave as the open plan, marks it unsaved and says "Recovered the unsaved work; save it to keep it"; the autosave is deleted when you next save. **Discard** deletes it. Discard has no key, so a stray `Enter` cannot throw work away.
- **After a crash.** A panic in the editor, and an exit that skips the unsaved-changes prompt (`Cmd+Q` on macOS is handled by the system and quits at once), write the unsaved plan to `~/.plan-studio/recovery/recovery-<yyyymmdd-hhmmss>.psplan` (the newest 10 stay). The next launch, when no plan is named on the command line, offers the newest one: "Plan Studio did not close normally. A copy of an untitled plan from <time> UTC was kept." Recover opens it as an untitled plan (Save asks for a name); Discard deletes it and the older crash copies. A plan that was open and had no changes writes nothing, and neither does a new plan nobody touched.

### Unsaved-changes prompts

New Plan, Open (including Open Recent, a dropped file, a Finder open and an archive copy), Close Plan, File > Import > Chief Plan... and Quit all stop with **Unsaved Changes** when the plan has unsaved changes: "Do you want to save the changes you made to “name” before you start a new plan / open another plan / close it / import a Chief plan / quit?" The window's close button asks too. The buttons and keys:

| Button | Key | Does |
|---|---|---|
| Save | `Enter` | Saves (Save As for a plan with no file; cancelling that dialog stays where you were), then carries on. |
| Don't Save | `Cmd+D` (`Ctrl+D` off macOS) | Carries on and discards the changes and the autosave. |
| Cancel | `Esc` | Stays. |

### Opening a plan from outside the program

- **Drag and drop.** Dropping a `.psplan` on the window opens it (with the prompt above if needed); any other file is refused with "Drop a .psplan file to open it".
- **Command line.** `plan-studio house.psplan` opens that plan at the first frame. On Windows and Linux a double-click passes the path this way, as a plain path or a `file://` URL (`%20`-style escapes are decoded); options and macOS's `-psn_...` argument are skipped.
- **Finder (macOS).** A double-click on a `.psplan`, a drop on the Dock icon and `open -a "Plan Studio" file.psplan` open the plan, both when the program starts and when it is already running (in which case the unsaved-changes prompt applies). Finder does not put the file on the command line: it sends an "open documents" Apple Event, and `mac_open.rs` registers its own handler for it through the Objective-C runtime (winit does not). The `.app` built by `scripts/macos-bundle.sh` declares the `.psplan` type (identifier `com.danielallendesigns.plan-studio.psplan`, conforming to JSON) and makes Plan Studio its owner, so the extension opens here. On Linux, copy `scripts/linux/plan-studio-psplan.xml` to `~/.local/share/mime/packages/` and run `update-mime-database ~/.local/share/mime` for the same (see the comment in `plan-studio.desktop`).

Limits: the dirty check hashes the whole plan, which takes a moment on a very large one, so it runs a short while after the last change and never during a drag. Archives and autosaves are `.psplan` files like any other; the program does not clean `Archives/` folders of plans you deleted. The pictures a plan uses are not copied into archives or autosaves (Backup Entire Plan does copy them).

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
- **Scale** (on top of the units), **Rotation** (degrees, about the insertion point), **Base point** (the drawing's
  origin or the lower-left corner of its extents) and **Insert it at** (x and y, feet-inches) place the drawing in the plan.
- **Layer mapping**: every DXF layer is listed with its object count and a choice of where it goes: Keep (under the prefix),
  Do not import, one of the plan's layers (a layer named like a plan layer is preselected), or a new layer of a name you type.
  Blocks (INSERTs) are always exploded.
- **Convert to walls** (with the layer to read, "A-WALL" preselected when there is a wall layer) also runs the CAD to Walls matcher
  with its default options on that layer's lines and adds the walls in the same undo step.
- **Import** adds the drawing to the active floor as CAD objects, as one undo step ("Import Drawing"). Layers the
  plan does not have are created, hidden when the DXF layer was off. The status bar says "Imported n objects (k
  new layers)", or "The drawing had nothing to import", and the number of walls when it made some.

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


## 12.4a Underlays (PNG, JPEG, scanned PDF)

An **underlay** is a picture placed under the plan for tracing: a scanned survey, an existing-house drawing, a photo
of a sketch. File > Import > Underlay Picture and Tools > Underlays... open the **Underlays** window. Underlays live on the floor
(`Floor.underlays`), on the **Underlays** layer (created on the first import; hide or lock it like any layer), and are drawn above the
grid and under everything else.

- **Pictures**: PNG; baseline JPEG (sequential Huffman, grayscale or YCbCr; a progressive JPEG is refused with a message, save it
  as PNG; the shared decoder in `plan-library` reads progressive files too, but the underlay tool still has its own baseline-only decoder); and PDF files that hold scanned pages as JPEG pictures (the **PDF page** box picks which). This build has no PDF renderer, so
  a PDF drawn with vector lines and text cannot be used: print its page to a PNG first. The picture is kept by file path (pictures taken
  out of a PDF are saved under `~/.plan-studio/underlays/`), decoded on a thread, and drawn at 2,048 pixels on its long side at most.
  While it loads, or when it cannot be shown, a framed placeholder with the reason is drawn; it still moves, scales and calibrates.
- A new picture is centered on the walls (else the origin) at a placeholder scale of 40 feet on its long side. **Calibrate**: with the
  Underlay tool click two points on the picture (a printed dimension, a wall of known length), type the real distance
  (`24'-0"`) in the window and Apply. The picture is scaled about the first point so those two points are exactly that far apart
  (one undo step, "Calibrate Underlay"). **Real width** sets the width directly, for pictures with no good second point.
- **Opacity** (0-100%, default 60%), **Rotation** (about the picture's middle), **Show** and **Lock** (a locked underlay, or a locked
  Underlays layer, refuses to move, scale or calibrate), **Move** (the Underlay tool: drag the picture; click another to pick it) and Delete.
  Every change is one named undo step. Underlays are not printed or exported.

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
  File > Print > Print... opens the Print dialog (paper, scale, tiling, color, page range) and sends the PDF to a file, the system printer (CUPS `lp`) or the viewer.
  The PDF writer is the same dependency-free PDF 1.4 one for all of them. A layout PDF carries one bookmark per printed sheet.
- **PNG**: the Ray Trace window's Save PNG... (chapter 10.6), uncompressed 8-bit RGBA.
- **Fonts in PDFs** (Round 13): a text style's font is looked up among the fonts installed on the machine (`/System/Library/Fonts`, `/Library/Fonts` and `~/Library/Fonts` on a Mac; the usual folders on Linux and Windows) and a subset of the face (only the glyphs the document uses, 8 to 15 KB per face) is embedded as a TrueType font. Limits: a font with PostScript outlines (most `.otf` files) is not embedded and prints as Helvetica with a note; a font whose licence forbids embedding (the `fsType` restricted-licence or bitmap-only bit) is never embedded, and the note says so; only Latin-1 and the usual punctuation print, other characters print as `?`; layout page CAD text, leaders and title blocks stay Helvetica. Fonts are embedded only into the PDFs you make on your own machine; nothing is copied into plans, templates or the program. Preferences > Fonts turns the whole thing off (chapter 1.9a).
- **CSV**: every schedule window and the Materials List window have Export CSV.... Fields are
  quoted as needed; open them in any spreadsheet. **XLSX**: the same windows (and the Schedule Specification) have Export Excel..., and the layout's table boxes export with Layout > Export Table to Excel...; the workbook has one sheet per table. The Materials List CSV has the columns Category, ID, Description, Size, Count, Unit, Unit Price and Price, and a Total row when any row is priced; the Materials List window also has Export PDF....
- **Markdown**: the Plan Check window's Save Report... writes findings grouped by severity.

## 12.7 Chief catalogs (`.calib`, `.calibz`)

`plan-calib` reads the user's own Chief library catalogs in place, and the Library Browser shows them (chapter 6.6). There is no command that imports a Chief catalog: catalogs are read where Chief keeps them. (The Library menu's **Import Library...** reads only Plan Studio's own export zip, 12.7a.)

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
  geometry is partial; chapter 6.6). `LibraryObjects.Type`, texture link tables and `Content/<hash>-01` image resolution are not decoded. (The textures of walls, roofs, floors and the landscape in the 3D view are a different thing: they come from Chief's texture folders, chapter 10.8a.)

Opening a `.calibz` writes its embedded `.calib` to `<temp dir>/plan-studio/` as a cache, a derived copy
of your own file outside the repository; delete it any time. Details: `docs/chief-library-format.md`.

### Licensing rule

Chief catalogs, their thumbnails and textures are Chief Architect and manufacturer licensed content.
Plan Studio **reads them in place** from your installation; it never ships, copies or commits any of it. That includes the texture files the 3D view and the ray tracer
use when your Chief install is present (chapter 10.8a): they are decoded in memory at run time, never written anywhere, and the program works without them.
`.gitignore` excludes `*.calib`, `*.calibz`, `*.calib_error`, `*.plan` and `*.layout`. Tests use synthetic
fixtures built at test time; tests against a real install are marked `#[ignore]`. This is decision 3 in
`DECISIONS.md`.

## 12.7a Your own library: models and the export zip

The User catalog (chapter 6.4a) takes in 3D models and moves between computers as one zip. None of these formats are Chief's, and Chief Architect cannot read them.

**OBJ and glTF models.** Library > **Import 3D Model (OBJ, glTF)...** picks a Wavefront `.obj` (colors from a `.mtl` beside it), a glTF 2.0 `.gltf` with its `.bin` or a binary `.glb`. `plan-import::{obj, gltf}` reads them into an indexed triangle mesh in inches (`ImportedModel`):
glTF scenes and node transforms, triangle meshes and base colors, but not textures, sparse accessors or Draco compression. The window asks for the file's **units** (inches, feet, millimeters, centimeters, meters; glTF defaults to meters), the **up axis** (Y or Z), the
**folder** and the **placement**, and shows the triangle count and size. The result is stored as a `.psm` model beside your library.

**`.psm` (Plan Studio model).** A compact binary file: the magic `PSM1`, a part count, then per part its name, an optional color and the vertices and triangle indices, all little-endian. The model is normalized in the library: centered at x = 0, bottom at y = 0, back face at z = 0, front facing +Z (the
frame of a placed symbol). The Library Browser draws its preview pane from the model with a small software rasterizer (one headlight and a key light, 2 x 2 supersampling).

**The export zip.** Library > **Export Library (Plan Studio only)...** writes a zip with the extension `.calibz` so it sits next to Chief libraries in a file picker. It is a plain zip of stored (uncompressed) entries: `plan-studio-library.json` (`format`, `version`, the catalog and the folders, favorites and
recents), `user-models/<name>.psm` for each model, and a `README.txt` that says it is not a Chief library. **Import Library...** reads exactly what Export writes (items with the same id are replaced). A zip made by another tool imports only if its entries are stored, not deflated. Do not confuse it with a Chief `.calibz`
(deflated textures plus a SQLite `.calib`, 12.7), which Plan Studio reads in place and never imports.

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

**Reading a plan's content: see 12.8a.** Everything above reads a template's *defaults*; the next section reads a project's *building*.

**Automatic seeding.** You do not have to import to use these values. Plan Studio finds your default plan and layout templates (from Chief's preferences INI,
else by their stock names), decodes them once into `~/.plan-studio/template-seed.json`, and lays the decoded wall types, text styles, dimension sets and default height over its
defaults when no saved `defaults.json` exists. **Edit > Default Settings > Preferences > Templates** shows and changes the two paths and the seeding switch, and the Import Chief Template window's
**Set as default plan / layout template** button writes a path there (chapter 1.7.1 has the whole story). The module is `plan-app/src/templates.rs`, with the Chief INI reader in
`plan-config/src/templates.rs`.

## 12.8a File > Import > Chief Plan...

**File > Import > Chief Plan...** picks a Chief Architect project `.plan` and builds a new Plan Studio plan from it (`plan_chiefplan::import::import_plan`; the file formats are in `docs/chief-plan-format.md`, sections 4.1 to 4.14). Nothing is copied from the file into the repository, and your Chief file is only read.

1. If the open plan has unsaved changes, the command asks first (Save, Don't Save, Cancel), as New and Open do.
2. Pick the `.plan` file. A `.layout` file holds sheets, not a building, and is refused ("a .layout file holds sheets, not plan geometry").
3. The new plan replaces the one in the window, named after the file, with no file path yet (use Save As).
4. The **status bar** gets the one-line **headline**, for example `Imported House.plan: 4 floors, 228 walls, 46 doors, 34 windows, 19 named rooms, 174 dimensions, 137 texts, 65 cabinets, 62 symbols, 401 electrical devices, 10 stairs, 31 roof planes`.
5. A **Chief Plan Import** window opens with the same headline and the full **summary**: one line per note in plain language ("floor names are positional", "dimension offsets are assumed", the roof planes it could not read). **Copy** puts the text on the clipboard; **Close** dismisses it.

What is imported (Round 13 added the object rows from Cabinets down):

| Plan Studio | From the Chief file | How well it is understood |
|---|---|---|
| Floors | Count, order, floor elevation and ceiling height | High. **Names are positional** (Foundation for a floor below grade, 1st Floor, 2nd Floor ..., Attic for a top floor that holds roof planes): the file stores no names. |
| Walls | The wall line, the wall type (its layer stack replaces a same-named default so the registry and the thickness agree), the height, which side is exterior, and curved walls (the minor arc) | Line and type high; height, side and arcs medium. The line is the centerline of the main layer, shifted for asymmetric exterior walls from one measured type, so some types may sit a fraction of an inch off. Joined ends are healed. |
| Doors and windows | The openings that belong to a wall: center, width, height | High for position and size. **The style is guessed from the width** (door vs doorway, window kind), swing is not read. |
| Rooms | The names of rooms (`kitchen`, `F. Porch`), from the room records and, since Round 13, from typed text labels | Medium. Plan Studio detects the room shapes itself from the walls and anchors the names in them (a label's centre is the anchor). |
| Dimensions | Linear dimension strings: their points (X18 and, since Round 13, X17) | Medium for points. **The dimension line's position is not stored where it could be found**, so every string is placed 36" outside the floor's walls, one dimension per consecutive pair of points. Text overrides and arrows are not read. |
| Text | Free text notes: position and string | Medium. The size is assumed (4.5" regular, 8" for bold text), angle 0. Automatic wall labels are skipped. |
| Cabinets | Base, wall, tall, shelf and soffit boxes with their position, size and rotation, and free-form (island) countertops | High for the box; the catalog name, the door and drawer layout and per-part materials are not read. A cabinet is a box with its own style guess. |
| Library objects | Placed furniture, fixtures and appliances: position, size, rotation | High for position and size. A plan holds its own copy of each library object and no link to a catalog, so each object comes in as a placed symbol named `chief-plan.<name>` (for example `chief-plan.elongated-toilet`); the plan and the 3D view draw it as a **labelled stand-in box** with the Chief name. Objects that belong to a cabinet are left to the cabinet. |
| Electrical devices | Switches, outlets, lights and the rest: position and kind | High for position and kind; the height comes from the kind, wall devices are attached to the nearest wall, and the label is the Chief name. |
| Stairs | Stair flights and landings | Medium. A U-shaped stair comes in as two straight flights and a landing; each flight's rise is its risers times the riser height and a second flight starts at floor level, not at the landing's height, so the 3D view shows the parts side by side. |
| Roof planes | The roof planes of every floor that holds them (porch roofs on the first and second floors, the main roof on the attic floor) | Medium. Planes only: pitch, outline and baseline. They are manual planes on the `Roof Planes` layer, so Build Roof leaves them alone (Delete Roof Planes clears them). The plan draws a roof while its floor is active, the 3D view draws every floor's. |
| Layers, layer sets, text styles, wall types | Read by the same scan as templates (12.8) | X18 files only. |

**What does not import:** circuits and the switch-to-load connections of the electrical devices; the heights at which stair flights and landings stack; the rail and wall sides, stringer and tread styles of stairs, winders and curved stairs; per-edge hip and gable flags, overhang and fascia sizes, holes, skylights, materials and dormers of a roof; framing; moldings and trim; room polygons; floor names; door styles and swings; dimension line offsets; wall bottoms and sloped tops; wall connection records; major arcs and bay or bow walls; the catalog link of a library object; and the materials, door and drawer layouts and corner or blind shapes of cabinets. A few roof objects with an outline but no baseline record (probably ceiling or deck surfaces) are left out and counted in the report. You will redraw the roof edge details, stair railings and fixtures' catalog links on the imported shell, or place them from the library.

An X17 file imports walls, wall types, openings, floors, rooms, dimensions and roof planes, but not the layers and layer sets (those come from the X18 header).

The importer also builds a report (`ImportReport`): counts, per-floor counts, the skipped classes (class number, label, confidence, how many) and notes in plain language. The report window shows the counts line and the notes; the whole report with the skipped classes is available to code and to the `import` example (`cargo run --release -p plan-chiefplan --example import -- "House.plan" --json`). Importing a 75 MB file takes about 0.3 s once the file is in the operating system's cache. All archived Chief projects of the test set (3 to 170 MB) import without an error.

## 12.9 Chief hotkeys, toolbars and preferences

`plan-config` reads Chief's `UserHotkeys.xml`, the `.toolbar` files and the preferences INI. The app uses the
hotkeys (chapter 13) and the toolbar sets (chapter 1.4a: the per-view toolbar sets start from Daniel's four Chief toolbars, and Customize Toolbars can import any Chief `.toolbar` file, mapping each button to its Plan Studio counterpart). Daniel's own files are embedded at compile time from `docs/chief-config-raw/`. The one thing read from the live Chief
INI (`~/.config/Chief Architect Inc/Chief Architect Premier X18.ini`, then X17's) is the names of the default plan and layout templates (chapter 1.7.1); reading its other values
is (planned).

## 12.10 Notes

- Opening or exporting never touches your Chief files. Everything is read-only against Chief data.
- Large exports (glTF of a big model, 1920 x 1080 ray traces at 1024 samples) are slow in a debug build; use
  `cargo run --release -p plan-app`.
