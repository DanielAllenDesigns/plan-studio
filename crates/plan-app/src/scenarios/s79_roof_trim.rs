//! Scenario 79: Gable/Roof Lines, Gable Over Door/Window, the roof trim parts
//! and skylights (manual pp. 861 to 879; RF-44, RF-83..RF-87, RF-133..RF-143).
//!
//! Build the roof over the shell, put a door in the south wall, make a gable
//! over it, edit the gable line's specification, delete it and draw one by
//! hand; Build Roof then turns the kept lines into gables. Switch the roof
//! trim on (rafter tails, ridge caps, gutters, frieze), edit one piece and
//! regenerate; place a skylight and give it a shape, a rim and a ceiling
//! hole. Every action is one undo step.

use super::{draw_shell, Sim};
use crate::dialogs::roof_trim as dlg;
use crate::dialogs::skylight as sky;
use crate::editor::details_view;
use crate::editor::roof_view::{self, RoofPlaneRecord};
use crate::editor::ObjectRef;
use crate::tools::gable_line as gl;
use crate::tools::roof::RoofMode;
use crate::tools::roof_trim as trim;
use crate::tools::ToolId;
use plan_3d::TrimKind;
use plan_core::geometry::{polygon_area, Point};
use plan_core::moldings::builtin_profiles;
use plan_core::{Id, OpeningKind};
use plan_roof::{CeilingHole, GableLineProblem, HoleRim, SkylightShape};

const W: f64 = 480.0;
const H: f64 = 288.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    draw_shell(&mut sim, W, H);
    sim
}

fn label(sim: &Sim) -> Option<String> {
    sim.app.cx.undo_label().map(String::from)
}

fn planes(sim: &Sim) -> Vec<RoofPlaneRecord> {
    roof_view::load(sim.app.cx.floor()).planes
}

fn lines(sim: &Sim) -> Vec<gl::GableLineRecord> {
    gl::lines(sim.app.cx.floor())
}

/// Build Roof the way a user does: pick the tool, click, OK the dialog.
fn build_roof(sim: &mut Sim) {
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(W / 2.0, H / 2.0);
    sim.ok();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
}

/// Places a door on the south wall centred at `x` and leaves it selected.
fn door_at(sim: &mut Sim, x: f64) -> Id {
    sim.tool(ToolId::Door);
    sim.click(x, 0.0);
    let id = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .filter(|o| o.kind == OpeningKind::Door)
        .map(|o| o.id)
        .max()
        .expect("a door was placed");
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Opening(id));
    id
}

#[test]
fn gable_over_a_door_makes_one_line_in_one_undo_step() {
    let mut sim = house();
    build_roof(&mut sim);
    let door = door_at(&mut sim, 240.0);
    let before = lines(&sim).len();

    // The Edit toolbar offers the command for the selected door.
    let acts = gl::edit_actions(&sim.app.cx);
    assert_eq!(acts.len(), 1, "Delete Gable Over Opening needs a gable");
    assert_eq!(acts[0].label, "Gable Over Door/Window");

    assert!(gl::run_command(&mut sim.app.cx, gl::cmd::OVER_OPENING));
    assert_eq!(label(&sim).as_deref(), Some("Gable Over Door/Window"));
    let made = lines(&sim);
    assert_eq!(made.len(), before + 1);
    let l = &made[made.len() - 1];
    // 36" door plus 12" past each side, on the outside face of the wall.
    assert!(
        (l.line().length() - 60.0).abs() < 0.5,
        "{}",
        l.line().length()
    );
    assert_eq!(l.openings, vec![door]);
    assert!((l.pitch - 8.0).abs() < 1e-9 && (l.overhang - 16.0).abs() < 1e-9);
    assert!(l.a.y.abs() < 8.0 && l.b.y.abs() < 8.0);

    // Now the Delete button shows too.
    assert_eq!(gl::edit_actions(&sim.app.cx).len(), 2);

    sim.undo();
    assert_eq!(lines(&sim).len(), before, "one undo step");
    sim.redo();
    assert_eq!(lines(&sim).len(), before + 1);
}

