//! Curve geometry for the DXF importer: bulged segments, elliptical arcs,
//! NURBS splines, and the similarity/affine transform of an INSERT.

use plan_core::Point;
use std::f64::consts::{FRAC_PI_2, TAU};

/// Samples per quarter turn when a curve is turned into straight pieces.
pub const SAMPLES_PER_QUARTER: f64 = 8.0;

/// Intermediate points (excluding both ends) of the arc from `a` to `b` with
/// the DXF bulge `tan(sweep / 4)` (positive = counter-clockwise).
pub fn bulge_points(a: Point, b: Point, bulge: f64) -> Vec<Point> {
    let chord = b.sub(a);
    let d = chord.length();
    if bulge.abs() < 1e-9 || d < 1e-12 {
        return Vec::new();
    }
    let sweep = 4.0 * bulge.atan();
    let half = sweep * 0.5;
    let center = Point::lerp(a, b, 0.5).add(chord.normalized().perp().scale(d * 0.5 / half.tan()));
    let radius = a.dist(center);
    let start = a.sub(center).angle();
    let steps = ((sweep.abs() / FRAC_PI_2 * SAMPLES_PER_QUARTER).ceil() as usize).max(1);
    (1..steps)
        .map(|k| {
            let ang = start + sweep * k as f64 / steps as f64;
            Point::new(center.x + radius * ang.cos(), center.y + radius * ang.sin())
        })
        .collect()
}

/// Vertices of a polyline with bulged segments replaced by sampled arcs.
/// The result keeps the original vertices; for a closed polyline the start
/// point is not repeated at the end.
pub fn sample_polyline(points: &[Point], bulges: &[f64], closed: bool) -> Vec<Point> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    let segments = if closed { n } else { n - 1 };
    let mut out = vec![points[0]];
    for i in 0..segments {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let bulge = bulges.get(i).copied().unwrap_or(0.0);
        out.extend(bulge_points(a, b, bulge));
        if i + 1 < n {
            out.push(b);
        }
    }
    out
}

/// Points of `center + u cos t + v sin t` for `t` from `t0` to `t1`
/// (radians, `t1 > t0`), with `per_quarter` samples per quarter turn.
pub fn ellipse_points(center: Point, u: Point, v: Point, t0: f64, t1: f64) -> Vec<Point> {
    let sweep = (t1 - t0).abs();
    let steps = ((sweep / FRAC_PI_2 * (SAMPLES_PER_QUARTER * 2.0)).ceil() as usize).clamp(4, 2048);
    (0..=steps)
        .map(|k| {
            let t = t0 + (t1 - t0) * k as f64 / steps as f64;
            center.add(u.scale(t.cos())).add(v.scale(t.sin()))
        })
        .collect()
}

/// Sweep `(start, end)` in radians with `end > start`, from DXF parameters
/// (a full turn when they are equal or span more than a turn).
pub fn ellipse_range(start: f64, end: f64) -> (f64, f64) {
    let mut e = end;
    while e <= start {
        e += TAU;
    }
    if e - start > TAU - 1e-9 {
        e = start + TAU;
    }
    (start, e)
}

/// A NURBS curve as DXF stores it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Nurbs<'a> {
    pub degree: usize,
    pub knots: &'a [f64],
    pub weights: &'a [f64],
    pub control: &'a [Point],
}

impl Nurbs<'_> {
    /// Does the knot vector match the control points?
    pub fn is_valid(&self) -> bool {
        let n = self.control.len();
        n > self.degree && self.degree >= 1 && self.knots.len() == n + self.degree + 1
    }

    /// Sampled curve points. A malformed knot vector falls back to a clamped
    /// uniform one.
    pub fn sample(&self, per_span: usize) -> Vec<Point> {
        let n = self.control.len();
        if n < 2 {
            return self.control.to_vec();
        }
        let degree = self.degree.clamp(1, n - 1);
        let fallback;
        let knots: &[f64] = if self.knots.len() == n + degree + 1 && self.degree == degree {
            self.knots
        } else {
            fallback = clamped_uniform(n, degree);
            &fallback
        };
        let (lo, hi) = (knots[degree], knots[n]);
        if hi <= lo {
            return self.control.to_vec();
        }
        let spans = (n - degree).max(1);
        let steps = (spans * per_span.max(2)).min(8192);
        let rational =
            self.weights.len() == n && self.weights.iter().any(|w| (w - 1.0).abs() > 1e-12);
        (0..=steps)
            .map(|k| {
                let t = lo + (hi - lo) * k as f64 / steps as f64;
                eval(self.control, self.weights, rational, knots, degree, t)
            })
            .collect()
    }
}

