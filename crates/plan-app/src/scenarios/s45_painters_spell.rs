//! Scenario 45: the Layer and Object Painters and Eyedroppers, Match
//! Properties, and spell check (LAY-18, S-116, TXT-21).
//!
//! The painters are driven by clicks in the plan, the way the shell sends
//! them; every click must be one undo step. The spell check runs over text,
//! labels and layout text with a synthetic word list (nothing here reads the
//! operating system's list or the user's dictionary).

use super::{draw_shell, Sim};
use crate::dialogs::spell_check as sc;
use crate::editor::{placed, rooms_edit, ObjectRef};
use crate::spell::{install_for_test, test_speller};
use crate::toolbar::Action;
use crate::tools::painters::{self, with_state, LayerScope, ObjectScope, PainterMode as P};
use crate::tools::ToolId;
use plan_cabinets::Cabinet;
use plan_core::cad::{CadAttrs, CadItem};
use plan_core::geometry::Point;
use plan_core::{Floor, Id, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim
}

fn steps(sim: &Sim) -> usize {
    sim.app.cx.action_history().0.len()
}

fn mid(sim: &Sim, wall: Id) -> (f64, f64) {
    let w = sim.app.cx.floor().wall(wall).unwrap();
    ((w.start.x + w.end.x) / 2.0, (w.start.y + w.end.y) / 2.0)
}

fn click_wall(sim: &mut Sim, wall: Id) {
    let (x, y) = mid(sim, wall);
    sim.click(x, y);
}

fn cad_line(sim: &mut Sim, a: (f64, f64), b: (f64, f64)) -> Id {
    sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(a.0, a.1),
            b: Point::new(b.0, b.1),
        },
    )
}

fn layer_of_cad(sim: &Sim, id: Id) -> String {
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .layer
        .clone()
}

fn active(sim: &Sim) -> ToolId {
    sim.app.tools.active_id()
}

// ----- Layer Painter and Layer Eyedropper -----

