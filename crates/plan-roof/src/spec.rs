//! Per-edge roof overrides for the automatic roof builder (Chief wall
//! "Roof" tab: pitch, overhang, Full Gable Wall, High Shed/Gable Wall, Extend
//! Slope Downward; RF-18..RF-26).

use crate::geom::{self, V3};
use crate::{build_roof, EdgeKind, EdgeRoof, Roof, RoofPlane};
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Roof directive of one footprint edge.
///
/// Mapping onto the straight-skeleton builder ([`build_roof`]), in priority
/// order:
///
/// 1. `high_shed_gable`: [`EdgeKind::Shed`] with zero overhang (the wall is
///    the high side; no plane and no overhang on that side).
/// 2. `gable` or `full_gable_wall`: [`EdgeKind::Gable`] (vertical end; the
///    neighbouring planes extend to this edge plus its overhang as the rake).
///    The two flags are equivalent here: the gable wall always reaches the
///    ridge.
/// 3. otherwise [`EdgeKind::Hip`] rising at `pitch`.
///
/// `extend_slope_downward: Some(d)` keeps the edge's plane sloping `d` inches
/// (vertical) below the eave line after the roof is built.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeRoofSpec {
    /// Rise per 12 of run.
    pub pitch: f64,
    /// Horizontal overhang beyond the footprint edge, inches.
    pub overhang: f64,
    pub gable: bool,
    pub full_gable_wall: bool,
    pub high_shed_gable: bool,
    pub extend_slope_downward: Option<f64>,
}

impl Default for EdgeRoofSpec {
    /// Hip, 8:12, 16" overhang.
    fn default() -> Self {
        Self {
            pitch: 8.0,
            overhang: 16.0,
            gable: false,
            full_gable_wall: false,
            high_shed_gable: false,
            extend_slope_downward: None,
        }
    }
}

impl EdgeRoofSpec {
    /// The skeleton-builder settings for this edge.
    pub fn to_edge_roof(&self) -> EdgeRoof {
        let (kind, overhang) = if self.high_shed_gable {
            (EdgeKind::Shed, 0.0)
        } else if self.gable || self.full_gable_wall {
            (EdgeKind::Gable, self.overhang)
        } else {
            (EdgeKind::Hip, self.overhang)
        };
        EdgeRoof {
            pitch_in_12: self.pitch,
            kind,
            overhang,
        }
    }
}

/// Extend `plane` down its fall line so the eave drops `drop` inches. The new
/// strip is added below the old eave; the polygon still starts with the eave.
pub(crate) fn extend_downward(plane: &RoofPlane, drop: f64) -> RoofPlane {
    let n = plane.normal();
    let p = &plane.polygon3d;
    if drop <= 1e-9 || p.len() < 3 || n[1] <= 1e-9 {
        return plane.clone();
    }
    let h2 = n[0] * n[0] + n[2] * n[2];
    let Some(f) = geom::unit3([n[0], -h2 / n[1], n[2]]) else {
        return plane.clone();
    };
    if f[1] >= -1e-9 {
        return plane.clone(); // flat plane: no fall line
    }
    let shift = geom::scale3(f, drop / -f[1]);
    let (v0, v1) = (geom::add3(p[0], shift), geom::add3(p[1], shift));
    let mut poly: Vec<V3> = vec![v0, v1];
    poly.push(p[1]);
    poly.extend(p[2..].iter().copied());
    poly.push(p[0]);
    RoofPlane {
        baseline: (geom::to_plan(v0), geom::to_plan(v1)),
        polygon3d: poly,
        pitch_in_12: plane.pitch_in_12,
        source_edge: plane.source_edge,
    }
}

