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
    /// Distance of the walkline from the inside edge (half the width when
    /// the walkline is off).
    pub walk_off: f64,
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
        self.inner + self.walk_off
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
    /// Set for curved ramps.
    pub ramp_arc: Option<RampArc>,
}

/// A curved ramp: the arc it sweeps and its runs and landings along it.
#[derive(Debug, Clone)]
pub(crate) struct RampArc {
    /// The arc (`treads` is 1 and `step` the whole sweep).
    pub curve: Curve,
    /// Runs and landings in order: `(from angle, to angle, height at the
    /// start above the floor, rise over the segment)`; a landing has no
    /// rise.
    pub segs: Vec<(f64, f64, f64, f64)>,
}

impl RampArc {
    /// Height of the surface at angle `a` along the arc.
    pub(crate) fn height_at(&self, a: f64) -> f64 {
        for &(a0, a1, base, rise) in &self.segs {
            if a <= a1 + 1e-9 {
                let t = if a1 > a0 { ((a - a0) / (a1 - a0)).clamp(0.0, 1.0) } else { 0.0 };
                return base + rise * t;
            }
        }
        self.segs.last().map_or(0.0, |&(_, _, b, r)| b + r)
    }
}

/// Points per half-ellipse of a flared tread.
const APRON_STEPS: usize = 10;

/// The outline `(s, lateral)` of a flared or bullnosed bottom tread (the
/// apron): the straight tread of depth `tread` (plus the `nosing` in front)
/// and `width` wide, with a half-ellipse cap on each side that reaches
/// `reach.0` past the left edge and `reach.1` past the right one (a reach of
/// zero is a straight end). Convex. Measured along the flight from the first
/// riser line.
pub(crate) fn apron_outline(tread: f64, nosing: f64, width: f64, reach: (f64, f64)) -> Vec<Uv> {
    let a = (tread + nosing) * 0.5;
    let mid = (tread - nosing) * 0.5;
    let mut pts = Vec::with_capacity(2 * APRON_STEPS + 4);
    if reach.0 > 1e-9 {
        for i in 0..=APRON_STEPS {
            let phi = std::f64::consts::PI * i as f64 / APRON_STEPS as f64;
            pts.push((mid + a * phi.cos(), -reach.0 * phi.sin()));
        }
    } else {
        pts.push((mid + a, 0.0));
        pts.push((mid - a, 0.0));
    }
    if reach.1 > 1e-9 {
        for i in 0..=APRON_STEPS {
            let phi = std::f64::consts::PI * (1.0 - i as f64 / APRON_STEPS as f64);
            pts.push((mid + a * phi.cos(), width + reach.1 * phi.sin()));
        }
    } else {
        pts.push((mid - a, width));
        pts.push((mid + a, width));
    }
    pts
}

impl Layout {
    /// The rounded starter treads (the apron of the bottom tread and, with
    /// two starters, the second one) as `(tread number, outline)`; empty
    /// when the stair has none, or is a ramp, landing or curve.
    pub(crate) fn aprons(&self, params: &crate::StairParams) -> Vec<(u32, Vec<Uv>)> {
        let Some(f) = self.flights.first() else {
            return Vec::new();
        };
        let reach = params.apron_reach();
        if (reach.0 <= 1e-9 && reach.1 <= 1e-9)
            || self.is_ramp
            || self.is_landing
            || self.curve.is_some()
            || f.treads == 0
        {
            return Vec::new();
        }
        let n = params.starter.count().max(1).min(f.treads);
        (1..=n)
            .map(|j| {
                let scale = f64::from(n - j + 1) / f64::from(n);
                let outline = apron_outline(
                    self.tread_depth,
                    params.nosing,
                    f.width,
                    (reach.0 * scale, reach.1 * scale),
                );
                let shift = f64::from(j - 1) * self.tread_depth;
                let pts = outline
                    .into_iter()
                    .map(|(s, lat)| f.at(s + shift, lat))
                    .collect();
                (j, pts)
            })
            .collect()
    }

