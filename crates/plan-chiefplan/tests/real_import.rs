//! Tests against Daniel's real Chief projects. They read the files in place
//! (nothing is copied, nothing is written) and skip cleanly when a file is
//! absent. They are `#[ignore]`d because CI has no Chief install.
//!
//! Client project names stay out of the repository: the files are named by
//! environment variables.
//!
//! ```text
//! CHIEF_PLAN_A=".../Archives/<X18 construction set>/<job>.plan" \
//! CHIEF_PLAN_B=".../<second X18 job>.plan" \
//! CHIEF_PLAN_C=".../<X18 schematic design job>.plan" \
//! CHIEF_PLAN_X17=".../<job saved by X17>.plan" \
//!   cargo test -p plan-chiefplan --release --test real_import -- --ignored --nocapture
//! ```
//!
//! A is the reference job the assertions were written against (4 floors, 228
//! walls, one curved wall, a 34-window schedule); B and C only get the sanity
//! checks. The default template is found in the Templates folder.

use plan_chiefplan::import::{import_plan, ImportOptions, ImportResult};
use plan_core::model::Project;
use std::path::PathBuf;

/// The file named by environment variable `var`, if set and present.
fn env_plan(var: &str) -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os(var)?);
    p.exists().then_some(p)
}

/// `~/Documents/Chief Architect Premier X18 Data/<rel>`.
fn x18(rel: &str) -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("HOME")?)
        .join("Documents")
        .join("Chief Architect Premier X18 Data")
        .join(rel);
    p.exists().then_some(p)
}

const DEFAULT_TEMPLATE: &str = "Templates/x17 Working Template 2025-08-20.plan";

fn run(path: &PathBuf) -> ImportResult {
    let t = std::time::Instant::now();
    let r = import_plan(path, &ImportOptions::default()).expect("import");
    println!(
        "{}: {:.1}s\n{}",
        path.file_name().unwrap().to_string_lossy(),
        t.elapsed().as_secs_f64(),
        r.report.summary()
    );
    for f in &r.report.floors {
        println!(
            "  {:<11} elev {:>9.3} ceil {:>8.3} walls {:>3} openings {:>3} rooms {:>2} dims {:>3}",
            f.name, f.elevation, f.ceiling_height, f.walls, f.openings, f.rooms, f.dimensions
        );
    }
    r
}

