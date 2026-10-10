//! Roof Baseline Polylines (manual pp. 850 to 852; RF-62, RF-125..RF-127).
//!
//! A roof does not have to follow the footprint of the walls. A porch, an
//! addition or a roof over an existing roof is built from a closed polyline
//! whose edges carry roof directives, like the roof panel of a wall: the
//! Roof Baseline Polyline. Make Roof Baseline Polylines creates one along the
//! outside of the exterior walls, the user reshapes it and gives each edge a
//! directive (Roof Baseline Specification), and Use Existing Roof Baselines
//! builds the planes from the polylines instead of from the walls.
//!
//! This module is the geometry and the data. Edges are straight (the polyline
//! cannot be curved or severed) and the polyline's overhangs are measured from
//! the polyline itself, which already lies on the outside face of the walls.
//! The directive letters shown along an edge are [`directive_text`].
//!
//! The data lives in a [`RoofBaseline`] next to the polyline's points: the
//! application keeps the points in a CAD polyline and this record (the
//! Baseline Height and one [`BaselineEdge`] per edge) beside it, keyed by the
//! polyline's id.

use crate::geom::{self, V3};
use crate::spec::{build_roof_with_faces, EdgeRoofSpec};
use crate::Roof;
use plan_core::geometry::polygon_area;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// The roof option of one baseline edge (the first six options of the Roof
/// Baseline panel; Extend Slope Downward and Against Wall are separate
/// switches because they combine with the others).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BaselineOption {
    /// A plane rises from the edge.
    #[default]
    Hip,
    /// A vertical gable end at the edge.
    FullGable,
    /// A plane up to the break, a short gable above it.
    DutchGable,
    /// The high side of a shed or gable: no plane, no overhang.
    HighShed,
    /// The plane bears on a knee wall; the edge is labelled K.
    Knee,
}

impl BaselineOption {
    pub const ALL: [BaselineOption; 5] = [
        BaselineOption::Hip,
        BaselineOption::FullGable,
        BaselineOption::DutchGable,
        BaselineOption::HighShed,
        BaselineOption::Knee,
    ];

    /// The name the Roof Baseline panel gives the option.
    pub fn label(self) -> &'static str {
        match self {
            BaselineOption::Hip => "Hip Wall",
            BaselineOption::FullGable => "Full Gable Wall",
            BaselineOption::DutchGable => "Dutch Gable Wall",
            BaselineOption::HighShed => "High Shed/Gable Wall",
            BaselineOption::Knee => "Knee Wall",
        }
    }
}

/// The roof directives of one edge of a Roof Baseline Polyline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BaselineEdge {
    pub option: BaselineOption,
    /// Extend Slope Downward: the plane continues this far below the eave,
    /// inches.
    pub extend_down: Option<f64>,
    /// Against Wall: the roof rising from the other baselines butts a wall at
    /// this edge. Like High Shed/Gable Wall: no plane, no overhang.
    pub against_wall: bool,
    /// Rise per 12 of run.
    pub pitch: f64,
    /// Horizontal overhang beyond the baseline, inches.
    pub overhang: f64,
    /// Second pitch above the break (rise per 12), or for a Full Gable the
    /// pitch of the half hip that clips its peak.
    pub upper_pitch: Option<f64>,
    /// Rise above the eave, inches, where the second pitch or the Dutch gable
    /// starts.
    pub break_rise: Option<f64>,
}

impl Default for BaselineEdge {
    /// Hip, 8:12, 16" overhang (the stock roof defaults).
    fn default() -> Self {
        Self {
            option: BaselineOption::Hip,
            extend_down: None,
            against_wall: false,
            pitch: 8.0,
            overhang: 16.0,
            upper_pitch: None,
            break_rise: None,
        }
    }
}

impl BaselineEdge {
    /// Does a plane rise from this edge?
    pub fn rises(&self) -> bool {
        !self.against_wall
            && !matches!(
                self.option,
                BaselineOption::FullGable | BaselineOption::HighShed
            )
    }

