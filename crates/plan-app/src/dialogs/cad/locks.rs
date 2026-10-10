//! The Lock choices of the Line and Arc Specification dialogs (CAD-116,
//! manual pp. 332 and 339).
//!
//! A lock says which part of the line or arc stays where it is while a value
//! changes. [`line_edit`] and [`arc_edit`] take the shape before the edit,
//! the lock and the one value the user changed, and answer the shape after.
//! A value the lock freezes is not available in the dialog; asking for it
//! anyway leaves the shape alone.

use plan_core::geometry::Point;
use std::f64::consts::TAU;

/// What a line keeps fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineLock {
    /// The start stays; the length, angle and end move the end.
    #[default]
    Start,
    /// The end stays; the length, angle and start move the start.
    End,
    /// The center stays; the length grows both ways and the angle turns the
    /// line about it.
    Center,
    /// The length and angle stay; moving one end moves the other with it.
    LengthAngle,
}

impl LineLock {
    pub const ALL: [LineLock; 4] = [
        LineLock::Start,
        LineLock::End,
        LineLock::Center,
        LineLock::LengthAngle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LineLock::Start => "Start",
            LineLock::End => "End",
            LineLock::Center => "Center",
            LineLock::LengthAngle => "Length/Angle",
        }
    }

    /// Can the Start Point boxes be used?
    pub fn start_free(self) -> bool {
        !matches!(self, LineLock::Start | LineLock::Center)
    }

    /// Can the End Point boxes be used?
    pub fn end_free(self) -> bool {
        self != LineLock::End
    }

    /// Can the Length and Angle boxes be used?
    pub fn length_angle_free(self) -> bool {
        self != LineLock::LengthAngle
    }
}

/// The one value of a line the user changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineEdit {
    Start(Point),
    End(Point),
    Length(f64),
    /// Degrees counter-clockwise from east.
    Angle(f64),
}

fn polar(len: f64, deg: f64) -> Point {
    let a = deg.to_radians();
    Point::new(a.cos(), a.sin()).scale(len)
}

/// The line `(a, b)` after `edit` under `lock`.
pub fn line_edit(a: Point, b: Point, lock: LineLock, edit: LineEdit) -> (Point, Point) {
    let len = a.dist(b);
    let deg = if len > 1e-9 {
        b.sub(a).angle().to_degrees()
    } else {
        0.0
    };
    let c = Point::lerp(a, b, 0.5);
    match edit {
        LineEdit::Start(p) => match lock {
            LineLock::End => (p, b),
            LineLock::LengthAngle => (p, b.add(p.sub(a))),
            LineLock::Start | LineLock::Center => (a, b),
        },
        LineEdit::End(p) => match lock {
            LineLock::Start => (a, p),
            LineLock::Center => (c.add(c.sub(p)), p),
            LineLock::LengthAngle => (a.add(p.sub(b)), p),
            LineLock::End => (a, b),
        },
        LineEdit::Length(l) | LineEdit::Angle(l) if lock == LineLock::LengthAngle || l.is_nan() => {
            (a, b)
        }
        LineEdit::Length(l) => {
            if l <= 0.0 {
                return (a, b);
            }
            match lock {
                LineLock::Start => (a, a.add(polar(l, deg))),
                LineLock::End => (b.sub(polar(l, deg)), b),
                _ => (c.sub(polar(l / 2.0, deg)), c.add(polar(l / 2.0, deg))),
            }
        }
        LineEdit::Angle(d) => match lock {
            LineLock::Start => (a, a.add(polar(len, d))),
            LineLock::End => (b.sub(polar(len, d)), b),
            _ => (c.sub(polar(len / 2.0, d)), c.add(polar(len / 2.0, d))),
        },
    }
}

/// What an arc keeps fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArcLock {
    /// The start point stays.
    Start,
    /// The end point stays.
    End,
    /// The center stays.
    #[default]
    Center,
    /// The arc itself stays; only its location changes.
    Arc,
    /// The chord (start and end points) stays when the radius changes.
    Chord,
}

impl ArcLock {
    pub const ALL: [ArcLock; 5] = [
        ArcLock::Start,
        ArcLock::End,
        ArcLock::Center,
        ArcLock::Arc,
        ArcLock::Chord,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ArcLock::Start => "Start",
            ArcLock::End => "End",
            ArcLock::Center => "Center",
            ArcLock::Arc => "Arc",
            ArcLock::Chord => "Chord",
        }
    }

    /// Can the center be moved by number?
    pub fn center_free(self) -> bool {
        self != ArcLock::Center
    }

    /// Can the radius be changed?
    pub fn radius_free(self) -> bool {
        self != ArcLock::Arc
    }

    /// Can the start and end angles be changed?
    pub fn angles_free(self) -> bool {
        !matches!(self, ArcLock::Arc | ArcLock::Chord)
    }

    /// Can the chord length and angle be typed?
    pub fn chord_free(self) -> bool {
        !matches!(self, ArcLock::Arc | ArcLock::Chord)
    }
}

