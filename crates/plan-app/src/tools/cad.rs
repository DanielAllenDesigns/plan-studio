//! CAD drawing tools (CAD-1..CAD-36 in `docs/parity/dimensions-text-cad.md`).
//!
//! One tool object, many modes (Chief's Lines, Arcs, Circles, Boxes, Splines,
//! Points and CAD Block flyouts). The mode is picked with [`CadTool::set_mode`]
//! or the option strip drawn at the top of the canvas, because `ToolId` has a
//! single `Cad` value and cannot carry the flyout choice.
//!
//! * lines: click-click or press-drag-release; the chain continues when
//!   "Connect CAD Segments" is on (CAD-3); Enter after the first click (or
//!   the Input Line mode) types a length and angle (CAD-5);
//! * polylines and splines: click vertices, click the first vertex to close,
//!   Enter or double-click to end (CAD-15, CAD-29); Backspace drops the last
//!   vertex;
//! * arcs: three-point, center-start-end and start-end-tangent creation
//!   modes (CAD-7); Input Arc types radius, start angle and sweep (CAD-8);
//! * circles, ellipses (stored as 48-segment closed polylines, the model has
//!   no ellipse), ovals, rectangular polylines, regular polygons (CAD-11,
//!   CAD-12), revision clouds (CAD-36), points (CAD-2);
//! * Make / Explode CAD Block groups and ungroups the selected CAD objects
//!   (CAD-31, CAD-32);
//! * a selected CAD object shows edit handles: line ends, polyline vertices
//!   and midpoint "add vertex" handles, circle center and radius (CAD-9,
//!   CAD-16).
//!
//! New objects go on the current CAD layer, `CAD, Default` (CAD-1).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::ops;
use crate::editor::selection::{cad_by_id, hit_test};
use crate::editor::tempdim::EditField;
use crate::editor::{render, Camera, EditorContext, EditorRequest, ObjectRef};
use crate::theme::Palette;
use crate::toolbar::ViewFlag;
use edit::{EditState, SplineKind};
use eframe::egui::{self, Align2, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use plan_core::cad::CadItem;
use plan_core::geometry::{polygon_area, Point};
use plan_core::units::parse_ft_in;
use plan_core::Id;
use std::cell::RefCell;
use std::f64::consts::{PI, TAU};

mod edit;
#[cfg(test)]
mod edit_tests;
mod style;
#[allow(unused_imports)]
pub use edit::{
    apply_hatch, closed_outline, edit_actions, hatch_lines, hatch_pattern, item_segments,
    offset_item, plan_hatch, run_edit_command, trim_polyline, HatchJob, EDIT_COMMANDS, HATCHES,
};
use edit::{ensure_layer, TEMP_POINT_LAYER};
#[allow(unused_imports)]
pub use style::draw_cad_styled;

/// The layer new CAD objects go on (CAD-1).
pub const CAD_LAYER: &str = plan_core::cad::DEFAULT_CAD_LAYER;
/// Pixels between press and release that make a drag-draw.
const DRAG_PX: f32 = 4.0;
/// Shapes smaller than this are not created.
const MIN_SIZE: f64 = 0.5;
/// Segments of a sampled ellipse.
pub const ELLIPSE_SEGMENTS: usize = 48;
/// Segments per span of a sampled spline (CAD-29).
pub const SPLINE_SEGMENTS_PER_SPAN: usize = 8;
/// Revision cloud scallop size in plan inches (CAD-36).
pub const CLOUD_ARC: f64 = 12.0;
const CLOUD_SAMPLES: usize = 6;
/// Half length of a point's cross.
const POINT_SIZE: f64 = 2.0;

// ----- shared helpers (also used by the dimension and text tools) -----

/// `EditField::index` of the placeholder that makes the shell forward typed
/// characters to the active tool (it only does so while
/// `cx.temp.editing` is set).
pub const TYPING_INDEX: usize = usize::MAX;

/// Turns the "typing" mode on or off. While on, the shell sends typed text to
/// [`Tool::key`] and suspends the single-key hotkeys.
pub fn set_typing(cx: &mut EditorContext, on: bool) {
    if on {
        if cx.temp.editing.is_none() {
            cx.temp.editing = Some(EditField {
                index: TYPING_INDEX,
                text: String::new(),
            });
        }
    } else if cx
        .temp
        .editing
        .as_ref()
        .is_some_and(|e| e.index == TYPING_INDEX)
    {
        cx.temp.editing = None;
    }
}

/// One button of an [`OptionStrip`].
#[derive(Clone, Debug)]
pub struct StripButton {
    pub label: String,
    pub id: u16,
    pub on: bool,
}

impl StripButton {
    pub fn new(label: impl Into<String>, id: u16, on: bool) -> Self {
        Self {
            label: label.into(),
            id,
            on,
        }
    }
}

/// A row of small buttons drawn over the top of the canvas; it carries the
/// variant choices (flyout entries) of a tool whose `ToolId` has no variants.
/// `draw` records where the buttons are so `hit` can answer clicks.
#[derive(Default)]
pub struct OptionStrip {
    rects: RefCell<Vec<(Rect, u16)>>,
}

impl OptionStrip {
    pub fn draw(&self, painter: &Painter, cam: &Camera, pal: &Palette, items: &[StripButton]) {
        let mut rects = self.rects.borrow_mut();
        rects.clear();
        let left = cam.rect.min.x + 8.0;
        let right = cam.rect.max.x - 8.0;
        let (mut x, mut y) = (left, cam.rect.min.y + 8.0);
        for it in items {
            let galley =
                painter.layout_no_wrap(it.label.clone(), FontId::proportional(11.0), pal.text);
            let size = galley.size() + Vec2::new(10.0, 6.0);
            if x + size.x > right && x > left {
                x = left;
                y += size.y + 3.0;
            }
            let r = Rect::from_min_size(Pos2::new(x, y), size);
            let fill = if it.on {
                pal.selection.gamma_multiply(0.45)
            } else {
                pal.background.gamma_multiply(0.92)
            };
            painter.rect_filled(r, 3.0, fill);
            painter.rect_stroke(
                r,
                3.0,
                Stroke::new(1.0_f32, pal.ghost_stroke),
                egui::StrokeKind::Inside,
            );
            painter.galley(r.min + Vec2::new(5.0, 3.0), galley, pal.text);
            rects.push((r, it.id));
            x += size.x + 3.0;
        }
    }

    /// The button under screen position `p`.
    pub fn hit(&self, p: Pos2) -> Option<u16> {
        self.rects
            .borrow()
            .iter()
            .find(|(r, _)| r.contains(p))
            .map(|(_, id)| *id)
    }
}

/// A closed triangle for an arrowhead whose tip is `tip` and which points
/// away from `toward` (the line it ends).
pub fn arrowhead(tip: Point, toward: Point, size: f64) -> Vec<Point> {
    let dir = toward.sub(tip).normalized();
    let base = tip.add(dir.scale(size));
    let half = dir.perp().scale(size * 0.3);
    vec![tip, base.add(half), base.sub(half)]
}

/// Groups new CAD objects so they select together (Leader lines, CAD blocks).
pub fn group_cad(cx: &mut EditorContext, ids: &[Id]) {
    if ids.len() < 2 {
        return;
    }
    let members: Vec<plan_core::ObjectRef> = ids
        .iter()
        .map(|id| plan_core::ObjectRef::Cad(*id))
        .collect();
    let fl = cx.floor;
    cx.project.make_group(fl, &members);
}

/// Adds `items` to the floor on `layer` as one undo step `label`, grouping
/// them when there are several, and selects the first. Returns the ids, or
/// `None` when the layer is locked.
pub fn add_cad_items(
    cx: &mut EditorContext,
    layer: &str,
    items: Vec<CadItem>,
    label: &str,
) -> Option<Vec<Id>> {
    if items.is_empty() {
        return None;
    }
    if cx.layers().is_locked(layer) {
        cx.status = format!("The layer \"{layer}\" is locked");
        return None;
    }
    cx.begin_change(label);
    let fl = cx.floor;
    let ids: Vec<Id> = items
        .into_iter()
        .map(|it| cx.project.add_cad(fl, layer, it))
        .collect();
    group_cad(cx, &ids);
    cx.selection.set(ObjectRef::Cad(ids[0]));
    cx.mark_dirty();
    Some(ids)
}

// ----- pure geometry -----

/// Center and radius of the circle through three points.
pub fn circumcircle(a: Point, b: Point, c: Point) -> Option<(Point, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let (a2, b2, c2) = (
        a.x * a.x + a.y * a.y,
        b.x * b.x + b.y * b.y,
        c.x * c.x + c.y * c.y,
    );
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let center = Point::new(ux, uy);
    Some((center, center.dist(a)))
}

/// Three-point arc: starts at `a`, passes through `m`, ends at `b`.
pub fn arc_three_point(a: Point, m: Point, b: Point) -> Option<CadItem> {
    let (c, r) = circumcircle(a, m, b)?;
    let (a1, a2, am) = (a.sub(c).angle(), b.sub(c).angle(), m.sub(c).angle());
    let sweep = (a2 - a1).rem_euclid(TAU);
    let rel = (am - a1).rem_euclid(TAU);
    Some(if rel <= sweep {
        CadItem::Arc {
            center: c,
            radius: r,
            start_angle: a1,
            end_angle: a2,
        }
    } else {
        CadItem::Arc {
            center: c,
            radius: r,
            start_angle: a2,
            end_angle: a1,
        }
    })
}

/// Center-start-end arc: radius from `start`, counter-clockwise from the
/// start angle to the angle of `end`.
pub fn arc_center_start_end(center: Point, start: Point, end: Point) -> Option<CadItem> {
    let radius = center.dist(start);
    if radius < MIN_SIZE || center.dist(end) < MIN_SIZE {
        return None;
    }
    Some(CadItem::Arc {
        center,
        radius,
        start_angle: start.sub(center).angle(),
        end_angle: end.sub(center).angle(),
    })
}

/// Start-end-tangent arc: from `s` to `e`, leaving `s` toward `t`.
pub fn arc_start_end_tangent(s: Point, e: Point, t: Point) -> Option<CadItem> {
    let tan = t.sub(s).normalized();
    if tan.length() < 0.5 {
        return None;
    }
    let d = e.sub(s);
    let n = tan.perp();
    let denom = 2.0 * n.dot(d);
    if denom.abs() < 1e-9 {
        return None;
    }
    let r = d.dot(d) / denom;
    let center = s.add(n.scale(r));
    let radial = s.sub(center);
    let ccw = radial.cross(tan) > 0.0;
    let (a_s, a_e) = (s.sub(center).angle(), e.sub(center).angle());
    let (start_angle, end_angle) = if ccw { (a_s, a_e) } else { (a_e, a_s) };
    Some(CadItem::Arc {
        center,
        radius: r.abs(),
        start_angle,
        end_angle,
    })
}

/// Catmull-Rom spline through `pts`, sampled `per_span` segments per span
/// (CAD-29). Open: `(n - 1) * per_span + 1` points; closed: `n * per_span`.
pub fn catmull_rom(pts: &[Point], closed: bool, per_span: usize) -> Vec<Point> {
    let n = pts.len();
    if n < 2 || per_span == 0 {
        return pts.to_vec();
    }
    let at = |i: isize| -> Point {
        if closed {
            pts[i.rem_euclid(n as isize) as usize]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(spans * per_span + 1);
    for s in 0..spans as isize {
        let (p0, p1, p2, p3) = (at(s - 1), at(s), at(s + 1), at(s + 2));
        for k in 0..per_span {
            let t = k as f64 / per_span as f64;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(Point::new(
                f(p0.x, p1.x, p2.x, p3.x),
                f(p0.y, p1.y, p2.y, p3.y),
            ));
        }
    }
    if !closed {
        out.push(pts[n - 1]);
    }
    out
}

/// A closed polygon approximating an ellipse: center `c`, semi-axis `rx`
/// along unit vector `u`, semi-axis `ry` along its left normal.
pub fn ellipse_points(c: Point, u: Point, rx: f64, ry: f64) -> Vec<Point> {
    let v = u.perp();
    (0..ELLIPSE_SEGMENTS)
        .map(|i| {
            let t = TAU * i as f64 / ELLIPSE_SEGMENTS as f64;
            c.add(u.scale(rx * t.cos())).add(v.scale(ry * t.sin()))
        })
        .collect()
}

/// Corners of the axis-aligned rectangle with opposite corners `a`, `b`.
pub fn rect_corners(a: Point, b: Point) -> Vec<Point> {
    vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)]
}

/// Regular polygon with `sides` vertices, first vertex at `first`.
pub fn regular_polygon(center: Point, first: Point, sides: u32) -> Vec<Point> {
    let v = first.sub(center);
    let (sn, cs) = (
        (TAU / sides.max(3) as f64).sin(),
        (TAU / sides.max(3) as f64).cos(),
    );
    let mut cur = v;
    (0..sides.max(3))
        .map(|_| {
            let p = center.add(cur);
            cur = Point::new(cur.x * cs - cur.y * sn, cur.x * sn + cur.y * cs);
            p
        })
        .collect()
}

/// A scalloped closed outline around the polygon `poly` (CAD-36): every edge
/// is cut into chords of about `arc` inches, each bulging outward as a
/// half circle sampled with six points.
pub fn revision_cloud(poly: &[Point], arc: f64) -> Vec<Point> {
    if poly.len() < 3 {
        return poly.to_vec();
    }
    let ccw = polygon_area(poly) > 0.0;
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        let len = p.dist(q);
        if len < 1e-6 {
            continue;
        }
        let u = q.sub(p).normalized();
        // Counter-clockwise: the interior is on the left, so out is right.
        let outward = if ccw { u.perp().scale(-1.0) } else { u.perp() };
        let n = ((len / arc.max(1.0)).round() as usize).max(1);
        for k in 0..n {
            let c0 = Point::lerp(p, q, k as f64 / n as f64);
            let c1 = Point::lerp(p, q, (k + 1) as f64 / n as f64);
            let r = c0.dist(c1) * 0.5;
            let m = Point::lerp(c0, c1, 0.5);
            for s in 0..CLOUD_SAMPLES {
                let th = PI * s as f64 / CLOUD_SAMPLES as f64;
                out.push(
                    m.add(u.scale(-r * th.cos()))
                        .add(outward.scale(r * th.sin())),
                );
            }
        }
    }
    out
}

