//! Shared geometry: where the flights, landings and winders of a stair sit.
//!
//! Everything is expressed in a local `(u, v)` frame: `u` runs along the
//! first flight's direction of travel and `v` runs to the *right* of it. The
//! [`Frame`] maps that into plan space.

use crate::{effective_landing, solve, split, Stair, StairShape, Turn};
use plan_core::Point;

/// A point in the local `(u, v)` frame.
pub(crate) type Uv = (f64, f64);

/// Local-to-plan mapping: `u` along the first flight, `v` to its right.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frame {
    origin: Point,
    along: Point,
    right: Point,
}

impl Frame {
    fn new(origin: Point, direction: f64) -> Self {
        let along = Point::new(direction.cos(), direction.sin());
        Self {
            origin,
            along,
            right: Point::new(along.y, -along.x),
        }
    }

    /// Plan position of local `(u, v)`.
    pub(crate) fn point(&self, u: f64, v: f64) -> Point {
        self.origin + self.along * u + self.right * v
    }

    /// Plan position of a local point.
    pub(crate) fn uv(&self, p: Uv) -> Point {
        self.point(p.0, p.1)
    }

    /// Plan vector of a local displacement.
    pub(crate) fn vector(&self, d: Uv) -> Point {
        self.along * d.0 + self.right * d.1
    }
}

/// One straight run of steps (or a ramp when `risers == 0`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Flight {
    /// Left corner of the first riser.
    pub start: Uv,
    /// Unit direction of travel.
    pub dir: Uv,
    /// Height of the bottom of the first riser above the floor.
    pub base: f64,
    /// Risers in this flight; the last one rises onto the next platform.
    pub risers: u32,
    /// Treads in this flight (`risers - 1`).
    pub treads: u32,
    /// Length from the first to the last riser line.
    pub len: f64,
    /// Clear width.
    pub width: f64,
}

impl Flight {
    /// Unit vector to the right of the direction of travel.
    pub(crate) fn right(&self) -> Uv {
        (-self.dir.1, self.dir.0)
    }

    /// Local point `along` the flight and `lateral` from its left edge.
    pub(crate) fn at(&self, along: f64, lateral: f64) -> Uv {
        let r = self.right();
        (
            self.start.0 + self.dir.0 * along + r.0 * lateral,
            self.start.1 + self.dir.1 * along + r.1 * lateral,
        )
    }
}

/// A flat slab at a given height: a landing or one winder tread.
#[derive(Debug, Clone)]
pub(crate) struct Slab {
    pub poly: Vec<Uv>,
    /// Top surface height above the floor.
    pub top: f64,
}

/// A riser board between two winder treads.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TurnRiser {
    pub a: Uv,
    pub b: Uv,
    /// Thickness vector, pointing into the upper tread.
    pub thickness: Uv,
    /// Height of the bottom of the riser.
    pub base: f64,
}

/// The complete geometric description of a stair.
#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub frame: Frame,
    pub riser_height: f64,
    pub tread_depth: f64,
    pub flights: Vec<Flight>,
    /// Landing / winder-square outlines drawn in plan (between flights 0 and 1).
    pub outlines: Vec<Vec<Uv>>,
    /// Solid slabs (landing, winder wedges).
    pub slabs: Vec<Slab>,
    /// Winder riser boards.
    pub turn_risers: Vec<TurnRiser>,
    /// Outline of everything, in local coordinates.
    pub footprint: Vec<Uv>,
    /// Floor-to-floor rise actually built.
    pub total_rise: f64,
    /// Set for ramps.
    pub is_ramp: bool,
}

fn dedupe(mut pts: Vec<Uv>) -> Vec<Uv> {
    pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
    if pts.len() > 1 {
        let (f, l) = (pts[0], pts[pts.len() - 1]);
        if (f.0 - l.0).abs() < 1e-9 && (f.1 - l.1).abs() < 1e-9 {
            pts.pop();
        }
    }
    pts
}

