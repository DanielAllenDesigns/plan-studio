//! A named room's ceiling height and floor height offset reach the 3D
//! platforms (R-23, R-24, R-33); rooms without an override keep the floor's.

use plan_3d::{build_scene, Material};
use plan_core::{Point, Project, RoomName, WallKind};

/// Two 120" x 120" rooms side by side sharing a partition.
fn two_rooms() -> Project {
    let mut p = Project::new("two rooms");
    let c = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 120.0),
        Point::new(0.0, 120.0),
    ];
    for i in 0..4 {
        p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
    }
    p.add_wall(
        0,
        Point::new(120.0, 0.0),
        Point::new(120.0, 120.0),
        4.5,
        109.0,
        WallKind::Interior,
    );
    p
}

/// Highest vertex of the meshes of `material` with plan x in `[x0, x1]`.
fn top(p: &Project, material: Material, x0: f32, x1: f32) -> f32 {
    build_scene(p)
        .meshes
        .iter()
        .filter(|m| m.material == material)
        .flat_map(|m| m.vertices.iter())
        .filter(|v| v.position[0] >= x0 && v.position[0] <= x1)
        .map(|v| v.position[1])
        .fold(f32::MIN, f32::max)
}

#[test]
fn a_room_ceiling_override_only_moves_that_rooms_ceiling() {
    let mut p = two_rooms();
    let default_top = top(&p, Material::Ceiling, 0.0, 240.0);
    let mut n = RoomName::new(Point::new(60.0, 60.0), "Great Room", "Living Room");
    n.ceiling_height = Some(144.0);
    p.floors[0].room_names.push(n);
    assert!(top(&p, Material::Ceiling, 0.0, 100.0) >= 144.0);
    assert!((top(&p, Material::Ceiling, 140.0, 240.0) - default_top).abs() < 0.01);
}

#[test]
fn a_floor_height_offset_raises_the_floor_and_the_ceiling() {
    let mut p = two_rooms();
    let floor_top = top(&p, Material::Floor, 140.0, 240.0);
    let mut n = RoomName::new(Point::new(180.0, 60.0), "Sunken", "Living Room");
    n.floor_height_offset = 6.0;
    p.floors[0].room_names.push(n);
    assert!((top(&p, Material::Floor, 140.0, 240.0) - (floor_top + 6.0)).abs() < 0.01);
    assert!((top(&p, Material::Floor, 0.0, 100.0) - floor_top).abs() < 0.01);
}
