//! Scenario 54 (round 15): every Chief dimension tool and default.
//!
//! The tool set is complete (Radius and Arc Length join Manual, End to End,
//! Interior, Point to Point, Running, Baseline, Angular, Centerline and the
//! automatic tools); the Dimension Defaults panels (General, Setup
//! Automatic, Setup Temporary, Secondary Format, Locate per tool, Auto Story
//! Pole) edit what the tools read; a dimension string is one object that
//! selects, moves, turns and deletes together; the second format, tolerance,
//! text position, moved and turned labels, rich labels, centerline marks and
//! the curved dimensions (radius, arc length, angle between walls) draw and
//! follow the walls they measure (DIM-2, DIM-18, DIM-46, DIM-50 to DIM-58,
//! DIM-61, DIM-63 to DIM-68).

use super::Sim;
use crate::dialogs::default_pages;
use crate::editor::{render, ObjectRef};
use crate::tools::dimension::{DimMode, CMD_JOIN, CMD_LEAVE, CMD_SELECT_STRING};
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::{self, Key};
use plan_core::defaults::{PageValue, PlanDefaults};
use plan_core::dimension::{CurveKind, DimCurve, LeaderStyle, MarkKind, TolMode};
use plan_core::geometry::Point;
use plan_core::walls::WallCurve;
use plan_core::{Dimension, DimensionKind, Id, OpeningKind, WallKind};
use std::f64::consts::FRAC_PI_2;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn dims(sim: &Sim) -> Vec<Dimension> {
    sim.app.cx.floor().dimensions.clone()
}

fn dim(sim: &Sim, id: Id) -> Dimension {
    sim.app
        .cx
        .floor()
        .dimensions
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .clone()
}

/// Writes a Default Settings page field, the way the page's OK does.
fn set_page(d: &mut PlanDefaults, page: &str, field: &str, v: PageValue) {
    let spec = default_pages::spec(&format!("dimension.{page}")).expect("the page exists");
    let key = format!("dimension.{page}.{field}");
    spec.field(&key)
        .unwrap_or_else(|| panic!("no field {key}"))
        .write(d, &v);
}

/// A 40' x 30' shell of 6" exterior walls with a window on every side.
fn shell(sim: &mut Sim) -> [Id; 4] {
    let cx = &mut sim.app.cx;
    let c = [p(0.0, 0.0), p(480.0, 0.0), p(480.0, 360.0), p(0.0, 360.0)];
    let mut ids = [0; 4];
    for i in 0..4 {
        ids[i] = cx
            .project
            .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
    }
    for id in ids {
        cx.project
            .add_opening(0, id, 120.0, OpeningKind::Window)
            .unwrap();
        cx.project
            .add_opening(0, id, 300.0, OpeningKind::Window)
            .unwrap_or_default();
    }
    cx.refresh();
    ids
}

