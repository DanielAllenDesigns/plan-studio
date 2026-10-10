//! Lesson 14, Cabinet Styles (pp. 237-260).
use crate::editor::placed::load_cabinets;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::Sim;
use crate::tools::ToolId;
use plan_cabinets::CabinetKind;

fn place(sim: &mut Sim, kind: CabinetKind, x: f64, y: f64) {
    sim.tool(ToolId::CabinetVariant(kind));
    sim.app.cx.selection.clear();
    sim.click(x, y);
}

#[test]
fn base_wall_and_full_height_cabinets_are_one_undo_step_each() {
    let mut sim = cottage();
    for (kind, x) in [
        (CabinetKind::Base, 60.0),
        (CabinetKind::Wall, 140.0),
        (CabinetKind::FullHeight, 300.0),
    ] {
        assert_one_undo_step(&mut sim, "cabinet", |s| place(s, kind, x, 20.0));
    }
    let cabs = load_cabinets(sim.app.cx.floor());
    assert_eq!(cabs.len(), 3);
    // Before any Apply Properties the three share the template's styles.
    assert!(cabs.iter().all(|c| c.door_style == cabs[0].door_style));
}

#[test]
#[ignore = "T7-14: CB-395 (library door style and bar pull; Apply Properties to the other cabinets)"]
fn door_style_and_pull_from_the_library() {
    assert_ignored_break("CB-395");
}

#[test]
#[ignore = "T7-14: DS-27 (Set as Default changes the next cabinet)"]
fn set_as_default() {
    assert_ignored_break("DS-27");
}

#[test]
#[ignore = "T7-14: CB-631 (crown molding on a wall cabinet From Ceiling)"]
fn crown_from_ceiling() {
    assert_ignored_break("CB-631");
}

#[test]
#[ignore = "T7-14: CB-632 (secondary drawer style from the library)"]
fn secondary_drawer_style() {
    assert_ignored_break("CB-632");
}

#[test]
#[ignore = "T7-14: CB-344 (Split Vertical 10 / 22 / 10 widths summing to 48)"]
fn split_vertical_face_items() {
    assert_ignored_break("CB-344");
}

#[test]
#[ignore = "T7-14: DS-26 (feet)"]
fn feet() {
    assert_ignored_break("DS-26");
}
