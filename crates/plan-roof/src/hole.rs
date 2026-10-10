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

/// How far a hole piece is drawn back from the plane boundary it shares with
/// the neighbouring plane, inches. Holes must lie inside the plane they cut.
const PIECE_SETBACK: f64 = 0.02;

/// A hole that crosses several planes, cut into one piece per plane.
///
/// `outline` (a roofless room, for instance) is clipped to each plane's plan
/// outline; a plane that the outline only touches gets nothing. Each piece is
/// pulled `0.02` inches inside the plane it belongs to where it meets the
/// plane's boundary (a ridge or hip shared with the next piece), so
/// [`roof_plane_with_holes`] accepts it: together the pieces leave the roof
/// open over the whole outline, with a sliver no wider than the setback at
/// the joints.
///
/// Returns `(index into planes, piece outline)` for every plane the outline
/// reaches. A concave plane is clipped triangle by triangle; planes with fewer
/// than three corners are skipped.
pub fn hole_pieces(planes: &[RoofPlane], outline: &[Point]) -> Vec<(usize, Vec<Point>)> {
    let ring = geom::ccw(outline);
    let mut out = Vec::new();
    if ring.len() < 3 {
        return out;
    }
    for (k, plane) in planes.iter().enumerate() {
        let poly = geom::ccw(&plane.plan_polygon());
        if poly.len() < 3 || plane.normal()[1] < 1e-6 {
            continue;
        }
        let clips: Vec<Vec<Point>> = if geom::is_convex(&poly) {
            vec![poly.clone()]
        } else {
            geom::ear_triangles(&poly)
                .into_iter()
                .map(|t| t.to_vec())
                .collect()
        };
        for clip in clips {
            let piece = geom::clip_convex(&ring, &clip);
            if piece.len() < 3 || polygon_area(&piece).abs() < 1.0 {
                continue;
            }
            let centroid = plan_core::geometry::polygon_centroid(&piece);
            let moved: Vec<Point> = piece
                .iter()
                .map(|&p| {
                    if geom::boundary_dist(p, &poly) < PIECE_SETBACK * 2.0 {
                        let toward = centroid.sub(p);
                        if toward.length() > 1e-9 {
                            return p.add(toward.normalized().scale(PIECE_SETBACK * 2.0));
                        }
                    }
                    p
                })
                .collect();
            let ok = moved
                .iter()
                .all(|&p| geom::strictly_inside(p, &poly, INSIDE_TOL));
            if ok {
                out.push((k, moved));
            }
        }
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

// ===================================================================
// Skylight shapes, inside hole rim and ceiling hole (RF-43, RF-87)
// ===================================================================

/// The shape of a skylight's opening (Skylight Specification, General
/// panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SkylightShape {
    #[default]
    Rectangle,
    /// A circle of the width as diameter.
    Circle,
    Ellipse,
    /// A stadium: two straight sides and half-circle ends.
    Oval,
    /// The outline was edited by hand (Edit Skylight Shape): it is whatever
    /// the polyline says.
    Custom,
}

impl SkylightShape {
    pub const ALL: [SkylightShape; 5] = [
        SkylightShape::Rectangle,
        SkylightShape::Circle,
        SkylightShape::Ellipse,
        SkylightShape::Oval,
        SkylightShape::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SkylightShape::Rectangle => "Rectangle",
            SkylightShape::Circle => "Circle",
            SkylightShape::Ellipse => "Ellipse",
            SkylightShape::Oval => "Oval",
            SkylightShape::Custom => "Custom",
        }
    }
}

/// Segments in a full circle of a round skylight outline.
pub const SKYLIGHT_FACETS: usize = 32;
/// Size of the default skylight, inches (a click without a drag, 2 by 2 ft).
pub const DEFAULT_SKYLIGHT_SIZE: f64 = 24.0;

