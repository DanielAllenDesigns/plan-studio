//! Scenario 65: survey-style entry, Round 16 brief 06. A six-course plot
//! plan typed as quadrant bearings and distances in Input Line, Next
//! chaining (Connect CAD Segments), live Show Length / Show Angle labels,
//! one undo step per line, and Convert to Terrain Perimeter.

use super::Sim;
use crate::editor::site_view::load_terrain;
use crate::toolbar::ViewFlag;
use crate::tools::cad::{survey, CadMode};
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::Key;
use plan_core::bearing::{closure_error, course_between, NumberStyle};
use plan_core::cad::{CadItem, CadLabels};
use plan_core::geometry::Point;
use plan_core::Id;

/// (bearing, distance in feet) as typed. The last course closes the lot.
const COURSES: [(&str, &str); 6] = [
    ("N", "100"),
    ("E", "60"),
    ("S 45 E", "28.2842712475"),
    ("S", "40"),
    ("S 45 W", "28.2842712475"),
    ("S 71 33 54.1842 W", "63.2455532034"),
];

fn type_text(sim: &mut Sim, s: &str) {
    sim.key(KeyEvent::text(s));
}

fn lines(sim: &Sim) -> Vec<(Id, Point, Point)> {
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .filter_map(|c| match c.item {
            CadItem::Line { a, b } => Some((c.id, a, b)),
            _ => None,
        })
        .collect()
}

/// Draws the six courses with the Input Line tool, one Enter per course.
fn enter_lot() -> Sim {
    survey::reset();
    survey::set_number_style(NumberStyle::survey());
    let mut sim = Sim::new();
    sim.cx().view_flags.insert(ViewFlag::ConnectCad);
    sim.tool(ToolId::CadVariant(CadMode::InputLine));
    sim.click(0.0, 0.0);
    for (bearing, feet) in COURSES {
        type_text(&mut sim, feet);
        sim.key(KeyEvent::key(Key::Tab));
        type_text(&mut sim, bearing);
        sim.key(KeyEvent::key(Key::Enter));
    }
    sim.esc();
    sim.esc();
    sim
}

#[test]
fn six_typed_courses_close_the_lot() {
    let sim = enter_lot();
    let ls = lines(&sim);
    assert_eq!(ls.len(), 6, "one line per course");
    // Next chaining: each course starts where the last ended.
    for w in ls.windows(2) {
        assert!(w[0].2.dist(w[1].1) < 1e-9);
    }
    assert!(ls[0].1.dist(Point::ZERO) < 1e-9);
    // The typed distances and bearings landed where the survey says.
    assert!(ls[0].2.dist(Point::new(0.0, 1200.0)) < 1e-6);
    assert!(ls[2].2.dist(Point::new(960.0, 960.0)) < 1e-3);
    // Closure error under 1e-3 in, by the traverse and by the drawn lines.
    let courses: Vec<_> = ls.iter().map(|(_, a, b)| course_between(*a, *b)).collect();
    assert!(closure_error(Point::ZERO, &courses) < 1e-3);
    assert!(ls[5].2.dist(ls[0].1) < 1e-3, "{:?}", ls[5].2);
}

#[test]
fn show_length_and_angle_equal_the_typed_values() {
    let mut sim = enter_lot();
    let st = survey::number_style();
    let ls = lines(&sim);
    let flags = CadLabels {
        show_length: true,
        show_angle: true,
        ..CadLabels::default()
    };
    for (i, (id, _, _)) in ls.iter().enumerate() {
        let fl = sim.app.cx.floor;
        sim.cx()
            .project
            .edit_cad_attrs(fl, *id, |a| a.labels = flags);
        let c = sim.app.cx.floor().cad.iter().find(|c| c.id == *id).unwrap();
        let got = survey::edge_labels(&c.item, &flags, &st);
        let (bearing, feet) = COURSES[i];
        let typed_len = survey::parse_length(feet).unwrap();
        let typed_dir = survey::parse_angle_text(bearing).unwrap();
        assert_eq!(got[0].text, st.format_length(typed_len), "length {i}");
        assert_eq!(got[1].text, st.format_angle(typed_dir), "angle {i}");
    }
    let c0 = sim.app.cx.floor().cad[0].clone();
    let l = survey::edge_labels(&c0.item, &flags, &st);
    assert_eq!((l[0].text.as_str(), l[1].text.as_str()), ("100'", "N"));
}

#[test]
fn each_entered_line_is_one_undo_step() {
    let mut sim = enter_lot();
    for left in (0..6).rev() {
        assert!(sim.undo().is_some());
        assert_eq!(lines(&sim).len(), left);
    }
    for n in 1..=6 {
        assert!(sim.redo().is_some());
        assert_eq!(lines(&sim).len(), n);
    }
}

