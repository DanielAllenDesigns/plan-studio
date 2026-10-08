//! Truss geometry: Fink, Howe, king post, scissor, attic and mono trusses as
//! 2D chords and webs in the truss plane, plus the envelope the 3D view needs.
//!
//! The truss plane has `x` along the span from the left bearing (`0`) to the
//! right bearing (`span`) and `y` up from the underside of the bottom chord
//! at the bearing. All member ends are **centreline nodes**. The bottom chord
//! is split into panels at the web nodes; top chords run from the overhang
//! tail to the apex in one piece.

use crate::lumber::{Lumber, TWO_BY_FOUR};
use crate::manual::OrientedBox;
use crate::member::{add, cross, scale, Vec3};
use plan_core::Point;
use serde::{Deserialize, Serialize};

const EPS: f64 = 1e-6;

/// The common truss families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrussType {
    Fink,
    Howe,
    KingPost,
    Scissor,
    Attic,
    Mono,
}

impl TrussType {
    pub fn name(&self) -> &'static str {
        match self {
            TrussType::Fink => "Fink",
            TrussType::Howe => "Howe",
            TrussType::KingPost => "King post",
            TrussType::Scissor => "Scissor",
            TrussType::Attic => "Attic",
            TrussType::Mono => "Mono",
        }
    }
}

/// Role of a truss member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrussRole {
    TopChord,
    BottomChord,
    Web,
}

impl TrussRole {
    pub fn name(&self) -> &'static str {
        match self {
            TrussRole::TopChord => "truss top chord",
            TrussRole::BottomChord => "truss bottom chord",
            TrussRole::Web => "truss web",
        }
    }
}

/// Inputs for [`Truss::generate`]. Lengths are inches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrussSpec {
    pub kind: TrussType,
    /// Bearing to bearing.
    pub span: f64,
    /// Top chord pitch, rise per 12 of run (6.0 = 6:12).
    pub pitch: f64,
    /// Height of the heel node above the bearing plane. Anything at or below
    /// half the bottom chord depth means a standard heel; more adds a vertical
    /// heel member (a raised heel).
    pub heel_height: f64,
    /// Horizontal overhang of the top chord past the bearing.
    pub overhang: f64,
    /// Plies side by side (1 for a common truss, 2-4 for a girder).
    pub plies: u32,
    pub chord: Lumber,
    pub web: Lumber,
    /// Scissor bottom chord pitch (rise per 12). `0.0` selects half the top pitch.
    pub bottom_pitch: f64,
    /// Attic room width. `0.0` selects half the span.
    pub attic_width: f64,
}

impl TrussSpec {
    /// A single-ply truss with 2x4 chords and webs and a 12" overhang.
    pub fn new(kind: TrussType, span: f64, pitch: f64) -> Self {
        Self {
            kind,
            span,
            pitch,
            heel_height: 0.0,
            overhang: 12.0,
            plies: 1,
            chord: TWO_BY_FOUR,
            web: TWO_BY_FOUR,
            bottom_pitch: 0.0,
            attic_width: 0.0,
        }
    }

    /// Total thickness of all plies.
    pub fn thickness(&self) -> f64 {
        f64::from(self.plies.max(1)) * self.chord.thickness
    }
}

/// One chord or web in the truss plane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrussMember2 {
    pub role: TrussRole,
    pub lumber: Lumber,
    /// Centreline ends `(x along span, y up)`.
    pub a: Point,
    pub b: Point,
}

impl TrussMember2 {
    pub fn length(&self) -> f64 {
        self.a.dist(self.b)
    }

    pub fn midpoint(&self) -> Point {
        Point::lerp(self.a, self.b, 0.5)
    }

    /// The member as a rectangle in the truss plane, `lumber.depth` wide
    /// around the centreline: `a+n, b+n, b-n, a-n`.
    pub fn polygon(&self) -> [Point; 4] {
        let n = (self.b - self.a).normalized().perp() * (self.lumber.depth / 2.0);
        [self.a + n, self.b + n, self.b - n, self.a - n]
    }
}

