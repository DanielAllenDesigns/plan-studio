//! Scenario 73: wall connection repair and wall edit tools (W-130, W-132,
//! W-133, W-135, W-136, W-145): the off-angle wall and its dialog, Connect
//! Walls over a gap, the Auto Connect lock, Ignore / Reset Notification
//! Icons, Make Wall(s) Invisible / Visible and Align With Wall Below. Every
//! action is one undo step.

use crate::dialogs::fix_connections::FixOffAngleDialog;
use crate::editor::{wall_edit, EditorContext, ObjectRef};
use crate::plan_defaults;
use plan_core::geometry::Point;
use plan_core::wall_repair::FixLock;
use plan_core::{Id, WallEnd, WallKind};

fn cx() -> EditorContext {
    EditorContext::new(plan_defaults::embedded())
}

fn add(cx: &mut EditorContext, floor: usize, a: (f64, f64), b: (f64, f64)) -> Id {
    cx.project.add_wall(
        floor,
        Point::new(a.0, a.1),
        Point::new(b.0, b.1),
        4.5,
        96.0,
        WallKind::Interior,
    )
}

fn select(cx: &mut EditorContext, ids: &[Id]) {
    cx.selection.clear();
    for id in ids {
        cx.selection.add(ObjectRef::Wall(*id));
    }
}

#[test]
fn an_off_angle_wall_is_fixed_through_the_dialog_in_one_undo_step() {
    let mut cx = cx();
    // 2 degrees off east, with a wall joined at its far end.
    let a = add(&mut cx, 0, (0.0, 0.0), (240.0, 8.4));
    let b = add(&mut cx, 0, (240.0, 8.4), (240.0, 200.0));
    assert!(wall_edit::off_angle_target(&cx, a).is_some());
    assert!(wall_edit::off_angle_target(&cx, b).is_none());
    select(&mut cx, &[a]);
    let mut d = FixOffAngleDialog::new(&cx, a).unwrap();
    assert!(d.old_angle > 1.9 && d.old_angle < 2.1, "{}", d.old_angle);
    assert!(d.new_angle.abs() < 1e-9);
    d.lock = FixLock::Start;
    assert!(d.apply(&mut cx));
    assert_eq!(cx.undo_label(), Some("Fix Off Angle Wall"));
    let wa = cx.floor().wall(a).unwrap();
    assert_eq!(wa.start, Point::new(0.0, 0.0));
    assert!(wa.end.y.abs() < 1e-6);
    // The joined wall's end followed.
    assert!(cx.floor().wall(b).unwrap().start.dist(wa.end) < 1e-6);
    assert!(wall_edit::off_angle_target(&cx, a).is_none());
    cx.undo();
    assert!((cx.floor().wall(a).unwrap().end.y - 8.4).abs() < 1e-9);
    assert!(wall_edit::off_angle_target(&cx, a).is_some());
}

#[test]
fn connect_walls_closes_a_gap_and_a_locked_end_stays_open() {
    let mut cx = cx();
    // A 12 inch gap, wider than the automatic connect distance.
    let a = add(&mut cx, 0, (0.0, 0.0), (120.0, 0.0));
    let b = add(&mut cx, 0, (132.0, 0.0), (132.0, 120.0));
    assert!(!cx.project.end_is_connected(0, a, WallEnd::End));
    // A locked end is refused and leaves no undo step.
    select(&mut cx, &[a]);
    assert_eq!(
        wall_edit::toggle_auto_connect_lock(&mut cx, WallEnd::End),
        Some(true)
    );
    assert_eq!(cx.undo_label(), Some("Auto Connect Lock"));
    assert!(!cx.project.unconnected_ends(0, a).contains(&WallEnd::End));
    select(&mut cx, &[a, b]);
    assert!(wall_edit::run_command(&mut cx, wall_edit::CONNECT_WALLS));
    assert_eq!(cx.undo_label(), Some("Auto Connect Lock"));
    // Unlocked, Connect Walls joins them at the corner in one step.
    select(&mut cx, &[a]);
    assert_eq!(
        wall_edit::toggle_auto_connect_lock(&mut cx, WallEnd::End),
        Some(false)
    );
    select(&mut cx, &[a, b]);
    assert!(wall_edit::run_command(&mut cx, wall_edit::CONNECT_WALLS));
    assert_eq!(cx.undo_label(), Some("Connect Walls"));
    let (wa, wb) = (cx.floor().wall(a).unwrap(), cx.floor().wall(b).unwrap());
    assert!(wa.end.dist(wb.start) < 1e-6, "{:?} {:?}", wa.end, wb.start);
    assert!(cx.project.end_is_connected(0, a, WallEnd::End));
    cx.undo();
    assert!(!cx.project.end_is_connected(0, a, WallEnd::End));
}