#[test]
fn doors_within_thirty_inches_share_one_gable_line() {
    let mut sim = house();
    build_roof(&mut sim);
    let a = door_at(&mut sim, 200.0);
    let b = door_at(&mut sim, 260.0);
    sim.app.cx.selection.set(ObjectRef::Opening(a));
    sim.app.cx.selection.add(ObjectRef::Opening(b));
    assert_eq!(gl::gable_over_openings(&mut sim.app.cx), 1);
    let l = lines(&sim);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].openings.len(), 2);
    // 182 to 278 plus 12" at each end.
    assert!((l[0].line().length() - (96.0 + 24.0)).abs() < 1.0);

    // Far apart they get a gable each.
    let mut sim = house();
    build_roof(&mut sim);
    let a = door_at(&mut sim, 100.0);
    let b = door_at(&mut sim, 380.0);
    sim.app.cx.selection.set(ObjectRef::Opening(a));
    sim.app.cx.selection.add(ObjectRef::Opening(b));
    assert_eq!(gl::gable_over_openings(&mut sim.app.cx), 2);
}

#[test]
fn delete_gable_over_opening_removes_only_that_openings_line() {
    let mut sim = house();
    build_roof(&mut sim);
    let a = door_at(&mut sim, 100.0);
    gl::gable_over_openings(&mut sim.app.cx);
    let b = door_at(&mut sim, 380.0);
    gl::gable_over_openings(&mut sim.app.cx);
    assert_eq!(lines(&sim).len(), 2);

    sim.app.cx.selection.set(ObjectRef::Opening(a));
    assert!(gl::run_command(
        &mut sim.app.cx,
        gl::cmd::DELETE_OVER_OPENING
    ));
    assert_eq!(label(&sim).as_deref(), Some("Delete Gable Over Opening"));
    let left = lines(&sim);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].openings, vec![b]);

    // No gable over the opening any more: nothing to delete, no undo step.
    let depth = label(&sim);
    assert_eq!(gl::delete_over_openings(&mut sim.app.cx), 0);
    assert_eq!(label(&sim), depth);
    sim.undo();
    assert_eq!(lines(&sim).len(), 2);
}

#[test]
fn a_hand_drawn_line_must_be_parallel_to_a_wall_and_within_ten_feet() {
    let mut sim = house();
    build_roof(&mut sim);
    let cx = &mut sim.app.cx;
    // Parallel to the north wall (y = 288), 40" in front of it.
    let id = gl::draw_line(cx, Point::new(100.0, 248.0), Point::new(220.0, 248.0)).unwrap();
    assert_eq!(cx.undo_label(), Some("Gable/Roof Line"));
    assert_eq!(gl::line(cx.floor(), id).map(|l| l.pitch), Some(8.0));

    let slanted = gl::draw_line(cx, Point::new(100.0, 100.0), Point::new(220.0, 160.0));
    assert_eq!(
        slanted.unwrap_err(),
        GableLineProblem::NotParallel.message()
    );
    let far = gl::draw_line(cx, Point::new(100.0, 140.0), Point::new(220.0, 140.0));
    assert_eq!(far.unwrap_err(), GableLineProblem::TooFar.message());
    let short = gl::draw_line(cx, Point::new(100.0, 248.0), Point::new(105.0, 248.0));
    assert_eq!(short.unwrap_err(), GableLineProblem::TooShort.message());
    assert_eq!(
        gl::lines(cx.floor()).len(),
        1,
        "refused lines leave nothing"
    );
}