/// An arc as the model keeps it: counter-clockwise from `a0` to `a1` (radians).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcShape {
    pub center: Point,
    pub radius: f64,
    pub a0: f64,
    pub a1: f64,
}

impl ArcShape {
    pub fn at(&self, angle: f64) -> Point {
        self.center
            .add(Point::new(angle.cos(), angle.sin()).scale(self.radius))
    }

    pub fn start(&self) -> Point {
        self.at(self.a0)
    }

    pub fn end(&self) -> Point {
        self.at(self.a1)
    }

    pub fn sweep(&self) -> f64 {
        (self.a1 - self.a0).rem_euclid(TAU)
    }
}

/// The one value of an arc the user changed (angles in radians).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArcEdit {
    Center(Point),
    Radius(f64),
    StartAngle(f64),
    EndAngle(f64),
    /// Straight distance from start to end (inches).
    ChordLength(f64),
    /// Direction of the chord (radians, counter-clockwise from east).
    ChordAngle(f64),
    /// Length along the arc (inches).
    ArcLength(f64),
}

/// The arc after `edit` under `lock`.
pub fn arc_edit(arc: ArcShape, lock: ArcLock, edit: ArcEdit) -> ArcShape {
    let ArcShape {
        center,
        radius,
        a0,
        a1,
    } = arc;
    let dir = |a: f64| Point::new(a.cos(), a.sin());
    match edit {
        ArcEdit::Center(c) => match lock {
            ArcLock::Center => arc,
            // Moving the center with the arc fixed moves the whole arc.
            ArcLock::Arc => ArcShape { center: c, ..arc },
            ArcLock::Start => {
                let s = arc.start();
                let r = s.dist(c);
                if r < 1e-6 {
                    return arc;
                }
                ArcShape {
                    center: c,
                    radius: r,
                    a0: s.sub(c).angle(),
                    a1,
                }
            }
            ArcLock::End => {
                let e = arc.end();
                let r = e.dist(c);
                if r < 1e-6 {
                    return arc;
                }
                ArcShape {
                    center: c,
                    radius: r,
                    a0,
                    a1: e.sub(c).angle(),
                }
            }
            ArcLock::Chord => {
                // The center stays on the bisector of the chord.
                let (s, e) = (arc.start(), arc.end());
                let mid = Point::lerp(s, e, 0.5);
                let chord = e.sub(s);
                if chord.length() < 1e-6 {
                    return arc;
                }
                let n = chord.normalized().perp();
                let on = mid.add(n.scale(n.dot(c.sub(mid))));
                let r = on.dist(s);
                ArcShape {
                    center: on,
                    radius: r,
                    a0: s.sub(on).angle(),
                    a1: e.sub(on).angle(),
                }
            }
        },
        ArcEdit::Radius(r) => {
            if r <= 1e-6 || !arc.radius.is_finite() {
                return arc;
            }
            match lock {
                ArcLock::Arc => arc,
                ArcLock::Center => ArcShape { radius: r, ..arc },
                // The start stays and the center slides along the radius
                // through it.
                ArcLock::Start => ArcShape {
                    center: arc.start().sub(dir(a0).scale(r)),
                    radius: r,
                    ..arc
                },
                ArcLock::End => ArcShape {
                    center: arc.end().sub(dir(a1).scale(r)),
                    radius: r,
                    ..arc
                },
                ArcLock::Chord => {
                    let (s, e) = (arc.start(), arc.end());
                    let half = s.dist(e) / 2.0;
                    if half < 1e-9 {
                        return arc;
                    }
                    let r = r.max(half);
                    let mid = Point::lerp(s, e, 0.5);
                    let n = e.sub(s).normalized().perp();
                    // Keep the center on the side it was (a major arc has
                    // it on the same side as the bulge).
                    let side = if n.dot(center.sub(mid)) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let c = mid.add(n.scale(side * (r * r - half * half).max(0.0).sqrt()));
                    ArcShape {
                        center: c,
                        radius: r,
                        a0: s.sub(c).angle(),
                        a1: e.sub(c).angle(),
                    }
                }
            }
        }
        ArcEdit::StartAngle(a) => match lock {
            ArcLock::Arc | ArcLock::Chord => arc,
            // The start point stays: the center moves around it.
            ArcLock::Start => ArcShape {
                center: arc.start().sub(dir(a).scale(radius)),
                a0: a,
                ..arc
            },
            ArcLock::End | ArcLock::Center => ArcShape { a0: a, ..arc },
        },
        ArcEdit::EndAngle(a) => match lock {
            ArcLock::Arc | ArcLock::Chord => arc,
            ArcLock::End => ArcShape {
                center: arc.end().sub(dir(a).scale(radius)),
                a1: a,
                ..arc
            },
            ArcLock::Start | ArcLock::Center => ArcShape { a1: a, ..arc },
        },
        ArcEdit::ChordLength(l) => {
            let sweep = arc.sweep();
            if l <= 1e-6 || sweep < 1e-9 || matches!(lock, ArcLock::Arc | ArcLock::Chord) {
                return arc;
            }
            let (s, e) = (arc.start(), arc.end());
            let cd = e.sub(s);
            if cd.length() < 1e-9 {
                return arc;
            }
            let u = cd.normalized();
            match lock {
                ArcLock::Start => from_chord(s, s.add(u.scale(l)), sweep),
                ArcLock::End => from_chord(e.sub(u.scale(l)), e, sweep),
                // The center and radius stay: the arc opens or closes
                // evenly about the middle of its sweep.
                _ => {
                    if l > 2.0 * radius {
                        return arc;
                    }
                    let half = 2.0 * (l / (2.0 * radius)).asin() / 2.0;
                    let mid = a0 + sweep / 2.0;
                    ArcShape {
                        a0: mid - half,
                        a1: mid + half,
                        ..arc
                    }
                }
            }
        }
        ArcEdit::ChordAngle(a) => {
            if matches!(lock, ArcLock::Arc | ArcLock::Chord) {
                return arc;
            }
            let (s, e) = (arc.start(), arc.end());
            let cd = e.sub(s);
            if cd.length() < 1e-9 {
                return arc;
            }
            let delta = a - cd.angle();
            let spin = |p: Point, about: Point| {
                let v = p.sub(about);
                let (sn, cs) = delta.sin_cos();
                about.add(Point::new(v.x * cs - v.y * sn, v.x * sn + v.y * cs))
            };
            let about = match lock {
                ArcLock::Start => s,
                ArcLock::End => e,
                _ => center,
            };
            ArcShape {
                center: spin(center, about),
                a0: a0 + delta,
                a1: a1 + delta,
                ..arc
            }
        }
        ArcEdit::ArcLength(len) => {
            if len <= 1e-6 || radius <= 1e-9 || lock == ArcLock::Arc {
                return arc;
            }
            if lock == ArcLock::Chord {
                let (s, e) = (arc.start(), arc.end());
                let c = s.dist(e);
                // The arc is at least as long as its chord and shorter than
                // a full circle on it; arc length rises with the sweep.
                if c < 1e-9 || len <= c {
                    return arc;
                }
                let (mut lo, mut hi) = (1e-6, TAU - 1e-6);
                for _ in 0..80 {
                    let th = (lo + hi) / 2.0;
                    if c * th / (2.0 * (th / 2.0).sin()) < len {
                        lo = th;
                    } else {
                        hi = th;
                    }
                }
                return from_chord(s, e, (lo + hi) / 2.0);
            }
            let th = (len / radius).min(TAU - 1e-6);
            match lock {
                ArcLock::Start => ArcShape { a1: a0 + th, ..arc },
                ArcLock::End => ArcShape { a0: a1 - th, ..arc },
                _ => {
                    let mid = a0 + arc.sweep() / 2.0;
                    ArcShape {
                        a0: mid - th / 2.0,
                        a1: mid + th / 2.0,
                        ..arc
                    }
                }
            }
        }
    }
}

