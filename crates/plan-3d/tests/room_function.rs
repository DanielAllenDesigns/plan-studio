//! Room function reaches the 3D platforms (R-11, R-28..R-30, R-40, R-41): a
//! garage floor drops, a deck has no ceiling, an Open Below room has no floor
//! and opens the ceiling under it, a room's Floor Structure sets the platform
//! thickness, and a nested room cuts a hole in the room around it.

use plan_3d::{build_scene, Material, Mesh};
use plan_core::extras::{RoomMisc, StructureLayer};
use plan_core::rooms::{apply_function_defaults, function_defaults};
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

fn meshes(p: &Project, material: Material) -> Vec<Mesh> {
    build_scene(p)
        .meshes
        .into_iter()
        .filter(|m| m.material == material)
        .collect()
}

/// Vertex heights of the `material` meshes with plan x in `[x0, x1]`.
fn heights(p: &Project, material: Material, x0: f32, x1: f32) -> Vec<f32> {
    meshes(p, material)
        .iter()
        .flat_map(|m| m.vertices.iter())
        .filter(|v| v.position[0] >= x0 && v.position[0] <= x1)
        .map(|v| v.position[1])
        .collect()
}

fn top(p: &Project, material: Material, x0: f32, x1: f32) -> f32 {
    heights(p, material, x0, x1)
        .into_iter()
        .fold(f32::MIN, f32::max)
}

fn bottom(p: &Project, material: Material, x0: f32, x1: f32) -> f32 {
    heights(p, material, x0, x1)
        .into_iter()
        .fold(f32::MAX, f32::min)
}

/// Area of the upward-facing triangles whose heights lie in `[y0, y1]`.
fn top_area(ms: &[Mesh], y0: f32, y1: f32) -> f64 {
    let mut a = 0.0;
    for m in ms {
        for t in m.indices.chunks(3) {
            let v = |i: u32| m.vertices[i as usize];
            let (p, q, r) = (v(t[0]), v(t[1]), v(t[2]));
            if p.normal[1] < 0.5 || p.position[1] < y0 || p.position[1] > y1 {
                continue;
            }
            let (ux, uz) = (q.position[0] - p.position[0], q.position[2] - p.position[2]);
            let (vx, vz) = (r.position[0] - p.position[0], r.position[2] - p.position[2]);
            a += f64::from((ux * vz - uz * vx).abs()) * 0.5;
        }
    }
    a
}

fn named(function: &str, ty: &str, at: Point) -> RoomName {
    let mut n = RoomName::new(at, ty, ty);
    apply_function_defaults(&mut n, &function_defaults(function, ty), 0.75);
    n
}

#[test]
fn a_garage_floor_drops_24_inches_with_a_slab_and_no_finish() {
    let mut p = two_rooms();
    let house_floor = top(&p, Material::Floor, 140.0, 240.0);
    p.floors[0]
        .room_names
        .push(named("Garage", "Garage", Point::new(180.0, 60.0)));
    // The finish (0.75") goes and the slab top sits 24" under the house floor.
    let dropped = top(&p, Material::Floor, 140.0, 240.0);
    assert!(
        (dropped - (house_floor - 0.75 - 24.0)).abs() < 0.01,
        "{dropped}"
    );
    // A 4" concrete slab.
    let low = bottom(&p, Material::Floor, 140.0, 240.0);
    assert!((dropped - low - 4.0).abs() < 0.01);
    // The house side is untouched.
    assert!((top(&p, Material::Floor, 0.0, 100.0) - house_floor).abs() < 0.01);
}

#[test]
fn a_deck_has_no_ceiling_platform_but_keeps_its_floor() {
    let mut p = two_rooms();
    let ceiling_x = |p: &Project, x0, x1| heights(p, Material::Ceiling, x0, x1).len();
    assert!(ceiling_x(&p, 140.0, 240.0) > 0);
    p.floors[0]
        .room_names
        .push(named("Deck", "Deck", Point::new(180.0, 60.0)));
    assert_eq!(ceiling_x(&p, 140.0, 240.0), 0, "no ceiling over the deck");
    assert!(
        ceiling_x(&p, 0.0, 100.0) > 0,
        "the other room keeps its ceiling"
    );
    assert!(!heights(&p, Material::Floor, 140.0, 240.0).is_empty());
    // Porch likewise.
    let mut q = two_rooms();
    q.floors[0]
        .room_names
        .push(named("Porch", "Porch", Point::new(180.0, 60.0)));
    assert_eq!(heights(&q, Material::Ceiling, 140.0, 240.0).len(), 0);
}

