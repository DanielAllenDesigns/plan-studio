# Parity spec: Select Objects and editing

> Status (2026-10-08): 112 ids: 51 Works, 32 Partial, 28 Missing, 1 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.

Reference: Chief Architect X18 plan view. Written from Chief's Reference Manual and documented behavior plus
our captures in `docs/chief-x18-*.md`. Lines marked **(verify in Chief)** are recalled but not confirmed against
the running program; Daniel should check them in Chief before a builder relies on the exact detail.
Statement ids (`S-n`) are stable; cite them in commits and tests.

## 0. Plan Studio today (code snapshot)

Select Objects (`Space`) picks every object kind (walls, openings, dimensions, CAD and text, cabinets, symbols, stairs, roof planes, electrical devices, cameras, rooms, terrain) by its drawn geometry, with hover highlight, Shift add, Tab cycling, marquee (enclose or touch), handles, drag-to-move, temporary dimensions with type-to-move, layer display and lock rules, and the contextual Edit toolbar (Open Object, Delete, Copy, Paste in Place, Reverse Swing and per-kind commands). Undo and redo are whole-plan snapshots and the menus name the step. The snap engine (endpoint, midpoint, intersection, centerline, grid, angle, perpendicular) serves every tool, with Alt suspending the angle snap. Missing from the menus: Cut, Select All, Delete Objects, Snap Settings, Edit Behaviors, Edit Area, Stretch CAD, and the context menu on right-click. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## 1. Click selection

- S-1: With Select Objects active, a single left click on an object selects it and deselects everything else. The selected object shows its edit handles and its Edit toolbar.
- S-2: Clicking empty space deselects everything. Clicking an already-selected object keeps it selected (it does not toggle).
- S-3: The pick target is the object's drawn geometry, not a centerline. A wall is picked by clicking anywhere inside its filled footprint or on its outline; a line by its stroke; a closed polyline/box by its edge, and by its interior only when it has a fill; text by its text box; a door/window by its symbol (jamb-to-jamb span including the swing arc and the leaf). Pick tolerance is a few pixels beyond the drawn edge.
- S-4: When objects overlap, the topmost by object draw order wins; openings win over their host wall; CAD and text on the active layer win over objects on other layers (verify in Chief).
- S-5: Objects on a layer that is not displayed cannot be selected. Objects on a **locked** layer can be selected but not moved, resized or deleted (verify in Chief: locked-layer objects are selectable only for viewing the specification).
- S-6: Hovering an object with Select Objects active highlights it before the click and the status bar shows the object's description (for a wall "Exterior Wall", length, etc.) (verify in Chief).
- S-7: Double-click opens the object's specification dialog (see `docs/chief-x18-dialogs.md`). The Open Object edit button does the same.
- S-8: Right-click on an object selects it and opens a context menu whose first group matches the Edit toolbar buttons for that object type; right-click on empty space shows the view/zoom/paste menu. Right-drag in empty plan space pans in Plan Studio; in Chief the right button is the context menu and middle-drag/Pan Window pans (verify in Chief).
- S-9: Escape clears the selection when no drawing operation is in progress. Switching tools with a selection keeps nothing selected unless the tool is an edit-type tool.
- S-10: Space bar toggles Select Objects as the resting tool; after a placement tool finishes (Esc) the active tool is Select Objects.

## 2. Edit handles