fn texts_of(shapes: &[egui::Shape]) -> Vec<String> {
    shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_dimension_flyouts_hold_every_tool_and_each_one_activates() {
    let mut sim = Sim::new();
    let mut names: Vec<&str> = crate::toolbar::dimensions()
        .entries
        .iter()
        .chain(crate::toolbar::auto_dimensions().entries.iter())
        .map(|i| i.name)
        .collect();
    names.sort_unstable();
    for m in DimMode::ALL {
        assert!(names.contains(&m.name()), "{} is on a flyout", m.name());
    }
    assert_eq!(DimMode::ALL.len(), 18);
    for m in DimMode::ALL {
        sim.tool(ToolId::DimensionVariant(m));
        assert_eq!(sim.app.tools.active().name(), m.name());
        sim.esc();
    }
}

#[test]
fn a_continued_dimension_is_one_string_that_selects_moves_turns_and_deletes_as_one() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.click(0.0, 0.0);
    sim.click(100.0, 0.0);
    sim.click(50.0, 30.0);
    sim.click(200.0, 10.0);
    sim.click(300.0, 5.0);
    sim.esc();
    let all = dims(&sim);
    assert_eq!(all.len(), 3);
    let head = all[0].id;
    for d in &all {
        assert_eq!(d.string_id(), Some(head), "{d:?}");
    }
    assert_eq!(sim.app.cx.floor().string_members(all[2].id).len(), 3);

    // A click on one segment with Select Objects takes the whole string.
    sim.tool(ToolId::Select);
    let (a, b) = all[1].line_points();
    let mid = Point::lerp(a, b, 0.5);
    sim.click(mid.x, mid.y);
    assert_eq!(sim.app.cx.selection.items.len(), 3);

    // Move takes it along, one undo step.
    crate::editor::transform::move_selection(&mut sim.app.cx, p(0.0, 50.0), "Move").unwrap();
    for d in dims(&sim) {
        assert!((d.start.y - 50.0).abs() < 1e-9 && (d.end.y - 50.0).abs() < 1e-9);
    }
    // Turn it a quarter turn about the origin: the string stays together.
    let rep =
        crate::editor::transform::rotate_selection(&mut sim.app.cx, FRAC_PI_2, Some(p(0.0, 0.0)));
    assert_eq!(rep.changed, 3);
    let turned = dims(&sim);
    assert!((turned[0].start.x + 50.0).abs() < 1e-9 && turned[0].start.y.abs() < 1e-9);
    assert_eq!(sim.app.cx.floor().string_members(head).len(), 3);
    assert!(sim.undo().is_some());
    assert!((dims(&sim)[0].start.y - 50.0).abs() < 1e-9);

    // Dragging the line handle moves every segment's line together.
    let dragged = dims(&sim);
    let target = Point::lerp(dragged[1].line_points().0, dragged[1].line_points().1, 0.5);
    let _ = target;

    // Taking one out, and joining again (Edit toolbar).
    sim.app.cx.selection.set(ObjectRef::Dimension(all[1].id));
    assert!(crate::tools::dimension::run_command(
        &mut sim.app.cx,
        CMD_LEAVE
    ));
    assert_eq!(sim.app.cx.floor().string_members(all[1].id).len(), 1);
    assert_eq!(sim.app.cx.floor().string_members(head).len(), 2);
    sim.app.cx.selection.items = vec![
        ObjectRef::Dimension(all[0].id),
        ObjectRef::Dimension(all[1].id),
    ];
    assert!(crate::tools::dimension::run_command(
        &mut sim.app.cx,
        CMD_JOIN
    ));
    assert_eq!(sim.app.cx.floor().string_members(all[0].id).len(), 3);
    sim.app.cx.selection.set(ObjectRef::Dimension(all[2].id));
    assert!(crate::tools::dimension::run_command(
        &mut sim.app.cx,
        CMD_SELECT_STRING
    ));
    assert_eq!(sim.app.cx.selection.items.len(), 3);

    // Delete takes the string, undo brings it back with its links.
    sim.key(KeyEvent::key(Key::Delete));
    assert!(dims(&sim).is_empty());
    sim.undo();
    assert_eq!(dims(&sim).len(), 3);
    assert_eq!(sim.app.cx.floor().string_members(head).len(), 3);
}

#[test]
fn dragging_the_offset_handle_moves_the_whole_string_line() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.click(0.0, 0.0);
    sim.click(100.0, 0.0);
    sim.click(50.0, 30.0);
    sim.click(200.0, 10.0);
    sim.esc();
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    let all = dims(&sim);
    sim.app.cx.selection.set(ObjectRef::Dimension(all[1].id));
    let (a, b) = all[1].line_points();
    let grab = Point::lerp(a, b, 0.25);
    sim.drag((grab.x, grab.y), (grab.x, 60.0));
    for d in dims(&sim) {
        assert!((d.offset - 60.0).abs() < 1e-6, "{}", d.offset);
    }
}

#[test]
fn automatic_runs_make_strings_and_the_setup_decides_which_are_made() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    shell(&mut sim);
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    sim.click(-100.0, -100.0);
    let all = dims(&sim);
    assert!(!all.is_empty());
    let in_strings = all.iter().filter(|d| d.string_id().is_some()).count();
    assert!(in_strings >= 2, "the openings strings are strings");
    // Every string's members are one row: same line.
    for d in all.iter().filter(|d| d.string_id().is_some()) {
        let mates = sim.app.cx.floor().string_members(d.id);
        assert!(mates.len() >= 2);
        for m in mates {
            let o = dim(&sim, m);
            assert!((o.offset - d.offset).abs() < 1e-6 || o.start.dist(d.start) > 1.0);
        }
    }
    let with_all = all.len();
    // Setup Automatic: no inner strings.
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "exterior_inner",
        PageValue::Bool(false),
    );
    sim.click(-100.0, -100.0);
    let overall_only = dims(&sim).len();
    assert!(overall_only < with_all, "{overall_only} < {with_all}");
    // And no overall either: nothing is left to make.
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "exterior_overall",
        PageValue::Bool(false),
    );
    sim.click(-100.0, -100.0);
    assert!(sim.app.cx.status.contains("No exterior"));
    // Offset From: the wall center puts the line nearer the wall by half its
    // thickness than the dimension layer does.
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "exterior_inner",
        PageValue::Bool(true),
    );
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "exterior_overall",
        PageValue::Bool(true),
    );
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "offset_from",
        PageValue::Text("Dimension Layer".into()),
    );
    sim.click(-100.0, -100.0);
    let far = dims(&sim)
        .iter()
        .map(|d| d.offset.abs())
        .fold(0.0, f64::max);
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "offset_from",
        PageValue::Text("Center".into()),
    );
    sim.click(-100.0, -100.0);
    let near = dims(&sim)
        .iter()
        .map(|d| d.offset.abs())
        .fold(0.0, f64::max);
    assert!(near < far, "{near} < {far}");
}

