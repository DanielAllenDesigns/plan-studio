//! Countertops that are not just a cabinet's own slab: custom (free-form)
//! tops and backsplashes, sink and cooktop cutouts, and the "Generate
//! Countertop" command that joins the tops of adjacent base cabinets.

use plan_core::geometry::{point_in_polygon, polygon_area, Point};
use plan_core::Id;
use serde::{Deserialize, Serialize};

use crate::cabinet::{Cabinet, CabinetKind};
use crate::geom;

/// The shape of a countertop's exposed edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EdgeProfile {
    #[default]
    Square,
    /// One 45 degree chamfer along the top edge.
    Beveled,
    /// A rounded (quarter-round) top edge.
    Bullnose,
    /// An S-curve edge: a cove under a bead.
    Ogee,
}

impl EdgeProfile {
    pub const ALL: [EdgeProfile; 4] = [
        EdgeProfile::Square,
        EdgeProfile::Beveled,
        EdgeProfile::Bullnose,
        EdgeProfile::Ogee,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EdgeProfile::Square => "Square",
            EdgeProfile::Beveled => "Beveled",
            EdgeProfile::Bullnose => "Bullnose",
            EdgeProfile::Ogee => "Ogee",
        }
    }

    /// Number of steps the top edge is built from.
    pub fn steps(self) -> usize {
        match self {
            EdgeProfile::Square => 0,
            EdgeProfile::Beveled => 1,
            EdgeProfile::Bullnose => 3,
            EdgeProfile::Ogee => 8,
        }
    }
}

/// How the outside corners of a countertop are shaped (Chief's Corner
/// Treatment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CornerTreatment {
    #[default]
    None,
    /// Cut off square across the corner.
    Clipped,
    /// A round corner.
    Rounded,
}

impl CornerTreatment {
    pub const ALL: [CornerTreatment; 3] = [
        CornerTreatment::None,
        CornerTreatment::Clipped,
        CornerTreatment::Rounded,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CornerTreatment::None => "None",
            CornerTreatment::Clipped => "Clipped",
            CornerTreatment::Rounded => "Rounded",
        }
    }
}

/// Points of a rounded corner's arc.
const ARC_STEPS: usize = 6;

/// `ring` with the convex corners that `pick` accepts clipped or rounded by
/// `size` (a leg of the cut, or the radius). A corner never takes more than
/// half of either edge. Other corners, and `CornerTreatment::None`, keep the
/// ring as it is; the result is counter-clockwise.
pub fn treat_corners(
    ring: &[Point],
    treatment: CornerTreatment,
    size: f64,
    pick: impl Fn(Point) -> bool,
) -> Vec<Point> {
    if treatment == CornerTreatment::None || size <= 1e-6 || ring.len() < 3 {
        return ring.to_vec();
    }
    let ring = geom::ccw(ring);
    let n = ring.len();
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let (a, v, b) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
        let convex = v.sub(a).cross(b.sub(v)) > 1e-9;
        let (l1, l2) = (v.dist(a), v.dist(b));
        if !convex || !pick(v) || l1 < 1e-9 || l2 < 1e-9 {
            out.push(v);
            continue;
        }
        let (u1, u2) = (a.sub(v).normalized(), b.sub(v).normalized());
        let limit = (l1 / 2.0).min(l2 / 2.0);
        match treatment {
            CornerTreatment::Clipped | CornerTreatment::None => {
                let t = size.min(limit);
                out.push(v.add(u1.scale(t)));
                out.push(v.add(u2.scale(t)));
            }
            CornerTreatment::Rounded => {
                let theta = u1.dot(u2).clamp(-1.0, 1.0).acos();
                let half = (theta / 2.0).tan();
                if half < 1e-6 {
                    out.push(v);
                    continue;
                }
                let t = (size / half).min(limit);
                let r = t * half;
                let (p1, p2) = (v.add(u1.scale(t)), v.add(u2.scale(t)));
                let c = v.add(u1.add(u2).normalized().scale(r / (theta / 2.0).sin()));
                let a1 = (p1.y - c.y).atan2(p1.x - c.x);
                let a2 = (p2.y - c.y).atan2(p2.x - c.x);
                let mut sweep = (a2 - a1).rem_euclid(std::f64::consts::TAU);
                if sweep > std::f64::consts::PI {
                    sweep -= std::f64::consts::TAU;
                }
                for k in 0..=ARC_STEPS {
                    let th = a1 + sweep * k as f64 / ARC_STEPS as f64;
                    out.push(Point::new(c.x + r * th.cos(), c.y + r * th.sin()));
                }
            }
        }
    }
    out
}

