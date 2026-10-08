//! Railings: flat guard railings, stair railings that follow the pitch line,
//! and the plan symbol.
//!
//! A railing is a wall-like object with a [`RailStyle`], newel posts at both
//! ends and at a maximum spacing, a top rail, and either a bottom rail or a
//! half-wall with a cap. All lengths are inches. Scene space is X right, Y up,
//! Z = -plan y (see `plan-3d`).

use crate::layout::{Curve, Layout};
use crate::model3d::{solid, V3};
use crate::{effective_landing, Stair, Stroke};
use plan_3d::{Material, Mesh};
use plan_core::{Id, Point};
use serde::{Deserialize, Serialize};

/// Largest clear opening between balusters, IRC R312.1.3 (4" sphere rule).
pub const MAX_BALUSTER_CLEAR: f64 = 4.0;
/// Guard height, IRC R312.1.2 (36").
pub const GUARD_HEIGHT: f64 = 36.0;
/// Handrail height above the nosing line on stairs (34"-38", IRC R311.7.8.1).
pub const STAIR_RAIL_HEIGHT: f64 = 34.0;

/// Gap between the floor and the underside of the bottom rail.
const BOTTOM_CLEARANCE: f64 = 3.0;
/// Thickness of a half-wall cap.
const CAP_HEIGHT: f64 = 1.5;
/// Cap overhang on each side of a half-wall or newel.
const CAP_OVERHANG: f64 = 0.5;
/// Half-wall thickness.
const HALF_WALL_THICKNESS: f64 = 5.5;
/// Thickness of a framed panel infill.
const PANEL_THICKNESS: f64 = 1.5;
/// Thickness of a solid infill.
const SOLID_THICKNESS: f64 = 3.5;
/// Thickness of a glass infill.
const GLASS_THICKNESS: f64 = 0.5;
/// Thickness of a stair half-wall.
const STAIR_HALF_WALL_THICKNESS: f64 = 5.5;
/// Thickness of a stair wall.
const STAIR_WALL_THICKNESS: f64 = 4.5;
/// Height of a stair wall above the nosing line (under the 80" headroom).
const STAIR_WALL_HEIGHT: f64 = 78.0;
/// Newels are placed along a curved stair at most this far apart.
const CURVE_NEWEL_SPACING: f64 = 96.0;
/// Cross-section of a cable run.
const CABLE_SIZE: f64 = 0.25;
/// Infill on stairs starts this far above the nosing line.
const STAIR_INFILL_LIFT: f64 = 2.0;
/// Newels closer than this in plan are the same newel.
const NEWEL_MERGE: f64 = 1.0;
/// Fallback maximum newel spacing.
const DEFAULT_NEWEL_SPACING: f64 = 96.0;

/// The infill between the rails.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RailStyle {
    /// Square balusters at a maximum clear spacing.
    Balusters {
        /// Maximum clear opening between balusters.
        spacing: f64,
        /// Baluster cross-section.
        size: f64,
    },
    /// Framed panels between newels.
    Panels,
    /// A solid infill between newels.
    Solid,
    /// Horizontal cables.
    Cable {
        /// Number of cable runs.
        rows: u32,
    },
    /// Glass panels between newels.
    Glass,
}

impl Default for RailStyle {
    fn default() -> Self {
        RailStyle::Balusters {
            spacing: MAX_BALUSTER_CLEAR,
            size: 1.5,
        }
    }
}

/// Newel post settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewelParams {
    /// Square post size.
    pub size: f64,
    /// Post height above the floor.
    pub height: f64,
    /// Add a cap block on top.
    pub cap: bool,
    /// Maximum centre-to-centre spacing on a flat railing.
    pub max_spacing: f64,
}

impl Default for NewelParams {
    fn default() -> Self {
        Self {
            size: 3.5,
            height: 40.0,
            cap: true,
            max_spacing: DEFAULT_NEWEL_SPACING,
        }
    }
}

/// The inputs of the Rails / Newels / Balusters tabs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RailingParams {
    /// Height of the top of the top rail above the floor (36" guard).
    pub height: f64,
    /// Top rail `(width, height)`.
    pub top_rail: (f64, f64),
    /// Bottom rail `(width, height)`; replaced by the cap when there is a half-wall.
    pub bottom_rail: (f64, f64),
    /// Newel posts.
    pub newel: NewelParams,
    /// Infill style.
    pub style: RailStyle,
    /// Height of a half-wall under the infill (without its cap), if any.
    pub half_wall: Option<f64>,
}

impl Default for RailingParams {
    fn default() -> Self {
        Self {
            height: GUARD_HEIGHT,
            top_rail: (3.5, 1.5),
            bottom_rail: (3.5, 1.5),
            newel: NewelParams::default(),
            style: RailStyle::default(),
            half_wall: None,
        }
    }
}

impl RailingParams {
    /// Height of the top of the lowest rail (bottom rail or half-wall cap):
    /// where the infill starts.
    fn infill_bottom(&self) -> f64 {
        match self.half_wall {
            Some(h) => h.max(0.0) + CAP_HEIGHT,
            None => BOTTOM_CLEARANCE + self.bottom_rail.1,
        }
    }

    fn max_spacing(&self) -> f64 {
        if self.newel.max_spacing > 1e-9 {
            self.newel.max_spacing
        } else {
            DEFAULT_NEWEL_SPACING
        }
    }
}

/// Which side of a stair, facing the direction of travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailSide {
    /// Left when facing the direction of travel.
    Left,
    /// Right when facing the direction of travel.
    Right,
}

