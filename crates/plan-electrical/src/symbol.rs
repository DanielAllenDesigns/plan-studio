//! Plan symbols: tiny vector drawings in device-local space.
//!
//! Local space is inches with the origin at the device position, **+X pointing
//! the way the device faces** (out of the wall, into the room) and +Y along the
//! wall. [`Device::symbol_world`](crate::Device::symbol_world) rotates and
//! translates the strokes into plan space.

use crate::device::DeviceKind;
use plan_core::Point;
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI};

/// One drawing primitive of a plan symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stroke {
    /// Straight segment.
    Line { a: Point, b: Point },
    /// Connected segments, optionally closed and/or filled.
    Polyline {
        points: Vec<Point>,
        closed: bool,
        filled: bool,
    },
    /// Circular arc: `start` angle and signed `sweep` in radians (CCW positive).
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        sweep: f64,
    },
    /// Circle, outlined or filled.
    Circle {
        center: Point,
        radius: f64,
        filled: bool,
    },
    /// A text label anchored at its center; text is never rotated with the symbol.
    Text {
        at: Point,
        text: String,
        height: f64,
    },
}

fn xform(p: Point, origin: Point, angle: f64) -> Point {
    let (s, c) = angle.sin_cos();
    Point::new(origin.x + p.x * c - p.y * s, origin.y + p.x * s + p.y * c)
}

impl Stroke {
    /// Scale about the local origin by `k`.
    pub fn scaled(&self, k: f64) -> Stroke {
        let sp = |p: &Point| Point::new(p.x * k, p.y * k);
        match self {
            Stroke::Line { a, b } => Stroke::Line { a: sp(a), b: sp(b) },
            Stroke::Polyline {
                points,
                closed,
                filled,
            } => Stroke::Polyline {
                points: points.iter().map(sp).collect(),
                closed: *closed,
                filled: *filled,
            },
            Stroke::Arc {
                center,
                radius,
                start,
                sweep,
            } => Stroke::Arc {
                center: sp(center),
                radius: radius * k,
                start: *start,
                sweep: *sweep,
            },
            Stroke::Circle {
                center,
                radius,
                filled,
            } => Stroke::Circle {
                center: sp(center),
                radius: radius * k,
                filled: *filled,
            },
            Stroke::Text { at, text, height } => Stroke::Text {
                at: sp(at),
                text: text.clone(),
                height: height * k,
            },
        }
    }

    /// Rotate by `angle` radians about the local origin, then translate to `origin`.
    pub fn transformed(&self, origin: Point, angle: f64) -> Stroke {
        match self {
            Stroke::Line { a, b } => Stroke::Line {
                a: xform(*a, origin, angle),
                b: xform(*b, origin, angle),
            },
            Stroke::Polyline {
                points,
                closed,
                filled,
            } => Stroke::Polyline {
                points: points.iter().map(|p| xform(*p, origin, angle)).collect(),
                closed: *closed,
                filled: *filled,
            },
            Stroke::Arc {
                center,
                radius,
                start,
                sweep,
            } => Stroke::Arc {
                center: xform(*center, origin, angle),
                radius: *radius,
                start: start + angle,
                sweep: *sweep,
            },
            Stroke::Circle {
                center,
                radius,
                filled,
            } => Stroke::Circle {
                center: xform(*center, origin, angle),
                radius: *radius,
                filled: *filled,
            },
            Stroke::Text { at, text, height } => Stroke::Text {
                at: xform(*at, origin, angle),
                text: text.clone(),
                height: *height,
            },
        }
    }
}

fn line(ax: f64, ay: f64, bx: f64, by: f64) -> Stroke {
    Stroke::Line {
        a: Point::new(ax, ay),
        b: Point::new(bx, by),
    }
}

fn circle(cx: f64, cy: f64, radius: f64, filled: bool) -> Stroke {
    Stroke::Circle {
        center: Point::new(cx, cy),
        radius,
        filled,
    }
}

fn text(x: f64, y: f64, s: &str, height: f64) -> Stroke {
    Stroke::Text {
        at: Point::new(x, y),
        text: s.to_string(),
        height,
    }
}

