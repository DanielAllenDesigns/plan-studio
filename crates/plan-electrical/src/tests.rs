use super::*;
use plan_core::{detect_rooms, Floor, Opening, Point, Room, Wall, WallKind};
use std::f64::consts::FRAC_PI_2;

fn wall(id: u64, a: (f64, f64), b: (f64, f64)) -> Wall {
    Wall {
        id,
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        thickness: 4.5,
        height: 109.125,
        kind: WallKind::Interior,
        layer: "Walls, Normal".into(),
        ..Default::default()
    }
}

/// 20' x 12' room (walls 1 south, 2 east, 3 north, 4 west) with one 36" door
/// centred 120" along the south wall.
fn room_20x12() -> (Floor, Vec<Room>) {
    let mut floor = Floor::new("1st Floor", 0.0);
    floor.walls = vec![
        wall(1, (0.0, 0.0), (240.0, 0.0)),
        wall(2, (240.0, 0.0), (240.0, 144.0)),
        wall(3, (240.0, 144.0), (0.0, 144.0)),
        wall(4, (0.0, 144.0), (0.0, 0.0)),
    ];
    floor.openings = vec![Opening::default_door(10, 1, 120.0)];
    let rooms = detect_rooms(&floor.walls, 0.5);
    assert_eq!(rooms.len(), 1);
    (floor, rooms)
}

fn offset_on(floor: &Floor, d: &Device) -> f64 {
    let w = floor.wall(d.wall_id.unwrap()).unwrap();
    d.position.sub(w.start).dot(w.direction())
}

#[test]
fn every_kind_has_a_symbol_and_legend_entry() {
    for kind in DeviceKind::all() {
        let kind = match kind {
            DeviceKind::RopeLight { .. } => DeviceKind::RopeLight { length: 48.0 },
            k => k,
        };
        assert!(!kind.symbol().is_empty(), "{kind:?}");
        assert!(!kind.name().is_empty());
    }
    assert_eq!(legend().len(), DeviceKind::all().len());
}

#[test]
fn symbol_world_rotates_and_translates() {
    let mut d = place_free(DeviceKind::Switch, Point::new(10.0, 20.0));
    d.angle = FRAC_PI_2;
    let Stroke::Line { a, b } = d.symbol_world()[0].clone() else {
        panic!("switch starts with its stem");
    };
    assert!(a.dist(Point::new(10.0, 20.0)) < 1e-9);
    // Stem runs 4" along local +X, which is now plan +Y.
    assert!(b.dist(Point::new(10.0, 24.0)) < 1e-9);
}

#[test]
fn kind_classification() {
    assert!(DeviceKind::Outlet110.is_wall_mounted());
    assert!(!DeviceKind::OutletFloor.is_wall_mounted());
    assert!(DeviceKind::CeilingFan.is_ceiling());
    assert!(!DeviceKind::WallSconce.is_ceiling());
    assert_eq!(DeviceKind::Outlet110.default_height(), 12.0);
    assert_eq!(DeviceKind::Switch.default_height(), 48.0);
    assert_eq!(DeviceKind::WallSconce.default_height(), 66.0);
    assert_eq!(DeviceKind::Panel.default_height(), 60.0);
}

#[test]
fn place_on_wall_is_half_thickness_off_centerline() {
    let mut w = wall(7, (0.0, 0.0), (100.0, 0.0));
    w.thickness = 6.0;
    let left = place_on_wall(DeviceKind::Outlet110, &w, 40.0, WallSide::Left);
    assert!(left.position.dist(Point::new(40.0, 3.0)) < 1e-9);
    assert!((left.angle - FRAC_PI_2).abs() < 1e-9);
    assert_eq!(left.wall_id, Some(7));
    assert_eq!(left.height, 12.0);
    let right = place_on_wall(DeviceKind::Switch, &w, 40.0, WallSide::Right);
    assert!(right.position.dist(Point::new(40.0, -3.0)) < 1e-9);
    assert!((right.angle + FRAC_PI_2).abs() < 1e-9);
    assert_eq!(right.height, 48.0);
}