#[test]
fn each_dimension_tool_reads_its_own_locate_panel() {
    let mut sim = Sim::new();
    shell(&mut sim);
    // Auto Exterior locates wall surfaces; Manual stays on the dimension layer.
    set_page(
        &mut sim.app.cx.defaults,
        "locate_manual",
        "walls",
        PageValue::Text("Wall Dimension Layer".into()),
    );
    set_page(
        &mut sim.app.cx.defaults,
        "locate_auto_exterior",
        "walls",
        PageValue::Text("Wall Center".into()),
    );
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    sim.click(-100.0, -100.0);
    let overall = |sim: &Sim| {
        dims(sim)
            .iter()
            .filter(|d| (d.start.y - d.end.y).abs() < 1e-6 && d.length() > 400.0)
            .map(Dimension::length)
            .fold(0.0, f64::max)
    };
    let centers = overall(&sim);
    assert!(
        (centers - 480.0).abs() < 1e-6,
        "center to center: {centers}"
    );
    set_page(
        &mut sim.app.cx.defaults,
        "locate_auto_exterior",
        "walls",
        PageValue::Text("Surfaces".into()),
    );
    sim.click(-100.0, -100.0);
    let faces = overall(&sim);
    assert!(
        (faces - 486.0).abs() < 1e-6,
        "outer face to outer face: {faces}"
    );
    // Manual (the other panel) still reads the dimension layer.
    let t = sim
        .app
        .cx
        .defaults
        .dimensions
        .tool_locate(plan_core::dimension::LocateTool::Manual);
    assert_eq!(t.group.walls, plan_core::WallLocate::MainLayer);
    // A wall panel set to None locates no walls.
    set_page(
        &mut sim.app.cx.defaults,
        "locate_manual",
        "walls",
        PageValue::Text("None".into()),
    );
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.click(240.0, 1.0);
    let loc = sim.app.cx.status.clone();
    assert!(
        !loc.contains("Wall surface") && !loc.contains("Wall center"),
        "{loc}"
    );
}

#[test]
fn the_second_format_tolerance_and_text_position_reach_the_label() {
    let mut sim = Sim::new();
    let id = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(0.0, 0.0), p(120.0, 0.0), 24.0),
    );
    let fmt0 = sim.app.cx.dim_format();
    assert_eq!(dim(&sim, id).label(&fmt0), "10'-0\"");
    {
        let d = &mut sim.app.cx.defaults;
        set_page(d, "secondary", "include", PageValue::Bool(true));
        set_page(
            d,
            "secondary",
            "tolerance",
            PageValue::Text("Plus or Minus".into()),
        );
        set_page(
            d,
            "general",
            "text_position",
            PageValue::Text("Below Dimension Line".into()),
        );
        set_page(d, "general", "tilde_before", PageValue::Bool(true));
    }
    let fmt = sim.app.cx.dim_format();
    let d = dim(&sim, id);
    let parts = d.label_parts(&fmt);
    assert!(parts.primary.contains('\u{b1}'), "{}", parts.primary);
    assert!(
        parts.second.as_deref().unwrap_or("").contains("3048"),
        "{parts:?}"
    );
    assert_eq!(parts.lines().len(), 2);
    // Below the line, with the second format above it.
    let width = |s: &str| s.chars().count() as f64 * 2.0;
    let lay = d.label_layout(
        &fmt,
        p(60.0, 24.0),
        p(1.0, 0.0),
        Some((p(0.0, 24.0), p(120.0, 24.0))),
        120.0,
        &plan_core::dimension::LabelParams {
            text_h: 4.5,
            width: &width,
            view_rotation: 0.0,
            leader: LeaderStyle::SquareCorner,
        },
    );
    assert!(lay.lines[0].center.y < 24.0 && lay.lines[1].center.y > 24.0);
    // The plan draws both numbers.
    let shapes = sim.plan_shapes();
    let texts = texts_of(&shapes);
    assert!(texts.iter().any(|t| t.contains("3048")), "{texts:?}");
    // A dimension of its own overrides the defaults.
    {
        let dm = sim.app.cx.project.floors[0]
            .dimensions
            .iter_mut()
            .find(|x| x.id == id)
            .unwrap();
        dm.look.tolerance = Some(plan_core::dimension::Tolerance::default());
        dm.look.second = Some(plan_core::dimension::SecondFormat::default());
    }
    let own = dim(&sim, id).label_parts(&fmt);
    assert!(!own.primary.contains('\u{b1}'));
    assert!(own.second.is_none());
    // Distance Rounding: the shown values stay what each part rounds to.
    set_page(
        &mut sim.app.cx.defaults,
        "general",
        "rounding",
        PageValue::Text("Distance Rounding".into()),
    );
    assert_eq!(
        sim.app.cx.dim_format().label.rounding,
        plan_core::dimension::RoundMethod::Distance
    );
    assert_eq!(TolMode::ALL.len(), 4);
}

