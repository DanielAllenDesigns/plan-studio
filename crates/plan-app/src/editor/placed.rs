//! Placed cabinets and library symbols: storage, drawing, hit-testing,
//! handles and the commands the Edit toolbar needs (CB-1..CB-21 cabinets,
//! CB-55..CB-57 library placement in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! Cabinets live in the opaque `Floor.cabinets` JSON slot as
//! `plan_cabinets::Cabinet`; symbols live in `Floor.symbols`. A symbol's
//! `position` is the **back-center** of its footprint for every placement
//! type (the `PlacedSymbol::footprint` convention); free-standing, ceiling and
//! countertop catalog symbols, whose library origin is their center, are drawn
//! from `position + front * depth / 2`.

use super::handles::{Handle, HandleKind};
use super::{Camera, EditorContext, ObjectRef};
use crate::tools::library::find_item;
use eframe::egui::{self, Color32, CursorIcon, FontId, Pos2, Shape, Vec2};
use plan_cabinets::Stroke as CabStroke;
use plan_cabinets::{plan_symbol, Cabinet, CabinetKind, CutoutKind, FaceItem, FaceLayout};
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::{Floor, Id, PlacedSymbol, Project};
use plan_library::{Placement, Stroke as LibStroke, Symbol2d};
use std::cell::RefCell;
use std::f64::consts::TAU;

/// A cabinet or a symbol of the plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlacedRef {
    Cabinet(Id),
    Symbol(Id),
}

impl PlacedRef {
    pub fn object(self) -> ObjectRef {
        match self {
            PlacedRef::Cabinet(i) => ObjectRef::Cabinet(i),
            PlacedRef::Symbol(i) => ObjectRef::Symbol(i),
        }
    }

    pub fn from_object(o: ObjectRef) -> Option<PlacedRef> {
        match o {
            ObjectRef::Cabinet(i) => Some(PlacedRef::Cabinet(i)),
            ObjectRef::Symbol(i) => Some(PlacedRef::Symbol(i)),
            _ => None,
        }
    }
}

/// A copied cabinet or symbol.
#[derive(Clone, Debug)]
pub enum PlacedItem {
    Cabinet(Box<Cabinet>),
    Symbol(Box<PlacedSymbol>),
}

thread_local! {
    /// The placed-object clipboard (the shared `Clipboard` has no slot for
    /// cabinets or symbols).
    static CLIP: RefCell<Vec<PlacedItem>> = const { RefCell::new(Vec::new()) };
}

// ----- cabinet storage -----

/// The layer a cabinet is drawn on (and hidden/locked with).
pub fn cabinet_layer(kind: CabinetKind) -> &'static str {
    if kind.is_wall_like() || matches!(kind, CabinetKind::Soffit | CabinetKind::Shelf) {
        "Cabinets, Wall"
    } else {
        "Cabinets, Base"
    }
}

/// The floor's cabinets; entries that do not parse are skipped.
pub fn load_cabinets(floor: &Floor) -> Vec<Cabinet> {
    floor
        .cabinets
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect()
}

pub fn cabinet_by_id(floor: &Floor, id: Id) -> Option<Cabinet> {
    load_cabinets(floor).into_iter().find(|c| c.id == id)
}

/// The floor's cabinets split into the ones this build reads and the raw
/// records it cannot (a newer build's kind), which every edit writes back
/// untouched (QA-29).
fn split_cabinets(floor: &Floor) -> (Vec<Cabinet>, Vec<serde_json::Value>) {
    let (good, bad) = plan_core::foreign::read_each::<Cabinet>(&floor.cabinets);
    (good, bad.into_iter().map(|(_, v)| v).collect())
}

/// Stores `list` followed by the `raw` records that could not be read.
fn store_cabinets(floor: &mut Floor, list: &[Cabinet], raw: Vec<serde_json::Value>) -> bool {
    if floor.set_cabinets(list).is_err() {
        return false;
    }
    floor.cabinets.extend(raw);
    true
}

/// Runs `f` on the typed cabinet list and stores the result; records this
/// build cannot read stay as they are. `None` only when the list cannot be
/// written.
fn edit_cabinets<R>(
    project: &mut Project,
    floor: usize,
    f: impl FnOnce(&mut Vec<Cabinet>) -> R,
) -> Option<R> {
    let (mut list, raw) = split_cabinets(&project.floors[floor]);
    let r = f(&mut list);
    store_cabinets(&mut project.floors[floor], &list, raw).then_some(r)
}

/// Adds a cabinet under a fresh id.
pub fn add_cabinet(project: &mut Project, floor: usize, mut cab: Cabinet) -> Option<Id> {
    let id = project.alloc_id();
    cab.id = id;
    edit_cabinets(project, floor, |v| v.push(cab))?;
    // The labels live on their own layer from the first cabinet on.
    project.layers.ensure_cabinet_label_layer();
    Some(id)
}

/// Replaces the cabinet with `cab.id`.
pub fn replace_cabinet(project: &mut Project, floor: usize, cab: &Cabinet) -> bool {
    edit_cabinets(project, floor, |v| {
        v.iter_mut()
            .find(|c| c.id == cab.id)
            .map(|c| *c = cab.clone())
            .is_some()
    })
    .unwrap_or(false)
}

pub fn remove_cabinet(project: &mut Project, floor: usize, id: Id) -> bool {
    edit_cabinets(project, floor, |v| {
        let n = v.len();
        v.retain(|c| c.id != id);
        v.len() != n
    })
    .unwrap_or(false)
}

/// Does the placed object still exist (selection bookkeeping)?
pub fn exists(floor: &Floor, r: PlacedRef) -> bool {
    match r {
        PlacedRef::Cabinet(id) => cabinet_by_id(floor, id).is_some(),
        PlacedRef::Symbol(id) => floor.symbol(id).is_some(),
    }
}

// ----- geometry -----

fn unit(angle: f64) -> Point {
    Point::new(angle.cos(), angle.sin())
}

fn poly_dist(p: Point, poly: &[Point]) -> f64 {
    if point_in_polygon(p, poly) {
        return 0.0;
    }
    let n = poly.len();
    (0..n)
        .map(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]))
        .fold(f64::INFINITY, f64::min)
}

/// Unit vectors along the width and toward the front of a symbol.
pub fn symbol_axes(s: &PlacedSymbol) -> (Point, Point) {
    let u = unit(s.angle.to_radians());
    (u, u.perp())
}

pub fn symbol_center(s: &PlacedSymbol) -> Point {
    let (_, v) = symbol_axes(s);
    s.position + v * (s.depth * 0.5)
}

/// Is `a` the same direction as `b` (radians, wrapped)?
pub fn same_angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(TAU);
    d < 1e-6 || TAU - d < 1e-6
}

/// Countertops of touching cabinets merged into one outline (CB-14): the tops
/// of every cabinet (any angle, corner cabinets included) are unioned, so
/// shared sides disappear and overhangs show on free edges only. Returns the
/// closed boundary rings in plan coordinates (holes wind clockwise).
pub fn merged_countertops(cabs: &[Cabinet]) -> Vec<Vec<Point>> {
    let polys: Vec<Vec<Point>> = cabs.iter().filter_map(Cabinet::top_polygon).collect();
    plan_cabinets::union_polygons(&polys)
}

// ----- symbols -----

fn warp_stroke(s: &LibStroke, sx: f64, sy: f64) -> LibStroke {
    let m = |p: Point| Point::new(p.x * sx, p.y * sy);
    match s {
        LibStroke::Polyline { points, closed } => LibStroke::Polyline {
            points: points.iter().map(|p| m(*p)).collect(),
            closed: *closed,
        },
        LibStroke::Circle { center, radius } => LibStroke::Polyline {
            points: (0..32)
                .map(|i| {
                    let a = TAU * f64::from(i) / 32.0;
                    m(Point::new(
                        center.x + radius * a.cos(),
                        center.y + radius * a.sin(),
                    ))
                })
                .collect(),
            closed: true,
        },
        LibStroke::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let sweep = if end_deg - start_deg >= 360.0 {
                360.0
            } else {
                (end_deg - start_deg).rem_euclid(360.0)
            };
            LibStroke::Polyline {
                points: (0..=24)
                    .map(|i| {
                        let a = (start_deg + sweep * f64::from(i) / 24.0).to_radians();
                        m(Point::new(
                            center.x + radius * a.cos(),
                            center.y + radius * a.sin(),
                        ))
                    })
                    .collect(),
                closed: false,
            }
        }
    }
}

/// The strokes of a placed symbol in plan coordinates, mapped from the
/// library symbol with `Symbol2d::transformed`. Uneven sizes and flipped
/// symbols are pre-scaled stroke by stroke (arcs and circles become
/// polylines then). `None` when the catalog id is unknown.
pub fn placed_symbol_strokes(s: &PlacedSymbol) -> Option<Symbol2d> {
    let item = find_item(&s.catalog_id)?;
    let sx = if item.width > 1e-9 {
        s.width / item.width
    } else {
        1.0
    };
    let sy = if item.depth > 1e-9 {
        s.depth / item.depth
    } else {
        1.0
    };
    let (_, v) = symbol_axes(s);
    let origin = if item.placement == Placement::WallMounted {
        s.position
    } else {
        s.position + v * (s.depth * 0.5)
    };
    let angle = s.angle.to_radians();
    if !s.flip && (sx - sy).abs() < 1e-9 && sx > 0.0 {
        return Some(item.symbol.transformed(origin, angle, sx));
    }
    let fx = if s.flip { -sx } else { sx };
    let warped = Symbol2d::new(
        item.symbol
            .strokes
            .iter()
            .map(|k| warp_stroke(k, fx, sy))
            .collect(),
    );
    Some(warped.transformed(origin, angle, 1.0))
}

/// The catalog placement of a symbol (free-standing when unknown).
pub fn symbol_placement(s: &PlacedSymbol) -> Placement {
    find_item(&s.catalog_id).map_or(Placement::FreeStanding, |i| i.placement)
}

// ----- hit testing -----

/// The topmost cabinet under `p` (within `tol` of its box) that passes
/// `filter`; the newest cabinet wins.
pub fn hit_cabinet(
    cx: &EditorContext,
    p: Point,
    tol: f64,
    filter: impl Fn(&Cabinet) -> bool,
) -> Option<Id> {
    let cabs = load_cabinets(cx.floor());
    let near = |c: &&Cabinet| {
        cx.layers().is_visible(cabinet_layer(c.kind))
            && !c.auto_filler
            && filter(c)
            && poly_dist(p, &c.footprint()) <= tol
    };
    // A joined countertop covers the cabinets under it; they win the pick so
    // that a base cabinet in a run can still be selected.
    cabs.iter()
        .rev()
        .filter(|c| c.joined.is_empty())
        .find(near)
        .or_else(|| {
            cabs.iter()
                .rev()
                .filter(|c| !c.joined.is_empty())
                .find(near)
        })
        .map(|c| c.id)
}

