//! Floors, foundations and fireplace foundations through the public API
//! (Round 16 brief 17: Build New Floor options, Foundation Defaults and
//! build behavior, fireplace bases; manual pp. 737-773).

use plan_core::floors::DeriveFrom;
use plan_core::floors::{
    FloorPlacement, FoundationKind, FoundationOptions, FoundationRooms, NewFloorOptions,
    FLOOR_PLATFORM_THICKNESS, MAX_LIVING_FLOORS,
};
use plan_core::foundation::{
    foundation_takeoff, pier_positions, step_markers, FoundationLayer, PierShape, STEP_TOLERANCE,
};
use plan_core::{FloorKind, OpeningKind, OpeningStyle, Point, Project, RoomName, WallKind};

const WALL_H: f64 = 109.125;

/// Two 120 x 120 in rooms side by side with a partition between them; the
/// exterior walls are split at the partition so each belongs to one room.
fn two_rooms() -> Project {
    let mut p = Project::new("two");
    let ring = [
        ((0.0, 0.0), (120.0, 0.0)),
        ((120.0, 0.0), (240.0, 0.0)),
        ((240.0, 0.0), (240.0, 120.0)),
        ((240.0, 120.0), (120.0, 120.0)),
        ((120.0, 120.0), (0.0, 120.0)),
        ((0.0, 120.0), (0.0, 0.0)),
    ];
    for (a, b) in ring {
        p.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            6.5,
            WALL_H,
            WallKind::Exterior,
        );
    }
    p.add_wall(
        0,
        Point::new(120.0, 0.0),
        Point::new(120.0, 120.0),
        4.5,
        WALL_H,
        WallKind::Interior,
    );
    p
}

fn name(p: &mut Project, at: (f64, f64), ty: &str) -> usize {
    p.floors[0]
        .room_names
        .push(RoomName::new(Point::new(at.0, at.1), ty, ty));
    p.floors[0].room_names.len() - 1
}

const LEFT: (f64, f64) = (60.0, 60.0);
const RIGHT: (f64, f64) = (180.0, 60.0);

fn stem(h: f64) -> FoundationOptions {
    FoundationOptions::new(FoundationKind::StemWall { height: h })
}

// ----- Build New Floor options -----

#[test]
fn stepping_a_new_floor_keeps_the_ceilings_of_the_floor_below() {
    let mut p = two_rooms();
    let l = name(&mut p, LEFT, "Den");
    name(&mut p, RIGHT, "Bedroom");
    // A vaulted den: 24 in more ceiling.
    p.floors[0].room_names[l].ceiling_height = Some(WALL_H + 24.0);
    let mut o = NewFloorOptions {
        derive: DeriveFrom::AllWalls,
        step_elevations: true,
        ..NewFloorOptions::default()
    };
    let up = p.build_new_floor_with(&o).unwrap();
    let f = &p.floors[up];
    assert_eq!(
        f.room_names.len(),
        1,
        "only the stepped room needs an entry"
    );
    assert_eq!(f.room_names[0].floor_height_offset, 24.0);
    assert!(f.room_names[0].anchor.x < 120.0, "over the den");
    // Without the option nothing is stepped.
    o.step_elevations = false;
    let up = p.build_new_floor_with(&o).unwrap();
    assert!(p.floors[up].room_names.is_empty());
}

#[test]
fn inserting_below_steps_the_new_ceilings_to_the_floors_above() {
    let mut p = two_rooms();
    let r = name(&mut p, RIGHT, "Den");
    p.floors[0].room_names[r].floor_height_offset = 24.0;
    let o = NewFloorOptions {
        source: Some(0),
        place: FloorPlacement::Below,
        derive: DeriveFrom::AllWalls,
        step_elevations: true,
        ..NewFloorOptions::default()
    };
    let at = p.build_new_floor_with(&o).unwrap();
    assert_eq!(at, 0);
    assert_eq!(p.floors.len(), 2);
    // The new floor's walls come from the old floor's, and its right room
    // reaches up to the raised floor above.
    assert_eq!(p.floors[0].walls.len(), 7);
    let n = &p.floors[0].room_names[0];
    assert_eq!(n.ceiling_height, Some(p.floors[0].ceiling_height + 24.0));
    assert_eq!(p.floors[0].name, "1st Floor");
    assert_eq!(p.floors[1].name, "2nd Floor");
}

