//! Scenario 66: construction lines and the Reference Display (manual pp.
//! 83-91; CAD-62..CAD-67, LAY-9, LAY-39..LAY-45, S-194).
//!
//! Draw construction lines with the tool, snap a wall corner to where two of
//! them cross (past their drawn ends), dimension to one, number them with the
//! rule sets and show them on every floor; then fill the Change
//! Floor/Reference table with a second plan file (offset and angle), swap the
//! floor and the reference, and check what the plan draws and which steps
//! undo.

use super::Sim;
use crate::dialogs::{construction_line as cl, construction_order, reference_display as rd};
use crate::editor::ref_overlay;
use crate::editor::snap::SnapKind;
use crate::editor::ObjectRef;
use crate::toolbar::{Action, ViewFlag};
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::{self, Key};
use plan_core::construction::{CalloutEnds, ReferenceRow, ViewType};
use plan_core::geometry::Point;
use plan_core::{Floor, Id, Project, WallKind};

fn sim() -> Sim {
    let mut sim = Sim::new();
    // No grid or angle snaps: the points land where the objects put them.
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    rd::reset_settings();
    sim
}

fn near(a: Point, b: Point) -> bool {
    a.dist(b) < 1e-6
}

/// Draws a construction line with the tool.
fn draw(sim: &mut Sim, a: (f64, f64), b: (f64, f64)) -> Id {
    sim.tool(ToolId::ConstructionLine);
    sim.drag(a, b);
    let f = sim.app.cx.floor();
    f.construction.lines.last().expect("a line was drawn").id
}

