//! Scenario 3: interior walls split the shell into three rooms; room names
//! and heights through the Room Specification (R-1, R-9, R-13..R-15, R-19,
//! R-23, R-24, R-33, W-35).

use super::{draw_shell, Sim};
use crate::dialogs::room::RoomDialog;
use crate::editor::{rooms_edit, EditorRequest};
use crate::tools::ToolId;
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::WallKind;

const W: f64 = 480.0;
const H: f64 = 360.0;

/// Shell plus two full-depth interior walls at x = 160 and x = 320.
fn three_room_house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((160.0, 0.0), (160.0, H + 1.0));
    sim.drag((320.0, 0.0), (320.0, H + 1.0));
    sim.app.cx.refresh();
    sim
}

#[test]
fn two_interior_walls_make_three_rooms() {
    let mut sim = three_room_house();
    let cx = &sim.app.cx;
    assert_eq!(cx.rooms.len(), 3, "R-9: a wall across the room splits it");
    let interior: Vec<_> = cx
        .floor()
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Interior)
        .collect();
    assert_eq!(interior.len(), 2);
    for w in &interior {
        assert_eq!(w.thickness, cx.wall_thickness(WallKind::Interior));
        // W-12: each end is a T on an exterior wall's centerline.
        for end in [w.start, w.end] {
            let on_exterior = cx
                .floor()
                .walls
                .iter()
                .filter(|o| o.kind == WallKind::Exterior)
                .any(|o| dist_to_segment(end, o.start, o.end) < 0.6);
            assert!(on_exterior, "interior wall end {end:?} floats");
        }
    }
    // The three rooms tile the shell: their interior areas add up to the
    // shell's interior minus the two partitions.
    let sum: f64 = cx.rooms.iter().map(|r| r.interior_area_sq_ft()).sum();
    let t_ext = cx.wall_thickness(WallKind::Exterior);
    let t_int = cx.wall_thickness(WallKind::Interior);
    let shell = (W - t_ext) * (H - t_ext) / 144.0;
    let partitions = 2.0 * t_int * (H - t_ext) / 144.0;
    assert!(
        (sum - (shell - partitions)).abs() / shell < 0.02,
        "rooms {sum} vs {}",
        shell - partitions
    );
    // The two end rooms are the same size, the middle one is the same too.
    let mut areas: Vec<f64> = cx.rooms.iter().map(|r| r.interior_area_sq_ft()).collect();
    areas.sort_by(f64::total_cmp);
    assert!(areas[2] / areas[0] < 1.05, "{areas:?}");
    // Unnamed rooms are labelled Room 1..3 (manual 4.2).
    let mut labels: Vec<_> = cx.rooms.iter().map(|r| r.label.clone()).collect();
    labels.sort();
    assert_eq!(labels, ["Room 1", "Room 2", "Room 3"]);
    let _ = &mut sim;
}

#[test]
fn undoing_a_partition_merges_the_rooms_again() {
    let mut sim = three_room_house();
    assert_eq!(sim.undo().as_deref(), Some("Draw Wall"));
    assert_eq!(sim.app.cx.rooms.len(), 2);
    sim.undo();
    assert_eq!(sim.app.cx.rooms.len(), 1);
    sim.redo();
    sim.redo();
    assert_eq!(sim.app.cx.rooms.len(), 3);
}

#[test]
fn double_click_in_a_room_requests_the_room_specification_and_it_opens() {
    let mut sim = three_room_house();
    sim.tool(ToolId::Select);
    sim.double_click(80.0, 180.0);
    // R-19: the click selected the room under the pointer ...
    let idx = rooms_edit::selected_room(&sim.app.cx).expect("room selected");
    assert!(idx < 3);
    // ... and asked for its specification (a room request, not OpenSpec).
    assert!(!sim
        .requests
        .iter()
        .any(|r| matches!(r, EditorRequest::OpenSpec(_))));
    assert_eq!(rooms_edit::take_room_dialog_request(&sim.app.cx), Some(idx));
    rooms_edit::request_room_dialog(&sim.app.cx, idx);
    // The dialog host picks the request up, draws the dialog, and OK applies
    // it as one undo step.
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(
        rooms_edit::take_room_dialog_request(&sim.app.cx).is_none(),
        "the dialog host consumed the request"
    );
    sim.dialog_frame(true);
    sim.dialog_frame(false);
    assert_eq!(sim.app.cx.undo_label(), Some("Room Specification"));
}

