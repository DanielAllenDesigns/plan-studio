//! Units and conversion of DXF entities into plan-core CAD objects.

use crate::dxf::{DxfDrawing, DxfEntity, DxfUnits};
use plan_core::{CadItem, CadObject, Id, Point, Project};
use std::f64::consts::FRAC_PI_2;

/// Segments used per quarter turn when sampling bulged polyline arcs.
const SAMPLES_PER_QUARTER: f64 = 8.0;

/// Multiplier that converts drawing units to inches.
///
/// `override_units` (a user's choice in the import dialog) wins over the
/// units declared in the file. Unitless drawings are taken to be inches.
pub fn to_inches_factor(units: DxfUnits, override_units: Option<DxfUnits>) -> f64 {
    match override_units.unwrap_or(units) {
        DxfUnits::Unitless | DxfUnits::Inches => 1.0,
        DxfUnits::Feet => 12.0,
        DxfUnits::Millimeters => 1.0 / 25.4,
        DxfUnits::Centimeters => 1.0 / 2.54,
        DxfUnits::Meters => 1.0 / 0.0254,
    }
}

/// Convert a drawing's entities (INSERTs exploded) to CAD objects scaled by
/// `factor` (see [`to_inches_factor`]).
///
/// Each object's layer is `layer_prefix` followed by the DXF layer name, e.g.
/// prefix `"Import: "` gives `"Import: A-WALL"`. Bulged polyline segments are
/// sampled into straight pieces (8 per 90 degrees). Object ids are
/// sequential placeholders starting at 1; use [`apply_cad`] to add the result
/// to a [`Project`] with real ids.
pub fn to_cad_objects(drawing: &DxfDrawing, factor: f64, layer_prefix: &str) -> Vec<CadObject> {
    let scale = |p: Point| p.scale(factor);
    let mut out = Vec::new();
    for entity in drawing.explode_inserts() {
        let (layer, item) = match entity {
            DxfEntity::Line { a, b, layer } => (
                layer,
                CadItem::Line {
                    a: scale(a),
                    b: scale(b),
                },
            ),
            DxfEntity::Polyline {
                points,
                closed,
                layer,
                bulges,
            } => (
                layer,
                CadItem::Polyline {
                    points: sample_polyline(&points, &bulges, closed)
                        .into_iter()
                        .map(scale)
                        .collect(),
                    closed,
                },
            ),
            DxfEntity::Circle {
                center,
                radius,
                layer,
            } => (
                layer,
                CadItem::Circle {
                    center: scale(center),
                    radius: radius * factor,
                },
            ),
            DxfEntity::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                layer,
            } => (
                layer,
                CadItem::Arc {
                    center: scale(center),
                    radius: radius * factor,
                    start_angle: start_deg.to_radians(),
                    end_angle: end_deg.to_radians(),
                },
            ),
            DxfEntity::Text {
                pos,
                text,
                height,
                angle_deg,
                layer,
            } => (
                layer,
                CadItem::Text {
                    pos: scale(pos),
                    text,
                    height: height * factor,
                    angle: angle_deg.to_radians(),
                },
            ),
            // `explode_inserts` never returns INSERTs.
            DxfEntity::Insert { .. } => continue,
        };
        out.push(CadObject {
            id: out.len() as Id + 1,
            layer: format!("{layer_prefix}{layer}"),
            item,
        });
    }
    out
}

/// Add converted objects to `floor` of `project` with fresh ids; returns them.
pub fn apply_cad(project: &mut Project, floor: usize, objects: &[CadObject]) -> Vec<Id> {
    objects
        .iter()
        .map(|o| project.add_cad(floor, o.layer.clone(), o.item.clone()))
        .collect()
}

/// Vertices of a polyline with bulged segments replaced by sampled arcs.
/// The result keeps the original vertices; for a closed polyline the start
/// point is not repeated at the end.
fn sample_polyline(points: &[Point], bulges: &[f64], closed: bool) -> Vec<Point> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    let segments = if closed { n } else { n - 1 };
    let mut out = vec![points[0]];
    for i in 0..segments {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let bulge = bulges.get(i).copied().unwrap_or(0.0);
        out.extend(arc_between(a, b, bulge));
        if i + 1 < n {
            out.push(b);
        }
    }
    out
}

