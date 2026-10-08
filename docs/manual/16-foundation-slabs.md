# Chapter 16: Slabs, Pads, Piers and Platform Holes

This chapter covers the objects you draw by hand below and through the floors: slabs (with or
without a footing), holes in a slab, square pads, round piers, and the holes you cut in a floor or
ceiling platform for a stairwell or a light well. They sit beside the whole-building foundation of
chapter 4.5 (Build Foundation), which makes a foundation floor of walls, a monolithic slab or piers
in one step; the objects here are drawn one at a time, like Chief's Slab flyout.

## 16.1 How the objects work

Every object belongs to one floor and is stored in that floor's own `foundation` slot (a typed field
of the plan file), so it saves, loads and undoes with the plan. Older files kept the same data as a
hidden `Foundation, Data` text record; the program converts such a file when it opens it and removes
the layer.

| Object | What it is | Defaults |
|---|---|---|
| **Slab** | A flat concrete polygon with a thickness and a top height | 4" thick, top at the floor's elevation (0), Concrete |
| **Slab with Footing** | A slab with a footing under its outer edge | Footing 16" wide, 8" deep |
| **Slab Hole** | A hole cut in the slabs it lies inside | Plain dashed outline |
| **Slab Hole with Footing** | The same, with a footing around the hole | Footing size of the slab it cuts, else 16" x 8" |
| **Square Pad** | A square pad placed with one click | 24" square, 12" thick, top at the floor's elevation |
| **Round Pier** | A round post placed with one click | 12" across, 36" high, no footing |
| **Hole in Floor Platform** | A polygon cut out of the floor platform | |
| **Hole in Ceiling Platform** | A polygon cut out of the ceiling platform | |

All heights are measured from the finished-floor elevation of the floor the object is on: a slab whose
top height is 0 has its top at the floor and its body below it. Lengths are inches, shown as feet and inches.

## 16.2 Tools

The slab tools are in the **Slab** flyout on row 2 (Build > Slab). The two platform-hole tools are in the
**Floor** flyout (Build > Floor), next to Floor Material Region (chapter 17). None of them has a
hotkey.

| Button | Today |
|---|---|
| Slab | Works. |
| Slab with Footing | Works. |
| Slab Hole | Works. |
| Slab Hole with Footing | Works. |
| Square Pad | Works. |
| Round Pier | Works. |
| Hole in Floor Platform | Works (Floor flyout). |
| Hole in Ceiling Platform | Works (Floor flyout). |
| Slab Footing (Straight Wall flyout) | Works: draw the polygon as for Slab with Footing; it makes the same foundation slab with a footing (chapter 17.2). |

### Drawing a polygon

The polygon tools (Slab, Slab with Footing, Slab Hole, Slab Hole with Footing and the two platform holes) work like
Chief's Polyline:

| Gesture | Result |
|---|---|
| Click, click, click ... | Each click adds a corner; corners snap like walls. The status bar shows `Length: ...` from the last corner. |
| Double-click, `Enter` | Closes the shape and makes the object (at least 3 corners; the area must be at least 1 sq in). |
| Click the first corner again | Closes the shape (after at least 3 corners). |
| Press at the first point and drag | Draws a rectangle instead (the status bar shows `w x d`); release makes the object. |
| `Backspace` or `Delete` | Drops the last corner; with none, deletes the selected object (below). |
| `Esc` | Cancels the shape in progress. |
| Hold `Alt` | Free angle for the next corner (the angle snap is suspended). |

"Slab needs at least 3 corners" is shown when you close a shape too early. Each finished object is one undo step named after the tool
("Slab", "Slab Hole with Footing", ...), and it becomes the selection. **Square Pad** and **Round Pier** place one object per click, showing a
ghost at the default size.

### Selecting, moving and deleting

- **Select Objects** picks a slab, hole, pad, pier or platform hole by clicking it (piers and pads first, then hole edges, then a slab by its edge, then by
  its inside, smallest slab first; a slab picked only by its inside ranks below the room under it). Dragging the object's body moves it as one undo step; Delete removes it;
  a marquee box-selects (left to right: those inside; right to left: those touched); double-click or `Enter` opens its specification. A selected object is outlined.
