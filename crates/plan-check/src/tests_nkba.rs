//! Tests of the NKBA kitchen and bathroom guidelines (`rules_nkba.rs`). A
//! kitchen and a bathroom that meet every guideline are built once; each test
//! changes one thing and expects exactly that guideline to fire.

use super::*;
use crate::rules_nkba::{self as nkba, GUIDELINES};
use plan_core::{Opening, OpeningKind, OpeningStyle, PlacedSymbol, WallKind};
use serde_json::{json, Value};

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

fn opening_mut(p: &mut Project, id: u64) -> &mut Opening {
    p.floors[0]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
        .unwrap()
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

/// The findings of the NKBA group (not the older "kitchen aisle width" and
/// "counter depth" rules of the fixtures file).
fn nkba_only(f: &[Finding]) -> Vec<&Finding> {
    f.iter()
        .filter(|f| GUIDELINES.iter().any(|g| g.id == f.rule))
        .collect()
}

fn of<'a>(f: &'a [Finding], rule: &str) -> Vec<&'a Finding> {
    f.iter().filter(|f| f.rule == rule).collect()
}

fn fires(f: &[Finding], rule: &str) -> bool {
    !of(f, rule).is_empty()
}

fn sev(f: &[Finding], rule: &str) -> Option<Severity> {
    of(f, rule).first().map(|f| f.severity)
}

/// A cabinet: `kind` with its back-left corner at `at`, turned `angle`
/// radians. Base-like cabinets get a countertop with a front overhang.
fn cab(id: u64, kind: &str, at: (f64, f64), angle: f64, size: (f64, f64)) -> Value {
    let base = matches!(kind, "Base" | "CornerBase" | "BlindBase");
    let mut v = json!({
        "id": id, "kind": kind, "position": { "x": at.0, "y": at.1 }, "angle": angle,
        "width": size.0, "depth": size.1,
        "height": if base { 36.0 } else { 30.0 },
        "elevation": if base { 0.0 } else { 54.0 },
    });
    if base {
        v["countertop"] = json!({
            "thickness": 1.5, "overhang_front": 1.0, "overhang_sides": 0.0, "overhang_back": 0.0,
            "corner": "None",
        });
    }
    v
}

/// A cabinet with a cutout (`kind` Sink or Cooktop) of `w` x `d` centred at
/// the local point `(cx, cy)`.
fn with_cutout(mut v: Value, kind: &str, c: (f64, f64), w: f64, d: f64) -> Value {
    let (x0, x1, y0, y1) = (c.0 - w / 2.0, c.0 + w / 2.0, c.1 - d / 2.0, c.1 + d / 2.0);
    let cut = json!({
        "kind": kind, "name": kind,
        "outline": [
            {"x": x0, "y": y0}, {"x": x1, "y": y0}, {"x": x1, "y": y1}, {"x": x0, "y": y1}
        ],
    });
    v.as_object_mut()
        .unwrap()
        .entry("cutouts")
        .or_insert(json!([]))
        .as_array_mut()
        .unwrap()
        .push(cut);
    v
}

fn cab_mut(p: &mut Project, id: u64) -> &mut Value {
    p.floors[0]
        .cabinets
        .iter_mut()
        .find(|c| c["id"] == id)
        .expect("cabinet")
}

fn add_cab(p: &mut Project, v: Value) {
    p.floors[0].cabinets.push(v);
}

/// A symbol with its back-centre at `at`.
#[allow(clippy::too_many_arguments)]
fn sym(
    p: &mut Project,
    id: &str,
    at: (f64, f64),
    size: (f64, f64),
    angle: f64,
    elevation: f64,
    height: f64,
) -> u64 {
    let mut s = PlacedSymbol::new(id, pt(at.0, at.1), size.0, size.1, height);
    s.angle = angle;
    s.elevation = elevation;
    p.add_symbol(0, s)
}

fn remove_symbols(p: &mut Project, needle: &str) {
    p.floors[0]
        .symbols
        .retain(|s| !s.catalog_id.contains(needle));
}

fn set_devices(p: &mut Project, list: &[(&str, f64, f64)]) {
    let devs: Vec<Value> = list
        .iter()
        .map(|(k, x, y)| json!({ "kind": k, "position": { "x": x, "y": y } }))
        .collect();
    p.floors[0].electrical = Some(json!({ "devices": devs }));
}

const FRIDGE: &str = "core.appliances.refrigerator_36x30";
const RANGE: &str = "core.appliances.range_30";
const DISHWASHER: &str = "core.appliances.dishwasher_24";
const HOOD: &str = "core.bathkitchen.range_hood_30";
const MICROWAVE: &str = "core.appliances.microwave";
const OVEN: &str = "core.bathkitchen.wall_oven_30";

/// A 15' x 12' kitchen that meets every guideline: a south run (refrigerator,
/// cabinet, sink, dishwasher, trash cabinet), a north run around the range
/// and hood, GFCI receptacles along both walls and a 3'-0" exterior door.
///
/// Cabinets: 1 south beside the refrigerator, 2 the sink base, 3 the trash
/// cabinet, 4 to 7 the north run.
fn good_kitchen() -> Project {
    let mut p = Project::new("kitchen");
    let w = rect(&mut p, 0.0, 0.0, 180.0, 144.0, WallKind::Exterior);
    open(&mut p, w[3], 72.0, 36.0, 80.0, 0.0, OpeningKind::Door);
    sym(&mut p, FRIDGE, (21.0, 3.0), (36.0, 30.0), 0.0, 0.0, 70.0);
    sym(
        &mut p,
        DISHWASHER,
        (135.0, 3.0),
        (24.0, 24.0),
        0.0,
        0.0,
        34.0,
    );
    sym(&mut p, RANGE, (90.0, 141.0), (30.0, 25.0), 180.0, 0.0, 36.0);
    sym(&mut p, HOOD, (90.0, 141.0), (30.0, 20.0), 180.0, 66.0, 18.0);
    let pi = std::f64::consts::PI;
    add_cab(&mut p, cab(1, "Base", (39.0, 3.0), 0.0, (36.0, 24.0)));
    add_cab(
        &mut p,
        with_cutout(
            cab(2, "Base", (75.0, 3.0), 0.0, (48.0, 24.0)),
            "Sink",
            (24.0, 12.0),
            33.0,
            22.0,
        ),
    );
    let mut trash = cab(3, "Base", (147.0, 3.0), 0.0, (30.0, 24.0));
    trash["label"] = json!("Trash pullout");
    add_cab(&mut p, trash);
    // The north run faces south: turned half way round, back-left corner on
    // the east side of each cabinet.
    for (id, x1) in [(4, 39.0), (5, 75.0), (6, 141.0), (7, 177.0)] {
        add_cab(&mut p, cab(id, "Base", (x1, 141.0), pi, (36.0, 24.0)));
    }
    set_devices(
        &mut p,
        &[
            ("Gfci", 30.0, 0.0),
            ("Gfci", 78.0, 0.0),
            ("Gfci", 126.0, 0.0),
            ("Gfci", 170.0, 0.0),
            ("Gfci", 30.0, 144.0),
            ("Gfci", 78.0, 144.0),
            ("Gfci", 126.0, 144.0),
            ("Gfci", 170.0, 144.0),
        ],
    );
    p
}

fn kitchen_findings(p: &Project) -> Vec<Finding> {
    run(p, &["Kitchen"])
}

#[test]
fn a_well_planned_kitchen_has_no_nkba_findings() {
    let f = kitchen_findings(&good_kitchen());
    let bad: Vec<String> = nkba_only(&f)
        .iter()
        .map(|f| format!("{}: {}", f.rule, f.message))
        .collect();
    assert!(bad.is_empty(), "{bad:#?}");
}

// ----- entry door and swing -----

/// A kitchen beside a hall with a door of `w` and `style` between them.
fn kitchen_door(w: f64, style: OpeningStyle) -> Vec<Finding> {
    let mut p = Project::new("t");
    let a = rect(&mut p, 0.0, 0.0, 96.0, 96.0, WallKind::Exterior);
    let b = rect(&mut p, 96.0, 0.0, 240.0, 96.0, WallKind::Exterior);
    for id in [a[1], b[3]] {
        p.floors[0].wall_mut(id).unwrap().kind = WallKind::Interior;
    }
    let d = open(&mut p, b[3], 48.0, w, 80.0, 0.0, OpeningKind::Door);
    opening_mut(&mut p, d).style = style;
    run(&p, &["Kitchen", "Hall"])
}