pub fn hit_symbol(cx: &EditorContext, p: Point, tol: f64) -> Option<Id> {
    cx.floor()
        .symbols
        .iter()
        .rev()
        .filter(|s| cx.layers().is_visible(&s.layer))
        .find(|s| match &s.distribution {
            // A distribution is picked on its path or outline, not anywhere
            // in its bounding box.
            Some(d) => d.distance_to(p) <= tol.max(DISTRIBUTION_PICK_TOL),
            None => poly_dist(p, &s.footprint()) <= tol,
        })
        .map(|s| s.id)
}

/// How close to its path a click picks a distribution record, inches.
const DISTRIBUTION_PICK_TOL: f64 = 4.0;

/// The cabinet or symbol under `p`: symbols first (they sit on top), then
/// cabinets, newest first. Objects on hidden layers are skipped.
pub fn hit_placed(cx: &EditorContext, p: Point, tol: f64) -> Option<PlacedRef> {
    hit_symbol(cx, p, tol)
        .map(PlacedRef::Symbol)
        .or_else(|| hit_cabinet(cx, p, tol, |_| true).map(PlacedRef::Cabinet))
}

// ----- handles -----

/// `Reshape(n)` of the handle on the middle of the front edge: drags the
/// depth, the back staying put.
pub const DEPTH_FRONT: usize = 1;
/// The middle of the back edge: drags the depth, the front staying put.
pub const DEPTH_BACK: usize = 2;
/// The corner handles: width and depth change together, the opposite corner
/// staying put.
pub const CORNER_BACK_LEFT: usize = 3;
pub const CORNER_BACK_RIGHT: usize = 4;
pub const CORNER_FRONT_RIGHT: usize = 5;
pub const CORNER_FRONT_LEFT: usize = 6;

/// The resize cursor along (or across) a direction.
fn axis_cursor(dir: Point) -> CursorIcon {
    let (ax, ay) = (dir.x.abs(), dir.y.abs());
    if ay < ax * 0.3827 {
        CursorIcon::ResizeHorizontal
    } else if ax < ay * 0.3827 {
        CursorIcon::ResizeVertical
    } else if dir.x * dir.y > 0.0 {
        CursorIcon::ResizeNeSw
    } else {
        CursorIcon::ResizeNwSe
    }
}

/// Edit handles of a placed object (Move, width resize on both ends,
/// Rotate; symbols also get a depth handle, `Reshape(1)`).
pub fn placed_handles(floor: &Floor, r: PlacedRef, scale: f64) -> Vec<Handle> {
    let target = r.object();
    let h = |kind, pos, cursor| Handle {
        kind,
        pos,
        cursor,
        target,
    };
    let off = 24.0 / scale.max(1e-6);
    match r {
        PlacedRef::Cabinet(id) => {
            let Some(c) = cabinet_by_id(floor, id) else {
                return Vec::new();
            };
            let v = unit(c.angle).perp();
            let (w, d) = (c.width, c.depth);
            let mut hs = vec![h(
                HandleKind::Move,
                c.to_plan(Point::new(w / 2.0, d / 2.0)),
                CursorIcon::Move,
            )];
            // A free-form top has no width to stretch: its outline is the shape.
            if !c.kind.is_custom() {
                hs.push(h(
                    HandleKind::ResizeStart,
                    c.to_plan(Point::new(0.0, d / 2.0)),
                    CursorIcon::ResizeHorizontal,
                ));
                hs.push(h(
                    HandleKind::ResizeEnd,
                    c.to_plan(Point::new(w, d / 2.0)),
                    CursorIcon::ResizeHorizontal,
                ));
                // Depth from the front and the back, and the four corners
                // (width and depth together), CB-8. They are `Reshape(n)`
                // handles, see [`DEPTH_FRONT`] and friends.
                let u = unit(c.angle);
                let side = axis_cursor(v);
                hs.push(h(
                    HandleKind::Reshape(DEPTH_FRONT),
                    c.to_plan(Point::new(w / 2.0, d)),
                    side,
                ));
                hs.push(h(
                    HandleKind::Reshape(DEPTH_BACK),
                    c.to_plan(Point::new(w / 2.0, 0.0)),
                    side,
                ));
                for (n, x, y) in [
                    (CORNER_BACK_LEFT, 0.0, 0.0),
                    (CORNER_BACK_RIGHT, w, 0.0),
                    (CORNER_FRONT_RIGHT, w, d),
                    (CORNER_FRONT_LEFT, 0.0, d),
                ] {
                    let out = u * (if x > 0.0 { 1.0 } else { -1.0 })
                        + v * (if y > 0.0 { 1.0 } else { -1.0 });
                    hs.push(h(
                        HandleKind::Reshape(n),
                        c.to_plan(Point::new(x, y)),
                        axis_cursor(out),
                    ));
                }
            }
            hs.push(h(
                HandleKind::Rotate,
                c.to_plan(Point::new(w / 2.0, d)) + v * off,
                CursorIcon::Grab,
            ));
            // The label's drag handle sits just past the end of the text so
            // it never covers the Move handle at the centre.
            if let Some((at, text, height, angle)) = cabinet_label_spot(&c) {
                let along = unit(angle);
                let half = text.chars().count() as f64 * height * 0.3;
                hs.push(h(
                    HandleKind::Label,
                    at + along * (half + 6.0 / scale.max(1e-6)),
                    CursorIcon::Grab,
                ));
            }
            hs
        }
        PlacedRef::Symbol(id) => {
            let Some(s) = floor.symbol(id) else {
                return Vec::new();
            };
            let (u, v) = symbol_axes(s);
            let front = s.position + v * s.depth;
            vec![
                h(HandleKind::Move, symbol_center(s), CursorIcon::Move),
                h(
                    HandleKind::ResizeStart,
                    symbol_center(s) - u * (s.width * 0.5),
                    CursorIcon::ResizeHorizontal,
                ),
                h(
                    HandleKind::ResizeEnd,
                    symbol_center(s) + u * (s.width * 0.5),
                    CursorIcon::ResizeHorizontal,
                ),
                h(HandleKind::Reshape(1), front, CursorIcon::ResizeVertical),
                h(HandleKind::Rotate, front + v * off, CursorIcon::Grab),
            ]
        }
    }
}

// ----- drawing -----

fn sc(cam: &Camera, p: Point) -> Pos2 {
    cam.world_to_screen(p)
}

fn arc_points(center: Point, radius: f64, start_deg: f64, end_deg: f64) -> Vec<Point> {
    let sweep = if end_deg - start_deg >= 360.0 {
        360.0
    } else {
        (end_deg - start_deg).rem_euclid(360.0)
    };
    (0..=32)
        .map(|i| {
            let a = (start_deg + sweep * f64::from(i) / 32.0).to_radians();
            Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect()
}

/// Library strokes as polylines (`closed` flag); circles and arcs are
/// sampled. Used by previews that draw into their own rectangle.
pub fn stroke_polylines(sym: &Symbol2d) -> Vec<(Vec<Point>, bool)> {
    sym.strokes
        .iter()
        .map(|k| match k {
            LibStroke::Polyline { points, closed } => (points.clone(), *closed),
            LibStroke::Circle { center, radius } => (
                arc_points(*center, *radius, 0.0, 360.0)
                    .into_iter()
                    .take(32)
                    .collect(),
                true,
            ),
            LibStroke::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => (arc_points(*center, *radius, *start_deg, *end_deg), false),
        })
        .collect()
}

/// Draws library strokes already in plan coordinates.
pub fn draw_library_strokes(
    painter: &egui::Painter,
    cam: &Camera,
    sym: &Symbol2d,
    stroke: egui::Stroke,
) {
    for k in &sym.strokes {
        match k {
            LibStroke::Polyline { points, closed } => {
                let pts: Vec<Pos2> = points.iter().map(|p| sc(cam, *p)).collect();
                painter.add(if *closed {
                    Shape::closed_line(pts, stroke)
                } else {
                    Shape::line(pts, stroke)
                });
            }
            LibStroke::Circle { center, radius } => {
                painter.circle_stroke(sc(cam, *center), (*radius * cam.px_per_in) as f32, stroke);
            }
            LibStroke::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let pts = arc_points(*center, *radius, *start_deg, *end_deg)
                    .into_iter()
                    .map(|p| sc(cam, p))
                    .collect();
                painter.add(Shape::line(pts, stroke));
            }
        }
    }
}

fn draw_text(
    painter: &egui::Painter,
    cam: &Camera,
    at: Point,
    text: &str,
    height: f64,
    angle: f64,
    color: Color32,
) {
    let size = ((height * cam.px_per_in) as f32).clamp(7.0, 16.0);
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(size), color);
    // Keep the text readable: never upside down.
    let a = if angle.cos() < -1e-9 {
        angle + std::f64::consts::PI
    } else {
        angle
    };
    let s = -(a as f32);
    let half = galley.size() * 0.5;
    let rot = Vec2::new(
        half.x * s.cos() - half.y * s.sin(),
        half.x * s.sin() + half.y * s.cos(),
    );
    let pos = sc(cam, at) - rot;
    painter.add(egui::epaint::TextShape::new(pos, galley, color).with_angle(s));
}

/// Draws one cabinet's plan symbol. The countertop outline (the second
/// stroke) is left out when `merged_tops` draws it instead.
pub fn draw_cabinet(
    painter: &egui::Painter,
    cam: &Camera,
    cab: &Cabinet,
    color: Color32,
    merged_tops: bool,
) {
    draw_cabinet_parts(painter, cam, cab, color, merged_tops, true);
}

/// [`draw_cabinet`] with the label optional: it belongs to the layer
/// "Cabinets, Labels", which can be hidden apart from the cabinets.
pub fn draw_cabinet_parts(
    painter: &egui::Painter,
    cam: &Camera,
    cab: &Cabinet,
    color: Color32,
    merged_tops: bool,
    labels: bool,
) {
    draw_cabinet_strokes(painter, cam, cab, plan_symbol(cab), color, merged_tops, labels);
}

/// [`draw_cabinet_parts`] for the given strokes (the plan symbol with the
/// run display and the Plan Display Options applied, see
/// `plan_cabinets::plan_strokes`).
pub fn draw_cabinet_strokes(
    painter: &egui::Painter,
    cam: &Camera,
    cab: &Cabinet,
    strokes: Vec<CabStroke>,
    color: Color32,
    merged_tops: bool,
    labels: bool,
) {
    let stroke = egui::Stroke::new(1.2_f32, color);
    // The countertop outline is the stroke after the footprint; a footprint
    // with hidden edges is several leading lines.
    let lead = strokes
        .iter()
        .take_while(|k| matches!(k, CabStroke::Line(..)))
        .count();
    let top_index = if lead > 0 { lead } else { 1 };
    for (i, k) in strokes.iter().enumerate() {
        match k {
            CabStroke::Line(a, b) => {
                painter.line_segment([sc(cam, *a), sc(cam, *b)], stroke);
            }
            CabStroke::Polyline(pts, closed) => {
                if merged_tops && cab.countertop.is_some() && i == top_index {
                    continue;
                }
                let s: Vec<Pos2> = pts.iter().map(|p| sc(cam, *p)).collect();
                painter.add(if *closed {
                    Shape::closed_line(s, stroke)
                } else {
                    Shape::line(s, stroke)
                });
            }
            CabStroke::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let pts = arc_points(*center, *radius, start.to_degrees(), end.to_degrees())
                    .into_iter()
                    .map(|p| sc(cam, p))
                    .collect();
                painter.add(Shape::line(pts, stroke));
            }
            CabStroke::Text {
                at,
                text,
                height,
                angle,
            } => {
                if labels && !text.is_empty() {
                    draw_text(painter, cam, *at, text, *height, *angle, color);
                }
            }
        }
    }
}

