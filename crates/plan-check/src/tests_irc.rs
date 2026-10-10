//! Tests of the IRC rules of round 14: the egress door and net clear
//! openings, stair handrails and guards, the shower entrance, the garage
//! separation wall, alarms outside sleeping areas, receptacle wall spaces,
//! rafter spans, footings and the Georgia preset. Each rule has a plan that
//! trips it and a plan that does not.

use super::*;
use plan_core::floors::FoundationOptions;
use plan_core::foundation::{FoundationLayer, Pad, Pier};
use plan_core::{
    Floor, FloorKind, FoundationKind, Opening, OpeningKind, OpeningStyle, PlacedSymbol, RoomName,
    WallKind, WallLayer, WallTypeDef,
};
use plan_stairs::{RailStyle, RailingParams, SideKind, Stair, StairParams, StairShape};
use serde_json::json;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(p: &mut Project, x0: f64, y0: f64, x1: f64, y1: f64, kind: WallKind) -> [u64; 4] {
    let c = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let mut ids = [0; 4];
    for i in 0..4 {
        let (a, b) = (c[i], c[(i + 1) % 4]);
        ids[i] = p.add_wall(0, pt(a.0, a.1), pt(b.0, b.1), 6.0, 109.0, kind);
    }
    ids
}

fn open(
    p: &mut Project,
    wall: u64,
    offset: f64,
    w: f64,
    h: f64,
    sill: f64,
    kind: OpeningKind,
) -> u64 {
    let id = p.alloc_id();
    let mut op = match kind {
        OpeningKind::Door => Opening::default_door(id, wall, offset),
        OpeningKind::Window => Opening::default_window(id, wall, offset),
    };
    (op.width, op.height, op.sill_height) = (w, h, sill);
    p.floors[0].openings.push(op);
    id
}

fn run(p: &Project, types: &[&str]) -> Vec<Finding> {
    let rooms = detect_rooms(&p.floors[0].walls, 1.0);
    let t: Vec<(usize, String)> = types
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.to_string()))
        .collect();
    plan_check(p, 0, &rooms, &t, &[], &CheckOptions::default())
}

fn count(f: &[Finding], rule: &str) -> usize {
    f.iter().filter(|x| x.rule == rule).count()
}

fn has(f: &[Finding], rule: &str) -> bool {
    count(f, rule) > 0
}

fn sev(f: &[Finding], rule: &str) -> Option<Severity> {
    f.iter().find(|x| x.rule == rule).map(|x| x.severity)
}

fn first<'a>(f: &'a [Finding], rule: &str) -> &'a Finding {
    f.iter()
        .find(|x| x.rule == rule)
        .unwrap_or_else(|| panic!("no finding for {rule}"))
}

/// Two 20' x 20' rooms side by side sharing the wall at x = 240; the west room
/// is `west`, the east room `east` (their names set the types). Returns the
/// project and the id of the shared wall.
fn two_rooms(west: &str, east: &str) -> (Project, u64) {
    let mut p = Project::new("t");
    let mut wall = |a: (f64, f64), b: (f64, f64), kind| {
        p.add_wall(0, pt(a.0, a.1), pt(b.0, b.1), 6.0, 109.0, kind)
    };
    wall((0.0, 0.0), (240.0, 0.0), WallKind::Exterior);
    let shared = wall((240.0, 0.0), (240.0, 240.0), WallKind::Interior);
    wall((240.0, 240.0), (0.0, 240.0), WallKind::Exterior);
    wall((0.0, 240.0), (0.0, 0.0), WallKind::Exterior);
    wall((240.0, 0.0), (480.0, 0.0), WallKind::Exterior);
    wall((480.0, 0.0), (480.0, 240.0), WallKind::Exterior);
    wall((480.0, 240.0), (240.0, 240.0), WallKind::Exterior);
    p.floors[0]
        .room_names
        .push(RoomName::new(pt(120.0, 120.0), west, west));
    p.floors[0]
        .room_names
        .push(RoomName::new(pt(360.0, 120.0), east, east));
    (p, shared)
}

// ----- R311.2 egress door -----

