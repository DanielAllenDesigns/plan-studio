//! Edit Area, Edit Area Visible and Stretch CAD (S-90, S-91).
//!
//! Edit > Edit Area asks for a rubber band. The rectangle it leaves is the
//! edit area: dragging inside it moves everything wholly inside (Ctrl or Cmd
//! held when the drag starts copies it instead), dragging its Rotate handle
//! turns it about its center, Delete removes the contents, Esc or a click
//! outside ends the mode. A wall that crosses the rectangle's edge does not
//! come along whole: the end inside moves (or turns) and the end outside
//! stays, so the wall stretches, with its doors and windows keeping their
//! place. "Edit Area Visible" takes only objects on displayed layers; "Edit
//! Area" also takes those on hidden (never locked) layers.
//!
//! Stretch CAD takes the same rubber band, then one drag: every CAD vertex
//! inside the band moves with the pointer, the vertices outside stay.
//!
//! Each drag is rebuilt from the plan as it was when the drag began, and is
//! one undo step. Typed digits give the distance (and angle with Tab) of a
//! move, or the degrees of a turn.

use super::*;
use crate::editor::typed_input::TypedKey;
use plan_core::cad::CadItem;
use plan_core::geometry::point_in_polygon;
use plan_core::transform::Xform;
use std::cell::RefCell;

/// Which of the two commands is under way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AreaKind {
    Edit { visible_only: bool },
    StretchCad,
}

enum Stage {
    /// Waiting for the rubber band (`start` set once the button is down).
    Band {
        start: Option<Point>,
        current: Point,
    },
    /// The region is placed.
    Placed { lo: Point, hi: Point },
    /// Dragging the contents.
    Moving {
        lo: Point,
        hi: Point,
        start: Point,
        original: Box<Project>,
        copy: bool,
        shift: Point,
    },
    /// Dragging the Rotate handle.
    Turning {
        lo: Point,
        hi: Point,
        start: Point,
        original: Box<Project>,
        angle: f64,
    },
}

/// What a command covers beyond a rectangle on the current floor: all floors,
/// or a closed polyline as the marquee (manual p. 298 to 299).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scope {
    pub all_floors: bool,
    /// The outline of the selected closed polyline, when it is the marquee.
    pub poly: Option<Vec<Point>>,
    /// That polyline's CAD object and floor.
    pub poly_cad: Option<Id>,
    pub poly_floor: usize,
    /// The "Including the Polyline" forms move the polyline with the contents.
    pub include_poly: bool,
}

struct State {
    kind: AreaKind,
    stage: Stage,
    scope: Scope,
}

/// The region as the geometry sees it: a rectangle or the polyline outline.
struct Region<'a> {
    lo: Point,
    hi: Point,
    poly: Option<&'a [Point]>,
}

impl<'a> Region<'a> {
    fn new(lo: Point, hi: Point, scope: &'a Scope) -> Self {
        Region {
            lo,
            hi,
            poly: scope.poly.as_deref(),
        }
    }

    fn contains(&self, p: Point) -> bool {
        match self.poly {
            Some(q) => point_in_polygon(p, q),
            None => in_rect(p, self.lo, self.hi),
        }
    }

    fn edges(&self) -> Vec<(Point, Point)> {
        let pts: Vec<Point> = match self.poly {
            Some(q) => q.to_vec(),
            None => rect_corners(self.lo, self.hi).to_vec(),
        };
        (0..pts.len())
            .map(|i| (pts[i], pts[(i + 1) % pts.len()]))
            .collect()
    }
}