/// What the 3D view needs of a truss: its bounding profile and extents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrussEnvelope {
    pub span: f64,
    /// Span plus the overhangs.
    pub overall_width: f64,
    pub min_x: f64,
    pub max_x: f64,
    /// Highest point of the top chord's upper edge above the bearing plane.
    pub peak_height: f64,
    /// Heel node height above the bearing plane.
    pub heel_height: f64,
    /// All plies together.
    pub thickness: f64,
    /// Closed profile in the truss plane, counter-clockwise, ending at the
    /// left bearing.
    pub outline: Vec<Point>,
}

impl TrussEnvelope {
    /// Profile area, square inches.
    pub fn area(&self) -> f64 {
        let n = self.outline.len();
        (0..n)
            .map(|i| self.outline[i].cross(self.outline[(i + 1) % n]))
            .sum::<f64>()
            / 2.0
    }
}

/// A generated truss.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Truss {
    pub spec: TrussSpec,
    pub members: Vec<TrussMember2>,
    pub envelope: TrussEnvelope,
}

impl Truss {
    /// Generate the chords and webs for `spec`. A non-positive span or pitch
    /// yields no members.
    pub fn generate(spec: &TrussSpec) -> Truss {
        let w = spec.span;
        let valid = w > 12.0 && spec.pitch >= 0.0;
        let members = if valid { build(spec) } else { Vec::new() };
        let envelope = envelope(spec, valid);
        Truss {
            spec: spec.clone(),
            members,
            envelope,
        }
    }

    /// Distinct web end points, sorted by x then y.
    pub fn web_nodes(&self) -> Vec<Point> {
        let mut nodes: Vec<Point> = Vec::new();
        for m in self.members.iter().filter(|m| m.role == TrussRole::Web) {
            for p in [m.a, m.b] {
                if !nodes.iter().any(|q| q.dist(p) < 1e-4) {
                    nodes.push(p);
                }
            }
        }
        nodes.sort_by(|p, q| p.x.total_cmp(&q.x).then(p.y.total_cmp(&q.y)));
        nodes
    }

    /// Thin oriented boxes for the 3D view. `base` is the plan point of the
    /// left bearing, `dir` the plan direction of the span (need not be unit),
    /// `elevation` the bearing plane height. The box thickness is all plies.
    pub fn to_boxes(&self, base: Point, dir: Point, elevation: f64) -> Vec<OrientedBox> {
        let d = dir.normalized();
        let d3: Vec3 = [d.x, 0.0, -d.y];
        let up: Vec3 = [0.0, 1.0, 0.0];
        let origin: Vec3 = [base.x, elevation, -base.y];
        let thickness = self.spec.thickness();
        self.members
            .iter()
            .filter(|m| m.length() > EPS)
            .map(|m| {
                let u = (m.b - m.a).normalized();
                let ax = add(scale(d3, u.x), scale(up, u.y));
                let ay = add(scale(d3, -u.y), scale(up, u.x));
                let az = cross(ax, ay);
                let c = m.midpoint();
                let center = add(origin, add(scale(d3, c.x), scale(up, c.y)));
                OrientedBox::from_axes(
                    center,
                    [m.length(), m.lumber.depth, thickness],
                    [ax, ay, az],
                )
            })
            .collect()
    }
}

fn seg(role: TrussRole, lumber: Lumber, a: (f64, f64), b: (f64, f64)) -> TrussMember2 {
    TrussMember2 {
        role,
        lumber,
        a: Point::new(a.0, a.1),
        b: Point::new(b.0, b.1),
    }
}

/// Heel node height: the larger of the requested height and the bottom chord's
/// centreline height.
fn heel_node(spec: &TrussSpec) -> f64 {
    spec.heel_height.max(spec.chord.depth / 2.0)
}