#[test]
fn the_lot_converts_to_a_terrain_perimeter_and_a_cad_detail() {
    let mut sim = enter_lot();
    let ids: Vec<Id> = lines(&sim).iter().map(|l| l.0).collect();
    let miss = survey::convert_to_terrain_perimeter(sim.cx(), &ids).expect("converts");
    assert!(miss < 1e-3);
    let per = load_terrain(&sim.app.cx.project).unwrap().terrain.perimeter;
    assert_eq!(per.len(), 6);
    assert!(per[3].dist(Point::new(960.0, 960.0)) < 1e-3);
    // One undo step takes the perimeter back, the lines stay.
    sim.undo();
    let has = load_terrain(&sim.app.cx.project).is_some_and(|r| !r.terrain.perimeter.is_empty());
    assert!(!has);
    assert_eq!(lines(&sim).len(), 6);
    // An open chain is refused.
    assert!(survey::convert_to_terrain_perimeter(sim.cx(), &ids[..4]).is_err());
}

#[test]
fn bad_bearings_are_refused_without_drawing() {
    survey::reset();
    survey::set_number_style(NumberStyle::survey());
    let mut sim = Sim::new();
    let r = survey::enter_traverse(sim.cx(), Point::ZERO, &[("N 95 E", "10")]);
    assert!(r.is_err());
    assert!(lines(&sim).is_empty());
    let r = survey::enter_traverse(sim.cx(), Point::ZERO, &[("N 45 E", "10"), ("Q", "5")]);
    assert!(r.is_err(), "nothing drawn when any course is bad");
    assert!(lines(&sim).is_empty());
}

#[test]
fn the_number_style_is_stored_in_the_plan_and_follows_undo() {
    survey::reset();
    survey::set_number_style(NumberStyle::default());
    let mut sim = Sim::new();
    sim.cx().refresh();
    assert!(sim.app.cx.project.number_style.is_default());
    assert!(survey::store_number_style(sim.cx(), NumberStyle::survey()));
    assert_eq!(sim.app.cx.project.number_style, NumberStyle::survey());
    assert_eq!(survey::number_style(), NumberStyle::survey());
    // Storing the same style again is not a step.
    assert!(!survey::store_number_style(sim.cx(), NumberStyle::survey()));
    // The plan file keeps it and a plan without it reads the default.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    assert!(json.contains("number_style"));
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.number_style, NumberStyle::survey());
    // Undo is one step and the style in force follows it.
    sim.undo();
    sim.cx().refresh();
    assert!(sim.app.cx.project.number_style.is_default());
    assert_eq!(survey::number_style(), NumberStyle::default());
    sim.redo();
    sim.cx().refresh();
    assert_eq!(survey::number_style(), NumberStyle::survey());
    survey::set_number_style(NumberStyle::default());
}

#[test]
fn delete_removes_temporary_points_in_reverse_order_and_selecting_one_makes_it_current() {
    use plan_core::cad::CadItem as Item;
    survey::reset();
    survey::set_number_style(NumberStyle::survey());
    let mut sim = Sim::new();
    let spots = [(10.0, 10.0), (50.0, 10.0), (50.0, 50.0)];
    for (x, y) in spots {
        survey::enter_point(
            sim.cx(),
            survey::PointSpec::Absolute { x, y },
            survey::PointKind::Input,
        )
        .unwrap();
    }
    let count = |sim: &Sim| sim.app.cx.floor().cad.len();
    assert_eq!(count(&sim), 6, "a cross of two lines per point");
    assert_eq!(survey::current_point(), Some(Point::new(50.0, 50.0)));
    // Selecting the first point makes it current (the refresh hook).
    let first_id = sim.app.cx.floor().cad[0].id;
    sim.cx()
        .selection
        .set(crate::editor::ObjectRef::Cad(first_id));
    sim.cx().refresh();
    assert_eq!(survey::current_point(), Some(Point::new(10.0, 10.0)));
    sim.cx().selection.clear();
    // Delete with nothing selected takes the latest point first.
    assert!(survey::delete_key(sim.cx()));
    assert_eq!(count(&sim), 4);
    assert!(survey::delete_key(sim.cx()));
    assert!(survey::delete_key(sim.cx()));
    assert_eq!(count(&sim), 0);
    assert!(!survey::delete_key(sim.cx()));
    // Each delete is one undo step.
    sim.undo();
    assert_eq!(count(&sim), 2);
    let _ = Item::Line {
        a: Point::ZERO,
        b: Point::ZERO,
    };
}

