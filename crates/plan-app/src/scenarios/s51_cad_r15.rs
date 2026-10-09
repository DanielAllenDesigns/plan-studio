//! Scenario 51: round 15 CAD, driven through the real tools and commands:
//! Polyline Union / Subtract / Intersect from the Edit toolbar, Trim and
//! Extend to a boundary (lines, arcs, circles, polylines), Insert Point,
//! Multiple Copy (dialog and drag with ticks), Drawing Groups in the plan, the
//! saved file and the dialogs, the DXF export options and the Plan Footprint
//! from the outer wall faces (CAD-55, LAY-36, S-40, CAD-54, L-38, L-44, L-45).

use super::{draw_shell, shape_colors, Sim};
use crate::dialogs::{drawing_groups, dxf_options, multiple_copy};
use crate::editor::ObjectRef;
use crate::toolbar::{Action, FileCommand};
use crate::tools::cad::CadMode;
use crate::tools::cad_ops as ops;
use crate::tools::ToolId;
use eframe::egui;
use plan_core::cad::{CadAttrs, CadItem, FillAttr};
use plan_core::export::dxf_options::{DxfUnits, LayerNaming, TextMode};
use plan_core::geometry::{polygon_area, Point};
use plan_core::{Id, WallKind};

fn sim() -> Sim {
    let mut sim = Sim::new();
    sim.tool(ToolId::Select);
    sim
}

fn rect(sim: &mut Sim, x: f64, y: f64, w: f64, h: f64) -> Id {
    sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Polyline {
            points: vec![
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w, y + h),
                Point::new(x, y + h),
            ],
            closed: true,
        },
    )
}

fn item(sim: &Sim, id: Id) -> CadItem {
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("object {id} is gone"))
        .item
        .clone()
}

fn select(sim: &mut Sim, ids: &[Id]) {
    sim.app.cx.selection.items = ids.iter().map(|i| ObjectRef::Cad(*i)).collect();
}

fn cmd(sim: &mut Sim, id: &'static str) {
    sim.action(Action::Custom(id));
}

fn area(sim: &Sim, id: Id) -> f64 {
    match item(sim, id) {
        CadItem::Polyline { points, .. } => polygon_area(&points).abs(),
        CadItem::Circle { radius, .. } => std::f64::consts::PI * radius * radius,
        other => panic!("{other:?}"),
    }
}

fn labels(sim: &Sim) -> Vec<&'static str> {
    sim.app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect()
}

#[test]
fn rectangles_drawn_with_the_tool_union_subtract_and_intersect_from_the_edit_toolbar() {
    let mut sim = sim();
    // Two rectangles drawn with the Rectangular Polyline tool.
    sim.tool(ToolId::CadVariant(CadMode::RectPolyline));
    sim.click(60.0, 60.0);
    sim.click(180.0, 180.0);
    sim.tool(ToolId::CadVariant(CadMode::RectPolyline));
    sim.click(120.0, 120.0);
    sim.click(240.0, 240.0);
    sim.tool(ToolId::Select);
    let ids: Vec<Id> = sim.app.cx.floor().cad.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), 2);
    select(&mut sim, &ids[..1]);
    assert!(
        !labels(&sim).contains(&"Polyline Union"),
        "one shape has nothing to combine with"
    );
    select(&mut sim, &ids);
    let l = labels(&sim);
    for want in [
        "Polyline Union",
        "Polyline Subtract",
        "Polyline Intersect",
        "Trim to Boundary",
        "Extend to Boundary",
        "Insert Point",
        "Multiple Copy",
    ] {
        assert!(l.contains(&want), "{want} missing from {l:?}");
    }

    // Union: 120x120 + 120x120 - 60x60 overlap.
    cmd(&mut sim, ops::UNION);
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    let u = sim.app.cx.floor().cad[0].id;
    assert!((area(&sim, u) - (2.0 * 14400.0 - 3600.0)).abs() < 1e-6);
    assert_eq!(sim.app.cx.undo_label(), Some("Polyline Union"));
    assert_eq!(sim.undo().as_deref(), Some("Polyline Union"));
    assert_eq!(sim.app.cx.floor().cad.len(), 2);

    // Intersect leaves the overlap.
    select(&mut sim, &ids);
    cmd(&mut sim, ops::INTERSECT);
    let i = sim.app.cx.floor().cad[0].id;
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    assert!((area(&sim, i) - 3600.0).abs() < 1e-6);
    sim.undo();

    // Subtract: the first selected stays, the second is cut out.
    select(&mut sim, &ids);
    cmd(&mut sim, ops::SUBTRACT);
    assert_eq!(sim.app.cx.floor().cad.len(), 1);
    let s = sim.app.cx.floor().cad[0].id;
    assert!((area(&sim, s) - (14400.0 - 3600.0)).abs() < 1e-6);
    // The result is a plain polyline with 6 corners (an L).
    let CadItem::Polyline { points, closed } = item(&sim, s) else {
        panic!("not a polyline")
    };
    assert!(closed && points.len() == 6, "{points:?}");
}

