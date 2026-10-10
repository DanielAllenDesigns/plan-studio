//! Lesson 22, Plot Plans (pp. 382-403). Real: the guide's first three
//! survey courses typed in Input Line (Round 16 brief 06). Open: Disconnect
//! Edges, the Lock Chord arc and the CAD Defaults number style.
use crate::scenarios::tutorials_support::*;
use crate::scenarios::Sim;
use crate::toolbar::ViewFlag;
use crate::tools::cad::{survey, CadMode};
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::Key;
use plan_core::bearing::NumberStyle;
use plan_core::cad::CadItem;
use plan_core::geometry::Point;

/// (bearing, distance in feet) as the guide types them.
const COURSES: [(&str, &str); 3] = [
    ("N 61 25 10 E", "155.69"),
    ("S 28 29 35 E", "118.65"),
    ("N 49 59 11 E", "154.37"),
];

fn type_text(sim: &mut Sim, s: &str) {
    sim.key(KeyEvent::text(s));
}

fn enter() -> Sim {
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

fn lines(sim: &Sim) -> Vec<(Point, Point)> {
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .filter_map(|c| match c.item {
            CadItem::Line { a, b } => Some((a, b)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_guides_first_three_courses_chain_at_the_surveyed_bearings() {
    let sim = enter();
    let ls = lines(&sim);
    assert_eq!(ls.len(), 3, "one line per course");
    assert_eq!(sim.app.cx.undo_depth(), 3, "one undo step per line");
    // +y is north. Azimuths from the north: 61 25 10 E, 151 30 25 (S 28 29 35 E), 49 59 11 E.
    let dms = |d: f64, m: f64, s: f64| (d + m / 60.0 + s / 3600.0).to_radians();
    let legs = [
        (dms(61.0, 25.0, 10.0), 155.69),
        (dms(151.0, 30.0, 25.0), 118.65),
        (dms(49.0, 59.0, 11.0), 154.37),
    ];
    let mut at = Point::ZERO;
    for (i, (az, feet)) in legs.iter().enumerate() {
        let d = feet * 12.0;
        let next = Point::new(at.x + d * az.sin(), at.y + d * az.cos());
        assert!(
            ls[i].0.dist(at) < 0.01,
            "course {i} starts where the last ended"
        );
        assert!(
            ls[i].1.dist(next) < 0.05,
            "course {i}: {:?} vs {next:?}",
            ls[i].1
        );
        at = next;
    }
}

#[test]
fn the_entered_courses_undo_one_line_at_a_time() {
    let mut sim = enter();
    sim.undo();
    assert_eq!(lines(&sim).len(), 2);
}

#[test]
#[ignore = "T7-22: S-172 (Disconnect Edges fixes one bad segment)"]
fn disconnect_edges() {
    assert_ignored_break("S-172");
}

#[test]
#[ignore = "T7-22: CAD-22 (Change Line/Arc: arc closing edge with Lock Chord radius 450 ft)"]
fn closing_arc_lock_chord() {
    assert_ignored_break("CAD-22");
}

#[test]
#[ignore = "T7-22: PR-31 (CAD Defaults: decimal feet, 2 places, quadrant bearing display for the plot plan view)"]
fn cad_defaults_number_style() {
    assert_ignored_break("PR-31");
}
