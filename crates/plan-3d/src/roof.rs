//! Meshes for roof planes with holes, skylights, ceiling planes and dormers.
//!
//! Inputs come from `plan-roof`, in its roof space (`X = plan x`, `Y` up,
//! `Z = -plan y`), which is also scene space. Every solid is a polygon
//! (optionally with hole rings) extruded `depth` inches against its outward
//! normal: a face, a back face and side bands around the outer ring and the
//! hole rings.

use crate::builder::{MeshBuilder, MeshSet};
use crate::cover::RoofDetail;
use crate::eave::{eave_detail_meshes, EavePlane};
use crate::frame::Frame;
use crate::mesh::{Material, Mesh};
use crate::triangulate::ear_clip_with_holes;
use plan_core::{OpeningStyle, Point};
use plan_roof::{
    CeilingPlane, Dormer, DormerWall, Roof, RoofHole, RoofPlane, RoofPolygonWithHoles, Skylight,
    WindowOpening,
};

type V3d = [f64; 3];

/// Material of the framing above a ceiling plane.
pub const CEILING_FRAMING_MATERIAL: Material = Material::Framing;

/// Window frame ring width on dormer windows, inches.
const WINDOW_FRAME: f64 = 2.0;
/// Dormer window glass pane thickness, inches.
const PANE: f64 = 0.5;

const IN_PER_FT: f64 = 12.0;

