//! Tests of the code rules in `rules_code.rs` and `rules_fixtures.rs`, the
//! settings and the ignore list. Each rule has a plan that trips it and a
//! plan that does not.

use super::*;
use plan_core::details::{DeckPolygon, DetailsLayer};
use plan_core::{Opening, OpeningKind, OpeningStyle, PlacedSymbol, WallKind};
use plan_stairs::{SideKind, Stair, StairParams, StairShape};
use serde_json::json;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A closed rectangle of 6" walls; returns the ids south, east, north, west.
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

fn run_with(p: &Project, types: &[&str], stairs: &[Stair]) -> Vec<Finding> {
    let rooms = detect_rooms(&p.floors[0].walls, 1.0);
    let t: Vec<(usize, String)> = types
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.to_string()))
        .collect();
    plan_check(p, 0, &rooms, &t, stairs, &CheckOptions::default())
}

fn run(p: &Project, types: &[&str]) -> Vec<Finding> {
    run_with(p, types, &[])
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

// ----- rooms -----

#[test]
fn ceiling_height_is_checked_per_room_type() {
    let build = |h: f64, room_type: &str| {
        let mut p = Project::new("t");
        rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
        p.floors[0].ceiling_height = h;
        run(&p, &[room_type])
    };
    assert_eq!(
        sev(&build(80.0, "Bedroom"), "IRC R305.1 ceiling height"),
        Some(Severity::Error)
    );
    assert!(!has(&build(96.0, "Bedroom"), "IRC R305.1 ceiling height"));
    // A bathroom may be 6'-8", but not lower.
    assert!(!has(&build(80.0, "Bathroom"), "IRC R305.1 ceiling height"));
    assert!(has(&build(76.0, "Bathroom"), "IRC R305.1 ceiling height"));
}

#[test]
fn a_room_ceiling_override_beats_the_floor_height() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
    let mut name = plan_core::RoomName::new(pt(80.0, 70.0), "Den", "Den");
    name.ceiling_height = Some(78.0);
    p.floors[0].room_names.push(name);
    assert!(has(&run(&p, &[]), "IRC R305.1 ceiling height"));
}

// ----- stairs -----

fn stair(params: StairParams) -> Stair {
    Stair::new(3, pt(0.0, 0.0), 0.0, params)
}

#[test]
fn handrail_is_required_from_four_risers() {
    let p = Project::new("t");
    let check = |params: StairParams| {
        plan_check(&p, 0, &[], &[], &[stair(params)], &CheckOptions::default())
    };
    let tall = StairParams::default();
    let f = check(tall.clone());
    assert_eq!(sev(&f, "IRC R311.7.8 handrails"), Some(Severity::Warning));
    assert_eq!(
        f.iter()
            .find(|f| f.rule == "IRC R311.7.8 handrails")
            .unwrap()
            .object,
        Some(Target::Stair(3))
    );
    assert!(!has(
        &check(StairParams {
            handrail: true,
            ..tall.clone()
        }),
        "IRC R311.7.8 handrails"
    ));
    assert!(!has(
        &check(StairParams {
            right_side: SideKind::Railing,
            ..tall.clone()
        }),
        "IRC R311.7.8 handrails"
    ));
    // A wall alone is not a handrail.
    assert!(has(
        &check(StairParams {
            left_side: SideKind::Wall,
            ..tall.clone()
        }),
        "IRC R311.7.8 handrails"
    ));
    // A Handrail side is one, and it is not a guard (no guard-height finding).
    let gripped = check(StairParams {
        left_side: SideKind::Handrail,
        ..tall.clone()
    });
    assert!(!has(&gripped, "IRC R311.7.8 handrails"));
    assert!(!has(&gripped, "IRC R312.1.2 guard height"));
    // Three risers need none.
    let short = StairParams {
        total_rise: 22.0,
        ..tall
    };
    assert!(!has(&check(short), "IRC R311.7.8 handrails"));
}

#[test]
fn stair_railing_height() {
    let p = Project::new("t");
    let with_height = |h: f64| {
        let mut params = StairParams {
            right_side: SideKind::Railing,
            ..StairParams::default()
        };
        params.railing.height = h;
        plan_check(&p, 0, &[], &[], &[stair(params)], &CheckOptions::default())
    };
    assert_eq!(
        sev(&with_height(30.0), "IRC R312.1.2 guard height"),
        Some(Severity::Error)
    );
    assert!(!has(&with_height(34.0), "IRC R312.1.2 guard height"));
    assert!(!has(&with_height(36.0), "IRC R312.1.2 guard height"));
}