    /// The skeleton-builder directive of this edge.
    pub fn to_spec(&self) -> EdgeRoofSpec {
        let mut s = EdgeRoofSpec {
            pitch: self.pitch,
            overhang: self.overhang.max(0.0),
            ..EdgeRoofSpec::default()
        };
        match self.option {
            BaselineOption::Hip | BaselineOption::Knee => {}
            BaselineOption::FullGable => s.full_gable_wall = true,
            BaselineOption::DutchGable => s.dutch_gable = true,
            BaselineOption::HighShed => s.high_shed_gable = true,
        }
        // Against Wall is a High Shed/Gable Wall at the plane's own end.
        if self.against_wall {
            s.high_shed_gable = true;
        }
        if s.high_shed_gable {
            return s;
        }
        if self.option == BaselineOption::FullGable {
            // A gable wall has no plane to break: the second pitch is the
            // hip that clips its peak (a half hip).
            s.half_hip_pitch = self.upper_pitch.filter(|p| *p > 0.0);
            s.half_hip_rise = self.break_rise.filter(|r| *r > 0.0);
        } else {
            s.break_rise = self.break_rise.filter(|r| *r > 0.0);
            if self.option != BaselineOption::DutchGable {
                s.upper_pitch = self.upper_pitch.filter(|p| *p > 0.0);
            }
            s.extend_slope_downward = self.extend_down.filter(|d| *d > 0.0);
        }
        s
    }

    /// The edge a wall's directive (`spec`) becomes on a baseline lying on the
    /// outside face of a wall `half_thickness` out from the centerline.
    pub fn from_spec(spec: &EdgeRoofSpec, half_thickness: f64) -> Self {
        let option = if spec.high_shed_gable {
            BaselineOption::HighShed
        } else if spec.dutch_gable {
            BaselineOption::DutchGable
        } else if spec.gable || spec.full_gable_wall {
            BaselineOption::FullGable
        } else {
            BaselineOption::Hip
        };
        let (upper_pitch, break_rise) = if option == BaselineOption::FullGable {
            (spec.half_hip_pitch, spec.half_hip_rise)
        } else {
            (spec.upper_pitch, spec.break_rise)
        };
        Self {
            option,
            extend_down: spec.extend_slope_downward,
            against_wall: false,
            pitch: spec.pitch,
            overhang: (spec.overhang - half_thickness).max(0.0),
            upper_pitch,
            break_rise,
        }
    }
}

/// The Roof Baseline Specification of one polyline: the Baseline Height and
/// the directives of its edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofBaseline {
    /// Baseline Height: the elevation of the eaves above the floor datum of
    /// the floor the polyline is on, inches. One per polyline (a roof at
    /// another height gets a polyline of its own, as Chief makes one per
    /// height).
    pub height: f64,
    /// One directive per polyline edge (`points[i] -> points[i + 1]`).
    pub edges: Vec<BaselineEdge>,
}

impl Default for RoofBaseline {
    fn default() -> Self {
        Self {
            height: 96.0,
            edges: Vec::new(),
        }
    }
}

impl RoofBaseline {
    /// A baseline of `n` edges that all carry `edge`.
    pub fn uniform(height: f64, n: usize, edge: BaselineEdge) -> Self {
        Self {
            height,
            edges: vec![edge; n],
        }
    }

    /// Brings the edge list to `n` entries after the polyline gained or lost
    /// corners: a new edge takes the directive of the one before it.
    pub fn fit(&mut self, n: usize) {
        if self.edges.len() > n {
            self.edges.truncate(n);
        }
        while self.edges.len() < n {
            let next = self.edges.last().cloned().unwrap_or_default();
            self.edges.push(next);
        }
    }

    /// The skeleton-builder directives, one per edge of an `n`-sided polyline.
    pub fn specs(&self, n: usize) -> Vec<EdgeRoofSpec> {
        (0..n)
            .map(|i| self.edges.get(i).cloned().unwrap_or_default().to_spec())
            .collect()
    }
}

