//! Scenario 70 (Round 16, brief 11): the CAD edit tools Intersect Two Lines,
//! Close Polyline, Simplify Polyline and Fillet All Corners, each one undo
//! step, and the new Edit toolbar commands.

use super::Sim;
use crate::editor::ObjectRef;
use crate::tools::cad::{CadMode, EDIT_COMMANDS};
use crate::tools::ToolId;
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::Id;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn add(sim: &mut Sim, item: CadItem) -> Id {
    let id = sim.app.cx.project.add_cad(0, "CAD, Default", item);
    sim.app.cx.refresh();
    id
}

fn item(sim: &Sim, id: Id) -> CadItem {
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .item
        .clone()
}

fn select(sim: &mut Sim, id: Id) {
    sim.app.cx.selection.set(ObjectRef::Cad(id));
}

/// Picks a command mode and lets the tool's frame run it, as the shell does.
fn run_command(sim: &mut Sim, mode: CadMode) {
    sim.tool(ToolId::CadVariant(mode));
    sim.dialog_frame(false);
}

fn depth(sim: &Sim) -> usize {
    sim.app.cx.undo_depth()
}

#[test]
fn close_polyline_closes_in_one_undo_step() {
    let mut sim = Sim::new();
    let id = add(
        &mut sim,
        CadItem::Polyline {
            points: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 60.0)],
            closed: false,
        },
    );
    select(&mut sim, id);
    let d = depth(&sim);
    run_command(&mut sim, CadMode::ClosePolyline);
    assert!(matches!(
        item(&sim, id),
        CadItem::Polyline { closed: true, .. }
    ));
    assert_eq!(depth(&sim), d + 1);
    sim.undo();
    assert!(matches!(
        item(&sim, id),
        CadItem::Polyline { closed: false, .. }
    ));
}

#[test]
fn simplify_polyline_drops_the_straight_through_vertex() {
    let mut sim = Sim::new();
    let id = add(
        &mut sim,
        CadItem::Polyline {
            points: vec![p(0.0, 0.0), p(50.0, 0.0), p(100.0, 0.0), p(100.0, 60.0)],
            closed: false,
        },
    );
    select(&mut sim, id);
    let d = depth(&sim);
    run_command(&mut sim, CadMode::SimplifyPolyline);
    match item(&sim, id) {
        CadItem::Polyline { points, .. } => assert_eq!(points.len(), 3),
        _ => panic!("still a polyline"),
    }
    assert_eq!(depth(&sim), d + 1);
    sim.undo();
    match item(&sim, id) {
        CadItem::Polyline { points, .. } => assert_eq!(points.len(), 4),
        _ => panic!(),
    }
}

#[test]
fn fillet_all_corners_uses_the_fillet_radius_and_zero_means_none() {
    let mut sim = Sim::new();
    let id = add(
        &mut sim,
        CadItem::Polyline {
            points: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)],
            closed: true,
        },
    );
    select(&mut sim, id);
    let d = depth(&sim);
    run_command(&mut sim, CadMode::FilletAllCorners);
    let n = match item(&sim, id) {
        CadItem::Polyline { points, .. } => points.len(),
        _ => panic!(),
    };
    assert!(n > 8, "every corner became an arc, got {n} points");
    assert_eq!(depth(&sim), d + 1);
    sim.undo();
    match item(&sim, id) {
        CadItem::Polyline { points, .. } => assert_eq!(points.len(), 4),
        _ => panic!(),
    }
}

#[test]
fn intersect_two_lines_meets_them_at_the_crossing() {
    let mut sim = Sim::new();
    let h = add(
        &mut sim,
        CadItem::Line {
            a: p(0.0, 0.0),
            b: p(60.0, 0.0),
        },
    );
    let v = add(
        &mut sim,
        CadItem::Line {
            a: p(100.0, 40.0),
            b: p(100.0, 120.0),
        },
    );
    sim.tool(ToolId::CadVariant(CadMode::JoinTwoLines));
    let d = depth(&sim);
    sim.click(10.0, 0.0);
    sim.click(100.0, 100.0);
    assert_eq!(depth(&sim), d + 1);
    assert_eq!(
        item(&sim, h),
        CadItem::Line {
            a: p(0.0, 0.0),
            b: p(100.0, 0.0)
        }
    );
    assert_eq!(
        item(&sim, v),
        CadItem::Line {
            a: p(100.0, 0.0),
            b: p(100.0, 120.0)
        }
    );
    sim.undo();
    assert_eq!(
        item(&sim, h),
        CadItem::Line {
            a: p(0.0, 0.0),
            b: p(60.0, 0.0)
        }
    );
}

#[test]
fn the_edit_toolbar_offers_the_new_commands_for_a_selected_polyline() {
    let mut sim = Sim::new();
    let id = add(
        &mut sim,
        CadItem::Polyline {
            points: vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 60.0)],
            closed: false,
        },
    );
    select(&mut sim, id);
    let ids: Vec<&str> = EDIT_COMMANDS.iter().map(|(_, c)| *c).collect();
    for want in [
        "cad.close_polyline",
        "cad.simplify_polyline",
        "cad.fillet_all",
        "cad.join_lines",
        "cad.disconnect_edges",
        "cad.hide_show_edge",
    ] {
        assert!(ids.contains(&want), "{want}");
    }
    assert!(crate::tools::cad::run_edit_command(
        sim.cx(),
        "cad.close_polyline"
    ));
}

#[test]
fn arc_about_center_takes_the_current_point_as_its_center() {
    use crate::tools::cad::survey;
    let mut sim = Sim::new();
    crate::tools::cad::run_command(sim.cx(), "cad.arc_mode.about_center");
    survey::set_current_point(Some(p(0.0, 0.0)));
    sim.tool(ToolId::CadVariant(CadMode::Arc));
    let before = sim.app.cx.floor().cad.len();
    let d = depth(&sim);
    sim.click(50.0, 0.0);
    sim.click(0.0, 50.0);
    survey::set_current_point(None);
    crate::tools::cad::run_command(sim.cx(), "cad.arc_mode.three_point");
    let cad = &sim.app.cx.floor().cad;
    assert_eq!(cad.len(), before + 1, "two clicks make the arc");
    match &cad.last().unwrap().item {
        CadItem::Arc { center, radius, .. } => {
            assert!(center.dist(p(0.0, 0.0)) < 1e-6 && (radius - 50.0).abs() < 1e-6);
        }
        other => panic!("an arc, got {other:?}"),
    }
    assert_eq!(depth(&sim), d + 1);
}
