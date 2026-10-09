//! Gable lines and roof returns (Chief "Gable/Roof Line" and "Auto Roof
//! Return", RF-27 and RF-44).

use crate::geom::{self, V3};
use crate::{build_roof, EdgeKind, EdgeRoof, Roof, RoofPlane};
use plan_core::geometry::polygon_area;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Largest accepted gap between two baselines that should share a corner.
const JOIN_TOL: f64 = 1e-3;

/// Turn the hip edge `edge_index` (a plane's `source_edge`) of `roof` into a
/// gable: the plane disappears, the end becomes a vertical wall on that eave
/// line, and the neighbouring planes extend out to it.
///
/// The eave outline and the other edges' pitches are read back from the
/// planes' baselines, then the roof is rebuilt with the skeleton, so the result
/// is a normal [`build_roof`] roof with that edge set to [`EdgeKind::Gable`]
/// (overhang already included in the baselines). Plane `source_edge` values
/// keep their original meaning.
///
/// Returns `None` when `edge_index` has no plane, or when the outline cannot
/// be recovered from the planes alone: the planes must form one closed ring,
/// or one open chain whose gap is a single straight gable/shed edge (checked
/// by comparing the plane areas with the recovered outline). Build from the
/// footprint with [`build_roof`] in the other cases.
pub fn apply_gable_line(roof: &Roof, edge_index: usize) -> Option<Roof> {
    let planes = &roof.planes;
    let target = planes.iter().position(|p| p.source_edge == edge_index)?;
    let (outline, edges, orig) = recover_outline(planes)?;
    let want = polygon_area(&outline);
    let got: f64 = planes.iter().map(RoofPlane::projected_area).sum();
    if want <= 0.0 || (got - want).abs() > 1e-3 * want {
        return None;
    }
    let mut edges = edges;
    // `target` is a position in `planes`; the chain order may differ.
    let pos = orig.iter().position(|&o| o == planes[target].source_edge)?;
    edges[pos].kind = EdgeKind::Gable;
    let mut out = build_roof(&outline, &edges, roof.baseline_elevation);
    out.fascia_height = roof.fascia_height;
    out.approximate |= roof.approximate;
    for plane in &mut out.planes {
        plane.source_edge = *orig.get(plane.source_edge)?;
    }
    Some(out)
}

/// Eave outline (CCW), per-edge settings with zero overhang, and the original
/// `source_edge` of each outline edge (`usize::MAX` for a recovered gap edge).
type Recovered = (Vec<Point>, Vec<EdgeRoof>, Vec<usize>);

fn recover_outline(planes: &[RoofPlane]) -> Option<Recovered> {
    let m = planes.len();
    if m == 0 {
        return None;
    }
    let near = |a: Point, b: Point| a.dist(b) <= JOIN_TOL;
    let next =
        |k: usize| (0..m).find(|&j| j != k && near(planes[j].baseline.0, planes[k].baseline.1));
    let has_prev =
        |k: usize| (0..m).any(|j| j != k && near(planes[j].baseline.1, planes[k].baseline.0));

    let starts: Vec<usize> = (0..m).filter(|&k| !has_prev(k)).collect();
    let (first, closed) = match starts.as_slice() {
        [] => (0, true),
        [s] => (*s, false),
        _ => return None,
    };
    let mut order = vec![first];
    while let Some(n) = next(*order.last()?) {
        if n == first {
            break;
        }
        if order.contains(&n) || order.len() > m {
            return None;
        }
        order.push(n);
    }
    if order.len() != m {
        return None;
    }
    let mut outline: Vec<Point> = order.iter().map(|&k| planes[k].baseline.0).collect();
    let mut edges: Vec<EdgeRoof> = order
        .iter()
        .map(|&k| EdgeRoof {
            pitch_in_12: planes[k].pitch_in_12,
            kind: EdgeKind::Hip,
            overhang: 0.0,
        })
        .collect();
    let mut orig: Vec<usize> = order.iter().map(|&k| planes[k].source_edge).collect();
    if !closed {
        // Open chain: close with one gable edge from the last eave end back to
        // the first eave start.
        outline.push(planes[*order.last()?].baseline.1);
        edges.push(EdgeRoof {
            pitch_in_12: 8.0,
            kind: EdgeKind::Gable,
            overhang: 0.0,
        });
        orig.push(usize::MAX);
    }
    Some((outline, edges, orig))
}

/// Shape of a roof return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReturnKind {
    /// Quadrilateral continuing the main roof plane around the corner.
    Full,
    /// Triangular half of the full return.
    Half,
    /// Level (flat) boxed cornice return at eave height.
    Boxed,
}

/// Roof return settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReturnSpec {
    pub kind: ReturnKind,
    /// How far the return runs along the gable wall, and how far it projects
    /// past the corner along the eave direction, inches.
    pub length: f64,
}

/// A small plane at an eave corner.
#[derive(Debug, Clone, PartialEq)]
pub struct RoofReturn {
    pub kind: ReturnKind,
    /// The return as a roof plane: first edge on the eave line extended past
    /// the corner, normal up, `pitch_in_12` of the main plane for full and
    /// half returns and 0 for a boxed return.
    pub plane: RoofPlane,
    /// The eave corner the return wraps.
    pub corner: V3,
}

/// Roof return at the corner at the END of polygon edge `edge` of `plane`
/// (vertex `edge + 1`). See [`roof_return_at`].
pub fn roof_return(plane: &RoofPlane, edge: usize, spec: ReturnSpec) -> Option<RoofReturn> {
    roof_return_at(plane, edge, false, spec)
}

