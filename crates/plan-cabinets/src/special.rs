//! Special cabinet shapes (reference manual pp. 664 to 668): end cabinets,
//! radius ends, the peninsula radius, angled and bow fronts, plus the blind
//! cabinets that two cabinets meeting in a corner make.
//!
//! Every special shape keeps the cabinet's frame (back on `y = 0`, front
//! towards `+y`, straight sides at `x = 0` and `x = width`) and only changes
//! the front line `y = f(x)`, so face items keep running along `x`.

use plan_core::geometry::Point;
use plan_core::Id;
use serde::{Deserialize, Serialize};
use std::f64::consts::FRAC_PI_2;

use crate::cabinet::{BlindSide, BlindSpec, Cabinet, CabinetKind};
use crate::filler::{run_class, run_mates};

/// Segments per quarter circle (and per bow).
const ARC_SEGMENTS: usize = 8;
/// Smallest visible face a blind cabinet keeps, inches.
const MIN_VISIBLE: f64 = 3.0;
/// How near a cabinet's end must come to another cabinet's front to count
/// as meeting it, inches.
const MEET: f64 = 0.5;

/// The special shapes that are a Type of the General panel's Cabinet Style
/// list (Corner is a cabinet kind of its own and Standard is no special).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpecialShape {
    LeftEnd,
    RightEnd,
    LeftRadiusEnd,
    RightRadiusEnd,
    PeninsulaRadius,
    AngledFront,
    BowFront,
}

impl SpecialShape {
    pub const ALL: [SpecialShape; 7] = [
        SpecialShape::LeftEnd,
        SpecialShape::RightEnd,
        SpecialShape::LeftRadiusEnd,
        SpecialShape::RightRadiusEnd,
        SpecialShape::PeninsulaRadius,
        SpecialShape::AngledFront,
        SpecialShape::BowFront,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SpecialShape::LeftEnd => "Left End",
            SpecialShape::RightEnd => "Right End",
            SpecialShape::LeftRadiusEnd => "Left Radius End",
            SpecialShape::RightRadiusEnd => "Right Radius End",
            SpecialShape::PeninsulaRadius => "Peninsula Radius",
            SpecialShape::AngledFront => "Angled Front",
            SpecialShape::BowFront => "Bow Front",
        }
    }

    /// End cabinets must be no wider than they are deep.
    fn is_end(self) -> bool {
        matches!(
            self,
            SpecialShape::LeftEnd
                | SpecialShape::RightEnd
                | SpecialShape::LeftRadiusEnd
                | SpecialShape::RightRadiusEnd
                | SpecialShape::PeninsulaRadius
        )
    }

    /// The label of the General panel's extra depth field.
    pub fn amount_label(self) -> &'static str {
        match self {
            SpecialShape::BowFront => "Bow Depth",
            SpecialShape::AngledFront => "Right Depth",
            SpecialShape::LeftEnd | SpecialShape::RightEnd => "Corner Cut",
            _ => "Radius",
        }
    }
}

/// A special shape and its one extra dimension.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Special {
    pub shape: SpecialShape,
    /// Bow Front: the bow depth (negative for an inside bow). Angled Front:
    /// the depth of the right side (`Cabinet::depth` is the left). Ends: the
    /// leg of the angled cut. Radius ends and the peninsula: the radius.
    /// `0` asks for the automatic size.
    pub amount: f64,
}

impl Special {
    pub fn new(shape: SpecialShape) -> Self {
        Self { shape, amount: 0.0 }
    }

    /// A bow front with bow depth `bow`.
    pub fn bow(bow: f64) -> Self {
        Self {
            shape: SpecialShape::BowFront,
            amount: bow,
        }
    }

    /// The automatic value of the extra dimension for a `w` by `d` cabinet.
    pub fn auto_amount(shape: SpecialShape, w: f64, d: f64) -> f64 {
        match shape {
            SpecialShape::BowFront => (w / 8.0).min(3.0),
            SpecialShape::AngledFront => d,
            SpecialShape::LeftEnd | SpecialShape::RightEnd => w.min(d) / 2.0,
            SpecialShape::LeftRadiusEnd | SpecialShape::RightRadiusEnd => w.min(d),
            SpecialShape::PeninsulaRadius => w.min(d) / 2.0,
        }
    }

    /// The extra dimension with `0` resolved to its automatic size.
    pub fn resolved(&self, w: f64, d: f64) -> f64 {
        if self.amount == 0.0 {
            Self::auto_amount(self.shape, w, d)
        } else {
            self.amount
        }
    }

