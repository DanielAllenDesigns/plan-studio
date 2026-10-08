# Parity spec: doors and windows (openings)

Reference: Chief Architect X18 door and window tools, opening edit handles and labels. Source: Chief's Reference
Manual and documented behavior plus `docs/chief-x18-subtools.md` and the Door/Window Specification capture in
`docs/chief-x18-dialogs.md`. **(verify in Chief)** marks recalled-but-unconfirmed detail. Ids are `DW-n`.

## 0. Plan Studio today (code snapshot)

Hinged Door (`D, H`) and Window (`Shift+W`) are placement tools with a ghost preview, 1" snapping and distances to both wall ends; openings select, slide along the wall by dragging, and open the Door and Window Specification dialogs (double-click). The model keeps swing side (`swing_flipped`) and hinge end (`hinge_at_end`) separately, and the dialog has separate Swing side and Hinge side controls; the plan symbol draws the hinge from `hinge_at_end` and the swing from `swing_flipped`. Defaults come from Default Settings (interior door, exterior door, window). The model and 3D builder know all fifteen opening styles, but the other flyout entries (Doorway, Sliding, Pocket, Bifold, Barn, Fixed, Garage, Shower, Bay, Bow, Box, Pass-Through, Wall Niche) are not tools yet, and several dialog extras (door style, window type, casing, jamb, lites, label) are session-only. Missing: plan labels, mulling, resize handles. (Refreshed 2026-10-08. The behavior statements in this document are Chief's and unchanged; the gap table below is the original audit and is partly out of date.)

## 1. Activating the tools and the placement preview