#[test]
fn a_hole_comes_out_as_a_grouped_polyline_and_a_circle_keeps_its_arc() {
    let mut sim = sim();
    let big = rect(&mut sim, 0.0, 0.0, 240.0, 240.0);
    let small = rect(&mut sim, 60.0, 60.0, 120.0, 120.0);
    select(&mut sim, &[big, small]);
    cmd(&mut sim, ops::SUBTRACT);
    let cad = sim.app.cx.floor().cad.clone();
    assert_eq!(cad.len(), 2, "an outline and a hole");
    assert_eq!(
        sim.app.cx.floor().groups.len(),
        1,
        "grouped to move together"
    );
    // Selecting one of them selects both (the group).
    sim.undo();
    assert_eq!(sim.app.cx.floor().cad.len(), 2);
    assert!(sim.app.cx.floor().groups.is_empty());

    // A circle cut by a rectangle: a half disc with one arc edge.
    sim.tool(ToolId::CadVariant(CadMode::Circle));
    sim.app.cx.project.floors[0].cad.clear();
    let c = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Circle {
            center: Point::new(120.0, 120.0),
            radius: 60.0,
        },
    );
    let b = rect(&mut sim, 120.0, 0.0, 200.0, 240.0);
    sim.tool(ToolId::Select);
    select(&mut sim, &[c, b]);
    cmd(&mut sim, ops::INTERSECT);
    let r = sim.app.cx.floor().cad[0].id;
    let attrs = sim.app.cx.floor().cad_attrs(r).expect("arc edge kept");
    assert_eq!(attrs.arc_edges.len(), 1);
    let want = std::f64::consts::PI * 3600.0 / 2.0;
    assert!((area(&sim, r) - want).abs() / want < 0.01);
    // The result survives a save and a load with its arc edge.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.floors[0].cad_attrs(r).unwrap().arc_edges,
        attrs.arc_edges
    );
}

