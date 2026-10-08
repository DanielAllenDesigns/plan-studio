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
- The jambs stay at least 2" from a wall end and at least 2" from the next
  opening. An opening that does not fit is refused, never shrunk.
- A door's `Floor to Bottom` is 0. A window sits on a sill: Daniel's template
  window is 24" off the floor, 72" tall (head at 96").
- Deleting a wall deletes its openings. Moving or stretching a wall moves its
  openings with it, keeping their distance from the wall start, clamped to fit.
- The model stores: width, height, sill height, door or window, swing side and
  hinge side, an opening style, a label override, a schedule number, casing,
  lites and the egress and tempered flags.

## 3.2 Tools

### Door Tools (Build > Door, row 2)

| Button | Hotkey | Today |
|---|---|---|
| Hinged Door | `D, H` (alias `3`) | Works. |
| Doorway | `D, W` | (planned) |
| Sliding Door | `S, D` | (planned) |
| Pocket Door | `D, P` | (planned) |
| Bifold Door | `Ctrl+Alt+Cmd+O` | (planned) |
| Barn Door | `Ctrl+Alt+Cmd+P` | (planned) |
| Fixed Door | `Ctrl+Alt+Cmd+R` | (planned) |
| Garage Door | `G, D` | (planned) |
| Shower Door | `Ctrl+Alt+Cmd+Q` | (planned) |

### Window Tools (Build > Window, row 2)

| Button | Hotkey | Today |
|---|---|---|
| Window | `Shift+W` (alias `4`) | Works. |
| Bay Window | `Ctrl+Alt+Cmd+S` | (planned) |
| Bow Window | `Ctrl+Alt+Cmd+T` | (planned) |
| Box Window | `Ctrl+Alt+Cmd+U` | (planned) |
| Pass-Through | `Ctrl+Alt+Cmd+V` | (planned) |
| Wall Niche | `Ctrl+Alt+Cmd+W` | (planned) |

The model already knows all fifteen opening styles (Hinged, Sliding, Pocket,
Bifold, Garage, Doorway, Barn, Shower, Fixed, Window, Bay, Bow, Box, Pass-Through,
Wall Niche) so plans stay compatible when the tools arrive. Only Hinged Door
and Window have plan symbols today.

### Placing an opening

With Hinged Door or Window active:

| Gesture | Result |
|---|---|
| Move over a wall | The wall highlights and a ghost of the opening follows the pointer, already cut into the wall. **Temporary dimensions** show the distance from each jamb to both wall ends (with Temporary Dimensions on). |
| Click | Places the opening centered on the pointer's projection onto the wall, snapped to the 1" snap unit. The tool stays active so you can place more. |
| Click where it does not fit | Nothing is placed. The reason appears in the status bar (wall too short, overlaps another opening). |
| Click away from any wall | Nothing happens. |
| `Esc`, or `Space` | Return to Select Objects. |

A new opening takes its size from the Default Settings templates (chapter 1.7):
a door on an **exterior** wall uses the Exterior Door defaults (36" x 96"), a door
on an interior wall the Interior Door defaults (30" x 96"), and a window the Window
defaults (32" x 72", 24" sill). Each placement is one undo step.

Hinge and swing: a placed hinged door hinges at the wall-start jamb and swings to
the left side of the wall (looking from start to end). Use Reverse Swing or the
swing handle (below) to flip the swing to the other side. Choosing the swing side and
hinge jamb from the pointer position while placing (Chief's behavior) is (planned).

### Editing with Select Objects

Select an opening by clicking it (the opening wins over its host wall).

| Action | Effect |
|---|---|
| Drag the opening | Slides it along its wall. Dragging it onto another wall re-hosts it there. Snaps to 1". It stops at the clearance from the wall ends and from neighbors. |
| Temporary dimension (jamb to wall end or neighbor) | Click the value, type a length, `Enter`: the opening moves so that dimension takes the value; the other one changes. |
| Click the **swing handle** at the free end of the door leaf (a small pointing-hand handle) | Reverses the swing, the same as the Edit toolbar button. A click, not a drag. |
| Edit toolbar: **Reverse Swing** | Flips the door's swing to the other side of the wall; the hinge jamb stays. One undo step. Moving the hinge to the other jamb (Flip Hinge) has no control yet (planned). |
| Edit toolbar: Open Object, Delete Objects, Copy, Paste in Place | As for any object. |
| `Delete` | Removes the opening; the wall is untouched. |
| Double-click, `Enter` | Opens the Door or Window Specification. |

Not built yet: resize handles on the jambs, mulling adjacent windows, Center
Object and arrow-key nudging (all planned).

## 3.3 What the plan shows

- The wall fill is cut across the opening and jamb lines are drawn at both ends.
- A **door** shows its leaf as a line perpendicular to the wall and a quarter-circle
  swing arc of radius equal to the door width. The hinge jamb and the swing side are drawn
  from two separate settings in the model.
- A **window** shows three parallel lines across the opening (the two wall faces
  and the glass line).
- The 2D label (for example `3068` for a 3'-0" x 6'-8" door) is computed
  (`Opening::auto_label`) but not drawn on the plan yet (planned). Casing, threshold
  and sill marks are not drawn either (planned).

## 3.4 Dialog: Door Specification

Open by double-clicking a door, or Edit > Default Settings > Doors > Interior Door
or Exterior Door for the defaults. The dialog uses the shared frame (tab list,
panel, preview, Help / Cancel / OK) and edits a copy; OK is one undo step. The
preview shows an elevation sketch of the door and a plan sketch in its wall.

| Tab | Status |
|---|---|
| General | Works |
| Options | Works |
| Casing | Works (session only for a placed door; saved for the defaults) |
| Lintel, Sill/Threshold | (disabled) |
| Lites | (disabled) for doors |
| Jamb | Works (session only for a placed door) |
| Arch, Hardware, Shutters, Opening Indicators, Rough Opening, Framing, Energy Values | (disabled) |
| Layer, Materials | (disabled) |
| Label | Works (session only) |
| Components, Object Information, Schedule | (disabled) |

### General

- **General**: Door Style (Hinged, Sliding, Pocket, Bifold, Barn ... kept per session
  and only Hinged changes the plan symbol today), Library Style (set when a library
  door was chosen), Door Type (disabled, "Hinged").
- **Size and Position**:
  - Width, Height, Thickness (thickness is session only).
  - Elevation Reference (disabled, "From Floor").
  - **Floor to Top** and **Floor to Bottom**: editing Floor to Top changes Height
    with the bottom fixed; editing Floor to Bottom moves the opening up or down.
  - **Distance from Wall Start**: the center position. A "Center on wall" button
    centers it. The value is clamped so the jambs keep 2" from the wall ends.
- OK is blocked, with the reason in red, when the opening is wider than its wall
  or overlaps another opening.

### Options

- **Door Swing**: Hinge side Left or Right, with hover text "Hinge on the wall-start jamb" and "Hinge on the
  wall-end jamb". In the current build these two radios flip the **swing side** (the same setting as
  Reverse Swing), not the hinge jamb, so the label is misleading. Swing Angle (session only).
- **Open/Close Display**: Show Open in 2D (session only); Show Open in 3D (disabled).
- Disabled sections: Door Panels (Single / Double / Calculate from Width, All Glass),
  Plan Display (Top Edge), Safety (Tempered Glass, Fire Door), Recessed into Wall,
  Plinth Blocks.

### Casing

Use Interior Casing with Width (3 1/2"), Depth (3/4") and Reveal (1/4"); Use
Exterior Casing (3 1/4" x 1", available only on exterior walls). Disabled:
Double Wall Options (Through, Enlarged, Double), Curved Wall Casing (Straight,
Radial, Parallel).

### Jamb

Has Jamb, Positioning (Door Size Includes Jamb / Excludes Jamb), Sides Width, Top
Width, Fit Jamb to Wall, Depth (when not fit to wall), Inset. Defaults: 3/4" jamb.

### Label

Display Options (Suppress Label in All Views, Display in Plan View), Label Content
(Automatic Label or Specify Label), Size Format (Height/Width, Width/Height, Width
Only), Include Schedule Number, Include Type. The Label Layer (Doors, Labels) is
disabled. All of this is session only until the plan labels are drawn.

## 3.5 Dialog: Window Specification

Open by double-clicking a window, or Edit > Default Settings > Windows > Window.

| Tab | Status |
|---|---|
| General | Works |
| Options | Works |
| Casing | (disabled) |
| Lintel, Sill/Threshold, Sash, Shape, Arch, Treatments, Shutters | (disabled) |
| Frame | Works (session only for a placed window) |
| Lites | Works for counts |
| Opening Indicators, Rough Opening, Framing, Energy Values, Layer, Materials | (disabled) |
| Label | Works (session only) |
| Components, Object Information, Schedule | (disabled) |

- **General**: Window Type (Single Casement ... kept per session), Width, Height,
  Floor to Top, Floor to Bottom (the sill), Distance from Wall Start.
- **Options**: Egress and Tempered Glass (session only for a placed window; the
  Window defaults store them), Show Open in 2D; disabled Interior and Exterior
  Corner Block, Show Open in 3D, Recessed into Wall.
- **Frame**: Has Frame, Positioning (Window Size Includes / Excludes Frame), Sides
  Width, Top Width, Bottom Width, Fit Frame to Wall, Depth, Inset, Corner Join (Post
  or Mitered).
- **Lites**: Lites Across and Lites Vertical (stored with the window), Muntin Width
  (session only). Disabled: Type, Lites in Fixed, Lites in Movable, Muntin in
  Corner, Auto Adjust Lites for Component Size, Round Top Arch.
- **Label**: as for doors.

## 3.6 Egress and checks

A window's Egress flag is metadata. Tools > Checks > Door/Window Check runs the
opening rules of Plan Check (egress size and sill height for bedrooms, door
widths, swing conflicts; chapter 4) and lists each finding with a fix.

## 3.7 Schedules

Tools > Schedules > Door Schedule and Window Schedule list every opening of the
active floor with its size, style and position, and export CSV (chapter 11).

## 3.8 Differences from Chief

- The model has separate hinge and swing settings (four combinations, as in Chief), but only the swing side has a
  control in the editor; the hinge jamb is fixed at the wall-start jamb (planned).
- Opening Specification choices are session only for placed openings; only the
  Default Settings dialogs write the template values that new openings copy.
- Plan labels, schedule callouts, casing marks, mulled windows, bay/bow/box
  windows, garage and sliding door symbols are all (planned).
