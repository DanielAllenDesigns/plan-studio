//! plan-roof: Chief-style automatic roofs ("Build Roof") from a footprint.
//!
//! Given a counter-clockwise building outline and per-edge roof settings
//! (pitch, hip/gable/shed, overhang), [`build_roof`] returns a set of planar
//! 3D roof planes. There is no GPU or rendering code here; the planes are plain
//! polygons a viewer, exporter or take-off can consume.
//!
//! Coordinates of the output follow the 3D convention of the rest of the
//! project: `X = plan x`, `Y = elevation`, `Z = -plan y`, so polygons that are
//! counter-clockwise in plan are counter-clockwise seen from above. All lengths
//! are inches.
//!
//! See the crate README for the algorithm (a weighted straight skeleton) and
//! its limits.

mod baseline;
mod ceiling;
mod curved;
mod dormer;
mod edges;
mod footprint;
mod gable;
mod geom;
mod group;
mod halfhip;
mod hole;
mod join;
mod retain;
mod skeleton;
mod spec;
mod staged;
mod switches;

pub use baseline::{
    baseline_from_footprint, build_baseline_roof, directive_text, outside_outline,
    perimeter_and_area, BaselineEdge, BaselineOption, RoofBaseline,
};
pub use ceiling::{
    cathedral_ceiling_planes, cathedral_height_at, ceiling_height_at,
    ceiling_planes_for_vaulted_room, room_ceiling_height_at, subtract_polygon,
    tray_ceiling_planes, CeilingPlane, Shelf,
};
pub use curved::{
    curve_height, curved_facets, plane_run, section_points, CurvedSpec, JoinLock,
    DEFAULT_FACET_ANGLE,
};
pub use dormer::{
    auto_dormer, cricket_behind, dormer_returns, dormer_room_ceiling, dormer_shaft,
    explode_dormer, gambrel_dormer, Cricket, Dormer, DormerKind, DormerRoom, DormerSpec,
    DormerWall, ExplodedDormer, SecondPitch, WindowOpening,
};
pub use edges::{classify_edges, EdgeRole, PlaneEdge};
pub use footprint::{flatten_curved_walls, footprint_from_walls};
pub use group::{assign_roof_groups, has_groups, GroupedRoom, RoofAssignment};
pub use gable::{
    apply_gable_line, apply_gable_lines, check_gable_line, gable_lines_over_openings, roof_return,
    roof_return_at, GableLine, GableLineProblem, GabledRoof, OpeningSpan, PlaneOrigin, ReturnKind,
    ReturnSpec, RoofReturn, WallFace, GABLE_LINE_REACH, MIN_GABLE_LINE, OPENING_GABLE_MARGIN,
    OPENING_GABLE_MERGE,
};
pub use halfhip::DEFAULT_CLIP_FRACTION;
pub use hole::{
    ceiling_hole_outline, hole_pieces, move_shape_corner, rim_walls, roof_plane_with_holes,
    shape_outline, CeilingHole, HoleKind, HoleRim, RimWall, RoofHole, RoofPolygonWithHoles,
    Skylight, SkylightOptions, SkylightShape, SkylightSpec, DEFAULT_SKYLIGHT_SIZE,
    SKYLIGHT_FACETS,
};
pub use join::join_planes;
pub use retain::{drop_replaced, replaces as retained_plane_replaces};
pub use spec::{
    build_roof_at_plate, build_roof_at_plate_with_faces, build_roof_with_faces,
    build_roof_with_specs, flat_roof_plane, flat_roof_plane_with_overhang, plate_baseline,
    EdgeRoofSpec,
};
pub use staged::DEFAULT_BREAK_FRACTION;
pub use switches::{
    clamp_segment_angle, degrees_to_pitch, pitch_display_degrees, pitch_text, pitch_to_degrees,
    set_pitch_display_degrees, BuildSwitches, DEFAULT_MIN_ALCOVE, DEFAULT_SEGMENT_ANGLE,
    PITCH_DEGREES_RANGE, SEGMENT_ANGLE_RANGE,
};

use plan_core::geometry::polygon_area;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Nominal fascia board height reported on every roof, inches (a 2x6 fascia).
pub const DEFAULT_FASCIA_HEIGHT: f64 = 6.0;

/// Smallest pitch accepted (rise in 12); flatter values are clamped to it.
const MIN_PITCH: f64 = 0.01;

