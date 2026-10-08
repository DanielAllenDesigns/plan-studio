//! Tests of the Edit commands: clipboard, cursor-attached paste, select all,
//! groups, Transform/Replicate, reflect, align, the click-driven modes and
//! the right-click menu, all through the real tools and `run_custom`.

use super::edit_commands::{ids, DUPLICATE_OFFSET};
use super::transform::{self, ReflectAxis, TransformParams};
use super::{EditAction, EditorContext, EditorRequest, ObjectRef};
use crate::plan_defaults;
use crate::toolbar::Action;
use crate::tools::select::SelectTool;
use crate::tools::{KeyEvent, PointerEvent, Tool, ToolId};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::transform::{AlignMode, Axis};
use plan_core::{Id, OpeningKind, WallKind};

fn cx() -> EditorContext {
    EditorContext::new(plan_defaults::embedded())
}

fn wall(cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) -> Id {
    cx.project.add_wall(
        0,
        Point::new(a.0, a.1),
        Point::new(b.0, b.1),
        6.0,
        96.0,
        WallKind::Interior,
    )
}

fn line(cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) -> Id {
    cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(a.0, a.1),
            b: Point::new(b.0, b.1),
        },
    )
}

fn at(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
    PointerEvent::at(cx, Point::new(x, y))
}

/// A click (press and release) at the point through the tool.
fn click(t: &mut SelectTool, cx: &mut EditorContext, x: f64, y: f64) {
    t.pointer_down(cx, at(cx, x, y));
    t.pointer_up(cx, at(cx, x, y));
}

fn wall_count(cx: &EditorContext) -> usize {
    cx.floor().walls.len()
}

/// A wall with a door, selected, and its twin in the clipboard.
fn wall_with_door() -> (EditorContext, Id) {
    let mut cx = cx();
    let w = wall(&mut cx, (0.0, 0.0), (240.0, 0.0));
    cx.project
        .add_opening(0, w, 120.0, OpeningKind::Door)
        .unwrap();
    cx.selection.set(ObjectRef::Wall(w));
    (cx, w)
}

#[test]
fn copy_and_paste_hold_position_duplicate_walls_with_their_openings() {
    let (mut cx, w) = wall_with_door();
    cx.run_custom(ids::COPY);
    assert!(cx.clipboard.is_some());
    cx.run_custom(ids::PASTE_HOLD);
    assert_eq!(wall_count(&cx), 2);
    assert_eq!(cx.floor().openings.len(), 2);
    let copy = cx.selection.single().unwrap();
    assert_ne!(
        copy,
        ObjectRef::Wall(w),
        "the copy is selected, with a new id"
    );
    assert_eq!(cx.undo_label(), Some("Paste Hold Position"));
    cx.undo();
    assert_eq!(wall_count(&cx), 1);
    assert_eq!(cx.floor().openings.len(), 1);
}

#[test]
fn cut_is_one_undo_step_and_paste_hold_brings_it_back() {
    let (mut cx, _) = wall_with_door();
    cx.run_custom(ids::CUT);
    assert_eq!(wall_count(&cx), 0);
    assert!(cx.floor().openings.is_empty());
    assert_eq!(cx.undo_label(), Some("Cut"));
    cx.run_custom(ids::PASTE_HOLD);
    assert_eq!(wall_count(&cx), 1);
    assert_eq!(cx.floor().openings.len(), 1);
    cx.undo();
    cx.undo();
    // One step undid the paste, one the cut: the original wall is back.
    assert_eq!(wall_count(&cx), 1);
    assert_eq!(cx.floor().openings.len(), 1);
}

#[test]
fn copy_paste_in_place_chord_command_duplicates_at_the_same_place() {
    let (mut cx, w) = wall_with_door();
    cx.run_custom(ids::COPY_PASTE_IN_PLACE);
    assert_eq!(wall_count(&cx), 2);
    let a = cx.floor().wall(w).unwrap().clone();
    let b = cx.floor().walls.iter().find(|x| x.id != w).unwrap().clone();
    assert_eq!((a.start, a.end), (b.start, b.end));
}

#[test]
fn paste_attaches_to_the_pointer_and_a_click_drops_it() {
    let (mut cx, w) = wall_with_door();
    cx.run_custom(ids::COPY);
    let mut tool = SelectTool::default();
    cx.run_custom(ids::PASTE);
    assert!(transform::mode_active(), "paste hangs on the pointer");
    assert!(cx
        .requests
        .contains(&EditorRequest::SetTool(ToolId::Select)));
    assert_eq!(wall_count(&cx), 1, "nothing is pasted before the click");
    // The pointer carries the ghost; a click at (500, 300) drops the center there.
    let ev = at(&cx, 400.0, 250.0);
    tool.pointer_move(&mut cx, ev);
    click(&mut tool, &mut cx, 500.0, 300.0);
    assert!(!transform::mode_active());
    assert_eq!(wall_count(&cx), 2);
    assert_eq!(cx.floor().openings.len(), 2);
    let copy = cx.selection.single().unwrap();
    let ObjectRef::Wall(id) = copy else {
        panic!("the pasted wall is selected")
    };
    let pasted = cx.floor().wall(id).unwrap();
    let mid = Point::lerp(pasted.start, pasted.end, 0.5);
    assert!(
        mid.dist(Point::new(500.0, 300.0)) < 1.5,
        "centered on the click: {mid:?}"
    );
    assert_eq!(cx.undo_label(), Some("Paste"));
    cx.undo();
    assert_eq!(wall_count(&cx), 1);
    assert!(cx.floor().wall(w).is_some());
}

#[test]
fn escape_cancels_a_hanging_paste() {
    let (mut cx, _) = wall_with_door();
    cx.run_custom(ids::COPY);
    let mut tool = SelectTool::default();
    cx.run_custom(ids::PASTE);
    assert!(transform::mode_active());
    let res = tool.key(&mut cx, KeyEvent::escape());
    assert!(res.consumed);
    assert!(!transform::mode_active());
    assert_eq!(wall_count(&cx), 1);
    // A click after Esc selects as usual, it does not paste.
    click(&mut tool, &mut cx, 500.0, 300.0);
    assert_eq!(wall_count(&cx), 1);
}