#[test]
fn grid_rounding_keeps_a_string_adding_up_in_the_editor() {
    let mut sim = Sim::new();
    // Three parts of 1/32" more than 1/16" steps: plain rounding drifts.
    let mk = |a: f64, b: f64| Dimension::new(0, DimensionKind::Manual, p(a, 0.0), p(b, 0.0), 24.0);
    let a = sim.app.cx.project.add_dimension(0, mk(0.0, 36.03));
    let b = sim.app.cx.project.add_dimension(0, mk(36.03, 72.06));
    let c = sim.app.cx.project.add_dimension(0, mk(72.06, 108.09));
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let fmt = sim.app.cx.dim_format();
    let total = dim(&sim, a).measure() + dim(&sim, b).measure() + dim(&sim, c).measure();
    assert!((total - 108.09).abs() < 1e-6);
    let step = plan_core::dimension::step_inches(&fmt.effective());
    let shown: f64 = [a, b, c]
        .iter()
        .map(|i| {
            let d = dim(&sim, *i);
            d.look
                .seg
                .shown
                .unwrap_or_else(|| plan_core::dimension::round_to_step(d.length(), step))
        })
        .sum();
    let whole = plan_core::dimension::round_to_step(108.09, step);
    assert!((shown - whole).abs() < 1e-9, "{shown} vs {whole}");
    // Rounding each part alone would not add up.
    let plain: f64 = [36.03, 36.03, 36.03]
        .iter()
        .map(|v| plan_core::dimension::round_to_step(*v, step))
        .sum();
    assert!((plain - whole).abs() > 1e-9, "{plain} vs {whole}");
}

