//! Small 2D/3D helpers shared by the roof feature modules (holes, ceilings,
//! dormers, gable lines, returns). Crate-private.

use plan_core::geometry::{dist_to_segment, point_in_polygon, polygon_area};
use plan_core::Point;

pub(crate) type V3 = [f64; 3];

pub(crate) fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn add3(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn scale3(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
pub(crate) fn dot3(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cross3(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn unit3(a: V3) -> Option<V3> {
    let len = dot3(a, a).sqrt();
    (len > 1e-12).then(|| scale3(a, 1.0 / len))
}

/// Plan point of a roof-space vertex (`X = x`, `Z = -y`).
pub(crate) fn to_plan(p: V3) -> Point {
    Point::new(p[0], -p[2])
}

/// Roof-space vertex at plan point `p` and elevation `y`.
pub(crate) fn lift(p: Point, y: f64) -> V3 {
    [p.x, y, -p.y]
}

/// Newell vector of a 3D polygon (twice the area, along the face normal).
pub(crate) fn newell(poly: &[V3]) -> V3 {
    let n = poly.len();
    let mut s = [0.0; 3];
    for i in 0..n {
        let (c, d) = (poly[i], poly[(i + 1) % n]);
        s[0] += (c[1] - d[1]) * (c[2] + d[2]);
        s[1] += (c[2] - d[2]) * (c[0] + d[0]);
        s[2] += (c[0] - d[0]) * (c[1] + d[1]);
    }
    s
}

/// Reorder `pts` so the polygon faces upward (counter-clockwise seen from
/// above) while keeping the edge `pts[0] -> pts[1]` as the first edge.
pub(crate) fn up_eave_first(pts: Vec<V3>) -> Vec<V3> {
    let n = pts.len();
    if n < 3 || newell(&pts)[1] >= 0.0 {
        return pts;
    }
    // Reversed, the eave edge reads pts[1] -> pts[0].
    let mut out = Vec::with_capacity(n);
    out.push(pts[1]);
    out.push(pts[0]);
    out.extend(pts[2..].iter().rev().copied());
    out
}

/// Reverse `poly` when its Newell normal points away from `outward`.
pub(crate) fn orient_toward(mut poly: Vec<V3>, outward: V3) -> Vec<V3> {
    if dot3(newell(&poly), outward) < 0.0 {
        poly.reverse();
    }
    poly
}

pub(crate) fn ccw(poly: &[Point]) -> Vec<Point> {
    let mut v = poly.to_vec();
    if polygon_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

/// True for a counter-clockwise polygon with no reflex corner.
pub(crate) fn is_convex(poly: &[Point]) -> bool {
    let n = poly.len();
    (0..n).all(|i| {
        let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
        b.sub(a).cross(c.sub(b)) >= -1e-9
    })
}

/// Sutherland-Hodgman: `subject` clipped to the convex CCW polygon `clip`.
pub(crate) fn clip_convex(subject: &[Point], clip: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = subject.to_vec();
    let m = clip.len();
    for k in 0..m {
        if out.is_empty() {
            break;
        }
        let (a, b) = (clip[k], clip[(k + 1) % m]);
        let side = |p: Point| b.sub(a).cross(p.sub(a));
        let input = std::mem::take(&mut out);
        let n = input.len();
        for i in 0..n {
            let (cur, prev) = (input[i], input[(i + n - 1) % n]);
            let (sc, sp) = (side(cur), side(prev));
            if sc >= 0.0 {
                if sp < 0.0 {
                    out.push(crossing(prev, cur, sp, sc));
                }
                out.push(cur);
            } else if sp >= 0.0 {
                out.push(crossing(prev, cur, sp, sc));
            }
        }
    }
    out
}

fn crossing(p: Point, q: Point, sp: f64, sq: f64) -> Point {
    let t = sp / (sp - sq);
    Point::lerp(p, q, t)
}

/// Offset a CCW polygon inward by `d` (outward for negative `d`) by
/// intersecting neighbouring offset lines. `None` if an edge collapses.
pub(crate) fn offset_polygon(poly: &[Point], d: f64) -> Option<Vec<Point>> {
    let n = poly.len();
    if n < 3 {
        return None;
    }
    let dir = |k: usize| poly[(k + 1) % n].sub(poly[k]).normalized();
    let mut res = Vec::with_capacity(n);
    for k in 0..n {
        let a = (k + n - 1) % n;
        let (da, db) = (dir(a), dir(k));
        let pa = poly[a].add(da.perp().scale(d));
        let pb = poly[k].add(db.perp().scale(d));
        let denom = da.cross(db);
        let q = if denom.abs() < 1e-9 {
            poly[k].add(da.perp().scale(d))
        } else {
            let t = pb.sub(pa).cross(db) / denom;
            pa.add(da.scale(t))
        };
        res.push(q);
    }
    for k in 0..n {
        if res[(k + 1) % n].sub(res[k]).dot(dir(k)) <= 1e-9 {
            return None;
        }
    }
    Some(res)
}

/// A counter-clockwise polygon with edge `i` (`pts[i] -> pts[i + 1]`) moved
/// outward by `shifts[i]` (inward for a negative shift; missing entries are
/// zero). Each new corner is where the two moved edge lines meet; two
/// parallel edges slide the corner by the larger shift. The vertex count and
/// order stay the same.
pub(crate) fn offset_edges(pts: &[Point], shifts: &[f64]) -> Vec<Point> {
    let n = pts.len();
    let shift = |i: usize| shifts.get(i).copied().unwrap_or(0.0);
    // Edge line i: a point on it and its direction.
    let line = |i: usize| {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let d = b.sub(a).normalized();
        (a.add(Point::new(d.y, -d.x).scale(shift(i))), d)
    };
    (0..n)
        .map(|i| {
            let prev = (i + n - 1) % n;
            let ((po, dp), (qo, dq)) = (line(prev), line(i));
            let denom = dp.cross(dq);
            if denom.abs() < 1e-9 {
                let (k, d) = if shift(i).abs() >= shift(prev).abs() {
                    (shift(i), dq)
                } else {
                    (shift(prev), dp)
                };
                pts[i].add(Point::new(d.y, -d.x).scale(k))
            } else {
                po.add(dp.scale(qo.sub(po).cross(dq) / denom))
            }
        })
        .collect()
}

/// Distance from `p` to the boundary of `poly`.
pub(crate) fn boundary_dist(p: Point, poly: &[Point]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]))
        .fold(f64::INFINITY, f64::min)
}

/// `p` lies inside `poly` and at least `tol` away from its boundary.
pub(crate) fn strictly_inside(p: Point, poly: &[Point], tol: f64) -> bool {
    point_in_polygon(p, poly) && boundary_dist(p, poly) > tol
}

/// True when segments `ab` and `cd` cross at a point interior to both.
fn proper_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (b.sub(a).cross(c.sub(a)), b.sub(a).cross(d.sub(a)));
    let (d3, d4) = (d.sub(c).cross(a.sub(c)), d.sub(c).cross(b.sub(c)));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// True when the two simple polygons overlap or touch in any way that matters
/// for hole placement (a vertex of one inside the other, or crossing edges).
pub(crate) fn polygons_overlap(a: &[Point], b: &[Point]) -> bool {
    if a.iter().any(|&p| point_in_polygon(p, b)) || b.iter().any(|&p| point_in_polygon(p, a)) {
        return true;
    }
    let (na, nb) = (a.len(), b.len());
    (0..na).any(|i| (0..nb).any(|j| proper_cross(a[i], a[(i + 1) % na], b[j], b[(j + 1) % nb])))
}

/// Triangles of a simple polygon (ear clipping, no holes). Used to split a
/// concave clip polygon into convex pieces.
pub(crate) fn ear_triangles(poly: &[Point]) -> Vec<[Point; 3]> {
    let mut ring = ccw(poly);
    let mut out = Vec::new();
    while ring.len() > 3 {
        let n = ring.len();
        let ear = (0..n).find(|&i| {
            let (a, b, c) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
            if b.sub(a).cross(c.sub(b)) <= 1e-9 {
                return false;
            }
            !ring.iter().enumerate().any(|(j, &p)| {
                if j == i || j == (i + 1) % n || j == (i + n - 1) % n {
                    return false;
                }
                b.sub(a).cross(p.sub(a)) >= 0.0
                    && c.sub(b).cross(p.sub(b)) >= 0.0
                    && a.sub(c).cross(p.sub(c)) >= 0.0
            })
        });
        let i = ear.unwrap_or(0);
        let (a, b, c) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
        out.push([a, b, c]);
        ring.remove(i);
    }
    if ring.len() == 3 {
        out.push([ring[0], ring[1], ring[2]]);
    }
    out
}
