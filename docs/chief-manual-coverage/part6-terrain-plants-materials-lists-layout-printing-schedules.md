# Chief Architect X18 Reference Manual coverage, part 6: Terrain, Roads, Plants, Materials Lists, Layout, Printing, Ruby, Resources (pages 1307-1497)

Written 2026-10-08 by the manual audit (part 6). Source: Chief Architect's Reference Manual pages 1307 to 1497 (chapters 42 Terrain, 43 Roads, Driveways, and Sidewalks, 44 Plants and Sprinklers, 45 Materials Lists, 46 Layout, 47 Printing and Plotting, 48 Ruby in Chief Architect, 49 Resources and Support, 50 What's New in X18; the Index on pages 1479 to 1497 is not a feature list), read in full from `~/plan-studio-dev/chief-docs`. Every row below is written in my own words; only tool, dialog, panel and field names are Chief's. Nothing from the manual text is stored in the repo.

**Range note.** The brief for this part names a Schedules and Object Labels chapter. In this manual the schedule chapter sits at pages 708 to 740, outside the range, so it belongs to another part; here schedules appear only where terrain, roads, plants, the Materials List and layout tables touch them. The brief also mentions "Plan Check details" and "Appendix/Error messages": the manual has no Plan Check chapter, and the only error-message page (1465) is listed in chapter 49.

**How status was found.** For each feature I checked `docs/chief-feature-coverage.md`, the `docs/parity/*.md` rows and `docs/parity-status.md`, then the code (grep of `crates/` on branch `wip/round-14-partial`, read only, with Round 15 builders still editing). Where the earlier audit and the working tree disagree I trust the tree and say so. Statuses: **Works**, **Partial**, **Missing**, **Differs** (differs by design), **Out-of-scope** (Ruby, cloud, licensing, vendor services), **In progress (Round 15)** when a brief in `~/plan-studio-dev/briefs` covers it (none of the rows in this range qualified: the Round 15 Default Settings brief lists no road, watermark or Materials List Polyline pages). "NO SPEC (id)" means no parity row covered the feature before this audit; the id is the row appended to `docs/parity/*.md` and `docs/parity-status.md`.

## Counts

| Measure | Count |
|---|---|
| Features enumerated (table rows) | 460 |
| Works | 77 |
| Partial | 176 |
| Missing | 172 |
| Differs | 9 |
| Out-of-scope | 26 |
| In progress | 0 |
| Rows that had no parity spec (NO SPEC), each now a new parity row | 274 |
| New parity rows by file | cabinets-stairs-framing-terrain-library.md CB-505..626 (122); documentation-layout.md L-84..228 (145); dimensions-text-cad.md TXT-61..64 (4); preferences-hotkeys-toolbars.md APP-143..144 (2); preferences-hotkeys-toolbars.md DS-49..49 (1) |
| Dialogs and dialog groups compared in the panel table | 42 |

## 42. Terrain (pp. 1307-1347)

### 42.1 Toolbar configuration and Terrain menu

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1307 | Terrain menu with the parent tool groups (Elevation, Elevation Data, Modifier, Feature, Garden Bed, Grass Region, Water Feature, Stepping Stone, Terrain Wall and Curb, Road, Driveway, Sidewalk, Plant, Sprinkler) | Partial | CB-43..CB-50; menus.rs terrain_menu, toolbar.rs terrain_menu() lists 15 flyouts; Chief's Elevation Tools parent (perimeter, contour generate/remove) is split into menu rows; Round and Stream children are absent (see 42.6, 42.7) |
| 1307 | Terrain toolbar configuration that shows the terrain, road, plant and sprinkler tools on the toolbar | Missing | TB-7 (Terrain configuration: no spec yet); toolbar/config.rs has no Terrain configuration |

### 42.2 Terrain Perimeter, reference point, build and clear

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1308 | Create Terrain Perimeter (a closed polyline that bounds the 3D terrain and the contours; one per plan) | Partial | CB-43; tools/terrain.rs TerrainVariant::Perimeter is drawn by clicking corners, one record per plan (`Project.terrain`) |
| 1308 | Terrain Perimeter appears ready-made: 50 ft x 100 ft in an empty plan, grown to enclose any 3D model already drawn, Fill Window to see it | Partial | NO SPEC (CB-505): ours is clicked point by point; `Terrain::default` is a 100 ft x 80 ft lot but the tool never uses it; no auto-size to the building footprint (tools/terrain.rs, plan-terrain model.rs) |
| 1308 | Convert a closed CAD polyline into the Terrain Perimeter (Convert Polyline) | Missing | CAD-103 (Convert Polyline types: Partial); tools/cad/edit.rs has only Convert to Polyline, Spline and Lines, no terrain target |
| 1308 | Perimeter is resized, moved and reshaped like any closed polyline-based object | Works | CB-44; s36 `the_perimeter_is_edited_by_its_corner_handles_and_by_nudging` |
| 1308 | A new perimeter is flat at elevation 0 (sea level) until elevation data exists; one data object alone leaves the terrain flat | Works | CB-43; plan-terrain surface.rs (no data gives z = 0) |
| 1308 | Plot plan from CAD or Plan Footprint, then turned into a Terrain Perimeter | Partial | CAD-134 (plot plan entry by polar and bearing: Partial); L-38 Plan Footprint (Partial: traces room boundary, not outer wall faces) |
| 1309 | Terrain elevations are absolute (sea level), independent of the Floor 1 height, so real survey elevations can be used with floors still starting at 0 | Partial | NO SPEC (CB-506): plan-terrain keeps absolute inches and `site_view` ties the pad to the first floor; no explicit sea-level model or Absolute Elevation switch |
| 1309 | Automatic placement of the terrain below Floor 1: floor platform thickness plus 6 in (150 mm) or 8 in (200 mm) by foundation type plus sill plates, measured at the footprint corner whose elevation is closest to the average | Partial | NO SPEC (CB-507): ours is one number, "Terrain to first floor" (`subfloor_height_above_terrain`, default 6 in), applied at the building pad; foundation type, sill plates and the closest-to-average corner are not used (dialogs/terrain.rs General) |
| 1309 | Terrain Elevation Reference Point: the point (X, Y) where the surface-to-subfloor distance is measured, shown with its own handle only while the perimeter is selected, snaps to grid and objects, does not print | Missing | NO SPEC (CB-508): no reference point anywhere in plan-terrain, site_view or the dialogs (grep finds nothing) |
| 1309 | Place Terrain Elevation Reference Point and Remove Terrain Elevation Reference Point edit buttons on the selected perimeter | Missing | NO SPEC (CB-509): not in toolbar/edit tools (grep finds nothing) |
| 1309 | Build Terrain generates the surface and updates the Building Pad Elevation value | Works | CB-43; plan-terrain `build_terrain_with_progress`, site_view `build_surface_with_progress`; Terrain > Build Terrain |
| 1309 | Auto Rebuild Terrain (rebuild before each 3D view or Sun Shadow; turn off for big terrain) with a pointer badge when off and stale | Partial | CB-43 (switch in Terrain Specification, off keeps the surface marked stale); APP-74 (pointer icons: Missing); the Sun Shadow rebuild trigger is not wired |
| 1309 | Clear Terrain deletes only the generated contours and 3D surface; the perimeter, elevation data and features stay | Differs | NO SPEC (CB-510): main.rs TerrainCommand::Clear replaces the whole record with a new empty one (`TerrainRecord::new()`), so the perimeter, data and features are erased (one undo step "Clear Terrain"). This contradicts the manual |
| 1310 | Terrain surface triangles: the Triangle Count (or maximum Triangle Size) sets detail; Low 1000, Medium 2000, High 4000 suit about 20,000 sq ft; increase for a large perimeter | Partial | NO SPEC (CB-511): ours builds from a grid (Grid spacing, Subdivision); there is no triangle count or size field (dialogs/terrain.rs General, plan-terrain mesh.rs) |

### 42.3 Elevation data tools (pp. 1310-1314)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1310 | Elevation data objects are absolute and several at different heights make sloping terrain; the tools exist only when a perimeter exists | Works | CB-44; tools/terrain.rs refuses without a perimeter ("Draw a Terrain Perimeter first") |
| 1310 | Avoid placing elevation data of different elevations at the same place (including Terrain Breaks): the program warns or resolves a clash | Missing | NO SPEC (CB-512): no clash check in plan-terrain; the later object wins in the interpolation |
| 1311 | Elevation Point: click to place, specification opens, the last elevation value is offered again for the next point | Works | CB-44; tools/terrain.rs (type the elevation inline, empty takes the last) |
| 1311 | Elevation Line: drag a line, open it, enter the elevation (a new line starts at 0), connected lines make a polyline | Works | CB-44; tools/terrain.rs ElevationLine, typed elevation |
| 1312 | Elevation Spline: click and drag segments, open it to give the elevation, edited like a spline | Works | CB-44; `ElevationLine::spline` keeps control points and tension |
| 1313 | Elevation Region: click once for an 8 ft (0.6 m) square, or drag a rectangle; open it to set the elevation; also made by a closed loop of Elevation Lines | Partial | NO SPEC (CB-513): CB-44; drawn as a clicked polygon with a typed elevation; no click-once 8 ft square and no closed-loop conversion (tools/terrain.rs) |
| 1314 | Terrain Break: a division line that stops elevation data on one side from affecting the other (sharp contours when it spans the perimeter, blended near free ends) | Partial | CB-44; plan-terrain `TerrainBreak` is a line held at one elevation whose vertices stay fixed while smoothing; it keeps a crease but does not isolate the two sides, and the edge blending at free ends is not modelled |
| 1315 | Retaining Wall tool draws a Terrain Break plus a wall that rests against it, top matching the high side and bottom the low side | Partial | NO SPEC (CB-514): CB-46 (terrain walls cut the TIN with a `retain` drop and optional stepping); there is no Retaining Wall tool and no wall height taken from the two sides of a break (see 42.6) |

### 42.4 Terrain modifier tools (pp. 1315-1316)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1315 | Raised Region and Lowered Region: a plateau or depression whose flat top or bottom follows the terrain surface | Works | CB-45; `ModifierKind::{RaisedRegion, LoweredRegion}`; Raised/Lowered Region Specification |
| 1315 | Hill and Valley: raised or lowered areas that come to a point instead of flattening | Works | CB-45; `ModifierKind::{Hill, Valley}` with Height |
| 1316 | Flat Region: an area levelled like a plateau, level (not following the contours) | Partial | CB-45; `ModifierKind::FlatRegion` levels to the mean elevation of the polygon; no elevation field (matches Chief, which has none) |
| 1316 | Modifiers click once for a 10 ft (4 m) square or drag a rectangle; edited like splines; only affect terrain inside their outline; display in 3D only inside the perimeter; can be blocked with images and stored in the Library | Partial | NO SPEC (CB-515): CB-45; polygon click drawing, no click-once square, no spline editing for modifiers (clicked polygons), no library storage of terrain modifiers |

### 42.5 Terrain features (pp. 1316-1318)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1316 | Terrain Features follow the terrain, have a height, thickness and material, can sit above or sink into the ground | Works | CB-46; `Feature` (pad with `height`, material) |
| 1316 | Terrain Features can be drawn in plan and in camera views, only inside a perimeter, and be converted from closed CAD polylines or splines (Convert Polyline) | Partial | NO SPEC (CB-516): CB-46; plan only (no drawing in camera views); Convert Polyline to a terrain feature: Missing (CAD-103) |
| 1317 | Terrain Features and the Library: custom features can be added to the Library and blocked with architectural objects such as exterior fixtures | Partial | NO SPEC (CB-517): CB-46; terrain objects cannot be sent to the Library (Add to Library covers symbols, cabinets, CAD) |
| 1317 | Feature types: Terrain Feature (concrete), Terrain Hole, Garden Bed, Grass Region, Water Feature (Pond, Stream), Stepping Stone | Works | CB-46, CB-49; `LandscapeKind::{GardenBed, GrassRegion, WaterFeature, SteppingStones}`; Pond works as the Water Feature; Stream is a separate item (below) |
| 1317 | Polyline Feature children: Rectangular Feature, Terrain Hole, Polyline Garden Bed, Polyline Grass Region, Polyline Stepping Stone (a rectangle edited to any shape, 10 ft per side on one click) | Partial | CB-46; toolbar.rs terrain_feature(), garden_bed(), grass_region(), stepping_stone() (drawn as clicked polygons; no 10 ft click-once) |
| 1317 | Stepping Stone tools build a walkway of individual stepping stones, each a Terrain Feature (Polyline Stepping Stone, Round Stepping Stone) with path-typical material and height | Partial | CB-49; ours is one Stepping Stones object: stones of a chosen size and spacing laid along a clicked polyline or spline (landscape.rs `LandscapeKind::SteppingStones`), not separate editable features |
| 1317 | Round Feature children: Round Feature, Round Garden Bed, Round Grass Region, Round Pond, Round Stepping Stone (spline based, about 150 sq ft on one click) | Partial | NO SPEC (CB-518): CB-46 (Round Feature only); no Round Garden Bed, Round Grass Region, Round Pond or Round Stepping Stone tool (toolbar.rs) |
| 1318 | Kidney Shaped Feature children: Kidney Shaped Feature, Garden Bed, Grass Region, Pond (about 67 sq ft on one click) | Partial | NO SPEC (CB-519): CB-46; Kidney Feature, Kidney Garden Bed and Kidney Grass Region exist; no Kidney Pond/Water Feature (toolbar.rs water_feature() has Polyline and Spline only) |
| 1318 | Terrain Holes: the program cuts a hole around any structure on the Terrain Perimeter's floor and follows wall edits; Make Terrain Hole(s) Around Building(s) makes manual hole polylines | Works | CB-47; site_view `auto_building_hole`; Terrain > Make Terrain Hole Around Building |
| 1318 | Make Terrain Hole(s) Around Building(s) as an edit button on the selected perimeter, one hole per building on that floor | Missing | NO SPEC (CB-520): only a Terrain menu command that cuts around the building (main.rs HoleAroundBuilding); no edit-toolbar button or per-building holes |

### 42.6 Terrain walls, curbs, retaining walls and streams (pp. 1318-1319)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1318 | Straight Terrain Wall and Spline Terrain Wall that sit on top of and follow the terrain, 5 ft (1500 mm) concrete by default | Partial | CB-46; `TerrainWall::new` is 36 in high, 12 in below, 8 in thick, concrete; tools Straight and Curved Terrain Wall. Default height differs (3 ft against 5 ft); DECISIONS 108 steps new walls where Chief's follows the ground |
| 1319 | Straight Retaining Wall and Curved Retaining Wall: a Terrain Break plus a wall; height set by the terrain on each side (top matches the high side, bottom the low side; looks like a concrete strip on flat ground) | Missing | NO SPEC (CB-521): CB-46 has `retain` and `stepped` on a terrain wall but no tool that also creates the break or sizes the wall from the two sides; DECISIONS 108 makes new terrain walls stepped by default, which the manual does not describe |
| 1319 | Straight Terrain Curb and Spline Terrain Curb with a height on the General panel (landscaping curbs around beds and paths) | Works | CB-46; `WallKind::Curb` (6 in high); Straight/Curved Terrain Curb tools |
| 1319 | Streams: a spline Terrain Curb with a water material that follows a downhill course; tool under Water Feature | Missing | NO SPEC (CB-522): no Stream tool (toolbar.rs water_feature(); `WaterFeature` is a region) |
| 1319 | Fencing with gates from the Doors library placed in the terrain (Fencing Tools, p. 378) | Partial | Fence wall type exists (Default Settings > Walls > Fence, `walls.fence`); gate placement from the Doors library: see the part 2 audit of Fencing Tools (pp. 378-384); not a terrain object |

