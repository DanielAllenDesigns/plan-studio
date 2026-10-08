# Parity spec: Documentation and Layout (Chief Architect X18)

> Status (2026-10-08, re-audited against HEAD 4c11b69): 47 ids: 22 Works, 21 Partial, 3 Missing, 1 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.
>
> Since that count (Round 13): L-10 Save As Template and Apply Template for layouts (JSON under `~/.plan-studio/templates`, a default per sheet size for new layouts); L-9/L-11 revision clouds add rows to the REVISIONS table, and the sheet index follows page reorders without shifting the numbering; plan boxes print the opening labels; leaders can have bends; schedules and the Materials List export to Excel (.xlsx); perspective boxes and Print Model are lit by the plan's point lights; Print Preview shows the Print dialog's colour mode. Still open: the OS print panel (out of scope) and a separate editor for page templates per sheet type (the saved layout templates and the Page Template page flag cover it).
>
> Round 14 (layout and print): a plan can hold several layout files (`Project::layout_files`; New Layout File, the file picker and a Layout file choice in Send to Layout); Page Specification per page (title, sheet number, Page Template flag, its own sheet size and orientation, no title block; the PDF page, title block and packing follow it); Customize Sheet Sizes (custom named sizes, hidden standard sizes, Daniel's ARCH preset; custom sizes in the Print paper list); Shift-click multi-select with Align, Spread, Copy to Page, Duplicate and Open Source View for layout boxes; a Print Preview window drawn from the print's own primitives (colour mode, pen weights or hairlines, scale, tiles, pictures); hatches made for the box scale (camera drawings re-hatched); layout elevations, sections and perspective boxes drawn from the 3D view's scene (roofs, stairs, cabinets, terrain, casing); a picture of the open 3D view can be sent to a page (ray traced, embedded); schedule boxes follow the plan's Schedule Specification (Show All and Reset Columns in the dialog); placed schedules are tables in the floor DXF and boxes in the construction set; Export CSV / Excel for the tables on a layout page and Excel for the framing takeoff; wall-type layers are quantities in the Materials List; a spiral stair is "Spiral" in the Stair Schedule; the Elevation DXF uses the 3D view's scene. Still open: cabinets, stairs and symbols in plan boxes (integration queue), a GL read-back snapshot, floor, ceiling and roof component quantities.

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
| L-38 Plan Footprint | `plan_roof::footprint_from_walls` exists (centerline) | Med | Expose as CAD command using outer wall faces (`joins::wall_outlines` union) |
| L-39 Auto Detail | Round 14: done from a section or elevation (`tools/details/cad_detail.rs`; wall assembly from the wall type layers, framing members, insulation, hatch, notes; manual chapter 20); verify in Chief | Med | Starting from a click on a wall (Chief) is not built |
| L-40, L-41 CAD Detail from View, CAD Detail Management | Round 14: done (details are floors marked `Floor.detail`; `dialogs/details/management.rs`; Send to Layout as a `CadDetail` box) | Med | No .dtl export |
| L-42 CAD to Walls | Missing | Med | Convert selected lines/polylines to walls with default type |
| L-43 DXF/DWG import with units/layers/blocks | plan-import empty | High | DXF ASCII parser (`dxf` crate) to `CadItem`, units dialog, layer map; DWG via external converter (ODA licensing) deferred |
| L-44, L-45 DXF/DWG export options (version, units, blocks, 3D) | R12 ASCII only, floor 0-n single, no options | Med | Version choice (R12/2000+), units, explode toggle, selection export; DWG deferred |
| L-46 PDF/image underlay with calibration | Round 14: PNG/JPEG underlays, PDFs whose pages are pictures (own object-table reader), Point to Point Resize and Rotate to Align (`tools/underlay/{pdf,inflate,trace}.rs`) | High | Vector PDF pages need a rasterizer: export the page as PNG |
| Layout pages in Project Browser, Window menu tabs | None | Med | Project browser panel listing views, layouts, pages |

<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| L-48 | Export ▸ glTF 3D model: Export the 3D model for viewers (Chief exports 3D Viewer, OBJ, 3DS, COLLADA, STL, SKP). (Not captured; verify in Chief.) | Works | menus.rs "glTF…"; plan-view3d export.rs |
| L-49 | Export ▸ Picture / 3D view image (PNG, JPEG, BMP, TIFF): One Export Picture command that saves the active view (plan, 3D, layout page) as a picture file with chosen size and format (PNG, JPEG, TIFF, BMP). Also covers: Export picture (JPEG, PNG, TIFF) of a view. | Partial | File > Export > Picture (dialogs/export_picture.rs, encode.rs): plan (floor + layer set), exterior elevation, elevation/section camera drawing, live 3D view (ray traced from the viewport camera); pixel size or paper size + DPI; PNG, JPEG (own baseline encoder), BMP, TIFF; transparent background for line views in PNG/TIFF; tests export_picture::tests, encode::tests, scenarios s43 (export_picture_saves_the_plan_in_each_format_and_size, export_picture_covers_elevations_and_refuses_an_empty_view). A layout page is not exported as a picture (Print > PDF does it); the 3D picture is ray traced, not a GL read-back. Verify in Chief: dialog wording and option set |
| L-50 | Import ▸ Chief Plan (.plan): Open a Chief Architect plan (read-only decode of Daniel's install files, Round 13/14 stages). (Not captured; verify in Chief.) | Works | menus.rs "Chief Plan…"; plan-chiefplan crate; main.rs import_chief_plan |
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