/// The free-form part of a custom countertop or backsplash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomTop {
    /// Outline (a closed ring) or path (open), in the cabinet's local frame.
    pub outline: Vec<Point>,
    /// Slab thickness, or the strip thickness of a backsplash.
    pub thickness: f64,
    pub edge: EdgeProfile,
    /// Size of the edge treatment, inches.
    pub edge_size: f64,
    /// A countertop is closed; a backsplash follows an open path.
    pub closed: bool,
    /// Treatment of the outside corners of a closed top.
    #[serde(default)]
    pub corner: CornerTreatment,
    /// Size of that treatment, inches.
    #[serde(default = "default_corner_size")]
    pub corner_size: f64,
}

/// Default size of a corner treatment, inches.
pub(crate) fn default_corner_size() -> f64 {
    2.0
}

impl CustomTop {
    /// The region it covers, local frame: the ring itself, or the thickened path.
    pub fn footprint(&self) -> Vec<Point> {
        if self.closed {
            self.outline.clone()
        } else {
            geom::thicken_path(&self.outline, self.thickness)
        }
    }

    /// The outline with its corner treatment applied (the shape that is
    /// drawn and built).
    pub fn treated(&self) -> Vec<Point> {
        if self.closed {
            treat_corners(&self.outline, self.corner, self.corner_size, |_| true)
        } else {
            self.footprint()
        }
    }
}

/// What a countertop hole is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CutoutKind {
    Sink,
    Cooktop,
    Custom,
}

/// A hole in a countertop (local frame of the cabinet that owns it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cutout {
    pub kind: CutoutKind,
    pub name: String,
    pub outline: Vec<Point>,
}

impl Cutout {
    /// A `w` by `d` rectangle centred on `center`.
    pub fn rect(kind: CutoutKind, name: &str, center: Point, w: f64, d: f64) -> Self {
        let (hx, hy) = (w / 2.0, d / 2.0);
        Self {
            kind,
            name: name.to_string(),
            outline: vec![
                Point::new(center.x - hx, center.y - hy),
                Point::new(center.x + hx, center.y - hy),
                Point::new(center.x + hx, center.y + hy),
                Point::new(center.x - hx, center.y + hy),
            ],
        }
    }

    pub fn area(&self) -> f64 {
        geom::area(&self.outline)
    }
}

/// A countertop made from adjacent base cabinets.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedTop {
    /// The new custom countertop (id `0`; the caller assigns one).
    pub top: Cabinet,
    /// The cabinets it covers.
    pub sources: Vec<Id>,
}

/// A point strictly inside the ring (the first triangle's centroid).
fn interior_point(ring: &[Point]) -> Point {
    geom::triangulate(ring, &[])
        .first()
        .map(|t| {
            Point::new(
                (t[0].x + t[1].x + t[2].x) / 3.0,
                (t[0].y + t[1].y + t[2].y) / 3.0,
            )
        })
        .unwrap_or(Point::ZERO)
}

