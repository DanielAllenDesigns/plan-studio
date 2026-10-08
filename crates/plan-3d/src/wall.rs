//! Wall solids: a box per wall with rectangular holes cut through both faces.

mod arc;

use crate::builder::{cross, sub, MeshBuilder, MeshSet, V3};
use crate::clip::{area, Piece, Side, TopProfile, P2};
use crate::frame::{Axis, Frame};
use crate::mesh::{Material, Mesh};
pub use arc::EndCuts;
pub(crate) use arc::WallFrame;
use plan_core::walls::PlatformAdjust;
use plan_core::{Opening, OpeningStyle, Wall, WallKind};

/// Geometric tolerance, inches.
const EPS: f64 = 1e-6;

/// A rectangular hole in a wall, in wall-local inches (clamped to the wall).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hole {
    pub s0: f64,
    pub s1: f64,
    pub h0: f64,
    pub h1: f64,
    /// `Some(depth)` for a recess that does not pass through the wall.
    pub niche_depth: Option<f64>,
}

/// Per-wall appearance chosen by the scene builder (wall type, pony wall...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallLook {
    /// Material of the exterior face and the wall's end/top caps.
    pub exterior: Material,
}

impl Default for WallLook {
    fn default() -> Self {
        Self {
            exterior: Material::WallExterior,
        }
    }
}

/// The hole an opening cuts in its wall, or `None` if it falls outside the wall.
///
/// Heights are measured from the floor the wall stands on, so a wall that
/// starts above the floor (`bottom_offset`, a dormer wall) only holds the part
/// of the opening between its bottom and its top. On a curved wall the hole's
/// span is measured along the arc.
pub fn hole_for(wall: &Wall, opening: &Opening) -> Option<Hole> {
    // A size that leaves out the jamb or frame clears the wall wider than the
    // unit by that much on each side (the plan cuts it the same).
    let reach = plan_core::opening_symbol::cleared_reach(opening);
    let hole = Hole {
        s0: (opening.start_offset() - reach).max(0.0),
        s1: (opening.end_offset() + reach).min(wall.path_length()),
        h0: opening.sill_height.max(wall.bottom_offset).max(0.0),
        h1: (opening.sill_height + opening.height).min(wall.bottom_offset + wall.height),
        niche_depth: (opening.style == OpeningStyle::WallNiche)
            .then(|| opening.niche_depth(wall.thickness)),
    };
    (hole.s1 - hole.s0 > EPS && hole.h1 - hole.h0 > EPS).then_some(hole)
}

/// Which side of the wall is the room (interior) side: `1.0` left, `-1.0` right.
pub type InteriorSign = f64;

/// Rectangles `(s0, s1, h0, h1)` covering the wall face except the holes.
///
/// The face is sliced vertically at every hole edge; each strip then keeps the
/// parts below, between and above the holes covering it.
pub fn solid_rects(length: f64, height: f64, holes: &[Hole]) -> Vec<(f64, f64, f64, f64)> {
    let mut cuts = vec![0.0, length];
    cuts.extend(holes.iter().flat_map(|h| [h.s0, h.s1]));
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() <= EPS);

    let mut rects = Vec::new();
    for pair in cuts.windows(2) {
        let (xa, xb) = (pair[0], pair[1]);
        let mut gaps: Vec<(f64, f64)> = holes
            .iter()
            .filter(|h| h.s0 <= xa + EPS && h.s1 >= xb - EPS)
            .map(|h| (h.h0, h.h1))
            .collect();
        gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor = 0.0;
        for (g0, g1) in gaps {
            if g0 - cursor > EPS {
                rects.push((xa, xb, cursor, g0));
            }
            cursor = cursor.max(g1);
        }
        if height - cursor > EPS {
            rects.push((xa, xb, cursor, height));
        }
    }
    rects
}

/// The parts of the head (`head`) or the sill of `hole` that no other
/// hole continues across: where a window stands over a door the two holes are
/// one opening and the head of the door and the sill of the window are not
/// faces of the wall.
fn uncovered(hole: &Hole, others: &[Hole], head: bool) -> Vec<(f64, f64)> {
    let at = if head { hole.h1 } else { hole.h0 };
    let mut free = vec![(hole.s0, hole.s1)];
    for o in others {
        let same = (o.s0 - hole.s0).abs() <= EPS
            && (o.s1 - hole.s1).abs() <= EPS
            && (o.h0 - hole.h0).abs() <= EPS
            && (o.h1 - hole.h1).abs() <= EPS;
        let across = if head {
            o.h0 <= at + EPS && o.h1 > at + EPS
        } else {
            o.h1 >= at - EPS && o.h0 < at - EPS
        };
        if same || o.niche_depth.is_some() || !across {
            continue;
        }
        free = free
            .into_iter()
            .flat_map(|(a, b)| {
                if o.s1 <= a + EPS || o.s0 >= b - EPS {
                    return vec![(a, b)];
                }
                let mut out = Vec::new();
                if o.s0 > a + EPS {
                    out.push((a, o.s0));
                }
                if o.s1 < b - EPS {
                    out.push((o.s1, b));
                }
                out
            })
            .collect();
    }
    free.retain(|(a, b)| b - a > EPS);
    free
}