/// Plan-level description of a flat railing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RailingGeometry {
    /// Newel centres (both ends, then every <= max spacing).
    pub newels: Vec<Point>,
    /// Baluster centres (empty unless the style is balusters).
    pub balusters: Vec<Point>,
    /// Rails as `(from, to, height of the rail's top surface)`: the top rail
    /// first, then the bottom rail (or the half-wall cap).
    pub rails: Vec<(Point, Point, f64)>,
}

/// Evenly spaced newel stations along a run of length `len`.
fn newel_stations(len: f64, params: &RailingParams) -> Vec<f64> {
    if len < 1e-9 {
        return vec![0.0];
    }
    let spans = (len / params.max_spacing() - 1e-9).ceil().max(1.0) as usize;
    (0..=spans).map(|i| len * i as f64 / spans as f64).collect()
}

/// Baluster centre offsets from the start of a span of centre-to-centre length
/// `span` between two newels of size `newel`: evenly spread, every clear
/// opening (including newel to baluster) at most `spacing`.
fn span_balusters(span: f64, newel: f64, spacing: f64, size: f64) -> Vec<f64> {
    let spacing = if spacing > 1e-9 {
        spacing
    } else {
        MAX_BALUSTER_CLEAR
    };
    let size = size.max(1e-3);
    let clear = span - newel;
    if clear <= spacing + 1e-9 {
        return Vec::new();
    }
    let n = ((clear - spacing) / (size + spacing) - 1e-9)
        .ceil()
        .max(1.0) as usize;
    let gap = ((clear - n as f64 * size) / (n as f64 + 1.0)).max(0.0);
    (0..n)
        .map(|i| newel / 2.0 + gap * (i as f64 + 1.0) + size * (i as f64 + 0.5))
        .collect()
}

/// Newel, baluster and rail layout of a straight railing from `start` to `end`.
///
/// Newels sit at both ends and are spread evenly so no span exceeds the
/// maximum spacing; balusters are spread evenly in each span with every clear
/// opening at most the style's spacing.
pub fn railing_segments(start: Point, end: Point, params: &RailingParams) -> RailingGeometry {
    let d = end - start;
    let len = d.length();
    let dir = d.normalized();
    let stations = newel_stations(len, params);
    let newels: Vec<Point> = stations.iter().map(|&s| start + dir * s).collect();
    let mut balusters = Vec::new();
    if let RailStyle::Balusters { spacing, size } = params.style {
        for w in stations.windows(2) {
            for o in span_balusters(w[1] - w[0], params.newel.size, spacing, size) {
                balusters.push(start + dir * (w[0] + o));
            }
        }
    }
    let rails = vec![
        (start, end, params.height),
        (start, end, params.infill_bottom()),
    ];
    RailingGeometry {
        newels,
        balusters,
        rails,
    }
}

// ---------------------------------------------------------------------------
// Mesh helpers
// ---------------------------------------------------------------------------

fn sc(p: Point, y: f64) -> V3 {
    [p.x, y, -p.y]
}

fn lateral(n: Point) -> V3 {
    [n.x, 0.0, -n.y]
}

fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: V3) -> Option<V3> {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (l > 1e-9).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

fn mul3(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn add3(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A rectangular bar from `a` to `b` with a `w` x `h` cross-section; `w` runs
/// along `lat` (a hint, any vector not parallel to the axis).
pub(crate) fn bar(
    a: V3,
    b: V3,
    lat: V3,
    (w, h): (f64, f64),
    material: Material,
    id: Option<Id>,
) -> Option<Mesh> {
    let axis = unit3(sub3(b, a))?;
    let up = unit3(cross3(axis, lat))?;
    let side = unit3(cross3(up, axis))?;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let corner = |sw: f64, sh: f64| add3(a, add3(mul3(side, sw * hw), mul3(up, sh * hh)));
    let profile = [
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
    ];
    Some(solid(&profile, sub3(b, a), material, id))
}

/// A vertical-plane slab: bottom/top heights at each end, centred on the line
/// `a`-`b`, `thick` thick.
struct Slab {
    a: Point,
    b: Point,
    a_range: (f64, f64),
    b_range: (f64, f64),
}

fn slab(s: &Slab, thick: f64, material: Material, id: Option<Id>) -> Option<Mesh> {
    if s.a_range.1 - s.a_range.0 < 0.01 && s.b_range.1 - s.b_range.0 < 0.01 {
        return None;
    }
    let d = (s.b - s.a).normalized();
    if d == Point::ZERO {
        return None;
    }
    let n = d.perp();
    let off = n * (-thick / 2.0);
    let (a, b) = (s.a + off, s.b + off);
    let profile = [
        sc(a, s.a_range.0),
        sc(b, s.b_range.0),
        sc(b, s.b_range.1),
        sc(a, s.a_range.1),
    ];
    Some(solid(&profile, mul3(lateral(n), thick), material, id))
}

/// A newel: shaft plus optional cap, `foot` above the floor, `lat` any
/// horizontal direction in scene space.
fn newel_meshes(
    at: Point,
    foot: f64,
    nw: &NewelParams,
    lat: V3,
    id: Option<Id>,
    out: &mut Vec<Mesh>,
) {
    let top = foot + nw.height;
    let shaft_top = if nw.cap { top - CAP_HEIGHT } else { top };
    let size = (nw.size, nw.size);
    out.extend(bar(
        sc(at, foot),
        sc(at, shaft_top),
        lat,
        size,
        Material::WallInterior,
        id,
    ));
    if nw.cap {
        let cap = nw.size + 2.0 * CAP_OVERHANG;
        out.extend(bar(
            sc(at, shaft_top),
            sc(at, top),
            lat,
            (cap, cap),
            Material::WallInterior,
            id,
        ));
    }
}

/// Infill between two rail endpoints. `lo`/`hi` are `(at a, at b)` heights.
fn infill(
    style: RailStyle,
    a: Point,
    b: Point,
    lo: (f64, f64),
    hi: (f64, f64),
    id: Option<Id>,
    out: &mut Vec<Mesh>,
) {
    let slab_of = |thick: f64, mat: Material, out: &mut Vec<Mesh>| {
        let s = Slab {
            a,
            b,
            a_range: (lo.0, hi.0),
            b_range: (lo.1, hi.1),
        };
        out.extend(slab(&s, thick, mat, id));
    };
    match style {
        RailStyle::Balusters { .. } => {}
        RailStyle::Panels => slab_of(PANEL_THICKNESS, Material::WallInterior, out),
        RailStyle::Solid => slab_of(SOLID_THICKNESS, Material::WallInterior, out),
        RailStyle::Glass => slab_of(GLASS_THICKNESS, Material::WindowGlass, out),
        RailStyle::Cable { rows } => {
            let n = (b - a).normalized().perp();
            for i in 0..rows {
                let f = f64::from(i + 1) / f64::from(rows + 1);
                let ya = lo.0 + (hi.0 - lo.0) * f;
                let yb = lo.1 + (hi.1 - lo.1) * f;
                out.extend(bar(
                    sc(a, ya),
                    sc(b, yb),
                    lateral(n),
                    (CABLE_SIZE, CABLE_SIZE),
                    Material::WindowFrame,
                    id,
                ));
            }
        }
    }
}

/// Meshes for one or more flat railing runs sharing newels at coincident ends.
pub(crate) fn run_meshes(
    runs: &[(Point, Point)],
    floor_elev: f64,
    params: &RailingParams,
    id: Option<Id>,
) -> Vec<Mesh> {
    let mut out = Vec::new();
    let mut newels: Vec<Point> = Vec::new();
    let (tw, th) = params.top_rail;
    for &(start, end) in runs {
        let geom = railing_segments(start, end, params);
        let d = (end - start).normalized();
        if d == Point::ZERO {
            continue;
        }
        let n = d.perp();
        let lat = lateral(n);
        let y = |h: f64| floor_elev + h;

        // Top rail, full length.
        out.extend(bar(
            sc(start, y(params.height - th / 2.0)),
            sc(end, y(params.height - th / 2.0)),
            lat,
            (tw, th),
            Material::WallInterior,
            id,
        ));
        // Bottom rail or half-wall with cap.
        match params.half_wall {
            Some(hw) if hw > 0.0 => {
                out.extend(slab(
                    &Slab {
                        a: start,
                        b: end,
                        a_range: (y(0.0), y(hw)),
                        b_range: (y(0.0), y(hw)),
                    },
                    HALF_WALL_THICKNESS,
                    Material::WallInterior,
                    id,
                ));
                out.extend(bar(
                    sc(start, y(hw + CAP_HEIGHT / 2.0)),
                    sc(end, y(hw + CAP_HEIGHT / 2.0)),
                    lat,
                    (HALF_WALL_THICKNESS + 2.0 * CAP_OVERHANG, CAP_HEIGHT),
                    Material::WallInterior,
                    id,
                ));
            }
            _ => {
                let (bw, bh) = params.bottom_rail;
                let c = y(BOTTOM_CLEARANCE + bh / 2.0);
                out.extend(bar(
                    sc(start, c),
                    sc(end, c),
                    lat,
                    (bw, bh),
                    Material::WallInterior,
                    id,
                ));
            }
        }

        // Infill.
        let lo = y(params.infill_bottom());
        let hi = y(params.height - th);
        if hi - lo > 0.01 {
            match params.style {
                RailStyle::Balusters { size, .. } => {
                    for &p in &geom.balusters {
                        out.extend(bar(
                            sc(p, lo),
                            sc(p, hi),
                            lat,
                            (size, size),
                            Material::WallInterior,
                            id,
                        ));
                    }
                }
                RailStyle::Cable { .. } => {
                    infill(params.style, start, end, (lo, lo), (hi, hi), id, &mut out)
                }
                _ => {
                    let inset = d * (params.newel.size / 2.0);
                    for w in geom.newels.windows(2) {
                        infill(
                            params.style,
                            w[0] + inset,
                            w[1] - inset,
                            (lo, lo),
                            (hi, hi),
                            id,
                            &mut out,
                        );
                    }
                }
            }
        }

        for &p in &geom.newels {
            if newels.iter().all(|q| q.dist(p) > NEWEL_MERGE) {
                newels.push(p);
                newel_meshes(p, y(0.0), &params.newel, lat, id, &mut out);
            }
        }
    }
    out
}

/// 3D meshes of a straight railing from `start` to `end` on a floor at
/// `floor_elev`.
pub fn railing_meshes(
    start: Point,
    end: Point,
    floor_elev: f64,
    params: &RailingParams,
) -> Vec<Mesh> {
    run_meshes(&[(start, end)], floor_elev, params, None)
}

// ---------------------------------------------------------------------------
// Stair railings
// ---------------------------------------------------------------------------

/// Geometry of a stair railing; every elevation is absolute (the stair's
/// floor elevation is included).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StairRailingGeometry {
    /// Newel centres in plan and the elevation of their foot.
    pub newels: Vec<(Point, f64)>,
    /// Balusters as plan position, foot elevation and top elevation.
    pub balusters: Vec<(Point, f64, f64)>,
    /// Top-rail runs as `(from, elevation of rail top, to, elevation of rail top)`.
    pub rails: Vec<(Point, f64, Point, f64)>,
}

impl StairRailingGeometry {
    fn add_newel(&mut self, p: Point, foot: f64) {
        match self
            .newels
            .iter_mut()
            .find(|(q, _)| q.dist(p) <= NEWEL_MERGE)
        {
            Some(existing) => existing.1 = existing.1.min(foot),
            None => self.newels.push((p, foot)),
        }
    }
}

/// Where a flight's rail starts and ends, for joining flights across landings.
struct FlightEnd {
    start: Point,
    e_start: f64,
    end: Point,
    e_end: f64,
    dir: Point,
    foot_top: f64,
}

/// The path joining the top of one flight's rail to the start of the next:
/// the corner point if the two rails meet at a corner.
fn landing_path(a: &FlightEnd, b: &FlightEnd, landing: f64) -> Vec<Point> {
    let (p, q) = (a.end, b.start);
    if p.dist(q) < NEWEL_MERGE {
        return vec![p];
    }
    let denom = a.dir.cross(b.dir);
    if denom.abs() > 1e-6 {
        let t = (q - p).cross(b.dir) / denom;
        if t > -1e-6 {
            return vec![p, p + a.dir * t, q];
        }
    } else if (q - p).cross(a.dir).abs() > 1.0 {
        // Parallel flights (U shape): guard around the far edge of the landing.
        return vec![p, p + a.dir * landing, q + a.dir * landing, q];
    }
    vec![p, q]
}

/// Newel, baluster and rail layout of a stair railing on one `side`.
///
/// The rail follows the pitch line: its top is [`STAIR_RAIL_HEIGHT`] above the
/// nosing line. Newels sit at the bottom and top of each flight and at the
/// landing corners (never at intermediate spacing). Balusters stand on the
/// treads, enough per tread to keep every clear opening within the style's
/// spacing. Rails across landings are linked to the flights by straight runs
/// that interpolate between the two rail elevations; a half-wall is ignored
/// on stairs.
pub fn stair_railing_geometry(
    stair: &Stair,
    side: RailSide,
    params: &RailingParams,
) -> StairRailingGeometry {
    let layout = Layout::build(stair);
    let sp = &stair.params;
    let floor = stair.bottom_elevation();
    let (h, t) = (layout.riser_height, layout.tread_depth);
    let mut g = StairRailingGeometry::default();
    if layout.is_landing {
        return g;
    }
    if let Some(c) = &layout.curve {
        curved_geometry(&layout, c, stair, side, params, &mut g);
        return g;
    }
    let mut ends: Vec<Option<FlightEnd>> = Vec::new();

    for f in &layout.flights {
        let lat = match side {
            RailSide::Left => 0.0,
            RailSide::Right => f.width,
        };
        let plan = |s: f64| layout.frame.uv(f.at(s, lat));
        // Surface height at s = 0, slope, and the nosing offset along the run.
        let (surf0, slope, nose, foot_top) = if layout.is_ramp {
            let slope = if f.len > 1e-9 { f.rise / f.len } else { 0.0 };
            (f.base, slope, 0.0, f.base + f.rise)
        } else {
            (
                f.base + h,
                h / t,
                sp.nosing,
                f.base + f64::from(f.risers) * h,
            )
        };
        let rail_top = |s: f64| floor + surf0 + slope * (s + nose) + STAIR_RAIL_HEIGHT;

        g.add_newel(plan(0.0), floor + surf0);
        if f.len < 1e-9 {
            ends.push(None);
            continue;
        }
        g.add_newel(plan(f.len), floor + foot_top);
        g.rails
            .push((plan(0.0), rail_top(0.0), plan(f.len), rail_top(f.len)));

        if let RailStyle::Balusters { spacing, size } = params.style {
            let below = params.top_rail.1;
            if layout.is_ramp {
                for o in span_balusters(f.len, params.newel.size, spacing, size) {
                    g.balusters
                        .push((plan(o), floor + f.base + slope * o, rail_top(o) - below));
                }
            } else {
                let per = ((t / (spacing.max(1e-3) + size)) - 1e-9).ceil().max(1.0) as u32;
                for j in 1..=f.treads {
                    for k in 0..per {
                        let s = f64::from(j - 1) * t + (f64::from(k) + 0.5) * t / f64::from(per);
                        let foot = floor + f.base + f64::from(j) * h;
                        g.balusters.push((plan(s), foot, rail_top(s) - below));
                    }
                }
            }
        }

        let dir = layout.frame.vector(f.dir).normalized();
        ends.push(Some(FlightEnd {
            start: plan(0.0),
            e_start: rail_top(0.0),
            end: plan(f.len),
            e_end: rail_top(f.len),
            dir,
            foot_top: floor + foot_top,
        }));
    }

    {
        let landing = effective_landing(sp);
        for pair in ends.windows(2) {
            let (Some(a), Some(b)) = (&pair[0], &pair[1]) else {
                continue;
            };
            let path = landing_path(a, b, landing);
            if path.len() < 2 {
                continue;
            }
            let total: f64 = path.windows(2).map(|w| w[0].dist(w[1])).sum();
            let mut run = 0.0;
            let elev = |run: f64| a.e_end + (b.e_start - a.e_end) * (run / total.max(1e-9));
            for w in path.windows(2) {
                let (r0, r1) = (run, run + w[0].dist(w[1]));
                g.rails.push((w[0], elev(r0), w[1], elev(r1)));
                run = r1;
            }
            for &c in &path[1..path.len() - 1] {
                g.add_newel(c, a.foot_top);
            }
        }
    }
    g
}

/// The plan paths of a side railing between flights: the rail that joins the
/// top of one flight to the start of the next across the landing or the turn
/// (a corner for an L stair, around the far edge for a U stair). One polyline
/// per pair of flights; empty for landings, ramps' straight joins and curved
/// stairs. The path starts at the end of the first flight's rail and ends at
/// the start of the next one's.
pub(crate) fn landing_rail_paths(stair: &Stair, side: RailSide) -> Vec<Vec<Point>> {
    let layout = Layout::build(stair);
    if layout.is_landing || layout.curve.is_some() {
        return Vec::new();
    }
    let ends: Vec<Option<FlightEnd>> = layout
        .flights
        .iter()
        .map(|f| {
            let lat = match side {
                RailSide::Left => 0.0,
                RailSide::Right => f.width,
            };
            (f.len >= 1e-9).then(|| FlightEnd {
                start: layout.frame.uv(f.at(0.0, lat)),
                e_start: 0.0,
                end: layout.frame.uv(f.at(f.len, lat)),
                e_end: 0.0,
                dir: layout.frame.vector(f.dir).normalized(),
                foot_top: 0.0,
            })
        })
        .collect();
    let landing = effective_landing(&stair.params);
    ends.windows(2)
        .filter_map(|pair| match (&pair[0], &pair[1]) {
            (Some(a), Some(b)) => Some(landing_path(a, b, landing)),
            _ => None,
        })
        .filter(|path| path.len() >= 2)
        .collect()
}

/// The edges of a landing's outline that face `side` of the direction of
/// travel (their outward normals within about 45 degrees of the side), in
/// plan. These are the open sides a landing railing guards; the ends, where
/// the flights arrive and leave, are not included. Empty for anything that
/// is not a landing.
pub fn landing_edges(stair: &Stair, side: RailSide) -> Vec<(Point, Point)> {
    let layout = Layout::build(stair);
    let Some(slab) = layout.slabs.first().filter(|_| layout.is_landing) else {
        return Vec::new();
    };
    let mut pts: Vec<Point> = slab.poly.iter().map(|&p| layout.frame.uv(p)).collect();
    if plan_core::geometry::polygon_area(&pts) < 0.0 {
        pts.reverse();
    }
    let travel = Point::new(stair.direction.cos(), stair.direction.sin());
    let left = travel.perp();
    let want = match side {
        RailSide::Left => left,
        RailSide::Right => left * -1.0,
    };
    let n = pts.len();
    (0..n)
        .filter_map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let d = (b - a).normalized();
            let outward = Point::new(d.y, -d.x);
            (a.dist(b) > 1e-6 && outward.dot(want) > 0.7).then_some((a, b))
        })
        .collect()
}

