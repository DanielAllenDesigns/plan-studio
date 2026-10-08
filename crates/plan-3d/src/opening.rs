//! Door panels and window frames/glass that fill wall holes.

use crate::builder::MeshSet;
use crate::frame::Frame;
use crate::mesh::{Material, Mesh};
use crate::wall::Hole;
use plan_core::{Opening, OpeningKind, Wall};

/// Door panel thickness, 1 3/8".
const DOOR_THICKNESS: f64 = 1.375;
/// Window frame border width, 1 1/2".
const FRAME_BORDER: f64 = 1.5;
/// Window frame depth through the wall.
const FRAME_DEPTH: f64 = 3.5;
/// Window glass thickness, 1/4".
const GLASS_THICKNESS: f64 = 0.25;

/// Build the meshes that fill `hole` for `opening`.
pub fn build_opening(wall: &Wall, opening: &Opening, hole: &Hole, elevation: f64) -> Vec<Mesh> {
    let frame = Frame::new(wall, elevation);
    let mut set = MeshSet::default();
    match opening.kind {
        OpeningKind::Door => add_door(&frame, hole, &mut set),
        OpeningKind::Window => add_window(&frame, wall.thickness, hole, &mut set),
    }
    set.finish(Some(opening.id))
}

fn add_door(frame: &Frame, hole: &Hole, set: &mut MeshSet) {
    let half = DOOR_THICKNESS * 0.5;
    let panel = set.material(Material::DoorPanel);
    frame.cuboid(panel, (hole.s0, hole.s1), (-half, half), (hole.h0, hole.h1));
}

fn add_window(frame: &Frame, wall_thickness: f64, hole: &Hole, set: &mut MeshSet) {
    let border = FRAME_BORDER
        .min((hole.s1 - hole.s0) / 4.0)
        .min((hole.h1 - hole.h0) / 4.0);
    let depth = FRAME_DEPTH.min(wall_thickness) * 0.5;
    let t = (-depth, depth);
    let (s0, s1, h0, h1) = (hole.s0, hole.s1, hole.h0, hole.h1);

    let ring = set.material(Material::WindowFrame);
    frame.cuboid(ring, (s0, s1), t, (h0, h0 + border));
    frame.cuboid(ring, (s0, s1), t, (h1 - border, h1));
    frame.cuboid(ring, (s0, s0 + border), t, (h0 + border, h1 - border));
    frame.cuboid(ring, (s1 - border, s1), t, (h0 + border, h1 - border));

    let glass = set.material(Material::WindowGlass);
    let g = GLASS_THICKNESS * 0.5;
    frame.cuboid(
        glass,
        (s0 + border, s1 - border),
        (-g, g),
        (h0 + border, h1 - border),
    );
}
