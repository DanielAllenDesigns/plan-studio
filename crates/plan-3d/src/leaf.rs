//! A rotatable 2D frame in the wall's `(s, t)` plane for door leaves, window
//! sashes and projected window panels, plus lite-grid (glass pane) helpers.

use crate::builder::{MeshBuilder, V3};
use crate::frame::Frame;
use plan_core::openings::{LiteStyle, OpeningSpec};

/// Muntin (lite divider) width, 7/8".
pub const MUNTIN: f64 = 0.875;
/// Glass pane thickness, 1/4".
pub const GLASS_THICKNESS: f64 = 0.25;

/// Local axes: `u` runs along `dir`, `tt` across the leaf along `nrm`
/// (`dir` rotated 90 degrees counter-clockwise), both in wall `(s, t)` space.
#[derive(Debug, Clone, Copy)]
pub struct Leaf {
    origin: (f64, f64),
    dir: (f64, f64),
    nrm: (f64, f64),
}

impl Leaf {
    /// A leaf at `origin` whose `u` axis points along `dir` (normalized here).
    pub fn new(origin: (f64, f64), dir: (f64, f64)) -> Self {
        let len = dir.0.hypot(dir.1).max(1e-12);
        let d = (dir.0 / len, dir.1 / len);
        Self {
            origin,
            dir: d,
            nrm: (-d.1, d.0),
        }
    }

    /// Wall `(s, t)` of leaf-local `(u, tt)`.
    pub fn st(&self, u: f64, tt: f64) -> (f64, f64) {
        (
            self.origin.0 + self.dir.0 * u + self.nrm.0 * tt,
            self.origin.1 + self.dir.1 * u + self.nrm.1 * tt,
        )
    }

    /// Unit `tt` axis in wall `(s, t)` space.
    pub fn normal(&self) -> (f64, f64) {
        self.nrm
    }

    /// Emit a box spanning the leaf-local ranges.
    pub fn boxed(
        &self,
        frame: &Frame,
        mesh: &mut MeshBuilder,
        u: (f64, f64),
        tt: (f64, f64),
        h: (f64, f64),
    ) {
        let pts = [
            self.st(u.0, tt.0),
            self.st(u.1, tt.0),
            self.st(u.1, tt.1),
            self.st(u.0, tt.1),
        ];
        frame.prism(mesh, &pts, h);
    }

    /// Emit a convex polygon given as leaf-local `(u, h)` points (any
    /// winding), extruded across `tt`: the diagonal muntins, the pieces of an
    /// arched frame and the strips filling the corners over an arch.
    pub fn slab(&self, frame: &Frame, mesh: &mut MeshBuilder, pts: &[(f64, f64)], tt: (f64, f64)) {
        let n = pts.len();
        if n < 3 || tt.1 - tt.0 <= 1e-9 {
            return;
        }
        let area: f64 = (0..n)
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum();
        if area.abs() < 1e-9 {
            return;
        }
        let mut poly = pts.to_vec();
        if area < 0.0 {
            poly.reverse();
        }
        let at = |u: f64, h: f64, t: f64| -> V3 {
            let (s, tw) = self.st(u, t);
            frame.point(s, tw, h)
        };
        // Scene-space unit vector of a leaf-local step.
        let step = |du: f64, dt: f64, dh: f64| -> V3 {
            let a = at(0.0, 0.0, 0.0);
            let b = at(du, dh, dt);
            let v = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
            [v[0] / l, v[1] / l, v[2] / l]
        };
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let (du, dh) = (b.0 - a.0, b.1 - a.1);
            let len = du.hypot(dh);
            if len <= 1e-9 {
                continue;
            }
            // Outward normal of a counter-clockwise polygon.
            let normal = step(dh / len, 0.0, -du / len);
            let quad = [
                at(a.0, a.1, tt.0),
                at(b.0, b.1, tt.0),
                at(b.0, b.1, tt.1),
                at(a.0, a.1, tt.1),
            ];
            mesh.quad(
                quad,
                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                normal,
            );
        }
        let front = step(0.0, 1.0, 0.0);
        let back = front.map(|c| -c);
        for i in 1..n - 1 {
            let tri = [poly[0], poly[i], poly[i + 1]];
            let uv = tri.map(|p| [(p.0 / 12.0) as f32, (p.1 / 12.0) as f32]);
            mesh.tri(tri.map(|p| at(p.0, p.1, tt.1)), uv, front);
            mesh.tri(tri.map(|p| at(p.0, p.1, tt.0)), uv, back);
        }
    }
}

/// A lite grid inside a rectangle: `cols` panes across by `rows` vertical,
/// or panes cut at explicit dividers, or one pane crossed by diagonals.
#[derive(Debug, Clone)]
pub struct Grid {
    pub u: (f64, f64),
    pub h: (f64, f64),
    pub cols: u32,
    pub rows: u32,
    /// Muntin width, inches.
    pub muntin: f64,
    /// Divider centers as fractions `0..1` across and up (prairie, custom);
    /// they replace `cols` and `rows`.
    pub cuts: Option<(Vec<f64>, Vec<f64>)>,
    /// Diamond lattice of `n x m` cells, drawn as diagonal muntins.
    pub diamond: Option<(u32, u32)>,
}