#[test]
fn switching_tools_drops_a_hanging_paste() {
    let (mut cx, _) = wall_with_door();
    cx.run_custom(ids::COPY);
    let mut tool = SelectTool::default();
    cx.run_custom(ids::PASTE);
    tool.deactivate(&mut cx);
    assert!(!transform::mode_active());
}

#[test]
fn pasted_walls_join_the_walls_they_land_on() {
    let mut cx = cx();
    // A wall to join and a loose wall to copy.
    let base = wall(&mut cx, (0.0, 0.0), (200.0, 0.0));
    let loose = wall(&mut cx, (0.0, 500.0), (0.0, 600.0));
    cx.selection.set(ObjectRef::Wall(loose));
    cx.run_custom(ids::COPY);
    let mut tool = SelectTool::default();
    cx.run_custom(ids::PASTE);
    // Center (0, 550) lands so the copy runs (200,-50)..(200,50): its middle
    // touches nothing, but its end at y = 0 is not on the base either; drop
    // it so that its start meets the base's end instead.
    click(&mut tool, &mut cx, 200.0, 50.0);
    assert_eq!(wall_count(&cx), 3);
    let copy = cx.selection.single().unwrap();
    let ObjectRef::Wall(copy) = copy else {
        panic!("wall")
    };
    let c = cx.floor().wall(copy).unwrap();
    let b = cx.floor().wall(base).unwrap();
    assert!(
        c.start.dist(b.end) < 1.0 || c.end.dist(b.end) < 1.0,
        "the copy meets the end of the base wall: {:?} {:?} vs {:?}",
        c.start,
        c.end,
        b.end
    );
}

#[test]
fn paste_special_as_group_makes_one_group() {
    let mut cx = cx();
    let a = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    let b = wall(&mut cx, (100.0, 0.0), (100.0, 100.0));
    cx.selection.items = vec![ObjectRef::Wall(a), ObjectRef::Wall(b)];
    cx.run_custom(ids::COPY);
    let mut tool = SelectTool::default();
    cx.run_custom(ids::PASTE_GROUP);
    click(&mut tool, &mut cx, 600.0, 400.0);
    assert_eq!(wall_count(&cx), 4);
    assert_eq!(cx.floor().groups.len(), 1);
    assert_eq!(cx.floor().groups[0].members.len(), 2);
}

#[test]
fn duplicate_offsets_by_twelve_inches_and_leaves_the_clipboard() {
    let mut cx = cx();
    let l = line(&mut cx, (0.0, 0.0), (100.0, 0.0));
    cx.selection.set(ObjectRef::Cad(l));
    cx.run_custom(ids::DUPLICATE);
    assert_eq!(cx.floor().cad.len(), 2);
    assert!(cx.clipboard.is_none());
    let ObjectRef::Cad(new) = cx.selection.single().unwrap() else {
        panic!("cad")
    };
    let CadItem::Line { a, b } = cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == new)
        .unwrap()
        .item
        .clone()
    else {
        panic!("line")
    };
    assert_eq!(a, Point::ZERO + DUPLICATE_OFFSET);
    assert_eq!(b, Point::new(100.0, 0.0) + DUPLICATE_OFFSET);
    assert_eq!(cx.undo_label(), Some("Duplicate"));
}

#[test]
fn select_all_takes_the_floor_but_not_hidden_or_locked_layers() {
    let mut cx = cx();
    wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    wall(&mut cx, (0.0, 0.0), (0.0, 100.0));
    let l = line(&mut cx, (0.0, 0.0), (50.0, 50.0));
    let _ = l;
    cx.refresh();
    cx.run_custom(ids::SELECT_ALL);
    assert_eq!(cx.selection.len(), 3);
    // Lock the CAD layer, hide the wall layer.
    cx.project.layers.set_locked("CAD, Default", true);
    cx.run_custom(ids::SELECT_ALL);
    assert_eq!(cx.selection.len(), 2, "the locked CAD line stays out");
    let wall_layer = cx.floor().walls[0].layer.clone();
    cx.project.layers.set_display(&wall_layer, false);
    cx.run_custom(ids::SELECT_ALL);
    assert!(cx.selection.is_empty(), "walls hidden, CAD locked");
}

#[test]
fn select_same_type_picks_every_object_of_the_kind() {
    let mut cx = cx();
    let w1 = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    wall(&mut cx, (0.0, 50.0), (100.0, 50.0));
    line(&mut cx, (0.0, 0.0), (50.0, 50.0));
    cx.selection.set(ObjectRef::Wall(w1));
    cx.run_custom(ids::SELECT_SAME);
    assert_eq!(cx.selection.len(), 2);
    assert!(cx
        .selection
        .items
        .iter()
        .all(|o| matches!(o, ObjectRef::Wall(_))));
}

#[test]
fn group_ungroup_and_clicking_a_member_selects_the_group() {
    let mut cx = cx();
    let a = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    let b = wall(&mut cx, (0.0, 200.0), (100.0, 200.0));
    cx.selection.items = vec![ObjectRef::Wall(a), ObjectRef::Wall(b)];
    cx.run_custom(ids::GROUP);
    assert_eq!(cx.floor().groups.len(), 1);
    assert_eq!(cx.undo_label(), Some("Group"));
    cx.selection.clear();
    let mut tool = SelectTool::default();
    cx.px_per_in = 2.0;
    click(&mut tool, &mut cx, 50.0, 0.0);
    assert_eq!(cx.selection.len(), 2, "one click picks the whole group");
    // Ungroup: the click then picks one wall again.
    cx.run_custom(ids::UNGROUP);
    assert!(cx.floor().groups.is_empty());
    cx.selection.clear();
    click(&mut tool, &mut cx, 50.0, 0.0);
    assert_eq!(cx.selection.len(), 1);
}

#[test]
fn deleting_a_member_dissolves_a_two_member_group() {
    let mut cx = cx();
    let a = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    let b = wall(&mut cx, (0.0, 200.0), (100.0, 200.0));
    cx.selection.items = vec![ObjectRef::Wall(a), ObjectRef::Wall(b)];
    cx.group_selection();
    cx.selection.set(ObjectRef::Wall(a));
    cx.delete_selection();
    assert!(cx.floor().groups.is_empty());
}