/// 3D meshes of the guard along the open sides of a landing: the Left and
/// Right sides as the stair's `left_side` / `right_side` ask (a railing, or
/// a half wall that is a railing on a half-height wall). A full wall is not
/// drawn on a landing. The rail stands on the landing's top.
pub(crate) fn landing_railing(stair: &Stair) -> Vec<Mesh> {
    let layout = Layout::build(stair);
    let Some(top) = layout
        .slabs
        .first()
        .map(|s| s.top)
        .filter(|_| layout.is_landing)
    else {
        return Vec::new();
    };
    let elevation = stair.bottom_elevation() + top;
    let mut out = Vec::new();
    for (side, kind) in [
        (RailSide::Left, stair.params.left_side),
        (RailSide::Right, stair.params.right_side),
    ] {
        let mut params = stair.params.railing_for(side);
        match kind {
            crate::SideKind::Railing => {}
            crate::SideKind::HalfWall => {
                params.half_wall = Some(params.half_wall.unwrap_or(GUARD_HEIGHT * 0.5));
            }
            _ => continue,
        }
        let runs = landing_edges(stair, side);
        out.extend(run_meshes(&runs, elevation, &params, Some(stair.id)));
    }
    out
}

/// Rail, newel and baluster layout along the edge of a curved stair.
fn curved_geometry(
    layout: &Layout,
    c: &Curve,
    stair: &Stair,
    side: RailSide,
    params: &RailingParams,
    g: &mut StairRailingGeometry,
) {
    let floor = stair.bottom_elevation();
    let h = layout.riser_height;
    let lat = match side {
        RailSide::Left => 0.0,
        RailSide::Right => c.width,
    };
    let rho = c.rho(lat);
    let plan = |a: f64| layout.frame.uv(c.at(a, rho));
    // The rail follows the steps: one level per riser line, rising at the
    // nosing line.
    let rail_top = |a: f64| floor + h + h * (a / c.step) + STAIR_RAIL_HEIGHT;
    let sweep = c.sweep();
    if c.treads == 0 || sweep < 1e-9 {
        return;
    }
    for k in 0..c.treads {
        let (a0, a1) = (c.step * f64::from(k), c.step * f64::from(k + 1));
        g.rails
            .push((plan(a0), rail_top(a0), plan(a1), rail_top(a1)));
    }
    g.add_newel(plan(0.0), floor + h);
    g.add_newel(plan(sweep), floor + h * f64::from(c.treads + 1));
    // Intermediate newels along long curves.
    let arc = sweep * rho;
    let extra = (arc / CURVE_NEWEL_SPACING - 1e-9).ceil() as u32;
    for i in 1..extra {
        let a = sweep * f64::from(i) / f64::from(extra);
        let k = (a / c.step).floor();
        g.add_newel(plan(a), floor + h * (k + 1.0));
    }
    if let RailStyle::Balusters { spacing, size } = params.style {
        let below = params.top_rail.1;
        let tread_arc = c.step * rho;
        let per = ((tread_arc / (spacing.max(1e-3) + size)) - 1e-9)
            .ceil()
            .max(1.0) as u32;
        for j in 1..=c.treads {
            for k in 0..per {
                let a = c.step * (f64::from(j - 1) + (f64::from(k) + 0.5) / f64::from(per));
                let foot = floor + h * f64::from(j);
                g.balusters.push((plan(a), foot, rail_top(a) - below));
            }
        }
    }
}