    /// Why a `w` by `d` cabinet cannot have this shape; `Ok` when it can
    /// (the warnings of reference manual p. 664).
    ///
    /// # Errors
    /// A sentence saying what is needed.
    pub fn check(&self, w: f64, d: f64) -> Result<(), String> {
        if self.shape.is_end() && w > d + 1e-9 {
            return Err(format!(
                "A {} cabinet's width must be no greater than its depth.",
                self.shape.name()
            ));
        }
        match self.shape {
            SpecialShape::BowFront => {
                let bow = self.resolved(w, d);
                if bow.abs() > w / 2.0 + 1e-9 {
                    return Err("The bow depth cannot exceed half the cabinet width.".into());
                }
            }
            SpecialShape::AngledFront => {
                if self.resolved(w, d) < 1.0 || d < 1.0 {
                    return Err("Both depths of an angled front must be at least 1\".".into());
                }
            }
            SpecialShape::LeftEnd | SpecialShape::RightEnd => {
                if self.resolved(w, d) > w + 1e-9 {
                    return Err("The cut cannot be wider than the cabinet.".into());
                }
            }
            SpecialShape::LeftRadiusEnd | SpecialShape::RightRadiusEnd => {
                if self.resolved(w, d) > w.min(d) + 1e-9 {
                    return Err("The radius cannot exceed the width or the depth.".into());
                }
            }
            SpecialShape::PeninsulaRadius => {
                if self.resolved(w, d) > w / 2.0 + 1e-9 {
                    return Err("The radius cannot exceed half the width.".into());
                }
            }
        }
        Ok(())
    }

    /// The front line from the left end to the right end, local frame
    /// (first point at `x = 0`, last at `x = w`).
    pub fn front(&self, w: f64, d: f64) -> Vec<Point> {
        dedup_line(self.front_raw(w, d))
    }

    /// [`Special::front`] before repeated points are dropped (a radius as
    /// big as the cabinet starts its arc on the point before it).
    fn front_raw(&self, w: f64, d: f64) -> Vec<Point> {
        let a = self.resolved(w, d);
        match self.shape {
            SpecialShape::BowFront => bow_line(w, d, a),
            SpecialShape::AngledFront => vec![Point::new(0.0, d), Point::new(w, a.max(0.0))],
            SpecialShape::RightEnd => {
                let c = a.clamp(0.0, w.min(d));
                vec![
                    Point::new(0.0, d),
                    Point::new(w - c, d),
                    Point::new(w, d - c),
                ]
            }
            SpecialShape::LeftEnd => {
                let c = a.clamp(0.0, w.min(d));
                vec![Point::new(0.0, d - c), Point::new(c, d), Point::new(w, d)]
            }
            SpecialShape::RightRadiusEnd => {
                let r = a.clamp(0.0, w.min(d));
                let mut pts = vec![Point::new(0.0, d)];
                pts.extend(quarter(Point::new(w - r, d - r), r, FRAC_PI_2, 0.0));
                pts
            }
            SpecialShape::LeftRadiusEnd => {
                let r = a.clamp(0.0, w.min(d));
                let mut pts = quarter(Point::new(r, d - r), r, 2.0 * FRAC_PI_2, FRAC_PI_2);
                pts.push(Point::new(w, d));
                pts
            }
            SpecialShape::PeninsulaRadius => {
                let r = a.clamp(0.0, (w / 2.0).min(d));
                let mut pts = quarter(Point::new(r, d - r), r, 2.0 * FRAC_PI_2, FRAC_PI_2);
                pts.extend(quarter(Point::new(w - r, d - r), r, FRAC_PI_2, 0.0));
                pts
            }
        }
    }

    /// The outline, counter-clockwise from the back-left corner.
    pub fn footprint(&self, w: f64, d: f64) -> Vec<Point> {
        let mut ring = vec![Point::new(0.0, 0.0), Point::new(w, 0.0)];
        let front = self.front(w, d);
        ring.extend(front.iter().rev().copied());
        dedup(ring)
    }

    /// The depth of the cabinet at `x` (the front line's `y`).
    pub fn depth_at(&self, w: f64, d: f64, x: f64) -> f64 {
        let front = self.front(w, d);
        let x = x.clamp(0.0, w);
        for pair in front.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if x >= a.x - 1e-9 && x <= b.x + 1e-9 {
                let span = b.x - a.x;
                if span.abs() < 1e-9 {
                    return a.y.max(b.y);
                }
                return a.y + (b.y - a.y) * (x - a.x) / span;
            }
        }
        d
    }

    /// The deepest point of the footprint.
    pub fn max_depth(&self, w: f64, d: f64) -> f64 {
        self.front(w, d).iter().fold(0.0, |m, p| m.max(p.y))
    }

    /// The front line between `x0` and `x1` (clipped to them), for a face
    /// item that spans that stretch.
    pub fn front_between(&self, w: f64, d: f64, x0: f64, x1: f64) -> Vec<Point> {
        let mut pts = vec![Point::new(x0, self.depth_at(w, d, x0))];
        for p in self.front(w, d) {
            if p.x > x0 + 1e-9 && p.x < x1 - 1e-9 {
                pts.push(p);
            }
        }
        pts.push(Point::new(x1, self.depth_at(w, d, x1)));
        pts
    }
}

/// `line` without consecutive repeated points (an open line: its ends are
/// not compared).
fn dedup_line(line: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(line.len());
    for p in line {
        if out.last().is_none_or(|q| q.dist(p) > 1e-9) {
            out.push(p);
        }
    }
    out
}

