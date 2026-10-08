//! Scenario 16: wall drawing with typed input and the modifier keys, then
//! the wall Edit commands (Break Wall, Reverse Layers, Change Line/Arc with
//! its bulge handle and the Arc section of the Wall Specification), the Snap
//! Settings window and Edit Behaviors > Resize (W-8, W-15..W-18, W-23, W-43,
//! W-44, W-66..W-68, S-65, S-68..S-72).

use super::{draw_shell, Sim};
use crate::dialogs::edit_behaviors;
use crate::dialogs::snap_settings::SnapDraft;
use crate::editor::handles::{self, HandleKind};
use crate::editor::snap::SnapKind;
use crate::editor::{wall_edit, ObjectRef};
use crate::toolbar::Action;
use crate::tools::cad::CadMode;
use crate::tools::{KeyEvent, ToolId};
use crate::ActiveDialog;
use eframe::egui::{Key, Modifiers};
use plan_core::cad::CadItem;
use plan_core::defaults::EditBehavior;
use plan_core::geometry::Point;
use plan_core::walls::WallCurve;
use plan_core::{Id, OpeningKind, WallKind};

fn exterior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Exterior,
    }
}

fn interior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Interior,
    }
}

/// A drag with the modifier keys held, the way the shell sends it.
fn drag_with(sim: &mut Sim, a: (f64, f64), b: (f64, f64), mods: Modifiers) {
    sim.move_to(a.0, a.1);
    sim.down(a.0, a.1);
    let ev = sim.event(b.0, b.1).with_down(true).with_modifiers(mods);
    let mv = sim.app.tools.active_mut().pointer_move(&mut sim.app.cx, ev);
    sim.finish(mv);
    let ev = sim.event(b.0, b.1).with_modifiers(mods);
    let up = sim.app.tools.active_mut().pointer_up(&mut sim.app.cx, ev);
    sim.finish(up);
}

fn angle_deg(a: Point, b: Point) -> f64 {
    (b.y - a.y).atan2(b.x - a.x).to_degrees()
}

fn wall(sim: &Sim, i: usize) -> plan_core::Wall {
    sim.app.cx.floor().walls[i].clone()
}

// ----- typed input and the modifier keys -----

#[test]
fn typing_twelve_feet_at_ninety_degrees_draws_a_vertical_wall_and_esc_drops_the_text() {
    let mut sim = Sim::new();
    sim.tool(interior());
    sim.click(0.0, 0.0);
    assert!(sim.app.cx.typed_input.is_armed());
    // Typed text is shown in the readout before Enter.
    sim.key(KeyEvent::text("12'"));
    assert!(sim.app.cx.typed_input.has_text());
    sim.key(KeyEvent::key(Key::Tab));
    sim.key(KeyEvent::text("90"));
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.floor_walls(), 1);
    let w = wall(&sim, 0);
    assert_eq!(w.start, Point::new(0.0, 0.0));
    assert!(w.end.dist(Point::new(0.0, 144.0)) < 1e-6, "{:?}", w.end);
    assert_eq!(sim.app.cx.undo_label(), Some("Draw Wall"));

    // The chain goes on from the new end: 8' at 0 degrees.
    sim.key(KeyEvent::text("8'"));
    sim.key(KeyEvent::key(Key::Tab));
    sim.key(KeyEvent::text("0"));
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.floor_walls(), 2);
    assert!(wall(&sim, 1).end.dist(Point::new(96.0, 144.0)) < 1e-6);

    // Esc with text typed clears the text but keeps the chain going.
    sim.key(KeyEvent::text("5'"));
    assert!(sim.app.cx.typed_input.has_text());
    sim.esc();
    assert!(!sim.app.cx.typed_input.has_text());
    assert_eq!(sim.floor_walls(), 2);
    // A second Esc ends the chain and keeps the finished walls (W-8).
    sim.esc();
    assert_eq!(sim.floor_walls(), 2);
    assert!(!sim.app.cx.typed_input.is_armed());
}

