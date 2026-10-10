//! Scenario 80 (round 16, brief 21): the staircase engine. A flight broken in
//! two sections with a landing, section numbers and the specification table,
//! Make Best Fit, the tread depth modes, Lock Top / Lock Bottom, merging two
//! flights, the landing rules and the Staircase Specification read-outs
//! (CB-24 to CB-35, CB-107, CB-133, CB-139 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`).

use super::{draw_shell, Sim};
use crate::dialogs::stairs::StairDialog;
use crate::editor::stairs_view::{self as view, staircase, StairCommand, StairKind, StairObj};
use crate::editor::ObjectRef;
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_stairs::{
    best_fit, solve, tagged_meshes, EdgeRail, LockEnd, Stair, StairParams, StairShape, TreadMode,
};

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim
}

fn stairs(sim: &Sim) -> Vec<StairObj> {
    view::load(sim.app.cx.floor())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// A straight flight of 7 risers of 7.5" (10" treads) heading east.
fn flight(sim: &mut Sim, origin: Point, base: f64) -> u64 {
    let st = Stair::new(
        0,
        origin,
        0.0,
        StairParams {
            total_rise: 52.5,
            riser_height_target: 7.5,
            tread_depth: 10.0,
            ..StairParams::default()
        },
    );
    let mut o = StairObj {
        stair: st,
        x: view::StairExtras::default(),
    };
    o.stair.base = base;
    let fl = sim.app.cx.floor;
    view::add(&mut sim.app.cx.project, fl, o)
}

/// A drawn straight stair divided by Complete Break: lower flight, landing,
/// upper flight, with the stair off its Best Fit first.
fn broken_stair(sim: &mut Sim) -> (u64, u64) {
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((60.0, 100.0), (60.0, 300.0));
    let all = stairs(sim);
    assert_eq!(all.len(), 1);
    let id = all[0].id();
    let fl = sim.app.cx.floor;
    let best = best_fit(all[0].stair.params.total_rise);
    // Two risers fewer than the Best Fit: steeper than it.
    let fewer = best.risers.saturating_sub(2).max(5);
    view::update(&mut sim.app.cx.project, fl, id, |o| {
        view::set_risers(o, fewer);
    });
    sim.app.cx.selection.set(ObjectRef::Stair(id));
    assert!(view::run_command(
        &mut sim.app.cx,
        StairCommand::CompleteBreak
    ));
    let secs = staircase::sections(sim.app.cx.floor(), id);
    assert_eq!(secs.len(), 2, "a lower and an upper flight");
    (secs[0].id(), secs[1].id())
}

#[test]
fn a_broken_stair_numbers_its_sections_and_lists_them_in_the_table() {
    let mut sim = house();
    let (lo, up) = broken_stair(&mut sim);
    let all = staircase::staircase(sim.app.cx.floor(), lo);
    assert_eq!(all.len(), 3, "flight, landing, flight");
    assert!(all[1].is_landing());
    let rows = staircase::spec_table(sim.app.cx.floor(), up);
    assert_eq!(
        rows.iter().map(|r| r.number.as_str()).collect::<Vec<_>>(),
        ["1", "2"]
    );
    assert!(close(rows[0].bottom_height, 0.0));
    // The upper flight begins where the lower one ends.
    assert!(close(rows[1].bottom_height, rows[0].top_height));
    let labels = staircase::section_labels(sim.app.cx.floor(), lo);
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0].1, "1");
    assert_eq!(labels[1].1, "2");
    let info = staircase::staircase_info(sim.app.cx.floor(), lo).unwrap();
    assert_eq!((info.sections, info.landings), (2, 1));
}