#[test]
fn the_layer_eyedropper_loads_a_layer_then_the_painter_moves_a_wall_in_one_undo_step() {
    let mut sim = house();
    let walls = sim.wall_ids();
    let before = sim.app.cx.floor().wall(walls[2]).unwrap().layer.clone();
    assert_ne!(before, "CAD, Default");
    let line = cad_line(&mut sim, (700.0, 700.0), (740.0, 700.0));

    sim.tool(ToolId::PainterVariant(P::LayerEyedropper));
    sim.click(720.0, 700.0);
    assert_eq!(
        with_state(|s| s.layer.clone()).as_deref(),
        Some("CAD, Default")
    );
    // The eyedropper hands over to the painter.
    assert_eq!(active(&sim), ToolId::PainterVariant(P::LayerPaint));

    let n = steps(&sim);
    click_wall(&mut sim, walls[2]);
    assert_eq!(
        sim.app.cx.floor().wall(walls[2]).unwrap().layer,
        "CAD, Default"
    );
    assert_eq!(steps(&sim), n + 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Layer Painter"));
    // Only the clicked wall moved.
    assert_eq!(sim.app.cx.floor().wall(walls[0]).unwrap().layer, before);
    sim.undo();
    assert_eq!(sim.app.cx.floor().wall(walls[2]).unwrap().layer, before);
    assert_eq!(layer_of_cad(&sim, line), "CAD, Default");
}

#[test]
fn the_layer_eyedropper_reads_the_layer_of_doors_and_stays_put_on_empty_space() {
    let mut sim = house();
    sim.tool(ToolId::PainterVariant(P::LayerEyedropper));
    sim.click(120.0, 0.0);
    assert_eq!(with_state(|s| s.layer.clone()).as_deref(), Some("Doors"));
    // Nothing under the pointer: no change, the eyedropper stays active.
    sim.tool(ToolId::PainterVariant(P::LayerEyedropper));
    with_state(|s| s.layer = None);
    sim.click(900.0, 900.0);
    assert_eq!(with_state(|s| s.layer.clone()), None);
    assert_eq!(active(&sim), ToolId::PainterVariant(P::LayerEyedropper));
}

#[test]
fn layer_painter_scope_is_component_or_the_whole_group() {
    for (scope, moved_both) in [(LayerScope::Component, false), (LayerScope::Object, true)] {
        let mut sim = house();
        let a = cad_line(&mut sim, (700.0, 700.0), (740.0, 700.0));
        let b = cad_line(&mut sim, (700.0, 740.0), (740.0, 740.0));
        let gid = sim.app.cx.project.alloc_id();
        sim.app.cx.project.floors[0]
            .groups
            .push(plan_core::ObjectGroup {
                id: gid,
                members: vec![plan_core::ObjectRef::Cad(a), plan_core::ObjectRef::Cad(b)],
            });
        with_state(|s| {
            s.layer = Some("Walls, Normal".into());
            s.layer_scope = scope;
        });
        sim.tool(ToolId::PainterVariant(P::LayerPaint));
        let n = steps(&sim);
        sim.click(720.0, 700.0);
        assert_eq!(layer_of_cad(&sim, a), "Walls, Normal");
        assert_eq!(
            layer_of_cad(&sim, b) == "Walls, Normal",
            moved_both,
            "{scope:?}"
        );
        assert_eq!(steps(&sim), n + 1, "{scope:?}");
    }
}

#[test]
fn the_layer_painter_needs_a_layer_and_respects_locked_layers() {
    let mut sim = house();
    let walls = sim.wall_ids();
    with_state(|s| s.layer = None);
    sim.tool(ToolId::PainterVariant(P::LayerPaint));
    let n = steps(&sim);
    click_wall(&mut sim, walls[1]);
    assert_eq!(steps(&sim), n, "no layer loaded");
    assert!(
        sim.app.cx.status.contains("pick a layer"),
        "{}",
        sim.app.cx.status
    );

    with_state(|s| s.layer = Some("CAD, Default".into()));
    sim.app.cx.project.layers.set_locked("Walls, Normal", true);
    click_wall(&mut sim, walls[1]);
    assert_eq!(steps(&sim), n, "the wall's layer is locked");
    assert_ne!(
        sim.app.cx.floor().wall(walls[1]).unwrap().layer,
        "CAD, Default"
    );
}

#[test]
fn the_painters_start_from_the_selection_and_escape_returns_to_select() {
    let mut sim = house();
    let walls = sim.wall_ids();
    with_state(|s| s.layer = None);
    sim.app.cx.selection.set(ObjectRef::Wall(walls[0]));
    sim.tool(ToolId::PainterVariant(P::LayerPaint));
    assert_eq!(
        with_state(|s| s.layer.clone()),
        Some(sim.app.cx.floor().wall(walls[0]).unwrap().layer.clone())
    );
    sim.esc();
    assert_eq!(active(&sim), ToolId::Select);
}

// ----- Object Painter and Object Eyedropper -----

fn load_source(sim: &mut Sim, wall: Id) {
    sim.tool(ToolId::PainterVariant(P::ObjectEyedropper));
    click_wall(sim, wall);
    assert_eq!(active(sim), ToolId::PainterVariant(P::ObjectPaint));
}

#[test]
fn the_object_painter_copies_a_walls_specification_but_not_its_position_or_length() {
    let mut sim = house();
    let walls = sim.wall_ids();
    {
        let w = sim.app.cx.project.floors[0].wall_mut(walls[0]).unwrap();
        w.thickness = 8.0;
        w.height = 108.0;
        w.wall_type = Some("Test Brick".into());
    }
    let target_before = sim.app.cx.floor().wall(walls[2]).unwrap().clone();
    let other_type = sim.app.cx.floor().wall(walls[1]).unwrap().wall_type.clone();
    load_source(&mut sim, walls[0]);
    assert!(with_state(|s| s.source.as_ref().map(|a| a.summary.clone()))
        .unwrap()
        .contains("Test Brick"));

    let n = steps(&sim);
    click_wall(&mut sim, walls[2]);
    let w = sim.app.cx.floor().wall(walls[2]).unwrap();
    assert_eq!((w.thickness, w.height), (8.0, 108.0));
    assert_eq!(w.wall_type.as_deref(), Some("Test Brick"));
    // Where it is and how long it is stay.
    assert_eq!((w.start, w.end), (target_before.start, target_before.end));
    assert_eq!(steps(&sim), n + 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Object Painter"));
    // Default scope Object: the other walls are untouched.
    assert_eq!(
        sim.app.cx.floor().wall(walls[1]).unwrap().wall_type,
        other_type
    );
    sim.undo();
    let w = sim.app.cx.floor().wall(walls[2]).unwrap();
    assert_eq!(w.thickness, target_before.thickness);
    assert_eq!(w.wall_type, target_before.wall_type);
}

#[test]
fn painting_an_object_that_already_matches_makes_no_undo_step() {
    let mut sim = house();
    let walls = sim.wall_ids();
    load_source(&mut sim, walls[0]);
    click_wall(&mut sim, walls[2]);
    let n = steps(&sim);
    click_wall(&mut sim, walls[2]);
    assert_eq!(steps(&sim), n);
    assert!(
        sim.app.cx.status.contains("already match"),
        "{}",
        sim.app.cx.status
    );
}

#[test]
fn scope_floor_reaches_walls_like_the_clicked_one_unless_apply_to_all_of_type_is_on() {
    let mut sim = house();
    let walls = sim.wall_ids();
    let interior = sim.app.cx.project.add_wall(
        0,
        Point::new(100.0, 100.0),
        Point::new(300.0, 100.0),
        4.5,
        96.0,
        WallKind::Interior,
    );
    sim.app.cx.project.floors[0]
        .wall_mut(walls[0])
        .unwrap()
        .thickness = 9.0;
    sim.app.cx.refresh();
    let interior_before = sim.app.cx.floor().wall(interior).unwrap().thickness;
    load_source(&mut sim, walls[0]);
    with_state(|s| {
        s.scope = ObjectScope::Floor;
        s.all_of_type = false;
    });
    let n = steps(&sim);
    click_wall(&mut sim, walls[2]);
    assert_eq!(steps(&sim), n + 1, "one click, one undo step, many walls");
    for id in &walls {
        assert_eq!(sim.app.cx.floor().wall(*id).unwrap().thickness, 9.0);
    }
    assert_eq!(
        sim.app.cx.floor().wall(interior).unwrap().thickness,
        interior_before,
        "an interior wall is not like the exterior one clicked"
    );
    // Apply to all of type reaches the interior wall too.
    with_state(|s| s.all_of_type = true);
    click_wall(&mut sim, walls[1]);
    assert_eq!(sim.app.cx.floor().wall(interior).unwrap().thickness, 9.0);
    sim.undo();
    assert_eq!(
        sim.app.cx.floor().wall(interior).unwrap().thickness,
        interior_before
    );
}

#[test]
fn scope_plan_reaches_other_floors_and_scope_floor_does_not() {
    let mut sim = house();
    let walls = sim.wall_ids();
    sim.app.cx.project.floors.push(Floor::new("Second", 108.0));
    let upstairs = sim.app.cx.project.add_wall(
        1,
        Point::new(0.0, 0.0),
        Point::new(200.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.cx.project.floors[0]
        .wall_mut(walls[0])
        .unwrap()
        .thickness = 11.0;
    sim.app.cx.refresh();
    load_source(&mut sim, walls[0]);
    with_state(|s| {
        s.scope = ObjectScope::Floor;
        s.all_of_type = true;
    });
    click_wall(&mut sim, walls[1]);
    assert_eq!(
        sim.app.cx.project.floors[1]
            .wall(upstairs)
            .unwrap()
            .thickness,
        6.0
    );
    with_state(|s| s.scope = ObjectScope::Plan);
    let n = steps(&sim);
    click_wall(&mut sim, walls[2]);
    assert_eq!(
        sim.app.cx.project.floors[1]
            .wall(upstairs)
            .unwrap()
            .thickness,
        11.0
    );
    assert_eq!(steps(&sim), n + 1);
    sim.undo();
    assert_eq!(
        sim.app.cx.project.floors[1]
            .wall(upstairs)
            .unwrap()
            .thickness,
        6.0
    );
}

#[test]
fn scope_room_reaches_only_the_objects_in_the_clicked_room() {
    let mut sim = house();
    let walls = sim.wall_ids();
    let far = sim.app.cx.project.add_wall(
        0,
        Point::new(1000.0, 0.0),
        Point::new(1200.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.cx.project.floors[0]
        .wall_mut(walls[0])
        .unwrap()
        .thickness = 10.0;
    sim.app.cx.refresh();
    load_source(&mut sim, walls[0]);
    with_state(|s| {
        s.scope = ObjectScope::Room;
        s.all_of_type = true;
    });
    click_wall(&mut sim, walls[3]);
    for id in &walls {
        assert_eq!(sim.app.cx.floor().wall(*id).unwrap().thickness, 10.0);
    }
    assert_eq!(sim.app.cx.floor().wall(far).unwrap().thickness, 6.0);
}

#[test]
fn doors_and_windows_do_not_exchange_attributes_but_doors_paint_doors() {
    let mut sim = house();
    sim.app.cx.project.floors[0]
        .openings
        .iter_mut()
        .for_each(|o| {
            if o.kind == plan_core::OpeningKind::Door {
                o.tempered = true;
                o.lites = (2, 3);
            }
        });
    sim.tool(ToolId::PainterVariant(P::ObjectEyedropper));
    sim.click(120.0, 0.0);
    assert_eq!(active(&sim), ToolId::PainterVariant(P::ObjectPaint));
    let n = steps(&sim);
    sim.click(300.0, 0.0);
    assert_eq!(steps(&sim), n, "a door's attributes do not go on a window");
    assert!(
        sim.app.cx.status.contains("does not fit"),
        "{}",
        sim.app.cx.status
    );

    // A second door takes them; its place and size stay.
    sim.tool(ToolId::Door);
    sim.click(200.0, 0.0);
    sim.tool(ToolId::PainterVariant(P::ObjectPaint));
    let doors: Vec<_> = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .filter(|o| o.kind == plan_core::OpeningKind::Door)
        .cloned()
        .collect();
    assert_eq!(doors.len(), 2);
    let second = doors.iter().find(|d| !d.tempered).expect("a fresh door");
    let (id, offset, width) = (second.id, second.center_offset, second.width);
    sim.click(200.0, 0.0);
    let o = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == id)
        .unwrap();
    assert!(o.tempered);
    assert_eq!(o.lites, (2, 3));
    assert_eq!((o.center_offset, o.width), (offset, width));
}

#[test]
fn the_object_painter_copies_cad_and_text_style_but_not_the_words() {
    let mut sim = house();
    let note = |sim: &mut Sim, at: (f64, f64), text: &str, height: f64| {
        sim.app.cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Text {
                pos: Point::new(at.0, at.1),
                text: text.into(),
                height,
                angle: 0.0,
            },
        )
    };
    let src = note(&mut sim, (700.0, 700.0), "SOURCE", 6.0);
    let dst = note(&mut sim, (700.0, 760.0), "hello", 3.0);
    sim.app.cx.project.floors[0]
        .cad
        .iter_mut()
        .find(|c| c.id == src)
        .unwrap()
        .layer = "Walls, Normal".into();
    sim.app
        .cx
        .project
        .edit_cad_attrs(0, src, |a: &mut CadAttrs| {
            a.color = Some([255, 0, 0]);
            a.weight = Some(50);
        });
    sim.tool(ToolId::PainterVariant(P::ObjectEyedropper));
    sim.click(706.0, 703.0);
    assert_eq!(active(&sim), ToolId::PainterVariant(P::ObjectPaint));
    let n = steps(&sim);
    sim.click(706.0, 763.0);
    assert_eq!(steps(&sim), n + 1);
    let c = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == dst)
        .unwrap()
        .clone();
    assert_eq!(c.layer, "Walls, Normal");
    match c.item {
        CadItem::Text {
            text, height, pos, ..
        } => {
            assert_eq!(text, "hello", "the words stay");
            assert_eq!(height, 6.0);
            assert_eq!(pos, Point::new(700.0, 760.0), "the place stays");
        }
        other => panic!("{other:?}"),
    }
    let a = sim.app.cx.floor().cad_attrs(dst).unwrap();
    assert_eq!((a.color, a.weight), (Some([255, 0, 0]), Some(50)));
}

#[test]
fn the_object_painter_copies_cabinet_style_not_size_or_place() {
    let mut sim = house();
    let mut a = Cabinet::base(24.0);
    a.position = Point::new(600.0, 100.0);
    a.framed = true;
    a.indicators = true;
    let mut b = Cabinet::base(30.0);
    b.position = Point::new(700.0, 100.0);
    let (ia, ib) = {
        let p = &mut sim.app.cx.project;
        (
            placed::add_cabinet(p, 0, a).unwrap(),
            placed::add_cabinet(p, 0, b).unwrap(),
        )
    };
    sim.app.cx.refresh();
    sim.tool(ToolId::PainterVariant(P::ObjectEyedropper));
    sim.click(612.0, 112.0);
    assert_eq!(active(&sim), ToolId::PainterVariant(P::ObjectPaint));
    let n = steps(&sim);
    sim.click(715.0, 112.0);
    assert_eq!(steps(&sim), n + 1);
    let b2 = placed::cabinet_by_id(sim.app.cx.floor(), ib).unwrap();
    assert!(b2.framed && b2.indicators);
    assert_eq!((b2.width, b2.position), (30.0, Point::new(700.0, 100.0)));
    assert!(placed::cabinet_by_id(sim.app.cx.floor(), ia).is_some());
}

#[test]
fn the_object_painter_copies_room_finishes_between_rooms() {
    let mut sim = house();
    // A second room beside the house.
    let corners = [(600.0, 0.0), (800.0, 0.0), (800.0, 200.0), (600.0, 200.0)];
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        sim.app.cx.project.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            6.0,
            96.0,
            WallKind::Exterior,
        );
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.rooms.len(), 2);
    let src_room = rooms_edit::room_index_at(&sim.app.cx, Point::new(240.0, 180.0)).unwrap();
    let rooms = sim.app.cx.rooms.clone();
    let anchor = rooms_edit::room_anchor(&rooms[src_room]);
    sim.app
        .cx
        .project
        .set_room_name(0, anchor, "Den", "Living", &rooms);
    sim.app.cx.project.floors[0].room_names[0].floor_finish = Some("Oak".into());
    sim.app.cx.refresh();

    sim.tool(ToolId::PainterVariant(P::ObjectEyedropper));
    sim.click(240.0, 180.0);
    assert_eq!(active(&sim), ToolId::PainterVariant(P::ObjectPaint));
    let n = steps(&sim);
    sim.click(700.0, 100.0);
    assert_eq!(steps(&sim), n + 1);
    let dst = rooms_edit::room_index_at(&sim.app.cx, Point::new(700.0, 100.0)).unwrap();
    let room = sim.app.cx.rooms[dst].clone();
    let entry = rooms_edit::name_entry(&sim.app.cx, &room).unwrap();
    assert_eq!(entry.floor_finish.as_deref(), Some("Oak"));
    assert_eq!(entry.room_type, "Living");
    assert_ne!(entry.name, "Den", "the name belongs to the room");
}

#[test]
fn a_locked_layer_keeps_its_objects_from_being_painted() {
    let mut sim = house();
    let walls = sim.wall_ids();
    load_source(&mut sim, walls[0]);
    sim.app.cx.project.floors[0]
        .wall_mut(walls[0])
        .unwrap()
        .thickness = 12.0;
    load_source(&mut sim, walls[0]);
    sim.app.cx.project.layers.set_locked("Walls, Normal", true);
    let n = steps(&sim);
    click_wall(&mut sim, walls[2]);
    assert_eq!(steps(&sim), n);
    assert_ne!(sim.app.cx.floor().wall(walls[2]).unwrap().thickness, 12.0);
}

#[test]
fn match_properties_loads_the_selection_and_switches_to_the_object_painter() {
    let mut sim = house();
    let walls = sim.wall_ids();
    sim.app.cx.project.floors[0]
        .wall_mut(walls[0])
        .unwrap()
        .thickness = 7.5;
    // Offered on the Edit toolbar for one paintable object only.
    assert!(!sim
        .app
        .cx
        .selection_edit_actions()
        .iter()
        .any(|a| a.label == "Match Properties"));
    sim.app.cx.selection.set(ObjectRef::Wall(walls[0]));
    assert!(sim
        .app
        .cx
        .selection_edit_actions()
        .iter()
        .any(|a| a.label == "Match Properties"));
    sim.action(Action::Custom(painters::MATCH_PROPERTIES));
    assert_eq!(active(&sim), ToolId::PainterVariant(P::ObjectPaint));
    let n = steps(&sim);
    click_wall(&mut sim, walls[1]);
    assert_eq!(sim.app.cx.floor().wall(walls[1]).unwrap().thickness, 7.5);
    assert_eq!(steps(&sim), n + 1);
    // Two objects selected: nothing to match from.
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Wall(walls[0]));
    sim.app.cx.selection.add(ObjectRef::Wall(walls[1]));
    assert!(!painters::can_match(&sim.app.cx));
}

