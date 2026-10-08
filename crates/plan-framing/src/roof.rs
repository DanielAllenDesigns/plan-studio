//! Roof framing: rafters, ridge, hips, valleys, fascia, collar ties, ceiling
//! joists and simple Fink trusses for a [`Roof`] from `plan-roof`.
//!
//! Roof plane polygons use the 3D frame `X = plan x`, `Y = elevation`,
//! `Z = -plan y`. Every plane's first polygon edge is its eave (`baseline`),
//! so the plane is described by an eave frame: `e` along the eave, `n` (plan,
//! to the left of `e`, which is up the slope for a counter-clockwise roof)
//! and `t` = distance up the slope in plan.

use crate::detail::Stroke;
use crate::lumber::{Lumber, TWO_BY_EIGHT, TWO_BY_FOUR, TWO_BY_SIX, TWO_BY_TEN};
use crate::member::{add, dot, scale, Member, MemberKind, Transform3, Vec3};
use crate::takeoff::{takeoff, Takeoff};
use plan_core::Point;
use plan_roof::{Roof, RoofPlane};
use serde::{Deserialize, Serialize};

/// Shortest member kept, inches (drops slivers where a rafter line only
/// grazes a hip corner).
const MIN_MEMBER: f64 = 3.0;
/// Tolerance when matching shared plane vertices, inches.
const NODE_TOL: f64 = 0.05;
/// Text height of plan labels, inches.
const LABEL_HEIGHT: f64 = 6.0;
/// Lumber used for collar ties.
const COLLAR_TIE_LUMBER: Lumber = TWO_BY_FOUR;
/// Lumber used for ceiling joists.
const CEILING_JOIST_LUMBER: Lumber = TWO_BY_SIX;
/// Lumber used for every truss chord and web.
const TRUSS_LUMBER: Lumber = TWO_BY_FOUR;
/// A truss member: kind and its two ends as (span, height) in the truss plane.
type TrussSeg = (MemberKind, (f64, f64), (f64, f64));
const TRUSS_TAG: &str = " [truss ";

/// How rafter tails are cut at the overhang.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverhangCut {
    Plumb,
    Square,
}

/// Roof framing defaults, the equivalent of Chief's Roof Framing defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoofFramingDefaults {
    pub rafter: Lumber,
    /// Rafter spacing on centre, inches.
    pub spacing: f64,
    pub ridge: Lumber,
    pub hip_valley: Lumber,
    /// Birdsmouth seat cut length, inches (cut-list data only: members are
    /// plain boxes, so the notch is not modelled).
    pub birdsmouth_seat: f64,
    /// Tail cut (cut-list data only, see `birdsmouth_seat`).
    pub overhang_cut: OverhangCut,
    pub collar_ties: bool,
    pub ceiling_joists: bool,
    /// Frame with Fink trusses instead of rafters and ceiling joists.
    pub trusses: bool,
    /// Truss spacing on centre, inches.
    pub truss_spacing: f64,
    pub fascia: Lumber,
    /// Switch to trusses automatically when the building span (shorter
    /// bounding dimension) exceeds this many inches. `0.0` disables it.
    pub use_trusses_over_span: f64,
}

impl Default for RoofFramingDefaults {
    fn default() -> Self {
        Self {
            rafter: TWO_BY_EIGHT,
            spacing: 16.0,
            ridge: TWO_BY_TEN,
            hip_valley: TWO_BY_TEN,
            birdsmouth_seat: 3.5,
            overhang_cut: OverhangCut::Plumb,
            collar_ties: false,
            ceiling_joists: false,
            trusses: false,
            truss_spacing: 24.0,
            fascia: TWO_BY_SIX,
            use_trusses_over_span: 0.0,
        }
    }
}

fn plan(v: Vec3) -> Point {
    Point::new(v[0], -v[2])
}

fn dir3(p: Point) -> Vec3 {
    [p.x, 0.0, -p.y]
}

