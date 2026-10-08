# Plan Studio

Plan Studio is an open-source Rust re-creation of Chief Architect X18's workflow for residential design. You draw
walls on a floor plan, and rooms, 3D views, elevations, schedules, plan checks and construction documents are
generated from that one model. Tool names, dialog tabs, menus and hotkeys follow Chief Architect X18, so a designer who
knows that workflow can sit down and draw.

**Status: pre-alpha, but a working editor.** Plan Studio opens, edits and saves plans, draws in 2D and 3D, arranges
plans on layout sheets, runs Plan Check, and writes schedules, DXF, glTF, PNG renders and PDF. The workspace has 22
crates and about 4,000 tests (counted from the source on 2026-10-08). No version has been tagged yet; the first
release will be `v0.1.0` (see the [release checklist](docs/release-checklist.md)).

Plan Studio is an independent project. It is not made by, affiliated with or endorsed by Chief Architect, Inc. Chief
Architect is a trademark of its owner and is named here only to describe compatibility.

## Status

How close the editor is to Chief Architect X18, counted from the per-id rows of [docs/parity-status.md](docs/parity-status.md)
(snapshot of 2026-10-08, 765 behavior ids; Works = present and exercised by a test or the tool itself, Partial = present
with a stated limit, Missing = no code path, Differs = deliberately different).

| Area | Ids | Works | Partial | Missing | Differs |
|---|---|---|---|---|---|
| Walls | 105 | 84 | 18 | 1 | 2 |
| Select and edit | 112 | 89 | 18 | 4 | 1 |
| Doors and windows | 110 | 92 | 12 | 5 | 1 |
| Rooms and floors | 71 | 48 | 22 | 1 | 0 |
| Roofs | 60 | 52 | 8 | 0 | 0 |
| Dimensions | 45 | 38 | 6 | 0 | 1 |
| Text | 19 | 14 | 5 | 0 | 0 |
| CAD | 42 | 30 | 10 | 2 | 0 |
| Layers | 15 | 15 | 0 | 0 | 0 |
| 3D and cameras | 71 | 40 | 26 | 5 | 0 |
| Cabinets | 21 | 16 | 5 | 0 | 0 |
| Stairs | 13 | 8 | 5 | 0 | 0 |
| Framing | 8 | 4 | 3 | 1 | 0 |
| Terrain | 10 | 8 | 2 | 0 | 0 |
| Library | 9 | 7 | 1 | 0 | 1 |
| Electrical | 7 | 7 | 0 | 0 | 0 |
| Layout and documents | 47 | 34 | 12 | 0 | 1 |
| **Overall** | **765** | **586** | **153** | **19** | **7** |

These rows move every round. `python3 scripts/parity-score.py` recounts them (and prints a weighted score and the
build order of what is still missing); [ROADMAP.md](ROADMAP.md) lists what is done and what is left, and
[CHANGELOG.md](CHANGELOG.md) says what each round added. "Works" is not the same as "a person has used it for a
real project": the manual QA pass in the [release checklist](docs/release-checklist.md) has not been done yet.

## Screenshots

![Plan Studio with Chief-style toolbars, menus and the Low Glare theme](docs/screenshot-chief-toolbars.jpg)

That window is from 2026-10-07, early in the project, and is the only screenshot in the repository today. The program
has far more working buttons, dialogs and docks now. Current screenshots are expected under `docs/screenshots/` with
these names (the folder does not exist until they are captured):

| File | What it shows |
|---|---|
| `docs/screenshots/plan-overview.png` | The two-story colonial sample in plan view with the Project Browser and Active Layer Display Options docks open |
| `docs/screenshots/spec-dialog.png` | A specification dialog (the Wall Specification, with its tabs) over a plan |
| `docs/screenshots/3d-view.png` | A Full Camera or Doll House view of a sample, with shadows on |
| `docs/screenshots/render.png` | A path-traced render (Physically Based technique) of an interior or exterior view |
| `docs/screenshots/layout-sheet.png` | A layout page from the construction set (title block, plan view box, sheet index) |
| `docs/screenshots/library-browser.png` | The Library Browser dock showing the built-in symbol catalogs |
| `docs/screenshots/plan-check.png` | The Plan Check window with its findings list |

How to capture them:

1. Build a release binary (`cargo run --release -p plan-app`) and open one of the plans in [`samples/`](samples/README.md)
   with File > Open Plan. Use the default Low Glare theme and a window about 1600 x 1000 points.