/// The plan outline of a skylight of `shape` centred on `center`:
/// `width` across `across` (a unit plan vector, normally across the slope)
/// and `length` along its perpendicular, counter-clockwise. A circle is
/// `width` in diameter. [`SkylightShape::Custom`] has no outline of its own:
/// an empty list comes back.
pub fn shape_outline(
    shape: SkylightShape,
    center: Point,
    across: Point,
    width: f64,
    length: f64,
    facets: usize,
) -> Vec<Point> {
    let a = if across.length() < 1e-9 {
        Point::new(1.0, 0.0)
    } else {
        across.normalized()
    };
    let b = a.perp();
    let at = |x: f64, y: f64| center.add(a.scale(x)).add(b.scale(y));
    let (hw, hl) = (width.max(0.0) * 0.5, length.max(0.0) * 0.5);
    let n = facets.max(8);
    match shape {
        SkylightShape::Custom => Vec::new(),
        _ if hw < 1e-9 || hl < 1e-9 => Vec::new(),
        SkylightShape::Rectangle => vec![at(-hw, -hl), at(hw, -hl), at(hw, hl), at(-hw, hl)],
        SkylightShape::Circle => (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / n as f64;
                at(hw * t.cos(), hw * t.sin())
            })
            .collect(),
        SkylightShape::Ellipse => (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / n as f64;
                at(hw * t.cos(), hl * t.sin())
            })
            .collect(),
        SkylightShape::Oval => {
            // Straight sides along b between two half circles of radius hw; an
            // oval no longer than it is wide is a circle.
            if hl <= hw {
                return shape_outline(SkylightShape::Circle, center, a, width, width, facets);
            }
            let straight = hl - hw;
            let half = n / 2;
            let mut out = Vec::with_capacity(2 * half + 2);
            for i in 0..=half {
                let t = std::f64::consts::PI * i as f64 / half as f64;
                out.push(at(hw * t.cos(), straight + hw * t.sin()));
            }
            for i in 0..=half {
                let t = std::f64::consts::PI * (1.0 + i as f64 / half as f64);
                out.push(at(hw * t.cos(), -straight + hw * t.sin()));
            }
            out
        }
    }
}

/// How the walls inside the opening, from the roof down to the ceiling, are
/// cut (Skylight Specification, Inside Hole Rim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HoleRim {
    /// Square to the roof surface: the walls along the slope are vertical,
    /// the head and sill lean with the roof.
    #[default]
    Square,
    /// Every wall vertical.
    Plumb,
    /// The sill (low side) is plumb, the head (high side) is square to the
    /// roof.
    PlumbSquare,
}

impl HoleRim {
    pub const ALL: [HoleRim; 3] = [HoleRim::Square, HoleRim::Plumb, HoleRim::PlumbSquare];

    pub fn label(self) -> &'static str {
        match self {
            HoleRim::Square => "Square",
            HoleRim::Plumb => "Plumb",
            HoleRim::PlumbSquare => "Plumb/Square",
        }
    }
}

/// What is done to the ceiling under a skylight (Ceiling Hole).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CeilingHole {
    /// A hole where the rim meets the ceiling.
    #[default]
    Automatic,
    /// A hole of a polyline the user drew.
    Manual,
    /// Do Not Cut: the ceiling stays whole.
    DoNotCut,
}

impl CeilingHole {
    pub const ALL: [CeilingHole; 3] = [
        CeilingHole::Automatic,
        CeilingHole::Manual,
        CeilingHole::DoNotCut,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CeilingHole::Automatic => "Automatically Generate",
            CeilingHole::Manual => "Use Manual Polyline",
            CeilingHole::DoNotCut => "Do Not Cut",
        }
    }
}

/// Skylight settings that go with [`SkylightSpec`]: shape, size, rim and
/// ceiling hole.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkylightOptions {
    pub shape: SkylightShape,
    /// Size of the opening, inches: across the slope and along it.
    pub width: f64,
    pub length: f64,
    pub rim: HoleRim,
    pub ceiling_hole: CeilingHole,
    /// Draw the glass and frame in plan view (Display in Plan View).
    pub display_in_plan: bool,
}

impl Default for SkylightOptions {
    fn default() -> Self {
        Self {
            shape: SkylightShape::Rectangle,
            width: DEFAULT_SKYLIGHT_SIZE,
            length: DEFAULT_SKYLIGHT_SIZE,
            rim: HoleRim::Square,
            ceiling_hole: CeilingHole::Automatic,
            display_in_plan: true,
        }
    }
}

/// The wall of the opening between the roof and the ceiling, as one quad per
/// outline edge: the two corners on the roof surface, then the two below.
pub type RimWall = [V3; 4];