#[test]
fn the_kitchen_doorway_is_32_inches_clear() {
    // A 36" leaf is 32" clear; 34" is not.
    assert!(!fires(
        &kitchen_door(36.0, OpeningStyle::Hinged),
        nkba::K_DOOR
    ));
    let f = kitchen_door(34.0, OpeningStyle::Hinged);
    assert_eq!(sev(&f, nkba::K_DOOR), Some(Severity::Warning));
    assert!(of(&f, nkba::K_DOOR)[0].message.contains("30\" clear"));
    assert!(matches!(
        of(&f, nkba::K_DOOR)[0].object,
        Some(Target::Opening(_))
    ));
    assert!(of(&f, nkba::K_DOOR)[0].location.is_some());
    // A cased opening has no leaf: its width is the clear width.
    assert!(!fires(
        &kitchen_door(32.0, OpeningStyle::Doorway),
        nkba::K_DOOR
    ));
    assert!(fires(
        &kitchen_door(31.0, OpeningStyle::Doorway),
        nkba::K_DOOR
    ));
    // A pocket door loses 2"; a sliding door is half its width clear.
    assert!(!fires(
        &kitchen_door(34.0, OpeningStyle::Pocket),
        nkba::K_DOOR
    ));
    assert!(fires(
        &kitchen_door(60.0, OpeningStyle::Sliding),
        nkba::K_DOOR
    ));
    assert!(!fires(
        &kitchen_door(72.0, OpeningStyle::Sliding),
        nkba::K_DOOR
    ));
}

#[test]
fn a_pantry_door_is_not_the_kitchen_entry() {
    let mut p = Project::new("t");
    let a = rect(&mut p, 0.0, 0.0, 96.0, 96.0, WallKind::Exterior);
    let b = rect(&mut p, 96.0, 0.0, 144.0, 96.0, WallKind::Exterior);
    for id in [a[1], b[3]] {
        p.floors[0].wall_mut(id).unwrap().kind = WallKind::Interior;
    }
    open(&mut p, b[3], 48.0, 24.0, 80.0, 0.0, OpeningKind::Door);
    assert!(!fires(&run(&p, &["Kitchen", "Pantry"]), nkba::K_DOOR));
    assert!(fires(&run(&p, &["Kitchen", "Hall"]), nkba::K_DOOR));
}

#[test]
fn a_door_must_not_swing_into_a_cabinet_or_appliance() {
    // Door on the wall between a kitchen (west) and a hall; a base cabinet
    // stands just inside the kitchen. The door swings into the kitchen for
    // exactly one of the two hand settings.
    let build = |flip: bool, cabinet: bool| {
        let mut p = Project::new("t");
        let a = rect(&mut p, 0.0, 0.0, 96.0, 96.0, WallKind::Exterior);
        let b = rect(&mut p, 96.0, 0.0, 240.0, 96.0, WallKind::Exterior);
        for id in [a[1], b[3]] {
            p.floors[0].wall_mut(id).unwrap().kind = WallKind::Interior;
        }
        let d = open(&mut p, b[3], 48.0, 36.0, 80.0, 0.0, OpeningKind::Door);
        opening_mut(&mut p, d).swing_flipped = flip;
        if cabinet {
            add_cab(&mut p, cab(1, "Base", (70.0, 40.0), 0.0, (24.0, 24.0)));
        }
        run(&p, &["Kitchen", "Hall"])
    };
    let hits = [false, true].map(|flip| fires(&build(flip, true), nkba::K_SWING));
    assert_eq!(hits.iter().filter(|h| **h).count(), 1, "{hits:?}");
    assert!(!fires(&build(false, false), nkba::K_SWING));
    assert!(!fires(&build(true, false), nkba::K_SWING));
    let flip = hits[1];
    let f = build(flip, true);
    assert_eq!(sev(&f, nkba::K_SWING), Some(Severity::Warning));
    assert!(matches!(
        of(&f, nkba::K_SWING)[0].object,
        Some(Target::Opening(_))
    ));
}

// ----- the work triangle -----

/// The good kitchen with the refrigerator moved to back-centre `(x, y)`.
fn triangle(fridge: (f64, f64), range: (f64, f64), sink_x: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 240.0, 144.0, WallKind::Exterior);
    add_cab(
        &mut p,
        with_cutout(
            cab(2, "Base", (sink_x - 24.0, 3.0), 0.0, (48.0, 24.0)),
            "Sink",
            (24.0, 12.0),
            33.0,
            22.0,
        ),
    );
    sym(&mut p, FRIDGE, fridge, (36.0, 30.0), 180.0, 0.0, 70.0);
    sym(&mut p, RANGE, range, (30.0, 25.0), 180.0, 0.0, 36.0);
    run(&p, &["Kitchen"])
}

#[test]
fn the_work_triangle_legs_and_total() {
    // Sink front (60, 29); range (30, 115); refrigerator (100, 110): legs of
    // 91", 70" and 90" and a total of 251".
    let ok = triangle((100.0, 141.0), (30.0, 141.0), 60.0);
    assert!(
        !fires(&ok, nkba::K_TRIANGLE),
        "{:?}",
        of(&ok, nkba::K_TRIANGLE)
    );
    // The sink and range far apart: a leg over 9' and a total over 26'.
    let far = triangle((100.0, 141.0), (230.0, 141.0), 30.0);
    let f = of(&far, nkba::K_TRIANGLE);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].severity, Severity::Warning);
    assert!(f[0].message.contains("at most 9'"), "{}", f[0].message);
    assert!(f[0].message.contains("at most 26'"), "{}", f[0].message);
    assert!(matches!(f[0].object, Some(Target::Cabinet(2))));
    // All three within a few feet: legs under 4' and a total under 13'.
    let tight = triangle((70.0, 141.0), (100.0, 141.0), 60.0);
    // Refrigerator and range are 30" apart: only the short leg is reported.
    let f = of(&tight, nkba::K_TRIANGLE);
    assert_eq!(f.len(), 1, "{tight:?}");
    assert!(f[0].message.contains("at least 4'"), "{}", f[0].message);
}

#[test]
fn an_island_may_cut_a_leg_by_a_foot_not_more() {
    let build = |island_depth: f64| {
        let mut p = Project::new("t");
        rect(&mut p, 0.0, 0.0, 240.0, 144.0, WallKind::Exterior);
        add_cab(
            &mut p,
            with_cutout(
                cab(2, "Base", (36.0, 3.0), 0.0, (48.0, 24.0)),
                "Sink",
                (24.0, 12.0),
                33.0,
                22.0,
            ),
        );
        sym(
            &mut p,
            FRIDGE,
            (100.0, 141.0),
            (36.0, 30.0),
            180.0,
            0.0,
            70.0,
        );
        sym(&mut p, RANGE, (30.0, 141.0), (30.0, 25.0), 180.0, 0.0, 36.0);
        // An island across the leg from the refrigerator to the sink.
        add_cab(
            &mut p,
            cab(9, "Base", (50.0, 60.0), 0.0, (48.0, island_depth)),
        );
        run(&p, &["Kitchen"])
    };
    assert!(
        !fires(&build(10.0), nkba::K_TRIANGLE),
        "11 inches of island"
    );
    let f = build(24.0);
    assert!(fires(&f, nkba::K_TRIANGLE));
    assert!(of(&f, nkba::K_TRIANGLE)[0].message.contains("cuts the"));
}

#[test]
fn a_triangle_needs_all_three_work_centers() {
    let mut p = good_kitchen();
    remove_symbols(&mut p, "refrigerator");
    let f = kitchen_findings(&p);
    assert!(!fires(&f, nkba::K_TRIANGLE));
    let report = nkba_report_of(&p);
    let row = report
        .rows
        .iter()
        .find(|r| r.guideline == nkba::K_TRIANGLE)
        .unwrap();
    assert_eq!(row.status, NkbaStatus::NotInPlan);
}

// ----- aisles and seating -----

