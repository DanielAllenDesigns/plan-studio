//! Truss geometry: Fink, Howe, king post, scissor, attic and mono trusses as
//! 2D chords and webs in the truss plane, plus the envelope the 3D view needs.
//!
//! The truss plane has `x` along the span from the left bearing (`0`) to the
//! right bearing (`span`) and `y` up from the underside of the bottom chord
//! at the bearing. All member ends are **centreline nodes**. The bottom chord
//! is split into panels at the web nodes; top chords run from the overhang
//! tail to the apex in one piece.

use crate::lumber::{Lumber, TWO_BY_FOUR, TWO_BY_THICKNESS};
use crate::manual::{FramingMember, MemberKind as ManualKind, OrientedBox};
use crate::member::{add, cross, scale, Member, MemberKind, Vec3};
use plan_core::{Id, Point};
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
    /// Six bottom chord panels, a "W" web on each side of the king post.
    DoubleFink,
    /// Eight bottom chord panels with verticals and diagonals.
    DoubleHowe,
}

impl TrussType {
    pub fn name(&self) -> &'static str {
        match self {
            TrussType::DoubleFink => "Double Fink",
            TrussType::Fink => "Fink",
            TrussType::Howe => "Howe",
            TrussType::KingPost => "King post",
            TrussType::Scissor => "Scissor",
            TrussType::Attic => "Attic",
            TrussType::Mono => "Mono",
            TrussType::DoubleHowe => "Double Howe",
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

fn yes() -> bool {
    true
}

fn default_block_spacing() -> f64 {
    24.0
}

fn default_web_thickness() -> f64 {
    TWO_BY_THICKNESS
}

fn default_drop() -> f64 {
    7.25
}

fn default_flat_depth() -> f64 {
    12.0
}

fn default_stud_spacing() -> f64 {
    16.0
}

/// How high an Energy Heel raises the heel node above the bearing plane,
/// inches (the manual asks for a roof raised at least 7" off the plates).
pub const ENERGY_HEEL_RAISE: f64 = 7.0;

/// Inputs for [`Truss::generate`]. Lengths are inches.
///
/// The fields after `attic_width` are the Roof Truss and Floor/Ceiling Truss
/// Specification dialogs (manual pp. 955 to 962); each has a serde default,
/// so plans saved before them read as they were.
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
    /// The top chord, and the bottom chord too while `bottom_chord_depth` is 0.
    pub chord: Lumber,
    pub web: Lumber,
    /// Scissor bottom chord pitch (rise per 12). `0.0` selects half the top pitch.
    pub bottom_pitch: f64,
    /// Attic room width. `0.0` selects half the span.
    pub attic_width: f64,
    /// Bottom Chord depth when it differs from the top chord's; `0.0` uses
    /// the top chord's.
    #[serde(default)]
    pub bottom_chord_depth: f64,
    /// Maximum Horizontal Span along the top chord between supports; `0.0`
    /// leaves the webbing of `kind`. A smaller span refines the webbing
    /// (King post, Fink, Howe, Double Fink, Double Howe).
    #[serde(default)]
    pub max_span_top: f64,
    /// Maximum Horizontal Span along the bottom chord.
    #[serde(default)]
    pub max_span_bottom: f64,
    /// Horizontal Blocking between the verticals of an End Truss.
    #[serde(default)]
    pub horizontal_blocking: bool,
    /// Vertical spacing of the horizontal blocking, centre to centre.
    #[serde(default = "default_block_spacing")]
    pub block_spacing: f64,
    /// Rollout Offset: bottom of the truss to the lowest blocking member.
    #[serde(default)]
    pub rollout_offset: f64,
    /// Rollout Offset "Automatic": the Vertical Spacing is the offset.
    #[serde(default = "yes")]
    pub rollout_auto: bool,
    /// Require Kingpost: a vertical web from the apex to the bottom chord.
    #[serde(default)]
    pub require_kingpost: bool,
    /// End Truss: vertical members at the wall stud spacing replace the webbing.
    #[serde(default)]
    pub end_truss: bool,
    /// Stud spacing of an End Truss's verticals.
    #[serde(default = "default_stud_spacing")]
    pub stud_spacing: f64,
    /// Energy Heel: a raised heel with a vertical member over the wall.
    #[serde(default)]
    pub energy_heel: bool,
    /// Drop Hip Truss: the top is lowered by `drop_depth` so common rafters
    /// and hip ridges pass over it.
    #[serde(default)]
    pub drop_hip: bool,
    /// How far a Drop Hip Truss lowers its top (the rafter depth).
    #[serde(default = "default_drop")]
    pub drop_depth: f64,
    /// Reduced Gable: no overhang, the top lowered by a chord depth so
    /// lookouts pass over the truss.
    #[serde(default)]
    pub reduced_gable: bool,
    /// Sloping Flat Truss: parallel chords inside the roof structure,
    /// `flat_depth` deep.
    #[serde(default)]
    pub sloping_flat: bool,
    /// Structure depth of a Sloping Flat Truss, and the overall depth of a
    /// floor or ceiling truss.
    #[serde(default = "default_flat_depth")]
    pub flat_depth: f64,
    /// Lock Truss Envelope and Webbing: a moved or copied truss keeps its shape.
    #[serde(default)]
    pub locked: bool,
    /// Use Special Snapping (unchecked by editing the ends with the handles).
    #[serde(default = "yes")]
    pub special_snapping: bool,
    /// Calculate Chords/Webbing in Materials List: the chords and webs are
    /// counted as pieces; unchecked the truss is one object.
    #[serde(default = "yes")]
    pub calc_chords: bool,
    /// Floor/Ceiling trusses: Vertical Supports.
    #[serde(default)]
    pub vertical_supports: bool,
    /// Floor/Ceiling trusses: thickness of the webbing.
    #[serde(default = "default_web_thickness")]
    pub web_thickness: f64,
    /// Show Multi-Ply Lines of a girder.
    #[serde(default)]
    pub show_ply_lines: bool,
    /// Force Truss Rebuild: asked in the specification dialog and acted on
    /// when it is accepted; never saved.
    #[serde(skip)]
    pub force_rebuild: bool,
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
            bottom_chord_depth: 0.0,
            max_span_top: 0.0,
            max_span_bottom: 0.0,
            horizontal_blocking: false,
            block_spacing: default_block_spacing(),
            rollout_offset: 0.0,
            rollout_auto: true,
            require_kingpost: false,
            end_truss: false,
            stud_spacing: default_stud_spacing(),
            energy_heel: false,
            drop_hip: false,
            drop_depth: default_drop(),
            reduced_gable: false,
            sloping_flat: false,
            flat_depth: default_flat_depth(),
            locked: false,
            special_snapping: true,
            calc_chords: true,
            vertical_supports: false,
            web_thickness: default_web_thickness(),
            show_ply_lines: false,
            force_rebuild: false,
        }
    }

    /// Total thickness of all plies.
    pub fn thickness(&self) -> f64 {
        f64::from(self.plies.max(1)) * self.chord.thickness
    }

    /// The bottom chord's lumber: the top chord's unless a Bottom Chord depth
    /// is set.
    pub fn bottom_lumber(&self) -> Lumber {
        if self.bottom_chord_depth > 0.0 {
            Lumber {
                thickness: self.chord.thickness,
                depth: self.bottom_chord_depth,
            }
        } else {
            self.chord
        }
    }

    /// The webbing family the Maximum Horizontal Spans lead to: `kind`, or a
    /// finer family of the same line (King post, Fink, Howe, Double Fink,
    /// Double Howe) when `kind`'s panels are longer than a maximum span.
    /// Scissor, Attic and Mono trusses keep their own webbing.
    pub fn resolved_kind(&self) -> TrussType {
        const LADDER: [TrussType; 5] = [
            TrussType::KingPost,
            TrussType::Fink,
            TrussType::Howe,
            TrussType::DoubleFink,
            TrussType::DoubleHowe,
        ];
        let Some(start) = LADDER.iter().position(|k| *k == self.kind) else {
            return self.kind;
        };
        if self.max_span_top <= 0.0 && self.max_span_bottom <= 0.0 {
            return self.kind;
        }
        let w = self.span;
        // Longest horizontal panel along the bottom and top chord of each family.
        let panels = |k: TrussType| -> (f64, f64) {
            match k {
                TrussType::KingPost => (w / 2.0, w / 2.0),
                TrussType::Fink => (w / 3.0, w / 4.0),
                TrussType::Howe => (w / 4.0, w / 4.0),
                TrussType::DoubleFink => (w / 6.0, w / 6.0),
                _ => (w / 8.0, w / 8.0),
            }
        };
        LADDER[start..]
            .iter()
            .copied()
            .find(|k| {
                let (b, t) = panels(*k);
                (self.max_span_bottom <= 0.0 || b <= self.max_span_bottom + 1e-9)
                    && (self.max_span_top <= 0.0 || t <= self.max_span_top + 1e-9)
            })
            .unwrap_or(TrussType::DoubleHowe)
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

/// The numbers every part of a truss is built from, after the directives
/// (energy heel, drop hip, reduced gable, sloping flat) are applied.
struct Shape {
    w: f64,
    /// Rise per inch of run of the top chord (after a lowered top).
    m: f64,
    /// Heel node height above the bearing plane.
    yh: f64,
    /// Overhang of the top chord.
    ov: f64,
    /// Half the bottom chord depth: the bottom chord centreline height.
    bc: f64,
    /// Centre-to-centre gap of a sloping flat truss's chords.
    gap: f64,
}

fn shape(spec: &TrussSpec) -> Shape {
    let w = spec.span;
    let bc = spec.bottom_lumber().depth / 2.0;
    let mut yh = spec.heel_height.max(bc);
    if spec.energy_heel {
        yh = yh.max(ENERGY_HEEL_RAISE);
    }
    let gap = (spec.flat_depth - spec.chord.depth / 2.0 - bc).max(1.0);
    if spec.sloping_flat {
        yh = yh.max(bc + gap);
    }
    let m0 = spec.pitch.max(0.0) / 12.0;
    let lower = if spec.reduced_gable {
        spec.chord.depth
    } else if spec.drop_hip {
        spec.drop_depth
    } else {
        0.0
    };
    // The apex drops by `lower`, never by more than 80% of the rise.
    let rise = m0 * w / 2.0;
    let lower = lower.min(rise * 0.8).max(0.0);
    let m = if w > 0.0 {
        (rise - lower) / (w / 2.0)
    } else {
        m0
    };
    Shape {
        w,
        m,
        yh,
        ov: if spec.reduced_gable {
            0.0
        } else {
            spec.overhang.max(0.0)
        },
        bc,
        gap,
    }
}

fn build(spec: &TrussSpec) -> Vec<TrussMember2> {
    let Shape {
        w, m, yh, ov, bc, ..
    } = shape(spec);
    let chord = spec.chord;
    let bottom = spec.bottom_lumber();
    let web = spec.web;
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
                bottom,
                (p[0], y(p[0])),
                (p[1], y(p[1])),
            ));
        }
    };
    let flat = |_: f64| bc;
    let kind = if spec.sloping_flat {
        None
    } else {
        Some(spec.resolved_kind())
    };

    match kind {
        None => {
            // Parallel chords `gap` apart, verticals and alternating diagonals.
            let g = shape(spec).gap;
            let yb = |x: f64| top_at(x) - g;
            top_chords(&mut out);
            let xs: Vec<f64> = (0..=8).map(|i| w * f64::from(i) / 8.0).collect();
            panels(&mut out, &xs, &yb);
            for i in 1..=8usize {
                let (a, b) = (xs[i - 1], xs[i]);
                if i < 8 {
                    out.push(seg(TrussRole::Web, web, (b, yb(b)), (b, top_at(b))));
                }
                // Diagonals lean toward the middle.
                if b <= w / 2.0 + 1e-9 {
                    out.push(seg(TrussRole::Web, web, (a, yb(a)), (b, top_at(b))));
                } else {
                    out.push(seg(TrussRole::Web, web, (b, yb(b)), (a, top_at(a))));
                }
            }
            heel_webs(&mut out, true, true);
        }
        Some(TrussType::Fink) => {
            top_chords(&mut out);
            panels(&mut out, &[0.0, w / 3.0, 2.0 * w / 3.0, w], &flat);
            for (bx, tx) in [(w / 3.0, w / 4.0), (2.0 * w / 3.0, 3.0 * w / 4.0)] {
                out.push(seg(TrussRole::Web, web, (bx, bc), (tx, top_at(tx))));
                out.push(seg(TrussRole::Web, web, (bx, bc), apex));
            }
            heel_webs(&mut out, true, true);
        }
        Some(TrussType::Howe) => {
            top_chords(&mut out);
            panels(&mut out, &[0.0, w / 4.0, w / 2.0, 3.0 * w / 4.0, w], &flat);
            for tx in [w / 4.0, 3.0 * w / 4.0] {
                out.push(seg(TrussRole::Web, web, (tx, bc), (tx, top_at(tx))));
                out.push(seg(TrussRole::Web, web, (w / 2.0, bc), (tx, top_at(tx))));
            }
            out.push(seg(TrussRole::Web, web, (w / 2.0, bc), apex));
            heel_webs(&mut out, true, true);
        }
        Some(TrussType::DoubleFink) => {
            top_chords(&mut out);
            let xs: Vec<f64> = (0..=6).map(|i| w * f64::from(i) / 6.0).collect();
            panels(&mut out, &xs, &flat);
            // Top chord nodes of the left half; the right half mirrors them.
            let tl = [w / 12.0, w / 4.0, 5.0 * w / 12.0];
            for (i, bx) in [w / 6.0, w / 3.0].into_iter().enumerate() {
                for (tx, mx) in [(tl[i], w - tl[i]), (tl[i + 1], w - tl[i + 1])] {
                    // Mirror of the bottom node w - bx: bottom 5w/6 and 2w/3.
                    out.push(seg(TrussRole::Web, web, (bx, bc), (tx, top_at(tx))));
                    out.push(seg(TrussRole::Web, web, (w - bx, bc), (mx, top_at(mx))));
                }
            }
            out.push(seg(
                TrussRole::Web,
                web,
                (w / 2.0, bc),
                (tl[2], top_at(tl[2])),
            ));
            out.push(seg(
                TrussRole::Web,
                web,
                (w / 2.0, bc),
                (w - tl[2], top_at(tl[2])),
            ));
            out.push(seg(TrussRole::Web, web, (w / 2.0, bc), apex));
            heel_webs(&mut out, true, true);
        }
        Some(TrussType::DoubleHowe) => {
            top_chords(&mut out);
            let xs: Vec<f64> = (0..=8).map(|i| w * f64::from(i) / 8.0).collect();
            panels(&mut out, &xs, &flat);
            for i in 1..=3 {
                let (l, r) = (w * f64::from(i) / 8.0, w - w * f64::from(i) / 8.0);
                out.push(seg(TrussRole::Web, web, (l, bc), (l, top_at(l))));
                out.push(seg(TrussRole::Web, web, (r, bc), (r, top_at(r))));
                // Diagonals from the next bottom node toward the middle up to this top node.
                let (nl, nr) = (w * f64::from(i + 1) / 8.0, w - w * f64::from(i + 1) / 8.0);
                out.push(seg(TrussRole::Web, web, (nl, bc), (l, top_at(l))));
                out.push(seg(TrussRole::Web, web, (nr, bc), (r, top_at(r))));
            }
            out.push(seg(TrussRole::Web, web, (w / 2.0, bc), apex));
            heel_webs(&mut out, true, true);
        }
        Some(TrussType::KingPost) => {
            top_chords(&mut out);
            panels(&mut out, &[0.0, w / 2.0, w], &flat);
            out.push(seg(TrussRole::Web, web, (w / 2.0, bc), apex));
            heel_webs(&mut out, true, true);
        }
        Some(TrussType::Scissor) => {
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
        Some(TrussType::Attic) => {
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
            out.push(seg(TrussRole::BottomChord, bottom, (xk, yk), (w - xk, yk)));
            if apex.1 - yk > 6.0 {
                out.push(seg(TrussRole::Web, web, (w / 2.0, yk), apex));
            }
            heel_webs(&mut out, true, true);
        }
        Some(TrussType::Mono) => {
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

    if spec.end_truss {
        end_truss_webbing(spec, &mut out, &top_at);
    } else if spec.require_kingpost {
        let has_post = out.iter().any(|t| {
            t.role == TrussRole::Web
                && (t.a.x - w / 2.0).abs() < 1e-6
                && (t.b.x - w / 2.0).abs() < 1e-6
        });
        if !has_post && spec.kind != TrussType::Mono {
            // The bottom chord node under the apex.
            let yb = out
                .iter()
                .filter(|t| t.role == TrussRole::BottomChord)
                .flat_map(|t| [t.a, t.b])
                .find(|p| (p.x - w / 2.0).abs() < 1e-6)
                .map_or(bc, |p| p.y);
            out.push(seg(TrussRole::Web, web, (w / 2.0, yb), apex));
        }
    }
    if spec.horizontal_blocking {
        horizontal_blocking(spec, &mut out, &top_at);
    }
    out
}

/// End Truss: the webbing is replaced by verticals at the wall stud spacing
/// and the bottom chord runs in one piece.
fn end_truss_webbing(
    spec: &TrussSpec,
    out: &mut Vec<TrussMember2>,
    top_at: &dyn Fn(f64) -> f64,
) {
    let (w, bc) = (spec.span, spec.bottom_lumber().depth / 2.0);
    out.retain(|t| t.role == TrussRole::TopChord);
    out.push(seg(
        TrussRole::BottomChord,
        spec.bottom_lumber(),
        (0.0, bc),
        (w, bc),
    ));
    let step = spec.stud_spacing.max(spec.web.thickness * 2.0);
    let mut x = step;
    while x < w - spec.web.thickness {
        out.push(seg(TrussRole::Web, spec.web, (x, bc), (x, top_at(x))));
        x += step;
    }
}

/// Horizontal members between the verticals of an End Truss (or any truss with
/// verticals), every `block_spacing` up from the rollout offset.
fn horizontal_blocking(
    spec: &TrussSpec,
    out: &mut Vec<TrussMember2>,
    top_at: &dyn Fn(f64) -> f64,
) {
    let mut xs: Vec<f64> = out
        .iter()
        .filter(|t| {
            t.role == TrussRole::Web && (t.a.x - t.b.x).abs() < 1e-6 && t.a.y.min(t.b.y) < 20.0
        })
        .map(|t| t.a.x)
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let spacing = spec.block_spacing.max(6.0);
    let first = if spec.rollout_auto || spec.rollout_offset <= 0.0 {
        spacing
    } else {
        spec.rollout_offset
    };
    let t2 = spec.web.depth / 2.0;
    for pair in xs.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let ceiling = top_at(a).min(top_at(b)) - spec.chord.depth / 2.0 - t2;
        let mut y = first;
        while y < ceiling {
            out.push(seg(TrussRole::Web, spec.web, (a, y), (b, y)));
            y += spacing;
        }
    }
}

fn envelope(spec: &TrussSpec, valid: bool) -> TrussEnvelope {
    let Shape { w, m, yh, ov, .. } = shape(spec);
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

// ===================================================================
// Truss configurations, labels and the schedule
// ===================================================================

/// One distinct truss configuration in the plan: the trusses that share a
/// label, one diagram and one Truss Detail drawing (manual p. 943, 944).
#[derive(Debug, Clone, PartialEq)]
pub struct TrussConfig {
    /// `TR-1` for a roof truss, `FTR-1` for a floor or ceiling truss: the
    /// number is the order the configuration first appeared.
    pub label: String,
    /// A floor or ceiling truss.
    pub floor: bool,
    /// How many trusses share the configuration.
    pub count: usize,
    pub span: f64,
    /// The inputs of a drawn or laid-out truss; `None` for the trusses the
    /// roof framing made.
    pub spec: Option<TrussSpec>,
    /// The diagram: chords and webs in the truss plane.
    pub members: Vec<TrussMember2>,
    /// Ids of the manual or laid-out members that have this configuration.
    pub ids: Vec<Id>,
}

impl TrussConfig {
    /// Overall width of the diagram, inches (the span plus overhangs).
    pub fn overall_width(&self) -> f64 {
        let (lo, hi) = self.members.iter().flat_map(|m| [m.a.x, m.b.x]).fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(lo, hi), x| (lo.min(x), hi.max(x)),
        );
        if lo.is_finite() {
            hi - lo
        } else {
            self.span
        }
    }

    /// Height of the diagram above the bearing plane, inches.
    pub fn height(&self) -> f64 {
        self.members
            .iter()
            .flat_map(|m| [m.a.y, m.b.y])
            .fold(0.0, f64::max)
    }
}

fn sixteenth(v: f64) -> f64 {
    (v * 16.0).round() / 16.0
}

/// The configuration a manual truss member belongs to: floor/ceiling or roof
/// side and its spec with the span set to the member's length (rounded to
/// 1/16"). `None` for members that are not trusses.
pub fn config_of(m: &FramingMember) -> Option<(bool, TrussSpec)> {
    let mut spec = m.truss.clone()?;
    if !m.kind.is_truss() {
        return None;
    }
    spec.span = sixteenth(m.plan_length());
    spec.plies = m.plies.max(1);
    Some((m.kind == ManualKind::FloorCeilingTruss, spec))
}

/// The 2D diagram of one truss the roof framing made: its members' centre
/// lines in the truss plane (x along the span from the bottom chord's start).
fn auto_diagram(group: &[&Member]) -> Vec<TrussMember2> {
    let Some(base) = group
        .iter()
        .find(|m| m.kind == MemberKind::TrussBottomChord)
        .or_else(|| group.first())
    else {
        return Vec::new();
    };
    let o = base.transform.origin;
    let ax = base.transform.axis_x;
    let h = (ax[0].hypot(ax[2])).max(1e-9);
    let s = [ax[0] / h, ax[2] / h];
    let to = |p: Vec3| Point::new((p[0] - o[0]) * s[0] + (p[2] - o[2]) * s[1], p[1] - o[1]);
    group
        .iter()
        .map(|m| {
            let a = m.transform.origin;
            let b = add(a, scale(m.transform.axis_x, m.length));
            TrussMember2 {
                role: match m.kind {
                    MemberKind::TrussTopChord => TrussRole::TopChord,
                    MemberKind::TrussBottomChord => TrussRole::BottomChord,
                    _ => TrussRole::Web,
                },
                lumber: m.lumber,
                a: to(a),
                b: to(b),
            }
        })
        .collect()
}

/// A key that is equal for two diagrams that draw the same truss.
fn diagram_key(d: &[TrussMember2]) -> Vec<(u8, i64, i64, i64, i64)> {
    let q = |v: f64| (v * 10.0).round() as i64;
    let mut v: Vec<(u8, i64, i64, i64, i64)> = d
        .iter()
        .map(|m| {
            let (a, b) = if (m.a.x, m.a.y) <= (m.b.x, m.b.y) {
                (m.a, m.b)
            } else {
                (m.b, m.a)
            };
            (m.role as u8, q(a.x), q(a.y), q(b.x), q(b.y))
        })
        .collect();
    v.sort_unstable();
    v
}

/// Every distinct truss configuration of a plan, in the order each first
/// appears: the manual and laid-out trusses of `manual` (in the order given:
/// the order they were created), then the trusses the roof framing made
/// (`auto`, grouped by the truss number tagged on their labels). Roof and
/// girder trusses are `TR-n`, floor and ceiling trusses `FTR-n`, each
/// numbered from 1. Trusses that share a configuration share a label.
pub fn truss_configs(manual: &[FramingMember], auto: &[Member]) -> Vec<TrussConfig> {
    let mut out: Vec<TrussConfig> = Vec::new();
    let mut keys: Vec<(bool, Option<TrussSpec>, Vec<(u8, i64, i64, i64, i64)>)> = Vec::new();
    let (mut roofs, mut floors) = (0, 0);
    let mut label_for = |floor: bool| {
        if floor {
            floors += 1;
            format!("FTR-{floors}")
        } else {
            roofs += 1;
            format!("TR-{roofs}")
        }
    };
    for m in manual {
        let Some((floor, spec)) = config_of(m) else {
            continue;
        };
        if let Some(i) = keys
            .iter()
            .position(|(f, s, _)| *f == floor && s.as_ref() == Some(&spec))
        {
            out[i].count += 1;
            out[i].ids.push(m.id);
            continue;
        }
        let truss = Truss::generate(&spec);
        out.push(TrussConfig {
            label: label_for(floor),
            floor,
            count: 1,
            span: spec.span,
            members: truss.members,
            spec: Some(spec.clone()),
            ids: vec![m.id],
        });
        keys.push((floor, Some(spec), Vec::new()));
    }
    // The roof framing's own trusses, one group per tagged number.
    let mut groups: Vec<(u32, Vec<&Member>)> = Vec::new();
    for m in auto {
        let Some(n) = crate::roof::truss_id(m) else {
            continue;
        };
        match groups.iter_mut().find(|(g, _)| *g == n) {
            Some((_, v)) => v.push(m),
            None => groups.push((n, vec![m])),
        }
    }
    for (_, g) in groups {
        let diagram = auto_diagram(&g);
        let key = diagram_key(&diagram);
        if let Some(i) = keys
            .iter()
            .position(|(f, s, k)| !*f && s.is_none() && *k == key)
        {
            out[i].count += 1;
            continue;
        }
        let span = diagram
            .iter()
            .filter(|m| m.role == TrussRole::BottomChord)
            .flat_map(|m| [m.a.x, m.b.x])
            .fold(0.0, f64::max);
        out.push(TrussConfig {
            label: label_for(false),
            floor: false,
            count: 1,
            span,
            spec: None,
            members: diagram,
            ids: Vec::new(),
        });
        keys.push((false, None, key));
    }
    out
}

/// The automatic label of every truss member in `manual`, by id.
pub fn truss_labels(manual: &[FramingMember], auto: &[Member]) -> Vec<(Id, String)> {
    truss_configs(manual, auto)
        .into_iter()
        .flat_map(|c| c.ids.into_iter().map(move |id| (id, c.label.clone())))
        .collect()
}

/// One row of the framing schedule for a truss configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct TrussRow {
    pub label: String,
    pub quantity: usize,
    /// Roof, girder or floor/ceiling truss, with the webbing: `Fink roof truss`.
    pub description: String,
    pub span: f64,
    pub pitch: f64,
    pub plies: u32,
    /// Nominal sizes of the chords and the webbing: `2x4 / 2x4 / 2x4`.
    pub members: String,
    pub overhang: f64,
}

