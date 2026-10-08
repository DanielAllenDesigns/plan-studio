//! Wall outline geometry with Chief-style joins.
//!
//! * Two non-collinear walls sharing an endpoint are mitered: both outlines
//!   end at the same two corner points, so there is no overlap or gap.
//! * A wall ending on the interior of another wall (T-junction) is extended or
//!   trimmed so its end lies on the near face of the through wall.
//! * Collinear continuations, free ends and junctions of three or more walls
//!   keep plain square ends.
//! * A miter longer than [`MITER_LIMIT`] times the wall thickness (very sharp
//!   angles) falls back to square ends.
//! * [`wall_layer_outlines`] applies the same rules to every layer boundary of
//!   layered wall types (W-46..W-49), aligning neighbours on their main layer
//!   (W-26, W-102): the boundary at distance `r` outside a wall's main layer
//!   meets the neighbour's boundary nearest to `r` outside its own main layer.

use crate::defaults::WallTypeDef;
use crate::geometry::{BoxGrid, Point};
use crate::model::{Id, Wall, WallEnd};

/// Maximum miter length as a multiple of the thicker wall.
pub const MITER_LIMIT: f64 = 4.0;
/// `|sin|` of the angle between two walls below which they count as parallel.
const MIN_SIN: f64 = 0.02;

#[derive(Debug, Clone)]
pub struct WallOutline {
    pub wall_id: Id,
    /// Four points for a straight wall: start-left, end-left, end-right,
    /// start-right, where left is the side of the +normal (same order as
    /// [`Wall::footprint`]). A curved wall's is the faceted band of its arc
    /// (the left face forward, then the right face back, as
    /// [`Wall::plan_polygon`] lays it out) with its ends mitered.
    pub polygon: Vec<Point>,
}

/// The four unjoined face corners of a wall: `(start_left, end_left, end_right, start_right)`.
pub fn wall_faces(wall: &Wall) -> (Point, Point, Point, Point) {
    let f = wall.footprint();
    (f[0], f[1], f[2], f[3])
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Start,
    End,
}

fn end_point(w: &Wall, e: End) -> Point {
    match e {
        End::Start => w.start,
        End::End => w.end,
    }
}

/// Unit direction pointing from the given end back along the wall. For a
/// curved wall it is the arc's tangent at that end, so joins see the
/// direction the wall really leaves in, not the chord (W-67, W-68).
fn away_dir(w: &Wall, e: End) -> Point {
    if let Some(c) = w.curve.filter(|c| !c.is_straight()) {
        return match e {
            End::Start => c.tangent_at_start(w.start, w.end),
            End::End => -c.tangent_at_end(w.start, w.end),
        };
    }
    match e {
        End::Start => w.direction(),
        End::End => -w.direction(),
    }
}

/// Sign of the wall's left (+normal) side relative to `away_dir`'s perpendicular.
fn left_sign(e: End) -> f64 {
    match e {
        End::Start => 1.0,
        End::End => -1.0,
    }
}

/// Are the two wall lists the same in every field? [`Wall`] has no
/// `PartialEq`; callers that cache what they derived from walls (rooms,
/// outlines) compare with this. The destructuring is exhaustive on purpose: a
/// field added to `Wall` stops this from compiling until it is compared.
pub fn walls_equal(a: &[Wall], b: &[Wall]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| wall_eq(x, y))
}

fn wall_eq(a: &Wall, b: &Wall) -> bool {
    let Wall {
        id,
        start,
        end,
        thickness,
        height,
        kind,
        layer,
        flags,
        wall_type,
        resize_about,
        curve,
        roof,
        exterior_side,
        extras,
        class,
        foundation_height,
        is_deck_edge,
        bottom_offset,
    } = a;
    *id == b.id
        && *start == b.start
        && *end == b.end
        && *thickness == b.thickness
        && *height == b.height
        && *kind == b.kind
        && *layer == b.layer
        && *flags == b.flags
        && *wall_type == b.wall_type
        && *resize_about == b.resize_about
        && *curve == b.curve
        && *roof == b.roof
        && *exterior_side == b.exterior_side
        && *extras == b.extras
        && *class == b.class
        && *foundation_height == b.foundation_height
        && *is_deck_edge == b.is_deck_edge
        && *bottom_offset == b.bottom_offset
}

/// Walls found by position, for the join queries that look for neighbours of
/// one wall end: a grid over the walls' boxes (a scan for short lists).
/// Candidates come back in wall order, so a loop over them behaves like a
/// loop over all walls.
struct Near {
    grid: Option<BoxGrid>,
    len: usize,
}

/// Below this many walls a scan beats building a grid.
const GRID_MIN_WALLS: usize = 24;

impl Near {
    fn new(walls: &[Wall], tol: f64) -> Near {
        let grid = (walls.len() >= GRID_MIN_WALLS).then(|| {
            let boxes: Vec<(Point, Point)> = walls
                .iter()
                .map(|w| {
                    let (mut lo, mut hi) = (
                        Point::new(w.start.x.min(w.end.x), w.start.y.min(w.end.y)),
                        Point::new(w.start.x.max(w.end.x), w.start.y.max(w.end.y)),
                    );
                    // An arc bulges out of its chord's box: take the box of
                    // points along it, padded by what a facet of them cuts.
                    if let Some((_, r)) = w.arc_center_radius() {
                        let sweep = w.curve.map_or(0.0, |k| k.sweep(w.start, w.end).abs());
                        let pad = r * (1.0 - (sweep / 48.0).cos());
                        for q in w.sample_points(24) {
                            lo = Point::new(lo.x.min(q.x - pad), lo.y.min(q.y - pad));
                            hi = Point::new(hi.x.max(q.x + pad), hi.y.max(q.y + pad));
                        }
                    }
                    (
                        Point::new(lo.x - tol, lo.y - tol),
                        Point::new(hi.x + tol, hi.y + tol),
                    )
                })
                .collect();
            BoxGrid::new(&boxes)
        });
        Near {
            grid,
            len: walls.len(),
        }
    }

    /// No index: every wall is a candidate.
    fn all(walls: &[Wall]) -> Near {
        Near {
            grid: None,
            len: walls.len(),
        }
    }

    /// The walls whose box (grown by the `tol` given to [`Near::new`]) may
    /// contain `p`.
    fn around(&self, p: Point, out: &mut Vec<usize>) {
        match &self.grid {
            Some(g) => g.query(p, p, out),
            None => {
                out.clear();
                out.extend(0..self.len);
            }
        }
    }
}

/// Outlines for every wall, in input order.
pub fn wall_outlines(walls: &[Wall], tol: f64) -> Vec<WallOutline> {
    outlines_with(walls, tol, &Near::new(walls, tol))
}

fn outlines_with(walls: &[Wall], tol: f64, near: &Near) -> Vec<WallOutline> {
    walls
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if w.is_curved() {
                return WallOutline {
                    wall_id: w.id,
                    polygon: curved_polygon_near(walls, near, i, tol, None),
                };
            }
            let (sl, el, er, sr) = wall_faces(w);
            let (sl, sr) = end_faces(walls, near, i, End::Start, tol).unwrap_or((sl, sr));
            let (el, er) = end_faces(walls, near, i, End::End, tol).unwrap_or((el, er));
            WallOutline {
                wall_id: w.id,
                polygon: vec![sl, el, er, sr],
            }
        })
        .collect()
}

