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

// ----- Round 17 brief 03: Edit Area variants, Transform/Replicate, positioning -----

mod r17_edit_area {
    use super::*;
    use crate::editor::edit_commands::ids;
    use crate::editor::transform::{self as xf, ReflectAxis, TransformParams};
    use crate::toolbar::Action;
    use crate::tools::select::{self, area};
    use plan_core::{Floor, WallKind};

    fn wall(sim: &mut Sim, floor: usize, a: Point, b: Point) -> Id {
        let id = sim
            .app
            .cx
            .project
            .add_wall(floor, a, b, 6.0, 96.0, WallKind::Exterior);
        sim.app.cx.refresh();
        id
    }

    fn walls_on(sim: &Sim, floor: usize) -> Vec<(Point, Point)> {
        sim.app.cx.project.floors[floor]
            .walls
            .iter()
            .map(|w| (w.start, w.end))
            .collect()
    }

    #[test]
    fn a_wall_through_the_area_is_cut_and_stays_connected() {
        let mut sim = Sim::new();
        sim.tool(ToolId::Select);
        let id = wall(&mut sim, 0, p(0.0, 0.0), p(240.0, 0.0));
        sim.action(Action::Custom(select::EDIT_AREA));
        sim.drag((80.0, -40.0), (160.0, 40.0));
        // Across the wall: the middle part moves, the outer parts follow it.
        let d = depth(&sim);
        let res = sim.drag((120.0, 20.0), (120.0, 44.0));
        assert_eq!(res.commit.as_deref(), Some("Move Edit Area"));
        assert_eq!(depth(&sim), d + 1, "one undo step");
        let w = walls_on(&sim, 0);
        assert_eq!(w.len(), 3, "cut into three walls: {w:?}");
        assert_eq!(w[0], (p(0.0, 0.0), p(80.0, 24.0)));
        assert_eq!(w[1], (p(80.0, 24.0), p(160.0, 24.0)));
        assert_eq!(w[2], (p(160.0, 24.0), p(240.0, 0.0)));
        sim.undo();
        assert_eq!(walls_on(&sim, 0), vec![(p(0.0, 0.0), p(240.0, 0.0))]);
        sim.esc();
        // Along the wall the parts line up again and rejoin as one wall.
        sim.action(Action::Custom(select::EDIT_AREA));
        sim.drag((80.0, -40.0), (160.0, 40.0));
        sim.drag((120.0, 20.0), (144.0, 20.0));
        assert_eq!(walls_on(&sim, 0), vec![(p(0.0, 0.0), p(240.0, 0.0))]);
        assert!(sim.app.cx.floor().wall(id).is_some());
    }

    #[test]
    fn a_door_in_the_moved_part_goes_with_it() {
        let mut sim = Sim::new();
        sim.tool(ToolId::Select);
        let id = wall(&mut sim, 0, p(0.0, 0.0), p(240.0, 0.0));
        sim.app
            .cx
            .project
            .add_opening(0, id, 120.0, plan_core::OpeningKind::Door);
        sim.app.cx.refresh();
        sim.action(Action::Custom(select::EDIT_AREA));
        sim.drag((80.0, -40.0), (160.0, 40.0));
        sim.drag((120.0, 20.0), (120.0, 44.0));
        let f = sim.app.cx.floor();
        let o = &f.openings[0];
        let host = f.wall(o.wall_id).unwrap();
        assert_eq!((host.start, host.end), (p(80.0, 24.0), p(160.0, 24.0)));
        assert!((o.center_offset - 40.0).abs() < 1e-6);
    }