/// Reveal faces (jambs, head, sill) lining one hole.
fn add_reveals(frame: &WallFrame, wall: &Wall, hole: &Hole, others: &[Hole], set: &mut MeshSet) {
    let half = wall.thickness * 0.5;
    let mesh = set.material(Material::WallInterior);
    let t = (-half, half);
    if hole.s0 > EPS {
        frame.face(mesh, Axis::S, 1.0, hole.s0, t, (hole.h0, hole.h1));
    }
    if hole.s1 < frame.length() - EPS {
        frame.face(mesh, Axis::S, -1.0, hole.s1, t, (hole.h0, hole.h1));
    }
    if hole.h1 < wall.height - EPS {
        for span in uncovered(hole, others, true) {
            frame.face(mesh, Axis::H, -1.0, hole.h1, span, t);
        }
    }
    if hole.h0 > EPS {
        for span in uncovered(hole, others, false) {
            frame.face(mesh, Axis::H, 1.0, hole.h0, span, t);
        }
    }
}

/// Faces lining a recess that stops short of the far face of the wall.
fn add_niche(frame: &WallFrame, wall: &Wall, hole: &Hole, side: f64, set: &mut MeshSet) {
    let depth = hole.niche_depth.unwrap_or(0.0);
    let half = wall.thickness * 0.5;
    let (a, b) = (side * half, side * (half - depth));
    let t = (a.min(b), a.max(b));
    let mesh = set.material(Material::WallInterior);
    frame.face(mesh, Axis::S, 1.0, hole.s0, t, (hole.h0, hole.h1));
    frame.face(mesh, Axis::S, -1.0, hole.s1, t, (hole.h0, hole.h1));
    frame.face(mesh, Axis::H, -1.0, hole.h1, (hole.s0, hole.s1), t);
    frame.face(mesh, Axis::H, 1.0, hole.h0, (hole.s0, hole.s1), t);
    frame.face(
        mesh,
        Axis::T,
        side as f32,
        b,
        (hole.s0, hole.s1),
        (hole.h0, hole.h1),
    );
}

/// Materials for the (left, right) faces and for caps/top/bottom.
fn wall_materials(
    wall: &Wall,
    interior: InteriorSign,
    ext: Material,
) -> (Material, Material, Material) {
    match wall.kind {
        WallKind::Interior => (
            Material::WallInterior,
            Material::WallInterior,
            Material::WallInterior,
        ),
        WallKind::Exterior if interior > 0.0 => (Material::WallInterior, ext, ext),
        WallKind::Exterior => (ext, Material::WallInterior, ext),
    }
}

/// Holes measured from the floor, re-measured from the bottom of a wall that
/// starts `bottom` above it and clipped to its `height`.
fn holes_from_wall_bottom(holes: &[Hole], bottom: f64, height: f64) -> Vec<Hole> {
    holes
        .iter()
        .filter_map(|h| {
            let (h0, h1) = ((h.h0 - bottom).max(0.0), (h.h1 - bottom).min(height));
            (h1 - h0 > EPS).then_some(Hole { h0, h1, ..*h })
        })
        .collect()
}

/// Build the meshes for one wall at `elevation`, with `holes` cut through it.
///
/// The wall spans `wall.bottom_offset..wall.bottom_offset + wall.height` above
/// `elevation`; `holes` are measured from `elevation` too (as [`hole_for`]
/// makes them).
#[cfg_attr(not(test), allow(dead_code))]
pub fn build_wall(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    look: WallLook,
) -> Vec<Mesh> {
    build_wall_with_top(wall, elevation, holes, interior, look, None)
}

/// [`build_wall`] whose top follows `top` (heights above the wall's bottom)
/// instead of the level `wall.height`: a gable triangle, a top clipped to a
/// roof plane, a top that rises to a vaulted ceiling. `None`, or a top level
/// at the wall height, builds the plain box.
pub fn build_wall_with_top(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    look: WallLook,
    top: Option<&TopProfile>,
) -> Vec<Mesh> {
    build_wall_with_top_cut(wall, elevation, holes, interior, look, top, &EndCuts::NONE)
}

/// [`build_wall_with_top`] with the ends of a curved wall cut by `cuts`.
pub fn build_wall_with_top_cut(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    look: WallLook,
    top: Option<&TopProfile>,
    cuts: &EndCuts,
) -> Vec<Mesh> {
    let shape = Shape {
        top,
        ..Shape::default()
    };
    build_wall_shaped_cut(wall, elevation, holes, interior, look, &shape, cuts)
}

/// Where the exterior face of a wall changes material along a roof line
/// (Lower Wall Type if Split by Butting Roof, RF-28; the part of a gable
/// above the plate, RF-31): under `profile` the face is `below`, over it
/// `above`; `None` keeps the wall's own material there.
#[derive(Debug, Clone, PartialEq)]
pub struct Split {
    pub profile: TopProfile,
    pub below: Option<Material>,
    pub above: Option<Material>,
}

/// How the roof shapes a wall: its top, its bottom (cut by a roof under it)
/// and its material split. Heights are measured from the wall's bottom.
#[derive(Debug, Clone, Copy, Default)]
pub struct Shape<'a> {
    pub top: Option<&'a TopProfile>,
    pub bottom: Option<&'a TopProfile>,
    pub split: Option<&'a Split>,
}

