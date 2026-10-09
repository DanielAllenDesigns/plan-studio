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
- Automatic labels follow Chief's four-part format, Key + Code + Size + Door Swing (Label, below): `B24` (base 24"),
  `3DB24` (a bank of three drawers), `SB24R` (sink base with a right door), `W3030` (wall cabinet, width then
  height), `U242484` (full height), `BF3` (a filler), `OTC362490` (tall oven), with `SO` for a soffit. Shelves,
  partitions and the fillers the program makes have no automatic label.
- Countertop merging is Generate Countertop (`G`, 6.2). One engine helper has no editor command yet: `run_along_wall` (a row of cabinets along a wall).

## 6.2 Tools

### Cabinet Tools (row 2, Cabinet flyout; Build > Cabinet)

| Variant | Hotkey | Today |
|---|---|---|
| Base Cabinet | `Shift+T` | Works. |
| Wall Cabinet | `Cmd+T` | Works. |
| Full Height | `Ctrl+Alt+Cmd+X` | Works. |
| Soffit | `T` | Works as a cabinet-like box. |
| Soffit Polygon | | Click the corners of the soffit (or drag a rectangle); `Enter` or the first corner finishes, `Backspace` removes a corner. It is stored as a Soffit with a polygon outline (one undo step, "Place Soffit Polygon"). A closed CAD polyline you have selected becomes the same soffit with the Edit toolbar button **Convert Polyline to Soffit** (the polyline is replaced; one undo step). |
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
| Place or drag next to another cabinet | It slides to butt against the neighbor and aligns its back line (bumping). The Edit toolbar button **Neighbors: Bump / Push / Pass Through** changes what a dragged cabinet does (below). |
| `Tab`, `Shift+Tab` | Next cabinet kind; next library type (Vanity, Pantry, Tall Oven, Refrigerator). |
| Click the width, gap or distance of the temporary dimensions | Types a value (below). |
| `G` | **Generate Countertop**: joins the countertops of touching base cabinets into custom countertops (below). |
| `Esc` | Returns to Select Objects. |

