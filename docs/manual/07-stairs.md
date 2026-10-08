# Chapter 7: Stairs

Stairs connect floors. Plan Studio's stair engine solves the number of risers
from the floor-to-floor height, checks the result against the International
Residential Code (IRC), and draws a Chief-style plan symbol with the UP arrow,
tread lines and a break line.

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
- The solver picks a **whole number of risers** so the riser height is as close to the
  target (7 1/2") as possible, without exceeding the IRC maximum.
- IRC checks (R311.7): riser 4" to 7 3/4", tread at least 10", clear width at least 36",
  headroom at least 6'-8" (80"). The comfort rule 2R + T (two risers plus a tread, 24" to
  25") is reported as a warning only.
- Default width 36", tread 10", nosing 1", tread thickness 1", riser thickness 3/4", stringer depth 11 1/4".
- Shapes: **Straight**, **L-shaped** (a 90 degree turn on a landing), **U-shaped** (a 180
  degree turn on a landing), **Winder** (pie-shaped treads in the turn) and **Ramp**, each
  turning left or right. A ramp is limited to 30" of rise between landings with a 1:12 slope.
- A **landing** is a flat rectangular platform, the width of the stair by a landing depth
  (36" by default).
- Stairs are stored on the floor as the JSON of a `plan_stairs::Stair` plus plan-only
  settings (label, line and fill style, railings, break line, materials, stairwell wall
  ids) under the key `"x"`.

## 7.2 Tools

### Stair Tools (row 2, Stairs flyout; Build > Stairs)

| Variant | Hotkey | Today |
|---|---|---|
| Draw Stairs | `Shift+Y` | Works. Starts the default shape (Straight). |
| Straight Stairs | `Ctrl+Alt+Shift+Cmd+B` | Works. |
| L-Shaped Stair | `Ctrl+Alt+Shift+Cmd+C` | Works. |
| U-Shaped Stair | `Ctrl+Alt+Shift+Cmd+D` | Works. |
| Curve to Left | `Ctrl+Alt+Shift+Cmd+E` | Works (winders). |
| Curve to Right | `Ctrl+Alt+Shift+Cmd+F` | Works (winders). |
| Landing | `Ctrl+Alt+Shift+Cmd+G` | Works. |
| Draw Ramp | `Ctrl+Alt+Shift+Cmd+H` | Works. |

Each flyout entry (or its hotkey) starts the stair tool in that variant. The four-modifier
chords can only be typed on macOS.

### Drawing a stair

| Gesture | Result |
|---|---|
| Press, drag, release | Draws a stair. The **drag direction** is the direction of travel and the **drag length** is the run. The number of risers is solved from the floor-to-floor rise. |
| Plain click | Places a default stair pointing up the screen. |
| `Tab` | Flips the turn (left or right) of an L, U or curved stair. |
| `Esc` | Cancels. |
| `Delete` | Removes the selected stair (and the stairwell walls it created). |

The status bar describes what is being drawn, for example "Landing: 3'-0" x 3'-0"".
When the top of a new stair lands inside a room of the floor above, the status bar offers
the **Auto Stairwell** (7.4).

### Editing a stair

A selected stair shows four handles, which work with the Stairs tool and with Select Objects:

| Handle | Drag to |
|---|---|
| Move | Move the whole stair. |
| Rotate | Turn it (snaps to 15 degrees when within 2.5 degrees of one). |
| Run | Change the length of the first flight. The tread depth follows because the number of treads is fixed by the rise; a landing grows; a ramp changes slope. Lock Tread in the dialog keeps the tread depth. |
| Width | Change the width (36" minimum). |

Double-click or `Enter` opens the Stair Specification. The Edit toolbar adds four stair commands:

| Command | Does |
|---|---|
| Auto Stairwell | Cuts the stairwell opening on the floor above (7.4). |
| Flare/Curve Stairs | Toggles the stair to winders and back. |
| Add/Remove Stair Breakline | Shows or hides the break line. |
| Make Railing | A placeholder: marks both sides for a railing. The railing itself is (planned). |

## 7.3 What the plan shows

- On the stair's own floor: the outline, one line per riser, landings, the UP arrow
  with the riser count, and a zigzag **break line** where the floor above would cut the
  stair. Chief's default break is at the two-thirds point of the flight.
- On the floor above: the part of the stair beyond the break line, its outline, and a
  **DN** arrow pointing back down.
- In 3D: stairs are built by the engine (`plan_stairs::meshes`: treads, risers, stringers,
  landings, ramp slab, optional handrail), but the 3D view does not draw them yet (planned;
  see chapter 10).

## 7.4 Auto Stairwell

Select a stair whose top arrives under a room of the floor above and choose **Auto Stairwell**
on the Edit toolbar. The program adds a closed ring of invisible room-divider walls on the
floor above, following the stair's footprint, so a "Stairwell" room forms there. The
walls are remembered with the stair: deleting the stair removes them (one undo step).
Cutting the hole in the floor platform and generating guard railings around the opening
are (planned).

## 7.5 Dialog: Stair Specification

Open by double-clicking a stair. The preview draws the plan symbol on top and a side-elevation
stick figure of the risers and treads below. The dialog edits a copy; OK is one undo step.

| Tab | Status |
|---|---|
| General | Works |
| Style | Works |
| Railing | Works (placeholder, see below) |
| Line Style | Works |
| Fill Style | Works |
| Materials | Works (list kept with the stair) |
| Label | Works |

### General

- **General**: Width; for a stair, Tread Depth, Riser Height (the target the solver rounds to a
  whole number of risers), Lock check boxes (Tread depth, Riser height: a Run handle drag leaves
  a locked value alone) and Headroom; for a landing, Depth.
- **Shape**: Stair Shape (Straight, L-Shaped, U-Shaped, Winder, Ramp), Treads Before Landing
  (L and U), Winder Treads (winder), Slope (1 in) (ramp), Turn Left or Right, Landing Depth (L and U).
- **Solved from the floor-to-floor rise**: the live result: Total Rise, Number of Risers, Number
  of Treads, Actual Riser Height, Total Run, and either "Meets the IRC limits." or the code
  warnings in red (riser too tall, tread too short, narrow stair, low headroom). OK is not
  blocked by a code warning, because a stair that breaks the code can still be a valid sketch;
  it is blocked only for a zero width, tread or riser.

### Style

- **Treads and Risers**: Nosing, Tread Thickness, Riser Thickness; open risers.
- **Stringers**: Stringer Depth.
- **Handrail**: Handrail on both sides (3D).

### Railing

Left railing and Right railing check boxes, and Rail Style, Newel Style, Baluster Style
(placeholders). The note reads: "Railing objects arrive with the Railing tool; the sides
marked here are kept on the stair." The engine has rail, newel and baluster geometry
(`stair_railing`, `deck_edge_railing`); no editor command uses it yet (planned).

### Line Style, Fill Style, Materials, Label

Line Style: Line Weight, dashed. Fill Style: Plan Fill and Fill Tone. Materials: a table of
component and material pairs. Label: Label Text and whether to show it.

## 7.6 Code checks

Tools > Checks > Plan Check includes a stair rule: riser height, tread depth, width and
headroom against the IRC limits above, reported with the rule's section number and a
suggested fix. See chapter 4.8.

## 7.7 Differences from Chief

- Stairs are a plan-only object in the editor today: no 3D, no framing, no stairwell cut in
  the floor platform, no automatic railings.
- Auto Stairwell uses invisible room dividers, so the stairwell shows as a room. Chief
  names it by Function.
- There is no Stair Schedule (planned).
- Flare/Curve Stairs toggles winders; free-form curved stairs with an arbitrary radius and a
  flared apron are (planned).
