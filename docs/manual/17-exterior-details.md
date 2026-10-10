# Chapter 17: Exterior Details: Trim, Material Regions, Decks and 3D Solids

This chapter covers the objects that finish a model: corner boards and quoins on the corners, moldings, material
regions on floors and walls, wall hatching, polygon-shaped decks, and the 3D solids (box, cylinder, sphere, cone,
pyramid, polyline solid and face). They are drawn by one tool with a flavor per button, share one storage slot and
one family of specification dialogs, and are picked, reshaped and deleted by Select Objects like any other object.

## 17.1 How the objects work

Every object belongs to one floor and is stored in that floor's own `details` slot (a typed field of the plan file,
`Floor.details`; chapter 12.2), so it saves, loads and undoes with the plan. Each kind draws on its own layer, which is added to
the plan the first time it is needed; turning the layer off hides the objects.

| Object | What it is | Layer | Defaults |
|---|---|---|---|
| **Corner board** | Two boards on the outer faces of an exterior corner and the square where they meet | `Corner Trim` | 3 1/2" wide, 3/4" thick, bottom 0, 96" high (the corner's wall height when auto placed), Painted White Trim |
| **Quoin** | The same "L", stacked in courses of long and short blocks | `Corner Trim` | Width 16", Quoin Height 8", Thickness 1 1/2", no gap, Staggered, stack 96", Stone Veneer - Fieldstone |
| **Molding** (line or polyline) | One or more profiles swept along a 3D line (chapter 17.7) | `Moldings` | The active library profile (the square profile until you pick another), bottom height 0 (a crown starts at the ceiling), Painted White Trim |
| **Floor material region** | A polygon of floor finish | `Material Regions` | 1/4" thick, Ceramic Tile 12x12 |
| **Wall material region** | A rectangle on one face of a wall | `Material Regions` | 1/4" thick, bottom 0, top the wall height |
| **Wall hatching** | A hatch pattern drawn on a wall in the plan | `Material Regions` | Scale 1, angle 45 degrees |
| **Polygon deck** | A polygon of decking boards, optionally with a railing | `Decks` | Top at 0, 1 1/2" boards, Oak Flooring, no railing |
| **3D solid** | A box, cylinder, sphere, cone, polyline solid, pyramid or face | `3D Solids` | 48" high until you change it in the dialog that opens after drawing, Concrete |

Lengths are inches, shown as feet and inches. Heights ("Bottom height", "Top height") are measured from the floor the object is on.

## 17.2 Tools

All the tools are one tool object with a flavor per button. The Trim flyout (row 2, Build > Trim) holds the first six; Wall Hatching, Wall Material Region and Slab Footing are on the
Straight Wall flyout, Floor Material Region on the Floor flyout, Polygon Shaped Deck at the foot of the Railing and Deck flyout, and the 3D Solid flyout holds the solids. None of them has a hotkey.

| Button | Flyout | Gesture |
|---|---|---|
| Corner Boards | Trim | Click an exterior corner to put boards on it. |
| Auto Place Corner Boards | Trim | One click puts boards on every convex exterior corner of the floor. |
| Quoins | Trim | Click an exterior corner. |
| Auto Place Quoins | Trim | One click quoins every convex exterior corner. |
| Molding Line | Trim | Click two points, or press and drag. Its start and end heights are set in the Selected Line panel (17.7). |
| Molding Polyline | Trim | A click per corner; a double-click or `Enter` finishes; press and drag draws a closed rectangle, clockwise, so the profile lies inside. |
| Replace Moldings | Trim | Click a molding: it takes the active library profile. |
| Floor Material Region | Floor | A polygon (below). |
| Wall Material Region | Straight Wall | Click a wall, or press on it and drag along it for part of its length; a dialog sets the heights and the face. |
| Wall Hatching | Straight Wall | Click a wall; a dialog picks the pattern. |
| Slab Footing | Straight Wall | A polygon; makes a foundation slab with a footing (chapter 16). |
| Polygon Shaped Deck | Railing and Deck | A polygon. |
| 3D Solid | 3D Solid | A polygon makes a prism (drag a rectangle for a box). |
| Face | 3D Solid | A polygon makes a flat, two-sided face. |
| Pyramid | 3D Solid | A polygon makes a pyramid. |
| Cone, Cylinder, Sphere | 3D Solid | Click the center, then the radius (a second click, or drag out from the center). |
| 3D Solid Feature | 3D Solid | Places the active library item as a solid (chapter 6.7). |

