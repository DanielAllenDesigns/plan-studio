//! Plain 2D annotation primitives (Chief's CAD tools): lines, arcs, circles,
//! polylines and text. Angles are radians, counter-clockwise from +X.

use crate::geometry::{segment_intersection, Point};
use crate::layers::LineStyle;
use crate::model::{Id, Project};
use crate::text_styles::RichRun;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CadItem {
    Line {
        a: Point,
        b: Point,
    },
    /// Counter-clockwise arc from `start_angle` to `end_angle` (radians).
    Arc {
        center: Point,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    Polyline {
        points: Vec<Point>,
        closed: bool,
    },
    /// Text anchored at its bottom-left; `height` in inches, `angle` radians.
    Text {
        pos: Point,
        text: String,
        height: f64,
        angle: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadObject {
    pub id: Id,
    pub layer: String,
    pub item: CadItem,
}

/// Default layer for new CAD items.
pub const DEFAULT_CAD_LAYER: &str = "CAD, Default";

fn min_max(points: impl IntoIterator<Item = Point>) -> (Point, Point) {
    let mut it = points.into_iter();
    let Some(first) = it.next() else {
        return (Point::ZERO, Point::ZERO);
    };
    let (mut lo, mut hi) = (first, first);
    for p in it {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    (lo, hi)
}

/// Is `angle` within the counter-clockwise sweep from `start` to `end`?
fn in_sweep(angle: f64, start: f64, end: f64) -> bool {
    let sweep = (end - start).rem_euclid(TAU);
    let rel = (angle - start).rem_euclid(TAU);
    rel <= sweep
}

/// Rough text width: average glyph is about 0.6 of the text height.
pub const TEXT_WIDTH_FACTOR: f64 = 0.6;

impl CadItem {
    /// Axis-aligned bounds `(min, max)`.
    pub fn bounds(&self) -> (Point, Point) {
        match self {
            CadItem::Line { a, b } => min_max([*a, *b]),
            CadItem::Circle { center, radius } => min_max([
                Point::new(center.x - radius, center.y - radius),
                Point::new(center.x + radius, center.y + radius),
            ]),
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let at =
                    |a: f64| Point::new(center.x + radius * a.cos(), center.y + radius * a.sin());
                let mut pts = vec![at(*start_angle), at(*end_angle)];
                for k in 0..4 {
                    let a = k as f64 * FRAC_PI_2;
                    if in_sweep(a, *start_angle, *end_angle) {
                        pts.push(at(a));
                    }
                }
                min_max(pts)
            }
            CadItem::Polyline { points, .. } => min_max(points.iter().copied()),
            CadItem::Text {
                pos,
                text,
                height,
                angle,
            } => {
                let w = text.chars().count() as f64 * height * TEXT_WIDTH_FACTOR;
                let (c, s) = (angle.cos(), angle.sin());
                let corner =
                    |dx: f64, dy: f64| Point::new(pos.x + dx * c - dy * s, pos.y + dx * s + dy * c);
                min_max([
                    corner(0.0, 0.0),
                    corner(w, 0.0),
                    corner(w, *height),
                    corner(0.0, *height),
                ])
            }
        }
    }
}

impl CadObject {
    pub fn bounds(&self) -> (Point, Point) {
        self.item.bounds()
    }
}

// ===== CAD editing geometry (fillet, chamfer, offset, trim, extend, break) =====

const EPS: f64 = 1e-9;

/// Intersection of the infinite lines through `a1 a2` and `b1 b2`; `None`
/// when they are parallel.
pub fn line_intersection(a1: Point, a2: Point, b1: Point, b2: Point) -> Option<Point> {
    let r = a2.sub(a1);
    let s = b2.sub(b1);
    let denom = r.cross(s);
    if denom.abs() <= 1e-12 {
        return None;
    }
    let t = b1.sub(a1).cross(s) / denom;
    Some(a1.add(r.scale(t)))
}

/// A rounded corner between two rays (see [`fillet_corner`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Fillet {
    /// Where the arc leaves the first ray.
    pub t0: Point,
    /// Where the arc leaves the second ray.
    pub t1: Point,
    pub center: Point,
    /// The counter-clockwise arc from one tangent point to the other.
    pub arc: CadItem,
}

/// Rounds the corner at `corner` between the rays toward `p0` and `p1` with an
/// arc of `radius` tangent to both. `None` when the radius is not positive,
/// the rays are collinear, or the tangent points would lie beyond `p0`/`p1`.
pub fn fillet_corner(p0: Point, corner: Point, p1: Point, radius: f64) -> Option<Fillet> {
    if radius <= EPS {
        return None;
    }
    let (v0, v1) = (p0.sub(corner), p1.sub(corner));
    let (l0, l1) = (v0.length(), v1.length());
    if l0 < EPS || l1 < EPS {
        return None;
    }
    let (u0, u1) = (v0.scale(1.0 / l0), v1.scale(1.0 / l1));
    let theta = u0.dot(u1).clamp(-1.0, 1.0).acos();
    if !(1e-6..=PI - 1e-6).contains(&theta) {
        return None;
    }
    let half = theta * 0.5;
    let d = radius / half.tan();
    if d > l0 + EPS || d > l1 + EPS {
        return None;
    }
    let t0 = corner.add(u0.scale(d));
    let t1 = corner.add(u1.scale(d));
    let center = corner.add(u0.add(u1).normalized().scale(radius / half.sin()));
    let (a0, a1) = (t0.sub(center).angle(), t1.sub(center).angle());
    let (start_angle, end_angle) = if (a1 - a0).rem_euclid(TAU) <= PI {
        (a0, a1)
    } else {
        (a1, a0)
    };
    Some(Fillet {
        t0,
        t1,
        center,
        arc: CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        },
    })
}

/// The result of filleting or chamfering two segments: both shortened (or
/// extended) to the joint, plus the joining arc or line.
#[derive(Debug, Clone, PartialEq)]
pub struct JoinedLines {
    pub first: (Point, Point),
    pub second: (Point, Point),
    /// The fillet arc or chamfer line; `None` for a sharp corner.
    pub joiner: Option<CadItem>,
}

/// `seg` with the end that is not `keep` moved to `to`.
fn retarget(seg: (Point, Point), keep: Point, to: Point) -> (Point, Point) {
    if seg.0.dist(keep) <= seg.1.dist(keep) {
        (seg.0, to)
    } else {
        (to, seg.1)
    }
}

/// The end of `seg` on the side of `corner` that `pick` is on; with the pick
/// on neither side (or on the corner) the end farther from `corner`.
fn kept_end(seg: (Point, Point), corner: Point, pick: Point) -> Point {
    let toward = pick.sub(corner);
    let (d0, d1) = (seg.0.sub(corner).dot(toward), seg.1.sub(corner).dot(toward));
    if (d0 - d1).abs() > EPS {
        return if d0 > d1 { seg.0 } else { seg.1 };
    }
    if seg.0.dist(corner) >= seg.1.dist(corner) {
        seg.0
    } else {
        seg.1
    }
}

/// Fillets two segments (extending or shortening both to their meeting
/// point). `pick0` and `pick1` say which side of the meeting point to keep on
/// each (the points the user clicked). Radius zero gives a sharp corner.
/// `None` for parallel segments or when the radius does not fit.
pub fn fillet_lines_picked(
    l0: (Point, Point),
    pick0: Point,
    l1: (Point, Point),
    pick1: Point,
    radius: f64,
) -> Option<JoinedLines> {
    let c = line_intersection(l0.0, l0.1, l1.0, l1.1)?;
    let (k0, k1) = (kept_end(l0, c, pick0), kept_end(l1, c, pick1));
    if radius <= EPS {
        return Some(JoinedLines {
            first: retarget(l0, k0, c),
            second: retarget(l1, k1, c),
            joiner: None,
        });
    }
    let f = fillet_corner(k0, c, k1, radius)?;
    Some(JoinedLines {
        first: retarget(l0, k0, f.t0),
        second: retarget(l1, k1, f.t1),
        joiner: Some(f.arc),
    })
}

/// [`fillet_lines_picked`] keeping the far end of each segment.
pub fn fillet_lines(l0: (Point, Point), l1: (Point, Point), radius: f64) -> Option<JoinedLines> {
    let c = line_intersection(l0.0, l0.1, l1.0, l1.1)?;
    let far = |s: (Point, Point)| {
        if s.0.dist(c) >= s.1.dist(c) {
            s.0
        } else {
            s.1
        }
    };
    fillet_lines_picked(l0, far(l0), l1, far(l1), radius)
}

/// Chamfers two segments: `d0` along the first and `d1` along the second
/// from their meeting point, keeping the sides the picks are on. `None` for
/// parallel segments or when a distance is longer than its segment.
pub fn chamfer_lines_picked(
    l0: (Point, Point),
    pick0: Point,
    l1: (Point, Point),
    pick1: Point,
    d0: f64,
    d1: f64,
) -> Option<JoinedLines> {
    let c = line_intersection(l0.0, l0.1, l1.0, l1.1)?;
    let (k0, k1) = (kept_end(l0, c, pick0), kept_end(l1, c, pick1));
    if d0 <= EPS || d1 <= EPS || d0 > k0.dist(c) + EPS || d1 > k1.dist(c) + EPS {
        return None;
    }
    let t0 = c.add(k0.sub(c).normalized().scale(d0));
    let t1 = c.add(k1.sub(c).normalized().scale(d1));
    Some(JoinedLines {
        first: retarget(l0, k0, t0),
        second: retarget(l1, k1, t1),
        joiner: Some(CadItem::Line { a: t0, b: t1 }),
    })
}

/// [`chamfer_lines_picked`] keeping the far end of each segment.
pub fn chamfer_lines(
    l0: (Point, Point),
    l1: (Point, Point),
    d0: f64,
    d1: f64,
) -> Option<JoinedLines> {
    let c = line_intersection(l0.0, l0.1, l1.0, l1.1)?;
    let far = |s: (Point, Point)| {
        if s.0.dist(c) >= s.1.dist(c) {
            s.0
        } else {
            s.1
        }
    };
    chamfer_lines_picked(l0, far(l0), l1, far(l1), d0, d1)
}

/// Neighbours of vertex `i` of a polyline, `None` at the ends of an open one.
fn neighbours(points: &[Point], closed: bool, i: usize) -> Option<(Point, Point)> {
    let n = points.len();
    if i >= n || n < 3 {
        return None;
    }
    if closed {
        Some((points[(i + n - 1) % n], points[(i + 1) % n]))
    } else if i == 0 || i + 1 == n {
        None
    } else {
        Some((points[i - 1], points[i + 1]))
    }
}

/// The polyline with vertex `i` rounded by an arc of `radius`, sampled in
/// steps of at most `step_deg` degrees (the model has no arc segments).
pub fn fillet_polyline_vertex(
    points: &[Point],
    closed: bool,
    i: usize,
    radius: f64,
    step_deg: f64,
) -> Option<Vec<Point>> {
    let (prev, next) = neighbours(points, closed, i)?;
    let f = fillet_corner(prev, points[i], next, radius)?;
    let a0 = f.t0.sub(f.center).angle();
    let a1 = f.t1.sub(f.center).angle();
    let mut delta = (a1 - a0).rem_euclid(TAU);
    if delta > PI {
        delta -= TAU;
    }
    let n = ((delta.abs().to_degrees() / step_deg.max(1.0)).ceil() as usize).max(2);
    let mut arc: Vec<Point> = (0..=n)
        .map(|k| {
            let a = a0 + delta * k as f64 / n as f64;
            Point::new(f.center.x + radius * a.cos(), f.center.y + radius * a.sin())
        })
        .collect();
    arc[0] = f.t0;
    if let Some(last) = arc.last_mut() {
        *last = f.t1;
    }
    let mut out = points[..i].to_vec();
    out.extend(arc);
    out.extend_from_slice(&points[i + 1..]);
    Some(out)
}

/// The polyline with vertex `i` cut off by a chamfer of `d0` back along the
/// previous segment and `d1` along the next.
pub fn chamfer_polyline_vertex(
    points: &[Point],
    closed: bool,
    i: usize,
    d0: f64,
    d1: f64,
) -> Option<Vec<Point>> {
    let (prev, next) = neighbours(points, closed, i)?;
    let c = points[i];
    if d0 <= EPS || d1 <= EPS || d0 > prev.dist(c) + EPS || d1 > next.dist(c) + EPS {
        return None;
    }
    let t0 = c.add(prev.sub(c).normalized().scale(d0));
    let t1 = c.add(next.sub(c).normalized().scale(d1));
    let mut out = points[..i].to_vec();
    out.extend([t0, t1]);
    out.extend_from_slice(&points[i + 1..]);
    Some(out)
}

/// A polyline offset by `dist` to the left of its direction of travel
/// (negative: right), with mitered corners. Consecutive duplicate points are
/// ignored. Fewer than two distinct points give an empty result.
pub fn offset_polyline(points: &[Point], closed: bool, dist: f64) -> Vec<Point> {
    let mut pts: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if pts.last().is_none_or(|q| q.dist(*p) > EPS) {
            pts.push(*p);
        }
    }
    if closed && pts.len() > 1 && pts[0].dist(pts[pts.len() - 1]) <= EPS {
        pts.pop();
    }
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let segs = if closed { n } else { n - 1 };
    let normal = |i: usize| pts[(i + 1) % n].sub(pts[i]).normalized().perp();
    let shifted = |i: usize| {
        let k = normal(i).scale(dist);
        (pts[i].add(k), pts[(i + 1) % n].add(k))
    };
    let joint = |prev: usize, next: usize, at: Point| {
        let (p0, p1) = shifted(prev);
        let (q0, q1) = shifted(next);
        match line_intersection(p0, p1, q0, q1) {
            Some(m) if m.dist(at) <= dist.abs() * 4.0 + EPS => m,
            _ => at.add(normal(prev).add(normal(next)).normalized().scale(dist)),
        }
    };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if closed {
            out.push(joint((i + segs - 1) % segs, i, pts[i]));
        } else if i == 0 {
            out.push(pts[0].add(normal(0).scale(dist)));
        } else if i == n - 1 {
            out.push(pts[n - 1].add(normal(n - 2).scale(dist)));
        } else {
            out.push(joint(i - 1, i, pts[i]));
        }
    }
    out
}

