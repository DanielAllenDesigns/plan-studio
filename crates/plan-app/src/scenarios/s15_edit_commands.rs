//! Scenario 15: the Edit menu's commands driven through the Select tool and
//! the command router: cursor-attached Paste, Paste Hold Position, Select
//! All on locked layers, Duplicate, Transform/Replicate, Reflect About Object,
//! Point to Point Move, Center Object, Align/Distribute, the right-click
//! menu, Group, Delete Objects and the Action History (S-8, S-32..S-36,
//! S-47, S-48, S-52..S-54, S-79, S-81..S-84, S-88, S-101..S-106, DW-109).

use super::{draw_shell, Sim};
use crate::dialogs::action_history::{self, Jump};
use crate::dialogs::delete_objects::{self, Category};
use crate::dialogs::transform::TransformDialog;
use crate::editor::edit_commands::{ids, DUPLICATE_OFFSET};
use crate::editor::transform::{self, TransformParams};
use crate::editor::ObjectRef;
use crate::toolbar::Action;
use crate::tools::cad::CadMode;
use crate::tools::ToolId;
use eframe::egui::Key;
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::{Id, OpeningKind, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Select);
    sim
}

/// A free-standing partition drawn with the Interior Wall tool.
fn partition(sim: &mut Sim, a: (f64, f64), b: (f64, f64)) -> Id {
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag(a, b);
    sim.tool(ToolId::Select);
    sim.app.cx.floor().walls.last().unwrap().id
}

/// A CAD line drawn with the Draw Line tool.
fn cad_line(sim: &mut Sim, a: (f64, f64), b: (f64, f64)) -> Id {
    sim.tool(ToolId::CadVariant(CadMode::Line));
    sim.drag(a, b);
    sim.tool(ToolId::Select);
    sim.app.cx.floor().cad.last().unwrap().id
}

fn has_clip(sim: &Sim) -> bool {
    sim.app.cx.clipboard.as_ref().is_some_and(|c| !c.is_empty())
}

fn wall_ids(sim: &Sim) -> Vec<Id> {
    sim.wall_ids()
}

fn line_of(sim: &Sim, id: Id) -> (Point, Point) {
    match sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .item
    {
        CadItem::Line { a, b } => (a, b),
        _ => panic!("not a line"),
    }
}

fn cmd(sim: &mut Sim, id: &'static str) {
    sim.action(Action::Custom(id));
}

fn select(sim: &mut Sim, items: &[ObjectRef]) {
    sim.app.cx.selection.items = items.to_vec();
}

fn mid(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

#[test]
fn copy_then_paste_hangs_on_the_pointer_until_a_click_drops_it_and_esc_cancels() {
    let mut sim = house();
    let w = partition(&mut sim, (100.0, 100.0), (300.0, 100.0));
    // Pick the partition with a click on it.
    sim.click(200.0, 100.0);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Wall(w)));
    cmd(&mut sim, ids::COPY);
    assert!(has_clip(&sim));
    assert_eq!(sim.floor_walls(), 5, "copy changes nothing");

    // Paste from another tool: the tool changes to Select Objects and the
    // copy hangs on the pointer.
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    cmd(&mut sim, ids::PASTE);
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
    assert!(transform::mode_active());
    assert_eq!(sim.floor_walls(), 5, "nothing lands before the click");
    sim.move_to(250.0, 250.0);
    // Esc cancels: no wall, no undo step.
    let before = sim.app.cx.undo_label().map(String::from);
    sim.esc();
    assert!(!transform::mode_active());
    assert_eq!(sim.floor_walls(), 5);
    assert_eq!(sim.app.cx.undo_label().map(String::from), before);

    // Paste again and click: the copy's center lands on the click.
    cmd(&mut sim, ids::PASTE);
    let r = sim.click(250.0, 250.0);
    assert_eq!(r.commit.as_deref(), Some("Paste"));
    assert_eq!(sim.floor_walls(), 6);
    let ObjectRef::Wall(copy) = sim.app.cx.selection.single().unwrap() else {
        panic!("the copy is selected")
    };
    assert_ne!(copy, w);
    let c = sim.app.cx.floor().wall(copy).unwrap().clone();
    assert!(
        mid(c.start, c.end).dist(Point::new(250.0, 250.0)) <= 1.5,
        "{c:?}"
    );
    // The original did not move; one undo step takes the copy away.
    let o = sim.app.cx.floor().wall(w).unwrap();
    assert_eq!(
        (o.start, o.end),
        (Point::new(100.0, 100.0), Point::new(300.0, 100.0))
    );
    assert_eq!(sim.undo().as_deref(), Some("Paste"));
    assert_eq!(sim.floor_walls(), 5);
}