#[test]
fn connect_walls_with_one_wall_waits_for_the_second_click_and_esc_cancels() {
    let mut cx = cx();
    let a = add(&mut cx, 0, (0.0, 0.0), (120.0, 0.0));
    let _b = add(&mut cx, 0, (130.0, 0.0), (130.0, 120.0));
    select(&mut cx, &[a]);
    assert!(wall_edit::run_command(&mut cx, wall_edit::CONNECT_WALLS));
    assert!(wall_edit::break_pending());
    // A click off every wall keeps the mode; Esc ends it.
    assert!(!wall_edit::break_click(&mut cx, Point::new(500.0, 500.0)));
    assert!(wall_edit::break_pending());
    assert!(wall_edit::cancel_break());
    assert!(!wall_edit::break_pending());
}

#[test]
fn ignoring_icons_and_resetting_them() {
    let mut cx = cx();
    let a = add(&mut cx, 0, (0.0, 0.0), (240.0, 8.4));
    select(&mut cx, &[a]);
    assert_eq!(cx.project.unconnected_walls(0).len(), 2);
    assert_eq!(wall_edit::ignore_icons(&mut cx, false), 1);
    assert_eq!(cx.undo_label(), Some("Ignore"));
    assert!(wall_edit::off_angle_target(&cx, a).is_none());
    assert!(cx.project.unconnected_walls(0).is_empty());
    assert_eq!(wall_edit::reset_icons(&mut cx), 1);
    assert_eq!(cx.undo_label(), Some("Reset Notification Icons"));
    assert!(wall_edit::off_angle_target(&cx, a).is_some());
    assert_eq!(cx.project.unconnected_walls(0).len(), 2);
    // Nothing ignored: no step, no change.
    assert_eq!(wall_edit::reset_icons(&mut cx), 0);
    assert_eq!(cx.undo_label(), Some("Reset Notification Icons"));
}

#[test]
fn make_walls_invisible_and_visible_each_take_one_undo_step() {
    let mut cx = cx();
    let a = add(&mut cx, 0, (0.0, 0.0), (240.0, 0.0));
    let b = add(&mut cx, 0, (240.0, 0.0), (240.0, 240.0));
    select(&mut cx, &[a, b]);
    assert!(wall_edit::run_command(&mut cx, wall_edit::MAKE_INVISIBLE));
    assert_eq!(cx.undo_label(), Some("Make Walls Invisible"));
    assert!(cx.floor().walls.iter().all(|w| w.flags.invisible));
    // The Edit toolbar now offers the opposite button.
    let ids: Vec<_> = wall_edit::edit_actions(&cx)
        .iter()
        .map(|e| e.label)
        .collect();
    assert!(ids.contains(&"Make Wall(s) Visible") && !ids.contains(&"Make Wall(s) Invisible"));
    assert!(wall_edit::run_command(&mut cx, wall_edit::MAKE_VISIBLE));
    assert!(cx.floor().walls.iter().all(|w| !w.flags.invisible));
    cx.undo();
    assert!(cx.floor().walls.iter().all(|w| w.flags.invisible));
}

#[test]
fn a_wall_aligns_with_the_wall_below_and_its_joined_wall_follows() {
    let mut cx = cx();
    cx.project
        .floors
        .push(plan_core::model::Floor::new("Second", 108.0));
    let low = add(&mut cx, 0, (0.0, 0.0), (240.0, 0.0));
    cx.project.floors[0].wall_mut(low).unwrap().thickness = 6.0;
    let up = add(&mut cx, 1, (0.0, 4.0), (240.0, 4.0));
    let side = add(&mut cx, 1, (240.0, 4.0), (240.0, 200.0));
    cx.floor = 1;
    select(&mut cx, &[up]);
    assert_eq!(wall_edit::align_with(&mut cx, -1), 1);
    assert_eq!(cx.undo_label(), Some("Align With Wall Below"));
    let w = cx.floor().wall(up).unwrap();
    assert!(
        (w.start.y - 5.25).abs() < 1e-9 && (w.end.y - 5.25).abs() < 1e-9,
        "{:?}",
        w.start
    );
    // The wall joined at the end follows; the lined-up edge is the lower wall's.
    assert!(cx.floor().wall(side).unwrap().start.dist(w.end) < 1e-9);
    // Already aligned: nothing more to do and no extra undo step.
    assert_eq!(wall_edit::align_with(&mut cx, -1), 0);
    assert_eq!(cx.undo_label(), Some("Align With Wall Below"));
    // No wall above: refused.
    assert_eq!(wall_edit::align_with(&mut cx, 1), 0);
    cx.undo();
    assert!((cx.floor().wall(up).unwrap().start.y - 4.0).abs() < 1e-9);
}
