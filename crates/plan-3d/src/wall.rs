//! Wall solids: a box per wall with rectangular holes cut through both faces.

use crate::builder::MeshSet;
use crate::frame::{Axis, Frame};
use crate::mesh::{Material, Mesh};
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
pub fn hole_for(wall: &Wall, opening: &Opening) -> Option<Hole> {
    let hole = Hole {
        s0: opening.start_offset().max(0.0),
        s1: opening.end_offset().min(wall.length()),
        h0: opening.sill_height.max(0.0),
        h1: (opening.sill_height + opening.height).min(wall.height),
        niche_depth: (opening.style == OpeningStyle::WallNiche)
            .then(|| (wall.thickness - 1.0).clamp(0.5, 3.5)),
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
fn add_reveals(frame: &Frame, wall: &Wall, hole: &Hole, set: &mut MeshSet) {
    let half = wall.thickness * 0.5;
    let mesh = set.material(Material::WallInterior);
    let t = (-half, half);
    if hole.s0 > EPS {
        frame.face(mesh, Axis::S, 1.0, hole.s0, t, (hole.h0, hole.h1));
    }
    if hole.s1 < wall.length() - EPS {
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
fn add_niche(frame: &Frame, wall: &Wall, hole: &Hole, side: f64, set: &mut MeshSet) {
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

/// Build the meshes for one wall at `elevation`, with `holes` cut through it.
pub fn build_wall(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    look: WallLook,
) -> Vec<Mesh> {
    let (length, height, half) = (wall.length(), wall.height, wall.thickness * 0.5);
    if length <= EPS || height <= EPS || half <= EPS {
        return Vec::new();
    }
    let frame = Frame::new(wall, elevation);
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
    for (s0, s1, h0, h1) in solid_rects(length, height, &face_holes(1.0)) {
        frame.face(set.material(left), Axis::T, 1.0, half, (s0, s1), (h0, h1));
    }
    for (s0, s1, h0, h1) in solid_rects(length, height, &face_holes(-1.0)) {
        frame.face(
            set.material(right),
            Axis::T,
            -1.0,
            -half,
            (s0, s1),
            (h0, h1),
        );
    }
    let t = (-half, half);
    let cap = set.material(trim);
    frame.face(cap, Axis::S, -1.0, 0.0, t, (0.0, height));
    frame.face(cap, Axis::S, 1.0, length, t, (0.0, height));
    frame.face(cap, Axis::H, 1.0, height, (0.0, length), t);
    frame.face(cap, Axis::H, -1.0, 0.0, (0.0, length), t);
    for hole in holes {
        if hole.niche_depth.is_some() {
            add_niche(&frame, wall, hole, niche_side, &mut set);
        } else {
            add_reveals(&frame, wall, hole, &mut set);
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

    #[test]
    fn opening_outside_the_wall_has_no_hole() {
        let w = wall();
        let door = Opening::default_door(1, w.id, 500.0);
        assert!(hole_for(&w, &door).is_none());
    }
}
