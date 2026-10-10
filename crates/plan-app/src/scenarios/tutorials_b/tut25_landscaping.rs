//! Lesson 25, Landscaping Design (pp. 433-450). Real: a polyline garden bed
//! is one undo step. Open: fencing and gates, plant distribution, the
//! shrub block and the parallel-edge steps.
use super::support_b::*;
use crate::editor::site_view::load_terrain;
use crate::scenarios::tutorials_support::*;
use crate::tools::terrain::TerrainVariant as V;

#[test]
fn a_polyline_garden_bed_is_one_undo_step() {
    let mut sim = cottage_lot();
    let features = |s: &crate::scenarios::Sim| {
        load_terrain(&s.app.cx.project)
            .map_or(0, |r| crate::editor::site_view::all_hits(&r.terrain).len())
    };
    let base = features(&sim);
    assert_one_undo_step(&mut sim, "bed", |s| {
        draw_terrain(
            s,
            V::BedPolyline,
            &[
                (100.0, 560.0),
                (300.0, 540.0),
                (420.0, 640.0),
                (120.0, 680.0),
            ],
            &[],
        )
    });
    assert_eq!(features(&sim), base + 1);
    sim.undo();
    assert_eq!(features(&sim), base);
}

#[test]
#[ignore = "T7-25: CAD-22 (Change Line/Arc on the bed edge; Make Parallel to the setbacks)"]
fn bed_edges_parallel_to_setbacks() {
    assert_ignored_break("CAD-22");
}

#[test]
#[ignore = "T7-25: S-170 (fence replaces a retaining wall at the same place; undo restores)"]
fn fence_over_retaining_wall() {
    assert_ignored_break("S-170");
}

#[test]
#[ignore = "T7-25: CB-649 (gate in the fence: height 72, floor to bottom 4)"]
fn fence_gate() {
    assert_ignored_break("CB-649");
}

#[test]
#[ignore = "T7-25: CB-427 (three shrubs as an Architectural Block; copies share geometry; Explode)"]
fn shrub_block() {
    assert_ignored_break("CB-427");
}

#[test]
#[ignore = "T7-25: CB-617 (Plant Chooser fir; one click in a bed distributes copies)"]
fn plants_fill_a_bed() {
    assert_ignored_break("CB-617");
}