fn dedup(ring: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(ring.len());
    for p in ring {
        if out.last().is_none_or(|q| q.dist(p) > 1e-9) {
            out.push(p);
        }
    }
    if out.len() > 1 && out[0].dist(out[out.len() - 1]) <= 1e-9 {
        out.pop();
    }
    out
}

/// Points of a quarter circle about `c` from angle `from` to angle `to`
/// (radians), both ends included.
fn quarter(c: Point, r: f64, from: f64, to: f64) -> Vec<Point> {
    (0..=ARC_SEGMENTS)
        .map(|k| {
            let th = from + (to - from) * k as f64 / ARC_SEGMENTS as f64;
            Point::new(c.x + r * th.cos(), c.y + r * th.sin())
        })
        .collect()
}

/// A circular arc through `(0, d)` and `(w, d)` that reaches `d + bow` in
/// the middle (an inside bow for a negative `bow`).
fn bow_line(w: f64, d: f64, bow: f64) -> Vec<Point> {
    if bow.abs() < 1e-9 || w <= 0.0 {
        return vec![Point::new(0.0, d), Point::new(w, d)];
    }
    let half = w / 2.0;
    let s = bow.abs();
    let radius = (half * half + s * s) / (2.0 * s);
    let sign = bow.signum();
    // The circle's centre sits below the chord for an outward bow.
    let cy = d + bow - sign * radius;
    (0..=2 * ARC_SEGMENTS)
        .map(|k| {
            let x = w * k as f64 / (2 * ARC_SEGMENTS) as f64;
            let dx = x - half;
            let y = cy + sign * (radius * radius - dx * dx).max(0.0).sqrt();
            Point::new(x, y)
        })
        .collect()
}

// ----- blind cabinets -----

/// The part of `a`'s front that cabinet `b` hides, when `b` meets `a` at a
/// corner: `b` stands at a right angle in front of `a`'s front line and
/// reaches one of its ends. Returns the end and the hidden width, inches.
fn hidden_by(a: &Cabinet, b: &Cabinet) -> Option<(BlindSide, f64)> {
    // The angle between the two must be a right angle.
    let turn = (b.angle - a.angle).rem_euclid(std::f64::consts::TAU);
    let quarter = (turn - FRAC_PI_2).abs() < 0.02 || (turn - 3.0 * FRAC_PI_2).abs() < 0.02;
    if !quarter {
        return None;
    }
    let local = |p: Point| {
        let d = p.sub(a.position);
        let (s, c) = a.angle.sin_cos();
        Point::new(d.x * c + d.y * s, -d.x * s + d.y * c)
    };
    let corners: Vec<Point> = b.corners().iter().map(|p| local(*p)).collect();
    let (y_lo, y_hi) = corners
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p.y), h.max(p.y)));
    let (x_lo, x_hi) = corners
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p.x), h.max(p.x)));
    // `b` stands in front of `a`: its near end touches `a`'s front line.
    if (y_lo - a.depth).abs() > MEET || y_hi <= a.depth + MEET {
        return None;
    }
    let (lo, hi) = (x_lo.max(0.0), x_hi.min(a.width));
    let hidden = hi - lo;
    if hidden < MEET {
        return None;
    }
    if hi >= a.width - MEET && lo > MEET {
        Some((BlindSide::Right, hidden))
    } else if lo <= MEET && hi < a.width - MEET {
        Some((BlindSide::Left, hidden))
    } else {
        // Hides the whole front or only the middle: not a blind end.
        None
    }
}

/// The blind cabinets two cabinets meeting in a corner make: for every
/// plain box cabinet whose front end is covered by the end of another
/// cabinet of its family and height standing at a right angle to it, the
/// hidden width and which end it is. Cabinets that already carry a blind of
/// the user's own are left out (`Cabinet::blind_auto` marks the program's). A cabinet needs [`MIN_VISIBLE`] inches of
/// face left.
pub fn blind_corners(cabs: &[Cabinet]) -> Vec<(Id, BlindSpec)> {
    let mut out: Vec<(Id, BlindSpec)> = Vec::new();
    for a in cabs {
        if !is_box(a) || (a.blind.is_some() && !a.blind_auto) || a.appliance.is_some() {
            continue;
        }
        for b in cabs {
            if a.id == b.id || !is_box(b) || !run_mates(a, b) {
                continue;
            }
            if let Some((side, hidden)) = hidden_by(a, b) {
                if a.width - hidden >= MIN_VISIBLE && !out.iter().any(|(id, _)| *id == a.id) {
                    out.push((
                        a.id,
                        BlindSpec {
                            side,
                            blind_width: hidden,
                        },
                    ));
                }
            }
        }
    }
    out
}

fn is_box(c: &Cabinet) -> bool {
    run_class(c).is_some()
        && !c.kind.is_corner()
        && !c.kind.is_filler()
        && c.custom.is_none()
        && c.special.is_none()
        && c.kind != CabinetKind::Soffit
}