#[test]
fn auto_outlets_respect_spacing_and_doors() {
    let (floor, rooms) = room_20x12();
    let devices = auto_place_outlets(&floor, &rooms, &[], &AutoOutletOptions::default());
    assert!(!devices.is_empty());
    assert!(devices.iter().all(|d| d.wall_id.is_some()));
    assert!(devices
        .iter()
        .all(|d| d.kind == DeviceKind::Outlet110 && d.height == 12.0));

    let half = 2.25;
    for w in &floor.walls {
        let mut spaces = vec![(half, w.length() - half)];
        if w.id == 1 {
            spaces = vec![(half, 102.0), (138.0, w.length() - half)];
        }
        let mut here: Vec<f64> = devices
            .iter()
            .filter(|d| d.wall_id == Some(w.id))
            .map(|d| offset_on(&floor, d))
            .collect();
        here.sort_by(f64::total_cmp);
        assert!(!here.is_empty(), "wall {} has no outlet", w.id);
        for (a, b) in spaces {
            let mut stops = vec![a];
            stops.extend(here.iter().copied().filter(|t| *t >= a && *t <= b));
            stops.push(b);
            assert!(
                stops.len() > 2,
                "wall {} space {a}..{b} has no outlet",
                w.id
            );
            for (i, pair) in stops.windows(2).enumerate() {
                let gap = pair[1] - pair[0];
                let is_end = i == 0 || i == stops.len() - 2;
                let limit = if is_end { 72.0 } else { 144.0 };
                assert!(gap <= limit + 1e-6, "wall {} gap {gap} over {limit}", w.id);
            }
        }
    }
    for d in devices.iter().filter(|d| d.wall_id == Some(1)) {
        let t = offset_on(&floor, d);
        assert!(
            (t - 102.0).abs() >= 6.0 && (t - 138.0).abs() >= 6.0,
            "outlet at {t}"
        );
    }
    // Every device sits on the room side of its wall.
    for d in &devices {
        assert!(plan_core::geometry::point_in_polygon(
            d.position + Point::new(d.angle.cos(), d.angle.sin()) * 2.0,
            &rooms[0].polygon
        ));
    }
}

#[test]
fn kitchen_gets_gfci_counter_outlets_at_44() {
    let (floor, rooms) = room_20x12();
    let types = vec![(rooms[0].label.clone(), RoomFunction::Kitchen)];
    let devices = auto_place_outlets(&floor, &rooms, &types, &AutoOutletOptions::default());
    assert!(!devices.is_empty());
    assert!(devices
        .iter()
        .all(|d| d.kind == DeviceKind::Gfci && d.height == 44.0 && d.wall_id.is_some()));
    for w in &floor.walls {
        let mut here: Vec<f64> = devices
            .iter()
            .filter(|d| d.wall_id == Some(w.id))
            .map(|d| offset_on(&floor, d))
            .collect();
        here.sort_by(f64::total_cmp);
        assert!(here.len() >= 2, "wall {} counter outlets: {here:?}", w.id);
        // Nothing within a counter's reach of 4' is uncovered inside a space.
        for pair in here.windows(2) {
            let straddles_door = w.id == 1 && pair[0] < 102.0 && pair[1] > 138.0;
            assert!(
                straddles_door || pair[1] - pair[0] <= 48.0 + 1e-6,
                "{pair:?}"
            );
        }
    }
}

#[test]
fn bath_is_gfci_at_wall_height_and_garage_always_gfci() {
    let (floor, rooms) = room_20x12();
    let label = rooms[0].label.clone();
    let opts = AutoOutletOptions::default();
    let bath = auto_place_outlets(
        &floor,
        &rooms,
        &[(label.clone(), RoomFunction::Bath)],
        &opts,
    );
    assert!(bath
        .iter()
        .all(|d| d.kind == DeviceKind::Gfci && d.height == 12.0));
    let no_gfci = AutoOutletOptions {
        gfci_in_wet_rooms: false,
        ..opts.clone()
    };
    let plain = auto_place_outlets(
        &floor,
        &rooms,
        &[(label.clone(), RoomFunction::Bath)],
        &no_gfci,
    );
    assert!(plain.iter().all(|d| d.kind == DeviceKind::Outlet110));
    let garage = auto_place_outlets(&floor, &rooms, &[(label, RoomFunction::Garage)], &no_gfci);
    assert!(garage.iter().all(|d| d.kind == DeviceKind::Gfci));
}