#[test]
fn shift_holds_the_angle_increment_and_alt_returns_the_raw_point() {
    // With the angle snaps off a plain drag to (200, 30) keeps its angle.
    let mut sim = Sim::new();
    sim.app.cx.defaults.editing.angle_snaps = false;
    sim.tool(interior());
    drag_with(&mut sim, (0.0, 0.0), (200.0, 30.0), Modifiers::NONE);
    let plain = wall(&sim, 0);
    let a = angle_deg(plain.start, plain.end);
    assert!((a / 15.0 - (a / 15.0).round()).abs() > 0.05, "plain {a}");

    // Shift holds the angle increment (15 degrees) even with them off.
    let mut sim = Sim::new();
    sim.app.cx.defaults.editing.angle_snaps = false;
    sim.tool(interior());
    drag_with(&mut sim, (0.0, 0.0), (200.0, 30.0), Modifiers::SHIFT);
    let w = wall(&sim, 0);
    let a = angle_deg(w.start, w.end);
    assert!((a / 15.0 - (a / 15.0).round()).abs() < 0.01, "shift {a}");

    // With the angle snaps on (the default) the plain drag lands on 15 too.
    let mut sim = Sim::new();
    sim.tool(interior());
    drag_with(&mut sim, (0.0, 0.0), (200.0, 30.0), Modifiers::NONE);
    let w = wall(&sim, 0);
    let a = angle_deg(w.start, w.end);
    assert!((a - 15.0).abs() < 0.01, "angle snap {a}");

    // Alt suspends every snap: the end is the pointer, to the thousandth.
    let mut sim = Sim::new();
    sim.tool(interior());
    drag_with(&mut sim, (0.0, 0.0), (200.37, 30.21), Modifiers::ALT);
    let w = wall(&sim, 0);
    assert!(w.end.dist(Point::new(200.37, 30.21)) < 1e-9, "{:?}", w.end);
}

// ----- Break Wall, Reverse Layers -----

fn shell_with_window() -> (Sim, Id, Id) {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.tool(ToolId::Window);
    sim.click(360.0, 0.0);
    sim.tool(ToolId::Select);
    let win = sim.app.cx.floor().openings[0].id;
    let wall = sim.app.cx.floor().openings[0].wall_id;
    (sim, wall, win)
}

#[test]
fn break_wall_splits_at_the_click_joins_both_halves_and_hands_the_window_to_one() {
    let (mut sim, wall_id, win) = shell_with_window();
    sim.app.cx.selection.set(ObjectRef::Wall(wall_id));
    let len = sim.app.cx.floor().wall(wall_id).unwrap().length();
    assert_eq!(sim.floor_walls(), 4);
    // The Edit toolbar button waits for a click on the wall.
    sim.app.cx.run_custom(wall_edit::BREAK_WALL);
    assert!(wall_edit::break_pending());
    // Esc backs out.
    sim.esc();
    assert!(!wall_edit::break_pending());
    assert_eq!(sim.floor_walls(), 4);
    sim.app.cx.run_custom(wall_edit::BREAK_WALL);
    let r = sim.click(120.0, 1.0);
    assert_eq!(r.commit.as_deref(), Some("Break Wall"));
    assert_eq!(sim.floor_walls(), 5);
    assert!(!wall_edit::break_pending());
    // Two joined walls make up the old one.
    let fl = sim.app.cx.floor;
    let ids: Vec<Id> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .filter(|w| w.start.y.abs() < 3.0 && w.end.y.abs() < 3.0)
        .map(|w| w.id)
        .collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    let conns = sim.app.cx.project.wall_connections(fl, ids[0]);
    assert!(conns.iter().any(|c| c.other == ids[1]), "{conns:?}");
    let (a, b) = (
        sim.app.cx.floor().wall(ids[0]).unwrap().length(),
        sim.app.cx.floor().wall(ids[1]).unwrap().length(),
    );
    assert!((a + b - len).abs() < 1.0, "{a} + {b} vs {len}");
    // The window (at 360) went to the long half and kept its place.
    let o = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == win)
        .unwrap();
    assert_eq!(o.kind, OpeningKind::Window);
    assert!(ids.contains(&o.wall_id));
    // One undo step brings the single wall back, the window with it.
    assert_eq!(sim.undo().as_deref(), Some("Break Wall"));
    assert_eq!(sim.floor_walls(), 4);
    let o = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == win)
        .unwrap();
    assert_eq!(o.wall_id, wall_id);
}

