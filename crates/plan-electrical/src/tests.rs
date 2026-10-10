#![allow(clippy::field_reassign_with_default)]
use super::*;
use plan_core::rooms::ElectricalRules;
use plan_core::{detect_rooms, Floor, Id, Opening, Point, Room, Wall, WallKind};
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
fn meshes_are_plates_with_details_and_discs() {
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
    let of = |id: u64, m: plan_3d::Material| {
        ms.iter()
            .find(|x| x.object_id == Some(id) && x.material == m)
    };
    // Four devices; the outlet adds a dark slot mesh to its white plate.
    assert_eq!(ms.len(), 5);
    assert!(ms
        .iter()
        .all(|m| m.triangle_count() > 0 && m.object_id.is_some()));
    assert!(of(1, plan_3d::Material::Asphalt).is_some());

    // Plate: 2.75" wide, 4.5" tall, 0.25" proud of the left face (y = 2.25).
    let (lo, hi) = of(1, plan_3d::Material::WindowFrame)
        .unwrap()
        .bounds()
        .unwrap();
    assert!((hi[0] - lo[0] - 2.75).abs() < 1e-4);
    assert!((hi[1] - lo[1] - 4.5).abs() < 1e-4);
    assert!((hi[2] - lo[2] - 0.25).abs() < 1e-4);
    assert!((lo[1] - 9.75).abs() < 1e-4 && (hi[1] - 14.25).abs() < 1e-4);
    // Scene Z is -plan y: the plate spans plan y 2.25..2.5.
    assert!((lo[2] + 2.5).abs() < 1e-4 && (hi[2] + 2.25).abs() < 1e-4);

    // Ceiling disc: 12" across, hanging from the ceiling.
    let (lo, hi) = of(2, plan_3d::Material::WindowFrame)
        .unwrap()
        .bounds()
        .unwrap();
    assert!((hi[0] - lo[0] - 12.0).abs() < 1e-4);
    assert!((hi[1] - 109.125).abs() < 1e-4);
}

#[test]
fn fixture_meshes_exist_for_cans_pendants_fans_and_jacks() {
    let w = wall(1, (0.0, 0.0), (240.0, 0.0));
    let mut layer = ElectricalLayer::default();
    let can = layer.add(place_free(DeviceKind::RecessedCan, Point::new(40.0, 40.0)));
    let pendant = layer.add(place_free(DeviceKind::PendantLight, Point::new(80.0, 40.0)));
    let fan = layer.add(place_free(DeviceKind::CeilingFan, Point::new(120.0, 40.0)));
    let jack = layer.add(place_on_wall(DeviceKind::TvJack, &w, 50.0, WallSide::Left));
    let sconce = layer.add(place_on_wall(
        DeviceKind::WallSconce,
        &w,
        150.0,
        WallSide::Left,
    ));
    layer.device_mut(jack).unwrap().finish = "Black".into();
    let ms = meshes(&layer, &[w], 0.0);
    let parts = |id: u64| -> Vec<plan_3d::Material> {
        ms.iter()
            .filter(|m| m.object_id == Some(id))
            .map(|m| m.material)
            .collect()
    };
    use plan_3d::Material as M;
    // Can: white trim ring + dark aperture, flush with the ceiling.
    assert_eq!(parts(can), [M::WindowFrame, M::Asphalt]);
    // Pendant: shade + cord that reaches the ceiling.
    assert!(parts(pendant).contains(&M::WindowFrame) && parts(pendant).contains(&M::Metal));
    let cord = ms
        .iter()
        .find(|m| m.object_id == Some(pendant) && m.material == M::Metal)
        .unwrap();
    assert!((cord.bounds().unwrap().1[1] - 109.125).abs() < 1e-3);
    // Fan: hub + rod in metal, four blades in the body finish.
    assert!(parts(fan).contains(&M::Metal) && parts(fan).contains(&M::WindowFrame));
    // A finish changes the plate material.
    assert!(parts(jack).contains(&M::Asphalt));
    assert!(!parts(jack).contains(&M::WindowFrame));
    assert!(parts(sconce).contains(&M::WindowFrame));
    assert_eq!(finish_material("Stainless Steel"), M::Metal);
    assert_eq!(finish_material(""), M::WindowFrame);
}

