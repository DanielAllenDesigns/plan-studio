//! The snap engine (S-68..S-74, W-11).
//!
//! Priority when several apply within the snap distance (S-69): Endpoint,
//! Intersection, Midpoint, Center, Quadrant, Perpendicular, Tangent, On
//! Object, then Angle (from the pending start) and finally Grid. Alt
//! suspends every snap (S-74). The switches live in [`SnapSettings`] (Edit >
//! Snap Settings, mirrored in `PlanDefaults::editing`).

use super::selection::ObjectRef;
use plan_core::geometry::{dist_to_segment, project_on_segment, segment_intersection, Point};
use plan_core::{CadItem, EditingDefaults, Floor, Id, LayerSet};
use std::f64::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapKind {
    Endpoint,
    Intersection,
    Midpoint,
    /// Foot of the perpendicular from the pending start onto a wall.
    Perpendicular,
    /// Anywhere on a wall centerline.
    OnObject,
    /// Center of a CAD circle or arc.
    Center,
    /// North, south, east or west point of a CAD circle or arc.
    Quadrant,
    /// Point of a CAD circle or arc where a line from the pending start
    /// touches it.
    Tangent,
    /// A point on the extension of a wall or CAD line beyond its end.
    Extension,
    /// A CAD point or marker (Place Point, Point Marker, a numbered marker).
    Marker,
    Angle,
    Grid,
    /// Nothing applied; the raw point.
    Free,
}

impl SnapKind {
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::Endpoint => "Endpoint",
            SnapKind::Intersection => "Intersection",
            SnapKind::Midpoint => "Midpoint",
            SnapKind::Perpendicular => "Perpendicular",
            SnapKind::OnObject => "On Object",
            SnapKind::Center => "Center",
            SnapKind::Quadrant => "Quadrant",
            SnapKind::Tangent => "Tangent",
            SnapKind::Extension => "Extension",
            SnapKind::Marker => "Points/Markers",
            SnapKind::Angle => "Angle",
            SnapKind::Grid => "Grid",
            SnapKind::Free => "Free",
        }
    }

    /// Object snaps (the ones that attach to existing geometry).
    pub fn is_object_snap(self) -> bool {
        matches!(
            self,
            SnapKind::Endpoint
                | SnapKind::Intersection
                | SnapKind::Midpoint
                | SnapKind::Perpendicular
                | SnapKind::OnObject
                | SnapKind::Center
                | SnapKind::Quadrant
                | SnapKind::Tangent
                | SnapKind::Extension
                | SnapKind::Marker
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapResult {
    pub point: Point,
    pub kind: SnapKind,
    pub source: Option<ObjectRef>,
}

impl SnapResult {
    pub fn free(point: Point) -> Self {
        Self {
            point,
            kind: SnapKind::Free,
            source: None,
        }
    }
}

/// Which snaps are on (Edit > Snap Settings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapSettings {
    /// Master switch for the object snaps.
    pub object_snaps: bool,
    pub endpoint: bool,
    pub midpoint: bool,
    pub intersection: bool,
    pub perpendicular: bool,
    pub on_object: bool,
    pub center: bool,
    pub quadrant: bool,
    pub tangent: bool,
    /// Points on the extension of a wall or CAD line beyond its ends.
    pub extension: bool,
    /// CAD points and markers.
    pub markers: bool,
    pub angle: bool,
    pub grid: bool,
    /// Screen-space snap distance.
    pub tolerance_px: f64,
}

impl SnapSettings {
    /// The switches stored in the plan defaults (Snap Settings dialog).
    pub fn from_editing(e: &EditingDefaults) -> Self {
        Self {
            object_snaps: e.object_snaps,
            endpoint: e.snap_endpoint,
            midpoint: e.snap_midpoint,
            intersection: e.snap_intersection,
            perpendicular: e.snap_perpendicular,
            on_object: e.snap_on_object,
            center: e.snap_center,
            quadrant: e.snap_quadrant,
            tangent: e.snap_tangent,
            extension: e.snap_extension,
            markers: e.snap_markers,
            angle: e.angle_snaps,
            grid: e.grid_snaps,
            tolerance_px: if e.snap_distance_px > 0.0 {
                e.snap_distance_px
            } else {
                10.0
            },
        }
    }

    /// Writes the switches back to the plan defaults.
    pub fn store_in(&self, e: &mut EditingDefaults) {
        e.object_snaps = self.object_snaps;
        e.snap_endpoint = self.endpoint;
        e.snap_midpoint = self.midpoint;
        e.snap_intersection = self.intersection;
        e.snap_perpendicular = self.perpendicular;
        e.snap_on_object = self.on_object;
        e.snap_center = self.center;
        e.snap_quadrant = self.quadrant;
        e.snap_tangent = self.tangent;
        e.snap_extension = self.extension;
        e.snap_markers = self.markers;
        e.angle_snaps = self.angle;
        e.grid_snaps = self.grid;
        e.snap_distance_px = self.tolerance_px;
    }
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            object_snaps: true,
            endpoint: true,
            midpoint: true,
            intersection: true,
            perpendicular: true,
            on_object: true,
            center: true,
            quadrant: true,
            tangent: true,
            // Off until asked for: a point near the line of any wall would
            // otherwise jump onto it.
            extension: false,
            markers: true,
            angle: true,
            grid: true,
            tolerance_px: 10.0,
        }
    }
}

/// Everything one snap needs besides the raw point.
pub struct SnapQuery<'a> {
    pub floor: &'a Floor,
    pub layers: &'a LayerSet,
    /// Snap distance in inches.
    pub tol: f64,
    pub grid_step: f64,
    pub angle_deg: f64,
    /// Allowed angles (degrees); empty means every multiple of `angle_deg`.
    pub angles: &'a [f64],
    /// The pending start point (angle and perpendicular snaps measure from it).
    pub origin: Option<Point>,
    /// No angle snap (the angle is free but object and grid snaps apply).
    pub suspend_angle: bool,
    /// Alt held: no snap at all (S-74).
    pub suspend_all: bool,
    /// Walls to ignore (the ones being dragged).
    pub exclude: &'a [Id],
}

