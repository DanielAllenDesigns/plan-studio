//! Resolution-independent 2D plan symbols made of strokes.

use plan_core::geometry::Point;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// An axis-aligned bounding box in the symbol's coordinate space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Lower-left corner (smallest x and y).
    pub min: Point,
    /// Upper-right corner (largest x and y).
    pub max: Point,
}

impl Bounds {
    /// Extent along X.
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// Extent along Y.
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Center of the box.
    pub fn center(&self) -> Point {
        Point::lerp(self.min, self.max, 0.5)
    }

    /// Smallest box containing all `points`, or `None` if there are none.
    pub fn from_points<I: IntoIterator<Item = Point>>(points: I) -> Option<Bounds> {
        let mut it = points.into_iter();
        let first = it.next()?;
        let mut b = Bounds {
            min: first,
            max: first,
        };
        for p in it {
            b.min.x = b.min.x.min(p.x);
            b.min.y = b.min.y.min(p.y);
            b.max.x = b.max.x.max(p.x);
            b.max.y = b.max.y.max(p.y);
        }
        Some(b)
    }
}

/// One drawing primitive of a [`Symbol2d`].
///
/// Angles are in degrees, measured counter-clockwise from +X in the symbol's
/// local frame (X right, Y into the room).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Stroke {
    /// A connected run of line segments, optionally closed back to its start.
    Polyline {
        /// Vertices in drawing order.
        points: Vec<Point>,
        /// Whether the last vertex connects back to the first.
        #[serde(default)]
        closed: bool,
    },
    /// A circular arc swept counter-clockwise from `start_deg` to `end_deg`.
    Arc {
        /// Center of the circle the arc lies on.
        center: Point,
        /// Radius in inches.
        radius: f64,
        /// Start angle in degrees.
        start_deg: f64,
        /// End angle in degrees (sweep is `end - start`, wrapped into 0..360;
        /// a sweep of 360 or more is a full circle).
        end_deg: f64,
    },
    /// A full circle.
    Circle {
        /// Center of the circle.
        center: Point,
        /// Radius in inches.
        radius: f64,
    },
}

/// Maps a local point through scale, rotation, then translation.
fn map_point(q: Point, pos: Point, sin: f64, cos: f64, scale: f64) -> Point {
    Point::new(
        scale * (q.x * cos - q.y * sin) + pos.x,
        scale * (q.x * sin + q.y * cos) + pos.y,
    )
}

impl Stroke {
    /// Points that, taken together, bound the stroke exactly: vertices for
    /// polylines, extremes for circles, endpoints plus any axis extremes
    /// swept by an arc.
    fn bounding_points(&self) -> Vec<Point> {
        match self {
            Stroke::Polyline { points, .. } => points.clone(),
            Stroke::Circle { center, radius } => vec![
                Point::new(center.x - radius, center.y - radius),
                Point::new(center.x + radius, center.y + radius),
            ],
            Stroke::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let at = |deg: f64| {
                    let r = deg.to_radians();
                    Point::new(center.x + radius * r.cos(), center.y + radius * r.sin())
                };
                let sweep = if end_deg - start_deg >= 360.0 {
                    360.0
                } else {
                    (end_deg - start_deg).rem_euclid(360.0)
                };
                let mut pts = vec![at(*start_deg), at(start_deg + sweep)];
                for k in 0..4 {
                    let axis = f64::from(k) * 90.0;
                    if (axis - start_deg).rem_euclid(360.0) <= sweep {
                        pts.push(at(axis));
                    }
                }
                pts
            }
        }
    }

    /// Returns this stroke scaled by `scale`, rotated by `angle_rad`
    /// counter-clockwise about the origin, then moved by `pos`.
    pub fn transformed(&self, pos: Point, angle_rad: f64, scale: f64) -> Stroke {
        // A negative scale is a scale by |scale| plus a half turn.
        let (s, a) = if scale < 0.0 {
            (-scale, angle_rad + PI)
        } else {
            (scale, angle_rad)
        };
        let (sin, cos) = a.sin_cos();
        match self {
            Stroke::Polyline { points, closed } => Stroke::Polyline {
                points: points
                    .iter()
                    .map(|&q| map_point(q, pos, sin, cos, s))
                    .collect(),
                closed: *closed,
            },
            Stroke::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let da = a.to_degrees();
                Stroke::Arc {
                    center: map_point(*center, pos, sin, cos, s),
                    radius: radius * s,
                    start_deg: start_deg + da,
                    end_deg: end_deg + da,
                }
            }
            Stroke::Circle { center, radius } => Stroke::Circle {
                center: map_point(*center, pos, sin, cos, s),
                radius: radius * s,
            },
        }
    }
}

