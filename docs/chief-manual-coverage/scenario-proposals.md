# Scenario test proposals: replaying Chief's own tutorials (manual audit part 7)

Written 2026-10-08 with `part7-tutorial-workflows.md`. One headless scenario per tutorial lesson, so the sweep can replay the 28 lessons of Chief's Tutorial Guide against Plan Studio. No Rust is written here; each entry gives a name, the steps in terms of the existing `Sim` harness (`crates/plan-app/src/scenarios/mod.rs`: `Sim::new`, `tool`, `drag`, `click`, `double_click`, `key`, `esc`, `action`, `undo`, `redo`, `open_spec`, `ok`, `cancel`, `dialog_frame`, `plan_shapes`, `floor_walls`) and the assertions. The figures come from the guide's own steps; the shell is approximated as the guide's roughly 40 ft sides with a garage bump-out and a porch, because the guide shows its plan only as pictures.

**Conventions.**
- Put the new files next to the existing scenarios as `scenarios/tut01_exterior_walls.rs` ... `tut28_send_to_layout.rs`, registered with one `mod` line each in `scenarios/mod.rs`, and share a helper `chic_cottage(sim: &mut Sim)` in a new `scenarios/tut_support.rs` that draws the cumulative shell the later lessons stand on (the guide carries one plan through lessons 1 to 25).
- Every step that the guide says "remember to Save" is one `sim.action(Save)`-style call and an assertion that the plan is clean; every "File > Make a Copy" is `SaveCopy` plus an assertion that the copy file exists (the break is recorded, not hidden).
- Where the audit found a break, the test is written to the guide's behaviour and carries `#[ignore = "T7-nn: <break id>"]` with the parity id from `part7-tutorial-workflows.md`; when the feature lands the attribute is removed. This keeps the gate green and turns each break into a one-line switch.
- Each scenario also asserts "one undo step per user action" for the steps that edit the model (the project rule).
- "Today" says what the audit expects: **runs** (should pass now), **partial** (passes up to the named step, then `#[ignore]`d), or **ignored** (the first step already breaks).

---

## 1 tut01_exterior_walls (pp. 3-27)
Steps: new plan; Wall Defaults: set the default exterior type to a copy of Siding-6 named Stone-6 with layer 1 = 3" (Wall Types dialog `New Type`, rename, edit thickness, OK); set 1st Floor ceiling height 97 1/8 in Floor Defaults; draw walls (2 exterior, then pony walls) clockwise with `drag`, approximate lengths, to a 40 x 40 outline with a bump-out; Auto Exterior Dimensions; move three walls by typed dimension values (`click` the dimension, `key` digits, Enter); resize a wall via its length dimension; add a Rich Text with typed text; File > Make a Copy.
Assertions: the walls close into one room with a Living Area; the two walls drawn before and after the default change have different types; the room's ceiling height reads 97 1/8; every dimension edit moves exactly one object and is one undo step; the Back Clipped Cross Section camera exists. Break checks: layer fill, Select Material, Floor/Ceiling Platform Defaults (R-143), Auto Refresh (RF-164), Move Both Ends (DIM-48), Uppercase.
Today: partial (up to the Wall Types dialog; ignore from "Platform Defaults").

## 2 tut02_interior_walls (pp. 28-44)
Steps: Interior Wall defaults (Interior-4); draw three interior walls from midpoint snaps; assign Kitchen, Dining, Living, Entry, Bedroom, Garage by `open_spec` on each room; subdivide the bedroom; delete one room label; merge two closets by dragging a wall end; set two walls to Fire-6 (copy of Interior-6 with a red main-layer fill) and Reverse Layers; make a Room Divider by changing a wall; Interior Dimension across the garage; Auto Room Dimension for the entry; delete dimensions.
Assertions: each new room inherits its parent's type; the merged room keeps the larger room's type; the Room Divider has thickness 0 and is dashed; the Interior Dimension reads 11" less than Auto Exterior across the same wall (two 5 1/2" framing layers); Delete Objects with All Rooms On This Floor removes manual dimensions.
Today: partial (ignore at Room Divider wall type W-152, Auto Room Dimension R-107, layer fill).

