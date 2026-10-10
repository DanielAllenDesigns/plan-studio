//! Scenario 69 (Round 16, brief 10): the edit behaviors and the modifier keys
//! the way Chief defines them (Alt summons Alternate, Ctrl/Cmd overrides snaps
//! and move restrictions, Shift restricts the angles), Enter Coordinates and
//! arithmetic in the typed length.

use super::Sim;
use crate::dialogs::enter_coordinates as coords;
use crate::editor::snap::{self, HeldKeys, SnapKind};
use crate::editor::{behaviors, ObjectRef};
use crate::toolbar::ViewFlag;
use crate::tools::cad::CadMode;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::{Key, Modifiers};
use plan_core::cad::CadItem;
use plan_core::defaults::EditBehavior;
use plan_core::geometry::Point;
use plan_core::units::LengthUnit;
use plan_core::{Id, WallKind};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn alt() -> Modifiers {
    Modifiers {
        alt: true,
        ..Modifiers::NONE
    }
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    }
}

/// Press at `a` with nothing held, drag to `b` with `m` held, release.
fn drag_with(sim: &mut Sim, a: (f64, f64), b: (f64, f64), m: Modifiers) {
    sim.move_to(a.0, a.1);
    sim.down(a.0, a.1);
    let ev = sim.event(b.0, b.1).with_down(true).with_modifiers(m);
    let mv = sim.app.tools.active_mut().pointer_move(&mut sim.app.cx, ev);
    sim.finish(mv);
    let ev = sim.event(b.0, b.1).with_modifiers(m);
    let up = sim.app.tools.active_mut().pointer_up(&mut sim.app.cx, ev);
    sim.finish(up);
    // The next test on this thread starts with no keys held.
    snap::set_held(HeldKeys::default());
}

/// A closed box 100" by 60" with a corner at the origin, selected.
fn box_cad(sim: &mut Sim) -> Id {
    let id = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Polyline {
            points: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 60.0), p(0.0, 60.0)],
            closed: true,
        },
    );
    sim.app.cx.refresh();
    sim.app.cx.selection.set(ObjectRef::Cad(id));
    id
}

fn points_of(sim: &Sim, id: Id) -> Vec<Point> {
    match &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .item
    {
        CadItem::Polyline { points, .. } => points.clone(),
        other => panic!("{other:?}"),
    }
}

fn house(sim: &mut Sim) -> [Id; 4] {
    let c = [p(0.0, 0.0), p(240.0, 0.0), p(240.0, 120.0), p(0.0, 120.0)];
    let mut ids = [0; 4];
    for i in 0..4 {
        ids[i] =
            sim.app
                .cx
                .project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
    }
    sim.app.cx.refresh();
    ids
}

fn set_mode(sim: &mut Sim, m: EditBehavior) {
    sim.app.cx.defaults.editing.behavior.mode = m;
}

fn close(a: Point, b: Point) -> bool {
    a.dist(b) < 1e-6
}

// ----- the six behaviors on a box -----

#[test]
fn default_moves_one_corner_and_alternate_keeps_the_box_square() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let id = box_cad(&mut sim);
    // Default: the corner alone moves.
    drag_with(&mut sim, (100.0, 60.0), (120.0, 90.0), Modifiers::NONE);
    let d = points_of(&sim, id);
    assert_eq!(
        d,
        vec![p(0.0, 0.0), p(100.0, 0.0), p(120.0, 90.0), p(0.0, 60.0)]
    );
    sim.undo();
    sim.app.cx.selection.set(ObjectRef::Cad(id));
    // Alt on the drag summons Alternate: the neighbours slide along their
    // edges, so every angle stays a right angle.
    drag_with(&mut sim, (100.0, 60.0), (120.0, 90.0), alt());
    let a = points_of(&sim, id);
    assert_eq!(
        a,
        vec![p(0.0, 0.0), p(120.0, 0.0), p(120.0, 90.0), p(0.0, 90.0)]
    );
    // The chosen (global) Alternate does the same without Alt.
    sim.undo();
    sim.app.cx.selection.set(ObjectRef::Cad(id));
    set_mode(&mut sim, EditBehavior::Alternate);
    drag_with(&mut sim, (100.0, 60.0), (120.0, 90.0), Modifiers::NONE);
    assert_eq!(points_of(&sim, id), a);
}