/// A point object: a small cross (and a circle for Point Marker).
pub fn point_items(p: Point, marker: bool) -> Vec<CadItem> {
    let s = POINT_SIZE;
    let mut v = vec![
        CadItem::Line {
            a: Point::new(p.x - s, p.y),
            b: Point::new(p.x + s, p.y),
        },
        CadItem::Line {
            a: Point::new(p.x, p.y - s),
            b: Point::new(p.x, p.y + s),
        },
    ];
    if marker {
        v.push(CadItem::Circle {
            center: p,
            radius: s * 1.5,
        });
    }
    v
}

fn arc_point(center: Point, radius: f64, angle: f64) -> Point {
    Point::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    )
}

// ----- modes -----

/// How the Draw Arc tool takes its clicks (CAD-7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArcMode {
    /// Start, end, then a point the arc passes through.
    ThreePoint,
    /// Center, start (radius), end (sweep counter-clockwise).
    CenterStartEnd,
    /// Start, end, then a point giving the tangent direction at the start.
    StartEndTangent,
}

impl ArcMode {
    pub const ALL: [ArcMode; 3] = [
        ArcMode::ThreePoint,
        ArcMode::CenterStartEnd,
        ArcMode::StartEndTangent,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ArcMode::ThreePoint => "Three-Point",
            ArcMode::CenterStartEnd => "Center-Start-End",
            ArcMode::StartEndTangent => "Start-End-Tangent",
        }
    }
}

/// The CAD tool variants (the Lines, Arcs, Circles, Boxes, Points and CAD
/// Block flyouts of the toolbar, and the CAD edit tools).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CadMode {
    Line,
    InputLine,
    LineArrow,
    Polyline,
    RectPolyline,
    Polygon,
    Arc,
    InputArc,
    ArcArrow,
    Circle,
    CircleAboutCenter,
    Ellipse,
    Oval,
    Spline,
    RevisionCloud,
    PlacePoint,
    InputPoint,
    PointMarker,
    MakeBlock,
    ExplodeBlock,
    /// Boxes: click an edge, then the depth.
    Box,
    CrossBox,
    BlockingBox,
    Insulation,
    /// Removes the points Place Point and Input Point dropped.
    DeleteTempPoints,
    AddInsertionPoint,
    AddBackoffPoint,
    EditBlock,
    BlockManagement,
    InsertBlock,
    Fillet,
    Chamfer,
    Offset,
    Trim,
    Extend,
    BreakLine,
    ReverseDirection,
    MakeParallel,
    MakePerpendicular,
    ConvertToPolyline,
    ConvertToSpline,
    PolylineToLines,
    Hatch,
    DetailFromView,
}

impl CadMode {
    pub const ALL: [CadMode; 44] = [
        CadMode::Line,
        CadMode::InputLine,
        CadMode::LineArrow,
        CadMode::Polyline,
        CadMode::RectPolyline,
        CadMode::Polygon,
        CadMode::Arc,
        CadMode::InputArc,
        CadMode::ArcArrow,
        CadMode::Circle,
        CadMode::CircleAboutCenter,
        CadMode::Ellipse,
        CadMode::Oval,
        CadMode::Spline,
        CadMode::RevisionCloud,
        CadMode::PlacePoint,
        CadMode::InputPoint,
        CadMode::PointMarker,
        CadMode::MakeBlock,
        CadMode::ExplodeBlock,
        CadMode::Box,
        CadMode::CrossBox,
        CadMode::BlockingBox,
        CadMode::Insulation,
        CadMode::DeleteTempPoints,
        CadMode::AddInsertionPoint,
        CadMode::AddBackoffPoint,
        CadMode::EditBlock,
        CadMode::BlockManagement,
        CadMode::InsertBlock,
        CadMode::Fillet,
        CadMode::Chamfer,
        CadMode::Offset,
        CadMode::Trim,
        CadMode::Extend,
        CadMode::BreakLine,
        CadMode::ReverseDirection,
        CadMode::MakeParallel,
        CadMode::MakePerpendicular,
        CadMode::ConvertToPolyline,
        CadMode::ConvertToSpline,
        CadMode::PolylineToLines,
        CadMode::Hatch,
        CadMode::DetailFromView,
    ];