#[test]
fn the_grade_floor_needs_a_side_hinged_egress_door() {
    let rule = "IRC R311.2 egress door";
    let build = |w: f64, h: f64, style: OpeningStyle| {
        let mut p = Project::new("t");
        let ids = rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
        let d = open(&mut p, ids[0], 84.0, w, h, 0.0, OpeningKind::Door);
        p.floors[0].openings.last_mut().unwrap().style = style;
        let _ = d;
        run(&p, &["Living Room"])
    };
    assert!(!has(&build(36.0, 80.0, OpeningStyle::Hinged), rule));
    assert!(!has(&build(72.0, 80.0, OpeningStyle::DoubleDoor), rule));
    // 30" leaf is 26" clear; 78" high is 76" clear; a slider is not side-hinged.
    let narrow = build(30.0, 80.0, OpeningStyle::Hinged);
    assert_eq!(sev(&narrow, rule), Some(Severity::Error));
    assert!(matches!(first(&narrow, rule).object, Some(Target::Room(_))));
    assert!(first(&narrow, rule).location.is_some());
    assert!(has(&build(36.0, 78.0, OpeningStyle::Hinged), rule));
    assert!(has(&build(72.0, 80.0, OpeningStyle::Sliding), rule));
}

#[test]
fn no_door_at_all_and_upper_floors_and_unnamed_rooms() {
    let rule = "IRC R311.2 egress door";
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
    assert!(has(&run(&p, &["Living Room"]), rule));
    // Not a habitable room: nothing to leave.
    assert!(!has(&run(&p, &["Closet"]), rule));
    // An upper floor needs no egress door of its own.
    p.floors[0].elevation = 120.0;
    assert!(!has(&run(&p, &["Living Room"]), rule));
}

#[test]
fn a_garage_door_is_not_the_egress_door() {
    let rule = "IRC R311.2 egress door";
    let (mut p, _) = two_rooms("Garage", "Living Room");
    let south_garage = p.floors[0].walls[0].id;
    open(
        &mut p,
        south_garage,
        120.0,
        36.0,
        80.0,
        0.0,
        OpeningKind::Door,
    );
    assert!(has(&run(&p, &[]), rule));
    let south_living = p.floors[0].walls[4].id;
    open(
        &mut p,
        south_living,
        120.0,
        36.0,
        80.0,
        0.0,
        OpeningKind::Door,
    );
    assert!(!has(&run(&p, &[]), rule));
}

// ----- R310.2.1 net clear opening -----

fn bedroom_window(style: OpeningStyle, w: f64, h: f64, sill: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    let ids = rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
    open(&mut p, ids[0], 84.0, w, h, sill, OpeningKind::Window);
    p.floors[0].openings.last_mut().unwrap().style = style;
    run(&p, &["Bedroom"])
}

#[test]
fn a_fixed_window_is_not_an_escape_opening() {
    let rule = "IRC R310.2 egress";
    assert!(has(
        &bedroom_window(OpeningStyle::Fixed, 48.0, 60.0, 24.0),
        rule
    ));
    assert!(!has(
        &bedroom_window(OpeningStyle::Casement, 32.0, 48.0, 30.0),
        rule
    ));
}

#[test]
fn a_sliding_window_gives_half_its_width() {
    let rule = "IRC R310.2 egress";
    // 72" x 48" slides to 36" x 48" net: fine.
    assert!(!has(
        &bedroom_window(OpeningStyle::SlidingWindow, 72.0, 48.0, 30.0),
        rule
    ));
    // 40" x 30" slides to 20" x 30" = 4.2 sq ft: short.
    let f = bedroom_window(OpeningStyle::SlidingWindow, 40.0, 30.0, 30.0);
    let e = first(&f, rule);
    assert_eq!(e.severity, Severity::Error);
    assert!(e.message.contains("net clear"), "{}", e.message);
    assert!(e.message.contains("sq ft"), "{}", e.message);
}

// ----- stairs -----

fn stair(id: u64, params: StairParams) -> Stair {
    Stair::new(id, pt(0.0, 0.0), 0.0, params)
}

fn stair_check(stairs: &[Stair]) -> Vec<Finding> {
    let p = Project::new("t");
    plan_check(&p, 0, &[], &[], stairs, &CheckOptions::default())
}

