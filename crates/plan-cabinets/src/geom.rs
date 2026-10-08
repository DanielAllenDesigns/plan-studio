//! Small polygon toolbox for cabinets: union, miter offset, slab
//! triangulation (with holes) and free-span search along a run.
//!
//! Everything works on plain `Vec<Point>` rings in inches. Rings may be given
//! in either winding unless a function says otherwise.

use plan_core::geometry::{point_in_polygon, polygon_area, segment_intersection, Point};
use std::collections::HashMap;

/// Coordinates are snapped to this grid (1/1000") so shared corners of
/// neighbouring cabinets compare equal.
const SNAP: f64 = 1000.0;
/// Distance under which a point counts as lying on an edge.
const ON_EDGE: f64 = 2e-3;

fn snap(v: f64) -> f64 {
    (v * SNAP).round() / SNAP
}

fn snap_pt(p: Point) -> Point {
    Point::new(snap(p.x), snap(p.y))
}

fn key(p: Point) -> (i64, i64) {
    ((p.x * SNAP).round() as i64, (p.y * SNAP).round() as i64)
}

/// Axis-aligned bounds `(min, max)` of the points.
pub fn bbox(pts: &[Point]) -> Option<(Point, Point)> {
    let first = *pts.first()?;
    Some(pts.iter().fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

/// The ring wound counter-clockwise.
pub fn ccw(ring: &[Point]) -> Vec<Point> {
    let mut v = ring.to_vec();
    if polygon_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

/// Drops repeated points and vertices in the middle of straight runs.
pub fn clean_ring(ring: &[Point]) -> Vec<Point> {
    let mut pts: Vec<Point> = Vec::with_capacity(ring.len());
    for p in ring {
        if pts.last().is_none_or(|q| q.dist(*p) > 1e-6) {
            pts.push(*p);
        }
    }
    while pts.len() > 1 && pts[0].dist(pts[pts.len() - 1]) <= 1e-6 {
        pts.pop();
    }
    let n = pts.len();
    if n < 3 {
        return pts;
    }
    (0..n)
        .filter(|&i| {
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            b.sub(a).cross(c.sub(b)).abs() > 1e-6
        })
        .map(|i| pts[i])
        .collect()
}

/// Absolute area of a ring.
pub fn area(ring: &[Point]) -> f64 {
    polygon_area(ring).abs()
}

fn on_segment(p: Point, a: Point, b: Point) -> bool {
    plan_core::geometry::dist_to_segment(p, a, b) <= ON_EDGE
}

/// Union of simple polygons. Overlapping and abutting polygons merge; shared
/// boundaries disappear. Returns every boundary ring: outer rings wind
/// counter-clockwise, holes clockwise (check with [`polygon_area`]).
pub fn union_polygons(polys: &[Vec<Point>]) -> Vec<Vec<Point>> {
    let polys: Vec<Vec<Point>> = polys
        .iter()
        .map(|p| clean_ring(&ccw(&p.iter().map(|q| snap_pt(*q)).collect::<Vec<_>>())))
        .filter(|p| p.len() >= 3 && area(p) > 1e-6)
        .collect();
    let mut kept: Vec<(Point, Point)> = Vec::new();
    for (i, poly) in polys.iter().enumerate() {
        let n = poly.len();
        for e in 0..n {
            let (a, b) = (poly[e], poly[(e + 1) % n]);
            let mut ts = vec![0.0_f64, 1.0];
            for (j, other) in polys.iter().enumerate() {
                if j == i {
                    continue;
                }
                let m = other.len();
                for f in 0..m {
                    let (c, d) = (other[f], other[(f + 1) % m]);
                    for v in [c, d] {
                        if on_segment(v, a, b) {
                            ts.push(project_t(v, a, b));
                        }
                    }
                    if let Some((t, _)) = segment_intersection(a, b, c, d) {
                        ts.push(t);
                    }
                }
            }
            ts.sort_by(|x, y| x.total_cmp(y));
            ts.dedup_by(|x, y| (*x - *y).abs() * a.dist(b) < 1e-4);
            for w in ts.windows(2) {
                let (p, q) = (
                    snap_pt(Point::lerp(a, b, w[0])),
                    snap_pt(Point::lerp(a, b, w[1])),
                );
                if p.dist(q) < 1e-4 {
                    continue;
                }
                if keep_edge(&polys, i, p, q) {
                    kept.push((p, q));
                }
            }
        }
    }
    chain(kept)
}

fn project_t(p: Point, a: Point, b: Point) -> f64 {
    let ab = b.sub(a);
    let l2 = ab.dot(ab);
    if l2 <= 1e-18 {
        0.0
    } else {
        (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0)
    }
}

/// Does the sub-edge `p -> q` of polygon `i` stay on the union's boundary?
fn keep_edge(polys: &[Vec<Point>], i: usize, p: Point, q: Point) -> bool {
    let mid = Point::lerp(p, q, 0.5);
    let dir = q.sub(p);
    for (j, other) in polys.iter().enumerate() {
        if j == i {
            continue;
        }
        let m = other.len();
        let mut on_boundary = None;
        for f in 0..m {
            let (c, d) = (other[f], other[(f + 1) % m]);
            if on_segment(mid, c, d) {
                on_boundary = Some(d.sub(c));
                break;
            }
        }
        match on_boundary {
            Some(edge) => {
                if edge.dot(dir) < 0.0 || j < i {
                    // Opposite directions cancel; same-direction duplicates are
                    // kept once (by the lowest polygon index).
                    return false;
                }
            }
            None => {
                if point_in_polygon(mid, other) {
                    return false;
                }
            }
        }
    }
    true
}

/// Chains directed edges into rings.
fn chain(edges: Vec<(Point, Point)>) -> Vec<Vec<Point>> {
    let mut next: HashMap<(i64, i64), Vec<Point>> = HashMap::new();
    for (p, q) in &edges {
        next.entry(key(*p)).or_default().push(*q);
    }
    let mut starts: Vec<(i64, i64)> = next.keys().copied().collect();
    starts.sort_unstable();
    let mut rings = Vec::new();
    for s in starts {
        while let Some(first) = next.get_mut(&s).and_then(Vec::pop) {
            let start = Point::new(s.0 as f64 / SNAP, s.1 as f64 / SNAP);
            let mut ring = vec![start];
            let mut cur = first;
            let mut guard = edges.len() + 2;
            while key(cur) != s && guard > 0 {
                ring.push(cur);
                match next.get_mut(&key(cur)).and_then(Vec::pop) {
                    Some(n) => cur = n,
                    None => break,
                }
                guard -= 1;
            }
            let ring = clean_ring(&ring);
            if ring.len() >= 3 && area(&ring) > 1e-6 {
                rings.push(ring);
            }
        }
    }
    rings
}

/// Offsets a ring by `d` inches along its outward normals (negative shrinks),
/// mitering the corners. Winding is normalized to counter-clockwise first.
pub fn offset_ring(ring: &[Point], d: f64) -> Vec<Point> {
    let r = ccw(ring);
    let n = r.len();
    if n < 3 {
        return r;
    }
    (0..n)
        .map(|i| {
            let (a, b, c) = (r[(i + n - 1) % n], r[i], r[(i + 1) % n]);
            let n1 = edge_normal(a, b);
            let n2 = edge_normal(b, c);
            let denom = 1.0 + n1.dot(n2);
            if denom.abs() < 1e-9 {
                b.add(n1.scale(d))
            } else {
                b.add(n1.add(n2).scale(d / denom))
            }
        })
        .collect()
}

/// Outward unit normal of the counter-clockwise edge `a -> b`.
fn edge_normal(a: Point, b: Point) -> Point {
    let e = b.sub(a).normalized();
    Point::new(e.y, -e.x)
}

/// The closed outline of an open path thickened to `thickness` (mitered).
pub fn thicken_path(path: &[Point], thickness: f64) -> Vec<Point> {
    let n = path.len();
    if n < 2 {
        return Vec::new();
    }
    let half = thickness / 2.0;
    let normal_at = |i: usize| -> Point {
        let prev = if i == 0 { path[0] } else { path[i - 1] };
        let next = if i + 1 == n { path[n - 1] } else { path[i + 1] };
        if i == 0 {
            return left(path[1].sub(path[0])).normalized();
        }
        if i + 1 == n {
            return left(path[n - 1].sub(path[n - 2])).normalized();
        }
        let n1 = left(path[i].sub(prev)).normalized();
        let n2 = left(next.sub(path[i])).normalized();
        let denom = 1.0 + n1.dot(n2);
        if denom.abs() < 1e-9 {
            n1
        } else {
            n1.add(n2).scale(1.0 / denom)
        }
    };
    let mut out: Vec<Point> = (0..n)
        .map(|i| path[i].add(normal_at(i).scale(half)))
        .collect();
    out.extend((0..n).rev().map(|i| path[i].sub(normal_at(i).scale(half))));
    out
}

fn left(v: Point) -> Point {
    v.perp()
}

/// A triangle in the plane, counter-clockwise.
pub type Tri = [Point; 3];

/// Triangulates the region inside `outer` and outside every ring of `holes`
/// (even-odd) by vertical slab decomposition: robust for concave outlines
/// and holes, at the price of more triangles than ear clipping.
pub fn triangulate(outer: &[Point], holes: &[Vec<Point>]) -> Vec<Tri> {
    let rings: Vec<&[Point]> = std::iter::once(outer)
        .chain(holes.iter().map(Vec::as_slice))
        .collect();
    let mut edges: Vec<(Point, Point)> = Vec::new();
    for r in &rings {
        let n = r.len();
        for i in 0..n {
            let (a, b) = (r[i], r[(i + 1) % n]);
            if (a.x - b.x).abs() > 1e-9 {
                edges.push(if a.x < b.x { (a, b) } else { (b, a) });
            }
        }
    }
    let mut xs: Vec<f64> = rings.iter().flat_map(|r| r.iter().map(|p| p.x)).collect();
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let y_at = |e: &(Point, Point), x: f64| {
        let t = (x - e.0.x) / (e.1.x - e.0.x);
        e.0.y + t * (e.1.y - e.0.y)
    };
    let mut tris = Vec::new();
    for w in xs.windows(2) {
        let (x0, x1) = (w[0], w[1]);
        if x1 - x0 < 1e-9 {
            continue;
        }
        let xm = (x0 + x1) / 2.0;
        let mut span: Vec<&(Point, Point)> = edges
            .iter()
            .filter(|e| e.0.x <= x0 + 1e-9 && e.1.x >= x1 - 1e-9)
            .collect();
        span.sort_by(|a, b| y_at(a, xm).total_cmp(&y_at(b, xm)));
        for pair in span.chunks(2) {
            let [lo, hi] = pair else { break };
            let (a, b) = (Point::new(x0, y_at(lo, x0)), Point::new(x1, y_at(lo, x1)));
            let (c, d) = (Point::new(x1, y_at(hi, x1)), Point::new(x0, y_at(hi, x0)));
            for t in [[a, b, c], [a, c, d]] {
                if polygon_area(&t).abs() > 1e-9 {
                    tris.push(t);
                }
            }
        }
    }
    tris
}

/// Area of the region inside `outer` and outside `holes`.
pub fn region_area(outer: &[Point], holes: &[Vec<Point>]) -> f64 {
    area(outer) - holes.iter().map(|h| area(h)).sum::<f64>()
}

/// The probe point of [`free_span`] lies inside an obstacle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsideObstacle;

/// The free interval along `u` around `s0` in the strip `t in [t0, t1]`.
///
/// Coordinates are measured from `origin` along the unit vectors `u` and `v`
/// (`s = (p - origin) . u`, `t = (p - origin) . v`). Every obstacle polygon is
/// clipped to the strip; those wholly left of `s0` bound the span from the
/// left, those wholly right bound it from the right. `None` on a side means
/// nothing blocks that way. [`InsideObstacle`] means `s0` itself lies inside
/// an obstacle.
pub fn free_span(
    obstacles: &[Vec<Point>],
    origin: Point,
    u: Point,
    v: Point,
    (t0, t1): (f64, f64),
    s0: f64,
) -> Result<(Option<f64>, Option<f64>), InsideObstacle> {
    let mut left: Option<f64> = None;
    let mut right: Option<f64> = None;
    for poly in obstacles {
        let st: Vec<(f64, f64)> = poly
            .iter()
            .map(|p| {
                let d = p.sub(origin);
                (d.dot(u), d.dot(v))
            })
            .collect();
        let clipped = clip_band(&st, t0, t1);
        if clipped.len() < 2 {
            continue;
        }
        let smin = clipped.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let smax = clipped.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        if smax - smin < 1e-6 {
            continue;
        }
        if smax <= s0 + 1e-9 {
            left = Some(left.map_or(smax, |l| l.max(smax)));
        } else if smin >= s0 - 1e-9 {
            right = Some(right.map_or(smin, |r| r.min(smin)));
        } else {
            return Err(InsideObstacle);
        }
    }
    Ok((left, right))
}

/// Sutherland-Hodgman clip of `poly` to `t0 <= t <= t1`.
fn clip_band(poly: &[(f64, f64)], t0: f64, t1: f64) -> Vec<(f64, f64)> {
    let clip = |input: Vec<(f64, f64)>, keep_above: bool, lim: f64| -> Vec<(f64, f64)> {
        let inside = |p: &(f64, f64)| if keep_above { p.1 >= lim } else { p.1 <= lim };
        let mut out = Vec::new();
        let n = input.len();
        for i in 0..n {
            let (a, b) = (input[i], input[(i + 1) % n]);
            let (ia, ib) = (inside(&a), inside(&b));
            if ia {
                out.push(a);
            }
            if ia != ib {
                let k = (lim - a.1) / (b.1 - a.1);
                out.push((a.0 + k * (b.0 - a.0), lim));
            }
        }
        out
    };
    clip(clip(poly.to_vec(), true, t0), false, t1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    #[test]
    fn union_of_abutting_rects_is_one_rectangle() {
        let u = union_polygons(&[rect(0.0, 0.0, 24.0, 25.0), rect(24.0, 0.0, 48.0, 25.0)]);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].len(), 4);
        assert!((area(&u[0]) - 48.0 * 25.0).abs() < 1e-6);
    }

    #[test]
    fn union_of_an_l_and_a_notch_filler_and_disjoint_pieces() {
        // Two overlapping rectangles make an L.
        let l = union_polygons(&[rect(0.0, 0.0, 36.0, 25.0), rect(0.0, 0.0, 25.0, 36.0)]);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].len(), 6);
        assert!((area(&l[0]) - (36.0 * 25.0 + 25.0 * 36.0 - 25.0 * 25.0)).abs() < 1e-6);
        // A gap keeps two rings.
        let two = union_polygons(&[rect(0.0, 0.0, 10.0, 10.0), rect(12.0, 0.0, 20.0, 10.0)]);
        assert_eq!(two.len(), 2);
        // Different-angle neighbours join too.
        let tri = vec![
            Point::new(10.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(10.0, 10.0),
        ];
        let j = union_polygons(&[rect(0.0, 0.0, 10.0, 10.0), tri]);
        assert_eq!(j.len(), 1);
        assert!((area(&j[0]) - 150.0).abs() < 1e-6);
    }

    #[test]
    fn union_keeps_an_enclosed_hole_as_a_clockwise_ring() {
        let ring = union_polygons(&[
            rect(0.0, 0.0, 30.0, 10.0),
            rect(0.0, 20.0, 30.0, 30.0),
            rect(0.0, 10.0, 10.0, 20.0),
            rect(20.0, 10.0, 30.0, 20.0),
        ]);
        assert_eq!(ring.len(), 2);
        let holes = ring.iter().filter(|r| polygon_area(r) < 0.0).count();
        assert_eq!(holes, 1);
    }

    #[test]
    fn triangulation_conserves_area_with_holes_and_concavity() {
        let outer = rect(0.0, 0.0, 60.0, 25.0);
        let hole = rect(20.0, 8.0, 40.0, 18.0);
        let t = triangulate(&outer, std::slice::from_ref(&hole));
        let a: f64 = t.iter().map(|t| polygon_area(t)).sum();
        assert!((a - (1500.0 - 200.0)).abs() < 1e-6, "{a}");
        assert!(t.iter().all(|t| polygon_area(t) > 0.0));
        // An L-shaped outline.
        let l = union_polygons(&[rect(0.0, 0.0, 36.0, 25.0), rect(0.0, 0.0, 25.0, 36.0)]);
        let t = triangulate(&l[0], &[]);
        let a: f64 = t.iter().map(|t| polygon_area(t)).sum();
        assert!((a - area(&l[0])).abs() < 1e-6);
    }

    #[test]
    fn offset_and_thicken() {
        let o = offset_ring(&rect(0.0, 0.0, 10.0, 10.0), 1.0);
        assert!((area(&o) - 144.0).abs() < 1e-9);
        let strip = thicken_path(&[Point::new(0.0, 0.0), Point::new(10.0, 0.0)], 2.0);
        assert!((area(&strip) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn free_span_finds_the_gap_between_obstacles() {
        let o = [rect(-10.0, -5.0, 0.0, 50.0), rect(30.0, -5.0, 40.0, 50.0)];
        let span = free_span(
            &o,
            Point::ZERO,
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            (0.05, 23.95),
            15.0,
        )
        .unwrap();
        assert_eq!(span, (Some(0.0), Some(30.0)));
        // Inside an obstacle there is no span.
        assert!(free_span(
            &o,
            Point::ZERO,
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            (0.05, 23.95),
            35.0
        )
        .is_err());
    }
}