    /// Chief's name from the toolbar flyouts.
    pub fn name(self) -> &'static str {
        match self {
            CadMode::Line => "Draw Line",
            CadMode::InputLine => "Input Line",
            CadMode::LineArrow => "Line With Arrow",
            CadMode::Polyline => "Polyline",
            CadMode::RectPolyline => "Rectangular Polyline",
            CadMode::Polygon => "Regular Polygon",
            CadMode::Arc => "Draw Arc",
            CadMode::InputArc => "Input Arc",
            CadMode::ArcArrow => "Arc With Arrow",
            CadMode::Circle => "Circle",
            CadMode::CircleAboutCenter => "Circle About Center",
            CadMode::Ellipse => "Ellipse",
            CadMode::Oval => "Oval",
            CadMode::Spline => "Spline",
            CadMode::RevisionCloud => "Revision Cloud",
            CadMode::PlacePoint => "Place Point",
            CadMode::InputPoint => "Input Point",
            CadMode::PointMarker => "Point Marker",
            CadMode::MakeBlock => "Make CAD Block",
            CadMode::ExplodeBlock => "Explode CAD Block",
            CadMode::Box => "Box",
            CadMode::CrossBox => "Cross Box",
            CadMode::BlockingBox => "Blocking Box",
            CadMode::Insulation => "Insulation",
            CadMode::DeleteTempPoints => "Delete Temporary Points",
            CadMode::AddInsertionPoint => "Add Insertion Point",
            CadMode::AddBackoffPoint => "Add Arrow Backoff Point",
            CadMode::EditBlock => "Edit CAD Block",
            CadMode::BlockManagement => "CAD Block Management",
            CadMode::InsertBlock => "Insert CAD Block",
            CadMode::Fillet => "Fillet",
            CadMode::Chamfer => "Chamfer",
            CadMode::Offset => "Offset",
            CadMode::Trim => "Trim Line",
            CadMode::Extend => "Extend Line",
            CadMode::BreakLine => "Break Line",
            CadMode::ReverseDirection => "Reverse Direction",
            CadMode::MakeParallel => "Make Parallel",
            CadMode::MakePerpendicular => "Make Perpendicular",
            CadMode::ConvertToPolyline => "Convert to Polyline",
            CadMode::ConvertToSpline => "Convert to Spline",
            CadMode::PolylineToLines => "Convert Polyline to Lines",
            CadMode::Hatch => "Hatch",
            CadMode::DetailFromView => "CAD Detail From View",
        }
    }

    pub fn from_name(name: &str) -> Option<CadMode> {
        CadMode::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }

    fn short(self) -> &'static str {
        match self {
            CadMode::Line => "Line",
            CadMode::InputLine => "Input Line",
            CadMode::LineArrow => "Line Arrow",
            CadMode::Polyline => "Polyline",
            CadMode::RectPolyline => "Rectangle",
            CadMode::Polygon => "Polygon",
            CadMode::Arc => "Arc",
            CadMode::InputArc => "Input Arc",
            CadMode::ArcArrow => "Arc Arrow",
            CadMode::Circle => "Circle",
            CadMode::CircleAboutCenter => "Circle Ctr",
            CadMode::Ellipse => "Ellipse",
            CadMode::Oval => "Oval",
            CadMode::Spline => "Spline",
            CadMode::RevisionCloud => "Rev. Cloud",
            CadMode::PlacePoint => "Point",
            CadMode::InputPoint => "Input Point",
            CadMode::PointMarker => "Point Marker",
            CadMode::MakeBlock => "Make Block",
            CadMode::ExplodeBlock => "Explode Block",
            CadMode::Box => "Box",
            CadMode::CrossBox => "Cross Box",
            CadMode::BlockingBox => "Blocking Box",
            CadMode::Insulation => "Insulation",
            CadMode::DeleteTempPoints => "Delete Temp Points",
            CadMode::AddInsertionPoint => "Insertion Pt",
            CadMode::AddBackoffPoint => "Backoff Pt",
            CadMode::EditBlock => "Edit Block",
            CadMode::BlockManagement => "Blocks...",
            CadMode::InsertBlock => "Insert Block",
            CadMode::Fillet => "Fillet",
            CadMode::Chamfer => "Chamfer",
            CadMode::Offset => "Offset",
            CadMode::Trim => "Trim",
            CadMode::Extend => "Extend",
            CadMode::BreakLine => "Break",
            CadMode::ReverseDirection => "Reverse",
            CadMode::MakeParallel => "Parallel",
            CadMode::MakePerpendicular => "Perpendicular",
            CadMode::ConvertToPolyline => "To Polyline",
            CadMode::ConvertToSpline => "To Spline",
            CadMode::PolylineToLines => "To Lines",
            CadMode::Hatch => "Hatch",
            CadMode::DetailFromView => "Detail From View",
        }
    }

    /// Clicks that complete the shape; `None` for variable-length modes, the
    /// typed ones and the edit tools.
    fn clicks(self) -> Option<usize> {
        match self {
            CadMode::Line
            | CadMode::InputLine
            | CadMode::LineArrow
            | CadMode::RectPolyline
            | CadMode::Polygon
            | CadMode::Circle
            | CadMode::CircleAboutCenter
            | CadMode::Oval => Some(2),
            CadMode::Arc
            | CadMode::ArcArrow
            | CadMode::Ellipse
            | CadMode::Box
            | CadMode::CrossBox
            | CadMode::BlockingBox
            | CadMode::Insulation => Some(3),
            CadMode::PlacePoint | CadMode::PointMarker | CadMode::InputPoint => Some(1),
            _ => None,
        }
    }

    fn is_line(self) -> bool {
        matches!(
            self,
            CadMode::Line | CadMode::InputLine | CadMode::LineArrow
        )
    }

    fn is_variable(self) -> bool {
        matches!(
            self,
            CadMode::Polyline | CadMode::Spline | CadMode::RevisionCloud
        )
    }

    /// Runs once when picked (on the selection or by opening a dialog).
    fn is_command(self) -> bool {
        matches!(
            self,
            CadMode::MakeBlock
                | CadMode::ExplodeBlock
                | CadMode::DeleteTempPoints
                | CadMode::EditBlock
                | CadMode::BlockManagement
                | CadMode::ConvertToPolyline
                | CadMode::ConvertToSpline
                | CadMode::PolylineToLines
                | CadMode::DetailFromView
        )
    }

    /// Edit tools that work on the CAD objects clicked.
    fn is_pick(self) -> bool {
        matches!(
            self,
            CadMode::AddInsertionPoint
                | CadMode::AddBackoffPoint
                | CadMode::InsertBlock
                | CadMode::Fillet
                | CadMode::Chamfer
                | CadMode::Offset
                | CadMode::Trim
                | CadMode::Extend
                | CadMode::BreakLine
                | CadMode::ReverseDirection
                | CadMode::MakeParallel
                | CadMode::MakePerpendicular
                | CadMode::Hatch
        )
    }

    /// The modes shown together in the option strip.
    fn family(self) -> &'static [CadMode] {
        use CadMode as C;
        match self {
            C::PlacePoint | C::InputPoint | C::PointMarker | C::DeleteTempPoints => &[
                C::PlacePoint,
                C::InputPoint,
                C::PointMarker,
                C::DeleteTempPoints,
            ],
            C::Line | C::InputLine | C::LineArrow | C::Polyline | C::Spline => {
                &[C::Line, C::InputLine, C::LineArrow, C::Polyline, C::Spline]
            }
            C::Arc | C::InputArc | C::ArcArrow => &[C::Arc, C::InputArc, C::ArcArrow],
            C::Circle | C::CircleAboutCenter | C::Ellipse | C::Oval => {
                &[C::Circle, C::CircleAboutCenter, C::Ellipse, C::Oval]
            }
            C::RectPolyline
            | C::Polygon
            | C::Box
            | C::CrossBox
            | C::BlockingBox
            | C::Insulation
            | C::RevisionCloud => &[
                C::RectPolyline,
                C::Box,
                C::Polygon,
                C::CrossBox,
                C::BlockingBox,
                C::Insulation,
                C::RevisionCloud,
            ],
            C::MakeBlock
            | C::ExplodeBlock
            | C::AddInsertionPoint
            | C::AddBackoffPoint
            | C::EditBlock
            | C::BlockManagement
            | C::InsertBlock => &[
                C::MakeBlock,
                C::ExplodeBlock,
                C::AddInsertionPoint,
                C::AddBackoffPoint,
                C::EditBlock,
                C::BlockManagement,
            ],
            _ => &[
                C::Fillet,
                C::Chamfer,
                C::Offset,
                C::Trim,
                C::Extend,
                C::BreakLine,
                C::ReverseDirection,
                C::MakeParallel,
                C::MakePerpendicular,
                C::ConvertToPolyline,
                C::ConvertToSpline,
                C::PolylineToLines,
                C::Hatch,
                C::DetailFromView,
            ],
        }
    }

    fn hint(self) -> &'static str {
        match self {
            CadMode::Line => {
                "Draw Line: click the start, click the end (Enter types length and angle)"
            }
            CadMode::InputLine => "Input Line: click the start, type length, Tab, angle, Enter",
            CadMode::LineArrow => "Line With Arrow: click the start, click the arrow tip",
            CadMode::Polyline => {
                "Polyline: click vertices; click the first to close; Enter or double-click ends"
            }
            CadMode::RectPolyline => "Rectangular Polyline: click two opposite corners",
            CadMode::Polygon => {
                "Regular Polygon: click the center, click a vertex (sides in the option strip)"
            }
            CadMode::Arc => "Draw Arc: click the points of the arc (mode in the option strip)",
            CadMode::InputArc => "Input Arc: click the center, type radius, start angle and sweep",
            CadMode::ArcArrow => "Arc With Arrow: three-point arc ending in an arrowhead",
            CadMode::Circle => "Circle: click the center, click a point on the circle",
            CadMode::CircleAboutCenter => {
                "Circle About Center: click the center, click or type the radius"
            }
            CadMode::Ellipse => {
                "Ellipse: click the center, an axis end, then a point for the other radius"
            }
            CadMode::Oval => "Oval: click two opposite corners of its bounding box",
            CadMode::Spline => {
                "Spline: click points; click the first to close; Enter or double-click ends"
            }
            CadMode::RevisionCloud => {
                "Revision Cloud: click the outline; click the first to close; Enter ends"
            }
            CadMode::PlacePoint => "Place Point: click to drop a temporary point",
            CadMode::InputPoint => "Input Point: type X, Tab, Y, Enter",
            CadMode::PointMarker => "Point Marker: click to drop a marked point",
            CadMode::MakeBlock => "Make CAD Block: select the CAD objects, then use this command",
            CadMode::ExplodeBlock => "Explode CAD Block: select a block, then use this command",
            CadMode::Box => "Box: click the ends of one edge, then click the depth",
            CadMode::CrossBox => "Cross Box: click the ends of one edge, then click the depth",
            CadMode::BlockingBox => {
                "Blocking Box: click the ends of one edge, then click the depth"
            }
            CadMode::Insulation => {
                "Insulation: click the ends of one edge, then click the thickness"
            }
            CadMode::DeleteTempPoints => "Delete Temporary Points: removes every temporary point",
            CadMode::AddInsertionPoint => {
                "Add Insertion Point: click a CAD block, then click its insertion point"
            }
            CadMode::AddBackoffPoint => {
                "Add Arrow Backoff Point: click a CAD block, then click where arrows stop"
            }
            CadMode::EditBlock => "Edit CAD Block: select a block to edit its name and points",
            CadMode::BlockManagement => "CAD Block Management: rename, insert, edit or delete blocks",
            CadMode::InsertBlock => "Insert CAD Block: click where the block's insertion point goes",
            CadMode::Fillet => {
                "Fillet: click two lines (or a polyline corner); Enter types the radius"
            }
            CadMode::Chamfer => {
                "Chamfer: click two lines (or a polyline corner); Enter types the distances"
            }
            CadMode::Offset => {
                "Offset: click an object, then the side; Enter types a distance (0 = through the click)"
            }
            CadMode::Trim => "Trim Line: click the part of a line to remove at its nearest cutters",
            CadMode::Extend => "Extend Line: click the end of a line to extend it to the next object",
            CadMode::BreakLine => "Break Line: click the point where a line or polyline splits",
            CadMode::ReverseDirection => "Reverse Direction: click a line or polyline to reverse it",
            CadMode::MakeParallel => {
                "Make Parallel: click the end of a line to turn, then the line to match"
            }
            CadMode::MakePerpendicular => {
                "Make Perpendicular: click the end of a line to turn, then the line to square to"
            }
            CadMode::ConvertToPolyline => {
                "Convert to Polyline: select connected lines, then use this command"
            }
            CadMode::ConvertToSpline => {
                "Convert to Spline: select polylines, then use this command"
            }
            CadMode::PolylineToLines => {
                "Convert Polyline to Lines: select polylines, then use this command"
            }
            CadMode::Hatch => {
                "Hatch: click inside a closed polyline or circle (pattern in the option strip)"
            }
            CadMode::DetailFromView => {
                "CAD Detail From View: copies this view's lines into a new CAD Detail floor"
            }
        }
    }
}

// ----- typed input -----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TypedKind {
    Line,
    Arc,
    Point,
    Radius,
    /// Fillet radius, chamfer distances or the offset distance.
    Setting,
}

