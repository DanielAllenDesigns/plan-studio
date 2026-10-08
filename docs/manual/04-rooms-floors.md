# Chapter 4: Rooms and Floors

In Plan Studio you never draw a room. You draw walls, and every closed loop of
walls becomes a room automatically. This chapter explains how rooms are found,
named and measured, how to stack floors and foundations, and the two planning
helpers that work with rooms: the Space Planning Assistant and Plan Check.

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
| Interior Area | The interior wall surfaces (the inner polygon) | The plan label, the Room Specification, schedules and Total Living Area. |
| Standard Area | Outside of exterior walls, center of shared walls | The optional second readout. |

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
| Floor Material Region | | (planned) |
| Hole in Floor Platform, Hole in Ceiling Platform | | Work: draw a polygon, and the floor or ceiling platform is cut there in 3D (chapter 16). |

Each of these is one undo step, and none of them needs a canvas click.

### Row 1 floor controls

| Button | Hotkey | What it does |
|---|---|---|
| Down One Floor | `Ctrl+Z` | Moves the view to the floor below. |
| Floor number | | Shows the current floor. |
| Up One Floor | `Ctrl+A` | Moves the view to the floor above. |
| Floor Defaults | `Shift+Cmd+Y` | (planned) |
| Reference Display | `F9` (view bar) | Draws the walls of the floor below in gray, walls only and honoring their layers' display (nothing on the lowest floor). A saved plan view that has it set turns it on. |

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
| Deck | (disabled) |
| Moldings | Works (the Base and Crown profile names are stored) |
| Wall Covering | Works (session only) |
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
- **Function** (Standard, Living, Utility, Deck, Garage, Porch, Open Below ...) is
  shown read-only; it comes from the room type. Function-specific behavior such as a
  dropped garage floor or Open Below cut-outs is (planned).
- **Living Area**: Include in Total Living Area Calculation, Exclude, or Use Default
  (follows the room type). Stored. Garage, Deck, Porch and similar types are
  excluded by default.
- **Conditioned Room**: Conditioned, Unconditioned or Use Default. Stored.
- The dialog also shows the room's Interior Area, Standard Area, perimeter and
  the plan's Total Living Area across all floors.

### Structure

- **Floor Height** and **Ceiling Height**, each Absolute or Relative (the absolute/
  relative toggle is session only; the offset values are stored).
- **Rough Ceiling Height** for dropped ceilings (stored).
- **Finish** thicknesses for floor and ceiling (stored finish names; thickness session only).
- **Platforms**: Floor Under This Room, Ceiling Over This Room (stored), Roof Over This Room (session only).
- **Stem Wall** with its height. Stored (a room with a stem wall keeps its height; turning it off clears it).
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

### Components, Object Information, Schedule

Components lists the floor and ceiling finish layers. Object Information and
Schedule show the room's Room Finish Schedule row. Editing prices, accounting codes
and custom fields is (planned).

## 4.5 Dialog: Build New Floor, Build Foundation, Delete Current Floor

### Build New Floor

Two options: **Derive new 2nd Floor plan from the 1st Floor plan** (copies the exterior
walls with their doors and windows) or **Make new blank plan**. OK creates the
floor above the current one and switches to it.

### Build Foundation

Foundation Type: **Walls with Footings**, **Monolithic Slab**, **Piers**. For Walls with
Footings, enter **Stem Wall Height** (from the Foundation Wall default, 48") and **Minimum
Stem Wall** (12"); OK is blocked if the stem height is zero. A "Build Garage Floor..."
check box is shown, but the model does not store it. A new foundation becomes floor
0 and the active floor index moves up by one so you stay on the same floor.
Foundation walls use the `Foundation-8` wall type (8" thick).

### Delete Current Floor

A confirmation: "Delete 2nd Floor and everything on it? Undo brings it back."

## 4.6 Total Living Area

Total Living Area is the sum of the Interior Areas of every room, across all floors,
whose Living Area setting resolves to Included. The Room Specification shows the
total. A standalone readout (plan information, schedules footer) is (planned).

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

Plan Check applies twelve rules based on the 2021 International Residential Code
(IRC) to the active floor: room size, bedroom egress, ventilation, door widths
and swings, hallways, stairs, garage, wall geometry, opening geometry, room access
and natural light. Limits live in `CheckOptions` and default to the 2021 IRC;
there is no settings dialog for them yet.

- **Plan Check** runs all rules. **Door/Window Check** runs only the opening rules.
- The Check window shows one finding at a time: severity (error, warning, info), the
  rule and its IRC section, what is wrong, and a suggested fix.
- **Previous** and **Next** step through findings. **Zoom to** centers the view on
  the finding and selects its object. **Check Again** re-runs after you fix something.
  **Save Report...** writes a Markdown report grouped by severity.
- Room types come from the room names. Rooms are measured to wall centerlines.
- **Plan Footprint** (Tools > Checks > Plan Footprint) traces the outer boundary of the
  rooms and adds it to the plan as a closed CAD polyline with an area note.

## 4.9 Known differences from Chief

- Rooms are traced from centerlines and offset inward for the interior polygon.
- Nested rooms do not yet cut a hole in the enclosing room's area.
- Room Specification values beyond the name, type, living-area flag, heights, finishes, conditioned
  setting, stem wall, base and crown moldings, fill and label options (all stored with the plan) are kept
  for the session only: Wall Covering, Roof Over This Room, the Absolute/Relative toggles and the finish
  thicknesses.
- Floor Defaults, Floor Material Region and Attic floors from Build Roof are (planned).
- Slabs, slab holes, pads, piers and the holes in the floor and ceiling platforms are objects of their
  own, not part of Build Foundation; see chapter 16.

## 4.10 Room Types (Edit > Default Settings)

Edit > Default Settings... > Floors and Rooms > **Room Types** (double-click it or press Edit) opens the
list behind the Room Type drop-down in the Room Specification.

- The table shows **Name**, **Function**, **Living Area** (Yes or No) and **Conditioned** (Yes or No) for every type
  in the template (about 49 in Daniel's).
- **Add** appends a Standard room type named "New Room Type" (made unique) and opens it for editing.
  **Edit...** (or double-click a row) opens **Room Type - <name>**: Function (Standard, Living, Utility, Deck, Garage,
  Porch, Open Below), Include in Living Area, Conditioned and Default Floor Finish. **Rename...** asks for a new
  name. **Delete** removes the selected type.
- Names must be filled in and unique ("A name is required", "That name is already used"). "Unspecified", the
  type rooms without a type use, cannot be deleted.
- OK applies the draft; Cancel or Escape drops it. The new list becomes the defaults' room types (save them to
  your template with File > Templates > Save Current Defaults as My Template...). Renaming a type also renames
  it on the rooms of the open plan that used it, as one undo step ("Rename Room Types"), so those rooms keep
  their type.
- Function-driven behavior is still (planned) (see 4.4).
