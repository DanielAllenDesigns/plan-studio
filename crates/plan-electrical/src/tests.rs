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
fn kitchen_gets_gfci_counter_outlets_at_42() {
    let (floor, rooms) = room_20x12();
    let types = vec![(rooms[0].label.clone(), RoomFunction::Kitchen)];
    let devices = auto_place_outlets(&floor, &rooms, &types, &AutoOutletOptions::default());
    assert!(!devices.is_empty());
    assert!(devices
        .iter()
        .all(|d| d.kind == DeviceKind::Gfci && d.height == 42.0 && d.wall_id.is_some()));
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
    let (lo, hi) = of(1, plan_3d::Material::WindowFrame).unwrap().bounds().unwrap();
    assert!((hi[0] - lo[0] - 2.75).abs() < 1e-4);
    assert!((hi[1] - lo[1] - 4.5).abs() < 1e-4);
    assert!((hi[2] - lo[2] - 0.25).abs() < 1e-4);
    assert!((lo[1] - 9.75).abs() < 1e-4 && (hi[1] - 14.25).abs() < 1e-4);
    // Scene Z is -plan y: the plate spans plan y 2.25..2.5.
    assert!((lo[2] + 2.5).abs() < 1e-4 && (hi[2] + 2.25).abs() < 1e-4);

    // Ceiling disc: 12" across, hanging from the ceiling.
    let (lo, hi) = of(2, plan_3d::Material::WindowFrame).unwrap().bounds().unwrap();
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
    let sconce = layer.add(place_on_wall(DeviceKind::WallSconce, &w, 150.0, WallSide::Left));
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
    assert_eq!((count_kind(&duplex, is_circle), count_kind(&duplex, is_line)), (1, 2));
    let quad = DeviceKind::Outlet110Quad.symbol();
    assert_eq!((count_kind(&quad, is_circle), count_kind(&quad, is_line)), (1, 4));
    // 220: three slots; GFCI: duplex plus tick marks and its label.
    assert_eq!(count_kind(&DeviceKind::Outlet220.symbol(), is_line), 3);
    assert_eq!(texts(DeviceKind::Gfci), ["GFCI"]);
    // Floor outlet: duplex inside a square.
    let floor = DeviceKind::OutletFloor.symbol();
    assert_eq!((count_kind(&floor, is_circle), count_kind(&floor, is_poly)), (1, 1));
    // Switches: S, S3, S4, SD share the stem and S glyph.
    assert!(texts(DeviceKind::Switch).is_empty());
    assert_eq!(texts(DeviceKind::Switch3Way), ["3"]);
    assert_eq!(texts(DeviceKind::Switch4Way), ["4"]);
    assert_eq!(texts(DeviceKind::SwitchDimmer), ["D"]);
    for k in [DeviceKind::Switch, DeviceKind::Switch3Way, DeviceKind::Switch4Way, DeviceKind::SwitchDimmer] {
        let s = k.symbol();
        assert_eq!((count_kind(&s, is_line), count_kind(&s, is_poly)), (1, 1), "{k:?}");
    }
    // Lights: ceiling = circle and cross, can = ring and dot, pendant likewise.
    let ceiling = DeviceKind::CeilingLight.symbol();
    assert_eq!((count_kind(&ceiling, is_circle), count_kind(&ceiling, is_line)), (1, 2));
    assert_eq!(count_kind(&DeviceKind::RecessedCan.symbol(), is_circle), 2);
    assert_eq!(count_kind(&DeviceKind::PendantLight.symbol(), is_circle), 2);
    assert!(matches!(DeviceKind::WallSconce.symbol()[0], Stroke::Arc { .. }));
    // Fan: hub and four blades.
    let fan = DeviceKind::CeilingFan.symbol();
    assert_eq!((count_kind(&fan, is_circle), count_kind(&fan, is_poly)), (1, 4));
    // Detectors, thermostat, doorbell and the low-voltage jacks carry letters.
    assert_eq!(texts(DeviceKind::SmokeDetector), ["SD"]);
    assert_eq!(texts(DeviceKind::CoDetector), ["CO"]);
    assert_eq!(texts(DeviceKind::Thermostat), ["T"]);
    assert_eq!(texts(DeviceKind::Doorbell), ["DB"]);
    assert_eq!(texts(DeviceKind::DataJack), ["D"]);
    assert_eq!(texts(DeviceKind::PhoneJack), ["T"]);
    assert_eq!(texts(DeviceKind::TvJack), ["TV"]);
    for k in [DeviceKind::DataJack, DeviceKind::PhoneJack, DeviceKind::TvJack] {
        let s = k.symbol();
        assert_eq!((count_kind(&s, is_poly), count_kind(&s, is_text)), (1, 1), "{k:?}");
    }
    assert_eq!(count_kind(&DeviceKind::Panel.symbol(), is_text), 1);
}