#[test]
fn electrical_meshes_walks_every_floor_and_respects_the_layer() {
    let mut project = plan_core::Project::new("t");
    let mut upper = Floor::new("2nd Floor", 109.125);
    upper.walls = vec![wall(1, (0.0, 0.0), (240.0, 0.0))];
    project.floors.push(upper);
    for (i, y) in [(0, 0.0), (1, 60.0)] {
        let mut layer = ElectricalLayer::default();
        layer.add(place_free(DeviceKind::SmokeDetector, Point::new(y, 20.0)));
        project.floors[i].set_electrical(&layer).unwrap();
    }
    let ms = electrical_meshes(&project);
    assert_eq!(ms.len(), 2);
    // The second floor's detector hangs from 109 1/8" + its own ceiling.
    let top = ms
        .iter()
        .map(|m| m.bounds().unwrap().1[1])
        .fold(f32::MIN, f32::max);
    assert!((top - (109.125 + 109.125)).abs() < 1e-3, "{top}");
    project.layers.get_mut("Electrical").unwrap().display = false;
    assert!(electrical_meshes(&project).is_empty());
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

// ----- the Chief symbol set, heights and families -----

fn count_kind(strokes: &[Stroke], f: impl Fn(&Stroke) -> bool) -> usize {
    strokes.iter().filter(|s| f(s)).count()
}

#[test]
fn symbol_parts_per_kind() {
    let is_line = |s: &Stroke| matches!(s, Stroke::Line { .. });
    let is_circle = |s: &Stroke| matches!(s, Stroke::Circle { .. });
    let is_text = |s: &Stroke| matches!(s, Stroke::Text { .. });
    let is_poly = |s: &Stroke| matches!(s, Stroke::Polyline { .. });
    let texts = |k: DeviceKind| -> Vec<String> {
        k.symbol()
            .into_iter()
            .filter_map(|s| match s {
                Stroke::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    };
    // Duplex: a circle and two slots; quad: the same circle with four.
    let duplex = DeviceKind::Outlet110.symbol();
    assert_eq!(
        (count_kind(&duplex, is_circle), count_kind(&duplex, is_line)),
        (1, 2)
    );
    let quad = DeviceKind::Outlet110Quad.symbol();
    assert_eq!(
        (count_kind(&quad, is_circle), count_kind(&quad, is_line)),
        (1, 4)
    );
    // 220: three slots; GFCI: duplex plus tick marks and its label.
    assert_eq!(count_kind(&DeviceKind::Outlet220.symbol(), is_line), 3);
    assert_eq!(texts(DeviceKind::Gfci), ["GFCI"]);
    // Floor outlet: duplex inside a square.
    let floor = DeviceKind::OutletFloor.symbol();
    assert_eq!(
        (count_kind(&floor, is_circle), count_kind(&floor, is_poly)),
        (1, 1)
    );
    // Switches: S, S3, S4, SD share the stem and S glyph.
    assert!(texts(DeviceKind::Switch).is_empty());
    assert_eq!(texts(DeviceKind::Switch3Way), ["3"]);
    assert_eq!(texts(DeviceKind::Switch4Way), ["4"]);
    assert_eq!(texts(DeviceKind::SwitchDimmer), ["D"]);
    for k in [
        DeviceKind::Switch,
        DeviceKind::Switch3Way,
        DeviceKind::Switch4Way,
        DeviceKind::SwitchDimmer,
    ] {
        let s = k.symbol();
        assert_eq!(
            (count_kind(&s, is_line), count_kind(&s, is_poly)),
            (1, 1),
            "{k:?}"
        );
    }
    // Lights: ceiling = circle and cross, can = ring and dot, pendant likewise.
    let ceiling = DeviceKind::CeilingLight.symbol();
    assert_eq!(
        (
            count_kind(&ceiling, is_circle),
            count_kind(&ceiling, is_line)
        ),
        (1, 2)
    );
    assert_eq!(count_kind(&DeviceKind::RecessedCan.symbol(), is_circle), 2);
    assert_eq!(count_kind(&DeviceKind::PendantLight.symbol(), is_circle), 2);
    assert!(matches!(
        DeviceKind::WallSconce.symbol()[0],
        Stroke::Arc { .. }
    ));
    // Fan: hub and four blades.
    let fan = DeviceKind::CeilingFan.symbol();
    assert_eq!(
        (count_kind(&fan, is_circle), count_kind(&fan, is_poly)),
        (1, 4)
    );
    // Detectors, thermostat, doorbell and the low-voltage jacks carry letters.
    assert_eq!(texts(DeviceKind::SmokeDetector), ["SD"]);
    assert_eq!(texts(DeviceKind::CoDetector), ["CO"]);
    assert_eq!(texts(DeviceKind::Thermostat), ["T"]);
    assert_eq!(texts(DeviceKind::Doorbell), ["DB"]);
    assert_eq!(texts(DeviceKind::DataJack), ["D"]);
    assert_eq!(texts(DeviceKind::PhoneJack), ["T"]);
    assert_eq!(texts(DeviceKind::TvJack), ["TV"]);
    for k in [
        DeviceKind::DataJack,
        DeviceKind::PhoneJack,
        DeviceKind::TvJack,
    ] {
        let s = k.symbol();
        assert_eq!(
            (count_kind(&s, is_poly), count_kind(&s, is_text)),
            (1, 1),
            "{k:?}"
        );
    }
    assert_eq!(count_kind(&DeviceKind::Panel.symbol(), is_text), 1);
}

#[test]
fn default_heights_per_kind() {
    use DeviceKind as K;
    for k in [
        K::Outlet110,
        K::Outlet110Quad,
        K::Outlet220,
        K::Gfci,
        K::DataJack,
        K::PhoneJack,
    ] {
        assert_eq!(k.default_height(), 12.0, "{k:?}");
    }
    for k in [
        K::Switch,
        K::Switch3Way,
        K::Switch4Way,
        K::SwitchDimmer,
        K::Doorbell,
    ] {
        assert_eq!(k.default_height(), 48.0, "{k:?}");
    }
    assert_eq!(K::OutletFloor.default_height(), 0.0);
    assert_eq!(COUNTER_OUTLET_HEIGHT, 44.0);
    // Every kind except the rope light is in `all` once and wall/ceiling are exclusive.
    for k in DeviceKind::all() {
        assert!(!(k.is_wall_mounted() && k.is_ceiling()), "{k:?}");
    }
    assert_eq!(DeviceKind::all().len(), 28);
}

#[test]
fn a_kind_can_change_within_its_family() {
    use DeviceKind as K;
    let outlets = K::Outlet110.family();
    assert!(outlets.contains(&K::Outlet110Quad) && outlets.contains(&K::Gfci));
    assert!(!outlets.contains(&K::Switch));
    let switches = K::Switch.family();
    assert_eq!(
        switches,
        [
            K::Switch,
            K::SwitchWp,
            K::Switch3Way,
            K::Switch4Way,
            K::SwitchDimmer
        ]
    );
    // A rope light keeps its own length.
    assert_eq!(
        K::RopeLight { length: 30.0 }.family(),
        [K::RopeLight { length: 30.0 }]
    );
    assert!(K::PendantLight.family().contains(&K::RecessedCan));
    assert!(!K::PendantLight.family().contains(&K::OutletFloor));
}

// ----- connections: 3-way pairs and the bend handle -----

#[test]
fn a_three_way_pair_controls_the_light_from_both_ends() {
    let mut layer = ElectricalLayer::default();
    let a = layer.add(place_free(DeviceKind::Switch3Way, Point::new(0.0, 0.0)));
    let b = layer.add(place_free(DeviceKind::Switch3Way, Point::new(200.0, 0.0)));
    let light = layer.add(place_free(
        DeviceKind::CeilingLight,
        Point::new(100.0, 80.0),
    ));
    assert!(connect(&mut layer, a, light));
    assert!(connect(&mut layer, a, b), "the traveler between the pair");
    assert!(!connect(&mut layer, b, a), "already wired");
    // Both switches control the light; there are only two arcs.
    let mut by = layer.device(light).unwrap().switched_by.clone();
    by.sort();
    assert_eq!(by, [a, b]);
    assert_eq!(layer.connections.len(), 2);
    assert_eq!(layer.switch_group(b), [b, a]);
    assert_eq!(layer.loads_of(b), [light]);
    // A plain switch cannot be wired to another switch.
    let s1 = layer.add(place_free(DeviceKind::Switch, Point::new(0.0, 50.0)));
    let s2 = layer.add(place_free(DeviceKind::Switch, Point::new(10.0, 50.0)));
    assert!(!connect(&mut layer, s1, s2));
    assert!(!connect(&mut layer, s1, a));

    // Wiring the pair after the light also works, and disconnect undoes it.
    let mut other = ElectricalLayer::default();
    let a = other.add(place_free(DeviceKind::Switch3Way, Point::ZERO));
    let b = other.add(place_free(DeviceKind::Switch4Way, Point::new(100.0, 0.0)));
    let c = other.add(place_free(DeviceKind::Switch3Way, Point::new(200.0, 0.0)));
    let l = other.add(place_free(DeviceKind::RecessedCan, Point::new(100.0, 60.0)));
    assert!(connect(&mut other, a, b) && connect(&mut other, b, c));
    assert!(connect(&mut other, c, l));
    let mut by = other.device(l).unwrap().switched_by.clone();
    by.sort();
    assert_eq!(by, [a, b, c], "a 3-4-3 run: every switch controls the can");
    assert!(disconnect(&mut other, c, l));
    assert!(other.device(l).unwrap().switched_by.is_empty());
    assert!(!disconnect(&mut other, c, l));
}

#[test]
fn bending_a_connection_moves_the_arc_midpoint() {
    let (mut layer, sw, light) = layer_with_switch_and_light();
    assert!(connect(&mut layer, sw, light));
    let before = layer.arc_midpoint(&layer.connections[0]).unwrap();
    let a = layer.device(sw).unwrap().position;
    let b = layer.device(light).unwrap().position;
    let mid = Point::lerp(a, b, 0.5);
    let n = b.sub(a).perp().normalized();
    let target = mid + n * 30.0;
    assert!(layer.bend_connection(0, target));
    let after = layer.arc_midpoint(&layer.connections[0]).unwrap();
    assert!(after.dist(target) < 1e-9);
    assert_ne!(before, after);
    assert_eq!(
        layer.connection_handle_at(after + Point::new(1.0, 1.0), 3.0),
        Some(0)
    );
    assert_eq!(
        layer.connection_handle_at(Point::new(-500.0, 500.0), 3.0),
        None
    );
    // The stored arc really passes through the handle.
    let Some(Stroke::Arc { center, radius, .. }) = layer.connection_arc(&layer.connections[0])
    else {
        panic!("a bent connection is an arc");
    };
    assert!((center.dist(after) - radius).abs() < 1e-6);
    // Bending to the chord straightens it; a bad index is refused.
    assert!(layer.bend_connection(0, mid));
    assert!(matches!(
        layer.connection_arc(&layer.connections[0]),
        Some(Stroke::Line { .. })
    ));
    assert!(!layer.bend_connection(5, mid));
}

// ----- Auto Place Outlets on a 40' x 30' shell (NEC spacing) -----

/// Walls of a 40' x 30' shell (S, E, N, W), a 36" door near the south-west
/// corner and a 72" window on the east wall.
fn shell_40x30() -> (Floor, Vec<Room>) {
    let mut floor = Floor::new("1st Floor", 0.0);
    floor.walls = vec![
        wall(1, (0.0, 0.0), (480.0, 0.0)),
        wall(2, (480.0, 0.0), (480.0, 360.0)),
        wall(3, (480.0, 360.0), (0.0, 360.0)),
        wall(4, (0.0, 360.0), (0.0, 0.0)),
    ];
    for w in &mut floor.walls {
        w.thickness = 6.5;
    }
    floor.openings = vec![
        Opening::default_door(10, 1, 100.0),
        Opening::default_window(11, 2, 180.0),
    ];
    let rooms = detect_rooms(&floor.walls, 0.5);
    assert_eq!(rooms.len(), 1);
    (floor, rooms)
}

#[test]
fn outlets_on_the_40x30_shell_follow_the_nec_rules() {
    let (floor, rooms) = shell_40x30();
    let label = rooms[0].label.clone();
    let opts = AutoOutletOptions::default();
    let devices = auto_place_outlets(&floor, &rooms, &[], &opts);
    // 12' rule: along every wall space, no gap over 12' between outlets and
    // no point over 6' from one at the space ends.
    for w in &floor.walls {
        let door = floor
            .openings_on(w.id)
            .find(|o| o.kind == plan_core::OpeningKind::Door);
        let half = 3.25;
        let spaces = match door {
            Some(o) => vec![
                (half, o.start_offset()),
                (o.end_offset(), w.length() - half),
            ],
            None => vec![(half, w.length() - half)],
        };
        let here: Vec<f64> = devices
            .iter()
            .filter(|d| d.wall_id == Some(w.id))
            .map(|d| offset_on(&floor, d))
            .collect();
        for (a, b) in spaces {
            if b - a < opts.min_wall_segment {
                continue;
            }
            let mut stops = vec![a];
            let mut inside: Vec<f64> = here
                .iter()
                .copied()
                .filter(|t| *t >= a && *t <= b)
                .collect();
            inside.sort_by(f64::total_cmp);
            stops.extend(inside);
            stops.push(b);
            assert!(stops.len() > 2, "wall {} space {a}..{b}", w.id);
            for (i, pair) in stops.windows(2).enumerate() {
                let limit = if i == 0 || i == stops.len() - 2 {
                    72.0
                } else {
                    144.0
                };
                assert!(
                    pair[1] - pair[0] <= limit + 1e-6,
                    "wall {} gap {pair:?}",
                    w.id
                );
            }
        }
    }
    // Outlets stay a hand clear of the door jambs.
    for d in devices.iter().filter(|d| d.wall_id == Some(1)) {
        let t = offset_on(&floor, d);
        assert!(
            (t - 82.0).abs() >= 6.0 && (t - 118.0).abs() >= 6.0,
            "at {t}"
        );
    }

    // Kitchen: 44" GFCI counter outlets at most 4' apart on every wall.
    let kitchen = auto_place_outlets(
        &floor,
        &rooms,
        &[(label.clone(), RoomFunction::Kitchen)],
        &opts,
    );
    assert!(kitchen
        .iter()
        .all(|d| d.kind == DeviceKind::Gfci && d.height == 44.0));
    for w in &floor.walls {
        let mut here: Vec<f64> = kitchen
            .iter()
            .filter(|d| d.wall_id == Some(w.id))
            .map(|d| offset_on(&floor, d))
            .collect();
        here.sort_by(f64::total_cmp);
        assert!(here.len() >= 3, "wall {} has {here:?}", w.id);
        for pair in here.windows(2) {
            let across_door = w.id == 1 && pair[0] < 82.0 && pair[1] > 118.0;
            assert!(across_door || pair[1] - pair[0] <= 48.0 + 1e-6, "{pair:?}");
        }
    }
    // More outlets than the 12' rule gives.
    assert!(kitchen.len() > devices.len());

    // Bath: GFCI at the usual 12".
    let bath = auto_place_outlets(&floor, &rooms, &[(label, RoomFunction::Bath)], &opts);
    assert!(bath
        .iter()
        .all(|d| d.kind == DeviceKind::Gfci && d.height == 12.0));
    assert_eq!(bath.len(), devices.len());
}

// ----- round 14: flags, defaults, multi-way switches, exterior outlets -----

#[test]
fn flags_voltage_and_symbols_of_the_outlet_kinds() {
    use DeviceKind as K;
    assert_eq!(K::Outlet110.flags(), ["110V"]);
    assert_eq!(K::Outlet220.flags(), ["220V", "Dedicated"]);
    assert_eq!(K::Gfci.flags(), ["110V", "GFCI"]);
    assert_eq!(K::OutletWp.flags(), ["110V", "GFCI", "WP"]);
    assert_eq!(K::OutletDedicated.flags(), ["110V", "Dedicated"]);
    assert!(K::Switch.flags().is_empty() && K::Switch.voltage().is_none());
    let tag = |k: K, t: &str| {
        k.symbol()
            .iter()
            .any(|s| matches!(s, Stroke::Text { text, .. } if text == t))
    };
    assert!(tag(K::Gfci, "GFCI") && tag(K::OutletWp, "WP"));
    assert!(tag(K::Outlet220, "220V") && tag(K::OutletDedicated, "DED"));
    // Every outlet is a wall outlet except the floor one, all on the outlet family.
    assert!(K::OutletWp.is_wall_mounted() && K::OutletDedicated.is_wall_mounted());
    assert!(K::Outlet110.family().contains(&K::OutletWp));
    assert_eq!(K::OutletWp.default_height(), 18.0);
}

#[test]
fn default_heights_are_stored_in_the_project() {
    use DeviceKind as K;
    let mut project = plan_core::Project::new("x");
    let mut defaults = ElectricalDefaults::load(&project);
    assert_eq!(defaults.height(K::Outlet110), 12.0);
    assert_eq!(defaults.height(K::Switch), 48.0);
    assert_eq!(defaults.counter_height(), 44.0);
    defaults.set_height(K::Outlet110, 18.0);
    defaults.set_counter_height(40.0);
    defaults.set_height(K::Switch, 48.0); // the built-in height stores nothing
    defaults.store(&mut project);
    let back = ElectricalDefaults::load(&project);
    assert_eq!(back.height(K::Outlet110), 18.0);
    assert_eq!(
        back.height(K::Gfci),
        18.0,
        "one Outlet height for every receptacle"
    );
    assert_eq!(back.height(K::PhoneJack), 18.0, "and the data jacks");
    assert_eq!(back.height(K::WallSconce), 66.0, "lights keep their own");
    assert_eq!(back.counter_height(), 40.0);
    assert!(back.is_builtin(K::Switch) && !back.is_builtin(K::Outlet110));
    let json = project.to_json().unwrap();
    let again = plan_core::Project::from_json(&json).unwrap();
    assert_eq!(ElectricalDefaults::load(&again), back);
    // Auto Place Outlets honors the heights.
    let (floor, rooms) = room_20x12();
    let opts = AutoOutletOptions::with_defaults(&back);
    let general = auto_place_outlets(&floor, &rooms, &[], &opts);
    assert!(general.iter().all(|d| d.height == 18.0));
    let label = rooms[0].label.clone();
    let kitchen = auto_place_outlets(&floor, &rooms, &[(label, RoomFunction::Kitchen)], &opts);
    assert!(kitchen.iter().all(|d| d.height == 40.0));
    // A cleared record is not stored.
    ElectricalDefaults::default().store(&mut project);
    assert!(project.electrical_defaults.is_none());
}

fn light_with_switches(n: usize) -> (ElectricalLayer, Vec<Id>, Id) {
    let mut layer = ElectricalLayer::default();
    let light = layer.add(place_free(
        DeviceKind::CeilingLight,
        Point::new(100.0, 100.0),
    ));
    let sw: Vec<Id> = (0..n)
        .map(|i| {
            layer.add(place_free(
                DeviceKind::Switch,
                Point::new(i as f64 * 80.0, 0.0),
            ))
        })
        .collect();
    (layer, sw, light)
}

#[test]
fn a_light_with_two_switches_gets_3_way_switches_and_three_gets_a_4_way() {
    let (mut layer, sw, light) = light_with_switches(3);
    let kind = |l: &ElectricalLayer, i: usize| l.device(sw[i]).unwrap().kind;
    assert!(connect(&mut layer, sw[0], light));
    assert_eq!(
        kind(&layer, 0),
        DeviceKind::Switch,
        "one switch stays single"
    );
    assert!(connect(&mut layer, sw[1], light));
    assert_eq!(kind(&layer, 0), DeviceKind::Switch3Way);
    assert_eq!(kind(&layer, 1), DeviceKind::Switch3Way);
    assert_eq!(kind(&layer, 2), DeviceKind::Switch, "not wired yet");
    assert!(connect(&mut layer, sw[2], light));
    // The first and last to control the light are 3-way, the middle one 4-way.
    assert_eq!(kind(&layer, 0), DeviceKind::Switch3Way);
    assert_eq!(kind(&layer, 1), DeviceKind::Switch4Way);
    assert_eq!(kind(&layer, 2), DeviceKind::Switch3Way);
    // The S3 / S4 symbols carry the number.
    let label = |k: DeviceKind| {
        k.symbol().iter().find_map(|s| match s {
            Stroke::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
    };
    assert_eq!(label(DeviceKind::Switch3Way).as_deref(), Some("3"));
    assert_eq!(label(DeviceKind::Switch4Way).as_deref(), Some("4"));
    assert_eq!(label(DeviceKind::Switch), None);
    // Disconnecting one switch turns the others back.
    assert!(disconnect(&mut layer, sw[1], light));
    assert_eq!(kind(&layer, 0), DeviceKind::Switch3Way);
    assert_eq!(
        kind(&layer, 1),
        DeviceKind::Switch4Way,
        "no longer wired: kept"
    );
    assert_eq!(kind(&layer, 2), DeviceKind::Switch3Way);
    assert!(disconnect(&mut layer, sw[2], light));
    assert_eq!(kind(&layer, 0), DeviceKind::Switch, "alone again");
    // Removing a device does the same.
    let (mut layer, sw, light) = light_with_switches(2);
    connect(&mut layer, sw[0], light);
    connect(&mut layer, sw[1], light);
    layer.remove(sw[1]);
    assert_eq!(layer.device(sw[0]).unwrap().kind, DeviceKind::Switch);
    // A dimmer keeps its kind.
    let (mut layer, sw, light) = light_with_switches(2);
    layer.device_mut(sw[0]).unwrap().kind = DeviceKind::SwitchDimmer;
    connect(&mut layer, sw[0], light);
    connect(&mut layer, sw[1], light);
    assert_eq!(layer.device(sw[0]).unwrap().kind, DeviceKind::SwitchDimmer);
    assert_eq!(layer.device(sw[1]).unwrap().kind, DeviceKind::Switch3Way);
}

#[test]
fn exterior_receptacles_go_on_the_outside_front_and_back() {
    let (mut floor, rooms) = room_20x12();
    for w in &mut floor.walls {
        w.kind = WallKind::Exterior;
    }
    floor.openings = vec![Opening::default_door(10, 2, 72.0)];
    let opts = AutoOutletOptions::default();
    let wp = auto_place_exterior_outlets(&floor, &rooms, &opts);
    assert_eq!(wp.len(), 2, "front and back");
    assert!(wp
        .iter()
        .all(|d| d.kind == DeviceKind::OutletWp && d.height == 18.0));
    // The longest wall (south, 240") is the front, the north wall the back.
    let on = |id: Id| wp.iter().find(|d| d.wall_id == Some(id)).unwrap();
    let (south, north) = (on(1), on(3));
    // Outside: below the south wall (y < 0) and above the north wall (y > 144).
    assert!(south.position.y < -2.0, "{:?}", south.position);
    assert!(north.position.y > 146.0, "{:?}", north.position);
    // Facing away from the house.
    assert!(south.angle.sin() < -0.99 && north.angle.sin() > 0.99);
    // Not inside any room.
    assert!(wp.iter().all(|d| !rooms[0].polygon.is_empty()
        && !plan_core::geometry::point_in_polygon(d.position, &rooms[0].polygon)));
    // Interior walls get none.
    let (interior, rooms2) = room_20x12();
    assert!(auto_place_exterior_outlets(&interior, &rooms2, &opts).is_empty());
}

#[test]
fn a_tiny_bath_still_gets_a_gfci() {
    let mut floor = Floor::new("1st Floor", 0.0);
    floor.walls = vec![
        wall(1, (0.0, 0.0), (28.0, 0.0)),
        wall(2, (28.0, 0.0), (28.0, 28.0)),
        wall(3, (28.0, 28.0), (0.0, 28.0)),
        wall(4, (0.0, 28.0), (0.0, 0.0)),
    ];
    let rooms = detect_rooms(&floor.walls, 0.5);
    let types = [(rooms[0].label.clone(), RoomFunction::Bath)];
    let none = AutoOutletOptions {
        min_wet_segment: 40.0,
        ..AutoOutletOptions::default()
    };
    assert!(auto_place_outlets(&floor, &rooms, &types, &none).is_empty());
    let placed = auto_place_outlets(&floor, &rooms, &types, &AutoOutletOptions::default());
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].kind, DeviceKind::Gfci);
}

#[test]
fn the_schedule_rows_carry_mark_type_height_circuit_and_flags() {
    let mut layer = ElectricalLayer::default();
    let a = layer.add(place_free(DeviceKind::OutletWp, Point::ZERO));
    let b = layer.add(place_free(DeviceKind::Outlet220, Point::new(60.0, 0.0)));
    layer.device_mut(b).unwrap().circuit = Some(7);
    let rows = schedule_rows(&layer);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, a);
    assert_eq!(rows[0].mark, "E-01");
    assert_eq!(rows[0].kind, "WP Outlet");
    assert_eq!(rows[0].height, 18.0);
    assert_eq!(rows[0].flags, "110V, GFCI, WP");
    assert_eq!(rows[1].mark, "E-02");
    assert_eq!(rows[1].circuit, "7");
    assert_eq!(rows[1].flags, "220V, Dedicated");
    // The dedicated kinds each get a circuit of their own.
    let mut layer = ElectricalLayer::default();
    layer.add(place_free(DeviceKind::OutletDedicated, Point::ZERO));
    layer.add(place_free(DeviceKind::Outlet220, Point::new(30.0, 0.0)));
    layer.add(place_free(DeviceKind::Outlet110, Point::new(60.0, 0.0)));
    let list = circuits(&layer, &CircuitOptions::default());
    assert_eq!(list.len(), 3);
    assert!(list[0].description.starts_with("Dedicated"));
}

#[test]
fn wall_devices_are_modeled_facing_out_of_their_wall() {
    // A WP cover and a plate on the north wall of a room stand proud of the
    // wall face on the room side, not on the far side.
    let mut layer = ElectricalLayer::default();
    let w = wall(1, (0.0, 100.0), (200.0, 100.0));
    for (k, x) in [(DeviceKind::Outlet110, 50.0), (DeviceKind::OutletWp, 150.0)] {
        // Room side of the north wall is -y (the left normal of an eastward wall is +y).
        layer.add(place_on_wall(k, &w, x, WallSide::Right));
    }
    let meshes = meshes(&layer, std::slice::from_ref(&w), 0.0);
    assert!(!meshes.is_empty());
    for m in &meshes {
        // Scene Z = -plan y; the room side is plan y < 98 -> scene z > -98.
        let nearest_wall = m
            .vertices
            .iter()
            .map(|v| f64::from(v.position[2]))
            .fold(f64::INFINITY, f64::min);
        assert!(
            nearest_wall >= -98.0 - 1e-3,
            "starts on the room side: {nearest_wall}"
        );
    }
    // The WP cover is deeper than the plate.
    let depth = |k: DeviceKind| {
        let mut l = ElectricalLayer::default();
        l.add(place_on_wall(k, &w, 50.0, WallSide::Right));
        let zs: Vec<f64> = super::meshes(&l, std::slice::from_ref(&w), 0.0)
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| f64::from(v.position[2])))
            .collect();
        zs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - zs.iter().cloned().fold(f64::INFINITY, f64::min)
    };
    assert!(depth(DeviceKind::OutletWp) > depth(DeviceKind::Outlet110) + 0.5);
}