/// Joins the countertops of touching base-like cabinets into custom
/// countertops, one per connected group at the same top height. Overhangs
/// stay on the free edges only, corner cabinets contribute their L or
/// diagonal outline, gaps break the top, and the sink and cooktop cutouts of
/// the source cabinets carry over as holes.
pub fn generate_countertops(cabs: &[Cabinet]) -> Vec<GeneratedTop> {
    let cands: Vec<&Cabinet> = cabs
        .iter()
        .filter(|c| c.kind.is_base_like() && c.top_polygon_square().is_some())
        .collect();
    let mut heights: Vec<i64> = cands
        .iter()
        .map(|c| ((c.elevation + c.height) * 8.0).round() as i64)
        .collect();
    heights.sort_unstable();
    heights.dedup();
    let mut out = Vec::new();
    for h in heights {
        let group: Vec<&Cabinet> = cands
            .iter()
            .copied()
            .filter(|c| ((c.elevation + c.height) * 8.0).round() as i64 == h)
            .collect();
        let polys: Vec<Vec<Point>> = group
            .iter()
            .filter_map(|c| c.top_polygon_square())
            .collect();
        let rings = geom::union_polygons(&polys);
        let holes: Vec<&Vec<Point>> = rings.iter().filter(|r| polygon_area(r) < 0.0).collect();
        for outer in rings.iter().filter(|r| polygon_area(r) > 0.0) {
            let sources: Vec<&Cabinet> = group
                .iter()
                .copied()
                .filter(|c| {
                    c.top_polygon_square()
                        .is_some_and(|p| point_in_polygon(interior_point(&p), outer))
                })
                .collect();
            if sources.is_empty() {
                continue;
            }
            let thickness = sources
                .iter()
                .filter_map(|c| c.countertop.map(|t| t.thickness))
                .fold(0.0, f64::max);
            let top = sources
                .iter()
                .map(|c| c.elevation + c.height)
                .fold(f64::MIN, f64::max);
            let Some(mut slab) = Cabinet::custom_countertop(outer, thickness, top) else {
                continue;
            };
            // The joined top takes the shaping of the cabinets' tops.
            if let (Some(custom), Some(src)) = (
                slab.custom.as_mut(),
                sources.iter().find_map(|c| c.countertop),
            ) {
                custom.corner = src.corner;
                custom.corner_size = src.corner_size;
                custom.edge = src.edge;
                custom.edge_size = src.edge_size;
            }
            let origin = slab.position;
            for hole in &holes {
                if point_in_polygon(interior_point(&geom::ccw(hole)), outer) {
                    slab.cutouts.push(Cutout {
                        kind: CutoutKind::Custom,
                        name: "Opening".to_string(),
                        outline: geom::ccw(hole).iter().map(|p| p.sub(origin)).collect(),
                    });
                }
            }
            for s in &sources {
                for cut in &s.cutouts {
                    slab.cutouts.push(Cutout {
                        kind: cut.kind,
                        name: cut.name.clone(),
                        outline: cut
                            .outline
                            .iter()
                            .map(|p| s.to_plan(*p).sub(origin))
                            .collect(),
                    });
                }
            }
            out.push(GeneratedTop {
                top: slab,
                sources: sources.iter().map(|c| c.id).collect(),
            });
        }
    }
    out
}

impl Cabinet {
    /// Gives this cabinet's countertop to a generated custom top: the cabinet
    /// loses its slab (and cutouts) and shrinks by the slab thickness so the
    /// custom top sits exactly on its box. Returns false when it had none.
    pub fn hand_over_top(&mut self) -> bool {
        match self.countertop.take() {
            Some(t) => {
                self.height = (self.height - t.thickness).max(1.0);
                self.cutouts.clear();
                self.backsplash = None;
                true
            }
            None => false,
        }
    }

    /// Is this a free-form top or backsplash?
    pub fn is_custom_top(&self) -> bool {
        self.kind == CabinetKind::CustomCountertop || self.kind == CabinetKind::CustomBacksplash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cabinet::run_along_wall;

    fn base_at(x: f64, y: f64, w: f64) -> Cabinet {
        let mut c = Cabinet::base(w);
        c.position = Point::new(x, y);
        c
    }

    #[test]
    fn custom_countertop_volume_is_area_times_thickness_less_cutouts() {
        let ring = [
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 25.0),
            Point::new(0.0, 25.0),
        ];
        let mut c = Cabinet::custom_countertop(&ring, 1.5, 36.0).unwrap();
        assert_eq!(
            (c.width, c.depth, c.elevation, c.height),
            (60.0, 25.0, 34.5, 1.5)
        );
        assert!((c.countertop_volume() - 1500.0 * 1.5).abs() < 1e-6);
        // A cooktop hole removes its area from the slab.
        c.cutouts.push(Cutout::rect(
            CutoutKind::Cooktop,
            "Cooktop",
            Point::new(30.0, 12.5),
            30.0,
            18.0,
        ));
        assert!((c.countertop_volume() - (1500.0 - 540.0) * 1.5).abs() < 1e-6);
        // A degenerate outline is refused.
        assert!(Cabinet::custom_countertop(&ring[..2], 1.5, 36.0).is_none());
    }

    #[test]
    fn custom_countertop_follows_position_and_angle() {
        let tri = [
            Point::new(100.0, 100.0),
            Point::new(130.0, 100.0),
            Point::new(100.0, 140.0),
        ];
        let mut c = Cabinet::custom_countertop(&tri, 2.0, 36.0).unwrap();
        assert_eq!(c.position, Point::new(100.0, 100.0));
        assert!((geom::area(&c.footprint()) - 600.0).abs() < 1e-9);
        c.position = Point::new(0.0, 0.0);
        assert!(c.footprint().iter().all(|p| p.x <= 30.0 + 1e-9));
    }

