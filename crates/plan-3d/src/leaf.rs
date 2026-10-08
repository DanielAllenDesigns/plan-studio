//! A rotatable 2D frame in the wall's `(s, t)` plane for door leaves, window
//! sashes and projected window panels, plus lite-grid (glass pane) helpers.

use crate::builder::MeshBuilder;
use crate::frame::Frame;

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
}

/// A lite grid inside a rectangle: `cols` panes across by `rows` vertical.
#[derive(Debug, Clone, Copy)]
pub struct Grid {
    pub u: (f64, f64),
    pub h: (f64, f64),
    pub cols: u32,
    pub rows: u32,
}

impl Grid {
    /// Pane ranges `(u0, u1, h0, h1)`; counts shrink until panes are >= 1" wide.
    pub fn cells(&self) -> Vec<(f64, f64, f64, f64)> {
        let fit = |extent: f64, n: u32| {
            let mut n = n.clamp(1, 16);
            while n > 1 && (extent - MUNTIN * f64::from(n - 1)) / f64::from(n) < 1.0 {
                n -= 1;
            }
            n
        };
        let (w, hh) = (self.u.1 - self.u.0, self.h.1 - self.h.0);
        if w <= 0.0 || hh <= 0.0 {
            return Vec::new();
        }
        let (c, r) = (fit(w, self.cols), fit(hh, self.rows));
        let pw = (w - MUNTIN * f64::from(c - 1)) / f64::from(c);
        let ph = (hh - MUNTIN * f64::from(r - 1)) / f64::from(r);
        let mut cells = Vec::new();
        for j in 0..r {
            for i in 0..c {
                let u0 = self.u.0 + f64::from(i) * (pw + MUNTIN);
                let h0 = self.h.0 + f64::from(j) * (ph + MUNTIN);
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
        let cells = self.cells();
        let tt = (tt_center - thickness * 0.5, tt_center + thickness * 0.5);
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
}