/// The text shown along an edge: V, G, K or L plus the pitch, or `(vert)`
/// when no plane slopes toward the edge (manual p. 851). V is an edge against
/// a wall, G a gable or shed edge, K a knee wall, L a slope extended
/// downward; a plain hip edge shows its pitch alone.
pub fn directive_text(edge: &BaselineEdge) -> String {
    let vertical = !edge.rises();
    let mut out = String::new();
    if edge.against_wall {
        out.push('V');
    } else {
        match edge.option {
            BaselineOption::FullGable | BaselineOption::HighShed | BaselineOption::DutchGable => {
                out.push('G')
            }
            BaselineOption::Knee => out.push('K'),
            BaselineOption::Hip => {}
        }
    }
    if edge.extend_down.is_some() && !vertical {
        out.push('L');
    }
    if !out.is_empty() {
        out.push(' ');
    }
    if vertical {
        out.push_str("(vert)");
    } else {
        out.push_str(&crate::switches::pitch_text(edge.pitch));
    }
    out
}

/// Builds the roof of one Roof Baseline Polyline: the polygon `points` (the
/// polyline's corners, either winding), its [`RoofBaseline`] and the eave
/// elevation `elevation` (the baseline height plus the floor's elevation).
/// The planes come with their Dutch gable faces like
/// [`build_roof_with_faces`]; `source_edge` indexes the polyline's edges.
pub fn build_baseline_roof(
    points: &[Point],
    baseline: &RoofBaseline,
    elevation: f64,
) -> (Roof, Vec<Vec<V3>>) {
    let specs = baseline.specs(points.len());
    build_roof_with_faces(points, &specs, elevation)
}

/// `centerline` (a footprint along wall centerlines) moved out by
/// `half_thickness[i]` on edge `i`: the polygon along the outside of the wall.
/// The result is counter-clockwise; `centerline` may wind either way, in
/// which case `half_thickness` follows the counter-clockwise edge order
/// (use [`baseline_from_footprint`], which handles the winding).
pub fn outside_outline(centerline: &[Point], half_thickness: &[f64]) -> Vec<Point> {
    if centerline.len() < 3 {
        return centerline.to_vec();
    }
    geom::offset_edges(&geom::ccw(centerline), half_thickness)
}

/// The Roof Baseline Polyline Make Roof Baseline Polylines makes from a
/// roofed footprint: its outline along the outside of the walls and the
/// directives of the walls. `footprint` is the centerline polygon (either
/// winding), `specs` and `half_thickness` follow its edges. `None` for a
/// footprint with no area. The polygon is counter-clockwise, so the
/// directives are reordered to match when the input is clockwise.
pub fn baseline_from_footprint(
    footprint: &[Point],
    specs: &[EdgeRoofSpec],
    half_thickness: &[f64],
    height: f64,
) -> Option<(Vec<Point>, RoofBaseline)> {
    let n = footprint.len();
    if n < 3 || polygon_area(footprint).abs() < 1e-6 {
        return None;
    }
    let spec_at = |i: usize| specs.get(i).copied().unwrap_or_default();
    let half_at = |i: usize| half_thickness.get(i).copied().unwrap_or(0.0);
    // Edge k of the reversed ring is old edge (n - 2 - k) mod n.
    let order: Vec<usize> = if polygon_area(footprint) >= 0.0 {
        (0..n).collect()
    } else {
        (0..n).map(|k| (2 * n - 2 - k) % n).collect()
    };
    let halves: Vec<f64> = order.iter().map(|&i| half_at(i)).collect();
    let edges = order
        .iter()
        .zip(&halves)
        .map(|(&i, &h)| BaselineEdge::from_spec(&spec_at(i), h))
        .collect();
    let outline = outside_outline(footprint, &halves);
    Some((outline, RoofBaseline { height, edges }))
}

