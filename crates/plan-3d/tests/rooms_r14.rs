//! Round 14 rooms in 3D: the Open Below rule, rough ceiling and finish
//! thickness, basements and footings, monolithic slabs, room moldings and
//! per-surface materials (R-18, R-25, R-27, R-31, R-34, R-36, R-61, R-62).

use plan_3d::{build_scene, Material, Mesh};
use plan_core::extras::{MoldingKind, MoldingRef, RoomMisc};
use plan_core::floors::{FoundationKind, FoundationOptions, FoundationRooms};
use plan_core::rooms::{apply_function_defaults, function_defaults, RoomSlab};
use plan_core::{Opening, OpeningKind, Point, Project, RoomName, WallKind};

const CEIL: f64 = 108.0;

/// A box of 240" x 120" outside walls with a partition at `split`.
fn house_with_partition(floor: usize, p: &mut Project, split: f64) {
    let c = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 120.0),
        Point::new(0.0, 120.0),
    ];
    for i in 0..4 {
        p.add_wall(floor, c[i], c[(i + 1) % 4], 6.0, CEIL, WallKind::Exterior);
    }
    p.add_wall(
        floor,
        Point::new(split, 0.0),
        Point::new(split, 120.0),
        4.5,
        CEIL,
        WallKind::Interior,
    );
}

fn one_room() -> Project {
    let mut p = Project::new("room");
    p.floors[0].ceiling_height = CEIL;
    let c = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 120.0),
        Point::new(0.0, 120.0),
    ];
    for i in 0..4 {
        p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, CEIL, WallKind::Exterior);
    }
    p
}

fn meshes(p: &Project, material: Material) -> Vec<Mesh> {
    build_scene(p)
        .meshes
        .into_iter()
        .filter(|m| m.material == material)
        .collect()
}

fn ys(ms: &[Mesh]) -> Vec<f32> {
    ms.iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .collect()
}

fn top_of(ms: &[Mesh]) -> f32 {
    ys(ms).into_iter().fold(f32::MIN, f32::max)
}

fn bottom_of(ms: &[Mesh]) -> f32 {
    ys(ms).into_iter().fold(f32::MAX, f32::min)
}

fn xs_of(ms: &[Mesh]) -> Vec<f32> {
    ms.iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[0]))
        .collect()
}

fn named(function: &str, ty: &str, at: Point) -> RoomName {
    let mut n = RoomName::new(at, ty, ty);
    apply_function_defaults(&mut n, &function_defaults(function, ty), 0.75);
    n
}

// ---- Open Below ----

/// Two floors, each with a partition; the upper partition at `upper_split`
/// and its left room Open Below. The lower partition is at 120".
fn open_below_house(upper_split: f64) -> Project {
    let mut p = Project::new("open below");
    p.floors[0].ceiling_height = CEIL;
    house_with_partition(0, &mut p, 120.0);
    p.build_new_floor(false);
    house_with_partition(1, &mut p, upper_split);
    p.floors[1]
        .room_names
        .push(named("Open Below", "Open Below", Point::new(30.0, 60.0)));
    p
}

fn ceiling_of_floor0(p: &Project) -> Vec<Mesh> {
    meshes(p, Material::Ceiling)
        .into_iter()
        .filter(|m| ys(std::slice::from_ref(m)).iter().all(|y| *y < 150.0))
        .collect()
}

