# Roadmap

Chief Architect X18 is the reference. The feature inventory below is organized
the way Chief organizes its Build toolbar, and ordered by what a residential
designer needs first. `[x]` is done and in the editor; `[ ]` is open. Items
that exist as an engine in a crate but have no screen yet say so. The detail
behind every line is in the [manual](docs/manual/00-index.md) (each feature
carries a status mark) and in `docs/integration-queue.md`.

## Phase 0 — Foundation (done)

- [x] Cargo workspace: now 22 crates (model, editor, and one crate per engine)
- [x] Walls with thickness, height, exterior/interior kind
- [x] Doors and windows hosted in walls, with placement rules
- [x] Automatic room detection from wall centerlines (T-junctions, crossings)
- [x] Feet-and-inches formatting and parsing, JSON save/load (`.psplan`)
- [x] 2D plan editor: grid, snapping, zoom/pan, wall chaining, select/delete
- [x] Status bar with live X/Y coordinates like Chief's
- [x] GitHub Actions CI (fmt, clippy, test on macOS/Linux/Windows) and a tagged
      release workflow

## Phase 1 — A usable 2D plan tool (done, with gaps below)

- [x] Wall joins: mitered corners, clean T-intersections, wall layers (framing,
      drywall, siding) with correct face offsets, Wall Type Definitions
- [x] Select, move, stretch; multi-select; copy/paste in place; handles
- [x] Undo/redo (whole-plan snapshots, named steps)
- [x] Dimensions: manual, end to end, interior, point to point, running,
      baseline, centerline, angular, tape measure, auto exterior and interior,
      temporary dimensions with type-to-move
- [x] Text, rich text, leader lines, callouts, markers, notes; CAD lines, arcs,
      circles, boxes, polylines, splines, revision clouds, CAD blocks
- [x] Room labels: names, interior/standard area, finishes, room types, living
      area; Room Specification
- [x] Layers, layer sets and saved plan views (Active Layer Display Options)
- [x] Default settings dialogs: walls, doors, windows, saved dimension defaults,
      room types, text styles; templates (built-in Chief X18 template, Save My
      Template, Import Chief Template)
- [x] Reference Display of the floor below (walls, in gray)
- [x] DXF export of a floor and of the four elevations; DXF import with a units
      choice; CAD to Walls
- [x] PDF construction set to scale; View toggles (Color, Line Weights, Drawing
      Sheet, Print Preview)