/// The good kitchen plus a second sink on an island whose front is `aisle`
/// inches from the south run's counter and a refrigerator at the island.
fn two_cook(aisle: f64, second_sink: bool) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 150.0, WallKind::Exterior);
    add_cab(
        &mut p,
        with_cutout(
            cab(1, "Base", (10.0, 3.0), 0.0, (96.0, 24.0)),
            "Sink",
            (30.0, 12.0),
            33.0,
            22.0,
        ),
    );
    // Island facing south; its front is 'aisle' from the run's front (27).
    let back = 27.0 + aisle + 24.0;
    let mut island = cab(2, "Base", (106.0, back), std::f64::consts::PI, (96.0, 24.0));
    if second_sink {
        island = with_cutout(island, "Sink", (48.0, 12.0), 33.0, 22.0);
    }
    add_cab(&mut p, island);
    run(&p, &["Kitchen"])
}

#[test]
fn two_cooks_need_a_48_inch_work_aisle() {
    // The ray starts at the sink's counter front (1" overhang).
    assert!(fires(&two_cook(46.0, true), nkba::K_AISLE2));
    assert_eq!(
        sev(&two_cook(46.0, true), nkba::K_AISLE2),
        Some(Severity::Info)
    );
    assert!(!fires(&two_cook(51.0, true), nkba::K_AISLE2));
    // One sink is one cook: the 42" aisle of the older rule applies.
    assert!(!fires(&two_cook(46.0, false), nkba::K_AISLE2));
}

/// A south run with an island whose seating side faces south: the overhang
/// is `over`, the island is `height` high and the aisle in front of the
/// seating is `aisle` inches clear.
fn seating(height: f64, over: f64, aisle: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 150.0, WallKind::Exterior);
    add_cab(&mut p, cab(1, "Base", (10.0, 3.0), 0.0, (96.0, 24.0)));
    // Island turned to face south; seats on its south side.
    let front = 27.0 + aisle + over;
    let back = front + 24.0;
    let mut island = cab(2, "Base", (106.0, back), std::f64::consts::PI, (96.0, 24.0));
    island["height"] = json!(height);
    island["countertop"]["overhang_front"] = json!(over);
    add_cab(&mut p, island);
    run(&p, &["Kitchen"])
}

#[test]
fn seating_has_knee_space_for_its_counter_height() {
    // 36" counter: 15"; 42" bar: 12"; 30" table: 18".
    assert!(!fires(&seating(36.0, 15.0, 50.0), nkba::K_SEAT_KNEE));
    let f = seating(36.0, 12.0, 50.0);
    assert_eq!(sev(&f, nkba::K_SEAT_KNEE), Some(Severity::Warning));
    assert!(of(&f, nkba::K_SEAT_KNEE)[0]
        .message
        .contains("15\" of knee space"));
    assert!(!fires(&seating(42.0, 12.0, 50.0), nkba::K_SEAT_KNEE));
    assert!(fires(&seating(42.0, 8.0, 50.0), nkba::K_SEAT_KNEE));
    assert!(fires(&seating(30.0, 12.0, 50.0), nkba::K_SEAT_KNEE));
    assert!(!fires(&seating(30.0, 18.0, 50.0), nkba::K_SEAT_KNEE));
    // A 1" counter edge is not seating.
    assert!(!fires(&seating(36.0, 1.0, 50.0), nkba::K_SEAT_KNEE));
}

#[test]
fn seating_has_36_inches_behind_it_and_44_where_people_pass() {
    let tight = seating(36.0, 15.0, 30.0);
    assert_eq!(sev(&tight, nkba::K_SEAT_CLEAR), Some(Severity::Warning));
    let mid = seating(36.0, 15.0, 40.0);
    assert_eq!(sev(&mid, nkba::K_SEAT_CLEAR), Some(Severity::Info));
    assert!(!fires(&seating(36.0, 15.0, 46.0), nkba::K_SEAT_CLEAR));
    assert!(matches!(
        of(&tight, nkba::K_SEAT_CLEAR)[0].object,
        Some(Target::Cabinet(2))
    ));
}

// ----- landings -----

/// A south run: `left` inches of base cabinet, a `sink_w` cutout in a cabinet
/// of `width`, `right` inches more.
fn sink_run(width: f64, cutout_x: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 144.0, WallKind::Exterior);
    add_cab(
        &mut p,
        with_cutout(
            cab(1, "Base", (10.0, 3.0), 0.0, (width, 24.0)),
            "Sink",
            (cutout_x, 12.0),
            33.0,
            22.0,
        ),
    );
    run(&p, &["Kitchen"])
}

#[test]
fn the_sink_has_24_inches_on_one_side_and_18_on_the_other() {
    // 96" cabinet, sink centred: 31.5" each side.
    assert!(!fires(&sink_run(96.0, 48.0), nkba::K_SINK));
    // Sink 20" from one end: 3.5" on that side.
    let f = sink_run(96.0, 20.0);
    assert_eq!(sev(&f, nkba::K_SINK), Some(Severity::Warning));
    assert!(
        of(&f, nkba::K_SINK)[0].message.contains("4\" of counter"),
        "{}",
        of(&f, nkba::K_SINK)[0].message
    );
    // 24" and 18": the edge of the rule.
    // Cutout 33" wide; width 33 + 24 + 18 = 75, cutout starts at 24.
    assert!(!fires(&sink_run(75.0, 24.0 + 16.5), nkba::K_SINK));
    assert!(fires(&sink_run(75.0 - 1.0, 24.0 + 16.5), nkba::K_SINK));
    assert!(fires(&sink_run(75.0, 23.0 + 16.5), nkba::K_SINK));
    // 18" on both sides is not 24" on one.
    assert!(fires(&sink_run(33.0 + 36.0, 18.0 + 16.5), nkba::K_SINK));
    assert!(matches!(
        of(&sink_run(96.0, 20.0), nkba::K_SINK)[0].object,
        Some(Target::Cabinet(1))
    ));
}

#[test]
fn a_dishwasher_beside_the_sink_counts_as_counter() {
    let mut p = good_kitchen();
    assert!(!fires(&kitchen_findings(&p), nkba::K_SINK));
    // Without the dishwasher the right landing is 7.5" of the sink cabinet
    // and the 30" cabinet beyond a gap: too little.
    remove_symbols(&mut p, "dishwasher");
    assert!(fires(&kitchen_findings(&p), nkba::K_SINK));
}

#[test]
fn a_second_sink_needs_18_and_3_inches() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 240.0, 144.0, WallKind::Exterior);
    // A big sink with ample counter, and a bar sink with 4" beside it.
    add_cab(
        &mut p,
        with_cutout(
            cab(1, "Base", (10.0, 3.0), 0.0, (96.0, 24.0)),
            "Sink",
            (48.0, 12.0),
            33.0,
            22.0,
        ),
    );
    add_cab(
        &mut p,
        with_cutout(
            cab(2, "Base", (140.0, 3.0), 0.0, (60.0, 24.0)),
            "Sink",
            (20.0, 12.0),
            15.0,
            15.0,
        ),
    );
    let f = run(&p, &["Kitchen"]);
    assert!(!fires(&f, nkba::K_SINK), "{:?}", of(&f, nkba::K_SINK));
    // The bar sink at the end of its cabinet with 1" on one side and 3"...
    let mut p2 = p.clone();
    cab_mut(&mut p2, 2)["cutouts"][0]["outline"] = json!([
        {"x": 0.0, "y": 5.0}, {"x": 15.0, "y": 5.0}, {"x": 15.0, "y": 20.0}, {"x": 0.0, "y": 20.0}
    ]);
    // 0" on the left, 45" on the right: the 3" side is short.
    assert!(fires(&run(&p2, &["Kitchen"]), nkba::K_SINK));
}

/// A range between two cabinets on the south wall.
fn range_between(left: f64, right: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 144.0, WallKind::Exterior);
    sym(&mut p, RANGE, (100.0, 3.0), (30.0, 25.0), 0.0, 0.0, 36.0);
    if left > 0.0 {
        add_cab(
            &mut p,
            cab(1, "Base", (85.0 - left, 3.0), 0.0, (left, 24.0)),
        );
    }
    if right > 0.0 {
        add_cab(&mut p, cab(2, "Base", (115.0, 3.0), 0.0, (right, 24.0)));
    }
    run(&p, &["Kitchen"])
}