#[test]
fn make_best_fit_gives_every_riser_the_height_nearest_six_and_three_quarters() {
    let mut sim = house();
    let (lo, up) = broken_stair(&mut sim);
    let before = stairs(&sim);
    let total: f64 = [lo, up]
        .iter()
        .map(|id| {
            view::find(sim.app.cx.floor(), *id)
                .unwrap()
                .stair
                .params
                .total_rise
        })
        .sum();
    let best = best_fit(total);
    let had: u32 = [lo, up]
        .iter()
        .map(|id| {
            view::find(sim.app.cx.floor(), *id)
                .unwrap()
                .solution()
                .risers
        })
        .sum();
    assert_ne!(had, best.risers, "the setup is off the Best Fit");
    sim.app.cx.selection.set(ObjectRef::Stair(lo));
    let enabled = view::edit_commands(&sim.app.cx)
        .into_iter()
        .any(|(c, on)| c == StairCommand::MakeBestFit && on);
    assert!(enabled);
    assert!(view::run_command(
        &mut sim.app.cx,
        StairCommand::MakeBestFit
    ));
    assert!(
        sim.app.cx.status.starts_with("Make Best Fit"),
        "{}",
        sim.app.cx.status
    );
    let (a, b) = (
        view::find(sim.app.cx.floor(), lo).unwrap(),
        view::find(sim.app.cx.floor(), up).unwrap(),
    );
    assert_eq!(a.solution().risers + b.solution().risers, best.risers);
    for o in [&a, &b] {
        assert!(close(o.solution().riser_height, best.riser_height));
    }
    // The landing and the upper flight follow the new heights.
    let landing = staircase::staircase(sim.app.cx.floor(), lo)
        .into_iter()
        .find(StairObj::is_landing)
        .unwrap();
    assert!(close(landing.landing_height(), a.top_height()));
    assert!(close(b.bottom_height(), a.top_height()));
    assert!(close(b.top_height(), total));
    // Already there: the command is off.
    sim.app.cx.selection.set(ObjectRef::Stair(lo));
    assert!(!view::edit_commands(&sim.app.cx)
        .into_iter()
        .any(|(c, on)| c == StairCommand::MakeBestFit && on));
    // One undo step.
    assert_eq!(sim.undo().as_deref(), Some("Make Best Fit"));
    assert_eq!(stairs(&sim), before);
}

#[test]
fn lock_top_and_lock_bottom_choose_which_end_of_a_section_moves() {
    let mut sim = house();
    let (lo, up) = broken_stair(&mut sim);
    let start = stairs(&sim);
    let upper = view::find(sim.app.cx.floor(), up).unwrap();
    let len = staircase::section_length(&upper);
    let top0 = plan_stairs::top_point(&upper.stair).0;
    let along = upper.along();

    // Lock Bottom: the top of the upper flight moves 20" on.
    let delta = staircase::resize_section(&mut sim.app.cx, up, len + 20.0, LockEnd::Bottom)
        .expect("resized");
    assert!(close(delta, 20.0));
    let moved = view::find(sim.app.cx.floor(), up).unwrap();
    assert!(close(moved.stair.origin.x, upper.stair.origin.x));
    assert!(close(moved.stair.origin.y, upper.stair.origin.y));
    let top1 = plan_stairs::top_point(&moved.stair).0;
    assert!(close(top1.dist(top0), 20.0));
    assert_eq!(sim.undo().as_deref(), Some("Stair Section Length"));
    assert_eq!(stairs(&sim), start);

    // Lock Top: the top stays; the section, the landing and the lower flight
    // move 20" back.
    staircase::resize_section(&mut sim.app.cx, up, len + 20.0, LockEnd::Top).unwrap();
    let moved = view::find(sim.app.cx.floor(), up).unwrap();
    let top2 = plan_stairs::top_point(&moved.stair).0;
    assert!(close(top2.dist(top0), 0.0), "{top2:?} {top0:?}");
    let back = along * -20.0;
    let lower_before = start.iter().find(|o| o.id() == lo).unwrap();
    let lower_after = view::find(sim.app.cx.floor(), lo).unwrap();
    let d = lower_after.stair.origin - lower_before.stair.origin;
    assert!(close(d.x, back.x) && close(d.y, back.y), "{d:?} {back:?}");
    assert_eq!(sim.undo().as_deref(), Some("Stair Section Length"));
    assert_eq!(stairs(&sim), start);
}

