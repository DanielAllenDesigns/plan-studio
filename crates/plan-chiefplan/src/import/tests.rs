//! Synthetic end-to-end tests of the importer: every file is built in the
//! test (no Chief file is read or copied).

use super::dims::tests::dim_obj;
use super::floors::tests::floor_obj;
use super::openings::tests::{opening_obj, opening_obj_flags};
use super::rooms::tests::room_obj;
use super::texts::tests::text_obj;
use super::tree::testutil::*;
use super::walls::tests::{line_obj, point_obj, wall_obj, wall_type_obj};
use super::*;
use crate::scan::testutil::build_template;
use plan_core::model::{OpeningKind, WallKind};
use serde_json::json;

fn types_body() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(with_id(
        581,
        wall_type_obj(
            "Brick-6",
            &[
                (4.0, 1, false),
                (1.0, 2, false),
                (5.5, 3, true),
                (0.5, 4, false),
            ],
        ),
    ));
    b.extend(with_id(
        351,
        wall_type_obj(
            "Interior-6",
            &[(0.5, 4, false), (5.5, 3, true), (0.5, 4, false)],
        ),
    ));
    b.extend(with_id(
        218,
        wall_type_obj("10\" Concrete Stem Wall", &[(10.0, 5, true)]),
    ));
    b
}

/// Foundation, first floor, an empty second floor.
fn house() -> Vec<u8> {
    let mut body = types_body();
    // Foundation: one stem wall.
    let stem = wall_obj(line_obj(0.0, 0.0, 1.0, 0.0, 400.0), Some(218), None, &[]);
    body.extend(floor_obj(-132.25, 115.5, &[stem]));
    // First floor: a rectangle of exterior walls (counter-clockwise), one
    // interior wall with a window and a door, a room, a dimension, a cabinet.
    let ext = |x: f64, y: f64, dx: f64, dy: f64, len: f64| {
        wall_obj(line_obj(x, y, dx, dy, len), Some(581), Some(121.125), &[])
    };
    let win = opening_obj(10, 0x312, [62.0, 32.0, 0.625, 54.0, 84.0], b"");
    let door = opening_obj_flags(9, 0x312, [150.0, 32.0, 0.625, 96.0, 96.0], b"", (1, 1));
    let partition = wall_obj(
        line_obj(200.0, 0.0, 0.0, 1.0, 300.0),
        Some(351),
        Some(121.125),
        &[win, door, point_obj(200.0, 0.0)],
    );
    let kids = vec![
        ext(0.0, 0.0, 1.0, 0.0, 400.0),
        ext(400.0, 0.0, 0.0, 1.0, 300.0),
        ext(400.0, 300.0, -1.0, 0.0, 400.0),
        ext(0.0, 300.0, 0.0, -1.0, 300.0),
        partition,
        room_obj("Default", Some("Kitchen"), (190.0, 290.0), (100.0, 150.0)),
        room_obj("Default", None, (190.0, 290.0), (300.0, 150.0)),
        dim_obj(&[(10.0, 10.0), (210.0, 10.0), (410.0, 10.0)]),
        sized(15, 0, 0x300, |_| {}),
        text_obj("10' CEILING HT", false, 100.0, 120.0, -55),
        text_obj("Wall Layer 2 - Viewed From Outside", false, 10.0, 10.0, -23),
    ];
    body.extend(floor_obj(0.0, 121.125, &kids));
    body.extend(floor_obj(137.875, 109.125, &[]));
    body
}

#[test]
fn empty_plan_reports_zeroes() {
    let bytes = build_template(&[], &[]);
    let r = import_bytes(&bytes, "Empty.plan", &ImportOptions::default()).unwrap();
    assert_eq!(r.project.floors.len(), 1);
    assert_eq!(r.report.counts["walls"], 0);
    assert_eq!(r.report.counts["floors"], 1);
    assert!(r
        .report
        .warnings
        .iter()
        .any(|w| w.contains("no floor objects")));
    assert_eq!(r.report.file_name, "Empty.plan");
    assert_eq!(r.project.name, "Empty");
    assert!(r.report.summary().contains("0 walls"));
}

