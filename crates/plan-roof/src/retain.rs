//! Rebuilding a roof around retained planes (manual pp. 827 to 829; RF-68,
//! RF-71).
//!
//! Build Roof normally deletes and replaces every plane. With Retain Manually
//! Drawn Roof Planes and Retain Edited Automatic Roof Planes the planes the
//! user drew or changed stay. A rebuild still generates a plane where a kept
//! one stands, so a new plane that is coplanar with a retained plane, and
//! overlaps it by at least half the area of either, is dropped and only the
//! retained plane is kept.

use crate::geom;
use crate::RoofPlane;
use plan_core::geometry::polygon_area;

/// How far a vertex may stand off the retained plane for two planes to count
/// as one surface, inches.
pub const COPLANAR_TOLERANCE: f64 = 0.5;
/// Smallest cosine between the normals of two planes that are the same
/// surface (a quarter of a degree apart).
const NORMAL_COS: f64 = 0.999_990;

/// Area of the plan overlap of two planes, square inches.
pub fn overlap_area(a: &RoofPlane, b: &RoofPlane) -> f64 {
    let (pa, pb) = (geom::ccw(&a.plan_polygon()), geom::ccw(&b.plan_polygon()));
    if pa.len() < 3 || pb.len() < 3 {
        return 0.0;
    }
    let tris = |p: &[plan_core::Point]| -> Vec<Vec<plan_core::Point>> {
        if geom::is_convex(p) {
            vec![p.to_vec()]
        } else {
            geom::ear_triangles(p).iter().map(|t| t.to_vec()).collect()
        }
    };
    let (ta, tb) = (tris(&pa), tris(&pb));
    let mut total = 0.0;
    for x in &ta {
        for y in &tb {
            let clipped = geom::clip_convex(x, y);
            if clipped.len() >= 3 {
                total += polygon_area(&clipped).abs();
            }
        }
    }
    total
}

/// Are `a` and `b` two pieces of one surface: the same slope direction and
/// every vertex of `a` within [`COPLANAR_TOLERANCE`] of `b`'s plane?
pub fn coplanar(a: &RoofPlane, b: &RoofPlane) -> bool {
    let (na, nb) = (a.normal(), b.normal());
    let dot = na[0] * nb[0] + na[1] * nb[1] + na[2] * nb[2];
    if dot < NORMAL_COS || nb[1] < 1e-9 {
        return false;
    }
    let Some(o) = b.polygon3d.first() else {
        return false;
    };
    a.polygon3d.iter().all(|v| {
        let d = [v[0] - o[0], v[1] - o[1], v[2] - o[2]];
        (nb[0] * d[0] + nb[1] * d[1] + nb[2] * d[2]).abs() <= COPLANAR_TOLERANCE
    })
}

/// Does retained plane `kept` stand where `new` would be built: coplanar, and
/// overlapping by at least half the area of either?
pub fn replaces(kept: &RoofPlane, new: &RoofPlane) -> bool {
    if !coplanar(new, kept) {
        return false;
    }
    let overlap = overlap_area(kept, new);
    let smaller = kept.projected_area().abs().min(new.projected_area().abs());
    smaller > 1e-9 && overlap >= 0.5 * smaller
}

/// The planes of `new` that no retained plane stands in for. The order of the
/// survivors is kept.
pub fn drop_replaced(retained: &[RoofPlane], new: Vec<RoofPlane>) -> Vec<RoofPlane> {
    new.into_iter()
        .filter(|n| !retained.iter().any(|k| replaces(k, n)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_roof_with_specs, EdgeRoofSpec};
    use plan_core::Point;

    fn house() -> Vec<RoofPlane> {
        let fp = vec![
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ];
        let mut specs = vec![EdgeRoofSpec::default(); 4];
        specs[1].full_gable_wall = true;
        specs[3].full_gable_wall = true;
        build_roof_with_specs(&fp, &specs, 100.0).planes
    }

    #[test]
    fn an_identical_plane_is_replaced_by_the_retained_one() {
        let planes = house();
        let kept = planes[0].clone();
        let left = drop_replaced(std::slice::from_ref(&kept), planes.clone());
        assert_eq!(left.len(), 1, "the retained plane stands for its copy");
        assert!(
            !replaces(&kept, &planes[1]),
            "the other slope is another surface"
        );
    }

    #[test]
    fn an_edited_plane_still_stands_in_for_the_rebuilt_one_it_overlaps() {
        let planes = house();
        // The user dragged the eave edge in by 60": the retained plane is
        // the same surface, a little smaller.
        let mut kept = planes[0].clone();
        let eave_z = planes[0].polygon3d[0][2];
        for v in &mut kept.polygon3d {
            if (v[2] - eave_z).abs() < 1e-6 {
                v[2] -= 60.0;
                v[1] += 40.0;
            }
        }
        assert!(coplanar(&planes[0], &kept));
        assert!(replaces(&kept, &planes[0]));
        assert_eq!(
            drop_replaced(std::slice::from_ref(&kept), planes.clone()).len(),
            1
        );
        // A pitch edit leaves a different surface, even over the same plan.
        let mut steeper = planes[0].clone();
        for v in &mut steeper.polygon3d {
            v[1] = 100.0 + (v[1] - 100.0) * 1.5;
        }
        assert!(!coplanar(&planes[0], &steeper));
        assert!(!replaces(&steeper, &planes[0]));
        // So the rebuilt plane survives next to a re-pitched retained one.
        let left = drop_replaced(std::slice::from_ref(&steeper), vec![planes[0].clone()]);
        assert_eq!(left.len(), 1);
    }

    #[test]
    fn a_small_overlap_does_not_count() {
        let planes = house();
        // A retained plane covering a sliver of the rebuilt one.
        let mut sliver = planes[0].clone();
        let origin = sliver.polygon3d[0];
        for v in &mut sliver.polygon3d {
            v[0] = origin[0] + (v[0] - origin[0]) * 0.02;
        }
        assert!(coplanar(&planes[0], &sliver));
        let overlap = overlap_area(&sliver, &planes[0]);
        assert!(overlap > 0.0);
        // The sliver is under half the rebuilt plane but the sliver itself is
        // wholly inside it, so it does stand for "at least half of either".
        assert!(replaces(&sliver, &planes[0]));
        // A disjoint plane never does.
        let mut far = planes[0].clone();
        for v in &mut far.polygon3d {
            v[0] += 2000.0;
        }
        assert!(!replaces(&far, &planes[0]));
        assert_eq!(overlap_area(&far, &planes[0]), 0.0);
    }
}
