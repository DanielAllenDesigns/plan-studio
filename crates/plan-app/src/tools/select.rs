//! Select Objects (S-1..S-38 in `docs/parity/select-and-edit.md`).
//!
//! * click selects (Shift adds/removes), click on empty space deselects;
//!   Tab cycles through the objects under the pointer;
//! * dragging a wall (body or middle handle) is Chief's perpendicular move:
//!   connected walls keep their directions and stretch, openings travel with
//!   it; Alt moves it freely and connected ends follow;
//! * the end handles stretch a wall with snapping, connected ends follow, and
//!   dropping an end on another wall splits that wall (T-junction);
//! * dragging an opening slides it along its wall (or onto another wall);
//! * CAD items: Move, Rotate, line/vertex/radius handles;
//! * drag on empty space is a marquee: left-to-right selects what it
//!   encloses, right-to-left what it touches;
//! * clicking a temporary dimension value edits it; Enter moves the object;
//! * every drag is one undo step, Esc cancels it;
//! * double-click or Enter opens the specification; Delete deletes.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::handles::{self, hit_handle, HandleKind};
use crate::editor::ops::{self, cad_center, JOIN_TOL};
use crate::editor::rooms_edit;
use crate::editor::selection::{extra_in_rect, hit_test_cx, layer_of};
use crate::editor::snap::snap_to_grid;
use crate::editor::stairs_view::{self, StairHandleKind};
use crate::editor::{placed, roof_view, site_view, tempdim};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, EditorRequest, ObjectRef};
use crate::shell::view3d_panel::{Outbox, ViewRequest};
use crate::toolbar::ViewFlag;
use crate::tools::camera::{self as camera_tool, CamHandle};
use eframe::egui::{self, Key, Pos2, Rect, Shape, Stroke};
use plan_core::cad::CadItem;
use plan_core::geometry::{
    dist_to_segment, point_in_polygon, project_on_segment, segment_intersection, Point,
};
use plan_core::{Id, OpeningKind, Project, WallEnd};

/// Pixels the pointer must travel before a press becomes a drag (S-26).
const DRAG_THRESHOLD_PX: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
enum Op {
    /// Perpendicular move of one wall (free with Alt).
    WallMove(Id),
    WallEnd(Id, WallEnd),
    OpeningSlide(Id),
    /// Offset of a dimension line.
    DimOffset(Id),
    CadRotate(Id),
    /// Line end, polyline vertex or circle radius.
    CadVertex(Id, HandleKind),
    /// Plain translate of the whole selection.
    Group,
    /// Click-only handle: flip the door swing.
    Swing(Id),
    /// A handle (or the body, `Move`) of a stair.
    Stair(Id, StairHandleKind),
    Cabinet(Id, HandleKind),
    Symbol(Id, HandleKind),
    DeviceMove(Id),
    RoofMove(Id),
    RoofVertex(Id, usize),
    Camera(Id, CamHandle),
}

impl Op {
    fn label(self) -> &'static str {
        match self {
            Op::WallMove(_) => "Move Wall",
            Op::WallEnd(..) => "Stretch Wall",
            Op::OpeningSlide(_) | Op::Swing(_) => "Move Opening",
            Op::DimOffset(_) => "Move Dimension",
            Op::CadRotate(_) => "Rotate",
            Op::CadVertex(..) => "Reshape",
            Op::Group => "Move Objects",
            Op::Stair(_, k) => stairs_view::drag_label(k),
            Op::Cabinet(..) => "Edit Cabinet",
            Op::Symbol(..) => "Edit Symbol",
            Op::DeviceMove(_) => "Move Device",
            Op::RoofMove(_) => "Move Roof Plane",
            Op::RoofVertex(..) => "Reshape Roof Plane",
            Op::Camera(..) => "Edit Camera",
        }
    }
}

struct Active {
    op: Op,
    original: Project,
    start: Point,
    /// Walls the snap engine ignores (the dragged wall and its joined ones).
    exclude: Vec<Id>,
}

enum Drag {
    None,
    Armed {
        op: Op,
        start: Point,
        screen: Pos2,
    },
    Active(Box<Active>),
    Marquee {
        start: Point,
        current: Point,
        screen: Pos2,
        add: bool,
    },
}

pub struct SelectTool {
    drag: Drag,
    cursor: egui::CursorIcon,
    /// The room under the press when it landed on empty floor (R-16).
    room_click: Option<usize>,
}

impl Default for SelectTool {
    fn default() -> Self {
        Self {
            drag: Drag::None,
            cursor: egui::CursorIcon::Default,
            room_click: None,
        }
    }
}

fn snap_unit_round(v: f64, unit: f64) -> f64 {
    (v / unit).round() * unit
}

// ----- the move operations (shared by drags and arrow-key nudges) -----

/// Moves a wall by `delta`: perpendicular only, or freely with `free`.
fn move_wall(cx: &mut EditorContext, id: Id, delta: Point, free: bool) {
    let fl = cx.floor;
    let unit = cx.snap_unit();
    if free {
        let d = Point::new(
            snap_unit_round(delta.x, unit),
            snap_unit_round(delta.y, unit),
        );
        ops::translate_walls_with_followers(&mut cx.project, fl, &[id], d);
    } else if let Some(w) = cx.floor().wall(id) {
        let s = snap_unit_round(delta.dot(w.normal()), unit);
        if s.abs() > 1e-9 {
            ops::move_wall_perpendicular(&mut cx.project, fl, id, s);
        }
    }
}