/// A wall or half-wall along one side of a stair: a sloped panel under a cap
/// rail (half-wall) or up to [`STAIR_WALL_HEIGHT`] above the nosing line
/// (wall), following the rails across landings and around curves.
pub fn stair_half_wall(
    stair: &Stair,
    side: RailSide,
    params: &RailingParams,
    full_height: bool,
) -> Vec<Mesh> {
    let solid_style = RailingParams {
        style: RailStyle::Solid,
        ..*params
    };
    let g = stair_railing_geometry(stair, side, &solid_style);
    let id = Some(stair.id);
    let depth = stair.params.stringer_depth;
    let (tw, th) = params.top_rail;
    let travel = Point::new(stair.direction.cos(), stair.direction.sin());
    let mut out = Vec::new();
    let thick = if full_height {
        STAIR_WALL_THICKNESS
    } else {
        STAIR_HALF_WALL_THICKNESS
    };
    for &(a, ea, b, eb) in &g.rails {
        let (na, nb) = (ea - STAIR_RAIL_HEIGHT, eb - STAIR_RAIL_HEIGHT);
        let (top_a, top_b) = if full_height {
            (na + STAIR_WALL_HEIGHT, nb + STAIR_WALL_HEIGHT)
        } else {
            (ea - th, eb - th)
        };
        let s = Slab {
            a,
            b,
            a_range: (na - depth, top_a),
            b_range: (nb - depth, top_b),
        };
        out.extend(slab(&s, thick, Material::WallInterior, id));
        if !full_height {
            let d = (b - a).normalized();
            if d != Point::ZERO {
                out.extend(bar(
                    sc(a, ea - th / 2.0),
                    sc(b, eb - th / 2.0),
                    lateral(d.perp()),
                    (tw.max(thick), th),
                    Material::WallInterior,
                    id,
                ));
            }
        }
    }
    if !full_height {
        for &(p, foot) in &g.newels {
            newel_meshes(p, foot, &params.newel, lateral(travel.perp()), id, &mut out);
        }
    }
    out
}