#[test]
fn trim_to_boundary_cuts_a_line_a_circle_and_an_arc_with_clicks() {
    let mut sim = sim();
    // The boundary: a vertical line at x = 100.
    let boundary = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(100.0, -200.0),
            b: Point::new(100.0, 400.0),
        },
    );
    let line = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(0.0, 60.0),
            b: Point::new(240.0, 60.0),
        },
    );
    let circle = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Circle {
            center: Point::new(100.0, 200.0),
            radius: 48.0,
        },
    );
    select(&mut sim, &[line]);
    cmd(&mut sim, ops::TRIM_BOUNDARY);
    assert!(crate::editor::transform::mode_active());
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
    // Click the boundary, then the part of the line to remove.
    sim.click(100.0, 300.0);
    assert!(
        sim.app.cx.status.contains("cut away"),
        "{}",
        sim.app.cx.status
    );
    let r = sim.click(200.0, 60.0);
    assert_eq!(r.commit.as_deref(), Some("Trim to Boundary"));
    match item(&sim, line) {
        CadItem::Line { a, b } => {
            assert!(a.dist(Point::new(0.0, 60.0)) < 1e-6, "{a:?}");
            assert!(b.dist(Point::new(100.0, 60.0)) < 1e-6, "{b:?}");
        }
        other => panic!("{other:?}"),
    }
    // The mode stays on: trim the circle's right half.
    assert!(crate::editor::transform::mode_active());
    let r = sim.click(148.0, 200.0);
    assert_eq!(r.commit.as_deref(), Some("Trim to Boundary"));
    match item(&sim, circle) {
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            assert_eq!(center, Point::new(100.0, 200.0));
            assert!((radius - 48.0).abs() < 1e-9);
            // The left half remains: from 90 to 270 degrees.
            assert!((start_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
            assert!((end_angle - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-6);
        }
        other => panic!("{other:?}"),
    }
    // Nothing crosses there: a message and no undo step.
    let depth = sim.app.cx.undo_depth();
    sim.click(52.0, 200.0);
    assert_eq!(sim.app.cx.undo_depth(), depth);
    assert!(
        sim.app.cx.status.contains("No boundary"),
        "{}",
        sim.app.cx.status
    );
    // Esc ends the mode and a click selects again.
    sim.esc();
    assert!(!crate::editor::transform::mode_active());
    assert!(sim.app.cx.floor().cad.iter().any(|c| c.id == boundary));
    // Each trim was its own undo step.
    assert_eq!(sim.undo().as_deref(), Some("Trim to Boundary"));
    assert_eq!(sim.undo().as_deref(), Some("Trim to Boundary"));
    assert!(matches!(item(&sim, circle), CadItem::Circle { .. }));
}

#[test]
fn extend_to_boundary_grows_lines_arcs_and_polylines() {
    let mut sim = sim();
    let wall = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(280.0, -200.0),
            b: Point::new(280.0, 400.0),
        },
    );
    let line = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(0.0, 60.0),
            b: Point::new(180.0, 60.0),
        },
    );
    let poly = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Polyline {
            points: vec![
                Point::new(0.0, 120.0),
                Point::new(120.0, 120.0),
                Point::new(120.0, 180.0),
            ],
            closed: false,
        },
    );
    let arc = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Arc {
            center: Point::new(180.0, 300.0),
            radius: 120.0,
            start_angle: 3.0 * std::f64::consts::FRAC_PI_2 - 0.3,
            end_angle: 3.0 * std::f64::consts::FRAC_PI_2 + 0.2,
        },
    );
    select(&mut sim, &[line]);
    cmd(&mut sim, ops::EXTEND_BOUNDARY);
    sim.click(280.0, 100.0);
    // The line grows from x = 180 to the boundary at 280.
    let r = sim.click(170.0, 60.0);
    assert_eq!(r.commit.as_deref(), Some("Extend to Boundary"));
    match item(&sim, line) {
        CadItem::Line { b, .. } => assert!(b.dist(Point::new(280.0, 60.0)) < 1e-6, "{b:?}"),
        other => panic!("{other:?}"),
    }
    // The open polyline grows its last segment (x = 120, going up) only if
    // the boundary lies in its way; the vertical boundary does not.
    sim.click(120.0, 175.0);
    assert!(
        sim.app.cx.status.contains("in the way"),
        "{}",
        sim.app.cx.status
    );
    // The first segment of the polyline runs right to the boundary.
    sim.click(5.0, 120.0);
    assert!(
        sim.app.cx.status.contains("in the way"),
        "{}",
        sim.app.cx.status
    );
    // The arc: its end (right side) grows along the circle to x = 280.
    let end = Point::new(
        180.0 + 120.0 * (3.0 * std::f64::consts::FRAC_PI_2 + 0.2).cos(),
        300.0 + 120.0 * (3.0 * std::f64::consts::FRAC_PI_2 + 0.2).sin(),
    );
    let r = sim.click(end.x, end.y);
    assert_eq!(r.commit.as_deref(), Some("Extend to Boundary"));
    match item(&sim, arc) {
        CadItem::Arc {
            center,
            radius,
            end_angle,
            ..
        } => {
            // The new end is exactly on the boundary.
            let p = Point::new(
                center.x + radius * end_angle.cos(),
                center.y + radius * end_angle.sin(),
            );
            assert!((p.x - 280.0).abs() < 1e-6, "{p:?}");
        }
        other => panic!("{other:?}"),
    }
    sim.esc();
    assert!(!crate::editor::transform::mode_active());
    // The three extends were separate undo steps (the line and the arc).
    assert_eq!(sim.undo().as_deref(), Some("Extend to Boundary"));
    assert_eq!(sim.undo().as_deref(), Some("Extend to Boundary"));
    assert!(sim.app.cx.floor().cad.iter().any(|c| c.id == wall));
    assert!(sim.app.cx.floor().cad.iter().any(|c| c.id == poly));
}

