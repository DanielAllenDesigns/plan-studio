//! Edit Behaviors (S-65): what a body or handle drag of the Select tool does
//! under each mode of Edit > Edit Behaviors (`PlanDefaults::editing.behavior`).
//!
//! * Default: move and reshape in place (the Select tool's own code); a
//!   polyline corner moves alone, a wall or box moves square to its edges;
//! * Alternate (manual p. 253; Round 16): a corner handle keeps the angles
//!   beside it (the neighbouring corners slide along their edges), a wall or
//!   group moves at the allowed angles, drawing tools chain continuously.
//!   Holding Alt (or the right button) summons it for one operation;
//! * Move: any resize handle moves the whole object (summon Z or /);
//! * Resize: a body drag scales the selected CAD objects from the corner of
//!   their bounds opposite the one nearest the press; so does a polyline
//!   corner handle, in proportion (summon X or .);
//! * Concentric: a body drag of a circle, arc, line or polyline leaves it and
//!   adds offset copies (the drag sets the distance, or the dialog does); a
//!   polyline handle moves every edge the same distance, in Concentric Jump
//!   steps (summon C, or the X1 mouse button);
//! * Fillet: dragging a polyline corner handle rounds that corner (summon F);
//! * Chamfer: dragging a polyline corner cuts it off with a chamfer;
//! * Replicate: a body drag leaves the originals and places copies, each one
//!   more drag delta along (or, with `replicate_dialog`, releasing the drag
//!   opens Transform/Replicate Object with the drag as its Move).
//!
//! The active behavior is global and restored to Default when the program
//! closes ([`restore_default`]). [`summon`] says which behavior the keys and
//! mouse buttons held right now call up; [`mode`] is that, else the global
//! setting.
//!
//! The Select tool calls [`apply_group`] and [`apply_vertex`] before its own
//! drag code, with the project already reset to the drag's starting state, so
//! every call rebuilds the result from scratch.

use super::ops::translate_cad;
use super::selection::ObjectRef;
use super::EditorContext;
use crate::tools::PointerEvent;
use plan_core::cad::{self, CadItem};
use plan_core::defaults::EditBehavior;
use plan_core::geometry::{project_on_segment, Point};
use plan_core::Id;
use std::cell::Cell;

