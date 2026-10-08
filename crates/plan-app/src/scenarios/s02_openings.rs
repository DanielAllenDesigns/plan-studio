//! Scenario 2: doors and windows placed by click on the shell's walls, the
//! Door / Window Specification dialogs and their apply path
//! (DW-1, DW-4, DW-6, DW-7, DW-9, DW-31, DW-32, DW-77, DW-78).

use super::{draw_shell, Sim};
use crate::dialogs::OpeningTarget;
use crate::editor::{EditorRequest, ObjectRef};
use crate::tools::ToolId;
use crate::ActiveDialog;
use plan_core::{Opening, OpeningKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn openings(sim: &Sim) -> Vec<Opening> {
    sim.app.cx.floor().openings.clone()
}

/// The south wall (id, start x) so click positions can be turned into offsets.
fn south_wall(sim: &Sim) -> plan_core::Wall {
    sim.app.cx.floor().walls[0].clone()
}

#[test]
fn a_door_click_centers_the_opening_with_the_default_door_size() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    assert_eq!(sim.app.tools.active().name(), "Hinged Door");
    let wall = south_wall(&sim);

    let r = sim.click(120.4, 0.0);
    assert_eq!(
        r.commit.as_deref(),
        Some("Place Door"),
        "DW-77: one undo step"
    );
    let doors = openings(&sim);
    assert_eq!(doors.len(), 1);
    let d = &doors[0];
    assert_eq!(d.kind, OpeningKind::Door);
    assert_eq!(d.wall_id, wall.id);
    // DW-9: the center is under the pointer, snapped to 1".
    assert_eq!(d.center_offset, 120.0);
    // DW-6: an exterior wall takes the Exterior Door defaults.
    let defaults = sim.app.cx.defaults.exterior_door.clone();
    assert_eq!((d.width, d.height), (defaults.width, defaults.height));
    assert_eq!((d.width, d.height, d.sill_height), (36.0, 96.0, 0.0));
    // DW-77: the new door is selected (not its wall) so its Edit toolbar
    // shows, and it keeps its dialog extras for this session.
    assert_eq!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::Opening(d.id))
    );
    assert!(sim.app.cx.extras.openings.contains_key(&d.id));
    // The tool stays active for more openings (DW-1).
    assert_eq!(sim.app.tools.active_id(), ToolId::Door);
}

#[test]
fn a_door_click_off_the_south_wall_uses_the_wall_not_empty_space() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    // DW-3: nothing happens in the middle of the room.
    sim.click(240.0, 180.0);
    assert!(openings(&sim).is_empty());
    assert!(!sim.app.cx.can_undo() || sim.app.cx.undo_label() == Some("Draw Wall"));
}

#[test]
fn interior_door_defaults_apply_on_interior_walls() {
    let mut sim = house();
    sim.tool(ToolId::Wall {
        kind: plan_core::WallKind::Interior,
    });
    sim.drag((240.0, 2.0), (240.0, H));
    sim.tool(ToolId::Door);
    let interior = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .find(|w| w.kind == plan_core::WallKind::Interior)
        .unwrap()
        .clone();
    let mid = (interior.start + interior.end) * 0.5;
    sim.click(mid.x, mid.y);
    let d = openings(&sim).pop().unwrap();
    // DW-7: Width 30", Height 96".
    assert_eq!((d.width, d.height), (30.0, 96.0));
    assert_eq!(d.wall_id, interior.id);
}

#[test]
fn a_window_click_uses_the_window_defaults_and_sill() {
    let mut sim = house();
    sim.tool(ToolId::Window);
    assert_eq!(sim.app.tools.active().name(), "Window");
    let r = sim.click(300.0, 0.0);
    assert_eq!(r.commit.as_deref(), Some("Place Window"));
    let w = openings(&sim).pop().unwrap();
    assert_eq!(w.kind, OpeningKind::Window);
    assert_eq!(w.center_offset, 300.0);
    let d = &sim.app.cx.defaults.window;
    assert_eq!(
        (w.width, w.height, w.sill_height),
        (d.width, d.height, d.sill_height)
    );
    assert!(w.sill_height > 0.0, "a window sits above the floor");
}

#[test]
fn overlapping_or_too_wide_placements_are_refused_without_an_undo_step() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    let undo_before = sim.app.cx.undo_label().map(String::from);
    // DW-4, DW-78: a second door on top of the first is refused.
    sim.click(130.0, 0.0);
    assert_eq!(openings(&sim).len(), 1);
    assert!(
        sim.app.cx.status.contains("does not fit"),
        "{}",
        sim.app.cx.status
    );
    assert_eq!(sim.app.cx.undo_label().map(String::from), undo_before);
    // Windows go next to it without trouble.
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    assert_eq!(openings(&sim).len(), 2);
}

#[test]
fn door_and_window_placement_each_undo_and_redo_in_one_step() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(100.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    assert_eq!(sim.app.cx.floor().openings.len(), 2);
    assert_eq!(sim.undo().as_deref(), Some("Place Window"));
    assert_eq!(sim.app.cx.floor().openings.len(), 1);
    assert_eq!(sim.undo().as_deref(), Some("Place Door"));
    assert!(sim.app.cx.floor().openings.is_empty());
    sim.redo();
    sim.redo();
    assert_eq!(sim.app.cx.floor().openings.len(), 2);
    // The room survives openings (DW-67).
    assert_eq!(sim.app.cx.rooms.len(), 1);
}

