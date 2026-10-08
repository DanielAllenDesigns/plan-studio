//! Small geometry helpers on top of `plan_core::geometry`.

use plan_core::geometry::{point_in_polygon, segment_intersection};
use plan_core::Point;

/// Smallest width of the polygon measured across each of its edge directions
/// (the minimum caliper width). For a rectangle this is the short side.
pub(crate) fn min_dimension(poly: &[Point]) -> f64 {
    let n = poly.len();
    let mut best = f64::INFINITY;
    for i in 0..n {
        let d = poly[(i + 1) % n].sub(poly[i]);
        if d.length() < 1e-9 {
            continue;
        }
        let v = d.normalized().perp();
        let (lo, hi) = poly.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            let t = p.dot(v);
            (lo.min(t), hi.max(t))
        });
        best = best.min(hi - lo);
    }
    if best.is_finite() {
        best
    } else {
        0.0
    }
}

/// Whether two simple polygons overlap (edges cross or one contains the other).
pub(crate) fn polygons_overlap(a: &[Point], b: &[Point]) -> bool {
    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    for i in 0..a.len() {
        for j in 0..b.len() {
            if segment_intersection(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()])
                .is_some()
            {
                return true;
            }
        }
    }
    point_in_polygon(a[0], b) || point_in_polygon(b[0], a)
}
