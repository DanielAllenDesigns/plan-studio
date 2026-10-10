//! Door and window Edit toolbar commands: Center on Wall Segment (DW-24),
//! Make Mulled Unit and Explode Mulled Unit (DW-51, DW-52, DW-160: windows
//! and doors of one wall within 24 in, side by side or stacked), Select Next
//! Object (stacked openings and the components of a unit), Set as Default and
//! Explode Bay/Bow Window (DW-48, DW-124), Delete Duplicate (the Caution
//! symbol), Show Open/Closed and the Gable Over Door/Window hook, and
//! `follow_defaults` that makes openings on Use Default follow a changed
//! default (manual pp. 103, 104). Also Reset Label Position (DW-63), and the standard widths a jamb handle snaps
//! to (DW-27), and Renumber Schedule (DW-61, L-26: the marks of the doors or
//! the windows set in the order they were drawn). The buttons come from `EditorContext::extra_edit_actions` and
//! run through `run_custom`.

use super::selection::ObjectRef;
use super::{EditAction, EditActionKind, EditorContext};
use crate::dialogs::OpeningTarget;
use plan_core::openings::mull::UNIT_REACH;
use plan_core::openings::StandardWidths;
use plan_core::schedules::{Numbering, ScheduleKind};
use plan_core::{Id, OpeningKind, OpeningStyle, Project};

/// Custom command ids (the `id` of `EditActionKind::Custom`).
pub const CENTER_SEGMENT: &str = "opening.center_segment";
pub const MULL: &str = "opening.mull";
pub const UNMULL: &str = "opening.unmull";
pub const REVERSE_SIDE: &str = "opening.reverse_side";
pub const RESET_LABEL: &str = "opening.reset_label";
/// Add a transom over the selected door or window.
pub const ADD_TRANSOM: &str = "opening.add_transom";
/// Renumber Schedule for the kinds of the selected openings (Edit toolbar).
pub const RENUMBER: &str = "opening.renumber";
/// Schedules menu: Renumber Door / Window Schedule.
pub const RENUMBER_DOORS: &str = "opening.renumber_doors";
pub const RENUMBER_WINDOWS: &str = "opening.renumber_windows";
/// 3D menu: show every door open / closed.
pub const DOORS_OPEN: &str = "opening.doors_open_3d";
/// 3D menu: casing, jambs, sills and thresholds on / off.
pub const CASING_3D: &str = "opening.casing_3d";
/// Select Next Object: the next opening in the stack under the selected one.
pub const SELECT_NEXT: &str = "opening.select_next";
/// Set as Default: the selected opening becomes the default of its type.
pub const SET_DEFAULT: &str = "opening.set_default";
/// Explode Bay/Bow Window into walls, windows and a room.
pub const EXPLODE_BAY: &str = "opening.explode_bay";
/// Delete Duplicate from the Caution symbol's menu.
pub const DELETE_DUPLICATE: &str = "opening.delete_duplicate";
/// Show Open / Show Closed in 2D and 3D.
pub const SHOW_OPEN_2D: &str = "opening.show_open_2d";
pub const SHOW_CLOSED_2D: &str = "opening.show_closed_2d";
pub const SHOW_OPEN_3D: &str = "opening.show_open_3d";
pub const SHOW_CLOSED_3D: &str = "opening.show_closed_3d";
/// Height of a new transom, inches.
const TRANSOM_HEIGHT: f64 = 18.0;
/// Flip Hinge is run by the shared edit commands (`edit.flip_hinge`).
const FLIP_HINGE: &str = "edit.flip_hinge";

fn selected_openings(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Opening(id) => Some(*id),
            _ => None,
        })
        .collect()
}

/// The standard widths in force for openings of `kind`: what a Default
/// Settings dialog set this session, else the plan defaults (DW-27).
pub fn standard_widths(cx: &EditorContext, kind: OpeningKind) -> StandardWidths {
    let keys: &[OpeningTarget] = match kind {
        OpeningKind::Door => &[
            OpeningTarget::DefaultDoor,
            OpeningTarget::DefaultExteriorDoor,
        ],
        OpeningKind::Window => &[OpeningTarget::DefaultWindow],
    };
    keys.iter()
        .find_map(|k| {
            cx.extras
                .openings
                .get(&k.key())
                .and_then(|e| e.standard_widths())
        })
        .cloned()
        .unwrap_or_else(|| cx.defaults.opening_variants.widths.clone())
}

/// Whether `o` can be part of a mulled unit: any door or window but a bay,
/// box or bow window or a wall niche.
fn mullable(o: &plan_core::Opening) -> bool {
    !o.style.projects() && o.style != OpeningStyle::WallNiche
}

