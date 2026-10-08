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
}

impl EdgeProfile {
    pub const ALL: [EdgeProfile; 3] = [
        EdgeProfile::Square,
        EdgeProfile::Beveled,
        EdgeProfile::Bullnose,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EdgeProfile::Square => "Square",
            EdgeProfile::Beveled => "Beveled",
            EdgeProfile::Bullnose => "Bullnose",
        }
    }

    /// Number of steps the top edge is built from.
    pub fn steps(self) -> usize {
        match self {
            EdgeProfile::Square => 0,
            EdgeProfile::Beveled => 1,
            EdgeProfile::Bullnose => 3,
        }
    }
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
        .filter(|c| c.kind.is_base_like() && c.top_polygon().is_some())
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
        let polys: Vec<Vec<Point>> = group.iter().filter_map(|c| c.top_polygon()).collect();
        let rings = geom::union_polygons(&polys);
        let holes: Vec<&Vec<Point>> = rings.iter().filter(|r| polygon_area(r) < 0.0).collect();
        for outer in rings.iter().filter(|r| polygon_area(r) > 0.0) {
            let sources: Vec<&Cabinet> = group
                .iter()
                .copied()
                .filter(|c| {
                    c.top_polygon()
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
}