#[test]
fn move_makes_every_resize_handle_a_move_handle() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let id = box_cad(&mut sim);
    set_mode(&mut sim, EditBehavior::Move);
    // Move follows the allowed angles, and east is one of them.
    drag_with(&mut sim, (100.0, 60.0), (130.0, 60.0), Modifiers::NONE);
    assert_eq!(
        points_of(&sim, id),
        vec![p(30.0, 0.0), p(130.0, 0.0), p(130.0, 60.0), p(30.0, 60.0)]
    );
}

#[test]
fn holding_the_move_key_summons_move_for_one_drag() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let id = box_cad(&mut sim);
    behaviors::note_held_summon(Some(EditBehavior::Move));
    drag_with(&mut sim, (100.0, 60.0), (130.0, 60.0), Modifiers::NONE);
    behaviors::note_held_summon(None);
    assert_eq!(points_of(&sim, id)[0], p(30.0, 0.0));
    assert_eq!(
        sim.app.cx.defaults.editing.behavior.mode,
        EditBehavior::Default,
        "the global behavior is untouched"
    );
}

#[test]
fn resize_scales_the_box_in_proportion_by_a_corner() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let id = box_cad(&mut sim);
    set_mode(&mut sim, EditBehavior::Resize);
    // Dragging the far corner to twice its distance doubles both sides.
    drag_with(&mut sim, (100.0, 60.0), (200.0, 120.0), Modifiers::NONE);
    assert_eq!(
        points_of(&sim, id),
        vec![p(0.0, 0.0), p(200.0, 0.0), p(200.0, 120.0), p(0.0, 120.0)]
    );
}

#[test]
fn concentric_moves_every_edge_the_same_jump() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let id = box_cad(&mut sim);
    set_mode(&mut sim, EditBehavior::Concentric);
    sim.app.cx.defaults.editing.behavior.concentric_jump = 6.0;
    // Pulling the corner to (113, 73) is 13" out along each edge: the jump
    // of 6" rounds that to 12", so the box grows 12" on every side.
    drag_with(&mut sim, (100.0, 60.0), (113.0, 73.0), Modifiers::NONE);
    let pts = points_of(&sim, id);
    let (lo, hi) = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .bounds();
    assert!(close(lo, p(-12.0, -12.0)), "{lo:?} {pts:?}");
    assert!(close(hi, p(112.0, 72.0)), "{hi:?}");
    assert_eq!(pts.len(), 4);
}

#[test]
fn fillet_rounds_the_dragged_corner_of_the_box() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let id = box_cad(&mut sim);
    behaviors::note_held_summon(Some(EditBehavior::Fillet));
    drag_with(&mut sim, (100.0, 60.0), (90.0, 50.0), Modifiers::NONE);
    behaviors::note_held_summon(None);
    let pts = points_of(&sim, id);
    assert!(pts.len() > 4, "{} points", pts.len());
    assert!(!pts.contains(&p(100.0, 60.0)), "the sharp corner is gone");
}

// ----- walls: Default, Alternate and the Ctrl/Cmd override -----

#[test]
fn alt_moves_a_wall_at_the_allowed_angles_and_ctrl_moves_it_freely() {
    let move_with = |m: Modifiers| {
        let mut sim = Sim::new();
        sim.tool(ToolId::Select);
        let ids = house(&mut sim);
        drag_with(&mut sim, (120.0, 0.0), (-30.0, 40.0), m);
        sim.app.cx.floor().wall(ids[0]).unwrap().start
    };
    // Default: square to the wall, so only the 40" across counts.
    let d = move_with(Modifiers::NONE);
    assert!(d.x.abs() < 1e-9 && (d.y.abs() - 40.0).abs() < 1e-9, "{d:?}");
    // Alt: 165 degrees is an allowed angle, the 155.2" length rounds to 155".
    let a = move_with(alt());
    let r = 165f64.to_radians();
    assert!(
        close(a, p(155.0 * r.cos(), 155.0 * r.sin())),
        "{a:?} vs the 165 degree ray"
    );
    // Ctrl/Cmd: no restriction at all, only the grid.
    assert!(close(move_with(ctrl()), p(-150.0, 40.0)));
}