/// Brings the automatic blind ends of `cabs` up to date: cabinets that meet
/// a neighbour in a corner get a blind end, cabinets whose automatic blind
/// no longer applies lose it. A blind the user set (not `auto`) stays.
/// Returns how many cabinets changed.
pub fn apply_blind_corners(cabs: &mut [Cabinet]) -> usize {
    let want = blind_corners(cabs);
    let mut changed = 0;
    for c in cabs.iter_mut() {
        let wanted = want.iter().find(|(id, _)| *id == c.id).map(|(_, b)| *b);
        let have_auto = c.blind.is_some() && c.blind_auto;
        match (wanted, have_auto) {
            (Some(w), _) if c.blind != Some(w) || !c.blind_auto => {
                c.blind = Some(w);
                c.blind_auto = true;
                changed += 1;
            }
            (None, true) => {
                c.blind = None;
                c.blind_auto = false;
                changed += 1;
            }
            _ => {}
        }
    }
    changed
}

// ----- exposed ends -----

/// How far outside an end face the probes stand, inches.
const PROBE: f64 = 0.3;

/// Do cabinets `a` and `b` count as mates for the ends between them?
/// Base cabinets and full height cabinets mate with one another (and with
/// partitions and appliance bays), wall cabinets with wall cabinets.
fn end_mates(a: &Cabinet, b: &Cabinet) -> bool {
    use crate::filler::RunClass;
    let family = |c: &Cabinet| -> Option<bool> {
        if c.kind == CabinetKind::Partition {
            return Some(true);
        }
        match run_class(c)? {
            RunClass::Base | RunClass::FullHeight => Some(true),
            RunClass::Wall => Some(false),
        }
    };
    matches!((family(a), family(b)), (Some(x), Some(y)) if x == y)
}

/// Which ends of each cabinet touch a wall or another cabinet (mated); the
/// rest are exposed: they get the countertop overhang, corner treatment, feet
/// and closed toe (reference manual p. 665). `walls` are the wall outlines in
/// plan. Only plain box cabinets and special shapes have ends; the others
/// are left out.
pub fn exposures(cabs: &[Cabinet], walls: &[Vec<Point>]) -> Vec<(Id, crate::options::Ends)> {
    use crate::options::Ends;
    use plan_core::geometry::point_in_polygon;
    let footprints: Vec<Vec<Point>> = cabs.iter().map(Cabinet::footprint).collect();
    let mut out = Vec::new();
    for (i, c) in cabs.iter().enumerate() {
        let ordinary =
            run_class(c).is_some() && !c.kind.is_corner() && c.custom.is_none() && !c.auto_filler;
        if !ordinary {
            continue;
        }
        let (w, d) = (c.width, c.depth);
        let probes = |end: u8| -> [Point; 3] {
            let at = |t: f64| match end {
                0 => Point::new(-PROBE, d * t),
                1 => Point::new(w + PROBE, d * t),
                _ => Point::new(w * t, -PROBE),
            };
            [at(0.2), at(0.5), at(0.8)]
        };
        let mated = |end: u8| -> (bool, bool) {
            let mut by_cab = 0;
            let mut by_wall = 0;
            for p in probes(end) {
                let plan = c.to_plan(p);
                let cab_hit = cabs.iter().enumerate().any(|(j, o)| {
                    j != i && end_mates(c, o) && point_in_polygon(plan, &footprints[j])
                });
                let wall_hit = walls.iter().any(|r| point_in_polygon(plan, r));
                by_cab += usize::from(cab_hit);
                by_wall += usize::from(wall_hit);
            }
            (by_cab + by_wall >= 2, by_wall >= 2)
        };
        let (left, _) = mated(0);
        let (right, _) = mated(1);
        let (back, back_wall) = mated(2);
        out.push((
            c.id,
            Ends {
                left,
                right,
                back,
                back_wall,
            },
        ));
    }
    out
}

/// Stores [`exposures`] on the cabinets (`Cabinet::ends`); cabinets that
/// have no ends lose theirs. Returns how many changed.
pub fn apply_exposures(cabs: &mut [Cabinet], walls: &[Vec<Point>]) -> usize {
    let want = exposures(cabs, walls);
    let mut changed = 0;
    for c in cabs.iter_mut() {
        let e = want.iter().find(|(id, _)| *id == c.id).map(|(_, e)| *e);
        if c.ends != e {
            c.ends = e;
            changed += 1;
        }
    }
    changed
}

// ----- the Cabinet Style list -----

/// The Cabinet Style list of the General panel (reference manual p. 669).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CabinetStyle {
    Standard,
    Corner,
    Special(SpecialShape),
}

impl CabinetStyle {
    /// The list in the order the dialog shows it.
    pub const ALL: [CabinetStyle; 9] = [
        CabinetStyle::Standard,
        CabinetStyle::Corner,
        CabinetStyle::Special(SpecialShape::LeftEnd),
        CabinetStyle::Special(SpecialShape::RightEnd),
        CabinetStyle::Special(SpecialShape::LeftRadiusEnd),
        CabinetStyle::Special(SpecialShape::RightRadiusEnd),
        CabinetStyle::Special(SpecialShape::PeninsulaRadius),
        CabinetStyle::Special(SpecialShape::AngledFront),
        CabinetStyle::Special(SpecialShape::BowFront),
    ];