pub fn snap_to_grid(p: Point, step: f64) -> Point {
    if step <= 0.0 {
        return p;
    }
    Point::new((p.x / step).round() * step, (p.y / step).round() * step)
}

/// Snap `p` to a multiple of `increment_deg` from `start`, then re-snap the length to `step`.
pub fn angle_snap(start: Point, p: Point, step: f64, increment_deg: f64) -> Option<Point> {
    let v = p.sub(start);
    let len = v.length();
    if len < 1e-6 {
        return None;
    }
    let inc = increment_deg.max(1.0).to_radians();
    let a = (v.angle() / inc).round() * inc;
    let len = if step > 0.0 {
        (len / step).round() * step
    } else {
        len
    };
    Some(start.add(Point::new(a.cos(), a.sin()).scale(len)))
}

/// Snap `p` to the nearest allowed angle in `angles` (degrees, each also
/// allowing its opposite) from `start`, then re-snap the length to `step`.
pub fn angle_snap_list(start: Point, p: Point, step: f64, angles: &[f64]) -> Option<Point> {
    let v = p.sub(start);
    let len = v.length();
    if len < 1e-6 || angles.is_empty() {
        return None;
    }
    let here = v.angle().to_degrees().rem_euclid(360.0);
    let diff = |a: f64| {
        let d = (here - a).rem_euclid(360.0);
        d.min(360.0 - d)
    };
    let best = angles
        .iter()
        .flat_map(|a| [*a, *a + 180.0])
        .min_by(|a, b| diff(*a).total_cmp(&diff(*b)))?;
    let len = if step > 0.0 {
        (len / step).round() * step
    } else {
        len
    };
    let r = best.to_radians();
    Some(start.add(Point::new(r.cos(), r.sin()).scale(len)))
}

/// `a` is within the counter-clockwise sweep from `start` to `end`.
fn in_sweep(a: f64, start: f64, end: f64) -> bool {
    (a - start).rem_euclid(TAU) <= (end - start).rem_euclid(TAU) + 1e-9
}

/// A CAD circle or arc: center, radius and, for an arc, its counter-clockwise
/// sweep `(start, end)` in radians.
type Round = (Point, f64, Option<(f64, f64)>);

/// Circles and arcs of the visible CAD objects.
fn cad_rounds(q: &SnapQuery) -> Vec<Round> {
    q.floor
        .cad
        .iter()
        .filter(|c| q.layers.is_visible(&c.layer))
        .filter_map(|c| match &c.item {
            CadItem::Circle { center, radius } => Some((*center, *radius, None)),
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => Some((*center, *radius, Some((*start_angle, *end_angle)))),
            _ => None,
        })
        .collect()
}

fn on_round(center: Point, radius: f64, sweep: Option<(f64, f64)>, a: f64) -> Option<Point> {
    if sweep.is_some_and(|(s, e)| !in_sweep(a, s, e)) {
        return None;
    }
    Some(Point::new(
        center.x + radius * a.cos(),
        center.y + radius * a.sin(),
    ))
}

/// Segments of the visible CAD lines and polylines.
fn cad_segments(q: &SnapQuery) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    for c in q.floor.cad.iter().filter(|c| q.layers.is_visible(&c.layer)) {
        match &c.item {
            CadItem::Line { a, b } => out.push((*a, *b)),
            CadItem::Polyline { points, closed } => {
                for w in points.windows(2) {
                    out.push((w[0], w[1]));
                }
                if let (true, Some(f), Some(l)) = (*closed, points.first(), points.last()) {
                    out.push((*l, *f));
                }
            }
            _ => {}
        }
    }
    out
}

/// Arm half-length of the CAD point cross (Place Point, Point Marker).
use crate::tools::cad::POINT_SIZE as POINT_ARM;

/// The points of the visible CAD point objects and markers: the center of a
/// small cross (two perpendicular lines of the point size sharing their
/// midpoint, Place Point and Point Marker), of a Point Marker's circle, and
/// of a numbered marker (a circle around a centered number on the Text layer).
fn cad_marker_points(q: &SnapQuery) -> Vec<Point> {
    let mut out = Vec::new();
    let mut arms: Vec<(Point, Point)> = Vec::new();
    for c in q.floor.cad.iter().filter(|c| q.layers.is_visible(&c.layer)) {
        match &c.item {
            CadItem::Line { a, b } if (a.dist(*b) - 2.0 * POINT_ARM).abs() < 1e-6 => {
                arms.push((Point::lerp(*a, *b, 0.5), b.sub(*a)));
            }
            CadItem::Circle { center, radius } if (radius - POINT_ARM * 1.5).abs() < 1e-6 => {
                out.push(*center);
            }
            CadItem::Circle { center, radius } => {
                // A numbered marker: digits whose middle lies at the center.
                let numbered = q.floor.cad.iter().any(|t| match &t.item {
                    CadItem::Text {
                        pos, text, height, ..
                    } => {
                        !text.is_empty()
                            && text.chars().all(|ch| ch.is_ascii_digit())
                            && (*radius - height * 1.2).abs() < 1e-6
                            && Point::new(
                                pos.x
                                    + height
                                        * plan_core::cad::TEXT_WIDTH_FACTOR
                                        * text.chars().count() as f64
                                        * 0.5,
                                pos.y + height * 0.5,
                            )
                            .dist(*center)
                                < 1e-6
                    }
                    _ => false,
                });
                if numbered {
                    out.push(*center);
                }
            }
            _ => {}
        }
    }
    for (i, (m, d)) in arms.iter().enumerate() {
        let crossed = arms[i + 1..]
            .iter()
            .any(|(m2, d2)| m.dist(*m2) < 1e-6 && d.dot(*d2).abs() < 1e-6);
        if crossed {
            out.push(*m);
        }
    }
    out
}