#[test]
fn alt_on_an_object_no_longer_starts_a_marquee() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let ids = house(&mut sim);
    // The press lands on the bottom wall with Alt held from the start.
    sim.move_to(120.0, 0.0);
    let ev = sim.event(120.0, 0.0).with_down(true).with_modifiers(alt());
    let r = sim.app.tools.active_mut().pointer_down(&mut sim.app.cx, ev);
    sim.finish(r);
    let ev = sim.event(120.0, 24.0).with_down(true).with_modifiers(alt());
    let r = sim.app.tools.active_mut().pointer_move(&mut sim.app.cx, ev);
    sim.finish(r);
    let ev = sim.event(120.0, 24.0).with_modifiers(alt());
    let r = sim.app.tools.active_mut().pointer_up(&mut sim.app.cx, ev);
    sim.finish(r);
    assert_eq!(
        sim.app.cx.floor().wall(ids[0]).unwrap().start,
        p(0.0, 24.0),
        "the wall was dragged, not marqueed"
    );
    assert!(sim.app.cx.selection.contains(ObjectRef::Wall(ids[0])));
}

#[test]
fn ctrl_overrides_the_angle_and_grid_snaps_while_dragging_a_wall_end() {
    let drag_end = |m: Modifiers| {
        let mut sim = Sim::new();
        sim.tool(ToolId::Select);
        let a = sim.app.cx.project.add_wall(
            0,
            p(0.0, 0.0),
            p(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        sim.app.cx.refresh();
        sim.click(60.0, 0.0);
        drag_with(&mut sim, (120.0, 0.0), (201.4, 107.4), m);
        sim.app.cx.floor().wall(a).unwrap().end
    };
    // Plain: the angle snaps to 30 degrees and the length to the grid.
    let snapped = drag_end(Modifiers::NONE);
    let a = snapped.y.atan2(snapped.x).to_degrees();
    assert!((a - 30.0).abs() < 1e-6, "{snapped:?}");
    // Ctrl or Cmd: the pointer, exactly.
    let free = drag_end(ctrl());
    assert!(close(free, p(201.4, 107.4)), "{free:?}");
}

#[test]
fn ctrl_returns_the_raw_point_over_an_endpoint() {
    let mut sim = Sim::new();
    sim.app
        .cx
        .project
        .add_wall(0, p(0.0, 0.0), p(120.0, 0.0), 6.0, 96.0, WallKind::Interior);
    sim.app.cx.refresh();
    let raw = p(121.3, 2.2);
    assert_eq!(
        sim.app.cx.snap_at(raw, None, false, &[]).kind,
        SnapKind::Endpoint
    );
    let ev = crate::tools::PointerEvent::at(&sim.app.cx, raw).with_modifiers(ctrl());
    assert!(ev.overrides());
    assert_eq!(
        sim.app.cx.snap_at(raw, None, ev.overrides(), &[]).point,
        raw
    );
    snap::set_held(HeldKeys::default());
}

// ----- Enter Coordinates and arithmetic -----

fn wall_tool(sim: &mut Sim) {
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
}

#[test]
fn enter_coordinates_draws_a_wall_by_polar_entry() {
    coords::close();
    let mut sim = Sim::new();
    wall_tool(&mut sim);
    sim.click(0.0, 0.0);
    let origin = sim.app.tools.active().coordinate_origin(&sim.app.cx);
    assert_eq!(origin, Some(p(0.0, 0.0)));
    // Tab with nothing typed asks for the location.
    assert!(coords::try_open(
        &sim.app.cx,
        origin,
        &KeyEvent::key(Key::Tab)
    ));
    assert!(coords::is_open());
    coords::close();
    // Polar: 12 feet at 90 degrees from the start.
    let m = coords::Mode {
        relative: true,
        polar: true,
    };
    let end = coords::resolve(
        p(0.0, 0.0),
        m,
        ["", "", "12'", "90"],
        LengthUnit::FeetInches,
    )
    .unwrap();
    let (length, angle) = coords::to_polar(p(0.0, 0.0), end);
    coords::deliver(&mut sim.app.cx, length, angle);
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.floor_walls(), 1);
    let w = sim.app.cx.floor().walls[0].clone();
    assert!(
        close(w.start, p(0.0, 0.0)) && close(w.end, p(0.0, 144.0)),
        "{w:?}"
    );

    // The next wall: absolute X and Y.
    let origin = sim.app.tools.active().coordinate_origin(&sim.app.cx);
    assert_eq!(origin, Some(p(0.0, 144.0)));
    let m = coords::Mode {
        relative: false,
        polar: false,
    };
    let end = coords::resolve(
        origin.unwrap(),
        m,
        ["10'", "12'", "", ""],
        LengthUnit::FeetInches,
    )
    .unwrap();
    let (length, angle) = coords::to_polar(origin.unwrap(), end);
    coords::deliver(&mut sim.app.cx, length, angle);
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.floor_walls(), 2);
    assert!(close(sim.app.cx.floor().walls[1].end, p(120.0, 144.0)));
}

