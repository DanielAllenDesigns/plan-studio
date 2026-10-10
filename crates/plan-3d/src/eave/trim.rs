//! Roof trim as parts (manual pp. 831 to 834, 861 to 879; RF-15, RF-26,
//! RF-52 and the roof trim rows of docs/parity/roofs.md).
//!
//! Chief makes the pieces of an eave as molding polylines on the Roofs
//! layers: Build Roof generates them, the Materials List counts them, and
//! editing one removes its Automatic flag. [`roof_trim_lines`] lists them for
//! a set of roof planes as [`TrimLine`]s: a polyline (plan points with the
//! elevation of the bottom edge at each, a 3D molding line), the profile and
//! its size, the side the profile projects to and the mitre at every joint.
//! The app turns each into a `MoldingLine` (`tools::roof_trim`), whose sweep
//! cuts the same mitres.
//!
//! # The parts
//!
//! * **Rafter tails**: a short member every `rafter_spacing` inches under
//!   each eave, from the eave tip back to the wall face and on to the
//!   rafter plate, plus `extend_past_subfascia` beyond the subfascia. The
//!   recipe says how much shows: Exposed (the tails show and no soffit
//!   closes them in), Hidden (a boxed soffit covers them and none is made)
//!   or Partially Exposed (the tails show past a soffit at the wall, only
//!   the last `exposed_length` inches). Stretch to Fit Rafter sizes a tail
//!   like the roof's rafter; otherwise it has its own size.
//! * **Ridge caps** on ridges and hips: one strip per plane lying on its
//!   surface, which is the cap bent to the roof pitch (Bend to Roof Pitch),
//!   or a single level strip centred on the line without it. The Ridge Cap
//!   setting of an edge (Automatic, On, Off) overrides where caps go:
//!   Automatic puts them on ridges and hips only.
//! * **Gutters** along the eaves that do not slope.
//! * **Frieze** under the eaves and under the gable overhangs, against the
//!   wall face at the underside of the roof structure.
//! * **Shadow boards** on the face of the fascia along the eaves and rakes.
//! * **Subfascia** behind the fascia, **lookouts** that carry the rake
//!   overhangs.
//!
//! Soffits are surfaces, not trim; [`SoffitStyle`] and [`soffit_boxed`] say
//! whether an eave gets a horizontal (boxed) soffit or one that follows the
//! rafters (flush).

use super::{plan, EavePlane, V3d};
use crate::cover::RoofDetail;
use plan_core::details::MoldingSide;
use plan_core::geometry::Point;
use plan_roof::{classify_edges, EdgeRole};
use serde::{Deserialize, Serialize};

/// Joins of chain ends closer than this (inches) are one corner.
const JOIN_TOL: f64 = 0.5;
/// Edges shorter than this get no trim, inches.
const MIN_EDGE: f64 = 1.0;
/// Thickness of a ridge cap strip, inches.
const CAP_THICKNESS: f64 = 0.75;
/// How far a ridge cap stands off the roof surface, inches.
const CAP_LIFT: f64 = 0.25;
/// Gutters hang this far below the eave top, inches.
const GUTTER_DROP: f64 = 1.0;
/// Turning angles under this (degrees) are straight runs.
const STRAIGHT_DEG: f64 = 0.5;

/// What a trim part is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrimKind {
    RafterTail,
    RidgeCap,
    Gutter,
    Frieze,
    ShadowBoard,
    Subfascia,
    Lookout,
}

impl TrimKind {
    pub const ALL: [TrimKind; 7] = [
        TrimKind::RafterTail,
        TrimKind::RidgeCap,
        TrimKind::Gutter,
        TrimKind::Frieze,
        TrimKind::ShadowBoard,
        TrimKind::Subfascia,
        TrimKind::Lookout,
    ];

    /// The name the Materials List and the molding's label use.
    pub fn label(self) -> &'static str {
        match self {
            TrimKind::RafterTail => "Rafter Tail",
            TrimKind::RidgeCap => "Ridge Cap",
            TrimKind::Gutter => "Gutter",
            TrimKind::Frieze => "Frieze",
            TrimKind::ShadowBoard => "Shadow Board",
            TrimKind::Subfascia => "Subfascia",
            TrimKind::Lookout => "Lookout",
        }
    }
}

/// How much of the rafter tails shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RafterTailRecipe {
    /// Tails show under the overhang; no soffit.
    #[default]
    Exposed,
    /// No tails: a boxed soffit covers the overhang.
    Hidden,
    /// A soffit at the wall; only the ends of the tails show.
    PartiallyExposed,
}

/// The Ridge Cap setting of one roof edge (Roof Plane Specification, On
/// Selected Edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RidgeCapEdge {
    /// Ridges and hips get caps, other edges do not.
    #[default]
    Automatic,
    On,
    Off,
}

/// Boxed eaves have a horizontal soffit under the fascia; flush eaves a
/// soffit that follows the underside of the rafters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SoffitStyle {
    #[default]
    Boxed,
    Flush,
}

/// Does an eave at `eave_y` get a horizontal soffit? Boxed always; Flush
/// unless Higher Eaves Boxed is on and the eave is higher than `lowest_y`
/// (the roof's lowest eave).
pub fn soffit_boxed(style: SoffitStyle, higher_boxed: bool, eave_y: f64, lowest_y: f64) -> bool {
    match style {
        SoffitStyle::Boxed => true,
        SoffitStyle::Flush => higher_boxed && eave_y > lowest_y + 0.5,
    }
}

