# Chief Architect X18 Reference Manual coverage, part 2: CAD Objects, Walls, Rooms, Dimensions (pages 319-520)

Written 2026-10-08 by the manual audit (part 2). Source: Chief Architect's Reference Manual pages 319 to 520 (chapters 11 CAD Objects, 12 Walls Railings and Fencing, 13 Rooms, 14 Dimensions), read in full from `~/plan-studio-dev/chief-docs`. Every row below is written in my own words; only tool, dialog, panel and field names are Chief's. Nothing from the manual text is stored in the repo.

**How status was found.** For each feature I checked `docs/chief-feature-coverage.md`, the `docs/parity/*.md` rows and `docs/parity-status.md`, then the code (grep of `crates/` on branch `wip/round-14-partial`, read only, with about 25 Round 15 builders still editing). Where the earlier audit and the working tree disagree I trust the tree and say so. Statuses: **Works**, **Partial**, **Missing**, **Differs** (differs by design), **Out-of-scope**, **In progress (Round 15)** (a brief in `~/plan-studio-dev/briefs/r15` covers it; the evidence column gives today's state). "NO SPEC (id)" means no parity row covered the feature before this audit; the id is the row appended to `docs/parity/*.md` and `docs/parity-status.md`.

## Counts

| Measure | Count |
|---|---|
| Features enumerated (table rows) | 395 |
| Works | 83 |
| Partial | 191 |
| Missing | 93 |
| Differs | 4 |
| In progress (Round 15) | 23 |
| Out-of-scope | 1 |
| Rows that had no parity spec (NO SPEC) | 171 rows, folded into 103 new parity rows |
| Dialogs and dialog groups compared in the panel table | 29 |

## 11. CAD Objects (pp. 319-369)

### 11.1 CAD defaults and preferences

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 319 | CAD Defaults dialog, one per floor, section/elevation view, CAD detail and layout (not in 3D views) | Missing | NO SPEC (CAD-107): no CAD Defaults dialog; Active Layers by Tool (dialogs/layer_sets.rs) sets the Current CAD Layer, ViewFlag::ArcCenters the arc centers; the Default Settings cad.* pages (default_pages/cad.rs, 0 bound fields) are stored and not ... |
| 319 | Current CAD Layer is one setting shared by every view | Works | CAD-1, LAY-6; plan-core layers.rs tool_layer("cad") |
| 320 | Current CAD Layer drop-down with Define button inside CAD Defaults | Partial | CAD-1: Current CAD Layer button opens Active Layers by Tool (dialogs/layer_sets.rs); no Define button there |
| 320 | Displayed Line Length Format preview and Define button | Missing | NO SPEC (CAD-108): no CAD length indicators exist; plan-core dimension.rs DimFormat/LengthFormat already holds the same fields for dimensions (DIM-6) and could back it |
| 320-321 | Display Line Angles As: degrees, quadrant bearings, azimuth bearings | Missing | NO SPEC (CAD-109): no bearing parsing or display anywhere in plan-app/plan-core (grep bearing finds only unrelated crates) |
| 321 | Show Arc Centers and Ends (centers double as snap points) | Works | CAD-10; menus.rs ViewFlag::ArcCenters; snap.rs arc center snap (CAD-40) |
| 321-323 | Displayed Line Length dialog fields (units, zeros, separators, inches below N, fractions, accuracy, preview) | Missing | NO SPEC (CAD-108): no CAD length indicators exist; plan-core dimension.rs DimFormat/LengthFormat already holds the same fields for dimensions (DIM-6) and could back it |
| 319-320 | Revision Cloud Defaults dialog; saved defaults and Default Sets for clouds | Missing | NO SPEC (CAD-110): no revision cloud defaults page (default_pages/cad.rs has none); the tool uses a fixed 12 in scallop, tools/cad.rs CLOUD_ARC |
| 319 | Preferences panels CAD, Line Properties, Sun Angle, Behaviors and Snap Properties affect CAD | Partial | part 1 (pp. 123-156); dialogs/preferences/pages.rs CadPrefs (arc centers, end caps, line weights) and Snap Settings exist, Sun Angle page is C-63 |
| 354 | Endcaps: printed length of dash end caps set in Preferences | Partial | dialogs/preferences/pages.rs EndCap (Round, etc.); length setting not verified |

### 11.2 Menu structure and Current CAD Layer

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 323-324 | CAD menu groups: Points, Lines, Arcs, Boxes, Circles, Revision Cloud, Spline, Dimensions, Automatic Dimensions, Text, Current CAD Layer, Auto Detail | Works | menus.rs cad_menu; docs/chief-feature-coverage.md CAD menu rows 1-68 |
| 354 | Most CAD objects land on the Current CAD Layer; revision clouds use their own defaults layer | Partial | CAD-1, DECISIONS 34 (tools/cad.rs draw_layer): every CAD tool including clouds uses the Current CAD Layer; temporary points use their own layer |
| 354 | Current CAD Layer name shown on the status bar and saved in Saved Plan Views and Default Sets | Partial | status bar shows the layer; plan views keep layer sets (LAY-2, LAY-8); Default Sets are not a feature yet (DIM-40 notes only dimension sets) |
| 354 | Drawing Group of a CAD object (default group 21, newest in front) and editing the order | In progress (Round 15) | plan-core drawing_group.rs (Drawing Groups table, Bring to Front/Send to Back/Set Drawing Group), dialogs/drawing_groups.rs; CAD starts at group 80 in our table rather than Chief's 21 (cad2 brief); LAY-36, LAY-74 (part 1) |
| 354 | Object labels for polylines, CAD boxes and revision clouds (blank automatic label, custom text) | Missing | no label field on CAD objects (CadAttrs has none); NO SPEC (CAD-111) |
| 354-355 | Polylines can be listed in schedules | Missing | no CAD polyline schedule category (plan-docs schedule_kinds.rs); NO SPEC (CAD-111) |

### 11.3 Point tools

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 325 | Place Point: temporary CAD point at a click | Partial | CAD-2; CadMode::PlacePoint; points are CAD objects on layer 'CAD, Temporary Points' (selectable, unlike Chief's) |
| 325 | Input Point: New CAD Point dialog (absolute, relative, polar, Next) | Partial | NO SPEC (CAD-112): tools/cad.rs CadMode::InputPoint types X, Tab, Y, Enter (absolute only); no relative or polar form, no Next |
| 325 | Point Marker: permanent marker, cross by default, with Marker Specification (style, label) | Partial | CAD-2, CadMode::PointMarker; marker specification dialog belongs to part 3 (Markers, p. 555); snap.rs cad_marker_points (CAD-40) |
| 325 | Point to Point Dimension creates point markers at its ends; a dimension copied without its objects pastes onto point markers | Missing | tools/dimension.rs PointToPoint ties free points; DIM-38 pastes free ends, no markers created; NO SPEC (DIM-47) |
| 325 | Add Insertion Point: special marker that sets a CAD block's insertion point | Works | CAD-33; CadMode::AddInsertionPoint; plan-core cad.rs CadBlockInfo.insertion |
| 326 | The Current Point (latest created or selected temporary point) as start, rotation center and roof join marker | Missing | NO SPEC (CAD-113): temporary points are ordinary CAD objects on 'CAD, Temporary Points' (tools/cad/edit.rs TEMP_POINT_LAYER) with no current-point state |
| 327 | Delete key removes the Current Point, then earlier points in reverse order | Missing | NO SPEC (CAD-113): temporary points are ordinary CAD objects on 'CAD, Temporary Points' (tools/cad/edit.rs TEMP_POINT_LAYER) with no current-point state |
| 327-328 | Move Point dialog (double-click a point) | Missing | NO SPEC (CAD-114): no Move Point dialog; a temporary point can only be dragged or edited like a CAD object |
| 328 | Delete Temporary Points removes every temporary point (plan view: all floors) | Works | CAD-2; CadMode::DeleteTempPoints (tools/cad.rs); test edit_tests.rs |
| 325 | Temporary point size and color from Preferences (Snap Properties, Colors) | Partial | layer color of 'CAD, Temporary Points' (magenta 200,0,200); no size or color preference reads |

### 11.4 Line tools and the Line Specification dialog

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 328-329 | Draw Line: click-drag one line; polyline forms when Object Snaps and Connect CAD Segments are on | Works | CAD-3, CAD-4; CadMode::Line; Shift+F8 toggles Connect CAD Segments (toolbar.rs) |
| 329 | Continuous drawing by Alt, right mouse or alternate edit mode; Esc or two buttons stop; stops when a closed shape forms unless Behaviors says otherwise | Partial | CAD-68 (part 1); tools/cad.rs: with ViewFlag::ConnectCad on, Line chains from the last end until Esc (test line_chains_when_connect_cad_segments_is_on); no Alt or right-click chaining, no two-button stop, no Behaviors switch for staying in continuous mode |
| 329 | Drawing with a line style picked in the Library Browser | Missing | CAD-82 (part 1); no library line styles |
| 329 | Double-click the Draw Line parent button to open Input Line | Missing | toolbar buttons open tool defaults/flyouts, not Input Line (toolbar.rs); no Chief-style double-click action |
| 329-331 | Input Line / New CAD Line dialog: start point, absolute, relative to start (polar optional), relative to previous line, Next | Partial | CAD-5, CAD-39: click start then type length, Tab, angle, Enter (tools/cad.rs start_typed_line); no dialog, no relative-to-previous-line or absolute forms |
| 330-331 | Typing angles as bearings (N 20d 30' E, S 45' W, azimuth) | Missing | NO SPEC (CAD-109): no bearing parsing or display anywhere in plan-app/plan-core (grep bearing finds only unrelated crates) |
| 331 | Tab while drawing opens Enter Coordinates (distance, angle) | Partial | part 1 (p. 196); editor/typed_input.rs armed text entry (W-16, S-28) rather than a dialog |
| 331 | Line With Arrow: arrowhead at one or both ends, attributes from Arrow Defaults | Works | CAD-6; CadMode::LineArrow; arrow defaults page cad.* is stored only |
| 331 | A Line with Arrow snapped to an object keeps its connection when the object moves | Missing | NO SPEC (CAD-115): dialogs/cad.rs Arrow tab: five styles (None, Open, Filled, Tick, Dot), start and end separately, size; plan-core cad.rs ArrowStyle; no attach, auto position, line style, block arrowheads or fill color |
| 331 | Construction Lines tool for alignment guides | Missing | CAD-62..CAD-66 (part 1); no construction line tool (grep finds none) |
| 331 | Sun Angle (date, time, latitude, longitude for generic sun and shadows) | Partial | C-63; part 5 (p. 1217); no longitude/DST (see coverage audit CAD row 56) |
| 331 | North Pointer sets true north (bearings, shadows, conditioned area) | Works | toolbar.rs site_objects(); plan-terrain site_symbols.rs; north does not yet drive bearings or shadows in plan view |
| 332 | Disconnect Edges tool splits a polyline at an edge | Missing | S-172 (part 1); no tool in tools/cad.rs CadMode |
| 332 | Connect CAD Segments toggle | Works | tools/cad.rs connect flag; toolbar.rs connect_cad; Shift+F8 |
| 332 | Line Style Management dialog | Missing | CAD-83 (part 1); line looks are the fixed LineStyle enum plus per-object dash (CadAttrs.dash) |
| 332-333 | Line panel: Lock Start/End/Center/Length-Angle, length/angle, start and end points | Partial | dialogs/cad.rs General edits points, length and angle; lock modes missing (NO SPEC (CAD-116)) |
| 333 | Visibility: Show Selected Edge (polyline only) | Missing | NO SPEC (CAD-117): no per-edge visibility in plan-core cad.rs CadItem::Polyline |
| 333-334 | Line Style panel: layer with Default and Define, color/style/weight By Layer or own, weight 0 = thinnest | Partial | dialogs/cad.rs Line Style tab: color, weight, dash (CadAttrs); layer pick in Layer tab; no Default check, no Define button, no line-style library |
| 334 | Bumping options: CAD Stops Move, Wall Stops Move | Missing | S-140 (part 1); bump/push exists only for cabinets (DECISIONS 52, tools/cabinet.rs); CAD objects move freely |
| 334 | Drawing Group options (not in camera views) | In progress (Round 15) | dialogs/drawing_groups.rs, plan-core drawing_group.rs (cad2); LAY-36, LAY-74 (part 1) |
| 334 | Display Options: Show Length, Show Angle (arcs also radius), All Angles, Reverse Angle | Missing | NO SPEC (CAD-118): no length or angle indicators on CAD objects; only the status-bar readout while drawing (tools/cad.rs update_readout) |
| 334 | Show Border for picture, metafile and PDF boxes | Partial | dialogs/images.rs IMAGE_TABS (CAD-60); border option not verified; part 5 (p. 1245) owns images |
| 335 | Layout Page selection for objects drawn in layout files | Partial | plan-layout annot.rs holds page annotations; no CAD-style specification dialog with a page field found |
| 335 | Arrow panel | Partial | NO SPEC (CAD-115): dialogs/cad.rs Arrow tab: five styles (None, Open, Filled, Tick, Dot), start and end separately, size; plan-core cad.rs ArrowStyle; no attach, auto position, line style, block arrowheads or fill color |

### 11.5 Arc creation modes and Arc tools

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 335 | Edit > Arc Creation Modes: five modes (Free Form, Center/Radius/End, Start/End/On, Start/Tangent/End, Arc About Center); last mode remembered between sessions | Partial | CAD-7, DECISIONS 76: four modes (Three-Point, Center-Start-End, Start-End-Radius, Tangent) in Edit > Arc Creation Modes and the option strip; Chief's Start/End/On is split in two, its two center modes are one, Free Form is absent; the chosen mode is a thread-local (ARC_MODE), so probably not kept between sessions |
| 335-336 | Free Form arc: drag along the curve; Alt-click for continuous connected arcs | Missing | no drag-along-path arc; CAD-7 notes the mode set; NO SPEC (CAD-119) |
| 336 | Center/Radius/End arc: click center, drag radius, click end | Partial | CadMode::Arc ArcMode::CenterStartEnd (clicks center, start, end rather than drag-the-radius) |
| 336 | Start/End/On arc: drag start to end, then move to set the curvature | Partial | ArcMode::ThreePoint and StartEndRadius (third click sets the bulge); gesture differs |
| 336 | Start/Tangent/End arc: drag the start tangent, then set length and curvature | Partial | ArcMode::StartEndTangent: start, end, then a click for the start tangent (two clicks when continuing the last line or arc); the tangent is not dragged first |
| 336 | Arc About Center: click center first, then drag start to end (or use the Current Point as center) | Partial | ArcMode::CenterStartEnd has the same click order; the Current Point as center is missing (see Current Point row) |
| 337 | Draw Arc, Input Arc and Arc With Arrow tools | Works | CAD-7, CAD-8, CAD-6; CadMode::Arc, InputArc, ArcArrow |
| 337-338 | New CAD Arc dialog: start point, start or chord direction, radius with right/left curve, length by arc angle, arc length or chord length | Partial | CadMode::InputArc typed fields (CAD-8, tools/cad.rs start_typed_arc); no dialog, no chord direction option |
| 338 | Alt-drag an end handle bends a line into an arc and back | Works | CAD-22 and DECISIONS 75: Change Line/Arc edit tool and diamond bulge handle on polylines (sampled arcs) |
| 339-341 | Arc Specification dialog: locks, center, radius, start/end/arc angle, arc length, facet angle, start and end points, directions, chord length and angle | Partial | dialogs/cad.rs: center, radius, start and end angles; no locks, directions, chord fields or facet angle |
| 341 | Arc and Line Style panel, Arrow panel | Partial | dialogs/cad.rs Line Style and Arrow tabs for arcs |

### 11.6 Circles, ovals, ellipses

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 341 | Circle by diameter drag; Circle About Center by center and radius drag | Works | CAD-11; CadMode::Circle, CircleAboutCenter |
| 342 | Oval (four-arc approximation) and Ellipse drawn by an angled drag | Partial | CAD-11; both stored as closed polylines, so the oval is not a four-arc shape; NO SPEC (CAD-120) |
| 342-343 | Circle/Oval/Ellipse Specification: center, angle, length and width or diameter, radius, circumference; Line Style and Fill Style | Partial | dialogs/cad.rs circle: center and radius, Line Style, Fill Style; NO SPEC (CAD-120) |

### 11.7 Polylines and splines

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 343 | Polyline from lines/arcs connected end to end on the same layer with identical attributes; closed polylines take fills and convert to slabs, countertops and more | Works | CAD-4, CAD-15, CAD-26, CAD-28 (Partial: no roof plane/room/solid conversion); CadMode::Polyline |
| 343 | Rectangular Polyline: drag corners, or one click for a 24 in square (600 mm) | Partial | CAD-12; CadMode::RectPolyline drag; single-click default square not verified |
| 343-344 | Polyline handles reveal polyline vs line; Close Polyline edit tool | Partial | CAD-15, CAD-16 vertex handles; CAD-90 (part 1): closing is a Specification check box (dialogs/cad.rs), no Close Polyline edit button |
| 344 | Same Line Type edit handles on line ends (add a new segment inheriting color, weight, style, layer, arrow); can be switched off in Preferences | Missing | NO SPEC (CAD-121): editor/handles.rs CAD handles are move, rotate, end and vertex handles only |
| 344 | Polyline Holes: closed CAD polylines can contain holes | Missing | CAD-91 (part 1); CadItem::Polyline has no hole list; Boolean subtract is in the Round 15 CAD brief (clip.rs) |
| 344-347 | Polyline Specification panels (Polyline, Spline, Selected Line/Arc, Line Style, Fill Style, Arrow, Label, Components, Object Information, Schedule) | Partial | NO SPEC (CAD-111): dialogs/cad.rs shows Perimeter and Area (CAD-26) and vertex list; no hole count, volume, spline segment angle, selected-edge panel, label, components, object information or schedule |
| 352-353 | Spline tool: draw segments, curve forms from the second segment, close by joining ends | Partial | CAD-29, CAD-30; NO SPEC (CAD-122) |
| 353 | Convert open or closed polyline to spline (Convert to Spline edit tool) | Works | CAD-23; CadMode::ConvertToSpline |
| 355 | Hide Selected Edge / Show Selected Edge edit tools | Missing | NO SPEC (CAD-117): no per-edge visibility in plan-core cad.rs CadItem::Polyline |
| 355 | Fill Styles on closed CAD shapes; fill colors by layer | Works | CAD-13, CAD-69 (part 1); dialogs/cad.rs Fill Style tab (solid, hatch patterns, FillAttr); layer fill colors not verified |

### 11.8 Box tools, Regular Polygon and Revision Cloud

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 347 | Regular Polygon: side length or radius to corner/side from the New Regular Polygon dialog | Partial | NO SPEC (CAD-123): CadMode::Polygon is center click plus vertex click with the sides count in the option strip (tools/cad.rs regular_polygon); no side-length or radius-to-side forms, no dialog |
| 347 | Box: 2D rectangle with four 90 degree corners, resizable but stays rectangular | Partial | CAD-12; CadMode::Box click an edge and depth, stored as a polyline (can be reshaped into any shape) |
| 348 | Cross Box (X in the box) for joists and rafters cut by the clip plane; Blocking Box (diagonal); Insulation (curves only, no perimeter) | Works | CAD-12; CadMode::CrossBox, BlockingBox, Insulation (plan-core cad.rs cross_box_items, blocking_box_items, insulation_items) |
| 348-349 | Box Specification dialog (Box Style, position, angle, size, Line Style, Fill Style) | Partial | NO SPEC (CAD-124): boxes are closed polylines; the Cross, Blocking and Insulation tools draw groups of lines (cad.rs cross_box_items, insulation_items) so they have no Box Style switch or size fields; dialogs/cad.rs lists vertices |
| 349 | Revision Cloud tool: drag a rectangle with arc edges | Works | CAD-36; CadMode::RevisionCloud; revision clouds feed the title block table (DECISIONS 35) |
| 349-350 | Cloud shortcuts: click object, click empty space, Alternate edges, Around Objects, Convert Polyline | Partial | NO SPEC (CAD-125): tools/cad.rs RevisionCloud: drag a rectangle or click 3+ points; no click-on-object, no Around Objects edit button, no convert |
| 350-352 | Revision Cloud Specification (Cloud, Selected Line/Arc, Line Style, Fill Style, Label) | Missing | NO SPEC (CAD-126): CadMode::RevisionCloud builds a fixed-size scalloped closed polyline (tools/cad.rs revision_cloud, CLOUD_ARC); after drawing it is a plain polyline with no cloud parameters, no regenerate |

### 11.9 Arrowheads

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 355-356 | Arrow style list and library, Include Arrow, custom line style, arrow fill color, size, both ends | Partial | NO SPEC (CAD-115): dialogs/cad.rs Arrow tab: five styles (None, Open, Filled, Tick, Dot), start and end separately, size; plan-core cad.rs ArrowStyle; no attach, auto position, line style, block arrowheads or fill color |
| 356-357 | Attach Tail/Head to other objects and Auto Position Tail/Head | Missing | NO SPEC (CAD-115): dialogs/cad.rs Arrow tab: five styles (None, Open, Filled, Tick, Dot), start and end separately, size; plan-core cad.rs ArrowStyle; no attach, auto position, line style, block arrowheads or fill color |
| 357 | Custom arrowheads drawn as CAD blocks with a Backoff Point; insertion point pivots the shape | Partial | NO SPEC (CAD-127): Add Arrow Backoff Point tool and trimmed lines exist (CAD-6, CadMode::AddBackoffPoint); the Arrow tab cannot pick a block as the arrowhead |

### 11.10 CAD blocks

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 357-358 | Make CAD Block from CAD objects, text, dimensions, nested blocks and CAD-based objects (slabs, framing, custom countertops); terrain objects excluded | Partial | CAD-31; CadMode::MakeBlock groups CAD objects; dimensions, nested blocks and CAD-based architectural objects not included |
| 358 | Explode CAD Block (nested blocks stay blocks; resized blocks cannot explode) | Partial | CAD-32; CadMode::ExplodeBlock; non-uniform resize does not exist |
| 358-359 | Edit CAD Block in its own window; update all instances or only this one | Partial | NO SPEC (CAD-128): Edit CAD Block edits name and points in place (CAD-32); no separate window, no choice between all and this instance; nesting unverified |
| 359-360 | Insertion Points: default center, Move handle at the insertion point, rotation about it, Select Insertion Point edit button, X/Y offset in dialog | Partial | CAD-33; Add Insertion Point tool; handle and dialog offset (BLOCK_TABS Insertion Point); no Select Insertion Point button |
| 360 | Add CAD block to the Library; Import Drawing blocks from DXF/DWG | Partial | CAD-31 Gap: block not in Library Browser; dialogs/exchange.rs DXF import brings blocks (L-44) |
| 360 | Layers and Saved Defaults/Text Styles in a library block are created in the destination file | Missing | no block-in-library path; blocks keep object layers (CAD-37 Partial); NO SPEC (CAD-129) |
| 360-361 | Custom 2D plan symbols: choose a CAD block for a symbol or image; names unique per file | Partial | dialogs/symbol.rs 2D block choice not verified; part 3 (p. 1034) owns the symbol panel |
| 361-362 | CAD Block Management dialog | Partial | NO SPEC (CAD-130): dialogs/cad/blocks.rs lists, inserts, edits, renames and deletes blocks (CAD-34); no Used marks, no Purge or auto-purge, no Add to Library from the dialog (blocks are not in the Library Browser, CAD-31), no preview |
| 362-364 | CAD Block Specification dialog | Partial | NO SPEC (CAD-131): dialogs/cad/blocks.rs BLOCK_TABS General, Insertion Point, Arrow Backoff (name, insertion and backoff points); blocks are groups: no instance size or factors, no By Block/By Object switch, no copyright, no label panel |
| 364 | Block copy to another file carries its definition | Works | blocks are groups with tagged info inside the plan (cad.rs CadBlockInfo); copy and paste of the group copies it |

### 11.11 CAD details, plot plans and Plan Footprint

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 365 | CAD Details: special views for site plans and section details, saved in the file, listed in Project Browser, usable on layout pages | Works | L-39..L-41 and DECISIONS 100, 101, 102 (details are floors); dialogs/details/management.rs |
| 365 | CAD Detail Management: New, Rename, Duplicate, Delete, Open (check mark when open) | Works | coverage audit CAD row 60; dialogs/details/management.rs |
| 366 | Automatic Truss Detail and Wall Details; temporary Pattern, Molding Profile, Skylight and CAD Block windows | Partial | NO SPEC (CAD-132): no Truss Detail view; no Edit Pattern, Edit Molding Profile or Edit Skylight Shape windows (grep finds none); Wall Details exist (plan-framing detail.rs, CAD-35) |
| 366 | CAD Detail from View converts the current line view (plan, Vector view) into editable CAD, inheriting layer set, dimension defaults, rotation and text | Works | CAD-35; tools::details::detail_from_view; text styles become custom styles not verified |
| 367 | CAD Detail Specification (General, Selected Defaults) | Partial | NO SPEC (CAD-133): CAD details are floors with CadDetailInfo (DECISIONS 100, dialogs/details/management.rs: new, rename, duplicate, delete, open, send to layout); no per-detail dialog for remember-zoom, color, watermark or defaults |
| 367 | Plot plan from CAD tools sent to layout; convertible to a Terrain Perimeter | Partial | NO SPEC (CAD-134): Input Line takes length and angle by typing (CAD-5, CAD-39); no bearings, no relative-to-previous-line form; a terrain perimeter can be drawn from points (CB-44) |
| 367-368 | CAD > Plan Footprint: new CAD detail with the floor's outline; footprints per building; insert into a detail | Partial | NO SPEC (CAD-135): Tools > Checks > Plan Footprint (Action::PlanFootprint, tools/cad_ops.rs) makes a CAD polyline and area from room boundaries (L-38); no footprint object, no CAD Detail insert, no specification dialog |
| 368 | Plan Footprint layer set and Layer Display Options control what shows | Missing | no footprint view or layer set; NO SPEC (CAD-135) |
| 368-369 | Plan Footprint Specification | Partial | NO SPEC (CAD-135): Tools > Checks > Plan Footprint (Action::PlanFootprint, tools/cad_ops.rs) makes a CAD polyline and area from room boundaries (L-38); no footprint object, no CAD Detail insert, no specification dialog |

## 12. Walls, Railings, and Fencing (pp. 370-442)

### 12.1 Wall, railing and fencing defaults

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 370 | Default Settings > Walls: a Defaults dialog per wall tool, shift/ctrl multi-select, reached also by double-clicking a tool button | Partial | NO SPEC (W-119): Exterior, Interior and Foundation open the real Wall Specification (dialogs/defaults.rs DefaultsEntry); Railing, Fence, Pony, Half, Glass and Attic are pages with a wall type and a height (default_pages/architectural.rs ... |
| 370-371 | General Wall Defaults: Resize About, Auto Rebuild Attic Walls, Auto Reverse Wall Layers, Auto Merge Collinear Walls | Missing | NO SPEC (W-120): no dialog; ResizeAbout::default_for(kind) is fixed (plan-core walls.rs: exterior main layer outside, interior wall center) and each wall carries its own value; Auto Rebuild Attic Walls is a Preferences > Architectural switch ... |
| 371 | Exterior and Interior Wall Defaults dialogs (Wall Specification panels) | Works | dialogs/defaults.rs DefaultsEntry::ExteriorWall, InteriorWall (title 'Wall Specification (Exterior Wall Defaults)'); W-1, W-51 |
| 371-372 | Foundation Wall Defaults (also drives Retaining Wall; footing size for stem wall or pier foundations) | Partial | DefaultsEntry::FoundationWall; Default Settings Foundation page; Retaining Wall type from this dialog not verified; R-82 foundation defaults |
| 372 | Slab Footing Defaults (monolithic slab foundation and garage curbs) | Missing | dialogs/defaults.rs DefaultsEntry has Exterior, Interior and Foundation wall leaves only; the Slab tool dialogs (dialogs/foundation.rs) carry their own values (R-75, R-76); NO SPEC (W-119) |
| 372 | Pony Wall Defaults (Display in Plan View saved per Saved Plan View; openings display option) | Partial | default_pages/architectural.rs 'pony' page: upper type, lower type, lower wall top elevation; no plan display options |
| 372 | Railing and Deck Railing Defaults (wall types Interior Railing and Deck Railing/Fence) | Partial | default_pages 'railing' page: wall type and height; no Deck Railing leaf |
| 372 | Glass Wall and Glass Pony Wall Defaults | Partial | 'glass' page: wall type only; no Glass Pony defaults |
| 373 | Half-Wall Defaults (36 in solid railing with handrail) | Partial | 'half' page: top height; W-54 |
| 373 | Fencing Defaults and Room Divider Defaults | Partial | 'fence' page: wall type and height; no Room Divider defaults page |
| 373 | Wall Hatching Defaults | Missing | no Wall Hatching leaf in the Default Settings tree (dialogs/defaults.rs TREE); a new hatch starts from the constants in plan-core details.rs; NO SPEC (W-119) |
| 373 | Set as Default edit button picks the matching defaults by the wall's role | Missing | NO SPEC (W-121): dialogs/exchange.rs set_as_default_template only makes a Chief template the default; no Set as Default edit button for walls |

### 12.2 Wall tools

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 374 | Exterior Wall and Curved Exterior Wall tools | Works | W-1, W-64; tools/wall.rs WallVariant |
| 374 | Interior Wall and Curved Interior Wall tools | Works | W-1, W-64 |
| 374 | Foundation Wall and Curved Foundation Wall tools (any floor) | Works | W-52, W-64; WallClass::Foundation |
| 374 | Slab Footing tool (single concrete layer; rooms get ceiling height 0 and a monolithic slab) | Partial | W-58 Works for the tool (tools/details.rs SlabFooting); the rule that rooms bounded by slab footings get ceiling height 0 and a monolithic slab is not verified (room Monolithic Slab flag R-31 is set by hand) |
| 374 | Pony Wall and Curved Pony Wall tools | Works | W-53, W-64 |
| 374 | Straight Glass Wall; glass walls do not cut floor, ceiling or adjacent wall surfaces | Partial | W-57 Works; the 'does not cut' behaviour needs the Partition Wall flag (NO SPEC (W-122)) |
| 374 | Straight Glass Pony Wall | Works | W-57; WallClass::GlassPony |
| 374-375 | Half-Wall and Curved Half-Wall (solid railing, 36 in with handrail) | Works | W-54, W-64; WallClass::HalfWall |
| 375 | Room Divider (invisible, zero thickness, Air Gap, displays in plan not 3D) | Works | W-55; WallClass::RoomDivider; layer Walls, Invisible |
| 375 | Polygon Shaped Room tool | Missing | NO SPEC (W-123): no tool; Polygon Shaped Deck exists (W-107) as a clicked polygon |
| 375 | Wall Hatching tool (click a wall; resize to part) | Partial | NO SPEC (W-124): W-59: DetailsVariant::WallHatching (tools/details.rs), WallHatch has wall_id, pattern, scale, angle, layer, style (plan-core details.rs); one hatch covers the whole wall; dialog tabs General, Line Style, Layer (no Fill Style or ... |
| 375 | Wall Material Region (Build > Wall > Wall Material Region) | Works | W-59; DetailsVariant::WallMaterialRegion |
| 375 | Fix Wall Connections tool | Works | W-41; Action FixWallConnections (editor/connect.rs) |
| 375 | Break Wall (legacy tool, Ctrl+B) and Add Break | Works | W-43, W-44; wall_edit.rs break_wall_at |
| 375 | Define Wall Types tool | Works | W-46; dialogs/wall_types.rs |
| 375 | Library walls: draw a wall saved in the Library Browser; type name collisions get a _2 suffix | Missing | NO SPEC (W-125): Wall Types Library button is disabled (W-47); no wall items in the Library Browser |
| 375 | Curved variants of each straight wall tool | Works | W-64, W-65 (third click or Change Line/Arc) |

### 12.3 Railing, deck and fencing tools

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 376 | Railing and Curved Railing (defines interior spaces, set heights, stairwells) | Works | W-56; WallClass::Railing |
| 376 | Straight/Curved Deck Railing: deck room with framing and supports when a foundation level exists | In progress (Round 15) | W-56; deck rooms CB-86 (DECISIONS 205, 206); deck framing under Build Framing > Deck |
| 376 | Straight/Curved Deck Edge (deck framing without a railing, no supports) | Works | W-106; WallStyle::DeckEdge |
| 377 | Polygon Shaped Deck | Partial | NO SPEC (W-126): W-107: DetailsVariant::PolygonDeck draws a clicked polygon; no dialog, no regular polygon sizes, no railing switch |
| 377 | Railing types: Baluster, Solid (Half Wall), Panels from the library | In progress (Round 15) | NO SPEC (W-127) |
| 377 | Railing wall type 'Interior Railing' (three layers) / 'Deck Railing/Fence'; wall type sets plan look, solid thickness, floor platform edge | Works | wall types Railing-4 and Fence-Wood-2 in the template defaults (default_pages special_wall); W-56 |
| 377 | Railing newels and balusters hidden in plan by default, switchable | In progress (Round 15) | plan display options of the Newels/Balusters tab (wall_spec_tabs.rs); not drawn in plan today |
| 378 | Glass panel above a half wall recipe: half wall + Pony Wall + Wall Cap full width + Panels rail style | Missing | needs the Panels rail style and a railing that can also be a pony wall (WallClass is exclusive) |
| 378 | Fencing and Curved Fencing: follow the terrain, no room definition | Partial | W-108 Works for the tool and 3D wall_kinds; Step Terrain / Follow Terrain choices live in the Rail Style tab (in progress, NO SPEC (W-127)) |

### 12.4 Exterior, interior, foundation and pony walls; invisible walls; polygons

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 378-379 | Exterior/interior determined by position in the model, not by tool; thermal envelope separates conditioned from unconditioned | Differs | NO SPEC (W-128): walls carry the kind of the tool that drew them (WallKind, W-22); dimensions treat exterior vs interior by wall.kind (DIM-4); the primary/secondary side distinction for symmetrical interior walls is not modeled |
| 379-380 | Foundation wall footings: centered, on main layer, on outside or offset; footing material; select footing in 3D and edit handles; footing layer and fill | Partial | W-52 Foundation tab: Footing Offset, Center Footing on Main Layer, Align Footing on Outside, Vertical Footing, Automatic Footing Bottom Height; footing fill/layer dimmed; no 3D footing handles |
| 379 | Slab Footing used for garage curbs and interior footings with wider footing | Partial | W-58; Curb Width field not found (Curb Width appears only in plan-terrain) |
| 380 | Brick ledges (needs concrete wall under masonry layer and Brick Ledge Depth) | Missing | NO SPEC (W-129): no brick ledge in plan-3d wall.rs or floors.rs build_foundation (grep brick_ledge finds none) |
| 381-382 | Pony wall: two wall types, adjustable division height, linked parts, exterior/interior pony walls | Works | W-53; dialogs/wall.rs Pony Wall, Lower Wall Type, Elevation of Lower Wall Top |
| 381 | Pony Wall Display in Plan View (upper, lower, outlines, both) and openings display option | Partial | WallExtras.display_in_plan is a single check box (dialogs/wall.rs); six-way choice and opening option missing |
| 382 | Lower pony wall cap (water table), splits between the parts or full width | Partial | Wall Cap tab (DECISIONS 50, W-114): Wall Cap, Full Wall Width and Split Pony Wall check boxes with a three-profile table; outward water-table default and a profile library are not built |
| 382 | Make Wall(s) Invisible / Make Wall(s) Visible edit buttons | Missing | NO SPEC (W-130): Invisible is a check box in the Wall Specification (W-24); no edit-toolbar buttons (wall_edit.rs offers Reverse Layers, Convert to Polyline, Break Wall, Remove Break, Change Line/Arc, Make Arc Tangent) |
| 382-383 | Invisible walls: plan yes, 3D and Materials List no; no doors or windows placed; floor, wall and ceiling areas computed per room; ignored by Auto Place Outlets; cabinets pass through | Partial | W-55, W-93, R-3; opening placement refusal and Auto Place Outlets skip unverified |
| 382 | Zero-thickness room divider between different floor/ceiling heights becomes the default interior wall | Missing | no such rule found in plan-core walls.rs or rooms.rs; W-55 only covers the divider closing rooms |
| 383-384 | Polygon Shaped Room / Deck dialog (side length, radius to corner, radius to side, number of sides, Include Railing) | Partial | NO SPEC (W-126): W-107: DetailsVariant::PolygonDeck draws a clicked polygon; no dialog, no regular polygon sizes, no railing switch |

### 12.5 Wall Hatching

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 384 | Apply fill pattern to a wall segment; breaks need one hatch per side; not on invisible or locked-layer walls | Partial | NO SPEC (W-124): W-59: DetailsVariant::WallHatching (tools/details.rs), WallHatch has wall_id, pattern, scale, angle, layer, style (plan-core details.rs); one hatch covers the whole wall; dialog tabs General, Line Style, Layer (no Fill Style or ... |
| 384-385 | Hatching layer Walls, Hatching and Drawing Group; covers all displayed wall layers; main layer only on railings | Partial | W-59 layer on Layer tab (WallHatch.layer); drawing group per Round 15 (drawing_group.rs); no main-layer-only mode; NO SPEC (W-124) |
| 385 | Wall Hatching Specification (Layer, Fill Style, Label) | Partial | dialogs/details.rs HATCH_TABS General, Line Style, Layer; no Fill Style/Label tab; NO SPEC (W-124) |

### 12.6 Creating and connecting walls

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 385 | Click-drag walls like CAD lines; draw in plan, camera views and overviews | Partial | W-3, W-4; walls draw in plan only (wall tool in 3D views not found) |
| 385 | Other ways: auto exterior walls for a new floor, foundation walls, attic walls, Space Planning, CAD to Walls | Works | R-59 build_new_floor_with, R-61, RF-31, R-79, W-89 |
| 385-386 | Drawing tips: exterior first, grid snaps for the shell, extension snaps and 'sticky' points, angle snaps over object snaps (switchable) | Partial | W-11, W-14 (Partial: no dashed guide lines), W-17, W-18; 'Always Snap Walls On Allowed Angles' preference not found; walls2 builder owns guides |
| 385 | Exterior walls are oriented so the siding side faces outward when walls enclose a room (switchable) | Missing | NO SPEC (W-131): W-22: the tool chosen sets the side, nothing auto-detects interior vs exterior; Reverse Layers button exists (W-23) |
| 386 | Initial wall height from the floor's default heights | Works | W-6, W-60; Floor Defaults |
| 386 | Wall angles: draw with angle snaps; off-angle wall icon and Caution symbol; Fix Off Angle Wall, Ignore, Ignore All; Allowed Angles | Missing | NO SPEC (W-132): no caution symbols in editor/render.rs (grep off angle, Ignore Unconnected, Reset Notification finds none); Fix Wall Connections runs as a command (W-41) |
| 386 | Fix Off Angle Wall dialog (old angle, new angle, lock start/center/end) | Missing | NO SPEC (W-133): angle snap increments exist (Snap Settings, W-18) but there is no allowed-angles list and no fix dialog |
| 386-387 | Temporary wall readout (length and angle in the status bar) while drawing; dimensions position walls afterwards | Works | W-15, W-16; tempdim.rs; DIM-35 |
| 387 | Entering wall lengths and angles (Spec dialog, Tab to Enter Coordinates) with Resize About outer surface | Works | W-16, W-75; editor/typed_input.rs |
| 387 | Openings via door and window objects, not gaps in the walls | Works | W-84; openings hosted by walls |
| 387 | Temporary dimensions along a wall as it is drawn | Works | W-74; ViewFlag::TemporaryDimensions |
| 387 | Continuous wall drawing by right-click, Alt-click or Alt-drag; stops when a room closes | Partial | W-3 Partial: right-click ends the chain; Alt chain not found; W-5 closes on the start point |
| 387-388 | Curved walls follow the Arc Creation Mode; Change Line/Arc; center shown with Show Arc Centers; room as a circle needs two curved walls | Partial | W-64, W-67, CAD-7: wall draw uses a third click through-point only, not the five arc modes |
| 388 | Space Planning Assistant room boxes convert to walls | Works | R-79; plan-spaceplan |
| 388 | Wall framing built per wall (Build Framing for Selected Object) unless Retain Wall Framing | Partial | CB-36 framing builder (Build Framing dialog, Round 14); per-object build button and Retain Wall Framing are dimmed (NO SPEC (W-134)) |
| 388 | Wall snapping when centerlines are within the larger wall width, even with Object Snaps off; moving wall adjusts, other wall keeps position | Works | W-12, W-40, W-103; editor/connect.rs auto_connect |
| 388-389 | Auto Connect icon: lock a wall end from snapping; Enable Auto Connect; Reset to Defaults clears all | Missing | NO SPEC (W-135): tools/wall.rs and editor/connect.rs auto_connect run for every end; no per-end lock |
| 389 | Wall intersections: one wall builds through, into the intersected wall's main layer; Walls, Through Wall Lines layer shows which | Partial | W-34, W-37, W-45 Works for builds-through; the Through Wall Lines layer is not provided (NO SPEC (LAY-76)) |
| 389 | Acute junctions (< 60 degrees): the through wall terminates square, adding a third edge | Partial | W-32 miters with a limit (joins.rs MITER_LIMIT); the square-termination rule below 60 degrees is not verified |
| 389 | Partition Wall type: stops at floor, ceiling and wall surfaces instead of building through the finish layers (glass shower walls) | Missing | NO SPEC (W-122): wall types have no partition flag (dialogs/wall_types.rs); Glass wall classes build as normal walls |
| 390 | Fix Wall Connections: caution symbol menu (Delete, Ignore Unconnected Wall), Connect Walls edit tool | Partial | W-41 Works as a command; symbol and per-wall tool missing; NO SPEC (W-136) |
| 390 | Removing wall breaks by dragging the break end handle a few inches; Auto Merge Collinear Walls switch (off for panelized walls) | Partial | W-45 Remove Break button (join_collinear_walls); no automatic merge on drag, no switch; NO SPEC (W-120) |

### 12.7 Displaying walls

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 390-391 | Wall display layers: Walls Layers / Main Layer Only / Through Wall Lines / Footings / Brick Ledge Lines / hidden layers hide the openings too | Partial | NO SPEC (LAY-76): plan-core layers.rs default set has 'Walls, Normal' (both wall tools) and 'Walls, Invisible' only, and the wall dialog names 'Walls, Labels'; no layer-lines on/off or main-layer-only display, no through-wall lines, footing or ... |
| 391 | Plan appearance (line weights, colors, layer fills) comes from the Wall Type Definition; separate footing fill | Partial | W-50, W-94: plan fills come from wall kind and class only (editor/render.rs fill_wall); no per-layer fill patterns; NO SPEC (W-137) |
| 391 | Reference floor shows another floor's walls | Works | R-65, LAY-9 |
| 391-392 | Notification icons for off-angle and connection problems; Reset Notification Icons | Missing | NO SPEC (W-132): no caution symbols in editor/render.rs (grep off angle, Ignore Unconnected, Reset Notification finds none); Fix Wall Connections runs as a command (W-41) |
| 392 | 3D display of wall layers on custom layers; framing layer shown solid until framing is built; Delete Surface tool | Partial | plan-3d wall.rs layer meshes; Delete Surface tool exists (part 5); per-assembly-layer display layer missing |
| 392 | Hide Camera-Facing Exterior Walls (and attic walls) in exterior camera views | Missing | no such command in menus.rs (part 5, p. 1205 owns the row); the wall dialog has the Ignored by Hide Exterior Walls check box |
| 392 | Cross sections: Auto Detail uses each wall layer's fill style and display layer | Partial | CAD-35 Auto Detail (tools/details/cad_detail.rs); layer fill styles are fixed by wall layer material, not a fill column; NO SPEC (W-138) |
| 392 | Poche fill for walls in plan, sections, floor overviews and the cross section slider | Partial | CAD-78, LAY-55 (part 1); plan-layout hatch.rs poche for sections and elevations; plan view poche option not found |
| 392-393 | Wall assembly layers on custom display layers (not Walls, Layers/Main Layer Only) | Missing | NO SPEC (W-138): WallLayer is name, thickness, is_main, material (plan-core defaults.rs); walls frame from Framing Defaults (CB-36) rather than per layer |
| 393 | Wall labels: Walls, Labels layer, shown in plan, sections and cameras; automatic labels for framed walls from Wall Details; custom labels, named values | Partial | W-81: Label tab with specified text and suppress switch; text style/alignment/layer controls dimmed; no automatic framed-wall labels |
| 393 | Wall materials: Use Default falls back to the room's material, then the general wall material; Material Painter | Partial | C-60, C-61, DECISIONS 126-129; Wall Materials tab in progress (walls2) |
| 393-394 | Materials List categories for walls: Siding, Framing, Insulation, Wallboard, Interior Trim, Foundation, General; openings netted; pour numbers | Partial | DECISIONS 67 (wall components in 4x8 sheets, net of openings); L-35; insulation and foundation categories not verified |
| 394 | Wall Schedule: all wall types by default, per-wall Include in Schedule, invisible walls included on request | In progress (Round 15) | Wall schedule pages (default_pages 'schedules.wall', Action::WallSchedule); per-wall Schedule tab and ScheduleKind::Wall are in walls2 |

### 12.8 Measuring walls and dimension interplay

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 395 | Check dimension defaults before moving walls by dimensions; wall length indicator follows Resize About, not the dimension locate | Works | W-26, W-74, W-75; DIM-4 |
| 395 | Dimension defaults can locate wall steps, brick ledge lines, newel centers and sides, invisible walls at centerline, displayed parts of pony walls | Partial | DIM-4, DIM-40 Locate Objects; wall steps, ledge lines and newels are not located (they do not exist) |
| 395-396 | Primary and Secondary side of a wall for dimension location (exterior side primary; symmetrical interior walls left/bottom) | Differs | NO SPEC (W-128): walls carry the kind of the tool that drew them (WallKind, W-22); dimensions treat exterior vs interior by wall.kind (DIM-4); the primary/secondary side distinction for symmetrical interior walls is not modeled |
| 396 | Extension lines can be edited to locate surface, dimension layer, centerline, main layer center or inside, interior surface; foundation footings only by edit | Partial | DIM-7, DIM-41: add/delete extension line only; moving an extension to another layer line is not supported |
| 396-397 | Move walls by editing a dimension value with end buttons (Move Left/Right/Top/Bottom End, Both Ends, Along Rails); slab footing thickness by dimension | Missing | NO SPEC (DIM-48): tools/dimension.rs finish_edit moves the object at the nearer end (DECISIONS 83); editor/tempdim.rs has no end buttons |
| 397 | No Locate: skipped by Auto Exterior Dimensions with its openings; railings, deck railings and dividers default to No Locate; moves to Walls, No Locate layer | Partial | W-24, DIM-5 skip No Locate; defaults for railings/dividers and the layer move are not verified |

### 12.9 Editing walls

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 397 | Selecting walls in all views; Select Room Before Wall in 3D preference; top surface selects in camera views; Edit Area; Find Wall from framing | Partial | S-34 Select Next Object (Tab); Select Room Before Wall and Find Wall not found; 3D wall pick exists (C-43) |
| 398 | Wall Hatching may be selected before the wall; Select Next Object | Partial | S-34 Tab cycling covers hatch objects |
| 397-398 | Edit handles in any view; Specification dialog; Edit toolbar; Eyedropper / Match Properties copy a wall's attributes | Partial | S-116 Match Properties/Object Eyedropper (Round 15 painters); wall edit handles see 12.10 |
| 398 | Reverse Layers edit button and Ignore Reversed Wall caution | Works | W-23, DECISIONS 11; caution symbol missing (NO SPEC (W-132)) |
| 398-399 | Wall thickness: changing it creates a numbered copy of the type whose innermost main layer absorbs the change; main layer at least 1/16 in; total at least the type minus the main layer | Partial | W-27, W-29, W-30; DECISIONS 47 allows no less than the fixed layers plus 1/8 in (manual says 1/16 in) |
| 399 | Wall heights follow the room ceiling heights; Exterior Room edit; per-wall top and bottom edge in sections and 3D; pony division edge | Partial | W-60; edge editing missing (NO SPEC (W-139)) |
| 399-400 | Wall length edits: spec dialog, Add Break, end handles; short moves snap back at joined ends | Works | W-75, W-43, W-105; handles.rs ResizeEnd |
| 400 | Newel post spacing: Manual spacing, Move Newels edit tool and reset | Missing | NO SPEC (W-140): no newel edit tool; spacing fields only in the in-progress Newels/Balusters tab (dialogs/wall/tabs.rs) |
| 400 | Add Wall to Library edit button | Missing | NO SPEC (W-125): Wall Types Library button is disabled (W-47); no wall items in the Library Browser |

### 12.10 Edit handles

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 401 | Plan: handles along the Resize About line; move perpendicular only (Alternate for allowed angles); label Move and Rotate handles | Partial | handles.rs wall handles (start, end, mid move, bulge); DECISIONS 13 reads Alternate as 'dominant axis only', not 'allowed angles'; wall label handles missing |
| 401 | Cross section and elevation: top and bottom edges edit like closed polylines (break, angle, curve); side edges fixed | Missing | NO SPEC (W-141) |
| 401 | Moving a wall moves the cabinets attached to it, in any view | Missing | NO SPEC (W-142): cabinets are placed objects with positions (editor/placed.rs); no attachment to a wall id (grep finds none); moving a wall leaves them where they stand |
| 401 | Camera view: surface click shows corner and edge handles; top surface shows plan-style handles; footing handles | Missing | NO SPEC (W-139): heights come from the Structure tab and Floor Defaults (W-60); no height handles in sections or 3D (grep finds none) |
| 401-402 | Same Wall Type edit handles | Missing | NO SPEC (W-143): editor/handles.rs wall handles are start, end, move and bulge only (unit test wall_handles_and_hit_priority) |
| 402 | Edit Wall Intersections / layer handles / Reset Wall Layer Intersections | Missing | NO SPEC (W-144): joins.rs wall_layer_outlines solves layer corners automatically (W-37); no manual layer intersection handles |
| 403 | Editing curved walls like arcs; straight/curved combination with locked centers; Alternate overrides lock | Partial | W-66, W-67; Lock Center in the Wall dialog; override with Alternate missing |
| 403-404 | Make Arc Tangent with the Radius of Tangent Curved Wall dialog (wall radius, measured-to layer) | Partial | W-68 Works as an edit button that refits to the connected wall; no dialog with a radius |
| 404 | Lock Center edit button and dialog check box | Works | W-66 (Lock Ends / Arc Center radios); dialogs/wall.rs Lock Center |

### 12.11 Aligning walls

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 404 | Resize About line drives what stays fixed on type change, where walls snap and where length is measured | Works | W-26, W-27; plan-core walls.rs ResizeAbout (5 choices, per wall) |
| 405 | Collinear walls merge when identical; create a nook with Add Break and dragging the middle section | Works | W-45, W-43; nook recipe uses Break Wall |
| 405-406 | Align With Wall Above / Below (straight, curved, pony; foundation to line) | Missing | NO SPEC (W-145): no align-between-floors commands (grep Align With Wall finds none); the Reference Display shows the other floor and snaps to it (R-65) |
| 406 | Aligning foundation walls: Foundation To This Line in the Wall Type Definition | Missing | NO SPEC (W-137) |
| 406-407 | Aligning railings on different platforms | Missing | NO SPEC (W-146): Generate Between Platforms is stored (W-63) but nothing generates; there is no Generate on Low Platform railing option |

### 12.12 Roof directives, attic walls, knee walls

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 407 | Hip default; Full Gable Wall; High Shed/Gable Wall; Change to Gable/Hip Wall(s) edit buttons | Partial | RF-18..RF-24, W Roof tab (dialogs/wall.rs); the two edit buttons do not exist (Round 14 roof presets RF-3/RF-4 write directives instead) |
| 407 | Pitch of the roof plane over a wall; second upper pitch (mansard, gambrel, gull wing, half hip) | Works | RF-25, DECISIONS 117; dialogs/wall.rs Upper Pitch, Starts at Height |
| 408 | Overhang per wall (only when Same Height at Exterior Walls is off); Auto Roof Return on gable walls | Partial | RF-26; NO SPEC (W-147) |
| 408 | Extend Slope Downward over a bump-out | Works | RF-24, DECISIONS 119; dialogs/wall.rs Extend Slope Downward |
| 408 | Roof Cuts Wall at Bottom (floating dormer cheeks, clerestory walls) | Partial | plan-core defaults.rs roof_cuts_wall_at_bottom; plan-3d cover.rs roof_cuts_wall; wall field not found in the dialog (default only) |
| 408-409 | Lower Wall Type if Split By Butting Roof (exterior above, interior below) | Missing | NO SPEC (W-148): WallRoofDirective (plan-core walls.rs) has kind, pitch, upper pitch (rise, height), overhang, roof return and extend drop only |
| 409 | Attic Walls generated where a roof leaves a gap, on any floor; marked automatic; Auto Rebuild Attic Walls; removal choices; Combine with Above Wall | Partial | RF-31, R-68, DECISIONS 309; plan-3d cover.rs attic panels at scene time; Attic Wall check box dimmed in the Wall dialog; Combine with Above missing; NO SPEC (W-148) |
| 410 | Knee Walls: interior walls that build up to the roof | Works | RF-23; dialogs/wall.rs Knee Wall radio; plan-3d cover.rs |
| 429 | Roof panel: Dutch Gable Wall; Treat As Part of Bay/Box/Bow Window options; Include Frieze; Include Automatic End Truss Above | Partial | Dutch Gable Wall works (RF-18); remaining options missing (NO SPEC (W-148)) |

### 12.13 Stepped, raked and double walls; CAD to Walls

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 410-411 | Stepped walls and footings; S symbol; steps located by dimensions in elevation | Missing | NO SPEC (W-141): a wall has one height and a bottom offset (plan-core walls.rs height, bottom_offset); no edge breaks; see also R-88 stepped footings |
| 411 | Raked and compound raked walls | Missing | NO SPEC (W-141): a wall has one height and a bottom offset (plan-core walls.rs height, bottom_offset); no edge breaks; see also R-88 stepped footings |
| 412-413 | Double walls: Frame Through, Split Framing, Furred Wall; openings through both; curved excluded | Missing | NO SPEC (W-149): Double Wall radios are dimmed in dialogs/wall.rs; no split-framing or furred logic in plan-framing |
| 413-414 | CAD to Walls: layers for walls, doors, windows, rails; wall type by closest width within 1 in; curved lines; Convert CAD to Walls dialog | Partial | W-89; plan_import::cad_to_walls, dialogs/exchange.rs; door, window and rail layer mapping and closest-width typing not verified |

### 12.14 Wall types and the Main Layer

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 414 | Wall types are plan specific; import types from another plan; save a wall in the library | Partial | wall types live in the plan; plan-chiefplan template import brings types; Import Settings from Plan is part 1 (p. 114); library walls missing |
| 414-416 | Main Layer: structural layer, drives platforms, joins, windows, roof baseline, alignment; multiple main layers; Dimension Layer | Partial | W-46 (one Main Layer flag); roof baseline on main layer outside; multiple main layers and a separate Dimension Layer are not modeled; NO SPEC (W-137) |
| 415-416 | Interior and exterior surface naming drives materials per side and Wall Detail direction | Works | Wall.exterior_side (W-21); Materials tab per surface in progress |
| 416 | Legacy wall types converted on open (Wall-X naming) | Out-of-scope | legacy Chief X1/X2 conversion; the Chief importer maps template types instead (plan-chiefplan) |
| 416-419 | Wall Type Definitions dialog | Partial | NO SPEC (W-137): dialogs/wall_types.rs: layer table (name, thickness, material, one Main Layer), insert/delete/move, Resize About and a stack preview (W-46..W-49); no fill column, extension, energy values, platform/dimension/foundation/roof layer ... |
| 419-422 | Wall Layer Specification dialog (General, Line Style, Fill Style, Materials) | Missing | NO SPEC (W-138): WallLayer is name, thickness, is_main, material (plan-core defaults.rs); walls frame from Framing Defaults (CB-36) rather than per layer |

### 12.15 Wall Specification dialog panels

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 422-424 | General: Foundation Wall, Railing, Terrain Retaining Wall, Attic Wall, thickness, angle, length, lock start/end/center | Partial | W-24, W-75; Terrain Retaining Wall and Attic Wall dimmed (dialogs/wall.rs) |
| 423-424 | General Options: Invisible, No Room Definition, No Locate, Lock Center, No Room Moldings Exterior/Interior, Automatically Generated Wall, Ignored by Hide Exterior Walls | Works | W-24, W-25; flags in plan-core walls.rs WallFlags (auto_generated) and dialogs/wall.rs |
| 424 | Curved Wall block: radius measured to outer surface / main layer outside / inside / inner surface; Arc Center or Ends fixed; Automatic Facet Angle | Partial | W-66; Automatic Facet Angle dimmed |
| 424-427 | Structure: default wall heights, Generate Between Platforms, Ceiling and Floor platform options, wall intersection (Through Wall At Start/End), rim joists, double wall, stud layout, framing | Partial | W-60, W-62, W-63, W-39 (NO SPEC (W-134)) |
| 427-429 | Roof panel (hip, full/Dutch gable, high shed, knee, extend slope, roof cuts wall, frieze, end truss, combine, pitch, overhang, return, lower type, bay) | Partial | NO SPEC (W-148) |
| 430-431 | Foundation panel: Foundation Wall, Slab Footing, thickness / Curb Width, footing size / offset / chamfers / pour number / sill plate | Partial | W-52, DECISIONS 49; Foundation tab live (dialogs/wall.rs: footing, vertical footing, offset, center, align outside, chamfers, pour number, sill plate); footing Fill Style and Layer, Curb Width and sill Width/Height/Max Length/Count missing |
| 432-434 | Wall Types panel: type, Define, Library, layer, Pony Wall, Lower Wall Type, division height (elevation or off floor), Align Pony Walls at, Display in Plan View, openings display | Partial | W-47, W-53, W-79; 'Height Off Floor' and 'Align Pony Walls at' absent; Library disabled; wall class is one exclusive choice (WallClass), so a pony wall cannot also be a railing or a foundation wall as in Chief |
| 434 | Wall Cap panel (profile table like Moldings panel) | Partial | W-114, DECISIONS 50; three built-in profiles |
| 434-435 | Wall Covering panel: list, Add New / Replace / Delete from the library, top to ceiling and floor to bottom, interior / exterior, No Room Wall Coverings | In progress (Round 15) | W-115; dialogs/wall/tabs.rs Wall Covering (Wainscot, Wainscot Height/Thickness, Chair Rail, Base, Crown molding bands) - bands from a molding list rather than library covering objects |
| 435-441 | Rail Style, Newels/Balusters and Rails panels | In progress (Round 15) | NO SPEC (W-127); W-116, W-117 |
| 436 | Layer panel with Drawing Group | In progress (Round 15) | W-80, LAY-74 (part 1); Drawing Group dimmed in the tree today; drawing_group.rs landing |
| 436 | Materials panel: Exterior and Interior Wall Surface (not in the Materials List) | In progress (Round 15) | C-60, C-61; Materials tab (dialogs/wall/tabs.rs WallMaterials) |
| 436 | Label panel, Components panel, Object Information panel, Schedule panel | In progress (Round 15) | W-81 (Partial); tabs in dialogs/wall/tabs.rs (Components, Object Information, Schedule); W-118 |
| 437 | Railing and Fencing Specification dialogs add Rail Style, Newels/Balusters, Rails to the wall panels | In progress (Round 15) | W-56, W-116, W-117; dialogs/wall.rs WALL_TABS_EXTERIOR lists all three |
| 441-442 | Wall Hatching Specification dialog and Wall Schedules | Partial | NO SPEC (W-124) |

## 13. Rooms (pp. 443-475)

### 13.1 Room definition and defaults

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 443 | A room is any fully enclosed area of walls and railings, visible or invisible; floors, ceilings and roof generate automatically; definition vanishes when a wall is deleted | Works | R-1..R-10, R-30; rooms.rs detect_rooms |
| 443 | Subdividing a room: inherited characteristics; invisible wall break rules for floor and ceiling heights | Partial | NO SPEC (R-98): rooms.rs detect_rooms uses invisible walls (R-4, R-5); the height-dependent break edge is not implemented; the split-off room keeps the parent's name only via RoomName anchor (R-15) |
| 443 | Copy and paste rooms with their walls (type kept when using Edit Area tools) | Partial | S-90 Edit Area (DECISIONS 73) copies objects inside the area; room name/type travel with the anchor point (R-14); not verified for pasted copies |
| 444 | Floor Level Defaults: ceiling heights, materials, moldings; Floor 1 height fixed at 0; only Floor 1 defaults set ahead of time | Works | R-56, R-57; dialogs/floor_defaults.rs (part 4 owns the rows) |
| 444 | Floor and ceiling platform structure and finish as a hierarchy of dynamic defaults (Default check boxes) | Works | R-22, R-28, R-29, R-56; dialogs/room.rs 'Use Floor Default' switches |
| 444-445 | Room Types dialog: Edit, Copy, Rename, Delete, Select All, Clear All | Works | R-37, R-38; DECISIONS 310 (check boxes drive Delete; manual's Select All/Clear All only select) |
| 445 | Room Type Defaults dialog panels (name, function, living, conditioned, ceiling and floor structure and finish, deck framing, layer, fill, moldings, components); multi-type editing | Partial | NO SPEC (R-99): plan-core defaults.rs RoomTypeDef has only name, function, living area, conditioned, default floor finish; dialogs/default_lists.rs RoomTypesDialog edits those (R-37, R-38) |
| 445 | Room Label Defaults dialog: Label and Dimension Format panels; existing labels unchanged | Partial | NO SPEC (R-100): default_pages/plan.rs room_label page (name, area, dimensions, ceiling, text style, border, alignment) is stored only (no bound field); per-room Label tab works (R-44..R-50) |

### 13.2 Room types and functions

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 445 | New enclosed areas start as type Unspecified; type assignment in the Room Specification | Works | R-13, R-39; rooms_edit.rs |
| 446 | Room Functions: non-editable property sets, Interior / Exterior / Hybrid, default Function per type | Partial | NO SPEC (R-101): rooms.rs function_defaults handles Garage, Deck, Porch, Open Below, Flat Roof plus type names Attic, Courtyard, Basement, Crawl Space; no Balcony, Court or Slab function; Slab-thickness floor rule absent; DECISIONS 300 invents ... |
| 446 | Living and Conditioned Area defaults by function (open below conditioned) | Partial | R-42, R-43 (conditioned stored, not used by any schedule or export); function table NO SPEC (R-101) |
| 446 | Ceilings and roofs by function: interior flat ceiling and roof; exterior none; attic ignored by roof generator; garage, slab and porch have ceiling and roof | Partial | R-30, R-40; roof_view.rs room_roof; function table NO SPEC (R-101) |
| 446-447 | Floors and foundations by function: Open Below no floor, stairwells and crawl spaces; garage foundation with slab; Garage/Slab floors show on Floor 0; Slab = slab thickness; Deck no foundation | Partial | R-40, R-18, DECISIONS 300, 308; Garage drop and slab are built; Slab function and floor-0 display rule are not; NO SPEC (R-101) |
| 447 | Doors and windows follow room function (window faces out, exterior door defaults and threshold) | Differs | NO SPEC (R-102): openings choose exterior/interior from the wall kind and the opening's own style (DECISIONS 42, 43, plan-core openings), not from the rooms on each side |
| 447 | Electrical by function: weatherproof outdoor fixtures, GFCI over kitchen and bath base cabinets, fewer outlets in hybrid rooms, none in exterior or Open Below | Partial | E-10, DECISIONS 90 (exterior WP GFCI); Auto Place Outlets rules by room function not verified |
| 447 | Plan Check uses room functions (closet vs bedroom smoke detector) | Partial | plan-check rules use room type names (rules_irc.rs); function property sets not used |

### 13.3 Displaying and selecting rooms

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 447 | A room is a space bounded by walls, not an individual object; labels in plan, sections and camera views | Partial | R-16, R-44; labels draw in plan; section and camera labels not found |
| 447 | Room layers: primary and secondary layers, Object Layer Properties edit button, Rooms layer default and custom layer | Missing | NO SPEC (R-103): ROOM_TABS in dialogs/room.rs has no Layer tab; Drawing Groups table covers 'Rooms' by kind (drawing_group.rs KINDS) |
| 447-448 | Room fill: transparent by default; solid or pattern per floor or per room; needs the room layer on | Works | R-35; Floor Defaults fill; render.rs |
| 448 | Floor overview, room layer off hides floor and ceiling and makes the room unselectable | Partial | part 5 (p. 1153); layer off hides rooms in plan (W-93 rule); 3D behaviour not verified |
| 448 | Calculate Materials in Room / From Selection | Missing | NO SPEC (R-104): Materials List counts the whole plan by surface (DECISIONS 131); no per-room or per-selection list |
| 448 | Room Finish Schedule (size, structure, materials, moldings, trim, 2D symbol), room types included by default | Partial | default_pages 'schedules.room_finish'; plan-docs schedule_kinds.rs Room kinds (L-23..L-32); tray ceiling and 2D symbol columns not verified |
| 448 | Create Schedule from Room | Missing | NO SPEC (R-105): schedules are plan-wide (plan-docs schedule_kinds.rs); no room filter |
| 449 | Selecting a room: click inside highlights interior; group select; Marquee Select Similar with restrictive selection; needs unlocked layer; selection color preference | Works | R-16, S-29, S-30; Preferences > Colors selection |
| 449-450 | Exterior Room selected by clicking outside an exterior wall, or its label | Missing | NO SPEC (R-106): no exterior room object (grep exterior room finds none); Floor Defaults dialog sets level heights (R-56) and the status line reports living area (DECISIONS 311) |
| 450 | Select a room in 3D by clicking floor or wall (Select Next Object, Select Room Before Wall in 3D) | Partial | C-43 3D pick, S-34 Tab cycling; the preference is not found |
| 450 | Match Properties for rooms (shared attributes, Apply Properties) | Partial | S-116 Object Painter/Match Properties (Round 15 painters, DECISIONS 451-453) |

### 13.4 Editing rooms

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 450-451 | Edit default floor/ceiling heights of a level by dragging the Exterior Room edges; room heights by dragging a wall edge handle; wall heights in 3D | Missing | NO SPEC (W-139) |
| 452 | Room Specification dialog (multi-room limited); Match Properties; Style Palettes | Partial | dialogs/room.rs single room; multi-room editing absent (only walls have it, W-83); Style Palettes are part 1 (p. 308) |
| 451-452 | Material Painter changes room surfaces; room moldings replaced from the library in 3D | Partial | C-56 Material Painter (DECISIONS 126); room molding replacement from library in 3D not found |
| 452 | Room Edit toolbar buttons | Missing | NO SPEC (R-107): editor/actions.rs common_edit_actions only; the room has Open Object and delete-type buttons (S-107); Auto Interior Dimensions and Auto Interior Elevations exist as tools you click in a room (DIM-27, tools/camera.rs) |
| 452 | Turn Ceiling Off / Turn Ceiling On (removes the flat ceiling) | Partial | RoomName.has_ceiling toggle in the Structure tab (R-30); no edit buttons |
| 452 | Build Framing for Selected Object on a room | Partial | Build Framing dialog builds whole floors (CB-36); selected-object build not found |
| 452 | Create Room Elevation Views | Partial | tools/camera.rs Auto Interior Elevations: click a room (four wall elevations); no Edit button from a selected room |
| 452 | Auto Room Dimensions edit button | Partial | DIM-27 DimMode::AutoInterior: click a room; no edit button |

### 13.5 Room labels and Living Area

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 453 | Automatic label: room name from type or custom name; size lines Interior Dimensions, Interior Area, Standard Area per type and per room | Works | R-44..R-50; extras.rs RoomLabelOptions; rooms.rs standard_area_sq_in |
| 453 | Per-view display layers for the three size formats (Rooms, Interior Dimensions / Interior Area / Standard Area) | Missing | layers.rs default set has Rooms and Room Labels only |
| 453 | Standard Area measured by the Living Area to setting (outside surface or main layer outside), rounded to the nearest square foot, bay windows excluded | Partial | NO SPEC (R-108): plan-core rooms.rs fill_surface_areas offsets exterior walls to their outside surface only (R-49); no main-layer choice, no bay-window exclusion, rounding not verified |
| 453 | Name-value pairs %dimensions%, %internal_area%, %standard_area% in macros | Partial | R-47: label tokens <name>, <area>, <std_area>, <dims>... (ours); text macros are part 3 (p. 568) |
| 454 | Custom labels, suppressing the label, text macros with room.nvp_name context, absolute or relative position, own text style | Partial | R-46, R-47; DECISIONS 311 (five placements, dragged offset) |
| 454 | Room labels on Rooms, Labels layer with its text style; label settings per room | Works | R-46; Label tab; layer 'Room Labels' |
| 454-455 | Living Area rules: interior rooms in, exterior and hybrid out, rough ceiling under 48 in out, per-room override, Living Area label per structure, Show Living Area Label switch, Make Living Area Polyline | Partial | R-42, R-51 (override works); rest NO SPEC (R-109) |
| 455 | Footprint of a floor by Make Room Polyline on the Exterior Room | Missing | NO SPEC (R-110): Tools > Checks > Plan Footprint makes one outline polyline (L-38); no room polyline commands (grep find none) |

### 13.6 Decks

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 455 | Deck room: exterior-type room with planking and framing platform; created by Deck Railing, Deck Edge, Polygon Shaped Deck or by assigning the Deck type | In progress (Round 15) | CB-86 (decks_chimneys brief); rooms.rs function_defaults 'Deck'; plan-core deck.rs DeckSpec; DECISIONS 205 |
| 456 | Deck planking and framing defaults in Automatic Framing Defaults; Deck Room Defaults in Room Type Defaults | Missing | no Automatic Framing Defaults deck section; Deck type defaults page not found; NO SPEC (R-99) |
| 456 | No roof by default; Roof Over This Room; Post to Beam railing for roof support; doorway gates in deck railings | Partial | RoomMisc.roof_over (R-30); Post to Beam in the in-progress Rail Style tab (dialogs/wall/tabs.rs) |
| 456 | Calculate Structural Materials for Deck (planks ripped to full width in the list) | Partial | Build Framing > Deck members count in the takeoff and framing layers (DECISIONS 206); a deck-only calculation button is absent |
| 456 | Deck framing, planking, posts and beams on their own Framing, Deck... layers | Partial | DECISIONS 206 members labelled 'Deck ...' on the framing layers; separate Deck layers not created |
| 456 | Room under a deck: uncheck Roof Over and Flat Ceiling Under; the deck platform is not its ceiling | Missing | no rule found (deck platform handling of rooms below is not described in DECISIONS 205-207) |
| 456-457 | Editing and retaining deck framing: Automatically regenerate deck framing, Keep deck framing after the room is deleted, Build Deck Framing edit tool, schedule of deck planking by room | In progress (Round 15) | DECISIONS 206: Build Framing > Deck makes manual members replaced on rebuild or Delete Deck Framing; no regenerate-on-change switch, no keep-after-delete switch |
| 470-471 | Deck panel: Plank Overhang, Max Plank Length, Plank Gap Width, Plank Direction (automatic or degrees), Number of Border Planks, No Border Against Walls, Herringbone, Treated, joist spacing and direction, ledger offset, connect rim joists to ledger | In progress (Round 15) | dialogs/room.rs Deck tab: Board Width/Thickness/Material, Gap Between Boards, Board Direction, Picture Frame Border, Border Rows, Joist Size/Spacing/Angle, Rim Joists, Ledger, Overhang at the Rim; no Max Plank Length, No Border Against Walls, Herringbone or Treated |
| 472-474 | Deck Support panel: beams (construction, depth, ply width and count, with/under joists, max spacing, offset from edge, flush), posts (construction, depth, ply width and count, match depth, max spacing, offset from end, alignment), post footings (height above terrain, thickness, width, shape, rebar) | In progress (Round 15) | dialogs/room.rs Deck tab: Beam Size, Beam Plies, Beam Set Back from the Rim, Post Size, Greatest Post Spacing, Footing Size/Thickness, Deck Height Above Grade; no round footings, rebar, alignment, offset from end or framing member definitions |

### 13.7 Tray, coffered and special ceilings; room moldings; room polylines

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 457-458 | Tray Ceiling Polyline tool, Make Tray Ceiling in Room, Make Nested Tray Ceiling, coffered ceilings, Explode Tray Ceiling | Missing | NO SPEC (R-111): no tray ceiling tool or object (grep tray, coffer find nothing relevant); Ceiling Plane tool exists for sloped ceilings (plan-roof ceiling.rs, R-32) |
| 458-460 | Tray Ceiling Specification (General, Polyline, Moldings, Rope Lights, Line Style, Fill Style, Materials, Label, Components) | Missing | NO SPEC (R-112): no dialog; rope lights exist as electrical items (tools/electrical.rs) but not on a tray |
| 460-461 | Lowered ceiling by adding a framing layer in the Ceiling Finish Definition without changing wall tops | Partial | R-27, R-29: Define editors for ceiling structure and finish layers; a layer role of Framing or structural type is not available |
| 461-462 | Cathedral ceiling (uncheck Flat Ceiling Over This Room) and vaulted ceiling (Ceiling Planes), Turn Ceiling Off | Partial | R-30 Flat Roof / has_ceiling; R-32 Ceiling Planes (roof_view Build Ceiling Planes); cathedral tied to the roof underside not verified |
| 462 | Cantilever underside material on the Room Materials panel (No Material option) | Partial | R-36, DECISIONS 307: platform edges and underside are not separate materials |
| 462 | Room moldings: floor, room type and room levels; suppression behind cabinets and by wall flags; convert to molding polylines | Partial | NO SPEC (R-113): R-34 moldings built for the room (DECISIONS 306) break at openings; wall No Room Moldings flags are stored but 'not built' (DECISIONS 306); no cabinet suppress switch; no floor-level or room-type molding defaults |
| 463-464 | Room polylines: Make Room Polyline, Standard Area Polyline, Room Molding Polyline (+dialog), Expand Room Polyline | Missing | NO SPEC (R-110): Tools > Checks > Plan Footprint makes one outline polyline (L-38); no room polyline commands (grep find none) |

### 13.8 Room Specification dialog

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 464-466 | General panel: Room Type with Define, name, Show Room Label, Living Area 3-way, Conditioned 3-way, Roof Group, Room Information report, preview | Partial | R-19..R-21, R-42, R-43; Function shown read-only; Roof Group missing; NO SPEC (R-114) |
| 466-470 | Structure panel (absolute and relative heights, ceiling, floor) and cross-section preview | Partial | NO SPEC (R-115): dialogs/room.rs Structure tab has Floor Height, Ceiling Height, Rough Ceiling, Finish thicknesses, Floor Under, Ceiling Over, Roof Over, Flat Roof, Stem Wall Height, Monolithic Slab, platform Define editors, a section preview ... |
| 470-474 | Deck and Deck Support panels | In progress (Round 15) | see Decks rows (CB-86) |
| 474 | Moldings panel (base, crown, chair rail from a molding table like other dialogs) | Partial | R-34, DECISIONS 306: nine built-in profiles, one each of three kinds |
| 474 | Wall Covering panel (applies to every wall of the room) | Partial | R-84; Round 15 wall tabs let walls carry coverings (dialogs/wall/tabs.rs) |
| 474 | Layer panel with Drawing Group | Missing | NO SPEC (R-103): ROOM_TABS in dialogs/room.rs has no Layer tab; Drawing Groups table covers 'Rooms' by kind (drawing_group.rs KINDS) |
| 474 | Fill Style panel | Works | R-35 |
| 474 | Materials panel (floor, ceiling, wall surfaces) | Partial | R-36, DECISIONS 307 |
| 474 | Label panel | Partial | R-44..R-50 (Text Style per R-46); 'Fill Style panel' heading in the manual is a typo for Label |
| 475 | Components panel, Object Information panel, Schedule panel | Partial | dialogs/room.rs tabs Components (finish layers only, L-35), Object Information, Schedule (L-29, L-30) |
| 449 | Exterior Room Specification dialog (exterior wall coverings and materials) | Missing | NO SPEC (R-106): no exterior room object (grep exterior room finds none); Floor Defaults dialog sets level heights (R-56) and the status line reports living area (DECISIONS 311) |

## 14. Dimensions (pp. 476-520)

### 14.1 Preferences, defaults and accuracy

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 476 | Files keep one unit system chosen at creation (US or metric); template choice in Preferences New Plans | Partial | UnitDefaults.imperial in plan-core defaults.rs; metric/US switching and New Plans panel are part 1 (p. 138) |
| 476 | Minimum on-screen size of dimension numbers (Preferences Appearance) | Partial | DIM-7 printed-size text; no minimum-size preference found |
| 476 | Dimension Line Separation Snaps preference | Missing | NO SPEC (DIM-49): dialogs/snap_settings.rs and Preferences Snap Properties have no such option (grep Separation Snaps finds none) |
| 476 | Default Settings > Dimension: Dimension Defaults and Auto Story Pole defaults, reachable by double-clicking the Dimension Tools button and the Temporary Dimension Defaults button | Partial | dialogs/defaults.rs Dimension group (Dimensions plus eight kind pages, Round 15); double-clicking the Dimension Tools button to open the defaults was not found |
| 476-477 | Saved Dimension Defaults (multiple) activated from a toolbar control, Active Defaults dialog, Saved Defaults dialog, saved views or Default Sets | Partial | DIM-6, DIM-40: 14 template sets (default_lists.rs Saved Dimension Defaults), active set switch; saved views and Default Sets do not carry a set |
| 477 | Rounding methods (Grid Rounding, Distance Rounding) and accuracy indicators | Missing | NO SPEC (DIM-50): plan-core dimension.rs DimFormat::fmt_len rounds each value to the smallest fraction; no rounding method, no indicators |

### 14.2 Dimension Defaults dialog panels

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 478-480 | General panel | Partial | NO SPEC (DIM-51): default_lists.rs: Text Above Dimension Line, Leader Style, Fraction Text Size, Line Separation, Exterior Offset (DIM-6, DIM-40); default_pages/dimension.rs baseline_separation and reach; no centered/below/angle choice, no leader ... |
| 481-482 | Setup Automatic panel | Partial | NO SPEC (DIM-52): default_lists.rs Setup Automatic: Exterior Offset, Line Separation and the Exterior Strings list (openings, wall to wall, overall; DIM-24, DIM-26); no minimum area, 3D height, auto refresh, room or elevation options, no ... |
| 482-484 | Setup Temporary panel | Partial | NO SPEC (DIM-53): DimensionDefaults.temp_locate (LocateGroup) edited on the merged Locate Objects tab (DIM-40, tempdim.rs TempLocate); no row limit, reach, primary/secondary side or inside-CAD options |
| 484-489 | Locate Manual / End to End / Centerline / Interior / Auto Exterior / Auto Room / Auto Elevation / Elevations panels | Partial | NO SPEC (DIM-54): one merged Locate Objects tab (default_lists.rs DIM_TABS): Walls, Openings (sides or centers), Cabinets, Fixtures with groups for Manual and Automatic, Temporary and Elevation (DIM-4, DIM-40); no per-tool panels, no outer/inner ... |
| 489-491 | Primary Format panel: units, unit indicators, zeros, thousands separator, display as inches, fraction style and size, decimals 0-20 or smallest fraction 1-128, show denominator, reduce, GCD or closest, angular format | Partial | DIM-6 Works: Units (6), Smallest Fraction, Fraction Style, Decimal Places, Unit Indicators, Trailing Zeroes (dialogs/default_lists.rs Primary Format; DimFormat; zero-feet suppression is only a per-dimension override); thousands separator, leading zeros, display-as-inches threshold, reduce method and angular format missing |
| 491 | Secondary Format panel | In progress (Round 15) | NO SPEC (DIM-55): DIM-46 Missing; dims2 brief adds Secondary Format and tolerance; DimFormat has no second format today |
| 491-493 | Extensions panel and Centerline options | Partial | NO SPEC (DIM-56): DIM-7: gap, overshoot (extension_past), toward length and fixed proximity (DimensionDefaults extension_toward, extension_proximity); no Auto Mark Centerlines, no centerline symbol options |
| 493 | Layer panel | Partial | NO SPEC (DIM-57): layers 'Dimensions, Manual' and 'Dimensions, Automatic' (layers.rs); dialogs/dimension.rs Layer tab per dimension; no Layer panel in the defaults dialog |
| 493 | Arrow panel (default arrow style and size) | Works | DIM-39; default_lists.rs Arrow (DimArrow Tick, Arrow, Dot, None; size; Leader Style) |
| 493 | Text Style panel | Works | DIM-39; Text Style tab; dimension labels use the defaults' style, not the layer's |
| 493-496 | Auto Story Pole Dimension Defaults dialog (General, Inner/Outer/Marker Format, Locate Objects, Locate Elevations, Layer) | In progress (Round 15) | NO SPEC (DIM-58): dims2 brief; default_pages/dimension.rs Story Pole page (Pole Width, Tick Length, Label the Floors, Mark Plate Heights) invents fields that Chief does not have, and omits Chief's sides, reach, ridge/height marks and format panels ... |

### 14.3 Manually drawn dimension tools

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 497 | Manual Dimension: locate all eligible objects between the start and end points within Reach | Works | DIM-11, DIM-4; DimMode::Manual |
| 497 | End-to-End Dimension: locate only the ends | Works | DIM-13 |
| 498 | Angular Dimension between any two straight edges (walls, cabinets, polyline sides, edges up to four levels inside a CAD block) | Partial | DIM-18; walls and CAD lines; cabinets and block-nested edges not verified |
| 498 | Interior Dimension: room interiors, no exterior wall faces by default, no wall widths | Works | DIM-14; interior_locates_interior_surfaces |
| 498 | Point to Point: point markers at its ends (Default behaviour) or locating objects along its length (Alternate) | Partial | DIM-15; marker creation missing; NO SPEC (DIM-47) |
| 498-499 | Baseline Dimension: several lines from one origin, Line Separation spacing, not in layout files | Works | DIM-17; DimensionDefaults.baseline_separation |
| 499 | Running Dimension: several measurements on one line from a start circle sized like the arrow | Works | DIM-16; start circle size follows arrow size not verified |
| 499-500 | Centerline Dimension: centers of walls, openings, cabinets, fixtures; centerline symbol on dashed extension lines; also stairs, footings, electrical, CAD via an extension at the midpoint | Partial | DIM-19; DimMode::Centerline; centerline symbol and extension at arbitrary midpoint not verified; NO SPEC (DIM-59) |
| 500 | Tape Measure: temporary line between two points, snappable, disappears on release | Works | DIM-20; DimMode::TapeMeasure |
| 500 | Delete Dimensions tool (view, or all layout pages) | Partial | NO SPEC (DIM-60): Delete Objects dialog by category (S-88, dialogs/delete_objects.rs Dimensions) deletes all manual/automatic dimensions; no tool; segment removal is Delete Extension Line (DECISIONS 82) |
| 497 | Dimension lines only locate parallel (or nearly) objects; Angular exception; zoom level changes what is located | Works | DIM-4, DIM-21; tools/dimension.rs locate() |
| 497 | Dimensions update when located objects move or resize; objects move by editing the value | Works | DIM-3, DIM-29, DIM-32, DIM-35; dim_assoc.rs |
| 497 | Dimensions available in plan, sections and elevations, CAD details and layout pages | Partial | plan and CAD details (details are floors); layout pages carry dimensions through boxes (plan-layout), no free layout dimensions; sections/elevations have none (NO SPEC (DIM-61)) |

### 14.4 Automatic dimension tools

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 500 | Temporary Dimensions on selected objects (edge to edge and neighbours, endpoints within 4 ft, View > Temporary Dimensions toggle) | Works | S-56..S-63, W-74; editor/tempdim.rs; ViewFlag::TemporaryDimensions |
| 500 | Temporary Dimensions in 3D views report height and width of doors, windows and cabinet sides | Missing | no temporary dimensions in 3D (NO SPEC (DIM-62)) |
| 501 | Auto Exterior Dimensions: up to three rows per wall (openings, walls, overall); needs defined rooms; plan, cameras and overviews | Partial | DIM-24..DIM-26 Works in plan; camera views missing (NO SPEC (DIM-62)) |
| 501 | Auto Elevation Dimensions in cross section / elevation views | In progress (Round 15) | NO SPEC (DIM-61): DIM-28 Differs-by-design (plan-axis strings); DimMode::AutoElevation and AutoStoryPole in tools/dimension.rs; dims2 brief builds elevation versions; dimensions live on floors, not in section views |
| 501 | Auto Story Pole Dimensions | In progress (Round 15) | NO SPEC (DIM-61): DIM-28 Differs-by-design (plan-axis strings); DimMode::AutoElevation and AutoStoryPole in tools/dimension.rs; dims2 brief builds elevation versions; dimensions live on floors, not in section views |
| 501 | Auto Room Dimensions (edit button on selected rooms, inside the room, ignores No Locate) | Partial | DIM-27 as a tool; edit button missing (NO SPEC (R-107)) |
| 500 | Edited automatic dimensions are marked and survive a regenerate | Works | DIM-25, DIM-33; Dimension.auto_group cleared on edit |
| 481-482 | Auto Refresh: delete and replace automatic dimensions whenever the model changes | Partial | DIM-29: strings follow their walls and openings (dim_assoc.rs) but new walls never get new dimensions; no switch (NO SPEC (DIM-52)) |

### 14.5 Displaying and selecting dimension lines

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 501 | Layer Display Options and Drawing Groups control dimension display; Default Sets switch looks | Partial | layers 'Dimensions, Manual/Automatic' (LAY-1); Drawing Groups kind 'Dimensions' (Round 15 cad2); Default Sets are not a feature (Default Settings 'Default Sets' page is stored only) |
| 501-502 | Components of a dimension line: line, numbered extension lines, arrows, labels, leader lines | Partial | render.rs draw_dimension_look draws line, extensions, ticks/arrows, text with outside-text leader (DIM-8, DIM-9); extension numbers when selected absent |
| 502 | Dimension labels: text style from defaults (not the layer), solid background fill like the Preferences background or transparent style, scaling to paper, min on-screen size, primary and secondary format, centered/above/below | Partial | DIM-7, DIM-8; secondary format missing (DIM-46); background mask fill not verified |
| 503 | Dimension label edit handles (move, rotate); leader lines for offset labels (line or arc, 1-2 segments, arrow) | Missing | NO SPEC (DIM-63) |
| 503 | Dimension arrowheads: style, color, size; hidden when two overlap | Partial | DIM-39 four end marks (DimArrow); arrow library, colors and overlap hiding absent |
| 503-504 | Elevation markers on vertical dimension extensions, and grade level reference | In progress (Round 15) | NO SPEC (DIM-64): story_pole_dimensions (plan-core dimension.rs) writes heights; no marker object, no Marker Format, no grade-level setting (General Plan Defaults Elevation Reference not found) |
| 504 | Dimension lines in camera views and overviews | Missing | NO SPEC (DIM-62): dimensions are floor objects drawn in plan (render.rs draw_dimension); the 3D view draws none (grep finds no 3D dimension drawing); part 5 (p. 1177) owns Annotating 3D Views |
| 504 | Selecting a dimension line: click along it, click an extension line for limited handles, Esc for full handles, group select with limits | Partial | select.rs picks dimensions (ObjectRef::Dimension, Op::DimOffset); extension-only selection absent |
| 504-505 | Seven handle types | Partial | NO SPEC (DIM-63): editor/handles.rs gives a selected dimension one perpendicular move handle; end points move through the Dimension tool (DIM-30) and extension lines through Add/Delete Extension Line (DECISIONS 82); no label move/rotate, no line ... |
| 505 | Edit toolbar: reposition, copy, delete; Edit Extensions; Add Additional Text | Partial | DIM-41 edit_actions (Reverse, Convert to Manual, Align, Distribute, Add/Delete Extension Line); the other two buttons are absent |
| 506 | Dimension number size per dimension; subject to print scaling | Partial | DIM-7: text style character height or printed size per set; per-dimension Number Height absent (look overrides do not include it) |
| 506 | Copy and paste dimensions between views and files; defaults matched by name; pasted onto point markers when objects not copied | Partial | DIM-38 anchors re-tied (DECISIONS 85); saved-default matching by name and point markers absent |
| 506 | Deleting dimensions: Delete key, Delete Dimensions tool, Delete Objects dialog by scope, Auto Refresh prompt, single segment removal | Partial | NO SPEC (DIM-60): Delete Objects dialog by category (S-88, dialogs/delete_objects.rs Dimensions) deletes all manual/automatic dimensions; no tool; segment removal is Delete Extension Line (DECISIONS 82) |

### 14.6 Editing extension lines, additional text and moving objects

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 507-509 | Add, move and delete extension lines; Add Extension Line handle; extension numbering | Partial | NO SPEC (DIM-59): Add/Delete Extension Line (DECISIONS 82); fixed extension length in DimOverrides; no moving to another mark, no per-extension resize, no Centerline mark edit tool (DimMode::Centerline draws centerline dimensions) |
| 509 | Resize extension lines (spec dialog or end handles; Edit Extensions button) | Missing | NO SPEC (DIM-59): Add/Delete Extension Line (DECISIONS 82); fixed extension length in DimOverrides; no moving to another mark, no per-extension resize, no Centerline mark edit tool (DimMode::Centerline draws centerline dimensions) |
| 510 | Fixed Proximity for one extension line | Partial | DimensionDefaults.extension_proximity; DimOverrides fixed length; per-extension choice absent; NO SPEC (DIM-59) |
| 510 | Mark as Centerline / Remove Centerline Mark on an extension line | Missing | NO SPEC (DIM-59): Add/Delete Extension Line (DECISIONS 82); fixed extension length in DimOverrides; no moving to another mark, no per-extension resize, no Centerline mark edit tool (DimMode::Centerline draws centerline dimensions) |
| 510-511 | Additional Text dialog: leading text, trailing text, suppress value; also in the Segments panel | Partial | NO SPEC (DIM-65): Dimension.text_override replaces the whole text (DIM-10, dialogs/dimension.rs Label tab); no leading/trailing text, no per-segment settings, no highlight preference |
| 511 | Highlight Overridden Dimension Text preference | Missing | NO SPEC (DIM-65): Dimension.text_override replaces the whole text (DIM-10, dialogs/dimension.rs Label tab); no leading/trailing text, no per-segment settings, no highlight preference |
| 511 | Move objects by editing a dimension value (any dimension type, locked layers allowed, hand pointer, negative values pass the other object, math in the field, Esc cancels, bumping) | Works | DIM-32, DIM-33, DECISIONS 83; tempdim.rs; locked-layer move through dimension not verified |
| 511-512 | Move Edge / Move Entire Object buttons; resizing polyline and box objects by edge dimension | Partial | NO SPEC (DIM-66): DIM-32, DIM-33: the typed value moves the object at the nearer end; no button choice; resize of polylines by dimension not verified |
| 396-397 | Wall resize buttons (Move Left/Right/Top/Bottom End, Both Ends, Along Rails) | Missing | NO SPEC (DIM-48) |
| 512 | Resizing a structure with exterior dimensions one wall at a time | Works | W-99, DIM-43; typed dimension moves the wall (select.rs test) |
| 512-513 | Angular dimension changes: rotate edge or rotate entire polyline (Set Angular Dimension dialog) | Missing | NO SPEC (DIM-67): angular dimensions are measured and editable by value like others (DIM-18, DIM-32); no rotate-edge-versus-whole choice dialog |

### 14.7 Dimension Line Specification dialog

| Page | Feature | Status | Evidence |
|---|---|---|---|
| 513-515 | Dimension panel (inherits from, locate settings, number height, display wall widths, gaps between cabinet face items, rounded indicators, text position and orientation) | Partial | NO SPEC (DIM-68): dialogs/dimension.rs tabs General, Primary Format, Arrow, Text Style, Layer, Label (DIM-31, DIM-39); missing Secondary and Marker Format, Extensions/Markers, Segments, saved-default switch, Display Wall Widths, rounded-value ... |
| 515 | Angular Dimension panel variant (Use Default Angle Style) | Missing | NO SPEC (DIM-68): dialogs/dimension.rs tabs General, Primary Format, Arrow, Text Style, Layer, Label (DIM-31, DIM-39); missing Secondary and Marker Format, Extensions/Markers, Segments, saved-default switch, Display Wall Widths, rounded-value ... |
| 516 | Primary Format panel (Use Default Formatting off) | Works | DIM-31, DECISIONS 84; DimOverrides units, fractions, decimals, unit indicators, zero feet |
| 516 | Secondary Format panel | In progress (Round 15) | NO SPEC (DIM-55): DIM-46 Missing; dims2 brief adds Secondary Format and tolerance; DimFormat has no second format today |
| 516 | Marker Format panel | Missing | NO SPEC (DIM-64): story_pole_dimensions (plan-core dimension.rs) writes heights; no marker object, no Marker Format, no grade-level setting (General Plan Defaults Elevation Reference not found) |
| 516-518 | Extensions/Markers panel: extension table, fixed proximity, length settings, centerline or elevation marker | Missing | NO SPEC (DIM-68): dialogs/dimension.rs tabs General, Primary Format, Arrow, Text Style, Layer, Label (DIM-31, DIM-39); missing Secondary and Marker Format, Extensions/Markers, Segments, saved-default switch, Display Wall Widths, rounded-value ... |
| 518-520 | Segments panel: blank segment, leading and trailing text, suppress value, angle, leader line | Missing | NO SPEC (DIM-65): Dimension.text_override replaces the whole text (DIM-10, dialogs/dimension.rs Label tab); no leading/trailing text, no per-segment settings, no highlight preference |
| 520 | Layer panel (with Default check cleared on edit) and Arrow panel | Partial | dialogs/dimension.rs Layer tab (Manual or Automatic) and Arrow tab (style, size, fill) |
| 498 | Dimensions in CAD blocks | Missing | NO SPEC (DIM-69): Make CAD Block excludes dimensions (CAD-31 Partial) |
| 511 | Dimensions with added text exported to DXF/DWG as text and lines | Differs | NO SPEC (DIM-70): plan-core export/dxf.rs writes every dimension as lines and text (L-44, L-32); no DIMENSION entities, so this is already the stated Chief fallback |

## Dialog panels

For every specification and defaults dialog in pages 319-520: Chief's panel list, what Plan Studio has today, the panels missing, and the fields missing per panel. "Dimmed" means a control that exists in `dialogs/wall.rs` but is disabled.

| Dialog | Pages | Chief panels | Plan Studio today | Missing panels | Missing fields per panel |
|---|---|---|---|---|---|
| CAD Defaults / Displayed Line Length | 319-323 | one page: Current CAD Layer (+Define), Displayed Line Length Format (+Define), Display Line Angles As, Options (Show Arc Centers and Ends); Displayed Line Length dialog: Format, Accuracy, Preview | no dialog; Current CAD Layer in Active Layers by Tool; Default Settings > CAD has 12 stored pages (General CAD, Arcs, Boxes, Circles, Lines, Polylines, Splines, Points, Markers, Callouts, Leaders, Insert Point) that no tool reads | CAD Defaults; Displayed Line Length | all fields: length format (units, zeros, thousands separator, inches threshold, fraction style and size, accuracy, reduce method), angle style (degrees, quadrant, azimuth), Define buttons |
| Revision Cloud Defaults / Specification | 319, 350-352 | Cloud (Arc Length and Number: automatic / average / count; Length Diversity; Minimum and Maximum Bulge; preview), Selected Line or Arc, Line Style, Fill Style, Label | none (a drawn cloud is a polyline: General, Line Style, Fill Style, Layer) | Cloud, Selected Line/Arc, Label, Defaults | arc length mode, average arc length, arc count, diversity, bulge limits; saved defaults |
| New CAD Point / Move Point / New CAD Line / New CAD Arc | 325-338 | Point: current point, absolute / relative / polar, Next. Move Point: current location, absolute / relative to itself / previous point / along line (distance or %). Line: start point, end point by absolute / relative to start (polar) / relative to previous line. Arc: start point, direction (start or chord), radius and curve side, length by angle / arc length / chord length | typed fields in the canvas strip: Input Point (X, Y), Input Line (length, angle), Input Arc | four dialogs | relative and polar forms, Next, previous-line form, chord direction, three length methods, Move Point entirely |
| Line Specification | 332-335 | Line (locks, length/angle, start, end, visibility), Line Style (layer, color, style, weight, bumping, drawing group, display options, layout page), Arrow | CAD dialog: General, Line Style, Arrow, Layer (OPEN_TABS in dialogs/cad.rs) | none by name | Line: Lock Start/End/Center/Length-Angle, Show Selected Edge. Line Style: Default layer check, Define, By Layer boxes for style and weight, CAD Stops Move, Wall Stops Move, Drawing Group, Show Length, Show Angle, All Angles, Reverse Angle, Layout Page. Arrow: attach and auto-position tail/head, arrow line style, block fill and fill color (start and end styles are separate, which covers both ends) |
| Arc Specification | 339-341 | Arc (locks; center, radius, start/end/arc angle, arc length, facet angle; start point and direction; end point and direction; chord), Line Style, Arrow | same CAD dialog (center, radius, start and end angles) | none by name | Arc: locks, arc length, facet angle, start/end X and Y, start/end direction, chord length and angle |
| Circle / Oval / Ellipse Specification | 342-343 | General (center, angle, size / diameter, radius, circumference), Line Style, Fill Style | CAD dialog: General, Line Style, Fill Style, Layer for a circle; oval and ellipse are polylines | none for circle; oval/ellipse have no dialog of their own | General: angle, length and width for oval and ellipse, circumference for circle |
| Polyline Specification | 344-347 | Polyline (counts, length, perimeter, area, volume, holes), Spline, Selected Line, Selected Arc, Line Style, Fill Style, Arrow, Label, Components, Object Information, Schedule | CAD dialog: General (vertices, closed), Line Style, Fill Style, Arrow, Layer | Spline, Selected Line, Selected Arc, Label, Components, Object Information, Schedule | Polyline: volume, hole count and area; Line Style: as Line; selected-edge visibility |
| Box Specification | 348-349 | General (Box Style Normal / Cross / Insulation, position, angle, size), Line Style, Fill Style | polyline dialog (no box object) | General (box) | box style, center, angle, height, width |
| New Regular Polygon | 347 | side length, radius to corner, radius to side; number of sides | sides count in the option strip; center and vertex clicks | dialog | all size modes |
| CAD Block Specification / Management | 361-364 | General (name, position, angle, By Block Layer / By Object, size with factors and aspect lock, insertion point offset, copyright), Line Style, Fill Style, Label; Management: list with Used column, Insert, Edit, Rename, Purge, Delete, Add to Library, auto-purge, preview | dialogs/cad/blocks.rs: General, Insertion Point, Arrow Backoff; Management list with insert, edit, rename, delete | Line Style, Fill Style, Label | General: By Block/By Object, size factors, aspect lock, copyright; Management: Used marks, Purge, Add to Library, preview, auto-purge |
| CAD Detail Management / Specification | 365-367 | Management: New, Rename, Duplicate, Delete, Open; Specification: General (name, Remember Zoom/Rotation, Show Color, Show Watermark), Selected Defaults | dialogs/details/management.rs (New, Rename, Duplicate, Delete, Open, Send to Layout) | Specification dialog | Remember Zoom/Rotation, Show Color, Show Watermark, Selected Defaults |
| Plan Footprint Specification | 368-369 | General (floor, Display Footprint Polyline, Display Plan Details, Use Current Layer Set), Polyline, Line Style, Fill Style, Label | none (a CAD polyline from Plan Footprint) | entire dialog | all |
| Convert CAD to Walls | 413-414 | Set Layers (wall, window, door, rail source layers, Define), Set Wall Types (two types, closest width) | dialogs/exchange.rs CAD to Walls options (layer and wall type picks) | not compared field by field | window, door and rail layer mapping and closest-width typing not verified |
| General Wall Defaults | 370-371 | Resize About (5), Automatic Walls (Auto Rebuild Attic Walls, Auto Reverse Wall Layers, Auto Merge Collinear Walls) | none (per-wall ResizeAbout; Preferences > Architectural has Auto Rebuild Attic Walls) | entire dialog | Resize About default, Auto Reverse Wall Layers, Auto Merge Collinear Walls |
| Wall Defaults dialogs (13) | 371-373 | Exterior, Interior, Foundation, Slab Footing, Pony, Railing, Deck Railing, Glass, Glass Pony, Half-Wall, Fencing, Room Divider, Wall Hatching; each with the Wall Specification (or Hatching) panels | Exterior, Interior, Foundation = Wall Specification dialogs; Railing, Fence, Pony, Half, Glass, Attic = short pages (type, height); Wall Hatching none | Slab Footing, Deck Railing, Glass Pony, Room Divider, Wall Hatching defaults | every panel beyond type and height on the short pages |
| Wall Type Definitions | 416-419 | Wall Types list (New, Copy, Rename, Delete, Delete All Unused), Wall Layers table (Pattern/Texture, Fill, Thickness, Extension; Insert Above/Below, Move, Delete, Edit Layer, Total Thickness, Settings), Energy Values, Wall Settings (7 choices + Brick Ledge Depth, Foundation Offset, Partition Wall), Preview (4 techniques, explode, line weights) | dialogs/wall_types.rs: layer table (name, thickness, material, main flag), Insert/Delete/Move, Resize About, stack preview | Energy Values, Wall Settings | Fill and Extension columns, Edit Layer, Total Thickness edit, Delete All Unused, multiple Main Layers, Brick Ledge Depth, platform / dimension / foundation / roof layer choices, partition flag, preview techniques |
| Wall Layer Specification | 419-422 | General (Layer Options, Wall Layer Role, Framing Assembly, 3D Cladding), Line Style, Fill Style, Materials | none | entire dialog | all |
| Wall Specification | 422-441 | General, Structure, Roof, Foundation, Wall Types, Wall Cap, Wall Covering, Rail Style*, Newels/Balusters*, Rails*, Layer, Materials, Label, Components, Object Information, Schedule (*railings and fencing) | dialogs/wall.rs WALL_TABS: all 16 tab names present; Round 15 walls2 fills Wall Covering, Newels/Balusters, Rails, Materials, Components, Object Information, Schedule (dialogs/wall/tabs.rs) | none by name (in progress) | General: Terrain Retaining Wall, Attic Wall, Automatic Facet Angle (dimmed). Structure: Use Framing Reference, Reverse Stud Rollout, Retain Wall Framing, Stagger Multiple Framing Layers, Bearing Wall, Create Wall/Footing Below, Insert Floor Framing Below, Rim Joists, Double Wall options (dimmed). Roof: Include Frieze, Include Automatic End Truss Above, Combine with Above Wall, Lower Wall Type if Split by Butting Roof, Treat As Part Of Bay, return type and trim. Foundation: footing Fill Style and Layer; sill plate Width, Height, Max Length and Count (the tab has one Construction choice instead). Wall Types: Height Off Floor, Align Pony Walls at, six-way Display in Plan View, opening display. Wall Cap: profile table. Layer: Drawing Group. Label: text style, alignment, border, label layer |
| Wall Hatching Specification | 441 | Layer, Fill Style, Label | dialogs/details.rs HATCH_TABS: General (pattern, scale, angle), Line Style, Layer | Fill Style (as a panel), Label | fill style selection from the fill library, label |
| Fix Off Angle Wall / Radius of Tangent Curved Wall | 386, 403-404 | Off angle: old angle, new angle, lock start / center / end. Tangent: wall radius, measured-to layer | none (Make Arc Tangent runs directly) | both dialogs | all |
| Room Types / Room Type Defaults / Room Label Defaults | 444-445 | Room Types list (Edit, Copy, Rename, Delete, Select All, Clear All); Type Defaults panels (General, Structure floor/ceiling, Deck, Deck Support, Moldings, Layer, Fill Style, Components...); Label Defaults (Label, Dimension Format) | dialogs/default_lists.rs RoomTypesDialog (name, function, living, conditioned, default floor finish); Room Label page (stored) | Type Defaults panels, Label Defaults | ceiling and floor structure and finish, deck framing, layer and drawing group, fill, moldings, components per type; label text style, border, dimension format |
| Room Specification | 464-475 | General, Structure, Deck, Deck Support, Moldings, Wall Covering, Layer, Fill Style, Materials, Label, Components, Object Information, Schedule | dialogs/room.rs ROOM_TABS: General, Structure, Deck, Moldings, Wall Covering, Fill Style, Materials, Label, Components, Object Information, Schedule (Deck Support folded into Deck) | Layer; Deck Support as its own panel | General: Roof Group, Function editable. Structure: see Room Structure fields. Deck: Max Plank Length, No Border Against Walls, Herringbone, Treated, automatic regenerate and keep switches. Deck Support: framing member definitions, ply count, with/under joists, flush, alignment, offset from end, round footings, rebar. Moldings: molding table with Add/Replace. Wall Covering: library covering objects |
| Exterior Room Specification | 449 | room panels for exterior wall coverings and materials | none | entire dialog | all |
| Tray Ceiling Specification | 458-460 | General, Polyline, Moldings, Rope Lights, Line Style, Fill Style, Materials, Label, Components | none | entire dialog | all |
| Make Room Molding Polyline | 463 | Convert Molding drop-down (or Blank Molding), Height | none | dialog | all |
| Dimension Defaults | 478-493 | 17 panels: General, Setup Automatic, Setup Temporary, Locate Manual, Locate End to End, Locate Centerline, Locate Interior, Locate Auto Exterior, Locate Auto Room, Locate Auto Elevation, Locate Elevations, Primary Format, Secondary Format, Extensions, Layer, Arrow, Text Style | dialogs/default_lists.rs DIM_TABS: Primary Format, Setup Automatic, Extensions, Arrow, Text Style, Locate Objects (merged); Round 15 adds eight kind pages under Default Settings > Dimension (default_pages/dimension.rs) | General, Setup Temporary, 8 separate Locate panels (merged), Secondary Format, Layer | General: rounding method and indicators, text position choices, leader arrow and second segment, 3D display, Snap Line Separation (layout). Setup Automatic: first line offset from, minimum area, 3D height, vertical labels, overall/inner switches, auto refresh, allow duplicates, line position, elevation sides. Setup Temporary: row limit, reach, wall side options, inside-CAD/terrain options. Locate: per-tool panels and the CAD, solids, framing, electrical, plant, newel, ledge-line and step marks. Primary Format: leading zeros, thousands separator, display as inches, reduce method, angular format. Extensions: Auto Mark Centerlines, Same Angle, Offset From Extension |
| Auto Story Pole Dimension Defaults | 493-496 | General, Inner Format, Outer Format, Marker Format, Locate Objects, Locate Elevations, Layer | Default Settings > Dimension > Story Pole page (4 invented fields) | all seven panels | all of Chief's fields (sides, reach, separation, first offset, ridge and height mark switches, three formats, locate lists, elevation mark lists) |
| Dimension Line Specification | 513-520 | Dimension, Primary Format, Secondary Format, Marker Format, Extensions/Markers, Segments, Layer, Arrow (Angular: Dimension only) | dialogs/dimension.rs DIM_TABS: General, Primary Format, Arrow, Text Style, Layer, Label | Secondary Format, Marker Format, Extensions/Markers, Segments | Dimension: saved-default choice, Display Wall Widths, cabinet gap display, rounded indicators, text position. Extensions: table, per-extension length, marker style. Segments: blank, leading/trailing text, suppress value, angle, leader |
| Additional Text / Set Angular Dimension | 510, 513 | Additional Text: leading, trailing, suppress value. Set Angular Dimension: previous value, new value, rotate edge or entire polyline | none (text override on the Label tab) | both dialogs | all |

## Gaps to build

Ranked by how much a residential designer producing construction documents (custom homes, remodels, layout sheets) depends on the feature. Size: S (days), M (about a week), L (more than a week). Items marked in progress are already assigned to a Round 15 brief.

| Rank | Gap | Why it matters | Size | Parity file / ids |
|---|---|---|---|---|
| 1 | Wall Type Definitions depth: per-layer fill/pattern columns, multiple main layers, Dimension Layer, platform and foundation alignment layers, Brick Ledge Depth, Delete All Unused | Wall assemblies drive plan poche, dimensions and framing on every set; today plan fills come from wall kind only | L | walls.md W-46..W-49 plus new rows |
| 2 | Show Length / Show Angle on CAD lines with bearing input and quadrant/azimuth display (CAD Defaults line formats) | Site and plot plans for every lot need bearings and distances; the survey-style entry is missing | M | dimensions-text-cad.md CAD rows (line format, bearings, show length) |
| 3 | Dimension Line Specification and handles: Segments panel with leading/trailing text, label move/rotate, Add Segments, extension editing, Fixed Proximity, Mark as Centerline | Edited dimension text and leaders appear on every set; strings are separate objects today | L | dimensions-text-cad.md DIM-30, DIM-31, DIM-41 and new rows |
| 4 | Dimension Defaults panel structure: per-tool Locate panels, Setup Temporary, General (rounding, text position, leader), Secondary Format, Layer | Dimension setup is done once per office standard; one merged tab hides the per-tool differences | M | dimensions-text-cad.md DIM-6, DIM-40, DIM-46 |
| 5 | Auto Elevation and Auto Story Pole dimensions with elevation markers and a grade reference | Exterior elevations and sections need height strings | M (in progress) | dimensions-text-cad.md DIM-28 (dims2) |
| 6 | Wall edit tools: Edit Wall Intersections, Auto Connect lock, Align With Wall Above/Below, Connect Walls, Make Invisible/Visible, Same Wall Type handles, Set as Default | Daily wall cleanup between floors and at odd corners | M | walls.md new rows |
| 7 | Off-angle and unconnected wall caution symbols with Fix Off Angle dialog and Reset Notification Icons | Catches bad geometry before rooms or roofs fail | M | walls.md new rows |
| 8 | General Wall Defaults dialog (Resize About, Auto Reverse Layers, Auto Merge Collinear) and the missing Wall Defaults dialogs | Office standards for wall behaviour | S | walls.md W-1, W-26 plus new rows |
| 9 | Exterior Room object with Living Area label per structure and Make Living Area / Room Polyline | Living area labels and footprint areas go on every permit set | M | rooms-floors.md R-51, R-52 and new rows |
| 10 | Room Type Defaults dialog with the full Room Specification panels per type | Room types are how finishes, moldings and decks are set once | M | rooms-floors.md R-37, R-38 |
| 11 | Tray and coffered ceiling polylines with specification and framing | Common feature in custom homes | L | rooms-floors.md new rows |
| 12 | Room function set (Balcony, Court, Slab, Attic) and function-driven behaviour | Porches, garages and slabs behave through functions | S | rooms-floors.md R-40 and new rows |
| 13 | Deck Specification fidelity: Deck Support panel, plank options, auto-regenerate, keep-after-delete, deck material takeoff | Decks are a frequent remodel add-on | M (in progress) | cabinets-stairs-framing-terrain-library.md CB-86 |
| 14 | Stepped and raked walls, edge breaks and height handles in sections and 3D | Vaulted and sloped walls, stepped foundations | L | walls.md new rows, rooms-floors.md R-88 |
| 15 | Revision Cloud specification, saved defaults and click-an-object clouds | Revision clouds go on every reissued set | S | dimensions-text-cad.md CAD-36 and new rows |
| 16 | Wall roof directive extras: Include Frieze, auto end truss above, combine with above wall, lower type under butting roof, bay roof options, roof return options | Roof-to-wall detail on custom homes | M | roofs.md RF-18..RF-28 and walls.md new rows |
| 17 | Railing Rail Style / Newels / Balusters / Rails fields, Move Newels, glass panel above half wall | Stair and deck guard design | M (in progress) | walls.md W-116, W-117 (walls2, stairs2) |
| 18 | Polygon Shaped Room and the regular polygon dialog; box and oval/ellipse as true objects with specification dialogs | Bays, octagons and towers | S | walls.md and dimensions-text-cad.md new rows |
| 19 | Plan Footprint object with specification and CAD detail insertion | Plot plan building outline | S | dimensions-text-cad.md CAD-35, L-38 |
| 20 | CAD block specification (By Block/By Object, size factors), Used marks, Purge, library blocks with layers | Detail library reuse | M | dimensions-text-cad.md CAD-31..CAD-34 |
| 21 | Calculate Materials in Room, Create Schedule from Room, Create Room Elevation Views buttons | Quantity takeoff by room | M | rooms-floors.md new rows |
| 22 | Point tools: Current Point, Move Point dialog, New CAD Point/Line/Arc dialogs | Precise site layout | M | dimensions-text-cad.md CAD-2, CAD-5, CAD-8 |
| 23 | Wall Layer Specification dialog (role, framing assembly, cladding) | Per-layer framing and cladding control | L | walls.md new rows |
| 24 | Wall hatching: partial length, overlap warnings, Fill Style and Label tabs, Wall Hatching defaults | Plan hatching for rated walls | S | walls.md W-59 |
| 25 | Polyline holes, Hide/Show Selected Edge, Disconnect Edges, Same Line Type handles (edit rules live in part 1) | CAD detail drawing | M | dimensions-text-cad.md new rows |
| 26 | Double walls (Frame Through, Split Framing, Furred Wall) | Modular and party-wall conditions | M | walls.md new rows |
| 27 | Brick ledges under masonry veneer | Brick veneer foundations in Georgia | M | walls.md new rows |
| 28 | Dimension rounding method and rounded-value indicators | Accuracy marks avoid field confusion | S | dimensions-text-cad.md new rows |
| 29 | Dimensions in camera views and overviews | Annotated 3D views | L | dimensions-text-cad.md new rows |
| 30 | Library walls and Add to Library for walls | Reuse of wall assemblies across jobs | S | walls.md W-47 |
| 31 | Partition Wall type flag for glass shower walls | Bath design | S | walls.md new rows |
| 32 | Dimension Line Separation Snaps preference and Delete Dimensions tool | Small drafting conveniences | S | dimensions-text-cad.md new rows |
| 33 | Auto Reverse Wall Layers / interior-exterior by position | Auto-correct siding facing | M | walls.md W-22 |
| 34 | Hide/Show wall system layers (Walls, Layers; Main Layer Only; Through Wall Lines; Footings; Brick Ledge Lines; No Locate; Attic) | Plan views for framing vs finished plan | M | dimensions-text-cad.md LAY rows |
| 35 | Point to Point dimension point markers; Set Angular Dimension; Move Left/Right End buttons | Dimension editing polish | S | dimensions-text-cad.md new rows |

## Where this audit disagrees with DECISIONS.md or with earlier audits

- **DECISIONS 13** (Edit Behaviors, Alternate built as a move along the dominant axis only): the manual (pp. 329, 401, 403, 498) uses Alternate as the other mode of each tool: continuous drawing for lines and arcs, moving a wall at allowed angles instead of perpendicular only, overriding a locked arc center, and locating objects along the length of a Point to Point dimension.
- **DECISIONS 47** (minimum wall thickness = fixed layers plus 1/8 in): the manual (p. 399) allows a main layer as thin as 1/16 in and a total no less than the old total minus the main layer.
- **DECISIONS 76** (Arc Creation Modes, four modes: Three-Point, Center-Start-End, Start-End-Radius, Tangent): the manual (pp. 335-336) lists five: Free Form, Center/Radius/End, Start/End/On, Start/Tangent/End and Arc About Center, and says the last mode is remembered between sessions. Ours splits Start/End/On into two modes, folds the two center modes into one and has no Free Form; the Tangent mode takes its tangent as a click, not a drag.
- **DECISIONS 300** (Basement and Crawl Space as room functions, Crawl Space with function Utility): the manual (pp. 446-447) has no such functions; stairwells and crawl spaces use the Open Below function, the hybrid functions are Attic, Garage, Open Below, Porch and Slab and the exterior ones Balcony, Court and Deck.
- **DECISIONS 310** (Room Types: check boxes, Select All / Clear All tick every type, Delete removes the checked): the manual (pp. 444-445) selects list rows; Select All and Clear All select and deselect rows.
- **DECISIONS 81 and 82** (a dimension string is several two-point dimension objects that share a line): the manual treats one dimension line with N extension lines and N-1 segments as one object (pp. 501-505, 518-520); Move, Rotate and the Segments panel act on the whole line. This is a structural difference, not a contradiction, and the dims2 brief asks for the string-as-one-object option.
- **Drawing group numbers**: the manual (p. 354) puts CAD objects in Drawing Group 21; `plan-core/src/drawing_group.rs` (Round 15) starts CAD and Text at 80 and uses its own 15-kind table.
- **Default Settings pages for CAD and Dimension (default_pages/cad.rs, dimension.rs)**: they invent fields (Pole Width, Tick Length, Work Aisle, Show Control Points...) that Chief's dialogs do not have, and omit Chief's real ones (Displayed Line Length, line angle format, Auto Story Pole sides and reach). The CAD pages are stored only; no tool reads them (DS-21).
- **docs/chief-feature-coverage.md Wall Specification table** is stale: it lists Wall Covering, Newels/Balusters, Rails, Materials, Components, Object Information and Schedule as disabled; the Round 15 walls2 builder has pages for them in the working tree (`dialogs/wall/tabs.rs`).

## New parity rows appended by this audit

| Id | Feature | Status | Parity file |
|---|---|---|---|
| CAD-107 | CAD Defaults dialog (per view) | Missing | dimensions-text-cad.md |
| CAD-108 | Displayed Line Length format | Missing | dimensions-text-cad.md |
| CAD-109 | Display Line Angles As (degrees, quadrant bearings, azimuth bearings) | Missing | dimensions-text-cad.md |
| CAD-110 | Revision Cloud Defaults and multiple saved defaults | Missing | dimensions-text-cad.md |
| CAD-111 | Polyline Specification panels (Polyline report, Spline, Selected Line/Arc, Visibility, Label, Components, Object Information, Schedule) | Partial | dimensions-text-cad.md |
| CAD-112 | New CAD Point dialog (absolute, relative, polar, Next) | Partial | dimensions-text-cad.md |
| DIM-47 | Point to Point Dimension makes point markers | Missing | dimensions-text-cad.md |
| CAD-113 | The Current CAD Point | Missing | dimensions-text-cad.md |
| CAD-114 | Move Point dialog | Missing | dimensions-text-cad.md |
| CAD-115 | Arrow attach, auto-position and arrow line style | Partial | dimensions-text-cad.md |
| CAD-116 | Line and Arc Specification lock modes | Missing | dimensions-text-cad.md |
| CAD-117 | Hide and Show Selected Edge | Missing | dimensions-text-cad.md |
| CAD-118 | Show Length, Show Angle, All Angles, Reverse Angle | Missing | dimensions-text-cad.md |
| CAD-119 | Free Form arc creation mode | Missing | dimensions-text-cad.md |
| CAD-120 | Oval and Ellipse as true objects with a specification dialog | Partial | dimensions-text-cad.md |
| CAD-121 | Same Line Type edit handles on CAD lines, arcs, open polylines and splines | Missing | dimensions-text-cad.md |
| CAD-122 | Spline as an editable spline object | Partial | dimensions-text-cad.md |
| CAD-123 | New Regular Polygon dialog | Partial | dimensions-text-cad.md |
| CAD-124 | Box Specification dialog (Normal, Cross, Insulation) | Partial | dimensions-text-cad.md |
| CAD-125 | Revision Cloud creation shortcuts | Partial | dimensions-text-cad.md |
| CAD-126 | Revision Cloud Specification (Cloud panel) | Missing | dimensions-text-cad.md |
| CAD-127 | Custom arrowheads from CAD blocks with a Backoff Point | Partial | dimensions-text-cad.md |
| CAD-128 | CAD Block window with save and update-all/this-instance choice | Partial | dimensions-text-cad.md |
| CAD-129 | Library CAD blocks carry layers, saved dimension defaults and text styles | Missing | dimensions-text-cad.md |
| CAD-130 | CAD Block Management (Used marks, Purge, Insert, Rename, Add to Library, auto-purge) | Partial | dimensions-text-cad.md |
| CAD-131 | CAD Block Specification (instance and definition) | Partial | dimensions-text-cad.md |
| CAD-132 | Automatic Truss Detail and temporary pattern/molding/skylight detail windows | Missing | dimensions-text-cad.md |
| CAD-133 | CAD Detail Specification (Remember Zoom/Rotation, Show Color, Watermark, Selected Defaults) | Partial | dimensions-text-cad.md |
| CAD-134 | Plot plan entry by polar and bearing input | Partial | dimensions-text-cad.md |
| CAD-135 | Plan Footprint Specification (floor, polyline, plan details, layer set) | Partial | dimensions-text-cad.md |
| W-119 | Wall Defaults dialogs share the Wall Specification panels | Partial | walls.md |
| W-120 | General Wall Defaults dialog | Missing | walls.md |
| W-121 | Set as Default edit button for a wall | Missing | walls.md |
| W-122 | Partition Wall wall types (glass shower walls) | Missing | walls.md |
| W-123 | Polygon Shaped Room tool | Missing | walls.md |
| W-124 | Wall Hatching placement and editing | Partial | walls.md |
| W-125 | Walls, railings and fencing in the Library Browser | Missing | walls.md |
| W-126 | Polygon Shaped Deck dialog and Include Railing switch | Partial | walls.md |
| W-127 | Rail Style panel: styles, end posts, build-from | In progress (Round 15) | walls.md |
| W-128 | Exterior vs interior wall by position, Primary and Secondary sides | Differs | walls.md |
| W-129 | Brick ledges | Missing | walls.md |
| W-130 | Make Wall(s) Invisible / Visible edit buttons | Missing | walls.md |
| W-131 | Auto Reverse Wall Layers | Missing | walls.md |
| W-132 | Wall caution symbols and notification icons | Missing | walls.md |
| W-133 | Fix Off Angle Wall dialog and Allowed Angles list | Missing | walls.md |
| W-134 | Wall framing options on the Structure tab | Partial | walls.md |
| W-135 | Auto Connect (magnet) lock on a wall end | Missing | walls.md |
| LAY-76 | Wall system layers for plan display | Partial | dimensions-text-cad.md |
| W-136 | Connect Walls edit tool | Missing | walls.md |
| W-137 | Wall Type Definitions dialog detail | Partial | walls.md |
| W-138 | Wall Layer Specification dialog | Missing | walls.md |
| DIM-48 | Wall resize buttons in the dimension edit field | Missing | dimensions-text-cad.md |
| W-139 | Wall, room and floor heights by dragging edges in 3D and sections | Missing | walls.md |
| W-140 | Move Newels, Add Newel, Delete Newel, Reset Newels | Missing | walls.md |
| W-141 | Stepped and raked walls and footings | Missing | walls.md |
| W-142 | Cabinets attached to a wall move with it | Missing | walls.md |
| W-143 | Same Wall Type edit handles | Missing | walls.md |
| W-144 | Edit Wall Intersections (per-layer corner edit) and Reset Wall Layer Intersections | Missing | walls.md |
| W-145 | Align With Wall Above / Below | Missing | walls.md |
| W-146 | Railing on different platforms (Generate on Low Platform, No Room Def) | Missing | walls.md |
| W-147 | Auto Roof Return options | Partial | walls.md |
| W-148 | Wall Roof tab extras | Missing | walls.md |
| W-149 | Double walls: Frame Through, Split Framing, Furred Wall | Missing | walls.md |
| R-98 | Rooms split by an invisible wall break at the wall's centerline or lower/higher edge | Partial | rooms-floors.md |
| R-99 | Room Type Defaults dialog panels | Partial | rooms-floors.md |
| R-100 | Room Label Defaults dialog (Label and Dimension Format panels) | Partial | rooms-floors.md |
| R-101 | Room Function set (Interior, Exterior, Hybrid) and what each function does | Partial | rooms-floors.md |
| R-102 | Room function drives door and window exterior/interior behaviour | Differs | rooms-floors.md |
| R-103 | Layer panel in the Room Specification (layer and Drawing Group) | Missing | rooms-floors.md |
| R-104 | Calculate Materials in Room / From Selection | Missing | rooms-floors.md |
| R-105 | Create Schedule from Room | Missing | rooms-floors.md |
| R-106 | Exterior Room object and Exterior Room Specification | Missing | rooms-floors.md |
| R-107 | Room Edit toolbar buttons | Missing | rooms-floors.md |
| R-108 | Standard Area boundary choice and rounding | Partial | rooms-floors.md |
| R-109 | Living Area label per structure and its rules | Missing | rooms-floors.md |
| R-110 | Room polylines (room, standard area, living area, molding) | Missing | rooms-floors.md |
| R-111 | Tray and coffered ceilings (Tray Ceiling Polyline tool) | Missing | rooms-floors.md |
| R-112 | Tray Ceiling Specification dialog | Missing | rooms-floors.md |
| R-113 | Room moldings suppressed behind cabinets and per wall | Partial | rooms-floors.md |
| R-114 | Roof Group on the room | Missing | rooms-floors.md |
| R-115 | Room Structure panel fields not yet present | Partial | rooms-floors.md |
| DIM-49 | Dimension Line Separation Snaps preference | Missing | dimensions-text-cad.md |
| DIM-50 | Rounding method and rounded-value indicators | Missing | dimensions-text-cad.md |
| DIM-51 | Dimension Defaults General panel | Partial | dimensions-text-cad.md |
| DIM-52 | Setup Automatic panel (exterior, room, elevation options) | Partial | dimensions-text-cad.md |
| DIM-53 | Setup Temporary panel | Partial | dimensions-text-cad.md |
| DIM-54 | Locate panels (Manual, End to End, Centerline, Interior, Auto Exterior, Auto Room, Auto Elevation, Elevations) | Partial | dimensions-text-cad.md |
| DIM-55 | Secondary Format panel and tolerance | In progress (Round 15) | dimensions-text-cad.md |
| DIM-56 | Extensions panel | Partial | dimensions-text-cad.md |
| DIM-57 | Dimension Defaults Layer panel | Partial | dimensions-text-cad.md |
| DIM-58 | Auto Story Pole Dimension Defaults dialog | In progress (Round 15) | dimensions-text-cad.md |
| DIM-59 | Editing extension lines | Partial | dimensions-text-cad.md |
| DIM-60 | Delete Dimensions tool and Auto Refresh prompt | Partial | dimensions-text-cad.md |
| DIM-61 | Auto Elevation and Auto Story Pole Dimensions in section and elevation views | In progress (Round 15) | dimensions-text-cad.md |
| DIM-62 | Dimension lines in camera views and overviews | Missing | dimensions-text-cad.md |
| DIM-63 | Dimension line edit handles | Partial | dimensions-text-cad.md |
| DIM-64 | Elevation markers on vertical dimension extensions | In progress (Round 15) | dimensions-text-cad.md |
| DIM-65 | Additional Text and the Segments panel | Partial | dimensions-text-cad.md |
| DIM-66 | Move Edge versus Move Entire Object buttons when editing a value | Partial | dimensions-text-cad.md |
| DIM-67 | Set Angular Dimension dialog | Missing | dimensions-text-cad.md |
| DIM-68 | Dimension Line Specification panels | Partial | dimensions-text-cad.md |
| DIM-69 | Dimensions inside CAD blocks and CAD Details | Missing | dimensions-text-cad.md |
| DIM-70 | Dimension export to DXF/DWG keeps real dimension entities | Differs | dimensions-text-cad.md |