#[test]
fn paste_hold_position_puts_the_copy_exactly_on_the_original() {
    let mut sim = house();
    let w = partition(&mut sim, (100.0, 100.0), (300.0, 100.0));
    select(&mut sim, &[ObjectRef::Wall(w)]);
    cmd(&mut sim, ids::COPY);
    cmd(&mut sim, ids::PASTE_HOLD);
    assert!(!transform::mode_active(), "no ghost: it lands at once");
    assert_eq!(sim.floor_walls(), 6);
    let copy = *sim.wall_ids().last().unwrap();
    let (a, b) = {
        let c = sim.app.cx.floor().wall(copy).unwrap();
        (c.start, c.end)
    };
    assert_eq!((a, b), (Point::new(100.0, 100.0), Point::new(300.0, 100.0)));
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Wall(copy)));
    assert_eq!(sim.app.cx.undo_label(), Some("Paste Hold Position"));
}

#[test]
fn cut_is_one_undo_step_keeps_the_clipboard_and_a_locked_layer_refuses_it() {
    let mut sim = house();
    let l1 = cad_line(&mut sim, (60.0, 60.0), (160.0, 60.0));
    select(&mut sim, &[ObjectRef::Cad(l1)]);
    cmd(&mut sim, ids::CUT);
    assert!(sim.app.cx.floor().cad.is_empty());
    assert!(has_clip(&sim));
    assert_eq!(sim.app.cx.undo_label(), Some("Cut"));
    cmd(&mut sim, ids::PASTE_HOLD);
    assert_eq!(sim.app.cx.floor().cad.len(), 1, "paste brings it back");

    // Lock the line's layer: Cut refuses and leaves the line alone.
    let id = sim.app.cx.floor().cad[0].id;
    select(&mut sim, &[ObjectRef::Cad(id)]);
    cmd(&mut sim, ids::LOCK);
    assert!(
        sim.app.cx.selection.is_empty(),
        "locked objects leave the selection"
    );
    select(&mut sim, &[ObjectRef::Cad(id)]);
    cmd(&mut sim, ids::CUT);
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    assert!(
        sim.app.cx.status.contains("locked"),
        "{}",
        sim.app.cx.status
    );
}

#[test]
fn select_all_takes_everything_on_open_layers_and_leaves_locked_and_hidden_ones() {
    let mut sim = house();
    let l1 = cad_line(&mut sim, (60.0, 60.0), (160.0, 60.0));
    let l2 = cad_line(&mut sim, (60.0, 90.0), (160.0, 90.0));
    cmd(&mut sim, ids::SELECT_ALL);
    let all = sim.app.cx.selection.len();
    assert!(
        sim.app.cx.selection.items.contains(&ObjectRef::Cad(l1))
            && sim.app.cx.selection.items.contains(&ObjectRef::Cad(l2))
    );
    assert!(all >= 6, "four walls and two lines, got {all}");

    // Put line 2 on a layer of its own and lock it, as Chief's Lock command does.
    let other = "CAD, Extra".to_string();
    sim.app
        .cx
        .project
        .layers
        .add(plan_core::Layer::new(other.clone(), [10, 20, 30], 2));
    select(&mut sim, &[ObjectRef::Cad(l2)]);
    assert_eq!(sim.app.cx.send_selection_to_layer(&other), 1);
    sim.app.cx.project.layers.set_locked(&other, true);
    cmd(&mut sim, ids::SELECT_ALL);
    assert!(!sim.app.cx.selection.items.contains(&ObjectRef::Cad(l2)));
    assert!(sim.app.cx.selection.items.contains(&ObjectRef::Cad(l1)));
    assert_eq!(sim.app.cx.selection.len(), all - 1);

    // Hidden layers drop out as well.
    sim.app.cx.project.layers.set_locked(&other, false);
    sim.app.cx.project.layers.set_display(&other, false);
    cmd(&mut sim, ids::SELECT_ALL);
    assert!(!sim.app.cx.selection.items.contains(&ObjectRef::Cad(l2)));
    sim.app.cx.project.layers.set_display(&other, true);
    cmd(&mut sim, ids::SELECT_ALL);
    assert_eq!(sim.app.cx.selection.len(), all);
}

