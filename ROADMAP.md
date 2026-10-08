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

### Round 8 (done, commit `6f6a7b9`)

- [x] QA fixes: QA-01 door swing and hinge follow the pointer, QA-02 a room's floor and ceiling heights in 3D, QA-03 the Room Schedule's interior area (with a hidden Standard
      Area column), QA-04 Auto Stairwell cuts the opening in the upper floor, QA-05 cabinets and QA-06 stairs in the 3D scene, QA-07 the roof tool's names. No scenario is ignored any more
- [x] Stairs: rectangle and polygon landings joined to stair sections, curved stairs and winders, ramps with landings, Click Stairs, a wall, railing (newels, balusters, rails) or half wall on each side,
      open and closed risers, stringer styles, lock settings, Bottom and Top Height with fit to floor-to-floor, the Staircase and Landing Specifications
- [x] Typed CAD slots (own styles, blocks, text macros and note types leave the hidden `CAD, Data` layer, with migration), schedules in the normal selection, Project Information as its own action,
      and one undo stack for the layout and the plan
- [x] Terrain and landscaping: terrain walls and curbs (straight and curved), Elevation Spline, Terrain Break, rectangular, kidney and spline features, garden beds, grass, water features, stepping stones,
      spline roads, driveways and sidewalks, plant and sprinkler runs, an object dialog, layers, and the terrain surface, roads and landscape in the 3D scene
- [x] Images: Create Image, Create Billboard Image, Create Image Library (`~/.plan-studio/user-library.json`), polyline and spline distribution paths and regions, 3D Solid Feature
- [x] Off macOS, Control+Z and Command+Z fold into one `Ctrl+Z`: Undo wins and Down One Floor is reported unmapped
- [x] No flyout entry of the built tool groups is a stub (`ALLOWED_NOT_IMPLEMENTED` is empty); 1,852 tests
- [x] The manual brought up to Round 7, with an exterior-details chapter (and, after the commit, to Round 8; the README, this file, the changelog and the release checklist)

### Round 9 (done, commit `0ceb404`)

- [x] 3D picking: click an object in the 3D view to select it in the plan; Shift-click, double-click and `Delete`
- [x] Terrain selection: pick, move and delete single terrain objects with Select Objects
- [x] Landscape materials (Grass, Mulch, Foliage, Water, Asphalt, Gravel) and billboards that turn to face the camera
- [x] An integration-queue sweep: Roof Return settings, Dutch gable, upper pitch, knee walls and the Wall Roof tab; editable cabinet sides and back, countertop corners and an Ogee edge, the Soffit Polygon;
      associative dimensions, Find/Replace Text, Edit toolbar buttons for the CAD edit tools; schedule row click, grouping and totals, Room Finish, Note and Stair schedules, schedule layout boxes; layout box rotation and portrait, the extra Project Information macros,
      layout JSON import and export; a stairwell guard railing, dashed hidden treads, landing handles; detail line styles, molding miters, custom molding profiles
- [x] A performance pass (linear room detection and wall joins, cached signatures, a 591-wall benchmark)
- [x] A parity audit against Chief Architect X18 (`docs/parity-status.md`, 765 ids)

### Round 10 (done in the working tree; the commit is not made yet)

- [x] Edit commands: Cut, Copy, cursor-attached Paste, Paste Hold Position, Paste As Group, Copy and Paste in Place (`C, P, P`), Duplicate, Select All, Select Same Type, Group, Delete Objects, Transform/Replicate Object, Reflect About Object, Point to Point Move, Center Object,
      Make Parallel and Perpendicular, Align/Distribute, Move to Front and Back, Lock and Unlock, Send to Layer, Action History, the right-click context menu
