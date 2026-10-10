//! Lesson 9, Dormers (pp. 159-176).
use crate::editor::roof_view;
use crate::scenarios::tutorials_support::*;
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;

#[test]
fn floating_dormer_on_the_cottage_roof_is_one_undo_step() {
    let mut sim = cottage();
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 240.0);
    sim.ok();
    let south = roof_view::load(sim.app.cx.floor())
        .planes
        .into_iter()
        .filter(|p| (p.baseline.0.y - p.baseline.1.y).abs() < 1e-6)
        .min_by(|a, b| a.baseline.0.y.total_cmp(&b.baseline.0.y))
        .expect("a south plane");
    let c = south.centroid();
    sim.tool(ToolId::RoofVariant(RoofMode::FloatingDormer));
    assert_one_undo_step(&mut sim, "Floating Dormer", |s| {
        s.click(c.x, c.y);
        s.ok();
    });
    assert_eq!(roof_view::load(sim.app.cx.floor()).dormers.len(), 1);
}

#[test]
#[ignore = "T7-09: RF-166 (Knee Wall roof directive on an interior wall, attic room)"]
fn knee_wall_and_attic_room() {
    assert_ignored_break("RF-166");
}

#[test]
#[ignore = "T7-09: RF-80 (Ceiling Break Lines layer)"]
fn ceiling_break_lines() {
    assert_ignored_break("RF-80");
}

#[test]
#[ignore = "T7-09: S-137 (Tape Measure / Center Object on the dormer)"]
fn center_object_and_tape_measure() {
    assert_ignored_break("S-137");
}
