# Roadmap

Chief Architect X18 is the reference. The feature inventory below is organized
the way Chief organizes its Build toolbar, and ordered by what a residential
designer needs first. `[x]` is done and in the editor; `[ ]` is open. Items
that exist as an engine in a crate but have no screen yet say so. The detail
behind every line is in the [manual](docs/manual/00-index.md) (each feature
carries a status mark) and in `docs/integration-queue.md`.

## Rounds

The work lands in numbered rounds, one commit each. The phases below show the feature inventory; this section shows
what the latest rounds finished and what is in flight.

### Round 5 (done, commit `b038749`)

- [x] Chief Architect catalogs in the Library Browser: Core, Bonus, Manufacturer and User catalogs read in place, background
      discovery, thumbnails, search, Open Object, a "Catalog folders..." window, decoded meshes in 3D, Replace From Library
- [x] Typed storage: roofs, electrical, terrain, slabs, and room, opening and wall extras and section lines, with automatic migration of old files
- [x] Wall kinds: foundation, pony, glass, glass pony, half-wall, room divider, railing, deck railing, deck edge and fencing, straight and
      curved (three-click curve), with the Wall Class list, pony section, Foundation Height, Half-Wall Height, Fence Style and Rail Style
- [x] Slabs: Slab, Slab with Footing, Slab Hole(s), Square Pad, Round Pier, Hole in Floor and Ceiling Platform, with dialogs and 3D
- [x] Roof features: Roof Hole, Skylight, Ceiling Plane, Delete Ceiling Planes, Auto Dormer, Explode Dormer, Gable Line, Roof Return, and the
      Holes and Build Roof Edge tabs of the Roof Plane Specification
- [x] Framing: 19 manual framing tools, the Framing Member Specification, Build Framing honoring layout objects, Framing Takeoff with
      Export Material List
- [x] Elevations and sections: camera "Elevation rendering" (hatch, shadows from a sun or a date, line weight by distance, labels, back-clip depth)
- [x] Layouts: page background, Layout Edge weight, layer colors, weights and dashes, Daniel's 18 x 24 title block with a REVISIONS table and the
      macros, template page, automatic scale, multi-floor labels with SCALE notes, poche and shadow fills, image boxes
- [x] DXF export includes the Roof Planes and Framing layers
- [x] Chief template decode: 103 wall types, 15 text styles, 14 dimension sets

### Round 6 (done, commit `9134a2a`)

- [x] Template seeding: Chief's default plan and layout templates read from its preferences INI, decoded once into a cache, laid over the defaults;
      the Preferences > Templates page; Set as default in Import Chief Template
- [x] Framing and slab selection in Select Objects, with handles (end handles on members, corner handles on slabs, holes and truss bases)
- [x] Roofs: Ceiling Plane Specification, Join Roof Planes, Auto Floating Dormer, Build Ceiling Planes, Extend Slope Downward, Auto Roof Return
- [x] Exterior details: corner boards, quoins, moldings, floor and wall material regions, wall hatching, polygon decks, 3D solids, with dialogs, plan drawing and 3D
- [x] Vector elevations (Vector View and Technical Illustration) you can open, Wall Elevation, Auto Elevations and Auto Back-Clipped Elevations, walkthrough
      paths with Play and Record, Add Lights and Adjust Lights, the Sun Angle dialog
- [x] Manual chapters brought up to Round 6

### Round 7 (done, commit `8112e1c`)

- [x] The layout view: File > New Layout, page tabs, boxes with handles, Send to Layout, Layout Box Specification, Page Setup, Layout Page Table,
      Print Layout and Export Layout PDF, the Layout menu, Window > Floor Plan View / Layout, the Project Browser Layout section; the layout is stored in the plan
- [x] Schedules placed in the plan (ten kinds) with callout labels and the Schedule Specification; Project Information and the title block macros
- [x] CAD, Text and Dimension completeness: boxes, cross, blocking and insulation boxes, splines, CAD blocks and their manager, fillet, chamfer, offset, trim, extend,
      break, reverse, parallel, perpendicular, converts, hatch, CAD Detail From View, rich text markup and B / I / U, text macros, note types, square callouts, Auto Elevation
      and Auto Story Pole dimensions, the CAD dialog's Line Style, Fill Style and Arrow tabs
- [x] Cabinet completeness: fillers, corner and blind cabinets, custom countertop, backsplash and counter hole, Generate Countertop (`G`), appliance openings, sinks and
      cooktops, the front face editor, door and drawer styles, Opening Indicators, moldings, materials and labels
- [x] Selection of exterior details with handles, details that follow their walls, Wall Bottom Height, Explode Dormer keeps its walls, lights as a typed slot,
      `Ctrl+Alt+Cmd+L` for Adjust Lights
- [x] Scenario tests (twelve files driving the tools headlessly) and `docs/qa-findings.md` (seven findings); the test count is 1,728

### Round 8 (in flight)

- [ ] QA fixes: QA-01 door swing and hinge from the click, QA-02 a room's ceiling height in 3D, QA-03 the Room Schedule's interior area, QA-05 cabinets in the 3D scene,
      QA-06 stairs in the 3D scene, QA-07 the roof tool's names