#[test]
fn the_cooking_surface_has_15_inches_on_one_side_and_12_on_the_other() {
    assert!(!fires(&range_between(24.0, 12.0), nkba::K_COOKTOP));
    assert!(!fires(&range_between(15.0, 12.0), nkba::K_COOKTOP));
    assert!(!fires(&range_between(12.0, 15.0), nkba::K_COOKTOP));
    let f = range_between(15.0, 11.0);
    assert_eq!(sev(&f, nkba::K_COOKTOP), Some(Severity::Warning));
    assert!(fires(&range_between(14.0, 14.0), nkba::K_COOKTOP));
    assert!(fires(&range_between(24.0, 0.0), nkba::K_COOKTOP));
    assert!(matches!(
        of(&f, nkba::K_COOKTOP)[0].object,
        Some(Target::Symbol(_))
    ));
}

#[test]
fn a_cooktop_cutout_is_a_cooking_surface_too() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 144.0, WallKind::Exterior);
    // 30" of cooktop in a 36" cabinet with no counter beside it.
    add_cab(
        &mut p,
        with_cutout(
            cab(1, "Base", (60.0, 3.0), 0.0, (36.0, 24.0)),
            "Cooktop",
            (18.0, 12.0),
            30.0,
            21.0,
        ),
    );
    let f = run(&p, &["Kitchen"]);
    assert!(fires(&f, nkba::K_COOKTOP));
    assert!(fires(&f, nkba::K_VENT), "a cooktop with no hood");
}

/// An appliance (`id`) at the end of the south run with `beside` inches of
/// counter next to it, and optionally an island across.
fn appliance_landing(
    id: &str,
    size: (f64, f64),
    elevation: f64,
    beside: f64,
    island: bool,
) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 144.0, WallKind::Exterior);
    sym(&mut p, id, (60.0, 3.0), size, 0.0, elevation, 70.0);
    if beside > 0.0 {
        add_cab(
            &mut p,
            cab(1, "Base", (60.0 + size.0 / 2.0, 3.0), 0.0, (beside, 24.0)),
        );
    }
    if island {
        add_cab(&mut p, cab(2, "Base", (40.0, 60.0), 0.0, (36.0, 24.0)));
    }
    run(&p, &["Kitchen"])
}

#[test]
fn the_refrigerator_oven_and_microwave_have_15_inches_beside_or_across() {
    for (rule, id, size, elev) in [
        (nkba::K_FRIDGE, FRIDGE, (36.0, 30.0), 0.0),
        (nkba::K_OVEN, OVEN, (30.0, 24.0), 0.0),
        (nkba::K_MICRO, MICROWAVE, (24.0, 16.0), 0.0),
    ] {
        assert!(
            !fires(&appliance_landing(id, size, elev, 15.0, false), rule),
            "{rule}"
        );
        let f = appliance_landing(id, size, elev, 14.0, false);
        assert_eq!(sev(&f, rule), Some(Severity::Warning), "{rule}");
        assert!(
            fires(&appliance_landing(id, size, elev, 0.0, false), rule),
            "{rule}"
        );
        // 15" of counter across the aisle within 48" counts. The front of
        // the appliance is 30" from the wall at most; the island is at
        // y = 60..84.
        assert!(
            !fires(&appliance_landing(id, size, elev, 0.0, true), rule),
            "{rule} island"
        );
    }
}

// ----- the dishwasher -----

fn dishwasher_at(x: f64, opposite: Option<f64>) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 240.0, 144.0, WallKind::Exterior);
    add_cab(
        &mut p,
        with_cutout(
            cab(1, "Base", (10.0, 3.0), 0.0, (60.0, 24.0)),
            "Sink",
            (30.0, 12.0),
            33.0,
            22.0,
        ),
    );
    sym(&mut p, DISHWASHER, (x, 3.0), (24.0, 24.0), 0.0, 0.0, 34.0);
    if let Some(gap) = opposite {
        // A run across from the dishwasher, `gap` inches from its front (27).
        add_cab(
            &mut p,
            cab(
                2,
                "Base",
                (x + 30.0, 27.0 + gap + 24.0),
                std::f64::consts::PI,
                (60.0, 24.0),
            ),
        );
    }
    run(&p, &["Kitchen"])
}

#[test]
fn the_dishwasher_is_near_the_sink_with_21_inches_of_standing_space() {
    // Sink cutout spans 25..55 (centre 40); the dishwasher right beside it.
    assert!(!fires(&dishwasher_at(80.0, None), nkba::K_DISHWASHER));
    // 36" from the cutout's edge is the limit.
    let edge = 55.0 + 36.0 + 12.0;
    assert!(!fires(&dishwasher_at(edge, None), nkba::K_DISHWASHER));
    let far = dishwasher_at(edge + 2.0, None);
    assert_eq!(sev(&far, nkba::K_DISHWASHER), Some(Severity::Warning));
    assert!(of(&far, nkba::K_DISHWASHER)[0]
        .message
        .contains("from the sink"));
    // A run 18" across leaves 18" of standing space.
    let tight = dishwasher_at(80.0, Some(18.0));
    assert!(fires(&tight, nkba::K_DISHWASHER));
    assert!(of(&tight, nkba::K_DISHWASHER)[0]
        .message
        .contains("18\" of standing space"));
    assert!(!fires(&dishwasher_at(80.0, Some(22.0)), nkba::K_DISHWASHER));
}

// ----- cooking surface clearance, safety and ventilation -----

fn range_with_wall_cabinet(elevation: f64) -> Vec<Finding> {
    let mut p = good_kitchen();
    let mut w = cab(
        20,
        "Wall",
        (85.0, 129.0),
        std::f64::consts::PI,
        (30.0, 12.0),
    );
    // Over the range on the north wall: footprint x 55..85? Place it over x 75..105.
    w["position"] = json!({ "x": 105.0, "y": 141.0 });
    w["elevation"] = json!(elevation);
    add_cab(&mut p, w);
    kitchen_findings(&p)
}

#[test]
fn a_cabinet_over_the_cooking_surface_is_24_or_30_inches_up() {
    // The range top is 36" high.
    assert!(!fires(&range_with_wall_cabinet(66.0), nkba::K_COOK_CLEAR));
    let mid = range_with_wall_cabinet(62.0);
    assert_eq!(sev(&mid, nkba::K_COOK_CLEAR), Some(Severity::Info));
    assert!(of(&mid, nkba::K_COOK_CLEAR)[0].message.contains("26\""));
    let low = range_with_wall_cabinet(54.0);
    assert_eq!(sev(&low, nkba::K_COOK_CLEAR), Some(Severity::Warning));
    assert!(matches!(
        of(&low, nkba::K_COOK_CLEAR)[0].object,
        Some(Target::Cabinet(20))
    ));
    // At exactly 24" it is the protected-surface case.
    assert_eq!(
        sev(&range_with_wall_cabinet(60.0), nkba::K_COOK_CLEAR),
        Some(Severity::Info)
    );
}

#[test]
fn a_microwave_over_the_range_counts_for_clearance_and_ventilation() {
    let mut p = good_kitchen();
    remove_symbols(&mut p, "hood");
    sym(
        &mut p,
        MICROWAVE,
        (90.0, 141.0),
        (30.0, 16.0),
        180.0,
        54.0,
        16.0,
    );
    let f = kitchen_findings(&p);
    // 54 - 36 = 18" under the microwave.
    assert_eq!(sev(&f, nkba::K_COOK_CLEAR), Some(Severity::Warning));
    assert!(!fires(&f, nkba::K_VENT), "the microwave vents the range");
}

#[test]
fn a_hood_is_needed_and_as_wide_as_the_cooktop() {
    let mut p = good_kitchen();
    remove_symbols(&mut p, "hood");
    let f = kitchen_findings(&p);
    assert_eq!(sev(&f, nkba::K_VENT), Some(Severity::Warning));
    assert!(matches!(
        of(&f, nkba::K_VENT)[0].object,
        Some(Target::Symbol(_))
    ));
    // A 24" hood over a 30" range.
    sym(
        &mut p,
        "core.bathkitchen.range_hood_24",
        (90.0, 141.0),
        (24.0, 20.0),
        180.0,
        66.0,
        18.0,
    );
    let narrow = kitchen_findings(&p);
    assert_eq!(sev(&narrow, nkba::K_VENT), Some(Severity::Info));
    // An exhaust fan within 5' will do.
    remove_symbols(&mut p, "hood");
    sym(
        &mut p,
        "core.lighting.exhaust_fan_14",
        (90.0, 100.0),
        (14.0, 14.0),
        0.0,
        96.0,
        4.0,
    );
    assert!(!fires(&kitchen_findings(&p), nkba::K_VENT));
}