#[test]
fn the_tread_depth_mode_decides_what_a_new_length_changes() {
    let mut sim = house();
    let (_, up) = broken_stair(&mut sim);
    let o = view::find(sim.app.cx.floor(), up).unwrap();
    let (depth, treads) = (o.stair.params.tread_depth, o.solution().treads);
    let len = staircase::section_length(&o);

    // Automatic and Lock Number of Treads: the depth follows.
    let mut a = o.clone();
    staircase::set_length(&mut a, len + 30.0);
    assert_eq!(a.solution().treads, treads);
    assert!(close(
        a.stair.params.tread_depth,
        depth + 30.0 / f64::from(treads)
    ));

    // Lock Tread Depth: whole treads at a time, the riser height follows.
    assert!(staircase::set_tread_mode(
        &mut sim.app.cx,
        up,
        TreadMode::LockDepth
    ));
    let mut b = view::find(sim.app.cx.floor(), up).unwrap();
    assert_eq!(staircase::tread_mode(&b), TreadMode::LockDepth);
    staircase::set_length(&mut b, len + 2.0 * depth);
    assert!(close(b.stair.params.tread_depth, depth));
    assert_eq!(b.solution().treads, treads + 2);
    assert_eq!(sim.undo().as_deref(), Some("Tread Depth Mode"));

    // No Change refuses.
    let mut c = o.clone();
    c.x.lock_tread = true;
    c.x.lock_count = true;
    assert_eq!(staircase::tread_mode(&c), TreadMode::NoChange);
    let before = c.clone();
    staircase::set_length(&mut c, len + 50.0);
    assert_eq!(c, before);
}

#[test]
fn two_flights_end_to_end_merge_into_one_section_of_two_subsections() {
    let mut sim = house();
    let lower = flight(&mut sim, Point::new(60.0, 200.0), 0.0);
    let upper = flight(&mut sim, Point::new(130.0, 200.0), 52.5);
    let start = stairs(&sim);
    sim.app.cx.selection.set(ObjectRef::Stair(lower));
    let enabled = view::edit_commands(&sim.app.cx)
        .into_iter()
        .any(|(c, on)| c == StairCommand::MergeSections && on);
    assert!(enabled);
    assert_eq!(
        staircase::merge_partner(sim.app.cx.floor(), upper),
        Some((lower, upper))
    );
    assert!(view::run_command(
        &mut sim.app.cx,
        StairCommand::MergeSections
    ));
    let all = stairs(&sim);
    assert_eq!(all.len(), 1);
    let m = &all[0];
    assert_eq!(m.id(), lower);
    assert_eq!(solve(&m.stair.params).risers, 14);
    assert_eq!(m.stair.params.subsections, vec![6, 7]);
    let rows = staircase::spec_table(sim.app.cx.floor(), lower);
    assert_eq!(
        rows.iter().map(|r| r.number.as_str()).collect::<Vec<_>>(),
        ["1-1", "1-2"]
    );
    let labels = staircase::section_labels(sim.app.cx.floor(), lower);
    assert_eq!(
        labels.iter().map(|l| l.1.as_str()).collect::<Vec<_>>(),
        ["1-1", "1-2"]
    );
    // The 3D model rises to the top of the upper flight.
    let top = tagged_meshes(&m.stair)
        .iter()
        .filter_map(|(_, mesh)| mesh.bounds())
        .map(|(_, hi)| f64::from(hi[1]))
        .fold(f64::MIN, f64::max);
    assert!((105.0 - 1e-3..120.0).contains(&top), "{top}");
    // One undo step puts both flights back.
    assert_eq!(sim.undo().as_deref(), Some("Merge Sections"));
    assert_eq!(stairs(&sim), start);
    // A flight at the wrong height, or a ramp, does not merge.
    let high = flight(&mut sim, Point::new(300.0, 100.0), 10.0);
    assert!(staircase::merge_partner(sim.app.cx.floor(), high).is_none());
}