/// Where the segment `a`-`b` crosses `c`-`d`, as a fraction of `a`-`b`.
fn seg_cross_t(a: Point, b: Point, c: Point, d: Point) -> Option<f64> {
    let (r, s) = (b - a, d - c);
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-12 {
        return None;
    }
    let q = c - a;
    let t = (q.x * s.y - q.y * s.x) / den;
    let u = (q.x * r.y - q.y * r.x) / den;
    ((0.0..=1.0).contains(&u) && t > 0.0 && t < 1.0).then_some(t)
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let l2 = ab.dot(ab);
    let t = if l2 < 1e-12 {
        0.0
    } else {
        ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
    };
    p.dist(a + ab * t)
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn take() -> Option<State> {
    STATE.with(|s| s.borrow_mut().take())
}

fn put(st: State) {
    STATE.with(|s| *s.borrow_mut() = Some(st));
}

/// Is Edit Area or Stretch CAD waiting for or holding a region?
pub fn active() -> bool {
    STATE.with(|s| s.borrow().is_some())
}

/// The placed region (corners `lo`, `hi`), once the rubber band is done.
pub fn region() -> Option<(Point, Point)> {
    STATE.with(|s| match s.borrow().as_ref().map(|s| &s.stage) {
        Some(Stage::Placed { lo, hi })
        | Some(Stage::Moving { lo, hi, .. })
        | Some(Stage::Turning { lo, hi, .. }) => Some((*lo, *hi)),
        _ => None,
    })
}

/// Starts the command: the next press-drag draws the rubber band.
pub fn begin(cx: &mut EditorContext, kind: AreaKind) {
    begin_with(cx, kind, false, false);
}

/// The selected closed polyline, if exactly one is selected (CAD object id,
/// its outline).
fn selected_closed_polyline(cx: &EditorContext) -> Option<(Id, Vec<Point>)> {
    let [ObjectRef::Cad(id)] = cx.selection.items[..] else {
        return None;
    };
    match &cx.floor().cad.iter().find(|c| c.id == id)?.item {
        CadItem::Polyline {
            points,
            closed: true,
        } if points.len() >= 3 => Some((id, points.clone())),
        _ => None,
    }
}

/// Starts an Edit Area command. `all_floors` takes every floor; `including`
/// moves the marquee polyline along with its contents. When a closed
/// polyline is selected it becomes the marquee at once (no rubber band).
pub fn begin_with(cx: &mut EditorContext, kind: AreaKind, all_floors: bool, including: bool) {
    transform::cancel_mode(cx);
    cancel(cx);
    let mut scope = Scope {
        all_floors,
        ..Scope::default()
    };
    let mut stage = Stage::Band {
        start: None,
        current: Point::ZERO,
    };
    if matches!(kind, AreaKind::Edit { .. }) {
        if let Some((id, pts)) = selected_closed_polyline(cx) {
            let lo = Point::new(
                pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
                pts.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
            );
            let hi = Point::new(
                pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max),
                pts.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
            );
            scope.poly = Some(pts);
            scope.poly_cad = Some(id);
            scope.poly_floor = cx.floor;
            scope.include_poly = including;
            stage = Stage::Placed { lo, hi };
        }
    }
    let placed = matches!(stage, Stage::Placed { .. });
    put(State { kind, stage, scope });
    if placed {
        cx.status = "Edit area from the polyline: drag to move, the handle to rotate, Delete removes, Esc ends".into();
        return;
    }
    cx.status = match kind {
        AreaKind::Edit { visible_only: true } => {
            "Edit Area Visible: drag a rectangle around what to edit".into()
        }
        AreaKind::Edit { .. } => "Edit Area: drag a rectangle around what to edit".into(),
        AreaKind::StretchCad => "Stretch CAD: drag a window around the vertices to stretch".into(),
    };
}

/// Ends the mode; a drag in progress is undone.
pub fn cancel(cx: &mut EditorContext) {
    if let Some(st) = take() {
        match st.stage {
            Stage::Moving { original, .. } | Stage::Turning { original, .. } => {
                cx.project = *original;
                cx.cancel_change();
                cx.typed_input.disarm();
                cx.readout = None;
                cx.mark_dirty();
            }
            _ => {}
        }
    }
}

const ROTATE_HANDLE_PX: f64 = 26.0;

/// The Rotate handle above the middle of the region's top edge.
pub fn rotate_handle(lo: Point, hi: Point, px_per_in: f64) -> Point {
    Point::new(
        (lo.x + hi.x) * 0.5,
        hi.y + ROTATE_HANDLE_PX / px_per_in.max(1e-6),
    )
}

fn center_of(lo: Point, hi: Point) -> Point {
    Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5)
}

// ----- what the region holds -----

/// The objects wholly inside the region, apart from walls and openings (those
/// are handled end by end). The marquee polyline is left out unless the
/// command includes it.
fn region_items(
    cx: &EditorContext,
    reg: &Region,
    visible_only: bool,
    scope: &Scope,
) -> Vec<ObjectRef> {
    let mut v: Vec<ObjectRef> = objects_in_box(cx, reg.lo, reg.hi, false, !visible_only)
        .into_iter()
        .filter(|o| !matches!(o, ObjectRef::Wall(_) | ObjectRef::Opening(_)))
        .collect();
    if let Some(q) = reg.poly {
        v.retain(|o| {
            let pts = transform::object_points(cx, *o);
            !pts.is_empty() && pts.iter().all(|p| point_in_polygon(*p, q))
        });
    }
    if let Some(id) = scope.poly_cad {
        v.retain(|o| *o != ObjectRef::Cad(id));
        if scope.include_poly && cx.floor == scope.poly_floor {
            v.push(ObjectRef::Cad(id));
        }
    }
    v
}

/// The walls that lie wholly inside the region.
fn walls_inside(cx: &EditorContext, reg: &Region, visible_only: bool) -> Vec<Id> {
    let edges = reg.edges();
    objects_in_box(cx, reg.lo, reg.hi, false, !visible_only)
        .into_iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(id),
            _ => None,
        })
        .filter(|id| {
            reg.poly.is_none()
                || cx.floor().wall(*id).is_some_and(|w| {
                    reg.contains(w.start)
                        && reg.contains(w.end)
                        && edges
                            .iter()
                            .all(|(c, d)| seg_cross_t(w.start, w.end, *c, *d).is_none())
                })
        })
        .collect()
}