/// Roof return at an eave corner of `plane`.
///
/// `edge` is the polygon edge `polygon3d[edge] -> polygon3d[edge + 1]` lying on
/// the eave. The corner is its end vertex, or its start vertex when
/// `at_start`. From the corner, the return runs `length` along the adjacent
/// (rake or hip) edge's plan direction and projects `length` past the corner
/// along the eave line; heights follow the main plane (full, half) or stay at
/// eave height (boxed). Returns `None` for a degenerate plane, an
/// out-of-range edge, a non-positive length, or a vertical adjacent edge.
pub fn roof_return_at(
    plane: &RoofPlane,
    edge: usize,
    at_start: bool,
    spec: ReturnSpec,
) -> Option<RoofReturn> {
    let p = &plane.polygon3d;
    let n = p.len();
    if n < 3 || edge >= n || spec.length <= 1e-9 {
        return None;
    }
    let (p0, p1) = (p[edge], p[(edge + 1) % n]);
    let (corner, e_dir, adj) = if at_start {
        (
            p0,
            geom::to_plan(p0).sub(geom::to_plan(p1)),
            p[(edge + n - 1) % n],
        )
    } else {
        (
            p1,
            geom::to_plan(p1).sub(geom::to_plan(p0)),
            p[(edge + 2) % n],
        )
    };
    let e_out = e_dir.normalized();
    let r_in = geom::to_plan(adj).sub(geom::to_plan(corner)).normalized();
    if e_out == Point::ZERO || r_in == Point::ZERO {
        return None;
    }
    let l = spec.length;
    let c = geom::to_plan(corner);
    let (a, d) = (c, c.add(e_out.scale(l)));
    let (b, cc) = (
        c.add(r_in.scale(l)),
        c.add(e_out.scale(l)).add(r_in.scale(l)),
    );
    let height = |q: Point| match spec.kind {
        ReturnKind::Boxed => Some(corner[1]),
        _ => plane.height_at(q),
    };
    let lift = |q: Point| height(q).map(|y| geom::lift(q, y));
    let poly: Vec<V3> = match spec.kind {
        ReturnKind::Half => vec![lift(a)?, lift(d)?, lift(b)?],
        _ => vec![lift(a)?, lift(d)?, lift(cc)?, lift(b)?],
    };
    let poly = geom::up_eave_first(poly);
    let pitch = if spec.kind == ReturnKind::Boxed {
        0.0
    } else {
        plane.pitch_in_12
    };
    Some(RoofReturn {
        kind: spec.kind,
        plane: RoofPlane {
            baseline: (geom::to_plan(poly[0]), geom::to_plan(poly[1])),
            polygon3d: poly,
            pitch_in_12: pitch,
            source_edge: plane.source_edge,
        },
        corner,
    })
}


// ===================================================================
// Gable/Roof Line objects (RF-44, RF-133..RF-136)
// ===================================================================

/// How far from a wall's Main Layer a Gable/Roof Line may be drawn, inches
/// (ten feet, manual p. 862).
pub const GABLE_LINE_REACH: f64 = 120.0;
/// A gable over a door or window reaches this far past each side of it,
/// inches (p. 863).
pub const OPENING_GABLE_MARGIN: f64 = 12.0;
/// Two openings on one wall whose clear gap is at most this share a single
/// gable, inches (p. 863).
pub const OPENING_GABLE_MERGE: f64 = 30.0;
/// Shortest Gable/Roof Line, inches.
pub const MIN_GABLE_LINE: f64 = 12.0;
/// Largest cross product of the unit directions that still counts as
/// "exactly parallel" to a wall (about half a degree).
const PARALLEL_TOL: f64 = 0.01;

/// A Gable/Roof Line object: a line whose length is the width of the gable
/// at the wall's Main Layer. It stays in the plan until the next Build Roof,
/// which turns it into a gable (two roof planes of `pitch`, wider than the
/// line by `overhang` on each side), and it is kept until it is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GableLine {
    pub a: Point,
    pub b: Point,
    /// Rise per 12 of the two gable planes.
    pub pitch: f64,
    /// Overhang of the two planes past the line (and past the gable end),
    /// inches.
    pub overhang: f64,
}

impl Default for GableLine {
    /// Build Roof's stock values: 8:12 and a 16" overhang.
    fn default() -> Self {
        Self {
            a: Point::ZERO,
            b: Point::ZERO,
            pitch: 8.0,
            overhang: 16.0,
        }
    }
}

impl GableLine {
    pub fn new(a: Point, b: Point, pitch: f64, overhang: f64) -> Self {
        Self {
            a,
            b,
            pitch,
            overhang,
        }
    }

    pub fn length(&self) -> f64 {
        self.a.dist(self.b)
    }

    pub fn midpoint(&self) -> Point {
        Point::lerp(self.a, self.b, 0.5)
    }
}

/// An exterior wall as a Gable/Roof Line sees it: the centre line and the
/// thickness (the Main Layer is taken to be the whole thickness).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallFace {
    pub start: Point,
    pub end: Point,
    pub thickness: f64,
}

/// Why a Gable/Roof Line cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GableLineProblem {
    /// Shorter than [`MIN_GABLE_LINE`].
    TooShort,
    /// No exterior wall runs exactly parallel to the line.
    NotParallel,
    /// The nearest parallel wall's Main Layer is more than ten feet away.
    TooFar,
}

impl GableLineProblem {
    pub fn message(self) -> &'static str {
        match self {
            GableLineProblem::TooShort => "The Gable/Roof Line is too short",
            GableLineProblem::NotParallel => {
                "Draw the Gable/Roof Line exactly parallel to an exterior wall"
            }
            GableLineProblem::TooFar => {
                "The Gable/Roof Line must be within 10 feet of the wall's Main Layer"
            }
        }
    }
}

