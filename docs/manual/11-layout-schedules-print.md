# Chapter 11: Layout, Schedules and Printing

This chapter covers turning the model into documents: schedules (as windows and as tables placed in the plan),
the layout view where plan views, elevations and sections are arranged on sheets, Project Information and the
title block, the Framing Takeoff, the materials list, the Create Construction Set PDF, and the printing path. It
is honest about what is still missing: there is no CAD-detail box in the Send to Layout dialog, and the schedule tables are not in the DXF (11.9).

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

Chief's Tools > Schedules submenu lists 12 schedule types. The four windows build their table for the active floor. **Click a row** of a schedule window to select its object in the plan: the program goes to its floor, centers the plan on it and selects it (doors, windows, walls, cabinets, devices, placed symbols, stairs, notes and rooms; a grouped row selects its first object; a totals row and the terrain's plants select nothing).

| Menu item | Columns | Today |
|---|---|---|
| Door Schedule | Number, Floor, Width, Height, Type, Wall, Swing (and more) | Works. |
| Window Schedule | Number, Width, Height, Sill, Head, Type, Wall | Works. |
| Room Schedule | Number, Name, Area (sq ft), Perimeter (ft), Ceiling height | Works. |
| Wall Schedule | Number, Type, Length, Thickness, Height, Area (sq ft), Openings | Works. |
| Framing Takeoff | Item, Qty | Works, see 11.11. |
| Stair, Room Finish, Note | | Placed in the plan only (the Schedule flyout below), not as windows. |