fn poly(pts: &[(f64, f64)], closed: bool, filled: bool) -> Stroke {
    Stroke::Polyline {
        points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        closed,
        filled,
    }
}

/// Circle with two parallel lines: the standard duplex receptacle.
fn duplex(radius: f64) -> Vec<Stroke> {
    vec![
        circle(0.0, 0.0, radius, false),
        line(-1.0, -2.0, -1.0, 2.0),
        line(1.0, -2.0, 1.0, 2.0),
    ]
}

/// Switch: a stem leaving the wall with an "S" glyph at its end.
fn switch_base() -> Vec<Stroke> {
    // Glyph drawn in (u, v) space, then mapped to local x = 6 + v, y = u.
    const S: [(f64, f64); 12] = [
        (1.2, 1.4),
        (0.4, 2.0),
        (-0.8, 2.0),
        (-1.5, 1.3),
        (-1.5, 0.5),
        (-0.8, 0.0),
        (0.8, 0.0),
        (1.5, -0.5),
        (1.5, -1.3),
        (0.8, -2.0),
        (-0.4, -2.0),
        (-1.2, -1.4),
    ];
    let pts: Vec<(f64, f64)> = S.iter().map(|&(u, v)| (6.0 + v, u)).collect();
    vec![line(0.0, 0.0, 4.0, 0.0), poly(&pts, false, false)]
}

/// A fan blade: a thin rectangle from the hub outwards, rotated by `angle`.
fn blade(angle: f64) -> Stroke {
    let (s, c) = angle.sin_cos();
    let corners = [(2.0, -2.5), (14.0, -2.5), (14.0, 2.5), (2.0, 2.5)];
    let pts: Vec<(f64, f64)> = corners
        .iter()
        .map(|&(x, y)| (x * c - y * s, x * s + y * c))
        .collect();
    poly(&pts, true, false)
}

/// Low-voltage jack: a triangle pointing into the room with its letters beside it.
fn jack(letters: &str) -> Vec<Stroke> {
    vec![
        poly(&[(0.0, -3.0), (0.0, 3.0), (5.0, 0.0)], true, false),
        text(8.0, 0.0, letters, 2.5),
    ]
}

