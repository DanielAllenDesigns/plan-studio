# Chief Architect X18 Tutorial Guide: workflow parity (part 7, Tutorial Guide pages 1-517)

Written 2026-10-08 by the manual audit (part 7). Source: Chief Architect's Tutorial Guide (517 pages), read in full from `~/plan-studio-dev/chief-docs` with `pages.py tut`. Every row is in my own words; only tool, dialog, panel, field and layer names are Chief's. Nothing from the guide's text is stored in the repo.

**What this part is.** Parts 1 to 6 audited the Reference Manual feature by feature. The Tutorial Guide shows how Chief expects a designer to work, so this part replays each lesson step by step against Plan Studio's code (branch `wip/round-14-partial`, read only, about 25 Round 15 builders still editing), marks where our workflow breaks or differs, and lists the guide's assessment questions as feature checks. It follows the guide's own running example (the "Chic Cottage" project, 28 lessons: floor plans, roofs, interior design, kitchen and bath, framing, landscaping, layout). Statuses: **Works**, **Partial**, **Missing**, **Differs** (by design or by Daniel's template), **Out-of-scope** (cloud content, knowledge-base links), **In progress (Round 15)** (a brief in `~/plan-studio-dev/briefs/r15` covers it).

**How to read the tables.** Each lesson has a step table (page, step in tool / dialog / field order, status, evidence or break) and an assessment table (each question of the guide's quiz as a feature check). Evidence cites parity ids from `docs/parity-status.md` (checked to exist) and code. "see W-150" style references point to the rows this audit added (W-150..W-155, R-143..R-146, RF-164..RF-166, DIM-71, TXT-65, APP-145, CB-627..CB-649, L-229..L-236, S-194..S-197, E-30..E-33, DW-168; 56 rows, appended under "Manual audit additions (part 7)" in each `docs/parity/*.md` and to `docs/parity-status.md`). "Not confirmed in code" means I did not read the exact code path; treat that row as unsettled, not as Works.

## Counts

| Measure | Count |
|---|---|
| Lessons replayed | 28 (pages 3 to 500; pages 501 to 517 are the answer key, already folded into each lesson's quiz) |
| Workflow steps enumerated | 1169 |
| Works | 455 |
| Partial | 421 |
| Missing | 265 |
| Differs | 11 |
| In progress (Round 15) | 12 |
| Out-of-scope | 5 |
| Assessment questions checked as features | 198 (Works 72, Partial 75, Missing 46, Differs 2, In progress 2, Out-of-scope 1) |
| Steps with a gap that had no parity row when first checked | 136 (56 new parity rows cover those no other part had added; the rest cite rows the other parts appended meanwhile) |
| Rows marked "not confirmed in code" | about 100 (statuses there are my best reading) |

| Lesson | Pages | Steps | Works | Partial | Missing | Other |
|---|---|---|---|---|---|---|
| 1 Exterior Walls | 3-27 | 56 | 29 | 17 | 8 | 2 |
| 2 Interior Walls | 28-44 | 53 | 29 | 15 | 8 | 1 |
| 3 Multiple Floors | 45-59 | 46 | 18 | 14 | 14 | 0 |
| 4 Interior Stairs | 60-81 | 61 | 24 | 21 | 16 | 0 |
| 5 Doors and Windows | 82-105 | 66 | 33 | 23 | 7 | 3 |
| 6 Decks and Porches | 105-126 | 58 | 17 | 28 | 13 | 0 |
| 7 Basic Roof Styles | 127-135 | 21 | 11 | 7 | 2 | 1 |
| 8 Chic Cottage Roof | 136-158 | 41 | 19 | 16 | 5 | 1 |
| 9 Dormers | 159-176 | 45 | 25 | 14 | 5 | 1 |
| 10 Custom Ceilings | 177-190 | 28 | 7 | 9 | 12 | 0 |
| 11 Finish Materials | 191-211 | 48 | 20 | 15 | 8 | 5 |
| 12 Room Moldings | 211-222 | 26 | 6 | 9 | 10 | 1 |
| 13 Interior Furnishings | 222-236 | 41 | 11 | 17 | 11 | 2 |
| 14 Cabinet Styles | 237-260 | 50 | 16 | 19 | 13 | 2 |
| 15 Cabinet Layout | 260-277 | 48 | 26 | 14 | 8 | 0 |
| 16 Appliances and Fixtures | 277-298 | 66 | 19 | 30 | 17 | 0 |
| 17 Light Fixtures | 298-317 | 38 | 12 | 15 | 8 | 3 |
| 18 Electrical Objects | 317-331 | 26 | 12 | 9 | 3 | 2 |
| 19 Floor Framing | 331-349 | 38 | 14 | 14 | 10 | 0 |
| 20 Wall Framing | 349-364 | 36 | 11 | 12 | 12 | 1 |
| 21 Roof and Ceiling Framing | 364-382 | 43 | 18 | 15 | 10 | 0 |
| 22 Plot Plans | 382-403 | 43 | 14 | 14 | 15 | 0 |
| 23 Terrain Elevation | 403-420 | 36 | 18 | 11 | 7 | 0 |
| 24 Driveways, Sidewalks, Roads | 420-433 | 25 | 9 | 7 | 8 | 1 |
| 25 Landscaping Design | 433-450 | 34 | 13 | 11 | 9 | 1 |
| 26 Layout Page Templates | 451-460 | 16 | 2 | 10 | 4 | 0 |
| 27 Title Blocks and Borders | 460-483 | 37 | 6 | 21 | 10 | 0 |
| 28 Sending Views to Layout | 483-500 | 43 | 16 | 14 | 12 | 1 |

Lesson numbers match the guide's own Tutorial numbers (1 to 28); "Other" counts Differs, In progress and Out-of-scope steps.

## What the tutorials reveal

Three things stand out beyond the single-step gaps.

1. **The guide's world is Projects, Saved Plan Views and Default Sets.** Every lesson starts by opening a named view ("Working Plan View", "Roof Plan View", "Kitchen & Bath Plan View", "Electrical Plan View", "Framing, Floor Plan View", "Plot Plan View") whose layer set and saved defaults put annotations on the right layers, and every lesson ends with File > Make a Copy into the project and Close All Views. Plan Studio has plan views with layer sets (LAY-49, LAY-8) and Save a Copy / archives (APP-96), but no Default Sets, no per-view Saved Defaults (DS-28, DS-30, DS-32), no starter views in the template and no project. Any lesson can be followed by hand, but the guide's habit of "switch the view and the right layers and text defaults follow" does not exist.
2. **Daniel's template is not Chief's Residential Template.** The tutorials assume a template that builds the roof and the exterior dimensions the moment a room closes (pp. 6, 15) and ships Siding-6, Fire-6, Room Divider and Interior Railing wall types, saved views and layer sets. Ours starts from Daniel's decoded Chief template (`chief-x18-daniel.json`: Stucco-6, 109 1/8 in walls, Interior-4/6, `stone-6`), so wall-type names and default numbers differ (table below).
3. **The weakest area for a construction-document practice is the last chapter.** The layout lessons assume a Label with `#` numbering (A0.1, A1.2, E1.1), several named page templates assigned per page, site-plan scales to 1 in = 100 ft, and layout CAD with Center Object, Point to Point Move and concentric copies. Plan Studio's layout numbers pages A-1, A-2..., has one template flag and a fixed 1/16 in snap (L-188, L-190, L-229, L-231). The site-plan lesson additionally needs surveyor entry (quadrant bearings, Next, Disconnect Edges, Change Line/Arc) that is entirely absent (CAD-109, CAD-134, CAD-22, S-172).

## Workflow breaks, ranked

Ranked by how much a residential designer doing custom homes, remodels and layout sheets depends on the step. Size: S = days, M = a week or two, L = a builder round. "Break" means the guide's own steps cannot be followed to the end; the file named is the parity file the fix belongs to.

| Rank | Break | Lessons / pages | Ids | Size | Parity file |
|---|---|---|---|---|---|
| 1 | Sheet numbering: no page Label with `#` prefix numbering (A0.#, A1.#, E1.#) and no per-page template choice (cover vs standard); the Page Specification refuses duplicate numbers (DECISIONS 62) | 26, 27: pp. 455-459, 473-474 | L-188, L-189, L-190, L-191, L-192 | M | documentation-layout.md |
| 2 | Site plan from a survey: no bearings (N 61 25 10 E) in Input Line or angle fields, no Current Point / Next, no Disconnect Edges, no Change Line/Arc with a Lock Chord radius, no length/bearing labels on polylines, no quadrant display; the lot cannot be entered | 22: pp. 384-399 | CAD-109, CAD-112, CAD-113, CAD-118, CAD-134, CAD-22, S-172, PR-31 | L | dimensions-text-cad.md |
| 3 | Send to Layout offers no site-plan scales (1 in = 30 / 40 / 50 / 100 ft) or typed ratio and gives no too-large warning | 28: p. 488 | L-229 | S | documentation-layout.md |
| 4 | Roof panel missing for a multi-wall selection and for Interior-kind walls (Half Wall, Railing, interior Knee Wall): the gambrel / mansard / half-hip recipes, the porch gull wing, the knee wall + attic + dormer recipe cannot be followed | 7, 8, 9: pp. 130-135, 145, 148-149, 163-164 | RF-166, RF-165, RF-18 | M | roofs.md |
| 5 | Residential-template behaviours: no roof and no Auto Exterior Dimensions when a room closes, no Auto Refresh switch; the guide's later screenshots assume them | 1, 2, 6, 8: pp. 6, 15, 30, 110 | RF-164, DIM-52, DIM-60 | M | roofs.md |
| 6 | Wall Type Definitions: no layer Fill colour, no Role (framing, air gap, standard), no library material per layer (text boxes), no Wall Layer Specification (stud spacing), no Foundation / Dimension to Exterior of Layer, no Interior vs Main layer groups; so the Stone-6 / Shingle-6 / Fire-6 / furred-basement recipes lose their point | 1, 2, 3, 20: pp. 9-11, 19, 37, 48, 51, 353 | W-137, W-138, W-150, C-87 | L | walls.md |
| 7 | Layered floor / ceiling finish and structure definitions (Platform Defaults, hat-channel lowered ceiling, tile over backerboard, I-joist vs lumber): finishes are a thickness and a name | 1, 10, 11: pp. 13, 178-183, 204-206 | R-143, R-144, R-122 | L | rooms-floors.md |
| 8 | Project model: no Project, no Make a Copy list of archives, no folders, no Dashboard pin; every lesson ends with these | all | APP-93, APP-94, APP-95, APP-96, APP-117, APP-67 | M | preferences-hotkeys-toolbars.md |
| 9 | Saved Plan Views with Selected Defaults and Default Sets (text and dimension defaults, layers, scale by task) and the dozen starter views | all from 5 on | LAY-49, DS-28, DS-30, DS-32 | L | preferences-hotkeys-toolbars.md |
| 10 | Window > Tile Vertically / Swap Views (plan beside 3D or section) used in 13 lessons | 3-12, 16, 21, 23 | APP-41, APP-40 | S | preferences-hotkeys-toolbars.md |
| 11 | Dimensions and CAD in section / elevation views (headroom checks, stair section notes, exterior elevation annotation) and Clip Elevation / Clip Sides | 4, 28: pp. 70-77, 491 | DIM-62, CAD-132, C-136 | L | dimensions-text-cad.md |
| 12 | Architectural Blocks (kitchen island, furniture groups, ganged switches, plant groups, library blocks) | 13, 15, 16, 18, 25 | CB-427..CB-438, E-25 | L | cabinets-stairs-framing-terrain-library.md |
| 13 | Library modal pickers (Select Library Object / Select Material): newels, hardware, door styles, moldings, panels, feet, materials per component | 4, 5, 6, 12, 14, 15 | CB-395, C-87, C-91 | L | cabinets-stairs-framing-terrain-library.md |
| 14 | Schedules: Categories to Include tree, room scope, picture columns, wall-legend columns, Move Row and Move Up/Down in Schedule, Find Object in Plan, Create Schedule from Room, Number Formatting | 3, 4, 11, 12, 13, 16, 17 | L-233, L-234, L-235, L-63, S-118, R-105, L-72 | M | documentation-layout.md |
| 15 | Layout CAD editing for title blocks (Center Object, Point to Point Move, concentric copies, Selected Edge dimensions, object snaps, editable grid snap unit) | 27 | L-230, L-231 | M | documentation-layout.md |
| 16 | Exterior elevations sent as Plot Lines with Color Fill or as Live Views; Link Saved Plan View | 28: pp. 486, 492-493 | L-155, L-156, L-161 | L | documentation-layout.md |
| 17 | Page-linked callout labels on layout section / elevation boxes | 28: pp. 494-496 | L-232, L-145, L-150 | M | documentation-layout.md |
| 18 | Wall framing workflow: Retain Wall Framing is a disabled check box, Build Framing for Selected Object(s) missing, Wall Details have no UI, Bearing Wall is a disabled stub (joist direction by bearing wall), Build Once per floor missing | 19, 20, 21 | CB-261, CB-256, CB-645, CB-638, W-134 | M | cabinets-stairs-framing-terrain-library.md |
| 19 | Stairs: Make Best Fit, Lock Top / Lock Bottom, Staircase Information read-outs, DN stairs to grade with one click, doorway cut in the deck railing | 4, 6 | CB-160, CB-162, CB-159, CB-109, CB-170 | M | cabinets-stairs-framing-terrain-library.md |
| 20 | Terrain: Absolute Elevation + Reference Point, Selected Line / Arc panels on elevation lines, road curb cuts, road Height, driveway flare, Auto Generate Sidewalks, plants distributed in a bed | 6, 23, 24, 25 | CB-530, CB-531, CB-508, CB-647, CB-628..CB-630, CB-588, CB-649 | M | cabinets-stairs-framing-terrain-library.md |
| 21 | Rich Text: Uppercase button, Insert Macro with object-referencing macros (%comment%, %description%, %nominal_size%), per-view Rich Text defaults, Print Size | 1, 5, 9, 16, 17, 21, 28 | TXT-65, TXT-57, TXT-43, DS-28 | M | dimensions-text-cad.md |
| 22 | Garage foundation options (curb and lowered stem), "S" markers, Room Supplies Floor, deck footings against terrain | 3, 6 | R-133, R-131, R-145, CB-627 | M | rooms-floors.md |
| 23 | Cathedral ceilings: no Flat Ceiling Over This Room switch; no Tray Ceiling tool | 8, 10 | R-146, R-111 | S / M | rooms-floors.md |
| 24 | Cabinets: multi-select Open Object, Auto door item, per-item shelves, Suppress Label, module-lines layer, Set as Default, stack on appliance, library door apply | 14, 15, 16 | CB-631..CB-635, DS-27 | M | cabinets-stairs-framing-terrain-library.md |
| 25 | Electrical: context-sensitive Light and Outlet tools, interior vs exterior wall-light defaults, recess Options panel, resize by width, symbols as library objects | 17, 18 | E-30..E-33, E-19 | M | electrical.md |
| 26 | Roofs: Curved Roof planes, Display On Floor Above, frieze profiles, Ceiling Break Lines | 8, 9 | RF-61, RF-96, RF-86, RF-80 | M | roofs.md |
| 27 | Moldings: Floor Defaults molding table, Exterior Room, Make Room Molding Polyline, per-edge molding | 12 | R-117, R-106, R-110, S-170 | M | rooms-floors.md |
| 28 | Arithmetic in dialog number fields; Enter Coordinates dialog | 14, 16, 17, 18 | APP-145, S-132, S-146 | S | preferences-hotkeys-toolbars.md |
| 29 | Drawing Sheet Setup per view and its Drawing Scale feeding Send to Layout | 28: p. 485 | L-199, L-200, L-204, L-207 | M | documentation-layout.md |
| 30 | Allowed Angles entry by bearing, DMS or decimal; Number Style / Angle Style dialog | 22, 23 | DS-47, PR-30, PR-31 | M | preferences-hotkeys-toolbars.md |

## Defaults that differ

"Chief" is what the tutorial text states or sets; "ours" is Daniel's embedded template (`crates/plan-app/assets/templates/chief-x18-daniel.json`) or the code default. Where the guide changes a value in the lesson I say so; the stock Residential Template's own starting value is not printed in the guide.

| Setting | Chief (tutorial) | Ours |
|---|---|---|
| New file | Project + Plan (Residential Template) + Layout (Arch D 24x36), units chosen at creation | One `.psplan` from Daniel's template; layout inside the plan; units from the template (APP-94) |
| Roof and exterior dimensions on room definition | Built automatically; Auto Rebuild Roofs / Auto Refresh on in the template (turned off for lesson 7) | Not built automatically; Auto Rebuild defaults on once a roof exists (DECISIONS 97) |
| Default exterior wall type | Siding-6 (changed to Stone-6 in lesson 1) | Stucco-6 (7 5/8 in); template also has Siding-6, Brick-6, `stone-6` |
| Interior wall type | Interior-4 (2x4 + drywall) | Interior-4 (4.5 in) and Interior-6 |
| Extra wall types the guide uses | Fire-6, Room Divider (0 in), Deck Railing/Fence, Interior Railing (3 layers), 8" Concrete Stem Wall | not present; Deck Railing-4, Railing-4, Foundation-8 |
| Pony wall | Upper Siding-6, lower Stone-6, Elevation of Lower Wall Top 20" | upper Stucco-6, lower Foundation-8, split 36" |
| Ceiling height Floor 1 | set to 97 1/8" (rough ceiling; 2nd floor default 97 1/8") | 109 1/8" (walls 109 1/8") |
| Floor structure | 12" (3/4" OSB + 2x12 lumber, after the guide's edit; I-joist before) | one 10 1/4" thickness per floor default; no layer table |
| Floor / ceiling finish | Layered Floor Finish Definition (e.g. 7/8" with underlayment), Ceiling Finish drywall + paint, 5/8" drywall in garage | thickness 3/4" floor, 5/8" ceiling, material name |
| Garage / porch structure | Garage: 4" concrete slab; Porch: concrete, ceiling and roof over | room-type defaults hold function and finish name only |
| Window default | Single Casement 30 x 54, lites 3 x 4 (as set in lesson 5), rough opening +1" | Single Casement 32 x 72, sill 24", lites 1 x 1 |
| Door defaults | Interior Door P04; exterior door with four lites over a panel | Door P04, interior 30 in, exterior 36 in; casing 3 1/2 / 3 1/4 in |
| Auto Exterior Dimension locate | Openings: Sides; walls at Wall Dimension Layer; Reach 24" in Electrical set | openings: None, walls: MainLayer, centres on; reach 0 |
| Stairs | Interior stairs 44" wide, newels 4 x 44", riser about 7 5/8" | code defaults: width 36", riser 7.5", tread 10", headroom 80" |
| Deck | Plank gap 1/4"; footings Height Above Terrain 6", Thickness 30" | plank gap 0.25"; footing height_above_grade 36", thickness 12", size 18" |
| Roof | Rafters, pitch set 12 in 12 for the cottage, overhang 6" for dormers, structure depth 11 1/4", rafter spacing 16" | hip, pitch 8 in 12, overhang 16", rafter spacing 24", depth 5 1/2" |
| Soffit | 24 x 12 x 12 at 84" (lesson 15) | 24 x 12 x 12 at 84" (same) |
| Outlet / switch heights | outlet 11 1/2", switch 48", counter outlet 43" | outlet 12", switch 48", counter outlet 44" |
| Rich text character height | 6" (plan scale 1/4"), 3" (kitchen and bath scale) | text height 6"; no per-view saved text defaults |
| Layout sheet numbering | Label A0.#, A1.#, E1.# | `A-{n}` numbers, duplicates refused |
| Layout grid snap | 1/8" Grid Snap Unit, nudge = unit | fixed 1/16", nudge 1/16" (Shift 1/4") |
| Layout drawing scale | 1 in = 1 in; plan default 1/4" = 1' | implicit 1:1; Send to Layout picks its own scale |
| Send to Layout site scale | 1 ft = 100 ft | not offered (list stops at 1" = 20') |
| Reference grid / crosshairs | grid and crosshairs on in the template | grid 12" with 1" snap; crosshairs a View toggle |

## Dialog panels the tutorials open

Panels the guide uses that Plan Studio's matching dialog lacks (an extension of the parts 1-6 panel tables to the tutorial path).

| Dialog | Panels / fields the guide uses that we lack |
|---|---|
| Wall Type Definitions | Fill and Pattern per layer, Role, Interior / Main / Exterior layer groups, Wall Properties tab (Dimension / Foundation to Exterior of Layer), Edit Layer (Wall Layer Specification: spacing, stud width), Select Material picker |
| Wall Specification | Roof tab for several walls and Interior-kind walls; Structure: Bearing Wall and Retain Wall Framing are disabled stubs; Wall Types preview rotate and plan / 3D selector |
| Floor / Ceiling Platform Defaults, Floor Structure Specification | the page itself; Framing Method, Construction, Width, Role per layer; Ceiling / Floor Finish Definition |
| Room Specification | Flat Ceiling Over This Room, Room Supplies Floor for the Room Above, Default check box beside Floor Finish; Moldings tab with Use Default and a profile table |
| Build Foundation | Garage Options (Garage Floor to Stem Wall Top, Minimum Garage Height) |
| Staircase Specification | Staircase Information read-outs, Make Best Fit, Lock Top / Bottom / Number of Treads, Open Underneath, Appliance of library newels |
| Camera (section / elevation) | Clip Elevation, Clip Sides, Clip Width, top and bottom clip elevations; Plan Display callout label |
| Schedule Specification | Categories to Include tree, Include Objects from Room, Number Formatting, picture columns, two-level sort |
| Dimension Defaults | Setup Automatic Auto Refresh; Locate Interior Display Wall Widths; Saved Defaults list with Currently Active |
| Framing Specification | Fill Style and Label (Insert Macro) panels, Raise/Lower with Apply, Flat to Inside, Lock Total Height |
| Build Framing | Build Once for a chosen floor; Stagger Blocking |
| Terrain Specification / Elevation Line | General Absolute Elevation with Retain Surface Elevation At; Selected Line and Selected Arc panels |
| Road / Driveway | Curb cut option, Height, Flare |
| Electrical Service Specification | Options panel (Distance from Wall, Cuts Floor/Ceiling/Wall, Cut and Insert Depth), Width / Height with Retain Aspect Ratio |
| Layout Page Information | Label, Description, Comments, Selected Page picker, Page Template assignment, Page Revisions table |
| Send to Layout | Link Saved Plan View, Camera View Options (Live View, Plot Lines, Color Fill), site-plan scales, too-large warning |
| Callout Label Specification (layout) | the whole dialog: page link, arrows, cross-section line |

## Contradictions with DECISIONS.md

| Row | What the tutorial says | Conflict |
|---|---|---|
| 62 | Page Specification refuses a sheet number another page already has | pp. 457-458 give several pages the same prefix with `#` numbering (A1.#); duplicates are the point |
| 97 | Auto Rebuild defaults on | p. 129 says Auto Rebuild Roofs is off by default (while p. 6 says the Residential Template builds the roof at room definition); our "on" holds only after a roof exists; unresolved wording, verify in Chief |
| 309 | Attic floor walls are interior walls with the attic flag | the Roof tab is disabled for Interior-kind walls (dialogs/wall.rs), but the guide edits knee walls and attic walls on the Roof panel (pp. 163-164) |
| 53 | Alt reaches the Alternate behaviour; Ctrl overrides snaps and movement | consistent with the guide (pp. 222, 231, 413): no conflict, noted because the guide leans on Ctrl to override restrictions |

No row contradicts the guide on a dimensional default.

## Rows added to the parity files

56 rows: W-150..W-155, R-143..R-146, RF-164..RF-166, DIM-71, TXT-65, APP-145, CB-627..CB-649, L-229..L-236, S-194..S-197, E-30..E-33, DW-168. Each is in `docs/parity/<file>.md` under "Manual audit additions (part 7)" and in `docs/parity-status.md` under "Manual audit (part 7), 2026-10-08". The totals table in `docs/parity-status.md` was not touched.

---

## Lesson 1: Exterior Walls (pp. 3-27)

Workflow: new project from a template, tour plan views, draw a wall, zoom, wall types, defaults (exterior and pony wall, floor structures, 1st floor), pony-wall perimeter, room definition, a camera and a cross section, auto exterior dimensions, moving walls by dimension and by handle, rich text, a revision copy.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 4 | File > New Project (or Dashboard): New Project from Template dialog with Project Name, Measurement Units, Plan Name, Plan Template ("Residential Template"), Layout Name, Layout Template ("Arch D 24x36") | Differs | One `.psplan` holds the plan and its layout (`Project.layout`); File > New Plan starts from the embedded Daniel template (`plan_defaults::embedded`). No project object, no unit pick at creation (APP-94, APP-93). BREAK: step 2-5 of the lesson cannot be followed literally |
| 4 | Project Browser lists Plan and Layout Templates plus the open project, its plan, layout, archives | Partial | Project Browser dock shows Floors / Plan Views / Cameras / Layout (shell/docks.rs, APP-35 Works); no templates folder and no multi-plan project node (APP-117) |
| 4 | Project Browser: expand the open plan to reach views and schedules | Works | APP-35, APP-15 |
| 5 | Edit > Snap Settings > Grid Snaps on; F10 toggles Angle Snaps; wall angle shown in 15 degree steps | Works | snap.rs, template `grid_snaps`/`angle_snaps`; F10 itself is Chief's default hotkey, ours are Daniel's (hotkeys.rs) |
| 5 | Temporary Dimensions toggle (View) assumed on | Works | `ViewFlag::TemporaryDimensions` (tools/select.rs line ~1933) |
| 5-6 | Saved Plan View Specification dialog (Edit View on "Working Plan View"): Floor, Default Set, Layer Set, Saved Defaults, Reference Floor | Partial | dialogs/plan_views.rs; Default Sets and per-view Saved Defaults are missing (LAY-49, DS-32); the "Working Plan View" and the other starter views do not ship in our template |
| 6 | Residential Template turns on Reference Grid, Crosshairs, Auto Exterior Dimensions and Auto Rebuild Roofs | Differs | Reference Grid and Crosshairs exist (LAY-22, LAY-20). Closing a room does NOT create exterior dimensions or a roof: `roof_view::auto_rebuild` only follows a roof that was built once, and Auto Exterior runs only from its tool. BREAK: the template behaviour the guide relies on from page 15 onward is absent (see RF-164) |
| 7 | Build > Wall > Straight Exterior Wall: drag right to left, length under the wall and in the status bar | Works | W-1, W-3, W-4; tools/wall.rs |
| 8 | Release at 13-14 ft and 180 degrees; do not worry about exact length | Works | W-31 auto-connect; s01_house_shell |
| 8 | Scroll wheel zoom, drag pan; Zoom tool rectangle; Window > Undo Zoom | Works | LAY-24, LAY-26 |
| 8 | Window > Fill Window Building Only, Fill Window | Works | view_commands.rs FILL_BUILDING (LAY-27 row still says Missing: stale) |
| 8 | Wall Specification > Wall Types panel: wall type name, 3D preview you can rotate, Show Plan View button | Partial | dialogs/wall.rs Wall Types tab has a type combo, Define button and a layer preview; no rotatable 3D preview and no plan/3D toggle (APP-86 Partial) |
| 9 | Wall Type drop-down switches type and the preview | Works | W-47; dialogs/wall.rs `wall_type_combo` |
| 9 | Define button opens Wall Type Definitions; layers with materials and thicknesses | Works | dialogs/wall_types.rs |
| 9 | Build > Wall > Define Wall Types, pick Siding-6, Copy button, rename in the drop-down | Partial | "New Type" copies the current type as "<name> copy n" (wall_types.rs `new_type`), so Copy works; the dialog is reachable only through Define in a Wall Specification or Wall Defaults, there is no Build > Wall > Define Wall Types menu command (menus.rs has no wall-types entry); see W-151 (menu command) |
| 9 | Edit a layer's Thickness (type 3", Tab) | Works | wall_types.rs DragValue (typed values accept feet-inch text through egui only as decimals: no `3"` parsing, minor) |
| 9-10 | Click Material cell: Select Material dialog with a Library Materials panel (filters, search, folders) | Missing | the Material cell is a free text box (wall_types.rs); no material picker; see C-87 (see W-137 for columns) |
| 10 | Fill cell: Layer Fill Style dialog, "No Pattern", solid fill with a colour on the Main Layer | Missing | WallLayer has no fill or pattern (plan-core defaults.rs `WallLayer`); see W-137 |
| 10 | Insert Below copies the selected layer; set Material, Fill, Thickness 3/8" and 1/16" | Partial | `insert_below` adds a blank layer rather than a copy |
| 10-11 | Wall Settings: "Foundation to Exterior of Layer" drop-down | Missing | no such field in WallTypeDef; see W-150 (W-137 lists Brick Ledge but not this) |
| 11 | Edit > Default Settings > Walls > Exterior Wall > Edit: Wall Defaults dialog with a Wall Types panel | Works | W-119 (Partial: panels shared), default_pages/architectural.rs |
| 12 | Default exterior type changes only newly drawn walls; existing wall keeps its type; set a wall's type in its own spec | Works | s25_walls_r14 |
| 13 | Default Settings > Walls > Pony Wall: Upper Wall Type, Lower Wall Type, Elevation of Lower Wall Top, which wall displays in plan | Partial | dialogs/default_pages/architectural.rs pony upper/lower and split height (template `pony_split_height`); which part shows in plan is LAY-55 Missing |
| 13 | Default Settings > Floors and Rooms > Floor/Ceiling Platform: Floor Structure and Ceiling Structure Edit buttons | Missing | There is no Floor/Ceiling Platform Defaults page: a floor default holds only two thicknesses (dialogs/floor_defaults.rs `floor_structure_thickness`); layer stacks exist only per room (dialogs/room.rs `define_editor`, R-28). see R-143 (platform defaults with Floor and Ceiling Structure Edit buttons) |
| 13 | Floor Structure Specification: Layer 1 OSB 3/4", Layer 2 framing; Framing Method Joists; Construction I-Joist then Lumber; Width 1 1/2"; thickness 11 1/4" | Missing | Structure layers are Material + Thickness only (dialogs/room.rs `define_editor`, `StructureLayer`); no Framing Method, Construction (Lumber, I-Joist...) or Width fields, so layer 2 cannot be made a 2x12 joist layer. see R-144 |
| 13 | Default Settings > Room Types > Garage > Structure panel (garage, porch, slab floor structure, 4" concrete) | Partial | R-37, R-38, R-99 (Room Type Defaults panels Partial) |
| 13-14 | Default Settings > Floor Levels > 1st Floor > Structure panel: Ceiling Height 97 1/8", Floor Structure 12" drawn from Platform Defaults; floor height of floor 1 fixed at 0 | Partial | R-56, R-57 (floor_defaults.rs); the displayed floor-structure total linked to the Platform Defaults is not a linked value in ours |
| 14 | Build > Wall > Straight Pony Wall; drag a wall from the top of the vertical wall, 28 ft left | Works | W-53 |
| 14 | Right-click drag, then click once per further wall | Works | W-3; tools/wall.rs `secondary_click` |
| 14 | Draw clockwise so surfaces face correctly | Works | W-69 |
| 15 | Closed walls create a room, a Living Area label, auto exterior dimensions, an automatic roof, and floor and ceiling platforms | Partial | Room, Living Area (R-42, R-51) and platforms work; auto exterior dimensions and the roof are not generated on closure (see page 6 row) |
| 15 | Select the room, Open Object: Room Specification Structure panel shows Ceiling Height from 1st Floor Defaults and the Floor Structure from the Platform Defaults | Works | R-56, R-57, dialogs/room.rs |
| 15 | Gaps in walls stop room definition | Works | rooms detection (plan-core rooms.rs), s01 |
| 15-16 | 3D > Create Perspective View > Full Camera: click and drag sets camera point and direction | Works | C-4 tools/camera.rs |
| 16 | Floor platform and ceiling appear in the 3D view; Mouse-Orbit Camera active, drag to rotate; other tool disables it; 3D > Move Camera With Mouse | Works | C-34, C-35 (view3d_panel.rs mouse orbit) |
| 16 | File > Close View returns to plan | Works | APP-10 |
| 17 | 3D > Create Orthographic View > Back Clipped Cross Section: short vertical drag inside a bump-out | Works | tools/camera.rs `BackClipped`, C-8 |
| 17 | CAD > Dimension > Tape Measure on a section to read wall and floor layer thicknesses | Partial | DIM-20 Works in plan; Tape Measure inside section camera views: DIM-62 (camera-view dimensions) Missing |
| 18 | Edit > Default Settings > Dimension > Dimensions > Edit: Saved Dimension Defaults list ("Plan Dimension Defaults"), then Dimension Defaults dialog | Partial | default_pages/dimension.rs; named dimension sets exist (template `dimension_sets`, 13 sets); the Saved Defaults dialog with Copy/Rename/Delete is DS-30 Missing |
| 18-19 | Setup Automatic panel: Exterior Auto Refresh check box | Missing | no auto-refresh flag (DIM-52 Partial, DIM-60 Missing); see DIM-52, DIM-60 for Auto Refresh as a stored switch |
| 19 | Locate Auto Exterior panel: walls at Surfaces or at Wall Dimension Layer | Partial | template `locate_walls: MainLayer`; Surfaces versus Wall Dimension Layer choice exists as Locate (DIM-4, DIM-54 Partial) |
| 19 | Wall Type Definitions > Wall Properties tab > Dimension to Exterior of Layer ("4: Fir Framing 2") | Missing | no per-type dimension layer setting (we use the main layer); see W-150 |
| 20 | CAD > Automatic Dimensions > Auto Exterior Dimensions; dimensions locate the outside of the framing layer | Works | DIM-24, W-73; s17 auto_exterior tests |
| 20-21 | Window > Fill Window Building Only; select a wall; hover a dimension: Pointing Hand; click; type a distance with ' and " marks; Enter moves the wall | Works | DIM-32 (inline edit moves object); unit suffix parsing DIM-66 Partial |
| 22 | Work clockwise around the building moving walls by dimension | Works | s17 `a_manual_dimension_is_tied_to_its_walls_and_follows_when_one_moves` |
| 23 | Edit a dimension that states the selected wall's own length: choose Move Both Ends, Left/Right End, Top/Bottom End | Missing | editing moves "the nearer end" only (DIM-66 note); see DIM-48 |
| 23 | Connected walls follow a moved end | Works | W-31, editor/connect.rs |
| 23 | Select a wall and drag its Move edit handle perpendicular to the wall | Works | S-1 family, tools/select.rs handles |
| 24 | Arrow keys nudge the selected object | Partial | S-129 (nudge by snap unit) |
| 24 | Default Settings > Text, Callouts and Markers > Rich Text > Edit: Saved Rich Text Defaults list ("Plan Rich Text Defaults"), then Rich Text Defaults dialog with a Rich Text panel (Uppercase button) and an Appearance panel (layer "Text") | Partial | Rich text defaults exist (TXT-4, default_pages/text.rs); saved-default lists DS-28 Partial; a per-run Uppercase exists in the model (`RichRun.upper`, drawn by tools/cad/style.rs) but no Uppercase button was found in the Rich Text editor or its defaults; TXT-57 |
| 25 | CAD > Text > Rich Text: click; Rich Text Specification Rich Text panel; type; select one letter and toggle Uppercase | Partial | TXT-4 (inline markup editor); `RichRun.upper` is drawn, but no Uppercase toggle button was found |
| 25 | Misspelled word underlined in red in the text field | Works | dialogs/spell_check.rs layouter underlines misspelled words in red inside the text field |
| 25 | Select the text, Move edit handle drag | Works | S-25 |
| 25 | Right-click a word > Learn Spelling | Partial | spell check dialog has Add (dialogs/spell_check.rs, spell.rs); not confirmed in code a right-click Learn Spelling inside the Rich Text Specification field |
| 25-26 | File > Make a Copy: New Plan dialog, name "Chic Cottage-Shell"; archive appears in Project Browser unopened | Partial | APP-96 (Save a Copy exists, APP-19); copy is not registered as a project member and not listed in the browser |
| 26 | File > Close All Views | Works | APP-10 |

### Lesson 1 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Value of file revisions (archives) | Works | files.rs archives, autosave; Make a Copy Partial |
| Default settings and why they matter | Works | Default Settings tree (defaults.rs) |
| What a Saved Plan View stores (floor, Default Set, Reference Floor) | Partial | LAY-49 |
| Defaults to set before exterior walls (Exterior Wall, Floor/Ceiling Platform, 1st Floor, Auto Exterior Dimension) | Partial | see rows above |
| Structural defaults set overall height | Works | R-56 |
| Room definition, floor and ceiling platforms, Living Area | Works | R-1, R-2, R-42 |
| Two ways to move walls (dimension, Move handle) | Works | DIM-32, S-1 |
| Distinct colour for a Main Layer fill | Missing | no layer fill (see Fill row) |

## Lesson 2: Interior Walls (pp. 28-44)

Workflow: reopen the working plan, set Interior Wall defaults, draw interior walls from midpoint snaps, assign room types, check them in a cross section and a floor overview, subdivide and merge rooms, control room labels, change a wall type and reverse layers, use room dividers, lay out with Interior Dimension and Auto Room Dimension, delete dimensions, align collinear walls, edit layer intersections, annotate, make a revision.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 28-29 | Reopen the plan: Dashboard recent list, File > Recent Documents, double-click in the Projects panel, Open All Plans/Layouts on a project | Partial | File > Open Recent works (APP-5); no Dashboard (APP-6), no project nodes to open from (APP-117) |
| 29 | Pin a document to the top of the Recent Documents list (Pin File icon on the Dashboard) | Missing | APP-67, APP-123 |
| 29 | Shift-click adds to the selection | Works | selection add (tools/select.rs) |
| 29 | Object Snap Indicators (red Midpoint and Intersection markers) while drawing | Works | snap.rs; S-68..S-71 |
| 29-30 | Floor Defaults and Room Type Defaults hold structure definitions (as in lesson 1) | Partial | see Lesson 1 platform-defaults break |
| 30 | Default Settings > Walls > Interior Wall > Edit; Wall Types panel shows "Interior-4"; Define shows 2x4 framing with drywall each side | Works | template wall type `Interior-4` (4.5 in) exists; default_pages/architectural.rs |
| 30 | Tip: turn on Auto Refresh for Auto Exterior Dimensions while adding interior walls | Missing | Auto Refresh flag absent (Lesson 1, DIM-52); see DIM-52 (Auto Refresh as stored, per-set switch that re-runs Auto Exterior after every edit) |
| 30 | Working Plan View is the right view for drawing | Partial | our default view set is one plan view per floor (LAY-49); no named starter views |
| 30-31 | Build > Wall > Straight Interior Wall; click near the midpoint of a wall (red midpoint marker) and drag across to the far wall | Works | W-1, W-11 midpoint snap; s01, s14_wall_edit_and_snaps |
| 31 | Draw a second vertical wall and a horizontal wall across the middle; the plan divides into six zones | Works | R-9 (wall across a room splits it), s03_interior |
| 32 | Room types give attributes (porch: concrete, ceiling and roof; deck: planking, no ceiling) | Works | R-37, R-38, R-53; template `room_types` (46) |
| 32 | Select Objects, click in a room (highlights), Open Object: Room Specification General panel, Room Type drop-down (Kitchen) | Works | dialogs/room.rs "Room Type" row (line ~233), R-19..R-37 |
| 33 | Assign Dining, Living, Entry, Bedroom, Garage | Works | R-37 |
| 33 | A horizontal wall that crosses meets intersecting walls with a new separate segment on the far side | Works | W-34, W-105 (break at T) |
| 33 | Back Clipped Cross Section inside the Entry: Garage floor structure differs from Entry and Living | Partial | camera exists (C-8); the Garage room type default structure (4" concrete) versus a framed floor: our room-type defaults carry `default_floor_finish` and function but no per-type floor structure (R-99 Partial) |
| 34 | 3D > Create Perspective View > Perspective Floor Overview shows floor finishes per room | Works | C-11 |
| 34 | File > Close View in each | Works | APP-10 |
| 34-35 | Subdivide a room with a new interior wall; the new room inherits the type of the larger room | Partial | rooms re-detected on each wall change; identity kept by anchor (R-14, R-15 Partial: "inherit larger" only through the room-name anchor). BREAK risk: a new room created by splitting may lose the type if the anchor sits in the smaller piece |
| 35 | New wall collinear with an exterior wall aligns to the exterior surface of the framing layers, leaving a small interior jog | Partial | W-31/W-45 join rules; the alignment-by-main-layer-outside rule: not confirmed in code (Lesson 2 p.42 row) |
| 35 | Assign types to the new rooms (Foyer closets, Bath, Bedrooms) | Works | R-37 |
| 36 | Select a room label, Delete edit button hides it; reopen the room, check Show Room Label | Partial | label elements are individual checkboxes in the room's Label tab (dialogs/room.rs lines ~800-808: Room Name, area...); no single Show Room Label box and no verified label-only delete |
| 36 | Room Name field renames Entry to "Foyer" | Works | R-19, dialogs/room.rs "Room Name" |
| 36 | Rotate handle (triangle) and Move handle (square) on a room label | Partial | R-44 Works (move); rotate handle on labels: not confirmed in code, no row |
| 36 | Active Layer Display Options side window with Name Filter ("room"); layers Room Labels, Rooms Interior Area, Rooms Interior Dimensions, Rooms Standard Area; clear the Disp check mark | Works | LAY-3, dialogs/layer_display.rs `name_filter`; layer names in plan-core layers.rs (not confirmed in code exact names match) |
| 36 | Draw a wall dividing the Closet in two; select the vertical wall, Add Break edit button, click the intersection | Works | W-44 |
| 36-37 | Drag the Resize handle of a wall end to snap onto another wall; the smaller room vanishes and its type merges into the larger | Works | W-105, R-15 (Partial for type inheritance on merge) |
| 37 | Select a wall, Shift-click another: group selection; Open Object edits both (Wall Specification Wall Types panel) | Works | W-83 multi-wall Open Object |
| 37 | Vector View drop-down above the preview: Show Plan View | Missing | no plan/vector/3D preview selector in the wall dialog (APP-86 Partial); see W-153 (Wall Types preview with Vector View/Plan View switch) |
| 37 | Wall Type "Fire-6" (fire-rated drywall layer with solid red fill) | Differs | Daniel's template ships Interior-4/6 only; there is no Fire-6 and no layer fill colour (Lesson 1 Fill row) |
| 37 | Define: change the Fill of the Main Layer to red | Missing | no layer fill (Lesson 1) |
| 38 | Reverse Layers edit button flips layers of the two selected walls | Works | W-23, S-55 |
| 38 | Drag a Move handle slightly to realign a thicker reversed wall | Works | S-1 handles |
| 38 | Room Divider: a wall that defines rooms without being a wall (ceiling height or flooring change) | Works | W-55, R-4 |
| 39 | Drag a wall's end handle; a room label disappears as two rooms merge | Works | R-15 |
| 39 | Build > Wall > Room Divider, drag across the opening; dashed line; both rooms keep labels | Works | W-55 (`WallVariant` room divider), s16 |
| 39 | Add Break, select segment, Make Wall(s) Invisible edit button (pair of dashed lines) | Missing | W-130: only an Invisible check box in the dialog |
| 39 | Wall Specification General: Thickness 4 1/2", Invisible checked, No Locate checked | Works | W-24 flags (no_locate), DIM-5 |
| 39 | Wall Types panel: choose "Room Divider" wall type; Thickness drops to 0" and the preview shows the plan look | Missing | there is no Room Divider wall type; a divider is a flag with its own layer (walls.rs `ROOM_DIVIDER_LAYER`); see W-152 (Room Divider as a wall type with 0 thickness that a normal wall can be switched to) |
| 39-40 | Interior Dimension tool locates exterior walls on their inner side and shows room interiors | Works | DIM-14, DIM-44 |
| 40 | Default Settings > Dimension > Dimensions > Edit: Saved Dimension Defaults (multiple per scale, Electrical, Framing plan), Currently Active; Edit opens Plan Dimension Defaults | Partial | named dimension sets (template `dimension_sets` 13; active_dimension_set); no Saved Defaults dialog with Currently Active list (DS-30 Missing) |
| 40 | Locate Interior panel: Walls at Main Layer Inside or Surfaces; Wall Option "Display Wall Widths" | Partial | template `interior_locates_interior_surfaces`; DIM-54 Partial; Display Wall Widths: no row; see DIM-54, DIM-68 |
| 40 | CAD > Dimension > Interior Dimension: drag across the Garage interior | Works | DIM-14 |
| 40 | Auto exterior and interior dimension differ by the framing thickness (11" = 2 x 5 1/2") | Works | s17 `locate_objects_walls_decides_which_surface_a_manual_dimension_measures` |
| 40-41 | Select a room, Auto Room Dimension edit button: dimension line along each wall | Missing | R-107 lists Auto Room Dimensions among the missing room edit buttons; `DimMode::AutoInterior` is a click-in-room tool (DIM-27), so the result is reachable by another gesture |
| 41 | Reposition interior walls by dimension | Works | DIM-32 |
| 41 | Select a dimension, Delete edit button, Edit > Delete, or the Delete key | Works | S-88 |
| 41 | CAD > Dimensions > Delete Dimensions: deletes all in the view; confirms disabling Auto Refresh | Partial | DIM-60 (Delete Objects category only); no Auto Refresh prompt |
| 41-42 | Edit > Delete Objects: All Rooms On This Floor radio, CAD heading: Manual Dimensions, Automatic Dimensions | Partial | S-192 (scopes this floor / all floors; room scopes: no spec) |
| 42 | Collinear walls of different types snap with the outside of the Main Layers aligned; realign with the Move handle; red Intersection indicator when inside surfaces meet | Partial | W-45/W-36 joins; main-layer-outside alignment of differing thickness, stepped faces: W-36 Partial; indicator: snap.rs |
| 42 | Set closet depth to 2' by moving the front wall | Works | DIM-32 |
| 42-43 | Edit Wall Layer Intersection edit handle on a wall; drag the handle at the middle of the Main Layer and of the fire-rated layer | Missing | W-144 |
| 43 | Rich Text: type "2x4 exterior walls 16" OC U.N.O."; U.N.O. no longer flagged after Learn Spelling; Uppercase on the x | Partial | TXT-4; learned word persists in the spell dictionary (spell.rs Add); Uppercase missing (Lesson 1) |
| 44 | File > Make a Copy, name "Chic Cottage-Rooms"; File > Close All Views | Partial | APP-96, APP-10 |

### Lesson 2 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Recent Documents list and pinning | Partial | APP-5 Works; pin APP-67 Missing |
| Room Types and what they change | Works | R-37, R-38, R-53 |
| Subdividing and merging rooms (inherit from the larger room) | Partial | R-14, R-15 |
| Controlling room label display (delete label, layer display) | Partial | R-44, LAY-3 |
| Reversing a wall's layers | Works | W-23 |
| Room Dividers | Partial | W-55 Works; as a wall type: Missing |
| Interior Dimension vs Auto Exterior Dimension locate | Works | DIM-14 |
| Auto Room Dimension | Missing | R-107 |
| Deleting dimensions (one, view, all views) | Partial | S-88, DIM-60 |
| Aligning collinear walls and layer intersections | Partial | W-45, W-144 Missing |

Further assessment topics of lesson 2 (p. 45): the Kitchen versus Garage room types (Kitchen: finish layers over a framed floor platform; Garage: slab with no finish) is Partial (R-99, room types carry no per-type structure in the template); the Room Divider has 0 in thickness by default is Missing (our divider keeps the wall thickness; see the Room Divider row).

## Lesson 3: Multiple Floors (pp. 45-59)

Workflow: derive a second floor from the first, copy and edit a wall type for the upper floor, navigate floors, build a furred basement foundation with garage curbs, align walls between floors (Reference Display, Align With Wall Above, Paste Hold Position in a second tiled view), then build a wall legend with a Wall Schedule on a CAD Detail.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 46 | Add new floors after Floor 1 walls are done; a plan can hold up to 30 floors | Works | floors.rs has no floor-count limit (none found) |
| 46 | Align With Floor Above / Below edit buttons | Missing | W-145 |
| 46-47 | Copy and Paste Hold Position (Alt+Shift+V) | Works | S-83 |
| 46-47 | Navigate with Up/Down One Floor, Change Floor/Reference tool, Project Browser | Partial | Up/Down buttons and Project Browser floors exist; the Change Floor/Reference list dialog is LAY-42 Partial, LAY-43 Missing |
| 47 | Floor 1 stays at 0; foundation is Floor 0 | Works | floors.rs; template `floor_defaults` |
| 47 | Build > Floor > Build New Floor; New Floor dialog: "Derive new 2nd floor plan from the 1st floor plan" | Works | R-59; dialogs/floor.rs (Derive from exterior walls / all walls / blank) |
| 47 | Derived walls take the 1st floor exterior wall types; walls over a pony wall take its upper type | Partial | floors.rs `DeriveFrom::ExteriorWalls` copies types; pony upper-type inheritance: not confirmed in code (floors.rs build_new_floor_with) |
| 47-48 | 2nd Floor Defaults dialog opens after OK: default Rough Ceiling 97 1/8", Floor Structure | Partial | R-116 (Floor Defaults after a new floor: Partial); the "Rough Ceiling" naming and the page-205 flooring default by floor: R-56 |
| 48 | Build > Wall > Define Wall Type, Copy "Siding-6", name "Shingle-6" | Partial | Copy exists in the dialog; the menu command does not (Lesson 1) |
| 48 | Layer 1 thickness 3/4", Material "Shake - Natural" from Select Material (Core Catalogs > Materials > Roofing > Shakes and Shingles > Wood) | Missing | Material is a free text cell, no library material picker (Lesson 1) |
| 48 | Change the Fill of Layer 4 to a pale purple solid | Missing | no layer fill |
| 48-49 | Shift-select the Siding-6 walls on Floor 2, Open Object, Wall Types panel, pick Shingle-6 | Works | W-83 |
| 49 | Down/Up One Floor; Tools > Reference Floors > Change Floor/Reference lists all floors | Partial | LAY-42 |
| 49 | Project Browser > Floor Levels > right-click a floor > Open View | Works | shell/docks.rs floor nodes (APP-35) |
| 49 | Second floor in a separate view window (New Plan View) | Partial | tabs per floor exist; `Tools > New Plan View` as a separate window of the same plan: APP-41 Missing (no tile) |
| 50 | Build > Floor > Build Foundation: Foundation panel, Foundation Type "Walls with Footings" | Works | R-61; dialogs/foundation.rs |
| 50 | Stem Walls: Minimum Stem Wall Height 113 1/8" | Works | dialogs/foundation.rs "Minimum Stem Wall" |
| 50 | Garage Options: Garage Floor to Stem Wall Top (12") and Minimum Garage Height (37 1/2"): 12" curb around the garage, stem walls 24" below the garage slab | Missing | dialogs/floor.rs only offers a "Build Garage Floor" check box that is not stored ("Not stored by the model yet", spec.garage_floor). see R-133 (garage curb and lowered stem height options on Build Foundation) |
| 50 | Derive New Foundation Plan From the First Floor Plan; garage foundation separated; "S" markers where the stem wall top height changes | Partial | R-18, R-26 dropped room stem wall; no S markers (see R-131) |
| 50 | Perspective Floor Overview to see the garage curb; Room Specification Structure panel: "Room Supplies Floor for the Room Above" | Missing | no supplies-floor flag; the Garage on Floor 1 inheriting its slab and curb from the Floor 0 room: R-115 Partial; see R-145 |
| 50-51 | Select the foundation wall; Wall Types panel shows `8" Concrete Stem Wall`; Define; Copy; rename "8" Concrete with Furring" | Partial | template has `Foundation-8`; Copy yes; names with quote marks accepted |
| 51 | Insert Below, then Move Down moves the layer from the "Main Layers" group into "Interior Layers" | Missing | our table is a flat list with a single Main radio (W-137: groups Exterior/Main/Interior, multiple Main layers) |
| 51 | Layer 2: material "Insulation Air Gap", thickness 1", Role "Air Gap", no Fill | Missing | no Role column (W-138) |
| 51 | Layers 3 and 4: "Fir Framing 2" 3 1/2" Role Framing with a pale green fill; "Drywall" 1/2" Role Standard | Missing | no Role, no Fill (W-138) |
| 51 | Select Material dialog also has a Plan Materials panel listing materials already in the plan | Missing | no Select Material dialog |
| 51 | Edit Layer button: Wall Layer Specification dialog with Wall Layer Role, Stud Construction "Wall Framing - Lumber", Stud Width 1 1/2", Stud Spacing 16" OC | Missing | W-138; plan-framing wall framing takes spacing from its own settings (Build Framing dialog), not from the wall layer |
| 51 | Add Break at two wall intersections; Shift-select the basement walls | Works | W-44 |
| 52 | Open Object, choose the new type for all selected walls; rotate and zoom the preview (drag, wheel) | Partial | W-83; preview not rotatable |
| 52 | Foundation panel: Center Footing on Main Layer; Footing Offset updates (-2 1/2") | Works | dialogs/wall.rs lines ~1698-1701 `footing_center`, `footing_offset`; W-52 family. Check that the offset recomputes from the interior layers when the box is ticked: not confirmed in code |
| 53 | Tools > Floor/Reference Display > Reference Display (F9): floor shown in red | Works | LAY-9, R-65 |
| 53 | Draw a wall over the reference wall: snaps; light-blue highlight of edge lines when exactly aligned | Partial | snap.rs `snap_with_reference` (LAY-9); the light-blue "aligned" edge highlight: no row; see S-194 |
| 54 | Arrow key nudges the wall 1" | Works | S-129 |
| 54 | Align With Wall Above edit button appears only when out of alignment; click moves the wall back | Missing | W-145 |
| 54 | Select the wall, Edit > Copy; Tools > New Plan View creates a second window; Up One Floor x2; Window > Tile Vertically | Missing | APP-41; second window of the same plan |
| 54 | Edit > Paste > Paste Hold Position puts the copy at the same x,y on another floor | Works | S-83 |
| 55 | Default Settings > Schedules > Wall Schedule > Edit: Columns/Rows panel, Columns to Include / Available Columns with Add and Remove; add "Total Width", "Wall Construction, Upper" and "Wall Construction, Lower" | Partial | schedule defaults per kind: L-31; the wall schedule has Thickness, Wall Type, Interior/Exterior Covering but no Total Width or Upper/Lower Wall Construction columns (plan-core schedules.rs `WALL_FIELDS`); see L-235 |
| 55-56 | Display Column Headings in Schedule check box | Works | schedule_spec.rs (columns, headings) |
| 55-56 | Main Text Style panel: Use Layer for Text Style, Use Text Style ("Schedule Style"), Custom | Partial | Schedule has `text_style` by name (schedules.rs); the by-layer/ by-style/ custom trio is TXT-17 family: not confirmed in code |
| 56 | Project Browser > plan > CAD Details > right-click > New CAD Detail; name "Legends" | Partial | L-41 CAD Detail Management Works; creation from the Project Browser context menu: APP-121 Partial |
| 57 | Tools > Schedules > Wall Schedule; click to place it in the detail; Open Object General panel: Main Title "Wall Legend" | Works | ToolId::Schedule, ScheduleKind::Wall; dialogs/schedule_spec.rs `title` |
| 57-58 | Selected schedule shows diamond Resize Columns handles in the title row and square Move Column handles in the heading row; drag to resize | Missing | tools/select.rs selects, moves, opens and deletes a schedule as one object; no column handles. see L-63 |
| 58 | General panel: select a column in Columns to Include, Rename button, Rename Schedule Column dialog | Works | schedule_spec.rs rename (test at line ~563) |
| 58 | Wall Specification > Schedule panel: uncheck Include in Schedule | Works | dialogs/wall/tabs.rs line ~566 `Include in Schedule` on the wall's Schedule tab leaves the wall out of the Wall schedule (Round 15 walls2, parity/walls.md); the schedule's own Categories to Include tree is separate |
| 58 | Wall Schedule Specification General panel: Categories to Include tree, uncheck "Room Divider" | Missing | only a text `filter` narrows a schedule (schedules.rs `filter`); no category tree. see L-233 |
| 58 | Close the CAD Detail view | Works | APP-10 |
| 59 | File > Make a Copy "Chic Cottage-Floors"; Close All Views | Partial | APP-96 |

### Lesson 3 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Deriving a floor from an existing floor | Works | R-59 |
| Two ways to switch floors | Works | toolbar buttons, Project Browser |
| Reference Floor Display toggle and its F9 hotkey (Tools > Reference Floors) | Works | LAY-9 (the hotkey follows Daniel's set) |
| Two ways to align walls between floors (Align With Wall Above/Below; Paste Hold Position) | Partial | W-145 Missing; S-83 Works |
| Reading the current floor from the Change Floor/Reference button | Partial | LAY-42 |
| Garage Room Type: stem walls and curb, "S" markers, slab inherited from the room below | Missing | see Build Foundation rows |
| Meaning of an "S" marker | Missing | no marker |
| CAD Detail views recommended for schedules | Works | L-40, L-41 |
| Three ways to assign text style to an object (layer, style, custom) | Partial | TXT-17 |
| Adjust a schedule column width (Resize Columns handle) | Missing | see handle row |

## Lesson 4: Interior Stairs (pp. 60-81)

Workflow: stair and railing defaults, a short flight between rooms, a flight between floors, position by dimension, rise/run in the Staircase Specification, a tiled section with a saved camera, Auto Stairwell, headroom with End to End dimensions and point markers, a stacked basement stair, turning layers off, converting auto-stairwell railings to walls, clip planes on a section, note schedule defaults, a CAD Detail with a Note Schedule, plan notes.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 61-62 | Tab selects the next object under the pointer (Select Next Object edit tool, also Spacebar) | Works | tools/select.rs `cycle` (Tab; Shift+Tab backwards); the Spacebar binding is Chief's default, ours is Daniel's |
| 62 | Finalize floor and ceiling heights first (stair rise depends on them) | Works | stairs recompute from floor heights (CB-98 Partial) |
| 62 | Default Settings > Stairs and Ramps > Interior Stairs > Edit: General panel Width 44" | Works | default_pages/architectural.rs "stairs" page; template/CodeDefaults `stair_width` is 36 in |
| 62-63 | Newels/Balusters panel: Newels Width 4", uncheck Rail Passes Over Newel, Height 44" | Partial | dialogs/stairs.rs `newels_balusters`: Newel Size, Newel Height, spacing, cap; no "Rail passes over newel" switch; see W-116, CB-395 |
| 63 | Newel Type > Library button opens Select Library Object (type-filtered modal browser, search "newel", folder filter, Boxed > BX-02) | Missing | CB-395, W-116: newel/baluster styles are built-in shapes; no library newel pick |
| 63-64 | Materials panel: select a component, Ctrl-click more components with the same material, Select Material, "Color - White" from Plan Materials, "Red Oak - Natural" from Library Materials, Tread "Red Oak 3-4-5" Plank" | Partial | dialogs/stairs.rs `materials` lists components with a free-text material box each; no multi-select by shared material, no Select Material dialog with Plan Materials and Library Materials panels (C-55..C-61 materials work is separate); see C-87 (Select Material dialog) |
| 64 | Go to Floor 1; Build > Stairs > Straight Stairs; click once in the Garage corner: a short flight spanning the Garage-to-Foyer floor difference with an arrow and "UP" label | Partial | StairKind::Straight; CB-101 Partial (one click builds the flight between floors; the level-change span between rooms on one floor is not done); UP label CB-117 |
| 64-65 | Build > Stairs > Draw Stairs; click right of the wall and drag upward; stairs snap to the room side | Works | CB-23; tools/stairs.rs |
| 65 | CAD > Dimension > Manual Dimension from the stair's bottom to the exterior wall; drag the diamond end handle to the inside of the Main Layer | Works | DIM-11, DIM-30 |
| 65 | Click a located stair dimension (Pointing Hand), type 4' 6", Enter: stair moves | Partial | dim_assoc.rs anchors cover walls, openings, cabinets and fixtures/symbols; a stair is not an anchor target (AnchorTarget): moving a stair by its dimension value: not supported. see DIM-71 |
| 65 | Staircase Specification General panel: Staircase Information (number of risers, rise angle; "does not reach Floor 2, recommends 16 risers") | Partial | CB-159 (read-outs of riser height and totals; no rise angle, no recommendation text) |
| 65-66 | Advanced Options: Lock Bottom, Make Best Fit, Lock Number of Treads, Tread Depth 10 1/2" | Partial | dialogs/stairs.rs: "Fit stair to floor-to-floor" button, Lock Settings Tread depth and Riser height; no Lock Top/Bottom, no Lock Number of Treads (CB-160, CB-162) |
| 66 | Extend the wall beside the stair to the stair's top end with its end handle | Works | W-1 handles |
| 66-67 | 3D > Create Orthographic View > Back Clipped Cross Section, horizontal drag, entirely inside the stair width | Works | C-19 |
| 67 | Window > Tile Vertically puts the section beside the plan; the active view has the darker title bar | Missing | APP-41 (tabs only) |
| 67 | Tools > Active View > Save Active View; Edit Active View: Camera panel Name "Stair Section"; Plan Display panel (callout, Callout Label S1); Layer panel ("Cameras") | Partial | toolbar actions exist (toolbar/config.rs); camera spec tabs Camera, Label etc. (dialogs/camera.rs); callout label S1 numbering and layer: not confirmed in code (C-4..C-30) |
| 68 | Saved camera views listed by name in the Project Browser | Works | shell/docks.rs cameras node |
| 68 | A stairwell is a room of type Open Below with no floor platform, found on the floor above the stair | Partial | CB-30 (room "Stairwell", has_floor false; not RoomFunction::OpenBelow); R-101 |
| 68 | Default Settings > Walls > Railing > Edit: Rail Style panel "Rail to Post"; Newels/Posts Width 4", Height 38", Library newel BX-02 | Partial | W-127 (Round 15 walls2 brief); newel library pick Missing (W-116) |
| 68 | Railing Materials panel: Balusters/Alabaster components to "Color - White", "Rail" to "Red Oak - Natural" | Missing | railings have no component materials list (W-116 area); see C-87 (Materials panel for railing defaults) |
| 68 | Select the staircase, Auto Stairwell edit button: floor platform above the stair is removed in the section | Works | CB-29 (hole), dispatch.rs `auto_stairwell`; guard rails round the opening are still open (CB-29 Partial) |
| 69 | Window > Tab Windows returns to tabbed windows | Partial | APP-42 |
| 69 | Up One Floor: new room beside the interior wall; railings enclose the rest; the top railing has a Doorway | Partial | CB-29: guard rails and the doorway opening in the top railing are not generated |
| 69 | Switch view windows via tab, Window menu list, or Project Browser > Open View | Works | plan_tabs.rs, docks.rs |
| 70 | CAD > Dimensions > End to End dimension in a section from nosing to ceiling; preview locates the second tread within Reach | Partial | DIM-62 Missing (dimensions in camera views) |
| 70 | Information message: the dimension locates point markers, not model objects | Missing | no dimensioning in section views (DIM-62) |
| 70-71 | Point Marker at the end of a section dimension: select it (or Select Next Object), drag its Move handle; snaps to the top of the tread; check the marker against the ceiling surface line | Missing | DIM-62, DIM-47 |
| 71 | Pan: Window > Pan Window or middle-mouse drag | Works | LAY-24 family, Pan tool |
| 71 | Window > Fill Window | Works | LAY-27 |
| 72 | Stacked stair: Floor 1, select the staircase, Copy; Down One Floor; Paste Hold Position puts it directly below | Works | S-83; stairs copy with the object |
| 72 | Extend the wall beside the basement stair | Works | handles |
| 72 | Basement stair: Staircase Information reports "steep"; Riser Height 7 5/8" | Missing | no steepness remark; CB-159 |
| 72 | Lock Top, Make Best Fit, OK | Missing | CB-160, CB-162 |
| 72-73 | Tape Measure from the stair foot to the exterior wall; too narrow; Edit > Undo restores the stair | Works | DIM-20, one undo step per edit |
| 73 | Select the staircase, Auto Stairwell again; save; Up One Floor shows the new stairwell on Floor 1 | Works | CB-29 |
| 73 | Active Layer Display Options: Name Filter "stairs", clear Disp on "Stairs & Ramps"; the stair dimension still shows | Works | LAY-3; layer name check: not confirmed in code plan-core layers.rs |
| 74 | Select the auto-stairwell Railing; Railing Specification General: uncheck Railing and No Locate | Partial | railing walls carry a Railing flag in dialogs/wall.rs; unchecking converts it to a normal wall: not confirmed in code |
| 74 | Wall Types panel: Interior-4 from the Type drop-down | Works | W-47 |
| 74-75 | Drag an interior wall's end handle to replace a railing under it; "as you drag, the Interior-4 wall replaces the railing" | Missing | no replace-on-drag behaviour; a wall dragged over a railing makes crossing walls (W-36). see W-154 |
| 75 | Same Wall Type diamond edit handle above a wall end: drag to extend replacing railing and opening | Missing | W-143 |
| 75 | Add Break; click a temporary dimension; type 6' Enter | Works | W-44; typed temp dimension S-59..S-61 |
| 75 | Turn "Stairs & Ramps" layer back on | Works | LAY-3 |
| 75 | Open the saved "Stair Section": select the callout and Open View, double-click the callout, or Project Browser > Cross Sections > Open View | Partial | camera nodes open from the browser; double-click on a plan callout opening the view: not confirmed in code (C-4 family) |
| 75-76 | End to End dimension for basement headroom in the section | Missing | DIM-62 |
| 76 | Tools > Edit Active View > Camera panel: check Clip Elevation and Clip Sides; Clip Width, Bottom and Top Clip Elevation | Missing | camera section has only a back clip distance (plan-core camera.rs `back_clip`); see C-136 |
| 76 | Active Layer Display Options: show layer "CAD, Clip Lines"; the clip rectangle appears in the plan | Missing | see C-136 |
| 76-77 | Select the Top Clip Elevation line; Move Line Segment handle; Ctrl overrides grid snaps; move by dimension | Missing | see C-136 (clip lines are draggable plan objects) |
| 77 | Default Settings > Schedules > Note Schedule > Edit | Partial | schedule defaults per kind (L-31); default_pages/architectural.rs schedule page |
| 77-78 | General panel: Include Objects from All Floors; Categories to Include tree: untick Note, tick General | Missing | schedules have `floor_scope` but no category tree (Lesson 3); see L-233 already listed |
| 78 | Columns/Rows: Display Column Headings off; Columns to Include only "2D Symbol" and "Text"; Group Similar Objects; Object Preview Options (Scale Images, Use Plan View Scale) | Partial | columns and `group_by` exist (schedules.rs); 2D symbol column and preview options absent; see L-234 |
| 78 | Labels panel: Callout Shape "Capsule" | Missing | callout shapes are Square, Circle, Hexagon (tools/text.rs CalloutShape); see TXT-56 |
| 78 | CAD > CAD Detail Management: select "Legends", Rename to "Legends and Notes", Open | Works | L-41, dialogs/details/management.rs |
| 78 | Tools > Schedules > Note Schedule; click to place right of the Wall Legend | Works | ScheduleKind::Note |
| 79 | Note Schedule Specification General: Main Title "Floor Plan Notes" | Works | dialogs/schedule_spec.rs |
| 79 | Shift-click both schedules; Align/Distribute Objects edit button; Move Objects Vertically to Top Edges | Partial | S-54, S-149 (dialog has align modes; "Don't Move/Top Edges" structure not confirmed in code) |
| 79 | Default Settings > Text, Callouts and Markers > Note > Saved Note Defaults > Edit "Plan Note Defaults": Note panel (Text empty, Type "General Notes", Generate Shape from Schedule checked) | Partial | TXT-9 note types in dialogs/text/manage.rs; the schedule-driven callout shape link is absent |
| 79 | Window > Tile Vertically (plan + CAD Detail); Down One Floor | Missing | APP-41 |
| 79-80 | CAD > Text > Note; click near the stair; Note panel Text "headroom 6' 8" min.", Type General | Works | TextMode::Note, TXT-9 |
| 80 | The note shows as a Capsule with number 1 in the plan and as line 1 in the Note Schedule | Partial | note numbering text exists (`Note 1: ...` with NoteTypes prefix); the callout-shape symbol linked to the schedule row: absent |
| 80 | Copy the note and Paste Hold Position on Floor 0; schedule includes all floors and groups similar: one "1" row | Partial | `floor_scope` all floors and `group_by`; label derives from the schedule: not confirmed in code |
| 80 | File > Make a Copy "Chic Cottage-Stairs" | Partial | APP-96 |

### Lesson 4 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Stairs drawn on Floor 1 seek Floor 2 | Works | CB-98 |
| Stairwell Room Type is "Open Below", on the floor above | Partial | CB-30 uses a "Stairwell" room, R-101 |
| Auto Stairwell edit button | Works | CB-29 |
| Check headroom in a cross section with End to End dimension and point markers | Missing | DIM-62 |
| Section view clip planes (Clip Elevation, Clip Sides) | Missing | see C-136 |
| Multiple saved camera views listed in the Project Browser | Works | docks.rs |
| Note callouts and Note Schedules (types, "General" only, group similar objects) | Partial | TXT-9, ScheduleKind::Note |
| Align/Distribute schedules to top edges | Partial | S-149 |

## Lesson 5: Doors and Windows (pp. 82-105)

Workflow: set door and window defaults, place hinged, sliding, doorway, bifold, pocket and garage doors, flip hinge and swing, resize, change lites, assign library hardware, style and casing, place and edit windows, centre openings, position by dimension, reflect and replicate windows, make a mulled unit, set schedule defaults, door and window schedules in a CAD Detail, schedule callouts, comments and a leader line with a macro, plan notes.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 83 | Hinged Door and Sliding Door tools make an exterior or interior door depending on the wall | Works | plan_defaults `door_template(d, exterior)`; DW-7; tools/opening.rs |
| 83 | Center Object edit tool | Works | S-53 |
| 83 | Transform/Replicate Object edit tool | Works | S-47, S-103 |
| 83 | Library > Get Additional Content (name brand catalogs) | Out-of-scope | cloud content download |
| 83 | Ctrl/Cmd+C is Concentric Resize while resizing | Differs | Chief default hotkey; ours follow Daniel's set (shell/hotkeys.rs); behaviour exists as Concentric (S-65) |
| 83-84 | Each Door Tool has one or two defaults dialogs (Interior and Exterior for Hinged and Sliding) | Partial | DW-124; template has `interior_door` and `exterior_door`; defaults pages for the other door tools are shared |
| 84 | Default Settings > Door > Interior Door > Edit: General (Door Style "Door P04" with preview), Casing, Rough Opening, Framing, Hardware, Materials | Works | dialogs/opening.rs tabs General, Casing, Rough Opening, Framing, Hardware, Materials; template `style: Door P04`; DW-100 |
| 84 | Casing panel: a basic stock profile when no profile is chosen | Works | DW-57, casing.rs fallback |
| 84-85 | Materials panel: select "Jamb (Exterior)", Shift-click "Molding" under Interior Casing to select the range, Select Material, "Color - White" from Plan Materials | Partial | Materials tab lists components with a material per component (dialogs/opening/tabs.rs); no range select, no Select Material dialog (see Lesson 4); see C-87 already raised |
| 85 | Build > Door > Hinged Door: press, drag along the wall to set hinge side, across to set swing, release | Works | DW-5, DW-8; tools/opening.rs |
| 85 | Open Object title reads "Exterior Door Specification" or "Interior Door Specification"; preview rotates by dragging | Partial | spec dialog class by wall: door default class by wall (plan_defaults door_template exterior flag); dialog title text "Exterior Door Specification": not confirmed |
| 85 | Place three more hinged doors | Works | DW-1 |
| 86 | Build > Door > Sliding Door: drag left to right sets the moveable side and which side is interior | Works | OpeningVariant Sliding, DW-8 family |
| 86 | Doorway, Bifold Door, Pocket Door, Garage Door tools with drag to set hinge, direction | Works | s13_opening_variants; DW-41, DW-42, DW-43 |
| 87 | Change Opening/Hinge Side and Change Swing Side edit buttons | Partial | DW-139 (Partial); actions exist in editor/opening_edit.rs; not confirmed in code both buttons appear in the contextual toolbar |
| 87-88 | Triangular door swing handle: drag along the arc to change the open angle, parallel to the wall to change hinge, perpendicular to change swing side | Partial | swing angle exists in the spec (`swing_angle`); a drag handle that flips hinge and swing: DW-19-ish, not confirmed in code (select.rs handles) |
| 88 | Resize a door with end handles; hold C for concentric resize; Width 60" in the General panel | Works | DW-26, S-65; spec width field |
| 88 | Exterior Door Specification Lites panel: Lites Across 3, Lites Vertical 5 | Works | DW-112, dialogs/opening.rs Lites tab |
| 88-89 | Hardware panel: Exterior Handle "Library" then Library button opens Select Library Object, type-filtered to Hardware, filter by folder "Knobs and Levers", pick "Exterior Handle 2" | Partial | Hardware tab exists (round 15 opening_tabs) with built-in handle choices; the library modal browser is missing (CB-395) |
| 89 | Interior Lock and Exterior Lock via the Library: Dead Bolt (interior/exterior), "Hardware Options: Lock" symbol-option filter | Missing | CB-395, DW-132; see DW-168 (lock hardware as library symbol options on doors) |
| 89 | Select Next Object edit button to reach a door under a stair or wall | Works | tools/select.rs Tab cycles |
| 89-90 | General panel Style drop-down + Library button; Select Library Object preview pane shows the door in its materials | Missing | CB-395; door style is a drop-down of built-in styles with the library door styles via DW-101 preview; library browser modal missing |
| 90 | An exterior-class door keeps exterior materials after being used as an interior door | Differs | our materials are per object spec; no class-based material defaults (below) |
| 90 | Default Settings > Materials > Edit (Material Defaults): select "Doors (Interior)" and Ctrl-click "Room Moldings", Select Material "Color - White" | Partial | Project.materials defaults per object class exist (plan-core model.rs Materials Defaults); default_pages/materials: not confirmed in code; same missing Select Material dialog |
| 90 | Preview pane rotates by dragging (four-headed arrow) | Partial | APP-86 |
| 90 | Casing panel: Exterior Casing Width and Depth match Interior | Works | casing_width / casing_depth per side in template `exterior_door`/`interior_door`, Casing tab |
| 91 | Exterior Casing Profile > Library > "Casings" folder (Core Catalogs > Architectural > Moldings > Casing), "BM11 - Base Molding" | Missing | no library molding profile pick on doors (CB-343, CB-395) |
| 91 | Default Settings > Window > Edit: General Window Type "Single Casement", Width 30", Height 54"; Lites 3 across 4 vertical; Rough Opening additional 1"; Framing panel header and trimmers | Works | template `window` (32 x 72 sill 24, lites 1 x 1, type Single Casement); dialogs/opening.rs. Defaults differ: see the defaults table |
| 91 | Build > Window > Window; click the Foyer wall left of the door; place 11 windows | Works | DW-1 |
| 92 | Window General: Window Type "Fixed Glass", Width 12", Height 77", Floor to Top 80" | Works | dialogs/opening.rs lines 828-838 ("Floor to Top", "Floor to Bottom", Elevation Reference); window types DW-165 |
| 92 | Lites: Lites Across 1, Lites Vertical 5 | Works | DW-120 |
| 92 | Height 36" makes Floor to Bottom 44" | Works | DW-54 size fields recompute |
| 92 | Elevation Reference "Absolute" with Elevation at Top 80" | Partial | an Elevation Reference row exists; the full Absolute / From Floor / From Finish set: R-89 Missing |
| 93 | Center Object edit button: the room under the cursor highlights, a dashed centring axis shows at the wall midpoint; hover the Room Divider to centre on it; click | Works | S-53 `center_opening_in_wall`, `center_between_walls`, `center_in_room` (S-46 family) |
| 93-94 | Centre the door against the garage stairs (horizontal axis down the stairs), then centre the kitchen, living, dining and bedroom windows, closet openings, garage door | Partial | centre on rooms/walls Works; centre on another object's axis (stairs): not confirmed in code transform.rs |
| 94 | Dimension Defaults: Setup Automatic panel, Exterior Auto Refresh checked | Missing | Lesson 1 (Auto Refresh). The guide says Auto Exterior Dimensions refresh as openings are added: absent |
| 95 | Locate Auto Exterior panel: Openings "Sides" | Works | template `locate_openings`, DIM-4, s17 |
| 95 | Locate Manual panel: Openings "Sides" | Works | DIM-54 Partial; `locate_openings` |
| 95 | Layer panel: manual dimensions on "Dimensions, Plan" | Partial | layer name per saved default: check; DIM-40 |
| 95 | Select a window; click the dimension that states its distance from the other window; type 4 1/2" | Works | DIM-32; s17 `a_dimension_to_a_window_follows_the_window_when_it_slides` |
| 95 | Shift-select two windows; Center Objects edit button centres both as a group in the room | Partial | S-53 centres one; centring a multi-selection as one group: not confirmed in code (transform.rs) |
| 96 | Auto Exterior dimension values differ left and right because wall thicknesses differ; Interior Dimension gives equal values | Works | DIM-14, DIM-44 |
| 96 | Manual Dimension along the Bedroom/Bath wall; set the door 6" from the closet wall by dimension | Works | DIM-32 |
| 97 | Select the window; Copy/Paste edit button; Reflect About Object; hover the door: dashed reflection axis; click | Works | S-48 |
| 97 | Transform/Replicate Object: tick Copy, Number of Copies 2, tick Move, Y Delta -35 1/2" | Works | S-103, S-104; dialogs/transform.rs |
| 98 | Shift-add the original and centre the three windows in the room | Partial | as above |
| 98 | Mulled unit: change the middle window to Fixed Glass; first window: Component Options "Right Hinge" instead of "Left Hinge" | Partial | window types per opening; a Component Options hinge-side choice on a casement: dialogs/opening.rs hinge_at_end |
| 98 | Shift-select three windows; Make Mulled Unit edit button | Works | DW-51, opening_edit.rs Mull |
| 99 | Default Settings > Schedules > Door Schedule > Edit: Columns/Rows: Display Column Headings; Available Columns "Dimensions" Add, Move Down to sit between Floor and Size; add "Top"; Remove "Size", "3D Exterior Elevation", "Code", "Manufacturer" | Partial | schedule defaults per kind (L-31, default_pages/architectural.rs schedule pages); door schedule fields (schedules.rs DOOR_FIELDS): "Dimensions" and "Top" columns and the Add/Remove/Move list with insertion under the selected item: not confirmed in code; "3D Exterior Elevation" column: Missing (graphic column) |
| 99 | Window Schedule General panel: add Dimensions, Top; remove 3D Exterior Elevation, Size, Egress, Code, Manufacturer | Partial | same |
| 99-100 | Main Text Style: Use Layer for Text Style | Partial | TXT-17 |
| 100 | Project Browser > CAD Details: right-click "Schedules Detail" > Open View | Partial | our template has no pre-made "Schedules Detail"; L-41 |
| 100 | Tools > Schedules > Door Schedule, click to place; Window Schedule below | Works | ToolId::ScheduleVariant(Door/Window) |
| 100 | Shift-select both; Align/Distribute Objects dialog: Move Objects Horizontally to Left Edges | Partial | S-54, S-149 |
| 100 | Window > Tile Vertically; plan shows callout labels with schedule numbers | Missing | APP-41; callout labels with schedule numbers exist (`show_labels`, schedules.rs) |
| 100 | Window Schedule Specification: Label panel "Use Label" radio; remove "Number" column; plan returns to original labels | Partial | schedules.rs `show_labels` and `label_prefix`; the Use Label / schedule-number radio: not confirmed in code dialogs/schedule_spec.rs |
| 100 | Schedules sorted by the Labels column by default; two ways to change the order | Partial | schedules.rs `sort`, `numbering` (L-31) |
| 101 | Preferences > Text panel: Number of Segments for Leader Lines; Create Rich Text off makes simple Text | Missing | PR-20 Partial; leader segment count and Create Rich Text switch: see PR-20 |
| 101 | Open Object > Object Information panel: Comment "20 minute" + Enter + "self-closing" | Works | dialogs/opening.rs line ~1677 Comment (multi-line), DW-100 |
| 102 | Move the Garage room label down | Works | R-44 |
| 102 | CAD > Text > Leader Line: drag a line from the door, drag a second segment; Rich Text spec opens | Works | TXT-5; segment count behaviour: PR-20 |
| 102 | Rich Text spec: Insert Macro button > Referenced Object > Comment; the text shows the door's comment, and the Door Schedule Comments column too | Missing | Rich Text dialog has no Insert Macro button; built-in macros are only room.*, plan.*, floor.* (plan-core text_styles.rs `BUILT_IN_MACROS`); no referenced-object macros such as %comment%. see TXT-65 |
| 103 | CAD > Text > Note near the sliding door; Note panel Text "tempered glass", Type General; numbered 2 because 1 exists; copy gets same number because the text is the same | Partial | TXT-9; numbering by identical text: not confirmed in code (note numbering by text equality) |
| 103 | Click the note shape; Copy/Paste edit button; click to paste | Works | S-40 |
| 103 | Open the Schedules Detail to see the updated Floor Plan Notes | Works | ScheduleKind::Note |
| 103 | File > Make a Copy "Chic Cottage-Windows" | Partial | APP-96 |

### Lesson 5 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Hinge and swing side by mouse while placing, by edit handles, and by Change Opening/Hinge Side and Change Swing Side | Partial | DW-8, DW-139 |
| Library attributes of doors: hardware, door style, materials | Partial | no library browser modal (CB-395) |
| Changing door and window width (handles, spec dialog) | Works | DW-26 |
| Transform/Replicate to create multiple windows | Works | S-47 |
| Copy/Paste and Reflect About Object for symmetrical windows | Works | S-48 |
| Where Auto Exterior Dimensions locate doors, windows and interior walls (Auto Exterior Dimension Defaults) | Works | DIM-4 |
| Controlling callout labels (Labels panel of the Schedule Specification) | Partial | schedules.rs `show_labels` |
| Comments in schedules and the %comment% referenced-object macro | Partial | Comment Works; macro Missing |
| Leader Line tool; Insert Macro button in the Rich Text Specification | Partial | TXT-5 Works; Insert Macro Missing |

## Lesson 6: Decks and Porches (pp. 105-126)

Workflow: pin the working plan, set Deck Railing, Railing and Half Wall defaults, Porch and Deck room defaults, Automatic Framing deck options, draw a porch with half walls and a deck with deck railings, move and extend auto dimensions, add terrain, show deck post footings, porch stem walls, exterior stair defaults, deck stairs and concrete stairs, notes and reordering of the note schedule, folders in the Project Browser.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 106-107 | Pin CHIC COTTAGE-CURRENT on the Dashboard; clear pinned and recent lists in Preferences > File Management | Missing | APP-67, APP-123; clear recent works (APP-5) |
| 107 | Deck Railings that define a room make it a Deck automatically; deck posts and beams generate on the floor below | Partial | a deck is a room whose name carries `deck` (plan-core deck.rs, room type "Deck"); automatic assignment when deck railings close a room: not found by that rule (the Room Type is set by hand or by the deck tool in Round 15) |
| 107 | Stairs placed outside a room draw downward and seek the terrain | Missing | CB-109 (no downward gesture; our deck stairs use DeckSpec.stairs `to_grade`, DECISIONS 207) |
| 107 | Template plans with Deck/Porch room-type defaults | Works | template `room_types` (46) in plan_defaults |
| 108 | Three railing kinds: Deck Railing, Railing, solid Half Wall; Deck Railing uses a single-layer wall type with no siding | Works | W-54, W-56, template `wall_variants` (deck_railing_type "Deck Railing-4", railing_type, half wall) |
| 108 | Default Settings > Walls > Deck Railing Defaults: General has No Locate checked | Partial | flag `no_locate` exists (walls.rs line 218) and Round 15 DeckEdge/DeckRailing styles; whether the deck-railing default sets it: not confirmed in code (default_pages/architectural.rs) |
| 108 | Wall Types panel: "Deck Railing/Fence" 3 1/2" single framing layer; Define; set thickness to 5 1/2" | Works | wall_types.rs (template has "Deck Railing-4" 4 in, not "Deck Railing/Fence"; defaults differ) |
| 108 | Rail Style panel, Newels/Posts: "Post to Beam" | Partial | W-127 (Round 15 walls2 brief builds the Rail Style tab) |
| 108-109 | Newels/Balusters panel: Post width 5 1/2"; Type from the Library ("Capped Post"); Balusters width 1 1/4", spacing 5" on center, type Square | Partial | W-116 Missing (tab disabled at the audit; Round 15 stairs2/walls2 builders); library type pick CB-395 Missing |
| 109 | Materials panel for the deck railing: multi-select Baluster, Beam, Rail, Capped Post Main; "Color - White" | Missing | see C-87 raised in Lesson 4 (Materials panel for railing walls); no Select Material dialog |
| 109 | Railing Defaults: No Locate checked; wall type "Interior Railing" (3 1/2" framing between two 1/2" drywall layers) | Partial | template `railing_type: Railing-4` (single 4 in); no Interior Railing three-layer type |
| 109 | Half Wall Defaults: Railing and No Locate checked; uncheck No Locate; Siding-6; tick Pony Wall with Lower Wall Type Stone-6 and Elevation of Lower Wall Top 20"; Height 16" affects only the upper wall | Partial | half wall height and pony split exist in `wall_variants` (half_wall_height, pony_split_height); pony half wall (a railing that is also a pony wall) as one wall: W-54/W-53 separately; combined: see W-155 |
| 109-110 | Default Settings > Room Types > Porch > Structure panel: Roof and Ceiling Over This Room checked; Floor Structure Edit: 4" concrete | Partial | R-99 (Room Type Defaults panels Partial); R-53; our room types hold function and finish but not floor structure/roof/ceiling flags per type (template `room_types` fields: name, function, include_in_living_area, conditioned, default_floor_finish) |
| 110 | Build > Wall > Straight Half Wall: two half walls enclosing the area outside the front door | Works | W-54, WallVariant half wall |
| 110 | Auto Exterior Dimensions update to locate the new walls (the entry door and side windows drop out) | Missing | Auto Refresh absent (Lesson 1); this behaviour shows only when auto exterior dimensions are live |
| 110 | Use the auto exterior dimension to position the porch wall 8 ft from the entry wall; drag the left wall's Move handle to align | Works | DIM-32, S-1 |
| 110 | Open Object on the new room; Room Type "Porch" | Works | R-37 |
| 110-111 | Default Settings > Room Types > Deck > Structure: Floor Finish Default unchecked (0 in); Planks/Joists Edit opens the Floor Structure Definition: Layer 1 Framing 5 1/2" wide Deck Planking, thickness to 1" | Partial | deck planking lives in DeckSpec (board width, gap, thickness) not in a floor structure layer stack; Floor Structure Definition dialog with layer roles: Lesson 1 see R-144 |
| 111 | Default Settings > Framing > Automatic Framing > Deck panel: Plank Gap Width 1/4" | Partial | DeckSpec.planking gap defaults to 0.25 (plan-core deck.rs line 65) edited in the room's Deck tab; there is no Automatic Framing Defaults page Deck panel (default_pages has no framing page: DS rows) |
| 111 | Deck Support panel: Deck Post Footings Height Above Terrain 6", Thickness 30" | Partial | DeckFraming has `footing_size` 18, `footing_thickness` 12 and `height_above_grade` 36 (deck.rs lines 102-130), not tied to a terrain surface; see CB-627 (footings sized to terrain: top above terrain, total thickness) |
| 111 | Build > Railing and Deck > Straight Deck Railing: three railings at the back; the enclosed room becomes a Deck with a label | Partial | WallVariant DeckRailing/DeckEdge (tools/wall.rs); the Deck room type assignment on closure: round 15 |
| 111 | Auto Exterior Dimensions do not update because deck railings are No Locate | Works | DIM-5 |
| 111 | Deck Room Specification General: Living Area excluded by default; Structure: Roof Over / Ceiling Over unchecked | Works | R-53, R-30; dialogs/room.rs lines 264-390 |
| 112 | Group-select three dimensions by marquee or Shift-click; Transform/Replicate: Question "turn off Auto Refresh?"; Move Y Delta 96"; Relative to Itself | Partial | S-47 Works; the Auto Refresh question is absent (no flag); "Relative to Itself" option: not confirmed in code dialogs/transform.rs |
| 112 | Edited automatic dimensions stay manual even if Auto Refresh is turned on again | Works | DIM-25, DIM-33 |
| 113 | Select an auto exterior dimension: square Move handle (red), diamond Add Extension Line handle; drag onto the deck railing's outer side | Partial | DIM-41 (ExtensionAdd mode), DIM-63 Partial (seven handles incl. Add Extension Line), DIM-59 Partial |
| 113-114 | Circular Add Segments handle beyond each end: drag to the garage wall; new extension lines locate door and side-lite sides | Missing | DIM-63 lists Add Segments as a handle: not present (not confirmed in code); tools/dimension.rs ExtensionAdd is a click mode |
| 114 | Extension Line End diamond handle: drag away to remove the extension line | Partial | DIM-41 `ExtensionDelete` is a click mode; handle drag-off removal: DIM-59 |
| 114 | Resize the deck by dimension: 36' / 36" / 8' typed | Works | DIM-32 |
| 115 | 3D > Create Perspective View > Floor Overview: porch slab; Mouse-Orbit; deck planking, joists, beams, posts and footings generated; footings at foundation level with no terrain | Partial | plan-3d deck.rs draws planking; framing members from the Build Deck Framing command (plan-framing deck.rs) |
| 115 | Terrain should be drawn on Floor 0 or 1 | Works | CB-43 |
| 115 | On Floor 1: Terrain > Create Terrain Perimeter; Window > Fill Window | Works | CB-43, TerrainVariant perimeter |
| 115 | Saved Plan View Control in the top toolbar: switch to "Terrain Plan View" | Partial | plan view selector exists (LAY-49); no "Terrain Plan View" in the template |
| 116 | Terrain > Terrain Specification > General: Absolute Elevation, uncheck Automatic; Retain Surface Elevation At "Reference Point"; Surface at Reference Point -28" | Missing | dialogs/terrain.rs has Subdivision, smoothing, contours, pad, materials, layer; no Absolute Elevation group. see CB-530, CB-531 |
| 116 | Terrain Elevation Reference Point marker (level-line) shows when the perimeter is selected; drag its Move handle onto a corner | Missing | see CB-508, CB-509; CB-47 covers the building pad reference only |
| 116 | Select the perimeter's top edge, click its temporary dimension, type 60' | Works | S-59, S-60 |
| 116-117 | Center Object on the terrain perimeter: horizontal axis across the wall; the exterior room outline highlights | Partial | S-53 centres openings/objects; centring a terrain perimeter on an exterior room edge: not confirmed in code; R-106 (Exterior Room object) Missing |
| 117 | Perspective Floor Overview: post footings now stop at the terrain | Missing | deck footings have no terrain link (see Deck Support row) |
| 117 | Down One Floor: deck framing and post footings display; the Working Plan View hides them | Partial | framing objects are layer-controlled; plan views are layer sets (LAY-49) |
| 117 | Active Layer Display Options: Name Filter "deck"; layer "Footings, Deck Post" Disp on | Partial | LAY-3 Works; layer "Footings, Deck Post" exists? not confirmed in code in plan-core layers.rs / framing layers |
| 118 | Tools > Reference Display on; Build > Wall > Straight Foundation Wall over the porch walls | Works | LAY-9, W-52 |
| 118 | Align with Wall Above edit button on each foundation wall | Missing | W-145 |
| 118 | Room Specification Structure: Stem Wall Height 37 1/2"; uncheck Floor Under this Room | Works | dialogs/room.rs "Stem Wall Height" line ~418, "Floor Under This Room" line 383 |
| 119 | Default Settings > Stairs and Ramps > Exterior Stairs > Newels/Balusters: library "Capped Post"; width 5 1/2"; uncheck Rail Passes Over Newel; height 44" | Partial | CB-99 (Interior/Exterior Stair Defaults); newel library pick Missing; Rail Passes Over Newel: see W-116, CB-395 raised in Lesson 4 |
| 119 | Materials: Baluster, Rails, Capped Post Main "Color - White"; Tread "Redwood"; Stringer "CA-B Pressure Treated Lumber" | Partial | free-text material per component; no Select Material dialog |
| 119-120 | Build > Stairs > Straight Stairs just outside the deck railing; Midpoint indicator on the railing; one click makes a flight to the terrain with a "DN" label | Partial | CB-101/CB-109 (stairs to grade from a deck room edge via DeckSpec.stairs; "DN" label only for the floor above, CB-33); no terrain-seeking one-click flight |
| 120 | A Doorway opening is added automatically to the deck railing at the top of the stair | Missing | CB-170 (Automatic Railing Openings), DW-144 |
| 120 | Staircase Specification General Width 5'; Railing panel: Railing On Left and Right; the stair resizes about its centre line and the doorway resizes with it | Partial | dialogs/stairs.rs Rails tab (applies-to both sides); the railing-opening link absent |
| 120-121 | Concrete porch stairs: Straight Stairs outside the porch wall; Center Object relative to the front door (vertical axis over the door) | Partial | S-53 centres openings in walls/rooms; centring a stair on a door axis: not confirmed in code |
| 121 | Exterior Staircase Specification: Width 6'; Style panel: Tread Overhang 0", uncheck Open Underneath and Open Risers; Railing On Left and Right | Partial | Style tab has "Open risers", Nosing, Tread Thickness, stringer styles (dialogs/stairs.rs lines 480-525); no "Open Underneath" or support-wall option |
| 121 | Materials: "Concrete" on Riser, Stringer, Support Wall, Tread, Underside, Wall Trim | Partial | component list is a text box per component (stairs.rs `materials`); the solid-stair components Support Wall/Underside/Wall Trim: Components tab (CB-?) not confirmed in code |
| 121 | Perspective Floor Overview to check | Works | C-11 |
| 122 | Rich Text "concrete steps to grade"; second text "framed steps to grade" | Works | TXT-4 |
| 122 | Note "exterior landing 36" minimum in direction of travel", numbered 3 by creation order | Partial | Note numbering by creation order: not confirmed in code (note types NoteType prefix numbering) |
| 122-123 | CAD Detail Management; open the detail; Tile Vertically; select the Floor Plan Notes schedule; square Move Row handles in the first column; drag a row to reorder | Missing | L-63 (schedule edit handles Missing); APP-41 |
| 123 | Note's edit toolbar: Move Up in Schedule and Move Down in Schedule buttons | Missing | L-63; see L-63 (the two edit buttons) |
| 124 | File > Make a Copy "Chic Cottage-Porches" | Partial | APP-96 |
| 124-125 | Project Browser: right-click project > New Document Folder; name; Shift-select documents; drag into the folder | Missing | APP-95 (project folders), APP-110/APP-117; no multi-document Project Browser |

### Lesson 6 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Deck Railings: single-layer wall type; room becomes a Deck automatically | Partial | template wall type; auto room type: round 15 |
| Deck versus Porch (framed floor, no roof or ceiling versus slab, ceiling, roof) | Partial | R-30, R-40, R-99 |
| Add Extension Line handle (diamond) and Add Segments handle (circle) | Partial | DIM-63, DIM-59 |
| Remove an extension line (Extension Line End handle) | Partial | DIM-41 |
| Include a room in the Living Area (room spec General; Room Type Defaults) | Works | R-42, R-53 |
| Deck plank gap and width (Deck Room Defaults Deck panel; Room Specification) | Partial | DeckSpec in the room dialog; no defaults page panel |
| Straight Stairs for deck and porch stairs | Partial | CB-101 |
| Reorder note numbers: Move Row handle; Move Up/Down in Schedule | Missing | L-63 |

## Lesson 7: Roof tutorials, automatic roof styles (pp. 127-135)

Workflow: a 34 ft by 24 ft rectangle of exterior walls drawn clockwise, a Perspective Full Overview tiled beside the plan, then the same shell re-roofed ten ways by editing the Roof panel of the exterior walls (Hip, Gable, Dutch Gable, Shed, Offset Gable, Gambrel, Gull Wing, Half Hip, Mansard) and Build Roof.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 127 | File > Close All; File > New Plan; draw a 34 x 24 ft clockwise rectangle with Straight Exterior Wall | Works | APP-10, W-1; s01_house_shell draws a 40 x 30 shell |
| 127 | 3D > Create Perspective View > Perspective Full Overview | Works | C-10 |
| 128 | Window > Tile Vertically (plan + overview) | Missing | APP-41; this tile is the lesson's working layout |
| 128 | Default roof: a plane on every exterior wall without a room-defining wall above it (hip), at the Build Roof pitch | Works | RF-1, RF-30 |
| 128 | Group-select walls with Shift and open the Wall Specification Roof panel: Roof Options, Pitch Options, Overhang, Auto Roof Return, Lower Wall Type if Split by Butting Roof | Works | RF-18, RF-28, W-83; dialogs/wall.rs `roof()` |
| 128 | Edit > Reset to Defaults: Reset Scope All Floors, Roof Directives in Walls | Differs | DS-41: our Reset to Defaults resets the defaults tree; no scoped reset of wall roof directives; the Wall Roof panel's own Hip choice clears one wall (RF-19); see DS-41 (bulk reset of roof directives) |
| 128-129 | Attic walls fill the space below the roof (the gable triangle is an attic wall); Layer Display Options (`~` key) "Walls, Attic" unchecked | Partial | RF-31 Partial (attic walls generated at scene time, not stored, not a selectable layer); the "~" key opens the layer dialog only if bound in Daniel's set (hotkeys) |
| 129 | Delete a roof plane; Build > Roof > Delete Roof Planes; Edit > Delete Objects with Roof Planes ticked; warning to turn off Auto Rebuild first | Partial | RF-40 Works; S-88 category dialog Works; the Auto Rebuild warning is absent: a deleted plane comes back at the next rebuild only for automatic planes (RF-6) |
| 129 | Auto Rebuild Roofs off by default in this tutorial; Build Roof > Roof panel checkbox | Works | RF-2; our default `auto_rebuild: true` (roof_view.rs line 912): differs from the tutorial's off state |
| 129 | Hip roof: Build > Roof > Build Roof, tick Build Roof Planes, OK | Works | RF-1, s06_roof |
| 130 | Gable roof: Shift-select the two end walls, Roof panel Full Gable Wall (or the Change to Gable Wall(s) edit button; Change to Hip Wall(s) removes it) | Partial | Full Gable Wall is available as an edit-toolbar command on selected walls (roof_view.rs WALL_ROOF_COMMANDS: Hip Wall, Full Gable Wall, High Shed/Gable Wall, Knee Wall, Dutch Gable Wall) but only when the selection includes an Exterior wall; the Wall Specification over several walls has no Roof tab (dialogs/wall.rs WALL_TABS_MULTI), so Dutch/Upper Pitch/Pitch for a group of walls cannot be typed once. BREAK: the Gambrel, Gull Wing, Half Hip and Mansard recipes need one wall at a time |
| 131 | Dutch gable: Dutch Gable Wall with Starts at Height 180" | Partial | per wall in the Wall Specification Roof tab (RF-21); for the two end walls together: not possible (no Roof tab for a group, WALL_TABS_MULTI) |
| 131 | Shed roof: end walls Full Gable, one wall High Shed/Gable Wall, Build Roof pitch 2 in 12 | Works | RF-22 |
| 132 | Offset gable: the lower wall's own Pitch 12 in 12 with Roof Options unchecked; Build Roof pitch 2 in 12 | Works | RF-5 (per-wall pitch override) |
| 132-133 | Gambrel: side walls Full Gable; front/back walls Upper Pitch 6 in 12, Start Height 156"; "Active Defaults" icon on the Pitch field; Build Roof pitch 12 in 12 | Partial | Upper Pitch per wall works (RF-25); the guide's step 3 selects the two walls together and opens one Roof panel: not possible with several walls |
| 133-134 | Gull wing: Upper Pitch 12 in 12 at Start Height 125", Build Roof pitch 3 in 12 | Works | RF-25 |
| 134 | Half hip: end walls Full Gable with Upper Pitch 3 in 12 at 170" | Works | RF-25 (staged.rs) |
| 135 | Mansard: all four walls Upper Pitch 1.5 in 12 at 132", Build Roof pitch 24 in 12 | Partial | same: four walls need four dialogs |
| 135 | Find the start of an upper pitch: build with the lower pitch, section view, CAD > Points > Place Point, Point-to-Point Dimension from the floor or the baseline to the point | Partial | CAD-2 Place Point and DIM-15 Point to Point exist in plan; in a cross section view they do not (DIM-62 Missing) |
| 136 | Enter either Starts at Height or In From Baseline in the Wall Specification, Tab to update the other | Missing | dialogs/wall.rs offers only Starts at Height (line 1971); see RF-165 (In From Baseline field linked to Start Height) |
| 129,141 | Roof style buttons ("Roof Styles") in Build Roof | Works | RF-3 (Hip, Gable, Shed, Gambrel, Dutch Gable, Half Hip presets): beyond the tutorial's wall-by-wall method |

### Lesson 7 assessment topics (answer key p. 506, Tutorial 7 - Basic Roof Styles)

| Topic | Status | Evidence |
|---|---|---|
| The program generates a hip roof by default (a plane over every exterior wall without a wall above it) | Works | RF-1 |
| Auto Rebuild Roofs lives on the Roof panel of the Build Roof dialog | Works | RF-2 |
| Non-default pitch is set on the Roof panel of the Wall Specification (Exterior walls only in ours) | Partial | RF-5; Roof tab disabled for Interior-kind walls |
| A shed roof needs Full Gable Wall on the two side walls (and High Shed/Gable on one) | Works | RF-20, RF-22 |
| Gull wing: end walls Full Gable, bearing walls Upper Pitch with Start Height | Partial | RF-25 per wall (no group Roof tab) |
| Add Break divides a wall into two walls | Works | W-44 |
| Reset roof directives in walls to defaults | Differs | DS-41 |
| Hide attic walls by layer | Partial | RF-31 |
| Delete roof planes three ways | Partial | RF-40, S-88 |

## Lesson 8: Chic Cottage Roof (pp. 136-158)

Workflow: open the Roof Plan View (its own layer set and defaults), tile a Perspective Full Overview, set Roof Defaults with Auto Rebuild, make gable walls (including a nested gable over the porch from a new wall type and Set as Default), a reverse gable, a shed roof over the deck, a story and a half (Raise/Lower, Ignore Top Floor), repair the nested gable, a gull-wing porch roof, edit eaves with Auto Rebuild off, a curved roof plane joined to a larger plane, roof returns and frieze molding, move the display of roof planes to the floor above, roof annotations with an O.H. dimension.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 137 | Hotkeys: Build Roof, Add Break (3), Join Roof Planes (2) | Differs | Daniel's hotkey set (docs/chief-hotkeys-resolved.md), not Chief's defaults |
| 138 | Default Settings changes that affect roof height: ceiling heights and platform thickness | Works | R-56; roof_view follows floor heights |
| 138 | Project Browser > Plan Views > "Roof Plan View": right-click Edit View, Open View; two view tabs open; both views marked open | Partial | plan views per floor (LAY-49); no preset "Roof Plan View" in our template, and the browser marks open views (APP-35) |
| 138 | Saved Plan View Specification: General panel; Selected Defaults panel (Default Set, "Roof Plan Layer Set"); Reference Display panel | Partial | dialogs/plan_views.rs has layer set and reference display; Selected Defaults (saved defaults per view, Default Sets) Missing: LAY-49, DS-32 |
| 139 | Compare the two plan views tiled: labels, camera symbols, multiple wall layers differ; Active Layer Display Options shows the layer set per view ("Roof Plan Layer Set" vs "Working Layer Set"); wall colour grey versus black | Partial | layer sets per view Work (LAY-2, LAY-8); tiling Missing; the starter "Roof"/"Working" layer sets are not shipped in the template |
| 140 | Close a plan view: Close View in the browser, the tab's Close button, File > Close View | Works | APP-8, APP-10 |
| 140 | Go Up One Floor; Perspective Full Overview; Tile Vertically; Mouse-Orbit; the darker title bar marks the active view | Partial | tile absent (APP-41) |
| 141 | Default Settings > Roof > Roof Defaults: Roof panel Auto Rebuild Roofs on; Framing Method Rafters; Pitch 12 in 12; Structure panel; Fill Style panel "No Pattern" | Partial | dialogs/defaults.rs roof page, dialogs/roof.rs (Auto Rebuild, framing radios stored only: RF-2); Roof plane Fill Style default: not confirmed in code |
| 141 | Wall Specification Roof panel Full Gable Wall, or the Change to Gable Wall(s) edit button | Works | RF-18 |
| 142 | Reference Display; Straight Exterior Wall over the red porch half-wall lines on Floor 2 (nested gable); the new wall is a yellow-fill Stone-6 | Partial | reference display snaps (LAY-9); the wall fill colour shown in plan is the main-layer fill, which we lack (Lesson 1 Fill row) |
| 142 | Change that wall's type to Shingle-6 | Works | W-47 |
| 142 | Set as Default edit button: message that the Exterior Wall tool has been updated | Missing | W-121, DS-27 |
| 142-143 | Next wall drawn is purple Shingle-6 (default changed); confirm alignment with the half walls | Partial | after Set as Default; the colour cue again depends on layer fill |
| 143 | Horizontal wall: Full Gable Wall; Reference Display off (F9) | Works | RF-20 |
| 143 | Reverse gable: Add Break on the right wall behind the garage; the back part Full Gable; temporary dimension to 28' | Works | W-44, S-59 |
| 144 | Single ridge line across the structure when both gables match | Works | RF-11, RF-20 |
| 145 | Deck room: Roof Over This Room and Flat Ceiling Over This Room (Room Specification Structure panel) | Works | dialogs/room.rs lines 387-390 ("Roof Over This Room", "Flat Roof Over This Room"); the tutorial's box is "Flat Ceiling Over This Room": ceiling flag R-30 |
| 145 | Side deck railings Full Gable Walls; the horizontal deck railing's Roof panel Pitch 2 in 12 | Works | deck railing is an Exterior-kind wall (tools/wall.rs DeckRailing -> WallKind::Exterior) so its Roof tab is live |
| 146 | Build Roof dialog, Roof panel, Roof Height heading: Raise/Lower All Roof Planes 18 1/8"; Ignore Top (2nd) Floor | Partial | dialogs/roof.rs "Raise Roof Off Plate" and "Ignore Top Floor"; "Raise/Lower All Roof Planes" as a global offset: not confirmed in code (RF-2) |
| 146 | With Ignore Top Floor the roof builds over Floor 1 ceilings and uses Floor 1 wall directives | Works | roof_view.rs `build_floor(..., ignore_top_floor, ...)` |
| 146 | Back Clipped Cross Section; Tape Measure from wall top to roof underside equals the raise value | Partial | DIM-20 Works in plan; measuring inside a section: DIM-62 Missing |
| 146-147 | Front walls of garage and porch and both side walls set to Full Gable | Works | RF-20 |
| 147 | Close the gap behind the nested gable: drag the porch-gable wall's end handle to the right wall; small "Living" room over the garage front becomes an Attic room | Works | W-1 handles, R-37 |
| 148 | Add Break at the intersection to stop the attic-room wall building through the Shingle-6 wall | Works | W-44 |
| 148-149 | Gull wing on the porch: the half wall's Roof panel (Railing Specification) Pitch 4 in 12, Upper Pitch 12 in 12, In From Baseline 84" | Missing | the Half Wall tool makes an Interior-kind wall (tools/wall.rs line ~222) and the Roof tab is disabled for Interior walls (dialogs/wall.rs WALL_TABS has off("Roof")): pitch, upper pitch and In From Baseline cannot be set on the porch half wall. BREAK. see RF-166 |
| 149 | Lower porch roof gets a deeper eave to keep the same fascia height as the adjacent eave | Works | RF-14, RF-15 (fascia alignment): not confirmed in code; plan-3d eave.rs |
| 149 | Editing individual roof planes needs Auto Rebuild Roofs off | Works | RF-6, RF-37 (manual planes survive rebuild) |
| 150 | Select an eave edge; click its temporary dimension; Question: turn off Auto Rebuild Roofs? Yes; type 18" | Partial | S-59 temp dims on roof edges exist (RF-38); the Question prompt is absent (editing a roof edge under Auto Rebuild: the plane becomes manual, RF-37) |
| 150 | Add Break on a roof plane edge at a Square Endpoint Snap Indicator; drag the new corner handle to the porch side wall | Works | RF-38, CAD-95; roof plane edit handles |
| 151 | Roof Plane Specification, Roof panel: Lock radio beside Ridge Top Height; Curved Roof checkbox; Angle at Eave 0° (Tab updates Angle at Ridge and Radius to Roof Surface) | Missing | RF-61 (planes are planar), RF-115 (no ridge-top lock); see RF-61, RF-115 (Curved Roof checkbox with eave/ridge angle and radius) |
| 151-152 | Select the porch plane's edge; Join Roof Planes; hover the larger plane; click: curved valley | Partial | RF-41 Works for straight planes; curved valley Missing with the above |
| 152-153 | Wall on Floor 2: Auto Roof Return checked, Roof Type Hip, Include Frieze | Partial | RF-27/W-147 (return length); Roof Type Hip/other and Include Frieze: W-148 Missing |
| 153 | Build > Roof > Edit All Roof Planes: General panel values "No Change"; Frieze panel Add New | Works | RF-39 (AllPlanesDialog); Frieze panel: RF-86 Partial |
| 153-154 | Select Library Object (Moldings and Profiles), search Frieze, Show in Browser, "FZ02" profile; frieze type "Eave and Gable" | Missing | RF-86 (Frieze Board switch only); CB-395; see RF-86 (frieze profile pick with Eave and Gable / Eave / Gable types) |
| 154-155 | Move the display of roof planes: Roof Plane tool with Shift-marquee (7 objects), Display On Floor Above edit button; planes unchanged in the 3D overview; roof now on Floor 2 | Missing | no such edit button (`display_on_floor` not found); roof planes live on the floor they were built for; see RF-96 |
| 155 | Saved Plan View Specification: General panel, "Floor used whenever this view is opened" = 2nd Floor | Works | dialogs/plan_views.rs floor per view (LAY-49) |
| 155-156 | Roof Plan View Selected Defaults: Roof Default Set; Dimension Defaults titled "Roof Dimension Defaults"; Layer panel "Dimensions, Roofs"; Locate Manual: Walls at Wall Dimension Layer, CAD Objects Lines/Sides checked | Partial | named dimension sets exist (template `dimension_sets`); per-view active set (active_dimension_set) is plan-wide, not per view (DS-28, DS-32); Locate Manual panel DIM-54 Partial |
| 156 | Manual Dimension from the eave to the wall framing layer | Works | DIM-11, DIM-4 |
| 156-157 | Dimension Line Specification, Primary Format: uncheck Use Default Formatting, Units = inch symbol; Segments panel: Leading Text "O.H." | Partial | DIM-39 Partial (tabs), DIM-65 Partial (Additional Text and Segments panel) |
| 157 | Move Dimension Label handle drags the text (leader line for dimension text) | Partial | DIM-63 Partial (label handles) |
| 158 | File > Make a Copy "Chic Cottage-Roof" | Partial | APP-96 |

### Lesson 8 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Why use the Roof Plan View (no labels or text, lighter wall lines, text on "Text, Roofs" layer) | Partial | layer sets Work; the starter view and layer are not in our template |
| Change to Gable Wall(s) sets the side walls of a shed roof | Works | RF-18 |
| Add Break adds corners to roof planes | Works | CAD-95 |
| Join Roof Planes for valley, hip and ridge | Works | RF-41 |
| Initial roof height follows room ceiling heights | Works | roof_view.rs |
| Auto Roof Returns live on the Roof panel of the Wall Specification and need a Full Gable Wall | Partial | RF-27, W-147 |
| Layer for dimensions in the Roof Plan View ("Dimensions, Roof") | Partial | layer names per saved default (DIM-40) |
| Non-default number format: Primary Format panel, Use Default Formatting off | Partial | DIM-39 |

## Lesson 9: Dormers (pp. 159-176)

Workflow: make a design-option copy of the plan, set Dormer and Window defaults, place an Auto Floating Dormer and position it by centring and temporary dimension, edit its window, create a knee wall and Attic room, place a structural Auto Dormer, draw a dormer manually (roof hole, walls, window, roof planes, joins, reshaped hole), annotate the roof plan with Rich Text, a CAD box parallel to a valley, leader lines, and file the plans in a Project Browser folder.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 159-160 | File > Make a Copy "Chic Cottage-Dormer Option"; close the current plan; open the copy from the Project Browser | Partial | APP-96; the copy is a separate file opened from File > Open (no project tree) |
| 160 | Roof Plan View is the working view | Partial | no starter Roof Plan View |
| 161 | Default Settings > Dormer > Roof panel Framing: Frieze checked; Walls panel Wall Type Shingle-6, Width 96" | Partial | dormer defaults page (default_pages/architectural.rs dormer, RF-48); dormer wall type and width fields: not confirmed in code; Frieze under Framing: RF-86 |
| 161 | Window Defaults: do not change Window Type (dynamic); Width 72", Height 48", Floor to Top 93 1/2", Lites Vertical 3 | Works | template `window`; DW-54. The tutorial note that the Window Type default is "dynamic" and affects existing windows: not mirrored (ours applies defaults to new windows only) |
| 161-162 | Build > Roof > Auto Floating Dormer; click once in the room next to the stair | Works | RF-49; tools/roof.rs |
| 162 | Select the dormer by its front or side wall; Select Next Object if the window gets picked | Works | tools/select.rs Tab cycle |
| 162 | Center Object: highlight the room, vertical axis, click | Works | S-53 `center_in_room` |
| 162 | Temporary dimension from the dormer front wall to the exterior wall: type 2' | Works | S-59 |
| 163 | Dormer window: Window Specification, Double Casement, Width 72, Height 48, Floor to Top 56"; Component Options Left Hinges / Right Hinge | Partial | DW-165 (window types); Component Options per side: dialogs/opening.rs hinge_at_end on a single leaf; multi-sash hinge options: not confirmed in code |
| 163 | Full Camera view of the dormer interior: none of the dormer walls reach the floor | Works | RF-49 (floating dormer records) in plan-3d |
| 163 | Active Layer Display Options: Name Filter "ceiling"; show "Ceiling Break Lines" | Missing | RF-80, RF-95; no ceiling break lines layer (grep "Ceiling Break" finds only the Chief importer) |
| 163 | Tape Measure from the Ceiling Break Line to the wall (8' 2" with a 12:12 pitch) | Missing | RF-80 |
| 163-164 | Straight Interior Wall across the back near the break line; Wall Specification Roof panel Knee Wall; temporary dimension 8' | Missing | Knee Wall is on the Roof tab, which is off for Interior walls (dialogs/wall.rs WALL_TABS); the edit-toolbar Knee Wall command shows only when an Exterior wall is in the selection (roof_view.rs `wall_edit_actions`). BREAK: knee wall + Attic room recipe. see RF-166 |
| 164 | Narrow room beside the knee wall: Room Type Attic | Works | R-37 |
| 164 | Build > Roof > Auto Dormer; click in the Attic room | Works | RF-48 |
| 164 | Edit handle over the dormer's front wall: temporary dimension measures from the edge of the Deck roof, not the back wall | Partial | S-60; dimension anchoring to roof edges (RF-38) |
| 164-165 | End to End Dimension from the exterior wall to the dormer front; type 2' | Works | DIM-13 End to End, DIM-32 |
| 165 | Modify the dormer window to Double Casement; Full Camera view: all three dormer walls bear on the floor | Works | RF-48 |
| 165 | Structural dormer by interior conditions (KB-00449) | Out-of-scope | external knowledge base article |
| 166 | Tile a Perspective Full Overview with the plan | Missing | APP-41 |
| 166 | Build > Roof > Roof Hole; drag a rectangle over the attic and knee wall inside a single roof plane | Works | RF-42 |
| 166 | Add Break on the hole's front edge; drag the Reshape handle toward the front, not past the ridge | Works | CAD-95, RF-42 hole is a polygon with vertex handles: not confirmed in code the add-corner handle |
| 167 | Straight Exterior Wall inside the hole from the knee wall up; two more walls form a small room; the wall extends up through the hole | Works | RF-34, RF-42 |
| 167 | Knee wall: Add Break, Sticky Mode edit button (keep the Break tool active), click the two intersections; select the middle segment and Delete | Partial | W-44 Add Break; Sticky Mode Missing (S-137); the same result takes three separate Add Break activations |
| 167-168 | Temporary dimensions set the dormer walls: 3', 8', 2' | Works | S-59 |
| 168 | Build > Window > Window at the midpoint of the dormer front wall; Double Casement | Works | DW-1, DW-10 |
| 168 | Default Settings > Roof: Raise/Lower from Ceiling Height 0", Ignore Top Floor off, Eave and Gable Overhang 6" each; Gutter panel Delete removes the gutter profile | Partial | dialogs/roof.rs Roof Defaults (overhang, gutters on/off checkbox only); "Raise/Lower from Ceiling Height": RF-2 not confirmed in code; gutter profile table: RF-95 family Missing |
| 168-169 | Build > Roof > Roof Plane: drag the baseline along the wall's outside Main Layer (snaps), click inside to set pitch direction and ridge | Works | RF-35 (Roof Plane tool, baseline first) |
| 169 | Select the top edge; temporary dimension to the wall; type -6" (negative moves the edge to the opposite side) | Partial | S-59 typed values; negative-sign semantics: not confirmed in code tempdim.rs |
| 169-170 | Second plane over the right wall; Join Roof Planes at the selected vertical edge of the left plane, hover and click the right plane: correct ridge | Works | RF-41 |
| 170 | Roof hole edges: select an edge, Resize handle drag snaps to the dormer walls | Works | RF-42; handles |
| 170-171 | Join Roof Planes between a dormer plane's bottom edge and the angled hole edge: valley | Partial | RF-41 joins plane edges; joining a plane to a roof-hole edge as the valley partner: RF-34/RF-42 (not verified) |
| 171 | Roof Plan View Selected Defaults: Edit Rich Text defaults "Roof Rich Text Defaults": Uppercase button; character size 6"; Print Size button opens the Print Size Calculator (1/4" = 1' gives 1/8" capitals); Appearance layer "Text, Roofs" | Partial | TXT-43 Partial (Print Size Calculator); per-view Rich Text defaults Missing (DS-28/DS-32); Uppercase Missing (Lesson 1) |
| 171-172 | CAD > Text > Rich Text "valley flashing"; Uppercase active; Rotate handle matches the valley angle | Partial | TXT-4, S-25; Uppercase button not found (`RichRun.upper` exists) |
| 172 | Current CAD Layer "CAD, Roof" from the Roof Plan View's Selected Defaults | Partial | CAD-1/LAY-6 current CAD layer is a project setting, not per saved view (LAY-49) |
| 172 | CAD > Boxes > Rectangular Polyline; resize an edge by dimension to 24 | Works | CadMode box; DIM-32 on CAD edges (CAD-94 Partial) |
| 172 | Double-click Make Parallel/Perpendicular edit button: dialog "Rotate entire polyline"; hover the valley: dashed parallel axis; click | Partial | CAD-54 (tools/cad/edit.rs MakeParallel/Perpendicular); the "double-click the edit button for the options dialog" and axis preview: not confirmed in code |
| 172-173 | Center Object along the valley's parallel axis; polyline resize handles | Partial | S-53; centring on a roof valley axis: not confirmed in code |
| 173 | Select polyline + text; Copy/Paste > Reflect About Object over the ridge line | Works | S-48 |
| 173-174 | Leader Line: drag a horizontal segment, then click once more to finish; Rich Text "flashing at wall" | Works | TXT-5 |
| 174 | Center Object on the leader text between the dormers by highlighting the attic room | Works | S-53 |
| 174 | Copy/Paste > Paste Hold Position on a leader line; drag its arrow-end handle to snap to the dormer wall | Works | S-83; leader handles |
| 174-175 | Group select both leaders; Reflect About Object on the Rich Text object (the text provides the axis) | Works | S-48 (reflect about object) |
| 175 | File > Close All Views (no revision needed for a design option) | Works | APP-10 |
| 175 | Project Browser: New Document Folder "Roof Tutorials"; Shift-select two plans; drag into the folder | Missing | APP-95, APP-117 |

### Lesson 9 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Auto Floating Dormer (non-structural) vs Auto Dormer (walls reach the floor) | Works | RF-48, RF-49 |
| Knee Wall between living area and Attic room | Works | RF-23 |
| Roof Defaults and Build Roof share defaults for manual planes | Works | RF-2 |
| Roof Hole tool within one roof plane | Works | RF-42 |
| Join Roof Planes also joins planes to hole edges | Partial | RF-41 |
| Print Size button in Rich Text Defaults | Partial | TXT-43 |
| Copy/Paste and Reflect About Object for mirrored annotations | Works | S-48 |

## Lesson 10: Custom Ceilings (pp. 177-189)

Workflow: set the plan-wide and per-room-type ceiling finish definitions, switch back to the Working Plan View, make a hat-channel framing type, member default and material, lower the basement ceiling through its Ceiling Finish Definition, view the finish in a section, remove a finish from one room, make a cathedral ceiling by clearing Flat Ceiling Over This Room, build a tray and a coffered ceiling out of soffits, annotate.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 178 | Soffit tool models straight-sided objects attached to walls | Partial | CB-17: Soffit is a cabinet kind (cabinet-like box, not a polyline soffit); template `cabinets.soffit` |
| 178 | Default Settings > Floors and Rooms > Floor/Ceiling Platform > Ceiling Finish Edit: Ceiling Finish Definition with two layers (drywall, paint colour) | Missing | finishes are one thickness plus a material name (R-27, FloorSettings `ceiling_finish_thickness` 0.625 in template); R-122 notes finishes are not layer stacks. see R-122 (Ceiling/Floor Finish Definition dialog with layer table, role and Insert Above/Below) |
| 179 | Room Types > Garage > Edit > Structure panel > Ceiling Finish Edit; layer 1 thickness 5/8"; Select Material "Drywall Fire Rated" from the Wallboard folder; Insert Above a 1 1/2" layer "Insulation Rigid" | Missing | R-99 (Room Type Defaults hold a default floor finish name only); the same R-122 gap; Select Material C-87 |
| 179-180 | Saved Plan View Control drop-down switches the current window between Roof Plan View and Working Plan View | Works | toolbar plan view selector (LAY-49); the preset views are not shipped |
| 180 | Default Settings > Framing > Framing Types: copy "U Channel" as "Hat Channel" (Steel, Steel U) | Missing | CB-213 Partial/CB-210 Missing: no Framing Type management; plan-framing has fixed member kinds |
| 180 | Framing Member Defaults Management: copy a member as "Hat Channel", Role Ceiling Joist, Type Hat Channel | Missing | CB-210 |
| 180-181 | 3D > Materials > Plan Materials: New; Define Material: Pattern panel material colour grey; Materials List panel Material Name "Hat Channel", Calculation "None" | Partial | C-61 Material Builder Works (colour, roughness, texture); a Plan Materials dialog and the Materials List panel (name for lists, calculation type): C-91 Missing |
| 181 | Plan Materials: select the new material, Add to Library: lands in the User Catalog | Partial | CB-58 User Library, Add to Library Works for objects; adding a plan material to the library from a plan-materials list: C-91 |
| 181 | Default Settings > Floors and Rooms > Floor Levels > Floor 0 > Structure panel > Ceiling Finish Edit; Insert Above a layer 1" thick, material "Hat Channel" from Library (User Catalog) or Plan Materials | Missing | R-122, C-87, C-91 |
| 182 | Ceiling Finish Definition: layer Role "Framing"; Structure: Construction "Hat Channel", Spacing 24" OC, Width 1" | Missing | R-122, CB-210, W-138 (layer roles) |
| 182 | Framing generation later uses the hat channels | Missing | depends on the framing type pieces above; plan-framing ceiling framing takes joist size/spacing from Build Framing only |
| 182-183 | Back Clipped Cross Section in the Foyer: closet ceiling one layer, garage ceiling two layers | Partial | C-19 Works; the section shows a single ceiling thickness |
| 183 | Remove a ceiling finish: Floor 0 room below the porch, Structure panel > Ceiling Finish Edit, delete each layer | Partial | set Ceiling Finish thickness to 0 in dialogs/room.rs (R-27); no layer table |
| 183 | Cathedral ceiling: Floor 1 deck, Back Clipped Cross Section drawn horizontally; Tile Vertically | Partial | C-19; tile Missing (APP-41) |
| 184 | Room Specification Structure: uncheck Flat Ceiling Over This Room; the deck's ceiling becomes the underside of the shed roof | Missing | our checkbox is "Flat Roof Over This Room" (dialogs/room.rs line 390; plan-core extras.rs `flat_roof`), which puts a level roof at the ceiling: a different feature. The vaulted-ceiling mechanism is Build Ceiling Planes (RF-46). No row for the room-level switch. see R-146 (Flat Ceiling Over This Room) |
| 184 | Roof Plane Specification: uncheck Use Room Ceiling Finish to use a different ceiling material per plane | Missing | RF-81 |
| 184-185 | Tray ceiling: Tray Ceiling Polyline tool or Make Tray Ceiling in Room edit tool (requires an uninterrupted flat ceiling) | Missing | R-111, R-107 |
| 185 | Default Settings > Cabinets > Soffit: Width 60", Height 12", Depth 36", Floor to Top 106 5/8", Use Floor Finish checked | Partial | template `cabinets.soffit` (width, depth, height, elevation 84); "Use Floor Finish" (height measured from the floor finish) and a Floor to Top field: CB-17; not confirmed in code dialogs/cabinet.rs |
| 185-186 | Build > Cabinet > Soffit: click along each wall; soffits snap to the wall side; two soffits either side of the stair wall | Works | CabinetKind::Soffit snaps to walls (CB-5..CB-3) |
| 186 | Extend a soffit across the room with its edit handles | Works | CB-8 resize handles |
| 186 | Camera view in the basement to see the tray | Works | C-4 Full Camera |
| 186 | Window > Swap Views returns to plan view | Missing | APP-40 |
| 186-187 | Coffered ceiling: extend a soffit with its Resize handle; Copy/Paste edit button and drag the Move handle to a wall; Copy/Paste > Reflect About Object about a soffit's top edge; Resize handles to snap to the stair | Works | S-47, S-48, CB-8 |
| 188 | Perspective Floor Overview to review | Works | C-11 |
| 188-189 | Rich Text on the Text layer: "36" x 12" soffits" (basement) and "2:12 cathedral ceiling" (Floor 1 deck) | Works | TXT-4 |
| 189 | Floor 2: roof annotations hidden in the Working Plan View; switch to Roof Plan View and back | Partial | per-view layer sets Work; shipped starter views absent |
| 189 | Show the "Ceiling Break Lines" layer; two Rich Text notes along the dashed lines | Missing | RF-80 |
| 189 | File > Make a Copy "Chic Cottage-Ceilings" | Partial | APP-96 |

### Lesson 10 assessment topics (the answers are on the first page of the next lesson)

| Topic | Status | Evidence |
|---|---|---|
| Default and per-room-type ceiling finish definitions | Missing | R-122 |
| Lowered ceiling through the Ceiling Finish Definition, without moving wall tops | Missing | R-122 |
| Cathedral ceiling = underside of the roof above | Missing | see R-146 |
| Tray and coffered ceilings from soffits or tray polylines | Partial | soffits Work, tray polyline Missing (R-111) |

## Lesson 11: Finish Materials (pp. 191-211)

Workflow: build custom library folders in the User Catalog, paint every interior wall with the Material Painter in Plan Mode, eyedrop to fix fire-rated walls, paint a room (Room Mode) and one wall (Object Mode), select a wall in 3D, add a wall covering, define Wall Material Region defaults, make a tile shower surround in a Wall Elevation and copy it, set flooring defaults plan-wide, per floor and per room type, move a floor finish with the Object Eyedropper, add a Floor Material Region, build a Room Finish Schedule, annotate.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 192 | Name brand material catalogs from Library > Get Additional Content | Out-of-scope | cloud download |
| 192 | Wall Elevation tool confined to one room | Works | C-20 |
| 192-193 | Library Browser > Folders: User Catalog > right-click > New > Folder ("Beach Palette") | In progress (Round 15) | plan-library manage.rs `create_folder`; briefs/r15/library.md builds the right-click New Folder/Rename/Delete; s52_library_r15 |
| 193 | Folders: Core Catalogs > Materials > Colors > Generic Colors > Blue; Filter Results panel reports 20 items; List Mode and Tile Mode toggles | Partial | CB-373 (Tile Mode), library dock has an Objects/Materials switch (shell/docks.rs); count line and list/tile toggle: not confirmed in code |
| 193 | Right-click "Color - Dew" > Copy to User Catalog; the copy is selected in the Folders panel | In progress (Round 15) | CB-58 Add to Library; the context command by that name: library brief |
| 193 | Shift-select five materials and drag them onto the "Beach Palette" folder | In progress (Round 15) | library brief (move between folders); see CB-391 if the drag-move of multiple items is not in its list |
| 193 | Material Painter has five Modes controlling the scope of a paint operation | Works | C-56; tools/materials.rs `PaintScope` (Component, Object, Room, Floor, Plan, Blend Colors) |
| 194 | Floor 1; 3D > Create Perspective View > Perspective Floor Overview; orbit up | Works | C-11 |
| 194 | 3D > Material Painter > Material Painter Plan Mode, then Material Painter; Select Material dialog (Library Materials panel, filter by "Beach Palette" folder); choose "Color - Butter" | Partial | Paint scope Plan exists (palette window); the modal Select Material dialog that opens when the painter starts is C-87 Partial; materials come from the Materials window/library dock, not a filtered modal |
| 194 | Paint roller cursor; one click paints nearly all interior wall surfaces | Works | tools/materials.rs apply_paint (Plan scope) |
| 195 | Adjust Material Definition on an interior wall surface on Floors 2 and 0 shows "Color - Butter" | Partial | C-58 (Adjust Materials lists parts of the selected object; a click-on-a-surface to open the material definition) |
| 195 | Two walls kept the fire-rated drywall; Material Eyedropper picks "Color - Butter" from a painted surface, then click either fire wall in Plan Mode | Works | C-57 |
| 195-196 | Room Specification Materials panel: tree of components, "Walls" component material, Select Material button; choose "Color - Fresh" | Partial | dialogs/room.rs Materials tab (R-36 Partial: per-room surfaces); component tree with Select Material: no modal |
| 196 | Material Painter Room Mode: paint the Bedroom walls "Color - Dew" | Works | PaintScope::Room |
| 196 | Material Painter Object Mode: choose a material straight from the Library Browser; click the wall between Dining and Bedroom | Partial | clicking a library material to make it the painter's active material: tools/materials.rs (active material); the Objects/Materials switch in the dock |
| 197 | Select a wall in 3D: Select Objects, click the Living room's exterior wall (selects the room first); Select Next Object edit button or Tab; Open Object | Works | view3d picking (s24_view3d_picking_textures); Tab cycles |
| 198 | Wall Specification Materials panel: a pony wall has four surface components (two exterior, two interior); "No Change" where the wall spans rooms with different finishes | Partial | dialogs/wall.rs has a live Materials tab that hands each layer or surface to the per-object paint (Project::sync_wall_materials_from, parity/walls.md); a pony wall's four surfaces and a No Change entry where the interior spans rooms with different finishes: not confirmed in code; C-60 |
| 198-199 | Room Specification Wall Covering panel: Add New opens Select Material; "Beadboard" folder > "White Beadboard"; Height 48", Floor to Bottom 0" | Partial | R-84 (Wall Covering tab live: wainscot entries); the material choice is by name; Select Material dialog missing |
| 199 | "Cabinets, Soffits" layer off so the beadboard is not hidden behind the coffer | Works | LAY-3; soffits are cabinets on a cabinet layer |
| 199 | Default Settings > Material Region > Wall Material Region > Edit: Structure panel "Cut Finish Layers of Parent Object" checked; Edit shows Material Layers Definition (backerboard, thinset mortar, tile) | Partial | dialogs/details.rs "Cut finish layers" check box (MaterialRegion.cut_finish_layers); a region is one material and thickness, not a layer stack (plan-core details.rs `MaterialRegion`); see CB-646 (Material Layers Definition for regions) |
| 200 | 3D > Create Orthographic View > Wall Elevation: drag a camera inside the Bath pointing at the left wall; only that wall of the room, objects between show | Works | C-20 |
| 200 | Build > Wall > Wall Material Region: click the wall surface; a region covering the entire wall is created | Works | tools/details.rs DetailsVariant WallMaterialRegion (W-59) in elevation views: not confirmed in code the tool places in a wall elevation view |
| 200-201 | Edge handle; click the temporary height dimension; type "-18" after the value (relative edit); lower the top by 12" | Partial | S-59 typed values; relative expression suffix editing: DIM-66 Partial (math in the inline field) |
| 201 | Wall Elevation pointed at the bottom wall; Region edges have two segments with diamond Reshape handles (the wall is two segments); Simplify Polyline edit button | Missing | CAD-101 Simplify Polyline Missing |
| 202 | Left vertical edge dimension to 32"; edges raised 18", lowered 12" | Works | DIM-32 on region edges: not confirmed in code |
| 202 | Region extends out past the drywall; select it (status bar names the object); Select Next Object if the wall is picked | Works | details.rs; Tab cycle |
| 202-203 | Copy/Paste > Reflect About Object over the room (horizontal axis) creates the opposite region | Works | S-48 |
| 203 | Custom backsplashes are a Wall Material Region that does not cut into the surface | Partial | CabinetKind backsplash is a cabinet setting (template `cabinets.backsplash`); see Lesson 15 |
| 203 | Flooring materials are Dynamic Defaults: changing the default updates rooms that use it; customised rooms keep theirs | Partial | DS-26 Partial; room Structure keeps explicit overrides only when set (extras.floor_structure empty follows default, dialogs/room.rs line 148) |
| 204 | Default Settings > Floors and Rooms > Floor/Ceiling Platform > Floor Finish Edit: Floor Finish Definition; Layer 1 name/pattern/texture cell opens Select Material filtered to the "Walnut" folder; pick "Red Oak 3-4-5" Plank - Natural"; all rooms except Bath, Garage, Porch, Deck update live | Missing | R-122 (floor finish is a thickness + material name, not a layer table); template `default_floor_material`; the live update behind the open dialog: Default Settings applies on Done |
| 205 | Floor 0's room uses the new default as well | Works | floors default material applies to all floors (FloorSettings.default_floor_material) |
| 205 | 2nd Floor Defaults Structure panel: Floor Finish Default check box; unchecking after picking carpet "Carpet-01 Dover" ties Floor 2 to its own finish | Partial | dialogs/floor_defaults.rs has a Finish section (thicknesses and "Floor Material" per floor?); the per-floor "Default" check box and carpet pick: not confirmed in code |
| 205 | Room Types > Bath > Structure panel: Default check box unchecked; Floor Finish Edit shows Ivory Tile, Thinset Mortar, Backerboard | Missing | R-99, R-122 (room types carry `default_floor_finish` name only) |
| 206 | Tools > Object Painter > Object Eyedropper; click the Bath floor; Select Properties to Paint edit button; Select Properties to Load: Clear All, tick Floor Finish; click the Kitchen floor | In progress (Round 15) | briefs/r15/painters_spell.md (Object Painter / Eyedropper); S-189 Partial (dialogs/painters.rs); the Floor Finish property depends on layered finish (R-122) |
| 206 | Kitchen Room Specification Structure: Define for Floor Finish now shows tile, thinset and backerboard | Missing | R-122 |
| 206 | Material Painter in Object Mode to replace the surface | Works | PaintScope::Object |
| 207 | Window > Tile Vertically (overview + plan) | Missing | APP-41 |
| 207 | Build > Floor > Floor Material Region: drag a rectangle in the Foyer | Works | R-72 |
| 207 | Material Region Specification, Structure: Cut Finish Layers of Parent Object; Edit: Material Layers Definition: Insert Below copy, Move Down, Layer 3 "Backerboard" from Plan Materials; Total Thickness 3/4" versus the room's 7/8" finish | Missing | see Wall Material Region row; C-91 Plan Materials |
| 208 | Increase Thinset Mortar to 3/8" | Missing | same |
| 208 | Line Style panel: Drawing Group "38 - Room Fill" | Partial | drawing groups exist (drawing_group.rs, LAY-36); the Line Style tab of material regions edits colour/weight/dash (dialogs/details.rs line 360), no group picker; our table numbers differ (CAD starts at group 80) |
| 208 | Resize: drag the bottom edge handle to snap to the door wall interior surface; temporary dimensions to 72" x 48"; Center Object in front of the entry door | Works | S-53, S-59 |
| 208 | Material Painter in Plan Mode with "Natural Slate Tiles" (Core Catalogs > Materials > Tile > Stone > Slate) | Works | PaintScope::Plan; core library has Tile folder: not confirmed in code names |
| 208 | Default Settings > Schedules > Room Finish Schedule: General > Categories to Include > Rooms: uncheck Attic, Deck, Porch | Missing | no category tree; room-type exclusion in schedules is by filter text (Lesson 3 see L-233) |
| 209 | CAD Detail "Schedules Detail": Tools > Schedules > Room Finish Schedule; Main Title "Room Finish Materials" | Works | ScheduleKind::RoomFinish (L-30) |
| 209 | Columns/Rows: multi-select with Shift and Ctrl, Remove all but Room Name, Wall Material, Floor Finish | Partial | schedule_spec.rs columns show/hide; multi-select (Shift/Ctrl) with Remove: not confirmed in code |
| 209 | Rich Text "concrete", copy to the garage, "1 x 5 1/2 composite decking" in the deck | Works | TXT-4, S-40 |
| 210 | File > Make a Copy "Chic Cottage-Finishes" | Partial | APP-96 |

### Lesson 11 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Material Painter Plan Mode paints all interior walls | Works | PaintScope::Plan |
| Assign a paint colour to a room: Room Specification or Material Painter Room Mode | Works | R-36, PaintScope::Room |
| Object Eyedropper transfers several attributes to a similar object | In progress (Round 15) | painters_spell brief |
| Wall Material Region versus Wall Covering | Partial | W-59, R-84, W-115 |
| Where the default floor finish for all rooms is set (Floor/Ceiling Platform Defaults > Floor Finish Edit) | Missing | no Platform Defaults dialog (Lesson 1 see R-143); R-122 |
| Default check box in the room Structure panel shows use of the default finish | Missing | R-122 |

## Lesson 12: Room Moldings (pp. 211-221)

Workflow: set default base, crown and chair rail profiles per floor (Floor Defaults Moldings panel), customise one room (Use Default off, Add New) and remove a molding, draw a rectangular molding profile with CAD and add it to the library, select the Exterior Room, make a Room Molding Polyline from it, edit that polyline per edge, list moldings in a Room Finish Schedule placed with Extension Snaps.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 212 | Each floor has an Exterior Room used to edit the floor's exterior and add perimeter moldings | Missing | R-106 |
| 212 | Selected Edge of a polyline-based object is editable in special ways | Missing | S-170 (Selected edge markers, Select Next Edge) |
| 213 | Default Settings > Floors and Rooms > Floor Levels > Floor 1 > Moldings panel: table of Selected Profile with Type (Base Molding); Replace, Add New, Delete buttons; Retain Aspect Ratio; per-profile Height field | Missing | dialogs/floor_defaults.rs has Heights, Finish and New Rooms sections only; room moldings are chosen per room (R-34 Partial). see R-117 (floor-level default molding table with Replace/Add New/Delete) |
| 213 | Room moldings are Dynamic Defaults | Missing | per-room only; DS-26 |
| 213 | Replace opens Select Library Object, filtered to Moldings and Profiles; search "base molding"; Core Catalogs > Architectural > Moldings > Base Molding; "BM08 - Base Molding" | Partial | room Moldings tab picks from a built-in molding list (rooms.rs MOLDING_LIBRARY `molding_defs`: base, chair, crown); library molding profiles from Chief's catalog (BM08, CM02, CR01) need the library browser modal (CB-395, CB-343) |
| 213-214 | Add New: "CM02 - Crown Molding"; Type "Crown Molding"; Retain Aspect Ratio; set its Height 3" | Partial | profile height/projection are fixed by the profile definition (dialogs/room.rs line ~713 shows "high, projection"); an editable height with aspect ratio: Missing |
| 214 | Interior Full Camera view shows the moldings | Works | C-4; plan-3d slab.rs `room_moldings` |
| 214 | Stairwells and Open Below rooms, porches/exterior rooms and garages (hybrid) get no default moldings | Partial | R-34; per-function default of moldings: function_defaults (R-40) |
| 215 | Room Specification Moldings panel: uncheck Use Default; Add New; "CR01 - Chair Rail Molding"; Type Chair Rail | Partial | R-34: three slots (Base, Chair Rail, Crown), no Use Default switch (an empty slot is None), no Add New list |
| 216 | Remove molding from a room: uncheck Use Default, select the CM02 row, Delete | Works | room Moldings tab: choose "None" for the slot |
| 216 | CAD > Boxes > Rectangular Polyline; Temporary Dimensions; width 1 1/2", height 11 1/4" via inline fields | Works | CadMode box; S-59 |
| 217 | Add to Library edit button on the polyline: a "Molding" item is added to the User Catalog | Partial | CB-72/CB-58: library "Add to Library" works for symbols and 3D solids; a CAD polyline becoming a molding profile: see CB-72 |
| 217 | Right-click the item > Rename ("2x12 Profile"); Open Object: Molding Specification General panel shows Height 11 1/4" and Width 1 1/2" | In progress (Round 15) | library brief right-click Rename and Library Object Specification (dialogs/library_object.rs); molding-specific panel: not confirmed in code |
| 217 | Delete the CAD polyline | Works | S-88 |
| 217 | Select Objects, click just outside an exterior wall; Select Next Object (Tab) until the Exterior Room is selected; band highlighted; status bar says "Exterior room" | Missing | R-106 (no Exterior Room object); Tab cycling exists |
| 217 | Perspective Full Overview tiled with the plan | Missing | APP-41 |
| 217-218 | Exterior Room selected: Make Room Molding Polyline edit button; dialog: Convert Molding "Blank Molding", Height 96" | Missing | R-110, R-107; W-111 draws a molding along a hand-drawn line/polyline instead |
| 218 | Molding Polyline Specification, Moldings panel: Add New "2x12 Profile"; Horizontal Position "Outside Polyline" | Partial | W-111 (Molding Line) with position options: not confirmed in code dialogs/details.rs molding page (lines 4, 743) |
| 218 | Orbit the overview: the molding wraps all sides | Works | plan-3d details.rs molding sweep |
| 219 | Click an edge of the molding polyline to make it the Selected Edge; Remove Molding from Selected Edge edit button (swaps to Add Molding to Selected Edge) | Missing | S-170; the molding line has no per-edge flag (not confirmed in code plan-core details.rs) |
| 219 | Molding Polyline Specification General: uncheck No Molding on Selected Edge | Missing | same |
| 219-220 | Drag a polyline edge's centre handle to the house wall; Add Molding to Selected Edge | Partial | polyline vertex/edge handles Work; edge-wise molding flag Missing |
| 220 | Snap Settings: at least one Extension Snap on; Tools > Schedules > Room Finish Schedule; hover over the first schedule's bottom-left corner: Endpoint indicator with an Extension anchor; move straight down: extension line and Extension Snap icon; click to place | Partial | S-125 Partial (extension snaps with anchors); snapping to a schedule table's corner: not confirmed in code (schedule is a selectable object with a bounding box) |
| 221 | Room Finish Schedule Specification General: Main Title "Room Moldings"; Objects to Include > Rooms: uncheck Open Below | Missing | no category tree (Lesson 3) |
| 221 | Columns/Rows: keep only Room Name, Base Molding, Chair Rail, Crown Molding | Works | schedules.rs ROOM_FINISH_FIELDS includes base/chair/crown (L-30) |
| 221 | File > Make a Copy "Chic Cottage-Moldings" | Partial | APP-96 |

### Lesson 12 assessment topics (p. 222)

| Topic | Status | Evidence |
|---|---|---|
| Default room moldings are specified per floor in the Floor Defaults dialogs | Missing | no Floor Defaults Moldings panel |
| Select the Exterior Room: click outside an exterior wall, Select Next Object until the status bar says Exterior Room | Missing | R-106 |
| Molding Polyline around the exterior, made with the Create Room Molding Polyline edit tool | Missing | R-110 |
| Extension Snaps align an object being drawn with a snap point of another object | Partial | S-125 |
| Room Finish Schedule tool lists interior moldings | Works | L-30 |
| Per-room molding override and removal | Partial | R-34 |
| Custom molding profile into the User Catalog | Partial | CB-72 |
| Remove molding from a selected edge | Missing | S-170 |

## Lesson 13: Interior Furnishings (pp. 222-236)

Workflow: Material Defaults for furniture and upholstery, library search by folder filter, place a bed, end tables and dresser (they snap back-to-wall), furniture sets, draw order with Drawing Groups, accessories on top of furniture, resize and re-material a picture frame into a mirror, Replace From Library on identical objects, place an accessory inside a furniture object with Ctrl, Architectural Blocks (make, edit component, explode, add to library), User Catalog folders and linked folders, a Furniture Schedule with a custom column, number formatting, blocks in schedules, Find Object in Plan, Include in Schedule, project folders.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 223 | Ctrl/Cmd while moving or resizing overrides snapping and movement restrictions | Partial | S-121 Partial (Ctrl suspends snaps); overriding the "occupied space" restriction when dragging an accessory through its host: see S-195 |
| 223 | C key while dragging = concentric edit behaviour | Works | S-65 Concentric |
| 223 | Create Architectural Blocks of furniture groupings and add to library | Missing | CB-427, CB-435 |
| 223 | Library Browser Advanced Search; right-click menu on an item | In progress (Round 15) | CB-54 Works (search and filters); right-click menu: library brief |
| 223-224 | Default Settings > Materials > Material Defaults: "Furniture" is "Birch (honey)"; "Furniture Upholstery" is "Chenille - Pebble"; set "Accent Upholstery" via Select Material with the "Fabric & Wall Coverings" folder: "Twill Cornflower" | Partial | Project.materials defaults per object class (dialogs/materials.rs "Materials Defaults"; C-60); class list names Furniture, Upholstery, Accent Upholstery: not confirmed in code; Select Material dialog Partial (C-87) |
| 224 | Search "queen" in the Library Browser: drop-down of matches; double-click the "Queen" folder filter; the Folders panel expands to Core Catalogs > Interiors > Furniture > Beds | Partial | CB-54 search and filters Work; folder-filter chips with location tool tips and the dropdown-of-suggestions: library brief |
| 225 | Select a bed: Preview panel, pointer shows a furniture icon, a preview outline follows with status bar info | Works | tools/library (Place Library Object), CB-54 preview |
| 225 | The bed outline snaps to the nearest wall with its back to the wall | Works | symbols snap to walls (placement ghost) per tools/library |
| 225 | Click to place; place two End Tables with Drawers ("Basket Drawer") and a dresser against the stair wall | Works | tools/library |
| 226 | Living Room Sets folder (Interiors > Furniture > Seating): sofa and arm chair from "Rita"; a coffee table | Works | library items from Daniel's Chief install at runtime (chief-library-format.md) |
| 226 | Dining table, dining side chair; Ctrl+drag the chair's Move handle partially under the table; the chair draws in front; status bar names Drawing Group "32 - Fixture/Furniture" | Partial | LAY-74 (Round 15 `DG_*` Bring to Front/Send to Back/Set Drawing Group); our table numbers differ from Chief's (32 vs ours): drawing_group.rs |
| 226 | Six chairs; View Draw Order Edit Tools edit button; Select Drawing Group reports 32; Move Forward / Move Backward within a group | Partial | LAY-74: Bring/Send Forward/Backward steps Missing |
| 227 | Accessories: vase "Tulips" placed on the dresser; mirrors and frames "Double Mat Frame" placed on the wall behind | Works | tools/library; wall snapping for wall-hung items: not confirmed in code |
| 227 | Full Camera in the Foyer facing the dresser | Works | C-4 |
| 227 | Window > Select Next Tab returns to plan without closing the camera view | Partial | APP-42 (tab cycling) |
| 227 | Coffee table shows ten edit handles for resize, reshape and rotate in plan | Partial | S-1 handle set for symbols (CB-8 resize/rotate, 4 extend + rotate + corner): not confirmed in code count |
| 228 | A frame selected on its side in a camera view shows four Extend handles; resize it; hold C for concentric; prompt to regenerate the frame's 2D symbol: No | Partial | 3D-view object resize: C-? 3D editing (S-? "edit objects in camera views", W-139 Missing for heights); regenerate-2D-symbol prompt Missing; see S-197 |
| 228-229 | Furniture Specification General: Rendering Technique (Standard) above the Preview pane; Finished Floor to Top 80"; Width/Height | Partial | dialogs/symbol.rs symbol spec (size, elevation); technique selector in the preview: APP-86 Partial, C-92 Missing |
| 229 | Materials panel: component "Image" > Select Material "Mirror" | Partial | symbol Materials tab with per-component text/drop-down; Select Material dialog |
| 229 | 3D > Material Painter > Component Mode; Material Eyedropper picks the mirror; the spray-can pointer paints the green mat | Works | PaintScope::Component, C-57 |
| 229 | Replace a library object: Wall Elevation in the Bedroom; show layer "Furniture, Interior" in Active Layer Display Options (layer set "Camera View Layer Set" in a camera view) | Partial | LAY-3 Works; per-view-type layer sets for camera views: LAY-2/LAY-8 Partial; layer "Furniture, Interior" name: not confirmed in code layers.rs |
| 230 | Select an end table; Replace From Library edit button: dialog with Replace "identical objects in room" option; Library button; search "bedside"; "Manning Bedside Table"; both tables replaced | Partial | CB-57 Works (tools/library/user.rs `replace_other`); the "replace identical in room / plan" scope options: not confirmed in code |
| 230 | Default furniture material does not apply to library items with their own material; Material Eyedropper applies lighter Ash wood to the tables | Works | C-57 |
| 231 | Accessory inside a furniture object: library "Baskets, Crates, Boxes" > "Storage Box"; click on the bedside table; in the elevation the box rests on top; Ctrl+drag it down to the lower shelf | Partial | placing on top of a host object by Z snap: tools/library place (CB-?) not confirmed in code; Ctrl override of occupied space: see first row |
| 231 | Architectural Block: Ctrl-click vase, wall frame and dresser; status bar "3 objects currently selected"; Make Architectural Block edit button; selected object reads "Architectural Block"; button becomes Explode Architectural Block | Missing | CB-427, CB-434; our S-35 Group/Ungroup is the nearest (no layer/label/spec) |
| 232 | Block moves as one; Undo restores | Works | group move (S-37) |
| 232 | Group the dining table and chairs into another block | Missing | CB-427 |
| 232 | Select a block component: click the vase (block selected), Select Next Object; edit its handles, tools and dialog | Partial | S-36 selects a group member with Tab; CB-438 (sub-object edit): Missing for blocks |
| 232 | Add to Library edit button adds the block to the User Catalog; right-click Rename "Foyer Table" (default name "Untitled") | Missing | CB-435; library Rename: Round 15 |
| 232-233 | User Catalog > New > Folder "Chic Cottage"; drag items into it ("Foyer Table", "Hat Channel", "2x12 Profile") | In progress (Round 15) | library brief |
| 233 | Copy a folder; Paste Link into another folder: the copied folder's materials stay linked to the originals | Missing | see CB-391 (linked library folders) |
| 233 | Default Settings > Schedules > Furniture Schedule: General panel: remove all columns but Qty and Description; add "3D Perspective" and move it to the top; add "Dimensions" and move to the bottom; Object Preview Options: Scale Images | Partial | ScheduleKind::Furniture (schedules.rs SYMBOL_FIELDS); schedule columns add/move: Works; a "3D Perspective" picture column and Scale Images: Missing (see L-234, with the 2D Symbol column in Lesson 4) |
| 234 | CAD Detail Management > Schedules Detail > Open; Tools > Schedules > Furniture Schedule | Works | L-41, ScheduleVariant(Furniture) |
| 234 | Schedule Specification Number Formatting panel: pick the "Dimensions" Format Column, units and accuracy; Smallest Fraction 1/1 | Missing | L-72 |
| 234 | Align the furniture schedule below the moldings schedule using Extension Snaps | Partial | S-125 |
| 234 | Architectural Block Specification: General "Treat as One Object"; Schedule panel: Include in Schedule As (Furniture > Indoor); Object Information: delete the %automatic_description% macro and type "Dining Set" | Missing | CB-436, CB-437; the %automatic_description% macro: not among BUILT_IN_MACROS (plan-core text_styles.rs); see CB-436, CB-437 |
| 234 | The schedule lists one "Dining Set" line replacing separate chairs and table | Missing | blocks |
| 234 | Select a schedule line item; Find Object in Plan edit button selects the object in plan and activates that window | Missing | S-118, L-65 |
| 234 | Furniture Specification Schedule panel: uncheck Include in Schedule (vase, storage boxes) | Partial | L-29 (Include in Schedule exists for doors and windows; symbols: not confirmed in code dialogs/symbol.rs Schedule tab) |
| 235 | Custom Object Fields tool for custom schedule columns | Missing | DW-118, W-118 (custom fields on the Object Information tab); no Custom Object Fields tool: see L-70 |
| 235 | File > Make a Copy "Chic Cottage-Furnishings"; Project Browser folders ("New Folder"): "Roof Tutorials", "Interior Design Tutorials"; drag plans into them | Missing | APP-95, APP-117 |

### Lesson 13 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Material Defaults for library furnishings (Furniture, Upholstery, Accent Upholstery) | Partial | C-60 |
| Two ways to find a library object: browse or search | Works | CB-54 |
| Replace From Library edit tool for all identical instances | Partial | CB-57 |
| Ctrl overrides movement restrictions | Partial | S-121 |
| What an Architectural Block is | Missing | CB-427 |
| Custom Object Fields for custom schedule columns | Missing | no tool |
| Block in a schedule needs Treat as One Object + Include in Schedule | Missing | CB-436 |
| Find Object in Plan from a schedule line | Missing | S-118 |
| Include in Schedule check box on the Schedule panel | Partial | L-29 |

## Lesson 14: Cabinet Styles (pp. 237-259)

Workflow: open the Kitchen & Bath Plan View, place a base cabinet, give it a library door style and bar pull, match those properties onto a wall and a full-height cabinet, set countertop and backsplash, add an edge profile, crown molding and light rail from the library, change cabinet materials with Default links and the Material Painter, use Set as Default, make a drawer base, a secondary drawer style, framed box with inset overlay, a split front with item widths, cabinet feet, and custom materials.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 238 | Material Defaults hold the cabinet door/drawer and cabinet box materials (Cabinet Door/Drawer, Cabinet) | Partial | Project.materials defaults per class (dialogs/materials.rs); cabinet Materials tab uses fixed stand-ins when "Default" (dialogs/cabinet.rs ~line 1537) |
| 238 | Match Properties and Set as Default edit tools | Partial | Match Properties and Apply Properties exist (tools/painters.rs, S-188 Partial); Set as Default is DS-27 Missing |
| 238 | Apply a door/drawer from the Library to a cabinet by selecting it and clicking the cabinet in plan or camera view | In progress (Round 15) | CB-57 Replace From Library; click-to-apply of a door style onto a cabinet face: library brief ("Replace from Library icon") ; see Round 15 library brief if absent |
| 238-239 | Cabinet Defaults set before placing; or place, customise, then Set as Default | Partial | CB-20 Works; Set as Default Missing |
| 239 | Saved Plan View Control: "Kitchen & Bath Plan View": layer set hides wall layers and manual dimensions, keeps room-label dimensions, smaller labels and camera symbol | Partial | plan view selector; the starter Kitchen & Bath view and layer set are not in our template (LAY-49) |
| 239 | Saved Plan View Specification, Selected Defaults panel: saved defaults named "Kitchen and Bath ..." | Missing | LAY-49, DS-28, DS-32 |
| 239 | Build > Cabinet > Base Cabinet: click in the Kitchen; back snaps to the wall | Works | CB-3 |
| 239-240 | Base Cabinet Specification, Door/Drawer panel: Library button under Door Panel opens Select Library Object, Type "Cabinet Doors/Drawers/Panels", folder Architectural > Cabinet Doors, Drawers, & Panels > Doors; choose "Beaded Recessed Panel Door" | Partial | CB-12 (door/drawer style drop-down from library .calib symbols by category "Cabinet Doors", dialogs/cabinet.rs Door/Drawer tab); the modal browser with folder location: CB-395 Missing |
| 240 | Door Handle Main Style drop-down: None, three parametric knob/handle types, Library; "CP04 Bar Pull"; Vertical Position: Distance from Top 6" | Partial | HandleStyle choices (dialogs/cabinet.rs handle_combo); `handle_from_top` Distance From Top and Distance From Edge fields exist (lines 1131-1144); library handle symbols: Missing (CB-12/CB-395) |
| 240 | Build > Cabinet > Wall Cabinet; Full Height cabinet | Works | CB-1 |
| 240-241 | Match Properties dialog: Search field filters attributes ("door", "handle"), tick Main Door Style and Main Door Handle Style | Partial | tools/painters.rs Match Properties loads an object; the searchable attribute list: S-189 Partial (dialogs/painters.rs lists attributes; search: no spec) |
| 241 | Apply Properties edit button then click targets: each target joins a group selection | Partial | S-188 (apply by click Works; the "targets join the selection set" cue: not verified) |
| 241 | Adjust handle vertical position on wall and full-height cabinets (Up From Bottom / Distance From Top) | Partial | as above |
| 241-242 | General panel: Countertop Thickness/Overhang fields carry the Active Defaults icon (red check = linked to the Cabinet Defaults); typing "3 cm" converts, then "1 1/4"" | Partial | units parse `3 cm`, mm and m (plan-core units.rs parse_length); no per-field Active Defaults indicator (DS-26) |
| 242 | Entering a value breaks the link (icon loses its red check) | Missing | DS-26 |
| 242 | Backsplash Height 4"; tick Side | Works | cabinet backsplash fields (dialogs/cabinet.rs `bs_thick`, height), template `backsplash` |
| 242-243 | Moldings panel: Add New opens Select Library Object (Moldings and Profiles): "EM05 - Edge Molding"; Retain Aspect Ratio; Height 1 1/4"; rename "EM05 - Counter Edge Profile" | Missing | cabinet Moldings tab offers "Add Crown" and "Add Light Rail" with Projection and Height fields only (dialogs/cabinet.rs lines ~1488-1525); countertop edge is an EdgeProfile enum; CB-344, CB-350 |
| 243 | New moldings take the cabinet box material | Partial | Materials tab "Molding" component |
| 243 | Perspective Full Camera pointed at the cabinet | Works | C-4 |
| 243 | Wall Cabinet: placed at its default height snapped on the wall (Wall Cabinet Defaults) | Works | template `cabinets.wall.elevation 54`; CB-6 |
| 243 | General panel Elevation Reference "From Ceiling", Ceiling to Top 0" | Missing | cabinet General has Finished Floor to Bottom / Top only (dialogs/cabinet.rs lines ~534-545); R-89 Missing |
| 243-244 | Crown molding "CM02 - Crown Molding" from the library; Retain Aspect Ratio; Height 3"; Vertical Offset 3"; Type Crown Molding; Position on Object From Top | Partial | "Add Crown" (fixed built-in profile, projection, height; crown sits on top); Type / Position on Object / Vertical Offset fields: CB-344 Missing |
| 244 | Ceiling to Top math: place the cursor after 0", press Space, type "+ 3"", Tab: the field evaluates | Missing | length fields parse a plain length with a unit suffix (plan-core units.rs); no `+`/`-` expression evaluation in dialog fields; DIM-66 covers math only for dimension values. see APP-145 (arithmetic in dialog number fields) |
| 245 | Match Properties/Apply Properties apply the crown to the full-height cabinet | Partial | as above |
| 245 | Light rail two ways: raise the lowest Separation face item's Item Height, or a molding "L-Bracket Small" (Brackets and Channels): Type Light Rail, Position on Object Bottom, Profile Rotate 180, Width/Height 1", Horizontal and Vertical Offset -1", rename "Light Rail" | Partial | "Add Light Rail" exists (light rail under the cabinet along the front with returns); Profile Rotate, Offsets, custom library bracket profile: Missing (CB-344, CB-350) |
| 247 | Materials panel: components list with "Default: <material>" prefix meaning a dynamic link to the Cabinet Defaults; Shift-select "Cabinet" to "Cabinet Door/Drawer"; Select Material from the User Catalog "Beach Palette" | Partial | dialogs/cabinet.rs Materials tab: Box, Door Fronts, Drawer Fronts, Countertop, Backsplash, Toe Kick, Molding combos with "Default keeps the part's usual stand-in"; no "Default:" dynamic prefix, no multi-select, no Select Material dialog |
| 247-248 | Material Painter Object Mode replaces every instance of "Pewter Tankard" on the cabinet; Component Mode + Eyedropper applies the countertop material to the edge profile only | Works | PaintScope::Object / Component, C-57 |
| 248 | Match Properties with keyword "material" copies materials to other cabinets | Partial | S-189 |
| 248 | Select Material dialog > MATERIAL DEFAULTS panel: "Room Moldings" default ("Color - White") assigned to the cabinet's Crown Molding component | Missing | C-87 (Material Defaults panel absent); R-34 room molding material default |
| 248 | Set as Default edit button: message "Base Cabinet Defaults have been updated"; the template cabinet no longer uses dynamic defaults | Missing | DS-27 |
| 249 | Group-select two cabinets, Open Object: "No Change" shows where values differ (Countertop, Backsplash, Door/Drawer, Materials) | Missing | Open Object over several cabinets is not supported: shell/spec_dialogs.rs opens a CabinetDialog for a single cabinet only (walls and others have multi: W-83). see CB-631 |
| 249 | Copy/Paste edit button places a copy of the cabinet along the wall | Works | S-40 |
| 249 | Front/Sides/Back panel (the tutorial also calls it FRONT): click the drawer in the preview pane to select a face item; Item Type, Item Height; set the door's Item Type to "Drawer" | Works | CB-10, CB-11; dialogs/cabinet.rs face-item tree with preview selection |
| 249 | "Auto Right Door": single right-hand door under 24", double door over 24" | Missing | Item Type list has Door, Double Door, Drawer... with explicit hinge side (dialogs/cabinet.rs lines 60-124); no automatic door count/handedness item type. see CB-632 |
| 250 | New Cabinet Face Item dialog: Item Type Drawer, Height 12" | Works | dialogs/cabinet.rs "Add New" with type combo |
| 250 | Shelves for any Door or Opening front item | Missing | no shelf settings on face items in dialogs/cabinet.rs (only Lazy Susan shelves on a corner); see CB-633 (shelves per face item, see Lesson 15) |
| 250 | Face Items table shows "Default: Framed Panel Drawer" (main style) vs "Slab" (secondary style on one item) | Partial | per-item style override: CB-497 Missing (Face Item Specification dialog) |
| 250 | Appliance/Door/Drawer Specify button opens Face Item Specification: Drawer Handle Style "Use Default"; Drawer panel Style "Slab Drawers"; library choice or Use Default | Missing | CB-497 |
| 250-251 | Library Browser: Search "drawer", folder "Drawer Fronts"; select "Beaded Framed Drawer"; pointer shows the Replace from Library icon; click the cabinet box/countertop: applies to every drawer/false drawer face using the main style; click directly on a front to replace just that one | In progress (Round 15) | library brief; CB-57; see Round 15 library brief for applying a library door/drawer onto a cabinet |
| 252 | Default Settings > Materials: select "Cabinet Door/Drawer", Shift-click "Cabinet", Select Material (Plan Materials panel, search "Robin's Egg"); Done: the existing drawer base updates | Missing | dynamic Material Defaults do not drive cabinet components (cabinet Materials tab is its own stand-ins); C-87, DS-26 |
| 253 | Copy the drawer base to the Bath; move it with handles | Works | CB-8 |
| 253 | Box Construction panel: Framed; Door/Drawer Overlay: Inset | Works | dialogs/cabinet.rs lines 749-780 (Framed/Frameless; Traditional/Full/Inset overlay) |
| 254 | Front/Sides/Back: select "3 Separation - Horizontal" and "5 Separation - Horizontal"; Item Height 1/2"; drawers 8 3/4" and 9" | Works | CB-11 face solver |
| 254 | Top drawer: Specify > Door Panel Style "Use Default" | Missing | CB-497 |
| 255-256 | Width 48"; Vertical Layout Parent: Split Vertical; select "1.1 Layout - Vertical" and split again; set Item Width 10" for 1.1 and 1.5, 22" for 1.3; separations 1/2" wide | Works | dialogs/cabinet.rs "Split Vertical", item widths via the face solver |
| 256 | Delete two drawers; Item Type "Door - Double" for the remaining middle drawer | Works | ItemType::DoubleDoor |
| 256 | Door Handle Main Style "Knob"; Drawer Handle Main Style "Knob" | Works | HandleStyle |
| 256-257 | General: Toe Kick Height 6"; Accessories panel: Library button next to Foot Style: Select Library Object Millwork > Cabinet Feet > "Narrow Taper" | Partial | ToeKick and FootStyle exist (dialogs/cabinet.rs lines 29-30, 633); a library cabinet-feet pick: built-in FootStyle enum; CB-498 Partial |
| 257 | Camera in the Bath; Material Painter Object Mode; Select Material in Core Catalogs > Materials > Wood: "Birch - Honey"; click paints all "Robin's Egg" on the cabinet | Works | PaintScope::Object |
| 258 | File > Make a Copy "Chic Cottage-Cabinet Style" | Partial | APP-96 |

### Lesson 14 assessment topics (pp. 259-260)

| Topic | Status | Evidence |
|---|---|---|
| Two ways to assign a door/drawer to a cabinet (Door/Drawer panel; click a library item onto the cabinet) | Partial | CB-12; click-to-apply in progress |
| Moldings (counter edge, light rail) on the cabinet Moldings panel | Partial | CB-344 |
| Split Vertical for drawers next to a door | Works | CB-11 |
| Set as Default edit tool | Missing | DS-27 |
| Match Properties edit tool | Partial | S-188 |
| Material Defaults keep cabinet materials uniform | Missing | DS-26 |

## Lesson 15: Cabinet Layout (pp. 260-276)

Workflow: bump and push cabinets, Cabinet Defaults with Diagonal Door, place base cabinets (orientation follows neighbours, corner shape near a corner), resize in 3 in steps and by dialog, centre under a window, copy/reflect a drawer base in a Wall Elevation with Sticky Mode, fillers and module lines, build an island (rotate, copy, countertop Uniform off, backsplash Always Present), breakfast bar with a side panel and library panel style, custom countertop, Architectural Block, wall cabinets and soffits, a full-height cabinet in the garage, adjustable shelves, Rich Text annotation and a custom cabinet label.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 261 | Move handle drag: a cabinet bumps into a wall and stops; dragged against another cabinet, bumps; dragged again, pushes both | Partial | CB-4 Partial; tools/cabinet.rs bump/push; S-73 Partial |
| 262 | Default Settings > Cabinets: Shift-select Base, Wall and Full Height, Edit opens a shared Cabinet Defaults dialog; General: Diagonal Door | Partial | dialogs/cabinet.rs CabinetDefaultsDialog has pages Base, Wall, Full Height... (lines 2082-2090) each separate; multi-select Edit of several defaults at once: Missing; Diagonal corner style exists (line 648) but as a corner spec, not a "Diagonal Door" default check |
| 262 | Build > Cabinet > Base Cabinet: back snaps to the wall | Works | CB-3 |
| 262 | Preview matches an adjacent cabinet's orientation when close, else faces down-screen | Works | tools/cabinet.rs ghost orientation (CB-2/CB-3) |
| 262 | Preview changes to a corner-cabinet shape at a room corner; click places a corner cabinet | Works | tools/cabinet.rs corner detection (CB-6); template `corner_base_leg` |
| 262 | Place one more cabinet beside the diagonal corner cabinet | Works | CB-5 |
| 263 | Select a cabinet by clicking with Select Objects, with the cabinet tool active, or by right-click with any tool | Works | clicking selects a cabinet in Select; with the cabinet tool active and right-click as in S-8 context menu: Works |
| 263 | Handles and a front indicator; hovering a handle shows an arrow and a status bar description | Works | CB-8; select.rs handle hints |
| 263 | Drag a side: width changes in 3" steps with a temporary dimension; label updates B24R -> B27R; Esc restores | Works | tools/cabinet.rs 3" steps; CB-13 auto labels |
| 263-264 | Open Object, Width 30", OK; second cabinet 30"; cabinet next to the diagonal corner 18" | Works | dialogs/cabinet.rs |
| 264 | Center Objects edit button: hover the middle window; dashed horizontal axis; click | Works | S-53 |
| 264 | Wall Elevation: drag a horizontal camera arrow in the Kitchen pointing at the left wall | Works | C-20 |
| 264-265 | Select the drawer base; Copy/Paste edit button then Sticky Mode edit button | Missing | S-137 (Sticky Mode) |
| 265 | Reflect About Object: hover the 2-door cabinet: dashed vertical axis; click creates the copy; repeat for the sink base; Main Edit Mode leaves Sticky Mode | Partial | S-48 reflect Works once per activation; the repeated copies without re-activating (Sticky Mode): Missing |
| 265 | Resize the drawer base to 18" and drag it left until it bumps into the corner cabinet | Works | CB-4 |
| 265-266 | Fillers fill gaps automatically; module-line partition between cabinets wider where a gap is; dragging a cabinet shifts the neighbours and moves the wide separation | Works | CB-5, CB-19 fillers |
| 266 | Filler boards visible in plan | Works | tools/cabinet.rs |
| 267 | Active Layer Display Options: layer "Cabinets, Module Lines" off removes dashed box lines; Undo restores | Missing | no module-lines layer in plan-core layers.rs or the cabinet symbol; cabinet plan lines are part of the cabinet symbol. see CB-634 (module lines on their own layer) |
| 267-268 | Island: cabinet Width 30"; Rotate handle rotates 180 degrees | Works | CB-8 |
| 268 | Copy/Paste + Sticky Mode: pasted drawer bases take the neighbour's orientation | Partial | cabinet paste orientation by neighbour: CB-3 (rotate flush to nearest wall) applies when placing; pasted copies adjacent to an island cabinet: not confirmed in code |
| 268 | Shift-select three island cabinets; the status bar counts them | Works | selection |
| 268 | General: Countertop heading Uniform off, Back overhang 0"; Backsplash Height 6", Always Present; the Active Defaults icons lose their check mark | Partial | Overhang Front, Back, sides are separate fields (dialogs/cabinet.rs 686-688); Uniform switch and Always Present backsplash: not found; DS-26 for the icons |
| 268 | Uncheck Backsplash then OK on the three cabinets | Missing | no multi-cabinet Open Object (Lesson 14) |
| 268 | Center Object on the island group relative to a sink base's top edge: horizontal axis along the edge | Partial | S-53 centres the selection about a target; a block (CB-427) is how Chief keeps the island: group centre on an edge of a cabinet: transform.rs centre_* functions handle openings and rooms; cabinet-edge axes: not confirmed |
| 268 | Move the Kitchen room label | Works | R-44 |
| 269 | Breakfast bar: base cabinet, Height 42", Depth 7"; delete the drawer; door Item Type "Side Panel - Applied" | Partial | dialogs/cabinet.rs ItemType list has no "Side Panel - Applied" item type (sides are Plain/Finished Panel/Opening per face, CB-10) |
| 269 | Accessories panel: Panel Style Library button: "Beaded Recessed Panel Door" | Missing | CB-498, CB-395 |
| 269 | Label panel: Suppress Label | Missing | dialogs/cabinet.rs Label tab has Automatic label and Specify label only (lines 1563-1580); see CB-634 (Suppress Label on cabinets; walls/openings have it) |
| 269 | Resize the cabinet by snapping edges to the drawer bases so its width equals the island | Works | snap to cabinet sides |
| 270 | Generate Custom Countertop edit button on the selected cabinet; polyline edge; temporary dimension: overhang 12" | Works | CB-15 (Custom Countertop generated from cabinets, dialogs/cabinet.rs "Joined from {} cabinets") |
| 270 | Custom countertop edited into custom shapes | Works | custom countertop outline editing (CAD edit handles) |
| 270 | Select the island: click the room label, Shift + drag a marquee: cabinets and countertop become selected, the label deselects; Make Architectural Block | Missing | CB-427 |
| 270 | Wall cabinets: corner preview shape; click either side to place on both walls | Works | CB-1, CB-6 |
| 271 | Move into the corner and resize to 36", 12", 30" | Works | CB-8 |
| 271 | Full Height cabinet 84" high; delete its CM02 crown molding | Works | cabinet Moldings Delete (dialogs/cabinet.rs line ~1493) |
| 271 | Default Settings > Cabinets > Soffit: Width 24", Height 12", Depth 12", Floor to Bottom 84" | Works | dialogs/cabinet.rs CabinetDefaultsDialog Soffit page (line 2085); template `cabinets.soffit` (24 x 12 x 12, elevation 84: same numbers) |
| 271 | Build > Cabinet > Soffit above the full height cabinet; "the room's crown molding wraps around the soffit automatically" | Missing | R-113 (room moldings suppressed behind/around cabinets: Missing); our room moldings run flat along the wall |
| 271 | Material Eyedropper assigns the wall colour to the soffit | Works | C-57 |
| 272 | Full Height in the garage: 36 x 72 x 18; Framed, Inset; delete the upper door and the opening; Door Style "Slab"; handle "Pull Vertical"; Horizontal Position Distance From Edge 2" | Partial | HandleStyle choices; `handle_from_edge` Distance From Edge exists (dialogs/cabinet.rs 1144); "Pull Vertical" is not a named style |
| 272 | Accessories: Foot Style "Caster", Always Present | Partial | FootStyle has None, Block, Bun, Bracket only (plan-cabinets dress.rs): no Caster; Always Present: CB-498 Partial |
| 272 | Materials: "Color - Mouse" on Cabinet, Doors/Drawers/Panels, Shelves | Partial | Materials tab combos for Box, Door/Drawer Fronts...; a "Shelves" component: not confirmed in code |
| 273-274 | Face item Specify next to Shelves: Cabinet Shelf Specification dialog: Manual, Number of Shelves 5 | Missing | no per-item shelf dialog found in dialogs/cabinet.rs; CB-497; see CB-633 (Cabinet Shelf Specification with Automatic/Manual shelf count) |
| 274 | Cabinets listed in schedules; schedule numbering | Works | CB-21 |
| 274 | Rich Text defaults of the Kitchen & Bath Plan View: character size 3"; Print Size button; Printed Scale 1/2" = 1', Desired Print Size 1/8"; Appearance layer "Text, Kitchen & Bath" | Partial | TXT-43 Partial; per-view Rich Text defaults Missing |
| 274 | Rich Text "breakfast bar"; Rotate handle 90 degrees left; move beside the countertop edge | Works | TXT-4, S-25 |
| 275 | Label panel: Specify Label, delete text, Insert Macro > Object Specific > Automatic Label, new line "w/ cd", Alignment "Centered" | Partial | dialogs/cabinet.rs line 1565-1580: Specify label with macro help text (`<L>`); an Insert Macro button/menu and label alignment under Appearance: Missing |
| 275 | Notes in the Kitchen & Bath Plan View use "Kitchen and Bath Note Defaults" on "Text, Kitchen & Bath" | Partial | per-view Saved Defaults Missing (DS-28) |
| 276 | File > Make a Copy "Chic Cottage-Cabinets" | Partial | APP-96 |

### Lesson 15 assessment topics

| Topic | Status | Evidence |
|---|---|---|
| Center Object aligns a cabinet with a window or another cabinet | Works | S-53 |
| Copy/Paste + Reflect About Object create identical cabinets on both sides | Works | S-48 |
| Hide cabinet module lines via the "Cabinets, Module Lines" layer | Missing | no layer |
| Generate Custom Countertop edit tool | Works | CB-15 |
| Make Architectural Block for islands | Missing | CB-427 |
| Soffits fill space between wall/full-height cabinets and the ceiling | Works | CB-17 Partial |

## Lesson 16: Appliances and Fixtures (pp. 277-298)

Workflow: Material Defaults for appliances and fixtures, freestanding refrigerator, a wall cabinet above it, a partition, an undercounter dishwasher, explode and rebuild the island with a range and hood, a built-in microwave in a wall cabinet front, a vanity sink and faucet, tub, toilet, tub/shower trim as a block, reverse the tub symbol, Wall Elevation defaults and Auto Elevation Dimensions with extension line editing and label moves, a cross box, save a named camera, Fixture Schedules by category and room, Find in Plan, labels (layer text style, automatic and specified labels with macros, move label, suppress label).

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 277 | Architectural Blocks of cooktop islands into the library | Missing | CB-427, CB-435 |
| 278 | Freestanding versus built-in appliances; symbols from the Library have no Default Settings | Works | tools/library (symbols), CB-16 |
| 278 | Material Defaults: "Appliances", "Appliance Trim", "Fixtures", "Fixture Trim" | Partial | Project.materials defaults per class; class names: not confirmed in code in dialogs/materials.rs |
| 278-279 | Library: Core Catalogs > Architectural > Appliances > Refrigerators > Side-by-Side; place along the wall | Works | library from Daniel's install at runtime |
| 279 | Select the doorway; click its temporary dimension to the wall; type 18" | Works | S-59 |
| 279 | Wall cabinet placed at the back of the refrigerator: its bottom height automatically adjusts to meet the appliance top (Height shown 23 1/8") | Missing | tools/cabinet.rs places wall cabinets at their default elevation; stacking onto an appliance below: see CB-635 |
| 279 | General: Height 18", Ceiling to Top 3", Width 42", Depth 24" | Partial | Ceiling to Top (From Ceiling reference) Missing (Lesson 14) |
| 279 | Build > Cabinet > Partition between fridge and doorway | Works | CB-18 |
| 279 | Point to Point Move edit button: click the partition's lower-left corner (Endpoint indicator), then the wall cabinet's lower-right corner | Works | S-52 |
| 279 | Drag the front-edge Move handle to the cabinet's front | Works | CB-8 |
| 280 | Partition Specification General: Height "92 5/8 - 3"" typed as an expression; Finished Floor to Bottom 0" | Partial | expression arithmetic in length fields: not confirmed in code (Lesson 14 row) |
| 280 | Partition Moldings panel: "CM02 - Crown Molding" with Retain Aspect Ratio, Height 3", Vertical Offset -3" | Partial | cabinet Moldings tab (Add Crown) applies to all cabinet kinds including Partition; library profile + offsets: Missing (CB-344) |
| 280 | Undercounter appliance: Library > Appliances > Dishwashers; pointer shows the Fixtures icon; click in the 24" gap; snaps to the wall; adjacent countertops extend over it | Works | CB-16 `snap_symbol_to_bay`; countertop extension: CB-14 |
| 280 | Arrow key nudges the dishwasher forward | Works | S-129 |
| 280 | Explode Architectural Block | Missing | CB-434 |
| 281 | Move a cabinet 2" with the Up arrow twice; delete the 30" cabinet | Works | S-129 |
| 281 | Library > Ranges > Electric and Induction Ranges > "Flat Top Range - Integrated": click into the gap | Works | CB-16 |
| 281 | Library > Hoods > Large > "Glass Island Hood": place, move by handle, rotate, Center Objects over the range | Works | S-53 |
| 281 | Resize the breakfast bar support and countertop; Make Architectural Block | Missing | CB-427 |
| 281-282 | Built-in microwave in a cabinet front: increase wall cabinet depth to 18"; Library > Microwave Ovens > Built-In > "Built-In Microwave 2"; click the cabinet; Wall Cabinet Specification shows the symbol at the top of the face items; click it in the preview: DOOR/DRAWER/PANEL panel opens; Move Down twice; Move Up on the door | Partial | CB-16 (appliances inserted into cabinets, cut-outs); the face-item list reordering with Move Up/Down exists (dialogs/cabinet.rs "Move Up"/"Move Down"); an appliance as a face item inside a wall cabinet: CB-492 Partial; not confirmed in code built-in microwave symbols snap into wall cabinet bays |
| 282 | Library search "bathroom sink": pick the "Symbol Type: Sinks (bathroom)" suggestion, folder Bathroom Sinks > Vanity > "Rectangular Sink" | Partial | CB-54 search and filters; filter chip by symbol type: library brief |
| 282 | Click the base cabinet: sink inserted at its centre; message "convert all top drawers to False Drawers?" -> No | Partial | drop-in fixture into a cabinet: CB-16; the false-drawer prompt: CB-492 Partial (false drawer face items) |
| 282 | Base Cabinet Specification specifies drop-in fixture and position | Partial | dialogs/cabinet.rs Add Sink / Add Cooktop buttons (lines 720-723) with position fields |
| 283 | Faucet: Library > Plumbing Fixtures > Faucets > Fixtures > Deck Mounted > "Classic Faucet"; click the sink; faucets seek the countertop; Fixture Specification General: Finished Floor to Bottom typed "+3/4"" appended | Partial | placement on the sink: CB-16 family; "seek the countertop" default and the typed addition (expression): not confirmed in code |
| 283 | Kitchen sink "Double Sink Narrow" and "Claiborne Faucet" under the window | Works | library symbols |
| 283 | Bathtub: Plumbing Fixtures > Bathtubs > Standard Tubs > "Standard Tub 1"; preview snaps wall to wall with a V-shaped front indicator | Works | symbol placement against walls |
| 283 | Fixture Specification General: Width 60"; uncheck Cut Room Moldings | Missing | dialogs/symbol.rs General has size/position/angle/Reflect but no "Cut Room Moldings"; room moldings cutting around fixtures: R-113 Missing; see R-113 for the per-object switch |
| 283-284 | Center Object: hover the right of the Bath: highlight, horizontal axis along the room centre line | Works | S-53 `center_in_room` |
| 284 | Toilet "Standard Toilet" between tub and cabinet | Works | library |
| 284 | Tub spout "Utilitarian 2" along the wall; shower head "Standard Head"; Center Object the head over the spout; Shift-select; Make Architectural Block; centre the block over the tub | Partial | symbols and centring Work; block Missing (CB-427) |
| 285 | Fixture Specification 3D panel: Reflect Geometry flips the tub drain side | Partial | dialogs/symbol.rs "Reflect (mirror left to right)" (flip); the tutorial's Reflect Geometry is a 3D-panel mirror of the model geometry only, ours mirrors the whole object |
| 285 | Default Settings > Camera Tools > Wall Elevation > Selected Defaults panel: saved defaults for elevations; "Kitchen and Bath Elevation Layer Set"; Edit Default next to Dimensions | Partial | default_pages/camera.rs (Back Clipped etc.); selected defaults / layer set per camera type: Missing (DS-28, C-119 family) |
| 285-286 | Dimension Defaults for elevations: Setup Automatic panel (which sides of the elevation get Auto Elevation Dimensions); Locate Auto Elevation (Outer Dimension; objects located); Layer "Dimensions, Kitchen & Bath"; Text Style 1/2" | Partial | DIM-61 Partial, DIM-40, DIM-54; elevation dimension defaults groups |
| 286 | 3D > Create Orthographic View > Wall Elevation: drag a horizontal camera right to left; only the Kitchen is visible; Kitchen and Bath Elevation Layer Set active | Partial | C-20 Works; clip to room (C-138 Partial); layer set per camera view type: not confirmed in code |
| 286 | Tools > Edit Active View: General panel Clip to Room checked | Partial | C-138 |
| 286 | CAD > Automatic Dimensions > Auto Elevation Dimensions: dimensions on all four sides | Partial | DIM-61 (DimMode::AutoElevation writes heights; Round 15 dims2 builder) |
| 286-287 | Preferences > Colors > Handle Fill Color | Missing | PR-17 Partial (Colors panel); the handle fill colour setting: not confirmed in code preferences colours |
| 287 | Select an auto elevation dimension: end points and extensions numbered left to right; diamond Move Extension Line handles under each CL extension; drag to snap onto each locatable object; value previews update | Missing | DIM-63 Partial (Round 15) , DIM-61; elevation dimensions are heights only today |
| 287 | Question "turn off Auto Refresh Elevation Dimensions?" Yes | Missing | no auto-refresh flag |
| 287 | Delete an extension line: drag onto the next extension (replaces it) or drag away from the dimension line | Partial | DIM-41 ExtensionDelete (click mode) |
| 288 | Move Dimension Label handle (four-headed arrow) | Partial | DIM-63 |
| 288 | Edit Active View > Selected Defaults: Current CAD Layer "CAD, Kitchen and Bath" | Missing | Current CAD Layer is project-wide (CAD-1, LAY-49) |
| 289 | CAD > Boxes > Cross Box: drag from the fridge's top back corner to the bottom front corner (Object Snaps on); resize keeps four 90 degree corners | Missing | CadMode has Rectangle, Rotated Box, Cross Box?: not confirmed in code tools/cad.rs (CAD box family); cross box in elevation views: CAD in camera views Missing |
| 289 | Save Active View; Edit Active View: General Name "Kitchen Elevation"; Plan Display panel: callout label E1; Layer panel "Cameras" | Partial | same as the stair section row (Lesson 4) |
| 289 | File > Close View | Works | APP-10 |
| 290 | Default Settings > Schedules > Fixture Schedule: Categories to Include: expand Fixture, uncheck all but Appliances and Plumbing; Columns: Qty, Description, Code, Manufacturer, Comments; Labels panel: Use Label | Missing | the Fixture schedule lists every symbol of the Fixture class; no category tree (Lesson 3); a Qty/Description/Code/Manufacturer/Comments column set: SYMBOL_FIELDS (schedules.rs) |
| 290 | CAD Detail Management > Schedules Detail > Tools > Schedules > Fixture Schedule; Extension Snaps to align | Works | ScheduleVariant(Fixture) |
| 290 | Fixture Schedule Specification General: Main Title "Kitchen Appliance Schedule"; uncheck Include Objects from All Floors; Include Objects from Room "Kitchen"; uncheck Plumbing | Partial | `floor_scope` (schedules.rs); a room scope ("Include Objects from Room") and object categories: Missing; see L-233 (schedule room scope) |
| 290 | Second schedule: "Plumbing Fixture Schedule": uncheck Appliances, tick Indoor | Missing | category tree |
| 290 | Columns/Rows: add "Room Name" at the top; Automatically Sort by Room Name ascending, Then by Description ascending | Partial | schedules.rs `sort` (one sort field, group_by); two-level sort: Missing |
| 291 | Tile Vertically; select the refrigerator line; Find in Plan edit button selects it in plan | Missing | S-118, APP-41 |
| 291 | Fixture Specification 3D panel: Name "Counter-depth Side-by-Side Refrigerator"; Include Size option "Width" produces "[39 5/16W]" in the name; choose None | Missing | dialogs/symbol.rs TABS has `3D` and `Components` disabled (off(...)): no Name/Include Size/Reflect Geometry panel; see CB-636 |
| 291 | Object Information panel: %automatic_description% as Description; Code, Manufacturer, Comment | Partial | DW-118/W-118 Missing for openings/walls; symbols: L-29; the macro name: not in BUILT_IN_MACROS |
| 291 | The sink's automatic label updates and the schedule description follows | Partial | automatic labels exist (CB-13) |
| 291-293 | Active Layer Display Options: Options > Columns > Text Style column; Name Filter "label"; most label layers use "1/2" Text Style" | Partial | LAY-3 Works (name filter); Columns options and Text Style column: LAY-68 Partial |
| 293 | Selecting an object lists only the layers that affect it (a base cabinet: 7 layers; a refrigerator: 2) | Missing | LAY-72 (Object Layer Properties), LAY-68 |
| 293 | Select a layer: details (Text Style Define button) at the bottom; Saved Text Styles Defaults dialog; "1/2" Scale Text Style" Edit; Uppercase checked | Partial | TXT-17 (named text styles Work); Uppercase attribute Missing |
| 293-294 | Only the dishwasher label shows (inserted fixture); turn "Fixtures, Labels" on with the Display column, Layer Display Options, or Object Layer Properties | Partial | Display column Works (LAY-3); Object Layer Properties Missing (LAY-72) |
| 294 | Fixture Label panel: custom layer for a label; Suppress Label (in all views) | Missing | dialogs/symbol.rs Label tab: not confirmed in code; suppress for cabinets Missing (Lesson 15); see CB-637 for symbols if absent |
| 294 | Select Next Object: block, then the interior fixture; status bar reports "Interior Fixture" | Works | Tab cycle |
| 294 | Label panel: Specify Label "Flat Top Range"; Insert Macro button: Object Specific > Width; %width% shows with the inch sign; brackets | Partial | specify label with macros (`<W>`), no Insert Macro menu and no %width% style macros (text_styles.rs macro set: room.*, plan.*, floor.*); see TXT-65 already raised for referenced object macros |
| 295 | Built-in fixture label: wall cabinet Label panel: Specify Label; Insert Macro > Object Specific > Automatic Label; second line "Built-In Microwave"; Alignment Centered | Partial | Lesson 15 label rows |
| 295 | Move Label handle next to the Move handle; drag it beyond the symbol | Works | DW-63 (label handle on openings); symbols: not confirmed in code |
| 296 | Label panel: Plan View Position and Orientation: X Offset, Y Offset (18") | Partial | DW-63 (label placement) ; symbol Label tab fields: not confirmed in code |
| 296 | Suppress Label in all Views on the faucet | Missing | see above |
| 296 | File > Make a Copy "Chic Cottage-Fixtures" | Partial | APP-96 |

### Lesson 16 assessment topics (pp. 297-298)

| Topic | Status | Evidence |
|---|---|---|
| Freestanding versus built-in appliances | Works | CB-16 |
| Removing a cabinet drawer (Front/Sides/Back, Delete) | Works | CB-10 |
| Point to Point Move edit tool | Works | S-52 |
| Reverse Symbol on the Fixture Specification General panel | Partial | Reflect (flip) |
| Wall Elevation camera tool for a single room | Works | C-20 |
| Auto Elevation Dimensions tool | Partial | DIM-61 |
| Remove an extension: drag Move Extension Line handle onto another or away | Partial | DIM-41 |
| Object label text style comes from its layer ("Fixtures, Labels") | Partial | LAY-3, TXT-17 |
| Move or rotate a label with handles or the Label panel | Partial | DW-63 |

## Lesson 17: Light Fixtures (pp. 298-317)

Workflow: set the General Electrical defaults for light symbols (Ceiling, Outdoor, Wall) from the library, switch to the Electrical Plan View, place ceiling lights with the context-sensitive Light tool and from the library, set Electrical Dimension Defaults, position a can light by dimensions with an added extension, replicate with Copy + Enter Coordinates, Reflect About Object and Transform/Replicate, array lights with Multiple Copy and its interval dialog, wall sconces and their heights, table and floor lamps, a room-based electrical schedule moved to a CAD Detail, Electrical Rich Text defaults and a %description% macro in a leader.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 298-299 | Default Sets activate defaults + layer settings for a purpose (e.g. an electrical plan) | Missing | DS-32 |
| 299 | Tab while moving = Enter Coordinates | Partial | S-132, S-146 (typed length/angle; no coordinate dialog) |
| 299 | Default Settings > Electrical > General Electrical > Edit: scrollable list of electrical objects ("Light - Ceiling" default 4" Recessed can; "Light - Outdoor" bollard; "Light - Wall") | Partial | E-19 Partial: default_pages/electrical.rs lists heights per device kind, not a library symbol per use; our kinds: Light, Recessed Light, Pendant Light, Wall Light, Rope Light |
| 300 | "Light - Wall" Library button: Select Library Object, Type Electrical, folder filter, search "lighting", Lighting Suites > Wall Mounted > Semi-Flush Mount > "Narciss Sconce" | Missing | E-19, CB-395: devices are generated glyphs/meshes (plan-electrical DeviceKind), not library symbols |
| 300-301 | Saved Plan View Control: "Electrical Plan View" with "Electrical Layer Set": kitchen/bath text, door/window/cabinet labels, furnishings and section symbol hidden | Partial | layer sets per view Work; the starter Electrical Plan View/Layer Set is not in our template |
| 301 | Selected Defaults panel names begin with "Electrical" | Missing | DS-28, DS-32 |
| 301 | The Light tool places by click position: wall -> sconce, room middle -> ceiling fixture, outside a structure -> path light | Partial | ElecVariant has separate Light (room-centre snap, s08 `a_light_snaps_to_the_room_center...`), Recessed, Pendant and Wall lights; the single context-sensitive Light tool (outdoor path light): see E-30 |
| 301 | Build > Electrical > Light, click between the island and the dishwasher: a recessed can light per the defaults | Partial | tools/electrical.rs Light; the 4" recessed default comes from the defaults: our Light default is a ceiling light kind |
| 301 | Library Browser: Lighting > Chandeliers > Pendant Chandeliers "Bowl Chandelier"; Ceiling Mounted > Flush Mount "Half Dome"; Recessed "6" Recessed Light - Adjustable Eyeball"; Lighting Suites "Caged Lantern Pendant" on the deck | Works | library symbols placed via Place Library Object; an electrical-category symbol also behaves as an electrical object (appears in the Electrical Schedule): not confirmed in code (E-17) |
| 302 | Default Settings > Dimension > Dimensions: "Electrical Dimension Defaults"; General panel Reach 24" | Partial | dimension sets (template `dimension_sets` 13, active_dimension_set); Reach: DIM-51 Partial (manual Reach 24 in) |
| 302 | Locate Manual panel, Locate Objects: Cabinets Sides, Fixtures/Appliances Sides/Corners, Electrical (all), Other: Architectural Blocks | Partial | template `locate_cabinets: Sides`, `locate_fixtures: Sides`; electrical and block locates: DIM-54 Partial; blocks Missing |
| 302-303 | Manual Dimension from the refrigerator front to the island bottom edge drawn beyond the 24" Reach; click the dimension with the island selected and type 42" | Works | DIM-11, DIM-32 |
| 303-304 | Select the dimension; diamond Add Extension Line handle drag onto the can light; the dimension now locates the light | Partial | DIM-41 ExtensionAdd (click mode), DIM-63 (handle) |
| 304 | Select the light (Select Next Object/Tab when the dimension is picked); click a dimension label; type 1' 9": the light moves | Works | DIM-32 on object-anchored ends of a dimension (device anchors: dim_assoc.rs covers walls, openings, cabinets, fixtures; electrical devices: not confirmed in code) |
| 304 | Manual Dimension from the dishwasher to the can light; type 21" | Works | DIM-32 |
| 304 | Delete dimension lines | Works | S-88 |
| 305 | Copy/Paste edit button; drag the Move handle upward and press Tab: Enter Coordinates dialog, End Point Y Position 40"; OK creates a copy 40" above | Partial | Copy/Paste works; the Enter Coordinates dialog: S-132/S-146 Partial |
| 305-306 | Shift-select the two lights; Copy/Paste > Reflect About Object over the island block (horizontal dashed axis) | Partial | S-48 reflect about object Works; the island block Missing (CB-427) |
| 306 | Transform/Replicate: Copy 1, Move X Delta 6' 6"; deselect the two middle lights; Copy 1, X Delta -3' 3" | Works | S-103, S-104 |
| 307 | Basement can light in the lower-left corner; End to End dimensions to the two nearest exterior walls; type 5' for each | Works | DIM-13, DIM-32 |
| 308 | Multiple Copy edit button; Multiple Copy Interval edit button: dialog General Objects Primary Offset 50", Secondary Offset 52" | In progress (Round 15) | briefs/r15/cad2.md; dialogs/multiple_copy.rs; S-138 Missing, S-139 Partial (separate intervals for general/roof/floor trusses) |
| 308-309 | Pointer shows the Multiple Copy icon; right-click and drag upward for a row at the primary interval; release, then drag right (no button) and click for the second direction at the secondary interval: an array | In progress (Round 15) | S-138 (array with the Alternate behaviour); cad2 builder |
| 310 | Select two lights, Multiple Copy, drag right: three more pairs | In progress (Round 15) | same |
| 310 | Build > Electrical > Light on the wall above the sink: an interior sconce; Open Object title "Electrical Service Specification" showing "Small Cone Sconce" default interior wall light | Partial | ElecVariant::WallLight; dialog title and default sconce symbol name: library-less |
| 311 | Click on the deck: "Wide Brim Sconce", the default exterior wall light | Missing | exterior vs interior wall light default: see E-31 (outdoor wall light default) |
| 311 | Library: Lighting > Wall Mounted > Flush Mount > "Prism Sconce" above each side of the vanity | Works | library |
| 311-312 | Wall Elevation: select the sconce; Electrical Service Specification General: Retain Aspect Ratio, Width 12", Height to Center 86" | Partial | dialogs/electrical.rs General has Height (mount height) but no Width/Depth/Retain Aspect Ratio resizing of a device; see E-33 (resizing an electrical object by width/height) |
| 312 | Center Object over the left window: outline around the window and vertical axis; click; copy to the middle window; repeat | Works | S-53 |
| 312 | File > Close View | Works | APP-10 |
| 313 | Active Layer Display Options: Name Filter "furniture", turn on "Furniture, Interior" | Partial | LAY-3 |
| 313 | Library: Lamps > Table Lamps "Table Lamp" on each night stand (lamps seek the table top); Floor Lamps "Basic Floor Lamp" | Partial | symbol placement on furniture (Z snap onto host): see Lesson 13 |
| 313 | Select the Kitchen room; Create Schedule from Room edit button; Create Room Schedule dialog: Electrical; click to place | Missing | R-105, L-60 |
| 313-314 | Electrical Schedule Specification General: Main Title "Kitchen Lighting Schedule"; Include Objects from Rooms list shows "Kitchen 13'-0" x 15' x 7""; Included Categories: expand Electrical, uncheck Outlets, Switches, Other | Missing | no room scope or category tree (Lessons 3 and 16) |
| 314 | Edit > Cut the schedule; CAD Detail Management > Schedules Detail > Open; Edit > Paste > Paste | Works | schedules are cut/paste-able objects (S-81): not confirmed in code |
| 314-315 | Electrical Plan View annotations go on "Text, Electrical"; Rich Text Defaults titled "Electrical Rich Text Defaults": Uppercase, character size 6", Appearance layer "Text, Electrical" | Missing | per-view saved defaults (DS-28/DS-32) and Uppercase |
| 315 | Leader Line from the centre of a ceiling light; drag 1-2 plan feet; second segment optional; click once more for none | Works | TXT-5 |
| 315 | Rich Text content `%description%` shows the description of the object the leader points at ("CRAFTSMAN LANTERN") | Missing | text macros do not reference an attached object (see TXT-65 raised in Lesson 5); electrical devices carry a label, not a description (E-12 Partial) |
| 316 | File > Make a Copy "Chic Cottage-Lighting" | Partial | APP-96 |

### Lesson 17 assessment topics (pp. 316-317)

| Topic | Status | Evidence |
|---|---|---|
| Light tool behaves by click location | Partial | ElecVariant split |
| Two ways to get a dimension to locate a light (draw from the light; add an extension line) | Partial | DIM-41 |
| Transform/Replicate and Multiple Copy for evenly spaced lights | Works / In progress | S-103; S-138 |
| Multiple Copy for arrays | In progress (Round 15) | cad2 |
| Create Schedule from Room edit tool | Missing | R-105 |

## Lesson 18: Electrical Objects (pp. 317-330)

Workflow: place 110V, weatherproof and floor outlets with the Outlet tools (symbol and height depend on the click), recess a wall outlet, place a GFCI above a counter, rotate a wall symbol in an elevation, lay out counter outlets with Enter Coordinates and Transform/Replicate, library outlets and switches, ganged blocks, Electrical Connection splines with automatic 3-way switches, data and security symbols, an electrical legend.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 318 | Electrical Defaults pick the symbols the tools place | Partial | E-19 (device kinds, not library symbols) |
| 318 | Wall-mounted items are best edited in camera views | Partial | 3D editing of devices: view3d picking exists (s24); handles for devices in elevation: not confirmed in code |
| 319 | Build > Electrical > 110V Outlet; click along the wall: standard outlet; the type depends on location | Works | E-3 (wall snap, face into room) |
| 319 | Outlet on the deck: symbol with "WP" (weatherproof) | Partial | separate tools: 110V, GFCI, WP, Floor, Dedicated (ElecVariant); the Chief single Outlet tool that picks WP outside, floor in a room's middle, 110V on a wall by click location is not how ours works (Differs); WP exists (E-1) |
| 319 | The deck outlet sits off the wall because it attaches to the stone layer of the pony wall | Differs | our devices mount on the wall face (E-3), not the outermost layer of a pony wall: not confirmed in code, minor |
| 319 | Floor outlet preview has a square around it when the pointer is in the middle of a room | Partial | ElecVariant::OutletFloor is its own tool; no automatic square-in-room preview from the 110V tool |
| 319 | Select Next Object until the outlet is selected; Open Object: Electrical Service Specification General: Height to Center 11 1/2" | Works | dialogs/electrical.rs "Height" (mount height); default 12 in (E-11) vs tutorial 11 1/2": defaults table |
| 319-320 | Options panel: Distance from Wall -4"; check Cuts Floor/Ceiling/Wall; Cut Depth and Insert Depth 4": the outlet draws flush | Missing | dialogs/electrical.rs has General, Switches, Materials, Label, Layer; no Options tab; see E-32 (recess a device into the wall: distance from wall, Cuts Floor/Ceiling/Wall, Cut Depth, Insert Depth) |
| 320 | 3D > Rendering Techniques > Glass House | Works | C-50 |
| 321 | Build > Electrical > GFCI Outlet; click along the wall under a window: Height to Center 43" because a countertop is present; change to 39" | Partial | counter outlets: E-8 auto at 44 in over base cabinets (counter_runs); a manually clicked GFCI keeps the default height, not a countertop-aware 43 in |
| 321 | Wall Elevation: select the outlet; Rotate handle 90 degrees to orient horizontally | Partial | rotation of a device in a camera view: not confirmed in code |
| 322 | Question "regenerate the outlet's 2D plan symbol?" -> No | Out-of-scope | no 2D symbol regeneration concept in our devices |
| 322 | Center Object on the sink base; Move handle drag then Tab: Enter Coordinates (Relative to Start; End Point X 72", Y 0") | Partial | S-132, S-146 |
| 322-323 | Transform/Replicate: Copy 3, Move Y Delta -48", Relative to Itself | Works | S-103; "Relative to Itself" option: not confirmed in code |
| 323 | Copy/Paste one more GFCI to the horizontal wall | Works | S-40 |
| 323 | Library: Mechanical, Electrical, Plumbing > Electrical > Outlets > Floor Mounted > 220V "220V Receptacle" on the island; Surface Mounted > Appliances "Garbage Disposal" outlet | Works | library symbols (Daniel's install) |
| 324 | Build > Electrical > Switch: click the wall right of the sliding door; second switch by the dining doorway | Works | E-4 |
| 324 | Library Switches: "Air Switch - Countertop" on the countertop near the sink | Works | library symbols on a countertop: not confirmed in code Z snap |
| 325 | Place two more switches beside the first; group-select; Make Ganged Electrical Block; same at the dining doorway | Missing | E-25, CB-428 |
| 326 | Build > Electrical > Electrical Connection: click a switch, drag to the exterior wall light and release: an Electrical Connection spline; a second from the wall light to the deck ceiling light | Works | E-5 (connection tool; arcs) ; spline versus arc: our connection is a bendable arc, E-21 Missing for Connection Defaults |
| 326-327 | Three-way connections: switch -> can light -> next light ... -> a final switch at the other end | Works | E-6 normalize_switch_kinds (S3/S4) |
| 328 | When the circuit has switches at each end they become 3-way automatically | Works | E-6 |
| 328 | Connection splines are editable like other spline objects | Partial | connections are arcs with a bulge fraction; spline handles: E-21 area |
| 328 | Library Core Catalogs > MEP > Electrical: Detectors and Alarms, Jacks, Special Symbols | Works | library; also built-in kinds (E-17) |
| 328 | CAD Detail "Legends and Notes": Tools > Schedules > Electrical Schedule; Main Title "Electrical Legend"; Scale Images and Use Plan View Scale checked | Partial | ScheduleKind::Electrical (E-13); Scale Images/Use Plan View Scale (a symbol picture column): see L-234 raised in Lesson 13 |
| 329 | File > Make a Copy "Chic Cottage-Electrical"; Project Browser: New Folder "Kitchen and Bath Tutorials"; Shift-select five plans; drag | Missing | APP-95, APP-117 |

### Lesson 18 assessment topics (p. 330)

| Topic | Status | Evidence |
|---|---|---|
| Outlet tool: interior wall -> 110V; exterior room or building exterior -> weatherproof; room middle -> floor outlet | Partial | E-3, E-1; automatic weatherproof/floor choice by click location: not confirmed in code |
| Recess a wall outlet with Cuts Floor/Ceiling/Wall, Cut Depth, Insert Depth (Options panel) | Missing | no Options panel |
| Multi-gang switches/outlets with Make Ganged Electrical Block | Missing | E-25 |
| Connect Electrical tool for switches to lights/outlets | Works | E-5 |
| Where an outlet mounts (floor, wall, ceiling) is on the Options panel | Missing | the kind decides (ElecVariant::OutletFloor); see E-32 covers the Options panel |
| Do not regenerate a wall outlet's 2D block | Out-of-scope | no 2D-block regeneration |

## Lesson 19: Floor Framing (pp. 331-348)

Workflow: review Framing Type and Framing Member defaults, snap settings, switch to the "Framing, Floor Plan View", Automatic Framing Defaults (floor structure depth, spacing, joist width, rim joist), place a Framing Reference Marker, copy a layer set, Build Framing with auto-rebuild of floors and ceilings on floor 0, steer joists with a Joist Direction line and with a Bearing Wall, build once for one floor, Manual Framing Defaults for posts and beams, post with footing, beams at joist level and below joists with Raise/Lower, move joists with bumping/pushing and trim them, annotate with bordered Rich Text.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 331 | Framing Reference Marker placed before generating framing | Works | CB-38; FramingVariant::ReferenceMarker |
| 331-332 | Grid Snaps off and Object Snaps on while framing | Works | snap settings (snap_settings.rs) |
| 332 | Default Settings > Framing > Framing Types: Framing Type Management with I-Joist (Wood, I-Joist/Beam shape, not nominal sizes), Lumber (Wood, rectangular, nominal sizes), LVL (Wood, rectangular, not nominal); Edit and Cancel | Missing | CB-213 Partial (FramingMaterial Lumber, Steel, Glulam, LVL, PSL; shapes are boxes), CB-210 Missing; no Framing Type Management dialog |
| 333 | I-Joist replaced by Lumber for joists; LVL is the default for rim joists | Partial | plan-framing defaults: material per group (rim joist rim_plies); the "Framing Member Defaults" abstraction is Missing |
| 333 | Edit > Snap Settings > Grid Snaps toggle, Object Snaps toggle | Works | snap_settings.rs |
| 333 | Saved Plan View Control: "Framing, Floor Plan View" with "Framing, Floor Layer Set": cabinets, fixtures, electrical and room labels hidden; openings show header lines only with a dashed style, no casing or sills | Partial | framing view layers exist (framing_view.rs, framing layers per group, CB-268 plan symbols); the starter view, and openings drawn as header lines in a framing layer set: Missing (DW-? opening plan display per layer set) |
| 333 | Selected Defaults panel names all begin with "Framing" and end with "Floor" | Missing | DS-28, DS-32 |
| 334 | Floor framing for a floor is generated on the floor below | Works | plan-framing floor.rs; framing_view.rs |
| 334 | Default Settings > Framing > Automatic Framing > Foundation panel, "Floor Structure for Floor 1": depth 12", Spacing 16", joist Width 1 1/2" | Partial | dialogs/framing.rs Framing Defaults page (Walls, Headers, Floor, Roof tabs) has joist size and spacing (CB-220); floor structure depth is a Floor Defaults value; one panel per floor ("Foundation", "1ST", "2ND"): CB-220 Partial |
| 334 | Rim Joist Construction: Define opens Framing Member Defaults Management; Rim Joist member General panel: generate as "LVL" | Missing | CB-210; rim joist material is a field of plan-framing defaults |
| 334 | Place a Framing Reference Marker at the wall intersection (Endpoint indicator at the outside corner of the framing) | Works | CB-38; snap to wall layer corners (snap.rs) |
| 335 | Active Layer Display Options: Copy Set button; New Layer Set dialog "Framing Set - Working"; turn on "Framing, Ceiling Joists" | Works | LAY-8 (Save layer set) ; dialogs/layer_sets.rs |
| 335-336 | Go to Floor 0; Build > Framing > Build Framing: choose categories; Automatically Rebuild Framing: Floors and Ceilings checked | Partial | CB-36 Partial (Build Framing dialog with per-group Build and auto-rebuild checks; "Automatically Rebuild Framing" per group Works via `auto_rebuild` in framing_view.rs) |
| 336 | Lowered ceiling framing (hat channels) is generated with the floor framing on the floor 0 | Missing | ceiling framing exists (frame_ceiling_holes) but depends on a Ceiling Finish Definition with a framing layer (Lesson 10: R-122/CB-210) |
| 336 | Joist Direction line: click in the basement room and drag horizontally; both floor and ceiling joists rebuild to that direction; rotate it 90 degrees with its Rotate handle; delete it: joists revert; Undo | Works | FramingVariant::JoistDirection (tools/framing.rs); auto rebuild (CB-37); CB-266 Missing (the label text); rotate handle: not confirmed in code |
| 337 | Turn Automatically Build Framing off; deleting the line changes nothing | Works | framing_view.rs `auto_rebuild` off respected |
| 337 | Bearing wall: select the wall left of the stair; General panel: Foundation Wall checked (footing shown in the preview) | Works | W-52 Foundation flag, dialogs/wall.rs Foundation tab |
| 337 | Structure panel: Bearing Wall checkbox; floor joists change direction to bear on it | Missing | dialogs/wall.rs line ~1661 `dis_check(ui, "Bearing Wall", false)` is a disabled stub; plan-framing floor.rs uses BearingMode::{AllWalls, ExteriorAndBearingLines} with Bearing Lines drawn as objects, not a wall flag. see CB-261 (Bearing Wall flag on walls feeding floor/ceiling joist direction and spans) |
| 337-338 | Build Framing: Build Framing Once > Floors > Floor 1 from the drop-down to rebuild one floor | Missing | dialogs/framing.rs offers Build per group (Floor, Ceiling, Roof, Wall, Posts, Trusses) but no per-floor drop-down for Build Framing Once. see CB-638 |
| 338 | Default Settings > Framing > Manual Framing > Beams panel: Floor/Ceiling Beam Construction "Floor/Ceiling Beam" 12" x 3 1/2"; Define shows Framing Member Defaults (Lumber); Options: With Joists | Missing | CB-247, CB-210 |
| 338 | Posts panel: Post Construction "Post" 3 1/2" x 3 1/2"; Post Footings (Post with Footing tool) same; Footing Shape Square | Partial | post size lives in the Build Framing Posts tab; Footing Shape (Square) has no field: footing is a square size and thickness (dialogs/framing.rs lines ~105-115) |
| 339 | Active Layer Display Options: pick "Working Layer Set" from the Layer Sets drop-down: framing off, wall layers on | Works | LAY-8 |
| 339 | Build > Framing > Post with Footing: click at the end of an interior wall; message asks to turn on the "Framing, Posts" layer: Yes | Partial | FramingVariant::PostWithFooting Works; the layer-on prompt: Missing (minor) |
| 340 | Build > Framing > Floor/Ceiling Beam: click at the Midpoint indicator of the post bottom edge, drag upward; extend to the furring layer's outside surface; Post with Footing at the far end; automatic labels use nominal sizes | Works | FramingVariant::FloorCeilingBeam, PostWithFooting; CB-41 labels |
| 340 | Framing Specification (Floor Beam) General: Top Height -3/4" (under the subfloor, even with the joist tops) | Partial | CB-283 Partial (Bottom elevation field only) |
| 341 | Beam under the joists: Raise/Lower checkbox with -11 7/8", Apply button: Top and Bottom Height adjust | Missing | CB-283, CB-247 (placement With/Under Joists); see CB-283 (Raise/Lower field with Apply on the framing spec) |
| 341-342 | Build Framing once for Floor 1 (see above); joists butt a flush beam, lap over a beam under them | Missing | joist-to-beam bearing relationship (butt versus lap): CB-39 Works for rims/stairwell headers; beam interaction: not confirmed in code plan-framing floor.rs |
| 342 | Turn off the "Soffits" and "Framing, Ceiling Joists" layers to see it | Works | LAY-3 |
| 342-343 | Back Clipped Cross Section in the basement to compare | Works | C-19 |
| 343 | The lowered ceiling framing need not extend over the exterior concrete walls | Partial | CB-39 |
| 343 | Layers: turn off "Framing, Floor Joists" and "Footings", on "Walls, Layers", off "Walls, Main Layer Only", lock "Walls, Foundation" | Partial | LAY-3 lock/display Work; the layer names "Walls, Layers" / "Walls, Main Layer Only": LAY-76 Partial |
| 343-344 | Select a ceiling joist (grey Selection Fill colour in Preferences > Colors); Move handle drag upward: other joists display the selection fill and are bumped | Partial | framing members are selectable and movable (RF-53); bumping of framing members by a moving member: S-73 Partial (cabinets only) |
| 344 | Edit > Snap Settings > Bumping/Pushing toggle: with it off other joists are not pushed | Partial | template `editing.bumping`; applies to cabinets only; see S-196 (bumping/pushing of framing members) |
| 345 | Trim Object(s) edit button then Sticky Mode: drag a temporary cutting line through joist ends; they trim to the selected joist's edge; repeat | Missing | S-137 Sticky Mode; Trim Object(s) edit tool for framing members: see CB-639 (CAD Trim exists for lines, CAD-54) |
| 345-346 | Framing, Floor Plan View annotations go on "Text, Framing - Floor"; Rich Text Defaults "Framing Rich Text Defaults, Floor": Uppercase and Align Center buttons; Appearance panel: Border checked (colour, line style, weight), Margins 6" | Partial | TXT-1 Works (text box border/fill/alignment: text builder round 14, TXT-3, TXT-13); per-view defaults and Uppercase Missing |
| 346-347 | Rich Text "all floor joists are u.n.o.:" Enter, "2x12s SPF #2 or better @16" O.C."; underline the first line; clear Uppercase on selected letters | Partial | TXT-4 (runs keep bold, italic, underline); Uppercase attribute Missing |
| 347 | Copy/Paste the border text near the deck joists; replace words | Works | S-40 |
| 347 | File > Make a Copy "Chic Cottage-Floor Framing" | Partial | APP-96 |

### Lesson 19 assessment topics (pp. 348-349)

| Topic | Status | Evidence |
|---|---|---|
| Framing Reference Marker sets where the framing layout is measured from | Works | CB-38 |
| Benefits of the Framing, Floor Plan View (layer set + saved defaults beginning "Framing") | Partial | LAY-49 |
| Soffits against the ceiling stop lowered ceiling framing at that spot | Missing | see CB-640 |
| Joist direction: shortest span by default; Joist Direction line; bearing walls and beams change the shortest span | Partial | JoistDirection::Auto (floor.rs line 110); bearing wall flag Missing |
| Bearing wall on floor 0 changes floor joist direction but not the lowered ceiling framing | Missing | as above |
| Two beams: at joist height versus lower | Partial | CB-247 |
| Trim Objects edit tool for joist lengths | Missing | see CB-639 |

## Lesson 20: Wall Framing (pp. 349-363)

Workflow: Automatic Framing Defaults for walls (stud construction, plates), Framing Member Defaults, generate wall framing once, edit corner studs (move, replicate, width, point to point), stud spacing by wall type layer, regenerate everything versus selected walls versus Retain Wall Framing, name and open a Wall Detail, draw blocking in it, Wall Detail dimension defaults and a new Default Set, End to End and Running dimensions with secondary labels and extension lines.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 350 | Wall Details open from the Project Browser | Missing | the browser (shell/docks.rs) lists floors, plan views, cameras and layout only; wall details are a data function (`framing_view::wall_detail_of`) and tests. see CB-645 (Open Wall Detail edit button and the Wall Details folder per floor) |
| 350 | Hotkey `'` opens Layer Display Options | Differs | Daniel's hotkeys |
| 350 | Some wall framing is defined in the Wall Type Definition; the rest in Automatic Framing Defaults, reachable from Build Framing | Partial | W-137/W-138 (layer roles) Missing; dialogs/framing.rs Framing Defaults (Walls, Headers, Floor, Roof) reached from Build Framing's last tab |
| 350 | Working Plan View + layer set "Framing Set - Working" to see framing | Works | LAY-8 |
| 351 | Build Framing > Automatic Framing Defaults button | Partial | the Build Framing dialog has a "Framing Defaults" tab instead of a button |
| 351 | Wall panel: Stud Construction "Wall Framing - Lumber"; Top and Bottom Plate Construction "Match Stud Default"; options for plates, intersections, and more | Partial | dialogs/framing.rs Walls tab (stud size, spacing, plates, corners); "Match Stud Default" via Framing Member Defaults: CB-210 Missing |
| 351 | Define > Framing Member Defaults Management > Edit "Wall Framing - Lumber": General panel (Framing Type Lumber); Fill Style panel Solid white | Missing | CB-210, CB-213; framing members have Line Style and Layer tabs but no Fill Style tab (dialogs/framing.rs `TABS_PLAIN`) |
| 351 | Build Framing Once > Walls: only studs and headers display in plan regardless of layers | Works | CB-38 ; plan symbols CB-268 Partial |
| 351 | Floor 1: door and window headers get automatic labels (size and number) | Works | CB-41 (nominal-size labels on headers) |
| 351 | 3D > Perspective Framing Overview shows plates and sills | Works | C-12 Partial (Perspective Framing Overview) |
| 351 | Layers: turn off "Framing, Ceiling Joists" and "Framing, Rim Joists"; lock "Walls, Railings" | Works | LAY-3 |
| 352 | Select a corner stud; Move handle to move it out of the corner | Works | RF-53 family; members selectable (framing tools) |
| 352 | Transform/Replicate Object: Copy 3, Move Y Delta 1 1/2" to make four horizontal corner studs | Works | S-103 |
| 353 | Framing Specification (Wall Framing) General: Width 3 1/2"; Flat to Inside | Partial | member spec General has Lumber Size, Material, Plies, Label, Rotation (dialogs/framing.rs); no Flat to Inside/Outside orientation; see CB-641 |
| 353 | Point to Point Move edit button: stud corner to corner (Endpoint indicators) | Works | S-52 |
| 353 | Stud spacing comes from the properties of the material assigned to each wall type's framing layer | Missing | our wall framing spacing is a Framing Defaults value (dialogs/framing.rs Walls), not read from the wall type; W-138 |
| 353 | Build > Wall > Define Wall Types; select the `8" Concrete with Furring` type; Fir Framing 2 layer; Edit Layer button: Wall Layer Specification General: Spacing 24" On Center | Missing | W-138; no menu command (Lesson 1) |
| 354 | After a spacing change the framing must be regenerated: three ways (auto wall framing in Build Framing, Build Framing Once > Wall, Build Framing for Selected Object(s)) | Partial | Build Framing Once per group Works; "Build Framing for Selected Object(s)" edit button: Missing; see CB-256 |
| 354 | Plan-wide regeneration replaces edited corner framing; Undo restores | Works | framing_view.rs undo step per build |
| 354 | A locked layer only prevents selection, not edits made by regeneration ("Walls, Foundation" locked still rebuilds) | Works | LAY-3 lock semantics |
| 354 | Select six basement walls, then Build Framing for Selected Object(s) | Missing | as above |
| 354-355 | Railing Specification Structure panel: Retain Wall Framing; Build Framing with Build Wall Framing: customised corner stays | Partial | engine exists (`BuildOptions.retain_walls`, `framing_view::set_walls_retained`, test in s33), but the Wall Specification Structure tab shows "Retain Wall Framing" as a disabled check box (dialogs/wall.rs ~line 1660); see W-134 (wire Retain Wall Framing to the Wall spec and an edit button) |
| 355 | Wall Detail name = the wall's label: Label panel Specify Label shows the default detail name; rename "Kitchen Window Wall" | Missing | no wall detail naming |
| 355 | Open Wall Detail edit button; or Project Browser > Wall Details > subfolder per floor | Missing | see first row |
| 356 | Build Framing Wall panel: Stagger Blocking checked | Missing | no Stagger Blocking option in the Walls tab of Framing Defaults (dialogs/framing.rs lines ~722 `Corners, tees and blocking` has none by that name); see CB-642 |
| 356-357 | Build > Framing > Blocking in a Wall Detail: click at the midpoint of an end stud (Midpoint indicator), drag across to the window side; blocking can only be drawn in a Wall Detail | Partial | FramingVariant::Blocking draws in plan; no Wall Detail view to draw in |
| 357 | Wall Details use the Default Set active at creation and the "Detail Layer Set"; both can be changed | Missing | DS-32, LAY-49 |
| 358 | Tools > Active View > Edit Active View: CAD Detail Specification Selected Defaults: Currently Using "Plan Default Set"; choose "1/2" Scale Default Set"; New Default button for Dimension: "Wall Detail Dimension Defaults" | Missing | DS-28, DS-30, DS-32; our dimension sets are named presets (template `dimension_sets` includes 1/2" Scale) |
| 358 | Primary Format: Units inch symbol; Smallest Fraction denominator 16; Locate Manual: Framing Both Sides; Locate End to End: Framing Both Sides | Partial | DIM-6 Works (formats); framing-specific locate options (Both Sides): DIM-54 Partial |
| 358 | Save New Active Default button: New Default Set dialog "Wall Detail Default Set" | Missing | DS-32 |
| 359 | End to End dimensions: vertical overall, horizontal 12" above the top plate, move the line up 24", a second identical line, one inside each window opening | Partial | DIM-13 Works; in a wall detail view: the detail's own dimensions are generated by `plan_framing::wall_detail_dims`, user-drawn dimensions in the detail view: Missing (CAD in section/detail views: CAD-132 family) |
| 359 | Dimension Line Specification Secondary Format: uncheck Use Default Formatting, Use Second Format, Units foot symbol, Accuracy Smallest Fraction | Missing | DIM-46 |
| 360 | Add Extension Line handle drag to the header's bottom midpoint (Midpoint indicator); both headers; blocking sides; vertical line to the horizontal blocking | Partial | DIM-41, DIM-63 |
| 361 | Running Dimension: drag a vertical line from the bottom of the wall framing to the header bottom; Move handle drag 12" beyond; circle at the start end; one-way arrows; cumulative segments | Works | DIM-16 `DimMode::Running` (tick labels: noted open in the not confirmed in code list) |
| 361 | Move and remove extension lines with the diamond handles: snap onto the other window's sill; drag one past the line to delete it | Partial | DIM-41 ExtensionDelete |
| 362 | File > Make a Copy "Chic Cottage-Wall Framing" | Partial | APP-96 |

### Lesson 20 assessment topics (p. 363)

| Topic | Status | Evidence |
|---|---|---|
| Wall studs and headers are the only wall framing seen in plan | Works | CB-38 |
| Stud spacing belongs in the Wall Type Definition | Missing | W-138 |
| Door/window rough openings are set in their defaults dialogs and customised per object | Works | opening Rough Opening tab (opening_tabs brief) |
| Lock a layer to prevent selection | Works | LAY-3 |
| Protect edited wall framing: Build Framing for Selected Object(s) or Retain Wall Framing | Partial | engine only |
| Wall blocking is drawn only in a Wall Detail view with the Blocking tool | Missing | no wall detail view |
| Two dimension labels via the Secondary Format panel | Missing | DIM-46 |
| Running Dimension measures all segments from one point | Works | DIM-16 |

## Lesson 21: Roof and Ceiling Framing (pp. 364-381)

Workflow: switch to the "Framing, Roof Plan View" and set its floor, Build Roof structure defaults (use framing reference, rafter spacing, roof structure definition, ridge size), generate roof framing, discover that existing planes keep their own specs, change all planes with Edit All Roof Planes and one plane with Roof Plane Specification, rebuild framing for the selected plane, break the ridge over a post, stack load-bearing posts across floors with Paste Hold Position and heights, move a wall to another layer, ceiling framing direction with a Bearing Line and Joist Direction lines, shift ceiling framing by dragging with Transform/Replicate and a customised Reference Display, annotate members with a fill and a nominal-size label macro.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 364 | Paste Hold Position copies an object to another floor at the same position | Works | S-83 |
| 364 | With a drawing tool active, Shift + marquee selects objects of that tool's type only | Works | tools/select.rs (Alt/Shift marquee by tool class: S-6 family); the Joist tool selecting joists is used below: not confirmed in code per framing tool |
| 365 | Roof framing generates inside the roof planes, so the roof must be right first | Works | plan-framing roof.rs |
| 365 | Framing Reference Marker; Grid Snaps off, Object Snaps on | Works | CB-38 |
| 365-366 | Saved Plan View Control: "Framing, Roof Plan View" with "Framing, Roof Layer Set"; Edit View: General panel floor "2nd Floor"; Selected Defaults names begin "Framing" and end "Roof" | Partial | LAY-49; no starter views |
| 366 | Roof defaults can be set in Framing Defaults and Build Roof; roof framing uses each plane's own spec | Works | RF-36 (Roof Plane Specification Structure tab), dialogs/framing.rs Roof tab |
| 366 | Build > Roof > Build Roof > Structure panel: Use Framing Reference; Rafter Spacing 16" OC; Roof Layers: Structure Edit button | Partial | dialogs/roof.rs Build Roof tabs Roof/Options/Materials (RF-2); the Structure panel with rafter spacing and use-framing-reference: Structure tab appears on the plane dialog (RF-36) but not on Build Roof defaults: not confirmed in code |
| 366-367 | Roof Structure Definition: Thickness 11 1/4"; Rafter Construction = "Rafters" Default Framing Member; Define shows Framing Type "Lumber" | Partial | RF-36 Structure tab "Define" (framing, member size, spacing, sheathing); Default Framing Member abstraction Missing (CB-210) |
| 367 | Ridge board Width 3 1/2", Depth 15 1/4" | Partial | ridge settings in roof spec (RF-36): not confirmed in code |
| 367 | Message: the changes do not affect existing roof planes | Missing | no such prompt; our Roof Defaults edit applies to new planes (RF-5) |
| 367 | Build Framing Once > Roof | Works | CB-36 |
| 367 | Open a rafter: Framing Specification (Rafter) General: Depth 9 1/4" (from the plane's old spec) | Works | roof framing built from each plane (plan-framing roof.rs) |
| 368 | Perspective Framing Overview; ridge board depth reported 11 1/4" via temporary dimension and spec "Framing Specification (Rafter Ridge)" | Partial | C-12; temp dims on framing members (S-56 family) |
| 368-369 | Build > Roof > Edit All Roof Planes: values "No Change" where planes differ; Baseline Height locked for all; Structure panel: Rafter Spacing 16", Ridge Width 3 1/2" | Works | RF-39 (AllPlanesDialog "No Change" semantic) ; baseline lock: RF-115 Partial |
| 369 | Build Framing Once > Roofs; check the ridge board width | Works | CB-36 |
| 369 | Roof Plane tool active: only roof planes are selectable | Works | tools/roof.rs Plane mode selection |
| 369 | Roof Plane Specification General: Top Plate Height shown and locked; another height reference can be locked; Baseline Height 122 13/16" | Partial | RF-115 Partial: "Pitch and Baseline Height only; no ridge or fascia heights and no pivot locks" |
| 370 | Structure panel: Rafter Spacing 24"; Structure Edit: framing depth 11 1/4"; the plane's Top Plate Height stays; Baseline Height rises 124 7/8" (outside moves up, inside fixed) | Missing | RF-115 (no pivot lock; changing structure thickness grows which side: not confirmed in code roof_view) |
| 370 | Build Framing for Selected Object(s) edit button on one plane | Missing | see CB-256 raised in Lesson 20 |
| 370 | Rafter Spacing change; confirm rafter size | Works | roof framing |
| 370 | Build > Framing > Post at the Intersection indicator where the ridge crosses the wall centre | Works | FramingVariant::Post |
| 370-371 | Select the ridge board (Select Next Object; status bar "Rafter Ridge"); Add Break; click at the post's bottom edge midpoint: ridge divides in two | Partial | a member can be selected and split at a point by the Break edit tool: framing members are not CAD, W-44/CAD-95 do not cover them; Add Break on framing members: not found; see CB-643 |
| 371 | Back Clipped Cross Section at the post; Active Layer Display Options: "3D Framing Set" shows only framing and foundation walls | Partial | C-19 Works; the starter layer set isn't shipped |
| 371-372 | Window > Tile Vertically; select the post; Edit > Copy; Down One Floor; Paste Hold Position; the copy has the same absolute height so it isn't visible | Missing | APP-41 |
| 372 | Framing Specification (Post) General: Top Height 206 1/4"; Raise/Lower with -97 1/8" and Apply -> Top 109 1/8"; then -3/4" and Apply | Missing | CB-283 (no Raise/Lower or Apply); see CB-283 raised in Lesson 19 |
| 372-373 | Floor 0 paste; Lock Total Height radio; Top Height -3/4" | Missing | CB-283 |
| 373 | Hide "Walls, Foundation" in the section; post bottom end handle extends to the wall's bottom plate (snaps); copy/paste footing and Center Object Snap to centre it | Partial | handles/snaps on members: tools/framing.rs; "Center Object Snap": snap center (template `snap_center`) |
| 374 | Set heights of the other posts by handle snaps to the post below and to the roof ridge rafter | Partial | members snap to other members' ends: not confirmed in code snap.rs framing snap targets |
| 374 | Wall Specification Layer panel: change the interior footing wall to layer "Walls, Normal" so the foundation layer can stay on in the 3D framing set | Works | W-80 Layer tab Partial / dialogs/wall.rs Layer tab (W-24 flags): not confirmed in code the layer drop-down |
| 374-375 | "Framing, Ceiling Plan View"; Floor 2 ceiling joists vertical where a flat full-height ceiling exists | Partial | frame_ceiling_holes; ceiling joists only over flat ceilings: not confirmed in code (R-30/RF-46) |
| 374 | Build > Framing > Bearing Line: drag a horizontal line snapping to the shorter joists' bottom edge | Works | FramingVariant::BearingLine |
| 375 | Joist Direction line horizontal below the Bearing Line and vertical above it | Works | FramingVariant::JoistDirection |
| 375 | Build Framing > Framing Defaults button: 2ND panel: Use Framing Reference; Spacing 16" OC | Partial | CB-220 (Use Framing Reference per floor Partial) |
| 375-376 | Build Framing Once > Ceilings > Floor 2; build again | Missing | per-floor Build Once dropdown: Lesson 19 see CB-638 |
| 376 | Tools > Floor/Reference Display > Reference Display; Change Floor/Reference dialog: Reference Display table row #1: floor "2nd Floor", Define cell opens Layer Display Options with "Reference Display Layer Set" | Partial | LAY-42 Partial (reference_display.rs has floor below/above/fixed floor and layer set), LAY-43 Missing (table rows) |
| 376-377 | Layer Display Options: Name Filter "roof": turn on "Framing, Roof Rafters", off "Roof Planes"; rafters show in the Reference Display aligned with the joists | Partial | reference display layer set (LAY-41); layer names "Framing, Roof Rafters": plan-framing layers per group (not confirmed in code naming) |
| 377 | Build > Framing > Joist; Shift + marquee around the drawing selects all ceiling joists; Open Object title "Framing Specification (Ceiling Joist)" | Works | tools/framing.rs (tool-class marquee), dialog title by kind (dialogs/framing.rs `kind_label`) |
| 377 | Transform/Replicate Object: Move X Delta -3/4": joists sit against rafter sides instead of aligned centres; repeat with Y Delta for the horizontal ones | Works | S-103 |
| 378 | Toggle Reference Display off | Works | LAY-9 |
| 378 | Annotation: most framing members have no automatic labels (posts and beams do); the "Framing, Direction Lines" layer hidden | Partial | posts and beams get labels (CB-41); a framing member has a plain Label text field in General (no Label tab, no Insert Macro menu) |
| 378 | Select three valley rafters (Ctrl-click); Framing Specification (Rafter Ridge) Fill Style panel: Solid grey | Missing | `TABS_PLAIN` = General, Line Style, Layer: no Fill Style tab on framing members |
| 378-379 | Label panel: Specify Label; Insert Macro > Object Specific > Nominal Size inserts `%nominal_size%`; add "VALLEY RAFTER"; Move Label handle | Missing | framing members have only a Label text field in General (dialogs/framing.rs line ~370); no Fill Style tab (TABS_PLAIN = General, Line Style, Layer) and no Insert Macro; see CB-644 |
| 379 | File > Make a Copy "Chic Cottage-Roof Framing"; New Folder "Framing Tutorials"; drag plans | Missing | APP-96, APP-95 |

### Lesson 21 assessment topics (pp. 380-381)

| Topic | Status | Evidence |
|---|---|---|
| Roof framing uses the plane's specification, not the current defaults | Works | plan-framing roof.rs |
| Edit All Roof Planes changes all planes' specification | Works | RF-39 |
| Rebuild Framing for Selected Object(s) rebuilds one plane | Missing | no edit button |
| Paste Hold Position copies to the same location | Works | S-83 |
| Transform/Replicate Object moves selected objects a precise distance | Works | S-103 |

## Lesson 22: Plot Plans (pp. 382-403)

Workflow: a separate plan file for the site, Plot Plan View, CAD Defaults (line length format, quadrant bearing display), Input Point at the origin, Input Line with relative polar entry using quadrant bearings from a surveyor's description, correcting or reversing a line, a curved lot line with Change Line/Arc and Lock Chord radius, length/bearing display, setback lines by Concentric edit behavior with a Jump distance, layers for setbacks, paste into the house plan, Convert Polyline to Terrain Perimeter, North Pointer, rotate the lot to an exact angle, position the structure by dimensions to point markers.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 382 | File > New Plan; Save As "Chic Cottage-Plot Plan" in the project folder | Works | files.rs, APP-19 (a separate .psplan; no project folder concept) |
| 382 | Two Norths: site plan with North straight up and the house plan with its own north | Partial | LAY-37 (north angle lives with the North Pointer/Terrain spec); a per-plan-file north: Works by separate files |
| 383 | Input Line tool: lines with specific starting points, lengths and angles; Concentric edit behaviour with distance increments | Works | CAD-5, CAD-25 |
| 383 | Dialog Number/Angle Style dialog controls length/angle entry and display in dialogs and the status bar | Missing | PR-30 Partial (global dialog), PR-31 Missing |
| 383 | Default Sets for a specific purpose (e.g. plot plan) | Missing | DS-32 |
| 383 | Trace a lot image: import an image and trace over it | Works | tools/images.rs picture/underlay with two-point calibration (docs: underlay); not confirmed in code ToolId::Underlay calibration |
| 384 | Import a DXF/DWG of the plot plan and elevations | Works | L-43 DXF import with units and layers; DWG import: L-78 Partial |
| 384 | "Plot Plan View" with "Plot Plan Layer Set": terrain perimeter, doors/windows and General Note callouts visible; roof framing and labels hidden; Selected Defaults begin "Plot Plan" | Partial | layer sets Work; starter view/set not shipped; DS-28 |
| 384 | Edit > Default Settings > CAD > General CAD > CAD Defaults: Displayed Line Length Format Preview with Define | Missing | CAD-107, CAD-108 |
| 385 | Displayed Line Length dialog: Units "ft" or ' ; Decimal Places 2 | Missing | CAD-108 |
| 385 | Display Line Angle as: Quadrant Bearing | Missing | CAD-109 |
| 386 | CAD > Points > Input Point: New CAD Point dialog, Absolute Location X 0, Y 0; creates the Current Point | Partial | CAD-112 Partial (X, Tab, Y, Enter absolute only), CAD-113 Missing (Current Point concept) |
| 386 | Window > Pan Window; Zoom out | Works | Pan tool |
| 386 | CAD > Lines > Input Line: New CAD Line dialog with Start Point = Current CAD Point; drag the dialog aside | Partial | CadMode::InputLine takes length and angle by typing; the dialog with a start point taken from the Current Point: CAD-134 Partial |
| 387 | Number Style button: Number Style Decimal Feet, Angle Style Quadrant Bearing | Missing | PR-30, PR-31 |
| 387 | Relative to Start Point + Polar; Distance in decimal feet; Angle as "N 61 25 10 E" (primary direction, degrees minutes seconds, secondary direction; Tab adds the symbols) | Missing | CAD-109, CAD-134: no bearing parsing anywhere in plan-app/plan-core; survey metes-and-bounds entry is a gap for site plans. BREAK: cannot enter a surveyor's lot description |
| 388 | Next button: new Start Point = end of the previous line; a new CAD point at the end is the new Current Point; OK on the last | Missing | CAD-112/CAD-113 (no Next, no Current Point) |
| 388 | Closed or unclosed result; some descriptions go the other way around | Works | polylines open or closed |
| 389 | Correct an error: CAD > Lines > Disconnect Edges toggle; click the bad segment (only it shows handles); Delete; toggle off; Place Point at the last good end; continue Input Line | Missing | S-172 |
| 389 | Reverse a line by entering the opposite letters ("S 49 59 11 W" for "N 49 59 11 E") | Missing | CAD-109 |
| 389-390 | Curved lot line: Angle Snaps off (the icon follows the pointer as a reminder); Draw Line between open ends | Partial | angle snap toggle Works (F10 in Chief; ours per Daniel's set); the pointer badge for snaps off: APP-73 Partial |
| 391 | Change Line/Arc edit button turns the line into an arc; Reshape handle chooses the bulge side | Missing | CAD-22 (planned) |
| 391 | Polyline Specification Selected Arc panel: Lock Chord, Radius 450' (or Arc Length) | Missing | CAD polyline arcs have a bulge but no radius/chord lock dialog (CAD-? polyline spec); see CAD-111, CB-543 (Selected Arc panel of the polyline spec) |
| 391 | CAD > Points > Delete Temporary Points | Works | CAD-2 "delete temps" |
| 391 | Edit > Snap Settings > Angle Snaps back on | Works | snap_settings |
| 391-392 | Polyline Specification Line Style panel: Show Length, Show Angle, All Angles | Missing | CAD-118 |
| 392-393 | Preferences > Behaviors: Edit Type Concentric; Jump field 10' | Partial | Edit Behaviors dialog (dialogs/edit_behaviors.rs) with concentric distance and copies; the pointer badge; S-65 Works |
| 393 | Select the polyline; Copy/Paste edit button; drag a corner handle toward the centre: an inner copy appears | Works | CAD-25 (polyline body drag leaves offset copies); copy via handle: S-135 Partial |
| 393 | Inner polyline Line Style panel: dashed style (or Library line styles); uncheck Show Length and Show Angle | Partial | CAD line styles Work (dialogs/cad.rs line style); Show Length/Angle Missing; library line styles: Round 15 library |
| 393-394 | Number Style dialog back to Fractional Inches and Degrees; Edit > Edit Behaviors > Default | Partial | edit behaviors default reset Works; number style Missing |
| 394 | Setback polyline on a new layer: Line Style panel Layer Define > Layer Display Options > New "CAD, Setback Lines" | Works | LAY-3, dialogs/layer_display.rs new layer; polyline Layer tab |
| 394 | Select perimeter + setback; Edit > Copy; File > Open Plan the house plan; File > Open Recent Documents; Floor 1; switch to "Plot Plan View"; Edit > Paste | Works | clipboard across plans (S-81), Open Recent (APP-5); copy-paste between two open plans in one session: not confirmed in code |
| 395 | CAD Defaults (line length and angle formats) are view-specific, so the pasted polylines show feet-inches and degrees | Missing | CAD-107 (no CAD Defaults) |
| 395-396 | Delete the existing Terrain Perimeter; select the pasted perimeter; Convert Polyline edit button: Terrain heading > Terrain Perimeter; Layer Options: Default Layer for Converted Object Type; the option is greyed if a perimeter already exists | Partial | CAD-103 Partial (Convert Polyline covers slab, countertop, room, wall...; terrain perimeter: not confirmed in code tools/cad.rs); CAD-104 Missing (layer options) |
| 396 | Terrain Specification Line Style panel: Layer "Terrain Perimeter" with "Plot Plan Text Style" in the Plot Plan Layer Set; uncheck Show Length and Show Angle | Partial | dialogs/terrain.rs layer row (line ~455); Show Length/Angle Missing; per-layer text style in layer sets: template `layers[...].text_style` |
| 397 | CAD > North Pointer: click and drag a pointer (points up initially); angle labels unchanged when it points straight up | Works | CB-84 |
| 397 | Rotate setback, perimeter and North Pointer together with the Rotate handle; Ctrl overrides angle snaps; line angles update | Works | S-1 rotate handle for a group (S-37); Ctrl override S-121 Partial |
| 398 | Quadrant bearings stay the same when rotating because they are measured from the North Pointer | Missing | CAD-109 |
| 398-399 | Transform/Replicate Object with Rotate, Relative Angle, "1 30 25" -> 1.506944 degrees after Tab | Partial | S-103/S-104 rotate Works; DMS entry "1 30 25" in angle fields: PR-31 Missing (angle style) |
| 399-400 | End to End Dimension from the structure corner to the setback line; select setback + perimeter; click the dimension, type 50' | Works | DIM-13, DIM-32 (dimension edit moves a polyline: DIM-66 Partial for polylines) |
| 400 | Dimension to a non-parallel setback line: End to End drawn horizontally; On Object snap; a Point Marker is created on the line and the dimension locates it | Partial | DIM-47 Missing (point-to-point dimension creates point markers); tools/dimension.rs ties free points |
| 401 | Select the Point Marker; click its dimension; type 30'; Copy/Paste drag to the setback; Shift-select marker, setback and perimeter; Point to Point Move from the new marker to the original | Partial | CAD-2 point markers Works; S-52 Works; marker-located dimensions follow DIM-47 gap |
| 401 | Save the plot plan file, File > Close View; then save a copy of the house plan "Chic Cottage-Lot Lines" | Partial | APP-96 |

### Lesson 22 assessment topics (pp. 402-403)

| Topic | Status | Evidence |
|---|---|---|
| Input Line tool for length and bearing entry; Change Line/Arc for arcs | Partial | CAD-5 Works; bearings Missing; CAD-22 Missing |
| Length values in dialogs in feet: Number Style button | Missing | PR-30 |
| Quadrant Bearings set in CAD Defaults | Missing | CAD-107, CAD-109 |
| Concentric Edit Behavior for setbacks | Works | CAD-25 |
| Convert Polyline edit tool to a Terrain Perimeter | Partial | CAD-103 |
| Transform/Replicate Object to rotate the lot to a specific angle | Partial | S-103 |

## Lesson 23: Terrain Elevation (pp. 403-419)

Workflow: add the lot angle to Additional Allowed Angles, duplicate the Plot Plan View into a Landscaping Plan View with a copied layer set, set the building pad height (Absolute Elevation, Reference Point), draw Elevation Lines and give them elevations, copy lines to shape slopes, control steepness by spacing, rotate and lock angles, make a line parallel to a polyline chord and a curved one with a locked radius, build an Elevation Polyline with the Same Line Type handle, add retaining walls and position elevation data 8 in from them.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 403 | Open the house plan again; the lot polyline pasted earlier is in it | Works | clipboard |
| 404 | Elevation Data import from DXF/DWG, text and GPS files | Works | CB-75 (DXF points, CSV/text, GPX) |
| 404 | Object Snaps on | Works | snap_settings |
| 404-405 | Default Settings > Plan > General Plan Defaults > Angle/Grid panel: Allowed Angles: Additional Angles; type the lot-line angle as a quadrant bearing, as DMS "-78 28 46", or as decimal degrees; Tab converts to the current format | Partial | DS-47 Partial (Snap Properties: allowed-angle text); bearing and DMS entry: PR-31 Missing |
| 405 | Project Browser > Plan Views: right-click "Plot Plan View" > Duplicate; New Saved Plan View dialog "Landscaping Plan View"; the new view opens in a second window | Partial | LAY-50 Partial (dialogs/plan_views.rs `duplicate_view`: copies the view under a free name); right-click Duplicate in the browser: not confirmed in code docks.rs context menu (APP-121 Partial) |
| 405-406 | Active Layer Display Options: Copy Set "Landscaping Layer Set"; turn on "Terrain, Primary Contours"; off "Dimensions, Plot Plan"; Tools > Active View > Save Active View | Partial | Copy Set (LAY-8) Works; layer names: "Terrain, Primary Contours" (terrain contour layer names: not confirmed in code plan-terrain/site_view); Save Active View exists |
| 406 | Terrain > Terrain Specification > General: Absolute Elevation (Automatic off), Retain Surface Elevation At "Reference Point", Surface at Reference Point -28" | Missing | see CB-530 (Lesson 6) |
| 406 | Terrain Elevation Reference Point Move handle: drag to the centre of the garage door | Missing | same |
| 406 | Importing elevation data (video) | Works | CB-75 |
| 407 | Floor 1; Terrain > Elevation Data > Elevation Line: drag across the entire Terrain Perimeter, snapping to the edges or extending beyond | Works | TerrainVariant::ElevationLine (CB-44) |
| 407 | Prompt: turn on the "Terrain, Elevation Data" layer in the current view: Yes | Missing | see (minor, no row) (layer-on prompt; minor) |
| 407 | Elevation Line Specification: Elevation panel, Elevation 0"; Number Style button: Decimal Feet | Partial | dialogs/terrain/object.rs "Elevation" field (line 296 `line_z`); Number Style button/units per dialog: PR-30 Partial |
| 408 | Move handle snaps to the garage front wall; Down arrow moves 1" | Works | snap, S-129 |
| 408 | Perspective Full Overview tiled with plan; Mouse-Orbit | Partial | C-10 Works; tiling Missing (APP-41) |
| 408-409 | Copy/Paste edit button; drag the Move handle toward the bottom; the copy has Elevation 0"; set -8' in the General panel | Works | S-40; field edit |
| 409 | Contour lines appear evenly spaced; the 3D terrain is a continuous slope | Works | CB-43, CB-45 (Build Terrain, contour lines) ; terrain builds on Build Terrain (not live): not confirmed in code auto-rebuild of terrain on edit |
| 409-410 | Additional Elevation Lines and positions change the slope; contours spread out | Works | CB-44 |
| 411 | Steepness = difference of elevations over distance; wave shape between lines at equal elevation | Works | spline surface (plan-terrain) |
| 412 | Elevation lines with different elevations should never touch or cross | Missing | CB-512 (warning of conflicting data at the same place) |
| 413 | Rotate the top two lines as a group with the Rotate handle; the status bar shows the angle | Works | S-37, handle |
| 413 | Resize handle drag snapping at 15 degrees; Ctrl overrides movement restrictions | Partial | S-121 Partial |
| 413 | Elevation Line Specification Selected Line panel: Lock Start Point; Angle -13 degrees | Missing | dialogs/terrain/object.rs has Elevation only; see CB-647 (Selected Line panel with lock and angle for terrain lines, like the CAD line spec) |
| 414 | Make Parallel edit button: pick a polyline; dashed alignment axis; Open Object: Chord Angle | Partial | CAD-54 Make Parallel/Perpendicular works for CAD; for terrain elevation lines: not confirmed in code; Chord Angle field Missing |
| 414 | Change Line/Arc edit button: arc; Reshape handle; Selected Arc panel: Lock Chord, Radius 450' | Missing | CAD-22 Missing; arcs on elevation lines: TerrainVariant::ElevationSpline instead (Differs); Selected Arc panel Missing |
| 414-415 | Elevation Polyline = two or more Elevation Lines snapped together; closed shape = Elevation Region | Works | TerrainVariant::ElevationLine polylines, ElevationRegion (CB-44) |
| 415 | Layer filter "Contours": turn off "Terrain, Primary Contours" | Works | LAY-3 |
| 415 | Draw a horizontal Elevation Line to the middle of the terrain; status bar shows length and angle (180 degrees right to left) | Works | status readouts (W-15 family) |
| 415 | Same Line Type diamond edit handle at the end of the line (with the Elevation Line tool active): drag to start an angled segment at 165 degrees through the perimeter edge | Missing | W-143 (Same Wall Type handles) Missing; the polyline continuation handle for terrain lines: see CAD-121 |
| 415 | The new polyline's default Elevation is interpolated between neighbouring lines; set 0" | Works | CB-44: not confirmed in code interpolated initial elevation |
| 416 | Add Break on the Elevation Line: click to add a corner and handle; drag the segment end | Works | CAD-95 / CB-44 "Break" |
| 416 | Terrain > Terrain Wall and Curb > Straight Retaining Wall: click on the setback polyline, drag along it; second wall at 15 degrees; handle to the setback line | Works | TerrainVariant::StraightWall (CB-46) |
| 417 | Retaining Walls make elevation data on either side not interact; default height from the terrain on both sides | Partial | plan-terrain landscape.rs WallKind: top follows the terrain; independent grades each side: left/right offsets (landscape.rs line ~121); see CB-648 not confirmed in code against Chief's wall-as-terrain-break |
| 417 | Temporary Dimension from the Elevation Line to the wall: type 8" | Works | S-59 |
| 418 | Add Break twice on an Elevation Polyline; Move Segment handle toward the wall; reshape so two edges are parallel to the walls; 8" from the walls via temporary dimensions | Partial | segment handles on terrain polylines: not confirmed in code; temp dims Work |
| 418 | Leader line for the retaining wall; dimension with additional text on the setback lines | Partial | TXT-5 Works; dimension additional text: DIM-65 Partial |
| 418-419 | Save a copy "Chic Cottage-Terrain"; Close All Views | Partial | APP-96, APP-10 |

### Lesson 23 assessment topics (p. 419)

| Topic | Status | Evidence |
|---|---|---|
| Building pad height on the General panel of the Terrain Specification | Partial | CB-51 Works (Building Pad panel: first floor elevation, side slope); the tutorial's Absolute Elevation General panel: Missing |
| A slope needs two Elevation Data objects at different elevations | Works | CB-44 |
| Contour display via the "Terrain, Primary Contours" layer | Works | CB-51 contours |
| Change Line/Arc edit tool | Missing | CAD-22 |
| Add Break edit tool | Works | CAD-95 |
| Retaining wall height defaults from the terrain on both sides | Partial | landscape.rs |

## Lesson 24: Driveways, Sidewalks, and Roads (pp. 420-433)

Workflow: Road Defaults (width, curbs cut driveways), draw a road that follows the lot edge (Change Line/Arc, Point to Point Move), a road stripe, a CAD copy of the Terrain Perimeter to show the road in 3D, road height from the terrain centre line, Driveway Defaults (width, flare, material), a driveway, Sidewalk Defaults, Auto Generate Sidewalks, drawn and curved sidewalks.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 420 | Road objects display in 3D only where they cross the Terrain Perimeter | Differs | our strips drape on the built surface (CB-48); no such limit |
| 420 | Number Style/Angle Style: feet and fractional inches for large objects | Missing | PR-30 |
| 421 | Default Settings > Terrain > Road > Edit: General Width 28'; Curb panel: raised curbs and "cut driveways and sidewalks" | Partial | default_settings_terrain.rs and RoadStrip (width, `curb`, `curb_height`, `crown`); no option that curbs are cut for driveways/sidewalks; see CB-628 (curb cuts at driveways/sidewalks) |
| 422 | Terrain > Road > Straight Road: drag across the bottom of the Terrain Perimeter; Midpoint indicator at the lower-left corner; preview angles downward | Works | TerrainVariant::Road (CB-48) |
| 422 | Change Line/Arc edit button on the road; drag the Reshape handle toward the perimeter's curved edge until the On Object indicator shows | Missing | CAD-22 Missing; roads have a spline option (TerrainVariant spline roads: CB-48 "straight or spline") instead of an arc |
| 423 | Point to Point Move edit button: road top-edge midpoint to the curved edge midpoint | Works | S-52 |
| 423 | Drag the road's right end handle to the perimeter corner (Endpoint indicator) | Works | handles |
| 424 | Terrain > Road > Road Stripe: drag from midpoint to midpoint; Change Line/Arc and Reshape handle toward the road centre; temporary dimension 6' from the road's bottom edge | Partial | RoadKind::Marking Works; arc via Change Line/Arc Missing; temp dim Works |
| 425 | To show a road in 3D, it must be inside the Terrain Perimeter: Copy the Terrain Perimeter, Paste Hold Position: the copy is a CAD polyline (only one Terrain Perimeter can exist) on "CAD, Plot Plan"; resize the real perimeter's curved edge to the road's bottom edge; Select Next Object to tell "Terrain" from "Special Polyline" | Partial | one Terrain Perimeter per plan: Works; Paste Hold Position of a terrain perimeter yields a CAD polyline: not confirmed in code; the status-bar names "Terrain" / "Special Polyline": status naming S-98 |
| 425 | Terrain Specification Line Style: uncheck Show Length and Show Angle | Missing | CAD-118 |
| 426 | Perspective Full Overview; Window > Swap Views back to plan | Missing | APP-40 |
| 426 | Roads are flat across their width and take height from the terrain along the centre line; copy the curved Elevation Line to the road centre: a gentler slope | Partial | RoadStrip `crown` and drape; road height from the terrain centre line: Works (drape) |
| 427 | Road Specification General: Height 1" | Missing | no road height field (RoadStrip has crown and curb_height); see CB-629 |
| 427 | Default Settings > Terrain > Driveway: General Width 13'; Flare: End checked, Width 3' | Missing | no flare field in RoadStrip; see CB-630 |
| 427 | Materials panel: component "Driveway", Select Material: library "Blacktop 1" | Partial | RoadStrip.material by name ("Asphalt", "Concrete", "Gravel", "Stone", "Brick"); library material pick: C-87 |
| 427 | Terrain > Driveway > Straight Driveway: Midpoint indicator in front of the garage; drag straight down to the road | Works | TerrainVariant::Driveway |
| 428 | Default Settings > Terrain > Sidewalk: Width 60" | Works | default_settings_terrain.rs |
| 428 | Select the road; Auto Generate Sidewalks edit button; dialog: Offset from Road 60" | Missing | see CB-588 (Auto Generate Sidewalks) |
| 428 | Camera view near the intersection of driveway and road | Works | C-4 |
| 429 | Terrain > Terrain > Straight Sidewalk: Midpoint of the stair bottom edge; drag down; click along the right side and drag the second segment toward the driveway | Works | TerrainVariant::Sidewalk |
| 429-430 | Move the horizontal segment's handle to snap to the vertical segment's bottom | Works | handles |
| 430 | Curved sidewalk: delete both segments; Straight Sidewalk dragged at 315 degrees to the driveway; status bar angle | Works | status readout |
| 431 | Change Line/Arc edit button on the sidewalk; Reshape handle; Ctrl for fine adjustments | Missing | CAD-22; spline sidewalks exist |
| 431 | Annotations: street name via the Road object's label; Rich Text for driveway/sidewalk; custom hardscaping schedule | Partial | road label: Missing (RoadStrip has no label); schedule of terrain items: L-31 custom schedules General kind |
| 432 | Save a copy "Chic Cottage-Hardscape" | Partial | APP-96 |

### Lesson 24 assessment topics (pp. 432-433)

| Topic | Status | Evidence |
|---|---|---|
| Curb panel of Road Defaults controls curb cuts for driveways | Missing | no curb-cut option |
| Road parts outside the Terrain Perimeter do not show in 3D | Differs | draping |
| Driveway flares on the General panel of the Driveway Defaults and Specification | Missing | no flare |
| Change Line/Arc turns a straight road into a curve | Missing | CAD-22 |

## Lesson 25: Landscaping Design (pp. 433-450)

Workflow: lock road layers in the landscaping layer set, set Dirt material defaults, draw and edit a polyline Garden Bed, convert a copy of the lot polyline to a planting border Garden Bed (Change Line/Arc, Add Break with Sticky Mode, Make Parallel to setbacks), set Fencing Defaults (rails, posts, library panel), draw fences and avoid overlapping a retaining wall, place a gate, Plant Chooser, browse the Plants catalog, reverse and resize plant images, block shrubs into groups, explode, folders.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 433 | Plant image catalogs from Library > Get Additional Content | Out-of-scope | cloud content |
| 434 | Selected Edge of a polyline object editable in a variety of ways; Ctrl for fine adjustments | Missing | S-170 (Selected edge); Ctrl: S-121 |
| 435 | Active Layer Display Options: lock "Roads" and "Roads, Sidewalks"; hide "Terrain, Elevation Data" | Partial | LAY-3 lock/display Works; road layers: RoadStrip.layer is a free field defaulting to the terrain layer (model.rs) so "Roads" layers may not exist; not confirmed in code plan-core layers |
| 435-436 | Default Settings > Materials > Material Defaults: "Dirt" component, Select Material: library "Mulch - Dark" (Landscaping and Roadways > Bark and Mulch) | Partial | Project.materials defaults (Dirt class: not confirmed in code dialogs/materials.rs); Select Material dialog Missing |
| 436 | Terrain > Garden Bed > Polyline Garden Bed: click-drag from the garage corner to the stair back corner; snap | Works | TerrainVariant::BedPolyline (CB-49) |
| 437 | Edit handle on the bottom edge: drag to the sidewalk corner (Endpoint snap) | Works | polygon edge handles (tools/terrain) |
| 437 | Add Break; Move Edge handle drag to snap to the driveway | Works | CB-44 Break / CAD-95 |
| 438 | Diamond corner handle dragged onto the other corner removes the bottom edge | Works | vertex merge: not confirmed in code; handle drag onto a vertex |
| 438 | Selected Edge > Change Line/Arc; Reshape Arc handle drag to the sidewalk's inner curve | Missing | CAD-22; garden beds have spline variants (BedSpline, BedKidney) instead |
| 438 | Convert Polyline can create a generic Terrain Feature from existing objects | Partial | CAD-103 Partial |
| 439 | Select the plot-plan polyline (status bar: "Standard Polyline"); Copy/Paste then Paste Hold Position makes an identical copy | Works | S-83 |
| 439 | Convert Polyline dialog: Garden Bed radio; Garden Bed Specification with defaults | Partial | CAD-103/CAD-104; tools/cad.rs convert targets: not confirmed in code Garden Bed |
| 439 | Straighten the curved edge with Change Line/Arc | Missing | CAD-22 |
| 439 | Add Break then Sticky Mode (keeps Break active); click two points; Main Edit Mode | Missing | S-137 |
| 439 | Drag the middle edge handle upward past the deck railing | Works | handles |
| 440-441 | Selected Edge > Make Parallel/Perpendicular edit button: hover the angled setback line; dashed axis; click; edge becomes parallel; drag the edge's Move handle to snap | Missing | S-170 (Selected Edge) ; Make Parallel for a polyline edge: CAD-54 covers CAD lines; edge-level on terrain features: see S-170 |
| 441-442 | Add Break; drag the new corner to the retaining wall's back corner; pull the bottom edges back | Works | handles |
| 442 | "Terrain walls and fences follow the contours of the terrain" | Works | WallKind::Wall/Curb follows ground (landscape.rs); fencing as a regular wall class follows terrain? not confirmed in code (Fencing class WallClass::Fencing is a normal wall: plan-3d wall_kinds.rs): partial |
| 442 | Default Settings > Walls > Fencing > Newels/Balusters panel: Railing Height 72", Posts Width 5 1/2", Height 74"; Panels Library button: Select Library Object: Architectural > Fences and Railings > Ironwork > "Pole" panel | Partial | template `wall_variants` fencing_type "Fence-Wood-2", fencing_height 72; fence look (walls.rs line 107 `Fence`); library panel style: Missing (CB-395, W-116) |
| 442-443 | Rail Style panel: Include Top Rail, Include Bottom Rail; Raise/Lower Bottom 4"; Rails panel: Top and Bottom Rail Height 5 1/2", Width 1 1/2" | Partial | W-127 (Round 15 Rail Style); Rails profile table: W-116 Missing |
| 443 | Build > Fencing > Straight Fencing along the setback lines | Works | WallVariant fencing (W-56 family) |
| 443 | A fence drawn at the retaining wall's location replaces it because two walls cannot overlap; Undo restores | Partial | overlap handling for walls (W-36 Partial); terrain walls are a separate class: not confirmed in code |
| 443 | Move the retaining wall's Move handle (avoid the footing-edge handles); Endpoint indicator; Elevation Polyline moved outside the footing line | Partial | terrain wall handles: not confirmed in code; footing edges: plan-terrain wall has footing? see CB-648 |
| 444 | Gate: Library > Fences & Railings > Gates > "Wrought Iron Gate (arched)": click the fence; Interior Door Specification General: Height 72", Floor to Bottom 4" | Works | library doors/gates place in walls (DW-1); spec fields (DW-54) |
| 444 | Terrain > Plant > Plant Chooser (also toolbar button of the Library Browser) | Works | CB-50 (dialogs/terrain/object.rs `plant_chooser`) |
| 445 | Plant Chooser: Common Name "fir"; Search; results on the right; radio Common Name; selecting shows it in the Library Browser folder tree and as a plan placement preview with the Plant icon | Partial | CB-50 Partial (list by category with search; plants are terrain-owned runs, not library symbols) |
| 445 | View Item opens the Plant Information dialog (size and growing information) | Missing | see CB-617 (Plant Information dialog) |
| 445 | Click to place a tree image | Works | TerrainVariant plant placement |
| 446 | Inside a planting border Garden Bed, the pointer shows the Replace from Library icon; a click distributes copies of the tree within the bed | Missing | see CB-649 (plants distributed in a garden bed by click) |
| 446 | Browse Plants > Trees > Deciduous > Betula > Betula papyrifera: place two | Works | library catalog |
| 447 | Plant Image Specification, Image panel: Reverse Image; Move handle and a top-edge Resize handle | Partial | plant runs have size/form fields (CB-50); plants as images: billboards (PlantForm::Billboard); Reverse Image: Missing |
| 447-448 | Place three "Rhododendron (cream)" shrubs; select as a group; Make Architectural Block; Move and Rotate handles; Copy/Paste copies rotated differently; Explode Architectural Block | Missing | CB-427 |
| 448 | Rich Text for the privacy fence; Plant Schedule | Partial | TXT-4; plants are in the Plant Schedule (CB-50 note, ScheduleKind::Plant) |
| 449 | Save a copy "Chic Cottage-Landscaping"; New Folder "Landscaping Tutorials"; drag four plans | Missing | APP-96, APP-95 |

### Lesson 25 assessment topics (p. 450)

| Topic | Status | Evidence |
|---|---|---|
| Default materials for Garden Beds ("Dirt") and plain Terrain Features ("Foundation/Slab") are in the Material Defaults dialog | Partial | dialogs/materials.rs Materials Defaults; class names not all confirmed |
| Copy and Paste in Place creates a copy at the same location with one click | Works | S-83 (Paste Hold Position) |
| Make Parallel/Perpendicular matches an object's or a selected edge's angle to another | Partial | CAD-54 Works for CAD lines; selected edge of a terrain feature: S-170 Missing |
| Plant Chooser dialog searches the Library by common name | Works | CB-50, CB-612 |
| Garden Beds are closed polylines edited like other closed polylines (Add Break, Change Line/Arc, Move Edge) | Partial | CAD-95 Works; Change Line/Arc CAD-22 Missing |
| Fencing is a special wall; a gate is placed like a door | Works | W-56 family, DW-1 |
| Plants can be blocked into groups (Make Architectural Block) | Missing | CB-427 |

## Lesson 26: Layout Page Templates (pp. 451-460)

Workflow: open the project's layout, make page 0 a "Standard Template" and page 1 a "Cover Sheet Template" (Layout Page Information: Title, Use as Page Template), assign templates and titles to pages 2-6, give them Labels with a "#" numbering convention (A0.#, A1.#, E1.#), insert pages before and after, save a layout revision copy.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 451 | One layout per project is usually enough; open the layout from Dashboard, Recent Documents, Project Browser, File > Open Plan/Layout, or Open All | Partial | the layout lives inside the plan file (`Project.layout`) and a second layout can be added (NewLayoutFile, SwitchLayout in layout_window.rs); no separate layout document to open (Differs); APP-117 |
| 452 | Layout page templates are part of the layout template file | Works | LayoutTemplate (template.rs), Save As Template / Apply Template (L-10) |
| 452 | Project Browser lists the layout's pages; templates do not print as part of a range | Partial | shell/docks.rs layout node (pages); template pages are skipped by `content_pages()` (titleblock.rs line 144) |
| 452-453 | Two page templates: one for the cover and general notes, one for the sheets that follow | Partial | L-140/L-190: only one template page set ("Chief's Page Template page: its boxes and CAD repeat on every other page", model.rs `template_page`); several named templates assigned per page: Missing; see L-190 already implied by L-190 Partial |
| 453 | Tools > Layout > Page Down to page 0; Tools > Layout > Edit Page Information: Layout Page Information dialog: Title "Standard Template", Use as Page Template checked | Partial | Layout > Page Specification (PageSpecDialog: title, sheet number, template flag, own sheet size, no title block); no Label, Description, Comments, Include in Layout Table (L-191 Missing) |
| 453-454 | Project Browser > layout > Pages folder: icons for template, blank, content pages; green check on the open page | Partial | pages are listed with sheet number and title (shell/docks.rs); distinct icons for template, blank and content pages: not found |
| 454 | Right-click page 1 > Edit Page Information: delete the Label, Title "Cover Sheet Template", tick Use as Page Template; the icon changes | Partial | page context menu in docks.rs offers Insert Page After and other page commands; Page Specification opens from the Layout menu (LayoutCommand::PageSpecification) |
| 455 | Assign templates: Edit Page Information, Selected Page drop-down "Page 2", Page Template drop-down "Cover Sheet Template"; Page 3 "Standard Template"; pages 4-6 same | Missing | no per-page Page Template assignment (L-192 Partial: only the flag); the selected-page drop-down inside the dialog (L-191) |
| 456 | Pages 2-6 appear as blank pages; Titles "Cover Sheet", "General Notes", "Site Plan" set from the Selected Page drop-down | Partial | titles per page Work (L-7); editing other pages from one dialog: Missing |
| 457 | Page Label with prefix and `#`: "A0.#" on page 3 (General Notes), "A1.#" on pages 4 and 5, "E1.#" on page 6; the browser groups by prefix | Missing | L-188: LayoutPage has only `number: u32` (sheet "A-{n}", model.rs `sheet_number`) and `title`; no free-text Label and no per-prefix auto numbering. BREAK: the sheet numbers A0.1, A1.1, A1.2, E1.1 that the guide produces, and the US National CAD Standard style, cannot be set |
| 458 | The `#` is replaced by the sequence number among pages with the same prefix | Missing | L-188 |
| 458 | Tools > Layout > Change Layout Page (or the page-number button) to page 5; Insert Page Before: original page 5 becomes 6 and active; the new page 5 has no label or title | Works | LayoutCommand::InsertPageBefore, GoToPage (L-186 Partial) ; menus.rs line 2237 |
| 458 | Edit Page Information on the new page: Label "A1.#", Title "Foundation Plan", Page Template "Standard Template" | Missing | label and template assignment as above |
| 458-459 | Project Browser: right-click page 6 > Insert Page After; page 7 new; edit: Label "A2.#", Title "Elevations"; add another after 7: "Sections" | Partial | InsertPageAfter exists in docks.rs row menu (line ~855); label Missing |
| 459 | File > Make a Copy creates "Chic Cottage-Page Templates" layout in the project | Partial | APP-96; Save As Template / NewLayoutFile |
| 460 | Quiz: Project Browser lists page templates, blank pages and pages with content; Layout Page Information assigns sheet titles; `#` in the Label sets prefix numbering | Partial | see rows; L-191, L-188 |

### Lesson 26 assessment topics (p. 460)

| Topic | Status | Evidence |
|---|---|---|
| The side window that lists layout pages (Project Browser) | Works | docks.rs |
| Page categories: templates, blank pages, pages with content | Partial | |
| What a page template holds (title block, border) | Works | L-10 |
| Dialog that assigns sheet titles (Layout Page Information) | Partial | PageSpecDialog |
| `#` in the Label for custom page numbering | Missing | L-188 |

## Lesson 27: Title Blocks and Borders (pp. 460-483)

Workflow: Drawing Sheet Setup (size, orientation, 1 in = 1 in scale, margins), Grid Snap Unit 1/8" in General Layout Defaults, delete the stock border, check the CAD line weight and style defaults, draw a border with Rectangular Polyline and Concentric edit behaviour (Jump 1/8"), build the title block frame and component frames with Copy/Paste, concentric copies, Point to Point Move, Nudge and temporary dimensions, Rich Text defaults and text with centring, text macros for title, label and date, a Layout Revision Table, page revisions, a cover sheet template with a copied border, a logo picture, a title bar, a Layout Page Table as a sheet index.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 461 | Center Object centres text and CAD inside polylines; text macros report page numbers | Missing | Layout page edit tools are Align, Distribute, Copy to Page and Duplicate (LayoutCommand in shell/layout_window.rs); there is no Center Object and no Point to Point Move on layout boxes or page CAD. see L-231 |
| 462 | Import an image of your logo; layout template files for new layouts | Works | AddImageBox (layout_window.rs), layout templates (L-10) |
| 462 | Grid Snaps on in layout; Grid Snap Unit also sets the Nudge distance | Partial | Layout drag snap is a fixed 1/16 in (`SNAP_IN`, layout_window.rs line 66) and arrow keys nudge 1/16 in (Shift 1/4 in); no editable Grid Snap Unit and no General Layout Defaults page. see L-230 |
| 462 | Defaults before drawing a title block: Drawing Sheet, Grid Snap Unit, default CAD line weight and style, Rich Text defaults | Partial | see rows |
| 463 | File > Print > Drawing Sheet Setup: Orientation, Size, Drawing Scale 1 in = 1 in (layout files always), Margins | Partial | L-204 Partial, L-207 Missing (margins); layout scale is implicit 1:1; Customize Sheet Sizes exists (CustomizeSheetSizes command, Daniel's ARCH sizes) |
| 463 | Window > Fill Window to see the whole sheet | Works | LayoutCommand::FitPage |
| 463 | Installed layout templates ship with a title block and border on page 0 | Works | Daniel 18x24 block (L-10); TitleBlockTemplate (titleblock.rs line 189) |
| 464 | Edit > Default Settings > Layout > General Layout Defaults: Use Snap Grid/Units; Grid Snap Unit 1/8" | Missing | no Layout defaults page in default_pages (page list: architectural, cad, camera, dimension, electrical, plan, page); see L-230 (General Layout Defaults) |
| 464 | View > Drawing Sheet toggles the sheet display so it isn't group-selected | Partial | Menu has a Drawing Sheet entry (menus.rs line 2074) for the plan view; in layout the sheet is the page itself (not a selectable object, so the group-select problem does not arise) |
| 464 | Page Down to page 0, marquee-select everything and Delete to remove the stock title block and border | Works | layout CAD selection and delete (layout_window.rs); template page 0 |
| 464 | Tools > Layer Settings > Display Options in layout: Layout Page Display Options dialog; layer "CAD, Default": line weight 18, solid style | Partial | LayoutLayers (layers.rs) with LAYER_CAD "Layout CAD" and LAYER_TEXT; dialog LayerDisplay command; weight in 1/100 mm vs Chief units; default layer names differ (Differs) |
| 464-465 | Preferences > Behaviors: Edit Type Concentric, Jump 1/8" (the pointer shows the Concentric icon) | Partial | Edit Behaviors dialog (dialogs/edit_behaviors.rs) works in plan CAD; whether layout CAD honours the Concentric behaviour: see L-231 (layout CAD edit behaviours) |
| 464-465 | CAD > Boxes > Rectangular Polyline: drag across the Drawing Sheet; Endpoint snaps at the corners; the Sheet Boundary object is selectable (Select Next Object) | Partial | LayoutTool::Box draws a rectangle; layout drawing snaps only to the 1/16 in grid (no Endpoint or other object snaps, layout_window.rs `snap`); see L-231 (object snaps for layout CAD) |
| 465 | Corner handle drag toward the centre makes the polyline step by the Concentric Jump; Copy/Paste then corner drag makes a concentric copy (double border) | Partial | concentric works for plan CAD (CAD-25); layout: same see L-231 |
| 465 | Fillet Lines on the border corners | Missing | CAD-98 Partial in plan; layout CAD: no fillet tool (layout CAD tools: Line, Box, Polyline, Text, TextBox, Circle, Arc, Leader, Cloud) |
| 466 | Title block frame: Copy/Paste of the inner border with C held (temporary Concentric hotkey), two jumps = 1/4"; Selected Edge temporary dimension: type 2" | Missing | no temporary dimensions or Selected Edge on layout CAD; no Concentric copy in layout (layout tools: Line, Box, Polyline, Text, TextBox, Circle, Arc, Leader, Cloud). see L-231 |
| 466-467 | Component frame: concentric copy 1/8", Selected Edge dimension 1"; replicate with Copy/Paste + Point to Point Move; Up arrow nudges 1/8" | Missing | no concentric/Point to Point copies on layout CAD; Duplicate and Copy to Page exist; nudge 1/16 in. see L-231 |
| 468 | Further frames at 4", 5", 4" tall via temporary dimensions | Missing | as above |
| 468 | Default Settings > Text, Callouts and Markers > Rich Text: font "Chief Blueprint" at 1/8"; Bold/Italic off by default | Partial | DS-28; text style by name/size; the "Chief Blueprint" font: fonts.rs (docs/fonts.md reads Chief fonts from Daniel's install) |
| 469 | Rich Text: Align Center button; company name 3/16" typed in the size field for the highlighted part; Rotate handle 90 degrees | Partial | layout text boxes (TextBox dialog); mixed sizes inside one text via run markup `<size=...>` (TXT-4); alignment buttons TXT-3; rotation of layout text: CadItem::Text has `angle` |
| 470 | Center Object: horizontal axis against the left side of a frame; again against the top edge for vertical centring | Missing | no Center Object on layout objects (see first row) |
| 471 | Project information in the 5" frame above the company block | Works | title block macros (%project.name%, %client%, %address%, %designer%...) |
| 471-472 | Insert Macro button: Global > Layout Info > "Layout Page Title" inserts `%layout.title%`; shows the page 0 title "Standard Template" | Partial | our macro is `%sheet.title%` (titleblock.rs); text inside layout text boxes expands them; there's no Insert Macro menu and the Chief macro names differ (Differs); L-189 Missing |
| 473-474 | `%layout.label%`: shows "A0.1" on page 3 and a blank label on page 0 (border still drawn); Border on the Appearance panel | Missing | L-188, L-189 (no label); text box border exists (TXT-3) |
| 474-475 | Date: Insert Macro > Global > Time Date > "Short Date" inserts `%date.short%` | Partial | `%date%`, `%date.long%` exist (titleblock.rs); `%date.short%` name differs; value comes from Project Information date, not the current date (Differs) |
| 475 | Tools > Layout > Layout Revision Table: click to place; rotate 90 degrees; P2P Move; Up arrow; Revision Table Specification Columns/Rows: Minimum Rows 7; Resize handle | Partial | L-196 Partial (revision table in the title block REVISIONS table from Project Information; a separately placeable Revision Table object: L-62 Partial) |
| 476 | Page 3 > Edit Page Information > Page Revisions: New; Revision Specification: Page Range "3,4", label, description; the table shows the revision only on those pages | Missing | L-193 (Page Revisions table Missing), L-194 Partial: project-level revision list has no page range (plan-core schedules.rs `revisions: Vec<(String,String,String)>`) |
| 476-477 | Cover Sheet Template: copy both border polylines from page 0 to page 1 (Page Up, Paste Hold Position) | Partial | Copy to Page copies boxes (LayoutCommand::CopyBoxToPage); copying page CAD polylines between pages with Paste Hold Position: not found (page CAD is not in the plan clipboard); see L-236 |
| 477-478 | File > Import > Import Picture: logo fills the view; Picture File Box Specification General: Save in Plan; Retain Aspect Ratio; size 3" x 3"; Label panel: Suppress Label in All Views | Works | AddImageBox embeds the picture in the plan file (BoxSource::ImageData) so it is saved in the plan; Suppress Label in All Views: Missing (L-182) |
| 478 | P2P Move the image to the border corner; Up x2, Right x2 nudge 1/4" | Partial | nudge distance |
| 478-479 | CAD > Lines > Draw Line across the sheet above the picture; move down to snap to the picture's top; Up x2 for a 1/4" gap; company info aligned left and project info aligned right | Partial | LayoutTool::Line; alignment of text boxes TXT-3 Works |
| 479 | Tools > Layout > Layout Page Table: click to place; Layout Page Table Specification Columns/Rows: Name "Sheet Index"; remove Description and Comments; rename "Label" -> "Sheet" | Partial | AddSheetIndex places a sheet index table box (L-11); the Layout Page Table dialog edits sheet, title and template flag only (dialogs/layout.rs PageTableDialog); column choice and rename: Missing (L-195) |
| 480 | Attributes panel: Horizontal Alignment Centered; Title Text Style panel: Font and Character Height 1/4" | Partial | L-195 Partial |
| 480 | Resize columns by handles; P2P Move and nudge into the top right corner; the table fills as pages gain content | Partial | table boxes resize as boxes; column handles: L-63 |
| 480-481 | Main Title Display unchecked; a separate Rich Text 5/8" title above | Partial | L-195 |
| 481 | File > Make a Copy "Chic Cottage-Title Block"; Project Browser New Folder "Layout Tutorials"; drag two layouts | Missing | APP-96, APP-95 |
| 482 | Quiz: Grid Snap Unit controls | Partial | see grid row |

### Lesson 27 assessment topics (pp. 482-483)

| Topic | Status | Evidence |
|---|---|---|
| Snap Grid Unit: interval for drawing/resizing/moving and arrow-key nudge | Partial | fixed 1/16" |
| Three ways to enable Concentric (Preferences Behaviors, Edit > Behaviors, hold C) | Partial | S-65 |
| Center Object aligns an object along an axis through another object's side or midline | Works | S-53 |
| Point to Point Move snaps one object's corner to a point of another | Works | S-52 |
| Two font sizes in one Rich Text object | Works | TXT-4 |
| Add a logo with File > Import > Import Picture | Works | AddImageBox |
| Text macros for the layout page title and page numbers | Partial | `%sheet.title%` / `%sheet.number%` |
| Layout Revision Table and Layout Page Table | Partial | L-196, L-195 |

## Lesson 28: Sending Views to Layout (pp. 483-500)

Workflow: set the plan's default Drawing Scale, send the Floor Plan View Dimensioned, Electrical Plan View and a Plot Plan View (1 ft = 100 ft) to numbered pages, rotate, crop, centre and relabel the site view, send a saved cross section and a back-clipped elevation (Plot Lines with Color Fill), switch a box to Live View, crop and move boxes, customise a callout label that links to a page, send a Full Camera view as a picture, print to PDF on ARCH D.

| Page | Step (tool / dialog / field) | Status | Evidence or break |
|---|---|---|---|
| 484 | Layer sets (Plot Plan Set, Electrical Set) serve both for working and for views sent to layout | Partial | Send to Layout dialog picks a layer set for the plan view (dialogs/layout.rs "Layer set" row); layer sets per view Work |
| 484 | Ctrl/Cmd+U = Send to Layout | Differs | Daniel's hotkeys (SendToLayout command exists) |
| 485 | Layout template with sheet size, page organisation, title block before sending | Works | L-10, Create Construction Set |
| 485 | The layout file's Drawing Scale should always be 1 in = 1 in | Works | implicit (layout in paper inches) |
| 485 | Layout Box Defaults (line and fill styles, label format) | Partial | L-139 Partial |
| 485 | File > Print > Drawing Sheet Setup in the plan: Drawing Scale 1/4 in = 1 ft sets the default scale for views sent to layout | Partial | L-200 Partial, L-204/L-205 Partial: our Send to Layout dialog has its own Scale row; no plan-wide default drawing scale feeding it |
| 485-486 | Saved Plan View Control: "Floor Plan View Dimensioned" | Partial | plan view selector; no starter view of that name |
| 485 | Project Browser > layout Pages folder lists pages and titles ("the Floor 1 plan goes to Page 6") | Works | docks.rs pages with titles (labels missing: L-188) |
| 486 | File > Send to Layout: Choose Layout (the project's layout, others, "New Layout") | Works | dialogs/layout.rs "Send to" row (layout files, new) |
| 486 | Send Position: Page Number 6; Show Layout Page checked | Works | "Page" row; the dialog switches to the layout (`ShowLayout`) |
| 486 | Send Options: Entire Plan/View | Partial | L-154 Partial (Entire Plan/View default; Current Screen variants) |
| 486 | Link Saved Plan View checked: the layout view follows the saved plan view | Missing | L-155 (dynamic link to saved plan view settings), L-158 |
| 486 | Scaling default 1/4 in = 1 ft | Works | "Scale" row; dialogs/layout.rs |
| 487 | OK sends the view to the centre of the page; the label under it reports its name | Works | Placement::Centered; LayoutBox.label |
| 487 | Send "Electrical Plan View" to Page 9 | Works | same dialog |
| 487 | A site plan uses a different scale | Works | scale list (L-22) |
| 488 | Send the Plot Plan View: Fill Window first; Scaling 1 ft = 100 ft; a message warns the view is too large for the page at that scale | Missing | the Send to Layout scale list is `Scale::ALL` (3 in down to 1 in = 20 ft) plus Largest that fits (dialogs/layout.rs lines 405-416): no 1 in = 30/40/50/60/100 ft site-plan scales and no typed ratio (the Print dialog has a custom ratio). BREAK for the site plan page. see L-229 |
| 489 | Rotate the box 90 degrees clockwise with the large Rotate handle; crop with side Resize handles; fine-tune by temporary dimensions | Partial | RotateBox is a quarter turn button; a free rotate handle on the box: L-4 Partial (rotate knob on the selected box); cropping `clip` Works |
| 489 | Center Object relative to the inner border polyline's top edge, then to the sheet's left edge | Missing | no Center Object on layout boxes; Align/Distribute only |
| 489 | Move Label handle: move the label from left to bottom centre | Partial | label placement is a box setting; no label drag handle found (L-165 Partial) |
| 489 | Add legal lot description and directions with the text tools on the layout page | Works | LayoutTool::Text / TextBox |
| 489-490 | Saved cross sections/elevations are listed under "Cross Sections" in the Project Browser and can be sent at any time | Partial | cameras node in the browser (docks.rs `SendCamera`); folder name differs |
| 490 | Open View "Stair Section"; File > Send to Layout; Page 8; Scaling 1/2 in = 1 ft | Works | BoxSource::Camera; scale list |
| 490-491 | Back Clipped Cross Section drawn as an exterior elevation: camera in the terrain outside the roof overhang, line of sight horizontal into the porch stairs | Works | C-19 Back-Clipped Cross Section |
| 491 | Zoom in and annotate with CAD and Text (roof heights, pitches, materials) in the section view | Missing | DIM-62 and CAD in camera views Missing (CAD-132 family) |
| 491 | 3D > Edit Active View: Name "Exterior Elevation - Right"; the Saved check box checks itself | Partial | camera Name field in dialogs/camera.rs; a camera that is named is kept (saved) by default in our model |
| 492 | Send to Layout: Page 7; Plot Lines radio with Color Fill; Scaling 1/4 in = 1 ft | Missing | L-156 (Camera View Options: Live View or Plot Lines with Color Fill) ; L-161 Plot Line views Missing; our camera boxes are live renders with hidden-line/vector (C-22) |
| 492 | Message: the view is too large for the sheet at that scale | Missing | as above |
| 493 | Images (trees) are not included in Plot Line views | Missing | L-161 |
| 493 | Layout Box Specification Linked View panel: Camera View Options: Live View | Missing | L-178 |
| 493 | A Live View needs a saved camera; deleting it leaves the box empty | Partial | L-166 Missing (missing-view caution) |
| 493-494 | Crop the elevation box by dragging side handles; Move handle to a page corner; move the Stair Section on page 8 | Works | LayoutBox.clip, rect_in handles |
| 494 | Label panel: callouts, markers or regular text; Callout Label Specification (Edit next to Use Callout) | Missing | L-165 Partial (labels are text); no callout-shaped labels linked to cameras on layout; L-150 Partial |
| 494-495 | Callout panel: Callout Label macro `%referenced_view_callout_label%` -> "S1"; Attributes: Text Above Line `%automatic_label%` -> "Stair Section"; Text Below Line `%box_scale%` | Missing | L-145 (object-specific macros in labels) Missing; see TXT-65, L-145 already raised for referenced-object macros |
| 495 | Automatic Text Below Line; Link panel: Link button: Link View dialog: choose "Page 6 - A1.3 - Floor 1 Plan": the callout shows the linked page's label | Missing | L-150 (callout linked to detail); page-linked callouts: see L-232 (a section/elevation callout that reports the sheet where the view is drawn) |
| 495-496 | Callout Arrows: Large, Filled, Number of Arrows 1; Cross Section Line Length; Alignment of Text Above Line "Away From Callout" | Missing | C-158 Partial (callout placement, section line style); layout callout arrows: Missing |
| 496 | Label Position Reference "Bottom Center" | Partial | LayoutBox.label is text with a fixed place under the box; Position Reference choices: Missing (L-182) |
| 496 | 3D > Create Perspective View > Full Camera; Orbit; 3D > Edit Active View > Camera panel Name "Cover Sheet View" | Works | C-4, dialogs/camera.rs |
| 497 | Send to Layout: Page 2; Send Options: Current Screen as Image (other options unavailable); a "Picture File Box" is created | Partial | dialogs/layout.rs "Send a picture of the 3D view as it is now" checkbox with Width/Resolution (BoxSource::Perspective render); L-154 Partial |
| 497-498 | Picture box: corner Resize handle keeps the aspect ratio; side Reshape handles crop the picture | Partial | picture boxes (AddImageBox) resize; crop handles: L-170 Partial |
| 498 | Try Technical Illustration or Hand Drawn Lines rendering techniques | Partial | C-50 techniques: Glass House and others; Technical Illustration and Hand Drawn Lines: not found by name |
| 498 | File > Print > Print: Print Layout dialog: Destination "Chief Architect Save as PDF"; Paper ARCH D (24" x 36"); Save as PDF; Choose PDF File Name | Works | dialogs/print.rs (PDF file as destination, paper size list incl. ARCH D, L-18/L-20) |
| 498-499 | Multiple layouts for plan variants (e.g. with and without dormers) | Works | NewLayoutFile / SwitchLayout (plan keeps several layout files) |

### Lesson 28 assessment topics (pp. 499-500)

| Topic | Status | Evidence |
|---|---|---|
| Plan-wide default drawing scale in Drawing Sheet Setup | Partial | L-200 |
| The Project Browser's page list guides the Send Position | Works | docks.rs |
| Live Views include images, Plot Lines do not; Live View needs a saved camera | Missing | L-156, L-161 |
| Current Screen as Image makes a resizable picture box | Partial | L-154 |
| Resizing a layout box crops its contents | Works | LayoutBox.clip |
| Default layout template file chosen on the New Plans panel of Preferences | Missing | PR-25, DS-33 Partial (template per unit system) |

## Rows not confirmed in code

About 100 step rows say "not confirmed in code": I read the surrounding dialog or tool but not the exact path (for example whether a symbol's Label tab has an offset field). The status in those rows is my best reading; before building from one, grep the named file first.

## Source files read for this part

`crates/plan-app/src/{tools,dialogs,editor,shell,menus.rs,toolbar.rs}`, `crates/plan-core/src/{defaults,schedules,deck,dim_assoc,text_styles,units,walls}.rs`, `crates/plan-layout/src/{model,template,titleblock}.rs`, `crates/plan-docs/src/pdf/scale.rs`, `crates/plan-terrain/src/{model,landscape}.rs`, `crates/plan-framing/src`, `crates/plan-app/assets/templates/chief-x18-daniel.json`, `docs/parity-status.md`, `docs/parity/*.md` and `DECISIONS.md`. No Rust file, CHANGELOG or integration queue was edited, and no cargo command was run.