/// How the roof treats one footprint edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EdgeKind {
    /// A sloping plane rises from this edge (Chief "Hip Wall").
    #[default]
    Hip,
    /// Vertical gable end (Chief "Full Gable Wall"): no plane is emitted and
    /// the neighbouring planes extend out to this edge.
    Gable,
    /// No roof rises from this edge (lean-to / shed wall); neighbours continue
    /// to it. Geometrically like a gable but intended as the high side.
    Shed,
}

/// Roof settings for one footprint edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeRoof {
    /// Rise per 12 of run, e.g. `8.0` for an 8:12 roof.
    pub pitch_in_12: f64,
    pub kind: EdgeKind,
    /// Horizontal overhang beyond the footprint edge, inches.
    pub overhang: f64,
}

impl Default for EdgeRoof {
    /// Hip, 8:12, 16" overhang (Chief's wall-dialog defaults).
    fn default() -> Self {
        Self {
            pitch_in_12: 8.0,
            kind: EdgeKind::Hip,
            overhang: 16.0,
        }
    }
}

/// One planar roof face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoofPlane {
    /// Vertices `[x, y, z]` with `X = plan x`, `Y = elevation`, `Z = -plan y`,
    /// counter-clockwise seen from above. The first edge (`[0] -> [1]`) is the
    /// eave on the baseline; the remaining vertices are hip/ridge/valley nodes.
    pub polygon3d: Vec<[f64; 3]>,
    pub pitch_in_12: f64,
    /// The overhung eave line this plane rises from (plan coordinates).
    pub baseline: (Point, Point),
    /// Index of the footprint edge this plane belongs to.
    pub source_edge: usize,
}

impl RoofPlane {
    /// Newell vector: twice the area, pointing along the face normal.
    fn newell(&self) -> [f64; 3] {
        let n = self.polygon3d.len();
        let mut s = [0.0; 3];
        for i in 0..n {
            let (c, d) = (self.polygon3d[i], self.polygon3d[(i + 1) % n]);
            s[0] += (c[1] - d[1]) * (c[2] + d[2]);
            s[1] += (c[2] - d[2]) * (c[0] + d[0]);
            s[2] += (c[0] - d[0]) * (c[1] + d[1]);
        }
        s
    }

    /// True (sloped) surface area, square inches.
    pub fn area(&self) -> f64 {
        let s = self.newell();
        (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() * 0.5
    }

    /// Area of the plane projected onto the plan, square inches. Positive for
    /// an upward-facing plane.
    pub fn projected_area(&self) -> f64 {
        self.newell()[1] * 0.5
    }

    /// Unit normal (pointing up for a proper roof plane); zero for a
    /// degenerate polygon.
    pub fn normal(&self) -> [f64; 3] {
        let s = self.newell();
        let len = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        if len <= f64::EPSILON {
            [0.0; 3]
        } else {
            [s[0] / len, s[1] / len, s[2] / len]
        }
    }

    /// Elevation of the plane above the plan point `p` (inches), or `None` for
    /// a vertical or degenerate plane.
    pub fn height_at(&self, p: Point) -> Option<f64> {
        let n = self.normal();
        if n[1] < 1e-9 {
            return None;
        }
        let o = *self.polygon3d.first()?;
        Some(o[1] - (n[0] * (p.x - o[0]) + n[2] * (-p.y - o[2])) / n[1])
    }

    /// The plane's outline in plan coordinates (counter-clockwise).
    pub fn plan_polygon(&self) -> Vec<Point> {
        self.polygon3d.iter().map(|&p| geom::to_plan(p)).collect()
    }

    /// Height of the plane's highest point above its lowest (the eave), inches.
    pub fn ridge_height(&self) -> f64 {
        let (lo, hi) = self
            .polygon3d
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                (lo.min(p[1]), hi.max(p[1]))
            });
        if hi >= lo {
            hi - lo
        } else {
            0.0
        }
    }
}

/// A generated roof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roof {
    pub planes: Vec<RoofPlane>,
    /// Fascia board height, inches ([`DEFAULT_FASCIA_HEIGHT`]).
    pub fascia_height: f64,
    /// Elevation of the eaves (top of wall), inches.
    pub baseline_elevation: f64,
    /// `true` when the exact weighted skeleton could not be computed. The
    /// planes are then the best available approximation: first the same
    /// footprint with one uniform pitch (the length-weighted mean of the hip
    /// edges; each plane's `pitch_in_12` reports the pitch actually used), and
    /// if even that fails a hip roof on the footprint's bounding box.
    pub approximate: bool,
}

