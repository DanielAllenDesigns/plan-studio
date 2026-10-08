//! Scenario 12: Daniel's hotkeys through the hotkey state machine and the
//! real egui key path (`D, H` hinged door, `-` zoom in, `2` `3` `4`
//! aliases, `Cmd+Z` undo, floor up / down), and what the actions do.

use super::{draw_shell, Sim};
use crate::shell::hotkeys::{self, Chord, HotkeyMap, HotkeyState};
use crate::toolbar::{Action, SEQUENCE_TIMEOUT};
use crate::tools::ToolId;
use eframe::egui::{self, Key, Modifiers};
use plan_core::WallKind;
use std::time::{Duration, Instant};

fn plain(key: Key) -> Chord {
    Chord {
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
        key,
    }
}

fn cmd(key: Key) -> Chord {
    Chord {
        meta: true,
        ..plain(key)
    }
}

fn ctrl(key: Key) -> Chord {
    Chord {
        ctrl: true,
        ..plain(key)
    }
}

/// Presses the chords one after the other, 100 ms apart, and returns the
/// actions they fired.
fn press_all(state: &mut HotkeyState, chords: &[Chord]) -> Vec<Action> {
    let mut out = Vec::new();
    let mut now = Instant::now();
    for c in chords {
        state.press(*c, now, &mut out);
        now += Duration::from_millis(100);
    }
    out
}

fn state() -> HotkeyState {
    // Defaults only: the user's own hotkeys.json must not matter.
    HotkeyState::new(HotkeyMap::defaults())
}

#[test]
fn d_then_h_is_the_hinged_door_tool() {
    let mut st = state();
    let fired = press_all(&mut st, &[plain(Key::D), plain(Key::H)]);
    assert_eq!(fired, vec![Action::SetTool(ToolId::Door)]);
    // D alone waits for the second key and says so in the status bar.
    let mut st = state();
    let fired = press_all(&mut st, &[plain(Key::D)]);
    assert!(fired.is_empty());
    assert_eq!(st.pending_label().as_deref(), Some("D, ..."));
    // The second key after the timeout starts over: H alone is not a door.
    let mut out = Vec::new();
    st.press(
        plain(Key::H),
        Instant::now() + SEQUENCE_TIMEOUT * 2,
        &mut out,
    );
    assert!(!out.contains(&Action::SetTool(ToolId::Door)), "{out:?}");
    // Escape drops the pending prefix.
    let mut st = state();
    press_all(&mut st, &[plain(Key::D), plain(Key::Escape)]);
    assert!(st.pending_label().is_none());
}

#[test]
fn the_other_d_sequences_pick_the_other_door_flavors_and_dimensions() {
    let mut st = state();
    // Each D, x is a different command; none of them is the hinged door.
    let mut seen = Vec::new();
    for second in [Key::E, Key::I, Key::P, Key::W] {
        let fired = press_all(&mut st, &[plain(Key::D), plain(second)]);
        seen.push((second, fired));
    }
    for (k, fired) in &seen {
        assert!(!fired.contains(&Action::SetTool(ToolId::Door)), "D,{k:?}");
    }
    // D, E is End to End Dimension (Daniel's Chief key).
    assert_eq!(
        seen[0].1,
        vec![Action::SetTool(ToolId::DimensionVariant(
            crate::tools::dimension::DimMode::EndToEnd
        ))]
    );
}

#[test]
fn minus_zooms_in_and_the_action_scales_the_view() {
    let mut st = state();
    assert_eq!(
        press_all(&mut st, &[plain(Key::Minus)]),
        vec![Action::ZoomIn]
    );
    let mut sim = Sim::new();
    let before = sim.app.camera.px_per_in;
    sim.action(Action::ZoomIn);
    assert!(sim.app.camera.px_per_in > before);
    // Undo Zoom brings the old view back.
    sim.action(Action::UndoZoom);
    assert!((sim.app.camera.px_per_in - before).abs() < 1e-9);
}

