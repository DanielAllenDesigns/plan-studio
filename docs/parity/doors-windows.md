# Parity spec: doors and windows (openings)

> Status (2026-10-08, re-audited against HEAD 4c11b69): 110 ids: 82 Works, 15 Partial, 12 Missing, 1 Differs-by-design. Per-id evidence and gaps are in [../parity-status.md](../parity-status.md); the spec text below is the original and its code snapshots are out of date.

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
- DW-65: The hover tooltip/status for an opening shows its description and size, e.g. "Hinged Door 3068". **Round 14:** built. The hover tooltip and the status bar read "Hinged Door 3068" (style and size label) for a door or window under the pointer; test `hover_and_selection_are_described_in_the_status_bar`.

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

## 18. Round 14 status (2026-10-08)

Built: DW-3 (the "no" cursor and no ghost where nothing fits), DW-4 (touching windows), DW-5 (click then drag, one undo step), DW-10 and DW-75 (alignment snaps), DW-87 (2 in clearance off the face of a wall that meets the host), DW-35 (Swings Both Directions), DW-57 (Recessed To Layer, plan only), DW-61 (Renumber Schedule), DW-81 (threshold line), DW-82 (door jambs in plan, Size Includes Frame in plan), DW-83 (Opening Indicators), DW-85 (sill line), L-26 and L-29 for doors and windows (callouts with Renumber, Schedule tab). The rules and their open points ("verify in Chief") are in DECISIONS.md and in the Doors and windows, round 14 section of `docs/integration-queue.md`. Code: `tools/opening.rs` and `tools/opening/place.rs` (placement), `plan-core/opening_symbol.rs` (the new `PartKind`s Threshold, Sill and Indicator, and `OpeningSymbol::span`), `editor/opening_edit.rs` (Renumber), `dialogs/opening.rs` (the Sill/Threshold, Opening Indicators and Schedule tabs); scenarios in `scenarios/s26_openings_r14.rs`.


<!-- coverage-audit:start -->
## Coverage audit additions (2026-10-08)