impl Grid {
    /// An even `cols x rows` grid of standard-width muntins.
    pub fn plain(u: (f64, f64), h: (f64, f64), cols: u32, rows: u32) -> Self {
        Self {
            u,
            h,
            cols,
            rows,
            muntin: MUNTIN,
            cuts: None,
            diamond: None,
        }
    }

    /// The grid of an opening's Lites tab over the rectangle `u` x `h`.
    pub fn for_spec(spec: &OpeningSpec, lites: (u32, u32), u: (f64, f64), h: (f64, f64)) -> Self {
        let (cols, rows) = (lites.0.max(1), lites.1.max(1));
        let muntin = spec.muntin_width.clamp(0.125, 4.0);
        let base = Self {
            muntin,
            ..Self::plain(u, h, 1, 1)
        };
        let (w, ht) = (u.1 - u.0, h.1 - h.0);
        match spec.lite_style {
            LiteStyle::Standard => Self { cols, rows, ..base },
            LiteStyle::Diamond => Self {
                diamond: Some((cols, rows)),
                ..base
            },
            LiteStyle::Prairie => {
                let m = (w.min(ht) * 0.2).clamp(2.0, 8.0);
                let cut = |extent: f64| {
                    if extent > 4.0 * m {
                        vec![m / extent, 1.0 - m / extent]
                    } else {
                        Vec::new()
                    }
                };
                Self {
                    cuts: Some((cut(w), cut(ht))),
                    ..base
                }
            }
            LiteStyle::Custom => Self {
                cuts: Some((spec.custom_across.clone(), spec.custom_up.clone())),
                ..base
            },
        }
    }

    /// Pane ranges `(u0, u1, h0, h1)`; counts shrink until panes are >= 1" wide.
    pub fn cells(&self) -> Vec<(f64, f64, f64, f64)> {
        let m = self.muntin;
        let (w, hh) = (self.u.1 - self.u.0, self.h.1 - self.h.0);
        if w <= 0.0 || hh <= 0.0 {
            return Vec::new();
        }
        if let Some((cu, ch)) = &self.cuts {
            let spans = |lo: f64, extent: f64, cuts: &[f64]| -> Vec<(f64, f64)> {
                let mut c: Vec<f64> = cuts
                    .iter()
                    .copied()
                    .filter(|f| *f > 0.0 && *f < 1.0)
                    .map(|f| lo + f * extent)
                    .collect();
                c.sort_by(f64::total_cmp);
                let mut out = Vec::new();
                let mut start = lo;
                for x in c {
                    let end = x - m * 0.5;
                    if end - start >= 1.0 {
                        out.push((start, end));
                        start = x + m * 0.5;
                    }
                }
                if lo + extent - start >= 1.0 {
                    out.push((start, lo + extent));
                }
                out
            };
            let (us, hs) = (spans(self.u.0, w, cu), spans(self.h.0, hh, ch));
            let mut cells = Vec::new();
            for (h0, h1) in &hs {
                for (u0, u1) in &us {
                    cells.push((*u0, *u1, *h0, *h1));
                }
            }
            return cells;
        }
        let fit = |extent: f64, n: u32| {
            let mut n = n.clamp(1, 16);
            while n > 1 && (extent - m * f64::from(n - 1)) / f64::from(n) < 1.0 {
                n -= 1;
            }
            n
        };
        let (c, r) = (fit(w, self.cols), fit(hh, self.rows));
        let pw = (w - m * f64::from(c - 1)) / f64::from(c);
        let ph = (hh - m * f64::from(r - 1)) / f64::from(r);
        let mut cells = Vec::new();
        for j in 0..r {
            for i in 0..c {
                let u0 = self.u.0 + f64::from(i) * (pw + m);
                let h0 = self.h.0 + f64::from(j) * (ph + m);
                cells.push((u0, u0 + pw, h0, h0 + ph));
            }
        }
        cells
    }

    /// One thin pane per cell, centered on `tt = 0` of the leaf.
    pub fn panes(&self, frame: &Frame, leaf: &Leaf, mesh: &mut MeshBuilder, tt_center: f64) {
        let g = GLASS_THICKNESS * 0.5;
        for (u0, u1, h0, h1) in self.cells() {
            leaf.boxed(
                frame,
                mesh,
                (u0, u1),
                (tt_center - g, tt_center + g),
                (h0, h1),
            );
        }
    }