#[test]
fn a_window_over_the_cooktop_is_a_hazard() {
    let build = |x: f64, sill: f64| {
        let mut p = Project::new("t");
        let w = rect(&mut p, 0.0, 0.0, 200.0, 144.0, WallKind::Exterior);
        sym(&mut p, RANGE, (100.0, 3.0), (30.0, 25.0), 0.0, 0.0, 36.0);
        open(&mut p, w[0], x, 36.0, 36.0, sill, OpeningKind::Window);
        run(&p, &["Kitchen"])
    };
    let f = build(100.0, 36.0);
    assert_eq!(sev(&f, nkba::K_COOK_SAFE), Some(Severity::Info));
    assert!(matches!(
        of(&f, nkba::K_COOK_SAFE)[0].object,
        Some(Target::Opening(_))
    ));
    // Beside the range, within 12" of its side.
    assert!(fires(&build(145.0, 36.0), nkba::K_COOK_SAFE));
    // Well away, or high enough to clear the hood.
    assert!(!fires(&build(20.0, 36.0), nkba::K_COOK_SAFE));
    assert!(!fires(&build(100.0, 70.0), nkba::K_COOK_SAFE));
}

// ----- waste, counters, edges -----

#[test]
fn a_kitchen_needs_a_waste_receptacle() {
    let mut p = good_kitchen();
    cab_mut(&mut p, 3)["label"] = json!("Pantry base");
    let f = kitchen_findings(&p);
    assert_eq!(sev(&f, nkba::K_WASTE), Some(Severity::Info));
    sym(
        &mut p,
        "core.bathkitchen.trash_compactor_15",
        (20.0, 60.0),
        (15.0, 24.0),
        0.0,
        0.0,
        34.0,
    );
    assert!(!fires(&kitchen_findings(&p), nkba::K_WASTE));
}

/// A small L kitchen (under 150 sq ft) with `n` 36" cabinets.
fn small_kitchen(n: usize, sink: bool) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 140.0, 120.0, WallKind::Exterior);
    for k in 0..n {
        let mut c = cab(
            k as u64 + 1,
            "Base",
            (3.0 + 36.0 * (k % 3) as f64, 3.0 + 40.0 * (k / 3) as f64),
            0.0,
            (36.0, 24.0),
        );
        if sink && k == 0 {
            c = with_cutout(c, "Sink", (18.0, 12.0), 33.0, 22.0);
        }
        add_cab(&mut p, c);
    }
    run(&p, &["Kitchen"])
}

#[test]
fn counter_frontage_is_158_inches_in_a_small_kitchen_198_in_a_large_one() {
    // 140" x 120" is 117 sq ft: 158" needed. Four cabinets are 144".
    let f = small_kitchen(4, false);
    assert_eq!(sev(&f, nkba::K_FRONTAGE), Some(Severity::Warning));
    assert!(of(&f, nkba::K_FRONTAGE)[0].message.contains("144\""));
    assert!(of(&f, nkba::K_FRONTAGE)[0].message.contains("158\""));
    // Five are 180".
    assert!(!fires(&small_kitchen(5, false), nkba::K_FRONTAGE));
    // The sink takes its 33" out of the frontage: 147".
    assert!(fires(&small_kitchen(5, true), nkba::K_FRONTAGE));
    assert!(of(&small_kitchen(5, true), nkba::K_FRONTAGE)[0]
        .message
        .contains("147\""));
    // A kitchen of 150 sq ft or more needs 198".
    let mut p = good_kitchen();
    assert!(!fires(&kitchen_findings(&p), nkba::K_FRONTAGE));
    cab_mut(&mut p, 7)["width"] = json!(8.0);
    let f = kitchen_findings(&p);
    assert!(
        of(&f, nkba::K_FRONTAGE)[0].message.contains("198\""),
        "{f:?}"
    );
}

#[test]
fn corner_cabinets_do_not_count_toward_the_frontage() {
    let mut p = good_kitchen();
    cab_mut(&mut p, 7)["kind"] = json!("CornerBase");
    let f = kitchen_findings(&p);
    assert!(fires(&f, nkba::K_FRONTAGE), "225 - 36 = 189");
}

#[test]
fn island_corners_are_clipped_or_rounded() {
    let mut p = good_kitchen();
    assert!(!fires(&kitchen_findings(&p), nkba::K_EDGES));
    add_cab(&mut p, cab(9, "Base", (60.0, 60.0), 0.0, (48.0, 30.0)));
    let f = kitchen_findings(&p);
    assert_eq!(sev(&f, nkba::K_EDGES), Some(Severity::Info));
    assert!(matches!(
        of(&f, nkba::K_EDGES)[0].object,
        Some(Target::Cabinet(9))
    ));
    cab_mut(&mut p, 9)["countertop"]["corner"] = json!("Clipped");
    assert!(!fires(&kitchen_findings(&p), nkba::K_EDGES));
    cab_mut(&mut p, 9)["countertop"]["corner"] = json!("Rounded");
    assert!(!fires(&kitchen_findings(&p), nkba::K_EDGES));
}

// ----- electrical -----

#[test]
fn the_counter_has_a_receptacle_within_24_inches_of_every_point() {
    let mut p = good_kitchen();
    // Drop the east receptacles on the south wall.
    set_devices(
        &mut p,
        &[
            ("Gfci", 30.0, 0.0),
            ("Gfci", 30.0, 144.0),
            ("Gfci", 78.0, 144.0),
            ("Gfci", 126.0, 144.0),
            ("Gfci", 170.0, 144.0),
        ],
    );
    let f = kitchen_findings(&p);
    assert_eq!(sev(&f, nkba::K_RECEPT), Some(Severity::Warning));
    assert!(of(&f, nkba::K_RECEPT)[0].location.is_some());
    assert!(matches!(
        of(&f, nkba::K_RECEPT)[0].object,
        Some(Target::Cabinet(_))
    ));
    // A kitchen with no electrical layer at all is not judged.
    p.floors[0].electrical = None;
    let f = kitchen_findings(&p);
    assert!(!fires(&f, nkba::K_RECEPT));
    assert!(!fires(&f, nkba::K_GFCI));
}

#[test]
fn an_island_needs_a_receptacle() {
    let mut p = good_kitchen();
    add_cab(&mut p, cab(9, "Base", (60.0, 60.0), 0.0, (48.0, 30.0)));
    cab_mut(&mut p, 9)["countertop"]["corner"] = json!("Clipped");
    let f = kitchen_findings(&p);
    let r = of(&f, nkba::K_RECEPT);
    assert_eq!(r.len(), 1);
    assert!(r[0].message.contains("island"));
    // A receptacle on the island's end.
    set_devices(
        &mut p,
        &[
            ("Gfci", 30.0, 0.0),
            ("Gfci", 78.0, 0.0),
            ("Gfci", 126.0, 0.0),
            ("Gfci", 170.0, 0.0),
            ("Gfci", 30.0, 144.0),
            ("Gfci", 78.0, 144.0),
            ("Gfci", 126.0, 144.0),
            ("Gfci", 170.0, 144.0),
            ("OutletFloor", 84.0, 80.0),
        ],
    );
    assert!(!fires(&kitchen_findings(&p), nkba::K_RECEPT));
}

#[test]
fn receptacles_within_six_feet_of_the_sink_are_gfci() {
    let mut p = good_kitchen();
    assert!(!fires(&kitchen_findings(&p), nkba::K_GFCI));
    set_devices(
        &mut p,
        &[
            ("Gfci", 30.0, 0.0),
            ("Outlet110", 78.0, 0.0),
            ("Outlet110Quad", 126.0, 0.0),
        ],
    );
    let f = kitchen_findings(&p);
    let g = of(&f, nkba::K_GFCI);
    assert_eq!(g.len(), 1);
    assert!(g[0].message.contains("2 receptacles"), "{}", g[0].message);
    assert_eq!(g[0].severity, Severity::Warning);
    // A plain receptacle 12' from the sink is not near the water.
    set_devices(&mut p, &[("Outlet110", 170.0, 144.0)]);
    assert!(!fires(&kitchen_findings(&p), nkba::K_GFCI));
}