#[test]
fn duplicate_offsets_the_copy_by_twelve_inches_and_leaves_the_clipboard_alone() {
    let mut sim = house();
    let w = partition(&mut sim, (100.0, 100.0), (300.0, 100.0));
    select(&mut sim, &[ObjectRef::Wall(w)]);
    assert!(!has_clip(&sim));
    cmd(&mut sim, ids::DUPLICATE);
    assert!(!has_clip(&sim), "Duplicate does not use the clipboard");
    assert_eq!(sim.floor_walls(), 6);
    let copy = *sim.wall_ids().last().unwrap();
    let c = sim.app.cx.floor().wall(copy).unwrap();
    assert_eq!(c.start, Point::new(100.0, 100.0) + DUPLICATE_OFFSET);
    assert_eq!(c.end, Point::new(300.0, 100.0) + DUPLICATE_OFFSET);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Wall(copy)));
    assert_eq!(sim.app.cx.undo_label(), Some("Duplicate"));
    // A second Duplicate copies the copy.
    cmd(&mut sim, ids::DUPLICATE);
    let third = sim.app.cx.floor().walls.last().unwrap().start;
    assert_eq!(
        third,
        Point::new(100.0, 100.0) + DUPLICATE_OFFSET + DUPLICATE_OFFSET
    );
}

#[test]
fn transform_replicate_makes_three_copies_each_one_offset_further() {
    let mut sim = house();
    let l = cad_line(&mut sim, (50.0, 50.0), (100.0, 50.0));
    select(&mut sim, &[ObjectRef::Cad(l)]);
    // The menu command opens the window.
    cmd(&mut sim, ids::TRANSFORM);
    assert!(crate::dialogs::transform::is_open());
    let mut d = TransformDialog {
        make_copies: true,
        copies: 3,
        move_x: "2'".into(),
        move_y: "1'".into(),
        ..TransformDialog::default()
    };
    assert!(d.apply(&mut sim.app.cx));
    assert_eq!(sim.app.cx.floor().cad.len(), 4, "the original and 3 copies");
    assert_eq!(sim.app.cx.undo_label(), Some("Transform/Replicate"));
    // Copy k sits k times the offset from the original.
    let starts: Vec<Point> = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .map(|c| match c.item {
            CadItem::Line { a, .. } => a,
            _ => unreachable!(),
        })
        .collect();
    for (k, a) in starts.iter().enumerate() {
        let want = Point::new(50.0 + 24.0 * k as f64, 50.0 + 12.0 * k as f64);
        assert!(a.dist(want) < 1e-6, "copy {k}: {a:?} vs {want:?}");
    }
    // The three copies are selected and one undo takes them all.
    assert_eq!(sim.app.cx.selection.len(), 3);
    assert_eq!(sim.undo().as_deref(), Some("Transform/Replicate"));
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
}