#[test]
fn kitchen_counter_outlets_follow_the_base_cabinets_against_the_walls() {
    let (floor, rooms) = room_20x12();
    let types = vec![(rooms[0].label.clone(), RoomFunction::Kitchen)];
    // A run of base cabinets along the north wall (face at y = 141.75), 24"
    // deep, from x = 40 to 200; an island in the middle floor is no run.
    let run = vec![
        Point::new(40.0, 117.75),
        Point::new(200.0, 117.75),
        Point::new(200.0, 141.75),
        Point::new(40.0, 141.75),
    ];
    let island = vec![
        Point::new(80.0, 50.0),
        Point::new(160.0, 50.0),
        Point::new(160.0, 74.0),
        Point::new(80.0, 74.0),
    ];
    let opts = AutoOutletOptions {
        counter_runs: vec![run, island],
        ..AutoOutletOptions::default()
    };
    let devices = auto_place_outlets(&floor, &rooms, &types, &opts);
    assert!(!devices.is_empty());
    // Only the north wall (3) has counter, so only it gets outlets.
    assert!(devices.iter().all(|d| d.wall_id == Some(3)), "{devices:?}");
    // They stay inside the cabinet run (the wall runs east to west, so the
    // offset along it counts from x = 240).
    for d in &devices {
        assert!(
            d.position.x >= 40.0 && d.position.x <= 200.0,
            "{:?}",
            d.position
        );
        assert_eq!(d.height, 44.0);
    }
    // 160" of counter at most 48" apart: at least four outlets.
    assert!(devices.len() >= 4, "{}", devices.len());
    // Without cabinets every wall keeps its outlets, as before.
    let plain = auto_place_outlets(&floor, &rooms, &types, &AutoOutletOptions::default());
    assert!(plain.iter().any(|d| d.wall_id == Some(1)));
    assert!(plain.iter().any(|d| d.wall_id == Some(2)));
}