#[test]
fn a_plan_has_at_most_thirty_living_floors() {
    let mut p = two_rooms();
    let o = NewFloorOptions::default();
    for _ in 1..MAX_LIVING_FLOORS {
        assert!(p.build_new_floor_with(&o).is_some());
    }
    assert_eq!(p.living_floor_count(), MAX_LIVING_FLOORS);
    assert!(p.new_floor_blocker(&o).unwrap().contains("30"));
    assert!(p.build_new_floor_with(&o).is_none());
    assert_eq!(p.floors.len(), MAX_LIVING_FLOORS);
    // A foundation or an attic is not a living floor.
    p.build_foundation(FoundationKind::MonolithicSlab);
    assert_eq!(p.living_floor_count(), MAX_LIVING_FLOORS);
}

fn plane(y: f64) -> serde_json::Value {
    serde_json::json!({
        "kind": "plane",
        "id": 900,
        "polygon3d": [[0.0, y, 0.0], [240.0, y, 0.0], [120.0, y + 40.0, -60.0]],
    })
}

#[test]
fn the_highest_floors_roof_moves_up_with_a_new_floor() {
    let mut p = two_rooms();
    p.floors[0].roofs.push(plane(110.0));
    p.floors[0]
        .roofs
        .push(serde_json::json!({"kind": "settings"}));
    let o = NewFloorOptions {
        move_roof_up: true,
        ..NewFloorOptions::default()
    };
    let up = p.build_new_floor_with(&o).unwrap();
    let rise = p.floors[up].elevation - p.floors[0].elevation;
    assert!(rise > 100.0);
    assert!(p.floors[0].roofs.is_empty(), "the roof left the old floor");
    let y = p.floors[up].roofs[0]["polygon3d"][0][1].as_f64().unwrap();
    assert!(
        (y - (110.0 + rise)).abs() < 1e-9,
        "raised by what the stack grew"
    );
    // Off, the roof stays where it is.
    let mut q = two_rooms();
    q.floors[0].roofs.push(plane(110.0));
    let up = q.build_new_floor_with(&NewFloorOptions::default()).unwrap();
    assert_eq!(q.floors[0].roofs.len(), 1);
    assert!(q.floors[up].roofs.is_empty());
}

#[test]
fn a_floor_inserted_below_lifts_the_roof_of_the_floors_above() {
    let mut p = two_rooms();
    p.floors[0].roofs.push(plane(110.0));
    let o = NewFloorOptions {
        source: Some(0),
        place: FloorPlacement::Below,
        move_roof_up: true,
        ..NewFloorOptions::default()
    };
    p.build_new_floor_with(&o).unwrap();
    // The old first floor is the second now and keeps its roof, higher.
    let y = p.floors[1].roofs[0]["polygon3d"][0][1].as_f64().unwrap();
    assert!(y > 110.0 + 100.0, "{y}");
    assert!(p.floors[0].roofs.is_empty());
}

#[test]
fn floor_zero_stays_while_auto_rebuild_foundation_is_on() {
    let mut p = two_rooms();
    let mut o = stem(48.0);
    p.build_foundation_with(&o);
    assert!(!p.foundation_auto_rebuild());
    o.settings.auto_rebuild = true;
    p.build_foundation_with(&o);
    assert!(p.foundation_auto_rebuild());
    assert!(!p.delete_floor(0), "Floor 0 cannot be deleted");
    assert_eq!(p.floors.len(), 2);
    o.settings.auto_rebuild = false;
    p.build_foundation_with(&o);
    assert!(p.delete_floor(0));
    assert_eq!(p.floors[0].kind, FloorKind::Normal);
}

#[test]
fn drawing_on_the_attic_floor_warns_and_it_gets_no_rooms() {
    let mut p = two_rooms();
    let a = p.build_attic_floor().unwrap();
    assert!(p.attic_floor_warning(a).is_none());
    assert!(p.floors[a].room_names.is_empty());
    // A user-drawn wall on the attic floor: the warning shows, and
    // refreshing the attic still leaves rooms out.
    p.add_wall(
        a,
        Point::new(10.0, 10.0),
        Point::new(100.0, 10.0),
        4.5,
        96.0,
        WallKind::Interior,
    );
    assert!(p
        .attic_floor_warning(a)
        .unwrap()
        .contains("not meant to be a living area"));
    p.floors[a]
        .room_names
        .push(RoomName::new(Point::new(5.0, 5.0), "Loft", "Bedroom"));
    p.refresh_attic_floor();
    assert!(p.floors[a].room_names.is_empty());
    assert!(p.attic_floor_warning(0).is_none(), "only the attic warns");
}

// ----- Foundation build behavior -----