// ----- the bathroom -----

/// An 8' x 10' bath: a south vanity with a basin, a toilet, a 36" shower
/// with its valve, grab bars, a mirror and a window; a 3'-0" door.
///
/// Cabinet 1 is the vanity.
fn good_bath() -> Project {
    let mut p = Project::new("bath");
    let w = rect(&mut p, 0.0, 0.0, 120.0, 96.0, WallKind::Exterior);
    open(&mut p, w[0], 90.0, 24.0, 36.0, 36.0, OpeningKind::Window);
    open(&mut p, w[1], 48.0, 36.0, 80.0, 0.0, OpeningKind::Door);
    add_cab(&mut p, {
        let mut v = with_cutout(
            cab(1, "Base", (3.0, 3.0), 0.0, (60.0, 21.0)),
            "Sink",
            (30.0, 10.5),
            20.0,
            16.0,
        );
        v["height"] = json!(34.0);
        v
    });
    sym(
        &mut p,
        "core.bathkitchen.toilet_elongated",
        (90.0, 3.0),
        (20.0, 28.0),
        0.0,
        0.0,
        30.0,
    );
    sym(
        &mut p,
        "core.bathkitchen.shower_36x36",
        (21.0, 93.0),
        (36.0, 36.0),
        180.0,
        0.0,
        80.0,
    );
    sym(
        &mut p,
        "core.plumbing.shower_valve",
        (21.0, 93.0),
        (4.0, 4.0),
        180.0,
        40.0,
        8.0,
    );
    sym(
        &mut p,
        "core.bathkitchen.grab_bar_24",
        (105.0, 20.0),
        (24.0, 2.0),
        90.0,
        33.0,
        2.0,
    );
    sym(
        &mut p,
        "core.bathkitchen.grab_bar_36",
        (39.0, 70.0),
        (36.0, 2.0),
        90.0,
        33.0,
        2.0,
    );
    sym(
        &mut p,
        "core.bathkitchen.mirror_36",
        (33.0, 3.0),
        (36.0, 1.0),
        0.0,
        40.0,
        30.0,
    );
    set_devices(&mut p, &[("Gfci", 33.0, 0.0)]);
    p
}

fn bath_findings(p: &Project) -> Vec<Finding> {
    run(p, &["Bathroom"])
}

#[test]
fn a_well_planned_bath_has_no_nkba_findings() {
    let f = bath_findings(&good_bath());
    let bad: Vec<String> = nkba_only(&f)
        .iter()
        .map(|f| format!("{}: {}", f.rule, f.message))
        .collect();
    assert!(bad.is_empty(), "{bad:#?}");
}

fn bath_door(w: f64, style: OpeningStyle) -> Vec<Finding> {
    let mut p = Project::new("t");
    let a = rect(&mut p, 0.0, 0.0, 96.0, 96.0, WallKind::Exterior);
    let b = rect(&mut p, 96.0, 0.0, 240.0, 96.0, WallKind::Exterior);
    for id in [a[1], b[3]] {
        p.floors[0].wall_mut(id).unwrap().kind = WallKind::Interior;
    }
    let d = open(&mut p, b[3], 48.0, w, 80.0, 0.0, OpeningKind::Door);
    opening_mut(&mut p, d).style = style;
    run(&p, &["Bathroom", "Hall"])
}

#[test]
fn the_bathroom_doorway_is_32_inches_clear() {
    assert!(!fires(&bath_door(36.0, OpeningStyle::Hinged), nkba::B_DOOR));
    assert_eq!(
        sev(&bath_door(30.0, OpeningStyle::Hinged), nkba::B_DOOR),
        Some(Severity::Warning)
    );
    assert!(!fires(
        &bath_door(32.0, OpeningStyle::Doorway),
        nkba::B_DOOR
    ));
    assert!(fires(&bath_door(24.0, OpeningStyle::Doorway), nkba::B_DOOR));
}

#[test]
fn the_lavatory_tub_and_shower_have_30_inches_clear_in_front() {
    // Vanity front at y = 25; the shower at the far wall is 57..93.
    let mut p = good_bath();
    assert!(!fires(&bath_findings(&p), nkba::B_CLEAR));
    // Move the shower's front to 20" from the vanity: 25 + 20 = 45.
    p.floors[0]
        .symbols
        .iter_mut()
        .find(|s| s.catalog_id.contains("shower_36x36"))
        .unwrap()
        .position = pt(21.0, 81.0);
    let f = bath_findings(&p);
    assert_eq!(sev(&f, nkba::B_CLEAR), Some(Severity::Warning));
    // The basin and the shower both fall short.
    assert_eq!(
        of(&f, nkba::B_CLEAR).len(),
        2,
        "{:?}",
        of(&f, nkba::B_CLEAR)
    );
    assert!(of(&f, nkba::B_CLEAR)[0].message.contains("clear floor"));
    // A tub with the wall 20" off its front.
    let mut t = Project::new("t");
    rect(&mut t, 0.0, 0.0, 120.0, 56.0, WallKind::Exterior);
    sym(
        &mut t,
        "core.bathkitchen.tub_alcove_60x30_left",
        (60.0, 3.0),
        (60.0, 30.0),
        0.0,
        0.0,
        20.0,
    );
    let f = bath_findings(&t);
    assert!(fires(&f, nkba::B_CLEAR));
    assert!(of(&f, nkba::B_CLEAR)[0].message.contains("tub"));
}

fn toilet_in(width: f64, depth: f64) -> Vec<Finding> {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, width, depth, WallKind::Exterior);
    sym(
        &mut p,
        "core.bathkitchen.toilet_elongated",
        (width / 2.0, 3.0),
        (20.0, 28.0),
        0.0,
        0.0,
        30.0,
    );
    run(&p, &["Bathroom"])
}

#[test]
fn the_toilet_has_16_inches_to_a_side_obstruction_and_30_in_front() {
    // Centre to wall face: (width - 6) / 2.
    let tight = toilet_in(37.0, 100.0);
    assert_eq!(sev(&tight, nkba::B_TOILET), Some(Severity::Info));
    assert!(of(&tight, nkba::B_TOILET)[0].message.contains("15.5\""));
    assert!(!fires(&toilet_in(40.0, 100.0), nkba::B_TOILET));
    // Under 15" is the code's rule (IRC), not this advisory.
    let code = toilet_in(34.0, 100.0);
    assert!(!fires(&code, nkba::B_TOILET));
    assert!(fires(&code, "IRC R307.1 water closet clearance"));
    // Front clearance: 3 + 28 = 31 to the toilet's front; wall face at depth - 3.
    let front = toilet_in(60.0, 59.0);
    assert_eq!(sev(&front, nkba::B_TOILET), Some(Severity::Info));
    assert!(of(&front, nkba::B_TOILET)[0]
        .message
        .contains("25\" clear in front"));
    assert!(!fires(&toilet_in(60.0, 64.0), nkba::B_TOILET));
    assert!(
        !fires(&toilet_in(60.0, 50.0), nkba::B_TOILET),
        "under 21\" is the IRC rule"
    );
}

#[test]
fn a_shower_is_36_by_36_inside() {
    let build = |w: f64, d: f64| {
        let mut p = Project::new("t");
        rect(&mut p, 0.0, 0.0, 120.0, 96.0, WallKind::Exterior);
        sym(
            &mut p,
            "core.bathkitchen.shower_36x36",
            (60.0, 3.0),
            (w, d),
            0.0,
            0.0,
            80.0,
        );
        run(&p, &["Bathroom"])
    };
    assert!(!fires(&build(36.0, 36.0), nkba::B_SHOWER));
    assert!(!fires(&build(48.0, 36.0), nkba::B_SHOWER));
    let small = build(34.0, 36.0);
    assert_eq!(sev(&small, nkba::B_SHOWER), Some(Severity::Warning));
    assert!(of(&small, nkba::B_SHOWER)[0]
        .message
        .contains("34\" x 36\""));
    // 32" x 32" passes the code (30" x 30") but not the NKBA.
    let f = build(32.0, 32.0);
    assert!(fires(&f, nkba::B_SHOWER));
    assert!(!fires(&f, "IRC P2708.1 shower and tub size"));
    // A tub is not a shower.
    let mut t = Project::new("t");
    rect(&mut t, 0.0, 0.0, 120.0, 96.0, WallKind::Exterior);
    sym(
        &mut t,
        "core.bathkitchen.tub_alcove_60x30_left",
        (60.0, 3.0),
        (60.0, 30.0),
        0.0,
        0.0,
        20.0,
    );
    assert!(!fires(&run(&t, &["Bathroom"]), nkba::B_SHOWER));
}

