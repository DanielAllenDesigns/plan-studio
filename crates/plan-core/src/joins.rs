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
use crate::geometry::{project_on_segment, Point};
use crate::model::{Id, Wall, WallEnd};

/// Maximum miter length as a multiple of the thicker wall.
pub const MITER_LIMIT: f64 = 4.0;
/// `|sin|` of the angle between two walls below which they count as parallel.
const MIN_SIN: f64 = 0.02;

#[derive(Debug, Clone)]
pub struct WallOutline {
    pub wall_id: Id,
    /// Four points: start-left, end-left, end-right, start-right, where left
    /// is the side of the +normal (same order as [`Wall::footprint`]).
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

/// Unit direction pointing from the given end back along the wall.
fn away_dir(w: &Wall, e: End) -> Point {
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

/// Outlines for every wall, in input order.
pub fn wall_outlines(walls: &[Wall], tol: f64) -> Vec<WallOutline> {
    walls
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let (sl, el, er, sr) = wall_faces(w);
            let (sl, sr) = end_faces(walls, i, End::Start, tol).unwrap_or((sl, sr));
            let (el, er) = end_faces(walls, i, End::End, tol).unwrap_or((el, er));
            WallOutline {
                wall_id: w.id,
                polygon: vec![sl, el, er, sr],
            }
        })
        .collect()
}

/// Joined `(left, right)` face points at one end, or `None` for a square end.
fn end_faces(walls: &[Wall], i: usize, e: End, tol: f64) -> Option<(Point, Point)> {
    let w = &walls[i];
    if w.length() <= tol {
        return None;
    }
    let p = end_point(w, e);
    let mut touching = Vec::new();
    for (j, o) in walls.iter().enumerate() {
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
        0 => t_faces(walls, i, e, p, tol),
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

/// The nearest wall (other than `i`) whose interior, not its ends, contains
/// `p`, with the closest point on it: the host of a T-junction at `p`.
fn tee_host(walls: &[Wall], i: usize, p: Point, tol: f64) -> Option<(usize, Point)> {
    let mut best: Option<(f64, usize, Point)> = None;
    for (j, t) in walls.iter().enumerate() {
        if j == i || t.length() <= tol {
            continue;
        }
        let (_, q) = project_on_segment(p, t.start, t.end);
        let d = q.dist(p);
        if d <= tol
            && q.dist(t.start) > tol
            && q.dist(t.end) > tol
            && best.is_none_or(|(bd, _, _)| d < bd)
        {
            best = Some((d, j, q));
        }
    }
    best.map(|(_, j, q)| (j, q))
}

fn t_faces(walls: &[Wall], i: usize, e: End, p: Point, tol: f64) -> Option<(Point, Point)> {
    let w = &walls[i];
    let dw = away_dir(w, e);
    let (tj, q) = tee_host(walls, i, p, tol)?;
    let t = &walls[tj];

    let dt = t.direction();
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
        if let Some((j, _)) = tee_host(walls, i, p, tol) {
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
    stacks: &[Vec<LayerBand>],
    i: usize,
    e: End,
    tol: f64,
) -> Vec<Point> {
    let w = &walls[i];
    let p = end_point(w, e);
    let qa = end_sign(e);
    let (own, _, _) = boundaries(&stacks[i], 1.0);
    let square = || -> Vec<Point> { own.iter().map(|l| p + w.normal() * *l).collect() };
    if w.length() <= tol {
        return square();
    }
    let mut touching = Vec::new();
    for (j, o) in walls.iter().enumerate() {
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
            let Some((tj, q)) = tee_host(walls, i, p, tol) else {
                return square();
            };
            let t = &walls[tj];
            let dt = t.direction();
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
        let starts = layer_end_points(walls, &stacks, i, End::Start, tol);
        let ends = layer_end_points(walls, &stacks, i, End::End, tol);
        let (b, _, _) = boundaries(&stacks[i], 1.0);
        for (k, band) in stacks[i].iter().enumerate() {
            let (hi, lo) = if b[k] >= b[k + 1] {
                (k, k + 1)
            } else {
                (k + 1, k)
            };
            out.push(WallLayerOutline {
                wall_id: w.id,
                layer_index: k,
                name: band.name.clone(),
                is_main: band.is_main,
                polygon: vec![starts[hi], ends[hi], ends[lo], starts[lo]],
            });
        }
    }
    out
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
}
