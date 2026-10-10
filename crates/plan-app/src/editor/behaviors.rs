//! Edit Behaviors (S-65): what a body or handle drag of the Select tool does
//! under each mode of Edit > Edit Behaviors (`PlanDefaults::editing.behavior`).
//!
//! * Default: move and reshape in place (the Select tool's own code);
//! * Resize: a body drag scales the selected CAD objects from the corner of
//!   their bounds opposite the one nearest the press;
//! * Concentric: a body drag of a circle, arc, line or polyline leaves it and
//!   adds offset copies (the drag sets the distance, or the dialog does);
//! * Fillet: dragging a polyline corner handle rounds that corner;
//! * Chamfer: dragging a polyline corner cuts it off with a chamfer;
//! * Alternate: a body drag moves along the dominant axis only;
//! * Replicate: a body drag leaves the originals and places copies, each one
//!   more drag delta along (or, with `replicate_dialog`, releasing the drag
//!   opens Transform/Replicate Object with the drag as its Move).
//!
//! The Select tool calls [`apply_group`] and [`apply_vertex`] before its own
//! drag code, with the project already reset to the drag's starting state, so
//! every call rebuilds the result from scratch.

use super::ops::translate_cad;
use super::selection::ObjectRef;
use super::EditorContext;
use plan_core::cad::{self, CadItem};
use plan_core::defaults::EditBehavior;
use plan_core::geometry::{project_on_segment, Point};
use plan_core::Id;
use std::cell::Cell;

thread_local! {
    /// The Replicate drag waiting to be handed to the Transform dialog on
    /// release: the drag delta and the copy count.
    static HANDOFF: Cell<Option<(Point, u32)>> = const { Cell::new(None) };
}

/// Smallest scale a Resize drag may reach.
const MIN_SCALE: f64 = 0.05;
/// Largest sampling step of a fillet arc, degrees.
const FILLET_STEP_DEG: f64 = 7.5;
/// Copies one drag may leave.
const MAX_COPIES: u32 = 50;

pub fn mode(cx: &EditorContext) -> EditBehavior {
    cx.defaults.editing.behavior.mode
}

/// The status-bar indicator of the active behavior ("Edit Behavior: Resize"),
/// `None` under Default (S-66).
pub fn indicator(cx: &EditorContext) -> Option<String> {
    match mode(cx) {
        EditBehavior::Default => None,
        m => Some(format!("Edit Behavior: {}", m.label())),
    }
}

/// Back to the Default behavior, as Chief does when the tool changes from
/// Select Objects (S-66). True when something was reset.
pub fn reset(cx: &mut EditorContext) -> bool {
    if mode(cx) == EditBehavior::Default {
        return false;
    }
    cx.defaults.editing.behavior.mode = EditBehavior::Default;
    HANDOFF.with(|h| h.set(None));
    cx.status = "Edit Behavior reset to Default".into();
    true
}

/// The undo label of a body drag of the selection.
pub fn group_label(cx: &EditorContext) -> &'static str {
    match mode(cx) {
        EditBehavior::Resize => "Resize Objects",
        EditBehavior::Concentric => "Concentric Copy",
        EditBehavior::Replicate => "Replicate Objects",
        _ => "Move Objects",
    }
}

fn round_to(v: f64, unit: f64) -> f64 {
    if unit > 0.0 {
        (v / unit).round() * unit
    } else {
        v
    }
}

/// The delta of a body drag: Alternate keeps only the dominant axis.
pub fn group_delta(cx: &EditorContext, total: Point) -> Point {
    let b = &cx.defaults.editing.behavior;
    if b.mode == EditBehavior::Alternate && b.alternate_lock_axis {
        if total.x.abs() >= total.y.abs() {
            Point::new(total.x, 0.0)
        } else {
            Point::new(0.0, total.y)
        }
    } else {
        total
    }
}

fn cad_ids(items: &[ObjectRef]) -> Vec<Id> {
    items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) | ObjectRef::Text(id) => Some(*id),
            _ => None,
        })
        .collect()
}

