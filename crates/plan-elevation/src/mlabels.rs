//! Material labels: a text leader per cladding or roofing region naming what
//! the surface is made of ("SIDING", "BRICK", "ROOFING"...).

use crate::drawing::{Drawing, EdgeKind, Line2, LineWeight, RegionKind, TEXT_H};
use plan_3d::Material;
use plan_core::Point;

/// Regions smaller than this are not labelled, square inches (about 3 sq ft).
const MIN_AREA: f64 = 432.0;
/// A second region of a material is labelled when it is at least this fraction
/// of the largest and this far from it, inches.
const SECOND_FRACTION: f64 = 0.4;
const SECOND_DISTANCE: f64 = 120.0;
/// Gap between the drawing and the label column, inches.
const COLUMN_GAP: f64 = 96.0;
/// Vertical room one label takes, inches.
const LABEL_PITCH: f64 = 1.8 * TEXT_H;

/// The label of a material, if surfaces of it get one.
pub fn material_label(m: Material) -> Option<&'static str> {
    Some(match m {
        Material::Siding => "SIDING",
        Material::Brick => "BRICK",
        Material::Stucco => "STUCCO",
        Material::Stone => "STONE",
        Material::Roof => "ROOFING",
        Material::Concrete => "CONCRETE",
        Material::Trim => "TRIM",
        Material::Metal => "METAL",
        _ => return None,
    })
}

/// A point well inside a (weakly simple) ring: the middle of the widest span of
/// a few horizontal scanlines, even-odd.
pub fn interior_point(ring: &[Point]) -> Option<Point> {
    let (lo, hi) = ring
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
    if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    let mut best: Option<(f64, Point)> = None;
    for f in [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8] {
        let y = lo + (hi - lo) * f;
        let mut xs: Vec<f64> = (0..ring.len())
            .filter_map(|i| {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let (p, q) = if a.y <= b.y { (a, b) } else { (b, a) };
                (p.y <= y && y < q.y).then(|| p.x + (q.x - p.x) * (y - p.y) / (q.y - p.y))
            })
            .collect();
        xs.sort_by(f64::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            let w = pair[1] - pair[0];
            if best.is_none_or(|(bw, _)| w > bw) {
                best = Some((w, Point::new(0.5 * (pair[0] + pair[1]), y)));
            }
        }
    }
    best.filter(|(w, _)| *w > 1.0).map(|(_, p)| p)
}

/// Adds the labels to `drawing` from its face regions, in a column right of
/// the drawing, each with a leader to a point on its region.
pub(crate) fn add_material_labels(drawing: &mut Drawing) {
    let right = drawing.bounds.1.x + COLUMN_GAP;
    // (label, area, point) of the faces worth naming, largest first.
    let mut faces: Vec<(&'static str, f64, Point)> = drawing
        .regions_of(RegionKind::Face)
        .filter_map(|r| {
            let name = material_label(r.material)?;
            let area = r.area();
            (area >= MIN_AREA)
                .then(|| interior_point(&r.polygon))
                .flatten()
                .map(|p| (name, area, p))
        })
        .collect();
    faces.sort_by(|a, b| b.1.total_cmp(&a.1));

    let mut picked: Vec<(&'static str, Point)> = Vec::new();
    for &(name, area, p) in &faces {
        let same: Vec<&(&str, f64, Point)> = faces.iter().filter(|f| f.0 == name).collect();
        let largest = same.first().map_or(area, |f| f.1);
        let taken: Vec<Point> = picked
            .iter()
            .filter(|(n, _)| *n == name)
            .map(|(_, q)| *q)
            .collect();
        let ok = match taken.len() {
            0 => true,
            1 => area >= SECOND_FRACTION * largest && taken[0].dist(p) >= SECOND_DISTANCE,
            _ => false,
        };
        if ok {
            picked.push((name, p));
        }
    }
    picked.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));

    // Spread the labels so none overlaps the next.
    let mut ys: Vec<f64> = picked.iter().map(|(_, p)| p.y).collect();
    for i in 1..ys.len() {
        ys[i] = ys[i].max(ys[i - 1] + LABEL_PITCH);
    }
    for ((name, p), y) in picked.iter().zip(ys) {
        let anchor = Point::new(right, y - 0.35 * TEXT_H);
        for (a, b) in [
            (*p, Point::new(right - 4.0, y)),
            (
                Point::new(p.x - 2.5, p.y - 2.5),
                Point::new(p.x + 2.5, p.y + 2.5),
            ),
            (
                Point::new(p.x - 2.5, p.y + 2.5),
                Point::new(p.x + 2.5, p.y - 2.5),
            ),
        ] {
            drawing.lines.push(Line2 {
                a,
                b,
                weight: LineWeight::Light,
                kind: EdgeKind::Annotation,
            });
        }
        drawing.texts.push((anchor, (*name).to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interior_point_avoids_a_hole() {
        // A square with a square hole in the middle, joined by a slit.
        let ring = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
            Point::new(0.0, 50.0),
            Point::new(30.0, 50.0),
            Point::new(30.0, 30.0),
            Point::new(70.0, 30.0),
            Point::new(70.0, 70.0),
            Point::new(30.0, 70.0),
            Point::new(30.0, 50.0),
            Point::new(0.0, 50.0),
        ];
        let p = interior_point(&ring).expect("a point");
        assert!(!(p.x > 30.0 && p.x < 70.0 && p.y > 30.0 && p.y < 70.0));
        assert!(p.x > 0.0 && p.x < 100.0 && p.y > 0.0 && p.y < 100.0);
    }

    #[test]
    fn only_cladding_and_roofing_are_named() {
        assert_eq!(material_label(Material::Roof), Some("ROOFING"));
        assert_eq!(material_label(Material::Brick), Some("BRICK"));
        assert_eq!(material_label(Material::WindowGlass), None);
    }
}
