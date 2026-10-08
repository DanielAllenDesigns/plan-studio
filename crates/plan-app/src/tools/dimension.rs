//! Dimension tools (DIM-1..DIM-38 in `docs/parity/dimensions-text-cad.md`).
//!
//! One tool object with a mode per Chief dimension tool. `ToolId` has a
//! single `Dimension` value, so the mode is chosen with
//! [`DimensionTool::set_mode`], a `ToolId::DimensionVariant(mode)` payload
//! (handled by `Tool::set_variant`) or the option strip
//! drawn at the top of the canvas.
//!
//! * Manual / Centerline / Point to Point (DIM-11, DIM-15, DIM-19): click the
//!   first and second point (or press-drag-release), then move to set the
//!   dimension line and click. Points locate to wall surfaces (the main
//!   layer faces, DIM-4), opening centers or the snap point; Alt suspends the
//!   locating (DIM-21). The measuring direction follows the placement click
//!   (horizontal, vertical or aligned).
//! * End to End (DIM-13), Interior (DIM-14), Running (DIM-16), Baseline
//!   (DIM-17), Angular (DIM-18), Tape Measure (DIM-20).
//! * Auto Exterior (DIM-24, DIM-25) and Auto Interior (DIM-27) run on one
//!   click. Auto Elevation and Auto Story Pole dimensions run on one click
//!   too: the click's x is the line of a vertical string of level heights
//!   (floor platforms, ceiling heights, heights above the first floor) with
//!   each level named beside it.
//! * A dimension is selected by clicking it (DIM-30). Its handles move the
//!   dimension line and relocate either measured point; clicking its text
//!   opens an inline value edit that moves the located object at the end
//!   nearer the cursor (DIM-32): walls move perpendicular (or lengthen when
//!   the dimension runs along them), openings slide. Locked layers refuse
//!   (DIM-34). Double-click asks the shell for the Dimension Specification
//!   (DIM-31).
//!
//! Dimensions are stored in `floor.dimensions` (DIM-23, one undo step each).
//! The model keeps no object associations (DIM-3, DIM-29, DIM-35), so the
//! dimension line stays where it is when objects move.

use super::cad::{add_cad_items, set_typing, OptionStrip, StripButton};
use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::selection::hit_opening;
use crate::editor::{ops, render, Camera, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use plan_core::cad::CadItem;
use plan_core::geometry::{
    dist_to_segment, point_in_polygon, project_on_segment, segment_intersection, Point,
};
use plan_core::units::parse_ft_in;
use plan_core::{
    auto_exterior_dimensions, wall_layer_bands, Dimension, DimensionKind, Id, Wall, WallEnd,
};
use std::f64::consts::{PI, TAU};

pub const MANUAL_LAYER: &str = "Dimensions, Manual";
pub const AUTO_LAYER: &str = "Dimensions, Automatic";
/// Dimensions shorter than this are not created.
const MIN_LENGTH: f64 = 0.5;
const DRAG_PX: f32 = 4.0;
/// How close a measured point must be to a wall face to count as located on
/// it when a value edit looks for the object to move.
const LOCATE_TOL: f64 = 0.75;
/// Rooms smaller than this get no automatic interior dimensions (sq ft).
const MIN_ROOM_SQ_FT: f64 = 10.0;

// ----- modes -----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DimMode {
    Manual,
    EndToEnd,
    Interior,
    PointToPoint,
    Running,
    Baseline,
    Centerline,
    Angular,
    TapeMeasure,
    AutoExterior,
    AutoInterior,
    AutoElevation,
    AutoStoryPole,
}

impl DimMode {
    pub const ALL: [DimMode; 13] = [
        DimMode::Manual,
        DimMode::EndToEnd,
        DimMode::Interior,
        DimMode::PointToPoint,
        DimMode::Running,
        DimMode::Baseline,
        DimMode::Centerline,
        DimMode::Angular,
        DimMode::TapeMeasure,
        DimMode::AutoExterior,
        DimMode::AutoInterior,
        DimMode::AutoElevation,
        DimMode::AutoStoryPole,
    ];

    /// Chief's name from the toolbar flyouts.
    pub fn name(self) -> &'static str {
        match self {
            DimMode::Manual => "Manual Dimension",
            DimMode::EndToEnd => "End to End Dimension",
            DimMode::Interior => "Interior Dimension",
            DimMode::PointToPoint => "Point to Point Dimension",
            DimMode::Running => "Running Dimension",
            DimMode::Baseline => "Baseline Dimension",
            DimMode::Centerline => "Centerline Dimension",
            DimMode::Angular => "Angular Dimension",
            DimMode::TapeMeasure => "Tape Measure",
            DimMode::AutoExterior => "Auto Exterior Dimensions",
            DimMode::AutoInterior => "Auto Interior Dimensions",
            DimMode::AutoElevation => "Auto Elevation Dimensions",
            DimMode::AutoStoryPole => "Auto Story Pole Dimensions",
        }
    }

    pub fn from_name(name: &str) -> Option<DimMode> {
        DimMode::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }

    fn short(self) -> &'static str {
        match self {
            DimMode::Manual => "Manual",
            DimMode::EndToEnd => "End to End",
            DimMode::Interior => "Interior",
            DimMode::PointToPoint => "Point to Point",
            DimMode::Running => "Running",
            DimMode::Baseline => "Baseline",
            DimMode::Centerline => "Centerline",
            DimMode::Angular => "Angular",
            DimMode::TapeMeasure => "Tape",
            DimMode::AutoExterior => "Auto Exterior",
            DimMode::AutoInterior => "Auto Interior",
            DimMode::AutoElevation => "Auto Elevation",
            DimMode::AutoStoryPole => "Auto Story Pole",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            DimMode::Manual => {
                "Manual Dimension: click two points, then click to place the dimension line"
            }
            DimMode::EndToEnd => {
                "End to End Dimension: click a wall or line, then click to place the dimension line"
            }
            DimMode::Interior => {
                "Interior Dimension: click inside a room, then click to place the dimension line"
            }
            DimMode::PointToPoint => {
                "Point to Point Dimension: click two points, then click to place the dimension line"
            }
            DimMode::Running => {
                "Running Dimension: click the points; Enter or double-click places the string"
            }
            DimMode::Baseline => {
                "Baseline Dimension: click the origin, then each point to stack a dimension"
            }
            DimMode::Centerline => {
                "Centerline Dimension: click two centers, then click to place the dimension line"
            }
            DimMode::Angular => {
                "Angular Dimension: click the vertex and two arm points, then the arc radius"
            }
            DimMode::TapeMeasure => {
                "Tape Measure: click two points to read the distance; Esc clears"
            }
            DimMode::AutoExterior => {
                "Auto Exterior Dimensions: click to dimension the exterior walls"
            }
            DimMode::AutoInterior => "Auto Interior Dimensions: click to dimension every room",
            DimMode::AutoElevation => {
                "Auto Elevation Dimensions: click where the level dimensions go (heights above the first floor)"
            }
            DimMode::AutoStoryPole => {
                "Auto Story Pole Dimensions: click where the story pole goes (ceiling heights and floor platforms)"
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            DimMode::Manual | DimMode::PointToPoint | DimMode::Centerline => "Manual Dimension",
            DimMode::EndToEnd => "End to End Dimension",
            DimMode::Interior => "Interior Dimension",
            DimMode::Running => "Running Dimension",
            DimMode::Baseline => "Baseline Dimension",
            DimMode::Angular => "Angular Dimension",
            DimMode::TapeMeasure => "Tape Measure",
            DimMode::AutoExterior => "Auto Exterior Dimensions",
            DimMode::AutoInterior => "Auto Interior Dimensions",
            DimMode::AutoElevation => "Auto Elevation Dimensions",
            DimMode::AutoStoryPole => "Auto Story Pole Dimensions",
        }
    }

    fn is_auto(self) -> bool {
        matches!(
            self,
            DimMode::AutoExterior
                | DimMode::AutoInterior
                | DimMode::AutoElevation
                | DimMode::AutoStoryPole
        )
    }
}

// ----- pure geometry -----

/// How a two-point dimension measures (DIM-11, DIM-15).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    /// Horizontal or vertical by where the placement point lies.
    Auto,
    /// Along the line joining the points.
    Aligned,
    Horizontal,
    Vertical,
}

/// A dimension between `a` and `b` with its line through `place`.
pub fn linear_dimension(a: Point, b: Point, place: Point, axis: Axis) -> Dimension {
    let axis = match axis {
        Axis::Auto => {
            if (a.y - b.y).abs() < 1e-6 || (a.x - b.x).abs() < 1e-6 {
                Axis::Aligned
            } else {
                let d = place.sub(Point::lerp(a, b, 0.5));
                if d.y.abs() >= d.x.abs() {
                    Axis::Horizontal
                } else {
                    Axis::Vertical
                }
            }
        }
        other => other,
    };
    let (start, end) = match axis {
        Axis::Horizontal => {
            let yr = if (a.y - place.y).abs() <= (b.y - place.y).abs() {
                a.y
            } else {
                b.y
            };
            (Point::new(a.x, yr), Point::new(b.x, yr))
        }
        Axis::Vertical => {
            let xr = if (a.x - place.x).abs() <= (b.x - place.x).abs() {
                a.x
            } else {
                b.x
            };
            (Point::new(xr, a.y), Point::new(xr, b.y))
        }
        _ => (a, b),
    };
    let dir = end.sub(start).normalized();
    let offset = place.sub(start).dot(dir.perp());
    Dimension::new(0, DimensionKind::Manual, start, end, offset)
}

/// Horizontal or vertical, whichever the segment `a`..`b` is nearer to.
fn dominant_axis(a: Point, b: Point) -> Axis {
    if (b.x - a.x).abs() >= (b.y - a.y).abs() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    }
}

fn project_on_axis(origin: Point, p: Point, axis: Axis) -> Point {
    match axis {
        Axis::Vertical => Point::new(origin.x, p.y),
        _ => Point::new(p.x, origin.y),
    }
}