#[test]
fn stem_wall_height_runs_from_the_footing_to_the_underside_of_the_platform() {
    let mut p = two_rooms();
    p.build_foundation_with(&stem(48.0));
    let f = &p.floors[0];
    // 48 in less the 10 1/4 in platform that bears on the wall tops.
    for w in f.walls.iter().filter(|w| w.flags.foundation) {
        assert_eq!(w.height, 48.0 - FLOOR_PLATFORM_THICKNESS);
        assert_eq!(w.bottom_offset, 0.0);
    }
    assert_eq!(f.elevation, -48.0);
    // Hang 1st Floor Platform Inside Foundation Walls: up to the top of it.
    let mut o = stem(48.0);
    o.settings.hang_platform = true;
    p.build_foundation_with(&o);
    assert!(p.floors[0]
        .walls
        .iter()
        .filter(|w| w.flags.foundation)
        .all(|w| w.height == 48.0));
    // A lowered room is never left with a stem wall shorter than the Minimum
    // Height: the footing there goes deeper and Floor 0 with it.
    let mut q = two_rooms();
    let r = name(&mut q, RIGHT, "Den");
    name(&mut q, LEFT, "Den");
    q.floors[0].room_names[r].floor_height_offset = -20.0;
    let mut o = stem(36.0);
    o.settings.min_height = 12.0;
    q.build_foundation_with(&o);
    let f = &q.floors[0];
    let sunk = f
        .walls
        .iter()
        .find(|w| w.start.x == 240.0 && w.end.x == 240.0)
        .unwrap();
    assert_eq!(sunk.height, 12.0);
    assert_eq!(f.elevation, -(20.0 + FLOOR_PLATFORM_THICKNESS + 12.0));
}

#[test]
fn rooms_at_different_floor_heights_get_a_stepped_foundation_with_s_markers() {
    let mut p = two_rooms();
    name(&mut p, LEFT, "Den");
    let r = name(&mut p, RIGHT, "Den");
    p.floors[0].room_names[r].floor_height_offset = 24.0;
    p.build_foundation_with(&stem(36.0));
    let f = &p.floors[0];
    // The partition between the two floor heights has a foundation.
    assert_eq!(f.walls.iter().filter(|w| w.flags.foundation).count(), 7);
    let marks = step_markers(f);
    assert_eq!(
        marks.len(),
        2,
        "one S at each end of the partition: {marks:?}"
    );
    for m in &marks {
        assert!((m.high - m.low - 24.0).abs() < 1e-9);
        assert!(m.at.x == 120.0 && (m.at.y == 0.0 || m.at.y == 120.0));
    }
    // The raised room's walls are taller; the footings stay level.
    let tall = f
        .walls
        .iter()
        .find(|w| w.start.x == 240.0 && w.end.x == 240.0)
        .unwrap();
    assert_eq!(tall.height, 36.0 - FLOOR_PLATFORM_THICKNESS + 24.0);
    assert_eq!(tall.bottom_offset, 0.0);
    // A level plan has no steps; so does one within 1/16 in.
    let mut q = two_rooms();
    q.build_foundation_with(&stem(36.0));
    assert!(step_markers(&q.floors[0]).is_empty());
    assert!(STEP_TOLERANCE < 0.07);
}

#[test]
fn a_room_stem_wall_height_overrides_the_default() {
    let mut p = two_rooms();
    let r = name(&mut p, RIGHT, "Den");
    p.floors[0].room_names[r].stem_wall_height = Some(60.0);
    p.build_foundation_with(&stem(36.0));
    let f = &p.floors[0];
    let right = f
        .walls
        .iter()
        .find(|w| w.start.x == 240.0 && w.end.x == 240.0)
        .unwrap();
    assert_eq!(right.height, 60.0);
    // It goes deeper than the rest, so Floor 0 does too.
    assert!(f.elevation <= -(60.0 + FLOOR_PLATFORM_THICKNESS) + 1e-9);
}

