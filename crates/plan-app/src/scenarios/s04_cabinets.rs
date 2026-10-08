//! Scenario 4: base and wall cabinets along a wall of the shell: they sit
//! against the wall face, butt against each other, open the Cabinet
//! Specification on double-click and delete / undo (CB-1..CB-9, CB-20).

use super::{draw_shell, Sim};
use crate::editor::placed::{self, apply_cabinet, load_cabinets, same_angle};
use crate::editor::{EditorRequest, ObjectRef};
use crate::tools::ToolId;
use plan_cabinets::{Cabinet, CabinetKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn cabs(sim: &Sim) -> Vec<Cabinet> {
    load_cabinets(sim.app.cx.floor())
}

/// Half the exterior wall thickness: the south wall's inner face is here.
fn face() -> f64 {
    7.625 / 2.0
}

fn place(sim: &mut Sim, kind: CabinetKind, x: f64, y: f64) {
    sim.tool(ToolId::CabinetVariant(kind));
    sim.app.cx.selection.clear();
    sim.click(x, y);
}

#[test]
fn a_base_cabinet_sits_flush_against_the_wall_it_is_clicked_near() {
    let mut sim = house();
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    assert_eq!(sim.app.tools.active().name(), "Base Cabinet");
    // 9" inside the south wall's centerline: within the 12" reach (CB-3).
    let r = sim.click(100.0, 10.0);
    assert_eq!(r.commit.as_deref(), Some("Place Base Cabinet"));
    let c = &cabs(&sim)[0];
    let d = sim.app.cx.defaults.cabinets.base.clone();
    // CB-6, CB-20: the size is the Default Settings base cabinet.
    assert_eq!((c.width, c.depth, c.height), (d.width, d.depth, d.height));
    // Back against the wall's inner face, front into the room.
    assert!(same_angle(c.angle, 0.0), "angle {}", c.angle);
    assert!((c.position.y - face()).abs() < 1e-6, "{:?}", c.position);
    // Centered on the click.
    assert!((c.position.x + c.width / 2.0 - 100.0).abs() <= 1.0);
    // All of it is inside the room.
    assert!(c.corners().iter().all(|p| p.y >= face() - 1e-6));
    // The new cabinet is selected.
    assert_eq!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::Cabinet(c.id))
    );
}

#[test]
fn a_second_cabinet_butts_against_the_first_and_aligns_the_back() {
    let mut sim = house();
    place(&mut sim, CabinetKind::Base, 100.0, 10.0);
    // Click overlapping the first cabinet: it slides to butt against it (CB-4).
    place(&mut sim, CabinetKind::Base, 108.0, 12.0);
    let list = cabs(&sim);
    assert_eq!(list.len(), 2);
    let (a, b) = (&list[0], &list[1]);
    let gap = b.position.x - (a.position.x + a.width);
    assert!(gap.abs() < 1e-6, "gap {gap}");
    assert!((b.position.y - a.position.y).abs() < 1e-6, "backs aligned");
    // A third one to the left butts the other way round.
    place(&mut sim, CabinetKind::Base, 84.0, 10.0);
    let c = &cabs(&sim)[2];
    let gap = a.position.x - (c.position.x + c.width);
    assert!(gap.abs() < 1e-6, "left gap {gap}");
    // One undo step each.
    assert_eq!(sim.undo().as_deref(), Some("Place Base Cabinet"));
    assert_eq!(cabs(&sim).len(), 2);
}

#[test]
fn wall_cabinets_hang_above_the_bases_on_the_same_wall() {
    let mut sim = house();
    place(&mut sim, CabinetKind::Base, 100.0, 10.0);
    place(&mut sim, CabinetKind::Wall, 100.0, 10.0);
    let list = cabs(&sim);
    assert_eq!(list.len(), 2);
    let (base, wall) = (&list[0], &list[1]);
    assert_eq!(wall.kind, CabinetKind::Wall);
    // It did not bump sideways into the base cabinet below it.
    assert!((wall.position.x + wall.width / 2.0 - 100.0).abs() <= 1.0);
    assert!(
        wall.elevation >= base.height,
        "{} vs {}",
        wall.elevation,
        base.height
    );
    assert!(wall.depth < base.depth, "wall cabinets are shallower");
    assert!((wall.position.y - face()).abs() < 1e-6, "back on the wall");
    // A second wall cabinet butts against the first.
    place(&mut sim, CabinetKind::Wall, 105.0, 10.0);
    let w2 = cabs(&sim)[2].clone();
    assert!((w2.position.x - (wall.position.x + wall.width)).abs() < 1e-6);
}