/// Intersections of two segments, a segment and a circle or arc, and two
/// circles or arcs among the CAD objects and the walls (the wall pairs are
/// the walls' own, handled separately), keeping those within `reach` of
/// `raw`.
fn cad_intersections(
    raw: Point,
    reach: f64,
    walls: &[(Point, Point)],
    segments: &[(Point, Point)],
    rounds: &[Round],
) -> Vec<Point> {
    let near = |a: &Point, b: &Point| dist_to_segment(raw, *a, *b) <= reach;
    let segs: Vec<(Point, Point)> = segments
        .iter()
        .filter(|(a, b)| near(a, b))
        .copied()
        .collect();
    let wall_segs: Vec<(Point, Point)> =
        walls.iter().filter(|(a, b)| near(a, b)).copied().collect();
    let mut out = Vec::new();
    for (i, (a, b)) in segs.iter().enumerate() {
        for (c, d) in segs[i + 1..].iter().chain(wall_segs.iter()) {
            if let Some((t, _)) = segment_intersection(*a, *b, *c, *d) {
                out.push(Point::lerp(*a, *b, t));
            }
        }
        for (c, r, sweep) in rounds {
            out.extend(segment_round(*a, *b, *c, *r, *sweep));
        }
    }
    for (a, b) in &wall_segs {
        for (c, r, sweep) in rounds {
            out.extend(segment_round(*a, *b, *c, *r, *sweep));
        }
    }
    for (i, (c1, r1, s1)) in rounds.iter().enumerate() {
        for (c2, r2, s2) in &rounds[i + 1..] {
            out.extend(round_round(*c1, *r1, *s1, *c2, *r2, *s2));
        }
    }
    out.retain(|p| p.dist(raw) <= reach);
    out
}

/// Where the segment `ab` crosses a circle (or the arc's sweep).
fn segment_round(a: Point, b: Point, c: Point, r: f64, sweep: Option<(f64, f64)>) -> Vec<Point> {
    let d = b.sub(a);
    let f = a.sub(c);
    let qa = d.dot(d);
    if qa < 1e-12 {
        return Vec::new();
    }
    let qb = 2.0 * f.dot(d);
    let qc = f.dot(f) - r * r;
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return Vec::new();
    }
    let root = disc.sqrt();
    [(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]
        .into_iter()
        .filter(|t| (0.0..=1.0).contains(t))
        .map(|t| a.add(d.scale(t)))
        .filter(|p| sweep.is_none_or(|(s, e)| in_sweep(p.sub(c).angle(), s, e)))
        .collect()
}

/// Where two circles (or arcs) cross.
fn round_round(
    c1: Point,
    r1: f64,
    s1: Option<(f64, f64)>,
    c2: Point,
    r2: f64,
    s2: Option<(f64, f64)>,
) -> Vec<Point> {
    let d = c1.dist(c2);
    if d < 1e-9 || d > r1 + r2 || d < (r1 - r2).abs() {
        return Vec::new();
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h = (r1 * r1 - a * a).max(0.0).sqrt();
    let u = c2.sub(c1).scale(1.0 / d);
    let mid = c1.add(u.scale(a));
    let off = u.perp().scale(h);
    [mid.add(off), mid.sub(off)]
        .into_iter()
        .filter(|p| s1.is_none_or(|(s, e)| in_sweep(p.sub(c1).angle(), s, e)))
        .filter(|p| s2.is_none_or(|(s, e)| in_sweep(p.sub(c2).angle(), s, e)))
        .collect()
}

/// The point of the line through `a` and `b` nearest `raw`, when it lies
/// beyond an end of the segment (the extension of the line) and within `tol`.
fn on_extension(raw: Point, a: Point, b: Point, tol: f64) -> Option<Point> {
    let len = a.dist(b);
    if len < 1e-6 {
        return None;
    }
    let u = b.sub(a).scale(1.0 / len);
    let t = raw.sub(a).dot(u);
    // Beyond an end by more than the snap distance: nearer than that is the
    // end itself (Endpoint) or the segment (On Object).
    if t > -tol && t < len + tol {
        return None;
    }
    let p = a.add(u.scale(t));
    (p.dist(raw) <= tol).then_some(p)
}

fn nearest(cands: impl Iterator<Item = (Point, Id)>, raw: Point, tol: f64) -> Option<(Point, Id)> {
    cands
        .map(|(p, id)| (p, id, p.dist(raw)))
        .filter(|(_, _, d)| *d <= tol)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(p, id, _)| (p, id))
}