/// The inline replacement for Chief's Input Line / Input Arc / Input Point
/// dialogs: labelled fields typed into one after the other.
#[derive(Clone, Debug)]
struct Typed {
    kind: TypedKind,
    /// `(label, text, is_angle)`
    fields: Vec<(&'static str, String, bool)>,
    active: usize,
}

impl Typed {
    fn new(kind: TypedKind, fields: Vec<(&'static str, String, bool)>) -> Self {
        Self {
            kind,
            fields,
            active: 0,
        }
    }

    fn value(&self, i: usize) -> Option<f64> {
        let (_, text, angle) = self.fields.get(i)?;
        if *angle {
            text.trim().trim_end_matches('\u{b0}').trim().parse().ok()
        } else {
            parse_ft_in(text)
        }
    }

    fn display(&self) -> String {
        self.fields
            .iter()
            .enumerate()
            .map(|(i, (label, text, _))| {
                let cursor = if i == self.active { "|" } else { "" };
                format!("{label}: {text}{cursor}")
            })
            .collect::<Vec<_>>()
            .join("   ")
    }
}

// ----- edit handles of a selected CAD object (CAD-9, CAD-16) -----

#[derive(Clone, Copy, PartialEq, Debug)]
enum HKind {
    Move,
    A,
    B,
    Vertex(usize),
    /// Midpoint of segment `i`: dragging inserts a vertex after `i`.
    Mid(usize),
    Center,
    Radius,
    ArcStart,
    ArcEnd,
}

#[derive(Clone, Copy, Debug)]
struct CHandle {
    kind: HKind,
    pos: Point,
}

fn handles_of(item: &CadItem) -> Vec<CHandle> {
    let h = |kind, pos| CHandle { kind, pos };
    match item {
        CadItem::Line { a, b } => vec![
            h(HKind::A, *a),
            h(HKind::B, *b),
            h(HKind::Move, Point::lerp(*a, *b, 0.5)),
        ],
        CadItem::Polyline { points, closed } => {
            let mut v: Vec<CHandle> = points
                .iter()
                .enumerate()
                .map(|(i, p)| h(HKind::Vertex(i), *p))
                .collect();
            let segs = if *closed {
                points.len()
            } else {
                points.len().saturating_sub(1)
            };
            for i in 0..segs {
                let (p, q) = (points[i], points[(i + 1) % points.len()]);
                v.push(h(HKind::Mid(i), Point::lerp(p, q, 0.5)));
            }
            v
        }
        CadItem::Circle { center, radius } => vec![
            h(HKind::Center, *center),
            h(HKind::Radius, Point::new(center.x + radius, center.y)),
        ],
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            vec![
                h(HKind::ArcStart, arc_point(*center, *radius, *start_angle)),
                h(HKind::ArcEnd, arc_point(*center, *radius, *end_angle)),
                h(
                    HKind::Radius,
                    arc_point(*center, *radius, start_angle + sweep * 0.5),
                ),
                h(HKind::Move, *center),
            ]
        }
        CadItem::Text { .. } => vec![h(HKind::Move, ops::cad_center(item))],
    }
}

/// Applies a handle drag: `to` is the pointer, `delta` the movement since the
/// last event (for Move).
fn drag_handle(item: &mut CadItem, kind: HKind, to: Point, delta: Point) {
    match (item, kind) {
        (it, HKind::Move) => ops::translate_cad(it, delta),
        (CadItem::Line { a, .. }, HKind::A) => *a = to,
        (CadItem::Line { b, .. }, HKind::B) => *b = to,
        (CadItem::Polyline { points, .. }, HKind::Vertex(i)) => {
            if let Some(p) = points.get_mut(i) {
                *p = to;
            }
        }
        (CadItem::Circle { center, .. }, HKind::Center) => *center = to,
        (CadItem::Circle { center, radius }, HKind::Radius) => {
            *radius = center.dist(to).max(MIN_SIZE);
        }
        (CadItem::Arc { center, radius, .. }, HKind::Radius) => {
            *radius = center.dist(to).max(MIN_SIZE);
        }
        (
            CadItem::Arc {
                center,
                start_angle,
                ..
            },
            HKind::ArcStart,
        ) => *start_angle = to.sub(*center).angle(),
        (
            CadItem::Arc {
                center, end_angle, ..
            },
            HKind::ArcEnd,
        ) => {
            *end_angle = to.sub(*center).angle();
        }
        _ => {}
    }
}

/// A press on a handle that has not moved yet: a drag edits the object, a
/// plain click is an ordinary click (so a new line can start at the end of
/// the line just drawn).
struct Grab {
    id: Id,
    kind: HKind,
    pos: Point,
    event: PointerEvent,
}

struct HandleDrag {
    id: Id,
    kind: HKind,
    last: Point,
    changed: bool,
}

// ----- the tool -----

pub struct CadTool {
    mode: CadMode,
    arc_mode: ArcMode,
    sides: u32,
    pts: Vec<Point>,
    hover: Option<Point>,
    press: Option<Pos2>,
    typed: Option<Typed>,
    drag: Option<HandleDrag>,
    grab: Option<Grab>,
    strip: OptionStrip,
    /// Settings and state of the edit tools, blocks and hatching.
    edit: EditState,
}

impl Default for CadTool {
    fn default() -> Self {
        Self {
            mode: CadMode::Line,
            arc_mode: ArcMode::ThreePoint,
            sides: 6,
            pts: Vec::new(),
            hover: None,
            press: None,
            typed: None,
            drag: None,
            grab: None,
            strip: OptionStrip::default(),
            edit: EditState::default(),
        }
    }
}

const BTN_ARC_MODE: u16 = 100;
const BTN_SIDES_MINUS: u16 = 110;
const BTN_SIDES_PLUS: u16 = 111;
const BTN_SET_MINUS: u16 = 120;
const BTN_SET_LABEL: u16 = 121;
const BTN_SET_PLUS: u16 = 122;
const BTN_HATCH_NEXT: u16 = 123;
const BTN_SPLINE_FIT: u16 = 124;
const BTN_SPLINE_BEZIER: u16 = 125;

impl CadTool {
    pub fn mode(&self) -> CadMode {
        self.mode
    }

    pub fn arc_mode(&self) -> ArcMode {
        self.arc_mode
    }

    pub fn set_arc_mode(&mut self, m: ArcMode) {
        self.arc_mode = m;
        self.pts.clear();
    }

    pub fn set_sides(&mut self, n: u32) {
        self.sides = n.clamp(3, 64);
    }

    /// Switches the variant (a flyout entry). Drops any shape in progress.
    pub fn set_mode(&mut self, mode: CadMode) {
        self.mode = mode;
        self.pts.clear();
        self.press = None;
        self.typed = None;
        self.drag = None;
        self.grab = None;
        self.edit.pick = None;
        self.edit.block = None;
        self.edit.pending = mode.is_command();
    }

    /// Switches the variant by Chief's name; false when the name is unknown.
    pub fn set_mode_by_name(&mut self, name: &str) -> bool {
        match CadMode::from_name(name) {
            Some(m) => {
                self.set_mode(m);
                true
            }
            None => false,
        }
    }

    /// Clicked points of the shape in progress.
    pub fn pending(&self) -> &[Point] {
        &self.pts
    }

    /// The point angle snaps measure from: the last clicked point, for the
    /// tools that draw along a direction.
    fn origin(&self) -> Option<Point> {
        if self.mode.is_line() || self.mode.is_variable() || self.mode == CadMode::Polygon {
            self.pts.last().copied()
        } else {
            None
        }
    }

    fn idle(&self) -> bool {
        self.pts.is_empty() && self.typed.is_none()
    }

    fn cancel(&mut self, cx: &mut EditorContext) {
        self.pts.clear();
        self.press = None;
        self.grab = None;
        self.typed = None;
        set_typing(cx, false);
        cx.readout = None;
    }

    fn snap(&self, cx: &EditorContext, p: &PointerEvent) -> Point {
        cx.snap_at(p.world, self.origin(), p.modifiers.alt, &[])
            .point
    }

    /// The items the click points `pts` (the last one being the pointer or
    /// the click just made) describe in the current mode.
    fn items_for(&self, pts: &[Point]) -> Option<Vec<CadItem>> {
        let first = *pts.first()?;
        let last = *pts.last()?;
        let two = pts.len() >= 2;
        match self.mode {
            CadMode::Line | CadMode::InputLine if two => {
                (first.dist(last) >= MIN_SIZE).then(|| vec![CadItem::Line { a: first, b: last }])
            }
            CadMode::LineArrow if two => {
                if first.dist(last) < MIN_SIZE {
                    return None;
                }
                Some(vec![
                    CadItem::Line { a: first, b: last },
                    CadItem::Polyline {
                        points: arrowhead(last, first, first.dist(last).min(6.0) * 0.5 + 1.5),
                        closed: true,
                    },
                ])
            }
            CadMode::Polyline if two => Some(vec![CadItem::Polyline {
                points: pts.to_vec(),
                closed: false,
            }]),
            CadMode::Spline if two => Some(vec![CadItem::Polyline {
                points: self.spline_points(pts, false),
                closed: false,
            }]),
            CadMode::Box | CadMode::CrossBox | CadMode::BlockingBox | CadMode::Insulation
                if two =>
            {
                if pts.len() == 2 {
                    return (first.dist(last) >= MIN_SIZE)
                        .then(|| vec![CadItem::Line { a: first, b: last }]);
                }
                let corners = plan_core::cad::oriented_box(pts[0], pts[1], pts[2]);
                if corners.is_empty() {
                    return None;
                }
                Some(match self.mode {
                    CadMode::CrossBox => plan_core::cad::cross_box_items(&corners),
                    CadMode::BlockingBox => plan_core::cad::blocking_box_items(&corners),
                    CadMode::Insulation => plan_core::cad::insulation_items(&corners),
                    _ => vec![CadItem::Polyline {
                        points: corners,
                        closed: true,
                    }],
                })
            }
            CadMode::RevisionCloud if pts.len() >= 3 => Some(vec![CadItem::Polyline {
                points: revision_cloud(pts, CLOUD_ARC),
                closed: true,
            }]),
            CadMode::RevisionCloud if two => Some(vec![CadItem::Polyline {
                points: pts.to_vec(),
                closed: false,
            }]),
            CadMode::RectPolyline if two => {
                let ok =
                    (first.x - last.x).abs() >= MIN_SIZE && (first.y - last.y).abs() >= MIN_SIZE;
                ok.then(|| {
                    vec![CadItem::Polyline {
                        points: rect_corners(first, last),
                        closed: true,
                    }]
                })
            }
            CadMode::Polygon if two => (first.dist(last) >= MIN_SIZE).then(|| {
                vec![CadItem::Polyline {
                    points: regular_polygon(first, last, self.sides),
                    closed: true,
                }]
            }),
            CadMode::Circle | CadMode::CircleAboutCenter if two => (first.dist(last) >= MIN_SIZE)
                .then(|| {
                    vec![CadItem::Circle {
                        center: first,
                        radius: first.dist(last),
                    }]
                }),
            CadMode::Oval if two => {
                let rx = (last.x - first.x).abs() * 0.5;
                let ry = (last.y - first.y).abs() * 0.5;
                (rx >= MIN_SIZE && ry >= MIN_SIZE).then(|| {
                    vec![CadItem::Polyline {
                        points: ellipse_points(
                            Point::lerp(first, last, 0.5),
                            Point::new(1.0, 0.0),
                            rx,
                            ry,
                        ),
                        closed: true,
                    }]
                })
            }
            CadMode::Ellipse if two => {
                let axis = last.sub(first);
                let rx = axis.length();
                if rx < MIN_SIZE {
                    return None;
                }
                if pts.len() == 2 {
                    // Preview: a circle until the second radius is clicked.
                    return Some(vec![CadItem::Polyline {
                        points: ellipse_points(first, axis.normalized(), rx, rx * 0.5),
                        closed: true,
                    }]);
                }
                let u = pts[1].sub(first).normalized();
                let ry = last.sub(first).dot(u.perp()).abs();
                (ry >= MIN_SIZE).then(|| {
                    vec![CadItem::Polyline {
                        points: ellipse_points(first, u, pts[1].sub(first).length(), ry),
                        closed: true,
                    }]
                })
            }
            CadMode::Arc | CadMode::ArcArrow => self.arc_items(pts),
            CadMode::PlacePoint => Some(point_items(first, false)),
            CadMode::PointMarker => Some(point_items(first, true)),
            _ => None,
        }
    }