#[test]
fn an_open_below_room_opens_only_the_rooms_wholly_inside_it() {
    // The Open Below room is 0..180 wide; the room under 0..120 is wholly
    // inside it and has no ceiling, the room under 120..240 is only partly
    // under it and keeps its ceiling whole (no hole, so no vertex at x=180).
    let p = open_below_house(180.0);
    let ceil = ceiling_of_floor0(&p);
    let xs = xs_of(&ceil);
    assert!(!xs.is_empty(), "the partly covered room keeps its ceiling");
    assert!(
        xs.iter().all(|x| *x > 118.0),
        "nothing over the covered room: {:?}",
        xs.iter().cloned().fold(f32::MAX, f32::min)
    );
    assert!(
        !xs.iter().any(|x| (*x - 180.0).abs() < 1.0),
        "the partly covered ceiling is not cut at the Open Below edge"
    );
    // When the rooms line up, the one under it opens and the other stays.
    let q = open_below_house(120.0);
    let xs = xs_of(&ceiling_of_floor0(&q));
    assert!(!xs.is_empty() && xs.iter().all(|x| *x > 118.0));
}

#[test]
fn an_open_below_room_gets_no_ceiling_surface_plate() {
    // The lower left room names a wood-plank ceiling; under an Open Below
    // room the plate would float, so only the other room gets one.
    let mut p = open_below_house(120.0);
    let mut n = RoomName::new(Point::new(30.0, 60.0), "Den", "Den");
    n.ceiling_finish = Some("Wood Planks".into());
    p.floors[0].room_names.push(n);
    let plates = |p: &Project| {
        meshes(p, Material::Floor)
            .into_iter()
            .filter(|m| (bottom_of(std::slice::from_ref(m)) - (CEIL as f32 - 0.06)).abs() < 0.01)
            .count()
    };
    assert_eq!(plates(&p), 0, "no plate under the open room");
    // Without the Open Below room above, the plate is there.
    p.floors[1].room_names.clear();
    assert_eq!(plates(&p), 1);
}

#[test]
fn a_room_larger_than_the_open_below_room_keeps_its_ceiling() {
    // An Open Below room 0..60 over a lower room 0..120.
    let p = open_below_house(60.0);
    let xs = xs_of(&ceiling_of_floor0(&p));
    assert!(
        xs.iter().any(|x| *x < 10.0),
        "the left room has its ceiling"
    );
}

// ---- rough ceiling and finish thickness ----

#[test]
fn the_ceiling_finish_thickness_lifts_the_ceiling_platform() {
    let mut p = one_room();
    let base = top_of(&meshes(&p, Material::Ceiling));
    let mut n = RoomName::new(Point::new(60.0, 60.0), "Den", "Den");
    n.misc = Some(RoomMisc {
        ceiling_finish_thickness: 3.0,
        ..RoomMisc::default()
    });
    p.floors[0].room_names.push(n);
    let ceil = meshes(&p, Material::Ceiling);
    // The underside stays at the finished ceiling; the platform is 3" higher.
    assert!(
        (bottom_of(&ceil) - CEIL as f32).abs() < 0.01,
        "{}",
        bottom_of(&ceil)
    );
    assert!(
        (top_of(&ceil) - (base - 0.625 + 3.0)).abs() < 0.01,
        "{} vs {}",
        top_of(&ceil),
        base
    );
}

#[test]
fn a_rough_ceiling_puts_the_framing_at_its_height() {
    let mut p = one_room();
    let mut n = RoomName::new(Point::new(60.0, 60.0), "Den", "Den");
    n.rough_ceiling = Some(CEIL + 12.0);
    p.floors[0].room_names.push(n);
    let ceil = meshes(&p, Material::Ceiling);
    assert!((bottom_of(&ceil) - CEIL as f32).abs() < 0.01);
    // Rough height plus the 1" platform.
    assert!((top_of(&ceil) - (CEIL as f32 + 12.0 + 1.0)).abs() < 0.01);
    // A rough ceiling lower than the finish layers change nothing odd.
    let mut q = one_room();
    let mut n = RoomName::new(Point::new(60.0, 60.0), "Den", "Den");
    n.rough_ceiling = Some(CEIL - 5.0);
    q.floors[0].room_names.push(n);
    let ceil = meshes(&q, Material::Ceiling);
    assert!(top_of(&ceil) > bottom_of(&ceil));
}

// ---- basements, crawl spaces and foundations ----

