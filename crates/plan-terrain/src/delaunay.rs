//! Bowyer-Watson Delaunay triangulation.

use std::collections::HashSet;

use plan_core::Point;

/// A triangle with its cached circumcircle (normalized coordinates).
struct Tri {
    v: [usize; 3],
    cx: f64,
    cy: f64,
    r2: f64,
}

impl Tri {
    /// Build a counter-clockwise triangle; `None` when (nearly) degenerate.
    fn new(pts: &[(f64, f64)], a: usize, mut b: usize, mut c: usize) -> Option<Tri> {
        let (ax, ay) = pts[a];
        let cross = |b: usize, c: usize| {
            (pts[b].0 - ax) * (pts[c].1 - ay) - (pts[b].1 - ay) * (pts[c].0 - ax)
        };
        let area2 = cross(b, c);
        if area2.abs() < 1e-18 {
            return None;
        }
        if area2 < 0.0 {
            std::mem::swap(&mut b, &mut c);
        }
        let (bx, by) = (pts[b].0 - ax, pts[b].1 - ay);
        let (cx, cy) = (pts[c].0 - ax, pts[c].1 - ay);
        let d = 2.0 * (bx * cy - by * cx);
        let b2 = bx * bx + by * by;
        let c2 = cx * cx + cy * cy;
        let ux = (cy * b2 - by * c2) / d;
        let uy = (bx * c2 - cx * b2) / d;
        Some(Tri {
            v: [a, b, c],
            cx: ax + ux,
            cy: ay + uy,
            r2: ux * ux + uy * uy,
        })
    }

    /// Whether `p` lies strictly inside the circumcircle (cocircular points are outside).
    fn circumcircle_contains(&self, p: (f64, f64)) -> bool {
        let (dx, dy) = (p.0 - self.cx, p.1 - self.cy);
        dx * dx + dy * dy < self.r2 * (1.0 - 1e-10)
    }
}

/// Delaunay-triangulate `points`, returning counter-clockwise index triples.
///
/// Points are expected to be distinct; fewer than three points yield no triangles.
/// Cocircular configurations (such as regular grids) are resolved arbitrarily.
pub fn triangulate(points: &[Point]) -> Vec<[usize; 3]> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }
    // Normalize into roughly [-0.5, 0.5] so the super-triangle size is scale free.
    let (mut lo, mut hi) = (points[0], points[0]);
    for p in points {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let mid = Point::lerp(lo, hi, 0.5);
    let scale = (hi.x - lo.x).max(hi.y - lo.y).max(1e-12);
    let mut pts: Vec<(f64, f64)> = points
        .iter()
        .map(|p| ((p.x - mid.x) / scale, (p.y - mid.y) / scale))
        .collect();
    const SUPER: f64 = 100.0;
    pts.extend([(-SUPER, -SUPER), (SUPER, -SUPER), (0.0, SUPER)]);

    let mut tris: Vec<Tri> = Tri::new(&pts, n, n + 1, n + 2).into_iter().collect();
    for i in 0..n {
        let p = pts[i];
        let mut edges: Vec<(usize, usize)> = Vec::new();
        tris.retain(|t| {
            if t.circumcircle_contains(p) {
                edges.extend([(t.v[0], t.v[1]), (t.v[1], t.v[2]), (t.v[2], t.v[0])]);
                false
            } else {
                true
            }
        });
        // Cavity boundary: directed edges whose reverse is not also present.
        let all: HashSet<(usize, usize)> = edges.iter().copied().collect();
        for &(a, b) in &edges {
            if !all.contains(&(b, a)) {
                tris.extend(Tri::new(&pts, a, b, i));
            }
        }
    }
    tris.into_iter()
        .filter(|t| t.v.iter().all(|&v| v < n))
        .map(|t| t.v)
        .collect()
}