/// Profile and size of one kind of trim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrimSpec {
    pub enabled: bool,
    /// Name of a molding profile of the library; empty is a plain board.
    pub profile: String,
    /// Projection from the line, inches.
    pub width: f64,
    /// Vertical size, inches.
    pub height: f64,
}

impl TrimSpec {
    pub fn new(enabled: bool, profile: &str, width: f64, height: f64) -> Self {
        Self {
            enabled,
            profile: profile.to_string(),
            width,
            height,
        }
    }
}

impl Default for TrimSpec {
    fn default() -> Self {
        Self::new(false, "", 0.75, 3.5)
    }
}

/// Ridge Caps panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RidgeCapSpec {
    pub spec: TrimSpec,
    /// Bend to Roof Pitch: one strip on each plane; off is one level strip.
    pub bend_to_pitch: bool,
}

impl Default for RidgeCapSpec {
    fn default() -> Self {
        Self {
            // Width is the whole cap, across both planes.
            spec: TrimSpec::new(false, "", 10.0, CAP_THICKNESS),
            bend_to_pitch: true,
        }
    }
}

/// Rafter Tails panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RafterTailSpec {
    pub enabled: bool,
    pub recipe: RafterTailRecipe,
    pub profile: String,
    /// Stretch to Fit Rafter: the tail is as wide and deep as the rafter.
    pub stretch_to_fit: bool,
    /// Own size when not stretched, inches.
    pub width: f64,
    pub height: f64,
    /// How far the tail goes on past the subfascia, inches.
    pub extend_past_subfascia: f64,
    /// Partially Exposed: how much of the tail shows beyond the soffit.
    pub exposed_length: f64,
}

impl Default for RafterTailSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            recipe: RafterTailRecipe::Exposed,
            profile: String::new(),
            stretch_to_fit: true,
            width: 1.5,
            height: 5.5,
            extend_past_subfascia: 0.0,
            exposed_length: 6.0,
        }
    }
}

/// Every setting of the roof trim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofTrimOptions {
    pub rafter_tails: RafterTailSpec,
    pub ridge_caps: RidgeCapSpec,
    pub gutters: TrimSpec,
    pub frieze: TrimSpec,
    pub shadow_boards: TrimSpec,
    pub subfascia: TrimSpec,
    pub lookouts: TrimSpec,
    /// Distance between lookouts along a rake, inches.
    pub lookout_spacing: f64,
    pub soffit: SoffitStyle,
    pub higher_eaves_boxed: bool,
    /// Trim Framing To Soffits: roof framing is cut back to the soffit.
    pub trim_framing_to_soffits: bool,
    /// The Ridge Cap setting of single edges: `(plane, edge)` as the plane
    /// list and [`plan_roof::PlaneEdge::index`] number them.
    pub edge_caps: Vec<(usize, usize, RidgeCapEdge)>,
}

impl Default for RoofTrimOptions {
    fn default() -> Self {
        Self {
            rafter_tails: RafterTailSpec::default(),
            ridge_caps: RidgeCapSpec::default(),
            gutters: TrimSpec::new(false, "", 4.0, 5.0),
            frieze: TrimSpec::new(false, "", 0.75, 5.5),
            shadow_boards: TrimSpec::new(false, "", 0.75, 1.5),
            subfascia: TrimSpec::new(false, "", 1.5, 7.25),
            lookouts: TrimSpec::new(false, "", 1.5, 3.5),
            lookout_spacing: 24.0,
            soffit: SoffitStyle::Boxed,
            higher_eaves_boxed: false,
            trim_framing_to_soffits: false,
            edge_caps: Vec::new(),
        }
    }
}

impl RoofTrimOptions {
    /// Is any trim part switched on?
    pub fn any(&self) -> bool {
        self.rafter_tails.enabled
            || self.ridge_caps.spec.enabled
            || self.gutters.enabled
            || self.frieze.enabled
            || self.shadow_boards.enabled
            || self.subfascia.enabled
            || self.lookouts.enabled
    }

    fn cap_edge(&self, plane: usize, edge: usize) -> RidgeCapEdge {
        self.edge_caps
            .iter()
            .find(|(p, e, _)| *p == plane && *e == edge)
            .map_or(RidgeCapEdge::Automatic, |c| c.2)
    }
}

/// One generated trim part: a molding polyline.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimLine {
    pub kind: TrimKind,
    /// The first plane the line follows.
    pub plane: usize,
    /// Plan points of the line.
    pub points: Vec<Point>,
    /// Elevation of the molding's bottom edge at each point, inches.
    pub bottoms: Vec<f64>,
    pub closed: bool,
    /// Which side of the drawing direction the profile projects to.
    pub side: MoldingSide,
    pub profile: String,
    /// Projection and vertical size, inches.
    pub width: f64,
    pub height: f64,
    /// A custom section `(projection, height)` that replaces the profile:
    /// the strip of a bent ridge cap lying on its plane.
    pub section: Option<Vec<Point>>,
    /// The mitre at each point, degrees from a square cut (half the turn of
    /// the line there); zero at the ends of an open line.
    pub miter_deg: Vec<f64>,
}

