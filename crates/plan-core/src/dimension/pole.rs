//! Auto Story Pole and Auto Elevation Dimensions: the elevation marks of a
//! plan (floors, plates, ceilings, roof eaves and ridges, opening sills and
//! heads) and the vertical dimension strings between them (manual pp. 493 to
//! 497, in our own words), and the roof as a section cut sees it.
//!
//! The plan has no elevation view to put the strings on, so a string stands
//! on a vertical line of the plan whose Y axis is the height: lengths read as
//! heights. Positive offsets run to the left (west) of the line.

use super::settings::{MarkKind, PoleSetup};
use super::{AutoGroup, Dimension, DimensionKind};
use crate::geometry::Point;
use crate::model::Floor;

/// One elevation a pole locates.
#[derive(Debug, Clone, PartialEq)]
pub struct ElevationMark {
    pub kind: MarkKind,
    /// The name shown beside the mark (the Locate Elevations panel's name,
    /// with the floor's name before a floor's own marks).
    pub name: String,
    /// Inches above the plan's origin level (the first floor's finished
    /// floor is `0`).
    pub elevation: f64,
    /// Plan x the mark was found at (a roof ridge's, a floor's centre).
    pub x: f64,
    /// Also on the outer string.
    pub outer: bool,
}

/// An elevation read off a roof: the eave or ridge of a plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoofMark {
    pub kind: MarkKind,
    pub elevation: f64,
    pub x: f64,
}

/// The eaves and ridges of roof planes given as `[x, elevation, -y]`
/// vertices: each plane's lowest vertices are its eave, its highest its
/// ridge (a flat plane has only an eave).
pub fn roof_marks(planes: &[Vec<[f64; 3]>]) -> Vec<RoofMark> {
    let mut out: Vec<RoofMark> = Vec::new();
    for poly in planes {
        let lo = poly.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let hi = poly.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
        if !lo.is_finite() || !hi.is_finite() {
            continue;
        }
        let x_at = |h: f64| {
            let xs: Vec<f64> = poly
                .iter()
                .filter(|p| (p[1] - h).abs() < 1e-6)
                .map(|p| p[0])
                .collect();
            xs.iter().sum::<f64>() / xs.len().max(1) as f64
        };
        out.push(RoofMark {
            kind: MarkKind::Eave,
            elevation: lo,
            x: x_at(lo),
        });
        if hi - lo > 0.5 {
            out.push(RoofMark {
                kind: MarkKind::Ridge,
                elevation: hi,
                x: x_at(hi),
            });
        }
    }
    out
}

/// The marks of the floors: top of subfloor, top of plate, ceiling and the
/// openings' sills and heads, for the kinds the setup locates.
pub fn floor_marks(floors: &[Floor], setup: &PoleSetup) -> Vec<ElevationMark> {
    let wants = |k: MarkKind| setup.marks.iter().find(|m| m.kind == k);
    let mut out = Vec::new();
    for f in floors {
        let xs: Vec<f64> = f.walls.iter().flat_map(|w| [w.start.x, w.end.x]).collect();
        let x = if xs.is_empty() {
            0.0
        } else {
            xs.iter().sum::<f64>() / xs.len() as f64
        };
        let mut add = |kind: MarkKind, elevation: f64| {
            if let Some(m) = wants(kind) {
                out.push(ElevationMark {
                    kind,
                    name: format!("{} {}", f.name, m.display()),
                    elevation,
                    x,
                    outer: m.outer,
                });
            }
        };
        add(
            MarkKind::TopOfSubfloor,
            f.elevation - f.settings.floor_finish_thickness,
        );
        let plate = f.walls.iter().map(|w| w.height).fold(0.0, f64::max);
        if plate > 0.0 {
            add(MarkKind::TopOfPlate, f.elevation + plate);
        }
        add(MarkKind::Ceiling, f.elevation + f.ceiling_height);
        if wants(MarkKind::OpeningSill).is_some() || wants(MarkKind::OpeningHead).is_some() {
            let mut sills: Vec<f64> = f.openings.iter().map(|o| o.sill_height).collect();
            let mut heads: Vec<f64> = f.openings.iter().map(|o| o.sill_height + o.height).collect();
            for v in [&mut sills, &mut heads] {
                v.sort_by(f64::total_cmp);
                v.dedup_by(|a, b| (*a - *b).abs() < 0.01);
            }
            for s in sills.into_iter().filter(|s| *s > 0.5) {
                add(MarkKind::OpeningSill, f.elevation + s);
            }
            for h in heads {
                add(MarkKind::OpeningHead, f.elevation + h);
            }
        }
    }
    out
}

