//! Lesson 7, basic roof styles (pp. 127-135): a 34' x 24' clockwise rectangle.
use crate::editor::roof_view::{self, RoofSettings, RoofStyle};
use crate::editor::ObjectRef;
use crate::scenarios::tutorials_support::*;
use crate::scenarios::{draw_shell, Sim};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use plan_core::defaults::RoofWallKind;

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

/// Builds the hip roof with the Build Roof tool, as the guide's first step does.
fn hip_built() -> Sim {
    let mut sim = rectangle();
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(204.0, 144.0);
    sim.ok();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim
}

fn plane_count(sim: &Sim) -> usize {
    roof_view::load(sim.app.cx.floor()).planes.len()
}

/// Ids of the two walls at the short ends (the ones that run north-south).
fn end_walls(sim: &Sim) -> Vec<plan_core::Id> {
    sim.app
        .cx
        .floor()
        .walls
        .iter()
        .filter(|w| (w.end.y - w.start.y).abs() > (w.end.x - w.start.x).abs())
        .map(|w| w.id)
        .collect()
}

#[test]
fn gable_roof_by_two_full_gable_walls() {
    let mut sim = hip_built();
    assert_eq!(plane_count(&sim), 4);
    let ends = end_walls(&sim);
    assert_eq!(ends.len(), 2);
    // One Roof tab for the Shift-selection of both ends.
    sim.app.cx.selection.set(ObjectRef::Wall(ends[0]));
    sim.app.cx.selection.add(ObjectRef::Wall(ends[1]));
    assert_one_undo_step(&mut sim, "Full Gable Wall", |s| {
        s.app.cx.run_custom("roof.wall.gable");
    });
    for id in &ends {
        assert_eq!(
            sim.app.cx.floor().wall(*id).unwrap().roof.kind,
            RoofWallKind::FullGable
        );
    }
    assert_eq!(plane_count(&sim), 2, "a gable has two planes");
}

#[test]
fn the_other_styles_by_preset_and_group_edit() {
    let mut sim = rectangle();
    let fl = sim.app.cx.floor;
    let s = RoofSettings::from_defaults(&sim.app.cx.defaults);
    let mut counts = Vec::new();
    for style in RoofStyle::ALL {
        roof_view::apply_style(&mut sim.app.cx.project, fl, style, s.pitch).unwrap();
        roof_view::rebuild(&mut sim.app.cx.project, fl, s.clone(), false).unwrap();
        counts.push((style, plane_count(&sim)));
    }
    let n = |st: RoofStyle| counts.iter().find(|(c, _)| *c == st).unwrap().1;
    assert_eq!(n(RoofStyle::Hip), 4);
    assert_eq!(n(RoofStyle::Gable), 2);
    assert_eq!(n(RoofStyle::Shed), 1);
    assert_eq!(n(RoofStyle::Gambrel), 4);
    assert!(n(RoofStyle::DutchGable) > n(RoofStyle::Gable));
    assert!(n(RoofStyle::HalfHip) >= 3);

    // Mansard through the Roof tab on all four walls: a steep lower plane and
    // a shallow upper plane on every side.
    roof_view::apply_style(&mut sim.app.cx.project, fl, RoofStyle::Hip, s.pitch).unwrap();
    for w in &mut sim.app.cx.project.floors[fl].walls {
        w.roof.pitch_in_12 = Some(24.0);
        w.roof.upper_pitch = Some((6.0, w.height + 36.0));
    }
    roof_view::rebuild(&mut sim.app.cx.project, fl, s.clone(), false).unwrap();
    assert_eq!(plane_count(&sim), 8, "mansard: two planes per side");

    // Gull wing: 4 in 12 off the half wall, 12 in 12 from 84 in.
    roof_view::apply_style(&mut sim.app.cx.project, fl, RoofStyle::Hip, s.pitch).unwrap();
    for w in &mut sim.app.cx.project.floors[fl].walls {
        w.height = 36.0;
        w.roof.upper_pitch = None;
        w.roof.pitch_in_12 = Some(4.0);
    }
    sim.app.cx.project.floors[fl].walls[0].roof.upper_pitch = Some((12.0, 84.0));
    roof_view::rebuild(&mut sim.app.cx.project, fl, s, false).unwrap();
    let set = roof_view::load(sim.app.cx.floor());
    assert!(set.planes.iter().any(|p| p.pitch == 12.0));
    assert!(set.planes.iter().any(|p| p.pitch == 4.0));
}

#[test]
fn upper_pitch_break_height_equals_starts_at() {
    // In From Baseline and Starts at Height are two views of one number.
    let wall = 109.0;
    for (pitch, in_from) in [(6.0, 36.0), (12.0, 24.0), (4.0, 60.0)] {
        let start = plan_roof::start_height_for_in_from_baseline(wall, pitch, in_from);
        let back = plan_roof::in_from_baseline_for_start_height(wall, pitch, start);
        assert!((back - in_from).abs() < 1e-9, "{pitch}: {back}");
    }
    // A break at or below the wall top has no distance in.
    assert_eq!(
        plan_roof::in_from_baseline_for_start_height(wall, 6.0, 100.0),
        0.0
    );
}

#[test]
#[ignore = "T7-07: offset gable has no preset or Roof tab directive of its own; attic room is lesson 9"]
fn offset_gable_and_attic_room() {
    assert_ignored_break("RF-166");
}