#[test]
fn lock_unlock_and_send_to_layer() {
    let mut cx = cx();
    let w = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    cx.selection.set(ObjectRef::Wall(w));
    cx.run_custom(ids::LOCK);
    let layer = cx.floor().wall(w).unwrap().layer.clone();
    assert!(cx.project.layers.is_locked(&layer));
    assert!(
        cx.selection.is_empty(),
        "locked objects leave the selection"
    );
    cx.selection.set(ObjectRef::Wall(w));
    cx.run_custom(ids::UNLOCK);
    assert!(!cx.project.layers.is_locked(&layer));
    // Send to Layer moves the wall.
    cx.selection.set(ObjectRef::Wall(w));
    assert_eq!(cx.send_selection_to_layer("CAD, Default"), 1);
    assert_eq!(cx.floor().wall(w).unwrap().layer, "CAD, Default");
    cx.undo();
    assert_eq!(cx.floor().wall(w).unwrap().layer, layer);
    assert_eq!(cx.send_selection_to_layer("No Such Layer"), 0);
}

fn near(a: Point, b: Point) -> bool {
    a.dist(b) < 1e-6
}

#[test]
fn transform_replicate_makes_n_copies_at_the_right_offsets() {
    let mut cx = cx();
    let w = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    cx.selection.set(ObjectRef::Wall(w));
    let p = TransformParams {
        copies: 3,
        move_x: 0.0,
        move_y: 40.0,
        ..TransformParams::default()
    };
    transform::transform_replicate(&mut cx, &p).unwrap();
    assert_eq!(wall_count(&cx), 4);
    let mut ys: Vec<f64> = cx.floor().walls.iter().map(|w| w.start.y).collect();
    ys.sort_by(f64::total_cmp);
    assert_eq!(
        ys,
        vec![0.0, 40.0, 80.0, 120.0],
        "copy k sits at k times the delta"
    );
    assert_eq!(cx.selection.len(), 3, "the copies are selected");
    assert_eq!(cx.undo_label(), Some("Transform/Replicate"));
    cx.undo();
    assert_eq!(wall_count(&cx), 1);
}

#[test]
fn transform_replicate_rotation_with_copies_is_a_radial_array() {
    let mut cx = cx();
    let w = wall(&mut cx, (100.0, 0.0), (200.0, 0.0));
    cx.selection.set(ObjectRef::Wall(w));
    let p = TransformParams {
        copies: 3,
        rotate_deg: 90.0,
        rotate_about: Some(Point::ZERO),
        ..TransformParams::default()
    };
    transform::transform_replicate(&mut cx, &p).unwrap();
    assert_eq!(wall_count(&cx), 4);
    let ends: Vec<(Point, Point)> = cx.floor().walls.iter().map(|w| (w.start, w.end)).collect();
    let has = |a: Point, b: Point| ends.iter().any(|(s, e)| near(*s, a) && near(*e, b));
    assert!(has(Point::new(100.0, 0.0), Point::new(200.0, 0.0)));
    assert!(has(Point::new(0.0, 100.0), Point::new(0.0, 200.0)));
    assert!(has(Point::new(-100.0, 0.0), Point::new(-200.0, 0.0)));
    assert!(has(Point::new(0.0, -100.0), Point::new(0.0, -200.0)));
}

#[test]
fn transform_without_copies_moves_resizes_and_undoes() {
    let mut cx = cx();
    let l = line(&mut cx, (0.0, 0.0), (100.0, 0.0));
    cx.selection.set(ObjectRef::Cad(l));
    let p = TransformParams {
        move_x: 10.0,
        resize: 2.0,
        ..TransformParams::default()
    };
    transform::transform_replicate(&mut cx, &p).unwrap();
    let CadItem::Line { a, b } = cx.floor().cad[0].item.clone() else {
        panic!("line")
    };
    // Resized about the center (50, 0): -50..150, then 10 over.
    assert!(
        near(a, Point::new(-40.0, 0.0)) && near(b, Point::new(160.0, 0.0)),
        "{a:?} {b:?}"
    );
    cx.undo();
    let CadItem::Line { b, .. } = cx.floor().cad[0].item.clone() else {
        panic!("line")
    };
    assert!(near(b, Point::new(100.0, 0.0)));
}

#[test]
fn transform_refuses_locked_layers_and_an_empty_selection() {
    let mut cx = cx();
    let l = line(&mut cx, (0.0, 0.0), (100.0, 0.0));
    let p = TransformParams {
        move_x: 10.0,
        ..TransformParams::default()
    };
    assert!(transform::transform_replicate(&mut cx, &p).is_err());
    cx.selection.set(ObjectRef::Cad(l));
    cx.project.layers.set_locked("CAD, Default", true);
    assert!(transform::transform_replicate(&mut cx, &p).is_err());
    assert!(!cx.can_undo(), "a refused transform leaves no undo step");
}

#[test]
fn reflect_about_a_wall_mirrors_a_wall_its_door_and_swings() {
    let mut cx = cx();
    // The mirror wall is along y = 100; the selected wall sits below it.
    let axis = wall(&mut cx, (0.0, 100.0), (300.0, 100.0));
    let w = wall(&mut cx, (50.0, 0.0), (50.0, 60.0));
    let door = cx
        .project
        .add_opening(0, w, 30.0, OpeningKind::Door)
        .unwrap();
    let swing_before = cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == door)
        .unwrap()
        .swing_flipped;
    cx.selection.set(ObjectRef::Wall(w));
    cx.run_custom(ids::REFLECT);
    assert!(transform::mode_active());
    let mut tool = SelectTool::default();
    click(&mut tool, &mut cx, 250.0, 100.0);
    assert!(!transform::mode_active());
    let m = cx.floor().wall(w).unwrap();
    assert!(
        near(m.start, Point::new(50.0, 200.0)) && near(m.end, Point::new(50.0, 140.0)),
        "{m:?}"
    );
    let d = cx.floor().openings.iter().find(|o| o.id == door).unwrap();
    assert_ne!(
        d.swing_flipped, swing_before,
        "a mirrored door flips its swing"
    );
    assert!((d.center_offset - 30.0).abs() < 1e-9);
    assert!(cx.floor().wall(axis).is_some());
    assert_eq!(cx.undo_label(), Some("Reflect"));
    cx.undo();
    assert!(near(
        cx.floor().wall(w).unwrap().start,
        Point::new(50.0, 0.0)
    ));
}