#[test]
fn a_landing_gets_no_handrail_finding() {
    let p = Project::new("t");
    let landing = stair(StairParams {
        shape: StairShape::Landing { depth: 48.0 },
        ..StairParams::default()
    });
    let f = plan_check(&p, 0, &[], &[], &[landing], &CheckOptions::default());
    assert!(!has(&f, "IRC R311.7.8 handrails"));
}

#[test]
fn a_spiral_stair_is_judged_by_the_spiral_code() {
    let p = Project::new("t");
    let check = |params: StairParams| {
        plan_check(&p, 0, &[], &[], &[stair(params)], &CheckOptions::default())
    };
    let spiral = |width: f64, tread: f64, headroom: f64| StairParams {
        shape: StairShape::Curved { inner_radius: 2.0 },
        spiral: true,
        width,
        tread_depth: tread,
        headroom_min: headroom,
        total_rise: 109.125,
        ..StairParams::default()
    };
    let rule = "IRC R311.7.10.1 spiral stairways";
    // 28" wide, 8 1/2" treads, 80" headroom: fine as a spiral, though the
    // straight-stair width, tread and 2R+T rules would all complain.
    let ok = check(spiral(28.0, 8.5, 80.0));
    assert!(!has(&ok, rule), "{ok:?}");
    for straight in [
        "IRC R311.7.5.2 tread depth",
        "IRC R311.7.1 stair width",
        "IRC R311.7.5 stair comfort (2R+T)",
    ] {
        assert!(!has(&ok, straight), "{straight}");
    }
    // The same numbers on an ordinary curved stair trip them.
    let plain = check(StairParams {
        spiral: false,
        ..spiral(28.0, 8.5, 80.0)
    });
    assert_eq!(
        sev(&plain, "IRC R311.7.5.2 tread depth"),
        Some(Severity::Error)
    );
    assert_eq!(
        sev(&plain, "IRC R311.7.1 stair width"),
        Some(Severity::Error)
    );
    // Too narrow, too shallow, too low a ceiling, too tall a riser.
    assert_eq!(
        sev(&check(spiral(24.0, 8.5, 80.0)), rule),
        Some(Severity::Error)
    );
    assert_eq!(
        sev(&check(spiral(28.0, 6.0, 80.0)), rule),
        Some(Severity::Error)
    );
    assert_eq!(
        sev(&check(spiral(28.0, 8.5, 76.0)), rule),
        Some(Severity::Error)
    );
    let tall = StairParams {
        riser_height_target: 11.0,
        total_rise: 120.0,
        ..spiral(28.0, 8.5, 80.0)
    };
    // The solver adds risers to stay within 9 1/2"; the target alone is an error.
    assert_eq!(sev(&check(tall), rule), Some(Severity::Error));
}

// ----- decks, exterior doors -----

fn put_deck(p: &mut Project, outline: Vec<Point>, elevation: f64, railing: bool) {
    let deck = DeckPolygon {
        id: 77,
        outline,
        elevation,
        railing,
        ..DeckPolygon::default()
    };
    let layer = DetailsLayer {
        decks: vec![deck],
        ..DetailsLayer::default()
    };
    p.floors[0].set_details(&layer).unwrap();
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)]
}

#[test]
fn a_tall_deck_needs_a_guard() {
    let build = |elev: f64, railing: bool| {
        let mut p = Project::new("t");
        put_deck(&mut p, square(200.0, 0.0, 300.0, 100.0), elev, railing);
        run(&p, &[])
    };
    let f = build(40.0, false);
    assert_eq!(sev(&f, "IRC R312.1.1 guards"), Some(Severity::Error));
    assert_eq!(
        f.iter()
            .find(|f| f.rule == "IRC R312.1.1 guards")
            .unwrap()
            .object,
        Some(Target::Detail(77))
    );
    assert!(!has(&build(40.0, true), "IRC R312.1.1 guards"));
    assert!(!has(&build(24.0, false), "IRC R312.1.1 guards"));
}

/// A 14'x12' room whose south wall carries an exterior door of height `h`.
fn room_with_door(h: f64) -> (Project, u64) {
    let mut p = Project::new("t");
    let ids = rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
    open(&mut p, ids[0], 84.0, 36.0, h, 0.0, OpeningKind::Door);
    (p, ids[0])
}