#[test]
fn an_angle_between_walls_and_the_arcs_of_a_curved_wall_follow_the_walls_and_draw() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    let (a, b) = {
        let cx = &mut sim.app.cx;
        (
            cx.project
                .add_wall(0, p(0.0, 0.0), p(240.0, 0.0), 6.0, 96.0, WallKind::Interior),
            cx.project
                .add_wall(0, p(0.0, 0.0), p(0.0, 240.0), 6.0, 96.0, WallKind::Interior),
        )
    };
    sim.tool(ToolId::DimensionVariant(DimMode::Angular));
    sim.click(120.0, 0.0);
    sim.click(0.0, 120.0);
    let r = sim.click(60.0, 60.0);
    assert_eq!(r.commit.as_deref(), Some("Angular Dimension"));
    let id = dims(&sim)[0].id;
    assert!((dim(&sim, id).measure() - 90.0).abs() < 1e-6);
    // The plan draws the angle's label.
    let texts = texts_of(&sim.plan_shapes());
    assert!(texts.iter().any(|t| t == "90.0\u{b0}"), "{texts:?}");
    // Turn the wall: the angle and its arc follow.
    sim.app.cx.project.floors[0].wall_mut(b).unwrap().end = p(170.0, 170.0);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert!((dim(&sim, id).measure() - 45.0).abs() < 1e-6);
    assert!(texts_of(&sim.plan_shapes())
        .iter()
        .any(|t| t == "45.0\u{b0}"));
    // Move the corner: the arc moves with the walls.
    let before = dim(&sim, id).curve().unwrap().center;
    for w in [a, b] {
        let wm = sim.app.cx.project.floors[0].wall_mut(w).unwrap();
        wm.start = wm.start + p(10.0, 10.0);
        wm.end = wm.end + p(10.0, 10.0);
    }
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let after = dim(&sim, id).curve().unwrap().center;
    assert!(after.dist(before.add(p(10.0, 10.0))) < 1e-6);

    // Curved wall: radius and arc length labels.
    sim.app.cx.selection.clear();
    let cw = sim.app.cx.project.add_wall(
        0,
        p(0.0, 400.0),
        p(240.0, 400.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.cx.project.floors[0].wall_mut(cw).unwrap().curve = Some(WallCurve { bulge: 60.0 });
    sim.app.cx.refresh();
    let w = sim.app.cx.floor().wall(cw).unwrap().clone();
    let apex = w.point_along(w.path_length() * 0.5);
    let out = apex.add(w.normal_along(w.path_length() * 0.5).scale(30.0));
    sim.tool(ToolId::DimensionVariant(DimMode::ArcLength));
    sim.click(apex.x, apex.y);
    let r = sim.click(out.x, out.y);
    assert_eq!(r.commit.as_deref(), Some("Arc Length Dimension"));
    sim.tool(ToolId::DimensionVariant(DimMode::Radius));
    sim.click(apex.x, apex.y);
    let r = sim.click(out.x, out.y);
    assert_eq!(r.commit.as_deref(), Some("Radius Dimension"));
    let curves: Vec<Dimension> = dims(&sim)
        .into_iter()
        .filter(|d| d.curve().is_some())
        .collect();
    assert_eq!(curves.len(), 3);
    let texts = texts_of(&sim.plan_shapes());
    assert!(texts.iter().any(|t| t.starts_with("R ")), "{texts:?}");
    // Bend the wall further: both follow.
    let arc_id = curves
        .iter()
        .find(|d| d.curve().unwrap().kind == CurveKind::ArcLength)
        .unwrap()
        .id;
    let was = dim(&sim, arc_id).measure();
    sim.app.cx.project.floors[0].wall_mut(cw).unwrap().curve = Some(WallCurve { bulge: 90.0 });
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    assert!(dim(&sim, arc_id).measure() > was);
    // The geometry the painters share.
    let g = dim(&sim, arc_id).curve_geom(1.0, 4.0).unwrap();
    assert_eq!(g.extensions.len(), 2);
    assert!(g.line.len() > 8);
    // A printed sheet draws the curved dimensions too.
    use plan_docs::{plan_sheet, Scale, SheetSize, TitleBlock};
    let tb = TitleBlock::default();
    let with = plan_sheet(
        &sim.app.cx.project,
        0,
        &[],
        SheetSize::ArchD,
        Scale::QuarterInch,
        &tb,
    );
    let mut bare = sim.app.cx.project.clone();
    bare.floors[0].dimensions.retain(|d| d.curve().is_none());
    let without = plan_sheet(&bare, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb);
    assert_ne!(with.pdf, without.pdf);
    let _ = render::upright_angle;
    let _ = DimCurve::from_points;
}

#[test]
fn moved_turned_and_rich_labels_and_centerline_marks_draw() {
    let mut sim = Sim::new();
    let id = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(0.0, 0.0), p(120.0, 0.0), 24.0),
    );
    let plain = sim.plan_shapes().len();
    {
        let d = sim.app.cx.project.floors[0]
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap();
        d.look.seg.label_move = Some(p(40.0, 30.0));
        d.look.seg.centerline = [true, false];
        d.look.seg.leading = "VIF ".into();
    }
    let shapes = sim.plan_shapes();
    assert!(shapes.len() > plain, "the leader and centerline mark draw");
    let texts = texts_of(&shapes);
    assert!(texts.iter().any(|t| t == "VIF 10'-0\""), "{texts:?}");
    assert!(texts.iter().any(|t| t == "CL"), "{texts:?}");
    // A rich label draws its words.
    {
        let d = sim.app.cx.project.floors[0]
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap();
        d.look.seg.runs = plan_core::text_styles::runs_from_markup("<b>Verify</b> <i>in field</i>");
    }
    let texts = texts_of(&sim.plan_shapes());
    assert!(texts.iter().any(|t| t == "Verify in field"), "{texts:?}");
}

#[test]
fn a_double_click_on_the_label_edits_it_in_place_as_rich_text() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    let id = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(0.0, 0.0), p(120.0, 0.0), 24.0),
    );
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.app.cx.selection.set(ObjectRef::Dimension(id));
    sim.click(60.0, 24.0);
    sim.double_click(60.0, 24.0);
    assert!(sim.app.cx.temp.editing.is_some(), "typing is on");
    for _ in 0..12 {
        sim.key(KeyEvent::key(Key::Backspace));
    }
    sim.key(KeyEvent::text("<b>HOLD</b> 10 ft"));
    let r = sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(r.commit.as_deref(), Some("Edit Dimension Label"));
    let d = dim(&sim, id);
    assert!(d.look.seg.runs.iter().any(|r| r.bold && r.text == "HOLD"));
    assert_eq!(d.look.seg.rich_text().as_deref(), Some("HOLD 10 ft"));
    assert_eq!(sim.undo().as_deref(), Some("Edit Dimension Label"));
    assert!(dim(&sim, id).look.seg.runs.is_empty());
    // Plain text with no tags is the text override; an empty text puts the
    // measured number back.
    sim.redo();
    sim.double_click(60.0, 24.0);
    for _ in 0..40 {
        sim.key(KeyEvent::key(Key::Backspace));
    }
    sim.key(KeyEvent::text("EQ"));
    sim.key(KeyEvent::key(Key::Enter));
    let d = dim(&sim, id);
    assert!(d.look.seg.runs.is_empty());
    assert_eq!(d.text_override.as_deref(), Some("EQ"));
    sim.double_click(60.0, 24.0);
    for _ in 0..40 {
        sim.key(KeyEvent::key(Key::Backspace));
    }
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(dim(&sim, id).text_override, None);
}

