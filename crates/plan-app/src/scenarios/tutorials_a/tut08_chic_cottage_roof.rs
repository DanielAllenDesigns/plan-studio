//! Lesson 8, Chic Cottage Roof (pp. 136-158).
use crate::editor::roof_view;
use crate::scenarios::tutorials_support::*;
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;

#[test]
fn build_roof_on_the_cottage_covers_every_wall() {
    let mut sim = cottage();
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    assert_one_undo_step(&mut sim, "Build Roof", |s| {
        s.click(240.0, 240.0);
        s.ok();
    });
    let set = roof_view::load(sim.app.cx.floor());
    assert!(set.planes.len() >= 6, "{} planes", set.planes.len());
    sim.undo();
    assert!(roof_view::load(sim.app.cx.floor()).planes.is_empty());
}

#[test]
#[ignore = "T7-08: RF-166 (Roof tab on a porch half wall: gull wing 4 in 12, upper 12 at 84)"]
fn porch_half_wall_gull_wing() {
    assert_ignored_break("RF-166");
}

#[test]
#[ignore = "T7-08: RF-61 (Join Roof Planes into a curved valley)"]
fn curved_valley() {
    assert_ignored_break("RF-61");
}

#[test]
#[ignore = "T7-08: RF-86 (frieze profile on Edit All Roof Planes)"]
fn frieze_profile() {
    assert_ignored_break("RF-86");
}

#[test]
#[ignore = "T7-08: RF-96 (Display On Floor Above)"]
fn roof_planes_display_on_the_floor_above() {
    assert_ignored_break("RF-96");
}

#[test]
#[ignore = "T7-08: DS-27 (O.H. dimension with leading text)"]
fn overhang_dimension_with_leading_text() {
    assert_ignored_break("DS-27");
}
