//! Lesson 4, Interior Stairs (pp. 60-81).
use crate::editor::stairs_view::{self, StairKind};
use crate::scenarios::tutorials_support::*;
use crate::toolbar::Action;
use crate::tools::ToolId;

#[test]
fn draw_stairs_between_floors_reaches_the_next_floor_in_one_undo_step() {
    let mut sim = cottage();
    sim.action(Action::BuildNewFloor);
    sim.ok();
    sim.action(Action::FloorDown);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    let r = assert_one_undo_step(&mut sim, "Draw Stairs", |s| {
        s.drag((100.0, 100.0), (250.0, 100.0))
    });
    assert_eq!(r.commit.as_deref(), Some("Draw Stairs"));
    let all = stairs_view::load(&sim.app.cx.project.floors[0]);
    assert_eq!(all.len(), 1);
    let rise = sim.app.cx.project.floors[1].elevation - sim.app.cx.project.floors[0].elevation;
    assert!((all[0].stair.params.total_rise - rise).abs() < 1e-6);
    let sol = all[0].solution();
    assert!((14..=18).contains(&sol.risers), "{} risers", sol.risers);
}

#[test]
#[ignore = "T7-04: CB-160 (Lock Bottom, Make Best Fit, Lock Number of Treads, tread 10 1/2)"]
fn staircase_specification_best_fit() {
    assert_ignored_break("CB-160");
}

#[test]
#[ignore = "T7-04: CB-162 (stacked basement stair by Copy + Paste Hold Position)"]
fn stacked_stair_hold_position() {
    assert_ignored_break("CB-162");
}

#[test]
#[ignore = "T7-04: DIM-62 (End to End headroom dimension in the Stair Section)"]
fn headroom_dimension_in_a_section() {
    assert_ignored_break("DIM-62");
}

#[test]
#[ignore = "T7-04: C-136 (clip planes of the Back Clipped Cross Section)"]
fn section_clip_planes() {
    assert_ignored_break("C-136");
}

#[test]
#[ignore = "T7-04: TXT-56 (Capsule note and Note Schedule)"]
fn capsule_notes_and_note_schedule() {
    assert_ignored_break("TXT-56");
}