#[test]
fn reflect_copy_leaves_the_original_for_symbols() {
    use plan_core::PlacedSymbol;
    let mut cx = cx();
    let axis = line(&mut cx, (0.0, 0.0), (0.0, 100.0));
    let _ = axis;
    let id = cx.project.add_symbol(
        0,
        PlacedSymbol::new("x", Point::new(50.0, 20.0), 10.0, 10.0, 10.0),
    );
    cx.selection.set(ObjectRef::Symbol(id));
    cx.run_custom(ids::REFLECT_COPY);
    let mut tool = SelectTool::default();
    click(&mut tool, &mut cx, 0.0, 50.0);
    assert_eq!(cx.floor().symbols.len(), 2);
    let positions: Vec<Point> = cx.floor().symbols.iter().map(|s| s.position).collect();
    assert!(positions.iter().any(|p| near(*p, Point::new(50.0, 20.0))));
    assert!(positions.iter().any(|p| near(*p, Point::new(-50.0, 20.0))));
}

#[test]
fn point_to_point_move_moves_by_the_two_clicks() {
    let mut cx = cx();
    let l = line(&mut cx, (0.0, 0.0), (100.0, 0.0));
    cx.selection.set(ObjectRef::Cad(l));
    cx.run_custom(ids::POINT_TO_POINT);
    let mut tool = SelectTool::default();
    click(&mut tool, &mut cx, 10.0, 10.0);
    assert!(transform::mode_active(), "waiting for the second point");
    click(&mut tool, &mut cx, 40.0, 70.0);
    assert!(!transform::mode_active());
    let CadItem::Line { a, b } = cx.floor().cad[0].item.clone() else {
        panic!("line")
    };
    assert!(near(a, Point::new(30.0, 60.0)) && near(b, Point::new(130.0, 60.0)));
    assert_eq!(cx.undo_label(), Some("Point to Point Move"));
}

#[test]
fn center_object_puts_a_door_mid_wall_and_an_object_between_two_walls() {
    let mut cx = cx();
    let w = wall(&mut cx, (0.0, 0.0), (240.0, 0.0));
    let door = cx
        .project
        .add_opening(0, w, 60.0, OpeningKind::Door)
        .unwrap();
    cx.selection.set(ObjectRef::Opening(door));
    cx.run_custom(ids::CENTER);
    let o = cx.floor().openings.iter().find(|o| o.id == door).unwrap();
    assert!((o.center_offset - 120.0).abs() < 1e-6);

    // A CAD line centered between two parallel walls.
    let w1 = wall(&mut cx, (0.0, 200.0), (300.0, 200.0));
    let w2 = wall(&mut cx, (0.0, 400.0), (300.0, 400.0));
    let l = line(&mut cx, (100.0, 230.0), (200.0, 230.0));
    cx.selection.set(ObjectRef::Cad(l));
    cx.run_custom(ids::CENTER);
    let mut tool = SelectTool::default();
    click(&mut tool, &mut cx, 150.0, 200.0);
    click(&mut tool, &mut cx, 150.0, 400.0);
    assert!(!transform::mode_active());
    let CadItem::Line { a, b } = cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == l)
        .unwrap()
        .item
        .clone()
    else {
        panic!("line")
    };
    assert!(
        near(a, Point::new(100.0, 300.0)) && near(b, Point::new(200.0, 300.0)),
        "{a:?} {b:?}"
    );
    let _ = (w1, w2);
}

#[test]
fn center_object_in_a_room_by_clicking_the_room() {
    let mut cx = cx();
    for (a, b) in [
        ((0.0, 0.0), (400.0, 0.0)),
        ((400.0, 0.0), (400.0, 300.0)),
        ((400.0, 300.0), (0.0, 300.0)),
        ((0.0, 300.0), (0.0, 0.0)),
    ] {
        wall(&mut cx, a, b);
    }
    cx.refresh();
    assert!(!cx.rooms.is_empty());
    let l = line(&mut cx, (20.0, 20.0), (60.0, 20.0));
    cx.selection.set(ObjectRef::Cad(l));
    cx.run_custom(ids::CENTER);
    let mut tool = SelectTool::default();
    click(&mut tool, &mut cx, 300.0, 200.0);
    let CadItem::Line { a, b } = cx.floor().cad[0].item.clone() else {
        panic!("line")
    };
    let mid = Point::lerp(a, b, 0.5);
    // The room is the inside of the wall faces; its middle is (200, 150).
    assert!(mid.dist(Point::new(200.0, 150.0)) < 1.0, "{mid:?}");
}

#[test]
fn make_parallel_and_perpendicular_turn_walls_about_their_start() {
    let mut cx = cx();
    let reference = wall(&mut cx, (0.0, 500.0), (400.0, 500.0));
    let w = wall(&mut cx, (0.0, 0.0), (100.0, 80.0));
    let len = Point::new(100.0, 80.0).length();
    cx.selection.set(ObjectRef::Wall(w));
    cx.run_custom(ids::PARALLEL);
    let mut tool = SelectTool::default();
    click(&mut tool, &mut cx, 200.0, 500.0);
    let m = cx.floor().wall(w).unwrap();
    assert!(near(m.start, Point::ZERO));
    assert!(near(m.end, Point::new(len, 0.0)), "{:?}", m.end);
    cx.selection.set(ObjectRef::Wall(w));
    cx.run_custom(ids::PERPENDICULAR);
    click(&mut tool, &mut cx, 200.0, 500.0);
    let m = cx.floor().wall(w).unwrap();
    assert!(
        m.end.x.abs() < 1e-6 && (m.end.y.abs() - len).abs() < 1e-6,
        "{:?}",
        m.end
    );
    let _ = reference;
}