2. On macOS press `Shift+Cmd+4`, then `Space`, then click the window (this captures the window with its shadow; hold
   `Option` while clicking to leave the shadow out). On Windows use `Win+Shift+S`; on Linux use your desktop's screenshot tool.
3. Save as PNG under the file name above, at 1600 px wide or more, and keep each file under about 1.5 MB.
4. Do not capture Chief Architect library thumbnails, textures or templates: those are read from a Chief install and
   are not ours to redistribute. Use the built-in symbol catalogs and the generated textures (turn off any Chief
   texture folder, or capture on a machine without Chief installed).

## Why

Plan Studio aims to give residential designers an open, inspectable tool for the core design workflow they already know,
with a modern Rust codebase that others can read and extend. Its measure of progress is the parity specification in
[docs/parity/](docs/parity/): each behavior of the reference workflow has a stable id, and each id has a status and the
code and tests that back it.

## What it does today

- **Tools** (Chief's names and hotkeys): walls (straight and curved exterior, interior, foundation, pony, glass, half
  walls, room dividers, railings, deck railings, fencing; typed length and angle; wall hatching and material regions);
  doors and windows (ten door styles and eleven window flavors with mulling, sidelites and plan symbols); dimensions
  (manual, automatic exterior and interior strings, angular, running, baseline, elevation, story pole, NKBA runs;
  tied to the walls and openings they measure); text (text, rich text, leaders, callouts, markers, notes, text macros);
  CAD (lines, polylines, arcs, circles, boxes, splines, revision clouds, CAD blocks, fillet, chamfer, offset, trim,
  extend, break, hatch); cabinets (base, wall, full height, soffit, corner and blind types, countertops, backsplashes,
  labels with macros); library symbols; stairs (straight, L, U, winder, curved, spiral, Click Stairs, landings, ramps,
  railings); slabs, pads and piers; roofs (Build Roof, roof planes, gable lines, holes, skylights, dormers, ceiling
  planes); exterior details; electrical devices (23 kinds, Auto Place Outlets and Switches); terrain and landscaping;
  pictures and distributions; framing (Build Framing for walls, floors and roofs, 19 manual framing tools, a lumber
  takeoff); cameras (full, section, elevation, walkthrough) and lights.
- **Select Objects** picks, moves, stretches and edits every object kind, with temporary dimensions, Chief's snaps,
  an Edit toolbar, a right-click menu, a full Edit menu (Cut, Copy, Paste, Duplicate, Group, Transform/Replicate,
  Reflect, Align/Distribute, Lock, Send to Layer, Action History), and one shared undo stack for plan and layout
  edits that names each step.
- **Dialogs** in Chief's layout and with Chief's tab names for walls, doors, windows, rooms, floors, foundations,
  dimensions, text, CAD, cabinets, symbols, stairs, roofs, framing, electrical, terrain, cameras and layout boxes;
  Default Settings, Preferences, Customize Hotkeys, Customize Toolbars, Layer Display Options, Plan Views, the Space
  Planning Assistant, and Plan Check (52 IRC-based rules).
- **Docks and views**: Active Layer Display Options, Project Browser, Library Browser (about 145 built-in 2D symbols
  and a User Catalog with folders, favorites and OBJ or glTF model import), and tabs for saved plan views.
- **3D**: overviews, Doll House, Full Camera, sections and elevations in an orbitable viewport with shadows and ambient
  occlusion; vector elevations and sections; a CPU path tracer with a clear sky, depth of field and a denoiser; glTF export.
- **Documents**: door, window, room and wall schedules, a Materials List, framing takeoff, layouts with page tabs,
  boxes, title blocks and a ten-sheet construction set, Print and PDF export, DXF export and import, and underlay pictures.
- **Plan files**: `.psplan` is plain JSON with typed slots. Saves are atomic, keep the newest 20 previous versions in an
  `Archives` folder, and offer an autosave back after a crash.
- **Samples**: ready-to-open plans in [`samples/`](samples/README.md) (ranch, two-story colonial, studio ADU, large house).

The [reference manual](docs/manual/00-index.md) has a chapter for each area, with an honest status mark on every feature.
It is also built into the program (Help > Launch Help).

## Chief Architect content

Plan Studio reads Chief Architect's libraries, textures and templates from **your own Chief install, at run time, in
place**. Nothing from Chief is bundled in this repository, in the release packages or in your plans, and nothing is
copied out of your install: the Library Browser lists your Chief catalogs where they sit, textures are read from your
Chief data folder when it exists, and File > Templates > Import Chief Template and File > Import > Chief Plan read the
files you point them at. Chief `.plan`, `.layout`, `.calib` and `.calibz` files and textures are listed in `.gitignore` and
must never be committed.