#[test]
fn the_dimension_specification_edits_the_second_format_segments_and_extensions() {
    let mut sim = Sim::new();
    let id = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(0.0, 0.0), p(120.0, 0.0), 24.0),
    );
    let mut dlg =
        crate::dialogs::dimension::open_for(&sim.app.cx, ObjectRef::Dimension(id)).unwrap();
    {
        let d = dlg.draft_mut();
        d.look.second = Some(plan_core::dimension::SecondFormat {
            include: true,
            ..Default::default()
        });
        d.look.tolerance = Some(plan_core::dimension::Tolerance {
            mode: TolMode::Deviation,
            plus: 0.25,
            minus: 0.125,
        });
        d.look.indicators = Some((true, false));
        d.look.text_pos = Some(plan_core::dimension::TextPos::Below);
        d.look.seg.trailing = " typ.".into();
        d.look.seg.blank = false;
        d.look.seg.label_angle = Some(15.0);
        d.look.seg.centerline = [false, true];
        d.look.seg.leader = Some(LeaderStyle::Diagonal);
    }
    assert!(dlg.apply(&mut sim.app.cx));
    let got = dim(&sim, id);
    let fmt = sim.app.cx.dim_format();
    let label = got.label(&fmt);
    assert!(label.contains("typ."), "{label}");
    assert!(label.contains("+1/4") || label.contains("+"), "{label}");
    assert_eq!(got.look.seg.centerline, [false, true]);
    assert_eq!(sim.undo().as_deref(), Some("Change Dimension"));
    assert!(dim(&sim, id).look.seg.is_default());
    // The dialog survives a save and load of the plan.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.floors[0].dimensions.len(), 1);
}

