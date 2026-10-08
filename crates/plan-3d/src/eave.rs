//! Eave detail of roof planes: fascia, soffit, rake boards, frieze, ridge and
//! hip caps, and the flashing line where a lower roof butts a wall.
//!
//! Everything is derived from the planes' polygons (roof space: `X = plan x`,
//! `Y` up, `Z = -plan y`) and their overhang. [`eave_elements`] lists the
//! pieces with their faces; [`eave_detail_meshes`] turns them into meshes.

use crate::builder::MeshSet;
use crate::cover::RoofDetail;
use crate::mesh::{Material, Mesh};
use plan_core::defaults::EaveCut;
use plan_core::{Id, Point};
use plan_roof::{classify_edges, EdgeRole, RoofPlane};
use serde::{Deserialize, Serialize};

type V3d = [f64; 3];

/// Eave options one roof plane sets for itself (Roof Plane Specification >
/// Eaves); `None` follows the roof's [`RoofDetail`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EaveOverrides {
    pub eave_cut: Option<EaveCut>,
    pub rafter_tails: Option<bool>,
    pub fascia: Option<bool>,
    pub soffit: Option<bool>,
    pub frieze: Option<bool>,
    pub gutters: Option<bool>,
}

impl EaveOverrides {
    /// Does the plane follow the roof's detail in everything?
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// `detail` with this plane's choices applied.
    pub fn apply(&self, detail: &RoofDetail) -> RoofDetail {
        let mut d = *detail;
        if let Some(v) = self.eave_cut {
            d.eave_cut = v;
        }
        if let Some(v) = self.rafter_tails {
            d.rafter_tails = v;
        }
        if let Some(v) = self.fascia {
            d.fascia = v;
        }
        if let Some(v) = self.soffit {
            d.soffit = v;
        }
        if let Some(v) = self.frieze {
            d.frieze = v;
        }
        if let Some(v) = self.gutters {
            d.gutters = v;
        }
        d
    }
}

/// A roof plane with the facts its eave detail needs.
#[derive(Debug, Clone, PartialEq)]
pub struct EavePlane {
    pub plane: RoofPlane,
    /// Horizontal overhang beyond the wall face, inches (`0` skips soffit
    /// and frieze).
    pub overhang: f64,
    /// Draw ridge and hip caps on this plane's ridges and hips.
    pub ridge_caps: bool,
    /// Edges made where a taller wall trimmed the plane (flashing lines).
    pub cuts: Vec<(V3d, V3d)>,
    /// Record id the meshes are tagged with.
    pub id: Option<Id>,
    /// This plane's own eave choices (cut, tails, fascia, soffit, gutters).
    pub opts: EaveOverrides,
}

impl EavePlane {
    /// A plane with no overhang, caps or cuts.
    pub fn bare(plane: RoofPlane) -> Self {
        Self {
            plane,
            overhang: 0.0,
            ridge_caps: false,
            cuts: Vec::new(),
            id: None,
            opts: EaveOverrides::default(),
        }
    }
}

/// What an eave piece is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EaveKind {
    /// The board along an eave.
    Fascia,
    /// The board along a gable end.
    RakeFascia,
    /// Underside between the wall and the fascia.
    Soffit,
    /// Underside of the gable overhang.
    RakeSoffit,
    /// Block on the wall between the plate and the roof underside.
    Frieze,
    /// Cap along a ridge or hip.
    RidgeCap,
    /// Strip along a roof-to-wall butt line.
    Flashing,
    /// An exposed rafter end under the eave.
    RafterTail,
    /// Gutter along an eave.
    Gutter,
}

impl EaveKind {
    fn material(self) -> Material {
        match self {
            EaveKind::RidgeCap => Material::Roof,
            EaveKind::Flashing | EaveKind::Gutter => Material::Metal,
            _ => Material::Trim,
        }
    }
}

/// One piece of eave detail: its faces, each with an outward normal.
#[derive(Debug, Clone, PartialEq)]
pub struct EaveElement {
    pub kind: EaveKind,
    /// Index into the plane list.
    pub plane: usize,
    pub faces: Vec<([V3d; 4], V3d)>,
}

fn add(a: V3d, b: V3d) -> V3d {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3d, b: V3d) -> V3d {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V3d, k: f64) -> V3d {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V3d, b: V3d) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3d, b: V3d) -> V3d {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: V3d) -> V3d {
    let l = dot(a, a).sqrt();
    if l < 1e-12 {
        [0.0, 1.0, 0.0]
    } else {
        scale(a, 1.0 / l)
    }
}
fn plan(v: V3d) -> Point {
    Point::new(v[0], -v[2])
}
fn lift(p: Point, y: f64) -> V3d {
    [p.x, y, -p.y]
}
/// A plan direction as a roof-space vector.
fn plan_vec(p: Point) -> V3d {
    [p.x, 0.0, -p.y]
}