#[test]
fn exterior_door_height() {
    let (short, _) = room_with_door(78.0);
    assert_eq!(
        sev(
            &run(&short, &["Living Room"]),
            "IRC R311.2 egress door height"
        ),
        Some(Severity::Warning)
    );
    let (full, _) = room_with_door(80.0);
    assert!(!has(
        &run(&full, &["Living Room"]),
        "IRC R311.2 egress door height"
    ));
}

#[test]
fn an_upper_floor_door_needs_a_deck_outside() {
    let (mut p, _) = room_with_door(80.0);
    p.floors[0].elevation = 120.0;
    assert_eq!(
        sev(&run(&p, &["Bedroom"]), "IRC R312.1.1 door to a drop"),
        Some(Severity::Error)
    );
    // A deck under the door's landing area clears it.
    put_deck(&mut p, square(40.0, -80.0, 130.0, 0.0), 120.0, true);
    assert!(!has(&run(&p, &["Bedroom"]), "IRC R312.1.1 door to a drop"));
    // On the grade floor there is nothing to ask for.
    let (grade, _) = room_with_door(80.0);
    assert!(!has(
        &run(&grade, &["Bedroom"]),
        "IRC R312.1.1 door to a drop"
    ));
}

#[test]
fn a_stair_straight_outside_a_door_needs_a_landing() {
    let (p, _) = room_with_door(80.0);
    // Facing south, away from the door, starting at its outer face.
    let steps = Stair::new(
        9,
        pt(102.0, -4.0),
        -std::f64::consts::FRAC_PI_2,
        StairParams {
            total_rise: 30.0,
            handrail: true,
            ..StairParams::default()
        },
    );
    let f = run_with(&p, &["Living Room"], std::slice::from_ref(&steps));
    assert_eq!(
        sev(&f, "IRC R311.3 landing at door"),
        Some(Severity::Warning)
    );
    // A landing outside the door, then the stair beyond it.
    let landing = Stair::new(
        10,
        pt(102.0, -4.0),
        -std::f64::consts::FRAC_PI_2,
        StairParams {
            shape: StairShape::Landing { depth: 40.0 },
            ..StairParams::default()
        },
    );
    let f = run_with(&p, &["Living Room"], &[steps, landing]);
    assert!(!has(&f, "IRC R311.3 landing at door"));
    // A stair far from the door.
    let far = Stair::new(
        11,
        pt(500.0, 500.0),
        0.0,
        StairParams {
            handrail: true,
            ..StairParams::default()
        },
    );
    assert!(!has(
        &run_with(&p, &["Living Room"], &[far]),
        "IRC R311.3 landing at door"
    ));
}

// ----- ventilation area -----

#[test]
fn openable_glazing_is_four_percent_of_the_floor() {
    let build = |style: OpeningStyle| {
        let mut p = Project::new("t");
        let ids = rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
        let w = open(&mut p, ids[0], 84.0, 36.0, 48.0, 30.0, OpeningKind::Window);
        p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == w)
            .unwrap()
            .style = style;
        run(&p, &["Bedroom"])
    };
    // 12 sq ft of glass in a 168 sq ft room: a fixed unit opens nothing, a
    // double hung opens half (6 sq ft, under the 6.7 needed), a casement opens it all.
    assert_eq!(
        sev(&build(OpeningStyle::Fixed), "IRC R303.1 ventilation area"),
        Some(Severity::Warning)
    );
    assert!(has(
        &build(OpeningStyle::Window),
        "IRC R303.1 ventilation area"
    ));
    assert!(!has(
        &build(OpeningStyle::Casement),
        "IRC R303.1 ventilation area"
    ));
}

// ----- smoke alarms -----

fn wire(p: &mut Project, kinds: &[&str]) {
    let devices: Vec<_> = kinds
        .iter()
        .map(|k| json!({ "kind": k, "position": { "x": 80.0, "y": 70.0 } }))
        .collect();
    p.floors[0].electrical = Some(json!({ "devices": devices, "connections": [] }));
}