**Polygons** work like the slab tools (chapter 16.2): click the corners; a double-click, `Enter` or a click on the first corner closes the shape; press at the first point and drag to draw a rectangle; `Backspace` drops the last corner;
`Esc` cancels. The area must be at least 1 sq in. A new solid, wall region or hatch opens its dialog at once so the height, the range or the pattern can be set right away.

With a details tool active, `Cmd` (`Ctrl` off macOS) plus a click picks an existing object, `Cmd` plus a drag moves it, `Delete` or `Backspace` deletes the selected or hovered one, and a double-click on one outside a drawing opens its
dialog. Every change is one undo step.

## 17.3 Selecting and editing

Select Objects picks every kind of detail, in an order that favors what you meant: corner boards, quoins, moldings and solids first (above the walls); wall regions and hatching just after their wall; floor regions and decks below the rooms.
Click, `Shift`+click, `Tab` and a marquee all work; `Delete` removes the selection (one undo step); double-click or `Enter` opens the dialog; dragging the body moves it.

Handles:

| Object | Handles |
|---|---|
| Floor region, deck, polyline solid, pyramid, face | A corner handle on every vertex ("Reshape Detail"). |
| Molding line | An end handle at each end. |
| Molding polyline | A corner handle on every point. |
| Box, cylinder, cone, sphere, corner board, quoin, wall region, wall hatch | None: they move by their body. |

**Details follow walls.** Corner boards and quoins follow their walls when you drag, stretch or nudge a wall with Select Objects: the apex slides to where the two outer faces meet and the boards turn with the walls.
A wall region or hatch belongs to its wall and is measured along it, so it follows too, and deleting the wall deletes its regions and hatches in the same undo step. Not covered: a wall edited through the Wall Specification dialog or
the wall tool, or removed by Delete All, does not move its trim; moldings never follow walls.

## 17.4 The specification dialogs

Each object has its own Specification dialog on the shared frame (Corner Board, Quoin, Molding, Material Region, Wall Hatching, Deck and 3D Solid Specification). OK writes the changes as one undo step.

| Tab | Holds |
|---|---|
| General | The size fields of the object (below). |
| Materials | The **Material**, from a short built-in list (Painted White Trim, Oak Flooring, Wood, Concrete, Stone, Metal, Glass, Ceramic Tile 12x12) and the library's own materials. Wall hatching has no Materials tab. |
| Line Style | **Own color**, **Own line weight** (5 to 200 hundredths of a millimeter) and **Line style** (As the layer, Solid, Dashed, Dotted, Dash-dot): the detail's own look in the plan when it differs from its layer's. Nothing set means the layer's color, weight and dash. |
| Layer | The layer the object is drawn on. |

General, by object:

- **Corner Board**: Width, Thickness, **Set Bottom** (Bottom height) and **Set Top** (Top height), **Recessed To Sheathing Layer**, and the lumber in cubic feet. With Set Top and Set Bottom off the board runs from the floor to the top plate of its walls and follows them when Select Objects moves or resizes them (17.3).
  Its tabs are General, Line Style, Materials, Components and Layer.
- **Quoin**: Width, Thickness, **Quoin Height**, **Quoin Gap**, **Style** (Uniform, Staggered or Mirrored), **Swap Start Block**, Set Bottom and Set Top, Recessed To Sheathing Layer, and the number of courses and blocks. Same tabs as the corner board.
- **Molding**: see 17.7. Its tabs are General, Moldings, Selected Line, Line Style, Fill Style (not built yet, shown dimmed), Materials, Label, Components and Layer.
- **Material Region**: on a floor, **Cut finish layers** (the region sits flush with the floor instead of on it); on a wall, which face it is on (picked by the side you clicked), **From** and **To** along the wall, and **Bottom** and **Top** heights; both: **Thickness**.
- **Wall Hatching**: **Pattern** (Lines, Cross Hatch, Brick, Block, Shingle, Lap Siding, Board and Batten, Tile, Herringbone, Insulation, Concrete, Earth, Grass), **Scale** (0.1 to 10) and **Angle**.
- **Deck**: **Top height**, **Board thickness** and **Railing around the edge**.
- **3D Solid**: by kind, Width, Depth and Height of a box; Radius and Height of a cylinder or cone; Radius of a sphere; Height of a polyline solid or pyramid; plus **Bottom height** and **Rotation**.