impl DeviceKind {
    /// The plan symbol in device-local coordinates (see the module docs).
    pub fn symbol(&self) -> Vec<Stroke> {
        match self {
            DeviceKind::Outlet110 => duplex(3.0),
            DeviceKind::Outlet110Quad => vec![
                circle(0.0, 0.0, 3.5, false),
                line(-1.2, -2.6, -1.2, 2.6),
                line(1.2, -2.6, 1.2, 2.6),
                line(-2.6, -1.2, 2.6, -1.2),
                line(-2.6, 1.2, 2.6, 1.2),
            ],
            DeviceKind::DataJack => jack("D"),
            DeviceKind::PhoneJack => jack("T"),
            DeviceKind::TvJack => jack("TV"),
            DeviceKind::Switch4Way => {
                let mut s = switch_base();
                s.push(text(6.0, 4.5, "4", 2.5));
                s
            }
            DeviceKind::Gfci => {
                let mut s = duplex(3.0);
                s.push(line(0.0, 3.0, 0.0, 4.5));
                s.push(line(0.0, -3.0, 0.0, -4.5));
                s.push(text(6.5, 0.0, "GFCI", 2.5));
                s
            }
            DeviceKind::Outlet220 => vec![
                circle(0.0, 0.0, 3.5, false),
                line(-1.6, -2.5, -1.6, 2.5),
                line(0.0, -3.2, 0.0, 3.2),
                line(1.6, -2.5, 1.6, 2.5),
                text(7.5, 0.0, "220V", 2.5),
            ],
            // Weatherproof GFCI: the GFCI duplex with a WP tag.
            DeviceKind::OutletWp => {
                let mut s = duplex(3.0);
                s.push(line(0.0, 3.0, 0.0, 4.5));
                s.push(line(0.0, -3.0, 0.0, -4.5));
                s.push(text(6.5, 0.0, "WP", 2.5));
                s
            }
            // Dedicated: a single receptacle (one blade line, solid hub) tagged DED.
            DeviceKind::OutletDedicated => vec![
                circle(0.0, 0.0, 3.0, false),
                line(0.0, -2.0, 0.0, 2.0),
                circle(0.0, 0.0, 0.8, true),
                text(6.5, 0.0, "DED", 2.5),
            ],
            DeviceKind::OutletFloor => {
                let mut s = duplex(3.0);
                s.push(poly(
                    &[(-4.0, -4.0), (4.0, -4.0), (4.0, 4.0), (-4.0, 4.0)],
                    true,
                    false,
                ));
                s
            }
            DeviceKind::Switch => switch_base(),
            // Weatherproof switch: the switch with a WP tag (verify in Chief).
            DeviceKind::SwitchWp => {
                let mut s = switch_base();
                s.push(text(6.0, 4.5, "WP", 2.5));
                s
            }
            DeviceKind::Switch3Way => {
                let mut s = switch_base();
                s.push(text(6.0, 4.5, "3", 2.5));
                s
            }
            DeviceKind::SwitchDimmer => {
                let mut s = switch_base();
                s.push(text(6.0, 4.5, "D", 2.5));
                s
            }
            DeviceKind::CeilingLight => vec![
                circle(0.0, 0.0, 6.0, false),
                line(-4.24, -4.24, 4.24, 4.24),
                line(-4.24, 4.24, 4.24, -4.24),
            ],
            DeviceKind::RecessedCan => {
                vec![circle(0.0, 0.0, 4.0, false), circle(0.0, 0.0, 1.0, true)]
            }
            DeviceKind::PendantLight => {
                vec![circle(0.0, 0.0, 5.0, false), circle(0.0, 0.0, 2.0, true)]
            }
            DeviceKind::WallSconce => vec![
                Stroke::Arc {
                    center: Point::ZERO,
                    radius: 4.0,
                    start: -FRAC_PI_2,
                    sweep: PI,
                },
                line(0.0, -4.0, 0.0, 4.0),
            ],
            // Exterior wall light: the sconce's half circle with a WP tag
            // (verify in Chief).
            DeviceKind::WallLightExterior => vec![
                Stroke::Arc {
                    center: Point::ZERO,
                    radius: 4.0,
                    start: -FRAC_PI_2,
                    sweep: PI,
                },
                line(0.0, -4.0, 0.0, 4.0),
                text(7.0, 0.0, "WP", 2.0),
            ],
            // Path light: a small circle with four short rays.
            DeviceKind::PathLight => vec![
                circle(0.0, 0.0, 2.0, false),
                circle(0.0, 0.0, 0.7, true),
                line(3.0, 0.0, 5.0, 0.0),
                line(-3.0, 0.0, -5.0, 0.0),
                line(0.0, 3.0, 0.0, 5.0),
                line(0.0, -3.0, 0.0, -5.0),
            ],
            DeviceKind::CeilingFan => {
                let mut s = vec![circle(0.0, 0.0, 2.0, true)];
                s.extend((0..4).map(|k| blade(f64::from(k) * FRAC_PI_2)));
                s
            }
            DeviceKind::SmokeDetector => {
                vec![circle(0.0, 0.0, 4.5, false), text(0.0, 0.0, "SD", 2.5)]
            }
            DeviceKind::CoDetector => vec![circle(0.0, 0.0, 4.5, false), text(0.0, 0.0, "CO", 2.5)],
            DeviceKind::Thermostat => vec![circle(0.0, 0.0, 3.0, false), text(0.0, 0.0, "T", 2.5)],
            DeviceKind::Doorbell => vec![circle(0.0, 0.0, 2.5, false), text(4.5, 0.0, "DB", 2.0)],
            DeviceKind::Panel => vec![
                poly(
                    &[(0.0, -7.0), (4.0, -7.0), (4.0, 7.0), (0.0, 7.0)],
                    true,
                    true,
                ),
                text(7.0, 0.0, "P", 3.0),
            ],
            DeviceKind::RopeLight { length } => vec![
                line(0.0, 0.0, 0.0, *length),
                circle(0.0, 0.0, 0.75, true),
                circle(0.0, *length, 0.75, true),
                text(2.5, length * 0.5, "RL", 2.0),
            ],
        }
    }
}