/// The truss rows of the framing schedule, one per configuration.
pub fn truss_schedule(manual: &[FramingMember], auto: &[Member]) -> Vec<TrussRow> {
    truss_configs(manual, auto)
        .into_iter()
        .map(|c| match &c.spec {
            Some(s) => TrussRow {
                description: format!(
                    "{} {} truss",
                    s.resolved_kind().name(),
                    if c.floor { "floor/ceiling" } else { "roof" }
                ),
                span: c.span,
                pitch: s.pitch,
                plies: s.plies,
                members: format!(
                    "{} / {} / {}",
                    s.chord.nominal_name(),
                    s.bottom_lumber().nominal_name(),
                    s.web.nominal_name()
                ),
                overhang: s.overhang,
                label: c.label,
                quantity: c.count,
            },
            None => TrussRow {
                description: "Fink roof truss".to_string(),
                span: c.span,
                pitch: 0.0,
                plies: 1,
                members: "2x4 / 2x4 / 2x4".to_string(),
                overhang: 0.0,
                label: c.label,
                quantity: c.count,
            },
        })
        .collect()
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

    // ----- Round 16: spans, directives, labels -----

    fn count(t: &Truss, r: TrussRole) -> usize {
        t.members.iter().filter(|m| m.role == r).count()
    }

    fn mirrored(t: &Truss, w: f64) -> bool {
        let nodes = t.web_nodes();
        nodes.iter().all(|p| {
            nodes
                .iter()
                .any(|q| (q.x - (w - p.x)).abs() < 1e-6 && (q.y - p.y).abs() < 1e-6)
        })
    }

    #[test]
    fn a_maximum_horizontal_span_refines_the_webbing() {
        let mut spec = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        assert_eq!(spec.resolved_kind(), TrussType::Fink);
        // Fink panels are 96" along the bottom chord: a 60" maximum needs a double Fink.
        spec.max_span_bottom = 60.0;
        assert_eq!(spec.resolved_kind(), TrussType::DoubleFink);
        let t = Truss::generate(&spec);
        assert_eq!(count(&t, TrussRole::BottomChord), 6);
        assert_eq!(count(&t, TrussRole::TopChord), 2);
        assert!(mirrored(&t, 288.0));
        assert!(t.members.iter().all(|m| m.length() > 1.0));
        // 40" needs eight panels.
        spec.max_span_bottom = 40.0;
        assert_eq!(spec.resolved_kind(), TrussType::DoubleHowe);
        let t = Truss::generate(&spec);
        assert_eq!(count(&t, TrussRole::BottomChord), 8);
        assert!(mirrored(&t, 288.0));
        // A top chord maximum alone does it too, and never coarsens a type.
        let mut top = TrussSpec::new(TrussType::Howe, 288.0, 6.0);
        top.max_span_top = 200.0;
        assert_eq!(top.resolved_kind(), TrussType::Howe);
        top.max_span_top = 50.0;
        assert_eq!(top.resolved_kind(), TrussType::DoubleFink);
        // Scissor and Mono keep their own webbing.
        let mut sc = TrussSpec::new(TrussType::Scissor, 288.0, 8.0);
        sc.max_span_bottom = 30.0;
        assert_eq!(sc.resolved_kind(), TrussType::Scissor);
    }

    #[test]
    fn the_new_webbing_types_are_connected_and_symmetric() {
        for kind in [TrussType::DoubleFink, TrussType::DoubleHowe] {
            let t = Truss::generate(&TrussSpec::new(kind, 360.0, 8.0));
            assert!(t.members.len() > 15, "{kind:?}");
            assert!(t.envelope.area() > 0.0);
            assert!(mirrored(&t, 360.0), "{kind:?}");
        }
    }

    #[test]
    fn require_kingpost_adds_a_vertical_web_where_there_is_none() {
        // A Fink has no vertical post; the king post type already has one.
        let mut f = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        let before = Truss::generate(&f).members.len();
        f.require_kingpost = true;
        let t = Truss::generate(&f);
        assert_eq!(t.members.len(), before + 1);
        assert!(t
            .members
            .iter()
            .any(|m| m.role == TrussRole::Web && (m.a.x - 144.0).abs() < 1e-6 && (m.b.x - 144.0).abs() < 1e-6));
        let mut k = TrussSpec::new(TrussType::KingPost, 288.0, 6.0);
        let kp = Truss::generate(&k).members.len();
        k.require_kingpost = true;
        assert_eq!(Truss::generate(&k).members.len(), kp);
    }

    #[test]
    fn an_end_truss_has_verticals_at_the_stud_spacing_and_takes_blocking() {
        let mut spec = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        spec.end_truss = true;
        let t = Truss::generate(&spec);
        assert_eq!(count(&t, TrussRole::BottomChord), 1);
        let verticals: Vec<_> = t
            .members
            .iter()
            .filter(|m| m.role == TrussRole::Web)
            .collect();
        // 16" o.c. across 288": 17 interior studs.
        assert_eq!(verticals.len(), 17);
        assert!(verticals.iter().all(|m| (m.a.x - m.b.x).abs() < 1e-9));
        spec.horizontal_blocking = true;
        spec.block_spacing = 24.0;
        let blocked = Truss::generate(&spec);
        assert!(blocked.members.len() > t.members.len());
        // Rows start one spacing up (Automatic rollout) and are horizontal.
        let rows: Vec<_> = blocked
            .members
            .iter()
            .filter(|m| m.role == TrussRole::Web && (m.a.y - m.b.y).abs() < 1e-9)
            .collect();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|m| (m.a.y % 24.0).abs() < 1e-9));
        // A manual rollout offset moves the first row.
        spec.rollout_auto = false;
        spec.rollout_offset = 10.0;
        let offset = Truss::generate(&spec);
        assert!(offset
            .members
            .iter()
            .any(|m| m.role == TrussRole::Web && (m.a.y - 10.0).abs() < 1e-9 && (m.a.y - m.b.y).abs() < 1e-9));
    }

    #[test]
    fn reduced_gable_and_drop_hip_lower_the_top_and_energy_heel_raises_the_heel() {
        let plain = Truss::generate(&TrussSpec::new(TrussType::Fink, 288.0, 6.0));
        let mut g = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        g.reduced_gable = true;
        let gable = Truss::generate(&g);
        assert!(gable.envelope.peak_height < plain.envelope.peak_height);
        assert!(gable.envelope.overall_width < plain.envelope.overall_width);
        assert!((gable.envelope.max_x - gable.envelope.min_x - 288.0).abs() < 4.0);
        let mut d = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        d.drop_hip = true;
        let drop = Truss::generate(&d);
        assert!((plain.envelope.peak_height - drop.envelope.peak_height - 7.25).abs() < 1.0);
        let mut e = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        e.energy_heel = true;
        let heel = Truss::generate(&e);
        assert!(heel.envelope.heel_height >= ENERGY_HEEL_RAISE - 1e-9);
        // The vertical heel members appear over the walls.
        assert_eq!(heel.members.len(), plain.members.len() + 2);
    }

    #[test]
    fn a_sloping_flat_truss_has_parallel_chords_and_a_bottom_chord_depth_can_differ() {
        let mut s = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        s.sloping_flat = true;
        s.flat_depth = 12.0;
        let t = Truss::generate(&s);
        let bottoms: Vec<_> = t
            .members
            .iter()
            .filter(|m| m.role == TrussRole::BottomChord)
            .collect();
        assert_eq!(bottoms.len(), 8);
        // The bottom chord climbs with the roof: it is not flat.
        assert!(bottoms.iter().any(|m| (m.a.y - m.b.y).abs() > 1.0));
        assert!(mirrored(&t, 288.0));
        let mut b = TrussSpec::new(TrussType::Fink, 288.0, 6.0);
        b.bottom_chord_depth = 5.5;
        let t = Truss::generate(&b);
        let bottom = t
            .members
            .iter()
            .find(|m| m.role == TrussRole::BottomChord)
            .unwrap();
        assert!((bottom.lumber.depth - 5.5).abs() < 1e-9);
        let top = t.members.iter().find(|m| m.role == TrussRole::TopChord).unwrap();
        assert!((top.lumber.depth - 3.5).abs() < 1e-9);
    }

    #[test]
    fn old_truss_specs_read_with_the_new_fields_at_their_defaults() {
        let json = r#"{"kind":"Fink","span":288.0,"pitch":6.0,"heel_height":0.0,"overhang":12.0,"plies":1,
            "chord":{"thickness":1.5,"depth":3.5},"web":{"thickness":1.5,"depth":3.5},
            "bottom_pitch":0.0,"attic_width":0.0}"#;
        let spec: TrussSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec, TrussSpec::new(TrussType::Fink, 288.0, 6.0));
        assert!(spec.special_snapping && spec.calc_chords && spec.rollout_auto);
        let back: TrussSpec = serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        assert_eq!(back, spec);
    }

    fn roof_truss(id: Id, span: f64, at_y: f64) -> FramingMember {
        let mut m = FramingMember::new(
            id,
            ManualKind::RoofTruss,
            Point::new(0.0, at_y),
            Point::new(span, at_y),
        );
        m.truss = Some(TrussSpec::new(TrussType::Fink, span, 6.0));
        m
    }

    #[test]
    fn three_identical_trusses_share_one_label_and_a_different_one_gets_the_next() {
        let members = vec![
            roof_truss(1, 288.0, 0.0),
            roof_truss(2, 288.0, 24.0),
            roof_truss(3, 288.0, 48.0),
            roof_truss(4, 240.0, 72.0),
            FramingMember::new(
                5,
                ManualKind::FloorCeilingTruss,
                Point::new(0.0, 0.0),
                Point::new(200.0, 0.0),
            ),
            FramingMember::new(
                6,
                ManualKind::FloorCeilingTruss,
                Point::new(0.0, 16.0),
                Point::new(200.0, 16.0),
            ),
            FramingMember::new(
                7,
                ManualKind::Joist,
                Point::new(0.0, 16.0),
                Point::new(200.0, 16.0),
            ),
        ];
        let configs = truss_configs(&members, &[]);
        assert_eq!(configs.len(), 3);
        assert_eq!((configs[0].label.as_str(), configs[0].count), ("TR-1", 3));
        assert_eq!((configs[1].label.as_str(), configs[1].count), ("TR-2", 1));
        assert_eq!((configs[2].label.as_str(), configs[2].count), ("FTR-1", 2));
        assert!(configs[2].floor && !configs[0].floor);
        assert_eq!(configs[0].ids, vec![1, 2, 3]);
        let labels = truss_labels(&members, &[]);
        assert_eq!(labels.len(), 6);
        assert!(labels.iter().filter(|(_, l)| l == "TR-1").count() == 3);
        // A ply makes a different configuration; a joist is not a truss.
        let mut girder = roof_truss(8, 288.0, 96.0);
        girder.plies = 3;
        let mut all = members.clone();
        all.push(girder);
        assert_eq!(truss_configs(&all, &[]).len(), 4);
    }

    #[test]
    fn the_truss_schedule_has_one_row_per_configuration_with_its_quantity() {
        let members = vec![
            roof_truss(1, 288.0, 0.0),
            roof_truss(2, 288.0, 24.0),
            roof_truss(3, 240.0, 48.0),
        ];
        let rows = truss_schedule(&members, &[]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "TR-1");
        assert_eq!(rows[0].quantity, 2);
        assert_eq!(rows[0].description, "Fink roof truss");
        assert!((rows[0].span - 288.0).abs() < 1e-9);
        assert_eq!(rows[0].members, "2x4 / 2x4 / 2x4");
        assert_eq!(rows[1].label, "TR-2");
        assert_eq!(rows[1].quantity, 1);
    }
}
