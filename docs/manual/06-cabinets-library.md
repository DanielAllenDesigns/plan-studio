# Chapter 6: Cabinets and the Library

This chapter covers the parametric cabinet tools and the Library Browser that
places symbols (fixtures, appliances, furniture, plants, lighting) into the
plan.

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
- Kinds: Base, Wall, Full Height, Soffit, Shelf, Partition.
- Chief defaults in Daniel's template: Base 24" x 24" x 36" high (the 36" includes a
  1 1/2" countertop with a 1" overhang, and a 4" x 3" toe kick); Wall cabinet 24" x 12" x 30"
  with its bottom at 54"; Full Height 24" x 24" x 84". Door style Lincoln Door,
  drawer style Lincoln Flat Panel Drawer, Knob handles.
- Automatic labels follow the industry style: `B24` (base 24"), `B36-SB` (sink base),
  `W3030` (wall cabinet, width then height), `FH2484` (full height), with `SO`, `SH`,
  `PT` for soffit, shelf, partition.
- Engine helpers that exist but have no editor command yet: `run_along_wall`
  (a row of cabinets along a wall) and countertop merging (planned).

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
| Base Filler | `Ctrl+Alt+Cmd+0` | (planned) |
| Wall Filler | `Ctrl+Alt+Cmd+1` | (planned) |
| Full Height Filler | `Ctrl+Alt+Cmd+2` | (planned) |
| Custom Countertop | `Ctrl+Alt+Cmd+3` | (planned; reports "not yet implemented") |
| Custom Backsplash | `Ctrl+Alt+Cmd+4` | (planned) |
| Custom Counter Hole | `Ctrl+Alt+Cmd+5` | (planned) |

All six working entries start the one Cabinet tool in that kind, so the flyout choice and the hotkey
select Base, Wall, Full Height, Soffit, Shelf or Partition directly. Press `Tab` while the tool is active
to cycle the kind; the status bar names it ("Base Cabinet: click to place, drag to set the width; Tab
changes the cabinet type").

### Placing and editing

| Gesture | Result |
|---|---|
| Move | A ghost cabinet follows the pointer. |
| Click | Places a cabinet. Within 12" of a wall, the cabinet's back rotates to that wall and sits flush to its face. Away from walls it keeps the tool's angle. |
| Press, drag, release | Sets the width in 3" steps (one cabinet). |
| Place or drag next to another cabinet | It slides to butt against the neighbor and aligns its back line (bumping). |
| `Tab` | Next cabinet kind. |
| `Esc` | Returns to Select Objects. |

A placed cabinet is selected. Its handles:

- **Move**: dragging keeps the rotation until the cabinet bumps a wall, where it
  re-rotates to it. Hold `Ctrl` to turn the wall rotation off.
- **Resize** at both ends: changes the width in 3" steps; the cabinet grows from the dragged side.
- **Rotate**.

Double-click or press `Enter` opens the Cabinet Specification. The Edit toolbar offers
Open Object, Delete Objects, Copy Selected Objects, Paste in Place and **Reverse Door
Swing**. Select a cabinet with Select Objects too; the Cabinet tool can also pick cabinets.

Not built: cabinet depth/corner resize handles, fillers, merged countertops over adjacent
cabinets, appliance insertion with a countertop cut-out, a Cabinet Schedule (the engine
is ready), elevation views of a cabinet run (all planned).

## 6.3 Dialog: Cabinet Specification

The preview shows the plan symbol and a front elevation of the resolved face items.

| Tab | Status |
|---|---|
| General | Works |
| Box Construction | Works |
| Front/Sides/Back | Works |
| Door/Drawer | Works |
| Accessories | Works |
| Opening Indicators | (disabled) |
| Moldings | Works (list only) |
| Layer | Works |
| Fill Style | (disabled) |
| Materials | Works (list) |
| Label | Works |
| Components, Object Information, Schedule | (disabled) |

### General

- **Cabinet Style**: Type (Standard); Treat As Filler.
- **Size/Position**: Width, Height (including countertop), Depth, Finished Floor to Bottom,
  Finished Floor to Top, Position X and Y (the back-left corner), Angle.
- **Countertop**: Thickness (1 1/2"), Overhang Front, Back and Sides (1"), Corner Treatment
  None / Clipped / Rounded.
- **Backsplash**: Height, Thickness.
- **Toe Kick**: Height (4"), Depth (3").

### Box Construction

- **Box Construction**: Framed (with Separation) or Frameless.
- **Top/Bottom/Sides**: Top Auto / Has Top / No Top; Bottom Auto / Has Bottom / No Bottom;
  Side / Back Thickness.
- **Door/Drawer Overlay**: Traditional Overlay, Full Overlay, Inset.

### Front/Sides/Back

- **Cabinet Side**: picks the face to edit (Front is the default), and Side Type.
- **Face Items**: an indented tree such as `Vertical Layout > Separation, Layout > Drawer,
  Separation, Door - Auto Right, Separation`. Buttons: **Add New**, **Delete**, **Move
  Up**, **Move Down**, **Split Vertical**, **Split Horizontal**, **Equalize**, **Reset to
  Default Face**, **Sink Base Face**.
- **Selected Item Properties**: Item Type (Door, Drawer, Separation, Opening, Appliance,
  Horizontal Layout ...), Item Height (0 = auto), Item Width, Lock from Auto-Resize, and an
  Appliance field.
- Splitting an item recomputes its siblings' heights and widths unless one is locked.

### Door/Drawer, Accessories

Door/Drawer: Door Panel (Main Style, Thickness), Door Handle (Main Style), Drawer Panel,
Drawer Handle. Accessories: Front Pilasters, Feet (Foot Style), Side Panels (Main Panel
Style, Full Size Panel). Style names come from the engine's built-in styles (Lincoln Door,
Lincoln Flat Panel Drawer); library styles are (planned).

### Moldings, Layer, Materials, Label

Moldings lists profiles (editing is planned). Layer picks the layer (Cabinets, Base;
Cabinets, Wall). Materials lists the default materials. Label shows the automatic label
(`B24`) and lets you type your own.

## 6.4 The Library Browser

Open it with the Library Browser button on the view bar or `Cmd+L`. It is a dock
(chapter 1.5) with:

- A **search field** with a clear button. Search is case-insensitive; every word must
  match. Ranking, best first: name (whole word, prefix, substring), then tags, then category.
- A **category tree** with item counts. Click a category to filter. The tree has a
  collapsed "Chief catalogs" node that says "Connect Chief catalogs (plan-calib) - coming".
- A **result list** of up to 200 rows, each with a small drawing of the item's 2D symbol, its
  name and its size (for example `30 x 28 in`).
- The line "Active item: ..." shows what a click in the plan will place.
- A right-click menu on a result with Open Object and Add to User Library (both "coming").

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

## 6.6 Chief catalogs (.calib)

Reading Daniel's Chief libraries (Core, Bonus, Manufacturer, User) is written but not part
of the editor yet. The `plan-calib` crate opens `.calib` and `.calibz` files in place, lists
categories and thumbnails, searches across catalogs, and turns objects into library items with a decoded
size and a plan symbol projected from the object's 3D geometry (placeholder when the decode does not
fit); see chapter 12.7 for how it works and what is and is not decoded. The Library Browser shows a
"Chief catalogs" node that says "coming", and Library > Import Library (.calib, .calibz)... in the menu is
dimmed (planned).

Two decisions apply (`DECISIONS.md`): Chief catalog content is read at runtime from the
user's own Chief install and never copied into the repository, and our own starter symbols
are the only library content that ships.

## 6.7 Differences from Chief

- 2D symbols only; no 3D models for fixtures and furniture.
- No manufacturer catalogs, user library folder, or Add to Library.
- No cabinet-to-appliance insertion, joined countertops or fillers yet.