## 3 tut03_multiple_floors (pp. 45-59)
Steps: Build New Floor deriving the 2nd floor from the 1st; make Shingle-6 and swap Siding-6 upper walls; Build Foundation (Walls with Footings, min stem 113 1/8, garage options); copy `8" Concrete Stem Wall` to a furred type with an Air Gap and a Framing layer; Reference Display; draw a wall over the red lines and assert alignment; nudge 1" then Align With Wall Above; Copy + Paste Hold Position onto Floor 2; Wall Schedule with columns Total Width / Wall Construction Upper and Lower in a new CAD Detail "Legends", rename a column, drop Room Divider from the schedule.
Assertions: the derived floor's exterior walls take the floor-1 types; the garage has a 12" curb and stems 24" below its slab; the pasted wall equals the original in x,y; the schedule lists exactly the used wall types. Breaks: W-145, R-133/R-131/R-145, W-138, L-235, L-233.
Today: partial (ignore at garage options and Align With Wall Above).

## 4 tut04_interior_stairs (pp. 60-81)
Steps: Interior Stair Defaults (width 44, newel 4 x 44); Straight Stairs by one click between garage and foyer; Draw Stairs between floors; locate by dimension 4' 6"; Staircase Specification: Lock Bottom, Make Best Fit, Lock Number of Treads, tread depth 10 1/2; a Back Clipped Cross Section named "Stair Section"; Auto Stairwell; End to End dimension in the section; stacked basement stair by Copy + Paste Hold Position; turn off the Stairs & Ramps layer; Note and Note Schedule defaults; Notes numbered 1.
Assertions: the stair reaches the next floor (riser count 16 for the lesson's heights, tread 10 1/2 after the edit); the floor platform above has a hole; Note schedule lists one row for equal notes; the stairwell room type. Breaks: CB-160, CB-162, DIM-62 (headroom dimension in a section), C-136 (clip planes), TXT-56 (Capsule).
Today: partial (ignore at Make Best Fit and the section dimension).

## 5 tut05_doors_windows (pp. 82-105)
Steps: Door and Window defaults (window 30 x 54, lites 3 x 4); place hinged (press-drag sets hinge and swing), sliding, doorway, bifold, pocket, garage doors; Change Opening/Hinge Side, Change Swing Side; resize with C held; Lites 3 x 5; hardware via the library; 11 windows; edit window types (Fixed Glass 12 x 77, floor to top 80); Center Object (room, Room Divider, stairs axis); dimension edits 4 1/2" and 6"; reflect a window about the door; Transform/Replicate 2 copies at Y delta -35 1/2; Make Mulled Unit; Door and Window schedules with Dimensions/Top columns, Use Label; Comment "20 minute" and a leader macro `%comment%`; Note 2 copies share a number.
Assertions: exterior vs interior class by wall; mulled unit is one object; three windows centred in the kitchen; schedule columns as set; the leader text equals the door comment. Breaks: DW-168, CB-395, TXT-65.
Today: partial (ignore at the lock library pick and `%comment%`).

## 6 tut06_decks_porches (pp. 105-126)
Steps: Deck Railing / Railing / Half Wall defaults (post to beam, 5 1/2 posts, balusters 1 1/4 at 5); Room Types Porch / Deck structure; two Half Walls enclose the porch, then set Porch; three Deck Railings enclose a Deck; move auto exterior dimension lines with Transform/Replicate; add extension lines; resize the deck by dimensions; Create Terrain Perimeter, Absolute Elevation -28 with a Reference Point; porch stem walls (Reference Display, Foundation wall, Align With Wall Above, Stem Wall Height 37 1/2, Floor Under off); Exterior Stair Defaults; one-click deck stairs to grade (DN label, Doorway in the railing); concrete porch stairs; Notes renumbered by Move Up in Schedule.
Assertions: deck room named Deck and excluded from Living Area; porch has roof and ceiling flags; deck post footings stop at the terrain; stair is centred on the railing opening. Breaks: CB-627, CB-530/531, W-145, CB-109, CB-170, L-63.
Today: partial (ignore at the terrain-linked footings, DN stairs, Move Up in Schedule).

## 7 tut07_basic_roof_styles (pp. 127-135)
Steps: new plan; a 34 x 24 ft clockwise rectangle; Build Roof (hip); then for each style set wall directives on the walls and rebuild: gable (two walls Full Gable), Dutch (Dutch Gable, start 180), shed (two Full Gable + one High Shed/Gable, pitch 2), offset gable (one wall pitch 12), gambrel (side walls gable; both long walls upper pitch 6 start 156; pitch 12), gull wing (125, pitch 3), half hip (gable walls upper pitch 3 start 170), mansard (all four walls upper 1.5 at 132, pitch 24). Each style: select walls with Shift and open ONE Wall Specification.
Assertions: plane counts and ridge heights per style (ridge present or not, number of planes: hip 4, gable 2, shed 1); upper-pitch break height equals the Starts At value. Breaks: RF-166 (group Roof tab), RF-165 (In From Baseline).
Today: ignored for the group steps; runnable if each wall is set separately.

## 8 tut08_chic_cottage_roof (pp. 136-158)
Steps: Roof Defaults (auto rebuild on, rafters, pitch 12); Full Gable on the left, garage front and porch walls; a nested gable (new Shingle-6 wall drawn over the porch half walls); reverse gable by Add Break and typed 28'; deck room Roof Over + Flat Ceiling off, side deck railings Full Gable, horizontal railing pitch 2; Build Roof raise/lower 18 1/8 with Ignore Top Floor; edit nested gable wall; porch half-wall gull wing 4 in 12 / upper 12 at 84; eave edit with the Auto Rebuild question; Join Roof Planes into a curved valley; Auto Roof Return Hip + Frieze; Edit All Roof Planes frieze; Display On Floor Above; O.H. dimension with leading text.
Assertions: one ridge across; gap behind the nested gable closes; raise value equals wall-top-to-underside distance; the roof planes sit on floor 2 after Display On Floor Above. Breaks: RF-166 (half wall Roof tab), RF-61, RF-86, RF-96, DS-27.
Today: partial (ignore at the porch gull wing, curved plane, Display On Floor Above).

## 9 tut09_dormers (pp. 159-176)
Steps: Dormer defaults (Shingle-6, width 96); window defaults 72 x 48, floor to top 93 1/2; Auto Floating Dormer, centre by Center Object, typed 2'; window Double Casement; Ceiling Break Lines layer on, Tape Measure; interior wall as Knee Wall at 8' and Attic room; Auto Dormer; manual dormer: Roof Hole, three exterior walls, window, Roof Plane baselines, typed -6", Join Roof Planes (ridge and valley), resize hole edges; Rich Text and a box parallel to the valley; leader lines.
Assertions: the floating dormer's walls stop above the floor, the Auto Dormer's walls reach the floor; hole edges coincide with the dormer walls; ridge and valley are straight lines. Breaks: RF-166 (knee wall on an interior wall), RF-80, S-137.
Today: ignored at the knee wall step.

## 10 tut10_custom_ceilings (pp. 177-190)
Steps: Ceiling Finish Definition (drywall + paint); garage fire-rated drywall + 1 1/2 insulation; hat-channel framing type and member; Floor 0 Ceiling Finish with a 1" hat channel layer at 24 OC; Back Clipped Cross Section to read both layers; remove a finish; cathedral deck ceiling by clearing Flat Ceiling Over This Room; Soffit defaults (60 x 12 x 36, floor to top 106 5/8) and a soffit ring; coffered soffits by Copy + Reflect.
Assertions: ceiling thickness equals the sum of layers; a cathedral room has its ceiling on the roof underside; soffits do not pass through intersecting walls. Breaks: R-122, R-146, CB-210, R-111.
Today: ignored (layered finish first).

## 11 tut11_finish_materials (pp. 191-211)
Steps: User Catalog folder "Beach Palette" and five colours copied into it; Perspective Floor Overview; Material Painter Plan Mode paints all interior walls; Material Eyedropper for the fire walls; Room Mode and Object Mode; Wall Covering "White Beadboard" 48 high; Wall Material Region defaults, a Wall Elevation, region 18" up and 12" down, Simplify, Reflect copy; flooring defaults (plan, floor 2 carpet, Bath kept); Object Eyedropper floor finish onto the kitchen; Floor Material Region 72 x 48 centred; Room Finish Schedule excluding Attic, Deck, Porch.
Assertions: no unpainted interior wall remains except the fire walls before the eyedropper step; kitchen floor layers equal the bath's; schedule row count. Breaks: CB-646, R-122, CAD-101, L-233.
Today: partial (ignore at layered floor finish and Simplify Polyline).

## 12 tut12_room_moldings (pp. 211-222)
Steps: Floor 1 Defaults Moldings: replace base with BM08, add CM02 crown at 3" with Retain Aspect Ratio; Dining room Use Default off + chair rail; closet crown deleted; a 1 1/2 x 11 1/4 rectangle added to the library as "2x12 Profile"; select the Exterior Room by Tab; Make Room Molding Polyline height 96 with the profile Outside Polyline; Remove / Add Molding to Selected Edge; moldings schedule with Extension Snap placement.
Assertions: dining room molding set differs from the floor default; the molding polyline has no molding on the removed edge. Breaks: R-117, R-106, R-110, S-170.
Today: ignored.

## 13 tut13_interior_furnishings (pp. 222-236)
Steps: Material Defaults for Furniture, Upholstery, Accent; place bed, end tables, dresser; sofa set; dining table and six chairs with Drawing Group Move Forward; vase and frame; resize the frame, set Finished Floor to Top 80, replace the image component with Mirror; Replace From Library on identical tables; storage box inside a table with Ctrl; Make Architectural Block of dresser + vase + frame, edit a component via Select Next Object, Explode, Add to Library; folders and Paste Link; Furniture Schedule with 3D Perspective + Dimensions; block "Treat as One Object" listed as "Dining Set"; Find Object in Plan.
Assertions: the bed's back faces the wall; replaced tables share one library id; the block moves as one and explodes to its parts; the schedule row count after the block. Breaks: CB-427, CB-436, S-118, S-195, L-234.
Today: ignored at the first block step.

## 14 tut14_cabinet_styles (pp. 237-260)
Steps: base cabinet; library door style and bar pull; Match Properties / Apply Properties to the wall and full height cabinets; countertop 1 1/4 and overhang 1 1/2, backsplash 4 + side; edge molding EM05; crown CM02 on a wall cabinet From Ceiling; light rail L-Bracket; materials with Default links; Set as Default; drawer base (Item Type changes, secondary drawer style from the library); Framed + Inset; Split Vertical with 10 / 22 / 10 widths and Double Door; feet.
Assertions: the three cabinets report the same door and handle style after Apply; Set as Default changes the next cabinet; face item widths sum to 48. Breaks: CB-12/CB-395, DS-27, CB-631, CB-632, CB-344, DS-26.
Today: partial (ignore at From Ceiling, Set as Default, the secondary style).

## 15 tut15_cabinet_layout (pp. 260-277)
Steps: bump and push; Cabinet Defaults Diagonal Door; four base cabinets with a corner; 3" resize steps (B24R -> B27R) and Esc; centre under the window; Wall Elevation, Copy + Sticky Mode + Reflect drawer base twice; fillers; island with rotate, copies, Uniform off, Back 0, backsplash 6 Always Present; breakfast bar (Side Panel - Applied, Suppress Label) and Generate Custom Countertop with 12" overhang; Make Architectural Block; wall cabinets and soffit above a full height cabinet; full height cabinet options and Cabinet Shelf Specification (5 shelves); label with Insert Macro.
Assertions: widths step by 3"; fillers close gaps; island block is one object; shelf count 5. Breaks: S-137, CB-634, CB-633, CB-427, TXT-65, CB-632.
Today: partial (ignore at Sticky Mode and shelves).

## 16 tut16_appliances_fixtures (pp. 277-298)
Steps: place refrigerator, wall cabinet above it (bottom meets the appliance top), partition with Point to Point Move, dishwasher between cabinets with countertop extension; explode block; range and hood; built-in microwave in a cabinet front with Move Up/Down; vanity sink (False Drawer question), faucet with +3/4" height typed, tub (Cut Room Moldings off), toilet, trim block; Reflect Geometry; Wall Elevation with Auto Elevation Dimensions and extension line edits; cross box; Fixture schedules by category and room; Find in Plan; label macros, Move Label, Suppress Label.
Assertions: cabinet bottom = appliance top; dishwasher countertop continuous; schedule rows for the kitchen only; label text. Breaks: CB-635, CB-636, L-233, S-118, TXT-65, DIM-63.
Today: ignored at the stacked wall cabinet.

## 17 tut17_light_fixtures (pp. 298-317)
Steps: General Electrical defaults (symbols); Electrical Plan View; Light tool in a kitchen, dining and deck; Electrical dimension defaults; position a can light by dimension with an extension line; Copy + Enter Coordinates 40"; Reflect; Transform/Replicate; Multiple Copy with intervals 50 / 52; wall sconces interior and exterior, resize 12 wide at 86; table and floor lamps; Create Schedule from Room; Cut/Paste into a CAD Detail; leader with `%description%`.
Assertions: spacing of the lights (40, 6' 6", 3' 3" copies); Multiple Copy array of 5 x 4; sconce width 12 and centre height 86; the leader shows "CRAFTSMAN LANTERN" style text. Breaks: E-30, E-31, E-33, S-132, S-138, R-105, TXT-65.
Today: partial (ignore at Enter Coordinates, Create Schedule from Room).

## 18 tut18_electrical_objects (pp. 317-331)
Steps: 110V outlets (interior, deck WP, floor), recess the WP outlet with Distance from Wall -4 and Cuts Wall; GFCI under a window at counter height 39; rotate in an elevation; counter outlet row by Enter Coordinates 72 and Transform/Replicate Y -48 x 3; library 220V receptacle and disposal outlet; switches; ganged block of three; Electrical Connection splines with automatic 3-way; data and security symbols; Electrical Schedule legend.
Assertions: WP outlet on the deck; recessed outlet flush; three switches become one ganged block; end switches of a chain become 3-way. Breaks: E-30, E-32, E-25, S-132.
Today: partial (ignore at recess and ganged block).

## 19 tut19_floor_framing (pp. 331-349)
Steps: Framing Type review; snap settings; Framing, Floor Plan View; Automatic Framing Defaults (16 OC, 1 1/2); Framing Reference Marker; copy layer set; Build Framing (floors and ceilings auto) on Floor 0; Joist Direction line, rotate, delete, undo; Bearing Wall on the wall left of the stair and rebuild Floor 1 only; Manual Framing Defaults (With Joists); Post with Footing x2; beam at joist level and a beam under joists (Raise/Lower -11 7/8); move a joist with bumping on and off; Trim Object(s) with Sticky Mode; bordered Rich Text.
Assertions: joist direction follows the line; with the Bearing Wall flag the Floor 1 joists run toward it; ceiling joists unchanged; beam tops differ by 11 7/8. Breaks: CB-261, CB-638, CB-283, S-196, CB-639, S-137.
Today: partial (ignore at Bearing Wall, Raise/Lower, Trim).

## 20 tut20_wall_framing (pp. 349-364)
Steps: Automatic Framing Defaults walls; Build Framing Once > Walls; edit corner studs (move, 3 copies at 1 1/2, width 3 1/2, P2P); wall type layer spacing 24 OC; regenerate all (corner replaced; undo); Build Framing for Selected Object(s) on six basement walls; Retain Wall Framing on two railings then rebuild; name a Wall Detail from the wall label and open it; Blocking in the detail; Wall Detail dimension defaults and Default Set; End to End, secondary format, extension lines, Running dimension.
Assertions: customised corner survives a rebuild only with Retain; basement spacing changes from 16 to 24 after the layer edit; the detail opens by name. Breaks: W-134, CB-256, CB-645, W-138, DIM-46, DS-32.
Today: ignored at the layer spacing step.

## 21 tut21_roof_ceiling_framing (pp. 364-381)
Steps: Framing, Roof Plan View floor 2; Build Roof structure (use framing reference, rafter spacing 16, structure 11 1/4, ridge 3 1/2 x 15 1/4); Build Framing Once > Roof (rafters keep the plane's older depth 9 1/4); Edit All Roof Planes spacing 16 / ridge 3 1/2; single plane 24 OC and 11 1/4 structure, rebuild for selected; post under the ridge, Add Break on the ridge board; stacked posts with Paste Hold Position and Raise/Lower / Lock Total Height; ceiling Bearing Line and Joist Direction lines; Build Ceilings Floor 2; Reference Display with rafters; shift joists by -3/4 with Transform/Replicate; label valley rafters with `%nominal_size%` and a Fill.
Assertions: rafter depth follows the plane spec, not the defaults; ridge broken into two members; ceiling joists offset 3/4. Breaks: CB-256, CB-643, CB-283, CB-644, CB-638.
Today: ignored at the single-plane rebuild.

## 22 tut22_plot_plans (pp. 382-403)
Steps: new plan; Plot Plan View; CAD Defaults: feet, 2 decimals, quadrant bearing; Input Point (0,0); Input Line with Polar bearings N 61 25 10 E 155.69', S 28 29 35 E 118.65', N 49 59 11 E 154.37'; Disconnect Edges to fix an edge; reverse a line with S 49 59 11 W; arc closing edge with Lock Chord radius 450'; Show Length / Angle; Concentric Jump 10' setback copy; layer "CAD, Setback Lines"; paste into the house plan; Convert Polyline to Terrain Perimeter; North Pointer; rotate the lot; Transform/Replicate rotate by "1 30 25"; position by End to End dimension 50' and a point marker 30'.
Assertions: the traverse closes within the arc; bearing labels read back N 61 25 10 E; the setback polyline is 10' inside; rotating by 1 30 25 changes the back line to horizontal within 1e-6. Breaks: CAD-109, CAD-134, S-172, CAD-22, CAD-118, PR-31.
Today: ignored (the first Input Line with a bearing).

## 23 tut23_terrain_elevation (pp. 403-420)
Steps: Additional Allowed Angle from the bearing; Duplicate the Plot Plan View as "Landscaping Plan View" with a copied layer set; Terrain Specification Absolute Elevation -28 with the Reference Point at the garage door; Elevation Lines at 0, -8', 6', 5' with copies and typed distances; rotate two lines 15 degrees, resize with Ctrl, lock start and -13 degrees; Make Parallel to the chord; Change Line/Arc curved elevation line radius 450'; Elevation Polyline through the Same Line Type handle; retaining walls with 8" spacing to elevation data.
Assertions: slope between two lines equals the elevation difference over the distance; the reference point elevation is -28; lines at different elevations never cross (warning). Breaks: CB-530, CB-531, CB-647, CAD-121, CAD-22, CB-512, PR-31.
Today: partial (ignore at reference point and curved lines).

## 24 tut24_driveways_roads (pp. 420-433)
Steps: Road defaults 28' with curbs that cut driveways; Straight Road across the lot, curved with Change Line/Arc, P2P Move, ends to corners; Road Stripe 6' from the edge; Paste Hold Position of the Terrain Perimeter as a CAD polyline and resize the real one to the road; road Height 1"; Driveway defaults 13' with a 3' end flare and Blacktop; draw from the garage midpoint; Sidewalk 60"; Auto Generate Sidewalks offset 60"; curved sidewalk at 315.
Assertions: the driveway meets the road; curb is cut at the driveway; flare width 3' at the end. Breaks: CB-628, CB-629, CB-630, CB-588, CAD-22.
Today: ignored at curb cuts.

## 25 tut25_landscaping (pp. 433-450)
Steps: lock road layers, hide elevation data; Dirt material default; Polyline Garden Bed with Add Break and Change Line/Arc; Paste Hold Position of the lot polyline, Convert Polyline to Garden Bed, Make Parallel edges to the setbacks; Fencing defaults (72, posts 5 1/2 x 74, Pole panel, top and bottom rails); fences along setbacks (a fence replaces a retaining wall at the same place: undo); gate in the fence (height 72, floor to bottom 4); Plant Chooser "fir"; one click inside a bed distributes copies; Reverse Image; three shrubs as an Architectural Block, copies, Explode.
Assertions: the bed edges are parallel to the setbacks within 1e-6; gate sits 4" above the ground; block copies share geometry. Breaks: CAD-22, S-170, CB-649, CB-427, CB-617.
Today: partial (ignore at Change Line/Arc and block).

## 26 tut26_layout_page_templates (pp. 451-460)
Steps: open the layout; Page 0 Title "Standard Template", page 1 "Cover Sheet Template" with Use as Page Template; pages 2-6: assign templates and titles from one Layout Page Information dialog; Labels A0.#, A1.#, A1.#, E1.#; Insert Page Before page 5, Insert Page After 6; labels and titles for the inserted pages; copy the layout.
Assertions: the printed sheet numbers read A0.1, A1.1, A1.2, E1.1, A1.3...; a page keeps its assigned template; template pages are absent from the print range. Breaks: L-188, L-190, L-191, L-192.
Today: ignored at the first Label.

## 27 tut27_title_blocks_borders (pp. 460-483)
Steps: Drawing Sheet Setup (size, 1 in = 1 in, margins); General Layout Defaults grid snap 1/8"; delete the stock border by marquee; draw a rectangle border with Concentric Jump 1/8; title block frame by concentric copy 1/4 and a 2" Selected Edge dimension; component frames by Copy + P2P Move + nudge; Rich Text defaults; company text 3/16 centred in a frame with Center Object; `%layout.title%`, `%layout.label%`, short date macros; Layout Revision Table with 7 rows; page revision for pages 3,4; copy borders to page 1 with Paste Hold Position; import a logo (Save in Plan, 3 x 3, Suppress Label); Layout Page Table "Sheet Index" with a renamed Label column.
Assertions: border insets by 1/8 and 1/4 steps; the title text shows the page title; the revision appears only on pages 3 and 4; the sheet index lists labels. Breaks: L-230, L-231, L-189, L-193, L-195, L-236.
Today: ignored at the grid unit.

## 28 tut28_send_to_layout (pp. 483-500)
Steps: Drawing Sheet Setup default scale 1/4" = 1'; send Floor Plan View Dimensioned to page 6 with Link Saved Plan View, Electrical Plan View to page 9, Plot Plan View to page 4 at 1 ft = 100 ft (assert the too-large warning); rotate 90, crop, centre and move the label; send the Stair Section to page 8 at 1/2" = 1'; send an exterior elevation to page 7 as Plot Lines with Color Fill, switch to Live View, crop and move; customise the callout label with page link, arrows; send a Full Camera as Current Screen as Image to page 2, resize and crop; print to PDF on ARCH D.
Assertions: each box lands on its page with the requested scale; the site plan scale is exactly 1:1200 paper-to-model; the PDF has one page per content page at 24 x 36; the callout reads the linked page label. Breaks: L-229, L-155, L-156, L-161, L-232.
Today: partial (ignore at 1 ft = 100 ft, Plot Lines, callout).

---

## Running the whole set

`cargo test -p plan-app scenarios::tut` replays the tutorials; each ignored test names its break id, so `cargo test -p plan-app scenarios::tut -- --ignored` lists the open work in tutorial order. A final assertion in `tut_support.rs` could count the ignored tests and compare it with `docs/parity-status.md` so a feature that lands without its `#[ignore]` being removed fails the gate.

---

## Status, lessons 1 to 14 (Round 16, brief 34)

Replays live in `crates/plan-app/src/scenarios/tutorials_a/` with the shared `chic_cottage` helper in `scenarios/tutorials_support.rs`. `cargo test -p plan-app --bin plan-studio scenarios::tutorials` is green (19 pass, 61 ignored); `-- --ignored` lists the open work in lesson order. Ignore tags read `T7-<lesson>: <break id>`.

| Lesson | Status | Green now | Ignored (break ids) |
|---|---|---|---|
| 1 | partial | six walls close into one room, one undo step per wall; Floor Defaults ceiling 97 1/8; Auto Exterior Dimensions undo | wall-type-layers, R-143, RF-164, DIM-48, TXT-65, APP-93 |
| 2 | partial | three partitions make four rooms; delete a partition merges; Wall Specification opens | W-152, R-107, wall-type-layers |
| 3 | partial | Build New Floor derives the shell in one step; floor elevation | R-133, W-138, W-145, L-235 |
| 4 | partial | Draw Stairs between floors: rise equals floor delta, 14-18 risers | CB-160, CB-162, DIM-62, C-136, TXT-56 |
| 5 | partial | door and window clicks, interior door in a partition | CB-395, DW-168, TXT-65 |
| 6 | ignored | cottage stays one room | CB-627, CB-530, W-145, CB-109, CB-170, L-63 (half-wall and railing flyout steps not driven) |
| 7 | partial | hip roof, four planes, one step | RF-166 (all other styles), RF-165 |
| 8 | partial | Build Roof on the cottage, undo | RF-166, RF-61, RF-86, RF-96, DS-27 |
| 9 | partial | Floating Dormer, one step | RF-166, RF-80, S-137 |
| 10 | ignored | none | R-122, R-146, CB-210, R-111 |
| 11 | partial | room floor finish name, one step | R-122, CB-646, CAD-101, L-233 |
| 12 | ignored | none | R-117, R-106, R-110, S-170 |
| 13 | ignored | none | CB-427, CB-436, S-118, S-195, L-234 |
| 14 | partial | base, wall, full-height cabinets, one step each | CB-395, DS-27, CB-631, CB-632, CB-344, DS-26 |

---

## Status of lessons 15 to 28 (round 16, brief 35)

Implemented in `crates/plan-app/src/scenarios/tutorials_b/` (one file per lesson, `support_b.rs` for the framing/terrain/layout helpers). `cargo test -p plan-app --bin plan-studio scenarios::tutorials_b` runs the green replays; add `-- --ignored` for the open work in lesson order (each ignore carries `T7-<lesson>: <id>`).

| Lesson | Real replay (green) | Still ignored |
|---|---|---|
| 15 | butting row of base cabinets, filler, wall cabinet | S-137, CB-427, CB-634, CB-633, R-113 |
| 16 | partition and wall cabinet, one undo step each | CB-635, CB-434, CB-636, L-233, S-118, DIM-63 |
| 17 | lights on a 40 in pitch, wall light on its wall | E-33, R-105, TXT-65, E-31 |
| 18 | outlet family, three switches | E-32, E-25, CB-428 |
| 19 | floor 2 framing, one step, floor 1 untouched | CB-283, S-137, CB-638 |
| 20 | wall framing studs, one step | W-138, CB-645, DIM-46, DS-32 |
| 21 | roof rafters from Build Framing | CB-283, RF-115, CB-644 |
| 22 | survey courses of the guide chain at the bearings | S-172, CAD-22, PR-31 |
| 23 | two elevation lines, slope | CB-530, CAD-22, CB-512 |
| 24 | road, driveway, sidewalk strips | CB-628, CB-629, CB-630, CB-588 |
| 25 | polyline garden bed | CAD-22, S-170, CB-649, CB-427, CB-617 |
| 26 | patterned labels (A0.1, A1.1, A1.2, E1.1), template pages not printed, insert page | L-190 (template boxes repeat), L-192 |
| 27 | page on its own sheet and back | L-230, L-231, L-189, L-195, L-236, L-193 |
| 28 | site plan at 1:1200, PDF one ARCH D page per content page | L-229, L-155, L-232, L-161 |
