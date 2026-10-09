# Parity spec: Documentation and Layout (Chief Architect X18)

> Status (2026-10-08, re-audited against HEAD 4c11b69): 47 ids: 22 Works, 21 Partial, 3 Missing, 1 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.
>
> Since that count (Round 13): L-10 Save As Template and Apply Template for layouts (JSON under `~/.plan-studio/templates`, a default per sheet size for new layouts); L-9/L-11 revision clouds add rows to the REVISIONS table, and the sheet index follows page reorders without shifting the numbering; plan boxes print the opening labels; leaders can have bends; schedules and the Materials List export to Excel (.xlsx); perspective boxes and Print Model are lit by the plan's point lights; Print Preview shows the Print dialog's colour mode. Still open: the OS print panel (out of scope) and a separate editor for page templates per sheet type (the saved layout templates and the Page Template page flag cover it).
>
> Round 14 (layout and print): a plan can hold several layout files (`Project::layout_files`; New Layout File, the file picker and a Layout file choice in Send to Layout); Page Specification per page (title, sheet number, Page Template flag, its own sheet size and orientation, no title block; the PDF page, title block and packing follow it); Customize Sheet Sizes (custom named sizes, hidden standard sizes, Daniel's ARCH preset; custom sizes in the Print paper list); Shift-click multi-select with Align, Spread, Copy to Page, Duplicate and Open Source View for layout boxes; a Print Preview window drawn from the print's own primitives (colour mode, pen weights or hairlines, scale, tiles, pictures); hatches made for the box scale (camera drawings re-hatched); layout elevations, sections and perspective boxes drawn from the 3D view's scene (roofs, stairs, cabinets, terrain, casing); a picture of the open 3D view can be sent to a page (ray traced, embedded); schedule boxes follow the plan's Schedule Specification (Show All and Reset Columns in the dialog); placed schedules are tables in the floor DXF and boxes in the construction set; Export CSV / Excel for the tables on a layout page and Excel for the framing takeoff; wall-type layers are quantities in the Materials List; a spiral stair is "Spiral" in the Stair Schedule; the Elevation DXF uses the 3D view's scene. Integration pass 2: cabinets (with their Fill Style and merged countertops), placed symbols, stairs and the dashed treads seen through a stairwell are drawn in plan boxes, Print Preview, the PDF and the floor DXF (`editor/plan_overlay.rs`, `LayoutRenderContext::with_plan_overlay`; scenario s58); File > New Layout makes a second layout file when the plan has one, and the Project Browser lists the parked files; a group drag moves the page drawings selected with it (Shift-click); text boxes draw wrapped and framed in CAD-detail boxes and in the DXF. Still open: a GL read-back snapshot, floor, ceiling and roof component quantities.

> Round 14 (details and pictures): Auto Detail (L-39), CAD Detail From View (L-40) and CAD Detail Management (L-41) are built: a CAD detail is a floor marked `Floor.detail` (`plan_core::details::CadDetailInfo`, appended after the last floor and listed under CAD Details), Auto Detail turns a section or elevation into CAD (cut lines, hatch per material pattern, the wall assembly from the wall type layers, framing members from plan-framing or assumed plates, insulation batts in the cavity, notes), the name follows the camera's name and callout number until renamed, and Send to Layout puts a detail box on a page at the detail's scale; Detail Components (2x framing sections, insulation, flashing, sheathing, siding profiles, anchor bolts) place as CAD blocks. L-46: Import Picture and its Image Specification, PDFs whose pages are pictures, Point to Point Resize and Rotate to Align. Manual chapter 20; tests in `tools/details/`, `tools/underlay/` and scenario `s38_details_r14`.

Scope: Layout (Send to Layout, layout boxes, line weights, sheet sizes, title
blocks, pages, plot plans), Printing and PDF, Schedules, Materials List, Plan
Footprint, Auto Detail, CAD Detail from View, DXF/DWG import and export.

Sources: Chief X18 Reference Manual (Layout, Printing, Schedules, Materials
List, Import/Export chapters) from memory and the captured menu/hotkey docs
(`chief-x18-menus.md`, `chief-x18-subtools.md`). Lines marked "(verify in
Chief)" need checking in the application. Lengths in inches.

Code read: `crates/plan-docs/src/{lib,schedule,materials}.rs`,
`plan-docs/src/pdf/{mod,sheet}.rs`, `plan-core/src/export/dxf.rs`,
`plan-core/src/layers.rs` (pen weights), `plan-core/src/dimension.rs`.
`plan-elevation` and `plan-import` are 1-line placeholder crates.

## 1. Send to Layout and layout boxes

L-1. File > Send to Layout (hotkey `S, L`; toolbar button Send to Layout) sends
the **active view** to a layout file. The view can be a floor plan, elevation,
cross section, perspective or orthographic camera view, framing view, schedule,
CAD detail, or Materials List.

L-2. The Send to Layout dialog asks: which Layout file (new or an open one), which
page (new page or existing), the **Scale** (plan views; 3D views use a "fit"),
and whether to keep the view linked. For a new layout it can apply a layout
template (title block, borders). (verify field names in Chief)

L-3. The result is a **layout box** on the page: a rectangular viewport on the
sheet showing the view at the chosen scale. Layout boxes are **linked** to the
plan/view and refresh when the plan changes (automatic or via Update Layout Box;
verify mode names in Chief).

L-4. Layout box properties (Layout Box Specification): Name, Source view (saved
view / camera), Scale (e.g. 1/4" = 1'-0", 1:50), Rotation, Clip/crop to the box
frame, Display border (line weight), Background (transparent or white), and
Display options per box (which layers, whether dimensions show). Scale changes
resize the content within the box, not the box.

L-5. Plan views in a layout box are drawn as **vectors**; Vector View elevations
and cross sections are vectors; shaded 3D views (Standard, Physically Based) are
**raster images** at a chosen resolution. Raster boxes can be re-rendered at
print resolution.

L-6. Layout boxes can be moved, resized, copied between pages, aligned, and
double-clicked to open their dialog; "Open Source View" jumps back to the plan.
Edits to text/dimensions in the plan are not made in the box.

L-7. Layout pages: a layout file has many pages; each page has a **Page
Specification**: size, orientation, name (e.g. "A-1.0 Floor Plan"), number, a
page border, and page-level text. Pages are listed in the Project Browser.

L-8. **Sheet sizes**: ANSI A (8.5x11), B (11x17), C (17x22), D (22x34), E
(34x44); Architectural A (9x12), B (12x18), C (18x24), D (24x36), E (36x48);
ISO A4/A3/A2/A1/A0. Custom sizes via File > Customize Sheet Sizes. Orientation
landscape or portrait. (verify the full list in Chief)

L-9. **Title blocks** are drawn on the page (CAD in layout) or placed from the
library/template. Fields use **text macros**: project name, client, designer,
date, sheet number, sheet title, scale of a named box, revision. Macros update
across all pages.

L-10. **Layout templates** (File > Templates > New Layout From Template, Save As
Template) store sheet size, title block, borders and standard boxes.

L-11. **Sheet index / drawing list**: a text macro or object lists all pages'
numbers and titles (Page Index) and updates when pages are renamed or added.

L-12. **Line weights**: each layer has a pen weight (Layer Display Options), each
object may override. The `Line Weights` toggle shows weights on screen; in layout
boxes the weights print according to the plotted scale: pen weights are in
points/mm of **paper**, not scaled with the drawing (a 0.50 mm wall line stays
0.50 mm at any scale). Layout boxes have a "Line Weight Scaling" multiplier.
(verify multiplier name in Chief)

L-13. Line styles/dash patterns and fill styles (solid, hatch, patterns)
scale to **paper** size as well; fill patterns can be set to follow the plan
scale (pattern scale option) so brick hatch spacing stays correct.

L-14. **Plot plan / site plan**: a layout box of the plan view with Terrain
visible and a site layer set; usually at 1" = 10' or 1" = 20', often with north
arrow, scale bar and property lines (Terrain Perimeter, CAD). North pointer is a
CAD > North Pointer object tied to the plan's north angle. (verify scale bar
object in Chief)

L-15. Dimensions in layout boxes scale with the box (text height is in paper
points, set in Dimension Defaults); text in the plan scales to stay legible via
Text Defaults "Scale text with plan scale".

L-16. Layout boxes inside a layout can be **overlaid with layout CAD** (lines,
text, leaders) that is not part of the plan; layout text uses its own layer set.

L-17. When a source plan is renamed, linked boxes keep their link by internal id.

## 2. Printing and PDF

L-18. File > Print > Print... prints the active view or layout page. Print
setup: printer, paper size, orientation, **Print Scale** (fit to page, or a chosen
architectural scale), margins, and **Tiling** (print a large drawing over several
sheets with overlap marks).

L-19. **Print Preview** (right-edge toggle) shows page breaks and the **Drawing
Sheet** frame (right-edge toggle); the preview is WYSIWYG including line weights
and colour/B&W mode. Black-and-white and grayscale modes are options.

L-20. **Print to PDF / Export PDF**: from a layout file, exports all or selected
pages to one PDF; plan views as vector, shaded 3D as raster at DPI setting;
bookmarks per page name; embedded fonts. A single plan view can also be exported
(File > Export > PDF) at its set scale.

L-21. Printed scale accuracy: a line of known length on the plan measures to
scale on the printed sheet within printer tolerances; the scale note
`1/4" = 1'-0"` is printed from the box scale macro.