### 42.7 Library, display and editing (pp. 1319-1322)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1319 | Terrain objects in the Library Browser (plants, exterior fixtures, accessories, roadway objects) placed into the terrain | Partial | NO SPEC (CB-523): CB-50 and the Library rows (CB-53..CB-60); the Plants catalog (plan-library catalog_plants.rs) is the only terrain content; no roadway objects, exterior fixtures category or accessories |
| 1320 | Terrain object display controlled in Layer Display Options; objects sit in Drawing Groups and can change their drawing order | Partial | NO SPEC (CB-524): terrain, landscape and plant objects carry a layer name (`Terrain`, `Terrain, Features`, `Terrain, Walls`, `Terrain, Breaks`, `Landscaping, ...`, `Plants`, `Sprinklers`; site_view/landscape.rs ensure_landscape_layers) so Layer Display Options can hide them; Drawing Groups (dialogs/drawing_groups.rs) are for CAD and architectural objects, not terrain; Chief's `Terrain Labels`, `Terrain, Primary Contours` and `Terrain, Secondary Contours` layers are absent |
| 1320 | Line and fill styles of terrain features, roads and other terrain objects customised in plan view | Partial | CB-46; `ObjectStyle` (layer, line color/weight/dashed, fill none/solid/hatch/ripple) in the Line Style and Fill Style tabs; no line style library or fill style library pickers (CAD-73) |
| 1320 | Plant images drawn in plan by 2D CAD symbols chosen in the Plant Image Specification | Partial | CB-50; plants are plant runs drawn with the catalog's plan symbol; no per-plant 2D symbol picker |
| 1320 | Sun Shadows computed from the terrain contour and shown as an automatic CAD polyline in plan | Partial | NO SPEC (CB-525): the north angle feeds the sun azimuth (site_view::plan_sun_azimuth, site_plan.rs); a terrain-aware plan shadow polyline is not built; verify in Chief |
| 1320 | Building the terrain: gathers all data, interpolates smooth contours, a Building Terrain progress dialog, automatic before a 3D view or Sun Shadow | Partial | CB-43 (stage callbacks and a final summary, not a live bar) |
| 1320 | Contour lines drawn in plan as primary and secondary on two layers with their own label Text Styles ("Terrain, Primary Contours", "Terrain, Secondary Contours") | Partial | NO SPEC (CB-526): CB-51; primary and secondary line styles exist; both families draw on the single `Terrain` layer and use one label style; no separate layers (plan-terrain contour.rs, site_view) |
| 1321 | Terrain and Road labels: terrain objects (perimeter, elevation data, features, modifiers, paths, roads) show custom or automatic labels on the "Terrain Labels" layer in plan and section/elevation views, with a Label panel | Missing | NO SPEC (CB-527): only contour elevation labels and the Plant labels exist; terrain object dialogs have no Label panel (dialogs/terrain/object.rs) |
| 1321 | Roads and features visible in 3D only inside the perimeter; Grass Region 3D grass absent in techniques without textures | Partial | CB-48; the 3D mesh clips to the perimeter (site_view terrain_feature_meshes); 3D grass blades do not exist (grep blade in plan-terrain finds nothing) |
| 1321 | Schedules for terrain and roads: Terrain Perimeter, Driveways, Medians, Roads and Road Markings in categories of their own; Sidewalks, Terrain Walls and Curbs in Terrain Paths; features, modifiers, garden beds, water features and stepping stones in Terrain Features; any object can be reassigned on its Schedule panel | Missing | NO SPEC (CB-528): ScheduleKind has Plant but no terrain, road or path kind (plan-core schedules.rs); the cut and fill report is separate (CB-76) |
| 1321 | Selecting terrain objects in plan view (and the perimeter and features in 3D views) | Partial | CB-44; `ObjectRef::TerrainObject` (editor/selection.rs); 3D pick of terrain objects is not built (C-43 covers symbols, cabinets) |
| 1321 | Copy terrain objects from one plan to another (they show in 3D only when a perimeter exists) | Missing | S-81 (clipboard excludes terrain elements) |
| 1322 | Edit handles for terrain objects: perimeter like a polyline, elevation points move, lines and breaks like line/spline objects, modifiers like splines, features like polylines or splines | Works | CB-44, CB-45, CB-46; handles.rs terrain handles; s36 |
| 1322 | Terrain object labels have their own Move and Rotate edit handles in plan view | Missing | NO SPEC (CB-529): no terrain labels (see above) |
| 1322 | Edit tools on a selected terrain object (Edit Toolbar buttons) and resizing or moving terrain objects with dimensions | Partial | the Edit toolbar rows of select-and-edit.md; terrain objects have Delete and nudge; no terrain-specific edit buttons; dimensions do not locate terrain objects (DIM-53 Setup Temporary option "terrain objects inside a selected object" is Partial) |

### 42.8 Terrain Specification dialog (pp. 1322-1327) - General panel

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1323 | Open the Terrain Specification from Terrain > Terrain Specification, Open Object on the perimeter, or double-click | Works | CB-51; menus.rs Terrain Specification, tools/terrain.rs double-click opens it |
| 1323 | Auto Rebuild Terrain check box (rebuild before a 3D view or Sun Shadow) | Works | CB-43; dialogs/terrain.rs "Rebuild the terrain after every edit" |
| 1323 | Absolute Elevation, Automatic: Chief sets the distance between Floor 1 and the terrain; uncheck to set it by hand | Partial | NO SPEC (CB-530): ours "Terrain to first floor" is always manual (default 6 in) and "Level the terrain under the building automatically" is a different switch; no Automatic check box |
| 1323 | Retain Surface Elevation At: Reference Point or Contour 0 | Missing | NO SPEC (CB-531): no such choice (dialogs/terrain.rs General) |
| 1324 | Surface at Reference Point / Surface at Contour 0 (vertical distance from the default Floor 1 subfloor to the surface, normally negative) | Partial | NO SPEC (CB-532): ours: Terrain to first floor (positive distance, building pad) |
| 1324 | Terrain Elevation Reference Point X Position and Y Position fields | Missing | NO SPEC (CB-533): not in dialog |
| 1324 | Flatten Pad: flatten the area beneath and around the building (leave off for sloping building perimeter) | Partial | CB-47; "Level the terrain under the building automatically" (`flatten_pad`), Building Pad tab margin and slope |
| 1324 | Skirt: thickness of the 3D skirt around the terrain edge, Flat base (uniform elevation below the lowest point) or Follow Terrain (constant distance below the surface) | Missing | NO SPEC (CB-534): no skirt model (grep skirt in plan-terrain finds only landscape_mesh.rs unrelated); 3D terrain has no side skirt |
| 1324 | Terrain Surface Smoothing: Low, Medium, High, or Linear (flat triangles between points, for dense data) | Partial | NO SPEC (CB-535): ours "Smoothing passes" 0 to a maximum (Laplacian on the grid) in dialogs/terrain.rs; no Linear mode |
| 1324 | Triangle Count: Low 1000, Medium 2000, High 4000 or Custom, or a maximum Triangle Size | Missing | NO SPEC (CB-536): grid spacing and subdivision instead (dialogs/terrain.rs General) |
| 1324 | Clipping: Hide Terrain Intersected by Building (no contours inside the house; use a Terrain Hole when the foundation differs from the first floor footprint) | Missing | NO SPEC (CB-537): building hole exists (CB-47) but there is no check box; the automatic hole is always on |

### 42.9 Terrain Specification - other panels

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1325 | Contours panel, Frequency: Interval between contours | Partial | CB-51; dialogs/terrain.rs Contours "Contour interval" (also repeated on General) |
| 1325 | Contours panel, Offset (shifts which elevation gets the zero contour) | Missing | NO SPEC (CB-538): no offset field; contours.rs starts at multiples of the interval |
| 1325 | Contours panel, Primary contour every N contours (1 = all primary) | Works | CB-43, CB-51; "Major contour every" |
| 1325 | Contours panel, Smoothing on/off and Passes for the 2D contour lines | Missing | NO SPEC (CB-539): no 2D contour smoothing control (the Smoothing passes smooth the surface) |
| 1325 | Contours panel, Label Primary Contours and Label Secondary Contours | Partial | CB-51; "Labeled with their elevation" per family (the primary box is always on and disabled) |
| 1326 | Highlight Negative Elevations (labels below 0 in red) | Missing | NO SPEC (CB-540): grep finds nothing |
| 1326 | Label Units: inches or decimal feet (mm or meters) | Missing | NO SPEC (CB-541): contour labels use the plan's length format |
| 1326 | Contour labels use the Text Styles of the two contour layers | Partial | Text Styles exist (TXT-1..TXT-19); contour labels use a fixed style (plan-terrain contour labels) |
| 1326 | Polyline panel for the perimeter: perimeter length, Area and Volume, Number of Lines (holes subtracted) | Missing | NO SPEC (CB-542): no Polyline panel; only the Building Pad tab reports areas |
| 1326 | Spline panel (New Segment Angle) when the perimeter is a spline; Selected Line and Selected Arc panels | Missing | NO SPEC (CB-543): the perimeter is straight segments only; no spline or arc edges (CAD-22/24 give polyline arcs for CAD only) |
| 1326 | Line Style and Fill Style panels for the perimeter | Missing | NO SPEC (CB-544): Layer tab only; the perimeter has fixed look (dialogs/terrain.rs) |
| 1327 | Materials panel for terrain surface and terrain skirt materials (not counted in the Materials List) | Partial | CB-51; Materials tab has Ground and Bare ground (DECISIONS 106: bare ground is stored but only Ground reaches 3D); no skirt material |
| 1327 | Label panel (custom label, uses the Terrain Labels layer) | Missing | NO SPEC (CB-545): see 42.7 |
| 1327 | Object Information panel and Schedule panel on the perimeter | Missing | NO SPEC (CB-546): no panels (dialogs/terrain.rs tabs General, Contours, Building Pad, Materials, Layer) |

### 42.10 Elevation Point, Line/Region, Flat, Hill/Valley, Raised/Lowered specifications (pp. 1327-1333)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1327 | Elevation Point Specification: Elevation, X and Y position, Number Style button | Works | CB-44; dialogs/terrain/object.rs (Elevation, Position X, Position Y) |
| 1328 | Elevation Point Display: note text beside the point, Insert Macro (for example the elevation macro), Marker Radius | Missing | NO SPEC (CB-547): not in the dialog (fields only Elevation and position) |
| 1328 | Elevation Point Line Style and Text Style panels | Missing | NO SPEC (CB-548): the dialog is General only for points, lines, regions and modifiers (object.rs GENERAL_ONLY); no Line Style or Text Style panel |
| 1328 | Elevation Line/Spline/Region Specification, Elevation panel: Elevation of the line, spline, polyline or region | Works | CB-44; object.rs "Elevation" with spline tension |
| 1329 | Interior is Flat (closed Elevation Regions keep a flat interior at the elevation; unchecked, only the perimeter is held) | Missing | NO SPEC (CB-549): grep finds nothing; a region is always flat inside (`ElevationRegion` has only polygon and z) |
| 1329 | Interpolate Tangent to Edge (flatten the surface as it approaches the edges of a region with Interior is Flat off) | Missing | NO SPEC (CB-550): not modelled |
| 1329 | Polyline panel (length or perimeter, area), Spline panel, Selected Line/Arc panel for elevation objects | Missing | NO SPEC (CB-551): the dialogs show General only |
| 1329 | Elevation Line Line Style panel (appearance and bumping) and Label panel (default label is the elevation) | Missing | NO SPEC (CB-552): General only (object.rs GENERAL_ONLY); elevation labels exist only as contour labels |
| 1330 | Flat Region Specification: Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Label, Object Information, Schedule panels (volume 0, no material) | Partial | NO SPEC (CB-553): CB-45; General only (object.rs GENERAL_ONLY); every other panel absent |
| 1331 | Hill / Valley Specification: Height relative to the surface from the elevation data | Works | CB-45; object.rs Height |
| 1331 | Hill/Valley and Raised/Lowered panels beyond Height: Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Label, Object Information, Schedule | Partial | NO SPEC (CB-554): same gap as Flat Region |
| 1332 | Raised / Lowered Region Specification (same panels as Hill/Valley) | Works | CB-45 (height or depth) |