    pub fn name(self) -> &'static str {
        match self {
            CabinetStyle::Standard => "Standard",
            CabinetStyle::Corner => "Corner",
            CabinetStyle::Special(s) => s.name(),
        }
    }
}

/// Offsets every edge of the counter-clockwise `ring` outward by its own
/// amount (`offs[i]` for the edge from point `i` to point `i + 1`), meeting
/// neighbouring edges at their intersection.
fn offset_edges(ring: &[Point], offs: &[f64]) -> Vec<Point> {
    let n = ring.len();
    let normal = |a: Point, b: Point| {
        let e = b.sub(a).normalized();
        Point::new(e.y, -e.x)
    };
    let reach = offs.iter().fold(0.0_f64, |m, o| m.max(o.abs())) * 3.0 + 1e-9;
    (0..n)
        .map(|i| {
            let prev = (i + n - 1) % n;
            let (a, b, c) = (ring[prev], ring[i], ring[(i + 1) % n]);
            let (n1, n2) = (normal(a, b), normal(b, c));
            let (o1, o2) = (offs[prev], offs[i]);
            let det = n1.x * n2.y - n1.y * n2.x;
            let d = if det.abs() < 1e-9 {
                n1.scale((o1 + o2) / 2.0)
            } else {
                Point::new((o1 * n2.y - o2 * n1.y) / det, (n1.x * o2 - n2.x * o1) / det)
            };
            let len = d.dist(Point::ZERO);
            if len > reach {
                b.add(d.scale(reach / len))
            } else {
                b.add(d)
            }
        })
        .collect()
}

impl Cabinet {
    /// What the Cabinet Style list shows for this cabinet.
    pub fn style(&self) -> CabinetStyle {
        if self.kind.is_corner() {
            CabinetStyle::Corner
        } else if let Some(s) = &self.special {
            CabinetStyle::Special(s.shape)
        } else {
            CabinetStyle::Standard
        }
    }

    /// Turns this cabinet into `style`: a corner cabinet from a standard
    /// one (its width must be greater than its depth), an end, radius end,
    /// peninsula, angled or bow front, or back to a standard cabinet. The
    /// face items stay as they are.
    ///
    /// # Errors
    /// The warning of reference manual p. 664 when the cabinet does not
    /// meet the requirements; nothing changes then.
    pub fn convert(&mut self, style: CabinetStyle) -> Result<(), String> {
        if self.style() == style {
            return Ok(());
        }
        let ordinary = matches!(
            self.kind,
            CabinetKind::Base
                | CabinetKind::Wall
                | CabinetKind::FullHeight
                | CabinetKind::CornerBase
                | CabinetKind::CornerWall
        ) && self.appliance.is_none();
        if style != CabinetStyle::Standard && !ordinary {
            return Err("Only base, wall and full height cabinets can change style.".into());
        }
        match style {
            CabinetStyle::Standard => {
                self.special = None;
                self.corner_bow = 0.0;
                self.kind = match self.kind {
                    CabinetKind::CornerBase => CabinetKind::Base,
                    CabinetKind::CornerWall => CabinetKind::Wall,
                    k => k,
                };
                self.corner = None;
            }
            CabinetStyle::Corner => {
                if self.width <= self.depth + 1e-9 {
                    return Err(
                        "Before a corner cabinet can be specified the cabinet's width \
                                must be greater than its depth."
                            .into(),
                    );
                }
                let kind = match self.kind {
                    CabinetKind::Base => CabinetKind::CornerBase,
                    CabinetKind::Wall => CabinetKind::CornerWall,
                    _ => {
                        return Err(
                            "A corner cabinet is a base or a wall cabinet in Plan Studio.".into(),
                        )
                    }
                };
                self.kind = kind;
                self.special = None;
                let arm: f64 = if kind == CabinetKind::CornerBase {
                    24.0
                } else {
                    12.0
                };
                self.corner = Some(crate::cabinet::CornerSpec {
                    arm_depth: arm.min(self.depth),
                    ..crate::cabinet::CornerSpec::default()
                });
            }
            CabinetStyle::Special(shape) => {
                let mut sp = Special::new(shape);
                if shape == SpecialShape::AngledFront {
                    sp.amount = self.depth;
                }
                sp.check(self.width, self.depth)?;
                if self.kind.is_corner() {
                    self.kind = if self.kind == CabinetKind::CornerBase {
                        CabinetKind::Base
                    } else {
                        CabinetKind::Wall
                    };
                    self.corner = None;
                    self.corner_bow = 0.0;
                }
                self.special = Some(sp);
            }
        }
        Ok(())
    }

