# Chapter 7: Stairs

Stairs connect floors. Plan Studio's stair engine solves the number of risers from the
floor-to-floor height, checks the result against the International Residential Code (IRC),
draws a Chief-style plan symbol with the UP arrow, tread lines and a break line, and builds
the stair in 3D with its stringers, railings and landings. Round 8 rewrote this chapter's
subject: landings, curved stairs, ramps with landings, railings, walls and half-walls on each
side, open and closed risers, stringer styles, lock settings, Click Stairs, the Staircase and
Landing Specifications, and an Auto Stairwell that cuts the hole in the floor above.

## 7.1 How stairs work

You give the stair a position, a direction and a shape. The program does the arithmetic:

```
   Section (side view)                      Plan view
        ___                                +------------------+
    ___|                                   | | | | | | | | | | |   <- tread lines
 ___|   rise  (total_rise / risers)        | | | | | | | | | | |
|            run (tread depth)             ==> UP                    arrow, riser count
                                           +------------------+
```

- **Total rise** is the floor-to-floor height: the next floor's elevation minus this
  one's, or the ceiling height plus a 12 1/8" platform when there is no floor above yet.
  The value is remembered with the stair as its **story rise**, which the Fit to Floor-to-Floor
  button of the dialog uses (7.6).
- The solver picks a **whole number of risers** so the riser height is as close to the
  target (7 1/2") as possible, without exceeding the IRC maximum.
- IRC checks (R311.7): riser 4" to 7 3/4", tread at least 10", clear width at least 36",
  headroom at least 6'-8" (80"). The comfort rule 2R + T (two risers plus a tread, 24" to
  25") is reported as a warning only. A curved stair is also checked for a tread of at least
  6" at the inside edge (R311.7.5.2.1).
- Defaults: width 36", tread 10", nosing 1", tread thickness 1", riser thickness 3/4",
  stringer depth 11 1/4", slab thickness (landings, winders, ramps) 3 1/2".
- Shapes: **Straight**, **L-shaped** (a 90 degree turn on a landing), **U-shaped** (a 180
  degree turn on a landing), **Winder** (pie-shaped treads in the turn), **Curved** (every
  tread fanned around a centre) and **Ramp**, each turning left or right. A ramp is limited to
  30" of rise between landings, with a 1:12 slope and 60" flat landings between its runs.
- A **landing** is a flat platform: a rectangle (the width of the stair by a depth, 36" by
  default) or a polygon of any outline. Its height is not typed in the first place: it takes
  the height of the stair section that arrives on it (7.4).
- Each side of a stair, facing the direction of travel, is **None**, **Wall**, **Railing** or
  **Half Wall** (7.5). A railing has newels, balusters or another infill, a top rail and a
  bottom rail that follow the pitch of the flight.
- Stairs are stored on the floor as the JSON of a `plan_stairs::Stair` plus plan-only
  settings (label, line and fill style, locks, materials, stairwell wall and hole ids) under
  the key `"x"`, in the floor's typed `stairs` slot (chapter 12.2). Stairs and landings live
  on the layer `Stairs`.

## 7.2 Tools

### Stair Tools (row 2, Stairs flyout; Build > Stairs)

| Variant | Hotkey | Gesture |
|---|---|---|
| Draw Stairs | `Shift+Y` | Drag the run (below). Starts the default shape (Straight). |
| Click Stairs | | One click places a straight stair of the default length. |
| Straight Stairs | `Ctrl+Alt+Shift+Cmd+B` | Drag, or click for a default stair. |
| L-Shaped Stair | `Ctrl+Alt+Shift+Cmd+C` | Drag the first flight; the turn and landing are added. |
| U-Shaped Stair | `Ctrl+Alt+Shift+Cmd+D` | Drag the first flight; the second flight returns beside it. |
| Curve to Left | `Ctrl+Alt+Shift+Cmd+E` | A winder stair (three pie treads in the turn) turning left. |
| Curve to Right | `Ctrl+Alt+Shift+Cmd+F` | The same, turning right. |
| Curved Stairs | | Press at the centre, drag to the walking radius. |
| Landing | `Ctrl+Alt+Shift+Cmd+G` | Drag a rectangle, or click the corners of a polygon. |
| Draw Ramp | `Ctrl+Alt+Shift+Cmd+H` | Drag the run; a click places a 1:12 ramp. |

Each flyout entry (or its hotkey) starts the stair tool in that variant. Click Stairs and
Curved Stairs have no Chief hotkey. Off macOS the four-modifier chords are typed as
`Ctrl+Alt+Shift+...` (chapter 13.2).

### Drawing a stair

| Gesture | Result |
|---|---|
| Press, drag, release | Draws a stair. The **drag direction** is the direction of travel and the **drag length** is the run. The number of risers is solved from the floor-to-floor rise, and the tread depth is the run divided by the number of treads. |
| Plain click | Places a default stair pointing up the screen (Draw, Straight and the turning shapes). |
| Click Stairs | One click places a straight stair whose length is the solved number of 10" treads, pointing the way the pointer was heading when it arrived (up the screen if it had not moved). |
| Curved Stairs | Press at the centre of the curve and drag to the walking line: the stair starts at the drag end and turns left. A plain click puts the walking line 60" below the click. |
| Landing | Drag a rectangle; or click the corners of a polygon and double-click the last one (`Enter` also finishes, `Backspace` drops the last corner, `Esc` cancels). A double-click on its own places a 3' square. |
| `Tab` | Flips the turn (left or right) of an L, U, winder or curved stair, before you draw it. |
| `Esc` | Cancels the stair, handle drag or landing polygon in progress. |
| `Delete`, `Backspace` | Removes the selected stair (and the stairwell walls and hole it created). |

The status bar describes what is being drawn, for example "Landing: 3'-0" x 3'-0"" or
"Stairs: 16 risers at 7 5/8", 15 treads at 10", run 12'-6"" ("Curved stairs: ..." for a curved one). When the top of a new stair
lands inside a room of the floor above, the status bar offers the **Auto Stairwell** (7.4).

### Landings and stair sections (CB-27)

A landing and a stair join when an end of the stair meets the landing's outline (within 6"):

- A landing that a stair arrives on takes **the height of that stair's top**.
- A stair that starts on a landing begins **at the landing's height** and rises the rest of
  the floor-to-floor height. This is how two- and three-section stairs are drawn: stair,
  landing, stair, with the landing at the height of the first section.
- Joining happens when you draw either object, finish a handle drag with the Stairs tool, or press OK
  in the dialog. After that the heights are ordinary numbers: you can retype Bottom Height
  and Top Height in the Staircase Specification, and moving a landing away does not unjoin them.

Ramps use the same idea: a ramp whose rise exceeds 30" is split by the engine into runs of at
most 30" with a flat 60" landing between them.

### Editing a stair

A selected stair shows handles, which work with the Stairs tool and with Select Objects:

| Handle | Drag to |
|---|---|
| Move (centre) | Move the whole stair or landing. |
| Rotate (behind the bottom riser) | Turn it about the bottom (snaps to 15 degrees when within 2.5 degrees of one). |
| Run (top end) | Change the length of the first flight. The tread depth follows because the number of treads is fixed by the rise; a landing grows; a ramp changes slope; on a curved stair the handle sets how far round the stair goes. The Lock Tread setting in the dialog keeps the tread depth. |
| Width (left and right, mid-flight) | Change the width (12" minimum in the handle; the code check wants 36"). |

A polygon landing has only the Move handle, because its outline is its corners.

Double-click or `Enter` opens the Staircase Specification (or the Landing Specification).
The Edit toolbar adds four stair commands:

| Command | Does |
|---|---|
| Auto Stairwell | Cuts the stairwell opening on the floor above (7.4). |
| Flare/Curve Stairs | Cycles the shape of a stepped stair: winders become an L-shaped stair, a curved stair becomes straight, anything else becomes a winder stair. |
| Add/Remove Stair Breakline | Shows or hides the break line. |
| Make Railing | Sets both sides of the stair to Railing (newels, balusters, rails). |

## 7.3 What the plan shows

- On the stair's own floor: the outline, one line per riser, landings, the UP arrow
  with the riser count (`14R`), and a zigzag **break line** where the floor above would cut the
  stair. Chief's default break is at the two-thirds point of the flight. A side set to Railing
  is drawn as a double line with newel squares along the flight; a Wall or Half Wall as a solid band.
- On the floor above: the part of the stair beyond the break line, its outline, and a
  **DN** arrow pointing back down.
- Line weight, dashes, a plan fill and an optional label come from the Line Style, Fill Style
  and Label tabs.
- **In 3D** (since Round 8; QA-06): treads, risers, stringers, winder and landing slabs, ramp
  slabs, and for each side the wall, half-wall or railing (newels, balusters, top and bottom
  rails). Treads, landings and ramps use the lumber (Framing) material; risers, stringers, walls
  and railings the Trim material. The 3D view rebuilds when a stair changes. See chapter 10.1.

## 7.4 Auto Stairwell

Select a stair whose top arrives under a room of the floor above and choose **Auto Stairwell**
on the Edit toolbar. The program does two things on the floor above:

1. It cuts a **hole in the floor platform** where the stair passes through (a platform hole,
   as the Hole in Floor Platform tool makes, chapter 16). The 3D view shows the opening
   (QA-04, fixed in Round 8).
2. It adds a closed ring of invisible room-divider walls that follows the stair's footprint,
   so a "Stairwell" room forms there. The room has no floor of its own.

The hole and the walls are remembered with the stair: deleting the stair removes both, moving or
reshaping the stair moves them with it, and Auto Stairwell is one undo step. A stair that already has a stairwell says so and does nothing.
A landing has no stairwell; a stair on the top floor says "There is no floor above: build one first".

Guard railings around the opening on the upper floor are (planned).

## 7.5 Dialogs: Staircase Specification and Landing Specification

Open by double-clicking a stair or landing. The title is **Staircase Specification**,
**Ramp Specification** or **Landing Specification**. The preview draws the plan symbol on top and a
side-elevation stick figure of the risers and treads below. The dialog edits a copy; OK is one
undo step. OK is blocked by a zero width, tread depth or riser height, but not by a code warning,
because a stair that breaks the code can still be a valid sketch.

### Staircase Specification tabs

| Tab | What it holds |
|---|---|
| General | Width, tread depth, riser height, number of risers and treads, bottom and top height, floor-to-floor, lock settings, headroom, shape and the solved result |
| Style | Open risers, nosing, tread and riser thickness, stringer style and depth, slab thickness, handrail on both sides |
| Newels/Balusters | Newel size, height, maximum spacing and cap; the infill style and its sizes |
| Rails | What stands on the left and right side; guard height and the top and bottom rail sizes |
| Line Style | Line weight, dashed lines, break line, show number of risers |
| Fill Style | Fill the stair in plan, and the fill tone |
| Materials | Component and material pairs (treads, risers, stringers, handrail, balusters), kept with the stair |
| Label | Label text and whether to show it in the plan |

A ramp uses the same tabs without the riser and tread fields.

### General

- **General**: Width; for a stair Tread Depth, Riser Height (the target the solver rounds to a
  whole number of risers), **Number of Risers** and **Number of Treads** (typing one changes the
  riser height; a straight stair keeps its total run unless the tread depth is locked).
- **Heights**: **Bottom Height** (the stair's bottom above the floor it was drawn on; non-zero for a
  section that starts on a landing) and **Top Height**. Changing either changes the total rise.
  **Floor to Floor** shows the story rise recorded when the stair was drawn, with a
  **Fit stair to floor-to-floor** button that sets the Top Height to it. (The row is absent
  for a stair with no recorded story rise.)
- **Lock Settings**: Tread depth, Riser height and Number of treads. A locked value stays put
  when the heights or the number of risers change: with the number of treads locked the riser
  height follows; with the riser height locked the count follows. A Run handle drag leaves a
  locked tread depth alone.
- **Headroom**: the clear height the check compares with 80".
- **Shape**: Stair Shape (Straight, L-Shaped, U-Shaped, L-Shaped with winders, Curved, Ramp),
  Treads Before Landing (L and U), Winder Treads (winder), Inside Radius (curved), Slope (1 in)
  (ramp), a check box to use winders instead of a landing in an L-shaped stair, Turn Left or Right,
  and Landing Depth (L and U).
- **Solved from the floor-to-floor rise**: the live result: Total Rise, Actual Riser Height,
  Total Run (Ramp Landings for a ramp), and either "Meets the IRC limits." or the code
  warnings in red (riser too tall, tread too short, narrow stair, low headroom).

### Style

- **Treads and Risers**: **Open risers** (drops the riser boards in 3D), Nosing, Tread Thickness,
  Riser Thickness.
- **Stringers**: **Stringer Style** Closed (a full board whose top follows the nosing line), Open
  (a notched stringer) or None (the treads span between walls), Stringer Depth, Slab Thickness.
- **Handrail**: Handrail on both sides (a plain rail in 3D; the Rails tab gives a full railing).

### Newels/Balusters and Rails

The **Rails** tab sets the **Left Side** and **Right Side** (None, Wall, Railing, Half Wall) and, for
the railings, the Guard Height (36" by default), and the Top Rail and Bottom Rail width and height.
**Newels/Balusters** sets the Newel Size, Newel Height, Maximum Spacing between newels and a Newel cap,
and the Infill: Balusters (Clear Spacing, Baluster Size), Panels, Solid, Cables (rows) or Glass. Balusters
stand on the treads, enough per tread to keep every opening within the clear spacing (4" by code).
A Half Wall is a solid panel with a cap rail; a Wall is full height.

### Landing Specification

A landing has General, Line Style, Fill Style, Materials and Label. General holds the **Width**,
the **Depth** (not shown for a polygon landing, whose outline is its corners), the **Height** of its
top above the floor and its **Thickness**. A stair section that arrives on the landing sets its
height; one that starts on it begins there.

## 7.6 Code checks

Tools > Checks > Plan Check includes a stair rule: riser height, tread depth, width and
headroom against the IRC limits above, reported with the rule's section number and a
suggested fix. See chapter 4.8. A landing is handed to the check as a stair with no risers,
so the 2R + T rule prints an Info line for each landing; this is a known nuisance (the rule
should skip landings; `docs/integration-queue.md`).

## 7.7 Differences from Chief

- No **Stair Schedule** and no Components or Schedule tab in the Staircase Specification (planned).
- Guard railings around the stairwell opening of the floor above are not generated, and a
  railing is not drawn across a landing in the plan symbol (planned).
- The stair side railings run the full flight in 3D; the floor above does not trim them.
- Hidden treads of a stair are not drawn dashed on the other floors.
- Auto Stairwell uses invisible room dividers, so the stairwell shows as a room. Chief
  names it by Function.
- Flare/Curve Stairs cycles shapes. Curved Stairs are a true fan around a centre with an
  inside radius, but a flared apron (a wider bottom step) is (planned).
- Click Stairs places the stair toward the pointer's last movement direction; this is a guess to
  verify against Chief (`DECISIONS.md`, item 6).
