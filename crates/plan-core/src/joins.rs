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

use crate::geometry::{project_on_segment, Point};
use crate::model::{Id, Wall};

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

fn t_faces(walls: &[Wall], i: usize, e: End, p: Point, tol: f64) -> Option<(Point, Point)> {
    let w = &walls[i];
    let dw = away_dir(w, e);

    // Nearest wall whose interior (not its ends) contains p.
    let mut best: Option<(f64, &Wall, Point)> = None;
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
            best = Some((d, t, q));
        }
    }
    let (_, t, q) = best?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WallKind;

    fn wall(id: u64, x0: f64, y0: f64, x1: f64, y1: f64, t: f64) -> Wall {
        Wall {
            id,
            start: Point::new(x0, y0),
            end: Point::new(x1, y1),
            thickness: t,
            height: 109.125,
            kind: WallKind::Interior,
            layer: "Walls, Normal".into(),
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
}