L-22. Drawing scales supported: architectural (1/16" to 3" = 1'-0"), engineering
(1" = 10' ... 1" = 100') and metric (1:1 to 1:500) selected by unit setting;
custom ratio. (verify exact list in Chief)

## 3. Schedules

L-23. Tools > Schedules lists: Cabinet, Door, Window, Electrical, Fixture,
Framing, Furniture, Note, Plant, Room Finish, Wall, Custom Schedule, and Manage
Custom Schedules.

L-24. A schedule is a **live table** built from plan objects of that category,
across floors (floor filter option). It opens in its own window and can be sent
to Layout as a box.

L-25. **Columns**: each schedule has a column definition set (Edit Columns /
Column Order): e.g. Door: Mark/Label, Type, Width, Height, Thickness, Style, Hardware,
Notes, Count. Columns are added, removed and re-ordered, and each can be shown or
hidden; the format of numbers follows Dimension defaults.

L-26. **Callouts**: doors and windows display a **callout marker** in plan (a
letter or number in a circle/hexagon) set in the Label tab that references the
schedule row (Mark). Cabinets, fixtures and wall types have labels by macro.
Changing the mark in the schedule or object renumbers the callout.

L-27. **Edit from schedule**: clicking a row selects the object in the plan;
double-clicking opens that object's dialog (or edit cells in place for
editable columns such as Mark, Notes); changes update the plan and the schedule.

L-28. Sorting, grouping (by type/size), totals and counts: rows can be grouped
(same size/type counted together) or listed individually; Totals row optional.

L-29. Schedule data come from **Object Information / Schedule tab** of each
object: Schedule include flag, manufacturer, model, cost, notes (the Schedule
tab on object dialogs).

L-30. **Room Finish Schedule**: rows are rooms with Name, Number, Floor Finish,
Wall Finish, Ceiling Finish, Base, Area, Ceiling Height, using materials from
the Room Specification. Area uses Interior Area (rooms-floors.md R-49).

L-31. **Custom Schedules** let the user define a schedule from any object type
with chosen fields and filters (Manage Custom Schedules).

L-32. Schedules export as CSV/Excel (Save As > text/CSV) and print/PDF.

## 4. Materials List

L-33. Tools > Materials List > Create Materials List: a take-off of the whole
plan, floor, or selected objects by **category**: Framing, Sheathing, Insulation,
Drywall, Roofing, Siding, Doors, Windows, Cabinets, Fixtures, etc.

L-34. Columns: Item/description, Quantity, Unit, Unit price, Extended price,
Waste factor, Supplier/Manufacturer, Category. Categories define the sort.

L-35. Quantities come from component layers (Components tab on walls, floors,
roofs, rooms), so a 2x6 wall with sheathing and siding contributes to each
material: studs by count/length, sheathing by area/4x8 sheets (32 sq ft),
siding by area, and so on.

L-36. **Waste** percentages per category; lengths round up to stock lengths
(lumber 8', 10', 12', 14', 16').

L-37. Materials List can be sent to Layout and exported CSV/PDF/XLS.

## 5. Plan Footprint, Auto Detail, CAD Detail from View

L-38. **Plan Footprint** (CAD > Plan Footprint): creates a closed CAD polyline
of the building's exterior outline (outer faces of exterior walls) at the current
floor, on the CAD layer, for use in site plans, roof outlines and terrain.

L-39. **Auto Detail** (CAD > Auto Detail, toolbar button): after you click a
location on a wall, wall section or framing, Chief generates a **detail**
(cross section through the wall assembly) as CAD with the layers drawn to scale
and labeled. Details are saved in the CAD Detail Management list. (verify in
Chief; Auto Detail is disabled with no selection)

L-40. **CAD Detail from View** (CAD > CAD Detail From View): captures the
**current view** (a cross section, elevation or plan region) as a CAD detail,
converting it to 2D CAD lines/fills that can be edited and reused. The detail
is stored in the plan's detail library.

L-41. **CAD Detail Management** lists details; a detail can be inserted into
plans/layouts as an object, exported to a .dtl/library, or placed in a layout
box.

L-42. **CAD to Walls**: converts selected CAD lines/polylines to walls
(File/CAD menu), used when tracing an imported DXF.

## 6. DXF / DWG import and export

L-43. File > Import > DXF/DWG (and CAD to Walls): options for **units**
(inches, feet, mm...), target (current plan CAD, new CAD detail, new plan),
layer mapping (keep layers, map to Chief layers), explode blocks, and scale to
fit. Result is CAD objects on layers; 3D solids import as 3D solids; text maps to
Chief text.

L-44. File > Export > DXF/DWG exports the active view or layout page. Options:
DWG/DXF **version** (R12 to 2018), units, layers (as Chief layers or merged),
text style conversion, line weights, scale, "Export 3D as solids" for 3D views,
region (current view, selection). Export writes entities as polylines, lines,
arcs, circles, text, hatches and (optionally) blocks for symbols.

L-45. Walls export as closed polylines on layers named after the Chief layers;
dimensions export as dimension entities or exploded lines (option); symbols
export as blocks or 2D geometry (option).

L-46. PDF as underlay: import a PDF or image as a scaled underlay for tracing
(calibrate by two points); not exporting.

L-47. Other exports: SketchUp, IFC, Image (JPEG/PNG), Animation; out of scope
for this spec.

## 7. Plan Studio today

Output works as a library with menu commands: `plan_docs::plan_sheet` and `plan-layout` write scaled sheets and the construction set PDF (Tools > Schedules > Create Construction Set, File > Export > Construction Set PDF), with title blocks, plan, elevation, section and schedule boxes; door, window, room and wall schedules and the Materials List export CSV, and the Framing Takeoff too. `plan-elevation` draws hidden-line elevations and sections, exported as DXF (File > Export > Elevation DXF). DXF export of the active floor (File > Export > DXF), DXF import with a units choice (File > Import > Import Drawing) and CAD to Walls are in the editor, as is Plan Footprint. The Drawing Sheet and Print Preview toggles outline and gray out the active layout's sheet size and scale on the plan. Still open: an interactive Layout (page tabs, box editing, Send to Layout, Open Layout, layouts stored in the plan), real Print, PDF of the active plan view from the menu, placing schedules on the plan, cabinet, electrical, fixture and room-finish schedules, Auto Detail, CAD Detail from View, DWG, and layer pen weights in the PDF. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## 8. Gap table

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| L-1..L-3 Send to Layout with linked layout boxes | Single `plan_sheet` call producing a one-page PDF; no layout model | Critical | New `Layout { pages: Vec<Page> }` and `LayoutBox { source_view, scale, rect, rotation, clip }` in plan-docs; Send to Layout command; refresh on plan change |
| L-4 Layout box dialog (scale, clip, rotate, border) | Scale enum only | High | Box struct + dialog on shared frame |
| L-5 Vector vs raster boxes | Vector only | Med | Render raster boxes from `plan-view3d` offscreen at DPI; embed as image XObject in PDF writer |
| L-7, L-8 Pages with names/numbers; full sheet size list incl. ANSI/Arch/ISO | 4 sizes (ArchD, ArchC, Letter, Tabloid), landscape only | High | Extend `SheetSize` to ANSI A-E, Arch A-E, ISO, custom, portrait |
| L-9, L-10, L-11 Title block macros, layout templates, sheet index | Fixed title block with 5 text fields | High | Macro engine (`<project>`, `<sheet number>`...); template JSON; index macro |
| L-12 Pen weights per layer, printed in paper units | Constant widths in `sheet.rs`; `Layer.line_weight` unused in PDF | Critical | Map `Layer.line_weight` (1/100 mm) to PDF points per object layer; global multiplier |
| L-13 Fill/pattern scaling | Solid gray fills only | Med | Hatch patterns library; scale option |
| L-14 Plot/site plan layout with north arrow, scale bar, terrain | Missing; no terrain | High | After terrain (CB-43); add north pointer and scale bar CAD objects |
| L-15 Dimension/text height in paper units, scaling with box | Dimensions not printed | Critical | Render dimensions and text in `plan_sheet` using `Dimension` model and paper-point heights |
| L-16 Layout CAD (text, lines) on pages | Missing | Med | Reuse `CadObject` with layout layer |
| L-18, L-19 Print dialog, Print Preview, Drawing Sheet, tiling | None (PDF only) | High | Print via `printpdf` or OS print of generated PDF; tiling splitter; preview pane |
| L-20 Multi-page PDF, bookmarks, embedded fonts, raster at DPI | Single page, Helvetica, no bookmarks | High | Multi-page `PdfDoc` exists (`new_page`); add outlines, font embedding via `ttf-parser` |
| L-22 Architectural/engineering/metric scale lists | 4 architectural scales | Med | Extend `Scale` to 1/16"..3", engineering, metric ratios; custom ratio |
| L-23 Schedule set: Cabinet, Electrical, Fixture, Framing, Furniture, Note, Plant, Room Finish, Custom | Only door, window, room, wall | High | Add schedule generators as their object families land; Custom Schedule definition struct |
| L-24, L-32 Live schedule windows; send to Layout | Static generation, CSV/Markdown | Med | Live schedule view in app reading `Schedule`; layout box kind `Schedule` |
| L-25 Column editor | Fixed columns | Med | `ColumnSpec` list per schedule type; edit dialog |
| L-26 Callouts linking plan markers to schedule marks | None; numbering by reading order | High | `mark` field on openings; callout symbol drawn from mark; renumber command |
| L-27 Edit from schedule (select/double-click) | Read-only text | Med | Row-to-id map in `Schedule`; app selects/opens object |
| L-28 Grouping, totals | Row per object | Low | Grouping option by size/type with counts |
| L-29 Schedule tab data on objects (manufacturer, model, cost, notes) | Not on model | Med | `ObjectInfo` struct on walls/openings/etc. |
| L-30 Room Finish Schedule from Room Specification | `room_schedule` has Number, Name, Area, Perimeter, Ceiling | High | Needs RoomSpec finishes (rooms-floors.md R-36); add columns Floor/Wall/Ceiling Finish, Base |
| L-33..L-37 Materials List with categories, waste, price, stock lengths, roofing/framing/cabinets | Simple take-off CSV | High | Extend `MaterialLine` (category, waste, unit price, stock length rounding); sources from Components tabs once defined |
| L-38 Plan Footprint | Done (round 15): `cad_ops::plan_footprint` unions the wall outlines (`plan_core::clip`), so the polyline runs along the outer faces of the exterior walls (interior walls are inside it), one polyline per detached building, the area written under it; used by Tools > Checks / CAD > Plan Footprint (`build_tools.rs`), the rooms' footprint stays the fallback when a floor has no walls. Test: `s51_cad_r15::plan_footprint_follows_the_outer_wall_faces_not_the_rooms`, `cad_ops::tests::plan_footprint_runs_along_the_outer_wall_faces`. | Done | Plan Footprint Specification (CAD-135) still open |
| L-39 Auto Detail | Round 14: done from a section or elevation (`tools/details/cad_detail.rs`; wall assembly from the wall type layers, framing members, insulation, hatch, notes; manual chapter 20); verify in Chief | Med | Starting from a click on a wall (Chief) is not built |
| L-40, L-41 CAD Detail from View, CAD Detail Management | Round 14: done (details are floors marked `Floor.detail`; `dialogs/details/management.rs`; Send to Layout as a `CadDetail` box) | Med | No .dtl export |
| L-42 CAD to Walls | Missing | Med | Convert selected lines/polylines to walls with default type |
| L-43 DXF/DWG import with units/layers/blocks | plan-import empty | High | DXF ASCII parser (`dxf` crate) to `CadItem`, units dialog, layer map; DWG via external converter (ODA licensing) deferred |
| L-44, L-45 DXF/DWG export options (version, units, blocks, 3D) | Done in part (round 15): File > Export > DXF opens the options window (`dialogs/dxf_options.rs`, writer `plan-core/src/export/dxf_options.rs`): units (in, ft, mm, cm, m, `$INSUNITS`), layer names (plan names or AIA, plus a custom map), line weights in the LAYER table (group 370), text as TEXT or as stroke lines, this floor / all floors / a pick with the floor name on each layer, 2D or 3D (floors at their elevation + the model as 3DFACE by material). Tests: `dxf_options::tests::*`, `dialogs::dxf_options::tests::*`, `s51_cad_r15::the_dxf_export_options_window_decides_units_names_text_and_floors`. | Med | Open: R12 ASCII only (AutoCAD version choice, DWG), selection export, symbols as blocks, dimensions as entities, hatches |
| L-46 PDF/image underlay with calibration | Round 14: PNG/JPEG underlays, PDFs whose pages are pictures (own object-table reader), Point to Point Resize and Rotate to Align (`tools/underlay/{pdf,inflate,trace}.rs`) | High | Vector PDF pages need a rasterizer: export the page as PNG |
| Layout pages in Project Browser, Window menu tabs | None | Med | Project browser panel listing views, layouts, pages |

<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| L-48 | Export ▸ glTF 3D model: Export the 3D model for viewers (Chief exports 3D Viewer, OBJ, 3DS, COLLADA, STL, SKP). (Not captured; verify in Chief.) | Works | menus.rs "glTF…"; plan-view3d export.rs |
| L-49 | Export ▸ Picture / 3D view image (PNG, JPEG, BMP, TIFF): One Export Picture command that saves the active view (plan, 3D, layout page) as a picture file with chosen size and format (PNG, JPEG, TIFF, BMP). Also covers: Export picture (JPEG, PNG, TIFF) of a view. | Partial | File > Export > Picture (dialogs/export_picture.rs, encode.rs): plan (floor + layer set), exterior elevation, elevation/section camera drawing, live 3D view (ray traced from the viewport camera); pixel size or paper size + DPI; PNG, JPEG (own baseline encoder), BMP, TIFF; transparent background for line views in PNG/TIFF; tests export_picture::tests, encode::tests, scenarios s43 (export_picture_saves_the_plan_in_each_format_and_size, export_picture_covers_elevations_and_refuses_an_empty_view). A layout page is not exported as a picture (Print > PDF does it); the 3D picture is ray traced, not a GL read-back. Verify in Chief: dialog wording and option set |
| L-50 | Import ▸ Chief Plan (.plan): Open a Chief Architect plan (read-only decode of Daniel's install files, Round 13/14 stages). (Not captured; verify in Chief.) | Works | menus.rs "Chief Plan…"; plan-chiefplan crate; main.rs import_chief_plan. Stage 3 (round 14): catalog GUID of placed objects resolved through `chief_link::resolve_symbol`, roof eave-first outlines with edge flags and overhang, stair stacking heights, gang boxes and connection arcs, guessed corner cabinets (`symbols::reads_the_item_guid_at_the_anchor_and_other_candidates`, `roofs::the_eave_edge_comes_first_with_flags_and_overhang`, `stairs::stacked_flights_carry_their_heights_and_base`, `tests::stage3_*`, `cabinets::a_square_box_in_a_wall_corner_is_a_corner_cabinet`; ignored real-file tests in `tests/real_stage3.rs`). Circuits, cabinet labels, blind cabinets, rail sides not found in the file; corner cabinets are a guess: verify in Chief |
| L-51 | Import ▸ Import Project / Merge Plan (not captured): Import another plan's objects, layers, library items into this plan. (Not captured; verify in Chief.) | Missing | no way to merge another plan file into the open plan |
| L-52 | Project Information…: Client, project, designer and revision data used in title blocks and macros. | Works | dialogs/project_info.rs tabs Client, Project, Designer, Revisions, Custom Fields; feeds title block macros (L-9) |
| L-53 | Watermark: Overlay a DRAFT / NOT FOR CONSTRUCTION watermark on screen and prints. | Missing | no watermark option on screen or in print |
| L-54 | Revision Table / Revision Cloud dialog (Project Information > Revisions): Revision list that feeds a revision table on the sheet. (Not captured; verify in Chief.) | Works | dialogs/project_info.rs "Revisions" tab; plan-layout revision table |
| L-55 | Export OBJ / STL / 3DS / COLLADA 3D models: Export the 3D model for 3D printing and other programs. (Not captured; verify in Chief.) | Missing | only glTF is exported (plan-view3d export.rs) |
| L-56 | Export Materials List to HTML: Save the materials list or a schedule as an HTML page. (Not captured; verify in Chief.) | Missing | Materials List exports CSV and XLSX (plan-docs materials.rs, xlsx.rs); no HTML table (grep html in plan-docs finds nothing) |
<!-- coverage-audit:end -->

## Beyond Chief: custom properties and the Excel round trip

Not in Chief X18 (Chief has only the fixed Object Information fields). Modelled on ArchiCAD's Property Manager and "Exchange Property Data with Excel". Manual: `docs/manual/11-layout-schedules-print.md`, "Excel round trip".

| ID | Behavior | Status | Evidence |
|---|---|---|---|
| BC-1 | Tools > Property Manager: user-defined properties per kind of object (door, window, cabinet, room, wall, fixture / symbol, electrical, stair, roof plane, framing) with name, type (text, number, length, yes / no, list), default and "show in schedule"; add, change and delete are undo steps. | Works | `plan-core/src/props.rs` (`PropTable`, tests); `dialogs/property_manager.rs` (`Manager`, `add_property`, `update_property`, `delete_property`; test `definitions_add_change_and_delete_each_in_one_undo_step`); s48 `the_tools_menu_opens_the_property_manager_and_definitions_are_undoable` |
| BC-2 | Properties tab in every specification dialog of a kind that has properties (shared frame); OK is one undo step with the dialog. | Works | `dialogs.rs SpecDialog::frame`; `shell/spec_dialogs.rs`; main.rs wall / opening hooks; `build_tools.rs` room; s48 `a_door_dialog_gets_a_properties_tab_and_ok_is_one_undo_step`, `cabinet_symbol_and_wall_dialogs_take_the_tab_through_the_shared_frame`. Roof plane, framing and device dialogs share the SpecDialogs path (unit-tested session; no scenario of their own). |
| BC-3 | Custom properties as schedule columns (`prop:<name>`), shown by the "show in schedule" flag or added in the Schedule Specification column list; sort, filter and group see them. | Works | `schedule_kinds.rs effective_columns`, `Entry::cell`; `schedules.rs reconcile_columns`; `schedule_spec.rs with_props`; tests `custom_properties_are_schedule_columns`, `custom_property_columns_are_listed_for_the_kind_and_follow_it`, s48 `a_custom_property_is_a_schedule_column_and_follows_the_tab` |
| BC-4 | Export for Editing (XLSX): one sheet per schedule, hidden `PlanStudio ID` column, header row, editable cells unlocked and tinted, computed cells shaded and locked by sheet protection, drop-downs for list and Yes / No properties, hidden `_meta` sheet (plan path, time, kind, column to field map, baseline). Tools menu, Schedule Specification button and the schedule's context menu. CSV when the file name ends in .csv. | Works | `plan-docs/src/xlsx.rs` (`to_xlsx_edit`), `props_exchange.rs` (`export_workbook`, `export_csv`); tests `the_workbook_has_a_hidden_id_column_meta_and_protection`, `a_written_workbook_reads_back_cell_for_cell`; Excel itself not run here: verify in Excel (protection, validation, hidden column) |
| BC-5 | Import Property Data (XLSX or CSV): own zip + inflate + sheet XML reader, rows matched by `PlanStudio ID` (fallback: mark and kind), diff of (object, field, old, new), review dialog with a checkbox per change, conflict notes (object deleted, value invalid for the type, computed column edited and ignored, also changed in the plan), one undo step, count in the status bar. | Works | `plan-docs/src/xlsx_read.rs`; `props_exchange.rs plan_import / apply_changes`; `dialogs/import_review.rs`; tests in those files and s48 `export_edit_import_applies_the_checked_changes_as_one_undo_step`, `deleted_objects_and_bad_values_are_noted_and_never_applied`, `csv_edited_in_a_text_editor_imports_the_same_way`. Reading a file Excel itself saved: verify in Excel (the reader is tested on Excel-style XML: shared strings, rich runs, prefixes, deflate) |
| BC-6 | "Workbook changed - import?" offer: the exported workbook is polled every 2 s and offered when newer than the last import. | Works | `property_manager.rs watch_frame`, `Watch::changed`; tests `the_watch_offers_only_a_workbook_newer_than_the_last_import`, s48 `the_status_bar_offers_a_workbook_that_changed_after_the_export` |

## Manual audit additions (part 3)

Rows added by the Chief X18 Reference Manual audit, part 3 (pages 521 to 761; `docs/chief-manual-coverage/part3-text-doors-windows-cabinets-electrical.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the program.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| L-57 | Displaying cabinets (manual p. 655): Cabinet Schedule with fillers (manual ones), soffits, shelves, partitions, panels, countertops, backsplashes, holes; Cabinet Accessories category; moldings in the Molding category; architectural block members | Partial | no spec yet (Cabinet Schedule kind with columns (CB-21); soffit/shelf/partition/countertop rows: cabinets of every kind are listed; accessories, molding and counter profile categories: no (L-57)); manual audit part 3; verify in Chief. |
| L-58 | Schedule defaults and tools (manual p. 708): Schedule Defaults dialogs per schedule type, same fields as the Schedule Specification (also for callout labels); open by double-clicking the Schedule Tools button | Partial | no spec yet (Default Settings > Schedules lists 14 kinds (default_pages/plan.rs SCHEDULES); each page is title, show title, border, grid lines, row height, text style, number rows, sort, callouts as stored values no schedule reads (DS2); new schedules start from ScheduleKind::default_columns); manual audit part 3; verify in Chief. |
| L-59 | Schedule defaults and tools (manual p. 708): Schedules listed in the Project Browser; Find in Plan / Find in Layout | Partial | no spec yet (schedules appear in the Project dock (shell/docks.rs); Find in Plan/Find in Layout context items: no); manual audit part 3; verify in Chief. |
| L-60 | Schedule defaults and tools (manual p. 709): Room Schedules: Create Schedule from Room edit tool, or Include Objects from Room in the specification; objects count by centre point; distributed objects by path/region centre | Missing | no spec yet (R-105); manual audit part 3; verify in Chief. |
| L-61 | Schedule defaults and tools (manual p. 710): Manage Custom Schedule Categories (Tools > Schedules, or Default Settings > Schedules > Custom Schedule Categories): create, rename, delete categories; assign objects in their Schedule panel; include in schedules on the General panel; also a New Custom Category button | Missing | no spec yet (L-23 notes Manage Custom Schedule Categories as open; no categories on objects (ScheduleKind only)); manual audit part 3; verify in Chief. |
| L-62 | Schedule defaults and tools (manual p. 710): Layout Page Table and Layout Revision Schedule tools in layout | Partial | no spec yet (Layout Page Table (menus.rs, shell/layout_window.rs); revisions print in the title block REVISIONS table from Project Information (L-52, L-54); a separate placeable Revision Schedule: not found); manual audit part 3; verify in Chief. |
| L-63 | Editing schedules (manual p. 711): Schedule edit handles: Move, side Resize for width, Rotate, Resize Column handles, Move Row handles, Move Column handles, Sort by Column triangles, Wrap Schedule diamond | Missing | no spec yet (editor/schedule_view.rs and handles.rs give a placed schedule no handles; it moves by dragging the table and edits in the dialog); manual audit part 3; verify in Chief. |
| L-64 | Editing schedules (manual p. 712): Swap Rows/Columns: list objects in columns and attributes in rows | Missing | no spec yet (a schedule has rows for objects only (schedule_view.rs); no transpose option); manual audit part 3; verify in Chief. |
| L-65 | Editing schedules (manual p. 713): Edit toolbar: Align Left/Right/Center/Justify on columns, Find Object in Plan from a row, Spell Check, Add to Library | Missing | no spec yet (a selected schedule gets only the common edit buttons (editor/actions.rs); none of the schedule-specific tools exist); manual audit part 3; verify in Chief. |
| L-66 | Editing schedules (manual p. 713): Open Row Object(s): open the specification of the objects in the selected row (several objects show No Change fields) | Partial | no spec yet (clicking a schedule row selects the object (L-27 select_row_target); no spec dialog from the row); manual audit part 3; verify in Chief. |
| L-67 | Editing schedules (manual p. 715): Default row numbering: existing objects first in alphanumeric label order, new unique objects at the bottom, stable edits (a single-quantity row keeps its place; a row that no longer matches drops to the bottom) | Differs-by-design | no spec yet (marks are computed in reading order across the plan and recomputed (opening_edit.rs renumber_marks, DECISIONS 44); Renumber Schedule writes overrides); manual audit part 3; verify in Chief. |
| L-68 | Columns, totals, custom fields (manual p. 716): Columns to Include by schedule type (Floor, Code, Comment, Description, Manufacturer, Supplier from Object Information); Categories to Include; 'Other' for architectural blocks | Partial | no spec yet (Field lists per kind (schedule_kinds.rs); Code, Comment, Description, Manufacturer, Supplier on door/window/cabinet only (DECISIONS XL3); Categories to Include: no); manual audit part 3; verify in Chief. |
| L-69 | Columns, totals, custom fields (manual p. 716): Area columns in Door, Window and Room Finish schedules with a Totals row on by default (and Volume in Room Finish) | Partial | no spec yet (Room schedule has an Area column and a count/area Totals line (L-28); the Door and Window schedules have no Area column (DOOR_FIELDS and WINDOW_FIELDS have width, height, sill, head only); Room Finish has no Volume); manual audit part 3; verify in Chief. |
| L-70 | Columns, totals, custom fields (manual p. 717): Custom columns by custom Sub Categories (Preferences) and Custom Object Fields; library objects keep their fields | Partial | no spec yet (custom properties via the Excel round trip (DECISIONS XL1, plan-core props.rs) show as property columns in Schedule Specification; Sub Categories and Manage Custom Fields: no); manual audit part 3; verify in Chief. |
| L-71 | Schedule Specification dialog (manual p. 722): Wrapping: Entries per Table or Max Table Size, Wrapped Schedule Offset, Justify Wrapped Tables, table alignment, title and headings on every wrapped table | Missing | no spec yet (a schedule is always one table (schedule_view.rs layout); a long schedule runs off the page instead of wrapping); manual audit part 3; verify in Chief. |
| L-72 | Schedule Specification dialog (manual p. 724): Number Formatting: fraction style and text size; per-column units, unit indicators, leading/trailing zeros, thousands separator, decimal places or smallest fraction, show denominator, reduce fractions with GCD or closest | Missing | no spec yet (schedule cells use one length format of the plan (units.rs); no per-column format); manual audit part 3; verify in Chief. |
| L-73 | Schedule Specification dialog (manual p. 726): Labels: Use Both Callout and Label / Callout only / Label only; Schedule Number Prefix and start; number format; leading zeros; ten callout shapes; fill colour and transparency; automatic size; shape and text angles; Auto Adjust Text Direction; Follow Label; callout layer choice | Partial | no spec yet (Labels tab: show labels, numbering, prefix (L-26); circle for doors and hexagon for windows; callout shape, fill, size, angle and layer controls: no); manual audit part 3; verify in Chief. |
| L-74 | Object labels (manual p. 728): Object labels in plan, camera and section views: automatic, custom with macros, or schedule callouts; CAD polylines, boxes and revision clouds can have labels too | Partial | no spec yet (labels exist for openings, cabinets, rooms, walls, devices (DW-59, CB-13, R-44); CAD object labels: no; camera-view labels: none); manual audit part 3; verify in Chief. |
| L-75 | Schedule panel (manual p. 735): Schedule panel on objects: Include in Schedule, Show Schedule Callout, Callout Location Rotation, Auto Schedule Category or Include in Schedule As (custom categories), New Custom Category | Partial | no spec yet (Schedule tab on openings and cabinets (Include in Schedule, Mark, manufacturer fields; L-29, DECISIONS 56); callout rotation, categories and custom categories: no); manual audit part 3; verify in Chief. |

## Manual audit additions (part 5)

Rows added by the Chief X18 Reference Manual audit, part 5 (pages 1099 to 1306; `docs/chief-manual-coverage/part5-materials-3d-rendering-pictures-import-export.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the program.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| L-76 | Export Picture options: active window size, open in viewer, remembered settings (manual p. 1253): Export Picture dialog: Use Active Window Size, Pixels or Units, Width and Height, Resolution in pixels per inch or mm (metadata only), Retain Aspect Ratio, Transparent Background, Open in Default Image Viewer; settings persist; a version in Send to Layout. | Partial | pixel width or paper size at a DPI, background white or transparent, JPEG quality, format (`dialogs/export_picture.rs`); Use Active Window Size, Units, the default-viewer option and persistence are missing |
| L-77 | PDF Box objects and page-range import (manual pp. 1259-1261): Import PDF (File > Import > Import PDF or drag): multi-page choice of current page, a range or all pages, one PDF Box per page; only 2D data; PDF Box Specification: file with Browse or Edit Path, Page, Save in Plan, center and angle, size and aspect, Reset Cropping, Line Style (Show Outline), Fill Style, Label (file and page). | Partial | File > Import > Underlay Picture (PNG, JPEG, PDF) for PDFs whose pages carry one embedded picture (L-46, `tools/underlay/pdf.rs`); no vector PDF rendering, no page range, one underlay per import / the Underlays window has a PDF page box, name, opacity, rotation and lock |
| L-78 | DWG import and the Import Drawing dialog options (CAD blocks, current view, library) (manual pp. 1289-1290): File > Import > Import Drawing (DWG/DXF) into plan, section, CAD detail or layout; dragging a file onto the window starts it; Import Drawing dialog: files list; Show Import Assistant; Show For Each File; Create CAD Blocks with Place In Current View, Auto Position Blocks, Add to Library. | Partial | File > Import > Import Drawing (DWG/DXF) opens the Import Drawing window (`dialogs/import_drawing.rs`: files list, Show Import Assistant, Show For Each File, Create CAD Blocks, Auto Position Blocks) and a .dxf or .dwg dropped on the window starts it; ASCII and binary DXF, R12 to 2018 (`plan-import` `parse_dxf_bytes`; tests `dxf::tests::*`, `s57_dxf_import::the_menu_command_opens_the_file_picker_hook_and_pages_run_in_order`, `a_binary_dxf_imports_like_the_text_one`); DWG shows how to save a DXF (DECISIONS DX1, `a_dwg_is_refused_with_the_way_to_save_a_dxf`) / Place In Current View and Add to Library are not built, nor import into sections, CAD details or layouts (DX12, integration queue); verify in Chief |
| L-79 | Import Drawing entity coverage (ellipse, spline, hatch, solid, dimension, attribute, point) (manual p. 1291): Entities imported: lines, circles, arcs, ellipses, splines (as polylines), polylines and lightweight polylines (bulges become arcs, widths ignored), points (only when a layer becomes Elevation Data), text and multi-line text (as rich text, first font wins, Arial fallback), multileaders, Unicode text, blocks and inserts, hatch (as solid polylines), 2D solids, 3D faces and polyface meshes, rotated/aligned/3-point angular dimensions, attributes, line styles by name, layers. | Works | `plan-import` reads LINE, LWPOLYLINE and POLYLINE (bulges kept as arc edges), CIRCLE, ARC, ELLIPSE, SPLINE (NURBS and fit points as polylines), HATCH (solid fill, or the Hatch tool's pattern lines), SOLID, TRACE, 3DFACE, polyface meshes, POINT (small circle), TEXT, MTEXT (justification, rich runs, first font, wrap width), ATTRIB/ATTDEF (as text), LEADER, MULTILEADER, linear and aligned DIMENSION (as dimensions; angular, radius, diameter and ordinate drawn as lines and text, DX6), INSERT/MINSERT (CAD blocks, nested), line types by name or pattern, layers (tests `dxf::tests::*`, `convert::tests::*`, `s57_dxf_import::*`); hatch look, points and angular dimensions differ from Chief (DX5, DX6, DX8; verify in Chief) |
| L-80 | Import Drawing Assistant (join lines, boxes, layers, duplicate blocks, unit scale) (manual pp. 1292-1295): Import Drawing Assistant, Select File page: Browse, Polylines (join lines with shared end points), Boxes (closed rectangles), CAD blocks all or referenced only, Import Hatch entities, password for protected files; Select Layers: visible layers checked, frozen unchecked, Select All and Clear All, Convert To Terrain Perimeter or Elevation Data (plan view only); Duplicate CAD Blocks page: auto-name (_Copy_1), replace, keep existing, or manage each block; Advanced page with Auto Name, Replace, Use Existing. | Partial | `dialogs/import_drawing.rs` pages Select File (Polylines, Boxes, Import Hatch, paper space), Select Layers (checks, visible/frozen defaults, Select All, Clear All, To walls with a wall type), Layer Mapping (single layer, same names with or without attributes), Advanced Layer Mapping, Duplicate CAD Blocks and Advanced Duplicate (Auto Name, Replace, Use Existing), Drawing Unit (units, scale, dimensions as dimensions or blocks, move to origin, insertion), Import Complete (tests `s57_dxf_import::*`, `convert::tests::a_block_the_floor_already_has_is_renamed_replaced_or_reused`) / Convert To Terrain Perimeter or Elevation Data, passwords and importing unused block definitions are not built (DX4, DX11) |
| L-81 | DXF/DWG export: AutoCAD version, DWG and binary DXF, split wall assemblies, pattern lines (manual pp. 1298-1299): Export Current View (plan, CAD detail or orthographic 3D view; a perspective view is not to scale) and Export All Floors (one file, layer names with a floor suffix such as "Electrical-2"); custom line styles become solid; Export Drawing dialog: AutoCAD version, Layer Set with Define, Split Wall Assemblies Into Layers, Export Only Displayed Layers or all used and named layers, Scaling Unit, Create Associative Dimensions, Export Pattern Lines, Export Filled Areas as 2D solids, Export AutoCAD Index Colors; .dwg, .dxf or binary .dxf. | Partial | File > Export > DXF (the active floor, all floors or a pick, "Floor name on each layer", 2D or 3D) and Elevation DXF (L-44, L-45); detail and 3D vector views export only through the camera drawing / units, layer naming (Chief or AIA with a map), line weights, text as text or lines, floors, 2D or 3D (`dialogs/dxf_options.rs`); R12 ASCII DXF only: no version, DWG, binary DXF, wall-layer split, associative dimensions, pattern lines, filled areas or index colors |
| L-82 | Export 360 Panorama: Save as Backdrop and HDR output (manual p. 1302): Export 360 Panorama: Height and Width in pixels, Limit Dimensions to Powers of Two, 2:1 ratio, Save to Disk or Project/Assets, Save as Backdrop (an .hdr for Physically Based or Clay, else a .jpg), Save to Chief Cloud. | Partial | C-77, C15-1: widths 1024-8192 at 2:1, samples, PNG plus an HTML viewer; Save as Backdrop is missing; the cloud is out of scope |
| L-83 | Thermal Envelope Data (CSV) and REScheck (.rxl) export (manual pp. 1304-1306): Export Thermal Envelope Data: a .csv by floor level with direction and area of each envelope component (floor platforms, ceiling platforms, walls, doors, windows); Export to REScheck (.rxl): dialog with Group Similar Walls and Group Similar Doors/Windows; project data (front faces, conditioned floor area, owner/agent from client info, designer info, New Construction, 1-and-2 Family Detached); location and permit not exported; Envelope data exported: floors (assembly, area, cavity and continuous R-values), slabs on grade (perimeter and R-value), ceilings, walls (assembly, orientation, area, R-values, grouped by orientation), doors and windows (assembly, orientation, area, U-factor, SHGC); skylights not exported; wall labels carry over. | Missing | the data exists (door and window U-factor and SHGC, DW-115; Conditioned room radios, R-43) but nothing exports it / same / same row; the North Pointer is not read by anything (see North Pointer) |

## Manual audit additions (part 6)

Rows added by the Chief X18 Reference Manual audit, part 6 (pages 1307 to 1497; `docs/chief-manual-coverage/part6-terrain-plants-materials-lists-layout-printing-schedules.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the application.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| L-84 | Tools > Materials List submenu (Calculate Materials for All Floors, in Room, From Selection, Materials List Polyline, Master List, Management, Generate a Report) (manual p. 1368) | Works | menus.rs Tools > Materials List submenu (All Floors, From Selection, in Room, Materials List Polyline and Defaults, Master List, Management, Generate a Report, Edit/Save Active View, Update To/From Master List, Export, Print); ids in dialogs/materials_list.rs `cmd`, run by `run_command` from dispatch.rs; test s58 `the_menu_opens_a_list_of_all_floors_with_the_default_columns`; verify in Chief. |
| L-85 | Calculate Materials From Selection edit button (selected objects and their components; a selected room counts moldings, finishes and subfloor, not framing or openings) (manual p. 1368) | Works | Edit toolbar button `materials.selection` (materials_list::edit_buttons, edit_commands.rs hook); plan-docs `Filter::Selection`; tests s58 `calculate_from_selection_counts_only_the_selected_objects`, list_tests `selection_counts_only_the_selected_objects`, `a_selected_room_counts_its_finishes_but_not_doors_or_framing`. Moldings and subfloor have no take-off rows (DECISIONS ML7); verify in Chief. |
| L-86 | Calculate Materials in Room (also on a room's edit toolbar) (manual p. 1368): contents with floor, ceiling and facing wall finishes; objects count if their center is inside; distributed objects count if their path or region center is inside | Works | `Filter::Room` in plan-docs materials/engine.rs (contents by center, floor and ceiling finish, wall surface facing the room, openings in its walls); room Edit toolbar button; tests s58 `calculate_in_room_has_the_rooms_finishes_and_its_openings`, list_tests `a_room_list_has_its_contents_finishes_and_facing_walls` (DECISIONS ML7); distributed objects are not separate rows; verify in Chief. |
| L-87 | Materials List Polyline (manual p. 1369): calculate for a drawn area (editable shape, saved, reusable, hole support, convert from a CAD polyline) | Partial | tools/materials_list_polyline.rs (drag or two clicks; closed CAD polyline plus `MaterialsPolyline` record, one undo step), list_tests `a_polyline_counts_objects_by_center_contained_or_intersected`, `a_hole_keeps_objects_out_and_a_deleted_polyline_says_so`, s58 `the_polyline_tool_draws_an_area_and_calculates_from_it`; Convert Polyline and drawing the holes are not wired (DECISIONS ML2); verify in Chief. |
| L-88 | Calculate Structural Materials for Deck edit button (includes accessories from the deck room's Components panel) (manual p. 1369) | Missing | deck framing exists (CB-86 Works, decks round 15) but no per-deck list button; manual audit part 6; verify in Chief. |
| L-89 | Master List (list of previously used materials with price, manufacturer, etc.) opened from the submenu (manual p. 1369) | Works | master_list.rs `MasterList` with Use, Quantity, Default, Supplier, Manufacturer, Code, Markup, Labor, Equipment, Comment, Accounting Code; Master List view in dialogs/materials_list/master.rs (Category, Columns, Find, Add Item, Delete); tests master_list.rs `update_to_matches_on_category_size_description_and_label`, `default_wins_then_the_last_entry_and_quantity_discounts_apply`, `find_walks_right_then_down`. |
| L-90 | Materials List Management (manual p. 1369): list of saved Live Lists and Reports with Edit, Copy, Rename, Delete | Works | dialogs/materials_list/extras.rs `management` (Edit, Copy, Rename, Delete); materials_list.rs `copy_saved`, `rename_saved`, `delete_saved` (one undo step each); test s58 `management_copies_renames_and_deletes_with_undo`. |
| L-91 | Generate a Report (manual p. 1369): freeze a live list as a static Report not linked to the model (editable, update to and from Master List) | Works | materials_list.rs `generate_report` (button and Tools menu); plan-core `ReportRow`; tests s58 `a_report_is_frozen_editable_and_saved_with_its_rows`, list_tests `reports_freeze_edit_and_update_from_the_master_list`. |
| L-92 | Preferences > Master List panel (current Master List file and the columns used) (manual p. 1369) | Partial | Preferences > Materials List page has waste, round up, show prices, all floors (MaterialsPrefs); the Master List file is fixed at ~/.plan-studio/master-list.json; manual audit part 6; verify in Chief. |
| L-93 | Code, Comment, Manufacturer, Supplier and Price, Extra, Markup, Labor, Equipment entered on objects (defaults dialogs and library items) (manual p. 1370) | Partial | Code, Comment, Manufacturer, Supplier, Price on every object dialog that has the new tabs (dialogs/object_info.rs); Extra, Markup, Labor and Equipment per component in the Components tab; not on defaults dialogs or library items; tests s58 `the_panels_open_with_the_wall_dialog_and_ok_is_one_undo_step`, `the_components_panel_lists_an_objects_line_items_and_overrides_them`. |
| L-94 | Material definitions decide how components are counted; framing as linear feet, cut list or buy list in the Structural Member Reporting dialog (manual p. 1370) | Partial | Per-list Buy List, Cut List and Linear Feet (`FramingStyle`, the Specification's General tab); test list_tests `framing_is_a_buy_list_a_cut_list_or_linear_feet`; the toolbar's Structural Member Reporting control and saved reporting defaults are not built (DECISIONS ML8). |
| L-95 | Custom Materials List formulas replace default calculations (manual p. 1370) | Missing | no formula editor (see 45.5); manual audit part 6; verify in Chief. |
| L-96 | Materials List by Layer Set (Options panel; default Layer Set Defaults) (manual p. 1371) | Missing | layer sets do not filter the list (grep finds nothing in materials.rs); manual audit part 6; verify in Chief. |
| L-97 | Categories (Electrical, Framing, ...) shown in the ID column; each line's category can be changed in the list, the Master List or a Components panel; the category list is fixed (manual p. 1371) | Works | Move to Category in a row's menu and the ID cell edit (`list::edit_cell`, `ObjectInfo.category`); test list_tests `editing_a_cell_writes_the_objects_and_the_next_list_shows_it`. |
| L-98 | Subcategory column (empty by default; typed in the list or the Components panel) (manual p. 1372) | Works | Sub Category column, typed in the list (`edit_cell`) or in Object Information; test list_tests `object_information_splits_rows_and_shows_in_the_columns`. |
| L-99 | Details dialog for selected line items (manual p. 1372): every column, Source Object, Find button to show the object in a plan view (opens when objects span floors) | Works | Details window (extras.rs `details`, plan-docs `list::details`) with every column and the Source Objects, and its Find button; test list_tests `rows_expand_into_their_objects`. |
| L-100 | Appearance of lists (manual p. 1372): grid lines, colors, font set in the Specification dialogs | Partial | Text Style tab: grid lines, solid or dashed, custom colors, size, bold, italic, underline, strikeout apply in the window; the font family is kept and used by HTML export only; test s58 `the_menu_opens_a_list_of_all_floors_with_the_default_columns` (draws), list_tests `html_escapes_and_colors_follow_the_choice`. |
| L-101 | Open from Tools > Active View > Edit Active View when a list or the Master List is open; the Master List dialog has only the Columns panel (manual p. 1372) | Works | Edit Active View opens the Specification (materials_list/spec.rs) from the menu or the button; the Master List view has only the Columns choice. |
| L-102 | Categories panel (manual p. 1373): check the categories shown (order as listed), Select All, Clear All, multiselect; hidden categories remain in the list and in exports | Works | Categories tab with Select All and Clear All (`ListSpec::show_all_categories`); hidden categories stay in exports (DECISIONS ML3); test list_tests `hidden_categories_leave_the_view_but_stay_in_exports`; verify in Chief. |
| L-103 | Columns panel (manual p. 1374): check columns, Move Up, Move Down | Works | Columns tab: check, Move Up, Move Down (`ListSpec::move_column`); plan-core materials_data tests `column_order_move_and_normalize`; list_tests `columns_show_hide_and_reorder_in_the_table`. |
| L-104 | Options panel (manual p. 1374): Layer Set (with Define), Structural Member Reporting default, Restrict to Floor, Restrict to Supplier (all, none or a supplier) | Partial | General tab: Restrict to Floor (scope), Restrict to Supplier (all, none, one), Structural Member Reporting; Layer Set is not built (L-96); tests list_tests `object_information_splits_rows_and_shows_in_the_columns`. |
| L-105 | Appearance panel (manual p. 1375): horizontal and vertical grid lines, solid lines, custom colors, font, size and styles, Reset to Defaults, preview | Partial | Text Style tab (see L-100). |
| L-106 | Column set (manual p. 1377): ID, Use, Sub Category, Floor, Label, Supplier, Manufacturer, Code, Size, Description, Quantity, Count, Extra, Price, % Markup, Labor, Equipment, Total Cost, Default, Comment, Accounting Code, with a table of where each is available and editable | Works | plan-core `MlColumn` (21 columns, titles, availability, editable) and plan-docs `list::cell`; test plan-core `there_are_21_columns_with_chiefs_titles`; list_tests `the_total_cost_formula_uses_extra_markup_labor_and_equipment`. |
| L-107 | ID column (manual p. 1377): automatically generated identifier per line; Size and Description are editable text (clear the text to restore) | Works | ID `FRM-003` and Size and Description typed over (an empty cell restores; a single space blanks Size); test list_tests `editing_a_cell_writes_the_objects_and_the_next_list_shows_it`. |
| L-108 | Count versus Extra (extra amount in the same unit, reflected in Count) and the Total Cost formula (Count + Extra) x Price x (1 + Markup/100) + Count x Labor + Count x Equipment (manual p. 1378) | Works | `list::total_cost` = (Count + Extra) x Price x (1 + Markup/100) + (Count + Extra) x Labor + (Count + Extra) x Equipment; Count includes the Master List's waste (DECISIONS ML5); test list_tests `the_total_cost_formula_uses_extra_markup_labor_and_equipment`. |
| L-109 | Accounting Code column (used for BuilderTREND export) (manual p. 1378) | Works | Accounting Code column, Object Information field and BuilderTREND export; test list_tests `builder_trend_csv_has_cost_codes_and_unit_costs`. |
| L-110 | Expand and Collapse rows (manual p. 1379): a line with an arrow represents several objects, shown separately when expanded; not in Reports | Works | Expand and Collapse in live lists (row-number arrow and buttons; `list::expand`); test list_tests `rows_expand_into_their_objects`. |
| L-111 | Find Object in Plan edit button (manual p. 1379): select a row to locate its object in the plan | Works | Find Object in Plan (button, double-click on a row number, Details > Find): materials_list.rs `find_object` goes to the floor, selects the object and centers the view; test s58 `find_object_goes_to_the_floor_selects_and_centres_the_view`. |
| L-112 | Adding information (manual p. 1379): type Price, Supplier, Code, Comment, Manufacturer into a cell of a live list or report, then add to the Master List | Works | Cells of live lists and Reports are typed into (Supplier, Code, Comment, Manufacturer, Price...), then Update To Master List; tests s58 `typing_in_a_cell_of_a_live_list_writes_the_object_and_is_one_undo_step`, `update_to_master_list_keeps_a_price_for_the_next_plan`. |
| L-113 | Tools > Update From Master List (look up Price, Supplier, Code by item, honoring Default) and Update to Master List (save a selected row; matching by Category, Size, Description, Label) (manual p. 1379) | Works | Update To Master List (selected rows, matching Category, Size, Description, Label) and Update From Master List (Reports; live lists read the Master List every time, honoring Default); tests master_list.rs `update_to_matches_on_category_size_description_and_label`, list_tests `update_to_master_list_saves_the_edited_rows`. |
| L-114 | Change information in any column of an individual list or report; change a line's category/subcategory from the ID column (manual p. 1379) | Works | Any editable column of a live list or Report (`list::edit_cell`, `edit_report_cell`); tests list_tests `reports_freeze_edit_and_update_from_the_master_list`. |
| L-115 | Changing count units (piece, linear ft/m, sq ft/m, cubic yd/m) by double-clicking a Count cell, with the same unit in Extra; Number Formatting dialog (units, formatting, accuracy) from the Count or Extra cell (manual p. 1379) | Missing | units fixed per row (ft, sq ft, cu yd, sheets, pieces); no per-cell unit or Number Formatting dialog (DIM-31 formats exist for dimensions only); manual audit part 6; verify in Chief. |
| L-116 | Currency from the operating system's Region settings (manual p. 1380) | Partial | fmt_money prints "$1,234.50" always (materials.rs); manual audit part 6; verify in Chief. |
| L-117 | Formulas and Count/formula tool tips showing more precise values when rounded (manual p. 1380) | Partial | Count cell tool tip with the exact value, waste and rounding (`list::count_tooltip`); no formula tool tips (L-121). |
| L-118 | Total Cost column using the Count, Price, % Markup, Labor and Equipment formula (manual p. 1380) | Works | Total Cost column (see L-108). |
| L-119 | Live lists and Auto Rebuild (manual p. 1380): editing an auto-generated framing member, roof plane, foundation component or attic wall prompts to turn off Auto Rebuild or to edit defaults | Missing | lists are read only, no prompt; manual audit part 6; verify in Chief. |
| L-120 | Copy parts of a list and paste into a Text object, word processor or spreadsheet (manual p. 1380) | Partial | Export CSV/Excel/PDF/Send to Layout instead (L-37); no clipboard copy of cells; manual audit part 6; verify in Chief. |
| L-121 | Custom formulas in Ruby syntax in editable cells ("=" prefix), Insert Macro with User Defined, Materials List Column, Object Specific and Parent Object macros; Apply Formula to Line Item or Source Object; Revert to Default; macros with Evaluate and Owner Object / Materials List Line Item contexts (manual p. 1381) | Missing | Ruby is out of scope (see chapter 48); no formula cells; manual audit part 6; verify in Chief. |
| L-122 | Materials List Polyline tool (rectangular polyline, edited to any shape, can sit in schedules, hole support) with Calculate Materials List edit button (manual p. 1382) | Partial | tools/materials_list_polyline.rs and the Calculate Materials List edit button (see L-87). |
| L-123 | Materials List Polyline Defaults dialog (categories and floors in advance) (manual p. 1382) | Partial | Tools > Materials List > Materials List Polyline Defaults (extras.rs `open_polyline_defaults`, one undo step); the Default Settings tree does not list it yet (docs/integration-queue.md). |
| L-124 | Included Floors/Categories grid (categories as rows, floors as columns; toggle selected, categories, floors, all; Include All Floors; Revert All Changes) (manual p. 1383) | Works | Included Floors/Categories grid with Toggle Selected, Category(s), Floor(s), All, Include All Floors and Revert All Changes (extras.rs `grid`, plan-core `PolylineSpec`); tests plan-core `polyline_grid_toggles`, list_tests `the_polyline_grid_turns_a_category_off_on_a_floor`. |
| L-125 | Included Objects choice (manual p. 1383): Intersected, Contained or by Center (No Change for mixed selections) | Works | Included Objects: Intersected, Contained, by Center (`IncludedObjects`); test list_tests `a_polyline_counts_objects_by_center_contained_or_intersected`. |
| L-126 | Polyline, Selected Line/Arc, Line Style, Fill Style and Label panels for the Materials List Polyline; rooms inside the polyline count whole-room finishes (manual p. 1384) | Partial | Polyline Specification: Label, Perimeter, Area and Number of Lines, and the grid; a room the area takes in counts its whole finishes; Selected Line/Arc, Line Style and Fill Style panels are the CAD polyline's own (CAD Specification). |
| L-127 | Save Active View, Save Active View As (copy), Edit Active View for live lists and Reports; prompt to save a list on close; saved lists in the Project Browser and Management dialog (manual p. 1384) | Works | Save Active View, Save Active View As, Edit Active View (menu and buttons), the save prompt on closing, saved lists as Project Browser rows (shell/docks.rs `BrowserNode::MaterialsLists`) and in Management; tests s58 `a_saved_list_follows_the_plan_when_opened_and_survives_a_file_round_trip`. |
| L-128 | Print a Materials List with File > Print (Print Materials List dialog) (manual p. 1385) | Partial | Print button and Tools > Materials List > Print Materials List: a paged PDF table to the printer, viewer or a file (plan-docs `export::to_pdf`); File > Print does not route to the active list yet (docs/integration-queue.md); test list_tests `print_writes_a_paged_pdf_table`. |
| L-129 | Export options (manual p. 1386): Include Column Headers, Include Hidden Columns, Open in Default Spreadsheet Editor, Export with Colors (XML/HTML) | Works | Export Materials List dialog (extras.rs `export_dialog`): Include Column Headers, Include Hidden Columns, Open in Default Spreadsheet Editor, Export with Colors; test list_tests `exports_write_tab_comma_xml_html_and_workbook`. |
| L-130 | Export Units (manual p. 1386): Include Units with Amounts, in a New Column, or none; currency always included | Works | Units with amounts, in a new column or none (`UnitsMode`); test list_tests `units_go_with_the_amount_in_a_column_or_nowhere`. |
| L-131 | Third Party Formats (manual p. 1387): No Formatting or BuilderTREND (CSV only) | Works | BuilderTREND CSV (`export::to_builder_trend`); the column set is a best guess (DECISIONS ML9); test list_tests `builder_trend_csv_has_cost_codes_and_unit_costs`; verify against BuilderTREND. |
| L-132 | More than one Master List, one active (set in Preferences) (manual p. 1387) | Partial | single file ~/.plan-studio/master-list.json (`MasterList::load/save`); no second list or file choice; manual audit part 6; verify in Chief. |
| L-133 | Master List Category drop-down, Columns choice, Find field (search from the selected cell) (manual p. 1388) | Works | Master List view: Category drop-down, Columns, Find Next (`MasterList::find_next`); test master_list.rs `find_walks_right_then_down`. |
| L-134 | Update to Master List (manual p. 1388): matches Category, Size, Description, Label; creates a new Master List line when none matches; Update from Master List: last entry wins unless one is marked Default | Works | See L-113; `MasterList::update_to`, `find_for` (last entered wins unless Default). |
| L-135 | Edit the Master List (manual p. 1389): modify or delete lines (select row number, Delete), Use check boxes (items bought as one unit), Quantity threshold for quantity discounts, Default column; back up mmaster.mat | Works | Master List view edits any cell, Delete, Use, Quantity, Default (master.rs `set_cell`, `MasterList::remove`, `set_default`); the file is JSON, not mmaster.mat. |
| L-136 | Components panel in object dialogs lists the object's Materials List line items (components and indented subcomponents) with Add Line Item, Remove Line Item, Restore, Revert; Materials List Data table (heading, Formula, Value); extra info for rooms and terrain features; editable also for User Catalog items (manual p. 1389) | Partial | Components tab (dialogs/object_info.rs): the object's line items with Add Line Item, Remove Line Item, Restore, Revert and the Count, Extra, Price, % Markup, Labor, Equipment and Total Cost of the selected line; no Formula column (Ruby); test s58 `the_components_panel_lists_an_objects_line_items_and_overrides_them`, `an_added_line_item_reaches_the_list_and_macros_expand`. |
| L-137 | Object Information panel (manual p. 1391): Code, Comment, Description, Manufacturer, Supplier, with an Insert Macro button on each field | Works | Object Information tab: Code, Comment, Description, Manufacturer, Supplier, with Insert Macro menus; also Sub Category, Accounting Code, Price; test s58 `the_panels_open_with_the_wall_dialog_and_ok_is_one_undo_step`. |
| L-138 | Layout background colour set in the Preferences Colors panel (manual p. 1393) | Partial | Page Setup has a "Layout background (warm off-white)" switch (dialogs/layout.rs PageSetup.page_background); no colour choice in Preferences > Colors; manual audit part 6; verify in Chief. |
| L-139 | Layout Box Defaults dialog (manual p. 1394): default line style, fill style and label format of layout boxes | Partial | Default Settings > Layout has Box border, gap and box title; no line style, fill style or label format defaults (default_pages/plan.rs layout); manual audit part 6; verify in Chief. |
| L-140 | Page Templates (title block and border applied to chosen pages) (manual p. 1394) | Partial | L-10, L-7; the Page Template flag makes a page's boxes and CAD repeat on every other page; no assignment of a template to a chosen page and no multiple templates in use (see 46.14); manual audit part 6; verify in Chief. |
| L-141 | Tools > Layout > Referenced Plan Files lists plan files used by the layout and lets you relink them (manual p. 1394) | Missing | a layout belongs to its plan (Project::layout), so there are no external plan references (DECISIONS 61); import of a Chief layout would need it; manual audit part 6; verify in Chief. |
| L-142 | Rescale Layout View edit tool opens Change Scale (manual p. 1394) | Partial | Layout Box Specification and box Scale drop-down (dialogs/layout.rs); no Change Scale dialog or No Scale choice for boxes; manual audit part 6; verify in Chief. |
| L-143 | Layout Box Layers edit tool opens Layer Display Options for the selected view; Edit Layout (Plot Lines) (manual p. 1394) | Partial | plan boxes choose a layer set in the Layout Box Specification (Source tab); no per-box Layer Display Options window; Edit Layout: see 46.10; manual audit part 6; verify in Chief. |
| L-144 | Edit > Delete Objects deletes categories of objects and Page Information over a scope of pages in the layout (manual p. 1395) | Partial | dialogs/delete_objects.rs is for plan objects; the layout window deletes selected boxes/page CAD only; manual audit part 6; verify in Chief. |
| L-145 | Layout box labels customised to report view information (object-specific macros such as %label_position_and_orientation%) (manual p. 1396) | Missing | box label is free text with the scale note; macros are not expanded in box labels (grep finds none in layout box labels); manual audit part 6; verify in Chief. |
| L-146 | Move a text or CAD object to another page via its Page number in the specification dialog (manual p. 1396) | Partial | boxes have a Page field and Copy Layout Box to Page (L-7); page text and CAD items are moved by cut and paste or not at all (no Page field in their dialogs); manual audit part 6; verify in Chief. |
| L-147 | Text in a rotated layout view rotates with it when its Text Style has Rotate with Plan (manual p. 1397) | Missing | quarter-turn box rotation exists (L-4) but Rotate with Plan text styles are not honoured: grep finds none; manual audit part 6; verify in Chief. |
| L-148 | Global text macros in view text (drawing scale, plan file name) are only valid when the text is in the plan view, not on the layout page (manual p. 1397) | Missing | macros expand by view context only in titles; no view-level macro scoping; manual audit part 6; verify in Chief. |
| L-149 | Dimension labels in camera views sent to layout as Plot Lines with a fill (Colored Fill, Use Edge Line Defaults) (manual p. 1397) | Missing | Plot Lines are not built (see 46.7); manual audit part 6; verify in Chief. |
| L-150 | Four callout types in plan views (manual p. 1397): standard Callout linked to a detail, Note callout linked to a Note Schedule, object label callout linked to a Schedule, Cross Section/Elevation callout with layout information | Partial | Callout and Note tools exist (TXT rows, Notes in the Text flyout); door and window schedule callouts: L-26; camera callouts with their camera label: C-* camera rows; a callout linked to a view or layout page is not found; DECISIONS 102 covers detail names only; manual audit part 6; verify in Chief. |
| L-151 | Pictures, metafiles and PDFs imported onto a layout page, selectable and editable, can sit on Page Templates (manual p. 1397) | Partial | L-5, L-10; pictures and scanned PDFs work (BoxSource::Image/ImageData, L-46); metafiles (WMF, EMF) are not read (grep wmf/emf/metafile finds nothing); manual audit part 6; verify in Chief. |
| L-152 | Source View panel (manual p. 1399): view type and name; Send All Remaining Views to Layout With These Settings when several views are selected | Partial | SendDialog shows the view; "Send All Floors to Layout" sends every floor in one go (C::SendAllFloors); multi-select send from the Project Browser: not found; manual audit part 6; verify in Chief. |
| L-153 | Send Position option (manual p. 1400): Snap to Active CAD Point | Missing | grep finds nothing; manual audit part 6; verify in Chief. |
| L-154 | Send Options (manual p. 1400): Entire Plan/View, Current Screen, Current Screen As Image (with Define for pixel size and transparent background) | Partial | SendDialog sends the view at a scale ("Largest that fits" or a list scale), 3D picture width/resolution; no Entire Plan/Current Screen choice, no transparent background; manual audit part 6; verify in Chief. |
| L-155 | Link Saved Plan View check box (dynamic link to the Saved Plan View or the current floor, layer set, reference floor and Default Set) (manual p. 1400) | Missing | boxes name a floor and a layer set (BoxSource::PlanView); Saved Plan Views are not linked (grep finds none in plan-layout); manual audit part 6; verify in Chief. |
| L-156 | Camera View Options (manual p. 1400): Live View with Update on Demand or Always Update; Plot Lines with Color Fill, Use Edge Line Defaults, Use Pattern Line Defaults | Missing | elevation, section and camera boxes always redraw from the plan (live) with Update Views for perspective boxes; Plot Lines are absent (grep plot.?lines finds nothing); manual audit part 6; verify in Chief. |
| L-157 | Scaling (manual p. 1401): Fit to Sheet (No Scale) at about half the sheet, or an exact U.S. and metric scale; Use Layout Line Scaling keeps line weights and dashes at sheet size | Partial | scale list + "Largest that fits" (L-22); box Line weight scaling multiplier 0.1 to 5 (L-12); no "No Scale" boxes and no per-view layout line scaling switch; manual audit part 6; verify in Chief. |
| L-158 | Saved and unsaved plan views (manual p. 1402): pony wall display, floor level, Reference Display, Color setting, Active Defaults, layer set and Rotate Plan View stay with the layout view; Create Saved Plan View and Unlink Saved Plan View edit tools | Missing | grep create saved plan view finds nothing; a box stores floor, layer set and a quarter-turn rotation only; manual audit part 6; verify in Chief. |
| L-159 | Semi-dynamic views update by Update View, Update All Views, Update All Live Views, and when printed (manual p. 1402) | Partial | C::UpdateViews (Layout > Update Layout Views) refreshes perspective boxes; other update menu entries are not split into Update All Live Views / Update All Plot Line Views; manual audit part 6; verify in Chief. |
| L-160 | Update on Demand views lose quality when zoomed; updating restores resolution; GPU ray trace layout views run 20 samples then denoise, print at Maximum Samples (manual p. 1403) | Partial | perspective boxes carry their own dpi and samples, rendered by Update Views (L-5); print uses the same cached picture; manual audit part 6; verify in Chief. |
| L-161 | Plot Line views (manual p. 1403): surface-edge and material-pattern lines drawn as automatic vector lines in camera views and cross sections; semi-dynamic; updated only by you; image objects, dimensions in cameras, reference models and cross-section slider effects are left out; CAD in cross sections stays dynamic | Missing | no Plot Lines mode (grep finds nothing); elevation and section boxes are already vector drawings (plan-elevation); manual audit part 6; verify in Chief. |
| L-162 | Items not shown in layout views (manual p. 1404): the Reference Grid and camera symbols (camera callouts do show when the Cameras layer is on) | Partial | render.rs omits grid; camera callouts in layout views: not found; manual audit part 6; verify in Chief. |
| L-163 | Layout view layer sets (manual p. 1404): the active layer set is used at send time; Layout Box Layers edits or changes the set for dynamic and semi-dynamic views; changes affect every view that uses the set; static views cannot change | Partial | L-4, Layout Box Specification Source tab picks a layer set (`layer_set`) for plan boxes; editing the set in a window opened from the box: not built; manual audit part 6; verify in Chief. |
| L-164 | Layout Box Borders layer shows borders; the border line style by layer and per box; box fill styles show when borders show (manual p. 1404) | Partial | L-4, L-16; Layout Layer Display Options and the box Line Style tab; box fill style: not built (Layout Box Specification has no Fill Style tab); manual audit part 6; verify in Chief. |
| L-165 | Layout Box Labels layer (manual p. 1405): automatic labels (for instance "1st Floor" or the camera name) editable with text and object-specific macros, with edit handles, callout and marker label shapes linked to another view or page | Partial | L-4; the caption under a box is free text with the scale; labels show the view name; macros and callouts/markers on labels: Missing; manual audit part 6; verify in Chief. |
| L-166 | Missing Layout Views (manual p. 1405): a selectable but invisible view means a broken link; a Caution symbol on unlinked boxes with Relink, Refresh Link, Ignore Invalid Links | Missing | n/a for plan-held layouts; a box whose source is gone (deleted camera) shows an empty frame (BoxSource::Camera note in model.rs); manual audit part 6; verify in Chief. |
| L-167 | Layout box Specification, edit tools, and dimensions that move or resize boxes (manual p. 1405) | Partial | Layout Box Specification (General, Source, Line Style); dimensions to locate boxes: Missing; manual audit part 6; verify in Chief. |
| L-168 | Pan/Scale Layout Box edit tool (manual p. 1406): drag to pan inside the box and type the scale in two inline fields | Missing | grep pan.scale finds nothing in the layout window; manual audit part 6; verify in Chief. |
| L-169 | Recenter Layout Box Contents and Scale Layout Box Contents to Fit edit tools (manual p. 1406) | Missing | not built; manual audit part 6; verify in Chief. |
| L-170 | Non-scaled (Fit to Sheet / No Scale) views (manual p. 1406): corner handle with the Alternate edit behaviour resizes box and image together, other handles crop | Partial | no No Scale boxes (see Scaling); Alternate edit behaviour on layout handles: not built; manual audit part 6; verify in Chief. |
| L-171 | Rescale a floor plan, CAD Detail or section/elevation view (manual p. 1407): Pan/Scale, Scale to Fit, Rescale Layout View (Change Scale dialog with No Scale / a scale / Use Layout Line Scaling) | Partial | box Scale drop-down and the Layout Box Specification scale; the three tools: Missing; manual audit part 6; verify in Chief. |
| L-172 | Edit Layout Lines tool (manual p. 1407): select, add and edit individual edge lines and material pattern lines of a Plot Line view without changing the 3D model (full CAD editing not available, CAD lines in sections not editable); lines are replaced when the view updates | Missing | grep finds nothing; manual audit part 6; verify in Chief. |
| L-173 | Layout line editing (manual p. 1408): handles for size, angle and position; Angle, Object and Grid snaps in layout; delete | Missing | no layout lines; manual audit part 6; verify in Chief. |
| L-174 | Layout Line Specification dialog (manual p. 1408): Line Type (Edge Line or Pattern Line), Line Weight with Use Default Weight, Line Style with Use Default Style and Library button, Line Color with Use Default Color | Missing | not built; manual audit part 6; verify in Chief. |
| L-175 | Edge line colour, weight and style set by layer; pattern lines use the Define Material attributes; in Plot Line views a view-wide default overrides them (manual p. 1407) | Partial | L-12, L-13; elevation/section hatch and line weights come from layers and `wall_face_hatch` (no per-view override); manual audit part 6; verify in Chief. |
| L-176 | Open from a selected layout view (the settings depend on view types selected); Line Style, Fill Style and Label panels match the Layout Box Defaults (manual p. 1409) | Partial | L-4; dialogs/layout.rs BoxTab::{General, Source, LineStyle}; Fill Style and Label panels absent; manual audit part 6; verify in Chief. |
| L-177 | Linked View panel, Plan views (manual p. 1410): File Name, View Name, View Type, Relink button; Dimensions Number Height (legacy only); Saved Plan View Options (Current Floor, Edit View); Floor Level Options (Current Floor, Current Default Set, Show Color); Poche; Reference Display settings | Partial | Source tab: floor and layer set for a plan box; no file name or view type text, no Default Set, Show Color, Poché or Reference Display choices on plan boxes (poche is drawn only in section and elevation drawings, plan-layout render.rs); manual audit part 6; verify in Chief. |
| L-178 | Linked View panel, Camera views (manual p. 1411): Live View (Update on Demand / Always Update), Plot Lines with Color Fill, Edge Line Defaults and Pattern Line Defaults (weight and colour) | Missing | no Plot Lines (see above); manual audit part 6; verify in Chief. |
| L-179 | Box Scale panel (manual p. 1412): No Scale or a scale, Use Layout Line Scaling, Scale Layout Box Contents Only (resizes the box with the scale) | Partial | L-4, L-12; Scale drop-down and Line weight scaling; no No Scale and no "box follows scale" switch; manual audit part 6; verify in Chief. |
| L-180 | Layout Box Polyline, Selected Line/Arc panels (manual p. 1413) | Missing | boxes are rectangles with 8 handles; position and size fields exist in the General tab; manual audit part 6; verify in Chief. |
| L-181 | Layout Box Line Style panel (border style and page) and Fill Style panel (fill shows only when borders show) (manual p. 1413) | Partial | Line Style tab (line weight, border switch); no fill style; manual audit part 6; verify in Chief. |
| L-182 | Layout Box Label panel (label shows on the Layout Box Labels layer with the layer's Text Style) (manual p. 1413) | Partial | Label field in General (free text); no text style choice or macros; manual audit part 6; verify in Chief. |
| L-183 | Double-click a dynamic or semi-dynamic layout view to open the original view; changes update the layout view; layer set changes affect every view using it (manual p. 1413) | Partial | L-6 Open Source View (plan boxes); double-click opens the Layout Box Specification (not the source view); manual audit part 6; verify in Chief. |
| L-184 | Active Defaults of a plan view, cross section or CAD Detail sent to layout become associated with the layout view and are restored when the original view is opened (manual p. 1414) | Missing | Default Sets exist (DS-*) but are not tied to layout boxes; manual audit part 6; verify in Chief. |
| L-185 | Drag a page in the Project Browser to change its page number (manual p. 1417) | Missing | Exchange With Next/Previous only; no drag reorder; manual audit part 6; verify in Chief. |
| L-186 | Navigation (manual p. 1417): arrow buttons, Go To Layout Page dialog (click the number), Tools > Layout > Page Up / Page Down, Shift+N (Page Up) and Shift+M (Page Down), double-click a page in the Project Browser, Show Page, Open Page View | Partial | arrows, page picker (`GoToPage`) and Project Browser clicks exist; Shift+N / Shift+M are listed in the hotkey table (chief-hotkeys-resolved.md) and mapped by name (toolbar/config.rs), not verified in the layout window; Open Page View in a new window: absent; manual audit part 6; verify in Chief. |
| L-187 | Page Templates assigned to other pages cannot be deleted, nor can page zero; Exchange With Next / Previous (not on page 1000 / 0) (manual p. 1418) | Partial | L-7; the template page: delete guard unverified; Exchange With Next/Previous exist; manual audit part 6; verify in Chief. |
| L-188 | Custom page numbering (manual p. 1418): a Label with a prefix and "#" (for example "A-#") numbered in sequence across pages with the same prefix; template pages' labels do not pass to pages | Missing | sheet numbers are plain integers shown as `A-n` (LayoutPage.number); no Label with # pattern; manual audit part 6; verify in Chief. |
| L-189 | Page macros %layout.label%, %page%, %page.print%, %numpages%, %lastpage% (print numbering skips blank pages) (manual p. 1418) | Missing | title block uses %sheet.number%, %page.count% (titleblock.rs); Chief's names are not recognised; no printed-number counting; manual audit part 6; verify in Chief. |
| L-190 | Layout Page Templates (manual p. 1418): title block and border drawn once and assigned to pages; multiple templates allowed; a template page does not print in a range (it prints as Current Sheet) | Partial | L-7, L-10; one template mechanism (flag); a template page is not printed in a range; single template per layout rather than per-page assignment; manual audit part 6; verify in Chief. |
| L-191 | Page Information fields (manual p. 1420): Selected Page picker, Label, Title, Description, Comments; Include in Layout Table (cleared automatically for templates) | Missing | Page Specification has Title and Sheet number only; no Label, Description, Comments, Include in Layout Table; manual audit part 6; verify in Chief. |
| L-192 | Page Template Options (manual p. 1420): Use as Page Template; Assign Page Template drop-down for the page | Partial | Use as Page Template exists; the assign drop-down does not (L-7); manual audit part 6; verify in Chief. |
| L-193 | Page Revisions (manual p. 1420): a table of revisions of the page with New, Edit, Delete, Move Up, Move Down | Missing | revisions are plan-wide rows in Project Information > Revisions (project_info.rs); clouds add rows (DECISIONS 35); not per page; manual audit part 6; verify in Chief. |
| L-194 | Revision Specification dialog (Tools > Layout > Add Layout Revision) (manual p. 1421): Revised Pages, Label, Date (auto), Revised By (default Designer Information), Description, Include in Revision Table | Partial | Project Information > Revisions rows hold label/date/description; no Revised Pages or Revised By, no Add Layout Revision command; manual audit part 6; verify in Chief. |
| L-195 | Layout Page Table (Tools > Layout > Layout Page Table, click a page to place) (manual p. 1421): lists pages with Page Information and data, can sit on a Page Template | Partial | L-11; Layout Page Table dialog (dialogs/layout.rs "Layout Page Table") and Sheet Index box; placement by click on a page; table is edited like schedules: Partial; manual audit part 6; verify in Chief. |
| L-196 | Layout Revision Table placed by click, page-specific, can sit on a Page Template and list only that page's revisions (manual p. 1422) | Partial | L-54 REVISIONS table in the title block (plan-layout titleblock.rs); not a placeable table and not per page; manual audit part 6; verify in Chief. |
| L-197 | Check plots (manual p. 1423): print a large sheet at a reduced scale fraction on smaller paper to check it first (see 47.5) | Missing | grep check plot finds nothing; manual audit part 6; verify in Chief. |
| L-198 | Export layout pages to DXF/DWG in scaled paper units rather than model units (manual p. 1423) | Partial | L-44 (DXF R12 export of plan floors; layout page DXF export: not found); manual audit part 6; verify in Chief. |
| L-199 | Drawing Sheet Setup is view-specific (manual p. 1425): plan view, each cross section/elevation and each CAD Detail has its own sheet size, margins, orientation and drawing scale; new views inherit the plan view settings | Missing | Page Setup is one sheet per layout file; plan views use the layout's sheet and the Print dialog's own scale (dialogs/layout.rs PageSetup, menus.rs 479); manual audit part 6; verify in Chief. |
| L-200 | Drawing Scale in Drawing Sheet Setup is the default scale for the active view's Print, Printed Size Input and Send to Layout dialogs (manual p. 1426) | Partial | Send to Layout and Print choose scales independently; no view-level drawing scale; manual audit part 6; verify in Chief. |
| L-201 | Print View settings remembered per view type (plan, cross section/elevation, layout, Materials List, CAD Details, Time Tracker Logs) across files; Remember Print Settings after Printing switch (manual p. 1426) | Missing | Print dialog starts from defaults each time (PrintDialog::for_plan/for_layout); settings persist per session only if at all (not verified); manual audit part 6; verify in Chief. |
| L-202 | File > Print submenu tools (manual p. 1427): Drawing Sheet Setup, Scale to Fit, Center Sheet, Print Preview, Print, Print Image, Export PDF, Customize Sheet Sizes, Clear Printer Info | Partial | menus.rs Print submenu: Print, Print Preview, Drawing Sheet Setup, Print Image, Print Model, Print Layout, Export Layout PDF; Customize Sheet Sizes is in the Layout menu; Scale to Fit, Center Sheet and Clear Printer Info are absent; manual audit part 6; verify in Chief. |
| L-203 | Scale to Fit (picks a scale that fits the plan to the sheet and recentres the sheet) and Center Sheet (moves the Drawing Sheet relative to the drawing, per floor) (manual p. 1427) | Missing | grep scale to fit and center sheet find nothing; the Drawing Sheet rectangle is fixed to the sheet size at the plan origin (View > Drawing Sheet); manual audit part 6; verify in Chief. |
| L-204 | Drawing Sheet Setup dialog (plan, section/elevation, CAD Detail or layout; not perspectives): Drawing Sheet Orientation and Size with Customize, Show Drawing Sheet in View (manual p. 1428) | Partial | Page Setup (sheet size list with customised sizes, orientation, margins, background, edge line weight); the Show Drawing Sheet switch is the View > Drawing Sheet toggle; manual audit part 6; verify in Chief. |
| L-205 | Drawing Scale for the view, two-part scale (1/4 in = 1 ft, 1 m = 50 m; 1 ft = 1 ft for layouts) with U.S. and metric units chosen independently (manual p. 1428) | Partial | L-22; scale list and custom ratio in the Print dialog (`ScaleChoice`) and in box scales; not stored per view; manual audit part 6; verify in Chief. |
| L-206 | Printer for View (manual p. 1429): choose a printer per view type, Remember Print Settings after Printing, Choose button, Default Printer for View dialog (printer, orientation, paper size, paper source) | Missing | printer is picked in the Print dialog each time (`lpstat -p`); manual audit part 6; verify in Chief. |
| L-207 | Drawing Margins (manual p. 1429): Top, Bottom, Left, Right with Populate from Printer | Missing | one margin value (inches) for the page and in the Print dialog; no per-edge margins or printer population; manual audit part 6; verify in Chief. |
| L-208 | Advanced Line Weights (manual p. 1429): Use 1 for all line weights (Home Designer compatibility), Line Weight Scale (denominator and unit; layout and its plan views must agree) with a preview of weights at the scale | Partial | L-12 pen weights in 1/100 mm with a 0.1 to 5 multiplier per box; no explicit line weight scale per file and no preview; Chief's default scale is 1 = 1/100 mm which matches; manual audit part 6; verify in Chief. |
| L-209 | Drawing Sheet is an object when displayed (manual p. 1432): edit handles to move or resize it (prefer Setup), dimensions can locate its edges; cannot be rotated or copied; blue border shows the printable area | Partial | View > Drawing Sheet draws the sheet outline (editor/render.rs); not selectable, not movable, no printable-area border; manual audit part 6; verify in Chief. |
| L-210 | Center Sheet is stored per floor and does not move object coordinates (manual p. 1432) | Missing | see Center Sheet above; manual audit part 6; verify in Chief. |
| L-211 | Check Plot at a scale fraction (for example 1/2) on smaller paper, with paper size set automatically; resets to To Scale / Fit to Paper afterwards (manual p. 1433) | Missing | grep check plot finds nothing; manual audit part 6; verify in Chief. |
| L-212 | Ways to set line weight (manual p. 1435): by layer, in some specification dialogs (Line Style panel), by wall type (Wall Type Definitions), material pattern lines (Define Material), fill pattern lines in object dialogs, surface edges in the Print dialog, dashed end-cap length in Preferences CAD panel, weight 0 draws a one pixel hairline | Partial | layers and wall types carry line weights (L-12); Define Material pattern line weight (part 4); surface edge weight in Print: absent; weight 0 = hairline in Print Preview when line weights are off (DECISIONS 64); end-cap length setting: CAD prefs (part 2 audit); manual audit part 6; verify in Chief. |
| L-213 | Exact line weights need a Vector View sent to layout with Plot Lines; in the example, dashed lines scale with the view when Layout Line Scaling is off (manual p. 1436) | Partial | Plot Lines: Missing; dashes and widths in paper units (L-12); manual audit part 6; verify in Chief. |
| L-214 | Watermark Defaults dialog (Edit > Default Settings > Watermark, or Define in the Print View dialog) shows a live preview behind the dialog (manual p. 1438) | Missing | not in the Default Settings tree (dialogs/defaults.rs); manual audit part 6; verify in Chief. |
| L-215 | Watermark Type Text (manual p. 1438): text, colour, Print Size (baseline to cap A height), font | Missing | none; manual audit part 6; verify in Chief. |
| L-216 | Watermark Type Image (manual p. 1439): file or Resource selection (Select, Import, Browse, Edit Path), Delete From Plan, Ratio to Sheet | Missing | none; manual audit part 6; verify in Chief. |
| L-217 | Watermark General (manual p. 1439): Layout (such as Tile, Border, Fit to Sheet), Angle, Transparency, Marks per Row and per Column, Margins (Use Drawing Sheet Margin, Top, Bottom, Left, Right), Update Automatically and Update button | Missing | none; manual audit part 6; verify in Chief. |
| L-218 | Print View dialog (plan, 3D Vector View, layout, CAD Detail, Time Tracker Log, Materials List) from File > Print > Print; Export PDF is a similar dialog (manual p. 1440) | Partial | L-18; PrintDialog for plan views and layouts; Materials List and CAD Details print through their own exports; Time Tracker: not built; manual audit part 6; verify in Chief. |
| L-219 | Print dialog saved settings are global per view type; Sheets and Copies reset; Print Source follows the Drawing Sheet toggle (manual p. 1440) | Missing | see Print View settings above; manual audit part 6; verify in Chief. |
| L-220 | Destination (manual p. 1441): printer name or "Chief Architect Save as PDF", DPI | Partial | Destination (PDF file, System printer, Open in viewer) with printer list; DPI only for perspective views; manual audit part 6; verify in Chief. |
| L-221 | Paper (manual p. 1441): Orientation, Size, Source; "Match Print Source" size uses the view's Drawing Sheet Setup | Partial | paper list with custom sizes (including the layout's custom sizes), orientation, margin; no paper source (tray) and no Match Print Source (X18 new); manual audit part 6; verify in Chief. |
| L-222 | Print Source (manual p. 1442): Drawing Sheet (whole sheet) or Current View (what is on screen) | Missing | grep print source finds nothing; manual audit part 6; verify in Chief. |
| L-223 | Drawing Scale (manual p. 1442): Fit to Paper (default 95 percent, global), To Scale, Check Plot | Partial | L-18, L-22; Fit to page, 100 percent, percentage, drawing scale, custom ratio; no 95 percent default note and no check plot; manual audit part 6; verify in Chief. |
| L-224 | Options (manual p. 1442): Copies with Collate, Include Watermark with Define, Print in Color (grayscale or black and white via Preferences Obey Color On/Off) | Partial | Copies and colour modes (Color, Grayscale, Black and white) exist; no Collate (grep collate: none), no watermark; Obey Color On/Off preference: absent; manual audit part 6; verify in Chief. |
| L-225 | Advanced Options (manual p. 1443): Open System Print Dialog button (not with No Printer) | Missing | no OS print panel (L-19 note, out of scope there); the button is not offered; manual audit part 6; verify in Chief. |
| L-226 | Information messages below the preview about page size, resolution and scale to prevent unwanted output (manual p. 1443) | Missing | not built; manual audit part 6; verify in Chief. |
| L-227 | Print Image dialog (manual p. 1444): Destination, DPI, Paper, Copies, Advanced Options, Preview and Information, Print/Save | Partial | ImageDialog (Width/Height, Quality, Scale) and Print Model (Camera, Resolution, Quality, Paper, Destination); no DPI, copies or in-dialog preview; manual audit part 6; verify in Chief. |
| L-228 | Opening plans and layouts of earlier versions (manual p. 1469): .plan and .layout open (X18 saves an unaltered copy in Archives; .pl and .la no longer supported) | Partial | L-50 reads Chief .plan files read-only (plan-chiefplan); Chief .layout files are not read (DECISIONS 36 says templates are not read or written); the Archives copy: DECISIONS 22 (Plan Studio's own Archives); manual audit part 6; verify in Chief. |

## Manual audit additions (part 7)

Rows added by the Chief X18 Tutorial Guide audit, part 7 (pages 1 to 517; `docs/chief-manual-coverage/part7-tutorial-workflows.md`). Each is a workflow step the tutorials rely on that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the application.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| L-229 | Send to Layout scale choices for site plans (tutorial p. 488): 1 in = 30, 40, 50, 60 and 100 ft, or a typed ratio, plus the warning that the view is too big for the sheet. | Missing | dialogs/layout.rs scale combo lists Scale::ALL (3 in to 1 in = 20 ft) and Largest that fits; the Print dialog has a custom ratio but Send to Layout does not; no too-large warning; tutorial audit (part 7). |
| L-230 | General Layout Defaults: Use Snap Grid/Units and an editable Grid Snap Unit that also sets the arrow-key nudge (tutorial pp. 462, 464). | Missing | layout drag snap and nudge are fixed at 1/16 in (shell/layout_window.rs SNAP_IN); no Layout defaults page in dialogs/default_pages; tutorial audit (part 7). |
| L-231 | Layout CAD editing parity for title blocks (tutorial pp. 461-470): Center Object, Point to Point Move, concentric copies, Selected Edge dimensions, object snaps, Fillet and Concentric behaviour on page drawings. | Missing | layout tools are Line, Box, Polyline, Text, TextBox, Circle, Arc, Leader, Cloud with Align/Distribute/Duplicate/Copy to Page and grid snap only (shell/layout_window.rs); tutorial audit (part 7). |
| L-232 | Callout labels on layout section and elevation boxes that link to a layout page and report its label, with arrows and a cross-section line (tutorial pp. 494-496). | Missing | LayoutBox.label is text; L-150 covers plan callouts; no page-linked callout on the layout; tutorial audit (part 7). |
| L-233 | Schedule Specification General: Categories to Include tree (expand Wall, Room, Fixture, Electrical and untick types) and Include Objects from Room / All Floors (tutorial pp. 58, 78, 208, 221, 290, 314). | Missing | Schedule has floor_scope and a text filter (plan-core schedules.rs); no category tree, no room scope; tutorial audit (part 7). |
| L-234 | Picture columns in schedules: 2D Symbol and 3D Perspective, with Scale Images and Use Plan View Scale (tutorial pp. 78, 233, 328). | Missing | schedule fields are text columns (schedules.rs *_FIELDS); tutorial audit (part 7). |
| L-235 | Wall schedule columns Total Width and Wall Construction, Upper and Lower (tutorial p. 55). | Missing | WALL_FIELDS (plan-core schedules.rs) has number, type, length, thickness, height, area, openings, floor, coverings and descriptive fields only; tutorial audit (part 7). |
| L-236 | Copy a page's border and drawings to another page, for example to build the cover template (tutorial p. 477). | Partial | Copy to Page handles layout boxes (LayoutCommand::CopyBoxToPage); page CAD has Duplicate on the same page only; tutorial audit (part 7). |
