//! Roles of roof plane edges (eave, rake, ridge, hip, valley) and the plane's
//! underside, for the 3D eave detail (soffit, fascia, rake boards, ridge caps).

use crate::RoofPlane;
use plan_core::Point;

/// What a polygon edge of a roof plane is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeRole {
    /// The baseline edge (`polygon3d[0] -> [1]`).
    Eave,
    /// A sloped edge no other plane shares: the gable end.
    Rake,
    /// A level edge two planes share, at the top.
    Ridge,
    /// A sloped edge two planes share, convex.
    Hip,
    /// An edge two planes share, concave.
    Valley,
    /// A level edge nothing shares (the high edge of a shed plane).
    Top,
}

/// One edge of one plane with its role.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneEdge {
    /// Index into the plane list.
    pub plane: usize,
    /// The edge runs from vertex `index` to the next.
    pub index: usize,
    pub role: EdgeRole,
    pub a: [f64; 3],
    pub b: [f64; 3],
    /// The plane sharing the edge, for ridges, hips and valleys.
    pub other: Option<usize>,
}

/// Distance tolerance for matching shared edges, inches.
const TOL: f64 = 0.5;

fn dist_to_line3(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    if len2 < 1e-12 {
        return f64::INFINITY;
    }
    let w = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let t = (w[0] * d[0] + w[1] * d[1] + w[2] * d[2]) / len2;
    let q = [w[0] - d[0] * t, w[1] - d[1] * t, w[2] - d[2] * t];
    (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt()
}

/// Length of the overlap of segments `ab` and `cd` that lie on one line.
fn overlap_len(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    if dist_to_line3(c, a, b) > TOL || dist_to_line3(d, a, b) > TOL {
        return 0.0;
    }
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len = (ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2]).sqrt();
    if len < 1e-9 {
        return 0.0;
    }
    let along =
        |p: [f64; 3]| ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / len;
    let (t0, t1) = (along(c).min(along(d)), along(c).max(along(d)));
    (t1.min(len) - t0.max(0.0)).max(0.0)
}

fn centroid_plan(plane: &RoofPlane) -> Point {
    let pts = plane.plan_polygon();
    let n = pts.len().max(1) as f64;
    let s = pts.iter().fold(Point::ZERO, |acc, p| acc + *p);
    s.scale(1.0 / n)
}

/// Every edge of every plane with its role. Edge `0` of a plane is its eave;
/// an edge another plane's edge overlaps is a ridge or hip (the neighbour
/// falls away below this plane's extension) or a valley (it rises above it);
/// the rest are rakes (sloped) or tops (level).
pub fn classify_edges(planes: &[RoofPlane]) -> Vec<PlaneEdge> {
    let mut out = Vec::new();
    for (k, plane) in planes.iter().enumerate() {
        let n = plane.polygon3d.len();
        if n < 3 {
            continue;
        }
        for i in 0..n {
            let (a, b) = (plane.polygon3d[i], plane.polygon3d[(i + 1) % n]);
            let len =
                ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            if len < 1e-6 {
                continue;
            }
            let level = (b[1] - a[1]).abs() < 0.01 * len + 1e-6;
            if i == 0 {
                out.push(PlaneEdge {
                    plane: k,
                    index: i,
                    role: EdgeRole::Eave,
                    a,
                    b,
                    other: None,
                });
                continue;
            }
            let shared = planes.iter().enumerate().find(|(l, q)| {
                *l != k && {
                    let m = q.polygon3d.len();
                    (0..m)
                        .any(|j| overlap_len(a, b, q.polygon3d[j], q.polygon3d[(j + 1) % m]) > 1.0)
                }
            });
            let (role, other) = match shared {
                Some((l, q)) => {
                    let c = centroid_plan(q);
                    let below = plane
                        .height_at(c)
                        .zip(q.height_at(c))
                        .is_none_or(|(mine, theirs)| theirs <= mine + 1e-6);
                    let role = if !below {
                        EdgeRole::Valley
                    } else if level {
                        EdgeRole::Ridge
                    } else {
                        EdgeRole::Hip
                    };
                    (role, Some(l))
                }
                None if level => (EdgeRole::Top, None),
                None => (EdgeRole::Rake, None),
            };
            out.push(PlaneEdge {
                plane: k,
                index: i,
                role,
                a,
                b,
                other,
            });
        }
    }
    out
}

impl RoofPlane {
    /// Elevation of the underside above plan point `p` for a roof `thickness`
    /// inches thick (measured along the plane normal), or `None` for a
    /// vertical or degenerate plane.
    pub fn underside_at(&self, p: Point, thickness: f64) -> Option<f64> {
        let top = self.height_at(p)?;
        let ny = self.normal()[1];
        Some(top - thickness.max(0.0) / ny.max(1e-9))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_roof, EdgeKind, EdgeRoof};

    fn rect() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 360.0),
            Point::new(0.0, 360.0),
        ]
    }

    fn count(edges: &[PlaneEdge], role: EdgeRole) -> usize {
        edges.iter().filter(|e| e.role == role).count()
    }

    #[test]
    fn a_gable_roof_has_two_eaves_four_rakes_and_a_ridge() {
        let gable = EdgeRoof {
            kind: EdgeKind::Gable,
            ..EdgeRoof::default()
        };
        // Edges 1 and 3 are the short ends.
        let edges = [EdgeRoof::default(), gable, EdgeRoof::default(), gable];
        let roof = build_roof(&rect(), &edges, 96.0);
        let e = classify_edges(&roof.planes);
        assert_eq!(roof.planes.len(), 2);
        assert_eq!(count(&e, EdgeRole::Eave), 2);
        assert_eq!(count(&e, EdgeRole::Rake), 4);
        assert_eq!(count(&e, EdgeRole::Ridge), 2, "one per plane");
        assert_eq!(count(&e, EdgeRole::Hip) + count(&e, EdgeRole::Valley), 0);
    }

    #[test]
    fn a_hip_roof_has_no_rakes() {
        let roof = build_roof(&rect(), &[EdgeRoof::default(); 4], 96.0);
        let e = classify_edges(&roof.planes);
        assert_eq!(count(&e, EdgeRole::Rake), 0);
        assert_eq!(count(&e, EdgeRole::Eave), 4);
        assert!(count(&e, EdgeRole::Hip) >= 8);
        assert!(count(&e, EdgeRole::Ridge) >= 2);
    }

    #[test]
    fn the_underside_hangs_below_the_surface_along_the_normal() {
        let roof = build_roof(&rect(), &[EdgeRoof::default(); 4], 96.0);
        let plane = &roof.planes[0];
        let p = plane.plan_polygon()[0].add(Point::new(0.0, 0.0));
        let top = plane.height_at(p).unwrap();
        let under = plane.underside_at(p, 6.0).unwrap();
        let ny = plane.normal()[1];
        assert!((top - under - 6.0 / ny).abs() < 1e-9);
    }
}
