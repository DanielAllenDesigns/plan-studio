# Roadmap

Chief Architect X18 is the reference. The feature inventory below is organized
the way Chief organizes its Build toolbar, and ordered by what a residential
designer needs first. Each phase is shippable on its own.

## Phase 0 — Foundation (in progress)

- [x] Cargo workspace: `plan-core` (model) + `plan-app` (egui editor)
- [x] Walls with thickness, height, exterior/interior kind
- [x] Doors and windows hosted in walls, with placement rules
- [x] Automatic room detection from wall centerlines (T-junctions, crossings)
- [x] Feet-and-inches formatting and parsing, JSON save/load
- [ ] 2D plan editor: grid, snapping, zoom/pan, wall chaining, select/delete
- [ ] Status bar with live X/Y coordinates like Chief's
- [ ] GitHub Actions CI (fmt, clippy, test on macOS/Linux/Windows)

## Phase 1 — A usable 2D plan tool

- Wall joins: mitered corners, clean T-intersections, wall layers (framing,
  drywall, siding) with correct face offsets
- Select, move, stretch, rotate; multi-select; copy/paste
- Undo/redo (command pattern over the model)
- Dimensions: manual, auto exterior, interior, temporary dimensions while
  drawing; editable dimension values that move the wall
- Text, rich text, arrows, CAD lines/arcs/circles/polylines
- Room labels: names, auto area, floor and ceiling finishes
- Layers and layer sets (Chief's "Active Layer Display Options" panel)
- Default settings dialogs per object type (Chief's "Defaults")
- Reference display of the floor below/above
- DXF/DWG export (`dxf` crate), PDF print to scale

## Phase 2 — 3D from the plan

- Generate wall solids with openings cut, floors, ceilings, platforms
- 3D viewport (wgpu via eframe's wgpu backend or `three-d`): orbit, dollhouse,
  perspective full overview, camera walkthrough
- Materials and material painter; glTF import for symbols/furniture
- Cabinets: base, wall, full-height, with doors/drawers and countertop
  generation
- Stairs and railings (straight, L, U, winders; auto stairwell)
- Multiple floors, foundations, attic, basement

## Phase 3 — Roofs and structure

- Auto roof from wall baselines: hip, gable, shed, Dutch gable, pitch per wall
- Manual roof planes, dormers, skylights, gutters, fascia
- Framing: joists, rafters, trusses, wall framing; framing schedule
- Terrain: elevation lines, pads, driveways, retaining walls

## Phase 4 — Documentation

- Elevations and cross sections generated from the 3D model (hidden line)
- Layout sheets, title blocks, viewports at scale, sheet index
- Schedules: door, window, cabinet, room finish, materials list
- Plan notes, callouts, markers, section/elevation cameras on plan
- Printing and PDF with line weights

## Phase 5 — Library and ecosystem

- Library browser with user, core and manufacturer catalogs
- Symbol import (OBJ, glTF, SKP via converter), 2D block/CAD details
- Plugin or scripting layer for custom tools
- Project templates and plan defaults, imperial and metric

## Technical choices

| Need | Crate |
|---|---|
| GUI | `eframe` / `egui` |
| 3D rendering | `wgpu` (eframe wgpu backend) or `three-d` |
| Math | `glam` for 3D, in-house 2D geometry in `plan-core` |
| Polygon ops | `geo` / `i_overlay` for booleans, `earcutr` for triangulation |
| Picking / collision | `parry3d` |
| Export | `dxf`, `printpdf`, `gltf` |
| B-rep CAD kernel (later) | `truck` |