#[test]
fn a_handrail_is_34_to_38_inches() {
    let rule = "IRC R311.7.8.1 handrail height";
    let tall = |h: f64, handrail: bool| StairParams {
        right_side: SideKind::Railing,
        handrail,
        railing: RailingParams {
            height: h,
            ..RailingParams::default()
        },
        ..StairParams::default()
    };
    assert!(!has(&stair_check(&[stair(1, tall(36.0, false))]), rule));
    assert!(!has(&stair_check(&[stair(1, tall(38.0, false))]), rule));
    let f = stair_check(&[stair(1, tall(42.0, false))]);
    assert_eq!(sev(&f, rule), Some(Severity::Warning));
    assert_eq!(first(&f, rule).object, Some(Target::Stair(1)));
    assert!(first(&f, rule).location.is_some());
    // A separate handrail excuses a taller guard.
    assert!(!has(&stair_check(&[stair(1, tall(42.0, true))]), rule));
    // A side's own rail below 34" is an error.
    let own = StairParams {
        left_side: SideKind::Railing,
        left_railing: Some(RailingParams {
            height: 30.0,
            ..RailingParams::default()
        }),
        ..StairParams::default()
    };
    assert_eq!(
        sev(&stair_check(&[stair(1, own)]), rule),
        Some(Severity::Error)
    );
}

#[test]
fn a_handrail_runs_on_through_the_landing() {
    let rule = "IRC R311.7.8.2 handrail continuity";
    let flight = StairParams {
        right_side: SideKind::Railing,
        ..StairParams::default()
    };
    let landing = |side: SideKind| StairParams {
        shape: StairShape::Landing { depth: 36.0 },
        total_rise: 60.0,
        right_side: side,
        ..StairParams::default()
    };
    let f = stair_check(&[stair(1, flight.clone()), stair(2, landing(SideKind::None))]);
    assert_eq!(sev(&f, rule), Some(Severity::Warning));
    assert_eq!(first(&f, rule).object, Some(Target::Stair(2)));
    assert!(!has(
        &stair_check(&[
            stair(1, flight.clone()),
            stair(2, landing(SideKind::Railing))
        ]),
        rule
    ));
    // A landing far from any flight is not part of one.
    let mut far = stair(2, landing(SideKind::None));
    far.origin = pt(2000.0, 2000.0);
    assert!(!has(&stair_check(&[stair(1, flight), far]), rule));
}

#[test]
fn guard_openings_stop_a_four_inch_sphere() {
    let rule = "IRC R312.1.3 opening limitation";
    let balusters = |spacing: f64| StairParams {
        right_side: SideKind::Railing,
        railing: RailingParams {
            style: RailStyle::Balusters { spacing, size: 1.5 },
            ..RailingParams::default()
        },
        ..StairParams::default()
    };
    assert!(!has(&stair_check(&[stair(1, balusters(4.0))]), rule));
    let f = stair_check(&[stair(1, balusters(5.5))]);
    assert_eq!(sev(&f, rule), Some(Severity::Error));
    assert!(first(&f, rule).message.contains("5.5"));
    // Cables: 4 runs in a 36" guard leave gaps of about 6.9".
    let cables = |rows: u32| StairParams {
        right_side: SideKind::Railing,
        railing: RailingParams {
            style: RailStyle::Cable { rows },
            ..RailingParams::default()
        },
        ..StairParams::default()
    };
    assert!(has(&stair_check(&[stair(1, cables(4))]), rule));
    assert!(!has(&stair_check(&[stair(1, cables(9))]), rule));
    // A half wall is solid.
    let wall = StairParams {
        right_side: SideKind::HalfWall,
        ..StairParams::default()
    };
    assert!(!has(&stair_check(&[stair(1, wall)]), rule));
}

#[test]
fn a_landing_guard_is_36_inches() {
    let rule = "IRC R312.1.2 guard height";
    let landing = |h: f64| StairParams {
        shape: StairShape::Landing { depth: 48.0 },
        total_rise: 60.0,
        left_side: SideKind::Railing,
        railing: RailingParams {
            height: h,
            ..RailingParams::default()
        },
        ..StairParams::default()
    };
    assert!(!has(&stair_check(&[stair(1, landing(36.0))]), rule));
    let f = stair_check(&[stair(1, landing(34.5))]);
    assert_eq!(sev(&f, rule), Some(Severity::Error));
    assert!(first(&f, rule).message.contains("landing"));
}