    /// Changes the extra dimension of the special shape (bow depth, right
    /// depth, cut, radius), checking it.
    ///
    /// # Errors
    /// Why the new value does not fit; nothing changes then.
    pub fn set_special_amount(&mut self, amount: f64) -> Result<(), String> {
        let Some(sp) = self.special else {
            return Err("The cabinet has no special shape.".into());
        };
        let next = Special { amount, ..sp };
        next.check(self.width, self.depth)?;
        self.special = Some(next);
        Ok(())
    }

    /// The interior points of a corner cabinet's bowed diagonal, from the
    /// right arm to the left one (the end points are the arm corners).
    pub(crate) fn corner_front(&self) -> Vec<Point> {
        let (w, d) = (self.width, self.depth);
        let a = self.arm_depth_clamped();
        let (p, q) = (Point::new(w, a), Point::new(a, d));
        let chord = q.sub(p);
        let len = chord.dist(Point::ZERO);
        let bow = self.corner_bow;
        if len < 1e-9 || bow.abs() < 1e-9 {
            return Vec::new();
        }
        // Outward is away from the inside corner at the origin.
        let mut out_n = Point::new(-chord.y, chord.x).normalized();
        if out_n.dot(p) < 0.0 {
            out_n = out_n.scale(-1.0);
        }
        let s = bow.abs();
        let radius = (len * len / 4.0 + s * s) / (2.0 * s);
        let mid = p.add(chord.scale(0.5));
        let sign = bow.signum();
        let centre = mid.add(out_n.scale(sign * (s - radius)));
        let n = 2 * ARC_SEGMENTS;
        (1..n)
            .map(|k| {
                let along = (k as f64 / n as f64 - 0.5) * len;
                let h = (radius * radius - along * along).max(0.0).sqrt();
                centre
                    .add(out_n.scale(sign * h))
                    .add(chord.normalized().scale(along))
            })
            .collect()
    }

    fn arm_depth_clamped(&self) -> f64 {
        let a = self.corner.map_or(24.0, |c| c.arm_depth);
        a.clamp(1.0, self.width.min(self.depth))
    }

    /// The footprint with the box's corner treatment (Box Construction)
    /// applied to the corners that are exposed.
    pub(crate) fn treat_box_corners(&self, ring: Vec<Point>) -> Vec<Point> {
        let b = &self.box_construction;
        if b.corner == crate::top::CornerTreatment::None || b.corner_size <= 1e-6 {
            return ring;
        }
        if !matches!(
            self.special.map(|s| s.shape),
            None | Some(SpecialShape::BowFront)
        ) {
            return ring;
        }
        let (w, d) = (self.width, self.depth);
        let ends = self.ends.unwrap_or_default();
        let known = self.ends.is_some();
        let (lo, hi) = (0.0, w);
        crate::top::treat_corners(&ring, b.corner, b.corner_size, |p| {
            let left = (p.x - lo).abs() < 1e-6;
            let right = (p.x - hi).abs() < 1e-6;
            if !left && !right {
                return false;
            }
            let front = p.y > d * 0.5;
            if b.auto_corners {
                if !known {
                    return true;
                }
                (if left { !ends.left } else { !ends.right }) && (front || !ends.back)
            } else {
                // Back Left, Back Right, Front Left, Front Right.
                let idx = match (front, left) {
                    (false, true) => 0,
                    (false, false) => 1,
                    (true, true) => 2,
                    (true, false) => 3,
                };
                b.corners[idx]
            }
        })
    }

    /// The countertop outline of a special shape or a bowed corner: the
    /// footprint with each edge pushed out by the overhang it carries.
    pub(crate) fn generic_top_ring(
        &self,
        t: &crate::cabinet::Countertop,
        treated: bool,
    ) -> Vec<Point> {
        let ring = geom_ccw(self.footprint_local());
        let w = self.width;
        let (d, corner) = (self.depth, self.kind.is_corner());
        let ends = self.ends.unwrap_or_default();
        let n = ring.len();
        let offs: Vec<f64> = (0..n)
            .map(|i| {
                let (a, b) = (ring[i], ring[(i + 1) % n]);
                let flat_y = a.y.abs() < 1e-6 && b.y.abs() < 1e-6;
                let left = a.x.abs() < 1e-6 && b.x.abs() < 1e-6;
                let right = (a.x - w).abs() < 1e-6 && (b.x - w).abs() < 1e-6;
                if flat_y {
                    if corner || ends.back {
                        0.0
                    } else {
                        t.overhang_back
                    }
                } else if left {
                    if corner || ends.left {
                        0.0
                    } else {
                        t.overhang_sides
                    }
                } else if right {
                    if corner || ends.right {
                        0.0
                    } else {
                        t.overhang_sides
                    }
                } else {
                    t.overhang_front
                }
            })
            .collect();
        let out = offset_edges(&ring, &offs);
        if !treated {
            return out;
        }
        let (lo, hi) = crate::geom::bbox(&out).map_or((0.0, w), |(a, b)| (a.x, b.x));
        crate::top::treat_corners(&out, t.corner, t.corner_size, |p| {
            ((p.x - lo).abs() < 1e-6 || (p.x - hi).abs() < 1e-6) && p.y > d * 0.5
        })
    }