- S-11: Handles appear only when exactly one object (or one group) is selected. With several objects selected only the Move handle (per object) and a shared Move/Rotate set appear (verify in Chief).
- S-12: Handle glyphs have fixed screen size regardless of zoom. Cursor changes on hover to a glyph describing the action (move arrows, resize arrow, rotate arc). Handles take hit priority over body drag.
- S-13: Move handle (four arrows): drag moves the whole object. Snaps apply (object snaps, then grid/angle). A hosted object (door/window) dropped on a different wall re-hosts onto that wall (see `doors-windows.md` DW-18).
- S-14: Resize handles (small squares or triangles at ends, corners and side midpoints): drag changes size while the opposite end/edge stays anchored. Shift while dragging (box-like objects) keeps proportions (verify in Chief).
- S-15: Rotate handle (circular arrow, offset from the object): drag rotates about the object center. Rotation snaps to the angle-snap increment (15 degrees default) and to the angle of nearby edges. A temporary angle readout follows the cursor.
- S-16: Reshape handles: polylines and splines show a handle at every vertex; dragging a vertex moves only that vertex. A mid-edge handle exists on polyline edges to move the whole edge (verify in Chief).
- S-17: Line/Arc handles: a straight CAD line shows Move (midpoint) and Resize at both ends; an arc shows Move, Resize at both ends, and a Change Radius/Arc-Through handle at its midpoint; a circle shows Move (center) and Resize (radius) handles.
- S-18: Wall handles (straight): Move handle at the midpoint, Resize/Move-end handle at each end, plus a Curve-edit handle only for curved walls. There is no Rotate handle on straight walls; rotating a wall is done with Transform/Replicate Object (verify in Chief).
- S-19: Dragging a wall's end handle moves that end freely (angle and length), subject to snaps. The other end stays fixed. Every wall whose end is joined to the dragged end follows it, so the corner stays connected (the joined wall's far end stays fixed).
- S-20: Dragging a wall's end handle onto another wall's centerline creates a T-junction there; onto another wall's end creates a corner join (walls: `docs/parity/walls.md` W-31 to W-40).
- S-21: Dragging a wall's Move handle (or its body) moves the wall **perpendicular to its length only**; the component along the wall is zero. Connected walls keep their directions and stretch/shorten so corners stay joined. This is Chief's signature behavior.
- S-22: Holding Alt/Option while dragging a wall body or Move handle allows a free-direction move; walls joined to it then change both length and angle (verify in Chief; Chief may instead use Point to Point Move for this).
- S-23: While a perpendicular move is in progress, the wall's temporary dimensions to the nearest parallel walls update live and the wall snaps to those walls' faces and centerlines.
- S-24: Door/window handles: Move (center), a Resize handle on each jamb edge (changes width; the opposite jamb stays fixed), a vertical Resize handle on the head (height; sill stays fixed) in elevation only, and a swing handle (see `doors-windows.md` DW-31 to DW-37).
- S-25: Text handles: Move (corner), Resize (right-edge handle that changes the wrap width), Rotate. A text box is not resized by dragging corners to scale the font (font size is a property) (verify in Chief).
- S-26: Dragging a handle begins only after the pointer moves a small threshold (about 3 px). A bare click on a handle does nothing except for toggle-type handles (swing flip).
- S-27: Releasing the mouse commits one undo step for the whole drag. Pressing Escape during the drag cancels it and restores the original geometry.
- S-28: Tab or typing a number during a handle drag opens the temporary-dimension entry for the dragged quantity (length for a resize, distance for a move); Enter commits it (verify in Chief).

## 3. Marquee, multi-select, Tab, groups

- S-29: Click-drag starting in empty space draws a marquee rectangle. Drag left-to-right: selects objects **entirely inside** the rectangle. Drag right-to-left: selects objects that are inside **or touched** by the rectangle (verify in Chief; recalled as the AutoCAD-style window vs crossing rule).
- S-30: A marquee that starts on an unselected object drags that object instead (S-13). To marquee from on top of objects, begin the drag in a gap or hold Alt (verify in Chief).
- S-31: Shift+marquee adds to the current selection; Shift-click on a selected object removes it, on an unselected object adds it. Ctrl/Cmd-click does not mean add in Chief (verify in Chief).
- S-32: Marquee Select Similar: Edit toolbar / context command that, with one object selected, selects every object of the same type (and, for walls, optionally the same wall type) inside the next marquee drawn; also reachable from the "Select Same Type" button on the right-edge vertical bar (see `chief-x18-ui-notes.md`) (verify in Chief for exact UI).
- S-33: Select All (Cmd+A) selects every selectable object on the active floor that is on a displayed, unlocked layer. Status bar shows the count.
- S-34: Tab with the pointer over overlapping objects cycles the selection through the stack at the pointer; Shift+Tab goes backwards. The current candidate is highlighted before commit (verify in Chief).
- S-35: Group: Edit > Group (Edit toolbar "Group Selected Objects") fuses the selection into a group. Clicking any member selects the whole group; the group shows one bounding box with Move/Rotate/Resize handles. Ungroup restores the members. Double-click inside a group opens the group member under the pointer (verify in Chief).
- S-36: Selecting a group member individually is done by Tab-cycling or by the Select Group Member command (verify in Chief).
- S-37: Multi-selection dragging moves all selected objects together by the same delta; hosted openings travel with their walls; a multi-selection containing walls does not use the perpendicular-only rule (it is a plain translate) (verify in Chief).
- S-38: The Edit toolbar for a multi-selection shows only the commands common to every selected type (Copy, Delete, Transform/Replicate, Reflect, Align/Distribute, Layer, Group).