/// The opening beside `id` it would be blocked with: the nearest one on the
/// same wall within [`UNIT_REACH`] with nothing in between, not yet in its
/// unit.
fn mull_partner(cx: &EditorContext, id: Id) -> Option<Id> {
    let f = cx.floor();
    let me = f.openings.iter().find(|o| o.id == id)?;
    if !mullable(me) {
        return None;
    }
    let unit = cx.project.mull_members(cx.floor, id);
    let (lo, hi) = cx.project.unit_span(cx.floor, id)?;
    let mut best: Option<(f64, Id)> = None;
    for o in f.openings_on(me.wall_id) {
        if unit.contains(&o.id) || !mullable(o) {
            continue;
        }
        let gap = if o.start_offset() >= hi - 1e-9 {
            o.start_offset() - hi
        } else if o.end_offset() <= lo + 1e-9 {
            lo - o.end_offset()
        } else {
            continue;
        };
        if gap <= UNIT_REACH + 1e-9 && best.is_none_or(|(g, _)| gap < g) {
            // Nothing else may sit in the gap.
            let (a, b) = if o.start_offset() >= hi - 1e-9 {
                (hi, o.start_offset())
            } else {
                (o.end_offset(), lo)
            };
            let blocked = f.openings_on(me.wall_id).any(|x| {
                x.id != o.id
                    && !unit.contains(&x.id)
                    && x.end_offset() > a + 1e-9
                    && x.start_offset() < b - 1e-9
            });
            if !blocked {
                best = Some((gap, o.id));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// The openings Make Mulled Unit would block: the selected doors and windows,
/// or a lone selected one and its nearest neighbour.
fn mull_set(cx: &EditorContext) -> Vec<Id> {
    let sel = selected_openings(cx);
    let f = cx.floor();
    let windows: Vec<Id> = sel
        .iter()
        .copied()
        .filter(|id| f.openings.iter().any(|o| o.id == *id && mullable(o)))
        .collect();
    match windows.as_slice() {
        [one] if sel.len() == 1 => mull_partner(cx, *one)
            .map(|p| vec![*one, p])
            .unwrap_or_default(),
        [] | [_] => Vec::new(),
        many => many.to_vec(),
    }
}

/// The opening buttons of the Edit toolbar for the current selection.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let sel = selected_openings(cx);
    if sel.is_empty() || cx.selection.items.len() != sel.len() {
        return Vec::new();
    }
    let button = |id, label, icon: &'static str, enabled| EditAction {
        kind: EditActionKind::Custom { id, label, icon },
        label,
        icon: (!icon.is_empty()).then_some(icon),
        enabled,
    };
    let mut v = Vec::new();
    let f = cx.floor();
    let find = |id: &Id| f.openings.iter().find(|o| o.id == *id);
    // Doors with a hinge can have it moved; windows with a swing or
    // projection have a side that can be reversed.
    if sel.iter().filter_map(find).any(|o| {
        o.kind == OpeningKind::Door
            && matches!(
                o.style,
                OpeningStyle::Hinged
                    | OpeningStyle::Shower
                    | OpeningStyle::Pocket
                    | OpeningStyle::Barn
                    | OpeningStyle::Sliding
                    | OpeningStyle::Bifold
            )
    }) {
        v.push(button(FLIP_HINGE, "Flip Hinge", "", true));
    }
    if sel.iter().filter_map(find).any(|o| {
        o.kind == OpeningKind::Window
            && matches!(
                o.style,
                OpeningStyle::Casement
                    | OpeningStyle::Awning
                    | OpeningStyle::Hopper
                    | OpeningStyle::BayWindow
                    | OpeningStyle::BowWindow
                    | OpeningStyle::BoxWindow
            )
    }) {
        v.push(button(REVERSE_SIDE, "Reverse Side", "", true));
    }
    if let [one] = sel.as_slice() {
        let room = cx.project.free_span(cx.floor, *one, true).is_some();
        v.push(button(CENTER_SEGMENT, "Center on Wall Segment", "", room));
    }
    if !mull_set(cx).is_empty() {
        v.push(button(MULL, "Make Mulled Unit", "", true));
    }
    // Select Next Object: stacked openings and the components of a unit.
    if let [one] = sel.as_slice() {
        if cx.project.select_next_opening(cx.floor, *one).is_some() {
            v.push(button(SELECT_NEXT, "Select Next Object", "", true));
        }
    }
    // Set as Default, for one opening or a few of one kind.
    v.push(button(SET_DEFAULT, "Set as Default", "", sel.len() == 1));
    if let [one] = sel.as_slice() {
        if find(one).is_some_and(|o| o.style.projects()) {
            v.push(button(EXPLODE_BAY, "Explode Bay/Bow Window", "", true));
        }
    }
    // Delete Duplicate, from the Caution symbol over four or more openings.
    if cx
        .project
        .stacked_clusters(cx.floor)
        .iter()
        .any(|c| c.ids.iter().any(|i| sel.contains(i)))
    {
        v.push(button(DELETE_DUPLICATE, "Delete Duplicate", "", true));
    }
    // Show Open / Show Closed in 2D and 3D (manual p. 613).
    let opens = sel.iter().filter_map(find).any(|o| {
        o.kind == OpeningKind::Door
            || matches!(
                o.style,
                OpeningStyle::Casement
                    | OpeningStyle::Awning
                    | OpeningStyle::Hopper
                    | OpeningStyle::SlidingWindow
                    | OpeningStyle::Window
            )
    });
    if opens {
        let any = |f: &dyn Fn(&plan_core::Opening) -> bool| sel.iter().filter_map(find).any(f);
        let open2 = any(&|o| o.extras.show_open_in_plan);
        let open3 = any(&|o| o.extras.spec.show_open_in_3d);
        v.push(if open2 {
            button(SHOW_CLOSED_2D, "Show Closed in 2D", "", true)
        } else {
            button(SHOW_OPEN_2D, "Show Open in 2D", "", true)
        });
        v.push(if open3 {
            button(SHOW_CLOSED_3D, "Show Closed in 3D", "", true)
        } else {
            button(SHOW_OPEN_3D, "Show Open in 3D", "", true)
        });
    }
    // Gable Over Door/Window (the roof brief draws the gable at the next
    // roof build): the button lives here, the work in `tools::gable_line`.
    v.extend(crate::tools::gable_line::edit_actions(cx));
    if let [one] = sel.as_slice() {
        if find(one).is_some_and(mullable) && transom_fits(cx, *one) {
            v.push(button(ADD_TRANSOM, "Add Transom", "", true));
        }
    }
    if sel
        .iter()
        .filter_map(find)
        .any(|o| o.extras.spec.label_offset != (0.0, 0.0))
    {
        v.push(button(RESET_LABEL, "Reset Label Position", "", true));
    }
    if f.openings
        .iter()
        .any(|o| sel.contains(&o.id) && o.mull_group.is_some())
    {
        v.push(button(UNMULL, "Explode Mulled Unit", "", true));
    }
    v.push(button(RENUMBER, "Renumber Schedule", "", true));
    v
}

/// Runs an opening command; false when `id` is not one of them.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        CENTER_SEGMENT => center_on_segment(cx),
        MULL => mull(cx),
        UNMULL => unmull(cx),
        REVERSE_SIDE => reverse_side(cx),
        RESET_LABEL => reset_label(cx),
        ADD_TRANSOM => add_transom(cx),
        RENUMBER => renumber_selected(cx),
        RENUMBER_DOORS => renumber(cx, &[OpeningKind::Door]),
        RENUMBER_WINDOWS => renumber(cx, &[OpeningKind::Window]),
        DOORS_OPEN => toggle_doors_open(cx),
        CASING_3D => toggle_casing(cx),
        SELECT_NEXT => select_next(cx),
        SET_DEFAULT => set_as_default(cx),
        EXPLODE_BAY => explode_bay(cx),
        DELETE_DUPLICATE => delete_duplicate(cx),
        SHOW_OPEN_2D => show_open(cx, true, false),
        SHOW_CLOSED_2D => show_open(cx, false, false),
        SHOW_OPEN_3D => show_open(cx, true, true),
        SHOW_CLOSED_3D => show_open(cx, false, true),
        _ => return crate::tools::gable_line::run_command(cx, id),
    }
    true
}