/// The rectangle `s0..s1` x `h0..h1` cut to the region between the bottom
/// and top profiles.
fn shaped_polys(shape: &Shape, s0: f64, s1: f64, h0: f64, h1: f64) -> Vec<Vec<P2>> {
    let base = match shape.top {
        Some(t) => t.clip_rect(s0, s1, h0, h1),
        None => vec![vec![(s0, h0), (s1, h0), (s1, h1), (s0, h1)]],
    };
    match shape.bottom {
        Some(b) => base.iter().flat_map(|p| b.clip_above(p)).collect(),
        None => base,
    }
}

/// [`build_wall_with_top`] with the full [`Shape`] (a bottom that follows a
/// roof below the wall and a split of the exterior material) for a wall that
/// may be curved. A curved wall is a run
/// of facets along its arc (`s` in the holes, the roof profiles and the
/// split is the arc length); `cuts` mitres its ends against the walls joined
/// to it. A straight wall ignores `cuts`.
pub fn build_wall_shaped_cut(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    look: WallLook,
    shape: &Shape,
    cuts: &EndCuts,
) -> Vec<Mesh> {
    let (height, half) = (wall.height, wall.thickness * 0.5);
    let frame = WallFrame::new(wall, elevation + wall.bottom_offset, cuts);
    let length = frame.length();
    if length <= EPS || height <= EPS || half <= EPS {
        return Vec::new();
    }
    let raised;
    let holes = if wall.bottom_offset == 0.0 {
        holes
    } else {
        raised = holes_from_wall_bottom(holes, wall.bottom_offset, height);
        &raised
    };
    let top = shape.top.filter(|t| !t.is_flat_at(height));
    let shape = Shape { top, ..*shape };
    let (left, right, trim) = wall_materials(wall, interior, look.exterior);
    let mut set = MeshSet::default();

    // Niches only break the face on the interior side.
    let niche_side = if interior >= 0.0 { 1.0 } else { -1.0 };
    let face_holes = |side: f64| -> Vec<Hole> {
        holes
            .iter()
            .filter(|h| h.niche_depth.is_none() || side == niche_side)
            .copied()
            .collect()
    };
    let face_top = top.map_or(height, TopProfile::max_height);
    let plain = shape.top.is_none() && shape.bottom.is_none() && shape.split.is_none();
    for (side, material, at) in [(1.0, left, half), (-1.0, right, -half)] {
        for (s0, s1, h0, h1) in solid_rects(length, face_top, &face_holes(side)) {
            if plain {
                frame.face(
                    set.material(material),
                    Axis::T,
                    side as f32,
                    at,
                    (s0, s1),
                    (h0, h1),
                );
                continue;
            }
            // Only the exterior face changes material along the split.
            let split = shape.split.filter(|_| material == look.exterior);
            for poly in shaped_polys(&shape, s0, s1, h0, h1) {
                match split {
                    None => poly_face(&frame, set.material(material), &poly, at, side as f32),
                    Some(sp) => {
                        for (part, m) in [
                            (sp.profile.clip_below(&poly), sp.below),
                            (sp.profile.clip_above(&poly), sp.above),
                        ] {
                            for q in part {
                                let mesh = set.material(m.unwrap_or(material));
                                poly_face(&frame, mesh, &q, at, side as f32);
                            }
                        }
                    }
                }
            }
        }
    }
    let t = (-half, half);
    let cap = set.material(trim);
    if shape.top.is_none() && shape.bottom.is_none() {
        frame.face(cap, Axis::S, -1.0, 0.0, t, (0.0, height));
        frame.face(cap, Axis::S, 1.0, length, t, (0.0, height));
        frame.face(cap, Axis::H, 1.0, height, (0.0, length), t);
        frame.face(cap, Axis::H, -1.0, 0.0, (0.0, length), t);
    } else {
        add_shaped_caps(&frame, cap, &shape, height, length, half);
    }
    for hole in holes {
        if hole.niche_depth.is_some() {
            add_niche(&frame, wall, hole, niche_side, &mut set);
        } else {
            match shape.top {
                None => add_reveals(&frame, wall, hole, holes, &mut set),
                Some(profile) => add_reveals_under(&frame, wall, hole, profile, &mut set),
            }
        }
    }
    set.finish(Some(wall.id))
}

/// Unit scene-space direction of the wall-local axis `d(s, t, h)`.
fn local_vec(frame: &Frame, s: f64, t: f64, h: f64) -> V3 {
    sub(frame.point(s, t, h), frame.point(0.0, 0.0, 0.0))
}