#[test]
fn every_wired_level_needs_a_smoke_alarm() {
    let build = |kinds: &[&str]| {
        let mut p = Project::new("t");
        rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
        if !kinds.is_empty() {
            wire(&mut p, kinds);
        }
        run(&p, &["Living Room"])
    };
    let rule = "IRC R314.3 smoke alarm on every level";
    assert_eq!(sev(&build(&["Outlet110"]), rule), Some(Severity::Warning));
    assert!(!has(&build(&["Outlet110", "SmokeDetector"]), rule));
    // A plan that has not been wired yet gets no electrical findings.
    assert!(!has(&build(&[]), rule));
}

// ----- roof pitch -----

#[test]
fn roof_pitch_warnings() {
    let build = |pitch: f64| {
        let mut p = Project::new("t");
        p.floors[0].roofs = vec![json!({
            "kind": "plane", "id": 5, "pitch": pitch,
            "baseline": [{ "x": 0.0, "y": 0.0 }, { "x": 100.0, "y": 0.0 }],
        })];
        run(&p, &[])
    };
    let f = build(1.0);
    assert_eq!(sev(&f, "IRC R905.2.2 roof slope"), Some(Severity::Warning));
    assert_eq!(
        f.iter()
            .find(|f| f.rule == "IRC R905.2.2 roof slope")
            .unwrap()
            .object,
        Some(Target::Roof(5))
    );
    assert_eq!(
        sev(&build(3.0), "IRC R905.1.1 underlayment"),
        Some(Severity::Info)
    );
    assert!(!has(&build(3.0), "IRC R905.2.2 roof slope"));
    let ok = build(6.0);
    assert!(!has(&ok, "IRC R905.2.2 roof slope") && !has(&ok, "IRC R905.1.1 underlayment"));
    assert!(!has(&ok, "Roof pitch: steep slope"));
    assert_eq!(
        sev(&build(14.0), "Roof pitch: steep slope"),
        Some(Severity::Info)
    );
}

// ----- fixtures -----

fn toilet(p: &mut Project, x: f64, y: f64) -> u64 {
    let mut s = PlacedSymbol::new(
        "core.bathkitchen.toilet_elongated",
        pt(x, y),
        20.0,
        29.0,
        30.0,
    );
    s.label = "Toilet".into();
    p.add_symbol(0, s)
}

/// A bathroom `w` wide and `d` deep with the toilet's back on the south wall
/// (whose inner face is at y = 3) at `x`.
fn bath_with_toilet(w: f64, d: f64, x: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, w, d, WallKind::Interior);
    toilet(&mut p, x, 3.0);
    run(&p, &["Bathroom"])
}

#[test]
fn a_toilet_needs_fifteen_inches_each_side() {
    let rule = "IRC R307.1 water closet clearance";
    // 12" from the centerline to the west wall's face.
    let f = bath_with_toilet(120.0, 96.0, 15.0);
    assert_eq!(sev(&f, rule), Some(Severity::Warning));
    assert!(f
        .iter()
        .find(|f| f.rule == rule)
        .unwrap()
        .message
        .contains("left"));
    assert!(matches!(
        f.iter().find(|f| f.rule == rule).unwrap().object,
        Some(Target::Symbol(_))
    ));
    // Exactly 15" is allowed; so is the middle of the room.
    assert!(!has(&bath_with_toilet(120.0, 96.0, 18.0), rule));
    assert!(!has(&bath_with_toilet(120.0, 96.0, 60.0), rule));
}

#[test]
fn a_toilet_needs_twenty_one_inches_in_front() {
    let rule = "IRC R307.1 water closet clearance";
    // The toilet's front is at y = 32; the north wall's inner face at 47 leaves 15".
    let tight = bath_with_toilet(120.0, 50.0, 60.0);
    assert!(tight
        .iter()
        .find(|f| f.rule == rule)
        .unwrap()
        .message
        .contains("in front"));
    // 96" deep leaves plenty.
    assert!(!has(&bath_with_toilet(120.0, 96.0, 60.0), rule));
}

#[test]
fn another_fixture_counts_as_an_obstruction() {
    let rule = "IRC R307.1 water closet clearance";
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 96.0, WallKind::Interior);
    toilet(&mut p, 60.0, 3.0);
    assert!(!has(&run(&p, &["Bathroom"]), rule));
    // A tub 8" beside the toilet's centerline.
    let tub = PlacedSymbol::new(
        "core.bathkitchen.tub_alcove_60x30_left",
        pt(78.0, 3.0),
        60.0,
        30.0,
        20.0,
    );
    p.add_symbol(0, tub);
    assert!(has(&run(&p, &["Bathroom"]), rule));
}