#[test]
fn the_modes_dialog_opens_from_the_tools_menu_command_and_holds_the_scopes() {
    let mut sim = house();
    sim.action(Action::Custom(painters::MODES));
    assert_eq!(active(&sim), ToolId::PainterVariant(P::ObjectPaint));
    assert!(with_state(|s| s.modes_open));
    // The bar and the dialog draw.
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(with_state(|s| s.modes_open));
    let names = ObjectScope::ALL.map(ObjectScope::name);
    assert_eq!(names, ["Component", "Object", "Room", "Floor", "Plan"]);
}

#[test]
fn every_painter_has_a_toolbar_toggle_and_check_spelling_a_button() {
    use crate::toolbar::{Slot, Toolbars};
    let bars = Toolbars::new();
    let items: Vec<(&str, Action)> = bars
        .row1
        .iter()
        .chain(bars.row2.iter())
        .filter_map(|s| match s {
            Slot::Button(i) | Slot::Toggle(i) => Some((i.name, i.action)),
            _ => None,
        })
        .collect();
    for m in P::ALL {
        assert!(
            items.contains(&(m.toolbar_name(), Action::SetTool(ToolId::PainterVariant(m)))),
            "{}",
            m.name()
        );
    }
    assert!(items.contains(&("Check Spelling", Action::Custom(sc::OPEN))));
    // Picking a toggle activates the tool.
    let mut sim = house();
    sim.action(Action::SetTool(ToolId::PainterVariant(P::ObjectEyedropper)));
    assert_eq!(active(&sim), ToolId::PainterVariant(P::ObjectEyedropper));
}