fn sub3(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm3(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

/// Eave frame of a plane: start `a`, unit `e` along the eave, unit `n` up the
/// slope (plan), eave length and eave elevation.
struct Eave {
    a: Point,
    e: Point,
    n: Point,
    len: f64,
    y: f64,
}

fn eave(plane: &RoofPlane) -> Eave {
    let (a, b) = plane.baseline;
    let e = b.sub(a).normalized();
    Eave {
        a,
        e,
        n: e.perp(),
        len: a.dist(b),
        y: plane.polygon3d[0][1],
    }
}

/// Rafter / truss positions along `len`: every `spacing` from the start, the
/// first and last inset by half the member thickness, plus an end member when
/// the last full step leaves more than one thickness.
fn positions(len: f64, spacing: f64, thickness: f64) -> Vec<f64> {
    if len <= thickness {
        return Vec::new();
    }
    let spacing = spacing.max(thickness);
    let n = (len / spacing + 1e-9).floor() as usize;
    let (lo, hi) = (thickness / 2.0, len - thickness / 2.0);
    let mut v: Vec<f64> = (0..=n)
        .map(|k| (k as f64 * spacing).clamp(lo, hi))
        .collect();
    if len - n as f64 * spacing > thickness {
        v.push(hi);
    }
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    v
}

/// Intervals of `t` where the line `x == s` lies inside `poly` (even-odd).
fn intervals(poly: &[Point], s: f64) -> Vec<(f64, f64)> {
    let mut hits = Vec::new();
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        if (p.x <= s) != (q.x <= s) {
            hits.push(p.y + (s - p.x) * (q.y - p.y) / (q.x - p.x));
        }
    }
    hits.sort_by(f64::total_cmp);
    hits.as_chunks::<2>()
        .0
        .iter()
        .map(|c| (c[0], c[1]))
        .collect()
}

fn merge(mut v: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (a, b) in v {
        match out.last_mut() {
            Some(last) if a <= last.1 + 0.5 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeClass {
    Ridge,
    Hip,
    Valley,
}

/// An edge shared by two planes, oriented lower end first (ridges keep the
/// first plane's order).
struct Shared {
    a: usize,
    b: usize,
    p0: Vec3,
    p1: Vec3,
    class: EdgeClass,
}

fn same(p: Vec3, q: Vec3) -> bool {
    norm3(sub3(p, q)) < NODE_TOL
}

fn shared_edges(planes: &[&RoofPlane]) -> Vec<Shared> {
    let mut out = Vec::new();
    for i in 0..planes.len() {
        for j in i + 1..planes.len() {
            let (pi, pj) = (&planes[i].polygon3d, &planes[j].polygon3d);
            for k in 1..pi.len() {
                let (p0, p1) = (pi[k], pi[(k + 1) % pi.len()]);
                let found = (1..pj.len()).any(|m| {
                    let (q0, q1) = (pj[m], pj[(m + 1) % pj.len()]);
                    (same(p0, q1) && same(p1, q0)) || (same(p0, q0) && same(p1, q1))
                });
                if !found || norm3(sub3(p1, p0)) < 0.5 {
                    continue;
                }
                // Convex when the neighbour falls away below plane `i`.
                let c = {
                    let n = pj.len() as f64;
                    let s = pj.iter().fold([0.0; 3], |s, v| add(s, *v));
                    scale(s, 1.0 / n)
                };
                let side = dot(sub3(c, p0), planes[i].normal());
                if side.abs() < 0.01 {
                    continue;
                }
                let horizontal = (p0[1] - p1[1]).abs() < 0.01;
                let class = match (side < 0.0, horizontal) {
                    (true, true) => EdgeClass::Ridge,
                    (true, false) => EdgeClass::Hip,
                    (false, _) => EdgeClass::Valley,
                };
                let (p0, p1) = if class != EdgeClass::Ridge && p0[1] > p1[1] {
                    (p1, p0)
                } else {
                    (p0, p1)
                };
                out.push(Shared {
                    a: i,
                    b: j,
                    p0,
                    p1,
                    class,
                });
            }
        }
    }
    out
}

/// Frame `roof` per `d`. Members are in the 3D frame of the roof planes.
///
/// * Rafters run perpendicular to each plane's eave at `spacing` o.c. from
///   the plane's first eave vertex (first and last inset half a rafter), from
///   the eave tail to the plane boundary (ridge, hip or valley centreline);
///   the top edge lies in the roof plane and the member length is the slope
///   length. Slivers shorter than 3" at hip corners are dropped.
/// * Ridge, hip and valley members follow edges shared by two planes. The
///   top of a ridge board is at the ridge line; hips and valleys sit under
///   the edge. Fascia hangs outside each eave, top flush with the eave line.
/// * Collar ties (2x4) sit a third of the ridge rise below each ridge on
///   every other rafter position, beside the rafter.
/// * Ceiling joists (2x6) span the shorter bounding dimension, top at
///   `baseline_elevation`, clipped to the roof outline (so L shapes work).
///   The outline includes the overhang, since the roof carries no footprint.
/// * With trusses (`trusses`, or span over `use_trusses_over_span`) each
///   truss is 7 members (2 top chords, bottom chord, 4 webs) spanning the
///   shorter bounding dimension, spaced along the longer, tagged in
///   `Member.label` (see [`truss_id`]); rafters, ridge, hips, valleys,
///   collar ties and ceiling joists are then not generated.
pub fn frame_roof(roof: &Roof, d: &RoofFramingDefaults) -> Vec<Member> {
    let planes: Vec<&RoofPlane> = roof
        .planes
        .iter()
        .filter(|p| p.polygon3d.len() >= 3)
        .collect();
    let Some((lo, hi)) = roof.bounds() else {
        return Vec::new();
    };
    if planes.is_empty() {
        return Vec::new();
    }
    let span = (hi[0] - lo[0]).min(hi[2] - lo[2]);
    let mut out = Vec::new();
    if d.trusses || (d.use_trusses_over_span > 0.0 && span > d.use_trusses_over_span) {
        out.extend(trusses(roof, d, lo, hi));
    } else {
        for p in &planes {
            rafters(p, d, &mut out);
        }
        let shared = shared_edges(&planes);
        for e in &shared {
            edge_member(e, d, &mut out);
        }
        if d.collar_ties {
            collar_ties(&planes, &shared, d, &mut out);
        }
        if d.ceiling_joists {
            ceiling_joists(&planes, roof.baseline_elevation, lo, hi, d, &mut out);
        }
    }
    for p in &planes {
        fascia(p, d, &mut out);
    }
    out
}

fn rafters(plane: &RoofPlane, d: &RoofFramingDefaults, out: &mut Vec<Member>) {
    let f = eave(plane);
    let rise = plane.pitch_in_12 / 12.0;
    let theta = rise.atan();
    let (sn, cs) = (theta.sin(), theta.cos());
    // Plane polygon in (s along eave, t up slope).
    let poly: Vec<Point> = plane
        .polygon3d
        .iter()
        .map(|v| {
            let r = plan(*v).sub(f.a);
            Point::new(r.dot(f.e), r.dot(f.n))
        })
        .collect();
    let axis_x = [f.n.x * cs, sn, -f.n.y * cs];
    let axis_y = [-f.n.x * sn, cs, f.n.y * sn];
    for s in positions(f.len, d.spacing, d.rafter.thickness) {
        for (t0, t1) in intervals(&poly, s) {
            let length = (t1 - t0) / cs;
            if length < MIN_MEMBER {
                continue;
            }
            let p = f.a.add(f.e.scale(s)).add(f.n.scale(t0));
            let top = [p.x, f.y + t0 * rise, -p.y];
            let origin = add(top, scale(axis_y, -d.rafter.depth / 2.0));
            let tf = Transform3 {
                origin,
                axis_x,
                axis_y,
            };
            out.push(Member::new(MemberKind::Rafter, d.rafter, length, tf, None));
        }
    }
}

fn edge_member(e: &Shared, d: &RoofFramingDefaults, out: &mut Vec<Member>) {
    let v = sub3(e.p1, e.p0);
    let length = norm3(v);
    let axis_x = scale(v, 1.0 / length);
    if e.class == EdgeClass::Ridge {
        let tf = Transform3 {
            origin: add(e.p0, [0.0, -d.ridge.depth / 2.0, 0.0]),
            axis_x,
            axis_y: [0.0, 1.0, 0.0],
        };
        out.push(Member::new(MemberKind::Ridge, d.ridge, length, tf, None));
        return;
    }
    // Depth direction: "up" made perpendicular to the member.
    let up = [0.0, 1.0, 0.0];
    let w = sub3(up, scale(axis_x, dot(up, axis_x)));
    let axis_y = scale(w, 1.0 / norm3(w));
    let kind = if e.class == EdgeClass::Hip {
        MemberKind::Hip
    } else {
        MemberKind::Valley
    };
    let tf = Transform3 {
        origin: add(e.p0, scale(axis_y, -d.hip_valley.depth / 2.0)),
        axis_x,
        axis_y,
    };
    out.push(Member::new(kind, d.hip_valley, length, tf, None));
}

fn collar_ties(
    planes: &[&RoofPlane],
    shared: &[Shared],
    d: &RoofFramingDefaults,
    out: &mut Vec<Member>,
) {
    let t = d.rafter.thickness;
    for e in shared.iter().filter(|e| e.class == EdgeClass::Ridge) {
        let (pa, pb) = (planes[e.a], planes[e.b]);
        let f = eave(pa);
        let (r0, r1) = (plan(e.p0), plan(e.p1));
        if r1.sub(r0).normalized().cross(f.e).abs() > 0.01 {
            continue;
        }
        let (s0, s1) = (r0.sub(f.a).dot(f.e), r1.sub(f.a).dot(f.e));
        let (smin, smax, rmin) = if s0 <= s1 { (s0, s1, r0) } else { (s1, s0, r1) };
        let drop = (e.p0[1] - f.y) / 3.0;
        if drop <= 0.0 {
            continue;
        }
        let ha = drop * 12.0 / pa.pitch_in_12.max(0.01);
        let hb = drop * 12.0 / pb.pitch_in_12.max(0.01);
        let y = e.p0[1] - drop;
        for (k, s) in positions(f.len, d.spacing, t).into_iter().enumerate() {
            if k % 2 != 0 {
                continue;
            }
            let mut st = s + t;
            if st > smax - t / 2.0 {
                st = s - t;
            }
            if st < smin + t / 2.0 || st > smax - t / 2.0 {
                continue;
            }
            let ridge_pt = rmin.add(f.e.scale(st - smin));
            let start = ridge_pt.sub(f.n.scale(ha));
            let tf = Transform3 {
                origin: [start.x, y, -start.y],
                axis_x: dir3(f.n),
                axis_y: [0.0, 1.0, 0.0],
            };
            out.push(Member::new(
                MemberKind::CollarTie,
                COLLAR_TIE_LUMBER,
                ha + hb,
                tf,
                None,
            ));
        }
    }
}

fn ceiling_joists(
    planes: &[&RoofPlane],
    elevation: f64,
    lo: Vec3,
    hi: Vec3,
    d: &RoofFramingDefaults,
    out: &mut Vec<Member>,
) {
    let (lo_x, hi_x, lo_y, hi_y) = (lo[0], hi[0], -hi[2], -lo[2]);
    let along_x = hi_x - lo_x <= hi_y - lo_y;
    // Polygons in (across, along) coordinates relative to the bounds.
    let polys: Vec<Vec<Point>> = planes
        .iter()
        .map(|pl| {
            pl.polygon3d
                .iter()
                .map(|v| {
                    let p = plan(*v);
                    let (x, y) = (p.x - lo_x, p.y - lo_y);
                    if along_x {
                        Point::new(y, x)
                    } else {
                        Point::new(x, y)
                    }
                })
                .collect()
        })
        .collect();
    let across_len = if along_x { hi_y - lo_y } else { hi_x - lo_x };
    let lumber = CEILING_JOIST_LUMBER;
    for c in positions(across_len, d.spacing, lumber.thickness) {
        let spans = merge(polys.iter().flat_map(|p| intervals(p, c)).collect());
        for (a0, a1) in spans {
            if a1 - a0 < MIN_MEMBER {
                continue;
            }
            let (start, dir) = if along_x {
                (Point::new(lo_x + a0, lo_y + c), Point::new(1.0, 0.0))
            } else {
                (Point::new(lo_x + c, lo_y + a0), Point::new(0.0, 1.0))
            };
            let tf = Transform3 {
                origin: [start.x, elevation - lumber.depth / 2.0, -start.y],
                axis_x: dir3(dir),
                axis_y: [0.0, 1.0, 0.0],
            };
            out.push(Member::new(
                MemberKind::CeilingJoist,
                lumber,
                a1 - a0,
                tf,
                None,
            ));
        }
    }
}

fn fascia(plane: &RoofPlane, d: &RoofFramingDefaults, out: &mut Vec<Member>) {
    let f = eave(plane);
    if f.len < MIN_MEMBER {
        return;
    }
    let start = f.a.sub(f.n.scale(d.fascia.thickness / 2.0));
    let tf = Transform3 {
        origin: [start.x, f.y - d.fascia.depth / 2.0, -start.y],
        axis_x: dir3(f.e),
        axis_y: [0.0, 1.0, 0.0],
    };
    out.push(Member::new(MemberKind::Fascia, d.fascia, f.len, tf, None));
}

fn trusses(roof: &Roof, d: &RoofFramingDefaults, lo: Vec3, hi: Vec3) -> Vec<Member> {
    let (lo_x, hi_x, lo_y, hi_y) = (lo[0], hi[0], -hi[2], -lo[2]);
    let elev = roof.baseline_elevation;
    let h = hi[1] - elev;
    let span_x = hi_x - lo_x <= hi_y - lo_y;
    let (w, lateral) = if span_x {
        (hi_x - lo_x, hi_y - lo_y)
    } else {
        (hi_y - lo_y, hi_x - lo_x)
    };
    if h <= 0.5 || w <= MIN_MEMBER {
        return Vec::new();
    }
    let span_dir = if span_x {
        Point::new(1.0, 0.0)
    } else {
        Point::new(0.0, 1.0)
    };
    let s3 = dir3(span_dir);
    let up = [0.0, 1.0, 0.0];
    // Fink: chords plus four webs from the third points to the chord mid-points and apex.
    let mid = (w / 2.0, h);
    let segs: [TrussSeg; 7] = [
        (MemberKind::TrussTopChord, (0.0, 0.0), mid),
        (MemberKind::TrussTopChord, (w, 0.0), mid),
        (MemberKind::TrussBottomChord, (0.0, 0.0), (w, 0.0)),
        (MemberKind::TrussWeb, (w / 4.0, h / 2.0), (w / 3.0, 0.0)),
        (MemberKind::TrussWeb, (w / 3.0, 0.0), mid),
        (
            MemberKind::TrussWeb,
            (3.0 * w / 4.0, h / 2.0),
            (2.0 * w / 3.0, 0.0),
        ),
        (MemberKind::TrussWeb, (2.0 * w / 3.0, 0.0), mid),
    ];
    let mut out = Vec::new();
    for (i, c) in positions(lateral, d.truss_spacing, TRUSS_LUMBER.thickness)
        .into_iter()
        .enumerate()
    {
        let base = if span_x {
            Point::new(lo_x, lo_y + c)
        } else {
            Point::new(lo_x + c, lo_y)
        };
        let base3 = [base.x, elev, -base.y];
        for (kind, p0, p1) in segs {
            let (dw, dh) = (p1.0 - p0.0, p1.1 - p0.1);
            let length = dw.hypot(dh);
            let (c_, s_) = (dw / length, dh / length);
            let axis_x = add(scale(s3, c_), scale(up, s_));
            let axis_y = add(scale(s3, -s_), scale(up, c_));
            let origin = add(base3, add(scale(s3, p0.0), scale(up, p0.1)));
            let tf = Transform3 {
                origin,
                axis_x,
                axis_y,
            };
            let mut m = Member::new(kind, TRUSS_LUMBER, length, tf, None);
            m.label = format!("{}{TRUSS_TAG}{}]", m.label, i + 1);
            out.push(m);
        }
    }
    out
}

/// The truss number tagged on a truss member's label, if it is one.
pub fn truss_id(m: &Member) -> Option<u32> {
    let i = m.label.rfind(TRUSS_TAG)?;
    m.label[i + TRUSS_TAG.len()..]
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// Lumber list for roof members. Truss tags are dropped from labels so
/// identical truss pieces count together.
pub fn roof_framing_takeoff(members: &[Member]) -> Takeoff {
    let stripped: Vec<Member> = members
        .iter()
        .map(|m| {
            let mut m = m.clone();
            if truss_id(&m).is_some() {
                if let Some(i) = m.label.rfind(TRUSS_TAG) {
                    m.label.truncate(i);
                }
            }
            m
        })
        .collect();
    takeoff(&stripped)
}

/// Plan-view lines for a Roof Framing plan: a centreline per rafter, ridge,
/// hip, valley, fascia and collar tie, labels on ridges, hips and valleys, and
/// each truss as its bottom chord labelled `T<n>`. Ceiling joists, truss
/// top chords and webs are omitted.
pub fn roof_plan_symbols(members: &[Member]) -> Vec<Stroke> {
    let mut out = Vec::new();
    for m in members {
        let label = match m.kind {
            MemberKind::Rafter | MemberKind::Fascia | MemberKind::CollarTie => None,
            MemberKind::Ridge => Some("RIDGE".to_string()),
            MemberKind::Hip => Some("HIP".to_string()),
            MemberKind::Valley => Some("VALLEY".to_string()),
            MemberKind::TrussBottomChord => truss_id(m).map(|n| format!("T{n}")),
            _ => continue,
        };
        let a = plan(m.transform.origin);
        let b = plan(add(m.transform.origin, scale(m.transform.axis_x, m.length)));
        out.push(Stroke::Line(a, b));
        if let Some(text) = label {
            out.push(Stroke::Text {
                pos: Point::lerp(a, b, 0.5),
                text,
                height: LABEL_HEIGHT,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_roof::{build_roof, EdgeKind, EdgeRoof};

    const ELEV: f64 = 108.0;

    fn rect() -> Vec<Point> {
        [(0.0, 0.0), (480.0, 0.0), (480.0, 288.0), (0.0, 288.0)]
            .iter()
            .map(|&(x, y)| Point::new(x, y))
            .collect()
    }

    fn edges(kinds: [EdgeKind; 4], overhang: f64) -> Vec<EdgeRoof> {
        kinds
            .iter()
            .map(|&kind| EdgeRoof {
                pitch_in_12: 8.0,
                kind,
                overhang,
            })
            .collect()
    }

    fn hip(overhang: f64) -> Roof {
        build_roof(&rect(), &edges([EdgeKind::Hip; 4], overhang), ELEV)
    }

    fn count(m: &[Member], k: MemberKind) -> usize {
        m.iter().filter(|m| m.kind == k).count()
    }

    fn unit(v: Vec3) -> bool {
        (norm3(v) - 1.0).abs() < 1e-9
    }

    fn all_sane(m: &[Member]) {
        for m in m {
            assert!(m.length > 0.0, "{}", m.label);
            assert!(
                unit(m.transform.axis_x) && unit(m.transform.axis_y),
                "{}",
                m.label
            );
            assert!(dot(m.transform.axis_x, m.transform.axis_y).abs() < 1e-9);
        }
    }

    /// Rafters of the two long (eave along X) planes, split by side.
    fn long_rafters(m: &[Member]) -> (Vec<&Member>, Vec<&Member>) {
        let long: Vec<&Member> = m
            .iter()
            .filter(|m| m.kind == MemberKind::Rafter)
            .filter(|m| m.transform.axis_x[2].abs() > m.transform.axis_x[0].abs())
            .collect();
        let (south, north) = long.into_iter().partition(|m| m.transform.axis_x[2] < 0.0);
        (south, north)
    }

    fn max_len(v: &[&Member]) -> f64 {
        v.iter().map(|m| m.length).fold(0.0, f64::max)
    }

    #[test]
    fn hip_roof_rafters_count_and_slope_length() {
        let cos = (8.0f64 / 12.0).atan().cos();
        let m = frame_roof(&hip(0.0), &RoofFramingDefaults::default());
        let (south, north) = long_rafters(&m);
        // 480/16 + 1 = 31 stations; the two at the hip corners are slivers.
        for side in [&south, &north] {
            assert!((29..=31).contains(&side.len()), "{}", side.len());
            assert!((max_len(side) - 144.0 / cos).abs() < 1e-6);
        }
        // With a 16" overhang the tail adds 16" of run to the slope length.
        let m = frame_roof(&hip(16.0), &RoofFramingDefaults::default());
        let (south, _) = long_rafters(&m);
        assert!((max_len(&south) - (144.0 + 16.0) / cos).abs() < 1e-6);
        assert!((29..=33).contains(&south.len()));
        all_sane(&m);
    }

    #[test]
    fn rafter_tops_lie_in_their_roof_plane() {
        let roof = hip(16.0);
        let m = frame_roof(&roof, &RoofFramingDefaults::default());
        for r in m.iter().filter(|m| m.kind == MemberKind::Rafter) {
            let t = &r.transform;
            let top0 = add(t.origin, scale(t.axis_y, r.lumber.depth / 2.0));
            let top1 = add(top0, scale(t.axis_x, r.length));
            for p in [top0, top1] {
                let ok = roof
                    .planes
                    .iter()
                    .any(|pl| dot(sub3(p, pl.polygon3d[0]), pl.normal()).abs() < 1e-6);
                assert!(ok);
            }
        }
    }

    #[test]
    fn plain_hip_has_one_ridge_four_hips_no_valleys() {
        let m = frame_roof(&hip(16.0), &RoofFramingDefaults::default());
        assert_eq!(count(&m, MemberKind::Ridge), 1);
        assert_eq!(count(&m, MemberKind::Hip), 4);
        assert_eq!(count(&m, MemberKind::Valley), 0);
        assert_eq!(count(&m, MemberKind::Fascia), 4);
        let ridge = m.iter().find(|m| m.kind == MemberKind::Ridge).unwrap();
        assert!((ridge.length - 192.0).abs() < 1e-6);
        all_sane(&m);
    }

    #[test]
    fn l_shaped_hip_roof_has_a_valley() {
        let l: Vec<Point> = [
            (0.0, 0.0),
            (240.0, 0.0),
            (240.0, 120.0),
            (120.0, 120.0),
            (120.0, 240.0),
            (0.0, 240.0),
        ]
        .iter()
        .map(|&(x, y)| Point::new(x, y))
        .collect();
        let roof = build_roof(&l, &[EdgeRoof::default(); 6], ELEV);
        let m = frame_roof(&roof, &RoofFramingDefaults::default());
        assert!(count(&m, MemberKind::Valley) >= 1);
        assert!(count(&m, MemberKind::Hip) >= 4);
        all_sane(&m);
    }

    #[test]
    fn gable_roof_has_no_hips_and_gable_end_rafters() {
        let kinds = [
            EdgeKind::Hip,
            EdgeKind::Gable,
            EdgeKind::Hip,
            EdgeKind::Gable,
        ];
        let roof = build_roof(&rect(), &edges(kinds, 0.0), ELEV);
        let m = frame_roof(&roof, &RoofFramingDefaults::default());
        assert_eq!(count(&m, MemberKind::Hip), 0);
        assert_eq!(count(&m, MemberKind::Valley), 0);
        assert_eq!(count(&m, MemberKind::Ridge), 1);
        let xs: Vec<f64> = m
            .iter()
            .filter(|m| m.kind == MemberKind::Rafter)
            .map(|m| m.transform.origin[0])
            .collect();
        let (min, max) = (
            xs.iter().cloned().fold(f64::INFINITY, f64::min),
            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        assert!((min - 0.75).abs() < 1e-6 && (max - 479.25).abs() < 1e-6);
        // 31 stations per side, all full length.
        let cos = (8.0f64 / 12.0).atan().cos();
        let (south, north) = long_rafters(&m);
        assert_eq!((south.len(), north.len()), (31, 31));
        assert!((max_len(&south) - 144.0 / cos).abs() < 1e-6);
        all_sane(&m);
    }

    #[test]
    fn collar_ties_and_ceiling_joists() {
        let d = RoofFramingDefaults {
            collar_ties: true,
            ceiling_joists: true,
            ..Default::default()
        };
        let roof = hip(0.0);
        let m = frame_roof(&roof, &d);
        let ties: Vec<&Member> = m
            .iter()
            .filter(|m| m.kind == MemberKind::CollarTie)
            .collect();
        assert!(!ties.is_empty());
        // A third of the 96" ridge rise below the ridge: 108 + 64.
        for t in &ties {
            assert!((t.transform.origin[1] - (ELEV + 64.0)).abs() < 1e-6);
            assert!((t.length - 2.0 * 32.0 * 1.5).abs() < 1e-6);
        }
        let joists: Vec<&Member> = m
            .iter()
            .filter(|m| m.kind == MemberKind::CeilingJoist)
            .collect();
        // Joists span the 288" side, spaced along the 480" side.
        assert_eq!(joists.len(), 31);
        assert!(joists.iter().all(|j| (j.length - 288.0).abs() < 1e-6));
        all_sane(&m);
    }

    #[test]
    fn truss_mode_places_fink_trusses() {
        let d = RoofFramingDefaults {
            trusses: true,
            ceiling_joists: true,
            ..Default::default()
        };
        let m = frame_roof(&hip(0.0), &d);
        let ids: Vec<u32> = m.iter().filter_map(truss_id).collect();
        assert_eq!(ids.len(), 21 * 7);
        for id in 1..=21 {
            assert_eq!(ids.iter().filter(|&&i| i == id).count(), 7);
        }
        assert_eq!(count(&m, MemberKind::TrussTopChord), 42);
        assert_eq!(count(&m, MemberKind::TrussBottomChord), 21);
        assert_eq!(count(&m, MemberKind::TrussWeb), 84);
        for k in [
            MemberKind::Rafter,
            MemberKind::Ridge,
            MemberKind::CeilingJoist,
        ] {
            assert_eq!(count(&m, k), 0);
        }
        // Trusses span the 288" dimension and peak 96" above the plate.
        let bottom = m
            .iter()
            .find(|m| m.kind == MemberKind::TrussBottomChord)
            .unwrap();
        assert!((bottom.length - 288.0).abs() < 1e-6);
        let top = m
            .iter()
            .find(|m| m.kind == MemberKind::TrussTopChord)
            .unwrap();
        assert!((top.length - (144.0f64.powi(2) + 96.0f64.powi(2)).sqrt()).abs() < 1e-6);
        all_sane(&m);
        // Takeoff aggregates identical truss pieces.
        let t = roof_framing_takeoff(&m);
        assert!(t.lines.iter().all(|(l, _)| !l.contains("[truss")));
        let bottoms = t
            .lines
            .iter()
            .find(|(l, _)| l.ends_with("truss bottom chord"));
        assert_eq!(bottoms.map(|b| b.1), Some(21));
    }

    #[test]
    fn long_spans_switch_to_trusses() {
        let d = RoofFramingDefaults {
            use_trusses_over_span: 240.0,
            ..Default::default()
        };
        let m = frame_roof(&hip(0.0), &d);
        assert!(count(&m, MemberKind::TrussBottomChord) > 0);
        assert_eq!(count(&m, MemberKind::Rafter), 0);
        let m = frame_roof(
            &hip(0.0),
            &RoofFramingDefaults {
                use_trusses_over_span: 300.0,
                ..Default::default()
            },
        );
        assert!(count(&m, MemberKind::Rafter) > 0);
    }

    #[test]
    fn plan_symbols_and_empty_roof() {
        let m = frame_roof(&hip(16.0), &RoofFramingDefaults::default());
        let s = roof_plan_symbols(&m);
        let texts: Vec<&str> = s
            .iter()
            .filter_map(|s| match s {
                Stroke::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts.iter().filter(|t| **t == "HIP").count(), 4);
        assert_eq!(texts.iter().filter(|t| **t == "RIDGE").count(), 1);
        let lines = s.iter().filter(|s| matches!(s, Stroke::Line(..))).count();
        assert_eq!(lines, m.len());
        let empty = build_roof(&[], &[], ELEV);
        assert!(frame_roof(&empty, &RoofFramingDefaults::default()).is_empty());
    }
}