/// Runs the engine for the raw cursor point `raw`.
pub fn snap(raw: Point, q: &SnapQuery, s: &SnapSettings) -> SnapResult {
    if q.suspend_all {
        return SnapResult::free(raw);
    }
    let walls: Vec<_> = q
        .floor
        .walls
        .iter()
        .filter(|w| !q.exclude.contains(&w.id) && q.layers.is_visible(&w.layer))
        .collect();
    let hit = |kind: SnapKind, found: Option<(Point, Id)>| {
        found.map(|(point, id)| SnapResult {
            point,
            kind,
            source: Some(ObjectRef::Wall(id)),
        })
    };

    let cad_hit = |kind: SnapKind, p: Option<Point>| {
        p.map(|point| SnapResult {
            point,
            kind,
            source: None,
        })
    };
    let nearest_pt = |pts: Vec<Point>| {
        pts.into_iter()
            .filter(|p| p.dist(raw) <= q.tol)
            .min_by(|a, b| a.dist(raw).total_cmp(&b.dist(raw)))
    };
    let (rounds, segments) = if s.object_snaps {
        (cad_rounds(q), cad_segments(q))
    } else {
        (Vec::new(), Vec::new())
    };

    if s.object_snaps {
        if s.markers {
            if let Some(r) = cad_hit(SnapKind::Marker, nearest_pt(cad_marker_points(q))) {
                return r;
            }
        }
        if s.endpoint {
            let ends = walls.iter().flat_map(|w| [(w.start, w.id), (w.end, w.id)]);
            if let Some(r) = hit(SnapKind::Endpoint, nearest(ends, raw, q.tol)) {
                return r;
            }
            let mut pts: Vec<Point> = segments.iter().flat_map(|(a, b)| [*a, *b]).collect();
            for (c, r, sweep) in &rounds {
                if let Some((a0, a1)) = sweep {
                    pts.extend(on_round(*c, *r, None, *a0));
                    pts.extend(on_round(*c, *r, None, *a1));
                }
            }
            if let Some(r) = cad_hit(SnapKind::Endpoint, nearest_pt(pts)) {
                return r;
            }
        }
        if s.intersection {
            let near: Vec<_> = walls
                .iter()
                .filter(|w| dist_to_segment(raw, w.start, w.end) <= q.tol * 2.0)
                .collect();
            let mut pts = Vec::new();
            for (i, a) in near.iter().enumerate() {
                for b in &near[i + 1..] {
                    if let Some((t, _)) = segment_intersection(a.start, a.end, b.start, b.end) {
                        pts.push((Point::lerp(a.start, a.end, t), a.id));
                    }
                }
            }
            if let Some(r) = hit(SnapKind::Intersection, nearest(pts.into_iter(), raw, q.tol)) {
                return r;
            }
            // CAD against CAD, walls and circles.
            let wall_segs: Vec<(Point, Point)> = walls.iter().map(|w| (w.start, w.end)).collect();
            let crossings = cad_intersections(raw, q.tol, &wall_segs, &segments, &rounds);
            if let Some(r) = cad_hit(SnapKind::Intersection, nearest_pt(crossings)) {
                return r;
            }
        }
        if s.midpoint {
            let mids = walls
                .iter()
                .map(|w| (Point::lerp(w.start, w.end, 0.5), w.id));
            if let Some(r) = hit(SnapKind::Midpoint, nearest(mids, raw, q.tol)) {
                return r;
            }
            let pts = segments.iter().map(|(a, b)| Point::lerp(*a, *b, 0.5));
            if let Some(r) = cad_hit(SnapKind::Midpoint, nearest_pt(pts.collect())) {
                return r;
            }
        }
        if s.center {
            let pts = rounds.iter().map(|(c, _, _)| *c).collect();
            if let Some(r) = cad_hit(SnapKind::Center, nearest_pt(pts)) {
                return r;
            }
        }
        if s.quadrant {
            let mut pts = Vec::new();
            for (c, r, sweep) in &rounds {
                for k in 0..4 {
                    pts.extend(on_round(*c, *r, *sweep, k as f64 * TAU / 4.0));
                }
            }
            if let Some(r) = cad_hit(SnapKind::Quadrant, nearest_pt(pts)) {
                return r;
            }
        }
        if s.perpendicular {
            if let Some(o) = q.origin {
                let feet = walls.iter().filter_map(|w| {
                    let (t, foot) = project_on_segment(o, w.start, w.end);
                    ((0.0..=1.0).contains(&t) && foot.dist(o) > 1e-6).then_some((foot, w.id))
                });
                if let Some(r) = hit(SnapKind::Perpendicular, nearest(feet, raw, q.tol)) {
                    return r;
                }
            }
        }
        if s.tangent {
            if let Some(o) = q.origin {
                let mut pts = Vec::new();
                for (c, r, sweep) in &rounds {
                    let d = o.dist(*c);
                    if d <= *r + 1e-6 {
                        continue;
                    }
                    // Both tangent points of a line from `o` to the circle.
                    let base = o.sub(*c).angle();
                    let off = (r / d).acos();
                    for a in [base + off, base - off] {
                        pts.extend(on_round(*c, *r, *sweep, a));
                    }
                }
                if let Some(r) = cad_hit(SnapKind::Tangent, nearest_pt(pts)) {
                    return r;
                }
            }
        }
        if s.extension {
            let lines = walls
                .iter()
                .map(|w| (w.start, w.end, Some(w.id)))
                .chain(segments.iter().map(|(a, b)| (*a, *b, None)));
            let best = lines
                .filter_map(|(a, b, id)| on_extension(raw, a, b, q.tol).map(|p| (p, id)))
                .min_by(|a, b| a.0.dist(raw).total_cmp(&b.0.dist(raw)));
            if let Some((point, id)) = best {
                return SnapResult {
                    point,
                    kind: SnapKind::Extension,
                    source: id.map(ObjectRef::Wall),
                };
            }
        }
        if s.on_object {
            let best = walls
                .iter()
                .map(|w| {
                    let (_, p) = project_on_segment(raw, w.start, w.end);
                    (p, w.id)
                })
                .filter(|(p, _)| p.dist(raw) <= q.tol)
                .min_by(|a, b| a.0.dist(raw).total_cmp(&b.0.dist(raw)));
            if let Some(r) = hit(SnapKind::OnObject, best) {
                return r;
            }
            let on = segments
                .iter()
                .map(|(a, b)| project_on_segment(raw, *a, *b).1)
                .collect();
            if let Some(r) = cad_hit(SnapKind::OnObject, nearest_pt(on)) {
                return r;
            }
        }
    }

    if s.angle && !q.suspend_angle {
        if let Some(start) = q.origin {
            let snapped = if q.angles.is_empty() {
                (q.angle_deg > 0.0)
                    .then(|| angle_snap(start, raw, q.grid_step, q.angle_deg))
                    .flatten()
            } else {
                angle_snap_list(start, raw, q.grid_step, q.angles)
            };
            if let Some(point) = snapped {
                return SnapResult {
                    point,
                    kind: SnapKind::Angle,
                    source: None,
                };
            }
        }
    }
    if s.grid && q.grid_step > 0.0 {
        return SnapResult {
            point: snap_to_grid(raw, q.grid_step),
            kind: SnapKind::Grid,
            source: None,
        };
    }
    SnapResult::free(raw)
}