/// All marks a pole locates, lowest first, with roof marks `roof` read by
/// the setup: only the kinds its Locate Elevations panel lists, only the
/// highest ridge when Primary Ridge Marks Only is on, and only those within
/// `reach` (a share of the width of the building from the pole's side,
/// `100` is all of it).
pub fn pole_marks(
    floors: &[Floor],
    roof: &[RoofMark],
    setup: &PoleSetup,
    left: bool,
) -> Vec<ElevationMark> {
    let mut marks = floor_marks(floors, setup);
    let mut roofs: Vec<RoofMark> = roof
        .iter()
        .copied()
        .filter(|r| setup.marks.iter().any(|m| m.kind == r.kind))
        .collect();
    if setup.primary_ridges_only {
        let top = roofs
            .iter()
            .filter(|r| r.kind == MarkKind::Ridge)
            .map(|r| r.elevation)
            .fold(f64::NEG_INFINITY, f64::max);
        roofs.retain(|r| r.kind != MarkKind::Ridge || (r.elevation - top).abs() < 1e-6);
    }
    for r in roofs {
        let m = setup.marks.iter().find(|m| m.kind == r.kind).unwrap();
        marks.push(ElevationMark {
            kind: r.kind,
            name: m.display().to_string(),
            elevation: r.elevation,
            x: r.x,
            outer: m.outer,
        });
    }
    // Reach: marks whose x lies within the share of the building's width
    // from the pole's side.
    let reach = if left { setup.left_reach } else { setup.right_reach };
    if reach < 100 {
        let xs: Vec<f64> = marks.iter().map(|m| m.x).collect();
        let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if lo.is_finite() && hi > lo {
            let limit = (hi - lo) * f64::from(reach.max(1)) / 100.0;
            marks.retain(|m| if left { m.x - lo } else { hi - m.x } <= limit + 1e-6);
        }
    }
    marks.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
    marks.dedup_by(|b, a| a.kind == b.kind && (a.elevation - b.elevation).abs() < 1e-6);
    marks
}

fn seg(x: f64, lo: f64, hi: f64, offset: f64) -> Dimension {
    let mut d = Dimension::new(
        0,
        DimensionKind::AutoExterior,
        Point::new(x, lo),
        Point::new(x, hi),
        offset,
    );
    d.auto_group = AutoGroup::Levels;
    d
}

/// The strings of one pole on the vertical line `x`: the inner string
/// between every pair of neighbouring marks, then the outer string between
/// the marks that are on it. `left` puts the strings to the west of the line.
/// Each inner list is one string (the editor joins it as one object); the
/// result is `(inner, outer)`.
pub fn pole_strings(
    marks: &[ElevationMark],
    setup: &PoleSetup,
    x: f64,
    left: bool,
) -> (Vec<Dimension>, Vec<Dimension>) {
    let side = if left { 1.0 } else { -1.0 };
    let string = |ms: &[&ElevationMark], offset: f64| -> Vec<Dimension> {
        ms.windows(2)
            .filter(|w| w[1].elevation - w[0].elevation >= 0.5)
            .map(|w| seg(x, w[0].elevation, w[1].elevation, offset))
            .collect()
    };
    let all: Vec<&ElevationMark> = marks.iter().collect();
    let outer_marks: Vec<&ElevationMark> = marks.iter().filter(|m| m.outer).collect();
    let first = side * setup.first_line_offset;
    let second = side * (setup.first_line_offset + setup.line_separation);
    let inner = if setup.inner { string(&all, first) } else { Vec::new() };
    // With no inner string the outer one takes its place nearest the pole.
    let outer = if setup.between_markers {
        string(&outer_marks, if setup.inner { second } else { first })
    } else {
        Vec::new()
    };
    (inner, outer)
}