/// Checks `line` against the exterior `walls`: it must be exactly parallel
/// to one and within [`GABLE_LINE_REACH`] of its Main Layer. Returns the
/// index of the nearest such wall.
///
/// A line lying on the Main Layer (distance 0) is accepted: that is how an
/// alcove is covered and how a gable over an opening sits. Whether the line
/// touches a wall is not checked (Chief asks for a gap only when the line
/// is drawn by hand).
pub fn check_gable_line(line: &GableLine, walls: &[WallFace]) -> Result<usize, GableLineProblem> {
    if line.length() < MIN_GABLE_LINE {
        return Err(GableLineProblem::TooShort);
    }
    let u = line.b.sub(line.a).normalized();
    let mut best: Option<(usize, f64)> = None;
    let mut parallel = false;
    for (i, w) in walls.iter().enumerate() {
        let d = w.end.sub(w.start);
        if d.length() < 1e-6 || u.cross(d.normalized()).abs() > PARALLEL_TOL {
            continue;
        }
        parallel = true;
        let across = line.midpoint().sub(w.start).cross(d.normalized()).abs();
        let face = (across - w.thickness * 0.5).max(0.0);
        if face <= GABLE_LINE_REACH && best.is_none_or(|(_, f)| face < f) {
            best = Some((i, face));
        }
    }
    match best {
        Some((i, _)) => Ok(i),
        None if parallel => Err(GableLineProblem::TooFar),
        None => Err(GableLineProblem::NotParallel),
    }
}

/// A door or window on an exterior wall, for [`gable_lines_over_openings`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpeningSpan {
    pub wall_start: Point,
    pub wall_end: Point,
    /// Distance of the opening's centre from `wall_start` along the wall.
    pub offset: f64,
    pub width: f64,
    /// Unit plan vector from the wall's centre line to its outside.
    pub outward: Point,
    /// Distance from the centre line to the outside face, inches.
    pub face: f64,
}

/// The Gable/Roof Lines "Gable Over Door/Window" makes: one per opening,
/// [`OPENING_GABLE_MARGIN`] past each side of it, with openings on the same
/// wall whose clear gap is at most [`OPENING_GABLE_MERGE`] sharing one line.
/// The lines lie on the wall's outside face.
pub fn gable_lines_over_openings(
    spans: &[OpeningSpan],
    pitch: f64,
    overhang: f64,
) -> Vec<GableLine> {
    let same_wall = |a: &OpeningSpan, b: &OpeningSpan| {
        a.wall_start.dist(b.wall_start) < 0.5 && a.wall_end.dist(b.wall_end) < 0.5
    };
    let mut done = vec![false; spans.len()];
    let mut out = Vec::new();
    for i in 0..spans.len() {
        if done[i] {
            continue;
        }
        let mut group: Vec<&OpeningSpan> = spans
            .iter()
            .enumerate()
            .filter(|(j, s)| !done[*j] && same_wall(&spans[i], s))
            .map(|(_, s)| s)
            .collect();
        for (j, s) in spans.iter().enumerate() {
            if same_wall(&spans[i], s) {
                done[j] = true;
            }
        }
        group.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        let mut runs: Vec<(f64, f64, &OpeningSpan)> = Vec::new();
        for s in group {
            let (lo, hi) = (s.offset - s.width * 0.5, s.offset + s.width * 0.5);
            match runs.last_mut() {
                Some(r) if lo - r.1 <= OPENING_GABLE_MERGE => r.1 = r.1.max(hi),
                _ => runs.push((lo, hi, s)),
            }
        }
        for (lo, hi, s) in runs {
            let dir = s.wall_end.sub(s.wall_start).normalized();
            let at = |t: f64| s.wall_start.add(dir.scale(t)).add(s.outward.scale(s.face));
            out.push(GableLine::new(
                at(lo - OPENING_GABLE_MARGIN),
                at(hi + OPENING_GABLE_MARGIN),
                pitch,
                overhang,
            ));
        }
    }
    out
}

/// Where a plane of a gabled roof came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneOrigin {
    /// A piece of input plane `i` (the plane, or what a gable left of it).
    Main(usize),
    /// One of the two planes of Gable/Roof Line `i`.
    Wing(usize),
}

/// The roof planes after [`apply_gable_lines`].
#[derive(Debug, Clone, PartialEq)]
pub struct GabledRoof {
    pub planes: Vec<RoofPlane>,
    /// One per plane of `planes`.
    pub origin: Vec<PlaneOrigin>,
    /// Lines that did not reach the roof and were left out.
    pub skipped: Vec<usize>,
}

/// Height of a plane as `c + gx * x + gy * y` in plan coordinates.
#[derive(Debug, Clone, Copy)]
struct Affine {
    c: f64,
    gx: f64,
    gy: f64,
}

impl Affine {
    fn of(plane: &RoofPlane) -> Option<Affine> {
        let c = plane.height_at(Point::ZERO)?;
        Some(Affine {
            c,
            gx: plane.height_at(Point::new(1.0, 0.0))? - c,
            gy: plane.height_at(Point::new(0.0, 1.0))? - c,
        })
    }

    fn at(&self, p: Point) -> f64 {
        self.c + self.gx * p.x + self.gy * p.y
    }
}

/// `poly` cut to the side where `f` is at least zero (`f` affine).
fn keep_nonnegative(poly: &[Point], f: &dyn Fn(Point) -> f64) -> Vec<Point> {
    let n = poly.len();
    let mut out = Vec::with_capacity(n + 2);
    for i in 0..n {
        let (cur, prev) = (poly[i], poly[(i + n - 1) % n]);
        let (fc, fp) = (f(cur), f(prev));
        if fc >= 0.0 {
            if fp < 0.0 {
                out.push(Point::lerp(prev, cur, fp / (fp - fc)));
            }
            out.push(cur);
        } else if fp >= 0.0 {
            out.push(Point::lerp(prev, cur, fp / (fp - fc)));
        }
    }
    out
}