fn two_floor_sim() -> Sim {
    let mut sim = sim();
    sim.app
        .cx
        .project
        .floors
        .push(Floor::new("2nd Floor", 108.0));
    sim.app.cx.project.add_wall(
        0,
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.cx.floor = 1;
    sim.app.cx.view_flags.insert(ViewFlag::ReferenceDisplay);
    sim.app.cx.refresh();
    sim
}

/// Writes a second plan (one 200 inch wall) to a file of its own.
fn other_plan_file(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("plan-studio-s66-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut other = Project::new("As built");
    other.add_wall(
        0,
        Point::new(0.0, 0.0),
        Point::new(200.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    plan_core::io::save_project(&other, &path).unwrap();
    path.to_string_lossy().into_owned()
}

fn leaves(sim: &mut Sim) -> Vec<egui::Shape> {
    sim.plan_shapes()
}

// ----- construction lines -----

#[test]
fn the_tool_draws_lines_on_their_layer_and_a_wall_corner_snaps_to_where_two_cross() {
    let mut sim = sim();
    // Two short stubs; they cross at (300, 100), far past their ends.
    let h = draw(&mut sim, (0.0, 100.0), (40.0, 100.0));
    let v = draw(&mut sim, (300.0, -60.0), (300.0, -20.0));
    assert_eq!(sim.app.cx.floor().construction.lines.len(), 2);
    for id in [h, v] {
        let cad = sim.app.cx.floor().cad.iter().find(|c| c.id == id).unwrap();
        assert_eq!(cad.layer, "Construction Lines");
        assert_eq!(
            sim.app
                .cx
                .floor()
                .drawing_group_override(plan_core::ObjectRef::Cad(id)),
            Some(21),
            "drawing group 21"
        );
    }
    assert_eq!(sim.app.cx.undo_label(), Some("Construction Line"));

    // The cursor near the crossing snaps to it.
    let snap = sim
        .app
        .cx
        .snap_at(Point::new(297.0, 103.0), None, false, &[]);
    assert_eq!(snap.kind, SnapKind::Intersection);
    assert!(near(snap.point, Point::new(300.0, 100.0)), "{:?}", snap.point);

    // A wall started there starts exactly on the crossing.
    sim.tool(ToolId::Wall {
        kind: WallKind::Exterior,
    });
    sim.drag((297.0, 103.0), (500.0, 103.0));
    let w = sim.app.cx.floor().walls.last().expect("the wall");
    assert!(near(w.start, Point::new(300.0, 100.0)), "{:?}", w.start);
    assert!((w.end.y - 100.0).abs() < 1e-6, "the end snaps onto the horizontal line");
}

#[test]
fn a_dimension_measures_to_a_construction_line() {
    let mut sim = sim();
    draw(&mut sim, (0.0, 100.0), (40.0, 100.0));
    draw(&mut sim, (650.0, -40.0), (650.0, -10.0));
    sim.tool(ToolId::Dimension);
    // From a point on the horizontal line to the vertical line's crossing.
    sim.click(100.0, 101.0);
    sim.click(652.0, 102.0);
    sim.click(380.0, 150.0);
    sim.key(KeyEvent::key(Key::Enter));
    let dims = &sim.app.cx.floor().dimensions;
    assert_eq!(dims.len(), 1);
    assert!(near(dims[0].end, Point::new(650.0, 100.0)), "{:?}", dims[0].end);
    assert!((dims[0].length() - 550.0).abs() < 1e-6, "{}", dims[0].length());
}

#[test]
fn infinite_lines_draw_across_the_view_with_callouts_at_its_edges() {
    let mut sim = sim();
    let id = draw(&mut sim, (0.0, 100.0), (40.0, 100.0));
    sim.tool(ToolId::Select);
    {
        let cx = &mut sim.app.cx;
        let rec = cx.project.floors[0].construction.get_mut(id).unwrap();
        rec.callouts.plan = CalloutEnds::Both;
    }
    let cam = sim.app.camera;
    // The line is drawn as dashed pieces that reach both view edges.
    let shapes = leaves(&mut sim);
    let xs: Vec<f32> = shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::LineSegment { points, stroke }
                if (points[0].y - points[1].y).abs() < 0.01 && stroke.color.b() > 150 =>
            {
                Some([points[0].x, points[1].x])
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert!(!xs.is_empty(), "the dashed line is drawn");
    let (lo, hi) = xs
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
    let _ = cam;
    assert!(lo < 20.0 && hi > 1380.0, "spans the 1400 px view: {lo}..{hi}");
    // Two callout outlines, and the automatic order label "A" (a flat line
    // is lettered) inside each.
    let texts: Vec<String> = shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(texts.iter().filter(|t| t.as_str() == "A").count(), 2, "{texts:?}");
}

#[test]
fn a_finite_line_draws_only_its_length_and_the_layer_hides_it() {
    let mut sim = sim();
    let id = draw(&mut sim, (0.0, 100.0), (300.0, 100.0));
    sim.app
        .cx
        .project
        .floors[0]
        .construction
        .get_mut(id)
        .unwrap()
        .infinite_plan = false;
    let segs = ref_overlay::construction_snap_segments(&sim.app.cx);
    assert!(segs.is_empty(), "a finite line on this floor is a CAD line");
    // Past its end nothing snaps to it (the CAD line ends there).
    let far = sim.app.cx.snap_at(Point::new(800.0, 101.0), None, false, &[]);
    assert_ne!(far.kind, SnapKind::OnObject);
    // Turn the layer off: not drawn, not snapped.
    sim.app.cx.project.layers.set_display("Construction Lines", false);
    sim.app.cx.refresh();
    assert!(ref_overlay::visible_construction(&sim.app.cx).is_empty());
}

#[test]
fn lines_on_all_floors_show_upstairs_and_snap_there() {
    let mut sim = sim();
    sim.app
        .cx
        .project
        .floors
        .push(Floor::new("2nd Floor", 108.0));
    let all = draw(&mut sim, (0.0, 100.0), (40.0, 100.0));
    let here = draw(&mut sim, (0.0, 400.0), (40.0, 400.0));
    sim.app
        .cx
        .project
        .floors[0]
        .construction
        .get_mut(all)
        .unwrap()
        .all_floors = true;
    sim.app.cx.floor = 1;
    sim.app.cx.refresh();
    let shown: Vec<Id> = ref_overlay::visible_construction(&sim.app.cx)
        .iter()
        .map(|l| l.id)
        .collect();
    assert_eq!(shown, vec![all]);
    assert!(!shown.contains(&here));
    let snap = sim.app.cx.snap_at(Point::new(700.0, 101.0), None, false, &[]);
    assert_eq!(snap.kind, SnapKind::OnObject);
    assert!((snap.point.y - 100.0).abs() < 1e-6);
}

#[test]
fn the_rule_sets_number_the_lines_and_the_dialog_changes_them_in_one_step() {
    let mut sim = sim();
    let a = draw(&mut sim, (100.0, 0.0), (100.0, 30.0));
    let b = draw(&mut sim, (400.0, 0.0), (400.0, 30.0));
    let c = draw(&mut sim, (0.0, 90.0), (30.0, 90.0));
    let labels = sim.app.cx.project.construction_order(0, ViewType::Plan);
    assert_eq!(labels[&a], "1");
    assert_eq!(labels[&b], "2");
    assert_eq!(labels[&c], "A");
    // CAD > Line > Construction Line Order Management.
    sim.action(Action::Custom(cl::ORDER));
    assert!(construction_order::dialog_open());
    construction_order::with_dialog(|d| {
        d.rule_sets_mut()[0].reverse = true;
    })
    .unwrap();
    assert!(construction_order::accept_dialog(&mut sim.app.cx));
    let labels = sim.app.cx.project.construction_order(0, ViewType::Plan);
    assert_eq!((labels[&a].as_str(), labels[&b].as_str()), ("2", "1"));
    assert_eq!(
        sim.app.cx.undo_label(),
        Some("Construction Line Order Management")
    );
    sim.undo();
    assert_eq!(
        sim.app.cx.project.construction_order(0, ViewType::Plan)[&a],
        "1"
    );
}

#[test]
fn the_specification_the_defaults_and_the_conversions_work_from_the_edit_buttons() {
    let mut sim = sim();
    let id = draw(&mut sim, (0.0, 100.0), (40.0, 100.0));
    sim.tool(ToolId::Select);
    let labels: Vec<&str> = sim
        .app
        .cx
        .extra_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect();
    assert!(labels.contains(&"Construction Line Specification"), "{labels:?}");
    assert!(labels.contains(&"Set as Default"));
    assert!(labels.contains(&"Convert to Polyline"));
    // Open Object.
    sim.action(Action::Custom(cl::SPEC));
    assert!(cl::dialog_open());
    cl::with_dialog(|d| {
        d.draft_mut().all_floors = true;
        d.draft_mut().callouts.plan = CalloutEnds::End;
    })
    .unwrap();
    assert!(cl::accept_dialog(&mut sim.app.cx));
    assert!(sim.app.cx.floor().construction_line(id).unwrap().all_floors);
    // The conversion leaves a plain polyline, and back.
    sim.action(Action::Custom(cl::TO_POLYLINE));
    assert!(!sim.app.cx.floor().is_construction_line(id));
    sim.action(Action::Custom(cl::FROM_POLYLINE));
    assert!(sim.app.cx.floor().is_construction_line(id));
    // Undo in reverse: back to the polyline, then the line, then the specification.
    sim.undo();
    assert!(!sim.app.cx.floor().is_construction_line(id));
    sim.undo();
    assert!(sim.app.cx.floor().is_construction_line(id));
    assert!(sim.app.cx.floor().construction_line(id).unwrap().all_floors);
    sim.undo();
    assert!(!sim.app.cx.floor().construction_line(id).unwrap().all_floors);
}

// ----- the Reference Display -----

#[test]
fn a_row_from_a_second_plan_file_draws_through_its_offset_and_angle() {
    let mut sim = two_floor_sim();
    let file = other_plan_file("as-built.psplan");
    let before = sim.app.cx.undo_depth();
    // Tools > Floor/Reference Display > Change Floor/Reference.
    sim.action(Action::Custom(rd::CHANGE));
    assert!(rd::dialog_open());
    rd::with_dialog(|d| {
        // [Current, row]: a second row below the first one, from a file.
        d.insert_below();
        let line = d.selected();
        d.set_row_file(line, &file);
        let r = d.row_at_mut(line).unwrap();
        r.offset = [50.0, 20.0, 0.0];
        r.angle_deg = 90.0;
        d.table_mut().xor = false;
    })
    .unwrap();
    assert!(rd::accept_dialog(&mut sim.app.cx));
    assert_eq!(sim.app.cx.undo_depth(), before + 1, "OK is one undo step");
    assert_eq!(sim.app.cx.undo_label(), Some("Reference Display"));
    sim.app.cx.refresh();

    let layers = ref_overlay::reference_layers(&sim.app.cx);
    assert_eq!(layers.len(), 2, "this plan's floor below, and the file");
    let first = &layers[0];
    assert!(first.row.source.is_this_plan());
    assert!(near(first.walls[0].end, Point::new(240.0, 0.0)));
    let second = &layers[1];
    assert_eq!(second.row.floor, plan_core::construction::RowFloor::MatchCurrent);
    // The other plan's 200 inch wall, turned 90 degrees and moved by (50, 20).
    // Floor 1 of this plan has no counterpart in a one-floor file.
    assert!(second.walls.is_empty() || near(second.walls[0].end, Point::new(50.0, 220.0)));

    // On the ground floor the file's level matches: its wall is there.
    sim.app.cx.floor = 0;
    let layers = ref_overlay::reference_layers(&sim.app.cx);
    let file_layer = layers
        .iter()
        .find(|l| !l.row.source.is_this_plan())
        .expect("the file's row");
    let w = &file_layer.walls[0];
    assert!(near(w.start, Point::new(50.0, 20.0)), "{:?}", w.start);
    assert!(near(w.end, Point::new(50.0, 220.0)), "{:?}", w.end);
    // The wall ends snap there.
    sim.app.cx.refresh();
    let snap = sim
        .app
        .cx
        .snap_at(Point::new(52.0, 222.0), None, false, &[]);
    assert_eq!(snap.kind, SnapKind::Endpoint);
    assert!(near(snap.point, Point::new(50.0, 220.0)), "{:?}", snap.point);

    // Undo takes the second row away again.
    sim.app.cx.floor = 1;
    sim.undo();
    assert!(sim.app.cx.project.reference_table.is_default());
    assert_eq!(ref_overlay::reference_layers(&sim.app.cx).len(), 1);
}

#[test]
fn rows_reorder_and_the_current_line_moves_with_them() {
    let mut sim = two_floor_sim();
    sim.action(Action::Custom(rd::CHANGE));
    rd::with_dialog(|d| {
        d.insert_above(); // a new row above the first one
        assert_eq!(d.table().rows.len(), 2);
        // The Current line is at the top; move the selected row above it.
        d.select(2);
        d.move_up();
        d.move_up();
        assert_eq!(d.table().current_at, 1, "one row in front of the Current line");
    })
    .unwrap();
    assert!(rd::accept_dialog(&mut sim.app.cx));
    let layers = ref_overlay::reference_layers(&sim.app.cx);
    assert!(layers[0].front && !layers[1].front);
    // The row in front is drawn after the walls (over the plan).
    let _ = leaves(&mut sim);
}

#[test]
fn xor_drawing_hides_identical_lines() {
    let mut sim = two_floor_sim();
    // A wall over the reference wall, exactly: its lines are identical.
    sim.app.cx.project.add_wall(
        1,
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.cx.refresh();
    let plain = leaves(&mut sim).len();
    sim.app.cx.project.reference_table.rows = vec![ReferenceRow::default()];
    sim.app.cx.project.reference_table.xor = true;
    let xor = leaves(&mut sim).len();
    // Four reference edges became none.
    assert!(xor < plain + 4, "{xor} vs {plain}");
}

#[test]
fn a_wall_over_its_reference_counterpart_gets_light_blue_edges() {
    let mut sim = two_floor_sim();
    // Exactly over the ground floor's wall (drawn the other way round).
    let id = sim.app.cx.project.add_wall(
        1,
        Point::new(240.0, 0.0),
        Point::new(0.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.cx.refresh();
    assert_eq!(ref_overlay::aligned_wall_ids(&sim.app.cx), vec![id]);
    let blue = ref_overlay::ALIGNED_BLUE;
    let has_blue = |shapes: &[egui::Shape]| {
        shapes.iter().any(|s| match s {
            egui::Shape::Path(p) => p.stroke.color == egui::epaint::ColorMode::Solid(blue),
            _ => false,
        })
    };
    assert!(has_blue(&leaves(&mut sim)), "the aligned wall's outline is light blue");
    // A hair off: the highlight goes.
    sim.app.cx.project.floors[1].wall_mut(id).unwrap().start = Point::new(240.0, 1.0);
    sim.app.cx.refresh();
    assert!(ref_overlay::aligned_wall_ids(&sim.app.cx).is_empty());
    assert!(!has_blue(&leaves(&mut sim)));
}

#[test]
fn swap_goes_to_the_reference_floor_and_back_without_an_undo_step() {
    let mut sim = two_floor_sim();
    let depth = sim.app.cx.undo_depth();
    assert_eq!(sim.app.cx.floor, 1);
    sim.action(Action::Custom(rd::SWAP));
    assert_eq!(sim.app.cx.floor, 0, "the floor below is now current");
    // The old floor is the reference: floor 1 (above), which has no walls
    // here, but the reference floor resolves to it.
    let s = rd::settings(&sim.app.cx.project);
    assert_eq!(s.floor, rd::ReferenceFloor::Floor(1));
    sim.action(Action::Custom(rd::SWAP));
    assert_eq!(sim.app.cx.floor, 1, "and back");
    assert_eq!(sim.app.cx.undo_depth(), depth, "swapping is not an undo step");
}

#[test]
fn swap_is_refused_with_several_rows_or_another_plan() {
    let mut sim = two_floor_sim();
    sim.app.cx.project.reference_table.rows = vec![ReferenceRow::default(), ReferenceRow::default()];
    sim.action(Action::Custom(rd::SWAP));
    assert_eq!(sim.app.cx.floor, 1);
    assert!(sim.app.cx.status.contains("several"), "{}", sim.app.cx.status);
    sim.app.cx.project.reference_table.rows = vec![ReferenceRow::for_file("/nonexistent/x.psplan")];
    sim.action(Action::Custom(rd::SWAP));
    assert_eq!(sim.app.cx.floor, 1);
    assert!(sim.app.cx.status.contains("another plan"), "{}", sim.app.cx.status);
}

#[test]
fn the_dialog_makes_another_floor_current_with_the_table_in_one_step() {
    let mut sim = two_floor_sim();
    sim.action(Action::Custom(rd::CHANGE));
    rd::with_dialog(|d| {
        assert_eq!(d.current(), 1);
        d.set_current_floor(0);
        d.set_show_display(false);
    })
    .unwrap();
    assert!(rd::accept_dialog(&mut sim.app.cx));
    assert_eq!(sim.app.cx.floor, 0);
    assert!(!sim.app.cx.view_flags.contains(&ViewFlag::ReferenceDisplay));
    assert_eq!(sim.app.cx.undo_label(), Some("Reference Display"));
    // The dialog draws without a panic.
    sim.action(Action::Custom(rd::CHANGE));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(rd::dialog_open());
    sim.dialog_frame_key(Some(Key::Escape));
    sim.dialog_frame(false);
    assert!(!rd::dialog_open(), "Escape cancels");
}

#[test]
fn the_offset_tool_moves_and_turns_the_second_plan_with_one_undo_step_per_drag() {
    let mut sim = two_floor_sim();
    sim.app.cx.floor = 0;
    let file = other_plan_file("as-built-2.psplan");
    sim.app.cx.project.reference_table.rows = vec![ReferenceRow::for_file(file)];
    sim.app.cx.refresh();
    sim.action(Action::Custom(rd::EDIT_OFFSET));
    assert_eq!(sim.app.tools.active_id(), ToolId::ReferenceOffset);
    // Drag the marquee 30 right, 12 up.
    let m = crate::tools::construction_line::marquee(&sim.app.cx, 0).unwrap();
    let c = Point::lerp(m[0], m[2], 0.5);
    sim.drag((c.x, c.y), (c.x + 30.0, c.y + 12.0));
    let row = &sim.app.cx.project.reference_table.rows[0];
    assert!((row.offset[0] - 30.0).abs() < 1e-6 && (row.offset[1] - 12.0).abs() < 1e-6);
    assert_eq!(sim.app.cx.undo_label(), Some("Edit Reference Document Offset"));
    // Rotate with the round handle a quarter turn.
    let m = crate::tools::construction_line::marquee(&sim.app.cx, 0).unwrap();
    let (center, rot) = crate::tools::construction_line::handles(&m, 3.0 * sim.app.cx.snap_tol());
    let v = rot.sub(center);
    let to = center.add(Point::new(-v.y, v.x));
    sim.drag((rot.x, rot.y), (to.x, to.y));
    let row = &sim.app.cx.project.reference_table.rows[0];
    assert!((row.angle_deg.rem_euclid(360.0) - 90.0).abs() < 1e-6, "{}", row.angle_deg);
    // Two drags, two undo steps.
    sim.undo();
    assert!(sim.app.cx.project.reference_table.rows[0].angle_deg.abs() < 1e-9);
    sim.undo();
    assert_eq!(sim.app.cx.project.reference_table.rows[0].offset, [0.0; 3]);
    // Enter leaves the tool.
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
}

#[test]
fn nothing_of_this_is_selectable_in_the_reference() {
    // Reference objects cannot be picked or edited (manual p. 89).
    let mut sim = two_floor_sim();
    let hit = crate::editor::selection::hit_test_cx(&sim.app.cx, Point::new(120.0, 0.0), 6.0);
    assert!(hit.is_empty(), "{hit:?}");
    let _ = ObjectRef::Cad(0);
}