/// `item` scaled about `anchor` by `sx` and `sy` (circles, arcs and text take
/// the geometric mean so they stay round).
pub fn scale_item(item: &mut CadItem, anchor: Point, sx: f64, sy: f64) {
    let at = |p: Point| {
        Point::new(
            anchor.x + (p.x - anchor.x) * sx,
            anchor.y + (p.y - anchor.y) * sy,
        )
    };
    let uniform = (sx * sy).abs().sqrt();
    match item {
        CadItem::Line { a, b } => {
            *a = at(*a);
            *b = at(*b);
        }
        CadItem::Circle { center, radius } | CadItem::Arc { center, radius, .. } => {
            *center = at(*center);
            *radius *= uniform;
        }
        CadItem::Polyline { points, .. } => points.iter_mut().for_each(|p| *p = at(*p)),
        CadItem::Text { pos, height, .. } => {
            *pos = at(*pos);
            *height *= uniform;
        }
    }
}

/// Signed distance from `p` to the polyline (positive on the left of its
/// direction of travel) and the distance's absolute value.
fn signed_distance(points: &[Point], closed: bool, p: Point) -> f64 {
    let mut best: Option<f64> = None;
    let n = points.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    for i in 0..segs {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let d = project_on_segment(p, a, b).1.dist(p);
        let side = if b.sub(a).cross(p.sub(a)) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        if best.is_none_or(|v| d < v.abs()) {
            best = Some(side * d);
        }
    }
    best.unwrap_or(0.0)
}

/// A body drag of the selection under Resize, Concentric or Replicate. True
/// when the mode did the work (the caller then skips its move).
pub fn apply_group(
    cx: &mut EditorContext,
    items: &[ObjectRef],
    start: Point,
    world: Point,
    shift: bool,
) -> bool {
    match mode(cx) {
        EditBehavior::Resize => resize(cx, items, start, world, shift),
        EditBehavior::Concentric => concentric(cx, items, world),
        EditBehavior::Replicate => replicate(cx, items, start, world),
        _ => false,
    }
}

fn resize(
    cx: &mut EditorContext,
    items: &[ObjectRef],
    start: Point,
    world: Point,
    shift: bool,
) -> bool {
    let ids = cad_ids(items);
    if ids.is_empty() || ids.len() != items.len() {
        return false;
    }
    let fl = cx.floor;
    let mut lo = Point::new(f64::MAX, f64::MAX);
    let mut hi = Point::new(f64::MIN, f64::MIN);
    for c in cx.project.floors[fl]
        .cad
        .iter()
        .filter(|c| ids.contains(&c.id))
    {
        let (a, b) = c.bounds();
        lo = Point::new(lo.x.min(a.x), lo.y.min(a.y));
        hi = Point::new(hi.x.max(b.x), hi.y.max(b.y));
    }
    if lo.x > hi.x {
        return false;
    }
    // The corner nearest the press is dragged; the opposite one stays.
    let corner = Point::new(
        if (start.x - lo.x).abs() <= (start.x - hi.x).abs() {
            lo.x
        } else {
            hi.x
        },
        if (start.y - lo.y).abs() <= (start.y - hi.y).abs() {
            lo.y
        } else {
            hi.y
        },
    );
    let anchor = Point::new(lo.x + hi.x - corner.x, lo.y + hi.y - corner.y);
    let unit = cx.snap_unit();
    let delta = Point::new(
        round_to(world.x - start.x, unit),
        round_to(world.y - start.y, unit),
    );
    let factor = |from: f64, anchor: f64, by: f64| {
        let span = from - anchor;
        if span.abs() < 1e-6 {
            1.0
        } else {
            ((from + by - anchor) / span).max(MIN_SCALE)
        }
    };
    let (mut sx, mut sy) = (
        factor(corner.x, anchor.x, delta.x),
        factor(corner.y, anchor.y, delta.y),
    );
    if shift || cx.defaults.editing.behavior.resize_proportional {
        let s = if (sx - 1.0).abs() >= (sy - 1.0).abs() {
            sx
        } else {
            sy
        };
        (sx, sy) = (s, s);
    }
    for c in cx.project.floors[fl]
        .cad
        .iter_mut()
        .filter(|c| ids.contains(&c.id))
    {
        scale_item(&mut c.item, anchor, sx, sy);
    }
    cx.mark_dirty();
    true
}