fn build(spec: &TrussSpec) -> Vec<TrussMember2> {
    let w = spec.span;
    let m = spec.pitch / 12.0;
    let bc = spec.chord.depth / 2.0;
    let yh = heel_node(spec);
    let ov = spec.overhang.max(0.0);
    let (chord, web) = (spec.chord, spec.web);
    // Top chord centreline height at distance x from the nearer bearing.
    let top = |x: f64| yh + m * x;
    // Height of the top chord at span position x for a double-slope truss.
    let top_at = |x: f64| top(x.min(w - x));
    let apex = (w / 2.0, top(w / 2.0));
    let mut out = Vec::new();

    let top_chords = |out: &mut Vec<TrussMember2>| {
        out.push(seg(TrussRole::TopChord, chord, (-ov, yh - m * ov), apex));
        out.push(seg(TrussRole::TopChord, chord, (w + ov, yh - m * ov), apex));
    };
    let raised_heel = yh > bc + 0.01;
    let heel_webs = |out: &mut Vec<TrussMember2>, left: bool, right: bool| {
        if raised_heel {
            if left {
                out.push(seg(TrussRole::Web, web, (0.0, bc), (0.0, yh)));
            }
            if right {
                out.push(seg(TrussRole::Web, web, (w, bc), (w, yh)));
            }
        }
    };
    let panels = |out: &mut Vec<TrussMember2>, xs: &[f64], y: &dyn Fn(f64) -> f64| {
        for p in xs.windows(2) {
            out.push(seg(
                TrussRole::BottomChord,
                chord,
                (p[0], y(p[0])),
                (p[1], y(p[1])),
            ));
        }
    };
    let flat = |_: f64| bc;

    match spec.kind {
        TrussType::Fink => {
            top_chords(&mut out);
            panels(&mut out, &[0.0, w / 3.0, 2.0 * w / 3.0, w], &flat);
            for (bx, tx) in [(w / 3.0, w / 4.0), (2.0 * w / 3.0, 3.0 * w / 4.0)] {
                out.push(seg(TrussRole::Web, web, (bx, bc), (tx, top_at(tx))));
                out.push(seg(TrussRole::Web, web, (bx, bc), apex));
            }
            heel_webs(&mut out, true, true);
        }
        TrussType::Howe => {
            top_chords(&mut out);
            panels(&mut out, &[0.0, w / 4.0, w / 2.0, 3.0 * w / 4.0, w], &flat);
            for tx in [w / 4.0, 3.0 * w / 4.0] {
                out.push(seg(TrussRole::Web, web, (tx, bc), (tx, top_at(tx))));
                out.push(seg(TrussRole::Web, web, (w / 2.0, bc), (tx, top_at(tx))));
            }
            out.push(seg(TrussRole::Web, web, (w / 2.0, bc), apex));
            heel_webs(&mut out, true, true);
        }
        TrussType::KingPost => {
            top_chords(&mut out);
            panels(&mut out, &[0.0, w / 2.0, w], &flat);
            out.push(seg(TrussRole::Web, web, (w / 2.0, bc), apex));
            heel_webs(&mut out, true, true);
        }
        TrussType::Scissor => {
            let bp = if spec.bottom_pitch > 0.0 {
                spec.bottom_pitch
            } else {
                spec.pitch / 2.0
            }
            .min((spec.pitch - 1.0).max(0.0));
            let mb = bp / 12.0;
            let yb = |x: f64| bc + mb * x.min(w - x);
            top_chords(&mut out);
            panels(&mut out, &[0.0, w / 4.0, w / 2.0, 3.0 * w / 4.0, w], &yb);
            out.push(seg(TrussRole::Web, web, (w / 2.0, yb(w / 2.0)), apex));
            for tx in [w / 4.0, 3.0 * w / 4.0] {
                out.push(seg(TrussRole::Web, web, (tx, yb(tx)), (tx, top_at(tx))));
                out.push(seg(TrussRole::Web, web, (tx, yb(tx)), apex));
            }
            heel_webs(&mut out, true, true);
        }
        TrussType::Attic => {
            let aw = if spec.attic_width > 0.0 {
                spec.attic_width
            } else {
                w / 2.0
            }
            .clamp(0.2 * w, 0.8 * w);
            let xk = (w - aw) / 2.0;
            let yk = top_at(xk);
            top_chords(&mut out);
            panels(
                &mut out,
                &[0.0, xk / 2.0, xk, w - xk, w - xk / 2.0, w],
                &flat,
            );
            for (kx, hx) in [(xk, xk / 2.0), (w - xk, w - xk / 2.0)] {
                out.push(seg(TrussRole::Web, web, (kx, bc), (kx, yk)));
                out.push(seg(TrussRole::Web, web, (hx, bc), (kx, yk)));
            }
            out.push(seg(TrussRole::BottomChord, chord, (xk, yk), (w - xk, yk)));
            if apex.1 - yk > 6.0 {
                out.push(seg(TrussRole::Web, web, (w / 2.0, yk), apex));
            }
            heel_webs(&mut out, true, true);
        }
        TrussType::Mono => {
            let hi = (w, top(w));
            out.push(seg(TrussRole::TopChord, chord, (-ov, yh - m * ov), hi));
            panels(&mut out, &[0.0, w / 3.0, 2.0 * w / 3.0, w], &flat);
            let t1 = (w / 3.0, top(w / 3.0));
            let t2 = (2.0 * w / 3.0, top(2.0 * w / 3.0));
            out.push(seg(TrussRole::Web, web, (w / 3.0, bc), t1));
            out.push(seg(TrussRole::Web, web, (2.0 * w / 3.0, bc), t2));
            out.push(seg(TrussRole::Web, web, (w, bc), hi));
            out.push(seg(TrussRole::Web, web, (w / 3.0, bc), t2));
            out.push(seg(TrussRole::Web, web, (2.0 * w / 3.0, bc), hi));
            heel_webs(&mut out, true, false);
        }
    }
    out
}

