//! Lesson 3, Multiple Floors (pp. 45-59).
use crate::scenarios::tutorials_support::*;
use crate::toolbar::Action;

#[test]
fn build_new_floor_derives_the_second_floor_in_one_undo_step() {
    let mut sim = cottage();
    let first: Vec<_> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| (w.start, w.end, w.kind))
        .collect();
    assert_one_undo_step(&mut sim, "Build New Floor", |s| {
        s.action(Action::BuildNewFloor);
        s.ok();
    });
    assert_eq!(sim.app.cx.project.floors.len(), 2);
    assert_eq!(sim.app.cx.floor, 1);
    let second: Vec<_> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| (w.start, w.end, w.kind))
        .collect();
    assert_eq!(first.len(), second.len(), "same exterior shell");
    assert_eq!(sim.app.cx.rooms.len(), 1);
    sim.undo();
    assert_eq!(sim.app.cx.project.floors.len(), 1);
}

#[test]
fn second_floor_sits_above_the_first() {
    let mut sim = cottage();
    sim.action(Action::BuildNewFloor);
    sim.ok();
    let f = &sim.app.cx.project.floors;
    assert!(f[1].elevation >= f[0].elevation + f[0].ceiling_height);
}

#[test]
#[ignore = "T7-03: R-133 (garage curb and stems 24 in below the slab)"]
fn garage_foundation_options() {
    assert_ignored_break("R-133");
}

#[test]
#[ignore = "T7-03: W-138 (Align With Wall Above)"]
fn align_with_wall_above() {
    assert_ignored_break("W-138");
}

#[test]
#[ignore = "T7-03: W-145 (furred 8 in stem wall with Air Gap and Framing layers)"]
fn furred_basement_wall_type() {
    assert_ignored_break("W-145");
}

#[test]
#[ignore = "T7-03: L-235 (Wall Schedule with Total Width / Upper and Lower columns)"]
fn wall_schedule_lists_exactly_the_used_wall_types() {
    assert_ignored_break("L-235");
}
