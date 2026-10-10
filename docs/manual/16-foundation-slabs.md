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

- **Slab, General**: **Hole in Slab** (turns the slab into a slab hole, which must lie inside another slab; untick it in the hole's own dialog to turn it back), Thickness (must be above zero), **Elevation Reference** (Absolute, From Floor, From Finished Floor, From Terrain, From Ceiling or From Roof, measured at the slab's middle) with **Top** and **Bottom** from it (typing the Top moves the slab, typing the Bottom changes its thickness), **Has Footing** with Height, Width and **Footing Offset** (how far the footing reaches out past the slab's edge; 0 puts its outside under the edge), Material
  (Concrete, Stone or Brick). A line shows the net area (its own holes and the slab holes inside it taken out), the perimeter and the concrete in cubic yards (body plus footing).
- **Slab, Fill Style**: Pattern (None, Solid, Hatch, Cross Hatch, Grid) and Color. **Line Style**: Solid, Dashed, Dotted or Dash Dot. **Layer**: the layer it is drawn on.
- **Slab Hole, General**: **Footing around the hole**, and its area. Line Style and Layer as above (a hole is dashed by default).
- **Pier/Pad, General**: **Type** (Round Pier or Square Pad; switching turns one into the other, keeping width, top and depth), **Top Height** and **Bottom Height** (from zero) and **Width**, Material and the concrete in cubic yards. A Round Pier also has a square footing (size and depth; it must be wider than the pier).
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

## 16.5 Foundation Defaults and Build Foundation (Round 16)

**Build > Floor > Build Foundation** (Cmd+F) and **Edit > Default Settings > Foundation > Foundation** open the same two panels; Build Foundation builds, the Defaults dialog only keeps the choices (and Build Foundation keeps its choices as the new defaults too). Opened on a plan that already has a foundation, Build Foundation starts from the choices it was built with and rebuilds in place.

**Foundation panel.** *Automatically Rebuild Foundation*; the **Foundation Type** (Walls with Footings, Monolithic Slab, Grade Beams on Piers); *Hang 1st Floor Platform Inside Foundation Walls* (Walls with Footings only: the stem walls rise to the top of Floor 1's platform instead of stopping under it); *Show "S" Markers on Step Foundation*. **Slab**: the default slab footing wall type with *Edit Default Slab Footing* (monolithic), **Slab Thickness**, *Slab at top of Stem Wall* and the chamfer width and height of a monolithic slab's edge. **Stem Walls**: *Edit Default Foundation Wall* and *Edit Garage Curb* (they open the Wall Type Definitions), **Stem Wall Height** (from the top of the footing to the underside of Floor 1's platform, which bears on top), **Minimum Height**, the Basement Ceiling Height (the stem wall height less the slab) with a note of what that builds, the footing under the walls and the room to make. **Piers**: Width, Depth, Maximum Separation and Round or Square. **Garage Options**: *Build Garage Floor*, Garage Floor to Stem Wall Top, Lower Garage Floor and Minimum Garage Height.

**Options panel.** Rebar for the footing, wall horizontal courses, wall vertical courses, piers and slab (bars per course, course spacing, size in eighths of an inch and the lap in bar diameters), *Use Mesh*, *Foam Seal* and *Termite Flashing*. They go to the Materials List only; nothing is drawn.

**What a build makes.**

- **Walls**: under every exterior wall of a room that builds a foundation below, under interior walls that ask for Create Wall/Footing Below or bear the floor above, and under walls between a garage or slab room and the rest, or between rooms of different floor heights or stem wall heights. A room with Build Foundation Below off leaves its walls out.
- **Stepped foundations**: each wall stands as tall as the higher of the rooms beside it needs (floor height less the platform, or the room's own Stem Wall Height, never below Minimum Height). Where walls of different tops meet, the plan of Floor 0 shows an **S** (layer `Footings, Step Markers`). The footings stay level; a wall made deeper by a minimum or by a room's Stem Wall Height takes Floor 0 down with it.
- **Basement rooms** (Walls with Footings): a clear height of 72 in or more after the 4 in slab gets a finished basement, 48 in or more a basement without a floor or ceiling finish that counts toward the Living Area, less a crawl space.
- **Garage and slab rooms**: a Garage or Slab room whose floor height is still 0 is lowered (the platform plus 12 in, or 3 1/2 in for a monolithic slab); it gets a slab of its own, stem walls (or curbs) and a Slab room under it that supplies the floor above, and *Floor Supplied by the Foundation Room Below* is ticked on the Floor 1 room. A door in a garage wall leaves a cutout in the stem wall or curb as wide as its rough opening plus Add for Concrete Cutout.
- **Monolithic Slab**: every room gets a slab inside thickened edges with a chamfer; Floor 1 rooms get *Monolithic Slab Foundation* and *Floor Supplied by the Foundation Room Below* ticked, and another type unticks them again.
- **Grade beams**: piers (round, or square pads) at the ends of the beams and no further apart than the Maximum Separation; the platform bears on the beams.
- **Fireplaces**: a masonry fireplace on Floor 1 gets a block on Floor 0 of the same size, in the same place in the wall, with no firebox, hearth or chimney (editable like any fireplace; the next build replaces it).
- **Terrain**: Terrain > Building Pad puts the automatic terrain 6 in below the stem wall tops (8 in below a monolithic slab).
- **Auto Rebuild**: the foundation is built again whenever Floor 1 changes in a way it follows. Floor 0 can then not be deleted, and the slab tools refuse to draw, move or delete on it.

## 16.6 Differences from Chief

- Slabs are objects you draw; they do not follow the walls. Build Foundation makes slabs for garage and slab rooms and for a monolithic slab, replacing any slab drawn by hand on Floor 0. A foundation **wall** has its own footing, slab chamfer and sill plate on the Foundation tab of the Wall Specification (chapter 2.6); the slab chamfer and pour number there are stored and not built.
- The Slab Footing wall variant, Wall Hatching and Floor Material Region are in chapter 17. Pads and piers have no resize handles (change their size in the dialog).
- Slab Hole footings use the footing of the slab they cut (or the defaults), not a size of their own.
- Fill patterns are simple screen patterns, not Chief's hatch library.
