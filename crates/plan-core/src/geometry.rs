use serde::{Deserialize, Serialize};

/// A 2D point or vector in plan space (inches, Y up).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point {
    #[serde(default, deserialize_with = "crate::foreign::finite_or_zero")]
    pub x: f64,
    #[serde(default, deserialize_with = "crate::foreign::finite_or_zero")]
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

/// A uniform grid over axis-aligned boxes: a broad-phase for "which of these
/// boxes can touch this one?" in near-linear time instead of scanning them
/// all. [`BoxGrid::query`] returns a superset of the boxes that overlap the
/// query box (callers run their exact test on the candidates), always in
/// ascending index order, so a loop over the candidates sees them in the same
/// order as a loop over all boxes would.
#[derive(Debug, Clone)]
pub struct BoxGrid {
    origin: Point,
    cell: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
    /// Boxes too large to store per cell; every query returns them.
    oversize: Vec<u32>,
}

/// A box covering more cells than this is kept in the oversize list.
const MAX_CELLS_PER_BOX: usize = 256;

impl BoxGrid {
    /// A grid over `boxes` (`(lower-left, upper-right)` per box, indexed in
    /// iteration order). Boxes with a NaN coordinate are never returned.
    pub fn new(boxes: &[(Point, Point)]) -> BoxGrid {
        let n = boxes.len();
        let (mut lo, mut hi) = (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        );
        let mut extent = 0.0;
        let mut counted = 0usize;
        for (a, b) in boxes {
            if !(a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite()) {
                continue;
            }
            lo = Point::new(lo.x.min(a.x), lo.y.min(a.y));
            hi = Point::new(hi.x.max(b.x), hi.y.max(b.y));
            extent += (b.x - a.x).max(b.y - a.y);
            counted += 1;
        }
        if counted == 0 {
            return BoxGrid {
                origin: Point::ZERO,
                cell: 1.0,
                nx: 0,
                ny: 0,
                cells: Vec::new(),
                oversize: Vec::new(),
            };
        }
        let (w, h) = (hi.x - lo.x, hi.y - lo.y);
        let mut cell = (extent / counted as f64).max(1e-3);
        let budget = 8 * n + 64;
        while ((w / cell).floor() as usize + 1).saturating_mul((h / cell).floor() as usize + 1)
            > budget
        {
            cell *= 1.5;
        }
        let nx = (w / cell).floor() as usize + 1;
        let ny = (h / cell).floor() as usize + 1;
        let mut g = BoxGrid {
            origin: lo,
            cell,
            nx,
            ny,
            cells: vec![Vec::new(); nx * ny],
            oversize: Vec::new(),
        };
        for (i, (a, b)) in boxes.iter().enumerate() {
            if !(a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite()) {
                continue;
            }
            let (x0, x1, y0, y1) = g.range(*a, *b);
            if (x1 - x0 + 1) * (y1 - y0 + 1) > MAX_CELLS_PER_BOX {
                g.oversize.push(i as u32);
                continue;
            }
            for y in y0..=y1 {
                for x in x0..=x1 {
                    g.cells[y * g.nx + x].push(i as u32);
                }
            }
        }
        g
    }

    /// The inclusive cell ranges `(x0, x1, y0, y1)` a box touches, clamped to
    /// the grid.
    fn range(&self, a: Point, b: Point) -> (usize, usize, usize, usize) {
        let ix = |v: f64, n: usize| -> usize {
            let c = ((v - self.origin.x.min(v)) / self.cell).floor();
            (c.max(0.0) as usize).min(n - 1)
        };
        let iy = |v: f64, n: usize| -> usize {
            let c = ((v - self.origin.y.min(v)) / self.cell).floor();
            (c.max(0.0) as usize).min(n - 1)
        };
        (
            ix(a.x, self.nx),
            ix(b.x, self.nx),
            iy(a.y, self.ny),
            iy(b.y, self.ny),
        )
    }

    /// Indices of every box that may overlap the box `lo..hi`, ascending and
    /// without duplicates, appended to `out` (cleared first).
    pub fn query(&self, lo: Point, hi: Point, out: &mut Vec<usize>) {
        out.clear();
        if self.nx == 0 {
            return;
        }
        // A query box fully off the grid still clamps to the border cells,
        // which only adds candidates.
        let (x0, x1, y0, y1) = self.range(lo, hi);
        for y in y0..=y1 {
            for x in x0..=x1 {
                out.extend(self.cells[y * self.nx + x].iter().map(|&i| i as usize));
            }
        }
        out.extend(self.oversize.iter().map(|&i| i as usize));
        out.sort_unstable();
        out.dedup();
    }
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

    #[test]
    fn box_grid_returns_every_overlapping_box_in_order() {
        let mut seed = 99u64;
        let mut rnd = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut boxes = Vec::new();
        for _ in 0..300 {
            let (x, y) = (rnd() * 5000.0 - 1000.0, rnd() * 5000.0);
            let (w, h) = (rnd() * 150.0, rnd() * 150.0);
            boxes.push((Point::new(x, y), Point::new(x + w, y + h)));
        }
        // One box that spans the plan, and a NaN one that is never returned.
        boxes.push((Point::new(-1000.0, 10.0), Point::new(4000.0, 12.0)));
        boxes.push((Point::new(f64::NAN, 0.0), Point::new(1.0, 1.0)));
        let grid = BoxGrid::new(&boxes);
        let mut out = Vec::new();
        for _ in 0..200 {
            let (x, y) = (rnd() * 6000.0 - 1500.0, rnd() * 6000.0 - 500.0);
            let (lo, hi) = (
                Point::new(x, y),
                Point::new(x + rnd() * 200.0, y + rnd() * 200.0),
            );
            grid.query(lo, hi, &mut out);
            assert!(out.windows(2).all(|w| w[0] < w[1]), "ascending, unique");
            for (i, (a, b)) in boxes.iter().enumerate() {
                let overlaps = a.x <= hi.x && b.x >= lo.x && a.y <= hi.y && b.y >= lo.y;
                if overlaps {
                    assert!(out.contains(&i), "box {i} overlaps {lo:?}..{hi:?}");
                }
            }
        }
        assert!(BoxGrid::new(&[]).nx == 0);
        grid.query(Point::ZERO, Point::ZERO, &mut out);
    }
}