/// Slides an opening along its wall by `delta`.
fn slide_opening_by(cx: &mut EditorContext, id: Id, delta: Point) {
    let fl = cx.floor;
    let unit = cx.snap_unit();
    let Some(o) = cx.floor().openings.iter().find(|o| o.id == id).cloned() else {
        return;
    };
    let Some(dir) = cx.floor().wall(o.wall_id).map(|w| w.direction()) else {
        return;
    };
    let center = snap_unit_round(o.center_offset + delta.dot(dir), unit);
    ops::place_opening_at(&mut cx.project, fl, id, o.wall_id, center);
}

/// Translates every selected object by `delta`.
fn move_group(cx: &mut EditorContext, items: &[ObjectRef], delta: Point) {
    let fl = cx.floor;
    let unit = cx.snap_unit();
    let d = Point::new(
        snap_unit_round(delta.x, unit),
        snap_unit_round(delta.y, unit),
    );
    let walls: Vec<Id> = items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect();
    ops::translate_walls_with_followers(&mut cx.project, fl, &walls, d);
    cx.translate_extra(items, d);
    for o in items {
        match *o {
            ObjectRef::Opening(id) => {
                let host_moves = cx
                    .floor()
                    .openings
                    .iter()
                    .find(|x| x.id == id)
                    .is_some_and(|x| walls.contains(&x.wall_id));
                if !host_moves {
                    slide_opening_by(cx, id, delta);
                }
            }
            ObjectRef::Dimension(id) => {
                if let Some(dim) = cx.project.floors[fl]
                    .dimensions
                    .iter_mut()
                    .find(|x| x.id == id)
                {
                    dim.start = dim.start + d;
                    dim.end = dim.end + d;
                }
            }
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                    ops::translate_cad(&mut c.item, d);
                }
            }
            _ => {}
        }
    }
}

fn single_op_for_body(o: ObjectRef) -> Op {
    match o {
        ObjectRef::Wall(id) => Op::WallMove(id),
        ObjectRef::Opening(id) => Op::OpeningSlide(id),
        ObjectRef::Dimension(id) => Op::DimOffset(id),
        ObjectRef::Stair(id) => Op::Stair(id, StairHandleKind::Move),
        ObjectRef::Cabinet(id) => Op::Cabinet(id, HandleKind::Move),
        ObjectRef::Symbol(id) => Op::Symbol(id, HandleKind::Move),
        ObjectRef::Device(id) => Op::DeviceMove(id),
        ObjectRef::RoofPlane(id) => Op::RoofMove(id),
        ObjectRef::Camera(id) => Op::Camera(id, CamHandle::Move),
        _ => Op::Group,
    }
}

/// Selects the object a click landed on; a room goes to the room selection.
fn select_hit(cx: &mut EditorContext, o: ObjectRef) {
    match o {
        ObjectRef::Room(i) => rooms_edit::select_room(cx, i),
        other => {
            rooms_edit::clear_room_selection();
            cx.selection.set(other);
        }
    }
}

// ----- marquee -----

fn rect_corners(lo: Point, hi: Point) -> [Point; 4] {
    [lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)]
}

fn in_rect(p: Point, lo: Point, hi: Point) -> bool {
    p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
}

/// Does the polygon touch the rectangle?
fn poly_touches_rect(poly: &[Point], lo: Point, hi: Point) -> bool {
    if poly.iter().any(|p| in_rect(*p, lo, hi)) {
        return true;
    }
    let corners = rect_corners(lo, hi);
    if corners.iter().any(|c| point_in_polygon(*c, poly)) {
        return true;
    }
    (0..poly.len()).any(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        (0..4).any(|k| segment_intersection(a, b, corners[k], corners[(k + 1) % 4]).is_some())
    })
}

fn objects_in_rect(cx: &EditorContext, a: Point, b: Point) -> Vec<ObjectRef> {
    let crossing = b.x < a.x;
    let lo = Point::new(a.x.min(b.x), a.y.min(b.y));
    let hi = Point::new(a.x.max(b.x), a.y.max(b.y));
    let floor = cx.floor();
    let hit = |poly: &[Point]| {
        if crossing {
            poly_touches_rect(poly, lo, hi)
        } else {
            poly.iter().all(|p| in_rect(*p, lo, hi))
        }
    };
    let usable = |o: ObjectRef| {
        layer_of(floor, o).is_none_or(|l| cx.layers().is_visible(&l) && !cx.layers().is_locked(&l))
    };
    let mut out = Vec::new();
    for w in &floor.walls {
        let r = ObjectRef::Wall(w.id);
        if usable(r) && hit(&w.footprint()) {
            out.push(r);
        }
        for o in floor.openings_on(w.id) {
            let n = w.normal().scale(w.thickness * 0.5);
            let (pa, pb) = (w.point_at(o.start_offset()), w.point_at(o.end_offset()));
            let r = ObjectRef::Opening(o.id);
            if usable(r) && hit(&[pa.add(n), pb.add(n), pb.sub(n), pa.sub(n)]) {
                out.push(r);
            }
        }
    }
    for d in &floor.dimensions {
        let (p, q) = d.line_points();
        let r = ObjectRef::Dimension(d.id);
        let ok = if crossing {
            poly_touches_rect(&[p, q], lo, hi)
        } else {
            [d.start, d.end, p, q].iter().all(|x| in_rect(*x, lo, hi))
        };
        if usable(r) && ok {
            out.push(r);
        }
    }
    for c in &floor.cad {
        let (blo, bhi) = c.bounds();
        let r = ObjectRef::Cad(c.id);
        let ok = if crossing {
            blo.x <= hi.x && bhi.x >= lo.x && blo.y <= hi.y && bhi.y >= lo.y
        } else {
            in_rect(blo, lo, hi) && in_rect(bhi, lo, hi)
        };
        if usable(r) && ok {
            out.push(r);
        }
    }
    out.extend(extra_in_rect(cx, lo, hi, crossing));
    out
}