impl Roof {
    /// Axis-aligned bounds `(min, max)` of all plane vertices, or `None` for
    /// an empty roof.
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let mut pts = self.planes.iter().flat_map(|p| p.polygon3d.iter());
        let first = *pts.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in pts {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        Some((lo, hi))
    }
}

/// A cleaned footprint: counter-clockwise, no zero-length edges, with the
/// original edge index and settings of every edge.
struct Prepared {
    poly: Vec<Point>,
    settings: Vec<EdgeRoof>,
    source: Vec<usize>,
}

fn prepare(footprint: &[Point], edges: &[EdgeRoof]) -> Option<Prepared> {
    let n = footprint.len();
    if n < 3 {
        return None;
    }
    let setting = |i: usize| edges.get(i).copied().unwrap_or_default();
    // Orientation: reverse a clockwise outline. Reversed edge k of the new list
    // is the old edge (n - 2 - k) mod n traversed backwards.
    let ccw = polygon_area(footprint) >= 0.0;
    let (pts, idx): (Vec<Point>, Vec<usize>) = if ccw {
        (footprint.to_vec(), (0..n).collect())
    } else {
        (
            footprint.iter().rev().copied().collect(),
            (0..n).map(|k| (2 * n - 2 - k) % n).collect(),
        )
    };
    // Drop duplicate consecutive vertices (zero-length edges).
    let mut poly = Vec::new();
    let mut source = Vec::new();
    for i in 0..n {
        if pts[i].dist(pts[(i + 1) % n]) > 1e-6 {
            poly.push(pts[i]);
            source.push(idx[i]);
        }
    }
    if poly.len() < 3 || polygon_area(&poly).abs() < 1e-6 {
        return None;
    }
    let settings = source.iter().map(|&i| setting(i)).collect();
    Some(Prepared {
        poly,
        settings,
        source,
    })
}

/// Offset every edge outward by its overhang and intersect neighbouring
/// offset lines. `None` if an edge collapses or flips.
fn overhang_polygon(poly: &[Point], over: &[f64]) -> Option<Vec<Point>> {
    let n = poly.len();
    let dir = |k: usize| poly[(k + 1) % n].sub(poly[k]).normalized();
    // Outward normal of a CCW edge is to its right.
    let out = |k: usize| dir(k).perp().scale(-1.0);
    let mut res = Vec::with_capacity(n);
    for k in 0..n {
        let a = (k + n - 1) % n;
        let (da, db) = (dir(a), dir(k));
        let pa = poly[a].add(out(a).scale(over[a]));
        let pb = poly[k].add(out(k).scale(over[k]));
        let denom = da.cross(db);
        let q = if denom.abs() < 1e-9 {
            poly[k].add(out(a).scale((over[a] + over[k]) * 0.5))
        } else {
            let t = pb.sub(pa).cross(db) / denom;
            pa.add(da.scale(t))
        };
        res.push(q);
    }
    for k in 0..n {
        let d = res[(k + 1) % n].sub(res[k]);
        if d.dot(dir(k)) <= 1e-9 {
            return None;
        }
    }
    Some(res)
}

/// Exact path: overhang, weighted skeleton, one plane per rising edge.
/// Returns `None` if the skeleton fails or its faces do not tile the outline.
fn build_exact(prep: &Prepared, baseline: f64) -> Option<Vec<RoofPlane>> {
    let n = prep.poly.len();
    let over: Vec<f64> = prep.settings.iter().map(|s| s.overhang.max(0.0)).collect();
    let outline = overhang_polygon(&prep.poly, &over)?;
    let speeds: Vec<f64> = prep
        .settings
        .iter()
        .map(|s| match s.kind {
            EdgeKind::Hip => 12.0 / s.pitch_in_12.max(MIN_PITCH),
            EdgeKind::Gable | EdgeKind::Shed => 0.0,
        })
        .collect();
    if speeds.iter().all(|&s| s == 0.0) {
        return Some(Vec::new());
    }
    let skel = skeleton::compute(&outline, &speeds)?;

    let mut planes = Vec::new();
    for i in 0..n {
        let s = &prep.settings[i];
        if s.kind != EdgeKind::Hip {
            continue;
        }
        let path = skel.face(i)?;
        let polygon3d = path
            .iter()
            .map(|&id| {
                let node = skel.nodes[id];
                [node.pos.x, baseline + node.t, -node.pos.y]
            })
            .collect();
        planes.push(RoofPlane {
            polygon3d,
            pitch_in_12: s.pitch_in_12,
            baseline: (outline[i], outline[(i + 1) % n]),
            source_edge: prep.source[i],
        });
    }

    // Faces of a valid skeleton tile the outline exactly.
    let want = polygon_area(&outline);
    let got: f64 = planes.iter().map(RoofPlane::projected_area).sum();
    if skel.height().is_finite() && (got - want).abs() <= 1e-3 * want.abs() {
        Some(planes)
    } else {
        None
    }
}