impl TrimLine {
    /// Length of the line in 3D, inches (what the Materials List counts).
    pub fn length(&self) -> f64 {
        let n = self.points.len();
        let segs = if self.closed { n } else { n.saturating_sub(1) };
        (0..segs)
            .map(|i| {
                let (a, b) = (self.points[i], self.points[(i + 1) % n]);
                let dz = self.bottoms[(i + 1) % n] - self.bottoms[i];
                a.dist(b).hypot(dz)
            })
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Chains of edges
// ---------------------------------------------------------------------------

/// An edge of a plane as the trim sees it: roof-space ends.
#[derive(Debug, Clone, Copy)]
struct Edge {
    plane: usize,
    a: V3d,
    b: V3d,
}

fn near(a: Point, b: Point) -> bool {
    a.dist(b) < JOIN_TOL
}

/// A run of joined edges: its vertices (roof space), whether it comes back
/// to its start and the plane of each edge (`planes[i]` owns the edge from
/// vertex `i` to the next).
struct Run {
    verts: Vec<V3d>,
    closed: bool,
    planes: Vec<usize>,
}

/// Joins `edges` end to start into runs.
fn chains(edges: &[Edge]) -> Vec<Run> {
    let mut left: Vec<Edge> = edges.to_vec();
    let mut out = Vec::new();
    while !left.is_empty() {
        let first = left.remove(0);
        let mut verts = vec![first.a, first.b];
        let mut planes = vec![first.plane];
        loop {
            let tail = plan(verts[verts.len() - 1]);
            let Some(k) = left.iter().position(|e| near(plan(e.a), tail)) else {
                break;
            };
            let e = left.remove(k);
            verts.push(e.b);
            planes.push(e.plane);
        }
        // Extend backwards too.
        loop {
            let head = plan(verts[0]);
            let Some(k) = left.iter().position(|e| near(plan(e.b), head)) else {
                break;
            };
            let e = left.remove(k);
            verts.insert(0, e.a);
            planes.insert(0, e.plane);
        }
        let closed = verts.len() >= 4 && near(plan(verts[0]), plan(verts[verts.len() - 1]));
        if closed {
            verts.pop();
        }
        out.push(Run {
            verts,
            closed,
            planes,
        });
    }
    out
}

/// Half the turn of a line through `p` coming from `a` and going to `b`,
/// degrees (the mitre of a corner).
fn mitre_at(a: Point, p: Point, b: Point) -> f64 {
    let (d1, d2) = (p.sub(a).normalized(), b.sub(p).normalized());
    let turn = d1.cross(d2).atan2(d1.dot(d2)).to_degrees().abs();
    if turn < STRAIGHT_DEG {
        0.0
    } else {
        turn * 0.5
    }
}

fn mitres(points: &[Point], closed: bool) -> Vec<f64> {
    let n = points.len();
    (0..n)
        .map(|i| {
            if !closed && (i == 0 || i == n - 1) {
                0.0
            } else {
                mitre_at(points[(i + n - 1) % n], points[i], points[(i + 1) % n])
            }
        })
        .collect()
}

/// `points` moved `dist` to their left (inward for a counter-clockwise
/// chain), corners moved along the bisector so every edge keeps its
/// direction. The ends of an open chain move straight sideways.
fn offset_chain(points: &[Point], closed: bool, dist: f64) -> Vec<Point> {
    let n = points.len();
    let seg = |i: usize| {
        let (a, b) = (points[i % n], points[(i + 1) % n]);
        let d = b.sub(a).normalized();
        (a.add(d.perp().scale(dist)), d)
    };
    (0..n)
        .map(|i| {
            let prev = if closed || i > 0 {
                Some(seg((i + n - 1) % n))
            } else {
                None
            };
            let next = if closed || i + 1 < n {
                Some(seg(i))
            } else {
                None
            };
            match (prev, next) {
                (Some((pa, pd)), Some((na, nd))) => {
                    let den = pd.cross(nd);
                    if den.abs() < 1e-9 {
                        na
                    } else {
                        pa.add(pd.scale(na.sub(pa).cross(nd) / den))
                    }
                }
                (None, Some((na, _))) => na,
                (Some((pa, pd)), None) => pa.add(pd.scale(points[i].sub(pa).dot(pd))),
                (None, None) => points[i],
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

/// All the roof trim `opts` switches on for `planes`. `detail` supplies the
/// structure thickness, the fascia and the rafters; a plane's own eave
/// choices (frieze, gutters, rafter tails off for that plane) are honoured.
pub fn roof_trim_lines(
    planes: &[EavePlane],
    detail: &RoofDetail,
    opts: &RoofTrimOptions,
) -> Vec<TrimLine> {
    let roofs: Vec<_> = planes.iter().map(|p| p.plane.clone()).collect();
    let edges = classify_edges(&roofs);
    let mut out: Vec<TrimLine> = Vec::new();

    // Eave and rake edges, per overhang class.
    let mut eaves: Vec<Edge> = Vec::new();
    let mut rakes: Vec<Edge> = Vec::new();
    for e in &edges {
        let ep = &planes[e.plane];
        if plan(e.a).dist(plan(e.b)) < MIN_EDGE || ep.skip.contains(&e.index) {
            continue;
        }
        let edge = Edge {
            plane: e.plane,
            a: e.a,
            b: e.b,
        };
        match e.role {
            EdgeRole::Eave => eaves.push(edge),
            EdgeRole::Rake => rakes.push(edge),
            _ => {}
        }
    }

    eave_trim(planes, detail, opts, &eaves, &mut out);
    rake_trim(planes, detail, opts, &rakes, &mut out);
    ridge_caps(planes, opts, &edges, &mut out);
    rafter_tails(planes, detail, opts, &eaves, &mut out);
    out
}

fn plane_detail(planes: &[EavePlane], k: usize, detail: &RoofDetail) -> RoofDetail {
    planes[k].opts.apply(detail)
}

fn planes_detail(planes: &[EavePlane], k: usize, detail: &RoofDetail) -> RoofDetail {
    plane_detail(planes, k, detail)
}

/// A line along `verts` (roof space) offset to the left by `inset` inches,
/// with the bottom edge `drop` below the vertices.
#[allow(clippy::too_many_arguments)]
fn along(
    kind: TrimKind,
    plane: usize,
    verts: &[V3d],
    closed: bool,
    inset: f64,
    drop: f64,
    side: MoldingSide,
    spec: &TrimSpec,
) -> TrimLine {
    let flat: Vec<Point> = verts.iter().map(|v| plan(*v)).collect();
    let points = if inset.abs() < 1e-9 {
        flat
    } else {
        offset_chain(&flat, closed, inset)
    };
    TrimLine {
        kind,
        plane,
        miter_deg: mitres(&points, closed),
        points,
        bottoms: verts.iter().map(|v| v[1] - drop).collect(),
        closed,
        side,
        profile: spec.profile.clone(),
        width: spec.width,
        height: spec.height,
        section: None,
    }
}

fn eave_trim(
    planes: &[EavePlane],
    detail: &RoofDetail,
    opts: &RoofTrimOptions,
    eaves: &[Edge],
    out: &mut Vec<TrimLine>,
) {
    for run in chains(eaves) {
        let Run {
            verts,
            closed,
            planes: owners,
        } = run;
        let plane = owners[0];
        let d = plane_detail(planes, plane, detail);
        let ov = planes[plane].overhang;
        // The eave chain runs counter-clockwise round the plane: its inside
        // is to the left, the eave overhang and the fascia to the right.
        if opts.gutters.enabled && planes[plane].opts.gutters != Some(false) {
            // Gutters hang on level eaves only.
            let level = verts.windows(2).all(|w| (w[0][1] - w[1][1]).abs() < 0.01);
            if level {
                out.push(along(
                    TrimKind::Gutter,
                    plane,
                    &verts,
                    closed,
                    0.0,
                    GUTTER_DROP + opts.gutters.height,
                    MoldingSide::Right,
                    &opts.gutters,
                ));
            }
        }
        if opts.shadow_boards.enabled && d.fascia {
            out.push(along(
                TrimKind::ShadowBoard,
                plane,
                &verts,
                closed,
                0.0,
                d.fascia_height,
                MoldingSide::Right,
                &opts.shadow_boards,
            ));
        }
        if opts.subfascia.enabled {
            // Behind the fascia: set in by its thickness, projecting inward.
            out.push(along(
                TrimKind::Subfascia,
                plane,
                &verts,
                closed,
                d.fascia_thickness.max(0.0),
                opts.subfascia.height,
                MoldingSide::Left,
                &opts.subfascia,
            ));
        }
        if opts.frieze.enabled && planes[plane].opts.frieze != Some(false) && ov >= 1.0 {
            // On the wall face: the eave moved in by the overhang, at the
            // underside of the roof structure.
            let flat: Vec<Point> = verts.iter().map(|v| plan(*v)).collect();
            let wall = offset_chain(&flat, closed, ov);
            // Each vertex takes the underside of the plane whose edge starts
            // there (the last one of an open run, that of the last edge).
            let tops: Vec<f64> = wall
                .iter()
                .zip(&verts)
                .enumerate()
                .map(|(i, (p, v))| {
                    let owner = owners[i.min(owners.len() - 1)];
                    planes[owner]
                        .plane
                        .underside_at(*p, planes_detail(planes, owner, detail).thickness)
                        .unwrap_or(v[1])
                })
                .collect();
            out.push(TrimLine {
                kind: TrimKind::Frieze,
                plane,
                miter_deg: mitres(&wall, closed),
                points: wall,
                bottoms: tops.iter().map(|t| t - opts.frieze.height).collect(),
                closed,
                side: MoldingSide::Right,
                profile: opts.frieze.profile.clone(),
                width: opts.frieze.width,
                height: opts.frieze.height,
                section: None,
            });
        }
    }
}

fn rake_trim(
    planes: &[EavePlane],
    detail: &RoofDetail,
    opts: &RoofTrimOptions,
    rakes: &[Edge],
    out: &mut Vec<TrimLine>,
) {
    for e in rakes {
        let ep = &planes[e.plane];
        let d = plane_detail(planes, e.plane, detail);
        let verts = [e.a, e.b];
        if opts.shadow_boards.enabled && d.rake_fascia && d.fascia {
            out.push(along(
                TrimKind::ShadowBoard,
                e.plane,
                &verts,
                false,
                0.0,
                d.fascia_height,
                MoldingSide::Right,
                &opts.shadow_boards,
            ));
        }
        if ep.overhang < 1.0 {
            continue;
        }
        let (pa, pb) = (plan(e.a), plan(e.b));
        let dir = pb.sub(pa).normalized();
        let inward = dir.perp();
        let (wa, wb) = (
            pa.add(inward.scale(ep.overhang)),
            pb.add(inward.scale(ep.overhang)),
        );
        let under = |p: Point| ep.plane.underside_at(p, d.thickness);
        let (Some(ua), Some(ub)) = (under(wa), under(wb)) else {
            continue;
        };
        if opts.frieze.enabled && ep.opts.frieze != Some(false) {
            out.push(TrimLine {
                kind: TrimKind::Frieze,
                plane: e.plane,
                points: vec![wa, wb],
                bottoms: vec![ua - opts.frieze.height, ub - opts.frieze.height],
                closed: false,
                side: MoldingSide::Right,
                profile: opts.frieze.profile.clone(),
                width: opts.frieze.width,
                height: opts.frieze.height,
                section: None,
                miter_deg: vec![0.0, 0.0],
            });
        }
        if opts.lookouts.enabled && opts.lookout_spacing > 1.0 {
            let len = pa.dist(pb);
            let count = (len / opts.lookout_spacing).floor() as usize;
            for i in 1..=count {
                let t = i as f64 * opts.lookout_spacing;
                if t > len - 1.0 {
                    break;
                }
                let on_edge = pa.add(dir.scale(t));
                let in_wall = on_edge.add(inward.scale(ep.overhang));
                let (Some(y0), Some(y1)) = (under(on_edge), under(in_wall)) else {
                    continue;
                };
                // A lookout lies under the roof from the rake tip to the wall.
                let h = opts.lookouts.height;
                out.push(TrimLine {
                    kind: TrimKind::Lookout,
                    plane: e.plane,
                    points: vec![on_edge, in_wall],
                    bottoms: vec![y0 - h, y1 - h],
                    closed: false,
                    side: MoldingSide::Left,
                    profile: opts.lookouts.profile.clone(),
                    width: opts.lookouts.width,
                    height: h,
                    section: None,
                    miter_deg: vec![0.0, 0.0],
                });
            }
        }
    }
}

fn ridge_caps(
    planes: &[EavePlane],
    opts: &RoofTrimOptions,
    edges: &[plan_roof::PlaneEdge],
    out: &mut Vec<TrimLine>,
) {
    if !opts.ridge_caps.spec.enabled {
        return;
    }
    let cap = &opts.ridge_caps;
    let half = cap.spec.width * 0.5;
    // A shared edge is capped once: by the plane that comes first.
    let mut done: Vec<(Point, Point)> = Vec::new();
    for e in edges {
        let twin = e.other.and_then(|o| {
            edges
                .iter()
                .find(|t| t.plane == o && near(plan(t.a), plan(e.b)) && near(plan(t.b), plan(e.a)))
        });
        // Off on either side of a shared edge wins, then On.
        let setting = {
            let mine = opts.cap_edge(e.plane, e.index);
            let theirs = twin.map_or(RidgeCapEdge::Automatic, |t| opts.cap_edge(t.plane, t.index));
            if mine == RidgeCapEdge::Off || theirs == RidgeCapEdge::Off {
                RidgeCapEdge::Off
            } else if mine == RidgeCapEdge::On || theirs == RidgeCapEdge::On {
                RidgeCapEdge::On
            } else {
                RidgeCapEdge::Automatic
            }
        };
        let wanted = match setting {
            RidgeCapEdge::Off => false,
            RidgeCapEdge::On => true,
            RidgeCapEdge::Automatic => matches!(e.role, EdgeRole::Ridge | EdgeRole::Hip),
        };
        let (pa, pb) = (plan(e.a), plan(e.b));
        if !wanted || pa.dist(pb) < MIN_EDGE {
            continue;
        }
        let key = if (pa.x, pa.y) <= (pb.x, pb.y) {
            (pa, pb)
        } else {
            (pb, pa)
        };
        let shared = e.other.is_some();
        if shared && done.iter().any(|d| near(d.0, key.0) && near(d.1, key.1)) {
            continue;
        }
        done.push(key);
        // The planes that meet on the edge: this one and the other.
        let mut on: Vec<(usize, Point, Point, V3d, V3d)> = vec![(e.plane, pa, pb, e.a, e.b)];
        if let Some(o) = e.other {
            on.push((o, pb, pa, e.b, e.a));
        }
        if cap.bend_to_pitch {
            for (k, qa, qb, va, vb) in on {
                let pl = &planes[k].plane;
                // This plane's inside is on the left of qa -> qb.
                let inward = qb.sub(qa).normalized().perp();
                let probe = |p: Point| pl.height_at(p.add(inward.scale(half)));
                let (Some(ha), Some(hb)) = (probe(qa), probe(qb)) else {
                    continue;
                };
                let rise = ((ha - va[1]) + (hb - vb[1])) * 0.5;
                let section = vec![
                    Point::new(0.0, 0.0),
                    Point::new(half, rise),
                    Point::new(half, rise + cap.spec.height),
                    Point::new(0.0, cap.spec.height),
                ];
                out.push(TrimLine {
                    kind: TrimKind::RidgeCap,
                    plane: k,
                    points: vec![qa, qb],
                    bottoms: vec![va[1] + CAP_LIFT, vb[1] + CAP_LIFT],
                    closed: false,
                    side: MoldingSide::Left,
                    profile: cap.spec.profile.clone(),
                    width: half,
                    height: cap.spec.height,
                    section: Some(section),
                    miter_deg: vec![0.0, 0.0],
                });
            }
        } else {
            // One level strip centred on the line, its back edge half the
            // width to the right of the line.
            let right = pb.sub(pa).normalized().perp().scale(-half);
            let (qa, qb) = (pa.add(right), pb.add(right));
            out.push(TrimLine {
                kind: TrimKind::RidgeCap,
                plane: e.plane,
                points: vec![qa, qb],
                bottoms: vec![e.a[1] + CAP_LIFT, e.b[1] + CAP_LIFT],
                closed: false,
                side: MoldingSide::Left,
                profile: cap.spec.profile.clone(),
                width: cap.spec.width,
                height: cap.spec.height,
                section: None,
                miter_deg: vec![0.0, 0.0],
            });
        }
    }
}

fn rafter_tails(
    planes: &[EavePlane],
    detail: &RoofDetail,
    opts: &RoofTrimOptions,
    eaves: &[Edge],
    out: &mut Vec<TrimLine>,
) {
    let spec = &opts.rafter_tails;
    if !spec.enabled {
        return;
    }
    for e in eaves {
        let ep = &planes[e.plane];
        let d = plane_detail(planes, e.plane, detail);
        // Exposed tails have no soffit; Hidden ones are behind a soffit (the
        // soffit surface follows `opts.soffit`, see `soffit_boxed`); Partially
        // Exposed ones show past a soffit at the wall.
        let shows = spec.recipe != RafterTailRecipe::Hidden;
        if !shows || ep.overhang < 1.0 || ep.opts.rafter_tails == Some(false) {
            continue;
        }
        let (pa, pb) = (plan(e.a), plan(e.b));
        let dir = pb.sub(pa).normalized();
        let inward = dir.perp();
        let len = pa.dist(pb);
        let (w, h) = if spec.stretch_to_fit {
            (d.rafter_width.max(0.25), d.rafter_depth.max(0.25))
        } else {
            (spec.width.max(0.25), spec.height.max(0.25))
        };
        let spacing = d.rafter_spacing.max(w + 0.25);
        let count = super::rafter_count(len, w, spacing);
        // The part of the overhang that shows.
        let shown = match spec.recipe {
            RafterTailRecipe::PartiallyExposed => spec.exposed_length.clamp(0.0, ep.overhang),
            _ => ep.overhang,
        };
        for i in 0..count {
            let along = i as f64 * spacing + w * 0.5;
            let tip = pa
                .add(dir.scale(along))
                .sub(inward.scale(spec.extend_past_subfascia.max(0.0)));
            let back = pa.add(dir.scale(along)).add(inward.scale(shown));
            let (Some(yt), Some(yb)) = (
                ep.plane.underside_at(pa.add(dir.scale(along)), d.thickness),
                ep.plane.underside_at(back, d.thickness),
            ) else {
                continue;
            };
            let slope = (yb - yt) / shown.max(1e-9);
            out.push(TrimLine {
                kind: TrimKind::RafterTail,
                plane: e.plane,
                points: vec![tip, back],
                bottoms: vec![yt - slope * spec.extend_past_subfascia.max(0.0) - h, yb - h],
                closed: false,
                side: MoldingSide::Left,
                profile: spec.profile.clone(),
                width: w,
                height: h,
                section: None,
                miter_deg: vec![0.0, 0.0],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cover::RoofDetail;
    use plan_roof::{build_roof, EdgeKind, EdgeRoof};

    const W: f64 = 480.0;
    const D: f64 = 288.0;
    const OVER: f64 = 16.0;
    const PLATE: f64 = 96.0;

    fn footprint() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(W, 0.0),
            Point::new(W, D),
            Point::new(0.0, D),
        ]
    }

    fn edge(kind: EdgeKind) -> EdgeRoof {
        EdgeRoof {
            pitch_in_12: 8.0,
            kind,
            overhang: OVER,
        }
    }

    fn eave_planes(kinds: [EdgeKind; 4]) -> Vec<EavePlane> {
        let e = kinds.map(edge);
        build_roof(&footprint(), &e, PLATE)
            .planes
            .into_iter()
            .map(EavePlane::bare)
            .map(|mut p| {
                p.overhang = OVER;
                p.ridge_caps = true;
                p
            })
            .collect()
    }

    fn hip() -> Vec<EavePlane> {
        eave_planes([EdgeKind::Hip; 4])
    }

    fn gable() -> Vec<EavePlane> {
        eave_planes([
            EdgeKind::Hip,
            EdgeKind::Gable,
            EdgeKind::Hip,
            EdgeKind::Gable,
        ])
    }

    fn only(f: impl FnOnce(&mut RoofTrimOptions)) -> RoofTrimOptions {
        let mut o = RoofTrimOptions::default();
        f(&mut o);
        o
    }

    fn kind(lines: &[TrimLine], k: TrimKind) -> Vec<&TrimLine> {
        lines.iter().filter(|l| l.kind == k).collect()
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn nothing_is_made_until_a_part_is_switched_on() {
        let o = RoofTrimOptions::default();
        assert!(!o.any());
        assert!(roof_trim_lines(&hip(), &RoofDetail::default(), &o).is_empty());
    }

    #[test]
    fn the_gutter_runs_round_the_eaves_in_one_loop_with_45_degree_mitres() {
        let o = only(|o| o.gutters.enabled = true);
        let lines = roof_trim_lines(&hip(), &RoofDetail::default(), &o);
        let g = kind(&lines, TrimKind::Gutter);
        assert_eq!(g.len(), 1, "one closed run");
        let g = g[0];
        assert!(g.closed);
        assert_eq!(g.points.len(), 4);
        assert!(
            g.miter_deg.iter().all(|m| near(*m, 45.0)),
            "{:?}",
            g.miter_deg
        );
        // The eave rectangle is the footprint grown by the overhang.
        assert!(near(
            g.length(),
            2.0 * ((W + 2.0 * OVER) + (D + 2.0 * OVER))
        ));
        // It hangs below the eave and projects away from the building.
        assert_eq!(g.side, MoldingSide::Right);
        assert!(g.bottoms.iter().all(|b| *b < PLATE));
    }

    #[test]
    fn gutters_skip_eaves_that_slope_and_planes_that_turn_them_off() {
        // A hand-made plane whose eave rises from one end to the other.
        let plane = plan_roof::RoofPlane {
            polygon3d: vec![
                [0.0, 100.0, 0.0],
                [200.0, 130.0, 0.0],
                [200.0, 180.0, -100.0],
                [0.0, 150.0, -100.0],
            ],
            pitch_in_12: 6.0,
            baseline: (Point::new(0.0, 0.0), Point::new(200.0, 0.0)),
            source_edge: 0,
        };
        let mut ep = EavePlane::bare(plane);
        ep.overhang = 12.0;
        let o = only(|o| o.gutters.enabled = true);
        let d = RoofDetail::default();
        let lines = roof_trim_lines(&[ep], &d, &o);
        assert!(kind(&lines, TrimKind::Gutter).is_empty(), "sloping eave");

        let mut planes = gable();
        planes[0].opts.gutters = Some(false);
        let lines = roof_trim_lines(&planes, &d, &o);
        // The gable's other eave still gets one.
        assert_eq!(kind(&lines, TrimKind::Gutter).len(), 1);
        assert_ne!(kind(&lines, TrimKind::Gutter)[0].plane, 0);
    }

    #[test]
    fn the_frieze_is_the_wall_face_loop_at_the_underside_of_the_roof() {
        let o = only(|o| o.frieze.enabled = true);
        let d = RoofDetail::default();
        let lines = roof_trim_lines(&hip(), &d, &o);
        let f = kind(&lines, TrimKind::Frieze);
        assert_eq!(f.len(), 1);
        let f = f[0];
        // The eave pulled in by the overhang is the footprint again.
        assert!(near(f.length(), 2.0 * (W + D)), "{}", f.length());
        for p in &f.points {
            let on_wall = near(p.x.min(W - p.x).min(p.y).min(D - p.y), 0.0);
            assert!(on_wall, "{p:?} is on the wall line");
        }
        assert!(f.miter_deg.iter().all(|m| near(*m, 45.0)));
        // Bottom edge is the frieze height under the roof structure.
        let plane = &hip()[0].plane;
        let under = plane.underside_at(f.points[0], d.thickness).unwrap();
        assert!(near(f.bottoms[0], under - o.frieze.height) || f.plane != 0);
        assert!(f.bottoms.iter().all(|b| *b < PLATE + 6.0));
    }

    #[test]
    fn a_gable_overhang_gets_a_frieze_along_the_sloping_rake() {
        let o = only(|o| o.frieze.enabled = true);
        let lines = roof_trim_lines(&gable(), &RoofDetail::default(), &o);
        let f = kind(&lines, TrimKind::Frieze);
        // Two eaves (open runs) and two rakes on each gable end.
        let rakes: Vec<&&TrimLine> = f
            .iter()
            .filter(|l| l.points.len() == 2 && l.bottoms[0] != l.bottoms[1])
            .collect();
        assert!(rakes.len() >= 2, "{} sloping friezes", rakes.len());
        for r in rakes {
            // The rake rises toward the ridge, so does the frieze.
            let rise = (r.bottoms[1] - r.bottoms[0]).abs();
            let run = r.points[0].dist(r.points[1]);
            assert!(near(rise / run, 8.0 / 12.0), "slope {}", rise / run);
        }
    }

    #[test]
    fn shadow_boards_follow_the_fascia_and_subfascia_sits_behind_it() {
        let d = RoofDetail::default();
        let o = only(|o| {
            o.shadow_boards.enabled = true;
            o.subfascia.enabled = true;
        });
        let lines = roof_trim_lines(&hip(), &d, &o);
        let s = kind(&lines, TrimKind::ShadowBoard);
        assert_eq!(s.len(), 1);
        // Same run as the eave: the bottom is the fascia height below it.
        assert!(near(s[0].bottoms[0], PLATE - d.fascia_height) || s[0].bottoms[0] < PLATE);
        assert_eq!(s[0].side, MoldingSide::Right);
        let sub = kind(&lines, TrimKind::Subfascia);
        assert_eq!(sub.len(), 1);
        assert_eq!(sub[0].side, MoldingSide::Left);
        // Set in from the eave by the fascia thickness.
        let inset = (W + 2.0 * OVER) - (sub[0].points[1].x - sub[0].points[0].x);
        assert!(near(inset, 2.0 * d.fascia_thickness), "{inset}");
        // With the fascia off no shadow board is made.
        let d2 = RoofDetail { fascia: false, ..d };
        assert!(kind(&roof_trim_lines(&hip(), &d2, &o), TrimKind::ShadowBoard).is_empty());
    }

    #[test]
    fn lookouts_stand_along_the_rake_at_their_spacing() {
        let o = only(|o| {
            o.lookouts.enabled = true;
            o.lookout_spacing = 24.0;
        });
        let lines = roof_trim_lines(&gable(), &RoofDetail::default(), &o);
        let l = kind(&lines, TrimKind::Lookout);
        assert!(!l.is_empty());
        // Each runs from the rake tip into the wall face: the overhang long.
        for x in &l {
            assert!(near(x.points[0].dist(x.points[1]), OVER));
        }
        // Spacing along the rake (the gable end spans the building depth).
        let per_rake = l.len() / 4;
        assert!(per_rake >= 3, "{} lookouts per rake", per_rake);
    }

    #[test]
    fn bent_ridge_caps_lie_on_both_planes_and_level_ones_make_one_strip() {
        let d = RoofDetail::default();
        let mut o = only(|o| o.ridge_caps.spec.enabled = true);
        let lines = roof_trim_lines(&hip(), &d, &o);
        let caps = kind(&lines, TrimKind::RidgeCap);
        // A hip roof on a rectangle has a ridge and four hips, two planes each.
        assert_eq!(caps.len(), 10);
        for c in &caps {
            let s = c.section.as_ref().expect("a bent cap has its own section");
            assert_eq!(s.len(), 4);
            assert!(near(s[1].x, 5.0), "half of the 10\" cap");
            assert!(near(s[2].y - s[1].y, CAP_THICKNESS));
        }
        // Along the ridge the strip falls half the width times the pitch.
        let ridge: Vec<_> = caps
            .iter()
            .filter(|c| near(c.bottoms[0], c.bottoms[1]))
            .collect();
        assert_eq!(ridge.len(), 2);
        for c in ridge {
            let rise = c.section.as_ref().unwrap()[1].y;
            assert!(
                near(rise, -5.0 * 8.0 / 12.0),
                "falls away from the ridge: {rise}"
            );
        }
        o.ridge_caps.bend_to_pitch = false;
        let level = roof_trim_lines(&hip(), &d, &o);
        let caps = kind(&level, TrimKind::RidgeCap);
        assert_eq!(caps.len(), 5, "one strip per ridge or hip");
        assert!(caps
            .iter()
            .all(|c| c.section.is_none() && near(c.width, 10.0)));
    }

    #[test]
    fn the_ridge_cap_setting_of_an_edge_overrides_automatic() {
        let d = RoofDetail::default();
        let planes = hip();
        let edges = classify_edges(&planes.iter().map(|p| p.plane.clone()).collect::<Vec<_>>());
        let ridge = edges.iter().find(|e| e.role == EdgeRole::Ridge).unwrap();
        let eave = edges
            .iter()
            .find(|e| e.role == EdgeRole::Eave && e.plane == 0)
            .unwrap();
        let mut o = only(|o| {
            o.ridge_caps.spec.enabled = true;
            o.ridge_caps.bend_to_pitch = false;
        });
        let base = kind(&roof_trim_lines(&planes, &d, &o), TrimKind::RidgeCap).len();
        // Off on the ridge: one cap fewer.
        o.edge_caps = vec![(ridge.plane, ridge.index, RidgeCapEdge::Off)];
        assert_eq!(
            kind(&roof_trim_lines(&planes, &d, &o), TrimKind::RidgeCap).len(),
            base - 1
        );
        // On along an eave: one more.
        o.edge_caps = vec![(eave.plane, eave.index, RidgeCapEdge::On)];
        assert_eq!(
            kind(&roof_trim_lines(&planes, &d, &o), TrimKind::RidgeCap).len(),
            base + 1
        );
    }

    #[test]
    fn rafter_tail_recipes_decide_what_shows() {
        let d = RoofDetail::default();
        let make = |recipe: RafterTailRecipe, extend: f64| {
            let o = only(|o| {
                o.rafter_tails.enabled = true;
                o.rafter_tails.recipe = recipe;
                o.rafter_tails.extend_past_subfascia = extend;
            });
            roof_trim_lines(&hip(), &d, &o)
        };
        let exposed = make(RafterTailRecipe::Exposed, 0.0);
        let tails = kind(&exposed, TrimKind::RafterTail);
        assert!(!tails.is_empty());
        // Stretch to Fit Rafter: the tail is the rafter's size.
        assert!(near(tails[0].width, d.rafter_width) && near(tails[0].height, d.rafter_depth));
        // Spacing: the first tail is flush with the end, the count is the
        // rafter count of each eave.
        let south: Vec<_> = tails.iter().filter(|t| t.plane == 0).collect();
        assert_eq!(
            south.len(),
            super::super::rafter_count(W + 2.0 * OVER, d.rafter_width, d.rafter_spacing)
        );
        // Exposed tails reach from the wall to the eave tip.
        assert!(near(tails[0].points[0].dist(tails[0].points[1]), OVER));

        assert!(kind(&make(RafterTailRecipe::Hidden, 0.0), TrimKind::RafterTail).is_empty());

        let part = make(RafterTailRecipe::PartiallyExposed, 0.0);
        let p = kind(&part, TrimKind::RafterTail);
        assert!(
            near(p[0].points[0].dist(p[0].points[1]), 6.0),
            "only 6\" shows"
        );

        let past = make(RafterTailRecipe::Exposed, 3.0);
        let q = kind(&past, TrimKind::RafterTail);
        assert!(near(q[0].points[0].dist(q[0].points[1]), OVER + 3.0));
    }

    #[test]
    fn own_size_tails_ignore_the_rafter() {
        let d = RoofDetail::default();
        let o = only(|o| {
            o.rafter_tails.enabled = true;
            o.rafter_tails.stretch_to_fit = false;
            o.rafter_tails.width = 2.0;
            o.rafter_tails.height = 4.0;
        });
        let lines = roof_trim_lines(&hip(), &d, &o);
        let t = kind(&lines, TrimKind::RafterTail);
        assert!(t.iter().all(|t| near(t.width, 2.0) && near(t.height, 4.0)));
    }

    #[test]
    fn boxed_eaves_always_have_a_horizontal_soffit_and_flush_ones_only_when_higher() {
        use SoffitStyle::*;
        assert!(soffit_boxed(Boxed, false, 100.0, 100.0));
        assert!(!soffit_boxed(Flush, false, 130.0, 100.0));
        assert!(!soffit_boxed(Flush, true, 100.0, 100.0), "the lowest eave");
        assert!(
            soffit_boxed(Flush, true, 130.0, 100.0),
            "Higher Eaves Boxed"
        );
    }

    #[test]
    fn the_soffit_style_does_not_change_exposed_tails() {
        let d = RoofDetail::default();
        let mut o = only(|o| o.rafter_tails.enabled = true);
        let boxed = roof_trim_lines(&hip(), &d, &o).len();
        o.soffit = SoffitStyle::Flush;
        assert_eq!(roof_trim_lines(&hip(), &d, &o).len(), boxed);
    }
}