/// A running dimension (DIM-16): consecutive segments on one line, each
/// labelled with the distance from the first point.
pub fn running_dimensions(
    points: &[Point],
    place: Point,
    fmt: &plan_core::DimFormat,
) -> Vec<Dimension> {
    if points.len() < 2 {
        return Vec::new();
    }
    let axis = dominant_axis(points[0], points[1]);
    let projected: Vec<Point> = points
        .iter()
        .map(|p| project_on_axis(points[0], *p, axis))
        .collect();
    let base = projected[1].sub(projected[0]).normalized();
    let off_vec = base.perp().scale(place.sub(projected[0]).dot(base.perp()));
    let mut out = Vec::new();
    for i in 1..projected.len() {
        let (s, e) = (projected[i - 1], projected[i]);
        if s.dist(e) < MIN_LENGTH {
            continue;
        }
        let dir = e.sub(s).normalized();
        let mut d = Dimension::new(0, DimensionKind::Manual, s, e, off_vec.dot(dir.perp()));
        if i >= 2 {
            d.text_override = Some(fmt.fmt_len(projected[0].dist(e)));
        }
        out.push(d);
    }
    out
}

/// Row `index` (from 1) of a baseline dimension string from `origin` to `p`
/// (DIM-17): rows stack `separation` apart, below horizontal strings and to
/// the right of vertical ones.
pub fn baseline_dimension(
    origin: Point,
    p: Point,
    index: usize,
    separation: f64,
    axis: Axis,
) -> Dimension {
    let end = project_on_axis(origin, p, axis);
    let dir = end.sub(origin).normalized();
    let side = match axis {
        Axis::Vertical => Point::new(1.0, 0.0),
        _ => Point::new(0.0, -1.0),
    };
    let offset = side.dot(dir.perp()) * separation * index as f64;
    Dimension::new(0, DimensionKind::Manual, origin, end, offset)
}

/// The arc, its extension lines and the value text of an angular dimension
/// (DIM-18): the arc runs the short way from the first arm to the second.
/// Returns the items and the angle in degrees.
pub fn angular_items(
    vertex: Point,
    p1: Point,
    p2: Point,
    radius: f64,
    text_height: f64,
) -> Option<(Vec<CadItem>, f64)> {
    if vertex.dist(p1) < MIN_LENGTH || vertex.dist(p2) < MIN_LENGTH || radius < 1.0 {
        return None;
    }
    let (a1, a2) = (p1.sub(vertex).angle(), p2.sub(vertex).angle());
    let sweep = (a2 - a1).rem_euclid(TAU);
    let (start, end, deg) = if sweep <= PI {
        (a1, a2, sweep.to_degrees())
    } else {
        (a2, a1, (TAU - sweep).to_degrees())
    };
    let at = |ang: f64, r: f64| Point::new(vertex.x + r * ang.cos(), vertex.y + r * ang.sin());
    let mid = start + (end - start).rem_euclid(TAU) * 0.5;
    let label_at = at(mid, radius + text_height * 0.5);
    let text = format!("{deg:.1}\u{b0}");
    let w = text.chars().count() as f64 * text_height * plan_core::cad::TEXT_WIDTH_FACTOR;
    Some((
        vec![
            CadItem::Arc {
                center: vertex,
                radius,
                start_angle: start,
                end_angle: end,
            },
            CadItem::Line {
                a: vertex,
                b: at(start, radius + 6.0),
            },
            CadItem::Line {
                a: vertex,
                b: at(end, radius + 6.0),
            },
            CadItem::Text {
                pos: Point::new(label_at.x - w * 0.5, label_at.y),
                text,
                height: text_height,
                angle: 0.0,
            },
        ],
        deg,
    ))
}

/// Distance along the ray `origin + dir * t` to the first segment it crosses.
fn ray_hit(segments: &[(Point, Point)], origin: Point, dir: Point) -> Option<Point> {
    const FAR: f64 = 1.0e6;
    let far = origin.add(dir.scale(FAR));
    segments
        .iter()
        .filter_map(|(a, b)| segment_intersection(origin, far, *a, *b).map(|(t, _)| t * FAR))
        .filter(|t| *t > 1e-6)
        .min_by(|x, y| x.total_cmp(y))
        .map(|t| origin.add(dir.scale(t)))
}

/// Where rays left, right, down and up from `origin` first meet `segments`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rays {
    pub left: Option<Point>,
    pub right: Option<Point>,
    pub down: Option<Point>,
    pub up: Option<Point>,
}

fn rays_from(segments: &[(Point, Point)], origin: Point) -> Rays {
    Rays {
        left: ray_hit(segments, origin, Point::new(-1.0, 0.0)),
        right: ray_hit(segments, origin, Point::new(1.0, 0.0)),
        down: ray_hit(segments, origin, Point::new(0.0, -1.0)),
        up: ray_hit(segments, origin, Point::new(0.0, 1.0)),
    }
}

fn polygon_edges(poly: &[Point]) -> Vec<(Point, Point)> {
    (0..poly.len())
        .map(|i| (poly[i], poly[(i + 1) % poly.len()]))
        .collect()
}

/// The clear span between opposing interior surfaces through `origin`
/// (DIM-14), placed by the pointer: a horizontal span when `place` is above or
/// below `origin`, otherwise a vertical one.
fn interior_dimension(r: &Rays, origin: Point, place: Point) -> Option<Dimension> {
    let d = place.sub(origin);
    let horizontal = r.left.zip(r.right);
    let vertical = r.down.zip(r.up);
    let (s, e) = if d.y.abs() >= d.x.abs() {
        horizontal.or(vertical)?
    } else {
        vertical.or(horizontal)?
    };
    let dir = e.sub(s).normalized();
    let offset = place.sub(s).dot(dir.perp());
    Some(Dimension::new(0, DimensionKind::Manual, s, e, offset))
}

// ----- locating -----

/// A measured point and what it was located on (DIM-12).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Located {
    pub point: Point,
    pub what: &'static str,
    pub obj: Option<ObjectRef>,
}

fn no_locate(_cx: &EditorContext, w: &Wall) -> bool {
    w.flags.no_locate
}

/// The main layer's lateral span `(lo, hi)` across the wall's thickness.
fn main_span(cx: &EditorContext, w: &Wall) -> (f64, f64) {
    let ty = w.wall_type.as_deref().and_then(|n| {
        cx.project
            .wall_type_def(n)
            .or_else(|| cx.defaults.wall_type(n))
    });
    let bands = wall_layer_bands(w, ty);
    match bands.iter().find(|b| b.is_main) {
        Some(b) => (b.outer.min(b.inner), b.outer.max(b.inner)),
        None => (-w.thickness * 0.5, w.thickness * 0.5),
    }
}

/// The wall under `p` (nearest centerline within the body plus the pick
/// distance), skipping hidden and No Locate walls (DIM-5).
fn wall_near(cx: &EditorContext, p: Point) -> Option<&Wall> {
    let tol = cx.pick_tol();
    cx.floor()
        .walls
        .iter()
        .filter(|w| cx.layers().is_visible(&w.layer) && !no_locate(cx, w))
        .map(|w| (w, dist_to_segment(p, w.start, w.end)))
        .filter(|(w, d)| *d <= w.thickness * 0.5 + tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w)
}

/// Locates the point under the pointer (DIM-4, DIM-12, DIM-19, DIM-21).
/// `centers` forces centers (Centerline Dimension).
fn locate(cx: &EditorContext, p: &PointerEvent, centers: bool, origin: Option<Point>) -> Located {
    if p.modifiers.alt {
        return Located {
            point: p.world,
            what: "Free point",
            obj: None,
        };
    }
    let tol = cx.pick_tol();
    let floor = cx.floor();
    if let Some(oid) = hit_opening(floor, p.world, tol * 0.5) {
        if let Some(o) = floor.openings.iter().find(|o| o.id == oid) {
            if let Some(w) = floor.wall(o.wall_id).filter(|w| !no_locate(cx, w)) {
                if centers || cx.defaults.dimensions.locate_openings_centers {
                    return Located {
                        point: w.point_at(o.center_offset),
                        what: "Opening center",
                        obj: Some(ObjectRef::Opening(oid)),
                    };
                }
                let (t, _) = project_on_segment(p.world, w.start, w.end);
                let along = t * w.length();
                let edge = if (along - o.start_offset()).abs() <= (along - o.end_offset()).abs() {
                    o.start_offset()
                } else {
                    o.end_offset()
                };
                let (lo, hi) = main_span(cx, w);
                let perp = p.world.sub(w.start).dot(w.normal());
                let off = if (perp - lo).abs() <= (perp - hi).abs() {
                    lo
                } else {
                    hi
                };
                return Located {
                    point: w.point_at(edge).add(w.normal().scale(off)),
                    what: "Opening edge",
                    obj: Some(ObjectRef::Opening(oid)),
                };
            }
        }
    }
    if let Some(w) = wall_near(cx, p.world) {
        let n = w.normal();
        let perp = p.world.sub(w.start).dot(n);
        let (lo, hi) = main_span(cx, w);
        let (off, what) = if centers {
            (0.0, "Wall center")
        } else if (perp - lo).abs() <= (perp - hi).abs() {
            (lo, "Wall surface")
        } else {
            (hi, "Wall surface")
        };
        let (t, _) = project_on_segment(p.world, w.start, w.end);
        let len = w.length();
        let mut along = t * len;
        if along < tol {
            along = 0.0;
        } else if along > len - tol {
            along = len;
        }
        return Located {
            point: w.point_at(along).add(n.scale(off)),
            what,
            obj: Some(ObjectRef::Wall(w.id)),
        };
    }
    let snap = cx.snap_at(p.world, origin, false, &[]);
    Located {
        point: snap.point,
        what: snap.kind.label(),
        obj: None,
    }
}

/// The segment a click on a wall or CAD line measures end to end (DIM-13):
/// the wall's face line on the clicked side, or the line itself.
fn end_to_end_target(cx: &EditorContext, p: Point) -> Option<(Point, Point)> {
    if let Some(w) = wall_near(cx, p) {
        let n = w.normal();
        let perp = p.sub(w.start).dot(n);
        let (lo, hi) = main_span(cx, w);
        let off = if (perp - lo).abs() <= (perp - hi).abs() {
            lo
        } else {
            hi
        };
        return Some((w.start.add(n.scale(off)), w.end.add(n.scale(off))));
    }
    let tol = cx.pick_tol();
    cx.floor().cad.iter().rev().find_map(|c| match c.item {
        CadItem::Line { a, b } if dist_to_segment(p, a, b) <= tol => Some((a, b)),
        _ => None,
    })
}

