# Changelog

All notable changes to Plan Studio are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

No version has been tagged yet (the workspace is `0.1.0`, pre-alpha), so the work is
recorded by **Round**: the numbered chunks of work that each landed as one commit. A Round
heading names its commit, and the first release (`v0.1.0`) will fold all of them together
(see [docs/release-checklist.md](docs/release-checklist.md)). Details of every feature are in
the [manual](docs/manual/00-index.md); the plan for what is next is in [ROADMAP.md](ROADMAP.md).
Test counts are the workspace totals the commit messages and manual state.

## [Unreleased]

Round 9 is in flight. Nothing below has landed.

### Added

- This changelog and [docs/release-checklist.md](docs/release-checklist.md) (how to cut a release, the licensing note and the manual QA list).

### Changed

- The manual brought up to Round 8: a rewritten chapter 7 (stairs), the terrain and landscaping half of chapter 9, pictures, billboards and
  distributions in chapter 6.7, and the typed CAD storage, shared undo stack, Windows and Linux `Ctrl+Z` rule, schedule selection and scenario results across
  chapters 0 to 5, 8, 10 to 15 and 17. README and ROADMAP refreshed (1,852 tests).

### Planned

- 3D picking: click an object in the 3D view to select it in the plan.
- Selecting, moving and deleting single terrain objects (walls, beds, plant runs) with Select Objects.
- Landscape materials: a green grass, mulch and foliage in 3D in place of the brown stand-ins.
- Billboard pictures that turn to face the camera in the cached 3D scene.
- A sweep of the open items in `docs/integration-queue.md` (for example listing saved pictures in the Library Browser).
- A performance pass.
- A parity audit against Chief Architect X18.
- Documentation brought up to the Round 9 commit.

## Round 8 - 2026-10-08 (`6f6a7b9`)

1,852 tests.

### Added

- **Stairs rebuilt.** Rectangle and polygon landings that join stair sections and take the height of the
  section arriving on them; curved stairs and winders; ramps with landings; Click Stairs; a Wall, Railing
  (newels, balusters, rails; panels, solid, cable or glass infill) or Half Wall on each side; open or closed
  risers; closed, open or no stringers; lock settings, Bottom Height and Top Height and Fit to floor-to-floor;
  the Staircase Specification (General, Style, Newels/Balusters, Rails, Line Style, Fill Style, Materials, Label)
  and the Landing Specification. Stairs, ramps, landings and their railings are in the 3D scene.
- **Terrain and landscaping.** Straight and curved terrain walls and curbs, Elevation Spline, Terrain Break,
  Rectangular, Kidney Shaped and Spline features, garden beds, grass regions, water features, stepping stones,
  spline roads, driveways and sidewalks, plant and sprinkler runs; each on its own layer, edited in an object
  specification dialog. The terrain surface, roads and landscape objects are in the 3D scene.
- **Images.** Create Image, Create Billboard Image and Create Image Library (saved to `~/.plan-studio/user-library.json`),
  Polyline and Spline Distribution Path and Region with spacing, offset, scatter and seeded random patterns, and 3D Solid
  Feature. PNG pictures are textured in the plan; JPEG pictures are a framed placeholder (no JPEG decoder yet). Distributions
  follow their record when it moves.
- **Typed CAD storage.** CAD object styles, CAD blocks, text macros and note types are typed fields of the plan
  (`Floor.cad_attrs`, `Floor.cad_blocks`, `Project.text_macros`, `Project.note_types`), with a one-time migration of the old hidden `CAD, Data` records.
- **Schedules in the normal selection** (`ObjectRef::Schedule`): Select Objects picks, moves, deletes and opens a placed schedule.
- **Tools > Project Information** is its own action rather than a tool.
- A hidden **Standard Area** column for the Room Schedule.
- Manual brought up to Round 7, with a new exterior-details chapter (17).

### Changed