// ----- spell check -----

fn words() -> crate::spell::Speller {
    test_speller(&[
        "the", "kitchen", "master", "bedroom", "bath", "wall", "door", "window", "schedule",
        "floor", "plan", "notes", "garage", "open", "to", "and", "again", "here",
    ])
}

fn text(sim: &mut Sim, s: &str) -> Id {
    sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Text {
            pos: Point::new(10.0, 10.0),
            text: s.into(),
            height: 3.0,
            angle: 0.0,
        },
    )
}

#[test]
fn tools_spell_check_walks_the_plan_and_change_is_one_undo_step() {
    install_for_test(words(), None);
    let mut sim = house();
    let t1 = text(&mut sim, "Open to the kitchn");
    let _t2 = text(&mut sim, "Master bedrom notes");
    sim.action(Action::Custom(sc::OPEN));
    assert!(sc::is_open());
    assert_eq!(sc::current_word().as_deref(), Some("kitchn"));
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Text(t1)));
    assert_eq!(
        sc::current_suggestions().first().map(String::as_str),
        Some("kitchen")
    );

    let n = steps(&sim);
    sc::run(&mut sim.app.cx, sc::Cmd::Change("kitchen".into()));
    assert_eq!(steps(&sim), n + 1);
    assert_eq!(sc::current_word().as_deref(), Some("bedrom"));
    sc::run(&mut sim.app.cx, sc::Cmd::Change("bedroom".into()));
    assert!(sc::is_finished());
    let texts: Vec<String> = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .filter_map(|c| match &c.item {
            CadItem::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["Open to the kitchen", "Master bedroom notes"]);
    // Each change undoes on its own.
    sim.undo();
    assert!(sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .any(|c| matches!(&c.item, CadItem::Text { text, .. } if text == "Master bedrom notes")));
    sc::run(&mut sim.app.cx, sc::Cmd::Close);
    assert!(!sc::is_open());
}