/// Do `a` and `b` lie on one line?
fn collinear(span: (Point, Point), t: (Point, Point)) -> bool {
    let u = span.1.sub(span.0).normalized();
    let n = u.perp();
    u.cross(t.1.sub(t.0).normalized()).abs() < 1e-3
        && t.0.sub(span.0).dot(n).abs() < 0.6
        && t.1.sub(span.0).dot(n).abs() < 0.6
}

/// Wall faces / CAD lines near `p` that value edits move.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Moveable {
    /// Move the wall perpendicular to its length.
    WallPerp(Id),
    /// Lengthen or shorten the wall at one end.
    WallEnd(Id, WallEnd),
    /// Slide the opening along its wall.
    Opening(Id, Id),
}

/// The object a measured point at `pt` is located on, for a move along
/// `axis` (DIM-32).
fn find_moveable(cx: &EditorContext, pt: Point, axis: Point) -> Option<Moveable> {
    let floor = cx.floor();
    for o in &floor.openings {
        let Some(w) = floor.wall(o.wall_id) else {
            continue;
        };
        let (t, _) = project_on_segment(pt, w.start, w.end);
        let along = t * w.length();
        let perp = pt.sub(w.start).dot(w.normal()).abs();
        let near_edge = (along - o.start_offset()).abs() <= LOCATE_TOL
            || (along - o.end_offset()).abs() <= LOCATE_TOL;
        if perp <= w.thickness * 0.5 + LOCATE_TOL
            && ((along - o.center_offset).abs() <= LOCATE_TOL || near_edge)
        {
            return Some(Moveable::Opening(o.id, w.id));
        }
    }
    let mut best: Option<(f64, Moveable)> = None;
    for w in floor.walls.iter().filter(|w| !no_locate(cx, w)) {
        let n = w.normal();
        let (lo, hi) = main_span(cx, w);
        let perp = pt.sub(w.start).dot(n);
        let on_face = [lo, 0.0, hi, -w.thickness * 0.5, w.thickness * 0.5]
            .iter()
            .any(|off| (perp - off).abs() <= LOCATE_TOL);
        let (t, _) = project_on_segment(pt, w.start, w.end);
        let along = t * w.length();
        let within = pt.sub(w.point_at(along)).dot(w.direction()).abs() <= LOCATE_TOL;
        if !on_face || !within {
            continue;
        }
        let across = axis.dot(n).abs();
        let cand = if across >= 0.5 {
            Moveable::WallPerp(w.id)
        } else if along < w.length() * 0.5 {
            Moveable::WallEnd(w.id, WallEnd::Start)
        } else {
            Moveable::WallEnd(w.id, WallEnd::End)
        };
        if best.is_none_or(|(s, _)| across > s) {
            best = Some((across, cand));
        }
    }
    best.map(|(_, m)| m)
}

// ----- the tool -----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DimHandle {
    Offset,
    Start,
    End,
}

/// A press on a handle that has not moved yet: dragging moves the handle, a
/// plain click is an ordinary click (so the next dimension can start where the
/// last one ended).
struct DimGrab {
    id: Id,
    handle: DimHandle,
    event: PointerEvent,
}

struct DimDrag {
    id: Id,
    handle: DimHandle,
    changed: bool,
}

struct ValueEdit {
    id: Id,
    buf: String,
}

pub struct DimensionTool {
    mode: DimMode,
    pts: Vec<Located>,
    span: Option<(Point, Point)>,
    interior: Option<(Point, Rays)>,
    /// Baseline: the axis chosen by the first row and the rows made so far.
    baseline: Option<(Axis, usize)>,
    hover: Option<Located>,
    cursor: Point,
    press: Option<Pos2>,
    drag: Option<DimDrag>,
    grab: Option<DimGrab>,
    edit: Option<ValueEdit>,
    strip: OptionStrip,
}

impl Default for DimensionTool {
    fn default() -> Self {
        Self {
            mode: DimMode::Manual,
            pts: Vec::new(),
            span: None,
            interior: None,
            baseline: None,
            hover: None,
            cursor: Point::ZERO,
            press: None,
            drag: None,
            grab: None,
            edit: None,
            strip: OptionStrip::default(),
        }
    }
}

impl DimensionTool {
    pub fn mode(&self) -> DimMode {
        self.mode
    }

    /// Switches the variant (a flyout entry), dropping work in progress.
    pub fn set_mode(&mut self, mode: DimMode) {
        self.mode = mode;
        self.reset();
    }

    pub fn set_mode_by_name(&mut self, name: &str) -> bool {
        match DimMode::from_name(name) {
            Some(m) => {
                self.set_mode(m);
                true
            }
            None => false,
        }
    }

    /// Points clicked so far in the dimension in progress.
    pub fn clicked(&self) -> Vec<Point> {
        self.pts.iter().map(|l| l.point).collect()
    }

    fn reset(&mut self) {
        self.pts.clear();
        self.span = None;
        self.interior = None;
        self.baseline = None;
        self.press = None;
        self.drag = None;
        self.grab = None;
        self.edit = None;
    }

    fn in_progress(&self) -> bool {
        !self.pts.is_empty()
            || self.span.is_some()
            || self.interior.is_some()
            || self.baseline.is_some()
    }

    fn separation(cx: &EditorContext) -> f64 {
        let s = cx.defaults.dimensions.auto_line_separation;
        if s > 0.0 {
            s
        } else {
            18.0
        }
    }

    fn axis(&self) -> Axis {
        if self.mode == DimMode::PointToPoint {
            Axis::Aligned
        } else {
            Axis::Auto
        }
    }

    /// The dimensions the work so far makes with the placement at `place`.
    fn build(&self, cx: &EditorContext, place: Point) -> Vec<Dimension> {
        match self.mode {
            DimMode::Manual | DimMode::Centerline | DimMode::PointToPoint
                if self.pts.len() >= 2 =>
            {
                vec![linear_dimension(
                    self.pts[0].point,
                    self.pts[1].point,
                    place,
                    self.axis(),
                )]
            }
            DimMode::EndToEnd => self
                .span
                .map(|(a, b)| {
                    let u = b.sub(a).normalized();
                    let off = place.sub(a).dot(u.perp());
                    vec![Dimension::new(0, DimensionKind::Manual, a, b, off)]
                })
                .unwrap_or_default(),
            DimMode::Interior => self
                .interior
                .and_then(|(o, r)| interior_dimension(&r, o, place))
                .into_iter()
                .collect(),
            DimMode::Running => {
                let mut pts = self.clicked();
                if let Some(h) = self.hover.filter(|_| self.edit.is_none()) {
                    if pts.last().is_some_and(|l| l.dist(h.point) >= MIN_LENGTH) {
                        pts.push(h.point);
                    }
                }
                running_dimensions(&pts, place, &cx.dim_format())
            }
            DimMode::Baseline if !self.pts.is_empty() => {
                let origin = self.pts[0].point;
                let sep = Self::separation(cx);
                let index = self.baseline.map_or(1, |(_, n)| n + 1);
                let axis = self
                    .baseline
                    .map_or_else(|| dominant_axis(origin, place), |(a, _)| a);
                vec![baseline_dimension(origin, place, index, sep, axis)]
            }
            _ => Vec::new(),
        }
    }

    fn lock_check(&self, cx: &mut EditorContext, layer: &str) -> bool {
        if cx.layers().is_locked(layer) {
            cx.status = format!("The layer \"{layer}\" is locked");
            false
        } else {
            true
        }
    }

    /// Adds the dimensions as one undo step and selects the last.
    fn commit(&mut self, cx: &mut EditorContext, dims: Vec<Dimension>) -> ToolResult {
        let dims: Vec<Dimension> = dims
            .into_iter()
            .filter(|d| d.length() >= MIN_LENGTH)
            .collect();
        if dims.is_empty() || !self.lock_check(cx, MANUAL_LAYER) {
            self.reset();
            return ToolResult::consumed();
        }
        let label = self.mode.label();
        cx.begin_change(label);
        let fl = cx.floor;
        let mut last = 0;
        for d in dims {
            last = cx.project.add_dimension(fl, d);
            // An end on a wall stays tied to it (it follows the wall).
            cx.project.floors[fl].attach_dimension(last);
        }
        cx.selection.set(ObjectRef::Dimension(last));
        cx.mark_dirty();
        cx.readout = None;
        cx.status.clear();
        ToolResult::committed(label)
    }

    // ----- automatic dimensions -----

    fn auto_exterior(&mut self, cx: &mut EditorContext) -> ToolResult {
        cx.refresh();
        let walls: Vec<Wall> = cx
            .floor()
            .walls
            .iter()
            .filter(|w| !no_locate(cx, w) && !w.flags.room_divider)
            .cloned()
            .collect();
        let offset = cx.defaults.dimensions.auto_exterior_offset;
        let dims = auto_exterior_dimensions(&walls, offset);
        if dims.is_empty() {
            cx.status = "No exterior walls to dimension".into();
            return ToolResult::consumed();
        }
        if !self.lock_check(cx, AUTO_LAYER) {
            return ToolResult::consumed();
        }
        let inner: Vec<Vec<Point>> = cx.rooms.iter().map(|r| r.inner_polygon.clone()).collect();
        cx.begin_change("Auto Exterior Dimensions");
        let fl = cx.floor;
        // DIM-25: a new run replaces the previous exterior dimensions.
        cx.project.floors[fl].dimensions.retain(|d| {
            let (a, b) = d.line_points();
            let mid = Point::lerp(a, b, 0.5);
            d.kind != DimensionKind::AutoExterior || inner.iter().any(|p| point_in_polygon(mid, p))
        });
        let n = dims.len();
        for d in dims {
            cx.project.add_dimension(fl, d);
        }
        cx.mark_dirty();
        cx.status = format!("Added {n} exterior dimensions");
        ToolResult::committed("Auto Exterior Dimensions")
    }

