# Chapter 11: Layout, Schedules and Printing

This chapter covers turning the model into documents: schedules, the Framing Takeoff, the materials
list, the Create Construction Set PDF, the drawing sheet and Print Preview on the plan, and the layout
system behind it. It is honest about the gap: schedules, framing, materials and the construction set
work today; interactive layout pages and Print do not.

## 11.1 The documentation pipeline

```
   plan model ----> schedules (tables) ---------> CSV files
        |      \--> materials list --------------> CSV file
        |       \-> 3D scene --> elevations/sections (plan-elevation)
        |                              |
        +-------------> plan-layout: pages of layout boxes ---> PDF
                          (cover, plans, elevations, section, schedules)
```

Everything is generated from the plan on demand, so the documents always match the model.

## 11.2 Tools: Tools > Schedules

Chief's Tools > Schedules submenu lists 12 schedule types. Plan Studio builds four, for the
active floor, and a Framing Takeoff (11.11).

| Menu item | Columns | Today |
|---|---|---|
| Door Schedule | Number, Floor, Width, Height, Type, Wall, Swing (and more) | Works. |
| Window Schedule | Number, Width, Height, Sill, Head, Type, Wall | Works. |
| Room Schedule | Number, Name, Area (sq ft), Perimeter (ft), Ceiling height | Works. |
| Wall Schedule | Number, Type, Length, Thickness, Height, Area (sq ft), Openings | Works. |
| Framing Takeoff | Item, Qty | Works, see 11.11. |
| Cabinet, Electrical, Fixture, Furniture, Note, Plant, Room Finish, Custom Schedule | | (planned) |

Each opens a window with a scrollable table and an **Export CSV...** button that asks for a
file name (default `Door_Schedule.csv` and so on). The tables are live: reopen the window after
editing the plan. **Place on Plan** is dimmed ("Placing a schedule on the plan comes with
Layout"). Schedule callouts on doors and windows, editing from the schedule, grouping and
totals are (planned). Schedule numbers follow creation order; the plan labels that would show them
are not drawn yet (chapter 3).

## 11.3 Tools > Materials List...

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

## 11.4 Tools > Schedules > Create Construction Set...

One click writes a multi-page PDF set. A file dialog asks where (default
`<Project Name> Construction Set.pdf`).

The set is on **18 x 24 in (Arch C) landscape** sheets, in Daniel's title block (11.7) on the page
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

## 11.5 The layout system (engine)

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
  in for each page; the editor's construction set supplies the project name only, so the client, address, job number,
  date, designer, revision and the REVISIONS table print blank until the editor has a place to enter them.
- `send_to_layout`: sizes a box to its source at a scale and shelf-packs it into the first free area of a page.
  `send_to_layout_auto` with no scale picks the largest scale that fits, up to 1/4" (11.4). Pass 3" as the ceiling for the
  pure largest fit.
- `render_pdf` and `render_box_lines`: draw the pages into a PDF through `plan-docs`; each clipped box gets one PDF clip
  rectangle. Bold labels (title-block labels, schedule headings, room names, box captions) are bold; vertical dimension text and
  CAD text with an angle are rotated.
- `default_construction_set`: the set in 11.4.

None of this has an interactive editor yet. File > New Layout, Open Layout, Send to Layout (`S, L`),
the Layout page tabs in the Project Browser and layout box editing are (planned). What exists in the
editor is the Project Browser's Layout section: one active layout sheet (size and scale, kept for the
session), which View > Drawing Sheet and Print Preview draw (11.6).

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

## 11.6 Printing