// ----- R307.1 shower entrance -----

fn shower_in(depth: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 120.0, depth, WallKind::Interior);
    let mut s = PlacedSymbol::new(
        "core.bathkitchen.shower_36",
        pt(60.0, 3.0),
        36.0,
        36.0,
        80.0,
    );
    s.label = "Shower".into();
    p.add_symbol(0, s);
    run(&p, &["Bathroom"])
}

#[test]
fn a_shower_needs_24_inches_in_front() {
    let rule = "IRC R307.1 shower entrance clearance";
    // The shower's front is at y = 39; the north wall's inner face at 57 leaves 18".
    let tight = shower_in(60.0);
    assert_eq!(sev(&tight, rule), Some(Severity::Warning));
    assert!(matches!(
        first(&tight, rule).object,
        Some(Target::Symbol(_))
    ));
    assert!(first(&tight, rule).message.contains("18"));
    assert!(!has(&shower_in(96.0), rule));
}

// ----- R302.6 garage separation -----

fn separation(layers: Vec<WallLayer>, ext_side_right: bool, typed: bool) -> Vec<Finding> {
    let (mut p, shared) = two_rooms("Garage", "Living Room");
    if typed {
        p.register_wall_type(WallTypeDef {
            props: Default::default(),
            name: "Test-4".into(),
            layers,
            kind: WallKind::Interior,
        });
        let w = p.floors[0].wall_mut(shared).unwrap();
        w.wall_type = Some("Test-4".into());
        if ext_side_right {
            w.exterior_side = plan_core::walls::Side::Right;
        }
    }
    run(&p, &[])
}

#[test]
fn the_garage_side_of_the_house_wall_needs_half_inch_gypsum() {
    let rule = "IRC R302.6 garage separation";
    let l = WallLayer::new;
    let good = vec![
        l("Drywall", 0.5, false, "Drywall"),
        l("Framing", 3.5, true, "Fir Framing"),
        l("Drywall", 0.5, false, "Drywall"),
    ];
    // An unspecified wall gets a note.
    let f = separation(vec![], false, false);
    assert_eq!(sev(&f, rule), Some(Severity::Info));
    assert!(matches!(first(&f, rule).object, Some(Target::Wall(_))));
    assert!(first(&f, rule).location.is_some());
    assert_eq!(count(&f, rule), 1);
    assert!(!has(&separation(good.clone(), false, true), rule));
    // The west (garage) side is the wall's left side, which is the first layer.
    let mut thin = good.clone();
    thin[0] = l("Drywall", 0.375, false, "Drywall");
    assert_eq!(
        sev(&separation(thin.clone(), false, true), rule),
        Some(Severity::Error)
    );
    // With the exterior on the right, the garage side is the last layer.
    assert!(!has(&separation(thin.clone(), true, true), rule));
    thin.swap(0, 2);
    assert_eq!(
        sev(&separation(thin, true, true), rule),
        Some(Severity::Error)
    );
    let mut siding = good;
    siding[0] = l("Siding", 0.5, false, "Siding");
    assert_eq!(
        sev(&separation(siding, false, true), rule),
        Some(Severity::Warning)
    );
}

#[test]
fn no_garage_no_separation_finding() {
    let (p, _) = two_rooms("Bedroom", "Living Room");
    assert!(!has(&run(&p, &[]), "IRC R302.6 garage separation"));
}

// ----- R314.3 and R315.3 alarms outside sleeping areas -----

fn wire_at(p: &mut Project, devices: &[(&str, f64, f64)]) {
    let d: Vec<_> = devices
        .iter()
        .map(|(k, x, y)| json!({ "kind": k, "position": { "x": x, "y": y } }))
        .collect();
    p.floors[0].electrical = Some(json!({ "devices": d, "connections": [] }));
}

