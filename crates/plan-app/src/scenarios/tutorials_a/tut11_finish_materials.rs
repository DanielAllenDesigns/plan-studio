//! Lesson 11, Finish Materials (pp. 191-211).
use crate::dialogs::room::RoomDialog;
use crate::editor::rooms_edit;
use crate::scenarios::tutorials_support::*;
use plan_core::geometry::Point;

#[test]
fn a_room_floor_finish_name_is_one_undo_step() {
    let mut sim = cottage();
    cottage_partitions(&mut sim);
    let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(100.0, 120.0)).unwrap();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let mut d = RoomDialog::new(init);
    let mut draft = d.room_name().clone();
    draft.floor_finish = Some("Carpet".into());
    assert_one_undo_step(&mut sim, "Room Specification", |s| {
        rooms_edit::apply_room_spec(&mut s.app.cx, idx, &draft, d.extras_mut())
    });
    assert!(sim
        .app
        .cx
        .floor()
        .room_names
        .iter()
        .any(|n| n.floor_finish.as_deref() == Some("Carpet")));
}

#[test]
#[ignore = "T7-11: R-122 (layered floor finish: kitchen layers equal the bath's via Object Eyedropper)"]
fn layered_floor_finish() {
    assert_ignored_break("R-122");
}

#[test]
#[ignore = "T7-11: CB-646 (User Catalog folder and colours copied into it)"]
fn user_catalog_beach_palette() {
    assert_ignored_break("CB-646");
}

#[test]
#[ignore = "T7-11: CAD-101 (Simplify Polyline on a material region)"]
fn simplify_polyline() {
    assert_ignored_break("CAD-101");
}

#[test]
#[ignore = "T7-11: L-233 (Room Finish Schedule excluding Attic, Deck, Porch)"]
fn room_finish_schedule_scope() {
    assert_ignored_break("L-233");
}