/// Fills a cabinet's outline in the plan as its Fill Style tab says: a
/// translucent solid, or hatch lines at 45 degrees (and 135 for a cross
/// hatch) clipped to the outline.
pub fn draw_cabinet_fill(painter: &egui::Painter, cam: &Camera, cab: &Cabinet) {
    let fill = &cab.fill;
    if !fill.is_visible() {
        return;
    }
    let ring = cab.footprint();
    let alpha = (fill.alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    let [r, g, b] = fill.color;
    let color = Color32::from_rgba_unmultiplied(r, g, b, alpha);
    if fill.pattern == plan_cabinets::FillPattern::Solid {
        for t in plan_cabinets::triangulate(&ring, &[]) {
            painter.add(Shape::convex_polygon(
                t.iter().map(|p| sc(cam, *p)).collect(),
                color,
                egui::Stroke::NONE,
            ));
        }
        return;
    }
    let stroke = egui::Stroke::new(1.0_f32, color);
    for [a, b] in fill.hatch_lines(&ring) {
        painter.line_segment([sc(cam, a), sc(cam, b)], stroke);
    }
}

/// Where a cabinet's label is drawn, its text height and angle (plan inches
/// and radians), as the plan symbol places it with the label offset.
pub fn cabinet_label_spot(c: &Cabinet) -> Option<(Point, String, f64, f64)> {
    plan_symbol(c).into_iter().rev().find_map(|k| match k {
        CabStroke::Text {
            at,
            text,
            height,
            angle,
        } => Some((at, text, height, angle)),
        _ => None,
    })
}

/// Dashed closed outline (Chief shows countertops dashed).
fn draw_dashed(painter: &egui::Painter, cam: &Camera, poly: &[Point], stroke: egui::Stroke) {
    let mut pts: Vec<Pos2> = poly.iter().map(|p| sc(cam, *p)).collect();
    if let Some(first) = pts.first().copied() {
        pts.push(first);
    }
    painter.extend(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
}

fn draw_outline(painter: &egui::Painter, cam: &Camera, poly: &[Point], stroke: egui::Stroke) {
    painter.add(Shape::closed_line(
        poly.iter().map(|p| sc(cam, *p)).collect(),
        stroke,
    ));
}

/// Draws every cabinet and placed symbol of the floor, merged countertops,
/// and the selection / hover highlight of these objects. Called by
/// `render::draw_plan`.
pub fn draw_placed(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    let floor = cx.floor();
    let labels_visible = cx
        .layers()
        .is_visible(plan_core::layers::CABINET_LABEL_LAYER);
    let cabs: Vec<Cabinet> = load_cabinets(floor)
        .into_iter()
        .filter(|c| cx.layers().is_visible(cabinet_layer(c.kind)))
        .collect();
    // Merged cabinets (side by side within 3 in, or meeting at a corner) show
    // module lines instead of the end faces they share; the layer "Cabinets,
    // Module Lines" turns the lines off.
    let general = &cx.defaults.cabinets.general;
    let display = plan_cabinets::run_display(
        &cabs,
        plan_cabinets::merge_reach(general.create_automatic_fillers),
        general.show_partial_module_lines,
    );
    let plan_options = plan_cabinets::PlanOptions::from_general(general);
    for c in &cabs {
        let color = if cabinet_layer(c.kind) == "Cabinets, Wall" {
            pal.text.gamma_multiply(0.7)
        } else {
            pal.text
        };
        draw_cabinet_fill(painter, cam, c);
        let hidden = display.hidden_edges.get(&c.id).map_or(&[][..], Vec::as_slice);
        let strokes = plan_cabinets::plan_strokes(c, hidden, &plan_options);
        draw_cabinet_strokes(painter, cam, c, strokes, color, true, labels_visible);
    }
    if cx.layers().is_visible(plan_cabinets::MODULE_LINES_LAYER) {
        let line = egui::Stroke::new(1.0_f32, pal.text.gamma_multiply(0.6));
        for l in &display.lines {
            let pts = [sc(cam, l.a), sc(cam, l.b)];
            if general.show_partial_module_lines {
                painter.line_segment(pts, line);
            } else {
                painter.extend(Shape::dashed_line(&pts, line, 5.0, 3.0));
            }
        }
    }
    let tops: Vec<Cabinet> = cabs
        .iter()
        .filter(|c| c.countertop.is_some())
        .cloned()
        .collect();
    let top_stroke = egui::Stroke::new(1.0_f32, pal.text.gamma_multiply(0.8));
    for poly in merged_countertops(&tops) {
        draw_dashed(painter, cam, &poly, top_stroke);
    }
    for s in &floor.symbols {
        if !cx.layers().is_visible(&s.layer) {
            continue;
        }
        let stroke = egui::Stroke::new(1.2_f32, pal.text);
        if s.image.is_some() {
            crate::tools::images::draw_image(painter, cam, s, pal.text);
        } else if s.distribution.is_some() {
            crate::tools::images::draw_distribution(painter, cam, s, pal.text);
        } else {
            match placed_symbol_strokes(s) {
                Some(sym) => draw_library_strokes(painter, cam, &sym, stroke),
                None => draw_outline(painter, cam, &s.footprint(), stroke),
            }
        }
        // An unknown catalog item is a labelled box (its own label, else
        // the words of its id).
        let unknown =
            s.image.is_none() && s.distribution.is_none() && find_item(&s.catalog_id).is_none();
        let label = if s.label.is_empty() && unknown {
            plan_library::standin::stand_in_label(&s.catalog_id)
        } else {
            s.label.clone()
        };
        if !label.is_empty() {
            draw_text(
                painter,
                cam,
                symbol_center(s),
                &label,
                3.0,
                s.angle.to_radians(),
                pal.text,
            );
        }
    }
    let outline = |r: ObjectRef, stroke: egui::Stroke| {
        let poly = match r {
            ObjectRef::Cabinet(id) => cabs.iter().find(|c| c.id == id).map(Cabinet::footprint),
            ObjectRef::Symbol(id) => floor.symbol(id).map(|s| s.footprint().to_vec()),
            _ => None,
        };
        if let Some(p) = poly {
            draw_outline(painter, cam, &p, stroke);
        }
    };
    if let Some(h) = cx.hover.filter(|h| !cx.selection.contains(*h)) {
        outline(h, egui::Stroke::new(2.0_f32, pal.hover));
    }
    for o in &cx.selection.items {
        outline(*o, egui::Stroke::new(3.0_f32, pal.selection));
    }
}

// ----- commands on the selection -----

fn locked(cx: &EditorContext, r: PlacedRef) -> bool {
    let layer = match r {
        PlacedRef::Cabinet(id) => {
            cabinet_by_id(cx.floor(), id).map(|c| cabinet_layer(c.kind).to_string())
        }
        PlacedRef::Symbol(id) => cx.floor().symbol(id).map(|s| s.layer.clone()),
    };
    layer.is_some_and(|l| cx.layers().is_locked(&l))
}

fn selected_placed(cx: &EditorContext) -> Vec<PlacedRef> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| PlacedRef::from_object(*o))
        .collect()
}

/// Deletes the selected cabinets and symbols (one undo step). Objects on
/// locked layers are refused. Returns how many were removed.
pub fn delete_placed(cx: &mut EditorContext) -> usize {
    let refs: Vec<PlacedRef> = selected_placed(cx)
        .into_iter()
        .filter(|r| !locked(cx, *r))
        .collect();
    if refs.is_empty() {
        return 0;
    }
    cx.begin_change("Delete");
    let fl = cx.floor;
    let mut n = 0;
    // Deleting a generated countertop gives the cabinets under it their own
    // slabs back (and the top is not regenerated by this step).
    let mut deleted_top = false;
    for r in &refs {
        if let PlacedRef::Cabinet(id) = r {
            if let Some(top) = cabinet_by_id(cx.floor(), *id).filter(|c| !c.joined.is_empty()) {
                deleted_top = true;
                let _ = edit_cabinets(&mut cx.project, fl, |v| {
                    plan_cabinets::release_joined_top(&top, v);
                });
            }
        }
    }
    for r in &refs {
        n += usize::from(match r {
            PlacedRef::Cabinet(id) => remove_cabinet(&mut cx.project, fl, *id),
            PlacedRef::Symbol(id) => cx.project.remove_symbol(fl, *id),
        });
    }
    if n == 0 {
        cx.cancel_change();
        return 0;
    }
    cx.selection
        .items
        .retain(|o| PlacedRef::from_object(*o).is_none());
    if !deleted_top {
        rejoin_if_enabled(cx);
    }
    cx.mark_dirty();
    n
}

/// How many placed objects the placed-object clipboard holds.
pub fn clipboard_len() -> usize {
    CLIP.with(|c| c.borrow().len())
}

/// Empties the placed-object clipboard (a copy of other objects replaced it).
pub fn clear_clipboard() {
    CLIP.with(|c| c.borrow_mut().clear());
}

/// Copies the selected cabinets and symbols to the placed-object clipboard.
pub fn copy_placed(cx: &mut EditorContext) -> usize {
    let items: Vec<PlacedItem> = selected_placed(cx)
        .into_iter()
        .filter_map(|r| match r {
            PlacedRef::Cabinet(id) => {
                cabinet_by_id(cx.floor(), id).map(|c| PlacedItem::Cabinet(Box::new(c)))
            }
            // A fireplace carries its specification with the copy.
            PlacedRef::Symbol(id) => cx
                .floor()
                .symbol(id)
                .map(|s| cx.floor().symbol_for_copy(s))
                .map(|s| PlacedItem::Symbol(Box::new(s))),
        })
        .collect();
    let n = items.len();
    if n == 0 {
        cx.status = "Nothing to copy".into();
        return 0;
    }
    CLIP.with(|c| *c.borrow_mut() = items);
    cx.status = format!("Copied {n} object{}", if n == 1 { "" } else { "s" });
    n
}

/// Pastes the placed-object clipboard at its original coordinates; the
/// copies become the selection.
pub fn paste_placed(cx: &mut EditorContext) -> usize {
    let items = CLIP.with(|c| c.borrow().clone());
    if items.is_empty() {
        return 0;
    }
    cx.begin_change("Paste in Place");
    let fl = cx.floor;
    let mut sel = Vec::new();
    for it in items {
        match it {
            PlacedItem::Cabinet(c) => {
                if let Some(id) = add_cabinet(&mut cx.project, fl, *c) {
                    sel.push(ObjectRef::Cabinet(id));
                }
            }
            PlacedItem::Symbol(mut s) => {
                // A pasted copy of a distributed object stands alone; a
                // pasted distribution record makes its own copies.
                s.owner = None;
                let record = s.distribution.is_some();
                let id = cx.project.add_symbol(fl, *s);
                if record {
                    cx.project.rebuild_distribution(fl, id);
                }
                sel.push(ObjectRef::Symbol(id));
            }
        }
    }
    if sel.is_empty() {
        cx.cancel_change();
        return 0;
    }
    let n = sel.len();
    cx.selection.items = sel;
    rejoin_if_enabled(cx);
    cx.mark_dirty();
    n
}

