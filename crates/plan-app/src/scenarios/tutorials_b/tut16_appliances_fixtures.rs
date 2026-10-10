//! Lesson 16, Appliances and Fixtures (pp. 277-298). Real: partition and
//! stacked wall cabinets are single undo steps. Open: the appliance snap,
//! explode, fixture options and the fixture schedules.
use crate::editor::placed::load_cabinets;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::Sim;
use crate::tools::ToolId;
use plan_cabinets::CabinetKind;

fn place(sim: &mut Sim, kind: CabinetKind, x: f64, y: f64) {
    sim.tool(ToolId::CabinetVariant(kind));
    sim.app.cx.selection.clear();
    sim.click(x, y);
}

#[test]
fn partition_and_wall_cabinet_are_one_undo_step_each() {
    let mut sim = cottage();
    assert_one_undo_step(&mut sim, "partition", |s| {
        place(s, CabinetKind::Partition, 200.0, 20.0)
    });
    assert_one_undo_step(&mut sim, "wall cabinet", |s| {
        place(s, CabinetKind::Wall, 100.0, 20.0)
    });
    let cabs = load_cabinets(sim.app.cx.floor());
    assert_eq!(cabs.len(), 2);
    let wall = cabs.iter().find(|c| c.kind == CabinetKind::Wall).unwrap();
    assert!(wall.elevation > 0.0, "a wall cabinet hangs above the floor");
}

#[test]
#[ignore = "T7-16: CB-635 (wall cabinet over the refrigerator: bottom meets the appliance top)"]
fn wall_cabinet_meets_refrigerator_top() {
    assert_ignored_break("CB-635");
}

#[test]
#[ignore = "T7-16: CB-434 (Explode Architectural Block)"]
fn explode_block() {
    assert_ignored_break("CB-434");
}

#[test]
#[ignore = "T7-16: CB-636 (dishwasher between cabinets: countertop extension continuous)"]
fn dishwasher_countertop() {
    assert_ignored_break("CB-636");
}

#[test]
#[ignore = "T7-16: L-233 (fixture schedules by category and room: Kitchen Appliance Schedule)"]
fn fixture_schedules_by_room() {
    assert_ignored_break("L-233");
}

#[test]
#[ignore = "T7-16: S-118 (Find Object in Plan from the schedule row)"]
fn find_in_plan() {
    assert_ignored_break("S-118");
}

#[test]
#[ignore = "T7-16: DIM-63 (auto elevation dimensions with Move Extension Line handles)"]
fn elevation_dimension_extensions() {
    assert_ignored_break("DIM-63");
}