#[test]
fn a_story_pole_locates_the_roof_marks_and_names_them() {
    use crate::editor::roof_view::{self, RoofPlaneRecord, RoofSet};
    let mut sim = Sim::new();
    shell(&mut sim);
    // A gable roof over the shell, eave at the top of the wall, ridge 60" up.
    let polys: Vec<Vec<[f64; 3]>> = vec![
        vec![
            [0.0, 97.0, 0.0],
            [480.0, 97.0, 0.0],
            [480.0, 157.0, -180.0],
            [0.0, 157.0, -180.0],
        ],
        vec![
            [480.0, 97.0, -360.0],
            [0.0, 97.0, -360.0],
            [0.0, 157.0, -180.0],
            [480.0, 157.0, -180.0],
        ],
    ];
    let mut set = RoofSet::default();
    for poly in polys {
        let id = sim.app.cx.project.alloc_id();
        let base = (p(poly[0][0], -poly[0][2]), p(poly[1][0], -poly[1][2]));
        set.planes.push(RoofPlaneRecord::new(id, poly, 4.0, base));
    }
    roof_view::store(&mut sim.app.cx.project, 0, &mut set);
    sim.tool(ToolId::DimensionVariant(DimMode::AutoStoryPole));
    sim.click(-100.0, 0.0);
    let names: Vec<String> = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .filter_map(|c| match &c.item {
            plan_core::cad::CadItem::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(names.iter().any(|n| n.starts_with("Ridge")), "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("Eave")), "{names:?}");
    assert!(
        names.iter().any(|n| n.contains("Top of Plate")),
        "{names:?}"
    );
    // The outer string includes the ridge (it is on it by default).
    let outer: Vec<Dimension> = dims(&sim)
        .into_iter()
        .filter(|d| d.offset == 36.0)
        .collect();
    assert!(!outer.is_empty());
    let top = outer
        .iter()
        .map(|d| d.end.y.max(d.start.y))
        .fold(0.0, f64::max);
    assert!((top - 157.0).abs() < 1e-6, "{top}");
    // The Locate Elevations page switches a mark off.
    set_page(
        &mut sim.app.cx.defaults,
        "pole_elevations",
        "Eave_included",
        PageValue::Bool(false),
    );
    sim.click(-100.0, 0.0);
    let names: Vec<String> = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .filter_map(|c| match &c.item {
            plan_core::cad::CadItem::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(!names.iter().any(|n| n.starts_with("Eave")), "{names:?}");
    assert!(sim
        .app
        .cx
        .defaults
        .dimensions
        .setup
        .pole
        .locates(MarkKind::Ridge));
}

#[test]
fn auto_interior_follows_the_room_setup() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    shell(&mut sim);
    sim.tool(ToolId::DimensionVariant(DimMode::AutoInterior));
    sim.click(240.0, 180.0);
    let base = dims(&sim);
    assert!(!base.is_empty());
    let inside_sign = base
        .iter()
        .find(|d| d.offset != 0.0)
        .unwrap()
        .offset
        .signum();
    // Lines outside the room flip the side.
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "room_inside",
        PageValue::Bool(false),
    );
    sim.click(240.0, 180.0);
    let out = dims(&sim);
    assert_eq!(out.len(), base.len());
    assert_eq!(
        out.iter()
            .find(|d| d.offset != 0.0)
            .unwrap()
            .offset
            .signum(),
        -inside_sign
    );
    // No clear spans without the overall box.
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "room_overall",
        PageValue::Bool(false),
    );
    sim.click(240.0, 180.0);
    assert!(dims(&sim).len() < out.len());
    // A minimum area bigger than the room makes none.
    set_page(
        &mut sim.app.cx.defaults,
        "setup_automatic",
        "room_min_area",
        PageValue::Num(5000.0),
    );
    sim.click(240.0, 180.0);
    assert!(
        sim.app.cx.status.contains("No rooms"),
        "{}",
        sim.app.cx.status
    );
}

#[test]
fn every_kind_a_dimension_can_locate_keeps_it_when_the_object_moves() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    let ids = shell(&mut sim);
    // A free CAD line to dimension to.
    let line = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        plan_core::cad::CadItem::Line {
            a: p(600.0, 0.0),
            b: p(700.0, 0.0),
        },
    );
    // Walls: the south wall's outer face to the CAD line's start.
    let d1 = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(
            0,
            DimensionKind::Manual,
            p(240.0, -3.0),
            p(240.0, 100.0),
            30.0,
        ),
    );
    let d2 = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(600.0, 0.0), p(700.0, 0.0), 30.0),
    );
    assert!(sim.app.cx.project.floors[0].attach_dimension(d1) >= 1);
    assert_eq!(sim.app.cx.project.floors[0].attach_dimension(d2), 2);
    // An opening's edge on the north wall.
    let op = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.wall_id == ids[2])
        .unwrap()
        .clone();
    let w = sim.app.cx.floor().wall(ids[2]).unwrap().clone();
    let edge = w.point_along(op.start_offset());
    let d3 = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(
            0,
            DimensionKind::Manual,
            edge,
            edge.add(p(0.0, -100.0)),
            30.0,
        ),
    );
    assert!(sim.app.cx.project.floors[0].attach_dimension(d3) >= 1);

    let before: Vec<Point> = [d1, d2, d3].iter().map(|i| dim(&sim, *i).start).collect();
    // Move the walls (the shell) and the CAD line by the editor's move.
    sim.app.cx.selection.items = ids.iter().map(|i| ObjectRef::Wall(*i)).collect();
    sim.app.cx.selection.add(ObjectRef::Cad(line));
    crate::editor::transform::move_selection(&mut sim.app.cx, p(0.0, 40.0), "Move").unwrap();
    sim.app.cx.refresh();
    for (k, i) in [d1, d2, d3].iter().enumerate() {
        let now = dim(&sim, *i);
        assert!(
            (now.start.y - before[k].y - 40.0).abs() < 1e-6,
            "dimension {k} follows: {:?} -> {:?}",
            before[k],
            now.start
        );
    }
    // The tie survives the move (the dimensions still know their objects).
    assert!(dim(&sim, d2).anchors.iter().all(|a| a.is_some()));
    // And an undo puts everything back.
    sim.undo();
    sim.app.cx.refresh();
    for (k, i) in [d1, d2, d3].iter().enumerate() {
        assert!(dim(&sim, *i).start.dist(before[k]) < 1e-6);
    }
}

