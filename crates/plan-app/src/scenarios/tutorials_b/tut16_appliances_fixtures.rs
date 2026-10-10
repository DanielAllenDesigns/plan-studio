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
#[ignore = "T7-16: CB-636 (dishwasher between cabinets: countertop extension continuous)"]
fn dishwasher_countertop() {
    assert_ignored_break("CB-636");
}

#[test]
#[ignore = "T7-16: DIM-63 (auto elevation dimensions with Move Extension Line handles)"]
fn elevation_dimension_extensions() {
    assert_ignored_break("DIM-63");
}

fn fridge(sim: &mut Sim, x: f64, y: f64) {
    let mut f = plan_core::PlacedSymbol::new(
        "appliances/refrigerator-side-by-side",
        plan_core::geometry::Point::new(x, y),
        36.0,
        30.0,
        70.0,
    );
    f.layer = "Fixtures, Interior".into();
    sim.app.cx.project.add_symbol(0, f);
    sim.app.cx.refresh();
}

#[test]
fn wall_cabinet_meets_refrigerator_top() {
    let mut sim = cottage();
    fridge(&mut sim, 60.0, 20.0);
    place(&mut sim, CabinetKind::Wall, 60.0, 20.0);
    let over = load_cabinets(sim.app.cx.floor())
        .into_iter()
        .find(|c| c.kind == CabinetKind::Wall)
        .unwrap();
    assert_eq!(over.elevation, 70.0, "the bottom meets the appliance top");
    place(&mut sim, CabinetKind::Wall, 300.0, 20.0);
    let far = load_cabinets(sim.app.cx.floor())
        .into_iter()
        .filter(|c| c.kind == CabinetKind::Wall)
        .max_by(|a, b| a.position.x.total_cmp(&b.position.x))
        .unwrap();
    assert_ne!(
        far.elevation, 70.0,
        "away from the appliance the usual bottom"
    );
}

#[test]
fn explode_releases_a_cabinet_block_in_one_step() {
    let mut sim = cottage();
    for i in 0..2 {
        place(&mut sim, CabinetKind::Base, 60.0 + 24.0 * i as f64, 20.0);
    }
    sim.app.cx.selection.items = load_cabinets(sim.app.cx.floor())
        .iter()
        .map(|c| crate::editor::ObjectRef::Cabinet(c.id))
        .collect();
    sim.action(crate::toolbar::Action::Custom(
        crate::tools::arch_block::MAKE_BLOCK,
    ));
    assert_eq!(sim.app.cx.floor().blocks.len(), 1);
    assert_one_undo_step(&mut sim, "explode", |s| {
        s.action(crate::toolbar::Action::Custom(
            crate::tools::arch_block::EXPLODE_BLOCK,
        ))
    });
    assert!(sim.app.cx.floor().blocks.is_empty());
    assert_eq!(load_cabinets(sim.app.cx.floor()).len(), 2);
}

#[test]
fn a_fixture_schedule_is_made_from_one_room() {
    use crate::editor::schedule_view as sv;
    let mut sim = cottage();
    assert!(!sim.app.cx.rooms.is_empty(), "the cottage encloses a room");
    let id = assert_one_undo_step(&mut sim, "schedule", |s| {
        sv::create_from_room(
            &mut s.app.cx,
            plan_core::schedules::ScheduleKind::Fixture,
            0,
            plan_core::geometry::Point::new(0.0, 700.0),
        )
    })
    .expect("created");
    let d = sv::find(&sim.app.cx, id).unwrap();
    assert_eq!(d.rooms.len(), 1, "limited to the one room");
}

#[test]
#[ignore = "T7-16: S-118 (a placed library refrigerator yields no Fixture schedule row in the app, so there is nothing to Find in Plan; investigate the schedule filter)"]
fn find_in_plan() {
    assert_ignored_break("S-118");
}