fn schedule_kind(kind: OpeningKind) -> ScheduleKind {
    match kind {
        OpeningKind::Door => ScheduleKind::Door,
        OpeningKind::Window => ScheduleKind::Window,
    }
}

/// Sets the mark of every door or window of the plan in the order they were
/// drawn: floor by floor, in the order each floor holds them (Renumber
/// Schedule, DW-61). A mark is `prefix` and a two-digit number (`D01`);
/// [`Numbering::ByFloor`] restarts on every floor. An opening left out of the
/// schedule has no mark. Returns how many marks changed.
pub fn renumber_marks(
    project: &mut Project,
    kind: OpeningKind,
    prefix: &str,
    numbering: Numbering,
) -> usize {
    let mut changed = 0;
    let mut whole = 0usize;
    for f in &mut project.floors {
        let mut on_floor = 0usize;
        for o in f.openings.iter_mut().filter(|o| o.kind == kind) {
            let mark = if o.extras.spec.schedule.include {
                on_floor += 1;
                whole += 1;
                let n = match numbering {
                    Numbering::ByFloor => on_floor,
                    Numbering::Whole => whole,
                };
                Some(format!("{prefix}{n:02}"))
            } else {
                None
            };
            if o.schedule_number != mark {
                o.schedule_number = mark;
                changed += 1;
            }
        }
    }
    changed
}

/// Renumber Schedule (DW-61, L-26) for the doors and/or windows of the plan:
/// every schedule of those kinds closes the gaps in its numbers and keeps the
/// order of its rows (manual p. 715, DECISIONS 44). The numbers are the
/// schedule's own, kept in the schedule; no mark is written into the
/// openings, so a door added afterwards simply takes the next number. One
/// undo step.
pub fn renumber(cx: &mut EditorContext, kinds: &[OpeningKind]) {
    let sks: Vec<ScheduleKind> = kinds.iter().map(|k| schedule_kind(*k)).collect();
    let n = super::schedule_view::renumber_kinds(cx, &sks);
    if n == 0 {
        cx.status = "The schedule numbers have no gaps".into();
    } else {
        cx.status = format!("Renumbered {n} schedule(s): the gaps are closed");
    }
}

/// The Edit toolbar's Renumber Schedule: the kinds among the selection.
fn renumber_selected(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    let f = cx.floor();
    let mut kinds: Vec<OpeningKind> = Vec::new();
    for o in f.openings.iter().filter(|o| sel.contains(&o.id)) {
        if !kinds.contains(&o.kind) {
            kinds.push(o.kind);
        }
    }
    if !kinds.is_empty() {
        renumber(cx, &kinds);
    }
}

/// Show every door of the plan open (or closed again) in the 3D view; kept
/// in the plan (`Project::opening_display`).
pub fn toggle_doors_open(cx: &mut EditorContext) {
    cx.begin_change("Show Doors Open");
    let d = &mut cx.project.opening_display;
    d.doors_open = !d.doors_open;
    cx.status = if d.doors_open {
        "Doors are shown open in 3D".into()
    } else {
        "Doors are shown closed in 3D".into()
    };
    cx.mark_dirty();
}

/// Casing, jambs, window sills and thresholds in the 3D view on or off.
pub fn toggle_casing(cx: &mut EditorContext) {
    cx.begin_change("Casing in 3D");
    let d = &mut cx.project.opening_display;
    d.casing = !d.casing;
    cx.status = if d.casing {
        "Casing, jambs and sills are shown in 3D".into()
    } else {
        "Casing, jambs and sills are hidden in 3D".into()
    };
    cx.mark_dirty();
}

/// Add a transom over the selected door or window (DW-52): a fixed window as
/// wide as its unit, mulled into it so the two share one frame and casing.
pub fn add_transom(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    let [id] = sel.as_slice() else {
        cx.status = "Select one door or window".into();
        return;
    };
    if locked(cx, &sel) {
        return;
    }
    cx.begin_change("Add Transom");
    let fl = cx.floor;
    match cx.project.add_transom(fl, *id, TRANSOM_HEIGHT) {
        Ok(new) => {
            cx.selection.items = vec![ObjectRef::Opening(new)];
            cx.mark_dirty();
            cx.status = "Added a transom".into();
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = e;
        }
    }
}

/// Whether a transom fits over the unit of `id`: no window over it yet and
/// room between its top and the top of the wall.
fn transom_fits(cx: &EditorContext, id: Id) -> bool {
    let f = cx.floor();
    let members = cx.project.mull_members(cx.floor, id);
    let in_unit: Vec<&plan_core::Opening> = f
        .openings
        .iter()
        .filter(|o| members.contains(&o.id))
        .collect();
    let Some(wall) = in_unit.first().and_then(|o| f.wall(o.wall_id)) else {
        return false;
    };
    let top = in_unit
        .iter()
        .map(|o| o.sill_height + o.height)
        .fold(0.0_f64, f64::max);
    let covered = f.openings_on(wall.id).any(|o| {
        in_unit
            .iter()
            .any(|u| plan_core::openings::stands_over(o, u))
    });
    !covered && wall.height - top >= plan_core::openings::MIN_TRANSOM_HEIGHT
}