fn house_on_foundation(height: f64, rooms: FoundationRooms) -> Project {
    let mut p = one_room();
    let mut o = FoundationOptions::new(FoundationKind::StemWall { height });
    o.rooms = rooms;
    p.build_foundation_with(&o);
    p
}

#[test]
fn a_basement_has_a_slab_and_takes_its_ceiling_from_the_first_floor_platform() {
    let p = house_on_foundation(108.0, FoundationRooms::Auto);
    let platform = p.floors[1].settings.floor_structure_thickness;
    assert_eq!(p.floors[0].room_names[0].room_type, "Basement");
    // The first floor's finished level is 0; the slab sits on the ground.
    let floor = meshes(&p, Material::Floor)
        .into_iter()
        .filter(|m| ys(std::slice::from_ref(m)).iter().all(|y| *y < -50.0))
        .collect::<Vec<_>>();
    assert!(!floor.is_empty(), "the basement slab");
    assert!(
        (bottom_of(&floor) + 108.0).abs() < 0.01,
        "{}",
        bottom_of(&floor)
    );
    assert!((top_of(&floor) + 108.0 - 4.0).abs() < 0.01);
    // The ceiling hangs from the underside of the first floor's platform and
    // stops under its 1" slab.
    let ceil = meshes(&p, Material::Ceiling)
        .into_iter()
        .filter(|m| ys(std::slice::from_ref(m)).iter().all(|y| *y < 0.5))
        .collect::<Vec<_>>();
    assert!(!ceil.is_empty());
    assert!(
        (bottom_of(&ceil) + platform as f32).abs() < 0.01,
        "{}",
        bottom_of(&ceil)
    );
    assert!((top_of(&ceil) + 1.0).abs() < 0.01, "{}", top_of(&ceil));
}

#[test]
fn a_crawl_space_has_no_slab_and_no_ceiling_of_its_own() {
    let p = house_on_foundation(36.0, FoundationRooms::Auto);
    assert_eq!(p.floors[0].room_names[0].room_type, "Crawl Space");
    let low_floor = meshes(&p, Material::Floor)
        .into_iter()
        .filter(|m| ys(std::slice::from_ref(m)).iter().all(|y| *y < -20.0))
        .count();
    assert_eq!(low_floor, 0);
    let low_ceiling = meshes(&p, Material::Ceiling)
        .into_iter()
        .filter(|m| ys(std::slice::from_ref(m)).iter().all(|y| *y < 0.5))
        .count();
    assert_eq!(low_ceiling, 0);
}

#[test]
fn foundation_walls_stand_on_footings_of_the_chosen_size() {
    let mut p = one_room();
    let mut o = FoundationOptions::new(FoundationKind::StemWall { height: 36.0 });
    o.footing_width = 20.0;
    o.footing_depth = 10.0;
    o.rooms = FoundationRooms::None;
    p.build_foundation_with(&o);
    let concrete = meshes(&p, Material::Concrete);
    assert!(
        (bottom_of(&concrete) + 36.0 + 10.0).abs() < 0.01,
        "footings reach 10\" under the walls: {}",
        bottom_of(&concrete)
    );
    // A 20" footing under 8" walls sticks out 6" each side.
    let min_x = xs_of(&concrete).into_iter().fold(f32::MAX, f32::min);
    assert!((min_x + 10.0).abs() < 0.5, "{min_x}");
    // No footing, no extra concrete.
    o.footing_width = 0.0;
    p.build_foundation_with(&o);
    let concrete = meshes(&p, Material::Concrete);
    assert!(bottom_of(&concrete) >= -36.0 - 0.01);
}