fn envelope(spec: &TrussSpec, valid: bool) -> TrussEnvelope {
    let w = spec.span;
    let m = spec.pitch.max(0.0) / 12.0;
    let yh = heel_node(spec);
    let ov = spec.overhang.max(0.0);
    let half = spec.chord.depth / 2.0;
    let theta = m.atan();
    let (sin, cos) = (theta.sin(), theta.cos());
    let mut outline = Vec::new();
    let (peak, min_x, max_x);
    if !valid {
        peak = 0.0;
        min_x = 0.0;
        max_x = w;
    } else if spec.kind == TrussType::Mono {
        // Low tail corners, then the high end's top edge.
        let tail = Point::new(-ov, yh - m * ov);
        let n = Point::new(-sin, cos) * half;
        let hi = Point::new(w, yh + m * w);
        outline.extend([
            Point::new(0.0, 0.0),
            tail - n,
            tail + n,
            hi + n,
            Point::new(w, 0.0),
        ]);
        peak = (hi + n).y;
        min_x = (tail - n).x.min(0.0);
        max_x = w;
    } else {
        let tl = Point::new(-ov, yh - m * ov);
        let tr = Point::new(w + ov, yh - m * ov);
        let nl = Point::new(-sin, cos) * half;
        let nr = Point::new(sin, cos) * half;
        let apex_top = Point::new(w / 2.0, yh + m * w / 2.0 + half / cos);
        outline.extend([
            Point::new(0.0, 0.0),
            tl - nl,
            tl + nl,
            apex_top,
            tr + nr,
            tr - nr,
            Point::new(w, 0.0),
        ]);
        peak = apex_top.y;
        min_x = (tl - nl).x.min(0.0);
        max_x = (tr - nr).x.max(w);
    }
    // Built clockwise from the left bearing; report counter-clockwise (the
    // reversed list ends at the left bearing).
    outline.reverse();
    let (min_x, max_x) = outline
        .iter()
        .fold((min_x, max_x), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
    TrussEnvelope {
        span: w,
        overall_width: max_x - min_x,
        min_x,
        max_x,
        peak_height: peak,
        heel_height: yh,
        thickness: spec.thickness(),
        outline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fink() -> Truss {
        Truss::generate(&TrussSpec::new(TrussType::Fink, 288.0, 6.0))
    }

    #[test]
    fn fink_24ft_6_12_has_nine_members_and_symmetric_web_nodes() {
        let t = fink();
        assert_eq!(t.members.len(), 9);
        let count = |r: TrussRole| t.members.iter().filter(|m| m.role == r).count();
        assert_eq!(count(TrussRole::TopChord), 2);
        assert_eq!(count(TrussRole::BottomChord), 3);
        assert_eq!(count(TrussRole::Web), 4);
        let nodes = t.web_nodes();
        assert_eq!(nodes.len(), 5);
        for p in &nodes {
            assert!(
                nodes
                    .iter()
                    .any(|q| (q.x - (288.0 - p.x)).abs() < 1e-6 && (q.y - p.y).abs() < 1e-6),
                "no mirror for {p:?}"
            );
        }
        // Apex: 6:12 over a 144" half span = 72" above the heel node (1.75").
        let apex = t.members[0].b;
        assert!((apex.x - 144.0).abs() < 1e-9);
        assert!((apex.y - (1.75 + 72.0)).abs() < 1e-9);
        // Top chord length = half-span run on the 6:12 slope + overhang run.
        let top = &t.members[0];
        let run = 144.0 + 12.0;
        assert!((top.length() - run * (1.0f64 + 0.25).sqrt()).abs() < 1e-6);
    }

    #[test]
    fn envelope_tracks_overhang_height_and_thickness() {
        let t = fink();
        let e = &t.envelope;
        // Square-cut tails: the tail corners sit within a chord depth of the 12" run.
        assert!((e.overall_width - (288.0 + 24.0)).abs() < 3.5);
        assert!(e.peak_height > 73.75);
        assert!(e.area() > 0.0);
        assert_eq!(e.outline.len(), 7);
        let mut spec = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        spec.plies = 3;
        assert!((Truss::generate(&spec).envelope.thickness - 4.5).abs() < 1e-9);
    }

    #[test]
    fn every_type_generates_connected_symmetric_geometry() {
        for (kind, n) in [
            (TrussType::Fink, 9),
            (TrussType::Howe, 11),
            (TrussType::KingPost, 5),
            (TrussType::Scissor, 11),
            (TrussType::Attic, 13),
            (TrussType::Mono, 9),
        ] {
            let t = Truss::generate(&TrussSpec::new(kind, 360.0, 8.0));
            assert_eq!(t.members.len(), n, "{kind:?}");
            assert!(t.members.iter().all(|m| m.length() > 1.0), "{kind:?}");
            assert!(t.envelope.area() > 0.0, "{kind:?}");
            if kind != TrussType::Mono {
                let nodes = t.web_nodes();
                for p in &nodes {
                    assert!(
                        nodes.iter().any(|q| (q.x - (360.0 - p.x)).abs() < 1e-6
                            && (q.y - p.y).abs() < 1e-6),
                        "{kind:?} {p:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn raised_heel_adds_vertical_heel_webs_and_bad_spans_are_empty() {
        let mut spec = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        spec.heel_height = 5.5;
        let t = Truss::generate(&spec);
        assert_eq!(t.members.len(), 11);
        assert!((t.envelope.heel_height - 5.5).abs() < 1e-9);
        spec.span = 0.0;
        assert!(Truss::generate(&spec).members.is_empty());
    }

    #[test]
    fn member_polygon_is_a_depth_wide_rectangle() {
        let t = fink();
        let bottom = t
            .members
            .iter()
            .find(|m| m.role == TrussRole::BottomChord)
            .unwrap();
        let p = bottom.polygon();
        assert!((p[0].dist(p[3]) - 3.5).abs() < 1e-9);
        assert!((p[0].dist(p[1]) - bottom.length()).abs() < 1e-9);
    }

    #[test]
    fn boxes_are_thin_and_follow_the_span_direction() {
        let t = fink();
        let boxes = t.to_boxes(Point::new(100.0, 50.0), Point::new(0.0, 1.0), 96.0);
        assert_eq!(boxes.len(), 9);
        for b in &boxes {
            assert!((b.size[2] - 1.5).abs() < 1e-9);
            // The truss plane contains the span axis (plan +y = 3D -z) and up,
            // so the thickness axis is horizontal and across the span.
            assert!(b.axes[2][1].abs() < 1e-9);
            assert!((b.axes[2][0].abs() - 1.0).abs() < 1e-9);
            assert!(b.center[1] >= 96.0);
        }
    }
}