#[test]
fn a_radial_replicate_turns_each_copy_a_further_step_about_the_pivot() {
    let mut sim = house();
    let l = cad_line(&mut sim, (200.0, 100.0), (260.0, 100.0));
    select(&mut sim, &[ObjectRef::Cad(l)]);
    let p = TransformParams {
        copies: 3,
        rotate_deg: 90.0,
        rotate_about: Some(Point::new(200.0, 100.0)),
        ..TransformParams::default()
    };
    transform::transform_replicate(&mut sim.app.cx, &p).unwrap();
    let lines: Vec<(Point, Point)> = sim
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
    assert_eq!(lines.len(), 4);
    let pivot = Point::new(200.0, 100.0);
    let want = [
        Point::new(260.0, 100.0),
        Point::new(200.0, 160.0),
        Point::new(140.0, 100.0),
        Point::new(200.0, 40.0),
    ];
    for (k, (a, b)) in lines.iter().enumerate() {
        // The pivot end stays; the far end walks around the circle.
        assert!(a.dist(pivot) < 1e-6 || b.dist(pivot) < 1e-6, "copy {k}");
        let far = if a.dist(pivot) < 1e-6 { b } else { a };
        assert!(
            far.dist(want[k]) < 1e-6,
            "copy {k}: {far:?} vs {:?}",
            want[k]
        );
    }
    // One undo step for the whole array.
    assert_eq!(sim.app.cx.undo_label(), Some("Transform/Replicate"));
    sim.undo();
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
}

#[test]
fn reflect_about_object_mirrors_a_wall_its_exterior_side_and_a_door_swing() {
    let mut sim = house();
    let w = partition(&mut sim, (100.0, 100.0), (300.0, 100.0));
    sim.tool(ToolId::Door);
    sim.click(150.0, 100.0);
    sim.tool(ToolId::Select);
    let door = sim.app.cx.floor().openings.last().unwrap().clone();
    assert_eq!(door.kind, OpeningKind::Door);
    // A CAD line is the mirror.
    let axis = cad_line(&mut sim, (400.0, 0.0), (400.0, 360.0));
    let side = sim.app.cx.floor().wall(w).unwrap().exterior_side;
    select(&mut sim, &[ObjectRef::Wall(w)]);
    cmd(&mut sim, ids::REFLECT);
    assert!(transform::mode_active());
    // A miss keeps the mode (no mirror line there).
    sim.click(30.0, 30.0);
    assert!(transform::mode_active());
    let r = sim.click(400.0, 200.0);
    assert_eq!(r.commit.as_deref(), Some("Reflect"));
    assert!(!transform::mode_active());
    let _ = axis;
    let m = sim.app.cx.floor().wall(w).unwrap().clone();
    // x = 400 mirror: 100..300 becomes 700..500, same y.
    assert!(m.start.dist(Point::new(700.0, 100.0)) < 1e-6, "{m:?}");
    assert!(m.end.dist(Point::new(500.0, 100.0)) < 1e-6, "{m:?}");
    assert_ne!(m.exterior_side, side, "the layers mirror with the wall");
    let md = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == door.id)
        .unwrap();
    assert_ne!(md.swing_flipped, door.swing_flipped, "the swing mirrors");
    assert_eq!(
        md.center_offset, door.center_offset,
        "same place along the wall"
    );
    assert_eq!(sim.undo().as_deref(), Some("Reflect"));
    let o = sim.app.cx.floor().wall(w).unwrap();
    assert_eq!(o.start, Point::new(100.0, 100.0));
}

#[test]
fn reflect_copy_leaves_the_original_and_makes_a_mirrored_twin() {
    let mut sim = house();
    let w = partition(&mut sim, (100.0, 100.0), (300.0, 100.0));
    cad_line(&mut sim, (400.0, 0.0), (400.0, 360.0));
    select(&mut sim, &[ObjectRef::Wall(w)]);
    cmd(&mut sim, ids::REFLECT_COPY);
    sim.click(400.0, 200.0);
    assert_eq!(sim.floor_walls(), 6);
    assert!(sim.app.cx.floor().wall(w).is_some());
    assert_eq!(sim.app.cx.undo_label(), Some("Reflect Copy"));
}