#[test]
fn a_break_inside_a_window_is_refused_and_remove_break_merges_the_halves_again() {
    let (mut sim, wall_id, _) = shell_with_window();
    sim.app.cx.selection.set(ObjectRef::Wall(wall_id));
    sim.app.cx.run_custom(wall_edit::BREAK_WALL);
    // 360 is the window's center: the break would cut through it.
    sim.click(360.0, 1.0);
    assert_eq!(sim.floor_walls(), 4);
    assert!(
        sim.app.cx.status.contains("Cannot break"),
        "{}",
        sim.app.cx.status
    );
    sim.esc();

    // Break elsewhere, select one half and Remove Break.
    sim.app.cx.selection.set(ObjectRef::Wall(wall_id));
    sim.app.cx.run_custom(wall_edit::BREAK_WALL);
    sim.click(100.0, 1.0);
    assert_eq!(sim.floor_walls(), 5);
    let half = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .find(|w| w.start.y.abs() < 3.0 && w.end.y.abs() < 3.0 && w.start.x < 50.0)
        .unwrap()
        .id;
    sim.app.cx.selection.set(ObjectRef::Wall(half));
    sim.app.cx.run_custom(wall_edit::REMOVE_BREAK);
    assert_eq!(sim.floor_walls(), 4);
    assert_eq!(sim.app.cx.undo_label(), Some("Remove Break"));
    assert_eq!(sim.app.cx.floor().openings.len(), 1);
}

#[test]
fn reverse_layers_swaps_the_wall_sides_in_plan_and_3d_and_undo_restores_them() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.tool(ToolId::Select);
    let before_sides: Vec<_> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| w.exterior_side)
        .collect();
    let hash0 = plan_view3d_hash(&sim);
    // Select the four walls with Select All, then Reverse Layers.
    sim.app.cx.run_custom("edit.select_all");
    assert!(sim.app.cx.selection.len() >= 4);
    sim.app.cx.run_custom(wall_edit::REVERSE_LAYERS);
    assert_eq!(sim.app.cx.undo_label(), Some("Reverse Layers"));
    for (w, was) in sim.app.cx.floor().walls.iter().zip(&before_sides) {
        assert_eq!(w.exterior_side, was.opposite());
    }
    assert_ne!(plan_view3d_hash(&sim), hash0, "the 3D view rebuilds");
    sim.undo();
    let after: Vec<_> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| w.exterior_side)
        .collect();
    assert_eq!(after, before_sides);
    assert_eq!(plan_view3d_hash(&sim), hash0);
}

fn plan_view3d_hash(sim: &Sim) -> u64 {
    crate::shell::view3d_panel::project_hash(&sim.app.cx.project)
}

// ----- Change Line/Arc, the bulge handle and the Arc section -----

fn one_wall() -> (Sim, Id) {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (240.0, 0.0));
    sim.tool(ToolId::Select);
    let id = sim.app.cx.floor().walls[0].id;
    sim.app.cx.selection.set(ObjectRef::Wall(id));
    (sim, id)
}