/// 3D meshes of the railing on one `side` of a stair; see
/// [`stair_railing_geometry`].
pub fn stair_railing(stair: &Stair, side: RailSide, params: &RailingParams) -> Vec<Mesh> {
    let g = stair_railing_geometry(stair, side, params);
    let id = Some(stair.id);
    let travel = Point::new(stair.direction.cos(), stair.direction.sin());
    let post_lat = lateral(travel);
    let (tw, th) = params.top_rail;
    let mut out = Vec::new();

    for &(p, foot) in &g.newels {
        newel_meshes(p, foot, &params.newel, post_lat, id, &mut out);
    }
    for &(p, foot, top) in &g.balusters {
        if let RailStyle::Balusters { size, .. } = params.style {
            out.extend(bar(
                sc(p, foot),
                sc(p, top),
                post_lat,
                (size, size),
                Material::WallInterior,
                id,
            ));
        }
    }
    for &(a, ea, b, eb) in &g.rails {
        let d = (b - a).normalized();
        let lat = lateral(d.perp());
        out.extend(bar(
            sc(a, ea - th / 2.0),
            sc(b, eb - th / 2.0),
            lat,
            (tw, th),
            Material::WallInterior,
            id,
        ));
        infill(
            params.style,
            a,
            b,
            (
                ea - STAIR_RAIL_HEIGHT + STAIR_INFILL_LIFT,
                eb - STAIR_RAIL_HEIGHT + STAIR_INFILL_LIFT,
            ),
            (ea - th, eb - th),
            id,
            &mut out,
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Plan symbol
// ---------------------------------------------------------------------------

/// Plan symbol of a flat railing: a thin double line (the top-rail width apart)
/// with a square at every newel.
pub fn plan_symbol_railing(start: Point, end: Point, params: &RailingParams) -> Vec<Stroke> {
    let geom = railing_segments(start, end, params);
    let d = (end - start).normalized();
    let n = d.perp();
    let half = params.top_rail.0 / 2.0;
    let mut out = Vec::new();
    if d != Point::ZERO {
        for side in [1.0, -1.0] {
            out.push(Stroke::Line(
                start + n * (half * side),
                end + n * (half * side),
            ));
        }
    }
    let r = params.newel.size / 2.0;
    let u = if d == Point::ZERO {
        Point::new(1.0, 0.0)
    } else {
        d
    };
    let v = u.perp();
    for c in geom.newels {
        out.push(Stroke::Polyline(
            vec![
                c + u * r + v * r,
                c - u * r + v * r,
                c - u * r - v * r,
                c + u * r - v * r,
            ],
            true,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{solve, StairParams, StairShape, Turn};

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn run(len: f64) -> (Point, Point) {
        (pt(10.0, 20.0), pt(10.0 + len, 20.0))
    }

    fn assert_unit_normals(meshes: &[Mesh]) {
        assert!(!meshes.is_empty());
        for m in meshes {
            assert_eq!(m.indices.len() % 3, 0);
            assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
            for v in &m.vertices {
                let n = v.normal;
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!((l - 1.0).abs() < 1e-4, "normal length {l}");
            }
        }
    }

    #[test]
    fn newel_counts_follow_max_spacing() {
        let p = RailingParams::default();
        for (len, count) in [(60.0, 2), (96.0, 2), (120.0, 3), (192.0, 3), (200.0, 4)] {
            let (a, b) = run(len);
            let g = railing_segments(a, b, &p);
            assert_eq!(g.newels.len(), count, "len {len}");
            assert!(g.newels[0].dist(a) < 1e-9 && g.newels[count - 1].dist(b) < 1e-9);
            for w in g.newels.windows(2) {
                assert!(w[0].dist(w[1]) <= 96.0 + 1e-9);
            }
        }
    }

    #[test]
    fn baluster_clear_spacing_is_at_most_four_inches() {
        let p = RailingParams::default();
        let RailStyle::Balusters { size, .. } = p.style else {
            panic!("default style is balusters");
        };
        for len in [20.0, 44.5, 96.0, 120.0, 250.0] {
            let (a, b) = run(len);
            let g = railing_segments(a, b, &p);
            assert!(!g.balusters.is_empty(), "len {len}");
            // (centre, half width) along the run, newels and balusters together.
            let mut items: Vec<(f64, f64)> = g
                .newels
                .iter()
                .map(|q| (q.x - a.x, p.newel.size / 2.0))
                .chain(g.balusters.iter().map(|q| (q.x - a.x, size / 2.0)))
                .collect();
            items.sort_by(|x, y| x.0.total_cmp(&y.0));
            for w in items.windows(2) {
                let clear = (w[1].0 - w[1].1) - (w[0].0 + w[0].1);
                assert!(clear > 0.0, "overlap at len {len}");
                assert!(clear <= 4.0 + 1e-9, "len {len}: clear {clear}");
            }
        }
    }

    #[test]
    fn rails_report_top_and_bottom_heights() {
        let (a, b) = run(60.0);
        let g = railing_segments(a, b, &RailingParams::default());
        assert_eq!(g.rails.len(), 2);
        assert!((g.rails[0].2 - 36.0).abs() < 1e-9);
        assert!((g.rails[1].2 - 4.5).abs() < 1e-9);
        let hw = RailingParams {
            half_wall: Some(18.0),
            ..RailingParams::default()
        };
        let g = railing_segments(a, b, &hw);
        assert!((g.rails[1].2 - 19.5).abs() < 1e-9);
    }

    #[test]
    fn non_baluster_styles_have_no_balusters() {
        let (a, b) = run(60.0);
        let p = RailingParams {
            style: RailStyle::Glass,
            ..RailingParams::default()
        };
        assert!(railing_segments(a, b, &p).balusters.is_empty());
    }

    #[test]
    fn every_style_meshes_with_unit_normals() {
        let (a, b) = run(120.0);
        for style in [
            RailStyle::default(),
            RailStyle::Panels,
            RailStyle::Solid,
            RailStyle::Cable { rows: 11 },
            RailStyle::Glass,
        ] {
            for half_wall in [None, Some(20.0)] {
                let p = RailingParams {
                    style,
                    half_wall,
                    ..RailingParams::default()
                };
                let meshes = railing_meshes(a, b, 100.0, &p);
                assert_unit_normals(&meshes);
                // Everything stands on the floor at 100" and reaches the newel tops.
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for m in &meshes {
                    let (l, h) = m.bounds().expect("non-empty");
                    lo = lo.min(l[1]);
                    hi = hi.max(h[1]);
                }
                assert!((lo - 100.0).abs() < 1e-3, "{style:?} floor {lo}");
                assert!((hi - 140.0).abs() < 1e-3, "{style:?} top {hi}");
            }
        }
    }

    #[test]
    fn cable_style_has_requested_rows() {
        let (a, b) = run(60.0);
        let mut p = RailingParams {
            style: RailStyle::Cable { rows: 11 },
            ..RailingParams::default()
        };
        let cables = railing_meshes(a, b, 0.0, &p)
            .iter()
            .filter(|m| m.material == Material::WindowFrame)
            .count();
        assert_eq!(cables, 11);
        p.style = RailStyle::Glass;
        let glass = railing_meshes(a, b, 0.0, &p)
            .iter()
            .filter(|m| m.material == Material::WindowGlass)
            .count();
        assert!(glass >= 1);
    }

    #[test]
    fn plan_symbol_is_double_line_with_newel_squares() {
        let (a, b) = run(120.0);
        let strokes = plan_symbol_railing(a, b, &RailingParams::default());
        let lines = strokes
            .iter()
            .filter(|s| matches!(s, Stroke::Line(..)))
            .count();
        let squares = strokes
            .iter()
            .filter(|s| matches!(s, Stroke::Polyline(pts, true) if pts.len() == 4))
            .count();
        assert_eq!((lines, squares), (2, 3));
        if let Stroke::Line(p, q) = &strokes[0] {
            assert!((p.y - 20.0 - 1.75).abs() < 1e-9 && (q.y - 20.0 - 1.75).abs() < 1e-9);
        }
    }

    fn straight_stair() -> Stair {
        Stair::new(7, pt(100.0, 50.0), 0.0, StairParams::default())
    }

    #[test]
    fn straight_stair_has_two_newels_and_rail_34_above_nosings() {
        let stair = straight_stair();
        let p = RailingParams::default();
        let sol = solve(&stair.params);
        let (h, t, nosing) = (sol.riser_height, sol.tread_depth, stair.params.nosing);
        for side in [RailSide::Left, RailSide::Right] {
            let g = stair_railing_geometry(&stair, side, &p);
            assert_eq!(g.newels.len(), 2, "{side:?}");
            assert_eq!(g.rails.len(), 1);
            let (a, ea, b, eb) = g.rails[0];
            let run = f64::from(sol.treads) * t;
            assert!((a.dist(b) - run).abs() < 1e-6);
            // Nosing tips: tread j (1-based) top is j*h at (j-1)*t - nosing from the first riser.
            for j in 1..=sol.treads {
                let s = f64::from(j - 1) * t - nosing;
                let nosing_y = f64::from(j) * h;
                let k = (s - 0.0) / run;
                let rail_y = ea + (eb - ea) * k;
                let off = rail_y - nosing_y;
                // rail top is 34" above the nosing tip, measured vertically.
                assert!((off - 34.0).abs() < 0.1, "tread {j}: {off}");
            }
        }
        // Left rail sits on the left edge, right rail one width over.
        let l = stair_railing_geometry(&stair, RailSide::Left, &p).rails[0];
        let r = stair_railing_geometry(&stair, RailSide::Right, &p).rails[0];
        assert!((l.0.y - r.0.y - 36.0).abs() < 1e-9);
    }

    #[test]
    fn stair_rail_follows_floor_elevation() {
        let mut stair = straight_stair();
        stair.floor_elevation = 96.0;
        let g = stair_railing_geometry(&stair, RailSide::Left, &RailingParams::default());
        let base =
            stair_railing_geometry(&straight_stair(), RailSide::Left, &RailingParams::default());
        assert!((g.rails[0].1 - base.rails[0].1 - 96.0).abs() < 1e-9);
        assert!((g.newels[0].1 - base.newels[0].1 - 96.0).abs() < 1e-9);
    }

    #[test]
    fn stair_balusters_stand_on_treads_with_small_gaps() {
        let stair = straight_stair();
        let p = RailingParams::default();
        let g = stair_railing_geometry(&stair, RailSide::Left, &p);
        let sol = solve(&stair.params);
        assert!(g.balusters.len() >= sol.treads as usize);
        let mut xs: Vec<f64> = g.balusters.iter().map(|b| b.0.x).collect();
        xs.sort_by(f64::total_cmp);
        for w in xs.windows(2) {
            assert!(w[1] - w[0] - 1.5 <= 4.0 + 1e-9, "{}", w[1] - w[0]);
        }
        for &(_, foot, top) in &g.balusters {
            assert!(top - foot > 20.0);
        }
    }

    #[test]
    fn l_shaped_stair_adds_landing_newels() {
        let params = StairParams {
            shape: StairShape::LShaped {
                treads_before_landing: 6,
            },
            turn: Turn::Left,
            ..StairParams::default()
        };
        let stair = Stair::new(3, pt(0.0, 0.0), 0.0, params);
        let p = RailingParams::default();
        for side in [RailSide::Left, RailSide::Right] {
            let g = stair_railing_geometry(&stair, side, &p);
            assert!(g.newels.len() > 2, "{side:?}: {}", g.newels.len());
            assert!(g.rails.len() >= 2);
            // Rails form one connected chain: every endpoint but the two ends is shared.
            let mut ends: Vec<Point> = g.rails.iter().flat_map(|r| [r.0, r.2]).collect();
            let mut loose = 0;
            while let Some(e) = ends.pop() {
                match ends.iter().position(|q| q.dist(e) < 1e-6) {
                    Some(i) => {
                        ends.swap_remove(i);
                    }
                    None => loose += 1,
                }
            }
            assert_eq!(loose, 2, "{side:?}: {:?}", g.rails);
        }
    }

    #[test]
    fn u_shaped_stair_rails_connect() {
        let params = StairParams {
            shape: StairShape::UShaped {
                treads_before_landing: 6,
            },
            turn: Turn::Right,
            ..StairParams::default()
        };
        let stair = Stair::new(3, pt(0.0, 0.0), 0.0, params);
        let g = stair_railing_geometry(&stair, RailSide::Left, &RailingParams::default());
        assert!(g.newels.len() >= 3);
        assert!(g.rails.len() >= 3);
    }

    #[test]
    fn stair_railing_meshes_have_unit_normals() {
        let stair = straight_stair();
        for style in [
            RailStyle::default(),
            RailStyle::Panels,
            RailStyle::Solid,
            RailStyle::Cable { rows: 11 },
            RailStyle::Glass,
        ] {
            let p = RailingParams {
                style,
                ..RailingParams::default()
            };
            let meshes = stair_railing(&stair, RailSide::Right, &p);
            assert_unit_normals(&meshes);
            assert!(meshes.iter().all(|m| m.object_id == Some(7)));
        }
        let ramp = Stair::new(
            1,
            pt(0.0, 0.0),
            0.5,
            StairParams {
                total_rise: 24.0,
                shape: StairShape::Ramp { slope_1_in: 12.0 },
                ..StairParams::default()
            },
        );
        let g = stair_railing_geometry(&ramp, RailSide::Left, &RailingParams::default());
        assert_eq!(g.newels.len(), 2);
        assert!(!g.balusters.is_empty());
        assert_unit_normals(&stair_railing(
            &ramp,
            RailSide::Left,
            &RailingParams::default(),
        ));
    }
}