#[test]
fn align_and_distribute_position_the_objects() {
    let mut cx = cx();
    let a = line(&mut cx, (0.0, 0.0), (10.0, 10.0));
    let b = line(&mut cx, (30.0, 5.0), (50.0, 15.0));
    let c = line(&mut cx, (90.0, 0.0), (100.0, 10.0));
    cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(b), ObjectRef::Cad(c)];
    assert_eq!(transform::align_selection(&mut cx, AlignMode::Left), Ok(2));
    let first =
        |cx: &EditorContext, id: Id| match cx.floor().cad.iter().find(|x| x.id == id).unwrap().item
        {
            CadItem::Line { a, .. } => a,
            _ => panic!("line"),
        };
    assert_eq!(first(&cx, b).x, 0.0);
    assert_eq!(first(&cx, c).x, 0.0);
    assert_eq!(cx.undo_label(), Some("Align Left"));
    cx.undo();
    // Distribute: gaps between 0..10, 30..50, 90..100 become equal (span 100,
    // widths 40, gap 30): the middle box moves from 30 to 40.
    assert_eq!(
        transform::distribute_selection(&mut cx, Axis::Horizontal, None),
        Ok(1)
    );
    assert_eq!(first(&cx, b).x, 40.0);
    assert_eq!(first(&cx, a).x, 0.0);
    assert_eq!(first(&cx, c).x, 90.0);
    // Fewer than two objects: a message, no step.
    cx.selection.set(ObjectRef::Cad(a));
    assert!(transform::align_selection(&mut cx, AlignMode::Top).is_err());
}

#[test]
fn group_rotate_handle_turns_the_selection_with_the_tool() {
    let mut cx = cx();
    let w1 = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    let w2 = wall(&mut cx, (0.0, 100.0), (100.0, 100.0));
    cx.selection.items = vec![ObjectRef::Wall(w1), ObjectRef::Wall(w2)];
    cx.px_per_in = 2.0;
    let (center, handle) =
        transform::group_rotate_handle(&cx).expect("two objects show the handle");
    assert!(near(center, Point::new(50.0, 50.0)));
    assert!(handle.y > 100.0);
    let mut tool = SelectTool::default();
    // Press the handle, drag a quarter turn to the left of the center, release.
    let ev = at(&cx, handle.x, handle.y);
    tool.pointer_down(&mut cx, ev);
    let to = Point::new(center.x - 100.0, center.y);
    let ev = at(&cx, to.x, to.y).with_down(true);
    tool.pointer_move(&mut cx, ev);
    let ev = at(&cx, to.x, to.y);
    tool.pointer_up(&mut cx, ev);
    let m = cx.floor().wall(w1).unwrap();
    // A quarter turn counter-clockwise about (50, 50): (0,0)->(100,0) becomes (100,0)->(100,100).
    assert!(
        near(m.start, Point::new(100.0, 0.0)) && near(m.end, Point::new(100.0, 100.0)),
        "{m:?}"
    );
    assert_eq!(cx.undo_label(), Some("Rotate Objects"));
    cx.undo();
    assert!(near(cx.floor().wall(w1).unwrap().start, Point::ZERO));
}

#[test]
fn move_to_front_reorders_cad() {
    let mut cx = cx();
    let a = line(&mut cx, (0.0, 0.0), (1.0, 0.0));
    let b = line(&mut cx, (0.0, 1.0), (1.0, 1.0));
    cx.selection.set(ObjectRef::Cad(a));
    cx.run_custom(ids::FRONT);
    assert_eq!(cx.floor().cad.last().unwrap().id, a);
    cx.run_custom(ids::BACK);
    assert_eq!(cx.floor().cad.first().unwrap().id, a);
    let _ = b;
}

#[test]
fn action_history_lists_steps_and_jumps() {
    let mut cx = cx();
    let l = line(&mut cx, (0.0, 0.0), (10.0, 0.0));
    cx.selection.set(ObjectRef::Cad(l));
    cx.begin_change("One");
    cx.mark_dirty();
    cx.begin_change("Two");
    cx.project.name = "second".into();
    cx.begin_change("Three");
    cx.project.name = "third".into();
    let (past, future) = cx.action_history();
    assert_eq!(past, vec!["One", "Two", "Three"]);
    assert!(future.is_empty());
    // Jump back to the state after step "One": undoes Two and Three.
    assert_eq!(cx.jump_back_to(0), 2);
    let (past, future) = cx.action_history();
    assert_eq!(past, vec!["One"]);
    assert_eq!(future, vec!["Two", "Three"]);
    // And forward again by two.
    assert_eq!(cx.jump_forward(2), 2);
    assert_eq!(cx.action_history().0.len(), 3);
}

#[test]
fn delete_objects_by_type_is_one_undo_step_and_honors_all_floors() {
    use crate::dialogs::delete_objects::{counts, delete_by_category, Category};
    let mut cx = cx();
    let w = wall(&mut cx, (0.0, 0.0), (240.0, 0.0));
    cx.project
        .add_opening(0, w, 120.0, OpeningKind::Window)
        .unwrap();
    line(&mut cx, (0.0, 0.0), (5.0, 5.0));
    cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Text {
            pos: Point::ZERO,
            text: "n".into(),
            height: 3.0,
            angle: 0.0,
        },
    );
    cx.project.build_new_floor(false);
    cx.project.add_wall(
        1,
        Point::ZERO,
        Point::new(10.0, 0.0),
        6.0,
        96.0,
        WallKind::Interior,
    );
    let n = counts(&cx);
    assert_eq!(
        n.iter().find(|(c, _)| *c == Category::Windows).unwrap().1,
        1
    );
    assert_eq!(n.iter().find(|(c, _)| *c == Category::Text).unwrap().1, 1);
    // Windows and text on this floor only.
    let gone = delete_by_category(&mut cx, &[Category::Windows, Category::Text], false);
    assert_eq!(gone, 2);
    assert!(cx.floor().openings.is_empty());
    assert_eq!(cx.floor().cad.len(), 1);
    assert_eq!(cx.undo_label(), Some("Delete Objects"));
    cx.undo();
    assert_eq!(cx.floor().openings.len(), 1);
    assert_eq!(cx.floor().cad.len(), 2);
    // Walls on every floor in one step.
    let gone = delete_by_category(&mut cx, &[Category::Walls], true);
    assert_eq!(gone, 2);
    assert!(cx.project.floors.iter().all(|f| f.walls.is_empty()));
    assert!(
        cx.floor().openings.is_empty(),
        "a wall takes its openings along"
    );
    cx.undo();
    assert_eq!(cx.project.floors[0].walls.len(), 1);
    assert_eq!(cx.project.floors[1].walls.len(), 1);
    // Locked layers stay.
    cx.project.layers.set_locked("CAD, Default", true);
    let gone = delete_by_category(&mut cx, &[Category::CadLines], false);
    assert_eq!(gone, 0);
}