#[test]
fn a_cabinet_on_the_east_wall_turns_to_face_into_the_room() {
    let mut sim = house();
    // East wall is at x = 481; click 9" inside it.
    place(&mut sim, CabinetKind::Base, W + 1.0 - 10.0, 180.0);
    let c = cabs(&sim)[0].clone();
    assert!(!same_angle(c.angle, 0.0), "the cabinet rotated to the wall");
    let max_x = c.corners().iter().map(|p| p.x).fold(f64::MIN, f64::max);
    assert!(
        (max_x - (W + 1.0 - face())).abs() < 1e-6,
        "back on the east face, max x {max_x}"
    );
    let min_x = c.corners().iter().map(|p| p.x).fold(f64::MAX, f64::min);
    assert!(min_x < max_x - c.depth + 1e-6 + 0.01);
}

#[test]
fn free_cabinets_away_from_walls_keep_angle_zero() {
    let mut sim = house();
    place(&mut sim, CabinetKind::Base, 240.0, 180.0);
    let c = &cabs(&sim)[0];
    assert_eq!(c.angle, 0.0);
}

#[test]
fn tab_cycles_through_the_cabinet_kinds_and_the_hint_names_the_tool() {
    let mut sim = house();
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    assert!(!sim.app.tools.active().hint().is_empty());
    sim.key(crate::tools::KeyEvent::key(eframe::egui::Key::Tab));
    assert_eq!(sim.app.tools.active().name(), "Wall Cabinet");
    sim.key(crate::tools::KeyEvent::key(eframe::egui::Key::Tab));
    assert_eq!(sim.app.tools.active().name(), "Full Height Cabinet");
}

#[test]
fn double_click_requests_the_cabinet_specification_and_the_dialog_opens() {
    let mut sim = house();
    place(&mut sim, CabinetKind::Base, 100.0, 10.0);
    let id = cabs(&sim)[0].id;
    sim.tool(ToolId::Select);
    sim.requests.clear();
    sim.double_click(100.0, 15.0);
    assert!(
        sim.requests
            .contains(&EditorRequest::OpenSpec(ObjectRef::Cabinet(id))),
        "{:?}",
        sim.requests
    );
    assert!(sim.app.spec.is_open());
    // The dialog draws two frames and OK closes it (one undo step).
    let before = sim.app.cx.undo_label().map(String::from);
    sim.ok();
    assert!(!sim.app.spec.is_open());
    assert_eq!(sim.app.cx.undo_label(), Some("Cabinet Specification"));
    assert_ne!(before.as_deref(), Some("Cabinet Specification"));
    // Cancelling another open leaves nothing behind.
    sim.double_click(100.0, 15.0);
    assert!(sim.app.spec.is_open());
    let steps = sim.app.cx.undo_label().map(String::from);
    sim.cancel();
    assert!(!sim.app.spec.is_open());
    assert_eq!(sim.app.cx.undo_label().map(String::from), steps);
}

#[test]
fn the_cabinet_specification_adjusts_the_cabinet_as_one_undo_step() {
    let mut sim = house();
    place(&mut sim, CabinetKind::Base, 100.0, 10.0);
    let orig = cabs(&sim)[0].clone();
    let mut draft = orig.clone();
    draft.width = 36.0;
    draft.height = 34.5;
    assert!(apply_cabinet(&mut sim.app.cx, &draft));
    let now = cabs(&sim)[0].clone();
    assert_eq!((now.width, now.height), (36.0, 34.5));
    assert_eq!(sim.app.cx.undo_label(), Some("Cabinet Specification"));
    assert_eq!(sim.undo().as_deref(), Some("Cabinet Specification"));
    let back = cabs(&sim)[0].clone();
    assert_eq!((back.width, back.height), (orig.width, orig.height));
    // The dialog preview opens for the stored cabinet and shows the draft.
    assert!(sim
        .app
        .spec
        .open(&mut sim.app.cx, ObjectRef::Cabinet(orig.id)));
}

#[test]
fn delete_removes_the_selected_cabinet_and_undo_brings_it_back() {
    let mut sim = house();
    place(&mut sim, CabinetKind::Base, 100.0, 10.0);
    place(&mut sim, CabinetKind::Base, 160.0, 10.0);
    assert_eq!(cabs(&sim).len(), 2);
    let keep = cabs(&sim)[0].id;
    sim.tool(ToolId::Select);
    sim.app
        .cx
        .selection
        .set(ObjectRef::Cabinet(cabs(&sim)[1].id));
    let r = sim.key(crate::tools::KeyEvent::key(eframe::egui::Key::Delete));
    assert_eq!(r.commit.as_deref(), Some("Delete"));
    let left = cabs(&sim);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, keep);
    assert_eq!(sim.undo().as_deref(), Some("Delete"));
    assert_eq!(cabs(&sim).len(), 2);
    assert_eq!(sim.redo().as_deref(), Some("Delete"));
    assert_eq!(cabs(&sim).len(), 1);
}

#[test]
fn cabinets_stay_put_and_selectable_when_the_wall_gets_an_opening() {
    let mut sim = house();
    place(&mut sim, CabinetKind::Base, 100.0, 10.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    assert_eq!(cabs(&sim).len(), 1);
    assert!(placed::exists(
        sim.app.cx.floor(),
        placed::PlacedRef::Cabinet(cabs(&sim)[0].id)
    ));
}