/// A convex polygon given in `(s, h)` on the face at `t`, facing `sign` along
/// the wall's cross axis. Fan-triangulated.
fn poly_face(frame: &WallFrame, mesh: &mut MeshBuilder, poly: &[P2], t: f64, sign: f32) {
    let frame = match frame {
        WallFrame::Straight(f, _) => f,
        WallFrame::Arc(a) => return a.poly_face(mesh, poly, t, sign),
    };
    let normal = local_vec(frame, 0.0, 1.0, 0.0).map(|c| c * sign);
    let pts: Vec<V3> = poly.iter().map(|&(s, h)| frame.point(s, t, h)).collect();
    let uv: Vec<[f32; 2]> = poly
        .iter()
        .map(|&(s, h)| [(s / 12.0) as f32, (h / 12.0) as f32])
        .collect();
    for i in 1..poly.len().saturating_sub(1) {
        mesh.tri(
            [pts[0], pts[i], pts[i + 1]],
            [uv[0], uv[i], uv[i + 1]],
            normal,
        );
    }
}

/// A sloped band along a profile piece, facing up (`up`) or down.
fn band(frame: &WallFrame, cap: &mut MeshBuilder, p: &Piece, half: f64, up: bool) {
    let frame = match frame {
        WallFrame::Straight(f, _) => f,
        WallFrame::Arc(a) => return a.band(cap, (p.s0, p.s1, p.h0, p.h1), up),
    };
    let pts = [
        frame.point(p.s0, -half, p.h0),
        frame.point(p.s1, -half, p.h1),
        frame.point(p.s1, half, p.h1),
        frame.point(p.s0, half, p.h0),
    ];
    let mut n = cross(sub(pts[1], pts[0]), sub(pts[3], pts[0]));
    if (n[1] < 0.0) == up {
        n = n.map(|c| -c);
    }
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
    let uv = |s: f64, t: f64| [(s / 12.0) as f32, (t / 12.0) as f32];
    cap.quad(
        pts,
        [
            uv(p.s0, -half),
            uv(p.s1, -half),
            uv(p.s1, half),
            uv(p.s0, half),
        ],
        n.map(|c| c / len),
    );
}

/// End faces, risers at jumps and the sloped top and bottom bands of a wall
/// whose top and bottom follow `shape` (a missing profile is level: the top
/// at `height`, the bottom at zero).
fn add_shaped_caps(
    frame: &WallFrame,
    cap: &mut MeshBuilder,
    shape: &Shape,
    height: f64,
    length: f64,
    half: f64,
) {
    let t = (-half, half);
    let top_at = |s: f64, side: Side| shape.top.map_or(height, |p| p.at(s, side));
    let bot_at = |s: f64, side: Side| shape.bottom.map_or(0.0, |p| p.at(s, side));
    let (b0, t0) = (bot_at(0.0, Side::After), top_at(0.0, Side::After));
    let (b1, t1) = (bot_at(length, Side::Before), top_at(length, Side::Before));
    if t0 - b0 > EPS {
        frame.face(cap, Axis::S, -1.0, 0.0, t, (b0, t0));
    }
    if t1 - b1 > EPS {
        frame.face(cap, Axis::S, 1.0, length, t, (b1, t1));
    }
    match shape.top {
        None => frame.face(cap, Axis::H, 1.0, height, (0.0, length), t),
        Some(profile) => {
            for pair in profile.pieces.windows(2) {
                let (before, after) = (pair[0].h1, pair[1].h0);
                if (before - after).abs() > EPS {
                    // The riser faces the lower side.
                    let sign = if after > before { -1.0 } else { 1.0 };
                    frame.face(
                        cap,
                        Axis::S,
                        sign,
                        pair[0].s1,
                        t,
                        (before.min(after), before.max(after)),
                    );
                }
            }
            for p in profile.pieces.iter().filter(|p| p.s1 - p.s0 > EPS) {
                band(frame, cap, p, half, true);
            }
        }
    }
    match shape.bottom {
        None => frame.face(cap, Axis::H, -1.0, 0.0, (0.0, length), t),
        Some(profile) => {
            for pair in profile.pieces.windows(2) {
                let (before, after) = (pair[0].h1, pair[1].h0);
                if (before - after).abs() > EPS {
                    // The riser faces the empty side, the lower bottom's.
                    let sign = if after > before { 1.0 } else { -1.0 };
                    frame.face(
                        cap,
                        Axis::S,
                        sign,
                        pair[0].s1,
                        t,
                        (before.min(after), before.max(after)),
                    );
                }
            }
            for p in profile.pieces.iter().filter(|p| p.s1 - p.s0 > EPS) {
                band(frame, cap, p, half, false);
            }
        }
    }
}

/// [`add_reveals`] for a wall whose top is `profile`: jambs stop at the top,
/// the head is drawn only when the opening sits under it.
fn add_reveals_under(
    frame: &WallFrame,
    wall: &Wall,
    hole: &Hole,
    profile: &TopProfile,
    set: &mut MeshSet,
) {
    let half = wall.thickness * 0.5;
    let mesh = set.material(Material::WallInterior);
    let t = (-half, half);
    if hole.s0 > EPS {
        let h1 = hole.h1.min(profile.at(hole.s0, Side::After));
        if h1 - hole.h0 > EPS {
            frame.face(mesh, Axis::S, 1.0, hole.s0, t, (hole.h0, h1));
        }
    }
    if hole.s1 < frame.length() - EPS {
        let h1 = hole.h1.min(profile.at(hole.s1, Side::Before));
        if h1 - hole.h0 > EPS {
            frame.face(mesh, Axis::S, -1.0, hole.s1, t, (hole.h0, h1));
        }
    }
    if hole.h1 < profile.min_over(hole.s0, hole.s1) - EPS {
        frame.face(mesh, Axis::H, -1.0, hole.h1, (hole.s0, hole.s1), t);
    }
    if hole.h0 > EPS {
        frame.face(mesh, Axis::H, 1.0, hole.h0, (hole.s0, hole.s1), t);
    }
}

