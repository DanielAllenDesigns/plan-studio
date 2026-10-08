# Parity spec: Documentation and Layout (Chief Architect X18)

> Status (2026-10-08, after the layout round): 47 ids: 17 Works, 24 Partial, 5 Missing, 1 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.

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
| L-39 Auto Detail | plan-elevation empty | Med | Wall section generator from wall layer definitions (Components); depends on section engine |
| L-40, L-41 CAD Detail from View, CAD Detail Management | Missing | Med | Convert vector view output to `CadItem` list; detail library JSON |
| L-42 CAD to Walls | Missing | Med | Convert selected lines/polylines to walls with default type |
| L-43 DXF/DWG import with units/layers/blocks | plan-import empty | High | DXF ASCII parser (`dxf` crate) to `CadItem`, units dialog, layer map; DWG via external converter (ODA licensing) deferred |
| L-44, L-45 DXF/DWG export options (version, units, blocks, 3D) | R12 ASCII only, floor 0-n single, no options | Med | Version choice (R12/2000+), units, explode toggle, selection export; DWG deferred |
| L-46 PDF/image underlay with calibration | Missing | High | Image object with two-point scale calibration; PDF via rasterize |
| Layout pages in Project Browser, Window menu tabs | None | Med | Project browser panel listing views, layouts, pages |