    #[test]
    fn all_floors_moves_every_floor_in_one_undo_step() {
        let mut sim = Sim::new();
        sim.tool(ToolId::Select);
        sim.app.cx.project.floors.push(Floor::new("Second", 108.0));
        wall(&mut sim, 0, p(100.0, 10.0), p(140.0, 10.0));
        wall(&mut sim, 1, p(100.0, 20.0), p(140.0, 20.0));
        // The current floor only.
        sim.action(Action::Custom(select::EDIT_AREA));
        sim.drag((60.0, -20.0), (200.0, 60.0));
        sim.drag((120.0, 15.0), (120.0, 39.0));
        assert_eq!(walls_on(&sim, 0)[0].0, p(100.0, 34.0));
        assert_eq!(walls_on(&sim, 1)[0].0, p(100.0, 20.0));
        sim.undo();
        sim.esc();
        let d = depth(&sim);
        sim.action(Action::Custom(select::EDIT_AREA_ALL));
        sim.drag((60.0, -20.0), (200.0, 60.0));
        sim.drag((120.0, 15.0), (120.0, 39.0));
        assert_eq!(walls_on(&sim, 0)[0].0, p(100.0, 34.0));
        assert_eq!(walls_on(&sim, 1)[0].0, p(100.0, 44.0));
        assert_eq!(depth(&sim), d + 1);
        assert_eq!(sim.app.cx.floor, 0, "back on the floor it started on");
        sim.undo();
        assert_eq!(walls_on(&sim, 1)[0].0, p(100.0, 20.0));
        sim.esc();
        // Delete takes both floors' contents.
        sim.action(Action::Custom(select::EDIT_AREA_ALL_VISIBLE));
        sim.drag((60.0, -20.0), (200.0, 60.0));
        sim.key(crate::tools::KeyEvent::key(eframe::egui::Key::Delete));
        assert!(walls_on(&sim, 0).is_empty() && walls_on(&sim, 1).is_empty());
    }

    #[test]
    fn a_selected_closed_polyline_is_the_marquee_and_moves_only_when_included() {
        for including in [false, true] {
            let mut sim = Sim::new();
            sim.tool(ToolId::Select);
            let inside = wall(&mut sim, 0, p(10.0, 30.0), p(40.0, 30.0));
            let outside = wall(&mut sim, 0, p(300.0, 50.0), p(360.0, 50.0));
            // A triangle: the corner (100, 100) is outside it.
            let poly = add(
                &mut sim,
                CadItem::Polyline {
                    points: vec![p(0.0, 0.0), p(100.0, 0.0), p(0.0, 100.0)],
                    closed: true,
                },
            );
            let corner = wall(&mut sim, 0, p(90.0, 90.0), p(110.0, 90.0));
            select(&mut sim, poly);
            let cmd = if including {
                select::EDIT_AREA_INCLUDING
            } else {
                select::EDIT_AREA
            };
            sim.action(Action::Custom(cmd));
            assert!(area::region().is_some(), "the polyline is the marquee");
            let d = depth(&sim);
            sim.drag((20.0, 20.0), (20.0, 44.0));
            assert_eq!(depth(&sim), d + 1);
            let f = sim.app.cx.floor();
            assert_eq!(f.wall(inside).unwrap().start, p(10.0, 54.0));
            assert_eq!(f.wall(outside).unwrap().start, p(300.0, 50.0));
            assert_eq!(f.wall(corner).unwrap().start, p(90.0, 90.0));
            let pts = match item(&sim, poly) {
                CadItem::Polyline { points, .. } => points,
                _ => unreachable!(),
            };
            assert_eq!(pts[0], if including { p(0.0, 24.0) } else { p(0.0, 0.0) });
        }
    }

    #[test]
    fn transform_replicate_moves_to_a_place_and_reflects_about_a_line() {
        let mut sim = Sim::new();
        let a = add(
            &mut sim,
            CadItem::Line {
                a: p(0.0, 0.0),
                b: p(20.0, 0.0),
            },
        );
        select(&mut sim, a);
        // Move the center (10, 0) to (110, 50).
        let to = TransformParams {
            move_x: 110.0,
            move_y: 50.0,
            move_to: true,
            ..Default::default()
        };
        xf::transform_replicate(&mut sim.app.cx, &to).unwrap();
        assert_eq!(
            item(&sim, a),
            CadItem::Line {
                a: p(100.0, 50.0),
                b: p(120.0, 50.0)
            }
        );
        // Mirror a copy about the vertical line through two points, then three
        // copies with a growing offset (each copy applies the step k times).
        let mirror = TransformParams {
            copies: 1,
            reflect: Some(ReflectAxis::Line(p(50.0, 0.0), p(50.0, 10.0))),
            ..Default::default()
        };
        xf::transform_replicate(&mut sim.app.cx, &mirror).unwrap();
        assert_eq!(sim.app.cx.floor().cad.len(), 2);
        select(&mut sim, a);
        let n = TransformParams {
            copies: 3,
            move_y: 10.0,
            ..Default::default()
        };
        xf::transform_replicate(&mut sim.app.cx, &n).unwrap();
        assert_eq!(sim.app.cx.floor().cad.len(), 5);
        // Along its own direction: a 45 degree line moved 10 ahead.
        let b = add(
            &mut sim,
            CadItem::Line {
                a: p(0.0, 0.0),
                b: p(10.0, 10.0),
            },
        );
        select(&mut sim, b);
        let own = TransformParams {
            move_x: 10.0,
            move_frame: xf::selection_angle(&sim.app.cx),
            ..Default::default()
        };
        xf::transform_replicate(&mut sim.app.cx, &own).unwrap();
        let CadItem::Line { a: s, .. } = item(&sim, b) else {
            unreachable!()
        };
        assert!((s.x - 7.0710678).abs() < 1e-4 && (s.y - 7.0710678).abs() < 1e-4);
    }