    #[test]
    fn sink_cutout_is_centred_and_refused_on_a_narrow_top() {
        let mut c = Cabinet::base(36.0);
        assert!(c.add_cutout(CutoutKind::Sink));
        let hole = &c.cutouts[0].outline;
        let (lo, hi) = geom::bbox(hole).unwrap();
        assert!(((lo.x + hi.x) / 2.0 - 18.0).abs() < 1e-9);
        assert!((hi.x - lo.x - 30.0).abs() < 1e-9);
        let mut narrow = Cabinet::base(12.0);
        assert!(!narrow.add_cutout(CutoutKind::Cooktop));
        let mut wall = Cabinet::wall(30.0);
        assert!(!wall.add_cutout(CutoutKind::Sink), "no countertop");
        // Volume of a cabinet's own top drops by the hole.
        let full = Cabinet::base(36.0).countertop_volume();
        assert!((full - 36.0 * 25.0 * 1.5).abs() < 1e-9);
        assert!((c.countertop_volume() - (36.0 * 25.0 - 30.0 * 18.0) * 1.5).abs() < 1e-9);
    }

    #[test]
    fn generated_top_covers_all_adjacent_bases_and_skips_the_rest() {
        let run = run_along_wall(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            &[24.0, 36.0, 24.0],
            CabinetKind::Base,
        );
        let mut cabs: Vec<Cabinet> = run
            .into_iter()
            .enumerate()
            .map(|(i, mut c)| {
                c.id = i as Id + 1;
                c
            })
            .collect();
        // A far-away cabinet and a wall cabinet do not join.
        let mut far = base_at(200.0, 0.0, 24.0);
        far.id = 9;
        let mut wall = Cabinet::wall(30.0);
        wall.id = 10;
        cabs.push(far);
        cabs.push(wall);
        assert!(cabs[1].add_cutout(CutoutKind::Sink));
        let gen = generate_countertops(&cabs);
        assert_eq!(gen.len(), 2);
        let main = gen.iter().find(|g| g.sources.len() == 3).unwrap();
        assert_eq!(main.sources, vec![1, 2, 3]);
        let ring = main.top.footprint();
        for c in &cabs[..3] {
            for p in c.corners() {
                // Every cabinet corner lies in or on the generated top.
                let inside = point_in_polygon(p, &ring)
                    || ring.iter().any(|q| q.dist(p) < 1e-6)
                    || ring
                        .windows(2)
                        .any(|e| plan_core::geometry::dist_to_segment(p, e[0], e[1]) < 1e-6);
                assert!(inside, "corner {p:?} uncovered");
            }
        }
        // 84 wide, 24 deep plus the 1" front overhang, 1.5 thick, one sink hole.
        let expected = (84.0 * 25.0 - 30.0 * 18.0) * 1.5;
        assert!((main.top.countertop_volume() - expected).abs() < 1e-6);
        assert_eq!(main.top.elevation + main.top.height, 36.0);
    }

    #[test]
    fn a_corner_cabinet_joins_runs_on_both_walls() {
        // Corner at the origin: a run along +x on one wall, another along +y.
        let mut corner = Cabinet::corner_base(36.0).with_pie_cut(true);
        corner.id = 1;
        let mut a = base_at(36.0, 0.0, 30.0);
        a.id = 2;
        // A run up the left wall: fronts face +x, so the cabinet turns -90
        // degrees and its width runs down from (0, 66) to the corner's end.
        let mut b = base_at(0.0, 66.0, 30.0);
        b.angle = -std::f64::consts::FRAC_PI_2;
        b.id = 3;
        let gen = generate_countertops(&[corner.clone(), a, b]);
        assert_eq!(gen.len(), 1);
        assert_eq!(gen[0].sources.len(), 3);
        // The L top is larger than the corner's own top alone.
        let own = geom::area(&corner.top_polygon().unwrap());
        assert!(geom::area(&gen[0].top.footprint()) > own + 30.0 * 24.0);
    }

    #[test]
    fn handing_over_the_top_shrinks_the_cabinet() {
        let mut c = Cabinet::base(24.0);
        assert!(c.hand_over_top());
        assert_eq!((c.height, c.countertop), (34.5, None));
        assert!(!c.hand_over_top());
        assert!((c.face_height() - 30.5).abs() < 1e-9);
    }