fn concentric(cx: &mut EditorContext, items: &[ObjectRef], world: Point) -> bool {
    let ids = cad_ids(items);
    let [id] = ids[..] else { return false };
    let fl = cx.floor;
    let Some(src) = cx.project.floors[fl]
        .cad
        .iter()
        .find(|c| c.id == id)
        .cloned()
    else {
        return false;
    };
    let b = cx.defaults.editing.behavior.clone();
    let copies = b.concentric_copies.clamp(1, MAX_COPIES);
    let unit = cx.snap_unit();
    // The signed step between copies: left of the line's direction, or outside
    // a round, is positive.
    let mut made = Vec::new();
    match &src.item {
        CadItem::Circle { center, radius } | CadItem::Arc { center, radius, .. } => {
            let drag = round_to(center.dist(world), unit) - radius;
            let step = if b.concentric_distance > 0.0 {
                b.concentric_distance * drag.signum()
            } else {
                drag
            };
            for k in 1..=copies {
                let r = radius + step * f64::from(k);
                if r < 0.5 {
                    break;
                }
                let mut item = src.item.clone();
                if let CadItem::Circle { radius, .. } | CadItem::Arc { radius, .. } = &mut item {
                    *radius = r;
                }
                made.push(item);
            }
        }
        CadItem::Line { a, b: end } => {
            let side = if end.sub(*a).cross(world.sub(*a)) >= 0.0 {
                1.0
            } else {
                -1.0
            };
            let dist = round_to(project_on_segment(world, *a, *end).1.dist(world), unit);
            let step = side
                * if b.concentric_distance > 0.0 {
                    b.concentric_distance
                } else {
                    dist
                };
            for k in 1..=copies {
                let (p, q) = cad::offset_segment(*a, *end, step * f64::from(k));
                made.push(CadItem::Line { a: p, b: q });
            }
        }
        CadItem::Polyline { points, closed } => {
            let d = signed_distance(points, *closed, world);
            let step = if b.concentric_distance > 0.0 {
                b.concentric_distance * d.signum()
            } else {
                round_to(d, unit)
            };
            if step.abs() < 1e-6 {
                return true;
            }
            for k in 1..=copies {
                let pts = cad::offset_polyline(points, *closed, step * f64::from(k));
                if pts.len() >= 2 {
                    made.push(CadItem::Polyline {
                        points: pts,
                        closed: *closed,
                    });
                }
            }
        }
        CadItem::Text { .. } => return false,
    }
    for item in made {
        cx.project.add_cad(fl, src.layer.clone(), item);
    }
    cx.mark_dirty();
    true
}

fn replicate(cx: &mut EditorContext, items: &[ObjectRef], start: Point, world: Point) -> bool {
    let unit = cx.snap_unit();
    let delta = Point::new(
        round_to(world.x - start.x, unit),
        round_to(world.y - start.y, unit),
    );
    let copies = cx
        .defaults
        .editing
        .behavior
        .replicate_copies
        .clamp(1, MAX_COPIES);
    if cx.defaults.editing.behavior.replicate_dialog {
        // The drag only measures the move; the dialog places the copies.
        HANDOFF.with(|h| h.set(Some((delta, copies))));
        cx.status = format!(
            "Release to replicate {} x {}",
            plan_core::units::fmt_ft_in(delta.x),
            plan_core::units::fmt_ft_in(delta.y)
        );
        return true;
    }
    let fl = cx.floor;
    let mut any = false;
    for o in items {
        match *o {
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                let Some(src) = cx.project.floors[fl]
                    .cad
                    .iter()
                    .find(|c| c.id == id)
                    .cloned()
                else {
                    continue;
                };
                for k in 1..=copies {
                    let mut item = src.item.clone();
                    translate_cad(&mut item, delta * f64::from(k));
                    cx.project.add_cad(fl, src.layer.clone(), item);
                    any = true;
                }
            }
            ObjectRef::Wall(id) => {
                let Some(src) = cx.project.floors[fl].wall(id).cloned() else {
                    continue;
                };
                for k in 1..=copies {
                    let mut w = src.clone();
                    w.id = cx.project.alloc_id();
                    w.start = w.start + delta * f64::from(k);
                    w.end = w.end + delta * f64::from(k);
                    cx.project.floors[fl].walls.push(w);
                    any = true;
                }
            }
            _ => {}
        }
    }
    if any {
        cx.mark_dirty();
    }
    // Objects of other kinds have no copy form: they stay where they were.
    true
}