/// Outline length and enclosed area of a baseline polyline (the Polyline
/// panel): `(perimeter, area)` in inches and square inches.
pub fn perimeter_and_area(points: &[Point]) -> (f64, f64) {
    let n = points.len();
    let perimeter = (0..n).map(|i| points[i].dist(points[(i + 1) % n])).sum();
    (perimeter, polygon_area(points).abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeKind, RoofPlane};

    fn pts(v: &[(f64, f64)]) -> Vec<Point> {
        v.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

    fn hip(pitch: f64, overhang: f64) -> BaselineEdge {
        BaselineEdge {
            pitch,
            overhang,
            ..BaselineEdge::default()
        }
    }

    fn area(roof: &Roof) -> f64 {
        roof.planes.iter().map(RoofPlane::projected_area).sum()
    }

    #[test]
    fn the_edge_options_map_to_the_wall_directives() {
        let g = BaselineEdge {
            option: BaselineOption::FullGable,
            ..hip(6.0, 12.0)
        };
        assert_eq!(g.to_spec().to_edge_roof().kind, EdgeKind::Gable);
        let h = BaselineEdge {
            option: BaselineOption::HighShed,
            ..hip(6.0, 12.0)
        };
        let e = h.to_spec().to_edge_roof();
        assert_eq!((e.kind, e.overhang), (EdgeKind::Shed, 0.0));
        // Against Wall is the same as a high shed side.
        let w = BaselineEdge {
            against_wall: true,
            ..hip(6.0, 12.0)
        };
        assert_eq!(w.to_spec().to_edge_roof().kind, EdgeKind::Shed);
        assert!(!w.rises());
        let d = BaselineEdge {
            option: BaselineOption::DutchGable,
            break_rise: Some(30.0),
            ..hip(8.0, 12.0)
        };
        assert!(d.to_spec().dutch_gable && d.to_spec().break_rise == Some(30.0));
        let x = BaselineEdge {
            extend_down: Some(12.0),
            ..hip(8.0, 12.0)
        };
        assert_eq!(x.to_spec().extend_slope_downward, Some(12.0));
        // A Full Gable's second pitch is the half hip clipping its peak.
        let hh = BaselineEdge {
            option: BaselineOption::FullGable,
            upper_pitch: Some(12.0),
            break_rise: Some(40.0),
            ..hip(8.0, 12.0)
        };
        assert_eq!(hh.to_spec().half_hip_pitch, Some(12.0));
        assert_eq!(hh.to_spec().half_hip_rise, Some(40.0));
    }

    #[test]
    fn directive_letters_follow_the_manual() {
        assert_eq!(directive_text(&hip(8.0, 16.0)), "8:12");
        let gable = BaselineEdge {
            option: BaselineOption::FullGable,
            ..hip(8.0, 0.0)
        };
        assert_eq!(directive_text(&gable), "G (vert)");
        let wall = BaselineEdge {
            against_wall: true,
            ..hip(8.0, 0.0)
        };
        assert_eq!(directive_text(&wall), "V (vert)");
        let knee = BaselineEdge {
            option: BaselineOption::Knee,
            ..hip(6.0, 0.0)
        };
        assert_eq!(directive_text(&knee), "K 6:12");
        let lower = BaselineEdge {
            extend_down: Some(12.0),
            ..hip(6.0, 0.0)
        };
        assert_eq!(directive_text(&lower), "L 6:12");
    }

    #[test]
    fn a_baseline_roof_matches_the_roof_of_the_same_footprint() {
        // A 40 x 24 ft house with 6" walls, 16" overhang from the wall face.
        let centre = pts(&[(0.0, 0.0), (480.0, 0.0), (480.0, 288.0), (0.0, 288.0)]);
        let spec = EdgeRoofSpec {
            overhang: 16.0 + 3.0,
            ..EdgeRoofSpec::default()
        };
        let specs = vec![spec; 4];
        let (outline, base) = baseline_from_footprint(&centre, &specs, &[3.0; 4], 100.0).unwrap();
        // The polyline lies on the outside face of the walls.
        let (lo, hi) = (outline[0], outline[2]);
        assert_eq!((lo.x, lo.y, hi.x, hi.y), (-3.0, -3.0, 483.0, 291.0));
        assert!(base.edges.iter().all(|e| (e.overhang - 16.0).abs() < 1e-9));
        let (from_base, _) = build_baseline_roof(&outline, &base, 100.0);
        let direct = crate::build_roof_with_specs(&centre, &specs, 100.0);
        assert_eq!(from_base.planes.len(), direct.planes.len());
        assert!((area(&from_base) - area(&direct)).abs() < 1e-3);
        let (a, b) = (from_base.bounds().unwrap(), direct.bounds().unwrap());
        for k in 0..3 {
            assert!((a.0[k] - b.0[k]).abs() < 1e-6 && (a.1[k] - b.1[k]).abs() < 1e-6);
        }
    }

    #[test]
    fn a_clockwise_footprint_keeps_each_directive_on_its_own_wall() {
        // Clockwise rectangle; the edge from (0,0) to (0,288) is a gable.
        let centre = pts(&[(0.0, 0.0), (0.0, 288.0), (480.0, 288.0), (480.0, 0.0)]);
        let mut specs = vec![EdgeRoofSpec::default(); 4];
        specs[0].full_gable_wall = true;
        specs[2].full_gable_wall = true;
        let (outline, base) = baseline_from_footprint(&centre, &specs, &[0.0; 4], 96.0).unwrap();
        assert!(polygon_area(&outline) > 0.0);
        // Counter-clockwise, the two gables are the short sides at x = 0, 480.
        for (i, e) in base.edges.iter().enumerate() {
            let (a, b) = (outline[i], outline[(i + 1) % 4]);
            let vertical_side = (a.x - b.x).abs() < 1e-6;
            assert_eq!(
                e.option == BaselineOption::FullGable,
                vertical_side,
                "edge {i}"
            );
        }
    }

    #[test]
    fn an_l_shaped_addition_butts_the_existing_gable() {
        // The existing house: 40 x 24 ft, a gable roof along x (gables at the
        // short sides), eaves at 100".
        let house = pts(&[(0.0, 0.0), (480.0, 0.0), (480.0, 288.0), (0.0, 288.0)]);
        let mut hs = vec![EdgeRoofSpec::default(); 4];
        hs[1].full_gable_wall = true;
        hs[3].full_gable_wall = true;
        let existing = crate::build_roof_with_specs(&house, &hs, 100.0);
        assert_eq!(existing.planes.len(), 2);
        let ridge = existing.bounds().unwrap().1[1];

        // The addition: an L along the south and east of the house's west
        // half. Its edge along the house wall (y = 0, x 0..240) is Against
        // Wall. Eaves at 88", 6:12, 12" overhang on the five free sides.
        let l = pts(&[
            (0.0, -144.0),
            (360.0, -144.0),
            (360.0, -48.0),
            (240.0, -48.0),
            (240.0, 0.0),
            (0.0, 0.0),
        ]);
        let mut edges = vec![hip(6.0, 12.0); 6];
        edges[4] = BaselineEdge {
            against_wall: true,
            ..hip(6.0, 12.0)
        };
        let base = RoofBaseline {
            height: 88.0,
            edges,
        };
        let (roof, faces) = build_baseline_roof(&l, &base, 88.0);
        assert!(!roof.approximate);
        assert!(faces.is_empty());
        // One plane per rising edge; the wall edge makes none and the
        // addition keeps below the house's ridge.
        assert_eq!(roof.planes.len(), 5);
        assert!(roof.planes.iter().all(|p| p.source_edge != 4));
        assert!(roof.bounds().unwrap().1[1] < ridge);
        // The roof is the L with 12" overhang on its five free sides.
        let want: f64 = {
            // Offset polygon area by hand: the free sides move out 12".
            let big = pts(&[
                (-12.0, -156.0),
                (372.0, -156.0),
                (372.0, -36.0),
                (252.0, -36.0),
                (252.0, 0.0),
                (-12.0, 0.0),
            ]);
            polygon_area(&big)
        };
        assert!(
            (area(&roof) - want).abs() < 1e-3,
            "{} vs {want}",
            area(&roof)
        );
        // Rebuilding the existing roof is not affected by the addition.
        let again = crate::build_roof_with_specs(&house, &hs, 100.0);
        assert_eq!(again, existing);
    }

    #[test]
    fn fitting_edges_after_the_polyline_gains_a_corner() {
        let mut b = RoofBaseline::uniform(96.0, 4, hip(8.0, 16.0));
        b.edges[3].against_wall = true;
        b.fit(5);
        assert_eq!(b.edges.len(), 5);
        assert!(b.edges[4].against_wall, "a new edge copies the one before");
        b.fit(3);
        assert_eq!(b.edges.len(), 3);
        assert!(!b.edges[2].against_wall);
    }

    #[test]
    fn the_polyline_panel_reports_perimeter_and_area() {
        let (p, a) = perimeter_and_area(&pts(&[
            (0.0, 0.0),
            (120.0, 0.0),
            (120.0, 60.0),
            (0.0, 60.0),
        ]));
        assert_eq!((p, a), (360.0, 7200.0));
    }
}
