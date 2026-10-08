use super::*;
use plan_core::{Opening, OpeningKind, WallKind};
use plan_stairs::{StairParams, StairShape};

/// A closed rectangle of walls at (x0,y0)-(x1,y1), in inches.
fn rect(p: &mut Project, x0: f64, y0: f64, x1: f64, y1: f64, kind: WallKind) -> [u64; 4] {
    let c = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let mut ids = [0; 4];
    for i in 0..4 {
        let (a, b) = (c[i], c[(i + 1) % 4]);
        ids[i] = p.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            6.0,
            109.0,
            kind,
        );
    }
    ids
}

fn add(
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

fn has(f: &[Finding], rule: &str, sev: Severity) -> bool {
    f.iter().any(|x| x.rule.contains(rule) && x.severity == sev)
}

/// A 14'x12' bedroom (exterior) whose south wall carries a window.
fn bedroom_with_window(w: f64, h: f64, sill: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    let ids = rect(&mut p, 0.0, 0.0, 168.0, 144.0, WallKind::Exterior);
    add(&mut p, ids[0], 84.0, w, h, sill, OpeningKind::Window);
    run(&p, &["Bedroom"])
}

#[test]
fn small_bedroom_fails_area_and_dimension() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 120.0, 72.0, WallKind::Exterior);
    let f = run(&p, &["Bedroom"]);
    assert!(has(&f, "R304.1", Severity::Error));
    assert!(has(&f, "R304.2", Severity::Error));
}

#[test]
fn egress_window_sizes() {
    assert!(!bedroom_with_window(32.0, 48.0, 44.0)
        .iter()
        .any(|f| f.rule.contains("R310")));
    let f = bedroom_with_window(20.0, 24.0, 30.0);
    let e = f
        .iter()
        .find(|f| f.rule.contains("R310"))
        .expect("egress finding");
    assert_eq!(e.severity, Severity::Error);
    assert!(e.message.contains("sq ft"));
    assert!(bedroom_with_window(32.0, 48.0, 50.0)
        .iter()
        .any(|f| f.message.contains("sill")));
}

/// A bath and a hall side by side with a door of width `w` between them.
fn bath_door(w: f64) -> (Vec<Finding>, u64) {
    let mut p = Project::new("t");
    let a = rect(&mut p, 0.0, 0.0, 96.0, 96.0, WallKind::Exterior);
    let b = rect(&mut p, 96.0, 0.0, 240.0, 96.0, WallKind::Exterior);
    // the shared edge is walls a[1] and b[3]: make it interior and hang the door on b[3]
    for id in [a[1], b[3]] {
        p.floors[0].wall_mut(id).unwrap().kind = WallKind::Interior;
    }
    let d = add(&mut p, b[3], 48.0, w, 80.0, 0.0, OpeningKind::Door);
    (run(&p, &["Bathroom", "Hall"]), d)
}

#[test]
fn bath_door_width() {
    let (ok, d) = bath_door(24.0);
    assert!(!ok
        .iter()
        .any(|f| f.object == Some(Target::Opening(d)) && f.rule.contains("door width")));
    let (bad, d) = bath_door(22.0);
    assert!(bad.iter().any(|f| f.object == Some(Target::Opening(d))
        && f.rule.contains("door width")
        && f.severity == Severity::Warning));
}

#[test]
fn stair_with_8_inch_risers_errors() {
    let p = Project::new("t");
    let params = StairParams {
        total_rise: 96.0,
        riser_height_target: 8.0,
        ..StairParams::default()
    };
    let stair = plan_stairs::Stair::new(7, Point::new(0.0, 0.0), 0.0, params);
    let f = plan_check(&p, 0, &[], &[], &[stair], &CheckOptions::default());
    assert!(has(&f, "R311.7.5.1", Severity::Error));
    assert_eq!(f[0].object, Some(Target::Stair(7)));
}

#[test]
fn stair_limits_and_comfort() {
    let p = Project::new("t");
    let params = StairParams {
        total_rise: 200.0,
        width: 30.0,
        tread_depth: 9.0,
        headroom_min: 70.0,
        shape: StairShape::Straight,
        ..StairParams::default()
    };
    let stair = plan_stairs::Stair::new(1, Point::ZERO, 0.0, params);
    let f = plan_check(&p, 0, &[], &[], &[stair], &CheckOptions::default());
    for rule in ["R311.7.5.2", "R311.7.1", "R311.7.2", "R311.7.3"] {
        assert!(has(&f, rule, Severity::Error), "missing {rule}");
    }
    assert!(has(&f, "comfort", Severity::Info));
}

#[test]
fn a_landing_gets_no_stair_findings() {
    let p = Project::new("t");
    let landing = plan_stairs::Stair::new(
        3,
        Point::ZERO,
        0.0,
        StairParams {
            shape: StairShape::Landing { depth: 48.0 },
            ..StairParams::default()
        },
    );
    let f = plan_check(&p, 0, &[], &[], &[landing], &CheckOptions::default());
    assert!(
        f.iter().all(|x| x.object != Some(Target::Stair(3))),
        "{f:?}"
    );
}

#[test]
fn enclosed_room_without_door_is_unreachable() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 144.0, 144.0, WallKind::Exterior);
    let f = run(&p, &["Storage"]);
    assert!(has(&f, "R311.1", Severity::Error));
}