fn clamped_uniform(n: usize, degree: usize) -> Vec<f64> {
    let m = n + degree + 1;
    let inner = n - degree;
    (0..m)
        .map(|i| {
            if i <= degree {
                0.0
            } else if i >= n {
                inner as f64
            } else {
                (i - degree) as f64
            }
        })
        .collect()
}

/// De Boor evaluation at `t`.
fn eval(ctrl: &[Point], w: &[f64], rational: bool, knots: &[f64], p: usize, t: f64) -> Point {
    let n = ctrl.len();
    // Knot span with knots[k] <= t < knots[k + 1].
    let mut k = p;
    while k + 1 < n && t >= knots[k + 1] {
        k += 1;
    }
    let mut d: Vec<(f64, f64, f64)> = (0..=p)
        .map(|j| {
            let i = k + j - p;
            let wi = if rational { w[i] } else { 1.0 };
            (ctrl[i].x * wi, ctrl[i].y * wi, wi)
        })
        .collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = k + j - p;
            let den = knots[i + p + 1 - r] - knots[i];
            let a = if den.abs() < 1e-14 {
                0.0
            } else {
                (t - knots[i]) / den
            };
            d[j] = (
                (1.0 - a) * d[j - 1].0 + a * d[j].0,
                (1.0 - a) * d[j - 1].1 + a * d[j].1,
                (1.0 - a) * d[j - 1].2 + a * d[j].2,
            );
        }
    }
    let (x, y, wt) = d[p];
    if rational && wt.abs() > 1e-14 {
        Point::new(x / wt, y / wt)
    } else {
        Point::new(x, y)
    }
}

/// The linear map of an INSERT: `p' = pos + R * S * (p - base)`, with
/// per-axis scale, applied to points, directions, radii and angles.
#[derive(Debug, Clone, Copy)]
pub struct Xf {
    pub base: Point,
    pub pos: Point,
    pub sx: f64,
    pub sy: f64,
    pub rot_deg: f64,
    cos: f64,
    sin: f64,
}

impl Xf {
    pub const IDENTITY: Xf = Xf {
        base: Point::ZERO,
        pos: Point::ZERO,
        sx: 1.0,
        sy: 1.0,
        rot_deg: 0.0,
        cos: 1.0,
        sin: 0.0,
    };

    pub fn new(base: Point, pos: Point, scale: (f64, f64), rot_deg: f64) -> Self {
        let r = rot_deg.to_radians();
        Self {
            base,
            pos,
            sx: scale.0,
            sy: scale.1,
            rot_deg,
            cos: r.cos(),
            sin: r.sin(),
        }
    }

    pub fn rotate(&self, v: Point) -> Point {
        Point::new(
            v.x * self.cos - v.y * self.sin,
            v.x * self.sin + v.y * self.cos,
        )
    }

    pub fn point(&self, p: Point) -> Point {
        let d = p.sub(self.base);
        self.pos
            .add(self.rotate(Point::new(d.x * self.sx, d.y * self.sy)))
    }

    /// A direction vector (no translation).
    pub fn vector(&self, v: Point) -> Point {
        self.rotate(Point::new(v.x * self.sx, v.y * self.sy))
    }

    /// Angle (degrees) of a direction after scale and rotation.
    pub fn angle(&self, deg: f64) -> f64 {
        let r = deg.to_radians();
        self.vector(Point::new(r.cos(), r.sin()))
            .angle()
            .to_degrees()
    }

    pub fn mirrored(&self) -> bool {
        self.sx * self.sy < 0.0
    }

    /// Equal scale on both axes (a similarity: circles stay circles).
    pub fn uniform(&self) -> bool {
        (self.sx.abs() - self.sy.abs()).abs() <= 1e-9 * self.sx.abs().max(self.sy.abs()).max(1e-12)
    }

