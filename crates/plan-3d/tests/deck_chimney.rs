//! Decks, chimneys and split-level floors reach the whole scene (CB-86,
//! CB-87, R-86): a deck draws boards in place of its platform slab, a
//! chimney chase cuts the platforms of the floors above, a fireplace built
//! into a wall cuts the wall, and a riser closes a split-level step.

use plan_3d::{build_scene, Material, Mesh, Scene};
use plan_core::deck::DeckSpec;
use plan_core::fireplace::FireplaceKind;
use plan_core::{Floor, Point, Project, RoomName, WallClass, WallKind};

fn tris(scene: &Scene, material: Material) -> usize {
    scene
        .meshes
        .iter()
        .filter(|m| m.material == material)
        .map(Mesh::triangle_count)
        .sum()
}

/// A 20 x 12 ft house of four exterior walls.
fn house() -> Project {
    let mut p = Project::new("house");
    let c = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 144.0),
        Point::new(0.0, 144.0),
    ];
    for i in 0..4 {
        p.add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.0, WallKind::Exterior);
    }
    p
}

#[test]
fn a_deck_draws_boards_instead_of_its_platform_slab() {
    let mut p = Project::new("deck");
    let c = [
        Point::new(0.0, 0.0),
        Point::new(144.0, 0.0),
        Point::new(144.0, 96.0),
        Point::new(0.0, 96.0),
    ];
    for i in 0..4 {
        let id = p.add_wall(0, c[i], c[(i + 1) % 4], 1.5, 36.0, WallKind::Exterior);
        let w = p.floors[0].wall_mut(id).unwrap();
        w.class = WallClass::DeckEdge;
        w.is_deck_edge = true;
    }
    let mut name = RoomName::new(Point::new(72.0, 48.0), "Deck", "Deck");
    name.has_ceiling = false;
    p.floors[0].room_names.push(name);
    let slab = tris(&build_scene(&p), Material::Floor);
    assert!(slab > 0 && slab <= 12, "a platform slab: {slab}");
    p.floors[0].room_names[0].deck = Some(DeckSpec::default());
    let boards = tris(&build_scene(&p), Material::Floor);
    // 17 boards of 12 triangles instead of the slab.
    assert!(boards > 150, "{boards}");
    // Boards switched off bring the slab back.
    p.floors[0].room_names[0]
        .deck
        .as_mut()
        .unwrap()
        .planking
        .enabled = false;
    assert_eq!(tris(&build_scene(&p), Material::Floor), slab);
}

#[test]
fn a_chase_cuts_the_floor_platform_above_and_a_wall_fireplace_cuts_its_wall() {
    let mut p = house();
    let mut upper = Floor::new("2nd Floor", 120.0);
    upper.walls = p.floors[0].walls.clone();
    p.floors.push(upper);
    let id = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(120.0, 60.0), 0.0);
    let with_chase = build_scene(&p);
    let mut fp = p.floors[0].fireplace_of(p.floors[0].symbol(id).unwrap());
    fp.chimney.chase_through_floors = false;
    p.floors[0].set_fireplace(fp);
    let without = build_scene(&p);
    assert!(
        tris(&with_chase, Material::Floor) > tris(&without, Material::Floor),
        "the 2nd floor's platform has a hole"
    );

    // The same fireplace built into the bottom wall: the wall is cut.
    let wall_id = p.floors[0].walls[0].id;
    let wall_tris = |scene: &Scene| -> usize {
        scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(wall_id))
            .map(Mesh::triangle_count)
            .sum()
    };
    let beside = wall_tris(&build_scene(&p));
    let mut fp = p.floors[0].fireplace_of(p.floors[0].symbol(id).unwrap());
    fp.in_wall = true;
    p.floors[0]
        .symbols
        .iter_mut()
        .find(|s| s.id == id)
        .unwrap()
        .position = Point::new(120.0, -10.0);
    p.floors[0].set_fireplace(fp);
    let in_wall = wall_tris(&build_scene(&p));
    assert_ne!(
        beside, in_wall,
        "the wall mesh changed where the body stands"
    );
}

#[test]
fn a_chimney_is_cut_out_of_nothing_and_always_tagged_with_its_symbol() {
    let mut p = house();
    let id = p.add_fireplace(0, FireplaceKind::Prefab, Point::new(120.0, 60.0), 0.0);
    let scene = build_scene(&p);
    let parts: Vec<&Mesh> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .collect();
    assert!(!parts.is_empty());
    // A prefab: drywall chase, siding shaft, a metal rain cap.
    let mats: Vec<Material> = parts.iter().map(|m| m.material).collect();
    assert!(mats.contains(&Material::Metal));
    assert!(mats.contains(&Material::Siding) || mats.contains(&Material::WallInterior));
}

#[test]
fn a_raised_room_beside_a_room_divider_gets_a_riser() {
    let mut p = house();
    let divider = p.add_wall(
        0,
        Point::new(120.0, 0.0),
        Point::new(120.0, 144.0),
        4.5,
        109.0,
        WallKind::Interior,
    );
    {
        let w = p.floors[0].wall_mut(divider).unwrap();
        w.class = WallClass::RoomDivider;
        w.flags.room_divider = true;
    }
    p.floors[0]
        .room_names
        .push(RoomName::new(Point::new(60.0, 72.0), "Low", "Living"));
    let mut high = RoomName::new(Point::new(180.0, 72.0), "High", "Living");
    high.floor_height_offset = 24.0;
    p.floors[0].room_names.push(high);
    let raised = tris(&build_scene(&p), Material::Floor);
    p.floors[0].room_names[1].floor_height_offset = 0.0;
    let level = tris(&build_scene(&p), Material::Floor);
    // A riser is 12 triangles of floor material.
    assert_eq!(raised, level + 12);
}