#[test]
fn move_point_is_one_undo_step_and_the_point_becomes_current() {
    survey::reset();
    let mut sim = Sim::new();
    let from = survey::enter_point(
        sim.cx(),
        survey::PointSpec::Absolute { x: 100.0, y: 100.0 },
        survey::PointKind::Input,
    )
    .unwrap();
    // Open Object on its cross finds the point (the double-click hook).
    let cross = sim.app.cx.floor().cad[0].id;
    assert_eq!(survey::temporary_point_of(&sim.app.cx, cross), Some(from));
    assert!(survey::temporary_point_at(&sim.app.cx, Point::new(101.0, 99.0), 5.0).is_some());
    assert!(survey::temporary_point_at(&sim.app.cx, Point::new(300.0, 99.0), 5.0).is_none());
    let to = survey::move_point(
        sim.cx(),
        from,
        survey::PointSpec::Polar {
            dist: 50.0,
            angle: 90.0,
        },
    )
    .unwrap();
    assert!(to.dist(Point::new(100.0, 150.0)) < 1e-9);
    assert_eq!(survey::current_point(), Some(to));
    // The cross moved with it.
    assert!(survey::temporary_point_at(&sim.app.cx, to, 1.0).is_some());
    assert!(survey::temporary_point_at(&sim.app.cx, from, 1.0).is_none());
    sim.undo();
    assert!(survey::temporary_point_at(&sim.app.cx, from, 1.0).is_some());
    // Something that is not a temporary point is refused.
    assert!(survey::move_point(
        sim.cx(),
        Point::new(5000.0, 5000.0),
        survey::PointSpec::Relative { dx: 1.0, dy: 1.0 }
    )
    .is_err());
}

#[test]
fn new_cad_arcs_chain_with_next_and_undo_one_at_a_time() {
    use crate::tools::cad::arcs::{Curve, Extent};
    survey::reset();
    survey::set_number_style(NumberStyle::survey());
    let mut sim = Sim::new();
    // A 90 degree curve of radius 100 ft heading north, curving right.
    let first = survey::enter_arc(
        sim.cx(),
        survey::ArcSpec {
            start: survey::StartSpec::Absolute { x: 0.0, y: 0.0 },
            direction: survey::DirSpec::Angle(survey::parse_angle_text("N").unwrap()),
            radius: survey::parse_length("100").unwrap(),
            curve: Curve::Right,
            extent: Extent::Angle(90.0),
            chord: false,
        },
    )
    .unwrap();
    assert!(
        first.end.dist(Point::new(1200.0, 1200.0)) < 1e-6,
        "{:?}",
        first.end
    );
    // Next: leaves the end heading east, a compound curve to the left.
    let second = survey::enter_arc(
        sim.cx(),
        survey::ArcSpec {
            start: survey::StartSpec::Relative { dx: 0.0, dy: 0.0 },
            direction: survey::DirSpec::Previous { turn: 0.0 },
            radius: survey::parse_length("50").unwrap(),
            curve: Curve::Left,
            extent: Extent::Angle(90.0),
            chord: false,
        },
    )
    .unwrap();
    assert!(second.start.dist(first.end) < 1e-9);
    assert!(
        second.end.dist(Point::new(1800.0, 1800.0)) < 1e-6,
        "{:?}",
        second.end
    );
    let arcs = |sim: &Sim| {
        sim.app
            .cx
            .floor()
            .cad
            .iter()
            .filter(|c| matches!(c.item, CadItem::Arc { .. }))
            .count()
    };
    assert_eq!(arcs(&sim), 2);
    sim.undo();
    assert_eq!(arcs(&sim), 1);
    // A radius of nothing is refused.
    assert!(survey::enter_arc(
        sim.cx(),
        survey::ArcSpec {
            start: survey::StartSpec::Absolute { x: 0.0, y: 0.0 },
            direction: survey::DirSpec::Angle(0.0),
            radius: 0.0,
            curve: Curve::Left,
            extent: Extent::Angle(10.0),
            chord: false,
        }
    )
    .is_err());
    survey::set_number_style(NumberStyle::default());
}

#[test]
fn the_survey_commands_open_their_dialogs() {
    use crate::dialogs::{input_arc, input_line, number_style};
    survey::reset();
    let mut sim = Sim::new();
    assert!(crate::tools::cad::is_command(survey::NEW_LINE));
    crate::tools::cad::run_command(sim.cx(), survey::NEW_LINE);
    assert!(input_line::is_open());
    crate::tools::cad::run_command(sim.cx(), survey::NEW_ARC);
    assert!(input_arc::is_open());
    crate::tools::cad::run_command(sim.cx(), survey::NUMBER_STYLE);
    assert!(number_style::is_open());
    // Draw a line from the dialog's own form: Next restarts at its end.
    let mut form = input_line::Form::new(false);
    form.end_a = "10'".into();
    form.end_b = "90".into();
    assert!(input_line::apply(sim.cx(), &mut form));
    form.next();
    form.end_a = "5'".into();
    form.end_b = "0".into();
    assert!(input_line::apply(sim.cx(), &mut form));
    let ls = lines(&sim);
    assert_eq!(ls.len(), 2);
    assert!(ls[1].1.dist(ls[0].2) < 1e-9 && ls[1].2.dist(Point::new(60.0, 120.0)) < 1e-9);
    // A form that does not read draws nothing and says why.
    form.end_a = "x".into();
    assert!(!input_line::apply(sim.cx(), &mut form));
    assert!(form.error.is_some());
    assert_eq!(lines(&sim).len(), 2);
    input_line::close();
    input_arc::close();
    number_style::close();
}
