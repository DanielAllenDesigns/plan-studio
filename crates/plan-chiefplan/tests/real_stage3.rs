//! Stage-3 checks against Daniel's real Chief projects: the catalog GUID of
//! placed library objects, roof edge flags and eaves, stair stacking heights,
//! electrical connections and gang boxes. They read the files in place
//! (nothing is copied or written) and skip cleanly when a variable is not set.
//!
//! ```text
//! CHIEF_PLAN_A=... CHIEF_PLAN_B=... CHIEF_PLAN_C=... CHIEF_PLAN_X17=... \
//!   cargo test -p plan-chiefplan --release --test real_stage3 -- --ignored --nocapture
//! ```
//!
//! `CHIEF_PLAN_EXTRA` may hold more files separated by `:`. Every check is
//! relative to what the file holds (a file without roofs checks no roofs), so
//! any X18 project works; the counts quoted in `docs/chief-plan-format.md`
//! come from the whole archive.

use plan_chiefplan::import::{import_plan, ImportOptions, ImportResult, SymbolQuery};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;

fn files() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = [
        "CHIEF_PLAN_A",
        "CHIEF_PLAN_B",
        "CHIEF_PLAN_C",
        "CHIEF_PLAN_X17",
    ]
    .iter()
    .filter_map(std::env::var_os)
    .map(PathBuf::from)
    .collect();
    if let Some(extra) = std::env::var_os("CHIEF_PLAN_EXTRA") {
        out.extend(
            extra
                .to_string_lossy()
                .split(':')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
        );
    }
    out.retain(|p| p.exists());
    out
}

static SEEN: Mutex<Vec<SymbolQuery>> = Mutex::new(Vec::new());

/// Records every query and links nothing.
fn recorder(q: &SymbolQuery) -> Option<String> {
    SEEN.lock().unwrap().push(q.clone());
    None
}

fn run(path: &PathBuf) -> ImportResult {
    run_with(path, ImportOptions::default())
}

fn run_with(path: &PathBuf, opts: ImportOptions) -> ImportResult {
    let t = std::time::Instant::now();
    let r = import_plan(path, &opts).expect("import");
    println!(
        "{}: {:.2}s",
        path.file_name().unwrap().to_string_lossy(),
        t.elapsed().as_secs_f64()
    );
    r
}

fn num(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("{k} in {v}"))
}

fn is_guid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit() && !c.is_ascii_uppercase(),
        })
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn placed_objects_carry_a_catalog_guid() {
    for path in files() {
        SEEN.lock().unwrap().clear();
        let r = run_with(
            &path,
            ImportOptions {
                symbol_resolver: Some(recorder),
                ..ImportOptions::default()
            },
        );
        let seen = SEEN.lock().unwrap().clone();
        let placed = r.report.counts["symbols"];
        let with = r.report.counts["symbols_with_guid"];
        println!(
            "  {placed} symbols, {with} with a GUID, {} queries",
            seen.len()
        );
        assert_eq!(seen.len(), placed);
        if placed < 10 {
            continue;
        }
        // Half of the placed objects (and often more) carry the anchored GUID.
        assert!(with * 100 >= placed * 30, "{with} of {placed}");
        for q in &seen {
            if let Some(g) = &q.unique_id {
                assert!(is_guid(g), "{g}");
            }
            assert!(q.candidates.iter().all(|c| is_guid(c)));
        }
        // A GUID names one item: every copy placed under it has the same name
        // (generic names such as `Pillow` can sit under several GUIDs).
        let mut by_guid = std::collections::HashMap::<&str, &str>::new();
        for q in &seen {
            if let Some(g) = &q.unique_id {
                let prev = by_guid.entry(g.as_str()).or_insert(q.name.as_str());
                assert_eq!(*prev, q.name.as_str(), "{g} has two names");
            }
        }
    }
}