#[test]
fn number_keys_are_aliases_for_select_wall_door_and_window() {
    let mut st = state();
    assert_eq!(
        press_all(&mut st, &[plain(Key::Num1)]),
        vec![Action::SetTool(ToolId::Select)]
    );
    assert_eq!(
        press_all(&mut st, &[plain(Key::Num2)]),
        vec![Action::CurrentWall]
    );
    assert_eq!(
        press_all(&mut st, &[plain(Key::Num3)]),
        vec![Action::SetTool(ToolId::Door)]
    );
    assert_eq!(
        press_all(&mut st, &[plain(Key::Num4)]),
        vec![Action::SetTool(ToolId::Window)]
    );
    // And the actions land on the right tools in the app.
    let mut sim = Sim::new();
    for (key, name) in [
        (Key::Num3, "Hinged Door"),
        (Key::Num4, "Window"),
        (Key::Num1, "Select Objects"),
    ] {
        let fired = press_all(&mut state(), &[plain(key)]);
        for a in fired {
            sim.action(a);
        }
        assert_eq!(sim.app.tools.active().name(), name, "{key:?}");
    }
    // `2` is the current wall tool (exterior until another one is picked).
    for a in press_all(&mut state(), &[plain(Key::Num2)]) {
        sim.action(a);
    }
    assert!(
        sim.app.tools.active().name().contains("Wall"),
        "{}",
        sim.app.tools.active().name()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn command_z_is_undo_and_it_undoes_the_last_wall() {
    let mut st = state();
    assert_eq!(press_all(&mut st, &[cmd(Key::Z)]), vec![Action::Undo]);
    // Redo is Cmd+Y in Daniel's file, Shift+Cmd+Z as Plan Studio's alias.
    assert_eq!(press_all(&mut state(), &[cmd(Key::Y)]), vec![Action::Redo]);
    let mut sim = Sim::new();
    sim.tool(ToolId::Wall {
        kind: WallKind::Exterior,
    });
    sim.drag((0.0, 0.0), (240.0, 0.0));
    assert_eq!(sim.floor_walls(), 1);
    for a in press_all(&mut state(), &[cmd(Key::Z)]) {
        sim.action(a);
    }
    assert_eq!(sim.floor_walls(), 0);
    assert_eq!(sim.app.cx.status, "Undid Draw Wall");
    for a in press_all(&mut state(), &[cmd(Key::Y)]) {
        sim.action(a);
    }
    assert_eq!(sim.floor_walls(), 1);
    assert_eq!(sim.app.cx.status, "Redid Draw Wall");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn control_z_is_bound_off_the_mac() {
    // One Ctrl key: whichever of Daniel's chords owns Ctrl+Z, it is bound.
    let map = HotkeyMap::defaults();
    assert!(map.lookup(&[cmd(Key::Z).normalized()]).is_some());
}

#[cfg(target_os = "macos")]
#[test]
fn control_z_and_control_a_move_the_floor_down_and_up() {
    let mut st = state();
    assert_eq!(press_all(&mut st, &[ctrl(Key::Z)]), vec![Action::FloorDown]);
    assert_eq!(press_all(&mut st, &[ctrl(Key::A)]), vec![Action::FloorUp]);
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.action(Action::BuildNewFloor);
    sim.ok();
    assert_eq!(sim.app.cx.floor, 1);
    for a in press_all(&mut state(), &[ctrl(Key::Z)]) {
        sim.action(a);
    }
    assert_eq!(sim.app.cx.floor, 0);
    for a in press_all(&mut state(), &[ctrl(Key::A)]) {
        sim.action(a);
    }
    assert_eq!(sim.app.cx.floor, 1);
}

/// The same keys through egui's own input path (`hotkeys::handle`), the way
/// the shell reads them every frame.
#[test]
fn keys_pressed_in_egui_frames_fire_the_sequence() {
    let sim = Sim::new();
    let mut st = state();
    let ctx = egui::Context::default();
    let mut actions = Vec::new();
    let key_frame =
        |ctx: &egui::Context, st: &mut HotkeyState, key: Key, actions: &mut Vec<Action>| {
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
            let _ = ctx.run(input, |ctx| hotkeys::handle(ctx, &sim.app.cx, st, actions));
        };
    key_frame(&ctx, &mut st, Key::D, &mut actions);
    assert!(actions.is_empty());
    key_frame(&ctx, &mut st, Key::H, &mut actions);
    assert_eq!(actions, vec![Action::SetTool(ToolId::Door)]);
    actions.clear();
    key_frame(&ctx, &mut st, Key::Minus, &mut actions);
    assert_eq!(actions, vec![Action::ZoomIn]);
}

#[test]
fn daniels_chief_keys_all_land_on_commands_and_no_chord_is_bound_twice() {
    let map = HotkeyMap::defaults();
    let s = map.daniel_stats();
    assert!(s.named >= 100, "{s:?}");
    assert!(s.live > 15, "{s:?}");
    assert_eq!(s.mapped + s.unmapped, s.named);
    // Every live command with a key has a unique, non-empty sequence.
    let mut owner: std::collections::HashMap<Vec<Chord>, String> = Default::default();
    for c in map.commands() {
        for seq in map.sequences(&c.name) {
            if let Some(prev) = owner.insert(seq.clone(), c.name.clone()) {
                panic!("{} and {prev} share a key sequence", c.name);
            }
        }
    }
}