- **Plan and layout share one undo stack.** Edit > Undo, `Cmd+Z` and the layout toolbar step back through both in the order the edits were made.
- **Hotkeys off macOS:** Daniel's Control+Z (Down One Floor) and Command+Z (Undo) fold onto one `Ctrl+Z` on Windows and Linux; the Command chord
  wins, so `Ctrl+Z` is Undo and Down One Floor is reported as unmapped there (`55aabaf`; `DECISIONS.md` item 4).
- No flyout entry of the built tool groups is a stub any more (`ALLOWED_NOT_IMPLEMENTED` in `toolbar.rs` is empty).
- Auto Stairwell now cuts a hole in the platform of the floor above as well as adding the room-divider walls; the hole and walls follow the stair.

### Fixed

The seven findings of the first scenario pass (`docs/qa-findings.md`):

- QA-01: doors swing and hinge from where the pointer is on the wall.
- QA-02: a room's own floor offset and ceiling height reach the 3D platforms.
- QA-03: the Room Schedule reports the Interior Area, and its ceiling column reads a room's override.
- QA-04: Auto Stairwell cuts the opening in the upper floor in 3D.
- QA-05: cabinets are in the 3D scene.
- QA-06: stairs are in the 3D scene.
- QA-07: each roof mode names itself after its toolbar entry.

## Round 7 - 2026-10-08 (`8112e1c`)

1,728 tests.

### Added

- **Layout view.** File > New Layout from Daniel's 18 x 24 template, page tabs, boxes you move and resize with handles, the Send to Layout
  dialog (plan views and cameras), Layout Box Specification, Page Setup, Layout Page Table, Print Layout and Export Layout PDF. The layout is saved in the plan.
- **Schedules in the plan.** Door, window, room, cabinet, electrical, framing, plant, fixture, furniture and general schedules placed as live tables,
  with D01 / W03 / C-01 / F-01 callout labels and the Schedule Specification.
- **Project Information** dialog (client, project, designer, revisions, custom fields) feeding the title block macros.
- **CAD, Text and Dimension completeness:** fillet, chamfer, offset, trim, extend, break, CAD blocks and their manager, hatch fills, boxes and insulation,
  rich text, text macros, note types, story pole and elevation dimensions, styled CAD drawing in the plan.
- **Cabinet completeness:** fillers, corner and blind cabinets, custom countertops, backsplashes and counter holes, Generate Countertop, appliance
  openings, the face editor, door and drawer styles, moldings and materials tabs, sinks and cooktops.
- Details selection with handles and wall-following trim; lights as a typed slot; Wall Bottom Height; Explode Dormer keeps its walls; an Adjust Lights hotkey.
- 94 headless scenario tests driving every tool, and `docs/qa-findings.md` (seven open findings).

### Fixed

- The macOS-only hotkey test imports are gated so Windows and Linux clippy pass (`0834122`).

## Round 6 - 2026-10-08 (`9134a2a`)

### Added

- **Template seeding:** new plans start from Daniel's Chief template (Chief `.ini` default templates, decoded wall stacks, Avenir text styles, dimension sets); a
  Preferences > Templates page; File > New Layout prints the 18 x 24 template layout.
- Framing and foundation objects in the normal selection, with handles; the Ceiling Plane dialog.
- Roofs: Join Roof Planes, Auto Floating Dormer, Build Ceiling Planes, Extend Slope Downward, Auto Roof Return.
- **Exterior details:** corner boards, quoins, moldings, floor and wall material regions, wall hatching, polygon decks and 3D solids, with plan, 3D and dialogs.
- Vector View and Technical Illustration elevations drawn from `plan-elevation` in the 3D panel; wall elevation and auto elevation cameras; camera-backed layout boxes;
  walkthrough paths that play and record; Add and Adjust Lights and the Sun Angle feeding the ray tracer and shadows.