/// Build a roof from per-edge [`EdgeRoofSpec`]s.
///
/// This is [`build_roof`] driven by Chief-style wall directives. Edges beyond
/// `specs.len()` use [`EdgeRoofSpec::default`]. Pitches may differ freely from
/// edge to edge: the skeleton is a true weighted straight skeleton (edge `i`
/// advances at `12 / pitch_i` inches per inch of rise), so a 4:12 and an 8:12
/// face meet in a ridge that is offset toward the steeper side. There is no
/// 2:12 limit. The only limit is the one documented on [`build_roof`]: in rare
/// parallel-wall jog cases the exact skeleton is not available and the roof is
/// approximated ([`Roof::approximate`] is set).
///
/// `extend_slope_downward` is applied afterwards: the
/// plane keeps its slope and gains a strip below its eave; the hips it shares
/// with neighbouring planes are not continued below the old eave.
pub fn build_roof_with_specs(
    footprint: &[Point],
    specs: &[EdgeRoofSpec],
    baseline_elevation: f64,
) -> Roof {
    let spec_of = |i: usize| specs.get(i).copied().unwrap_or_default();
    let edges: Vec<EdgeRoof> = (0..footprint.len())
        .map(|i| spec_of(i).to_edge_roof())
        .collect();
    let mut roof = build_roof(footprint, &edges, baseline_elevation);
    for plane in &mut roof.planes {
        if let Some(d) = spec_of(plane.source_edge).extend_slope_downward {
            *plane = extend_downward(plane, d);
        }
    }
    roof
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ]
    }

    fn gable_ends(south: f64, north: f64) -> Vec<EdgeRoofSpec> {
        let hip = |pitch| EdgeRoofSpec {
            pitch,
            overhang: 0.0,
            ..EdgeRoofSpec::default()
        };
        let gable = EdgeRoofSpec {
            gable: true,
            overhang: 0.0,
            ..EdgeRoofSpec::default()
        };
        vec![hip(south), gable, hip(north), gable]
    }

    /// Plan y of the ridge (highest vertices) of a roof.
    fn ridge_y(roof: &Roof) -> f64 {
        let (_, hi) = roof.bounds().unwrap();
        let ys: Vec<f64> = roof
            .planes
            .iter()
            .flat_map(|p| p.polygon3d.iter())
            .filter(|p| (p[1] - hi[1]).abs() < 1e-6)
            .map(|p| -p[2])
            .collect();
        assert!(!ys.is_empty());
        ys.iter().sum::<f64>() / ys.len() as f64
    }

    #[test]
    fn equal_pitches_put_the_ridge_in_the_middle() {
        let roof = build_roof_with_specs(&rect(), &gable_ends(8.0, 8.0), 100.0);
        assert!(!roof.approximate);
        assert_eq!(roof.planes.len(), 2);
        assert!((ridge_y(&roof) - 144.0).abs() < 1e-6);
    }

    #[test]
    fn four_vs_eight_in_twelve_offsets_the_ridge_toward_the_steeper_side() {
        let roof = build_roof_with_specs(&rect(), &gable_ends(4.0, 8.0), 100.0);
        assert!(!roof.approximate);
        assert_eq!(roof.planes.len(), 2);
        // Wavefront speeds 3 (4:12) and 1.5 (8:12) per inch of rise meet after
        // 288 / 4.5 = 64" of rise, at 3 * 64 = 192" from the south eave.
        assert!((ridge_y(&roof) - 192.0).abs() < 1e-6);
        let (_, hi) = roof.bounds().unwrap();
        assert!((hi[1] - 164.0).abs() < 1e-6);
        let south = roof.planes.iter().find(|p| p.source_edge == 0).unwrap();
        let north = roof.planes.iter().find(|p| p.source_edge == 2).unwrap();
        assert!((south.pitch_in_12 - 4.0).abs() < 1e-9);
        assert!((north.pitch_in_12 - 8.0).abs() < 1e-9);
        // The two faces tile the footprint and share the ridge exactly.
        let area: f64 = roof.planes.iter().map(RoofPlane::projected_area).sum();
        assert!((area - 480.0 * 288.0).abs() < 1e-3);
    }

    #[test]
    fn flags_map_to_edge_kinds() {
        let s = EdgeRoofSpec::default();
        assert_eq!(s.to_edge_roof().kind, EdgeKind::Hip);
        let g = EdgeRoofSpec {
            full_gable_wall: true,
            ..s
        };
        assert_eq!(g.to_edge_roof().kind, EdgeKind::Gable);
        let h = EdgeRoofSpec {
            gable: true,
            high_shed_gable: true,
            ..s
        };
        let e = h.to_edge_roof();
        assert_eq!((e.kind, e.overhang), (EdgeKind::Shed, 0.0));
    }

    #[test]
    fn high_shed_wall_gives_a_lean_to() {
        let mut specs = vec![EdgeRoofSpec::default(); 4];
        specs[0].pitch = 4.0;
        specs[0].overhang = 0.0;
        specs[1].gable = true;
        specs[2].high_shed_gable = true;
        specs[3].gable = true;
        let roof = build_roof_with_specs(&rect(), &specs, 100.0);
        assert_eq!(roof.planes.len(), 1);
        let (lo, hi) = roof.bounds().unwrap();
        // 288" run at 4:12 rises 96" up to the high wall.
        assert!((hi[1] - lo[1] - 96.0).abs() < 1e-6);
    }

    #[test]
    fn extend_slope_downward_adds_a_strip_below_the_eave() {
        let mut specs = gable_ends(8.0, 8.0);
        specs[0].extend_slope_downward = Some(12.0);
        let roof = build_roof_with_specs(&rect(), &specs, 100.0);
        let south = roof.planes.iter().find(|p| p.source_edge == 0).unwrap();
        let lowest = south.polygon3d.iter().fold(f64::MAX, |m, p| m.min(p[1]));
        assert!((lowest - 88.0).abs() < 1e-6);
        // Still one flat plane, same pitch, normal up.
        assert!((south.pitch_in_12 - 8.0).abs() < 1e-9);
        assert!(south.normal()[1] > 0.0);
        let n = south.normal();
        for p in &south.polygon3d {
            let o = south.polygon3d[0];
            let d = geom::sub3(*p, o);
            assert!(geom::dot3(d, n).abs() < 1e-6);
        }
        // First edge is still the (lowered) eave and is horizontal.
        assert!((south.polygon3d[0][1] - south.polygon3d[1][1]).abs() < 1e-9);
        // The other side is untouched.
        let north = roof.planes.iter().find(|p| p.source_edge == 2).unwrap();
        assert!((north.polygon3d[0][1] - 100.0).abs() < 1e-9);
    }
}