    fn square() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ]
    }

    #[test]
    fn clipped_corners_cut_a_triangle_off_each_picked_corner() {
        let ring = treat_corners(&square(), CornerTreatment::Clipped, 2.0, |_| true);
        assert_eq!(ring.len(), 8);
        // 100 sq in less four 2x2 right triangles.
        assert!((geom::area(&ring) - (100.0 - 4.0 * 2.0)).abs() < 1e-9);
        // Only the corners the filter accepts.
        let front = treat_corners(&square(), CornerTreatment::Clipped, 2.0, |p| p.y > 5.0);
        assert_eq!(front.len(), 6);
        assert!((geom::area(&front) - 96.0).abs() < 1e-9);
        // Never more than half an edge.
        let big = treat_corners(&square(), CornerTreatment::Clipped, 50.0, |_| true);
        assert!(geom::area(&big) > 0.0 && big.len() == 8);
    }

    #[test]
    fn rounded_corners_follow_a_circle_of_the_radius() {
        let r = 2.0;
        let ring = treat_corners(&square(), CornerTreatment::Rounded, r, |_| true);
        // 100 less four corners of (1 - pi/4) r^2 each, a polygon a little under that.
        let corner = (1.0 - std::f64::consts::FRAC_PI_4) * r * r;
        let area = geom::area(&ring);
        assert!(
            area > 100.0 - 4.0 * corner - 0.3 && area < 100.0 - 4.0 * corner + 0.5,
            "{area}"
        );
        // Every arc point is r from its centre (2,2) etc.: nothing pokes out.
        assert!(ring.iter().all(|p| p.x >= -1e-9 && p.x <= 10.0 + 1e-9));
        let near = ring
            .iter()
            .filter(|p| p.dist(Point::new(2.0, 2.0)) < r + 1e-6 && p.x < 2.0 && p.y < 2.0)
            .count();
        assert!(near >= 6);
        // No treatment: unchanged.
        assert_eq!(
            treat_corners(&square(), CornerTreatment::None, 2.0, |_| true),
            square()
        );
    }

    #[test]
    fn a_countertop_shapes_its_front_corners_only() {
        let mut c = Cabinet::base(36.0);
        let plain = c.top_local().unwrap();
        assert_eq!(plain.len(), 4);
        let t = c.countertop.as_mut().unwrap();
        t.corner = CornerTreatment::Clipped;
        t.corner_size = 3.0;
        let shaped = c.top_local().unwrap();
        assert_eq!(shaped.len(), 6, "two front corners clipped");
        assert!((geom::area(&shaped) - (geom::area(&plain) - 2.0 * 4.5)).abs() < 1e-9);
        // The square ring joined tops use is unchanged.
        assert_eq!(c.top_local_square().unwrap(), plain);
        // The shaped top builds as one slab with a smaller volume.
        let ms = crate::meshes(&c);
        assert!(ms.iter().any(|m| m.material == plan_3d::Material::Floor));
        assert!((c.countertop_volume() - (geom::area(&plain) - 9.0) * 1.5).abs() < 1e-9);
    }

    #[test]
    fn an_ogee_edge_builds_and_stays_inside_the_slab() {
        let mut c = Cabinet::base(36.0);
        let t = c.countertop.as_mut().unwrap();
        t.edge = EdgeProfile::Ogee;
        t.edge_size = 0.75;
        assert_eq!(EdgeProfile::Ogee.steps(), 8);
        let slab = |c: &Cabinet| {
            crate::meshes(c)
                .into_iter()
                .find(|m| m.material == plan_3d::Material::Floor)
                .unwrap()
        };
        let ogee = slab(&c);
        let (lo, hi) = ogee.bounds().unwrap();
        assert!((hi[1] - 36.0).abs() < 1e-4 && (lo[1] - 34.5).abs() < 1e-4);
        let square = {
            c.countertop.as_mut().unwrap().edge = EdgeProfile::Square;
            slab(&c)
        };
        // The profile removes material from the top edge: more triangles,
        // same footprint.
        assert!(ogee.indices.len() > square.indices.len());
        let sb = square.bounds().unwrap();
        assert!((sb.0[0] - lo[0]).abs() < 1e-4 && (sb.1[2] - hi[2]).abs() < 1e-4);
    }

    #[test]
    fn a_joined_top_carries_the_corner_and_edge_of_its_cabinets() {
        let mut a = base_at(0.0, 0.0, 36.0);
        let mut b = base_at(36.0, 0.0, 36.0);
        for c in [&mut a, &mut b] {
            let t = c.countertop.as_mut().unwrap();
            t.corner = CornerTreatment::Rounded;
            t.corner_size = 2.0;
            t.edge = EdgeProfile::Bullnose;
        }
        a.id = 1;
        b.id = 2;
        let tops = generate_countertops(&[a, b]);
        assert_eq!(tops.len(), 1);
        let custom = tops[0].top.custom.as_ref().unwrap();
        assert_eq!(custom.corner, CornerTreatment::Rounded);
        assert_eq!(custom.edge, EdgeProfile::Bullnose);
        // Joined from the square rings: one rectangle, then rounded.
        assert_eq!(custom.outline.len(), 4);
        assert!(tops[0].top.top_local().unwrap().len() > 4);
    }
}