    /// Is the outline something other than a rectangle or a corner cabinet's
    /// L: a special shape, or a box with clipped or rounded corners? Those
    /// are built as a shell of the outline.
    pub fn is_shaped(&self) -> bool {
        if self.kind.is_corner() || self.custom.is_some() || self.appliance.is_some() {
            return false;
        }
        if !matches!(
            self.kind,
            CabinetKind::Base | CabinetKind::Wall | CabinetKind::FullHeight
        ) {
            return false;
        }
        self.special.is_some()
            || (self.box_construction.corner != crate::top::CornerTreatment::None
                && self.box_construction.corner_size > 1e-6)
    }

    /// Does the box have a top panel (Box Construction, Top)?
    pub fn box_has_top(&self) -> bool {
        self.box_construction.top.resolve(!self.kind.is_base_like())
    }

    /// Does the box have a bottom panel (Box Construction, Bottom)? With
    /// Auto, one whose lowest face item is not a separation or a blank area
    /// is an appliance garage and has none.
    pub fn box_has_bottom(&self) -> bool {
        self.box_construction
            .bottom
            .resolve(self.face.bottom_is_closed())
    }
}

fn geom_ccw(ring: Vec<Point>) -> Vec<Point> {
    crate::geom::ccw(&ring)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::polygon_area;

    fn area(shape: SpecialShape, w: f64, d: f64, amount: f64) -> f64 {
        let s = Special { shape, amount };
        polygon_area(&s.footprint(w, d)).abs()
    }

    #[test]
    fn bow_front_bulges_by_the_bow_depth_and_has_the_arc_area() {
        let s = Special::bow(4.0);
        let front = s.front(36.0, 24.0);
        assert!((front[0].y - 24.0).abs() < 1e-9 && (front[front.len() - 1].y - 24.0).abs() < 1e-9);
        let mid = front[front.len() / 2];
        assert!((mid.x - 18.0).abs() < 1e-9 && (mid.y - 28.0).abs() < 1e-9);
        assert!((s.max_depth(36.0, 24.0) - 28.0).abs() < 1e-9);
        // A circular segment of chord 36 and height 4.
        let r = (18.0_f64 * 18.0 + 16.0) / 8.0;
        let seg = r * r * ((18.0 / r).asin()) - 18.0 * (r * r - 18.0 * 18.0).sqrt();
        let a = area(SpecialShape::BowFront, 36.0, 24.0, 4.0);
        assert!(
            (a - (36.0 * 24.0 + seg)).abs() < 0.8,
            "area {a} segment {seg}"
        );
        // An inside bow takes the same segment away.
        let inner = area(SpecialShape::BowFront, 36.0, 24.0, -4.0);
        assert!((inner - (36.0 * 24.0 - seg)).abs() < 0.8);
        assert!(Special::bow(-4.0).depth_at(36.0, 24.0, 18.0) < 24.0);
    }

    #[test]
    fn the_bow_cannot_exceed_half_the_width() {
        assert!(Special::bow(18.0).check(36.0, 24.0).is_ok());
        assert!(Special::bow(18.5).check(36.0, 24.0).is_err());
        assert!(Special::bow(-20.0).check(36.0, 24.0).is_err());
    }

    #[test]
    fn angled_front_joins_the_left_and_right_depths() {
        let s = Special {
            shape: SpecialShape::AngledFront,
            amount: 12.0,
        };
        let f = s.front(30.0, 24.0);
        assert_eq!(f.len(), 2);
        assert!((s.depth_at(30.0, 24.0, 0.0) - 24.0).abs() < 1e-9);
        assert!((s.depth_at(30.0, 24.0, 30.0) - 12.0).abs() < 1e-9);
        assert!((s.depth_at(30.0, 24.0, 15.0) - 18.0).abs() < 1e-9);
        // A trapezoid: width times the mean depth.
        let a = area(SpecialShape::AngledFront, 30.0, 24.0, 12.0);
        assert!((a - 30.0 * 18.0).abs() < 1e-6);
    }

    #[test]
    fn end_cabinets_need_width_no_greater_than_depth_and_clip_the_exposed_corner() {
        assert!(Special::new(SpecialShape::RightEnd)
            .check(12.0, 24.0)
            .is_ok());
        assert!(Special::new(SpecialShape::RightEnd)
            .check(30.0, 24.0)
            .is_err());
        assert!(Special::new(SpecialShape::RightRadiusEnd)
            .check(30.0, 24.0)
            .is_err());
        let s = Special::new(SpecialShape::RightEnd);
        let a = area(SpecialShape::RightEnd, 12.0, 24.0, 0.0);
        // The cut takes half of a 6 by 6 triangle.
        assert!((a - (12.0 * 24.0 - 18.0)).abs() < 1e-6, "{a}");
        // The right end is the one cut.
        assert!(s.depth_at(12.0, 24.0, 12.0) < 24.0);
        assert!((s.depth_at(12.0, 24.0, 0.0) - 24.0).abs() < 1e-9);
        let left = Special::new(SpecialShape::LeftEnd);
        assert!(left.depth_at(12.0, 24.0, 0.0) < 24.0);
        assert!((left.depth_at(12.0, 24.0, 12.0) - 24.0).abs() < 1e-9);
    }

    #[test]
    fn radius_ends_curve_to_the_side_they_name() {
        let right = Special::new(SpecialShape::RightRadiusEnd);
        let a = area(SpecialShape::RightRadiusEnd, 12.0, 24.0, 0.0);
        // A quarter circle of radius 12 is taken from the front corner... the
        // square r*r minus the quarter disc is what is lost.
        let lost = 12.0 * 12.0 - std::f64::consts::PI * 144.0 / 4.0;
        assert!((a - (12.0 * 24.0 - lost)).abs() < 1.0, "{a}");
        assert!((right.depth_at(12.0, 24.0, 0.0) - 24.0).abs() < 1e-9);
        assert!((right.depth_at(12.0, 24.0, 12.0) - 12.0).abs() < 1e-9);
        let left = Special::new(SpecialShape::LeftRadiusEnd);
        assert!((left.depth_at(12.0, 24.0, 12.0) - 24.0).abs() < 1e-9);
        assert!((left.depth_at(12.0, 24.0, 0.0) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn the_peninsula_radius_rounds_both_front_corners() {
        let s = Special {
            shape: SpecialShape::PeninsulaRadius,
            amount: 6.0,
        };
        assert!((s.depth_at(12.0, 24.0, 0.0) - 18.0).abs() < 1e-9);
        assert!((s.depth_at(12.0, 24.0, 12.0) - 18.0).abs() < 1e-9);
        assert!((s.depth_at(12.0, 24.0, 6.0) - 24.0).abs() < 1e-9);
        // Half the width at most.
        assert!(s.check(12.0, 24.0).is_ok());
        assert!(Special { amount: 7.0, ..s }.check(12.0, 24.0).is_err());
    }

    #[test]
    fn front_between_follows_the_curve() {
        let s = Special::bow(4.0);
        let part = s.front_between(36.0, 24.0, 6.0, 18.0);
        assert!(part.len() >= 3);
        assert!((part[0].x - 6.0).abs() < 1e-9 && (part[part.len() - 1].x - 18.0).abs() < 1e-9);
        assert!(part.windows(2).all(|p| p[0].x < p[1].x));
    }

    fn base_at(id: Id, x: f64, y: f64, angle: f64, w: f64) -> Cabinet {
        let mut c = Cabinet::base(w);
        c.id = id;
        c.position = Point::new(x, y);
        c.angle = angle;
        c
    }

    #[test]
    fn a_cabinet_whose_front_end_meets_another_goes_blind_by_that_depth() {
        // A runs along the back wall; B stands at its right end, turned a
        // quarter, its end face against A's front line.
        let a = base_at(1, 0.0, 0.0, 0.0, 48.0);
        // B's local x axis runs along plan +y after a quarter turn; place it
        // so its end (local x = 0) is at y = 24 and its depth covers x in
        // [24, 48].
        let b = base_at(2, 48.0, 24.0, std::f64::consts::FRAC_PI_2, 36.0);
        let want = blind_corners(&[a.clone(), b.clone()]);
        assert_eq!(want.len(), 1);
        let (id, spec) = want[0];
        assert_eq!(id, 1);
        assert_eq!(spec.side, BlindSide::Right);
        assert!(
            (spec.blind_width - 24.0).abs() < 1e-6,
            "{}",
            spec.blind_width
        );
        // The same pair apart does nothing.
        let far = base_at(2, 48.0, 40.0, std::f64::consts::FRAC_PI_2, 36.0);
        assert!(blind_corners(&[a.clone(), far]).is_empty());
        // The blind cabinet keeps its visible face: apply stores and clears it.
        let mut pair = vec![a, b];
        assert_eq!(apply_blind_corners(&mut pair), 1);
        assert!((pair[0].face_width() - 24.0).abs() < 1e-6);
        assert_eq!(apply_blind_corners(&mut pair), 0);
        pair[1].position = Point::new(48.0, 60.0);
        assert_eq!(apply_blind_corners(&mut pair), 1);
        assert!(pair[0].blind.is_none());
    }

    #[test]
    fn a_blind_the_user_set_is_never_replaced() {
        let mut a = base_at(1, 0.0, 0.0, 0.0, 48.0);
        a.blind = Some(BlindSpec {
            side: BlindSide::Left,
            blind_width: 9.0,
        });
        let b = base_at(2, 48.0, 24.0, std::f64::consts::FRAC_PI_2, 36.0);
        let mut pair = vec![a, b];
        assert_eq!(apply_blind_corners(&mut pair), 0);
        assert_eq!(pair[0].blind.unwrap().blind_width, 9.0);
    }
}
