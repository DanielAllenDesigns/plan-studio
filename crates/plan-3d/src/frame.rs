//! A wall-local coordinate frame that emits axis-aligned faces and boxes in scene space.
//!
//! Local axes: `s` runs along the wall from its start, `t` across the wall
//! (positive on the left when walking start to end), `h` up from the floor.

use crate::builder::{MeshBuilder, V3};
use plan_core::{Point, Wall};

/// Inches per foot, for UV scaling.
const IN_PER_FT: f64 = 12.0;

/// One of the three wall-local axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    S,
    T,
    H,
}

/// Map a plan point and elevation (inches) to scene space: X = x, Y = up, Z = -y.
pub fn to_scene(p: Point, elevation: f64) -> V3 {
    [p.x as f32, elevation as f32, (-p.y) as f32]
}

/// Wall-local frame positioned at a floor elevation.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    origin: Point,
    dir: Point,
    nrm: Point,
    elevation: f64,
}

impl Frame {
    pub fn new(wall: &Wall, elevation: f64) -> Self {
        Self {
            origin: wall.start,
            dir: wall.direction(),
            nrm: wall.normal(),
            elevation,
        }
    }

    /// Scene-space position of local `(s, t, h)`.
    pub fn point(&self, s: f64, t: f64, h: f64) -> V3 {
        let p = self.origin + self.dir * s + self.nrm * t;
        to_scene(p, self.elevation + h)
    }

    /// Unit scene-space direction of a local axis.
    fn axis_vec(&self, axis: Axis) -> V3 {
        match axis {
            Axis::S => [self.dir.x as f32, 0.0, -self.dir.y as f32],
            Axis::T => [self.nrm.x as f32, 0.0, -self.nrm.y as f32],
            Axis::H => [0.0, 1.0, 0.0],
        }
    }

    /// Emit a rectangle perpendicular to `axis` at coordinate `at`.
    ///
    /// `r1` and `r2` are the ranges along the two remaining axes, taken in the
    /// order S,T,H with `axis` removed. The quad faces `sign` along `axis`.
    pub fn face(
        &self,
        mesh: &mut MeshBuilder,
        axis: Axis,
        sign: f32,
        at: f64,
        r1: (f64, f64),
        r2: (f64, f64),
    ) {
        let local = |a: f64, b: f64| match axis {
            Axis::S => (at, a, b),
            Axis::T => (a, at, b),
            Axis::H => (a, b, at),
        };
        let corners = [(r1.0, r2.0), (r1.1, r2.0), (r1.1, r2.1), (r1.0, r2.1)];
        let pts = corners.map(|(a, b)| {
            let (s, t, h) = local(a, b);
            self.point(s, t, h)
        });
        let uv = corners.map(|(a, b)| [(a / IN_PER_FT) as f32, (b / IN_PER_FT) as f32]);
        let n = self.axis_vec(axis);
        mesh.quad(pts, uv, n.map(|c| c * sign));
    }

    /// Emit a closed box (six faces) spanning the given local ranges.
    pub fn cuboid(&self, mesh: &mut MeshBuilder, s: (f64, f64), t: (f64, f64), h: (f64, f64)) {
        self.face(mesh, Axis::S, -1.0, s.0, t, h);
        self.face(mesh, Axis::S, 1.0, s.1, t, h);
        self.face(mesh, Axis::T, -1.0, t.0, s, h);
        self.face(mesh, Axis::T, 1.0, t.1, s, h);
        self.face(mesh, Axis::H, -1.0, h.0, s, t);
        self.face(mesh, Axis::H, 1.0, h.1, s, t);
    }

    /// Emit a closed vertical prism over a convex footprint given as local
    /// `(s, t)` points (any winding), spanning the `h` range. Degenerate
    /// footprints emit nothing.
    pub fn prism(&self, mesh: &mut MeshBuilder, pts: &[(f64, f64)], h: (f64, f64)) {
        let n = pts.len();
        if n < 3 || h.1 - h.0 <= 1e-9 {
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
        let dir_of = |ns: f64, nt: f64| -> V3 {
            let v = self.dir * ns + self.nrm * nt;
            [v.x as f32, 0.0, -v.y as f32]
        };
        let (v0, v1) = ((h.0 / IN_PER_FT) as f32, (h.1 / IN_PER_FT) as f32);
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let (ds, dt) = (b.0 - a.0, b.1 - a.1);
            let len = ds.hypot(dt);
            if len <= 1e-9 {
                continue;
            }
            let normal = dir_of(dt / len, -ds / len);
            let quad = [
                self.point(a.0, a.1, h.0),
                self.point(b.0, b.1, h.0),
                self.point(b.0, b.1, h.1),
                self.point(a.0, a.1, h.1),
            ];
            let u = (len / IN_PER_FT) as f32;
            mesh.quad(quad, [[0.0, v0], [u, v0], [u, v1], [0.0, v1]], normal);
        }
        let uv = |p: (f64, f64)| [(p.0 / IN_PER_FT) as f32, (p.1 / IN_PER_FT) as f32];
        for i in 1..n - 1 {
            let tri = [poly[0], poly[i], poly[i + 1]];
            let uvs = tri.map(uv);
            mesh.tri(tri.map(|p| self.point(p.0, p.1, h.1)), uvs, [0.0, 1.0, 0.0]);
            mesh.tri(
                tri.map(|p| self.point(p.0, p.1, h.0)),
                uvs,
                [0.0, -1.0, 0.0],
            );
        }
    }
}