#[test]
fn tiny_wall_spaces_are_skipped() {
    let (mut floor, rooms) = room_20x12();
    // Door leaves only 10" between jamb and corner on the south wall's west end.
    floor.openings = vec![{
        let mut o = Opening::default_door(10, 1, 120.0);
        o.width = 215.0;
        o
    }];
    let devices = auto_place_outlets(&floor, &rooms, &[], &AutoOutletOptions::default());
    assert!(devices.iter().all(|d| d.wall_id != Some(1)));
}

#[test]
fn room_light_and_switch() {
    let (floor, rooms) = room_20x12();
    let light = auto_place_room_light(&rooms[0]);
    assert_eq!(light.kind, DeviceKind::CeilingLight);
    assert!(light.position.dist(Point::new(120.0, 72.0)) < 1e-6);
    assert!(light.wall_id.is_none());

    let door = &floor.openings[0];
    let sw = auto_place_switch(&rooms[0], door, &floor.walls[0]);
    assert_eq!(sw.kind, DeviceKind::Switch);
    assert_eq!(sw.height, 48.0);
    assert_eq!(sw.wall_id, Some(1));
    // Latch at the far jamb (138"), 6" beyond it, on the room (north) face.
    assert!(sw.position.dist(Point::new(144.0, 2.25)) < 1e-9);

    let mut flipped = door.clone();
    flipped.swing_flipped = true;
    let sw = auto_place_switch(&rooms[0], &flipped, &floor.walls[0]);
    assert!(sw.position.dist(Point::new(96.0, 2.25)) < 1e-9);
}

#[test]
fn room_light_stays_inside_an_l_shaped_room() {
    let pts = [
        (0.0, 0.0),
        (240.0, 0.0),
        (240.0, 60.0),
        (60.0, 60.0),
        (60.0, 240.0),
        (0.0, 240.0),
    ];
    let polygon: Vec<Point> = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
    let centroid = plan_core::geometry::polygon_centroid(&polygon);
    let room = Room {
        area_sq_in: plan_core::geometry::polygon_area(&polygon),
        centroid,
        polygon,
        label: "L".into(),
        ..Room::default()
    };
    let light = auto_place_room_light(&room);
    assert!(plan_core::geometry::point_in_polygon(
        light.position,
        &room.polygon
    ));
}

fn layer_with_switch_and_light() -> (ElectricalLayer, u64, u64) {
    let w = wall(1, (0.0, 0.0), (240.0, 0.0));
    let mut layer = ElectricalLayer::default();
    let sw = layer.add(place_on_wall(DeviceKind::Switch, &w, 50.0, WallSide::Left));
    let light = layer.add(place_free(
        DeviceKind::CeilingLight,
        Point::new(120.0, 72.0),
    ));
    (layer, sw, light)
}