/// A segment offset by `dist` to the left of a-to-b.
pub fn offset_segment(a: Point, b: Point, dist: f64) -> (Point, Point) {
    let k = b.sub(a).normalized().perp().scale(dist);
    (a.add(k), b.add(k))
}

/// Trims segment `seg` at the nearest cutters on both sides of `pick`,
/// removing the piece the pick is on. Returns the pieces that remain (zero,
/// one or two); `None` when no cutter crosses the segment.
pub fn trim_segment(
    seg: (Point, Point),
    pick: Point,
    cutters: &[(Point, Point)],
) -> Option<Vec<(Point, Point)>> {
    let mut ts: Vec<f64> = cutters
        .iter()
        .filter_map(|c| segment_intersection(seg.0, seg.1, c.0, c.1))
        .map(|(t, _)| t)
        .filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6)
        .collect();
    if ts.is_empty() {
        return None;
    }
    ts.sort_by(f64::total_cmp);
    let (tp, _) = crate::geometry::project_on_segment(pick, seg.0, seg.1);
    let lo = ts.iter().copied().filter(|t| *t <= tp).fold(0.0, f64::max);
    let hi = ts.iter().copied().filter(|t| *t >= tp).fold(1.0, f64::min);
    let mut out = Vec::new();
    if lo > 1e-6 {
        out.push((seg.0, Point::lerp(seg.0, seg.1, lo)));
    }
    if hi < 1.0 - 1e-6 {
        out.push((Point::lerp(seg.0, seg.1, hi), seg.1));
    }
    Some(out)
}

/// Extends the end of `seg` nearer `pick` along its direction until it meets
/// the nearest boundary segment. `None` when nothing is in the way.
pub fn extend_segment(
    seg: (Point, Point),
    pick: Point,
    boundaries: &[(Point, Point)],
) -> Option<(Point, Point)> {
    let at_end = pick.dist(seg.1) <= pick.dist(seg.0);
    let (from, to) = if at_end {
        (seg.0, seg.1)
    } else {
        (seg.1, seg.0)
    };
    let dir = to.sub(from);
    if dir.length() < EPS {
        return None;
    }
    let far = to.add(dir.normalized().scale(1.0e7));
    let best = boundaries
        .iter()
        .filter_map(|b| segment_intersection(to, far, b.0, b.1))
        .map(|(t, _)| t)
        .filter(|t| *t > 1e-9)
        .fold(f64::INFINITY, f64::min);
    if !best.is_finite() {
        return None;
    }
    let hit = Point::lerp(to, far, best);
    Some(if at_end { (from, hit) } else { (hit, from) })
}

/// Splits segment `a b` at the point of it nearest `p`.
pub fn break_segment(a: Point, b: Point, p: Point) -> [(Point, Point); 2] {
    let (_, q) = crate::geometry::project_on_segment(p, a, b);
    [(a, q), (q, b)]
}

/// Breaks a polyline at the point of it nearest `p`. An open polyline gives
/// two open pieces; a closed one gives a single open polyline that starts and
/// ends at the break. `None` when the polyline has fewer than two points.
pub fn break_polyline(points: &[Point], closed: bool, p: Point) -> Option<Vec<Vec<Point>>> {
    let n = points.len();
    if n < 2 {
        return None;
    }
    let segs = if closed { n } else { n - 1 };
    let (i, q) = (0..segs)
        .map(|i| {
            (
                i,
                crate::geometry::project_on_segment(p, points[i], points[(i + 1) % n]).1,
            )
        })
        .min_by(|x, y| x.1.dist(p).total_cmp(&y.1.dist(p)))?;
    let mut first: Vec<Point> = points[..=i].to_vec();
    first.push(q);
    let mut second = vec![q];
    second.extend_from_slice(&points[i + 1..]);
    if closed {
        // Walk from the break around the ring back to it.
        let mut ring = vec![q];
        ring.extend_from_slice(&points[i + 1..]);
        ring.extend_from_slice(&points[..=i]);
        ring.push(q);
        return Some(vec![ring]);
    }
    Some(vec![first, second])
}