- [x] Doors and windows: all ten door styles and eleven window flavors with plan symbols and 3D units, labels, jamb handles, typed widths, Mull and Unmull, Center on Wall Segment, Flip Hinge, Reverse Side (default sizes are estimates)
- [x] Walls: typed length and angle, Shift and Alt, the angle label, Snap Settings, Edit Behaviors, Break Wall, Remove Break, Reverse Layers, Change Line/Arc with a bulge handle, Make Arc Tangent, Convert to Polyline, the Arc section
- [x] Dimensions: Locate Objects, associative dimensions, three-string Auto Exterior (turned shells too), the openings string of Auto Interior, printed-size text, the Dimension Specification's located objects, extension toggle and Text Style tab
- [x] Roofs cut walls in 3D (gable triangles, hip clipping, attic walls, butting roofs, soffit, fascia, rake, frieze, ridge caps, flashing)
- [x] Rooms and floors: room function defaults, Floor Defaults, Build New Floor options and foundation, Insert New Floor Below, the Reference Display dialog, nested rooms, draggable room labels and macros, Floor and Ceiling Structure Define
- [x] Layout and print: text, perspective, picture and Materials List boxes, layout CAD, a rotation knob, the Print dialog, Print Preview, PDF bookmarks; the Materials List with a Master List, CSV, PDF and Send to Layout; the construction set grows to eight sheets
- [x] Underlay pictures with calibration, the material tools (Material Painter, Adjust Materials, Material Builder), Preferences, and the rest of the Chief menu bar; cabinet depth and corner handles, fit to a gap and automatic countertop joining
- [x] The manual and this file brought up to Round 10 (and the Round 9 queue sweep)

### Round 11 (partly done in the working tree; the commit is not made yet)

Landed:

- [x] Textures in the 3D view and the ray tracer: Chief's own texture files read from your install at run time (never copied) with generated fallbacks, planar mapping at the real tile size, a Textures switch, pictures and billboards with their own PNG or JPEG bitmaps, and a decoder for PNG and JPEG (baseline and progressive) written for the program
- [x] Door and window follow-ups: the Sash, Lites, Lintel, Arch, Hardware and Shutters tabs built in 3D, Calculate from Width, niche depth, door-plus-sidelite mulling with one shared casing, standard widths with snap, label layers with a draggable label and Reset Label Position, the new opening selected after placement
- [x] Roof and room follow-ups: Roof Defaults (eave cut, rafter tails, gutters, fascia and soffit, Auto Attic Walls, attic and lower wall types, Roof Cuts Wall at Bottom, baseline at the plate), per-plane eave options, half, pony, foundation and curved walls cut by the roof, Roof Over This Room and Flat Roof Over This Room, garage and room stem walls in 3D, snapping to Reference Display walls, the Ref column of Layer Display Options, `Shift+Cmd+Y`
- [x] File management: atomic saves, `Archives/` copies (the newest 20), Save a Copy, Revert, Close Plan, Backup Entire Plan, Manage Auto Archives, autosave every 5 minutes, recovery after a crash, Open Recent with Clear Menu, drag and drop, a Finder double-click, unsaved-changes prompts, the title dot and "Saved n min ago"; the Release workflow runs the tests
- [x] Dimension and snap follow-ups: Auto NKBA Dimensions, curved walls in Auto Exterior, printed-size text picking, DXF and PDF honoring hidden extension lines and text-style sizes, Locate Objects groups for temporary and elevation dimensions, CAD intersection, Extension and Points/Markers snaps, Reverse, Convert to Manual, Align and Distribute Dimensions
- [x] Layout follow-ups: page circles, arcs, leaders and revision clouds with handles, the layout layer set with Layer Display Options, text wrap, shrink and as typed, every door and window symbol on pages, PNG and JPEG picture boxes, per-box DPI and samples with a threaded Update Views, Print Model, the printer list, the sheet index box, Daniel's ten-sheet construction set in the live layout, Send to Layout from a Project Browser camera
- [x] The manual brought up to Round 11 so far (file management 12.2a, Roof Defaults 8.4a, textures 10.8a)

Still in flight (four builders are working in the tree; none of this is in the manual yet):

- [ ] Chief `.plan` import: read a plan's content, not only its defaults (`plan-chiefplan::import` is in the tree; no menu command calls it)
- [ ] Electrical and framing follow-ups (`plan-electrical`, `plan-framing`)
- [ ] The Library Browser work (a user library, OBJ and glTF model import)
- [ ] Theme, docks and toolbar rendering

Left over from the Round 11 plan:

- [ ] Open doors, casing, sills and thresholds in the editor's 3D view, threshold marks in plan, a transom over a door, door sizes checked against Chief
- [ ] Attic floors from Build Roof; bump maps; exact colors for painted materials
- [ ] Replace Fonts; ties to stairs, roof planes and framing
- [ ] Layout: Save As Template, opening labels on pages, CAD-detail boxes from the Send to Layout dialog, the File and Layout menu rows for Print Model, Layer Display Options and Add Sheet Index, Print Image of the 3D view
- [ ] The live manual QA pass on macOS, Windows and Linux ([docs/release-checklist.md](docs/release-checklist.md)), then the first tagged release

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
- [x] Associative dimensions: they follow the wall, opening, cabinet or fixture they were located on (Round 9, extended in Round 10)
- [x] Fillet, Chamfer, Offset, Trim, Extend, Break, Reverse, Make Parallel / Perpendicular, converts, Hatch (Round 7)
- [x] Printed-size text (text that stays 1/8" at any scale; Round 10) and Find/Replace Text (Round 9); Auto NKBA Dimensions, Align and Distribute Dimensions and the Extension and Points/Markers snaps (Round 11)
- [ ] Replace Fonts
- [x] Curved, foundation, pony, glass, glass pony and half walls, room dividers,
      railings, deck railings and edges, and fencing as drawing tools (Round 5)
- [x] Mitered joins on curved walls and an editable arc (Change Line/Arc, a bulge handle, the Arc section; Round 10)
- [ ] 3D door and window cuts on curved walls
- [x] Break Wall, Remove Break, Reverse Layers, Fix Wall Connections, typed length and angle while drawing, Shift and Alt, Snap Settings and Edit Behaviors (Round 10)
- [x] All door and window styles as tools (sliding, pocket, bifold, garage, barn, bay, bow, box ...), plan labels, mulling and resize handles (Round 10)
- [x] Door-plus-sidelite mulling with one shared casing, and the shaping tabs (Round 11)
- [ ] Casing and threshold marks in plan
- [x] Floor Material Region (Round 6)
- [x] Floor Defaults, function-driven room behavior and nested rooms (Round 10)
- [x] Reference Display of the floor above or any floor, and its options dialog (Round 10), and snapping to its walls (Round 11)
- [ ] Foundation undo leaves the active floor index clamped instead of shifted

## Phase 2 — 3D from the plan (done, with gaps below)

- [x] Wall solids with openings cut, floors, ceilings, roof planes
- [x] 3D viewport (eframe's glow backend, `plan-view3d`): orbit, doll house,
      perspective overviews, Full Camera, cross sections, elevations
- [x] Nine rendering techniques, Sun Angle, CPU path tracer with PNG output,
      glTF export, camera objects with a Camera Specification
- [x] Cabinets (six kinds, Cabinet Specification, face trees), library symbols
      (about 145 built-in 2D symbols)
- [x] Stairs (straight, L, U, winder, curved, ramp, landing; IRC solver; Auto Stairwell; railings; Round 8)
- [x] Multiple floors, foundations, Build New Floor, Insert/Delete/Exchange
- [x] Walls of every class, slabs, pads and piers, roof holes, skylights and dormers,
      manual framing, and placed library symbols (Chief objects with decoded meshes) in the 3D view
- [x] Stairs, cabinets, terrain, roads and landscape, pictures and 3D solids in the 3D view (Round 8)
- [ ] Electrical devices in the 3D view (the builder exists in its crate); Build Framing's own members in 3D
- [x] Walkthroughs (play and record), Add Lights and Adjust Lights, Create Auto Elevations and Wall Elevation cameras, vector elevations (Round 6)
- [x] 3D picking (Round 9) and the Material Painter, Adjust Materials and Material Builder (Round 10)
- [x] Textures in the viewport and the ray tracer (Round 11)
- [ ] Exact colors for painted materials, bump maps, pictures in the ray tracer
- [ ] Walkthrough recording at better than 8 samples per pixel (it uses the path tracer today, 640 x 480, as a PNG sequence)
- [ ] glTF/OBJ import for symbols
- [x] Stair railings, walls and half-walls on a stair, the stairwell cut in the floor (Round 8)
- [x] Stair Schedule and a guard railing around the stairwell opening (Round 9)
- [ ] A railing across a landing in the plan symbol

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
- [x] The Wall Roof tab, Dutch gable, knee wall and upper pitch (Round 9); roofs cut the walls in 3D, with fascia, soffit, rake, frieze, ridge caps and flashing (Round 10)
- [x] The eave cut, rafter tails and gutters in 3D, Roof Defaults and per-plane eave options (Round 11)
- [x] Manual framing tools (General Framing, Post, Joist, Rafter, Roof Truss ...),
      with manual framing in the 3D view and the DXF
- [x] Picking placed framing with Select Objects, with handles (Round 6)
- [ ] A framing defaults dialog, corner and T backing
- [x] Terrain in 3D, spline terrain tools, Terrain Break, terrain walls and curbs, landscaping objects (Round 8)
- [x] Selecting single terrain objects with Select Objects (Round 9)
- [ ] Terrain walls that cut the surface and cut-and-fill features
- [ ] Circuits UI and the electrical schedule

## Phase 4 — Documentation (partly done)

- [x] Hidden-line elevations and sections (`plan-elevation`), exported as DXF
- [x] Door, window, room and wall schedules (CSV), Materials List, Framing
      Takeoff (CSV), Plan Check and Door/Window Check reports
- [x] Headless layouts: pages, boxes, title blocks and macros, automatic scale,
      layer colors and weights, poche and shadow fills, construction set PDF
- [x] An interactive Layout in the editor: page tabs, box editing, Send to
      Layout, Open Layout, and the layout stored in the plan file (Round 7)
- [x] Box rotation (quarter turns), portrait sheets, and text, perspective, picture and Materials List boxes with layout CAD from the editor (Rounds 9 and 10)
- [ ] More than one layout per plan; CAD-detail boxes from the editor
- [x] Print Layout and Export Layout PDF (a PDF of the printed pages; Round 7)
- [x] Page leaders, revision clouds, circles and arcs, the layout layer set, text fit, Print Model, a printer list and the construction set in the live layout (Round 11)
- [x] The Print dialog: PDF, the system printer (CUPS `lp`) or the viewer, with paper, scale, tiling, color, line weights and page range; Print Preview, Print Image, PDF bookmarks, and the row 1 Print button (Round 10)
- [x] Place a schedule on the plan (door, window, room, cabinet, electrical, framing, plant, fixture, furniture, general) with callout labels (Round 7)
- [x] Placed schedules are normal selectable objects, and the layout and the plan share one undo stack (Round 8)
- [x] Room Finish, Note and Stair schedules, grouping and totals, click a row to select the object, a schedule layout box (Round 9); the Materials List with a Master List and a construction set sheet (Round 10)
- [ ] Schedule tables in the DXF and in the construction set's plan sheets
- [ ] Plan notes, callouts and markers tied to elevation and section cameras
- [x] Line weights, colors and dashes in PDF output from the layer pens
- [x] Project Information: client, address, job number, date, revisions and custom fields fill the title block (Round 7)
- [x] The extra Project Information macros (`%client.phone%`, `%company%`, `%custom.<name>%` ...) in layout text (Round 9)

## Phase 5 — Library and ecosystem (partly done)

- [x] Library Browser with the built-in catalog; `plan-library` catalog system
- [x] `plan-calib` reads Chief `.calib` / `.calibz` catalogs (403 catalogs on
      Daniel's install), with decoded sizes, plan symbols and meshes
- [x] Project templates and plan defaults; imperial and metric dimension units
- [x] Safe saves, archives, autosave, crash recovery, unsaved-changes prompts, Save a Copy, Revert, Backup and a Finder double-click (Round 11)
- [x] Chief hotkeys loaded and customizable
- [x] Chief catalogs in the Library Browser (tree, search, thumbnails, place),
      with placed Chief meshes in 3D and a settings toggle (Round 5)
- [x] Automatic seeding from your Chief default plan and layout templates (Round 6)
- [ ] `capture_typing` for tools that take typed input
- [x] Replace From Library (Symbol Specification)
- [x] Create Image Library: your own pictures saved to a user library file, pictures, billboards and distributions (Round 8)
- [ ] Symbol import (OBJ, glTF, SKP via converter); Add to User Library for symbols, the Library Browser listing the user library
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