#[test]
fn showers_and_tubs_are_at_least_thirty_inches() {
    let rule = "IRC P2708.1 shower and tub size";
    let build = |id: &str, w: f64, d: f64| {
        let mut p = Project::new("t");
        rect(&mut p, 0.0, 0.0, 200.0, 120.0, WallKind::Interior);
        p.add_symbol(0, PlacedSymbol::new(id, pt(100.0, 3.0), w, d, 80.0));
        run(&p, &["Bathroom"])
    };
    assert_eq!(
        sev(&build("core.bathkitchen.shower_36x36", 28.0, 36.0), rule),
        Some(Severity::Warning)
    );
    // 30" x 30" is 900 sq in: allowed. 36" x 36" is too.
    assert!(!has(
        &build("core.bathkitchen.shower_36x36", 30.0, 30.0),
        rule
    ));
    assert!(!has(
        &build("core.bathkitchen.shower_36x36", 36.0, 36.0),
        rule
    ));
    // A 26" wide tub fails; the usual 60" x 30" passes.
    assert!(has(
        &build("core.bathkitchen.tub_alcove_60x30_left", 60.0, 26.0),
        rule
    ));
    assert!(!has(
        &build("core.bathkitchen.tub_alcove_60x30_left", 60.0, 30.0),
        rule
    ));
}

/// A 96" square bath beside a hall; the door is on the wall they share.
fn bath_door_with_toilet_at(x: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    let a = rect(&mut p, 0.0, 0.0, 96.0, 96.0, WallKind::Exterior);
    let b = rect(&mut p, 96.0, 0.0, 240.0, 96.0, WallKind::Exterior);
    for id in [a[1], b[3]] {
        p.floors[0].wall_mut(id).unwrap().kind = WallKind::Interior;
    }
    let d = open(&mut p, b[3], 48.0, 30.0, 80.0, 0.0, OpeningKind::Door);
    // Swing into the bath.
    p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == d)
        .unwrap()
        .swing_flipped = true;
    toilet(&mut p, x, 48.0);
    run(&p, &["Bathroom", "Hall"])
}

#[test]
fn a_door_must_not_swing_into_a_fixture() {
    let rule = "IRC R307.1 door swing into fixture";
    let f = bath_door_with_toilet_at(80.0);
    assert_eq!(sev(&f, rule), Some(Severity::Warning));
    assert!(matches!(
        f.iter().find(|f| f.rule == rule).unwrap().object,
        Some(Target::Opening(_))
    ));
    assert!(!has(&bath_door_with_toilet_at(30.0), rule));
}

// ----- the kitchen -----

/// A cabinet of `kind` with its back-left corner at `at`, turned `angle`
/// radians, `size` wide and deep, with a front countertop overhang.
fn cab(
    id: u64,
    kind: &str,
    at: (f64, f64),
    angle: f64,
    size: (f64, f64),
    overhang: f64,
) -> serde_json::Value {
    let ((x, y), (w, d)) = (at, size);
    json!({
        "id": id, "kind": kind, "position": { "x": x, "y": y }, "angle": angle,
        "width": w, "depth": d, "height": 36.0, "elevation": 0.0,
        "countertop": { "thickness": 1.5, "overhang_front": overhang, "overhang_sides": 0.0, "overhang_back": 0.0 },
    })
}

/// A 200" x 150" kitchen with a 96" run on the south wall and an island whose
/// front is `aisle` inches from the run's front.
fn kitchen_with_island(aisle: f64, run_depth: f64, overhang: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 150.0, WallKind::Exterior);
    let front = 3.0 + run_depth;
    // The island is turned half way round: its front faces south.
    let back = front + aisle + 24.0;
    p.floors[0].cabinets = vec![
        cab(1, "Base", (10.0, 3.0), 0.0, (96.0, run_depth), overhang),
        cab(
            2,
            "Base",
            (106.0, back),
            std::f64::consts::PI,
            (96.0, 24.0),
            1.0,
        ),
    ];
    run(&p, &["Kitchen"])
}