/// Reverses the direction of a line or polyline; arcs, circles and text have
/// no direction. Returns whether anything changed.
pub fn reverse_item(item: &mut CadItem) -> bool {
    match item {
        CadItem::Line { a, b } => {
            std::mem::swap(a, b);
            true
        }
        CadItem::Polyline { points, .. } if points.len() > 1 => {
            points.reverse();
            true
        }
        _ => false,
    }
}

/// `seg` turned about its first point to run parallel to `reference`, keeping
/// its length and the sense it already had.
pub fn make_parallel(seg: (Point, Point), reference: (Point, Point)) -> (Point, Point) {
    let r = reference.1.sub(reference.0).normalized();
    let d = seg.1.sub(seg.0);
    let len = d.length();
    let dir = if d.dot(r) >= 0.0 { r } else { -r };
    (seg.0, seg.0.add(dir.scale(len)))
}

/// `seg` turned about its first point to run perpendicular to `reference`,
/// keeping its length and the side it already pointed to.
pub fn make_perpendicular(seg: (Point, Point), reference: (Point, Point)) -> (Point, Point) {
    let r = reference.1.sub(reference.0).normalized().perp();
    let d = seg.1.sub(seg.0);
    let len = d.length();
    let dir = if d.dot(r) >= 0.0 { r } else { -r };
    (seg.0, seg.0.add(dir.scale(len)))
}

// ===== splines =====

/// A smooth curve through `pts` as joined cubic Beziers (tangents from the
/// neighbouring points, `tension` 0.5 is Catmull-Rom like), sampled with
/// `per_span` points per span. The result has `(n - 1) * per_span + 1` points
/// (`n * per_span` when closed, the start not repeated).
pub fn bezier_spline(pts: &[Point], closed: bool, per_span: usize, tension: f64) -> Vec<Point> {
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
    let k = tension.clamp(0.0, 1.0) / 3.0 * 2.0;
    let mut out = Vec::with_capacity(spans * per_span + 1);
    for s in 0..spans as isize {
        let (p0, p1, p2, p3) = (at(s - 1), at(s), at(s + 1), at(s + 2));
        let c1 = p1.add(p2.sub(p0).scale(k / 2.0));
        let c2 = p2.sub(p3.sub(p1).scale(k / 2.0));
        for j in 0..per_span {
            let t = j as f64 / per_span as f64;
            out.push(bezier_point(p1, c1, c2, p2, t));
        }
    }
    if !closed {
        out.push(pts[n - 1]);
    }
    out
}

/// Point `t` (0..=1) on the cubic Bezier `p0 c1 c2 p1`.
pub fn bezier_point(p0: Point, c1: Point, c2: Point, p1: Point, t: f64) -> Point {
    let u = 1.0 - t;
    p0.scale(u * u * u)
        .add(c1.scale(3.0 * u * u * t))
        .add(c2.scale(3.0 * u * t * t))
        .add(p1.scale(t * t * t))
}

// ===== boxes =====

/// The four corners of a box whose first edge is `a b` and whose depth is the
/// signed distance of `depth_pt` from that edge (counter-clockwise when
/// `depth_pt` is on the left). Empty when the box would have no area.
pub fn oriented_box(a: Point, b: Point, depth_pt: Point) -> Vec<Point> {
    let dir = b.sub(a).normalized();
    let depth = depth_pt.sub(a).dot(dir.perp());
    if a.dist(b) < EPS || depth.abs() < EPS {
        return Vec::new();
    }
    let k = dir.perp().scale(depth);
    vec![a, b, b.add(k), a.add(k)]
}

/// A box outline with both diagonals (Cross Box).
pub fn cross_box_items(corners: &[Point]) -> Vec<CadItem> {
    if corners.len() != 4 {
        return Vec::new();
    }
    vec![
        CadItem::Polyline {
            points: corners.to_vec(),
            closed: true,
        },
        CadItem::Line {
            a: corners[0],
            b: corners[2],
        },
        CadItem::Line {
            a: corners[1],
            b: corners[3],
        },
    ]
}

/// A box outline with one diagonal (Blocking Box).
pub fn blocking_box_items(corners: &[Point]) -> Vec<CadItem> {
    if corners.len() != 4 {
        return Vec::new();
    }
    vec![
        CadItem::Polyline {
            points: corners.to_vec(),
            closed: true,
        },
        CadItem::Line {
            a: corners[0],
            b: corners[2],
        },
    ]
}

/// A box outline filled with a batt insulation wave that runs along its first
/// edge and swings across the depth.
pub fn insulation_items(corners: &[Point]) -> Vec<CadItem> {
    if corners.len() != 4 {
        return Vec::new();
    }
    let along = corners[1].sub(corners[0]);
    let across = corners[3].sub(corners[0]);
    let (len, depth) = (along.length(), across.length());
    if len < EPS || depth < EPS {
        return Vec::new();
    }
    // About one wave per box depth, 12 samples per wave.
    let waves = (len / depth).round().max(1.0);
    let samples = (waves * 12.0) as usize;
    let wave: Vec<Point> = (0..=samples)
        .map(|k| {
            let s = k as f64 / samples as f64;
            let swing = 0.5 + 0.5 * (s * waves * TAU).sin();
            corners[0]
                .add(along.scale(s))
                .add(across.scale(0.1 + 0.8 * swing))
        })
        .collect();
    vec![
        CadItem::Polyline {
            points: corners.to_vec(),
            closed: true,
        },
        CadItem::Polyline {
            points: wave,
            closed: false,
        },
    ]
}

// ===== moving and copying items =====

/// `item` moved by `d`.
pub fn translated(item: &CadItem, d: Point) -> CadItem {
    let mut it = item.clone();
    match &mut it {
        CadItem::Line { a, b } => {
            *a = *a + d;
            *b = *b + d;
        }
        CadItem::Arc { center, .. } | CadItem::Circle { center, .. } => *center = *center + d,
        CadItem::Polyline { points, .. } => points.iter_mut().for_each(|p| *p = *p + d),
        CadItem::Text { pos, .. } => *pos = *pos + d,
    }
    it
}

/// Converts a polyline into its separate line segments.
pub fn polyline_to_lines(points: &[Point], closed: bool) -> Vec<CadItem> {
    let n = points.len();
    if n < 2 {
        return Vec::new();
    }
    let segs = if closed && n > 2 { n } else { n - 1 };
    (0..segs)
        .map(|i| CadItem::Line {
            a: points[i],
            b: points[(i + 1) % n],
        })
        .collect()
}

/// Joins line segments that share end points (within `tol`) into polylines;
/// a chain that returns to its start is closed. Segments that touch nothing
/// stay two-point polylines.
pub fn lines_to_polylines(lines: &[(Point, Point)], tol: f64) -> Vec<(Vec<Point>, bool)> {
    let mut left: Vec<(Point, Point)> = lines.to_vec();
    let mut out = Vec::new();
    while let Some(seed) = left.pop() {
        let mut chain = vec![seed.0, seed.1];
        loop {
            let tail = chain[chain.len() - 1];
            let head = chain[0];
            if chain.len() > 2 && tail.dist(head) <= tol {
                break;
            }
            let next = left
                .iter()
                .position(|s| s.0.dist(tail) <= tol || s.1.dist(tail) <= tol);
            if let Some(i) = next {
                let s = left.remove(i);
                chain.push(if s.0.dist(tail) <= tol { s.1 } else { s.0 });
                continue;
            }
            let prev = left
                .iter()
                .position(|s| s.0.dist(head) <= tol || s.1.dist(head) <= tol);
            if let Some(i) = prev {
                let s = left.remove(i);
                chain.insert(0, if s.0.dist(head) <= tol { s.1 } else { s.0 });
                continue;
            }
            break;
        }
        let closed = chain.len() > 3 && chain[0].dist(chain[chain.len() - 1]) <= tol;
        if closed {
            chain.pop();
        }
        out.push((chain, closed));
    }
    out
}

// ===== per-object attributes (the style tabs of the specification dialogs) =====
//
// The extras of a CAD object live in typed slots: [`Floor::cad_attrs`] (one
// [`CadAttrs`] per styled object) and [`Floor::cad_blocks`] (one
// [`CadBlockInfo`] per CAD block). Older files kept them as tagged text
// records on a hidden "CAD, Data" layer; [`migrate_legacy`] moves those into
// the slots once on load.

/// The hidden layer older files kept attribute, block and settings records on.
const LEGACY_DATA_LAYER: &str = "CAD, Data";
const ATTR_TAG: &str = "cad-attrs:";
const BLOCK_TAG: &str = "cad-block:";
const BLOB_TAG: &str = "cad-blob:";

/// How a line ends (the Arrow tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArrowStyle {
    #[default]
    None,
    Open,
    Filled,
    Tick,
    Dot,
}

