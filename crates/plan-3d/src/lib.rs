//! plan-3d: turns a [`plan_core::Project`] into triangle meshes and glTF 2.0.
//!
//! No GPU code lives here: a [`Scene`] is plain vertex/index buffers that any
//! renderer can upload. Scene space is right-handed with **X = plan x**,
//! **Y = up** (inches, offset by the floor elevation) and **Z = -plan y**, so
//! the plan's Y-up reads as looking down -Z. UVs are in feet.

mod builder;
mod frame;
pub mod gltf;
mod mesh;
mod opening;
mod slab;
pub mod triangulate;
mod wall;

pub use mesh::{Bounds, Material, Mesh, Scene, Vertex};

use plan_core::geometry::point_in_polygon;
use plan_core::{detect_rooms, Floor, Project, Room, Wall, WallKind};
use wall::InteriorSign;

/// Snap tolerance handed to room detection, inches.
const ROOM_TOLERANCE: f64 = 0.5;

/// Build every mesh for the project: per floor, walls, openings, floors and ceilings.
pub fn build_scene(project: &Project) -> Scene {
    let mut scene = Scene::default();
    for floor in &project.floors {
        add_floor(floor, &mut scene);
    }
    scene
}

fn add_floor(floor: &Floor, scene: &mut Scene) {
    let rooms = detect_rooms(&floor.walls, ROOM_TOLERANCE);
    for wall in &floor.walls {
        add_wall(floor, wall, &rooms, scene);
    }
    let finished_floor = floor.elevation + slab::FLOOR_FINISH;
    let slabs = [
        (
            Material::Floor,
            finished_floor - slab::SLAB_THICKNESS,
            finished_floor,
        ),
        (
            Material::Ceiling,
            floor.elevation + floor.ceiling_height,
            floor.elevation + floor.ceiling_height + slab::SLAB_THICKNESS,
        ),
    ];
    for (material, y0, y1) in slabs {
        scene
            .meshes
            .extend(slab::build_slab(material, &rooms, y0, y1));
    }
}

fn add_wall(floor: &Floor, wall: &Wall, rooms: &[Room], scene: &mut Scene) {
    let hosted: Vec<_> = floor
        .openings_on(wall.id)
        .filter_map(|o| wall::hole_for(wall, o).map(|h| (o, h)))
        .collect();
    let holes: Vec<_> = hosted.iter().map(|(_, h)| *h).collect();
    let interior = interior_sign(wall, rooms);
    scene
        .meshes
        .extend(wall::build_wall(wall, floor.elevation, &holes, interior));
    for (opening, hole) in &hosted {
        scene
            .meshes
            .extend(opening::build_opening(wall, opening, hole, floor.elevation));
    }
}

/// The side of the wall facing a detected room (left `1.0`, right `-1.0`).
///
/// Only matters for exterior walls; falls back to the right side when neither
/// side touches a room, which leaves the left face as the exterior.
fn interior_sign(wall: &Wall, rooms: &[Room]) -> InteriorSign {
    if wall.kind == WallKind::Interior {
        return 1.0;
    }
    let mid = wall.point_at(wall.length() * 0.5);
    let reach = wall.thickness * 0.5 + 1.0;
    let side_in_room = |sign: f64| {
        let probe = mid + wall.normal() * (sign * reach);
        rooms.iter().any(|r| point_in_polygon(probe, &r.polygon))
    };
    if side_in_room(1.0) {
        1.0
    } else {
        -1.0
    }
}
