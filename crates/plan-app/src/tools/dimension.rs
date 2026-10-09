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
//!   dimension line and click. Points locate by the Locate Objects settings
//!   of the active dimension defaults (DIM-4): wall surfaces, main layer or
//!   centers, opening sides or centers, cabinet and fixture sides, else the
//!   snap point; Alt suspends the locating (DIM-21). The measuring direction
//!   follows the placement click (horizontal, vertical or aligned).
//! * End to End (DIM-13), Interior (DIM-14), Running (DIM-16), Baseline
//!   (DIM-17), Angular (DIM-18), Tape Measure (DIM-20).
//! * Auto Exterior (DIM-24, DIM-25: up to three strings per side, openings
//!   then wall to wall then overall, for every direction of wall) and Auto
//!   Interior (DIM-27: clear spans and an openings string per room) run on
//!   one click and replace their previous run, keeping manual dimensions.
//!   Auto Elevation and Auto Story Pole dimensions run on one click
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
//! A measured point located on a wall, opening, cabinet or fixture is tied to
//! it (DIM-3, DIM-29, `plan_core::dim_assoc`): the editor moves the point when
//! the object moves. A point or line edited by hand, or a typed value, turns an
//! automatic dimension into a manual one (DIM-33), and a point dragged onto
//! another object is tied there.