// ----- Round 16 brief 25: Electrical Defaults, outlets, splines, rope lights -----

#[test]
fn the_four_height_groups_resolve_in_every_place() {
    use DeviceKind as K;
    let d = ElectricalDefaults::default();
    // On a wall: the Outlet and Switch groups.
    assert_eq!(d.height_for(K::Outlet110, HeightContext::Wall), 12.0);
    assert_eq!(d.height_for(K::TvJack, HeightContext::Wall), 12.0);
    assert_eq!(d.height_for(K::Thermostat, HeightContext::Wall), 48.0);
    assert_eq!(d.height_for(K::Doorbell, HeightContext::Wall), 48.0);
    // Above a base cabinet: up from the counter top (36" + 1 1/2" slab).
    let over = HeightContext::AboveCounter { counter_top: 37.5 };
    assert_eq!(d.height_for(K::Gfci, over), 45.5);
    assert_eq!(d.height_for(K::Switch, over), 45.5, "switches too");
    // On the side of a cabinet: 32" up its bottom, kept on the box.
    let wall_cab = HeightContext::CabinetSide {
        bottom: 54.0,
        top: 84.0,
    };
    assert_eq!(
        d.height_for(K::Outlet110, wall_cab),
        84.0,
        "clamped to the top"
    );
    let low = HeightContext::CabinetSide {
        bottom: 0.0,
        top: 30.0,
    };
    assert_eq!(d.height_for(K::Outlet110, low), 30.0);
    let tall = HeightContext::CabinetSide {
        bottom: 0.0,
        top: 84.0,
    };
    assert_eq!(d.height_for(K::Outlet110, tall), 32.0);
    // Lights and detectors are not in a group.
    assert_eq!(d.height_for(K::WallSconce, over), 66.0);
    // Use Default Heights off: the height saved with the symbol.
    let mut off = d.clone();
    off.use_default_heights = false;
    off.outlet_height = 20.0;
    assert_eq!(off.height_for(K::Outlet110, HeightContext::Wall), 12.0);
    assert_eq!(off.height_for(K::Outlet110, over), 12.0);
    // The groups are editable and a changed one is stored.
    let mut e = ElectricalDefaults::default();
    e.set_height(K::Gfci, 16.0);
    e.set_height(K::SwitchDimmer, 44.0);
    e.above_base_cabinet = 6.0;
    assert_eq!(e.height(K::Outlet220), 16.0);
    assert_eq!(e.height(K::Switch3Way), 44.0);
    assert_eq!(e.height_for(K::Outlet110, over), 43.5);
    assert!(!e.is_builtin(K::Outlet110) && !e.is_builtin(K::Switch));
}