- **Corner handles.** A selected slab, slab hole or platform hole shows a handle on every corner; drag one to reshape the polygon (one undo step). Pads and piers have no handles: they move by their body.
- With a slab tool active, hold `Cmd` (`Ctrl` off macOS) and click an object to pick it and drag to move it (one undo step, "Move Slab" ...); `Delete` or `Backspace`
  deletes the selected object or the one under the pointer; double-click one outside a drawing to open its specification. Objects on a hidden layer cannot be picked.

## 16.3 Dialogs: the specifications

Each object has its own Specification dialog on the shared frame (tab list, panel, preview, Help / Cancel / OK). OK writes the changes as one undo step.

| Dialog | Tabs |
|---|---|
| Slab Specification | General, Fill Style, Line Style, Layer |
| Slab Hole Specification | General, Line Style, Layer |
| Square Pad Specification | General, Layer |
| Round Pier Specification | General, Layer |
| Platform Hole Specification | General |

- **Slab, General**: Thickness (must be above zero), Top height (relative to the floor), Footing (**Add footing** with **Footing width** and **Footing depth**), Material
  (Concrete, Stone or Brick). A line shows the net area (its own holes and the slab holes inside it taken out), the perimeter and the concrete in cubic yards (body plus footing).
- **Slab, Fill Style**: Pattern (None, Solid, Hatch, Cross Hatch, Grid) and Color. **Line Style**: Solid, Dashed, Dotted or Dash Dot. **Layer**: the layer it is drawn on.
- **Slab Hole, General**: **Footing around the hole**, and its area. Line Style and Layer as above (a hole is dashed by default).
- **Square Pad, General**: Size, Thickness, Top height, Material and the concrete in cubic yards. **Round Pier, General**: Diameter, Height, Top height, a square footing (size and depth; it must be wider than the pier), Material and the concrete.
- **Platform Hole, General**: **Hole in** Floor platform or Ceiling platform (switching moves it to the matching layer), its area.
- OK is refused with a reason for a bad length, a zero thickness, a footing with no width or depth, a pier footing no wider than the pier, or a pad with no size.

## 16.4 What the plan and 3D show

- **Plan**: a slab is filled with its pattern and color (hatch lines every 12" at 45 degrees, cross hatch at 45 and 135 degrees, a grid at 0 and 90 degrees, or a solid tint)
  and outlined in its line style on the `Slabs` layer; its own holes and the slab holes inside it show as dashed outlines, and a footing as a dashed ring just inside the edge. A pad
  is a square crossed by its diagonals and a pier a circle crossed by a plus, both on `Piers/Pads`. Platform holes are dashed outlines on `Floors, Holes` (red) and `Ceilings, Holes`
  (blue). The layers are added to the plan the first time a tool needs them; turning one off hides its objects (they stay in the plan).
- **3D**: a slab is a prism of its thickness with its holes cut out, plus its footing; pads are boxes, piers 16-sided prisms with their footings, in concrete unless the
  object's material names stone or brick. A hole in the floor or ceiling platform is cut out of that platform's mesh over the rooms (so a stairwell shows through). Build Framing frames a Floor Hole as an opening in the floor framing: trimmer joists on each side, header joists across the ends, the common joists cut short (chapter 11.11); the Floor tab of Framing Defaults sets the plies.
- The Materials List counts slab, pad and pier concrete in cubic yards under its Foundation category (11.5), with the Master List's waste factor; the dialogs show the same cubic yards.

## 16.5 Differences from Chief

- Slabs are objects you draw; they do not follow the walls, and Build Foundation does not make them. A foundation **wall** has its own footing, slab chamfer and sill plate on the Foundation tab of the Wall Specification (chapter 2.6); the slab chamfer and pour number there are stored and not built.
- The Slab Footing wall variant, Wall Hatching and Floor Material Region are in chapter 17. Pads and piers have no resize handles (change their size in the dialog).
- Slab Hole footings use the footing of the slab they cut (or the defaults), not a size of their own.
- Fill patterns are simple screen patterns, not Chief's hatch library.