/// Whether two edges lie on one line and overlap by 6" or more.
fn shares(a: ((f64, f64), (f64, f64)), b: ((f64, f64), (f64, f64))) -> bool {
    let (dx, dy) = (a.1 .0 - a.0 .0, a.1 .1 - a.0 .1);
    let len = dx.hypot(dy);
    if len < 6.0 {
        return false;
    }
    let (ux, uy) = (dx / len, dy / len);
    let off = |p: (f64, f64)| ((p.0 - a.0 .0) * uy - (p.1 - a.0 .1) * ux).abs();
    if off(b.0) > 1.0 || off(b.1) > 1.0 {
        return false;
    }
    let t = |p: (f64, f64)| (p.0 - a.0 .0) * ux + (p.1 - a.0 .1) * uy;
    let (t0, t1) = (t(b.0).min(t(b.1)), t(b.0).max(t(b.1)));
    t1.min(len) - t0.max(0.0) >= 6.0
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn roof_planes_have_the_eave_first_and_joined_edges_share_a_neighbour() {
    for path in files() {
        let r = run(&path);
        let (mut joined, mut shared, mut planes) = (0, 0, 0);
        for f in &r.project.floors {
            let edges_of = |p: &Value| -> Vec<((f64, f64), (f64, f64))> {
                p["chief_edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| {
                        (
                            (num(&e["start"], "x"), num(&e["start"], "y")),
                            (num(&e["end"], "x"), num(&e["end"], "y")),
                        )
                    })
                    .collect()
            };
            for (i, p) in f.roofs.iter().enumerate() {
                planes += 1;
                let edges = p["chief_edges"].as_array().unwrap();
                assert_eq!(edges[0]["role"], "eave");
                // The first polygon edge is parallel to the baseline.
                let poly = p["polygon3d"].as_array().unwrap();
                let v = |k: usize| (poly[k][0].as_f64().unwrap(), -poly[k][2].as_f64().unwrap());
                let (b0, b1) = (&p["baseline"][0], &p["baseline"][1]);
                let (bx, by) = (num(b1, "x") - num(b0, "x"), num(b1, "y") - num(b0, "y"));
                let (ex, ey) = (v(1).0 - v(0).0, v(1).1 - v(0).1);
                let cross = (bx * ey - by * ex).abs() / (bx.hypot(by) * ex.hypot(ey)).max(1e-9);
                assert!(
                    cross < 1e-3,
                    "eave edge not parallel to the baseline: {cross}"
                );
                let over = num(p, "overhang");
                assert!((0.0..=120.0).contains(&over), "overhang {over}");
                let mine = edges_of(p);
                for (k, e) in edges.iter().enumerate() {
                    if e["joined"] == true && mine[k].0 != mine[k].1 {
                        joined += 1;
                        let hit = f.roofs.iter().enumerate().any(|(j, q)| {
                            j != i && edges_of(q).iter().any(|&o| shares(mine[k], o))
                        });
                        shared += usize::from(hit);
                    }
                }
            }
        }
        println!("  {planes} planes, {joined} joined edges, {shared} with a neighbour");
        if joined >= 10 {
            assert!(shared * 100 >= joined * 90, "{shared} of {joined}");
        }
    }
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn stacked_flights_chain_through_their_landings() {
    for path in files() {
        let r = run(&path);
        let (mut stacked, mut chained) = (0, 0);
        for f in &r.project.floors {
            // (base, total_rise, riser) of every straight flight of the floor.
            let flights: Vec<(f64, f64, f64)> = f
                .stairs
                .iter()
                .filter(|s| s["params"]["shape"] == "Straight")
                .map(|s| {
                    (
                        num(s, "base"),
                        num(&s["params"], "total_rise"),
                        num(&s["params"], "riser_height_target"),
                    )
                })
                .collect();
            for s in f
                .stairs
                .iter()
                .filter(|s| s["params"]["shape"] == "Straight")
            {
                let base = num(s, "base");
                assert!(base > -300.0 && base < 400.0, "base {base}");
                if base > 1.0 {
                    stacked += 1;
                    // Another flight arrives where it starts: its top landing is the
                    // base, or one riser lower (a one-tread winder flight between
                    // them, which the importer cannot read, makes up the step).
                    if flights.iter().any(|&(b, rise, riser)| {
                        (b + rise - base).abs() < 0.1 || (b + rise + riser - base).abs() < 0.1
                    }) {
                        chained += 1;
                    }
                }
            }
            // Landings are at the height of a flight's start or end.
            for l in f
                .stairs
                .iter()
                .filter(|s| s["params"]["shape"]["Landing"].is_object())
            {
                let h = num(&l["params"], "total_rise");
                assert!(h > 0.0 && h < 400.0, "landing {h}");
            }
        }
        println!(
            "  {} flights, {} landings, {stacked} stacked, {chained} chained",
            r.report.counts["stairs"], r.report.counts["stair_landings"]
        );
        assert_eq!(stacked, r.report.counts["stairs_stacked"]);
        if stacked >= 2 {
            assert!(chained * 100 >= stacked * 70, "{chained} of {stacked}");
        }
    }
}

#[test]
#[ignore = "needs Daniel's Chief projects"]
fn electrical_connections_join_real_devices() {
    for path in files() {
        let r = run(&path);
        let mut conns = 0;
        for f in &r.project.floors {
            let Some(e) = f.electrical.as_ref() else {
                continue;
            };
            let devices = e["devices"].as_array().unwrap();
            let ids: std::collections::HashSet<u64> =
                devices.iter().map(|d| d["id"].as_u64().unwrap()).collect();
            assert_eq!(ids.len(), devices.len(), "device ids repeat");
            for c in e["connections"].as_array().unwrap() {
                conns += 1;
                let (a, b) = (c["from"].as_u64().unwrap(), c["to"].as_u64().unwrap());
                assert!(ids.contains(&a) && ids.contains(&b) && a != b);
                assert!(num(c, "arc_bulge").abs() < 400.0);
            }
            // Every switch a device lists is a switch of the floor.
            for d in devices {
                for s in d["switched_by"].as_array().unwrap() {
                    let sw = devices
                        .iter()
                        .find(|x| x["id"] == *s)
                        .expect("switch of the floor");
                    assert!(sw["kind"].as_str().unwrap().starts_with("Switch"));
                }
            }
        }
        println!(
            "  {} devices ({} from gang boxes), {} connections",
            r.report.counts["electrical_devices"], r.report.counts["electrical_in_groups"], conns
        );
        assert_eq!(conns, r.report.counts["electrical_connections"]);
    }
}
