//! The 2D plan symbol of a stair: outline, risers, direction arrow, UP label
//! and the break line Chief draws where the floor above cuts the flight.

use crate::layout::{Flight, Layout, Uv};
use crate::Stair;
use plan_core::Point;

/// Radius of the circle at the foot of the direction arrow.
const CIRCLE_RADIUS: f64 = 2.5;
/// Distance of that circle's centre from the first riser.
const CIRCLE_ALONG: f64 = 4.0;
/// Arrowhead length.
const ARROW_HEAD: f64 = 6.0;
/// Plan text height for the UP label.
const TEXT_HEIGHT: f64 = 6.0;
/// Half the zigzag amplitude of the break line.
const ZIGZAG: f64 = 3.0;

/// A plan-view drawing primitive (inches, plan Y-up).
#[derive(Debug, Clone, PartialEq)]
pub enum Stroke {
    /// A straight segment.
    Line(Point, Point),
    /// A polyline; the flag closes it into a polygon.
    Polyline(Vec<Point>, bool),
    /// A circular arc, counter-clockwise from `start_deg` to `end_deg`.
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    /// Text anchored at its baseline start, rotated by `angle` degrees.
    Text {
        pos: Point,
        text: String,
        height: f64,
        angle: f64,
    },
}

/// Draw the stair in plan.
///
/// `cut_at` is the fraction (0..1) of the total flight length, landings
/// excluded, at which the floor above cuts the stair. At that point the stair
/// is truncated and a zigzag break line is drawn; `Some(2.0 / 3.0)` matches
/// Chief's default. `None` draws the full run.
pub fn plan_symbol(stair: &Stair, cut_at: Option<f64>) -> Vec<Stroke> {
    let layout = Layout::build(stair);
    let to_plan = |p: Uv| layout.frame.uv(p);
    let flights = &layout.flights;
    let cut = locate_cut(flights, cut_at);
    let last = cut.map_or(flights.len() - 1, |(i, _)| i);
    let mut out = Vec::new();

    for (i, f) in flights.iter().enumerate().take(last + 1) {
        let clipped = cut.filter(|&(ci, _)| ci == i).map(|(_, s)| s);
        let len = clipped.unwrap_or(f.len);
        for j in 0..f.risers {
            let s = f64::from(j) * layout.tread_depth;
            let keep = if clipped.is_some() {
                s < len - 1e-9
            } else {
                true
            };
            if keep {
                out.push(Stroke::Line(
                    to_plan(f.at(s, 0.0)),
                    to_plan(f.at(s, f.width)),
                ));
            }
        }
        let corners = [
            f.at(0.0, 0.0),
            f.at(len, 0.0),
            f.at(len, f.width),
            f.at(0.0, f.width),
        ];
        if clipped.is_some() {
            let open = [corners[1], corners[0], corners[3], corners[2]];
            out.push(Stroke::Polyline(open.map(to_plan).to_vec(), false));
            out.push(break_line(f, len, &to_plan));
        } else if f.len > 1e-9 {
            out.push(Stroke::Polyline(corners.map(to_plan).to_vec(), true));
        }
    }

    if last >= 1 {
        for outline in &layout.outlines {
            out.push(Stroke::Polyline(
                outline.iter().map(|&p| to_plan(p)).collect(),
                true,
            ));
        }
        for r in &layout.turn_risers {
            out.push(Stroke::Line(to_plan(r.a), to_plan(r.b)));
        }
    }

    out.extend(direction_arrow(stair, &layout, last, cut.map(|(_, s)| s)));
    out
}

/// Map the cut fraction to `(flight index, distance along that flight)`.
fn locate_cut(flights: &[Flight], cut_at: Option<f64>) -> Option<(usize, f64)> {
    let total: f64 = flights.iter().map(|f| f.len).sum();
    let fraction = cut_at.filter(|f| f.is_finite())?.clamp(0.0, 1.0);
    if total <= 0.0 || fraction >= 1.0 {
        return None;
    }
    let mut remaining = fraction * total;
    for (i, f) in flights.iter().enumerate() {
        if remaining <= f.len {
            return Some((i, remaining));
        }
        remaining -= f.len;
    }
    None
}

