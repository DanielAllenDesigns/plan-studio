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
| **Quoin** | The same "L", stacked in courses of long and short blocks | `Corner Trim` | Long block 16", block height 8", depth 1 1/2", alternating, stack 96", Stone Veneer - Fieldstone |
| **Molding** (line or polyline) | A profile swept along a line | `Moldings` | Crown profile at its stock size, bottom height 0, Painted White Trim |
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
| Molding Line | Trim | Click two points, or press and drag. |
| Molding Polyline | Trim | A click per corner; a double-click or `Enter` finishes. |
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

- **Corner Board**: Width, Thickness, Bottom height, Height, and the lumber in cubic feet.
- **Quoin**: Long block, Block height, Depth, Bottom height, Stack height, **Alternate long and short blocks**, and the number of courses.
- **Molding**: **Profile** (Crown, Base, Chair, Casing; changing it resets the stock size), Height, Projection, Bottom height, and the molding's length. It projects to the left of the drawing direction. The **Cross section** block turns a preset into a **custom profile** (Edit as custom profile) and lists its points as Projection and Height from the bottom edge at the wall, with a + to add a point after one and a button to remove one; Reset to the box goes back.
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
- A preset molding profile is a plain box until you edit it as a custom profile; the profile editor edits points in numbers, not on a drawing.
- Wall material regions are not drawn on curved walls.
- Pyramid is an extra solid beyond Chief's list; **3D Solid Feature** (chapter 6.7) places a library item as a solid rather than drawing a primitive.
- Trim follows a wall only when Select Objects moves it (17.3).
- A **wall cap** (the Wall Cap tab of the Wall Specification, chapter 2.6: Flat Cap, Overhanging Cap or Thick Coping on the top of a wall, usually a half wall) is part of the wall, not a molding object; it is built in 3D on straight walls.