#[test]
fn adjacent_landings_stack_one_riser_up_and_lose_the_railing_between_them() {
    let mut sim = house();
    let fl = sim.app.cx.floor;
    let sq = |x: f64, y: f64| {
        vec![
            Point::new(x, y),
            Point::new(x + 36.0, y),
            Point::new(x + 36.0, y + 36.0),
            Point::new(x, y + 36.0),
        ]
    };
    let first = view::build_polygon_landing(&sim.app.cx.project, fl, &sq(100.0, 100.0));
    let a = view::add(&mut sim.app.cx.project, fl, first);
    view::update(&mut sim.app.cx.project, fl, a, |o| {
        o.stair.params.total_rise = 52.5
    });
    let second = view::build_polygon_landing(&sim.app.cx.project, fl, &sq(136.5, 100.0));
    let b = view::add(&mut sim.app.cx.project, fl, second);
    sim.app.cx.begin_change("Landing");
    view::connect(&mut sim.app.cx.project, fl, b);
    let (la, lb) = (
        view::find(sim.app.cx.floor(), a).unwrap(),
        view::find(sim.app.cx.floor(), b).unwrap(),
    );
    // The landing drawn first holds its height; the later one is a riser up.
    assert!(close(la.landing_height(), 52.5));
    assert!(close(lb.landing_height(), 60.0), "{}", lb.landing_height());
    // The shared edges carry no railing.
    assert!(lb.stair.params.edge_rails.contains(&EdgeRail::No));
    assert!(la.stair.params.edge_rails.contains(&EdgeRail::No));
    // Free-standing landings are 6 3/4" thick.
    assert!(close(lb.stair.params.slab_thickness, 6.75));
    // With Auto Adjust Height off the typed height holds.
    view::update(&mut sim.app.cx.project, fl, b, |o| {
        o.stair.params.landing_auto_height = false;
        o.stair.params.total_rise = 70.0;
    });
    view::connect(&mut sim.app.cx.project, fl, b);
    assert!(close(
        view::find(sim.app.cx.floor(), b).unwrap().landing_height(),
        70.0
    ));
    // A landing three inches away is not adjacent.
    let third = view::build_polygon_landing(&sim.app.cx.project, fl, &sq(300.0, 100.0));
    let c = view::add(&mut sim.app.cx.project, fl, third);
    let far = view::build_polygon_landing(&sim.app.cx.project, fl, &sq(339.0, 100.0));
    let d = view::add(&mut sim.app.cx.project, fl, far);
    view::connect(&mut sim.app.cx.project, fl, d);
    assert!(view::find(sim.app.cx.floor(), d)
        .unwrap()
        .stair
        .params
        .edge_rails
        .is_empty());
    assert_eq!(staircase::staircase(sim.app.cx.floor(), c).len(), 1);
    assert_eq!(staircase::staircase(sim.app.cx.floor(), a).len(), 2);
}

