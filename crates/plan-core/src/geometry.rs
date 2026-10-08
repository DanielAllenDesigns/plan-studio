use serde::{Deserialize, Serialize};

/// A 2D point or vector in plan space (inches, Y up).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    /// Vector sum. Also available as `a + b` via `std::ops::Add`.
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, o: Point) -> Point {
        Point::new(self.x + o.x, self.y + o.y)
    }
    /// Vector difference. Also available as `a - b` via `std::ops::Sub`.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, o: Point) -> Point {
        Point::new(self.x - o.x, self.y - o.y)
    }
    pub fn scale(self, k: f64) -> Point {
        Point::new(self.x * k, self.y * k)
    }
    pub fn dot(self, o: Point) -> f64 {
        self.x * o.x + self.y * o.y
    }
    /// 2D cross product (z component).
    pub fn cross(self, o: Point) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn dist(self, o: Point) -> f64 {
        self.sub(o).length()
    }
    /// Unit vector; returns ZERO for a zero-length input.
    pub fn normalized(self) -> Point {
        let l = self.length();
        if l <= f64::EPSILON {
            Point::ZERO
        } else {
            self.scale(1.0 / l)
        }
    }
    /// Rotated 90° counter-clockwise.
    pub fn perp(self) -> Point {
        Point::new(-self.y, self.x)
    }
    /// Angle in radians, (-π, π].
    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }
    pub fn lerp(a: Point, b: Point, t: f64) -> Point {
        a.add(b.sub(a).scale(t))
    }
}

impl std::ops::Add for Point {
    type Output = Point;
    fn add(self, o: Point) -> Point {
        Point::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        Point::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Mul<f64> for Point {
    type Output = Point;
    fn mul(self, k: f64) -> Point {
        self.scale(k)
    }
}

impl std::ops::Neg for Point {
    type Output = Point;
    fn neg(self) -> Point {
        Point::new(-self.x, -self.y)
    }
}

/// Closest point on segment `ab` to `p`. Returns `(t, point)` with `t` in `[0, 1]`.
pub fn project_on_segment(p: Point, a: Point, b: Point) -> (f64, Point) {
    let ab = b.sub(a);
    let len2 = ab.dot(ab);
    if len2 <= f64::EPSILON {
        return (0.0, a);
    }
    let t = (p.sub(a).dot(ab) / len2).clamp(0.0, 1.0);
    (t, Point::lerp(a, b, t))
}

pub fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    project_on_segment(p, a, b).1.dist(p)
}

/// Intersection of segments `a1a2` and `b1b2` as parameters `(t, u)` along each.
/// Returns `None` for parallel segments or when the intersection is outside both.
pub fn segment_intersection(a1: Point, a2: Point, b1: Point, b2: Point) -> Option<(f64, f64)> {
    let r = a2.sub(a1);
    let s = b2.sub(b1);
    let denom = r.cross(s);
    if denom.abs() <= 1e-12 {
        return None;
    }
    let qp = b1.sub(a1);
    let t = qp.cross(s) / denom;
    let u = qp.cross(r) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some((t, u))
    } else {
        None
    }
}

/// Signed shoelace area. Positive for counter-clockwise polygons.
pub fn polygon_area(pts: &[Point]) -> f64 {
    if pts.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        sum += a.cross(b);
    }
    sum * 0.5
}

/// Area-weighted centroid; falls back to the vertex average for degenerate polygons.
pub fn polygon_centroid(pts: &[Point]) -> Point {
    let area = polygon_area(pts);
    if pts.is_empty() {
        return Point::ZERO;
    }
    if area.abs() <= 1e-9 {
        let n = pts.len() as f64;
        return pts
            .iter()
            .fold(Point::ZERO, |acc, p| acc.add(*p))
            .scale(1.0 / n);
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        let f = a.cross(b);
        cx += (a.x + b.x) * f;
        cy += (a.y + b.y) * f;
    }
    let k = 1.0 / (6.0 * area);
    Point::new(cx * k, cy * k)
}

/// Even-odd point-in-polygon test.
pub fn point_in_polygon(p: Point, pts: &[Point]) -> bool {
    let mut inside = false;
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (pi, pj) = (pts[i], pts[j]);
        if (pi.y > p.y) != (pj.y > p.y) {
            let x = pj.x + (p.y - pj.y) * (pi.x - pj.x) / (pi.y - pj.y);
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_of_unit_square_is_positive_ccw() {
        let sq = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        assert!((polygon_area(&sq) - 1.0).abs() < 1e-12);
        let c = polygon_centroid(&sq);
        assert!((c.x - 0.5).abs() < 1e-12 && (c.y - 0.5).abs() < 1e-12);
    }

    #[test]
    fn crossing_segments_intersect_at_midpoints() {
        let r = segment_intersection(
            Point::new(0.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
            Point::new(2.0, 0.0),
        )
        .unwrap();
        assert!((r.0 - 0.5).abs() < 1e-12 && (r.1 - 0.5).abs() < 1e-12);
    }
}