/// The counter-clockwise arc from `s` to `e` that sweeps `sweep` radians.
fn from_chord(s: Point, e: Point, sweep: f64) -> ArcShape {
    let cd = e.sub(s);
    let half = sweep / 2.0;
    let r = cd.length() / (2.0 * half.sin());
    let c = Point::lerp(s, e, 0.5).add(cd.normalized().perp().scale(r * half.cos()));
    ArcShape {
        center: c,
        radius: r,
        a0: s.sub(c).angle(),
        a1: s.sub(c).angle() + sweep,
    }
}

/// The straight distance from the start to the end of an arc, and its
/// direction in degrees (the Chord Length and Chord Angle of the dialog).
pub fn chord(arc: &ArcShape) -> (f64, f64) {
    let d = arc.end().sub(arc.start());
    (d.length(), d.angle().to_degrees())
}

/// The directions of travel at the start and end of an arc, degrees
/// counter-clockwise from east (the Start and End Direction of the dialog).
pub fn directions(arc: &ArcShape) -> (f64, f64) {
    (
        (arc.a0.to_degrees() + 90.0).rem_euclid(360.0),
        (arc.a1.to_degrees() + 90.0).rem_euclid(360.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        a.dist(b) < 1e-6
    }

    #[test]
    fn a_line_keeps_what_the_lock_names() {
        let (a, b) = (p(0.0, 0.0), p(100.0, 0.0));
        let (s, e) = line_edit(a, b, LineLock::Start, LineEdit::Length(50.0));
        assert!(near(s, a) && near(e, p(50.0, 0.0)));
        let (s, e) = line_edit(a, b, LineLock::End, LineEdit::Length(50.0));
        assert!(near(s, p(50.0, 0.0)) && near(e, b));
        let (s, e) = line_edit(a, b, LineLock::Center, LineEdit::Length(50.0));
        assert!(near(s, p(25.0, 0.0)) && near(e, p(75.0, 0.0)));
        // Angle: the center turns the line about itself.
        let (s, e) = line_edit(a, b, LineLock::Center, LineEdit::Angle(90.0));
        assert!(
            near(s, p(50.0, -50.0)) && near(e, p(50.0, 50.0)),
            "{s:?} {e:?}"
        );
        let (s, e) = line_edit(a, b, LineLock::Start, LineEdit::Angle(90.0));
        assert!(near(s, a) && near(e, p(0.0, 100.0)));
    }

    #[test]
    fn length_angle_lock_drags_the_other_end_along() {
        let (a, b) = (p(0.0, 0.0), p(100.0, 0.0));
        let (s, e) = line_edit(a, b, LineLock::LengthAngle, LineEdit::Start(p(10.0, 5.0)));
        assert!(near(s, p(10.0, 5.0)) && near(e, p(110.0, 5.0)));
        let (s, e) = line_edit(a, b, LineLock::LengthAngle, LineEdit::End(p(90.0, -5.0)));
        assert!(near(s, p(-10.0, -5.0)) && near(e, p(90.0, -5.0)));
        // The length and angle boxes are not available.
        assert_eq!(
            line_edit(a, b, LineLock::LengthAngle, LineEdit::Length(5.0)),
            (a, b)
        );
        assert_eq!(
            line_edit(a, b, LineLock::LengthAngle, LineEdit::Angle(5.0)),
            (a, b)
        );
    }

    #[test]
    fn end_points_follow_the_lock() {
        let (a, b) = (p(0.0, 0.0), p(100.0, 0.0));
        assert_eq!(
            line_edit(a, b, LineLock::Start, LineEdit::Start(p(1.0, 1.0))),
            (a, b)
        );
        assert_eq!(
            line_edit(a, b, LineLock::End, LineEdit::End(p(1.0, 1.0))),
            (a, b)
        );
        let (s, e) = line_edit(a, b, LineLock::End, LineEdit::Start(p(10.0, 10.0)));
        assert!(near(s, p(10.0, 10.0)) && near(e, b));
        // Center lock: the end is mirrored through the center.
        let (s, e) = line_edit(a, b, LineLock::Center, LineEdit::End(p(60.0, 10.0)));
        assert!(near(e, p(60.0, 10.0)) && near(s, p(40.0, -10.0)));
        assert!(!LineLock::Center.start_free() && LineLock::Center.end_free());
        assert!(!LineLock::LengthAngle.length_angle_free());
    }

    fn quarter() -> ArcShape {
        ArcShape {
            center: p(0.0, 0.0),
            radius: 100.0,
            a0: 0.0,
            a1: std::f64::consts::FRAC_PI_2,
        }
    }

    #[test]
    fn an_arc_radius_change_under_each_lock() {
        let arc = quarter();
        let c = arc_edit(arc, ArcLock::Center, ArcEdit::Radius(200.0));
        assert!(near(c.center, arc.center) && (c.radius - 200.0).abs() < 1e-9);
        let s = arc_edit(arc, ArcLock::Start, ArcEdit::Radius(200.0));
        assert!(near(s.start(), arc.start()), "{:?}", s.start());
        assert!((s.radius - 200.0).abs() < 1e-9);
        let e = arc_edit(arc, ArcLock::End, ArcEdit::Radius(50.0));
        assert!(near(e.end(), arc.end()));
        let k = arc_edit(arc, ArcLock::Chord, ArcEdit::Radius(200.0));
        assert!(near(k.start(), arc.start()) && near(k.end(), arc.end()));
        assert!((k.radius - 200.0).abs() < 1e-9);
        // The chord of a quarter circle of 100 is 141.4; a smaller radius
        // than half of it is raised to half.
        let k = arc_edit(arc, ArcLock::Chord, ArcEdit::Radius(1.0));
        assert!((k.radius - 100.0 * 2f64.sqrt() / 2.0).abs() < 1e-6);
        assert_eq!(arc_edit(arc, ArcLock::Arc, ArcEdit::Radius(5.0)), arc);
    }

    #[test]
    fn an_arc_moves_whole_or_pivots_on_its_locked_end() {
        let arc = quarter();
        let moved = arc_edit(arc, ArcLock::Arc, ArcEdit::Center(p(10.0, 20.0)));
        assert!(near(moved.start(), p(110.0, 20.0)) && (moved.sweep() - arc.sweep()).abs() < 1e-12);
        assert_eq!(
            arc_edit(arc, ArcLock::Center, ArcEdit::Center(p(1.0, 1.0))),
            arc
        );
        let s = arc_edit(arc, ArcLock::Start, ArcEdit::Center(p(0.0, 100.0)));
        assert!(near(s.start(), arc.start()) && near(s.center, p(0.0, 100.0)));
        let a = arc_edit(arc, ArcLock::Start, ArcEdit::StartAngle(0.5));
        assert!(near(a.start(), arc.start()), "{:?}", a.start());
        let a = arc_edit(arc, ArcLock::End, ArcEdit::EndAngle(2.0));
        assert!(near(a.end(), arc.end()));
        let plain = arc_edit(arc, ArcLock::Center, ArcEdit::EndAngle(2.0));
        assert!(near(plain.center, arc.center) && (plain.a1 - 2.0).abs() < 1e-12);
        assert_eq!(arc_edit(arc, ArcLock::Chord, ArcEdit::StartAngle(0.5)), arc);
        assert!(!ArcLock::Arc.radius_free() && !ArcLock::Chord.angles_free());
    }

    #[test]
    fn the_chord_and_end_directions_of_an_arc() {
        let (len, ang) = chord(&quarter());
        assert!((len - 100.0 * 2f64.sqrt()).abs() < 1e-9 && (ang - 135.0).abs() < 1e-9);
        let (d0, d1) = directions(&quarter());
        assert!((d0 - 90.0).abs() < 1e-9 && (d1 - 180.0).abs() < 1e-9);
    }

    #[test]
    fn typed_chord_and_arc_length_follow_the_lock() {
        use std::f64::consts::{FRAC_PI_2, PI};
        let arc = ArcShape {
            center: p(0.0, 0.0),
            radius: 100.0,
            a0: 0.0,
            a1: FRAC_PI_2,
        };
        let c = 100.0 * 2f64.sqrt();
        // Chord Length with Start locked: start stays, the sweep is kept.
        let a = arc_edit(arc, ArcLock::Start, ArcEdit::ChordLength(c / 2.0));
        assert!(near(a.start(), arc.start()));
        assert!((a.radius - 50.0).abs() < 1e-6 && (a.sweep() - FRAC_PI_2).abs() < 1e-9);
        // Center locked: radius stays, the sweep opens to a half circle.
        let a = arc_edit(arc, ArcLock::Center, ArcEdit::ChordLength(200.0));
        assert!((a.sweep() - PI).abs() < 1e-9 && (a.radius - 100.0).abs() < 1e-9);
        // Locked chord or arc: nothing moves.
        assert_eq!(
            arc_edit(arc, ArcLock::Chord, ArcEdit::ChordLength(50.0)),
            arc
        );
        // Chord Angle with Start locked: the end swings round the start.
        let a = arc_edit(arc, ArcLock::Start, ArcEdit::ChordAngle(PI));
        assert!(near(a.start(), arc.start()));
        assert!(near(a.end(), p(100.0 - c, 0.0)), "{:?}", a.end());
        // Arc Length: Start locked keeps the radius and moves the end.
        let a = arc_edit(arc, ArcLock::Start, ArcEdit::ArcLength(100.0 * PI));
        assert!((a.sweep() - PI).abs() < 1e-9 && near(a.start(), arc.start()));
        // Chord locked: the arc bulges until it is that long.
        let a = arc_edit(arc, ArcLock::Chord, ArcEdit::ArcLength(c * PI / 2.0));
        assert!((a.sweep() - PI).abs() < 1e-6, "{}", a.sweep());
        assert!(near(a.start(), arc.start()) && near(a.end(), arc.end()));
    }
}
