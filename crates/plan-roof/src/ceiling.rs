//! Ceiling planes (Chief "Ceiling Plane" and "Build Ceiling Planes", RF-45/46).
//!
//! A [`CeilingPlane`] is a sloped ceiling surface over a plan region. For a
//! vaulted room it follows the roof: parallel to each roof plane above it,
//! lowered by the roof structure thickness.

use crate::geom;
use crate::RoofPlane;
use plan_core::geometry::{point_in_polygon, polygon_area};
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// A sloped ceiling surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CeilingPlane {
    /// Plan outline of the ceiling surface (inches, counter-clockwise).
    pub outline: Vec<Point>,
    /// The low edge: the plane rises toward the left of `baseline.0 -> .1`.
    pub baseline: (Point, Point),
    /// Rise per 12 of run away from the baseline.
    pub pitch_in_12: f64,
    /// Elevation of the ceiling surface along the baseline, inches (the same
    /// elevation scale as roof planes: scene Y).
    pub height_at_baseline: f64,
    /// Structure thickness above the visible ceiling surface, inches (along
    /// the plane normal).
    pub thickness: f64,
}

impl CeilingPlane {
    /// Elevation of the ceiling surface above plan point `p`.
    pub fn height_at(&self, p: Point) -> f64 {
        let (a, b) = self.baseline;
        let len = a.dist(b);
        if len <= 1e-9 {
            return self.height_at_baseline;
        }
        let run = b.sub(a).cross(p.sub(a)) / len;
        self.height_at_baseline + run * self.pitch_in_12 / 12.0
    }

    /// The outline lifted onto the plane, in roof space (`X = x`, `Y` up,
    /// `Z = -y`), counter-clockwise seen from above.
    pub fn polygon3d(&self) -> Vec<[f64; 3]> {
        geom::ccw(&self.outline)
            .iter()
            .map(|&p| geom::lift(p, self.height_at(p)))
            .collect()
    }

    /// Plan area, square inches.
    pub fn plan_area(&self) -> f64 {
        polygon_area(&self.outline).abs()
    }

    /// True sloped surface area, square inches.
    pub fn area(&self) -> f64 {
        self.plan_area() * (1.0 + (self.pitch_in_12 / 12.0).powi(2)).sqrt()
    }

    /// Unit normal of the ceiling surface seen from above (up component > 0).
    pub fn normal(&self) -> [f64; 3] {
        geom::unit3(geom::newell(&self.polygon3d())).unwrap_or([0.0, 1.0, 0.0])
    }
}

/// Plan overlap of a room and a roof plane's plan outline (zero or more
/// polygons; more than one only when both are concave).
fn overlap(room: &[Point], plane: &[Point]) -> Vec<Vec<Point>> {
    let (room, plane) = (geom::ccw(room), geom::ccw(plane));
    let pieces: Vec<Vec<Point>> = if geom::is_convex(&plane) {
        vec![geom::clip_convex(&room, &plane)]
    } else if geom::is_convex(&room) {
        vec![geom::clip_convex(&plane, &room)]
    } else {
        geom::ear_triangles(&plane)
            .iter()
            .map(|t| geom::clip_convex(&room, t))
            .collect()
    };
    pieces
        .into_iter()
        .filter(|p| p.len() >= 3 && polygon_area(p).abs() > 1.0)
        .collect()
}