/// Moves (or turns) the parts of walls that lie inside the region. A wall
/// the boundary crosses is cut there: the part inside moves with the region,
/// the parts outside stay, and the cut ends follow the moved part so the wall
/// stays connected. Parts that end up in line again are one wall once more
/// (the rejoin on drop); openings keep their place in the plan unless they
/// were inside. Returns how many walls changed.
fn move_wall_ends(project: &mut Project, fl: usize, reg: &Region, xf: &Xform) -> usize {
    let ids: Vec<Id> = project.floors[fl].walls.iter().map(|w| w.id).collect();
    let edges = reg.edges();
    let mut n = 0;
    for id in ids {
        let Some(w) = project.floors[fl].wall(id).cloned() else {
            continue;
        };
        let len = w.start.dist(w.end);
        if len < 1.0 {
            continue;
        }
        if w.is_curved() {
            // A curve is not cut: the ends inside the region move.
            let (s_in, e_in) = (reg.contains(w.start), reg.contains(w.end));
            if !s_in && !e_in {
                continue;
            }
            let ns = if s_in { xf.apply(w.start) } else { w.start };
            let ne = if e_in { xf.apply(w.end) } else { w.end };
            if ns.dist(ne) < 1.0 {
                continue;
            }
            if let Some(wm) = project.floors[fl].wall_mut(id) {
                wm.start = ns;
                wm.end = ne;
            }
            n += 1;
            continue;
        }
        let at = |t: f64| w.start + (w.end - w.start) * t;
        let mut ts: Vec<f64> = edges
            .iter()
            .filter_map(|(c, d)| seg_cross_t(w.start, w.end, *c, *d))
            .filter(|t| t * len >= 1.0 && (1.0 - t) * len >= 1.0)
            .collect();
        ts.sort_by(|a, b| a.total_cmp(b));
        ts.dedup_by(|a, b| (*a - *b).abs() * len < 0.5);
        let mut cuts = vec![0.0];
        cuts.extend(ts);
        cuts.push(1.0);
        // Spans along the wall, neighbours on the same side merged.
        let mut spans: Vec<(f64, f64, bool)> = Vec::new();
        for k in 0..cuts.len() - 1 {
            let inside = reg.contains(at((cuts[k] + cuts[k + 1]) * 0.5));
            match spans.last_mut() {
                Some(l) if l.2 == inside => l.1 = cuts[k + 1],
                _ => spans.push((cuts[k], cuts[k + 1], inside)),
            }
        }
        if !spans.iter().any(|s| s.2) {
            continue;
        }
        let mut nodes = vec![if spans[0].2 {
            xf.apply(w.start)
        } else {
            w.start
        }];
        for s in &spans[..spans.len() - 1] {
            nodes.push(xf.apply(at(s.1)));
        }
        nodes.push(if spans[spans.len() - 1].2 {
            xf.apply(w.end)
        } else {
            w.end
        });
        // Parts in line with each other are one wall.
        let mut pieces: Vec<(Point, Point)> = Vec::new();
        for k in 0..spans.len() {
            let (a, b) = (nodes[k], nodes[k + 1]);
            if let Some(last) = pieces.last_mut() {
                let (u, v) = (last.1 - last.0, b - a);
                let cross = u.x * v.y - u.y * v.x;
                if cross.abs() <= 1e-4 * u.length() * v.length() && u.dot(v) > 0.0 {
                    last.1 = b;
                    continue;
                }
            }
            pieces.push((a, b));
        }
        if pieces.iter().any(|p| p.0.dist(p.1) < 1.0) {
            continue;
        }
        // Where the openings stand after the move.
        let places: Vec<(Id, Point)> = if spans.len() > 1 {
            project.floors[fl]
                .openings_on(id)
                .map(|o| {
                    let t = o.center_offset / len;
                    let inside = spans
                        .iter()
                        .find(|s| t >= s.0 - 1e-9 && t <= s.1 + 1e-9)
                        .is_some_and(|s| s.2);
                    let p = at(t);
                    (o.id, if inside { xf.apply(p) } else { p })
                })
                .collect()
        } else {
            Vec::new()
        };
        // The longest part keeps the wall's id (the first on a tie).
        let mut keep = 0;
        for (i, pc) in pieces.iter().enumerate() {
            if pc.0.dist(pc.1) > pieces[keep].0.dist(pieces[keep].1) + 1e-9 {
                keep = i;
            }
        }
        let mut piece_ids = Vec::new();
        for i in 0..pieces.len() {
            if i == keep {
                piece_ids.push(id);
                continue;
            }
            let nid = project.alloc_id();
            let mut extra = w.clone();
            extra.id = nid;
            project.floors[fl].walls.push(extra);
            piece_ids.push(nid);
        }
        for (pid, (a, b)) in piece_ids.iter().zip(&pieces) {
            if let Some(wm) = project.floors[fl].wall_mut(*pid) {
                wm.start = *a;
                wm.end = *b;
            }
        }
        for (oid, q) in places {
            let best = (0..pieces.len())
                .min_by(|i, j| {
                    dist_to_segment(q, pieces[*i].0, pieces[*i].1).total_cmp(&dist_to_segment(
                        q,
                        pieces[*j].0,
                        pieces[*j].1,
                    ))
                })
                .unwrap_or(0);
            let (a, b) = pieces[best];
            let plen = a.dist(b);
            let off = (q - a).dot((b - a).normalized());
            if let Some(o) = project.floors[fl].openings.iter_mut().find(|o| o.id == oid) {
                let half = o.width * 0.5;
                o.wall_id = piece_ids[best];
                o.center_offset = off.clamp(half, (plen - half).max(half));
            }
        }
        n += 1;
    }
    n
}

