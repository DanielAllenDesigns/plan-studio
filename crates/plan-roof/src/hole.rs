//! Roof holes and skylights (Chief "Roof Hole" and "Skylight", RF-42/RF-43).
//!
//! A hole is a plan-space polygon; [`roof_plane_with_holes`] drops it onto a
//! roof plane and returns the plane as an outer polygon with hole rings, plus
//! the curb/glass/frame geometry of every skylight. Triangulating the result
//! (polygon with holes) is the renderer's job: `plan-3d` does it.

use crate::geom::{self, V3};
use crate::RoofPlane;
use plan_core::geometry::polygon_area;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// What a [`RoofHole`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HoleKind {
    /// A glazed skylight on a curb.
    Skylight,
    /// A plain opening (chimney, roof access, dormer footprint).
    #[default]
    Hole,
}

/// Skylight construction, inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SkylightSpec {
    /// Curb height above the roof surface, measured along the plane normal.
    pub curb_height: f64,
    /// Glass (and frame) thickness; clamped to `curb_height`.
    pub glass_thickness: f64,
    /// Width of the frame ring around the glass, measured in the plane.
    pub frame_width: f64,
}

impl Default for SkylightSpec {
    /// 6" curb, 1" glass, 2" frame.
    fn default() -> Self {
        Self {
            curb_height: 6.0,
            glass_thickness: 1.0,
            frame_width: 2.0,
        }
    }
}

/// A hole cut through roof planes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoofHole {
    /// Plan polygon (inches, any winding, no self-intersection).
    pub outline: Vec<Point>,
    pub kind: HoleKind,
    /// Used when `kind` is [`HoleKind::Skylight`]; `None` means the default.
    pub skylight: Option<SkylightSpec>,
}

impl RoofHole {
    /// A plain hole.
    pub fn hole(outline: Vec<Point>) -> Self {
        Self {
            outline,
            kind: HoleKind::Hole,
            skylight: None,
        }
    }

    /// A skylight with the given construction.
    pub fn skylight(outline: Vec<Point>, spec: SkylightSpec) -> Self {
        Self {
            outline,
            kind: HoleKind::Skylight,
            skylight: Some(spec),
        }
    }

    /// Axis-aligned rectangular outline from two plan corners.
    pub fn rect(a: Point, b: Point) -> Vec<Point> {
        let (lo, hi) = (
            Point::new(a.x.min(b.x), a.y.min(b.y)),
            Point::new(a.x.max(b.x), a.y.max(b.y)),
        );
        vec![lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)]
    }
}

/// Skylight geometry on a roof plane (roof-space vertices).
#[derive(Debug, Clone, PartialEq)]
pub struct Skylight {
    /// Outer curb ring on the roof surface (the hole outline lifted onto the
    /// plane), counter-clockwise seen from above.
    pub ring: Vec<V3>,
    /// Glass edge: `ring` inset by the frame width, on the roof surface.
    /// Empty when the frame is wider than the opening allows.
    pub inner: Vec<V3>,
    /// Unit plane normal (up).
    pub normal: V3,
    pub spec: SkylightSpec,
}

/// A roof plane with holes: what a renderer needs to mesh it.
#[derive(Debug, Clone, PartialEq)]
pub struct RoofPolygonWithHoles {
    /// The plane outline, counter-clockwise seen from above.
    pub outer: Vec<V3>,
    /// Hole rings lifted onto the plane, counter-clockwise seen from above.
    pub holes: Vec<Vec<V3>>,
    /// One entry per hole of kind [`HoleKind::Skylight`].
    pub skylights: Vec<Skylight>,
    /// Unit plane normal (up).
    pub normal: V3,
    pub pitch_in_12: f64,
    pub source_edge: usize,
    /// Indices (into the `holes` argument) of holes that were not applied:
    /// not fully inside the plane, overlapping an earlier hole, or degenerate.
    pub skipped_holes: Vec<usize>,
}

impl RoofPolygonWithHoles {
    /// True sloped area of the plane minus its holes, square inches.
    pub fn area(&self) -> f64 {
        let a = |poly: &[V3]| {
            let s = geom::newell(poly);
            geom::dot3(s, s).sqrt() * 0.5
        };
        a(&self.outer) - self.holes.iter().map(|h| a(h)).sum::<f64>()
    }
}

/// Tolerance for "inside the plane", inches.
const INSIDE_TOL: f64 = 1e-6;

/// Cut `holes` into `plane`.
///
/// A hole is applied when it lies completely inside the plane's plan outline
/// (not touching its boundary) and does not overlap an earlier hole. Holes that
/// straddle the plane boundary or a ridge/hip are reported in
/// [`RoofPolygonWithHoles::skipped_holes`] instead of being clipped; build a
/// hole per plane for features that span several planes.
pub fn roof_plane_with_holes(plane: &RoofPlane, holes: &[RoofHole]) -> RoofPolygonWithHoles {
    let normal = geom::unit3(geom::newell(&plane.polygon3d)).unwrap_or([0.0, 1.0, 0.0]);
    let mut out = RoofPolygonWithHoles {
        outer: plane.polygon3d.clone(),
        holes: Vec::new(),
        skylights: Vec::new(),
        normal,
        pitch_in_12: plane.pitch_in_12,
        source_edge: plane.source_edge,
        skipped_holes: Vec::new(),
    };
    let outline = geom::ccw(&plane.plan_polygon());
    let mut accepted: Vec<Vec<Point>> = Vec::new();
    for (i, hole) in holes.iter().enumerate() {
        let ring = geom::ccw(&hole.outline);
        let ok = ring.len() >= 3
            && polygon_area(&ring).abs() > 1e-6
            && ring
                .iter()
                .all(|&p| geom::strictly_inside(p, &outline, INSIDE_TOL))
            && accepted.iter().all(|a| !geom::polygons_overlap(a, &ring));
        let lifted: Option<Vec<V3>> = ok
            .then(|| {
                ring.iter()
                    .map(|&p| plane.height_at(p).map(|y| geom::lift(p, y)))
                    .collect()
            })
            .flatten();
        let Some(lifted) = lifted else {
            out.skipped_holes.push(i);
            continue;
        };
        if hole.kind == HoleKind::Skylight {
            let spec = hole.skylight.unwrap_or_default();
            out.skylights
                .push(skylight_on(plane, &lifted, normal, spec));
        }
        accepted.push(ring);
        out.holes.push(lifted);
    }
    out
}