// ----- the right-click menu -----

fn labels(entries: &[super::edit_commands::ContextEntry]) -> Vec<String> {
    entries.iter().map(|e| e.label.clone()).collect()
}

fn entries_for(cx: &EditorContext, ghost: bool) -> Vec<super::edit_commands::ContextEntry> {
    let tool = SelectTool::default();
    let bar: Vec<EditAction> = tool.edit_toolbar(cx);
    cx.context_entries(&bar, ghost)
}

#[test]
fn the_context_menu_of_empty_space_is_the_view_and_paste_menu() {
    let mut cx = cx();
    let l = entries_for(&cx, false);
    let names = labels(&l);
    for want in [
        "Paste",
        "Paste Hold Position",
        "Select All",
        "Zoom In",
        "Fill Window",
    ] {
        assert!(names.contains(&want.to_string()), "{want} in {names:?}");
    }
    // Nothing to paste yet.
    assert!(!l.iter().find(|e| e.label == "Paste").unwrap().enabled);
    // After a copy, Paste is on.
    let w = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    cx.selection.set(ObjectRef::Wall(w));
    cx.copy_selection();
    cx.selection.clear();
    let l = entries_for(&cx, false);
    assert!(l.iter().find(|e| e.label == "Paste").unwrap().enabled);
    // A door tool's menu starts with Select Objects (DW-109).
    let l = entries_for(&cx, true);
    assert_eq!(l[0].label, "Select Objects");
    assert_eq!(l[0].action, Action::SetTool(ToolId::Select));
}

#[test]
fn the_context_menu_of_a_wall_has_reverse_layers_and_the_common_rows() {
    let mut cx = cx();
    let w = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    cx.selection.set(ObjectRef::Wall(w));
    let names = labels(&entries_for(&cx, false));
    for want in [
        "Open Object",
        "Fix Wall Connections",
        "Reverse Layers",
        "Cut",
        "Copy",
        "Paste",
        "Delete",
        "Select Same Type",
        "Lock",
        "Send to Layer\u{2026}",
        "Transform/Replicate Object\u{2026}",
    ] {
        assert!(names.contains(&want.to_string()), "{want} in {names:?}");
    }
    assert!(!names.contains(&"Reverse Swing".to_string()));
    assert!(
        !names.contains(&"Group".to_string()),
        "one object cannot be grouped"
    );
    // Reverse Layers runs.
    let before = cx.floor().wall(w).unwrap().exterior_side;
    cx.run_custom(ids::REVERSE_LAYERS);
    assert_ne!(cx.floor().wall(w).unwrap().exterior_side, before);
}

#[test]
fn the_context_menu_of_a_door_offers_swing_and_hinge() {
    let (mut cx, w) = wall_with_door();
    let door = cx.floor().openings_on(w).next().unwrap().id;
    cx.selection.set(ObjectRef::Opening(door));
    let names = labels(&entries_for(&cx, false));
    assert!(names.contains(&"Reverse Swing".to_string()), "{names:?}");
    assert!(names.contains(&"Flip Hinge".to_string()), "{names:?}");
    assert!(!names.contains(&"Reverse Layers".to_string()));
    let hinge = cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == door)
        .unwrap()
        .hinge_at_end;
    cx.run_custom(ids::FLIP_HINGE);
    assert_ne!(
        cx.floor()
            .openings
            .iter()
            .find(|o| o.id == door)
            .unwrap()
            .hinge_at_end,
        hinge
    );
    cx.run_custom(ids::REVERSE_SWING);
    // A window has neither.
    let win = cx
        .project
        .add_opening(0, w, 40.0, OpeningKind::Window)
        .unwrap();
    cx.selection.set(ObjectRef::Opening(win));
    let names = labels(&entries_for(&cx, false));
    assert!(!names.contains(&"Flip Hinge".to_string()));
    assert!(!names.contains(&"Reverse Swing".to_string()));
}

#[test]
fn the_context_menu_groups_and_explodes_blocks_and_rebuilds_roofs() {
    let mut cx = cx();
    let a = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    let b = wall(&mut cx, (0.0, 200.0), (100.0, 200.0));
    cx.selection.items = vec![ObjectRef::Wall(a), ObjectRef::Wall(b)];
    let names = labels(&entries_for(&cx, false));
    assert!(names.contains(&"Group".to_string()));
    assert!(!names.contains(&"Ungroup".to_string()));
    cx.group_selection();
    let names = labels(&entries_for(&cx, false));
    assert!(names.contains(&"Ungroup".to_string()));
    // A CAD block member offers Explode.
    let ids_: Vec<Id> = (0..2)
        .map(|i| {
            line(
                &mut cx,
                (0.0, f64::from(i) * 5.0),
                (10.0, f64::from(i) * 5.0),
            )
        })
        .collect();
    cx.project.make_cad_block(0, &ids_, Some("Bench")).unwrap();
    cx.selection.items = ids_.iter().map(|i| ObjectRef::Cad(*i)).collect();
    let names = labels(&entries_for(&cx, false));
    assert!(names.contains(&"Explode".to_string()), "{names:?}");
    cx.run_custom(ids::EXPLODE);
    assert!(cx.floor().cad_blocks().is_empty());
}