/// Puts the labels of the selected openings back on their default spots.
pub fn reset_label(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    if sel.is_empty() || locked(cx, &sel) {
        return;
    }
    cx.begin_change("Reset Label Position");
    let fl = cx.floor;
    let mut n = 0;
    for o in cx.project.floors[fl].openings.iter_mut() {
        if sel.contains(&o.id) && o.extras.spec.label_offset != (0.0, 0.0) {
            o.extras.spec.label_offset = (0.0, 0.0);
            n += 1;
        }
    }
    if n > 0 {
        cx.mark_dirty();
    } else {
        cx.cancel_change();
    }
}

fn locked(cx: &mut EditorContext, ids: &[Id]) -> bool {
    ids.iter()
        .any(|id| !cx.check_unlocked(ObjectRef::Opening(*id)))
}

/// Center Object for an opening (DW-24): midway in the free span between its
/// neighbours and the wall ends.
pub fn center_on_segment(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    let [id] = sel.as_slice() else {
        cx.status = "Select one door or window".into();
        return;
    };
    if locked(cx, &sel) {
        return;
    }
    cx.begin_change("Center Object");
    let fl = cx.floor;
    if cx.project.center_opening(fl, *id) {
        cx.mark_dirty();
        cx.status.clear();
    } else {
        cx.cancel_change();
        cx.status = "It is already centered, or there is no room to center it".into();
    }
}

/// Make Mulled Unit (DW-51, DW-160): blocks the selected doors and windows (or
/// a lone one and its neighbour) into one unit. Nothing moves; the unit
/// starts from the Mulled Unit Defaults.
pub fn mull(cx: &mut EditorContext) {
    let ids = mull_set(cx);
    if ids.len() < 2 {
        cx.status = "Select the doors and windows to block into a unit".into();
        return;
    }
    if locked(cx, &ids) {
        return;
    }
    cx.begin_change("Make Mulled Unit");
    let fl = cx.floor;
    let defaults = cx.defaults.window.mulled.clone();
    match cx.project.make_mulled_unit(fl, &ids, &defaults) {
        Ok(_) => {
            cx.mark_dirty();
            cx.status = format!("Blocked {} openings into a mulled unit", ids.len());
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = e;
        }
    }
}

/// Reverse Side for windows: a casement swings, an awning or hopper opens and
/// a bay, bow or box window projects to the other side of the wall.
pub fn reverse_side(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    if sel.is_empty() || locked(cx, &sel) {
        return;
    }
    cx.begin_change("Reverse Side");
    let fl = cx.floor;
    let mut n = 0;
    for o in cx.project.floors[fl].openings.iter_mut() {
        if sel.contains(&o.id) && o.kind == OpeningKind::Window {
            o.swing_flipped = !o.swing_flipped;
            n += 1;
        }
    }
    if n == 0 {
        cx.cancel_change();
    } else {
        cx.mark_dirty();
    }
}

/// Explode Mulled Unit (DW-51): the unit of the selected opening becomes
/// separate windows and doors again.
pub fn unmull(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    if locked(cx, &sel) {
        return;
    }
    cx.begin_change("Explode Mulled Unit");
    let fl = cx.floor;
    let n: usize = sel
        .iter()
        .map(|id| cx.project.explode_mulled_unit(fl, *id))
        .sum();
    if n == 0 {
        cx.cancel_change();
        cx.status = "That opening is not part of a mulled unit".into();
    } else {
        cx.mark_dirty();
        cx.status = format!("Exploded the unit into {n} openings");
    }
}

/// Keeps the plan's Minimum Separation and Ignore Casing for Opening Resize in
/// step with the Window Defaults (the model functions read them from the
/// plan).
pub fn sync_rules(cx: &mut EditorContext) {
    let sep = cx.defaults.window.min_separation;
    let ignore = cx.defaults.window.ignore_casing;
    let d = &mut cx.project.opening_display;
    if d.min_separation != sep || d.ignore_casing != ignore {
        d.min_separation = sep;
        d.ignore_casing = ignore;
        cx.mark_dirty();
    }
}

/// Dynamic defaults (manual p. 103): every opening that is set to use the
/// default takes the values the defaults hold now. Returns how many changed.
/// Run after a defaults dialog changed `opening_variants` and whenever a tool
/// places an opening.
pub fn follow_defaults(cx: &mut EditorContext) -> usize {
    // Only when a default changed since the openings last followed: an undo
    // of Set as Default is not taken back at the next placement.
    // A mulled unit edited in a dialog goes to all its components and a
    // recess to a wall layer takes the layer's depth, whatever the defaults do.
    let mut n = cx.project.sync_unit_specs() + cx.project.sync_recess_depths();
    if cx.defaults.opening_variants.needs_follow() {
        let v = cx.defaults.opening_variants.clone();
        n += cx.project.follow_type_defaults(&v);
        cx.defaults.opening_variants.mark_followed();
    }
    if n > 0 {
        cx.mark_dirty();
    }
    n
}

/// Where the diamond-shaped Depth handle of the bay, box or bow window `id`
/// stands (manual p. 634): the middle of its front.
pub fn bay_depth_handle(cx: &EditorContext, id: Id) -> Option<plan_core::geometry::Point> {
    let f = cx.floor();
    let o = f
        .openings
        .iter()
        .find(|o| o.id == id && o.style.projects())?;
    let wall = f.wall(o.wall_id)?;
    let ext = plan_core::exterior_sign(wall, &cx.rooms);
    let sign = plan_core::openings::bay::unit_side(o, ext);
    let depth = o.extras.spec.bay.depth_for(o.style);
    Some(
        wall.point_along(o.center_offset).add(
            wall.normal_along(o.center_offset)
                .scale(sign * (wall.thickness * 0.5 + depth)),
        ),
    )
}

/// Drags the Depth handle of the unit `id` to `to`: outward increases the
/// depth, inward decreases it. Returns whether the depth changed. The caller
/// opens the undo step.
pub fn drag_bay_depth(cx: &mut EditorContext, id: Id, to: plan_core::geometry::Point) -> bool {
    let fl = cx.floor;
    let f = cx.floor();
    let Some(o) = f.openings.iter().find(|o| o.id == id && o.style.projects()) else {
        return false;
    };
    let Some(wall) = f.wall(o.wall_id) else {
        return false;
    };
    let ext = plan_core::exterior_sign(wall, &cx.rooms);
    let sign = plan_core::openings::bay::unit_side(o, ext);
    let n = wall.normal_along(o.center_offset).scale(sign);
    let out = to.sub(wall.point_along(o.center_offset)).dot(n) - wall.thickness * 0.5;
    let before = o.extras.spec.bay.depth_for(o.style);
    cx.project.set_bay_depth(fl, id, out) && (out.clamp(6.0, 96.0) - before).abs() > 1e-9
}