#[test]
fn connect_makes_a_bulging_arc() {
    let (mut layer, sw, light) = layer_with_switch_and_light();
    assert!(connect(&mut layer, sw, light));
    assert!(!connect(&mut layer, sw, light), "no duplicate connections");
    assert!(!connect(&mut layer, sw, sw));
    assert!(!connect(&mut layer, sw, 999));
    assert_eq!(layer.connections.len(), 1);
    let c = layer.connections[0].clone();
    assert!(c.arc_bulge.abs() > 1.0);
    // Switch faces +Y; the chord's left normal has a +Y component.
    assert!(c.arc_bulge > 0.0);
    assert_eq!(layer.device(light).unwrap().switched_by, vec![sw]);

    let Some(Stroke::Arc {
        center,
        radius,
        start,
        sweep,
    }) = layer.connection_arc(&c)
    else {
        panic!("expected an arc");
    };
    let at = |a: f64| Point::new(center.x + radius * a.cos(), center.y + radius * a.sin());
    let (p0, p1) = (
        layer.device(sw).unwrap().position,
        layer.device(light).unwrap().position,
    );
    assert!(at(start).dist(p0) < 1e-6);
    assert!(at(start + sweep).dist(p1) < 1e-6);
    // The arc midpoint sits `arc_bulge` off the chord.
    let mid = at(start + sweep * 0.5);
    let chord_mid = Point::lerp(p0, p1, 0.5);
    assert!((mid.dist(chord_mid) - c.arc_bulge.abs()).abs() < 1e-6);
}

#[test]
fn connect_in_bulges_away_from_the_nearest_wall() {
    let (mut layer, sw, light) = layer_with_switch_and_light();
    // Wall hugging the light's east side: arc should bulge toward -x of the chord.
    let walls = vec![wall(9, (130.0, -500.0), (130.0, 500.0))];
    assert!(connect_in(&mut layer, sw, light, &walls));
    let away_from_wall = layer.connections[0].arc_bulge;
    // Chord (120,72)-(50,2.25) runs up-right; its left normal points up-left (-x).
    assert!(away_from_wall > 0.0);
    layer.connections.clear();
    let walls = vec![wall(9, (-10.0, -500.0), (-10.0, 500.0))];
    assert!(connect_in(&mut layer, sw, light, &walls));
    assert!(layer.connections[0].arc_bulge < 0.0);
}

#[test]
fn circuits_group_outlets_ten_per_circuit() {
    let mut layer = ElectricalLayer::default();
    for i in 0..25 {
        let mut d = place_free(DeviceKind::Outlet110, Point::new(f64::from(i) * 30.0, 0.0));
        d.height = 12.0;
        layer.add(d);
    }
    layer.add(place_free(DeviceKind::Outlet220, Point::new(0.0, 100.0)));
    for i in 0..5 {
        let mut d = place_free(DeviceKind::Gfci, Point::new(f64::from(i) * 48.0, 300.0));
        d.height = 44.0;
        layer.add(d);
    }
    for i in 0..13 {
        layer.add(place_free(
            DeviceKind::RecessedCan,
            Point::new(f64::from(i) * 24.0, 200.0),
        ));
    }
    let list = assign_circuits(&mut layer, &CircuitOptions::default());

    let general: Vec<_> = list
        .iter()
        .filter(|c| c.description.starts_with("General"))
        .collect();
    assert_eq!(general.len(), 3);
    assert!(general
        .iter()
        .all(|c| c.devices.len() <= 10 && c.amps == 20));
    assert_eq!(general.iter().map(|c| c.devices.len()).sum::<usize>(), 25);

    let dedicated: Vec<_> = list
        .iter()
        .filter(|c| c.description.starts_with("Dedicated"))
        .collect();
    assert_eq!(dedicated.len(), 1);
    assert_eq!(dedicated[0].devices.len(), 1);

    let kitchen: Vec<_> = list
        .iter()
        .filter(|c| c.description.starts_with("Kitchen"))
        .collect();
    assert_eq!(kitchen.len(), 2);
    assert_eq!(kitchen.iter().map(|c| c.devices.len()).sum::<usize>(), 5);

    let lights: Vec<_> = list
        .iter()
        .filter(|c| c.description.starts_with("Lighting"))
        .collect();
    assert_eq!(lights.len(), 2);
    assert!(lights.iter().all(|c| c.devices.len() <= 12 && c.amps == 15));

    // Numbers are sequential from 1 and written to the devices.
    let numbers: Vec<u32> = list.iter().map(|c| c.number).collect();
    assert_eq!(numbers, (1..=list.len() as u32).collect::<Vec<_>>());
    assert!(layer.devices.iter().all(|d| d.circuit.is_some()));
}