#[test]
fn a_hall_outside_the_bedrooms_needs_smoke_and_co_alarms() {
    let smoke = "IRC R314.3 smoke alarm outside sleeping area";
    let co = "IRC R315.3 CO alarm outside sleeping area";
    let build = |devices: &[(&str, f64, f64)]| {
        let (mut p, shared) = two_rooms("Bedroom", "Hall");
        open(&mut p, shared, 120.0, 30.0, 80.0, 0.0, OpeningKind::Door);
        wire_at(&mut p, devices);
        run(&p, &[])
    };
    // The bedroom has its own alarm, the hall (x 240..480) has none.
    let f = build(&[("SmokeDetector", 120.0, 120.0)]);
    assert_eq!(sev(&f, smoke), Some(Severity::Warning));
    assert_eq!(sev(&f, co), Some(Severity::Info));
    assert!(matches!(first(&f, smoke).object, Some(Target::Room(_))));
    assert!(first(&f, smoke).location.unwrap().x > 240.0);
    let f = build(&[
        ("SmokeDetector", 360.0, 120.0),
        ("CoDetector", 360.0, 130.0),
    ]);
    assert!(!has(&f, smoke));
    assert!(!has(&f, co));
    // An unwired plan gets no alarm findings.
    let (mut p, shared) = two_rooms("Bedroom", "Hall");
    open(&mut p, shared, 120.0, 30.0, 80.0, 0.0, OpeningKind::Door);
    assert!(!has(&run(&p, &[]), smoke));
}

// ----- E3901.2 receptacle wall spaces -----

fn living_room(devices: &[(&str, f64, f64)]) -> Vec<Finding> {
    let mut p = Project::new("t");
    let ids = rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
    open(&mut p, ids[0], 84.0, 36.0, 80.0, 0.0, OpeningKind::Door);
    wire_at(&mut p, devices);
    run(&p, &["Living Room"])
}

#[test]
fn receptacles_serve_six_feet_of_wall_each_way() {
    let rule = "IRC E3901.2 receptacle spacing";
    // Six receptacles, one or two on each wall space: every point is within 6'.
    let good = [
        ("Outlet110", 33.0, 4.0),
        ("Outlet110", 135.0, 4.0),
        ("Outlet110", 164.0, 72.0),
        ("Outlet110", 42.0, 140.0),
        ("Outlet110", 126.0, 140.0),
        ("Outlet110", 4.0, 72.0),
    ];
    assert!(!has(&living_room(&good), rule));
    // The same count bunched on the west wall leaves the others bare.
    let bunched = [("Outlet110", 4.0, 72.0); 6];
    let f = living_room(&bunched);
    assert_eq!(sev(&f, rule), Some(Severity::Warning));
    assert!(matches!(first(&f, rule).object, Some(Target::Room(_))));
    assert!(first(&f, rule).location.is_some());
    // The count rule is not what fired.
    assert!(!has(&f, "NEC 210.52(A) receptacle spacing"));
}

#[test]
fn a_door_breaks_a_wall_space() {
    let rule = "IRC E3901.2 receptacle spacing";
    // Only the west part of the south wall (x 0..66, door 66..102) has outlets;
    // the east part (102..168) is served by none, though one is 51" away across the door.
    let devices = [
        ("Outlet110", 60.0, 4.0),
        ("Outlet110", 164.0, 72.0),
        ("Outlet110", 42.0, 140.0),
        ("Outlet110", 126.0, 140.0),
        ("Outlet110", 4.0, 72.0),
        ("Outlet110", 4.0, 40.0),
    ];
    assert!(has(&living_room(&devices), rule));
}

// ----- R802.4.1 rafter span -----

fn rafter(depth: f64, length: f64) -> serde_json::Value {
    // Rising 0.6 for every 0.8 of run: the plan projection is 0.8 of the length.
    json!({
        "kind": "Rafter",
        "lumber": { "thickness": 1.5, "depth": depth },
        "length": length,
        "transform": { "origin": [0.0, 100.0, 0.0], "axis_x": [0.8, 0.6, 0.0], "axis_y": [0.0, 1.0, 0.0] },
        "wall_id": null,
        "label": "rafter"
    })
}