/// The plan polygon of the curved wall `id` with its ends mitered against the
/// walls that join it (W-67): [`Wall::plan_polygon`] with the first and last
/// left and right points moved to where the neighbour's faces cross the arc's
/// end tangent. Limits: the miter treats the arc as its tangent line at the
/// joined end, so a wide arc meeting a steep corner shows a small kink at the
/// end facet; the miter length is capped at [`MITER_LIMIT`] and past it the
/// end stays square; a curved wall that meets another wall's side (a tee)
/// gets the tee cut. `None` when the wall is unknown or not curved.
pub fn curved_wall_polygon(walls: &[Wall], id: Id, tol: f64) -> Option<Vec<Point>> {
    curved_wall_polygon_n(walls, id, tol, None)
}

/// [`curved_wall_polygon`] with `facets` facets along the arc (`None`: the
/// facet angle's count). The polygon is the left face forward, then the right
/// face back, so it has `2 * (facets + 1)` points.
pub fn curved_wall_polygon_n(
    walls: &[Wall],
    id: Id,
    tol: f64,
    facets: Option<usize>,
) -> Option<Vec<Point>> {
    let i = walls.iter().position(|w| w.id == id)?;
    let w = &walls[i];
    if !w.is_curved() {
        return None;
    }
    Some(curved_polygon_near(
        walls,
        &Near::all(walls),
        i,
        tol,
        facets,
    ))
}

/// The band of curved wall `walls[i]` with its ends mitered.
fn curved_polygon_near(
    walls: &[Wall],
    near: &Near,
    i: usize,
    tol: f64,
    facets: Option<usize>,
) -> Vec<Point> {
    let w = &walls[i];
    let mut poly = match facets {
        Some(n) => w.plan_polygon_n(n),
        None => w.plan_polygon(),
    };
    let n = poly.len();
    let h = n / 2;
    if let Some((l, r)) = end_faces(walls, near, i, End::Start, tol) {
        poly[0] = l;
        poly[n - 1] = r;
    }
    if let Some((l, r)) = end_faces(walls, near, i, End::End, tol) {
        poly[h - 1] = l;
        poly[h] = r;
    }
    poly
}

/// The mitered `(left, right)` face points at the start and the end of curved
/// wall `id`, from the walls that join it (straight or curved, corners and
/// tees), or `None` for an end that stays square. The two points of an end
/// are the cut line of the miter; the neighbour's outline ends on the same
/// two points. `None` when the wall is unknown or not curved.
#[allow(clippy::type_complexity)]
pub fn curved_end_miters(walls: &[Wall], id: Id, tol: f64) -> Option<[Option<(Point, Point)>; 2]> {
    let i = walls.iter().position(|w| w.id == id)?;
    if !walls[i].is_curved() {
        return None;
    }
    let near = Near::all(walls);
    Some([
        end_faces(walls, &near, i, End::Start, tol),
        end_faces(walls, &near, i, End::End, tol),
    ])
}

/// Joined `(left, right)` face points at one end, or `None` for a square end.
fn end_faces(walls: &[Wall], near: &Near, i: usize, e: End, tol: f64) -> Option<(Point, Point)> {
    let w = &walls[i];
    if w.length() <= tol {
        return None;
    }
    let p = end_point(w, e);
    let mut touching = Vec::new();
    let mut candidates = Vec::new();
    near.around(p, &mut candidates);
    for &j in &candidates {
        let o = &walls[j];
        if j == i || o.length() <= tol {
            continue;
        }
        for oe in [End::Start, End::End] {
            if end_point(o, oe).dist(p) <= tol {
                touching.push((j, oe));
            }
        }
    }
    match touching.len() {
        1 => miter_faces(walls, i, e, touching[0].0, touching[0].1),
        0 => t_faces(walls, near, i, e, p, tol),
        _ => None,
    }
}

/// Face intersections of two walls' offset lines meeting at `p`.
/// Returns the points on wall A's `+1` and `-1` sides (relative to the
/// perpendicular of `da`); wall B's `-1` / `+1` sides meet there respectively.
fn miter_points(p: Point, da: Point, ta: f64, db: Point, tb: f64) -> Option<(Point, Point)> {
    let cross = da.cross(db);
    if cross.abs() < MIN_SIN {
        return None;
    }
    let mut out = [Point::ZERO; 2];
    for (k, s) in [1.0, -1.0].into_iter().enumerate() {
        let pa = p + da.perp() * (s * ta * 0.5);
        let pb = p + db.perp() * (-s * tb * 0.5);
        let u = (pb - pa).cross(db) / cross;
        let m = pa + da * u;
        if m.dist(p) > MITER_LIMIT * ta.max(tb) {
            return None;
        }
        out[k] = m;
    }
    Some((out[0], out[1]))
}

fn miter_faces(walls: &[Wall], i: usize, e: End, j: usize, oe: End) -> Option<(Point, Point)> {
    // Compute from the lower-indexed wall so both walls get identical points.
    let is_lo = i < j;
    let (lo, lo_e, hi, hi_e) = if is_lo { (i, e, j, oe) } else { (j, oe, i, e) };
    let (a, b) = (&walls[lo], &walls[hi]);
    let p = end_point(a, lo_e);
    let (plus, minus) = miter_points(
        p,
        away_dir(a, lo_e),
        a.thickness,
        away_dir(b, hi_e),
        b.thickness,
    )?;
    let at = |s: f64| if is_lo == (s > 0.0) { plus } else { minus };
    let ls = left_sign(e);
    Some((at(ls), at(-ls)))
}

/// The point of wall `t` closest to `p` and the unit direction of the wall
/// there: the tangent of an arc, the direction of a straight wall.
fn host_point(t: &Wall, p: Point) -> (Point, Point) {
    t.closest_point(p)
}

/// The nearest wall (other than `i`) whose interior, not its ends, contains
/// `p`, with the closest point on it and its direction there: the host of a
/// T-junction at `p`. A curved host is met along its tangent.
fn tee_host(
    walls: &[Wall],
    near: &Near,
    i: usize,
    p: Point,
    tol: f64,
) -> Option<(usize, Point, Point)> {
    let mut best: Option<(f64, usize, Point, Point)> = None;
    let mut candidates = Vec::new();
    near.around(p, &mut candidates);
    for &j in &candidates {
        let t = &walls[j];
        if j == i || t.length() <= tol {
            continue;
        }
        let (q, dir) = host_point(t, p);
        let d = q.dist(p);
        if d <= tol
            && q.dist(t.start) > tol
            && q.dist(t.end) > tol
            && best.is_none_or(|(bd, ..)| d < bd)
        {
            best = Some((d, j, q, dir));
        }
    }
    best.map(|(_, j, q, dir)| (j, q, dir))
}

fn t_faces(
    walls: &[Wall],
    near: &Near,
    i: usize,
    e: End,
    p: Point,
    tol: f64,
) -> Option<(Point, Point)> {
    let w = &walls[i];
    let dw = away_dir(w, e);
    let (tj, q, dt) = tee_host(walls, near, i, p, tol)?;
    let t = &walls[tj];

    let cross = dw.cross(dt);
    if cross.abs() < MIN_SIN {
        return None;
    }
    let side = if dt.perp().dot(dw) >= 0.0 { 1.0 } else { -1.0 };
    let face_pt = q + dt.perp() * (side * t.thickness * 0.5);

    let mut out = [Point::ZERO; 2];
    for (k, s) in [1.0, -1.0].into_iter().enumerate() {
        let wp = p + dw.perp() * (s * w.thickness * 0.5);
        let u = (face_pt - wp).cross(dt) / cross;
        let m = wp + dw * u;
        if m.dist(p) > MITER_LIMIT * w.thickness.max(t.thickness) {
            return None;
        }
        out[k] = m;
    }
    let ls = left_sign(e);
    let at = |s: f64| if s > 0.0 { out[0] } else { out[1] };
    Some((at(ls), at(-ls)))
}

// ----- junction queries -----