#[test]
fn default_heights_per_kind() {
    use DeviceKind as K;
    for k in [K::Outlet110, K::Outlet110Quad, K::Outlet220, K::Gfci, K::DataJack, K::PhoneJack] {
        assert_eq!(k.default_height(), 12.0, "{k:?}");
    }
    for k in [K::Switch, K::Switch3Way, K::Switch4Way, K::SwitchDimmer, K::Doorbell] {
        assert_eq!(k.default_height(), 48.0, "{k:?}");
    }
    assert_eq!(K::OutletFloor.default_height(), 0.0);
    assert_eq!(COUNTER_OUTLET_HEIGHT, 42.0);
    // Every kind except the rope light is in `all` once and wall/ceiling are exclusive.
    for k in DeviceKind::all() {
        assert!(!(k.is_wall_mounted() && k.is_ceiling()), "{k:?}");
    }
    assert_eq!(DeviceKind::all().len(), 23);
}

#[test]
fn a_kind_can_change_within_its_family() {
    use DeviceKind as K;
    let outlets = K::Outlet110.family();
    assert!(outlets.contains(&K::Outlet110Quad) && outlets.contains(&K::Gfci));
    assert!(!outlets.contains(&K::Switch));
    let switches = K::Switch.family();
    assert_eq!(switches, [K::Switch, K::Switch3Way, K::Switch4Way, K::SwitchDimmer]);
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
    let light = layer.add(place_free(DeviceKind::CeilingLight, Point::new(100.0, 80.0)));
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
    assert_eq!(layer.connection_handle_at(after + Point::new(1.0, 1.0), 3.0), Some(0));
    assert_eq!(layer.connection_handle_at(Point::new(-500.0, 500.0), 3.0), None);
    // The stored arc really passes through the handle.
    let Some(Stroke::Arc { center, radius, .. }) = layer.connection_arc(&layer.connections[0]) else {
        panic!("a bent connection is an arc");
    };
    assert!((center.dist(after) - radius).abs() < 1e-6);
    // Bending to the chord straightens it; a bad index is refused.
    assert!(layer.bend_connection(0, mid));
    assert!(matches!(layer.connection_arc(&layer.connections[0]), Some(Stroke::Line { .. })));
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
        let door = floor.openings_on(w.id).find(|o| o.kind == plan_core::OpeningKind::Door);
        let half = 3.25;
        let spaces = match door {
            Some(o) => vec![(half, o.start_offset()), (o.end_offset(), w.length() - half)],
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
            let mut inside: Vec<f64> = here.iter().copied().filter(|t| *t >= a && *t <= b).collect();
            inside.sort_by(f64::total_cmp);
            stops.extend(inside);
            stops.push(b);
            assert!(stops.len() > 2, "wall {} space {a}..{b}", w.id);
            for (i, pair) in stops.windows(2).enumerate() {
                let limit = if i == 0 || i == stops.len() - 2 { 72.0 } else { 144.0 };
                assert!(pair[1] - pair[0] <= limit + 1e-6, "wall {} gap {pair:?}", w.id);
            }
        }
    }
    // Outlets stay a hand clear of the door jambs.
    for d in devices.iter().filter(|d| d.wall_id == Some(1)) {
        let t = offset_on(&floor, d);
        assert!((t - 82.0).abs() >= 6.0 && (t - 118.0).abs() >= 6.0, "at {t}");
    }

    // Kitchen: 42" GFCI counter outlets at most 4' apart on every wall.
    let kitchen = auto_place_outlets(&floor, &rooms, &[(label.clone(), RoomFunction::Kitchen)], &opts);
    assert!(kitchen.iter().all(|d| d.kind == DeviceKind::Gfci && d.height == 42.0));
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
    assert!(bath.iter().all(|d| d.kind == DeviceKind::Gfci && d.height == 12.0));
    assert_eq!(bath.len(), devices.len());
}
