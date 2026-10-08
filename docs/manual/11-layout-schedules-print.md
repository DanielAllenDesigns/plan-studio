# Chapter 11: Layout, Schedules and Printing

This chapter covers turning the model into documents: schedules (as windows and as tables placed in the plan),
the layout view where plan views, elevations and sections are arranged on sheets, Project Information and the
title block, the Framing Takeoff, the materials list, the Create Construction Set PDF, and the printing path. It
is honest about what is still missing: a plan has one layout, boxes cannot be rotated, Print writes a PDF rather
than sending anything to a printer, and a few schedule kinds are still to come (11.9).

## 11.1 The documentation pipeline

```
   plan model --+--> schedule windows (tables) ------------------> CSV files
        |       +--> schedules placed in the plan (live tables, callout labels)
        |       +--> materials list --------------------------------> CSV file
        |       +--> 3D scene --> elevations / sections (plan-elevation)
        |                               |
        +-----> the layout (pages of layout boxes, stored in the plan)
                   |  Send to Layout: plan views and elevation / section cameras
                   +--> layout view (page tabs, boxes with handles) --> PDF
                   +--> Project Information fills the title block
```

Everything is generated from the plan on demand, so the documents always match the model.

## 11.2 Schedules

Three things are called schedules in Plan Studio: the four **schedule windows** (Tools > Schedules), the
**schedules you place in the plan** as live tables (the Schedule flyout), and the **Framing Takeoff** (11.11).

### Schedule windows: Tools > Schedules

Chief's Tools > Schedules submenu lists 12 schedule types. The four windows build their table for the active floor.

| Menu item | Columns | Today |
|---|---|---|
| Door Schedule | Number, Floor, Width, Height, Type, Wall, Swing (and more) | Works. |
| Window Schedule | Number, Width, Height, Sill, Head, Type, Wall | Works. |
| Room Schedule | Number, Name, Area (sq ft), Perimeter (ft), Ceiling height | Works. |
| Wall Schedule | Number, Type, Length, Thickness, Height, Area (sq ft), Openings | Works. |
| Framing Takeoff | Item, Qty | Works, see 11.11. |
| Note, Room Finish | | (planned) |

Each opens a window with a scrollable table and an **Export CSV...** button that asks for a file name (default
`Door_Schedule.csv` and so on). The tables are live: reopen the window after editing the plan. Schedule numbers follow
creation order in these windows.

Since Round 8 (QA-03, `docs/qa-findings.md`) the Room Schedule's "Area sq ft" is the **Interior Area**, the same number as the room label on the plan and the Room Specification, and its
"Ceiling height" column reads a room's own override when it has one (QA-02). Each row also carries the centerline-based **Standard Area**; its column is in the schedule's column list, hidden until you tick it.

### Placing a schedule in the plan: the Schedule flyout

The **Schedule** flyout on row 2 (and Tools > Schedules > **Place on Plan**) starts the Schedule tool in one flavor per kind:

| Flyout entry | Lists | Callout labels in the plan |
|---|---|---|
| Door Schedule | Doors | `D01`, `D02` ... |
| Window Schedule | Windows | `W01` ... |
| Room Schedule | Rooms | |
| Cabinet Schedule | Cabinets | `C-01` ... |
| Electrical Schedule | Electrical devices | |
| Framing Schedule | Framing members | |
| Plant Schedule | Placed symbols of the Plants category | |
| Fixture Schedule | Placed fixtures (plumbing, bath and kitchen, lighting) | `F-01` ... |
| Furniture Schedule | Placed furniture | |
| Create Schedule | Everything placed, narrowed by a filter you type | |

(The Wall Schedule is not in the flyout; it stays a window.) The tool works like this:

| Gesture | Result |
|---|---|
| Click empty plan | Places the schedule with its upper-left corner at the snapped point and selects it. One undo step. A ghost table follows the pointer. |
| Click an existing schedule | Selects it. |
| Drag a schedule | Moves it. |
| Double-click | Opens the **Schedule Specification**. |
| `Delete` or `Backspace` | Deletes the selected schedule. |