#[test]
fn the_gable_line_specification_edits_the_pitch_and_overhang() {
    let mut sim = house();
    build_roof(&mut sim);
    let id = gl::draw_line(
        &mut sim.app.cx,
        Point::new(100.0, 248.0),
        Point::new(220.0, 248.0),
    )
    .unwrap();
    assert!(dlg::open_spec(&sim.app.cx, id));
    dlg::with_dialog(|d| {
        d.record_mut().pitch = 12.0;
        d.record_mut().overhang = 24.0;
        d.record_mut().arrow_end = true;
    })
    .expect("the dialog is open");
    assert!(dlg::accept_dialog(&mut sim.app.cx));
    assert_eq!(label(&sim).as_deref(), Some("Gable Line Specification"));
    let l = gl::line(sim.app.cx.floor(), id).unwrap();
    assert_eq!((l.pitch, l.overhang, l.arrow_end), (12.0, 24.0, true));
    sim.undo();
    assert_eq!(gl::line(sim.app.cx.floor(), id).unwrap().pitch, 8.0);
    assert!(!dlg::open_spec(&sim.app.cx, 999_999), "no such line");
}

#[test]
fn build_roof_turns_the_kept_lines_into_gables_again_and_again() {
    let mut sim = house();
    build_roof(&mut sim);
    let hip = planes(&sim);
    let id = gl::draw_line(
        &mut sim.app.cx,
        Point::new(200.0, 0.0),
        Point::new(260.0, 0.0),
    )
    .unwrap();
    // The line alone changes nothing until Build Roof.
    assert_eq!(planes(&sim).len(), hip.len());

    let fl = sim.app.cx.floor;
    let used = gl::apply_stored(&mut sim.app.cx.project, fl);
    assert_eq!(used, 1);
    let gabled = planes(&sim);
    let wings: Vec<&RoofPlaneRecord> = gabled
        .iter()
        .filter(|p| p.auto && p.source.is_none())
        .collect();
    assert_eq!(wings.len(), 2, "two planes of the gable");
    assert!(wings.iter().all(|w| (w.pitch - 8.0).abs() < 1e-9));
    // The ridge of the gable stands over the line's middle, 46" out from
    // the eave: 100" eave height in the plan is not assumed, only relative.
    let eave = gabled[0].baseline_height();
    let top = wings
        .iter()
        .flat_map(|w| w.polygon3d.iter())
        .fold(f64::MIN, |m, v| m.max(v[1]));
    assert!((top - eave - 46.0 * 8.0 / 12.0).abs() < 1.0, "{top} {eave}");

    // The line is still there after a rebuild, and builds the same gable.
    build_roof(&mut sim);
    assert_eq!(gl::lines(sim.app.cx.floor()).len(), 1);
    assert_eq!(gl::lines(sim.app.cx.floor())[0].id, id);
    // Build Roof applies the stored lines itself (`roof_view::rebuild` calls
    // `gable_line::apply_stored`): the same gable as before.
    assert_eq!(planes(&sim).len(), gabled.len(), "the rebuild gables again");
}

#[test]
fn deleting_the_line_ends_the_gable() {
    let mut sim = house();
    build_roof(&mut sim);
    let id = gl::draw_line(
        &mut sim.app.cx,
        Point::new(200.0, 0.0),
        Point::new(260.0, 0.0),
    )
    .unwrap();
    assert!(gl::delete_line(&mut sim.app.cx, id));
    assert_eq!(label(&sim).as_deref(), Some("Delete Gable/Roof Line"));
    let fl = sim.app.cx.floor;
    assert_eq!(gl::apply_stored(&mut sim.app.cx.project, fl), 0);
    assert!(!gl::delete_line(&mut sim.app.cx, id), "already gone");
}

// ---------------------------------------------------------------------------
// Roof trim
// ---------------------------------------------------------------------------

fn trim_lines(sim: &Sim) -> Vec<plan_core::details::MoldingLine> {
    details_view::load(&sim.app.cx)
        .moldings
        .into_iter()
        .filter(trim::is_roof_trim)
        .collect()
}