#[test]
fn the_specification_dialog_shows_the_best_fit_and_the_table() {
    let mut sim = house();
    let (lo, _) = broken_stair(&mut sim);
    let o = view::find(sim.app.cx.floor(), lo).unwrap();
    let mut d = StairDialog::new(o.clone());
    // Make Best Fit on the draft: its own rise takes the nearest count.
    let best = best_fit(o.stair.params.total_rise);
    if best.risers != o.solution().risers {
        assert!(view::best_fit_draft(d.draft_mut()));
        assert_eq!(d.draft().solution().risers, best.risers);
    }
    assert!(!view::best_fit_draft(d.draft_mut()) || best.risers == d.draft().solution().risers);
    // The dialog's lock end and Apply to All Connected Sections ride on the
    // draft and are stored by one OK.
    d.draft_mut().x.lock_end = LockEnd::Top;
    d.draft_mut().x.apply_display_all = true;
    d.draft_mut().stair.params.plan.floor_above = plan_stairs::DisplayRule::Always;
    assert!(view::apply_edit(&mut sim.app.cx, d.draft()));
    for s in staircase::staircase(sim.app.cx.floor(), lo) {
        assert_eq!(
            s.stair.params.plan.floor_above,
            plan_stairs::DisplayRule::Always,
            "{}",
            s.id()
        );
    }
    assert_eq!(sim.undo().as_deref(), Some("Stair Specification"));
    let landing_rule = staircase::staircase(sim.app.cx.floor(), lo)
        .into_iter()
        .find(StairObj::is_landing)
        .unwrap()
        .stair
        .params
        .plan
        .floor_above;
    assert_eq!(landing_rule, plan_stairs::DisplayRule::Automatic);
    let _ = StairShape::Straight;
}

#[test]
fn the_dialog_table_covers_the_whole_staircase_and_follows_the_draft() {
    let mut sim = house();
    let (lo, _) = broken_stair(&mut sim);
    let fl = sim.app.cx.floor();
    let secs = staircase::sections(fl, lo);
    let landings = staircase::staircase(fl, lo).len() - secs.len();
    assert!(secs.len() >= 2 && landings >= 1);
    let o = view::find(fl, lo).unwrap();
    let alone = StairDialog::new(o.clone()).table_rows();
    let mut d = StairDialog::new(o).with_staircase(secs.clone(), landings);
    let whole = d.table_rows();
    assert!(
        whole.len() > alone.len(),
        "{} vs {}",
        whole.len(),
        alone.len()
    );
    assert_eq!(
        whole,
        plan_stairs::spec_rows(&secs.iter().map(|s| &s.stair).collect::<Vec<_>>())
    );
    // An edit of the draft shows in its own lines of the table.
    d.draft_mut().stair.params.width += 6.0;
    assert_ne!(d.table_rows(), whole);
}

// ---- Round 17: sections apart, stairwell stop, Convert Polyline ----

#[test]
fn a_joined_staircase_moves_as_one_unless_a_section_is_moved_apart() {
    let mut sim = house();
    let (lo, up) = broken_stair(&mut sim);
    let fl = sim.app.cx.floor();
    let all = staircase::staircase(fl, lo);
    assert!(all.len() >= 3, "two sections and the landing between them");
    // Default: the whole staircase. Shift (or the preference) moves one.
    let mut together = staircase::move_group(fl, lo, false, false);
    together.sort_unstable();
    let mut every: Vec<u64> = all.iter().map(StairObj::id).collect();
    every.sort_unstable();
    assert_eq!(together, every);
    assert_eq!(staircase::move_group(fl, up, false, true), vec![up]);
    assert_eq!(staircase::move_group(fl, up, true, false), vec![up]);
    assert_eq!(staircase::move_group(fl, up, true, true).len(), every.len());

    // A drag of one section carries the others the same distance.
    let before = stairs(&sim);
    let c = plan_core::geometry::polygon_centroid(&view::find(sim.app.cx.floor(), lo).unwrap().footprint());
    sim.tool(ToolId::Select);
    sim.drag((c.x, c.y), (c.x + 24.0, c.y));
    let after = stairs(&sim);
    for b in &before {
        let a = after.iter().find(|o| o.id() == b.id()).unwrap();
        assert!(
            close(a.stair.origin.x - b.stair.origin.x, 24.0)
                && close(a.stair.origin.y, b.stair.origin.y),
            "object {} moved with its staircase",
            b.id()
        );
    }
    assert_eq!(sim.undo().as_deref(), Some("Move Stairs"));

    // With the preference on, only the dragged section moves.
    sim.app.cx.defaults.editing.behavior.stair_sections_independent = true;
    sim.drag((c.x, c.y), (c.x + 24.0, c.y));
    let moved = stairs(&sim)
        .iter()
        .zip(&before)
        .filter(|(a, b)| a.stair.origin != b.stair.origin)
        .count();
    assert_eq!(moved, 1);
}