/// Build the skylight record: the curb ring and the frame-inset glass ring.
fn skylight_on(plane: &RoofPlane, ring: &[V3], normal: V3, spec: SkylightSpec) -> Skylight {
    let spec = SkylightSpec {
        curb_height: spec.curb_height.max(0.0),
        glass_thickness: spec.glass_thickness.clamp(0.0, spec.curb_height.max(0.0)),
        frame_width: spec.frame_width.max(0.0),
    };
    // In-plane basis: e1 along the eave, e2 = normal x e1 (right-handed).
    let p = &plane.polygon3d;
    let e1 = geom::unit3(geom::sub3(p[1], p[0])).unwrap_or([1.0, 0.0, 0.0]);
    let e2 = geom::unit3(geom::cross3(normal, e1)).unwrap_or([0.0, 0.0, -1.0]);
    let origin = ring[0];
    let local: Vec<Point> = ring
        .iter()
        .map(|&q| {
            let d = geom::sub3(q, origin);
            Point::new(geom::dot3(d, e1), geom::dot3(d, e2))
        })
        .collect();
    let inner = if spec.frame_width > 0.0 {
        geom::offset_polygon(&geom::ccw(&local), spec.frame_width)
            .map(|v| {
                v.iter()
                    .map(|q| {
                        geom::add3(
                            origin,
                            geom::add3(geom::scale3(e1, q.x), geom::scale3(e2, q.y)),
                        )
                    })
                    .collect::<Vec<V3>>()
            })
            .unwrap_or_default()
    } else {
        ring.to_vec()
    };
    Skylight {
        ring: ring.to_vec(),
        inner,
        normal,
        spec,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{build_roof, EdgeKind, EdgeRoof};

    /// 40 x 30 ft gable roof (ridge along x), no overhang.
    pub(crate) fn gable_roof_planes() -> Vec<RoofPlane> {
        let fp = vec![
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 360.0),
            Point::new(0.0, 360.0),
        ];
        let mut edges = vec![
            EdgeRoof {
                overhang: 0.0,
                ..EdgeRoof::default()
            };
            4
        ];
        edges[1].kind = EdgeKind::Gable;
        edges[3].kind = EdgeKind::Gable;
        build_roof(&fp, &edges, 108.0).planes
    }

    #[test]
    fn height_at_matches_the_plane() {
        let planes = gable_roof_planes();
        let south = planes.iter().find(|p| p.source_edge == 0).unwrap();
        // 8:12 over 180" run: ridge 120" above the eave.
        assert!((south.height_at(Point::new(240.0, 0.0)).unwrap() - 108.0).abs() < 1e-6);
        assert!((south.height_at(Point::new(240.0, 180.0)).unwrap() - 228.0).abs() < 1e-6);
    }

    #[test]
    fn hole_inside_the_plane_is_applied_and_skylight_is_recorded() {
        let planes = gable_roof_planes();
        let south = planes.iter().find(|p| p.source_edge == 0).unwrap();
        let rect = RoofHole::rect(Point::new(200.0, 60.0), Point::new(260.0, 120.0));
        let holes = [
            RoofHole::skylight(rect, SkylightSpec::default()),
            // Straddles the ridge: not inside this plane.
            RoofHole::hole(RoofHole::rect(
                Point::new(10.0, 170.0),
                Point::new(40.0, 190.0),
            )),
        ];
        let r = roof_plane_with_holes(south, &holes);
        assert_eq!(r.holes.len(), 1);
        assert_eq!(r.skipped_holes, vec![1]);
        assert_eq!(r.skylights.len(), 1);
        let sky = &r.skylights[0];
        assert_eq!(sky.ring.len(), 4);
        assert_eq!(sky.inner.len(), 4);
        // Hole ring vertices lie on the plane.
        for v in &r.holes[0] {
            let y = south.height_at(geom::to_plan(*v)).unwrap();
            assert!((y - v[1]).abs() < 1e-6);
        }
        // Net area is the plane minus the hole (60 x 60 in plan, sloped).
        let slope = (1.0f64 + (8.0f64 / 12.0).powi(2)).sqrt();
        assert!((r.area() - (south.area() - 3600.0 * slope)).abs() < 1e-3);
    }

    #[test]
    fn overlapping_and_degenerate_holes_are_skipped() {
        let planes = gable_roof_planes();
        let south = planes.iter().find(|p| p.source_edge == 0).unwrap();
        let a = RoofHole::rect(Point::new(100.0, 40.0), Point::new(160.0, 100.0));
        let b = RoofHole::rect(Point::new(140.0, 80.0), Point::new(200.0, 120.0));
        let line = vec![
            Point::new(300.0, 50.0),
            Point::new(310.0, 50.0),
            Point::new(320.0, 50.0),
        ];
        let r = roof_plane_with_holes(
            south,
            &[RoofHole::hole(a), RoofHole::hole(b), RoofHole::hole(line)],
        );
        assert_eq!(r.holes.len(), 1);
        assert_eq!(r.skipped_holes, vec![1, 2]);
    }
}