/// Stretch CAD: every CAD vertex inside the window moves by `d`. Circles,
/// arcs and text move whole when their center or anchor is inside. Returns the
/// number of objects that changed.
fn stretch_cad(cx: &mut EditorContext, lo: Point, hi: Point, d: Point) -> usize {
    let fl = cx.floor;
    let allowed: Vec<Id> = cx
        .floor()
        .cad
        .iter()
        .filter(|c| cx.layers().is_visible(&c.layer) && !cx.layers().is_locked(&c.layer))
        .map(|c| c.id)
        .collect();
    let mut n = 0;
    for c in cx.project.floors[fl].cad.iter_mut() {
        if !allowed.contains(&c.id) {
            continue;
        }
        let mut hit = false;
        let mut shift = |p: &mut Point| {
            if in_rect(*p, lo, hi) {
                *p = *p + d;
                hit = true;
            }
        };
        match &mut c.item {
            CadItem::Line { a, b } => {
                shift(a);
                shift(b);
            }
            CadItem::Polyline { points, .. } => points.iter_mut().for_each(shift),
            CadItem::Circle { center, .. } | CadItem::Arc { center, .. } => shift(center),
            CadItem::Text { pos, .. } => shift(pos),
        }
        if hit {
            n += 1;
        }
    }
    n
}

/// Runs `f` on each floor the command covers (the current one, or all with
/// `all`), then goes back to the floor it started on.
fn each_floor(cx: &mut EditorContext, all: bool, mut f: impl FnMut(&mut EditorContext)) {
    let home = cx.floor;
    let floors: Vec<usize> = if all {
        (0..cx.project.floors.len()).collect()
    } else {
        vec![home]
    };
    for fl in floors {
        cx.floor = fl;
        f(cx);
    }
    cx.floor = home;
}

/// Rebuilds the plan from `original` with the region's contents moved by `d`
/// (a copy of them placed `d` away with `copy`) or, for `turn`, turned.
/// Returns what changed, for the status line.
#[allow(clippy::too_many_arguments)]
fn apply(
    cx: &mut EditorContext,
    original: &Project,
    kind: AreaKind,
    (lo, hi): (Point, Point),
    scope: &Scope,
    d: Point,
    turn: Option<f64>,
    copy: bool,
) -> usize {
    cx.project = original.clone();
    let mut n = 0;
    if kind == AreaKind::StretchCad {
        n = stretch_cad(cx, lo, hi, d);
    } else if let AreaKind::Edit { visible_only } = kind {
        let reg = Region::new(lo, hi, scope);
        let all = scope.all_floors;
        each_floor(cx, all, |cx| {
            let fl = cx.floor;
            let walls_before = cx.floor().walls.clone();
            let items = region_items(cx, &reg, visible_only, scope);
            if copy {
                // Copies of everything inside, placed `d` away.
                let mut every = items;
                every.extend(
                    walls_inside(cx, &reg, visible_only)
                        .into_iter()
                        .map(ObjectRef::Wall),
                );
                let saved = std::mem::replace(&mut cx.selection.items, every);
                let clip = Clipboard::capture(cx);
                cx.selection.items = saved;
                n += clip.paste(cx, d, false).len();
            } else {
                let xf = match turn {
                    Some(a) => Xform::rotate(center_of(lo, hi), a),
                    None => Xform::translate(d),
                };
                let walls = move_wall_ends(&mut cx.project, fl, &reg, &xf);
                n += walls + transform::apply_xform(cx, &items, &xf).changed;
                details_view::follow_walls(&mut cx.project, fl, &walls_before);
            }
        });
    }
    crate::editor::placed::sync_distributions(cx);
    cx.mark_dirty();
    n
}

fn label(kind: AreaKind, turn: bool, copy: bool) -> &'static str {
    match (kind, turn, copy) {
        (AreaKind::StretchCad, _, _) => "Stretch CAD",
        (_, true, _) => "Rotate Edit Area",
        (_, _, true) => "Copy Edit Area",
        _ => "Move Edit Area",
    }
}

// ----- events -----

