//! Shared geometry: where the flights, landings and winders of a stair sit.
//!
//! Everything is expressed in a local `(u, v)` frame: `u` runs along the
//! first flight's direction of travel and `v` runs to the *right* of it. The
//! [`Frame`] maps that into plan space.

use crate::{effective_landing, ramp_runs, solve, split, Stair, StairShape, Turn, RAMP_LANDING};
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

    /// Local `(u, v)` of a plan point.
    pub(crate) fn local(&self, p: Point) -> Uv {
        let d = p - self.origin;
        (d.dot(self.along), d.dot(self.right))
    }
}

/// A curved stair: treads fanned around `center` (local frame).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Curve {
    pub center: Uv,
    /// Radius of the inside edge.
    pub inner: f64,
    pub width: f64,
    /// Turning left (counter-clockwise in plan).
    pub left: bool,
    /// Angle between two riser lines, radians.
    pub step: f64,
    pub treads: u32,
    pub risers: u32,
}

impl Curve {
    /// Radius of the walking line (the middle of the stair).
    pub(crate) fn walk(&self) -> f64 {
        self.inner + self.width / 2.0
    }

    pub(crate) fn outer(&self) -> f64 {
        self.inner + self.width
    }

    /// Angle from the first to the last riser line.
    pub(crate) fn sweep(&self) -> f64 {
        self.step * f64::from(self.treads)
    }

    /// Radius at lateral offset `lat` from the left edge.
    pub(crate) fn rho(&self, lat: f64) -> f64 {
        if self.left {
            self.inner + lat
        } else {
            self.inner + self.width - lat
        }
    }

    /// The point at angle `a` from the first riser and radius `rho`.
    pub(crate) fn at(&self, a: f64, rho: f64) -> Uv {
        let (s, c) = a.sin_cos();
        let v = if self.left { c } else { -c };
        (self.center.0 + rho * s, self.center.1 + rho * v)
    }

    /// The point at angle `a` and lateral offset `lat` from the left edge.
    pub(crate) fn at_lat(&self, a: f64, lat: f64) -> Uv {
        self.at(a, self.rho(lat))
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
    /// Rise over the length (ramps only).
    pub rise: f64,
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
    /// Set for landings.
    pub is_landing: bool,
    /// Set for curved stairs.
    pub curve: Option<Curve>,
}

/// Outline of a curved stair: the outer arc out, the inner arc back.
fn curve_footprint(c: &Curve) -> Vec<Uv> {
    let n = ((c.sweep().to_degrees() / 7.5).ceil() as usize).max(1);
    let mut pts = Vec::new();
    // Lateral 0 is the left edge, `width` the right edge.
    for i in 0..=n {
        pts.push(c.at_lat(c.sweep() * i as f64 / n as f64, 0.0));
    }
    for i in (0..=n).rev() {
        pts.push(c.at_lat(c.sweep() * i as f64 / n as f64, c.width));
    }
    dedupe(pts)
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

        if let StairShape::Landing { depth } = p.shape {
            let poly: Vec<Uv> = if p.outline.len() >= 3 {
                p.outline.iter().map(|&q| frame.local(q)).collect()
            } else {
                vec![(0.0, 0.0), (depth, 0.0), (depth, w), (0.0, w)]
            };
            let top = p.total_rise.max(0.0);
            let flight = Flight {
                start: (0.0, 0.0),
                dir: (1.0, 0.0),
                base: top,
                risers: 0,
                treads: 0,
                len: depth,
                width: w,
                rise: 0.0,
            };
            return Layout {
                frame,
                riser_height: 0.0,
                tread_depth: 0.0,
                flights: vec![flight],
                outlines: Vec::new(),
                slabs: vec![Slab {
                    poly: poly.clone(),
                    top,
                }],
                turn_risers: Vec::new(),
                footprint: poly,
                total_rise: top,
                is_ramp: false,
                is_landing: true,
                curve: None,
            };
        }

        if let StairShape::Ramp { slope_1_in } = p.shape {
            let runs = ramp_runs(p.total_rise);
            let rise = p.total_rise.max(0.0);
            let run_rise = rise / f64::from(runs);
            let run_len = run_rise * slope_1_in.max(0.0);
            let mut flights = Vec::new();
            let mut slabs = Vec::new();
            let mut outlines = Vec::new();
            for k in 0..runs {
                let u0 = f64::from(k) * (run_len + RAMP_LANDING);
                flights.push(Flight {
                    start: (u0, 0.0),
                    dir: (1.0, 0.0),
                    base: f64::from(k) * run_rise,
                    risers: 0,
                    treads: 0,
                    len: run_len,
                    width: w,
                    rise: run_rise,
                });
                if k + 1 < runs {
                    let poly = vec![
                        (u0 + run_len, 0.0),
                        (u0 + run_len + RAMP_LANDING, 0.0),
                        (u0 + run_len + RAMP_LANDING, w),
                        (u0 + run_len, w),
                    ];
                    outlines.push(poly.clone());
                    slabs.push(Slab {
                        poly,
                        top: f64::from(k + 1) * run_rise,
                    });
                }
            }
            return Layout {
                frame,
                riser_height: 0.0,
                tread_depth: 0.0,
                flights,
                outlines,
                slabs,
                turn_risers: Vec::new(),
                footprint: vec![
                    (0.0, 0.0),
                    (sol.total_run, 0.0),
                    (sol.total_run, w),
                    (0.0, w),
                ],
                total_rise: rise,
                is_ramp: true,
                is_landing: false,
                curve: None,
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
            is_landing: false,
            curve: None,
        };

        if let StairShape::Curved { inner_radius } = p.shape {
            let inner = inner_radius.max(0.0);
            let treads = sol.risers - 1;
            let left = p.turn == Turn::Left;
            let curve = Curve {
                center: if left {
                    (0.0, -inner)
                } else {
                    (0.0, w + inner)
                },
                inner,
                width: w,
                left,
                step: t / (inner + w / 2.0).max(1e-9),
                treads,
                risers: sol.risers,
            };
            layout.footprint = curve_footprint(&curve);
            // One pseudo-flight along the chord keeps flight-based consumers sane.
            let end = curve.at(curve.sweep(), curve.walk());
            let start = curve.at(0.0, curve.walk());
            layout.flights.push(Flight {
                start: (0.0, 0.0),
                dir: (1.0, 0.0),
                base: 0.0,
                risers: sol.risers,
                treads,
                len: ((end.0 - start.0).powi(2) + (end.1 - start.1).powi(2)).sqrt(),
                width: w,
                rise: 0.0,
            });
            layout.curve = Some(curve);
            return layout;
        }

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
                rise: f64::from(treads) * h,
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
            rise: f64::from(sp.t1) * h,
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
            rise: f64::from(sp.t2) * h,
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