    /// How far the flared sides of the stair stand out past its edges at
    /// distance `s` along flight `i`: `(left, right)`.
    pub(crate) fn reach(&self, params: &crate::StairParams, i: usize, s: f64) -> (f64, f64) {
        let fl = &params.flare_shape;
        if fl.corners.iter().all(|c| c.abs() < 1e-9)
            || self.is_ramp
            || self.is_landing
            || self.curve.is_some()
        {
            return (0.0, 0.0);
        }
        let Some(f) = self.flights.get(i) else {
            return (0.0, 0.0);
        };
        let ease = |x: f64| {
            let x = x.clamp(0.0, 1.0);
            (1.0 - fl.soften.clamp(0.0, 1.0)) * x + fl.soften.clamp(0.0, 1.0) * x * x
        };
        let zone = if fl.start > 1e-9 {
            f.len * fl.start.min(1.0)
        } else {
            f.len
        }
        .max(1e-9);
        let (mut l, mut r) = (0.0, 0.0);
        if i == 0 {
            let x = ease(1.0 - s / zone);
            l += fl.corners[0].max(0.0) * x;
            r += fl.corners[1].max(0.0) * x;
        }
        if i + 1 == self.flights.len() {
            let x = ease(1.0 - (f.len - s) / zone);
            l += fl.corners[2].max(0.0) * x;
            r += fl.corners[3].max(0.0) * x;
        }
        (l, r)
    }

    /// The bulge of the front edge of tread `j` (1 is the bottom one) of
    /// flight `i`: positive curves it down the stair.
    pub(crate) fn bulge(&self, params: &crate::StairParams, i: usize, j: u32) -> f64 {
        let fl = &params.flare_shape;
        let mut b = fl.curve_all;
        if i == 0 && j <= 2 {
            b += fl.curve_bottom;
        }
        b
    }

    /// The line across flight `i` at distance `s`, with its flared ends and
    /// its bulge: two points when it is straight, else a chain.
    pub(crate) fn across(
        &self,
        params: &crate::StairParams,
        i: usize,
        s: f64,
        bulge: f64,
    ) -> Vec<Uv> {
        let f = &self.flights[i];
        let (l, r) = self.reach(params, i, s);
        if bulge.abs() < 1e-9 {
            return vec![f.at(s, -l), f.at(s, f.width + r)];
        }
        const N: usize = 8;
        let (lo, hi) = (-l, f.width + r);
        let (mid, half) = ((lo + hi) / 2.0, ((hi - lo) / 2.0).max(1e-9));
        (0..=N)
            .map(|k| {
                let lat = lo + (hi - lo) * k as f64 / N as f64;
                let off = bulge * (1.0 - ((lat - mid) / half).powi(2));
                f.at(s - off, lat)
            })
            .collect()
    }