/// Intersection of the lines `p + t d` and `q + u e`, as a point on the first.
fn line_isect(p: Point, d: Point, q: Point, e: Point) -> Option<Point> {
    let den = d.cross(e);
    if den.abs() < 1e-9 {
        return None;
    }
    let t = q.sub(p).cross(e) / den;
    Some(p.add(d.scale(t)))
}

/// A box hung from the segment `p -> q`: the top ring `p, q, q+thick,
/// p+thick`, the bottom ring `down` below it. Six faces, outward normals.
fn board(p: V3d, q: V3d, thick: V3d, down: V3d) -> Vec<([V3d; 4], V3d)> {
    let top = [p, q, add(q, thick), add(p, thick)];
    let bot = top.map(|v| add(v, down));
    let faces = [
        [top[0], top[1], top[2], top[3]],
        [bot[0], bot[1], bot[2], bot[3]],
        [top[0], top[1], bot[1], bot[0]],
        [top[1], top[2], bot[2], bot[1]],
        [top[2], top[3], bot[3], bot[2]],
        [top[3], top[0], bot[0], bot[3]],
    ];
    let center = scale(
        top.iter()
            .chain(bot.iter())
            .fold([0.0; 3], |a, v| add(a, *v)),
        1.0 / 8.0,
    );
    faces
        .into_iter()
        .map(|f| {
            let c = scale(f.iter().fold([0.0; 3], |a, v| add(a, *v)), 0.25);
            let mut n = unit(cross(sub(f[1], f[0]), sub(f[3], f[0])));
            if dot(n, sub(c, center)) < 0.0 {
                n = scale(n, -1.0);
            }
            (f, n)
        })
        .collect()
}

/// A single face with its normal turned to `toward`'s side.
fn sheet(quad: [V3d; 4], toward: V3d) -> Vec<([V3d; 4], V3d)> {
    let mut n = unit(cross(sub(quad[1], quad[0]), sub(quad[3], quad[0])));
    if dot(n, toward) < 0.0 {
        n = scale(n, -1.0);
    }
    vec![(quad, n)]
}

/// The fascia board along the edge `a -> b`, cut as the roof's eave cut says
/// (`inward` is the plan direction into the plane, `normal` the plane's unit
/// normal, `slope` its rise per unit of run, `drop_v` the structure's vertical
/// thickness):
///
/// * plumb: a vertical board hanging `fascia_height` from the eave top;
/// * level: a flat board `fascia_thickness` thick under the structure's end,
///   `fascia_height` deep into the building;
/// * square: a board square to the plane, leaning back under the roof.
fn fascia_board(
    a: V3d,
    b: V3d,
    inward: Point,
    normal: V3d,
    slope: f64,
    drop_v: f64,
    d: &RoofDetail,
) -> Vec<([V3d; 4], V3d)> {
    let into = plan_vec(inward);
    match d.eave_cut {
        EaveCut::Plumb => board(
            a,
            b,
            scale(into, d.fascia_thickness),
            [0.0, -d.fascia_height, 0.0],
        ),
        EaveCut::Level => {
            let under = [0.0, -drop_v, 0.0];
            board(
                add(a, under),
                add(b, under),
                scale(into, d.fascia_height),
                [0.0, -d.fascia_thickness, 0.0],
            )
        }
        EaveCut::Square => {
            let along = unit(add(into, [0.0, slope, 0.0]));
            board(
                a,
                b,
                scale(along, d.fascia_thickness),
                scale(normal, -d.fascia_height),
            )
        }
    }
}