/// Where a vertical plane through `a` and `b` cuts roof planes (given as
/// `[x, elevation, -y]` polygons): the points of the roof profile as
/// `(distance along the cut from a, elevation)`, left to right with
/// repeated points merged.
pub fn section_profile(planes: &[Vec<[f64; 3]>], a: Point, b: Point) -> Vec<(f64, f64)> {
    let len = a.dist(b);
    if len < 1e-9 {
        return Vec::new();
    }
    let u = b.sub(a).normalized();
    let n = u.perp();
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for poly in planes {
        let k = poly.len();
        // Each edge crossing the cut plane contributes a point (the vertex
        // when it lies on it).
        let side = |p: &[f64; 3]| Point::new(p[0], -p[2]).sub(a).dot(n);
        for i in 0..k {
            let (p, q) = (poly[i], poly[(i + 1) % k]);
            let (sp, sq) = (side(&p), side(&q));
            if sp.abs() < 1e-9 {
                pts.push((Point::new(p[0], -p[2]).sub(a).dot(u), p[1]));
            }
            if sp * sq < 0.0 {
                let t = sp / (sp - sq);
                let x = p[0] + (q[0] - p[0]) * t;
                let y = -p[2] + (-q[2] + p[2]) * t;
                let h = p[1] + (q[1] - p[1]) * t;
                pts.push((Point::new(x, y).sub(a).dot(u), h));
            }
        }
    }
    pts.retain(|(s, _)| *s >= -1e-6 && *s <= len + 1e-6);
    pts.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.total_cmp(&q.1)));
    pts.dedup_by(|q, p| (p.0 - q.0).abs() < 1e-6 && (p.1 - q.1).abs() < 1e-6);
    pts
}

/// The marks a section cut shows on its roof: the eave at each end of the
/// profile and the ridge (its highest point). `x` of a mark is its distance
/// along the cut.
pub fn section_roof_marks(profile: &[(f64, f64)]) -> Vec<RoofMark> {
    let (Some(first), Some(last)) = (profile.first(), profile.last()) else {
        return Vec::new();
    };
    let mut out = vec![RoofMark {
        kind: MarkKind::Eave,
        elevation: first.1,
        x: first.0,
    }];
    if (last.0 - first.0).abs() > 1e-6 {
        out.push(RoofMark {
            kind: MarkKind::Eave,
            elevation: last.1,
            x: last.0,
        });
    }
    if let Some(top) = profile.iter().max_by(|p, q| p.1.total_cmp(&q.1)) {
        if top.1 - first.1.min(last.1) > 0.5 {
            out.push(RoofMark {
                kind: MarkKind::Ridge,
                elevation: top.1,
                x: top.0,
            });
        }
    }
    out
}