#[test]
fn interior_walls_get_a_footing_only_when_they_ask_or_bear() {
    let mut p = two_rooms();
    name(&mut p, LEFT, "Den");
    name(&mut p, RIGHT, "Den");
    p.build_foundation_with(&stem(36.0));
    assert_eq!(p.floors[0].walls.len(), 6, "exterior walls only");
    let part = p.floors[1]
        .walls
        .iter()
        .find(|w| w.kind == WallKind::Interior)
        .unwrap()
        .id;
    p.floors[1]
        .wall_mut(part)
        .unwrap()
        .spec
        .foundation
        .create_below = true;
    p.build_foundation_with(&stem(36.0));
    assert_eq!(p.floors[0].walls.len(), 7, "Create Wall/Footing Below");
    p.floors[1]
        .wall_mut(part)
        .unwrap()
        .spec
        .foundation
        .create_below = false;
    p.floors[1]
        .wall_mut(part)
        .unwrap()
        .spec
        .structure
        .bearing_wall = true;
    p.build_foundation_with(&stem(36.0));
    assert_eq!(p.floors[0].walls.len(), 7, "a bearing wall");
    // A room that builds no foundation below leaves its exterior walls out.
    p.floors[1]
        .wall_mut(part)
        .unwrap()
        .spec
        .structure
        .bearing_wall = false;
    p.floors[1].room_names[1].options.build_foundation_below = false;
    p.build_foundation_with(&stem(36.0));
    assert_eq!(p.floors[0].walls.len(), 3, "only the left room's walls");
}

fn garage_house() -> (Project, u64) {
    let mut p = two_rooms();
    name(&mut p, LEFT, "Den");
    name(&mut p, RIGHT, "Garage");
    // A 9 ft overhead door in the right room's front wall.
    let wall = p.floors[0]
        .walls
        .iter()
        .find(|w| w.start.x == 120.0 && w.end.x == 240.0 && w.start.y == 0.0)
        .unwrap()
        .id;
    let door = p.add_opening(0, wall, 60.0, OpeningKind::Door).unwrap();
    let o = p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == door)
        .unwrap();
    o.style = OpeningStyle::Garage;
    o.width = 108.0;
    o.extras.spec.rough.add_width = 4.0;
    o.extras.spec.rough.concrete_each_side = 6.0;
    (p, door)
}

#[test]
fn a_garage_gets_a_lowered_slab_stem_walls_and_a_cutout_the_size_of_its_door() {
    let (mut p, _) = garage_house();
    p.build_foundation_with(&stem(48.0));
    let drop = FLOOR_PLATFORM_THICKNESS + 12.0;
    // The garage floor was lowered when the foundation was built, and it
    // takes its floor from the foundation.
    let g = &p.floors[1].room_names[1];
    assert_eq!(g.floor_height_offset, -drop);
    assert!(g.options.floor_from_foundation);
    assert!(!p.floors[1].room_names[0].options.floor_from_foundation);
    let f = &p.floors[0];
    let layer = FoundationLayer::load(f);
    assert_eq!(layer.slabs.len(), 1);
    assert!((layer.slabs[0].top_elevation - (48.0 - drop)).abs() < 1e-9);
    // The partition between garage and house has a stem wall, and the room
    // under the garage supplies the floor above.
    assert_eq!(f.walls.iter().filter(|w| w.flags.foundation).count(), 7);
    let slab_room = f.room_names.iter().find(|n| n.room_type == "Slab").unwrap();
    assert!(slab_room.options.supplies_floor_above);
    // The cutout: rough width (108 + 4) plus 6 in each side.
    assert_eq!(f.openings.len(), 1);
    let cut = &f.openings[0];
    assert_eq!(cut.width, 112.0 + 12.0);
    assert_eq!(cut.center_offset, 60.0);
    assert!(f
        .walls
        .iter()
        .any(|w| w.id == cut.wall_id && w.flags.foundation));
    // Rebuilding makes the same one, not two.
    p.build_foundation_with(&stem(48.0));
    assert_eq!(p.floors[0].openings.len(), 1);
}

#[test]
fn a_garage_stem_wall_is_never_shorter_than_the_minimum_garage_height() {
    let (mut p, _) = garage_house();
    let mut o = stem(36.0);
    o.settings.min_garage_height = 40.0;
    p.build_foundation_with(&o);
    let f = &p.floors[0];
    let front = f
        .walls
        .iter()
        .find(|w| w.start.x == 120.0 && w.end.x == 240.0 && w.start.y == 0.0)
        .unwrap();
    assert!(front.height >= 40.0 - 1e-9, "{}", front.height);
    // Without Garage Floor ticked it is built like the rest of the house.
    o.settings.garage_floor = false;
    p.build_foundation_with(&o);
    assert!(FoundationLayer::load(&p.floors[0]).slabs.is_empty());
    assert!(p.floors[0].openings.is_empty());
}