#[test]
fn the_context_menu_of_a_roof_plane_has_rebuild_roofs() {
    let mut cx = cx();
    cx.selection.set(ObjectRef::RoofPlane(99));
    let names = labels(&entries_for(&cx, false));
    assert!(names.contains(&"Rebuild Roofs".to_string()), "{names:?}");
}

#[test]
fn the_edit_toolbar_offers_group_layer_lock_and_transform() {
    let mut cx = cx();
    let a = wall(&mut cx, (0.0, 0.0), (100.0, 0.0));
    let b = wall(&mut cx, (0.0, 200.0), (100.0, 200.0));
    cx.selection.items = vec![ObjectRef::Wall(a), ObjectRef::Wall(b)];
    let tool = SelectTool::default();
    let names: Vec<&str> = tool.edit_toolbar(&cx).iter().map(|a| a.label).collect();
    for want in [
        "Group Selected Objects",
        "Select Same Type",
        "Transform/Replicate Object",
        "Layer",
        "Lock",
        "Cut",
    ] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    assert!(!names.contains(&"Ungroup"));
}

// ----- the clipboard across kinds -----

#[test]
fn every_copyable_kind_comes_back_with_a_new_id() {
    use super::{
        details_view, foundation_view, framing_view, placed, schedule_view, site_view, stairs_view,
    };
    use plan_cabinets::CabinetKind;
    use plan_core::details::{MoldingProfile, SolidKind};
    use plan_core::{CameraKind, CameraObject, PlacedSymbol};
    let mut cx = cx();
    let w = wall(&mut cx, (0.0, 0.0), (480.0, 0.0));
    cx.project
        .add_opening(0, w, 100.0, OpeningKind::Door)
        .unwrap();
    let cab = crate::tools::cabinet::default_cabinet(&cx, CabinetKind::Base);
    placed::add_cabinet(&mut cx.project, 0, cab).unwrap();
    cx.project.add_symbol(
        0,
        PlacedSymbol::new("none", Point::new(100.0, 100.0), 24.0, 24.0, 30.0),
    );
    let stair = stairs_view::build(
        &cx.project,
        0,
        stairs_view::StairKind::Draw,
        plan_stairs::Turn::Left,
        Point::new(200.0, 50.0),
        Some(Point::new(350.0, 50.0)),
    );
    stairs_view::add(&mut cx.project, 0, stair);
    cx.project.add_camera(CameraObject::new(
        CameraKind::FullCamera,
        Point::new(50.0, 50.0),
        45.0,
        "Camera 1",
        0,
    ));
    let kind = crate::tools::electrical::ElecVariant::Outlet110
        .kind()
        .unwrap();
    let dev = crate::tools::electrical::placement(
        &cx,
        kind,
        Point::new(100.0, 0.0),
        Point::new(100.0, 0.0),
    )
    .unwrap();
    site_view::edit_electrical(&mut cx, "Place", |layer, _| {
        layer.add(dev);
    });
    let square = |x: f64, y: f64, s: f64| {
        vec![
            Point::new(x, y),
            Point::new(x + s, y),
            Point::new(x + s, y + s),
            Point::new(x, y + s),
        ]
    };
    foundation_view::add_slab(&mut cx, square(0.0, 0.0, 100.0), false);
    foundation_view::add_pad(&mut cx, Point::new(300.0, 300.0));
    foundation_view::add_pier(&mut cx, Point::new(340.0, 300.0));
    details_view::add_molding(
        &mut cx,
        vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)],
        MoldingProfile::Crown,
    );
    details_view::add_deck(&mut cx, square(500.0, 0.0, 100.0));
    details_view::add_solid(
        &mut cx,
        SolidKind::Box {
            w: 10.0,
            d: 10.0,
            h: 10.0,
        },
        Point::new(700.0, 0.0),
    );
    details_view::add_floor_region(&mut cx, square(0.0, 500.0, 50.0));
    schedule_view::add(
        &mut cx,
        plan_core::schedules::ScheduleKind::Door,
        Point::new(900.0, 0.0),
    );
    framing_view::add_record(&mut cx, "Place", |id| {
        let mut m = framing_view::new_member(
            &cx_floor_dummy(),
            plan_framing::ManualMemberKind::Joist,
            Point::new(0.0, 700.0),
            Point::new(100.0, 700.0),
        );
        m.id = id;
        framing_view::Record::Manual(m)
    });
    let mut rec = site_view::TerrainRecord::new();
    rec.terrain.walls.push(plan_terrain::TerrainWall::new(
        plan_terrain::WallKind::Wall,
        vec![Point::new(0.0, 900.0), Point::new(300.0, 900.0)],
        false,
    ));
    rec.terrain.landscape.push(plan_terrain::Landscape::new(
        plan_terrain::LandscapeKind::GardenBed,
        plan_terrain::ShapeKind::Polyline,
        square(0.0, 1000.0, 100.0),
    ));
    site_view::save_terrain(&mut cx.project, &rec);
    cx.refresh();

    let count = |cx: &EditorContext| -> Vec<usize> {
        let f = cx.floor();
        vec![
            f.walls.len(),
            f.openings.len(),
            placed::load_cabinets(f).len(),
            f.symbols.len(),
            super::stairs_view::load(f).len(),
            cx.project.cameras_on(0).count(),
            site_view::load_electrical(f).devices.len(),
            plan_core::foundation::FoundationLayer::load(f).len(),
            plan_core::details::DetailsLayer::load(f).len(),
            plan_core::schedules::ScheduleLayer::load(f).schedules.len(),
            framing_view::load_records(f).len(),
            site_view::load_terrain(&cx.project)
                .map_or(0, |r| r.terrain.walls.len() + r.terrain.landscape.len()),
        ]
    };
    let before = count(&cx);
    assert!(
        before.iter().all(|n| *n >= 1),
        "one of each kind: {before:?}"
    );
    cx.run_custom(ids::SELECT_ALL);
    let picked = cx.selection.len();
    assert!(picked >= 15, "select all found {picked} objects");
    cx.run_custom(ids::COPY);
    cx.run_custom(ids::PASTE_HOLD);
    let after = count(&cx);
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(*a, b * 2, "every kind doubled: {before:?} -> {after:?}");
    }
    // One undo step puts it all back.
    cx.undo();
    assert_eq!(count(&cx), before);
    // The ids are unique across the plan.
    cx.run_custom(ids::PASTE_HOLD);
    let ids_: Vec<Id> = cx
        .floor()
        .walls
        .iter()
        .map(|w| w.id)
        .chain(placed::load_cabinets(cx.floor()).iter().map(|c| c.id))
        .chain(cx.floor().symbols.iter().map(|s| s.id))
        .collect();
    let mut sorted = ids_.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids_.len());
}

