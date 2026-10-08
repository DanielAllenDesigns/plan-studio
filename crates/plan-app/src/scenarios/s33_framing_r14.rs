//! Scenario 33 (round 14): Build Framing as the user runs it. The Build
//! Framing dialog (Build > Framing > Build Framing...) saves its options and
//! builds in one undo step; ceiling framing, Retain Wall Framing, auto
//! rebuild, span warnings, the Framing Overview and the framing schedule
//! follow (CB-35..CB-41; the framing parity rows).

use super::{draw_shell, Sim};
use crate::dialogs::{defaults, framing as framing_dialog};
use crate::editor::framing_view::{self, FramingSettings};
use crate::toolbar::{Action, FramingCommand};
use plan_core::geometry::Point;
use plan_core::schedules::{Schedule, ScheduleKind};
use plan_core::{Id, OpeningKind};
use plan_framing::MemberKind;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    let first = sim.wall_ids()[0];
    sim.app
        .cx
        .project
        .add_opening(0, first, 200.0, OpeningKind::Door)
        .expect("a door fits");
    sim.app.cx.refresh();
    sim
}

fn count(sim: &Sim, k: MemberKind) -> usize {
    framing_view::load(sim.app.cx.floor())
        .iter()
        .filter(|m| m.kind == k)
        .count()
}

fn wall_studs(sim: &Sim, wall: Id) -> Vec<f64> {
    framing_view::load(sim.app.cx.floor())
        .iter()
        .filter(|m| m.wall_id == Some(wall) && m.kind == MemberKind::Stud)
        .map(|m| m.length)
        .collect()
}

fn change(sim: &mut Sim, edit: impl FnOnce(&mut FramingSettings)) {
    let mut st = framing_view::settings(&sim.app.cx.project);
    edit(&mut st);
    framing_view::set_settings(sim.cx(), st);
}

/// The menu item Build > Framing > Build Framing...: the Framing window opens
/// as the Build Framing dialog; Enter is its OK.
fn build_dialog_ok(sim: &mut Sim, all_floors: bool) {
    framing_dialog::request_build(all_floors);
    sim.action(Action::Custom(defaults::FRAMING));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame(true);
    sim.dialog_frame(false);
}