/// The inside walls of an opening `outline` through `plane`, down to the
/// level `bottom_y` (the ceiling), cut as `rim` says. The first two corners
/// of each wall are on the roof surface. Empty when the outline does not lie
/// over the plane or the plane is vertical.
pub fn rim_walls(
    plane: &RoofPlane,
    outline: &[Point],
    rim: HoleRim,
    bottom_y: f64,
) -> Vec<RimWall> {
    let ring = geom::ccw(outline);
    if ring.len() < 3 {
        return Vec::new();
    }
    let Some(top) = ring
        .iter()
        .map(|&p| plane.height_at(p).map(|y| (p, y)))
        .collect::<Option<Vec<_>>>()
    else {
        return Vec::new();
    };
    let bottoms = rim_bottoms(plane, &ring, &top, rim, bottom_y);
    let n = ring.len();
    (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            [
                geom::lift(top[i].0, top[i].1),
                geom::lift(top[j].0, top[j].1),
                geom::lift(bottoms[j], bottom_y),
                geom::lift(bottoms[i], bottom_y),
            ]
        })
        .collect()
}

/// Plan positions where each rim corner meets the level `bottom_y`.
fn rim_bottoms(
    plane: &RoofPlane,
    ring: &[Point],
    top: &[(Point, f64)],
    rim: HoleRim,
    bottom_y: f64,
) -> Vec<Point> {
    // The roof rises toward `up`; a square wall leans back up the slope: it
    // runs along minus the normal, which has the horizontal part `-up`.
    let n = plane.normal();
    let flat = (n[0] * n[0] + n[2] * n[2]).sqrt();
    let up = if flat < 1e-9 {
        Point::ZERO
    } else {
        // The normal's horizontal part points downhill: roof space has
        // z = -plan y.
        Point::new(-n[0] / flat, n[2] / flat)
    };
    let centre = plan_core::geometry::polygon_centroid(ring);
    ring.iter()
        .zip(top)
        .map(|(&p, &(_, y))| {
            let square = match rim {
                HoleRim::Square => true,
                HoleRim::Plumb => false,
                HoleRim::PlumbSquare => p.sub(centre).dot(up) > 1e-6,
            };
            if !square || flat < 1e-9 || n[1] < 1e-9 {
                return p;
            }
            // Down the wall by `drop` inches along the inward normal moves
            // `drop * flat / n_y` sideways, up the slope.
            let drop = (y - bottom_y).max(0.0);
            p.add(up.scale(drop * flat / n[1]))
        })
        .collect()
}

/// The outline of the hole in the ceiling under an opening, or `None` when
/// the ceiling is not cut. Automatic follows the rim down to `ceiling_y`;
/// Manual takes `manual` (when the polyline has at least three corners, else
/// it falls back to Automatic).
pub fn ceiling_hole_outline(
    plane: &RoofPlane,
    outline: &[Point],
    rim: HoleRim,
    mode: CeilingHole,
    ceiling_y: f64,
    manual: Option<&[Point]>,
) -> Option<Vec<Point>> {
    match mode {
        CeilingHole::DoNotCut => None,
        CeilingHole::Manual if manual.is_some_and(|m| m.len() >= 3) => {
            manual.map(<[Point]>::to_vec)
        }
        _ => {
            let ring = geom::ccw(outline);
            let top: Vec<(Point, f64)> = ring
                .iter()
                .map(|&p| plane.height_at(p).map(|y| (p, y)))
                .collect::<Option<_>>()?;
            (ring.len() >= 3).then(|| rim_bottoms(plane, &ring, &top, rim, ceiling_y))
        }
    }
}