#[test]
fn a_monolithic_slab_lowers_and_curbs_the_garage() {
    let (mut p, _) = garage_house();
    let mut o = FoundationOptions::new(FoundationKind::MonolithicSlab);
    o.edge_height = 16.0;
    p.build_foundation_with(&o);
    let g = &p.floors[1].room_names[1];
    assert_eq!(g.floor_height_offset, -3.5, "Lower Garage Floor");
    // Both rooms are slab rooms of the monolithic foundation.
    for n in &p.floors[1].room_names {
        assert!(n.monolithic_slab.is_some() && n.options.floor_from_foundation);
    }
    let f = &p.floors[0];
    let layer = FoundationLayer::load(f);
    assert_eq!(
        layer.slabs.len(),
        2,
        "a slab for the house and one for the garage"
    );
    let mut tops: Vec<f64> = layer.slabs.iter().map(|s| s.top_elevation).collect();
    tops.sort_by(f64::total_cmp);
    assert!((tops[1] - tops[0] - 3.5).abs() < 1e-9, "{tops:?}");
    // The curb between them is a wall and the door leaves a cutout in it.
    assert_eq!(f.walls.iter().filter(|w| w.flags.foundation).count(), 7);
    assert!(f.walls.iter().all(|w| w.spec.foundation.chamfer_monolithic));
    assert_eq!(f.openings.len(), 1);
    // Another foundation type unticks the boxes again.
    p.build_foundation_with(&stem(48.0));
    for n in &p.floors[1].room_names {
        assert!(n.monolithic_slab.is_none());
    }
    assert!(!p.floors[1].room_names[0].options.floor_from_foundation);
}

#[test]
fn a_basement_of_48_inches_counts_as_living_area_and_72_gets_a_slab() {
    use plan_core::defaults::RoomTypeDef;
    use plan_core::living::room_in_living_area;
    use plan_core::rooms::detect_rooms;
    let types = vec![
        RoomTypeDef::new("Basement", "Standard", true, true),
        RoomTypeDef::new("Crawl Space", "Open Below", false, false),
    ];
    let platform = FLOOR_PLATFORM_THICKNESS;
    let living = |height: f64| -> (String, bool, bool) {
        let mut p = two_rooms();
        p.build_foundation_with(&stem(height));
        let f = &p.floors[0];
        let rooms = detect_rooms(&f.walls, 0.5);
        let n = &f.room_names[0];
        (
            n.room_type.clone(),
            n.has_floor,
            room_in_living_area(f, &rooms[0], &types),
        )
    };
    // Clear height 47.9: a crawl space.
    assert_eq!(
        living(47.9 + platform),
        ("Crawl Space".into(), false, false)
    );
    // 48: a basement without a floor or ceiling finish, in the Living Area.
    assert_eq!(living(48.0 + platform), ("Basement".into(), false, true));
    // 72 and a 4 in slab on top of it: a finished basement.
    assert_eq!(living(76.0 + platform), ("Basement".into(), true, true));
    // Between, the slab is left out.
    assert_eq!(living(75.0 + platform).1, false);
    let mut p = two_rooms();
    let mut o = stem(76.0 + platform);
    o.rooms = FoundationRooms::CrawlSpace;
    p.build_foundation_with(&o);
    assert_eq!(p.floors[0].room_names[0].room_type, "Crawl Space");
}

#[test]
fn piers_are_round_or_square_and_no_further_apart_than_the_maximum() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(240.0, 0.0);
    let pts = pier_positions(a, b, 96.0);
    assert_eq!(pts.len(), 4, "3 spans of 80 in");
    assert!(pts.windows(2).all(|w| w[0].dist(w[1]) <= 96.0 + 1e-9));
    assert_eq!(pier_positions(a, b, 240.0).len(), 2);
    assert_eq!(
        pier_positions(a, a, 96.0).len(),
        1,
        "a zero-length run is one pier"
    );
    let mut p = two_rooms();
    let mut o = FoundationOptions::new(FoundationKind::Pier);
    o.pier_spacing = 60.0;
    o.settings.pier_shape = PierShape::Square;
    o.settings.pier_width = 16.0;
    o.pier_height = 30.0;
    p.build_foundation_with(&o);
    let layer = FoundationLayer::load(&p.floors[0]);
    assert!(layer.piers.is_empty());
    assert!(layer.pads.len() >= 8);
    assert!(layer
        .pads
        .iter()
        .all(|d| d.size == 16.0 && d.thickness == 30.0));
    o.settings.pier_shape = PierShape::Round;
    p.build_foundation_with(&o);
    let layer = FoundationLayer::load(&p.floors[0]);
    assert!(layer.pads.is_empty());
    assert!(layer
        .piers
        .iter()
        .all(|d| d.diameter == 16.0 && d.height == 30.0));
}

// ----- Fireplace foundations -----