    fn auto_interior(&mut self, cx: &mut EditorContext) -> ToolResult {
        cx.refresh();
        let sep = Self::separation(cx);
        let mut dims = Vec::new();
        let rooms = cx.rooms.clone();
        for room in &rooms {
            if room.interior_area_sq_in / 144.0 < MIN_ROOM_SQ_FT || room.inner_polygon.len() < 3 {
                continue;
            }
            let c = room.centroid;
            if !point_in_polygon(c, &room.inner_polygon) {
                continue;
            }
            let rays = rays_from(&polygon_edges(&room.inner_polygon), c);
            if let Some((l, r)) = rays.left.zip(rays.right) {
                dims.push(Dimension::new(0, DimensionKind::AutoExterior, l, r, sep));
            }
            if let Some((d, u)) = rays.down.zip(rays.up) {
                dims.push(Dimension::new(0, DimensionKind::AutoExterior, d, u, -sep));
            }
        }
        if dims.is_empty() {
            cx.status = "No rooms to dimension".into();
            return ToolResult::consumed();
        }
        if !self.lock_check(cx, AUTO_LAYER) {
            return ToolResult::consumed();
        }
        let inner: Vec<Vec<Point>> = rooms.iter().map(|r| r.inner_polygon.clone()).collect();
        cx.begin_change("Auto Interior Dimensions");
        let fl = cx.floor;
        cx.project.floors[fl].dimensions.retain(|d| {
            let (a, b) = d.line_points();
            let mid = Point::lerp(a, b, 0.5);
            d.kind != DimensionKind::AutoExterior || !inner.iter().any(|p| point_in_polygon(mid, p))
        });
        let n = dims.len();
        for d in dims {
            cx.project.add_dimension(fl, d);
        }
        cx.mark_dirty();
        cx.status = format!("Added {n} interior dimensions");
        ToolResult::committed("Auto Interior Dimensions")
    }

    // ----- elevation and story pole dimensions -----

    /// The x of the pole: the click, or a little east of the building.
    fn pole_x(cx: &EditorContext, at: Option<Point>) -> f64 {
        if let Some(p) = at {
            return p.x;
        }
        let east = cx
            .project
            .floors
            .iter()
            .flat_map(|f| f.walls.iter())
            .flat_map(|w| [w.start.x, w.end.x])
            .fold(f64::NEG_INFINITY, f64::max);
        if east.is_finite() {
            east + 96.0
        } else {
            0.0
        }
    }

    /// `+9'-0"` / `-3'-0"`: a level's height above the datum.
    fn level_text(cx: &EditorContext, height: f64) -> String {
        let sign = if height < -0.5 { "-" } else { "+" };
        format!("{sign}{}", cx.fmt_dim(height.abs()))
    }

    /// Story pole (`pole`) or elevation level dimensions: a string of vertical
    /// dimensions on one line, each level named beside it. The Y axis of the
    /// string is the height, so the values read as heights (the plan has no
    /// elevation view to put them on). A new run on the same line replaces
    /// the last one.
    fn auto_levels(&mut self, cx: &mut EditorContext, at: Option<Point>, pole: bool) -> ToolResult {
        let levels = plan_core::dimension::story_levels(&cx.project.floors);
        if levels.len() < 2 {
            cx.status = "There are no floor levels to dimension".into();
            return ToolResult::consumed();
        }
        let datum = plan_core::dimension::elevation_datum(&cx.project.floors);
        let x = Self::pole_x(cx, at);
        let sep = Self::separation(cx);
        let dims = if pole {
            plan_core::dimension::story_pole_dimensions(&levels, x, sep)
        } else {
            plan_core::dimension::elevation_dimensions(&levels, datum, x, sep)
        };
        if dims.is_empty() {
            cx.status = "There are no level heights to dimension".into();
            return ToolResult::consumed();
        }
        if !self.lock_check(cx, AUTO_LAYER) {
            return ToolResult::consumed();
        }
        let label = self.mode.label();
        let label_x = x + sep * 3.0 + 6.0;
        let text_h = cx.defaults.text.height.max(3.0);
        cx.begin_change(label);
        let fl = cx.floor;
        // A new run replaces the previous one on this line.
        cx.project.floors[fl].dimensions.retain(|d| {
            !(d.kind == DimensionKind::AutoExterior
                && (d.start.x - x).abs() < 1e-6
                && (d.end.x - x).abs() < 1e-6)
        });
        cx.project.floors[fl].cad.retain(|c| {
            !(c.layer == AUTO_LAYER
                && matches!(&c.item, CadItem::Text { pos, .. } if (pos.x - label_x).abs() < 1e-6))
        });
        let n = dims.len();
        for d in dims {
            cx.project.add_dimension(fl, d);
        }
        let mut ids = Vec::new();
        for l in &levels {
            let h = l.elevation - datum;
            let text = format!("{}  {}", l.name, Self::level_text(cx, h));
            ids.push(cx.project.add_cad(
                fl,
                AUTO_LAYER,
                CadItem::Text {
                    pos: Point::new(label_x, l.elevation - text_h * 0.5),
                    text,
                    height: text_h,
                    angle: 0.0,
                },
            ));
        }
        super::cad::group_cad(cx, &ids);
        cx.mark_dirty();
        cx.status = format!("Added {n} level dimensions");
        ToolResult::committed(label)
    }

    // ----- angular and tape measure -----

    fn angular_preview(&self, cx: &EditorContext, place: Point) -> Option<(Vec<CadItem>, f64)> {
        if self.pts.len() < 3 {
            return None;
        }
        let v = self.pts[0].point;
        let r = v.dist(place).max(6.0);
        angular_items(
            v,
            self.pts[1].point,
            self.pts[2].point,
            r,
            cx.defaults.text.height,
        )
    }

    fn commit_angular(&mut self, cx: &mut EditorContext, place: Point) -> ToolResult {
        let Some((items, deg)) = self.angular_preview(cx, place) else {
            return ToolResult::consumed();
        };
        self.reset();
        match add_cad_items(cx, MANUAL_LAYER, items, "Angular Dimension") {
            Some(_) => {
                cx.status = format!("Angle: {deg:.1}\u{b0}");
                ToolResult::committed("Angular Dimension")
            }
            None => ToolResult::consumed(),
        }
    }

    fn tape_readout(&self, cx: &EditorContext, to: Point) -> Option<String> {
        let a = self.pts.first()?.point;
        let d = to.sub(a);
        Some(format!(
            "Distance {}   \u{394}X {}   \u{394}Y {}   Angle {:.1}\u{b0}",
            cx.fmt_dim(a.dist(to)),
            cx.fmt_dim(d.x.abs()),
            cx.fmt_dim(d.y.abs()),
            d.angle().to_degrees()
        ))
    }

    // ----- selection, handles and value editing -----

    fn selected(&self, cx: &EditorContext) -> Option<Dimension> {
        match cx.selection.single()? {
            ObjectRef::Dimension(id) => cx.floor().dimensions.iter().find(|d| d.id == id).cloned(),
            _ => None,
        }
    }

    fn handle_pos(d: &Dimension, h: DimHandle) -> Point {
        match h {
            DimHandle::Offset => {
                let (a, b) = d.line_points();
                Point::lerp(a, b, 0.25)
            }
            DimHandle::Start => d.start,
            DimHandle::End => d.end,
        }
    }

    fn label_pos(d: &Dimension) -> Point {
        let (a, b) = d.line_points();
        Point::lerp(a, b, 0.5)
    }

    /// The dimension whose line, text or extension line is near `p` (DIM-30).
    fn dim_under(cx: &EditorContext, p: Point) -> Option<Id> {
        let tol = cx.pick_tol();
        cx.floor().dimensions.iter().rev().find_map(|d| {
            let layer = match d.kind {
                DimensionKind::AutoExterior => AUTO_LAYER,
                _ => MANUAL_LAYER,
            };
            if !cx.layers().is_visible(layer) {
                return None;
            }
            let (a, b) = d.line_points();
            let on_line = dist_to_segment(p, a, b) <= tol;
            let on_text = Self::label_pos(d).dist(p) <= 22.0 / cx.px_per_in.max(1e-6);
            let on_ext = d
                .extension_lines()
                .iter()
                .zip([d.start, d.end])
                .any(|((s, e), m)| dist_to_segment(p, *s, *e) <= tol && p.dist(m) > tol * 3.0);
            (on_line || on_text || on_ext).then_some(d.id)
        })
    }

    fn grab_handle(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> bool {
        let Some(d) = self.selected(cx) else {
            return false;
        };
        let tol = cx.pick_tol();
        let hit = [DimHandle::Start, DimHandle::End, DimHandle::Offset]
            .into_iter()
            .find(|h| Self::handle_pos(&d, *h).dist(p.world) <= tol);
        let Some(handle) = hit else { return false };
        self.grab = Some(DimGrab {
            id: d.id,
            handle,
            event: *p,
        });
        true
    }

    /// The grabbed handle moved: starts moving it.
    fn begin_drag(&mut self, cx: &mut EditorContext, g: DimGrab) {
        if !cx.check_unlocked(ObjectRef::Dimension(g.id)) {
            return;
        }
        cx.begin_change("Move Dimension");
        self.drag = Some(DimDrag {
            id: g.id,
            handle: g.handle,
            changed: false,
        });
    }

    fn drag_to(&mut self, cx: &mut EditorContext, p: &PointerEvent) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let unit = cx.snap_unit();
        let new_pt = match drag.handle {
            DimHandle::Offset => None,
            _ => Some(locate(cx, p, false, None).point),
        };
        let fl = cx.floor;
        if let Some(d) = cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .find(|d| d.id == drag.id)
        {
            match drag.handle {
                DimHandle::Offset => {
                    let dir = d.end.sub(d.start).normalized();
                    let mut off = p.world.sub(d.start).dot(dir.perp());
                    if !p.modifiers.alt {
                        off = (off / unit).round() * unit;
                    }
                    d.offset = off;
                }
                DimHandle::Start => d.start = new_pt.unwrap_or(d.start),
                DimHandle::End => d.end = new_pt.unwrap_or(d.end),
            }
            drag.changed = true;
        }
        cx.mark_dirty();
    }