**Narrow spaces.** A cabinet clicked into a space that has a wall or cabinet on both sides and is narrower than the cabinet takes the
largest multiple of the Resize Increment that fits (a 24" cabinet in a 20" space with a 3" increment becomes 18"), never below the Minimum
Cabinet Width; in a space narrower than the minimum nothing is placed and the status line says so. A **wall cabinet** placed over a
free-standing appliance (or a cabinet's appliance bay) that reaches above its usual bottom hangs from the appliance top.

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
- **Bumping and pushing**: in **Bump** (the default) a dragged cabinet stops butted against the cabinets it meets. In **Push** it pushes the cabinets of its run along ahead of it (they stay butted, and come back if you drag back); if a wall or another run is in the way it bumps instead. **Pass Through** ignores other cabinets. One undo step undoes the move and every push. The setting is for the session (Edit > Neighbors cycles it too) and the Select tool follows it when you drag a cabinet.
- **Resize** at both ends: changes the width by the **Resize Increment** (3" unless Default Settings > Cabinets > General Cabinet changes it, or asks for the Snap Grid); the cabinet grows from the dragged side. An edge dragged within 3" of a wall or the next cabinet snaps to it, so the cabinet fills the gap (`Alt` turns the snap off).
- **Depth** handles on the middle of the front and back edges: change the depth in 1" steps
  (3" at least); the opposite edge stays put, so the back can stay on the wall.
- **Corner** handles: change width and depth together, the opposite corner staying put.
- **Rotate**.
- **Label**: drag the cabinet's plan label to move it; the offset is kept with the cabinet.

The height is set in the Cabinet Specification. Free-form tops (custom countertops and backsplashes)
have no resize handles.

Double-click or press `Enter` opens the Cabinet Specification. The Edit toolbar offers
Open Object, Delete Objects, Copy Selected Objects, Paste in Place, **Reverse Door
Swing** and **Set as Default** (copies one standard cabinet into the defaults of its kind). Select a cabinet with Select Objects too; the Cabinet tool can also pick cabinets.

#### Automatic fillers, merging and module lines

Cabinets of one family (base, wall or full height) and one height that stand side by side with up to 3" between them, or between a
cabinet side and a wall, get a **filler** of exactly that width (to 1/16"), with the same toe kick, countertop, backsplash and moldings.
The countertop runs on over the filler to the wall. A filler also goes in the angle where two runs meet at a front corner within 3".
These fillers are made again after every edit (one undo step with it); you cannot pick them, they carry no label, and the Cabinet
Schedule leaves them out. A manually placed filler (below) is a cabinet like any other. Extended stiles on a framed cabinet count
towards the gap and put `XL`, `XR` or `XLR` in the label.

Merged cabinets (side by side within 3", or meeting at a front corner, or at a back corner facing away at 87 degrees or less) draw
**module lines** where they meet, dashed, on the layer **Cabinets, Module Lines**; turn the layer off and they read as one block.

Default Settings > Cabinets > **General Cabinet** (this dialog opens from there only) sets: the Minimum Cabinet Width (not under 1/16"),
Minimum Shelf Spacing, Auto Door Threshold; Create Automatic Fillers (also for angled connections; changing it rebuilds the fillers in
one undo step) and Create Automatic Blind Corner Cabinets; **Cabinet Resizing** by the Snap Grid or by a Resize Increment (not under
1/16"); and the **Plan Display Options**: Show Partial Module Lines (a short grey tick instead of a dashed line), Show Closed
Doors/Drawers and Panels, Show Pilasters, Display Molding Edges in Plan Views. Changing a Cabinet Default moves the existing cabinets
that still have the old value (countertop thickness and overhangs, toe kick, backsplash).

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
| Accessories | Works: front pilasters (plain or fluted, left and right, width), feet (block, bun or bracket, in place of the toe kick board) and finished end panels |
| Opening Indicators | Works |
| Moldings | Works (crown and light rail) |
| Layer | Works (shows the layer; follows the cabinet's type) |
| Fill Style | Works: None, Solid, Hatch or Cross Hatch with a colour, opacity and line spacing, in the plan view |
| Materials | Works (per part) |
| Label | Works |
| Components | Works: the parts with counts, sizes and materials |
| Object Information | Works: the facts, plus manufacturer, model number, description and notes |
| Schedule | Works: *List this cabinet in the Cabinet Schedule* and the cabinet's schedule row |

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
Door, Slab Door, Shaker Door and Raised Panel Door; the drawer styles are Lincoln Flat Panel Drawer, Slab Drawer, Shaker Drawer and Raised Panel Drawer; the **library styles** follow them in the Main Style list: every library object in a "Cabinet Doors" category (and "Cabinet Drawers" for drawer fronts) from the built-in and user libraries, and, after **Load Chief Library Styles** (with the Chief catalogs on), Chief's. A picked style copies its name, look (Slab, Shaker or Raised, Glass) and library id into the cabinet; the 3D door is built from the look. Handles (**hardware styles**) are None, Knob, Pull (a bar pull on two posts, vertical on a door and horizontal on a drawer), Cup Pull (a half-round plate at the top edge of a drawer) or Edge Pull (a thin lip along the free edge of a door or the top of a drawer front).
**Accessories**: Front Pilaster (None, Plain, Fluted; Left and Right; Width), Foot Style (None, Block, Bun, Bracket; Foot Size; feet need a toe kick and stand in its place) and Side Panels (a finished panel on the left or right end, the same as Side Type on the Front/Sides/Back tab).

### Opening Indicators, Moldings, Layer, Materials, Label

**Opening Indicators**: the check box *Show door swings and open drawers in plan* draws each door's quarter-circle swing from its hinge and each drawer pulled out in front of the cabinet; a second check box shows the same in the **3D view**: doors stand open with the shelves inside showing and drawers are pulled out with their boxes.
**Moldings** lists up to four profiles, each with a Projection, a Height and a Delete button; **Add Crown** puts a crown molding on top of the cabinet and **Add Light Rail** one under it; both run along the front and return at the ends.
**Layer** shows the layer (Cabinets, Base; Cabinets, Wall ...), which follows the kind. **Materials** gives each part (Box, Door Fronts, Drawer Fronts, Countertop, Backsplash, Toe Kick, Molding) a material from Default, Wood, Painted, Stone, Concrete,
Metal or Glass; Default keeps the part's usual stand-in. **Label** shows the automatic label (`B24`); *Specify label* lets you type your own, with the macros `<L>` the automatic label, `<T>` the type letters (`B`, `W`, `FH`, `VB`), `<W>` `<D>` `<H>` width, depth and height (whole inches or trimmed decimals), `<WxD>` `<WxH>` `<WxDxH>` sizes joined with x, `<N>` the cabinet's name (`Base Cabinet`, `Vanity Cabinet`), `<S>` the door style, `<F>` the finish, `<HW>` the hardware and `<A>` the appliance a bay holds. A label without macros stays literal.

#### Automatic cabinet labels

The automatic label has four parts. The **Key** is `B` base, `W` wall, `U` full height. The **Code** says more about the box: `SB` sink
base, `RB` range base, `OB` oven base, `3DB` a bank of three drawers, `FHB` a base with one full-height door, `2D` after a wall key
for drawers (`W2D3030`), `DC`, `LC`, `LS`, `LSD` and `BC` in front of the key for diagonal, left, lazy susan, lazy susan diagonal and
blind corners (`DCB36`, `BCW2436R`), `P` after the key for a peninsula (doors on the back), `F` for a filler (`BF3`, `WF330`,
`UF32484`), `XL`, `XR`, `XLR` for extended stiles, and `OTC` and `RTC` for the tall oven and tall refrigerator cabinets. The **Size**
is the width, then the depth and height when they are not standard (base 24" deep and 34 1/2" high under the top; wall 12" deep):
base and full height read width, depth, height; a wall cabinet reads width, height (always), then depth. The **Door Swing**, `L` or
`R`, is added only when every door swings the same way (an Auto door gives none). Library types keep their own letters (`VB30`,
`PN2484`). *Suppress Label* hides one cabinet's label; the layer *Cabinets, Labels* hides them all. Shelves, partitions and custom
countertops have a blank automatic label; type one in *Specify label* if you want it.

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

A separate dialog, **Default Settings > Cabinets > General Cabinet**, holds the automatic behaviors, the resize step and the plan
display options (6.2, Automatic fillers).

## 6.4 The Library Browser

Open it with the Library Browser button on the view bar or `Cmd+L`. It is a dock
(chapter 1.5) with:

- A **header** with a **Library** menu (New Folder, Add Selection to Library, Convert to Symbol, Replace From
  Library with a "Replace keeps size" switch, Import Library, Export Library, Import 3D Model, Rebuild Thumbnails,
  Empty Trash) and a **Preferences...** link that opens the Library page of Preferences.
- A **search field** with a clear button. Search is case-insensitive; every word must
  match the name or a keyword. Ranking, best first: name (whole word, prefix, substring), then tags, then category.
  With "Search Chief catalogs" on, the keywords stored in the `.calib` catalogs are searched too.
- A **Type** filter (a drop-down of check boxes): Cabinets, Doors, Windows, Fixtures, Furniture, Plants, Materials,
  Backdrops, Moldings, Images, Electrical, Hardware. Tick several to see their union; "All" (nothing ticked) keeps
  everything. An item's type comes from what it is (a swatch is a Material, a picture an Image, a saved cabinet a
  Cabinet) and from the words in its category, name and keywords; an object that fits none of the twelve is hidden
  while a type is ticked. The filter applies to the built-in and User items and to Chief rows.
- A **category tree** with folder icons and item counts. Click a category to filter. Below the built-in tree sit
  the **Chief Architect catalogs** (6.6): the "Use Chief Architect catalogs" check box, a
  "Catalog folders..." button and four collapsed folders (Chief Architect Core Catalogs, Bonus Catalogs,
  Manufacturer Catalogs, User Catalog), and last the **Trash** (6.4a).
- A **result list** of up to 200 rows, each with a small drawing of the item's 2D symbol, its
  name and its size (for example `30 x 28 in`). The **List / Grid / Names** switch picks rows, a thumbnail grid or
  just a file icon and the name per line.
- The line "Active item: ..." shows what a click in the plan will place.
- A **Filters** button (type, catalog, style or manufacturer word, size range, favorites only), a **Sort** menu
  (relevance, name, type, size, recently used) and a **List / Grid** toggle.
- **Favorites** and **Recently Used** entries above the tree. Star an item from its right-click menu or the
  preview pane; an item joins the recent list when you pick or place it. Both lists are kept in
  `~/.plan-studio/user-library-meta.json`.
- A **User** node in the tree: your own catalog (6.4a), with folders, favorites and recents.
- A **Preview and Object Information** pane for the clicked item with a **2D / 3D / Render** toggle. Render (the
  default) is a path-traced picture of the item's 3D shape (its model, or its plan outline raised to its height),
  made with `plan-render` at a low sample count and denoised, on a background thread ("Rendering..." until it
  lands). The picture is kept in `~/.plan-studio/thumbs/<hash>.png`; the hash covers the item, its model and the
  renderer version, so an edited item renders again and a known one appears at once in later sessions. Library >
  Rebuild Thumbnails deletes the folder's pictures. 3D is the software-shaded view you can turn (drag it or use the
  arrow buttons). Below: type, category, size, placement, layer, keywords and triangle count. The **Open** button
  opens the Library Object Specification (6.5).
- A right-click menu on a result: Add or Remove Favorite, **Open Object**, **Replace Selected With This**, Add to
  Library for built-in items; user items also get Object Information, Rename, Duplicate, Move To, New Folder and
  Delete (to the Trash). Chief rows have Open Object (6.6) and no Add to Library: Chief content is licensed, read in
  place and never copied.
- **Drag a row onto the plan** to place it where you let go (the same placement as a click, one undo step); drag a
  user item onto a User folder to move it instead. Letting go over a panel places nothing.

### 6.4a The User catalog

Your own items live in `~/.plan-studio/user-library.json`; their 3D models are `.psm` files in
`~/.plan-studio/user-models/`.

- **Folders.** Right-click User or any folder: New Folder, Rename Folder, Move Folder To, Delete Folder (the items
  inside go to the Trash, after a confirmation). The User node's menu also has Export Library and Import Library.
  Drag a user item onto a folder to move it, or use Move To.
- **Trash.** Delete (on an item, or on a folder's items) moves them to the **Trash** node at the bottom of the tree
  instead of erasing them; it is kept in `~/.plan-studio/user-library-trash.json`, and a trashed item keeps its model
  file. In the Trash, **Restore** puts an item back in the folder it came from (under a new id if another item took
  its id), **Delete Permanently** and **Empty Trash** erase for good after a confirmation.
- **Add to Library.** Select a symbol, cabinet, CAD line, arc, circle or polyline, or text, then Library > Add
  Selection to Library. Symbols keep their size, flip and 3D model; a cabinet is kept whole (placing it makes a
  cabinet again); CAD pieces become one block and text becomes a text item. Library > Add Active Material to Library
  saves the Material Painter's material as a 12 in swatch. The Library Object Specification has Add to Library and
  Convert to Symbol (save, then use the saved item).
- **Convert to Symbol.** Select 3D solids (3D Solid flyout: boxes, polyline solids, cylinders, cones, spheres,
  pyramids) and choose Library > Convert to Symbol (or the Edit toolbar button). The solids' meshes become the 3D model
  of one new User Catalog symbol in `User > 3D Models` ("Solid Symbol n"), its plan drawing is the model seen from
  above, its size the solids' bounding box and its elevation the lowest solid's. The solids are replaced by a
  placed copy of the symbol, all as one undo step ("Convert to Symbol"). A selection of flat faces is refused.
- **Replace From Library.** Pick an item in the Library Browser, select the plan objects to replace and choose
  Library > Replace From Library (the Edit toolbar button, or **Replace Selected With This** in a row's menu).
  Every selected symbol takes the item and keeps its position, angle, reflect, label, layer and elevation; the size
  stays too unless "Replace keeps size" is off, when the item's own size is used. One undo step for the whole
  selection. A single selected cabinet or electrical device is swapped as in 6.5.
- **Object Information.** Open Object on a user item (or Edit in the preview pane): name, keywords (comma
  separated; the search finds them), category folder, type, style, manufacturer, width, depth, height, elevation,
  placement, whether it turns to face a wall, default layer, the 2D symbol (keep, draw from the 3D model, plain
  rectangle) and the 3D rotation and origin offset.
- **Import 3D Model.** Library > Import 3D Model (OBJ, glTF) picks a `.obj`, `.gltf` or `.glb`. The window shows
  the triangle count and size and asks for the file's units (inches, feet, millimeters, centimeters, meters; glTF
  defaults to meters), the up axis (Y or Z), the folder and the placement. The plan symbol is drawn from the model
  seen from above. OBJ colors come from a `.mtl` beside the file; glTF reads scenes, node transforms, triangle
  meshes and base colors, not textures, sparse accessors or Draco compression.
- **Export Library / Import Library.** Library > Export Library (Plan Studio JSON) writes one `plan-studio-library.json`
  with your User Catalog items only, the folders and favorites, and the 3D models (`.psm`, base 64). It is Plan
  Studio's own format and Chief Architect cannot open it; Plan Studio never writes a `.calib` or `.calibz` (a name
  with those extensions is refused). Library > Import Library reads that JSON back (items with the same id are
  replaced; the older stored-zip export still imports). Pointed at a Chief `.calib` or `.calibz`, Import Library
  adds it to the browser **read-only and in place**: the file is not copied or converted, only its path is kept (the
  `library_imports` list in `~/.plan-studio/settings.json`), and the catalog appears under the User Catalog node
  after a rescan (6.6, chapter 12.7). Remove it by deleting the path from that list.
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
| Drop from the Library Browser | Drag a row onto the plan: the item is placed where you let go, with the same rules as a click. |
| Edit toolbar | Open Object, Delete Objects, Copy, Paste in Place and **Replace From Library**: pick an item in the Library Browser first, then press the button to swap every selected symbol for it, in one undo step ("Pick an item in the Library Browser, then choose Replace From Library" if none is active; 6.4a). A selected cabinet is replaced by a saved cabinet item (it keeps its sink and cooktop cutouts, appliance, moldings and label); a selected electrical device becomes the active library symbol. |

### Dialog: Library Object Specification

Double-click a placed symbol (or `Enter`) for the Library Object Specification of that copy. In the Library Browser,
**Open Object** (a row's menu, or the **Open** button of the preview pane) opens the same dialog for the library
item itself: it then edits the item's defaults on a stand-in copy, and OK saves them into a User Catalog item
(size, elevation, layer, label, schedule, options and Reflect; the drawing is stretched to a new width and depth,
and later copies start from these values). Built-in and Chief objects open read-only: OK is dimmed and the General
tab offers Add to Library, which saves an editable copy.

| Tab | Fields |
|---|---|
| General | Symbol (Name, Source catalog, Category, Placement; Replace From Library, Add to Library, Convert to Symbol), Size (Width, Depth, Height, **Keep aspect**, Reset to Library Size), Position (Elevation from floor, Position X and Y at the back center, Angle; for Open Object, Defaults: Elevation and **Rotation (3D model)** instead), **Reflect** (mirror left to right) |
| Options | Reflect, and the library-specific choices of the object's type: Doors have **Door style** and **Hardware**, Cabinets **Cabinet door** (the Cabinet Doors styles of the library) and **Hardware**, Windows **Hardware**; each is a drop-down of the library's matching objects and Default. The choice is stored with the object (`PlacedSymbol::options`); the 3D view does not use it yet |
| 3D | (disabled) |
| Materials | The material the symbol is painted with |
| Layer | The layer |
| Label | Label text |
| Components | (disabled) |
| Object Information | Name, Type, Browser type, Category, Source, Size, Placement, Default layer, Elevation, Manufacturer, Style, whether it has a 3D model, and its keywords |
| Schedule | **Include in schedules**, the **Schedule** it is filed under (by library category, Fixture, Furniture, Plant or Appliance), Mark, Manufacturer, Model and Note (`PlacedSymbol::schedule`; the schedule builder still groups by library category) |

**Keep aspect** scales the other two sizes with the one you type into. In the Edit toolbar, **Convert to Symbol**
appears when 3D solids are selected (6.4a).

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

Under the check box are four collapsed folders (with folder icons), in Chief's order: **Chief Architect Core Catalogs**, **Bonus
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
chapter 12.7. Library > Import Library takes a `.calib` or `.calibz` too: the file is added to this list as a
read-only catalog under the User Catalog node, read in place (6.4a).

The Chief importer (opening a Chief `.plan` file) links each library object of a Chief plan to a catalog item: first
in the installed Chief catalogs, by the item's catalog GUID and then by name, and when they hold nothing, in Plan
Studio's own library: a built-in or User Catalog item with the same name (case and punctuation ignored, the
plan's category tags breaking ties, User items first), or one carrying the tag `guid:<catalog GUID>`. What nothing
matches stays a labelled box named `chief-plan.<name>`.

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
- Chief catalogs are read, never written: Chief objects cannot be added to the User catalog. Import Library reads a Chief `.calib` or `.calibz` in place instead of copying it into a Chief-style library, and Export Library writes Plan Studio's JSON, never a `.calib` (6.4a). Chief's Trash catalog (`Trash.calib`) is not shown; the Trash node is Plan Studio's own.
- The Library Object Specification's 3D and Components tabs are disabled; Options choices (door style, cabinet door, hardware) and Schedule settings are stored but not yet used by the 3D view or the schedule builder. A drop from the browser uses the camera without a rotated plan view, and Chief rows cannot be dragged onto the plan (click them).
- Imported and saved 3D models show in the preview pane; the 3D view of the plan draws them once `user::placed_meshes` is hooked into `view3d_panel.rs` (open).
- Search filters have no "objects already in the plan" mode.
- A custom face on the Sides and Back needs a rectangular cabinet. Of a library door or drawer style (a "Cabinet Doors" object) only the look
  (profile, glass) reaches 3D: its own geometry and Chief's Library... and Edit... buttons on the Door/Drawer tab are not built, and the Chief styles you scan with
  Load Chief Library Styles are forgotten when the install folder or the discovered library changes.
- Front pilasters and feet are built for rectangular cabinets; corner, blind and custom kinds ignore them. The Object Information fields (manufacturer, model, description,
  notes) have no schedule columns yet.
- The Select tool and a drag in the 3D view always bump (the Push mode belongs to the Cabinet tool's Edit toolbar), and plan boxes, Print Preview and the PDF do not yet draw a cabinet's Fill Style (it shows on screen).