    /// Muntins between the panes, `tt` thick, centered on `tt_center`.
    pub fn muntins(
        &self,
        frame: &Frame,
        leaf: &Leaf,
        mesh: &mut MeshBuilder,
        tt_center: f64,
        thickness: f64,
    ) {
        let tt = (tt_center - thickness * 0.5, tt_center + thickness * 0.5);
        if let Some((n, m)) = self.diamond {
            self.diagonals(frame, leaf, mesh, n, m, tt);
            return;
        }
        let cells = self.cells();
        let mut us: Vec<(f64, f64)> = cells.iter().map(|c| (c.0, c.1)).collect();
        us.sort_by(|a, b| a.0.total_cmp(&b.0));
        us.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
        for pair in us.windows(2) {
            leaf.boxed(frame, mesh, (pair[0].1, pair[1].0), tt, self.h);
        }
        let mut hs: Vec<(f64, f64)> = cells.iter().map(|c| (c.2, c.3)).collect();
        hs.sort_by(|a, b| a.0.total_cmp(&b.0));
        hs.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
        for pair in hs.windows(2) {
            leaf.boxed(frame, mesh, self.u, tt, (pair[0].1, pair[1].0));
        }
    }

    /// The diagonal bars of a diamond lattice of `n x m` cells: every line of
    /// slope `h/m : w/n` through the lattice points, both ways, clipped to the
    /// rectangle.
    fn diagonals(
        &self,
        frame: &Frame,
        leaf: &Leaf,
        mesh: &mut MeshBuilder,
        n: u32,
        m: u32,
        tt: (f64, f64),
    ) {
        let (w, hh) = (self.u.1 - self.u.0, self.h.1 - self.h.0);
        let (n, m) = (n.clamp(1, 12), m.clamp(1, 12));
        let (a, b) = (w / f64::from(n), hh / f64::from(m));
        let half = self.muntin * 0.5;
        for mirror in [false, true] {
            for k in -(i64::from(m) - 1)..=(i64::from(n) - 1) {
                let k = k as f64;
                // x = a (y / b + k) for y between the rectangle's edges.
                let y0 = (-k * b).max(0.0);
                let y1 = ((f64::from(n) - k) * b).min(hh);
                if y1 - y0 < 1.0 {
                    continue;
                }
                let pt = |y: f64| {
                    let x = a * (y / b + k);
                    let x = if mirror { w - x } else { x };
                    (self.u.0 + x, self.h.0 + y)
                };
                let (p, q) = (pt(y0), pt(y1));
                let len = (q.0 - p.0).hypot(q.1 - p.1);
                let (dx, dy) = ((q.0 - p.0) / len, (q.1 - p.1) / len);
                let off = (-dy * half, dx * half);
                leaf.slab(
                    frame,
                    mesh,
                    &[
                        (p.0 + off.0, p.1 + off.1),
                        (q.0 + off.0, q.1 + off.1),
                        (q.0 - off.0, q.1 - off.1),
                        (p.0 - off.0, p.1 - off.1),
                    ],
                    tt,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Material;
    use plan_core::{Point, Wall, WallKind};

    /// Every face of a slab looks away from its middle, and the winding
    /// agrees with the normal.
    #[test]
    fn a_slab_is_a_closed_outward_facing_solid() {
        let wall = Wall::new(
            Point::new(10.0, 20.0),
            Point::new(10.0, 220.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let frame = Frame::new(&wall, 0.0);
        let leaf = Leaf::new((30.0, 0.0), (1.0, 0.0));
        for poly in [
            vec![(0.0, 0.0), (10.0, 0.0), (10.0, 6.0), (0.0, 6.0)],
            // Clockwise input and a slanted bar.
            vec![(0.0, 0.0), (0.0, 3.0), (12.0, 15.0), (12.0, 12.0)],
            vec![(0.0, 0.0), (8.0, 0.0), (12.0, 5.0), (4.0, 9.0), (-2.0, 4.0)],
        ] {
            let mut mesh = MeshBuilder::new(Material::Trim);
            leaf.slab(&frame, &mut mesh, &poly, (-1.0, 1.0));
            let m = mesh.finish(None);
            let n = m.vertices.len() as f32;
            let mid: Vec3 = m.vertices.iter().fold([0.0; 3], |a, v| {
                [
                    a[0] + v.position[0] / n,
                    a[1] + v.position[1] / n,
                    a[2] + v.position[2] / n,
                ]
            });
            assert!(m.triangle_count() >= 2 + poly.len() * 2 - 2);
            for t in m.indices.chunks(3) {
                let p: Vec<Vec3> = t.iter().map(|i| m.vertices[*i as usize].position).collect();
                let nrm = m.vertices[t[0] as usize].normal;
                let c: Vec3 = [
                    (p[0][0] + p[1][0] + p[2][0]) / 3.0 - mid[0],
                    (p[0][1] + p[1][1] + p[2][1]) / 3.0 - mid[1],
                    (p[0][2] + p[1][2] + p[2][2]) / 3.0 - mid[2],
                ];
                assert!(
                    c[0] * nrm[0] + c[1] * nrm[1] + c[2] * nrm[2] > -1e-4,
                    "inward face {poly:?}"
                );
                let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
                let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
                let cross = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                assert!(
                    cross[0] * nrm[0] + cross[1] * nrm[1] + cross[2] * nrm[2] > 0.0,
                    "winding disagrees with the normal"
                );
            }
        }
    }

    type Vec3 = [f32; 3];
}