#[test]
fn old_per_kind_heights_are_read_into_the_four_groups() {
    let mut project = plan_core::Project::new("x");
    project.electrical_defaults = Some(serde_json::json!({
        "heights": {"110V Outlet": 16.0, "Switch": 44.0, "Counter Outlet": 40.0}
    }));
    let d = ElectricalDefaults::load(&project);
    assert_eq!(d.outlet_height, 16.0);
    assert_eq!(d.switch_height, 44.0);
    assert_eq!(d.above_base_cabinet, 4.0, "40\" over a 36\" counter");
    assert!(d.use_default_heights);
    // Storing writes the new record only.
    d.store(&mut project);
    let v = project.electrical_defaults.clone().unwrap();
    assert!(v.get("heights").is_none(), "{v}");
    assert_eq!(v["outlet_height"], 16.0);
    assert_eq!(ElectricalDefaults::load(&project), d);
    // Garbage and unknown keys fall back to the built-in defaults.
    project.electrical_defaults = Some(serde_json::json!({"outlet_height": "tall"}));
    assert_eq!(
        ElectricalDefaults::load(&project),
        ElectricalDefaults::default()
    );
    project.electrical_defaults = Some(serde_json::json!({"something_else": 1}));
    assert_eq!(
        ElectricalDefaults::load(&project),
        ElectricalDefaults::default()
    );
}

#[test]
fn library_objects_connection_and_rope_defaults_round_trip() {
    use DeviceKind as K;
    let mut d = ElectricalDefaults::default();
    assert_eq!(d.object("Light", K::CeilingLight), K::CeilingLight);
    d.set_object("Light", K::CeilingLight, K::RecessedCan);
    d.set_object("110V Outlet", K::Outlet110, K::Outlet110);
    assert_eq!(d.object("Light", K::CeilingLight), K::RecessedCan);
    assert_eq!(d.objects.len(), 1, "the tool's own kind stores nothing");
    d.connection.curvature_ratio = 0.35;
    d.connection.line_style = plan_core::LineStyle::DashDot;
    d.connection.arrow = Arrow::Both;
    d.rope.spacing = 4.0;
    d.rope.center_lights = false;
    let mut project = plan_core::Project::new("x");
    d.store(&mut project);
    let json = project.to_json().unwrap();
    let back = ElectricalDefaults::load(&plan_core::Project::from_json(&json).unwrap());
    assert_eq!(back, d);
    // Every tool slot lists its own kind among its choices.
    for s in TOOL_SLOTS {
        assert!(s.choices.contains(&s.builtin), "{}", s.key);
        assert!(slot_of(s.builtin).is_some());
    }
    assert!(slot_of(K::OutletWp).is_none() && slot_of(K::Switch3Way).is_none());
}

#[test]
fn set_as_default_copies_the_type_and_height_of_a_device() {
    use DeviceKind as K;
    let w = wall(1, (0.0, 0.0), (240.0, 0.0));
    let mut dev = place_on_wall(K::RecessedCan, &w, 50.0, WallSide::Left);
    dev.height = 90.0;
    let mut d = ElectricalDefaults::default();
    assert!(d.set_from_device(&dev, HeightContext::Wall));
    assert_eq!(d.object("Light", K::CeilingLight), K::RecessedCan);
    assert!(!d.set_from_device(&dev, HeightContext::Wall), "nothing new");
    let mut outlet = place_on_wall(K::Gfci, &w, 80.0, WallSide::Left);
    outlet.height = 40.0;
    // Over a counter the Above Base Cabinet height is what changes.
    assert!(d.set_from_device(&outlet, HeightContext::AboveCounter { counter_top: 36.0 }));
    assert_eq!(d.above_base_cabinet, 4.0);
    assert_eq!(d.outlet_height, 12.0);
    // On the wall it is the Outlet group.
    assert!(d.set_from_device(&outlet, HeightContext::Wall));
    assert_eq!(d.outlet_height, 40.0);
}

#[test]
fn a_face_on_the_outside_is_weatherproof_and_one_on_the_inside_is_not() {
    use DeviceKind as K;
    let (mut floor, rooms) = room_20x12();
    let none = |_: &Room| false;
    // Interior-kind walls: neither face is outdoors.
    let w = floor.wall(1).unwrap().clone();
    assert!(!face_is_exterior(&w, WallSide::Left, 100.0, &rooms, &none));
    assert!(!face_is_exterior(&w, WallSide::Right, 100.0, &rooms, &none));
    // Exterior walls: the face away from the room is outdoors.
    for wl in &mut floor.walls {
        wl.kind = WallKind::Exterior;
    }
    let w = floor.wall(1).unwrap().clone();
    assert!(
        !face_is_exterior(&w, WallSide::Left, 100.0, &rooms, &none),
        "room side"
    );
    assert!(
        face_is_exterior(&w, WallSide::Right, 100.0, &rooms, &none),
        "outside"
    );
    // The north wall runs west: its left face is the room's, its right outside.
    let n = floor.wall(3).unwrap().clone();
    assert!(!face_is_exterior(&n, WallSide::Left, 100.0, &rooms, &none));
    assert!(face_is_exterior(&n, WallSide::Right, 100.0, &rooms, &none));
    // A room the closure calls exterior (a deck) makes its wall faces outdoor.
    let deck = |_: &Room| true;
    let inner = floor.wall(1).unwrap().clone();
    assert!(face_is_exterior(
        &inner,
        WallSide::Left,
        100.0,
        &rooms,
        &deck
    ));
    // The tools place the weatherproof counterparts there.
    assert_eq!(kind_for_setting(K::Outlet110, true), K::OutletWp);
    assert_eq!(kind_for_setting(K::Gfci, true), K::OutletWp);
    assert_eq!(kind_for_setting(K::Switch, true), K::SwitchWp);
    assert_eq!(kind_for_setting(K::WallSconce, true), K::WallLightExterior);
    assert_eq!(
        kind_for_setting(K::Outlet220, true),
        K::Outlet220,
        "no WP 220V"
    );
    assert_eq!(kind_for_setting(K::Outlet110, false), K::Outlet110);
    for k in [K::OutletWp, K::SwitchWp, K::WallLightExterior] {
        assert!(k.is_weatherproof() && k.is_wall_mounted(), "{k:?}");
        assert!(k.flags().contains(&"WP"), "{k:?}");
        assert!(!k.symbol().is_empty());
    }
    assert_eq!(K::OutletWp.indoor(), K::Gfci);
    assert_eq!(K::SwitchWp.indoor(), K::Switch);
    assert!(K::PathLight.is_light() && !K::PathLight.is_wall_mounted());
}