/// Opens the Roof Trim dialog, lets `f` set the options and presses OK.
fn set_trim(sim: &mut Sim, f: impl FnOnce(&mut plan_3d::RoofTrimOptions)) {
    dlg::open_trim(&sim.app.cx);
    dlg::with_trim_dialog(|d| f(d.options_mut())).expect("the dialog is open");
    assert!(dlg::accept_trim(&mut sim.app.cx));
}

#[test]
fn roof_trim_is_generated_as_molding_polylines_in_one_undo_step() {
    let mut sim = house();
    build_roof(&mut sim);
    assert!(trim_lines(&sim).is_empty(), "nothing until a part is on");
    assert!(!trim::build_trim(&mut sim.app.cx), "and nothing to build");

    set_trim(&mut sim, |o| {
        o.gutters.enabled = true;
        o.frieze.enabled = true;
        o.ridge_caps.spec.enabled = true;
        o.rafter_tails.enabled = true;
    });
    assert_eq!(label(&sim).as_deref(), Some("Roof Trim"));
    let lines = trim_lines(&sim);
    assert!(!lines.is_empty());
    assert!(lines.iter().all(|m| m.automatic && m.layer == trim::LAYER));
    let summary = trim::summary(sim.app.cx.floor());
    let of = |k: TrimKind| summary.iter().find(|(x, _, _)| *x == k).copied();
    // One gutter loop and one frieze loop round the hip roof; its ridge caps
    // lie on both planes of the ridge and the four hips.
    let gutter = of(TrimKind::Gutter).expect("gutters");
    let frieze = of(TrimKind::Frieze).expect("frieze");
    assert_eq!(gutter.1, 1);
    assert_eq!(frieze.1, 1);
    assert_eq!(of(TrimKind::RidgeCap).map(|c| c.1), Some(10));
    assert!(of(TrimKind::RafterTail).is_some_and(|c| c.1 > 20));
    // The gutter runs round the eaves, 16" past the wall faces (3" half wall).
    let want = 2.0 * ((W + 2.0 * (16.0 + 3.0)) + (H + 2.0 * (16.0 + 3.0)));
    assert!((gutter.2 - want).abs() < 20.0, "{} vs {want}", gutter.2);
    // The Materials List counts them under Exterior Trim.
    let takeoff = plan_core::moldings::trim_takeoff(&sim.app.cx.project);
    assert!(takeoff
        .iter()
        .any(|t| t.category == plan_core::moldings::EXTERIOR_TRIM && t.item.starts_with("Gutter")));

    sim.undo();
    assert!(trim_lines(&sim).is_empty(), "one undo step takes them all");
    sim.redo();
    assert_eq!(trim_lines(&sim).len(), lines.len());
}

#[test]
fn roof_trim_makes_automatic_molding_polylines_that_survive_an_edit() {
    let mut sim = house();
    build_roof(&mut sim);
    let profile = builtin_profiles()
        .into_iter()
        .find(|p| p.name.starts_with("Casing"))
        .expect("a built-in casing profile");
    set_trim(&mut sim, |o| {
        o.frieze.enabled = true;
        o.frieze.profile = profile.name.clone();
    });
    let before = trim_lines(&sim);
    assert_eq!(before.len(), 1);
    assert!(before[0].table.rows[0].profile.name.contains(&profile.name));

    // Editing the frieze (here: swapping its profile) removes its Automatic
    // flag, and a later Build Roof Trim leaves it alone and makes no twin.
    let id = before[0].id;
    let other = builtin_profiles()
        .into_iter()
        .find(|p| p.name != profile.name && p.name.starts_with("Base"))
        .expect("a base profile");
    assert_eq!(
        crate::tools::molding::replace_moldings(&mut sim.app.cx, &[id], &other),
        1
    );
    assert!(!trim_lines(&sim)[0].automatic);
    set_trim(&mut sim, |o| o.gutters.enabled = true);
    let after = trim_lines(&sim);
    let frieze: Vec<_> = after
        .iter()
        .filter(|m| m.label.ends_with("Frieze"))
        .collect();
    assert_eq!(frieze.len(), 1, "the edited frieze stays, no twin");
    assert_eq!(frieze[0].id, id);
    assert!(!frieze[0].automatic);
    assert_eq!(
        after.iter().filter(|m| m.label.ends_with("Gutter")).count(),
        1
    );

    // Switching a part off takes its automatic lines away.
    set_trim(&mut sim, |o| o.gutters.enabled = false);
    assert!(trim_lines(&sim)
        .iter()
        .all(|m| !m.label.ends_with("Gutter")));
}

