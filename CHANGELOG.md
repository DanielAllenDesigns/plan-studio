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

Round 11 is partly done: what has landed is under "Round 11" below. The rest is still being built in the working tree and is listed here, so nothing in this section is in the program yet.

### In flight

- Reading a Chief `.plan` for its content, not only its defaults (`plan-chiefplan::import` exists as a library module; no menu command calls it yet).
- Electrical and framing follow-ups (`plan-electrical` device finishes and meshes; `plan-framing` corner and tee backing, a header table, birdsmouth and tail cuts, eave framing, floor holes).
- The Library Browser work (a user library, model import from OBJ and glTF through `plan-import`, archive, browse, manage and preview modules in `plan-library`).
- Theme, docks and toolbar rendering changes (new and redrawn icons, `shell/docks.rs`, `shell/status.rs`, `shell/tooltips.rs`).

### Planned

- Open doors, casing, sills and thresholds in the editor's 3D view; threshold marks in plan; a transom over a door.
- Attic floors from Build Roof.
- Replace Fonts; ties from dimensions to stairs, roof planes and framing.
- Layout: Save As Template, opening labels on pages, CAD-detail boxes from the Send to Layout dialog, and the File and Layout menu rows for Print Model, Layer Display Options, Add Sheet Index and Print Image of the 3D view.
- Bump maps, and the ray tracer drawing pictures with their bitmaps.

## Round 11 - 2026-10-08 (working tree; the commit is not made yet; part of the round is still in flight)

About 2,750 tests (2,821 `#[test]` functions less 45 that are `#[ignore]`d, at the last count; the number is still rising as the unfinished Round 11 work lands, counted from the source, not from a `cargo test` run; Round 10 was about 2,430). Run `cargo test --workspace` for the exact number before tagging.

### Added

