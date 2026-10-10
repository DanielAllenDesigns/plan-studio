//! Lesson 15, Cabinet Layout (pp. 260-277). Real: a row of base cabinets
//! butts, fillers and wall cabinets place as one undo step each. Open: the
//! Sticky Mode, island block, suppress label and shelf dialog steps.
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
#[ignore = "T7-15: CB-427 (island: marquee the cabinets and countertop, Make Architectural Block)"]
fn island_architectural_block() {
    assert_ignored_break("CB-427");
}

#[test]
#[ignore = "T7-15: CB-634 (Suppress Label on a cabinet)"]
fn suppress_label() {
    assert_ignored_break("CB-634");
}

#[test]
#[ignore = "T7-15: CB-633 (Cabinet Shelf Specification: Manual, 5 shelves)"]
fn shelf_specification() {
    assert_ignored_break("CB-633");
}

#[test]
#[ignore = "T7-15: R-113 (room crown molding wraps the soffit above a full height cabinet)"]
fn soffit_crown_wraps() {
    assert_ignored_break("R-113");
}