fn spline_layer() -> (ElectricalLayer, u64, u64) {
    let mut layer = ElectricalLayer::default();
    let a = layer.add(place_free(DeviceKind::Switch, Point::new(0.0, 0.0)));
    let b = layer.add(place_free(DeviceKind::CeilingLight, Point::new(100.0, 0.0)));
    (layer, a, b)
}

#[test]
fn a_new_connection_is_an_arc_of_the_default_curvature_ratio() {
    let (mut layer, a, b) = spline_layer();
    assert!(connect(&mut layer, a, b));
    let c = layer.connections[0].clone();
    assert!(c.is_arc());
    assert!((layer.curvature_ratio(&c).unwrap() - DEFAULT_CURVATURE).abs() < 1e-9);
    // A straight run has a path of two points; an arc is sampled finely.
    let path = layer.connection_path(&c).unwrap();
    assert!(path.len() > 10);
    assert!(path[0].dist(Point::new(0.0, 0.0)) < 1e-6);
    assert!(path[path.len() - 1].dist(Point::new(100.0, 0.0)) < 1e-6);
    // The Electrical Connection Defaults set the ratio, style and arrow.
    let (mut l2, a2, b2) = spline_layer();
    let d = ConnectionDefaults {
        curvature_ratio: 0.4,
        line_style: plan_core::LineStyle::Solid,
        arrow: Arrow::End,
        label: "Hall".into(),
    };
    assert!(connect_with(&mut l2, a2, b2, &[], &d));
    let c2 = &l2.connections[0];
    assert!((l2.curvature_ratio(c2).unwrap() - 0.4).abs() < 1e-9);
    assert_eq!(c2.style(), plan_core::LineStyle::Solid);
    assert_eq!(c2.arrow, Arrow::End);
    assert_eq!(c2.label, "Hall");
    // Zero curvature is a straight line.
    let (mut l3, a3, b3) = spline_layer();
    let d0 = ConnectionDefaults {
        curvature_ratio: 0.0,
        ..ConnectionDefaults::default()
    };
    connect_with(&mut l3, a3, b3, &[], &d0);
    assert_eq!(l3.connection_path(&l3.connections[0]).unwrap().len(), 2);
}

#[test]
fn splines_take_vertices_and_reset_curvature_takes_them_away() {
    let (mut layer, a, b) = spline_layer();
    connect(&mut layer, a, b);
    let bulge = layer.connections[0].arc_bulge;
    // Handles: start, the arc's midpoint, end.
    let h = layer.connection_handles(&layer.connections[0]).unwrap();
    assert_eq!(h.len(), 3);
    // Double-click on the curve adds a vertex; the arc becomes a spline.
    let at = Point::new(70.0, 30.0);
    let hi = layer.insert_vertex(0, at).unwrap();
    assert!(!layer.connections[0].is_arc());
    assert_eq!(layer.connections[0].vertices.len(), 2);
    assert_eq!(
        layer
            .connection_handles(&layer.connections[0])
            .unwrap()
            .len(),
        4
    );
    assert!(hi >= 1);
    // The spline passes through its vertices.
    let path = layer.connection_path(&layer.connections[0]).unwrap();
    for v in &layer.connections[0].vertices {
        assert!(path.iter().any(|p| p.dist(*v) < 1e-6), "{v:?}");
    }
    // Moving a vertex moves only it.
    let before = layer.connections[0].vertices.clone();
    assert!(layer.move_handle(0, 1, Point::new(30.0, -40.0)));
    let after = layer.connections[0].vertices.clone();
    assert_eq!(after[0], Point::new(30.0, -40.0));
    assert_eq!(after[1], before[1]);
    // The ends are not moved by move_handle.
    assert!(!layer.move_handle(0, 0, Point::ZERO));
    assert!(!layer.move_handle(0, 3, Point::ZERO));
    // Removing both vertices makes it an arc again.
    assert!(layer.remove_vertex(0, 1));
    assert!(layer.remove_vertex(0, 1));
    assert!(layer.connections[0].is_arc());
    assert!(!layer.remove_vertex(0, 1), "an arc has no vertex to remove");
    // Reset Curvature: breaks removed, original direction, the given ratio.
    layer.insert_vertex(0, at);
    layer.insert_vertex(0, Point::new(20.0, 20.0));
    assert!(layer.reset_curvature(0, 0.3));
    let c = &layer.connections[0];
    assert!(c.is_arc());
    assert!((layer.curvature_ratio(c).unwrap() - 0.3).abs() < 1e-9);
    assert_eq!(c.arc_bulge.signum(), bulge.signum(), "same direction");
    // Bending the arc by its handle still works.
    assert!(layer.move_handle(0, 1, Point::new(50.0, 20.0)));
    assert!((layer.connections[0].arc_bulge - 20.0).abs() < 1e-9);
    // A click on the curve finds it; a click far away does not.
    let near = layer.arc_midpoint(&layer.connections[0]).unwrap();
    assert_eq!(layer.connection_at(near, 2.0), Some(0));
    assert_eq!(layer.connection_at(Point::new(300.0, 300.0), 2.0), None);
    assert_eq!(layer.connection_handle_hit(near, 2.0, false), Some((0, 1)));
    assert_eq!(layer.connection_handle_hit(Point::ZERO, 2.0, false), None);
    assert_eq!(
        layer.connection_handle_hit(Point::ZERO, 2.0, true),
        Some((0, 0))
    );
}

#[test]
fn dragging_an_end_off_detaches_it_and_dropping_it_on_a_device_attaches_it() {
    let (mut layer, a, b) = spline_layer();
    let c2 = layer.add(place_free(DeviceKind::RecessedCan, Point::new(100.0, 60.0)));
    connect(&mut layer, a, b);
    connect(&mut layer, a, c2);
    assert_eq!(layer.device(b).unwrap().switched_by, vec![a]);
    // Detach the light end of the first connection: the spline stays, free.
    assert!(layer.detach_end(0, ConnEnd::End, Point::new(160.0, 10.0)));
    let c = layer.connections[0].clone();
    assert_eq!((c.from, c.to), (a, 0));
    assert_eq!(c.to_at, Some(Point::new(160.0, 10.0)));
    assert!(
        layer.device(b).unwrap().switched_by.is_empty(),
        "no longer controlled"
    );
    assert_eq!(
        layer.device(c2).unwrap().switched_by,
        vec![a],
        "the other is untouched"
    );
    let ends = layer.connection_ends(&layer.connections[0]).unwrap();
    assert_eq!(ends.1, Point::new(160.0, 10.0));
    assert!(layer.connection_path(&layer.connections[0]).is_some());
    // The free end moves.
    assert!(layer.detach_end(0, ConnEnd::End, Point::new(170.0, 10.0)));
    assert_eq!(layer.connections[0].to_at, Some(Point::new(170.0, 10.0)));
    // Drop it back on the light: wired again.
    assert!(layer.attach_end(0, ConnEnd::End, b));
    assert_eq!(layer.connections[0].to, b);
    assert_eq!(layer.connections[0].to_at, None);
    assert_eq!(layer.device(b).unwrap().switched_by, vec![a]);
    // Dropping it on the other light's pair that already exists is refused.
    assert!(
        !layer.attach_end(0, ConnEnd::End, c2),
        "a already connects to c2"
    );
    assert!(!layer.attach_end(0, ConnEnd::End, 999));
    // Free splines: both ends free, one attached to a device.
    let d = ConnectionDefaults::default();
    let i = layer.add_free_connection(Point::new(0.0, 100.0), Point::new(80.0, 100.0), &d);
    assert_eq!((layer.connections[i].from, layer.connections[i].to), (0, 0));
    assert!(layer.attach_end(i, ConnEnd::Start, a));
    assert_eq!(
        layer.device(b).unwrap().switched_by,
        vec![a],
        "no wiring from a free end"
    );
    // Removing a device deletes every spline on it.
    layer.remove(a);
    assert!(layer.connections.iter().all(|c| c.from != a && c.to != a));
    // The free spline round-trips through JSON.
    let (mut l, a, _) = spline_layer();
    let i = l.add_free_connection(Point::new(1.0, 2.0), Point::new(3.0, 4.0), &d);
    l.insert_vertex(i, Point::new(2.0, 5.0));
    l.attach_end(i, ConnEnd::Start, a);
    let back: ElectricalLayer = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
    assert_eq!(back, l);
}