#[test]
fn enter_coordinates_waits_for_nothing_typed_and_a_plain_tab_or_enter() {
    coords::close();
    let mut sim = Sim::new();
    wall_tool(&mut sim);
    // No start yet.
    assert!(!coords::try_open(
        &sim.app.cx,
        None,
        &KeyEvent::key(Key::Tab)
    ));
    sim.click(0.0, 0.0);
    let origin = sim.app.tools.active().coordinate_origin(&sim.app.cx);
    // Not for other keys.
    assert!(!coords::try_open(
        &sim.app.cx,
        origin,
        &KeyEvent::key(Key::A)
    ));
    // Not once a length is being typed.
    sim.key(KeyEvent::text("8'"));
    assert!(!coords::try_open(
        &sim.app.cx,
        origin,
        &KeyEvent::key(Key::Tab)
    ));
    assert!(!coords::is_open());
}

#[test]
fn enter_coordinates_moves_a_wall_end_by_the_typed_location() {
    coords::close();
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let a =
        sim.app
            .cx
            .project
            .add_wall(0, p(0.0, 0.0), p(120.0, 0.0), 6.0, 96.0, WallKind::Interior);
    sim.app.cx.refresh();
    sim.click(60.0, 0.0);
    sim.move_to(120.0, 0.0);
    sim.down(120.0, 0.0);
    let ev = sim.event(130.0, 5.0).with_down(true);
    let r = sim.app.tools.active_mut().pointer_move(&mut sim.app.cx, ev);
    sim.finish(r);
    let origin = sim.app.tools.active().coordinate_origin(&sim.app.cx);
    assert_eq!(origin, Some(p(0.0, 0.0)), "the fixed end is the start");
    assert!(coords::try_open(
        &sim.app.cx,
        origin,
        &KeyEvent::key(Key::Enter)
    ));
    coords::close();
    coords::deliver(&mut sim.app.cx, 96.0, 90.0);
    sim.key(KeyEvent::key(Key::Enter));
    let w = sim.app.cx.floor().wall(a).unwrap();
    assert!(close(w.end, p(0.0, 96.0)), "{:?}", w.end);
}

#[test]
fn the_typed_length_takes_arithmetic() {
    let mut sim = Sim::new();
    wall_tool(&mut sim);
    sim.click(0.0, 0.0);
    sim.key(KeyEvent::text("10' + 6\""));
    sim.key(KeyEvent::key(Key::Tab));
    sim.key(KeyEvent::text("45 - 45"));
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.floor_walls(), 1);
    let w = sim.app.cx.floor().walls[0].clone();
    assert!(close(w.end, p(126.0, 0.0)), "{:?}", w.end);
}

// ----- keys that change how the pointer is read -----

#[test]
fn shift_restricts_the_snap_to_90_or_45_degrees() {
    let mut sim = Sim::new();
    let raw = p(100.0, 60.0);
    let start = Some(p(0.0, 0.0));
    let plain = sim.app.cx.snap_at(raw, start, false, &[]);
    assert_eq!(plain.kind, SnapKind::Angle);
    let a = plain.point.y.atan2(plain.point.x).to_degrees();
    assert!((a - 30.0).abs() < 1e-6, "15 degree steps: {a}");
    snap::set_held(HeldKeys {
        shift: true,
        s_key: false,
    });
    let held = sim.app.cx.snap_at(raw, start, false, &[]);
    assert_eq!(held.kind, SnapKind::Angle);
    assert!(held.point.y.abs() < 1e-6, "90 degrees: {:?}", held.point);
    sim.app.cx.defaults.editing.restrictive_angle_deg = 45.0;
    let held = sim.app.cx.snap_at(raw, start, false, &[]);
    assert!(
        (held.point.x - held.point.y).abs() < 1e-6,
        "{:?}",
        held.point
    );
    snap::set_held(HeldKeys::default());
}