    fn begin_edit(&mut self, cx: &mut EditorContext, id: Id) {
        let Some(d) = cx.floor().dimensions.iter().find(|d| d.id == id) else {
            return;
        };
        let len = d.length();
        self.edit = Some(ValueEdit {
            id,
            buf: format!("{len:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
        });
        set_typing(cx, true);
        cx.status =
            "Type the new dimension; the object at the nearer end moves. Enter applies".into();
    }

    fn end_edit(&mut self, cx: &mut EditorContext) {
        self.edit = None;
        set_typing(cx, false);
    }

    /// Applies a typed value (DIM-32, DIM-34).
    fn apply_value(&mut self, cx: &mut EditorContext, id: Id, new_len: f64) -> Result<(), String> {
        let Some(d) = cx.floor().dimensions.iter().find(|d| d.id == id).cloned() else {
            return Err("That dimension is gone".into());
        };
        if new_len < MIN_LENGTH {
            return Err("A dimension must be at least 1/2\"".into());
        }
        let delta = new_len - d.length();
        if delta.abs() < 1e-6 {
            return Ok(());
        }
        let dir = d.end.sub(d.start).normalized();
        let end_moves = self.cursor.dist(d.end) <= self.cursor.dist(d.start);
        let (pt, axis) = if end_moves {
            (d.end, dir)
        } else {
            (d.start, -dir)
        };
        let target = find_moveable(cx, pt, axis);
        if !cx.check_unlocked(ObjectRef::Dimension(id)) {
            return Err("That dimension is on a locked layer".into());
        }
        let fl = cx.floor;
        cx.begin_change("Edit Dimension Value");
        match target {
            Some(Moveable::WallPerp(wid)) => {
                if !cx.check_unlocked(ObjectRef::Wall(wid)) {
                    cx.cancel_change();
                    return Err("That wall is on a locked layer".into());
                }
                let n = cx
                    .floor()
                    .wall(wid)
                    .map(Wall::normal)
                    .unwrap_or(Point::ZERO);
                ops::move_wall_perpendicular(&mut cx.project, fl, wid, delta * axis.dot(n));
            }
            Some(Moveable::WallEnd(wid, end)) => {
                if !cx.check_unlocked(ObjectRef::Wall(wid)) {
                    cx.cancel_change();
                    return Err("That wall is on a locked layer".into());
                }
                let Some(w) = cx.floor().wall(wid).cloned() else {
                    cx.cancel_change();
                    return Err("Wall not found".into());
                };
                let cur = if end == WallEnd::Start {
                    w.start
                } else {
                    w.end
                };
                ops::move_wall_end_joined(
                    &mut cx.project,
                    fl,
                    wid,
                    end,
                    cur.add(axis.scale(delta)),
                );
            }
            Some(Moveable::Opening(oid, wid)) => {
                if !cx.check_unlocked(ObjectRef::Opening(oid)) {
                    cx.cancel_change();
                    return Err("That opening is on a locked layer".into());
                }
                let (Some(o), Some(w)) = (
                    cx.floor().openings.iter().find(|o| o.id == oid).cloned(),
                    cx.floor().wall(wid).cloned(),
                ) else {
                    cx.cancel_change();
                    return Err("Opening not found".into());
                };
                let center = o.center_offset + delta * axis.dot(w.direction());
                if !ops::place_opening_at(&mut cx.project, fl, oid, wid, center) {
                    cx.cancel_change();
                    return Err("The opening does not fit there".into());
                }
            }
            None => {}
        }
        // The dimension follows the moved end; a typed value replaces any
        // text override.
        let moved = pt.add(axis.scale(delta));
        if let Some(dm) = cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .find(|x| x.id == id)
        {
            if end_moves {
                dm.end = moved;
            } else {
                dm.start = moved;
            }
            dm.text_override = None;
        }
        cx.mark_dirty();
        Ok(())
    }

    fn edit_key(&mut self, cx: &mut EditorContext, k: &KeyEvent) -> ToolResult {
        if k.is(egui::Key::Escape) {
            self.end_edit(cx);
            cx.status.clear();
            return ToolResult::consumed();
        }
        if k.is(egui::Key::Enter) {
            let Some(ed) = self.edit.take() else {
                return ToolResult::ignored();
            };
            set_typing(cx, false);
            let Some(v) = parse_ft_in(&ed.buf) else {
                cx.status = format!("\"{}\" is not a length", ed.buf);
                self.edit = Some(ed);
                set_typing(cx, true);
                return ToolResult::consumed();
            };
            return match self.apply_value(cx, ed.id, v) {
                Ok(()) => {
                    cx.status.clear();
                    ToolResult::committed("Edit Dimension Value")
                }
                Err(e) => {
                    cx.status = e;
                    ToolResult::consumed()
                }
            };
        }
        if let Some(ed) = self.edit.as_mut() {
            if k.is(egui::Key::Backspace) {
                ed.buf.pop();
            } else if let Some(s) = &k.text {
                ed.buf.extend(s.chars().filter(|c| {
                    c.is_ascii_digit() || matches!(c, '.' | '-' | '/' | ' ' | '\'' | '"')
                }));
            }
        }
        ToolResult::consumed()
    }

    // ----- clicks -----

    fn strip_items(&self) -> Vec<StripButton> {
        DimMode::ALL
            .iter()
            .enumerate()
            .map(|(i, m)| StripButton::new(m.short(), i as u16, *m == self.mode))
            .collect()
    }

    /// Runs an automatic mode; `at` is the click that placed it (the story
    /// pole and the level dimensions go on its vertical line).
    fn run_auto(&mut self, cx: &mut EditorContext, at: Option<Point>) -> ToolResult {
        match self.mode {
            DimMode::AutoExterior => self.auto_exterior(cx),
            DimMode::AutoInterior => self.auto_interior(cx),
            DimMode::AutoElevation => self.auto_levels(cx, at, false),
            DimMode::AutoStoryPole => self.auto_levels(cx, at, true),
            _ => ToolResult::ignored(),
        }
    }

    /// What a click does when it is not on a handle: edit a selected
    /// dimension's value, select a dimension, or work on the new dimension.
    fn click_rest(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        if !self.in_progress() {
            // Clicking the selected dimension's text edits its value (DIM-32).
            if let Some(d) = self.selected(cx) {
                if Self::label_pos(&d).dist(p.world) <= 22.0 / cx.px_per_in.max(1e-6) {
                    if cx.check_unlocked(ObjectRef::Dimension(d.id)) {
                        self.begin_edit(cx, d.id);
                    }
                    return ToolResult::consumed();
                }
            }
            if !self.mode.is_auto() {
                if let Some(id) = Self::dim_under(cx, p.world) {
                    cx.selection.set(ObjectRef::Dimension(id));
                    return ToolResult::consumed();
                }
            }
        }
        self.start_click(cx, p)
    }

    fn start_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        match self.mode {
            DimMode::Manual | DimMode::PointToPoint | DimMode::Centerline => {
                let loc = self.locate_for(cx, p);
                match self.pts.len() {
                    0 => {
                        self.press = Some(p.screen);
                        cx.selection.clear();
                        self.pts.push(loc);
                    }
                    1 => {
                        if loc.point.dist(self.pts[0].point) >= MIN_LENGTH {
                            self.pts.push(loc);
                        }
                    }
                    _ => {
                        let dims = self.build(cx, p.snapped);
                        self.reset();
                        return self.commit(cx, dims);
                    }
                }
                ToolResult::consumed()
            }
            DimMode::EndToEnd => {
                let target = end_to_end_target(cx, p.world);
                match (self.span, target) {
                    (None, Some(t)) => {
                        cx.selection.clear();
                        self.span = Some(t);
                        ToolResult::consumed()
                    }
                    (Some(span), Some(t)) if collinear(span, t) => {
                        // Extend the string along the same line (DIM-13).
                        let u = span.1.sub(span.0).normalized();
                        let all = [span.0, span.1, t.0, t.1];
                        let along = |q: &Point| q.sub(span.0).dot(u);
                        let lo = all
                            .iter()
                            .copied()
                            .min_by(|a, b| along(a).total_cmp(&along(b)));
                        let hi = all
                            .iter()
                            .copied()
                            .max_by(|a, b| along(a).total_cmp(&along(b)));
                        if let (Some(lo), Some(hi)) = (lo, hi) {
                            self.span = Some((lo, hi));
                        }
                        ToolResult::consumed()
                    }
                    (Some(_), _) => {
                        let dims = self.build(cx, p.snapped);
                        self.reset();
                        self.commit(cx, dims)
                    }
                    (None, None) => {
                        cx.status = "Click a wall or line to dimension".into();
                        ToolResult::consumed()
                    }
                }
            }
            DimMode::Interior => {
                if let Some((o, r)) = self.interior {
                    let dims: Vec<Dimension> =
                        interior_dimension(&r, o, p.snapped).into_iter().collect();
                    self.reset();
                    return self.commit(cx, dims);
                }
                cx.refresh();
                let segs = match cx
                    .rooms
                    .iter()
                    .find(|r| point_in_polygon(p.world, &r.inner_polygon))
                {
                    Some(room) => polygon_edges(&room.inner_polygon),
                    None => cx
                        .floor()
                        .walls
                        .iter()
                        .filter(|w| !no_locate(cx, w))
                        .flat_map(|w| polygon_edges(&w.footprint()))
                        .collect(),
                };
                let rays = rays_from(&segs, p.world);
                if (rays.left.is_none() || rays.right.is_none())
                    && (rays.down.is_none() || rays.up.is_none())
                {
                    cx.status = "Click inside a room".into();
                    return ToolResult::consumed();
                }
                cx.selection.clear();
                self.interior = Some((p.world, rays));
                ToolResult::consumed()
            }
            DimMode::Running => {
                let loc = self.locate_for(cx, p);
                if self
                    .pts
                    .last()
                    .is_none_or(|l| l.point.dist(loc.point) >= MIN_LENGTH)
                {
                    if self.pts.is_empty() {
                        cx.selection.clear();
                    }
                    self.pts.push(loc);
                }
                ToolResult::consumed()
            }
            DimMode::Baseline => {
                let loc = self.locate_for(cx, p);
                if self.pts.is_empty() {
                    cx.selection.clear();
                    self.pts.push(loc);
                    self.baseline = Some((Axis::Auto, 0));
                    return ToolResult::consumed();
                }
                let origin = self.pts[0].point;
                if loc.point.dist(origin) < MIN_LENGTH {
                    return ToolResult::consumed();
                }
                let (axis, n) = match self.baseline {
                    Some((Axis::Auto, _)) | None => (dominant_axis(origin, loc.point), 0),
                    Some(s) => s,
                };
                let d = baseline_dimension(origin, loc.point, n + 1, Self::separation(cx), axis);
                self.baseline = Some((axis, n + 1));
                let saved = std::mem::take(&mut self.pts);
                let res = self.commit(cx, vec![d]);
                // The origin stays for the next row.
                self.pts = saved;
                self.baseline = Some((axis, n + 1));
                res
            }
            DimMode::Angular => {
                let loc = self.locate_for(cx, p);
                if self.pts.len() >= 3 {
                    return self.commit_angular(cx, p.snapped);
                }
                if self.pts.is_empty() {
                    cx.selection.clear();
                }
                if self
                    .pts
                    .last()
                    .is_none_or(|l| l.point.dist(loc.point) >= MIN_LENGTH)
                {
                    self.pts.push(loc);
                }
                ToolResult::consumed()
            }
            DimMode::TapeMeasure => {
                if self.pts.len() >= 2 {
                    self.pts.clear();
                }
                let loc = self.locate_for(cx, p);
                self.pts.push(loc);
                cx.readout = if self.pts.len() == 2 {
                    self.tape_readout(cx, loc.point)
                } else {
                    None
                };
                ToolResult::consumed()
            }
            DimMode::AutoExterior
            | DimMode::AutoInterior
            | DimMode::AutoElevation
            | DimMode::AutoStoryPole => self.run_auto(cx, Some(p.snapped)),
        }
    }