**Plan Studio works without Chief installed.** It then uses its built-in defaults (a Chief-style template, about 145
built-in symbols and generated textures). Lookup of the Chief folders is tested on macOS, where Chief keeps them under
`~/Documents/Chief Architect Premier X18 Data` and `~/Library/Application Support`; on Windows and Linux the
Chief-reading features find nothing unless the same folder layout exists, and you get the built-in defaults.
`PLAN_STUDIO_TEXTURES` can name an extra texture folder.

## Download

Releases are built by the Release workflow when a `vX.Y.Z` tag is pushed, and attached to the repository's GitHub
Releases page together with `SHA256SUMS.txt` and the sample plans. No version has been tagged yet, so for now build from
source (next section). When a release exists it carries:

| Platform | Asset | Notes |
|---|---|---|
| macOS (Apple silicon) | `plan-studio-vX.Y.Z-macos-arm64.dmg` and `.zip` | Open the disk image and drag Plan Studio onto Applications. Ad hoc signed, not notarized: right-click the app and choose Open the first time |
| macOS (Intel) | `plan-studio-vX.Y.Z-macos-x86_64.dmg` and `.zip` | The same, for Intel Macs |
| Windows (64-bit) | `Plan.Studio-X.Y.Z-windows-x64.zip` | Unzip and run `plan-studio.exe`. Unsigned, so SmartScreen warns once |
| Linux (x86_64) | `plan-studio-vX.Y.Z-linux-x86_64.tar.gz`, and an `.AppImage` when that build step succeeds | Unpack the tarball and run `./install.sh` (per-user, into `~/.local`; `--uninstall` removes it), or `chmod +x` the AppImage and run it. Needs the system GTK 3, xkbcommon, X11 or Wayland and OpenGL libraries |

The macOS packages are the primary target. The Windows and Linux packages are built by CI but have not been tried by
hand yet, so treat them as untested until a release note says otherwise.

## Building from source

