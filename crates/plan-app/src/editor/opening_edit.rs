//! Door and window Edit toolbar commands: Center on Wall Segment (DW-24),
//! Mull and Unmull (DW-51, DW-52: a door mulls with the windows beside it),
//! Reset Label Position (DW-63), and the standard widths a jamb handle snaps
//! to (DW-27). The buttons come from `EditorContext::extra_edit_actions` and
//! run through `run_custom`.

use super::selection::ObjectRef;
use super::{EditAction, EditActionKind, EditorContext};
use crate::dialogs::OpeningTarget;
use plan_core::openings::{StandardWidths, MULL_DOOR_STYLES, MULL_MAX_GAP};
use plan_core::{Id, OpeningKind, OpeningStyle};

/// Custom command ids (the `id` of `EditActionKind::Custom`).
pub const CENTER_SEGMENT: &str = "opening.center_segment";
pub const MULL: &str = "opening.mull";
pub const UNMULL: &str = "opening.unmull";
pub const REVERSE_SIDE: &str = "opening.reverse_side";
pub const RESET_LABEL: &str = "opening.reset_label";
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
        _ => return false,
    }
    true
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
}