/// A solid panel standing on `wall`'s line: the polygon `poly` in `(s, h)`
/// (`h` is an absolute scene elevation), as thick as the wall, in
/// `material`. Not selectable (no object id). Used for the attic walls the roof generates.
pub fn build_panel(wall: &Wall, poly: &[P2], material: Material) -> Vec<Mesh> {
    let half = wall.thickness * 0.5;
    if poly.len() < 3 || half <= EPS {
        return Vec::new();
    }
    let mut ring = poly.to_vec();
    if area(&ring) < 0.0 {
        ring.reverse();
    }
    let frame = Frame::new(wall, 0.0);
    let face_frame = WallFrame::Straight(frame, wall.length());
    let mut set = MeshSet::default();
    let mesh = set.material(material);
    poly_face(&face_frame, mesh, &ring, half, 1.0);
    poly_face(&face_frame, mesh, &ring, -half, -1.0);
    let s_axis = local_vec(&frame, 1.0, 0.0, 0.0);
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let (ds, dh) = (b.0 - a.0, b.1 - a.1);
        let len = ds.hypot(dh);
        if len <= EPS {
            continue;
        }
        // Counter-clockwise ring: the outward normal is to the right of the edge.
        let (ns, nh) = ((dh / len) as f32, (-ds / len) as f32);
        let normal = [s_axis[0] * ns, nh, s_axis[2] * ns];
        let pts = [
            frame.point(a.0, -half, a.1),
            frame.point(b.0, -half, b.1),
            frame.point(b.0, half, b.1),
            frame.point(a.0, half, a.1),
        ];
        let u = (len / 12.0) as f32;
        mesh.quad(pts, [[0.0, 0.0], [u, 0.0], [u, 1.0], [0.0, 1.0]], normal);
    }
    set.finish(None)
}

/// The wall as the platform options (Structure tab, W-62, R-69) shape it:
/// the bottom lowered to the platform below it and the top raised through the
/// platforms above it. Walls the options do not move come back unchanged.
#[cfg_attr(not(test), allow(dead_code))]
pub fn with_platforms(wall: &Wall, adj: PlatformAdjust) -> Wall {
    let mut w = wall.clone();
    w.bottom_offset -= adj.lower;
    w.height += adj.lower + adj.raise;
    w
}

/// The footing under a foundation wall, its sill plate and its cap
/// (Foundation and Wall Cap tabs, W-52). `wall` is the wall as drawn
/// (standing from `wall.bottom_offset` for `wall.height` above `elevation`);
/// `main_center` is the lateral offset of the main layer's center from the
/// centerline. Curved walls get none.
pub fn spec_meshes(wall: &Wall, elevation: f64, main_center: f64) -> Vec<Mesh> {
    if wall.is_curved() || wall.length() <= EPS {
        return Vec::new();
    }
    // A foundation wall stands below the floor and a half wall is lower than
    // its height: the extent the class builds.
    let mut drawn = wall.clone();
    match &wall.class {
        plan_core::WallClass::Foundation => {
            drawn.bottom_offset = wall.bottom_offset - wall.foundation_height;
            drawn.height = wall.foundation_height;
        }
        plan_core::WallClass::HalfWall { height } => {
            drawn.height = height.min(wall.height).max(0.0);
        }
        _ => {}
    }
    let wall = &drawn;
    let spec = &wall.spec;
    let frame = Frame::new(wall, elevation + wall.bottom_offset);
    let length = wall.length();
    let mut set = MeshSet::default();
    let mut put = |material: Material, b: plan_core::walls::WallBox, up: f64| {
        frame.cuboid(
            set.material(material),
            (0.0, length),
            b.across,
            (b.up.0 + up, b.up.1 + up),
        );
    };
    if let Some(b) = spec.foundation.footing_box(wall, main_center) {
        put(Material::Concrete, b, 0.0);
    }
    // The cap rests on the sill plate when the wall has one.
    let mut rise = wall.height;
    if wall.class == plan_core::WallClass::Foundation {
        if let Some(b) = spec.foundation.sill_box(wall, wall.height) {
            put(Material::Framing, b, 0.0);
            rise = b.up.1;
        }
    }
    if let Some(b) = spec.cap.cap_box(wall) {
        put(Material::Trim, b, rise);
    }
    set.finish(Some(wall.id))
}

/// The stretches of a wall `length` long that no hole blocks between the
/// heights `lo..hi` above the wall bottom (a through hole whose height range
/// overlaps the band interrupts it; a niche does not).
fn free_spans(length: f64, holes: &[Hole], bottom: f64, lo: f64, hi: f64) -> Vec<(f64, f64)> {
    let mut blocks: Vec<(f64, f64)> = holes
        .iter()
        .filter(|h| h.niche_depth.is_none() && h.h0 - bottom < hi && h.h1 - bottom > lo)
        .map(|h| (h.s0, h.s1))
        .collect();
    blocks.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut at = 0.0;
    for (s0, s1) in blocks {
        if s0 - at > 0.5 {
            out.push((at, s0.min(length)));
        }
        at = at.max(s1);
    }
    if length - at > 0.5 {
        out.push((at, length));
    }
    out
}

