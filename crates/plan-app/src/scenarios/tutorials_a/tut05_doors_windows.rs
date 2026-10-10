//! Lesson 5, Doors and Windows (pp. 82-105).
use crate::scenarios::tutorials_support::*;
use crate::tools::ToolId;

#[test]
fn door_and_window_clicks_are_one_undo_step_each() {
    let mut sim = cottage();
    sim.tool(ToolId::Door);
    assert_one_undo_step(&mut sim, "door", |s| s.click(120.0, 0.0));
    sim.tool(ToolId::Window);
    assert_one_undo_step(&mut sim, "window", |s| s.click(360.0, 0.0));
    assert_eq!(sim.app.cx.floor().openings.len(), 2);
    // Both sit on the south exterior wall.
    let wall = sim.app.cx.floor().walls[0].id;
    assert!(sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .all(|o| o.wall_id == wall));
    // The shell is still one room.
    assert_eq!(sim.app.cx.rooms.len(), 1);
}

#[test]
fn interior_door_goes_into_a_partition() {
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    sim.tool(ToolId::Door);
    assert_one_undo_step(&mut sim, "door", |s| s.click(200.0, 120.0));
    assert_eq!(sim.app.cx.floor().openings.len(), 1);
    let w = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .find(|w| w.id == sim.app.cx.floor().openings[0].wall_id)
        .unwrap();
    assert_eq!(w.kind, plan_core::WallKind::Interior);
}

#[test]
#[ignore = "T7-05: CB-395 (hardware picked from the library)"]
fn lock_hardware_from_the_library() {
    assert_ignored_break("CB-395");
}

#[test]
#[ignore = "T7-05: DW-168 (Make Mulled Unit as one object)"]
fn mulled_unit_is_one_object() {
    assert_ignored_break("DW-168");
}

#[test]
#[ignore = "T7-05: TXT-65 (leader macro %comment% equals the door comment)"]
fn comment_macro_in_a_leader() {
    assert_ignored_break("TXT-65");
}