- DW-1: Door and window tools are placement tools: each click on a wall places one opening using the Default Settings for that type. The tool stays active after a placement so more can be placed; Esc or Select Objects ends it.
- DW-2: While a door or window tool is active and the pointer is over a wall, a ghost of the opening follows the pointer, centered on the pointer's projection onto the wall, already cut into the wall and showing its symbol (swing arc for doors). The ghost updates the swing and hinge side as the pointer moves (DW-8).
- DW-3: When the pointer is not over a wall the ghost is hidden and the cursor shows a "no" glyph (or nothing happens on click). The tool never places an opening in empty space.
- DW-4: A door cannot be placed on a wall too short for it; a window cannot be placed where it would overlap another opening, **except** that adjacent windows are allowed to touch and then mull (DW-45) (verify in Chief). Refused placements show a status message.
- DW-5: Placement happens on mouse **click** (not drag). Click-and-drag after a click-placement moves the opening just placed along the wall before release (verify in Chief).
- DW-6: New openings take width, height, sill/floor-to-top, door style, swing angle, casing, jamb and label settings from Default Settings (Doors > Interior Door / Exterior Door / Garage Door / Window). A door placed on an exterior wall uses Exterior Door Defaults; on an interior wall, Interior Door Defaults (verify in Chief; the captured dialog lists both).
- DW-7: Captured interior door default: Width 30", Height 96" (floor to top 96"), Thickness 1 3/8", Swing Angle 90 degrees, Hinged style, interior casing 3 1/2" wide (verify in Chief for exact exterior defaults).
- DW-8: **Swing side** while placing a hinged door follows the pointer: the door swings toward the side of the wall where the pointer is (the ghost flips as the pointer crosses the wall center line). **Hinge side** is chosen automatically as the jamb nearer the nearest wall end/corner, so the door opens against the nearer wall (verify in Chief for both rules).

## 2. Position, snapping and temporary dimensions

- DW-9: The opening's **center** is under the pointer. Along the wall the center snaps to the Grid Snap Unit (1") measured from the wall start, then to alignment candidates (DW-10), unless snaps are suspended with Alt/Option.
- DW-10: Alignment candidates the center and each jamb snap to: the **midpoint** of the wall segment, the midpoint of the free span between neighbours, equal spacing between two neighbouring openings, the jamb of a neighbour at the same offset (to align heads), and the face of an intersecting wall plus a small default clearance (for a T-junction, the jamb snaps near the interior face of the through wall) (verify in Chief for the exact set and clearance).
- DW-11: While a ghost or a selected opening exists, temporary dimensions show: distance from each jamb to the nearest wall end or intersecting wall's face (measured to the finished face), distance from each jamb to the nearest neighbouring opening, and the opening width. Dimensions are measured along the wall.
- DW-12: Typing a value into either jamb-to-end temporary dimension moves the opening so that dimension takes that value; the **other** dimension then changes. Typing into the width dimension resizes the opening about its center (verify in Chief) or about the fixed jamb (the jamb away from the edited dimension is fixed).
- DW-13: The minimum distance between an opening jamb and a wall end is the wall's corner clearance (for exterior walls typically wall thickness-based; Plan Studio uses a flat 2") and may be 0 for doorways ending at an intersecting wall (verify in Chief).
- DW-14: Position is displayed and edited in the Door Specification General tab as **distance from the nearer wall end** (jamb side) and "Distance from Wall Start" style center readouts (verify in Chief for the exact field set; Plan Studio exposes "Distance from Wall Start").
- DW-15: An opening that does not fit between neighbours is not placed; it is not shrunk automatically.

## 3. Moving: dragging along the wall and between walls

- DW-16: A selected opening shows a Move handle at its center. Dragging it slides the opening along its host wall, maintaining the same swing and hinge. Snaps and temporary dimensions follow DW-9..DW-11.
- DW-17: Dragging the opening body (not just the handle) also moves it (Chief: any click-drag on the body moves it).
- DW-18: Dragging to a different wall re-hosts the opening: when the pointer is within the pick distance of another wall, the opening jumps to that wall, orienting to it. Dropping on empty space reverts to the original wall.
- DW-19: Dragging an opening through the wall (across the centerline) flips its swing side (in-swing becomes out-swing) in addition to sliding (verify in Chief).
- DW-20: An opening cannot be dragged past a wall end or over another opening; it stops at the clearance (DW-13).
- DW-21: Arrow keys nudge a selected opening along its wall by the Grid Snap Unit.
- DW-22: Cut/Copy/Paste of an opening works: Paste attaches it to the pointer and drops onto a wall like a new placement (DW-2).
- DW-23: Mirroring (Reflect About Object) a wall with openings mirrors their positions; the door's hinge and swing mirror too.
- DW-24: Center Object (Edit toolbar) centers the opening in the free span of its wall segment between its neighbours/ends.
- DW-25: Moving an opening is one undo step per drag (select-and-edit S-27).

## 4. Resizing

- DW-26: A selected opening shows two Resize handles, one at each jamb edge (at wall surface level). Dragging one changes the width; the opposite jamb stays fixed.
- DW-27: Resize snaps to the Grid Snap Unit and, when the opening comes from the library with a size list, to the manufacturer's available widths (Component sizes in the Components tab) (verify in Chief).
- DW-28: Resize cannot overlap a neighbour or push past the wall end clearance; it clamps.
- DW-29: Height and floor-to-sill are edited in the specification dialog or via the elevation/section view (not by plan handles).
- DW-30: Double doors, windows with sashes etc. recompute panel counts from width when Door Panels is "Calculate from Width" (Options tab).

## 5. Swing, hinge and reverse

- DW-31: A hinged door has a **swing** direction (which side of the wall the leaf opens toward) and a **hinge side** (left or right jamb when viewed from the swing side). Together four combinations.
- DW-32: The Edit toolbar for a selected door has Reverse Swing (flips swing to the other side of the wall, hinge unchanged in plan) and Flip Hinge / mirror (moves the hinge to the other jamb, swing side unchanged). Both are one-click and one undo step each (verify in Chief for button names).
- DW-33: A swing edit handle sits at the free end of the door leaf in plan: dragging it across the wall flips swing; clicking it toggles hinge side (the "diagonal" handle). (verify in Chief)
- DW-34: Swing Angle (Door Specification General) sets the open angle shown in plan (default 90 degrees); the arc and leaf draw to that angle.
- DW-35: "Swings Both Directions" (Options) draws both swing arcs for a double-acting door; "Swings from Center" and Both/Left/Right Swing Only options control double door leaves.
- DW-36: Show Open in 2D (Options) draws the open leaf and arc; when unchecked the door is drawn closed (leaf flush in the wall).
- DW-37: Reverse Swing uses the wall's side orientation (exterior vs interior) when "Swing Toward Exterior/Interior" helps; there is no such toggle in default Chief UI (verify in Chief).

## 6. Door types and their plan symbols

- DW-38: **Hinged**: single leaf line from the hinge jamb perpendicular to the wall, quarter-circle swing arc; double doors show two leaves and two arcs (Door Panels: Calculate from Width makes double doors at larger widths; threshold about 40" in Chief's defaults) (verify in Chief).
- DW-39: **Doorway** (cased opening): no leaf or arc; jambs and casing only.
- DW-40: **Sliding**: two overlapping thin panel rectangles inside the wall thickness, offset front/back, with an arrow or break line indicating sliding direction; no arc.
- DW-41: **Pocket**: leaf drawn open inside a dashed pocket rectangle that extends past one jamb into the wall; the host wall gets a wider cut (pocket framing) (verify in Chief).
- DW-42: **Bifold**: zig-zag folded leaf lines in the opening width, no arc.
- DW-43: **Garage**: a long overhead door drawn as a panel line with dashed path or hatch pattern, spanning the opening; default width 108"/96" (verify in Chief for the exact defaults).
- DW-44: **Barn Door**: a rectangular panel lying alongside the wall on the track side, with overhang values (Barn Doors section); panel sits outside the wall thickness.
- DW-45: **Shower Door**: glass leaf with swing arc, thin frame; **Fixed Door**: glazed panel without swing.
- DW-46: Door type is chosen by tool (flyout variant) at placement and can be changed in the General tab's Door Style / Door Type; changing the type updates the plan symbol immediately and applies default dimensions for that style.

## 7. Window types and mulled units

- DW-47: **Window** (the main tool) places a standard window; plan symbol is frame lines at the wall faces with a thin sash/glass line(s) between the jambs; casing shows as small rectangles at jambs when displayed. Window type (casement, double hung, slider, fixed, awning) mainly affects 3D and the sash/indicator symbols.
- DW-48: **Bay**, **Bow**, **Box** windows project outward from the wall: plan shows a polygonal (bay: angled sides) or curved (bow) projection with its own width, depth and angle; box window is a rectangular projection. They carry roof and seat options (Bay/Bow tabs).
- DW-49: **Pass-Through**: an opening through a half wall or serving counter, drawn like a window; **Wall Niche**: a recess cut partway through the wall, drawn with a dashed line on the recessed side and a depth parameter.
- DW-50: Window default sill height keeps the head at about 80" to align with door heads (floor-to-top default); Plan Studio's template is 24" sill + 60" height (head at 84") (verify in Chief for the default).
- DW-51: **Mulled windows**: two or more windows placed adjacent (touching jambs) are joined as a mulled unit with a shared frame/mullion; the unit moves, resizes (total width), and is selected as one object; the Mull command (Edit toolbar) joins adjacent selected windows; Unmull separates them (verify in Chief for auto vs manual mulling).
- DW-52: Windows adjacent to doors can be mulled with a transom or sidelite (door + window combination) via the same Mull command (verify in Chief).
- DW-53: A window may not span a wall junction; if the wall is broken or joined across the window, the window is split or refused.

## 8. Sizes, defaults and components

- DW-54: Size fields: Width, Height (door), Floor to Top, Floor to Bottom (sill for windows), Thickness; editing any recomputes others per the Elevation Reference ("From Floor" default). Setting Floor to Top changes Height if Bottom fixed.
- DW-55: Doors from the library are product-specific: choosing a library door sets style, panels, hardware; "Components" lists sizes with manufacturer code, supplier, price (captured Components table).
- DW-56: "Use Clearance Gaps" and Panel offset fields control the door leaf versus jamb gaps in 3D; not visible in plan.
- DW-57: Opening depth in the wall follows the host wall thickness plus casing depth; "Recessed into Wall" and "Recessed To Layer" put the door leaf at a layer inside the wall (captured Options).
- DW-58: Curved walls: openings can be Straight or Curved ("In Curved Wall" option); the plan symbol is radial.

## 9. Labels, schedule numbers and Tab

- DW-59: An opening label is shown in plan if "Display in Plan View" is checked (Label tab). Automatic label contents follow Size Format: Width/Height, Height/Width or Width Only; sizes use the architectural shorthand where feet and inches are concatenated, e.g. a 3'-0" x 6'-8" door is `3068` (3'0" wide, 6'8" tall); 2'-8" x 6'-8" is `2868`; fractions are shown as `+` or the next-lower inch (verify in Chief for fraction handling).
- DW-60: "Include Schedule Number" prepends the schedule ID inside a circle or after a dash (doors numbered 1, 2, 3 in order of creation; windows lettered A, B, C) and "Include Type" adds the style text (verify in Chief for numbering style).
- DW-61: Schedule numbers are assigned by the Door/Window Schedule tools; renumbering follows the schedule sort order; creating a door adds the next free number; deleting leaves a gap until Renumber is run.
- DW-62: Specify Label replaces the automatic text; macros like `%width%`, `%height%` can be inserted (Insert macro menu in the Label tab).
- DW-63: Label placement: centered over the opening on the interior side by default, with a relative offset and angle controlled in the Label tab; labels are on their own layer ("Doors, Labels" or the type's layer) and can be dragged with a label-handle (verify in Chief).
- DW-64: **Tab** with an opening selected cycles selection through objects stacked at the same pointer location (the opening, its casing, the host wall) (S-34); while a temporary-dimension edit is active, Tab moves between the jamb-to-end fields.
- DW-65: The hover tooltip/status for an opening shows its description and size, e.g. "Hinged Door 3068".

## 10. Interaction with wall layers and rooms

- DW-66: The opening is cut through **all** wall layers in plan; layer lines stop at the jambs and the jamb/casing/threshold symbols are drawn.
- DW-67: Door symbols do not affect room definition; a doorway or door does not break a room. A wall that is deleted removes its openings; changing a wall's thickness keeps opening widths.
- DW-68: Openings follow their wall when the wall is moved, stretched or rotated (center_offset preserved, clamped).
- DW-69: When a wall is broken at a point, openings go to the half they lie in; one straddling the break refuses the break.
- DW-70: Hosting rule: an opening has exactly one host wall; a window in a double wall is hosted by the nearer wall (verify in Chief).

## 11. Placement algorithm (reference for implementers)

- DW-71: Step 1, find the host: among walls on the active floor (visible, unlocked layers) take the wall whose **filled footprint** contains the pointer or, if none, whose nearest face is within the pick distance (about 10 px). Prefer the wall the pointer is over when several overlap at a junction (verify in Chief).
- DW-72: Step 2, project the pointer onto the host's reference line to get `t` (distance from wall start). Apply grid snap `round(t / 1") * 1"` unless snaps are suspended.
- DW-73: Step 3, compute the allowed center range `[w/2 + c, L - w/2 - c]` where `w` is the opening width, `L` the wall length and `c` the end clearance (DW-13). If the range is empty, refuse.
- DW-74: Step 4, for each neighbour opening, compute the blocked interval `[start - w/2 - g, end + w/2 + g]` with gap `g` (0 for windows that mull, a few inches for doors) and remove it from the allowed range; the center snaps to the nearest allowed point.
- DW-75: Step 5, test alignment candidates (DW-10) within the snap distance in screen pixels and prefer them over the plain grid snap.
- DW-76: Step 6, derive swing side (DW-8) from the pointer's signed distance to the wall centerline, and hinge from the nearer end.
- DW-77: Step 7, on click, create the opening, assign the next schedule number, select it (tool stays active), and push one undo step.
- DW-78: Placing a door where a window already exists is refused; there is no auto-replace.

## 12. Casing, trim and plan details

- DW-79: With interior/exterior casing on (Casing tab), plan view draws casing as small rectangles on each wall face at each jamb; width and depth come from the tab (3 1/2" x 3/4" interior, 3 1/4" x 1" exterior in the capture).
- DW-80: Plinth blocks (Options) add square marks at the base in elevation only.
- DW-81: Threshold/sill: the plan shows a thin line across the opening at the threshold position when Sill/Threshold is defined.
- DW-82: Door/window jamb thickness is drawn as the gap between wall layer cut and the leaf; "Size Includes Jamb" changes whether width refers to rough opening or unit.
- DW-83: Opening Indicators (Opening Indicators tab) add graphic marks such as "X" for fixed or an arrow for sliding direction in plan.
- DW-84: Door hardware and shutters are 3D/elevation only and do not alter the plan symbol, except shutters which draw small rectangles outside the wall.
- DW-85: Windows show a sill line projecting past the exterior face when a sill is defined.
- DW-86: Egress flag (window Options) is metadata used for checks and the schedule.

## 13. Edge cases

- DW-87: Opening placed across a wall junction: refused at the T (the jamb would enter the through wall). An opening next to a T snaps to the through wall's face plus clearance (DW-10).
- DW-88: Opening on a curved wall follows the arc; its center is an arc length along the wall, width is chord width at the baseline radius; the symbol is radial (verify in Chief for the exact measure).
- DW-89: Two openings may share a wall segment with no minimum gap if both are windows that are mulled; doors keep a minimum gap equal to the wall end clearance.
- DW-90: A garage door wider than the wall segment is refused; the tool does not span multiple walls.
- DW-91: Openings on foundation walls use their own defaults (foundation window, vent) and sill rules; the tool is the same.
- DW-92: Openings on invisible/room-divider walls are not allowed (the host must be a real wall or a doorway/pass-through variant) (verify in Chief).
- DW-93: Deleting a window or door leaves the wall intact and re-closes the cut; the wall's layer lines return.
- DW-94: Undo of a placement removes the opening and releases its schedule number if it was the last assigned.

## 14. Acceptance scenarios for builders

- DW-95: Draw a 10' wall, select Hinged Door, click at the wall's center: a door of the Default Settings width is centered (center offset 60"), swing arc toward the pointer side, hinge on the jamb nearer a wall end (tie broken by start end); temporary dimensions show jamb-to-end values summing with the width to 120".
- DW-96: With that door selected, drag its Move handle toward the end: the door slides, snapping at 1" steps; jamb-to-end dimension reads the clearance (e.g. 4") and stops there. Release: one undo step.
- DW-97: Press Reverse Swing: the arc flips to the other side of the wall, hinge unchanged. Press Flip Hinge: hinge moves to the other jamb. Undo twice restores the start state.
- DW-98: Place a window, then place a second window touching it: they are mulled into one unit; the total width reads in the dimension; Unmull separates them.
- DW-99: Change the wall length in its specification with Lock = End: openings keep their distance from the End and move relative to Start (matches `WallDialog` today).
- DW-100: Save and reopen: door style, swing, hinge, label settings and schedule numbers are all preserved.

## 15. Specification dialog behaviors

- DW-101: Double-clicking an opening (or Open Object) shows the Door/Window Specification with the tab list down the left and a live preview on the right (plan, elevation or 3D); changing width or height redraws the preview immediately.
- DW-102: Editing Width in the dialog resizes about the opening center if both jambs clear; otherwise about the jamb that has room; a value that cannot fit is clamped and the field turns red or shows a warning (Plan Studio: `clamp_center`).
- DW-103: Changing Door Style swaps default panel, hardware and symbol; it does not change width/height unless the new style has size limits (garage doors have a minimum width).
- DW-104: The Label tab previews the automatic label text before OK; Size Format radios (Width/Height, Height/Width, Width Only) update it live (Plan Studio has the radios).
- DW-105: Default Settings for Doors/Windows edit the templates used at placement; changing a default never alters placed openings.
- DW-106: Cancel discards all edits; OK applies as one undo step.

## 16. Keyboard and tool state

- DW-107: Hotkeys: Hinged Door `D, H`, Doorway `D, W`, Pocket `D, P`, Sliding `S, D`, Garage `G, D`, Window Shift+W (captured); Plan Studio binds only `D, H` and Shift+W.
- DW-108: The flyout button face shows the last used door or window variant; the status bar hint reads e.g. "Hinged Door: click a wall to place a door".
- DW-109: Escape while a ghost is showing cancels the tool and returns to Select Objects; right-click shows the context menu with Select Objects at the top.
- DW-110: Holding Alt/Option disables alignment snaps for free placement; Shift constrains nothing (verify in Chief).

## 17. Gap list

| Chief behavior | Plan Studio today | Severity | Suggested implementation |
|---|---|---|---|
| DW-2/DW-3 ghost preview with swing, cut into wall | Only wall highlight, no ghost | High | In canvas, for Door/Window tools compute placement (`dialogs::place_from_template` logic without mutating) and draw the opening with `draw_opening` at reduced alpha |
| DW-8 swing and hinge auto-chosen from pointer | Always swing to +normal, hinge at start (unless flipped in dialog) | High | Compute side = sign of cross(direction, pointer-center); hinge = jamb nearer the closer wall end; model needs separate `hinge_right: bool` and `swing_side: Side` |
| DW-31..DW-33 four swing/hinge combinations, Reverse Swing, handle | One `swing_flipped` bool couples hinge and swing | Critical | Split into `hinge_end: End` and `swing_toward: Side`; migrate old `swing_flipped` via serde default; update `draw_opening` and the dialog |
| DW-9..DW-12 snapping to 1", centers, neighbours; temporary dimensions; typed edit | Raw projection, 2" clamp only | High | Compute snap candidates in `plan-core::openings` (`snap_center(wall, width, raw, others) -> f64`); render temp dims; typed edit through shared edit field |
| DW-13/DW-14 jamb-to-end clearance, placement dims from nearer end | Clamp to flat 2"; "Distance from Wall Start" only | Medium | Add `distance_from_end` fields; make clearance configurable |
| DW-16..DW-21 click-select, drag along wall, re-host, flip through wall | Cannot select by click; no drag | Critical | Opening in `Selection`; handle system (select-and-edit S-11..S-28); `Project::move_opening(id, wall_id, center)` with validation |
| DW-26..DW-28 resize handles with clamping | None | High | Two handles per jamb; `resize_opening(id, which_jamb, new_edge)` |
| DW-38..DW-46 door types (doorway, sliding, pocket, bifold, garage, barn, shower, fixed) | Only hinged symbol; style is a dialog stub | High | Add `Opening.style: DoorStyle` to the model; implement per-style plan symbol functions in `draw_opening`; wire the flyout entries to `Tool::Door(style)` |
| DW-47..DW-53 window variants (bay, bow, box, pass-through, niche) and mulling | Single window only | Medium | `WindowKind` enum; bay/bow/box need projection geometry; mulling = `mull_group: Option<Id>` |
| DW-54..DW-58 size/elevation rules, component sizes, curved-wall doors | Width/height/sill fields only | Medium | Add `floor_to_top` derivation, size lists in the library layer later |
| DW-6/DW-7 per-type defaults (interior vs exterior door, captured 30x96) | One door template 36x80 and one window template | Medium | Separate `default_door_interior/exterior`; update numeric defaults after confirming in Chief |
| DW-59..DW-63 label, size shorthand, schedule number | Dialog options exist (session only) but no label drawn in plan, not saved | High | Move label settings into `Opening`; draw label in plan using `3068` formatter; schedule ids as `Option<u32>` assigned at creation |
| DW-64 Tab cycle / jamb dimension Tab | None | Low | With selection system |
| DW-66/DW-67 cut through all layers; unaffected rooms | Cut quad hides wall fill; no layers yet | Medium | Follows wall-layer work in `walls.md` W-46..W-50 |
| DW-68/DW-69 openings follow wall edits; break wall assigns | Partially: center_offset preserved by index only; no break | Medium | `revalidate_openings` after any wall edit (see walls W-85) |
| DW-22/DW-23 copy/paste/mirror of openings | None | Low | After clipboard/transform |
| DW-4 adjacent windows touch and mull | Overlap + 2" gap rejected | Low | Allow zero gap for windows when mulling is added |
| Persistence: door/window dialog extras are session-only | Lost on save | High | Promote fields to the `Opening` model with `#[serde(default)]` |
| DW-71..DW-78 placement algorithm: footprint-based host pick, neighbour intervals, alignment candidates | Centerline distance pick; clamp and overlap test only | High | Implement as pure function `plan_core::openings::place(floor, wall_id, raw_t, template, opts) -> Placement` with unit tests covering DW-95 |
| DW-79..DW-86 casing marks, threshold line, opening indicators in plan | Not drawn (dialog options are stubs) | Medium | Draw casing rectangles from `OpeningExtras` once promoted to model |
| DW-87..DW-92 edge cases (T-junction clearance, curved walls, invisible hosts) | Not handled | Medium | Reject hosts with `invisible`; add junction clearance via `joins` helpers |
| DW-101..DW-106 dialog behaviors | Implemented: preview, clamp, size-format radios, OK/Cancel | Low (done) | Keep; add live label preview text |
| DW-107..DW-110 hotkeys and flyout face | Only D,H and Shift+W bound; other door hotkeys are `todo_k` stubs | Medium | Bind remaining keys when styles exist; update flyout face to last used |
| DW-95..DW-100 acceptance scenarios | Not testable yet (no ghost, no drag, no persistence of style) | n/a | Convert to tests after the model changes above; DW-100 is the first (persistence) |
