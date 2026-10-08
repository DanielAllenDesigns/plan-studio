//! Tests of the object stage against Daniel's real Chief projects: cabinets,
//! library objects, electrical devices, stairs, roof planes and room labels.
//! They read the files in place (nothing is copied or written) and skip
//! cleanly when a variable is not set. Client names stay out of the repository:
//! the jobs are named by environment variables, as in `real_import.rs`.
//!
//! ```text
//! CHIEF_PLAN_A=... CHIEF_PLAN_B=... CHIEF_PLAN_C=... CHIEF_PLAN_X17=... \
//!   cargo test -p plan-chiefplan --release --test real_objects -- --ignored --nocapture
//! ```
//!
//! A is the X18 construction set (228 walls), B a second X18 construction set
//! (212 walls), C the schematic-design job (303 walls, no electrical and no
//! roof), X17 a job saved by X17.

use plan_chiefplan::import::{import_plan, ImportOptions, ImportResult};
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::model::Floor;
use serde_json::Value;
use std::path::PathBuf;

fn env_plan(var: &str) -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os(var)?);
    p.exists().then_some(p)
}

fn run(path: &PathBuf) -> ImportResult {
    let t = std::time::Instant::now();
    let r = import_plan(path, &ImportOptions::default()).expect("import");
    let secs = t.elapsed().as_secs_f64();
    println!(
        "{}: {:.2}s\n{}",
        path.file_name().unwrap().to_string_lossy(),
        secs,
        r.report.summary().lines().next().unwrap_or_default()
    );
    assert!(secs < 2.0, "import took {secs:.2}s");
    r
}

/// The walls' bounding box of a floor, `(min x, min y, max x, max y)`.
fn walls_box(f: &Floor) -> Option<(f64, f64, f64, f64)> {
    let mut it = f.walls.iter().flat_map(|w| [w.start, w.end]);
    let p = it.next()?;
    let mut b = (p.x, p.y, p.x, p.y);
    for p in it {
        b = (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y));
    }
    Some(b)
}

fn inside(b: (f64, f64, f64, f64), x: f64, y: f64, pad: f64) -> bool {
    x >= b.0 - pad && x <= b.2 + pad && y >= b.1 - pad && y <= b.3 + pad
}

fn num(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("{k} in {v}"))
}

fn pt(v: &Value) -> (f64, f64) {
    (num(v, "x"), num(v, "y"))
}