#[test]
fn small_house_round_trip() {
    let bytes = build_template(&house(), &[]);
    let r = import_bytes(&bytes, "House.plan", &ImportOptions::default()).unwrap();
    let p = &r.project;
    let rep = &r.report;
    // The empty trailing floor is dropped.
    assert_eq!(p.floors.len(), 2, "{:?}", rep.floors);
    assert_eq!(p.floors[0].name, "Foundation");
    assert_eq!(p.floors[0].kind, FloorKind::Foundation);
    assert_eq!(p.floors[0].elevation, -132.25);
    assert_eq!(p.floors[1].name, "1st Floor");
    assert_eq!(p.floors[1].ceiling_height, 121.125);
    assert_eq!(rep.counts["walls"], 6);
    assert_eq!(p.floors[0].walls.len(), 1);
    assert_eq!(p.floors[1].walls.len(), 5);
    assert_eq!(rep.counts["doors"], 1);
    assert_eq!(rep.counts["windows"], 1);
    assert_eq!(rep.counts["rooms"], 2);
    assert_eq!(rep.counts["rooms_named"], 1);
    assert_eq!(rep.counts["dimensions"], 2);
    assert_eq!(rep.counts["wall_types"], 3);
    assert_eq!(rep.counts["texts"], 1);

    // Walls: stem wall, exterior brick (shifted, exterior on the right),
    // interior partition.
    let stem = &p.floors[0].walls[0];
    assert_eq!(stem.wall_type.as_deref(), Some("10\" Concrete Stem Wall"));
    assert!(stem.is_foundation());
    assert_eq!(stem.layer.replace("  ", " "), "Walls, Foundation");
    assert!((stem.thickness - 10.0).abs() < 1e-9);
    assert_eq!(stem.height, 115.5);
    let south = &p.floors[1].walls[0];
    assert_eq!(south.wall_type.as_deref(), Some("Brick-6"));
    assert_eq!(south.kind, WallKind::Exterior);
    assert!((south.thickness - 11.0).abs() < 1e-9);
    assert!((south.height - 121.125).abs() < 1e-9);
    assert!(south.start.y < 0.0, "shifted to the right of a +x wall");
    let part = &p.floors[1].walls[4];
    assert_eq!(part.wall_type.as_deref(), Some("Interior-6"));
    assert_eq!(part.kind, WallKind::Interior);
    assert!((part.thickness - 6.5).abs() < 1e-9);
    // The partition ends were healed to the exterior walls' centrelines
    // (4.75" beyond the reference lines at each end).
    assert!((part.length() - 309.5).abs() < 1e-6, "{}", part.length());
    assert!(rep.counts["wall_ends_joined"] >= 2);

    // The file's own definitions are registered under the used names.
    let brick = p.wall_types.iter().find(|t| t.name == "Brick-6").unwrap();
    assert!((brick.thickness() - 11.0).abs() < 1e-9);

    // Openings sit on the partition, inside its length.
    let ops = &p.floors[1].openings;
    assert_eq!(ops.len(), 2);
    for o in ops {
        assert_eq!(o.wall_id, part.id);
        assert!(o.start_offset() >= 0.0 && o.end_offset() <= part.length());
    }
    let w = ops.iter().find(|o| o.kind == OpeningKind::Window).unwrap();
    // The window stays 62" from the original start: 4.75" more from the
    // healed one.
    assert!(
        (w.center_offset - 66.75).abs() < 1e-6,
        "{}",
        w.center_offset
    );
    assert_eq!((w.width, w.height, w.sill_height), (32.0, 54.0, 30.0));
    let d = ops.iter().find(|o| o.kind == OpeningKind::Door).unwrap();
    assert!(
        (d.center_offset - 154.75).abs() < 1e-6,
        "{}",
        d.center_offset
    );
    assert_eq!((d.width, d.height, d.sill_height), (32.0, 96.0, 0.0));

    // Room names with anchors inside the walls' box.
    let names = &p.floors[1].room_names;
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].name, "Kitchen");
    assert!(names[0].anchor.x > 0.0 && names[0].anchor.x < 400.0);

    // Dimensions: two segments, 200" each.
    let dims = &p.floors[1].dimensions;
    assert_eq!(dims.len(), 2);
    assert!((dims[0].length() - 200.0).abs() < 1e-9);

    // The note became CAD text: top-centre anchor -> bottom-left corner.
    let note = &p.floors[1].cad;
    assert_eq!(note.len(), 1);
    match &note[0].item {
        plan_core::CadItem::Text {
            pos, text, height, ..
        } => {
            assert_eq!(text, "10' CEILING HT");
            assert_eq!(*height, 4.5);
            assert!(pos.y < 120.0 && pos.x < 100.0);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(note[0].layer, "Text");
    // The cabinet decoy is reported as skipped, with its label.
    let cab = rep.skipped_classes.iter().find(|s| s.class == 15).unwrap();
    assert_eq!(cab.count, 1);
    assert!(cab.label.contains("cabinet"));
    // Ids are unique across walls and openings.
    let mut ids: Vec<Id> = p
        .floors
        .iter()
        .flat_map(|f| {
            f.walls
                .iter()
                .map(|w| w.id)
                .chain(f.openings.iter().map(|o| o.id))
        })
        .collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n);
    // The whole project survives JSON.
    let json = p.to_json().unwrap();
    assert!(Project::from_json(&json).is_ok());
    assert!(rep.floors.iter().map(|f| f.walls).sum::<usize>() == 6);
    assert!(rep.warnings.iter().any(|w| w.contains("door style")));
    // The door's flag bytes (side 1, hinge 1): hinge at the end, swing right.
    assert!(d.hinge_at_end && d.swing_flipped);
    assert!(!w.hinge_at_end && !w.swing_flipped);
}

