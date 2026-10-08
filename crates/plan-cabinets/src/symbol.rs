//! The 2D plan symbol for a cabinet.

use plan_core::geometry::Point;
use serde::{Deserialize, Serialize};

use crate::cabinet::{auto_label, Cabinet, CabinetKind};

/// Inset of the front-face line and the wall-cabinet inner outline, inches.
const INSET: f64 = 0.75;
/// Height of the label text, inches.
const LABEL_HEIGHT: f64 = 3.0;

/// A plan-view drawing primitive in plan coordinates (inches).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stroke {
    Line(Point, Point),
    /// A polyline; the flag closes it back to the first point.
    Polyline(Vec<Point>, bool),
    /// A circular arc counter-clockwise from `start` to `end` radians.
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        end: f64,
    },
    Text {
        at: Point,
        text: String,
        height: f64,
        /// Baseline rotation in radians.
        angle: f64,
    },
}

/// Draw the cabinet in plan.
///
/// Every cabinet gets its box outline and a centred label ([`Cabinet::label`]
/// or [`auto_label`]). Cabinets with a countertop add the countertop outline
/// (Chief shows it dashed; here it is a second solid outline). Base and tall
/// cabinets add a front-face line 3/4" in from the front; wall cabinets add an
/// inner offset outline and a diagonal cross. Door swing arcs are not drawn.
pub fn plan_symbol(cabinet: &Cabinet) -> Vec<Stroke> {
    let (w, d) = (cabinet.width, cabinet.depth);
    let p = |x: f64, y: f64| cabinet.to_plan(Point::new(x, y));
    let mut out = vec![Stroke::Polyline(cabinet.corners().to_vec(), true)];

    if let Some(top) = cabinet.countertop {
        let (x0, x1) = (-top.overhang_sides, w + top.overhang_sides);
        let (y0, y1) = (-top.overhang_back, d + top.overhang_front);
        out.push(Stroke::Polyline(
            vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)],
            true,
        ));
    }

    match cabinet.kind {
        CabinetKind::Base | CabinetKind::FullHeight => {
            out.push(Stroke::Line(p(0.0, d - INSET), p(w, d - INSET)));
        }
        CabinetKind::Wall => {
            let i = INSET.min(w / 2.0).min(d / 2.0);
            out.push(Stroke::Polyline(
                vec![p(i, i), p(w - i, i), p(w - i, d - i), p(i, d - i)],
                true,
            ));
            out.push(Stroke::Line(p(0.0, 0.0), p(w, d)));
            out.push(Stroke::Line(p(w, 0.0), p(0.0, d)));
        }
        CabinetKind::Soffit | CabinetKind::Shelf | CabinetKind::Partition => {}
    }

    let text = if cabinet.label.is_empty() {
        auto_label(cabinet)
    } else {
        cabinet.label.clone()
    };
    out.push(Stroke::Text {
        at: p(w / 2.0, d / 2.0),
        text,
        height: LABEL_HEIGHT,
        angle: cabinet.angle,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn bbox(points: &[Point]) -> (f64, f64, f64, f64) {
        let xs = points.iter().map(|p| p.x);
        let ys = points.iter().map(|p| p.y);
        (
            xs.clone().fold(f64::MAX, f64::min),
            ys.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
            ys.fold(f64::MIN, f64::max),
        )
    }

    #[test]
    fn outline_is_36_by_24_rectangle() {
        let c = Cabinet::base(36.0);
        let Stroke::Polyline(pts, true) = &plan_symbol(&c)[0] else {
            panic!("first stroke must be the closed outline");
        };
        assert_eq!(bbox(pts), (0.0, 0.0, 36.0, 24.0));
    }

    #[test]
    fn outline_rotates_ninety_degrees() {
        let mut c = Cabinet::base(36.0);
        c.position = Point::new(100.0, 100.0);
        c.angle = FRAC_PI_2;
        let Stroke::Polyline(pts, true) = &plan_symbol(&c)[0] else {
            panic!("first stroke must be the closed outline");
        };
        let (x0, y0, x1, y1) = bbox(pts);
        assert!((x0 - 76.0).abs() < 1e-9 && (x1 - 100.0).abs() < 1e-9);
        assert!((y0 - 100.0).abs() < 1e-9 && (y1 - 136.0).abs() < 1e-9);
        // Back-left corner stays at the position; width runs along +Y.
        assert!(pts[0].dist(Point::new(100.0, 100.0)) < 1e-9);
        assert!(pts[1].dist(Point::new(100.0, 136.0)) < 1e-9);
    }

    #[test]
    fn symbol_parts_per_kind() {
        let base = plan_symbol(&Cabinet::base(24.0));
        // outline, countertop outline, front line, label
        assert_eq!(base.len(), 4);
        let wall = plan_symbol(&Cabinet::wall(30.0));
        // outline, inner offset, two diagonals, label
        assert_eq!(wall.len(), 5);
        let Some(Stroke::Text { text, .. }) = wall.last() else {
            panic!("label last");
        };
        assert_eq!(text, "W3030");
    }
}