/// Edit Skylight Shape: the outline with corner `index` moved to `to`. The
/// shape becomes [`SkylightShape::Custom`]. False when `index` is out of
/// range or the new outline would cross itself.
pub fn move_shape_corner(
    outline: &mut [Point],
    shape: &mut SkylightShape,
    index: usize,
    to: Point,
) -> bool {
    if index >= outline.len() {
        return false;
    }
    let old = outline[index];
    outline[index] = to;
    let n = outline.len();
    for i in 0..n {
        for j in (i + 1)..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (a, b, c, d) = (
                outline[i],
                outline[(i + 1) % n],
                outline[j],
                outline[(j + 1) % n],
            );
            let side = |o: Point, u: Point, v: Point| u.sub(o).cross(v.sub(o));
            if side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0 {
                outline[index] = old;
                return false;
            }
        }
    }
    *shape = SkylightShape::Custom;
    true
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
    fn a_hole_over_the_ridge_is_cut_into_one_piece_per_plane() {
        let planes = gable_roof_planes();
        // 80 x 120 inches straddling the ridge (y = 180) between the gables.
        let room = RoofHole::rect(Point::new(200.0, 120.0), Point::new(280.0, 240.0));
        // The plain hole straddles two planes, so neither takes it.
        for p in &planes {
            let r = roof_plane_with_holes(p, &[RoofHole::hole(room.clone())]);
            assert_eq!(r.skipped_holes, vec![0]);
        }
        let pieces = hole_pieces(&planes, &room);
        assert_eq!(pieces.len(), 2);
        let mut area = 0.0;
        for (k, piece) in &pieces {
            let r = roof_plane_with_holes(&planes[*k], &[RoofHole::hole(piece.clone())]);
            assert!(r.skipped_holes.is_empty(), "plane {k} refused its piece");
            assert_eq!(r.holes.len(), 1);
            area += polygon_area(piece).abs();
        }
        // The pieces cover the outline but for the sliver at the ridge.
        let want = 80.0 * 120.0;
        assert!(area <= want + 1e-6 && area > want - 80.0 * 0.2, "{area}");
    }

    #[test]
    fn a_hole_inside_one_plane_stays_one_piece_and_a_far_hole_none() {
        let planes = gable_roof_planes();
        let inside = RoofHole::rect(Point::new(200.0, 40.0), Point::new(260.0, 100.0));
        let pieces = hole_pieces(&planes, &inside);
        assert_eq!(pieces.len(), 1);
        assert!((polygon_area(&pieces[0].1).abs() - 3600.0).abs() < 1e-6);
        let away = RoofHole::rect(Point::new(2000.0, 0.0), Point::new(2050.0, 40.0));
        assert!(hole_pieces(&planes, &away).is_empty());
        assert!(hole_pieces(&planes, &away[..2]).is_empty());
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

    #[test]
    fn skylight_shapes_have_the_areas_of_their_shapes() {
        let c = Point::new(100.0, 100.0);
        let across = Point::new(1.0, 0.0);
        let area = |s, w, l| polygon_area(&shape_outline(s, c, across, w, l, 128)).abs();
        assert!((area(SkylightShape::Rectangle, 24.0, 48.0) - 24.0 * 48.0).abs() < 1e-6);
        let circle = std::f64::consts::PI * 144.0;
        assert!((area(SkylightShape::Circle, 24.0, 99.0) - circle).abs() < 0.01 * circle);
        let ellipse = std::f64::consts::PI * 12.0 * 24.0;
        assert!((area(SkylightShape::Ellipse, 24.0, 48.0) - ellipse).abs() < 0.01 * ellipse);
        // A stadium: the rectangle between the half circles plus the circle.
        let oval = 24.0 * (48.0 - 24.0) + circle;
        assert!((area(SkylightShape::Oval, 24.0, 48.0) - oval).abs() < 0.01 * oval);
        // An oval no longer than wide is a circle; Custom has no outline.
        assert!((area(SkylightShape::Oval, 24.0, 20.0) - circle).abs() < 0.01 * circle);
        assert!(shape_outline(SkylightShape::Custom, c, across, 24.0, 24.0, 32).is_empty());
        assert!(shape_outline(SkylightShape::Rectangle, c, across, 0.0, 24.0, 32).is_empty());
    }

    #[test]
    fn a_shape_follows_the_across_direction_and_stays_counter_clockwise() {
        let c = Point::ZERO;
        for s in [
            SkylightShape::Rectangle,
            SkylightShape::Oval,
            SkylightShape::Ellipse,
        ] {
            let o = shape_outline(s, c, Point::new(0.0, 1.0), 20.0, 60.0, 32);
            assert!(polygon_area(&o) > 0.0, "{s:?}");
            // Width lies along the given direction (y), length along -x.
            let (lo, hi) = o
                .iter()
                .fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.x), m.1.max(p.x)));
            assert!(((hi - lo) - 60.0).abs() < 1e-6, "{s:?} length {}", hi - lo);
        }
    }

    fn south() -> RoofPlane {
        gable_roof_planes()
            .into_iter()
            .find(|p| p.source_edge == 0)
            .unwrap()
    }

    #[test]
    fn a_plumb_rim_drops_straight_and_a_square_rim_leans_up_the_slope() {
        let plane = south();
        let outline = RoofHole::rect(Point::new(200.0, 60.0), Point::new(260.0, 120.0));
        let floor_y = 96.0;
        let plumb = rim_walls(&plane, &outline, HoleRim::Plumb, floor_y);
        assert_eq!(plumb.len(), 4);
        for w in &plumb {
            assert!((w[0][0] - w[3][0]).abs() < 1e-9 && (w[0][2] - w[3][2]).abs() < 1e-9);
            assert!((w[3][1] - floor_y).abs() < 1e-9 && (w[2][1] - floor_y).abs() < 1e-9);
        }
        // Square: each corner drops (roof y - 96) and moves up the slope by
        // that drop times the pitch (the plane rises toward +y here).
        let square = rim_walls(&plane, &outline, HoleRim::Square, floor_y);
        for w in &square {
            let drop = w[0][1] - floor_y;
            let dy = -(w[3][2] - w[0][2]);
            assert!((dy - drop * 8.0 / 12.0).abs() < 1e-6, "{dy} vs {drop}");
            assert!((w[3][0] - w[0][0]).abs() < 1e-9, "no sideways lean");
        }
        // The wall along the slope stays in a vertical plane; head and sill lean.
        // Plumb/Square: the sill (low side) plumb, the head square.
        let mixed = rim_walls(&plane, &outline, HoleRim::PlumbSquare, floor_y);
        let sill = &mixed[0];
        let head = &mixed[2];
        assert!((sill[3][2] - sill[0][2]).abs() < 1e-9, "sill is plumb");
        assert!((head[3][2] - head[0][2]).abs() > 1.0, "head leans");
    }

    #[test]
    fn the_ceiling_hole_follows_the_rim_unless_it_is_manual_or_off() {
        let plane = south();
        let outline = RoofHole::rect(Point::new(200.0, 60.0), Point::new(260.0, 120.0));
        let auto = |rim| {
            ceiling_hole_outline(&plane, &outline, rim, CeilingHole::Automatic, 96.0, None).unwrap()
        };
        // Plumb: the hole is the opening itself.
        let plumb = auto(HoleRim::Plumb);
        assert!((polygon_area(&plumb).abs() - 3600.0).abs() < 1e-6);
        // Square: each corner moves up the slope by its drop times the pitch.
        // The head is 40 inches higher than the sill, so it moves 40 * 8/12
        // further: the hole is that much longer.
        let square = auto(HoleRim::Square);
        let longer = 60.0 + 40.0 * 8.0 / 12.0;
        assert!((polygon_area(&square).abs() - 60.0 * longer).abs() < 1e-6);
        let shift: Vec<f64> = (0..4)
            .map(|i| {
                let plan_y = geom::ccw(&outline)[i].y;
                let q = square[i];
                assert!((q.x - geom::ccw(&outline)[i].x).abs() < 1e-9);
                let roof = plane.height_at(Point::new(q.x, plan_y)).unwrap();
                (q.y - plan_y) - (roof - 96.0) * 8.0 / 12.0
            })
            .collect();
        assert!(shift.iter().all(|d| d.abs() < 1e-6), "{shift:?}");
        let manual = [
            Point::new(0.0, 0.0),
            Point::new(40.0, 0.0),
            Point::new(0.0, 40.0),
        ];
        let got = ceiling_hole_outline(
            &plane,
            &outline,
            HoleRim::Square,
            CeilingHole::Manual,
            96.0,
            Some(&manual),
        );
        assert_eq!(got.as_deref(), Some(&manual[..]));
        // A manual hole with no polyline falls back to the automatic one.
        let fallback = ceiling_hole_outline(
            &plane,
            &outline,
            HoleRim::Plumb,
            CeilingHole::Manual,
            96.0,
            None,
        );
        assert_eq!(fallback.unwrap().len(), 4);
        assert!(ceiling_hole_outline(
            &plane,
            &outline,
            HoleRim::Square,
            CeilingHole::DoNotCut,
            96.0,
            None
        )
        .is_none());
    }

    #[test]
    fn editing_the_shape_makes_it_custom_and_refuses_a_crossing_outline() {
        let mut outline = shape_outline(
            SkylightShape::Rectangle,
            Point::new(100.0, 100.0),
            Point::new(1.0, 0.0),
            24.0,
            24.0,
            32,
        );
        let mut shape = SkylightShape::Rectangle;
        assert!(move_shape_corner(
            &mut outline,
            &mut shape,
            2,
            Point::new(150.0, 150.0)
        ));
        assert_eq!(shape, SkylightShape::Custom);
        assert_eq!(outline[2], Point::new(150.0, 150.0));
        // Dragging a corner across the opposite side would cross the outline.
        let mut shape = SkylightShape::Rectangle;
        let before = outline.clone();
        assert!(!move_shape_corner(
            &mut outline,
            &mut shape,
            0,
            Point::new(200.0, 140.0)
        ));
        assert_eq!(outline, before);
        assert_eq!(shape, SkylightShape::Rectangle);
        assert!(!move_shape_corner(&mut outline, &mut shape, 9, Point::ZERO));
    }
}
