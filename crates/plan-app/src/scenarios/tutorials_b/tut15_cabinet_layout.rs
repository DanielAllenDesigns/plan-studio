//! Lesson 15, Cabinet Layout (pp. 260-277). Real: a row of base cabinets
//! butts, fillers and wall cabinets place as one undo step each, the island
//! becomes one block, a label is suppressed and a door takes five manual
//! shelves. Open: Sticky Mode and the soffit crown wrap.
use crate::editor::placed::load_cabinets;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::Sim;
use crate::tools::ToolId;
use plan_cabinets::{Cabinet, CabinetKind};

fn place(sim: &mut Sim, kind: CabinetKind, x: f64, y: f64) {
    sim.tool(ToolId::CabinetVariant(kind));
    sim.app.cx.selection.clear();
    sim.click(x, y);
}

fn cabs(sim: &Sim) -> Vec<Cabinet> {
    load_cabinets(sim.app.cx.floor())
}

#[test]
fn a_row_of_base_cabinets_butts_and_takes_a_filler() {
    let mut sim = cottage();
    assert_one_undo_step(&mut sim, "first base", |s| {
        place(s, CabinetKind::Base, 60.0, 20.0)
    });
    let w = cabs(&sim)[0].width;
    for i in 1..4 {
        let x = 60.0 + w * i as f64;
        assert_one_undo_step(&mut sim, "base", |s| place(s, CabinetKind::Base, x, 20.0));
    }
    let mut row = cabs(&sim);
    assert_eq!(row.len(), 4);
    row.sort_by(|a, b| a.position.x.partial_cmp(&b.position.x).unwrap());
    for p in row.windows(2) {
        let gap = p[1].position.x - (p[0].position.x + p[0].width);
        assert!(gap.abs() < 1.0, "bump and push leaves no gap: {gap}");
        assert!(same_angle_eq(p[0].angle, p[1].angle));
    }
    let end = row[3].position.x + row[3].width;
    assert_one_undo_step(&mut sim, "filler", |s| {
        place(s, CabinetKind::BaseFiller, end + 1.5, 20.0)
    });
    assert!(cabs(&sim).iter().any(|c| c.kind == CabinetKind::BaseFiller));
}

fn same_angle_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn wall_cabinets_hang_above_the_base_row() {
    let mut sim = cottage();
    place(&mut sim, CabinetKind::Base, 60.0, 20.0);
    assert_one_undo_step(&mut sim, "wall cabinet", |s| {
        place(s, CabinetKind::Wall, 60.0, 20.0)
    });
    let all = cabs(&sim);
    let base = all.iter().find(|c| c.kind == CabinetKind::Base).unwrap();
    let wall = all.iter().find(|c| c.kind == CabinetKind::Wall).unwrap();
    assert!(wall.elevation >= base.elevation + base.height);
}

#[test]
#[ignore = "T7-15: S-137 (Sticky Mode: Copy + Sticky Mode + Reflect the drawer base twice)"]
fn sticky_mode_reflect() {
    assert_ignored_break("S-137");
}

#[test]
#[ignore = "T7-15: R-113 (room crown molding wraps the soffit above a full height cabinet)"]
fn soffit_crown_wraps() {
    assert_ignored_break("R-113");
}

#[test]
fn the_island_becomes_one_architectural_block_and_explodes_back() {
    let mut sim = cottage();
    for i in 0..3 {
        place(&mut sim, CabinetKind::Base, 100.0 + 24.0 * i as f64, 200.0);
    }
    sim.app.cx.selection.items = cabs(&sim)
        .iter()
        .map(|c| crate::editor::ObjectRef::Cabinet(c.id))
        .collect();
    assert_eq!(sim.app.cx.selection.len(), 3);
    assert_one_undo_step(&mut sim, "make block", |s| {
        s.action(crate::toolbar::Action::Custom(
            crate::tools::arch_block::MAKE_BLOCK,
        ))
    });
    assert_eq!(sim.app.cx.floor().blocks.len(), 1);
    assert_one_undo_step(&mut sim, "explode", |s| {
        s.action(crate::toolbar::Action::Custom(
            crate::tools::arch_block::EXPLODE_BLOCK,
        ))
    });
    assert_eq!(sim.app.cx.floor().blocks.len(), 0);
    assert_eq!(cabs(&sim).len(), 3, "the members survive Explode");
}

#[test]
fn suppress_label_hides_one_cabinet_label_only() {
    let mut sim = cottage();
    place(&mut sim, CabinetKind::Base, 60.0, 20.0);
    place(&mut sim, CabinetKind::Base, 100.0, 20.0);
    let mut all = cabs(&sim);
    assert!(all.iter().all(|c| !c.display_label().is_empty()));
    all[0].suppress_label = true;
    assert!(crate::editor::placed::replace_cabinet(
        &mut sim.app.cx.project,
        0,
        &all[0]
    ));
    let now = cabs(&sim);
    let hidden = now.iter().filter(|c| c.display_label().is_empty()).count();
    assert_eq!(hidden, 1, "only the suppressed cabinet loses its label");
}

#[test]
fn a_door_takes_five_manual_shelves() {
    use plan_cabinets::ShelfSpec;
    let mut c = Cabinet::wall(24.0);
    let door = c.face.items.iter_mut().find(|i| i.is_door()).unwrap();
    let spec = &mut door.props_mut().unwrap().shelves;
    assert!(!spec.manual, "Automatic is the starting choice");
    crate::dialogs::cabinet_shelf::set_manual(spec, 30.0);
    *spec = ShelfSpec::manual_of(5);
    spec.equalize(30.0);
    assert!(spec.manual);
    assert_eq!(spec.shelves.len(), 5);
    crate::dialogs::cabinet_shelf::set_automatic(spec);
    assert!(!spec.manual && spec.shelves.is_empty());
}
