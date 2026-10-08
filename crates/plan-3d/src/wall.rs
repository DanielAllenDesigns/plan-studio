//! Wall solids: a box per wall with rectangular holes cut through both faces.

mod arc;

use crate::builder::{cross, sub, MeshBuilder, MeshSet, V3};
use crate::clip::{area, Piece, Side, TopProfile, P2};
use crate::frame::{Axis, Frame};
use crate::mesh::{Material, Mesh};
pub use arc::EndCuts;
pub(crate) use arc::WallFrame;
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
    let hole = Hole {
        s0: opening.start_offset().max(0.0),
        s1: opening.end_offset().min(wall.path_length()),
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

/// Reveal faces (jambs, head, sill) lining one hole.
fn add_reveals(frame: &WallFrame, wall: &Wall, hole: &Hole, set: &mut MeshSet) {
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
        frame.face(mesh, Axis::H, -1.0, hole.h1, (hole.s0, hole.s1), t);
    }
    if hole.h0 > EPS {
        frame.face(mesh, Axis::H, 1.0, hole.h0, (hole.s0, hole.s1), t);
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
                None => add_reveals(&frame, wall, hole, &mut set),
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
}
