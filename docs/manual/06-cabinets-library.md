# Chapter 6: Cabinets and the Library

This chapter covers the parametric cabinet tools and the Library Browser that
places symbols (fixtures, appliances, furniture, plants, lighting) into the
plan, including the Chief Architect catalogs it reads from your own Chief install.

## 6.1 How cabinets work

A cabinet is a box with a type, a countertop, a toe kick and a **face layout**:
the tree of doors, drawers, separations, openings and appliances on its front,
sides and back. Cabinets are parametric. They are stored on the floor as the JSON
of a `plan_cabinets::Cabinet`, so the plan file stays readable and the engine
(`plan-cabinets`) owns the rules.

```
   Plan view                       Front elevation (face layout)
   +------------------+            +--------------------+
   |   countertop     |            |  drawer            |  Item 2.1
   |  +------------+  |            +--------------------+
   |  | cabinet    |  |            |  door    |  door   |  Item 4 (auto: right)
   |  +------------+  |            |          |         |
   +------------------+            +--------------------+
   back against wall               toe kick
```

- Units are inches. In the cabinet's own frame the back is at y = 0 and the front faces +Y.
- Kinds: Base, Wall, Full Height, Soffit, Shelf, Partition; Base, Wall and Full Height Fillers; Corner Base and Corner Wall cabinets; Blind Base and Wall cabinets; and the
  custom tops Custom Countertop, Custom Backsplash and Custom Counter Hole (6.2).
- Chief defaults in Daniel's template: Base 24" x 24" x 36" high (the 36" includes a
  1 1/2" countertop with a 1" overhang, and a 4" x 3" toe kick); Wall cabinet 24" x 12" x 30"
  with its bottom at 54"; Full Height 24" x 24" x 84". Door style Lincoln Door,
  drawer style Lincoln Flat Panel Drawer, Knob handles.
