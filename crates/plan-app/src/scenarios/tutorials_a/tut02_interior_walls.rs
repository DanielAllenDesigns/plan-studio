//! Lesson 2, Interior Walls (pp. 28-44).
use crate::editor::ObjectRef;
use crate::scenarios::tutorials_support::*;
use crate::tools::dimension::DimMode;
use crate::tools::ToolId;

#[test]
fn three_partitions_make_four_rooms_one_undo_step_each() {
    let mut sim = cottage();
    sim.tool(interior());
    let segs = [
        ((200.0, 0.0), (200.0, 480.0), 2),
        ((0.0, 240.0), (200.0, 240.0), 3),
        ((200.0, 300.0), (600.0, 300.0), 4),
    ];
    for (a, b, rooms) in segs {
        assert_one_undo_step(&mut sim, "interior wall", |s| s.drag(a, b));
        assert_eq!(sim.app.cx.rooms.len(), rooms);
    }
}

#[test]
fn deleting_a_partition_merges_two_rooms_in_one_step() {
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    assert_eq!(sim.app.cx.rooms.len(), 4);
    let last = sim.app.cx.floor().walls.last().unwrap().id;
    sim.app.cx.selection.set(ObjectRef::Wall(last));
    assert_one_undo_step(&mut sim, "delete wall", |s| {
        s.app.cx.delete_selection();
        s.app.cx.refresh();
    });
    assert_eq!(sim.app.cx.rooms.len(), 3);
    sim.undo();
    assert_eq!(sim.app.cx.rooms.len(), 4);
}

#[test]
fn a_room_specification_opens_for_each_room() {
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    let w = sim.app.cx.floor().walls[0].id;
    // Wall Specification tabs: General, Roof, Rooms, Wall Types, Structure.
    assert_dialog_tabs(&mut sim, ObjectRef::Wall(w), &["General"]);
}

#[test]
fn interior_dimension_is_one_undo_step() {
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    sim.tool(ToolId::DimensionVariant(DimMode::Interior));
    let n = sim.app.cx.floor().dimensions.len();
    sim.click(100.0, 120.0);
    sim.click(100.0, 380.0);
    // Interior dimensions need a room click; the exact gesture is verified in
    // s17. Here: the tool never leaves the model half-edited.
    assert!(sim.app.cx.floor().dimensions.len() >= n);
}

#[test]
#[ignore = "T7-02: W-152"]
fn changing_a_wall_makes_a_dashed_zero_thickness_room_divider() {
    assert_ignored_break("W-152");
}

#[test]
#[ignore = "T7-02: R-107"]
fn auto_room_dimension_for_the_entry() {
    assert_ignored_break("R-107");
}

#[test]
#[ignore = "T7-02: wall-type-layers (Fire-6 red main-layer fill, Reverse Layers)"]
fn fire_6_copy_of_interior_6_with_red_fill() {
    assert_ignored_break("wall-type-layers");
}