#[test]
fn point_to_point_move_takes_two_clicks_and_is_one_undo_step() {
    let mut sim = house();
    let l = cad_line(&mut sim, (50.0, 50.0), (100.0, 50.0));
    select(&mut sim, &[ObjectRef::Cad(l)]);
    cmd(&mut sim, ids::POINT_TO_POINT);
    assert!(transform::mode_active());
    let r = sim.click(50.0, 50.0);
    assert!(
        r.consumed && r.commit.is_none(),
        "the first click only marks the point"
    );
    assert!(transform::mode_active());
    let r = sim.click(150.0, 210.0);
    assert_eq!(r.commit.as_deref(), Some("Point to Point Move"));
    let (a, b) = line_of(&sim, l);
    assert!(a.dist(Point::new(150.0, 210.0)) < 1e-6, "{a:?}");
    assert!(b.dist(Point::new(200.0, 210.0)) < 1e-6, "{b:?}");
    assert_eq!(sim.undo().as_deref(), Some("Point to Point Move"));
    assert_eq!(line_of(&sim, l).0, Point::new(50.0, 50.0));
    // Esc between the clicks drops the move.
    select(&mut sim, &[ObjectRef::Cad(l)]);
    cmd(&mut sim, ids::POINT_TO_POINT);
    sim.click(50.0, 50.0);
    sim.esc();
    assert!(!transform::mode_active());
    assert_eq!(line_of(&sim, l).0, Point::new(50.0, 50.0));
}

#[test]
fn center_object_puts_a_selection_in_the_middle_of_the_clicked_room() {
    let mut sim = house();
    let l = cad_line(&mut sim, (50.0, 50.0), (110.0, 50.0));
    select(&mut sim, &[ObjectRef::Cad(l)]);
    cmd(&mut sim, ids::CENTER);
    assert!(transform::mode_active());
    // A click on empty space outside the room names nothing.
    sim.click(-200.0, -200.0);
    assert!(transform::mode_active());
    let r = sim.click(300.0, 250.0);
    assert_eq!(r.commit.as_deref(), Some("Center Object"));
    let (a, b) = line_of(&sim, l);
    let c = mid(a, b);
    let room = sim.app.cx.rooms[0].polygon.clone();
    let (lo, hi) = room.iter().fold(
        (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        ),
        |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    );
    assert!(c.dist(mid(lo, hi)) < 1e-6, "{c:?} vs {:?}", mid(lo, hi));
    assert_eq!(sim.undo().as_deref(), Some("Center Object"));
}

#[test]
fn center_object_on_a_door_centers_it_on_its_wall_at_once() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(80.0, 0.0);
    sim.tool(ToolId::Select);
    let door = sim.app.cx.floor().openings[0].clone();
    let len = sim.app.cx.floor().wall(door.wall_id).unwrap().length();
    assert!((door.center_offset - len / 2.0).abs() > 50.0);
    select(&mut sim, &[ObjectRef::Opening(door.id)]);
    cmd(&mut sim, ids::CENTER);
    assert!(!transform::mode_active(), "no clicks needed for an opening");
    let after = sim.app.cx.floor().openings[0].center_offset;
    assert!((after - len / 2.0).abs() < 1.0, "{after} of {len}");
    assert_eq!(sim.app.cx.undo_label(), Some("Center Object"));
}

#[test]
fn align_and_distribute_line_objects_up_in_one_step_each() {
    let mut sim = house();
    let a = cad_line(&mut sim, (40.0, 40.0), (60.0, 50.0));
    let b = cad_line(&mut sim, (100.0, 90.0), (130.0, 100.0));
    let c = cad_line(&mut sim, (300.0, 150.0), (320.0, 160.0));
    let all = [ObjectRef::Cad(a), ObjectRef::Cad(b), ObjectRef::Cad(c)];
    select(&mut sim, &all);
    cmd(&mut sim, plan_core::transform::AlignMode::Left.id());
    let lefts: Vec<f64> = [a, b, c]
        .iter()
        .map(|i| line_of(&sim, *i).0.x.min(line_of(&sim, *i).1.x))
        .collect();
    assert!(lefts.iter().all(|x| (x - 40.0).abs() < 1e-6), "{lefts:?}");
    assert_eq!(sim.app.cx.undo_label(), Some("Align Left"));
    sim.undo();
    // Distribute horizontally: equal gaps between the three.
    select(&mut sim, &all);
    cmd(&mut sim, ids::DISTRIBUTE_H);
    let span = |i: Id| {
        let (p, q) = line_of(&sim, i);
        (p.x.min(q.x), p.x.max(q.x))
    };
    let (sa, sb, sc) = (span(a), span(b), span(c));
    let g1 = sb.0 - sa.1;
    let g2 = sc.0 - sb.1;
    assert!((g1 - g2).abs() < 1e-6, "gaps {g1} {g2}");
    assert_eq!(sim.app.cx.undo_label(), Some("Distribute Horizontally"));
    // The Align/Distribute window opens for two or more objects only.
    select(&mut sim, &[ObjectRef::Cad(a)]);
    cmd(&mut sim, ids::ALIGN_DIALOG);
    assert!(
        sim.app.cx.status.contains("two or more"),
        "{}",
        sim.app.cx.status
    );
}