| Item | Today |
|---|---|
| Print button (`Cmd+P`), File > Print | (planned) dimmed |
| Drawing Sheet (`Alt+F3`) | Works. Outlines the active layout's sheet, centered on the walls of the active floor, with a caption such as `ARCH D (24 x 36)  1/4" = 1'-0"`. |
| Print Preview (`Alt+F2`) | Works as a screen preview: it draws the same outline and grays out everything outside the sheet. Nothing is sent to a printer. |
| Sheet size and scale | Project Browser > Layout > Active layout sheet: a size list (Arch D 24 x 36, Arch C 18 x 24, Letter, Tabloid and the other Arch, ANSI and ISO sheets) and a scale list (1/2", 1/4", 3/16", 1/8" = 1'-0" and the other architectural and metric scales). The default is Arch D at 1/4". Kept for the session; the plan file does not store it. |
| Print to PDF of the active plan view | (planned; engine: `plan_docs::plan_sheet`) |
| Create Construction Set (PDF) | Works, see 11.4 |

`plan_docs::plan_sheet` is a scaled floor-plan sheet with a title block on ArchD (24 x 36),
ArchC (18 x 24), Letter or Tabloid paper at 1/2", 1/4", 3/16" or 1/8" = 1'-0". If the plan does not
fit, it steps down to the next smaller scale and reports the scale actually used. The editor
does not call it yet. The same PDF writer (`PdfDoc`, PDF 1.4) draws lines, polylines, filled
polygons, rectangles, arcs and text with the standard fonts.

## 11.7 Daniel's layout template

Daniel's default layout template is `18x24 PRESENTATION LAYOUT TEMPLATE.layout`: sheet size
ARCH C (18" x 24"), 6 layer sets, 7 text styles and 11 title-block macros. The layout engine's
presentation title strip is modeled on it, and `plan-chiefplan` inventories it (names and
decoded layer colors and line weights; see `docs/daniel-template-inventory.md`). Loading a `.layout`
into Plan Studio is not done; the file is read-only to the program and never copied.

The title block the construction set uses is **Daniel's 18 x 24**: a 2.5" strip down the right edge with PROJECT, CLIENT,
ADDRESS, SHEET TITLE, SHEET NO., DATE, SCALE and DRAWN BY, plus a **REVISIONS** table of five rows (the latest rows of the
revision list, drawn in the right strip only). Chief's layout template stores no page or box objects the reader could decode (the sheet size
comes from the file name), so this block is modeled by hand, not read from the file.

## 11.8 Line weights and fills

Layer line weights are stored in hundredths of a millimeter (a wall layer is 0.50 mm; a dimension
0.18 mm). The View > Line Weights toggle scales the on-screen stroke widths by each layer's weight:
0.25 mm draws at the base width, and the factor is held between 0.5 and 4 times. Text is not scaled.
The construction-set PDF applies the layer weights, colors and line styles to plan views (11.4) and the Layout Edge weight to the
page border. Fill patterns and hatches (`plan-materials::pattern_strokes`:
brick, block, shingle, lap siding, tile, herringbone, insulation, concrete, earth, grass) exist in
the engine; the plan does not use them yet.

## 11.9 Differences from Chief

- No interactive layout pages, no layout boxes you can move, no sheet index you can edit, and no place to enter the
  client, address, job number, revision or revisions that the title block macros print.
- No Print. Print Preview only grays out what lies outside the drawing sheet. The document output is the construction set PDF, DXF (chapter 12) and CSV.
- Four of the twelve schedules, plus the Framing Takeoff; no schedule callouts on the plan.
- Materials list is a quantity list, not a priced estimate.

## 11.10 A worked example: a first document set

With a finished first-floor plan open:

1. Run **Tools > Checks > Plan Check**. Step through the findings with Next; fix the ones that matter (a bedroom
   without egress, a hall under 36"), press Check Again, then **Save Report...** to keep a Markdown record.
2. Run **Tools > Checks > Plan Footprint** to add the outline and the building footprint area to the plan.
3. Use **Auto Exterior Dimensions** (`Shift+A`) and add room names in the Room Specification so the room
   schedule and labels read correctly.
4. Open **Tools > Schedules > Door Schedule** and **Window Schedule**; export each to CSV for the contractor
   or a spreadsheet.
5. Open **Tools > Materials List...** for a rough quantity check and export it.
6. Choose **Tools > Schedules > Create Construction Set...**, name the PDF, and open it. Check the scale on
   each sheet; a plan that does not fit at 1/4" steps down automatically.
7. Save the plan with `Cmd+S` so the `.psplan` and the PDF stay together.

The PDF is regenerated from the model every time, so after a change just run step 6 again.

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

- **Pick** a placed framing object with a framing tool by holding `Shift` or `Cmd` and clicking (`Shift` on a picked one drops it from the
  selection); drag to move the selection (one undo step, "Move Framing"); `Delete` or `Backspace` removes the picked objects or the one under
  the pointer. Placed framing is **not yet picked by Select Objects**.
- **Double-click** a member with a framing tool to open the **Framing Member Specification**.

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

Not built yet: Select Objects picking of framing, a framing defaults dialog (the library's built-in spacing, plates, header plies and joist size are used), corner and T backing,
combined headers for adjacent openings, wall-top slopes, connectors and notches, rim joists in directed floor framing, truss-to-truss girder placement, and beam sizing.