    fn locate_for(&self, cx: &EditorContext, p: &PointerEvent) -> Located {
        if self.mode == DimMode::PointToPoint {
            if p.modifiers.alt {
                return Located {
                    point: p.world,
                    what: "Free point",
                    obj: None,
                };
            }
            let s = cx.snap_at(p.world, self.pts.last().map(|l| l.point), false, &[]);
            return Located {
                point: s.point,
                what: s.kind.label(),
                obj: None,
            };
        }
        locate(
            cx,
            p,
            self.mode == DimMode::Centerline,
            self.pts.last().map(|l| l.point),
        )
    }

    fn update_readout(&self, cx: &mut EditorContext, to: Point) {
        cx.readout = match self.mode {
            DimMode::TapeMeasure => {
                if self.pts.len() == 1 {
                    self.tape_readout(cx, to)
                } else {
                    cx.readout.clone()
                }
            }
            DimMode::Manual | DimMode::PointToPoint | DimMode::Centerline
                if self.pts.len() == 1 =>
            {
                Some(format!(
                    "Length: {}",
                    cx.fmt_dim(self.pts[0].point.dist(to))
                ))
            }
            DimMode::Manual | DimMode::PointToPoint | DimMode::Centerline
                if self.pts.len() == 2 =>
            {
                self.build(cx, to)
                    .first()
                    .map(|d| format!("Length: {}", cx.fmt_dim(d.length())))
            }
            DimMode::Angular if self.pts.len() == 3 => self
                .angular_preview(cx, to)
                .map(|(_, deg)| format!("Angle: {deg:.1}\u{b0}")),
            _ => None,
        };
    }
}