    /// The arc (and its arrowhead) for the clicked points in the current arc
    /// mode. Two points preview as the start-end line.
    fn arc_items(&self, pts: &[Point]) -> Option<Vec<CadItem>> {
        if pts.len() == 2 {
            return Some(vec![CadItem::Line {
                a: pts[0],
                b: pts[1],
            }]);
        }
        if pts.len() < 3 {
            return None;
        }
        let arc = if self.mode == CadMode::ArcArrow {
            arc_three_point(pts[0], pts[2], pts[1])
        } else {
            match self.arc_mode {
                ArcMode::ThreePoint => arc_three_point(pts[0], pts[2], pts[1]),
                ArcMode::CenterStartEnd => arc_center_start_end(pts[0], pts[1], pts[2]),
                ArcMode::StartEndTangent => arc_start_end_tangent(pts[0], pts[1], pts[2]),
            }
        }?;
        let mut items = vec![arc.clone()];
        if self.mode == CadMode::ArcArrow {
            if let CadItem::Arc {
                center,
                radius,
                end_angle,
                ..
            } = arc
            {
                let tip = arc_point(center, radius, end_angle);
                let back = arc_point(center, radius, end_angle - 0.4);
                items.push(CadItem::Polyline {
                    points: arrowhead(tip, back, (radius * 0.1).clamp(1.5, 4.0)),
                    closed: true,
                });
            }
        }
        Some(items)
    }

    /// The layer the current mode draws on: Place Point and Input Point drop
    /// temporary points on their own layer (Delete Temporary Points).
    fn draw_layer(&self, cx: &mut EditorContext) -> &'static str {
        if matches!(self.mode, CadMode::PlacePoint | CadMode::InputPoint) {
            ensure_layer(&mut cx.project, TEMP_POINT_LAYER, [200, 0, 200], 13);
            TEMP_POINT_LAYER
        } else {
            CAD_LAYER
        }
    }

    /// The curve through spline points in the chosen fit.
    fn spline_points(&self, pts: &[Point], closed: bool) -> Vec<Point> {
        match self.edit.spline {
            SplineKind::Fit => catmull_rom(pts, closed, SPLINE_SEGMENTS_PER_SPAN),
            SplineKind::Bezier => plan_core::cad::bezier_spline(
                pts,
                closed,
                SPLINE_SEGMENTS_PER_SPAN,
                self.edit.tension,
            ),
        }
    }

    fn commit(&mut self, cx: &mut EditorContext, items: Vec<CadItem>) -> ToolResult {
        let label = format!("Draw {}", self.mode.short());
        let layer = self.draw_layer(cx);
        match add_cad_items(cx, layer, items, &label) {
            Some(_) => {
                cx.status.clear();
                cx.readout = None;
                ToolResult::committed(&label)
            }
            None => ToolResult::consumed(),
        }
    }

    /// Completes the shape from the clicked points; chains lines when
    /// Connect CAD Segments is on (CAD-3).
    fn complete(&mut self, cx: &mut EditorContext, pts: &[Point]) -> ToolResult {
        let Some(items) = self.items_for(pts) else {
            return ToolResult::consumed();
        };
        let last = *pts.last().expect("points");
        let res = self.commit(cx, items);
        self.pts.clear();
        self.press = None;
        if res.commit.is_some()
            && self.mode.is_line()
            && cx.view_flags.contains(&ViewFlag::ConnectCad)
        {
            self.pts.push(last);
        }
        if res.commit.is_some() && self.mode == CadMode::InputPoint {
            self.start_typed_point(cx);
        }
        res
    }

    /// Ends a polyline / spline / cloud with the points clicked so far.
    fn finish_variable(&mut self, cx: &mut EditorContext, closed: bool) -> ToolResult {
        let pts = std::mem::take(&mut self.pts);
        self.press = None;
        let min = if self.mode == CadMode::RevisionCloud {
            3
        } else {
            2
        };
        if pts.len() < min {
            cx.readout = None;
            return ToolResult::consumed();
        }
        let item = match self.mode {
            CadMode::Polyline => CadItem::Polyline {
                points: pts,
                closed,
            },
            CadMode::Spline => CadItem::Polyline {
                points: self.spline_points(&pts, closed),
                closed,
            },
            _ => CadItem::Polyline {
                points: revision_cloud(&pts, CLOUD_ARC),
                closed: true,
            },
        };
        self.commit(cx, vec![item])
    }

    fn start_typed_line(&mut self, cx: &mut EditorContext) {
        let (Some(start), Some(h)) = (self.pts.last().copied(), self.hover) else {
            return;
        };
        let angle = h.sub(start).angle().to_degrees();
        self.typed = Some(Typed::new(
            TypedKind::Line,
            vec![
                ("Length", String::new(), false),
                ("Angle", format!("{angle:.1}"), true),
            ],
        ));
        set_typing(cx, true);
    }

    fn start_typed_point(&mut self, cx: &mut EditorContext) {
        self.typed = Some(Typed::new(
            TypedKind::Point,
            vec![("X", String::new(), false), ("Y", String::new(), false)],
        ));
        set_typing(cx, true);
    }

    fn start_typed_arc(&mut self, cx: &mut EditorContext) {
        self.typed = Some(Typed::new(
            TypedKind::Arc,
            vec![
                ("Radius", String::new(), false),
                ("Start angle", "0".into(), true),
                ("Sweep", "90".into(), true),
            ],
        ));
        set_typing(cx, true);
    }

    fn start_typed_radius(&mut self, cx: &mut EditorContext) {
        self.typed = Some(Typed::new(
            TypedKind::Radius,
            vec![("Radius", String::new(), false)],
        ));
        set_typing(cx, true);
    }

    /// Enter in the typed fields: builds the shape.
    fn commit_typed(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(t) = self.typed.clone() else {
            return ToolResult::ignored();
        };
        let bad = |cx: &mut EditorContext| {
            cx.status = "Enter a valid number".into();
            ToolResult::consumed()
        };
        match t.kind {
            TypedKind::Line => {
                let (Some(len), Some(ang), Some(start)) =
                    (t.value(0), t.value(1), self.pts.last().copied())
                else {
                    return bad(cx);
                };
                if len < MIN_SIZE {
                    return bad(cx);
                }
                let end = start
                    .add(Point::new(ang.to_radians().cos(), ang.to_radians().sin()).scale(len));
                self.typed = None;
                set_typing(cx, false);
                self.complete(cx, &[start, end])
            }
            TypedKind::Radius => {
                let (Some(r), Some(c)) = (t.value(0), self.pts.first().copied()) else {
                    return bad(cx);
                };
                if r < MIN_SIZE {
                    return bad(cx);
                }
                self.typed = None;
                set_typing(cx, false);
                self.pts.clear();
                self.commit(
                    cx,
                    vec![CadItem::Circle {
                        center: c,
                        radius: r,
                    }],
                )
            }
            TypedKind::Arc => {
                let (Some(r), Some(a0), Some(sweep), Some(c)) = (
                    t.value(0),
                    t.value(1),
                    t.value(2),
                    self.pts.first().copied(),
                ) else {
                    return bad(cx);
                };
                if r < MIN_SIZE || sweep.abs() < 0.01 {
                    return bad(cx);
                }
                let (s, e) = if sweep >= 0.0 {
                    (a0, a0 + sweep)
                } else {
                    (a0 + sweep, a0)
                };
                self.typed = None;
                set_typing(cx, false);
                self.pts.clear();
                self.commit(
                    cx,
                    vec![CadItem::Arc {
                        center: c,
                        radius: r,
                        start_angle: s.to_radians(),
                        end_angle: e.to_radians(),
                    }],
                )
            }
            TypedKind::Setting => self.commit_setting(cx, &t),
            TypedKind::Point => {
                let (Some(x), Some(y)) = (t.value(0), t.value(1)) else {
                    return bad(cx);
                };
                self.typed = None;
                set_typing(cx, false);
                let items = point_items(Point::new(x, y), false);
                let res = self.commit(cx, items);
                self.start_typed_point(cx);
                res
            }
        }
    }

    fn typed_key(&mut self, cx: &mut EditorContext, k: &KeyEvent) -> ToolResult {
        if k.is(egui::Key::Escape) {
            self.typed = None;
            set_typing(cx, false);
            if self.mode == CadMode::InputPoint {
                self.start_typed_point(cx);
            }
            return ToolResult::consumed();
        }
        if k.is(egui::Key::Enter) {
            return self.commit_typed(cx);
        }
        let Some(t) = self.typed.as_mut() else {
            return ToolResult::ignored();
        };
        if k.is(egui::Key::Tab) {
            t.active = (t.active + 1) % t.fields.len();
        } else if k.is(egui::Key::Backspace) {
            t.fields[t.active].1.pop();
        } else if let Some(s) = &k.text {
            let angle = t.fields[t.active].2;
            let ok = |c: char| {
                c.is_ascii_digit()
                    || matches!(c, '.' | '-' | '/' | ' ')
                    || (!angle && matches!(c, '\'' | '"'))
            };
            t.fields[t.active].1.extend(s.chars().filter(|c| ok(*c)));
        } else {
            return ToolResult::ignored();
        }
        cx.readout = Some(t.display());
        ToolResult::consumed()
    }