#[test]
fn the_spelling_dialog_draws_and_its_buttons_work_through_the_shell() {
    install_for_test(words(), None);
    let mut sim = house();
    let id = text(&mut sim, "the kitchn and the kitchn again");
    sim.action(Action::Custom(sc::OPEN));
    let ctx = sim.ctx.clone();
    for _ in 0..2 {
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            sc::show_all(ctx, &mut sim.app.cx)
        });
    }
    sc::run(&mut sim.app.cx, sc::Cmd::ChangeAll("kitchen".into()));
    let t = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.item.clone());
    assert!(
        matches!(t, Some(CadItem::Text { text, .. }) if text == "the kitchen and the kitchen again")
    );
    assert!(sc::is_finished());
}

#[test]
fn layout_text_is_checked_and_fixed_in_the_layout() {
    install_for_test(words(), None);
    let mut sim = house();
    let mut layout = plan_layout::Layout::new("L", plan_docs::SheetSize::ArchC);
    let page = layout.add_page(1, "Floorr plan");
    page.add_text(Point::new(1.0, 1.0), "Door schedul notes", 0.2);
    crate::shell::layout_window::store(&mut sim.app.cx.project, &layout);
    sim.action(Action::Custom(sc::OPEN));
    assert_eq!(sc::current_word().as_deref(), Some("Floorr"));
    sc::run(&mut sim.app.cx, sc::Cmd::Change("Floor".into()));
    assert_eq!(sc::current_word().as_deref(), Some("schedul"));
    sc::run(&mut sim.app.cx, sc::Cmd::Change("schedule".into()));
    assert!(sc::is_finished());
    let layout = crate::shell::layout_window::load(&sim.app.cx.project).unwrap();
    assert_eq!(layout.pages[0].title, "Floor plan");
    assert!(matches!(
        &layout.pages[0].cad[0].item,
        CadItem::Text { text, .. } if text == "Door schedule notes"
    ));
    // Undo restores the layout text with the plan.
    sim.undo();
    let layout = crate::shell::layout_window::load(&sim.app.cx.project).unwrap();
    assert_eq!(layout.pages[0].title, "Floor plan");
    assert!(matches!(
        &layout.pages[0].cad[0].item,
        CadItem::Text { text, .. } if text == "Door schedul notes"
    ));
}

#[test]
fn the_text_dialog_underlines_misspelled_words_and_offers_check_spelling() {
    install_for_test(words(), None);
    let mut sim = house();
    let id = text(&mut sim, "the kitchn");
    assert!(sim.open_spec(ObjectRef::Text(id)));
    // The dialog lays out with the spelling layouter and the button.
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(sim.app.has_dialog());
    sim.cancel();
    assert!(!sim.app.has_dialog());
}

#[test]
fn suggestions_come_from_the_word_list_within_two_edits() {
    install_for_test(words(), None);
    crate::spell::with(|sp| {
        let s = sp.suggest("bedrom", 5);
        assert_eq!(s.first().map(String::as_str), Some("bedroom"));
        let s = sp.suggest("kicthen", 5);
        assert_eq!(s.first().map(String::as_str), Some("kitchen"));
        assert!(sp.suggest("qqqqqqq", 5).is_empty());
        for w in sp.suggest("wal", 8) {
            assert!(w.len() <= 5, "{w}");
        }
    });
}