/// Same footprint with every hip edge at the length-weighted mean pitch, or
/// `None` if the hip edges already share one pitch (nothing to level).
fn level_pitches(prep: &Prepared) -> Option<Prepared> {
    let n = prep.poly.len();
    let hips: Vec<(usize, f64)> = (0..n)
        .filter(|&i| prep.settings[i].kind == EdgeKind::Hip)
        .map(|i| (i, prep.poly[i].dist(prep.poly[(i + 1) % n])))
        .collect();
    let total: f64 = hips.iter().map(|&(_, len)| len).sum();
    if total <= 0.0 {
        return None;
    }
    let mean = hips
        .iter()
        .map(|&(i, len)| prep.settings[i].pitch_in_12 * len)
        .sum::<f64>()
        / total;
    if hips
        .iter()
        .all(|&(i, _)| (prep.settings[i].pitch_in_12 - mean).abs() < 1e-9)
    {
        return None;
    }
    let mut settings = prep.settings.clone();
    for &(i, _) in &hips {
        settings[i].pitch_in_12 = mean;
    }
    Some(Prepared {
        poly: prep.poly.clone(),
        settings,
        source: prep.source.clone(),
    })
}

/// Last resort: a hip roof on the footprint's bounding rectangle. Each side takes
/// the settings of the footprint edge whose midpoint lies closest to it.
fn build_bbox(prep: &Prepared, baseline: f64) -> Vec<RoofPlane> {
    let (mut lo, mut hi) = (prep.poly[0], prep.poly[0]);
    for p in &prep.poly {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let corners = vec![lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)];
    let n = prep.poly.len();
    let mid = |k: usize| Point::lerp(prep.poly[k], prep.poly[(k + 1) % n], 0.5);
    // Distance from an edge midpoint to each bounding-box side.
    let side_dist = |side: usize, m: Point| match side {
        0 => (m.y - lo.y).abs(),
        1 => (hi.x - m.x).abs(),
        2 => (hi.y - m.y).abs(),
        _ => (m.x - lo.x).abs(),
    };
    let mut settings = Vec::new();
    let mut source = Vec::new();
    for side in 0..4 {
        let best = (0..n)
            .min_by(|&a, &b| {
                side_dist(side, mid(a))
                    .partial_cmp(&side_dist(side, mid(b)))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0);
        settings.push(prep.settings[best]);
        source.push(prep.source[best]);
    }
    let rect = Prepared {
        poly: corners,
        settings,
        source,
    };
    build_exact(&rect, baseline).unwrap_or_default()
}

/// Build a roof over `footprint`.
///
/// * `footprint`: counter-clockwise simple polygon, inches (wall centerline or
///   outer face). A clockwise outline is accepted and reversed.
/// * `edges`: one setting per footprint edge `i = (p[i], p[i+1])`; missing
///   entries use [`EdgeRoof::default`].
/// * `baseline_elevation`: top of wall, inches; the eave line of every plane.
///
/// Each footprint edge is first moved outward by its overhang. A weighted
/// straight skeleton of that outline then gives the hips, ridges and valleys:
/// edge `i` rises at its own pitch, so differing pitches meet at the correct
/// skewed hips. [`EdgeKind::Gable`] and [`EdgeKind::Shed`] edges stay vertical
/// and emit no plane. If the skeleton cannot be computed (self-intersecting
/// input, or a faster plane overtaking a parallel neighbour across a collapsing
/// step) the roof is approximated and [`Roof::approximate`] is set, see there.
pub fn build_roof(footprint: &[Point], edges: &[EdgeRoof], baseline_elevation: f64) -> Roof {
    let mut roof = Roof {
        planes: Vec::new(),
        fascia_height: DEFAULT_FASCIA_HEIGHT,
        baseline_elevation,
        approximate: false,
    };
    let Some(prep) = prepare(footprint, edges) else {
        return roof;
    };
    match build_exact(&prep, baseline_elevation) {
        Some(planes) => roof.planes = planes,
        None => {
            roof.approximate = true;
            roof.planes = level_pitches(&prep)
                .and_then(|level| build_exact(&level, baseline_elevation))
                .unwrap_or_else(|| build_bbox(&prep, baseline_elevation));
        }
    }
    roof
}

#[cfg(test)]
mod tests;
