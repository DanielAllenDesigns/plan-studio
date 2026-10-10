# Chapter 4: Rooms and Floors

In Plan Studio you never draw a room. You draw walls, and every closed loop of
walls becomes a room automatically. This chapter explains how rooms are found,
named and measured, how to stack floors and foundations, and the two planning
helpers that work with rooms: the Space Planning Assistant and Plan Check (the check itself is chapter 18).

## 4.1 How rooms are detected

Rooms are derived data, recomputed after every edit. The detector (`plan_core::rooms`):

1. Reduces every wall that **defines rooms** to its centerline.
2. Splits the lines at every intersection and T-junction and snaps nearby points
   into a planar graph.
3. Traces each bounded face of that graph. Each face of at least 1 sq ft is a room.

```
   +-----------+-----------+          Two walls closing a loop = two rooms.
   |           |           |          A gap in the loop = no room and no area label.
   |  Room 1   |  Room 2   |          A door or window never breaks a room.
   |           |           |
   +-----------+-----------+
```

- A wall defines rooms unless its flags say otherwise: Invisible, No Room
  Definition and (in the model) Room Divider are handled by `WallFlags::defines_rooms`.
  The Invisible and No Room Definition checkboxes in the Wall Specification set those flags (chapter 2.9).
- A wall that dead-ends inside a room does not split it.
- Rooms are detected for the active floor only. Other floors never participate.
- A loop inside another loop is its own room; the enclosing room's area does not
  yet have a hole cut for it (planned).

### Three areas

Every room carries three areas:

| Area | Measured to | Used for |
|---|---|---|
| Centerline area | Wall centerlines | The outline drawn on the Rooms layer, and the number in the Properties panel's room list. |
| Interior Area | The interior wall surfaces (the inner polygon), plus the space inside bay, box and bow windows | The plan label, the Room Specification and the Room Schedule's Area column (QA-03, fixed in Round 8). |
| Standard Area | The outside surface (or the outside of the Main Layer) of exterior walls, the center of interior walls; no bay, box or bow windows; rounded to the nearest square foot in labels | The optional second readout, the Living Area (4.6) and a hidden Standard Area column of the Room Schedule. |