#[test]
fn rooms_are_named_through_the_room_specification_and_keep_their_names() {
    let mut sim = three_room_house();
    sim.app.cx.refresh();
    let names = ["Bedroom", "Kitchen", "Living Room"];
    // Name the rooms left to right (by the point inside each).
    for (i, (x, name)) in [80.0, 240.0, 400.0].iter().zip(names).enumerate() {
        sim.app.cx.refresh();
        let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(*x, 180.0)).unwrap();
        let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
        let mut d = RoomDialog::new(init);
        assert!(!d.has_error());
        d.set_name(name);
        let draft = d.room_name().clone();
        assert!(rooms_edit::apply_room_spec(
            &mut sim.app.cx,
            d.room_index(),
            &draft,
            d.extras()
        ));
        assert_eq!(sim.app.cx.undo_label(), Some("Room Specification"), "{i}");
    }
    for (x, name) in [80.0, 240.0, 400.0].iter().zip(names) {
        let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(*x, 180.0)).unwrap();
        let room = sim.app.cx.rooms[idx].clone();
        assert_eq!(sim.app.cx.room_name(&room), name);
    }
    // R-14: the names survive a later edit (a window in the south wall).
    sim.tool(ToolId::Window);
    sim.click(240.0, 0.0);
    let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(240.0, 180.0)).unwrap();
    let room = sim.app.cx.rooms[idx].clone();
    assert_eq!(sim.app.cx.room_name(&room), "Kitchen");
    // One undo step per rename: the last rename goes first.
    assert_eq!(sim.undo().as_deref(), Some("Place Window"));
    assert_eq!(sim.undo().as_deref(), Some("Room Specification"));
    let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(400.0, 180.0)).unwrap();
    let room = sim.app.cx.rooms[idx].clone();
    assert_ne!(sim.app.cx.room_name(&room), "Living Room");
}

#[test]
fn the_room_type_decides_the_name_and_living_area() {
    let mut sim = three_room_house();
    let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(80.0, 180.0)).unwrap();
    let before = rooms_edit::living_area_total_sq_ft(&sim.app.cx);
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let types: Vec<String> = init.types.iter().map(|t| t.name.clone()).collect();
    assert!(types.len() > 5, "Chief's room types are listed: {types:?}");
    let garage = types
        .iter()
        .find(|t| t.as_str() == "Garage")
        .cloned()
        .expect("Garage type");
    let mut d = RoomDialog::new(init);
    d.set_room_type(&garage);
    let draft = d.room_name().clone();
    rooms_edit::apply_room_spec(&mut sim.app.cx, idx, &draft, d.extras());
    // R-40..R-42: a garage is not living area.
    let after = rooms_edit::living_area_total_sq_ft(&sim.app.cx);
    assert!(after < before, "{after} !< {before}");
    // R-21: the auto name followed the type.
    let room = sim.app.cx.rooms[idx].clone();
    assert_eq!(sim.app.cx.room_name(&room), "Garage");
}

#[test]
fn the_room_specification_starts_from_the_floor_ceiling_height() {
    let mut sim = three_room_house();
    // R-57: the template's ceiling is 9'-1 1/8".
    assert_eq!(sim.app.cx.floor().ceiling_height, 109.125);
    let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(80.0, 180.0)).unwrap();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    assert_eq!(init.floor_ceiling_height, 109.125);
    assert_eq!(init.floor_elevation, sim.app.cx.floor().elevation);
    // R-24: a room-specific ceiling height is stored with the room.
    let mut d = RoomDialog::new(init);
    let mut draft = d.room_name().clone();
    draft.ceiling_height = Some(144.0);
    draft.floor_height_offset = 6.0;
    rooms_edit::apply_room_spec(&mut sim.app.cx, idx, &draft, d.extras_mut());
    let n = sim.app.cx.floor().room_names[0].clone();
    assert_eq!(n.ceiling_height, Some(144.0));
    assert_eq!(n.floor_height_offset, 6.0);
    // Re-opening shows the stored values.
    let again = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    assert_eq!(again.name.ceiling_height, Some(144.0));
    assert_eq!(sim.undo().as_deref(), Some("Room Specification"));
    assert!(
        sim.app.cx.floor().room_names.is_empty()
            || sim.app.cx.floor().room_names[0].ceiling_height.is_none()
    );
}

/// R-24 / R-33: a room's own ceiling height reaches the 3D platforms.
#[test]
#[ignore = "QA-02"]
fn a_room_ceiling_height_override_moves_that_rooms_3d_ceiling() {
    use crate::shell::view3d_panel::{build_view_scene, ViewScope};
    use plan_3d::Material;
    let mut sim = three_room_house();
    let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(80.0, 180.0)).unwrap();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let d = RoomDialog::new(init);
    let mut draft = d.room_name().clone();
    draft.ceiling_height = Some(144.0);
    rooms_edit::apply_room_spec(&mut sim.app.cx, idx, &draft, d.extras());
    let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let top = scene
        .meshes
        .iter()
        .filter(|m| m.material == Material::Ceiling)
        .flat_map(|m| m.vertices.iter())
        .filter(|v| v.position[0] < 150.0)
        .map(|v| v.position[1])
        .fold(f32::MIN, f32::max);
    assert!(top >= 144.0, "ceiling over the west room tops out at {top}");
}