/// All the eave detail of `planes`, in plane order.
pub fn eave_elements(planes: &[EavePlane], base: &RoofDetail) -> Vec<EaveElement> {
    let roofs: Vec<RoofPlane> = planes.iter().map(|p| p.plane.clone()).collect();
    let edges = classify_edges(&roofs);
    let mut out: Vec<EaveElement> = Vec::new();
    for (k, ep) in planes.iter().enumerate() {
        let plane = &ep.plane;
        let detail = &ep.opts.apply(base);
        let n = plane.polygon3d.len();
        if n < 3 {
            continue;
        }
        let normal = plane.normal();
        let ny = normal[1];
        if ny < 1e-6 {
            continue;
        }
        let drop_v = detail.thickness.max(0.0) / ny;
        let slope = normal[0].hypot(normal[2]) / ny;
        let pts = plane.plan_polygon();
        let centroid = pts
            .iter()
            .fold(Point::ZERO, |a, p| a + *p)
            .scale(1.0 / n as f64);
        let ov = ep.overhang;
        let push = |out: &mut Vec<EaveElement>, kind, faces| {
            out.push(EaveElement {
                kind,
                plane: k,
                faces,
            })
        };
        // Direction into the plane (plan), perpendicular to the edge a -> b.
        let inward_of = |a: Point, b: Point| {
            let d = b.sub(a).normalized();
            let i = d.perp();
            if centroid.sub(a).dot(i) < 0.0 {
                i.scale(-1.0)
            } else {
                i
            }
        };
        for e in edges.iter().filter(|e| e.plane == k) {
            let (a, b) = (e.a, e.b);
            let (pa, pb) = (plan(a), plan(b));
            if pa.dist(pb) < 1.0 {
                continue;
            }
            let inward = inward_of(pa, pb);
            match e.role {
                EdgeRole::Eave => {
                    if detail.fascia && detail.fascia_height > 0.0 && detail.fascia_thickness > 0.0
                    {
                        push(
                            &mut out,
                            EaveKind::Fascia,
                            fascia_board(a, b, inward, normal, slope, drop_v, detail),
                        );
                    }
                    if detail.gutters && detail.gutter_size > 0.0 {
                        let size = detail.gutter_size;
                        push(
                            &mut out,
                            EaveKind::Gutter,
                            board(
                                add(a, [0.0, -GUTTER_DROP, 0.0]),
                                add(b, [0.0, -GUTTER_DROP, 0.0]),
                                scale(plan_vec(inward), -size),
                                [0.0, -size * 0.8, 0.0],
                            ),
                        );
                    }
                    if ov < 1.0 {
                        continue;
                    }
                    if detail.rafter_tails {
                        let tails = rafter_tails(a, b, inward, normal, slope, ov, drop_v, detail);
                        for faces in tails {
                            push(&mut out, EaveKind::RafterTail, faces);
                        }
                    }
                    // The wall face: the eave line moved in by the overhang,
                    // ending where it meets the neighbouring edges.
                    let d = pb.sub(pa).normalized();
                    let origin = pa.add(inward.scale(ov));
                    let prev = plane.polygon3d[(e.index + n - 1) % n];
                    let next = plane.polygon3d[(e.index + 2) % n];
                    let ia = line_isect(origin, d, pa, pa.sub(plan(prev)))
                        .filter(|p| p.dist(origin) < 4.0 * ov)
                        .unwrap_or(origin);
                    let ib = line_isect(origin, d, pb, plan(next).sub(pb))
                        .filter(|p| p.dist(origin.add(d.scale(pa.dist(pb)))) < 4.0 * ov)
                        .unwrap_or_else(|| origin.add(d.scale(pa.dist(pb))));
                    // Exposed rafter tails are not closed in by a soffit.
                    if detail.soffit && !detail.rafter_tails {
                        let tuck = drop_v.min((detail.fascia_height - 0.5).max(0.0));
                        let ys = a[1] - tuck;
                        let rise = if detail.sloped_soffit {
                            ov * slope
                        } else {
                            0.0
                        };
                        let quad = [
                            lift(pa, ys),
                            lift(pb, b[1] - tuck),
                            lift(ib, b[1] - tuck + rise),
                            lift(ia, ys + rise),
                        ];
                        push(&mut out, EaveKind::Soffit, sheet(quad, [0.0, -1.0, 0.0]));
                    }
                    let under_face = a[1] - drop_v + ov * slope;
                    if detail.frieze && under_face > a[1] + 0.5 {
                        push(
                            &mut out,
                            EaveKind::Frieze,
                            board(
                                lift(ia, under_face),
                                lift(ib, under_face),
                                scale(plan_vec(inward), -1.0),
                                [0.0, -(under_face - a[1]), 0.0],
                            ),
                        );
                    }
                }
                EdgeRole::Rake => {
                    if detail.rake_fascia && detail.fascia && detail.fascia_height > 0.0 {
                        push(
                            &mut out,
                            EaveKind::RakeFascia,
                            fascia_board(a, b, inward, normal, slope, drop_v, detail),
                        );
                    }
                    if detail.soffit && ov >= 1.0 {
                        let under = |p: Point| plane.underside_at(p, detail.thickness);
                        let (qa, qb) = (pa.add(inward.scale(ov)), pb.add(inward.scale(ov)));
                        if let (Some(ya), Some(yb), Some(yqa), Some(yqb)) =
                            (under(pa), under(pb), under(qa), under(qb))
                        {
                            let quad = [lift(pa, ya), lift(pb, yb), lift(qb, yqb), lift(qa, yqa)];
                            push(
                                &mut out,
                                EaveKind::RakeSoffit,
                                sheet(quad, [0.0, -1.0, 0.0]),
                            );
                        }
                    }
                }
                EdgeRole::Ridge | EdgeRole::Hip => {
                    if !(detail.ridge_caps && ep.ridge_caps) {
                        continue;
                    }
                    let side = |p: Point| {
                        let q = p.add(inward.scale(CAP_WIDTH));
                        plane.height_at(q).map(|y| lift(q, y + CAP_LIFT / ny))
                    };
                    let lifted = |v: V3d| add(v, scale(normal, CAP_LIFT));
                    if let (Some(sa), Some(sb)) = (side(pa), side(pb)) {
                        let quad = [lifted(a), lifted(b), sb, sa];
                        push(&mut out, EaveKind::RidgeCap, sheet(quad, [0.0, 1.0, 0.0]));
                    }
                }
                _ => {}
            }
        }
        if detail.flashing {
            for &(p, q) in &ep.cuts {
                let (pp, pq) = (plan(p), plan(q));
                if pp.dist(pq) < 1.0 {
                    continue;
                }
                let inward = inward_of(pp, pq);
                push(
                    &mut out,
                    EaveKind::Flashing,
                    board(
                        add(p, [0.0, FLASHING_HEIGHT, 0.0]),
                        add(q, [0.0, FLASHING_HEIGHT, 0.0]),
                        scale(plan_vec(inward), FLASHING_THICKNESS),
                        [0.0, -FLASHING_HEIGHT, 0.0],
                    ),
                );
            }
        }
    }
    out
}