impl ArrowStyle {
    pub const ALL: [ArrowStyle; 5] = [
        ArrowStyle::None,
        ArrowStyle::Open,
        ArrowStyle::Filled,
        ArrowStyle::Tick,
        ArrowStyle::Dot,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ArrowStyle::None => "None",
            ArrowStyle::Open => "Open Arrow",
            ArrowStyle::Filled => "Filled Arrow",
            ArrowStyle::Tick => "Tick",
            ArrowStyle::Dot => "Dot",
        }
    }
}

/// The fill of a closed shape (the Fill Style tab). An empty `pattern` is a
/// solid fill of `color`; otherwise the named hatch pattern drawn in `color`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FillAttr {
    pub color: [u8; 3],
    /// 255 is opaque.
    pub opacity: u8,
    pub pattern: String,
    /// Hatch line spacing, inches.
    pub spacing: f64,
    pub angle_deg: f64,
    /// The CAD lines a Hatch drew for this fill (so a new hatch can replace
    /// them).
    pub lines: Vec<Id>,
}

impl Default for FillAttr {
    fn default() -> Self {
        Self {
            color: [200, 200, 200],
            opacity: 255,
            pattern: String::new(),
            spacing: 6.0,
            angle_deg: 45.0,
            lines: Vec::new(),
        }
    }
}

/// The extras of one CAD object. Every field at its default means "as the
/// layer says" and the record is dropped.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CadAttrs {
    /// The CAD object these belong to.
    pub target: Id,
    pub color: Option<[u8; 3]>,
    /// Plotted weight, hundredths of a millimetre.
    pub weight: Option<u32>,
    pub dash: Option<LineStyle>,
    pub fill: Option<FillAttr>,
    pub arrow_start: ArrowStyle,
    pub arrow_end: ArrowStyle,
    /// Arrow size in plan inches; zero is the default size.
    pub arrow_size: f64,
    /// A named text style; `None` is the layer's.
    pub text_style: Option<String>,
    /// Rich text runs; their plain text is the text item's `text`.
    pub runs: Vec<RichRun>,
    /// Wrap width, alignment, border and background of a text (TXT-1,
    /// TXT-3, TXT-16); plain unless the user made a box.
    pub text_box: crate::text_box::TextBox,
    /// Polyline edges drawn as arcs (CAD-22): the polyline's points between
    /// the ends of each span are samples along the arc, so every consumer
    /// (drawing, DXF, layout) sees a smooth curve in plain points. Empty for
    /// anything but a polyline with an arc edge.
    pub arc_edges: Vec<PolyArc>,
    /// Live length / angle / radius labels (CAD-118).
    pub labels: CadLabels,
    /// Polyline edges that are not drawn (Hide Selected Edge, CAD-117): the
    /// numbers of the control-vertex edges, edge `i` running from vertex `i`
    /// to the next.
    pub hidden_edges: Vec<usize>,
}

/// Which live labels a CAD line, arc or polyline shows (CAD-118): the length
/// centered above and the angle (or the radius of an arc) centered below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CadLabels {
    pub show_length: bool,
    pub show_angle: bool,
    /// An arc shows its radius under its length.
    pub show_radius: bool,
    /// Polyline: label the angle of every edge, not only the selected one.
    pub all_angles: bool,
    /// Show the angle the other way round (the line read from its far end).
    pub reverse_angle: bool,
}

impl CadLabels {
    pub fn any(&self) -> bool {
        self.show_length || self.show_angle || self.show_radius
    }
}

/// One arc edge of a polyline: `points[from..=to]` sample an arc that leaves
/// `points[from]` and reaches `points[to]` (an index equal to the number of
/// points means the first point, on a closed polyline).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PolyArc {
    pub from: usize,
    pub to: usize,
    /// `tan(sweep / 4)`, positive when the edge turns counter-clockwise
    /// from `from` to `to`.
    pub bulge: f64,
}

impl CadAttrs {
    pub fn new(target: Id) -> Self {
        Self {
            target,
            ..Self::default()
        }
    }

    /// Nothing differs from the layer's defaults.
    pub fn is_default(&self) -> bool {
        let blank = CadAttrs::new(self.target);
        *self == blank
    }
}

/// A CAD block: a group of CAD objects with a name and the points that place
/// it and trim arrows (CAD-31, CAD-32).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadBlockInfo {
    /// The id of the block's object group.
    pub group: Id,
    pub name: String,
    /// The point that lands on the cursor when the block is inserted.
    pub insertion: Option<Point>,
    /// Where an arrow line that ends on the block is trimmed back to.
    pub backoff: Option<Point>,
}

fn legacy_tagged<'a>(o: &'a CadObject, tag: &str) -> Option<&'a str> {
    if o.layer != LEGACY_DATA_LAYER {
        return None;
    }
    match &o.item {
        CadItem::Text { text, .. } => text.strip_prefix(tag),
        _ => None,
    }
}

impl crate::model::Floor {
    /// The attributes of CAD object `id`, if it has any.
    pub fn cad_attrs(&self, id: Id) -> Option<CadAttrs> {
        self.cad_attrs.iter().find(|a| a.target == id).cloned()
    }

    /// Every attribute entry of the floor, by target (for the renderer).
    pub fn cad_attr_map(&self) -> HashMap<Id, CadAttrs> {
        self.cad_attrs
            .iter()
            .map(|a| (a.target, a.clone()))
            .collect()
    }

    /// The blocks of the floor whose group still exists.
    pub fn cad_blocks(&self) -> Vec<CadBlockInfo> {
        self.cad_blocks
            .iter()
            .filter(|b| self.groups.iter().any(|g| g.id == b.group))
            .cloned()
            .collect()
    }

    pub fn cad_block(&self, group: Id) -> Option<CadBlockInfo> {
        self.cad_blocks().into_iter().find(|b| b.group == group)
    }

