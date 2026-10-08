//! Holes in the floor and ceiling platforms cut the room slabs of `build_scene`.

use plan_3d::{build_scene, Material, Mesh};
use plan_core::foundation::{rect_outline, FoundationLayer, PlatformHole, PlatformKind};
use plan_core::{Point, Project, WallKind};

fn room() -> Project {
    let mut p = Project::new("room");
    let (w, h) = (240.0, 120.0);
    let c = [
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ];
    for i in 0..4 {
        p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
    }
    p
}

/// Area of the upward-facing triangles of every mesh of `material`.
fn top_area(meshes: &[Mesh], material: Material) -> f64 {
    let mut a = 0.0;
    for m in meshes.iter().filter(|m| m.material == material) {
        for t in m.indices.chunks(3) {
            let v = |i: u32| m.vertices[i as usize];
            let (p, q, r) = (v(t[0]), v(t[1]), v(t[2]));
            if p.normal[1] < 0.5 {
                continue;
            }
            let (ux, uz) = (q.position[0] - p.position[0], q.position[2] - p.position[2]);
            let (vx, vz) = (r.position[0] - p.position[0], r.position[2] - p.position[2]);
            a += f64::from((ux * vz - uz * vx).abs()) * 0.5;
        }
    }
    a
}

#[test]
fn platform_holes_cut_the_floor_and_ceiling_slabs() {
    let mut p = room();
    let before = build_scene(&p);
    let (floor0, ceil0) = (
        top_area(&before.meshes, Material::Floor),
        top_area(&before.meshes, Material::Ceiling),
    );
    assert!((floor0 - 240.0 * 120.0).abs() < 1.0, "{floor0}");

    let mut layer = FoundationLayer::default();
    layer.platform_holes.push(PlatformHole::new(
        900,
        rect_outline(Point::new(60.0, 30.0), Point::new(100.0, 70.0)),
        PlatformKind::Floor,
    ));
    layer.platform_holes.push(PlatformHole::new(
        901,
        rect_outline(Point::new(120.0, 30.0), Point::new(150.0, 60.0)),
        PlatformKind::Ceiling,
    ));
    layer.store(&mut p.floors[0]);
    let after = build_scene(&p);
    let floor1 = top_area(&after.meshes, Material::Floor);
    let ceil1 = top_area(&after.meshes, Material::Ceiling);
    assert!(
        (floor0 - floor1 - 40.0 * 40.0).abs() < 1.0,
        "{floor0} {floor1}"
    );
    assert!((ceil0 - ceil1 - 30.0 * 30.0).abs() < 1.0, "{ceil0} {ceil1}");
}