#[test]
fn a_monolithic_slab_room_gets_a_thick_floor_and_a_thickened_edge() {
    let mut p = one_room();
    let base_floor = top_of(&meshes(&p, Material::Floor));
    let mut n = RoomName::new(Point::new(60.0, 60.0), "Den", "Den");
    n.monolithic_slab = Some(RoomSlab {
        thickness: 6.0,
        stem_height: 14.0,
    });
    p.floors[0].room_names.push(n);
    let floor = meshes(&p, Material::Floor);
    assert!((top_of(&floor) - base_floor).abs() < 0.01);
    assert!((top_of(&floor) - bottom_of(&floor) - 6.0).abs() < 0.01);
    let concrete = meshes(&p, Material::Concrete);
    assert!(!concrete.is_empty(), "the thickened edge");
    assert!(
        (bottom_of(&concrete) + 14.0).abs() < 0.01,
        "{}",
        bottom_of(&concrete)
    );
}

// ---- moldings ----

fn with_moldings(p: &mut Project, list: &[(MoldingKind, &str, f64)]) {
    let mut n = RoomName::new(Point::new(60.0, 60.0), "Den", "Den");
    for (kind, profile, height) in list {
        n.moldings.push(MoldingRef {
            kind: *kind,
            profile: (*profile).to_string(),
            height: *height,
        });
    }
    p.floors[0].room_names.push(n);
}

#[test]
fn base_crown_and_chair_rail_are_built_around_the_room() {
    let mut p = one_room();
    let none = meshes(&p, Material::Trim).len();
    with_moldings(
        &mut p,
        &[
            (MoldingKind::Base, "Base - Colonial 5 1/4", 5.25),
            (MoldingKind::Crown, "Crown - Cove 3 5/8", 3.625),
            (MoldingKind::Chair, "Chair Rail - Simple 2 1/2", 2.5),
        ],
    );
    let trim = meshes(&p, Material::Trim);
    assert_eq!(trim.len(), none + 3, "one closed run each");
    let floor_top = 0.75_f32;
    let bands: Vec<(f32, f32)> = trim
        .iter()
        .map(|m| {
            let y = ys(std::slice::from_ref(m));
            (
                y.iter().cloned().fold(f32::MAX, f32::min),
                y.iter().cloned().fold(f32::MIN, f32::max),
            )
        })
        .collect();
    // Base: from the finished floor up 5 1/4".
    assert!(bands
        .iter()
        .any(|b| (b.0 - floor_top).abs() < 0.01 && (b.1 - floor_top - 5.25).abs() < 0.01));
    // Crown: hangs from the finished ceiling.
    assert!(bands
        .iter()
        .any(|b| (b.1 - CEIL as f32).abs() < 0.02 && b.1 - b.0 > 3.0));
    // Chair rail: 32" above the finished floor.
    assert!(bands
        .iter()
        .any(|b| (b.0 - floor_top - 32.0).abs() < 0.01 && (b.1 - b.0 - 2.5).abs() < 0.01));
}

#[test]
fn the_base_stops_at_a_door_and_the_crown_does_not() {
    let mut p = one_room();
    let wall = p.floors[0].walls[0].id;
    p.floors[0].openings.push(Opening::new(
        wall,
        100.0,
        OpeningKind::Door,
        36.0,
        80.0,
        0.0,
    ));
    with_moldings(
        &mut p,
        &[
            (MoldingKind::Base, "Base - Square 3 1/4", 3.25),
            (MoldingKind::Crown, "Crown - Stepped 6", 6.0),
        ],
    );
    let trim = meshes(&p, Material::Trim);
    let by_height = |lo: f32, hi: f32| {
        trim.iter()
            .filter(|m| {
                let y = ys(std::slice::from_ref(m));
                y.iter().cloned().fold(f32::MIN, f32::max) > lo
                    && y.iter().cloned().fold(f32::MAX, f32::min) < hi
            })
            .collect::<Vec<_>>()
    };
    let base = by_height(0.0, 5.0);
    assert_eq!(base.len(), 1);
    // The gap: no base vertex between the jambs (x 82..118 on the inside).
    let gap = base[0]
        .vertices
        .iter()
        .filter(|v| v.position[0] > 83.0 && v.position[0] < 117.0 && v.position[2].abs() < 6.0)
        .count();
    assert_eq!(gap, 0, "the base is open across the door");
    let crown = by_height(CEIL as f32 - 7.0, CEIL as f32 + 1.0);
    assert_eq!(crown.len(), 1);
    // The base ends at the jambs (x 82 and 118); the crown runs unbroken
    // from corner to corner, with no vertex at the door.
    let near = |m: &Mesh, x: f32| m.vertices.iter().any(|v| (v.position[0] - x).abs() < 0.5);
    assert!(near(base[0], 82.0) && near(base[0], 118.0));
    assert!(!near(crown[0], 82.0) && !near(crown[0], 118.0));
}