#[test]
fn change_line_arc_gives_a_bulge_handle_that_drags_and_straightens_the_wall() {
    let (mut sim, id) = one_wall();
    sim.tool(ToolId::Select);
    sim.app.cx.run_custom(wall_edit::CHANGE_LINE_ARC);
    let w = sim.app.cx.floor().wall(id).unwrap().clone();
    assert!(w.is_curved());
    // Straight to arc: the rise is a quarter of the chord.
    assert!((w.curve.unwrap().bulge.abs() - 60.0).abs() < 1e-6);
    // The apex handle drags the bulge.
    let apex = handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in)
        .into_iter()
        .find(|h| h.kind == HandleKind::Bulge)
        .expect("a Bulge handle on the curved wall")
        .pos;
    let sign = w.curve.unwrap().bulge.signum();
    sim.drag((apex.x, apex.y), (apex.x, apex.y + sign * 24.0));
    let bulged = sim.app.cx.floor().wall(id).unwrap().curve.unwrap().bulge;
    assert!((bulged.abs() - 84.0).abs() <= 1.0, "{bulged}");
    assert_eq!(sim.app.cx.undo_label(), Some("Curve Wall"));
    // Dragging the apex back to the chord straightens it.
    let apex = handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in)
        .into_iter()
        .find(|h| h.kind == HandleKind::Bulge)
        .unwrap()
        .pos;
    sim.drag((apex.x, apex.y), (apex.x, 0.0));
    assert!(!sim.app.cx.floor().wall(id).unwrap().is_curved());
    // Change Line/Arc is a toggle, and undo steps back through the edits.
    sim.undo();
    assert!(sim.app.cx.floor().wall(id).unwrap().is_curved());
    sim.undo();
    sim.undo();
    assert!(!sim.app.cx.floor().wall(id).unwrap().is_curved());
}

#[test]
fn the_wall_specification_arc_section_round_trips_radius_and_rise_through_ok() {
    let (mut sim, id) = one_wall();
    assert!(sim.open_spec(ObjectRef::Wall(id)));
    let chord = 240.0;
    {
        let Some(ActiveDialog::Wall(d)) = sim.app.dialog.as_mut() else {
            panic!("the Wall Specification is open")
        };
        // A 150" radius arc, bulging left of start to end.
        d.draft_mut().curve = WallCurve::from_radius(chord, 150.0, true);
        assert!(d.draft_mut().curve.is_some());
    }
    sim.ok();
    let w = sim.app.cx.floor().wall(id).unwrap().clone();
    assert!(w.is_curved(), "OK keeps the arc");
    let r = w.curve.unwrap().radius(chord).unwrap();
    assert!((r - 150.0).abs() < 1e-6, "radius {r}");
    assert!(w.curve.unwrap().bulge > 0.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Wall Specification"));
    // The ends did not move.
    assert_eq!(
        (w.start, w.end),
        (Point::new(0.0, 0.0), Point::new(240.0, 0.0))
    );

    // Reopen: the same arc is there, and setting the rise to 0 straightens it.
    assert!(sim.open_spec(ObjectRef::Wall(id)));
    {
        let Some(ActiveDialog::Wall(d)) = sim.app.dialog.as_mut() else {
            panic!("the Wall Specification is open")
        };
        let again = d.draft_mut().curve.unwrap();
        assert!((again.radius(chord).unwrap() - 150.0).abs() < 1e-6);
        // Through the sweep and back.
        let sweep = again.sweep_abs(chord);
        let back = WallCurve::from_sweep(chord, sweep, true).unwrap();
        assert!((back.bulge - again.bulge).abs() < 1e-6);
        d.draft_mut().curve = None;
    }
    sim.ok();
    assert!(!sim.app.cx.floor().wall(id).unwrap().is_curved());
}

// ----- Snap Settings and Edit Behaviors -----

#[test]
fn snap_settings_toggles_change_what_the_pointer_gets() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.drag((0.0, 0.0), (120.0, 0.0));
    sim.tool(ToolId::Select);
    let near_mid = Point::new(60.4, 2.3);
    let kind = |sim: &Sim| sim.app.cx.snap_at(near_mid, None, false, &[]).kind;
    assert_eq!(kind(&sim), SnapKind::Midpoint);

    // Open the window from the Edit menu and OK it unchanged: nothing moves.
    sim.action(Action::SnapSettings);
    sim.ok();
    assert_eq!(sim.app.cx.status, "Snap settings updated");
    assert_eq!(kind(&sim), SnapKind::Midpoint);

    // Midpoint off: the point falls back to something else.
    let mut d = SnapDraft::from_context(&sim.app.cx);
    d.editing.snap_midpoint = false;
    assert!(d.apply(&mut sim.app.cx));
    assert_ne!(kind(&sim), SnapKind::Midpoint);

    // The master switch off: no object snap of any kind.
    let mut d = SnapDraft::from_context(&sim.app.cx);
    d.editing.snap_midpoint = true;
    d.editing.object_snaps = false;
    assert!(d.apply(&mut sim.app.cx));
    assert!(!kind(&sim).is_object_snap(), "{:?}", kind(&sim));

    // Grid snaps off too: the raw point comes back.
    let mut d = SnapDraft::from_context(&sim.app.cx);
    d.editing.grid_snaps = false;
    assert!(d.apply(&mut sim.app.cx));
    assert_eq!(
        sim.app.cx.snap_at(near_mid, None, false, &[]).point,
        near_mid
    );

    // A bad angle list is refused and changes nothing.
    let mut bad = SnapDraft::from_context(&sim.app.cx);
    bad.angles_text = "0, abc".into();
    assert!(bad.error().is_some());
    assert!(!bad.apply(&mut sim.app.cx));
}