Rows added by the Round 14 coverage audit (`docs/chief-feature-coverage.md`): Chief X18 features found in the menu, toolbar, sub-tool and dialog captures, or known from the product, that no row above covered. Status comes from a code search, not a Chief session; "verify in Chief" marks behavior known only from the product. Variants of one flyout or tab share one row.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| DW-111 | Lintel tab: Lintel profile above the opening with extend and wrap. | Works | dialogs/opening.rs DOOR_TABS "Lintel" live; plan-3d openings |
| DW-112 | Lites tab: Divided lites in a door panel. | Works | dialogs/opening.rs "Lites" live (lites across/vertical, muntin width) |
| DW-113 | Arch tab: Arched head options for the door. | Works | dialogs/opening.rs "Arch" live |
| DW-114 | Framing tab (header, trimmers, king studs, sills): Header construction, trimmer and king stud counts per opening. | Works | dialogs/opening/tabs.rs framing (Framing tab: Include Header, Construction lumber or LVL, Count, Depth or Calculate from Width, Trimmer and King Stud counts, Include Sill); plan-core spec/tabs.rs OpeningFraming; plan-framing wall.rs frame_opening and supports; tests framing::wall::an_opening_overrides_the_header_trimmers_and_king_studs, a_window_sill_can_be_left_out, s42 framing_tab_overrides_reach_plan_framing. Header placement top-of-wall and Combine Headers are not built (verify in Chief) |
| DW-115 | Energy Values tab (U-factor, SHGC): Door type, U-factor and solar heat gain for energy reports. | Works | dialogs/opening/tabs.rs energy_values (door type or glazing, U-Factor, SHGC); spec.energy; plan-docs schedule_kinds U-Factor and SHGC columns; tests schedule_kinds::rough_opening_energy_and_object_information_reach_the_schedule, s42 energy_layer_and_object_information_are_stored_and_undone |
| DW-116 | Layer tab: Layer and drawing group of the door. | Partial | dialogs/opening/tabs.rs layer_tab and spec.layer stored with the opening, Opening::layer_name(); the plan, hit test and layout still draw an opening on its kind's Doors or Windows layer (integration queue: editor/render.rs opening_layer, selection.rs layer_of); test the_layer_follows_the_tab_and_falls_back_to_the_kind |
| DW-117 | Materials tab: Material per door component (panel, frame, casing, glass). | Works | dialogs/opening/tabs.rs materials_tab (library search per component); spec.materials; plan-3d opening.rs paints the unit, sash, casing, sill, jamb and threshold sets; Project::sync_opening_materials hands it to the Material Painter paint; tests plan-3d opening_tabs::the_materials_tab_paints_each_component, s42 materials_paint_the_components_in_3d_and_reach_the_project |
| DW-118 | Object Information tab: Code, comment, manufacturer, supplier and custom fields. | Works | dialogs/opening/tabs.rs object_information (ID, Description, Manufacturer, Model Number, Supplier, Notes shared with the Schedule tab); spec.info; schedule ID and Description columns; test s42 energy_layer_and_object_information_are_stored_and_undone |
| DW-119 | Sash tab: Sash widths, depth, inset, curved options. | Works | dialogs/opening.rs WINDOW_TABS "Sash" live |
| DW-120 | Lites tab: Window lites, muntin width, round-top arch rays. | Works | dialogs/opening.rs "Lites" live |
| DW-121 | Shape tab (heights, corners): Custom window shapes: raked sides, angled top corners and bottom corners. | Works | dialogs/opening/tabs.rs shape_tab (Rectangle, Half Round, Quarter Round, Trapezoid, Triangle, Custom with side heights and corner cuts, Revert All, lite pattern); plan-core spec/shape.rs WindowShape; plan-3d opening/shape.rs; opening_symbol.rs add_arch_marks; tests shape::tests, plan-3d opening_tabs::a_shaped_window_is_glazed_to_its_outline_and_the_wall_fills_the_rest, s42 a_window_shape_changes_the_3d_glazing_and_the_plan_head_marks |
| DW-122 | Arch tab: Arched head options. | Works | dialogs/opening.rs "Arch" live |
| DW-123 | Treatments tab (curtains, blinds, exterior millwork): Curtains, blinds and millwork above and below the casing with library styles. Also covers: Window treatments (curtains, blinds). | Partial | dialogs/opening/tabs.rs treatments_tab (curtains, blinds, interior shutters, exterior millwork above and below); plan-3d opening/treatments.rs; plan omits them; tests plan-3d opening_tabs::curtains_blinds_shutters_and_millwork_are_built_on_their_sides, s42 treatments_are_built_in_3d_and_left_out_of_the_plan. Library styles and the Library... buttons are not built |
<!-- coverage-audit:end -->

## Manual audit additions (part 3)