OK is refused with a reason for a size of zero or less.

## 17.5 What the plan and 3D show

- **Plan**: floor regions, decks and solids draw under the walls; wall hatching, wall regions, corner boards, quoins and moldings draw over them, in their layer's color. Regions and hatching hatch with the pattern (hatching is skipped when you are zoomed far out). A deck is hatched with its boards (a 5 1/2" pitch). A selected object is outlined.
- **3D**: a corner board is three boxes (a board on each outer face and the square where they meet) standing off the siding; quoins are the same "L" stacked course by course, long and short blocks swapping faces on alternate courses; a molding is its
  profile swept along each segment, mitered at the corners of a polyline (a closed line has no end caps; corners sharper than the bisector limit are cut off at four times the section's projection); a region is a thin plate (on the floor it lies on the finished floor, or flush with it when it cuts the finish layers; on a wall it stands off the chosen face); a deck is a slab of decking boards, and with a railing
  it gets posts at most 6' apart and a 36" high top rail; solids are boxes, 24-sided cylinders and cones, a 24 x 12 sphere, prisms, pyramids and flat faces. Wall hatching is a plan drawing and has no 3D form.

## 17.6 Differences from Chief

- No mini elevation window for a Wall Material Region: you drag along the wall and set the heights in the dialog.
- Profiles are drawn as closed polylines with the CAD tools and added to the library from the Edit toolbar or the Library > Moldings menu. Chief opens a CAD detail window for Edit Molding Profile; here the profile is placed beside the plan as a polyline and you add it to the library again under the same name.
- A 3D molding that repeats a symbol along a path repeats the profile's own shape (a block of the profile section, Repeat Distance apart, mitred at corners); a library symbol's mesh is not placed along the path.
- The Molding Specification has no Selected Arc panel: an arc is a polyline of short edges, edited like any other.
- Wall material regions are not drawn on curved walls.
- Pyramid is an extra solid beyond Chief's list; **3D Solid Feature** (chapter 6.7) places a library item as a solid rather than drawing a primitive.
- Trim follows a wall only when Select Objects moves it (17.3).
- A **wall cap** (the Wall Cap tab of the Wall Specification, chapter 2.6: Flat Cap, Overhanging Cap or Thick Coping on the top of a wall, usually a half wall) is part of the wall, not a molding object; it is built in 3D on straight walls.

## 17.7 Moldings

Every object that has a Moldings panel (molding polylines now; rooms, the Floor Defaults, cabinets, tray ceilings, wall caps and soffits through the same widget) edits one **Moldings table**. The data and rules are in `plan_core::moldings`, the panel in `dialogs/molding.rs`, the sweep in `plan_3d::molding`.

**Profiles.** A profile is a closed polyline drawn at actual size, the back of the molding being its left edge. Draw it with the CAD Polyline tool, select it and choose **Edit toolbar > Add to Library as Molding Profile** (Library > Moldings > Add Closed Polyline as Molding Profile).
Several closed polylines selected together become one **stacked molding** (Add to Library as Stacked Molding) that keeps how they sit against each other, each part with its own material. The profile goes into the plan's own library and becomes the active profile.
**Place Molding Profile** puts a profile back in the plan as a closed polyline; **Edit Molding Profile** does the same beside the walls so you can reshape it and add it again under the same name. The library holds the nine room profiles of the Room Specification, a
square profile (the default of the Molding Polyline tool), and casing, wall cap, rail and strip profiles.

**The panel.** The table lists Profile, Width, Height, Repeat, Horizontal Offset, Vertical Offset and Stack. The buttons are **Add New** (pick a library profile), **Make Copy**, **Edit**, **Replace**, **Default** (back to the profile's size and type), **Delete**, **Make Stack** (tick the rows to stack),
**Explode Stack**, **Move Up** and **Move Down**. The Selected Profile Options hold Width and Height with **Retain Aspect Ratio**, **Repeat Distance** (a 3D molding: the profile's shape repeated along the path, half the repeat distance long unless you give a Symbol Length), the offsets (a negative
Horizontal Offset recesses the profile behind the back line, as for an under-cabinet light rail), **Auto Offset** (a stacked profile sits on the one before it), **Vertical Position**, **Type** (base, chair rail, crown, casing, wall cap, edge profile, rail, strip profile, eave, gable), **Profile Rotation**,
**Reflect** Horizontal and Vertical, **Full Wall Width** and **Split Profile** (rails and wall caps), **On Selected Edge** (Automatic, On, Off), Texture Up Direction and Count Components, and a preview. Rooms and floors add **Use Floor Default**, **These moldings** and **No Change**.
A stack of types hangs together from where its type belongs: a base on the floor, a chair rail 32" up, a crown from the ceiling.

**Molding polylines.** A molding polyline is a profile table swept along a polyline whose points can each have their own height (a 3D line). The profile lies on the **right** of the drawing direction, so a clockwise path has it inside; **Reverse Direction** swaps it and **Extrude Inside Polyline**
puts it inside a closed path whichever way it was drawn. Old molding lines keep projecting to the left. **Remove Molding from Selected Edge** and **Add Molding to Selected Edge** switch single edges off and on (select the edge by double-clicking near it, by the Selected Line panel, or with Select Next Edge); an edge
that is off is drawn dotted in the plan and has no molding in 3D. Joints are mitred in the plane that bisects the two edges, also for edges that rise and fall; at a twisted joint the two sections are averaged (**Auto Calc Orientation at Twisted Joints**), or cut square when **Mitre Molding at
Twisted Joints** is off, and **Mitre Molding If Next Edge Turned Off** cuts an end on the mitre plane of the edge that is off.

The **Molding Specification** has, besides the panels above, **General** (Height from Z=0, the three joint switches, the selected edge and whether the molding is on it, Automatically Generated) and **Selected Line** (3D Length, Angle in XY Plane, Angle from XY Plane, Start and End height of the edge, and
**Select Edit Plane**: move the edge along itself, across it or up, the edges next to it following). The plan shows the path and, dashed, the front line the profile's projection away from it; Show Label writes the label (the profile name, or your own text) on the molding.

**Room moldings.** A room's moldings come from its own table, else from its room type's table, else from the Floor Defaults table; porches, decks, garages, stairwells, Open Below rooms and courtyards get nothing from the Floor Defaults. They run along the interior surfaces and stop at openings,
behind cabinets (unless the cabinet's Cut Room Moldings is off), along walls that are not drawn or that are set to suppress room moldings, and on edges switched off for the room. **Make Room Molding Polyline** (Edit toolbar or Library > Moldings, with the pointer in the room) turns the room's moldings into editable
polylines and the room stops making them; **Make Exterior Room Molding Polyline** runs a closed polyline round the outer faces of the exterior walls at the ceiling height; **Make Cabinet Molding Polyline** does the same for the crown and light rail of the selected cabinets.
The older Moldings tab of the Room Specification keeps working; the new tables are stored by `tools::molding::set_room_molding_table` / `set_floor_molding_table` / `set_type_molding_table` for the dialogs of the room, Floor Defaults and Room Type Defaults.

**Replace.** The Replace Moldings tool (Trim flyout) and Library > Moldings > Replace Moldings From Library give the molding polylines the active profile, keeping their offsets and switches. Both are one undo step.

**Reporting.** The Materials List take-off (`plan_core::moldings::trim_takeoff`) lists the 3D linear length of every molding, grouped by profile and material, under **Interior Trim**, and the corner boards (two boards per corner), the quoins (stack height and block count) and eave and gable moldings
under **Exterior Trim**. Vertical casings show in the plan; every other molding is a 3D object.