impl Tool for DimensionTool {
    fn id(&self) -> ToolId {
        ToolId::Dimension
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
        if let ToolId::DimensionVariant(m) = id {
            self.set_mode(m);
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.reset();
        cx.status = self.mode.hint().into();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.reset();
        set_typing(cx, false);
        self.hover = None;
        cx.readout = None;
        cx.last_snap = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.cursor = p.world;
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
        let loc = if self.mode.is_auto() || self.mode == DimMode::Interior {
            Located {
                point: p.snapped,
                what: "",
                obj: None,
            }
        } else {
            self.locate_for(cx, &p)
        };
        self.hover = Some(loc);
        cx.last_snap = None;
        if !loc.what.is_empty() && !self.mode.is_auto() {
            cx.status = format!("{}: {}", self.mode.short(), loc.what);
        }
        self.update_readout(cx, loc.point);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(i) = self.strip.hit(p.screen) {
            if let Some(m) = DimMode::ALL.get(i as usize).copied() {
                self.end_edit(cx);
                self.set_mode(m);
                cx.status = m.hint().into();
                if m.is_auto() {
                    return self.run_auto(cx, None);
                }
            }
            return ToolResult::consumed();
        }
        self.cursor = p.world;
        if self.edit.is_some() {
            // A click away from the edit box cancels the edit.
            self.end_edit(cx);
            cx.status.clear();
        }
        if !self.in_progress() && self.grab_handle(cx, &p) {
            return ToolResult::consumed();
        }
        self.click_rest(cx, &p)
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(d) = self.drag.take() {
            return if d.changed {
                cx.mark_dirty();
                ToolResult::committed("Move Dimension")
            } else {
                cx.cancel_change();
                ToolResult::consumed()
            };
        }
        if let Some(g) = self.grab.take() {
            // A press on a handle that never moved is an ordinary click.
            let res = self.click_rest(cx, &g.event);
            self.press = None;
            return res;
        }
        let Some(press) = self.press.take() else {
            return ToolResult::ignored();
        };
        // Press-drag-release gives both points in one gesture (DIM-11).
        if self.pts.len() == 1
            && matches!(
                self.mode,
                DimMode::Manual | DimMode::PointToPoint | DimMode::Centerline
            )
            && (p.screen - press).length() >= DRAG_PX
        {
            let loc = self.locate_for(cx, &p);
            if loc.point.dist(self.pts[0].point) >= MIN_LENGTH {
                self.pts.push(loc);
            }
        }
        ToolResult::consumed()
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.mode == DimMode::Running && self.pts.len() >= 2 {
            let dims = self.build_running_final(cx, p.snapped);
            self.reset();
            return self.commit(cx, dims);
        }
        if !self.in_progress() {
            if let Some(id) = Self::dim_under(cx, p.world) {
                cx.selection.set(ObjectRef::Dimension(id));
                cx.requests
                    .push(EditorRequest::OpenSpec(ObjectRef::Dimension(id)));
                return ToolResult::consumed();
            }
        }
        ToolResult::ignored()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if self.edit.is_some() {
            return self.edit_key(cx, &k);
        }
        if k.is(egui::Key::Escape) {
            let had = self.in_progress() || cx.readout.is_some();
            self.reset();
            cx.readout = None;
            return if had {
                ToolResult::consumed()
            } else {
                ToolResult::ignored()
            };
        }
        if k.is(egui::Key::Enter) {
            match self.mode {
                DimMode::Running if self.pts.len() >= 2 => {
                    let dims = self.build_running_final(cx, self.cursor);
                    self.reset();
                    return self.commit(cx, dims);
                }
                DimMode::Angular if self.pts.len() == 3 => {
                    let v = self.pts[0].point;
                    let default_r = (v.dist(self.pts[1].point).min(v.dist(self.pts[2].point))
                        * 0.75)
                        .clamp(12.0, 60.0);
                    let dir = self.pts[1].point.sub(v).normalized();
                    return self.commit_angular(cx, v.add(dir.scale(default_r)));
                }
                _ => {}
            }
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        self.strip.draw(painter, cam, pal, &self.strip_items());
        let ghost = Stroke::new(1.0_f32, pal.ghost_stroke);
        let fmt = cx.dim_format();
        let hover = self.hover.map(|h| h.point).unwrap_or(self.cursor);

        // Selected dimension handles.
        if let Some(d) = self.selected(cx) {
            if !self.in_progress() {
                let sel = Stroke::new(1.5_f32, pal.selection);
                for h in [DimHandle::Start, DimHandle::End, DimHandle::Offset] {
                    let c = cam.world_to_screen(Self::handle_pos(&d, h));
                    if h == DimHandle::Offset {
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
                    } else {
                        let r = Rect::from_center_size(c, Vec2::splat(9.0));
                        painter.rect_filled(r, 0.0, pal.background);
                        painter.rect_stroke(r, 0.0, sel, egui::StrokeKind::Inside);
                    }
                }
            }
        }

        // The dimension in progress.
        for d in self.build(cx, hover) {
            render::draw_dimension(painter, cam, &d, &fmt, ghost, pal);
        }
        if self.mode == DimMode::Angular {
            if let Some((items, _)) = self.angular_preview(cx, hover) {
                for it in &items {
                    render::draw_cad(painter, cam, it, ghost, pal);
                }
            } else if let Some(v) = self.pts.first() {
                painter.line_segment(
                    [cam.world_to_screen(v.point), cam.world_to_screen(hover)],
                    ghost,
                );
            }
        }
        if self.mode == DimMode::TapeMeasure {
            if let Some(a) = self.pts.first() {
                let b = if self.pts.len() >= 2 {
                    self.pts[1].point
                } else {
                    hover
                };
                let d = Dimension::new(0, DimensionKind::Temporary, a.point, b, 12.0);
                render::draw_dimension(painter, cam, &d, &fmt, ghost, pal);
            }
        }
        if self.mode == DimMode::Interior {
            if let Some((o, _)) = self.interior {
                painter.circle_filled(cam.world_to_screen(o), 3.0, pal.ghost_stroke);
            }
        }
        for l in &self.pts {
            painter.circle_filled(cam.world_to_screen(l.point), 3.5, pal.ghost_stroke);
        }
        // The located feature under the pointer.
        if let Some(h) = self
            .hover
            .filter(|h| !h.what.is_empty() && !self.mode.is_auto())
        {
            let c = cam.world_to_screen(h.point);
            painter.circle_stroke(c, 6.0, Stroke::new(1.5_f32, pal.selection));
            painter.text(
                c + Vec2::new(10.0, -10.0),
                Align2::LEFT_BOTTOM,
                h.what,
                FontId::proportional(11.0),
                pal.dimension_text,
            );
        }
        if let Some(ed) = &self.edit {
            if let Some(d) = cx.floor().dimensions.iter().find(|d| d.id == ed.id) {
                let c = cam.world_to_screen(Self::label_pos(d));
                let galley = painter.layout_no_wrap(
                    format!("{}|", ed.buf),
                    FontId::proportional(12.0),
                    pal.dimension_text,
                );
                let r = Rect::from_center_size(c, galley.size() + Vec2::new(12.0, 6.0));
                painter.rect_filled(r, 2.0, pal.background);
                painter.rect_stroke(
                    r,
                    2.0,
                    Stroke::new(1.0_f32, pal.dimension_text),
                    egui::StrokeKind::Inside,
                );
                painter.galley(r.center() - galley.size() * 0.5, galley, pal.dimension_text);
            }
        }
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<crate::editor::EditAction> {
        cx.common_edit_actions()
    }
}

impl DimensionTool {
    /// The running string for the clicked points only (no live pointer). When
    /// `place` is on the measured line the string sits one standard row away.
    fn build_running_final(&self, cx: &EditorContext, place: Point) -> Vec<Dimension> {
        let pts = self.clicked();
        let fmt = cx.dim_format();
        let first = running_dimensions(&pts, place, &fmt);
        if first.first().is_some_and(|d| d.offset.abs() < 1.0) {
            let n = match dominant_axis(pts[0], pts[1]) {
                Axis::Vertical => Point::new(1.0, 0.0),
                _ => Point::new(0.0, 1.0),
            };
            return running_dimensions(&pts, pts[0].add(n.scale(Self::separation(cx))), &fmt);
        }
        first
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{OpeningKind, WallKind};

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    /// Two parallel interior walls 100" apart (centerlines), 6" thick.
    fn two_walls(cx: &mut EditorContext) -> (Id, Id) {
        let a = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let b = cx.project.add_wall(
            0,
            Point::new(0.0, 100.0),
            Point::new(240.0, 100.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        (a, b)
    }

    fn rect_room(cx: &mut EditorContext) {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
    }

    fn click(t: &mut DimensionTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        let r = t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
        r
    }

    fn tool(mode: DimMode) -> DimensionTool {
        let mut t = DimensionTool::default();
        t.set_mode(mode);
        t
    }

    #[test]
    fn manual_dimension_between_two_wall_faces() {
        let mut cx = new_cx();
        two_walls(&mut cx);
        let mut t = tool(DimMode::Manual);
        click(&mut t, &mut cx, 50.0, 4.0);
        click(&mut t, &mut cx, 50.0, 96.0);
        assert!(
            cx.floor().dimensions.is_empty(),
            "the line is not placed yet"
        );
        let r = click(&mut t, &mut cx, 80.0, 50.0);
        assert_eq!(r.commit.as_deref(), Some("Manual Dimension"));
        let d = &cx.floor().dimensions[0];
        assert_eq!(d.kind, DimensionKind::Manual);
        // Face to face: 100 - 3 - 3.
        assert!((d.length() - 94.0).abs() < 1e-9, "{}", d.length());
        assert!((d.line_points().0.x - 80.0).abs() < 1e-9);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Dimension(d.id)));
        // One undo step.
        assert_eq!(cx.undo().as_deref(), Some("Manual Dimension"));
        assert!(cx.floor().dimensions.is_empty());
    }

    #[test]
    fn press_drag_release_takes_both_points() {
        let mut cx = new_cx();
        two_walls(&mut cx);
        let mut t = tool(DimMode::Manual);
        let a = PointerEvent::at(&cx, Point::new(50.0, 4.0));
        let b = PointerEvent::at(&cx, Point::new(50.0, 96.0));
        t.pointer_move(&mut cx, a);
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        assert_eq!(t.clicked().len(), 2);
        click(&mut t, &mut cx, 80.0, 50.0);
        assert_eq!(cx.floor().dimensions.len(), 1);
    }

    #[test]
    fn opening_centers_and_alt_free_points() {
        let mut cx = new_cx();
        let (a, _) = two_walls(&mut cx);
        cx.project
            .add_opening(0, a, 120.0, OpeningKind::Door)
            .unwrap();
        let p = PointerEvent::at(&cx, Point::new(120.0, 1.0));
        let l = locate(&cx, &p, false, None);
        assert_eq!(l.what, "Opening center");
        assert!(l.point.dist(Point::new(120.0, 0.0)) < 1e-9);
        let alt = p.with_modifiers(egui::Modifiers {
            alt: true,
            ..egui::Modifiers::NONE
        });
        assert_eq!(locate(&cx, &alt, false, None).what, "Free point");
        // No Locate walls are skipped (DIM-5).
        cx.project.floors[0].walls[0].flags.no_locate = true;
        let l = locate(
            &cx,
            &PointerEvent::at(&cx, Point::new(50.0, 4.0)),
            false,
            None,
        );
        assert_ne!(l.obj, Some(ObjectRef::Wall(a)));
    }

    #[test]
    fn point_to_point_is_aligned() {
        let mut cx = new_cx();
        let mut t = tool(DimMode::PointToPoint);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 90.0, 90.0);
        click(&mut t, &mut cx, 30.0, 120.0);
        let d = &cx.floor().dimensions[0];
        // The angle snap puts the end on the 45 degree ray, aligned with it.
        assert!((d.length() - 90.0 * 2f64.sqrt()).abs() < 1.0, "{d:?}");
        assert!(d.start.dist(Point::ZERO) < 1e-9 && (d.end.x - d.end.y).abs() < 1e-6);
    }

    #[test]
    fn esc_cancels_a_dimension_in_progress() {
        let mut cx = new_cx();
        two_walls(&mut cx);
        let mut t = tool(DimMode::Manual);
        click(&mut t, &mut cx, 50.0, 4.0);
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(t.clicked().is_empty());
        assert!(
            !t.key(&mut cx, KeyEvent::escape()).consumed,
            "idle Esc leaves the tool"
        );
        assert!(cx.floor().dimensions.is_empty());
    }

    #[test]
    fn horizontal_and_vertical_follow_the_placement() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(100.0, 60.0);
        let h = linear_dimension(a, b, Point::new(50.0, 100.0), Axis::Auto);
        assert!((h.length() - 100.0).abs() < 1e-9 && h.start.y == h.end.y);
        let v = linear_dimension(a, b, Point::new(160.0, 30.0), Axis::Auto);
        assert!((v.length() - 60.0).abs() < 1e-9 && v.start.x == v.end.x);
    }

    #[test]
    fn end_to_end_measures_the_wall_face_and_extends_collinear_walls() {
        let mut cx = new_cx();
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        cx.project.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let mut t = tool(DimMode::EndToEnd);
        click(&mut t, &mut cx, 30.0, 4.0);
        click(&mut t, &mut cx, 160.0, 4.0);
        click(&mut t, &mut cx, 120.0, 60.0);
        let d = &cx.floor().dimensions[0];
        assert!((d.length() - 240.0).abs() < 1e-9, "{}", d.length());
        assert_eq!(cx.floor().dimensions.len(), 1);
        assert_eq!(cx.undo().as_deref(), Some("End to End Dimension"));
    }

    #[test]
    fn interior_dimension_is_the_clear_span_between_inner_faces() {
        let mut cx = new_cx();
        rect_room(&mut cx);
        let mut t = tool(DimMode::Interior);
        click(&mut t, &mut cx, 100.0, 60.0);
        // Placement above: the horizontal span, 240 - 6.
        click(&mut t, &mut cx, 100.0, 90.0);
        let d = &cx.floor().dimensions[0];
        assert!((d.length() - 234.0).abs() < 1e-6, "{}", d.length());
        // Placement to the side: the vertical span, 120 - 6.
        click(&mut t, &mut cx, 100.0, 60.0);
        click(&mut t, &mut cx, 160.0, 60.0);
        let d = &cx.floor().dimensions[1];
        assert!((d.length() - 114.0).abs() < 1e-6, "{}", d.length());
    }

    #[test]
    fn running_dimension_labels_cumulative_distances() {
        let mut cx = new_cx();
        let mut t = tool(DimMode::Running);
        for (x, y) in [(0.0, 0.0), (60.0, 0.0), (150.0, 0.0)] {
            click(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        let dims = &cx.floor().dimensions;
        assert_eq!(dims.len(), 2);
        assert_eq!(dims[0].text_override, None);
        assert_eq!(dims[1].text_override.as_deref(), Some("12'-6\""));
        assert_eq!(dims[0].offset, dims[1].offset);
    }

    #[test]
    fn baseline_dimensions_stack_from_one_origin() {
        let mut cx = new_cx();
        let mut t = tool(DimMode::Baseline);
        for (x, y) in [(0.0, 0.0), (60.0, 0.0), (150.0, 0.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let dims = &cx.floor().dimensions;
        assert_eq!(dims.len(), 2);
        assert!(dims.iter().all(|d| d.start == Point::ZERO));
        assert!((dims[0].length() - 60.0).abs() < 1e-9 && (dims[1].length() - 150.0).abs() < 1e-9);
        assert!((dims[1].offset.abs() - 2.0 * dims[0].offset.abs()).abs() < 1e-9);
        assert_eq!(t.clicked().len(), 1, "the origin stays");
    }

    #[test]
    fn angular_dimension_stores_arc_and_value() {
        let (items, deg) = angular_items(
            Point::ZERO,
            Point::new(100.0, 0.0),
            Point::new(0.0, 100.0),
            40.0,
            6.0,
        )
        .unwrap();
        assert!((deg - 90.0).abs() < 1e-9);
        assert!(matches!(items[0], CadItem::Arc { .. }));
        assert!(matches!(&items[3], CadItem::Text { text, .. } if text == "90.0\u{b0}"));
        let mut cx = new_cx();
        let mut t = tool(DimMode::Angular);
        for (x, y) in [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (45.0, 0.0)] {
            click(&mut t, &mut cx, x, y);
        }
        assert_eq!(cx.floor().cad.len(), 4);
        assert_eq!(cx.floor().groups.len(), 1);
        assert_eq!(cx.floor().cad[0].layer, MANUAL_LAYER);
    }

    #[test]
    fn tape_measure_adds_nothing() {
        let mut cx = new_cx();
        let mut t = tool(DimMode::TapeMeasure);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 90.0, 0.0);
        assert!(cx.floor().dimensions.is_empty());
        assert!(cx.readout.as_deref().unwrap_or("").contains("7'-6\""));
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(cx.readout.is_none());
    }

    #[test]
    fn auto_exterior_adds_four_dimensions_for_a_rectangle() {
        let mut cx = new_cx();
        rect_room(&mut cx);
        let mut t = tool(DimMode::AutoExterior);
        let r = click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(r.commit.as_deref(), Some("Auto Exterior Dimensions"));
        assert_eq!(cx.floor().dimensions.len(), 4);
        assert!(cx
            .floor()
            .dimensions
            .iter()
            .all(|d| d.kind == DimensionKind::AutoExterior));
        // Offsets come from the dimension defaults.
        assert!(
            (cx.floor().dimensions[0].offset - cx.defaults.dimensions.auto_exterior_offset).abs()
                < 1e-9
        );
        // Running again replaces them (DIM-25).
        click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(cx.floor().dimensions.len(), 4);
        assert_eq!(cx.undo().as_deref(), Some("Auto Exterior Dimensions"));
        assert_eq!(cx.floor().dimensions.len(), 4);
    }

    #[test]
    fn auto_interior_dimensions_each_room() {
        let mut cx = new_cx();
        rect_room(&mut cx);
        let mut t = tool(DimMode::AutoInterior);
        click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(cx.floor().dimensions.len(), 2);
        let mut lens: Vec<f64> = cx
            .floor()
            .dimensions
            .iter()
            .map(Dimension::length)
            .collect();
        lens.sort_by(f64::total_cmp);
        assert!((lens[0] - 114.0).abs() < 1e-6 && (lens[1] - 234.0).abs() < 1e-6);
        // Exterior dimensions are untouched by it, and vice versa.
        let mut ext = tool(DimMode::AutoExterior);
        click(&mut ext, &mut cx, 120.0, 60.0);
        assert_eq!(cx.floor().dimensions.len(), 6);
    }

    /// Selects the dimension, clicks its text, types a value, presses Enter.
    fn edit_value(
        t: &mut DimensionTool,
        cx: &mut EditorContext,
        id: Id,
        near: Point,
        text: &str,
    ) -> ToolResult {
        cx.selection.set(ObjectRef::Dimension(id));
        let d = cx
            .floor()
            .dimensions
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .clone();
        let label = DimensionTool::label_pos(&d);
        click(t, cx, label.x, label.y);
        assert!(t.edit.is_some(), "clicking the text opens the edit box");
        assert!(cx.temp.editing.is_some());
        let mv = PointerEvent::at(cx, near);
        t.pointer_move(cx, mv);
        for _ in 0..12 {
            t.key(cx, KeyEvent::key(egui::Key::Backspace));
        }
        t.key(cx, KeyEvent::text(text));
        t.key(cx, KeyEvent::key(egui::Key::Enter))
    }

    #[test]
    fn editing_a_dimension_value_moves_the_wall_at_the_nearer_end() {
        let mut cx = new_cx();
        let (w1, w2) = two_walls(&mut cx);
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(50.0, 3.0),
                Point::new(50.0, 97.0),
                30.0,
            ),
        );
        let mut t = tool(DimMode::Manual);
        let r = edit_value(&mut t, &mut cx, id, Point::new(50.0, 90.0), "104");
        assert_eq!(r.commit.as_deref(), Some("Edit Dimension Value"));
        // The end nearer the cursor (on wall 2) moved up by 10".
        let f = cx.floor();
        assert_eq!(f.wall(w2).unwrap().start.y, 110.0);
        assert_eq!(f.wall(w1).unwrap().start.y, 0.0);
        let d = &f.dimensions[0];
        assert!((d.length() - 104.0).abs() < 1e-9 && d.end.y == 107.0);
        assert!(cx.temp.editing.is_none());
        // Undo puts it all back.
        cx.undo();
        assert_eq!(cx.floor().wall(w2).unwrap().start.y, 100.0);
        assert!((cx.floor().dimensions[0].length() - 94.0).abs() < 1e-9);
    }

    #[test]
    fn editing_the_other_end_moves_the_other_wall() {
        let mut cx = new_cx();
        let (w1, w2) = two_walls(&mut cx);
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(50.0, 3.0),
                Point::new(50.0, 97.0),
                30.0,
            ),
        );
        let mut t = tool(DimMode::Manual);
        edit_value(&mut t, &mut cx, id, Point::new(50.0, 10.0), "7'");
        // 94 -> 84: the start end (wall 1) moves up by 10.
        assert_eq!(cx.floor().wall(w1).unwrap().start.y, 10.0);
        assert_eq!(cx.floor().wall(w2).unwrap().start.y, 100.0);
    }

    #[test]
    fn value_edit_slides_an_opening() {
        let mut cx = new_cx();
        let (a, _) = two_walls(&mut cx);
        let o = cx
            .project
            .add_opening(0, a, 100.0, OpeningKind::Window)
            .unwrap();
        // Wall start to the window center: 100".
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                -20.0,
            ),
        );
        let mut t = tool(DimMode::Manual);
        edit_value(&mut t, &mut cx, id, Point::new(95.0, 0.0), "10'");
        let c = cx
            .floor()
            .openings
            .iter()
            .find(|x| x.id == o)
            .unwrap()
            .center_offset;
        assert!((c - 120.0).abs() < 1e-9, "{c}");
    }

    #[test]
    fn value_edit_refuses_locked_layers() {
        let mut cx = new_cx();
        let (_, w2) = two_walls(&mut cx);
        cx.project.layers.set_locked("Walls, Normal", true);
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(50.0, 3.0),
                Point::new(50.0, 97.0),
                30.0,
            ),
        );
        let mut t = tool(DimMode::Manual);
        let r = edit_value(&mut t, &mut cx, id, Point::new(50.0, 90.0), "104");
        assert!(r.commit.is_none());
        assert!(cx.status.contains("locked"), "{}", cx.status);
        assert_eq!(cx.floor().wall(w2).unwrap().start.y, 100.0);
    }

    #[test]
    fn dragging_the_move_handle_changes_the_offset() {
        let mut cx = new_cx();
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::ZERO,
                Point::new(100.0, 0.0),
                20.0,
            ),
        );
        cx.selection.set(ObjectRef::Dimension(id));
        let mut t = tool(DimMode::Manual);
        // The move handle sits a quarter of the way along the dimension line.
        let down = PointerEvent::at(&cx, Point::new(25.0, 20.0));
        t.pointer_down(&mut cx, down.with_down(true));
        let to = PointerEvent::at(&cx, Point::new(25.0, 50.0));
        t.pointer_move(&mut cx, to.with_down(true));
        let r = t.pointer_up(&mut cx, to);
        assert_eq!(r.commit.as_deref(), Some("Move Dimension"));
        assert_eq!(cx.floor().dimensions[0].offset, 50.0);
        assert!(t.clicked().is_empty());
    }

    #[test]
    fn the_next_dimension_can_start_where_the_selected_one_ends() {
        let mut cx = new_cx();
        two_walls(&mut cx);
        let mut t = tool(DimMode::Manual);
        click(&mut t, &mut cx, 50.0, 4.0);
        click(&mut t, &mut cx, 50.0, 96.0);
        click(&mut t, &mut cx, 80.0, 50.0);
        assert_eq!(cx.floor().dimensions.len(), 1);
        // The dimension is selected and its end handle sits at (50, 97):
        // a plain click there starts the next dimension.
        click(&mut t, &mut cx, 50.0, 96.0);
        assert_eq!(t.clicked().len(), 1, "{:?}", t.clicked());
        assert_eq!(cx.floor().dimensions[0].end, Point::new(50.0, 97.0));
    }

    #[test]
    fn clicking_a_dimension_selects_it_and_double_click_opens_its_dialog() {
        let mut cx = new_cx();
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::ZERO,
                Point::new(100.0, 0.0),
                20.0,
            ),
        );
        let mut t = tool(DimMode::Manual);
        click(&mut t, &mut cx, 80.0, 20.0);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Dimension(id)));
        let p = PointerEvent::at(&cx, Point::new(80.0, 20.0));
        assert!(t.double_click(&mut cx, p).consumed);
        assert_eq!(
            cx.requests,
            vec![EditorRequest::OpenSpec(ObjectRef::Dimension(id))]
        );
    }

    #[test]
    fn variants_by_name_and_request() {
        let mut t = DimensionTool::default();
        assert!(t.set_mode_by_name("end to end dimension"));
        assert_eq!(t.mode(), DimMode::EndToEnd);
        assert!(!t.set_mode_by_name("nope"));
        t.set_variant(ToolId::DimensionVariant(DimMode::TapeMeasure));
        assert_eq!(t.mode(), DimMode::TapeMeasure);
        for m in DimMode::ALL {
            assert_eq!(DimMode::from_name(m.name()), Some(m));
        }
    }

    #[test]
    fn story_pole_dimensions_stack_heights_and_name_the_levels() {
        let mut cx = new_cx();
        let second = cx.project.build_new_floor(false);
        cx.floor = 0;
        let mut t = tool(DimMode::AutoStoryPole);
        click(&mut t, &mut cx, 400.0, 0.0);
        let dims: Vec<&Dimension> = cx.floor().dimensions.iter().collect();
        // 1st floor ceiling, platform, 2nd floor ceiling, plus the overall.
        assert_eq!(dims.len(), 4);
        assert!(dims.iter().all(|d| d.start.x == 400.0 && d.end.x == 400.0));
        let ceiling = cx.project.floors[0].ceiling_height;
        assert!(dims.iter().any(|d| (d.length() - ceiling).abs() < 1e-6));
        assert!(dims
            .iter()
            .any(|d| (d.length() - plan_core::floors::FLOOR_PLATFORM_THICKNESS).abs() < 1e-6));
        let names: Vec<&str> = cx
            .floor()
            .cad
            .iter()
            .filter_map(|c| match &c.item {
                CadItem::Text { text, .. } if c.layer == AUTO_LAYER => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names.len(), 4);
        assert!(names
            .iter()
            .any(|n| n.starts_with("1st Floor Floor") || n.contains("Floor")));
        let _ = second;
        // Running it again on the same line replaces, not stacks.
        click(&mut t, &mut cx, 400.0, 20.0);
        assert_eq!(cx.floor().dimensions.len(), 4);
        assert_eq!(
            cx.floor()
                .cad
                .iter()
                .filter(|c| c.layer == AUTO_LAYER)
                .count(),
            4
        );
        assert_eq!(cx.undo().as_deref(), Some("Auto Story Pole Dimensions"));
    }

    #[test]
    fn elevation_dimensions_measure_from_the_first_floor() {
        let mut cx = new_cx();
        cx.project.build_new_floor(false);
        cx.floor = 0;
        let mut t = tool(DimMode::AutoElevation);
        click(&mut t, &mut cx, 400.0, 0.0);
        let dims = &cx.floor().dimensions;
        // Levels above the datum: 1st ceiling, 2nd floor, 2nd ceiling.
        assert_eq!(dims.len(), 3);
        let top = dims.iter().map(|d| d.length()).fold(0.0, f64::max);
        let f1 = &cx.project.floors[0];
        let f2 = &cx.project.floors[1];
        assert!((top - (f2.elevation + f2.ceiling_height - f1.elevation)).abs() < 1e-6);
        assert!(dims[1].offset > dims[0].offset, "stacked baseline style");
        // Strip click runs it east of the building without a click.
        let mut t = tool(DimMode::AutoStoryPole);
        let n = cx.floor().dimensions.len();
        assert_eq!(n, 3);
        assert!(t.run_auto(&mut cx, None).commit.is_some());
    }
}