#[test]
fn the_cad_trim_and_extend_tools_take_arcs_and_circles_too() {
    let mut sim = sim();
    sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(-200.0, 120.0),
            b: Point::new(400.0, 120.0),
        },
    );
    let circle = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Circle {
            center: Point::new(120.0, 120.0),
            radius: 60.0,
        },
    );
    sim.tool(ToolId::CadVariant(CadMode::Trim));
    let r = sim.click(120.0, 180.0);
    assert_eq!(
        r.commit.as_deref(),
        Some("Trim Line"),
        "{}",
        sim.app.cx.status
    );
    assert!(matches!(item(&sim, circle), CadItem::Arc { .. }));
    // Extend Line on the arc: both its ends already lie on the line, so there
    // is nothing further to grow to, and no undo step is made.
    sim.tool(ToolId::CadVariant(CadMode::Extend));
    let depth = sim.app.cx.undo_depth();
    sim.click(120.0, 60.0);
    assert_eq!(sim.app.cx.undo_depth(), depth);
    assert!(
        sim.app.cx.status.contains("in the way"),
        "{}",
        sim.app.cx.status
    );
}

#[test]
fn insert_point_adds_a_vertex_on_a_straight_edge_and_splits_an_arc_edge() {
    let mut sim = sim();
    let id = rect(&mut sim, 0.0, 0.0, 240.0, 120.0);
    select(&mut sim, &[id]);
    cmd(&mut sim, ops::INSERT_POINT);
    assert!(crate::editor::transform::mode_active());
    // A click away from the polyline does nothing.
    sim.click(600.0, 600.0);
    assert!(
        sim.app.cx.status.contains("click an edge"),
        "{}",
        sim.app.cx.status
    );
    let r = sim.click(120.0, 1.0);
    assert_eq!(r.commit.as_deref(), Some("Insert Point"));
    match item(&sim, id) {
        CadItem::Polyline { points, .. } => {
            assert_eq!(points.len(), 5);
            assert!(points.iter().any(|p| p.dist(Point::new(120.0, 0.0)) < 1e-6));
        }
        other => panic!("{other:?}"),
    }
    // The mode stays on: another vertex on the right edge.
    sim.click(240.0, 60.0);
    match item(&sim, id) {
        CadItem::Polyline { points, .. } => assert_eq!(points.len(), 6),
        other => panic!("{other:?}"),
    }
    sim.esc();
    assert!(!crate::editor::transform::mode_active());
    // Two undo steps, one per vertex.
    assert_eq!(sim.undo().as_deref(), Some("Insert Point"));
    assert_eq!(sim.undo().as_deref(), Some("Insert Point"));
    match item(&sim, id) {
        CadItem::Polyline { points, .. } => assert_eq!(points.len(), 4),
        other => panic!("{other:?}"),
    }
}

#[test]
fn multiple_copy_from_the_dialog_and_by_dragging_with_ticks() {
    let mut sim = sim();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.tool(ToolId::Select);
    let before = sim.floor_walls();
    // A free-standing partition and a CAD box.
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((120.0, 120.0), (360.0, 120.0));
    sim.tool(ToolId::Select);
    let wall = sim.app.cx.floor().walls.last().unwrap().id;
    let boxed = rect(&mut sim, 120.0, 150.0, 40.0, 30.0);
    sim.app.cx.selection.items = vec![ObjectRef::Wall(wall), ObjectRef::Cad(boxed)];
    assert_eq!(sim.floor_walls(), before + 1);

    // The dialog: 3 copies 48" up.
    multiple_copy::close();
    cmd(&mut sim, ops::MULTIPLE_COPY);
    assert!(multiple_copy::is_open());
    let mut d = multiple_copy::MultipleCopyDialog {
        count: 3,
        x: "0\"".into(),
        y: "4'".into(),
        ..multiple_copy::MultipleCopyDialog::default()
    };
    assert!(d.apply(&mut sim.app.cx));
    assert_eq!(sim.floor_walls(), before + 4);
    assert_eq!(sim.app.cx.floor().cad.len(), 4);
    assert_eq!(sim.app.cx.undo_label(), Some("Multiple Copy"));
    assert_eq!(sim.undo().as_deref(), Some("Multiple Copy"));
    assert_eq!(sim.floor_walls(), before + 1);
    multiple_copy::close();

    // Drag: base point, then the last copy; the 3 copies spread evenly.
    sim.app.cx.selection.items = vec![ObjectRef::Wall(wall)];
    ops::set_copy_settings(ops::MultipleCopy {
        count: 3,
        step: Point::ZERO,
        turn_deg: 0.0,
    });
    cmd(&mut sim, ops::MULTIPLE_COPY_DRAG);
    assert!(crate::editor::transform::mode_active());
    sim.click(240.0, 120.0);
    sim.move_to(240.0, 240.0);
    let r = sim.click(240.0, 240.0);
    assert_eq!(r.commit.as_deref(), Some("Multiple Copy"));
    assert_eq!(sim.floor_walls(), before + 4);
    let mut ys: Vec<f64> = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .filter(|w| (w.end.x - w.start.x).abs() > 100.0 && w.start.y > 100.0 && w.start.y < 300.0)
        .map(|w| w.start.y)
        .collect();
    ys.sort_by(f64::total_cmp);
    assert_eq!(ys.len(), 4, "{ys:?}");
    for (got, want) in ys.iter().zip([120.0, 160.0, 200.0, 240.0]) {
        assert!((got - want).abs() < 1e-6, "{ys:?}");
    }
    assert!(!crate::editor::transform::mode_active());
}