/// Convex `subject` minus convex counter-clockwise `clip`, as convex pieces.
fn minus_convex(subject: &[Point], clip: &[Point]) -> Vec<Vec<Point>> {
    let mut rest = subject.to_vec();
    let mut pieces = Vec::new();
    let m = clip.len();
    for k in 0..m {
        if rest.len() < 3 {
            break;
        }
        let (a, b) = (clip[k], clip[(k + 1) % m]);
        let side = move |p: Point| b.sub(a).cross(p.sub(a));
        let outside = keep_nonnegative(&rest, &|p| -side(p));
        if outside.len() >= 3 && polygon_area(&outside).abs() > MIN_PIECE {
            pieces.push(outside);
        }
        rest = keep_nonnegative(&rest, &side);
    }
    pieces
}

/// Pieces smaller than this are dropped, square inches.
const MIN_PIECE: f64 = 2.0;

/// `ring` without vertices that lie on the line of their neighbours.
fn without_collinear(ring: &[Point]) -> Vec<Point> {
    let n = ring.len();
    (0..n)
        .filter(|&i| {
            let (a, b, c) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
            b.sub(a).cross(c.sub(b)).abs() > 1e-6 * b.sub(a).length().max(1.0) * c.sub(b).length().max(1.0)
        })
        .map(|i| ring[i])
        .collect()
}

/// Joins counter-clockwise convex pieces that share an edge when their union
/// is convex too, so one plane does not end up as several tiles.
fn merge_convex(mut pieces: Vec<Vec<Point>>) -> Vec<Vec<Point>> {
    let near = |a: Point, b: Point| a.dist(b) < 1e-6;
    'again: loop {
        for i in 0..pieces.len() {
            for j in (i + 1)..pieces.len() {
                let (pi, pj) = (&pieces[i], &pieces[j]);
                let (ni, nj) = (pi.len(), pj.len());
                for x in 0..ni {
                    let (a, b) = (pi[x], pi[(x + 1) % ni]);
                    let Some(y) = (0..nj).find(|&y| near(pj[y], b) && near(pj[(y + 1) % nj], a))
                    else {
                        continue;
                    };
                    // pi from b around to a, then pj's vertices strictly
                    // between a and b.
                    let mut ring: Vec<Point> = (0..ni).map(|k| pi[(x + 1 + k) % ni]).collect();
                    ring.extend((2..nj).map(|k| pj[(y + k) % nj]));
                    let ring = without_collinear(&ring);
                    if ring.len() >= 3 && geom::is_convex(&ring) {
                        pieces[i] = ring;
                        pieces.remove(j);
                        continue 'again;
                    }
                }
            }
        }
        return pieces;
    }
}

/// The roof plane through `poly`'s plan outline, the first edge being the
/// lowest level one (the eave), counter-clockwise from above.
fn plane_over(poly: &[Point], height: &Affine, pitch: f64, source_edge: usize) -> Option<RoofPlane> {
    let mut ring = geom::ccw(poly);
    ring.dedup_by(|a, b| a.dist(*b) < 1e-6);
    if ring.len() >= 2 && ring[0].dist(ring[ring.len() - 1]) < 1e-6 {
        ring.pop();
    }
    if ring.len() < 3 || polygon_area(&ring).abs() < MIN_PIECE {
        return None;
    }
    let n = ring.len();
    let h = |p: Point| height.at(p);
    // Eave: the level edge that is lowest, else the lowest edge.
    let level = |i: usize| (h(ring[i]) - h(ring[(i + 1) % n])).abs() < 1e-6;
    let low = |i: usize| h(ring[i]).min(h(ring[(i + 1) % n]));
    let first = (0..n)
        .filter(|&i| level(i))
        .min_by(|&x, &y| low(x).total_cmp(&low(y)))
        .or_else(|| (0..n).min_by(|&x, &y| low(x).total_cmp(&low(y))))?;
    ring.rotate_left(first);
    let polygon3d: Vec<V3> = ring.iter().map(|&p| geom::lift(p, h(p))).collect();
    Some(RoofPlane {
        baseline: (ring[0], ring[1]),
        polygon3d,
        pitch_in_12: pitch,
        source_edge,
    })
}

/// Highest surface of `planes` over `p` (the roof is a height field), or
/// `None` where there is no roof.
fn surface_at(planes: &[(RoofPlane, PlaneOrigin)], p: Point) -> Option<f64> {
    planes
        .iter()
        .filter(|(pl, _)| plan_core::geometry::point_in_polygon(p, &pl.plan_polygon()))
        .filter_map(|(pl, _)| pl.height_at(p))
        .fold(None, |m: Option<f64>, h| Some(m.map_or(h, |m| m.max(h))))
}

/// Adds the gables of `lines` to a roof (`planes`, eaves at `eave_elevation`).
///
/// A gable is a small ridged roof over the line: two planes of the line's
/// pitch whose eaves run away from the line at `eave_elevation`, a ridge
/// above the line's midpoint, and a rake overhang past the line. Its height
/// above the plan is the lower envelope of its two planes; the finished roof
/// is the higher of that and the existing roof, so the gable's planes meet
/// the roof planes in valleys and the roof planes keep what is above the
/// gable. The line is on the side the roof is: the gable runs from the line
/// into the roof and stops where its ridge meets the roof surface.
///
/// A line that never reaches the roof is listed in
/// [`GabledRoof::skipped`] and changes nothing.
pub fn apply_gable_lines(
    planes: &[RoofPlane],
    eave_elevation: f64,
    lines: &[GableLine],
) -> GabledRoof {
    let mut work: Vec<(RoofPlane, PlaneOrigin)> = planes
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, p)| (p, PlaneOrigin::Main(i)))
        .collect();
    let mut skipped = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !add_gable(&mut work, eave_elevation, line, i) {
            skipped.push(i);
        }
    }
    let (planes, origin) = work.into_iter().unzip();
    GabledRoof {
        planes,
        origin,
        skipped,
    }
}

