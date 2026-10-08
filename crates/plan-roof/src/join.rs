//! Join Roof Planes (Chief RF-41).
//!
//! Chief's Join Roof Planes joins two planes along the line where they
//! intersect: the edge picked on the first plane is extended or trimmed to
//! the intersection line with the second plane. The second plane is not
//! changed.

use crate::geom;
use crate::RoofPlane;
use plan_core::geometry::polygon_area;
use plan_core::Point;

/// Intersection of the infinite lines `p + t d` and `q + s e`; `None` when
/// they are parallel.
fn line_hit(p: Point, d: Point, q: Point, e: Point) -> Option<Point> {
    let den = d.cross(e);
    if den.abs() < 1e-9 * d.length().max(1e-12) * e.length().max(1e-12) {
        return None;
    }
    let t = q.sub(p).cross(e) / den;
    Some(p.add(d.scale(t)))
}

/// Joins plane `a` to plane `b` along `a`'s polygon edge `edge`
/// (`polygon3d[edge] -> polygon3d[edge + 1]`).
///
/// Both planes extend infinitely for the purpose of the join. Their plan
/// intersection line is where the two plane heights are equal; the two
/// vertices of `edge` slide along their neighbouring edges until they sit on
/// it, so the edge is extended when the planes meet beyond it and trimmed
/// when they meet inside. The vertices keep the height of plane `a`, which
/// therefore stays the same plane. A vertex that moves also moves the
/// matching end of `baseline` (vertex 0 and 1 are the eave, as in
/// [`RoofPlane::polygon3d`]).
///
/// Returns `None` when an index is out of range, a plane is vertical or
/// degenerate, the planes are parallel, an adjacent edge runs parallel to the
/// intersection line, or the result would collapse or fold over.
pub fn join_planes(a: &RoofPlane, edge: usize, b: &RoofPlane) -> Option<RoofPlane> {
    let n = a.polygon3d.len();
    if n < 3 || edge >= n || b.polygon3d.len() < 3 {
        return None;
    }
    // Heights as affine functions of the plan point.
    let o = Point::ZERO;
    let (ha0, hb0) = (a.height_at(o)?, b.height_at(o)?);
    let ga = Point::new(
        a.height_at(Point::new(1.0, 0.0))? - ha0,
        a.height_at(Point::new(0.0, 1.0))? - ha0,
    );
    let gb = Point::new(
        b.height_at(Point::new(1.0, 0.0))? - hb0,
        b.height_at(Point::new(0.0, 1.0))? - hb0,
    );
    let g = ga.sub(gb);
    let g2 = g.dot(g);
    if g2 < 1e-12 {
        return None; // parallel planes never meet in a line
    }
    // f(p) = (ha0 - hb0) + g . p is zero on the intersection line.
    let on_line = g.scale(-(ha0 - hb0) / g2);
    let along = g.perp();

    let plan = a.plan_polygon();
    let (i0, i1) = (edge, (edge + 1) % n);
    let prev = plan[(edge + n - 1) % n];
    let next = plan[(edge + 2) % n];
    let v0 = line_hit(plan[i0], plan[i0].sub(prev), on_line, along)?;
    let v1 = line_hit(plan[i1], plan[i1].sub(next), on_line, along)?;
    if v0.dist(v1) < 1e-6 {
        return None;
    }

    let mut out = a.clone();
    for (i, v) in [(i0, v0), (i1, v1)] {
        out.polygon3d[i] = geom::lift(v, a.height_at(v)?);
        if i == 0 {
            out.baseline.0 = v;
        } else if i == 1 {
            out.baseline.1 = v;
        }
    }
    // The result must still be a proper polygon facing the same way.
    let before = polygon_area(&plan);
    let after = polygon_area(&out.plan_polygon());
    if after.abs() < 1e-6 || before.signum() != after.signum() {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hole::tests::gable_roof_planes;

    /// A rectangle plane of south's slope (rise 8 in 12 away from y = 0)
    /// between plan y `y0` and `y1`, x 0..480, eave first.
    fn strip(y1: f64) -> RoofPlane {
        let h = |y: f64| 108.0 + y * 8.0 / 12.0;
        let poly = vec![
            geom::lift(Point::new(0.0, 0.0), h(0.0)),
            geom::lift(Point::new(480.0, 0.0), h(0.0)),
            geom::lift(Point::new(480.0, y1), h(y1)),
            geom::lift(Point::new(0.0, y1), h(y1)),
        ];
        RoofPlane {
            polygon3d: poly,
            pitch_in_12: 8.0,
            baseline: (Point::new(0.0, 0.0), Point::new(480.0, 0.0)),
            source_edge: 0,
        }
    }

    fn north() -> RoofPlane {
        let planes = gable_roof_planes();
        planes.into_iter().find(|p| p.source_edge == 2).unwrap()
    }

    #[test]
    fn a_short_plane_is_extended_to_the_ridge() {
        let a = strip(120.0);
        let out = join_planes(&a, 2, &north()).unwrap();
        // Its top edge now sits on the ridge, y = 180, 228" up.
        for i in [2, 3] {
            let v = out.polygon3d[i];
            assert!((-v[2] - 180.0).abs() < 1e-6, "{v:?}");
            assert!((v[1] - 228.0).abs() < 1e-6, "{v:?}");
        }
        // x extents and the eave are untouched; still one plane.
        assert!((out.polygon3d[2][0] - 480.0).abs() < 1e-6);
        assert_eq!(out.baseline, a.baseline);
        for v in &out.polygon3d {
            let y = a.height_at(Point::new(v[0], -v[2])).unwrap();
            assert!((y - v[1]).abs() < 1e-6);
        }
        assert!(out.area() > a.area());
    }

    #[test]
    fn a_long_plane_is_trimmed_to_the_ridge() {
        let a = strip(260.0);
        let out = join_planes(&a, 2, &north()).unwrap();
        for i in [2, 3] {
            assert!((-out.polygon3d[i][2] - 180.0).abs() < 1e-6);
        }
        assert!(out.area() < a.area());
        // Joining a plane to its own continuation is a no-op.
        let again = join_planes(&out, 2, &north()).unwrap();
        for (p, q) in again.polygon3d.iter().zip(&out.polygon3d) {
            assert!((p[0] - q[0]).abs() + (p[1] - q[1]).abs() + (p[2] - q[2]).abs() < 1e-6);
        }
    }

    #[test]
    fn the_eave_edge_moves_the_baseline() {
        // Joining the eave edge slides it up the sides to the ridge line, and
        // the baseline follows.
        let a = strip(240.0);
        let out = join_planes(&a, 0, &north()).unwrap();
        assert!((out.baseline.0.y - 180.0).abs() < 1e-6);
        assert!((out.baseline.1.y - 180.0).abs() < 1e-6);
        assert!((out.polygon3d[0][1] - 228.0).abs() < 1e-6);
        // Past the far edge the plane would fold over: refused.
        assert!(join_planes(&strip(120.0), 0, &north()).is_none());
    }

    #[test]
    fn parallel_planes_and_bad_edges_do_not_join() {
        let a = strip(120.0);
        let mut b = strip(120.0);
        for v in &mut b.polygon3d {
            v[1] += 24.0;
        }
        assert!(join_planes(&a, 2, &b).is_none());
        assert!(join_planes(&a, 9, &north()).is_none());
        // A side edge's neighbours run parallel to the ridge line: they never
        // reach it.
        assert!(join_planes(&a, 1, &north()).is_none());
    }
}