/// Select Next Object (manual pp. 258, 610, 612): the next opening in the
/// stack under the selected one, level 0 first.
pub fn select_next(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    let [id] = sel.as_slice() else {
        cx.status = "Select one door or window".into();
        return;
    };
    match cx.project.select_next_opening(cx.floor, *id) {
        Some(next) => {
            cx.selection.set(ObjectRef::Opening(next));
            cx.status = "Selected the next object".into();
        }
        None => cx.status = "There is nothing else at that place".into(),
    }
}

/// Set as Default (manual p. 104): the selected opening becomes the default of
/// its type, and the openings using the default follow it. One undo step (the
/// openings; the default itself is a setting of the plan, not an edit).
pub fn set_as_default(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    let [id] = sel.as_slice() else {
        cx.status = "Select one door or window".into();
        return;
    };
    let fl = cx.floor;
    let Some(o) = cx.floor().openings.iter().find(|o| o.id == *id).cloned() else {
        return;
    };
    let wall_kind = cx
        .floor()
        .wall(o.wall_id)
        .map_or(plan_core::WallKind::Interior, |w| w.kind);
    cx.begin_change("Set as Default");
    let key = cx.defaults.opening_variants.set_as_default(&o, wall_kind);
    let v = cx.defaults.opening_variants.clone();
    cx.project.follow_type_defaults(&v);
    cx.defaults.opening_variants.mark_followed();
    // The opening itself keeps its look; from now on it follows the default.
    if let Some(me) = cx.project.floors[fl]
        .openings
        .iter_mut()
        .find(|x| x.id == *id)
    {
        me.extras.spec.dynamic = plan_core::openings::types::UseDefault::all(me.kind);
    }
    cx.mark_dirty();
    cx.status = format!("{} defaults set from the selected object", key.name());
}

/// Explode Bay/Bow Window (manual p. 636).
pub fn explode_bay(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    let [id] = sel.as_slice() else {
        cx.status = "Select one bay, box or bow window".into();
        return;
    };
    if locked(cx, &sel) {
        return;
    }
    let fl = cx.floor;
    let Some(wall) = cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == *id)
        .and_then(|o| cx.floor().wall(o.wall_id))
        .cloned()
    else {
        return;
    };
    let exterior = plan_core::exterior_sign(&wall, &cx.rooms);
    cx.begin_change("Explode Bay/Bow Window");
    match cx.project.explode_bay(fl, *id, exterior) {
        Ok(r) => {
            cx.selection.items = r.walls.iter().map(|w| ObjectRef::Wall(*w)).collect();
            cx.mark_dirty();
            cx.status = format!(
                "Exploded into {} walls and {} windows",
                r.walls.len(),
                r.windows.len()
            );
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = e;
        }
    }
}

/// Delete Duplicate (manual p. 609): the newest opening of a Caution cluster.
pub fn delete_duplicate(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    let fl = cx.floor;
    let Some(id) = sel.iter().copied().find(|i| {
        cx.project
            .stacked_clusters(fl)
            .iter()
            .any(|c| c.ids.contains(i))
    }) else {
        cx.status = "There is no Caution at that place".into();
        return;
    };
    cx.begin_change("Delete Duplicate");
    match cx.project.delete_duplicate(fl, id) {
        Some(gone) => {
            cx.selection
                .items
                .retain(|o| *o != ObjectRef::Opening(gone));
            cx.mark_dirty();
        }
        None => cx.cancel_change(),
    }
}