#[test]
fn shower_controls_are_38_to_48_inches_up() {
    let build = |elevation: Option<f64>| {
        let mut p = good_bath();
        remove_symbols(&mut p, "shower_valve");
        if let Some(e) = elevation {
            sym(
                &mut p,
                "core.plumbing.shower_valve",
                (21.0, 93.0),
                (4.0, 4.0),
                180.0,
                e,
                8.0,
            );
        }
        bath_findings(&p)
    };
    // Centre of the valve: elevation + 4".
    assert!(!fires(&build(Some(36.0)), nkba::B_CONTROLS)); // 40"
    assert!(!fires(&build(Some(34.0)), nkba::B_CONTROLS)); // 38"
    assert!(!fires(&build(Some(44.0)), nkba::B_CONTROLS)); // 48"
    let low = build(Some(30.0));
    assert_eq!(sev(&low, nkba::B_CONTROLS), Some(Severity::Warning));
    assert!(of(&low, nkba::B_CONTROLS)[0].message.contains("34\""));
    assert!(fires(&build(Some(45.0)), nkba::B_CONTROLS));
    // No valve placed: a reminder.
    let none = build(None);
    assert_eq!(sev(&none, nkba::B_CONTROLS), Some(Severity::Info));
}

#[test]
fn grab_bar_blocking_at_the_toilet_tub_and_shower() {
    let mut p = good_bath();
    assert!(!fires(&bath_findings(&p), nkba::B_GRAB));
    remove_symbols(&mut p, "grab_bar_24");
    let f = bath_findings(&p);
    assert_eq!(sev(&f, nkba::B_GRAB), Some(Severity::Info));
    assert!(of(&f, nkba::B_GRAB)[0].message.contains("toilet"));
    remove_symbols(&mut p, "grab_bar");
    let f = bath_findings(&p);
    assert!(of(&f, nkba::B_GRAB)[0].message.contains("toilet"));
    assert!(of(&f, nkba::B_GRAB)[0].message.contains("shower"));
}

#[test]
fn the_lavatory_top_is_32_to_43_inches_up() {
    let build = |h: f64| {
        let mut p = good_bath();
        cab_mut(&mut p, 1)["height"] = json!(h);
        bath_findings(&p)
    };
    assert!(!fires(&build(34.0), nkba::B_LAV_HEIGHT));
    assert!(!fires(&build(32.0), nkba::B_LAV_HEIGHT));
    assert!(!fires(&build(43.0), nkba::B_LAV_HEIGHT));
    let f = build(30.0);
    assert_eq!(sev(&f, nkba::B_LAV_HEIGHT), Some(Severity::Info));
    assert!(of(&f, nkba::B_LAV_HEIGHT)[0].message.contains("30\""));
    assert!(fires(&build(45.0), nkba::B_LAV_HEIGHT));
}

#[test]
fn double_lavatories_are_30_inches_apart_and_15_from_a_side_obstruction() {
    let build = |a: f64, b: f64| {
        let mut p = Project::new("t");
        rect(&mut p, 0.0, 0.0, 120.0, 96.0, WallKind::Exterior);
        let mut v = cab(1, "Base", (3.0, 3.0), 0.0, (72.0, 21.0));
        v["height"] = json!(34.0);
        let v = with_cutout(
            with_cutout(v, "Sink", (a, 10.5), 20.0, 16.0),
            "Sink",
            (b, 10.5),
            20.0,
            16.0,
        );
        add_cab(&mut p, v);
        run(&p, &["Bathroom"])
    };
    // Centres 18 and 54 in the cabinet: 36" apart, 18" from the walls.
    assert!(!fires(&build(18.0, 54.0), nkba::B_LAV_SPACE));
    let near = build(20.0, 44.0);
    assert_eq!(sev(&near, nkba::B_LAV_SPACE), Some(Severity::Warning));
    assert!(of(&near, nkba::B_LAV_SPACE)[0]
        .message
        .contains("24\" apart"));
    // 30" apart exactly.
    assert!(!fires(&build(18.0, 48.0), nkba::B_LAV_SPACE));
    // 10" from the west wall.
    let wall = build(10.0, 50.0);
    assert!(fires(&wall, nkba::B_LAV_SPACE));
    assert!(of(&wall, nkba::B_LAV_SPACE)[0]
        .message
        .contains("obstruction"));
}

#[test]
fn a_mirror_at_the_lavatory() {
    let mut p = good_bath();
    remove_symbols(&mut p, "mirror");
    let f = bath_findings(&p);
    assert_eq!(sev(&f, nkba::B_MIRROR), Some(Severity::Info));
    assert!(matches!(
        of(&f, nkba::B_MIRROR)[0].object,
        Some(Target::Cabinet(1))
    ));
}

#[test]
fn a_bath_needs_a_fan_or_a_window_of_three_square_feet() {
    let bare = |window: Option<(f64, f64)>, fan: bool| {
        let mut p = Project::new("t");
        let w = rect(&mut p, 0.0, 0.0, 120.0, 96.0, WallKind::Exterior);
        if let Some((ww, wh)) = window {
            open(&mut p, w[0], 60.0, ww, wh, 40.0, OpeningKind::Window);
        }
        if fan {
            sym(
                &mut p,
                "core.lighting.exhaust_fan_14",
                (60.0, 48.0),
                (14.0, 14.0),
                0.0,
                96.0,
                4.0,
            );
        }
        sym(
            &mut p,
            "core.bathkitchen.toilet_elongated",
            (90.0, 3.0),
            (20.0, 28.0),
            0.0,
            0.0,
            30.0,
        );
        run(&p, &["Bathroom"])
    };
    let none = bare(None, false);
    assert_eq!(sev(&none, nkba::B_VENT), Some(Severity::Warning));
    assert!(of(&none, nkba::B_VENT)[0].message.contains("no window"));
    // 24" x 36" is 6 sq ft; 12" x 24" is 2 sq ft; exactly 3 sq ft passes.
    assert!(!fires(&bare(Some((24.0, 36.0)), false), nkba::B_VENT));
    assert!(!fires(&bare(Some((36.0, 12.0)), false), nkba::B_VENT));
    let small = bare(Some((12.0, 24.0)), false);
    assert!(fires(&small, nkba::B_VENT));
    assert!(of(&small, nkba::B_VENT)[0].message.contains("2.0 sq ft"));
    // A fan covers a bath with no window.
    assert!(!fires(&bare(None, true), nkba::B_VENT));
}

#[test]
fn a_receptacle_within_36_inches_of_each_lavatory() {
    let mut p = good_bath();
    assert!(!fires(&bath_findings(&p), nkba::B_RECEPT));
    set_devices(&mut p, &[("Gfci", 110.0, 90.0)]);
    let f = bath_findings(&p);
    assert_eq!(sev(&f, nkba::B_RECEPT), Some(Severity::Warning));
    assert!(matches!(
        of(&f, nkba::B_RECEPT)[0].object,
        Some(Target::Cabinet(1))
    ));
    // No electrical layer: not judged.
    p.floors[0].electrical = None;
    let f = bath_findings(&p);
    assert!(!fires(&f, nkba::B_RECEPT));
    assert!(!fires(&f, nkba::B_GFCI));
}

#[test]
fn bath_receptacles_near_the_water_are_gfci() {
    let mut p = good_bath();
    set_devices(&mut p, &[("Gfci", 33.0, 0.0), ("Outlet110", 50.0, 0.0)]);
    let f = bath_findings(&p);
    let g = of(&f, nkba::B_GFCI);
    assert_eq!(g.len(), 1);
    assert!(g[0].message.contains("1 receptacle "), "{}", g[0].message);
    set_devices(&mut p, &[("Gfci", 33.0, 0.0), ("Gfci", 50.0, 0.0)]);
    assert!(!fires(&bath_findings(&p), nkba::B_GFCI));
}