/// Mirrors a door's swing: left-hinged becomes right-hinged and back; an
/// automatic door becomes explicitly left-hinged first.
fn reverse_item(item: &FaceItem) -> FaceItem {
    match item {
        FaceItem::DoorLeft { height } => FaceItem::DoorRight { height: *height },
        FaceItem::DoorRight { height } | FaceItem::DoorAuto { height } => {
            FaceItem::DoorLeft { height: *height }
        }
        FaceItem::HorizontalLayout { height, cells } => FaceItem::HorizontalLayout {
            height: *height,
            cells: cells
                .iter()
                .map(|c| plan_cabinets::FaceCell {
                    item: reverse_item(&c.item),
                    width: c.width,
                })
                .collect(),
        },
        other => other.clone(),
    }
}

/// `layout` with every door's hinge side reversed.
pub fn reverse_door_swing_layout(layout: &FaceLayout) -> FaceLayout {
    FaceLayout {
        items: layout.items.iter().map(reverse_item).collect(),
        frame_width: layout.frame_width,
    }
}

/// Reverse Door Swing on the selected cabinets (one undo step).
pub fn reverse_door_swing(cx: &mut EditorContext) -> usize {
    let ids: Vec<Id> = selected_placed(cx)
        .into_iter()
        .filter_map(|r| match r {
            PlacedRef::Cabinet(id) if !locked(cx, r) => Some(id),
            _ => None,
        })
        .collect();
    if ids.is_empty() {
        return 0;
    }
    cx.begin_change("Reverse Door Swing");
    let fl = cx.floor;
    let mut n = 0;
    for id in ids {
        if let Some(mut c) = cabinet_by_id(cx.floor(), id) {
            c.face = reverse_door_swing_layout(&c.face);
            n += usize::from(replace_cabinet(&mut cx.project, fl, &c));
        }
    }
    if n == 0 {
        cx.cancel_change();
    }
    cx.mark_dirty();
    n
}

/// Stores an edited cabinet (the Cabinet Specification OK) as one undo step.
pub fn apply_cabinet(cx: &mut EditorContext, draft: &Cabinet) -> bool {
    cx.begin_change("Cabinet Specification");
    let fl = cx.floor;
    // The shaping of a generated countertop (edge, corners) is kept on the
    // slabs it replaced, so the next join gives the same look.
    let mut draft = draft.clone();
    if let Some(custom) = draft.custom.clone() {
        for j in &mut draft.joined {
            j.countertop.edge = custom.edge;
            j.countertop.edge_size = custom.edge_size;
            j.countertop.corner = custom.corner;
            j.countertop.corner_size = custom.corner_size;
        }
    }
    let draft = &draft;
    if replace_cabinet(&mut cx.project, fl, draft) {
        rejoin_if_enabled(cx);
        cx.mark_dirty();
        true
    } else {
        cx.cancel_change();
        false
    }
}

/// Stores an edited symbol (the Symbol Specification OK) as one undo step.
pub fn apply_symbol(cx: &mut EditorContext, draft: &PlacedSymbol) -> bool {
    if draft.distribution.is_some() {
        return crate::tools::images::apply_distribution(cx, draft);
    }
    cx.begin_change("Symbol Specification");
    let fl = cx.floor;
    match cx.project.floors[fl]
        .symbols
        .iter_mut()
        .find(|s| s.id == draft.id)
    {
        Some(s) => {
            *s = draft.clone();
            cx.mark_dirty();
            true
        }
        None => {
            cx.cancel_change();
            false
        }
    }
}

// ----- appliances snapping into bays -----

/// How far from a bay's centre a dropped appliance still snaps into it, in.
pub const BAY_SNAP_REACH: f64 = 30.0;

/// The appliance a library item stands for, from its catalog id
/// (`core.appliances.dishwasher_24`): "Dishwasher", "Range", "Oven",
/// "Refrigerator" or "Microwave".
pub fn appliance_of_catalog(catalog_id: &str) -> Option<&'static str> {
    let id = catalog_id.to_ascii_lowercase();
    [
        ("dishwasher", "Dishwasher"),
        ("refrigerator", "Refrigerator"),
        ("fridge", "Refrigerator"),
        ("microwave", "Microwave"),
        ("range", "Range"),
        ("cooktop", "Range"),
        ("oven", "Oven"),
    ]
    .into_iter()
    .find(|(k, _)| id.contains(k))
    .map(|(_, name)| name)
}

/// Does a bay made for `bay` take an appliance called `appliance`? A range
/// bay takes an oven and the other way round.
fn bay_takes(bay: &str, appliance: &str) -> bool {
    let (b, a) = (bay.to_ascii_lowercase(), appliance.to_ascii_lowercase());
    b == a
        || matches!(
            (b.as_str(), a.as_str()),
            ("range", "oven") | ("oven", "range")
        )
}

/// Snaps an appliance symbol dropped within `reach` of a matching bay (a
/// dishwasher near a Dishwasher opening, a refrigerator near a refrigerator
/// cabinet, a range near a Range opening...) into it: turned to the cabinet,
/// centred in the bay, its back on the cabinet's back, no wider or deeper
/// than the bay. Returns whether it snapped.
pub fn snap_symbol_to_bay(floor: &Floor, sym: &mut PlacedSymbol, reach: f64) -> bool {
    let Some(appliance) = appliance_of_catalog(&sym.catalog_id) else {
        return false;
    };
    let at = symbol_center(sym);
    let mut best: Option<(f64, Cabinet, plan_cabinets::ApplianceBay)> = None;
    for c in load_cabinets(floor) {
        for bay in c.appliance_bays() {
            if !bay_takes(&bay.name, appliance) {
                continue;
            }
            let mid = c.to_plan(Point::new((bay.x.0 + bay.x.1) / 2.0, c.depth / 2.0));
            let d = mid.dist(at);
            if d <= reach && best.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
                best = Some((d, c.clone(), bay));
            }
        }
    }
    let Some((_, c, bay)) = best else {
        return false;
    };
    let inside = bay.x.1 - bay.x.0;
    sym.angle = c.angle.to_degrees();
    sym.width = sym.width.min(inside);
    sym.depth = sym.depth.min(c.depth);
    sym.position = c.to_plan(Point::new((bay.x.0 + bay.x.1) / 2.0, 0.0));
    let toe = c.toe_kick.map_or(0.0, |t| t.height);
    let floor_bay = (bay.z.0 - toe).abs() < 1e-6;
    sym.elevation = c.elevation + if floor_bay { 0.0 } else { bay.z.0 };
    let room = bay.z.1 - if floor_bay { 0.0 } else { bay.z.0 };
    if sym.height > room && room > 1.0 {
        sym.height = room;
    }
    true
}

// ----- pictures, distributions and solids in 3D -----

/// Flat-coloured quads for every picture of the project (see
/// `plan_3d::images`); billboards keep their stored angle. The 3D view calls
/// [`image_meshes_facing`] with the camera instead.
pub fn image_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    image_meshes_facing(project, None)
}

/// Like [`image_meshes`], with billboards turned to face `eye` (the camera
/// position in scene axes: x = plan x, y = up, z = -plan y). Call it every
/// frame; billboards are cheap and need no rebuild of the cached scene.
pub fn image_meshes_facing(project: &Project, eye: Option<[f32; 3]>) -> Vec<plan_3d::Mesh> {
    plan_3d::images::image_meshes(project, eye)
}

/// The angle a billboard picture is drawn at for a camera at `eye` (scene
/// axes), degrees.
pub fn billboard_orientation(s: &PlacedSymbol, eye: Option<[f32; 3]>) -> f64 {
    plan_3d::images::billboard_orientation(s, eye)
}

/// 3D Solid Feature symbols as solids: the library object's meshes in the
/// solid (concrete) material, or a box. The 3D view draws these in place of
/// the normal symbol mesh of `PlacedSymbol::solid` symbols.
pub fn solid_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    use crate::tools::library::chief::{self, Chief3d};
    let mut out = Vec::new();
    for floor in &project.floors {
        for s in floor.symbols.iter().filter(|s| s.solid) {
            let mut meshes = if chief::is_chief_id(&s.catalog_id) {
                match chief::placed_meshes(s, floor.elevation) {
                    Chief3d::Meshes(m) => m,
                    Chief3d::Box => {
                        vec![crate::shell::view3d_panel::symbol_box(s, floor.elevation)]
                    }
                    Chief3d::Missing => Vec::new(),
                }
            } else {
                vec![crate::shell::view3d_panel::symbol_box(s, floor.elevation)]
            };
            for m in &mut meshes {
                m.material = plan_3d::Material::Concrete;
            }
            out.extend(meshes);
        }
    }
    out
}

/// Rebuilds the distributions whose record was moved (the Select tool moves
/// the record only); call it after a move or paste.
pub fn sync_distributions(cx: &mut EditorContext) -> usize {
    crate::tools::images::sync_distributions(cx)
}

/// The label drawn for a cabinet: its override with the macros expanded
/// (`<W>`, `<H>`, `<D>`, `<T>`, `<L>`), or the automatic one (`B36`, `W2430`).
pub fn cabinet_label(c: &Cabinet) -> String {
    c.display_label()
}

// ----- countertops, holes and fixtures -----

/// Edit-toolbar command id of Generate Countertop (run it with
/// [`run_command`]).
pub const GENERATE_COUNTERTOP: &str = "cabinet.generate_countertop";

/// Edit-toolbar command id of Convert Polyline to Soffit (CB-17): the
/// selected closed CAD polylines become polygon soffits.
pub const SOFFIT_FROM_POLYLINE: &str = "cabinet.soffit_from_polyline";

/// Runs a cabinet command by id; false when the id is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        GENERATE_COUNTERTOP => {
            generate_countertops(cx);
            true
        }
        SOFFIT_FROM_POLYLINE => {
            soffits_from_polylines(cx);
            true
        }
        crate::tools::cabinet::SET_AS_DEFAULT_COMMAND => {
            crate::tools::cabinet::set_as_default(cx);
            true
        }
        crate::tools::cabinet::BUMP_MODE_COMMAND => {
            let next = crate::tools::cabinet::bump_mode().next();
            crate::tools::cabinet::set_bump_mode(next);
            cx.status = format!("Cabinets: {}", next.toolbar_label());
            true
        }
        // Underlays, Preferences and the material tools (their menu rows
        // carry these ids), then the CAD edit tools (Fillet, Trim, ...),
        // which are Edit toolbar commands too.
        _ => {
            crate::tools::underlay::run_command(cx, id)
                || crate::tools::library::user::run_command(cx, id)
                || crate::tools::materials::run_command(cx, id)
                || crate::dialogs::preferences::run_command(cx, id)
                || crate::dialogs::app_info::run_command(cx, id)
                || crate::dialogs::layer_sets::run_command(cx, id)
                || crate::dialogs::plan_views::run_command(cx, id)
                || crate::dialogs::plan_check::run_command(cx, id)
                || crate::dialogs::defaults::run_command(cx, id)
                || crate::tools::cad::run_edit_command(cx, id)
                || crate::tools::dimension::run_command(cx, id)
        }
    }
}