/// The checks every imported project must pass.
fn sanity(p: &Project, expect_floors: usize) {
    let total_walls: usize = p.floors.iter().map(|f| f.walls.len()).sum();
    assert!(
        (20..=2000).contains(&total_walls),
        "{total_walls} walls is outside 20..=2000"
    );
    assert_eq!(p.floors.len(), expect_floors);
    let mut openings = 0usize;
    let mut off_wall = 0usize;
    let mut rooms = 0usize;
    let mut rooms_outside = 0usize;
    for f in &p.floors {
        // Every wall endpoint is finite and the floor fits in 60 m.
        let mut lo = (f64::MAX, f64::MAX);
        let mut hi = (f64::MIN, f64::MIN);
        for w in &f.walls {
            for pt in [w.start, w.end] {
                assert!(pt.x.is_finite() && pt.y.is_finite());
                assert!(pt.x.abs() < 50_000.0 && pt.y.abs() < 50_000.0, "{pt:?}");
                lo = (lo.0.min(pt.x), lo.1.min(pt.y));
                hi = (hi.0.max(pt.x), hi.1.max(pt.y));
            }
            assert!(
                w.length() > 0.0 && w.length() < 3000.0,
                "wall length {}",
                w.length()
            );
            assert!(w.thickness > 0.0 && w.thickness < 60.0);
            assert!(w.height >= 24.0 && w.height < 400.0, "height {}", w.height);
        }
        if !f.walls.is_empty() {
            assert!(
                hi.0 - lo.0 < 6000.0 && hi.1 - lo.1 < 6000.0,
                "floor box {lo:?} {hi:?}"
            );
        }
        // Openings lie on their wall.
        for o in &f.openings {
            openings += 1;
            let w = f.wall(o.wall_id).expect("host wall");
            assert!(o.center_offset >= 0.0 && o.center_offset <= w.length() + 0.5);
            if o.start_offset() < -1.0 || o.end_offset() > w.length() + 1.0 {
                off_wall += 1;
            }
            assert!(o.width > 0.0 && o.height > 0.0);
        }
        // Room anchors are inside the walls' box (24" margin).
        for r in &f.room_names {
            rooms += 1;
            if r.anchor.x < lo.0 - 24.0
                || r.anchor.x > hi.0 + 24.0
                || r.anchor.y < lo.1 - 24.0
                || r.anchor.y > hi.1 + 24.0
            {
                rooms_outside += 1;
            }
        }
    }
    println!("  openings {openings} (overhanging their wall: {off_wall}), room names {rooms} (outside walls' box: {rooms_outside})");
    assert!(
        off_wall * 10 <= openings.max(1),
        "{off_wall} of {openings} openings overhang"
    );
    assert!(
        rooms_outside * 10 <= rooms.max(1),
        "{rooms_outside} of {rooms} rooms outside"
    );
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn reference_job_a() {
    let Some(path) = env_plan("CHIEF_PLAN_A") else {
        eprintln!("skipped: CHIEF_PLAN_A is not set");
        return;
    };
    let r = run(&path);
    sanity(&r.project, 4);
    // Its window schedule lists 34 windows; its four floors hold 228 walls.
    assert_eq!(r.report.counts["windows"], 34);
    assert_eq!(r.report.counts["walls"], 228);
    assert!(r.report.counts["doors"] >= 40);
    // The first floor's exterior walls are Brick-6 and Brick-11.
    let f1 = &r.project.floors[1];
    assert!(f1
        .walls
        .iter()
        .any(|w| w.wall_type.as_deref() == Some("Brick-6")));
    assert!(f1
        .walls
        .iter()
        .any(|w| w.wall_type.as_deref() == Some("Interior-6")));
    let brick = r
        .project
        .wall_types
        .iter()
        .find(|t| t.name == "Brick-6")
        .expect("Brick-6");
    assert!((brick.thickness() - 11.51).abs() < 0.01);
    assert!(f1.room_names.iter().any(|n| n.name == "F. Porch"));
    // The project survives a save and load.
    let json = r.project.to_json().expect("json");
    let back = Project::from_json(&json).expect("reload");
    assert_eq!(back.floors.len(), r.project.floors.len());
    assert_eq!(back.floors[1].walls.len(), f1.walls.len());
    // One curved wall (a quarter circle of radius 24.27") on the second floor.
    assert!(r.report.counts["curved_walls"] >= 1);
    let curved: Vec<_> = r
        .project
        .floors
        .iter()
        .flat_map(|f| f.walls.iter())
        .filter(|w| w.curve.is_some())
        .collect();
    let (_, radius) = curved[0]
        .curve
        .unwrap()
        .arc_center_radius(curved[0].start, curved[0].end)
        .unwrap();
    assert!((radius - 24.27).abs() < 0.5, "radius {radius}");
    assert_eq!(r.project.floors[1].ceiling_height, 121.125);
    assert_eq!(r.project.floors[0].elevation, -132.25);
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn second_job_b() {
    let Some(path) = env_plan("CHIEF_PLAN_B") else {
        eprintln!("skipped: CHIEF_PLAN_B is not set");
        return;
    };
    let r = run(&path);
    sanity(&r.project, 4);
    assert!(r.report.counts["walls"] > 150);
    assert!(r.report.counts["windows"] >= 30);
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn schematic_design_job_c() {
    let Some(path) = env_plan("CHIEF_PLAN_C") else {
        eprintln!("skipped: CHIEF_PLAN_C is not set");
        return;
    };
    let r = run(&path);
    // The third floor holds only notes.
    sanity(&r.project, 4);
    assert!(r.report.counts["doors"] > 50 && r.report.counts["windows"] > 30);
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn job_saved_by_x17() {
    let Some(path) = env_plan("CHIEF_PLAN_X17") else {
        eprintln!("skipped: CHIEF_PLAN_X17 is not set");
        return;
    };
    let r = run(&path);
    sanity(&r.project, 4);
    assert!(r.report.generation.contains("X17"));
    // Wall types decode with the 377-byte X17 records.
    assert!(r.project.floors[1]
        .walls
        .iter()
        .all(|w| w.wall_type.is_some()));
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn the_default_template_has_no_geometry() {
    let Some(path) = x18(DEFAULT_TEMPLATE) else {
        eprintln!("skipped: {DEFAULT_TEMPLATE} not found");
        return;
    };
    let r = run(&path);
    assert_eq!(r.report.counts["walls"], 0);
    assert_eq!(r.report.counts["doors"], 0);
    // Its four floors keep only the stock notes the template carries.
    assert_eq!(r.project.floors.len(), 4);
    assert!(r.project.floors.iter().all(|f| f.walls.is_empty()));
    // Its own wall types and layers still seed the project.
    assert!(r.report.counts["wall_types"] > 50);
    assert!(r.project.wall_types.len() > 50);
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn reference_job_a_walls_close_into_rooms() {
    let Some(path) = env_plan("CHIEF_PLAN_A") else {
        eprintln!("skipped: CHIEF_PLAN_A is not set");
        return;
    };
    let r = import_plan(&path, &ImportOptions::default()).expect("import");
    for (fi, tol) in [(1usize, 6.0), (1, 12.0)] {
        let f = &r.project.floors[fi];
        let rooms = plan_core::detect_rooms(&f.walls, tol);
        let inside = f
            .room_names
            .iter()
            .filter(|n| rooms.iter().any(|room| room.contains(n.anchor)))
            .count();
        println!(
            "{} (tol {tol}): {} rooms detected from {} walls; {} of {} named anchors inside one",
            f.name,
            rooms.len(),
            f.walls.len(),
            inside,
            f.room_names.len()
        );
    }
}