#[test]
fn double_click_on_a_door_requests_the_door_specification_and_the_dialog_opens() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Select);
    sim.requests.clear();
    sim.double_click(120.0, 0.0);
    let door = openings(&sim)[0].clone();
    assert!(
        sim.requests
            .contains(&EditorRequest::OpenSpec(ObjectRef::Opening(door.id))),
        "{:?}",
        sim.requests
    );
    match &sim.app.dialog {
        Some(ActiveDialog::Opening(d)) => {
            assert_eq!(d.target(), OpeningTarget::Placed(door.id));
            assert_eq!(d.draft().width, 36.0);
        }
        _ => panic!("the Door Specification did not open"),
    }
}

#[test]
fn applying_a_width_change_in_the_door_dialog_is_one_undo_step() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Select);
    sim.double_click(120.0, 0.0);
    let id = openings(&sim)[0].id;
    if let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() {
        d.draft_mut().width = 42.0;
    } else {
        panic!("no door dialog");
    }
    let steps_before = sim.app.cx.undo_label().map(String::from);
    assert_eq!(steps_before.as_deref(), Some("Place Door"));
    // OK (Enter) goes through PlanApp::dialogs and apply_opening_dialog.
    sim.ok();
    assert!(!sim.app.has_dialog(), "OK closes the dialog");
    let d = openings(&sim)[0].clone();
    assert_eq!(d.id, id);
    assert_eq!(d.width, 42.0);
    assert_eq!(d.center_offset, 120.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Opening Specification"));
    // One undo step: back to the placed 36" door, and the next undo removes it.
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    assert_eq!(openings(&sim)[0].width, 36.0);
    assert_eq!(sim.undo().as_deref(), Some("Place Door"));
    assert!(openings(&sim).is_empty());
}

#[test]
fn cancelling_the_window_dialog_changes_nothing() {
    let mut sim = house();
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Select);
    sim.double_click(300.0, 0.0);
    assert!(matches!(sim.app.dialog, Some(ActiveDialog::Opening(_))));
    if let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() {
        assert_eq!(d.draft().kind, OpeningKind::Window);
        d.draft_mut().width = 60.0;
        d.draft_mut().sill_height = 40.0;
    }
    sim.cancel();
    assert!(!sim.app.has_dialog());
    let w = openings(&sim)[0].clone();
    assert_eq!((w.width, w.sill_height), (32.0, 24.0));
    assert_eq!(sim.app.cx.undo_label(), Some("Place Window"));
}

#[test]
fn window_dialog_edits_size_and_sill_as_one_step() {
    let mut sim = house();
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Select);
    sim.double_click(300.0, 0.0);
    if let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() {
        d.draft_mut().width = 48.0;
        d.draft_mut().height = 36.0;
        d.draft_mut().sill_height = 36.0;
    }
    sim.ok();
    let w = openings(&sim)[0].clone();
    assert_eq!((w.width, w.height, w.sill_height), (48.0, 36.0, 36.0));
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    assert_eq!(sim.undo().as_deref(), Some("Place Window"));
}

#[test]
fn reverse_swing_flips_the_leaf_side_in_one_undo_step() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    let id = openings(&sim)[0].id;
    let (swing, hinge) = (
        openings(&sim)[0].swing_flipped,
        openings(&sim)[0].hinge_at_end,
    );
    sim.app.cx.selection.set(ObjectRef::Opening(id));
    sim.app.cx.reverse_swing();
    let d = openings(&sim)[0].clone();
    // DW-32: swing flips; hinge is unchanged.
    assert_ne!(d.swing_flipped, swing);
    assert_eq!(d.hinge_at_end, hinge);
    assert_eq!(sim.undo().as_deref(), Some("Reverse Swing"));
    assert_eq!(openings(&sim)[0].swing_flipped, swing);
}

#[test]
fn a_centerline_click_near_the_wall_start_gives_the_default_swing_and_start_hinge() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    let d = openings(&sim)[0].clone();
    // A click on the centerline of a wall near its start: hinge at the wall
    // start and the swing on the wall's default side (DW-8 / DW-76; the test
    // below covers the pointer side and the far end).
    assert!(!d.hinge_at_end && !d.swing_flipped);
}

/// DW-8 / DW-76: the swing side follows the side of the wall the pointer is
/// on and the hinge goes to the jamb nearer a wall end.
#[test]
fn door_swing_follows_the_pointer_side_and_hinge_the_nearer_end() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    // Pointer 3" on one side of the south wall near its start ...
    sim.click(60.0, 3.0);
    // ... and 3" on the other side near its end.
    sim.click(420.0, -3.0);
    let ds = openings(&sim);
    assert_eq!(ds.len(), 2);
    let seen = format!(
        "door 1 swing_flipped={} hinge_at_end={}; door 2 swing_flipped={} hinge_at_end={}",
        ds[0].swing_flipped, ds[0].hinge_at_end, ds[1].swing_flipped, ds[1].hinge_at_end
    );
    assert_ne!(
        ds[0].swing_flipped, ds[1].swing_flipped,
        "swing follows the pointer side: {seen}"
    );
    assert_ne!(
        ds[0].hinge_at_end, ds[1].hinge_at_end,
        "hinge is on the nearer end: {seen}"
    );
}