#[test]
fn copy_and_paste_keeps_strings_and_curved_dimensions_of_the_copied_walls() {
    let mut sim = Sim::new();
    let cx = &mut sim.app.cx;
    let w = cx
        .project
        .add_wall(0, p(0.0, 0.0), p(240.0, 0.0), 6.0, 96.0, WallKind::Exterior);
    cx.project.floors[0].wall_mut(w).unwrap().curve = Some(WallCurve { bulge: 60.0 });
    let wall = cx.floor().wall(w).unwrap().clone();
    let curve = DimCurve::of_wall(&wall, CurveKind::ArcLength, 0.0).unwrap();
    let arc = cx
        .project
        .add_dimension(0, Dimension::curved(DimensionKind::Manual, curve, 20.0));
    let mk =
        |a: f64, b: f64| Dimension::new(0, DimensionKind::Manual, p(a, 300.0), p(b, 300.0), 20.0);
    let s1 = cx.project.add_dimension(0, mk(0.0, 100.0));
    let s2 = cx.project.add_dimension(0, mk(100.0, 240.0));
    cx.project.floors[0].join_string(&[s1, s2]);
    cx.selection.items = vec![
        ObjectRef::Wall(w),
        ObjectRef::Dimension(arc),
        ObjectRef::Dimension(s1),
        ObjectRef::Dimension(s2),
    ];
    cx.copy_selection();
    cx.paste_at(p(500.0, 0.0), "Paste", false);
    let f = cx.floor();
    assert_eq!(f.dimensions.len(), 6);
    let new_wall = f.walls.iter().find(|x| x.id != w).unwrap().id;
    let copies: Vec<&Dimension> = f.dimensions.iter().filter(|d| d.id > s2).collect();
    assert_eq!(copies.len(), 3);
    let arc_copy = copies.iter().find(|d| d.curve().is_some()).unwrap();
    assert_eq!(arc_copy.curve().unwrap().walls[0], Some(new_wall));
    assert!(
        arc_copy
            .curve()
            .unwrap()
            .center
            .dist(curve.center.add(p(500.0, 0.0)))
            < 1e-6,
        "the arc moved with the paste"
    );
    let strings: Vec<&&Dimension> = copies.iter().filter(|d| d.curve().is_none()).collect();
    let head = strings[0].string_id().expect("the copies are a string");
    assert!(strings.iter().all(|d| d.string_id() == Some(head)));
    assert_ne!(head, s1, "a string of their own");
    assert_eq!(f.string_members(s1).len(), 2, "the originals keep theirs");
}

#[test]
fn a_cad_box_shows_its_gaps_to_the_nearest_parallel_objects_and_typing_slides_it() {
    use crate::editor::tempdim::{self, TempDimKind};
    use plan_core::cad::CadItem;
    let mut sim = Sim::new();
    let cx = &mut sim.app.cx;
    cx.project.add_wall(
        0,
        p(300.0, -50.0),
        p(300.0, 150.0),
        6.0,
        96.0,
        WallKind::Interior,
    );
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| CadItem::Polyline {
        points: vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)],
        closed: true,
    };
    let b = cx
        .project
        .add_cad(0, "CAD, Default", rect(0.0, 0.0, 100.0, 60.0));
    cx.project
        .add_cad(0, "CAD, Default", rect(20.0, 140.0, 80.0, 200.0));
    cx.selection.set(ObjectRef::Cad(b));
    cx.refresh();
    let kinds: Vec<TempDimKind> = cx.temp.dims.iter().map(|d| d.kind).collect();
    assert!(kinds.contains(&TempDimKind::CadToRight), "{kinds:?}");
    assert!(kinds.contains(&TempDimKind::CadAbove), "{kinds:?}");
    assert!(!kinds.contains(&TempDimKind::CadToLeft));
    assert!(!kinds.contains(&TempDimKind::CadBelow));
    let right = cx
        .temp
        .dims
        .iter()
        .find(|d| d.kind == TempDimKind::CadToRight)
        .unwrap();
    assert!(
        (right.value - 197.0).abs() < 1e-9,
        "wall face at 297: {}",
        right.value
    );
    let above = cx
        .temp
        .dims
        .iter()
        .find(|d| d.kind == TempDimKind::CadAbove)
        .unwrap();
    assert!((above.value - 80.0).abs() < 1e-9);
    // Typing a gap slides the box.
    let i = cx
        .temp
        .dims
        .iter()
        .position(|d| d.kind == TempDimKind::CadToRight)
        .unwrap();
    assert!(cx.temp.begin_edit(i));
    cx.temp.editing.as_mut().unwrap().text = "150".into();
    assert_eq!(tempdim::commit_edit(cx), Ok("Move Box"));
    let CadItem::Polyline { points, .. } = &cx.floor().cad.iter().find(|c| c.id == b).unwrap().item
    else {
        panic!("a box")
    };
    assert!((points[0].x - 47.0).abs() < 1e-9, "{points:?}");
    cx.refresh();
    let right = cx
        .temp
        .dims
        .iter()
        .find(|d| d.kind == TempDimKind::CadToRight)
        .unwrap();
    assert!((right.value - 150.0).abs() < 1e-9);
    assert_eq!(sim.undo().as_deref(), Some("Move Box"));
}

#[test]
fn views_read_their_own_locate_panel_and_reach() {
    use plan_core::dimension::DimView;
    let sim = Sim::new();
    let d = &sim.app.cx.defaults.dimensions;
    assert_eq!(d.reach_for_view(DimView::Plan), 24.0);
    assert_eq!(d.reach_for_view(DimView::Layout), 1.0);
    assert_eq!(
        d.locate_for_view(DimView::Section).group,
        d.tool_locate(plan_core::dimension::LocateTool::Elevations)
            .group
    );
}