#[test]
fn deleting_a_connection_unwires_its_load() {
    let (mut layer, a, b) = spline_layer();
    connect(&mut layer, a, b);
    assert!(layer.remove_connection(0));
    assert!(layer.device(b).unwrap().switched_by.is_empty());
    assert!(!layer.remove_connection(0));
}

#[test]
fn three_and_four_way_numbering_is_automatic_unless_switched_off() {
    let (mut layer, sw, light) = light_with_switches(4);
    for s in &sw {
        assert!(connect(&mut layer, *s, light));
    }
    let kinds: Vec<_> = sw.iter().map(|s| layer.device(*s).unwrap().kind).collect();
    assert_eq!(
        kinds,
        [
            DeviceKind::Switch3Way,
            DeviceKind::Switch4Way,
            DeviceKind::Switch4Way,
            DeviceKind::Switch3Way
        ]
    );
    // The way number is in the symbol: S3 and S4.
    let has = |k: DeviceKind, t: &str| {
        k.symbol()
            .iter()
            .any(|s| matches!(s, Stroke::Text { text, .. } if text == t))
    };
    assert!(has(DeviceKind::Switch3Way, "3") && has(DeviceKind::Switch4Way, "4"));
    // Deleting a wire demotes them again.
    layer.remove_connection(1);
    layer.remove_connection(1);
    let kinds: Vec<_> = sw.iter().map(|s| layer.device(*s).unwrap().kind).collect();
    assert_eq!(kinds[0], DeviceKind::Switch3Way);
    assert_eq!(kinds[3], DeviceKind::Switch3Way);
    // A switch with Automatically Change Switch Type When Wiring off keeps its symbol.
    let (mut l2, sw2, light2) = light_with_switches(2);
    let mut o = DeviceOptions::default();
    o.auto_switch_type = false;
    l2.set_options(sw2[1], o);
    connect(&mut l2, sw2[0], light2);
    connect(&mut l2, sw2[1], light2);
    assert_eq!(l2.device(sw2[0]).unwrap().kind, DeviceKind::Switch3Way);
    assert_eq!(l2.device(sw2[1]).unwrap().kind, DeviceKind::Switch);
}

#[test]
fn rope_light_lights_are_spaced_evenly_along_the_path() {
    let p = vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
    // Centered: ten lights, 5" in from each end.
    let c = light_positions(&p, false, 10.0, true);
    assert_eq!(c.len(), 10);
    assert!((c[0].x - 5.0).abs() < 1e-9 && (c[9].x - 95.0).abs() < 1e-9);
    for w in c.windows(2) {
        assert!((w[1].x - w[0].x - 10.0).abs() < 1e-9);
    }
    // From the start: one at the start end, then every 10", the last at the end.
    let s = light_positions(&p, false, 10.0, false);
    assert_eq!(s.len(), 11);
    assert_eq!(s[0], Point::new(0.0, 0.0));
    assert!((s[10].x - 100.0).abs() < 1e-9);
    // A bend does not break the spacing (measured along the path).
    let bent = vec![
        Point::new(0.0, 0.0),
        Point::new(50.0, 0.0),
        Point::new(50.0, 50.0),
    ];
    let b = light_positions(&bent, false, 10.0, false);
    assert_eq!(b.len(), 11);
    assert!(b[5].dist(Point::new(50.0, 0.0)) < 1e-9);
    assert!(b[6].dist(Point::new(50.0, 10.0)) < 1e-9);
    // A path shorter than the spacing still has one light, centered.
    let short = light_positions(&[Point::ZERO, Point::new(4.0, 0.0)], false, 10.0, true);
    assert_eq!(short, vec![Point::new(2.0, 0.0)]);
    // A closed loop shares its length out evenly with no light doubled.
    let square = vec![
        Point::new(0.0, 0.0),
        Point::new(40.0, 0.0),
        Point::new(40.0, 40.0),
        Point::new(0.0, 40.0),
    ];
    let ring = light_positions(&square, true, 10.0, true);
    assert_eq!(ring.len(), 16);
    for (i, a) in ring.iter().enumerate() {
        let n = ring[(i + 1) % ring.len()];
        assert!(a.dist(n) <= 10.0 + 1e-9, "{a:?} {n:?}");
        assert!(a.dist(n) > 1.0);
    }
    // A nonsense spacing is clamped.
    assert!(light_positions(&p, false, 0.0, true).len() <= 200);
}

#[test]
fn rope_lights_are_edited_like_open_polylines() {
    let mut r = RopeLightPath::new(
        vec![Point::new(0.0, 0.0), Point::new(60.0, 0.0)],
        RopeSpec::default(),
    );
    assert_eq!(r.length(), 60.0);
    let v = r.insert_vertex(0, Point::new(30.0, 20.0));
    assert_eq!(v, 1);
    assert_eq!(r.points.len(), 3);
    assert!(r.length() > 60.0);
    assert_eq!(r.vertex_at(Point::new(31.0, 19.0), 3.0), Some(1));
    assert_eq!(r.midpoint_at(Point::new(45.0, 10.0), 3.0), Some(1));
    assert!(r.move_vertex(1, Point::new(30.0, 0.0)));
    assert_eq!(r.length(), 60.0);
    assert!(r.remove_vertex(1) && !r.remove_vertex(1), "two points stay");
    r.translate(Point::new(10.0, 5.0));
    assert_eq!(r.points[0], Point::new(10.0, 5.0));
    // Height measured from the floor or down from the ceiling.
    r.spec.height = 84.0;
    assert_eq!(r.top_elevation(96.0), 84.0);
    r.spec.reference = RopeReference::Ceiling;
    r.spec.height = 6.0;
    assert_eq!(r.top_elevation(96.0), 90.0);
    // A tray ceiling's loop makes a closed rope light at its elevation.
    let loop_ = vec![
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(100.0, 80.0),
        Point::new(0.0, 80.0),
    ];
    let t = RopeLightPath::from_tray_path(7, "Cove", &loop_, 102.0, &RopeSpec::default());
    assert!(t.closed && t.tray == 7);
    assert_eq!(t.top_elevation(120.0), 102.0);
    assert_eq!(t.length(), 360.0);
    assert_eq!(t.lights().len(), 60);
}

#[test]
fn ropes_and_options_live_in_the_layer_and_old_layers_still_read() {
    let mut layer = ElectricalLayer::default();
    let o = layer.add(place_free(DeviceKind::Outlet110, Point::ZERO));
    let rid = layer.add_rope(RopeLightPath::new(
        vec![Point::new(0.0, 0.0), Point::new(50.0, 0.0)],
        RopeSpec::default(),
    ));
    let rid2 = layer.add_rope(RopeLightPath::new(
        vec![Point::new(0.0, 9.0), Point::new(50.0, 9.0)],
        RopeSpec::default(),
    ));
    assert_eq!((rid, rid2), (1, 2));
    let mut opts = DeviceOptions::default();
    opts.recess.distance_from_wall = -2.0;
    layer.set_options(o, opts.clone());
    assert!(layer.rope_at(Point::new(25.0, 1.0), 2.0).is_some());
    assert_eq!(layer.rope_at(Point::new(25.0, 20.0), 2.0), None);
    let json = serde_json::to_string(&layer).unwrap();
    let back: ElectricalLayer = serde_json::from_str(&json).unwrap();
    assert_eq!(back, layer);
    assert_eq!(back.options_of(o), opts);
    // Default options store nothing; removing a device drops its options.
    layer.set_options(o, DeviceOptions::default());
    assert!(layer.options.is_empty());
    layer.set_options(o, opts);
    layer.remove(o);
    assert!(layer.options.is_empty());
    assert!(layer.remove_rope(rid) && !layer.remove_rope(rid));
    // A layer saved before ropes and options (and before the new kinds) reads;
    // the empty record writes neither key.
    let old = r#"{"devices":[],"connections":[{"from":1,"to":2,"arc_bulge":6.0}]}"#;
    let l: ElectricalLayer = serde_json::from_str(old).unwrap();
    assert_eq!(l.connections[0].arc_bulge, 6.0);
    assert!(l.connections[0].is_arc() && l.connections[0].style() == plan_core::LineStyle::Dashed);
    let written = serde_json::to_value(ElectricalLayer::default()).unwrap();
    assert!(written.get("ropes").is_none() && written.get("options").is_none());
    // A rope light record a newer build wrote that this one cannot read is kept.
    let odd = r#"{"devices":[],"connections":[],"ropes":[{"id":9,"points":"nope"}]}"#;
    let l: ElectricalLayer = serde_json::from_str(odd).unwrap();
    assert!(l.ropes.is_empty() && l.unreadable_ropes.len() == 1);
    assert!(serde_json::to_string(&l).unwrap().contains("nope"));
}