#[test]
fn the_build_framing_dialog_saves_its_options_and_builds_in_one_step() {
    let mut sim = house();
    // The wall tools made no framing; the layers are off until a build.
    assert!(count(&sim, MemberKind::Stud) == 0);
    change(&mut sim, |st| {
        st.build.build.ceiling = true;
        st.build.auto_rebuild.wall = true;
        st.walls.ceiling_joist_spacing = 24.0;
    });
    let steps = sim.app.cx.action_history().0.len();
    build_dialog_ok(&mut sim, false);
    // OK built the walls, the floor and the ceiling.
    assert!(count(&sim, MemberKind::Stud) > 20);
    assert!(count(&sim, MemberKind::Joist) > 10);
    assert!(count(&sim, MemberKind::CeilingJoist) > 5);
    assert!(count(&sim, MemberKind::Header) >= 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Build Framing"));
    assert_eq!(sim.app.cx.action_history().0.len(), steps + 1);
    // The options stay; the one-shot request does not.
    let st = framing_view::settings(&sim.app.cx.project);
    assert!(st.build.build.ceiling && st.build.auto_rebuild.wall);
    assert_eq!(st.build_on_ok, None);
    // The plan draws the framing: filled walls and dashed ceiling joists.
    assert!(sim.app.cx.layers().is_visible(framing_view::LAYER));
    assert!(sim.plan_shapes().len() > 100);
    // One undo takes the framing and the options' change back together.
    sim.undo();
    assert_eq!(count(&sim, MemberKind::Stud), 0);
    sim.redo();
    assert!(count(&sim, MemberKind::CeilingJoist) > 5);
}

#[test]
fn the_toolbar_build_command_and_build_all_still_work() {
    let mut sim = house();
    sim.action(Action::Framing(FramingCommand::Build));
    let one = count(&sim, MemberKind::Stud);
    assert!(one > 20);
    // A second floor is framed only by Build All Framing.
    sim.app.cx.project.build_new_floor(true);
    sim.action(Action::Framing(FramingCommand::Build));
    assert!(sim.app.cx.project.floors[1].framing.is_empty());
    sim.action(Action::Framing(FramingCommand::BuildAll));
    assert!(!sim.app.cx.project.floors[1].framing.is_empty());
    sim.action(Action::Framing(FramingCommand::Delete));
    assert!(sim.app.cx.floor().framing.iter().all(|v| {
        // Only the stored settings may remain.
        v.as_object()
            .is_some_and(|o| o.contains_key("FramingSettings"))
    }));
}

#[test]
fn a_retained_wall_survives_an_edit_and_a_rebuild() {
    let mut sim = house();
    sim.action(Action::Framing(FramingCommand::Build));
    let ids = sim.wall_ids();
    let before = wall_studs(&sim, ids[1]);
    assert!(!before.is_empty());
    assert_eq!(
        framing_view::set_walls_retained(sim.cx(), &[ids[1]], true),
        1
    );
    for w in &mut sim.app.cx.project.floors[0].walls {
        w.height = 96.0;
    }
    sim.action(Action::Framing(FramingCommand::Build));
    assert_eq!(
        wall_studs(&sim, ids[1]),
        before,
        "the retained wall is as it was"
    );
    assert!(wall_studs(&sim, ids[2]).iter().all(|l| *l < 95.0));
    // Undo the build: the retained wall and its flag both come back.
    sim.undo();
    assert!(framing_view::wall_retained(&sim.app.cx.project, ids[1]));
    assert!(wall_studs(&sim, ids[2]).iter().all(|l| *l > 100.0));
}

#[test]
fn auto_rebuild_keeps_the_walls_framing_current() {
    let mut sim = house();
    change(&mut sim, |st| st.build.auto_rebuild.wall = true);
    sim.action(Action::Framing(FramingCommand::Build));
    sim.app.cx.refresh();
    assert!(!framing_view::auto_rebuild(sim.cx()));
    let ids = sim.wall_ids();
    for w in &mut sim.app.cx.project.floors[0].walls {
        w.height = 96.0;
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert!(framing_view::auto_rebuild(sim.cx()));
    assert!(wall_studs(&sim, ids[2]).iter().all(|l| *l < 95.0));
    sim.app.cx.refresh();
    assert!(!framing_view::auto_rebuild(sim.cx()));
}

#[test]
fn spans_that_lumber_cannot_carry_show_in_the_status_line() {
    let mut sim = house();
    change(&mut sim, |st| {
        st.walls.joist_size = plan_framing::TWO_BY_SIX
    });
    sim.action(Action::Framing(FramingCommand::Build));
    let status = sim.app.cx.status.clone();
    assert!(
        status.contains("Check spans") && status.contains("2x6"),
        "{status}"
    );
}

#[test]
fn the_framing_overview_and_the_schedule_know_ceiling_framing() {
    let mut sim = house();
    change(&mut sim, |st| st.build.build.ceiling = true);
    sim.action(Action::Framing(FramingCommand::Build));
    // Framing Overview: a plan view of the framing alone, and a 3D scene of it.
    sim.action(Action::Custom(defaults::FRAMING_OVERVIEW));
    assert!(framing_view::in_overview(&sim.app.cx.project));
    let scene = framing_view::overview_scene(&sim.app.cx.project);
    assert!(scene.meshes.len() >= framing_view::load(sim.app.cx.floor()).len());
    sim.action(Action::Custom(defaults::FRAMING_OVERVIEW));
    assert!(!framing_view::in_overview(&sim.app.cx.project));
    // The framing schedule lists the ceiling joists by size and length.
    let rows = plan_docs::schedule_kinds::table(
        &sim.app.cx.project,
        &Schedule::new(ScheduleKind::Framing, Point::ZERO),
        0,
        None,
    )
    .rows;
    assert!(
        rows.iter()
            .any(|r| r.iter().any(|c| c.contains("ceiling joist"))),
        "{rows:?}"
    );
}

#[test]
fn a_wall_detail_is_dimensioned() {
    let mut sim = house();
    sim.action(Action::Framing(FramingCommand::Build));
    let door_wall = sim.wall_ids()[0];
    let d = framing_view::wall_detail_of(&sim.app.cx.project, 0, door_wall).expect("framed");
    let kinds: Vec<_> = d.dims.iter().map(|x| x.kind).collect();
    for k in [
        plan_framing::DimKind::Overall,
        plan_framing::DimKind::WallHeight,
        plan_framing::DimKind::StudSpacing,
        plan_framing::DimKind::RoughWidth,
        plan_framing::DimKind::RoughHeight,
        plan_framing::DimKind::HeaderHeight,
    ] {
        assert!(kinds.contains(&k), "missing {k:?}");
    }
}
