# Chapter 3: Doors and Windows

Doors and windows are **openings**: each is hosted by exactly one wall, cut
through every layer of that wall, and travels with it. This chapter covers
placing and editing openings and the Door and Window Specification dialogs.

## 3.1 How openings work

```
        jamb                         jamb
   ======+                            +======     wall (plan view)
         |   <----- width ----->      |
   ======+      (door leaf + arc)     +======
         ^                            ^
         start_offset                 end_offset
         |<-------- center_offset ---->|  measured from the wall's start
```

- Position is the distance from the wall's **start** to the opening's
  **center**, measured along the wall's centerline.
- The jambs stay at least 2" from a wall end, from the face of a wall that meets
  the host wall, and from the next opening; two windows may touch. An opening
  that does not fit is refused, never shrunk.
- A door's `Floor to Bottom` is 0. A window sits on a sill: Daniel's template
  window is 24" off the floor, 72" tall (head at 96").
- Deleting a wall deletes its openings. Moving or stretching a wall moves its
  openings with it, keeping their distance from the wall start, clamped to fit.
- The model stores: width, height, sill height, door or window, swing side and
  hinge side, an opening style, a label override and label settings, a schedule
  number, a mull group, casing, lites and the egress and tempered flags.

## 3.2 Tools

### Door Tools (Build > Door, row 2)

| Button | Hotkey | Plan symbol |
|---|---|---|
| Hinged Door | `D, H` (alias `3`) | Leaf from the hinge jamb and a swing arc to the Swing Angle. |
| Doorway | `D, W` | Jamb lines only: a cased opening. |
| Sliding Door | `S, D` | Two to four thin overlapping panels on two tracks (one panel per 4' of width) and an arrow. |
| Pocket Door | `D, P` | The leaf and a dashed pocket running into the wall past one jamb. |
| Bifold Door | `Ctrl+Alt+Cmd+O` | A V of folded leaves per pair: one pair up to 4', two pairs beyond. |
| Barn Door | `Ctrl+Alt+Cmd+P` | A panel hung outside the wall face, a dashed track, and (open) the closed position dashed. |
| Fixed Door | `Ctrl+Alt+Cmd+R` | A glass pane in the opening, no swing. |
| Garage Door | `G, D` | The closed panel and dashed overhead rails running into the garage. |
| Shower Door | `Ctrl+Alt+Cmd+Q` | A thin glass leaf with a swing arc. |
| Double Door | (none; Plan Studio's own) | Two leaves hinged at both jambs with two arcs. |

### Window Tools (Build > Window, row 2)

| Button | Hotkey | Plan symbol |
|---|---|---|
| Window | `Shift+W` (alias `4`) | The two wall faces and the glass line. |
| Bay Window | `Ctrl+Alt+Cmd+S` | An angled unit projecting 18" past the exterior face, drawn as a double outline. |
| Bow Window | `Ctrl+Alt+Cmd+T` | The same, on a five-segment arc. |
| Box Window | `Ctrl+Alt+Cmd+U` | The same, square. |
| Pass-Through | `Ctrl+Alt+Cmd+V` | Wall faces with a dashed center line and no glass. |
| Wall Niche | `Ctrl+Alt+Cmd+W` | A recess cut 3 1/2" deep from the room side; the wall behind it stays. |
| Casement, Fixed, Sliding, Awning, Hopper Window | (none; Plan Studio's own) | Swing arcs (one sash, two from 4' wide), a glass pane, two overlapping sashes with an arrow, dashed arms outward (awning) or inward (hopper). |

The `Ctrl+Alt+Cmd` chords are shown in the menus and flyout; the two-key ones
(`D, W`, `S, D`, `D, P`, `G, D`) are bound. Each flavor is sized by the variant
defaults in the plan defaults (for example a Garage Door is 108" x 96", a Bay Window
96" x 60"); the Hinged Door and Window use the Default Settings templates.
These variant sizes are Plan Studio's own estimates, not Chief's numbers (they are the
`opening_variants` list of the plan defaults; no dialog edits them yet, but a saved template keeps them).
Which side a bay, bow or box window projects to is the outside of the wall (the side
away from the room); **Reverse Side** projects it the other way.

### Placing an opening

With any door or window tool active:

| Gesture | Result |
|---|---|
| Move over a wall | The wall highlights and a ghost of the opening follows the pointer, already cut into the wall. **Temporary dimensions** show the width and the distance from each jamb to the wall end or the neighboring opening (with Temporary Dimensions on). |
| Click | Places the opening centered on the pointer's projection onto the wall, snapped to the 1" snap unit. The **new opening is selected** (not its wall) and its Edit toolbar shows; the tool stays active so you can place more. |
| Click where it does not fit | Nothing is placed. The reason appears in the status bar (wall too short, overlaps another opening). |
| Click away from any wall | Nothing happens. |
| `Esc` | The first press lets go of the opening just placed and leaves the tool ready for the next one; the next `Esc` (or `Space`) returns to Select Objects. |

**Placement feel (Round 14).** While you hover, the cursor becomes the "no" cursor and the ghost
turns off wherever the opening may not stand (over another opening, on the face of a wall that
meets the host). The opening **snaps** to the middle of the wall, the middle of a free stretch, equal
spacing between its neighbors, flush against a neighbor or a wall junction, and the centers and jambs of
openings on other walls and other floors; the status bar names the snap in force. Hold `Alt` to turn the
snaps off. Press, then drag before you let go, and the opening you just placed slides along its wall: the
placement and the slide are one undo step. Drop a **window onto a door** (the ghost shows the transom) and
the window goes over the door as its transom, mulled into the door's unit.

A new opening takes its size from the Default Settings templates (chapter 1.7):
a door on an **exterior** wall uses the Exterior Door defaults (36" x 96"), a door
on an interior wall the Interior Door defaults (30" x 96"), and a window the Window
defaults (32" x 72", 24" sill). Each placement is one undo step.

Hinge and swing: a placed hinged door **follows the pointer** (DW-8, DW-76; QA-01, fixed in Round 8). It swings toward
the side of the wall the pointer is on (to the wall's left side, looking from start to end, when the pointer is on that side
or on the centerline; to the right side when it is on the other), and its hinge goes on the jamb nearer the wall end
that the click is closer to (the start jamb when the click is in the first half of the wall, the end jamb in the second).
The ghost follows the same rule, so what you see is what is placed. A window has no swing and is unaffected. Use Reverse Swing or the
swing handle (below) to flip the swing afterwards, and the Door Specification's Swing side and Hinge side to set both. Casement windows follow the pointer the same way; the other flavors take their sides from the Options tab.

### Editing with Select Objects

Select an opening by clicking it (the opening wins over its host wall).

| Action | Effect |
|---|---|
| Drag the opening | Slides it along its wall. Dragging it onto another wall re-hosts it there. Snaps to 1". It stops at the clearance from the wall ends and from neighbors. A mulled window moves with its whole unit. |
| Drag a **jamb handle** (square handles at both jambs) | Resizes the opening; the other jamb stays. Snaps to 1" (hold `Alt` to skip), stops at the minimum width of 6", the 2" wall-end clearance and the neighboring openings. One undo step. With **Snap to standard widths** on (Default Settings > Doors or Windows, General) the width lands on the nearest manufacturer width of the style (2'-0", 2'-4", 2'-6", 2'-8", 3'-0" ... for doors; 2'-0" to 6'-0" for windows); `Alt` skips that too. |
| Drag the **label handle** (a small framed dot on the opening's label) | Moves the label off its spot; the offset is stored with the opening and follows it when the opening or wall moves. A schedule mark bubble follows the label. Edit toolbar: **Reset Label Position**. One undo step. |
| Temporary dimension (width, or jamb to wall end or neighbor) | Click the value, type a length, `Enter`. A jamb distance moves the opening so the dimension takes the value; the width resizes it about its center, or about the jamb that has room. |
| While dragging a jamb handle or the move handle, **type a number** | A jamb handle takes the number as the new width (`4'` or `48`); the move handle takes it as the gap from the nearer wall end or neighbor to the jamb. `Enter` finishes, `Esc` cancels. |
| Click the **swing handle** at the free end of the door leaf (a small pointing-hand handle) | Reverses the swing, the same as the Edit toolbar button. A click, not a drag. **Shift**-click moves the hinge to the other jamb. |
| Edit toolbar: **Reverse Swing** | Flips the door's swing to the other side of the wall; the hinge jamb stays. One undo step. **Flip Hinge** moves the hinge to the other jamb. |
| Edit toolbar: **Center on Wall Segment** | Puts the opening midway in the free space between its neighbors and the wall ends (the whole unit for a mulled window). |
| Edit toolbar: **Mull** | With two adjacent openings selected (or one with a neighbor within 12" and nothing in between), joins them into one unit; the gap is closed by moving the later ones against the first. Windows mull with windows, and one door (hinged, double, doorway or fixed) mulls with the windows beside it as sidelites. The unit shares one frame post (the Sash tab's Middle Width) and one casing around the whole unit, in plan and in 3D. |
| Edit toolbar: **Unmull** | Splits the unit of the selected opening back into separate ones. |
| Edit toolbar: **Reverse Side** | For a casement, awning, hopper or projecting window: swings, opens or projects to the other side. |
| Edit toolbar: **Add Transom** | With one door or window selected, adds a fixed window 18" tall directly over its unit, as wide as the whole unit and mulled into it. Offered only when there is room between the top of the unit and the top of the wall. One undo step. |
| Edit toolbar: **Renumber Schedule** | Gives the selected kinds of openings (doors, windows) schedule marks in the order they were drawn, floor by floor (`D01`, `W03`). The Schedules menu has **Renumber Door Schedule** and **Renumber Window Schedule** for the whole plan. |
| Edit toolbar: Open Object, Delete Objects, Copy, Paste in Place | As for any object. |
| `Delete` | Removes the opening; the wall is untouched. |
| Double-click, `Enter` | Opens the Door or Window Specification. |

A mulled unit shows its overall width as an extra read-only dimension, and only the
outer jambs of the unit have resize handles. A transom is part of its unit: it is drawn dashed in plan,
and it is built in 3D over the door or window as a fixed pane in the same frame and casing
(3.9).

## 3.3 What the plan shows

- The wall fill is cut across the opening (a niche only along its recess) and
  jamb lines are drawn at both ends.
- The symbol of each flavor is in 3.2. A hinged leaf is drawn at the Swing Angle
  (Door Specification > Options) and, with **Show Open in 2D** off, closed as a
  thin rectangle with no arc. The hinge jamb and the swing side are drawn from two
  separate settings in the model.
- Every door and window carries a **label** over the opening (3.4, Label): the
  size, as `3068` for a 3'-0" x 6'-8" door or as `3'-0" x 6'-8"`, or the schedule
  mark (`D01`, `W03`, in its circle or hexagon) once a Door or Window Schedule on
  the floor numbers it. The text uses the plan's label text style
  (Schedule Label, else Default Label Style) and follows the opening's layer
  visibility. Marks run in reading order across the plan, so a new door takes the
  next free mark and undoing its placement frees it again.
- **Casing** is drawn as small rectangles on both wall faces beside the jambs when the
  opening's Casing tab has **Show Casing in Plan** on (off by default); a mulled unit has
  one pair for the whole unit, none between its members. **Shutters** are drawn as small
  rectangles outside an exterior wall (beside the opening, or over it when shown closed).
  A window with a Frame width shows jamb blocks of that width; an **arched** head shows two
  dashed head lines across the opening. A door in an exterior wall draws a **threshold** line across its opening
  when its Sill/Threshold tab has **Show Threshold in Plan** on, a window draws its **exterior sill** past the
  exterior face (Use Exterior Sill), and **Show Jamb in Plan** (Jamb tab) adds the jamb blocks beside each jamb
  line. **Swings Both Directions** (Options) draws the swing arc on both sides of the wall. **Opening Indicators**
  adds an X over a fixed unit and an arrow for the way an awning or hopper opens. **Recessed To Layer** (Options)
  stands the door leaf and its swing in from the exterior face.
- Labels live on the **Doors, Labels** and **Windows, Labels** layers (they are added to a plan
  that predates them when its first opening is placed). Hiding the layer hides the labels
  without hiding the doors; hiding Doors hides both.

## 3.4 Dialog: Door Specification

Open by double-clicking a door, or Edit > Default Settings > Doors > Interior Door
or Exterior Door for the defaults. The dialog uses the shared frame (tab list,
panel, preview, Help / Cancel / OK) and edits a copy; OK is one undo step. The
preview shows an elevation sketch of the door and a plan sketch in its wall.

| Tab | Status |
|---|---|
| General | Works |
| Options | Works |
| Casing | Works (interior width, depth and reveal and the Use switches are stored; also Show Casing in Plan) |
| Lintel, Lites, Arch, Hardware, Shutters | Works (3.5a) |
| Sill/Threshold | Works (Show Threshold in Plan) |
| Jamb | Works (the jamb width, Positioning and Show Jamb in Plan are stored with a placed door; the other jamb settings are session only) |
| Opening Indicators | Works (Show Opening Indicators in Plan, Swing Direction Arrows) |
| Rough Opening, Framing, Energy Values | (disabled) |
| Layer, Materials | (disabled) |
| Label | Works (stored) |
| Schedule | Works (3.4a) |
| Components, Object Information | (disabled) |

### General

- **General**: Door Style (any of the flavors in 3.2; stored with the door, and it
  changes the plan symbol, the 3D door and the elevation sketch, but not the size),
  Library Style (set when a library
  door was chosen; stored with a placed door), Door Type (disabled, "Hinged").
- **Size and Position**:
  - Width, Height, Thickness (stored with a placed door).
  - Elevation Reference (disabled, "From Floor").
  - **Floor to Top** and **Floor to Bottom**: editing Floor to Top changes Height
    with the bottom fixed; editing Floor to Bottom moves the opening up or down.
  - **Distance from Wall Start**: the center position. A "Center on wall" button
    centers it. The value is clamped so the jambs keep 2" from the wall ends.
- OK is blocked, with the reason in red, when the opening is wider than its wall
  or overlaps another opening.

### Options

- **Swing** (Hinged, Shower; Double Door has the swing side only): two independent settings, as in
  Chief, so all four combinations are possible. **Swing side** is Left (the wall's normal side) or Right
  (the other side, flipped); it is the same setting as Reverse Swing. **Hinge side** is Start or End, with
  hover text "Hinge on the wall-start jamb" and "Hinge on the wall-end jamb" (stored as `hinge_at_end`).
  The preview, the plan symbol and the swing handle follow both. Swing Angle (stored with a placed door).
- The other flavors show their own fields instead: Sliding (panel count from the width, Opens
  toward), Pocket (Pocket on), Bifold (panel count, Folds toward, Hinged at), Garage (Overhead tracks
  on), Barn (Hung on, Slides toward), Doorway (no swing).
- **Open/Close Display**: Show Open in 2D (stored; unchecked draws the door closed); **Show Open in 3D**
  with an **Open** slider (0 to 100 %): builds this one opening's leaf or panels open in the 3D view, by that
  share of the Swing Angle (hinged) or of the way a sliding, pocket, bifold, barn or garage door travels
  (3.9). **Swings Both Directions** (hinged and double doors) draws a double-acting door's arc on both sides.
  **Recessed To Layer** with **Depth from Exterior Face** stands the leaf in from the exterior face.
- **Door Panels**: Single Door Only or Double Door Only switches between a hinged and a double door;
  **Calculate from Width** makes it single or double by the width (double from 40").
- Disabled sections: All Glass, Plan Display (Top Edge), Safety (Tempered Glass, Fire Door), Plinth Blocks.

### Casing

Use Interior Casing with Width (3 1/2"), Depth (3/4") and Reveal (1/4"); Use
Exterior Casing (3 1/4" x 1", available only on exterior walls), with its own Width, Depth and Reveal.
**Casing profile** (Flat, Head Cap, Plinth Blocks) shapes the boards in 3D: a head cap puts a projecting cap
on the head board, plinth blocks add a block at the foot of each leg and at each head corner. Disabled:
Double Wall Options (Through, Enlarged, Double), Curved Wall Casing (Straight,
Radial, Parallel).

### Jamb

Has Jamb, Positioning (Door Size Includes Jamb / Excludes Jamb), Show Jamb in Plan, Sides Width, Top
Width, Fit Jamb to Wall, Depth (when not fit to wall), Inset. Defaults: 3/4" jamb. With the size
**excluding** the jamb the plan clears a wider opening in the wall than the door's width. (The 3D wall hole
still follows the unit width; chapter 3.9.)

### Sill/Threshold, Opening Indicators

Door: **Show Threshold in Plan** (a thin line across the opening of a door in an exterior wall: hinged,
double, sliding and fixed doors). Window: **Use Exterior Sill** with Projection and Extend (also under
Lintel). **Opening Indicators**: Show Opening Indicators in Plan (an X over a fixed unit, an arrow for an
awning or hopper) and Swing Direction Arrows (an arrowhead at the free end of a swing arc).

## 3.4a Schedule tab

Include in Schedule (a cleared box leaves the opening out of the schedule and its numbering), the **Mark**
(blank numbers it automatically, `D01`, `W03`), and the supplier data the Door and Window Schedules
list as extra columns: Manufacturer, Model, Supplier and Comment. **Renumber Schedule** (Edit toolbar,
Schedules menu) sets the marks in draw order.

### Label

Display Options (Suppress Label in All Views, Display in Plan View), Label Content
(Automatic Label or Specify Label), Size Format (Height/Width, Width/Height, Width
Only), Size Style (`3068` shorthand or `2'-6" x 6'-8"`), Include Schedule Number,
Include Type and Placement (over the opening, interior side or exterior side). A
line under the controls previews the label as it will print, with and without a
schedule. The label draws on the system layer Doors, Labels (Windows, Labels for windows);
drag it with its handle in Select Objects.

- **Automatic Label** shows the schedule mark when a Door or Window Schedule numbers
  the opening and Include Schedule Number is on, otherwise the size.
- **Specify Label** replaces it with your text. Macros: `%automatic_label%` (the size),
  `%schedule_number%` (the mark), `%width%`, `%height%` and `%type%`.
- The settings are stored with the opening once you change them. An untouched label
  keeps following **Edit > Default Settings > Doors / Windows > Label**, which sets the
  format of every new label (and of every label that has no settings of its own).

## 3.5 Dialog: Window Specification

Open by double-clicking a window, or Edit > Default Settings > Windows > Window.

| Tab | Status |
|---|---|
| General | Works |
| Options | Works |
| Casing | Works (3.5a) |
| Lintel, Sash, Arch, Shutters | Works (3.5a) |
| Sill/Threshold | Works (Use Exterior Sill, Projection, Extend) |
| Shape, Treatments | (disabled) |
| Frame | Works (the frame width is stored and drawn in 3D and in plan; the rest is session only) |
| Lites | Works (counts, style and muntin width stored with the window) |
| Opening Indicators | Works (X over a fixed unit, arrow for an awning or hopper) |
| Rough Opening, Framing, Energy Values, Layer, Materials | (disabled) |
| Label | Works (stored) |
| Schedule | Works (3.4a) |
| Components, Object Information | (disabled) |

- **General**: Window Style (any of the window flavors in 3.2, stored with the window and driving the plan symbol and the 3D unit), Window Type (Single Casement ... stored with a placed window), Width, Height,
  Floor to Top, Floor to Bottom (the sill), Distance from Wall Start.
- **Options**: the fields of the flavor first (Casement: swing side, hinge side, swing angle;
  Sliding: Opens toward; Awning and Hopper: Open to the other side; Bay, Bow and Box: Project to the
  other side; Pass-Through and Wall Niche: a note), then Egress and Tempered Glass (session only for a
  placed window; the Window defaults store them), Show Open in 2D; Show Open in 3D with its Open slider (as for doors); disabled Interior and Exterior
  Corner Block, Recessed into Wall.
- **Frame**: Has Frame, Positioning (Window Size Includes / Excludes Frame), Sides
  Width, Top Width, Bottom Width, Fit Frame to Wall, Depth, Inset, Corner Join (Post
  or Mitered).
- **Lites**: Type (Standard grid, Diamond, Prairie, Custom Grid), Lites Across and Lites
  Vertical, Muntin Width, all stored with the window and built in 3D as panes and muntins.
  Disabled: Lites in Fixed, Lites in Movable, Muntin in Corner, Auto Adjust Lites for
  Component Size, Round Top Arch.
- **Label**: as for doors (the Window label defaults are separate from the Door ones).

## 3.5a The shaping tabs (Sash, Lites, Lintel, Arch, Hardware, Shutters)

These tabs shape the 3D unit (and the elevations, which are cut from it) and are stored with
the opening; Default Settings dialogs give new openings their starting values.

- **Sash** (windows): Has Sash, Side, Top and Bottom Width, and **Middle Width**, the post
  between the two sashes of a wide casement and between mulled units. Frame widths are on the
  Frame tab. A window without a sash is glass straight in the frame.
- **Lites**: **Standard** is an even grid; **Diamond** crosses diagonal muntins over one pane
  (the counts are the diamonds across and up); **Prairie** pulls the dividers in to a border of
  small lites round one large pane; **Custom Grid** puts dividers where you type them, as
  percents of the width and of the height (`33, 66`). Doors take the same styles for their glass.
- **Lintel**: Use Exterior Lintel and Use Interior Lintel with Style (Flat, Capped, Keystone),
  Height, Depth and Extend; windows also have an **Exterior Sill** (Projection, Thickness,
  Extend). Both are drawn whether or not casing is shown.
- **Arch**: Type (No Arch, Round Top, Segmental, Tudor, Gothic, Eyebrow) and Height (0 takes the
  type's own rise). The head of a window (plain, fixed or casement) or of a hinged, double,
  fixed or doorway door follows the curve: the frame band, the pane (one pane over the springline;
  the lite grid fills the rectangle below it) and wall fill in the corners above the curve. A door's leaves
  stop at the springline and the arch closes as a glazed transom. Other styles keep a square head and
  only mark the arch in plan.
- **Hardware** (doors): Show Hardware in 3D, Handle (None, Knob, Lever, Handle), Up from Bottom,
  In from Door Edge, Number of Hinges and their inset, drawn as simple metal shapes on hinged,
  double, fixed and pocket doors. Off by default, so a door is a plain slab.
- **Shutters**: Type (None, Panel, Louver), Sides, Width (0 = half the opening), Color, Show Closed,
  Outside Casing and Louver Size. Exterior walls only. The color is stored; the plan draws the
  outline and 3D the trim material.

Door Options > Door Panels has **Calculate from Width**: a hinged door becomes a double door from
40" wide (a sliding door has 2 to 4 panels and a bifold pair 2 or 4 by the same rule). A wall
niche's depth is editable on Options (3 1/2" by default, always leaving 1" of wall).

## 3.6 Egress and checks

A window's Egress flag is metadata. Tools > Checks > Door/Window Check runs the
opening rules of Plan Check (egress size and sill height for bedrooms, door
widths, swing conflicts; chapter 4) and lists each finding with a fix.

## 3.7 Schedules

Tools > Schedules > Door Schedule and Window Schedule list every opening of the
active floor with its size, style and position, and export CSV (chapter 11).

## 3.8 Differences from Chief

- Hinge side and Swing side are separate settings (four combinations, as in Chief), both in the Door
  Specification. A new door also takes both from the pointer position while placing (3.2).
- A placed opening stores its style, its library style or window type, a door's thickness and swing
  angle, the jamb or frame width, Show Open in 2D, its label settings and offset, its mull group, its
  lites, interior casing and every shaping tab of 3.5a (all saved with the plan and loaded with
  defaults from older files). Exterior casing widths stay session only (an opening has one casing). The Default Settings dialogs write
  the template values that new openings copy, and (Round 12) OK in the Default Door, Exterior Door and Window dialogs also saves the **standard widths**, **Snap to standard widths** and the shaping-tab values that new doors and windows start with into the plan defaults (`opening_variants`; `OpeningDialog::apply_to_variants`).
- Double Door, Casement, Fixed, Sliding, Awning and Hopper Window are Plan Studio's own flyout entries
  (Chief picks them in the dialog). Per-mesh shutter color, "Swings from Center / Left Swing Only / Right
  Swing Only" for double doors, and Custom left/right door panel counts are not built; the Rough Opening,
  Framing, Energy Values, Layer, Materials, Components and Object Information tabs stay dimmed.
  The select tool and the Specification dialog still apply the older fit rule (2" between two windows, 2" from
  a wall end) when you drag an opening or type its position; only the placement tools use the newer snaps and
  the window-to-window touching.
- In a **layout** every door and window is drawn with the same plan symbol as the editor, with its casing, and a mulled unit with the unit's span; plan boxes print the opening labels too (the size, your custom text or the schedule number, per Default Settings > Door
  and Window Labels), and the standalone sheet writer in `plan-docs` (`plan_sheet`, which the editor does not call; chapter 11.7) still draws simple door and window symbols.
- The editor's 3D view builds casing, jambs, window stools and aprons, sills and thresholds, and can show
  the doors open (3.9). The lintel, exterior sill, arch, hardware, shutters, sash and lites of 3.5a are always built.
  The 3D wall hole does not yet read Size Includes Frame, Recessed To Layer or Show Jamb in Plan, so with
  the size excluding the frame the plan's wider opening is not matched in 3D.

## 3.9 Openings in 3D (Round 13)

The 3D view now builds openings with their trim, the way the elevations are drawn.

- **3D > Casing, Jambs and Sills** (a toggle, on by default): interior and exterior casing around every
  opening, door jambs, window stools and aprons, and thresholds on exterior doors. The Casing tab sets the
  sizes (Use Interior Casing, Use Exterior Casing, each with Width, Depth and Reveal) and the **Casing
  profile** (Flat, Head Cap, Plinth Blocks). A mulled unit has one casing around the whole unit; a transom
  draws none of its own.
- **3D > Show Doors Open** (a toggle, off by default): every door stands open, hinged doors at the plan-wide
  open angle (90 degrees), sliding and pocket doors slid, barn doors slid along their track, bifold doors
  folded, garage doors raised. Both toggles are stored in the plan and are one undo step each.
- **Show Open in 3D** on an opening's Options tab opens that one door or window by the **Open** slider
  (0 to 100 % of its Swing Angle or travel), whatever the toggle says. A sliding window slides its movable sash by the same share.
- **Transoms.** A fixed window over a door or window, mulled into the unit (3.2). It is dashed in plan and
  built in the unit's frame in 3D, so a door with a transom shows glass above the leaf.
- **Curved walls.** A door or window in a curved wall is cut through the arc and built square to the arc's
  tangent at the middle of the opening.

To see the result, open the 3D view (chapter 10) after placing a door; the toggles are in the 3D menu next
to Rebuild 3D.
