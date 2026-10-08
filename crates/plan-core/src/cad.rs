//! Plain 2D annotation primitives (Chief's CAD tools): lines, arcs, circles,
//! polylines and text. Angles are radians, counter-clockwise from +X.

use crate::geometry::Point;
use crate::model::Id;
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, TAU};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CadItem {
    Line {
        a: Point,
        b: Point,
    },
    /// Counter-clockwise arc from `start_angle` to `end_angle` (radians).
    Arc {
        center: Point,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    Polyline {
        points: Vec<Point>,
        closed: bool,
    },
    /// Text anchored at its bottom-left; `height` in inches, `angle` radians.
    Text {
        pos: Point,
        text: String,
        height: f64,
        angle: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadObject {
    pub id: Id,
    pub layer: String,
    pub item: CadItem,
}

/// Default layer for new CAD items.
pub const DEFAULT_CAD_LAYER: &str = "CAD, Default";

fn min_max(points: impl IntoIterator<Item = Point>) -> (Point, Point) {
    let mut it = points.into_iter();
    let Some(first) = it.next() else {
        return (Point::ZERO, Point::ZERO);
    };
    let (mut lo, mut hi) = (first, first);
    for p in it {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    (lo, hi)
}

/// Is `angle` within the counter-clockwise sweep from `start` to `end`?
fn in_sweep(angle: f64, start: f64, end: f64) -> bool {
    let sweep = (end - start).rem_euclid(TAU);
    let rel = (angle - start).rem_euclid(TAU);
    rel <= sweep
}

/// Rough text width: average glyph is about 0.6 of the text height.
pub const TEXT_WIDTH_FACTOR: f64 = 0.6;

impl CadItem {
    /// Axis-aligned bounds `(min, max)`.
    pub fn bounds(&self) -> (Point, Point) {
        match self {
            CadItem::Line { a, b } => min_max([*a, *b]),
            CadItem::Circle { center, radius } => min_max([
                Point::new(center.x - radius, center.y - radius),
                Point::new(center.x + radius, center.y + radius),
            ]),
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let at =
                    |a: f64| Point::new(center.x + radius * a.cos(), center.y + radius * a.sin());
                let mut pts = vec![at(*start_angle), at(*end_angle)];
                for k in 0..4 {
                    let a = k as f64 * FRAC_PI_2;
                    if in_sweep(a, *start_angle, *end_angle) {
                        pts.push(at(a));
                    }
                }
                min_max(pts)
            }
            CadItem::Polyline { points, .. } => min_max(points.iter().copied()),
            CadItem::Text {
                pos,
                text,
                height,
                angle,
            } => {
                let w = text.chars().count() as f64 * height * TEXT_WIDTH_FACTOR;
                let (c, s) = (angle.cos(), angle.sin());
                let corner =
                    |dx: f64, dy: f64| Point::new(pos.x + dx * c - dy * s, pos.y + dx * s + dy * c);
                min_max([
                    corner(0.0, 0.0),
                    corner(w, 0.0),
                    corner(w, *height),
                    corner(0.0, *height),
                ])
            }
        }
    }
}

impl CadObject {
    pub fn bounds(&self) -> (Point, Point) {
        self.item.bounds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        a.dist(b) < 1e-9
    }

    fn obj(item: CadItem) -> CadObject {
        CadObject {
            id: 1,
            layer: DEFAULT_CAD_LAYER.into(),
            item,
        }
    }

    #[test]
    fn line_and_circle_bounds() {
        let l = obj(CadItem::Line {
            a: Point::new(10.0, -5.0),
            b: Point::new(-3.0, 8.0),
        });
        let (lo, hi) = l.bounds();
        assert!(close(lo, Point::new(-3.0, -5.0)) && close(hi, Point::new(10.0, 8.0)));
        let c = obj(CadItem::Circle {
            center: Point::new(5.0, 5.0),
            radius: 2.0,
        });
        let (lo, hi) = c.bounds();
        assert!(close(lo, Point::new(3.0, 3.0)) && close(hi, Point::new(7.0, 7.0)));
    }

    #[test]
    fn arc_bounds_include_axis_extremes() {
        // Quarter arc 0..90 deg around origin, radius 10.
        let a = obj(CadItem::Arc {
            center: Point::ZERO,
            radius: 10.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        });
        let (lo, hi) = a.bounds();
        assert!(close(lo, Point::new(0.0, 0.0)) && close(hi, Point::new(10.0, 10.0)));
        // Arc crossing +X (350..10 deg) reaches x = 10 although endpoints don't.
        let b = obj(CadItem::Arc {
            center: Point::ZERO,
            radius: 10.0,
            start_angle: 350f64.to_radians(),
            end_angle: 10f64.to_radians(),
        });
        let (_, hi) = b.bounds();
        assert!((hi.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn polyline_and_text_bounds() {
        let p = obj(CadItem::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(4.0, 1.0),
                Point::new(2.0, 9.0),
            ],
            closed: true,
        });
        let (lo, hi) = p.bounds();
        assert!(close(lo, Point::ZERO) && close(hi, Point::new(4.0, 9.0)));
        let t = obj(CadItem::Text {
            pos: Point::new(10.0, 10.0),
            text: "ABCDE".into(),
            height: 5.0,
            angle: 0.0,
        });
        let (lo, hi) = t.bounds();
        assert!(close(lo, Point::new(10.0, 10.0)));
        assert!(close(
            hi,
            Point::new(10.0 + 5.0 * 5.0 * TEXT_WIDTH_FACTOR, 15.0)
        ));
        let empty = obj(CadItem::Polyline {
            points: vec![],
            closed: false,
        });
        assert!(close(empty.bounds().0, Point::ZERO));
    }
}