#[test]
fn trim_options_live_with_the_roof_and_a_dialog_cancel_changes_nothing() {
    let mut sim = house();
    build_roof(&mut sim);
    set_trim(&mut sim, |o| {
        o.shadow_boards.enabled = true;
        o.soffit = plan_3d::SoffitStyle::Flush;
        o.higher_eaves_boxed = true;
    });
    let opts = trim::options(sim.app.cx.floor());
    assert!(opts.shadow_boards.enabled && opts.higher_eaves_boxed);
    assert_eq!(opts.soffit, plan_3d::SoffitStyle::Flush);
    // Open and drop the dialog (Cancel): nothing is written.
    dlg::open_trim(&sim.app.cx);
    dlg::with_trim_dialog(|d| d.options_mut().gutters.enabled = true).unwrap();
    assert!(dlg::trim_dialog_open());
    let depth = label(&sim);
    assert_eq!(trim::options(sim.app.cx.floor()), opts);
    assert_eq!(label(&sim), depth);
}

// ---------------------------------------------------------------------------
// Skylights
// ---------------------------------------------------------------------------

fn first_skylight(sim: &Sim) -> sky::SkylightRef {
    sky::skylights(sim.app.cx.floor())[0].0
}

fn hole_area(sim: &Sim, r: sky::SkylightRef) -> f64 {
    let (_, h) = sky::skylights(sim.app.cx.floor())
        .into_iter()
        .find(|(x, _)| *x == r)
        .expect("the skylight exists");
    polygon_area(&h.outline).abs()
}

#[test]
fn a_click_makes_a_two_foot_square_skylight_and_a_drag_a_rectangle() {
    let mut sim = house();
    build_roof(&mut sim);
    let at = Point::new(240.0, 60.0);
    let r = sky::place(&mut sim.app.cx, at, at).expect("on the south plane");
    assert_eq!(label(&sim).as_deref(), Some("Place Skylight"));
    assert!((hole_area(&sim, r) - 24.0 * 24.0).abs() < 1e-6);
    let o = sky::options_of(sim.app.cx.floor(), r).unwrap();
    assert_eq!(o.shape, SkylightShape::Rectangle);
    assert!((o.width - 24.0).abs() < 1e-6 && (o.length - 24.0).abs() < 1e-6);

    let drag = sky::place(
        &mut sim.app.cx,
        Point::new(200.0, 40.0),
        Point::new(240.0, 100.0),
    )
    .unwrap();
    assert!((hole_area(&sim, drag) - 40.0 * 60.0).abs() < 1e-6);

    // Outside the roof nothing is placed and no undo step is left.
    let depth = label(&sim);
    let n = sky::skylights(sim.app.cx.floor()).len();
    assert!(sky::place(
        &mut sim.app.cx,
        Point::new(900.0, 900.0),
        Point::new(900.0, 900.0)
    )
    .is_err());
    assert_eq!(sky::skylights(sim.app.cx.floor()).len(), n);
    assert_eq!(label(&sim), depth);
}