/// The centerlines of the reference floor's walls (R-65, LAY-10): what the
/// plan draws dimmed under the active floor, on the layers whose Ref box is
/// on. Empty when Reference Display is off.
pub fn reference_segments(cx: &super::EditorContext) -> Vec<(Point, Point)> {
    crate::dialogs::reference_display::reference_walls(cx)
        .iter()
        .map(|w| (w.start, w.end))
        .collect()
}

/// [`snap`] that also snaps to the reference floor (R-65): the ends and the
/// crossings of its wall centerlines. The active floor's own endpoints and
/// intersections come first; a reference end or crossing within the snap
/// distance beats the weaker snaps (midpoint, on object, angle, grid).
pub fn snap_with_reference(
    raw: Point,
    q: &SnapQuery,
    s: &SnapSettings,
    reference: &[(Point, Point)],
) -> SnapResult {
    let base = snap(raw, q, s);
    if reference.is_empty()
        || q.suspend_all
        || !s.object_snaps
        || matches!(base.kind, SnapKind::Endpoint | SnapKind::Intersection)
    {
        return base;
    }
    let nearest_of = |pts: Vec<Point>| {
        pts.into_iter()
            .filter(|p| p.dist(raw) <= q.tol)
            .min_by(|a, b| a.dist(raw).total_cmp(&b.dist(raw)))
    };
    if s.endpoint {
        let ends = reference.iter().flat_map(|(a, b)| [*a, *b]).collect();
        if let Some(point) = nearest_of(ends) {
            return SnapResult {
                point,
                kind: SnapKind::Endpoint,
                source: None,
            };
        }
    }
    if s.intersection {
        let near: Vec<_> = reference
            .iter()
            .filter(|(a, b)| dist_to_segment(raw, *a, *b) <= q.tol * 2.0)
            .collect();
        let mut pts = Vec::new();
        for (i, (a1, a2)) in near.iter().enumerate() {
            for (b1, b2) in &near[i + 1..] {
                if let Some((t, _)) = segment_intersection(*a1, *a2, *b1, *b2) {
                    pts.push(Point::lerp(*a1, *a2, t));
                }
            }
        }
        if let Some(point) = nearest_of(pts) {
            return SnapResult {
                point,
                kind: SnapKind::Intersection,
                source: None,
            };
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{Project, WallKind};

    fn project() -> Project {
        let mut p = Project::new("t");
        for (a, b) in [((0.0, 0.0), (120.0, 0.0)), ((60.0, -50.0), (60.0, 50.0))] {
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                4.5,
                100.0,
                WallKind::Interior,
            );
        }
        p
    }

    fn q<'a>(p: &'a Project, origin: Option<Point>) -> SnapQuery<'a> {
        SnapQuery {
            floor: &p.floors[0],
            layers: &p.layers,
            tol: 5.0,
            grid_step: 1.0,
            angle_deg: 15.0,
            angles: &[],
            origin,
            suspend_angle: false,
            suspend_all: false,
            exclude: &[],
        }
    }

    #[test]
    fn endpoint_beats_grid() {
        let p = project();
        let r = snap(
            Point::new(121.3, 2.2),
            &q(&p, None),
            &SnapSettings::default(),
        );
        assert_eq!(r.kind, SnapKind::Endpoint);
        assert_eq!(r.point, Point::new(120.0, 0.0));
        assert!(matches!(r.source, Some(ObjectRef::Wall(_))));
        let far = snap(
            Point::new(40.4, 30.6),
            &q(&p, None),
            &SnapSettings::default(),
        );
        assert_eq!(
            (far.kind, far.point),
            (SnapKind::Grid, Point::new(40.0, 31.0))
        );
    }

    #[test]
    fn intersection_midpoint_and_on_object() {
        let p = project();
        let s = SnapSettings::default();
        let r = snap(Point::new(61.0, 1.0), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Intersection, Point::new(60.0, 0.0))
        );
        let r = snap(Point::new(30.0, 2.0), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::OnObject, Point::new(30.0, 0.0))
        );
        let r = snap(Point::new(60.0, 47.0), &q(&p, None), &s);
        assert_eq!(r.kind, SnapKind::Endpoint);
        let r = snap(Point::new(60.0, 24.0), &q(&p, None), &s);
        assert_eq!(r.kind, SnapKind::OnObject);
    }

    #[test]
    fn perpendicular_from_the_pending_start() {
        let p = project();
        let origin = Some(Point::new(30.0, 40.0));
        let r = snap(
            Point::new(31.0, 2.0),
            &q(&p, origin),
            &SnapSettings::default(),
        );
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Perpendicular, Point::new(30.0, 0.0))
        );
    }

    #[test]
    fn angle_snap_unless_suspended() {
        let p = project();
        let origin = Some(Point::new(0.0, 100.0));
        let raw = Point::new(100.0, 105.0);
        let r = snap(raw, &q(&p, origin), &SnapSettings::default());
        assert_eq!(r.kind, SnapKind::Angle);
        assert!((r.point.y - 100.0).abs() < 1e-6);
        let mut query = q(&p, origin);
        query.suspend_angle = true;
        assert_eq!(
            snap(raw, &query, &SnapSettings::default()).kind,
            SnapKind::Grid
        );
    }

    #[test]
    fn hidden_walls_and_excluded_walls_do_not_snap() {
        let mut p = project();
        let id = p.floors[0].walls[0].id;
        let ids = [id];
        let mut query = q(&p, None);
        query.exclude = &ids;
        let r = snap(Point::new(121.0, 1.0), &query, &SnapSettings::default());
        assert_eq!(r.kind, SnapKind::Grid);
        p.layers.set_display("Walls, Normal", false);
        let r = snap(
            Point::new(121.0, 1.0),
            &q(&p, None),
            &SnapSettings::default(),
        );
        assert_eq!(r.kind, SnapKind::Grid);
    }

    fn with_circle() -> Project {
        let mut p = Project::new("t");
        p.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(200.0, 200.0),
                radius: 50.0,
            },
        );
        p
    }

    #[test]
    fn settings_switch_the_object_snaps_off_and_on() {
        let p = project();
        let raw = Point::new(121.3, 2.2);
        let mut s = SnapSettings::default();
        assert_eq!(snap(raw, &q(&p, None), &s).kind, SnapKind::Endpoint);
        s.endpoint = false;
        // The endpoint is still on the wall: On Object takes over.
        assert_eq!(snap(raw, &q(&p, None), &s).kind, SnapKind::OnObject);
        s.on_object = false;
        assert_eq!(snap(raw, &q(&p, None), &s).kind, SnapKind::Grid);
        s.grid = false;
        assert_eq!(snap(raw, &q(&p, None), &s).kind, SnapKind::Free);
        s.grid = true;
        s.endpoint = true;
        s.object_snaps = false;
        assert_eq!(snap(raw, &q(&p, None), &s).kind, SnapKind::Grid);
    }

    #[test]
    fn center_quadrant_and_tangent_snap_to_cad_circles() {
        let p = with_circle();
        let s = SnapSettings::default();
        let r = snap(Point::new(201.0, 199.0), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Center, Point::new(200.0, 200.0))
        );
        let r = snap(Point::new(251.0, 202.0), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Quadrant, Point::new(250.0, 200.0))
        );
        // From (200, 100) the tangent points are 50 from the center and 100
        // from the origin, at 30 degrees off the vertical.
        let origin = Some(Point::new(200.0, 100.0));
        let d: f64 = 100.0;
        let t = (50.0f64 / d).acos();
        let want = Point::new(
            200.0 + 50.0 * (-std::f64::consts::FRAC_PI_2 + t).cos(),
            200.0 + 50.0 * (-std::f64::consts::FRAC_PI_2 + t).sin(),
        );
        let r = snap(want + Point::new(1.0, -1.0), &q(&p, origin), &s);
        assert_eq!(r.kind, SnapKind::Tangent);
        assert!(r.point.dist(want) < 1e-6, "{:?} vs {want:?}", r.point);
        let mut off = s;
        off.center = false;
        off.quadrant = false;
        off.tangent = false;
        let r = snap(Point::new(201.0, 199.0), &q(&p, None), &off);
        assert_eq!(r.kind, SnapKind::Grid);
    }

    #[test]
    fn cad_line_endpoints_and_midpoints_snap() {
        let mut p = Project::new("t");
        p.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(0.0, 300.0),
                b: Point::new(100.0, 300.0),
            },
        );
        let s = SnapSettings::default();
        let r = snap(Point::new(101.0, 301.0), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Endpoint, Point::new(100.0, 300.0))
        );
        let r = snap(Point::new(50.0, 302.0), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Midpoint, Point::new(50.0, 300.0))
        );
    }

    #[test]
    fn alt_suspends_every_snap_and_the_tolerance_is_a_setting() {
        let p = project();
        let raw = Point::new(121.3, 2.2);
        let mut query = q(&p, None);
        query.suspend_all = true;
        let r = snap(raw, &query, &SnapSettings::default());
        assert_eq!((r.kind, r.point), (SnapKind::Free, raw));
        // `tol` is the screen tolerance over the zoom: a narrower one misses.
        let mut tight = q(&p, None);
        tight.tol = 1.0;
        assert_eq!(
            snap(raw, &tight, &SnapSettings::default()).kind,
            SnapKind::Grid
        );
    }

    #[test]
    fn allowed_angles_replace_the_increment() {
        let p = project();
        let origin = Some(Point::new(0.0, 100.0));
        // 40 degrees: the 15 degree increments give 45; a 0/90 list gives 0.
        let raw = Point::new(100.0, 100.0 + 100.0 * 40.0f64.to_radians().tan());
        let by_increment = snap(raw, &q(&p, origin), &SnapSettings::default());
        assert_eq!(by_increment.kind, SnapKind::Angle);
        assert!((by_increment.point.y - 100.0 - by_increment.point.x).abs() < 1e-6);
        let list = [0.0, 90.0];
        let mut query = q(&p, origin);
        query.angles = &list;
        let r = snap(raw, &query, &SnapSettings::default());
        assert_eq!(r.kind, SnapKind::Angle);
        assert!((r.point.y - 100.0).abs() < 1e-6, "{:?}", r.point);
        // The opposite direction is allowed too.
        let back = snap(Point::new(-100.0, 103.0), &query, &SnapSettings::default());
        assert!((back.point.y - 100.0).abs() < 1e-6 && back.point.x < 0.0);
        // A zero increment turns the angle snap into the grid.
        let mut none = q(&p, origin);
        none.angle_deg = 0.0;
        assert_eq!(
            snap(raw, &none, &SnapSettings::default()).kind,
            SnapKind::Grid
        );
    }

    fn with_cad(items: &[CadItem]) -> Project {
        let mut p = Project::new("t");
        for it in items {
            p.add_cad(0, "CAD, Default", it.clone());
        }
        p
    }

    #[test]
    fn cad_lines_snap_to_their_crossing() {
        let p = with_cad(&[
            CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(100.0, 100.0),
            },
            CadItem::Line {
                a: Point::new(0.0, 60.0),
                b: Point::new(120.0, 0.0),
            },
        ]);
        let s = SnapSettings::default();
        // The diagonal y = x meets y = 60 - x/2 at (40, 40), no one's midpoint.
        let r = snap(Point::new(42.0, 38.5), &q(&p, None), &s);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Intersection, Point::new(40.0, 40.0))
        );
        // Off: the lines only give On Object.
        let off = SnapSettings {
            intersection: false,
            ..s
        };
        let r = snap(Point::new(42.0, 38.5), &q(&p, None), &off);
        assert_eq!(r.kind, SnapKind::OnObject);
        // A wall crossing a CAD line.
        let mut p = with_cad(&[CadItem::Line {
            a: Point::new(30.0, -40.0),
            b: Point::new(30.0, 40.0),
        }]);
        p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            4.5,
            100.0,
            WallKind::Interior,
        );
        let r = snap(
            Point::new(31.0, 1.5),
            &q(&p, None),
            &SnapSettings::default(),
        );
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Intersection, Point::new(30.0, 0.0))
        );
    }

    #[test]
    fn cad_circles_and_arcs_snap_to_where_they_cross() {
        let p = with_cad(&[
            CadItem::Circle {
                center: Point::new(0.0, 0.0),
                radius: 50.0,
            },
            CadItem::Line {
                a: Point::new(-100.0, 30.0),
                b: Point::new(100.0, 30.0),
            },
            CadItem::Circle {
                center: Point::new(80.0, 0.0),
                radius: 50.0,
            },
        ]);
        let s = SnapSettings::default();
        // Line x circle: y = 30 on a radius 50 circle at x = 40.
        let r = snap(Point::new(41.5, 31.0), &q(&p, None), &s);
        assert_eq!(r.kind, SnapKind::Intersection);
        assert!(r.point.dist(Point::new(40.0, 30.0)) < 1e-9, "{:?}", r.point);
        // Circle x circle: x = 40, y = +-30.
        let r = snap(Point::new(41.0, -28.5), &q(&p, None), &s);
        assert_eq!(r.kind, SnapKind::Intersection);
        assert!(
            r.point.dist(Point::new(40.0, -30.0)) < 1e-9,
            "{:?}",
            r.point
        );
        // An arc only crosses where its sweep is: the upper half arc has no
        // crossing at y = -30.
        let p = with_cad(&[
            CadItem::Arc {
                center: Point::new(0.0, 0.0),
                radius: 50.0,
                start_angle: 0.0,
                end_angle: std::f64::consts::PI,
            },
            CadItem::Line {
                a: Point::new(-100.0, -30.0),
                b: Point::new(100.0, -30.0),
            },
        ]);
        let r = snap(Point::new(40.0, -30.0), &q(&p, None), &s);
        assert_ne!(r.kind, SnapKind::Intersection);
    }

    #[test]
    fn extension_snaps_to_the_line_beyond_a_wall_or_cad_line() {
        let p = project();
        let on = SnapSettings {
            extension: true,
            ..SnapSettings::default()
        };
        // Beyond the east end of the wall (0,0)-(120,0).
        let r = snap(Point::new(150.0, 2.0), &q(&p, None), &on);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Extension, Point::new(150.0, 0.0))
        );
        assert!(matches!(r.source, Some(ObjectRef::Wall(_))));
        // Before the west end too; far off the line it is just the grid.
        let r = snap(Point::new(-40.0, -1.5), &q(&p, None), &on);
        assert_eq!(
            (r.kind, r.point),
            (SnapKind::Extension, Point::new(-40.0, 0.0))
        );
        let r = snap(Point::new(150.0, 30.0), &q(&p, None), &on);
        assert_eq!(r.kind, SnapKind::Grid);
        // Off by default, and the end itself still wins near the end.
        let r = snap(
            Point::new(150.0, 2.0),
            &q(&p, None),
            &SnapSettings::default(),
        );
        assert_ne!(r.kind, SnapKind::Extension);
        let r = snap(Point::new(122.0, 1.0), &q(&p, None), &on);
        assert_eq!(r.kind, SnapKind::Endpoint);
        // A CAD line's extension.
        let c = with_cad(&[CadItem::Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(60.0, 60.0),
        }]);
        let r = snap(Point::new(100.0, 98.0), &q(&c, None), &on);
        assert_eq!(r.kind, SnapKind::Extension);
        assert!(r.point.dist(Point::new(99.0, 99.0)) < 1e-9, "{:?}", r.point);
    }

    #[test]
    fn cad_points_and_markers_snap_at_their_center() {
        use crate::tools::cad::point_items;
        use crate::tools::text::marker_items;
        let mut items = point_items(Point::new(200.0, 200.0), false);
        items.extend(point_items(Point::new(300.0, 200.0), true));
        items.extend(marker_items(Point::new(400.0, 200.0), 12, 6.0));
        let p = with_cad(&items);
        let s = SnapSettings::default();
        // Near the arm tips of the cross, the point's center still wins.
        for (raw, center) in [
            (Point::new(201.5, 200.5), Point::new(200.0, 200.0)),
            (Point::new(298.5, 201.0), Point::new(300.0, 200.0)),
            (Point::new(403.0, 198.0), Point::new(400.0, 200.0)),
        ] {
            let r = snap(raw, &q(&p, None), &s);
            assert_eq!((r.kind, r.point), (SnapKind::Marker, center), "{raw:?}");
        }
        // Off: the arm tip (an endpoint) takes over.
        let off = SnapSettings {
            markers: false,
            ..s
        };
        let r = snap(Point::new(201.5, 200.5), &q(&p, None), &off);
        assert_eq!(r.kind, SnapKind::Endpoint);
        // A lone short line is not a point.
        let p = with_cad(&[CadItem::Line {
            a: Point::new(10.0, 10.0),
            b: Point::new(14.0, 10.0),
        }]);
        let r = snap(Point::new(12.0, 10.5), &q(&p, None), &s);
        assert_ne!(r.kind, SnapKind::Marker);
    }

    /// A context with a house on floor 0 and the empty floor 1 active,
    /// Reference Display on (the floor below is the default reference).
    fn reference_context() -> crate::editor::EditorContext {
        use crate::editor::EditorContext;
        use crate::toolbar::ViewFlag;
        crate::dialogs::reference_display::reset_settings();
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let corners = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 144.0),
            Point::new(0.0, 144.0),
        ];
        for i in 0..4 {
            cx.project.add_wall(
                0,
                corners[i],
                corners[(i + 1) % 4],
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        cx.project
            .floors
            .push(plan_core::Floor::new("Second", 109.0));
        cx.floor = 1;
        cx.view_flags.insert(ViewFlag::ReferenceDisplay);
        cx
    }

    #[test]
    fn the_reference_floors_wall_ends_and_crossings_snap() {
        let cx = reference_context();
        let tol = cx.snap_tol();
        // Just off the reference corner at (240, 144).
        let raw = Point::new(240.0 - tol * 0.4, 144.0 + tol * 0.3);
        let r = cx.snap_at(raw, None, false, &[]);
        assert_eq!(r.kind, SnapKind::Endpoint);
        assert!(
            r.point.dist(Point::new(240.0, 144.0)) < 1e-9,
            "{:?}",
            r.point
        );
        // Away from any end the reference floor does nothing.
        let far = Point::new(120.0, 70.0);
        assert_ne!(cx.snap_at(far, None, false, &[]).kind, SnapKind::Endpoint);
        // Alt suspends it like every snap.
        assert_eq!(cx.snap_at(raw, None, true, &[]).kind, SnapKind::Free);
    }

    #[test]
    fn a_reference_crossing_snaps_as_an_intersection() {
        // Two reference walls that cross mid-wall, no shared end.
        let mut cx = reference_context();
        cx.project.floors[0].walls.clear();
        for (a, b) in [
            (Point::new(0.0, 60.0), Point::new(240.0, 60.0)),
            (Point::new(120.0, 0.0), Point::new(120.0, 144.0)),
        ] {
            cx.project.add_wall(0, a, b, 4.5, 96.0, WallKind::Interior);
        }
        let tol = cx.snap_tol();
        let r = cx.snap_at(
            Point::new(120.0 + tol * 0.3, 60.0 - tol * 0.3),
            None,
            false,
            &[],
        );
        assert_eq!(r.kind, SnapKind::Intersection);
        assert!(
            r.point.dist(Point::new(120.0, 60.0)) < 1e-9,
            "{:?}",
            r.point
        );
    }

    #[test]
    fn the_active_floors_own_ends_come_before_the_reference() {
        let mut cx = reference_context();
        // A wall on the active floor ending 2" from the reference corner.
        cx.project.add_wall(
            1,
            Point::new(238.0, 144.0),
            Point::new(238.0, 300.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let r = cx.snap_at(Point::new(238.5, 144.3), None, false, &[]);
        assert_eq!(r.kind, SnapKind::Endpoint);
        assert!(
            r.point.dist(Point::new(238.0, 144.0)) < 1e-9,
            "{:?}",
            r.point
        );
    }

    #[test]
    fn a_layer_with_its_ref_box_off_does_not_snap_on_the_reference_floor() {
        let mut cx = reference_context();
        let layer = cx.project.floors[0].walls[0].layer.clone();
        let raw = Point::new(239.0, 144.5);
        assert_eq!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Endpoint);
        assert!(!super::reference_segments(&cx).is_empty());
        assert!(crate::shell::docks::set_layer_reference(
            &mut cx, &layer, false
        ));
        assert!(
            super::reference_segments(&cx).is_empty(),
            "every wall is on {layer}"
        );
        assert_ne!(cx.snap_at(raw, None, false, &[]).kind, SnapKind::Endpoint);
        // And it no longer draws there either.
        assert!(crate::editor::render::reference_polygons(&cx).is_empty());
        assert!(crate::shell::docks::set_layer_reference(
            &mut cx, &layer, true
        ));
        assert!(!crate::editor::render::reference_polygons(&cx).is_empty());
        // The display off hides the reference altogether.
        cx.view_flags
            .remove(&crate::toolbar::ViewFlag::ReferenceDisplay);
        assert!(super::reference_segments(&cx).is_empty());
    }

    #[test]
    fn settings_round_trip_through_the_editing_defaults() {
        let mut e = EditingDefaults::default();
        assert_eq!(SnapSettings::from_editing(&e), SnapSettings::default());
        let s = SnapSettings {
            midpoint: false,
            tangent: false,
            tolerance_px: 14.0,
            ..SnapSettings::default()
        };
        s.store_in(&mut e);
        assert!(!e.snap_midpoint && !e.snap_tangent && e.snap_distance_px == 14.0);
        assert_eq!(SnapSettings::from_editing(&e), s);
        let s = SnapSettings {
            extension: true,
            markers: false,
            ..s
        };
        s.store_in(&mut e);
        assert!(e.snap_extension && !e.snap_markers);
        assert_eq!(SnapSettings::from_editing(&e), s);
    }
}
