//! Lesson 12, Room Moldings (pp. 211-222): the floor's molding table, the Exterior Room
//! reached with Tab, a room's moldings made into editable polylines, and the edge
//! commands on one of those polylines.
use crate::editor::details_view as dv;
use crate::editor::rooms_edit;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::{draw_shell, Sim};
use crate::tools::molding as mold;
use crate::tools::KeyEvent;
use eframe::egui::Key;
use plan_core::geometry::Point;
use plan_core::moldings::{builtin_profiles, MoldingTable, MoldingType};
use plan_core::RoomName;

const ANCHOR: Point = Point { x: 120.0, y: 96.0 };

fn house() -> Sim {
    mold::reset_active_profile();
    let mut sim = Sim::new();
    draw_shell(&mut sim, 240.0, 192.0);
    sim.cx()
        .floor_mut()
        .room_names
        .push(RoomName::new(ANCHOR, "Den", "Den"));
    sim.app.cx.refresh();
    sim
}

fn profile(kind: MoldingType) -> plan_core::moldings::ProfileDef {
    builtin_profiles()
        .into_iter()
        .find(|p| p.kind == kind)
        .unwrap()
}

#[test]
fn floor_defaults_replace_the_base_and_add_a_crown_at_three_inches() {
    let mut sim = house();
    let mut table = MoldingTable::single(profile(MoldingType::Base));
    let i = table.add_new(profile(MoldingType::Crown));
    table.rows[i].set_height(3.0);
    assert_one_undo_step(&mut sim, "Floor Defaults Moldings", |s| {
        mold::set_floor_molding_table(s.cx(), table.clone());
    });
    let back = mold::floor_molding_table(&sim.app.cx);
    assert_eq!(back.len(), 2);
    assert_eq!(back.rows[0].kind, MoldingType::Base);
    assert_eq!(back.rows[1].kind, MoldingType::Crown);
    assert_eq!(back.rows[1].height, 3.0);
}

#[test]
fn tab_after_the_wall_reaches_the_exterior_room() {
    let mut sim = cottage();
    sim.move_to(240.0, -2.0);
    sim.app.cx.cursor_world = Some(Point::new(240.0, -2.0));
    sim.key(KeyEvent::key(Key::Tab));
    assert!(sim.app.cx.selection.single().is_some(), "the wall first");
    sim.key(KeyEvent::key(Key::Tab));
    assert_eq!(rooms_edit::selected_exterior(&sim.app.cx), Some(0));
}

#[test]
fn make_room_molding_polyline_converts_the_room_moldings_in_one_undo_step() {
    let mut sim = house();
    let mut table = MoldingTable::single(profile(MoldingType::Base));
    table.add_new(profile(MoldingType::Crown));
    mold::set_floor_molding_table(sim.cx(), table);
    sim.app.cx.refresh();
    assert!(dv::load(&sim.app.cx).moldings.is_empty());
    let n = std::cell::Cell::new(0);
    assert_one_undo_step(&mut sim, "Make Room Molding Polyline", |s| {
        n.set(mold::make_room_molding_polylines(s.cx(), ANCHOR));
    });
    assert_eq!(n.get(), 2, "one polyline per molding row");
    let layer = dv::load(&sim.app.cx);
    assert_eq!(layer.moldings.len(), 2);
    assert!(layer.moldings.iter().all(|m| m.is_closed()));
}

#[test]
fn remove_and_add_molding_on_the_selected_edge_are_one_step_each() {
    let mut sim = house();
    mold::set_floor_molding_table(sim.cx(), MoldingTable::single(profile(MoldingType::Crown)));
    sim.app.cx.refresh();
    assert!(mold::make_room_molding_polylines(sim.cx(), ANCHOR) >= 1);
    let id = dv::load(&sim.app.cx).moldings[0].id;
    assert!(dv::load(&sim.app.cx).molding(id).unwrap().edge_on(0));
    assert_one_undo_step(&mut sim, "Remove Molding from Selected Edge", |s| {
        assert!(mold::set_edge(s.cx(), id, 0, false));
    });
    assert!(!dv::load(&sim.app.cx).molding(id).unwrap().edge_on(0));
    assert_one_undo_step(&mut sim, "Add Molding to Selected Edge", |s| {
        assert!(mold::set_edge(s.cx(), id, 0, true));
    });
    assert!(dv::load(&sim.app.cx).molding(id).unwrap().edge_on(0));
}

#[test]
#[ignore = "T7-12: S-170 (Extension Snap while drawing a molding polyline; S/E markers on the selected edge)"]
fn extension_snap_and_edge_markers() {
    assert_ignored_break("S-170");
}