#[test]
fn options_switch_dimensions_and_seeding_off() {
    let bytes = build_template(&house(), &[]);
    let opts = ImportOptions {
        project_name: Some("Renamed".into()),
        seed_from_file: false,
        dimensions: false,
        text: false,
        ..ImportOptions::default()
    };
    let r = import_bytes(&bytes, "House.plan", &opts).unwrap();
    assert_eq!(r.project.name, "Renamed");
    assert_eq!(r.report.counts["dimensions"], 0);
    assert!(r.project.floors[1].dimensions.is_empty());
    assert!(r.project.floors[1].cad.is_empty());
    assert_eq!(r.report.counts["walls"], 6);
}

#[test]
fn unresolved_wall_types_are_reported() {
    let mut body = types_body();
    let w = wall_obj(
        line_obj(0.0, 0.0, 1.0, 0.0, 100.0),
        Some(9999),
        Some(121.125),
        &[],
    );
    body.extend(floor_obj(0.0, 121.125, &[w]));
    let bytes = build_template(&body, &[]);
    let r = import_bytes(&bytes, "X.plan", &ImportOptions::default()).unwrap();
    assert_eq!(r.report.counts["walls"], 1);
    assert!(r.report.warnings.iter().any(|w| w.contains("wall type")));
    assert_eq!(r.project.floors[0].walls[0].thickness, 5.5);
}