#[test]
fn a_masonry_fireplace_gets_a_foundation_block_of_its_own_size() {
    use plan_core::fireplace::{body_poly, FireplaceKind};
    let mut p = two_rooms();
    let fp = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(60.0, 4.0), 0.0);
    p.add_fireplace(0, FireplaceKind::Prefab, Point::new(180.0, 4.0), 0.0);
    p.build_foundation_with(&stem(48.0));
    let f = &p.floors[0];
    assert_eq!(f.fireplaces.len(), 1, "a prefab unit has no masonry base");
    let base = &f.fireplaces[0];
    assert_eq!(base.base_of, Some(fp));
    assert!(base.no_firebox && !base.hearth.enabled && !base.chimney.enabled);
    let sym = f.symbols.iter().find(|s| s.id == base.id).unwrap();
    let above = p.floors[1].symbols.iter().find(|s| s.id == fp).unwrap();
    assert_eq!(body_poly(sym), body_poly(above));
    assert_eq!(base.height, Some(48.0 - FLOOR_PLATFORM_THICKNESS));
    // Rebuilding replaces the block; a monolithic slab has none.
    p.build_foundation_with(&stem(48.0));
    assert_eq!(p.floors[0].fireplaces.len(), 1);
    assert_eq!(p.floors[0].symbols.len(), 1);
    p.build_foundation(FoundationKind::MonolithicSlab);
    assert!(p.floors[0].fireplaces.is_empty());
}

// ----- Auto Rebuild and the Options panel -----

#[test]
fn auto_rebuild_follows_floor_one_and_only_when_switched_on() {
    let none = |_: &str| false;
    let mut p = two_rooms();
    let mut o = stem(48.0);
    p.build_foundation_with(&o);
    p.floors[1].walls[0].start = Point::new(-30.0, 0.0);
    assert!(!p.auto_rebuild_foundation(&none), "off: nothing happens");
    o.settings.auto_rebuild = true;
    p.build_foundation_with(&o);
    assert!(
        !p.auto_rebuild_foundation(&none),
        "unchanged since the build"
    );
    p.floors[1].walls[0].start = Point::new(-60.0, 0.0);
    assert!(p.auto_rebuild_foundation(&none));
    assert!(p.floors[0]
        .walls
        .iter()
        .any(|w| w.start.x == -60.0 && w.flags.foundation));
    assert!(!p.auto_rebuild_foundation(&none), "once per change");
    // A new floor above is not Floor 1 and changes nothing.
    p.build_new_floor(true);
    assert!(!p.auto_rebuild_foundation(&none));
}

#[test]
fn the_options_panel_feeds_the_materials_list() {
    let mut p = two_rooms();
    let mut o = stem(48.0);
    p.build_foundation_with(&o);
    let rows = foundation_takeoff(&p);
    assert!(rows.iter().any(|r| r.item == "Footing rebar"));
    assert!(!rows.iter().any(|r| r.item == "Foam seal"));
    o.settings.foam_seal = true;
    o.settings.termite_flashing = true;
    p.build_foundation_with(&o);
    let rows = foundation_takeoff(&p);
    let perimeter = 2.0 * (240.0 + 120.0) / 12.0;
    let foam = rows.iter().find(|r| r.item == "Foam seal").unwrap();
    assert!((foam.quantity - perimeter).abs() < 0.5, "{}", foam.quantity);
    assert!(rows.iter().any(|r| r.item == "Termite flashing"));
    // More bars per course, more rebar.
    let before = rows
        .iter()
        .find(|r| r.item == "Footing rebar")
        .unwrap()
        .quantity;
    o.settings.rebar.footing.bars = 4;
    p.build_foundation_with(&o);
    let after = foundation_takeoff(&p)
        .into_iter()
        .find(|r| r.item == "Footing rebar")
        .unwrap()
        .quantity;
    assert!((after - 2.0 * before).abs() < 1e-6);
    // No foundation, no take-off.
    assert!(foundation_takeoff(&two_rooms()).is_empty());
}

#[test]
fn foundation_options_and_settings_round_trip_and_old_files_load() {
    let mut p = two_rooms();
    let mut o = stem(48.0);
    o.settings.hang_platform = true;
    o.settings.rebar.slab.bars = 3;
    p.build_foundation_with(&o);
    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.floors[0].settings.foundation_options, Some(o));
    assert_ne!(back.floors[0].settings.foundation_signature, 0);
    // A floor that never had a foundation writes none of it.
    let plain = serde_json::to_string(&two_rooms().floors[0].settings).unwrap();
    assert!(!plain.contains("foundation_options") && !plain.contains("foundation_signature"));
    // An options record from before the settings loads with the defaults.
    let old = serde_json::json!({"kind": {"StemWall": {"height": 36.0}}, "footing_width": 16.0});
    let o: FoundationOptions = serde_json::from_value(old).unwrap();
    assert!(!o.settings.hang_platform && o.settings.s_markers);
}