#[test]
fn kitchen_aisles() {
    let rule = "NKBA kitchen aisle width";
    let f = kitchen_with_island(30.0, 24.0, 1.0);
    assert_eq!(sev(&f, rule), Some(Severity::Warning));
    assert!(f
        .iter()
        .find(|f| f.rule == rule)
        .unwrap()
        .message
        .contains("30\""));
    assert!(matches!(
        f.iter().find(|f| f.rule == rule).unwrap().object,
        Some(Target::Cabinet(_))
    ));
    assert_eq!(count(&f, rule), 1, "one finding per kitchen");
    assert_eq!(
        sev(&kitchen_with_island(40.0, 24.0, 1.0), rule),
        Some(Severity::Info)
    );
    assert!(!has(&kitchen_with_island(48.0, 24.0, 1.0), rule));
}

#[test]
fn a_doorway_is_not_a_wall_to_the_aisle() {
    let rule = "NKBA kitchen aisle width";
    let build = |door: bool| {
        let mut p = Project::new("t");
        let ids = rect(&mut p, 0.0, 0.0, 200.0, 150.0, WallKind::Exterior);
        // A 30" cabinet facing the north wall: its front is at y = 114, the
        // wall's inner face at 147, so the aisle is 33".
        p.floors[0].cabinets = vec![cab(1, "Base", (60.0, 90.0), 0.0, (30.0, 24.0), 1.0)];
        if door {
            // The north wall (ids[2]) runs east to west, so offsets count from x = 200.
            open(&mut p, ids[2], 125.0, 40.0, 80.0, 0.0, OpeningKind::Door);
        }
        run(&p, &["Kitchen"])
    };
    assert_eq!(sev(&build(false), rule), Some(Severity::Warning));
    assert!(
        !has(&build(true), rule),
        "the doorway leads out, it is not a wall"
    );
}

#[test]
fn counters_are_at_least_twenty_four_inches_deep() {
    let rule = "NKBA kitchen counter depth";
    // 21" cabinet with a 1" overhang: 22".
    assert_eq!(
        sev(&kitchen_with_island(60.0, 21.0, 1.0), rule),
        Some(Severity::Info)
    );
    assert!(!has(&kitchen_with_island(60.0, 24.0, 1.0), rule));
}

// ----- settings, ignore list, report -----

fn plan_with_findings() -> (Project, Vec<Room>) {
    let mut p = Project::new("t");
    let ids = rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
    open(&mut p, ids[0], 84.0, 20.0, 24.0, 60.0, OpeningKind::Window);
    p.floors[0].ceiling_height = 76.0;
    let rooms = detect_rooms(&p.floors[0].walls, 1.0);
    (p, rooms)
}

fn run_settings(p: &Project, rooms: &[Room]) -> CheckRun {
    run_plan_check(p, 0, rooms, &[(0, "Bedroom".to_string())], &[])
}

#[test]
fn settings_round_trip_through_the_project_file() {
    let mut p = Project::new("t");
    assert_eq!(CheckSettings::load(&p), CheckSettings::irc_2021());
    assert!(p.info.custom.is_empty(), "defaults store nothing");
    let mut s = CheckSettings::irc_2021();
    s.set_enabled("IRC R305.1 ceiling height", false);
    s.options.min_hall_width = 42.0;
    s.name_from_limits();
    assert_eq!(s.jurisdiction, "Custom");
    s.store(&mut p);
    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    let loaded = CheckSettings::load(&back);
    assert_eq!(loaded, s);
    assert!(!loaded.is_enabled("IRC R305.1 ceiling height"));
    assert!(loaded.is_enabled("IRC R304.1 minimum room area"));
    assert_eq!(loaded.options.min_hall_width, 42.0);
    // Back to the preset removes the entry again.
    CheckSettings::irc_2021().store(&mut p);
    assert!(p.info.custom.is_empty());
    let mut again = s.clone();
    again.options = CheckOptions::default();
    again.name_from_limits();
    assert_eq!(again.jurisdiction, "IRC 2021 residential");
}

#[test]
fn switching_a_rule_off_drops_its_findings() {
    let (mut p, rooms) = plan_with_findings();
    let before = run_settings(&p, &rooms);
    assert!(before
        .findings
        .iter()
        .any(|f| f.rule == "IRC R305.1 ceiling height"));
    let mut s = CheckSettings::irc_2021();
    s.set_enabled("IRC R305.1 ceiling height", false);
    s.store(&mut p);
    let after = run_settings(&p, &rooms);
    assert!(!after
        .findings
        .iter()
        .any(|f| f.rule == "IRC R305.1 ceiling height"));
    assert_eq!(after.switched_off, 1);
    assert_eq!(after.findings.len() + 1, before.findings.len());
    // A raised limit changes what is reported.
    let mut tall = CheckSettings::irc_2021();
    tall.options.min_ceiling = 70.0;
    tall.store(&mut p);
    assert!(!run_settings(&p, &rooms)
        .findings
        .iter()
        .any(|f| f.rule == "IRC R305.1 ceiling height"));
}