/// The press: starts the rubber band, a move or a turn, or ends the mode.
/// `None` while no area command is active.
pub fn pointer_down(cx: &mut EditorContext, p: &PointerEvent) -> Option<ToolResult> {
    let mut st = take()?;
    let kind = st.kind;
    let mut keep = true;
    match &mut st.stage {
        Stage::Band { start, current } => {
            *start = Some(p.world);
            *current = p.world;
        }
        Stage::Placed { lo, hi } => {
            let (lo, hi) = (*lo, *hi);
            let handle = rotate_handle(lo, hi, cx.px_per_in);
            let turn = matches!(kind, AreaKind::Edit { .. })
                && p.world.dist(handle) <= cx.pick_tol() * 1.5;
            if turn {
                cx.begin_change(label(kind, true, false));
                cx.typed_input.arm_angle();
                st.stage = Stage::Turning {
                    lo,
                    hi,
                    start: p.world,
                    original: Box::new(cx.project.clone()),
                    angle: 0.0,
                };
            } else if Region::new(lo, hi, &st.scope).contains(p.world) {
                let copy = matches!(kind, AreaKind::Edit { .. })
                    && (p.modifiers.ctrl || p.modifiers.command);
                cx.begin_change(label(kind, false, copy));
                cx.typed_input.arm();
                st.stage = Stage::Moving {
                    lo,
                    hi,
                    start: p.world,
                    original: Box::new(cx.project.clone()),
                    copy,
                    shift: Point::ZERO,
                };
            } else {
                keep = false;
                cx.status = "Edit area ended".into();
            }
        }
        // A press while a drag is under way (the button came up unseen).
        _ => {}
    }
    if keep {
        put(st);
    }
    Some(ToolResult::consumed())
}

/// The distance a move drag has gone, snapped to the grid unless a number is
/// typed or Alt is held.
fn move_delta(cx: &mut EditorContext, start: Point, p: &PointerEvent) -> Point {
    let raw = p.world;
    let world = typed_move_target(cx, start, raw);
    let mut d = world.unwrap_or(raw) - start;
    if world.is_none() && !p.modifiers.alt {
        let u = cx.snap_unit();
        d = Point::new(snap_unit_round(d.x, u), snap_unit_round(d.y, u));
    }
    let ti = &cx.typed_input;
    cx.readout = Some(format!(
        "Distance: {}   Angle: {}",
        if ti.has_text() && ti.field() == crate::editor::typed_input::TypedField::Length {
            format!("{}|", ti.length_text())
        } else {
            cx.fmt_dim(d.length())
        },
        if ti.has_text() && ti.field() == crate::editor::typed_input::TypedField::Angle {
            format!("{}|", ti.angle_text())
        } else {
            format!(
                "{:.1}\u{b0}",
                crate::editor::typed_input::angle_deg(Point::ZERO, d)
            )
        },
    ));
    d
}

/// The angle a turn drag has gone, in whole increments unless a number is
/// typed or Alt is held.
fn turn_angle(cx: &mut EditorContext, center: Point, start: Point, p: &PointerEvent) -> f64 {
    let typed = typed_rotate_target(cx, center, start);
    let to = typed.unwrap_or(p.world);
    let mut angle = (to - center).angle() - (start - center).angle();
    if typed.is_none() && !p.modifiers.alt {
        let inc = cx.defaults.grid.angle_snap_deg.max(1.0).to_radians();
        angle = (angle / inc).round() * inc;
    }
    let ti = &cx.typed_input;
    cx.readout = Some(format!(
        "Rotate: {}",
        if ti.has_text() {
            format!("{}|", ti.angle_text())
        } else {
            format!("{:.1}\u{b0}", angle.to_degrees())
        }
    ));
    angle
}

/// Re-applies the drag in progress for the pointer `p`.
fn drag_to(cx: &mut EditorContext, st: &mut State, p: &PointerEvent) {
    let kind = st.kind;
    let scope = st.scope.clone();
    match &mut st.stage {
        Stage::Moving {
            lo,
            hi,
            start,
            original,
            copy,
            shift,
        } => {
            let d = move_delta(cx, *start, p);
            *shift = d;
            apply(cx, original, kind, (*lo, *hi), &scope, d, None, *copy);
        }
        Stage::Turning {
            lo,
            hi,
            start,
            original,
            angle,
        } => {
            let center = center_of(*lo, *hi);
            let a = turn_angle(cx, center, *start, p);
            *angle = a;
            apply(
                cx,
                original,
                kind,
                (*lo, *hi),
                &scope,
                Point::ZERO,
                Some(a),
                false,
            );
        }
        _ => {}
    }
}

/// Moves the pointer: stretches the rubber band or carries the drag. True
/// when an area command took the event.
pub fn pointer_move(cx: &mut EditorContext, p: &PointerEvent) -> bool {
    let Some(mut st) = take() else {
        return false;
    };
    match &mut st.stage {
        Stage::Band {
            start: Some(_),
            current,
        } if p.down => *current = p.world,
        Stage::Moving { .. } | Stage::Turning { .. } if p.down => drag_to(cx, &mut st, p),
        _ => {}
    }
    put(st);
    true
}