Each opens a window with a scrollable table and an **Export CSV...** button that asks for a file name (default
`Door_Schedule.csv` and so on) and, since Round 13, an **Export Excel...** button beside it that writes the same table as an `.xlsx` workbook (`Door_Schedule.xlsx`; the program writes the file itself, no Excel is needed to make it). The Schedule Specification (below) and the Materials List have the same two buttons. The tables are live: reopen the window after editing the plan. Schedule numbers follow
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
| Stair Schedule | Stairs and ramps (landings are not listed) | |
| Room Finish Schedule | Rooms with their floor, wall, base, crown and ceiling finishes | |
| Note Schedule | The numbered notes of the plan (`Note 3: ...`, `E 1: ...`) | |
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
| General | **Title** (empty uses the kind's name). **Schedule type**. **Floors**: This floor only or All floors. **Filter**: keep only rows with a cell containing the text (any case). **Columns**: a table of Show, Heading and Field, with up and down buttons to reorder, and at least one column must stay shown. **Sort by** a column or "(order in plan)", with Descending. **Group by** a column: rows with equal values in the grouping column and the other shown columns are counted together in one line (marks read `first-last (n)`, with `*` where the members differ). **Totals line**: a last line with the number of objects and the sums of the area and perimeter columns. **Output**: **Export CSV...**, **Open in Window** and **Send to Layout** (a schedule box on the last page of the layout). |
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
| Cabinet | Mark, Label, Type, Width, Depth, Height | Elevation, Countertop, Floor, Door Style, Drawer Style, Finish, Hardware |
| Electrical | Mark, Type, Count, Label, Mount Height, Circuit | Wall, Floor |
| Framing | Mark, Member, Size, Length, Qty | Linear ft, Board ft, Floor |
| Fixture, Furniture, Plant | Mark, Name, Category, Width, Depth, Height | Elevation, Floor |
| Stair | Mark, Type, Width, Total rise, Risers, Riser height, Tread depth, Total run | Floor |
| Room Finish | Number, Name, Floor Finish, Base, Wall Finish, Ceiling Finish | Crown, Area sq ft, Ceiling height, Floor |
| Note | No., Type, Note | Floor |
| General (Create Schedule) | Mark, Category, Name, Size, Floor | |

The Electrical schedule gained a Count column and the Framing schedule the Linear ft and Board ft columns in Round 12; a schedule saved in the plan earlier keeps its old column list until the column list is reset.

**Selecting a schedule.** A placed schedule is a normal selectable object (Round 8, `ObjectRef::Schedule`): Select Objects picks it by clicking its table or by a marquee,
it moves when you drag it or move a selection that includes it, `Delete` removes it (one undo step, "Delete Schedule"), and a double-click, `Enter` or
Open Object opens the Schedule Specification. Its layer is the schedule's own layer. The Schedule tool shares the same selection.

A placed schedule can also be shown in the layout as a live table box (the Schedule Specification's Send to Layout button; the box follows the plan). The Plant Schedule lists the plants of the terrain's landscape runs, one row per plant (on the first floor; those rows select nothing).

Limits (`docs/integration-queue.md`): the tables are drawn in the plan and in layout boxes only: they are not in the DXF export or in the construction set PDF's plan sheets (the construction set has its own door, window and room schedule boxes), and the flyout has no icon of its own for the newer kinds. Cabinets and fixtures are listed from the plan's placed objects; the Cabinet Schedule columns come from the cabinet specification.

## 11.3 The layout view

Chief's layout is a separate view of the project where plan views, elevations and sections are arranged on sheets at an architectural scale.
In Plan Studio it fills the main area in place of the plan, and it is saved with the plan. The model is the `plan-layout` engine (11.6); the
view is `shell/layout_window.rs` with the dialogs in `dialogs/layout.rs`.

### Making and showing the layout

- **File > New Layout** makes the plan's layout and shows it: a **Page Template** (not printed; its boxes and CAD repeat on every page, and the border and Daniel's 18 x 24
  title block are drawn on every page) and an empty **Page 1**. The sheet comes from your layout template (1.7.1), else ARCH C (18 x 24), or a layout you saved as the default for that sheet size (**Layout > Save As Template**, below). "New layout: page template and page 1" appears in the
  status bar. If the plan already has a layout, File > New Layout opens it ("This plan already has a layout; opened it"); **Layout > New Layout File...** makes another.
- **File > Open Layout...**, **Window > Layout** and the Project Browser's **Open Layout** button show the layout (making it from the template if the plan has none). **Window > Floor Plan View** returns to the plan.
- **More than one layout file (Round 14).** **Layout > New Layout File...** asks for a name and makes a second layout in the same plan and opens it; the other layout files are kept in the plan. The **Layout file** list on the window's third toolbar row (below) switches between them. File > New Layout still opens the plan's first layout when it has one.
- The layout is stored in the plan file (`layout`, chapter 12.2), so it is saved and opened with the `.psplan`. Plan and layout share **one undo stack** (Round 8): Edit > Undo, `Cmd+Z` and the layout toolbar's Undo step back through plan and layout edits in the order you made them,
  whichever view is showing. Each step keeps its name ("Move Layout Box", "Send to Layout", "Insert Page", "New Layout" ...). A single stack was chosen over Chief's one per view so that undoing in the plan can never silently roll back
  a layout edit, or the other way round. The history holds 100 steps.

### The window

The layout view has a toolbar across the top, the sheet in the middle and **page tabs** along the bottom.

| Part | What it does |
|---|---|
| Toolbar | Plan (back to the floor plan), Send to Layout, Box Specification, Delete Box, Page Before, Page After, Duplicate, Delete Page, the two Exchange arrows, Page Table, Update Views, Page Setup, Project Info, Undo, Redo, Fit, Print, and the zoom as a percentage. Hover for the tooltip. |
| Third row (Round 14) | **Layout file** (a list of the plan's layout files) and **New Layout File...**, **Page Specification...**, **Sheet Sizes...**, **Align** (Left, Center, Right, Top, Middle, Bottom), **Spread H** and **Spread V**, **Copy to Page...**, **Open Source View**, **Duplicate Box**, and **Export CSV...** / **Export Excel...** for the table boxes (below). The row scrolls sideways. |
| Tool row | A second row: the drawing **Tool** (Select, Line, Box, Polyline, Circle, Arc, Text, Text Box, Leader, Revision Cloud), then **Add Text Box**, **Add Materials List**, **Add Picture**, **Add Sheet Index**, **Layers...**, **Construction Set**, **Rotate**, and the **Print...**, **Print Image...** and **Print Model...** buttons (11.3, Other boxes and page drawings). The row scrolls sideways when the window is narrow. |
| Page tabs | One tab per page, written `A-1  Page 1`. The template page reads `Page Template (template)` in italics. Click to open a page; double-click to rename it; the `+` tab adds a page after the last. Right-click a tab for Insert Page Before / After, Duplicate Page, Exchange With Previous / Next Page and Delete Page. |
| Sheet | The page drawn on a dark surround. Scroll or pinch zooms at the pointer; dragging empty space pans. |

Pages are numbered `A-1`, `A-2` ... in order, and renumber when you insert, delete or exchange pages; the numbering starts from the lowest sheet number, so moving a cover page that is sheet 0 keeps it sheet 0, and the sheet index follows. A layout keeps at least one printed page ("A layout keeps at least one page").
Duplicate Page copies the page and its boxes ("<title> copy").

### Boxes

A **layout box** shows one source at a scale inside a rectangle on the page. The view draws it live and redraws it when the plan changes; **Layout > Update Layout Views** forces a redraw.
You make view boxes with Send to Layout (below) and the other kinds with the tool row (Other boxes and page drawings, below).

| Gesture | Result |
|---|---|
| Click a box | Selects it. Clicking empty sheet clears the selection. |
| Drag a box | Moves it, snapping to 1/16" (hold `Alt` for free movement). One undo step ("Move Layout Box"). |
| Drag a handle | The selected box has eight handles (corners and edge midpoints). Dragging one resizes the box (smallest side 1/4"). Resizing changes the box, not its scale; the scale is a Box Specification setting. |
| Click the **rotate knob** | A small circle on a stem above the top edge of the selected box turns the box's content a quarter turn counter-clockwise (0, 90, 180, 270 degrees), the box keeping its place; the Rotate button and the Box Specification's Rotation list do the same. One undo step ("Rotate Layout Box"). A rotated box is picked by its turned content. |
| Arrow keys | Nudge the selected box 1/16"; `Shift` nudges 1/4". |
| Double-click a box | Opens the Layout Box Specification. |
| `Delete`, `Backspace` | Delete the selected box. `Esc` clears the selection or a placement in progress. |

**Arranging boxes and pages (Round 14).** The Layout menu and the third toolbar row hold:

- **Align Layout Boxes** (Left, Center, Right, Top, Middle, Bottom): lines the selected boxes up on that edge or center; a single box lines up with the drawing area. **Spread Horizontally** and **Spread Vertically** (the Layout menu's Align Layout Boxes list, and **Spread H** and **Spread V** on the toolbar) give three or more boxes equal gaps. Shift-click selects several boxes; dragging one of a group moves the others with it.
- **Copy Layout Box to Page...** asks for the page and copies the selected boxes there; **Duplicate Layout Box** copies them on the same page.
- **Open Source View** (one plan box selected): returns to the floor plan that box shows. A camera box tells you to open the camera from the Project Browser's cameras list.
- **Page Specification...** edits the current page's **Title**, **Sheet number** (`A-n`; two pages cannot share one), the **Page Template** flag, **No border or title block on this page**, and its own **Sheet size** and Landscape or Portrait. A page with its own sheet size prints at that size and its boxes pack into it.
- **Customize Sheet Sizes...** (the toolbar's **Sheet Sizes...**): **Daniel's sizes (ARCH)** shows the ARCH sizes only with 18 x 24 first, **Show all** restores the list, each standard size has a check box, and **Add size** makes a named custom size (width and height in inches). The layout's own sheet always stays in the list. The lists in Page Setup and Page Specification follow.
- **Save As Template...** asks for a name and stores the layout (pages, template page, boxes and page drawings; boxes of cameras, perspective views and placed schedules are left out) in `~/.plan-studio/templates/<name>.layout.json`. Tick **Start new layouts of this sheet size from it** to make it the default for that sheet size. **Apply Template...** lists the saved templates (name, sheet, page count, "default") and replaces the layout with the one you pick.
- **Export Table as CSV...** and **Export Table to Excel...** (also the toolbar's Export CSV... and Export Excel...) save the selected table box, or every table on the page when none is selected (the sheet index, a Materials List, a placed schedule), as CSV or as an `.xlsx` workbook with a sheet each.

### Send to Layout

**Send to Layout** (File menu and Layout menu, row 1 button, or `S, L`) opens a dialog:

| Field | Choices |
|---|---|
| View | A plan view: **Floor plan** (any floor) and **Layer set**. Or, when a 3D view of an elevation or section camera is open, that **Camera view**. Or, when a perspective camera (Full Camera, 10.2) is the open view, that **Perspective view** (ray traced, below). |
| Page | An existing page, or **New page** (after the last). Starts on the page you are looking at. |
| Scale | **Largest that fits** (the largest scale that fits the drawing area, up to 1/4" = 1'-0"; a 40' x 30' plan is 1/4" on Arch D and 1/8" on Letter), or any scale in the list. |
| Position | **First free area** of the drawing area, **Centered**, or **Click on page**: a ghost box follows the pointer and the next click on the page places it. |

**Send All Floors to Layout** (Layout menu) makes one page per floor, each titled like its plan (`FIRST FLOOR PLAN`), at the largest scale that fits. The plan view it sends honors the layer set; "All" ignores layer visibility.
In a vector elevation or section 3D view (10.7) the panel's **Send to Layout** button sends that camera, and its **Layout PDF...** button saves the layout as a PDF.

### Dialog: Layout Box Specification

Opened by double-click or Layout > Layout Box Specification....

| Tab | Fields |
|---|---|
| General | **Label** (the caption; empty for none), **Scale**, **Page** (moves the box to another page), **Left / Bottom** and **Width / Height** in paper inches, **Rotation** (0, 90, 180 or 270 degrees), **Draw border**, **Clip content to the box**. |
| Source | A plan view: **Floor plan** and **Layer set** ("All" ignores layer visibility). A text box: the text, its **Text height** in points, **Alignment** (Left, Center, Right), **Text fit** (Wrap, Shrink to fit, As typed) and **Bold**. A camera box: the camera. A perspective box: the camera, a **Resolution** in dots per paper inch (20 to 600; 80 is the default) and a **Quality** in samples per pixel (1 to 512; 8 is the default), with a line saying how many pixels it renders and that Update Views renders it again. A Materials List box: the **Floors** (All floors or one) and **Category** (All categories or one of the eleven). Other sources show their name. |
| Line Style | **Line weight scaling** (0.1 to 5 times), **Material hatches (elevations)**. Pen colors, weights and dashes come from each layer. |

OK is refused with a reason for a box with no size.

### Dialog: Page Setup

Layout > Page Setup... (or the toolbar and the Project Browser). **Sheet size** (the list of Arch, ANSI and ISO sheets), **Orientation** (Landscape or Portrait),
**Margins**, **Layout background (warm off-white)**, **Edge line weight** (in 1/100 mm; the border of every page), **Sheet index on the first page**. Changing the sheet size also moves the plan's Drawing Sheet outline (1.4).

### Dialog: Layout Page Table

Layout > Layout Page Table.... A row per page with its sheet number (`A-n`), an editable **Title** (the `%sheet.title%` of the title block) and a **Page Template** check box.
A template page is not printed, and its boxes repeat on every page.

### Other boxes and page drawings

The tool row of the layout view makes the boxes and drawings that are not views of the plan:

- **Text boxes.** **Add Text Box** puts a box with the word "Text" on the page and opens its dialog; the **Text Box** tool draws one by dragging its rectangle. Double-click a text box to edit it in the
  **Text Box** dialog: the text (several lines), the **Text height** in points (2 to 200), **Alignment** (Left, Center, Right), **Text fit** and **Bold**. Text fit is **Wrap** (the default: the text wraps at the box width and what does not fit the
  box height is clipped), **Shrink to fit** (wraps, then makes the type smaller, down to 4 pt, until all of it fits) or **As typed** (one line per line break, no wrapping, so a long line can run past the box and a clipping box cuts it off). Text boxes start without a border.
- **Perspective boxes.** Send to Layout with a perspective camera open puts a ray-traced picture of that camera on the page. It is rendered at the box's size at 80 dots per paper inch by default
  (a 6" x 4.5" box is 480 x 360 pixels) with 8 samples per pixel, or at the **Resolution** and **Quality** you set in the Layout Box Specification (above), using the default clear-day sun and sky and the plan's point lights (Round 13; Print Model is lit the same way), then embedded. The picture is cached while the plan and the camera are unchanged;
  **Update Layout Views** renders it again, and printing renders any that are out of date. The box shows a placeholder frame until a render exists. **Update Views runs on a background thread** with a progress bar in the toolbar ("Views 2/5"), so the window stays usable while perspective boxes render.
  Right-click a camera in the Project Browser and choose **Send to Layout...** to open the Send to Layout dialog already pointed at that camera.
- **Picture boxes.** **Add Picture** asks for a picture file and puts it on the page, fitted in the box and centered. PNG and JPEG (baseline and progressive) files are decoded by the shared image decoder (10.8a), shrunk to a size that prints well and cached until the file changes; a file that cannot be read or decoded prints a frame with its name. The picture is read from its path, so it must still be there when you print.
- **Materials List boxes.** **Add Materials List** puts the Materials List (11.5) on the page as a table box for all categories and all floors (the Box Specification's Source tab narrows it to one category or one floor). It follows the plan and is priced from the Master List, like a placed schedule.
  The materials window's **Send to Layout** button makes the same box. The sheet index of the first page is a table box too, kept up to date as pages change.
- **Page drawings (layout CAD).** The **Line**, **Box** (rectangle) and **Polyline** tools draw on the page in paper inches (drag, drag, and click the corners then double-click); the **Circle** tool drags a circle out from its center; the **Arc** tool takes three clicks, the center, the start and the end (counter-clockwise from the start); the **Text** tool asks for a line of text and its height and places it. Page drawings are
  picked, moved with the Select tool and deleted with `Delete`; the lines, circles and arcs are drawn on the `Layout CAD` layer and the text on the `Text` layer. Double-click page text to edit it; page text may use the macros of 11.4 (`%sheet.number%` ...).
- **Leaders.** The **Leader** tool drags from what the leader points at to where its text goes (a straight leader), or, since Round 13, you **click the tip, click each bend, and double-click where the text goes** (`Enter` also ends it) for a leader with bends. Either way it then asks for the text (several lines), the **Text height** (0.04" to 2"; 1/8" by default) and whether it has an **Arrowhead**. A leader is a line through its bends to an elbow, then a landing line under the text, with a filled arrowhead at the tip. Double-click a leader to edit it. Its text is on the `Text` layer.
- **Revision clouds.** The **Revision Cloud** tool drags the rectangle to go around and asks for the **Revision** mark (`1`, `A` ...) drawn in a triangle at the cloud's corner (empty: no tag). The cloud is a run of scalloped bumps (0.3" wide) with a smallest side of 0.3". Double-click a cloud to change its mark. Clouds are on the `Revision Clouds` layer. A cloud's mark that has no row in Project Information's Revisions table gets one when the title block is filled (Round 13): a description such as "Revision cloud on A-2, A-5" lists the pages that carry the mark, so the REVISIONS table of the title block shows every cloud.
- **Selecting and editing page drawings.** Every page drawing, leader and cloud is picked by clicking its line, moves when you drag it, nudges with the arrow keys (`Shift` for the larger step) and resizes by the eight handles of its bounding rectangle (a circle's radius, an arc's, a text's height and a cloud's rectangle follow); each is one undo step ("Move Layout Drawing", "Resize Layout Drawing").
- **Plan Check report page.** The Plan Check window's **Add to Layout** button (chapter 18.4) adds a page named **Plan Check** after the last page, holding the findings as one text box: a numbered paragraph per finding with its severity, code reference, place, message and fix. It is an ordinary text box once added; edit or restyle it like any other.
- **Sheet index.** **Add Sheet Index** puts the index of the layout's printed sheets on the page as a table box that stays current as pages are added, renamed and exchanged.
- **Layer Display Options.** The toolbar's **Layers...** button opens the *Layout Layer Display Options*: the layout's own five layers, each with a **Show** box, a **Line weight** (0.05 to 6 pt) and a **Color**: `Layout Box Borders` (0.75 pt), `Layout CAD` (0.5 pt), `Text` (0.5 pt), `Title Block` (0.75 pt, the default pens of the title block scale with it) and `Revision Clouds` (1 pt). The window and the printed page follow them; a hidden layer does not print. One undo step, "Layout Layer Display".

### Print

File > **Print...** (also the row 1 Print button, `Cmd+P`, Layout > Print Layout..., the layout toolbar's Print and the Project Browser's Print...) opens the **Print** dialog for the layout when its view is open, else for the active floor of the plan.
The Print Layout and Export Layout PDF commands in the File > Print and Layout menus are shortcuts to the same PDF writer (Export asks for the file at once and writes every printed page).

| Section | Choices |
|---|---|
| Destination | **PDF file** (a file dialog asks where; default `<layout name>.pdf`), **System printer** (CUPS `lp` on macOS and Linux, with Copies and a **Printer** list read from `lpstat -p`: the default printer, then every printer found, or "No printers found"; Windows saves the PDF instead) or **Open in viewer** (a temporary PDF in Preview, the system viewer or `start`). |
| Paper | Size (the Arch, ANSI and ISO list, or Custom size in inches), Orientation (landscape or portrait) and Margin (the unprintable border; default 1/4"). |
| Scale | A layout: Fit to page, 100% (actual size) or Percentage. A plan view: Fit to page, 1:1, any drawing scale from the Scale list, or a Custom ratio `1 : n`. **Tile onto several pages** cuts a sheet bigger than the paper into tiles with an Overlap (default 1/2"); the summary line says how many tiles per sheet and how many paper pages. |
| Appearance | Color, Grayscale or Black and white, and **Print line weights** (off: every line is a 0.5 pt hairline). |
| Print range | A layout: All pages or Pages `from` to `to` among the printed pages (the template page never prints). |
| Perspective views | A layout: render perspective boxes at the dialog's **DPI** and **samples** instead of each box's own (0 keeps each box's setting). |

**Print Preview** in the dialog sets the plan's Drawing Sheet to the chosen paper size (and, for a plan view, the print scale) and turns on View > Drawing Sheet and Print Preview, so the plan shows what the sheet will cover (11.7). Since Round 13 the preview also shows the dialog's **Appearance**: with Grayscale or Black and white chosen, the plan on screen is drawn in grays or in black on white, and the sheet's caption names the mode. **File > Print > Print Image...** saves the active floor's plan lines as a PNG (256 to 8000 pixels wide, fitted or at a drawing scale; for a rendering use Ray Trace > Save PNG).
Every printed sheet of a layout PDF gets a **bookmark** (`A-1 Page 1`), so a PDF viewer lists the sheets. The settings are remembered while the program runs. The status bar reports "Saved <path>" or "Print cancelled".

**Print Model...** (the layout toolbar's tool row; `LayoutCommand::PrintModel`) prints one perspective camera big: a dialog asks for the **Camera** (the plan's perspective cameras, the one in the 3D view first), a **Resolution** (20 to 600 dpi; 150 by default) and a **Quality** (1 to 512 samples per pixel; 16 by default), the paper, orientation and margin, and the destination (PDF, printer, viewer), and says how many pixels it renders. The camera is ray traced onto one sheet. "Print Model needs a perspective camera: add one with the Camera tools" if there is none.

**Print Image of the 3D view.** With a 3D view open, **File > Print > Print Image...** opens a size dialog (64 to 4096 pixels each way, 24 samples, starting at the view's aspect ratio) and ray traces the picture from the viewport's camera with the default sun and sky, because the live view has no offscreen target to read back (`print_image_3d`, `Image3dDialog`); you then save the PNG. With the floor plan showing, the command saves the plan view as a PNG as before. **Print Model...** is also in the File > Print menu, and **Layer Display Options...** and **Add Sheet Index** are rows of the Layout menu (as well as buttons of the layout window's toolbar).

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
The dialog also fills `%client.phone%`, `%client.email%`, `%client.address%`, `%company%`, `%drawn.by%`, `%checked.by%`, `%project.address%` and `%custom.<name>%` (a custom field's name, spaces trimmed); the layout expands them in text as it does the first list (Daniel's title block uses
only the macros in the first list). Unknown `%...%` text stays as written.

## 11.5 The materials list and the construction set

### Tools > Materials List...

Opens the **Materials List** window with two tabs. The **Materials List** tab lists the active floor or all floors (radio buttons), narrowed to one category or all, in Chief's columns:
**ID** (for example `FRM-003`), **Size** (`2x6 x 16'`, `3'-0" x 6'-8"`), **Description**, **Count**, **Unit**, **Unit Price** and **Price**, with the total. Its buttons are **Export CSV...**
(Category, ID, Description, Size, Count, Unit, Unit Price, Price), **Export Excel...** (`materials_list.xlsx`, the same columns), **Export PDF...** and **Send to Layout** (a table box on the current layout page, 11.3).

The take-off reads the plan model and is grouped in eleven **categories**, in this order:

| Category | What is counted |
|---|---|
| Foundation | Slab, pad and pier concrete, in cubic yards |
| Framing | Where a floor has framing members, their cut list rounded up to stock lengths (8, 10, 12, 14, 16 ft) and the board feet; otherwise a wall formula: studs at 16" on center (`ceil(length / 16) + 1` per wall, plus 2 kings and 2 trimmers for every opening) and plates (a bottom plate and a doubled top plate). Exterior walls are 2x6, interior walls 2x4. Wall sheathing sheets |
| Roofing | The stored roof planes' sloped area in square feet and in squares, shingle bundles (3 per square), drip edge along the eaves and gutters where a plane has them |
| Siding | Net exterior wall area, openings deducted |
| Windows, Doors | Counted by size |
| Cabinets | Counted by label and size, and the countertop area |
| Electrical | Devices by type |
| Fixtures | Placed fixture symbols |
| Interior Finishes | Drywall in 4x8 sheets (32 sq ft) and flooring, by room |
| Landscaping | The terrain's plants |

**Master List.** The second tab edits the **Master List**, kept in `~/.plan-studio/master-list.json`: a **waste factor** per category (default 10% for Framing, Roofing, Siding and Interior Finishes, 5% for Foundation, none elsewhere; counts round up to whole units),
the **stock lengths** of lumber, and a **unit price** and **supplier** for each item that appears in the list. **Save** writes the file. A row with no price shows none, and the list's total counts only priced rows.
The Materials List boxes in the layout and the construction set are priced and wasted from the same file.

Not built: Chief's Components-based list (it reads the plan model directly), markup and labor, and a price import.

### Tools > Schedules > Create Construction Set...

One click does two things. First it **adds Daniel's sheet set to the plan's layout** (making the layout first when the plan has none), so the sheets are pages you can edit, reorder and reprint: Cover (the project title, "CONSTRUCTION DOCUMENTS" and the sheet index as a live table), Site Plan, one plan per floor, Elevations (Front and Back, then Left and Right), Building Section, Details (a placeholder sheet for typical details), Schedules (door, window and room), a Materials List sheet when the plan has materials and a framing plan placeholder: **ten sheets for a one-floor plan** with materials. Pages that already have content stay and the new sheets are numbered after them; an empty first page is removed. The layout's own sheet size, title block and margins are used. The same set is the layout toolbar's **Construction Set** button. It is one undo step ("Create Construction Set"); the status bar says "Added the construction set: n sheet(s)" and the layout view opens at the cover. Then a file dialog asks where to save a multi-page **PDF copy** (default
`<Project Name> Construction Set.pdf`); cancelling leaves the sheets in the layout and says "no PDF saved". The PDF below is built by the older, separate generator and is the one described from here on.

The set is on **18 x 24 in (Arch C) landscape** sheets, in Daniel's title block (11.8) on the page
background color, numbered `A-0`, `A-1`, ... in order:

1. A cover with the project title and the sheet index.
2. One floor plan per floor.
3. Elevations: Front and Back, then Left and Right, two per sheet.
4. One longitudinal section through the middle of the building.
5. Door, window and room schedules.
6. A **Materials List** sheet with one table per category that has rows, priced and wasted from the Master List (left out when the plan has no materials).
7. A framing plan placeholder ("FRAMING PLAN TO BE DEVELOPED").

A one-floor plan with materials makes eight sheets (`A-0` to `A-7`): cover, floor plan, two elevation sheets, section, schedules, materials list, framing placeholder.

**Scale is automatic.** Each plan, elevation and section is sent at the largest scale that fits its sheet and is
no bigger than 1/4" = 1'-0": the candidates are 1/4", 3/16", 1/8", 1" = 10' and 1" = 20'. A 40' x 30' plan is
1/4" on Arch D and 1/8" on Letter. Every box has a bold caption in Chief's style
(`FIRST FLOOR PLAN`, `SECOND FLOOR PLAN` when the project has several floors; `1ST FLOOR PLAN` for a single floor) and a
`SCALE: 1/4" = 1'-0"` line under it. Elevations and the section come from the hidden-line drawings of
`plan-elevation` (chapter 10.7), with material hatches in the elevation and section boxes. Plans draw each layer in its
**color, line weight and line style** (dashed, dotted and dash-dot layers print as PDF dashes); elevations use 0.7, 0.35
and 0.18 pt for Heavy, Medium and Light, cut lines 1.0 pt, and hidden lines dashed. Cut regions of a section are filled with a
gray poche, and shadow regions (when a drawing has them) with a lighter gray, under the lines.

Limits of the PDF writer: an image box that points at a file prints a placeholder frame unless the program supplies a picture loader (printing from the editor does, for PNG and JPEG; the standalone generator does not), and raster pixel data embeds flattened on white with no transparency; the REVISIONS table
draws only in the right-strip title block; text-style text is embedded in the installed TrueType font when the font's licence allows it (12.6), but a font with PostScript outlines (most `.otf` files), a font that forbids embedding and any character outside Latin-1 print in Helvetica or as `?`; layout page CAD text, leaders and the title block are always Helvetica; clip rectangles cut anything in a box, rotated text included.

The construction set PDF is its own generator: it is built from the plan each time and does not read the plan's layout (11.3), so a PDF saved from this command shows the eight standard sheets even if you have since edited the layout's copies (print the layout for those). It prints the
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
- `default_construction_set`: the set in 11.5 (with the master list: `default_construction_set_with`).
- **Box sources** (`BoxSource`): plan view, elevation, section, camera drawing, schedule, placed schedule, CAD detail, image file, embedded image pixels, **text** (aligned, bold, fit to the box), **perspective** (a ray-traced camera view, rendered by a hook the application sets), **Materials List** and the **sheet index**.
  A box has a rotation in quarter turns, and a perspective box a DPI and sample count of its own (80 and 8 by default).
- **Page CAD** (`LayoutPage::cad`) in paper inches on the `Layout CAD` and `Text` layers (lines, circles, arcs, polylines and text); page **leaders** (`PageLeader`) and **revision clouds** (`RevisionCloud`), all in the `annot` module with one shared id space so the window can select, move, resize and delete any of them the same way; and `LayoutLayers`, the layout's own layer table (Layout Box Borders, Layout CAD, Text, Title Block and Revision Clouds, each with display, color and weight). The drawing code, the PDF and the window honor the layers; the tools and dialogs for all of it are in the layout window (11.3).
- `textfit` (`TextFit`: Wrap, Shrink, Off): the lines a text box shows and the type size they are set in.
- `print_model_pdf`: one perspective camera rendered at a chosen DPI and sample count onto one sheet (Print Model, 11.3).
- `append_construction_set`: adds Daniel's sheet set (10 sheets for a one-floor plan) to a live layout (11.5).
- `print_layout_pdf` and `print_plan_view_pdf` (`print.rs`): the Print dialog's engine. A sheet is scaled (fit, 100%, a percentage; a plan view at a drawing scale or ratio), centered on the paper or cut into overlapping tiles, drawn in color, grayscale or black and white, with or without line weights, one PDF bookmark per printed sheet.

The layout view (11.3) is the editor for this model. Plan Studio stores the open layout in `Project.layout` and any other layout files of the plan beside it; the editor creates plan-view, camera, perspective, text, picture and Materials List boxes and page drawings (CAD-detail boxes exist in the engine but have no way to be created in the editor yet).

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
| File > Print > Print... (the row 1 Print button, `Cmd+P`), Print Layout..., the layout toolbar's Print | The Print dialog (11.3): PDF file, system printer (CUPS `lp`; Windows saves the PDF) or open in the viewer, with paper, scale, tiling, color, line weights and page range. |
| File > Print > Export Layout PDF... | Works: every printed page of the layout. |
| Drawing Sheet (`Alt+F3`) | Works. Outlines the active layout's sheet, centered on the walls of the active floor, with a caption such as `ARCH D (24 x 36)  1/4" = 1'-0"`. |
| Print Preview (`Alt+F2`) | Works as a screen preview of the plan: it draws the same outline and grays out everything outside the sheet. The Print dialog's Print Preview button sets the sheet to the chosen paper and scale and turns it on. |
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

- **Layout files.** A plan holds more than one layout file (Layout > New Layout File...), but the Project Browser lists only the open layout's pages, Plan Check checks the open layout only and Export / Import Layout JSON handle the open layout only. A layout is landscape or portrait as a whole (Page Setup); a page can take its own sheet size in Page Specification.
- **Box rotation is in quarter turns** (the knob, the Rotate button, the Rotation list), not free angles.
- **Boxes.** The editor makes plan-view, camera, perspective, text, picture, Materials List and sheet index boxes, and page lines, rectangles, polylines, circles, arcs, text, leaders and revision clouds. There is no way yet to add a CAD-detail box from the Send to Layout dialog. The layout's page templates are the saved layout templates (11.3) plus the Page Template flag of a page; there is no separate per-sheet-type editor.
- **Plan boxes** draw walls, openings (with their labels), rooms, dimensions and CAD. Cabinet fill styles, the dashed treads under a stairwell and placed symbols and electrical devices show on screen and not yet in plan boxes, Print Preview or the PDF. Dragging a group moves its boxes but not the page drawings (CAD, leaders, clouds) selected with them; perspective boxes in Print Preview show only after Update Views. Layout elevations include the terrain and the site objects of the 3D view.
- The Materials List counts wall components (layer by layer); floor, ceiling and roof components and a concrete main layer in cubic yards are not quantities yet.
- **Printing** goes to a PDF file, the system printer (CUPS `lp`, macOS and Linux only) or the viewer; there is no native print dialog, and copies are the printer's business (a PDF holds one set).
- **Schedules.** Thirteen kinds can be placed in the plan; Wall stays a window; they have grouping and a totals line, but are not in the DXF or the construction set's plan sheets.
- **Title block.** Project Information fills the title block and every Project Information macro is expanded in layout text.
- The Materials List is priced from your own Master List (no prices ship with the program), has no markup or labor, and is not Chief's Components-based list.

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
| Framing Defaults... | | Opens the Framing Defaults window (below). Also under Edit > Default Settings > Framing. |
| Framing Overview | | Switches the plan to the Framing Overview view, or back (below). |

What Build Framing makes on a floor:

- **Walls**: every wall except Invisible, room divider and railing walls, with its openings: plates, studs at
  16" on center, king and trimmer studs, headers, cripples and sills, plus (Round 12) **corner studs** and **tee backing** where walls meet, and
  blocking between the studs (48" apart). The size of a header comes from the **header table**: 2x6 for openings up to 48", 2x8 up to 60", 2x10
  up to 72" and 2x12 beyond, unless a fixed depth is set. A **Framing Reference Marker** within 24" of a wall puts the
  first stud on the marker.
- **Floor platforms**: one per detected room (joists across the short side, rim and blocking), except on a
  foundation floor and for rooms under 1 sq ft. A **stairwell** (a Floor Hole of the platform, which Auto Stairwell makes) is framed as Chief does:
  trimmer joists run the full span on each side of the opening, header joists cross it at each end between the trimmers, and the common joists
  inside the opening are cut short and butt the headers. A room that holds a **Joist Direction** line, a **Bearing Line** or a
  Reference Marker is framed from them instead: joists run perpendicular to the direction line, and a bearing line splits the joists and
  adds a beam.
- **Roof**: the roof planes stored on the floor (chapter 8) are framed with rafters, ridge, hips, valleys and trusses. A rafter gets the **tail cut** of
  its plane's eave (plumb, level or square, from the plane's Eave settings or the Roof framing default) and a **birdsmouth** seat on the top plate (limited to a third
  of the rafter's depth).
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

Framing is in the 3D view: manual members as boxes (chords and webs for a truss, a footing under a post) and the members Build Framing makes from the walls, floors
and roof, one box per piece of lumber (a rafter with its tail cut and birdsmouth). Members on a hidden framing layer are left out, and the scene rebuilds when the
framing changes. The DXF export adds the manual framing (12.3).

#### Framing Defaults

**Build > Framing > Framing Defaults...** (or Edit > Default Settings > Framing > Framing Defaults) edits what Build Framing makes. It has four tabs and is stored with the
plan (in the first floor's framing slot, so it saves and undoes with the plan; OK is one undo step, "Framing Defaults"). Existing framing stays as built until the next Build Framing.

| Tab | Fields |
|---|---|
| Walls | Stud size and spacing (a 2x4 stud becomes 2x6 in a wall 6" or thicker), top and bottom plates (counts), king studs and trimmers per side, cripple spacing, corner studs, tee backing studs per side, blocking between studs and its spacing |
| Headers | The header table (openings up to a width get a size; Add a row, Remove; the last row covers everything wider), plies, and a fixed header depth (0 = use the table) |
| Floor | Joist size and spacing, rim joists, mid-span blocking, and the plies of the header and trimmer joists around a stairwell |
| Roof | Rafter size and spacing, ridge, hip and valley, birdsmouth seat, tail cut (plumb, level, square; a plane's own Eave cut wins), fascia; collar ties, ceiling joists, trusses instead of rafters and their spacing |

#### Framing Overview

**Build > Framing > Framing Overview** (a check mark shows while it is on) adds a saved plan view and layer set both called **Framing Overview** to the plan (once) and switches to
it: the framing layers show and every other layer is hidden, so the plan shows the framing of the active floor and nothing else. It builds the floor's framing first when there is none. In a
section or elevation made while the Overview is on, the wall surfaces are swapped for the wall framing, so the elevation draws studs, plates, headers and cripples. Choosing the command again (or
another plan view tab) leaves the Overview.

### Tools > Schedules > Framing Takeoff...

Opens the **Framing Takeoff** window, a lumber list of the framing built so far plus the members placed by hand.

- Two radios choose "<floor name> only" or "All floors".
- The table has two columns, **Item** (a cut line such as a stud length or a joist) and **Qty**, listed by **member type** (studs, headers, rafters ...), then size, longest cut first, followed by a
  **Total <size> (linear ft)** row for each lumber size and **Total board feet** (nominal sizes for sawn lumber, actual sizes for engineered).
- The footer shows the member count and two buttons. **Export CSV...** (default file name `framing_takeoff.csv`) writes the table as an `Item,Qty` file.
  **Export Material List...** (default `framing_material_list.csv`) writes the piece counts by size and length.
  With no framing the window says "No framing yet. Use Build > Framing > Build Framing."

The Framing schedule that can be placed in the plan (11.2) lists pieces by member type, size and cut length, with Linear ft and Board ft columns available.

Not built yet: combined headers for adjacent openings, framing of the stairwell in a floor framed from a Joist Direction or Bearing Line (directed floors do not frame stairwell holes), a per-group Build Framing dialog or automatic re-framing, dimensions on the wall detail, wall-top slopes, connectors and notches, rim joists in directed floor framing, truss-to-truss girder placement, and beam sizing.

### Excel round trip

Not in Chief; modelled on ArchiCAD's "Exchange Property Data with Excel". Export a schedule to Excel, edit names, marks, manufacturers and your own properties there, and import the workbook back: the plan updates in one undo step.

#### Custom properties

**Tools > Property Manager...** lists the plan's own properties by kind of object: door, window, cabinet, room, wall, fixture / symbol, electrical, stair, roof plane and framing. Pick **New**, choose the kind, give the property a **Name** and a **Type** (Text, Number, Length, Yes / No, or List with one choice per line), an optional **Default**, and **Show in schedules of this kind**, then **Add Property**. Selecting a property in the list lets you **Update** (rename, retype) or **Delete** it; renaming keeps the stored values, and a retype that no longer fits a value clears that value. Each of these is one undo step.

Once a kind has a property, its specification dialog (door, window, wall, room, cabinet, fixture, device, stair, roof plane, framing member) gets a last tab, **Properties**, with one field per property. OK applies the dialog and the Properties tab as one undo step; an entry the type refuses (a word in a Number property, a choice that is not on the list) blocks OK and is shown in red. An object that has no value shows the property's default.

A property is a schedule column. Flag it **Show in schedules** and it is the last column of every schedule of that kind; otherwise open **Schedule Specification** and show its column (it is listed with the other columns as `prop:<name>`). Property columns sort, filter, group and appear in layout schedule boxes like any other.

#### Export for Editing

**Tools > Export Property Data (XLSX)...** writes every schedule placed in the plan (or the standard eight when none is placed: door, window, room, wall, cabinet, electrical, fixture, stair) as one workbook. **Schedule Specification > Export for Editing (XLSX)...** and **Export for Editing** in the right-click menu of a selected schedule export just that schedule. Give the file a `.csv` name to get one schedule as CSV instead.

| Part of the workbook | What it holds |
|---|---|
| One sheet per schedule | A hidden first column **PlanStudio ID** (`door:12`, `room:0:120,84`), a bold frozen header row, the schedule's columns, then the custom properties of the kind the schedule does not show |
| Yellow cells | Editable: door and window Mark, Manufacturer, Model, Supplier, Comment, Description, ID, U-Factor, SHGC; room Name, Floor Finish, Ceiling Finish, Ceiling height; cabinet Label; electrical Label, Circuit, Mount Height; fixture, furniture and plant Name; every custom property |
| Gray cells | Computed (sizes, areas, counts, wall numbers): locked by sheet protection (Review > Unprotect Sheet, no password, if you need to) |
| Drop-downs | List and Yes / No properties |
| `_meta` (hidden sheet) | The plan path, the export time, each sheet's schedule kind and column to field map, and the text of every exported cell |

#### Import Property Data

**Tools > Import Property Data (XLSX)...** (also a button in Schedule Specification and a right-click entry on a schedule) reads a `.xlsx` or `.csv` and opens the **Import Property Data** review:

- Each row of the list is one change: object, field, the value in the plan, the value from the file, with a checkbox. Uncheck what you do not want.
- A value the field's type refuses is shown in red and cannot be checked. A change whose value was also changed in the plan since the export is marked **Conflict** (the file's value wins if you leave it checked).
- **Left out** lists what could not be used: an object deleted since the export, a computed column that was edited (ignored), a row with no id, a column or property that no longer exists.
- Rows are matched by the **PlanStudio ID** column; a row with no id is matched by its mark when exactly one object has it. A CSV (or a workbook without `_meta`) is read by column heading.
- **Import n Changes** applies the checked changes as one undo step named "Import Property Data" and the status bar reports the count. Only cells you changed in Excel count as edits: a mark that renumbered after you deleted a door is not read as an edit.

After an export, the plan checks the workbook every two seconds; when it is saved again in Excel a small bar over the status bar offers **Workbook changed - import?** (Import... or Dismiss). The offer ends when another plan is opened.

Not built yet: copy, paste and duplicate do not carry property values; values of deleted objects stay in the file until a purge; walls, stairs and framing have no built-in editable columns, only custom properties; the framing schedule's grouped lines cannot take properties; an `.xlsx` saved by Excel has been read here only through Excel-style XML tests (verify with a real Excel file).