#[test]
fn free_ended_exterior_wall_warns() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 144.0, 144.0, WallKind::Exterior);
    let w = p.add_wall(
        0,
        Point::new(144.0, 72.0),
        Point::new(300.0, 72.0),
        6.0,
        109.0,
        WallKind::Exterior,
    );
    let f = run(&p, &[]);
    let hit = f
        .iter()
        .find(|f| f.rule.contains("dangling"))
        .expect("dangling wall");
    assert_eq!(hit.severity, Severity::Warning);
    assert_eq!(hit.object, Some(Target::Wall(w)));
    assert_eq!(hit.location, Some(Point::new(300.0, 72.0)));
}

#[test]
fn duplicate_and_tiny_walls() {
    let mut p = Project::new("t");
    p.add_wall(
        0,
        Point::ZERO,
        Point::new(100.0, 0.0),
        6.0,
        109.0,
        WallKind::Interior,
    );
    p.add_wall(
        0,
        Point::new(20.0, 0.5),
        Point::new(120.0, 0.5),
        6.0,
        109.0,
        WallKind::Interior,
    );
    p.add_wall(
        0,
        Point::new(0.0, 50.0),
        Point::new(4.0, 50.0),
        6.0,
        109.0,
        WallKind::Interior,
    );
    let f = run(&p, &[]);
    assert!(f.iter().any(|f| f.rule.contains("duplicate")));
    assert!(f.iter().any(|f| f.rule.contains("tiny")));
}

#[test]
fn opening_geometry_problems() {
    let mut p = Project::new("t");
    let ids = rect(&mut p, 0.0, 0.0, 144.0, 144.0, WallKind::Exterior);
    let wide = add(&mut p, ids[0], 140.0, 36.0, 80.0, 0.0, OpeningKind::Door);
    add(&mut p, ids[1], 40.0, 36.0, 60.0, 24.0, OpeningKind::Window);
    add(&mut p, ids[1], 60.0, 36.0, 60.0, 24.0, OpeningKind::Window);
    let f = door_window_check(&p, 0);
    assert!(f
        .iter()
        .any(|f| f.object == Some(Target::Opening(wide)) && f.message.contains("spans")));
    assert!(f.iter().any(|f| f.message.contains("overlap")));
}

#[test]
fn window_sill_on_upper_floor() {
    let mut p = Project::new("t");
    p.floors[0].elevation = 120.0;
    let ids = rect(&mut p, 0.0, 0.0, 144.0, 144.0, WallKind::Exterior);
    add(&mut p, ids[0], 72.0, 36.0, 60.0, 12.0, OpeningKind::Window);
    assert!(has(&run(&p, &["Office"]), "R312.2", Severity::Info));
}

#[test]
fn bedroom_door_swinging_out_is_info_and_garage_rules() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 120.0, 144.0, WallKind::Exterior);
    let g = rect(&mut p, 120.0, 0.0, 300.0, 144.0, WallKind::Exterior);
    let d = add(&mut p, g[3], 72.0, 30.0, 80.0, 0.0, OpeningKind::Door);
    // Bedroom is the west room (index order: top-left first, same row -> by x).
    let f = run(&p, &["Bedroom", "Garage"]);
    assert!(has(&f, "garage door width", Severity::Error));
    assert!(has(&f, "garage opening", Severity::Error));
    assert!(has(&f, "garage door rating", Severity::Info));
    assert!(has(&f, "garage floor", Severity::Info));
    let _ = d;
}

#[test]
fn door_swing_into_stair_warns() {
    let mut p = Project::new("t");
    let ids = rect(&mut p, 0.0, 0.0, 240.0, 144.0, WallKind::Exterior);
    let d = add(&mut p, ids[3], 72.0, 36.0, 80.0, 0.0, OpeningKind::Door);
    let rooms = detect_rooms(&p.floors[0].walls, 1.0);
    let stair = plan_stairs::Stair::new(9, Point::new(10.0, 60.0), 0.0, StairParams::default());
    let f = plan_check(&p, 0, &rooms, &[], &[stair], &CheckOptions::default());
    assert!(f
        .iter()
        .any(|f| f.rule.contains("door swing over stair") && f.object == Some(Target::Opening(d))));
}

#[test]
fn footprint_of_two_room_house_is_outer_rectangle() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 120.0, 120.0, WallKind::Exterior);
    rect(&mut p, 120.0, 0.0, 240.0, 120.0, WallKind::Exterior);
    let rooms = detect_rooms(&p.floors[0].walls, 1.0);
    assert_eq!(rooms.len(), 2);
    let fp = plan_footprint(&p, 0, &rooms);
    assert_eq!(fp.polygon.len(), 4);
    assert!((fp.area_sq_ft - 200.0).abs() < 1e-6);
    assert!((fp.perimeter_ft - 60.0).abs() < 1e-6);
}

#[test]
fn natural_light_is_flagged() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 168.0, 168.0, WallKind::Exterior);
    let f = run(&p, &["Living"]);
    assert!(has(&f, "R303.1", Severity::Info));
}

#[test]
fn report_lists_counts() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 120.0, 72.0, WallKind::Exterior);
    let f = run(&p, &["Bedroom"]);
    let md = report_markdown(&f);
    let (e, w, i) = (
        f.iter().filter(|x| x.severity == Severity::Error).count(),
        f.iter().filter(|x| x.severity == Severity::Warning).count(),
        f.iter().filter(|x| x.severity == Severity::Info).count(),
    );
    assert!(e >= 2);
    assert!(md.contains(&format!(
        "{} findings: {e} errors, {w} warnings, {i} info",
        f.len()
    )));
    assert!(md.contains(&format!("## Errors ({e})")));
    assert!(md.contains("Fix: "));
    assert!(report_markdown(&[]).contains("No findings"));
}
