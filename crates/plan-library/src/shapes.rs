//! Small constructors for the strokes used by the built-in symbols.
//!
//! Coordinates are plain `(x, y)` tuples in inches to keep the symbol
//! definitions compact.

use crate::symbol::Stroke;
use plan_core::geometry::Point;

fn pt((x, y): (f64, f64)) -> Point {
    Point::new(x, y)
}

/// A single line segment.
pub(crate) fn line(a: (f64, f64), b: (f64, f64)) -> Stroke {
    polyline(&[a, b], false)
}

/// A polyline through `pts`.
pub(crate) fn polyline(pts: &[(f64, f64)], closed: bool) -> Stroke {
    Stroke::Polyline {
        points: pts.iter().copied().map(pt).collect(),
        closed,
    }
}

/// A closed axis-aligned rectangle from corner `(x0, y0)` to `(x1, y1)`.
pub(crate) fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Stroke {
    polyline(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true)
}

/// A circle.
pub(crate) fn circle(cx: f64, cy: f64, r: f64) -> Stroke {
    Stroke::Circle {
        center: Point::new(cx, cy),
        radius: r,
    }
}

/// A counter-clockwise arc from `start` to `end` degrees.
pub(crate) fn arc(cx: f64, cy: f64, r: f64, start: f64, end: f64) -> Stroke {
    Stroke::Arc {
        center: Point::new(cx, cy),
        radius: r,
        start_deg: start,
        end_deg: end,
    }
}

/// A rectangle with corner radius `r` (clamped to fit), as four lines and
/// four arcs. Falls back to a plain [`rect`] when `r` is not positive.
pub(crate) fn rounded_rect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Vec<Stroke> {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    if r <= 1e-9 {
        return vec![rect(x0, y0, x1, y1)];
    }
    let mut v = Vec::with_capacity(8);
    let mut side = |a: (f64, f64), b: (f64, f64)| {
        if (a.0 - b.0).abs() > 1e-9 || (a.1 - b.1).abs() > 1e-9 {
            v.push(line(a, b));
        }
    };
    side((x0 + r, y0), (x1 - r, y0));
    side((x1, y0 + r), (x1, y1 - r));
    side((x1 - r, y1), (x0 + r, y1));
    side((x0, y1 - r), (x0, y0 + r));
    v.push(arc(x1 - r, y0 + r, r, 270.0, 360.0));
    v.push(arc(x1 - r, y1 - r, r, 0.0, 90.0));
    v.push(arc(x0 + r, y1 - r, r, 90.0, 180.0));
    v.push(arc(x0 + r, y0 + r, r, 180.0, 270.0));
    v
}

/// A rounded rectangle given by center and size.
pub(crate) fn rounded_rect_c(cx: f64, cy: f64, w: f64, h: f64, r: f64) -> Vec<Stroke> {
    rounded_rect(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0, r)
}

/// A "door swing" mark: two lines meeting at `hinge`, running to the two
/// free corners `a` and `b` of the door.
pub(crate) fn door_swing(hinge: (f64, f64), a: (f64, f64), b: (f64, f64)) -> Stroke {
    polyline(&[a, hinge, b], false)
}

/// A letter "S" drawn from two arcs, `4 * r` tall, centered on `(cx, cy)`.
pub(crate) fn s_glyph(cx: f64, cy: f64, r: f64) -> Vec<Stroke> {
    vec![
        arc(cx, cy + r, r, 90.0, 270.0),
        arc(cx, cy - r, r, 270.0, 450.0),
    ]
}

/// A numeral "3" drawn from two right-bulging arcs, `4 * r` tall.
pub(crate) fn three_glyph(cx: f64, cy: f64, r: f64) -> Vec<Stroke> {
    vec![
        arc(cx, cy + r, r, -90.0, 90.0),
        arc(cx, cy - r, r, -90.0, 90.0),
    ]
}
