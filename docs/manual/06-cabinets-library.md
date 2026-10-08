# Chapter 6: Cabinets and the Library

This chapter covers the parametric cabinet tools and the Library Browser that
places symbols (fixtures, appliances, furniture, plants, lighting) into the
plan, including the Chief Architect catalogs it reads from your own Chief install,
and the Image and Distributed Objects tools (pictures, billboards, the image library,
objects spread along a path or over a region, and the 3D Solid Feature).

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
| Soffit Polygon | | Click the corners of the soffit (or drag a rectangle); `Enter` or the first corner finishes, `Backspace` removes a corner. It is stored as a Soffit with a polygon outline (one undo step, "Place Soffit Polygon"). |
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
| Vanity Cabinet, Pantry Cabinet, Tall Oven Cabinet, Refrigerator Cabinet | | Work: the **library types** at the foot of the flyout (below). |

All entries start the one Cabinet tool in that kind, so the flyout choice and the hotkey
select the kind directly. Press `Tab` while the tool is active
to cycle the kind (through all sixteen, in flyout order); the status bar names it ("Base Cabinet: click to place, drag to set the width; Tab
changes the cabinet type"). Press `Shift+Tab` to walk the **library types** instead: Vanity, Pantry, Tall Oven, Refrigerator, and then back
to the plain kinds.

#### Library types: Vanity, Pantry, Tall Oven, Refrigerator

These four are cabinets Chief's library offers as entries of their own; Plan Studio builds each from a plain kind with its own size and
face, and keeps the type on the cabinet for its label and the schedule. They start from the sizes of **Edit > Default Settings > Cabinets > Cabinet Defaults > Library Types** (6.3a).

| Type | Built from | Size (W x D x H) | Face | Label code |
|---|---|---|---|---|
| Vanity Cabinet | Base | 30 x 21 x 34 1/2" | A 5" drawer over doors | `VB` |
| Pantry Cabinet | Full Height | 24 x 24 x 84" | Two stacked doors | `PN` |
| Tall Oven Cabinet | Full Height | 30 x 24 x 84" | Upper door, oven and microwave openings, a drawer below | `OC` |
| Refrigerator Cabinet | Full Height | 36 x 25 x 84" | A 70" open bay under an upper cabinet | `REF` |

The Tall Oven and Refrigerator types carry **appliance bays**. A library appliance dropped within 30" of a bay that takes it turns
and sits in the bay (a range bay takes an oven and the other way round).

### Placing and editing

| Gesture | Result |
|---|---|
| Move | A ghost cabinet follows the pointer. |
| Click | Places a cabinet. Within 12" of a wall, the cabinet's back rotates to that wall and sits flush to its face. Away from walls it keeps the tool's angle. |
| Press, drag, release | Sets the width in 3" steps (one cabinet). |
| Place or drag next to another cabinet | It slides to butt against the neighbor and aligns its back line (bumping). |
| `Tab`, `Shift+Tab` | Next cabinet kind; next library type (Vanity, Pantry, Tall Oven, Refrigerator). |
| Click the width, gap or distance of the temporary dimensions | Types a value (below). |
| `G` | **Generate Countertop**: joins the countertops of touching base cabinets into custom countertops (below). |
| `Esc` | Returns to Select Objects. |

**Placement sizing.** A cabinet clicked into a gap (between a wall and a cabinet, or between two cabinets) whose width is within the
tolerance of the cabinet's own takes the gap's width and position, so a 36" default base clicked into a 34" gap becomes 34" and
sits against its neighbors. The tolerance is 2" and is changed in Preferences > Architectural (*Gap within ... of the cabinet's width*);
`Alt` while placing, or the Preferences switch off, places the cabinet at its own width. Soffits, shelves, partitions and the other
non-run kinds are not fitted this way.

**Temporary dimensions you can type.** A cabinet you place (as a ghost) or select shows its width and the distances from each
end to the nearest wall or cabinet (across the opening of the wall behind it) and to the near jamb of an opening in that wall.
Click one of a selected cabinet's values, type a number and press `Enter`: the cabinet resizes or moves to match.

A placed cabinet is selected. Its handles:

- **Move**: dragging keeps the rotation until the cabinet bumps a wall, where it
  re-rotates to it and sits back to the wall face. Hold `Ctrl` to turn the wall rotation off.
  A cabinet dragged into a gap (between a wall and a cabinet, or two cabinets) whose width is within 2"
  of its own **fits to the gap**: it takes the gap's width and position exactly. `Alt`, or
  Preferences > Architectural, turns that off.
- **Resize** at both ends: changes the width in 3" steps; the cabinet grows from the dragged side.
- **Depth** handles on the middle of the front and back edges: change the depth in 1" steps
  (3" at least); the opposite edge stays put, so the back can stay on the wall.
- **Corner** handles: change width and depth together, the opposite corner staying put.
- **Rotate**.
- **Label**: drag the cabinet's plan label to move it; the offset is kept with the cabinet.

The height is set in the Cabinet Specification. Free-form tops (custom countertops and backsplashes)
have no resize handles.

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
A counter hole is cut out of the countertop it lies in. A custom countertop has a thickness and an **Edge Profile** (Square, Beveled, Bullnose or Ogee, with an edge size; Waterfall applies to a cabinet's own top only); a custom backsplash has a height and a strip thickness.

#### Generate Countertop (`G`)

With the Cabinet tool active, `G` joins the countertops of touching base cabinets into custom countertops: the selected cabinets, or all of them when none is selected. The cabinets give up their own slab (they shrink by its thickness),
and their sink and cooktop holes move to the new top. One undo step ("Generate Countertop"); "No base cabinet countertops to join" if there is nothing to join.

#### Automatic countertop join

With *Join the countertops of touching base cabinets automatically* on (Preferences > Architectural,
on by default) the join of Generate Countertop happens by itself. Two or more base cabinets that touch
share one generated countertop; it is rebuilt whenever a cabinet under it is placed, moved, resized,
changed in its dialog, pasted or deleted, inside the undo step of that edit, and it keeps its identity
while the same cabinets stand under it. Move a cabinet away and the top comes apart: each cabinet gets its own
slab (and its own sink and cooktop holes) back. A lone cabinet keeps its own slab. Clicking a base cabinet in a run
picks the cabinet, not the top over it; the overhang past the cabinets picks the top.
Deleting a generated top gives the slabs back and the top is not rebuilt by that step. A change to the top's edge profile or
corner treatment in its dialog is remembered for the next join. Turn the preference off to join only by hand with `G`.

#### Appliance openings, sinks and cooktops

A base cabinet can hold an **open bay for an appliance** (General tab, *Appliance Opening*): Dishwasher, Range, Refrigerator or Microwave. The bay stays open and the appliance fills it in plan and 3D. A countertop can have **sink and cooktop cutouts**
(*Add Sink*, *Add Cooktop*, each with a *Remove* button; their area in square inches is listed).

#### Cabinet Schedule and callouts

A **Cabinet Schedule** from the Schedule flyout lists the plan's cabinets as a live table and, with *Show schedule number labels*, labels each cabinet `C-01`, `C-02` ... in the plan (chapter 11.2).

#### Countertop edge: Waterfall

The **Waterfall** edge (Edge Profile list, on a cabinet's own countertop) runs the slab down to the floor at both ends of the cabinet, as
thick as the top. A custom or generated top builds it square. A **full-height backsplash** (General tab, Backsplash) rises to the
underside of the wall cabinet above it, or to 54" above the floor when nothing hangs over it; its stored height is kept in step after
every edit.

#### Labels, label layer and macros

A cabinet's plan label is drawn on its own layer, **Cabinets, Labels**, so the labels can be hidden or restyled apart from the
cabinets (the layer is added to the plan with the first cabinet). The label text is the automatic label (`B24`, `W3030`, `VB30`) or your
own, which can hold macros (6.3, Label). The Cabinet Schedule columns are Mark, Label, Type, Width, Depth and Height, with Elevation,
Countertop, Floor, Door Style, Drawer Style, Finish and Hardware available to turn on (chapter 11.2).

Not built: elevation views of a cabinet run. Cabinets are in the 3D view (QA-05, fixed in Round 8; chapter 10.1): every kind is meshed at its stored position, with countertops in the Stone material and handles in Metal.

## 6.3 Dialog: Cabinet Specification

The preview shows the plan symbol and a front elevation of the resolved face items.

| Tab | Status |
|---|---|
| General | Works |
| Box Construction | Works |
| Front/Sides/Back | Works. The **Cabinet Side** list picks Front, Left, Right or Back; each of the three sides is a Plain Panel, a Finished Panel, Open, or a **Custom Face** with its own face-item tree edited like the front (rectangular cabinets only; the sides are built in 3D) |
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
- **Countertop**: Thickness (1 1/2"), Overhang Front, Back and Sides (1"), **Edge Profile** (Square, Beveled, Bullnose, Ogee or Waterfall) and Corner Treatment
  None / Clipped / Rounded with a size (on the front corners of a cabinet's own top and on every convex corner of a custom or joined top). **Sink and Cooktop Cutouts**: Add Sink, Add Cooktop, Remove.
- **Backsplash**: Height, Thickness, and **Full height** (to the underside of the wall cabinet above, else to 54").
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
Door, Slab Door, Shaker Door and Raised Panel Door; the drawer styles are Lincoln Flat Panel Drawer, Slab Drawer, Shaker Drawer and Raised Panel Drawer; library styles are (planned). Handles (**hardware styles**) are None, Knob, Pull (a bar pull on two posts, vertical on a door and horizontal on a drawer), Cup Pull (a half-round plate at the top edge of a drawer) or Edge Pull (a thin lip along the free edge of a door or the top of a drawer front).
**Accessories**: Front Pilasters, Feet (Foot Style), Side Panels (Main Panel Style, Full Size Panel); the controls are disabled.

### Opening Indicators, Moldings, Layer, Materials, Label

**Opening Indicators**: the check box *Show door swings and open drawers in plan* draws each door's quarter-circle swing from its hinge and each drawer pulled out in front of the cabinet; a second check box shows the same in the **3D view**: doors stand open with the shelves inside showing and drawers are pulled out with their boxes.
**Moldings** lists up to four profiles, each with a Projection, a Height and a Delete button; **Add Crown** puts a crown molding on top of the cabinet and **Add Light Rail** one under it; both run along the front and return at the ends.
**Layer** shows the layer (Cabinets, Base; Cabinets, Wall ...), which follows the kind. **Materials** gives each part (Box, Door Fronts, Drawer Fronts, Countertop, Backsplash, Toe Kick, Molding) a material from Default, Wood, Painted, Stone, Concrete,
Metal or Glass; Default keeps the part's usual stand-in. **Label** shows the automatic label (`B24`); *Specify label* lets you type your own, with the macros `<L>` the automatic label, `<T>` the type letters (`B`, `W`, `FH`, `VB`), `<W>` `<D>` `<H>` width, depth and height (whole inches or trimmed decimals), `<WxD>` `<WxH>` `<WxDxH>` sizes joined with x, `<N>` the cabinet's name (`Base Cabinet`, `Vanity Cabinet`), `<S>` the door style, `<F>` the finish, `<HW>` the hardware and `<A>` the appliance a bay holds. A label without macros stays literal.

### 6.3a Cabinet Defaults

**Edit > Default Settings > Cabinets > Cabinet Defaults** (Active Defaults tree, 1.7) sets what every new cabinet starts with. The
window has ten tabs; OK saves them to the plan's defaults and Cancel drops the edits.

| Tab | Holds |
|---|---|
| Base | Width, depth, height (with countertop), countertop thickness and front overhang, toe kick height and depth, door style, drawer style and handle |
| Wall | Width, depth, height, bottom from floor |
| Full Height | Width, depth, height |
| Soffit, Shelf, Partition | Width, depth, height and, for the soffit and the shelf, the elevation |
| Countertop | Thickness and the four overhangs, edge profile and size, corner treatment and size |
| Backsplash | Whether new base cabinets get one, full height or a height, thickness |
| Library Types | The sizes of the Vanity, Pantry, Tall Oven and Refrigerator cabinets |
| Fillers and Corners | Filler width, corner base and corner wall legs, blind width and hidden width |

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
- A **Filters** button (type, catalog, style or manufacturer word, size range, favorites only), a **Sort** menu
  (relevance, name, type, size, recently used) and a **List / Grid** toggle.
- **Favorites** and **Recently Used** entries above the tree. Star an item from its right-click menu or the
  preview pane; an item joins the recent list when you pick or place it. Both lists are kept in
  `~/.plan-studio/user-library-meta.json`.
- A **User** node in the tree: your own catalog (6.4a), with folders, favorites and recents.
- A **Preview and Object Information** pane for the clicked item: a 2D/3D toggle (the 3D view is a software-shaded
  picture of the item's model, or a box of its size; drag it or use the arrow buttons to rotate) and the item's
  type, category, size, placement, layer, keywords and triangle count.
- A right-click menu on a result: Add or Remove Favorite, Open Object, and Add to User Library for built-in items;
  user items also get Rename, Duplicate, Move To and Delete. Chief rows have Open Object (6.6) and a dimmed
  Add to User Library: Chief content is licensed, read in place and never copied.

### 6.4a The User catalog

Your own items live in `~/.plan-studio/user-library.json`; their 3D models are `.psm` files in
`~/.plan-studio/user-models/`.

- **Folders.** Right-click User or any folder: New Folder, Rename Folder, Move Folder To, Delete Folder (the items
  inside go with it, after a confirmation). Drag a user item onto a folder to move it, or use Move To.
- **Add to Library.** Select a symbol, cabinet, CAD line, arc, circle or polyline, or text, then Library > Add
  Selection to Library. Symbols keep their size, flip and 3D model; a cabinet is kept whole (placing it makes a
  cabinet again); CAD pieces become one block and text becomes a text item. Library > Add Active Material to Library
  saves the Material Painter's material as a 12 in swatch. The Symbol Specification has Add to Library and
  Convert to Symbol (save, then use the saved item).
- **Object Information.** Open Object on a user item (or Edit in the preview pane): name, keywords (comma
  separated; the search finds them), category folder, type, style, manufacturer, width, depth, height, elevation,
  placement, whether it turns to face a wall, default layer, the 2D symbol (keep, draw from the 3D model, plain
  rectangle) and the 3D rotation and origin offset.
- **Import 3D Model.** Library > Import 3D Model (OBJ, glTF) picks a `.obj`, `.gltf` or `.glb`. The window shows
  the triangle count and size and asks for the file's units (inches, feet, millimeters, centimeters, meters; glTF
  defaults to meters), the up axis (Y or Z), the folder and the placement. The plan symbol is drawn from the model
  seen from above. OBJ colors come from a `.mtl` beside the file; glTF reads scenes, node transforms, triangle
  meshes and base colors, not textures, sparse accessors or Draco compression.
- **Export Library / Import Library.** Library > Export Library (Plan Studio only) writes a `.calibz` zip of the catalog JSON, the
  folders, favorites and recents, and the `.psm` models. It is Plan Studio's own format: the zip is stored without compression and
  Chief Architect cannot open it (its README says so). Library > Import Library reads such a file back (items with the same id
  are replaced); a zip made by another tool works only if its entries are stored, not deflated. Chief `.calib` files are read in place
  instead (6.6, chapter 12.7).
- **Placement.** Items snap by their placement: wall-mounted items turn to the nearest wall; a free-standing item
  can be set to turn to walls too (a bookcase); ceiling items hang at the ceiling height. Placed copies go on the
  item's layer (cabinets on the cabinet layers, electrical and lighting on Electrical, text on Text, else
  CAD, Default).

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
| Edit toolbar | Open Object, Delete Objects, Copy, Paste in Place and **Replace From Library**: pick an item in the Library Browser first, then press the button to swap the selected symbol for it ("Pick an item in the Library Browser, then choose Replace From Library" if none is active). A selected cabinet is replaced by a saved cabinet item (it keeps its sink and cooktop cutouts, appliance, moldings and label); a selected electrical device becomes the active library symbol. |

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

## 6.7 Images, billboards and distributed objects

Three flyouts on the Build bar and menu hold these tools: **Image** (Create Image, Create Billboard Image, Create
Image Library), **Distributed Objects** (Polyline and Spline Distribution Path, Polyline and Spline Distribution
Region) and, at the foot of the 3D Solid flyout, **3D Solid Feature**. They are one tool with a flavor per entry
(`tools/images.rs`). None has a hotkey. A picture or a distribution record is a placed symbol (`image`,
`distribution`, `owner` and `solid` fields of `PlacedSymbol`), so it saves, undoes, copies and moves like any other.

### Pictures

| Tool | Gesture |
|---|---|
| Create Image | The first click opens a file picker (PNG or JPEG) and places the picture at the click, 36" on its long side. Later clicks place the same picture again until `Esc`. |
| Create Billboard Image | The same, but the picture stands upright (72" high by default) and faces the camera in 3D. |
| Create Image Library | Click a picture already in the plan to save it in the user library, or click empty space to pick a file to save. The picture becomes the **active library item**, ready for a distribution or the Library tool. |

- **PNG** pictures are decoded and drawn as a textured quad in the plan. **JPEG** pictures are drawn in the plan as a framed placeholder with the note "JPEG preview is not available
  yet; drawn as a frame" (the plan view still uses its older PNG-only decoder). The file is kept by path, so nothing is lost. Transparency applies to PNG only.
- **In 3D** (Round 11) a picture shows its own bitmap, PNG or JPEG (baseline or progressive), on its quad while the 3D view's Textures box is ticked (chapter 10.8a); with the box off, or when the file is missing or cannot be decoded, it is a flat-colored quad: the picture's average color
  mapped to the nearest material. The ray tracer always draws a picture as the flat-colored quad. A flat picture lies on its footprint 1/10" above the surface; a billboard stands upright.
  In the live 3D view a billboard turns to face the camera each frame; glTF export and the ray tracer still use its stored angle.
- **Image Specification** (double-click a picture): General (File with Browse, Picture size in pixels, Width, Height (billboard) or Depth (flat picture), Elevation
  from floor, Position X and Y at the back center, Rotation, Flip), Image (Make one colour transparent with a
  Tolerance, and the Billboard switch), Layer, Label. A **Match picture proportions** button restores the picture's aspect ratio; **Browse...** swaps the file.
- The **user library** is saved in `~/.plan-studio/user-library.json` (the settings folder of chapter 1) and
  read back when the program starts. The Library Browser does not list the user library yet: a saved picture is used through the
  active item, and a plan finds it again by its id (`user.image....`). Listing it in the browser is (planned).

### Distributed objects

Pick a library item first (a Library Browser click, or Create Image Library); then:

| Tool | Gesture |
|---|---|
| Polyline Distribution Path, Spline Distribution Path | Click the points of a path; `Enter` or a double-click finishes (at least 2 points). Copies of the item are placed along it. |
| Polyline Distribution Region, Spline Distribution Region | Click the corners of an outline; `Enter` or a double-click finishes (at least 3 points). Copies fill the region. |
| `Backspace`, `Delete` | Drop the last point. |
| `Esc` | Clears the points; a second `Esc` leaves the tool. |

The result is one **distribution record** (drawn as a dashed path or outline) that owns its copies. Open its
**Distribution Specification** by double-click:

| Tab | Fields |
|---|---|
| General | The object (and **Use Active Library Item**); Object width, depth, height and elevation. Path: Spacing, Offset (start of path), Side offset (left), Turn objects to follow the path. Region: Grid spacing, Offset (inset from edge), Pattern Grid or Random. A line counts the objects. |
| Random | Scatter (radius), Random rotation (+/- degrees), Random size (+/- percent), Random seed with a **New pattern** button |
| Layer, Label | The layer the copies are placed on; label text |

OK rebuilds the copies in the same undo step. Spacing is at least 1" and one record makes at most 5,000 copies. The same
seed gives the same random pattern. A record that is **moved** (drag, arrow keys, paste in Select Objects) moves its copies with
it; deleting the record deletes its copies.

### 3D Solid Feature

With a library item active, a click places it as a **solid**: in 3D the item is drawn in the Concrete material
(its decoded mesh for a Chief catalog object, otherwise a box of its width, depth and height) instead of its usual look. The plan
shows the usual symbol. This is Chief's way of making a library object read as a mass in a study model.

## 6.8 Differences from Chief

- Built-in symbols are 2D drawings with a box in 3D. Chief catalog objects show their decoded 3D meshes, but a
  fraction of them decode only partially and fall back to a box (6.6).
- Chief catalogs are read, never written: Chief objects cannot be added to the User catalog, and Import Library reads only Plan Studio's own export, not `.calib` files (6.4a).
- Imported and saved 3D models show in the preview pane; the 3D view of the plan draws them once `user::placed_meshes` is hooked into `view3d_panel.rs` (open).
- Search filters have no "objects already in the plan" mode.
- A custom face on the Sides and Back needs a rectangular cabinet. Library door and drawer styles and
  accessories are (planned).