Install a recent stable Rust toolchain with [rustup](https://rustup.rs). The workspace uses the 2021 edition and egui/eframe 0.31;
the editor needs an OpenGL driver (OpenGL 3.1 or better).

```bash
cargo run -p plan-app            # debug build, opens the editor (the binary is named plan-studio)
cargo run --release -p plan-app  # optimized build: faster 3D and ray tracing
cargo test --workspace           # every crate's tests
```

**macOS**: nothing else is needed. To make the app bundle and the disk image the release uses:

```bash
cargo build --release -p plan-app
CARGO_TARGET_DIR=target sh scripts/macos-bundle.sh release    # target/Plan Studio.app
sh scripts/macos-dmg.sh target                                # target/Plan Studio.dmg, with an Applications shortcut
```

**Windows**: install the MSVC build tools (Visual Studio Build Tools with the C++ workload) and Rust's
`x86_64-pc-windows-msvc` toolchain, then run the cargo commands above. `bash scripts/windows-package.sh` (Git Bash) writes
the release zip.

**Linux**: install the GUI development packages first (the same list CI uses), then run the cargo commands above:

```bash
sudo apt-get install -y libgtk-3-dev libxkbcommon-dev libwayland-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl1-mesa-dev
```

`bash scripts/linux-appimage.sh` writes the release tarball (and the AppImage when `appimagetool` is on `PATH`).
Other distributions need the equivalent packages. CI runs fmt, clippy and the tests on macOS, Linux and Windows
(`.github/workflows/ci.yml`); the Release workflow is described in the [release checklist](docs/release-checklist.md).

## Hotkeys

Plan Studio's hotkeys are Chief Architect X18's, taken from the maintainer's own Chief setup
([docs/chief-hotkeys-resolved.md](docs/chief-hotkeys-resolved.md)), with a few Plan Studio aliases (`Space` or `1` is
Select Objects). They are layered: a base table, then the Chief bindings, then your own edits, which Tools >
Toolbars and Hotkeys > Customize Hotkeys writes to `~/.plan-studio/hotkeys.json` (it can import a Chief `UserHotkeys.xml`).
Off macOS, Chief's `Ctrl` is the Control key and the four-modifier chords become `Ctrl+Alt+...`; where two Mac bindings
fold onto one key, the command chord wins (see [DECISIONS.md](DECISIONS.md), item 4). The full table is chapter 13 of the manual.

| Action | Input |
|---|---|
| Tools | `Space` (or `1`) Select, `2` current wall, `3` Hinged Door, `4` Window; every other tool has its Chief hotkey |
| Draw walls | click start, click end, keep clicking to chain, or press-drag-release for one wall; `Esc` to stop; type a length, `Tab`, an angle and `Enter` after the first click |
| Undo / redo | `Cmd+Z`, `Shift+Cmd+Z` or `Cmd+Y` (`Ctrl` instead of `Cmd` on Windows and Linux) |
| Snaps | automatic at 15 degrees (Edit > Snap Settings); hold `Alt` to suspend every snap |
| Zoom / pan | scroll wheel or pinch; middle- or right-drag |

## Architecture

Twenty-two crates in one Cargo workspace. Only `plan-app` and `plan-view3d` touch the GUI; everything else is plain
Rust, tested headlessly. The plan is the model: every view is derived from `plan-core`, lengths are stored in inches
as `f64`, and every Chief tool is one module behind a shared `Tool` trait.

| Crate | What it is |
|---|---|
| `plan-core` | The model (project, floors, walls, openings, rooms, slabs, layers, defaults, units), typed storage slots, geometry, snapshot undo, DXF export |
| `plan-app` | The desktop editor (egui/eframe): tools, dialogs, docks, menus, hotkeys, themes, headless scenario tests |
| `plan-3d` | Plan to triangle meshes and glTF 2.0 export (no GPU code) |
| `plan-view3d` | The egui/OpenGL 3D viewport widget, with shadow maps, ambient occlusion and screen passes |
| `plan-render` | CPU path tracer (next-event estimation, sky, depth of field, denoiser) |
| `plan-roof` | Automatic roofs from a footprint, roof holes, skylights, ceiling planes, dormers |
| `plan-stairs` | Parametric stair engine: IRC checks, landings, curved stairs, ramps, railings |
| `plan-cabinets` | Parametric cabinet engine: face layouts, countertops, labels and macros |
| `plan-electrical` | Devices, plan symbols and 3D fixtures, auto placement, circuits |
| `plan-terrain` | Terrain, roads, contours, terrain walls, and the landscape objects |
| `plan-framing` | Wall, floor and roof framing, manual members, trusses, lumber takeoff |
| `plan-materials` | Materials, 2D hatches, textures, rendering technique presets, sun |
| `plan-library` | The catalog system behind the Library Browser, the User Catalog, and the PNG and JPEG decoder |
| `plan-calib` | Read-only reader for Chief `.calib` and `.calibz` catalogs |
| `plan-chiefplan` | Read-only scanner and decoder for Chief `.plan` and `.layout` files, and the plan importer |
| `plan-config` | Reads Chief's hotkeys, toolbars and preferences |
| `plan-docs` | Schedules, materials list, the scaled plan-sheet PDF |
| `plan-layout` | Headless layouts: pages, boxes, title blocks, macros, automatic scale, construction set PDF |
| `plan-elevation` | Hidden-line elevations, sections and plan overhead drawings |
| `plan-import` | DXF reader, CAD objects from drawings, CAD to Walls, OBJ and glTF model readers |
| `plan-check` | Plan Check and Door/Window Check rule engine (52 rules) |
| `plan-spaceplan` | The Space Planning Assistant (room boxes to a first plan) |

See [docs/architecture-tools.md](docs/architecture-tools.md) and chapter 14 of the manual for how a tool, a dialog and a
test fit together.

## Documentation

[docs/README.md](docs/README.md) is the index of everything under `docs/`. The main entry points:

- [Reference manual](docs/manual/00-index.md): 19 chapters from first launch to contributor notes, built into the program.
- [Architecture of the tools](docs/architecture-tools.md) and [contributor notes](docs/manual/14-architecture-for-contributors.md).
- [Parity specifications](docs/parity/) and the [parity status](docs/parity-status.md) table that counts them.
- [DECISIONS.md](DECISIONS.md): open and settled decisions, one row each, with the assumption built into the code.
- [Integration queue](docs/integration-queue.md): wiring left for the owner of a shared file.
- [Release checklist](docs/release-checklist.md), [CHANGELOG.md](CHANGELOG.md) and [ROADMAP.md](ROADMAP.md).
- [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) and [SECURITY.md](SECURITY.md).

## Contributing

Issues and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) covers the toolchain, the workspace layout,
the test layers, how to add a tool or a dialog, and the pull request checklist. Please read the
[Code of Conduct](CODE_OF_CONDUCT.md) first.

## License

MIT. See [LICENSE](LICENSE). No Chief Architect code, assets or catalog content ships in this repository.
Plan Studio is an independent open-source program, not affiliated with or endorsed by Chief Architect, Inc. The same
notice is in Help > About Plan Studio.