fn labels(sim: &Sim) -> Vec<String> {
    let tb = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    sim.app
        .cx
        .context_entries(&tb, false)
        .into_iter()
        .map(|e| e.label)
        .collect()
}

#[test]
fn the_right_click_menu_lists_the_commands_that_fit_each_kind_of_object() {
    let mut sim = house();
    sim.app.cx.selection.clear();
    // Nothing selected: the view menu, Paste dimmed until something is copied.
    let tb = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    let empty = sim.app.cx.context_entries(&tb, false);
    let paste = empty.iter().find(|e| e.label == "Paste").unwrap();
    assert!(!paste.enabled);
    for want in [
        "Paste Hold Position",
        "Select All",
        "Zoom In",
        "Zoom Out",
        "Fill Window",
    ] {
        assert!(
            empty.iter().any(|e| e.label == want),
            "empty menu lacks {want}"
        );
    }

    // A wall.
    let w = partition(&mut sim, (100.0, 100.0), (300.0, 100.0));
    select(&mut sim, &[ObjectRef::Wall(w)]);
    let wall_menu = labels(&sim);
    for want in [
        "Open Object",
        "Reverse Layers",
        "Cut",
        "Copy",
        "Delete",
        "Select Same Type",
        "Lock",
        "Transform/Replicate Object\u{2026}",
    ] {
        assert!(
            wall_menu.iter().any(|l| l == want),
            "wall menu lacks {want}: {wall_menu:?}"
        );
    }
    assert!(!wall_menu.iter().any(|l| l == "Reverse Swing"));
    assert!(
        !wall_menu.iter().any(|l| l == "Group"),
        "one object cannot be grouped"
    );

    // A door adds the swing commands.
    sim.tool(ToolId::Door);
    sim.click(150.0, 100.0);
    sim.tool(ToolId::Select);
    let d = sim.app.cx.floor().openings.last().unwrap().id;
    select(&mut sim, &[ObjectRef::Opening(d)]);
    let door_menu = labels(&sim);
    for want in ["Reverse Swing", "Flip Hinge", "Cut", "Copy", "Delete"] {
        assert!(
            door_menu.iter().any(|l| l == want),
            "door menu lacks {want}: {door_menu:?}"
        );
    }

    // A CAD line: no wall commands, but Move to Front lives on the toolbar.
    let l = cad_line(&mut sim, (50.0, 250.0), (150.0, 250.0));
    select(&mut sim, &[ObjectRef::Cad(l)]);
    let cad_menu = labels(&sim);
    assert!(!cad_menu.iter().any(|x| x == "Reverse Layers"));
    assert!(cad_menu.iter().any(|x| x == "Delete"));

    // Two objects add Group; once copied, Paste is live.
    select(&mut sim, &[ObjectRef::Wall(w), ObjectRef::Cad(l)]);
    assert!(labels(&sim).iter().any(|x| x == "Group"));
    cmd(&mut sim, ids::COPY);
    let tb = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    let entries = sim.app.cx.context_entries(&tb, false);
    assert!(entries.iter().find(|e| e.label == "Paste").unwrap().enabled);

    // While a door tool shows its ghost the menu starts with Select Objects.
    sim.tool(ToolId::Door);
    let tb = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    let ghost = sim.app.cx.context_entries(&tb, true);
    assert_eq!(ghost[0].label, "Select Objects");
}