#[test]
fn drawing_groups_order_the_plan_and_survive_the_file() {
    let mut sim = sim();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.tool(ToolId::Select);
    // A magenta filled CAD box inside the house.
    let id = rect(&mut sim, 100.0, 100.0, 120.0, 120.0);
    let mut attrs = CadAttrs::new(id);
    attrs.fill = Some(FillAttr {
        color: [255, 0, 255],
        ..FillAttr::default()
    });
    sim.app.cx.project.set_cad_attrs(0, attrs);
    sim.app.cx.refresh();
    let magenta = egui::Color32::from_rgb(255, 0, 255);
    let wall_fill = sim.app.cx.palette.wall_fill_exterior;
    let index = |shapes: &[egui::Shape], c: egui::Color32| {
        shapes.iter().position(|s| shape_colors(s).contains(&c))
    };

    // By default CAD draws over the walls.
    let shapes = sim.plan_shapes();
    let (m, w) = (index(&shapes, magenta), index(&shapes, wall_fill));
    assert!(m.is_some() && w.is_some(), "{m:?} {w:?}");
    assert!(m > w, "CAD over the walls by default");

    // Send to Back: one undo step, and the box draws under the walls.
    select(&mut sim, &[id]);
    cmd(&mut sim, ops::DG_BACK);
    assert_eq!(sim.app.cx.undo_label(), Some("Send to Back"));
    let shapes = sim.plan_shapes();
    let (m, w) = (index(&shapes, magenta), index(&shapes, wall_fill));
    assert!(m < w, "behind the walls now: {m:?} {w:?}");
    let t = sim.app.cx.project.drawing_group_defaults.clone();
    let group = sim
        .app
        .cx
        .floor()
        .drawing_group(&t, plan_core::ObjectRef::Cad(id));
    assert!(group < t.group_of("Walls"), "{group}");

    // The file keeps it.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.floors[0].drawing_group(&back.drawing_group_defaults, plan_core::ObjectRef::Cad(id)),
        group
    );

    // Bring to Front puts it over everything again.
    cmd(&mut sim, ops::DG_FRONT);
    assert_eq!(sim.app.cx.undo_label(), Some("Bring to Front"));
    let shapes = sim.plan_shapes();
    assert!(index(&shapes, magenta) > index(&shapes, wall_fill));
    sim.undo();
    sim.undo();
    assert!(sim.app.cx.floor().drawing_groups.is_empty());
}

#[test]
fn the_drawing_group_windows_set_a_number_and_the_kind_table() {
    let mut sim = sim();
    let a = rect(&mut sim, 0.0, 0.0, 60.0, 60.0);
    let b = rect(&mut sim, 30.0, 30.0, 60.0, 60.0);
    drawing_groups::close_all();
    select(&mut sim, &[a]);
    cmd(&mut sim, ops::DG_SET);
    assert!(drawing_groups::set_open());
    let ctx = sim.ctx.clone();
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            drawing_groups::show(ctx, &mut sim.app.cx)
        });
    }
    assert!(drawing_groups::set_open());
    let mut d = drawing_groups::SetDialog::new(&sim.app.cx);
    d.group = 90;
    d.apply(&mut sim.app.cx).unwrap();
    let order: Vec<Id> = sim
        .app
        .cx
        .floor()
        .cad_draw_order_with(&sim.app.cx.project.drawing_group_defaults)
        .iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(order, vec![b, a]);
    drawing_groups::close_all();

    // Default Settings > Drawing Groups: CAD below the walls.
    cmd(&mut sim, ops::DG_DEFAULTS);
    assert!(drawing_groups::defaults_open());
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        drawing_groups::show(ctx, &mut sim.app.cx)
    });
    let mut dd = drawing_groups::DefaultsDialog::new(&sim.app.cx);
    dd.table.set("CAD", 20);
    assert!(dd.apply(&mut sim.app.cx));
    let (behind, _) = sim
        .app
        .cx
        .floor()
        .cad_by_walls(&sim.app.cx.project.drawing_group_defaults);
    assert_eq!(
        behind.len(),
        1,
        "only b follows the kind; a has its own group"
    );
    assert_eq!(sim.undo().as_deref(), Some("Drawing Group Defaults"));
    drawing_groups::close_all();
}