#[test]
fn an_unknown_molding_name_falls_back_to_the_kind_s_first_profile() {
    let mut p = one_room();
    let none = meshes(&p, Material::Trim).len();
    with_moldings(&mut p, &[(MoldingKind::Base, "Customer's Own", 4.0)]);
    assert_eq!(meshes(&p, Material::Trim).len(), none + 1);
}

// ---- surfaces ----

#[test]
fn a_rooms_surface_materials_are_laid_on_its_floor_ceiling_and_walls() {
    let mut p = one_room();
    assert!(meshes(&p, Material::Stone).is_empty());
    assert!(meshes(&p, Material::Brick).is_empty());
    let mut n = RoomName::new(Point::new(60.0, 60.0), "Den", "Den");
    n.floor_finish = Some("Ceramic Tile".into());
    n.ceiling_finish = Some("Wood Planks".into());
    n.misc = Some(RoomMisc {
        wall_covering: "Brick".into(),
        floor_finish_thickness: 0.75,
        ..RoomMisc::default()
    });
    p.floors[0].room_names.push(n);
    let tile = meshes(&p, Material::Stone);
    assert_eq!(tile.len(), 1);
    assert!((top_of(&tile) - (0.75 + 0.06)).abs() < 0.01);
    // The wood-plank ceiling is a Floor-material plate under the ceiling.
    let wood = meshes(&p, Material::Floor)
        .into_iter()
        .filter(|m| (bottom_of(std::slice::from_ref(m)) - (CEIL as f32 - 0.06)).abs() < 0.01)
        .count();
    assert_eq!(wood, 1);
    let brick = meshes(&p, Material::Brick);
    assert_eq!(brick.len(), 1);
    // Four walls, no openings: eight triangles each side... at least four quads.
    assert!(brick[0].indices.len() >= 4 * 6);
    // Wall panels leave the door open.
    let wall = p.floors[0].walls[0].id;
    p.floors[0].openings.push(Opening::new(
        wall,
        100.0,
        OpeningKind::Door,
        36.0,
        80.0,
        0.0,
    ));
    let brick = meshes(&p, Material::Brick);
    let below_head = brick[0]
        .vertices
        .iter()
        .filter(|v| v.position[0] > 83.0 && v.position[0] < 117.0 && v.position[1] < 70.0)
        .count();
    assert_eq!(below_head, 0, "no brick across the doorway");
}

#[test]
fn the_floors_default_materials_cover_rooms_without_their_own() {
    let mut p = one_room();
    p.floors[0].settings.wall_material = "Stucco".into();
    p.floors[0].settings.floor_material = "Marble".into();
    p.floors[0]
        .room_names
        .push(RoomName::new(Point::new(60.0, 60.0), "Den", "Den"));
    assert_eq!(meshes(&p, Material::Stucco).len(), 1);
    assert_eq!(meshes(&p, Material::Stone).len(), 1);
    // A room's own name wins over the floor's default.
    p.floors[0].room_names[0].floor_finish = Some("Brick".into());
    assert!(meshes(&p, Material::Stone).is_empty());
    assert_eq!(meshes(&p, Material::Brick).len(), 1);
}