/// Sweeps a molding `section` (`(projection, height)` points, counter-
/// clockwise) along the wall from `s0` to `s1`, standing on the face at
/// lateral `t_face` and projecting toward `sign`, its bottom `h0` above the
/// wall bottom.
fn extrude_profile(
    frame: &Frame,
    mesh: &mut MeshBuilder,
    section: &[(f64, f64)],
    (sign, t_face, h0): (f64, f64, f64),
    (s0, s1): (f64, f64),
) {
    let n = section.len();
    if n < 3 || s1 - s0 < 1e-6 {
        return;
    }
    let origin = frame.point(0.0, 0.0, 0.0);
    let t_axis = sub(frame.point(0.0, 1.0, 0.0), origin);
    let s_axis = sub(frame.point(1.0, 0.0, 0.0), origin);
    let at = |s: f64, p: (f64, f64)| frame.point(s, t_face + sign * p.0, h0 + p.1);
    let len_ft = ((s1 - s0) / 12.0) as f32;
    for i in 0..n {
        let (a, b) = (section[i], section[(i + 1) % n]);
        let (dp, dh) = (b.0 - a.0, b.1 - a.1);
        let l = dp.hypot(dh);
        if l < 1e-9 {
            continue;
        }
        // Outward normal of a counter-clockwise edge: its direction turned
        // a quarter clockwise.
        let (np, nh) = (dh / l, -dp / l);
        let normal = [
            t_axis[0] * (sign * np) as f32,
            t_axis[1] * (sign * np) as f32 + nh as f32,
            t_axis[2] * (sign * np) as f32,
        ];
        mesh.quad(
            [at(s0, a), at(s1, a), at(s1, b), at(s0, b)],
            [[0.0, 0.0], [len_ft, 0.0], [len_ft, (l / 12.0) as f32], [0.0, (l / 12.0) as f32]],
            normal,
        );
    }
    let pts: Vec<plan_core::Point> = section
        .iter()
        .map(|p| plan_core::Point::new(p.0, p.1))
        .collect();
    for tri in crate::triangulate::ear_clip(&pts) {
        for (s, dir) in [(s0, -1.0_f32), (s1, 1.0)] {
            let p = tri.map(|k| at(s, section[k]));
            let uv = tri.map(|k| [(section[k].0 / 12.0) as f32, (section[k].1 / 12.0) as f32]);
            mesh.tri(p, uv, s_axis.map(|c| c * dir));
        }
    }
}