/// The release: places the region, or ends the drag as one undo step.
pub fn pointer_up(cx: &mut EditorContext, p: &PointerEvent) -> Option<ToolResult> {
    let mut st = take()?;
    let kind = st.kind;
    let mut result = ToolResult::consumed();
    let mut keep = true;
    match std::mem::replace(
        &mut st.stage,
        Stage::Band {
            start: None,
            current: Point::ZERO,
        },
    ) {
        Stage::Band { start: Some(a), .. } => {
            let size = (p.world - a).length() * cx.px_per_in;
            if size >= 6.0 {
                let lo = Point::new(a.x.min(p.world.x), a.y.min(p.world.y));
                let hi = Point::new(a.x.max(p.world.x), a.y.max(p.world.y));
                st.stage = Stage::Placed { lo, hi };
                cx.status = match kind {
                    AreaKind::StretchCad => "Stretch CAD: drag inside the window".into(),
                    _ => "Edit area: drag to move, the handle to rotate, Delete removes, Esc ends"
                        .into(),
                };
            }
        }
        Stage::Moving {
            lo,
            hi,
            start,
            original,
            copy,
            shift,
        } => {
            // The last position of the pointer counts.
            let mut tmp = State {
                kind,
                scope: st.scope.clone(),
                stage: Stage::Moving {
                    lo,
                    hi,
                    start,
                    original: original.clone(),
                    copy,
                    shift,
                },
            };
            drag_to(cx, &mut tmp, p);
            let shift = match &tmp.stage {
                Stage::Moving { shift, .. } => *shift,
                _ => shift,
            };
            cx.typed_input.disarm();
            cx.readout = None;
            if original.to_json().ok() == cx.project.to_json().ok() {
                cx.cancel_change();
                st.stage = Stage::Placed { lo, hi };
            } else {
                result = ToolResult::committed(label(kind, false, copy));
                if copy || kind == AreaKind::StretchCad {
                    keep = false;
                } else {
                    st.stage = Stage::Placed {
                        lo: lo + shift,
                        hi: hi + shift,
                    };
                    if let Some(q) = st.scope.poly.as_mut() {
                        q.iter_mut().for_each(|c| *c = *c + shift);
                    }
                }
            }
        }
        Stage::Turning {
            lo,
            hi,
            start,
            original,
            angle,
        } => {
            let mut tmp = State {
                kind,
                scope: st.scope.clone(),
                stage: Stage::Turning {
                    lo,
                    hi,
                    start,
                    original: original.clone(),
                    angle,
                },
            };
            drag_to(cx, &mut tmp, p);
            cx.typed_input.disarm();
            cx.readout = None;
            if original.to_json().ok() == cx.project.to_json().ok() {
                cx.cancel_change();
                st.stage = Stage::Placed { lo, hi };
            } else {
                // The turned region is no longer a rectangle: the mode ends.
                result = ToolResult::committed(label(kind, true, false));
                keep = false;
            }
        }
        other => st.stage = other,
    }
    if keep {
        put(st);
    } else {
        cx.status.clear();
    }
    Some(result)
}

/// Keys: Esc, Delete and the typed number of a drag in progress.
pub fn key(cx: &mut EditorContext, k: &KeyEvent) -> Option<ToolResult> {
    let mut st = take()?;
    let kind = st.kind;
    let dragging = matches!(st.stage, Stage::Moving { .. } | Stage::Turning { .. });
    if dragging {
        let res = cx.typed_input.handle(k.key, k.text.as_deref());
        if res != TypedKey::Ignored {
            let at = cx.cursor_world.unwrap_or(Point::ZERO);
            let p = PointerEvent::at(cx, at).with_down(true);
            drag_to(cx, &mut st, &p);
            if res == TypedKey::Commit {
                put(st);
                let up = PointerEvent::at(cx, at);
                return pointer_up(cx, &up);
            }
            put(st);
            return Some(ToolResult::consumed());
        }
    }
    if k.is(Key::Escape) {
        put(st);
        cancel(cx);
        cx.status.clear();
        return Some(ToolResult::consumed());
    }
    if (k.is(Key::Delete) || k.is(Key::Backspace)) && !dragging {
        if let (Stage::Placed { lo, hi }, AreaKind::Edit { visible_only }) = (&st.stage, kind) {
            let (lo, hi) = (*lo, *hi);
            let scope = st.scope.clone();
            let reg = Region::new(lo, hi, &scope);
            let mut found = 0;
            each_floor(cx, scope.all_floors, |cx| {
                let mut all = region_items(cx, &reg, visible_only, &scope);
                all.extend(
                    walls_inside(cx, &reg, visible_only)
                        .into_iter()
                        .map(ObjectRef::Wall),
                );
                if !all.is_empty() {
                    found += all.len();
                    cx.selection.items = all;
                    cx.delete_selection();
                }
            });
            if found == 0 {
                cx.status = "Nothing inside the edit area".into();
                put(st);
                return Some(ToolResult::consumed());
            }
            put(st);
            return Some(ToolResult::committed("Delete Edit Area"));
        }
    }
    put(st);
    // Other keys are swallowed while a rubber band is wanted; once the region
    // is placed they behave as usual.
    None
}