#[test]
fn rafters_are_judged_by_their_plan_run() {
    let rule = "IRC R802.4.1 rafter span";
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 240.0, 144.0, WallKind::Exterior);
    // A 2x6 may run 10'-0" in plan: a 150" rafter runs 120", a 160" one 128".
    p.floors[0].framing = vec![rafter(5.5, 150.0), rafter(9.25, 240.0)];
    assert!(!has(&run(&p, &["Living Room"]), rule));
    p.floors[0].framing = vec![rafter(5.5, 160.0), rafter(5.5, 170.0), rafter(9.25, 240.0)];
    let f = run(&p, &["Living Room"]);
    let r = first(&f, rule);
    assert_eq!(r.severity, Severity::Warning);
    assert!(r.message.contains("2 2x6 rafters"), "{}", r.message);
    assert!(r.location.is_some());
    assert_eq!(count(&f, rule), 1);
}

// ----- R403 footings -----

fn house_with_foundation(opts: &FoundationOptions) -> Project {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 240.0, 192.0, WallKind::Exterior);
    p.build_foundation_with(opts);
    p
}

fn foundation_check(p: &Project) -> Vec<Finding> {
    assert_eq!(p.floors[0].kind, FloorKind::Foundation);
    plan_check(p, 0, &[], &[], &[], &CheckOptions::default())
}

#[test]
fn a_stem_wall_footing_reaches_below_the_frost_line() {
    let depth = "IRC R403.1.4 footing depth";
    // Grade is 6" below the floor, frost 12": the underside must be 18" down.
    let deep = house_with_foundation(&FoundationOptions::new(FoundationKind::StemWall {
        height: 24.0,
    }));
    assert!(!has(&foundation_check(&deep), depth));
    // 8" wall + 8" footing bottoms out 16" down: 2" short.
    let shallow = house_with_foundation(&FoundationOptions::new(FoundationKind::StemWall {
        height: 8.0,
    }));
    let f = foundation_check(&shallow);
    let d = first(&f, depth);
    assert_eq!(d.severity, Severity::Warning);
    assert!(matches!(d.object, Some(Target::Wall(_))));
    assert!(d.location.is_some());
    assert!(d.message.contains("frost"), "{}", d.message);
}

#[test]
fn a_taller_frost_depth_moves_the_line() {
    let depth = "IRC R403.1.4 footing depth";
    let p = house_with_foundation(&FoundationOptions::new(FoundationKind::StemWall {
        height: 12.0,
    }));
    // 12" + 8" = 20" down passes at 12" frost, fails at 24".
    assert!(!has(&foundation_check(&p), depth));
    let opts = CheckOptions {
        frost_depth: 24.0,
        ..CheckOptions::default()
    };
    assert!(has(&plan_check(&p, 0, &[], &[], &[], &opts), depth));
}

#[test]
fn footings_are_wide_and_thick_enough_for_the_storeys() {
    let size = "IRC R403.1.1 footing size";
    let mut o = FoundationOptions::new(FoundationKind::StemWall { height: 30.0 });
    let mut p = house_with_foundation(&o);
    assert!(
        !has(&foundation_check(&p), size),
        "16\" is fine for one storey"
    );
    // Two storeys need 15", three need 18".
    p.floors.push(Floor::new("Second", 109.0));
    assert!(!has(&foundation_check(&p), size));
    p.floors.push(Floor::new("Third", 218.0));
    let f = foundation_check(&p);
    assert!(
        first(&f, size).message.contains("18"),
        "{}",
        first(&f, size).message
    );
    // A 5" thick footing.
    o.footing_depth = 5.0;
    let thin = house_with_foundation(&o);
    assert!(first(&foundation_check(&thin), size)
        .message
        .contains("5.0\" thick"));
}