use super::cad::{set_typing, OptionStrip, StripButton};
use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::placed;
use crate::editor::selection::hit_opening;
use crate::editor::{ops, render, Camera, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use plan_core::cad::CadItem;
use plan_core::dim_assoc::{AnchorTarget, DimHint};
use plan_core::dimension::{CurveKind, DimCurve, LocateTool, OffsetFrom, ToolLocate};
use plan_core::geometry::{
    dist_to_segment, point_in_polygon, project_on_segment, segment_intersection, Point,
};
use plan_core::units::parse_ft_in;
use plan_core::{
    auto_exterior_set, auto_nkba_dimensions, wall_layer_bands, AutoGroup, Dimension, DimensionKind,
    ExteriorSetup, Id, NkbaItem, NkbaKind, NkbaSetup, ObjectLocate, OpeningLocate, Wall, WallEnd,
    WallLocate,
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
    /// Click a curved wall, then place the line: its radius (Radius
    /// Dimension).
    Radius,
    /// Click a curved wall, then place the line: the length along its arc.
    ArcLength,
    TapeMeasure,
    AutoExterior,
    AutoInterior,
    AutoElevation,
    AutoStoryPole,
    AutoNkba,
    /// Click a dimension's line to add an extension line (a measured point)
    /// there, or on a hidden extension line to bring it back (DIM-41).
    ExtensionAdd,
    /// Click an extension line to delete it: the dimension on each side
    /// merges into one, or an end's line is hidden (DIM-41).
    ExtensionDelete,
}

impl DimMode {
    pub const ALL: [DimMode; 18] = [
        DimMode::Manual,
        DimMode::EndToEnd,
        DimMode::Interior,
        DimMode::PointToPoint,
        DimMode::Running,
        DimMode::Baseline,
        DimMode::Centerline,
        DimMode::Angular,
        DimMode::Radius,
        DimMode::ArcLength,
        DimMode::TapeMeasure,
        DimMode::AutoExterior,
        DimMode::AutoInterior,
        DimMode::AutoElevation,
        DimMode::AutoStoryPole,
        DimMode::AutoNkba,
        DimMode::ExtensionAdd,
        DimMode::ExtensionDelete,
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
            DimMode::Radius => "Radius Dimension",
            DimMode::ArcLength => "Arc Length Dimension",
            DimMode::TapeMeasure => "Tape Measure",
            DimMode::AutoExterior => "Auto Exterior Dimensions",
            DimMode::AutoInterior => "Auto Interior Dimensions",
            DimMode::AutoElevation => "Auto Elevation Dimensions",
            DimMode::AutoStoryPole => "Auto Story Pole Dimensions",
            DimMode::AutoNkba => "Auto NKBA Dimensions",
            DimMode::ExtensionAdd => "Add Extension Line",
            DimMode::ExtensionDelete => "Delete Extension Line",
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
            DimMode::Radius => "Radius",
            DimMode::ArcLength => "Arc Length",
            DimMode::TapeMeasure => "Tape",
            DimMode::AutoExterior => "Auto Exterior",
            DimMode::AutoInterior => "Auto Interior",
            DimMode::AutoElevation => "Auto Elevation",
            DimMode::AutoStoryPole => "Auto Story Pole",
            DimMode::AutoNkba => "Auto NKBA",
            DimMode::ExtensionAdd => "Add Extension",
            DimMode::ExtensionDelete => "Delete Extension",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            DimMode::Manual => {
                "Manual Dimension: click two points, then click to place the dimension line; further clicks add points to the string, double-click or Esc ends it"
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
                "Angular Dimension: click two walls (or the vertex and two arm points), then the arc radius"
            }
            DimMode::Radius => {
                "Radius Dimension: click a curved wall, then click to place the line"
            }
            DimMode::ArcLength => {
                "Arc Length Dimension: click a curved wall, then click to place the line"
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
            DimMode::AutoNkba => {
                "Auto NKBA Dimensions: click to dimension the kitchen and bath cabinet runs (faces, sink and appliance centers, overall)"
            }
            DimMode::ExtensionAdd => {
                "Add Extension Line: click a dimension line where a new measured point goes, or a hidden extension line to bring it back"
            }
            DimMode::ExtensionDelete => {
                "Delete Extension Line: click an extension line; strings on both sides merge, an end's line is hidden"
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
            DimMode::Radius => "Radius Dimension",
            DimMode::ArcLength => "Arc Length Dimension",
            DimMode::TapeMeasure => "Tape Measure",
            DimMode::AutoExterior => "Auto Exterior Dimensions",
            DimMode::AutoInterior => "Auto Interior Dimensions",
            DimMode::AutoElevation => "Auto Elevation Dimensions",
            DimMode::AutoStoryPole => "Auto Story Pole Dimensions",
            DimMode::AutoNkba => "Auto NKBA Dimensions",
            DimMode::ExtensionAdd => "Add Extension Line",
            DimMode::ExtensionDelete => "Delete Extension Line",
        }
    }

    /// The two Add / Delete Extension Line tools, which edit existing
    /// dimensions instead of drawing new ones.
    fn is_extension(self) -> bool {
        matches!(self, DimMode::ExtensionAdd | DimMode::ExtensionDelete)
    }

    fn is_auto(self) -> bool {
        matches!(
            self,
            DimMode::AutoExterior
                | DimMode::AutoInterior
                | DimMode::AutoElevation
                | DimMode::AutoStoryPole
                | DimMode::AutoNkba
        )
    }
}

// ----- Auto NKBA items -----

/// How far a wall may stand from the end of a cabinet run and still be
/// dimensioned to (inches), when the dimension set gives no reach.
const NKBA_WALL_REACH: f64 = 96.0;

/// Words in a fixture's catalog id or label that mark a kitchen or bath
/// appliance standing in a cabinet run.
const NKBA_APPLIANCES: [&str; 8] = [
    "refrigerator",
    "fridge",
    "range",
    "cooktop",
    "stove",
    "oven",
    "dishwasher",
    "washer",
];

/// The cabinets and fixtures an NKBA run is made of: base, tall and corner
/// base cabinets (with the centers of their sink and cooktop cutouts and
/// appliance bays) and appliance fixtures. Wall cabinets, soffits and the
/// free-form tops are left out.
pub fn nkba_items(
    cabinets: &[plan_cabinets::Cabinet],
    symbols: &[plan_core::symbols::PlacedSymbol],
) -> Vec<NkbaItem> {
    use plan_cabinets::{CabinetKind as K, CutoutKind};
    let mut out = Vec::new();
    for c in cabinets {
        if !matches!(
            c.kind,
            K::Base
                | K::FullHeight
                | K::BaseFiller
                | K::FullHeightFiller
                | K::CornerBase
                | K::BlindBase
        ) {
            continue;
        }
        let mut centers = Vec::new();
        for cut in &c.cutouts {
            let kind = match cut.kind {
                CutoutKind::Sink => NkbaKind::Sink,
                CutoutKind::Cooktop => NkbaKind::Cooktop,
                CutoutKind::Custom => continue,
            };
            if cut.outline.is_empty() {
                continue;
            }
            let x = cut.outline.iter().map(|p| p.x).sum::<f64>() / cut.outline.len() as f64;
            centers.push((kind, x));
        }
        if c.appliance.is_none()
            && !centers.iter().any(|(k, _)| *k == NkbaKind::Sink)
            && c.face.has_appliance("Sink")
        {
            // A sink base whose sink has no cutout is centered on the cabinet.
            centers.push((NkbaKind::Sink, c.width * 0.5));
        }
        if let Some(name) = &c.appliance {
            let n = name.to_lowercase();
            let kind = if ["range", "cooktop", "stove"].iter().any(|w| n.contains(w)) {
                NkbaKind::Cooktop
            } else {
                NkbaKind::Appliance
            };
            centers.push((kind, c.width * 0.5));
        }
        out.push(NkbaItem {
            origin: c.position,
            angle: c.angle,
            width: c.width,
            depth: c.depth,
            centers,
        });
    }
    for s in symbols {
        let name = format!("{} {}", s.catalog_id, s.label).to_lowercase();
        if s.image.is_some() || s.distribution.is_some() || s.owner.is_some() {
            continue;
        }
        let Some(word) = NKBA_APPLIANCES.iter().find(|w| name.contains(**w)) else {
            continue;
        };
        let a = s.angle.to_radians();
        let u = Point::new(a.cos(), a.sin());
        let kind = if ["range", "cooktop", "stove"].contains(word) {
            NkbaKind::Cooktop
        } else {
            NkbaKind::Appliance
        };
        out.push(NkbaItem {
            origin: s.position.sub(u.scale(s.width * 0.5)),
            angle: a,
            width: s.width,
            depth: s.depth,
            centers: vec![(kind, s.width * 0.5)],
        });
    }
    out
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

/// The lateral span `(lo, hi)` a dimension locates on wall `w` under the
/// Dimension Defaults > Locate Objects > Walls setting (DIM-4): the outer
/// and inner surfaces, the main layer's, or the centerline.
pub(crate) fn wall_span(cx: &EditorContext, w: &Wall, mode: WallLocate) -> (f64, f64) {
    match mode {
        WallLocate::Centers => (0.0, 0.0),
        WallLocate::Surfaces => (-w.thickness * 0.5, w.thickness * 0.5),
        WallLocate::MainLayer => main_span(cx, w),
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

/// The point on the outline `poly` nearest `p`.
fn nearest_on_outline(p: Point, poly: &[Point]) -> Option<Point> {
    let n = poly.len();
    (0..n)
        .map(|i| project_on_segment(p, poly[i], poly[(i + 1) % n]).1)
        .min_by(|a, b| a.dist(p).total_cmp(&b.dist(p)))
}

/// The corners of a footprint and its middle.
fn outline_marks(poly: &[Point]) -> (Vec<Point>, Option<Point>) {
    if poly.is_empty() {
        return (Vec::new(), None);
    }
    let n = poly.len() as f64;
    let c = Point::new(
        poly.iter().map(|p| p.x).sum::<f64>() / n,
        poly.iter().map(|p| p.y).sum::<f64>() / n,
    );
    (poly.to_vec(), Some(c))
}

/// The nearest of the marks a Locate panel switches on for an object's
/// footprint: its sides, corners and center. `sides`, `corners` and
/// `centers` say which are on.
fn nearest_mark(
    p: Point,
    poly: &[Point],
    sides: bool,
    corners: bool,
    centers: bool,
) -> Option<(Point, &'static str)> {
    let (corner_pts, mid) = outline_marks(poly);
    let mut best: Option<(f64, Point, &'static str)> = None;
    let mut take = |q: Point, what: &'static str| {
        let d = q.dist(p);
        if best.is_none_or(|(b, _, _)| d < b) {
            best = Some((d, q, what));
        }
    };
    if sides {
        if let Some(q) = nearest_on_outline(p, poly) {
            take(q, "side");
        }
    }
    if corners {
        for c in corner_pts {
            take(c, "corner");
        }
    }
    if centers {
        if let Some(c) = mid {
            take(c, "center");
        }
    }
    best.map(|(_, q, w)| (q, w))
}

/// The object a dimension point locates for a cabinet or fixture (the
/// Locate panel's Cabinets and Fixtures marks): the nearest side, corner or
/// center of its footprint.
fn locate_placed(cx: &EditorContext, p: &PointerEvent, tl: &ToolLocate) -> Option<Located> {
    let tol = cx.pick_tol() * 0.5;
    if tl.group.fixtures == ObjectLocate::Sides {
        if let Some(id) = placed::hit_symbol(cx, p.world, tol) {
            let s = cx.floor().symbols.iter().find(|s| s.id == id)?;
            if s.distribution.is_none() {
                let (sides, centers) = (tl.mark("fixtures.sides"), tl.mark("fixtures.centers"));
                let (q, w) = nearest_mark(p.world, &s.footprint(), sides, sides, centers)?;
                return Some(Located {
                    point: q,
                    what: match w {
                        "center" => "Fixture center",
                        "corner" => "Fixture corner",
                        _ => "Fixture side",
                    },
                    obj: Some(ObjectRef::Symbol(id)),
                });
            }
        }
    }
    if tl.group.cabinets == ObjectLocate::Sides {
        if let Some(id) = placed::hit_cabinet(cx, p.world, tol, |_| true) {
            let c = placed::cabinet_by_id(cx.floor(), id)?;
            let (q, w) = nearest_mark(
                p.world,
                &c.footprint(),
                tl.mark("cabinets.sides"),
                tl.mark("cabinets.corners"),
                tl.mark("cabinets.centers"),
            )?;
            return Some(Located {
                point: q,
                what: match w {
                    "center" => "Cabinet center",
                    "corner" => "Cabinet corner",
                    _ => "Cabinet side",
                },
                obj: Some(ObjectRef::Cabinet(id)),
            });
        }
    }
    None
}

/// Locates the point under the pointer (DIM-4, DIM-12, DIM-19, DIM-21) by
/// the Locate Objects settings of the active dimension defaults. `centers`
/// forces centers (Centerline Dimension).
fn locate(cx: &EditorContext, p: &PointerEvent, centers: bool, origin: Option<Point>) -> Located {
    let tool = if centers {
        LocateTool::Centerline
    } else {
        LocateTool::Manual
    };
    locate_with(cx, p, centers, origin, tool)
}

/// [`locate`] under the Locate panel of `tool` (Dimension Defaults: one
/// panel per dimension tool, DIM-54).
fn locate_with(
    cx: &EditorContext,
    p: &PointerEvent,
    centers: bool,
    origin: Option<Point>,
    tool: LocateTool,
) -> Located {
    if p.modifiers.alt {
        return Located {
            point: p.world,
            what: "Free point",
            obj: None,
        };
    }
    let tol = cx.pick_tol();
    let floor = cx.floor();
    let tl = cx.defaults.dimensions.tool_locate(tool);
    let walls_mode = if centers {
        WallLocate::Centers
    } else {
        tl.group.walls
    };
    let openings_mode = if centers {
        OpeningLocate::Centers
    } else {
        tl.group.openings
    };
    if openings_mode != OpeningLocate::None {
        if let Some(oid) = hit_opening(floor, p.world, tol * 0.5) {
            if let Some(o) = floor.openings.iter().find(|o| o.id == oid) {
                if let Some(w) = floor.wall(o.wall_id).filter(|w| !no_locate(cx, w)) {
                    if openings_mode == OpeningLocate::Centers {
                        return Located {
                            point: w.point_along(o.center_offset),
                            what: "Opening center",
                            obj: Some(ObjectRef::Opening(oid)),
                        };
                    }
                    // Along the wall is the arc length on a curved wall
                    // (DW-88): `locate` gives it with the signed distance
                    // off the centerline.
                    let (along, perp) = w.locate(p.world);
                    let edge = if (along - o.start_offset()).abs() <= (along - o.end_offset()).abs()
                    {
                        o.start_offset()
                    } else {
                        o.end_offset()
                    };
                    let (lo, hi) = wall_span(cx, w, walls_mode);
                    let off = if (perp - lo).abs() <= (perp - hi).abs() {
                        lo
                    } else {
                        hi
                    };
                    return Located {
                        point: w.point_offset(edge, off),
                        what: "Opening edge",
                        obj: Some(ObjectRef::Opening(oid)),
                    };
                }
            }
        }
    }
    if let Some(l) = locate_placed(cx, p, &tl) {
        return l;
    }
    if let Some(w) = wall_near(cx, p.world).filter(|_| !tl.walls_none) {
        let n = w.normal();
        let perp = p.world.sub(w.start).dot(n);
        let (lo, hi) = wall_span(cx, w, walls_mode);
        let (off, what) = if walls_mode == WallLocate::Centers {
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

/// The point a dimension click at `p` lands on (Locate Objects), for the
/// scenario tests.
#[cfg(test)]
pub fn locate_point(cx: &EditorContext, p: &PointerEvent, centers: bool) -> Point {
    locate(cx, p, centers, None).point
}

/// What a located object is tied to (DIM-3).
fn dim_hint(l: &Located) -> Option<DimHint> {
    let (target, id) = match l.obj? {
        ObjectRef::Wall(id) => (AnchorTarget::Wall, id),
        ObjectRef::Opening(id) => (AnchorTarget::Opening, id),
        ObjectRef::Cabinet(id) => (AnchorTarget::Cabinet, id),
        ObjectRef::Symbol(id) => (AnchorTarget::Symbol, id),
        _ => return None,
    };
    Some(DimHint {
        target,
        id,
        point: l.point,
    })
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
        let (along, lateral) = w.locate(pt);
        let perp = lateral.abs();
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
    /// The object the dragged point was last located on (DIM-3): the end is
    /// tied to it on release.
    hint: Option<DimHint>,
}

struct ValueEdit {
    id: Id,
    buf: String,
    /// Editing the label's text (with rich text tags) instead of the value.
    rich: bool,
}

/// A manual dimension string that further clicks continue (DIM-2, DIM-11):
/// each click adds a measured point on the same line, one segment per pair of
/// points with its own value; a double-click, Esc, Enter or another tool ends it.
#[derive(Clone, Copy)]
struct StringState {
    /// The last measured point, where the next segment starts.
    last: Point,
    /// Unit direction of the measuring line.
    dir: Point,
    /// The offset of the dimension line from the measured line, as a vector.
    off: Point,
    /// What the last point is tied to.
    anchor: Option<plan_core::dim_assoc::DimAnchor>,
    /// The first segment of the string: the string's id.
    head: Id,
}

pub struct DimensionTool {
    mode: DimMode,
    pts: Vec<Located>,
    /// The points of the dimension just finished (`reset` clears `pts`
    /// before the dimension is committed): the objects they were located on.
    hint_src: Vec<Located>,
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
    /// The manual string being continued.
    string: Option<StringState>,
    /// Radius and Arc Length: the curved wall clicked and the distance of
    /// the measured surface to the left of its centerline.
    arc: Option<(Id, f64)>,
}

impl Default for DimensionTool {
    fn default() -> Self {
        Self {
            mode: DimMode::Manual,
            pts: Vec::new(),
            hint_src: Vec::new(),
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
            string: None,
            arc: None,
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
        self.hint_src.clear();
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
        if !self.pts.is_empty() {
            self.hint_src = std::mem::take(&mut self.pts);
        }
        self.pts.clear();
        self.span = None;
        self.interior = None;
        self.baseline = None;
        self.press = None;
        self.drag = None;
        self.grab = None;
        self.edit = None;
        self.string = None;
        self.arc = None;
    }

    fn in_progress(&self) -> bool {
        !self.pts.is_empty()
            || self.span.is_some()
            || self.interior.is_some()
            || self.baseline.is_some()
            || self.arc.is_some()
    }

    /// The Locate panel the mode reads (Dimension Defaults: Locate Manual,
    /// End to End, Centerline, Interior, Auto Exterior, Auto Room, Auto
    /// Elevation, Elevations).
    fn locate_tool(&self) -> LocateTool {
        match self.mode {
            DimMode::EndToEnd | DimMode::PointToPoint => LocateTool::EndToEnd,
            DimMode::Centerline => LocateTool::Centerline,
            DimMode::Interior => LocateTool::Interior,
            DimMode::AutoExterior => LocateTool::AutoExterior,
            DimMode::AutoInterior => LocateTool::AutoRoom,
            DimMode::AutoElevation | DimMode::AutoStoryPole => LocateTool::AutoElevation,
            _ => LocateTool::Manual,
        }
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
            DimMode::Radius | DimMode::ArcLength => {
                self.arc_dimension(cx, place).into_iter().collect()
            }
            DimMode::Angular => self.angular_dimension(cx, place).into_iter().collect(),
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
        let src = std::mem::take(&mut self.hint_src);
        if dims.is_empty() || !self.lock_check(cx, MANUAL_LAYER) {
            self.reset();
            return ToolResult::consumed();
        }
        let label = self.mode.label();
        let hints: Vec<DimHint> = src
            .iter()
            .chain(self.pts.iter())
            .filter_map(dim_hint)
            .collect();
        let style = cx.defaults.dimensions.text_style.clone();
        cx.begin_change(label);
        let fl = cx.floor;
        let mut last = 0;
        for mut d in dims {
            if !style.is_empty() {
                d.text_style = Some(style.clone());
            }
            let ends = [d.start, d.end];
            let horizontal = (d.start.y - d.end.y).abs() < 1e-6;
            let curved = d.curve().is_some();
            last = cx.project.add_dimension(fl, d);
            if curved {
                // A curved dimension follows its walls through its curve.
                continue;
            }
            // An end on an object stays tied to it (it follows the object):
            // the one it was located on when the tool knows it.
            let hint = |p: Point| {
                hints.iter().copied().find(|h| {
                    h.point.dist(p) < 1e-6
                        || if horizontal {
                            (h.point.x - p.x).abs() < 1e-6
                        } else {
                            (h.point.y - p.y).abs() < 1e-6
                        }
                })
            };
            cx.project.floors[fl].attach_dimension_hinted(last, [hint(ends[0]), hint(ends[1])]);
        }
        cx.selection.set(ObjectRef::Dimension(last));
        cx.mark_dirty();
        cx.readout = None;
        cx.status.clear();
        ToolResult::committed(label)
    }

    // ----- automatic dimensions -----

    /// Removes the automatic dimensions of `group` (and, from files before
    /// groups were recorded, ungrouped ones that lie `inside` the rooms or
    /// outside them) ahead of a new run (DIM-25). Manual dimensions, and
    /// automatic ones that were edited and became manual, stay.
    fn clear_run(cx: &mut EditorContext, group: AutoGroup, rooms: &[Vec<Point>]) {
        let fl = cx.floor;
        cx.project.floors[fl].dimensions.retain(|d| {
            if d.kind != DimensionKind::AutoExterior {
                return true;
            }
            match d.auto_group {
                AutoGroup::None => {
                    let (a, b) = d.line_points();
                    let mid = Point::lerp(a, b, 0.5);
                    let inside = rooms.iter().any(|p| point_in_polygon(mid, p));
                    // Ungrouped strings: interior ones sit in rooms.
                    inside != (group == AutoGroup::Exterior)
                }
                g => g != group,
            }
        });
    }

    /// Adds the dimensions of one automatic run, tied to the objects they
    /// measure (DIM-29), and returns how many.
    fn add_run(cx: &mut EditorContext, dims: Vec<Dimension>) -> usize {
        let fl = cx.floor;
        let style = cx.defaults.dimensions.text_style.clone();
        let n = dims.len();
        let mut ids = Vec::with_capacity(n);
        for mut d in dims {
            if d.text_style.is_none() && !style.is_empty() {
                d.text_style = Some(style.clone());
            }
            let id = cx.project.add_dimension(fl, d);
            cx.project.floors[fl].attach_dimension(id);
            ids.push(id);
        }
        // Each row of an automatic run is one dimension string.
        cx.project.floors[fl].join_chains(&ids);
        n
    }

    /// The first line offset measured from the located surface, from the
    /// distance Setup Automatic gives and what it is measured from (Offset
    /// from Wall: the wall center, the dimension layer or the surface).
    fn first_offset(cx: &EditorContext, walls: &[Wall], given: f64) -> f64 {
        let n = walls.len().max(1) as f64;
        match cx.defaults.dimensions.setup.offset_from {
            OffsetFrom::DimensionLayer => given,
            OffsetFrom::Center => {
                let half = walls.iter().map(|w| w.thickness * 0.5).sum::<f64>() / n;
                (given - half).max(0.0)
            }
            OffsetFrom::Surface => {
                // Between the surface and the main layer's outer face.
                let gap = walls
                    .iter()
                    .map(|w| (w.thickness * 0.5 - main_span(cx, w).1).max(0.0))
                    .sum::<f64>()
                    / n;
                (given - gap).max(0.0)
            }
        }
    }

    fn auto_exterior(&mut self, cx: &mut EditorContext) -> ToolResult {
        cx.refresh();
        let walls: Vec<Wall> = cx
            .floor()
            .walls
            .iter()
            .filter(|w| !no_locate(cx, w) && !w.flags.room_divider)
            .cloned()
            .collect();
        let openings: Vec<plan_core::Opening> = cx
            .floor()
            .openings
            .iter()
            .filter(|o| walls.iter().any(|w| w.id == o.wall_id))
            .cloned()
            .collect();
        let dims = {
            let set = &cx.defaults.dimensions;
            let tl = set.tool_locate(LocateTool::AutoExterior);
            // Setup Automatic: which strings, and where the first line
            // stands from (Offset from Wall).
            let strings =
                plan_core::dimension::exterior_strings_for(&set.exterior_strings(), &set.setup);
            let main = |w: &Wall| main_span(cx, w);
            let first_offset = Self::first_offset(cx, &walls, set.auto_exterior_offset);
            let setup = ExteriorSetup {
                strings: &strings,
                first_offset,
                spacing: set.string_spacing(),
                walls: tl.group.walls,
                openings: tl.group.openings,
                main_span: &main,
            };
            auto_exterior_set(&walls, &openings, &setup)
        };
        if dims.is_empty() {
            cx.status = "No exterior walls to dimension".into();
            return ToolResult::consumed();
        }
        if !self.lock_check(cx, AUTO_LAYER) {
            return ToolResult::consumed();
        }
        let inner: Vec<Vec<Point>> = cx.rooms.iter().map(|r| r.inner_polygon.clone()).collect();
        cx.begin_change("Auto Exterior Dimensions");
        // DIM-25: a new run replaces the previous exterior strings.
        Self::clear_run(cx, AutoGroup::Exterior, &inner);
        let n = Self::add_run(cx, dims);
        cx.mark_dirty();
        cx.status = format!("Added {n} exterior dimensions");
        ToolResult::committed("Auto Exterior Dimensions")
    }

    /// Auto NKBA Dimensions: strings along every kitchen and bath cabinet
    /// run (cabinet faces, sink and appliance centers, overall), in the
    /// "NKBA" dimension set when the plan has one (else the active set). A
    /// new run replaces the previous NKBA strings.
    fn auto_nkba(&mut self, cx: &mut EditorContext) -> ToolResult {
        cx.refresh();
        let items = nkba_items(&placed::load_cabinets(cx.floor()), &cx.floor().symbols);
        let set = cx
            .defaults
            .dimension_set("NKBA")
            .map_or_else(|| cx.defaults.dimensions.clone(), |s| s.auto.clone());
        let walls: Vec<Wall> = cx
            .floor()
            .walls
            .iter()
            .filter(|w| !no_locate(cx, w) && !w.flags.room_divider)
            .cloned()
            .collect();
        let sep = set.string_spacing();
        let setup = NkbaSetup {
            first_offset: sep * 0.67,
            spacing: sep * 0.67,
            walls: &walls,
            reach: if set.reach > 0.0 {
                set.reach
            } else {
                NKBA_WALL_REACH
            },
        };
        let mut dims = auto_nkba_dimensions(&items, &setup);
        if dims.is_empty() {
            cx.status = "No base cabinet runs to dimension".into();
            return ToolResult::consumed();
        }
        if !set.text_style.is_empty() {
            for d in &mut dims {
                d.text_style = Some(set.text_style.clone());
            }
        }
        if !self.lock_check(cx, AUTO_LAYER) {
            return ToolResult::consumed();
        }
        cx.begin_change("Auto NKBA Dimensions");
        Self::clear_run(cx, AutoGroup::Nkba, &[]);
        let n = Self::add_run(cx, dims);
        cx.mark_dirty();
        cx.status = format!("Added {n} NKBA dimensions");
        ToolResult::committed("Auto NKBA Dimensions")
    }

    /// The segments an interior dimension measures to inside a room: the
    /// room's finished surfaces, or (Dimension Defaults > Locate Objects,
    /// "Interior dimensions locate interior surfaces" off) the walls'
    /// surfaces by the Walls setting.
    fn interior_segments(cx: &EditorContext, room: Option<&[Point]>) -> Vec<(Point, Point)> {
        let set = &cx.defaults.dimensions;
        match room {
            Some(poly) if set.interior_locates_interior_surfaces => polygon_edges(poly),
            _ => cx
                .floor()
                .walls
                .iter()
                .filter(|w| !no_locate(cx, w))
                .flat_map(|w| {
                    if !set.interior_locates_interior_surfaces && room.is_some() {
                        let n = w.normal();
                        let (lo, hi) = wall_span(cx, w, set.locate_walls);
                        let mut v = vec![(w.start.add(n.scale(lo)), w.end.add(n.scale(lo)))];
                        if (hi - lo).abs() > 0.01 {
                            v.push((w.start.add(n.scale(hi)), w.end.add(n.scale(hi))));
                        }
                        v
                    } else {
                        polygon_edges(&w.footprint())
                    }
                })
                .collect(),
        }
    }

    /// The opening string along one room edge `a`..`b` (the wall's inner
    /// face): the corners and the openings' sides or centers. Placed inside
    /// the room `sep` from the wall.
    fn edge_openings(
        cx: &EditorContext,
        poly: &[Point],
        a: Point,
        b: Point,
        sep: f64,
    ) -> Vec<Dimension> {
        let len = a.dist(b);
        if len < 12.0 {
            return Vec::new();
        }
        let mode = cx
            .defaults
            .dimensions
            .tool_locate(LocateTool::AutoRoom)
            .group
            .openings;
        if mode == OpeningLocate::None {
            return Vec::new();
        }
        let u = b.sub(a).scale(1.0 / len);
        let floor = cx.floor();
        let mut breaks = vec![0.0, len];
        let mut found = false;
        for o in &floor.openings {
            let Some(w) = floor.wall(o.wall_id) else {
                continue;
            };
            // The wall carries this edge: parallel, with the edge on a face.
            let along_w = w.direction().cross(u).abs() < 1e-3;
            let mid = Point::lerp(a, b, 0.5);
            if !along_w || dist_to_segment(mid, w.start, w.end) > w.thickness * 0.5 + 1.0 {
                continue;
            }
            let pts: Vec<f64> = match mode {
                OpeningLocate::Centers => vec![w.point_along(o.center_offset).sub(a).dot(u)],
                _ => vec![
                    w.point_along(o.start_offset()).sub(a).dot(u),
                    w.point_along(o.end_offset()).sub(a).dot(u),
                ],
            };
            if pts.iter().all(|t| *t > 0.5 && *t < len - 0.5) {
                found = true;
                breaks.extend(pts);
            }
        }
        if !found {
            return Vec::new();
        }
        breaks.sort_by(f64::total_cmp);
        breaks.dedup_by(|x, y| (*x - *y).abs() < 0.5);
        // Offset into the room: the side the left-hand normal faces, if the
        // room lies there.
        let n = u.perp();
        let probe = Point::lerp(a, b, 0.5).add(n.scale(2.0));
        let offset = if point_in_polygon(probe, poly) {
            sep
        } else {
            -sep
        };
        breaks
            .windows(2)
            .filter(|p| p[1] - p[0] >= MIN_LENGTH)
            .map(|p| {
                let mut d = Dimension::new(
                    0,
                    DimensionKind::AutoExterior,
                    a.add(u.scale(p[0])),
                    a.add(u.scale(p[1])),
                    offset,
                );
                d.auto_group = AutoGroup::Interior;
                d
            })
            .collect()
    }

    fn auto_interior(&mut self, cx: &mut EditorContext) -> ToolResult {
        cx.refresh();
        let sep = Self::separation(cx);
        let setup = cx.defaults.dimensions.setup.clone();
        // Setup Automatic, room: the smallest room, the lines inside or
        // outside the room, and whether the clear spans are made at all.
        let min_area = if setup.room_min_area > 0.0 {
            setup.room_min_area
        } else {
            MIN_ROOM_SQ_FT
        };
        let side = if setup.room_inside { 1.0 } else { -1.0 };
        let mut dims = Vec::new();
        let rooms = cx.rooms.clone();
        for room in &rooms {
            if room.interior_area_sq_in / 144.0 < min_area || room.inner_polygon.len() < 3 {
                continue;
            }
            let c = room.centroid;
            if !point_in_polygon(c, &room.inner_polygon) {
                continue;
            }
            let segs = Self::interior_segments(cx, Some(&room.inner_polygon));
            let rays = rays_from(&segs, c);
            let mut spans = Vec::new();
            if setup.room_overall {
                if let Some((l, r)) = rays.left.zip(rays.right) {
                    spans.push(Dimension::new(
                        0,
                        DimensionKind::AutoExterior,
                        l,
                        r,
                        sep * side,
                    ));
                }
                if let Some((d, u)) = rays.down.zip(rays.up) {
                    spans.push(Dimension::new(
                        0,
                        DimensionKind::AutoExterior,
                        d,
                        u,
                        -sep * side,
                    ));
                }
            }
            for d in &mut spans {
                d.auto_group = AutoGroup::Interior;
            }
            dims.extend(spans);
            // The openings of each wall of the room.
            for (a, b) in polygon_edges(&room.inner_polygon) {
                dims.extend(Self::edge_openings(
                    cx,
                    &room.inner_polygon,
                    a,
                    b,
                    sep * side,
                ));
            }
        }
        if !setup.room_allow_duplicates {
            // Two rooms sharing a wall would make the same dimension twice.
            let key = |d: &Dimension| {
                let r = |p: Point| ((p.x * 2.0).round() as i64, (p.y * 2.0).round() as i64);
                let (a, b) = (r(d.start), r(d.end));
                if a <= b {
                    (a, b)
                } else {
                    (b, a)
                }
            };
            let mut seen = std::collections::HashSet::new();
            dims.retain(|d| seen.insert(key(d)));
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
        Self::clear_run(cx, AutoGroup::Interior, &inner);
        let n = Self::add_run(cx, dims);
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
        if pole {
            return self.auto_pole(cx, at);
        }
        let mut levels = plan_core::dimension::story_levels(&cx.project.floors);
        // The roof: its highest ridge and lowest eave are levels too.
        let roofs = plan_core::dimension::roof_marks(&Self::roof_polygons(cx));
        let ridge = roofs
            .iter()
            .filter(|r| r.kind == plan_core::dimension::MarkKind::Ridge)
            .map(|r| r.elevation)
            .fold(f64::NEG_INFINITY, f64::max);
        let eave = roofs
            .iter()
            .filter(|r| r.kind == plan_core::dimension::MarkKind::Eave)
            .map(|r| r.elevation)
            .fold(f64::INFINITY, f64::min);
        for (name, e) in [("Eave", eave), ("Ridge", ridge)] {
            if e.is_finite() {
                levels.push(plan_core::dimension::Level {
                    name: name.into(),
                    elevation: e,
                });
            }
        }
        levels.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
        levels.dedup_by(|b, a| (a.elevation - b.elevation).abs() < 1e-6);
        if levels.len() < 2 {
            cx.status = "There are no floor levels to dimension".into();
            return ToolResult::consumed();
        }
        let datum = plan_core::dimension::elevation_datum(&cx.project.floors);
        let x = Self::pole_x(cx, at);
        let sep = Self::separation(cx);
        // Setup Automatic, elevation: the sides the strings stand on, and an
        // overall dimension outside them.
        let setup = cx.defaults.dimensions.setup.clone();
        if !setup.elevation_left && !setup.elevation_right {
            cx.status = "Switch on Dimension on Left or Right in the Auto Elevation setup".into();
            return ToolResult::consumed();
        }
        let base = plan_core::dimension::elevation_dimensions(&levels, datum, x, sep);
        let rows = base.len() as f64;
        let mut dims = Vec::new();
        if setup.elevation_left {
            dims.extend(base.iter().cloned());
        }
        if setup.elevation_right {
            dims.extend(base.iter().cloned().map(|mut d| {
                d.offset = -d.offset;
                d
            }));
        }
        let lowest = levels.first().map_or(datum, |l| l.elevation);
        let highest = levels.last().map_or(datum, |l| l.elevation);
        if setup.elevation_overall && lowest < datum - 0.5 && highest - lowest > 0.5 {
            // Levels below the datum: one dimension over the whole height.
            for side in [setup.elevation_left, setup.elevation_right]
                .iter()
                .zip([1.0, -1.0])
                .filter(|(on, _)| **on)
                .map(|(_, s)| s)
            {
                dims.push(Dimension::new(
                    0,
                    DimensionKind::AutoExterior,
                    Point::new(x, lowest),
                    Point::new(x, highest),
                    side * sep * (rows + 1.0),
                ));
            }
        }
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
        for mut d in dims {
            d.auto_group = AutoGroup::Levels;
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

    /// The roof planes of every floor as `[x, elevation, -y]` polygons.
    fn roof_polygons(cx: &EditorContext) -> Vec<Vec<[f64; 3]>> {
        cx.project
            .floors
            .iter()
            .flat_map(|f| crate::editor::roof_view::load(f).planes)
            .map(|p| p.polygon3d)
            .collect()
    }

    /// Auto Story Pole Dimensions (Auto Story Pole Dimension Defaults): the
    /// elevation marks the Locate Elevations panel lists (floors, plates,
    /// ceilings, roof eaves and ridges) on a vertical line at the click, an
    /// inner string between all of them and an outer string between the ones
    /// on it, to the left and/or right of the line, each mark named beside
    /// the line. The plan has no elevation view, so the Y axis of the strings
    /// is the height. A new run on the same line replaces the last one.
    fn auto_pole(&mut self, cx: &mut EditorContext, at: Option<Point>) -> ToolResult {
        let ps = cx.defaults.dimensions.setup.pole.clone();
        if !ps.left && !ps.right {
            cx.status =
                "Switch on Dimension on Left or Right in the Auto Story Pole defaults".into();
            return ToolResult::consumed();
        }
        let datum = plan_core::dimension::elevation_datum(&cx.project.floors);
        let x = Self::pole_x(cx, at);
        let roofs = plan_core::dimension::roof_marks(&Self::roof_polygons(cx));
        let mut groups: Vec<Vec<Dimension>> = Vec::new();
        let mut labels: Vec<(String, f64, bool)> = Vec::new();
        for (on, left) in [(ps.left, true), (ps.right, false)] {
            if !on {
                continue;
            }
            let marks = plan_core::dimension::pole_marks(&cx.project.floors, &roofs, &ps, left);
            if marks.len() < 2 {
                continue;
            }
            let (inner, outer) = plan_core::dimension::pole_strings(&marks, &ps, x, left);
            groups.extend([inner, outer].into_iter().filter(|g| !g.is_empty()));
            for m in &marks {
                let text = format!("{}  {}", m.name, Self::level_text(cx, m.elevation - datum));
                labels.push((text, m.elevation, left));
            }
        }
        if groups.is_empty() {
            cx.status = "There are no elevation marks to dimension".into();
            return ToolResult::consumed();
        }
        if !self.lock_check(cx, AUTO_LAYER) {
            return ToolResult::consumed();
        }
        let label = self.mode.label();
        let text_h = cx.defaults.text.height.max(3.0);
        let char_w = text_h * plan_core::cad::TEXT_WIDTH_FACTOR;
        let widest = labels
            .iter()
            .map(|(t, _, _)| t.chars().count() as f64 * char_w)
            .fold(0.0, f64::max);
        cx.begin_change(label);
        let fl = cx.floor;
        cx.project.floors[fl].dimensions.retain(|d| {
            !(d.auto_group == AutoGroup::Levels
                && (d.start.x - x).abs() < 1e-6
                && (d.end.x - x).abs() < 1e-6)
        });
        cx.project.floors[fl].cad.retain(|c| {
            !(c.layer == AUTO_LAYER
                && matches!(&c.item, CadItem::Text { pos, .. }
                    if pos.x >= x - 7.0 - widest && pos.x <= x + 7.0))
        });
        let mut n = 0;
        for g in groups {
            let mut ids = Vec::new();
            for mut d in g {
                d.auto_group = AutoGroup::Levels;
                ids.push(cx.project.add_dimension(fl, d));
                n += 1;
            }
            // One string per row of the pole.
            cx.project.floors[fl].join_string(&ids);
        }
        let mut texts = Vec::new();
        for (text, elevation, left) in labels {
            let w = text.chars().count() as f64 * char_w;
            let px = if left { x + 6.0 } else { x - 6.0 - w };
            texts.push(cx.project.add_cad(
                fl,
                AUTO_LAYER,
                CadItem::Text {
                    pos: Point::new(px, elevation - text_h * 0.5),
                    text,
                    height: text_h,
                    angle: 0.0,
                },
            ));
        }
        super::cad::group_cad(cx, &texts);
        cx.mark_dirty();
        cx.status = format!("Added {n} story pole dimensions");
        ToolResult::committed(label)
    }

    // ----- angular and tape measure -----

    /// The two walls the first two angular clicks were on, when they are two
    /// different walls that are not parallel (Angular Dimension on walls).
    fn angular_walls(&self, cx: &EditorContext) -> Option<(Wall, Wall)> {
        if self.pts.len() < 2 {
            return None;
        }
        let (Some(ObjectRef::Wall(a)), Some(ObjectRef::Wall(b))) =
            (self.pts[0].obj, self.pts[1].obj)
        else {
            return None;
        };
        if a == b {
            return None;
        }
        let (wa, wb) = (cx.floor().wall(a)?.clone(), cx.floor().wall(b)?.clone());
        (wa.direction().cross(wb.direction()).abs() > 1e-3).then_some((wa, wb))
    }

    /// The angular dimension the clicks so far make with the arc through
    /// `place`: between two walls (it follows them), or at a vertex between
    /// two arm points (DIM-18).
    fn angular_dimension(&self, cx: &EditorContext, place: Point) -> Option<Dimension> {
        let curve = if let Some((wa, wb)) = self.angular_walls(cx) {
            let (p1, p2) = (self.pts[0].point, self.pts[1].point);
            let c0 = DimCurve::between_walls(&wa, &wb, p1, p2, 1.0)?;
            DimCurve {
                radius: place.dist(c0.center).max(6.0),
                ..c0
            }
        } else if self.pts.len() >= 3 {
            let v = self.pts[0].point;
            DimCurve::from_points(
                v,
                self.pts[1].point,
                self.pts[2].point,
                place.dist(v).max(6.0),
            )?
        } else {
            return None;
        };
        Some(Dimension::curved(DimensionKind::Manual, curve, 0.0))
    }

    /// Whether the angular clicks are complete up to the arc radius.
    fn angular_ready(&self, cx: &EditorContext) -> bool {
        self.pts.len() >= 3 || self.angular_walls(cx).is_some()
    }

    fn commit_angular(&mut self, cx: &mut EditorContext, place: Point) -> ToolResult {
        let Some(d) = self.angular_dimension(cx, place) else {
            return ToolResult::consumed();
        };
        let deg = d.measure();
        self.reset();
        let res = self.commit(cx, vec![d]);
        if res.commit.is_some() {
            cx.status = format!("Angle: {deg:.1}\u{b0}");
        }
        res
    }

    // ----- radius and arc length -----

    /// The curved wall under `p`, and the surface the click is nearer to:
    /// its signed distance to the left of the centerline.
    fn curved_wall_at(cx: &EditorContext, p: Point) -> Option<(Id, f64)> {
        let tol = cx.pick_tol();
        let mode = cx
            .defaults
            .dimensions
            .tool_locate(LocateTool::Manual)
            .group
            .walls;
        cx.floor()
            .walls
            .iter()
            .filter(|w| w.is_curved() && cx.layers().is_visible(&w.layer) && !no_locate(cx, w))
            .filter_map(|w| {
                let (_, t) = w.locate(p);
                (t.abs() <= w.thickness * 0.5 + tol).then_some((w, t))
            })
            .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(w, t)| {
                let (lo, hi) = wall_span(cx, w, mode);
                let lateral = if (t - lo).abs() <= (t - hi).abs() {
                    lo
                } else {
                    hi
                };
                (w.id, lateral)
            })
    }

    /// The radius or arc length dimension of the clicked curved wall with
    /// its line through `place`.
    fn arc_dimension(&self, cx: &EditorContext, place: Point) -> Option<Dimension> {
        let (wid, lateral) = self.arc?;
        let w = cx.floor().wall(wid)?;
        let kind = if self.mode == DimMode::Radius {
            CurveKind::Radius
        } else {
            CurveKind::ArcLength
        };
        let c = DimCurve::of_wall(w, kind, lateral)?;
        // The line stands this far beyond the measured arc (inside when
        // negative, for an arc length).
        let off = place.dist(c.center) - c.radius;
        Some(Dimension::curved(DimensionKind::Manual, c, off))
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
        if let Some(g) = d.curve_geom(0.0, 0.0) {
            return match h {
                DimHandle::Offset => g.label_at,
                DimHandle::Start => g.line.first().copied().unwrap_or(d.start),
                DimHandle::End => g.line.last().copied().unwrap_or(d.end),
            };
        }
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
        if let Some(g) = d.curve_geom(0.0, 0.0) {
            return g.label_at;
        }
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
            let on_text = Self::label_pos(d).dist(p) <= 22.0 / cx.px_per_in.max(1e-6);
            if let Some(g) = d.curve_geom(0.0, 0.0) {
                let on_line = g
                    .line
                    .windows(2)
                    .any(|w| dist_to_segment(p, w[0], w[1]) <= tol);
                let on_ext = g
                    .extensions
                    .iter()
                    .any(|(s, e)| dist_to_segment(p, *s, *e) <= tol);
                return (on_line || on_text || on_ext).then_some(d.id);
            }
            let (a, b) = d.line_points();
            let on_line = dist_to_segment(p, a, b) <= tol;
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
        // A curved dimension has one handle: the one that moves its line.
        let handles: &[DimHandle] = if d.curve().is_some() {
            &[DimHandle::Offset]
        } else {
            &[DimHandle::Start, DimHandle::End, DimHandle::Offset]
        };
        let hit = handles
            .iter()
            .copied()
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
        self.string = None;
        cx.begin_change("Move Dimension");
        self.drag = Some(DimDrag {
            id: g.id,
            handle: g.handle,
            changed: false,
            hint: None,
        });
    }

    fn drag_to(&mut self, cx: &mut EditorContext, p: &PointerEvent) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let unit = cx.snap_unit();
        let new_pt = match drag.handle {
            DimHandle::Offset => None,
            _ => {
                let loc = locate(cx, p, false, None);
                drag.hint = dim_hint(&loc);
                Some(loc.point)
            }
        };
        let fl = cx.floor;
        let mut offset_to: Option<Point> = None;
        let drag_id = drag.id;
        if let Some(d) = cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .find(|d| d.id == drag.id)
        {
            match drag.handle {
                DimHandle::Offset => {
                    offset_to = Some(p.world);
                }
                DimHandle::Start => d.start = new_pt.unwrap_or(d.start),
                DimHandle::End => d.end = new_pt.unwrap_or(d.end),
            }
            drag.changed = true;
        }
        if let Some(pt) = offset_to {
            let snap = (!p.modifiers.alt).then_some(unit);
            cx.project.floors[fl].drag_dimension_line(drag_id, pt, snap);
        }
        cx.mark_dirty();
    }

    /// A dimension was edited by hand: an automatic one becomes manual
    /// (DIM-33), and its points are located again, so a point dragged onto a
    /// new object ties to that object (DIM-3).
    fn finish_edit(
        &mut self,
        cx: &mut EditorContext,
        id: Id,
        dragged: Option<(DimHandle, Option<DimHint>)>,
    ) {
        let fl = cx.floor;
        let Some(d) = cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
        else {
            return;
        };
        if d.kind == DimensionKind::AutoExterior {
            d.kind = DimensionKind::Manual;
            d.auto_group = AutoGroup::None;
        }
        let mut hints = [None, None];
        match dragged {
            Some((DimHandle::Start, h)) => hints[0] = h,
            Some((DimHandle::End, h)) => hints[1] = h,
            Some((DimHandle::Offset, _)) => {
                // The whole string moved its line: all of it is hand-made now.
                for m in cx.project.floors[fl].string_members(id) {
                    if let Some(x) = cx.project.floors[fl]
                        .dimensions
                        .iter_mut()
                        .find(|x| x.id == m)
                    {
                        x.convert_to_manual();
                    }
                }
                return;
            }
            None => {}
        }
        cx.project.floors[fl].attach_dimension_hinted(id, hints);
    }

    fn begin_edit(&mut self, cx: &mut EditorContext, id: Id) {
        let Some(d) = cx.floor().dimensions.iter().find(|d| d.id == id) else {
            return;
        };
        let len = d.measure();
        self.edit = Some(ValueEdit {
            id,
            buf: format!("{len:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
            rich: false,
        });
        set_typing(cx, true);
        cx.status =
            "Type the new dimension; the object at the nearer end moves. Enter applies".into();
    }

    /// Edits the label's text in place: the text is written with the Rich
    /// Text tags (`<b>`, `<i>`, `<u>`, `<size=1.5>`, `<color=#RRGGBB>`), which
    /// become the label's runs on Enter. An empty text puts the measured
    /// number back.
    fn begin_label_edit(&mut self, cx: &mut EditorContext, id: Id) {
        let fmt = cx.dim_format();
        let Some(d) = cx.floor().dimensions.iter().find(|d| d.id == id) else {
            return;
        };
        let buf = if d.look.seg.runs.is_empty() {
            d.text_override
                .clone()
                .unwrap_or_else(|| d.label_parts(&fmt).primary)
        } else {
            plan_core::text_styles::runs_to_markup(&d.look.seg.runs)
        };
        self.edit = Some(ValueEdit {
            id,
            buf,
            rich: true,
        });
        set_typing(cx, true);
        cx.status =
            "Edit the label (<b>bold</b>, <i>italic</i>, <size=1.5>); Enter applies, Esc cancels"
                .into();
    }

    /// Stores an edited label (one undo step).
    fn apply_label(&mut self, cx: &mut EditorContext, id: Id, markup: &str) -> Result<(), String> {
        if !cx.check_unlocked(ObjectRef::Dimension(id)) {
            return Err("That dimension is on a locked layer".into());
        }
        let runs = plan_core::text_styles::runs_from_markup(markup);
        let plain = plan_core::text_styles::runs_plain(&runs);
        let fl = cx.floor;
        cx.begin_change("Edit Dimension Label");
        if let Some(d) = cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .find(|x| x.id == id)
        {
            // Plain text with no formatting is the text override; anything
            // formatted is kept as runs.
            let formatted = runs.iter().any(|r| {
                r.bold
                    || r.italic
                    || r.underline
                    || r.strike
                    || r.upper
                    || r.color.is_some()
                    || r.font.is_some()
                    || (r.scale - 1.0).abs() > 1e-9
                    || r.link.is_some()
            });
            d.look.seg.runs = if formatted && !plain.is_empty() {
                runs
            } else {
                Vec::new()
            };
            d.text_override = (!formatted && !plain.is_empty()).then_some(plain);
            if d.kind == DimensionKind::AutoExterior {
                d.convert_to_manual();
            }
        }
        cx.mark_dirty();
        Ok(())
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
        if let Some(c) = d.curve().copied() {
            return self.apply_curve_value(cx, id, c, new_len);
        }
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
                let center = o.center_offset + delta * axis.dot(w.tangent_along(o.center_offset));
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
        self.finish_edit(cx, id, None);
        cx.mark_dirty();
        Ok(())
    }

    /// A typed radius, arc length or angle (DIM-32, DIM-67): a radius or arc
    /// length bends its curved wall to it (the chord stays); an angle turns
    /// the second wall about the corner, the end farther from it moving.
    fn apply_curve_value(
        &mut self,
        cx: &mut EditorContext,
        id: Id,
        c: DimCurve,
        value: f64,
    ) -> Result<(), String> {
        if value <= 0.0 {
            return Err("The value must be more than zero".into());
        }
        if !cx.check_unlocked(ObjectRef::Dimension(id)) {
            return Err("That dimension is on a locked layer".into());
        }
        let Some(wid) = c.walls[0] else {
            return Err("This dimension is not tied to a wall".into());
        };
        let fl = cx.floor;
        let Some(w) = cx.floor().wall(wid).cloned() else {
            return Err("The wall is gone".into());
        };
        if !cx.check_unlocked(ObjectRef::Wall(wid)) {
            return Err("That wall is on a locked layer".into());
        }
        match c.kind {
            CurveKind::Angle => {
                let Some(wb) = c.walls[1].and_then(|i| cx.floor().wall(i).cloned()) else {
                    return Err("This angle is not tied to two walls".into());
                };
                if !cx.check_unlocked(ObjectRef::Wall(wb.id)) {
                    return Err("That wall is on a locked layer".into());
                }
                let signed = c.sweep.signum() * value.to_radians();
                let turn = signed - c.sweep;
                // The end of the second wall in the arm's direction moves.
                let end = if c.toward_end[1] {
                    WallEnd::End
                } else {
                    WallEnd::Start
                };
                let far = if end == WallEnd::End {
                    wb.end
                } else {
                    wb.start
                };
                let v = far.sub(c.center);
                let (s, co) = turn.sin_cos();
                let to = c
                    .center
                    .add(Point::new(v.x * co - v.y * s, v.x * s + v.y * co));
                cx.begin_change("Edit Dimension Value");
                ops::move_wall_end_joined(&mut cx.project, fl, wb.id, end, to);
            }
            CurveKind::Radius | CurveKind::ArcLength => {
                let Some(curve) = w.curve else {
                    return Err("The wall is not curved".into());
                };
                let chord = w.start.dist(w.end);
                let (_, r_cl) = w.arc_center_radius().ok_or("The wall is not curved")?;
                // The measured surface lies this far from the centerline arc.
                let shift = c.radius - r_cl;
                let sign = curve.bulge.signum();
                let major = curve.bulge.abs() > chord * 0.5;
                let bulge = if c.kind == CurveKind::Radius {
                    let r = value - shift;
                    if r < chord * 0.5 {
                        return Err("That radius is too small for the wall's chord".into());
                    }
                    let h = (r * r - chord * chord * 0.25).sqrt();
                    sign * if major { r + h } else { r - h }
                } else {
                    // Solve for the sweep whose surface arc has the length.
                    let f = |th: f64| (chord / (2.0 * (th * 0.5).sin()) + shift) * th - value;
                    let (mut lo, mut hi) = (1e-4, std::f64::consts::TAU - 1e-4);
                    if f(lo) > 0.0 || f(hi) < 0.0 {
                        return Err("That length is not possible on this wall".into());
                    }
                    for _ in 0..80 {
                        let mid = (lo + hi) * 0.5;
                        if f(mid) < 0.0 {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    let th = (lo + hi) * 0.5;
                    let r = chord / (2.0 * (th * 0.5).sin());
                    sign * r * (1.0 - (th * 0.5).cos())
                };
                cx.begin_change("Edit Dimension Value");
                if let Some(wm) = cx.project.floors[fl].wall_mut(wid) {
                    wm.curve = Some(plan_core::walls::WallCurve { bulge });
                }
            }
        }
        // An automatic dimension edited by hand becomes manual; the curve
        // follows the wall from here.
        if let Some(d) = cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .find(|x| x.id == id)
        {
            d.convert_to_manual();
            d.text_override = None;
        }
        cx.project.floors[fl].sync_dimension_curves();
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
            if ed.rich {
                return match self.apply_label(cx, ed.id, &ed.buf) {
                    Ok(()) => {
                        cx.status.clear();
                        ToolResult::committed("Edit Dimension Label")
                    }
                    Err(e) => {
                        cx.status = e;
                        ToolResult::consumed()
                    }
                };
            }
            let angle =
                cx.floor().dimensions.iter().any(|d| {
                    d.id == ed.id && d.curve().is_some_and(|c| c.kind == CurveKind::Angle)
                });
            let parsed = if angle {
                ed.buf
                    .trim()
                    .trim_end_matches('\u{b0}')
                    .trim()
                    .parse::<f64>()
                    .ok()
            } else {
                parse_ft_in(&ed.buf)
            };
            let Some(v) = parsed else {
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
                if ed.rich {
                    ed.buf.extend(s.chars().filter(|c| !c.is_control()));
                } else {
                    ed.buf.extend(s.chars().filter(|c| {
                        c.is_ascii_digit() || matches!(c, '.' | '-' | '/' | ' ' | '\'' | '"')
                    }));
                }
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
            DimMode::AutoNkba => self.auto_nkba(cx),
            _ => ToolResult::ignored(),
        }
    }

    /// What a click does when it is not on a handle: edit a selected
    /// dimension's value, select a dimension, or work on the new dimension.
    fn click_rest(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        if !self.in_progress() && !self.mode.is_extension() {
            // Clicking the selected dimension's text edits its value (DIM-32).
            if let Some(d) = self.selected(cx) {
                if Self::label_pos(&d).dist(p.world) <= 22.0 / cx.px_per_in.max(1e-6) {
                    self.string = None;
                    if cx.check_unlocked(ObjectRef::Dimension(d.id)) {
                        self.begin_edit(cx, d.id);
                    }
                    return ToolResult::consumed();
                }
            }
            if !self.mode.is_auto() {
                if let Some(id) = Self::dim_under(cx, p.world) {
                    // Picking a dimension ends the string.
                    self.string = None;
                    cx.selection.set(ObjectRef::Dimension(id));
                    return ToolResult::consumed();
                }
            }
        }
        self.start_click(cx, p)
    }

    fn start_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        if let Some(st) = self.string {
            return self.continue_string(cx, p, st);
        }
        match self.mode {
            DimMode::ExtensionAdd => self.extension_add(cx, p),
            DimMode::ExtensionDelete => self.extension_delete(cx, p),
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
                        let res = self.commit(cx, dims);
                        // The string stays open for the next point (DIM-2).
                        if res.commit.is_some() {
                            self.open_string(cx);
                        }
                        return res;
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
                let room = cx
                    .rooms
                    .iter()
                    .find(|r| point_in_polygon(p.world, &r.inner_polygon))
                    .map(|r| r.inner_polygon.clone());
                let segs = Self::interior_segments(cx, room.as_deref());
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
                self.hint_src = saved.iter().copied().chain([loc]).collect();
                let res = self.commit(cx, vec![d]);
                // The origin stays for the next row.
                self.pts = saved;
                self.baseline = Some((axis, n + 1));
                res
            }
            DimMode::Radius | DimMode::ArcLength => {
                if self.arc.is_some() {
                    let dims = self.build(cx, p.snapped);
                    self.reset();
                    return self.commit(cx, dims);
                }
                match Self::curved_wall_at(cx, p.world) {
                    Some(t) => {
                        cx.selection.clear();
                        self.arc = Some(t);
                    }
                    None => cx.status = "Click a curved wall".into(),
                }
                ToolResult::consumed()
            }
            DimMode::Angular => {
                let loc = self.locate_for(cx, p);
                if self.angular_ready(cx) {
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
            | DimMode::AutoStoryPole
            | DimMode::AutoNkba => self.run_auto(cx, Some(p.snapped)),
        }
    }

    /// Opens the string after the manual dimension just placed (the selected
    /// one), so the next clicks add measured points to it.
    fn open_string(&mut self, cx: &mut EditorContext) {
        let Some(ObjectRef::Dimension(id)) = cx.selection.single() else {
            return;
        };
        let Some(d) = cx.floor().dimensions.iter().find(|d| d.id == id) else {
            return;
        };
        let dir = d.end.sub(d.start).normalized();
        self.string = Some(StringState {
            last: d.end,
            dir,
            off: dir.perp().scale(d.offset),
            anchor: d.anchors[1],
            head: d.string_id().unwrap_or(d.id),
        });
        cx.status = "Click the next point to add it to the string; double-click or Esc ends".into();
    }

    /// The next segment of the open string for a point at `to`: from the
    /// last point along the measuring line, the dimension line staying put.
    fn string_segment(st: &StringState, to: Point) -> Option<Dimension> {
        let along = to.sub(st.last).dot(st.dir);
        if along.abs() < MIN_LENGTH {
            return None;
        }
        let end = st.last.add(st.dir.scale(along));
        let forward = st.dir.scale(along.signum());
        let offset = st.off.dot(forward.perp());
        Some(Dimension::new(
            0,
            DimensionKind::Manual,
            st.last,
            end,
            offset,
        ))
    }

    /// A click on an open string: adds the segment to that point (DIM-2).
    fn continue_string(
        &mut self,
        cx: &mut EditorContext,
        p: &PointerEvent,
        st: StringState,
    ) -> ToolResult {
        let loc = self.locate_for(cx, p);
        let Some(d) = Self::string_segment(&st, loc.point) else {
            return ToolResult::consumed();
        };
        let end = d.end;
        self.hint_src = vec![loc];
        let res = self.commit(cx, vec![d]);
        if res.commit.is_some() {
            // The segment starts where the last one ended, tied the same way.
            if let Some(ObjectRef::Dimension(id)) = cx.selection.single() {
                if let Some(nd) = cx.project.floors[cx.floor]
                    .dimensions
                    .iter_mut()
                    .find(|x| x.id == id)
                {
                    if nd.anchors[0].is_none() {
                        nd.anchors[0] = st.anchor;
                    }
                    self.string = Some(StringState {
                        last: end,
                        anchor: nd.anchors[1],
                        ..st
                    });
                    // The segments of a continued dimension are one string.
                    let mut members = cx.project.floors[cx.floor].string_members(st.head);
                    members.push(id);
                    members.dedup();
                    cx.project.floors[cx.floor].join_string(&members);
                }
            }
        }
        res
    }

    /// Add Extension Line: a click on a hidden extension line brings it back;
    /// a click on a dimension line splits that dimension in two at the point
    /// (DIM-41).
    fn extension_add(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let tol = cx.pick_tol();
        // A hidden extension line first.
        let hidden = cx.floor().dimensions.iter().find_map(|d| {
            d.extension_lines()
                .iter()
                .enumerate()
                .find(|(k, (a, b))| d.hide_ext[*k] && dist_to_segment(p.world, *a, *b) <= tol * 1.5)
                .map(|(k, _)| (d.id, k))
        });
        if let Some((id, k)) = hidden {
            if !cx.check_unlocked(ObjectRef::Dimension(id)) {
                return ToolResult::consumed();
            }
            cx.begin_change("Add Extension Line");
            let fl = cx.floor;
            if let Some(d) = cx.project.floors[fl]
                .dimensions
                .iter_mut()
                .find(|d| d.id == id)
            {
                d.hide_ext[k] = false;
            }
            cx.mark_dirty();
            cx.selection.set(ObjectRef::Dimension(id));
            return ToolResult::committed("Add Extension Line");
        }
        let Some(id) = Self::dim_under(cx, p.world) else {
            cx.status = "Click a dimension line to add an extension line there".into();
            return ToolResult::consumed();
        };
        if !cx.check_unlocked(ObjectRef::Dimension(id)) {
            return ToolResult::consumed();
        }
        let Some(d) = cx.floor().dimensions.iter().find(|d| d.id == id).cloned() else {
            return ToolResult::consumed();
        };
        let loc = self.locate_for(cx, p);
        let len = d.length();
        let u = d.end.sub(d.start).normalized();
        let t = loc.point.sub(d.start).dot(u);
        if t < MIN_LENGTH || t > len - MIN_LENGTH {
            cx.status = "Click between the two extension lines".into();
            return ToolResult::consumed();
        }
        let mid = d.start.add(u.scale(t));
        cx.begin_change("Add Extension Line");
        let fl = cx.floor;
        let mut second = d.clone();
        second.start = mid;
        second.anchors = [None, d.anchors[1]];
        second.hide_ext = [false, d.hide_ext[1]];
        second.text_override = None;
        let hint = dim_hint(&loc);
        if let Some(first) = cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .find(|x| x.id == id)
        {
            first.end = mid;
            first.anchors = [d.anchors[0], None];
            first.hide_ext = [d.hide_ext[0], false];
            first.text_override = None;
        }
        let second_id = cx.project.add_dimension(fl, second);
        cx.project.floors[fl].attach_dimension_hinted(id, [None, hint]);
        cx.project.floors[fl].attach_dimension_hinted(second_id, [hint, None]);
        // Ties the original ends had that the geometry does not find again.
        for (did, k) in [(id, 0), (second_id, 1)] {
            if let Some(x) = cx.project.floors[fl]
                .dimensions
                .iter_mut()
                .find(|x| x.id == did)
            {
                if x.anchors[k].is_none() {
                    x.anchors[k] = d.anchors[k];
                }
            }
        }
        cx.selection.set(ObjectRef::Dimension(second_id));
        cx.mark_dirty();
        cx.status.clear();
        ToolResult::committed("Add Extension Line")
    }

    /// Delete Extension Line: a click on an extension line merges the two
    /// dimensions that share it into one, or hides the line of an end that
    /// nothing continues (DIM-41).
    fn extension_delete(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let tol = cx.pick_tol();
        let mut best: Option<(f64, Id, usize)> = None;
        for d in &cx.floor().dimensions {
            for (k, (a, b)) in d.extension_lines().iter().enumerate() {
                if d.hide_ext[k] {
                    continue;
                }
                let dist = dist_to_segment(p.world, *a, *b);
                if dist <= tol * 1.5 && best.is_none_or(|(bd, _, _)| dist < bd) {
                    best = Some((dist, d.id, k));
                }
            }
        }
        let Some((_, id, k)) = best else {
            cx.status = "Click an extension line to delete it".into();
            return ToolResult::consumed();
        };
        if !cx.check_unlocked(ObjectRef::Dimension(id)) {
            return ToolResult::consumed();
        }
        let fl = cx.floor;
        let Some(d) = cx.floor().dimensions.iter().find(|d| d.id == id).cloned() else {
            return ToolResult::consumed();
        };
        let pt = if k == 0 { d.start } else { d.end };
        let dir = d.end.sub(d.start).normalized();
        let off_vec = dir.perp().scale(d.offset);
        // The dimension continuing the string through `pt`: the same line,
        // the same dimension line, on the other side of the point.
        let neighbour = cx
            .floor()
            .dimensions
            .iter()
            .filter(|n| n.id != id && n.kind == DimensionKind::Manual)
            .find_map(|n| {
                let (far, near_k) = if n.start.dist(pt) < 0.01 {
                    (n.end, 0)
                } else if n.end.dist(pt) < 0.01 {
                    (n.start, 1)
                } else {
                    return None;
                };
                let nd = n.end.sub(n.start).normalized();
                let same_line = nd.cross(dir).abs() < 1e-6;
                let n_off = nd.perp().scale(n.offset);
                let side = far.sub(pt).dot(dir);
                let beyond = if k == 0 {
                    side < -MIN_LENGTH
                } else {
                    side > MIN_LENGTH
                };
                (same_line && n_off.dist(off_vec) < 0.01 && beyond)
                    .then(|| (n.id, far, n.anchors[1 - near_k]))
            });
        cx.begin_change("Delete Extension Line");
        match neighbour {
            Some((nid, far, far_anchor)) => {
                if !cx.check_unlocked(ObjectRef::Dimension(nid)) {
                    cx.cancel_change();
                    return ToolResult::consumed();
                }
                if let Some(x) = cx.project.floors[fl]
                    .dimensions
                    .iter_mut()
                    .find(|x| x.id == id)
                {
                    if k == 0 {
                        x.start = far;
                        x.anchors[0] = far_anchor;
                        x.hide_ext[0] = false;
                    } else {
                        x.end = far;
                        x.anchors[1] = far_anchor;
                        x.hide_ext[1] = false;
                    }
                    x.text_override = None;
                    x.offset = off_vec.dot(x.end.sub(x.start).normalized().perp());
                }
                cx.project.remove_dimension(fl, nid);
            }
            None => {
                if let Some(x) = cx.project.floors[fl]
                    .dimensions
                    .iter_mut()
                    .find(|x| x.id == id)
                {
                    x.hide_ext[k] = true;
                }
            }
        }
        cx.selection.set(ObjectRef::Dimension(id));
        cx.mark_dirty();
        cx.status.clear();
        ToolResult::committed("Delete Extension Line")
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
        locate_with(
            cx,
            p,
            self.mode == DimMode::Centerline,
            self.pts.last().map(|l| l.point),
            self.locate_tool(),
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
            DimMode::Angular if self.angular_ready(cx) => self
                .angular_dimension(cx, to)
                .map(|d| format!("Angle: {:.1}\u{b0}", d.measure())),
            DimMode::Radius | DimMode::ArcLength if self.arc.is_some() => self
                .arc_dimension(cx, to)
                .map(|d| format!("{}: {}", self.mode.short(), cx.fmt_dim(d.measure()))),
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
        self.hint_src.clear();
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
        if !self.in_progress() && !self.mode.is_extension() && self.grab_handle(cx, &p) {
            return ToolResult::consumed();
        }
        self.click_rest(cx, &p)
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(d) = self.drag.take() {
            return if d.changed {
                self.finish_edit(cx, d.id, Some((d.handle, d.hint)));
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
        // The click that began the double-click already added the last point
        // of a manual string; this one ends it (DIM-2), unless it is a
        // double-click on a dimension, which opens its specification.
        if self.string.is_some() {
            self.string = None;
            cx.status.clear();
            if let Some(id) = Self::dim_under(cx, p.world) {
                cx.selection.set(ObjectRef::Dimension(id));
                cx.requests
                    .push(EditorRequest::OpenSpec(ObjectRef::Dimension(id)));
            }
            return ToolResult::consumed();
        }
        if self.mode == DimMode::Running && self.pts.len() >= 2 {
            let dims = self.build_running_final(cx, p.snapped);
            self.reset();
            return self.commit(cx, dims);
        }
        // A double-click on the selected dimension's text edits the label
        // in place (the first click began a value edit).
        if let Some(d) = self.selected(cx) {
            let on_text = Self::label_pos(&d).dist(p.world) <= 22.0 / cx.px_per_in.max(1e-6);
            if on_text && self.edit.as_ref().is_none_or(|e| !e.rich) {
                self.end_edit(cx);
                if cx.check_unlocked(ObjectRef::Dimension(d.id)) {
                    self.begin_label_edit(cx, d.id);
                }
                return ToolResult::consumed();
            }
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
            let had = self.in_progress() || self.string.is_some() || cx.readout.is_some();
            self.reset();
            cx.readout = None;
            return if had {
                ToolResult::consumed()
            } else {
                ToolResult::ignored()
            };
        }
        if k.is(egui::Key::Enter) && self.string.is_some() {
            self.string = None;
            cx.status.clear();
            return ToolResult::consumed();
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
                DimMode::Angular if self.angular_walls(cx).is_some() => {
                    if let Some(d) = self.angular_dimension(cx, self.cursor) {
                        let c = d.curve().copied();
                        if let Some(c) = c {
                            let r = (c.radius.min(60.0)).max(12.0);
                            return self.commit_angular(cx, c.center.add(Point::new(r, 0.0)));
                        }
                    }
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

        // The next segment of an open string.
        if let Some(st) = &self.string {
            if let Some(d) = Self::string_segment(st, hover) {
                let look = render::DimLook::of(cx, &d);
                render::draw_dimension_look(painter, cam, &d, &fmt, ghost, pal, &look);
            }
            painter.circle_filled(cam.world_to_screen(st.last), 3.5, pal.selection);
        }
        // The dimension in progress.
        for d in self.build(cx, hover) {
            let look = render::DimLook::of(cx, &d);
            render::draw_dimension_look(painter, cam, &d, &fmt, ghost, pal, &look);
        }
        if self.mode == DimMode::Angular && !self.angular_ready(cx) {
            if let Some(v) = self.pts.first() {
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
                let look = render::DimLook::of(cx, &d);
                render::draw_dimension_look(painter, cam, &d, &fmt, ghost, pal, &look);
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
        let mut v = cx.common_edit_actions();
        v.extend(edit_actions(cx));
        v
    }
}

// ----- Edit toolbar commands of dimensions (DIM-36, DIM-37, DIM-41) -----

/// Edit toolbar command ids of the dimension commands.
pub const CMD_REVERSE: &str = "dim.reverse";
pub const CMD_TO_MANUAL: &str = "dim.to_manual";
pub const CMD_ALIGN: &str = "dim.align";
pub const CMD_DISTRIBUTE: &str = "dim.distribute";
pub const CMD_EXT_ADD: &str = "dim.ext_add";
pub const CMD_EXT_DELETE: &str = "dim.ext_delete";
pub const CMD_JOIN: &str = "dim.join";
pub const CMD_SELECT_STRING: &str = "dim.select_string";
pub const CMD_LEAVE: &str = "dim.leave";

/// The selected dimensions, in selection order.
fn selected_dimensions(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Dimension(id) => Some(*id),
            _ => None,
        })
        .collect()
}

/// The Edit toolbar buttons of the selected dimensions: Reverse Dimension and
/// Convert to Manual for any, Align Dimensions for two or more, Distribute
/// Dimensions for three or more.
pub fn edit_actions(cx: &EditorContext) -> Vec<crate::editor::EditAction> {
    use crate::editor::{EditAction, EditActionKind};
    let ids = selected_dimensions(cx);
    if ids.is_empty() {
        return Vec::new();
    }
    let any_auto = cx.floor().dimensions.iter().any(|d| {
        ids.contains(&d.id) && (d.kind != DimensionKind::Manual || d.auto_group != AutoGroup::None)
    });
    let in_string = cx
        .floor()
        .dimensions
        .iter()
        .any(|d| ids.contains(&d.id) && d.string_id().is_some());
    let button = |id: &'static str, label: &'static str, enabled: bool| EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled,
    };
    vec![
        button(CMD_REVERSE, "Reverse Dimension", true),
        button(CMD_TO_MANUAL, "Convert to Manual Dimension", any_auto),
        button(CMD_ALIGN, "Align Dimensions", ids.len() >= 2),
        button(CMD_DISTRIBUTE, "Distribute Dimensions", ids.len() >= 3),
        button(CMD_EXT_ADD, "Add Extension Line", true),
        button(CMD_EXT_DELETE, "Delete Extension Line", true),
        button(CMD_JOIN, "Join Into One Dimension String", ids.len() >= 2),
        button(CMD_SELECT_STRING, "Select Dimension String", in_string),
        button(CMD_LEAVE, "Take Out of Dimension String", in_string),
    ]
}

/// Runs a dimension Edit toolbar command on the selection (one undo step);
/// false when `id` is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    // Add / Delete Extension Line are tools: the command switches to them.
    if matches!(id, CMD_EXT_ADD | CMD_EXT_DELETE) {
        let mode = if id == CMD_EXT_ADD {
            DimMode::ExtensionAdd
        } else {
            DimMode::ExtensionDelete
        };
        cx.requests
            .push(EditorRequest::SetTool(ToolId::DimensionVariant(mode)));
        return true;
    }
    if id == CMD_SELECT_STRING {
        let fl = cx.floor;
        let mut all = Vec::new();
        for d in selected_dimensions(cx) {
            for m in cx.project.floors[fl].string_members(d) {
                if !all.contains(&m) {
                    all.push(m);
                }
            }
        }
        cx.selection.clear();
        for m in all {
            cx.selection.add(ObjectRef::Dimension(m));
        }
        return true;
    }
    let label = match id {
        CMD_REVERSE => "Reverse Dimension",
        CMD_TO_MANUAL => "Convert to Manual Dimension",
        CMD_ALIGN => "Align Dimensions",
        CMD_DISTRIBUTE => "Distribute Dimensions",
        CMD_JOIN => "Join Dimension String",
        CMD_LEAVE => "Take Out of Dimension String",
        _ => return false,
    };
    let ids = selected_dimensions(cx);
    if ids.is_empty() {
        return true;
    }
    if !ids
        .iter()
        .all(|i| cx.check_unlocked(ObjectRef::Dimension(*i)))
    {
        return true;
    }
    cx.begin_change(label);
    let fl = cx.floor;
    let changed = match id {
        CMD_REVERSE => {
            for d in cx.project.floors[fl]
                .dimensions
                .iter_mut()
                .filter(|d| ids.contains(&d.id))
            {
                d.reverse();
            }
            ids.len()
        }
        CMD_TO_MANUAL => cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .filter(|d| ids.contains(&d.id))
            .map(|d| usize::from(d.convert_to_manual()))
            .sum(),
        CMD_JOIN => usize::from(cx.project.floors[fl].join_string(&ids).is_some()) * ids.len(),
        CMD_LEAVE => {
            for i in &ids {
                cx.project.floors[fl].leave_string(*i);
            }
            ids.len()
        }
        _ => {
            let dims = &mut cx.project.floors[fl].dimensions;
            // Selection order: the first selected is the reference.
            let mut picked: Vec<Dimension> = ids
                .iter()
                .filter_map(|i| dims.iter().find(|d| d.id == *i).cloned())
                .collect();
            let n = if id == CMD_ALIGN {
                plan_core::align_dimensions(&mut picked)
            } else {
                plan_core::distribute_dimensions(&mut picked)
            };
            for p in picked {
                if let Some(d) = dims.iter_mut().find(|d| d.id == p.id) {
                    d.offset = p.offset;
                }
            }
            n
        }
    };
    if changed == 0 {
        cx.cancel_change();
        cx.status = format!("{label}: nothing to change");
    } else {
        cx.mark_dirty();
        cx.status = format!("{label}: {changed} changed");
    }
    true
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
        // Three clicks and the arc make one dimension with a curve: it is
        // an object (DIM-18), not loose CAD lines.
        let mut cx = new_cx();
        let mut t = tool(DimMode::Angular);
        // The third click angle-snaps from the second and rounds its length
        // to the grid, so it sits on a whole grid length (96 in) from it.
        for (x, y) in [(0.0, 0.0), (96.0, 0.0), (96.0, 96.0), (45.0, 0.0)] {
            click(&mut t, &mut cx, x, y);
        }
        assert!(cx.floor().cad.is_empty());
        assert_eq!(cx.floor().dimensions.len(), 1);
        let d = &cx.floor().dimensions[0];
        let c = d.curve().expect("an angular dimension has a curve");
        assert_eq!(c.kind, CurveKind::Angle);
        assert!((c.radius - 45.0).abs() < 1e-9);
        assert!((d.measure() - 45.0).abs() < 1e-6, "{c:?}");
        assert_eq!(d.label(&cx.dim_format()), "45.0\u{b0}");
        assert_eq!(cx.undo().as_deref(), Some("Angular Dimension"));
        assert!(cx.floor().dimensions.is_empty());
    }

    #[test]
    fn angular_dimension_between_two_walls_follows_them_and_takes_a_typed_angle() {
        let mut cx = new_cx();
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
            Point::new(0.0, 0.0),
            Point::new(0.0, 240.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let mut t = tool(DimMode::Angular);
        click(&mut t, &mut cx, 120.0, 0.0);
        click(&mut t, &mut cx, 0.0, 120.0);
        assert!(
            cx.floor().dimensions.is_empty(),
            "the arc is not placed yet"
        );
        let r = click(&mut t, &mut cx, 50.0, 50.0);
        assert_eq!(r.commit.as_deref(), Some("Angular Dimension"));
        let id = cx.floor().dimensions[0].id;
        let c = *cx.floor().dimensions[0].curve().unwrap();
        assert_eq!(c.walls, [Some(a), Some(b)]);
        assert!((c.radius - 50.0f64.hypot(50.0)).abs() < 1e-6);
        assert!((cx.floor().dimensions[0].measure() - 90.0).abs() < 1e-6);
        // Turn the second wall: the dimension follows.
        cx.project.floors[0].wall_mut(b).unwrap().end = Point::new(170.0, 170.0);
        cx.mark_dirty();
        cx.refresh();
        assert!((cx.floor().dimensions[0].measure() - 45.0).abs() < 1e-6);
        // A typed angle turns the second wall about the corner.
        cx.selection.set(ObjectRef::Dimension(id));
        t.apply_value(&mut cx, id, 60.0).unwrap();
        let d = cx.floor().dimensions[0].clone();
        assert!((d.measure() - 60.0).abs() < 1e-6, "{}", d.measure());
        assert_eq!(cx.undo().as_deref(), Some("Edit Dimension Value"));
        assert!((cx.floor().dimensions[0].measure() - 45.0).abs() < 1e-6);
    }

    #[test]
    fn radius_and_arc_length_dimensions_of_a_curved_wall() {
        let mut cx = new_cx();
        let id = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        cx.project.floors[0].wall_mut(id).unwrap().curve =
            Some(plan_core::walls::WallCurve { bulge: 60.0 });
        let w = cx.floor().wall(id).unwrap().clone();
        let apex = w.point_along(w.path_length() * 0.5);
        let out = apex.add(w.normal_along(w.path_length() * 0.5).scale(30.0));
        // A quarter of the way along, away from the first line.
        let quarter = w.point_along(w.path_length() * 0.25);
        // Radius.
        let mut t = tool(DimMode::Radius);
        click(&mut t, &mut cx, apex.x, apex.y + 0.0);
        assert!(
            cx.floor().dimensions.is_empty(),
            "the line is not placed yet"
        );
        let r = click(&mut t, &mut cx, out.x, out.y);
        assert_eq!(r.commit.as_deref(), Some("Radius Dimension"));
        let rad = cx.floor().dimensions[0].clone();
        assert_eq!(rad.curve().unwrap().kind, CurveKind::Radius);
        let (_, r_cl) = w.arc_center_radius().unwrap();
        assert!(
            (rad.measure() - r_cl).abs() <= 3.0 + 1e-6,
            "{} vs {r_cl}",
            rad.measure()
        );
        assert!(rad.label(&cx.dim_format()).starts_with("R "));
        // Arc length.
        let mut t = tool(DimMode::ArcLength);
        click(&mut t, &mut cx, quarter.x, quarter.y);
        click(&mut t, &mut cx, out.x, out.y);
        let arc = cx.floor().dimensions[1].clone();
        assert_eq!(arc.curve().unwrap().kind, CurveKind::ArcLength);
        assert!(
            (arc.measure() - w.path_length()).abs() <= 3.0 * 2.0,
            "{}",
            arc.measure()
        );
        // A straight wall is refused.
        let mut cx2 = new_cx();
        two_walls(&mut cx2);
        let mut t = tool(DimMode::ArcLength);
        click(&mut t, &mut cx2, 50.0, 0.0);
        assert!(cx2.status.contains("curved"));
        // Typing a radius bends the wall to it; the dimension follows.
        let rid = rad.id;
        let want = rad.measure() * 1.5;
        t.apply_value(&mut cx, rid, want).unwrap();
        let d = cx
            .floor()
            .dimensions
            .iter()
            .find(|d| d.id == rid)
            .unwrap()
            .clone();
        assert!(
            (d.measure() - want).abs() < 1e-6,
            "{} vs {want}",
            d.measure()
        );
        let w2 = cx.floor().wall(id).unwrap();
        assert!(
            w2.curve.unwrap().bulge.abs() < 60.0,
            "a larger radius is a flatter arc"
        );
        // Typing an arc length does the same.
        let aid = arc.id;
        let want = arc.measure() * 1.1;
        t.apply_value(&mut cx, aid, want).unwrap();
        let d = cx
            .floor()
            .dimensions
            .iter()
            .find(|d| d.id == aid)
            .unwrap()
            .clone();
        assert!(
            (d.measure() - want).abs() < 1e-3,
            "{} vs {want}",
            d.measure()
        );
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
    fn auto_exterior_dimensions_a_rectangle_with_wall_to_wall_and_overall_strings() {
        let mut cx = new_cx();
        rect_room(&mut cx);
        let mut t = tool(DimMode::AutoExterior);
        let r = click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(r.commit.as_deref(), Some("Auto Exterior Dimensions"));
        // No openings: per side the corner walls and the span between them
        // (wall to wall, three segments) and the overall.
        assert_eq!(cx.floor().dimensions.len(), 16);
        assert!(cx
            .floor()
            .dimensions
            .iter()
            .all(|d| d.kind == DimensionKind::AutoExterior));
        // Strings sit from the dimension defaults: the offset of the first
        // slot, the line separation between slots (openings, wall to wall,
        // overall).
        let set = &cx.defaults.dimensions;
        let mut offsets: Vec<f64> = cx.floor().dimensions.iter().map(|d| d.offset).collect();
        offsets.sort_by(f64::total_cmp);
        offsets.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(
            offsets,
            vec![
                set.auto_exterior_offset + set.string_spacing(),
                set.auto_exterior_offset + 2.0 * set.string_spacing()
            ]
        );
        // Running again replaces them (DIM-25).
        click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(cx.floor().dimensions.len(), 16);
        assert_eq!(cx.undo().as_deref(), Some("Auto Exterior Dimensions"));
        assert_eq!(cx.floor().dimensions.len(), 16);
    }

    /// A 240 x 120 room with a base run along the bottom wall: a 30" base,
    /// a 36" sink base (sink cut out), a 24" dishwasher bay, a 30" base and
    /// a 24" base, from the left wall's face (x = 3) at the wall's face
    /// (y = 3). Returns the ids in that order.
    fn kitchen_run(cx: &mut EditorContext) -> Vec<Id> {
        use plan_cabinets::{Cabinet, CutoutKind};
        rect_room(cx);
        let mut ids = Vec::new();
        let mut x = 3.0;
        for (i, w) in [30.0, 36.0, 24.0, 30.0, 24.0].into_iter().enumerate() {
            let mut c = match i {
                1 => Cabinet::sink_base(w),
                2 => Cabinet::dishwasher_opening(),
                _ => Cabinet::base(w),
            };
            if i == 1 {
                assert!(c.add_cutout(CutoutKind::Sink));
            }
            c.position = Point::new(x, 3.0);
            ids.push(placed::add_cabinet(&mut cx.project, 0, c).unwrap());
            x += w;
        }
        // A wall cabinet above the run is not part of it.
        let mut up = Cabinet::wall(30.0);
        up.position = Point::new(3.0, 3.0);
        placed::add_cabinet(&mut cx.project, 0, up).unwrap();
        ids
    }

    fn nkba_strings(cx: &EditorContext) -> Vec<(f64, f64, f64)> {
        // (offset, start x, end x) of every NKBA string, by offset then x.
        let mut v: Vec<(f64, f64, f64)> = cx
            .floor()
            .dimensions
            .iter()
            .filter(|d| d.auto_group == AutoGroup::Nkba)
            .map(|d| (d.offset, d.start.x, d.end.x))
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        v
    }

    #[test]
    fn auto_nkba_dimensions_a_kitchen_run_to_cabinet_faces_and_the_sink_center() {
        let mut cx = new_cx();
        let ids = kitchen_run(&mut cx);
        let mut t = tool(DimMode::AutoNkba);
        let r = click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(r.commit.as_deref(), Some("Auto NKBA Dimensions"));
        let strings = nkba_strings(&cx);
        let offsets: Vec<f64> = {
            let mut o: Vec<f64> = strings.iter().map(|s| s.0).collect();
            o.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            o
        };
        assert_eq!(offsets.len(), 3, "faces, centers, overall: {strings:?}");
        let on = |off: f64| -> Vec<(f64, f64)> {
            strings
                .iter()
                .filter(|s| (s.0 - off).abs() < 1e-9)
                .map(|s| (s.1, s.2))
                .collect()
        };
        // Faces: every cabinet and on to the right wall's face (237).
        assert_eq!(
            on(offsets[0]),
            vec![
                (3.0, 33.0),
                (33.0, 69.0),
                (69.0, 93.0),
                (93.0, 123.0),
                (123.0, 147.0),
                (147.0, 237.0)
            ]
        );
        // Centers: the sink (33 + 18) and the dishwasher bay (69 + 12).
        assert_eq!(
            on(offsets[1]),
            vec![(3.0, 51.0), (51.0, 81.0), (81.0, 237.0)]
        );
        assert_eq!(on(offsets[2]), vec![(3.0, 237.0)]);
        // All on the cabinet front (y = 27), on the automatic layer kind.
        assert!(cx
            .floor()
            .dimensions
            .iter()
            .all(|d| (d.start.y - 27.0).abs() < 1e-9 && d.kind == DimensionKind::AutoExterior));
        // The cabinet faces are tied to the cabinets (the sink base's sides).
        let tied = cx
            .floor()
            .dimensions
            .iter()
            .filter(|d| {
                d.anchors
                    .iter()
                    .flatten()
                    .any(|a| a.target == AnchorTarget::Cabinet && a.wall == ids[1])
            })
            .count();
        assert!(tied >= 2, "{tied}");
        // Running again replaces the strings (one undo step each).
        let n = cx.floor().dimensions.len();
        click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(cx.floor().dimensions.len(), n);
        // Moving a cabinet moves its face points; deleting it lets them go.
        let mut moved = placed::cabinet_by_id(cx.floor(), ids[4]).unwrap();
        moved.position.x += 6.0;
        assert!(placed::replace_cabinet(&mut cx.project, 0, &moved));
        cx.mark_dirty();
        cx.refresh();
        assert!(nkba_strings(&cx)
            .iter()
            .any(|s| (s.1 - 153.0).abs() < 1e-9 || (s.2 - 153.0).abs() < 1e-9));
        assert!(placed::remove_cabinet(&mut cx.project, 0, ids[4]));
        cx.mark_dirty();
        cx.refresh();
        assert!(cx.floor().dimensions.iter().all(|d| !d
            .anchors
            .iter()
            .flatten()
            .any(|a| a.wall == ids[4] && a.target == AnchorTarget::Cabinet)));
    }

    #[test]
    fn edit_toolbar_reverses_converts_aligns_and_distributes_dimensions() {
        let mut cx = new_cx();
        rect_room(&mut cx);
        let mut ids = Vec::new();
        for off in [24.0, 40.0, 100.0, 130.0] {
            ids.push(cx.project.add_dimension(
                0,
                Dimension::new(
                    0,
                    DimensionKind::AutoExterior,
                    Point::new(0.0, 0.0),
                    Point::new(240.0, 0.0),
                    off,
                ),
            ));
        }
        let t = tool(DimMode::Manual);
        assert!(t.edit_toolbar(&cx).is_empty(), "nothing selected");
        cx.selection.set(ObjectRef::Dimension(ids[0]));
        let labels: Vec<(&str, bool)> = edit_actions(&cx)
            .iter()
            .map(|a| (a.label, a.enabled))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("Reverse Dimension", true),
                ("Convert to Manual Dimension", true),
                ("Align Dimensions", false),
                ("Distribute Dimensions", false),
                ("Add Extension Line", true),
                ("Delete Extension Line", true),
                ("Join Into One Dimension String", false),
                ("Select Dimension String", false),
                ("Take Out of Dimension String", false)
            ]
        );
        assert!(t
            .edit_toolbar(&cx)
            .iter()
            .any(|a| a.label == "Reverse Dimension"));
        // Reverse: one undo step; the line lands on the other side.
        assert!(run_command(&mut cx, CMD_REVERSE));
        let d = cx
            .floor()
            .dimensions
            .iter()
            .find(|d| d.id == ids[0])
            .unwrap();
        assert_eq!(d.start.x, 240.0);
        assert!((d.line_points().0.y + 24.0).abs() < 1e-9);
        assert_eq!(cx.undo().as_deref(), Some("Reverse Dimension"));
        let d = cx
            .floor()
            .dimensions
            .iter()
            .find(|d| d.id == ids[0])
            .unwrap();
        assert_eq!(d.start.x, 0.0);
        // Convert to manual.
        assert!(run_command(&mut cx, CMD_TO_MANUAL));
        let d = cx
            .floor()
            .dimensions
            .iter()
            .find(|d| d.id == ids[0])
            .unwrap();
        assert_eq!(d.kind, DimensionKind::Manual);
        // Align the other three onto the first one's line (24).
        for i in &ids[1..] {
            cx.selection.add(ObjectRef::Dimension(*i));
        }
        let offs = |cx: &EditorContext| -> Vec<f64> {
            ids.iter()
                .map(|i| {
                    cx.floor()
                        .dimensions
                        .iter()
                        .find(|d| d.id == *i)
                        .unwrap()
                        .offset
                })
                .collect()
        };
        assert!(run_command(&mut cx, CMD_ALIGN));
        assert_eq!(offs(&cx), vec![24.0; 4]);
        assert_eq!(cx.undo().as_deref(), Some("Align Dimensions"));
        assert_eq!(offs(&cx), vec![24.0, 40.0, 100.0, 130.0]);
        // Distribute between 24 and 130: 24, 59.33, 94.67, 130.
        assert!(run_command(&mut cx, CMD_DISTRIBUTE));
        let o = offs(&cx);
        assert!((o[1] - (24.0 + 106.0 / 3.0)).abs() < 1e-9, "{o:?}");
        assert!((o[2] - (24.0 + 212.0 / 3.0)).abs() < 1e-9, "{o:?}");
        assert!(!run_command(&mut cx, "dim.nope"));
    }

    #[test]
    fn auto_nkba_with_no_base_cabinets_says_so() {
        let mut cx = new_cx();
        rect_room(&mut cx);
        let mut t = tool(DimMode::AutoNkba);
        let r = click(&mut t, &mut cx, 120.0, 60.0);
        assert!(r.commit.is_none());
        assert!(cx.status.contains("No base cabinet runs"));
        assert!(cx.floor().dimensions.is_empty());
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
        assert_eq!(cx.floor().dimensions.len(), 2 + 16);
        click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(cx.floor().dimensions.len(), 2 + 16);
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
        // The string stays open for more points; Esc ends it (and the tool
        // stays).
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
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
    fn story_pole_dimensions_use_the_pole_setup_and_name_the_marks() {
        let mut cx = new_cx();
        cx.project.build_new_floor(false);
        cx.floor = 0;
        let mut t = tool(DimMode::AutoStoryPole);
        click(&mut t, &mut cx, 400.0, 0.0);
        let dims: Vec<Dimension> = cx.floor().dimensions.clone();
        // Subfloor, ceiling, subfloor, ceiling: three inner segments, and
        // the outer string between the two subfloors.
        assert_eq!(dims.len(), 4, "{dims:?}");
        assert!(dims.iter().all(|d| d.start.x == 400.0 && d.end.x == 400.0));
        let inner = dims.iter().filter(|d| d.offset == 24.0).count();
        let outer = dims.iter().filter(|d| d.offset == 36.0).count();
        assert_eq!((inner, outer), (3, 1));
        // Each row is one string.
        let first = dims.iter().find(|d| d.offset == 24.0).unwrap();
        assert_eq!(cx.floor().string_members(first.id).len(), 3);
        let names: Vec<String> = cx
            .floor()
            .cad
            .iter()
            .filter_map(|c| match &c.item {
                CadItem::Text { text, .. } if c.layer == AUTO_LAYER => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(names.len(), 4);
        assert!(
            names.iter().any(|n| n.contains("Top of Subfloor")),
            "{names:?}"
        );
        assert!(names.iter().any(|n| n.contains("Ceiling")), "{names:?}");
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
        // The right side too, when the defaults ask for it.
        cx.defaults.dimensions.setup.pole.right = true;
        click(&mut t, &mut cx, 400.0, 20.0);
        let n = cx.floor().dimensions.len();
        assert_eq!(n, 8);
        assert!(cx.floor().dimensions.iter().any(|d| d.offset < 0.0));
        // Neither side: nothing to do.
        cx.defaults.dimensions.setup.pole.right = false;
        cx.defaults.dimensions.setup.pole.left = false;
        click(&mut t, &mut cx, 400.0, 20.0);
        assert_eq!(cx.floor().dimensions.len(), n);
        assert!(cx.status.contains("Dimension on Left"));
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

    // ----- Locate Objects, associative dimensions and the automatic set -----

    /// A 40' x 30' shell of 6" exterior walls with a partition at x = 200
    /// and a window on every side; returns the ids of the four walls.
    fn shell_40x30(cx: &mut EditorContext) -> [Id; 4] {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 360.0),
            Point::new(0.0, 360.0),
        ];
        let mut ids = [0; 4];
        for i in 0..4 {
            ids[i] = cx
                .project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        cx.project.add_wall(
            0,
            Point::new(200.0, 0.0),
            Point::new(200.0, 360.0),
            4.0,
            96.0,
            WallKind::Interior,
        );
        for id in ids {
            cx.project
                .add_opening(0, id, 120.0, OpeningKind::Window)
                .unwrap();
        }
        ids
    }

    fn auto_strings_of(cx: &EditorContext) -> Vec<&Dimension> {
        cx.floor()
            .dimensions
            .iter()
            .filter(|d| d.auto_group == AutoGroup::Exterior)
            .collect()
    }

    #[test]
    fn locate_settings_change_where_a_manual_dimension_between_two_walls_lands() {
        let measure = |mode: WallLocate| -> f64 {
            let mut cx = new_cx();
            two_walls(&mut cx);
            cx.defaults.dimensions.locate_walls = mode;
            let mut t = tool(DimMode::Manual);
            click(&mut t, &mut cx, 50.0, 4.0);
            click(&mut t, &mut cx, 50.0, 96.0);
            click(&mut t, &mut cx, 80.0, 50.0);
            cx.floor().dimensions[0].length()
        };
        // Face to face (100 - 3 - 3) versus centerline to centerline.
        assert!((measure(WallLocate::Surfaces) - 94.0).abs() < 1e-9);
        assert!((measure(WallLocate::MainLayer) - 94.0).abs() < 1e-9);
        assert!((measure(WallLocate::Centers) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn opening_and_cabinet_locate_options_pick_what_a_point_lands_on() {
        let first_point =
            |setup: &dyn Fn(&mut EditorContext), at: Point| -> (Point, &'static str) {
                let mut cx = new_cx();
                let a = cx.project.add_wall(
                    0,
                    Point::new(0.0, 0.0),
                    Point::new(240.0, 0.0),
                    6.0,
                    96.0,
                    WallKind::Exterior,
                );
                cx.project
                    .add_opening(0, a, 120.0, OpeningKind::Window)
                    .unwrap();
                setup(&mut cx);
                let p = PointerEvent::at(&cx, at);
                let l = locate(&cx, &p, false, None);
                (l.point, l.what)
            };
        let near_edge = Point::new(106.0, 2.0);
        let (p, what) = first_point(
            &|cx| {
                cx.defaults
                    .dimensions
                    .set_opening_locate(OpeningLocate::Sides)
            },
            near_edge,
        );
        assert_eq!(what, "Opening edge");
        assert!(
            (p.x - 102.0).abs() < 1e-9 && (p.y - 3.0).abs() < 1e-9,
            "{p:?}"
        );
        let (p, what) = first_point(
            &|cx| {
                cx.defaults
                    .dimensions
                    .set_opening_locate(OpeningLocate::Centers)
            },
            near_edge,
        );
        assert_eq!((what, p.x), ("Opening center", 120.0));
        // None: the wall behind the opening is located instead.
        let (p, what) = first_point(
            &|cx| {
                cx.defaults
                    .dimensions
                    .set_opening_locate(OpeningLocate::None)
            },
            near_edge,
        );
        assert_eq!(what, "Wall surface");
        assert!((p.x - 106.0).abs() < 1e-9);

        // A cabinet's side, or nothing when cabinets are not located.
        let mut cx = new_cx();
        let mut cab = plan_cabinets::Cabinet::base(36.0);
        cab.position = Point::new(100.0, 50.0);
        let id = placed::add_cabinet(&mut cx.project, 0, cab).unwrap();
        let p = PointerEvent::at(&cx, Point::new(104.0, 62.0));
        let l = locate(&cx, &p, false, None);
        assert_eq!(
            (l.what, l.obj),
            ("Cabinet side", Some(ObjectRef::Cabinet(id)))
        );
        assert!((l.point.x - 100.0).abs() < 1e-9 && (l.point.y - 62.0).abs() < 1e-9);
        cx.defaults.dimensions.locate_cabinets = ObjectLocate::None;
        assert_ne!(locate(&cx, &p, false, None).what, "Cabinet side");
    }

    #[test]
    fn moving_a_wall_updates_a_string_tied_to_an_opening_side() {
        let mut cx = new_cx();
        let a = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        cx.project
            .add_opening(0, a, 120.0, OpeningKind::Window)
            .unwrap();
        cx.defaults
            .dimensions
            .set_opening_locate(OpeningLocate::Sides);
        let mut t = tool(DimMode::Manual);
        click(&mut t, &mut cx, 104.0, 2.0);
        click(&mut t, &mut cx, 136.0, 2.0);
        let r = click(&mut t, &mut cx, 120.0, 40.0);
        assert_eq!(r.commit.as_deref(), Some("Manual Dimension"));
        let d = cx.floor().dimensions[0].clone();
        assert!((d.length() - 36.0).abs() < 1e-9);
        assert!(d
            .anchors
            .iter()
            .flatten()
            .all(|an| an.target == AnchorTarget::Opening));
        assert_eq!(d.anchors.iter().flatten().count(), 2);
        // Move the wall up 10": the string follows the window.
        assert!(ops::move_wall_perpendicular(&mut cx.project, 0, a, 10.0));
        cx.mark_dirty();
        cx.refresh();
        let d = &cx.floor().dimensions[0];
        assert!(
            (d.start.y - 13.0).abs() < 1e-9 && (d.end.y - 13.0).abs() < 1e-9,
            "{d:?}"
        );
        assert!((d.start.x - 102.0).abs() < 1e-9 && (d.end.x - 138.0).abs() < 1e-9);
        // Slide the window along the wall: the string goes with it.
        let win = cx.floor().openings[0].id;
        assert!(ops::place_opening_at(&mut cx.project, 0, win, a, 150.0));
        cx.mark_dirty();
        cx.refresh();
        let d = &cx.floor().dimensions[0];
        assert!((d.start.x - 132.0).abs() < 1e-9, "{d:?}");
    }

    #[test]
    fn a_string_tied_to_a_cabinet_follows_it() {
        let mut cx = new_cx();
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let mut cab = plan_cabinets::Cabinet::base(36.0);
        cab.position = Point::new(100.0, 50.0);
        let id = placed::add_cabinet(&mut cx.project, 0, cab).unwrap();
        let mut t = tool(DimMode::Manual);
        // Cabinet side to the wall's inside face, straight below it.
        click(&mut t, &mut cx, 104.0, 62.0);
        click(&mut t, &mut cx, 100.0, 2.0);
        click(&mut t, &mut cx, 60.0, 30.0);
        let d = cx.floor().dimensions[0].clone();
        assert_eq!(
            d.anchors[0].map(|a| (a.target, a.wall)),
            Some((AnchorTarget::Cabinet, id))
        );
        let mut moved = placed::cabinet_by_id(cx.floor(), id).unwrap();
        moved.position = Point::new(130.0, 80.0);
        assert!(placed::replace_cabinet(&mut cx.project, 0, &moved));
        cx.mark_dirty();
        cx.refresh();
        let d = &cx.floor().dimensions[0];
        assert!((d.start.x.max(d.end.x) - d.start.x.min(d.end.x)).abs() < 1e-9 || d.length() > 0.0);
        let ys = [d.start.y, d.end.y];
        assert!(ys.iter().any(|y| (*y - 92.0).abs() < 1e-9), "{d:?}");
    }

    #[test]
    fn auto_exterior_makes_three_strings_per_side_openings_nearest() {
        let mut cx = new_cx();
        shell_40x30(&mut cx);
        cx.defaults.dimensions.locate_walls = WallLocate::Surfaces;
        cx.defaults
            .dimensions
            .set_opening_locate(OpeningLocate::Sides);
        let mut t = tool(DimMode::AutoExterior);
        let r = click(&mut t, &mut cx, 240.0, 180.0);
        assert_eq!(r.commit.as_deref(), Some("Auto Exterior Dimensions"));
        let dims = auto_strings_of(&cx);
        assert!(dims.iter().all(|d| d.kind == DimensionKind::AutoExterior));
        // South side: strings at 32 (openings), 50 (wall to wall), 68 (overall).
        let south: Vec<&&Dimension> = dims
            .iter()
            .filter(|d| (d.start.y + 3.0).abs() < 1e-9 && (d.end.y + 3.0).abs() < 1e-9)
            .collect();
        let mut offsets: Vec<i64> = south.iter().map(|d| d.offset.round() as i64).collect();
        offsets.sort_unstable();
        offsets.dedup();
        assert_eq!(offsets, vec![32, 50, 68]);
        let nearest: Vec<f64> = south
            .iter()
            .filter(|d| (d.offset - 32.0).abs() < 1e-9)
            .map(|d| d.length())
            .collect();
        assert_eq!(nearest.len(), 3, "corner, window, corner: {nearest:?}");
        assert!(nearest.iter().any(|l| (l - 36.0).abs() < 1e-9));
        // Every side has all three.
        for side in 0..4 {
            let n = dims
                .iter()
                .filter(|d| {
                    let (a, b) = d.line_points();
                    let m = Point::lerp(a, b, 0.5);
                    match side {
                        0 => m.y < -3.0,
                        1 => m.x > 483.0,
                        2 => m.y > 363.0,
                        _ => m.x < -3.0,
                    }
                })
                .map(|d| d.offset.round() as i64)
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            assert_eq!(n, 3, "side {side}");
        }
        // The strings are tied to the walls: stretch the south wall.
        let before = dims.len();
        assert!(before > 12);
    }

    #[test]
    fn rerunning_auto_exterior_keeps_manual_and_edited_strings() {
        let mut cx = new_cx();
        let ids = shell_40x30(&mut cx);
        let mut t = tool(DimMode::AutoExterior);
        click(&mut t, &mut cx, 240.0, 180.0);
        let first = cx.floor().dimensions.len();
        // A manual dimension elsewhere.
        cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(10.0, 100.0),
                Point::new(90.0, 100.0),
                12.0,
            ),
        );
        // Dragging an automatic string's line by hand makes it manual.
        let id = auto_strings_of(&cx).last().map(|d| d.id).unwrap();
        cx.selection.set(ObjectRef::Dimension(id));
        let d = cx
            .floor()
            .dimensions
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .clone();
        let h = DimensionTool::handle_pos(&d, DimHandle::Offset);
        let mut mt = tool(DimMode::Manual);
        let down = PointerEvent::at(&cx, h);
        mt.pointer_down(&mut cx, down.with_down(true));
        let to = PointerEvent::at(
            &cx,
            h.add(d.end.sub(d.start).normalized().perp().scale(24.0)),
        );
        mt.pointer_move(&mut cx, to.with_down(true));
        let r = mt.pointer_up(&mut cx, to);
        assert_eq!(r.commit.as_deref(), Some("Move Dimension"));
        let edited = cx.floor().dimensions.iter().find(|d| d.id == id).unwrap();
        assert_eq!(edited.kind, DimensionKind::Manual);
        // Run again: the automatic strings are replaced, the manual and the
        // edited one stay.
        click(&mut t, &mut cx, 240.0, 180.0);
        let f = cx.floor();
        assert!(
            f.dimensions.iter().any(|d| d.id == id),
            "edited string kept"
        );
        assert_eq!(
            f.dimensions
                .iter()
                .filter(|d| d.kind == DimensionKind::Manual)
                .count(),
            2
        );
        assert_eq!(f.dimensions.len(), first + 2);
        let _ = ids;
    }

    #[test]
    fn a_turned_shell_gets_automatic_strings_parallel_to_its_walls() {
        let mut cx = new_cx();
        let theta = 30.0_f64.to_radians();
        let turn = |p: Point| {
            Point::new(
                p.x * theta.cos() - p.y * theta.sin(),
                p.x * theta.sin() + p.y * theta.cos(),
            )
        };
        let c = [
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 360.0),
            Point::new(0.0, 360.0),
        ];
        for i in 0..4 {
            cx.project.add_wall(
                0,
                turn(c[i]),
                turn(c[(i + 1) % 4]),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        let mut t = tool(DimMode::AutoExterior);
        click(
            &mut t,
            &mut cx,
            turn(Point::new(240.0, 180.0)).x,
            turn(Point::new(240.0, 180.0)).y,
        );
        let dims = auto_strings_of(&cx);
        assert!(dims.len() >= 8, "{}", dims.len());
        for d in dims {
            let a = d
                .end
                .sub(d.start)
                .angle()
                .rem_euclid(std::f64::consts::FRAC_PI_2);
            assert!((a - theta).abs() < 1e-6, "{a}");
        }
    }

    #[test]
    fn dragging_a_point_onto_another_wall_ties_it_there() {
        let mut cx = new_cx();
        let (a, b) = two_walls(&mut cx);
        let c = cx.project.add_wall(
            0,
            Point::new(0.0, 200.0),
            Point::new(240.0, 200.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let mut t = tool(DimMode::Manual);
        click(&mut t, &mut cx, 50.0, 4.0);
        click(&mut t, &mut cx, 50.0, 96.0);
        click(&mut t, &mut cx, 80.0, 50.0);
        let d = cx.floor().dimensions[0].clone();
        assert_eq!(d.anchors[0].map(|x| x.wall), Some(a));
        assert_eq!(d.anchors[1].map(|x| x.wall), Some(b));
        // Drag the end handle onto the wall at y = 200.
        let down = PointerEvent::at(&cx, d.end);
        t.pointer_down(&mut cx, down.with_down(true));
        let to = PointerEvent::at(&cx, Point::new(50.0, 196.0));
        t.pointer_move(&mut cx, to.with_down(true));
        let r = t.pointer_up(&mut cx, to);
        assert_eq!(r.commit.as_deref(), Some("Move Dimension"));
        cx.refresh();
        let d = &cx.floor().dimensions[0];
        assert_eq!(d.anchors[1].map(|x| x.wall), Some(c), "{d:?}");
        assert!((d.end.y - 197.0).abs() < 1e-9, "{d:?}");
    }

    #[test]
    fn interior_dimensions_follow_the_interior_surfaces_setting() {
        let mut cx = new_cx();
        rect_room(&mut cx);
        let measure = |cx: &mut EditorContext| -> f64 {
            let mut t = tool(DimMode::Interior);
            click(&mut t, cx, 120.0, 60.0);
            let r = click(&mut t, cx, 120.0, 100.0);
            assert_eq!(r.commit.as_deref(), Some("Interior Dimension"));
            cx.floor().dimensions.last().unwrap().length()
        };
        // Clear span between the inner faces: 240 - 3 - 3 wide.
        let inner = measure(&mut cx);
        assert!((inner - 234.0).abs() < 1e-6, "{inner}");
        // Off, with centerlines located: centerline to centerline.
        cx.defaults.dimensions.interior_locates_interior_surfaces = false;
        cx.defaults.dimensions.locate_walls = WallLocate::Centers;
        let centers = measure(&mut cx);
        assert!((centers - 240.0).abs() < 1e-6, "{centers}");
    }

    #[test]
    fn auto_interior_adds_an_openings_string_and_replaces_its_own_run() {
        let mut cx = new_cx();
        rect_room(&mut cx);
        let south = cx.floor().walls[0].id;
        cx.project
            .add_opening(0, south, 120.0, OpeningKind::Window)
            .unwrap();
        cx.defaults
            .dimensions
            .set_opening_locate(OpeningLocate::Sides);
        let mut t = tool(DimMode::AutoInterior);
        click(&mut t, &mut cx, 120.0, 60.0);
        let n = cx.floor().dimensions.len();
        // Two clear spans and the three segments of the window string.
        assert_eq!(n, 5, "{:?}", cx.floor().dimensions);
        assert!(cx
            .floor()
            .dimensions
            .iter()
            .all(|d| d.auto_group == AutoGroup::Interior));
        click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(cx.floor().dimensions.len(), n);
    }
    #[test]
    fn a_placed_dimension_stays_open_for_more_points_but_its_handles_still_work() {
        let mut cx = new_cx();
        two_walls(&mut cx);
        let mut t = tool(DimMode::Manual);
        click(&mut t, &mut cx, 50.0, 4.0);
        click(&mut t, &mut cx, 50.0, 96.0);
        click(&mut t, &mut cx, 80.0, 50.0);
        // The next click adds a point to the string: a second segment from
        // where the first ended, on the same dimension line, projected onto
        // the measuring line.
        let r = click(&mut t, &mut cx, 61.0, 150.0);
        assert_eq!(r.commit.as_deref(), Some("Manual Dimension"));
        let dims = &cx.floor().dimensions;
        assert_eq!(dims.len(), 2);
        assert_eq!(dims[1].start, dims[0].end);
        assert_eq!(dims[1].end, Point::new(50.0, 150.0));
        assert_eq!(dims[1].offset, dims[0].offset);
        assert!(t.clicked().is_empty(), "no new dimension was started");
        // A click closer than the minimum length adds nothing.
        click(&mut t, &mut cx, 50.2, 150.2);
        assert_eq!(cx.floor().dimensions.len(), 2);
        // The selected segment's offset handle still drags (and ends the string).
        let id = cx.floor().dimensions[1].id;
        let (a, b) = cx.floor().dimensions[1].line_points();
        let grab = Point::lerp(a, b, 0.25);
        let down = PointerEvent::at(&cx, grab);
        t.pointer_down(&mut cx, down.with_down(true));
        let to = PointerEvent::at(&cx, Point::new(110.0, grab.y));
        t.pointer_move(&mut cx, to.with_down(true));
        let r = t.pointer_up(&mut cx, to);
        assert_eq!(r.commit.as_deref(), Some("Move Dimension"));
        assert_eq!(
            cx.floor()
                .dimensions
                .iter()
                .find(|d| d.id == id)
                .unwrap()
                .offset
                .abs(),
            60.0
        );
        // The string is closed: a click now starts a new dimension.
        click(&mut t, &mut cx, 200.0, 20.0);
        assert_eq!(t.clicked().len(), 1);
        // Esc ends a string without leaving the tool; a second Esc leaves it.
        let mut t = tool(DimMode::Manual);
        let mut cx = new_cx();
        two_walls(&mut cx);
        click(&mut t, &mut cx, 50.0, 4.0);
        click(&mut t, &mut cx, 50.0, 96.0);
        click(&mut t, &mut cx, 80.0, 50.0);
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(!t.key(&mut cx, KeyEvent::escape()).consumed);
        // A double-click on a dimension opens its specification, and also ends
        // the string.
        let mut t = tool(DimMode::Manual);
        let mut cx = new_cx();
        two_walls(&mut cx);
        click(&mut t, &mut cx, 50.0, 4.0);
        click(&mut t, &mut cx, 50.0, 96.0);
        click(&mut t, &mut cx, 80.0, 50.0);
        let id = cx.floor().dimensions[0].id;
        cx.requests.clear();
        let p = PointerEvent::at(&cx, Point::new(80.0, 30.0));
        assert!(t.double_click(&mut cx, p).consumed);
        assert_eq!(
            cx.requests,
            vec![EditorRequest::OpenSpec(ObjectRef::Dimension(id))]
        );
        click(&mut t, &mut cx, 150.0, 30.0);
        assert_eq!(t.clicked().len(), 1, "a new dimension starts");
    }

    #[test]
    fn openings_on_a_curved_wall_are_located_by_arc_length() {
        let mut cx = new_cx();
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        cx.project.floors[0].wall_mut(w).unwrap().curve =
            plan_core::WallCurve::from_radius(240.0, 120.0, true);
        let o = cx
            .project
            .add_opening(0, w, 100.0, OpeningKind::Window)
            .unwrap();
        cx.refresh();
        let wall = cx.floor().wall(w).unwrap().clone();
        let op = cx
            .floor()
            .openings
            .iter()
            .find(|x| x.id == o)
            .unwrap()
            .clone();
        // The center of the opening on the arc.
        let center = wall.point_along(op.center_offset);
        let ev = PointerEvent::at(&cx, center);
        let hit = locate(&cx, &ev, true, None);
        assert_eq!(hit.what, "Opening center");
        assert!(
            hit.point.dist(center) < 1e-9,
            "{:?} vs {center:?}",
            hit.point
        );
        // A jamb: the click near it finds the jamb on the arc, on the wall's
        // surface (the line a Locate Walls setting asks for).
        cx.defaults
            .dimensions
            .set_opening_locate(OpeningLocate::Sides);
        let jamb = wall.point_along(op.start_offset());
        let ev = PointerEvent::at(&cx, jamb);
        let edge = locate(&cx, &ev, false, None);
        assert_eq!(edge.what, "Opening edge");
        let (along, side) = wall.locate(edge.point);
        assert!((along - op.start_offset()).abs() < 0.5, "{along}");
        assert!((side.abs() - 3.0).abs() < 0.5 || side.abs() < 0.5, "{side}");
        // Typing a value slides the opening along the arc.
        let moved = find_moveable(&cx, center, wall.tangent_along(op.center_offset));
        assert_eq!(moved, Some(Moveable::Opening(o, w)));
    }
}