/// The rubber band, the region with its Rotate handle and the live outline of
/// a move or turn.
pub fn draw_overlay(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let (kind, band, placed, shift, angle, poly) = STATE.with(|s| {
        let b = s.borrow();
        let Some(st) = b.as_ref() else {
            return (None, None, None, Point::ZERO, 0.0, None);
        };
        let mut band = None;
        let mut placed = None;
        let mut shift = Point::ZERO;
        let mut angle = 0.0;
        match &st.stage {
            Stage::Band {
                start: Some(a),
                current,
            } => band = Some((*a, *current)),
            Stage::Placed { lo, hi } => placed = Some((*lo, *hi)),
            Stage::Moving {
                lo, hi, shift: d, ..
            } => {
                placed = Some((*lo, *hi));
                shift = *d;
            }
            Stage::Turning {
                lo, hi, angle: a, ..
            } => {
                placed = Some((*lo, *hi));
                angle = *a;
            }
            _ => {}
        }
        (
            Some(st.kind),
            band,
            placed,
            shift,
            angle,
            st.scope.poly.clone(),
        )
    });
    let Some(kind) = kind else { return };
    let col = cx.palette.selection;
    if let Some((a, b)) = band {
        let r = Rect::from_two_pos(cam.world_to_screen(a), cam.world_to_screen(b));
        painter.add(Shape::rect_filled(r, 0.0, col.gamma_multiply(0.10)));
        painter.rect_stroke(r, 0.0, Stroke::new(1.5_f32, col), egui::StrokeKind::Inside);
    }
    if let Some((lo, hi)) = placed {
        let center = center_of(lo, hi);
        let xf = Xform::rotate(center, angle);
        let outline: Vec<Point> = match poly {
            Some(q) => q,
            None => rect_corners(lo, hi).to_vec(),
        };
        let pts: Vec<Pos2> = outline
            .iter()
            .map(|c| cam.world_to_screen(xf.apply(*c) + shift))
            .collect();
        painter.add(Shape::convex_polygon(
            pts.clone(),
            col.gamma_multiply(0.07),
            Stroke::new(1.5_f32, col),
        ));
        if matches!(kind, AreaKind::Edit { .. }) {
            let h = xf.apply(rotate_handle(lo, hi, cx.px_per_in)) + shift;
            let top = xf.apply(Point::new(center.x, hi.y)) + shift;
            painter.line_segment(
                [cam.world_to_screen(top), cam.world_to_screen(h)],
                Stroke::new(1.0_f32, col),
            );
            painter.circle_filled(cam.world_to_screen(h), 5.0, col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::tools::select::SelectTool;
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        cancel_state();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.px_per_in = 2.0;
        cx
    }

    fn cancel_state() {
        take();
    }

    fn ev(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(cx, Point::new(x, y))
    }

    fn down(t: &mut SelectTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = ev(cx, x, y).with_down(true);
        t.pointer_down(cx, e)
    }

    fn mv(t: &mut SelectTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = ev(cx, x, y).with_down(true);
        t.pointer_move(cx, e)
    }

    fn up(t: &mut SelectTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = ev(cx, x, y);
        t.pointer_up(cx, e)
    }

    fn drag(t: &mut SelectTool, cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) {
        down(t, cx, a.0, a.1);
        mv(t, cx, b.0, b.1);
        up(t, cx, b.0, b.1);
    }

    /// A 20' x 10' box of walls and a free CAD line inside it.
    fn house(cx: &mut EditorContext) -> [Id; 4] {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        let mut ids = [0; 4];
        for i in 0..4 {
            ids[i] = cx
                .project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        ids
    }

    #[test]
    fn rubber_band_then_move_stretches_walls_that_cross_the_edge() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        assert!(active());
        // The band covers the right half of the house plus a margin: x 100..300.
        drag(&mut t, &mut cx, (100.0, -40.0), (300.0, 160.0));
        let (lo, hi) = region().expect("the region is placed");
        assert_eq!((lo.x, lo.y, hi.x, hi.y), (100.0, -40.0, 300.0, 160.0));
        // Move the region 48" to the right.
        drag(&mut t, &mut cx, (200.0, 60.0), (248.0, 60.0));
        let f = cx.floor();
        let bottom = f.wall(ids[0]).unwrap();
        assert_eq!(bottom.start, Point::new(0.0, 0.0), "the end outside stays");
        assert_eq!(bottom.end, Point::new(288.0, 0.0), "the end inside moves");
        let right = f.wall(ids[1]).unwrap();
        assert_eq!(right.start, Point::new(288.0, 0.0));
        assert_eq!(right.end, Point::new(288.0, 120.0));
        assert_eq!(f.wall(ids[3]).unwrap().start, Point::new(0.0, 120.0));
        // One undo step undoes the whole move.
        cx.undo();
        assert_eq!(cx.floor().wall(ids[0]).unwrap().end, Point::new(240.0, 0.0));
        // The region moved with the contents.
        assert!(active());
    }

    #[test]
    fn a_wall_inside_the_edit_area_turns_with_it_and_openings_keep_their_place() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let door = cx
            .project
            .add_opening(0, ids[0], 60.0, plan_core::OpeningKind::Door)
            .unwrap();
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        // Only the left end of the bottom wall and the left wall are inside.
        drag(&mut t, &mut cx, (-40.0, -40.0), (80.0, 160.0));
        // Drag the Rotate handle a quarter turn about the region's center.
        let (lo, hi) = region().unwrap();
        let h = rotate_handle(lo, hi, cx.px_per_in);
        let center = center_of(lo, hi);
        let to = center + (h - center).perp();
        drag(&mut t, &mut cx, (h.x, h.y), (to.x, to.y));
        let w = cx.floor().wall(ids[0]).unwrap();
        // Its start turned, its end stayed: the wall no longer lies along x.
        assert_eq!(w.end, Point::new(240.0, 0.0));
        assert!((w.start.x - 0.0).abs() > 1.0 || (w.start.y - 0.0).abs() > 1.0);
        assert!(cx.floor().openings.iter().any(|o| o.id == door));
        // Turning ends the mode.
        assert!(!active());
    }

    #[test]
    fn ctrl_drag_inside_the_edit_area_copies_everything_inside() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        drag(&mut t, &mut cx, (-20.0, -20.0), (260.0, 140.0));
        let mut down = ev(&cx, 100.0, 60.0).with_down(true);
        down.modifiers.ctrl = true;
        let _ = t.pointer_down(&mut cx, down);
        mv(&mut t, &mut cx, 100.0, 260.0);
        let upr = up(&mut t, &mut cx, 100.0, 260.0);
        assert_eq!(upr.commit.as_deref(), Some("Copy Edit Area"));
        assert_eq!(cx.floor().walls.len(), 8);
        assert_eq!(cx.floor().wall(ids[0]).unwrap().start, Point::new(0.0, 0.0));
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 4);
    }

    #[test]
    fn delete_removes_what_lies_wholly_inside() {
        let mut cx = cx();
        house(&mut cx);
        let line = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(300.0, 0.0),
                b: Point::new(340.0, 0.0),
            },
        );
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        drag(&mut t, &mut cx, (-20.0, -20.0), (260.0, 140.0));
        let res = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(res.commit.as_deref(), Some("Delete Edit Area"));
        assert!(cx.floor().walls.is_empty());
        assert!(cx.floor().cad.iter().any(|c| c.id == line), "outside stays");
    }

    #[test]
    fn stretch_cad_moves_only_the_vertices_inside_the_window() {
        let mut cx = cx();
        let line = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(100.0, 0.0),
            },
        );
        let poly = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 50.0),
                    Point::new(100.0, 50.0),
                    Point::new(100.0, 90.0),
                ],
                closed: false,
            },
        );
        let mut t = SelectTool::default();
        begin(&mut cx, AreaKind::StretchCad);
        // A window around the right-hand ends.
        drag(&mut t, &mut cx, (70.0, -20.0), (130.0, 110.0));
        drag(&mut t, &mut cx, (100.0, 20.0), (130.0, 20.0));
        let f = cx.floor();
        let CadItem::Line { a, b } = &f.cad.iter().find(|c| c.id == line).unwrap().item else {
            panic!()
        };
        assert_eq!((*a, *b), (Point::new(0.0, 0.0), Point::new(130.0, 0.0)));
        let CadItem::Polyline { points, .. } = &f.cad.iter().find(|c| c.id == poly).unwrap().item
        else {
            panic!()
        };
        assert_eq!(points[0], Point::new(0.0, 50.0));
        assert_eq!(points[1], Point::new(130.0, 50.0));
        assert_eq!(points[2], Point::new(130.0, 90.0));
        // One-shot: the mode is over and one undo restores the lines.
        assert!(!active());
        cx.undo();
        let CadItem::Line { b, .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        assert_eq!(*b, Point::new(100.0, 0.0));
    }

    #[test]
    fn typed_distance_moves_the_edit_area_exactly() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        drag(&mut t, &mut cx, (100.0, -40.0), (300.0, 160.0));
        down(&mut t, &mut cx, 200.0, 60.0);
        mv(&mut t, &mut cx, 212.0, 60.0);
        cx.cursor_world = Some(Point::new(212.0, 60.0));
        for ch in ["1", "0", "'"] {
            let r = t.key(&mut cx, KeyEvent::text(ch));
            assert!(r.consumed, "{ch} goes to the number");
        }
        assert!(cx.readout.as_deref().unwrap().contains("10'"));
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(r.commit.is_some());
        assert_eq!(cx.floor().wall(ids[0]).unwrap().end, Point::new(360.0, 0.0));
    }

    #[test]
    fn escape_cancels_a_drag_then_ends_the_mode() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let mut t = SelectTool::default();
        begin(&mut cx, AreaKind::Edit { visible_only: true });
        drag(&mut t, &mut cx, (100.0, -40.0), (300.0, 160.0));
        down(&mut t, &mut cx, 200.0, 60.0);
        mv(&mut t, &mut cx, 260.0, 60.0);
        assert_eq!(cx.floor().wall(ids[0]).unwrap().end, Point::new(300.0, 0.0));
        let _ = t.key(&mut cx, KeyEvent::escape());
        assert!(!active(), "Esc ends the mode and the drag");
        assert_eq!(cx.floor().wall(ids[0]).unwrap().end, Point::new(240.0, 0.0));
        assert!(!cx.can_undo() || cx.undo_label() != Some("Move Edit Area"));
    }

    #[test]
    fn a_press_outside_the_region_ends_the_mode() {
        let mut cx = cx();
        house(&mut cx);
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        drag(&mut t, &mut cx, (100.0, -40.0), (300.0, 160.0));
        assert!(active());
        down(&mut t, &mut cx, -500.0, -500.0);
        assert!(!active());
    }
}
