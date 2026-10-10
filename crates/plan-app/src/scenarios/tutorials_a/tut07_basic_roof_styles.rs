//! Lesson 7, basic roof styles (pp. 127-135): a 34' x 24' clockwise rectangle.
use crate::editor::roof_view;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::{draw_shell, Sim};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;

fn rectangle() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 408.0, 288.0);
    sim
}

#[test]
fn hip_roof_has_four_planes_in_one_undo_step() {
    let mut sim = rectangle();
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    assert_one_undo_step(&mut sim, "Build Roof", |s| {
        s.click(204.0, 144.0);
        s.ok();
    });
    let set = roof_view::load(sim.app.cx.floor());
    assert_eq!(set.planes.len(), 4, "hip: four planes");
}

#[test]
#[ignore = "T7-07: RF-166 (group Roof tab: Full Gable on two walls, one Wall Specification)"]
fn gable_roof_by_two_full_gable_walls() {
    assert_ignored_break("RF-166");
}

#[test]
#[ignore = "T7-07: RF-166 (Dutch, shed, offset gable, gambrel, gull wing, half hip, mansard via group edit)"]
fn the_other_seven_styles() {
    assert_ignored_break("RF-166");
}

#[test]
#[ignore = "T7-07: RF-165 (In From Baseline for upper pitch)"]
fn upper_pitch_break_height_equals_starts_at() {
    assert_ignored_break("RF-165");
}