fn sub(a: V3d, b: V3d) -> V3d {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3d, b: V3d) -> V3d {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
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
fn unit(a: V3d) -> Option<V3d> {
    let l = dot(a, a).sqrt();
    (l > 1e-12).then(|| scale(a, 1.0 / l))
}
fn f32v(a: V3d) -> [f32; 3] {
    a.map(|c| c as f32)
}

/// Newell vector of a polygon (twice the area along its normal).
fn newell(poly: &[V3d]) -> V3d {
    let n = poly.len();
    let mut s = [0.0; 3];
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        s[0] += (a[1] - b[1]) * (a[2] + b[2]);
        s[1] += (a[2] - b[2]) * (a[0] + b[0]);
        s[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    s
}

/// Materials of the three surfaces of an extruded solid.
#[derive(Clone, Copy)]
struct Skin {
    face: Material,
    back: Material,
    side: Material,
}

impl Skin {
    fn all(m: Material) -> Self {
        Self {
            face: m,
            back: m,
            side: m,
        }
    }
}

/// Extrude `outer` (with `holes`) from its face plane against the unit outward
/// normal `n` by `depth` inches. `depth <= 0` emits the face only.
fn add_extruded(
    set: &mut MeshSet,
    skin: Skin,
    outer: &[V3d],
    holes: &[Vec<V3d>],
    n: V3d,
    depth: f64,
) {
    if outer.len() < 3 {
        return;
    }
    let axis = (0..3)
        .max_by(|&a, &b| n[a].abs().total_cmp(&n[b].abs()))
        .unwrap_or(1);
    let (ia, ib) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let proj = |p: &V3d| Point::new(p[ia], p[ib]);
    let outer2: Vec<Point> = outer.iter().map(proj).collect();
    let holes2: Vec<Vec<Point>> = holes.iter().map(|h| h.iter().map(proj).collect()).collect();
    let all: Vec<V3d> = outer
        .iter()
        .chain(holes.iter().flatten())
        .copied()
        .collect();
    let uv = |p: V3d| [(p[ia] / IN_PER_FT) as f32, (p[ib] / IN_PER_FT) as f32];
    let drop = depth.max(0.0);
    for [a, b, c] in ear_clip_with_holes(&outer2, &holes2) {
        let tri = [all[a], all[b], all[c]];
        let cr = newell(&tri);
        if dot(cr, cr) < 1e-12 {
            continue; // zero-area bridge triangle
        }
        set.material(skin.face)
            .tri(tri.map(f32v), tri.map(uv), f32v(n));
        if drop > 0.0 {
            let back = tri.map(|p| f32v(sub(p, scale(n, drop))));
            set.material(skin.back)
                .tri(back, tri.map(uv), f32v(scale(n, -1.0)));
        }
    }
    if drop > 0.0 {
        let side = set.material(skin.side);
        add_side_band(side, outer, n, drop, false);
        for h in holes {
            add_side_band(side, h, n, drop, true);
        }
    }
}

/// Side quads around `ring`, from the face down `drop` against `n`. Faces away
/// from the ring's interior, or into it for a hole.
fn add_side_band(mesh: &mut MeshBuilder, ring: &[V3d], n: V3d, drop: f64, is_hole: bool) {
    let m = ring.len();
    let orient = if dot(newell(ring), n) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let flip = if is_hole { -1.0 } else { 1.0 };
    for i in 0..m {
        let (a, b) = (ring[i], ring[(i + 1) % m]);
        let Some(out) = unit(cross(sub(b, a), n)) else {
            continue;
        };
        let out = scale(out, orient * flip);
        let len = dot(sub(b, a), sub(b, a)).sqrt();
        let (a2, b2) = (sub(a, scale(n, drop)), sub(b, scale(n, drop)));
        let u = (len / IN_PER_FT) as f32;
        mesh.quad(
            [a, b, b2, a2].map(f32v),
            [[0.0, 0.0], [u, 0.0], [u, 1.0], [0.0, 1.0]],
            f32v(out),
        );
    }
}

/// Side quads of `ring` between offsets `from` and `to` along `n` (a curb
/// wall), facing away from the ring's interior.
fn add_curb_band(mesh: &mut MeshBuilder, ring: &[V3d], n: V3d, from: f64, to: f64) {
    let m = ring.len();
    let orient = if dot(newell(ring), n) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    for i in 0..m {
        let (a, b) = (ring[i], ring[(i + 1) % m]);
        let Some(out) = unit(cross(sub(b, a), n)) else {
            continue;
        };
        let out = scale(out, orient);
        let up = |p: V3d, k: f64| add(p, scale(n, k));
        let len = dot(sub(b, a), sub(b, a)).sqrt();
        let u = (len / IN_PER_FT) as f32;
        mesh.quad(
            [up(a, from), up(b, from), up(b, to), up(a, to)].map(f32v),
            [[0.0, 0.0], [u, 0.0], [u, 1.0], [0.0, 1.0]],
            f32v(out),
        );
    }
}

fn lifted(ring: &[V3d], n: V3d, k: f64) -> Vec<V3d> {
    ring.iter().map(|&p| add(p, scale(n, k))).collect()
}

fn skylight_into(set: &mut MeshSet, sky: &Skylight) {
    let n = sky.normal;
    let top = sky.spec.curb_height;
    let glass = sky.spec.glass_thickness.clamp(0.0, top);
    // Curb walls: roof surface up to the underside of the glazing.
    if top - glass > 1e-9 {
        add_curb_band(set.material(Material::Roof), &sky.ring, n, 0.0, top - glass);
    }
    // Frame ring and glass share the top `glass` inches of the curb height.
    let ring_top = lifted(&sky.ring, n, top);
    if sky.inner.len() >= 3 {
        let inner_top = lifted(&sky.inner, n, top);
        add_extruded(
            set,
            Skin::all(Material::Trim),
            &ring_top,
            std::slice::from_ref(&inner_top),
            n,
            glass,
        );
        add_extruded(set, Skin::all(Material::Glass), &inner_top, &[], n, glass);
    } else {
        add_extruded(set, Skin::all(Material::Trim), &ring_top, &[], n, glass);
    }
}

/// Curb, frame and glass of one skylight: the curb walls in [`Material::Roof`],
/// the frame ring in [`Material::Trim`] and the pane in [`Material::Glass`].
///
/// Everything lies between the roof surface and `curb_height` above it,
/// measured along the plane normal; the glass fills the top
/// `glass_thickness` of that height inside the frame.
pub fn skylight_meshes(sky: &Skylight) -> Vec<Mesh> {
    let mut set = MeshSet::default();
    skylight_into(&mut set, sky);
    set.finish(None)
}

/// A roof plane as a thick slab with its holes cut through, plus the curb,
/// frame and glass of every skylight on it. The slab hangs `thickness` inches
/// below the roof surface along the plane normal ([`Material::Roof`]).
pub fn roof_plane_meshes(poly: &RoofPolygonWithHoles, thickness: f64) -> Vec<Mesh> {
    let mut set = MeshSet::default();
    add_polygon_with_holes(&mut set, poly, thickness);
    set.finish(None)
}

fn add_polygon_with_holes(set: &mut MeshSet, poly: &RoofPolygonWithHoles, thickness: f64) {
    add_extruded(
        set,
        Skin::all(Material::Roof),
        &poly.outer,
        &poly.holes,
        poly.normal,
        thickness,
    );
    for sky in &poly.skylights {
        skylight_into(set, sky);
    }
}

/// Pitch of the roof over a bay, box or bow window, rise per 12.
pub const BAY_ROOF_PITCH: f64 = 6.0;
/// How far the roof over a projecting window reaches past its panels, inches
/// (none: the unit projects exactly `PROJECTION` from the wall).
pub const BAY_ROOF_OVERHANG: f64 = 0.0;
/// Thickness of that roof, inches.
pub const BAY_ROOF_THICKNESS: f64 = 2.0;

/// A small roof over a projecting window (RF-29, DW-48): `outline` is the
/// unit's open footprint in wall-local `(s, t)` (starting and ending on the
/// wall face, see `plan_core::opening_symbol::projection_footprint`), `head`
/// the height of the roof's eave above the floor of `frame`. A bay or bow
/// gets a hip roof whose back edge rises to the wall, a box window a shed roof
/// that slopes away from the wall. Returns `false` (and adds nothing) when the
/// outline has no area; the caller then draws its flat slab.
pub fn bay_roof_into(
    set: &mut MeshSet,
    frame: &Frame,
    outline: &[(f64, f64)],
    head: f64,
    style: OpeningStyle,
) -> bool {
    if outline.len() < 3 {
        return false;
    }
    let plan = |(s, t): (f64, f64)| {
        let v = frame.point(s, t, head);
        Point::new(f64::from(v[0]), -f64::from(v[2]))
    };
    let eave = f64::from(frame.point(0.0, 0.0, head)[1]);
    let mut ring: Vec<Point> = outline.iter().map(|p| plan(*p)).collect();
    if plan_core::geometry::polygon_area(&ring) < 0.0 {
        ring.reverse();
    }
    // The closing edge (last point back to the first) lies on the wall; it
    // is the last edge whichever way the ring runs.
    let n = ring.len();
    let closing = n - 1;
    let hip = plan_roof::EdgeRoofSpec {
        pitch: BAY_ROOF_PITCH,
        overhang: BAY_ROOF_OVERHANG,
        ..plan_roof::EdgeRoofSpec::default()
    };
    let mut specs = vec![hip; n];
    // Against the wall the roof rises to the wall: no plane, no overhang.
    specs[closing] = plan_roof::EdgeRoofSpec {
        high_shed_gable: true,
        overhang: 0.0,
        ..hip
    };
    if style == OpeningStyle::BoxWindow {
        // A shed roof: the sides are gable ends, only the front slopes.
        for (i, spec) in specs.iter_mut().enumerate() {
            let gable_side = i != closing && i != (closing + n / 2) % n;
            if gable_side {
                spec.full_gable_wall = true;
                spec.overhang = BAY_ROOF_OVERHANG;
            }
        }
    }
    let roof = plan_roof::build_roof_with_specs(&ring, &specs, eave);
    if roof.planes.is_empty() {
        return false;
    }
    for plane in &roof.planes {
        let Some(normal) = unit(newell(&plane.polygon3d)) else {
            continue;
        };
        if normal[1] < 0.0 {
            continue;
        }
        add_extruded(
            set,
            Skin::all(Material::Roof),
            &plane.polygon3d,
            &[],
            normal,
            BAY_ROOF_THICKNESS,
        );
    }
    true
}

/// Every plane of `roof` as a slab, cutting in whichever of `holes` fit on
/// each plane (see [`plan_roof::roof_plane_with_holes`]).
pub fn roof_meshes(roof: &Roof, holes: &[RoofHole], thickness: f64) -> Vec<Mesh> {
    let mut set = MeshSet::default();
    for plane in &roof.planes {
        let poly = plan_roof::roof_plane_with_holes(plane, holes);
        add_polygon_with_holes(&mut set, &poly, thickness);
    }
    set.finish(None)
}

/// A ceiling plane: the visible underside in [`Material::WallInterior`] and
/// the structure above it (`thickness` along the normal, top and edges) in
/// [`CEILING_FRAMING_MATERIAL`].
pub fn ceiling_plane_meshes(ceiling: &CeilingPlane) -> Vec<Mesh> {
    let poly = ceiling.polygon3d();
    let Some(up) = unit(newell(&poly)) else {
        return Vec::new();
    };
    let mut set = MeshSet::default();
    add_extruded(
        &mut set,
        Skin {
            face: Material::WallInterior,
            back: CEILING_FRAMING_MATERIAL,
            side: CEILING_FRAMING_MATERIAL,
        },
        &poly,
        &[],
        scale(up, -1.0),
        ceiling.thickness,
    );
    set.finish(None)
}

/// Does the edge `a -> b` lie along an edge of `poly` (collinear within half
/// an inch, overlapping by more than an inch)?
fn shares_edge(a: V3d, b: V3d, poly: &[V3d]) -> bool {
    let d = sub(b, a);
    let len = dot(d, d).sqrt();
    if len < 1.0 {
        return false;
    }
    let u = scale(d, 1.0 / len);
    let off = |p: V3d| {
        let w = sub(p, a);
        let q = sub(w, scale(u, dot(w, u)));
        dot(q, q).sqrt()
    };
    let m = poly.len();
    (0..m).any(|j| {
        let (c, e) = (poly[j], poly[(j + 1) % m]);
        if off(c) > 0.5 || off(e) > 0.5 {
            return false;
        }
        let (t0, t1) = (dot(sub(c, a), u), dot(sub(e, a), u));
        t1.max(t0).min(len) - t0.min(t1).max(0.0) > 1.0
    })
}

/// Solves `rows * x = rhs` for three planes (Cramer's rule); `None` when the
/// planes do not meet in one point.
fn solve3(rows: [V3d; 3], rhs: [f64; 3]) -> Option<V3d> {
    let det = dot(rows[0], cross(rows[1], rows[2]));
    if det.abs() < 1e-9 {
        return None;
    }
    let x = add(
        add(
            scale(cross(rows[1], rows[2]), rhs[0]),
            scale(cross(rows[2], rows[0]), rhs[1]),
        ),
        scale(cross(rows[0], rows[1]), rhs[2]),
    );
    Some(scale(x, 1.0 / det))
}

/// A ceiling plane whose edges meet the edges of `neighbours` in mitres.
///
/// Where this plane's edge runs along an edge of another ceiling plane (the
/// ridge of a vaulted ceiling, a hip, a valley), the structure above it ends
/// on the plane that bisects the two surfaces instead of on the plane
/// square to its own surface, so the slabs of two planes meet in one line
/// above the joint with no gap and no overlap. Edges nothing meets are cut
/// square, as in [`ceiling_plane_meshes`], which this equals when there are
/// no neighbours.
pub fn ceiling_plane_meshes_joined(
    ceiling: &CeilingPlane,
    neighbours: &[CeilingPlane],
) -> Vec<Mesh> {
    let poly = ceiling.polygon3d();
    let Some(up) = unit(newell(&poly)) else {
        return Vec::new();
    };
    let t = ceiling.thickness;
    let m = poly.len();
    let others: Vec<(V3d, Vec<V3d>)> = neighbours
        .iter()
        .map(|n| (n.normal(), n.polygon3d()))
        .filter(|(n, p)| dot(*n, up) < 1.0 - 1e-6 && p != &poly)
        .collect();
    // The direction each side face of the structure leans in: the plane's own
    // normal for a square edge, the bisector of the two normals at a mitre.
    let lean: Vec<V3d> = (0..m)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % m]);
            others
                .iter()
                .find(|(_, p)| shares_edge(a, b, p))
                .and_then(|(n, _)| unit(add(up, *n)))
                .unwrap_or(up)
        })
        .collect();
    if m < 3 || t <= 1e-9 {
        return ceiling_plane_meshes(ceiling);
    }
    // Top corners: the corner lifted `t` along the normal, slid along its two
    // side planes (the planes through the edges that contain the lean).
    let top: Vec<V3d> = (0..m)
        .map(|i| {
            let p = poly[i];
            let prev = (i + m - 1) % m;
            let side = |e: usize| cross(sub(poly[(e + 1) % m], poly[e]), lean[e]);
            let (n1, n2) = (side(prev), side(i));
            solve3([up, n1, n2], [dot(up, p) + t, dot(n1, p), dot(n2, p)])
                .unwrap_or_else(|| add(p, scale(up, t)))
        })
        .collect();
    let mut set = MeshSet::default();
    let skin = Skin {
        face: Material::WallInterior,
        back: CEILING_FRAMING_MATERIAL,
        side: CEILING_FRAMING_MATERIAL,
    };
    // Underside (face only), top, then the sides.
    add_extruded(&mut set, skin, &poly, &[], scale(up, -1.0), 0.0);
    add_extruded(&mut set, Skin::all(skin.back), &top, &[], up, 0.0);
    let centre = scale(
        poly.iter()
            .chain(top.iter())
            .fold([0.0; 3], |a, v| add(a, *v)),
        1.0 / (2 * m) as f64,
    );
    let side_mesh = set.material(skin.side);
    for i in 0..m {
        let j = (i + 1) % m;
        let (a, b, a2, b2) = (poly[i], poly[j], top[i], top[j]);
        let Some(mut n) = unit(cross(sub(b, a), sub(a2, a))) else {
            continue;
        };
        let mid = scale(add(add(a, b), add(a2, b2)), 0.25);
        if dot(n, sub(mid, centre)) < 0.0 {
            n = scale(n, -1.0);
        }
        let len = dot(sub(b, a), sub(b, a)).sqrt();
        let u = (len / IN_PER_FT) as f32;
        side_mesh.quad(
            [a, b, b2, a2].map(f32v),
            [[0.0, 0.0], [u, 0.0], [u, 1.0], [0.0, 1.0]],
            f32v(n),
        );
    }
    set.finish(None)
}

