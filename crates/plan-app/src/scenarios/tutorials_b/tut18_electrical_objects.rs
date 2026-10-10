//! Lesson 18, Electrical Objects (pp. 317-331). Real: the outlet family and
//! a run of three switches are placed one undo step each. Open: recessed
//! outlets and the ganged block.
use crate::editor::site_view::load_electrical;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::Sim;
use crate::tools::electrical::ElecVariant;
use crate::tools::ToolId;
use plan_electrical::{Device, DeviceKind};

fn wired() -> Sim {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    chic_cottage(&mut sim);
    sim
}

fn devices(sim: &Sim) -> Vec<Device> {
    load_electrical(sim.app.cx.floor()).devices
}

fn put(sim: &mut Sim, v: ElecVariant, x: f64, y: f64) {
    sim.tool(ToolId::ElectricalVariant(v));
    sim.click(x, y);
}

#[test]
fn the_outlet_family_places_distinct_devices() {
    let mut sim = wired();
    for (v, x, y) in [
        (ElecVariant::Outlet110, 100.0, 6.0),
        (ElecVariant::Gfci, 160.0, 6.0),
        (ElecVariant::Outlet220, 220.0, 6.0),
        (ElecVariant::OutletFloor, 240.0, 240.0),
    ] {
        assert_one_undo_step(&mut sim, "outlet", |s| put(s, v, x, y));
    }
    let d = devices(&sim);
    assert_eq!(d.len(), 4);
    assert!(d.iter().any(|d| d.kind == DeviceKind::Gfci));
    assert!(d.iter().any(|d| d.kind == DeviceKind::OutletFloor));
    let mut kinds: Vec<String> = d.iter().map(|d| format!("{:?}", d.kind)).collect();
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds.len(), 4, "four tools, four kinds");
}

#[test]
fn three_switches_side_by_side_stay_three_devices() {
    let mut sim = wired();
    for x in [60.0, 75.0, 90.0] {
        assert_one_undo_step(&mut sim, "switch", |s| put(s, ElecVariant::Switch, x, 6.0));
    }
    let sw = devices(&sim)
        .into_iter()
        .filter(|d| d.kind == DeviceKind::Switch)
        .count();
    assert_eq!(sw, 3);
}

#[test]
#[ignore = "T7-18: E-32 (Options panel: Distance from Wall -4 in, Cuts Wall: recessed weatherproof outlet)"]
fn recessed_outlet() {
    assert_ignored_break("E-32");
}

#[test]
#[ignore = "T7-18: E-25 (group-select three switches, Make Ganged Electrical Block)"]
fn ganged_switch_block() {
    assert_ignored_break("E-25");
}

#[test]
#[ignore = "T7-18: CB-428 (a ganged block moves and copies as one object)"]
fn ganged_block_behaves_as_one() {
    assert_ignored_break("CB-428");
}