#[test]
fn a_skylight_is_edited_and_reshaped_in_one_undo_step_each() {
    let mut sim = house();
    build_roof(&mut sim);
    let at = Point::new(240.0, 60.0);
    let r = sky::place(&mut sim.app.cx, at, at).unwrap();

    assert!(sky::open_spec(&sim.app.cx, r));
    sky::with_dialog(|d| {
        d.options_mut().shape = SkylightShape::Circle;
        d.options_mut().width = 30.0;
        d.options_mut().rim = HoleRim::Plumb;
        d.options_mut().ceiling_hole = CeilingHole::DoNotCut;
        d.options_mut().display_in_plan = false;
        d.spec_mut().frame_width = 3.0;
        d.spec_mut().curb_height = 8.0;
    })
    .expect("the dialog is open");
    assert!(sky::accept_dialog(&mut sim.app.cx));
    assert_eq!(label(&sim).as_deref(), Some("Skylight Specification"));
    let circle = std::f64::consts::PI * 15.0 * 15.0;
    assert!((hole_area(&sim, r) - circle).abs() < 0.02 * circle);
    let o = sky::options_of(sim.app.cx.floor(), r).unwrap();
    assert_eq!(
        (o.shape, o.rim, o.ceiling_hole),
        (SkylightShape::Circle, HoleRim::Plumb, CeilingHole::DoNotCut)
    );
    assert!(!o.display_in_plan);
    let (_, hole) = sky::skylights(sim.app.cx.floor())[0].clone();
    let spec = hole.skylight.unwrap();
    assert_eq!((spec.frame_width, spec.curb_height), (3.0, 8.0));

    // The schedule row carries the shape and the frame.
    let rows = sky::schedule_rows(sim.app.cx.floor());
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].shape, rows[0].frame_height),
        (SkylightShape::Circle, 8.0)
    );

    // Do Not Cut: no ceiling hole; Automatic follows the rim.
    let none = |_: sky::SkylightRef| None;
    assert!(sky::ceiling_holes(&sim.app.cx.project, 0, 96.0, &none).is_empty());

    // Edit Skylight Shape: one corner moved makes it Custom.
    let r = first_skylight(&sim);
    let corner = sky::skylights(sim.app.cx.floor())[0].1.outline[0];
    assert!(sky::move_corner(
        &mut sim.app.cx,
        r,
        0,
        Point::new(corner.x - 4.0, corner.y - 4.0)
    ));
    assert_eq!(label(&sim).as_deref(), Some("Edit Skylight Shape"));
    assert_eq!(
        sky::options_of(sim.app.cx.floor(), r).unwrap().shape,
        SkylightShape::Custom
    );

    sim.undo();
    assert_eq!(
        sky::options_of(sim.app.cx.floor(), first_skylight(&sim))
            .unwrap()
            .shape,
        SkylightShape::Circle
    );
    sim.undo();
    let r = first_skylight(&sim);
    assert!(
        (hole_area(&sim, r) - 24.0 * 24.0).abs() < 1e-6,
        "back to the square"
    );
}

#[test]
fn the_ceiling_hole_of_an_automatic_skylight_follows_its_rim() {
    let mut sim = house();
    build_roof(&mut sim);
    let at = Point::new(240.0, 60.0);
    let r = sky::place(&mut sim.app.cx, at, at).unwrap();
    let none = |_: sky::SkylightRef| None;
    let holes = sky::ceiling_holes(&sim.app.cx.project, 0, 96.0, &none);
    assert_eq!(holes.len(), 1);
    // Square rim: the ceiling hole leans up the slope from the roof opening.
    let roof = sky::skylights(sim.app.cx.floor())[0].1.outline.clone();
    let cy = |p: &[Point]| p.iter().map(|q| q.y).sum::<f64>() / p.len() as f64;
    assert!(cy(&holes[0]) > cy(&roof) + 1.0);

    sky::open_spec(&sim.app.cx, r);
    sky::with_dialog(|d| d.options_mut().rim = HoleRim::Plumb).unwrap();
    assert!(sky::accept_dialog(&mut sim.app.cx));
    let plumb = sky::ceiling_holes(&sim.app.cx.project, 0, 96.0, &none);
    assert!(
        (cy(&plumb[0]) - cy(&roof)).abs() < 1e-6,
        "plumb: straight down"
    );
}
