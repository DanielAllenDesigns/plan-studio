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
fn staircase_specification_best_fit() {
    use crate::editor::stairs_view::staircase;
    use crate::editor::ObjectRef;
    use plan_stairs::{best_fit, LockEnd, TreadMode};
    let mut sim = cottage();
    sim.action(Action::BuildNewFloor);
    sim.ok();
    sim.action(Action::FloorDown);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (250.0, 100.0));
    let id = stairs_view::load(&sim.app.cx.project.floors[0])[0].id();
    sim.app.cx.selection.set(ObjectRef::Stair(id));
    // Lock Bottom, Make Best Fit: the riser is the one nearest 6 3/4 inches.
    staircase::make_best_fit(&mut sim.app.cx, id, LockEnd::Bottom).expect("best fit");
    let o = stairs_view::find(&sim.app.cx.project.floors[0], id).unwrap();
    let best = best_fit(o.stair.params.total_rise);
    assert_eq!(o.solution().risers, best.risers);
    // Lock Number of Treads, then a tread depth of 10 1/2 inches: the count
    // stays and the section grows to fit.
    assert!(staircase::set_tread_mode(
        &mut sim.app.cx,
        id,
        TreadMode::LockCount
    ));
    let mut o = stairs_view::find(&sim.app.cx.project.floors[0], id).unwrap();
    let treads = o.solution().treads;
    let len = staircase::section_length(&o);
    staircase::set_length(&mut o, 10.5 * f64::from(treads));
    assert_eq!(o.solution().treads, treads);
    assert!((o.stair.params.tread_depth - 10.5).abs() < 1e-6, "{len}");
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