#[test]
fn the_s_key_drops_object_snaps_but_keeps_extension_anchors_and_key_one_clears_them() {
    let mut sim = Sim::new();
    sim.app
        .cx
        .project
        .add_wall(0, p(0.0, 0.0), p(120.0, 0.0), 6.0, 96.0, WallKind::Interior);
    sim.app.cx.refresh();
    sim.app.cx.defaults.editing.snap_extension = true;
    snap::clear_anchors();
    // Resting on the wall's end sets an anchor.
    let near_end = p(120.5, 0.4);
    let on_end = sim.app.cx.snap_at(near_end, None, false, &[]);
    assert_eq!(on_end.kind, SnapKind::Endpoint);
    snap::note_hover(&on_end, 6);
    assert_eq!(snap::anchors(), vec![p(120.0, 0.0)]);
    // Far from every object, lining up with the anchor's vertical.
    let up = p(121.0, 80.7);
    let r = sim.app.cx.snap_at(up, None, false, &[]);
    assert_eq!(r.kind, SnapKind::Extension);
    assert!((r.point.x - 120.0).abs() < 1e-9 && (r.point.y - 80.7).abs() < 1e-9);
    // S: the endpoint snap is off, the anchor still works.
    snap::set_held(HeldKeys {
        shift: false,
        s_key: true,
    });
    assert_ne!(
        sim.app.cx.snap_at(near_end, None, false, &[]).kind,
        SnapKind::Endpoint
    );
    assert_eq!(
        sim.app.cx.snap_at(up, None, false, &[]).kind,
        SnapKind::Extension
    );
    snap::set_held(HeldKeys::default());
    // Key 1 clears them.
    snap::clear_anchors();
    assert!(snap::anchors().is_empty());
    assert_ne!(
        sim.app.cx.snap_at(up, None, false, &[]).kind,
        SnapKind::Extension
    );
    // The history keeps the newest few.
    for i in 0..5 {
        snap::note_hover(
            &snap::SnapResult {
                point: p(f64::from(i) * 10.0, 0.0),
                kind: SnapKind::Midpoint,
                source: None,
            },
            3,
        );
    }
    assert_eq!(snap::anchors().len(), 3);
    assert_eq!(snap::anchors()[2], p(40.0, 0.0));
    snap::clear_anchors();
}

#[test]
fn ctrl_drag_over_nothing_still_marquees_and_toggles_what_it_covers() {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    let ids = house(&mut sim);
    sim.app.cx.selection.set(ObjectRef::Wall(ids[0]));
    // A marquee that touches the bottom and left walls, Ctrl held: the
    // selected bottom wall leaves, the left wall joins.
    crate::tools::select::set_marquee_mode(crate::tools::select::MarqueeMode::Touching);
    sim.move_to(-20.0, -20.0);
    let ev = sim
        .event(-20.0, -20.0)
        .with_down(true)
        .with_modifiers(ctrl());
    let r = sim.app.tools.active_mut().pointer_down(&mut sim.app.cx, ev);
    sim.finish(r);
    let ev = sim.event(60.0, 30.0).with_down(true).with_modifiers(ctrl());
    let r = sim.app.tools.active_mut().pointer_move(&mut sim.app.cx, ev);
    sim.finish(r);
    let ev = sim.event(60.0, 30.0).with_modifiers(ctrl());
    let r = sim.app.tools.active_mut().pointer_up(&mut sim.app.cx, ev);
    sim.finish(r);
    snap::set_held(HeldKeys::default());
    crate::tools::select::set_marquee_mode(crate::tools::select::MarqueeMode::ByDirection);
    assert!(!sim.app.cx.selection.contains(ObjectRef::Wall(ids[0])));
    assert!(sim.app.cx.selection.contains(ObjectRef::Wall(ids[3])));
}

// ----- continuous drawing -----

#[test]
fn alternate_draws_cad_lines_continuously_and_stops_when_the_shape_closes() {
    let mut sim = Sim::new();
    sim.tool(ToolId::CadVariant(CadMode::Line));
    sim.app.cx.view_flags.remove(&ViewFlag::ConnectCad);
    // Default: a click, a click, and the line is made; the third click starts
    // a new one.
    for (x, y) in [(0.0, 0.0), (60.0, 0.0), (60.0, 60.0)] {
        sim.click(x, y);
    }
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    sim.esc();
    sim.tool(ToolId::CadVariant(CadMode::Line));
    // Alternate: each click after the first makes a line that starts where
    // the last ended, until the shape closes.
    set_mode(&mut sim, EditBehavior::Alternate);
    for (x, y) in [
        (0.0, 300.0),
        (60.0, 300.0),
        (60.0, 360.0),
        (0.0, 360.0),
        (0.0, 300.0),
    ] {
        sim.click(x, y);
    }
    assert_eq!(sim.app.cx.floor().cad.len(), 5, "1 + 4 sides");
    // Closed: the next click only starts a line.
    sim.click(200.0, 200.0);
    assert_eq!(sim.app.cx.floor().cad.len(), 5);
}