## 4. The Edit toolbar (contextual) by object type

- S-39: When objects are selected, a floating Edit toolbar appears next to them (user-customisable; it is the same buttons as the right-click menu). The set is chosen from the selected type; unavailable commands are hidden, not greyed.
- S-40: Buttons common to nearly every object: Open Object, Copy/Paste (Copy Selected Objects), Delete Objects, Transform/Replicate Object, Reflect About Object, Align/Distribute (multi), Center Object, Point to Point Move, Layer (move to layer), Select Same Type (verify in Chief for exact order).
- S-41: **Straight wall** buttons: Open Object, Copy, Delete, Transform/Replicate Object, Reflect About Object, Make Parallel/Perpendicular, Break Wall (Add Break), Change Line/Arc (straight to curved), Fix Wall Connections, Reverse Layers, Center Object, Point to Point Move, Layer, Convert to Polyline (wall centerline to CAD polyline), Wall Type (verify in Chief).
- S-42: **Curved wall** buttons: as a straight wall but Change Line/Arc turns it back to straight; adds Make Arc Tangent (to the adjacent wall).
- S-43: **Door/window** buttons: Open Object, Copy, Delete, Reverse Swing (door) / Flip (window), Reflect About Object, Transform/Replicate Object, Center Object (centers on its wall, `doors-windows.md` DW-24), Layer, Select Same Type, Mulling commands for adjacent windows.
- S-44: **Text** buttons: Open Object (Text Specification), Copy, Delete, Transform/Replicate Object, Rotate, Convert text (to Rich Text), Layer.
- S-45: **CAD line / arc / polyline** buttons: Open Object, Copy, Delete, Transform/Replicate Object, Reflect About Object, Make Parallel/Perpendicular (lines), Break Line (splits into two at the chosen point), Add Break (inserts a vertex), Change Line/Arc, Convert to Polyline, Make Arc Tangent, Align/Distribute, Layer.
- S-46: Every Edit toolbar command acts on the current selection and is one undo step.
- S-47: Transform/Replicate Object opens a dialog: Move, Copy (count), Rotate, Reflect, Resize/Scale; Delta X/Y or Distance + Angle; "Number of copies"; "Multiple copies placed at increments of the delta". Edit Behaviors Replicate (S-65) shortcuts this for drag.
- S-48: Reflect About Object / Reflect About Line: after the command the user clicks a line or wall; the selection is mirrored about it (copy or move per the dialog) (verify in Chief).
- S-49: Make Parallel/Perpendicular: click the target line/wall; the selected line/wall rotates about its fixed end (the end not nearest the click) until parallel/perpendicular, its length unchanged.
- S-50: Break Line / Break Wall: the user clicks a point; the object is split in two; for walls the two halves stay connected end to end and openings go to the half that contains them.
- S-51: Fix Wall Connections: re-runs the wall join solver on the selected walls (see `walls.md` W-41).
- S-52: Point to Point Move: click a "from" point, then a "to" point; object moves by that vector with object snaps active, regardless of the perpendicular-only wall rule.
- S-53: Center Object: moves the object to the midpoint between two reference objects clicked by the user (or centers a door/window in its wall segment).
- S-54: Align/Distribute (multi): dialog with Align Left/Center/Right/Top/Middle/Bottom and Distribute Horizontally/Vertically with spacing option.
- S-55: Reverse Layers (wall): swaps which side the exterior and interior layers face (flips the wall's layer stack end to end) without moving the wall.

## 5. Temporary dimensions while selected

- S-56: With Temporary Dimensions on (right-edge toggle and View menu; both default on in Plan Studio) a selected object displays dimensions to its surroundings: for a straight wall, its own length, the offset from each of its two faces to the nearest parallel wall/object on each side, and the distance from each end to the nearest perpendicular wall. (Shown as thin blue-gray dimension lines with the value on the line.)
- S-57: For a door/window the dimensions run from each jamb to the next opening or to the wall end (measured to the face of the joining wall), plus the opening width (verify in Chief for the width entry).
- S-58: For a CAD line: its length and angle. For a box: width and height plus offsets to nearest parallel objects. For a circle: radius/diameter.
- S-59: Clicking a temporary dimension's value turns it into an edit field. Typing a number and pressing Enter moves/resizes the object so the dimension has that value, then clears the field. Tab cycles between that object's dimensions; Esc cancels.
- S-60: Direction rule: the **referenced** object (the one at the other end of the dimension) stays fixed; the **selected** object moves toward or away from it along the dimension's measurement axis. Increasing the number moves the selected object away from the reference.
- S-61: For a wall's own-length dimension the wall's Lock setting (Start / Center / End in the Wall Specification General tab) decides which end stays fixed; default Start (captured in `chief-x18-dialogs.md`). Joined walls follow the moving end.
- S-62: Face-to-face dimensions on a wall measure to the wall's surface (outside of main layer or finished face per Dimension Defaults > Locate settings); center-to-center when the Locate setting says so.
- S-63: A temporary dimension can be locked (lock glyph): the constraint persists and later edits of the referenced object move the selected one. Locked temporary dimensions become permanent dimensions in the Dimensions layer (verify in Chief).
- S-64: Temporary dimensions are never printed and never saved; they exist only while the object is selected or being drawn.

## 6. Edit Behaviors (Edit > Edit Behaviors)

- S-65: Edit Behaviors are a radio group that changes what dragging an edit handle does for CAD/Text and some objects: **Default** (move/resize in place), **Replicate** (dragging leaves the original and creates a copy; the Transform/Replicate dialog opens for count), **Resize** (a body drag resizes instead of moves for boxes/circles/arcs), **Concentric** (dragging an arc/circle/polyline creates a concentric offset copy), **Fillet** (clicking two CAD lines rounds their corner with a radius prompt), **Chamfer** (same but cuts the corner square).
- S-66: The active behavior shows as a toggled item under Edit > Edit Behaviors and as a status-bar hint; it resets to Default when switching away from Select Objects (verify in Chief).
- S-67: Fillet/Chamfer apply to CAD lines, arcs and polyline corners and to wall corners as "Wall Fillet" is not offered (verify in Chief).

## 7. Snap settings (Edit > Snap Settings)

- S-68: Object Snaps (master on/off, default on) with individual types: Endpoint, Midpoint, Intersection, Perpendicular, Tangent, Center, Quadrant, On Object, Extension, Points/Markers. Each has its own on/off in Snap Settings. The marker glyph (square = endpoint, triangle = midpoint, X = intersection, etc.) appears at the snap point and the status bar names it.
- S-69: Snap priority when several apply within the snap distance: Endpoint > Intersection > Midpoint > Center > Quadrant > Perpendicular/Tangent > Extension > On Object (verify in Chief).
- S-70: Snap distance is a screen-space tolerance (a few pixels) set in Preferences > Behaviors; independent of zoom.
- S-71: Angle Snaps (default on): while drawing or rotating, the segment direction snaps to multiples of 15 degrees relative to the +X axis, and also to the direction of nearby walls (parallel and perpendicular). Angle Snap Grid (View menu) draws the radial guide.
- S-72: Grid Snaps: snap to the Grid Snap Unit (default 1" in plan, independent of the Reference Grid spacing). Reference Grid visible does not imply snapping.
- S-73: Bumping/Pushing: items like cabinets and fixtures dragged against a wall bump to its face and align to it; with Pushing on, a moved object pushes adjacent objects instead of overlapping. Walls and openings are unaffected (verify in Chief).
- S-74: Holding Alt/Option temporarily suspends all snaps for the current operation (verify in Chief); in Plan Studio today Alt only suspends the wall angle snap.

## 8. Undo / Redo

- S-75: Cmd+Z undoes the last **model change**, Cmd+Y (and Shift+Cmd+Z) redoes. Selection changes, view changes (zoom, pan, floor change) and tool changes are not undoable.
- S-76: One user gesture is one undo step: a drag-move, a typed temporary dimension, a placed wall chain segment, an OK in a specification dialog, a paste. A multi-wall chain drawn without leaving the tool is one step per wall (verify in Chief).
- S-77: Undo restores selection to the objects that were selected by the undone action when they still exist.
- S-78: Undo levels are limited by Preferences > General (default 20; "unlimited" not offered) (verify in Chief; X18 may allow more).
- S-79: View > Action History panel lists the stack by name; clicking an entry undoes to it.
- S-80: Undo covers specification-dialog edits, default-settings edits (verify in Chief: default settings are **not** undoable), and file-open (not undoable; it resets the stack).

## 9. Clipboard

- S-81: Copy (Cmd+C) and Cut (Cmd+X) place the selection (with hosted openings, text, dimensions tied to selected walls) on the clipboard. The insertion reference point is the selection's center.
- S-82: Paste (Cmd+V) attaches the clipboard objects to the cursor at their reference point; the user clicks to drop. Esc cancels. Pasted walls auto-join to touching walls at the drop point.
- S-83: Paste Hold Position (toolbar button, Build row 2 item 18) pastes at the **original coordinates** with no cursor attachment; used to move content between floors or views.
- S-84: Copy and Paste in Place (key sequence `C, P, P`) duplicates the selection at the same position in one step; the duplicate is selected.
- S-85: Pasting across different Plan Studio/Chief files preserves layers by name; missing layers are created (verify in Chief).
- S-86: Copy/Paste does not copy the room, which is derived; names attached to rooms are not copied.

## 10. Delete and Delete Objects

- S-87: Delete/Backspace removes the selected objects; deleting a wall also removes its hosted doors/windows; deleting a door leaves the wall intact; walls reconnect their neighbors (a corner becomes two free ends). One undo step.
- S-88: Delete Objects (Shift+Space) opens a dialog listing object categories (Walls, Doors, Windows, Cabinets, Fixtures, Dimensions, CAD Lines, Text, etc.) with checkboxes; OK deletes every object of the checked categories on the active floor (or within the current selection when one exists) (verify in Chief).
- S-89: Deleting an object located on a locked layer is refused with a message.

## 11. Edit Area

- S-90: Edit > Edit Area offers Select Edit Area / Edit Area Current Floor / Clear Edit Area style commands that define a rectangular region; subsequent Move, Copy, Rotate, Delete and Stretch commands apply to everything inside the region, and walls crossing its edge stretch (verify in Chief for exact menu names).
- S-91: Stretch CAD (Edit menu) stretches CAD vertices inside a drawn window while leaving vertices outside fixed.

## 12. Keyboard, modifiers and cursor feedback

- S-92: Arrow keys nudge the selection by the Grid Snap Unit (1" default) in the arrow's direction; Shift+Arrow nudges by 10 units; a wall nudged with Left/Right arrows moves perpendicular-only per S-21 (verify in Chief).
- S-93: Enter with an object selected opens its specification (same as double-click). Delete/Backspace deletes. Cmd+D duplicates in place (verify in Chief; Chief's documented command is Copy and Paste in Place, `C, P, P`).
- S-94: Modifier table for Select Objects (macOS): Shift = add/remove from selection; Alt/Option = suspend snaps (and free move for walls, S-22); Cmd = nothing special for selection; Ctrl+drag = copy-drag in some X versions (verify in Chief). Space = toggle Select Objects.
- S-95: Cursor shapes: arrow in empty space; four-arrow over a movable body; resize double-arrow over a resize handle, oriented along the resize axis; curved arrow over a rotate handle; hand over Pan.
- S-96: Fill Window Selected Objects (right-edge bar) zooms the plan to the selection bounds with a margin; disabled with no selection.
- S-97: Select Objects draws the selection in a high-contrast color (default blue) with the handles; hidden-layer ghost objects are never drawn selected.
- S-98: The status bar left segment shows the selected object count/type ("1 Straight Wall selected") and cursor coordinates X/Y/Z in feet-inches with fractions (already captured in `chief-x18-ui-notes.md`).
- S-99: Dragging an object off the visible plan area auto-scrolls the view (verify in Chief).
- S-100: Pressing Escape while a handle drag is in progress cancels (S-27); pressing it with no drag deselects (S-9).

## 13. Rotate, Transform/Replicate and Reflect in detail

- S-101: The Rotate handle rotates about the object's rotation center (the object center, unless the user sets a different center by clicking first in Rotate mode of the Transform dialog). Rotation commits in the unit of the angle readout (degrees, 0.1 precision by default).
- S-102: Rotating a multi-selection rotates positions about the selection center and each object's own angle by the same amount; hosted openings follow their walls.
- S-103: Transform/Replicate Object dialog fields: Move (Delta X, Delta Y or Distance/Angle), Rotate (angle, About: selection center / point), Copy (check "Make copies", Number of Copies), Resize (scale %), Reflect (about axis). Apply is one undo step; Cancel changes nothing.
- S-104: "Replicate" with N copies and a delta places copy k at k*delta from the original (linear array). A radial array is made with Rotate + copies (verify in Chief).
- S-105: Reflect About Object with "Copy" unchecked mirrors in place; with Copy checked leaves the original. For walls the mirrored walls keep their layer stacks mirrored so the exterior still faces outward (verify in Chief).
- S-106: Reflected doors flip their hinge and swing so their appearance mirrors (not their function).

## 14. Selection of derived and special objects

- S-107: Rooms are not directly selected objects in the plan with Select Objects unless the click lands in the room interior (Chief selects the room object; its specification opens on double-click). Plan Studio's rooms are derived and have no object identity yet.
- S-108: Dimensions are selectable by their dimension line, extension lines or text; selected dimensions show the Move handle (drags the dimension line perpendicular to its measured axis), Resize handles at each extension line end, a text-move handle, and an Add/Delete extension-point control.
- S-109: Selecting a wall that is part of a closed loop does not select the loop; Shift-click each wall, or use Select Same Type.
- S-110: Selecting a hosted door/window also highlights its host wall softly (a thin outline) while the temporary dimensions are shown to the wall ends.
- S-111: Objects on the Reference Display (floor below/above in gray) are never selectable.
- S-112: Clicking a CAD object that lies behind a wall on the same position picks by layer order; the active CAD layer's objects are favored (verify in Chief).

## 15. Gap list

Priority order for builders (highest value first): history wiring (S-75..S-79), `Selection` set + object hit-testing (S-1..S-6), handles and drag state machine (S-11..S-28), perpendicular wall move with joined neighbors (S-19, S-21), temporary dimensions with typed edit (S-56..S-61), centralized snap module (S-68..S-74), clipboard (S-81..S-86), contextual Edit toolbar (S-39..S-55).


| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| S-1..S-6 object-geometry hit test, multi-type selection, hover highlight | Only walls, centerline pick, no hover highlight (hover is only for door/window tools) | High | Replace `selected: Option<Id>` with `Selection { items: Vec<ObjectRef> }`; add `ObjectRef::{Wall, Opening, Dimension, Cad}` and per-type `hit_test` in `plan-core` (`geometry.rs`); pick against footprint polygon; hover highlight in `draw()` |
| S-11..S-20 edit handles (Move, Resize ends) | None; walls cannot be dragged at all | Critical | New `handles.rs` in plan-app: build handles for the selection each frame, hit-test handles first, drag state machine (`Dragging { handle, start_world, original: Project }`) committing via history |
| S-21 perpendicular wall move with neighbors stretching | Not implemented | Critical | Core fn `move_wall_perpendicular(floor, id, delta)` that translates a wall and recomputes endpoints of walls joined at its ends by intersecting their infinite lines with the moved line; reuse tolerance logic from `joins.rs::end_faces` |
| S-19 end-handle drag with joined walls following | `Project::move_wall_endpoint` moves one wall only, no neighbors | Critical | Add `move_wall_end_joined` that moves every endpoint within `tol` of the old point |
| S-22 Alt free move | Not implemented | Medium | Pass `alt` into drag state; skip perpendicular projection |
| S-29..S-33 marquee, Shift, Select All | None | High | Marquee rect state in canvas; `Selection` set ops; Cmd+A binding; Shift multi-click |
| S-34 Tab cycling | None | Medium | On Tab, gather candidates at cursor from `hit_test`, rotate index |
| S-35..S-38 groups | None | Medium | `Group { id, members }` in `Floor`; group-aware selection |
| S-39..S-55 contextual Edit toolbar and its commands | Properties-panel buttons for wall only | High | `edit_toolbar.rs` floating panel positioned from selection bounds; implement in order of value: Delete, Copy/Paste, Reverse Layers, Break Wall, Transform/Replicate, Reflect, Make Parallel/Perpendicular, Point to Point Move, Center, Align/Distribute |
| S-56..S-64 temporary dimensions for selection with typed edit | Only a live length text while drawing | Critical | `dimension.rs` already has `DimensionKind::Temporary`; add `temp_dims_for(selection, floor)`, hit-test the value, edit field, apply with S-60 direction rule through `move_wall_perpendicular` / opening `center_offset` |
| S-65..S-67 Edit Behaviors | None | Low | Enum `EditBehavior` on app, radio in Edit menu; Replicate and Resize first |
| S-68..S-72 object snaps, angle snaps, grid snaps as settings | Hard-coded endpoint/grid/15 degree inside wall tool only; not available for drag or CAD | High | Central `snap.rs` with `SnapSettings` and `fn snap(raw, ctx) -> SnapResult { point, kind }`; use from wall, handles, openings, CAD tools; Snap Settings dialog |
| S-73 bumping/pushing | None | Low | Defer until cabinets exist |
| S-74 Alt suspends all snaps | Only angle snap | Low | Route through `snap()` |
| S-75..S-80 undo/redo | `History` implemented in plan-core but unused; menu inert | Critical | Instantiate `History` in `PlanApp`; call `history.push(&project)` before every mutation point (drawing, delete, dialog OK, drag commit); bind Cmd+Z/Cmd+Y; Action History dock listing step names (extend `History` to store a label per step) |
| S-81..S-86 copy/paste/paste in place/hold position | None | High | Clipboard struct holding cloned objects + reference point; paste attaches to cursor; `Paste Hold Position` toolbar button already exists as a stub |
| S-87 delete selection (multi, hosted openings, door-only delete) | Wall delete works; door delete via panel "x" only | Medium | Make Delete act on `Selection`; delete openings individually |
| S-88 Delete Objects dialog | Inert | Low | Dialog with category checklist |
| S-90..S-91 Edit Area, Stretch CAD | Inert | Low | After CAD tools land |
| S-5 locked layers block edits; S-4 layer visibility affects pick | `LayerSet` exists, never consulted by the app | Medium | Filter candidates in hit test by `layers.is_visible`/`is_locked` |
| S-27 Esc cancels drag, one undo step per gesture | Not applicable yet | High | Part of drag state machine |
| S-92 arrow-key nudge | None | Medium | In `handle_keys`, when selection non-empty and no text focus, translate by `snap_in`; reuse the S-21 wall rule |
| S-96 Fill Window Selected Objects | `fill_window` fits all walls only; the right-bar button is a stub | Low | Compute bounds of `Selection` (walls footprint, CAD `bounds()`) and reuse the camera fit |
| S-101..S-106 Rotate, Transform/Replicate, Reflect | None | High | Pure functions in `plan-core` (`transform.rs`: translate, rotate_about, reflect_about_line, scale) over `ObjectRef`; dialog in plan-app |
| S-107..S-108 select rooms and dimensions | Rooms are derived without identity; dimensions are never drawn or selectable | Medium | Draw dimensions first (see `dimensions-text-cad.md` DIM-1..), then add `Dimension` hit-test |
| S-94 modifier keys (Shift add, Alt suspend snaps) | Alt only toggles wall angle snap; no Shift | Medium | Read `egui::Modifiers` once per frame into an `InputState` struct passed to tools |
| S-7 double-click opens spec for any object | Works for walls and openings | Low (done) | Extend as new object types gain dialogs |
| S-33 Select All | Inert menu item | Low | Add after `Selection` exists |
| S-9, S-10 Esc/Space semantics | Esc ends chain then returns to Select; Space bound to Select | Low (done) | Verify Esc deselects when idle |