#[test]
fn recess_and_size_options_shape_the_plate_in_3d_and_the_plan_symbol() {
    use DeviceKind as K;
    let w = wall(1, (0.0, 0.0), (240.0, 0.0));
    let mut layer = ElectricalLayer::default();
    let a = layer.add(place_on_wall(K::Outlet110, &w, 50.0, WallSide::Left));
    let b = layer.add(place_on_wall(K::Outlet110, &w, 150.0, WallSide::Left));
    let mut o = DeviceOptions::default();
    o.recess.distance_from_wall = -1.0;
    o.set_width(K::Outlet110, 5.5);
    layer.set_options(b, o.clone());
    assert_eq!(o.size(K::Outlet110), (5.5, 9.0), "the height follows");
    let extent = |m: &plan_3d::Mesh, f: fn(&plan_3d::Vertex) -> f32| {
        let v: Vec<f32> = m.vertices.iter().map(f).collect();
        (
            v.iter().cloned().fold(f32::INFINITY, f32::min),
            v.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
        )
    };
    let meshes = meshes(&layer, std::slice::from_ref(&w), 0.0);
    let plate = |id: u64| {
        meshes
            .iter()
            .find(|m| m.object_id == Some(id) && m.material != plan_3d::Material::Asphalt)
            .unwrap()
    };
    let (ax0, ax1) = extent(plate(a), |v| v.position[0]);
    let (bx0, bx1) = extent(plate(b), |v| v.position[0]);
    assert!(
        ((bx1 - bx0) / (ax1 - ax0) - 2.0).abs() < 0.01,
        "twice as wide"
    );
    let (ay0, ay1) = extent(plate(a), |v| v.position[1]);
    let (by0, by1) = extent(plate(b), |v| v.position[1]);
    assert!(
        ((by1 - by0) / (ay1 - ay0) - 2.0).abs() < 0.01,
        "and twice as tall"
    );
    // The recessed plate stands 1" nearer the wall's centerline.
    let (az0, _) = extent(plate(a), |v| v.position[2]);
    let (bz0, _) = extent(plate(b), |v| v.position[2]);
    assert!((az0 - bz0).abs() > 0.5);
    // The plan symbol grows with the width.
    let small = layer.device(a).unwrap().symbol_world();
    let big = layer.device(b).unwrap().symbol_world_with(&o);
    let radius = |s: &[Stroke]| match &s[0] {
        Stroke::Circle { radius, .. } => *radius,
        _ => panic!(),
    };
    assert!((radius(&big) / radius(&small) - 2.0).abs() < 1e-9);
}

#[test]
fn rope_lights_are_modeled_as_strips_along_their_path() {
    let mut layer = ElectricalLayer::default();
    let mut spec = RopeSpec::default();
    spec.height = 60.0;
    layer.add_rope(RopeLightPath::new(
        vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 50.0),
        ],
        spec,
    ));
    let m = meshes(&layer, &[], 0.0);
    assert_eq!(m.len(), 1);
    let ys: Vec<f32> = m[0].vertices.iter().map(|v| v.position[1]).collect();
    let top = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (top - 60.0).abs() < 1e-3,
        "the top of the rope profile is its height"
    );
}

// ----- Auto Place Outlets by the room's electrical rules (manual p. 447) -----

fn rules_for(label: &str, function: &str, type_name: &str) -> Vec<(String, ElectricalRules)> {
    vec![(
        label.to_string(),
        plan_core::rooms::electrical_rules(function, type_name),
    )]
}

fn north_run() -> Vec<Point> {
    vec![
        Point::new(40.0, 117.75),
        Point::new(200.0, 117.75),
        Point::new(200.0, 141.75),
        Point::new(40.0, 141.75),
    ]
}

#[test]
fn exterior_rooms_porches_and_open_below_get_no_outlets_and_hybrids_fewer() {
    let (floor, rooms) = room_20x12();
    let label = rooms[0].label.clone();
    let types = vec![(label.clone(), RoomFunction::Other)];
    let opts = AutoOutletOptions::default();
    let full = auto_place_outlets_by_rules(
        &floor,
        &rooms,
        &types,
        &rules_for(&label, "Standard", "Bedroom"),
        &opts,
    );
    assert_eq!(
        full,
        auto_place_outlets(&floor, &rooms, &types, &opts),
        "a bedroom keeps the plain rules"
    );
    for (f, t) in [
        ("Deck", "Deck"),
        ("Balcony", "Balcony"),
        ("Porch", "Porch"),
        ("Open Below", "Open Below"),
    ] {
        let placed =
            auto_place_outlets_by_rules(&floor, &rooms, &types, &rules_for(&label, f, t), &opts);
        assert!(placed.is_empty(), "{f} gets none");
    }
    // A slab room is a hybrid: the same walls, but half as many outlets.
    let few = auto_place_outlets_by_rules(
        &floor,
        &rooms,
        &types,
        &rules_for(&label, "Slab", "Slab"),
        &opts,
    );
    assert!(
        !few.is_empty() && few.len() < full.len(),
        "{} vs {}",
        few.len(),
        full.len()
    );
}

#[test]
fn a_kitchen_gets_gfci_over_the_base_cabinets_and_standard_outlets_elsewhere() {
    let (floor, rooms) = room_20x12();
    let label = rooms[0].label.clone();
    let types = vec![(label.clone(), RoomFunction::Kitchen)];
    let opts = AutoOutletOptions {
        counter_runs: vec![north_run()],
        ..AutoOutletOptions::default()
    };
    let rules = rules_for(&label, "Standard", "Kitchen");
    let placed = auto_place_outlets_by_rules(&floor, &rooms, &types, &rules, &opts);
    let counter: Vec<_> = placed.iter().filter(|d| d.height == 44.0).collect();
    let standard: Vec<_> = placed.iter().filter(|d| d.height == 12.0).collect();
    assert!(counter.len() >= 4, "{}", counter.len());
    assert!(counter
        .iter()
        .all(|d| d.kind == DeviceKind::Gfci && d.wall_id == Some(3)));
    assert!(!standard.is_empty(), "the free walls take standard outlets");
    assert!(standard.iter().all(|d| d.kind == DeviceKind::Outlet110));
    assert!(standard.iter().any(|d| d.wall_id != Some(3)));
    // Without the rules a kitchen only has the counter outlets.
    let plain = auto_place_outlets(&floor, &rooms, &types, &opts);
    assert!(plain.iter().all(|d| d.height == 44.0));
}

#[test]
fn a_bath_gets_gfci_over_the_vanity_and_no_standard_outlets() {
    let (floor, rooms) = room_20x12();
    let label = rooms[0].label.clone();
    let types = vec![(label.clone(), RoomFunction::Bath)];
    let rules = rules_for(&label, "Standard", "Bath");
    let with_vanity = AutoOutletOptions {
        counter_runs: vec![north_run()],
        ..AutoOutletOptions::default()
    };
    let placed = auto_place_outlets_by_rules(&floor, &rooms, &types, &rules, &with_vanity);
    assert!(!placed.is_empty());
    assert!(placed
        .iter()
        .all(|d| d.kind == DeviceKind::Gfci && d.height == 44.0 && d.wall_id == Some(3)));
    // No vanity drawn: the walls keep their GFCI outlets at the wall height.
    let bare = auto_place_outlets_by_rules(
        &floor,
        &rooms,
        &types,
        &rules,
        &AutoOutletOptions::default(),
    );
    assert!(bare
        .iter()
        .all(|d| d.kind == DeviceKind::Gfci && d.height == 12.0));
}
