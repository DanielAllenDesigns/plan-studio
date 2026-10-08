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
}