/// Distance from `p` to the nearest wall centreline and that wall's half
/// thickness.
fn nearest_wall(f: &Floor, p: (f64, f64)) -> Option<(f64, f64)> {
    f.walls
        .iter()
        .map(|w| {
            (
                dist_to_segment(Point::new(p.0, p.1), w.start, w.end),
                w.thickness / 2.0,
            )
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
}

/// Counts of cabinets whose back sits against a wall and whose front faces away.
fn cabinet_wall_stats(f: &Floor) -> (usize, usize, usize) {
    let (mut boxes, mut backed, mut facing) = (0, 0, 0);
    for c in &f.cabinets {
        let kind = c["kind"].as_str().unwrap_or("");
        if matches!(kind, "CustomCountertop" | "Soffit") {
            continue;
        }
        boxes += 1;
        let a = num(c, "angle");
        let (px, py) = pt(&c["position"]);
        let w = num(c, "width");
        let back = (px + w / 2.0 * a.cos(), py + w / 2.0 * a.sin());
        let Some((d, half)) = nearest_wall(f, back) else {
            continue;
        };
        if d <= half + 3.0 {
            backed += 1;
            let front = (
                back.0 - a.sin() * num(c, "depth") / 2.0,
                back.1 + a.cos() * num(c, "depth") / 2.0,
            );
            if nearest_wall(f, front).is_some_and(|(d2, _)| d2 > d + 1.0) {
                facing += 1;
            }
        }
    }
    (boxes, backed, facing)
}

fn objects_sanity(r: &ImportResult, expect_cabinets: usize, has_electrical: bool, has_roof: bool) {
    let counts = &r.report.counts;
    println!(
        "  cabinets {} (soffits {}, countertops {}), symbols {} (in cabinets {}), devices {} (on wall {}), stairs {} + {} landings, roof planes {} (duplicates {}), labels {} applied {}",
        counts["cabinets"],
        counts["cabinet_soffits"],
        counts["countertops"],
        counts["symbols"],
        counts["symbols_in_cabinets"],
        counts["electrical_devices"],
        counts["electrical_on_wall"],
        counts["stairs"],
        counts["stair_landings"],
        counts["roof_planes"],
        counts["roof_duplicates"],
        counts["room_labels"],
        counts["room_labels_applied"],
    );
    assert!(counts["cabinets"] >= expect_cabinets);
    let (mut boxes, mut backed, mut facing) = (0, 0, 0);
    let mut sym_total = 0;
    let mut sym_in_rooms = 0;
    for f in &r.project.floors {
        let Some(b) = walls_box(f) else { continue };
        // Every cabinet position lies inside the walls' box (24" margin).
        for c in &f.cabinets {
            let (x, y) = pt(&c["position"]);
            assert!(inside(b, x, y, 24.0), "cabinet at {x},{y} outside {b:?}");
            let w = num(c, "width");
            assert!(w > 0.0 && w < 400.0 || c["kind"] == "CustomCountertop");
        }
        let (a, bk, fc) = cabinet_wall_stats(f);
        boxes += a;
        backed += bk;
        facing += fc;
        // Symbols sit inside the walls' box, mostly inside a detected room.
        let rooms = plan_core::detect_rooms(&f.walls, 12.0);
        for s in &f.symbols {
            assert!(
                inside(b, s.position.x, s.position.y, 60.0),
                "{} at {:?}",
                s.label,
                s.position
            );
            sym_total += 1;
            let a = s.angle.to_radians();
            // The footprint centre: the back centre moved half a depth along the front.
            let c = Point::new(
                s.position.x - a.sin() * s.depth / 2.0,
                s.position.y + a.cos() * s.depth / 2.0,
            );
            if rooms.iter().any(|room| room.contains(c)) {
                sym_in_rooms += 1;
            }
            assert!(s.width > 0.0 && s.depth > 0.0 && s.height > 0.0);
        }
        // Electrical devices sit inside the box; wall devices have a host wall.
        if let Some(e) = f.electrical.as_ref() {
            for d in e["devices"].as_array().unwrap() {
                let (x, y) = pt(&d["position"]);
                assert!(inside(b, x, y, 100.0), "device at {x},{y}");
            }
        }
    }
    println!(
        "  cabinet boxes {boxes}: backs on a wall {backed}, fronts facing away {facing}; symbols in a detected room {sym_in_rooms} of {sym_total}"
    );
    assert!(
        backed * 100 >= boxes * 80,
        "{backed} of {boxes} cabinets are against a wall"
    );
    assert!(
        facing * 100 >= backed * 90,
        "{facing} of {backed} face away"
    );
    assert!(
        sym_in_rooms * 100 >= sym_total * 60,
        "{sym_in_rooms} of {sym_total} symbols in a room"
    );
    if has_electrical {
        assert!(counts["electrical_devices"] > 200);
        let (mut wall, mut hosted) = (0, 0);
        for f in &r.project.floors {
            if let Some(e) = f.electrical.as_ref() {
                for d in e["devices"].as_array().unwrap() {
                    if matches!(
                        d["kind"].as_str().unwrap(),
                        "Outlet110" | "Gfci" | "Switch" | "Switch3Way" | "Switch4Way"
                    ) {
                        wall += 1;
                        hosted += usize::from(!d["wall_id"].is_null());
                    }
                }
            }
        }
        println!("  wall devices {wall}, with a host wall {hosted}");
        assert!(
            hosted * 100 >= wall * 95,
            "{hosted} of {wall} devices on a wall"
        );
    } else {
        assert_eq!(counts["electrical_devices"], 0);
    }
    if has_roof {
        assert!(counts["roof_planes"] >= 20);
        // The roof planes cover the footprint of the highest floor with walls
        // below the roof: their joint box contains it (within the overhang).
        let mut roof_box = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut total_area = 0.0;
        for f in &r.project.floors {
            for rec in &f.roofs {
                let poly = rec["polygon3d"].as_array().unwrap();
                let pts: Vec<(f64, f64)> = poly
                    .iter()
                    .map(|v| (v[0].as_f64().unwrap(), -v[2].as_f64().unwrap()))
                    .collect();
                for &(x, y) in &pts {
                    roof_box = (
                        roof_box.0.min(x),
                        roof_box.1.min(y),
                        roof_box.2.max(x),
                        roof_box.3.max(y),
                    );
                }
                let mut a = 0.0;
                for k in 0..pts.len() {
                    let (p, q) = (pts[k], pts[(k + 1) % pts.len()]);
                    a += p.0 * q.1 - q.0 * p.1;
                }
                total_area += (a / 2.0).abs();
                let pitch = num(rec, "pitch");
                assert!((0.0..=24.0).contains(&pitch), "pitch {pitch}");
                for v in poly {
                    let z = v[1].as_f64().unwrap();
                    assert!((40.0..700.0).contains(&z), "height {z}");
                }
                // Pitch is a round number of twelfths (quarter-inch steps).
                assert!(
                    (pitch * 4.0 - (pitch * 4.0).round()).abs() < 0.02,
                    "pitch {pitch}"
                );
            }
        }
        let top_walls = r
            .project
            .floors
            .iter()
            .rfind(|f| f.walls.len() > 20 && f.kind == plan_core::floors::FloorKind::Normal)
            .and_then(walls_box)
            .expect("a living floor");
        println!(
            "  roof box {roof_box:?} vs top living floor {top_walls:?}, area {:.0} sq ft",
            total_area / 144.0
        );
        assert!(roof_box.0 <= top_walls.0 + 6.0 && roof_box.1 <= top_walls.1 + 6.0);
        assert!(roof_box.2 >= top_walls.2 - 6.0 && roof_box.3 >= top_walls.3 - 6.0);
        let footprint = (top_walls.2 - top_walls.0) * (top_walls.3 - top_walls.1);
        assert!(
            total_area >= footprint * 0.5,
            "roof area {total_area} vs box {footprint}"
        );
    }
    // Stairs: at least one flight on a floor with walls, inside its box.
    let mut flights = 0;
    for f in &r.project.floors {
        let Some(b) = walls_box(f) else { continue };
        for s in &f.stairs {
            let (x, y) = pt(&s["origin"]);
            assert!(inside(b, x, y, 120.0), "stair at {x},{y} outside {b:?}");
            if s["params"]["shape"] == "Straight" {
                flights += 1;
                let tread = num(&s["params"], "tread_depth");
                assert!((6.0..=16.0).contains(&tread));
                let width = num(&s["params"], "width");
                assert!((20.0..=140.0).contains(&width));
            }
        }
    }
    assert!(flights >= 3, "{flights} flights");
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn objects_of_job_a() {
    let Some(path) = env_plan("CHIEF_PLAN_A") else {
        eprintln!("skipped: CHIEF_PLAN_A is not set");
        return;
    };
    let r = run(&path);
    objects_sanity(&r, 50, true, true);
    // Job A's first floor names its rooms from typed labels as well.
    assert!(r.report.counts["room_labels_applied"] >= 3);
    let json = r.project.to_json().unwrap();
    let back = plan_core::Project::from_json(&json).unwrap();
    assert_eq!(
        back.floors[1].cabinets.len(),
        r.project.floors[1].cabinets.len()
    );
    assert_eq!(
        back.floors[1].symbols.len(),
        r.project.floors[1].symbols.len()
    );
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn objects_of_job_b() {
    let Some(path) = env_plan("CHIEF_PLAN_B") else {
        eprintln!("skipped: CHIEF_PLAN_B is not set");
        return;
    };
    let r = run(&path);
    objects_sanity(&r, 50, true, true);
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn objects_of_the_schematic_job_c() {
    let Some(path) = env_plan("CHIEF_PLAN_C") else {
        eprintln!("skipped: CHIEF_PLAN_C is not set");
        return;
    };
    let r = run(&path);
    // Schematic design: cabinets and stairs but no electrical and no roof.
    objects_sanity(&r, 80, false, false);
    assert_eq!(r.report.counts["roof_planes"], 0);
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn x17_job_gets_dimensions_rooms_and_roofs() {
    let Some(path) = env_plan("CHIEF_PLAN_X17") else {
        eprintln!("skipped: CHIEF_PLAN_X17 is not set");
        return;
    };
    let r = run(&path);
    assert!(r.report.generation.contains("X17"));
    println!(
        "  X17: {} dimensions, {} named rooms of {}",
        r.report.counts["dimensions"], r.report.counts["rooms_named"], r.report.counts["rooms"]
    );
    // 554-byte dimension records and the X17 room layout now decode.
    assert!(r.report.counts["dimensions"] > 100);
    assert!(r.report.counts["rooms_named"] > 15);
    assert!(r.report.counts["roof_planes"] >= 10);
    // The roof angle sits one byte earlier than in X18 files: pitches are still
    // round numbers.
    objects_sanity(&r, 40, true, true);
}