/// The ring of a CAD polyline that can become a soffit: closed, or open with
/// its end on its start, with at least three distinct corners and some area.
pub fn closed_polyline_ring(item: &plan_core::cad::CadItem) -> Option<Vec<Point>> {
    let plan_core::cad::CadItem::Polyline { points, closed } = item else {
        return None;
    };
    let mut ring = points.clone();
    if !closed && ring.len() > 3 && ring[0].dist(ring[ring.len() - 1]) < 0.01 {
        ring.pop();
    } else if !closed {
        return None;
    }
    ring.dedup_by(|a, b| a.dist(*b) < 0.01);
    if ring.len() > 1 && ring[0].dist(ring[ring.len() - 1]) < 0.01 {
        ring.pop();
    }
    (ring.len() >= 3 && plan_cabinets::ring_area(&ring).abs() > 1e-6).then_some(ring)
}

/// Does the selection hold a closed CAD polyline? (The Edit toolbar offers
/// Convert Polyline to Soffit then.)
pub fn selection_has_closed_polyline(cx: &EditorContext) -> bool {
    cx.selection.items.iter().any(|o| match o {
        ObjectRef::Cad(id) => cx
            .floor()
            .cad
            .iter()
            .any(|c| c.id == *id && closed_polyline_ring(&c.item).is_some()),
        _ => false,
    })
}

/// Convert Polyline to Soffit (CB-17): each selected closed CAD polyline is
/// replaced by a polygon soffit with the outline of the polyline, the height
/// and elevation of the Soffit tool's defaults, on the soffit's layer. One
/// undo step. Returns how many soffits were made.
pub fn soffits_from_polylines(cx: &mut EditorContext) -> usize {
    let rings: Vec<(Id, Vec<Point>)> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) => cx
                .floor()
                .cad
                .iter()
                .find(|c| c.id == *id)
                .and_then(|c| closed_polyline_ring(&c.item))
                .map(|r| (*id, r)),
            _ => None,
        })
        .collect();
    if rings.is_empty() {
        cx.status = "Select a closed polyline first".into();
        return 0;
    }
    if cx.layers().is_locked(cabinet_layer(CabinetKind::Soffit)) {
        cx.status = "The soffit layer is locked".into();
        return 0;
    }
    let base = crate::tools::cabinet::default_cabinet(cx, CabinetKind::Soffit);
    cx.begin_change("Convert Polyline to Soffit");
    let fl = cx.floor;
    let mut made = Vec::new();
    for (cad_id, ring) in rings {
        let Some(cab) = Cabinet::soffit_polygon(&ring, base.height, base.elevation) else {
            continue;
        };
        let Some(id) = add_cabinet(&mut cx.project, fl, cab) else {
            continue;
        };
        cx.project.remove_cad(fl, cad_id);
        made.push(ObjectRef::Cabinet(id));
    }
    if made.is_empty() {
        cx.cancel_change();
        cx.status = "The plan's cabinets could not be read".into();
        return 0;
    }
    cx.project.prune_cad_data(fl);
    let n = made.len();
    cx.selection.items = made;
    cx.mark_dirty();
    cx.status = format!("Made {n} soffit{}", if n == 1 { "" } else { "s" });
    n
}

/// Generate Countertop (CB-14, CB-15): joins the countertops of touching base
/// cabinets (the selected ones, or all when none is selected) into custom
/// countertops. The cabinets give up their own slab, shrinking by its
/// thickness, and their sink and cooktop holes move to the new top. One undo
/// step. Returns how many countertops were made.
pub fn generate_countertops(cx: &mut EditorContext) -> usize {
    let selected: Vec<Id> = selected_placed(cx)
        .into_iter()
        .filter_map(|r| match r {
            PlacedRef::Cabinet(id) => Some(id),
            PlacedRef::Symbol(_) => None,
        })
        .collect();
    let cabs: Vec<Cabinet> = load_cabinets(cx.floor())
        .into_iter()
        .filter(|c| {
            (selected.is_empty() || selected.contains(&c.id))
                && c.countertop.is_some()
                && !cx.layers().is_locked(cabinet_layer(c.kind))
        })
        .collect();
    let tops = plan_cabinets::generate_countertops(&cabs);
    if tops.is_empty() {
        cx.status = "No base cabinet countertops to join".into();
        return 0;
    }
    cx.begin_change("Generate Countertop");
    let fl = cx.floor;
    let mut made = Vec::new();
    for g in tops {
        let Some(id) = add_cabinet(&mut cx.project, fl, g.top) else {
            continue;
        };
        made.push(ObjectRef::Cabinet(id));
        for sid in g.sources {
            if let Some(mut src) = cabinet_by_id(cx.floor(), sid) {
                if src.hand_over_top() {
                    replace_cabinet(&mut cx.project, fl, &src);
                }
            }
        }
    }
    if made.is_empty() {
        cx.cancel_change();
        cx.status = "The plan's cabinets could not be read".into();
        return 0;
    }
    let n = made.len();
    cx.selection.items = made;
    cx.mark_dirty();
    cx.status = format!("Generated {n} countertop{}", if n == 1 { "" } else { "s" });
    n
}

// ----- automatic countertop join (CB-14) -----