fn cx_floor_dummy() -> plan_core::Floor {
    cx().floor().clone()
}

#[test]
fn rotating_a_selection_with_cabinets_stairs_and_symbols_turns_them() {
    use super::{placed, stairs_view};
    use plan_cabinets::CabinetKind;
    use plan_core::PlacedSymbol;
    let mut cx = cx();
    let cab = crate::tools::cabinet::default_cabinet(&cx, CabinetKind::Base);
    let cab_id = placed::add_cabinet(&mut cx.project, 0, cab).unwrap();
    let sym = cx.project.add_symbol(
        0,
        PlacedSymbol::new("none", Point::new(100.0, 100.0), 24.0, 24.0, 30.0),
    );
    let stair = stairs_view::build(
        &cx.project,
        0,
        stairs_view::StairKind::Draw,
        plan_stairs::Turn::Left,
        Point::new(200.0, 50.0),
        Some(Point::new(350.0, 50.0)),
    );
    let stair_id = stairs_view::add(&mut cx.project, 0, stair);
    cx.selection.items = vec![
        ObjectRef::Cabinet(cab_id),
        ObjectRef::Symbol(sym),
        ObjectRef::Stair(stair_id),
    ];
    let cab0 = placed::cabinet_by_id(cx.floor(), cab_id).unwrap();
    let dir0 = stairs_view::find(cx.floor(), stair_id)
        .unwrap()
        .stair
        .direction;
    let rep = transform::rotate_selection(&mut cx, std::f64::consts::FRAC_PI_2, Some(Point::ZERO));
    assert_eq!(rep.changed, 3);
    assert!(rep.skipped.is_empty());
    let cab1 = placed::cabinet_by_id(cx.floor(), cab_id).unwrap();
    assert!((cab1.angle - cab0.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert!(near(
        cab1.position,
        Point::new(-cab0.position.y, cab0.position.x)
    ));
    let dir1 = stairs_view::find(cx.floor(), stair_id)
        .unwrap()
        .stair
        .direction;
    assert!((dir1 - dir0 - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert!(near(
        cx.floor().symbol(sym).unwrap().position,
        Point::new(-100.0, 100.0)
    ));
    assert_eq!(cx.undo_label(), Some("Rotate"));
}

#[test]
fn mirroring_a_cabinet_keeps_its_footprint_on_the_mirrored_side() {
    use super::placed;
    use plan_cabinets::CabinetKind;
    let mut cx = cx();
    let mut cab = crate::tools::cabinet::default_cabinet(&cx, CabinetKind::Base);
    cab.position = Point::new(100.0, 0.0);
    cab.angle = 0.0;
    let id = placed::add_cabinet(&mut cx.project, 0, cab).unwrap();
    let before = placed::cabinet_by_id(cx.floor(), id).unwrap();
    let mut xs: Vec<f64> = before.corners().iter().map(|c| c.x).collect();
    xs.sort_by(f64::total_cmp);
    cx.selection.set(ObjectRef::Cabinet(id));
    // Mirror about the vertical line x = 0.
    let rep = transform::reflect_selection(&mut cx, Point::ZERO, Point::new(0.0, 10.0), false);
    assert_eq!(rep.changed, 1);
    let after = placed::cabinet_by_id(cx.floor(), id).unwrap();
    let mut mx: Vec<f64> = after.corners().iter().map(|c| c.x).collect();
    mx.sort_by(f64::total_cmp);
    for (b, a) in xs.iter().zip(mx.iter().rev()) {
        assert!((b + a).abs() < 1e-6, "mirrored x: {xs:?} vs {mx:?}");
    }
}

#[test]
fn rotating_kinds_that_cannot_turn_says_so() {
    let mut cx = cx();
    let l = line(&mut cx, (0.0, 0.0), (10.0, 0.0));
    cx.selection.items = vec![
        ObjectRef::Cad(l),
        ObjectRef::RoofPlane(5),
        ObjectRef::Schedule(6),
    ];
    let rep = transform::apply_xform(
        &mut cx,
        &[ObjectRef::Cad(l), ObjectRef::RoofPlane(5)],
        &plan_core::transform::Xform::rotate(Point::ZERO, 1.0),
    );
    assert_eq!(rep.changed, 1);
    assert_eq!(rep.skipped, vec!["Roof Plane"]);
    assert!(rep.status("Rotated").contains("Roof Plane"));
    let p = TransformParams {
        reflect: Some(ReflectAxis::Vertical(0.0)),
        ..TransformParams::default()
    };
    assert!(!p.xform(Point::ZERO).is_identity());
}

#[test]
fn copy_leaves_a_note_for_the_system_clipboard_once() {
    use super::clipboard::take_system_clipboard_note;
    let (mut cx, _) = wall_with_door();
    assert!(!take_system_clipboard_note());
    cx.run_custom(ids::COPY);
    assert!(
        take_system_clipboard_note(),
        "egui needs text there to send Paste"
    );
    assert!(!take_system_clipboard_note(), "only once per copy");
    cx.run_custom(ids::CUT);
    assert!(take_system_clipboard_note());
}

#[test]
fn the_clipboard_pastes_onto_another_floor() {
    let (mut cx, _) = wall_with_door();
    cx.run_custom(ids::COPY);
    cx.project.build_new_floor(false);
    cx.floor = 1;
    cx.run_custom(ids::PASTE_HOLD);
    assert_eq!(cx.project.floors[1].walls.len(), 1);
    assert_eq!(cx.project.floors[1].openings.len(), 1);
    assert_eq!(cx.project.floors[0].walls.len(), 1, "floor 0 is untouched");
}