### 42.11 Terrain Feature, Grass Region, Terrain Break and Terrain Path specifications (pp. 1333-1341)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1333 | Terrain Feature Specification, General: Terrain to Top (negative sinks into terrain), Thickness | Partial | NO SPEC (CB-555): CB-46; "Height above ground" (one height, no separate thickness; the pad is a slab) |
| 1333 | Make Hole check box turns a feature into a Terrain Hole | Works | CB-46; the Terrain Hole is its own variant with a simple dialog |
| 1333 | Clip Overlapping Terrain Features (hide the part of a feature cut by a lower feature; for planters and pools) | Missing | NO SPEC (CB-556): grep finds nothing |
| 1333 | Garden Bed Distributed Plant panel (choose a plant and spacing to scatter inside the bed) | Missing | NO SPEC (CB-557): Garden Bed has material, mulch depth, edging only; Plant runs are separate objects laid along a path (object.rs) |
| 1333 | Polyline, Spline, Selected Line/Arc panels on features | Missing | NO SPEC (CB-558): General, Line Style, Fill Style, Layer only |
| 1334 | Terrain Feature Line Style and Fill Style panels | Partial | CB-46; object.rs Line Style, Fill Style tabs |
| 1334 | Terrain Feature Materials panel (3D material; counts in schedules and Materials List) | Missing | NO SPEC (CB-559): material is a name in General (Concrete, Stone, Brick, Grass); no Define Material picker; Landscaping lines in the Materials List cover plants and cut/fill only |
| 1334 | Terrain Feature Label panel (label visible in plan and in section/elevation views when Terrain to Top is positive), Components panel, Object Information panel, Schedule panel | Missing | NO SPEC (CB-560): none of these tabs exist for terrain objects |
| 1335 | Grass Region Specification, Blades: Density, Minimum and Maximum Height, Minimum and Maximum Width, Minimum and Maximum Curve (0 = straight) | Missing | NO SPEC (CB-561): Grass Region has a material and a lift only (landscape.rs); no blade parameters |
| 1336 | Grass Region Appearance: blade Colors, Noise Frequency, Roughness, Mow (Cut Height, Mow Line Intensity, Width, Angle) and a live preview | Missing | NO SPEC (CB-562): grep for mow and blade in plan-terrain finds nothing; the 3D grass is a flat tinted surface |
| 1336 | Grass Region has no thickness (volume 0); its Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Materials, Label (no section view label), Components, Object Information, Schedule panels | Missing | NO SPEC (CB-563): only General, Line Style, Fill Style, Layer |
| 1337 | Terrain Break Specification, General: Transition Distance (how far from the break its effect reaches) | Partial | NO SPEC (CB-564): CB-44; the break dialog has Elevation (our break holds one elevation, Chief's only divides the data); no Transition Distance field |
| 1338 | Terrain Break panels: Polyline, Selected Line/Arc, Spline, Line Style, Fill Style (when segments form a closed area) | Missing | NO SPEC (CB-565): break dialog shows General, Line Style, Layer |
| 1339 | Terrain Path Specification (driveways, sidewalks, streams, terrain walls, terrain curbs), General: Width, Terrain to Top, Thickness | Partial | CB-46, CB-48; wall dialog has Top above terrain, Bottom below terrain, Thickness; road dialog has Width, Crown, Curb height; no Terrain to Top on roads, no Thickness on roads |
| 1339 | Terrain Path Flare: Start and End check boxes with Radius for intersections with other paths (for example a 24 in flare) | Missing | NO SPEC (CB-566): grep flare in plan-terrain finds nothing |
| 1339 | Terrain Path preview pane | Partial | NO SPEC (CB-567): SpecDialog has previews for other objects; terrain object dialog has none (pv_text helpers only for plant chooser) |
| 1340 | Terrain Path Polyline (perimeter, area, volume), Spline, Selected Line/Arc, Materials, Label, Components, Object Information and Schedule panels | Missing | NO SPEC (CB-568): walls and curbs show General, Line Style, Fill Style, Layer; roads and driveways General and Layer (object.rs REGIONS, ROAD_TABS) |

### 42.12 Importing elevation data (pp. 1341-1347)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1341 | Elevation data import formats: DXF/DWG, GPS Exchange (.gpx) and text (.txt, .csv, .prn, .xyz, .auf, .nez) | Partial | CB-75; plan-terrain import.rs reads DXF (ASCII), GPX and XYZ text separated by space, comma, semicolon or tab; .prn, .auf, .nez, DWG not specially read |
| 1342 | File > Import > Terrain Data opens the Import Terrain Assistant (Select File, Filter Data, Scale Data steps) | Missing | NO SPEC (CB-569): the import is a form on the Terrain Specification (Terrain > Site Objects, DECISIONS 107), not an assistant and not under File > Import (dialogs/terrain.rs import_page) |
| 1342 | Select the data organization: XYZ, #XYZ, #XYZ Description, YXZ, #YXZ, #YXZ Description, comma or space delimited | Missing | NO SPEC (CB-570): import_points guesses x y z; there is no choice of column order, point number or description column |
| 1343 | Filter Data: show point count, restrict to X, Y and Z ranges, or reduce to a chosen number of evenly spread points (warns above 1000 to 2000 points) | Missing | NO SPEC (CB-571): no filtering or point thinning (plan-terrain import.rs) |
| 1344 | Scale Data: units per axis, map a file point to the plan origin, scale factor for relief, rotate north counterclockwise | Partial | NO SPEC (CB-572): ours: one unit choice, "Center the points on the terrain perimeter" and "lowest point at 0" options; no map-point, per-axis units, scale or rotation (dialogs/terrain.rs) |
| 1344 | File > Import > GPS Data opens the Import GPS Data Assistant (.gpx, GPX 1.1 only; Way Points carry elevation, Track Points do not, Route Points are ignored) | Missing | NO SPEC (CB-573): import.rs also reads route and track points as elevation points, which differs from the manual's rules; no assistant |
| 1345 | GPS Import As per item: Elevation Data (way points), Marker, Polyline, or Terrain Perimeter (closed, from way or track points) | Missing | NO SPEC (CB-574): not offered; every point becomes an elevation point |
| 1346 | GPS Transform Coordinates: Lower Elevation Data by, Rotate North Counterclockwise, map a latitude/longitude point to the origin | Missing | NO SPEC (CB-575): only a flat projection around the first point; no lowering, rotation or mapped origin (import.rs) |
| 1346 | Import Drawing Assistant converts any layer of a DWG/DXF into a Terrain Perimeter or Elevation Data; lines with equal Z become Elevation Lines, others one Elevation Point per vertex; points become Elevation Points; elevation data inside blocks is ignored | Partial | NO SPEC (CB-576): L-43 DXF import with per-layer map (keep, skip, plan layer); no terrain target for a layer. CB-75 imports DXF points and polylines as elevation points directly (3DFACE, POINT, POLYLINE vertices) |
| 1347 | A Terrain Perimeter is created around the data extents when none exists (or no layer was converted to it) on import | Partial | NO SPEC (CB-577): import adds points to the existing record; whether it creates a perimeter when none exists is untested (`add_elevation_points` only appends) |
| 1347 | Convert CAD lines, splines and polylines to terrain data with the Convert Polyline edit tool (only the perimeter is offered until one exists) | Missing | CAD-103; no terrain, road or landscape target in the CAD edit tools (tools/cad/edit.rs) |

## 43. Roads, Driveways, and Sidewalks (pp. 1348-1357)

### 43.1 Defaults (p. 1348)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1348 | Default Settings group "Roads, Sidewalks and Driveways" with Road Defaults, Driveway Defaults, Road Marking Defaults, Sidewalk Defaults dialogs (same fields as the specification dialogs); double-click the tool button to open them | Missing | NO SPEC (DS-49): Default Settings tree has Terrain Defaults only (dialogs/defaults.rs TREE); road width, curb and material come from fixed constants (`RoadStrip::default` 240 in wide, `curb_height` 6 in) |

### 43.2 Tools (pp. 1349-1351)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1349 | Road objects can be drawn in plan, camera views and overviews, only when a Terrain Perimeter exists; they inherit size and properties from their defaults dialogs | Partial | NO SPEC (CB-578): CB-48; plan only, requires a perimeter ("Draw a Terrain Perimeter first"); no defaults dialogs |
| 1349 | Straight and Spline road objects are flat across their width (easy on slopes), join end to end into intersections and networks, have one constant width | Partial | NO SPEC (CB-579): CB-48; `RoadStrip` centerline with width and crown; roads follow the built surface; no junction logic where two centerlines meet (no flare, no merged intersection, grep flare finds nothing) |
| 1349 | Polyline roads, driveways and sidewalks are terrain features drawn as rectangles and edited into any shape, following the ground | Partial | NO SPEC (CB-580): toolbar.rs "Polyline Road/Driveway/Sidewalk" draw a clicked centerline strip, not a closed shape; there is no closed-outline road object |
| 1349 | Convert Polyline turns a CAD polyline into most road objects (perimeter or center line forms) | Missing | CAD-103 (Road forms missing) |
| 1349 | Roads have curbs; Sidewalks and Driveways cut the curb and gutter wherever they meet a road or road polyline | Partial | NO SPEC (CB-581): CB-48; roads have a curb flag and height; sidewalks and driveways do not cut the curb (`curb` in model.rs RoadStrip only) |
| 1349 | Straight Road: click and drag a line, connect several sections; edited along the centerline like a line-based object | Works | CB-48; tools/terrain.rs TerrainVariant::Road |
| 1349 | Spline Road: curved road from spline segments, edited like a spline | Works | CB-48; TerrainVariant::SplineRoad |
| 1349 | Polyline Road: rectangular polyline road editable to any shape | Missing | NO SPEC (CB-582): see above (our Polyline Road is a centerline) |
| 1349 | Median: a rectangular polyline inside a road that cuts a hole revealing the terrain material, with its own curb when the road has one | Missing | NO SPEC (CB-583): no Median object (grep median in plan-terrain finds nothing) |
| 1350 | Cul-de-sac: a round road end placed by clicking on the end of a road (cannot attach to a road polyline) | Missing | NO SPEC (CB-584): not built |
| 1350 | Road Marking: draw a rectangular region on a road with a different material | Partial | NO SPEC (CB-585): CB-48 (DECISIONS 109): ours is a stripe along a path (`RoadKind::Marking`, 4 in, solid or dashed); the region-shaped marking is absent |
| 1350 | Road Stripe: a stripe of marking material on the road surface, several stripes connect | Partial | CB-48; Road Marking / Stripe line follows the ground or the crown (s36 `road_markings_lie_on_the_ground_and_on_the_crown_of_a_road`); dashes and color options are beyond Chief's panel |
| 1350 | Straight Driveway (connected sections), Spline Driveway | Works | CB-48; TerrainVariant::Driveway, SplineDriveway |
| 1350 | Driveway Polyline: rectangular polyline driveway editable to any shape (a terrain feature) | Partial | NO SPEC (CB-586): see Polyline Road |
| 1350 | Straight Sidewalk and Spline Sidewalk drawn along the center line | Works | CB-48; TerrainVariant::Sidewalk, SplineSidewalk |
| 1351 | Sidewalk Polyline: rectangular polyline sidewalk | Partial | NO SPEC (CB-587): see Polyline Road |
| 1351 | Auto Generate Sidewalk edit button on a Road or Median: dialog with Left Side and Right Side of Road, All Connected Roads, Offset From Road | Missing | NO SPEC (CB-588): grep finds nothing |

### 43.3 Display and editing (pp. 1351-1352)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1351 | Roads and sidewalks display in plan and 3D by the settings of their layers; the terrain rebuilds automatically when road objects change; visible in 3D only inside the perimeter | Partial | CB-48 (layer name on RoadStrip; auto rebuild switch) |
| 1351 | Schedule categories for roads: Driveways, Medians, Roads, Road Markings listed by name and Sidewalks under Terrain Paths | Missing | NO SPEC (CB-589): see 42.7 (no terrain or road schedule kind) |
| 1352 | Road objects are selectable in 2D and 3D and edited through edit handles, the Edit Toolbar and their dialogs | Partial | CB-48; selectable in plan through `ObjectRef::TerrainObject`; no 3D pick |
| 1352 | Edit handle behaviour: straight roads, driveways, stripes, terrain walls and curbs edit along the centerline; spline roads, sidewalks and walls like CAD splines | Works | CB-48, CB-44; handles.rs |
| 1352 | Polyline roads, culs-de-sac, medians, road markings, polyline driveways and sidewalks edit along their perimeter like standard polylines; road object labels have Move and Rotate handles | Missing | NO SPEC (CB-590): those objects do not exist (see above) |
| 1352 | Convert to Polyline Object edit button turns a straight or spline road, sidewalk or driveway into its polyline form | Missing | NO SPEC (CB-591): grep finds nothing |
| 1352 | Add terrain objects (elevation points and lines, features, roads, sidewalks, markings, even the perimeter) to the Library as one unit; placing them gives independent objects | Missing | NO SPEC (CB-592): no Library path for terrain objects (see 42.5) |

### 43.4 Road Specification dialog (pp. 1353-1355)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1353 | Road Specification General: Width (not for culs-de-sac), Terrain to Top (negative sinks), Thickness | Partial | NO SPEC (CB-593): CB-48; object.rs Width, Crown, Curb height; no Terrain to Top or Thickness (the strip lies on the surface) |
| 1353 | Road Flare: Start and End check boxes with Radius for intersecting roads (24 in example); not for medians or culs-de-sac | Missing | NO SPEC (CB-594): grep flare finds nothing in plan-terrain |
| 1354 | Road Curb panel: Has Curb, Select Curb Profile, Default Curb Profile, Height and Width, Cut Curb for Driveways and Sidewalks, profile preview | Partial | NO SPEC (CB-595): CB-48; Has Curb and Curb height only; no curb width, profile, cut-for-driveways or preview (object.rs) |
| 1354 | Road Specification Preview pane | Missing | NO SPEC (CB-596): none |
| 1354 | Road Polyline panel (perimeter, area, volume, number of lines), Selected Line/Arc panel, Spline panel | Missing | NO SPEC (CB-597): ROAD_TABS has General and Layer only |
| 1355 | Road Line Style and Fill Style panels | Partial | NO SPEC (CB-598): Line Style/Fill Style tabs exist for walls and regions but not for roads (object.rs ROAD_TABS = General, Layer) |
| 1355 | Road Materials panel (curb material not counted in the Materials List), Label panel (plan and section/elevation), Components panel, Object Information panel, Schedule panel | Missing | NO SPEC (CB-599): the road has a material name in General only (Asphalt, Concrete, Gravel, Brick, Stone) |

### 43.5 Road Marking Specification (pp. 1356-1357)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1356 | Road Marking Specification General (Road Stripes only): Width | Works | CB-48; object.rs "Line width" for a marking |
| 1356 | Road Marking Polyline, Selected Line/Arc, Line Style, Fill Style, Materials, Label (plan only), Object Information, Schedule panels | Missing | NO SPEC (CB-600): marking dialog shows General and Layer; color and dashed options are ours |

## 44. Plants and Sprinklers (pp. 1358-1367)

### 44.1 Plant tools and library (pp. 1358-1359)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1358 | Place plants from the Core Catalogs > Plants category (plant images and 3D plant symbols) and from downloadable Bonus Catalogs by clicking in any view; plants are image objects avoiding high 3D surface counts | Partial | CB-50, CB-73; ours Plant tool lays a run of plants along a clicked polyline or spline, drawn from the built-in Plants catalog (plan-library catalog_plants.rs, trees, shrubs, landscape); not placed one at a time from the Library Browser, not images; Chief's Core and Bonus plant catalogs are only read as symbols (CB-78 Missing for downloads) |
| 1358 | Add single plants and blocked groups of plants to the User Catalog | Missing | NO SPEC (CB-601): Library add path exists for symbols; plants are terrain runs and cannot be added (CB-50) |
| 1358 | Terrain > Plant menu: Plant Chooser, Create Plant Image, Grow All Plants, Show Hardiness Zones | Partial | NO SPEC (CB-602): our Plant flyout has Plant, Polyline Plant, Spline Plant only; Plant Chooser lives inside the plant specification; Create Plant Image, Grow All Plants, Hardiness Zones are absent |
| 1358 | Create Plant Image tool and New > Plant Image in the Library Browser menu: make a plant image object that can go to the Library | Missing | NO SPEC (CB-603): no plant image object (grep PlantImage finds nothing) |
| 1358 | Grow All Plants: Grow Plants dialog with a 0 to 20 year slider that scales plants that have a mature height and mature age | Missing | NO SPEC (CB-604): grep finds nothing; the catalog has mature sizes but no age data (CB-74) |
| 1359 | Garden Beds distribute copies of a plant image inside the bed (see Distributed Plant panel) | Missing | NO SPEC (CB-605): CB-49 garden beds have no plant distribution |
| 1359 | Plant Schedule tool: customisable plant schedules and plant labels showing schedule numbers | Works | L-23 Plant kind in ScheduleKind; schedule_kinds.rs terrain_plants; Plant Schedule defaults page |

### 44.2 Plant Chooser and Hardiness Zones (pp. 1359-1362)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1359 | Plant Chooser dialog, from Terrain > Plant or the button under the Library Browser; searches the library by any combination of options | Partial | CB-73; dialogs/terrain/object.rs plant_chooser (category list and search words) inside the plant specification; no menu entry or Library Browser button |
| 1360 | Chooser Name search: Common Name, Scientific Name, Variety Name, Pronunciation | Missing | NO SPEC (CB-606): search is by catalog name and tags (tools/terrain/scape.rs `PlantChoice::matches`) |
| 1360 | Chooser Type and Sub-Type check boxes | Missing | NO SPEC (CB-607): only the catalog category list (Trees > Deciduous, etc.) |
| 1360 | Chooser Needs: Sun, Water, Soil pH, Hardiness Zone range | Missing | NO SPEC (CB-608): no plant needs data (CB-74) |
| 1361 | Chooser Flowers and Foliage: Flower Color, Leaf Color (multiple), Bloom Time | Missing | NO SPEC (CB-609): no such data |
| 1361 | Chooser Height: height range From/To, Height at Maturity, Starting Age and Age at Maturity (months) | Missing | NO SPEC (CB-610): plants have a mature height and width only |
| 1361 | Chooser Special Characteristics and Object Type (3D Plants versus Plant Images) | Missing | NO SPEC (CB-611): none |
| 1361 | Chooser Search button, results list with count, toggle Common or Scientific Name, click a result to find it in the Library Browser, View Item opens Plant Information | Partial | NO SPEC (CB-612): CB-73; results list in the plant dialog; no scientific name, no Library Browser jump, no View Item dialog |
| 1361 | Show Hardiness Zones: regional USDA zone maps selectable from a drop-down | Missing | CB-74 (no hardiness data); no map viewer |

### 44.3 Plant Image Specification and Plant Specification (pp. 1362-1366)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1362 | Plant Image Specification opens from a selected plant image, from Create Plant Image or from New > Plant Image in the Library Browser | Missing | NO SPEC (CB-613): there is no plant image object; the plant run's dialog is the Plant Specification of a run (object.rs) |
| 1363 | Plant Image panel: Image File (Browse), 2D Plant Symbol drop-down or CAD block from the library or the plan, Height and Width with Retain Aspect Ratio and Reset Original Aspect Ratio, Elevation Reference with to Top and to Bottom, Center Point X and Y | Missing | NO SPEC (CB-614): run dialog has Plant, Canopy width, Height, Spacing and 3D form; no image file, symbol choice, aspect lock or elevation reference (object.rs) |
| 1364 | Plant Image Block Line/Fill Style (use image or block settings), Reverse Image, Image Always Faces Camera, Copyright text, preview (Glass House not available) | Missing | NO SPEC (CB-615): PlantForm::Billboard faces no camera (fixed crossed planes) |
| 1364 | Plant Image Transparency panel (as for Image Specification) | Missing | NO SPEC (CB-616): none |
| 1364 | Plant Information panel (type, sub-type, names, needs, colors, height and age) matching the Chooser, with "no change" icons for multi-selection | Missing | NO SPEC (CB-617): no plant info model |
| 1365 | Plant Description panel: Description, Lighting Comments, Hardiness Zone Comments | Missing | NO SPEC (CB-618): none |
| 1365 | Plant Image Layer panel (layer and Drawing Group) and Fill Style panel (default None/transparent; no effect on closed CAD blocks from the library) | Partial | NO SPEC (CB-619): Layer tab exists (object.rs layer text); Fill Style tab for plant runs not offered (REGIONS apply to regions only) |
| 1366 | Plant Label panel (label on the "Plants, Labels" layer), Components panel, Object Information panel, Schedule panel | Missing | NO SPEC (CB-620): plants appear in the Plant Schedule (L-23) but have no Label, Components, Object Information or Schedule tabs |
| 1366 | 3D plant symbols open the standard Plant Specification (a symbol object specification) and, with Open Symbol, the Symbol Specification | Partial | NO SPEC (CB-621): a Plants-catalog symbol placed from the Library opens the symbol dialog (dialogs/symbol.rs, LR6 Library Object Specification); the Terrain Plant tool makes runs instead of symbols |

### 44.4 Sprinklers (pp. 1366-1367)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1366 | Sprinkler Head tool: pick a sprinkler from the Select Library Object dialog, click to place any number; selectable and editable | Partial | NO SPEC (CB-622): CB-50; ours a Sprinkler run of heads along a polyline or spline (head spacing, radius, spray angle, riser) with its own dialog; heads are not library symbols and cannot be picked from the Library dialog |
| 1366 | Sprinkler Line tool: 2D lines and splines for irrigation pipe, drawn and edited like CAD lines, not shown in 3D | Missing | NO SPEC (CB-623): no sprinkler line object; the run path is the only line (landscape.rs LandscapeKind::Sprinklers) and CB-50 says heads are not connected to a supply |
| 1366 | Sprinkler display in plan shows the spray angle; Spray Area and Sprinkler Symbol fill styles set in the specification | Partial | NO SPEC (CB-624): CB-50; `Landscape::arc` spray angle drawn in plan (landscape_plan.rs); one fill style only |
| 1367 | Sprinkler heads appear in 3D; lines and splines do not; the Materials List counts heads and the total length of lines and splines; schedules may list them | Partial | NO SPEC (CB-625): sprinkler heads in 3D: landscape_mesh.rs; Materials List Landscaping counts plants and cut/fill soil, not sprinkler heads or lengths (plan-docs materials.rs); no Sprinkler schedule kind |
| 1367 | Select sprinklers of one type with Marquee Select Similar and Restrictive Selection; the Fill Style Painter paints the Spray Area, not the symbol | Missing | S-174 (Marquee Select Similar: Partial); no Fill Style Painter exists (CAD-73) |
| 1367 | Sprinkler Specification (a Symbol Object Specification) and Sprinkler Line/Spline specification like a Polyline Specification | Missing | NO SPEC (CB-626): our run dialog has Spray radius, Head spacing, Spray angle, Riser height only (object.rs) |

## 45. Materials Lists (pp. 1368-1392)

### 45.1 The Materials List tools (pp. 1368-1369)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1368 | A Materials List is a spreadsheet view of the model; changes in the model update every live list (and price edits flow back to the objects) | Works | L-33, L-34; dialogs/materials.rs rebuilds from the plan each frame; edits do not flow back to objects (price goes to the Master List by item key) |
| 1368 | Tools > Materials List submenu (Calculate Materials for All Floors, in Room, From Selection, Materials List Polyline, Master List, Management, Generate a Report) | Partial | NO SPEC (L-84): menus.rs Tools > Materials List... opens one window; no submenu |
| 1368 | Calculate Materials for All Floors (entire model) | Partial | L-33; window "All floors" check (`MaterialsScope::AllFloors`) |
| 1368 | Calculate Materials From Selection edit button (selected objects and their components; a selected room counts moldings, finishes and subfloor, not framing or openings) | Missing | NO SPEC (L-85): no selection-driven list (grep finds nothing); the By Surface tab covers a room, the floor or the plan |
| 1368 | Calculate Materials in Room (also on a room's edit toolbar): contents with floor, ceiling and facing wall finishes; objects count if their center is inside; distributed objects count if their path or region center is inside | Partial | NO SPEC (L-86): DECISIONS 131 By Surface tab with a Room region (dialogs/materials.rs RegionPick::Room); not on the room's edit toolbar; counts by surface and material, not by object center |
| 1369 | Materials List Polyline: calculate for a drawn area (editable shape, saved, reusable, hole support, convert from a CAD polyline) | Missing | NO SPEC (L-87): grep finds nothing |
| 1369 | Calculate Structural Materials for Deck edit button (includes accessories from the deck room's Components panel) | Missing | NO SPEC (L-88): deck framing exists (CB-86 Works, decks round 15) but no per-deck list button |
| 1369 | Master List (list of previously used materials with price, manufacturer, etc.) opened from the submenu | Partial | NO SPEC (L-89): L-36; Master List tab in dialogs/materials.rs (waste per category, stock lengths, unit price, supplier field in `MasterItem`); no manufacturer, code, markup, labor, quantity, Use or Default columns |
| 1369 | Materials List Management: list of saved Live Lists and Reports with Edit, Copy, Rename, Delete | Missing | NO SPEC (L-90): lists are not saved or named; the window is a single live view |
| 1369 | Generate a Report: freeze a live list as a static Report not linked to the model (editable, update to and from Master List) | Missing | NO SPEC (L-91): no static report (grep finds nothing); the list can be sent to a layout as a box or exported |
| 1369 | Preferences > Master List panel (current Master List file and the columns used) | Partial | NO SPEC (L-92): Preferences > Materials List page has waste, round up, show prices, all floors (MaterialsPrefs); the Master List file is fixed at ~/.plan-studio/master-list.json |
| 1369 | Accuracy rests on the model (heights, lengths, structural settings); CAD-drawn items do not count; custom materials need correct definitions; generic library shapes may count oddly | Partial | informational; model-driven take-off (materials.rs) matches the first point; Define Material calculation methods (part 4, Materials) |
| 1370 | Code, Comment, Manufacturer, Supplier and Price, Extra, Markup, Labor, Equipment entered on objects (defaults dialogs and library items) | Partial | NO SPEC (L-93): door/window Schedule tab holds Manufacturer, Model, Supplier, Comment (L-29); cabinets have Object Information (DECISIONS 56); no Price, Extra, Markup, Labor or Equipment fields on objects |
| 1370 | Material definitions decide how components are counted; framing as linear feet, cut list or buy list in the Structural Member Reporting dialog | Partial | NO SPEC (L-94): L-35; framing rows use stock lengths and board feet (materials.rs); Structural Member Reporting control: grep finds nothing |
| 1370 | Custom Materials List formulas replace default calculations | Missing | NO SPEC (L-95): no formula editor (see 45.5) |

### 45.2 Organizing lists (pp. 1370-1372)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1370 | Control what is calculated by category and subcategory, by layer set, and by columns shown | Partial | L-33; the Category filter exists; subcategories, the layer filter and column choice are not built (see below) |
| 1371 | Some parts of the model (floor and ceiling platforms) cannot be excluded; use a copy of the plan for part of a plan | Partial | informational; our scope choice is one floor or all floors |
| 1371 | Materials List by Layer Set (Options panel; default Layer Set Defaults) | Missing | NO SPEC (L-96): layer sets do not filter the list (grep finds nothing in materials.rs) |
| 1371 | Categories (Electrical, Framing, ...) shown in the ID column; each line's category can be changed in the list, the Master List or a Components panel; the category list is fixed | Partial | NO SPEC (L-97): ours has 11 fixed categories (Foundation, Framing, Roofing, Siding, Windows, Doors, Cabinets, Electrical, Fixtures, Interior Finishes, Landscaping; plan-docs materials.rs MATERIAL_CATEGORIES); the user cannot move a line to another category |
| 1372 | Subcategory column (empty by default; typed in the list or the Components panel) | Missing | NO SPEC (L-98): no subcategory |
| 1372 | Details dialog for selected line items: every column, Source Object, Find button to show the object in a plan view (opens when objects span floors) | Missing | NO SPEC (L-99): grep finds nothing; rows do not carry source objects (`MaterialLine` has category, item, size, quantity, key only) |
| 1372 | Appearance of lists: grid lines, colors, font set in the Specification dialogs | Partial | NO SPEC (L-100): dialogs/materials.rs uses the app style; see 45.3 Appearance panel |

### 45.3 Materials List Specification dialog (pp. 1372-1376)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1372 | Open from Tools > Active View > Edit Active View when a list or the Master List is open; the Master List dialog has only the Columns panel | Missing | NO SPEC (L-101): no specification dialog for the list; the Master List tab edits waste, stock lengths and prices |
| 1373 | Categories panel: check the categories shown (order as listed), Select All, Clear All, multiselect; hidden categories remain in the list and in exports | Partial | NO SPEC (L-102): dialogs/materials.rs Category combo shows one or all categories (hides the rest in export too); no multi-select checklist |
| 1374 | Columns panel: check columns, Move Up, Move Down | Missing | NO SPEC (L-103): columns are fixed: Category, ID, Description, Size, Count, Unit, Unit Price, Price (plan-docs COLUMNS) |
| 1374 | Options panel: Layer Set (with Define), Structural Member Reporting default, Restrict to Floor, Restrict to Supplier (all, none or a supplier) | Partial | NO SPEC (L-104): Restrict to Floor: floor or All floors (MaterialsScope); no layer set, member reporting default or supplier restriction |
| 1375 | Appearance panel: horizontal and vertical grid lines, solid lines, custom colors, font, size and styles, Reset to Defaults, preview | Missing | NO SPEC (L-105): none |

### 45.4 Columns (pp. 1377-1378)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1377 | Column set: ID, Use, Sub Category, Floor, Label, Supplier, Manufacturer, Code, Size, Description, Quantity, Count, Extra, Price, % Markup, Labor, Equipment, Total Cost, Default, Comment, Accounting Code, with a table of where each is available and editable | Partial | NO SPEC (L-106): ours shows 8: Category, ID, Description, Size, Count, Unit, Unit Price, Price (Total is a footer); Supplier lives in the Master List; Floor, Label, Manufacturer, Code, Extra, Markup, Labor, Equipment, Comment, Accounting Code, Default, Use, Quantity: absent (MaterialLine, MasterItem) |
| 1377 | ID column: automatically generated identifier per line; Size and Description are editable text (clear the text to restore) | Partial | NO SPEC (L-107): L-34; `number_rows` ids like FRM-003; Size and Description show but are not editable |
| 1378 | Count versus Extra (extra amount in the same unit, reflected in Count) and the Total Cost formula (Count + Extra) x Price x (1 + Markup/100) + Count x Labor + Count x Equipment | Missing | NO SPEC (L-108): our Price is quantity x unit price; waste is a percent per category in the Master List (L-36); no markup, labor, equipment or extra |
| 1378 | Accounting Code column (used for BuilderTREND export) | Missing | NO SPEC (L-109): `accounting_code` exists on material definitions (plan-materials) but not in the list |

### 45.5 Editing lists (pp. 1379-1382)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1379 | Expand and Collapse rows: a line with an arrow represents several objects, shown separately when expanded; not in Reports | Missing | NO SPEC (L-110): lines are totals; no expansion |
| 1379 | Find Object in Plan edit button: select a row to locate its object in the plan | Missing | NO SPEC (L-111): rows carry no object ids; compare L-27 (schedule rows select objects) |
| 1379 | Adding information: type Price, Supplier, Code, Comment, Manufacturer into a cell of a live list or report, then add to the Master List | Partial | NO SPEC (L-112): Master List tab edits unit prices and waste; the list's own cells are read only (dialogs/materials.rs) |
| 1379 | Tools > Update From Master List (look up Price, Supplier, Code by item, honoring Default) and Update to Master List (save a selected row; matching by Category, Size, Description, Label) | Missing | NO SPEC (L-113): prices come from the Master List automatically by key (`price_of`); there is no explicit update, default flag or row selection |
| 1379 | Change information in any column of an individual list or report; change a line's category/subcategory from the ID column | Missing | NO SPEC (L-114): read-only cells |
| 1379 | Changing count units (piece, linear ft/m, sq ft/m, cubic yd/m) by double-clicking a Count cell, with the same unit in Extra; Number Formatting dialog (units, formatting, accuracy) from the Count or Extra cell | Missing | NO SPEC (L-115): units fixed per row (ft, sq ft, cu yd, sheets, pieces); no per-cell unit or Number Formatting dialog (DIM-31 formats exist for dimensions only) |
| 1380 | Currency from the operating system's Region settings | Partial | NO SPEC (L-116): fmt_money prints "$1,234.50" always (materials.rs) |
| 1380 | Formulas and Count/formula tool tips showing more precise values when rounded | Missing | NO SPEC (L-117): no tool tips on cells |
| 1380 | Total Cost column using the Count, Price, % Markup, Labor and Equipment formula | Missing | NO SPEC (L-118): see Count versus Extra |
| 1380 | Live lists and Auto Rebuild: editing an auto-generated framing member, roof plane, foundation component or attic wall prompts to turn off Auto Rebuild or to edit defaults | Missing | NO SPEC (L-119): lists are read only, no prompt |
| 1380 | Copy parts of a list and paste into a Text object, word processor or spreadsheet | Partial | NO SPEC (L-120): Export CSV/Excel/PDF/Send to Layout instead (L-37); no clipboard copy of cells |
| 1381 | Custom formulas in Ruby syntax in editable cells ("=" prefix), Insert Macro with User Defined, Materials List Column, Object Specific and Parent Object macros; Apply Formula to Line Item or Source Object; Revert to Default; macros with Evaluate and Owner Object / Materials List Line Item contexts | Missing | NO SPEC (L-121): Ruby is out of scope (see chapter 48); no formula cells |
| 1381 | Number Formatting dialog (right-click a Count or Extra cell; same settings as Displayed Line Length Format) | Missing | CAD-108 Displayed Line Length Format: Missing |

### 45.6 Materials List Polylines (pp. 1382-1384)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1382 | Materials List Polyline tool (rectangular polyline, edited to any shape, can sit in schedules, hole support) with Calculate Materials List edit button | Missing | NO SPEC (L-122): no object (grep finds nothing) |
| 1382 | Materials List Polyline Defaults dialog (categories and floors in advance) | Missing | NO SPEC (L-123): none |
| 1383 | Included Floors/Categories grid (categories as rows, floors as columns; toggle selected, categories, floors, all; Include All Floors; Revert All Changes) | Missing | NO SPEC (L-124): none |
| 1383 | Included Objects choice: Intersected, Contained or by Center (No Change for mixed selections) | Missing | NO SPEC (L-125): none |
| 1384 | Polyline, Selected Line/Arc, Line Style, Fill Style and Label panels for the Materials List Polyline; rooms inside the polyline count whole-room finishes | Missing | NO SPEC (L-126): none |

### 45.7 Saving, printing, exporting (pp. 1384-1387)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1384 | Save Active View, Save Active View As (copy), Edit Active View for live lists and Reports; prompt to save a list on close; saved lists in the Project Browser and Management dialog | Missing | NO SPEC (L-127): lists are not saved objects (the Project Browser has Schedules and CAD Details, not Materials Lists, docks.rs BrowserNode) |
| 1385 | Print a Materials List with File > Print (Print Materials List dialog) | Partial | NO SPEC (L-128): Export PDF button and the construction set page carry the list (dialogs/materials.rs, L-37); File > Print does not print it |
| 1386 | Export Materials List dialog, File Type: Tab Delimited (TXT), Comma Delimited (CSV), Spreadsheet (XML for Excel), Web Page (HTML) | Partial | L-37, L-56; CSV and Excel (.xlsx) work; TXT, XML spreadsheet and HTML do not |
| 1386 | Export options: Include Column Headers, Include Hidden Columns, Open in Default Spreadsheet Editor, Export with Colors (XML/HTML) | Missing | NO SPEC (L-129): CSV always has headers; no options (to_csv) |
| 1386 | Export Units: Include Units with Amounts, in a New Column, or none; currency always included | Missing | NO SPEC (L-130): units are a column (Unit) always |
| 1387 | Third Party Formats: No Formatting or BuilderTREND (CSV only) | Missing | NO SPEC (L-131): grep builder.?trend finds nothing |
| 1387 | Export to commercial estimating programs (check that the program imports Chief Materials Lists; Chief gives no support for third-party software) | Out-of-scope | Excel and CSV are the exchange route (L-37) |

### 45.8 The Master List (pp. 1387-1389)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1387 | More than one Master List, one active (set in Preferences) | Partial | NO SPEC (L-132): single file ~/.plan-studio/master-list.json (`MasterList::load/save`); no second list or file choice |
| 1388 | Master List Category drop-down, Columns choice, Find field (search from the selected cell) | Missing | NO SPEC (L-133): Master List tab shows waste, stock lengths and the price rows for the current take-off only; no category, column or Find controls |
| 1388 | Update to Master List: matches Category, Size, Description, Label; creates a new Master List line when none matches; Update from Master List: last entry wins unless one is marked Default | Missing | NO SPEC (L-134): see 45.5 |
| 1389 | Edit the Master List: modify or delete lines (select row number, Delete), Use check boxes (items bought as one unit), Quantity threshold for quantity discounts, Default column; back up mmaster.mat | Missing | NO SPEC (L-135): price rows only; delete of a Master List line: not built; JSON file instead of mmaster.mat |

### 45.9 Object Components and Object Information panels (pp. 1389-1392)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1389 | Components panel in object dialogs lists the object's Materials List line items (components and indented subcomponents) with Add Line Item, Remove Line Item, Restore, Revert; Materials List Data table (heading, Formula, Value); extra info for rooms and terrain features; editable also for User Catalog items | Partial | NO SPEC (L-136): Wall Specification Components tab (L-35, DECISIONS 67) and Cabinet Components tab (DECISIONS 56) exist; no generic Components panel with formulas for the other objects (doors, windows, symbols, terrain, stairs, roofs) |
| 1391 | Object Information panel: Code, Comment, Description, Manufacturer, Supplier, with an Insert Macro button on each field | Partial | NO SPEC (L-137): door/window Schedule tab (Manufacturer, Model, Supplier, Comment), cabinet Object Information (manufacturer, model, description, notes); no Code field, no Insert Macro button; absent on walls, rooms, symbols, stairs, terrain (L-29) |
| 1392 | Custom Object Fields in Object Information (list of fields in the plan, Create New Field, a value per object) | Partial | BC-1/BC-2 Property Manager custom properties and the Properties tab (beyond Chief, same purpose); Project Information has a Custom Fields tab; not on the Object Information panel by that name |

## 46. Layout (pp. 1393-1423)

### 46.1 Layout preferences and defaults (pp. 1393-1394)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1393 | Layout background colour set in the Preferences Colors panel | Partial | NO SPEC (L-138): Page Setup has a "Layout background (warm off-white)" switch (dialogs/layout.rs PageSetup.page_background); no colour choice in Preferences > Colors |
| 1393 | Layout defaults via Edit > Default Settings > Layout (only CAD, dimensions and text can be drawn on layout pages); General Layout Defaults dialog is the General Plan Defaults | Partial | DS1/DS2 (Default Settings tree and generic pages); the Default Settings > Layout page exists (default_pages/plan.rs `layout`: page size, orientation, margin, border, box border, gap, box title) stored under `pages["layout.*"]`; the General Layout Defaults, CAD, dimension and text defaults are not separately kept for the layout |
| 1393 | Drawing Sheet Setup is file-specific and important in layout files | Partial | L-8; Page Setup (sheet size, orientation, margins, background, edge line weight) per layout file |
| 1393 | Layout Page Display Options (Tools > Layer Settings > Display Options): layer display for objects drawn on the page and for layout view box borders, not for objects inside views | Works | L-16; Layout Layer Display Options (Layout Box Borders, Layout CAD, Text, Title Block, Revision Clouds) in dialogs/layout.rs |
| 1394 | Layout Box Defaults dialog: default line style, fill style and label format of layout boxes | Partial | NO SPEC (L-139): Default Settings > Layout has Box border, gap and box title; no line style, fill style or label format defaults (default_pages/plan.rs layout) |
| 1394 | Page Templates (title block and border applied to chosen pages) | Partial | NO SPEC (L-140): L-10, L-7; the Page Template flag makes a page's boxes and CAD repeat on every other page; no assignment of a template to a chosen page and no multiple templates in use (see 46.14) |
| 1394 | Layout file templates (layout defaults and Page Templates saved as a template) | Partial | L-10; Save As Template / Apply Template as JSON (DECISIONS 36); Chief's .layout template files are not read or written |

### 46.2 The layout tools (pp. 1394-1396)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1394 | File > Send to Layout opens the Send to Layout dialog and sends the current view | Works | L-1, L-2; hotkey S, L |
| 1394 | Send Camera's View to Layout edit button on a saved cross section/elevation camera symbol (not already sent, not open) | Partial | L-1; Project Browser camera list Send to Layout (`LayoutCommand::SendCamera`); the edit button on the camera symbol: not found |
| 1394 | Tools > Layout > Referenced Plan Files lists plan files used by the layout and lets you relink them | Missing | NO SPEC (L-141): a layout belongs to its plan (Project::layout), so there are no external plan references (DECISIONS 61); import of a Chief layout would need it |
| 1394 | Open View edit tool opens the view that was sent (dynamic views only) | Works | L-6; Open Source View (plan boxes only; cameras from the Project Browser) |
| 1394 | Rescale Layout View edit tool opens Change Scale | Partial | NO SPEC (L-142): Layout Box Specification and box Scale drop-down (dialogs/layout.rs); no Change Scale dialog or No Scale choice for boxes |
| 1394 | Relink File edit tool on a layout view (Choose Layout File Reference dialog) | Differs | not needed: boxes keep their link by internal id (L-17) |
| 1394 | Layout Box Layers edit tool opens Layer Display Options for the selected view; Edit Layout (Plot Lines) | Partial | NO SPEC (L-143): plan boxes choose a layer set in the Layout Box Specification (Source tab); no per-box Layer Display Options window; Edit Layout: see 46.10 |
| 1395 | Page management commands in the Tools menu and the Project Browser: Insert Page Before / After, Duplicate Page, Delete Page, Exchange With Next / Previous Page | Works | L-7; menus.rs Layout menu; shell/layout_window.rs; Project Browser page list (docks.rs) |
| 1395 | Edit > Delete Objects deletes categories of objects and Page Information over a scope of pages in the layout | Partial | NO SPEC (L-144): dialogs/delete_objects.rs is for plan objects; the layout window deletes selected boxes/page CAD only |
| 1395 | Navigation: Page Up, Page Down, Change Layout Page; Window > Swap Views; Update Layout Views submenu | Partial | Previous/Next Page and Update Layout Views exist (menus.rs 2088, 2125); Go To Layout Page dialog: layout toolbar page picker (`C::GoToPage`); Swap Views: DS-* Window commands (round 15) |

### 46.3 Creating a layout file (p. 1396)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1396 | File > Open Layout, File > New Layout (blank file "Untitled.layout", first non-template page, layout template defaults and layers) | Partial | DECISIONS 61: File > New Layout opens the plan's layout; Layout > New Layout File makes another from a template; layouts are stored in the plan file (.psplan) instead of separate .layout files |
| 1396 | Open layout files are listed at the bottom of the Window menu and in the Project Browser; Swap Views and tab cycling include the layout | Works | DS10 tab ring (Ctrl+Tab includes the layout), Project Browser Layout section (docks.rs) |
| 1396 | Save the layout in the same folder as the plan; renaming or moving breaks the link | Differs | no external link (layout stored in the plan); Chief .layout import: unbuilt |

### 46.4 CAD and text in layout (pp. 1396-1397)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1396 | CAD, text and dimension lines can be drawn directly on a layout page (title block, border, notes) and edited like other views | Partial | L-16 (Line, Box, Polyline, Circle, Arc, Text, Text Box, Leader, Revision Cloud); layout dimensions: L-16 says page CAD has no dimensions, fillets or trim |
| 1396 | Layout Info text macros on pages (page number, revisions, ...) | Partial | L-9; title block macros %sheet.number%, %sheet.title%, %page.count%, %revision%, %date%, %scale% (plan-layout titleblock.rs) are not Chief's names; see 46.14 for the page macros |
| 1396 | Layout box labels customised to report view information (object-specific macros such as %label_position_and_orientation%) | Missing | NO SPEC (L-145): box label is free text with the scale note; macros are not expanded in box labels (grep finds none in layout box labels) |
| 1396 | Dimension lines drawn on the layout page position CAD and text; dimensions locating view objects belong in the original view | Partial | L-15; page-level dimensions: Missing per L-16 |
| 1396 | Pictures and PDFs can be imported into a layout | Works | L-5; `BoxSource::Image`/`ImageData`, Add Picture; PDF import follows underlay rules (L-46) |
| 1396 | Move a text or CAD object to another page via its Page number in the specification dialog | Partial | NO SPEC (L-146): boxes have a Page field and Copy Layout Box to Page (L-7); page text and CAD items are moved by cut and paste or not at all (no Page field in their dialogs) |
| 1396 | Page Template objects (title block, border) repeat on multiple pages | Partial | L-10; template pages repeat their boxes and CAD unscaled (DECISIONS 62) |
| 1397 | CAD, text and dimensions inside views sent to layout scale with the view; text size is chosen for the scaling at send time | Works | L-15; plan-layout render.rs printed-size styles print the same size at any box scale |
| 1397 | Text in a rotated layout view rotates with it when its Text Style has Rotate with Plan | Missing | NO SPEC (L-147): quarter-turn box rotation exists (L-4) but Rotate with Plan text styles are not honoured: grep finds none |
| 1397 | Global text macros in view text (drawing scale, plan file name) are only valid when the text is in the plan view, not on the layout page | Missing | NO SPEC (L-148): macros expand by view context only in titles; no view-level macro scoping |
| 1397 | Dimension labels in camera views sent to layout as Plot Lines with a fill (Colored Fill, Use Edge Line Defaults) | Missing | NO SPEC (L-149): Plot Lines are not built (see 46.7) |
| 1397 | Four callout types in plan views: standard Callout linked to a detail, Note callout linked to a Note Schedule, object label callout linked to a Schedule, Cross Section/Elevation callout with layout information | Partial | NO SPEC (L-150): Callout and Note tools exist (TXT rows, Notes in the Text flyout); door and window schedule callouts: L-26; camera callouts with their camera label: C-* camera rows; a callout linked to a view or layout page is not found; DECISIONS 102 covers detail names only |
| 1397 | Pictures, metafiles and PDFs imported onto a layout page, selectable and editable, can sit on Page Templates | Partial | NO SPEC (L-151): L-5, L-10; pictures and scanned PDFs work (BoxSource::Image/ImageData, L-46); metafiles (WMF, EMF) are not read (grep wmf/emf/metafile finds nothing) |
| 1397 | Large embedded pictures increase layout file size; limit large pictures | Partial | `BoxSource::ImageData` stored as base64 text in the plan file (DECISIONS 66) |

### 46.5 Sending views to layout (pp. 1398-1401)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1398 | Send to Layout from the File menu or the Project Browser (all view types except Floor Levels and Materials Lists) | Works | L-1; Project Browser Send to Layout for cameras and details; Materials List and schedules are added as boxes from the layout toolbar |
| 1398 | Restrictions: views go only to layouts with the same units and in the same Project, and cannot go from one layout to another | Differs | the layout lives in the plan, so units always match; layout-to-layout sending: n/a |
| 1398 | Send to Layout dialog is a dialog with a progress indicator in the status bar for large models | Works | L-2; `SendDialog`; Update Views thread with a progress bar for perspective boxes (L-5) |
| 1399 | Source View panel: view type and name; Send All Remaining Views to Layout With These Settings when several views are selected | Partial | NO SPEC (L-152): SendDialog shows the view; "Send All Floors to Layout" sends every floor in one go (C::SendAllFloors); multi-select send from the Project Browser: not found |
| 1399 | Choose Layout: all layouts in the project, or New Layout, with a Browse button for other layout files | Works | L-2; Layout file picker and name in SendDialog (`LayoutTarget`); Browse for a layout file on disk: not applicable (DECISIONS 61) |
| 1400 | Send Position: page number, Show Layout Page after sending | Works | L-2; SendDialog Page (existing or New page), Position (First free area, Centered, Click on page) |
| 1400 | Send Position option: Snap to Active CAD Point | Missing | NO SPEC (L-153): grep finds nothing |
| 1400 | Send Options: Entire Plan/View, Current Screen, Current Screen As Image (with Define for pixel size and transparent background) | Partial | NO SPEC (L-154): SendDialog sends the view at a scale ("Largest that fits" or a list scale), 3D picture width/resolution; no Entire Plan/Current Screen choice, no transparent background |
| 1400 | Link Saved Plan View check box (dynamic link to the Saved Plan View or the current floor, layer set, reference floor and Default Set) | Missing | NO SPEC (L-155): boxes name a floor and a layer set (BoxSource::PlanView); Saved Plan Views are not linked (grep finds none in plan-layout) |
| 1400 | Camera View Options: Live View with Update on Demand or Always Update; Plot Lines with Color Fill, Use Edge Line Defaults, Use Pattern Line Defaults | Missing | NO SPEC (L-156): elevation, section and camera boxes always redraw from the plan (live) with Update Views for perspective boxes; Plot Lines are absent (grep plot.?lines finds nothing) |
| 1401 | Scaling: Fit to Sheet (No Scale) at about half the sheet, or an exact U.S. and metric scale; Use Layout Line Scaling keeps line weights and dashes at sheet size | Partial | NO SPEC (L-157): scale list + "Largest that fits" (L-22); box Line weight scaling multiplier 0.1 to 5 (L-12); no "No Scale" boxes and no per-view layout line scaling switch |

### 46.6 Keeping views current (pp. 1401-1404)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1401 | Four view types: Dynamic (updates with the plan), Semi-Dynamic, Static (snapshot), Plot Line (mixed) | Partial | L-3, L-5; every plan, elevation, section and camera box is live; perspective and image boxes are rendered/stored (like semi-dynamic and static); the dynamic/semi/static choice is not exposed |
| 1401 | Dynamic views follow changes to the model, annotations, layer set and defaults | Works | L-3; layout_window.rs live redraw |
| 1402 | Saved and unsaved plan views: pony wall display, floor level, Reference Display, Color setting, Active Defaults, layer set and Rotate Plan View stay with the layout view; Create Saved Plan View and Unlink Saved Plan View edit tools | Missing | NO SPEC (L-158): grep create saved plan view finds nothing; a box stores floor, layer set and a quarter-turn rotation only |
| 1402 | Semi-dynamic views update by Update View, Update All Views, Update All Live Views, and when printed | Partial | NO SPEC (L-159): C::UpdateViews (Layout > Update Layout Views) refreshes perspective boxes; other update menu entries are not split into Update All Live Views / Update All Plot Line Views |
| 1403 | Static views (Current Screen As Image) are embedded pictures replaced rather than updated | Works | L-5; `BoxSource::ImageData`, Send to Layout from the 3D view sends a picture (DECISIONS 66) |
| 1403 | Update on Demand views lose quality when zoomed; updating restores resolution; GPU ray trace layout views run 20 samples then denoise, print at Maximum Samples | Partial | NO SPEC (L-160): perspective boxes carry their own dpi and samples, rendered by Update Views (L-5); print uses the same cached picture |
| 1403 | Plot Line views: surface-edge and material-pattern lines drawn as automatic vector lines in camera views and cross sections; semi-dynamic; updated only by you; image objects, dimensions in cameras, reference models and cross-section slider effects are left out; CAD in cross sections stays dynamic | Missing | NO SPEC (L-161): no Plot Lines mode (grep finds nothing); elevation and section boxes are already vector drawings (plan-elevation) |
| 1404 | Items not shown in layout views: the Reference Grid and camera symbols (camera callouts do show when the Cameras layer is on) | Partial | NO SPEC (L-162): render.rs omits grid; camera callouts in layout views: not found |

### 46.7 Displaying layout views (pp. 1404-1405)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1404 | Layout view layer sets: the active layer set is used at send time; Layout Box Layers edits or changes the set for dynamic and semi-dynamic views; changes affect every view that uses the set; static views cannot change | Partial | NO SPEC (L-163): L-4, Layout Box Specification Source tab picks a layer set (`layer_set`) for plan boxes; editing the set in a window opened from the box: not built |
| 1404 | Layout Box Borders layer shows borders; the border line style by layer and per box; box fill styles show when borders show | Partial | NO SPEC (L-164): L-4, L-16; Layout Layer Display Options and the box Line Style tab; box fill style: not built (Layout Box Specification has no Fill Style tab) |
| 1405 | Layout Box Labels layer: automatic labels (for instance "1st Floor" or the camera name) editable with text and object-specific macros, with edit handles, callout and marker label shapes linked to another view or page | Partial | NO SPEC (L-165): L-4; the caption under a box is free text with the scale; labels show the view name; macros and callouts/markers on labels: Missing |
| 1405 | Missing Layout Views: a selectable but invisible view means a broken link; a Caution symbol on unlinked boxes with Relink, Refresh Link, Ignore Invalid Links | Missing | NO SPEC (L-166): n/a for plan-held layouts; a box whose source is gone (deleted camera) shows an empty frame (BoxSource::Camera note in model.rs) |

### 46.8 Editing layout views (pp. 1405-1407)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1405 | Layout box edit handles like a closed polyline-based object | Works | L-6; 8 handles, nudge, rotate knob |
| 1405 | Layout box Specification, edit tools, and dimensions that move or resize boxes | Partial | NO SPEC (L-167): Layout Box Specification (General, Source, Line Style); dimensions to locate boxes: Missing |
| 1406 | Pan/Scale Layout Box edit tool: drag to pan inside the box and type the scale in two inline fields | Missing | NO SPEC (L-168): grep pan.scale finds nothing in the layout window |
| 1406 | Recenter Layout Box Contents and Scale Layout Box Contents to Fit edit tools | Missing | NO SPEC (L-169): not built |
| 1406 | Move a view to a different page by changing Page on the Line Style panel | Works | L-7; box Page field in the Layout Box Specification; Copy Layout Box to Page |
| 1406 | Copy and paste views across pages and files; preferably send again for independent control | Works | L-6; `copy_selected_to`, `duplicate_here`, Copy Layout Box to Page |
| 1406 | Scaled views keep their scale when the box is resized (blank space or cropping) | Works | plan-layout render clips at the box (`clip`); scale stays |
| 1406 | Non-scaled (Fit to Sheet / No Scale) views: corner handle with the Alternate edit behaviour resizes box and image together, other handles crop | Partial | NO SPEC (L-170): no No Scale boxes (see Scaling); Alternate edit behaviour on layout handles: not built |
| 1407 | Rescale a floor plan, CAD Detail or section/elevation view: Pan/Scale, Scale to Fit, Rescale Layout View (Change Scale dialog with No Scale / a scale / Use Layout Line Scaling) | Partial | NO SPEC (L-171): box Scale drop-down and the Layout Box Specification scale; the three tools: Missing |

### 46.9 Edge and pattern plot lines (pp. 1407-1409)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1407 | Edit Layout Lines tool: select, add and edit individual edge lines and material pattern lines of a Plot Line view without changing the 3D model (full CAD editing not available, CAD lines in sections not editable); lines are replaced when the view updates | Missing | NO SPEC (L-172): grep finds nothing |
| 1408 | Layout line editing: handles for size, angle and position; Angle, Object and Grid snaps in layout; delete | Missing | NO SPEC (L-173): no layout lines |
| 1408 | Layout Line Specification dialog: Line Type (Edge Line or Pattern Line), Line Weight with Use Default Weight, Line Style with Use Default Style and Library button, Line Color with Use Default Color | Missing | NO SPEC (L-174): not built |
| 1407 | Edge line colour, weight and style set by layer; pattern lines use the Define Material attributes; in Plot Line views a view-wide default overrides them | Partial | NO SPEC (L-175): L-12, L-13; elevation/section hatch and line weights come from layers and `wall_face_hatch` (no per-view override) |

### 46.10 Layout Box Specification dialog (pp. 1409-1413)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1409 | Open from a selected layout view (the settings depend on view types selected); Line Style, Fill Style and Label panels match the Layout Box Defaults | Partial | NO SPEC (L-176): L-4; dialogs/layout.rs BoxTab::{General, Source, LineStyle}; Fill Style and Label panels absent |
| 1410 | Linked View panel, Plan views: File Name, View Name, View Type, Relink button; Dimensions Number Height (legacy only); Saved Plan View Options (Current Floor, Edit View); Floor Level Options (Current Floor, Current Default Set, Show Color); Poche; Reference Display settings | Partial | NO SPEC (L-177): Source tab: floor and layer set for a plan box; no file name or view type text, no Default Set, Show Color, Poché or Reference Display choices on plan boxes (poche is drawn only in section and elevation drawings, plan-layout render.rs) |
| 1411 | Linked View panel, Camera views: Live View (Update on Demand / Always Update), Plot Lines with Color Fill, Edge Line Defaults and Pattern Line Defaults (weight and colour) | Missing | NO SPEC (L-178): no Plot Lines (see above) |
| 1412 | Box Scale panel: No Scale or a scale, Use Layout Line Scaling, Scale Layout Box Contents Only (resizes the box with the scale) | Partial | NO SPEC (L-179): L-4, L-12; Scale drop-down and Line weight scaling; no No Scale and no "box follows scale" switch |
| 1413 | Layer Set panel (active layer set of unsaved views) | Missing | layer set is a Source tab choice for plan boxes; separate panel: n/a |
| 1413 | Layout Box Polyline, Selected Line/Arc panels | Missing | NO SPEC (L-180): boxes are rectangles with 8 handles; position and size fields exist in the General tab |
| 1413 | Layout Box Line Style panel (border style and page) and Fill Style panel (fill shows only when borders show) | Partial | NO SPEC (L-181): Line Style tab (line weight, border switch); no fill style |
| 1413 | Layout Box Label panel (label shows on the Layout Box Labels layer with the layer's Text Style) | Partial | NO SPEC (L-182): Label field in General (free text); no text style choice or macros |

### 46.11 Opening views, links, protecting files (pp. 1413-1416)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1413 | Double-click a dynamic or semi-dynamic layout view to open the original view; changes update the layout view; layer set changes affect every view using it | Partial | NO SPEC (L-183): L-6 Open Source View (plan boxes); double-click opens the Layout Box Specification (not the source view) |
| 1414 | Active Defaults of a plan view, cross section or CAD Detail sent to layout become associated with the layout view and are restored when the original view is opened | Missing | NO SPEC (L-184): Default Sets exist (DS-*) but are not tied to layout boxes |
| 1414 | Protecting layout links (same folder, never rename or move plan files, File > Backup Entire Layout, do not delete layer sets used by layouts) | Differs | not needed: the layout is inside the plan; File > Backup Entire Plan zips the plan and its pictures (files.rs) and the Archives hold older versions (DECISIONS 22) |
| 1415 | Finding missing files: breadth-first search for linked plan files, then inside zip files; Warning, Referenced Plan Files dialog, Caution symbol on unlinked boxes with Relink, Refresh Link, Ignore Invalid Links | Differs | n/a (no external plan files) |
| 1415 | Relinking a layout view to a different view via the Relink edit tool or the Layout Box Specification; Referenced Plan Files relinks all references to equivalent views in another plan | Differs | n/a (DECISIONS 61) |
| 1415 | Missing Layer Set dialog (layout box information, replacement layer set from Available Layer Sets, Define button) | Missing | n/a for in-plan layouts; a missing layer set falls back to the plan's set by name (BoxSource::PlanView doc) |

### 46.12 Layout page management (pp. 1416-1421)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1416 | Layout pages numbered 0 to 1000; Project Browser lists pages in use (templates, pages with content, blank pages) with icons and a context menu (view, edit information, move, delete) | Works | L-7; docks.rs Layout section page_list; sheet numbers `A-{number}`; icons and the 0-1000 range: Partial (not capped) |
| 1417 | Drag a page in the Project Browser to change its page number | Missing | NO SPEC (L-185): Exchange With Next/Previous only; no drag reorder |
| 1417 | Current layout page indicator between Page Up/Down buttons; the current page is where views go | Works | L-7; layout toolbar page picker and arrows |
| 1417 | Navigation: arrow buttons, Go To Layout Page dialog (click the number), Tools > Layout > Page Up / Page Down, Shift+N (Page Up) and Shift+M (Page Down), double-click a page in the Project Browser, Show Page, Open Page View | Partial | NO SPEC (L-186): arrows, page picker (`GoToPage`) and Project Browser clicks exist; Shift+N / Shift+M are listed in the hotkey table (chief-hotkeys-resolved.md) and mapped by name (toolbar/config.rs), not verified in the layout window; Open Page View in a new window: absent |
| 1417 | Layout pages that are blank do not print, even with border and title block | Works | L-18; print range skips blank pages (dialogs/print.rs) |
| 1417 | Add pages: send a view to a new page or draw on it; Insert Page Before / After; Duplicate Page; Delete Page (also from the Project Browser context menu) | Works | L-7; menus.rs; Layout > Insert Page Before/After, Duplicate Page, Delete Page |
| 1418 | Page Templates assigned to other pages cannot be deleted, nor can page zero; Exchange With Next / Previous (not on page 1000 / 0) | Partial | NO SPEC (L-187): L-7; the template page: delete guard unverified; Exchange With Next/Previous exist |
| 1418 | Custom page numbering: a Label with a prefix and "#" (for example "A-#") numbered in sequence across pages with the same prefix; template pages' labels do not pass to pages | Missing | NO SPEC (L-188): sheet numbers are plain integers shown as `A-n` (LayoutPage.number); no Label with # pattern |
| 1418 | Page macros %layout.label%, %page%, %page.print%, %numpages%, %lastpage% (print numbering skips blank pages) | Missing | NO SPEC (L-189): title block uses %sheet.number%, %page.count% (titleblock.rs); Chief's names are not recognised; no printed-number counting |
| 1418 | Layout Page Templates: title block and border drawn once and assigned to pages; multiple templates allowed; a template page does not print in a range (it prints as Current Sheet) | Partial | NO SPEC (L-190): L-7, L-10; one template mechanism (flag); a template page is not printed in a range; single template per layout rather than per-page assignment |
| 1419 | Layout Page Zero is the default Page Template with the title "Default Page Template", assigned to all pages, never deletable | Differs | page 0 is the cover sheet (construction set); template pages are flagged freely (DECISIONS 62) |
| 1419 | Edit Page Information dialog (menu, toolbar button, Project Browser): feeds layout tables and macros | Partial | Page Specification (title, sheet number, template flag, own sheet size, no title block); dialog name and fields differ (DECISIONS 62) |
| 1420 | Page Information fields: Selected Page picker, Label, Title, Description, Comments; Include in Layout Table (cleared automatically for templates) | Missing | NO SPEC (L-191): Page Specification has Title and Sheet number only; no Label, Description, Comments, Include in Layout Table |
| 1420 | Page Template Options: Use as Page Template; Assign Page Template drop-down for the page | Partial | NO SPEC (L-192): Use as Page Template exists; the assign drop-down does not (L-7) |
| 1420 | Page Revisions: a table of revisions of the page with New, Edit, Delete, Move Up, Move Down | Missing | NO SPEC (L-193): revisions are plan-wide rows in Project Information > Revisions (project_info.rs); clouds add rows (DECISIONS 35); not per page |
| 1421 | Revision Specification dialog (Tools > Layout > Add Layout Revision): Revised Pages, Label, Date (auto), Revised By (default Designer Information), Description, Include in Revision Table | Partial | NO SPEC (L-194): Project Information > Revisions rows hold label/date/description; no Revised Pages or Revised By, no Add Layout Revision command |
| 1421 | Layout Page Table (Tools > Layout > Layout Page Table, click a page to place): lists pages with Page Information and data, can sit on a Page Template | Partial | NO SPEC (L-195): L-11; Layout Page Table dialog (dialogs/layout.rs "Layout Page Table") and Sheet Index box; placement by click on a page; table is edited like schedules: Partial |
| 1422 | Layout Revision Table placed by click, page-specific, can sit on a Page Template and list only that page's revisions | Partial | NO SPEC (L-196): L-54 REVISIONS table in the title block (plan-layout titleblock.rs); not a placeable table and not per page |

### 46.13 Printing layout files (pp. 1422-1423)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1422 | Print a layout with File > Print > Print (Print Layout dialog); Drawing Sheet Setup options apply to all pages | Works | L-18; dialogs/print.rs for_layout |
| 1422 | Pages with no data do not print; only pages with views, text, CAD or dimensions print; Page Templates only print as Current Sheet | Works | L-18, L-7; print range skips blank and template pages |
| 1422 | Physically Based Ray Trace live views print after 20 samples at 96 dpi | Partial | perspective boxes use their dpi (default 80) and samples (`DEFAULT_PERSPECTIVE_SAMPLES`) (L-5) |
| 1422 | Printing to scale: the layout's drawing scale should be 1 in = 1 in so views print at their scales | Works | L-21; layout prints at box scales on the sheet (plan-layout print.rs) |
| 1422 | Printing services: PDF writer, print to file, paper size of the print service in Drawing Sheet Setup | Works | L-18, L-20; Print dialog custom paper sizes |
| 1423 | Check plots: print a large sheet at a reduced scale fraction on smaller paper to check it first (see 47.5) | Missing | NO SPEC (L-197): grep check plot finds nothing |
| 1423 | Export layout pages to DXF/DWG in scaled paper units rather than model units | Partial | NO SPEC (L-198): L-44 (DXF R12 export of plan floors; layout page DXF export: not found) |

## 47. Printing and Plotting (pp. 1424-1445)

### 47.1 Introduction, terminology, printers (pp. 1424-1426)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1424 | Output routes: print from layout, print a view directly, print to PDF, export an STL file for a 3D printer | Partial | L-18, L-20 (print dialog, Export Layout PDF); an STL, OBJ or 3DS export of the model is Missing (L-55: only glTF is exported) |
| 1424 | Student, Academic and Presentation versions print a non-editable watermark | Differs | licensing; Plan Studio has no editions |
| 1424 | Terminology: Drawing Sheet size versus paper size (a sheet bigger than the paper prints across several pages), Check Plot, Line Weight, Drawing Scale | Works | L-18 tiling onto smaller paper; Page Setup sheet size and the Print dialog paper size are separate choices |
| 1425 | Printer drivers, Print to File through a driver (Windows only, not on the Mac system print dialog), Clear Printer Information | Out-of-scope | Print dialog lists CUPS printers (macOS, Linux) and prints through `lp`; drivers, paper trays and Print to File are the operating system's (L-18 note) |
| 1425 | File > Print > Clear Printer Info clears the printer-specific data (paper sizes) saved with a file, used when building templates without a printer | Out-of-scope | plans store no printer data, so there is nothing to clear |
| 1425 | Drawing Sheet Setup is view-specific: plan view, each cross section/elevation and each CAD Detail has its own sheet size, margins, orientation and drawing scale; new views inherit the plan view settings | Missing | NO SPEC (L-199): Page Setup is one sheet per layout file; plan views use the layout's sheet and the Print dialog's own scale (dialogs/layout.rs PageSetup, menus.rs 479) |
| 1426 | Drawing Scale in Drawing Sheet Setup is the default scale for the active view's Print, Printed Size Input and Send to Layout dialogs | Partial | NO SPEC (L-200): Send to Layout and Print choose scales independently; no view-level drawing scale |
| 1426 | Print View settings remembered per view type (plan, cross section/elevation, layout, Materials List, CAD Details, Time Tracker Logs) across files; Remember Print Settings after Printing switch | Missing | NO SPEC (L-201): Print dialog starts from defaults each time (PrintDialog::for_plan/for_layout); settings persist per session only if at all (not verified) |
| 1426 | Display controls for printing: objects print only if visible in Layer Display Options; a custom layer set for printing; layout view layers set on the box | Works | L-4, L-18; layer sets on boxes; print uses the active set |

### 47.2 Print tools and Drawing Sheet Setup (pp. 1427-1431)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1427 | File > Print submenu tools: Drawing Sheet Setup, Scale to Fit, Center Sheet, Print Preview, Print, Print Image, Export PDF, Customize Sheet Sizes, Clear Printer Info | Partial | NO SPEC (L-202): menus.rs Print submenu: Print, Print Preview, Drawing Sheet Setup, Print Image, Print Model, Print Layout, Export Layout PDF; Customize Sheet Sizes is in the Layout menu; Scale to Fit, Center Sheet and Clear Printer Info are absent |
| 1427 | Scale to Fit (picks a scale that fits the plan to the sheet and recentres the sheet) and Center Sheet (moves the Drawing Sheet relative to the drawing, per floor) | Missing | NO SPEC (L-203): grep scale to fit and center sheet find nothing; the Drawing Sheet rectangle is fixed to the sheet size at the plan origin (View > Drawing Sheet) |
| 1427 | Display toggles: Color, Line Weights, Drawing Sheet (preview relative to sheet), Reference Grid (does not print) | Works | L-19; View menu Color, Line Weights, Drawing Sheet, Reference Grid (menus.rs 1888-1940) |
| 1428 | Drawing Sheet Setup dialog (plan, section/elevation, CAD Detail or layout; not perspectives): Drawing Sheet Orientation and Size with Customize, Show Drawing Sheet in View | Partial | NO SPEC (L-204): Page Setup (sheet size list with customised sizes, orientation, margins, background, edge line weight); the Show Drawing Sheet switch is the View > Drawing Sheet toggle |
| 1428 | Drawing Scale for the view, two-part scale (1/4 in = 1 ft, 1 m = 50 m; 1 ft = 1 ft for layouts) with U.S. and metric units chosen independently | Partial | NO SPEC (L-205): L-22; scale list and custom ratio in the Print dialog (`ScaleChoice`) and in box scales; not stored per view |
| 1429 | Printer for View: choose a printer per view type, Remember Print Settings after Printing, Choose button, Default Printer for View dialog (printer, orientation, paper size, paper source) | Missing | NO SPEC (L-206): printer is picked in the Print dialog each time (`lpstat -p`) |
| 1429 | Drawing Margins: Top, Bottom, Left, Right with Populate from Printer | Missing | NO SPEC (L-207): one margin value (inches) for the page and in the Print dialog; no per-edge margins or printer population |
| 1429 | Advanced Line Weights: Use 1 for all line weights (Home Designer compatibility), Line Weight Scale (denominator and unit; layout and its plan views must agree) with a preview of weights at the scale | Partial | NO SPEC (L-208): L-12 pen weights in 1/100 mm with a 0.1 to 5 multiplier per box; no explicit line weight scale per file and no preview; Chief's default scale is 1 = 1/100 mm which matches |
| 1430 | Customize Sheet Sizes dialog: New, Copy, Delete, Edit (description, dimensions, units); data in the sheetSizes.sheet file in the Data folder (program-wide) | Partial | L-8; SheetSizesDialog (custom named sizes, hide standard sizes, Daniel's preset) stored per layout file (DECISIONS 63); the manual says the list is program-wide; no Copy/Edit-units |

### 47.3 Print Preview, display toggles, Drawing Sheet, Center Sheet (pp. 1431-1432)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1431 | Print Preview toggle shows how the view will print (drawing sheet, line weights, cameras, CAD points, text and dimensions as printed); only for scalable views; colour follows Print in Color | Works | L-19; Print Preview window and Drawing Sheet outline (DECISIONS 64) |
| 1431 | Reference Display prints if visible; Color toggle gives black and white or grayscale per the Colors preference; Reference Grid does not print | Works | L-19; Color toggle and print colour modes (Color, Grayscale, Black and white) |
| 1431 | Line Weights toggle: on-screen weights and dashed lines as printed, zoom to see them; also a Preferences > Appearance option | Works | L-12, L-19; View > Line Weights |
| 1432 | Drawing Sheet is an object when displayed: edit handles to move or resize it (prefer Setup), dimensions can locate its edges; cannot be rotated or copied; blue border shows the printable area | Partial | NO SPEC (L-209): View > Drawing Sheet draws the sheet outline (editor/render.rs); not selectable, not movable, no printable-area border |
| 1432 | Center Sheet is stored per floor and does not move object coordinates | Missing | NO SPEC (L-210): see Center Sheet above |

### 47.4 Printing to scale, multiple pages, PDF (pp. 1432-1434)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1432 | Orthogonal views (plan, orthographic 3D with the Vector View technique, CAD Detail, layout pages) print to scale; scale from Setup is inherited and can be overridden in Print and Send to Layout | Partial | L-18, L-22; plan, layout pages and details print to scale (Print dialog Scale list); Orthographic 3D views print to scale only as vector: n/a to our 3D view |
| 1433 | Larger architectural and engineering scales for property layouts (1 in = 50 ft, 1:200 m) | Works | L-22; scale list covers 1/16 in to 3 in and 1 in = 10 ft... 100 ft and metric |
| 1433 | Perspective views cannot be scaled; they print through Print Image with a size as a percentage of the page (Fit to Paper %, 50 percent example) | Works | L-18; Print Image dialog (Width, Scale Fit the plan); Print Model uses paper, orientation and margin; percent-of-page for perspective: Print dialog "Perspective views" DPI row |
| 1433 | Check Plot at a scale fraction (for example 1/2) on smaller paper, with paper size set automatically; resets to To Scale / Fit to Paper afterwards | Missing | NO SPEC (L-211): grep check plot finds nothing |
| 1433 | Printing text, dimensions and line styles: text and dimension numbers scale with the sheet; line styles scale when printed and when sent to layout under conditions; true-type fonts recommended | Works | L-15, L-12; printed-size text styles; fonts: docs/fonts.md |
| 1434 | Printing across multiple pages when the scale does not fit: 2 percent overlap, crop marks where to cut, solid line at the drawing sheet boundary, grey page-break lines on screen with Drawing Sheet and Fill Window | Works | L-18; Print dialog tiling with overlap and tile marks; screen page-break lines: Print Preview shows tiles |
| 1434 | Printing to a PDF file: choose "Chief Architect Save as PDF" or another PDF writer as the printer, or File > Export > Export PDF; Save as PDF or Save to Project/Assets | Works | L-20; PDF file destination and Export Layout PDF (the Save to Project/Assets button belongs to Chief Project Management, see 47.7) |

### 47.5 Line weights (pp. 1434-1437)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1434 | Line weight as a whole-number numerator of a fraction (1/100 mm or 1/1000 in) assigned to objects, patterns and layers; Line Weight Scale in Drawing Sheet Setup | Works | L-12; pen weights in 1/100 mm per layer (Layer.line_weight) |
| 1435 | Every view sent to layout and the layout file must share the same Line Weight Scale; scale saved in templates | Partial | both use 1/100 mm; the scale is not a setting that can differ, so the pitfall cannot occur |
| 1435 | Ways to set line weight: by layer, in some specification dialogs (Line Style panel), by wall type (Wall Type Definitions), material pattern lines (Define Material), fill pattern lines in object dialogs, surface edges in the Print dialog, dashed end-cap length in Preferences CAD panel, weight 0 draws a one pixel hairline | Partial | NO SPEC (L-212): layers and wall types carry line weights (L-12); Define Material pattern line weight (part 4); surface edge weight in Print: absent; weight 0 = hairline in Print Preview when line weights are off (DECISIONS 64); end-cap length setting: CAD prefs (part 2 audit) |
| 1435 | Line weights and styles scale with the print; a view sent to layout at another scale scales them further (a weight of 20 at 1 mm = 50 mm sent at 1 mm = 25 mm prints 40); Use Layout Line Scaling prevents that | Works | L-12; the box Line weight scaling multiplier and the layout box scale (plan-layout render.rs); Chief's Use Layout Line Scaling check box is a different model: pen widths are paper units, not scaled (L-12 note) |
| 1436 | Exact line weights need a Vector View sent to layout with Plot Lines; in the example, dashed lines scale with the view when Layout Line Scaling is off | Partial | NO SPEC (L-213): Plot Lines: Missing; dashes and widths in paper units (L-12) |
| 1437 | Printer limits: a 150 dpi printer cannot show a difference between 1/150 and 1/300 in; weight 0 prints as thinly as possible; default scale 1 = 1/100 mm suits most standards | Works | informational; Print dialog DPI row for perspective only |

### 47.6 Watermarks (pp. 1437-1440)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1437 | View > Watermark toggle: a customizable text or image watermark on any view, on every layout page when on; saved per view in plan files and in view specification dialogs; file-specific; can be in templates | Missing | L-53 (Watermark: Missing); no watermark option on screen or in print |
| 1437 | Printing a view with a watermark: Show Watermark in the Print View dialog, toggle before printing perspective views | Missing | L-53 |
| 1438 | Watermark Defaults dialog (Edit > Default Settings > Watermark, or Define in the Print View dialog) shows a live preview behind the dialog | Missing | NO SPEC (L-214): not in the Default Settings tree (dialogs/defaults.rs) |
| 1438 | Watermark Type Text: text, colour, Print Size (baseline to cap A height), font | Missing | NO SPEC (L-215): none |
| 1439 | Watermark Type Image: file or Resource selection (Select, Import, Browse, Edit Path), Delete From Plan, Ratio to Sheet | Missing | NO SPEC (L-216): none |
| 1439 | Watermark General: Layout (such as Tile, Border, Fit to Sheet), Angle, Transparency, Marks per Row and per Column, Margins (Use Drawing Sheet Margin, Top, Bottom, Left, Right), Update Automatically and Update button | Missing | NO SPEC (L-217): none |

### 47.7 Print View and Print Image dialogs (pp. 1440-1445)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1440 | Print View dialog (plan, 3D Vector View, layout, CAD Detail, Time Tracker Log, Materials List) from File > Print > Print; Export PDF is a similar dialog | Partial | NO SPEC (L-218): L-18; PrintDialog for plan views and layouts; Materials List and CAD Details print through their own exports; Time Tracker: not built |
| 1440 | Print dialog saved settings are global per view type; Sheets and Copies reset; Print Source follows the Drawing Sheet toggle | Missing | NO SPEC (L-219): see Print View settings above |
| 1441 | Destination: printer name or "Chief Architect Save as PDF", DPI | Partial | NO SPEC (L-220): Destination (PDF file, System printer, Open in viewer) with printer list; DPI only for perspective views |
| 1441 | Paper: Orientation, Size, Source; "Match Print Source" size uses the view's Drawing Sheet Setup | Partial | NO SPEC (L-221): paper list with custom sizes (including the layout's custom sizes), orientation, margin; no paper source (tray) and no Match Print Source (X18 new) |
| 1441 | Print Range (layouts only): All, Current Sheet, Sheets (comma or dash list; list order gives collated copies); templates and empty pages do not print | Works | L-18; Print range "Pages" (dialogs/print.rs) |
| 1442 | Print Source: Drawing Sheet (whole sheet) or Current View (what is on screen) | Missing | NO SPEC (L-222): grep print source finds nothing |
| 1442 | Drawing Scale: Fit to Paper (default 95 percent, global), To Scale, Check Plot | Partial | NO SPEC (L-223): L-18, L-22; Fit to page, 100 percent, percentage, drawing scale, custom ratio; no 95 percent default note and no check plot |
| 1442 | Options: Copies with Collate, Include Watermark with Define, Print in Color (grayscale or black and white via Preferences Obey Color On/Off) | Partial | NO SPEC (L-224): Copies and colour modes (Color, Grayscale, Black and white) exist; no Collate (grep collate: none), no watermark; Obey Color On/Off preference: absent |
| 1443 | Advanced Options: Open System Print Dialog button (not with No Printer) | Missing | NO SPEC (L-225): no OS print panel (L-19 note, out of scope there); the button is not offered |
| 1443 | Preview pane inside the dialog: page selector, zoom in/out/fit, Update Automatically/Update, drawing sheet in white, progress | Works | L-19; a separate Print Preview window with Previous, Next, Fit (dialogs/print.rs PrintPreviewDialog); the Print dialog has a Print Preview button instead of an embedded pane |
| 1443 | Information messages below the preview about page size, resolution and scale to prevent unwanted output | Missing | NO SPEC (L-226): not built |
| 1443 | Print/Save: Print for a printer; Save as PDF or Save to Project/Assets for a PDF destination | Works | L-18, L-20; Print and Save as PDF (file chooser); Save to Project/Assets has no counterpart (no Chief Project Management) |
| 1443 | Print Image prints pixels rather than vectors; the only way to print Ray Trace and most 3D views; denoise first for GPU ray traces | Partial | L-49 (Export Picture) and Print Image dialog (dialogs/print.rs ImageDialog: width, scale Fit the plan) prints the floor plan lines as a PNG; Print Model prints a ray-traced camera view (width/resolution/quality, paper) |
| 1444 | Print Image dialog: Destination, DPI, Paper, Copies, Advanced Options, Preview and Information, Print/Save | Partial | NO SPEC (L-227): ImageDialog (Width/Height, Quality, Scale) and Print Model (Camera, Resolution, Quality, Paper, Destination); no DPI, copies or in-dialog preview |

## 48. Ruby in Chief Architect (pp. 1446-1461)

### 48.1 User Defined Macros (pp. 1446-1447)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1446 | Ruby 2.4.0 scripting for custom macros used in object labels and custom Materials List calculations; Ruby macro code bookended by percent signs in text objects and labels | Out-of-scope | Ruby is out of scope (the project brief); there is no scripting engine (grep ruby finds nothing) |
| 1446 | Macro uses: shortcuts for repeated text, custom object labels, labels inside text objects, callouts and markers, Object Information, Materials List formulas | Partial | TXT-20 Text Macro Management (user %macro% shortcuts and nested macros, plan-core text_styles.rs) covers text shortcuts only; no evaluated macros, no labels/Object Information/formulas |
| 1446 | Two macro categories: text macros (global info, no calculations) and Ruby macros (Name-Value Pairs, calculations, scripts, with Context Owner Object, Referenced Object or Materials List Line Item) | Partial | NO SPEC (TXT-61): text macros exist (BUILT_IN_MACROS: room.name, room.number, room.area, plan.name, plan.date, floor, floor.number, floor.count, floor.height); Chief's global macro names differ; Ruby macros do not exist |
| 1446 | Macros update whenever the display is redrawn (zoom, pan, View > Refresh Display) | Works | text macros are expanded each time the text is drawn (text_styles.rs expand_macros); View > Refresh Display exists (menus.rs) |
| 1447 | Name-Value Pairs (NVPs) and publishers: an object's attribute names reported by a macro (for example %height% for a cabinet) | Out-of-scope | no NVP model; the Property Manager custom properties (BC-1) play a similar role for user fields |
| 1447 | Creating and evaluating custom macros in the Edit Text Macro dialog and the Ruby Console | Out-of-scope | not built |

### 48.2 Publishers, name-value pairs, context (pp. 1447-1452)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1447 | Owner Object versus Referenced Object publishers; list the NVPs of a selection with owner.names or referenced.names in the Ruby Console; room NVPs work only as owner macros | Out-of-scope | not built |
| 1448 | Related objects of a publisher (countertop, door panels, moldings, room) and the %object_properties% User Defined macro that lists all NVPs, publishers and collections for a text with an arrow | Out-of-scope | not built |
| 1448 | Collections: access by index or Enumerable blocks (layers of wall assemblies, cabinet panels) | Out-of-scope | not built |
| 1448 | A text object inside a room works as a custom room label using room.nvp_name; architectural objects can report the room they are in in labels, schedules and the Materials List | Partial | NO SPEC (TXT-62): text macros room.name, room.number and room.area expand for text under a room (BUILT_IN_MACROS); other room attributes (finishes, ceiling height other than floor.height) are not available |
| 1449 | NVP name rules and value types (Integer, Float, Measurement, TrueClass, FalseClass, NilClass, String, Symbol, NVPublisher, Collection, Hash for Custom Object Fields), _is_default NVPs | Out-of-scope | not built |
| 1449 | Macro contexts: non-evaluated text macros, Owner Object, Referenced Object (Text Line with Arrow), Materials List Line Item, None (context stated in the Value) | Out-of-scope | user macros are non-evaluated text only (text_styles.rs TextMacro) |

### 48.3 Measurement and NumberFormatter (pp. 1452-1454)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1452 | Measurement class with hard-coded units, to_s/to_f/convert_to/type/unitless?, default units by category, custom unit conversions | Out-of-scope | not built (Unit Conversions preference exists, part 1) |
| 1453 | Numeric to Measurement conversion helpers (inch, in, foot, ft, yard, yd, mm, cm, dm, m; sq_ and cu_ prefixes) | Out-of-scope | not built |
| 1453 | NumberFormatter class (unit, show_unit, leading and trailing zeros, fractions, decimal places, denominator, thousands separator, reduce_fractions) | Out-of-scope | not built; the dimension formats (DIM-31, DECISIONS 84) cover the same formatting for dimensions |

### 48.4 Text Macro Management, Ruby Console, migration (pp. 1455-1461)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1455 | Text Macro Management dialog (CAD > Text > Text Macro Management): list of user-defined macros, Valid check, Macro Value and Expanded Macro Value | Works | TXT-20; dialogs/text/manage.rs TextMacroDialog (name, text; names unique and not a built-in) |
| 1456 | Edit, New, Copy, Delete, Import, Export, Migrate buttons; Show Evaluation Error; object-specific macro management on library objects in an unlocked library | Partial | NO SPEC (TXT-63): New, Edit and Delete exist; Copy, Import, Export, Migrate, Show Evaluation Error, per-library-object macros: absent |
| 1457 | Edit Text Macro dialog: Name (unique), Value (Ruby), Insert button for existing macros, Evaluate with Context, Original/New Result | Partial | Name and Text with nested macros up to four deep; no Evaluate, Context or results |
| 1457 | Importing and exporting macros between plans, layouts and library objects, with the name-conflict dialog (rename, discard, replace, do for all) | Missing | NO SPEC (TXT-64): not built |
| 1458 | Tools > Ruby Console: input and output fields, > and => markers, resizable, owner/selected/referenced, names, puts, $ globals | Out-of-scope | not built |
| 1459 | Ruby Console interactive tutorial (show, toc) | Out-of-scope | not built |
| 1460 | Migrating legacy Ruby code from X11 and earlier (Floats to Measurements): Migrate Formula, Revert to Legacy, Ruby Migration dialog in Materials List and Text Macro dialogs | Out-of-scope | not built; Chief plans are imported without macros |
| 1461 | Ruby Migration dialog (Macro editor, Issue, Revert Changes, Original and New Result) | Out-of-scope | not built |

## 49. Resources and Support (pp. 1462-1466)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1462 | Help menu: Visit Chief Architect Web Site, View Reference Manual, View Tutorial Guide, Online Help Videos, glossary, Knowledge Base, ChiefTalk forum, Sample plans gallery, Webinars, On-site seminars, Personal Online Training, Technical Support | Partial | NO SPEC (APP-143): menus.rs Help: Launch Help, View Tutorial Guide, View Reference Manual, Keyboard Shortcuts, Plan Studio on GitHub, System Information, About; Chief's web resources and paid training do not apply |
| 1463 | ChiefTalk forum, online samples gallery (File > Download Sample Plans), webinars, seminars, personal training, Chief technical support and Support Center, priority telephone support | Out-of-scope | services of another company |
| 1464 | System Information dialog (Tools > System Information) with Copy and More Information buttons | Works | dialogs/app_info.rs System Information (menus.rs) |
| 1464 | File > View File Information: first and most recent saves of a plan or layout | Works | dialogs/app_info.rs (grep File Information) |
| 1465 | Help > Export Logs saves the program logs as a zipped folder for support | Missing | NO SPEC (APP-144): no log export (grep Export Logs finds nothing) |
| 1465 | Error messages: Check Knowledge Base button, Details button, error number and full text; Send Report and Automatically Send Error Reports (Preferences > General) | Partial | crash/error reporting is out of scope for an open-source tool; Plan Studio shows messages in the status bar and dialogs; a log file: not found |
| 1466 | More Information dialog and the project-management audit (Rename, Reset MIME Type, Show in Project Browser) | Missing | Chief Project Management has no counterpart |
| 1466 | Program Paths dialog (list of support files with paths, for Technical Support only) | Out-of-scope | Preferences > Folders lists the data folders (FolderPrefs); no support-only dialog |

## 50. What's New in Chief Architect X18 (pp. 1467-1478)

### 50.1 Migration and legacy content (pp. 1467-1472)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1467 | Getting Started Checklist and the Migrate Settings dialog (preferences, toolbars, hotkeys, library content, templates from X5 to X17 installs) | Out-of-scope | Plan Studio reads Daniel's install at runtime and imports settings (Import Chief Template, Import Chief Plan L-50); no migration wizard |
| 1468 | Legacy library content (X1-X5 .calib via Library > Import Library, custom textures, images, backdrops folders) | Partial | DECISIONS 19: Chief Import Library is a Plan Studio-format command; .calib/.calibz catalogs are read at runtime (plan-calib); custom graphics are read from folders by Preferences > Folders |
| 1469 | Migrating legacy templates (use installed templates or review defaults) | Out-of-scope | guidance for Chief users; Plan Studio imports a Chief template (APP-2) |
| 1469 | Opening plans and layouts of earlier versions: .plan and .layout open (X18 saves an unaltered copy in Archives; .pl and .la no longer supported) | Partial | NO SPEC (L-228): L-50 reads Chief .plan files read-only (plan-chiefplan); Chief .layout files are not read (DECISIONS 36 says templates are not read or written); the Archives copy: DECISIONS 22 (Plan Studio's own Archives) |
| 1470 | Legacy file effects (room labels as real labels, snap and reference grid colours in Preferences, legacy extensions removed, space planning box conversion up to 50, sun angle location in General Plan Defaults, automatic terrain height unchecked with the reference point, soffit/shelf/partition auto fillers removed, GPU samples times 10, dimension label positions, door thickness, PDF rotation on Mac, material emissivity whole numbers, window casing reveal, text indent and margins) | Out-of-scope | apply to opening old Chief files in Chief; Plan Studio's import maps by its own rules; the terrain item shows the reference point model that Plan Studio lacks (see 42.2) |

### 50.2 New and improved features that touch chapters 42-50 (pp. 1472-1478)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1475 | X18 Schedules: Schedule Numbers Format for callouts, 2D Symbol column for Doors, Windows, Cabinets and Rooms, Display Label in This View and Edit Label buttons, fraction format options, rooms on multiple floors, cabinet accessories in schedules, custom .to_s format for fractional inches | Missing | schedules are part of another audit; Fraction Format and the 2D Symbol column are Missing (L-23..L-31 list the schedule set) |
| 1477 | X18 Materials List: Calculate Materials in Room now includes wall finish materials; Materials List Polylines can be listed in schedules | Partial | By Surface tab counts wall faces by material for a room (DECISIONS 131); Materials List Polylines: Missing (45.6) |
| 1477 | X18 Layout: camera views can be sent to layout using Plot Lines regardless of the Rendering Technique | Missing | Plot Lines: Missing (46.6) |
| 1478 | X18 Printing: Match Print Source option, Collate option, Save to Project/Assets for PDF files | Missing | Match Print Source and Collate: Missing (47.7); Save to Project/Assets: n/a |
| 1478 | X18 Ruby: new NVPs for stairs, ramps, wall footings, cabinets, windows, corner boards, molding polylines; global NVPs with full file paths; %label_position_and_orientation% for layout boxes; %light_intensity% | Out-of-scope | Ruby is out of scope |
| 1478 | X18 Resources: System Information dialog | Works | dialogs/app_info.rs |
| 1472 | Other X18 changes listed by chapter (program overview, file management, project planning, preferences, toolbars, window and view tools, displaying objects, positioning, editing, CAD, walls, rooms, dimensions, text, doors and windows, cabinets, electrical, stairs, roofs, framing, trim, library, symbols, materials, 3D views, rendering, pictures, importing and exporting) | Out-of-scope | each belongs to the part of the manual audit for its chapter; terrain, roads and plants have no X18 changes beyond the Terrain Elevation Reference Point (p. 1469) and the Terrain Specification Absolute Elevation panel |

### 50.3 Index (pp. 1479-1497)

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 1479 | Index of terms (pages 1479 to 1497) | Out-of-scope | not a feature; used to cross-check topic coverage only |

## Dialog panels

Chief's panel list for every specification, defaults, assistant and print dialog in pages 1307 to 1466, and what Plan Studio has. "Panels missing" lists Chief panels with no counterpart; "Fields missing" lists fields on panels that exist in part.

| Dialog (page) | Chief panels | Plan Studio panels | Panels missing | Fields missing |
|---|---|---|---|---|
| Terrain Specification (1322) | General, Contours, Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Materials, Label, Object Information, Schedule | General, Contours, Building Pad, Materials, Layer; Cut and Fill report and Import forms (dialogs/terrain.rs) | Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Label, Object Information, Schedule | Absolute Elevation Automatic, Retain Surface Elevation At, Surface at Reference Point or Contour 0, Reference Point X and Y, Skirt (Flat or Follow Terrain, Thickness), Surface Smoothing Low/Medium/High/Linear, Triangle Count (Low/Medium/High/Custom, Triangle Size), Hide Terrain Intersected by Building, contour Offset, contour Smoothing and Passes, Highlight Negative Elevations, Label Units, skirt material |
| Elevation Point Specification (1327) | General, Line Style, Text Style | General only (Elevation, Position X and Y) | Line Style, Text Style | Text, Insert Macro, Marker Radius |
| Elevation Line / Region Specification (1328) | Elevation, Polyline, Spline, Selected Line/Arc, Line Style, Label | General only (Elevation, spline tension) | Polyline, Spline (New Segment Angle), Selected Line/Arc, Line Style, Label | Interior is Flat, Interpolate Tangent to Edge |
| Flat Region Specification (1330) | Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Label, Object Information, Schedule | General (no value) | all of Chief's panels | none besides the panels |
| Hill / Valley Specification (1331) | Hill/Valley, Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Label, Object Information, Schedule | General only (Height) | Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Label, Object Information, Schedule | none besides the panels |
| Raised / Lowered Region Specification (1332) | same as Hill/Valley | General (Height or depth) | same as Hill/Valley | none besides the panels |
| Terrain Feature / Garden Bed / Pond / Stepping Stone Specification (1333) | General, Distributed Plant (beds), Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Materials, Label, Components, Object Information, Schedule | General, Line Style, Fill Style, Layer | Distributed Plant, Polyline, Spline, Selected Line/Arc, Materials, Label, Components, Object Information, Schedule | Terrain to Top, Thickness (ours: one Height above ground), Clip Overlapping Terrain Features; Garden Bed plant, spacing |
| Grass Region Specification (1335) | General (Blades, Appearance, Preview), Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Materials, Label, Components, Object Information, Schedule | General (material, lift), Line Style, Fill Style, Layer | Polyline, Spline, Selected Line/Arc, Materials, Label, Components, Object Information, Schedule | Blade Density, Minimum and Maximum Height, Width and Curve, Colors, Noise Frequency, Roughness, Mow, Cut Height, Mow Line Intensity, Width and Angle, preview |
| Terrain Break Specification (1337) | General, Polyline, Selected Line/Arc, Spline, Line Style, Fill Style | General (Elevation), Line Style, Layer | Polyline, Selected Line/Arc, Spline, Fill Style | Transition Distance |
| Terrain Path Specification (1339) | General (Size, Flare, Preview), Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Materials, Label, Components, Object Information, Schedule | walls and curbs: General, Line Style, Fill Style, Layer; sidewalks and driveways: General, Layer | Polyline, Spline, Selected Line/Arc, Materials, Label, Components, Object Information, Schedule; preview; Line Style and Fill Style on roads | Width on walls, Terrain to Top, Thickness on roads, Flare Start and End with Radius |
| Road Specification (1353) | General, Curb, Polyline, Selected Line/Arc, Spline, Line Style, Fill Style, Materials, Label, Components, Object Information, Schedule | General (Type, Material, Width, Crown, Curb, Curb height), Layer | Curb panel, Polyline, Selected Line/Arc, Spline, Line Style, Fill Style, Materials, Label, Components, Object Information, Schedule; preview | Terrain to Top, Thickness, Flare, Select and Default Curb Profile, Curb Width, Cut Curb for Driveways and Sidewalks |
| Road Marking Specification (1356) | General (stripes), Polyline, Selected Line/Arc, Line Style, Fill Style, Materials, Label, Object Information, Schedule | General, Layer | all but General | none (ours adds color and dashed) |
| Road, Driveway, Road Marking and Sidewalk Defaults (1348) | the specification dialogs reached from Default Settings > Roads, Sidewalks and Driveways or by double-clicking the tool | none (Terrain Defaults only) | all four | all fields |
| Auto Generate Sidewalks (1351) | Left/Right Side of Road, All Connected Roads, Offset From Road | none | whole dialog | all fields |
| Plant Chooser (1359) | Name, Type, Sub-Type, Needs, Flowers and Foliage, Height, Special Characteristics, Object Type, Search, Search Results, View Item | Category list and word search inside the plant dialog | Name (Common, Scientific, Variety, Pronunciation), Type, Sub-Type, Needs (Sun, Water, pH, zone), Flowers and Foliage, Height and age, Special Characteristics, Object Type, View Item | scientific-name toggle, count of matches, jump to Library Browser |
| Hardiness Zones maps (1361) and Grow Plants (1358) | zone map viewer; slider 0 to 20 years | none | whole dialogs | all |
| Plant Image Specification (1362) | Image, Transparency, Plant Information, Plant Description, Layer, Fill Style, Label, Components, Object Information, Schedule | none (plant runs have General and Layer) | all of Chief's panels | image file, 2D symbol, size and aspect, elevation reference, center point, reverse, always faces camera, copyright |
| Plant Specification, 3D plant (1366) and Sprinkler Specification (1367) | symbol object specification dialog; sprinkler line like Polyline Specification | plant run and sprinkler run dialog (General, Layer) | symbol panels, Polyline panels for sprinkler lines | spray area and symbol fill styles |
| Import Terrain Assistant (1342) | Select File, Filter Data, Scale Data | one Import form on the Terrain Specification | the three-step assistant | data organization (six orders), point-number and description columns, X/Y/Z range filter, point reduction, per-axis units, map point to origin, scale, rotate north |
| Import GPS Data Assistant (1344) | Select File, Import Data As, Transform Coordinates | same Import form | the three-step assistant | Import As (Elevation Data, Marker, Polyline, Terrain Perimeter), Lower Elevation Data by, Rotate North, latitude/longitude origin; way-point-only rule |
| Materials List Specification (1372) | Categories, Columns, Options, Appearance | none (window with Category list, Floors check, tabs Materials List, By Surface, Master List) | all four panels | category checklist, column choice and order, Layer Set, Structural Member Reporting, Restrict to Supplier, grid lines, colors, font |
| Master List Specification (1372) | Columns only | Master List tab (waste, stock lengths, prices) | Columns | Category filter, Find, Use, Quantity, Default |
| Materials List Polyline Specification (1383) | Materials List, Polyline, Selected Line/Arc, Line Style, Fill Style, Label | none | whole dialog and defaults | Included Floors/Categories grid, Included Objects choice |
| Materials List Details dialog (1372) | all columns plus Source Object, Find | none | whole dialog | all |
| Export Materials List (1386) | File Type, Options, Units, Third Party Formats | Export CSV, Export Excel, Export PDF buttons | file type TXT, XML, HTML, BuilderTREND; Options; Units | headers, hidden columns, open in editor, colors, units placement |
| Materials List Management and Save Materials List (1385) | list of saved lists and Reports with Edit, Copy, Rename, Delete | none | whole dialogs | all |
| Number Formatting (1381) | units, formatting, accuracy for Count and Extra | none | whole dialog | all |
| Components panel and Object Information panel (1389, 1391) | Components tables with formulas; Code, Comment, Description, Manufacturer, Supplier, Custom Object Fields | door and window Schedule tab; cabinet Components and Object Information; wall Components; Properties tab (custom properties) | panel on stairs, rooms, symbols, roofs, terrain, plants | Code, Insert Macro, Price, Extra, Markup, Labor, Equipment |
| Send to Layout (1398) | Source View, Choose Layout, Send Position, Send Options, Camera View Options, Scaling | Layout file, View, Layout page (Page, Scale, Position) | Send Options, Camera View Options | Entire Plan/View or Current Screen, As Image, Link Saved Plan View, Live View update choice, Plot Lines, Snap to Active CAD Point, Fit to Sheet, Use Layout Line Scaling, Send All Remaining |
| Layout Box Specification (1409) | Linked View, Box Scale, Layer Set, Polyline, Selected Line/Arc, Line Style, Fill Style, Label | General, Source, Line Style | Fill Style, Label panel, Layer Set panel, Polyline, Selected Line/Arc | Relink, Default Set, Show Color, Poché, Reference Display, Plot Lines options, No Scale, Use Layout Line Scaling, Scale Layout Box Contents Only |
| Layout Line Specification (1408) | Line Type, Line Weight, Line Style, Line Color with Use Default | none | whole dialog | all |
| Layout Page Information (1420) | Page Information, Page Template Options, Page Revisions | Page Specification (Page, Sheet) | Page Revisions, Assign Template | Label, Description, Comments, Include in Layout Table |
| Revision Specification (1421) | Revised Pages, Revision Information | Project Information > Revisions rows | Revised Pages | Revised By, Include in Revision Table, per-page revisions |
| Go To Layout Page, Missing Layer Set, Referenced Plan Files (1417, 1415) | page number prompt; replacement layer set; relink list | page picker on the layout toolbar | Missing Layer Set, Referenced Plan Files (not needed for in-plan layouts) | none |
| Drawing Sheet Setup (1428) | Drawing Sheet, Drawing Scale, Printer for View, Drawing Margins, Advanced Line Weights, Preview of Line Weight | Page Setup: Sheet, Page | Drawing Scale, Printer for View, Advanced Line Weights, preview | per-edge margins and Populate from Printer, Line Weight Scale, Use 1 for all line weights, Remember Print Settings |
| Customize Sheet Sizes (1430) | New, Copy, Delete, Edit (description, size, units) | Standard sizes and Custom sizes lists | Copy | units choice; program-wide storage |
| Default Printer for View (1429) | Printer, Orientation, Paper Size, Paper Source | none (Print dialog) | whole dialog | paper source |
| Print View (1440) | Destination, Paper, Print Range, Print Source, Drawing Scale, Options, Advanced Options, Preview, Information, Print/Save | Destination, Paper, Scale, Appearance, Print range, Preview button | Print Source, Information, Advanced Options | DPI for vectors, paper source, Match Print Source, Check Plot, Fit to Paper percentage default, Collate, Include Watermark |
| Print Image (1443) | Destination, Paper, Options, Advanced Options, Preview, Information, Print/Save | Print Image (Width, Scale) and Print Model (Camera, Resolution, Quality, Paper, Destination) | Advanced Options, Preview, Information | DPI, copies |
| Watermark Defaults (1438) | Type, Text, Image, General, Margins, Preview | none | whole dialog | all |
| Text Macro Management and Edit Text Macro (1455, 1457) | list with Valid and values, Edit, New, Copy, Delete, Import, Export, Migrate, Show Evaluation Error; Name, Value, Evaluate, Context, Results | list, New, Edit, Delete; Name, Text | Copy, Import, Export, Migrate, Evaluation Error | Evaluate, Context, Results |
| Ruby Console and Ruby Migration (1458, 1461) | console input/output, tutorial; migration editor | none | whole dialogs (out of scope) | all |
| System Information, File Information, Program Paths, More Information (1464-1466) | summaries with Copy and More Information buttons | System Information and File Information (dialogs/app_info.rs) | Program Paths (support only), More Information (project management) | none |

## Gaps to build

Ranked by how much a residential designer producing construction documents (custom homes, remodels, layout sheets) depends on them. Sizes: S under a day, M a few days, L a week or more. The parity file is where the new ids live.

| Rank | Gap | Size | Parity file | Why it ranks here |
|---|---|---|---|---|
| 1 | Clear Terrain erases the perimeter, data and features instead of only the generated contours (main.rs `C::Clear`); make it clear the built surface and contours only | S | cabinets-stairs-framing-terrain-library.md | the command contradicts the manual and destroys a site model on a mis-click |
| 2 | Page Information on layout pages: Label with a # pattern (A-#), Description, Comments, Include in Layout Table; Chief's page macros %page%, %page.print%, %numpages%, %lastpage%, %layout.label% | M | documentation-layout.md | every CD set needs sheet numbering and an index that follow page order |
| 3 | Per-page revisions with Revised By and Date, Add Layout Revision, and a placeable page-specific Layout Revision Table that can sit on a Page Template | M | documentation-layout.md | revision schedules on permit and bid sets; today revisions are plan-wide rows in the title block |
| 4 | Watermark: View > Watermark, Watermark Defaults (text or image, tile or border, angle, transparency, margins), Include Watermark in the Print dialog | M | documentation-layout.md | DRAFT and NOT FOR CONSTRUCTION marks on review sets |
| 5 | Terrain Specification parity: Absolute Elevation (Automatic, Retain Surface At Reference Point or Contour 0), Terrain Elevation Reference Point with edit buttons, Hide Terrain Intersected by Building, Skirt, smoothing levels and triangle count | M | cabinets-stairs-framing-terrain-library.md | decides how the lot meets Floor 1, needed for daylight and walkout basements and sloped sites |
| 6 | Retaining Wall tools (break plus wall sized from the terrain on both sides), 5 ft default terrain wall that follows the ground, stepping as an option rather than the default | M | cabinets-stairs-framing-terrain-library.md | retaining walls are common on custom-home sites; DECISIONS 108 conflicts with the manual |
| 7 | Print dialog: Print Source (Drawing Sheet or Current View), Check Plot at a fraction, Collate, Scale to Fit and Center Sheet, per-view Drawing Sheet Setup with Drawing Scale and per-edge margins | M | documentation-layout.md | proofing 24x36 sets on 11x17 paper is daily work |
| 8 | Layout Box Specification: Fill Style and Label panels, box labels with macros and callout or marker shapes, Box Scale with No Scale and Use Layout Line Scaling, Pan/Scale, Recenter and Scale to Fit tools, Rescale dialog | M | documentation-layout.md | detail callouts and drawing titles under every view |
| 9 | Contour presentation: primary and secondary layers, Offset, label units, Highlight Negative Elevations, 2D smoothing passes, Terrain Labels layer, labels on terrain objects | M | cabinets-stairs-framing-terrain-library.md | site-plan legibility on the plot plan sheet |
| 10 | Terrain and road schedule categories (Terrain Perimeter, Roads, Driveways, Medians, Road Markings, Terrain Paths, Terrain Features) and a Schedule, Label, Object Information panel on terrain objects | M | cabinets-stairs-framing-terrain-library.md | site quantities (driveway, walks, walls) in the drawing set |
| 11 | Materials List columns and Specification: Floor, Label, Supplier, Manufacturer, Code, Extra, Markup, Labor, Equipment, Total Cost, Comment, Accounting Code; Categories, Columns, Options and Appearance panels; saved lists, Reports, Management | L | documentation-layout.md | cost estimates and builder bids rest on these columns |
| 12 | Materials List export formats and options: TXT, XML spreadsheet, HTML, header and hidden-column options, units placement, BuilderTREND CSV; File > Print of the list | S | documentation-layout.md | handing a take-off to a builder or estimating tool |
| 13 | Materials List scopes: Calculate From Selection, in Room on the room's edit toolbar, Materials List Polyline (with defaults), Calculate Structural Materials for Deck, Details and Find Object in Plan | M | documentation-layout.md | takeoffs for an addition or one room of a remodel |
| 14 | Components and Object Information panels on every object that has them in Chief (Code, Comment, Manufacturer, Supplier, price fields) | M | documentation-layout.md | feeds schedules and the Materials List for fixtures, symbols, stairs, rooms |
| 15 | Import Terrain and GPS assistants: column order, point filtering, range limits, units, scale, rotate north, GPX Import As Marker, Polyline or Terrain Perimeter, drawing-layer to terrain conversion | M | cabinets-stairs-framing-terrain-library.md | survey data is how most lots begin; GPX route and track rules also differ from the manual |
| 16 | Terrain feature shapes and click-once placement: Round Garden Bed, Grass Region, Pond and Stepping Stone, Kidney Pond, Stream, Terrain to Top and Thickness, Clip Overlapping | M | cabinets-stairs-framing-terrain-library.md | planters, ponds, pools set into grade |
| 17 | Road geometry: flare at intersections, Polyline Road, Driveway and Sidewalk shapes, Median, Cul-de-sac, curb profile and cut for driveways and sidewalks, Auto Generate Sidewalk, road defaults dialogs | L | cabinets-stairs-framing-terrain-library.md | driveway aprons and walks; road networks matter less on a lot |
| 18 | Edit Layout Lines and Plot Lines (camera views as vector lines with editable edge and pattern lines) and Layout Line Specification | L | documentation-layout.md | crisp section and elevation sheets for plan reviewers |
| 19 | Default Settings groups for Roads, Sidewalks and Driveways, Watermark, Layout Box, Materials List Polyline | S | preferences-hotkeys-toolbars.md | consistent first drawing of every new object |
| 20 | Plants as Library images with Plant Image Specification, richer Plant Chooser (type, needs, zone, color, height), Hardiness Zones, Grow All Plants, Garden Bed distributed plants, sprinkler lines | L | cabinets-stairs-framing-terrain-library.md | planting plans are a niche for this studio |
| 21 | Text macro import and export between plans, Chief global macro names, Copy and evaluation errors | S | dimensions-text-cad.md | sharing title block and note macros across jobs |
| 22 | Export Logs and a program log; keep Program Paths and Ruby as out of scope | S | preferences-hotkeys-toolbars.md | support only |

## DECISIONS.md rows the manual contradicts

| Row | What the row says | What the manual says |
|---|---|---|
| 35 | Revision clouds feed the title-block REVISIONS table at print time; nothing is stored | Revisions belong to pages (Page Revisions with Label, Date, Revised By, Description, Include in Revision Table) and a Layout Revision Table is page-specific (pp. 1420-1422) |
| 62 | Chief's page border and page text fields are folded into a "no border or title block" switch; each page gets its own sheet size | Layout Page Information has Label, Title, Description, Comments, Include in Layout Table, Page Template Options and Page Revisions; there are no border or text fields, and the sheet size is a layout-wide Drawing Sheet Setup, not per page (pp. 1419-1421, 1393) |
| 63 | Customize Sheet Sizes is per layout file; "verify whether program-wide" | The sheet sizes are stored in a sheetSizes.sheet file in the program Data folder, so they are program-wide (p. 1430) |
| 106 | Terrain Specification tabs are General, Contours, Building Pad, Materials, Layer; bare-ground material stored but unused | Panels are General, Contours, Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Materials (terrain surface and skirt), Label, Object Information, Schedule; the building pad is the Flatten Pad switch (pp. 1323-1327) |
| 107 | Terrain import is a form on the Terrain Specification reading DXF, GPX (way, route and track points as elevation) and XYZ | Import is two assistants under File > Import; only way points carry elevation, route points are ignored, track points become markers, polylines or a perimeter (pp. 1342-1346) |
| 108 | New terrain walls are stepped retaining walls by default | A terrain wall sits on top of and follows the terrain; a retaining wall is a separate tool made of a Terrain Break and a wall whose height comes from the terrain on each side (pp. 1318-1319) |
| 109 | Road Marking is a 4 in stripe that can be dashed | A Road Marking is a rectangular region of a different material; the stripe is the separate Road Stripe tool (p. 1350) |
| 110 | Plants are terrain-owned runs built as cones, round canopies or billboards | Plants are image objects (or 3D symbols) placed one at a time from the Library, with a Plant Image Specification and a Plant Chooser (pp. 1358-1366) |

Smaller tensions that are not contradictions: row 36 (layout templates keep a default per sheet size, Chief assigns a Page Template to each page and Page Zero is the default), row 61 (layouts live in the plan, so the Referenced Plan Files, Relink and Missing Layer Set features do not apply), row 64 (the Print Preview window sits beside Chief's on-screen toggle), and row 131 (Materials List By Surface is an addition; Chief's list counts objects by their center point). No DECISIONS row covers the Clear Terrain behavior; gap 1 should get one if the behavior stays.