- [ ] DWG import and export
- [ ] Associative dimensions (they do not follow a moved wall yet)
- [ ] Printed-size text (text that stays 1/8" at any scale), Fillet and Chamfer,
      Find/Replace Text, Replace Fonts
- [ ] Curved, foundation, pony, half and glass walls as drawing tools; Break
      Wall, Reverse Layers, the Fix Wall Connections button; typed length while
      drawing
- [ ] Other door and window styles as tools (sliding, pocket, bifold, garage,
      barn, bay, bow, box ...), plan labels, mulling, resize handles
- [ ] Floor Defaults, function-driven room behavior, nested-room holes
- [ ] Reference Display of the floor above, and its floor choice
- [ ] Foundation undo leaves the active floor index clamped instead of shifted

## Phase 2 — 3D from the plan (done, with gaps below)

- [x] Wall solids with openings cut, floors, ceilings, roof planes
- [x] 3D viewport (eframe's glow backend, `plan-view3d`): orbit, doll house,
      perspective overviews, Full Camera, cross sections, elevations
- [x] Nine rendering techniques, Sun Angle, CPU path tracer with PNG output,
      glTF export, camera objects with a Camera Specification
- [x] Cabinets (six kinds, Cabinet Specification, face trees), library symbols
      (about 145 built-in 2D symbols)
- [x] Stairs (straight, L, U, winder, ramp, landing; IRC solver; Auto Stairwell)
- [x] Multiple floors, foundations, Build New Floor, Insert/Delete/Exchange
- [ ] Stairs, cabinets, library 3D models, electrical devices and terrain in the
      3D view (the builders exist in their crates)
- [ ] Walkthroughs, Add Lights, Material Painter and textures in the viewport,
      3D picking, Create Auto Elevations and Wall Elevation cameras
- [ ] glTF/OBJ import for symbols
- [ ] Stair railings, stairwell cut in the floor, Stair Schedule

## Phase 3 — Roofs and structure (done, with gaps below)

- [x] Auto roof from wall baselines (Build Roof): hip, gable, shed, pitch and
      overhang per wall; manual roof planes, gable lines, holes, skylights
- [x] Framing: Build Framing and Build All Framing (walls with openings, floor
      platforms, roofs: rafters, ridge, hips, valleys, trusses), framing drawn
      on the Framing layer, Framing Takeoff with CSV export
- [x] Terrain: perimeter, elevation data, modifiers, features, roads, driveways,
      sidewalks, Build Terrain with contours, terrain hole around the building
- [x] Electrical: devices, Auto Place Outlets, connections
- [ ] Dormers, ceiling planes, gutters, fascia and soffit, Join Roof Planes, roof
      holes cut in 3D
- [ ] Manual framing tools (General Framing, Post, Joist, Rafter, Roof Truss ...)
      and framing in the 3D view
- [ ] Terrain in 3D, spline terrain tools, Terrain Break
- [ ] Circuits UI and the electrical schedule

## Phase 4 — Documentation (partly done)

- [x] Hidden-line elevations and sections (`plan-elevation`), exported as DXF
- [x] Door, window, room and wall schedules (CSV), Materials List, Framing
      Takeoff (CSV), Plan Check and Door/Window Check reports
- [x] Headless layouts: pages, boxes, title blocks, construction set PDF
- [ ] An interactive Layout in the editor: page tabs, box editing, Send to
      Layout, Open Layout, and layouts stored in the plan file (the active
      layout is a session sheet size and scale today)
- [ ] Print and Print Preview as printing (the toggles show the sheet today),
      PDF of the active plan view from the menu
- [ ] Place a schedule on the plan; cabinet, electrical, fixture, framing and
      room-finish schedules
- [ ] Plan notes, callouts and markers tied to elevation and section cameras
- [ ] Line weights in PDF output from the layer pens

## Phase 5 — Library and ecosystem (partly done)

- [x] Library Browser with the built-in catalog; `plan-library` catalog system
- [x] `plan-calib` reads Chief `.calib` / `.calibz` catalogs (403 catalogs on
      Daniel's install), with decoded sizes, plan symbols and meshes
- [x] Project templates and plan defaults; imperial and metric dimension units
- [x] Chief hotkeys loaded and customizable
- [ ] Chief catalogs in the Library Browser (tree, search, thumbnails, place),
      with placed Chief meshes in 3D and a settings toggle
- [ ] First-run auto-scan of Daniel's Chief template; `capture_typing` for tools
      that take typed input
- [ ] Symbol import (OBJ, glTF, SKP via converter); user library and Add to
      User Library; Replace From Library
- [ ] Plugin or scripting layer for custom tools
- [ ] Windows and Linux are built by CI but not yet tried by hand; the hotkey
      and settings paths are written for them

## Technical choices

| Need | Crate |
|---|---|
| GUI | `eframe` / `egui` (glow backend for 3D) |
| 3D rendering | `plan-view3d` (OpenGL through glow), `plan-render` (CPU path tracer) |
| Math | in-house 2D geometry in `plan-core`, small 3D helpers in the engine crates |
| Polygon ops | in-house (room detection, wall joins, ear-clipping triangulation) |
| Export | own writers: ASCII DXF, PDF 1.4, glTF 2.0, PNG |
| File dialogs | `rfd` |
| B-rep CAD kernel (later) | `truck` |
