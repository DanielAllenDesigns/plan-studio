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
fn door_hinge_and_swing_flip_one_undo_step_each() {
    // Lesson 5 (guide p. 85): the hinge side and the swing side of a placed
    // door are changed after the fact; each is one undo step and undoes.
    use crate::editor::ObjectRef;
    let mut sim = cottage();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Select);
    let id = sim.app.cx.floor().openings[0].id;
    sim.app.cx.selection.set(ObjectRef::Opening(id));
    let _ = id;
    let (hinge, swing) = {
        let o = &sim.app.cx.floor().openings[0];
        (o.hinge_at_end, o.swing_flipped)
    };
    assert_one_undo_step(&mut sim, "reverse swing", |s| {
        s.app
            .cx
            .run_custom(crate::editor::edit_commands::ids::REVERSE_SWING);
    });
    let o = &sim.app.cx.floor().openings[0];
    assert_eq!(o.swing_flipped, !swing);
    assert_eq!(o.hinge_at_end, hinge, "the hinge stays where it was");
    sim.undo();
    assert_eq!(sim.app.cx.floor().openings[0].swing_flipped, swing);
    // Flip Hinge: the hinge moves to the other jamb, the swing side stays.
    assert_one_undo_step(&mut sim, "flip hinge", |s| {
        s.app
            .cx
            .run_custom(crate::editor::edit_commands::ids::FLIP_HINGE);
    });
    let o = &sim.app.cx.floor().openings[0];
    assert_eq!(o.hinge_at_end, !hinge);
    assert_eq!(o.swing_flipped, swing);
}

#[test]
fn two_close_windows_make_a_mulled_unit_that_is_one_object() {
    // Lesson 5 (guide p. 100): Make Mulled Unit joins neighbouring windows
    // into one object; Unmull separates them. One undo step each.
    use crate::editor::ObjectRef;
    let mut sim = cottage();
    sim.tool(ToolId::Window);
    sim.click(120.0, 0.0);
    sim.click(172.0, 0.0);
    sim.tool(ToolId::Select);
    let ids: Vec<_> = sim.app.cx.floor().openings.iter().map(|o| o.id).collect();
    assert_eq!(ids.len(), 2);
    sim.app.cx.selection.items = ids.iter().map(|i| ObjectRef::Opening(*i)).collect();
    assert_one_undo_step(&mut sim, "make mulled unit", |s| {
        s.app.cx.run_custom(crate::editor::opening_edit::MULL);
    });
    let fl = sim.app.cx.floor;
    assert_eq!(sim.app.cx.project.mull_members(fl, ids[0]).len(), 2);
    assert_eq!(sim.app.cx.project.mull_members(fl, ids[1]).len(), 2);
    assert_one_undo_step(&mut sim, "unmull", |s| {
        s.app.cx.selection.set(ObjectRef::Opening(ids[0]));
        s.app.cx.run_custom(crate::editor::opening_edit::UNMULL);
    });
    assert_eq!(sim.app.cx.project.mull_members(fl, ids[0]).len(), 1);
}

#[test]
#[ignore = "T7-05: TXT-65 (leader macro %comment% equals the door comment)"]
fn comment_macro_in_a_leader() {
    assert_ignored_break("TXT-65");
}