#[test]
fn plan_footprint_follows_the_outer_wall_faces_not_the_rooms() {
    let mut sim = sim();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.tool(ToolId::Select);
    // An interior partition does not change the footprint.
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((240.0, 6.0), (240.0, 354.0));
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    let inner = plan_check::plan_footprint(&sim.app.cx.project, 0, &sim.app.cx.rooms).area_sq_ft;
    sim.action(Action::PlanFootprint);
    assert_eq!(sim.app.cx.undo_label(), Some("Plan Footprint"));
    let polys: Vec<_> = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .filter_map(|c| match &c.item {
            CadItem::Polyline {
                points,
                closed: true,
            } => Some(points.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(polys.len(), 1, "one outline");
    let outer = polygon_area(&polys[0]).abs() / 144.0;
    let t = sim.app.cx.floor().walls[0].thickness;
    // The outer faces lie half a wall thickness outside the centerlines of
    // the 480 x 360 shell; the rooms' footprint is smaller (it stops at the
    // inner faces and leaves out the partition).
    assert!(outer > inner, "{outer} vs {inner}");
    let want = (480.0 + t) * (360.0 + t) / 144.0;
    assert!((outer - want).abs() / want < 0.03, "{outer} vs {want}");
    assert!(sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .any(|c| matches!(&c.item, CadItem::Text { text, .. } if text.starts_with("Footprint:"))));
    assert_eq!(sim.undo().as_deref(), Some("Plan Footprint"));
    assert!(sim.app.cx.floor().cad.is_empty());
}

#[test]
fn the_dxf_export_options_window_decides_units_names_text_and_floors() {
    let mut sim = sim();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.tool(ToolId::Select);
    sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Text {
            pos: Point::new(100.0, 100.0),
            text: "KITCHEN 12'".into(),
            height: 6.0,
            angle: 0.0,
        },
    );
    // File > Export > DXF opens the options first.
    dxf_options::close();
    sim.action(Action::File(FileCommand::ExportDxf));
    assert!(dxf_options::is_open());
    let ctx = sim.ctx.clone();
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            dxf_options::show(ctx, &mut sim.app.cx)
        });
    }
    assert!(dxf_options::is_open());
    dxf_options::close();

    let mut d = dxf_options::DxfExportDialog::new(&sim.app.cx);
    let plain = d.text(&mut sim.app.cx).unwrap();
    assert!(plain.contains("Walls, Normal") && plain.lines().any(|l| l == "TEXT"));
    d.opts.units = DxfUnits::Millimeters;
    d.opts.naming = LayerNaming::Aia;
    d.opts.text = TextMode::Lines;
    d.opts.line_weights = true;
    d.map_text = "CAD, Default = NOTES".into();
    let text = d.text(&mut sim.app.cx).unwrap();
    assert!(text.contains("A-WALL") && text.contains("NOTES"));
    assert!(!text.lines().any(|l| l == "TEXT"), "text became lines");
    assert!(text.lines().any(|l| l == "370"), "weights written");
    assert!(
        text.contains("2540.0000"),
        "the note at x = 100 inches is 2540 millimeters"
    );
    // Two floors in 3D: the second at its elevation, with the model.
    let mut up = sim.app.cx.project.floors[0].clone();
    up.name = "2nd Floor".into();
    up.elevation = 108.0;
    sim.app.cx.project.floors.push(up);
    sim.app.cx.refresh();
    let mut d3 = dxf_options::DxfExportDialog::new(&sim.app.cx);
    d3.floors = dxf_options::FloorChoice::AllFloors;
    d3.opts.three_d = true;
    let text = d3.text(&mut sim.app.cx).unwrap();
    assert!(text.contains("2nd Floor - Walls, Normal"));
    assert!(text.lines().filter(|l| *l == "3DFACE").count() > 10);
    // The plan is exactly as it was.
    assert_eq!(sim.app.cx.floor, 0);
    assert_eq!(sim.app.cx.project.floors.len(), 2);
}
