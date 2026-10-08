//! Point location on a [`TerrainSurface`]: a uniform bucket index plus barycentric interpolation.

use plan_core::Point;

use crate::model::TerrainSurface;

/// Uniform grid of buckets, each listing the triangles whose bounding box overlaps it.
#[derive(Debug, Clone, Default)]
pub(crate) struct TriIndex {
    min: Point,
    cell: f64,
    cols: usize,
    rows: usize,
    buckets: Vec<Vec<u32>>,
}

impl TriIndex {
    pub(crate) fn build(vertices: &[[f64; 3]], triangles: &[[u32; 3]]) -> Self {
        if triangles.is_empty() {
            return TriIndex::default();
        }
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for v in vertices {
            lo = [lo[0].min(v[0]), lo[1].min(v[2])];
            hi = [hi[0].max(v[0]), hi[1].max(v[2])];
        }
        let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
        // About one triangle per bucket on average, with a floor against degenerate extents.
        let cell = ((w * h) / triangles.len() as f64)
            .sqrt()
            .max(w.max(h) / 1024.0)
            .max(1e-6);
        let cols = (w / cell).floor() as usize + 1;
        let rows = (h / cell).floor() as usize + 1;
        let mut idx = TriIndex {
            min: Point::new(lo[0], lo[1]),
            cell,
            cols,
            rows,
            buckets: vec![Vec::new(); cols * rows],
        };
        for (t, tri) in triangles.iter().enumerate() {
            let xs = tri.map(|i| vertices[i as usize][0]);
            let ys = tri.map(|i| vertices[i as usize][2]);
            let (c0, c1) = idx.col_range(min3(xs), max3(xs));
            let (r0, r1) = idx.row_range(min3(ys), max3(ys));
            for r in r0..=r1 {
                for c in c0..=c1 {
                    idx.buckets[r * cols + c].push(t as u32);
                }
            }
        }
        idx
    }

    fn col_range(&self, x0: f64, x1: f64) -> (usize, usize) {
        let f =
            |x: f64| (((x - self.min.x) / self.cell).floor().max(0.0) as usize).min(self.cols - 1);
        (f(x0), f(x1))
    }

    fn row_range(&self, y0: f64, y1: f64) -> (usize, usize) {
        let f =
            |y: f64| (((y - self.min.y) / self.cell).floor().max(0.0) as usize).min(self.rows - 1);
        (f(y0), f(y1))
    }

    /// Triangles that may contain `p`.
    fn candidates(&self, p: Point) -> &[u32] {
        if self.buckets.is_empty() {
            return &[];
        }
        let (c, r) = (
            ((p.x - self.min.x) / self.cell).floor(),
            ((p.y - self.min.y) / self.cell).floor(),
        );
        if c < 0.0 || r < 0.0 || c as usize >= self.cols || r as usize >= self.rows {
            return &[];
        }
        &self.buckets[r as usize * self.cols + c as usize]
    }
}

fn min3(v: [f64; 3]) -> f64 {
    v[0].min(v[1]).min(v[2])
}

fn max3(v: [f64; 3]) -> f64 {
    v[0].max(v[1]).max(v[2])
}

/// Barycentric coordinates of `p` in triangle `abc`; `None` for degenerate triangles.
pub(crate) fn barycentric(a: Point, b: Point, c: Point, p: Point) -> Option<[f64; 3]> {
    let denom = b.sub(a).cross(c.sub(a));
    if denom.abs() < 1e-12 {
        return None;
    }
    let l1 = p.sub(a).cross(c.sub(a)) / denom;
    let l2 = b.sub(a).cross(p.sub(a)) / denom;
    Some([1.0 - l1 - l2, l1, l2])
}

/// Elevation of the surface at plan point `p`, or `None` when no triangle covers it
/// (outside the perimeter or inside a terrain hole).
pub fn elevation_at(surface: &TerrainSurface, p: Point) -> Option<f64> {
    const EPS: f64 = -1e-9;
    surface.tri_index().candidates(p).iter().find_map(|&t| {
        let tri = surface.triangles[t as usize];
        let [a, b, c] = tri.map(|i| surface.vertices[i as usize]);
        let l = barycentric(
            Point::new(a[0], a[2]),
            Point::new(b[0], b[2]),
            Point::new(c[0], c[2]),
            p,
        )?;
        l.iter()
            .all(|&w| w >= EPS)
            .then(|| l[0] * a[1] + l[1] * b[1] + l[2] * c[1])
    })
}