    /// One side of flight `i` from its start to distance `len`, following
    /// the flare.
    pub(crate) fn edge_chain(
        &self,
        params: &crate::StairParams,
        i: usize,
        len: f64,
        right: bool,
    ) -> Vec<Uv> {
        let f = &self.flights[i];
        let flared = params.flare_shape.corners.iter().any(|c| c.abs() > 1e-9);
        let steps = if flared { 10 } else { 1 };
        (0..=steps)
            .map(|k| {
                let s = len * k as f64 / steps as f64;
                let (l, r) = self.reach(params, i, s);
                f.at(s, if right { f.width + r } else { -l })
            })
            .collect()
    }
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

/// The layout of a curved ramp: runs of at most 30" of rise on the arc,
/// joined by flat landings 60" long at the walkline.
fn curved_ramp(frame: Frame, p: &crate::StairParams, slope: f64, inner: f64) -> Layout {
    let w = p.width;
    let left = p.turn == Turn::Left;
    let rise = p.total_rise.max(0.0);
    let runs = ramp_runs(rise);
    let run_rise = rise / f64::from(runs);
    let mut curve = Curve {
        center: if left { (0.0, -inner) } else { (0.0, w + inner) },
        inner,
        width: w,
        walk_off: w / 2.0,
        left,
        step: 1.0,
        treads: 1,
        risers: 0,
    };
    let walk = curve.walk().max(1e-9);
    let run_sweep = run_rise * slope.max(0.0) / walk;
    let landing_sweep = RAMP_LANDING / walk;
    let mut segs = Vec::new();
    let mut outlines = Vec::new();
    let mut slabs = Vec::new();
    let mut a = 0.0;
    for k in 0..runs {
        segs.push((a, a + run_sweep, f64::from(k) * run_rise, run_rise));
        a += run_sweep;
        if k + 1 < runs {
            let poly = arc_band(&curve, a, a + landing_sweep);
            segs.push((a, a + landing_sweep, f64::from(k + 1) * run_rise, 0.0));
            outlines.push(poly.clone());
            slabs.push(Slab {
                poly,
                top: f64::from(k + 1) * run_rise,
            });
            a += landing_sweep;
        }
    }
    curve.step = a.max(1e-9);
    let end = curve.at(a, curve.walk());
    let start = curve.at(0.0, curve.walk());
    let flight = Flight {
        start: (0.0, 0.0),
        dir: (1.0, 0.0),
        base: 0.0,
        risers: 0,
        treads: 0,
        len: ((end.0 - start.0).powi(2) + (end.1 - start.1).powi(2)).sqrt(),
        width: w,
        rise,
    };
    Layout {
        frame,
        riser_height: 0.0,
        tread_depth: 0.0,
        flights: vec![flight],
        outlines,
        slabs,
        turn_risers: Vec::new(),
        footprint: curve_footprint(&curve),
        total_rise: rise,
        is_ramp: true,
        is_landing: false,
        curve: None,
        ramp_arc: Some(RampArc { curve, segs }),
    }
}

/// The band of an arc between two angles, as a polygon: the outer edge
/// out, the inner edge back.
pub(crate) fn arc_band(c: &Curve, a0: f64, a1: f64) -> Vec<Uv> {
    let n = (((a1 - a0).abs().to_degrees() / 7.5).ceil() as usize).max(1);
    let mut pts = Vec::new();
    for i in 0..=n {
        pts.push(c.at_lat(a0 + (a1 - a0) * i as f64 / n as f64, 0.0));
    }
    for i in (0..=n).rev() {
        pts.push(c.at_lat(a0 + (a1 - a0) * i as f64 / n as f64, c.width));
    }
    dedupe(pts)
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
                ramp_arc: None,
            };
        }

        if let (StairShape::Ramp { slope_1_in }, Some(inner)) = (p.shape, p.ramp_curve) {
            return curved_ramp(frame, p, slope_1_in, inner.max(0.0));
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
                ramp_arc: None,
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
            ramp_arc: None,
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
                walk_off: p.walk_offset(),
                left,
                step: t / (inner + p.walk_offset()).max(1e-9),
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
        let gap = if is_u { p.u_gap.max(0.0) } else { 0.0 };
        let split_landing = is_u && p.split_landing && sp.winders == 0;

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
                (u1, -gap),
                (-1.0, 0.0),
                [
                    (u1, -w - gap),
                    (u1 + ld, -w - gap),
                    (u1 + ld, w),
                    (u1, w),
                ],
                if gap > 1e-9 {
                    vec![
                        (0.0, w),
                        (u1 + ld, w),
                        (u1 + ld, -w - gap),
                        (u1 - l2, -w - gap),
                        (u1 - l2, -gap),
                        (u1, -gap),
                        (u1, 0.0),
                        (0.0, 0.0),
                    ]
                } else {
                    vec![
                        (0.0, w),
                        (u1 + ld, w),
                        (u1 + ld, -w),
                        (u1 - l2, -w),
                        (u1 - l2, 0.0),
                        (0.0, 0.0),
                    ]
                },
            ),
            (true, false) => (
                (u1, 2.0 * w + gap),
                (-1.0, 0.0),
                [
                    (u1, 0.0),
                    (u1 + ld, 0.0),
                    (u1 + ld, 2.0 * w + gap),
                    (u1, 2.0 * w + gap),
                ],
                if gap > 1e-9 {
                    vec![
                        (0.0, 0.0),
                        (u1 + ld, 0.0),
                        (u1 + ld, 2.0 * w + gap),
                        (u1 - l2, 2.0 * w + gap),
                        (u1 - l2, w + gap),
                        (u1, w + gap),
                        (u1, w),
                        (0.0, w),
                    ]
                } else {
                    vec![
                        (0.0, 0.0),
                        (u1 + ld, 0.0),
                        (u1 + ld, 2.0 * w),
                        (u1 - l2, 2.0 * w),
                        (u1 - l2, w),
                        (0.0, w),
                    ]
                },
            ),
        };

        // A split landing is two platforms side by side, the second one
        // riser higher; the second flight starts from it.
        let landing_top2 = landing_top + h;
        let base2 = if sp.winders > 0 {
            f64::from(sp.t1 + sp.winders) * h
        } else if split_landing {
            landing_top2
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
        layout.footprint = dedupe(footprint);

        if sp.winders == 0 && split_landing {
            // The first platform continues flight 1, the second flight 2;
            // the strip of gap between them is open.
            let (first, second): ([Uv; 4], [Uv; 4]) = if left {
                (
                    [(u1, 0.0), (u1 + ld, 0.0), (u1 + ld, w), (u1, w)],
                    [
                        (u1, -w - gap),
                        (u1 + ld, -w - gap),
                        (u1 + ld, -gap),
                        (u1, -gap),
                    ],
                )
            } else {
                (
                    [(u1, 0.0), (u1 + ld, 0.0), (u1 + ld, w), (u1, w)],
                    [
                        (u1, w + gap),
                        (u1 + ld, w + gap),
                        (u1 + ld, 2.0 * w + gap),
                        (u1, 2.0 * w + gap),
                    ],
                )
            };
            layout.outlines.push(first.to_vec());
            layout.outlines.push(second.to_vec());
            layout.slabs.push(Slab {
                poly: first.to_vec(),
                top: landing_top,
            });
            layout.slabs.push(Slab {
                poly: second.to_vec(),
                top: landing_top2,
            });
            if gap < 1e-9 {
                // The riser between the two platforms.
                let (a, b, th) = if left {
                    ((u1, 0.0), (u1 + ld, 0.0), (0.0, -p.riser_thickness))
                } else {
                    ((u1, w), (u1 + ld, w), (0.0, p.riser_thickness))
                };
                layout.turn_risers.push(TurnRiser {
                    a,
                    b,
                    thickness: th,
                    base: landing_top,
                });
            }
        } else {
            layout.outlines.push(landing.to_vec());
            if sp.winders == 0 {
                layout.slabs.push(Slab {
                    poly: landing.to_vec(),
                    top: landing_top,
                });
            } else {
                layout.add_winders(&sp, u1, w, left, p.riser_thickness, p.winder_contraction);
            }
        }
        layout
    }

    /// Fan the square landing into `winders` pie treads around the inside corner.
    ///
    /// `contraction` is the narrowest a tread may get at the inside corner
    /// (Max Tread Contraction): the points of the wedges are cut off so the
    /// inside end of every riser line is at least that wide. `0` keeps the
    /// points.
    fn add_winders(
        &mut self,
        sp: &crate::Split,
        u1: f64,
        w: f64,
        left: bool,
        riser_thickness: f64,
        contraction: f64,
    ) {
        let n = f64::from(sp.winders);
        let mirror = |(u, v): Uv| if left { (u, v) } else { (u, w - v) };
        let pivot = (u1, 0.0);
        let phi = |k: u32| f64::from(k) * std::f64::consts::FRAC_PI_2 / n;
        // Radius of the quarter circle the points are cut at, so the chord
        // of one wedge is `contraction` wide.
        let cut = if contraction > 1e-9 {
            (contraction / (2.0 * (std::f64::consts::FRAC_PI_2 / n / 2.0).sin()))
                .min(w * 0.45)
        } else {
            0.0
        };
        let inner = |k: u32| {
            let (s, c) = phi(k).sin_cos();
            (pivot.0 + s * cut, pivot.1 + c * cut)
        };
        let hit = |k: u32| {
            let (s, c) = phi(k).sin_cos();
            let reach = w / s.max(c);
            (pivot.0 + s * reach, pivot.1 + c * reach)
        };
        let corner = (u1 + w, w);
        for k in 0..sp.winders {
            let mut poly = if cut > 0.0 {
                vec![inner(k), hit(k)]
            } else {
                vec![pivot, hit(k)]
            };
            if phi(k) < std::f64::consts::FRAC_PI_4 - 1e-9
                && phi(k + 1) > std::f64::consts::FRAC_PI_4 + 1e-9
            {
                poly.push(corner);
            }
            poly.push(hit(k + 1));
            if cut > 0.0 {
                poly.push(inner(k + 1));
            }
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
                a: mirror(if cut > 0.0 { inner(k) } else { pivot }),
                b: mirror(hit(k)),
                thickness,
                base: f64::from(sp.t1 + k) * self.riser_height,
            });
        }
    }
}