impl Layout {
    pub(crate) fn build(stair: &Stair) -> Layout {
        let p = &stair.params;
        let sol = solve(p);
        let frame = Frame::new(stair.origin, stair.direction);
        let (w, t) = (p.width, p.tread_depth);

        if let StairShape::Ramp { .. } = p.shape {
            let flight = Flight {
                start: (0.0, 0.0),
                dir: (1.0, 0.0),
                base: 0.0,
                risers: 0,
                treads: 0,
                len: sol.total_run,
                width: w,
            };
            return Layout {
                frame,
                riser_height: 0.0,
                tread_depth: 0.0,
                flights: vec![flight],
                outlines: Vec::new(),
                slabs: Vec::new(),
                turn_risers: Vec::new(),
                footprint: vec![
                    (0.0, 0.0),
                    (sol.total_run, 0.0),
                    (sol.total_run, w),
                    (0.0, w),
                ],
                total_rise: p.total_rise.max(0.0),
                is_ramp: true,
            };
        }

        let h = sol.riser_height;
        let mut layout = Layout {
            frame,
            riser_height: h,
            tread_depth: t,
            flights: Vec::new(),
            outlines: Vec::new(),
            slabs: Vec::new(),
            turn_risers: Vec::new(),
            footprint: Vec::new(),
            total_rise: h * f64::from(sol.risers),
            is_ramp: false,
        };

        let Some(sp) = split(p, sol.risers) else {
            let treads = sol.risers - 1;
            let len = f64::from(treads) * t;
            layout.flights.push(Flight {
                start: (0.0, 0.0),
                dir: (1.0, 0.0),
                base: 0.0,
                risers: sol.risers,
                treads,
                len,
                width: w,
            });
            layout.footprint = vec![(0.0, 0.0), (len, 0.0), (len, w), (0.0, w)];
            return layout;
        };

        let left = p.turn == Turn::Left;
        let is_u = matches!(p.shape, StairShape::UShaped { .. });
        let u1 = f64::from(sp.t1) * t;
        let l2 = f64::from(sp.t2) * t;
        let ld = if sp.winders > 0 {
            w
        } else {
            effective_landing(p)
        };
        let landing_top = f64::from(sp.t1 + 1) * h;

        layout.flights.push(Flight {
            start: (0.0, 0.0),
            dir: (1.0, 0.0),
            base: 0.0,
            risers: sp.t1 + 1,
            treads: sp.t1,
            len: u1,
            width: w,
        });

        // Second flight: start corner, direction, and the landing rectangle.
        let (start2, dir2, landing, footprint): (Uv, Uv, [Uv; 4], Vec<Uv>) = match (is_u, left) {
            (false, true) => (
                (u1 + ld - w, 0.0),
                (0.0, -1.0),
                [(u1, 0.0), (u1 + ld, 0.0), (u1 + ld, w), (u1, w)],
                vec![
                    (0.0, 0.0),
                    (0.0, w),
                    (u1 + ld, w),
                    (u1 + ld, -l2),
                    (u1 + ld - w, -l2),
                    (u1 + ld - w, 0.0),
                ],
            ),
            (false, false) => (
                (u1 + ld, w),
                (0.0, 1.0),
                [(u1, 0.0), (u1 + ld, 0.0), (u1 + ld, w), (u1, w)],
                vec![
                    (0.0, 0.0),
                    (u1 + ld, 0.0),
                    (u1 + ld, w + l2),
                    (u1 + ld - w, w + l2),
                    (u1 + ld - w, w),
                    (0.0, w),
                ],
            ),
            (true, true) => (
                (u1, 0.0),
                (-1.0, 0.0),
                [(u1, -w), (u1 + ld, -w), (u1 + ld, w), (u1, w)],
                vec![
                    (0.0, w),
                    (u1 + ld, w),
                    (u1 + ld, -w),
                    (u1 - l2, -w),
                    (u1 - l2, 0.0),
                    (0.0, 0.0),
                ],
            ),
            (true, false) => (
                (u1, 2.0 * w),
                (-1.0, 0.0),
                [(u1, 0.0), (u1 + ld, 0.0), (u1 + ld, 2.0 * w), (u1, 2.0 * w)],
                vec![
                    (0.0, 0.0),
                    (u1 + ld, 0.0),
                    (u1 + ld, 2.0 * w),
                    (u1 - l2, 2.0 * w),
                    (u1 - l2, w),
                    (0.0, w),
                ],
            ),
        };

        let base2 = if sp.winders > 0 {
            f64::from(sp.t1 + sp.winders) * h
        } else {
            landing_top
        };
        layout.flights.push(Flight {
            start: start2,
            dir: dir2,
            base: base2,
            risers: sp.t2 + 1,
            treads: sp.t2,
            len: l2,
            width: w,
        });
        layout.outlines.push(landing.to_vec());
        layout.footprint = dedupe(footprint);

        if sp.winders == 0 {
            layout.slabs.push(Slab {
                poly: landing.to_vec(),
                top: landing_top,
            });
        } else {
            layout.add_winders(&sp, u1, w, left, p.riser_thickness);
        }
        layout
    }

    /// Fan the square landing into `winders` pie treads around the inside corner.
    fn add_winders(
        &mut self,
        sp: &crate::Split,
        u1: f64,
        w: f64,
        left: bool,
        riser_thickness: f64,
    ) {
        let n = f64::from(sp.winders);
        let mirror = |(u, v): Uv| if left { (u, v) } else { (u, w - v) };
        let pivot = (u1, 0.0);
        let phi = |k: u32| f64::from(k) * std::f64::consts::FRAC_PI_2 / n;
        let hit = |k: u32| {
            let (s, c) = phi(k).sin_cos();
            let reach = w / s.max(c);
            (pivot.0 + s * reach, pivot.1 + c * reach)
        };
        let corner = (u1 + w, w);
        for k in 0..sp.winders {
            let mut poly = vec![pivot, hit(k)];
            if phi(k) < std::f64::consts::FRAC_PI_4 - 1e-9
                && phi(k + 1) > std::f64::consts::FRAC_PI_4 + 1e-9
            {
                poly.push(corner);
            }
            poly.push(hit(k + 1));
            self.slabs.push(Slab {
                poly: poly.into_iter().map(mirror).collect(),
                top: f64::from(sp.t1 + 1 + k) * self.riser_height,
            });
        }
        for k in 1..sp.winders {
            let (s, c) = phi(k).sin_cos();
            let n_vec = (c * riser_thickness, -s * riser_thickness);
            let thickness = if left { n_vec } else { (n_vec.0, -n_vec.1) };
            self.turn_risers.push(TurnRiser {
                a: mirror(pivot),
                b: mirror(hit(k)),
                thickness,
                base: f64::from(sp.t1 + k) * self.riser_height,
            });
        }
    }
}