#[test]
fn an_angle_list_in_snap_settings_limits_the_directions_a_shift_drag_may_take() {
    let mut sim = Sim::new();
    let mut d = SnapDraft::from_context(&sim.app.cx);
    d.angles_text = "0, 45, 90".into();
    assert!(d.apply(&mut sim.app.cx));
    sim.tool(interior());
    // 30 degrees is not allowed: Shift picks the nearest of 0 / 45 / 90.
    drag_with(&mut sim, (0.0, 0.0), (200.0, 115.0), Modifiers::SHIFT);
    let w = wall(&sim, 0);
    let a = angle_deg(w.start, w.end).rem_euclid(180.0);
    assert!(
        [0.0, 45.0, 90.0].iter().any(|t| (a - t).abs() < 0.01),
        "{a}"
    );
}

#[test]
fn edit_behaviors_resize_scales_a_cad_selection_from_the_opposite_corner() {
    let mut sim = Sim::new();
    for (a, b) in [
        ((100.0, 100.0), (200.0, 100.0)),
        ((200.0, 100.0), (200.0, 200.0)),
    ] {
        // A press on the handle of the selected line would drag the handle
        // (the CAD tools edit the selected object), so deselect first.
        sim.app.cx.selection.clear();
        sim.tool(ToolId::CadVariant(CadMode::Line));
        sim.drag(a, b);
        sim.tool(ToolId::Select);
    }
    let ids: Vec<Id> = sim.app.cx.floor().cad.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), 2);
    sim.app.cx.selection.items = ids.iter().map(|i| ObjectRef::Cad(*i)).collect();

    // Edit > Edit Behaviors, Resize.
    sim.action(Action::EditBehaviors);
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    let mut draft = sim.app.cx.defaults.editing.behavior.clone();
    draft.mode = EditBehavior::Resize;
    sim.cancel();
    edit_behaviors::apply(&mut sim.app.cx, &draft);
    assert_eq!(sim.app.cx.status, "Edit behavior: Resize");

    // Grab the far corner of the L on its vertical leg and pull it out 100".
    sim.drag((200.0, 190.0), (300.0, 290.0));
    assert_eq!(sim.app.cx.undo_label(), Some("Resize Objects"));
    let ends: Vec<(Point, Point)> = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .map(|c| match c.item {
            CadItem::Line { a, b } => (a, b),
            _ => unreachable!(),
        })
        .collect();
    // The (100, 100) corner stays; everything scales 2x from there.
    assert!(ends[0].0.dist(Point::new(100.0, 100.0)) < 1.0, "{ends:?}");
    assert!(ends[0].1.dist(Point::new(300.0, 100.0)) < 1.5, "{ends:?}");
    assert!(ends[1].1.dist(Point::new(300.0, 300.0)) < 1.5, "{ends:?}");
    // Back to Default: the same drag moves the lines instead.
    sim.undo();
    draft.mode = EditBehavior::Default;
    edit_behaviors::apply(&mut sim.app.cx, &draft);
    sim.drag((200.0, 190.0), (230.0, 190.0));
    assert_eq!(sim.app.cx.undo_label(), Some("Move Objects"));
}