thread_local! {
    /// The behavior summoned for the operation under way (Alt, a key, a
    /// mouse button), `None` when the global one rules.
    static SUMMONED: Cell<Option<EditBehavior>> = const { Cell::new(None) };
    /// The behavior the held keys and mouse buttons call up, as the shell
    /// last saw them.
    static HELD_SUMMON: Cell<Option<EditBehavior>> = const { Cell::new(None) };
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

/// The behavior in force: the one summoned for this operation, else the
/// global setting.
pub fn mode(cx: &EditorContext) -> EditBehavior {
    SUMMONED
        .with(Cell::get)
        .unwrap_or(cx.defaults.editing.behavior.mode)
}

/// The behavior the keys and mouse buttons held down call up (manual pp.
/// 253-256): Z or / Move, X or . Resize (also the X2 button), C Concentric
/// (also the X1 button), F Fillet; with Alt held, Z or / calls up Default.
/// Pass `None` when none is held.
pub fn note_held_summon(held: Option<EditBehavior>) {
    HELD_SUMMON.with(|h| h.set(held));
}

/// What the keys and mouse buttons held in `i` call up (see
/// [`note_held_summon`]). Ctrl/Cmd chords call up nothing: they belong to the
/// hotkey table and to the override role.
pub fn held_summon(i: &eframe::egui::InputState) -> Option<EditBehavior> {
    use eframe::egui::{Key, PointerButton};
    if i.modifiers.command || i.modifiers.ctrl {
        return None;
    }
    let down = |k: Key| i.key_down(k);
    let move_key = down(Key::Z) || down(Key::Slash);
    if i.modifiers.alt && move_key {
        return Some(EditBehavior::Default);
    }
    if move_key {
        Some(EditBehavior::Move)
    } else if down(Key::X) || down(Key::Period) || i.pointer.button_down(PointerButton::Extra2) {
        Some(EditBehavior::Resize)
    } else if down(Key::C) || i.pointer.button_down(PointerButton::Extra1) {
        Some(EditBehavior::Concentric)
    } else if down(Key::F) {
        Some(EditBehavior::Fillet)
    } else {
        None
    }
}

/// Works out the behavior summoned for the operation `p` belongs to: a held
/// summon key, else Alt or the right button (Alternate). Call it at the start
/// of every pointer event of an edit or drawing operation.
pub fn summon(p: &PointerEvent) -> Option<EditBehavior> {
    let s = HELD_SUMMON
        .with(Cell::get)
        .or_else(|| p.alternate().then_some(EditBehavior::Alternate));
    SUMMONED.with(|c| c.set(s));
    s
}

/// The operation ended: the global behavior rules again.
pub fn end_summon() {
    SUMMONED.with(|c| c.set(None));
}

/// The program is closing: Default is restored (manual p. 252).
pub fn restore_default(cx: &mut EditorContext) {
    cx.defaults.editing.behavior.mode = EditBehavior::Default;
    end_summon();
    note_held_summon(None);
}

/// Is the Alternate behavior in force (chosen, or summoned by Alt)?
pub fn alternate(cx: &EditorContext) -> bool {
    mode(cx) == EditBehavior::Alternate
}

/// Do moves follow the allowed angles instead of squaring to the object's
/// edges? Alternate and Move do (manual pp. 236, 254), as does the Polar
/// Primary Movement Method.
pub fn polar_move(cx: &EditorContext) -> bool {
    matches!(mode(cx), EditBehavior::Alternate | EditBehavior::Move)
        || cx.defaults.editing.behavior.movement_polar
}

/// `delta` turned to the nearest allowed angle (its length kept, rounded to
/// the snap unit). `None` when angle snaps are off or no angle is allowed.
pub fn polar_delta(cx: &EditorContext, delta: Point) -> Option<Point> {
    if !cx.defaults.editing.angle_snaps {
        return None;
    }
    let set = super::snap::allowed_angle_set(
        &cx.defaults.editing,
        cx.defaults.grid.angle_snap_deg,
        super::snap::held().shift,
    );
    let len = delta.length();
    if set.is_empty() || len < 1e-9 {
        return None;
    }
    let here = delta.angle().to_degrees().rem_euclid(360.0);
    let diff = |a: f64| {
        let d = (here - a).rem_euclid(360.0);
        d.min(360.0 - d)
    };
    let best = set
        .iter()
        .copied()
        .min_by(|a, b| diff(*a).total_cmp(&diff(*b)))?;
    let unit = cx.snap_unit();
    let len = round_to(len, unit);
    let r = best.to_radians();
    let clean = |v: f64| (v * 1e12).round() / 1e12;
    Some(Point::new(clean(r.cos()) * len, clean(r.sin()) * len))
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
    end_summon();
    if cx.defaults.editing.behavior.mode == EditBehavior::Default {
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
        EditBehavior::Alternate | EditBehavior::Move => "Move Objects",
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

/// The delta of a body drag of a group: under Alternate (or Move, or the
/// Polar movement method) it follows an allowed angle unless `free` (Ctrl or
/// Cmd) overrides the restriction.
pub fn group_delta(cx: &EditorContext, total: Point, free: bool) -> Point {
    if !free && polar_move(cx) {
        if let Some(d) = polar_delta(cx, total) {
            return d;
        }
    }
    total
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

/// The polyline `points` with corner `i` at `to`, the angles beside it kept:
/// the neighbouring corners slide along their own edges so every edge keeps
/// its direction and only lengthens or shortens (Alternate, manual p. 253).
/// `None` when the shape cannot keep them (a triangle, parallel edges).
pub fn reshape_keeping_angles(
    points: &[Point],
    closed: bool,
    i: usize,
    to: Point,
) -> Option<Vec<Point>> {
    let n = points.len();
    if i >= n || n < 3 || (closed && n < 4) {
        return None;
    }
    let at = |k: isize| -> Option<usize> {
        if closed {
            Some(k.rem_euclid(n as isize) as usize)
        } else {
            (0..n as isize).contains(&k).then_some(k as usize)
        }
    };
    let ii = i as isize;
    let mut out = points.to_vec();
    out[i] = to;
    // One side: the neighbour `nb` whose far neighbour `far` stays, so `nb`
    // slides along the line `far`-`nb` until the edge to `to` is parallel to
    // the old one.
    let mut side = |nb: Option<usize>, far: Option<usize>| -> Option<()> {
        let (nb, far) = (nb?, far?);
        let edge = points[nb].sub(points[i]);
        let hold = points[nb].sub(points[far]);
        let d = edge.cross(hold);
        if d.abs() < 1e-9 {
            return None;
        }
        // to + t * edge lies on the line through `far` along `hold`.
        let t = points[far].sub(to).cross(hold) / d;
        out[nb] = to + edge * t;
        Some(())
    };
    let prev = at(ii - 1);
    let next = at(ii + 1);
    let mut moved = false;
    if prev.is_some() && at(ii - 2).is_some() {
        side(prev, at(ii - 2))?;
        moved = true;
    }
    if next.is_some() && at(ii + 2).is_some() {
        side(next, at(ii + 2))?;
        moved = true;
    }
    moved.then_some(out)
}

/// The signed step (`offset_polyline` convention: left of the direction of
/// travel is positive) that carries corner `i` of the polyline to where
/// `world` is, every edge moving the same distance. A closed polyline's
/// outward bisector decides how far a diagonal drag counts; an open one, or a
/// corner with no bisector, uses the distance to the line.
fn concentric_step(points: &[Point], closed: bool, i: usize, world: Point) -> f64 {
    let n = points.len();
    if closed && n >= 3 && i < n {
        let area: f64 = (0..n)
            .map(|k| points[k].cross(points[(k + 1) % n]))
            .sum::<f64>();
        let ccw = area > 0.0;
        // Unit outward normal of the edge from `a` to `b`.
        let out = |a: Point, b: Point| {
            let d = b.sub(a);
            let l = d.length();
            if l < 1e-9 {
                return Point::ZERO;
            }
            let right = Point::new(d.y / l, -d.x / l);
            if ccw {
                right
            } else {
                Point::new(-right.x, -right.y)
            }
        };
        let v = points[i];
        let n1 = out(points[(i + n - 1) % n], v);
        let n2 = out(v, points[(i + 1) % n]);
        let sum = n1 + n2;
        let len = sum.length();
        if len > 1e-9 {
            let b = Point::new(sum.x / len, sum.y / len);
            let c = n1.dot(n2);
            let outward = world.sub(v).dot(b) * ((1.0 + c) / 2.0).max(0.0).sqrt();
            // Left of travel is inside for a counter-clockwise polygon.
            return if ccw { -outward } else { outward };
        }
    }
    signed_distance(points, closed, world)
}

/// A polyline handle drag under Move, Resize, Concentric, Alternate, Fillet
/// or Chamfer, in place of the plain vertex move. `start` is where the drag
/// began and `world` where it is now. True when the mode took the drag.
pub fn apply_vertex(
    cx: &mut EditorContext,
    id: Id,
    kind: super::handles::HandleKind,
    start: Point,
    world: Point,
) -> bool {
    use super::handles::HandleKind;
    let fl = cx.floor;
    let unit = cx.snap_unit();
    let m = mode(cx);
    match m {
        EditBehavior::Move => {
            // Any handle moves the object.
            let d = Point::new(
                round_to(world.x - start.x, unit),
                round_to(world.y - start.y, unit),
            );
            let d = group_delta(cx, d, false);
            if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                translate_cad(&mut c.item, d);
                cx.mark_dirty();
                return true;
            }
            false
        }
        EditBehavior::Alternate => {
            let HandleKind::Reshape(i) = kind else {
                return false;
            };
            let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) else {
                return false;
            };
            let CadItem::Polyline { points, closed } = &mut c.item else {
                return false;
            };
            if let Some(pts) = reshape_keeping_angles(points, *closed, i, world) {
                *points = pts;
                cx.mark_dirty();
                return true;
            }
            false
        }
        EditBehavior::Resize => {
            // A corner handle scales the object in proportion about the
            // opposite corner of its bounds.
            let HandleKind::Reshape(i) = kind else {
                return false;
            };
            let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) else {
                return false;
            };
            let CadItem::Polyline { points, .. } = &c.item else {
                return false;
            };
            let Some(corner) = points.get(i).copied() else {
                return false;
            };
            let (lo, hi) = c.bounds();
            let mid = Point::lerp(lo, hi, 0.5);
            let anchor = Point::new(
                if corner.x >= mid.x { lo.x } else { hi.x },
                if corner.y >= mid.y { lo.y } else { hi.y },
            );
            let span = corner.sub(anchor);
            let len2 = span.dot(span);
            if len2 < 1e-9 {
                return false;
            }
            let s = (world.sub(anchor).dot(span) / len2).max(MIN_SCALE);
            scale_item(&mut c.item, anchor, s, s);
            cx.mark_dirty();
            true
        }
        EditBehavior::Concentric => {
            // Every edge moves the same distance, in Concentric Jump steps.
            let jump = {
                let j = cx.defaults.editing.behavior.concentric_jump;
                if j > 0.0 {
                    j
                } else {
                    unit
                }
            };
            let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) else {
                return false;
            };
            match &mut c.item {
                CadItem::Polyline { points, closed } => {
                    let i = match kind {
                        HandleKind::Reshape(i) => i,
                        _ => 0,
                    };
                    let step = round_to(concentric_step(points, *closed, i, world), jump);
                    if step.abs() < 1e-6 {
                        return true;
                    }
                    let pts = cad::offset_polyline(points, *closed, step);
                    if pts.len() >= 2 {
                        *points = pts;
                    }
                    cx.mark_dirty();
                    true
                }
                CadItem::Circle { center, radius } | CadItem::Arc { center, radius, .. } => {
                    *radius = round_to(center.dist(world), jump).max(0.5);
                    cx.mark_dirty();
                    true
                }
                _ => false,
            }
        }
        EditBehavior::Fillet | EditBehavior::Chamfer => {
            fillet_vertex(cx, id, kind, world, m == EditBehavior::Chamfer)
        }
        _ => false,
    }
}