    #[test]
    fn center_on_a_line_and_between_two_points() {
        let mut sim = Sim::new();
        sim.tool(ToolId::Select);
        let c = add(
            &mut sim,
            CadItem::Circle {
                center: p(50.0, 30.0),
                radius: 5.0,
            },
        );
        let l = add(
            &mut sim,
            CadItem::Line {
                a: p(0.0, 100.0),
                b: p(200.0, 100.0),
            },
        );
        let _ = l;
        select(&mut sim, c);
        sim.app.cx.run_custom(ids::CENTER);
        let d = depth(&sim);
        let res = sim.click(150.0, 100.0);
        assert_eq!(res.commit.as_deref(), Some("Center Object"));
        assert_eq!(depth(&sim), d + 1);
        assert_eq!(
            item(&sim, c),
            CadItem::Circle {
                center: p(50.0, 100.0),
                radius: 5.0
            }
        );
        // Point to Point Center: the circle goes to the midpoint of two clicks.
        select(&mut sim, c);
        sim.app.cx.run_custom(ids::POINT_TO_POINT_CENTER);
        sim.click(200.0, 200.0);
        let res = sim.click(300.0, 240.0);
        assert_eq!(res.commit.as_deref(), Some("Point to Point Center"));
        assert_eq!(
            item(&sim, c),
            CadItem::Circle {
                center: p(250.0, 220.0),
                radius: 5.0
            }
        );
        assert_eq!(sim.undo().as_deref(), Some("Point to Point Center"));
    }

    #[test]
    fn the_edit_toolbar_orders_align_after_center_object() {
        let mut sim = Sim::new();
        let a = add(
            &mut sim,
            CadItem::Line {
                a: p(0.0, 0.0),
                b: p(20.0, 0.0),
            },
        );
        let b = add(
            &mut sim,
            CadItem::Line {
                a: p(0.0, 40.0),
                b: p(20.0, 60.0),
            },
        );
        sim.app.cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(b)];
        let names: Vec<String> = sim
            .app
            .cx
            .selection_edit_actions()
            .iter()
            .map(|e| e.label.to_string())
            .collect();
        let at = |n: &str| {
            names
                .iter()
                .position(|x| x == n)
                .unwrap_or_else(|| panic!("{n} in {names:?}"))
        };
        assert!(at("Center Object") < at("Point to Point Center"));
        assert!(at("Point to Point Center") < at("Align/Distribute"));
        assert!(at("Align/Distribute") < at("Make Parallel"));
    }
}

#[test]
fn customize_hotkeys_lists_the_cad_edit_survey_and_point_to_point_center_commands() {
    use crate::editor::edit_commands::ids;
    use crate::shell::hotkeys::collect_commands;
    use crate::toolbar::Action;
    let cmds = collect_commands();
    let has = |id: &str| {
        cmds.iter()
            .any(|c| matches!(c.action, Action::Custom(x) if x == id))
    };
    assert!(has(ids::POINT_TO_POINT_CENTER));
    for (_, id) in EDIT_COMMANDS {
        assert!(has(id), "{id} can take a hotkey");
    }
    for (_, id) in crate::tools::cad::survey::MENU {
        assert!(has(id), "{id} can take a hotkey");
    }
    // Names stay unique so a binding finds one command.
    let mut names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    let n = names.len();
    names.dedup();
    assert_eq!(names.len(), n);
}