    /// The CAD objects of group `group`, in group order.
    pub fn group_members_cad(&self, group: Id) -> Vec<Id> {
        self.groups
            .iter()
            .find(|g| g.id == group)
            .into_iter()
            .flat_map(|g| g.members.iter())
            .filter_map(|m| match m {
                crate::groups::ObjectRef::Cad(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// The items of the CAD objects of group `group`.
    pub fn group_items(&self, group: Id) -> Vec<CadItem> {
        self.group_members_cad(group)
            .into_iter()
            .filter_map(|id| self.cad.iter().find(|o| o.id == id))
            .map(|o| o.item.clone())
            .collect()
    }

    /// Bounds of the CAD objects of group `group`.
    pub fn block_bounds(&self, group: Id) -> (Point, Point) {
        block_bounds(self, group)
    }
}

impl Project {
    /// Sets the attributes of a CAD object; default attributes remove the entry.
    pub fn set_cad_attrs(&mut self, floor: usize, attrs: CadAttrs) {
        let Some(f) = self.floors.get_mut(floor) else {
            return;
        };
        let at = f.cad_attrs.iter().position(|a| a.target == attrs.target);
        match (at, attrs.is_default()) {
            (Some(i), true) => {
                f.cad_attrs.remove(i);
            }
            (Some(i), false) => f.cad_attrs[i] = attrs,
            (None, false) => f.cad_attrs.push(attrs),
            (None, true) => {}
        }
    }

    /// Edits the attributes of `target` (starting from the defaults).
    pub fn edit_cad_attrs(&mut self, floor: usize, target: Id, edit: impl FnOnce(&mut CadAttrs)) {
        let mut a = self
            .floors
            .get(floor)
            .and_then(|f| f.cad_attrs(target))
            .unwrap_or_else(|| CadAttrs::new(target));
        edit(&mut a);
        self.set_cad_attrs(floor, a);
    }

    /// Removes attribute and block entries whose object (or block group) no
    /// longer exists. Returns how many were removed.
    pub fn prune_cad_data(&mut self, floor: usize) -> usize {
        let cameras: std::collections::HashSet<Id> = self.cameras.iter().map(|c| c.id).collect();
        let Some(f) = self.floors.get_mut(floor) else {
            return 0;
        };
        let live: std::collections::HashSet<Id> = f.cad.iter().map(|o| o.id).collect();
        let groups: std::collections::HashSet<Id> = f.groups.iter().map(|g| g.id).collect();
        let before = f.cad_attrs.len() + f.cad_blocks.len();
        f.cad_attrs.retain(|a| live.contains(&a.target));
        // A block goes with its group, or when none of its objects is left.
        let members: Vec<(Id, Vec<Id>)> = f
            .cad_blocks
            .iter()
            .map(|b| (b.group, f.group_members_cad(b.group)))
            .collect();
        f.cad_blocks.retain(|b| {
            groups.contains(&b.group)
                && members
                    .iter()
                    .find(|(g, _)| *g == b.group)
                    .is_some_and(|(_, m)| m.iter().any(|id| live.contains(id)))
        });
        let removed = before - f.cad_attrs.len() - f.cad_blocks.len();
        // A group none of whose objects is left (every member deleted) goes.
        use crate::groups::ObjectRef as R;
        let alive = |m: &R| match *m {
            R::Wall(id) => f.walls.iter().any(|w| w.id == id),
            R::Opening(id) => f.openings.iter().any(|o| o.id == id),
            R::Dimension(id) => f.dimensions.iter().any(|d| d.id == id),
            R::Cad(id) => live.contains(&id),
            R::Symbol(id) => f.symbols.iter().any(|s| s.id == id),
            R::Camera(id) => cameras.contains(&id),
            // Kinds kept as opaque records: the editor prunes those groups.
            _ => true,
        };
        let keep: Vec<bool> = f
            .groups
            .iter()
            .map(|g| g.members.iter().any(&alive))
            .collect();
        let mut it = keep.into_iter();
        f.groups.retain(|_| it.next().unwrap_or(true));
        removed
    }

    // ----- CAD blocks -----

    fn write_block(&mut self, floor: usize, info: &CadBlockInfo) {
        let blocks = &mut self.floors[floor].cad_blocks;
        match blocks.iter().position(|b| b.group == info.group) {
            Some(i) => blocks[i] = info.clone(),
            None => blocks.push(info.clone()),
        }
    }

    fn drop_block_record(&mut self, floor: usize, group: Id) {
        self.floors[floor].cad_blocks.retain(|b| b.group != group);
    }

    /// Makes a CAD block of the given CAD objects (two or more). The name
    /// defaults to "CAD Block n"; the insertion point to the middle of the
    /// block. Returns the block's group id.
    pub fn make_cad_block(&mut self, floor: usize, ids: &[Id], name: Option<&str>) -> Option<Id> {
        if floor >= self.floors.len() {
            return None;
        }
        let members: Vec<crate::groups::ObjectRef> = ids
            .iter()
            .filter(|id| self.floors[floor].cad.iter().any(|o| o.id == **id))
            .map(|id| crate::groups::ObjectRef::Cad(*id))
            .collect();
        let group = self.make_group(floor, &members)?;
        let (lo, hi) = block_bounds(&self.floors[floor], group);
        let name = match name {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => {
                let n = self.floors[floor].cad_blocks().len() + 1;
                format!("CAD Block {n}")
            }
        };
        self.write_block(
            floor,
            &CadBlockInfo {
                group,
                name,
                insertion: Some(Point::lerp(lo, hi, 0.5)),
                backoff: None,
            },
        );
        Some(group)
    }

    /// Explodes a CAD block back into separate CAD objects. Returns their ids.
    pub fn explode_cad_block(&mut self, floor: usize, group: Id) -> Option<Vec<Id>> {
        if floor >= self.floors.len() || self.floors[floor].cad_block(group).is_none() {
            return None;
        }
        let members = self.explode_group(floor, group)?;
        self.drop_block_record(floor, group);
        Some(
            members
                .into_iter()
                .filter_map(|m| match m {
                    crate::groups::ObjectRef::Cad(id) => Some(id),
                    _ => None,
                })
                .collect(),
        )
    }

    /// Edits a block's name and points. Returns false for an unknown block.
    pub fn edit_cad_block(
        &mut self,
        floor: usize,
        group: Id,
        edit: impl FnOnce(&mut CadBlockInfo),
    ) -> bool {
        let Some(mut info) = self.floors.get(floor).and_then(|f| f.cad_block(group)) else {
            return false;
        };
        edit(&mut info);
        self.write_block(floor, &info);
        true
    }

    /// Inserts a copy of a block with its insertion point on `at`. The copy is
    /// a new block with the same name. Returns the new group id and the ids of
    /// its objects.
    pub fn insert_cad_block(
        &mut self,
        floor: usize,
        group: Id,
        at: Point,
    ) -> Option<(Id, Vec<Id>)> {
        let info = self.floors.get(floor)?.cad_block(group)?;
        let members: Vec<Id> = self.floors[floor]
            .groups
            .iter()
            .find(|g| g.id == group)?
            .members
            .iter()
            .filter_map(|m| match m {
                crate::groups::ObjectRef::Cad(id) => Some(*id),
                _ => None,
            })
            .collect();
        let (lo, hi) = block_bounds(&self.floors[floor], group);
        let from = info.insertion.unwrap_or_else(|| Point::lerp(lo, hi, 0.5));
        let d = at.sub(from);
        let copies: Vec<(String, CadItem)> = members
            .iter()
            .filter_map(|id| self.floors[floor].cad.iter().find(|o| o.id == *id))
            .map(|o| (o.layer.clone(), translated(&o.item, d)))
            .collect();
        let new_ids: Vec<Id> = copies
            .into_iter()
            .map(|(layer, item)| self.add_cad(floor, layer, item))
            .collect();
        let refs: Vec<crate::groups::ObjectRef> = new_ids
            .iter()
            .map(|id| crate::groups::ObjectRef::Cad(*id))
            .collect();
        let new_group = self.make_group(floor, &refs)?;
        self.write_block(
            floor,
            &CadBlockInfo {
                group: new_group,
                name: info.name,
                insertion: Some(at),
                backoff: info.backoff.map(|p| p.add(d)),
            },
        );
        Some((new_group, new_ids))
    }

    /// Deletes a block and all of its objects. Returns how many objects went.
    pub fn delete_cad_block(&mut self, floor: usize, group: Id) -> usize {
        let Some(g) = self
            .floors
            .get(floor)
            .and_then(|f| f.groups.iter().find(|g| g.id == group))
        else {
            return 0;
        };
        let ids: Vec<Id> = g
            .members
            .iter()
            .filter_map(|m| match m {
                crate::groups::ObjectRef::Cad(id) => Some(*id),
                _ => None,
            })
            .collect();
        let _ = self.explode_group(floor, group);
        self.drop_block_record(floor, group);
        for id in &ids {
            self.remove_cad(floor, *id);
        }
        self.prune_cad_data(floor);
        ids.len()
    }
}

/// Bounds of the CAD members of group `group`.
fn block_bounds(f: &crate::model::Floor, group: Id) -> (Point, Point) {
    let pts: Vec<Point> = f
        .groups
        .iter()
        .find(|g| g.id == group)
        .into_iter()
        .flat_map(|g| g.members.iter())
        .filter_map(|m| match m {
            crate::groups::ObjectRef::Cad(id) => f.cad.iter().find(|o| o.id == *id),
            _ => None,
        })
        .flat_map(|o| {
            let (lo, hi) = o.bounds();
            [lo, hi]
        })
        .collect();
    min_max(pts)
}

// ===== migration of the old hidden-layer records =====

/// Moves attribute, block and settings records kept the old way (tagged text
/// on the hidden "CAD, Data" layer of the floors' CAD lists) into
/// [`Floor::cad_attrs`], [`Floor::cad_blocks`], [`Project::text_macros`] and
/// [`Project::note_types`], removes the records and the layer. Slots that are
/// already filled win. Runs once when a project is loaded; returns whether
/// anything changed.
pub fn migrate_legacy(project: &mut Project) -> bool {
    let mut changed = false;
    let mut macros = None;
    let mut notes = None;
    for f in project.floors.iter_mut() {
        let n = f.cad.len();
        let mut attrs: Vec<CadAttrs> = Vec::new();
        let mut blocks: Vec<CadBlockInfo> = Vec::new();
        f.cad.retain(|o| {
            if o.layer != LEGACY_DATA_LAYER {
                return true;
            }
            if let Some(j) = legacy_tagged(o, ATTR_TAG) {
                if let Ok(a) = serde_json::from_str::<CadAttrs>(j) {
                    attrs.push(a);
                }
            } else if let Some(j) = legacy_tagged(o, BLOCK_TAG) {
                if let Ok(b) = serde_json::from_str::<CadBlockInfo>(j) {
                    blocks.push(b);
                }
            } else if let Some(rest) = legacy_tagged(o, BLOB_TAG) {
                if let Some(j) = rest.strip_prefix("text-macros=") {
                    macros = serde_json::from_str::<crate::text_styles::TextMacros>(j).ok();
                } else if let Some(j) = rest.strip_prefix("note-types=") {
                    notes = serde_json::from_str::<crate::text_styles::NoteTypes>(j).ok();
                }
            }
            false
        });
        for a in attrs {
            if !f.cad_attrs.iter().any(|x| x.target == a.target) {
                f.cad_attrs.push(a);
            }
        }
        for b in blocks {
            if !f.cad_blocks.iter().any(|x| x.group == b.group) {
                f.cad_blocks.push(b);
            }
        }
        changed |= f.cad.len() != n;
    }
    if let Some(m) = macros {
        if project.text_macros.macros.is_empty() {
            project.text_macros = m;
        }
    }
    if let Some(n) = notes {
        if project.note_types == crate::text_styles::NoteTypes::default() {
            project.note_types = n;
        }
    }
    let n = project.layers.layers.len();
    project
        .layers
        .layers
        .retain(|l| l.name != LEGACY_DATA_LAYER);
    changed |= project.layers.layers.len() != n;
    changed
}

// ===== CAD detail from a view =====

/// The lines of a floor as CAD objects for a CAD detail: wall outlines as
/// closed polylines on `wall_layer`, plus copies of the floor's visible CAD
/// items on their own layers (data records and hidden layers are skipped).
pub fn detail_items(
    floor: &crate::model::Floor,
    layers: &crate::layers::LayerSet,
    wall_layer: &str,
) -> Vec<(String, CadItem)> {
    let mut out = Vec::new();
    for w in &floor.walls {
        let poly = if w.is_curved() {
            w.plan_polygon()
        } else {
            w.footprint().to_vec()
        };
        if poly.len() >= 3 {
            out.push((
                wall_layer.to_string(),
                CadItem::Polyline {
                    points: poly,
                    closed: true,
                },
            ));
        }
    }
    for o in &floor.cad {
        if layers.is_visible(&o.layer) {
            out.push((o.layer.clone(), o.item.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        a.dist(b) < 1e-9
    }

    fn obj(item: CadItem) -> CadObject {
        CadObject {
            id: 1,
            layer: DEFAULT_CAD_LAYER.into(),
            item,
        }
    }

    #[test]
    fn line_and_circle_bounds() {
        let l = obj(CadItem::Line {
            a: Point::new(10.0, -5.0),
            b: Point::new(-3.0, 8.0),
        });
        let (lo, hi) = l.bounds();
        assert!(close(lo, Point::new(-3.0, -5.0)) && close(hi, Point::new(10.0, 8.0)));
        let c = obj(CadItem::Circle {
            center: Point::new(5.0, 5.0),
            radius: 2.0,
        });
        let (lo, hi) = c.bounds();
        assert!(close(lo, Point::new(3.0, 3.0)) && close(hi, Point::new(7.0, 7.0)));
    }

    #[test]
    fn arc_bounds_include_axis_extremes() {
        // Quarter arc 0..90 deg around origin, radius 10.
        let a = obj(CadItem::Arc {
            center: Point::ZERO,
            radius: 10.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        });
        let (lo, hi) = a.bounds();
        assert!(close(lo, Point::new(0.0, 0.0)) && close(hi, Point::new(10.0, 10.0)));
        // Arc crossing +X (350..10 deg) reaches x = 10 although endpoints don't.
        let b = obj(CadItem::Arc {
            center: Point::ZERO,
            radius: 10.0,
            start_angle: 350f64.to_radians(),
            end_angle: 10f64.to_radians(),
        });
        let (_, hi) = b.bounds();
        assert!((hi.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn polyline_and_text_bounds() {
        let p = obj(CadItem::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(4.0, 1.0),
                Point::new(2.0, 9.0),
            ],
            closed: true,
        });
        let (lo, hi) = p.bounds();
        assert!(close(lo, Point::ZERO) && close(hi, Point::new(4.0, 9.0)));
        let t = obj(CadItem::Text {
            pos: Point::new(10.0, 10.0),
            text: "ABCDE".into(),
            height: 5.0,
            angle: 0.0,
        });
        let (lo, hi) = t.bounds();
        assert!(close(lo, Point::new(10.0, 10.0)));
        assert!(close(
            hi,
            Point::new(10.0 + 5.0 * 5.0 * TEXT_WIDTH_FACTOR, 15.0)
        ));
        let empty = obj(CadItem::Polyline {
            points: vec![],
            closed: false,
        });
        assert!(close(empty.bounds().0, Point::ZERO));
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn fillet_rounds_a_right_corner_with_a_tangent_arc() {
        let f = fillet_corner(p(100.0, 0.0), p(0.0, 0.0), p(0.0, 100.0), 10.0).unwrap();
        assert!(close(f.t0, p(10.0, 0.0)) && close(f.t1, p(0.0, 10.0)));
        assert!(close(f.center, p(10.0, 10.0)));
        let CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } = f.arc
        else {
            panic!("an arc")
        };
        assert!(close(center, p(10.0, 10.0)) && (radius - 10.0).abs() < 1e-9);
        // The arc is the quarter between the tangent points (not the other 270).
        let sweep = (end_angle - start_angle).rem_euclid(TAU);
        assert!((sweep - FRAC_PI_2).abs() < 1e-9);
        // The arc passes the point nearest the corner.
        let mid = start_angle + sweep / 2.0;
        let on = p(center.x + 10.0 * mid.cos(), center.y + 10.0 * mid.sin());
        assert!((on.dist(p(0.0, 0.0)) - (10.0 * 2f64.sqrt() - 10.0)).abs() < 1e-9);
        // Too big a radius, no radius and collinear rays refuse.
        assert!(fillet_corner(p(5.0, 0.0), p(0.0, 0.0), p(0.0, 5.0), 10.0).is_none());
        assert!(fillet_corner(p(5.0, 0.0), p(0.0, 0.0), p(0.0, 5.0), 0.0).is_none());
        assert!(fillet_corner(p(5.0, 0.0), p(0.0, 0.0), p(-5.0, 0.0), 1.0).is_none());
    }

    #[test]
    fn fillet_lines_trims_and_extends_both_to_the_arc() {
        // Two lines that stop short of crossing at (50, 50)... they meet at
        // the origin when extended.
        let l0 = (p(100.0, 0.0), p(20.0, 0.0));
        let l1 = (p(0.0, 100.0), p(0.0, 30.0));
        let j = fillet_lines(l0, l1, 10.0).unwrap();
        assert!(close(j.first.0, p(100.0, 0.0)) && close(j.first.1, p(10.0, 0.0)));
        assert!(close(j.second.0, p(0.0, 100.0)) && close(j.second.1, p(0.0, 10.0)));
        assert!(matches!(j.joiner, Some(CadItem::Arc { .. })));
        // Radius zero makes a sharp corner.
        let sharp = fillet_lines(l0, l1, 0.0).unwrap();
        assert!(close(sharp.first.1, p(0.0, 0.0)) && close(sharp.second.1, p(0.0, 0.0)));
        assert!(sharp.joiner.is_none());
        // Parallel lines cannot be filleted.
        assert!(fillet_lines(
            (p(0.0, 0.0), p(10.0, 0.0)),
            (p(0.0, 5.0), p(10.0, 5.0)),
            1.0
        )
        .is_none());
    }

    #[test]
    fn chamfer_cuts_the_corner_by_the_two_distances() {
        let l0 = (p(100.0, 0.0), p(0.0, 0.0));
        let l1 = (p(0.0, 0.0), p(0.0, 100.0));
        let j = chamfer_lines(l0, l1, 10.0, 20.0).unwrap();
        assert!(close(j.first.1, p(10.0, 0.0)));
        assert!(close(j.second.0, p(0.0, 20.0)) && close(j.second.1, p(0.0, 100.0)));
        let Some(CadItem::Line { a, b }) = j.joiner else {
            panic!("a line")
        };
        assert!(close(a, p(10.0, 0.0)) && close(b, p(0.0, 20.0)));
        assert!((a.dist(b) - (500f64).sqrt()).abs() < 1e-9);
        assert!(chamfer_lines(l0, l1, 200.0, 1.0).is_none());
        // On a polyline vertex: fillet and chamfer replace the vertex.
        let sq = [p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)];
        let ch = chamfer_polyline_vertex(&sq, true, 1, 10.0, 10.0).unwrap();
        assert_eq!(ch.len(), 5);
        assert!(close(ch[1], p(90.0, 0.0)) && close(ch[2], p(100.0, 10.0)));
        let fi = fillet_polyline_vertex(&sq, true, 2, 20.0, 15.0).unwrap();
        assert!(fi.len() > 5);
        assert!(close(fi[2], p(100.0, 80.0)));
        assert!(fi.iter().all(|q| q.dist(p(100.0, 100.0)) > 5.0));
        // Open polyline ends have no corner to round.
        assert!(fillet_polyline_vertex(&sq, false, 0, 5.0, 15.0).is_none());
        assert!(fillet_polyline_vertex(&sq, false, 3, 5.0, 15.0).is_none());
    }

    #[test]
    fn offsets_trims_extends_and_breaks() {
        // A closed square offset to its left (inward when counter-clockwise).
        let sq = [p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)];
        let inner = offset_polyline(&sq, true, 10.0);
        assert!(close(inner[0], p(10.0, 10.0)) && close(inner[2], p(90.0, 90.0)));
        let outer = offset_polyline(&sq, true, -10.0);
        assert!(close(outer[0], p(-10.0, -10.0)));
        let open = offset_polyline(&[p(0.0, 0.0), p(100.0, 0.0), p(100.0, 50.0)], false, 5.0);
        assert!(
            close(open[0], p(0.0, 5.0))
                && close(open[1], p(95.0, 5.0))
                && close(open[2], p(95.0, 50.0))
        );
        assert_eq!(
            offset_segment(p(0.0, 0.0), p(10.0, 0.0), 3.0),
            (p(0.0, 3.0), p(10.0, 3.0))
        );
        assert!(offset_polyline(&[p(1.0, 1.0)], false, 3.0).is_empty());

        // Trim: remove the middle piece between two cutters.
        let seg = (p(0.0, 0.0), p(100.0, 0.0));
        let cutters = [
            (p(30.0, -10.0), p(30.0, 10.0)),
            (p(70.0, -10.0), p(70.0, 10.0)),
        ];
        let r = trim_segment(seg, p(50.0, 0.0), &cutters).unwrap();
        assert_eq!(r.len(), 2);
        assert!(close(r[0].1, p(30.0, 0.0)) && close(r[1].0, p(70.0, 0.0)));
        // Picking the end piece leaves the rest.
        let r = trim_segment(seg, p(90.0, 0.0), &cutters).unwrap();
        assert_eq!(r.len(), 1);
        assert!(close(r[0].1, p(70.0, 0.0)));
        assert!(trim_segment(seg, p(50.0, 0.0), &[(p(0.0, 5.0), p(100.0, 5.0))]).is_none());

        // Extend the end nearer the pick to the boundary.
        let e = extend_segment(
            (p(0.0, 0.0), p(40.0, 0.0)),
            p(35.0, 0.0),
            &[(p(90.0, -5.0), p(90.0, 5.0))],
        )
        .unwrap();
        assert!(close(e.0, p(0.0, 0.0)) && close(e.1, p(90.0, 0.0)));
        let e = extend_segment(
            (p(0.0, 0.0), p(40.0, 0.0)),
            p(2.0, 0.0),
            &[(p(-20.0, -5.0), p(-20.0, 5.0))],
        )
        .unwrap();
        assert!(close(e.0, p(-20.0, 0.0)) && close(e.1, p(40.0, 0.0)));
        assert!(extend_segment((p(0.0, 0.0), p(40.0, 0.0)), p(35.0, 0.0), &[]).is_none());

        // Break: a line and a polyline.
        let [a, b] = break_segment(p(0.0, 0.0), p(100.0, 0.0), p(40.0, 9.0));
        assert!(close(a.1, p(40.0, 0.0)) && close(b.0, p(40.0, 0.0)));
        let two = break_polyline(
            &[p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0)],
            false,
            p(100.0, 60.0),
        )
        .unwrap();
        assert_eq!(two.len(), 2);
        assert_eq!(two[0].len(), 3);
        assert!(close(two[0][2], p(100.0, 60.0)) && close(two[1][0], p(100.0, 60.0)));
        let ring = break_polyline(&sq, true, p(50.0, 0.0)).unwrap();
        assert_eq!(ring.len(), 1);
        assert!(close(ring[0][0], p(50.0, 0.0)) && close(*ring[0].last().unwrap(), p(50.0, 0.0)));
        assert_eq!(ring[0].len(), 6);
    }

    #[test]
    fn reverse_parallel_and_perpendicular() {
        let mut l = CadItem::Line {
            a: p(0.0, 0.0),
            b: p(10.0, 5.0),
        };
        assert!(reverse_item(&mut l));
        assert_eq!(
            l,
            CadItem::Line {
                a: p(10.0, 5.0),
                b: p(0.0, 0.0)
            }
        );
        let mut pl = CadItem::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
            closed: false,
        };
        assert!(reverse_item(&mut pl));
        assert!(matches!(&pl, CadItem::Polyline { points, .. } if points[0] == p(1.0, 1.0)));
        assert!(!reverse_item(&mut CadItem::Circle {
            center: Point::ZERO,
            radius: 1.0
        }));
        // Parallel keeps the length and the sense.
        let r = make_parallel((p(0.0, 0.0), p(30.0, 40.0)), (p(0.0, 0.0), p(100.0, 0.0)));
        assert!(close(r.1, p(50.0, 0.0)));
        let r = make_parallel((p(0.0, 0.0), p(-30.0, 40.0)), (p(0.0, 0.0), p(100.0, 0.0)));
        assert!(close(r.1, p(-50.0, 0.0)));
        let r = make_perpendicular((p(0.0, 0.0), p(30.0, 40.0)), (p(0.0, 0.0), p(100.0, 0.0)));
        assert!(close(r.1, p(0.0, 50.0)));
        let r = make_perpendicular(
            (p(10.0, 10.0), p(10.0, -30.0)),
            (p(0.0, 0.0), p(100.0, 0.0)),
        );
        assert!(close(r.1, p(10.0, -30.0)));
    }

    #[test]
    fn spline_point_counts() {
        let pts = [p(0.0, 0.0), p(100.0, 50.0), p(200.0, 0.0), p(300.0, 50.0)];
        let open = bezier_spline(&pts, false, 8, 0.5);
        assert_eq!(open.len(), 3 * 8 + 1);
        assert!(close(open[0], pts[0]) && close(*open.last().unwrap(), pts[3]));
        // It passes through every control point.
        for (k, q) in pts.iter().enumerate() {
            assert!(close(open[k * 8], *q));
        }
        let closed = bezier_spline(&pts, true, 8, 0.5);
        assert_eq!(closed.len(), 4 * 8);
        assert_eq!(bezier_spline(&pts[..1], false, 8, 0.5).len(), 1);
        // Tension zero is the polyline itself (straight spans).
        let flat = bezier_spline(&pts, false, 4, 0.0);
        assert!(close(flat[2], Point::lerp(pts[0], pts[1], 0.5)));
    }

    #[test]
    fn boxes_and_insulation() {
        let c = oriented_box(p(0.0, 0.0), p(100.0, 0.0), p(40.0, 30.0));
        assert_eq!(
            c,
            vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 30.0), p(0.0, 30.0)]
        );
        let below = oriented_box(p(0.0, 0.0), p(100.0, 0.0), p(40.0, -30.0));
        assert!(close(below[2], p(100.0, -30.0)));
        assert!(oriented_box(p(0.0, 0.0), p(100.0, 0.0), p(40.0, 0.0)).is_empty());
        assert_eq!(cross_box_items(&c).len(), 3);
        assert_eq!(blocking_box_items(&c).len(), 2);
        let ins = insulation_items(&c);
        assert_eq!(ins.len(), 2);
        let CadItem::Polyline { points, closed } = &ins[1] else {
            panic!("a wave")
        };
        assert!(!closed && points.len() > 20);
        let (lo, hi) = ins[1].bounds();
        assert!(lo.x >= -1e-9 && hi.x <= 100.0 + 1e-9 && lo.y >= 0.0 && hi.y <= 30.0);
        assert!(cross_box_items(&[]).is_empty() && insulation_items(&[]).is_empty());
    }

    #[test]
    fn lines_and_polylines_convert_both_ways() {
        let sq = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
        let lines = polyline_to_lines(&sq, true);
        assert_eq!(lines.len(), 4);
        assert_eq!(polyline_to_lines(&sq, false).len(), 3);
        let segs: Vec<(Point, Point)> = lines
            .iter()
            .map(|l| match l {
                CadItem::Line { a, b } => (*a, *b),
                _ => unreachable!(),
            })
            .collect();
        let chains = lines_to_polylines(&segs, 1e-6);
        assert_eq!(chains.len(), 1);
        assert!(chains[0].1 && chains[0].0.len() == 4);
        let apart = lines_to_polylines(
            &[(p(0.0, 0.0), p(1.0, 0.0)), (p(5.0, 5.0), p(6.0, 5.0))],
            1e-6,
        );
        assert_eq!(apart.len(), 2);
        assert!(apart.iter().all(|(pts, closed)| pts.len() == 2 && !closed));
    }

    fn project_with_lines(n: usize) -> (Project, Vec<Id>) {
        let mut pr = Project::new("T");
        let ids = (0..n)
            .map(|i| {
                pr.add_cad(
                    0,
                    DEFAULT_CAD_LAYER,
                    CadItem::Line {
                        a: p(i as f64 * 10.0, 0.0),
                        b: p(i as f64 * 10.0, 10.0),
                    },
                )
            })
            .collect();
        (pr, ids)
    }

    #[test]
    fn cad_block_create_insert_and_explode_round_trip() {
        let (mut pr, ids) = project_with_lines(3);
        assert!(
            pr.make_cad_block(0, &ids[..1], None).is_none(),
            "one object is no block"
        );
        let g = pr.make_cad_block(0, &ids, Some("Chair")).unwrap();
        let blocks = pr.floors[0].cad_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "Chair");
        assert_eq!(blocks[0].group, g);
        // Insertion defaults to the middle of the block's bounds.
        assert!(close(blocks[0].insertion.unwrap(), p(10.0, 5.0)));
        assert!(pr.floors[0]
            .groups
            .iter()
            .any(|x| x.id == g && x.members.len() == 3));
        // The block is a typed slot entry; no hidden layer or record exists.
        assert_eq!(pr.floors[0].cad_blocks.len(), 1);
        assert!(pr.layers.get("CAD, Data").is_none());
        assert!(pr.floors[0].cad.iter().all(|o| o.layer != "CAD, Data"));

        assert!(pr.edit_cad_block(0, g, |b| {
            b.insertion = Some(p(0.0, 0.0));
            b.backoff = Some(p(5.0, 5.0));
        }));
        let (g2, new_ids) = pr.insert_cad_block(0, g, p(100.0, 100.0)).unwrap();
        assert_ne!(g2, g);
        assert_eq!(new_ids.len(), 3);
        let first = pr.floors[0]
            .cad
            .iter()
            .find(|o| o.id == new_ids[0])
            .unwrap();
        assert!(close(first.bounds().0, p(100.0, 100.0)));
        let info2 = pr.floors[0].cad_block(g2).unwrap();
        assert_eq!(info2.name, "Chair");
        assert!(close(info2.backoff.unwrap(), p(105.0, 105.0)));
        assert_eq!(pr.floors[0].cad_blocks().len(), 2);

        // Explode the first: its objects remain, ungrouped, and the record is gone.
        let freed = pr.explode_cad_block(0, g).unwrap();
        assert_eq!(freed, ids);
        assert!(pr.floors[0].cad_block(g).is_none());
        assert!(pr.floors[0].groups.iter().all(|x| x.id != g));
        for id in &ids {
            assert!(pr.floors[0].cad.iter().any(|o| o.id == *id));
        }
        assert!(pr.explode_cad_block(0, g).is_none(), "already exploded");
        // Deleting the second block removes its objects.
        assert_eq!(pr.delete_cad_block(0, g2), 3);
        assert!(pr.floors[0].cad_blocks().is_empty());
        let json = serde_json::to_string(&pr).unwrap();
        assert!(serde_json::from_str::<Project>(&json).is_ok());
    }

    #[test]
    fn attrs_round_trip_and_default_removes_the_record() {
        let (mut pr, ids) = project_with_lines(2);
        assert!(pr.floors[0].cad_attrs(ids[0]).is_none());
        pr.edit_cad_attrs(0, ids[0], |a| {
            a.color = Some([10, 20, 30]);
            a.weight = Some(50);
            a.dash = Some(LineStyle::Dashed);
            a.arrow_end = ArrowStyle::Filled;
            a.fill = Some(FillAttr {
                pattern: "Cross Hatch".into(),
                ..FillAttr::default()
            });
            a.runs = vec![RichRun::bold("x"), RichRun::plain("y")];
        });
        let a = pr.floors[0].cad_attrs(ids[0]).unwrap();
        assert_eq!(a.color, Some([10, 20, 30]));
        assert_eq!(a.arrow_end, ArrowStyle::Filled);
        assert_eq!(a.runs.len(), 2);
        assert!(pr.floors[0].cad_attr_map().contains_key(&ids[0]));
        // Editing again updates the one record.
        let n = pr.floors[0].cad.len();
        pr.edit_cad_attrs(0, ids[0], |a| a.weight = Some(70));
        assert_eq!(pr.floors[0].cad.len(), n);
        assert_eq!(pr.floors[0].cad_attrs(ids[0]).unwrap().weight, Some(70));
        // The hidden records survive a save and load.
        let back: Project = serde_json::from_str(&serde_json::to_string(&pr).unwrap()).unwrap();
        assert_eq!(
            back.floors[0].cad_attrs(ids[0]),
            pr.floors[0].cad_attrs(ids[0])
        );
        // Back to defaults drops the record; orphans are pruned.
        pr.set_cad_attrs(0, CadAttrs::new(ids[0]));
        assert!(pr.floors[0].cad_attrs(ids[0]).is_none());
        pr.edit_cad_attrs(0, ids[1], |a| a.weight = Some(5));
        pr.remove_cad(0, ids[1]);
        assert_eq!(pr.prune_cad_data(0), 1);
    }

    #[test]
    fn deleting_every_member_of_a_group_drops_the_group() {
        let (mut pr, ids) = project_with_lines(3);
        let g = pr
            .make_group(
                0,
                &[
                    crate::groups::ObjectRef::Cad(ids[0]),
                    crate::groups::ObjectRef::Cad(ids[1]),
                ],
            )
            .unwrap();
        assert!(pr.floors[0].groups.iter().any(|x| x.id == g));
        // One member left: the group stays.
        pr.remove_cad(0, ids[0]);
        pr.prune_cad_data(0);
        assert!(pr.floors[0].groups.iter().any(|x| x.id == g));
        // None left: it goes.
        pr.remove_cad(0, ids[1]);
        pr.prune_cad_data(0);
        assert!(pr.floors[0].groups.is_empty());
    }

    #[test]
    fn detail_items_copy_walls_and_visible_cad() {
        let (mut pr, ids) = project_with_lines(2);
        pr.add_wall(
            0,
            p(0.0, 0.0),
            p(100.0, 0.0),
            6.0,
            96.0,
            crate::model::WallKind::Exterior,
        );
        pr.edit_cad_attrs(0, ids[0], |a| a.weight = Some(9));
        let items = detail_items(&pr.floors[0], &pr.layers, "CAD, Default");
        // One wall outline and the two lines; the data record is left out.
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0].1, CadItem::Polyline { closed: true, .. }));
    }

    #[test]
    fn fillet_keeps_the_sides_that_were_clicked() {
        // Two crossing lines; the corner is the crossing at (50, 50).
        let h = (p(0.0, 50.0), p(100.0, 50.0));
        let v = (p(50.0, 0.0), p(50.0, 100.0));
        // Click the right of the horizontal and the top of the vertical.
        let j = fillet_lines_picked(h, p(90.0, 50.0), v, p(50.0, 90.0), 10.0).unwrap();
        assert!(close(j.first.0, p(60.0, 50.0)) && close(j.first.1, p(100.0, 50.0)));
        assert!(close(j.second.0, p(50.0, 0.0)) || close(j.second.1, p(50.0, 100.0)));
        assert!(close(j.second.1, p(50.0, 100.0)) && close(j.second.0, p(50.0, 60.0)));
        // The other pair of sides.
        let j = chamfer_lines_picked(h, p(10.0, 50.0), v, p(50.0, 10.0), 5.0, 5.0).unwrap();
        assert!(close(j.first.1, p(45.0, 50.0)) && close(j.second.1, p(50.0, 45.0)));
    }
    fn legacy_record(pr: &mut Project, text: String) {
        let id = pr.alloc_id();
        pr.floors[0].cad.push(CadObject {
            id,
            layer: LEGACY_DATA_LAYER.to_string(),
            item: CadItem::Text {
                pos: Point::ZERO,
                text,
                height: 1.0,
                angle: 0.0,
            },
        });
    }

    #[test]
    fn typed_slots_round_trip_through_json() {
        let (mut pr, ids) = project_with_lines(3);
        pr.edit_cad_attrs(0, ids[0], |a| a.weight = Some(35));
        let g = pr.make_cad_block(0, &ids, Some("Bench")).unwrap();
        let mut m = crate::text_styles::TextMacros::default();
        m.macros.push(crate::text_styles::TextMacro {
            name: "job".into(),
            text: "Smith".into(),
        });
        pr.text_macros = m.clone();
        let back: Project = serde_json::from_str(&serde_json::to_string(&pr).unwrap()).unwrap();
        assert_eq!(back.floors[0].cad_attrs(ids[0]).unwrap().weight, Some(35));
        assert_eq!(back.floors[0].cad_block(g).unwrap().name, "Bench");
        assert_eq!(back.text_macros, m);
        // The slots are plain fields: nothing hides in the CAD list.
        assert_eq!(back.floors[0].cad.len(), 3);
    }

    #[test]
    fn migrate_legacy_moves_hidden_records_into_the_slots() {
        let (mut pr, ids) = project_with_lines(3);
        let g = pr.make_cad_block(0, &ids, Some("Bench")).unwrap();
        let attrs = CadAttrs {
            weight: Some(70),
            ..CadAttrs::new(ids[1])
        };
        let block = pr.floors[0].cad_block(g).unwrap();
        let mut m = crate::text_styles::TextMacros::default();
        m.macros.push(crate::text_styles::TextMacro {
            name: "job".into(),
            text: "Smith".into(),
        });
        // A file written by the old storage: the slots empty, records present.
        pr.floors[0].cad_blocks.clear();
        pr.floors[0].cad_attrs.clear();
        legacy_record(
            &mut pr,
            format!("{ATTR_TAG}{}", serde_json::to_string(&attrs).unwrap()),
        );
        legacy_record(
            &mut pr,
            format!("{BLOCK_TAG}{}", serde_json::to_string(&block).unwrap()),
        );
        legacy_record(
            &mut pr,
            format!(
                "{BLOB_TAG}text-macros={}",
                serde_json::to_string(&m).unwrap()
            ),
        );
        let mut layer = crate::layers::Layer::new(LEGACY_DATA_LAYER, [128, 128, 128], 13);
        layer.display = false;
        pr.layers.add(layer);

        assert!(migrate_legacy(&mut pr));
        assert_eq!(pr.floors[0].cad_attrs(ids[1]), Some(attrs));
        assert_eq!(pr.floors[0].cad_block(g).unwrap().name, "Bench");
        assert_eq!(pr.text_macros, m);
        assert_eq!(
            pr.floors[0].cad.len(),
            3,
            "records are gone from the CAD list"
        );
        assert!(pr.layers.get(LEGACY_DATA_LAYER).is_none());
        assert!(!migrate_legacy(&mut pr), "second run changes nothing");
    }
}
