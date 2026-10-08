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

The set is on **18 x 24 in (Arch C) landscape** sheets with the 2.5" presentation title strip,
numbered `A-0`, `A-1`, ... in order:

1. A cover with the project title and the sheet index.
2. One floor plan per floor, at 1/4" = 1'-0".
3. Elevations: Front and Back, then Left and Right, two per sheet.
4. One longitudinal section through the middle of the building.
5. Door, window and room schedules.
6. A framing plan placeholder.

A view that does not fit a sheet at 1/4" steps down to the next smaller scale (3/16" or 1/8").
Only these four scales exist: 1/2", 1/4", 3/16" and 1/8" = 1'-0". Elevations and the section
come from the hidden-line drawings of `plan-elevation` (chapter 10.7). Line weights follow the
drawing's Heavy, Medium, Light and Hidden classes, in paper points.

Limits of the PDF writer: images print a placeholder; text cannot rotate; the only font is Helvetica.

## 11.5 The layout system (engine)

`plan-layout` is Chief's Layout, headless: a layout is a list of pages, each a set of
**layout boxes** showing a plan, elevation, section, schedule, CAD detail, image or text at an
architectural scale. Paper units are inches, origin bottom-left.

- `Layout`, `LayoutPage` and `LayoutBox`: the serde data model. A box has a source, a scale and a
  position.
- `send_to_layout`: sizes a box to its source at a scale and shelf-packs it into the first free area
  of a page.
- `TitleBlockTemplate::presentation_18x24()`: a 2.5" right-hand strip with fields filled by
  macros: `%project.name%`, `%client%`, `%address%`, `%designer%`, `%date%`, `%sheet.number%`,
  `%sheet.title%`, `%scale%`. Unknown `%...%` text stays as written.
- `render_pdf` and `render_box_lines`: draw the pages into a PDF through `plan-docs`; boxes are
  clipped in software.
- `default_construction_set`: the set in 11.4.

None of this has an interactive editor yet. File > New Layout, Open Layout, Send to Layout (`S, L`),
the Layout page tabs in the Project Browser and layout box editing are (planned). What exists in the
editor is the Project Browser's Layout section: one active layout sheet (size and scale, kept for the
session), which View > Drawing Sheet and Print Preview draw (11.6).

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

## 11.8 Line weights and fills

Layer line weights are stored in hundredths of a millimeter (a wall layer is 0.50 mm; a dimension
0.18 mm). The View > Line Weights toggle scales the on-screen stroke widths by each layer's weight:
0.25 mm draws at the base width, and the factor is held between 0.5 and 4 times. Text is not scaled.
The PDF writer applies the weights of its drawings. Fill patterns and hatches (`plan-materials::pattern_strokes`:
brick, block, shingle, lap siding, tile, herringbone, insulation, concrete, earth, grass) exist in
the engine; the plan does not use them yet.

## 11.9 Differences from Chief

- No interactive layout pages, no layout boxes you can move, no sheet index you can edit.
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

Plan Studio builds framing from the plan the way Chief's Build Framing does, using the `plan-framing`
library. The commands are in the **Build > Framing** menu and in the General Framing flyout on row 2 of the
toolbar.

| Command | Hotkey | What it does |
|---|---|---|
| Build Framing | `Shift+Cmd+S` | Frames the active floor (below). One undo step. |
| Build All Framing | | The same for every floor. One undo step. |
| Delete Framing | | Removes the active floor's framing. One undo step. With none the status bar says "There is no framing to delete". |

What Build Framing makes on a floor:

- **Walls**: every wall except Invisible, room divider and railing walls, with its openings: plates, studs at
  16" on center, king and trimmer studs, headers, cripples and sills.
- **Floor platforms**: one per detected room (joists across the short side, rim and blocking), except on a
  foundation floor and for rooms under 1 sq ft.
- **Roof**: the roof planes stored on the floor (chapter 8) are framed with rafters.

Building again replaces the floor's earlier framing. The members are stored on the floor, so they are saved
with the plan and undone with it. The Framing layer is created if it is missing and turned on; each member is
drawn as its plan outline, shaded in that layer's color, whenever the layer is shown. The status bar reports
"Built framing: n wall, n floor and n roof members".

### Tools > Schedules > Framing Takeoff...

Opens the **Framing Takeoff** window, a lumber list of the framing built so far.

- Two radios choose "<floor name> only" or "All floors".
- The table has two columns, **Item** (a cut line such as a stud length or a joist) and **Qty**, followed by a
  **Total <size> (linear ft)** row for each lumber size and **Total board feet**.
- The footer shows the member count and **Export CSV...** (default file name `framing_takeoff.csv`, an
  `Item,Qty` file). With no framing the window says "No framing yet. Use Build > Framing > Build Framing."

Not built yet: the manual framing tools (General Framing, Post, Post with Footing, Blocking, Framing Reference
Marker, Joist, Beam, Rafter, Truss ...) are (planned) and stay dimmed; there is no framing defaults dialog, so
the library's built-in defaults are used; corner and T backing and combined headers are not generated.