/// Inset of a four-corner rectangle by `f` on every side (corners in order
/// around the perimeter).
fn inset_rect(r: &[V3d], f: f64) -> Option<Vec<V3d>> {
    if r.len() != 4 {
        return None;
    }
    let (e1, e2) = (sub(r[1], r[0]), sub(r[3], r[0]));
    let (w, h) = (dot(e1, e1).sqrt(), dot(e2, e2).sqrt());
    if w <= 2.0 * f + 1e-9 || h <= 2.0 * f + 1e-9 {
        return None;
    }
    let (u, v) = (scale(e1, 1.0 / w), scale(e2, 1.0 / h));
    let c = add(r[0], add(scale(e1, 0.5), scale(e2, 0.5)));
    let (hw, hh) = (w * 0.5 - f, h * 0.5 - f);
    Some(
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .map(|&(su, sv)| add(c, add(scale(u, su * hw), scale(v, sv * hh))))
            .collect(),
    )
}

fn dormer_wall_into(
    set: &mut MeshSet,
    wall: &DormerWall,
    opening: Option<&WindowOpening>,
    thickness: f64,
) {
    let skin = Skin {
        face: Material::Stucco,
        back: Material::WallInterior,
        side: Material::Stucco,
    };
    let holes: Vec<Vec<V3d>> = opening.map(|w| w.polygon3d.clone()).into_iter().collect();
    add_extruded(set, skin, &wall.polygon3d, &holes, wall.normal, thickness);
    let Some(win) = opening else { return };
    // Frame ring in the wall thickness, glass pane in its middle.
    let n = wall.normal;
    let pane_offset = -(thickness - PANE) * 0.5;
    let pane = match inset_rect(&win.polygon3d, WINDOW_FRAME) {
        Some(inner) => {
            add_extruded(
                set,
                Skin::all(Material::WindowFrame),
                &win.polygon3d,
                std::slice::from_ref(&inner),
                n,
                thickness,
            );
            inner
        }
        None => win.polygon3d.clone(),
    };
    add_extruded(
        set,
        Skin::all(Material::WindowGlass),
        &lifted(&pane, n, pane_offset),
        &[],
        n,
        PANE,
    );
}