#[test]
fn a_floor_added_above_a_split_level_resets_its_ceiling_unless_stepped() {
    let make = || {
        let mut p = two_rooms();
        let r = name(&mut p, RIGHT, "Den");
        p.floors[0].room_names[r].floor_height_offset = 24.0;
        p.floors[0].room_names[r].ceiling_height = Some(WALL_H + 12.0);
        p
    };
    // Manual p. 772: the ceiling of the raised room goes back to the default.
    let mut p = make();
    p.build_new_floor_with(&NewFloorOptions::default()).unwrap();
    assert_eq!(p.floors[0].room_names[0].ceiling_height, None);
    // Stepping the new floor to match keeps it.
    let mut q = make();
    let o = NewFloorOptions {
        step_elevations: true,
        derive: DeriveFrom::AllWalls,
        ..NewFloorOptions::default()
    };
    q.build_new_floor_with(&o).unwrap();
    assert_eq!(
        q.floors[0].room_names[0].ceiling_height,
        Some(WALL_H + 12.0)
    );
}

// ----- fireplace plan parts -----

mod fireplaces {
    use super::*;
    use plan_core::fireplace::{
        body_poly, firebox_poly, hearth_poly, plan_dimensions, rough_poly, wall_cuts,
        FireplaceKind, PlanDimKind,
    };
    use plan_core::openings::spec::RoughMode;

    fn one(kind: FireplaceKind) -> (Project, u64) {
        let mut p = two_rooms();
        let id = p.add_fireplace(0, kind, Point::new(60.0, 4.0), 0.0);
        (p, id)
    }

    #[test]
    fn the_plan_shows_the_width_and_the_firebox_width() {
        let (p, id) = one(FireplaceKind::Masonry);
        let f = &p.floors[0];
        let sym = f.symbol(id).unwrap();
        let fp = f.fireplace_of(sym);
        let dims = plan_dimensions(&fp, sym);
        assert_eq!(dims.len(), 2);
        let firebox = dims
            .iter()
            .find(|d| d.kind == PlanDimKind::FireboxWidth)
            .unwrap();
        let width = dims.iter().find(|d| d.kind == PlanDimKind::Width).unwrap();
        assert_eq!(firebox.text, "3'-0\"");
        assert_eq!(width.text, "6'-0\"");
        assert!((firebox.from.dist(firebox.to) - 36.0).abs() < 1e-9);
        assert!((width.from.dist(width.to) - 72.0).abs() < 1e-9);
        // The width stands outside the firebox's line, in front of the hearth.
        assert!(
            width.from.y > firebox.from.y && firebox.from.y > sym.position.y + sym.depth + 16.0
        );
        // Suppress Dimensions, a chimney on its own and a base have none.
        let mut quiet = fp.clone();
        quiet.suppress_dimensions = true;
        assert!(plan_dimensions(&quiet, sym).is_empty());
        let (q, cid) = one(FireplaceKind::ChimneyOnly);
        let csym = q.floors[0].symbol(cid).unwrap();
        assert!(plan_dimensions(&q.floors[0].fireplace_of(csym), csym).is_empty());
        let mut no_box = fp;
        no_box.no_firebox = true;
        assert_eq!(plan_dimensions(&no_box, sym).len(), 1, "only the width");
    }

    #[test]
    fn the_firebox_can_be_offset_or_left_out() {
        let (p, id) = one(FireplaceKind::Masonry);
        let f = &p.floors[0];
        let sym = f.symbol(id).unwrap();
        let mut fp = f.fireplace_of(sym);
        let center = |poly: &[Point]| poly.iter().map(|q| q.x).sum::<f64>() / poly.len() as f64;
        let c0 = center(&firebox_poly(&fp, sym));
        fp.firebox.offset = 10.0;
        let moved = center(&firebox_poly(&fp, sym)) - c0;
        assert!((moved.abs() - 10.0).abs() < 1e-9, "{moved}");
        assert!(
            (center(&hearth_poly(&fp, sym)) - c0 - moved).abs() < 1e-9,
            "the hearth follows"
        );
        fp.no_firebox = true;
        assert!(firebox_poly(&fp, sym).is_empty() && hearth_poly(&fp, sym).is_empty());
        // The offset keeps the 4 in jambs when the record is read back.
        let mut wide = f.fireplace_of(sym);
        wide.firebox.offset = 500.0;
        let mut q = p.clone();
        q.floors[0].set_fireplace(wide);
        let read = q.floors[0].fireplace_of(q.floors[0].symbol(id).unwrap());
        assert!(
            (read.firebox.offset - (72.0 - 36.0) * 0.5 + 4.0).abs() < 1e-9,
            "{}",
            read.firebox.offset
        );
    }