/// Dragging a polyline corner handle under Fillet or Chamfer rounds or cuts
/// the corner instead of moving it.
fn fillet_vertex(
    cx: &mut EditorContext,
    id: Id,
    kind: super::handles::HandleKind,
    world: Point,
    chamfer: bool,
) -> bool {
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
    fn alternate_moves_a_group_at_the_allowed_angles() {
        let mut cx = cx_with(EditBehavior::Alternate);
        let id = square(&mut cx, Point::new(100.0, 100.0), 100.0);
        cx.selection.set(ObjectRef::Cad(id));
        // 40" east and 20" north is 26.6 degrees: the 30 degree ray wins and
        // the 44.7" length rounds to 45" (then to the grid, 39 and 23).
        drag(
            &mut cx,
            Point::new(150.0, 100.0),
            Point::new(190.0, 120.0),
            false,
        );
        assert_eq!(bounds(&cx, id).0, Point::new(139.0, 123.0));
        // With angle snaps off nothing restricts the move.
        cx.undo();
        cx.temp.clear();
        cx.defaults.editing.angle_snaps = false;
        drag(
            &mut cx,
            Point::new(150.0, 100.0),
            Point::new(190.0, 120.0),
            false,
        );
        assert_eq!(bounds(&cx, id).0, Point::new(140.0, 120.0));
    }

    #[test]
    fn polar_delta_snaps_to_the_nearest_allowed_angle() {
        let mut cx = cx_with(EditBehavior::Default);
        let d = polar_delta(&cx, Point::new(100.0, 10.0)).unwrap();
        // 5.7 degrees falls to 0; the 100.5" length rounds to 101" ... 100".
        assert!((d.y).abs() < 1e-9 && (d.x - 100.0).abs() < 1e-9, "{d:?}");
        let d = polar_delta(&cx, Point::new(-60.0, 70.0)).unwrap();
        // 130.6 degrees falls to 135.
        assert!((d.x + d.y).abs() < 1e-6 && d.y > 0.0, "{d:?}");
        // The opposing angle of 22.5 is allowed too.
        cx.defaults.editing.additional_angles = vec![22.5];
        let d = polar_delta(&cx, Point::new(-92.0, -38.0)).unwrap();
        let a = d.y.atan2(d.x).to_degrees().rem_euclid(360.0);
        assert!((a - 202.5).abs() < 1e-6, "{a}");
        cx.defaults.editing.angle_snaps = false;
        assert!(polar_delta(&cx, Point::new(1.0, 1.0)).is_none());
    }

    #[test]
    fn reshaping_a_box_corner_keeps_its_angles_by_sliding_the_neighbours() {
        let b = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 60.0),
            Point::new(0.0, 60.0),
        ];
        let out = reshape_keeping_angles(&b, true, 2, Point::new(120.0, 90.0)).unwrap();
        assert_eq!(out[0], Point::new(0.0, 0.0));
        assert_eq!(out[1], Point::new(120.0, 0.0));
        assert_eq!(out[2], Point::new(120.0, 90.0));
        assert_eq!(out[3], Point::new(0.0, 90.0));
        // A triangle cannot keep its angles.
        let t = &b[..3];
        assert!(reshape_keeping_angles(t, true, 1, Point::new(110.0, 10.0)).is_none());
        // An open polyline's inner corner slides the neighbour that has a
        // corner beyond it; a free end stays where it is.
        let open = vec![
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
            Point::new(50.0, 50.0),
            Point::new(50.0, 0.0),
        ];
        let out = reshape_keeping_angles(&open, false, 1, Point::new(-10.0, 60.0)).unwrap();
        assert_eq!(out[0], Point::new(0.0, 0.0));
        assert_eq!(out[1], Point::new(-10.0, 60.0));
        assert_eq!(out[2], Point::new(50.0, 60.0));
        assert_eq!(out[3], Point::new(50.0, 0.0));
        // A two-point line has nothing to keep.
        assert!(reshape_keeping_angles(&open[..2], false, 0, Point::new(5.0, 5.0)).is_none());
    }

    #[test]
    fn a_summoned_behavior_lasts_for_the_operation_only() {
        let mut cx = cx_with(EditBehavior::Default);
        let alt = eframe::egui::Modifiers {
            alt: true,
            ..eframe::egui::Modifiers::NONE
        };
        let p = PointerEvent::at(&cx, Point::ZERO).with_modifiers(alt);
        assert_eq!(summon(&p), Some(EditBehavior::Alternate));
        assert_eq!(mode(&cx), EditBehavior::Alternate);
        // Z calls up Move; Alt+Z calls up Default again.
        note_held_summon(Some(EditBehavior::Move));
        assert_eq!(summon(&p), Some(EditBehavior::Move));
        note_held_summon(Some(EditBehavior::Default));
        assert_eq!(summon(&p), Some(EditBehavior::Default));
        note_held_summon(None);
        let plain = PointerEvent::at(&cx, Point::ZERO);
        assert_eq!(summon(&plain), None);
        assert_eq!(mode(&cx), EditBehavior::Default);
        // The right button summons Alternate too.
        let mut right = plain;
        right.button = eframe::egui::PointerButton::Secondary;
        assert_eq!(summon(&right), Some(EditBehavior::Alternate));
        end_summon();
        // Closing the program restores Default.
        cx.defaults.editing.behavior.mode = EditBehavior::Fillet;
        restore_default(&mut cx);
        assert_eq!(mode(&cx), EditBehavior::Default);
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