#[test]
fn the_top_of_a_stair_stops_at_a_wall_of_the_stairwell_above() {
    let mut sim = house();
    sim.app.cx.project.floors.push(plan_core::Floor::new("Second", 108.0));
    let id = flight(&mut sim, Point::new(60.0, 100.0), 0.0);
    let fl = sim.app.cx.floor;
    view::update(&mut sim.app.cx.project, fl, id, |o| {
        o.stair.params.plan.floor_above = plan_stairs::DisplayRule::Always;
    });
    let orig = view::find(sim.app.cx.floor(), id).unwrap();
    // A wall across the path, 90 inches past the bottom, on the floor above.
    let mut w = sim.app.cx.project.floors[0].walls[0].clone();
    w.id = 9001;
    w.thickness = 6.0;
    w.start = Point::new(150.0, 0.0);
    w.end = Point::new(150.0, 400.0);
    sim.app.cx.project.floors[1].walls.push(w);
    let mut far = orig.clone();
    staircase::set_length(&mut far, 160.0);
    assert!(staircase::section_length(&far) > 150.0);
    let held = staircase::stairwell_stop(&sim.app.cx.project, fl, &orig, far.clone());
    let reach = staircase::section_length(&held);
    assert!(reach < 150.0 - 60.0 + 1e-6 + 0.0 || reach < 100.0, "stopped at the wall: {reach}");
    assert!(reach > 40.0);
    // A drag that stays short of the wall is untouched.
    let mut near = orig.clone();
    staircase::set_length(&mut near, 85.0);
    assert_eq!(staircase::stairwell_stop(&sim.app.cx.project, fl, &orig, near.clone()), near);
    // Never display on the floor above: no stairwell, no stop.
    view::update(&mut sim.app.cx.project, fl, id, |o| {
        o.stair.params.plan.floor_above = plan_stairs::DisplayRule::Never;
    });
    let orig = view::find(sim.app.cx.floor(), id).unwrap();
    assert_eq!(staircase::stairwell_stop(&sim.app.cx.project, fl, &orig, far.clone()), far);
}

#[test]
fn convert_polyline_to_landing_replaces_the_polyline_in_one_undo_step() {
    let mut sim = house();
    let fl = sim.app.cx.floor;
    let pts = vec![
        Point::new(100.0, 100.0),
        Point::new(200.0, 100.0),
        Point::new(200.0, 180.0),
        Point::new(100.0, 180.0),
    ];
    sim.app.cx.begin_change("Test polyline");
    sim.app.cx.project.floors[fl].cad.push(plan_core::CadObject {
        id: 777,
        layer: "CAD, Default".into(),
        item: plan_core::CadItem::Polyline { points: pts, closed: true },
    });
    sim.app.cx.selection.set(ObjectRef::Cad(777));
    assert_eq!(staircase::edit_buttons(&sim.app.cx).len(), 1);
    let n = sim.app.cx.floor().cad.len();
    sim.app.cx.run_custom(staircase::CONVERT_POLYLINE);
    assert_eq!(sim.app.cx.floor().cad.len(), n - 1);
    let landings: Vec<_> = stairs(&sim).into_iter().filter(StairObj::is_landing).collect();
    assert_eq!(landings.len(), 1);
    assert_eq!(landings[0].stair.params.outline.len(), 4);
    assert_eq!(sim.undo().as_deref(), Some("Convert Polyline to Landing"));
    assert_eq!(sim.app.cx.floor().cad.len(), n);
    assert!(stairs(&sim).iter().all(|o| !o.is_landing()));
    // Nothing to convert once something else is selected.
    sim.app.cx.selection.clear();
    assert!(staircase::edit_buttons(&sim.app.cx).is_empty());
}