    #[test]
    fn the_rough_opening_widens_the_cut_in_the_wall() {
        let mut p = Project::new("w");
        let wall = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            6.5,
            WALL_H,
            WallKind::Exterior,
        );
        let id = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(150.0, -3.25), 0.0);
        let f = &mut p.floors[0];
        let sym = f.symbol(id).unwrap().clone();
        let mut fp = f.fireplace_of(&sym);
        fp.in_wall = true;
        let w = f.wall(wall).unwrap().clone();
        let plain = wall_cuts(&fp, &sym, &w);
        assert_eq!(plain.len(), 1);
        assert!((plain[0].1 - plain[0].0 - 72.0).abs() < 1e-6);
        assert!(
            rough_poly(&fp, &sym).is_empty(),
            "no extra space, no rough outline"
        );
        fp.rough.mode = RoughMode::AdditionalSpace;
        fp.rough.add_width = 4.0;
        let wide = wall_cuts(&fp, &sym, &w);
        assert!((wide[0].1 - wide[0].0 - 76.0).abs() < 1e-6, "{wide:?}");
        assert_eq!(rough_poly(&fp, &sym).len(), 4);
        assert_eq!(body_poly(&sym).len(), 4);
        // A fireplace that is not in the wall cuts nothing.
        fp.in_wall = false;
        assert!(wall_cuts(&fp, &sym, &w).is_empty());
    }

    #[test]
    fn a_copy_of_the_symbol_brings_the_record_to_its_new_id() {
        let (mut p, id) = one(FireplaceKind::Masonry);
        let mut fp = p.floors[0].fireplace_of(p.floors[0].symbol(id).unwrap());
        fp.name = "Great Room Hearth".into();
        fp.base_of = Some(999);
        p.floors[0].set_fireplace(fp);
        let copy = p.floors[0].symbol_for_copy(p.floors[0].symbol(id).unwrap());
        let new = p.add_symbol(0, copy);
        assert_ne!(new, id);
        let f = &p.floors[0];
        let rec = f
            .fireplaces
            .iter()
            .find(|r| r.id == new)
            .expect("a record for the copy");
        assert_eq!(rec.name, "Great Room Hearth");
        assert_eq!(
            rec.base_of, None,
            "a copy of a foundation block stands alone"
        );
        assert!(
            f.symbol(new).unwrap().options.is_empty(),
            "the carrier is taken out"
        );
        // A plain symbol is copied as it is.
        let plain = f.symbol_for_copy(&plan_core::PlacedSymbol::new(
            "x",
            Point::ZERO,
            10.0,
            10.0,
            10.0,
        ));
        assert!(plain.options.is_empty());
    }
}

// ----- the terrain under a foundation, step marker places -----

#[test]
fn the_terrain_sits_under_the_stem_wall_tops() {
    use plan_core::foundation::terrain_elevation;
    let mut p = two_rooms();
    assert_eq!(terrain_elevation(&p), None, "no foundation, no rule");
    p.build_foundation_with(&stem(48.0));
    assert_eq!(
        terrain_elevation(&p),
        Some(-(FLOOR_PLATFORM_THICKNESS + 6.0))
    );
    let mut o = stem(48.0);
    o.settings.hang_platform = true;
    p.build_foundation_with(&o);
    assert_eq!(terrain_elevation(&p), Some(-6.0));
    p.build_foundation(FoundationKind::MonolithicSlab);
    assert_eq!(terrain_elevation(&p), Some(-8.0));
}

#[test]
fn the_s_of_a_step_sits_beside_the_taller_wall() {
    use plan_core::foundation::step_marker_label_at;
    let mut p = two_rooms();
    name(&mut p, LEFT, "Den");
    let r = name(&mut p, RIGHT, "Den");
    p.floors[0].room_names[r].floor_height_offset = 24.0;
    p.build_foundation_with(&stem(36.0));
    let f = &p.floors[0];
    for m in step_markers(f) {
        let at = step_marker_label_at(f, &m, 6.0);
        assert!(
            at.dist(m.at) > 6.0 && at.dist(m.at) < 20.0,
            "{at:?} vs {:?}",
            m.at
        );
    }
}
