//! Door and window Edit toolbar commands: Center on Wall Segment (DW-24),
//! Mull and Unmull (DW-51, DW-52: a door mulls with the windows beside it),
//! Reset Label Position (DW-63), and the standard widths a jamb handle snaps
//! to (DW-27), and Renumber Schedule (DW-61, L-26: the marks of the doors or
//! the windows set in the order they were drawn). The buttons come from `EditorContext::extra_edit_actions` and
//! run through `run_custom`.

use super::selection::ObjectRef;
use super::{EditAction, EditActionKind, EditorContext};
use crate::dialogs::OpeningTarget;
use plan_core::openings::{StandardWidths, MULL_DOOR_STYLES, MULL_MAX_GAP};
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

/// Whether `o` can be part of a mulled unit: a window, or a door with a swing
/// or a cased opening (its sidelites are the windows beside it).
fn mullable(o: &plan_core::Opening) -> bool {
    match o.kind {
        OpeningKind::Window => true,
        OpeningKind::Door => MULL_DOOR_STYLES.contains(&o.style),
    }
}

/// The window beside `id` it would mull with: the nearest window on the same
/// wall within [`MULL_MAX_GAP`] with nothing in between, not yet in its unit.
fn mull_partner(cx: &EditorContext, id: Id) -> Option<Id> {
    let f = cx.floor();
    let me = f.openings.iter().find(|o| o.id == id)?;
    if !mullable(me) {
        return None;
    }
    let unit_has_door = cx.project.mull_members(cx.floor, id).iter().any(|m| {
        f.openings
            .iter()
            .any(|o| o.id == *m && o.kind == OpeningKind::Door)
    });
    let unit = cx.project.mull_members(cx.floor, id);
    let (lo, hi) = cx.project.unit_span(cx.floor, id)?;
    let mut best: Option<(f64, Id)> = None;
    for o in f.openings_on(me.wall_id) {
        // A door takes windows beside it; a window takes a window, or the
        // door beside it when the unit has none yet.
        let partner_ok = o.kind == OpeningKind::Window
            || (!unit_has_door && me.kind == OpeningKind::Window && mullable(o));
        if unit.contains(&o.id) || !partner_ok {
            continue;
        }
        let gap = if o.start_offset() >= hi - 1e-9 {
            o.start_offset() - hi
        } else if o.end_offset() <= lo + 1e-9 {
            lo - o.end_offset()
        } else {
            continue;
        };
        if gap <= MULL_MAX_GAP + 1e-9 && best.is_none_or(|(g, _)| gap < g) {
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

/// The openings a Mull would join: the selected windows (and a door among
/// them), or a lone selected window or door and its nearest neighbour.
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
        v.push(button(MULL, "Mull", "", true));
    }
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
        v.push(button(UNMULL, "Unmull", "", true));
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
        _ => return false,
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

/// Renumber Schedule (DW-61, L-26) for the doors and/or windows of the plan,
/// using the prefix and numbering of the schedule on the floor that shows
/// that kind's callouts (else `D` / `W`, by floor). One undo step.
pub fn renumber(cx: &mut EditorContext, kinds: &[OpeningKind]) {
    let layer = super::schedule_view::load(cx);
    cx.begin_change("Renumber Schedule");
    let mut changed = 0;
    for &kind in kinds {
        let sk = schedule_kind(kind);
        let (prefix, numbering) = layer
            .schedules
            .iter()
            .find(|s| s.kind == sk)
            .map(|s| (s.label_prefix.clone(), s.numbering))
            .unwrap_or_else(|| (sk.default_prefix().to_string(), Numbering::ByFloor));
        changed += renumber_marks(&mut cx.project, kind, &prefix, numbering);
    }
    if changed == 0 {
        cx.cancel_change();
        cx.status = "The marks already follow the order they were drawn in".into();
    } else {
        cx.mark_dirty();
        cx.status = format!("Renumbered {changed} marks in the order they were drawn");
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

/// Mull the selected windows into one unit (DW-51).
pub fn mull(cx: &mut EditorContext) {
    let ids = mull_set(cx);
    if ids.len() < 2 {
        cx.status = "Select two adjacent windows, or a door and the window beside it".into();
        return;
    }
    if locked(cx, &ids) {
        return;
    }
    cx.begin_change("Mull Windows");
    let fl = cx.floor;
    match cx.project.mull_openings(fl, &ids) {
        Ok(_) => {
            cx.mark_dirty();
            cx.status = format!("Mulled {} openings", ids.len());
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

/// Split the mulled unit of the selected window (DW-51).
pub fn unmull(cx: &mut EditorContext) {
    let sel = selected_openings(cx);
    if locked(cx, &sel) {
        return;
    }
    cx.begin_change("Unmull Windows");
    let fl = cx.floor;
    let n: usize = sel
        .iter()
        .map(|id| cx.project.unmull_openings(fl, *id))
        .sum();
    if n == 0 {
        cx.cancel_change();
        cx.status = "That window is not mulled".into();
    } else {
        cx.mark_dirty();
        cx.status = format!("Unmulled {n} windows");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn mull_and_unmull_are_one_undo_step_each() {
        let (mut cx, [a, b]) = windows();
        // a: 82..118, b: 124..160 (6" apart).
        cx.selection.set(ObjectRef::Opening(a));
        // A lone window offers Mull with its neighbour.
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(
            labels.contains(&"Mull") && !labels.contains(&"Unmull"),
            "{labels:?}"
        );
        assert!(run_command(&mut cx, MULL));
        assert_eq!(span(&cx, a), (82.0, 118.0));
        assert_eq!(span(&cx, b), (118.0, 154.0));
        assert_eq!(cx.undo_label(), Some("Mull Windows"));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(
            labels.contains(&"Unmull") && !labels.contains(&"Mull"),
            "{labels:?}"
        );
        // Unmull splits them; the windows keep their places.
        assert!(run_command(&mut cx, UNMULL));
        assert_eq!(cx.undo_label(), Some("Unmull Windows"));
        assert!(cx.floor().openings.iter().all(|o| o.mull_group.is_none()));
        // Undo twice: back to the 6" gap.
        cx.undo();
        cx.undo();
        assert_eq!(span(&cx, b), (124.0, 160.0));
        // Selecting both windows mulls them too.
        cx.selection.set(ObjectRef::Opening(a));
        cx.selection.toggle(ObjectRef::Opening(b));
        assert!(run_command(&mut cx, MULL));
        assert_eq!(span(&cx, b), (118.0, 154.0));
    }

    #[test]
    fn mull_needs_adjacent_windows() {
        let (mut cx, [a, _]) = windows();
        let w = cx.floor().openings[0].wall_id;
        let far = cx
            .project
            .add_opening(0, w, 260.0, OpeningKind::Window)
            .unwrap();
        cx.selection.set(ObjectRef::Opening(far));
        // Nothing within reach: no Mull button.
        assert!(edit_actions(&cx).iter().all(|e| e.label != "Mull"));
        cx.selection.set(ObjectRef::Opening(a));
        cx.selection.toggle(ObjectRef::Opening(far));
        assert!(run_command(&mut cx, MULL));
        assert!(
            cx.status.contains("in between") || cx.status.contains("too far"),
            "{}",
            cx.status
        );
        assert!(!cx.can_undo());
        // A door with no window within reach is not offered Mull.
        let d = cx
            .project
            .add_opening(0, w, 20.0, OpeningKind::Door)
            .unwrap();
        cx.selection.set(ObjectRef::Opening(d));
        assert!(edit_actions(&cx).iter().all(|e| e.label != "Mull"));
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
    fn a_door_mulls_with_the_sidelite_beside_it_and_not_with_a_second_door() {
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
        let door2 = cx
            .project
            .add_opening(0, w, 200.0, OpeningKind::Door)
            .unwrap();
        // door 42..78, sidelite 86..122: 8" apart.
        cx.selection.set(ObjectRef::Opening(door));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(labels.contains(&"Mull"), "{labels:?}");
        // From the window too.
        cx.selection.set(ObjectRef::Opening(side));
        assert!(edit_actions(&cx).iter().any(|e| e.label == "Mull"));
        assert!(run_command(&mut cx, MULL));
        assert_eq!(span(&cx, door), (42.0, 78.0));
        assert_eq!(span(&cx, side), (78.0, 114.0));
        assert_eq!(cx.undo_label(), Some("Mull Windows"));
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
        // One unit: sliding moves both.
        cx.selection.set(ObjectRef::Opening(door));
        assert!(cx.project.slide_opening(0, door, 100.0));
        assert_eq!(span(&cx, side).0, span(&cx, door).1);
        // A second door cannot join the unit.
        cx.selection.set(ObjectRef::Opening(door));
        cx.selection.toggle(ObjectRef::Opening(door2));
        cx.selection.toggle(ObjectRef::Opening(side));
        cx.status.clear();
        cx.undo();
        assert!(run_command(&mut cx, MULL));
        assert!(
            cx.status.contains("Only one door") || cx.status.contains("in between"),
            "{}",
            cx.status
        );
        // A garage door is not mullable at all.
        let g = cx
            .project
            .add_opening(0, w, 270.0, OpeningKind::Door)
            .unwrap();
        cx.project.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == g)
            .unwrap()
            .style = OpeningStyle::Garage;
        cx.selection.set(ObjectRef::Opening(g));
        assert!(edit_actions(&cx).iter().all(|e| e.label != "Mull"));
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

    #[test]
    fn renumber_sets_the_marks_in_draw_order_as_one_undo_step() {
        let (mut cx, ids) = doors_drawn_right_to_left();
        // The Edit toolbar of a selected door offers it.
        cx.selection.set(ObjectRef::Opening(ids[0]));
        let labels: Vec<_> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(labels.contains(&"Renumber Schedule"), "{labels:?}");
        assert!(run_command(&mut cx, RENUMBER));
        // Draw order, not reading order: the first door drawn is D01.
        assert_eq!(mark(&cx, ids[0]).as_deref(), Some("D01"));
        assert_eq!(mark(&cx, ids[1]).as_deref(), Some("D02"));
        assert_eq!(mark(&cx, ids[2]).as_deref(), Some("D03"));
        assert_eq!(cx.undo_label(), Some("Renumber Schedule"));
        // Nothing to change the second time: no extra undo step.
        assert!(run_command(&mut cx, RENUMBER_DOORS));
        assert_eq!(cx.undo().as_deref(), Some("Renumber Schedule"));
        assert_eq!(mark(&cx, ids[0]), None);
        assert!(!cx.can_undo());
    }

    #[test]
    fn renumber_keeps_to_one_kind_and_skips_what_the_schedule_leaves_out() {
        let (mut cx, ids) = doors_drawn_right_to_left();
        let w = cx.floor().walls[0].id;
        let win = cx
            .project
            .add_opening(0, w, 30.0, OpeningKind::Window)
            .unwrap();
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
        assert!(run_command(&mut cx, RENUMBER_DOORS));
        assert_eq!(mark(&cx, ids[0]).as_deref(), Some("D01"));
        assert_eq!(mark(&cx, ids[1]), None);
        assert_eq!(mark(&cx, ids[2]).as_deref(), Some("D02"));
        // Windows are untouched until their own command runs.
        assert_eq!(mark(&cx, win), None);
        assert!(run_command(&mut cx, RENUMBER_WINDOWS));
        assert_eq!(mark(&cx, win).as_deref(), Some("W01"));
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