#[test]
fn ignored_findings_persist_and_come_back_after_a_reset() {
    let (mut p, rooms) = plan_with_findings();
    let run1 = run_settings(&p, &rooms);
    let first = run1.findings[0].clone();
    let total = run1.findings.len();
    let mut keys = ignored_keys(&p);
    assert!(keys.is_empty());
    keys.insert(finding_key(0, &first));
    set_ignored_keys(&mut p, &keys);
    // Through the file and back.
    let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(ignored_keys(&back), keys);
    let run2 = run_settings(&back, &rooms);
    assert_eq!(run2.findings.len(), total - 1);
    assert_eq!(run2.ignored, vec![first.clone()]);
    assert!(
        run2.summary().ends_with("(1 ignored)"),
        "{}",
        run2.summary()
    );
    // The same finding on another floor is another key.
    assert_ne!(finding_key(0, &first), finding_key(1, &first));
    // Clearing the list brings it back.
    set_ignored_keys(&mut p, &Default::default());
    assert!(p.info.custom.iter().all(|(k, _)| k != "plancheck.ignored"));
    assert_eq!(run_settings(&p, &rooms).findings.len(), total);
}

#[test]
fn summary_counts_by_severity() {
    let (p, rooms) = plan_with_findings();
    let run = run_settings(&p, &rooms);
    let (e, w, i) = run.counts();
    assert_eq!(e + w + i, run.findings.len());
    assert!(e > 0);
    let s = run.summary();
    assert!(s.starts_with("Plan Check: "), "{s}");
    assert!(s.contains(&format!("{i} info")), "{s}");
    assert_eq!(CheckRun::default().summary(), "Plan Check: no findings");
}

#[test]
fn report_table_has_one_row_per_finding() {
    let (p, rooms) = plan_with_findings();
    let run = run_settings(&p, &rooms);
    let t = report_table("1st Floor", &run.findings);
    assert_eq!(t.title, "Plan Check - 1st Floor");
    assert_eq!(t.rows.len(), run.findings.len());
    assert!(t.rows.iter().all(|r| r.len() == t.columns.len()));
    assert_eq!(t.rows[0][0], "1");
    assert_eq!(t.rows[0][1], run.findings[0].severity.singular());
    assert_eq!(t.rows[0][2], run.findings[0].rule);
    assert!(report_table("x", &[]).rows.is_empty());
}

// ----- the rule catalog -----

#[test]
fn the_catalog_lists_every_rule_once_and_only_real_ones() {
    let catalog = rule_catalog();
    let mut seen = std::collections::BTreeSet::new();
    for r in catalog {
        assert!(seen.insert(r.id), "{} listed twice", r.id);
        assert!(!r.summary.is_empty());
    }
    let sources = [
        include_str!("rules.rs"),
        include_str!("rules_mep.rs"),
        include_str!("rules_code.rs"),
        include_str!("rules_fixtures.rs"),
        include_str!("rules_irc.rs"),
        include_str!("rules_nkba.rs"),
    ];
    for r in catalog {
        let quoted = format!("\"{}\"", r.id);
        assert!(
            sources.iter().any(|s| s.contains(&quoted)),
            "no rule emits {}",
            r.id
        );
    }
}

#[test]
fn every_rule_of_the_new_files_is_in_the_catalog() {
    for src in [
        include_str!("rules_code.rs"),
        include_str!("rules_fixtures.rs"),
        include_str!("rules_irc.rs"),
        include_str!("rules_mep.rs"),
        include_str!("rules_nkba.rs"),
        include_str!("rules.rs"),
    ] {
        for lit in src.split('"').skip(1).step_by(2) {
            let looks_like_rule = ["IRC ", "NEC ", "NKBA ", "Roof pitch: ", "Plan geometry: "]
                .iter()
                .any(|p| lit.starts_with(p));
            if looks_like_rule && !lit.contains('{') {
                assert!(
                    rule_catalog().iter().any(|r| r.id == lit),
                    "rule {lit:?} is missing from the catalog"
                );
            }
        }
    }
}