#[test]
fn ceiling_over_this_room_off_removes_just_that_ceiling() {
    let mut p = two_rooms();
    let mut n = RoomName::new(Point::new(60.0, 60.0), "Room", "Standard");
    n.has_ceiling = false;
    p.floors[0].room_names.push(n);
    assert!(heights(&p, Material::Ceiling, 0.0, 100.0).is_empty());
    assert!(!heights(&p, Material::Ceiling, 140.0, 240.0).is_empty());
}

#[test]
fn open_below_removes_the_floor_and_opens_the_ceiling_under_it() {
    // Two floors, each a 240 x 120 box split into two rooms.
    let mut p = two_rooms();
    let up = p.build_new_floor(true);
    p.add_wall(
        up,
        Point::new(120.0, 0.0),
        Point::new(120.0, 120.0),
        4.5,
        109.0,
        WallKind::Interior,
    );
    let ceiling0 = |p: &Project| {
        // Floor 0's ceiling sits near 109"; floor 1's is far above it.
        top_area(&meshes(p, Material::Ceiling), 100.0, 130.0)
    };
    let floor1 = |p: &Project| {
        let e = p.floors[1].elevation as f32;
        top_area(&meshes(p, Material::Floor), e - 1.0, e + 2.0)
    };
    let (c0, f1) = (ceiling0(&p), floor1(&p));
    assert!((c0 - 240.0 * 120.0).abs() < 1.0, "{c0}");
    assert!((f1 - 240.0 * 120.0).abs() < 1.0, "{f1}");

    p.floors[up]
        .room_names
        .push(named("Open Below", "Open Below", Point::new(60.0, 60.0)));
    let f1_after = floor1(&p);
    assert!(
        (f1 - f1_after - 120.0 * 120.0).abs() < 1.0,
        "the floor platform loses the room: {f1} -> {f1_after}"
    );
    // The ceiling of the floor below opens under it too.
    let c0_after = ceiling0(&p);
    assert!(
        (c0 - c0_after - 120.0 * 120.0).abs() < 1.0,
        "the ceiling below loses the room: {c0} -> {c0_after}"
    );
}

#[test]
fn the_floor_structure_define_sets_the_platform_thickness() {
    let mut p = two_rooms();
    let default_top = top(&p, Material::Floor, 140.0, 240.0);
    let default_low = bottom(&p, Material::Floor, 140.0, 240.0);
    assert!(
        (default_top - default_low - 1.0).abs() < 0.01,
        "legacy 1\" slab"
    );
    let mut n = RoomName::new(Point::new(180.0, 60.0), "Hall", "Hall");
    n.misc = Some(RoomMisc {
        floor_finish_thickness: 0.75,
        floor_structure: vec![
            StructureLayer::new("Subfloor", 0.75),
            StructureLayer::new("Joist", 9.25),
        ],
        ceiling_structure: vec![StructureLayer::new("Drywall", 0.5)],
        ..RoomMisc::default()
    });
    p.floors[0].room_names.push(n);
    let top2 = top(&p, Material::Floor, 140.0, 240.0);
    let low2 = bottom(&p, Material::Floor, 140.0, 240.0);
    assert!(
        (top2 - default_top).abs() < 0.01,
        "the top stays at the finished floor"
    );
    assert!(
        (top2 - low2 - 10.0).abs() < 0.01,
        "10\" platform: {}",
        top2 - low2
    );
    let ceil_top = top(&p, Material::Ceiling, 140.0, 240.0);
    let ceil_low = bottom(&p, Material::Ceiling, 140.0, 240.0);
    assert!((ceil_top - ceil_low - 0.5).abs() < 0.01);
    // The other room keeps the default platform.
    let other = top(&p, Material::Floor, 0.0, 100.0) - bottom(&p, Material::Floor, 0.0, 100.0);
    assert!((other - 1.0).abs() < 0.01);
}

#[test]
fn a_nested_closet_is_a_hole_in_the_platform_around_it() {
    let mut p = Project::new("closet");
    let c = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 120.0),
        Point::new(0.0, 120.0),
    ];
    for i in 0..4 {
        p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
    }
    let k = [
        Point::new(60.0, 40.0),
        Point::new(108.0, 40.0),
        Point::new(108.0, 76.0),
        Point::new(60.0, 76.0),
    ];
    for i in 0..4 {
        p.add_wall(0, k[i], k[(i + 1) % 4], 4.5, 109.0, WallKind::Interior);
    }
    // Room and closet each get their own platform; together they cover the
    // box once, not the closet twice.
    let floor = top_area(&meshes(&p, Material::Floor), 0.0, 1.0);
    assert!((floor - 240.0 * 120.0).abs() < 1.0, "{floor}");
    let ceiling = top_area(&meshes(&p, Material::Ceiling), 100.0, 130.0);
    assert!((ceiling - 240.0 * 120.0).abs() < 1.0, "{ceiling}");
}