/// A dormer's solids: front and cheek walls in [`Material::Stucco`] (inside
/// face [`Material::WallInterior`]) `wall_thickness` thick, the front window
/// opening cut in the front wall with a [`Material::WindowFrame`] ring and a
/// [`Material::WindowGlass`] pane, and the roof planes as `roof_thickness`
/// slabs in [`Material::Roof`].
///
/// The hole in the main roof is not applied here: pass
/// `dormer.hole_in_main_roof` to [`plan_roof::roof_plane_with_holes`] with the
/// main plane.
pub fn dormer_meshes(dormer: &Dormer, wall_thickness: f64, roof_thickness: f64) -> Vec<Mesh> {
    let mut set = MeshSet::default();
    dormer_wall_into(
        &mut set,
        &dormer.front_wall,
        dormer.window_opening.as_ref(),
        wall_thickness,
    );
    for wall in &dormer.side_walls {
        dormer_wall_into(&mut set, wall, None, wall_thickness);
    }
    for plane in &dormer.overhang_planes {
        add_roof_plane(&mut set, plane, roof_thickness);
    }
    set.finish(None)
}

/// A vertical roof face (the triangle under a Dutch gable, see
/// `plan_roof::build_roof_with_faces`) as a wall: `polygon` in roof space with
/// its Newell normal pointing out, `thickness` deep, in [`Material::Stucco`]
/// with the inside in [`Material::WallInterior`].
pub fn gable_face_meshes(polygon: &[[f64; 3]], thickness: f64) -> Vec<Mesh> {
    let Some(n) = unit(newell(polygon)) else {
        return Vec::new();
    };
    let mut set = MeshSet::default();
    add_extruded(
        &mut set,
        Skin {
            face: Material::Stucco,
            back: Material::WallInterior,
            side: Material::Stucco,
        },
        polygon,
        &[],
        n,
        thickness,
    );
    set.finish(None)
}

/// The eave detail of a dormer's roof: fascia and gutters on its eaves, rake
/// boards, soffit under the overhang (`dormer.spec.overhang`), ridge and hip
/// caps where the planes have them on. The valleys against the main roof get
/// nothing. `detail` is the roof's detail (its fascia, soffit and so on).
pub fn dormer_eave_meshes(dormer: &Dormer, detail: &RoofDetail) -> Vec<Mesh> {
    let planes: Vec<EavePlane> = dormer
        .overhang_planes
        .iter()
        .enumerate()
        .map(|(k, plane)| {
            let mut ep = EavePlane::bare(plane.clone());
            ep.overhang = dormer.spec.overhang;
            ep.skip = dormer
                .valley_edges
                .iter()
                .filter(|(p, _)| *p == k)
                .map(|(_, e)| *e)
                .collect();
            ep
        })
        .collect();
    eave_detail_meshes(&planes, detail)
}

fn add_roof_plane(set: &mut MeshSet, plane: &RoofPlane, thickness: f64) {
    let Some(n) = unit(newell(&plane.polygon3d)) else {
        return;
    };
    add_extruded(
        set,
        Skin::all(Material::Roof),
        &plane.polygon3d,
        &[],
        n,
        thickness,
    );
}