- Automatic labels follow the industry style: `B24` (base 24"), `B36-SB` (sink base),
  `W3030` (wall cabinet, width then height), `FH2484` (full height), with `SO`, `SH`,
  `PT` for soffit, shelf, partition.
- Countertop merging is Generate Countertop (`G`, 6.2). One engine helper has no editor command yet: `run_along_wall` (a row of cabinets along a wall).

## 6.2 Tools

### Cabinet Tools (row 2, Cabinet flyout; Build > Cabinet)

| Variant | Hotkey | Today |
|---|---|---|
| Base Cabinet | `Shift+T` | Works. |
| Wall Cabinet | `Cmd+T` | Works. |
| Full Height | `Ctrl+Alt+Cmd+X` | Works. |
| Soffit | `T` | Works as a cabinet-like box. |
| Shelf | `Ctrl+Alt+Cmd+Y` | Works. |
| Partition | `Ctrl+Alt+Cmd+Z` | Works. |
| Base Filler | `Ctrl+Alt+Cmd+0` | Works: a click in a gap makes a filler the width of the gap. |
| Wall Filler | `Ctrl+Alt+Cmd+1` | Works. |
| Full Height Filler | `Ctrl+Alt+Cmd+2` | Works. |
| Custom Countertop | `Ctrl+Alt+Cmd+3` | Works: a polygon. |
| Custom Backsplash | `Ctrl+Alt+Cmd+4` | Works: a polygon (a strip). |
| Custom Counter Hole | `Ctrl+Alt+Cmd+5` | Works: a polygon cut out of the countertop it lies in. |
| Corner Base Cabinet, Corner Wall Cabinet | | Works (no hotkey of its own). |
| Blind Base Cabinet, Blind Wall Cabinet | | Works (no hotkey of its own). |

All entries start the one Cabinet tool in that kind, so the flyout choice and the hotkey
select the kind directly. Press `Tab` while the tool is active
to cycle the kind (through all sixteen, in flyout order); the status bar names it ("Base Cabinet: click to place, drag to set the width; Tab
changes the cabinet type").

### Placing and editing

| Gesture | Result |
|---|---|
| Move | A ghost cabinet follows the pointer. |
| Click | Places a cabinet. Within 12" of a wall, the cabinet's back rotates to that wall and sits flush to its face. Away from walls it keeps the tool's angle. |
| Press, drag, release | Sets the width in 3" steps (one cabinet). |
| Place or drag next to another cabinet | It slides to butt against the neighbor and aligns its back line (bumping). |
| `Tab` | Next cabinet kind. |
| `G` | **Generate Countertop**: joins the countertops of touching base cabinets into custom countertops (below). |
| `Esc` | Returns to Select Objects. |

A placed cabinet is selected. Its handles:

- **Move**: dragging keeps the rotation until the cabinet bumps a wall, where it
  re-rotates to it. Hold `Ctrl` to turn the wall rotation off.
- **Resize** at both ends: changes the width in 3" steps; the cabinet grows from the dragged side.
- **Rotate**.

Double-click or press `Enter` opens the Cabinet Specification. The Edit toolbar offers
Open Object, Delete Objects, Copy Selected Objects, Paste in Place and **Reverse Door
Swing**. Select a cabinet with Select Objects too; the Cabinet tool can also pick cabinets.

#### Fillers, corner and blind cabinets

- A **filler** (Base, Wall or Full Height) takes the width of the gap you click into, between a wall and a cabinet or between two cabinets.
- A **corner cabinet** clicked near the inside corner of two walls (within 30") turns to the corner and sits in it with its legs along both walls. In the specification its front is **Diagonal** or **Pie-Cut**
  (a pie-cut can have **Lazy Susan shelves**), with an **Arm Depth**.
- A **blind cabinet** turns its hidden end toward the nearest perpendicular wall (within 30"); the dialog sets the **Hidden end** (left or right) and the **Blind Width**.

#### Custom countertops, backsplashes and counter holes

These three work like the polygon tools of chapter 16: click the corners, or drag a rectangle; `Enter`, a double-click or a click on the first corner finishes; `Backspace` drops the last corner; `Esc` cancels.
A counter hole is cut out of the countertop it lies in. A custom countertop has a thickness and an **Edge Profile** (Square, Beveled or Bullnose, with an edge size); a custom backsplash has a height and a strip thickness.

#### Generate Countertop (`G`)

With the Cabinet tool active, `G` joins the countertops of touching base cabinets into custom countertops: the selected cabinets, or all of them when none is selected. The cabinets give up their own slab (they shrink by its thickness),
and their sink and cooktop holes move to the new top. One undo step ("Generate Countertop"); "No base cabinet countertops to join" if there is nothing to join.

#### Appliance openings, sinks and cooktops

A base cabinet can hold an **open bay for an appliance** (General tab, *Appliance Opening*): Dishwasher, Range, Refrigerator or Microwave. The bay stays open and the appliance fills it in plan and 3D. A countertop can have **sink and cooktop cutouts**
(*Add Sink*, *Add Cooktop*, each with a *Remove* button; their area in square inches is listed).

#### Cabinet Schedule and callouts

A **Cabinet Schedule** from the Schedule flyout lists the plan's cabinets as a live table and, with *Show schedule number labels*, labels each cabinet `C-01`, `C-02` ... in the plan (chapter 11.2).

Not built: cabinet depth and corner resize handles, elevation views of a cabinet run. Cabinets are not in the 3D view at the Round 7 commit (QA-05 in `docs/qa-findings.md`; Round 8 is fixing it).

## 6.3 Dialog: Cabinet Specification

The preview shows the plan symbol and a front elevation of the resolved face items.

| Tab | Status |
|---|---|
| General | Works |
| Box Construction | Works |
| Front/Sides/Back | Works for the Front; the Sides and Back faces are not editable (the Cabinet Side list is disabled and shows Front) |
| Door/Drawer | Works |
| Accessories | Works (mostly disabled controls) |
| Opening Indicators | Works |
| Moldings | Works (crown and light rail) |
| Layer | Works (shows the layer; follows the cabinet's type) |
| Fill Style | (disabled) |
| Materials | Works (per part) |
| Label | Works |
| Components, Object Information, Schedule | (disabled) |

### General

- **Cabinet Style**: Type (shows the kind, disabled); Treat As Filler (checked for the filler kinds, disabled).
- **Size/Position**: Width, Height (including countertop), Depth, Finished Floor to Bottom,
  Finished Floor to Top, Position X and Y (the back-left corner), Angle.
- **Corner Cabinet** (corner kinds): Front Diagonal or Pie-Cut, Lazy Susan shelves, Arm Depth. **Blind Corner** (blind kinds): Hidden end, Blind Width.
- **Appliance Opening** (base cabinets): the open-bay check box and the appliance.
- **Custom Top** (custom kinds): Thickness; Edge Profile and Edge Size (countertop); Height (backsplash).
- **Countertop**: Thickness (1 1/2"), Overhang Front, Back and Sides (1"), Corner Treatment
  None / Clipped / Rounded (only None is available; the others are disabled). **Sink and Cooktop Cutouts**: Add Sink, Add Cooktop, Remove.
- **Backsplash**: Height, Thickness.
- **Toe Kick**: Height (4"), Depth (3").

### Box Construction

- **Box Construction**: Framed (with Separation) or Frameless.
- **Top/Bottom/Sides**: Top Auto / Has Top / No Top; Bottom Auto / Has Bottom / No Bottom;
  Side / Back Thickness.
- **Door/Drawer Overlay**: Traditional Overlay, Full Overlay, Inset.

### Front/Sides/Back

- **Cabinet Side**: the list shows Front and is disabled, and so is Side Type: only the front face is editable, not the Sides or the Back.
- **Front Elevation** (the face editor): a drawing of the resolved face items; drag a divider to resize the items on either side of it.
- **Face Items**: an indented tree such as `Vertical Layout > Separation, Layout > Drawer,
  Separation, Door - Auto Right, Separation`. Buttons: **Add New**, **Delete**, **Move
  Up**, **Move Down**, **Split Vertical**, **Split Horizontal**, **Equalize**, **Reset to
  Default Face**, **Sink Base Face**.
- **Selected Item Properties**: Item Type (Door, Drawer, Separation, Opening, Appliance,
  Horizontal Layout ...), Item Height (0 = auto), Item Width, Lock from Auto-Resize, and an
  Appliance field.
- Splitting an item recomputes its siblings' heights and widths unless one is locked.

### Door/Drawer, Accessories

**Door/Drawer** has: *Door Panel* (Main Style, Panel Profile Slab, Shaker or Raised Panel, Thickness, Stile and Rail Width for framed profiles, Glass Doors); *Door Handle* (Main Style, Vertical Position Centered or Distance From Top,
Distance From Edge); *Door Hinges* (Hidden or Exposed, Up/Down From Edge); *Drawer Panel* (Main Style, Panel Profile, Thickness); *Drawer Handle* (Main Style, Vertical Position Centered or Near the top). The built-in door styles are Lincoln
Door, Slab Door, Shaker Door and Raised Panel Door; the drawer styles are Lincoln Flat Panel Drawer, Slab Drawer, Shaker Drawer and Raised Panel Drawer; library styles are (planned). Handles are None, Knob or Pull.
**Accessories**: Front Pilasters, Feet (Foot Style), Side Panels (Main Panel Style, Full Size Panel); the controls are disabled.

### Opening Indicators, Moldings, Layer, Materials, Label

**Opening Indicators**: the check box *Show door swings and open drawers in plan* draws each door's quarter-circle swing from its hinge and each drawer pulled out in front of the cabinet.
**Moldings** lists up to four profiles, each with a Projection, a Height and a Delete button; **Add Crown** puts a crown molding on top of the cabinet and **Add Light Rail** one under it; both run along the front and return at the ends.
**Layer** shows the layer (Cabinets, Base; Cabinets, Wall ...), which follows the kind. **Materials** gives each part (Box, Door Fronts, Drawer Fronts, Countertop, Backsplash, Toe Kick, Molding) a material from Default, Wood, Painted, Stone, Concrete,
Metal or Glass; Default keeps the part's usual stand-in. **Label** shows the automatic label (`B24`); *Specify label* lets you type your own, with the macros `<W>` width, `<D>` depth, `<H>` height, `<T>` the type letters and `<L>` the automatic label.

## 6.4 The Library Browser

Open it with the Library Browser button on the view bar or `Cmd+L`. It is a dock
(chapter 1.5) with:

- A **search field** with a clear button. Search is case-insensitive; every word must
  match. Ranking, best first: name (whole word, prefix, substring), then tags, then category.
- A **category tree** with item counts. Click a category to filter. Below the built-in tree sit
  the **Chief Architect catalogs** (6.6): the "Use Chief Architect catalogs" check box, a
  "Catalog folders..." button and four collapsed nodes.
- A **result list** of up to 200 rows, each with a small drawing of the item's 2D symbol, its
  name and its size (for example `30 x 28 in`).
- The line "Active item: ..." shows what a click in the plan will place.
- A right-click menu on a built-in result with Open Object and Add to User Library (both "coming").
  Chief rows have a working Open Object (6.6).

### Built-in catalogs

Everything the browser shows ships in `plan-library`. Each item has a 2D plan symbol made of
polylines, arcs and circles (3D models are planned), a default size, a bottom elevation and a
placement rule.

| Catalog | Items | Contents |
|---|---|---|
| Core (starter) | 40+ | Architectural: plumbing, appliances, cabinets, furniture, electrical, exterior |
| Plants | 21 | Deciduous and evergreen trees, palm, ornamental trees, shrubs, hedges, ground cover, flower bed, grasses, boulder, planters |
| Bath and Kitchen | 35 | Toilets, bidet, urinal, 8 sinks, 5 tubs, 4 showers, water heater, washer/dryer, range hoods, cooktops, ovens, refrigerators, wine fridge |
| Lighting and Electrical | 22 | Chandelier, pendant, track, recessed 4" and 6", drum, sconce, under-cabinet, vanity bar, step, lanterns, fans, heat lamp; 240V dryer outlet, USB and floor outlets, data and TV jacks, 200A panel |
| Furniture and Exterior | 27 | Sectional, chaise, recliner, ottoman, bookcase, media console, TV, pianos, crib and beds, bench, island with seating, bar stools; outdoor dining and lounge, grill, fire pit, hot tub, pool, mailbox, bicycle, SUV, pickup |

Placement rules: `wall_mounted` (toilets, tubs, showers, hoods, ovens, refrigerators, beds,
sconces, outlets: origin at the back center, rotates to face away from a wall),
`free_standing` (origin at the center), `ceiling` (centered, at the ceiling) and
`countertop` (cooktops, drop-in sinks: snaps onto the nearest cabinet top).

## 6.5 Library Symbol tool: placing symbols

Clicking a result makes it the active item and switches to the Library tool.

| Gesture | Result |
|---|---|
| Click in the plan | Places a copy of the active item. The tool stays active for repeats. |
| Near a wall | A wall-mounted item rotates to face away from the nearest wall and snaps flush within the auto-rotate distance (6"). |
| Click an existing symbol | Selects it. Handles: Move, Rotate, Resize (width on both sides, depth at the front). |
| Double-click or `Enter` | Opens the Symbol Specification. |
| Edit toolbar | Open Object, Delete Objects, Copy, Paste in Place and **Replace From Library**: pick an item in the Library Browser first, then press the button to swap the selected symbol for it ("Pick an item in the Library Browser, then choose Replace From Library" if none is active). |

### Dialog: Symbol Specification

| Tab | Fields |
|---|---|
| General | Symbol (Name, Category, Placement), Size (Width, Depth, Height), Position (Elevation from floor, Position X and Y at the back center, Angle) |
| Options | Flip |
| 3D | (disabled) |
| Layer | The layer |
| Label | Label text |
| Components, Object Information | (disabled) |

## 6.6 Chief catalogs

The Library Browser reads the catalogs of your own Chief Architect install **in place**: Core, Bonus,
Manufacturer and your User catalog. Nothing is copied into a plan or into Plan Studio (the licensing rule
in chapter 12.7 and `DECISIONS.md`). The reader is the `plan-calib` crate (chapter 12.7 explains what it
decodes); the Library Browser side lives in `shell/library_browser/chief_ui.rs` and
`tools/library/chief.rs`.

### Turning it on

- **Use Chief Architect catalogs** (a check box under the category tree) switches the Chief rows on or off.
  With no saved choice it is on when Chief's standard install folder exists
  (`/Library/Application Support/Chief Architect Premier X18`, or X17) and off otherwise. Hover text shows
  the licence note.
- **Catalog folders...** opens the "Chief catalog folders" window: a text field and **Browse...** for an install
  folder that holds `Core Libraries`, `Bonus Libraries` and `Manufacturer Libraries`, a "Folder found." or
  "Folder not found." check, **Use standard location** (clears the field) and **Apply**. Leave it empty for Chief's
  standard location. Applying saves the choice and the status line says "Chief catalog folder saved; expand a node
  to rescan." The program reads Chief's registry file (`Chief Library.json`) when it finds one; without it, it scans the
  three sub-folders for `.calib` and `.calibz` files (and the folder itself for user catalogs). Your user library at
  `~/Documents/Chief Architect Premier X18 Data/Database Libraries/User_Library.calib` is added when the file exists.
- Both settings are saved as the `chief_catalogs` object (`enabled`, `folder`) in `~/.plan-studio/settings.json`; the
  other keys of that file are kept.

### The four nodes

Under the check box are four collapsed nodes, in Chief's order: **Chief Architect Core Catalogs**, **Bonus
Catalogs**, **Manufacturer Catalogs** and **User Catalog**, each with the number of installed catalogs in
brackets. Catalogs that are listed in Chief's registry but not installed, and deleted entries, are left out;
the status line under the nodes reads "n Chief catalogs found" (with "(m not installed)" when some are missing).

- The **first time you expand a node** the program scans the install on a background thread ("Scanning Chief
  catalogs..."); the window stays responsive. An empty node says "No catalogs installed."
- Expanding a catalog loads its **category tree** and object list on another thread ("Loading...") and shows the
  tree with object counts. Click a category (or the catalog's name) to list its objects in the results area, headed
  by the catalog and category path with a **Show all** button and "n objects". A catalog that cannot be read says
  "Could not read this catalog: <reason>".
- Each object row is 54 px tall: the catalog's own **thumbnail** (decoded from its PNG, a few per frame so
  scrolling stays smooth; objects without one show the first letter of the name), the **name** and the decoded
  **size** (`w x d in`). Only the rows on screen are decoded.
- The search field filters the open category by name or keyword (every word must match). Tick **Search Chief
  catalogs** (shown while the Chief rows are on) to search every installed catalog: results appear under the
  built-in ones as "Chief catalogs: n hits" (the first 200) with the catalog name in each row. The search starts
  0.35 seconds after your last keystroke, on a background thread.

### Placing, Open Object and Replace From Library

- **Click a row** to make it the active item and switch to the Library tool; click in the plan to place it, as in
  6.5 (wall-mounted rotation, handles, Symbol Specification). The first click reads the object from the catalog and
  bridges it into a library item with the id `chief.<catalog-uuid>.<object id>`. The plan stores only that id;
  after you reopen it the item is read from the catalog again the first time it is needed.
- **Right-click a row > Open Object** opens a window with the Name, Category, Keywords, Size (width x depth x height
  in inches, or "not decoded"), Source catalog and the licence note.
- **Symbol Specification** for a Chief symbol shows its **Source catalog** and the licence note, and **Replace From
  Library** works as for built-in items: pick an item in the Library Browser, then press the button; position, angle
  and size stay.
- **3D**: a placed Chief symbol is drawn with the object's decoded meshes fitted to the symbol. When the decoded
  geometry is missing or partial (its height is under 60% of the object's size) the symbol shows as a box, and the
  status bar says once "3D: n Chief objects have only partial geometry and show as a box". When the catalog is not
  available (the folder moved, or the check box is off) the symbol is not drawn in 3D. Built-in symbols show as boxes in 3D too.

What is not decoded yet (partial or missing meshes, placeholder plan symbols, zero elevation) is listed in
chapter 12.7. Library > Import Library (.calib, .calibz)... in the menu is still dimmed (planned), as are Add to
User Library and the other Library menu items.

## 6.7 Differences from Chief

- Built-in symbols are 2D drawings with a box in 3D. Chief catalog objects show their decoded 3D meshes, but a
  fraction of them decode only partially and fall back to a box (6.6).
- Chief catalogs are read, never written: no Add to User Library, no Import Library, no editing of a catalog.
- Search filters are name and keyword only.
- Cabinets are not in the 3D view at the Round 7 commit (QA-05; Round 8 is wiring them in). Only the Front face of a cabinet can be edited; its Sides and Back cannot. Library door and drawer styles, countertop corner treatments and
  accessories are (planned).