/// A zigzag across the flight at distance `at` from its first riser.
fn break_line(f: &Flight, at: f64, to_plan: &impl Fn(Uv) -> Point) -> Stroke {
    let w = f.width;
    let pts = [
        (at, 0.0),
        (at, 0.4 * w),
        (at + ZIGZAG, 0.45 * w),
        (at - ZIGZAG, 0.55 * w),
        (at, 0.6 * w),
        (at, w),
    ];
    Stroke::Polyline(
        pts.iter().map(|&(s, l)| to_plan(f.at(s, l))).collect(),
        false,
    )
}

/// Circle at the bottom, centreline with arrowhead at the top, and the UP label.
fn direction_arrow(stair: &Stair, layout: &Layout, last: usize, cut: Option<f64>) -> Vec<Stroke> {
    let to_plan = |p: Uv| layout.frame.uv(p);
    let flights = &layout.flights;
    let f0 = &flights[0];
    let mid = f0.width / 2.0;
    let circle_along = CIRCLE_ALONG.min(f0.len / 2.0);
    let mut out = vec![Stroke::Arc {
        center: to_plan(f0.at(circle_along, mid)),
        radius: CIRCLE_RADIUS,
        start_deg: 0.0,
        end_deg: 360.0,
    }];

    let mut path = vec![f0.at(circle_along + CIRCLE_RADIUS, mid)];
    for (i, f) in flights.iter().enumerate().take(last + 1) {
        let end = if i == last {
            cut.unwrap_or(f.len)
        } else {
            f.len
        };
        let exit = f.at(f.len, mid);
        path.push(f.at(end, mid));
        if i < last {
            let next = &flights[i + 1];
            let entry = next.at(0.0, next.width / 2.0);
            if f.dir.0 * next.dir.0 + f.dir.1 * next.dir.1 == 0.0 {
                // Quarter turn: meet where the two centrelines cross.
                path.push(if f.dir.1 == 0.0 {
                    (entry.0, exit.1)
                } else {
                    (exit.0, entry.1)
                });
            } else {
                // Half turn: cross the landing at its middle.
                let half = landing_extent(layout, f.dir) / 2.0;
                path.push((exit.0 + f.dir.0 * half, exit.1 + f.dir.1 * half));
                path.push((entry.0 + f.dir.0 * half, entry.1 + f.dir.1 * half));
            }
            path.push(entry);
        }
    }

    if let [.., a, b] = path[..] {
        let seg = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        if seg > 1e-6 && path.len() > 1 {
            out.push(Stroke::Polyline(
                path.iter().map(|&p| to_plan(p)).collect(),
                false,
            ));
            let dir = ((b.0 - a.0) / seg, (b.1 - a.1) / seg);
            let head = ARROW_HEAD.min(seg);
            let back = (b.0 - dir.0 * head, b.1 - dir.1 * head);
            let side = (-dir.1 * head * 0.45, dir.0 * head * 0.45);
            let barbs = [
                b,
                (back.0 + side.0, back.1 + side.1),
                (back.0 - side.0, back.1 - side.1),
            ];
            out.push(Stroke::Polyline(barbs.map(to_plan).to_vec(), true));
        }
    }

    let label_lateral = (mid + CIRCLE_RADIUS + 0.5 * TEXT_HEIGHT + 1.0).min(f0.width);
    out.push(Stroke::Text {
        pos: to_plan(f0.at(circle_along + CIRCLE_RADIUS + 3.0, label_lateral)),
        text: "UP".into(),
        height: TEXT_HEIGHT,
        angle: stair.direction.to_degrees().rem_euclid(360.0),
    });
    out
}

/// Length of the landing outline measured along `dir`.
fn landing_extent(layout: &Layout, dir: Uv) -> f64 {
    let Some(outline) = layout.outlines.first() else {
        return 0.0;
    };
    let proj = |p: &Uv| p.0 * dir.0 + p.1 * dir.1;
    let max = outline.iter().map(proj).fold(f64::MIN, f64::max);
    let min = outline.iter().map(proj).fold(f64::MAX, f64::min);
    max - min
}
