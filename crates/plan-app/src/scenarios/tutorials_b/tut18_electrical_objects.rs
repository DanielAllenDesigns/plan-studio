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

/// Selects the devices and runs one of the electrical Edit commands.
fn command_on(sim: &mut Sim, ids: &[plan_core::Id], id: &str) {
    sim.app.cx.selection.items = ids
        .iter()
        .map(|d| crate::editor::ObjectRef::Device(*d))
        .collect();
    assert!(crate::tools::electrical::run_command(&mut sim.app.cx, id));
}

fn three_switches(sim: &mut Sim) -> Vec<plan_core::Id> {
    for x in [60.0, 75.0, 90.0] {
        put(sim, ElecVariant::Switch, x, 6.0);
    }
    let mut ids: Vec<_> = devices(sim).iter().map(|d| d.id).collect();
    ids.sort_unstable();
    ids
}

#[test]
fn recessed_outlet() {
    let mut sim = wired();
    // The outside face of the front wall: a weatherproof outlet.
    put(&mut sim, ElecVariant::Outlet110, 100.0, -6.0);
    let d = devices(&sim).pop().expect("an outlet");
    assert_eq!(d.kind, DeviceKind::OutletWp);
    // Options panel: Distance from Wall -4 in, Cuts Wall.
    let mut layer = load_electrical(sim.app.cx.floor());
    let mut o = layer.options_of(d.id);
    o.recess.distance_from_wall = -4.0;
    o.recess.cuts_wall = true;
    layer.set_options(d.id, o);
    crate::editor::site_view::save_electrical(&mut sim.app.cx.project, 0, &layer);
    let back = load_electrical(sim.app.cx.floor()).options_of(d.id);
    assert!(back.recess.is_recessed() && back.recess.cuts_wall);
}

#[test]
fn ganged_switch_block() {
    let mut sim = wired();
    let ids = three_switches(&mut sim);
    assert_eq!(ids.len(), 3);
    let before = sim.app.cx.undo_depth();
    command_on(&mut sim, &ids, crate::tools::electrical::cmd::MAKE_GANG);
    assert_eq!(sim.app.cx.undo_depth(), before + 1, "one undo step");
    let layer = load_electrical(sim.app.cx.floor());
    assert_eq!(layer.gang_members(ids[1]), ids);
    // Explode frees them again, as one undo step.
    command_on(
        &mut sim,
        &ids[1..2],
        crate::tools::electrical::cmd::EXPLODE_GANG,
    );
    let layer = load_electrical(sim.app.cx.floor());
    assert_eq!(layer.gang_members(ids[1]), vec![ids[1]]);
    sim.undo();
    let layer = load_electrical(sim.app.cx.floor());
    assert_eq!(
        layer.gang_members(ids[0]).len(),
        3,
        "undo restores the block"
    );
    // A second block over the same switches is refused.
    command_on(&mut sim, &ids, crate::tools::electrical::cmd::MAKE_GANG);
    assert!(
        sim.app.cx.status.contains("Explode"),
        "{}",
        sim.app.cx.status
    );
}

#[test]
#[ignore = "T7-18: CB-428 (a ganged block copies as one object; moving as one is covered by ganged_block_moves_as_one)"]
fn ganged_block_behaves_as_one() {
    assert_ignored_break("CB-428");
}

#[test]
fn ganged_block_moves_as_one() {
    let mut sim = wired();
    let ids = three_switches(&mut sim);
    command_on(&mut sim, &ids, crate::tools::electrical::cmd::MAKE_GANG);
    let x0: Vec<f64> = devices(&sim).iter().map(|d| d.position.x).collect();
    sim.tool(ToolId::Select);
    sim.click(75.0, 6.0);
    sim.drag((75.0, 6.0), (175.0, 6.0));
    let x1: Vec<f64> = devices(&sim).iter().map(|d| d.position.x).collect();
    for (a, b) in x0.iter().zip(&x1) {
        assert!((b - a - 100.0).abs() < 1.0, "{a} -> {b}");
    }
}