#[test]
fn layout_files_and_missing_files_are_errors() {
    let dir = std::env::temp_dir().join(format!("plan-chiefplan-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let layout = dir.join("Sheets.layout");
    std::fs::write(&layout, build_template(&[], &[])).unwrap();
    assert!(import_plan(&layout, &ImportOptions::default()).is_err());
    assert!(import_plan(dir.join("missing.plan"), &ImportOptions::default()).is_err());
    let plan = dir.join("A.plan");
    std::fs::write(&plan, build_template(&house(), &[])).unwrap();
    let r = import_plan(&plan, &ImportOptions::default()).unwrap();
    assert_eq!(r.project.name, "A");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn garbage_bodies_do_not_panic() {
    let mut body = vec![0u8; 2000];
    body.extend_from_slice(&[0x01, 0xCD, 0xAB, 6, 0, 0xFF, 0xFF, 0xFF, 0x7F]);
    body.extend_from_slice(&[0x01, 0xCD, 0xAB, 30, 0, 0x08, 0, 0, 0, 1, 2, 3, 4]);
    body.extend_from_slice(&[0xCD, 0xAB, 23, 0, 4, 0, 0, 0]);
    let bytes = build_template(&body, &[]);
    let r = import_bytes(&bytes, "Junk.plan", &ImportOptions::default()).unwrap();
    assert_eq!(r.report.counts["walls"], 0);
}

#[test]
fn corrupted_houses_never_panic() {
    // Deterministic byte flips and truncations of valid files, with and without
    // the object stage's classes: every decoder is bounds-checked, so none of
    // these may panic.
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for original in [
        build_template(&house(), &[]),
        build_template(&objects_house(), &[]),
    ] {
        for round in 0..120 {
            let mut bytes = original.clone();
            for _ in 0..(1 + round % 12) {
                let at = (next() as usize) % bytes.len();
                bytes[at] = (next() & 0xFF) as u8;
            }
            if round % 7 == 0 {
                let cut = 64 + (next() as usize) % (bytes.len() - 64);
                bytes.truncate(cut);
            }
            // Errors are fine (a damaged header); panics are not.
            let _ = import_bytes(&bytes, "Fuzz.plan", &ImportOptions::default());
        }
    }
}

/// A first floor with walls, a room and one object of each kind the object
/// stage reads, plus an attic floor holding a roof plane.
fn objects_house() -> Vec<u8> {
    use super::cabinets::tests::cabinet_obj;
    use super::electrical::tests::device_obj;
    use super::labels::tests::label_obj;
    use super::lines::tests::put_rect;
    use super::roofs::tests::plane_obj;
    use super::stairs::tests::flight_obj;
    use super::symbols::tests::symbol_obj;
    let mut body = types_body();
    let ext = |x: f64, y: f64, dx: f64, dy: f64, len: f64| {
        wall_obj(line_obj(x, y, dx, dy, len), Some(581), Some(121.125), &[])
    };
    let partition = wall_obj(
        line_obj(200.0, 0.0, 0.0, 1.0, 300.0),
        Some(351),
        Some(121.125),
        &[],
    );
    let kids = vec![
        ext(0.0, 0.0, 1.0, 0.0, 400.0),
        ext(400.0, 0.0, 0.0, 1.0, 300.0),
        ext(400.0, 300.0, -1.0, 0.0, 400.0),
        ext(0.0, 300.0, 0.0, -1.0, 300.0),
        partition,
        room_obj("Default", None, (190.0, 290.0), (100.0, 150.0)),
        label_obj("kitchen", 100.0, 100.0, 180.0, 150.0),
        cabinet_obj(
            0x300,
            [100.0, 24.0, 0.0, 1.0, 24.0, 36.0, 36.0, 36.0],
            2,
            &["Lincoln Door", "Lincoln Flat Panel Drawer"],
            false,
        ),
        symbol_obj(
            [150.0, 100.0, 0.0, 1.0, 30.0, 30.0, 30.0, 30.0],
            "Elongated Toilet",
            &["ADA"],
        ),
        device_obj(
            751,
            203.0,
            150.0,
            &["Duplex", "110V", "Outlets", "Wall Mounted"],
        ),
        flight_obj(
            (300.0, 100.0, 0.0, 1.0, 63.0),
            48.0,
            10.5,
            137.875,
            137.875 / 18.0,
        ),
        sized(47, 0, 2000, |b| put_rect(b, 671, 280.0, 160.0, 48.0, 46.0)),
    ];
    body.extend(floor_obj(0.0, 121.125, &kids));
    let roof = plane_obj(
        &[
            (-12.0, -12.0),
            (412.0, -12.0),
            (412.0, 312.0),
            (-12.0, 312.0),
        ],
        (0.0, 0.0, 1.0, 0.0, 400.0),
        129.84,
        (8.0f64 / 12.0).atan(),
    );
    body.extend(floor_obj(137.875, 109.125, &[roof]));
    body
}

#[test]
fn object_stage_end_to_end() {
    let bytes = build_template(&objects_house(), &[]);
    let r = import_bytes(&bytes, "Objects.plan", &ImportOptions::default()).unwrap();
    let (p, rep) = (&r.project, &r.report);
    assert_eq!(p.floors.len(), 2, "{:?}", rep.floors);
    assert_eq!(p.floors[1].name, "Attic");
    let f = &p.floors[0];

    // Cabinet: a base with a top, front toward +y so the angle is zero and the
    // origin is half a width left of the back centre.
    assert_eq!(f.cabinets.len(), 1);
    let c = &f.cabinets[0];
    assert_eq!(c["kind"], "Base");
    assert_eq!(c["angle"], 0.0);
    assert_eq!(c["position"]["x"], 100.0 - 18.0);
    assert_eq!(c["position"]["y"], 24.0);
    assert_eq!(c["countertop"]["thickness"], 1.5);
    assert_eq!(c["door_style"]["name"], "Lincoln Door");
    assert_eq!(rep.counts["cabinets"], 1);

    // Library object: a stand-in id from the name, angle in degrees.
    assert_eq!(f.symbols.len(), 1);
    let s = &f.symbols[0];
    assert_eq!(s.catalog_id, "chief-plan.elongated-toilet");
    assert_eq!((s.position.x, s.position.y), (150.0, 100.0));
    assert_eq!(s.angle, 0.0);
    assert_eq!(s.label, "Elongated Toilet");
    assert_eq!(rep.counts["symbols"], 1);

    // Electrical: the outlet sits on the partition and faces +x (the right of
    // a wall running +y).
    let layer = f.electrical.as_ref().unwrap();
    let devices = layer["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 1);
    let partition = &f.walls[4];
    assert_eq!(devices[0]["kind"], "Outlet110");
    assert_eq!(devices[0]["wall_id"], partition.id);
    assert!(devices[0]["angle"].as_f64().unwrap().abs() < 1e-9);
    assert_eq!(devices[0]["height"], 12.0);
    assert_eq!(rep.counts["electrical_on_wall"], 1);

    // Stairs: a flight of 6 treads and a landing.
    assert_eq!(f.stairs.len(), 2);
    assert_eq!(f.stairs[0]["params"]["shape"], "Straight");
    assert_eq!(f.stairs[1]["params"]["shape"]["Landing"]["depth"], 48.0);
    assert_eq!((rep.counts["stairs"], rep.counts["stair_landings"]), (1, 1));

    // Roof plane on the attic floor: 8/12 pitch, vertices follow the plane.
    let roofs = &p.floors[1].roofs;
    assert_eq!(roofs.len(), 1);
    assert!((roofs[0]["pitch"].as_f64().unwrap() - 8.0).abs() < 1e-9);
    assert_eq!(roofs[0]["polygon3d"].as_array().unwrap().len(), 4);
    assert_eq!(rep.counts["roof_planes"], 1);

    // The label named the unnamed room, at the label frame's centre.
    assert_eq!(rep.counts["room_labels_applied"], 1);
    let n = f.room_names.iter().find(|n| n.name == "kitchen").unwrap();
    assert_eq!((n.anchor.x, n.anchor.y), (190.0, 175.0));

    // Nothing of the object stage is reported as skipped, and the summary
    // names the new counts.
    for class in [15u8, 21, 46, 47, 48, 50, 123] {
        assert!(
            !rep.skipped_classes.iter().any(|s| s.class == class),
            "class {class} listed as skipped"
        );
    }
    let summary = rep.summary();
    assert_eq!(summary.lines().next().unwrap(), rep.headline());
    assert!(!rep.headline().contains('\n'));
    assert!(summary.contains("1 cabinets") && summary.contains("1 roof planes"));
    assert!(summary.contains("1 electrical devices") && summary.contains("2 stairs"));
    // The project, with its opaque slots, survives JSON.
    let back = Project::from_json(&p.to_json().unwrap()).unwrap();
    assert_eq!(back.floors[0].cabinets, f.cabinets);
    assert_eq!(back.floors[0].electrical, f.electrical);
    assert_eq!(back.floors[1].roofs, p.floors[1].roofs);
    assert_eq!(back.floors[0].stairs, f.stairs);
    // Ids are unique across every object kind.
    let mut ids: Vec<u64> = f
        .walls
        .iter()
        .map(|w| w.id)
        .chain(f.symbols.iter().map(|s| s.id))
        .chain(f.cabinets.iter().map(|c| c["id"].as_u64().unwrap()))
        .chain(f.stairs.iter().map(|c| c["id"].as_u64().unwrap()))
        .chain(devices.iter().map(|c| c["id"].as_u64().unwrap()))
        .chain(roofs.iter().map(|c| c["id"].as_u64().unwrap()))
        .collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total);
}

#[test]
fn symbol_resolver_and_stage_switches() {
    let bytes = build_template(&objects_house(), &[]);
    let opts = ImportOptions {
        symbol_resolver: Some(|q: &SymbolQuery| {
            (q.name == "Elongated Toilet" && q.tags.iter().any(|t| t == "ADA"))
                .then(|| "chief.abc.7".to_string())
        }),
        ..ImportOptions::default()
    };
    let r = import_bytes(&bytes, "Objects.plan", &opts).unwrap();
    assert_eq!(r.project.floors[0].symbols[0].catalog_id, "chief.abc.7");
    assert_eq!(r.report.counts["symbols_linked"], 1);
    assert!(r
        .report
        .warnings
        .iter()
        .any(|w| w.contains("1 linked to a catalog item and 0 left")));

    let off = ImportOptions {
        cabinets: false,
        symbols: false,
        electrical: false,
        stairs: false,
        roofs: false,
        room_labels: false,
        ..ImportOptions::default()
    };
    let r = import_bytes(&bytes, "Objects.plan", &off).unwrap();
    let f = &r.project.floors[0];
    assert!(f.cabinets.is_empty() && f.symbols.is_empty() && f.stairs.is_empty());
    assert!(f.electrical.is_none());
    assert!(r.project.floors.iter().all(|f| f.roofs.is_empty()));
    assert!(f.room_names.iter().all(|n| n.name != "kitchen"));
    assert_eq!(r.report.counts["cabinets"], 0);
    // Left unread, the classes show up as skipped.
    assert!(r.report.skipped_classes.iter().any(|s| s.class == 15));
    assert!(r.report.skipped_classes.iter().any(|s| s.class == 21));
}

/// The stage-3 objects: a library item with a catalog GUID, a U stair that
/// stacks on a landing, a switch and its light joined by an arc (and the
/// reverse arc of a second pair), a gang box, and a roof plane with joined
/// edges and an overhang.
fn stage3_house() -> Vec<u8> {
    use super::electrical::tests::{connection_obj, device_obj, gang_obj};
    use super::lines::tests::put_rect;
    use super::roofs::tests::plane_obj_flags;
    use super::stairs::tests::flight_obj_stacked;
    use super::symbols::tests::symbol_obj_guid;
    let mut body = types_body();
    let ext = |x: f64, y: f64, dx: f64, dy: f64, len: f64| {
        wall_obj(line_obj(x, y, dx, dy, len), Some(581), Some(121.125), &[])
    };
    let partition = wall_obj(
        line_obj(200.0, 0.0, 0.0, 1.0, 300.0),
        Some(351),
        Some(121.125),
        &[],
    );
    let switch =
        |y: f64, name: &str| device_obj(751, 203.0, y, &[name, "Switches", "Wall Mounted"]);
    let light = |x: f64, y: f64| {
        device_obj(
            755,
            x,
            y,
            &["Recessed Down Light 6", "Ceiling Mounted", "Lighting"],
        )
    };
    let duplex = |y: f64| {
        device_obj(
            751,
            203.0,
            y,
            &["Duplex", "110V", "Outlets", "Wall Mounted"],
        )
    };
    let toilet_guid = [
        0x4c, 0x52, 0x82, 0x23, 0xd7, 0x12, 0x45, 0xe5, 0xb7, 0xa3, 0x27, 0xf7, 0x5f, 0x3e, 0x88,
        0xc1,
    ];
    let kids = vec![
        ext(0.0, 0.0, 1.0, 0.0, 400.0),
        ext(400.0, 0.0, 0.0, 1.0, 300.0),
        ext(400.0, 300.0, -1.0, 0.0, 400.0),
        ext(0.0, 300.0, 0.0, -1.0, 300.0),
        partition,
        symbol_obj_guid(
            [150.0, 100.0, 0.0, 1.0, 30.0, 30.0, 30.0, 30.0],
            "Elongated Toilet",
            &["ADA"],
            Some(toilet_guid),
        ),
        switch(100.0, "Single Pole"),
        light(300.0, 150.0),
        switch(250.0, "Three Way"),
        light(350.0, 250.0),
        gang_obj(&[duplex(60.0), duplex(65.25)]),
        connection_obj((213.0, 100.0), (250.0, 140.0), (300.0, 150.0)),
        connection_obj((350.0, 250.0), (280.0, 270.0), (213.0, 250.0)),
        flight_obj_stacked(
            (300.0, 100.0, 0.0, 1.0, 94.5),
            42.0,
            10.5,
            133.875,
            7.875,
            0.875,
        ),
        sized(47, 0, 2000, |b| put_rect(b, 671, 276.0, 194.5, 48.0, 46.0)),
        flight_obj_stacked(
            (300.0, 240.5, 0.0, 1.0, 63.0),
            42.0,
            10.5,
            133.875,
            7.875,
            79.625,
        ),
    ];
    body.extend(floor_obj(0.0, 121.125, &kids));
    // South eave and east gable overhang; north (ridge) and west (hip) joined.
    let roof = plane_obj_flags(
        &[
            (412.0, -12.0), // starts at the east side so the eave is not first
            (412.0, 312.0),
            (-12.0, 312.0),
            (-12.0, -12.0),
        ],
        (0.0, 0.0, 1.0, 0.0, 400.0),
        129.84,
        (8.0f64 / 12.0).atan(),
        &[
            (None, true),
            (Some(1), false),
            (Some(0), false),
            (None, true),
        ],
    );
    body.extend(floor_obj(137.875, 109.125, &[roof]));
    body
}

#[test]
fn stage3_catalog_guid_reaches_the_resolver() {
    let bytes = build_template(&stage3_house(), &[]);
    let opts = ImportOptions {
        symbol_resolver: Some(|q: &SymbolQuery| {
            (q.unique_id.as_deref() == Some("4c528223-d712-45e5-b7a3-27f75f3e88c1"))
                .then(|| "chief.core-arch.77".to_string())
        }),
        ..ImportOptions::default()
    };
    let r = import_bytes(&bytes, "Stage3.plan", &opts).unwrap();
    let f = &r.project.floors[0];
    assert_eq!(f.symbols.len(), 1);
    assert_eq!(f.symbols[0].catalog_id, "chief.core-arch.77");
    assert_eq!(r.report.counts["symbols_with_guid"], 1);
    assert_eq!(r.report.counts["symbols_linked"], 1);
    // Without a resolver it stays a stand-in and the GUID is still counted.
    let r = import_bytes(&bytes, "Stage3.plan", &ImportOptions::default()).unwrap();
    assert_eq!(
        r.project.floors[0].symbols[0].catalog_id,
        "chief-plan.elongated-toilet"
    );
    assert_eq!(r.report.counts["symbols_with_guid"], 1);
    assert_eq!(r.report.counts["symbols_linked"], 0);
    assert!(r
        .report
        .warnings
        .iter()
        .any(|w| w.contains("no catalog resolver")));
}

#[test]
fn stage3_electrical_connections_and_gang_boxes() {
    let bytes = build_template(&stage3_house(), &[]);
    let r = import_bytes(&bytes, "Stage3.plan", &ImportOptions::default()).unwrap();
    let (f, rep) = (&r.project.floors[0], &r.report);
    let layer = f.electrical.as_ref().unwrap();
    let devices = layer["devices"].as_array().unwrap();
    // Two switches, two lights and the two outlets of the gang box.
    assert_eq!(devices.len(), 6, "{devices:?}");
    assert_eq!(rep.counts["electrical_devices"], 6);
    assert_eq!(rep.counts["electrical_in_groups"], 2);
    assert_eq!(rep.counts["electrical_connections"], 2);
    let by = |kind: &str, nth: usize| {
        devices
            .iter()
            .filter(|d| d["kind"] == kind)
            .nth(nth)
            .unwrap_or_else(|| panic!("no {kind} #{nth}"))
    };
    let (sw, sw3) = (by("Switch", 0), by("Switch3Way", 0));
    let (l1, l2) = (by("RecessedCan", 0), by("RecessedCan", 1));
    let conns = layer["connections"].as_array().unwrap();
    assert_eq!(conns.len(), 2);
    // Forward arc: switch (the start) to the light, bending left.
    assert_eq!(conns[0]["from"], sw["id"]);
    assert_eq!(conns[0]["to"], l1["id"]);
    let b0 = conns[0]["arc_bulge"].as_f64().unwrap();
    assert!(b0 > 15.0 && b0 < 17.0, "{b0}");
    // Reverse arc (drawn from the light): the switch is still `from`, and the
    // bulge flips sign with the direction.
    assert_eq!(conns[1]["from"], sw3["id"]);
    assert_eq!(conns[1]["to"], l2["id"]);
    assert!((conns[1]["arc_bulge"].as_f64().unwrap() - 20.0).abs() < 1e-6);
    // The lights know their switches.
    assert_eq!(l1["switched_by"], json!([sw["id"]]));
    assert_eq!(l2["switched_by"], json!([sw3["id"]]));
    assert_eq!(sw["switched_by"], json!([]));
    // The gang box's outlets are on the partition.
    let outlets: Vec<_> = devices
        .iter()
        .filter(|d| d["kind"] == "Outlet110")
        .collect();
    assert_eq!(outlets.len(), 2);
    assert!(outlets.iter().all(|d| !d["wall_id"].is_null()));
    // Nothing of it is reported as skipped.
    for class in [21u8, 34, 150] {
        assert!(
            !rep.skipped_classes.iter().any(|s| s.class == class),
            "class {class} listed as skipped"
        );
    }
    // The layer survives JSON as the real types would read it.
    let back = Project::from_json(&r.project.to_json().unwrap()).unwrap();
    assert_eq!(back.floors[0].electrical, f.electrical);
}

#[test]
fn stage3_stairs_stack_on_the_landing() {
    let bytes = build_template(&stage3_house(), &[]);
    let r = import_bytes(&bytes, "Stage3.plan", &ImportOptions::default()).unwrap();
    let f = &r.project.floors[0];
    assert_eq!(f.stairs.len(), 3);
    // Flights come first (file order), then the landing.
    let (a, b, landing) = (&f.stairs[0], &f.stairs[1], &f.stairs[2]);
    assert_eq!(a["base"], 0.0);
    // The second flight leaves the landing at 79.625 - 0.875 = 78.75 = 10 risers.
    assert!((b["base"].as_f64().unwrap() - 78.75).abs() < 1e-9);
    // The landing is at that height too (from the flight that arrives).
    assert!((landing["params"]["total_rise"].as_f64().unwrap() - 78.75).abs() < 1e-9);
    assert_eq!(r.report.counts["stairs_stacked"], 1);
    assert!(r
        .report
        .warnings
        .iter()
        .any(|w| w.contains("1 stand on a landing") && w.contains("1 of the landings")));
}

#[test]
fn stage3_roof_plane_has_its_eave_first_and_edge_flags() {
    let bytes = build_template(&stage3_house(), &[]);
    let r = import_bytes(&bytes, "Stage3.plan", &ImportOptions::default()).unwrap();
    let roofs = &r.project.floors.last().unwrap().roofs;
    assert_eq!(roofs.len(), 1);
    let plane = &roofs[0];
    // The eave (south) edge is first: the first two vertices share y = -12 and
    // sit at the baseline height less the 12" of overhang rise.
    let poly = plane["polygon3d"].as_array().unwrap();
    assert!((poly[0][2].as_f64().unwrap() - 12.0).abs() < 1e-9);
    assert!((poly[1][2].as_f64().unwrap() - 12.0).abs() < 1e-9);
    assert!(poly[0][1].as_f64().unwrap() < 129.84);
    assert_eq!(plane["overhang"], 12.0);
    let edges = plane["chief_edges"].as_array().unwrap();
    let roles: Vec<&str> = edges.iter().map(|e| e["role"].as_str().unwrap()).collect();
    assert_eq!(roles, vec!["eave", "rake", "ridge", "hip_or_valley"]);
    assert_eq!(edges[1]["overhangs"], true);
    assert_eq!(edges[2]["joined"], true);
    let c = &r.report.counts;
    assert_eq!(
        (
            c["roof_edges_joined"],
            c["roof_gable_edges"],
            c["roof_overhangs"]
        ),
        (2, 1, 1)
    );
}