/// Show Open / Show Closed (manual p. 613): in 2D the plan symbol, in 3D the
/// leaf or sash. The components of a mulled unit all follow.
pub fn show_open(cx: &mut EditorContext, open: bool, three_d: bool) {
    let mut ids = selected_openings(cx);
    let fl = cx.floor;
    for id in ids.clone() {
        for m in cx.project.mull_members(fl, id) {
            if !ids.contains(&m) {
                ids.push(m);
            }
        }
    }
    if ids.is_empty() || locked(cx, &ids) {
        return;
    }
    cx.begin_change(match (open, three_d) {
        (true, false) => "Show Open in 2D",
        (false, false) => "Show Closed in 2D",
        (true, true) => "Show Open in 3D",
        (false, true) => "Show Closed in 3D",
    });
    for o in cx.project.floors[fl].openings.iter_mut() {
        if ids.contains(&o.id) {
            if three_d {
                o.extras.spec.show_open_in_3d = open;
            } else {
                o.extras.show_open_in_plan = open;
            }
        }
    }
    cx.mark_dirty();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::schedule_view;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn windows() -> (EditorContext, [Id; 2]) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            6.0,
            109.125,
            WallKind::Exterior,
        );
        let a = cx
            .project
            .add_opening(0, w, 100.0, OpeningKind::Window)
            .unwrap();
        let b = cx
            .project
            .add_opening(0, w, 142.0, OpeningKind::Window)
            .unwrap();
        (cx, [a, b])
    }

    fn span(cx: &EditorContext, id: Id) -> (f64, f64) {
        let o = cx.floor().openings.iter().find(|o| o.id == id).unwrap();
        (o.start_offset(), o.end_offset())
    }

    #[test]
    fn make_mulled_unit_and_explode_are_one_undo_step_each() {
        let (mut cx, [a, b]) = windows();
        // a: 82..118, b: 124..160 (6" apart).
        cx.selection.set(ObjectRef::Opening(a));
        // A lone window offers Make Mulled Unit with its neighbour.
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(
            labels.contains(&"Make Mulled Unit") && !labels.contains(&"Explode Mulled Unit"),
            "{labels:?}"
        );
        assert!(run_command(&mut cx, MULL));
        // Nothing moves: the 6" between them stays.
        assert_eq!(span(&cx, a), (82.0, 118.0));
        assert_eq!(span(&cx, b), (124.0, 160.0));
        assert_eq!(cx.undo_label(), Some("Make Mulled Unit"));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(
            labels.contains(&"Explode Mulled Unit") && !labels.contains(&"Make Mulled Unit"),
            "{labels:?}"
        );
        // The unit takes the Mulled Unit Defaults.
        let o = cx.floor().openings.iter().find(|o| o.id == a).unwrap();
        assert!(o.extras.spec.mulled.is_some());
        // Explode splits them; the windows keep their places.
        assert!(run_command(&mut cx, UNMULL));
        assert_eq!(cx.undo_label(), Some("Explode Mulled Unit"));
        assert!(cx.floor().openings.iter().all(|o| o.mull_group.is_none()));
        // Undo twice: back to separate windows.
        cx.undo();
        cx.undo();
        assert!(cx.floor().openings.iter().all(|o| o.mull_group.is_none()));
        assert_eq!(span(&cx, b), (124.0, 160.0));
        // Selecting both windows blocks them too.
        cx.selection.set(ObjectRef::Opening(a));
        cx.selection.toggle(ObjectRef::Opening(b));
        assert!(run_command(&mut cx, MULL));
        assert_eq!(span(&cx, b), (124.0, 160.0));
        assert!(cx.floor().openings.iter().all(|o| o.mull_group.is_some()));
    }

    #[test]
    fn make_mulled_unit_needs_openings_within_24_inches() {
        let (mut cx, [a, _]) = windows();
        let w = cx.floor().openings[0].wall_id;
        let far = cx
            .project
            .add_opening(0, w, 260.0, OpeningKind::Window)
            .unwrap();
        cx.selection.set(ObjectRef::Opening(far));
        // Nothing within reach: no button.
        assert!(edit_actions(&cx)
            .iter()
            .all(|e| e.label != "Make Mulled Unit"));
        cx.selection.set(ObjectRef::Opening(a));
        cx.selection.toggle(ObjectRef::Opening(far));
        assert!(run_command(&mut cx, MULL));
        assert!(cx.status.contains("within"), "{}", cx.status);
        assert!(!cx.can_undo());
        // A door with no opening within reach is not offered it.
        let d = cx
            .project
            .add_opening(0, w, 20.0, OpeningKind::Door)
            .unwrap();
        cx.selection.set(ObjectRef::Opening(d));
        assert!(edit_actions(&cx)
            .iter()
            .all(|e| e.label != "Make Mulled Unit"));
    }

    #[test]
    fn center_on_wall_segment_uses_the_free_span() {
        let (mut cx, [a, b]) = windows();
        cx.selection.set(ObjectRef::Opening(b));
        assert!(edit_actions(&cx)
            .iter()
            .any(|e| e.label == "Center on Wall Segment"));
        // b's free span is from a's end + 2" to the wall end - 2".
        assert!(run_command(&mut cx, CENTER_SEGMENT));
        let (s, e) = span(&cx, b);
        assert!(
            ((s + e) * 0.5 - (120.0 + 298.0) * 0.5).abs() < 1e-9,
            "{s} {e}"
        );
        assert_eq!(cx.undo_label(), Some("Center Object"));
        // Again: already centered, no undo step.
        let before = cx.undo_label().map(str::to_string);
        assert!(run_command(&mut cx, CENTER_SEGMENT));
        assert_eq!(cx.undo_label().map(str::to_string), before);
        let _ = a;
        assert!(!run_command(&mut cx, "other"));
    }

    #[test]
    fn a_door_blocks_with_the_sidelite_beside_it_into_a_door_unit() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            6.0,
            109.125,
            WallKind::Exterior,
        );
        let door = cx
            .project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        let side = cx
            .project
            .add_opening(0, w, 104.0, OpeningKind::Window)
            .unwrap();
        // door 42..78, sidelite 86..122: 8" apart.
        cx.selection.set(ObjectRef::Opening(door));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(labels.contains(&"Make Mulled Unit"), "{labels:?}");
        // From the window too.
        cx.selection.set(ObjectRef::Opening(side));
        assert!(edit_actions(&cx)
            .iter()
            .any(|e| e.label == "Make Mulled Unit"));
        assert!(run_command(&mut cx, MULL));
        assert_eq!(span(&cx, door), (42.0, 78.0));
        assert_eq!(span(&cx, side), (86.0, 122.0));
        assert_eq!(cx.undo_label(), Some("Make Mulled Unit"));
        let group = cx
            .floor()
            .openings
            .iter()
            .find(|o| o.id == door)
            .unwrap()
            .mull_group;
        assert!(group.is_some());
        assert_eq!(
            cx.floor()
                .openings
                .iter()
                .find(|o| o.id == side)
                .unwrap()
                .mull_group,
            group
        );
        // A door is a component, so the unit is treated as a door.
        let spec = cx
            .project
            .mulled_spec(0, side)
            .expect("the unit has a spec");
        assert!(spec.treat_as_door);
        // One unit: sliding moves both and they keep their 8".
        cx.selection.set(ObjectRef::Opening(door));
        assert!(cx.project.slide_opening(0, door, 100.0));
        assert_eq!(span(&cx, side).0 - span(&cx, door).1, 8.0);
    }

    #[test]
    fn the_depth_handle_of_a_bay_window_stands_at_its_front_and_drags_the_depth() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            6.0,
            109.125,
            WallKind::Exterior,
        );
        let id = cx
            .project
            .add_opening(0, w, 150.0, OpeningKind::Window)
            .unwrap();
        {
            let o = &mut cx.project.floors[0].openings[0];
            o.style = OpeningStyle::BayWindow;
            o.width = 50.0;
            o.extras.spec.bay = plan_core::openings::BayUnit::for_style(OpeningStyle::BayWindow);
        }
        cx.refresh();
        let h = bay_depth_handle(&cx, id).unwrap();
        assert!((h.x - 150.0).abs() < 1e-9);
        // 3" to the face and 12" out, on the exterior side of the wall.
        assert!((h.y.abs() - 15.0).abs() < 1e-9, "{h:?}");
        // Dragging it outward 8" deepens the unit, inward shallows it.
        let out = Point::new(150.0, h.y + h.y.signum() * 8.0);
        cx.begin_change("Depth");
        assert!(drag_bay_depth(&mut cx, id, out));
        let d = cx.floor().openings[0]
            .extras
            .spec
            .bay
            .depth_for(OpeningStyle::BayWindow);
        assert!((d - 20.0).abs() < 1e-9, "{d}");
        let back = Point::new(150.0, h.y.signum() * 3.0 + h.y.signum() * 6.0);
        assert!(drag_bay_depth(&mut cx, id, back));
        assert_eq!(
            cx.floor().openings[0]
                .extras
                .spec
                .bay
                .depth_for(OpeningStyle::BayWindow),
            6.0
        );
        // A plain window has no handle.
        assert!(bay_depth_handle(&cx, 999).is_none());
    }

    #[test]
    fn set_as_default_and_the_unit_commands_are_offered_and_undone_in_one_step() {
        let (mut cx, [a, b]) = windows();
        cx.selection.set(ObjectRef::Opening(a));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(labels.contains(&"Set as Default"), "{labels:?}");
        assert!(labels.contains(&"Show Closed in 2D"), "{labels:?}");
        assert!(labels.contains(&"Gable Over Door/Window"), "{labels:?}");
        // Show Closed in 2D clears the flag of the selection; one undo step.
        assert!(run_command(&mut cx, SHOW_CLOSED_2D));
        assert!(
            !cx.floor()
                .openings
                .iter()
                .find(|o| o.id == a)
                .unwrap()
                .extras
                .show_open_in_plan
        );
        assert_eq!(cx.undo_label(), Some("Show Closed in 2D"));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(labels.contains(&"Show Open in 2D"), "{labels:?}");
        // Show Open in 3D sets the flag of the opening.
        cx.selection.set(ObjectRef::Opening(b));
        assert!(run_command(&mut cx, SHOW_OPEN_3D));
        assert!(
            cx.floor()
                .openings
                .iter()
                .find(|o| o.id == b)
                .unwrap()
                .extras
                .spec
                .show_open_in_3d
        );
    }

    #[test]
    fn reset_label_position_only_shows_for_a_moved_label() {
        let (mut cx, [a, _]) = windows();
        cx.selection.set(ObjectRef::Opening(a));
        assert!(edit_actions(&cx)
            .iter()
            .all(|e| e.label != "Reset Label Position"));
        cx.project.floors[0].openings[0].extras.spec.label_offset = (5.0, -4.0);
        assert!(edit_actions(&cx)
            .iter()
            .any(|e| e.label == "Reset Label Position"));
        assert!(run_command(&mut cx, RESET_LABEL));
        assert_eq!(cx.floor().openings[0].extras.spec.label_offset, (0.0, 0.0));
        assert_eq!(cx.undo_label(), Some("Reset Label Position"));
        cx.undo();
        assert_eq!(cx.floor().openings[0].extras.spec.label_offset, (5.0, -4.0));
    }

    #[test]
    fn a_door_offers_a_transom_that_is_one_undo_step() {
        let (mut cx, _) = windows();
        let w = cx.floor().openings[0].wall_id;
        let door = cx
            .project
            .add_opening(0, w, 20.0, OpeningKind::Door)
            .unwrap();
        cx.selection.set(ObjectRef::Opening(door));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(labels.contains(&"Add Transom"), "{labels:?}");
        assert!(run_command(&mut cx, ADD_TRANSOM));
        assert_eq!(cx.undo_label(), Some("Add Transom"));
        // The new window stands over the door, mulled with it, and is the
        // selection; the door keeps its place.
        let ObjectRef::Opening(t) = cx.selection.items[0] else {
            panic!("the transom is selected");
        };
        let tr = cx.floor().openings.iter().find(|o| o.id == t).unwrap();
        let d = cx.floor().openings.iter().find(|o| o.id == door).unwrap();
        assert!(plan_core::openings::stands_over(tr, d));
        assert_eq!(tr.mull_group, d.mull_group);
        assert_eq!((tr.start_offset(), tr.end_offset()), (2.0, 38.0));
        // Only one transom: the button is gone from the door and the transom.
        for id in [door, t] {
            cx.selection.set(ObjectRef::Opening(id));
            assert!(edit_actions(&cx).iter().all(|e| e.label != "Add Transom"));
        }
        cx.undo();
        assert_eq!(cx.floor().openings.len(), 3);
    }

    #[test]
    fn the_3d_menu_toggles_are_kept_in_the_plan() {
        let (mut cx, _) = windows();
        assert!(cx.project.opening_display.casing && !cx.project.opening_display.doors_open);
        assert!(run_command(&mut cx, DOORS_OPEN));
        assert!(cx.project.opening_display.doors_open);
        assert!(cx.status.contains("open"), "{}", cx.status);
        assert!(run_command(&mut cx, CASING_3D));
        assert!(!cx.project.opening_display.casing);
        // Each is an undo step.
        cx.undo();
        assert!(cx.project.opening_display.casing);
        cx.undo();
        assert!(!cx.project.opening_display.doors_open);
    }

    #[test]
    fn the_editors_3d_view_builds_what_the_plan_asks_for() {
        use crate::shell::view3d_panel::{build_view_scene, ViewScope};
        use plan_3d::Material;
        let (mut cx, _) = windows();
        let w = cx.floor().openings[0].wall_id;
        let door = cx
            .project
            .add_opening(0, w, 20.0, OpeningKind::Door)
            .unwrap();
        let door_z = |cx: &EditorContext| {
            let scene = build_view_scene(&cx.project, &ViewScope::default());
            let count = |m: Material| {
                scene
                    .meshes
                    .iter()
                    .filter(|x| x.object_id == Some(door) && x.material == m)
                    .map(plan_3d::Mesh::triangle_count)
                    .sum::<usize>()
            };
            let reach = scene
                .meshes
                .iter()
                .filter(|x| x.object_id == Some(door) && x.material == Material::DoorPanel)
                .flat_map(|x| x.vertices.iter().map(|v| v.position[2].abs()))
                .fold(0.0_f32, f32::max);
            (count(Material::Trim), reach)
        };
        // A new plan shows the casing, the jambs and the threshold, doors closed.
        let (trim, reach) = door_z(&cx);
        assert!(trim >= 13 * 12, "{trim}");
        assert!(reach < 1.0);
        assert!(run_command(&mut cx, DOORS_OPEN));
        assert!(door_z(&cx).1 > 25.0, "the door stands open");
        assert!(run_command(&mut cx, CASING_3D));
        assert_eq!(door_z(&cx).0, 0);
    }
    // ----- Renumber Schedule (DW-61, L-26) -----

    /// Two floors: doors drawn right to left, so the draw order is the
    /// opposite of reading order.
    fn doors_drawn_right_to_left() -> (EditorContext, Vec<Id>) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(400.0, 0.0),
            6.0,
            109.125,
            WallKind::Exterior,
        );
        let ids = [300.0, 200.0, 100.0]
            .iter()
            .map(|c| cx.project.add_opening(0, w, *c, OpeningKind::Door).unwrap())
            .collect();
        (cx, ids)
    }

    fn mark(cx: &EditorContext, id: Id) -> Option<String> {
        cx.floor()
            .openings
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .schedule_number
            .clone()
    }

    /// The marks the door schedule gives the doors, by door id.
    fn schedule_marks(cx: &EditorContext, sid: Id) -> Vec<(Id, String)> {
        let d = schedule_view::find(cx, sid).unwrap();
        plan_docs::schedule_kinds::rows(&cx.project, &d, 0, None)
            .into_iter()
            .map(|e| (e.id, e.cell("mark").to_string()))
            .collect()
    }

    #[test]
    fn renumber_closes_the_gaps_of_the_schedule_as_one_undo_step() {
        let (mut cx, ids) = doors_drawn_right_to_left();
        let sid = schedule_view::add(&mut cx, ScheduleKind::Door, Point::new(0.0, -80.0));
        // A schedule starts with the doors in reading order: the last one
        // drawn, at the left, is D01.
        let m = schedule_marks(&cx, sid);
        assert_eq!(m[0], (ids[2], "D01".to_string()));
        // A door goes: its number stays free until Renumber Schedule.
        cx.project.floors[0].openings.retain(|o| o.id != ids[2]);
        let m = schedule_marks(&cx, sid);
        assert_eq!(
            m,
            [(ids[1], "D02".to_string()), (ids[0], "D03".to_string())]
        );
        // The Edit toolbar of a selected door offers the command.
        cx.selection.set(ObjectRef::Opening(ids[0]));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(labels.contains(&"Renumber Schedule"), "{labels:?}");
        assert!(run_command(&mut cx, RENUMBER));
        let m = schedule_marks(&cx, sid);
        assert_eq!(
            m,
            [(ids[1], "D01".to_string()), (ids[0], "D02".to_string())]
        );
        assert_eq!(cx.undo_label(), Some("Renumber Schedule"));
        // No mark is written into the openings (DECISIONS 44).
        assert_eq!(mark(&cx, ids[0]), None);
        // Nothing to change the second time: no extra undo step.
        assert!(run_command(&mut cx, RENUMBER_DOORS));
        assert_eq!(cx.undo().as_deref(), Some("Renumber Schedule"));
        assert_eq!(schedule_marks(&cx, sid)[0].1, "D02");
    }

    #[test]
    #[ignore = "R16-04 in progress: numbering order change"]
    fn renumber_keeps_to_one_kind_and_skips_what_the_schedule_leaves_out() {
        let (mut cx, ids) = doors_drawn_right_to_left();
        let w = cx.floor().walls[0].id;
        // The middle door is left out of the schedule.
        cx.project.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == ids[1])
            .unwrap()
            .extras
            .spec
            .schedule
            .include = false;
        let door_s = schedule_view::add(&mut cx, ScheduleKind::Door, Point::new(0.0, -80.0));
        let win_s = schedule_view::add(&mut cx, ScheduleKind::Window, Point::new(0.0, -200.0));
        let win = cx
            .project
            .add_opening(0, w, 30.0, OpeningKind::Window)
            .unwrap();
        let other = cx
            .project
            .add_opening(0, w, 370.0, OpeningKind::Window)
            .unwrap();
        // Doors: the left-out door has no number. A window goes: a gap.
        let m = schedule_marks(&cx, door_s);
        assert_eq!(m.len(), 2);
        assert!(m.iter().all(|(id, _)| *id != ids[1]));
        cx.project.floors[0].openings.retain(|o| o.id != win);
        let before = schedule_marks(&cx, win_s);
        assert_eq!(before, [(other, "W02".to_string())]);
        // Windows are untouched until their own command runs.
        assert!(run_command(&mut cx, RENUMBER_DOORS));
        assert_eq!(schedule_marks(&cx, win_s), before);
        assert!(run_command(&mut cx, RENUMBER_WINDOWS));
        assert_eq!(schedule_marks(&cx, win_s), [(other, "W01".to_string())]);
    }

    #[test]
    fn renumber_by_floor_restarts_and_whole_counts_on() {
        let (mut cx, ids) = doors_drawn_right_to_left();
        let up = cx.project.insert_floor_above(0).unwrap();
        let w2 = cx.project.add_wall(
            up,
            Point::new(0.0, 0.0),
            Point::new(400.0, 0.0),
            6.0,
            109.125,
            WallKind::Exterior,
        );
        let upper = cx
            .project
            .add_opening(up, w2, 100.0, OpeningKind::Door)
            .unwrap();
        renumber_marks(&mut cx.project, OpeningKind::Door, "D", Numbering::ByFloor);
        let up_mark = |cx: &EditorContext| {
            cx.project.floors[up]
                .openings
                .iter()
                .find(|o| o.id == upper)
                .unwrap()
                .schedule_number
                .clone()
        };
        assert_eq!(up_mark(&cx).as_deref(), Some("D01"));
        assert_eq!(mark(&cx, ids[2]).as_deref(), Some("D03"));
        renumber_marks(&mut cx.project, OpeningKind::Door, "DR", Numbering::Whole);
        assert_eq!(up_mark(&cx).as_deref(), Some("DR04"));
    }
}
