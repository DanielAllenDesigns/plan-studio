# Parity spec: walls

Reference: Chief Architect X18 straight/curved wall tools, wall connections and wall specification. Source: Chief's
Reference Manual and documented behavior, plus `docs/chief-x18-subtools.md` and `docs/chief-x18-dialogs.md`.
**(verify in Chief)** marks recalled-but-unconfirmed detail. Ids (`W-n`) are stable.

## 0. Plan Studio today (code snapshot)

- Model (`plan-core/src/model.rs`): `Wall { id, start, end, thickness, height, kind: Exterior|Interior, layer }`. A wall is a single **centerline** segment with one thickness. No layer stack, no main layer, no wall type, no curved walls, no flags (invisible, pony, foundation, railing).
- Drawing (`main.rs::handle_click`, `snap_wall_point`): click-click chain only. Each click after the first adds a wall and starts the next at the same point (`pending_start = Some(p)`). Min length 1". No click-drag draw, no typed length, no closing-loop detection. Right-click or Esc ends the chain.
- Snapping while drawing: endpoint within 10 px, else grid (`snap_in`, 1" default), else 15 degree angle from the chain start with length rounded to `snap_in`. Alt disables the angle snap only.
- Readout (`draw_rubber_band`): ghost footprint plus a length label (gated by Temporary Dimensions). No angle readout.
- Joins: `plan-core/src/joins.rs::wall_outlines` implements mitered corners (2-wall endpoint match), T-junction trim/extend to the through wall's near face, square ends for collinear and 3+ junctions, miter limit 4 x thickness. **The app does not call it**: `draw_wall` draws `Wall::footprint()` (plain rectangles), so corners overlap/gap on screen.
- Rooms: `rooms.rs::detect_rooms` splits centerlines at intersections and traces faces; min area 1 sq ft; labels are generic.
- Wall flavors on the toolbar: only Straight Exterior and Straight Interior are live. All other wall flyout entries and Curved walls are `NotImplemented`.
- Edit: no endpoint/move handles; `Project::move_wall_endpoint` and `translate_wall` exist in core but are not wired to the UI.

## 1. Tool activation and drawing modes

- W-1: Straight Exterior Wall (Shift+Q) and Straight Interior Wall (Ctrl+Opt+Cmd+6) are separate tools; each draws with its own Default Settings (exterior default total thickness 7 5/8" in the captured template; interior default is a 2x4/2x6 interior type) (verify in Chief for the interior default thickness).
- W-2: The toolbar face shows the last-used wall variant; clicking the icon half re-activates it, the arrow half opens the flyout (see `chief-x18-toolbars.md`).
- W-3: Click-click drawing: first click sets the start, moving the mouse shows a ghost wall, the second click ends that wall and **starts the next wall at the same point**, so a continuous chain forms. Double-click, Esc, or choosing Select Objects ends the chain. Right-click shows the context menu and does not end the chain (verify in Chief).
- W-4: Click-and-drag: press at the start, drag, and release at the end draws exactly one wall; releasing ends that wall and the chain continues from the release point as in W-3 (verify in Chief; the alternative is that drag-release leaves no active chain).
- W-5: Clicking back on the **start point of the first wall in the chain** closes the loop; the junction there is a proper corner join and the chain ends (verify in Chief).
- W-6: Every wall is created on the layer given by the wall type's default layer (`Walls, Normal` in the capture), the active floor, and with the Default Settings height (W-60).
- W-7: Walls shorter than a tiny threshold (about 1/16") are discarded; the tool does not create zero-length walls.
- W-8: Esc cancels the wall being drawn but keeps walls already completed in the chain.
- W-9: The wall tool never selects objects; clicking an existing wall just places a start/end point there (and snaps to it).
- W-10: Switching between wall variants mid-chain keeps the chain and uses the new variant for the next wall (Plan Studio already does this).

## 2. Snapping and live readouts while drawing

- W-11: Snap order while drawing: object snaps (Endpoint of an existing wall's reference line or any layer corner, Midpoint, Intersection, On Object, Extension from another wall's line) first; then Angle Snap (15 degree multiples relative to the previous wall point; also parallel/perpendicular to nearby walls); then Grid Snap (1").
- W-12: When a wall end point is dragged or placed within the snap distance of another wall's reference line, it snaps onto it and a T-junction (wall butting the interior of the other wall) is created; no break of the through wall is made (see W-35).
- W-13: A snapped point shows a marker (square for endpoint, triangle for midpoint, cross for intersection) and the status bar names the snap type.
- W-14: Dashed alignment guides appear when the cursor aligns horizontally or vertically (or at 90/180 degrees to the previous wall) with an existing wall endpoint (smart guides) (verify in Chief for the exact guide set).
- W-15: A temporary dimension line is drawn alongside the ghost wall showing its **length**; a second readout shows the **angle** (degrees from the horizontal, positive counter-clockwise) (verify in Chief for the angle origin).
- W-16: Typing a number while the second point is pending opens the length entry (value shown in the temporary dimension). Typing "12" gives 12 inches; "12'6" or "12-6" gives feet-inches; Tab moves between Length and Angle fields; Enter creates the wall and starts the next one with the same angle reference (verify in Chief for exact Tab behaviour).
- W-17: Alt/Option held while moving suspends all snaps (grid, angle, object); hold it while clicking to place an arbitrary free point.
- W-18: Shift held while drawing constrains to the nearest 0/90 degree direction (verify in Chief; Chief may only use Angle Snaps).
- W-19: Length values display in the project's Dimension format (feet-inches with fractions, `12'-6 1/2"`); typed lengths round to the Dimension Defaults precision (1/16" minimum, 1/8" default) (verify in Chief).
- W-20: The ghost wall is drawn with the real wall footprint (including layer lines once layers exist), so the user sees which side is exterior.

## 3. Orientation, sides and exterior/interior

- W-21: A wall has a **direction** (start to end) and two sides; the right side of an exterior wall (counter-clockwise drawing) is its exterior surface and the left is the interior surface (verify in Chief: Chief documents drawing exterior walls in one rotational direction, and Reverse Layers fixes a wrong choice).
- W-22: There is no automatic exterior/interior detection from enclosure at draw time. The tool chosen (exterior/interior) sets the wall type; room definition and Reverse Layers handle orientation. After an enclosed room is formed, walls whose layer stack faces the wrong way are not auto-flipped (verify in Chief).
- W-23: Reverse Layers (wall Edit toolbar) mirrors the layer stack across the wall's baseline, so siding goes to the other face; the main layer's centerline position does not change.
- W-24: Wall Specification General > Options flags change behavior: Invisible, No Room Definition, No Locate (dimensions skip it), Lock Center (captured in `chief-x18-dialogs.md`). Foundation Wall, Railing, Terrain Retaining Wall, Attic Wall are exclusive wall-class flags.
- W-25: Automatically Generated Wall flag marks walls the program created (e.g. platform fill walls); users are warned when deleting them.

## 4. Wall reference line, thickness and "Resize About"

- W-26: Chief records the wall by a drawn **baseline** plus which part of the wall that baseline is: Outer Surface, Main Layer Outside, Wall Center, Main Layer Inside or Inner Surface (the same five choices appear on Pony Wall alignment and Curved Wall radius in the capture). The default for exterior walls is Main Layer Outside; interior walls default to Wall Center (verify in Chief).
- W-27: Changing a wall's thickness (by editing Thickness, or by changing wall type) keeps the **baseline reference fixed**; the extra thickness grows away from the baseline into the layers on each side according to the layer stack. Corner joins and dimension anchors that reference the main layer therefore do not move.
- W-28: The "Resize About" idea applies to dimension-driven resizing as well: when a temporary dimension edit changes a wall's face-to-face distance, the face not being dimensioned stays fixed.
- W-29: Editing Thickness in the General tab changes the **main layer thickness** (the other layers keep theirs) (verify in Chief).
- W-30: Thickness minimum about 1/8"; maximum limited by the Preferences. Zero-thickness walls are not allowed.

## 5. Wall connections (joins) and cleanup

- W-31: Two walls whose end points coincide (within the connection tolerance, a tiny fraction of an inch) are **connected**; the connection is remembered in the model, not just recalculated from geometry. Dragging one wall's end moves the shared point for both (S-19 in `select-and-edit.md`).
- W-32: Corner (L) join: both walls' faces end at the intersection of their corresponding offset lines (outer-to-outer, inner-to-inner), layer by layer, so no overlap or gap appears. Sharp angles (under about 8 degrees; Plan Studio uses miter length > 4 x thickness) fall back to butt joins.
- W-33: Collinear continuation (two walls end to end in a straight line): the wall appearance is continuous with no joint line; the walls can have different types, in which case the layers meet with a straight joint.
- W-34: Junctions of three or more walls meeting at one point: the two walls that are most nearly collinear are treated as the through wall, and the remaining wall(s) butt into it as in a T-junction (verify in Chief).
- W-35: **T-junction**: a wall whose end lies on the interior of another wall. The butting wall's layers end at the through wall's corresponding layers: finish layers stop at the through wall's finished face, the butting main (framing) layer stops at the through wall's main layer face. The through wall continues unbroken. The through wall is **not** split by the T (the wall remains one object with one id); rooms and framing treat it as if split (verify in Chief; if Chief auto-breaks at T-junctions the statement flips).
- W-36: **Crossing** walls (X): both walls pass through each other unbroken; plan fill and layer lines are cleaned up at the crossing (layers of the same class merge). Rooms split at the crossing.
- W-37: Wall layer alignment at corners: layers of the same function join each other. The main layers (the ones flagged Main in the wall type) join at the corner first; finish layers on the same side of the main layer join to each other with a miter; layers on opposite sides do not connect. If the two main layers differ in thickness, outer and inner faces still meet at the mitered points (no step) (verify in Chief for stepped faces).
- W-38: Walls joined at an angle other than 90 degrees miter correctly for any angle; 3D framing laps are a different concern (Through Wall At Start / At End flags in the Structure tab).
- W-39: Through Wall At Start / At End (Structure tab, Wall Intersections) lets the user make a wall run past the corner instead of mitering, creating a butt corner with this wall as the through wall.
- W-40: Join cleanup runs on every edit that changes a wall end (draw, drag, thickness, delete) and is part of the same undo step.

- W-101: Free wall ends (no connection) get a square cut perpendicular to the wall, with every layer ending flush.
- W-102: Two walls of different thickness meeting at an L corner: the outer faces meet on the outside of the corner and the inner faces meet on the inside, even though each wall's baseline is offset differently; the baseline reference of each wall (W-26) determines where the mitered points fall.
- W-103: A wall end placed on another wall's **end** that already has a join becomes a 3-way junction (W-34); the previously-joined pair keeps its join.
- W-104: Deleting one wall of a corner pair leaves the other with a free square end; deleting the through wall of a T leaves the butting wall with a free square end.
- W-105: Dragging the through wall of a T so the butting wall's end is no longer on it breaks the T; the butting wall's end is moved with the through wall if the connection was made by endpoint dragging (it follows) (verify in Chief).

## 6. Fix Wall Connections and Break Wall

- W-41: Fix Wall Connections (Wall flyout, Edit toolbar) is a tool or command: clicking a wall corner point (tool) or running it on the selection (button) re-solves the connections of all walls within tolerance and also connects ends that are within a short distance but not connected.
- W-42: Fix Wall Connections also repairs overlapping co-linear duplicate walls and removes zero-length walls (verify in Chief).
- W-43: Break Wall (wall flyout and Edit toolbar Break Line): click a point on the wall; the wall becomes two walls joined end to end, same type and properties; openings are assigned to the half they lie in; an opening that straddles the break point is refused with a message (verify in Chief).
- W-44: Add Break (Edit toolbar) inserts a break point without splitting into separate objects in polyline-like objects, but on walls it is the same as Break Wall (verify in Chief).
- W-45: Joining two collinear walls: select both and use Merge/Join (not in the default toolbar); it is rarely used (verify in Chief for X18).

## 7. Wall types and layer stacks

- W-46: A **wall type definition** is an ordered list of layers from exterior to interior. Each layer has Material, Thickness, Layer Type (Finish, Sheathing, Framing, Insulation, etc.), a Main Layer flag (exactly one), and optional Wall Covering behavior (captured in Wall Specification > Wall Types).
- W-47: Defined wall types are named ("Stucco-6", "Interior-6"), shared project-wide, and stored in the plan so renaming or editing a type updates every wall using it. A "Wall Type" can come from the library (Library... button).
- W-48: A wall's displayed Thickness equals the sum of its layer thicknesses; walls with different types adjacent to each other show a step in the face at their join (W-37).
- W-49: Editing a wall type definition while walls use it re-flows those walls: baseline reference stays fixed (W-27) and joins re-solve.
- W-50: Wall layers show in plan as thin lines at layer boundaries; framing layer shows its fill; each layer can have its own fill style; the "Wall Layers" display toggle in Display Options controls whether layer lines appear.
- W-51: Exterior and interior default wall types live in Default Settings; newly drawn walls copy the default at draw time (changing the default later does not change existing walls).

## 8. Wall kinds (flyout variants)

- W-52: **Foundation Wall**: same geometry on the foundation floor; has footing, slab, sill plate options (captured Foundation tab); drawn on the Foundation floor/level.
- W-53: **Pony Wall**: a wall whose upper part uses one wall type and lower part another, split at "Elevation of Lower Wall Top"; plan display options let the lower/upper outlines show (captured under Wall Types). Straight Glass Pony Wall is the glass variant.
- W-54: **Half-Wall**: a wall whose top height is lowered to a typical 42" half-wall; displayed with a distinct dashed or lighter plan style (verify in Chief).
- W-55: **Room Divider / Invisible wall**: a zero-thickness-in-3D wall that exists only to define room boundaries; shown as a thin dashed line in plan; excluded from 3D and from wall schedules; Wall Specification has Invisible and No Room Definition flags.
- W-56: **Railing wall**: a wall flagged Railing that draws as a railing in 3D (Rail Style tabs); Railing tools in the Build menu are separate tools but share this wall class.
- W-57: **Glass Wall** and **Glass Pony Wall**: wall types whose main layer material is glass; no framing.
- W-58: **Slab Footing**: a wall flagged Slab Footing that creates a footing strip under a slab.
- W-59: **Wall Hatching / Wall Material Region**: not wall objects. Wall Hatching draws a hatch pattern along a closed polyline over walls in plan; Material Region draws a polyline region painted onto a wall surface in 3D/elevation (verify in Chief).

## 9. Wall heights, platforms and floor relationships

- W-60: New walls take their top and bottom heights from the floor's platform defaults (Default Wall Top Height and Bottom Height checked in the Structure tab). When the floor's ceiling height or platform thicknesses change, walls with those boxes checked follow automatically; walls with them unchecked keep their value.
- W-61: Default ceiling height 9'-1 1/8" (109.125") in the capture is the wall height for a 9' ceiling plus the floor platform; Plan Studio mirrors this in `DEFAULT_CEILING_HEIGHT`.
- W-62: Walls on an upper floor sit on the floor platform below; Floor Platform Intersections options (Automatic, Stop at Floor Below, Balloon Through) control framing, not plan appearance.
- W-63: Wall Specification Structure > Platform Intersections: "Generate Invisible Walls and Railings Between Platforms" auto-creates invisible walls where floors of different height meet.

## 10. Curved walls

- W-64: Curved wall tools draw an arc in two or three clicks: start point, end point, then a third point (arc-through) or drag to set the bulge. The same Alt/snap/readout rules as straight walls apply, and the readout adds the radius (verify in Chief).
- W-65: Curved walls are stored as true arcs (radius, center, ends) and displayed faceted by the Facet Angle (default 7.5 degrees, Automatic Facet Angle checked, captured).
- W-66: Curved wall dialog fields: Radius measured to Outer Surface / Main Layer Outside / Main Layer Inside / Inner Surface; Lock Arc Center or Ends.
- W-67: Change Line/Arc converts a straight wall to a curved one (drag a bulge handle) and back; a curved wall keeps its openings in proportion along the arc.
- W-68: Make Arc Tangent adjusts a curved wall so it is tangent to the adjacent connected wall at the shared end.

## 11. Rooms interplay

- W-69: A closed loop of walls (no gaps; invisible and room-divider walls count) defines a room automatically. The room is the polygon of wall **inside faces** (floor area is measured to finished face by default; "No Room Definition" excludes a wall).
- W-70: A room's area used by labels is finished-floor area; Plan Studio computes area from **centerlines** (documented in `rooms.rs`), which will differ from Chief once wall thickness matters.
- W-71: Each room has an auto-generated type/name only when the user sets it; room labels display name and area in the plan.
- W-72: Rooms update live as walls move; deleting a wall that breaks a loop removes the room.

## 12. Dimensions and walls

- W-73: Automatic exterior dimensions locate to wall main layer outside or outer surface per Dimension Defaults > Locate. Plan Studio's `auto_exterior_dimensions` measures between centerlines with 36" offsets.
- W-74: A wall's temporary dimension shows its length measured along the baseline (W-26), not along an arbitrary face.

## 13. Wall Specification dialog behaviors

- W-75: General tab: editing **Wall Length** moves the end not named by Lock (Start / Center / End); default Lock is Start (captured). Plan Studio's `WallDialog` already implements the three locks and tests them.
- W-76: Editing **Wall Angle** rotates the wall about the Lock point and re-solves connected walls (their shared end moves, far ends fixed).
- W-77: When a length edit shortens a wall past an opening, the OK button is disabled or an error is shown; Plan Studio's dialog blocks OK when an adjusted opening no longer fits (`too_short`).
- W-78: Openings keep their distance from the **Locked** end when the length changes (Plan Studio: `adjusted_openings`). Chief keeps openings' distance from the wall end that does not move (verify in Chief for Center lock: openings may keep absolute position).
- W-79: Wall Types tab lists wall types in the plan plus a Library button; selecting one changes layer stack and thickness at once (W-27 rules).
- W-80: Layer tab sets the object layer (Default checkbox uses the type's layer) and Drawing Group; changing it moves the wall to that layer immediately.
- W-81: Label tab: automatic label shows wall type name and length when the label is enabled; the label sits on the wall's outside (mark) with a relative angle and offset.
- W-82: Dialog changes apply on OK as one undo step; Cancel changes nothing. The preview pane shows the plan view of the wall with its layers and the current opening positions.
- W-83: Multi-selection of walls then Open Object edits the shared fields in one dialog; fields with differing values show blank/indeterminate and are untouched unless edited (verify in Chief).

## 14. Hosted openings and walls

- W-84: Openings are hosted by exactly one wall; moving the wall moves its openings with it; deleting the wall deletes them (`Project::remove_wall`).
- W-85: Moving a wall end such that an opening no longer fits in the remaining length is allowed; the opening is clamped to the wall; if it cannot fit it is deleted with a warning (verify in Chief).
- W-86: Breaking a wall (W-43) splits openings by center. Merging two walls re-hosts their openings onto the merged wall at the right offsets.
- W-87: Thickness changes keep opening centers; the opening's depth follows the new thickness.
- W-88: A wall with an opening shows the opening cutout in plan across all wall layers; the wall layers' lines return at the jambs, and the opening symbol is drawn on top (`doors-windows.md`).

## 15. Other wall-related commands

- W-89: CAD to Walls (CAD menu): select CAD lines/polylines and convert them to walls of the default type; closed polylines convert to a loop. Joins are fixed after conversion.
- W-90: Convert to Polyline (wall Edit toolbar): replaces a wall chain's centerline with a CAD polyline; the wall is deleted or kept per prompt (verify in Chief).
- W-91: Rebuild Walls/Floors/Ceilings (F12, Floor menu) forces recalculation of derived geometry and clears stale connections.
- W-92: Wall > Wall Hatching and Material Region polylines are edited with CAD polyline rules (`dimensions-text-cad.md` CAD-15..CAD-30).
- W-93: Walls on Hidden layers (e.g. Walls, Invisible when the layer is off) are neither drawn nor snapped to; room detection still uses them unless the wall is also flagged No Room Definition (verify in Chief).
- W-94: Wall color/fill in plan is controlled by Display Options and layer settings; exterior and interior walls have different default fills (Plan Studio uses two palette fills).
- W-95: Print/plot line weights come from layer line weight; wall outline is heavier than layer lines.
- W-96: Re-drawing over an existing wall (same endpoints, same type) does not create a duplicate; Chief detects the overlap and merges or warns (verify in Chief).

## 16. Acceptance scenarios for builders

- W-97: Draw a 20' x 12' rectangle of exterior walls clockwise by clicking four corners then clicking the first corner: four connected walls, four mitered corners, one room of area about 240 sq ft (minus wall thickness in Chief), no wall overlaps visible.
- W-98: Draw a 12' interior wall starting on the interior face of the south wall and ending on the north wall: both ends form T-junctions; the exterior walls remain single objects; two rooms are created.
- W-99: Select the interior wall and type 5'-0" in its distance-to-west-wall temporary dimension: wall moves perpendicular, remains connected at both ends, rooms update.
- W-100: Change an exterior wall's type from Siding-6 (6 1/2") to Stucco-6 (7 5/8"): the framing (main layer) does not move, the exterior face moves out by exactly 1 1/8", the interior drywall face is unchanged, corners and T-junctions re-solve, and the change is one undo step.

## 17. Gap list

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| W-32/W-35/W-36 joined corners, T-cleanup rendered | `joins.rs` solver exists and is unit-tested but unused by drawing; app draws plain rectangles so corners overlap | Critical | In `draw_wall` call `wall_outlines(&floor.walls, 0.01)` once per frame (cache while not dirty) and fill the polygon from `WallOutline.polygon`; use outlines also for selection outline and hit-testing |
| W-31 persisted connections | Connections inferred from coordinates each time | High | Keep inference (simple, robust) but expose `connected_walls(point)` helper so moving a corner moves all touching ends; add tolerance constant |
| W-4 click-drag draws a wall | Click-click only | High | In the wall tool use `resp.drag_started/drag_released`; on release create wall, keep chain |
| W-5 close loop on first point | Chain continues forever | Medium | When snapped point equals first chain point, create wall and clear `pending_start` |
| W-3 double-click ends chain; right-click does not | Right-click ends chain | Low | Match Chief; use double-click and Esc |
| W-15/W-16 angle readout, typed length and Tab | Length label only, no typing | High | Text-edit state while `pending_start` is set: digits/quote/dash open entry; Enter commits with `units::parse_ft_in`; Tab swaps field; render angle readout |
| W-14 alignment guides | None | Medium | Compute horizontal/vertical alignment to existing endpoints and previous-wall perpendicular; draw dashed line |
| W-11/W-13 full object snaps and markers | Endpoint+grid+15 degree only, no markers | High | Central `snap.rs` (see select-and-edit gap list) |
| W-17/W-18 Alt suspends all snaps; Shift orthogonal | Alt disables angle snap only | Low | Pass modifier flags into `snap()` |
| W-21..W-23 side orientation, exterior side, Reverse Layers | Wall is a single-thickness centerline; no sides | High | Add `Wall.layers: Vec<WallLayer>` (see W-46) with side orientation = left of start->end or right per decision, then Reverse Layers command |
| W-24 flags (invisible, no room definition, no locate) | Stored only in session `WallExtras`, never affects room detection | High | Move `invisible`, `no_room_definition`, `no_locate`, `kind`/class into `Wall` (serde default); make `detect_rooms` honor them |
| W-26..W-30 baseline reference + thickness growth | Centerline only; thickness change grows both sides equally | High | Add `WallBaseline` enum to `Wall` and compute footprint from it; thickness edit holds baseline |
| W-46..W-51 wall type definitions with main layer | Hard-coded `WALL_TYPES` list in dialog UI only | High | `WallType { name, layers: Vec<Layer {material, thickness, kind, main}> }` in `Project`; walls reference by id; Wall Type dialog |
| W-37 per-layer corner join | Single-polygon miter | Medium | Extend `wall_outlines` to emit one polygon per layer once layers exist |
| W-41..W-45 Fix Wall Connections, Break Wall | Not implemented | High | `Project::break_wall(floor, id, at_dist)` splitting openings; `fix_wall_connections` snaps near ends within tolerance |
| W-52..W-59 wall variants (foundation, pony, half, divider, glass, railing, slab footing, hatch) | `NotImplemented` | Medium (Room Divider High) | Introduce `WallClass` enum and flags; Room Divider first because it drives room detection |
| W-60..W-63 heights follow platforms | `Wall.height` fixed at creation | Medium | `Wall.top: Option<f64>` meaning "follow floor"; resolve at read time from `Floor.ceiling_height` |
| W-64..W-68 curved walls | `NotImplemented` | High | Add `WallShape::{Straight, Arc{center,radius,start_angle,end_angle}}`; extend joins, rooms and opening hosting to arcs |
| W-69..W-72 room polygon from inside faces, labels, names | Centerline polygon; generic labels | Medium | Compute room as inside-face polygon from outlines; keep name anchors |
| W-1 default thicknesses (captured exterior 7 5/8") | Exterior 6.5", interior 4.5" | Low | Confirm Chief's template defaults; change `DEFAULT_EXTERIOR_THICKNESS` |
| W-12 end-drag onto another wall creates T | Not available (no drag) | Critical | Part of handle drag state machine (see select-and-edit S-19/S-20) |
| W-20 ghost shows true footprint with joins | Ghost is a plain rectangle | Low | Feed ghost wall through `wall_outlines` with the existing neighbors |
| W-75..W-78 Lock start/center/end, opening adjust on length edit | Implemented in `WallDialog` (tests exist) | Low (done) | Keep; extend Lock to the temporary-dimension length edit (select-and-edit S-61) |
| W-79..W-80 wall type switch, layer change from dialog | Type list is a UI-only preset (session extras), layer string is stored on `Wall` | Medium | Persist chosen wall type id on `Wall`; layer switch already stored |
| W-83 multi-wall Open Object | Single wall only | Low | Needs `Selection` first |
| W-84..W-88 opening hosting rules | Wall delete removes openings; clamp on length edit in dialog only; handle drag will need re-validation | Medium | `Project::revalidate_openings(floor, wall_id)` after any wall edit; delete or clamp per W-85 |
| W-89..W-91 CAD to Walls, Convert to Polyline, Rebuild | None | Low | After CAD tools; CAD to Walls is a simple polyline-to-wall conversion with join solving |
| W-93 invisible layer hidden but still defines rooms | `Wall.layer` ignored in drawing and rooms | Medium | Honor `layers.is_visible` in `draw()`; keep rooms independent of visibility |
| W-96 duplicate-wall detection | Overlapping duplicate walls allowed silently | Low | Check on add_wall; offer merge |
| W-97..W-100 acceptance scenarios | Scenarios 97 partly (no joins drawn), 98 partly (no T-clean in draw), 99/100 impossible today | n/a | Turn each into an integration test in `plan-core` once the features exist |