#[test]
fn group_binds_objects_so_a_click_takes_them_all_and_ungroup_lets_go() {
    let mut sim = house();
    let a = cad_line(&mut sim, (50.0, 50.0), (150.0, 50.0));
    let b = cad_line(&mut sim, (50.0, 200.0), (150.0, 200.0));
    select(&mut sim, &[ObjectRef::Cad(a)]);
    cmd(&mut sim, ids::GROUP);
    assert!(
        sim.app.cx.status.contains("two or more"),
        "one object is not a group"
    );
    select(&mut sim, &[ObjectRef::Cad(a), ObjectRef::Cad(b)]);
    cmd(&mut sim, ids::GROUP);
    assert_eq!(sim.app.cx.floor().groups.len(), 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Group"));
    // A click on one member selects both.
    sim.app.cx.selection.clear();
    sim.click(100.0, 50.0);
    assert_eq!(
        sim.app.cx.selection.len(),
        2,
        "{:?}",
        sim.app.cx.selection.items
    );
    // The Edit toolbar offers Ungroup for a group.
    let tb = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    assert!(tb.iter().any(|x| x.label == "Ungroup"));
    cmd(&mut sim, ids::UNGROUP);
    assert!(sim.app.cx.floor().groups.is_empty());
    sim.app.cx.selection.clear();
    sim.click(100.0, 50.0);
    assert_eq!(sim.app.cx.selection.len(), 1, "free again");
    // Undo restores the group.
    assert_eq!(sim.undo().as_deref(), Some("Ungroup"));
    assert_eq!(sim.app.cx.floor().groups.len(), 1);
}

#[test]
fn delete_objects_removes_every_object_of_the_checked_types_as_one_step() {
    let mut sim = house();
    cad_line(&mut sim, (50.0, 50.0), (150.0, 50.0));
    cad_line(&mut sim, (50.0, 80.0), (150.0, 80.0));
    sim.tool(ToolId::Door);
    sim.click(150.0, 0.0);
    sim.tool(ToolId::Select);
    // Shift+Space / Edit > Delete Objects opens the window.
    cmd(&mut sim, ids::DELETE_OBJECTS);
    sim.dialog_frame(false);
    let counts = delete_objects::counts(&sim.app.cx);
    let n = |c: Category| counts.iter().find(|(k, _)| *k == c).unwrap().1;
    assert_eq!(
        (
            n(Category::Walls),
            n(Category::Doors),
            n(Category::CadLines)
        ),
        (4, 1, 2)
    );
    let removed = delete_objects::delete_by_category(
        &mut sim.app.cx,
        &[Category::CadLines, Category::Doors],
        false,
    );
    assert_eq!(removed, 3);
    assert!(sim.app.cx.floor().cad.is_empty() && sim.app.cx.floor().openings.is_empty());
    assert_eq!(sim.floor_walls(), 4, "unchecked types stay");
    assert_eq!(sim.app.cx.undo_label(), Some("Delete Objects"));
    sim.undo();
    assert_eq!(sim.app.cx.floor().cad.len(), 2);
    assert_eq!(sim.app.cx.floor().openings.len(), 1);
    sim.cancel();
}

#[test]
fn the_action_history_lists_the_steps_and_a_click_jumps_there_and_back() {
    let mut sim = house();
    let steps_before = sim.app.cx.action_history().0.len();
    let a = cad_line(&mut sim, (50.0, 50.0), (150.0, 50.0));
    cad_line(&mut sim, (50.0, 80.0), (150.0, 80.0));
    cad_line(&mut sim, (50.0, 110.0), (150.0, 110.0));
    select(&mut sim, &[ObjectRef::Cad(a)]);
    cmd(&mut sim, ids::DUPLICATE);
    let (past, future) = sim.app.cx.action_history();
    assert_eq!(past.len(), steps_before + 4);
    assert!(future.is_empty());
    assert_eq!(past.last().map(String::as_str), Some("Duplicate"));
    assert_eq!(sim.app.cx.floor().cad.len(), 4);

    // The View menu toggles the window; a headless frame draws it.
    cmd(&mut sim, ids::HISTORY);
    assert!(action_history::is_open());
    sim.dialog_frame(false);

    // Click the first line's step: only that line is left.
    let went = action_history::jump(&mut sim.app.cx, Jump::Back(steps_before));
    assert_eq!(went, 3);
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    let (past, future) = sim.app.cx.action_history();
    assert_eq!(past.len(), steps_before + 1);
    assert_eq!(future.len(), 3);
    // Clicking the redo list goes forward again.
    action_history::jump(&mut sim.app.cx, Jump::Forward(3));
    assert_eq!(sim.app.cx.floor().cad.len(), 4);
    // A new action after a jump back drops the redo list.
    action_history::jump(&mut sim.app.cx, Jump::Back(steps_before));
    cad_line(&mut sim, (50.0, 300.0), (150.0, 300.0));
    assert!(sim.app.cx.action_history().1.is_empty());
    cmd(&mut sim, ids::HISTORY);
    assert!(!action_history::is_open());
    let _ = Key::Escape;
}

#[test]
fn duplicating_a_wall_takes_its_door_along_and_one_undo_removes_both() {
    let mut sim = house();
    let w = partition(&mut sim, (100.0, 100.0), (300.0, 100.0));
    sim.tool(ToolId::Door);
    sim.click(200.0, 100.0);
    sim.tool(ToolId::Select);
    assert_eq!(sim.app.cx.floor().openings.len(), 1);
    select(&mut sim, &[ObjectRef::Wall(w)]);
    cmd(&mut sim, ids::DUPLICATE);
    assert_eq!(sim.floor_walls(), 6);
    assert_eq!(
        sim.app.cx.floor().openings.len(),
        2,
        "the door was copied with the wall"
    );
    let copy = *sim.wall_ids().last().unwrap();
    let hosted: Vec<_> = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .filter(|o| o.wall_id == copy)
        .collect();
    assert_eq!(hosted.len(), 1);
    assert_eq!(
        hosted[0].center_offset,
        sim.app.cx.floor().openings[0].center_offset
    );
    sim.undo();
    assert_eq!(sim.floor_walls(), 5);
    assert_eq!(sim.app.cx.floor().openings.len(), 1);
}

#[test]
fn dragging_a_member_of_a_group_moves_the_whole_group_in_one_undo_step() {
    let mut sim = house();
    let a = cad_line(&mut sim, (50.0, 50.0), (150.0, 50.0));
    let b = cad_line(&mut sim, (50.0, 200.0), (150.0, 200.0));
    select(&mut sim, &[ObjectRef::Cad(a), ObjectRef::Cad(b)]);
    cmd(&mut sim, ids::GROUP);
    sim.app.cx.selection.clear();
    sim.click(100.0, 50.0);
    assert_eq!(sim.app.cx.selection.len(), 2);
    let steps = sim.app.cx.action_history().0.len();
    sim.drag((100.0, 50.0), (160.0, 80.0));
    assert_eq!(line_of(&sim, a).0, Point::new(110.0, 80.0));
    assert_eq!(line_of(&sim, b).0, Point::new(110.0, 230.0));
    assert_eq!(
        sim.app.cx.action_history().0.len(),
        steps + 1,
        "one step for the move"
    );
    sim.undo();
    assert_eq!(line_of(&sim, a).0, Point::new(50.0, 50.0));
    assert_eq!(line_of(&sim, b).0, Point::new(50.0, 200.0));
}

#[test]
fn copy_on_one_floor_pastes_on_another_and_undo_leaves_the_first_alone() {
    let mut sim = house();
    let w = partition(&mut sim, (100.0, 100.0), (300.0, 100.0));
    select(&mut sim, &[ObjectRef::Wall(w)]);
    cmd(&mut sim, ids::COPY);
    sim.action(Action::BuildNewFloor);
    sim.ok();
    assert_eq!(sim.app.cx.floor, 1);
    let upstairs = sim.floor_walls();
    cmd(&mut sim, ids::PASTE_HOLD);
    assert_eq!(
        sim.floor_walls(),
        upstairs + 1,
        "the wall landed on floor 2"
    );
    assert_eq!(
        sim.app.cx.project.floors[0].walls.len(),
        5,
        "floor 1 is untouched"
    );
    sim.undo();
    assert_eq!(sim.floor_walls(), upstairs);
}
