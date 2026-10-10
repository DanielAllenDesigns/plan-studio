//! Scenario 14: typed wall dimensions through the shell's key path, the wall
//! Edit toolbar commands, and the Snap Settings and Edit Behaviors windows.

use super::Sim;
use crate::editor::snap::SnapKind;
use crate::editor::{EditActionKind, ObjectRef};
use crate::shell::hotkeys::{self, HotkeyMap, HotkeyState};
use crate::toolbar::Action;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::{self, Key, Modifiers};
use plan_core::geometry::Point;
use plan_core::WallKind;

fn exterior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Exterior,
    }
}

/// Presses `key` for one frame of egui's own hotkey path and returns what it fired.
fn hotkey_frame(sim: &Sim, key: Key) -> Vec<Action> {
    let mut st = HotkeyState::new(HotkeyMap::defaults());
    let ctx = egui::Context::default();
    let mut actions = Vec::new();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        events: vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        hotkeys::handle(ctx, &sim.app.cx, &mut st, &mut actions)
    });
    actions
}

fn typed(sim: &mut Sim, length: &str, angle: &str) {
    sim.key(KeyEvent::text(length));
    sim.key(KeyEvent::key(Key::Tab));
    sim.key(KeyEvent::text(angle));
    sim.key(KeyEvent::key(Key::Enter));
}

#[test]
fn typed_dimensions_draw_a_closed_room_and_digits_are_not_hotkeys_meanwhile() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    // Nothing is being drawn: `2` is the Current Wall hotkey.
    assert!(!hotkey_frame(&sim, Key::Num2).is_empty());
    sim.click(0.0, 0.0);
    assert!(sim.app.cx.typed_input.is_armed());
    // Mid-chain the digit belongs to the dimension entry.
    assert!(hotkey_frame(&sim, Key::Num2).is_empty());
    typed(&mut sim, "12'", "0");
    typed(&mut sim, "10'", "90");
    typed(&mut sim, "12'", "180");
    typed(&mut sim, "10'", "270");
    assert_eq!(sim.floor_walls(), 4);
    let walls = &sim.app.cx.floor().walls;
    assert_eq!(walls[0].end, Point::new(144.0, 0.0));
    assert_eq!(walls[1].end, Point::new(144.0, 120.0));
    assert_eq!(walls[2].end, Point::new(0.0, 120.0));
    assert_eq!(walls[3].end, Point::new(0.0, 0.0));
    // The loop closed: one room, the chain over, the digits are hotkeys again.
    assert_eq!(sim.app.cx.rooms.len(), 1);
    assert!(!sim.app.cx.typed_input.is_armed());
    assert!(!hotkey_frame(&sim, Key::Num2).is_empty());
    // One undo step per wall.
    for _ in 0..4 {
        assert_eq!(sim.undo().as_deref(), Some("Draw Wall"));
    }
    assert_eq!(sim.floor_walls(), 0);
}

#[test]
fn the_wall_edit_commands_are_toolbar_buttons_that_run() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (240.0, 0.0));
    sim.tool(ToolId::Select);
    let id = sim.app.cx.floor().walls[0].id;
    sim.app.cx.selection.set(ObjectRef::Wall(id));
    let labels = |sim: &Sim| -> Vec<(&'static str, bool)> {
        sim.app
            .tools
            .active()
            .edit_toolbar(&sim.app.cx)
            .iter()
            .map(|a| (a.label, a.enabled))
            .collect()
    };
    let want = |sim: &Sim, name: &str| labels(sim).into_iter().find(|(l, _)| *l == name);
    for name in [
        "Reverse Layers",
        "Break Wall",
        "Remove Break",
        "Change Line/Arc",
        "Convert to Polyline",
    ] {
        assert_eq!(want(&sim, name).map(|x| x.1), Some(true), "{name}");
    }
    // Make Arc Tangent waits for a curved wall.
    assert_eq!(want(&sim, "Make Arc Tangent").map(|x| x.1), Some(false));

    // Change Line/Arc through the toolbar's own path.
    let run = |sim: &mut Sim, label: &str| {
        let a = sim
            .app
            .tools
            .active()
            .edit_toolbar(&sim.app.cx)
            .into_iter()
            .find(|a| a.label == label)
            .unwrap();
        assert!(matches!(a.kind, EditActionKind::Custom { .. }));
        sim.app.cx.apply_edit_action(a.kind);
    };
    run(&mut sim, "Change Line/Arc");
    assert!(sim.app.cx.floor().walls[0].is_curved());
    assert_eq!(want(&sim, "Make Arc Tangent").map(|x| x.1), Some(true));
    run(&mut sim, "Change Line/Arc");
    assert!(!sim.app.cx.floor().walls[0].is_curved());

    // Reverse Layers flips the side; Break Wall asks for a click.
    let side = sim.app.cx.floor().walls[0].exterior_side;
    run(&mut sim, "Reverse Layers");
    assert_eq!(sim.app.cx.floor().walls[0].exterior_side, side.opposite());
    run(&mut sim, "Break Wall");
    let r = sim.click(100.0, 1.0);
    assert_eq!(r.commit.as_deref(), Some("Break Wall"));
    assert_eq!(sim.floor_walls(), 2);
}

#[test]
fn snap_settings_and_edit_behaviors_open_from_the_edit_menu_and_close_with_keys() {
    let mut sim = Sim::new();
    sim.action(Action::SnapSettings);
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    // Esc cancels: no status message.
    sim.dialog_frame_key(Some(Key::Escape));
    sim.dialog_frame(false);
    assert_ne!(sim.app.cx.status, "Snap settings updated");
    sim.action(Action::SnapSettings);
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame(true);
    sim.dialog_frame(false);
    assert_eq!(sim.app.cx.status, "Snap settings updated");

    sim.action(Action::EditBehaviors);
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame(true);
    sim.dialog_frame(false);
    assert_eq!(sim.app.cx.status, "Edit behavior: Default");
}

#[test]
fn turning_a_snap_off_changes_what_the_pointer_gets() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (120.0, 0.0));
    let raw = Point::new(121.3, 2.2);
    assert_eq!(
        sim.app.cx.snap_at(raw, None, false, &[]).kind,
        SnapKind::Endpoint
    );
    sim.app.cx.defaults.editing.snap_endpoint = false;
    assert_ne!(
        sim.app.cx.snap_at(raw, None, false, &[]).kind,
        SnapKind::Endpoint
    );
    // Ctrl/Cmd (override) returns the raw point whatever is on.
    sim.app.cx.defaults.editing.snap_endpoint = true;
    assert_eq!(sim.app.cx.snap_at(raw, None, true, &[]).point, raw);
}
