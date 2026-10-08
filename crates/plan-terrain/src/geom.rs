//! Small 2D helpers shared across the crate.

use plan_core::geometry::{dist_to_segment, polygon_area};
use plan_core::Point;

/// Drop consecutive duplicate points (and a closing point equal to the first).
pub(crate) fn dedup_points(pts: &[Point], closed: bool) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for &p in pts {
        if out.last().is_none_or(|q| q.dist(p) > 1e-9) {
            out.push(p);
        }
    }
    if closed && out.len() > 1 && out[0].dist(out[out.len() - 1]) <= 1e-9 {
        out.pop();
    }
    out
}

/// The vertices of a polyline/polygon plus evenly spaced points so no gap exceeds `step`.
pub(crate) fn densify(poly: &[Point], step: f64, closed: bool) -> Vec<Point> {
    let n = poly.len();
    let mut out = Vec::new();
    let segs = if closed { n } else { n.saturating_sub(1) };
    for i in 0..segs {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let parts = if step > 0.0 {
            (a.dist(b) / step).ceil().max(1.0) as usize
        } else {
            1
        };
        for k in 0..parts {
            out.push(Point::lerp(a, b, k as f64 / parts as f64));
        }
    }
    if !closed {
        out.extend(poly.last().copied());
    }
    out
}

/// Distance from `p` to the nearest edge of the closed polygon.
pub(crate) fn dist_to_boundary(p: Point, poly: &[Point]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]))
        .fold(f64::INFINITY, f64::min)
}

/// Axis-aligned bounds `(min, max)` of a point set.
pub(crate) fn bounds(pts: &[Point]) -> Option<(Point, Point)> {
    let first = *pts.first()?;
    Some(pts.iter().fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

/// Mitered offset of a closed polygon: outward by `d` inches (negative insets).
pub(crate) fn offset_polygon(poly: &[Point], d: f64) -> Vec<Point> {
    let pts = dedup_points(poly, true);
    let n = pts.len();
    if n < 3 {
        return pts;
    }
    let orient = if polygon_area(&pts) >= 0.0 { 1.0 } else { -1.0 };
    // Outward normal of edge a->b: right of travel for CCW, left for CW.
    let normal = |a: Point, b: Point| b.sub(a).normalized().perp().scale(-orient);
    (0..n)
        .map(|i| {
            let n1 = normal(pts[(i + n - 1) % n], pts[i]);
            let n2 = normal(pts[i], pts[(i + 1) % n]);
            let denom = (1.0 + n1.dot(n2)).max(0.2);
            pts[i].add(n1.add(n2).scale(d / denom))
        })
        .collect()
}

/// Left and right edges of a strip around a polyline, mitered at the joints.
pub(crate) struct StripEdges {
    pub center: Vec<Point>,
    pub left: Vec<Point>,
    pub right: Vec<Point>,
}

/// Offset an open polyline by `half` inches to each side. Left is the CCW side of travel.
pub(crate) fn strip_edges(center: &[Point], half: f64) -> StripEdges {
    let center = dedup_points(center, false);
    let n = center.len();
    let seg_normal = |i: usize| center[i + 1].sub(center[i]).normalized().perp();
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for (i, &c) in center.iter().enumerate() {
        let miter = match (i.checked_sub(1), i + 1 < n) {
            (Some(prev), true) => {
                let (n1, n2) = (seg_normal(prev), seg_normal(i));
                n1.add(n2).scale(1.0 / (1.0 + n1.dot(n2)).max(0.2))
            }
            (Some(prev), false) => seg_normal(prev),
            (None, true) => seg_normal(i),
            (None, false) => Point::ZERO,
        };
        left.push(c.add(miter.scale(half)));
        right.push(c.sub(miter.scale(half)));
    }
    StripEdges {
        center,
        left,
        right,
    }
}

/// Flatten a Catmull-Rom spline through `control` into a polyline.
///
/// Terrain splines (Elevation Spline, Spline Feature, spline driveways) are stored
/// flattened; use this to convert the clicked control points. `samples` points are
/// generated per span (minimum 1).
pub fn flatten_spline(control: &[Point], closed: bool, samples: usize) -> Vec<Point> {
    let n = control.len();
    if n < 3 {
        return control.to_vec();
    }
    let samples = samples.max(1);
    let at = |i: isize| -> Point {
        if closed {
            control[i.rem_euclid(n as isize) as usize]
        } else {
            control[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(spans * samples + 1);
    for s in 0..spans as isize {
        let (p0, p1, p2, p3) = (at(s - 1), at(s), at(s + 1), at(s + 2));
        for k in 0..samples {
            let t = k as f64 / samples as f64;
            let (t2, t3) = (t * t, t * t * t);
            let term = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(Point::new(
                term(p0.x, p1.x, p2.x, p3.x),
                term(p0.y, p1.y, p2.y, p3.y),
            ));
        }
    }
    if !closed {
        out.push(control[n - 1]);
    }
    out
}