/// A 2D plan-view block: a set of strokes in a local frame.
///
/// The frame has X to the right and Y into the room. For wall-mounted items
/// the origin is the back-center on the wall; for everything else it is the
/// center of the item.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Symbol2d {
    /// The primitives making up the symbol.
    pub strokes: Vec<Stroke>,
}

impl Symbol2d {
    /// Wraps a list of strokes.
    pub fn new(strokes: Vec<Stroke>) -> Self {
        Self { strokes }
    }

    /// True when the symbol has no strokes.
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }

    /// Appends all strokes of `other` (already in this symbol's frame).
    pub fn merge(&mut self, other: Symbol2d) {
        self.strokes.extend(other.strokes);
    }

    /// Exact bounding box of every stroke, or `None` for an empty symbol.
    pub fn bounds(&self) -> Option<Bounds> {
        Bounds::from_points(self.strokes.iter().flat_map(Stroke::bounding_points))
    }

    /// Returns a copy scaled by `scale`, rotated by `angle_rad`
    /// counter-clockwise about the origin, then translated by `pos`. This is
    /// how a placed item is mapped from symbol space to plan space.
    pub fn transformed(&self, pos: Point, angle_rad: f64, scale: f64) -> Symbol2d {
        Symbol2d {
            strokes: self
                .strokes
                .iter()
                .map(|s| s.transformed(pos, angle_rad, scale))
                .collect(),
        }
    }

    /// Returns a copy stretched by `sx` along X and `sy` along Y about the
    /// origin (a resized library item). Circles and arcs keep a circular
    /// shape and scale by the geometric mean of the two factors.
    pub fn scaled_xy(&self, sx: f64, sy: f64) -> Symbol2d {
        let p = |q: &Point| Point::new(q.x * sx, q.y * sy);
        let k = (sx * sy).abs().sqrt();
        Symbol2d {
            strokes: self
                .strokes
                .iter()
                .map(|st| match st {
                    Stroke::Polyline { points, closed } => Stroke::Polyline {
                        points: points.iter().map(p).collect(),
                        closed: *closed,
                    },
                    Stroke::Circle { center, radius } => Stroke::Circle {
                        center: p(center),
                        radius: radius * k,
                    },
                    Stroke::Arc {
                        center,
                        radius,
                        start_deg,
                        end_deg,
                    } => Stroke::Arc {
                        center: p(center),
                        radius: radius * k,
                        start_deg: *start_deg,
                        end_deg: *end_deg,
                    },
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_symbol(w: f64, h: f64) -> Symbol2d {
        Symbol2d::new(vec![Stroke::Polyline {
            points: vec![
                Point::new(-w / 2.0, -h / 2.0),
                Point::new(w / 2.0, -h / 2.0),
                Point::new(w / 2.0, h / 2.0),
                Point::new(-w / 2.0, h / 2.0),
            ],
            closed: true,
        }])
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn transformed_rotates_bounds_for_90_degrees() {
        let b0 = rect_symbol(40.0, 20.0).bounds().unwrap();
        assert!(close(b0.width(), 40.0) && close(b0.height(), 20.0));
        let rotated = rect_symbol(40.0, 20.0).transformed(Point::ZERO, PI / 2.0, 1.0);
        let b = rotated.bounds().unwrap();
        assert!(close(b.width(), 20.0), "width {}", b.width());
        assert!(close(b.height(), 40.0), "height {}", b.height());
        assert!(close(b.center().x, 0.0) && close(b.center().y, 0.0));
    }

    #[test]
    fn transformed_scales_and_translates() {
        let s = rect_symbol(10.0, 10.0).transformed(Point::new(100.0, 50.0), 0.0, 2.0);
        let b = s.bounds().unwrap();
        assert!(close(b.min.x, 90.0) && close(b.max.x, 110.0));
        assert!(close(b.min.y, 40.0) && close(b.max.y, 60.0));
    }

    #[test]
    fn negative_scale_is_half_turn() {
        let s = Symbol2d::new(vec![Stroke::Circle {
            center: Point::new(5.0, 0.0),
            radius: 1.0,
        }]);
        let t = s.transformed(Point::ZERO, 0.0, -2.0);
        match &t.strokes[0] {
            Stroke::Circle { center, radius } => {
                assert!(close(center.x, -10.0) && close(center.y, 0.0));
                assert!(close(*radius, 2.0));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn arc_bounds_include_swept_extremes() {
        // Upper half circle: top extreme at (0, 10) is not an endpoint.
        let s = Symbol2d::new(vec![Stroke::Arc {
            center: Point::ZERO,
            radius: 10.0,
            start_deg: 0.0,
            end_deg: 180.0,
        }]);
        let b = s.bounds().unwrap();
        assert!(close(b.max.y, 10.0));
        assert!(b.min.y.abs() < 1e-9);
        assert!(close(b.min.x, -10.0) && close(b.max.x, 10.0));
    }

    #[test]
    fn arc_wraps_through_zero() {
        let s = Symbol2d::new(vec![Stroke::Arc {
            center: Point::ZERO,
            radius: 5.0,
            start_deg: 300.0,
            end_deg: 60.0,
        }]);
        let b = s.bounds().unwrap();
        assert!(close(b.max.x, 5.0), "rightmost point is swept");
        assert!(b.min.x > 0.0, "left half is not swept");
    }

    #[test]
    fn arc_rotates_with_symbol() {
        let s = Symbol2d::new(vec![Stroke::Arc {
            center: Point::new(1.0, 0.0),
            radius: 2.0,
            start_deg: 0.0,
            end_deg: 90.0,
        }]);
        let t = s.transformed(Point::ZERO, PI / 2.0, 1.0);
        match &t.strokes[0] {
            Stroke::Arc {
                center,
                start_deg,
                end_deg,
                ..
            } => {
                assert!(close(center.x, 0.0) && close(center.y, 1.0));
                assert!(close(*start_deg, 90.0) && close(*end_deg, 180.0));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn empty_symbol_has_no_bounds() {
        assert!(Symbol2d::default().bounds().is_none());
    }

    #[test]
    fn stroke_json_is_tagged() {
        let s = Stroke::Circle {
            center: Point::new(1.0, 2.0),
            radius: 3.0,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"type\":\"circle\""), "{json}");
        assert_eq!(serde_json::from_str::<Stroke>(&json).unwrap(), s);
    }

    #[test]
    fn scaled_xy_stretches_the_drawing() {
        let r = rect_symbol(10.0, 20.0).scaled_xy(2.0, 0.5);
        let b = r.bounds().unwrap();
        assert!(close(b.width(), 20.0) && close(b.height(), 10.0));
        let c = Symbol2d::new(vec![Stroke::Circle {
            center: Point::new(1.0, 1.0),
            radius: 4.0,
        }])
        .scaled_xy(4.0, 1.0);
        match &c.strokes[0] {
            Stroke::Circle { center, radius } => {
                assert!(close(center.x, 4.0) && close(center.y, 1.0));
                assert!(close(*radius, 8.0));
            }
            _ => unreachable!(),
        }
    }
}