fn add_gable(
    work: &mut Vec<(RoofPlane, PlaneOrigin)>,
    base: f64,
    line: &GableLine,
    index: usize,
) -> bool {
    let len = line.length();
    if len < MIN_GABLE_LINE || line.pitch <= 0.0 || work.is_empty() {
        return false;
    }
    let u = line.b.sub(line.a).normalized();
    let c = line.midpoint();
    let ov = line.overhang.max(0.0);
    let half = len * 0.5 + ov;
    let k = line.pitch / 12.0;
    let extent = {
        let mut lo = Point::new(f64::MAX, f64::MAX);
        let mut hi = Point::new(f64::MIN, f64::MIN);
        for (pl, _) in work.iter() {
            for p in pl.plan_polygon() {
                lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
            }
        }
        lo.dist(hi) + len
    };
    // The side the roof is on: the nearer of the two along the midpoint.
    let reach = |n: Point| {
        let mut t = 0.0;
        while t <= extent {
            if surface_at(work, c.add(n.scale(t))).is_some() {
                return Some(t);
            }
            t += 1.0;
        }
        None
    };
    let (n1, n2) = (u.perp(), u.perp().scale(-1.0));
    let n = match (reach(n1), reach(n2)) {
        (Some(a), Some(b)) => {
            if a <= b {
                n1
            } else {
                n2
            }
        }
        (Some(_), None) => n1,
        (None, Some(_)) => n2,
        (None, None) => return false,
    };
    // How far the gable runs: until its height is at or under the roof at
    // every sampled distance from the midline.
    let step = (half / 24.0).max(2.0);
    let mut depth = 0.0f64;
    let mut s = -half;
    let mut met = false;
    while s <= half + 1e-9 {
        let g = base + (half - s.abs()) * k;
        let mut t = 0.0;
        while t <= extent {
            let p = c.add(u.scale(s)).add(n.scale(t));
            if surface_at(work, p).is_some_and(|h| h >= g - 1e-6) {
                depth = depth.max(t);
                met = true;
                break;
            }
            t += 1.0;
        }
        s += step;
    }
    if !met {
        return false;
    }
    depth += 3.0;
    let at = |s: f64, t: f64| c.add(u.scale(s)).add(n.scale(t));
    let halves = [(-half, 0.0, -1.0), (0.0, half, 1.0)];
    let mut wings: Vec<(Vec<Point>, Affine)> = Vec::new();
    for (s0, s1, sign) in halves {
        let quad = geom::ccw(&[at(s0, -ov), at(s1, -ov), at(s1, depth), at(s0, depth)]);
        // g = base + (half - sign * s) * k with s = (p - c) . u
        let gx = -sign * k * u.x;
        let gy = -sign * k * u.y;
        let height = Affine {
            c: base + half * k - sign * k * (-(c.x * u.x + c.y * u.y)),
            gx,
            gy,
        };
        wings.push((quad, height));
    }
    // Cut the roof planes where a wing is higher.
    let mut next: Vec<(RoofPlane, PlaneOrigin)> = Vec::new();
    for (plane, origin) in work.drain(..) {
        let Some(h) = Affine::of(&plane) else {
            next.push((plane, origin));
            continue;
        };
        let poly = geom::ccw(&plane.plan_polygon());
        let tiles: Vec<Vec<Point>> = if geom::is_convex(&poly) {
            vec![poly.clone()]
        } else {
            geom::ear_triangles(&poly)
                .into_iter()
                .map(|t| t.to_vec())
                .collect()
        };
        let mut cuts: Vec<Vec<Point>> = Vec::new();
        for (quad, g) in &wings {
            for tile in &tiles {
                let inside = geom::clip_convex(tile, quad);
                if inside.len() < 3 {
                    continue;
                }
                let above = keep_nonnegative(&inside, &|p| g.at(p) - h.at(p));
                if above.len() >= 3 && polygon_area(&above).abs() > MIN_PIECE {
                    cuts.push(above);
                }
            }
        }
        if cuts.is_empty() {
            next.push((plane, origin));
            continue;
        }
        let mut pieces = tiles;
        for cut in &cuts {
            let cut = geom::ccw(cut);
            pieces = pieces
                .iter()
                .flat_map(|piece| {
                    // A piece the cut does not touch stays whole.
                    if geom::clip_convex(piece, &cut).len() < 3 {
                        vec![piece.clone()]
                    } else {
                        minus_convex(piece, &cut)
                    }
                })
                .collect();
        }
        for piece in merge_convex(pieces) {
            if let Some(pl) = plane_over(&piece, &h, plane.pitch_in_12, plane.source_edge) {
                next.push((pl, origin));
            }
        }
    }
    // The gable planes where they stay above the roof.
    let old = next.clone();
    for (w, (quad, g)) in wings.iter().enumerate() {
        let mut pieces = vec![quad.clone()];
        for (plane, _) in &old {
            let Some(h) = Affine::of(plane) else { continue };
            let poly = geom::ccw(&plane.plan_polygon());
            let tiles: Vec<Vec<Point>> = if geom::is_convex(&poly) {
                vec![poly]
            } else {
                geom::ear_triangles(&poly)
                    .into_iter()
                    .map(|t| t.to_vec())
                    .collect()
            };
            for tile in tiles {
                let dom = keep_nonnegative(&tile, &|p| h.at(p) - g.at(p));
                if dom.len() < 3 || polygon_area(&dom).abs() <= MIN_PIECE {
                    continue;
                }
                let dom = geom::ccw(&dom);
                pieces = pieces
                    .iter()
                    .flat_map(|piece| {
                        if geom::clip_convex(piece, &dom).len() < 3 {
                            vec![piece.clone()]
                        } else {
                            minus_convex(piece, &dom)
                        }
                    })
                    .collect();
            }
        }
        for piece in merge_convex(pieces) {
            if let Some(pl) = plane_over(&piece, g, line.pitch, usize::MAX - w) {
                next.push((pl, PlaneOrigin::Wing(index)));
            }
        }
    }
    *work = next;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hole::tests::gable_roof_planes;

    fn rect() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ]
    }

    #[test]
    fn gable_line_turns_a_hip_into_three_planes() {
        let edges = vec![EdgeRoof::default(); 4];
        let hip = build_roof(&rect(), &edges, 100.0);
        assert_eq!(hip.planes.len(), 4);
        let out = apply_gable_line(&hip, 1).unwrap();
        assert_eq!(out.planes.len(), 3);
        assert!(!out.approximate);
        assert!(out.planes.iter().all(|p| p.source_edge != 1));
        let mut ids: Vec<usize> = out.planes.iter().map(|p| p.source_edge).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![0, 2, 3]);
        // The east end is a vertical gable on the eave line (x = 480 + 16):
        // the long planes now reach it, so the roof spans the full width.
        let (lo, hi) = out.bounds().unwrap();
        assert!((hi[0] - 496.0).abs() < 1e-6 && (lo[0] + 16.0).abs() < 1e-6);
        let on_gable = out
            .planes
            .iter()
            .filter(|p| {
                p.polygon3d
                    .iter()
                    .filter(|v| (v[0] - 496.0).abs() < 1e-6)
                    .count()
                    >= 2
            })
            .count();
        assert_eq!(on_gable, 2);
        // Planes still tile the same eave polygon.
        let before: f64 = hip.planes.iter().map(RoofPlane::projected_area).sum();
        let after: f64 = out.planes.iter().map(RoofPlane::projected_area).sum();
        assert!((before - after).abs() < 1e-3 * before);
        // The ridge is longer than the hip roof's, and its height is unchanged.
        assert!((hi[1] - hip.bounds().unwrap().1[1]).abs() < 1e-6);
    }

    #[test]
    fn second_gable_line_gives_the_two_plane_gable_roof() {
        let hip = build_roof(&rect(), &[EdgeRoof::default(); 4], 100.0);
        let one = apply_gable_line(&hip, 1).unwrap();
        let two = apply_gable_line(&one, 3).unwrap();
        assert_eq!(two.planes.len(), 2);
    }

    #[test]
    fn gable_line_rejects_missing_edges_and_unrecoverable_outlines() {
        let hip = build_roof(&rect(), &[EdgeRoof::default(); 4], 100.0);
        assert!(apply_gable_line(&hip, 9).is_none());
        // Two gable ends leave two separate chains of one plane each.
        let mut edges = vec![EdgeRoof::default(); 4];
        edges[1].kind = EdgeKind::Gable;
        edges[3].kind = EdgeKind::Gable;
        let gable = build_roof(&rect(), &edges, 100.0);
        assert!(apply_gable_line(&gable, 0).is_none());
    }

    #[test]
    fn full_half_and_boxed_returns() {
        let planes = gable_roof_planes();
        let south = planes.iter().find(|p| p.source_edge == 0).unwrap();
        for kind in [ReturnKind::Full, ReturnKind::Half, ReturnKind::Boxed] {
            let spec = ReturnSpec { kind, length: 36.0 };
            let r = roof_return(south, 0, spec).unwrap();
            assert_eq!(r.corner, [480.0, 108.0, 0.0]);
            let pl = &r.plane;
            assert_eq!(
                pl.polygon3d.len(),
                if kind == ReturnKind::Half { 3 } else { 4 }
            );
            assert!(pl.normal()[1] > 0.0, "{kind:?}");
            // First edge: the eave line extended past the corner.
            let (e0, e1) = (pl.polygon3d[0], pl.polygon3d[1]);
            assert!((e0[1] - 108.0).abs() < 1e-9 && (e1[1] - 108.0).abs() < 1e-9);
            assert!((e0[2]).abs() < 1e-9 && (e1[2]).abs() < 1e-9);
            let xs = [e0[0], e1[0]];
            assert!(xs.iter().any(|&x| (x - 480.0).abs() < 1e-9));
            assert!(xs.iter().any(|&x| (x - 516.0).abs() < 1e-9));
            match kind {
                ReturnKind::Boxed => {
                    assert_eq!(pl.pitch_in_12, 0.0);
                    assert!(pl.polygon3d.iter().all(|v| (v[1] - 108.0).abs() < 1e-9));
                    assert!((pl.projected_area() - 36.0 * 36.0).abs() < 1e-6);
                }
                ReturnKind::Full => {
                    assert!((pl.pitch_in_12 - 8.0).abs() < 1e-9);
                    assert!((pl.projected_area() - 36.0 * 36.0).abs() < 1e-6);
                    // Runs 36" up the slope: 24" of rise.
                    let top = pl.polygon3d.iter().fold(f64::MIN, |m, v| m.max(v[1]));
                    assert!((top - 132.0).abs() < 1e-9);
                }
                ReturnKind::Half => {
                    assert!((pl.projected_area() - 36.0 * 36.0 * 0.5).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn return_at_the_start_corner_and_bad_input() {
        let planes = gable_roof_planes();
        let south = planes.iter().find(|p| p.source_edge == 0).unwrap();
        let spec = ReturnSpec {
            kind: ReturnKind::Full,
            length: 24.0,
        };
        let r = roof_return_at(south, 0, true, spec).unwrap();
        assert_eq!(r.corner, [0.0, 108.0, 0.0]);
        let xs: Vec<f64> = r.plane.polygon3d.iter().map(|v| v[0]).collect();
        assert!(xs.iter().any(|&x| (x + 24.0).abs() < 1e-9));
        assert!(r.plane.normal()[1] > 0.0);
        assert!(roof_return(
            south,
            0,
            ReturnSpec {
                length: 0.0,
                ..spec
            }
        )
        .is_none());
        assert!(roof_return(south, 7, spec).is_none());
    }

    // ----- Gable/Roof Line objects -----

    fn hip_roof() -> Roof {
        build_roof(&rect(), &[EdgeRoof::default(); 4], 100.0)
    }

    /// Roof height at `p`: the highest plane over it.
    fn surface(planes: &[RoofPlane], p: Point) -> Option<f64> {
        planes
            .iter()
            .filter(|pl| plan_core::geometry::point_in_polygon(p, &pl.plan_polygon()))
            .filter_map(|pl| pl.height_at(p))
            .fold(None, |m: Option<f64>, h| Some(m.map_or(h, |m| m.max(h))))
    }

    #[test]
    fn a_gable_line_over_a_hip_roof_adds_two_planes_and_valleys() {
        let roof = hip_roof();
        let line = GableLine::new(Point::new(200.0, 0.0), Point::new(260.0, 0.0), 8.0, 16.0);
        let out = apply_gable_lines(&roof.planes, roof.baseline_elevation, &[line]);
        assert!(out.skipped.is_empty());
        let wings: Vec<&RoofPlane> = out
            .planes
            .iter()
            .zip(&out.origin)
            .filter(|(_, o)| matches!(o, PlaneOrigin::Wing(0)))
            .map(|(p, _)| p)
            .collect();
        assert_eq!(wings.len(), 2, "one plane each side of the ridge");
        // Every plane faces up and has the eave first.
        for (pl, _) in out.planes.iter().zip(&out.origin) {
            assert!(pl.normal()[1] > 0.0);
            assert!((pl.polygon3d[0][1] - pl.polygon3d[1][1]).abs() < 1e-6 || pl.pitch_in_12 > 0.0);
        }
        // The gable is 60 + 2 * 16 = 92" wide at the eave: its planes reach
        // x = 184 and x = 276, and the rake overhang puts them 16" in front of
        // the line.
        let xs: Vec<f64> = wings
            .iter()
            .flat_map(|p| p.polygon3d.iter().map(|v| v[0]))
            .collect();
        let (lo, hi) = (
            xs.iter().cloned().fold(f64::MAX, f64::min),
            xs.iter().cloned().fold(f64::MIN, f64::max),
        );
        assert!((lo - 184.0).abs() < 1e-6, "left {lo}");
        assert!((hi - 276.0).abs() < 1e-6, "right {hi}");
        let front = wings
            .iter()
            .flat_map(|p| p.polygon3d.iter().map(|v| -v[2]))
            .fold(f64::MAX, f64::min);
        assert!((front + 16.0).abs() < 1e-6, "front {front}");
        // The ridge is level at eave + (46 * 8 / 12).
        let top = wings
            .iter()
            .flat_map(|p| p.polygon3d.iter().map(|v| v[1]))
            .fold(f64::MIN, f64::max);
        assert!((top - (100.0 + 46.0 * 8.0 / 12.0)).abs() < 1e-6, "top {top}");
    }

    #[test]
    fn the_gabled_roof_is_the_higher_of_the_gable_and_the_old_roof() {
        let roof = hip_roof();
        let line = GableLine::new(Point::new(200.0, 0.0), Point::new(260.0, 0.0), 8.0, 16.0);
        let out = apply_gable_lines(&roof.planes, roof.baseline_elevation, &[line]);
        let g = |p: Point| {
            let s = p.x - 230.0;
            100.0 + (46.0 - s.abs()) * 8.0 / 12.0
        };
        let mut checked = 0;
        let mut y = -10.0;
        while y < 150.0 {
            let mut x = 150.0;
            while x < 310.0 {
                let p = Point::new(x + 0.37, y + 0.41);
                let in_wing = (p.x - 230.0).abs() <= 46.0 && p.y >= -16.0 && p.y <= 100.0;
                let old = surface(&roof.planes, p);
                let want = match (old, in_wing) {
                    (Some(h), true) => Some(h.max(g(p))),
                    (None, true) => Some(g(p)),
                    (Some(h), false) => Some(h),
                    (None, false) => None,
                };
                // Planes tile the roof, so the highest and the only plane agree.
                let got = surface(&out.planes, p);
                match (want, got) {
                    (Some(w), Some(h)) => {
                        assert!((w - h).abs() < 1e-4, "at {p:?}: want {w}, got {h}");
                        checked += 1;
                    }
                    (None, None) => {}
                    (w, h) => panic!("at {p:?}: want {w:?}, got {h:?}"),
                }
                x += 7.0;
            }
            y += 7.0;
        }
        assert!(checked > 300);
        // The hip roof still has what is outside the gable.
        let kept = out
            .origin
            .iter()
            .filter(|o| matches!(o, PlaneOrigin::Main(_)))
            .count();
        assert!(kept >= 4);
        // No overlap: the plan area of the main pieces plus the gable equals
        // the old roof plus the gable's own footprint outside it.
        let area = |ps: &[RoofPlane]| ps.iter().map(RoofPlane::projected_area).sum::<f64>();
        let old = area(&roof.planes);
        let new = area(&out.planes);
        // The 92" wide strip projects 16" past the old eave? No: the old eave is
        // already 16" out, so the gable adds nothing outside the old outline
        // except where the rake overhang sticks out past the eave line.
        assert!(new >= old - 1.0, "{new} vs {old}");
    }

    #[test]
    fn lines_that_miss_the_roof_are_skipped_and_two_lines_stack() {
        let roof = hip_roof();
        let far = GableLine::new(Point::new(2000.0, 2000.0), Point::new(2060.0, 2000.0), 8.0, 16.0);
        let out = apply_gable_lines(&roof.planes, roof.baseline_elevation, &[far]);
        assert_eq!(out.skipped, vec![0]);
        assert_eq!(out.planes.len(), roof.planes.len());
        let a = GableLine::new(Point::new(100.0, 0.0), Point::new(160.0, 0.0), 8.0, 16.0);
        let b = GableLine::new(Point::new(300.0, 0.0), Point::new(360.0, 0.0), 6.0, 12.0);
        let two = apply_gable_lines(&roof.planes, roof.baseline_elevation, &[a, b]);
        assert!(two.skipped.is_empty());
        assert!(two.origin.contains(&PlaneOrigin::Wing(0)));
        assert!(two.origin.contains(&PlaneOrigin::Wing(1)));
        let p6 = two
            .planes
            .iter()
            .zip(&two.origin)
            .filter(|(_, o)| **o == PlaneOrigin::Wing(1))
            .all(|(p, _)| (p.pitch_in_12 - 6.0).abs() < 1e-9);
        assert!(p6);
    }

    #[test]
    fn the_line_may_stand_outside_the_wall_like_a_porch_gable() {
        let roof = hip_roof();
        // 60" in front of the south eave line (y = -16), 120" wide.
        let line = GableLine::new(Point::new(180.0, -60.0), Point::new(300.0, -60.0), 6.0, 12.0);
        let out = apply_gable_lines(&roof.planes, roof.baseline_elevation, &[line]);
        assert!(out.skipped.is_empty());
        let wing_area: f64 = out
            .planes
            .iter()
            .zip(&out.origin)
            .filter(|(_, o)| matches!(o, PlaneOrigin::Wing(0)))
            .map(|(p, _)| p.projected_area())
            .sum();
        // Out in front of the old roof the porch roof stands free: at least the
        // 60" gap and the rake overhang times its width.
        assert!(wing_area > 120.0 * 60.0, "{wing_area}");
    }

    #[test]
    fn a_gable_line_must_be_parallel_to_a_wall_and_within_ten_feet() {
        let walls = [
            WallFace {
                start: Point::new(0.0, 0.0),
                end: Point::new(480.0, 0.0),
                thickness: 6.0,
            },
            WallFace {
                start: Point::new(480.0, 0.0),
                end: Point::new(480.0, 288.0),
                thickness: 6.0,
            },
        ];
        let ok = GableLine::new(Point::new(100.0, -40.0), Point::new(160.0, -40.0), 8.0, 16.0);
        assert_eq!(check_gable_line(&ok, &walls), Ok(0));
        let tilted = GableLine::new(Point::new(100.0, -40.0), Point::new(160.0, -43.0), 8.0, 16.0);
        assert_eq!(
            check_gable_line(&tilted, &walls),
            Err(GableLineProblem::NotParallel)
        );
        let far = GableLine::new(Point::new(100.0, -200.0), Point::new(160.0, -200.0), 8.0, 16.0);
        assert_eq!(check_gable_line(&far, &walls), Err(GableLineProblem::TooFar));
        let short = GableLine::new(Point::new(0.0, -40.0), Point::new(5.0, -40.0), 8.0, 16.0);
        assert_eq!(check_gable_line(&short, &walls), Err(GableLineProblem::TooShort));
        // On the Main Layer (an alcove cover): distance zero is fine.
        let flush = GableLine::new(Point::new(100.0, -3.0), Point::new(160.0, -3.0), 8.0, 16.0);
        assert_eq!(check_gable_line(&flush, &walls), Ok(0));
    }

    #[test]
    fn gables_over_openings_extend_twelve_inches_and_merge_within_thirty() {
        let span = |offset: f64, width: f64| OpeningSpan {
            wall_start: Point::new(0.0, 0.0),
            wall_end: Point::new(480.0, 0.0),
            offset,
            width,
            outward: Point::new(0.0, -1.0),
            face: 3.0,
        };
        // One 36" door at 120: 12" each side.
        let one = gable_lines_over_openings(&[span(120.0, 36.0)], 8.0, 16.0);
        assert_eq!(one.len(), 1);
        assert!((one[0].length() - 60.0).abs() < 1e-9);
        assert!((one[0].a.x - 90.0).abs() < 1e-9 && (one[0].b.x - 150.0).abs() < 1e-9);
        assert!((one[0].a.y + 3.0).abs() < 1e-9, "on the outside face");
        // Two windows with a 24" gap share one gable; a 40" gap does not.
        let near = gable_lines_over_openings(&[span(60.0, 36.0), span(120.0, 36.0)], 8.0, 16.0);
        assert_eq!(near.len(), 1);
        assert!((near[0].length() - 120.0).abs() < 1e-9, "{}", near[0].length());
        let far = gable_lines_over_openings(&[span(60.0, 36.0), span(136.0, 36.0)], 8.0, 16.0);
        assert_eq!(far.len(), 2);
        // A different wall is a different gable.
        let mut other = span(60.0, 36.0);
        other.wall_start = Point::new(0.0, 288.0);
        other.wall_end = Point::new(480.0, 288.0);
        assert_eq!(
            gable_lines_over_openings(&[span(60.0, 36.0), other], 8.0, 16.0).len(),
            2
        );
    }
}
