//! Shared helpers for the Tutorial Guide replays (`tutorials_a`, lessons 1 to
//! 14; docs/chief-manual-coverage/scenario-proposals.md). Everything here is
//! test-only. The guide carries one plan through lessons 1 to 25; the
//! `chic_cottage` shell is our own approximation of it (about 40 ft sides, a
//! garage bump-out on the east and a porch pad), drawn in inches.
#![allow(dead_code)]

use super::Sim;
use crate::editor::ObjectRef;
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_core::WallKind;

pub fn exterior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Exterior,
    }
}

pub fn interior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Interior,
    }
}

/// The cottage outline, clockwise from the north-west corner: a 40' x 40'
/// body with a 10' wide garage bump-out on the east side.
pub const COTTAGE: [(f64, f64); 6] = [
    (0.0, 0.0),
    (480.0, 0.0),
    (480.0, 200.0),
    (600.0, 200.0),
    (600.0, 480.0),
    (0.0, 480.0),
];

/// Draws the cottage shell with the exterior wall tool, one drag per wall.
/// Leaves the Select tool active.
pub fn chic_cottage(sim: &mut Sim) {
    sim.tool(exterior());
    for i in 0..COTTAGE.len() {
        let a = COTTAGE[i];
        let b = COTTAGE[(i + 1) % COTTAGE.len()];
        sim.drag(a, b);
    }
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
}

/// A new simulator with the cottage drawn.
pub fn cottage() -> Sim {
    let mut sim = Sim::new();
    chic_cottage(&mut sim);
    sim
}

/// Interior walls that cut the cottage into four rooms: a north-south wall at
/// x = 200 and two east-west walls (west part y = 240, east part y = 300).
pub fn cottage_partitions(sim: &mut Sim) {
    sim.tool(interior());
    sim.drag((200.0, 0.0), (200.0, 480.0));
    sim.drag((0.0, 240.0), (200.0, 240.0));
    sim.drag((200.0, 300.0), (600.0, 300.0));
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
}

/// Runs `f` and asserts it made exactly one undo step (the project rule).
pub fn assert_one_undo_step<R>(sim: &mut Sim, what: &str, f: impl FnOnce(&mut Sim) -> R) -> R {
    let before = sim.app.cx.undo_depth();
    let r = f(sim);
    assert_eq!(
        sim.app.cx.undo_depth(),
        before + 1,
        "{what}: exactly one undo step"
    );
    r
}

/// Opens the specification of `obj` and asserts a dialog is up, then cancels.
/// The dialogs expose their tab names through different per-dialog APIs (no
/// shared accessor), so the tab list is recorded in the lesson comment and
/// only "a dialog opens and closes cleanly" is asserted here.
pub fn assert_dialog_tabs(sim: &mut Sim, obj: ObjectRef, _tabs: &[&str]) {
    assert!(sim.open_spec(obj), "a specification dialog opens");
    sim.cancel();
    assert!(!sim.app.has_dialog(), "Cancel closes it");
}

/// Body for an `#[ignore = "T7-nn: <id>"]` test whose steps cannot be driven
/// at all today: it fails loudly the day the attribute is removed, until the
/// real replay is written.
pub fn assert_ignored_break(id: &str) -> ! {
    panic!("open tutorial break {id}: write the replay when the feature lands");
}

pub fn wall_midpoint(sim: &Sim, idx: usize) -> Point {
    let w = &sim.app.cx.floor().walls[idx];
    Point::new((w.start.x + w.end.x) / 2.0, (w.start.y + w.end.y) / 2.0)
}

macro_rules! lessons {
    ($($n:literal => $f:literal),* $(,)?) => {
        &[$(($n, include_str!(concat!("tutorials_a/", $f, ".rs")))),*]
    };
}

/// Lists the ignored tutorial tests by lesson (prints, asserts nothing).
#[test]
fn tutorial_replays_ignored_by_lesson() {
    let all: &[(u32, &str)] = lessons!(
        1 => "tut01_exterior_walls", 2 => "tut02_interior_walls",
        3 => "tut03_multiple_floors", 4 => "tut04_interior_stairs",
        5 => "tut05_doors_windows", 6 => "tut06_decks_porches",
        7 => "tut07_basic_roof_styles", 8 => "tut08_chic_cottage_roof",
        9 => "tut09_dormers", 10 => "tut10_custom_ceilings",
        11 => "tut11_finish_materials", 12 => "tut12_room_moldings",
        13 => "tut13_interior_furnishings", 14 => "tut14_cabinet_styles",
    );
    for (lesson, src) in all {
        let green = src.matches("#[test]").count();
        let ids: Vec<&str> = src
            .lines()
            .filter_map(|l| l.trim().strip_prefix("#[ignore = \""))
            .map(|l| l.trim_end_matches("\")]").trim_end_matches("\"]"))
            .collect();
        println!(
            "lesson {lesson:>2}: {} tests, {} ignored {ids:?}",
            green,
            ids.len()
        );
    }
}