- Manual chapters, README and ROADMAP brought up to Round 5; 3D menu auto elevations and walkthroughs live.

### Fixed

- `plan-chiefplan` inventory paths use forward slashes on every platform (Windows CI, `d63f0fe`).

## Round 5 - 2026-10-08 (`b038749`)

### Added

- **Chief catalogs in the Library Browser:** Core, Bonus, Manufacturer and User catalogs read at runtime and never copied, with thumbnails, search,
  2D symbols, 3D meshes and Replace From Library.
- **Wall kinds:** foundation, pony, glass, glass pony, half-wall, room divider, deck railing and edge, fencing, and curved variants, in plan, 3D and the dialog.
- **Slab, footing, pad, pier and platform-hole tools** with specification dialogs, selection and 3D.
- **Roof features:** holes and skylights, ceiling planes, auto dormers, gable lines, roof returns, per-edge Build Roof overrides.
- **Framing:** manual members, direction and bearing lines, reference markers, trusses and a material list; 19 framing tools.
- Elevations: material hatches, section poche and back-clip, shadows, depth weights and labels; camera dialog options.
- Layouts: PDF clip rectangles, layer colors and weights, Daniel's 18 x 24 title block, a template page, poche fills, automatic scale.
- Chief template decode: 103 wall types, 15 text styles, 14 dimension sets.

### Changed

- Storage moved to typed slots: roofs, electrical, terrain, foundation, room, opening and wall extras, and section lines.
- Hotkey tests are platform-independent (Windows and Linux CI); connect distance and tee split come from the plan defaults.

## Round 4 - 2026-10-08 (`0d9ce61`)

### Added

- Wall flags, a hinge-side control, live hotkey labels, 3D View Defaults (`Cmd+1`), and the Saved Dimension Defaults, Room Types and Text Styles editors.
- DXF export and import, CAD to Walls, elevation DXF export, Build Framing with a takeoff; view toggles (Color, Line Weights, Drawing Sheet, Print Preview,
  Reference Display) that draw; Project Browser cameras and views.
- `plan-core` typed slots (roofs, electrical, framing, terrain), room, opening and wall extras, and section lines; the `plan-docs` PDF writer (RGB, dashes, clip,
  rotated and bold text, images, the full sheet and scale catalog); Chief meshes placed through `plan-3d`.

### Changed

- Cleanup from a manual audit; docs refreshed (README, ROADMAP, parity, architecture).

### Fixed

- Windows: `.gitattributes` forces LF and the Chief config parsers tolerate CRLF; settings paths and modifier mapping for non-macOS.

## Round 3 - 2026-10-08 (`b5ba113`)

958 tests, clippy `-D warnings` clean.

### Added

- **Every tool family**, wired by an integration pass: select and edit for every object kind with handles, Tab, marquee and the edit toolbar; wall auto-connect;
  dimensions (manual, end to end, interior, point, baseline, running, centerline, angular, automatic); text, leaders, callouts, markers and notes; the full CAD set
  with arc modes and splines; cabinets with Chief's face-item editor; library placement with wall auto-rotate; stairs with Auto Stairwell; roofs (Build Roof from
  per-wall directives, manual planes, holes, skylights); electrical with auto outlets; terrain with contours; rooms, floors and foundations; space planning, plan check, schedules, the materials list and the construction set PDF.
- Specification dialogs for every object; Default Settings; Customize Hotkeys loaded from Daniel's Chief hotkey file; Active Layer Display Options with layer sets; the Project and Library Browsers.
- **The 3D view:** orbit, doll house, full camera, elevations, camera objects, cross sections, nine rendering techniques, ray trace and glTF export.
- `plan-calib` reads all 403 Chief catalogs (decoded sizes, meshes and plan symbols); `plan-chiefplan` reads Daniel's template (34 layer sets, text styles, dimension sets).
- Low-glare UI, Daniel's Chief defaults and Import Chief Template; metric units; walkthrough paths; railings and decks; roof framing; samples; the release workflow; a 16-chapter user manual.