- [ ] Stairs and railings (stair railings, the stairwell cut in the floor: QA-04, by the stairs builder)
- [ ] Typed CAD slots (own styles, blocks, text macros and note types leave the hidden `CAD, Data` layer), a schedule as a selectable object in Select Objects,
      and one undo model for the layout and the plan
- [ ] The manual brought up to Round 7 (this edit)
- [ ] The remaining stubs: the dimmed buttons still in the flyouts and bars (for example other door and window styles, terrain walls and curbs, the Revision Cloud and Print buttons, Floor Defaults)

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
- [x] Fillet, Chamfer, Offset, Trim, Extend, Break, Reverse, Make Parallel / Perpendicular, converts, Hatch (Round 7)
- [ ] Printed-size text (text that stays 1/8" at any scale), Find/Replace Text, Replace Fonts
- [x] Curved, foundation, pony, glass, glass pony and half walls, room dividers,
      railings, deck railings and edges, and fencing as drawing tools (Round 5)
- [ ] Mitered joins and 3D door and window cuts on curved walls; an editable arc
- [ ] Break Wall, Reverse Layers, the Fix Wall Connections button; typed length while
      drawing
- [ ] Other door and window styles as tools (sliding, pocket, bifold, garage,
      barn, bay, bow, box ...), plan labels, mulling, resize handles
- [x] Floor Material Region (Round 6)
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
- [x] Walls of every class, slabs, pads and piers, roof holes, skylights and dormers,
      manual framing, and placed library symbols (Chief objects with decoded meshes) in the 3D view
- [ ] Stairs, cabinets (QA-05, QA-06: Round 8), electrical devices and terrain in the 3D view (the builders
      exist in their crates); Build Framing's own members in 3D
- [x] Walkthroughs (play and record), Add Lights and Adjust Lights, Create Auto Elevations and Wall Elevation cameras, vector elevations (Round 6)
- [ ] Material Painter and textures in the viewport, 3D picking
- [ ] Walkthrough recording at better than 8 samples per pixel (it uses the path tracer today, 640 x 480, as a PNG sequence)
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
- [x] Dormers (Auto Dormer, Explode Dormer), ceiling planes, roof returns, per-edge
      roof settings, roof holes cut in 3D (Round 5)
- [x] Join Roof Planes, Auto Floating Dormer, ceiling planes from Build Roof (Build Ceiling Planes), Extend Slope Downward, Auto Roof Return, Explode Dormer keeps its walls
- [ ] The Wall Roof tab, Dutch gable, knee wall and upper pitch, gutters, fascia and soffit
- [x] Manual framing tools (General Framing, Post, Joist, Rafter, Roof Truss ...),
      with manual framing in the 3D view and the DXF
- [x] Picking placed framing with Select Objects, with handles (Round 6)
- [ ] A framing defaults dialog, corner and T backing
- [ ] Terrain in 3D, spline terrain tools, Terrain Break
- [ ] Circuits UI and the electrical schedule

## Phase 4 — Documentation (partly done)

- [x] Hidden-line elevations and sections (`plan-elevation`), exported as DXF
- [x] Door, window, room and wall schedules (CSV), Materials List, Framing
      Takeoff (CSV), Plan Check and Door/Window Check reports
- [x] Headless layouts: pages, boxes, title blocks and macros, automatic scale,
      layer colors and weights, poche and shadow fills, construction set PDF
- [x] An interactive Layout in the editor: page tabs, box editing, Send to
      Layout, Open Layout, and the layout stored in the plan file (Round 7)
- [ ] More than one layout per plan; box rotation; text, image, CAD-detail and schedule boxes from the editor
- [x] Print Layout and Export Layout PDF (a PDF of the printed pages; Round 7)
- [ ] Print to a printer, and the row 1 Print button
- [x] Place a schedule on the plan (door, window, room, cabinet, electrical, framing, plant, fixture, furniture, general) with callout labels (Round 7)
- [ ] Room-finish and note schedules, grouping and totals, click-a-row-selects-the-object, schedules in the DXF and the construction set, a schedule layout box
- [ ] Plan notes, callouts and markers tied to elevation and section cameras
- [x] Line weights, colors and dashes in PDF output from the layer pens
- [x] Project Information: client, address, job number, date, revisions and custom fields fill the title block (Round 7)
- [ ] The extra Project Information macros (`%client.phone%`, `%company%`, `%custom.<name>%` ...) in the title block

## Phase 5 — Library and ecosystem (partly done)

- [x] Library Browser with the built-in catalog; `plan-library` catalog system
- [x] `plan-calib` reads Chief `.calib` / `.calibz` catalogs (403 catalogs on
      Daniel's install), with decoded sizes, plan symbols and meshes
- [x] Project templates and plan defaults; imperial and metric dimension units
- [x] Chief hotkeys loaded and customizable
- [x] Chief catalogs in the Library Browser (tree, search, thumbnails, place),
      with placed Chief meshes in 3D and a settings toggle (Round 5)
- [x] Automatic seeding from your Chief default plan and layout templates (Round 6)
- [ ] `capture_typing` for tools that take typed input
- [x] Replace From Library (Symbol Specification)
- [ ] Symbol import (OBJ, glTF, SKP via converter); user library and Add to
      User Library
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