#[test]
fn piers_pads_and_slab_footings_are_checked_too() {
    let depth = "IRC R403.1.4 footing depth";
    let size = "IRC R403.1.1 footing size";
    let mut p = Project::new("t");
    let mut layer = FoundationLayer::default();
    // A deep pier with a footing, a short one without, a thin shallow pad.
    let mut deep = Pier::new(1, pt(0.0, 0.0));
    deep.height = 36.0;
    deep.footing = Some(plan_core::foundation::Footing {
        width: 24.0,
        depth: 12.0,
    });
    let mut short = Pier::new(2, pt(100.0, 0.0));
    short.height = 8.0;
    let mut pad = Pad::new(3, pt(200.0, 0.0));
    pad.elevation = -4.0;
    pad.thickness = 4.0;
    layer.piers = vec![deep, short];
    layer.pads = vec![pad];
    layer.store(&mut p.floors[0]);
    let f = plan_check(&p, 0, &[], &[], &[], &CheckOptions::default());
    let at: Vec<f64> = f
        .iter()
        .filter(|x| x.rule == depth)
        .map(|x| x.location.unwrap().x)
        .collect();
    assert_eq!(at.len(), 2, "{at:?}");
    assert!(at.contains(&100.0) && at.contains(&200.0));
    assert_eq!(count(&f, size), 1, "only the 4\" pad is thin");
    // Each footing finding points at its slab, pad or pier.
    let thin = f.iter().find(|x| x.rule == size).unwrap();
    assert_eq!(thin.object, Some(crate::Target::Foundation(3)));
    let shallow_pier = f
        .iter()
        .find(|x| x.rule == depth && x.location.unwrap().x == 100.0)
        .unwrap();
    assert_eq!(shallow_pier.object, Some(crate::Target::Foundation(2)));
}

// ----- settings -----

#[test]
fn the_georgia_preset_is_a_named_set_of_limits() {
    assert_eq!(
        JURISDICTIONS,
        ["IRC 2021 residential", "Georgia 2020", "Custom"]
    );
    let ga = CheckSettings::preset("Georgia 2020").unwrap();
    assert_eq!(ga.jurisdiction, "Georgia 2020");
    assert_eq!(ga.options.code_year, 2018);
    assert_eq!(ga.options.frost_depth, 12.0);
    assert!(CheckSettings::preset("Custom").is_none());
    assert_eq!(
        CheckSettings::preset("IRC 2021 residential"),
        Some(CheckSettings::irc_2021())
    );
    // It is stored with the plan and comes back.
    let mut p = Project::new("t");
    ga.store(&mut p);
    let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(CheckSettings::load(&back), ga);
    // The name follows the limits.
    let mut s = ga.clone();
    s.options.frost_depth = 18.0;
    s.name_from_limits();
    assert_eq!(s.jurisdiction, "Custom");
    s.options = ga.options.clone();
    s.name_from_limits();
    assert_eq!(s.jurisdiction, "Georgia 2020");
    // Old settings without the new limits still load.
    let old: CheckSettings =
        serde_json::from_str(r#"{"jurisdiction":"Custom","options":{"min_ceiling":80.0}}"#)
            .unwrap();
    assert_eq!(old.options.min_ceiling, 80.0);
    assert_eq!(old.options.frost_depth, 12.0);
}

#[test]
fn rule_groups_switch_on_and_off() {
    let mut s = PlanCheckSettings::default();
    let groups = PlanCheckSettings::groups();
    assert_eq!(groups[0], "Rooms");
    assert!(groups.contains(&"Foundation"));
    let mut seen = std::collections::BTreeSet::new();
    for g in &groups {
        assert!(
            seen.insert(*g),
            "{g} listed twice: groups must be contiguous"
        );
    }
    assert!(s.group_enabled("Foundation"));
    s.set_group_enabled("Foundation", false);
    assert!(!s.group_enabled("Foundation"));
    assert!(!s.group_any_enabled("Foundation"));
    assert!(!s.is_enabled("IRC R403.1.4 footing depth"));
    assert!(s.group_enabled("Rooms"));
    // A group that is off drops its findings from a run.
    let p = house_with_foundation(&FoundationOptions::new(FoundationKind::StemWall {
        height: 8.0,
    }));
    let mut q = p.clone();
    s.store(&mut q);
    let all = foundation_check(&p);
    assert!(has(&all, "IRC R403.1.4 footing depth"));
    let run = filter_findings(&q, 0, all);
    assert!(!has(&run.findings, "IRC R403.1.4 footing depth"));
    assert!(run.switched_off >= 1);
    s.set_group_enabled("Foundation", true);
    assert!(s.group_enabled("Foundation"));
}