/// Roof planes as seen in a section: for every sloped stretch of the cut's
/// profile, a dimension along the slope of the roof (its rise over its run,
/// drawn in the section's own frame: x along the cut, y the height). Ids
/// are `0`.
pub fn roof_slope_dimensions(profile: &[(f64, f64)], offset: f64) -> Vec<Dimension> {
    profile
        .windows(2)
        .filter(|w| (w[1].0 - w[0].0).abs() > 0.5 && (w[1].1 - w[0].1).abs() > 0.5)
        .map(|w| {
            let mut d = Dimension::new(
                0,
                DimensionKind::AutoExterior,
                Point::new(w[0].0, w[0].1),
                Point::new(w[1].0, w[1].1),
                offset,
            );
            d.auto_group = AutoGroup::Levels;
            d
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    fn gable() -> Vec<Vec<[f64; 3]>> {
        // A 240 x 240 gable roof, ridge along x at y = 120, eave at 108",
        // ridge at 168".
        vec![
            vec![
                [0.0, 108.0, 0.0],
                [240.0, 108.0, 0.0],
                [240.0, 168.0, -120.0],
                [0.0, 168.0, -120.0],
            ],
            vec![
                [240.0, 108.0, -240.0],
                [0.0, 108.0, -240.0],
                [0.0, 168.0, -120.0],
                [240.0, 168.0, -120.0],
            ],
        ]
    }

    #[test]
    fn roof_planes_give_eaves_and_one_ridge_height() {
        let marks = roof_marks(&gable());
        let ridges: Vec<_> = marks.iter().filter(|m| m.kind == MarkKind::Ridge).collect();
        assert_eq!(ridges.len(), 2);
        assert!(ridges.iter().all(|m| (m.elevation - 168.0).abs() < 1e-9));
        assert!(marks
            .iter()
            .any(|m| m.kind == MarkKind::Eave && (m.elevation - 108.0).abs() < 1e-9));
    }

    #[test]
    fn a_pole_names_its_floors_and_roof_and_strings_the_marks() {
        let mut p = Project::new("t");
        p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            97.125,
            crate::model::WallKind::Exterior,
        );
        let setup = PoleSetup::default();
        let marks = pole_marks(&p.floors, &roof_marks(&gable()), &setup, true);
        let kinds: Vec<MarkKind> = marks.iter().map(|m| m.kind).collect();
        assert!(kinds.contains(&MarkKind::TopOfSubfloor));
        assert!(kinds.contains(&MarkKind::TopOfPlate));
        assert!(kinds.contains(&MarkKind::Ridge));
        // Lowest first.
        assert!(marks.windows(2).all(|w| w[0].elevation <= w[1].elevation));
        // One ridge although two planes have one: Primary Ridge Marks Only.
        assert_eq!(kinds.iter().filter(|k| **k == MarkKind::Ridge).count(), 1);
        let (inner, outer) = pole_strings(&marks, &setup, -50.0, true);
        assert!(inner.len() >= 3, "{inner:?}");
        assert!(!outer.is_empty());
        // The outer string stands farther out than the inner.
        assert!(outer[0].offset > inner[0].offset);
        // On the right the offsets run the other way.
        let (inner_r, _) = pole_strings(&marks, &setup, 300.0, false);
        assert!(inner_r[0].offset < 0.0);
        // Turning the inner string off moves the outer one in.
        let mut no_inner = setup.clone();
        no_inner.inner = false;
        let (i2, o2) = pole_strings(&marks, &no_inner, -50.0, true);
        assert!(i2.is_empty());
        assert!((o2[0].offset - no_inner.first_line_offset).abs() < 1e-9);
        // Neither string: nothing.
        no_inner.between_markers = false;
        let (i3, o3) = pole_strings(&marks, &no_inner, -50.0, true);
        assert!(i3.is_empty() && o3.is_empty());
    }

    #[test]
    fn reach_limits_marks_to_the_pole_side() {
        let mut p = Project::new("t");
        p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            97.125,
            crate::model::WallKind::Exterior,
        );
        let mut roof = roof_marks(&gable());
        // Move one plane's marks to the far end.
        for r in roof.iter_mut().skip(2) {
            r.x = 240.0;
        }
        let mut setup = PoleSetup::default();
        setup.primary_ridges_only = false;
        setup.left_reach = 20;
        let near = pole_marks(&p.floors, &roof, &setup, true);
        setup.left_reach = 100;
        let all = pole_marks(&p.floors, &roof, &setup, true);
        assert!(near.len() <= all.len());
    }

    #[test]
    fn a_section_through_a_gable_shows_eaves_and_ridge() {
        let profile = section_profile(&gable(), Point::new(120.0, -10.0), Point::new(120.0, 250.0));
        // Eave at the south, ridge in the middle, eave at the north.
        assert!(profile.len() >= 3, "{profile:?}");
        assert!((profile[0].1 - 108.0).abs() < 1e-6);
        let marks = section_roof_marks(&profile);
        assert!(marks.iter().any(|m| m.kind == MarkKind::Ridge && (m.elevation - 168.0).abs() < 1e-6));
        assert_eq!(marks.iter().filter(|m| m.kind == MarkKind::Eave).count(), 2);
        let slopes = roof_slope_dimensions(&profile, 12.0);
        assert_eq!(slopes.len(), 2);
        // The slope length is the roof surface: run 120, rise 60.
        let want = (120.0f64.powi(2) + 60.0f64.powi(2)).sqrt();
        assert!((slopes[0].length() - want).abs() < 1e-6, "{}", slopes[0].length());
    }
}