A placed schedule is a table drawn on the `Schedules` layer in the text style `Schedule Style`, rebuilt from the plan every
time the plan is drawn, so it is always current. Its definition (kind, columns, sort, filter, style, labels, position) is saved with the floor in the typed
`schedules` slot (chapter 12.2) and is undone with the plan. Objects are numbered by floor, then in reading order across the plan
(x, then y of the object's center); a mark is the prefix plus a two-digit number. A door or window whose own schedule number is set shows that instead.

**Callout labels.** While a schedule of a labelled kind (door, window, cabinet or fixture) with **Show schedule number labels in the plan** on is on the floor,
each listed object gets its label (`D01`, `W03`, `C-12`, `F-01`) drawn beside it in the plan, 9" off a wall face for doors and windows. Numbers are counted over the
whole plan, so a schedule that lists one floor still shows the right marks.

#### Dialog: Schedule Specification

| Tab | Fields |
|---|---|
| General | **Title** (empty uses the kind's name). **Schedule type**. **Floors**: This floor only or All floors. **Filter**: keep only rows with a cell containing the text (any case). **Columns**: a table of Show, Heading and Field, with up and down buttons to reorder, and at least one column must stay shown. **Sort by** a column or "(order in plan)", with Descending. **Output**: **Export CSV...** and **Open in Window**. |
| Labels | Show schedule number labels in the plan; **Numbering** By floor (restart at 01 on each floor) or Whole plan; **Prefix** (with a Default button); a list of the callouts by kind. |
| Text Style | The table's text style, from the plan's text styles. |
| Layer | The layer the schedule is drawn on. |

Columns each kind can show (the first group is shown in a new schedule, the rest is hidden until you tick it):

| Kind | Shown | Hidden |
|---|---|---|
| Door | Mark, Floor, Width, Height, Type, Wall, Swing | |
| Window | Mark, Width, Height, Sill, Head, Type, Wall | Floor |
| Room | Number, Name, Area sq ft (interior), Perimeter ft, Ceiling height | Standard Area, Floor Finish, Ceiling Finish, Floor |
| Wall | Number, Type, Length, Thickness, Height, Area sq ft, Openings | Floor |
| Cabinet | Mark, Label, Type, Width, Depth, Height | Elevation, Countertop, Floor |
| Electrical | Mark, Type, Label, Mount Height, Circuit | Wall, Floor |
| Framing | Mark, Member, Size, Length, Qty | Floor |
| Fixture, Furniture, Plant | Mark, Name, Category, Width, Depth, Height | Elevation, Floor |
| General (Create Schedule) | Mark, Category, Name, Size, Floor | |

**Selecting a schedule.** A placed schedule is a normal selectable object (Round 8, `ObjectRef::Schedule`): Select Objects picks it by clicking its table or by a marquee,
it moves when you drag it or move a selection that includes it, `Delete` removes it (one undo step, "Delete Schedule"), and a double-click, `Enter` or
Open Object opens the Schedule Specification. Its layer is the schedule's own layer. The Schedule tool shares the same selection.

Limits (`docs/integration-queue.md`): there is no grouping or totals row, no click-a-row-selects-the-object, and Room Finish and Note schedules are not separate kinds. The tables are drawn in the plan only: they are not in the DXF export or the
construction set PDF, and a layout box cannot show one yet. Cabinets and fixtures are listed from the plan's placed objects; the Cabinet Schedule columns come from the cabinet specification.

## 11.3 The layout view

Chief's layout is a separate view of the project where plan views, elevations and sections are arranged on sheets at an architectural scale.
In Plan Studio it fills the main area in place of the plan, and it is saved with the plan. The model is the `plan-layout` engine (11.6); the
view is `shell/layout_window.rs` with the dialogs in `dialogs/layout.rs`.

### Making and showing the layout

- **File > New Layout** makes the plan's layout and shows it: a **Page Template** (not printed; its boxes and CAD repeat on every page, and the border and Daniel's 18 x 24
  title block are drawn on every page) and an empty **Page 1**. The sheet comes from your layout template (1.7.1), else ARCH C (18 x 24). "New layout: page template and page 1" appears in the
  status bar. **A plan has one layout**: if it already has one, File > New Layout opens it ("This plan already has a layout; opened it").
- **File > Open Layout...**, **Window > Layout** and the Project Browser's **Open Layout** button show the layout (making it from the template if the plan has none). **Window > Floor Plan View** returns to the plan.
- The layout is stored in the plan file (`layout`, chapter 12.2), so it is saved and opened with the `.psplan`. Plan and layout share **one undo stack** (Round 8): Edit > Undo, `Cmd+Z` and the layout toolbar's Undo step back through plan and layout edits in the order you made them,
  whichever view is showing. Each step keeps its name ("Move Layout Box", "Send to Layout", "Insert Page", "New Layout" ...). A single stack was chosen over Chief's one per view so that undoing in the plan can never silently roll back
  a layout edit, or the other way round. The history holds 100 steps.

### The window

The layout view has a toolbar across the top, the sheet in the middle and **page tabs** along the bottom.

| Part | What it does |
|---|---|
| Toolbar | Plan (back to the floor plan), Send to Layout, Box Specification, Delete Box, Page Before, Page After, Duplicate, Delete Page, the two Exchange arrows, Page Table, Update Views, Page Setup, Project Info, Undo, Redo, Fit, Print, and the zoom as a percentage. Hover for the tooltip. |
| Page tabs | One tab per page, written `A-1  Page 1`. The template page reads `Page Template (template)` in italics. Click to open a page; double-click to rename it; the `+` tab adds a page after the last. Right-click a tab for Insert Page Before / After, Duplicate Page, Exchange With Previous / Next Page and Delete Page. |
| Sheet | The page drawn on a dark surround. Scroll or pinch zooms at the pointer; dragging empty space pans. |

Pages are numbered `A-1`, `A-2` ... in order, and renumber when you insert, delete or exchange pages. A layout keeps at least one printed page ("A layout keeps at least one page").
Duplicate Page copies the page and its boxes ("<title> copy").

### Boxes

A **layout box** shows one source at a scale inside a rectangle on the page. The view draws it live and redraws it when the plan changes; **Layout > Update Layout Views** forces a redraw.
You make boxes with Send to Layout (below); a layout from an older file may also hold text, image and CAD-detail boxes, which the view shows and the Source tab edits.

| Gesture | Result |
|---|---|
| Click a box | Selects it. Clicking empty sheet clears the selection. |
| Drag a box | Moves it, snapping to 1/16" (hold `Alt` for free movement). One undo step ("Move Layout Box"). |
| Drag a handle | The selected box has eight handles (corners and edge midpoints). Dragging one resizes the box (smallest side 1/4"). Resizing changes the box, not its scale; the scale is a Box Specification setting. |
| Arrow keys | Nudge the selected box 1/16"; `Shift` nudges 1/4". |
| Double-click a box | Opens the Layout Box Specification. |
| `Delete`, `Backspace` | Delete the selected box. `Esc` clears the selection or a placement in progress. |

### Send to Layout

**Send to Layout** (File menu and Layout menu, row 1 button, or `S, L`) opens a dialog:

| Field | Choices |
|---|---|
| View | A plan view: **Floor plan** (any floor) and **Layer set**. Or, when a 3D view of an elevation or section camera is open, that **Camera view**. |
| Page | An existing page, or **New page** (after the last). Starts on the page you are looking at. |
| Scale | **Largest that fits** (the largest scale that fits the drawing area, up to 1/4" = 1'-0"; a 40' x 30' plan is 1/4" on Arch D and 1/8" on Letter), or any scale in the list. |
| Position | **First free area** of the drawing area, **Centered**, or **Click on page**: a ghost box follows the pointer and the next click on the page places it. |

**Send All Floors to Layout** (Layout menu) makes one page per floor, each titled like its plan (`FIRST FLOOR PLAN`), at the largest scale that fits. The plan view it sends honors the layer set; "All" ignores layer visibility.
In a vector elevation or section 3D view (10.7) the panel's **Send to Layout** button sends that camera, and its **Layout PDF...** button saves the layout as a PDF.

### Dialog: Layout Box Specification

Opened by double-click or Layout > Layout Box Specification....

| Tab | Fields |
|---|---|
| General | **Label** (the caption; empty for none), **Scale**, **Page** (moves the box to another page), **Left / Bottom** and **Width / Height** in paper inches, **Rotation** (shown disabled: the layout model has no box rotation), **Draw border**, **Clip content to the box**. |
| Source | A plan view: **Floor plan** and **Layer set** ("All" ignores layer visibility). A text box: the text and its **Text height** in points. A camera box: the camera. Other sources show their name. |
| Line Style | **Line weight scaling** (0.1 to 5 times), **Material hatches (elevations)**. Pen colors, weights and dashes come from each layer. |

OK is refused with a reason for a box with no size.

### Dialog: Page Setup

Layout > Page Setup... (or the toolbar and the Project Browser). **Sheet size** (the list of Arch, ANSI and ISO sheets), **Orientation** (landscape only; shown disabled, "Sheets are landscape in the layout model"),
**Margins**, **Layout background (warm off-white)**, **Edge line weight** (in 1/100 mm; the border of every page), **Sheet index on the first page**. Changing the sheet size also moves the plan's Drawing Sheet outline (1.4).

### Dialog: Layout Page Table

Layout > Layout Page Table.... A row per page with its sheet number (`A-n`), an editable **Title** (the `%sheet.title%` of the title block) and a **Page Template** check box.
A template page is not printed, and its boxes repeat on every page.

### Print Layout and Export Layout PDF

File > Print > **Print Layout...**, Layout > Print Layout..., the toolbar Print button and the Project Browser's Print... open the **Print Layout** dialog: **All pages**, or **Pages** `from` `to` (1-based among the printed pages;
the dialog refuses a range outside the layout). Then a file dialog asks where to save the PDF (default `<layout name>.pdf`); the status bar reports "Saved <path>" or "Print cancelled". **Export Layout PDF...**
(File > Print and the Layout menu) asks for the file straight away and writes every printed page. The template page never prints. Neither command talks to a printer: print the PDF with your system's tools (11.7).

## 11.4 Project Information and the title block

**Tools > Project Information...** (also Layout > Project Information... and the layout toolbar's Project Info) is its own action (`Action::ProjectInfo`; it used to be a tool) and edits the plan's `Project.info`, the values the layout's title block prints. OK stores them as one undo step ("Project Information").

| Tab | Fields |
|---|---|
| Client | Name, Address (several lines), Phone, Email |
| Project | Project number, Project address, Date (with a **Today** button), Current revision |
| Designer | Designer, Company, Drawn by, Checked by (a "Drawn by" entry fills the title block's DRAWN BY box instead of the designer) |
| Revisions | A table of No., Date and Description with **Add Revision** (the next number, today's date, and it becomes the current revision) and a remove button per row. Blank rows are dropped. |
| Custom Fields | Name and Value pairs; **Add Field** |

The layout fills these into the title block and any text in a layout: `%project.name%` (the plan's name), `%project.number%`, `%client%`, `%address%` (the project address, else the client's address on one line), `%designer%`, `%date%`,
`%date.long%` (`October 7, 2026`), `%revision%`, and the per-sheet `%sheet.number%`, `%sheet.title%`, `%scale%` and `%page.count%`. The REVISIONS table of Daniel's block prints the latest five rows of the Revisions tab.
The dialog also defines `%client.phone%`, `%client.email%`, `%client.address%`, `%company%`, `%drawn.by%`, `%checked.by%`, `%project.address%` and `%custom.<name>%`; the layout does not expand those yet, so they print as typed (the title block uses
only the macros in the first list). Unknown `%...%` text stays as written.

## 11.5 The materials list and the construction set

### Tools > Materials List...

Opens a table of Category, Item, Quantity and Unit for the active floor, with **Export CSV...**.
The take-off is deliberately simple:

- **Studs** at 16" on center: `ceil(length / 16) + 1` per wall, plus 2 kings and 2 trimmers for
  every opening. Exterior walls are 2x6, interior walls 2x4.
- **Plates**: a bottom plate plus a doubled top plate (wall length x 3, in linear feet).
- **Drywall** in 4x8 sheets (32 sq ft): net wall area (openings deducted), both sides of
  interior walls and the inside of exterior walls.
- **Sheathing** (sheets) and **siding** (sq ft) from net exterior wall area.
- **Flooring** (sq ft) and **ceiling drywall** (sheets): one line per room.
- **Doors and windows** counted by size.

No waste factor, no stock-length rounding, no prices. Chief's Components-based materials list,
with unit costs, markup and waste, is (planned).

### Tools > Schedules > Create Construction Set...

One click writes a multi-page PDF set. A file dialog asks where (default
`<Project Name> Construction Set.pdf`).

The set is on **18 x 24 in (Arch C) landscape** sheets, in Daniel's title block (11.8) on the page
background color, numbered `A-0`, `A-1`, ... in order:

1. A cover with the project title and the sheet index.
2. One floor plan per floor.
3. Elevations: Front and Back, then Left and Right, two per sheet.
4. One longitudinal section through the middle of the building.
5. Door, window and room schedules.
6. A framing plan placeholder ("FRAMING PLAN TO BE DEVELOPED").

**Scale is automatic.** Each plan, elevation and section is sent at the largest scale that fits its sheet and is
no bigger than 1/4" = 1'-0": the candidates are 1/4", 3/16", 1/8", 1" = 10' and 1" = 20'. A 40' x 30' plan is
1/4" on Arch D and 1/8" on Letter. Every box has a bold caption in Chief's style
(`FIRST FLOOR PLAN`, `SECOND FLOOR PLAN` when the project has several floors; `1ST FLOOR PLAN` for a single floor) and a
`SCALE: 1/4" = 1'-0"` line under it. Elevations and the section come from the hidden-line drawings of
`plan-elevation` (chapter 10.7), with material hatches in the elevation and section boxes. Plans draw each layer in its
**color, line weight and line style** (dashed, dotted and dash-dot layers print as PDF dashes); elevations use 0.7, 0.35
and 0.18 pt for Heavy, Medium and Light, cut lines 1.0 pt, and hidden lines dashed. Cut regions of a section are filled with a
gray poche, and shadow regions (when a drawing has them) with a lighter gray, under the lines.

Limits of the PDF writer: the only font is Helvetica; an image box that points at a file prints a placeholder frame (PNG and
JPEG files are never decoded), and raster pixel data embeds flattened on white with no transparency; the REVISIONS table
draws only in the right-strip title block; clip rectangles cut anything in a box, rotated text included.

The construction set is its own generator: it is built from the plan each time and does not read the plan's layout (11.3). It prints the
project name and the Project Information (11.4) in the title block.

## 11.6 The layout engine

`plan-layout` is Chief's Layout, headless: a layout is a list of pages, each a set of
**layout boxes** showing a plan, elevation, section, schedule, CAD detail, image or text at an
architectural scale. Paper units are inches, origin bottom-left.

- `Layout`, `LayoutPage` and `LayoutBox`: the serde data model. A box has a source, a scale and a
  position. Every newer field has a serde default, so older layout JSON loads.
- **Page background** (`page_background`, on by default): Chief's layout background color (249, 248, 244) is
  filled first on every page. **Layout Edge** (`edge_line_weight`, 18 in hundredths of a millimeter) is the weight of
  the page border.
- **Template page** (`template_page`): Chief's "Page Template" page. It is not printed; its boxes and CAD (text may use macros)
  repeat on every other page. The title block and border are drawn on every page regardless.
- **Box options**: `hatch_materials` (on by default) draws material hatches in elevation and section boxes.
  A box can show embedded image pixels (`ImageData`, fitted and centered in the box) or a placeholder for a file path.
- **Title block macros**: `%project.name%`, `%project.number%`, `%client%`, `%address%`, `%designer%`, `%date%`,
  `%date.long%` (for example `October 7, 2026`), `%revision%`, `%sheet.number%`, `%sheet.title%`, `%scale%` and
  `%page.count%`. Unknown `%...%` text stays as written. The per-sheet values (number, title, scale, page count) are filled
  in for each page; the project name, client, address, job number, date, designer, revision and the REVISIONS table come
  from Project Information (11.4), for the layout view and for the construction set.
- `send_to_layout`: sizes a box to its source at a scale and shelf-packs it into the first free area of a page.
  `send_to_layout_auto` with no scale picks the largest scale that fits, up to 1/4" (11.5). Pass 3" as the ceiling for the
  pure largest fit.
- `render_pdf` and `render_box_lines`: draw the pages into a PDF through `plan-docs`; each clipped box gets one PDF clip
  rectangle. Bold labels (title-block labels, schedule headings, room names, box captions) are bold; vertical dimension text and
  CAD text with an angle are rotated.
- `default_construction_set`: the set in 11.4.

The layout view (11.3) is the editor for this model. Plan Studio stores one layout per plan, its boxes cannot be rotated, and the
editor creates plan-view and camera boxes (text, image, CAD-detail and schedule boxes exist in the engine but have no way to be created in the editor yet).

### Material hatch limits

The hatch in layout boxes is built from the 3D scene, not from the elevation's outline loops:

- Only faces that point at the camera and lie in a plane parallel to the view get a pattern, so vertical wall faces are hatched;
  sloped roofs, gables and chimneys cut at an angle are not.
- The mesh material picks the pattern: exterior wall and siding get lap siding, brick gets brick, stone gets block, stucco and
  concrete a stipple. Interior walls, trim, glass and roofs are not hatched.
- Occlusion is not tested: a wall face hidden behind a nearer porch or wing still shows its hatch. Section boxes hatch the faces
  beyond the cut only.
- Courses start at each face's lower-left corner, so bond lines do not line up between walls; patterns coarsen below 1/32" on
  paper; at most 40,000 hatch strokes per view.

## 11.7 Printing and PDF output

| Item | Today |
|---|---|
| File > Print > Print Layout..., Layout > Print Layout..., the layout toolbar's Print | Works as a PDF: choose all pages or a range, then a file name (11.3). Nothing is sent to a printer. |
| File > Print > Export Layout PDF... | Works: every printed page of the layout. |
| Row 1 Print button (`Cmd+P`) | (planned) dimmed |
| Drawing Sheet (`Alt+F3`) | Works. Outlines the active layout's sheet, centered on the walls of the active floor, with a caption such as `ARCH D (24 x 36)  1/4" = 1'-0"`. |
| Print Preview (`Alt+F2`) | Works as a screen preview of the plan: it draws the same outline and grays out everything outside the sheet. |
| Sheet size and scale | Project Browser > Layout > Active layout sheet: a size list (Arch D 24 x 36, Arch C 18 x 24, Letter, Tabloid and the other Arch, ANSI and ISO sheets) and a scale list (1/2", 1/4", 3/16", 1/8" = 1'-0" and the other architectural and metric scales). The default is Arch D at 1/4". Once the plan has a layout, the outline follows the layout's sheet size (Page Setup). |
| PDF of a plan view | Send to Layout, then Export Layout PDF. |
| Create Construction Set (PDF) | Works, see 11.5 |

`plan_docs::plan_sheet` is a scaled floor-plan sheet with a title block on ArchD (24 x 36),
ArchC (18 x 24), Letter or Tabloid paper at 1/2", 1/4", 3/16" or 1/8" = 1'-0". If the plan does not
fit, it steps down to the next smaller scale and reports the scale actually used. The editor
does not call it. The same PDF writer (`PdfDoc`, PDF 1.4) draws lines, polylines, filled
polygons, rectangles, arcs and text with the standard fonts.

## 11.8 Daniel's layout template, line weights and fills

Daniel's default layout template is `18x24 PRESENTATION LAYOUT TEMPLATE.layout`: sheet size
ARCH C (18" x 24"), 6 layer sets, 7 text styles and 11 title-block macros. The layout engine's
presentation title strip is modeled on it, and `plan-chiefplan` inventories it (names and
decoded layer colors and line weights; see `docs/daniel-template-inventory.md`). File > New Layout takes its
sheet size from your default layout template (1.7.1: the listed sheet name, or the `18x24` of the file name; ARCH C when none is found). The template's pages and boxes are not loaded and the file
is read-only to the program and never copied.

The title block the construction set uses is **Daniel's 18 x 24**: a 2.5" strip down the right edge with PROJECT, CLIENT,
ADDRESS, SHEET TITLE, SHEET NO., DATE, SCALE and DRAWN BY, plus a **REVISIONS** table of five rows (the latest rows of the
revision list, drawn in the right strip only). Chief's layout template stores no page or box objects the reader could decode (the sheet size
comes from the file name), so this block is modeled by hand, not read from the file.

### Line weights and fills

Layer line weights are stored in hundredths of a millimeter (a wall layer is 0.50 mm; a dimension
0.18 mm). The View > Line Weights toggle scales the on-screen stroke widths by each layer's weight:
0.25 mm draws at the base width, and the factor is held between 0.5 and 4 times. Text is not scaled.
The construction-set PDF and the layout PDF apply the layer weights, colors and line styles to plan views (11.5) and the Layout Edge weight to the
page border. Fill patterns and hatches (`plan-materials::pattern_strokes`:
brick, block, shingle, lap siding, tile, herringbone, insulation, concrete, earth, grass) are used by the CAD Hatch tool and the Fill Style
tab (chapter 5.4), by Wall Hatching (chapter 17) and by the elevation hatch (chapter 10.7).

## 11.9 Differences from Chief

- **One layout per plan.** Chief keeps several layout files; here the plan holds one layout (File > New Layout opens it again). Pages are landscape only.
- **No box rotation.** The Layout Box Specification shows Rotation disabled.
- **Boxes come from Send to Layout.** The editor makes plan-view and elevation / section camera boxes. There is no way yet to add a text, image, CAD-detail or schedule box,
  or to draw CAD or text on a page.
- **Print writes a PDF.** There is no printer dialog; the PDF is the output, along with DXF (chapter 12) and CSV.
- **Schedules.** Ten kinds can be placed in the plan; Wall stays a window; Note and Room Finish are not separate kinds; no grouping, totals, or click-a-row-selects-the-object; they are not in the DXF or the construction set.
- **Title block.** Project Information fills the title block; the extra macros it defines (`%client.phone%`, `%company%`, `%custom.<name>%` ...) are not expanded by the layout yet.
- Materials list is a quantity list, not a priced estimate.

## 11.10 A worked example: a first document set

With a finished first-floor plan open:

1. Run **Tools > Checks > Plan Check**. Step through the findings with Next; fix the ones that matter (a bedroom
   without egress, a hall under 36"), press Check Again, then **Save Report...** to keep a Markdown record.
2. Run **Tools > Checks > Plan Footprint** to add the outline and the building footprint area to the plan.
3. Use **Auto Exterior Dimensions** (`Shift+A`) and add room names in the Room Specification so the room
   schedule and labels read correctly.
4. Pick **Door Schedule** and **Window Schedule** from the Schedule flyout and click in the plan to place each table next to the house; the doors and windows get `D01` and
   `W01` callouts. Open **Tools > Schedules > Door Schedule** if you want the CSV for a contractor or a spreadsheet.
5. Open **Tools > Materials List...** for a rough quantity check and export it.
6. Choose **Tools > Project Information...** and fill the client, address, job number and date.
7. Choose **File > New Layout**. Press `S, L` to Send to Layout: pick the floor plan, leave the scale at "Largest that fits" and place it. For the elevations, make
   them with **3D > Create Auto Elevations > Auto Elevations**, open one in a 3D view (Vector View) and press its Send to Layout button. Drag boxes into place, add pages with the `+` tab, and
   rename them in the Layout Page Table.
8. Choose **Layout > Export Layout PDF...**, name the PDF, and open it. Check the scale and the title block on each sheet.
9. For the automatic set instead (cover, plans, elevations, section, schedules), choose **Tools > Schedules > Create Construction Set...**.
10. Save the plan with `Cmd+S` so the `.psplan` (which holds the layout) and the PDFs stay together.

The PDF is regenerated from the model every time, so after a change just run step 8 (or 9) again. In the layout view the boxes redraw as the plan changes; Update Layout Views forces it.

## 11.11 Framing and the Framing Takeoff

Plan Studio builds framing from the plan the way Chief's Build Framing does, and also lets you place framing by hand,
using the `plan-framing` library. The commands are in the **Build > Framing** menu and in three flyouts on row 2 of the
toolbar: **General Framing**, **Floor/Ceiling Framing** and **Roof Framing**.

| Command | Hotkey | What it does |
|---|---|---|
| Build Framing | `Shift+Cmd+S` | Frames the active floor (below). One undo step. |
| Build All Framing | | The same for every floor. One undo step. |
| Delete Framing | | Removes the active floor's built framing (members placed by hand and the layout lines stay). One undo step. With none the status bar says "There is no framing to delete". |

What Build Framing makes on a floor:

- **Walls**: every wall except Invisible, room divider and railing walls, with its openings: plates, studs at
  16" on center, king and trimmer studs, headers, cripples and sills. A **Framing Reference Marker** within 24" of a wall puts the
  first stud on the marker.
- **Floor platforms**: one per detected room (joists across the short side, rim and blocking), except on a
  foundation floor and for rooms under 1 sq ft. A room that holds a **Joist Direction** line, a **Bearing Line** or a
  Reference Marker is framed from them instead: joists run perpendicular to the direction line, and a bearing line splits the joists and
  adds a beam.
- **Roof**: the roof planes stored on the floor (chapter 8) are framed with rafters, ridge, hips, valleys and trusses.
- **Trusses**: each **Truss Base** is filled with trusses (a Roof Truss Direction line sets how they run).

Building again replaces the floor's earlier built framing. The members are stored on the floor (in its `framing` slot), so they are saved
with the plan and undone with it. The status bar reports "Built framing: n wall, n floor and n roof members". Built members are drawn on the
`Framing` layer (created and turned on if missing), each as its plan outline shaded in that layer's color.

### The manual framing tools (19)

All 19 entries work. A **lumber member** is drawn start to end: press at the start and drag to the end, or click the start and then the end
(`Esc` cancels); both ends snap like walls. A **post** and the **Framing Reference Marker** take one click. A **Truss Base** is a closed
polyline: click the corners, then `Enter`, a double-click or a click on the first corner closes it. Members shorter than 3" are not made.

| Flyout | Tool | What it makes | Layer |
|---|---|---|---|
| General Framing | General Framing, Blocking | A lumber member drawn start to end | `Framing` |
| | Post, Post with Footing | A vertical post (4x4 by default); with a footing, a pad centered under it (the dialog sets its size and thickness) | `Framing, Posts` |
| | Framing Reference Marker | A marker that anchors the first stud of nearby walls (a layout object, no lumber) | `Framing` |
| Floor/Ceiling Framing | Joist (2x10 at 16" o.c. by default), Joist Blocking | A single joist, drawn under the subfloor; joist blocking | `Framing, Floor Joists` |
| | Joist Direction | A line (no lumber) that steers Build Framing: joists run perpendicular to it | `Framing, Floor Joists` |
| | Bearing Line | A line (no lumber) where joists break and a beam is added | `Framing, Floor Joists` |
| | Floor/Ceiling Beam (4x10 by default) | A beam | `Framing, Beams` |
| | Floor/Ceiling Truss | A truss across the span | `Framing, Trusses` |
| Roof Framing | Rafter (2x8 at 24" o.c. by default), Roof Blocking, Roof Purlin | Single roof members | `Framing, Rafters` |
| | Roof Beam | A beam | `Framing, Beams` |
| | Roof Truss, Girder Truss | A truss drawn along its span (Fink, Howe, king post, scissor, attic or mono; a 24' 6:12 Fink has 9 members; a girder has plies) | `Framing, Trusses` |
| | Roof Truss Direction | A line (no lumber) that sets the truss run | `Framing, Trusses` |
| | Truss Base | The closed outline Build Framing fills with trusses | `Framing, Trusses` |

(The default lumber sizes come from `plan-framing`. The five `Framing, ...` layers are added to the plan the first time they are needed; chapter 5.5.)

Editing and moving framing:

- **Select Objects picks placed framing** (members, posts, layout lines, markers and truss bases): click, `Shift`+click, `Tab` and a marquee all
  work, a selected member shows handles, and `Delete` removes it. A line member and a layout line (Joist Direction, Bearing Line, Roof Truss
  Direction) have an **end handle** at each end that stretches it; a Truss Base has a **corner handle** on every corner; posts and markers move by
  their body. Dragging the body moves the selection (one undo step, "Move Framing"). Double-click or `Enter` opens the **Framing Member
  Specification** (a Framing Reference Marker has no dialog).
- With a framing tool active you can still pick by holding `Shift` or `Cmd` and clicking (`Shift` on a picked one drops it from the selection),
  drag to move, and double-click a member to open its dialog; `Delete` or `Backspace` removes the picked objects or the one under the pointer.

#### Dialog: Framing Member Specification

| Tab | Fields |
|---|---|
| General | **Type** (14 member types), **Lumber Size** (presets and the member's own), **Material** (Lumber, Steel, Glulam, LVL, PSL), **Plies** (1 to 6), **Label** (the cut-list label). **Dimensions**: Length (plan), or Height for a post, or Span for a truss; Rise (end - start) and the read-only Cut length; Bottom elevation; Rotation (roll about the member's own axis). A Post with Footing adds **Footing size** and **Footing thickness**. |
| Truss | Only for truss types: Truss Type (Fink, Howe, King Post, Scissor, Attic, Mono), Span (the member's length), Pitch (rise per 12), Heel height, Overhang, Plies, and a note of how many chords and webs the truss gets. |
| Line Style | Line style and weight, shown disabled: they follow the member's layer. |
| Layer | The layer the member is drawn on. |

Manual members show in the 3D view as boxes (chords and webs for a truss, a footing under a post); the members Build Framing makes from the walls, floors
and roof are drawn in plan only. The DXF export adds the manual framing (12.3).

### Tools > Schedules > Framing Takeoff...

Opens the **Framing Takeoff** window, a lumber list of the framing built so far plus the members placed by hand.

- Two radios choose "<floor name> only" or "All floors".
- The table has two columns, **Item** (a cut line such as a stud length or a joist) and **Qty**, followed by a
  **Total <size> (linear ft)** row for each lumber size and **Total board feet** (nominal sizes for sawn lumber, actual sizes for engineered).
- The footer shows the member count and two buttons. **Export CSV...** (default file name `framing_takeoff.csv`) writes the table as an `Item,Qty` file.
  **Export Material List...** (default `framing_material_list.csv`) writes the piece counts by size and length.
  With no framing the window says "No framing yet. Use Build > Framing > Build Framing."

Not built yet: a framing defaults dialog (the library's built-in spacing, plates, header plies and joist size are used), corner and T backing,
combined headers for adjacent openings, wall-top slopes, connectors and notches, rim joists in directed floor framing, truss-to-truss girder placement, and beam sizing.