/// How another wall meets one end of a wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    /// Two ends meet at an angle.
    Corner,
    /// This wall's end butts into the interior of another wall.
    Tee,
    /// Two ends meet in a straight continuation.
    Through,
}

fn to_end(e: WallEnd) -> End {
    match e {
        WallEnd::Start => End::Start,
        WallEnd::End => End::End,
    }
}

/// Walls (by index in `walls`) joined to end `which` of `walls[i]`, with the
/// kind of join, using the same matching as [`wall_outlines`]: ends within
/// `tol` of each other are a [`ConnectionKind::Corner`] (or `Through` when
/// they continue in a straight line); an end lying on another wall's interior
/// is a [`ConnectionKind::Tee`].
pub fn wall_end_joins(
    walls: &[Wall],
    i: usize,
    which: WallEnd,
    tol: f64,
) -> Vec<(usize, ConnectionKind)> {
    let w = &walls[i];
    if w.length() <= tol {
        return Vec::new();
    }
    let e = to_end(which);
    let p = end_point(w, e);
    let mut out = Vec::new();
    for (j, o) in walls.iter().enumerate() {
        if j == i || o.length() <= tol {
            continue;
        }
        for oe in [End::Start, End::End] {
            if end_point(o, oe).dist(p) <= tol {
                let (da, db) = (away_dir(w, e), away_dir(o, oe));
                let straight = da.cross(db).abs() < MIN_SIN && da.dot(db) < 0.0;
                let kind = if straight {
                    ConnectionKind::Through
                } else {
                    ConnectionKind::Corner
                };
                out.push((j, kind));
            }
        }
    }
    if out.is_empty() {
        if let Some((j, ..)) = tee_host(walls, &Near::all(walls), i, p, tol) {
            out.push((j, ConnectionKind::Tee));
        }
    }
    out
}

// ----- layered outlines -----

/// Smallest layer thickness kept when a wall's thickness differs from its type.
const MIN_LAYER: f64 = 0.125;

/// One layer of a wall laid across its thickness. `outer` and `inner` are
/// lateral offsets from the centerline along the wall's +normal; `outer` is
/// on the exterior side ([`Wall::exterior_side`]).
#[derive(Debug, Clone, PartialEq)]
pub struct LayerBand {
    pub name: String,
    pub is_main: bool,
    pub outer: f64,
    pub inner: f64,
}

/// The layer bands of `wall`, exterior to interior. With `ty == None` (or a
/// type with no layers) the wall is one main layer. When the wall's thickness
/// differs from the type's total, the main layer absorbs the difference
/// (W-29); if that would make it too thin, or there is no main layer, all
/// layers scale proportionally.
pub fn wall_layer_bands(wall: &Wall, ty: Option<&WallTypeDef>) -> Vec<LayerBand> {
    let t = wall.thickness;
    let mut layers: Vec<(String, f64, bool)> = match ty {
        Some(ty) if !ty.layers.is_empty() => ty
            .layers
            .iter()
            .map(|l| (l.name.clone(), l.thickness, l.is_main))
            .collect(),
        _ => vec![("Wall".to_string(), t, true)],
    };
    let sum: f64 = layers.iter().map(|l| l.1).sum();
    let diff = t - sum;
    if diff.abs() > 1e-9 {
        let main = layers.iter().position(|l| l.2);
        match main {
            Some(mi) if layers[mi].1 + diff >= MIN_LAYER => layers[mi].1 += diff,
            _ if sum > 1e-9 => {
                for l in layers.iter_mut() {
                    l.1 *= t / sum;
                }
            }
            _ => {}
        }
    }
    let ext = wall.exterior_side.sign();
    let mut depth = 0.0;
    layers
        .into_iter()
        .map(|(name, th, is_main)| {
            let outer = ext * (t * 0.5 - depth);
            depth += th;
            let inner = ext * (t * 0.5 - depth);
            LayerBand {
                name,
                is_main,
                outer,
                inner,
            }
        })
        .collect()
}

/// The four corners of the wall's main layer, unjoined, in the order of
/// [`wall_faces`]: `(start_left, end_left, end_right, start_right)`. With no
/// main layer the whole wall counts as the main layer.
pub fn main_layer_lines(wall: &Wall, ty: &WallTypeDef) -> (Point, Point, Point, Point) {
    let bands = wall_layer_bands(wall, Some(ty));
    let (hi, lo) = match bands.iter().find(|b| b.is_main) {
        Some(b) => (b.outer.max(b.inner), b.outer.min(b.inner)),
        None => (wall.thickness * 0.5, -wall.thickness * 0.5),
    };
    let n = wall.normal();
    (
        wall.start + n * hi,
        wall.end + n * hi,
        wall.end + n * lo,
        wall.start + n * lo,
    )
}

/// The polygon of one layer of one wall.
#[derive(Debug, Clone)]
pub struct WallLayerOutline {
    pub wall_id: Id,
    /// Index into the wall's layers, exterior to interior.
    pub layer_index: usize,
    pub name: String,
    pub is_main: bool,
    /// Start-left, end-left, end-right, start-right (left = +normal side).
    pub polygon: Vec<Point>,
}

/// Boundary laterals of a stack (`bands.len() + 1` values, exterior first)
/// and the main layer's `(lo, hi)` span, both in the frame of `q`
/// (`+1` = the wall's own +normal, `-1` = flipped, as seen from the end
/// looking back along the wall).
fn boundaries(bands: &[LayerBand], q: f64) -> (Vec<f64>, f64, f64) {
    let mut b: Vec<f64> = bands.iter().map(|x| x.outer * q).collect();
    if let Some(last) = bands.last() {
        b.push(last.inner * q);
    }
    let (lo, hi) = match bands.iter().find(|x| x.is_main) {
        Some(m) => {
            let (a, c) = (m.outer * q, m.inner * q);
            (a.min(c), a.max(c))
        }
        None => (0.0, 0.0),
    };
    (b, lo, hi)
}

/// Unit left normal of the wall's travel at end `e`, square to that end: the
/// wall's normal when straight, the arc's normal at the end when curved.
fn end_normal(w: &Wall, e: End) -> Point {
    if w.is_curved() {
        let travel = match e {
            End::Start => w.end_tangent(WallEnd::Start),
            End::End => -w.end_tangent(WallEnd::End),
        };
        return travel.perp();
    }
    w.normal()
}

fn end_sign(e: End) -> f64 {
    match e {
        End::Start => 1.0,
        End::End => -1.0,
    }
}