// ----- the group, the settings and the report -----

fn nkba_report_of(p: &Project) -> NkbaReport {
    let rooms = detect_rooms(&p.floors[0].walls, 1.0);
    nkba_report(p, 0, &rooms, &[(0, "Kitchen".to_string())], &[])
}

#[test]
fn every_guideline_is_in_the_catalog_under_the_nkba_group() {
    let catalog = rule_catalog();
    for g in GUIDELINES {
        let info = catalog
            .iter()
            .find(|r| r.id == g.id)
            .unwrap_or_else(|| panic!("{} missing", g.id));
        assert_eq!(info.group, "NKBA");
        assert_eq!(info.severity, g.severity);
        assert_eq!(info.summary, g.requirement);
    }
    assert!(CheckSettings::groups().contains(&"NKBA"));
    assert!(GUIDELINES.len() >= 31, "{}", GUIDELINES.len());
    let mut ids: Vec<&str> = GUIDELINES.iter().map(|g| g.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), GUIDELINES.len(), "rule ids are unique");
}

#[test]
fn the_nkba_group_has_its_own_switch() {
    let mut p = Project::new("t");
    rect(&mut p, 0.0, 0.0, 200.0, 144.0, WallKind::Exterior);
    sym(&mut p, RANGE, (100.0, 3.0), (30.0, 25.0), 0.0, 0.0, 36.0);
    let rooms = detect_rooms(&p.floors[0].walls, 1.0);
    let types = [(0usize, "Kitchen".to_string())];
    let on = run_plan_check(&p, 0, &rooms, &types, &[]);
    assert!(on.findings.iter().any(|f| f.rule == nkba::K_VENT));
    let mut settings = CheckSettings::load(&p);
    assert!(settings.group_enabled("NKBA"));
    settings.set_group_enabled("NKBA", false);
    assert!(!settings.group_any_enabled("NKBA"));
    assert!(settings.group_enabled("Rooms"), "other groups stay on");
    settings.store(&mut p);
    let off = run_plan_check(&p, 0, &rooms, &types, &[]);
    assert!(!off
        .findings
        .iter()
        .any(|f| GUIDELINES.iter().any(|g| g.id == f.rule)));
    assert!(off.switched_off >= 1);
    // One rule off, the rest on.
    let mut one = CheckSettings::default();
    one.set_enabled(nkba::K_VENT, false);
    one.store(&mut p);
    let run = run_plan_check(&p, 0, &rooms, &types, &[]);
    assert!(!run.findings.iter().any(|f| f.rule == nkba::K_VENT));
    assert!(run.findings.iter().any(|f| f.rule == nkba::K_COOKTOP));
}

#[test]
fn every_finding_has_a_place_and_an_object_to_zoom_to() {
    // A kitchen and a bath that break many guidelines at once.
    let mut p = good_kitchen();
    remove_symbols(&mut p, "hood");
    cab_mut(&mut p, 3)["label"] = json!("");
    add_cab(&mut p, cab(9, "Base", (60.0, 60.0), 0.0, (48.0, 30.0)));
    let f = kitchen_findings(&p);
    let n = nkba_only(&f);
    assert!(n.len() >= 3, "{n:?}");
    for x in n {
        assert!(x.location.is_some(), "{}", x.rule);
        assert!(x.object.is_some(), "{}", x.rule);
        assert!(!x.fix.is_empty());
    }
    let mut b = good_bath();
    remove_symbols(&mut b, "mirror");
    remove_symbols(&mut b, "grab_bar");
    for x in nkba_only(&bath_findings(&b)) {
        assert!(x.location.is_some(), "{}", x.rule);
        assert!(x.object.is_some(), "{}", x.rule);
    }
}

#[test]
fn the_kitchen_and_bath_report_lists_every_guideline() {
    let p = good_kitchen();
    let r = nkba_report_of(&p);
    let kitchen_rules = GUIDELINES
        .iter()
        .filter(|g| g.area == nkba::Area::Kitchen)
        .count();
    let unchecked = r
        .rows
        .iter()
        .filter(|x| x.status == NkbaStatus::NotChecked)
        .count();
    assert_eq!(r.rows.len(), kitchen_rules + unchecked);
    assert!(unchecked >= 5);
    let (met, not_met) = r.counts();
    assert_eq!(
        not_met,
        0,
        "{:?}",
        r.rows
            .iter()
            .filter(|x| x.status == NkbaStatus::NotMet)
            .collect::<Vec<_>>()
    );
    assert!(met >= 10, "{met}");
    // The rows that have nothing to test are 'not in the plan'.
    let row = |name: &str| r.rows.iter().find(|x| x.guideline == name).unwrap();
    assert_eq!(row(nkba::K_SEAT_KNEE).status, NkbaStatus::NotInPlan);
    assert_eq!(row(nkba::K_DISHWASHER).status, NkbaStatus::Met);
    assert_eq!(row(nkba::K_TRIANGLE).status, NkbaStatus::Met);
    assert_eq!(row(nkba::K_SINK).status, NkbaStatus::Met);
    assert!(r.summary().starts_with("NKBA: "), "{}", r.summary());
    assert!(r.summary().ends_with("1 room"), "{}", r.summary());
}

#[test]
fn the_report_marks_failures_and_switched_off_rules() {
    let mut p = good_kitchen();
    remove_symbols(&mut p, "hood");
    let r = nkba_report_of(&p);
    let vent = r.rows.iter().find(|x| x.guideline == nkba::K_VENT).unwrap();
    assert_eq!(vent.status, NkbaStatus::NotMet);
    assert!(vent.detail.contains("no hood"));
    let mut s = CheckSettings::load(&p);
    s.set_enabled(nkba::K_VENT, false);
    s.store(&mut p);
    let r = nkba_report_of(&p);
    let vent = r.rows.iter().find(|x| x.guideline == nkba::K_VENT).unwrap();
    assert_eq!(vent.status, NkbaStatus::SwitchedOff);
}

#[test]
fn the_report_covers_both_the_kitchen_and_the_bath() {
    let mut p = Project::new("t");
    let a = rect(&mut p, 0.0, 0.0, 144.0, 144.0, WallKind::Exterior);
    let b = rect(&mut p, 144.0, 0.0, 264.0, 96.0, WallKind::Exterior);
    for id in [a[1], b[3]] {
        p.floors[0].wall_mut(id).unwrap().kind = WallKind::Interior;
    }
    sym(&mut p, RANGE, (50.0, 3.0), (30.0, 25.0), 0.0, 0.0, 36.0);
    sym(
        &mut p,
        "core.bathkitchen.toilet_elongated",
        (200.0, 147.0 - 144.0),
        (20.0, 28.0),
        0.0,
        0.0,
        30.0,
    );
    let rooms = detect_rooms(&p.floors[0].walls, 1.0);
    assert_eq!(rooms.len(), 2);
    let types = [(0usize, "Kitchen".to_string()), (1, "Bathroom".to_string())];
    let r = nkba_report(&p, 0, &rooms, &types, &[]);
    let rooms_seen: std::collections::BTreeSet<&str> = r.rows.iter().map(|x| x.area).collect();
    assert_eq!(rooms_seen.len(), 2);
    let table = r.table("1st Floor");
    assert_eq!(table.title, "Kitchen and Bath Report - 1st Floor");
    assert_eq!(table.columns.len(), 5);
    assert!(table.rows.iter().all(|row| row.len() == 5));
    let md = r.markdown();
    assert!(md.starts_with("# Kitchen and Bath Report"));
    assert!(
        md.contains("(Kitchen)") && md.contains("(Bathroom)"),
        "{md}"
    );
    assert!(md.contains("Not met"));
    // A plan with no kitchen or bath says so.
    let empty = nkba_report(&Project::new("e"), 0, &[], &[], &[]);
    assert!(empty.rows.is_empty());
    assert!(empty.markdown().contains("No kitchen or bathroom"));
    assert_eq!(empty.summary(), "NKBA: 0 met, 0 not met in 0 rooms");
}

#[test]
fn unnamed_rooms_are_not_judged() {
    let f = run(&good_kitchen(), &[]);
    assert!(nkba_only(&f).is_empty());
}