    pub fn is_identity(&self) -> bool {
        self.base == Point::ZERO
            && self.pos == Point::ZERO
            && self.sx == 1.0
            && self.sy == 1.0
            && self.rot_deg == 0.0
    }

    /// Vertical scale magnitude (text height).
    pub fn height_scale(&self) -> f64 {
        self.sy.abs()
    }

    /// Radius of a circle after a uniform scale.
    pub fn radius(&self, r: f64) -> f64 {
        r * self.sx.abs()
    }

    /// `self` applied after `inner` (inner first).
    pub fn then_point(&self, inner: &Xf, p: Point) -> Point {
        self.point(inner.point(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_semicircle_bulge_is_a_true_arc() {
        let pts = sample_polyline(
            &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            &[1.0, 0.0],
            false,
        );
        assert_eq!(pts.len(), 17);
        for p in &pts {
            assert!((p.dist(Point::new(5.0, 0.0)) - 5.0).abs() < 1e-9);
        }
        assert!(pts[8].y < -4.9);
        let cw = sample_polyline(
            &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            &[-1.0, 0.0],
            false,
        );
        assert!(cw[8].y > 4.9);
    }

    #[test]
    fn a_quadratic_bezier_nurbs_hits_its_midpoint() {
        // Degree 2, three control points, clamped knots: the curve at t = 0.5
        // is (P0 + 2 P1 + P2) / 4.
        let ctrl = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 20.0),
            Point::new(20.0, 0.0),
        ];
        let n = Nurbs {
            degree: 2,
            knots: &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            weights: &[],
            control: &ctrl,
        };
        assert!(n.is_valid());
        let pts = n.sample(16);
        assert_eq!(pts.len(), 17);
        assert!(pts[0].dist(ctrl[0]) < 1e-9 && pts[16].dist(ctrl[2]) < 1e-9);
        assert!(pts[8].dist(Point::new(10.0, 10.0)) < 1e-9);
    }

    #[test]
    fn a_rational_quarter_circle_is_exact() {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let ctrl = [
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        let n = Nurbs {
            degree: 2,
            knots: &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            weights: &[1.0, w, 1.0],
            control: &ctrl,
        };
        for p in n.sample(8) {
            assert!((p.length() - 10.0).abs() < 1e-9, "{p:?}");
        }
    }

    #[test]
    fn a_bad_knot_vector_falls_back_to_uniform() {
        let ctrl = [
            Point::new(0.0, 0.0),
            Point::new(5.0, 5.0),
            Point::new(10.0, 0.0),
            Point::new(15.0, 5.0),
        ];
        let n = Nurbs {
            degree: 3,
            knots: &[],
            weights: &[],
            control: &ctrl,
        };
        let pts = n.sample(4);
        assert!(pts.first().unwrap().dist(ctrl[0]) < 1e-9);
        assert!(pts.last().unwrap().dist(ctrl[3]) < 1e-9);
    }

    #[test]
    fn ellipse_samples_stay_on_the_curve() {
        let (t0, t1) = ellipse_range(0.0, 0.0);
        assert!((t1 - t0 - TAU).abs() < 1e-12);
        let pts = ellipse_points(
            Point::new(1.0, 2.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 5.0),
            t0,
            t1,
        );
        for p in &pts {
            let q = p.sub(Point::new(1.0, 2.0));
            assert!(((q.x / 10.0).powi(2) + (q.y / 5.0).powi(2) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn the_transform_scales_rotates_and_moves() {
        let xf = Xf::new(
            Point::new(1.0, 1.0),
            Point::new(100.0, 0.0),
            (2.0, 2.0),
            90.0,
        );
        assert!(xf.point(Point::new(2.0, 1.0)).dist(Point::new(100.0, 2.0)) < 1e-9);
        assert!((xf.angle(0.0) - 90.0).abs() < 1e-9);
        assert!(xf.uniform() && !xf.mirrored());
        let m = Xf::new(Point::ZERO, Point::ZERO, (-1.0, 1.0), 0.0);
        assert!(m.mirrored());
        assert!((m.angle(0.0).abs() - 180.0).abs() < 1e-9);
    }
}