impl SelectTool {
    fn update_hover(&mut self, cx: &mut EditorContext, p: &PointerEvent) {
        let tol = cx.pick_tol();
        let hs = handles::handles_for(cx, cx.px_per_in);
        if let Some(h) = hit_handle(&hs, p.world, tol) {
            self.cursor = h.cursor;
            cx.hover = None;
            return;
        }
        let top = hit_test_cx(cx, p.world, tol).first().copied();
        self.cursor = if top.is_some() {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::Default
        };
        cx.hover = top;
    }

    /// The drag a press at `at` starts on a handle of the selected object.
    fn handle_op(cx: &EditorContext, at: Point, tol: f64) -> Option<Op> {
        match cx.selection.single()? {
            ObjectRef::Stair(id) => {
                let o = stairs_view::find(cx.floor(), id)?;
                let hs = stairs_view::handles(&o, cx.px_per_in);
                stairs_view::hit_handle(&hs, at, tol).map(|h| Op::Stair(id, h.kind))
            }
            ObjectRef::Camera(id) => {
                camera_tool::hit_handle(cx.project.camera(id)?, at, tol).map(|h| Op::Camera(id, h))
            }
            _ => {
                let hs = handles::handles_for(cx, cx.px_per_in);
                hit_handle(&hs, at, tol)
                    .as_ref()
                    .and_then(Self::op_for_handle)
            }
        }
    }

    /// The operation a handle starts, if it is a draggable one.
    fn op_for_handle(h: &handles::Handle) -> Option<Op> {
        Some(match (h.target, h.kind) {
            (ObjectRef::Cabinet(id), k) => Op::Cabinet(id, k),
            (ObjectRef::Symbol(id), k) => Op::Symbol(id, k),
            (ObjectRef::Device(id), HandleKind::Move) => Op::DeviceMove(id),
            (ObjectRef::RoofPlane(id), HandleKind::Move) => Op::RoofMove(id),
            (ObjectRef::RoofPlane(id), HandleKind::Reshape(i)) => Op::RoofVertex(id, i),
            (ObjectRef::Wall(id), HandleKind::ResizeStart) => Op::WallEnd(id, WallEnd::Start),
            (ObjectRef::Wall(id), HandleKind::ResizeEnd) => Op::WallEnd(id, WallEnd::End),
            (ObjectRef::Wall(id), HandleKind::PerpendicularMove) => Op::WallMove(id),
            (ObjectRef::Opening(id), HandleKind::PerpendicularMove) => Op::OpeningSlide(id),
            (ObjectRef::Opening(id), HandleKind::Swing) => Op::Swing(id),
            (ObjectRef::Dimension(id), HandleKind::PerpendicularMove) => Op::DimOffset(id),
            (ObjectRef::Cad(_) | ObjectRef::Text(_), HandleKind::Move) => Op::Group,
            (ObjectRef::Cad(id) | ObjectRef::Text(id), HandleKind::Rotate) => Op::CadRotate(id),
            (
                ObjectRef::Cad(id) | ObjectRef::Text(id),
                k @ (HandleKind::ResizeStart | HandleKind::ResizeEnd | HandleKind::Reshape(_)),
            ) => Op::CadVertex(id, k),
            _ => return None,
        })
    }

