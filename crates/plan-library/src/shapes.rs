//! Small constructors for the strokes used by the built-in symbols.
//!
//! Coordinates are plain `(x, y)` tuples in inches to keep the symbol
//! definitions compact.

use crate::symbol::Stroke;
use plan_core::geometry::Point;
use std::f64::consts::{PI, TAU};

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

/// The point at `deg` degrees (counter-clockwise from +X) on the circle of
/// radius `r` about `(cx, cy)`.
pub(crate) fn polar(cx: f64, cy: f64, r: f64, deg: f64) -> (f64, f64) {
    let a = deg.to_radians();
    (cx + r * a.cos(), cy + r * a.sin())
}

/// A closed polyline approximating an ellipse with `n` vertices. When `n` is
/// a multiple of four the axis extremes are hit exactly.
pub(crate) fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64, n: u32) -> Stroke {
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let a = TAU * f64::from(k) / f64::from(n);
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect();
    polyline(&pts, true)
}

/// A circle with `lobes` rounded bumps (a tree-canopy outline). The bump
/// tips lie on radius `r`; the notches between them are `depth` inside it.
/// A multiple of four lobes puts tips exactly on the axes.
pub(crate) fn scalloped_circle(cx: f64, cy: f64, r: f64, lobes: u32, depth: f64) -> Stroke {
    let n = lobes * 6;
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let a = TAU * f64::from(k) / f64::from(n);
            let rad = r - depth + depth * (a * f64::from(lobes) / 2.0).cos().abs();
            (cx + rad * a.cos(), cy + rad * a.sin())
        })
        .collect();
    polyline(&pts, true)
}

/// A spiky star outline with `tips` points reaching `r_out`, notches at
/// `r_in`. The first tip points along +X.
pub(crate) fn star(cx: f64, cy: f64, r_out: f64, r_in: f64, tips: u32) -> Stroke {
    let n = tips * 2;
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let a = TAU * f64::from(k) / f64::from(n);
            let rad = if k % 2 == 0 { r_out } else { r_in };
            (cx + rad * a.cos(), cy + rad * a.sin())
        })
        .collect();
    polyline(&pts, true)
}

/// A `w` by `h` rectangle centered on `(cx, cy)` whose two long edges bulge
/// in `bumps` scallops of height `amp` (a clipped-hedge outline).
pub(crate) fn wavy_rect(cx: f64, cy: f64, w: f64, h: f64, bumps: u32, amp: f64) -> Stroke {
    let steps = bumps * 4;
    let offset = |i: u32| amp * (1.0 - (PI * f64::from(i % 4) / 4.0).sin());
    let x_at = |i: u32| cx - w / 2.0 + w * f64::from(i) / f64::from(steps);
    let mut pts: Vec<(f64, f64)> = (0..=steps)
        .map(|i| (x_at(i), cy - h / 2.0 + offset(i)))
        .collect();
    pts.extend(
        (0..=steps)
            .rev()
            .map(|i| (x_at(i), cy + h / 2.0 - offset(i))),
    );
    polyline(&pts, true)
}

/// Rescales `pts` so their bounding box is exactly `w` by `h` centered on
/// `(cx, cy)`, then returns them as a polyline. Lets free-form outlines
/// (boulders, pools, pianos) match their declared size.
pub(crate) fn fitted(pts: &[(f64, f64)], cx: f64, cy: f64, w: f64, h: f64, closed: bool) -> Stroke {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in pts {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let mapped: Vec<(f64, f64)> = pts
        .iter()
        .map(|&(x, y)| {
            (
                cx + (x - x0) / (x1 - x0) * w - w / 2.0,
                cy + (y - y0) / (y1 - y0) * h - h / 2.0,
            )
        })
        .collect();
    polyline(&mapped, closed)
}

/// An irregular rounded blob (planting patch) with `k` wobbles, fitted to
/// exactly `2*rx` by `2*ry` about `(cx, cy)`.
pub(crate) fn blob(cx: f64, cy: f64, rx: f64, ry: f64, k: f64, phase: f64) -> Stroke {
    let pts: Vec<(f64, f64)> = (0..36)
        .map(|i| {
            let a = TAU * f64::from(i) / 36.0;
            let rad =
                1.0 + 0.16 * (k * a + phase).sin() + 0.07 * ((k + 2.0) * a + 2.1 * phase).sin();
            (rad * a.cos(), rad * a.sin())
        })
        .collect();
    fitted(&pts, cx, cy, 2.0 * rx, 2.0 * ry, true)
}