/// Layer boundary points at one end of `walls[i]`, indexed like the boundary
/// list of its stack (exterior first). Square ends where no join applies.
fn layer_end_points(
    walls: &[Wall],
    near: &Near,
    stacks: &[Vec<LayerBand>],
    i: usize,
    e: End,
    tol: f64,
) -> Vec<Point> {
    let w = &walls[i];
    let p = end_point(w, e);
    let qa = end_sign(e);
    let (own, _, _) = boundaries(&stacks[i], 1.0);
    let normal = end_normal(w, e);
    let square = || -> Vec<Point> { own.iter().map(|l| p + normal * *l).collect() };
    if w.length() <= tol {
        return square();
    }
    let mut touching = Vec::new();
    let mut candidates = Vec::new();
    near.around(p, &mut candidates);
    for &j in &candidates {
        let o = &walls[j];
        if j == i || o.length() <= tol {
            continue;
        }
        for oe in [End::Start, End::End] {
            if end_point(o, oe).dist(p) <= tol {
                touching.push((j, oe));
            }
        }
    }
    let dw = away_dir(w, e);
    // Boundary laterals in the away frame (perp of `dw`).
    let (a_lat, a_lo, a_hi) = boundaries(&stacks[i], qa);
    match touching.len() {
        1 => {
            let (j, oe) = touching[0];
            let o = &walls[j];
            let db = away_dir(o, oe);
            let cross = dw.cross(db);
            if cross.abs() < MIN_SIN {
                return square();
            }
            let (b_lat, b_lo, b_hi) = boundaries(&stacks[j], end_sign(oe));
            let limit = MITER_LIMIT * w.thickness.max(o.thickness);
            let mut pts = Vec::with_capacity(a_lat.len());
            for &la in &a_lat {
                // Distance outside the main layer, on the side `la` lies.
                let (r, plus_side) = if la >= a_hi - 1e-9 {
                    (la - a_hi, true)
                } else {
                    (a_lo - la, false)
                };
                // The neighbour's matching side is the opposite one.
                let mut best: Option<(f64, f64)> = None;
                for &lb in &b_lat {
                    let rb = if plus_side {
                        if lb > b_lo + 1e-9 {
                            continue;
                        }
                        b_lo - lb
                    } else {
                        if lb < b_hi - 1e-9 {
                            continue;
                        }
                        lb - b_hi
                    };
                    let d = (rb - r).abs();
                    if best.is_none_or(|(bd, _)| d < bd - 1e-12) {
                        best = Some((d, lb));
                    }
                }
                let Some((_, lb)) = best else {
                    return square();
                };
                let pa = p + dw.perp() * la;
                let pb = p + db.perp() * lb;
                let u = (pb - pa).cross(db) / cross;
                let m = pa + dw * u;
                if m.dist(p) > limit {
                    return square();
                }
                pts.push(m);
            }
            // `pts` follow the stack order; boundaries were mapped in the
            // away frame, which only reorders by the sign of `qa`.
            pts
        }
        0 => {
            let Some((tj, q, dt)) = tee_host(walls, near, i, p, tol) else {
                return square();
            };
            let t = &walls[tj];
            let cross = dw.cross(dt);
            if cross.abs() < MIN_SIN {
                return square();
            }
            let side = if dt.perp().dot(dw) >= 0.0 { 1.0 } else { -1.0 };
            let face_pt = q + dt.perp() * (side * t.thickness * 0.5);
            let limit = MITER_LIMIT * w.thickness.max(t.thickness);
            let mut pts = Vec::with_capacity(a_lat.len());
            for &la in &a_lat {
                let wp = p + dw.perp() * la;
                let u = (face_pt - wp).cross(dt) / cross;
                let m = wp + dw * u;
                if m.dist(p) > limit {
                    return square();
                }
                pts.push(m);
            }
            pts
        }
        _ => square(),
    }
}

/// Polygons for every layer of every wall, in input order and exterior to
/// interior layer order. Each wall's layers come from its `wall_type` looked
/// up in `types` (a wall with no or an unknown type is one main layer of its
/// thickness); joins follow the rules of [`wall_outlines`] applied to each
/// layer boundary line, with neighbours aligned on their main layers.
pub fn wall_layer_outlines(
    walls: &[Wall],
    types: &[WallTypeDef],
    tol: f64,
) -> Vec<WallLayerOutline> {
    layer_outlines_with(walls, types, tol, &Near::new(walls, tol))
}

fn layer_outlines_with(
    walls: &[Wall],
    types: &[WallTypeDef],
    tol: f64,
    near: &Near,
) -> Vec<WallLayerOutline> {
    layer_outlines_facets(walls, types, tol, near, None, None)
}

/// [`layer_outlines_with`] with `facets` along curved walls (`None`: the facet
/// angle's count), for the wall at index `only` alone or, with `None`, all.
fn layer_outlines_facets(
    walls: &[Wall],
    types: &[WallTypeDef],
    tol: f64,
    near: &Near,
    facets: Option<usize>,
    only: Option<usize>,
) -> Vec<WallLayerOutline> {
    let stacks: Vec<Vec<LayerBand>> = walls
        .iter()
        .map(|w| {
            let ty = w
                .wall_type
                .as_deref()
                .and_then(|n| types.iter().find(|t| t.name == n));
            wall_layer_bands(w, ty)
        })
        .collect();
    let mut out = Vec::new();
    for (i, w) in walls.iter().enumerate() {
        if only.is_some_and(|o| o != i) {
            continue;
        }
        let starts = layer_end_points(walls, near, &stacks, i, End::Start, tol);
        let ends = layer_end_points(walls, near, &stacks, i, End::End, tol);
        let (b, _, _) = boundaries(&stacks[i], 1.0);
        // A curved wall's boundaries are arcs between the joined end points.
        let arcs: Vec<Vec<Point>> = if w.is_curved() {
            let n = facets.map_or_else(|| default_facets(w), |n| n.max(2));
            b.iter()
                .enumerate()
                .map(|(k, lateral)| {
                    let mut pts = w.offset_curve(*lateral, n);
                    pts[0] = starts[k];
                    pts[n] = ends[k];
                    pts
                })
                .collect()
        } else {
            Vec::new()
        };
        for (k, band) in stacks[i].iter().enumerate() {
            let (hi, lo) = if b[k] >= b[k + 1] {
                (k, k + 1)
            } else {
                (k + 1, k)
            };
            let polygon = if w.is_curved() {
                let back = arcs[lo].iter().rev();
                arcs[hi].iter().copied().chain(back.copied()).collect()
            } else {
                vec![starts[hi], ends[hi], ends[lo], starts[lo]]
            };
            out.push(WallLayerOutline {
                wall_id: w.id,
                layer_index: k,
                name: band.name.clone(),
                is_main: band.is_main,
                polygon,
            });
        }
    }
    out
}

/// The facet count a curved wall's polygons use unless told otherwise.
fn default_facets(w: &Wall) -> usize {
    w.curve.map_or(2, |c| c.facet_count(w.start, w.end)).max(2)
}