#[test]
fn schedule_counts_by_kind() {
    let (mut layer, _, _) = layer_with_switch_and_light();
    layer.add(place_free(
        DeviceKind::RopeLight { length: 48.0 },
        Point::ZERO,
    ));
    layer.add(place_free(
        DeviceKind::RopeLight { length: 96.0 },
        Point::ZERO,
    ));
    let s = schedule(&layer);
    assert_eq!(
        s,
        vec![
            ("Switch".to_string(), 1),
            ("Ceiling Light".to_string(), 1),
            ("Rope Light".to_string(), 2),
        ]
    );
}

#[test]
fn add_keeps_ids_unique_and_remove_cleans_connections() {
    let (mut layer, sw, light) = layer_with_switch_and_light();
    assert_ne!(sw, light);
    let mut dup = place_free(DeviceKind::Doorbell, Point::ZERO);
    dup.id = sw;
    let dup_id = layer.add(dup);
    assert_ne!(dup_id, sw);
    assert_eq!(layer.devices.len(), 3);
    connect(&mut layer, sw, light);
    layer.remove(sw);
    assert!(layer.connections.is_empty());
    assert!(layer.device(light).unwrap().switched_by.is_empty());
}

#[test]
fn meshes_are_white_boxes_and_discs() {
    let w = wall(1, (0.0, 0.0), (240.0, 0.0));
    let mut layer = ElectricalLayer::default();
    layer.add(place_on_wall(
        DeviceKind::Outlet110,
        &w,
        100.0,
        WallSide::Left,
    ));
    layer.add(place_free(
        DeviceKind::CeilingLight,
        Point::new(120.0, 72.0),
    ));
    layer.add(place_free(
        DeviceKind::RopeLight { length: 48.0 },
        Point::new(10.0, 10.0),
    ));
    layer.add(place_on_wall(DeviceKind::Panel, &w, 20.0, WallSide::Left));
    let ms = meshes(&layer, &[w], 0.0);
    assert_eq!(ms.len(), 4);
    assert!(ms
        .iter()
        .all(|m| m.material == plan_3d::Material::WindowFrame));
    assert!(ms
        .iter()
        .all(|m| m.triangle_count() > 0 && m.object_id.is_some()));

    // Plate: 2.75" wide, 4.5" tall, 0.25" proud of the left face (y = 2.25).
    let (lo, hi) = ms[0].bounds().unwrap();
    assert!((hi[0] - lo[0] - 2.75).abs() < 1e-4);
    assert!((hi[1] - lo[1] - 4.5).abs() < 1e-4);
    assert!((hi[2] - lo[2] - 0.25).abs() < 1e-4);
    assert!((lo[1] - 9.75).abs() < 1e-4 && (hi[1] - 14.25).abs() < 1e-4);
    // Scene Z is -plan y: the plate spans plan y 2.25..2.5.
    assert!((lo[2] + 2.5).abs() < 1e-4 && (hi[2] + 2.25).abs() < 1e-4);

    // Ceiling disc: 6" across, hanging from the ceiling.
    let (lo, hi) = ms[1].bounds().unwrap();
    assert!((hi[0] - lo[0] - 6.0).abs() < 1e-4);
    assert!((hi[1] - 109.125).abs() < 1e-4);
}

#[test]
fn serde_round_trip() {
    let (mut layer, sw, light) = layer_with_switch_and_light();
    layer.add(place_free(
        DeviceKind::RopeLight { length: 72.0 },
        Point::new(5.0, 6.0),
    ));
    connect(&mut layer, sw, light);
    let json = serde_json::to_string(&layer).unwrap();
    let back: ElectricalLayer = serde_json::from_str(&json).unwrap();
    assert_eq!(layer, back);
    let opts = AutoOutletOptions::default();
    let back: AutoOutletOptions =
        serde_json::from_str(&serde_json::to_string(&opts).unwrap()).unwrap();
    assert_eq!(opts, back);
}