/// Intermediate points (excluding both ends) of the arc from `a` to `b` with
/// the given DXF bulge (`tan(sweep / 4)`, positive = counter-clockwise).
fn arc_between(a: Point, b: Point, bulge: f64) -> Vec<Point> {
    let chord = b.sub(a);
    let d = chord.length();
    if bulge.abs() < 1e-9 || d < 1e-9 {
        return Vec::new();
    }
    let sweep = 4.0 * bulge.atan();
    let half = sweep * 0.5;
    // Center lies on the chord's perpendicular bisector; left of a->b for
    // counter-clockwise arcs, right (negative offset) for clockwise ones, and
    // flipped again for arcs past a semicircle.
    let center = Point::lerp(a, b, 0.5).add(chord.normalized().perp().scale(d * 0.5 / half.tan()));
    let radius = a.dist(center);
    let start = a.sub(center).angle();
    let steps = ((sweep.abs() / FRAC_PI_2 * SAMPLES_PER_QUARTER).ceil() as usize).max(1);
    (1..steps)
        .map(|k| {
            let ang = start + sweep * k as f64 / steps as f64;
            Point::new(center.x + radius * ang.cos(), center.y + radius * ang.sin())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dxf::parse_dxf;

    #[test]
    fn unit_factors() {
        assert_eq!(to_inches_factor(DxfUnits::Millimeters, None), 1.0 / 25.4);
        assert_eq!(to_inches_factor(DxfUnits::Feet, None), 12.0);
        assert_eq!(to_inches_factor(DxfUnits::Inches, None), 1.0);
        assert_eq!(to_inches_factor(DxfUnits::Unitless, None), 1.0);
        assert!((to_inches_factor(DxfUnits::Meters, None) - 39.370_078_74).abs() < 1e-6);
        assert!((to_inches_factor(DxfUnits::Centimeters, None) - 1.0 / 2.54).abs() < 1e-12);
        // The override wins over the file's declared units.
        assert_eq!(
            to_inches_factor(DxfUnits::Millimeters, Some(DxfUnits::Feet)),
            12.0
        );
    }

    #[test]
    fn semicircle_bulge_samples_a_true_arc() {
        // Bulge 1 = half circle from (0,0) to (10,0); center (5,0), radius 5.
        let pts = sample_polyline(
            &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            &[1.0, 0.0],
            false,
        );
        assert_eq!(pts.len(), 17); // 16 steps over 180 degrees
        for p in &pts {
            assert!((p.dist(Point::new(5.0, 0.0)) - 5.0).abs() < 1e-9);
        }
        // Counter-clockwise from (0,0) means the arc bows below the chord.
        assert!(pts[8].y < -4.9);
        // Negative bulge bows the other way.
        let cw = sample_polyline(
            &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            &[-1.0, 0.0],
            false,
        );
        assert!(cw[8].y > 4.9);
    }

    #[test]
    fn closed_polyline_with_bulge_keeps_closing_arc() {
        let sq = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        let pts = sample_polyline(&sq, &[0.0, 0.0, 0.0, 0.414_213_562], true);
        // 4 vertices + 8 samples for a 90 degree closing arc, minus the shared end.
        assert_eq!(pts.len(), 4 + 7);
        assert_eq!(pts[0], sq[0]);
    }

    #[test]
    fn drawing_converts_with_scale_prefix_and_exploded_inserts() {
        let src = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB\n10\n0\n20\n0\n\
0\nLINE\n8\n0\n10\n0\n20\n0\n11\n254\n21\n0\n0\nENDBLK\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n0\nINSERT\n8\nL\n2\nB\n10\n0\n20\n0\n\
0\nTEXT\n8\nT\n10\n254\n20\n254\n40\n127\n1\nHi\n50\n90\n\
0\nCIRCLE\n8\nC\n10\n0\n20\n0\n40\n25.4\n0\nENDSEC\n0\nEOF\n";
        let d = parse_dxf(src).unwrap();
        let objs = to_cad_objects(&d, 1.0 / 25.4, "Import: ");
        assert_eq!(objs.len(), 3);
        assert_eq!(objs[0].layer, "Import: L");
        assert!(matches!(
            objs[0].item,
            CadItem::Line { b, .. } if b.dist(Point::new(10.0, 0.0)) < 1e-9
        ));
        match &objs[1].item {
            CadItem::Text {
                pos,
                height,
                angle,
                text,
            } => {
                assert!(pos.dist(Point::new(10.0, 10.0)) < 1e-9);
                assert!((height - 5.0).abs() < 1e-9);
                assert!((angle - FRAC_PI_2).abs() < 1e-9);
                assert_eq!(text, "Hi");
            }
            other => panic!("expected text, got {other:?}"),
        }
        assert!(
            matches!(objs[2].item, CadItem::Circle { radius, .. } if (radius - 1.0).abs() < 1e-9)
        );

        let mut p = Project::new("x");
        let ids = apply_cad(&mut p, 0, &objs);
        assert_eq!(ids.len(), 3);
        assert_eq!(p.floors[0].cad.len(), 3);
    }
}