- **Textures in 3D** (`plan-library/image/`, `plan-materials/textures.rs`, `plan-view3d/{texturing, gpu}`, `plan-render/albedo.rs`, `shell/view3d_panel/textures.rs`): 16 of the 23 scene materials show a bitmap in the Standard and Physically Based techniques and in the ray tracer. Chief's own texture files are read at run time from your install (`PLAN_STUDIO_TEXTURES`, `~/Documents/Chief Architect Premier X18 Data/Textures`, `/Library/Application Support/Chief Architect Premier X18/Referenced Files`) and never copied; a generated bitmap stands in for any that is missing. Planar mapping at the file's real tile size (`Brick(36).jpg` repeats every 36"), mipmapped and anisotropic on the card, at most two uploads a frame, decoding on a background thread, a Textures switch in the 3D bar. Pictures and billboards show their own PNG or JPEG bitmap. A decoder written for the program reads PNG (every type and depth, Adam7) and JPEG (baseline and progressive, CMYK, any subsampling). Limits: no bump maps, a painted object shows the texture of the closest scene material, pictures are flat quads in the ray tracer, the mapping is planar, not triplanar.
- **Doors and windows** (`plan-core/openings/spec.rs`, `plan-3d`, `dialogs/opening.rs`): the Sash, Lites (Standard, Diamond, Prairie, Custom Grid), Lintel with an exterior sill, Arch (Round Top, Segmental, Tudor, Gothic, Eyebrow), Hardware and Shutters tabs, built in 3D and so in the elevations; Calculate from Width for door panels; a wall niche's depth; a door mulled with sidelite windows in one frame and one casing; standard widths per style with snap-to-standard on the jamb handles; the Doors, Labels and Windows, Labels layers with a draggable label and Reset Label Position; the new opening is selected after it is placed.
- **Roofs** (`plan-3d/{cover, eave}.rs`, `plan-roof/{edges, spec}.rs`, `dialogs/{roof, defaults}.rs`): Roof Defaults (Edit > Default Settings > Roofs, and a Detail tab in Build Roof) with the eave cut (plumb, level, square), fascia, soffit (level or sloped), frieze, ridge caps, gutters, flashing, exposed rafter tails, Auto Attic Walls, the attic and lower wall types, Roof Cuts Wall at Bottom and the baseline-at-top-plate rule; per-plane eave choices on the Roof Plane Specification's Options tab; half, pony, foundation and curved walls cut by the roof.
- **Rooms** (`plan-core/{extras, rooms}.rs`, `plan-3d/slab.rs`, `editor/{roof_view, snap}.rs`, `shell/docks.rs`): Roof Over This Room off leaves the room out of Build Roof; Flat Roof Over This Room and the Flat Roof room type; concrete stem walls in 3D under a dropped garage floor or a room with a Stem Wall height; snapping to the ends and crossings of the Reference Display floor's walls; a Ref column in Active Layer Display Options; `Shift+Cmd+Y` bound to Floor Defaults as a base key.
- **File management** (`plan-app/{files, mac_open}.rs`, `dialogs/unsaved.rs`, `plan-core/io.rs`): saves go to a temporary file renamed over the plan; the replaced version is copied to `Archives/<plan>/` (the newest 20); Save a Copy, Revert to Saved, Close Plan, Backup Entire Plan (a zip of the plan and its pictures), Manage Auto Archives; autosave every 5 minutes while there are unsaved changes, never over the file; recovery offered when an autosave is newer than the plan and after a crash or a skipped prompt (`~/.plan-studio/recovery/`, the newest 10, a panic hook); Open Recent with Clear Menu; drag and drop; a Finder double-click through an Apple Event handler (and the `.psplan` document type in the macOS bundle; a MIME file for Linux); unsaved-changes prompts for New, Open, Close and Quit (`Enter` Save, `Cmd+D` Don't Save, `Esc` Cancel); a dot in the window title; "Saved 2 min ago" in the status bar. Times in names and prompts are UTC.
- **Dimensions and snaps** (`plan-core/{dimension, dim_assoc, export/dxf}.rs`, `tools/dimension.rs`, `editor/snap.rs`): Auto NKBA Dimensions; curved walls in Auto Exterior; Locate Objects groups for temporary and elevation dimensions; printed-size text picked by the box it is drawn in; DXF and PDF output that honor hidden extension lines and text-style sizes; Reverse Dimension, Convert to Manual, Align and Distribute Dimensions on the Edit toolbar; CAD intersection snaps, an Extension snap (off by default) and a Points/Markers snap.
- **Layout** (`plan-layout/{annot, layers, textfit, print}.rs`, `shell/layout_window.rs`, `dialogs/{layout, print}.rs`): page circles, arcs, leaders and revision clouds with move, resize and nudge; the layout's five-layer set with Layout Layer Display Options; text boxes that wrap, shrink to fit or stay as typed; every door and window symbol and its casing on pages; JPEG as well as PNG picture boxes; a DPI and sample count for each perspective box and an Update Views that renders on a thread with a progress bar; Print Model; a printer list (`lpstat -p`) for the system printer; the sheet index as a table box; Daniel's sheet set (ten sheets for a one-floor plan) installed into the live layout by Create Construction Set; Send to Layout from a Project Browser camera. The engine and dialog for Print Image of the 3D view are in, but the File menu does not call them yet.

### Changed

- The Release workflow (`.github/workflows/release.yml`) runs `cargo test --workspace` on each runner before it builds a package.
- Create Construction Set now adds the sheets to the plan's layout as well as offering the PDF copy.
- The manual brought up to Round 11 so far: chapters 1 to 5, 8, 10 to 15 (file management is the new 12.2a, Roof Defaults 8.4a, textures 10.8a), and the status table of chapter 0. The parts of Round 11 that are still being built are not described.

### Known issues

- The standard widths and the tab values new openings start with are not yet saved with the plan defaults (`docs/integration-queue.md`).

## Round 10 - 2026-10-08 (working tree; the commit is not made yet)

About 2,430 tests. The figure is counted from the source (the `#[test]` functions that are not `#[ignore]`d), not from a `cargo test` run; the same count matched the reported totals within ten at Rounds 7 and 8. Run `cargo test --workspace` for the exact number before tagging.

### Added

- **Edit commands** (`editor/{clipboard, edit_commands, transform}.rs`, `plan-core/transform.rs`): Cut, Copy and Paste (the copy hangs on the pointer until a click), Paste Hold Position,
  Paste Special > As Group, Copy and Paste in Place (`C, P, P`), Duplicate (`Cmd+D`, 12" right and down), Select All (`Cmd+A`), Select Same Type, Group and Ungroup (`Cmd+G`), Delete Objects (`Shift+Space`, by object type),
  Transform/Replicate Object (move, rotate, resize and reflect with copies as arrays), Rotate and a rotate handle on multi-selections, Reflect About Object, Point to Point Move, Center Object,
  Make Parallel and Make Perpendicular, Align and Distribute, Move to Front and Back, Lock and Unlock (by layer), Send to Layer and the Action History window. A right-click context menu opens the same commands. Every command has an id, a hotkey name and a menu row.
- **Doors and windows** (`plan-core/{openings, opening_symbol}.rs`, `editor/{opening_view, opening_edit}.rs`, `dialogs/opening.rs`, `plan-3d`): ten door styles and eleven window flavors as tools, each with a plan symbol and a 3D unit;
  size or `D01` / `W01` labels in plan with the Label tab; jamb resize handles, typed widths and jamb distances; Flip Hinge, Reverse Side, Center on Wall Segment, Mull and Unmull. The variant default sizes are estimates (`DECISIONS.md`).
- **Walls** (`tools/wall.rs`, `editor/{typed_input, wall_edit, behaviors}.rs`, `dialogs/{snap_settings, edit_behaviors}.rs`): typed length and angle while drawing and while dragging a wall end, Shift (hold the angle increment) and Alt (suspend every snap), an angle label,
  Edit > Snap Settings (object snaps one by one, grid and angle snaps, bumping, snap distance), Edit > Edit Behaviors, Break Wall, Remove Break, Reverse Layers, Change Line/Arc with a bulge handle, Make Arc Tangent, Convert to Polyline, Fix Wall Connections, and the Arc section of the Wall Specification.
- **Dimensions** (`plan-core/{dimension, dim_assoc, text_styles}.rs`, `tools/dimension.rs`, `dialogs/{dimension, default_lists}.rs`): Locate Objects settings; dimensions tied to walls, openings, cabinets and fixtures that follow them; Auto Exterior Dimensions in up to three strings per side
  (openings, wall to wall, overall) for any direction of wall, so a turned shell works; an openings string in Auto Interior Dimensions; printed-size text and dimensions (Text Styles Character Height or Printed Size); the Dimension Specification's located objects, per-point extension line toggle and Text Style tab.
- **Roofs in 3D** (`plan-3d/{clip, cover, eave}.rs`, `plan-roof/edges.rs`): walls follow the roof (gable triangles, hip clipping, interior walls rising to a vaulted ceiling, attic walls, butting roofs trimmed with flashing), and the eave detail (fascia, soffit, rake boards, optional frieze, ridge and hip caps).
  Limits: curved, pony, half and foundation walls keep flat tops; the eave cut and rafter tails are not drawn yet.
- **Rooms and floors** (`editor/rooms_edit.rs`, `dialogs/{room, floor, floor_defaults, reference_display}.rs`): room function defaults (a garage floor 24" down on a slab, no ceiling over a deck or porch, no floor under Open Below, Attic and Courtyard rooms), Floor Defaults, Build New Floor with derive options and a foundation,
  Insert New Floor Below, the Reference Display dialog (any floor, a layer set, a color), nested rooms, draggable room labels with a label template (`<name> <type> <area> <std_area> <cl_area> <dims> <ceiling> <floor> <perimeter>`), and Floor and Ceiling Structure Define.
- **Layout, print and materials** (`shell/layout_window.rs`, `dialogs/{print, materials}.rs`, `plan-layout/print.rs`, `plan-docs/materials.rs`): text boxes, perspective camera boxes (ray traced, 480 x 360 by default), picture boxes (PNG), Materials List boxes, a layout CAD row (line, rectangle, polyline, text), a rotation knob;
  the Print dialog (PDF, system printer or viewer; paper, scale, tiling, color, line weights, page range), Print Preview, Print Image and PDF bookmarks; a Materials List with eleven categories, waste factors, stock lengths and unit prices from a Master List
  (`~/.plan-studio/master-list.json`), CSV, PDF and Send to Layout; the construction set gains a Materials List sheet (eight sheets for a one-floor plan).
- **Underlays** (`plan-core/underlay.rs`, `tools/underlay*`, `dialogs/underlay.rs`): pictures (PNG, JPEG, scanned PDF) placed under the plan for tracing, with two-point calibration, opacity, rotation, show and lock.
- **Materials tools** (`tools/materials.rs`, `dialogs/materials.rs`, `plan-core/object_materials.rs`, `plan-materials/painter.rs`): Materials..., Material Painter, Material Eyedropper, Adjust Materials..., Material Builder... and Delete Surface. The 3D view shows a painted object in the closest of its fixed scene materials (no textures yet).
- **Preferences and menus** (`dialogs/{preferences, app_info}.rs`, `menus.rs`): Preferences (appearance, colors, library, folders, render, edit, snaps, architectural), Open Recent Documents, View File Information, Color Chooser, New Plan View, Refresh Display (`F5`), Fill Window Selected Objects,
  Status Bar and Toolbars toggles, Enter Full Screen, the Help menu and System Information. Chief menu rows with no Plan Studio counterpart are removed rather than dimmed (`DECISIONS.md` item 19).
- **Cabinets**: depth and corner resize handles, fit to a gap, and automatic joining of touching base countertops (Preferences > Architectural).
- **Hotkeys**: `Cmd+X`, `Cmd+C`, `Cmd+V`, `Cmd+A`, `Cmd+D`, `Cmd+G`, `C, P, P` and `Shift+Space`; the door and window flyout keys, Floor Defaults (`Shift+Cmd+Y`), Preferences, Print and Revision Cloud become live. 121 of Daniel's 143 named bindings work.
- Two scenario files (`s13_opening_variants`, `s14_wall_edit_and_snaps`).

### Changed

- The manual brought up to Round 10 and, where it was behind, to Round 9: the Edit menu and clipboard (ch. 2.5), every door and window style (ch. 3), rooms and floors (ch. 4), dimensions and printed size (ch. 5), the Round 9 cabinet faces and countertop edges (ch. 6), the stairwell guard (ch. 7), the wall Roof tab, Dutch gable and upper pitch (ch. 8),
  roofs in 3D (ch. 8.3, 10), the Print dialog, boxes and Materials List (ch. 11), underlays (ch. 12), the hotkey table (ch. 13), the new modules (ch. 14) and the glossary. README and ROADMAP refreshed.
- `DECISIONS.md` items 11 to 20 record the open questions of this round (Reverse Layers, Shift while drawing, Edit Behaviors, Lock, Duplicate, Resize, painted materials, automatic countertop join, removed menu rows, underlay formats).

## Round 9 - 2026-10-08 (`0ceb404`)

About 1,950 tests (counted from the source as for Round 10; the commit message gives no total).

### Added

- **3D picking.** Click an object in the 3D view to select it (orange highlight), Shift-click to add, double-click to open its specification, `Delete` to delete it.
- **Terrain selection.** Select Objects picks, moves, nudges and deletes single terrain objects (features, breaks, walls, curbs, roads and landscape objects), with vertex handles on small ones.
- **Landscape materials.** Grass, Mulch, Foliage, Water, Asphalt and Gravel in the 3D scene in place of the brown stand-ins; billboards turn to face the camera in the live view.
- **Queue sweep.** Roofs: the Roof Return settings dialog, Dutch gable and upper pitch (gambrel and mansard) roofs, knee walls and the Roof tab of the Wall Specification. Cabinets: editable side and back faces, countertop corner treatments and an Ogee edge, the Soffit Polygon tool.
  Dimensions and text: associative dimensions, Find/Replace Text, Edit toolbar buttons for the CAD edit tools. Schedules: click a row to select the object, grouping and totals, Stair, Room Finish and Note schedules, schedule layout boxes. Layout: box rotation and portrait sheets, the extra Project Information macros, layout JSON import and export.
  Stairs: a guard railing around the stairwell opening, dashed treads beyond the break line, landing corner handles. Details: own line styles, molding miters, custom molding profiles. A dimension pasted from the clipboard is not tied to the walls the original was.
- **Performance.** Linear room detection and wall joins, cached schedules, framing and layout signatures, a 591-wall sample and a benchmark, a release profile.
- `docs/parity-status.md` (765 parity ids), the manual for Round 8, the changelog and the release checklist; the bundle version comes from `Cargo.toml`.

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