Interior dimensions in a label are the width and length of the interior bounding
rectangle, in the plan's dimension format, for example `12'-4" x 14'-0"`.

## 4.2 Selecting rooms and labels

With Select Objects, click on empty floor inside a room to select it (its outline
highlights). Double-click inside it, or press `Enter` with the room selected, to open
the Room Specification. `Esc` clears the selection.

Each room draws a label at its centroid on the Room Labels layer: the room name, then
(per the Label tab) the interior dimensions and interior area in sq ft. An unnamed
room is labelled "Room 1", "Room 2" and so on, top-left first. The room outline
is on the Rooms layer. Turn either layer off in the Active Layer Display Options.

**Drag a label** to move it: press on the label with Select Objects (this also selects
its room) and drag. The offset from the room's label point is stored on the room, so the
label keeps its place relative to the room when the plan moves; Undo puts it back, and
Reset Position on the Label tab sends it home.

**Nested rooms (R-11).** A closed loop of walls wholly inside another room (a closet
pod, a free-standing chimney box) is its own room, and the room around it gives up the
island: its areas (centerline, interior, standard) exclude it, the floor and ceiling
platforms in 3D have a hole under it, and a name anchored inside the island belongs to
the island. Clicking inside the island picks the island.

**The Exterior Room (Round 16).** Each structure (a group of rooms that share a wall or a
corner) on each floor has an Exterior Room. Click **just outside an exterior wall** with
Select Objects, or on the structure's **Living Area label**, to select it: a highlighted
band follows the outline and the status line says "Exterior Room". If a wall is picked
instead, press **Tab** (Select Next Object) until the Exterior Room is selected; Shift+Tab goes
back. Double-click it, or press `Enter`, to open the **Exterior Room Specification** (4.12).
The band has two grips at its right: dragging the top one up or down sets the level's
default ceiling height, the bottom one the thickness of the level's floor platform, which
sets its default floor height (Floor 1 and a layered floor structure have no bottom grip).
Walls at the old ceiling height and the floors above follow; one drag is one undo step.

A room is remembered by a point inside it, not by an index. Renaming survives
edits as long as the point stays inside the room; split a room with a new wall
and the name stays with the piece that holds the point.

## 4.3 Tools

### Floor tools (Build > Floor, row 2)

| Button | Hotkey | What it does |
|---|---|---|
| Build New Floor | `Shift+X` | Opens the Build New Floor dialog (4.5). |
| Insert New Floor | `Ctrl+Alt+Shift+Cmd+I` | Inserts an empty floor above the current one. A message says if the top is an attic. |
| Build Foundation | `Cmd+F` | Opens the Build Foundation dialog (4.5). |
| Delete Current Floor | `Ctrl+Alt+Shift+Cmd+J` | Asks for confirmation (Undo restores). Refuses to delete the only floor. |
| Delete Foundation | `Ctrl+Alt+Shift+Cmd+K` | Removes floor 0 of the foundation. "There is no foundation" if none. |
| Exchange With Floor Above | `Ctrl+Alt+Shift+Cmd+L` | Swaps the contents of this floor and the one above; both keep their numbers. |
| Exchange With Floor Below | `Ctrl+Alt+Shift+Cmd+M` | The same with the floor below. |
| Rebuild Walls/Floors/Ceilings | `F12` | Restacks the floors and recomputes derived geometry. |
| Floor Material Region | | Works: draw a polygon of floor finish (chapter 17). |
| Hole in Floor Platform, Hole in Ceiling Platform | | Work: draw a polygon, and the floor or ceiling platform is cut there in 3D (chapter 16). |

Each of these is one undo step, and none of them needs a canvas click.

### Row 1 floor controls

| Button | Hotkey | What it does |
|---|---|---|
| Down One Floor | `Ctrl+Z` | Moves the view to the floor below. |
| Floor number | | Shows the current floor. |
| Up One Floor | `Ctrl+A` | Moves the view to the floor above. |
| Floor Defaults | `Shift+Cmd+Y` (Chief's own key and Daniel's) | Opens Floor Defaults for the active floor (4.5); also Build > Floor > Floor Defaults. The chord is bound even without Daniel's hotkey file. |
| Reference Display | `F9` (view bar) | Draws the walls of the reference floor in gray, walls only and honoring the display of their layers: the floor below unless the Reference Display dialog chose another (4.5). Nothing when there is no such floor. A saved plan view that has it set turns it on. |
| Reference Display Options | (view bar) | Opens the Reference Display dialog; also Tools > Floor/Reference Display. |

Those two floor hotkeys are Daniel's Chief bindings (factory Chief uses `Shift+M`
and `Shift+N`). Switching floors keeps the zoom, pan and active tool, and clears
the selection. The Project Browser lists the floors, and clicking one switches to it.

### Floors in the model

- Floors are listed bottom to top and have a **kind**: Foundation, Normal or
  Attic. New Normal floors are named "1st Floor", "2nd Floor", "3rd Floor" and so on.
- A floor has a finished-floor elevation, a ceiling height (109 1/8" by default),
  and its own walls, openings, dimensions, CAD, symbols, cabinets, stairs, groups, roof
  records, electrical devices, framing and slab objects (the last four live in typed slots of the
  floor, chapter 12.2).
- The default floor platform thickness is 10 1/4" (`FLOOR_PLATFORM_THICKNESS`).
  Floor-to-floor rise is the ceiling height plus that platform.

## 4.4 Dialog: Room Specification

Open by double-clicking a room, or select the room and press `Enter`. The dialog
uses the shared frame with an interior plan preview and a live cross-section.

| Tab | Status |
|---|---|
| General | Works |
| Structure | Works |
| Deck | Works (the Deck Specification of a Deck room) |
| Moldings | Works (the Base, Chair Rail and Crown profile names are stored) |
| Wall Covering | Works (session only) |
| Layer | Works (stored: layer and Drawing Group; the plan still draws rooms on the Rooms layer) |
| Fill Style | Works (stored) |
| Materials | Works (finish names are stored) |
| Label | Works (the display switches are stored) |
| Components | Read-only summary |
| Object Information | Read-only |
| Schedule | Read-only |

OK writes the changes as one undo step. Fields stored with the plan are marked
below, and old files load with the defaults for every field they lack. The rest are
(session only), meaning kept in memory per room.

### General

- **Room Name** and **Room Type** (a drop-down of the template's room types: Attic,
  Balcony, Bath, Bedroom, Closet, Dining, Entry, Garage, Kitchen, Living, Master
  Bath, Porch, Utility and so on; edit the list in 4.10). Stored.
- **Function** is shown read-only with its category, for example "Garage (Hybrid)"; it
  comes from the room type and is a fixed set of properties (4.11). Choosing a room type
  sets the defaults of the Structure tab from its function and gives the room the type's
  own settings (4.10); every value stays editable afterwards:
  - **Garage**: the floor drops 24" (Floor Height offset -24"), on a 4" concrete slab with
    no floor finish.
  - **Deck**: no ceiling over the room; the floor is a deck platform (1 1/2" decking on 7 1/4"
    joists) with no finish. **Porch**: no ceiling, a 4" concrete slab, no finish.
  - **Open Below**: no floor under the room, and the ceiling of the room under it opens
    to it. The Attic room type has no floor platform either, and a **Courtyard** has neither a floor nor a ceiling (Round 13).
  - **Flat Roof**: no ceiling, and a membrane deck (1/2" membrane on 7 1/4" joists) as its floor structure.
  - **Balcony**, **Court** and **Slab** follow 4.11. Any other type goes back to the floor's own finish and platform.
- **Living Area**: Include in Total Living Area Calculation, Exclude, or Use Default
  (follows the room type). Stored. Garage, Deck, Porch and similar types are
  excluded by default.
- **Conditioned Room**: Conditioned, Unconditioned or Use Default. Stored.
- **Options**: the Roof Group number (0 is the default; rooms of another group are roofed
  as a separate building, chapter 8).
- **Room Information** reports the interior dimensions, Interior Area (with bay, box and bow
  windows), Standard Area, Centerline Area, perimeter, whether the room is in the Living
  Area now and why ("by its room type", "set for this room", "rough ceiling under 48 in"),
  and the Living Area of its structure.

### Structure

- **Absolute Elevations** (from zero, the top of Floor 1's subfloor): Floor Above (No Change
  when rooms of different heights lie above), Ceiling and Floor (editable), Floor Below and SWT
  Below (the top of the stem walls of the room below). **Relative Heights**: Rough Ceiling and
  Finished Ceiling (editable), Ceiling Below and, with Floor Supplied by the Foundation Room
  Below, Stem Wall Top to Ceiling and Floor to Stem Wall Top. The preview shows the cross
  section: the floors above and below, the platforms and the heights, with Floor and Ceiling
  callouts. **Floor Height** and **Ceiling Height** reach the 3D view (QA-02, fixed in Round 8; R-23, R-24, R-33): the 3D floor platform of a room is raised by its Floor Height offset, and its ceiling platform sits at its own Ceiling Height measured from that raised floor. A room with no named entry, or with no override, keeps the floor's ceiling height. Rooms that share the same levels share one platform; the 3D view rebuilds when you change them.
- **Rough Ceiling** for dropped ceilings (stored; equal to the finished ceiling plus its finish
  means none).
- **Ceiling**: Ceiling Over This Room, Roof Over This Room, Flat Roof Over This Room, **Flat
  Ceiling Over This Room** (off makes a cathedral ceiling that follows the roof above; the
  Turn Ceiling Off/On buttons set it too), **Shelf Ceiling** (no attic walls over the interior walls
  of the room) and **Use Soffit Surface for Ceiling**.
- **Floor**: Floor Under This Room, **Floor Supplied by the Foundation Room Below**, **Room
  Supplies Floor for the Room Above** (a garage on a slab), **Build Foundation Below**, **Raised
  Floor For Bump Out**, **Retain Floor/Ceiling Framing**, Framing Group (or Slab Pour Number with
  a monolithic slab) and **On Structure Resize** (Lock Floor Top or Bottom). These are stored; their
  effects on framing and foundations belong to those chapters.
- **Finish** thicknesses for floor and ceiling (stored finish names; the floor finish thickness reaches the 3D floor).
- **Platforms**: Floor Under This Room, Ceiling Over This Room (stored and honored by the 3D platforms: off removes that platform; turning the ceiling off also makes Build Roof add a vaulted ceiling plane over the room, chapter 8.2), **Roof Over This Room** (on by default; stored with the room) and **Flat Roof Over This Room** (stored; available only while Roof Over This Room is on). With Roof Over This Room off, **Build Roof leaves the room out**: its exterior walls stop shaping the roof, a partition between it and a roofed room becomes the roof's edge, and a roofless room inside one plane gets a hole in that plane (a courtyard, an open deck; chapter 8.1). With Flat Roof Over This Room on, Build Roof puts a level roof plane at the room's ceiling instead of the pitched roof; since Round 13 that plane **overhangs** on the room's exterior edges (half the wall plus the wall's or the roof settings' overhang) and not on its partition edges (chapter 8.1). When a roofless room lies across a ridge, hip or valley, Build Roof cuts a hole piece in each plane (chapter 8.2). Auto Rebuild Roofs reruns when either changes.
- **Floor Structure Define...** and **Ceiling Structure Define...** (R-28, R-29) edit the room's
  layer stack (material and thickness per layer, top first; Add Layer, Remove, move up/down, Use
  Default). The layers are stored with the room and their total is the thickness of the 3D platform;
  an empty stack keeps the floor's default 1" platform.
- **Stem Wall** with its height. Stored (a room with a stem wall keeps its height; turning it off clears it). In 3D a room with a stem wall height, and a **Garage** whose floor is dropped below the house floor, get concrete **stem walls** under their exterior walls: from the underside of the floor platform up to the floor level (a dropped floor with a stem wall height uses the deeper of the two), stopping at garage doors, in the foundation wall type's thickness (chapter 10.1).
- The preview draws the room cross-section with the CEILING and FLOOR dimensions.

### Moldings, Wall Covering, Fill Style, Materials

Moldings: Base and Crown profile names; a profile you name is stored on the room with a default height
(5 1/4" base, 3 1/2" crown), and clearing the name removes it. Wall Covering: Interior Wall Covering (session
only). Fill Style: Pattern (None, Solid, Hatch, Cross Hatch, Grid) and Color; stored on the room and drawn in
plan. Materials: Floor Finish and Ceiling Finish (stored by name). Other material surfaces are (planned).

### Label

Display in All Views: Interior Dimensions, Interior Area, Standard Area, Display in
Plan View. Appearance: Text Style (disabled). The defaults show dimensions and
interior area. The label options are stored on the room.

**Label Text** (R-47) is an optional template that replaces those lines: `<name>`,
`<type>`, `<area>`, `<std_area>`, `<cl_area>`, `<dims>`, `<ceiling>`, `<floor>` and
`<perimeter>` are replaced by the room's values, `\n` starts a new line, and other text
stays as typed. Chief's name-value pairs work too: `%dimensions%` (interior dimensions),
`%internal_area%` (Interior Area, with bay, box and bow windows) and `%standard_area%`
(Standard Area), both rounded to the nearest square foot. Empty keeps the checkboxes' lines.

### Components, Object Information, Schedule

Components lists the floor and ceiling finish layers. Object Information and
Schedule show the room's Room Finish Schedule row. Editing prices, accounting codes
and custom fields is (planned).

## 4.5 Dialog: Build New Floor, Build Foundation, Delete Current Floor

### Build New Floor

Builds from the current floor (the top floor when the current one is the foundation):

- **Plan**: derive from the exterior walls (with their doors and windows), derive from all the
  walls (partitions too), or make a blank plan. Derived floors can also copy the room names and
  types, and the slab, pad and pier data.
- **Place**: above or below the current floor (nothing goes above the attic or below the
  foundation).
- **Heights**: from the Floor Defaults (ceiling height and floor settings of the plan defaults,
  see below) or the same as the current floor.
- **Move Highest Floor's Roof Up**: moves the roof planes built on the highest floor up with the
  new floor. Only available when roof planes are built there and Auto Rebuild Roofs is off.
- **Step floor/ceiling elevations to match existing floor**: gives the rooms of the new floor the
  floor heights (above) or ceiling heights (below) that keep the ceilings and floors of the
  existing floor where they are. Without it, a floor built above resets the ceiling height of
  rooms with a raised or lowered floor.
- **Also build a foundation**: shown when the plan has none; takes the Foundation Defaults and
  Build Foundation panels (chapter 16.5).

OK creates the floor and switches to it, as one undo step, and opens the Floor Defaults of the
new floor. A plan has at most 30 living floors. **Insert New Floor Below** opens the same dialog as
"Insert New Floor" with Place set to below the current floor, derived from its walls; **Insert New
Floor** (above) adds an empty floor.

### Floor Defaults

Toolbar button, Build > Floor > Floor Defaults, and Edit > Default Settings > Floors and Rooms >
Floor Defaults (for floors built from now on). Fields: Ceiling Height, Floor Structure and Ceiling
Structure thickness (the floor-to-floor rise is the ceiling height plus the lower floor's ceiling
structure plus this floor's floor structure; the dialog shows the Floor Height), floor and ceiling
finish thickness, the Default Room Type and the floor and ceiling materials a new room starts
with. New rooms in the Room Specification start from them (a named default type also brings its
function defaults). OK moves the floors above by the change, and walls that stood at the old
ceiling height follow the new one. On a floor, the check box "Use these for floors built from now
on" also sets the plan defaults.

### Reference Display

Tools > Floor/Reference Display, or the options button next to the Reference Display toggle: show
the reference floor, which floor it is (below, above or any other floor), which layer set decides
what of it is drawn, and its color. The choices are kept for the session; the floor is also written
to the active saved plan view.

What of the reference floor shows is decided layer by layer: the **Ref** column of Active Layer Display Options (chapter 5.5) has a box for each layer, on by default; a layer with its Ref box off does not draw on a reference floor and does not snap.

**Snapping to the reference floor.** While Reference Display is on, the pointer snaps to the ends and the crossings of the reference floor's wall centerlines (as Endpoint and Intersection snaps, subject to Edit > Snap Settings and Alt), so a second-floor wall lands exactly over the one below. The active floor's own endpoints and intersections come first; a reference end or crossing within the snap distance then beats the weaker snaps (midpoint, on object, angle, grid). Only walls are snapped to; the other reference objects are not.

### Build Foundation

The Foundation and Options panels, the Foundation Defaults and everything a build makes are in
chapter 16.5. A new foundation becomes floor 0 and the active floor index moves up by one so you
stay on the same floor; Build Foundation on a foundation that exists rebuilds it in place.
Foundation walls use the `Foundation-8` wall type (8" thick). Floor 0 cannot be deleted while
Auto Rebuild Foundation is on. Rooms cannot be created on the Attic floor, which warns when walls
or objects are drawn on it.

### Delete Current Floor

A confirmation: "Delete 2nd Floor and everything on it? Undo brings it back."

## 4.6 Total Living Area

Each structure on each floor has a **Living Area label**, centered under it on the Room
Labels layer ("Living Area: 1,235 sq ft") and shown with the Exterior Room. It is the sum of
the **Standard Areas** of the rooms that count, measured to the center of interior walls and to
the outside surface of exterior walls (or to the outside of their Main Layer: Edit > Default
Settings > General Plan Defaults > **Living Area to**), rounded to the nearest square foot,
without bay, box and bow windows. A structure with no counted room has no label. **Show Living
Area Label** in General Plan Defaults hides all of them without touching room labels. The label
is not an object you can delete.

A room counts when:

- its Living Area setting says Include (always) or Exclude (never); else
- its Room Type is in the Living Area (interior rooms are, exterior and hybrid ones are not), and
- its rough ceiling is **48 in or more**; a lower one (a crawl space, a low attic) is out. A
  basement of 48 in or more counts.

The total across all floors is the sum of the structures' rounded numbers; the status line
shows it after a Room Specification (and per floor when there are several).

## 4.7 Tools > Space Planning > Space Planning Assistant

An answer-a-few-questions layout generator, modelled on Chief's Space Planning
Assistant. Open it from Tools > Space Planning > Space Planning Assistant....

1. **Answer the questionnaire**: Bedrooms, Baths (half steps), Garage bays, Stories,
   room sizes (Living room, Kitchen, Dining, Master bedroom, Other bedrooms, in
   sq ft), and check boxes Office, Laundry, Mudroom, Pantry, Covered porch, Deck.
   The default is 3 bed, 2.5 bath, 2-car garage, 1 story, with a laundry room. The
   window shows the expected total area.
2. **Generate** makes named, colored boxes on the canvas, auto-arranged by a
   greedy affinity packer (kitchen next to dining, bedrooms grouped, and so on).
3. **Drag the boxes** with Select Objects. Boxes snap to a 6" grid and to
   neighbors' edges and corners and never overlap. A message lists problems:
   overlaps, floating rooms, a bath without a hall or bedroom, a garage not touching the house.
4. **Build House** converts the boxes into walls (a shared edge becomes one interior
   wall), doors, windows and room names, as one undo step. Deck and Porch boxes get no walls.
5. **Clear Boxes** discards the boxes.

## 4.8 Tools > Checks

**Plan Check** applies 52 rules based on the 2021 International Residential Code (IRC), with a few NEC and NKBA guidelines, to the active floor: room size, bedroom egress, ventilation and light,
door widths and swings, hallways, stairs and guards, bath and kitchen clearances, the garage, roof slope, smoke and CO alarms, GFCI and receptacle spacing, headers and joists, wall and opening geometry. The
window steps through the findings with Previous, Next, Zoom to and Ignore; **Settings...** holds the IRC 2021 preset, 32 editable limits and a tick box for every rule; the report is written as Markdown, as a PDF or as a
page of the layout. Chapter 18 describes all of it.

- **Plan Check** runs all the rules that are on. **Door/Window Check** runs only the two opening rules (door widths and opening geometry).
- Room types come from the room names. Rooms are measured to wall centerlines.
- **Plan Footprint** (Tools > Checks > Plan Footprint) traces the outer boundary of the
  rooms and adds it to the plan as a closed CAD polyline with an area note.

## 4.9 Known differences from Chief

- Rooms are traced from centerlines and offset inward for the interior polygon.
- Room Specification values beyond the name, type, living-area flag, heights, finishes, conditioned
  setting, stem wall, base, chair rail and crown moldings, fill and label options, Roof Over This Room, Flat Roof Over This Room, Flat Ceiling Over This Room, Roof Group and the Structure and Layer switches (all stored with the plan) are kept
  for the session only: Wall Covering and the finish thicknesses.
- The Exterior Room is selected in plan view only; Chief also selects it in 3D views and drags 3D wall handles. The plan view's
  grips do the same job (chapter 4.2). The Layer panel is stored but the plan draws every room on the Rooms layer.
- Attic floors from Build Roof are (planned). Floor Material Region is in chapter 17.
- Only the walls of a Reference Display floor are snappable (ends and crossings), and Open Below cuts only the ceiling of a room
  whose outline lies wholly inside (or exactly covers) a room below. The stem wall of a garage or of a room with a Stem Wall height is a plain concrete wall in 3D only (it is not a wall of the plan, and it does not appear in the plan view).
- Slabs, slab holes, pads, piers and the holes in the floor and ceiling platforms are objects of their
  own, not part of Build Foundation; see chapter 16.

## 4.10 Room Types (Edit > Default Settings)

Edit > Default Settings... > Floors and Rooms > **Room Types** (double-click it or press Edit) opens the
list behind the Room Type drop-down in the Room Specification.

- The table shows **Name**, **Function**, **Living Area** (Yes or No), **Conditioned** (Yes or No) and **In Use** for every type
  in the template (about 49 in Daniel's). Click a row to select it; Ctrl or Cmd toggles a row, Shift
  selects a range. **Select All** and **Clear All** select and clear the rows.
- **Add** appends a Standard room type named "New Room Type" (made unique) and opens it for editing.
  **Edit...** (or double-click a row) opens **Room Type Defaults** for the selected row (**Multiple Room Type
  Defaults** when several are selected). **Rename...** asks for a new name. **Copy...** makes a new type
  from the selected one. **Delete** removes the selected rows.
- **Room Type Defaults** has the panels of the Room Specification that a type hands its rooms: **General** (name,
  Function, Include in Living Area, Conditioned, default floor finish; choosing a function resets the last two to
  that function's defaults), **Structure** (Floor and Ceiling Structure and Finish, with Use Default), **Deck**
  (planking, framing, stairs), **Moldings**, **Layer** (layer and Drawing Group), **Fill Style** and **Label**
  (what the label shows and a macro template). Giving a room the type overwrites the room's settings with these.
  Multiple Room Type Defaults changes only what you set (a setting left on No Change keeps each type's own):
  function, Living Area, Conditioned, moldings, layer and fill.
- Names must be filled in and unique ("A name is required", "That name is already used"). "Unspecified", the
  type rooms without a type use, cannot be deleted.
- OK applies the draft; Cancel or Escape drops it. The new list becomes the defaults' room types (save them to
  your template with File > Templates > Save Current Defaults as My Template...). Renaming a type also renames
  it on the rooms of the open plan that used it, as one undo step ("Rename Room Types"), so those rooms keep
  their type.
- Function-driven behavior is described in 4.11.

## 4.11 Room functions

A **Room Function** is a fixed set of properties; a Room Type names one and can only change its own
settings (name, Living Area, Conditioned, platforms). There are three categories:

| Category | Functions | Living and Conditioned | Ceiling and roof |
|---|---|---|---|
| Interior | Standard, Utility | In both | Flat ceiling, roof over |
| Exterior | Balcony, Court, Deck | In neither | No ceiling, no roof, no foundation; doors and windows face out |
| Hybrid | Attic, Garage, Open Below, Porch, Slab | Out of the Living Area; Open Below is conditioned | Garage, Slab and Porch keep a ceiling and roof; an Attic has no floor, ceiling or roof; Open Below has no floor |

- **Floors and foundations.** Garage and Slab floors drop to Floor 0 on a concrete slab (a Slab as thick as
  the Foundation Defaults slab); a Deck or Balcony floor is decking on joists; a stairwell or crawl space
  is an Open Below room. **Basement and Crawl Space are not functions**: a Basement is an interior room that counts
  in the Living Area from a rough ceiling of 48 in.
- **Doors and windows.** Between an exterior and an interior room a window faces out and a hinged or sliding door takes
  the exterior defaults.
- **Electrical.** Fixtures on the wall of an exterior room are weatherproof; Auto Place Outlets skips exterior rooms,
  Porches and Open Below rooms, places fewer in other hybrid rooms, and puts GFCI outlets over base cabinets in kitchens
  and baths (kitchens also get standard-height outlets).
- **Plan Check** applies the habitable-room rules to interior rooms only.

## 4.12 Exterior Room Specification

Select the Exterior Room (4.2) and press Open Object, or double-click it. **General** reports the floor, the rooms
in the structure, its exterior walls, footprint and Living Area, and edits the level's default ceiling height and floor
platform thickness (the grips' values). **Wall Covering** puts a covering, wainscot, chair rail, base and crown on the
outside face of every exterior wall of the structure. **Materials** sets the Exterior Wall Surface material and color. OK
is one undo step.

## 4.13 The room Edit toolbar

Select a room and the Edit toolbar offers:

- **Calculate Materials in Room**: the Materials List of the room's contents (not its walls).
- **Turn Ceiling Off / On**: sets Flat Ceiling Over This Room (a cathedral ceiling follows the roof).
- **Make Room Polyline**, **Make Standard Area Polyline**: static closed CAD polylines on the current CAD layer along the
  room's surfaces and the extent of its Standard Area. They are not linked to the room afterwards.
- **Expand Room Polyline**: when the room has invisible walls or railings around it, temporarily selects the enlarged room
  that ignores them; the polyline buttons then use it. Any other selection ends it.
- **Create Schedule from Room**: asks for the schedule type, then click to place a schedule that lists only the objects in the room.
- **Create Room Elevation Views**: the four interior elevations of the room (running it again updates them).
- **Auto Room Dimensions**: interior dimension strings along each wall of the room (running it again replaces them).

The Exterior Room offers **Make Room Polyline** (a polyline around its exterior walls) and **Make Living Area Polyline** (the
exact extent of the Living Area). Each button is one undo step.