/// Ceiling planes for a vaulted room: Chief's "Ceiling Over This Room" off
/// plus a vaulted ceiling, so the ceiling follows the roof minus its structure.
///
/// For every sloped roof plane whose plan outline overlaps `room_poly`, one
/// ceiling plane is produced over the overlap, with the roof plane's baseline
/// and pitch, lowered so that it sits `thickness` inches (along the plane
/// normal) below the roof surface. Flat or vertical planes are skipped.
/// Planes come out in `roof_planes` order, so the result is deterministic.
pub fn ceiling_planes_for_vaulted_room(
    room_poly: &[Point],
    roof_planes: &[RoofPlane],
    thickness: f64,
) -> Vec<CeilingPlane> {
    let thickness = thickness.max(0.0);
    let mut out = Vec::new();
    for plane in roof_planes {
        if plane.pitch_in_12 <= 1e-6 || plane.polygon3d.len() < 3 {
            continue;
        }
        let eave_y = plane.polygon3d[0][1];
        let drop = thickness * (144.0 + plane.pitch_in_12 * plane.pitch_in_12).sqrt() / 12.0;
        for piece in overlap(room_poly, &plane.plan_polygon()) {
            out.push(CeilingPlane {
                outline: piece,
                baseline: plane.baseline,
                pitch_in_12: plane.pitch_in_12,
                height_at_baseline: eave_y - drop,
                thickness,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Cathedral ceilings, shelf ceilings and sampling (R-146, R-32)
// ---------------------------------------------------------------------------

/// A plant shelf: a smaller room with its own flat ceiling inside the area of
/// a cathedral ceiling. The cathedral ceiling stops at its outline.
#[derive(Debug, Clone, PartialEq)]
pub struct Shelf {
    /// Plan outline of the shelf room.
    pub outline: Vec<Point>,
    /// Elevation of the shelf room's flat ceiling (the same scale as the
    /// ceiling planes: scene Y).
    pub height: f64,
}

/// `poly` (any winding) clipped to the side of the line `a -> b`: the left
/// side when `left`, else the right.
fn clip_half_plane(poly: &[Point], a: Point, b: Point, left: bool) -> Vec<Point> {
    let side = |p: Point| {
        let s = b.sub(a).cross(p.sub(a));
        if left {
            s
        } else {
            -s
        }
    };
    let n = poly.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (cur, prev) = (poly[i], poly[(i + n - 1) % n]);
        let (sc, sp) = (side(cur), side(prev));
        let cut = |sp: f64, sc: f64| Point::lerp(prev, cur, sp / (sp - sc));
        if sc >= 0.0 {
            if sp < 0.0 {
                out.push(cut(sp, sc));
            }
            out.push(cur);
        } else if sp >= 0.0 {
            out.push(cut(sp, sc));
        }
    }
    out
}

/// `piece` (convex) minus the convex polygon `hole`: up to one convex piece
/// per edge of the hole, together exactly the part of `piece` outside it.
fn subtract_convex(piece: &[Point], hole: &[Point]) -> Vec<Vec<Point>> {
    let hole = geom::ccw(hole);
    let m = hole.len();
    let mut out = Vec::new();
    let mut inside: Vec<Point> = piece.to_vec();
    for k in 0..m {
        if inside.len() < 3 {
            break;
        }
        let (a, b) = (hole[k], hole[(k + 1) % m]);
        let outside = clip_half_plane(&inside, a, b, false);
        if outside.len() >= 3 && polygon_area(&outside).abs() > 1.0 {
            out.push(outside);
        }
        inside = clip_half_plane(&inside, a, b, true);
    }
    out
}

/// The parts of the convex `piece` that lie outside `hole` (any simple
/// polygon). A concave hole is split into triangles first.
pub fn subtract_polygon(piece: &[Point], hole: &[Point]) -> Vec<Vec<Point>> {
    let parts: Vec<Vec<Point>> = if geom::is_convex(&geom::ccw(hole)) {
        vec![hole.to_vec()]
    } else {
        geom::ear_triangles(hole)
            .iter()
            .map(|t| t.to_vec())
            .collect()
    };
    let mut pieces = vec![geom::ccw(piece)];
    for part in &parts {
        pieces = pieces
            .iter()
            .flat_map(|p| subtract_convex(p, part))
            .collect();
    }
    pieces
}

/// Ceiling planes for a room by its Flat Ceiling Over This Room switch: none
/// when `flat` (the room keeps its flat ceiling platform), otherwise the
/// cathedral ceiling that follows the roof (see
/// [`ceiling_planes_for_vaulted_room`]) with every shelf room's outline cut
/// out of it. Planes come out in roof-plane order, then the pieces of each.
pub fn cathedral_ceiling_planes(
    flat: bool,
    room_poly: &[Point],
    shelves: &[Shelf],
    roof_planes: &[RoofPlane],
    thickness: f64,
) -> Vec<CeilingPlane> {
    if flat {
        return Vec::new();
    }
    let mut out = Vec::new();
    for plane in ceiling_planes_for_vaulted_room(room_poly, roof_planes, thickness) {
        let mut pieces = vec![geom::ccw(&plane.outline)];
        for shelf in shelves {
            pieces = pieces
                .iter()
                .flat_map(|p| subtract_polygon(p, &shelf.outline))
                .collect();
        }
        for outline in pieces {
            if outline.len() >= 3 && polygon_area(&outline).abs() > 1.0 {
                out.push(CeilingPlane {
                    outline,
                    ..plane.clone()
                });
            }
        }
    }
    out
}

/// Elevation of the ceiling surface of whichever of `planes` holds plan point
/// `p` (the highest when two meet there), or `None` when none does.
pub fn ceiling_height_at(planes: &[CeilingPlane], p: Point) -> Option<f64> {
    planes
        .iter()
        .filter(|c| geom::boundary_dist(p, &c.outline) < 1e-6 || point_in_polygon(p, &c.outline))
        .map(|c| c.height_at(p))
        .max_by(f64::total_cmp)
}

/// Elevation of the underside of the roof above plan point `p`, less the
/// `thickness` of its structure (along the normal): where a cathedral
/// ceiling is at `p`. `None` when no sloped roof plane is over `p`.
pub fn cathedral_height_at(roof_planes: &[RoofPlane], thickness: f64, p: Point) -> Option<f64> {
    let thickness = thickness.max(0.0);
    roof_planes
        .iter()
        .filter(|pl| pl.pitch_in_12 > 1e-6 && pl.polygon3d.len() >= 3)
        .filter(|pl| {
            let poly = pl.plan_polygon();
            point_in_polygon(p, &poly) || geom::boundary_dist(p, &poly) < 1e-6
        })
        .filter_map(|pl| {
            let drop = thickness * (144.0 + pl.pitch_in_12 * pl.pitch_in_12).sqrt() / 12.0;
            pl.height_at(p).map(|h| h - drop)
        })
        .max_by(f64::total_cmp)
}

/// The finished ceiling elevation at `p` in a room: a shelf's own flat ceiling
/// wins (the lowest if shelves overlap); otherwise a flat room is at
/// `flat_height` and a cathedral room is at its ceiling plane over `p`
/// (`flat_height` where none reaches).
pub fn room_ceiling_height_at(
    flat: bool,
    flat_height: f64,
    planes: &[CeilingPlane],
    shelves: &[Shelf],
    p: Point,
) -> f64 {
    let shelf = shelves
        .iter()
        .filter(|s| point_in_polygon(p, &s.outline))
        .map(|s| s.height)
        .min_by(f64::total_cmp);
    if let Some(h) = shelf {
        return h;
    }
    if flat {
        return flat_height;
    }
    ceiling_height_at(planes, p).unwrap_or(flat_height)
}

// ---------------------------------------------------------------------------
// Tray ceilings as ceiling planes (Explode Tray Ceiling)
// ---------------------------------------------------------------------------

/// The ceiling planes a tray ceiling is made of (manual p. 457): the outer
/// ceiling (a ring, cut into convex pieces) at `h_outer`; with `recess` (Recess
/// into Ceiling) the raised inner ceiling at `h_inner` (without it the inner
/// ceiling is the room's own ceiling platform, which stays); and, for
/// sloped sides (`pitch_in_12` above zero), one sloped plane along each edge
/// of the hole that rises to the inner ceiling over a horizontal `run`.
/// Heights are elevations on the scale the planes use (scene Y); `inner` and
/// `outer` are the hole and the outside edge of the outer ceiling.
pub fn tray_ceiling_planes(
    inner: &[Point],
    outer: &[Point],
    h_outer: f64,
    h_inner: f64,
    recess: bool,
    pitch_in_12: f64,
    run: f64,
    thickness: f64,
) -> Vec<CeilingPlane> {
    let inner = geom::ccw(inner);
    let outer = geom::ccw(outer);
    let flat = |outline: Vec<Point>, height: f64| {
        let base = (outline[0], outline[1 % outline.len()]);
        CeilingPlane {
            outline,
            baseline: base,
            pitch_in_12: 0.0,
            height_at_baseline: height,
            thickness,
        }
    };
    let mut out = Vec::new();
    let sloped = pitch_in_12 > 1e-6 && run > 1e-6;
    let band_edge = if sloped {
        plan_core::tray::outset_outline(&inner, run)
    } else {
        inner.clone()
    };
    for piece in subtract_polygon(&outer, &band_edge) {
        out.push(flat(piece, h_outer));
    }
    if sloped {
        let n = inner.len();
        for i in 0..n {
            let (a, b) = (inner[i], inner[(i + 1) % n]);
            let (ao, bo) = (band_edge[i], band_edge[(i + 1) % n]);
            if a.dist(b) < 1e-6 {
                continue;
            }
            out.push(CeilingPlane {
                outline: vec![ao, bo, b, a],
                baseline: (ao, bo),
                pitch_in_12,
                height_at_baseline: h_outer,
                thickness,
            });
        }
    }
    if recess && h_inner > h_outer + 1e-6 {
        out.push(flat(inner, h_inner));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hole::tests::gable_roof_planes;

    fn room_40x30() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 360.0),
            Point::new(0.0, 360.0),
        ]
    }

    #[test]
    fn gable_roof_gives_two_ceiling_planes_meeting_at_the_ridge() {
        let roof = gable_roof_planes();
        assert_eq!(roof.len(), 2);
        let ceil = ceiling_planes_for_vaulted_room(&room_40x30(), &roof, 9.0);
        assert_eq!(ceil.len(), 2);
        // Both planes reach the same height on the ridge line (y = 180).
        let ridge = Point::new(240.0, 180.0);
        let (h0, h1) = (ceil[0].height_at(ridge), ceil[1].height_at(ridge));
        assert!((h0 - h1).abs() < 1e-6, "{h0} vs {h1}");
        // Roof ridge is 120" above the eave; the ceiling is 9" (normal) lower.
        let drop = 9.0 * (144.0f64 + 64.0).sqrt() / 12.0;
        assert!((h0 - (228.0 - drop)).abs() < 1e-6);
        // Together they cover the room; each is half.
        let area: f64 = ceil.iter().map(CeilingPlane::plan_area).sum();
        assert!((area - 480.0 * 360.0).abs() < 1e-3);
        // Slope direction: the ceiling rises toward the ridge on both sides.
        assert!(ceil[0].height_at(Point::new(240.0, 20.0)) < h0);
        assert!(ceil[1].height_at(Point::new(240.0, 340.0)) < h1);
        // 9" perpendicular thickness: the surface is 9" under the roof plane.
        let roof_h = roof[0].height_at(Point::new(240.0, 20.0)).unwrap();
        let under = roof_h - ceil[0].height_at(Point::new(240.0, 20.0));
        assert!((under - drop).abs() < 1e-6);
    }

    #[test]
    fn room_smaller_than_the_roof_is_clipped() {
        let roof = gable_roof_planes();
        let room = vec![
            Point::new(60.0, 60.0),
            Point::new(300.0, 60.0),
            Point::new(300.0, 120.0),
            Point::new(60.0, 120.0),
        ];
        let ceil = ceiling_planes_for_vaulted_room(&room, &roof, 6.0);
        // The room sits entirely under the south plane.
        assert_eq!(ceil.len(), 1);
        assert!((ceil[0].plan_area() - 240.0 * 60.0).abs() < 1e-3);
        assert!(ceil[0].polygon3d().len() >= 4);
        assert!(ceil[0].normal()[1] > 0.0);
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    #[test]
    fn subtracting_a_polygon_keeps_the_rest_exactly() {
        let piece = rect(0.0, 0.0, 100.0, 80.0);
        // A hole in the middle: the four pieces cover the ring.
        let hole = rect(30.0, 20.0, 70.0, 60.0);
        let parts = subtract_polygon(&piece, &hole);
        let area: f64 = parts.iter().map(|p| polygon_area(p).abs()).sum();
        assert!((area - (8000.0 - 1600.0)).abs() < 1e-6, "{area}");
        for p in &parts {
            let c = plan_core::geometry::polygon_centroid(p);
            assert!(!point_in_polygon(c, &hole));
        }
        // A concave (L shaped) hole is handled through its triangles.
        let ell = vec![
            Point::new(10.0, 10.0),
            Point::new(60.0, 10.0),
            Point::new(60.0, 30.0),
            Point::new(30.0, 30.0),
            Point::new(30.0, 60.0),
            Point::new(10.0, 60.0),
        ];
        let parts = subtract_polygon(&piece, &ell);
        let area: f64 = parts.iter().map(|p| polygon_area(p).abs()).sum();
        assert!(
            (area - (8000.0 - polygon_area(&ell).abs())).abs() < 1e-6,
            "{area}"
        );
        // A hole outside leaves the piece whole.
        let far = rect(200.0, 200.0, 220.0, 220.0);
        let parts = subtract_polygon(&piece, &far);
        let area: f64 = parts.iter().map(|p| polygon_area(p).abs()).sum();
        assert!((area - 8000.0).abs() < 1e-6);
    }

    #[test]
    fn flat_ceiling_switch_decides_cathedral_planes() {
        let roof = gable_roof_planes();
        let room = room_40x30();
        assert!(cathedral_ceiling_planes(true, &room, &[], &roof, 9.0).is_empty());
        let cath = cathedral_ceiling_planes(false, &room, &[], &roof, 9.0);
        assert_eq!(cath.len(), 2);
        assert_eq!(cath, ceiling_planes_for_vaulted_room(&room, &roof, 9.0));
    }

    #[test]
    fn cathedral_follows_the_roof_and_sampling_agrees() {
        let roof = gable_roof_planes();
        let room = room_40x30();
        let cath = cathedral_ceiling_planes(false, &room, &[], &roof, 9.0);
        // The ceiling rises toward the ridge at y = 180 and equals the roof
        // underside minus the structure at every sample.
        let mut last = -1.0;
        for y in [20.0, 60.0, 100.0, 140.0, 180.0] {
            let p = Point::new(240.0, y);
            let h = ceiling_height_at(&cath, p).unwrap();
            let r = cathedral_height_at(&roof, 9.0, p).unwrap();
            assert!((h - r).abs() < 1e-6, "{h} vs {r}");
            assert!(h > last);
            last = h;
        }
        // The flat height is the eave; the ridge is 120" above it.
        let ridge = ceiling_height_at(&cath, Point::new(240.0, 180.0)).unwrap();
        let eave = ceiling_height_at(&cath, Point::new(240.0, 0.0)).unwrap();
        assert!((ridge - eave - 120.0).abs() < 1e-6);
        // Outside the roof: nothing.
        assert!(cathedral_height_at(&roof, 9.0, Point::new(-500.0, 0.0)).is_none());
    }

    #[test]
    fn room_height_is_flat_cathedral_or_a_shelf() {
        let roof = gable_roof_planes();
        let room = room_40x30();
        let cath = cathedral_ceiling_planes(false, &room, &[], &roof, 9.0);
        let at = Point::new(240.0, 150.0);
        let flat = room_ceiling_height_at(true, 96.0, &cath, &[], at);
        assert_eq!(flat, 96.0);
        let vault = room_ceiling_height_at(false, 96.0, &cath, &[], at);
        assert!(vault > 96.0 + 50.0, "{vault}");
        // A plant shelf at 84": its own flat ceiling inside the vault.
        let shelf = Shelf {
            outline: rect(200.0, 120.0, 280.0, 170.0),
            height: 84.0,
        };
        assert_eq!(
            room_ceiling_height_at(false, 96.0, &cath, &[shelf.clone()], at),
            84.0
        );
        // The vault has a hole where the shelf is.
        let shelved = cathedral_ceiling_planes(false, &room, &[shelf.clone()], &roof, 9.0);
        let area: f64 = shelved.iter().map(CeilingPlane::plan_area).sum();
        assert!(
            (area - (480.0 * 360.0 - 80.0 * 50.0)).abs() < 1e-3,
            "{area}"
        );
        assert!(shelved.iter().all(|c| c.outline.len() >= 3));
        assert!(ceiling_height_at(&shelved, at).is_none());
        // Beside the shelf the vault is as before.
        let beside = Point::new(100.0, 150.0);
        assert!(
            (room_ceiling_height_at(false, 96.0, &shelved, &[shelf], beside)
                - room_ceiling_height_at(false, 96.0, &cath, &[], beside))
            .abs()
                < 1e-6
        );
    }

    #[test]
    fn tray_planes_cover_the_ring_and_slope_up_the_sides() {
        let outer = rect(0.0, 0.0, 240.0, 180.0);
        let inner = rect(24.0, 24.0, 216.0, 156.0);
        // Vertical sides, ring dropped 8": the ring only.
        let planes = tray_ceiling_planes(&inner, &outer, 88.0, 96.0, false, 0.0, 0.0, 0.625);
        let area: f64 = planes.iter().map(CeilingPlane::plan_area).sum();
        assert!((area - (240.0 * 180.0 - 192.0 * 132.0)).abs() < 1e-6);
        assert!(planes
            .iter()
            .all(|c| c.pitch_in_12 == 0.0 && c.height_at_baseline == 88.0));
        // Sloped (12 in 12, 8" deep): a band of 8" along each of 4 edges.
        let planes = tray_ceiling_planes(&inner, &outer, 88.0, 96.0, false, 12.0, 8.0, 0.625);
        let bands: Vec<&CeilingPlane> = planes.iter().filter(|c| c.pitch_in_12 > 0.0).collect();
        assert_eq!(bands.len(), 4);
        for b in &bands {
            // Low at the outer edge, 8" higher at the hole.
            let lo = b.height_at(b.baseline.0);
            let hi = b.height_at(Point::lerp(b.outline[2], b.outline[3], 0.5));
            assert!((lo - 88.0).abs() < 1e-6, "{lo}");
            assert!((hi - 96.0).abs() < 1e-6, "{hi}");
        }
        let flat: f64 = planes
            .iter()
            .filter(|c| c.pitch_in_12 == 0.0)
            .map(CeilingPlane::plan_area)
            .sum();
        let band: f64 = bands.iter().map(|c| c.plan_area()).sum();
        assert!((flat + band - (240.0 * 180.0 - 192.0 * 132.0)).abs() < 1e-6);
        // Recess: the inner ceiling is a plane too, raised.
        let planes = tray_ceiling_planes(&inner, &outer, 96.0, 104.0, true, 0.0, 0.0, 0.625);
        let top = planes
            .iter()
            .find(|c| c.height_at_baseline == 104.0)
            .unwrap();
        assert!((top.plan_area() - 192.0 * 132.0).abs() < 1e-6);
    }
}