/// The wall coverings of the Wall Covering tab (W-115): a wainscot or a
/// full-height covering as a slab on the face, base, chair rail and crown
/// molding swept along it from the library profile. Each band stops at doors
/// and windows that reach into its height and runs between the joined
/// corners of the wall's outline. `wall` is the wall as drawn (its height is
/// the top the crown hangs from); `holes` are its openings. A wall with no
/// covering, and a curved wall, give nothing.
pub fn covering_meshes(floor: &plan_core::Floor, wall: &Wall, elevation: f64, holes: &[Hole]) -> Vec<Mesh> {
    let spec = &wall.spec.covering;
    if spec.is_empty() || wall.is_curved() || wall.length() <= EPS {
        return Vec::new();
    }
    let length = wall.length();
    let height = wall.covering_height();
    let bands = spec.bands(height);
    if bands.is_empty() {
        return Vec::new();
    }
    // The joined corners give each face its stretch (left face first).
    let dir = wall.direction();
    let mut faces = [(0.0, length), (0.0, length)];
    if let Some(o) = plan_core::joins::wall_outlines(&floor.walls, 0.5)
        .into_iter()
        .find(|o| o.wall_id == wall.id && o.polygon.len() == 4)
    {
        let s = |p: plan_core::Point| (p - wall.start).dot(dir).clamp(0.0, length);
        let q = &o.polygon;
        faces[0] = (s(q[0]).min(s(q[1])), s(q[0]).max(s(q[1])));
        faces[1] = (s(q[3]).min(s(q[2])), s(q[3]).max(s(q[2])));
    }
    let frame = Frame::new(wall, elevation + wall.bottom_offset);
    let mut set = MeshSet::default();
    for band in &bands {
        let sign = wall.covering_face_sign(band.side);
        let (f0, f1) = faces[usize::from(sign < 0.0)];
        let t_face = sign * wall.thickness * 0.5;
        let plain = match (band.kind, band.side) {
            (plan_core::walls::BandKind::Covering | plan_core::walls::BandKind::Wainscot, plan_core::walls::CoveringSide::Interior) => {
                Material::WallInterior
            }
            (plan_core::walls::BandKind::Covering | plan_core::walls::BandKind::Wainscot, _) => {
                Material::WallExterior
            }
            _ => Material::Trim,
        };
        let material = crate::details::material_of(band.material, plain);
        for (a, b) in free_spans(length, holes, wall.bottom_offset, band.lo, band.hi) {
            let (a, b) = (a.max(f0), b.min(f1));
            if b - a < 0.5 {
                continue;
            }
            let mesh = set.material(material);
            match band.profile {
                Some(def) => extrude_profile(&frame, mesh, def.section, (sign, t_face, band.lo), (a, b)),
                None => {
                    let t = (t_face, t_face + sign * band.depth);
                    frame.cuboid(mesh, (a, b), (t.0.min(t.1), t.0.max(t.1)), (band.lo, band.hi));
                }
            }
        }
    }
    set.finish(Some(wall.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{OpeningKind, Point, Project};

    fn wall() -> Wall {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            4.5,
            100.0,
            WallKind::Interior,
        );
        p.floors[0].wall(id).unwrap().clone()
    }

    #[test]
    fn no_holes_is_one_rect() {
        assert_eq!(solid_rects(120.0, 100.0, &[]).len(), 1);
    }

    #[test]
    fn door_hole_leaves_three_strips_with_head_above() {
        let w = wall();
        let door = Opening::default_door(1, w.id, 60.0);
        let hole = hole_for(&w, &door).unwrap();
        let rects = solid_rects(120.0, 100.0, &[hole]);
        assert_eq!(rects.len(), 3);
        let area: f64 = rects.iter().map(|r| (r.1 - r.0) * (r.3 - r.2)).sum();
        assert!((area - (120.0 * 100.0 - 36.0 * 80.0)).abs() < 1e-6);
    }

    #[test]
    fn window_hole_leaves_rects_below_and_above() {
        let w = wall();
        let win = Opening::default_window(2, w.id, 60.0);
        assert_eq!(win.kind, OpeningKind::Window);
        let hole = hole_for(&w, &win).unwrap();
        let rects = solid_rects(120.0, 100.0, &[hole]);
        assert_eq!(rects.len(), 4);
    }

    fn y_range(meshes: &[Mesh]) -> (f32, f32) {
        let ys: Vec<f32> = meshes
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .collect();
        (
            ys.iter().copied().fold(f32::MAX, f32::min),
            ys.iter().copied().fold(f32::MIN, f32::max),
        )
    }

    #[test]
    fn a_raised_wall_spans_bottom_offset_to_bottom_plus_height() {
        let mut w = wall();
        let floor = 12.0;
        let flat = build_wall(&w, floor, &[], 1.0, WallLook::default());
        assert_eq!(y_range(&flat), (12.0, 112.0));
        w.bottom_offset = 30.0;
        let raised = build_wall(&w, floor, &[], 1.0, WallLook::default());
        assert_eq!(y_range(&raised), (42.0, 142.0));
        // Same face area, just higher.
        let area = |m: &[Mesh]| m.iter().map(|x| x.indices.len()).sum::<usize>();
        assert_eq!(area(&flat), area(&raised));
    }

    #[test]
    fn openings_in_a_raised_wall_keep_their_height_above_the_floor() {
        let mut w = wall();
        w.bottom_offset = 30.0;
        // A window with its sill at 50" above the floor: 20" above the wall bottom.
        let mut win = Opening::default_window(2, w.id, 60.0);
        win.sill_height = 50.0;
        win.height = 24.0;
        let hole = hole_for(&w, &win).unwrap();
        assert_eq!((hole.h0, hole.h1), (50.0, 74.0));
        // A door whose head is below the wall bottom falls outside the wall;
        // one that starts below it is cut at the wall bottom.
        let mut low = Opening::default_window(3, w.id, 20.0);
        low.sill_height = 0.0;
        low.height = 20.0;
        assert!(hole_for(&w, &low).is_none());
        low.height = 50.0;
        let cut = hole_for(&w, &low).unwrap();
        assert_eq!((cut.h0, cut.h1), (30.0, 50.0));
        let raised = build_wall(&w, 0.0, &[hole], 1.0, WallLook::default());
        assert!(!raised.is_empty());
        // The hole is cut at 50..74 above the floor: reveals sit there.
        let reveal_ys: Vec<f32> = raised
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .filter(|y| (*y - 50.0).abs() < 1e-3 || (*y - 74.0).abs() < 1e-3)
            .collect();
        assert!(!reveal_ys.is_empty(), "no reveal at the sill or head");
    }

    #[test]
    fn opening_outside_the_wall_has_no_hole() {
        let w = wall();
        let door = Opening::default_door(1, w.id, 500.0);
        assert!(hole_for(&w, &door).is_none());
    }

    #[test]
    fn platforms_lower_the_bottom_and_raise_the_top() {
        let w = wall();
        let adj = PlatformAdjust {
            lower: 10.0,
            raise: 12.0,
        };
        let moved = with_platforms(&w, adj);
        assert_eq!((moved.bottom_offset, moved.height), (-10.0, 122.0));
        let m = build_wall(&moved, 0.0, &[], 1.0, WallLook::default());
        assert_eq!(y_range(&m), (-10.0, 112.0));
        assert_eq!(
            with_platforms(&w, PlatformAdjust::default()).height,
            w.height
        );
    }

    #[test]
    fn a_footing_cap_and_sill_are_built_from_the_wall_spec() {
        let mut w = wall();
        w.class = plan_core::WallClass::Foundation;
        w.height = 48.0;
        w.spec.foundation.footing = true;
        w.spec.foundation.sill_plate = true;
        w.spec.cap.enabled = true;
        let m = spec_meshes(&w, 0.0, 0.0);
        let (lo, hi) = y_range(&m);
        // The 48" stem wall reaches below the floor, the 12" footing under it;
        // the sill plate (1.5") and the cap (1.5") stand on its top.
        assert_eq!(lo, -60.0);
        assert!((hi - 3.0).abs() < 1e-3, "{hi}");
        let concrete = m
            .iter()
            .filter(|x| x.material == Material::Concrete)
            .count();
        let trim = m.iter().filter(|x| x.material == Material::Trim).count();
        let framing = m.iter().filter(|x| x.material == Material::Framing).count();
        assert_eq!((concrete, trim, framing), (1, 1, 1));
        // Nothing is added to a plain wall.
        assert!(spec_meshes(&wall(), 0.0, 0.0).is_empty());
    }

    /// Two stacked floors with a wall on the lower one; returns the project
    /// and the wall's id.
    fn two_floors() -> (Project, u64) {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            p.floors[0].ceiling_height,
            WallKind::Exterior,
        );
        p.build_new_floor(false);
        (p, id)
    }

    fn wall_y_range(p: &Project, id: u64) -> (f32, f32) {
        let scene = crate::build_scene(p);
        let own: Vec<Mesh> = scene
            .meshes
            .into_iter()
            .filter(|m| m.object_id == Some(id) && m.material != Material::Concrete)
            .collect();
        assert!(!own.is_empty());
        y_range(&own)
    }

    #[test]
    fn a_balloon_wall_runs_up_through_the_platforms_and_a_stopped_one_does_not() {
        use plan_core::walls::CeilingPlatform;
        let (mut p, id) = two_floors();
        let ceiling = p.floors[0].ceiling_height as f32;
        let (_, top) = wall_y_range(&p, id);
        assert!((top - ceiling).abs() < 1e-3, "{top} vs {ceiling}");
        let through = (p.floors[0].settings.ceiling_structure_thickness
            + p.floors[1].settings.floor_structure_thickness) as f32;
        assert!(through > 0.0);
        p.floors[0]
            .wall_mut(id)
            .unwrap()
            .spec
            .structure
            .ceiling_platform = CeilingPlatform::BalloonThroughCeilingAbove;
        let (_, top) = wall_y_range(&p, id);
        assert!((top - (ceiling + through)).abs() < 1e-3, "{top}");
        // The top of the floor platform above is the wall top.
        let upper = (p.floors[1].elevation) as f32;
        assert!((top - upper).abs() < 1e-3, "{top} vs {upper}");
        p.floors[0]
            .wall_mut(id)
            .unwrap()
            .spec
            .structure
            .ceiling_platform = CeilingPlatform::StopAtCeilingAbove;
        let (_, top) = wall_y_range(&p, id);
        assert!((top - ceiling).abs() < 1e-3);
    }

    #[test]
    fn stop_at_floor_below_drops_the_upper_wall_through_its_floor_platform() {
        use plan_core::walls::FloorPlatform;
        let (mut p, _) = two_floors();
        let up = p.add_wall(
            1,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            p.floors[1].ceiling_height,
            WallKind::Exterior,
        );
        let elevation = p.floors[1].elevation as f32;
        let scene = |p: &Project| {
            let s = crate::build_scene(p);
            let m: Vec<Mesh> = s
                .meshes
                .into_iter()
                .filter(|m| m.object_id == Some(up))
                .collect();
            y_range(&m)
        };
        assert!((scene(&p).0 - elevation).abs() < 1e-3);
        p.floors[1]
            .wall_mut(up)
            .unwrap()
            .spec
            .structure
            .floor_platform = FloorPlatform::StopAtFloorBelow;
        let drop = p.floors[1].settings.floor_structure_thickness as f32;
        assert!((scene(&p).0 - (elevation - drop)).abs() < 1e-3);
        // The ground floor has no floor below: nothing moves.
        if let Some(w) = p.floors[0].wall_mut(1) {
            w.spec.structure.floor_platform = FloorPlatform::BalloonThroughFloorBelow;
        }
        assert_eq!(wall_y_range(&p, 1).0, p.floors[0].elevation as f32);
    }

    #[test]
    fn a_foundation_footing_and_a_half_wall_cap_show_in_the_scene() {
        let (mut p, id) = two_floors();
        {
            let w = p.floors[0].wall_mut(id).unwrap();
            w.set_class(plan_core::WallClass::HalfWall { height: 36.0 });
            w.spec.cap.enabled = true;
            w.spec.foundation.footing = true;
        }
        let scene = crate::build_scene(&p);
        let own: Vec<&Mesh> = scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id))
            .collect();
        let top = own
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::MIN, f32::max);
        // The half wall is 36" with a 1.5" flat cap on top.
        assert!((top - 37.5).abs() < 1e-3, "{top}");
        assert!(own.iter().any(|m| m.material == Material::Concrete));
        assert!(own.iter().any(|m| m.material == Material::Trim));
    }
}
