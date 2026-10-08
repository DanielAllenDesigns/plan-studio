//! Ceiling planes (Chief "Ceiling Plane" and "Build Ceiling Planes", RF-45/46).
//!
//! A [`CeilingPlane`] is a sloped ceiling surface over a plan region. For a
//! vaulted room it follows the roof: parallel to each roof plane above it,
//! lowered by the roof structure thickness.

use crate::geom;
use crate::RoofPlane;
use plan_core::geometry::polygon_area;
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
}