/// The layer outlines of curved wall `id` with `facets` facets along the arc
/// (`None`: the facet angle's count). Each polygon is the layer's left
/// boundary forward along the arc, then its right boundary back, with the
/// ends mitered against the joined walls like the straight outlines;
/// `2 * (facets + 1)` points. `None` when the wall is unknown or straight.
pub fn curved_layer_outlines(
    walls: &[Wall],
    types: &[WallTypeDef],
    id: Id,
    tol: f64,
    facets: Option<usize>,
) -> Option<Vec<WallLayerOutline>> {
    let i = walls.iter().position(|w| w.id == id)?;
    if !walls[i].is_curved() {
        return None;
    }
    let near = Near::all(walls);
    Some(layer_outlines_facets(
        walls,
        types,
        tol,
        &near,
        facets,
        Some(i),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WallKind;

    fn wall(id: u64, x0: f64, y0: f64, x1: f64, y1: f64, t: f64) -> Wall {
        Wall {
            id,
            ..Wall::new(
                Point::new(x0, y0),
                Point::new(x1, y1),
                t,
                109.125,
                WallKind::Interior,
            )
        }
    }

    fn has(poly: &[Point], x: f64, y: f64) -> bool {
        poly.iter().any(|p| p.dist(Point::new(x, y)) < 1e-9)
    }

    #[test]
    fn l_corner_is_mitered_with_shared_points() {
        let walls = vec![
            wall(1, 0.0, 0.0, 120.0, 0.0, 6.0),
            wall(2, 120.0, 0.0, 120.0, 120.0, 6.0),
        ];
        let o = wall_outlines(&walls, 0.01);
        assert_eq!(o.len(), 2);
        assert_eq!(o[0].polygon.len(), 4);
        assert_eq!(o[1].polygon.len(), 4);
        // Outer corner (123, -3) and inner corner (117, 3) shared by both.
        for poly in [&o[0].polygon, &o[1].polygon] {
            assert!(has(poly, 123.0, -3.0), "outer corner missing");
            assert!(has(poly, 117.0, 3.0), "inner corner missing");
        }
        // Far ends stay square.
        assert!(has(&o[0].polygon, 0.0, 3.0) && has(&o[0].polygon, 0.0, -3.0));
        assert!(has(&o[1].polygon, 117.0, 120.0) && has(&o[1].polygon, 123.0, 120.0));
    }

    #[test]
    fn miter_is_independent_of_wall_order_and_direction() {
        // Same L, second wall drawn toward the corner, listed first.
        let walls = vec![
            wall(2, 120.0, 120.0, 120.0, 0.0, 6.0),
            wall(1, 0.0, 0.0, 120.0, 0.0, 6.0),
        ];
        let o = wall_outlines(&walls, 0.01);
        for out in &o {
            assert!(has(&out.polygon, 123.0, -3.0) && has(&out.polygon, 117.0, 3.0));
        }
    }

    #[test]
    fn t_junction_ends_on_through_wall_face() {
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0, 6.0),
            wall(2, 120.0, 100.0, 120.0, 0.0, 4.5),
        ];
        let o = wall_outlines(&walls, 0.01);
        // Through wall unchanged.
        let f = walls[0].footprint();
        for (a, b) in o[0].polygon.iter().zip(f.iter()) {
            assert!(a.dist(*b) < 1e-9);
        }
        // Butting wall ends on the near (top) face y = +3.
        let poly = &o[1].polygon;
        assert!(has(poly, 120.0 - 2.25, 3.0) || has(poly, 120.0 + 2.25, 3.0));
        assert_eq!(poly.iter().filter(|p| (p.y - 3.0).abs() < 1e-9).count(), 2);
        assert!(poly.iter().all(|p| p.y >= 3.0 - 1e-9));
    }

    #[test]
    fn t_junction_extends_short_wall_to_face() {
        // Butting wall stops at the centerline of the through wall from below.
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0, 6.0),
            wall(2, 120.0, -100.0, 120.0, 0.0, 4.5),
        ];
        let o = wall_outlines(&walls, 0.01);
        let poly = &o[1].polygon;
        assert_eq!(poly.iter().filter(|p| (p.y + 3.0).abs() < 1e-9).count(), 2);
        assert!(poly.iter().all(|p| p.y <= -3.0 + 1e-9));
    }

    #[test]
    fn free_wall_equals_footprint() {
        let w = wall(1, 0.0, 0.0, 100.0, 50.0, 6.0);
        let o = wall_outlines(std::slice::from_ref(&w), 0.01);
        let f = w.footprint();
        assert_eq!(o[0].polygon.len(), 4);
        for (a, b) in o[0].polygon.iter().zip(f.iter()) {
            assert!(a.dist(*b) < 1e-9);
        }
        let (a, b, c, d) = wall_faces(&w);
        assert_eq!([a, b, c, d], f);
    }

    #[test]
    fn collinear_and_three_way_junctions_stay_square() {
        let walls = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0, 6.0),
            wall(2, 100.0, 0.0, 200.0, 0.0, 6.0),
        ];
        let o = wall_outlines(&walls, 0.01);
        for (w, out) in walls.iter().zip(&o) {
            for (a, b) in out.polygon.iter().zip(w.footprint().iter()) {
                assert!(a.dist(*b) < 1e-9);
            }
        }
        // Three walls meeting at one point: all square.
        let walls = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0, 6.0),
            wall(2, 100.0, 0.0, 100.0, 100.0, 6.0),
            wall(3, 100.0, 0.0, 200.0, 0.0, 6.0),
        ];
        let o = wall_outlines(&walls, 0.01);
        for (w, out) in walls.iter().zip(&o) {
            for (a, b) in out.polygon.iter().zip(w.footprint().iter()) {
                assert!(a.dist(*b) < 1e-9);
            }
        }
    }

    #[test]
    fn sharp_angle_falls_back_to_square() {
        // ~5 degree corner: miter would be far longer than 4x thickness.
        let a = 5f64.to_radians();
        let walls = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0, 6.0),
            wall(2, 100.0, 0.0, 100.0 - 100.0 * a.cos(), 100.0 * a.sin(), 6.0),
        ];
        let o = wall_outlines(&walls, 0.01);
        for (w, out) in walls.iter().zip(&o) {
            for (p, q) in out.polygon.iter().zip(w.footprint().iter()) {
                assert!(p.dist(*q) < 1e-9);
            }
        }
    }

    // ----- layered outlines -----

    fn stucco() -> WallTypeDef {
        crate::defaults::PlanDefaults::chief_x18_daniel()
            .wall_type("Stucco-6")
            .unwrap()
            .clone()
    }

    fn typed_wall(id: u64, x0: f64, y0: f64, x1: f64, y1: f64, ty: &WallTypeDef) -> Wall {
        Wall {
            wall_type: Some(ty.name.clone()),
            kind: WallKind::Exterior,
            ..wall(id, x0, y0, x1, y1, ty.thickness())
        }
    }

    #[test]
    fn layer_bands_follow_exterior_side() {
        let ty = stucco();
        let mut w = typed_wall(1, 0.0, 0.0, 100.0, 0.0, &ty);
        let b = wall_layer_bands(&w, Some(&ty));
        assert_eq!(b.len(), 4);
        assert!((b[0].outer - 7.625 / 2.0).abs() < 1e-9);
        assert!((b[3].inner + 7.625 / 2.0).abs() < 1e-9);
        assert!(b[2].is_main);
        w.exterior_side = crate::walls::Side::Right;
        let b = wall_layer_bands(&w, Some(&ty));
        assert!((b[0].outer + 7.625 / 2.0).abs() < 1e-9);
        // A thickness edit goes into the main layer (W-29).
        w.thickness += 1.0;
        let b = wall_layer_bands(&w, Some(&ty));
        let main = b.iter().find(|x| x.is_main).unwrap();
        assert!(((main.outer - main.inner).abs() - 6.5).abs() < 1e-9);
        // main_layer_lines: framing rectangle of the unjoined wall.
        let w = typed_wall(1, 0.0, 0.0, 100.0, 0.0, &ty);
        let (sl, el, er, sr) = main_layer_lines(&w, &ty);
        let top = 7.625 / 2.0 - 1.0625 - 0.5625;
        assert!((sl.y - top).abs() < 1e-9 && (el.y - top).abs() < 1e-9);
        assert!((sr.y - (top - 5.5)).abs() < 1e-9 && (er.y - sr.y).abs() < 1e-9);
        assert_eq!((sl.x, el.x), (0.0, 100.0));
    }

    #[test]
    fn layered_l_corner_shares_vertices_and_main_layers_meet() {
        let ty = stucco();
        let walls = vec![
            typed_wall(1, 0.0, 0.0, 120.0, 0.0, &ty),
            typed_wall(2, 120.0, 0.0, 120.0, 120.0, &ty),
        ];
        // Counter-clockwise box corner: the interior is on the left, so the
        // exterior layers are on the right of both walls.
        let mut walls = walls;
        walls[0].exterior_side = crate::walls::Side::Right;
        walls[1].exterior_side = crate::walls::Side::Right;
        let o = wall_layer_outlines(&walls, std::slice::from_ref(&ty), 0.01);
        for k in 0..4 {
            let pa = &o
                .iter()
                .find(|l| l.wall_id == 1 && l.layer_index == k)
                .unwrap()
                .polygon;
            let pb = &o
                .iter()
                .find(|l| l.wall_id == 2 && l.layer_index == k)
                .unwrap()
                .polygon;
            // Wall A's end corners (indices 1 and 2) are wall B's start corners (0 and 3).
            let a_end = [pa[1], pa[2]];
            let b_start = [pb[0], pb[3]];
            for p in a_end {
                assert!(
                    b_start.iter().any(|q| q.dist(p) < 1e-9),
                    "layer {k}: {p:?} not shared with {b_start:?}"
                );
            }
        }
        // Main-layer outer faces meet at one mitered point: A's right side
        // is the exterior (y = -3.8125 outside, framing outside at -(3.8125-1.625)).
        let fa = &o
            .iter()
            .find(|l| l.wall_id == 1 && l.is_main)
            .unwrap()
            .polygon;
        let fb = &o
            .iter()
            .find(|l| l.wall_id == 2 && l.is_main)
            .unwrap()
            .polygon;
        let main_out = 7.625 / 2.0 - 1.0625 - 0.5625;
        let corner = Point::new(120.0 + main_out, -main_out);
        assert!(fa.iter().any(|p| p.dist(corner) < 1e-9), "{fa:?}");
        assert!(fb.iter().any(|p| p.dist(corner) < 1e-9), "{fb:?}");
        // The outermost layers meet at the building's outside corner.
        let siding = &o
            .iter()
            .find(|l| l.wall_id == 1 && l.layer_index == 0)
            .unwrap()
            .polygon;
        let out_corner = Point::new(120.0 + 7.625 / 2.0, -7.625 / 2.0);
        assert!(siding.iter().any(|p| p.dist(out_corner) < 1e-9));
    }

    #[test]
    fn single_layer_outlines_equal_wall_outlines() {
        let walls = vec![
            wall(1, 0.0, 0.0, 240.0, 0.0, 6.0),
            wall(2, 240.0, 0.0, 240.0, 120.0, 6.0),
            wall(3, 120.0, 100.0, 120.0, 0.0, 4.5),
        ];
        let plain = wall_outlines(&walls, 0.01);
        let layered = wall_layer_outlines(&walls, &[], 0.01);
        assert_eq!(layered.len(), 3);
        for (p, l) in plain.iter().zip(&layered) {
            assert_eq!(p.wall_id, l.wall_id);
            for (a, b) in p.polygon.iter().zip(&l.polygon) {
                assert!(a.dist(*b) < 1e-9, "{:?} vs {:?}", p.polygon, l.polygon);
            }
        }
    }

    #[test]
    fn different_layer_counts_use_nearest_boundary() {
        let d = crate::defaults::PlanDefaults::chief_x18_daniel();
        let ext = d.wall_type("Stucco-6").unwrap().clone();
        let int = d.wall_type("Interior-4").unwrap().clone();
        let mut walls = vec![
            typed_wall(1, 0.0, 0.0, 120.0, 0.0, &ext),
            typed_wall(2, 120.0, 0.0, 120.0, 120.0, &int),
        ];
        walls[0].exterior_side = crate::walls::Side::Right;
        walls[1].exterior_side = crate::walls::Side::Right;
        let o = wall_layer_outlines(&walls, &[ext, int], 0.01);
        assert_eq!(o.len(), 7);
        // Main layers (framing) meet exactly on the miter of their outside lines.
        let main_a = &o
            .iter()
            .find(|l| l.wall_id == 1 && l.is_main)
            .unwrap()
            .polygon;
        let main_b = &o
            .iter()
            .find(|l| l.wall_id == 2 && l.is_main)
            .unwrap()
            .polygon;
        let shared = main_a
            .iter()
            .filter(|p| main_b.iter().any(|q| q.dist(**p) < 1e-9))
            .count();
        assert!(shared >= 1, "{main_a:?} / {main_b:?}");
        assert!(o
            .iter()
            .all(|l| l.polygon.iter().all(|p| p.x.is_finite() && p.y.is_finite())));
    }

    #[test]
    fn end_joins_classify_corner_through_tee() {
        let walls = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0, 6.0),
            wall(2, 100.0, 0.0, 100.0, 100.0, 6.0),
            wall(3, 100.0, 0.0, 200.0, 0.0, 6.0),
            wall(4, 50.0, 80.0, 50.0, 0.0, 6.0),
        ];
        let j = wall_end_joins(&walls, 0, WallEnd::End, 0.5);
        assert!(j.contains(&(1, ConnectionKind::Corner)));
        assert!(j.contains(&(2, ConnectionKind::Through)));
        assert_eq!(
            wall_end_joins(&walls, 3, WallEnd::End, 0.5),
            vec![(0, ConnectionKind::Tee)]
        );
        assert!(wall_end_joins(&walls, 3, WallEnd::Start, 0.5).is_empty());
    }

    // ----- the grid broad-phase changes nothing -----

    /// A grid of walls with gaps, jitter, Ts, diagonals and mixed thickness.
    fn messy_plan(n: usize, seed: u64) -> Vec<Wall> {
        let mut s = seed;
        let mut rnd = move || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut walls = Vec::new();
        let mut id = 0;
        let mut add = |walls: &mut Vec<Wall>, a: (f64, f64), b: (f64, f64), t: f64| {
            id += 1;
            walls.push(wall(id, a.0, a.1, b.0, b.1, t));
        };
        for r in 0..=n {
            for c in 0..n {
                if rnd() < 0.15 {
                    continue;
                }
                let (x, y) = (c as f64 * 120.0, r as f64 * 120.0);
                let len = if rnd() < 0.1 { 60.0 } else { 120.0 };
                add(
                    &mut walls,
                    (x, y),
                    (x + len, y),
                    [4.5, 6.0, 7.625][(r + c) % 3],
                );
            }
        }
        for c in 0..=n {
            for r in 0..n {
                if rnd() < 0.15 {
                    continue;
                }
                let (x, y) = (c as f64 * 120.0, r as f64 * 120.0);
                add(&mut walls, (x, y), (x, y + 120.0), 4.5);
            }
        }
        for _ in 0..n {
            let x = (rnd() * n as f64).floor() * 120.0;
            let y = (rnd() * n as f64).floor() * 120.0;
            add(&mut walls, (x, y), (x + 120.0, y + 120.0), 4.5);
            add(&mut walls, (x + 60.0, y), (x + 60.0, y + 120.0), 4.5);
        }
        walls
    }

    #[test]
    fn outlines_with_the_grid_equal_outlines_from_a_scan() {
        for (n, seed) in [(6, 1), (9, 2), (14, 3)] {
            let walls = messy_plan(n, seed);
            assert!(walls.len() > GRID_MIN_WALLS, "{} walls", walls.len());
            let scan = Near::all(&walls);
            let a = format!("{:?}", outlines_with(&walls, 0.5, &Near::new(&walls, 0.5)));
            let b = format!("{:?}", outlines_with(&walls, 0.5, &scan));
            assert_eq!(a, b, "plan {n}");
            let types = crate::defaults::PlanDefaults::chief_x18_daniel().wall_types;
            let mut typed = walls.clone();
            for (i, w) in typed.iter_mut().enumerate() {
                if i % 2 == 0 {
                    w.wall_type = Some(types[i % types.len()].name.clone());
                }
            }
            let a = format!(
                "{:?}",
                layer_outlines_with(&typed, &types, 0.5, &Near::new(&typed, 0.5))
            );
            let b = format!("{:?}", layer_outlines_with(&typed, &types, 0.5, &scan));
            assert_eq!(a, b, "layers of plan {n}");
        }
    }

    #[test]
    fn walls_equal_sees_every_difference() {
        let a = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0, 6.0),
            wall(2, 0.0, 0.0, 0.0, 90.0, 4.5),
        ];
        assert!(walls_equal(&a, &a.clone()));
        let mut b = a.clone();
        b[1].thickness = 4.6;
        assert!(!walls_equal(&a, &b));
        let mut b = a.clone();
        b[0].flags.invisible = true;
        assert!(!walls_equal(&a, &b));
        let mut b = a.clone();
        b[0].wall_type = Some("x".into());
        assert!(!walls_equal(&a, &b));
        assert!(!walls_equal(&a, &a[..1]));
    }

    #[test]
    fn a_tangent_arc_and_its_neighbour_are_a_through_join() {
        let mut p = crate::model::Project::new("t");
        let a = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            crate::model::WallKind::Exterior,
        );
        let b = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(200.0, 100.0),
            6.0,
            100.0,
            crate::model::WallKind::Exterior,
        );
        // Straight neighbours at an angle: a corner.
        let kinds = |p: &crate::model::Project, id| {
            p.wall_connections(0, id)
                .into_iter()
                .map(|c| c.kind)
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds(&p, a), vec![ConnectionKind::Corner]);
        // Curve the second wall tangent to the first: now it continues it.
        p.floors[0].wall_mut(b).unwrap().curve = Some(crate::walls::WallCurve { bulge: -5.0 });
        p.make_arc_tangent(0, b).unwrap();
        assert_eq!(kinds(&p, a), vec![ConnectionKind::Through]);
        assert_eq!(kinds(&p, b), vec![ConnectionKind::Through]);
    }

    #[test]
    fn a_curved_wall_polygon_is_mitered_at_a_corner() {
        let mut walls = vec![
            Wall::new(
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                10.0,
                100.0,
                crate::model::WallKind::Exterior,
            ),
            Wall::new(
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
                10.0,
                100.0,
                crate::model::WallKind::Exterior,
            ),
        ];
        walls[0].id = 1;
        walls[1].id = 2;
        // The second wall bows to the right; the first is straight.
        walls[1].curve = Some(crate::walls::WallCurve { bulge: -10.0 });
        let straight = walls[1].plan_polygon();
        let poly = curved_wall_polygon(&walls, 2, 0.5).unwrap();
        assert_eq!(poly.len(), straight.len());
        // Away from the joined start the polygon is unchanged.
        let n = poly.len();
        let h = n / 2;
        assert_eq!(poly[h - 1], straight[h - 1]);
        assert_eq!(poly[h], straight[h]);
        // The start corner moved: it now lies on wall 1's outer face (y = -5)
        // or inner face (y = 5), not on the square end of the arc.
        let on_face = |p: Point| (p.y.abs() - 5.0).abs() < 1e-6;
        assert!(
            on_face(poly[0]) && on_face(poly[n - 1]),
            "{:?} {:?}",
            poly[0],
            poly[n - 1]
        );
        assert!(poly[0] != straight[0] || poly[n - 1] != straight[n - 1]);
        // A straight wall has no curved polygon.
        assert!(curved_wall_polygon(&walls, 1, 0.5).is_none());
        assert!(curved_wall_polygon(&walls, 99, 0.5).is_none());
        // A lone arc stays square.
        let lone = vec![walls[1].clone()];
        assert_eq!(curved_wall_polygon(&lone, 2, 0.5).unwrap(), straight);
    }

    // ----- curved walls: miters, tangent joins, layer outlines -----

    fn arc(id: u64, a: (f64, f64), b: (f64, f64), t: f64, bulge: f64) -> Wall {
        let mut w = wall(id, a.0, a.1, b.0, b.1, t);
        w.kind = WallKind::Exterior;
        w.curve = Some(crate::walls::WallCurve { bulge });
        w
    }

    /// Which side of the line `a`-`b` the point lies on (signed).
    fn side(a: Point, b: Point, p: Point) -> f64 {
        b.sub(a).cross(p.sub(a))
    }

    #[test]
    fn a_curved_wall_meets_a_straight_one_with_no_gap_or_overlap() {
        // Straight A into curved B into straight C: both ends of B are joined.
        let walls = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0, 10.0),
            arc(2, (100.0, 0.0), (160.0, 70.0), 10.0, 14.0),
            wall(3, 160.0, 70.0, 260.0, 70.0, 10.0),
        ];
        let out = wall_outlines(&walls, 0.5);
        let b = curved_wall_polygon(&walls, 2, 0.5).unwrap();
        let (n, h) = (b.len(), b.len() / 2);
        // The cut line at each joint is shared: A's end corners are B's
        // start corners, B's end corners are C's start corners.
        let same = |p: Point, q: Point| p.dist(q) < 1e-9;
        let (a_poly, c_poly) = (&out[0].polygon, &out[2].polygon);
        assert!(
            (same(a_poly[1], b[0]) && same(a_poly[2], b[n - 1]))
                || (same(a_poly[1], b[n - 1]) && same(a_poly[2], b[0])),
            "{a_poly:?} {:?} {:?}",
            b[0],
            b[n - 1]
        );
        assert!(
            (same(c_poly[0], b[h - 1]) && same(c_poly[3], b[h]))
                || (same(c_poly[0], b[h]) && same(c_poly[3], b[h - 1])),
            "{c_poly:?} {:?} {:?}",
            b[h - 1],
            b[h]
        );
        // The neighbours lie on the far side of the cut from the arc: no overlap.
        let (cut_a, cut_b) = (b[0], b[n - 1]);
        let arc_side = side(cut_a, cut_b, b[1]);
        for p in [a_poly[0], a_poly[3]] {
            assert!(side(cut_a, cut_b, p) * arc_side < 0.0, "A overlaps B");
        }
        let (cut_c, cut_d) = (b[h - 1], b[h]);
        let arc_side = side(cut_c, cut_d, b[h - 2]);
        for p in [c_poly[1], c_poly[2]] {
            assert!(side(cut_c, cut_d, p) * arc_side < 0.0, "C overlaps B");
        }
        // The mitered arc keeps its band: area is about arc length x thickness.
        let want = walls[1].path_length() * 10.0;
        let got = crate::geometry::polygon_area(&b).abs();
        assert!((got - want).abs() / want < 0.12, "area {got} vs {want}");
    }

    #[test]
    fn a_curved_wall_tee_on_a_curved_host_is_cut_to_its_face() {
        // A straight wall ends on the middle of an arc host.
        let host = arc(1, (0.0, 0.0), (200.0, 0.0), 10.0, 40.0);
        let (c, r) = host.arc_center_radius().unwrap();
        let apex = Point::new(100.0, 40.0);
        // Come in from inside the circle, ending on the arc's centerline.
        let from = apex.sub(c).normalized().scale(r - 80.0).add(c);
        let walls = vec![host, wall(2, from.x, from.y, apex.x, apex.y, 6.0)];
        let out = wall_outlines(&walls, 0.5);
        // The tee wall's end corners sit on the host's inner face (radius r - 5).
        let end_pts = [out[1].polygon[1], out[1].polygon[2]];
        for p in end_pts {
            assert!(
                (p.dist(c) - (r - 5.0)).abs() < 0.2,
                "tee corner {p:?} is {} from the center, wanted {}",
                p.dist(c),
                r - 5.0
            );
        }
        assert_eq!(
            wall_end_joins(&walls, 1, WallEnd::End, 0.5),
            vec![(0, ConnectionKind::Tee)]
        );
    }

    #[test]
    fn two_tangent_arcs_join_square_and_a_corner_of_arcs_is_mitered() {
        // B leaves A's end along A's tangent: a through join, ends coincide.
        let a = arc(1, (0.0, 0.0), (100.0, 0.0), 8.0, 25.0);
        let tangent = a.curve.unwrap().tangent_at_end(a.start, a.end);
        let end = Point::new(190.0, -60.0);
        let curve = crate::walls::WallCurve::tangent_to(a.end, end, tangent).unwrap();
        let mut b = wall(2, 100.0, 0.0, end.x, end.y, 8.0);
        b.kind = WallKind::Exterior;
        b.curve = Some(curve);
        let walls = vec![a, b];
        assert_eq!(
            wall_end_joins(&walls, 0, WallEnd::End, 0.5),
            vec![(1, ConnectionKind::Through)]
        );
        let pa = curved_wall_polygon(&walls, 1, 0.5).unwrap();
        let pb = curved_wall_polygon(&walls, 2, 0.5).unwrap();
        let (ha, nb) = (pa.len() / 2, pb.len());
        assert!(pa[ha - 1].dist(pb[0]) < 1e-6, "left corners meet");
        assert!(pa[ha].dist(pb[nb - 1]) < 1e-6, "right corners meet");
        // Same band thickness through the joint: no step.
        assert!((pa[ha - 1].dist(pa[ha]) - 8.0).abs() < 1e-6);

        // A kinked pair of arcs (no common tangent) is mitered instead.
        let c = arc(3, (0.0, 0.0), (100.0, 0.0), 8.0, 25.0);
        let d = arc(4, (100.0, 0.0), (140.0, -80.0), 8.0, -10.0);
        let walls = vec![c, d];
        assert_eq!(
            wall_end_joins(&walls, 0, WallEnd::End, 0.5),
            vec![(1, ConnectionKind::Corner)]
        );
        let pc = curved_wall_polygon(&walls, 3, 0.5).unwrap();
        let pd = curved_wall_polygon(&walls, 4, 0.5).unwrap();
        let (hc, nd) = (pc.len() / 2, pd.len());
        let shared = |p: Point, q: Point| p.dist(q) < 1e-9;
        assert!(
            (shared(pc[hc - 1], pd[0]) && shared(pc[hc], pd[nd - 1]))
                || (shared(pc[hc - 1], pd[nd - 1]) && shared(pc[hc], pd[0]))
        );
    }

    #[test]
    fn layer_outlines_of_a_curved_wall_follow_the_arc() {
        let ty = stucco();
        let chord = 240.0;
        let curve = crate::walls::WallCurve::from_radius(chord, 120.0, true).unwrap();
        let mut w = typed_wall(1, 0.0, 0.0, chord, 0.0, &ty);
        w.curve = Some(curve);
        let (center, r) = w.arc_center_radius().unwrap();
        assert!((r - 120.0).abs() < 1e-9);
        let walls = vec![w.clone()];
        let outs = wall_layer_outlines(&walls, std::slice::from_ref(&ty), 0.5);
        assert_eq!(outs.len(), 4, "one outline per layer");
        let bands = wall_layer_bands(&w, Some(&ty));
        // The arc turns clockwise (it bulges left), so the left side is outside.
        let sign = curve.sweep(w.start, w.end).signum();
        for (o, band) in outs.iter().zip(&bands) {
            assert!(o.polygon.len() > 8, "faceted, not four corners");
            let mut radii: Vec<f64> = o.polygon.iter().map(|p| p.dist(center)).collect();
            radii.sort_by(f64::total_cmp);
            let want_lo = r - sign * band.outer.max(band.inner);
            let want_hi = r - sign * band.outer.min(band.inner);
            let (lo, hi) = (want_lo.min(want_hi), want_lo.max(want_hi));
            assert!((radii[0] - lo).abs() < 1e-6, "{} inner radius", o.name);
            assert!(
                (radii[radii.len() - 1] - hi).abs() < 1e-6,
                "{} outer",
                o.name
            );
            // Every point is on one of the two boundary radii.
            assert!(radii
                .iter()
                .all(|d| (d - lo).abs() < 1e-6 || (d - hi).abs() < 1e-6));
        }
        // More facets on request: a finer polygon for a high zoom.
        let fine =
            curved_layer_outlines(&walls, std::slice::from_ref(&ty), 1, 0.5, Some(90)).unwrap();
        assert_eq!(fine[0].polygon.len(), 2 * 91);
        assert!(curved_layer_outlines(&walls, &[], 9, 0.5, None).is_none());
        let line = vec![wall(5, 0.0, 0.0, 10.0, 0.0, 4.0)];
        assert!(curved_layer_outlines(&line, &[], 5, 0.5, None).is_none());
    }

    #[test]
    fn a_curved_layer_stack_is_mitered_to_a_straight_neighbour() {
        let ty = stucco();
        let mut a = typed_wall(1, 0.0, 0.0, 120.0, 0.0, &ty);
        a.curve = None;
        let mut b = typed_wall(2, 120.0, 0.0, 180.0, 90.0, &ty);
        b.curve = Some(crate::walls::WallCurve { bulge: 15.0 });
        let walls = vec![a, b];
        let outs = wall_layer_outlines(&walls, std::slice::from_ref(&ty), 0.5);
        // Layer k of the straight wall ends where layer k of the arc starts.
        let straight: Vec<_> = outs.iter().filter(|o| o.wall_id == 1).collect();
        let curved: Vec<_> = outs.iter().filter(|o| o.wall_id == 2).collect();
        assert_eq!(straight.len(), curved.len());
        for (s, c) in straight.iter().zip(&curved) {
            let n = c.polygon.len();
            let ends = [s.polygon[1], s.polygon[2]];
            let starts = [c.polygon[0], c.polygon[n - 1]];
            for e in ends {
                assert!(
                    starts.iter().any(|p| p.dist(e) < 1e-6),
                    "layer {} of the arc does not start on the straight wall's end",
                    c.name
                );
            }
        }
    }

    #[test]
    fn the_outline_of_a_curved_wall_is_its_mitered_arc_band() {
        let walls = vec![
            wall(1, 0.0, 0.0, 100.0, 0.0, 10.0),
            arc(2, (100.0, 0.0), (160.0, 70.0), 10.0, 14.0),
        ];
        let out = wall_outlines(&walls, 0.5);
        let band = curved_wall_polygon(&walls, 2, 0.5).unwrap();
        assert_eq!(out[1].polygon, band);
        assert!(
            out[1].polygon.len() > 8,
            "the arc is faceted, not a rectangle"
        );
        // The straight neighbour keeps its four corners.
        assert_eq!(out[0].polygon.len(), 4);
        // A lone arc's outline is the unjoined band.
        let lone = vec![walls[1].clone()];
        assert_eq!(wall_outlines(&lone, 0.5)[0].polygon, lone[0].plan_polygon());
    }

    #[test]
    fn curve_facets_follow_the_radius_and_zoom() {
        let (a, b) = (Point::new(0.0, 0.0), Point::new(240.0, 0.0));
        let c = crate::walls::WallCurve::from_radius(240.0, 120.0, true).unwrap();
        let base = c.facet_count(a, b);
        assert!(base >= 24);
        // A loose tolerance keeps the facet angle's count; a tight one adds.
        assert_eq!(c.facet_count_for_sag(a, b, 5.0), base);
        let fine = c.facet_count_for_sag(a, b, 0.01);
        assert!(fine > base * 2, "{fine} vs {base}");
        // A 40 ft radius needs more facets than a 5 ft one for the same sweep.
        let big = crate::walls::WallCurve::from_radius(960.0, 480.0, true).unwrap();
        let small = crate::walls::WallCurve::from_radius(120.0, 60.0, true).unwrap();
        let (e, f) = (Point::new(960.0, 0.0), Point::new(120.0, 0.0));
        assert!(big.facet_count_for_sag(a, e, 0.05) > small.facet_count_for_sag(a, f, 0.05));
    }
}