/// How far a gutter hangs below the eave top, inches.
const GUTTER_DROP: f64 = 1.0;

/// The rafter tails along the eave `a -> b`: boxes `rafter_width` wide every
/// `rafter_spacing` inches (the first flush with the end), from the eave
/// tip back to the wall face, hanging `rafter_depth` below the underside of
/// the structure.
#[allow(clippy::too_many_arguments)]
fn rafter_tails(
    a: V3d,
    b: V3d,
    inward: Point,
    normal: V3d,
    slope: f64,
    overhang: f64,
    drop_v: f64,
    d: &RoofDetail,
) -> Vec<Vec<([V3d; 4], V3d)>> {
    let len = plan(a).dist(plan(b));
    let w = d.rafter_width.max(0.25);
    let spacing = d.rafter_spacing.max(w + 0.25);
    let count = rafter_count(len, w, spacing);
    let along_edge = unit(sub(b, a));
    let up_slope = unit(add(plan_vec(inward), [0.0, slope, 0.0]));
    let run = overhang * (1.0 + slope * slope).sqrt();
    let under = add(a, [0.0, -drop_v, 0.0]);
    (0..count)
        .map(|i| {
            let start = i as f64 * spacing;
            let p = add(under, scale(along_edge, start));
            let q = add(p, scale(along_edge, w));
            board(
                p,
                q,
                scale(up_slope, run),
                scale(normal, -d.rafter_depth.max(0.25)),
            )
        })
        .collect()
}

/// Rafters of width `w` at `spacing` along an eave of length `len`.
pub fn rafter_count(len: f64, w: f64, spacing: f64) -> usize {
    (((len - w) / spacing).floor().max(0.0) as usize) + 1
}

/// Ridge cap half-width and lift above the roof surface, inches.
const CAP_WIDTH: f64 = 5.0;
const CAP_LIFT: f64 = 1.0;
/// Flashing strip height above the roof and thickness, inches.
const FLASHING_HEIGHT: f64 = 6.0;
const FLASHING_THICKNESS: f64 = 0.25;

/// Meshes of [`eave_elements`]: one mesh per material per plane, tagged with
/// the plane's id when it has one.
pub fn eave_detail_meshes(planes: &[EavePlane], detail: &RoofDetail) -> Vec<Mesh> {
    let elements = eave_elements(planes, detail);
    let mut out = Vec::new();
    for (k, ep) in planes.iter().enumerate() {
        let mut set = MeshSet::default();
        for e in elements.iter().filter(|e| e.plane == k) {
            let mesh = set.material(e.kind.material());
            for (quad, normal) in &e.faces {
                let f = |v: V3d| [v[0] as f32, v[1] as f32, v[2] as f32];
                let uv = |v: V3d| [((v[0] + v[2]) / 12.0) as f32, (v[1] / 12.0) as f32];
                mesh.quad(quad.map(f), quad.map(uv), f(*normal));
            }
        }
        out.extend(set.finish(ep.id));
    }
    out
}