thread_local! {
    /// Join the countertops of touching base cabinets automatically (the
    /// Preferences > Architectural switch). Off until the app turns it on
    /// from its settings, so a bare editor context never rewrites cabinets.
    static AUTO_JOIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Turns the automatic countertop join on or off.
pub fn set_auto_join(on: bool) {
    AUTO_JOIN.with(|a| a.set(on));
}

/// Is the automatic countertop join on?
pub fn auto_join_enabled() -> bool {
    AUTO_JOIN.with(std::cell::Cell::get)
}

/// Brings the generated countertops up to date with the cabinets under them:
/// every generated top is taken apart (the cabinets get their own slabs and
/// holes back) and the touching base cabinets are joined again, so a moved,
/// resized, added or deleted cabinet changes the top it belongs to. A top
/// keeps its id while the same cabinets stand under it. Part of the undo step
/// the caller has begun; changes nothing (and writes nothing) when the result
/// equals what is stored. Returns how many countertops stand after the call,
/// 0 when the base layer is locked or the cabinets cannot be read.
pub fn rejoin_countertops(cx: &mut EditorContext) -> usize {
    let fl = cx.floor;
    if cx.layers().is_locked("Cabinets, Base") {
        return 0;
    }
    let (stored, raw) = split_cabinets(&cx.project.floors[fl]);
    let mut cabs = stored.clone();
    // Take every generated top apart.
    let old_tops: Vec<Cabinet> = cabs
        .iter()
        .filter(|c| !c.joined.is_empty())
        .cloned()
        .collect();
    for top in &old_tops {
        plan_cabinets::release_joined_top(top, &mut cabs);
    }
    cabs.retain(|c| c.joined.is_empty());
    // Join what touches now.
    let mut tops = plan_cabinets::join_touching_countertops(&cabs);
    let mut claimed: Vec<Id> = Vec::new();
    for g in &mut tops {
        let mut ids: Vec<Id> = g.sources.clone();
        ids.sort_unstable();
        // A top keeps its id while its cabinets stand: the old top with the
        // same cabinets, else the unclaimed one that shares the most of them
        // (a cabinet moved off or added to the run).
        let shared = |t: &Cabinet| t.joined.iter().filter(|j| ids.contains(&j.id)).count();
        let reuse = old_tops
            .iter()
            .find(|t| {
                let mut have: Vec<Id> = t.joined.iter().map(|j| j.id).collect();
                have.sort_unstable();
                !claimed.contains(&t.id) && have == ids
            })
            .or_else(|| {
                old_tops
                    .iter()
                    .filter(|t| !claimed.contains(&t.id) && shared(t) > 0)
                    .max_by_key(|t| (shared(t), std::cmp::Reverse(t.id)))
            });
        g.top.id = match reuse {
            Some(t) => {
                claimed.push(t.id);
                t.id
            }
            None => cx.project.alloc_id(),
        };
        for sid in &g.sources {
            if let Some(src) = cabs.iter_mut().find(|c| c.id == *sid) {
                src.hand_over_top();
            }
        }
    }
    let count = tops.len();
    cabs.extend(tops.into_iter().map(|g| g.top));
    let sorted = |v: &[Cabinet]| {
        let mut v = v.to_vec();
        v.sort_by_key(|c| c.id);
        v
    };
    if sorted(&cabs) == sorted(&stored) {
        return count;
    }
    if !store_cabinets(&mut cx.project.floors[fl], &cabs, raw) {
        return 0;
    }
    let project = &cx.project;
    cx.selection.items.retain(|o| o.exists_in(project, fl));
    cx.mark_dirty();
    count
}

/// Keeps full-height backsplashes as high as the wall cabinets over them
/// (`plan_cabinets::fit_full_height_backsplashes`). Part of the undo step the
/// caller has begun. Returns how many changed.
pub fn refresh_backsplashes(cx: &mut EditorContext) -> usize {
    let fl = cx.floor;
    let (mut cabs, raw) = split_cabinets(&cx.project.floors[fl]);
    let n = plan_cabinets::fit_full_height_backsplashes(&mut cabs);
    if n > 0 && store_cabinets(&mut cx.project.floors[fl], &cabs, raw) {
        cx.mark_dirty();
        return n;
    }
    0
}

/// [`rejoin_countertops`] when the automatic join is on; full-height
/// backsplashes follow the wall cabinets either way.
pub fn rejoin_if_enabled(cx: &mut EditorContext) -> usize {
    // Automatic fillers first: the generated tops then run over them.
    crate::tools::cabinet::sync_auto_fillers(cx);
    refresh_backsplashes(cx);
    if auto_join_enabled() {
        rejoin_countertops(cx)
    } else {
        0
    }
}

/// A plan point in the cabinet's local frame.
fn to_local(c: &Cabinet, p: Point) -> Point {
    let d = p.sub(c.position);
    let (s, co) = c.angle.sin_cos();
    Point::new(d.x * co + d.y * s, -d.x * s + d.y * co)
}

/// The topmost countertop (a cabinet's own slab or a custom countertop)
/// whose outline contains every point of `ring`.
fn countertop_under(floor: &Floor, ring: &[Point]) -> Option<Cabinet> {
    load_cabinets(floor).into_iter().rev().find(|c| {
        c.top_polygon()
            .is_some_and(|top| ring.iter().all(|p| point_in_polygon(*p, &top)))
    })
}

/// Custom Counter Hole: cuts the polygon `ring` (plan coordinates) out of the
/// countertop it lies in. Returns the countertop's cabinet id, or `None`
/// (changing nothing) when no countertop contains the whole polygon or its
/// layer is locked. The caller owns the undo step.
pub fn add_counter_hole(cx: &mut EditorContext, ring: &[Point]) -> Option<Id> {
    if ring.len() < 3 || plan_cabinets::ring_area(ring) < 1.0 {
        return None;
    }
    let mut target = countertop_under(cx.floor(), ring)?;
    if cx.layers().is_locked(cabinet_layer(target.kind)) {
        return None;
    }
    let local: Vec<Point> = ring.iter().map(|p| to_local(&target, *p)).collect();
    target.cutouts.push(plan_cabinets::Cutout {
        kind: CutoutKind::Custom,
        name: "Opening".to_string(),
        outline: plan_cabinets::ring_ccw(&local),
    });
    let fl = cx.floor;
    replace_cabinet(&mut cx.project, fl, &target).then_some(target.id)
}

/// Sets a sink or cooktop into the countertop of cabinet `id` (hole plus
/// fixture symbol). One undo step; false when the cabinet has no countertop
/// or the fixture does not fit.
pub fn add_fixture(cx: &mut EditorContext, id: Id, kind: CutoutKind) -> bool {
    let Some(mut cab) = cabinet_by_id(cx.floor(), id) else {
        return false;
    };
    if !cab.add_cutout(kind) {
        cx.status = "That countertop has no room for the fixture".into();
        return false;
    }
    cx.begin_change(match kind {
        CutoutKind::Sink => "Place Sink",
        CutoutKind::Cooktop => "Place Cooktop",
        CutoutKind::Custom => "Place Counter Hole",
    });
    let fl = cx.floor;
    if replace_cabinet(&mut cx.project, fl, &cab) {
        cx.mark_dirty();
        true
    } else {
        cx.cancel_change();
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::tools::library::library_catalog;
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn base_at(x: f64, w: f64) -> Cabinet {
        let mut c = Cabinet::base(w);
        c.position = Point::new(x, 0.0);
        c
    }

    #[test]
    fn two_adjacent_24in_bases_merge_into_one_48in_top() {
        let tops = merged_countertops(&[base_at(0.0, 24.0), base_at(24.0, 24.0)]);
        assert_eq!(tops.len(), 1);
        let t = &tops[0];
        assert_eq!(t.len(), 4, "{t:?}");
        let xs = t.iter().map(|p| p.x);
        let (x0, x1) = (
            xs.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
        );
        assert!((x1 - x0 - 48.0).abs() < 1e-6);
        // 24" box plus the 1" front overhang.
        let ys = t.iter().map(|p| p.y);
        let (y0, y1) = (
            ys.clone().fold(f64::MAX, f64::min),
            ys.fold(f64::MIN, f64::max),
        );
        assert!((y1 - y0 - 25.0).abs() < 1e-6);
    }

    #[test]
    fn a_gap_breaks_the_countertop_and_rotated_runs_merge() {
        let apart = merged_countertops(&[base_at(0.0, 24.0), base_at(30.0, 24.0)]);
        assert_eq!(apart.len(), 2);
        // A run along a vertical wall merges too.
        let run = plan_cabinets::run_along_wall(
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0),
            6.0,
            &[24.0, 24.0],
            CabinetKind::Base,
        );
        let tops = merged_countertops(&run);
        assert_eq!(tops.len(), 1);
        assert_eq!(tops[0].len(), 4);
    }

    #[test]
    fn cabinets_round_trip_through_the_floor_slot_with_ids() {
        let mut cx = cx();
        let a = add_cabinet(&mut cx.project, 0, Cabinet::base(24.0)).unwrap();
        let b = add_cabinet(&mut cx.project, 0, Cabinet::wall(30.0)).unwrap();
        assert_ne!(a, b);
        assert_eq!(load_cabinets(cx.floor()).len(), 2);
        let mut c = cabinet_by_id(cx.floor(), b).unwrap();
        c.width = 36.0;
        assert!(replace_cabinet(&mut cx.project, 0, &c));
        assert_eq!(cabinet_by_id(cx.floor(), b).unwrap().width, 36.0);
        assert!(remove_cabinet(&mut cx.project, 0, a));
        assert!(!remove_cabinet(&mut cx.project, 0, a));
        assert_eq!(load_cabinets(cx.floor()).len(), 1);
    }

    #[test]
    fn hit_placed_finds_cabinets_and_symbols() {
        let mut cx = cx();
        let c = add_cabinet(&mut cx.project, 0, base_at(0.0, 24.0)).unwrap();
        let s = cx.project.add_symbol(
            0,
            PlacedSymbol::new(
                "core.plumbing.toilet_elongated",
                Point::new(200.0, 0.0),
                20.0,
                28.0,
                30.0,
            ),
        );
        assert_eq!(
            hit_placed(&cx, Point::new(12.0, 12.0), 2.0),
            Some(PlacedRef::Cabinet(c))
        );
        assert_eq!(
            hit_placed(&cx, Point::new(200.0, 14.0), 2.0),
            Some(PlacedRef::Symbol(s))
        );
        assert_eq!(hit_placed(&cx, Point::new(100.0, 100.0), 2.0), None);
        // Hidden layers cannot be picked.
        cx.project.layers.set_display("Cabinets, Base", false);
        assert_eq!(hit_placed(&cx, Point::new(12.0, 12.0), 2.0), None);
    }

    #[test]
    fn handles_for_cabinet_and_symbol() {
        let mut cx = cx();
        let c = add_cabinet(&mut cx.project, 0, base_at(0.0, 24.0)).unwrap();
        let hs = placed_handles(cx.floor(), PlacedRef::Cabinet(c), 2.0);
        let kinds: Vec<HandleKind> = hs.iter().map(|h| h.kind).collect();
        assert_eq!(
            kinds,
            [
                HandleKind::Move,
                HandleKind::ResizeStart,
                HandleKind::ResizeEnd,
                HandleKind::Reshape(DEPTH_FRONT),
                HandleKind::Reshape(DEPTH_BACK),
                HandleKind::Reshape(CORNER_BACK_LEFT),
                HandleKind::Reshape(CORNER_BACK_RIGHT),
                HandleKind::Reshape(CORNER_FRONT_RIGHT),
                HandleKind::Reshape(CORNER_FRONT_LEFT),
                HandleKind::Rotate,
                HandleKind::Label
            ]
        );
        assert!(hs[2].pos.dist(Point::new(24.0, 12.0)) < 1e-9);
        // Depth handles sit on the middle of the front and back edges, the
        // corner handles on the corners.
        assert!(hs[3].pos.dist(Point::new(12.0, 24.0)) < 1e-9);
        assert!(hs[4].pos.dist(Point::new(12.0, 0.0)) < 1e-9);
        assert!(hs[5].pos.dist(Point::new(0.0, 0.0)) < 1e-9);
        assert!(hs[7].pos.dist(Point::new(24.0, 24.0)) < 1e-9);
        let s = cx
            .project
            .add_symbol(0, PlacedSymbol::new("x", Point::ZERO, 20.0, 30.0, 10.0));
        assert_eq!(
            placed_handles(cx.floor(), PlacedRef::Symbol(s), 2.0).len(),
            5
        );
    }

    #[test]
    fn delete_copy_paste_and_undo() {
        let mut cx = cx();
        let c = add_cabinet(&mut cx.project, 0, base_at(0.0, 24.0)).unwrap();
        cx.selection.set(ObjectRef::Cabinet(c));
        assert_eq!(copy_placed(&mut cx), 1);
        assert_eq!(delete_placed(&mut cx), 1);
        assert!(load_cabinets(cx.floor()).is_empty());
        assert_eq!(paste_placed(&mut cx), 1);
        assert_eq!(load_cabinets(cx.floor()).len(), 1);
        cx.undo();
        assert!(load_cabinets(cx.floor()).is_empty());
        cx.undo();
        assert_eq!(load_cabinets(cx.floor()).len(), 1);
    }

    #[test]
    fn reverse_door_swing_toggles_hinges() {
        let mut cx = cx();
        let mut cab = Cabinet::base(24.0);
        cab.face = FaceLayout {
            items: vec![FaceItem::DoorLeft { height: 0.0 }],
            frame_width: 1.5,
        };
        let id = add_cabinet(&mut cx.project, 0, cab).unwrap();
        cx.selection.set(ObjectRef::Cabinet(id));
        assert_eq!(reverse_door_swing(&mut cx), 1);
        assert_eq!(
            cabinet_by_id(cx.floor(), id).unwrap().face.items[0],
            FaceItem::DoorRight { height: 0.0 }
        );
    }

    #[test]
    fn symbol_strokes_follow_size_and_flip() {
        let item = library_catalog()
            .get("core.plumbing.toilet_elongated")
            .unwrap();
        let mut s = PlacedSymbol::new(
            item.id.clone(),
            Point::new(100.0, 50.0),
            item.width,
            item.depth,
            item.height,
        );
        let a = placed_symbol_strokes(&s).unwrap().bounds().unwrap();
        s.width *= 2.0;
        let b = placed_symbol_strokes(&s).unwrap().bounds().unwrap();
        assert!(
            (b.width() - 2.0 * a.width()).abs() < 1e-6,
            "{} {}",
            a.width(),
            b.width()
        );
        s.flip = true;
        let c = placed_symbol_strokes(&s).unwrap().bounds().unwrap();
        assert!((c.width() - b.width()).abs() < 1e-6);
        assert!(
            placed_symbol_strokes(&PlacedSymbol::new("nope", Point::ZERO, 1.0, 1.0, 1.0)).is_none()
        );
    }

    #[test]
    fn draws_cabinets_and_symbols_without_panicking() {
        let mut cx = cx();
        cx.project.add_wall(
            0,
            Point::new(0.0, -3.0),
            Point::new(200.0, -3.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let a = add_cabinet(&mut cx.project, 0, base_at(0.0, 24.0)).unwrap();
        add_cabinet(&mut cx.project, 0, base_at(24.0, 36.0)).unwrap();
        let mut w = Cabinet::wall(30.0);
        w.position = Point::new(60.0, 0.0);
        add_cabinet(&mut cx.project, 0, w).unwrap();
        let item = library_catalog()
            .get("core.plumbing.toilet_elongated")
            .unwrap()
            .clone();
        let mut s = PlacedSymbol::new(
            item.id.clone(),
            Point::new(150.0, 0.0),
            item.width,
            item.depth,
            item.height,
        );
        s.label = "WC".into();
        let sid = cx.project.add_symbol(0, s);
        cx.selection.set(ObjectRef::Cabinet(a));
        cx.hover = Some(ObjectRef::Symbol(sid));
        let egui_ctx = egui::Context::default();
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(Vec2::new(800.0, 600.0), egui::Sense::hover());
                let mut cam = Camera::default_view();
                cam.rect = painter.clip_rect();
                draw_placed(&cx, &painter, &cam);
            });
        });
    }

    #[test]
    fn align_helpers() {
        assert!(same_angle(0.0, TAU));
        assert!(!same_angle(0.0, 0.1));
    }

    fn base_ids(cx: &mut EditorContext, cabs: Vec<Cabinet>) -> Vec<Id> {
        cabs.into_iter()
            .map(|c| add_cabinet(&mut cx.project, 0, c).unwrap())
            .collect()
    }

    #[test]
    fn tops_of_corner_and_perpendicular_runs_merge_into_one_outline() {
        let mut corner = Cabinet::corner_base(36.0).with_pie_cut(true);
        corner.position = Point::ZERO;
        let mut a = Cabinet::base(30.0);
        a.position = Point::new(36.0, 0.0);
        let mut b = Cabinet::base(30.0);
        b.angle = -std::f64::consts::FRAC_PI_2;
        b.position = Point::new(0.0, 66.0);
        let rings = merged_countertops(&[corner, a, b]);
        assert_eq!(rings.len(), 1);
        // The L: two 25" wide arms, the long way 66" each.
        let area = plan_cabinets::ring_area(&rings[0]);
        assert!(
            (area - (66.0 * 25.0 + 66.0 * 25.0 - 25.0 * 25.0)).abs() < 1e-6,
            "{area}"
        );
    }

    #[test]
    fn generate_countertop_covers_adjacent_bases_in_one_undo_step() {
        let mut cx = cx();
        let mut run = plan_cabinets::run_along_wall(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            &[24.0, 36.0, 24.0],
            CabinetKind::Base,
        );
        assert!(run[1].add_cutout(CutoutKind::Sink));
        let mut apart = Cabinet::base(24.0);
        apart.position = Point::new(200.0, 3.0);
        run.push(apart);
        let mut wall = Cabinet::wall(30.0);
        wall.position = Point::new(0.0, 3.0);
        run.push(wall);
        let ids = base_ids(&mut cx, run);
        cx.selection.items.clear();
        assert_eq!(generate_countertops(&mut cx), 2);
        let cabs = load_cabinets(cx.floor());
        let tops: Vec<&Cabinet> = cabs
            .iter()
            .filter(|c| c.kind == CabinetKind::CustomCountertop)
            .collect();
        assert_eq!(tops.len(), 2);
        let main = tops.iter().find(|t| t.width > 80.0).unwrap();
        // 84" by 24" plus the front overhang, less the sink hole.
        let expected = (84.0 * 25.0 - 30.0 * 18.0) * 1.5;
        assert!((main.countertop_volume() - expected).abs() < 1e-6);
        // Each source cabinet gave up its slab and shrank to sit under it.
        for id in &ids[..3] {
            let c = cabinet_by_id(cx.floor(), *id).unwrap();
            assert!(c.countertop.is_none() && c.cutouts.is_empty());
            assert_eq!(c.height, 34.5);
        }
        // The wall cabinet is untouched, and the top lands on the boxes.
        assert_eq!(cabinet_by_id(cx.floor(), ids[4]).unwrap().height, 30.0);
        assert_eq!(main.elevation + main.height, 36.0);
        assert_eq!(cx.undo().as_deref(), Some("Generate Countertop"));
        let back = load_cabinets(cx.floor());
        assert_eq!(back.len(), 5);
        assert!(back.iter().take(3).all(|c| c.countertop.is_some()));
        // A selection limits the joined cabinets; nothing to join says so.
        cx.redo();
        assert_eq!(generate_countertops(&mut cx), 0);
        assert!(cx.status.contains("No base cabinet"));
    }

    #[test]
    fn counter_holes_cut_the_countertop_under_them() {
        let mut cx = cx();
        let mut c = Cabinet::base(36.0);
        c.position = Point::new(10.0, 10.0);
        c.angle = std::f64::consts::FRAC_PI_2;
        let id = add_cabinet(&mut cx.project, 0, c).unwrap();
        // Rotated 90 degrees: width runs along +y, depth along -x.
        let hole = [
            Point::new(-5.0, 20.0),
            Point::new(5.0, 20.0),
            Point::new(5.0, 30.0),
            Point::new(-5.0, 30.0),
        ];
        assert_eq!(add_counter_hole(&mut cx, &hole), Some(id));
        let got = cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!(got.cutouts.len(), 1);
        assert!((plan_cabinets::ring_area(&got.cutouts[0].outline) - 100.0).abs() < 1e-9);
        assert!((got.countertop_volume() - (36.0 * 25.0 - 100.0) * 1.5).abs() < 1e-6);
        // Off the countertop: nothing happens.
        let off: Vec<Point> = hole.iter().map(|p| *p + Point::new(500.0, 0.0)).collect();
        assert_eq!(add_counter_hole(&mut cx, &off), None);
        // A sink goes in through add_fixture, as one undo step.
        assert!(add_fixture(&mut cx, id, CutoutKind::Sink));
        assert_eq!(cabinet_by_id(cx.floor(), id).unwrap().cutouts.len(), 2);
        assert_eq!(cx.undo().as_deref(), Some("Place Sink"));
        assert_eq!(cabinet_by_id(cx.floor(), id).unwrap().cutouts.len(), 1);
    }

    #[test]
    fn picking_follows_the_true_footprint_and_custom_tops_have_no_resize() {
        let mut cx = cx();
        let corner = add_cabinet(
            &mut cx.project,
            0,
            Cabinet::corner_base(36.0).with_pie_cut(false),
        )
        .unwrap();
        // Inside the notch of the L (past both 24" arms) there is nothing.
        assert_eq!(hit_placed(&cx, Point::new(30.0, 30.0), 1.0), None);
        assert_eq!(
            hit_placed(&cx, Point::new(30.0, 10.0), 1.0),
            Some(PlacedRef::Cabinet(corner))
        );
        let ring = [
            Point::new(100.0, 0.0),
            Point::new(160.0, 0.0),
            Point::new(160.0, 30.0),
            Point::new(100.0, 30.0),
        ];
        let top = add_cabinet(
            &mut cx.project,
            0,
            Cabinet::custom_countertop(&ring, 1.5, 36.0).unwrap(),
        )
        .unwrap();
        let kinds: Vec<HandleKind> = placed_handles(cx.floor(), PlacedRef::Cabinet(top), 2.0)
            .iter()
            .map(|h| h.kind)
            .collect();
        assert_eq!(
            kinds,
            [HandleKind::Move, HandleKind::Rotate, HandleKind::Label]
        );
        assert_eq!(cabinet_layer(CabinetKind::CornerWall), "Cabinets, Wall");
        assert_eq!(cabinet_layer(CabinetKind::BaseFiller), "Cabinets, Base");
        assert_eq!(
            cabinet_layer(CabinetKind::CustomCountertop),
            "Cabinets, Base"
        );
        assert_eq!(cabinet_layer(CabinetKind::Soffit), "Cabinets, Wall");
    }

    #[test]
    fn cabinet_labels_expand_macros() {
        let mut c = Cabinet::wall(24.0);
        assert_eq!(cabinet_label(&c), "W2430");
        c.label = "<T><W>/<H>".into();
        assert_eq!(cabinet_label(&c), "W24/30");
        assert_eq!(cabinet_label(&Cabinet::base(36.0)), "B36");
        assert!(!run_command(&mut cx(), "nope"));
    }

    // ----- automatic countertop join -----

    fn bases(cx: &mut EditorContext, xs: &[f64]) -> Vec<Id> {
        xs.iter()
            .map(|x| add_cabinet(&mut cx.project, 0, base_at(*x, 24.0)).unwrap())
            .collect()
    }

    fn joined_tops(cx: &EditorContext) -> Vec<Cabinet> {
        load_cabinets(cx.floor())
            .into_iter()
            .filter(|c| !c.joined.is_empty())
            .collect()
    }

    #[test]
    fn rejoining_joins_touching_bases_and_takes_the_top_apart_when_they_part() {
        let mut cx = cx();
        set_auto_join(true);
        let ids = bases(&mut cx, &[0.0, 24.0, 100.0]);
        assert_eq!(rejoin_countertops(&mut cx), 1);
        let tops = joined_tops(&cx);
        assert_eq!(tops.len(), 1);
        let mut sources: Vec<Id> = tops[0].joined.iter().map(|j| j.id).collect();
        sources.sort_unstable();
        assert_eq!(sources, &ids[..2]);
        // The two cabinets under it gave up their slabs; the lone one kept its.
        let list = load_cabinets(cx.floor());
        let by = |id: Id| list.iter().find(|c| c.id == id).unwrap();
        assert!(by(ids[0]).countertop.is_none() && by(ids[1]).countertop.is_none());
        assert!(by(ids[2]).countertop.is_some());
        assert!((by(ids[0]).height - 34.5).abs() < 1e-9);
        // Nothing changed: nothing is rewritten, and the top keeps its id.
        let before = cx.project.floors[0].cabinets.clone();
        assert_eq!(rejoin_countertops(&mut cx), 1);
        assert_eq!(cx.project.floors[0].cabinets, before);
        // Move the second cabinet away: the top comes apart and both get
        // their slabs (and heights) back.
        let mut c = cabinet_by_id(cx.floor(), ids[1]).unwrap();
        c.position = Point::new(60.0, 0.0);
        assert!(replace_cabinet(&mut cx.project, 0, &c));
        assert_eq!(rejoin_countertops(&mut cx), 0);
        let list = load_cabinets(cx.floor());
        assert_eq!(list.len(), 3);
        assert!(list
            .iter()
            .all(|c| c.countertop.is_some() && c.joined.is_empty()));
        assert!(list.iter().all(|c| (c.height - 36.0).abs() < 1e-9));
        set_auto_join(false);
    }

    #[test]
    fn a_joined_top_keeps_its_id_while_the_same_cabinets_stand_under_it() {
        let mut cx = cx();
        let ids = bases(&mut cx, &[0.0, 24.0]);
        rejoin_countertops(&mut cx);
        let top = joined_tops(&cx)[0].id;
        // Widen the first cabinet by 6": the same two cabinets, a new outline.
        let mut c = cabinet_by_id(cx.floor(), ids[0]).unwrap();
        c.width = 30.0;
        c.position = Point::new(-6.0, 0.0);
        replace_cabinet(&mut cx.project, 0, &c);
        rejoin_countertops(&mut cx);
        let tops = joined_tops(&cx);
        assert_eq!(tops.len(), 1);
        assert_eq!(tops[0].id, top);
        let area = plan_cabinets::ring_area(&tops[0].top_polygon().unwrap());
        assert!(area > 54.0 * 24.0, "{area}");
    }

    #[test]
    fn sinks_ride_the_joined_top_and_come_back_to_their_cabinet() {
        let mut cx = cx();
        let ids = bases(&mut cx, &[0.0, 24.0]);
        assert!(add_fixture(&mut cx, ids[0], CutoutKind::Sink));
        let sink = cabinet_by_id(cx.floor(), ids[0]).unwrap().cutouts.clone();
        assert_eq!(sink.len(), 1);
        rejoin_countertops(&mut cx);
        assert_eq!(joined_tops(&cx)[0].cutouts.len(), 1);
        assert!(cabinet_by_id(cx.floor(), ids[0])
            .unwrap()
            .cutouts
            .is_empty());
        let mut c = cabinet_by_id(cx.floor(), ids[1]).unwrap();
        c.position = Point::new(80.0, 0.0);
        replace_cabinet(&mut cx.project, 0, &c);
        rejoin_countertops(&mut cx);
        let back = cabinet_by_id(cx.floor(), ids[0]).unwrap();
        assert_eq!(back.cutouts.len(), 1);
        assert_eq!(back.cutouts[0].kind, CutoutKind::Sink);
        for (a, b) in back.cutouts[0].outline.iter().zip(&sink[0].outline) {
            assert!(a.dist(*b) < 1e-6, "{a:?} {b:?}");
        }
    }

    #[test]
    fn deleting_the_top_restores_the_slabs_and_it_stays_deleted() {
        let mut cx = cx();
        set_auto_join(true);
        let ids = bases(&mut cx, &[0.0, 24.0]);
        rejoin_countertops(&mut cx);
        let top = joined_tops(&cx)[0].id;
        cx.selection.set(ObjectRef::Cabinet(top));
        assert_eq!(delete_placed(&mut cx), 1);
        let list = load_cabinets(cx.floor());
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|c| c.countertop.is_some() && c.id != top));
        assert!(ids.iter().all(|i| list.iter().any(|c| c.id == *i)));
        set_auto_join(false);
    }

    #[test]
    fn auto_join_off_leaves_the_cabinets_alone_and_sources_win_the_pick() {
        let mut cx = cx();
        set_auto_join(false);
        let ids = bases(&mut cx, &[0.0, 24.0]);
        // The first pass stores the program-made ends of the cabinets; after
        // that nothing more is written.
        rejoin_if_enabled(&mut cx);
        let before = cx.project.floors[0].cabinets.clone();
        assert_eq!(rejoin_if_enabled(&mut cx), 0);
        assert_eq!(cx.project.floors[0].cabinets, before);
        // Joined by hand, a click on a base cabinet still picks the cabinet,
        // not the top that covers it; the overhang past the cabinets picks
        // the top.
        rejoin_countertops(&mut cx);
        let top = joined_tops(&cx)[0].id;
        assert_eq!(
            hit_cabinet(&cx, Point::new(12.0, 12.0), 0.5, |_| true),
            Some(ids[0])
        );
        assert_eq!(
            hit_cabinet(&cx, Point::new(36.0, 24.5), 0.2, |_| true),
            Some(top)
        );
    }

    // ----- appliances, backsplashes and labels -----

    fn dishwasher_symbol(at: Point, angle_deg: f64) -> PlacedSymbol {
        let mut s = PlacedSymbol::new("core.appliances.dishwasher_24", at, 24.0, 24.0, 34.0);
        s.angle = angle_deg;
        s
    }

    #[test]
    fn a_dishwasher_dropped_near_its_bay_snaps_into_it() {
        let mut cx = cx();
        let mut bay = Cabinet::dishwasher_opening();
        bay.position = Point::new(48.0, 3.0);
        add_cabinet(&mut cx.project, 0, bay).unwrap();
        // 10" off to the side and a little in front.
        let mut s = dishwasher_symbol(Point::new(70.0, 20.0), 12.0);
        assert!(snap_symbol_to_bay(cx.floor(), &mut s, BAY_SNAP_REACH));
        assert!(
            s.position.dist(Point::new(60.0, 3.0)) < 1e-9,
            "{:?}",
            s.position
        );
        assert_eq!((s.angle, s.elevation), (0.0, 0.0));
        assert!(s.width <= 22.5, "{}", s.width);
        // Out of reach it stays where it was dropped.
        let mut far = dishwasher_symbol(Point::new(300.0, 20.0), 12.0);
        assert!(!snap_symbol_to_bay(cx.floor(), &mut far, BAY_SNAP_REACH));
        assert_eq!(far.angle, 12.0);
        // A refrigerator does not take a dishwasher bay, nor a stool anything.
        let mut fridge = PlacedSymbol::new(
            "core.appliances.refrigerator_36x30",
            Point::new(60.0, 10.0),
            36.0,
            30.0,
            70.0,
        );
        assert!(!snap_symbol_to_bay(cx.floor(), &mut fridge, BAY_SNAP_REACH));
        let mut stool = PlacedSymbol::new(
            "core.furniture.stool",
            Point::new(60.0, 10.0),
            12.0,
            12.0,
            24.0,
        );
        assert!(!snap_symbol_to_bay(cx.floor(), &mut stool, BAY_SNAP_REACH));
        assert_eq!(
            appliance_of_catalog("core.appliances.range_30"),
            Some("Range")
        );
    }

    #[test]
    fn a_refrigerator_snaps_into_the_refrigerator_cabinet_bay_and_a_range_into_a_range_bay() {
        let mut cx = cx();
        let mut f = Cabinet::refrigerator(36.0);
        f.position = Point::new(0.0, 3.0);
        add_cabinet(&mut cx.project, 0, f).unwrap();
        let mut r = Cabinet::range_opening(30.0);
        r.position = Point::new(100.0, 3.0);
        add_cabinet(&mut cx.project, 0, r).unwrap();
        let mut fridge = PlacedSymbol::new(
            "core.appliances.refrigerator_36x30",
            Point::new(30.0, 40.0),
            36.0,
            30.0,
            70.0,
        );
        fridge.angle = 90.0;
        assert!(snap_symbol_to_bay(cx.floor(), &mut fridge, BAY_SNAP_REACH));
        assert!(fridge.position.dist(Point::new(18.0, 3.0)) < 1e-9);
        assert_eq!(fridge.angle, 0.0);
        assert!(fridge.depth <= 25.0 && fridge.width <= 34.5);
        let mut range = PlacedSymbol::new(
            "core.appliances.range_30",
            Point::new(120.0, 25.0),
            30.0,
            26.0,
            36.0,
        );
        assert!(snap_symbol_to_bay(cx.floor(), &mut range, BAY_SNAP_REACH));
        assert!(
            range.position.dist(Point::new(115.0, 3.0)) < 1e-9,
            "{:?}",
            range.position
        );
    }

    #[test]
    fn full_height_backsplashes_follow_the_wall_cabinet_over_them() {
        let mut cx = cx();
        set_auto_join(false);
        let mut base = Cabinet::base(48.0);
        base.position = Point::new(0.0, 3.0);
        let mut bs = plan_cabinets::Backsplash::new(4.0, 0.5);
        bs.full_height = true;
        base.backsplash = Some(bs);
        let id = add_cabinet(&mut cx.project, 0, base).unwrap();
        let mut wall = Cabinet::wall(36.0);
        wall.position = Point::new(6.0, 3.0);
        wall.elevation = 57.0;
        add_cabinet(&mut cx.project, 0, wall).unwrap();
        cx.begin_change("t");
        assert_eq!(refresh_backsplashes(&mut cx), 1);
        let b = cabinet_by_id(cx.floor(), id).unwrap().backsplash.unwrap();
        assert!((b.height - 21.0).abs() < 1e-9, "{}", b.height);
        assert_eq!(refresh_backsplashes(&mut cx), 0);
        // Edits keep it fitted.
        let mut w = load_cabinets(cx.floor())
            .into_iter()
            .find(|c| c.kind == CabinetKind::Wall)
            .unwrap();
        w.elevation = 60.0;
        replace_cabinet(&mut cx.project, 0, &w);
        rejoin_if_enabled(&mut cx);
        let b = cabinet_by_id(cx.floor(), id).unwrap().backsplash.unwrap();
        assert!((b.height - 24.0).abs() < 1e-9);
    }

    #[test]
    fn a_joined_run_keeps_its_backsplash_standing_on_the_generated_top() {
        let mut cx = cx();
        set_auto_join(true);
        let ids = bases(&mut cx, &[0.0, 24.0]);
        for id in &ids {
            let mut c = cabinet_by_id(cx.floor(), *id).unwrap();
            c.backsplash = Some(plan_cabinets::Backsplash::new(4.0, 0.5));
            replace_cabinet(&mut cx.project, 0, &c);
        }
        cx.begin_change("t");
        assert_eq!(rejoin_countertops(&mut cx), 1);
        for id in &ids {
            let c = cabinet_by_id(cx.floor(), *id).unwrap();
            assert!(c.countertop.is_none());
            let b = c.backsplash.expect("the strip stays");
            assert!((b.lift - 1.5).abs() < 1e-9, "stands on the 1 1/2\" top");
            // The mesh reaches the old top height plus the strip: 36 + 4.
            let top = plan_cabinets::meshes(&c)
                .iter()
                .filter_map(|m| m.bounds())
                .map(|(_, hi)| hi[1])
                .fold(0.0f32, f32::max);
            assert!((top - 40.0).abs() < 1e-3, "{top}");
        }
        // Taking the top apart gives each cabinet its slab and strip back.
        set_auto_join(false);
        cx.begin_change("t2");
        for id in &ids {
            let mut c = cabinet_by_id(cx.floor(), *id).unwrap();
            c.position.y += 60.0;
            replace_cabinet(&mut cx.project, 0, &c);
        }
        set_auto_join(true);
        rejoin_countertops(&mut cx);
        for id in &ids {
            let c = cabinet_by_id(cx.floor(), *id).unwrap();
            if c.countertop.is_some() {
                assert_eq!(c.backsplash.unwrap().lift, 0.0);
            }
        }
        set_auto_join(false);
    }

    #[test]
    fn labels_belong_to_the_label_layer_and_the_ghost_draws_without_them() {
        let mut cx = cx();
        add_cabinet(&mut cx.project, 0, base_at(0.0, 24.0)).unwrap();
        assert!(cx.project.layers.get("Cabinets, Labels").is_some());
        let c = load_cabinets(cx.floor())[0].clone();
        let (at, text, _, _) = cabinet_label_spot(&c).unwrap();
        assert_eq!(text, "B24");
        assert!(at.dist(c.to_plan(Point::new(12.0, 12.0))) < 1e-9);
        let mut moved = c.clone();
        moved.label_offset = Point::new(5.0, 2.0);
        let (at2, _, _, _) = cabinet_label_spot(&moved).unwrap();
        assert!((at2.x - at.x - 5.0).abs() < 1e-9 && (at2.y - at.y - 2.0).abs() < 1e-9);
        // Hidden label layer: the draw skips the text but not the cabinet.
        cx.project.layers.set_display("Cabinets, Labels", false);
        assert!(!cx.layers().is_visible("Cabinets, Labels"));
        assert!(cx.layers().is_visible("Cabinets, Base"));
        let egui_ctx = egui::Context::default();
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(Vec2::new(400.0, 300.0), egui::Sense::hover());
                let mut cam = Camera::default_view();
                cam.rect = painter.clip_rect();
                for labels in [true, false] {
                    draw_cabinet_parts(&painter, &cam, &c, Color32::BLACK, false, labels);
                }
                draw_placed(&cx, &painter, &cam);
            });
        });
        // Pre-existing plans gain the layer once, after the wall cabinets'.
        let mut set = plan_core::LayerSet::default_floor_plan();
        assert!(set.ensure_cabinet_label_layer());
        assert!(!set.ensure_cabinet_label_layer());
        let names: Vec<_> = set.layers.iter().map(|l| l.name.as_str()).collect();
        let i = names.iter().position(|n| *n == "Cabinets, Wall").unwrap();
        assert_eq!(names[i + 1], "Cabinets, Labels");
    }
}