    /// Re-applies `a`'s operation for the pointer position, starting from the
    /// original project (so the result never accumulates error).
    fn apply(&self, cx: &mut EditorContext, a: &Active, p: &PointerEvent) {
        cx.project = a.original.clone();
        let fl = cx.floor;
        let alt = p.modifiers.alt;
        let total = p.world - a.start;
        match a.op {
            Op::WallMove(id) => move_wall(cx, id, total, alt),
            Op::WallEnd(id, end) => {
                let Some(w) = cx.floor().wall(id).cloned() else {
                    return;
                };
                let fixed = if end == WallEnd::Start {
                    w.end
                } else {
                    w.start
                };
                let s = cx.snap_at(p.world, Some(fixed), alt, &a.exclude);
                cx.last_snap = Some(s);
                if s.point.dist(fixed) >= 1.0 {
                    ops::move_wall_end_joined(&mut cx.project, fl, id, end, s.point);
                }
            }
            Op::OpeningSlide(id) => {
                let unit = cx.snap_unit();
                let Some(o) = cx.floor().openings.iter().find(|o| o.id == id).cloned() else {
                    return;
                };
                // Over another wall: re-host there (DW-18).
                let tol = cx.pick_tol();
                let other = cx
                    .floor()
                    .walls
                    .iter()
                    .filter(|w| w.id != o.wall_id && cx.layers().is_visible(&w.layer))
                    .map(|w| (w, dist_to_segment(p.world, w.start, w.end)))
                    .filter(|(_, d)| *d <= tol)
                    .min_by(|x, y| x.1.total_cmp(&y.1))
                    .map(|(w, _)| (w.id, w.length(), w.start, w.end));
                if let Some((wid, len, s0, s1)) = other {
                    let (t, _) = project_on_segment(p.world, s0, s1);
                    let center = snap_unit_round(t * len, unit);
                    ops::place_opening_at(&mut cx.project, fl, id, wid, center);
                } else {
                    slide_opening_by(cx, id, total);
                }
            }
            Op::DimOffset(id) => {
                let unit = cx.snap_unit();
                if let Some(d) = cx.project.floors[fl]
                    .dimensions
                    .iter_mut()
                    .find(|d| d.id == id)
                {
                    let n = d.end.sub(d.start).normalized().perp();
                    d.offset = snap_unit_round(p.world.sub(d.start).dot(n), unit);
                }
            }
            Op::CadRotate(id) => {
                let Some(c) = a.original.floors[fl].cad.iter().find(|c| c.id == id) else {
                    return;
                };
                let center = cad_center(&c.item);
                let mut angle = p.world.sub(center).angle() - a.start.sub(center).angle();
                if !alt {
                    let inc = cx.defaults.grid.angle_snap_deg.max(1.0).to_radians();
                    angle = (angle / inc).round() * inc;
                }
                if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                    ops::rotate_cad(&mut c.item, center, angle);
                }
            }
            Op::CadVertex(id, kind) => {
                let to = cx.snap_at(p.world, None, alt, &[]).point;
                if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
                    match (&mut c.item, kind) {
                        (CadItem::Line { a, .. }, HandleKind::ResizeStart) => *a = to,
                        (CadItem::Line { b, .. }, HandleKind::ResizeEnd) => *b = to,
                        (CadItem::Circle { center, radius }, HandleKind::ResizeEnd) => {
                            *radius = center.dist(to).max(0.5);
                        }
                        (CadItem::Polyline { points, .. }, HandleKind::Reshape(i)) => {
                            if let Some(v) = points.get_mut(i) {
                                *v = to;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Op::Group => {
                let items = cx.selection.items.clone();
                move_group(cx, &items, total);
            }
            Op::Swing(_) => {}
            Op::Stair(id, kind) => {
                if let Some(orig) = stairs_view::find(&a.original.floors[fl], id) {
                    let to = match kind {
                        StairHandleKind::Rotate | StairHandleKind::Run => {
                            cx.snap_at(p.world, Some(orig.bottom_center()), alt, &[])
                                .point
                        }
                        _ => p.world,
                    };
                    let mut n = stairs_view::drag_handle(&orig, kind, a.start, to);
                    if kind == StairHandleKind::Move && !alt {
                        n.stair.origin = snap_to_grid(n.stair.origin, cx.snap_unit());
                    }
                    stairs_view::update(&mut cx.project, fl, id, |o| *o = n);
                }
            }
            Op::Cabinet(id, kind) => {
                if let Some(orig) = placed::cabinet_by_id(&a.original.floors[fl], id) {
                    let c = crate::tools::cabinet::apply_edit(cx, kind, &orig, a.start, p);
                    placed::replace_cabinet(&mut cx.project, fl, &c);
                }
            }
            Op::Symbol(id, kind) => {
                if let Some(orig) = a.original.floors[fl].symbol(id).cloned() {
                    let s = crate::tools::library::apply_drag(cx, kind, &orig, a.start, p);
                    if let Some(slot) = cx.project.floors[fl]
                        .symbols
                        .iter_mut()
                        .find(|x| x.id == id)
                    {
                        *slot = s;
                    }
                }
            }
            Op::DeviceMove(id) => {
                let unit = cx.snap_unit();
                let mut layer = site_view::load_electrical(&a.original.floors[fl]);
                if let Some(d) = layer.device_mut(id) {
                    match d.wall_id.and_then(|w| cx.floor().wall(w)).cloned() {
                        Some(w) => site_view::slide_on_wall(d, &w, p.world, unit),
                        None => {
                            d.position = Point::new(
                                snap_unit_round(d.position.x + total.x, unit),
                                snap_unit_round(d.position.y + total.y, unit),
                            );
                        }
                    }
                }
                site_view::save_electrical(&mut cx.project, fl, &layer);
            }
            Op::RoofMove(id) => {
                let unit = cx.snap_unit();
                let mut set = roof_view::load(&a.original.floors[fl]);
                if let Some(r) = set.plane_mut(id) {
                    r.translate(Point::new(
                        snap_unit_round(total.x, unit),
                        snap_unit_round(total.y, unit),
                    ));
                    roof_view::store(&mut cx.project, fl, &mut set);
                }
            }
            Op::RoofVertex(id, i) => {
                let to = cx.snap_at(p.world, None, alt, &[]).point;
                let mut set = roof_view::load(&a.original.floors[fl]);
                if let Some(r) = set.plane_mut(id) {
                    r.move_vertex(i, to);
                    roof_view::store(&mut cx.project, fl, &mut set);
                }
            }
            Op::Camera(id, h) => {
                if let Some(orig) = a.original.camera(id).cloned() {
                    let unit = cx.snap_unit();
                    let to = if h == CamHandle::Move {
                        Point::new(
                            snap_unit_round(orig.position.x + total.x, unit),
                            snap_unit_round(orig.position.y + total.y, unit),
                        )
                    } else {
                        p.world
                    };
                    cx.project.update_camera(id, |c| {
                        *c = orig.clone();
                        camera_tool::apply_handle(c, h, to, p.modifiers.shift);
                    });
                }
            }
        }
        cx.mark_dirty();
    }

    fn start_drag(&mut self, cx: &mut EditorContext, op: Op, start: Point) {
        let mut exclude = Vec::new();
        if let Op::WallEnd(id, end) = op {
            exclude.push(id);
            if let Some(w) = cx.floor().wall(id) {
                let at = if end == WallEnd::Start {
                    w.start
                } else {
                    w.end
                };
                exclude.extend(
                    ops::walls_at(&cx.project, cx.floor, at, JOIN_TOL, Some(id))
                        .into_iter()
                        .map(|(i, _)| i),
                );
            }
        }
        cx.begin_change(op.label());
        self.drag = Drag::Active(Box::new(Active {
            op,
            original: cx.project.clone(),
            start,
            exclude,
        }));
    }

    fn finish(&mut self, cx: &mut EditorContext, a: Active) -> ToolResult {
        let fl = cx.floor;
        // Dropping a wall end on the middle of another wall splits it there.
        if let Op::WallEnd(id, end) = a.op {
            if let Some(w) = cx.floor().wall(id) {
                let p = if end == WallEnd::Start {
                    w.start
                } else {
                    w.end
                };
                let mut skip = vec![id];
                skip.extend(
                    ops::walls_at(&cx.project, fl, p, JOIN_TOL, None)
                        .iter()
                        .map(|x| x.0),
                );
                ops::split_walls_at_point(&mut cx.project, fl, p, &skip);
            }
        }
        // Chief auto-connects a wall whose end or body was dragged near other
        // walls (W-31..W-36); run inside the drag's own undo step.
        if let Op::WallEnd(id, _) | Op::WallMove(id) = a.op {
            crate::editor::connect::auto_connect(cx, id);
        }
        cx.last_snap = None;
        cx.mark_dirty();
        if a.original.to_json().ok() == cx.project.to_json().ok() {
            cx.cancel_change();
            return ToolResult::consumed();
        }
        if let Op::Camera(id, _) = a.op {
            Outbox::global().post(ViewRequest::RefreshCamera(id));
        }
        ToolResult::committed(a.op.label())
    }

    fn cancel_drag(&mut self, cx: &mut EditorContext) -> bool {
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Active(a) => {
                cx.project = a.original;
                cx.cancel_change();
                cx.last_snap = None;
                cx.mark_dirty();
                true
            }
            Drag::None => false,
            _ => true,
        }
    }

    fn cycle(&mut self, cx: &mut EditorContext, backwards: bool) -> ToolResult {
        let Some(at) = cx.cursor_world else {
            return ToolResult::ignored();
        };
        let hits = hit_test_cx(cx, at, cx.pick_tol());
        if hits.is_empty() {
            return ToolResult::ignored();
        }
        let n = hits.len();
        let next = match cx
            .selection
            .single()
            .and_then(|s| hits.iter().position(|h| *h == s))
        {
            Some(i) if backwards => (i + n - 1) % n,
            Some(i) => (i + 1) % n,
            None => 0,
        };
        select_hit(cx, hits[next]);
        ToolResult::consumed()
    }

    fn nudge(&mut self, cx: &mut EditorContext, dir: Point, big: bool) -> ToolResult {
        if cx.selection.is_empty() {
            return ToolResult::ignored();
        }
        let delta = dir * (cx.snap_unit() * if big { 10.0 } else { 1.0 });
        let items = cx.selection.items.clone();
        if items.iter().any(|o| !cx.check_unlocked(*o)) {
            return ToolResult::consumed();
        }
        cx.begin_change("Nudge");
        match cx.selection.single() {
            Some(ObjectRef::Wall(id)) => move_wall(cx, id, delta, false),
            Some(ObjectRef::Opening(id)) => slide_opening_by(cx, id, delta),
            _ => move_group(cx, &items, delta),
        }
        cx.mark_dirty();
        ToolResult::committed("Nudge")
    }

    fn key_while_editing(&mut self, cx: &mut EditorContext, k: &KeyEvent) -> ToolResult {
        if let Some(t) = &k.text {
            let ok: String = t
                .chars()
                .filter(|c| c.is_ascii_digit() || " '\"-/.".contains(*c))
                .collect();
            cx.temp.type_text(&ok);
        } else if k.is(Key::Backspace) {
            cx.temp.backspace();
        } else if k.is(Key::Tab) {
            cx.temp.next_field();
        } else if k.is(Key::Escape) {
            cx.temp.cancel();
        } else if k.is(Key::Enter) {
            match tempdim::commit_edit(cx) {
                Ok(label) => return ToolResult::committed(label),
                Err(e) => cx.status = e,
            }
        }
        ToolResult::consumed()
    }
}

impl Tool for SelectTool {
    fn id(&self) -> ToolId {
        ToolId::Select
    }

    fn name(&self) -> &'static str {
        "Select Objects"
    }

    fn hint(&self) -> String {
        "Select: click an object; drag to move or marquee; Tab cycles; Delete removes".into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        self.cursor
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.cancel_drag(cx);
        cx.temp.cancel();
        cx.hover = None;
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        // Space Planning boxes sit on top of the plan and are dragged first.
        if rooms_edit::space_pointer_down(cx, p.world) {
            return ToolResult::consumed();
        }
        self.room_click = None;
        let tol = cx.pick_tol();
        if let Some(i) = cx.temp.hit_label(p.world, cx.px_per_in) {
            cx.temp.cancel();
            cx.temp.begin_edit(i);
            return ToolResult::consumed();
        }
        cx.temp.cancel();
        let shift = p.modifiers.shift;
        if let Some(op) = Self::handle_op(cx, p.world, tol) {
            self.drag = Drag::Armed {
                op,
                start: p.world,
                screen: p.screen,
            };
            return ToolResult::consumed();
        }
        let hits = hit_test_cx(cx, p.world, tol);
        // A room on top means empty floor: the click selects the room.
        let top_hit = hits
            .first()
            .copied()
            .filter(|o| !matches!(o, ObjectRef::Room(_)));
        if let Some(top) = top_hit {
            rooms_edit::clear_room_selection();
            if shift {
                cx.selection.toggle(top);
                return ToolResult::consumed();
            }
            if !cx.selection.contains(top) {
                cx.selection.set(top);
            }
            let op = if cx.selection.len() == 1 {
                single_op_for_body(top)
            } else {
                Op::Group
            };
            self.drag = Drag::Armed {
                op,
                start: p.world,
                screen: p.screen,
            };
            return ToolResult::consumed();
        }
        if !shift {
            cx.selection.clear();
            rooms_edit::clear_room_selection();
        }
        self.room_click = rooms_edit::room_index_at(cx, p.world);
        self.drag = Drag::Marquee {
            start: p.world,
            current: p.world,
            screen: p.screen,
            add: shift,
        };
        ToolResult::consumed()
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if p.down && rooms_edit::space_dragging() {
            rooms_edit::space_pointer_move(p.world);
            return ToolResult::consumed();
        }
        if !p.down {
            self.update_hover(cx, &p);
            return ToolResult {
                repaint: true,
                ..ToolResult::default()
            };
        }
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Armed { op, start, screen } => {
                if (p.screen - screen).length() < DRAG_THRESHOLD_PX || matches!(op, Op::Swing(_)) {
                    self.drag = Drag::Armed { op, start, screen };
                    return ToolResult::consumed();
                }
                // Locked layers refuse the edit (S-5).
                let items: Vec<ObjectRef> = cx.selection.items.clone();
                if items.iter().any(|o| !cx.check_unlocked(*o)) {
                    return ToolResult::consumed();
                }
                self.start_drag(cx, op, start);
                if let Drag::Active(a) = std::mem::replace(&mut self.drag, Drag::None) {
                    self.apply(cx, &a, &p);
                    self.drag = Drag::Active(a);
                }
            }
            Drag::Active(a) => {
                self.apply(cx, &a, &p);
                self.drag = Drag::Active(a);
            }
            Drag::Marquee {
                start, screen, add, ..
            } => {
                self.drag = Drag::Marquee {
                    start,
                    current: p.world,
                    screen,
                    add,
                };
            }
            Drag::None => {}
        }
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if rooms_edit::space_pointer_up() {
            return ToolResult::consumed();
        }
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Active(a) => self.finish(cx, *a),
            Drag::Armed {
                op: Op::Swing(id), ..
            } => {
                if cx.check_unlocked(ObjectRef::Opening(id)) {
                    cx.selection.set(ObjectRef::Opening(id));
                    cx.reverse_swing();
                }
                ToolResult::committed("Reverse Swing")
            }
            Drag::Marquee {
                start, screen, add, ..
            } => {
                if (p.screen - screen).length() >= DRAG_THRESHOLD_PX {
                    let found = objects_in_rect(cx, start, p.world);
                    if !add {
                        cx.selection.clear();
                    }
                    for o in found {
                        cx.selection.add(o);
                    }
                } else if let Some(room) = self.room_click.take() {
                    // A plain click on empty floor selects the room.
                    rooms_edit::select_room(cx, room);
                }
                ToolResult::consumed()
            }
            _ => ToolResult::ignored(),
        }
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.drag = Drag::None;
        let hits = hit_test_cx(cx, p.world, cx.pick_tol());
        let top = hits
            .first()
            .copied()
            .filter(|o| !matches!(o, ObjectRef::Room(_)));
        match top {
            Some(o) => {
                select_hit(cx, o);
                cx.requests.push(EditorRequest::OpenSpec(o));
                ToolResult::consumed()
            }
            None => match rooms_edit::room_index_at(cx, p.world) {
                // Double-click inside a room opens the Room Specification (R-19).
                Some(room) => {
                    rooms_edit::select_room(cx, room);
                    rooms_edit::request_room_dialog(cx, room);
                    ToolResult::consumed()
                }
                None => ToolResult::ignored(),
            },
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if cx.temp.editing.is_some() {
            return self.key_while_editing(cx, &k);
        }
        if k.is(Key::Escape) {
            if self.cancel_drag(cx) {
                return ToolResult::consumed();
            }
            if rooms_edit::selected_room(cx).is_some() {
                rooms_edit::clear_room_selection();
                return ToolResult::consumed();
            }
            if !cx.selection.is_empty() {
                cx.selection.clear();
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(Key::Delete) || k.is(Key::Backspace) {
            if cx.selection.is_empty() {
                return ToolResult::ignored();
            }
            cx.delete_selection();
            return ToolResult::committed("Delete");
        }
        if k.is(Key::Enter) {
            if let (true, Some(room)) = (cx.selection.is_empty(), rooms_edit::selected_room(cx)) {
                rooms_edit::request_room_dialog(cx, room);
                return ToolResult::consumed();
            }
            return match cx.selection.single() {
                Some(o) => {
                    cx.requests.push(EditorRequest::OpenSpec(o));
                    ToolResult::consumed()
                }
                None => ToolResult::ignored(),
            };
        }
        if k.is(Key::Tab) {
            return self.cycle(cx, k.modifiers.shift);
        }
        let big = k.modifiers.shift;
        match k.key {
            Some(Key::ArrowLeft) => self.nudge(cx, Point::new(-1.0, 0.0), big),
            Some(Key::ArrowRight) => self.nudge(cx, Point::new(1.0, 0.0), big),
            Some(Key::ArrowUp) => self.nudge(cx, Point::new(0.0, 1.0), big),
            Some(Key::ArrowDown) => self.nudge(cx, Point::new(0.0, -1.0), big),
            _ => ToolResult::ignored(),
        }
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        if let Drag::Marquee { start, current, .. } = &self.drag {
            let r = Rect::from_two_pos(cam.world_to_screen(*start), cam.world_to_screen(*current));
            let crossing = current.x < start.x;
            let col = if crossing { pal.hover } else { pal.selection };
            painter.add(Shape::rect_filled(r, 0.0, col.gamma_multiply(0.12)));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, col), egui::StrokeKind::Inside);
        }
        if cx.view_flags.contains(&ViewFlag::TemporaryDimensions) {
            tempdim::draw(&cx.temp, painter, cam, pal, &cx.defaults.dim_format());
        }
        let hs = handles::handles_for(cx, cam.px_per_in);
        handles::draw(&hs, painter, cam, pal);
        if let (Drag::Active(_), Some(s)) = (&self.drag, cx.last_snap) {
            crate::editor::render::draw_snap_marker(painter, cam, &s, pal.ghost_stroke);
        }
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        let mut v = cx.common_edit_actions();
        v.extend(cx.extra_edit_actions());
        let floor = cx.floor();
        let has_door = cx.selection.items.iter().any(|o| match o {
            ObjectRef::Opening(id) => floor
                .openings
                .iter()
                .any(|x| x.id == *id && x.kind == OpeningKind::Door),
            _ => false,
        });
        if has_door {
            v.push(EditAction::new(EditActionKind::ReverseSwing));
        }
        if cx
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Wall(_)))
        {
            v.push(EditAction::new(EditActionKind::FixWallConnections));
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn room() -> (EditorContext, [Id; 4]) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 96.0),
            Point::new(0.0, 96.0),
        ];
        let mut ids = [0; 4];
        for i in 0..4 {
            ids[i] = cx
                .project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 100.0, WallKind::Exterior);
        }
        cx.refresh();
        (cx, ids)
    }

    fn ev(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(cx, Point::new(x, y))
    }

    fn drag(t: &mut SelectTool, cx: &mut EditorContext, from: (f64, f64), to: (f64, f64)) {
        let a = ev(cx, from.0, from.1);
        t.pointer_move(cx, a);
        t.pointer_down(cx, a.with_down(true));
        let b = ev(cx, to.0, to.1).with_down(true);
        t.pointer_move(cx, b);
        t.pointer_up(cx, b);
    }

    #[test]
    fn click_selects_and_empty_click_deselects() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        let p = ev(&cx, 60.0, 1.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(ids[0])));
        let q = ev(&cx, 60.0, 48.0);
        t.pointer_down(&mut cx, q.with_down(true));
        t.pointer_up(&mut cx, q);
        assert!(cx.selection.is_empty());
    }

    #[test]
    fn perpendicular_drag_moves_a_wall_and_keeps_neighbours_attached() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        // Drag the top wall down 20", sideways drift must be ignored.
        drag(&mut t, &mut cx, (60.0, 96.0), (75.0, 76.0));
        let f = cx.floor();
        let top = f.wall(ids[2]).unwrap();
        assert_eq!(
            (top.start, top.end),
            (Point::new(120.0, 76.0), Point::new(0.0, 76.0))
        );
        assert_eq!(f.wall(ids[1]).unwrap().end, Point::new(120.0, 76.0));
        assert_eq!(f.wall(ids[3]).unwrap().start, Point::new(0.0, 76.0));
        cx.refresh();
        assert_eq!(cx.rooms.len(), 1);
        // One undo step restores everything.
        assert_eq!(cx.undo().as_deref(), Some("Move Wall"));
        assert_eq!(
            cx.floor().wall(ids[2]).unwrap().start,
            Point::new(120.0, 96.0)
        );
        assert_eq!(
            cx.floor().wall(ids[1]).unwrap().end,
            Point::new(120.0, 96.0)
        );
        assert!(!cx.can_undo());
    }

    #[test]
    fn escape_cancels_a_drag() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        let a = ev(&cx, 60.0, 96.0);
        t.pointer_down(&mut cx, a.with_down(true));
        let b = ev(&cx, 60.0, 60.0).with_down(true);
        t.pointer_move(&mut cx, b);
        assert_ne!(cx.floor().wall(ids[2]).unwrap().start.y, 96.0);
        t.key(&mut cx, KeyEvent::escape());
        assert_eq!(cx.floor().wall(ids[2]).unwrap().start.y, 96.0);
        assert!(!cx.can_undo());
    }

    #[test]
    fn end_handle_stretches_with_the_connected_wall_and_splits_on_drop() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        // Select the bottom wall, then drag its end handle up the right wall.
        let p = ev(&cx, 30.0, 1.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        // Alt suspends the 15 degree angle snap so the end lands exactly.
        let alt = eframe::egui::Modifiers {
            alt: true,
            ..eframe::egui::Modifiers::NONE
        };
        let a = ev(&cx, 120.0, 0.0);
        t.pointer_down(&mut cx, a.with_down(true));
        let b = ev(&cx, 120.0, 40.0).with_down(true).with_modifiers(alt);
        t.pointer_move(&mut cx, b);
        t.pointer_up(&mut cx, b);
        // The bottom wall's end moved; the right wall's start followed.
        assert_eq!(
            cx.floor().wall(ids[0]).unwrap().end,
            Point::new(120.0, 40.0)
        );
        assert_eq!(
            cx.floor().wall(ids[1]).unwrap().start,
            Point::new(120.0, 40.0)
        );

        // Dropping an end in the middle of another wall splits that wall.
        let (mut cx, ids) = room();
        let inner = cx.project.add_wall(
            0,
            Point::new(60.0, 40.0),
            Point::new(60.0, 70.0),
            4.5,
            100.0,
            WallKind::Interior,
        );
        cx.selection.set(ObjectRef::Wall(inner));
        drag(&mut t, &mut cx, (60.0, 70.0), (60.0, 96.0));
        let f = cx.floor();
        assert_eq!(f.walls.len(), 6);
        assert_eq!(f.wall(ids[2]).unwrap().end, Point::new(60.0, 96.0));
    }

    #[test]
    fn dragging_an_opening_slides_it() {
        let (mut cx, ids) = room();
        let o = cx
            .project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        let mut t = SelectTool::default();
        drag(&mut t, &mut cx, (60.0, 0.0), (80.4, 2.0));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Opening(o)));
        let op = cx.floor().openings.iter().find(|x| x.id == o).unwrap();
        assert_eq!(op.center_offset, 80.0);
    }

    #[test]
    fn marquee_window_and_crossing() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        // Left to right around the bottom wall only.
        drag(&mut t, &mut cx, (-20.0, -20.0), (140.0, 15.0));
        assert_eq!(cx.selection.items, vec![ObjectRef::Wall(ids[0])]);
        // Right to left touching bottom, right and left walls.
        drag(&mut t, &mut cx, (140.0, 15.0), (-20.0, -20.0));
        assert_eq!(cx.selection.len(), 3);
    }

    #[test]
    fn delete_removes_the_selection_with_its_openings_and_undoes() {
        let (mut cx, ids) = room();
        cx.project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        cx.selection.set(ObjectRef::Wall(ids[0]));
        let mut t = SelectTool::default();
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete"));
        assert_eq!(cx.floor().walls.len(), 3);
        assert!(cx.floor().openings.is_empty());
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 4);
        assert_eq!(cx.floor().openings.len(), 1);
    }

    #[test]
    fn tab_cycles_through_the_objects_under_the_pointer() {
        let (mut cx, ids) = room();
        let o = cx
            .project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        cx.cursor_world = Some(Point::new(60.0, 0.0));
        let mut t = SelectTool::default();
        t.key(&mut cx, KeyEvent::key(Key::Tab));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Opening(o)));
        t.key(&mut cx, KeyEvent::key(Key::Tab));
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(ids[0])));
    }

    #[test]
    fn typing_a_temporary_dimension_moves_the_wall() {
        let (mut cx, ids) = room();
        // A partition in the middle of the room, 40" from the bottom wall.
        let part = cx.project.add_wall(
            0,
            Point::new(10.0, 40.0),
            Point::new(110.0, 40.0),
            4.0,
            100.0,
            WallKind::Interior,
        );
        cx.selection.set(ObjectRef::Wall(part));
        cx.refresh();
        let mut t = SelectTool::default();
        let gap = cx
            .temp
            .dims
            .iter()
            .position(|d| d.value < 40.0 && d.kind == tempdim::TempDimKind::WallGap)
            .unwrap();
        // Click the value, type 3'-0" and press Enter.
        let at = cx.temp.dims[gap].label_pos(cx.px_per_in);
        let p = ev(&cx, at.x, at.y);
        t.pointer_down(&mut cx, p.with_down(true));
        assert!(cx.temp.editing.is_some());
        cx.temp.editing.as_mut().unwrap().text.clear();
        t.key(&mut cx, KeyEvent::text("3'"));
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Move Wall"));
        let y = cx.floor().wall(part).unwrap().start.y;
        // Gap to the bottom wall's face is now 36": 3 + 2 + 36 = 41.
        assert!((y - 41.0).abs() < 1e-9, "{y}");
        let _ = ids;
    }

    #[test]
    fn copy_and_paste_in_place_duplicates_walls_and_openings() {
        let (mut cx, ids) = room();
        cx.project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        cx.selection.set(ObjectRef::Wall(ids[0]));
        cx.apply_edit_action(EditActionKind::Copy);
        cx.apply_edit_action(EditActionKind::PasteInPlace);
        assert_eq!(cx.floor().walls.len(), 5);
        assert_eq!(cx.floor().openings.len(), 2);
        assert_eq!(cx.selection.len(), 1);
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 4);
    }

    #[test]
    fn reverse_swing_is_offered_for_doors_only() {
        let (mut cx, ids) = room();
        let o = cx
            .project
            .add_opening(0, ids[0], 60.0, OpeningKind::Door)
            .unwrap();
        let t = SelectTool::default();
        cx.selection.set(ObjectRef::Opening(o));
        let kinds: Vec<_> = t.edit_toolbar(&cx).iter().map(|a| a.kind).collect();
        assert!(kinds.contains(&EditActionKind::ReverseSwing));
        cx.apply_edit_action(EditActionKind::ReverseSwing);
        assert!(cx.floor().openings[0].swing_flipped);
        cx.selection.set(ObjectRef::Wall(ids[0]));
        let kinds: Vec<_> = t.edit_toolbar(&cx).iter().map(|a| a.kind).collect();
        assert!(!kinds.contains(&EditActionKind::ReverseSwing));
        assert!(kinds.contains(&EditActionKind::FixWallConnections));
    }

    #[test]
    fn clicking_inside_a_room_selects_the_room_and_double_click_opens_it() {
        let (mut cx, ids) = room();
        let mut t = SelectTool::default();
        let q = ev(&cx, 60.0, 48.0);
        t.pointer_down(&mut cx, q.with_down(true));
        t.pointer_up(&mut cx, q);
        assert!(cx.selection.is_empty());
        assert_eq!(rooms_edit::selected_room(&cx), Some(0));
        // Clicking an object takes the selection back from the room.
        let w = ev(&cx, 60.0, 1.0);
        t.pointer_down(&mut cx, w.with_down(true));
        t.pointer_up(&mut cx, w);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(ids[0])));
        assert_eq!(rooms_edit::selected_room(&cx), None);
        // Double-click inside the room asks for the Room Specification.
        let dc = ev(&cx, 60.0, 48.0);
        let r = t.double_click(&mut cx, dc);
        assert!(r.consumed);
        assert_eq!(rooms_edit::selected_room(&cx), Some(0));
        assert_eq!(rooms_edit::take_room_dialog_request(&cx), Some(0));
        // Esc drops the room selection.
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert_eq!(rooms_edit::selected_room(&cx), None);
        // Outside every room a double-click does nothing.
        let far = ev(&cx, 500.0, 500.0);
        assert!(!t.double_click(&mut cx, far).consumed);
    }

    #[test]
    fn dragging_a_space_planning_box_moves_it() {
        use plan_spaceplan::{generate_boxes, Questionnaire};
        let (mut cx, _) = room();
        let mut t = SelectTool::default();
        let boxes = generate_boxes(&Questionnaire::default());
        let first = boxes[0].clone();
        rooms_edit::set_space_boxes(boxes);
        let c = first.center();
        drag(&mut t, &mut cx, (c.x, c.y), (c.x + 240.0, c.y + 240.0));
        let moved = rooms_edit::space_boxes()
            .into_iter()
            .find(|b| b.id == first.id)
            .unwrap();
        assert_ne!(moved.rect, first.rect);
        assert!(cx.selection.is_empty());
        rooms_edit::clear_space_boxes();
    }
}