    // ----- handles -----

    fn selected_cad(&self, cx: &EditorContext) -> Option<(Id, CadItem)> {
        match cx.selection.single()? {
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                cad_by_id(cx.floor(), id).map(|c| (id, c.item.clone()))
            }
            _ => None,
        }
    }

    fn grab_handle(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> bool {
        let Some((id, item)) = self.selected_cad(cx) else {
            return false;
        };
        let tol = cx.pick_tol();
        let hit = handles_of(&item)
            .into_iter()
            .filter(|h| h.pos.dist(p.world) <= tol)
            .min_by(|a, b| {
                // Vertices and ends before the middle handles.
                let da = a.pos.dist(p.world)
                    + f64::from(matches!(a.kind, HKind::Move | HKind::Mid(_))) * 1e-6;
                let db = b.pos.dist(p.world)
                    + f64::from(matches!(b.kind, HKind::Move | HKind::Mid(_))) * 1e-6;
                da.total_cmp(&db)
            });
        let Some(h) = hit else { return false };
        self.grab = Some(Grab {
            id,
            kind: h.kind,
            pos: h.pos,
            event: *p,
        });
        true
    }

    /// The grabbed handle moved: starts editing the object.
    fn begin_drag(&mut self, cx: &mut EditorContext, g: Grab) {
        if !cx.check_unlocked(ObjectRef::Cad(g.id)) {
            return;
        }
        cx.begin_change("Edit CAD Object");
        let mut kind = g.kind;
        let mut changed = false;
        if let HKind::Mid(i) = g.kind {
            let fl = cx.floor;
            if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == g.id) {
                if let CadItem::Polyline { points, .. } = &mut c.item {
                    points.insert(i + 1, g.pos);
                    kind = HKind::Vertex(i + 1);
                    changed = true;
                }
            }
            cx.mark_dirty();
        }
        self.drag = Some(HandleDrag {
            id: g.id,
            kind,
            last: g.event.world,
            changed,
        });
    }

    fn drag_to(&mut self, cx: &mut EditorContext, p: &PointerEvent) {
        let Some(d) = self.drag.as_mut() else { return };
        let to = p.snapped;
        let delta = p.world.sub(d.last);
        d.last = p.world;
        let fl = cx.floor;
        if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == d.id) {
            drag_handle(
                &mut c.item,
                d.kind,
                if d.kind == HKind::Move { p.world } else { to },
                delta,
            );
            d.changed = true;
        }
        cx.mark_dirty();
    }

    // ----- block commands (CAD-31, CAD-32) -----

    fn make_block(&mut self, cx: &mut EditorContext) -> ToolResult {
        let ids: Vec<Id> = cx
            .selection
            .items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Cad(id) | ObjectRef::Text(id) => Some(*id),
                _ => None,
            })
            .collect();
        if ids.len() < 2 {
            cx.status = "Select two or more CAD objects to make a CAD block".into();
            return ToolResult::consumed();
        }
        cx.begin_change("Make CAD Block");
        let fl = cx.floor;
        if cx.project.make_cad_block(fl, &ids, None).is_none() {
            cx.cancel_change();
            cx.status = "Those objects cannot be made into a CAD block".into();
            return ToolResult::consumed();
        }
        cx.mark_dirty();
        cx.status = format!("Made a CAD block of {} objects", ids.len());
        ToolResult::committed("Make CAD Block")
    }

    fn explode_block(&mut self, cx: &mut EditorContext) -> ToolResult {
        let fl = cx.floor;
        let mut groups: Vec<Id> = Vec::new();
        for o in &cx.selection.items {
            if let ObjectRef::Cad(id) | ObjectRef::Text(id) = o {
                if let Some(g) = cx.floor().group_of(plan_core::ObjectRef::Cad(*id)) {
                    if !groups.contains(&g.id) {
                        groups.push(g.id);
                    }
                }
            }
        }
        if groups.is_empty() {
            cx.status = "Select a CAD block to explode".into();
            return ToolResult::consumed();
        }
        cx.begin_change("Explode CAD Block");
        let mut freed = Vec::new();
        for g in groups {
            let members = if cx.floor().cad_block(g).is_some() {
                cx.project
                    .explode_cad_block(fl, g)
                    .map(|ids| ids.into_iter().map(plan_core::ObjectRef::Cad).collect())
            } else {
                cx.project.explode_group(fl, g)
            };
            freed.extend(members.unwrap_or_default());
        }
        cx.selection.items = freed
            .into_iter()
            .filter_map(|m| match m {
                plan_core::ObjectRef::Cad(id) => Some(ObjectRef::Cad(id)),
                _ => None,
            })
            .collect();
        cx.mark_dirty();
        cx.status = format!("Exploded into {} objects", cx.selection.len());
        ToolResult::committed("Explode CAD Block")
    }

    /// A click that places a point of the shape in progress.
    fn place_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.typed.is_some() && self.mode == CadMode::InputArc {
            return ToolResult::consumed();
        }
        if self.typed.is_some() && self.mode != CadMode::InputPoint {
            // A click while typing falls through and places the point.
            self.typed = None;
            set_typing(cx, false);
        }
        let s = self.snap(cx, &p);
        if self.pts.is_empty() {
            self.press = Some(p.screen);
        }
        // Closing a polyline, spline or cloud on its first vertex (CAD-15).
        if self.mode.is_variable()
            && self.pts.len() >= 3
            && (s.dist(self.pts[0]) <= cx.snap_tol() || p.world.dist(self.pts[0]) <= cx.snap_tol())
        {
            return self.finish_variable(cx, true);
        }
        self.pts.push(s);
        match self.mode.clicks() {
            Some(n) if self.pts.len() >= n => {
                let pts = std::mem::take(&mut self.pts);
                self.complete(cx, &pts)
            }
            _ => {
                match self.mode {
                    CadMode::InputLine if self.pts.len() == 1 => {
                        self.hover = Some(s);
                        self.start_typed_line(cx);
                    }
                    CadMode::InputArc if self.pts.len() == 1 => self.start_typed_arc(cx),
                    CadMode::CircleAboutCenter if self.pts.len() == 1 => {
                        // Click for the radius, or Enter to type it.
                    }
                    _ => {}
                }
                self.update_readout(cx, s);
                ToolResult::consumed()
            }
        }
    }

    /// Release after a click: press-drag-release draws a two-click shape in
    /// one gesture.
    fn finish_press(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let Some(press) = self.press.take() else {
            return ToolResult::ignored();
        };
        if self.pts.len() == 1
            && self.mode.clicks() == Some(2)
            && (p.screen - press).length() >= DRAG_PX
        {
            let end = self.snap(cx, &p);
            let start = self.pts[0];
            self.pts.clear();
            self.typed = None;
            set_typing(cx, false);
            return self.complete(cx, &[start, end]);
        }
        ToolResult::consumed()
    }

    fn strip_items(&self) -> Vec<StripButton> {
        let index = |m: &CadMode| CadMode::ALL.iter().position(|x| x == m).unwrap_or(0) as u16;
        let mut v: Vec<StripButton> = self
            .mode
            .family()
            .iter()
            .map(|m| StripButton::new(m.short(), index(m), *m == self.mode))
            .collect();
        v.extend(self.setting_buttons());
        if matches!(self.mode, CadMode::Arc) {
            for (i, m) in ArcMode::ALL.iter().enumerate() {
                v.push(StripButton::new(
                    m.name(),
                    BTN_ARC_MODE + i as u16,
                    *m == self.arc_mode,
                ));
            }
        }
        if self.mode == CadMode::Polygon {
            v.push(StripButton::new("Sides \u{2212}", BTN_SIDES_MINUS, false));
            v.push(StripButton::new(
                format!("{} sides", self.sides),
                BTN_SIDES_PLUS + 1,
                true,
            ));
            v.push(StripButton::new("Sides +", BTN_SIDES_PLUS, false));
        }
        v
    }

    fn strip_click(&mut self, cx: &mut EditorContext, id: u16) -> ToolResult {
        if let Some(m) = CadMode::ALL.get(id as usize).copied() {
            self.cancel(cx);
            self.set_mode(m);
            cx.status = m.hint().into();
            if m == CadMode::InputPoint {
                self.start_typed_point(cx);
            }
            if m.is_command() {
                self.edit.pending = false;
                return self.run_command(cx, m);
            }
            return ToolResult::consumed();
        }
        if self.setting_click(id) {
            return ToolResult::consumed();
        }
        match id {
            i if (BTN_ARC_MODE..BTN_ARC_MODE + 3).contains(&i) => {
                self.set_arc_mode(ArcMode::ALL[(i - BTN_ARC_MODE) as usize]);
            }
            BTN_SIDES_MINUS => self.set_sides(self.sides.saturating_sub(1)),
            BTN_SIDES_PLUS => self.set_sides(self.sides + 1),
            _ => {}
        }
        ToolResult::consumed()
    }

    fn update_readout(&self, cx: &mut EditorContext, to: Point) {
        if let Some(t) = &self.typed {
            cx.readout = Some(t.display());
            return;
        }
        let Some(first) = self.pts.first().copied() else {
            cx.readout = None;
            return;
        };
        let last = self.pts.last().copied().unwrap_or(first);
        cx.readout = match self.mode {
            m if m.is_line() || m.is_variable() => {
                let ang = to.sub(last).angle().to_degrees();
                Some(format!(
                    "Length: {}  Angle: {ang:.1}\u{b0}",
                    cx.fmt_dim(last.dist(to))
                ))
            }
            CadMode::Circle | CadMode::CircleAboutCenter | CadMode::Polygon => {
                Some(format!("Radius: {}", cx.fmt_dim(first.dist(to))))
            }
            CadMode::RectPolyline | CadMode::Oval => Some(format!(
                "{} x {}",
                cx.fmt_dim((to.x - first.x).abs()),
                cx.fmt_dim((to.y - first.y).abs())
            )),
            _ => None,
        };
    }
}

impl Tool for CadTool {
    fn id(&self) -> ToolId {
        ToolId::Cad
    }