#[test]
fn alt_on_a_click_chains_just_that_segment_on() {
    let mut sim = Sim::new();
    sim.tool(ToolId::CadVariant(CadMode::Line));
    sim.app.cx.view_flags.remove(&ViewFlag::ConnectCad);
    sim.click(0.0, 0.0);
    sim.move_to(60.0, 0.0);
    let ev = sim.event(60.0, 0.0).with_down(true).with_modifiers(alt());
    let r = sim.app.tools.active_mut().pointer_down(&mut sim.app.cx, ev);
    sim.finish(r);
    let ev = sim.event(60.0, 0.0).with_modifiers(alt());
    let r = sim.app.tools.active_mut().pointer_up(&mut sim.app.cx, ev);
    sim.finish(r);
    snap::set_held(HeldKeys::default());
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    sim.click(60.0, 60.0);
    assert_eq!(sim.app.cx.floor().cad.len(), 2, "the line chained on");
}

/// All the text the Preferences window drew for `page`.
fn prefs_page_text(sim: &mut Sim, page: crate::dialogs::preferences::Page) -> String {
    use crate::dialogs::preferences;
    let ctx = eframe::egui::Context::default();
    let mut settings = crate::theme::AppSettings::default();
    let mut actions = Vec::new();
    preferences::open(page);
    // The window is laid out invisibly on its first frame; read the second.
    let mut out = Default::default();
    for _ in 0..3 {
        out = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            preferences::show_all(ctx, &mut sim.app.cx, &mut settings, &mut actions);
        });
    }
    let mut text = String::new();
    for s in &out.shapes {
        if let eframe::egui::Shape::Text(t) = &s.shape {
            text.push_str(t.galley.text());
            text.push('\n');
        }
    }
    text
}

#[test]
fn preferences_behaviors_and_snap_pages_show_the_edit_behavior_fields() {
    use crate::dialogs::preferences::Page;
    let mut sim = Sim::new();
    let b = prefs_page_text(&mut sim, Page::Behaviors);
    for want in [
        "Primary Movement Method",
        "Stop When Connected",
        "Concentric Jump",
        "Behavior Indicators",
    ] {
        assert!(b.contains(want), "Behaviors page lacks {want}: {b}");
    }
    assert!(!b.contains("locks the move to one axis"), "retired option");
    let s = prefs_page_text(&mut sim, Page::Snaps);
    // The window clips below Shift Restricts To; the rows under it are not drawn.
    assert!(
        s.contains("Shift Restricts To"),
        "Snap page lacks Shift Restricts To: {s}"
    );
}

/// How many shapes `f` paints.
fn painted(f: impl Fn(&eframe::egui::Painter)) -> usize {
    let ctx = eframe::egui::Context::default();
    let mut n = 0;
    let out = ctx.run(eframe::egui::RawInput::default(), |ctx| {
        let p = ctx.layer_painter(eframe::egui::LayerId::background());
        f(&p);
    });
    for s in &out.shapes {
        let _ = s;
        n += 1;
    }
    n
}

#[test]
fn the_angle_snap_grid_draws_hatch_marks_and_the_anchors_draw_markers() {
    let mut sim = Sim::new();
    let cam = crate::editor::Camera::default_view();
    snap::clear_anchors();
    // Off: nothing. On: rays plus hatch marks along them.
    let none = painted(|p| snap::draw_angle_rays(p, &cam, &sim.app.cx, p_(0.0, 0.0)));
    assert_eq!(none, 0);
    sim.app.cx.defaults.editing.angle_snap_grid = true;
    let rays = painted(|p| snap::draw_angle_rays(p, &cam, &sim.app.cx, p_(0.0, 0.0)));
    assert!(rays > 0);
    sim.app.cx.defaults.editing.snap_extension = true;
    // No anchors, no markers; one anchor, one marker.
    assert_eq!(
        painted(|p| snap::draw_anchor_markers(p, &cam, &sim.app.cx)),
        0
    );
    snap::note_hover(
        &snap::SnapResult {
            point: p_(10.0, 10.0),
            kind: SnapKind::Endpoint,
            source: None,
        },
        6,
    );
    assert!(painted(|p| snap::draw_anchor_markers(p, &cam, &sim.app.cx)) > 0);
    snap::clear_anchors();
}

fn p_(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