Rows added by the Chief X18 Reference Manual audit, part 3 (pages 521 to 761; `docs/chief-manual-coverage/part3-text-doors-windows-cabinets-electrical.md`). Each is a manual feature that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the program.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| DW-124 | Door defaults (manual p. 571): Door Defaults dialogs: Interior and Exterior for Hinged and Sliding, plus Doorway, Pocket, Bifold, Garage, Fixed, Barn, Shower; double-click a Door Tools button to open the matching one | Partial | no spec yet (Default Settings > Doors lists Interior Door, Exterior Door (Door Specification as defaults, dialogs/defaults.rs) and Garage Door (a stored page); every other type takes its size from variant defaults in tools/opening.rs, not from its own defaults; no double-click on the button (DS-22)); manual audit part 3; verify in Chief. |
| DW-125 | Door tools (manual p. 572): Doors cannot go in Invisible walls or on locked layers | Missing | no spec yet (DW-92 (invisible walls not refused); tools/opening.rs and place.rs never test the wall's layer lock); manual audit part 3; verify in Chief. |
| DW-126 | Door tools (manual p. 574): Doors & Doorways library catalog (Core Catalogs > Architectural > Doors and Doorways) and placing a library door into a doorway or replacing the door there | Partial | no spec yet (plan-calib catalogs read from the user's install (CB-60); placing a library door as an opening or into an existing doorway: not built (tools/library.rs does not create openings)); manual audit part 3; verify in Chief. |
| DW-127 | Displaying doors (manual p. 575): Doors, labels, opening indicators, header lines and casing controlled by layers; the Doors layer off hides doors but not wall openings | Partial | no spec yet (Doors, Doors Labels layers exist (layers.rs ensure_opening_label_layers); casing layers, Opening Header Lines and Opening Indicators layers are not separate (DW-79, DW-83)); manual audit part 3; verify in Chief. |
| DW-128 | Displaying doors (manual p. 575): Opening Header Lines layer: dashed header lines inside each opening in plan | Missing | no spec yet (no Opening Header Lines layer or dashed header line in the plan symbol (opening_symbol.rs draws jambs, leaf, swing, casing, threshold)); manual audit part 3; verify in Chief. |
| DW-129 | Displaying doors (manual p. 575): Doors in a garage reaching the stem wall show a concrete cutout on the floor below, size and visibility from the Rough Opening tab | Partial | no spec yet (DW-91; concrete_each_side and concrete_show_below are stored (openings/spec/tabs.rs RoughOpening) but nothing in plan-3d or the plan views reads them); manual audit part 3; verify in Chief. |
| DW-130 | Displaying doors (manual p. 575): Doors display in 3D independent of walls when walls are hidden (Hide Camera-Facing Exterior Walls, Display Openings Independent of Walls and Roofs) | Missing | no spec yet (no such 3D view options (3D View Defaults, C-* rows)); manual audit part 3; verify in Chief. |
| DW-131 | Displaying doors (manual p. 575): Plan view: jamb, casing and swing; threshold line across the opening for exterior walls, doors to exterior rooms and between rooms at different floor heights | Partial | no spec yet (DW-81 draws a threshold on exterior-wall doors only (DECISIONS 43); the garage/deck and floor-height-step cases are not drawn); manual audit part 3; verify in Chief. |
| DW-132 | Displaying doors (manual p. 576): 3D: hardware and millwork; threshold choice for exterior doors; Opaque Window/Door Glass rendering technique option with per-door override (Automatic, Opaque, Transparent) | Partial | no spec yet (3D hardware and threshold built (DW-81); the technique option and the per-door Opaque Glass choice do not exist); manual audit part 3; verify in Chief. |
| DW-133 | Displaying doors (manual p. 576): Show Open / Show Closed edit buttons (2D in plan, 3D in camera and section views) for doors, windows and mulled units | Partial | no spec yet (Show Open in 2D / 3D are Options-tab checkboxes (DW-36) and 3D > Show Doors Open toggles the plan; no Edit toolbar buttons, nothing for mulled units); manual audit part 3; verify in Chief. |
| DW-134 | Displaying doors (manual p. 576): Opening indicator arrows in Vector Views for doors, windows and cabinet doors | Partial | no spec yet (spec.indicators draw swing arrows and X marks in plan (DW-83); Vector View arrows in 3D: not drawn); manual audit part 3; verify in Chief. |
| DW-135 | Displaying doors (manual p. 577): Door Schedule: customizable table and schedule-number labels; casing and lintel in Room Finish schedules | Partial | no spec yet (L-23, DW-60, DW-61 for the door schedule and labels; the Room Finish schedule lists floor, base, wall, crown and ceiling finishes (plan-core schedules.rs ROOM_FINISH_FIELDS) and no door or window casing, lintel or sill rows); manual audit part 3; verify in Chief. |
| DW-136 | Displaying doors (manual p. 578): Hinge Side (L/R) and Swing (In/Out) columns viewed from the exterior side | Partial | no spec yet (the Door Schedule has one Swing column (plan-core schedules.rs DOOR_FIELDS 'swing', schedule_kinds.rs); no Hinge Side column and no L/R and In/Out wording from the exterior); manual audit part 3; verify in Chief. |
| DW-137 | Editing doors (manual p. 578): Select a door with Select Objects or any door tool; blocked units select with Select Next Object | Partial | no spec yet (DW-64 (Tab cycling); mulled unit components cannot be selected one at a time inside a unit (DW-137)); manual audit part 3; verify in Chief. |
| DW-138 | Editing doors (manual p. 578): Resize follows the active Edit Behavior; a door moved against an intersecting wall stops when its casing meets the wall (Ignore Casing for Opening Resize preference) | Partial | no spec yet (DW-13 and DECISIONS 42: a flat 2 in off the wall face instead of the casing width; no Ignore Casing preference; Edit Behavior not read by openings); manual audit part 3; verify in Chief. |
| DW-139 | Editing doors (manual p. 579): Edit toolbar: Change Opening/Hinge Side, Change Swing Side, Show Open/Closed, Gable Over Door/Window, Make/Explode Mulled Unit, Add to Library / Add to Library As | Partial | no spec yet (Flip Hinge, Reverse Swing, Center on Wall Segment, Mull, Unmull, Add Transom, Renumber (editor/opening_edit.rs); no Gable Over Door/Window, no Show Open/Closed, no Add to Library for openings); manual audit part 3; verify in Chief. |
| DW-140 | Editing doors (manual p. 580): Door sides: interior side faces the swing by default, reversible; separate materials per side | Partial | no spec yet (Materials tab per component, not per side (DW-117); Reverse Interior/Exterior: no); manual audit part 3; verify in Chief. |
| DW-141 | Special doors (manual p. 583): Wrapped openings from the library, or by clearing Use Interior/Exterior Casing on a doorway so base molding wraps the opening | Partial | no spec yet (Use Interior/Exterior Casing flags exist (DW-79); base molding wrapping a casing-less doorway: R-34 room moldings stop at openings, no wrap); manual audit part 3; verify in Chief. |
| DW-142 | Special doors (manual p. 583): Recessed doors in brick or stone walls (Recessed Into Wall with a structural layer chosen) | Partial | no spec yet (DW-57: recess is a depth from the exterior face (DECISIONS 43), not a wall layer pick; the wall layers do not re-form around the casing); manual audit part 3; verify in Chief. |
| DW-143 | Special doors (manual p. 584): Blocked units of doors and windows count as door or window in the Materials List and schedules | Partial | no spec yet (mulled unit with a door: DW-52; Treat as Door flag: not present (DW-143)); manual audit part 3; verify in Chief. |
| DW-144 | Special doors (manual p. 584): Doorway tool opens a railing; a doorway wider than the railing section opens the whole section; Door Type Door with a gate Door Style | Missing | no spec yet (DW-91, DW-92 family; no railing-opening behaviour verified; gates come from the Fences & Railings library); manual audit part 3; verify in Chief. |
| DW-145 | Special doors (manual p. 585): Gable Over Door/Window edit button creates a gable at the next roof build, editable like any gable line | Missing | no spec yet (no opening-driven gable line (RF-* gable lines exist; grep finds no opening hook)); manual audit part 3; verify in Chief. |
| DW-146 | Special doors (manual p. 585): Custom muntins on glass doors via Load Muntins | Missing | no spec yet (Lites tab holds custom divider positions only; no CAD-block muntins for doors (see windows)); manual audit part 3; verify in Chief. |
| DW-147 | Door Specification: General and Options (manual p. 587): Panel: Automatic height, Bottom Offset, From Floor Finish, Panel Offset | Missing | no spec yet (no panel height or offset on the door (door leaf fills the opening above the sill)); manual audit part 3; verify in Chief. |
| DW-148 | Door Specification: General and Options (manual p. 587): Panel Frame Widths: stile and top rail uniform or separate, bottom rail | Missing | no spec yet (only Lites / Sash widths exist; door panel frame widths are fixed); manual audit part 3; verify in Chief. |
| DW-149 | Door Specification: General and Options (manual p. 588): Options: number of panels (Single Door Only, Double Door Only, Calculate From Width, Custom left/right for slider, pocket, bifold; vertical panels and All Glass for garage) | Partial | no spec yet (Door Panels radios Single, Double, Calculate (DW-30); Custom left/right counts for slider/pocket/bifold and garage vertical panels: not editable); manual audit part 3; verify in Chief. |
| DW-150 | Door Specification: other panels (manual p. 594): Arch panel: arch type (Round Top, Octagonal, Tudor, Double), height, radius, Reflect Vertically, Full / Left / Right arch | Partial | no spec yet (Arch tab: No Arch, Round Top, Segmental, Tudor, Gothic, Eyebrow with height (DW-113); no Octagonal or Double, no Reflect Vertically, no Left/Right arch); manual audit part 3; verify in Chief. |
| DW-151 | Door Specification: other panels (manual p. 595): Hardware panel: interior and exterior handle and lock from lists or the library with Edit (size and angle), handle position In From Door Edge / Up From Bottom, hinges style and count and offsets, Match Interior | Partial | no spec yet (Hardware tab: Handle style, height, in-from-edge, number of hinges, hinge inset (DW-55); locks, library hardware, Edit size dialog, Match Interior: no); manual audit part 3; verify in Chief. |
| DW-152 | Door Specification: other panels (manual p. 599): Framing panel: Include Header with Construction (framing member Define), Determine Framing Type from Wall Layer, Framing Method (Standard, Flat, Boxed), Count to 10, Thickness, Depth with Calculate from Width, Evenly Spaced and Spacing, Depth and Vertical Placement, Combine Headers with Maximum Distance, Trimmer and King Stud construction and count, bottom and top Sills with double options | Partial | no spec yet (Framing tab: include header, plies, depth, lumber/LVL, trimmer and king stud counts, window sill (DW-114, DECISIONS 313); no method, placement, combine headers, member constructions or sill thickness options); manual audit part 3; verify in Chief. |
| DW-153 | Door Specification: other panels (manual p. 602): Manufacturer panel (contact information) for manufacturer-catalog doors | Missing | no spec yet (no Manufacturer tab on any dialog; object Manufacturer text exists in Object Information (CB-83)); manual audit part 3; verify in Chief. |
| DW-154 | Window defaults (manual p. 603): Window Defaults dialog (double-click the Window Tools button); panels like the Window Specification, plus Minimum Separation between window and door units | Partial | no spec yet (Default Settings > Windows > Window opens the Window Specification as defaults (DefaultsEntry::Window); one defaults set for all window types; no Minimum Separation field and no double-click on the button (DS-22)); manual audit part 3; verify in Chief. |
| DW-155 | Window tools (manual p. 604): Bay, Box and Bow have no preview; refused with a warning under a 30 in width; they need a single straight wall | Partial | no spec yet (DW-48: Bay, Bow, Box are projecting window styles placed like windows with a ghost; no 30 in rule, no warning (DW-155)); manual audit part 3; verify in Chief. |
| DW-156 | Special windows (manual p. 606): Corner windows: two fixed windows moved into the corner, Mitered or Post join, same heights and sills, shaped tops allowed with equal corner heights | Partial | no spec yet (Frame tab Corner Join Mitered / Post and Muntin in Corner (DW-82 family, DECISIONS 317); moving a window into a corner uses the end handle, the move-past-corner gesture and its minimal post are not built); manual audit part 3; verify in Chief. |
| DW-157 | Special windows (manual p. 607): Window symbols imported to the library; symbols are type Custom in schedules with an automatic CU label; symbol Type and plan CAD block set in the Symbol Specification | Partial | no spec yet (symbols placed from Chief catalogs are symbol objects (placed.rs); window-symbol type and schedule type: no Type field in dialogs/symbol.rs); manual audit part 3; verify in Chief. |
| DW-158 | Grouped, mulled and stacked windows (manual p. 608): Automatically mulled openings: casings touching share one casing; sills equal; windows mull to doors when the sill is at the floor; Minimum Separation sets the shared casing width | Missing | no spec yet (DECISIONS 42: two windows may touch but are not mulled until Mull is pressed (DW-51, DW-89)); manual audit part 3; verify in Chief. |
| DW-159 | Grouped, mulled and stacked windows (manual p. 609): Multiple stacked openings: four or more at one place show a Caution symbol with Delete Duplicate | Missing | no spec yet (Project::add_opening refuses overlapping openings (openings_conflict); stacking at one place is allowed only with separate heights, and no Caution symbol or Delete Duplicate exists); manual audit part 3; verify in Chief. |
| DW-160 | Grouped, mulled and stacked windows (manual p. 609): Make Mulled Unit: windows and doors in one wall within 24 in side to side or top to bottom, parallel straight facing edges, either horizontal or vertical, nested units for complex ones; inherits Mulled Unit Defaults | Partial | no spec yet (Project::mull_openings (DW-51, DW-52): adjacent openings in a wall; vertical stacks, the 24 in rule and nested units: not modelled; no Mulled Unit Defaults dialog); manual audit part 3; verify in Chief. |
| DW-161 | Grouped, mulled and stacked windows (manual p. 611): Window Levels: stacked openings can be given a level; level 0 draws in the layer colour and is picked first, other levels draw light grey; dimension lines locate only level 0 | Missing | no spec yet (no window level field; stacked openings overlap in plan (DW-74 Partial)); manual audit part 3; verify in Chief. |
| DW-162 | Displaying windows (manual p. 613): Windows in non-displayed parts of pony walls: outline only, or full display, or hidden (Pony Wall Defaults) | Missing | no spec yet (pony wall defaults page has no window display option); manual audit part 3; verify in Chief. |
| DW-163 | Editing windows (manual p. 616): Edit toolbar: Gable Over, Show Open/Closed, Change Wall Side (niche), Make/Explode Mulled Unit, Add to Library | Partial | no spec yet (Mull, Unmull, Reverse Side (swing/projection), Add Transom, Center on Wall Segment, Renumber Schedule; no Change Wall Side for niches, no Add to Library (DW-139)); manual audit part 3; verify in Chief. |
| DW-164 | Window Specification: General and Options (manual p. 619): Window Type list with Use Default (Fixed, Single/Double Hung, Single/Double/Triple Casement, Left/Right/Triple Sliding, Single/Double/Triple Awning, Double/Triple Hopper, Louvered, Glass Louver, Pass-Through) | Partial | no spec yet (Window Style list: Window (double hung), Casement, Fixed, Sliding, Awning, Hopper, Pass-Through, Niche (OpeningStyle); no single hung, double/triple multiples or louvered); manual audit part 3; verify in Chief. |
| DW-165 | Window Specification: General and Options (manual p. 627): Shape panel: Match Roof, Revert All, left/right side heights, Top Inside Corners with height and offset, bottom corners; custom shape forces Fixed Glass | Partial | no spec yet (Shape tab with Rectangle, Half Round, Quarter Round, Trapezoid, Triangle, Custom incl. inner corners, bottom corners, Revert All (DW-121, DECISIONS 314); Match Roof button: no); manual audit part 3; verify in Chief. |
| DW-166 | Window Specification: General and Options (manual p. 630): Shutters panel: library type, match opening width/height or fixed, position (On Casing, Outside Casing, Custom offsets), Louver Size, Reverse Direction, Match Arch, Show Closed, Sides (Automatic, Left, Both, Right) | Partial | no spec yet (Shutters tab: style, sides, width, colour, closed, outside casing, louver size (DW-84); Match Opening Height/Arch, custom offsets, Reverse Direction, library types: no); manual audit part 3; verify in Chief. |
| DW-167 | Bay, Box and Bow windows (manual p. 636): Roof options for bay/box/bow: default hip with California ridge for angled sides, Use Existing Roof, Extend Existing Roof Over, Rectangular Roof Over; rebuild roofs to apply; gable by editing planes | Differs-by-design | no spec yet (plan-3d roof.rs bay_roof_into builds a fixed 6:12 hip (bay, bow) or shed (box) for the unit only (DECISIONS 125, RF-29); none of the three roof options); manual audit part 3; verify in Chief. |

## Manual audit additions (part 7)

Rows added by the Chief X18 Tutorial Guide audit, part 7 (pages 1 to 517; `docs/chief-manual-coverage/part7-tutorial-workflows.md`). Each is a workflow step the tutorials rely on that had no parity row. Status comes from a code search on 2026-10-08 (Round 15 builders still editing); lines marked verify in Chief are not confirmed against the application.

| ID | Chief behavior | Status | Evidence |
|---|---|---|---|
| DW-168 | Door lock hardware picked from the library as symbol options (Interior Lock, Exterior Lock: Dead Bolt interior/exterior) (tutorial p. 89). | Missing | Hardware tab uses built-in handle choices (opening_tabs brief); no lock symbols; library picker is CB-395; tutorial audit (part 7). |