/// Called when a body drag of the selection is released: a Replicate drag
/// with the dialog hand-off on opens Transform/Replicate Object with the
/// drag as its Move. True when the dialog opened.
pub fn finish_group(cx: &mut EditorContext) -> bool {
    let Some((delta, copies)) = HANDOFF.with(Cell::take) else {
        return false;
    };
    if delta.length() < 1e-6 || mode(cx) != EditBehavior::Replicate {
        return false;
    }
    crate::dialogs::transform::open_replicate(cx, delta, copies);
    true
}

/// Dragging a polyline corner handle under Fillet or Chamfer rounds or cuts
/// the corner instead of moving it. True when the mode took the drag.
pub fn apply_vertex(
    cx: &mut EditorContext,
    id: Id,
    kind: super::handles::HandleKind,
    world: Point,
) -> bool {
    let chamfer = match mode(cx) {
        EditBehavior::Fillet => false,
        EditBehavior::Chamfer => true,
        _ => return false,
    };
    let super::handles::HandleKind::Reshape(i) = kind else {
        return false;
    };
    let fl = cx.floor;
    let unit = cx.snap_unit();
    let fixed = if chamfer {
        cx.defaults.editing.behavior.chamfer_distance
    } else {
        cx.defaults.editing.behavior.fillet_radius
    };
    let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) else {
        return false;
    };
    let CadItem::Polyline { points, closed } = &mut c.item else {
        return false;
    };
    let Some(corner) = points.get(i).copied() else {
        return false;
    };
    let radius = if fixed > 0.0 {
        fixed
    } else {
        round_to(corner.dist(world), unit)
    };
    if radius < 0.5 {
        return true;
    }
    let cut = if chamfer {
        cad::chamfer_polyline_vertex(points, *closed, i, radius, radius)
    } else {
        cad::fillet_polyline_vertex(points, *closed, i, radius, FILLET_STEP_DEG)
    };
    if let Some(cut) = cut {
        *points = cut;
        cx.mark_dirty();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::tools::select::SelectTool;
    use crate::tools::{PointerEvent, Tool};
    use plan_core::WallKind;

    fn square(cx: &mut EditorContext, at: Point, size: f64) -> Id {
        cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Polyline {
                points: vec![
                    at,
                    at + Point::new(size, 0.0),
                    at + Point::new(size, size),
                    at + Point::new(0.0, size),
                ],
                closed: true,
            },
        )
    }

    fn cx_with(mode: EditBehavior) -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.defaults.editing.behavior.mode = mode;
        cx
    }

    fn bounds(cx: &EditorContext, id: Id) -> (Point, Point) {
        cx.floor().cad.iter().find(|c| c.id == id).unwrap().bounds()
    }

    /// Presses at `from`, drags to `to` and releases, as the shell would.
    fn drag(cx: &mut EditorContext, from: Point, to: Point, shift: bool) {
        let mut tool = SelectTool::default();
        let m = eframe::egui::Modifiers {
            shift,
            ..eframe::egui::Modifiers::NONE
        };
        // Shift pressed on a selected object would toggle it: hold it after.
        let down = PointerEvent::at(cx, from).with_down(true);
        tool.pointer_down(cx, down);
        for t in [0.25, 0.5, 1.0] {
            let p = PointerEvent::at(cx, Point::lerp(from, to, t))
                .with_modifiers(m)
                .with_down(true);
            tool.pointer_move(cx, p);
        }
        tool.pointer_up(cx, PointerEvent::at(cx, to).with_modifiers(m));
    }

    #[test]
    fn resize_scales_a_cad_box_from_the_opposite_corner() {
        let mut cx = cx_with(EditBehavior::Resize);
        let id = square(&mut cx, Point::new(100.0, 100.0), 100.0);
        cx.selection.set(ObjectRef::Cad(id));
        // Press on the top edge, nearer its right end: the bottom-left corner
        // stays.
        drag(
            &mut cx,
            Point::new(170.0, 200.0),
            Point::new(270.0, 150.0),
            false,
        );
        let (lo, hi) = bounds(&cx, id);
        assert_eq!(lo, Point::new(100.0, 100.0));
        assert!(
            (hi.x - 300.0).abs() < 1e-6 && (hi.y - 150.0).abs() < 1e-6,
            "{hi:?}"
        );
        // The drag was one undo step labelled for the mode.
        assert_eq!(cx.undo().as_deref(), Some("Resize Objects"));
        assert_eq!(bounds(&cx, id).1, Point::new(200.0, 200.0));
    }

    #[test]
    fn resize_with_shift_or_the_setting_keeps_the_proportions() {
        let mut cx = cx_with(EditBehavior::Resize);
        let id = square(&mut cx, Point::new(0.0, 0.0), 100.0);
        cx.selection.set(ObjectRef::Cad(id));
        drag(
            &mut cx,
            Point::new(80.0, 100.0),
            Point::new(180.0, 140.0),
            true,
        );
        let (_, hi) = bounds(&cx, id);
        assert!((hi.x - hi.y).abs() < 1e-6 && hi.x > 150.0, "{hi:?}");
        cx.undo();
        cx.defaults.editing.behavior.resize_proportional = true;
        drag(
            &mut cx,
            Point::new(80.0, 100.0),
            Point::new(30.0, 80.0),
            false,
        );
        let (_, hi) = bounds(&cx, id);
        assert!((hi.x - hi.y).abs() < 1e-6 && hi.x < 60.0, "{hi:?}");
    }

    #[test]
    fn default_mode_still_moves() {
        let mut cx = cx_with(EditBehavior::Default);
        let id = square(&mut cx, Point::new(100.0, 100.0), 100.0);
        cx.selection.set(ObjectRef::Cad(id));
        drag(
            &mut cx,
            Point::new(150.0, 100.0),
            Point::new(170.0, 130.0),
            false,
        );
        assert_eq!(bounds(&cx, id).0, Point::new(120.0, 130.0));
        assert_eq!(cx.undo().as_deref(), Some("Move Objects"));
    }

    #[test]
    fn alternate_moves_along_the_dominant_axis() {
        let mut cx = cx_with(EditBehavior::Alternate);
        let id = square(&mut cx, Point::new(100.0, 100.0), 100.0);
        cx.selection.set(ObjectRef::Cad(id));
        drag(
            &mut cx,
            Point::new(150.0, 100.0),
            Point::new(190.0, 120.0),
            false,
        );
        assert_eq!(bounds(&cx, id).0, Point::new(140.0, 100.0));
        cx.defaults.editing.behavior.alternate_lock_axis = false;
        cx.undo();
        // The temporary dimensions of the first drag would take the press.
        cx.temp.clear();
        drag(
            &mut cx,
            Point::new(150.0, 100.0),
            Point::new(190.0, 120.0),
            false,
        );
        assert_eq!(bounds(&cx, id).0, Point::new(140.0, 120.0));
    }

    #[test]
    fn concentric_leaves_the_original_and_adds_offset_copies() {
        let mut cx = cx_with(EditBehavior::Concentric);
        let id = square(&mut cx, Point::new(100.0, 100.0), 100.0);
        cx.selection.set(ObjectRef::Cad(id));
        cx.defaults.editing.behavior.concentric_copies = 2;
        // Drag from the bottom edge to 12" outside it.
        drag(
            &mut cx,
            Point::new(150.0, 100.0),
            Point::new(150.0, 88.0),
            false,
        );
        let boxes: Vec<_> = cx.floor().cad.iter().map(|c| c.bounds()).collect();
        assert_eq!(boxes.len(), 3);
        assert_eq!(
            boxes[0],
            (Point::new(100.0, 100.0), Point::new(200.0, 200.0))
        );
        let has = |d: f64| {
            boxes.iter().any(|b| {
                (b.0.x - (100.0 - d)).abs() < 1e-6
                    && (b.0.y - (100.0 - d)).abs() < 1e-6
                    && (b.1.x - (200.0 + d)).abs() < 1e-6
                    && (b.1.y - (200.0 + d)).abs() < 1e-6
            })
        };
        assert!(has(12.0) && has(24.0), "{boxes:?}");
        assert_eq!(cx.undo().as_deref(), Some("Concentric Copy"));
        assert_eq!(cx.floor().cad.len(), 1);
    }

    #[test]
    fn concentric_circle_follows_the_pointer_radius() {
        let mut cx = cx_with(EditBehavior::Concentric);
        let id = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(0.0, 0.0),
                radius: 40.0,
            },
        );
        cx.selection.set(ObjectRef::Cad(id));
        let mut tool = SelectTool::default();
        let down = PointerEvent::at(&cx, Point::new(0.0, 40.0)).with_down(true);
        tool.pointer_down(&mut cx, down);
        let to = PointerEvent::at(&cx, Point::new(0.0, 55.0)).with_down(true);
        tool.pointer_move(&mut cx, to);
        tool.pointer_move(&mut cx, to);
        tool.pointer_up(&mut cx, to);
        let radii: Vec<f64> = cx
            .floor()
            .cad
            .iter()
            .filter_map(|c| match c.item {
                CadItem::Circle { radius, .. } => Some(radius),
                _ => None,
            })
            .collect();
        assert_eq!(radii.len(), 2);
        assert!(radii.contains(&40.0) && radii.contains(&55.0), "{radii:?}");
    }

    #[test]
    fn fillet_rounds_a_dragged_corner() {
        let mut cx = cx_with(EditBehavior::Fillet);
        let id = square(&mut cx, Point::new(0.0, 0.0), 100.0);
        cx.selection.set(ObjectRef::Cad(id));
        // Grab the corner at (100, 100) and drag 20" away: radius 20.
        drag(
            &mut cx,
            Point::new(100.0, 100.0),
            Point::new(112.0, 116.0),
            false,
        );
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!("polyline");
        };
        assert!(points.len() > 4, "{} points", points.len());
        // The corner itself is gone; the arc starts 20" short of it.
        assert!(!points.contains(&Point::new(100.0, 100.0)));
        assert!(points
            .iter()
            .any(|p| (p.x - 80.0).abs() < 1e-6 && (p.y - 100.0).abs() < 1e-6));
        // A fixed radius from the dialog wins over the drag.
        cx.undo();
        cx.defaults.editing.behavior.fillet_radius = 10.0;
        drag(
            &mut cx,
            Point::new(100.0, 100.0),
            Point::new(180.0, 180.0),
            false,
        );
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!("polyline");
        };
        assert!(points
            .iter()
            .any(|p| (p.x - 90.0).abs() < 1e-6 && (p.y - 100.0).abs() < 1e-6));
    }

    #[test]
    fn replicate_leaves_the_original_and_places_copies() {
        let mut cx = cx_with(EditBehavior::Replicate);
        let id = square(&mut cx, Point::new(0.0, 0.0), 50.0);
        let wall = cx.project.add_wall(
            0,
            Point::new(0.0, 300.0),
            Point::new(100.0, 300.0),
            6.0,
            100.0,
            WallKind::Interior,
        );
        cx.defaults.editing.behavior.replicate_copies = 2;
        cx.selection.items = vec![ObjectRef::Cad(id), ObjectRef::Wall(wall)];
        drag(
            &mut cx,
            Point::new(25.0, 0.0),
            Point::new(125.0, 0.0),
            false,
        );
        let origins: Vec<f64> = cx.floor().cad.iter().map(|c| c.bounds().0.x).collect();
        assert_eq!(origins, vec![0.0, 100.0, 200.0]);
        assert_eq!(cx.floor().walls.len(), 3);
        assert_eq!(cx.floor().wall(wall).unwrap().start, Point::new(0.0, 300.0));
        assert_eq!(cx.undo().as_deref(), Some("Replicate Objects"));
        assert_eq!(cx.floor().cad.len(), 1);
    }

    #[test]
    fn scale_item_scales_every_kind() {
        let o = Point::new(10.0, 10.0);
        let mut c = CadItem::Circle {
            center: Point::new(20.0, 10.0),
            radius: 5.0,
        };
        scale_item(&mut c, o, 2.0, 2.0);
        assert_eq!(
            c,
            CadItem::Circle {
                center: Point::new(30.0, 10.0),
                radius: 10.0
            }
        );
        let mut t = CadItem::Text {
            pos: Point::new(20.0, 20.0),
            text: "A".into(),
            height: 4.0,
            angle: 0.0,
        };
        scale_item(&mut t, o, 3.0, 3.0);
        assert!(
            matches!(t, CadItem::Text { pos, height, .. } if pos == Point::new(40.0, 40.0) && (height - 12.0).abs() < 1e-9)
        );
    }

    #[test]
    fn chamfer_cuts_a_dragged_corner() {
        let mut cx = cx_with(EditBehavior::Chamfer);
        let id = square(&mut cx, Point::new(0.0, 0.0), 100.0);
        cx.selection.set(ObjectRef::Cad(id));
        // Grab the corner at (100, 100) and drag 20" away: distance 20.
        drag(
            &mut cx,
            Point::new(100.0, 100.0),
            Point::new(112.0, 116.0),
            false,
        );
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!("polyline");
        };
        assert_eq!(points.len(), 5, "one corner became two points");
        assert!(!points.contains(&Point::new(100.0, 100.0)));
        assert!(points.contains(&Point::new(80.0, 100.0)));
        assert!(points.contains(&Point::new(100.0, 80.0)));
        // A fixed distance from the dialog wins over the drag.
        cx.undo();
        cx.defaults.editing.behavior.chamfer_distance = 10.0;
        drag(
            &mut cx,
            Point::new(100.0, 100.0),
            Point::new(180.0, 180.0),
            false,
        );
        let CadItem::Polyline { points, .. } = &cx.floor().cad[0].item else {
            panic!("polyline");
        };
        assert!(points.contains(&Point::new(90.0, 100.0)));
        assert!(points.contains(&Point::new(100.0, 90.0)));
    }

    #[test]
    fn replicate_can_hand_the_drag_to_the_transform_dialog() {
        let mut cx = cx_with(EditBehavior::Replicate);
        cx.defaults.editing.behavior.replicate_dialog = true;
        cx.defaults.editing.behavior.replicate_copies = 3;
        let id = square(&mut cx, Point::new(0.0, 0.0), 40.0);
        cx.selection.set(ObjectRef::Cad(id));
        crate::dialogs::transform::close_for_tests();
        drag(
            &mut cx,
            Point::new(20.0, 20.0),
            Point::new(80.0, 20.0),
            false,
        );
        // Nothing was placed by the drag itself.
        assert_eq!(cx.floor().cad.len(), 1);
        let d = crate::dialogs::transform::current().expect("the dialog opened");
        assert!(d.make_copies && d.copies == 3);
        assert_eq!(plan_core::units::parse_ft_in(&d.move_x), Some(60.0));
        assert_eq!(plan_core::units::parse_ft_in(&d.move_y), Some(0.0));
        // Apply from the dialog places the copies, one undo step.
        let mut d = d;
        assert!(d.apply(&mut cx));
        assert_eq!(cx.floor().cad.len(), 4);
        // Without the switch a drag places copies at once and opens nothing.
        crate::dialogs::transform::close_for_tests();
        cx.defaults.editing.behavior.replicate_dialog = false;
        cx.project.floors[0].cad.truncate(1);
        cx.selection.set(ObjectRef::Cad(id));
        drag(
            &mut cx,
            Point::new(20.0, 20.0),
            Point::new(80.0, 20.0),
            false,
        );
        assert!(!crate::dialogs::transform::is_open());
        assert_eq!(cx.floor().cad.len(), 4);
    }
    #[test]
    fn the_indicator_names_the_behavior_and_leaving_select_resets_it() {
        let mut cx = cx_with(EditBehavior::Default);
        assert_eq!(indicator(&cx), None);
        cx.defaults.editing.behavior.mode = EditBehavior::Resize;
        assert_eq!(indicator(&cx).as_deref(), Some("Edit Behavior: Resize"));
        // Switching away from Select Objects resets it (S-66).
        let mut tool = SelectTool::default();
        tool.deactivate(&mut cx);
        assert_eq!(mode(&cx), EditBehavior::Default);
        assert_eq!(indicator(&cx), None);
        assert!(cx.status.contains("reset"));
        assert!(!reset(&mut cx));
    }
}