    fn name(&self) -> &'static str {
        self.mode.name()
    }

    fn hint(&self) -> String {
        self.mode.hint().into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::CadVariant(m) = id {
            self.set_mode(m);
        }
    }

    fn frame(&mut self, cx: &mut EditorContext, ctx: &egui::Context) {
        if self.edit.pending {
            self.edit.pending = false;
            let mode = self.mode;
            self.run_command(cx, mode);
            if self.edit.dialog.is_none() {
                // The command is done: back to Select Objects.
                cx.requests.push(EditorRequest::SetTool(ToolId::Select));
            }
        }
        self.frame_dialogs(cx, ctx);
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.pts.clear();
        self.typed = None;
        self.drag = None;
        self.grab = None;
        cx.status = self.mode.hint().into();
        if self.mode == CadMode::InputPoint {
            self.start_typed_point(cx);
        }
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.cancel(cx);
        self.drag = None;
        self.hover = None;
        cx.last_snap = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(g) = self.grab.take() {
            if p.down && (p.screen - g.event.screen).length() >= DRAG_PX {
                self.begin_drag(cx, g);
            } else {
                self.grab = Some(g);
            }
        }
        if self.drag.is_some() {
            self.drag_to(cx, &p);
            return ToolResult::consumed();
        }
        if self.grab.is_some() {
            return ToolResult::consumed();
        }
        let s = cx.snap_at(p.world, self.origin(), p.modifiers.alt, &[]);
        self.hover = Some(s.point);
        cx.last_snap = Some(s);
        self.update_readout(cx, s.point);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(id) = self.strip.hit(p.screen) {
            return self.strip_click(cx, id);
        }
        if self.mode.is_command() || self.edit.dialog.is_some() {
            return ToolResult::consumed();
        }
        if self.mode.is_pick() {
            return self.pick_click(cx, p);
        }
        if self.idle() {
            if self.grab_handle(cx, &p) {
                return ToolResult::consumed();
            }
            if p.modifiers.shift {
                let tol = cx.pick_tol();
                let hit = hit_test(cx.floor(), cx.layers(), p.world, tol)
                    .into_iter()
                    .find(|o| matches!(o, ObjectRef::Cad(_)));
                if let Some(o) = hit {
                    cx.selection.set(o);
                    return ToolResult::consumed();
                }
            }
        }
        self.place_click(cx, p)
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(d) = self.drag.take() {
            return if d.changed {
                cx.mark_dirty();
                ToolResult::committed("Edit CAD Object")
            } else {
                cx.cancel_change();
                ToolResult::consumed()
            };
        }
        if let Some(g) = self.grab.take() {
            // A press on a handle that never moved is an ordinary click.
            let res = self.place_click(cx, g.event);
            self.press = None;
            return res;
        }
        self.finish_press(cx, p)
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.mode.is_variable() && !self.pts.is_empty() {
            return self.finish_variable(cx, false);
        }
        if self.idle() && !self.mode.is_pick() {
            let tol = cx.pick_tol();
            let hit = hit_test(cx.floor(), cx.layers(), p.world, tol)
                .into_iter()
                .find(|o| matches!(o, ObjectRef::Cad(_)));
            if let Some(o) = hit {
                cx.selection.set(o);
                cx.requests.push(EditorRequest::OpenSpec(o));
                return ToolResult::consumed();
            }
        }
        ToolResult::ignored()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if self.typed.is_some() {
            return self.typed_key(cx, &k);
        }
        if k.is(egui::Key::Escape) {
            if !self.pts.is_empty() {
                self.cancel(cx);
                return ToolResult::consumed();
            }
            if self.edit.pick.is_some() || self.edit.block.is_some() {
                self.edit.pick = None;
                self.edit.block = None;
                cx.status = self.mode.hint().into();
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(egui::Key::Enter)
            && self.pts.is_empty()
            && matches!(
                self.mode,
                CadMode::Fillet | CadMode::Chamfer | CadMode::Offset
            )
        {
            self.start_typed_setting(cx);
            return ToolResult::consumed();
        }
        if k.is(egui::Key::Enter) && !self.pts.is_empty() {
            if self.mode.is_variable() {
                return self.finish_variable(cx, false);
            }
            if self.mode.is_line() {
                self.start_typed_line(cx);
                return ToolResult::consumed();
            }
            if matches!(self.mode, CadMode::Circle | CadMode::CircleAboutCenter) {
                self.start_typed_radius(cx);
                return ToolResult::consumed();
            }
        }
        if k.is(egui::Key::Backspace) && self.mode.is_variable() && !self.pts.is_empty() {
            self.pts.pop();
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        self.strip.draw(painter, cam, pal, &self.strip_items());
        self.draw_edit_overlay(cx, painter, cam);
        let ghost = Stroke::new(1.0_f32, pal.ghost_stroke);
        // Handles of the selected object.
        if self.idle() {
            if let Some((_, item)) = self.selected_cad(cx) {
                for h in handles_of(&item) {
                    let c = cam.world_to_screen(h.pos);
                    let sel = Stroke::new(1.5_f32, pal.selection);
                    match h.kind {
                        HKind::Mid(_) => {
                            painter.circle_filled(c, 3.5, pal.background);
                            painter.circle_stroke(c, 3.5, sel);
                        }
                        HKind::Move | HKind::Center | HKind::Radius => {
                            painter.add(Shape::convex_polygon(
                                vec![
                                    c + Vec2::new(0.0, -6.0),
                                    c + Vec2::new(6.0, 0.0),
                                    c + Vec2::new(0.0, 6.0),
                                    c + Vec2::new(-6.0, 0.0),
                                ],
                                pal.background,
                                sel,
                            ));
                        }
                        _ => {
                            let r = Rect::from_center_size(c, Vec2::splat(9.0));
                            painter.rect_filled(r, 0.0, pal.background);
                            painter.rect_stroke(r, 0.0, sel, egui::StrokeKind::Inside);
                        }
                    }
                }
            }
        }
        // The shape in progress.
        if let Some(h) = self.hover {
            if !self.pts.is_empty() {
                let mut pts = self.pts.clone();
                pts.push(h);
                if let Some(items) = self.items_for(&pts) {
                    for it in &items {
                        render::draw_cad(painter, cam, it, ghost, pal);
                    }
                } else if pts.len() >= 2 {
                    painter.line_segment(
                        [
                            cam.world_to_screen(pts[pts.len() - 2]),
                            cam.world_to_screen(h),
                        ],
                        ghost,
                    );
                }
                // The clicked skeleton of variable shapes.
                if self.mode.is_variable() && self.pts.len() >= 2 {
                    let poly: Vec<Pos2> =
                        self.pts.iter().map(|q| cam.world_to_screen(*q)).collect();
                    painter.add(Shape::line(poly, ghost));
                }
            }
        }
        for q in &self.pts {
            painter.circle_filled(cam.world_to_screen(*q), 3.0, pal.ghost_stroke);
        }
        if let Some(t) = &self.typed {
            let at = self
                .hover
                .or(self.pts.last().copied())
                .map_or(cam.rect.center(), |q| cam.world_to_screen(q));
            painter.text(
                at + Vec2::new(14.0, -14.0),
                Align2::LEFT_BOTTOM,
                format!("{}  (Tab next, Enter ok, Esc cancel)", t.display()),
                FontId::proportional(13.0),
                pal.dimension_text,
            );
        }
        if let Some(s) = cx.last_snap {
            render::draw_snap_marker(painter, cam, &s, pal.ghost_stroke);
        }
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<crate::editor::EditAction> {
        let mut v = cx.common_edit_actions();
        v.extend(edit_actions(cx));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn tool(mode: CadMode) -> CadTool {
        let mut t = CadTool::default();
        t.set_mode(mode);
        t
    }

    fn click(t: &mut CadTool, cx: &mut EditorContext, x: f64, y: f64) {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
    }

    fn cad_items(cx: &EditorContext) -> Vec<CadItem> {
        cx.floor().cad.iter().map(|c| c.item.clone()).collect()
    }

    #[test]
    fn line_click_click_creates_a_line_and_undo_removes_it() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::Line);
        click(&mut t, &mut cx, 0.0, 0.0);
        assert!(cx.floor().cad.is_empty());
        click(&mut t, &mut cx, 120.0, 0.0);
        assert_eq!(
            cad_items(&cx),
            vec![CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(120.0, 0.0)
            }]
        );
        assert_eq!(cx.floor().cad[0].layer, CAD_LAYER);
        assert!(
            t.pending().is_empty(),
            "no chain without Connect CAD Segments"
        );
        assert!(cx.undo().is_some());
        assert!(cx.floor().cad.is_empty());
    }

    #[test]
    fn line_chains_when_connect_cad_segments_is_on() {
        let mut cx = new_cx();
        cx.view_flags.insert(ViewFlag::ConnectCad);
        let mut t = tool(CadMode::Line);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 60.0, 0.0);
        click(&mut t, &mut cx, 60.0, 60.0);
        assert_eq!(cx.floor().cad.len(), 2);
        assert_eq!(t.pending().len(), 1);
        assert!(t.pending()[0].dist(Point::new(60.0, 60.0)) < 1e-6);
        t.key(&mut cx, KeyEvent::escape());
        assert!(t.pending().is_empty());
    }

    #[test]
    fn press_drag_release_draws_one_line() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::Line);
        let a = PointerEvent::at(&cx, Point::new(0.0, 0.0));
        let b = PointerEvent::at(&cx, Point::new(60.0, 0.0));
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        assert_eq!(cx.floor().cad.len(), 1);
    }

    #[test]
    fn input_line_types_length_and_angle() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::InputLine);
        click(&mut t, &mut cx, 0.0, 0.0);
        assert!(
            cx.temp.editing.is_some(),
            "typing mode asks the shell for text"
        );
        t.key(&mut cx, KeyEvent::text("10'"));
        t.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        // Replace the default angle.
        for _ in 0..8 {
            t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
        }
        t.key(&mut cx, KeyEvent::text("90"));
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert!(cx.temp.editing.is_none());
        let CadItem::Line { a, b } = &cx.floor().cad[0].item else {
            panic!("a line")
        };
        assert_eq!(*a, Point::ZERO);
        assert!((b.x).abs() < 1e-9 && (b.y - 120.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn polyline_closes_on_the_first_vertex_and_ends_on_enter() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::Polyline);
        for (x, y) in [(0.0, 0.0), (120.0, 0.0), (120.0, 96.0), (0.0, 0.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let CadItem::Polyline { points, closed } = &cx.floor().cad[0].item else {
            panic!("a polyline")
        };
        assert_eq!(points.len(), 3);
        assert!(*closed);
        for (x, y) in [(200.0, 0.0), (260.0, 0.0), (260.0, 60.0)] {
            click(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        let CadItem::Polyline { points, closed } = &cx.floor().cad[1].item else {
            panic!("a polyline")
        };
        assert_eq!(points.len(), 3);
        assert!(!*closed);
    }

    #[test]
    fn rectangular_polyline_is_a_closed_quad() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::RectPolyline);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 96.0, 48.0);
        let CadItem::Polyline { points, closed } = &cx.floor().cad[0].item else {
            panic!("a polyline")
        };
        assert!(*closed);
        assert_eq!(points.len(), 4);
        assert_eq!(points[2], Point::new(96.0, 48.0));
    }

    #[test]
    fn circle_takes_center_and_radius() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::Circle);
        click(&mut t, &mut cx, 50.0, 50.0);
        click(&mut t, &mut cx, 80.0, 50.0);
        assert_eq!(
            cad_items(&cx),
            vec![CadItem::Circle {
                center: Point::new(50.0, 50.0),
                radius: 30.0
            }]
        );
        // Typed radius with Enter.
        let mut t = tool(CadMode::CircleAboutCenter);
        click(&mut t, &mut cx, 200.0, 0.0);
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        t.key(&mut cx, KeyEvent::text("2'"));
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(
            cx.floor().cad[1].item,
            CadItem::Circle {
                center: Point::new(200.0, 0.0),
                radius: 24.0
            }
        );
    }

    #[test]
    fn three_point_arc_passes_through_the_third_click() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::Arc);
        click(&mut t, &mut cx, 120.0, 0.0);
        click(&mut t, &mut cx, -120.0, 0.0);
        click(&mut t, &mut cx, 0.0, 120.0);
        let CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } = cx.floor().cad[0].item
        else {
            panic!("an arc")
        };
        assert!(center.dist(Point::ZERO) < 1e-6 && (radius - 120.0).abs() < 1e-6);
        assert!(start_angle.abs() < 1e-6 && (end_angle - PI).abs() < 1e-6);
    }

    #[test]
    fn center_start_end_and_tangent_arcs() {
        let a = arc_center_start_end(Point::ZERO, Point::new(10.0, 0.0), Point::new(0.0, 10.0))
            .unwrap();
        let CadItem::Arc {
            radius,
            start_angle,
            end_angle,
            ..
        } = a
        else {
            panic!("arc")
        };
        assert!((radius - 10.0).abs() < 1e-9);
        assert!(start_angle.abs() < 1e-9 && (end_angle - PI / 2.0).abs() < 1e-9);
        let t = arc_start_end_tangent(Point::ZERO, Point::new(10.0, 10.0), Point::new(5.0, 0.0))
            .unwrap();
        let CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } = t
        else {
            panic!("arc")
        };
        assert!(center.dist(Point::new(0.0, 10.0)) < 1e-9 && (radius - 10.0).abs() < 1e-9);
        assert!((start_angle + PI / 2.0).abs() < 1e-9 && end_angle.abs() < 1e-9);
    }

    #[test]
    fn spline_is_sampled_at_eight_segments_per_span() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(50.0, 40.0),
            Point::new(100.0, 0.0),
            Point::new(150.0, 40.0),
        ];
        assert_eq!(catmull_rom(&pts, false, 8).len(), 3 * 8 + 1);
        assert_eq!(catmull_rom(&pts, true, 8).len(), 4 * 8);
        // The curve passes through the fit points.
        let s = catmull_rom(&pts, false, 8);
        assert!(s[8].dist(pts[1]) < 1e-9 && s[16].dist(pts[2]) < 1e-9);

        let mut cx = new_cx();
        let mut t = tool(CadMode::Spline);
        for p in pts {
            click(&mut t, &mut cx, p.x, p.y);
        }
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        let CadItem::Polyline { points, closed } = &cx.floor().cad[0].item else {
            panic!("a polyline")
        };
        assert_eq!(points.len(), 25);
        assert!(!*closed);
    }

    #[test]
    fn regular_polygon_uses_the_side_count() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::Polygon);
        t.set_sides(5);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 60.0, 0.0);
        let CadItem::Polyline { points, closed } = &cx.floor().cad[0].item else {
            panic!("a polyline")
        };
        assert!(*closed);
        assert_eq!(points.len(), 5);
        for p in points {
            assert!((p.length() - 60.0).abs() < 1e-6);
        }
    }

    #[test]
    fn ellipse_and_oval_are_closed_polylines() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::Oval);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 80.0, 40.0);
        let CadItem::Polyline { points, closed } = &cx.floor().cad[0].item else {
            panic!("a polyline")
        };
        assert!(*closed);
        assert_eq!(points.len(), ELLIPSE_SEGMENTS);
        let (lo, hi) = cx.floor().cad[0].bounds();
        assert!((hi.x - lo.x - 80.0).abs() < 1.0 && (hi.y - lo.y - 40.0).abs() < 1.0);
        let mut t = tool(CadMode::Ellipse);
        click(&mut t, &mut cx, 0.0, 200.0);
        click(&mut t, &mut cx, 60.0, 200.0);
        click(&mut t, &mut cx, 0.0, 220.0);
        assert_eq!(cx.floor().cad.len(), 2);
    }

    #[test]
    fn revision_cloud_is_a_scalloped_closed_outline() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(48.0, 0.0),
            Point::new(48.0, 48.0),
            Point::new(0.0, 48.0),
        ];
        let cloud = revision_cloud(&square, 12.0);
        // 4 edges * 4 scallops * 6 samples.
        assert_eq!(cloud.len(), 96);
        let (lo, hi) = plan_core::cad::CadItem::Polyline {
            points: cloud,
            closed: true,
        }
        .bounds();
        assert!(
            lo.x < 0.0 && lo.y < 0.0 && hi.x > 48.0 && hi.y > 48.0,
            "bulges outward"
        );
    }

    #[test]
    fn points_markers_and_input_points() {
        let mut cx = new_cx();
        let mut t = tool(CadMode::PointMarker);
        click(&mut t, &mut cx, 10.0, 10.0);
        // Cross lines and a circle, grouped as one object.
        assert_eq!(cx.floor().cad.len(), 3);
        assert_eq!(cx.floor().groups.len(), 1);
        let mut t = tool(CadMode::InputPoint);
        t.activate(&mut cx);
        t.key(&mut cx, KeyEvent::text("5'"));
        t.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        t.key(&mut cx, KeyEvent::text("2'"));
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(cx.floor().cad.len(), 5);
        let CadItem::Line { a, .. } = cx.floor().cad[3].item else {
            panic!("line")
        };
        assert!((a.x - (60.0 - POINT_SIZE)).abs() < 1e-9 && (a.y - 24.0).abs() < 1e-9);
    }

    #[test]
    fn make_and_explode_cad_block() {
        let mut cx = new_cx();
        let a = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(10.0, 0.0),
            },
        );
        let b = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Circle {
                center: Point::ZERO,
                radius: 4.0,
            },
        );
        cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(b)];
        let mut t = tool(CadMode::MakeBlock);
        assert!(t.make_block(&mut cx).commit.is_some());
        assert_eq!(cx.floor().groups.len(), 1);
        assert!(t.explode_block(&mut cx).commit.is_some());
        assert!(cx.floor().groups.is_empty());
        assert_eq!(cx.selection.len(), 2);
    }

    #[test]
    fn selected_line_ends_can_be_dragged() {
        let mut cx = new_cx();
        let id = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(100.0, 0.0),
            },
        );
        cx.selection.set(ObjectRef::Cad(id));
        let mut t = tool(CadMode::Line);
        let down = PointerEvent::at(&cx, Point::new(100.0, 1.0));
        t.pointer_down(&mut cx, down.with_down(true));
        let to = PointerEvent::at(&cx, Point::new(100.0, 50.0));
        t.pointer_move(&mut cx, to.with_down(true));
        t.pointer_up(&mut cx, to);
        let CadItem::Line { b, .. } = cad_by_id(cx.floor(), id).unwrap().item else {
            panic!("line")
        };
        assert_eq!(b, Point::new(100.0, 50.0));
        assert!(
            t.pending().is_empty(),
            "the press grabbed a handle, it did not start a line"
        );
        assert_eq!(cx.undo().as_deref(), Some("Edit CAD Object"));
    }

    #[test]
    fn clicking_a_selected_lines_end_still_starts_the_next_line() {
        let mut cx = new_cx();
        let id = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(100.0, 0.0),
            },
        );
        cx.selection.set(ObjectRef::Cad(id));
        let mut t = tool(CadMode::Line);
        click(&mut t, &mut cx, 100.0, 0.0);
        assert_eq!(t.pending(), &[Point::new(100.0, 0.0)]);
        click(&mut t, &mut cx, 100.0, 60.0);
        assert_eq!(cx.floor().cad.len(), 2);
        assert_eq!(
            cad_by_id(cx.floor(), id).unwrap().item,
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(100.0, 0.0)
            },
            "the first line is untouched"
        );
    }

    #[test]
    fn polyline_midpoint_handle_adds_a_vertex() {
        let mut cx = new_cx();
        let id = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Polyline {
                points: vec![Point::ZERO, Point::new(100.0, 0.0)],
                closed: false,
            },
        );
        cx.selection.set(ObjectRef::Cad(id));
        let mut t = tool(CadMode::Polyline);
        let down = PointerEvent::at(&cx, Point::new(50.0, 0.0));
        t.pointer_down(&mut cx, down.with_down(true));
        let to = PointerEvent::at(&cx, Point::new(50.0, 40.0));
        t.pointer_move(&mut cx, to.with_down(true));
        t.pointer_up(&mut cx, to);
        let CadItem::Polyline { points, .. } = &cad_by_id(cx.floor(), id).unwrap().item else {
            panic!("polyline")
        };
        assert_eq!(points.len(), 3);
        assert_eq!(points[1], Point::new(50.0, 40.0));
    }

    #[test]
    fn locked_layer_refuses_new_objects() {
        let mut cx = new_cx();
        cx.project.layers.set_locked(CAD_LAYER, true);
        let mut t = tool(CadMode::Line);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 60.0, 0.0);
        assert!(cx.floor().cad.is_empty());
        assert!(cx.status.contains("locked"));
    }

    #[test]
    fn variant_requests_pick_the_mode() {
        let mut t = CadTool::default();
        t.set_variant(ToolId::CadVariant(CadMode::CircleAboutCenter));
        assert_eq!(t.mode(), CadMode::CircleAboutCenter);
        t.set_variant(ToolId::Cad);
        assert_eq!(
            t.mode(),
            CadMode::CircleAboutCenter,
            "a plain re-pick keeps the mode"
        );
    }

    #[test]
    fn mode_names_round_trip() {
        for m in CadMode::ALL {
            assert_eq!(CadMode::from_name(m.name()), Some(m));
        }
        assert_eq!(CadMode::from_name("nope"), None);
    }
}