## Round 2 - 2026-10-08 (`337e9f6`)

483 tests.

### Added

- **The tool-plugin editor:** `editor/` (context, selection, snap engine, edit handles, temporary dimensions with type-to-move, undo and redo, mitered layered-wall rendering)
  and `tools/` (Select with Chief's perpendicular move, joined-end stretch, marquee and Tab cycling; Wall with click-drag and click-click and T-splits; Door and Window with a ghost and distance readouts).
- The Wall Type Definitions dialog with a main layer and resize-about.
- Parity-driven `plan-core` work: wall flags, types, curves and roof directives; layered joins aligned at the main layer; four-state swings; inner-face room polygons; living area;
  floors and foundations; cameras; placed symbols; groups; the clipboard.
- **New engines** (the commit subject counts twelve): `plan-terrain`, `plan-materials`, `plan-layout`, `plan-electrical`, `plan-spaceplan`, `plan-check`, `plan-render` (CPU path tracer), `plan-config`
  (Daniel's Chief hotkeys, 143 of 208 named, and four toolbar sets), and an expanded `plan-library` (147 symbols).
- Docs: Chief library format research (`.calib` is SQLite, `.calibz` is a zip) and Daniel's Chief setup inventory.

## Round 1 - 2026-10-07 (`a373ce1`)

### Added

- Engine crates, all dependency-free and unit-tested: `plan-core` (undo and redo history, mitered wall joins, dimensions, CAD and text, layers, DXF export, Daniel's Chief X18 plan defaults),
  `plan-3d` (wall, opening, floor and ceiling meshes with holes; glTF export), `plan-view3d` (the egui/glow 3D viewport with Chief camera modes), `plan-roof` (weighted straight-skeleton
  automatic roofs, per-edge pitch), `plan-cabinets`, `plan-stairs` and `plan-framing` (parametric engines with 2D symbols, 3D meshes and takeoffs), `plan-docs` (door, window, room and wall schedules,
  materials list, PDF sheets), `plan-elevation` (hidden-line elevations and sections), `plan-import` (DXF reader, CAD to walls), `plan-library` (catalog format, search, 42 starter plan symbols),
  and a `plan-config` skeleton.
- Chief-style specification dialogs (Wall, Door, Window, Default Settings), startup from Daniel's Chief defaults, brighter icons with white halos, higher-contrast UI text.
- Docs: Chief parity behavior specs (`docs/parity`), Daniel's Chief setup inventory, the tool-plugin architecture plan.

## Round 0 - 2026-10-07 (`e4c12b7` to `7155648`)

The foundation, before the work was numbered.

### Added

- `plan-core` (walls, doors and windows, floors, project JSON, 2D geometry, feet-and-inches units, automatic room detection by planar face tracing that handles T-junctions and crossings; 10 unit tests)
  and `plan-app` (an egui/eframe desktop editor with a grid, endpoint, grid and 15 degree angle snaps, continuous wall drawing, door and window placement, selection, live room labels, save and load, and a Chief-style status bar).
- README, a phased ROADMAP mirroring Chief Architect's Build tools, the X18 UI study notes, a macOS bundle script, and CI for macOS, Linux and Windows.
- Chief-style toolbars (three bars), flyouts with variants and hotkeys from Chief's own command list, 152 original SVG icons, the full menu bar with Build, CAD and Terrain menus
  generated from the toolbar tables, two-key hotkey sequences, and the Low Glare canvas theme (with Paper, Dark and High Contrast) and a UI brightness slider (`32779e7`).
- Analyses of Chief X18's toolbars, menus, sub-tools and specification dialogs; a dimension dialog analysis and a shared dialog frame spec; a screenshot of the toolbars (`0e4dbea`, `9f8bd17`).
- The open-decisions log, `DECISIONS.md` (`7155648`).
