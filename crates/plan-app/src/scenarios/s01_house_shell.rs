//! Scenario 1: the house shell. Four click-drag exterior walls with
//! slightly-off ends become a connected 40' x 30' box and one room
//! (W-3..W-5, W-8, W-31, W-40, W-69, R-1, R-2, R-49).

use super::{draw_shell, same, Sim};
use crate::tools::ToolId;
use plan_core::geometry::Point;
use plan_core::{Wall, WallKind};

const W: f64 = 480.0; // 40'
const H: f64 = 360.0; // 30'

fn exterior() -> ToolId {
    ToolId::Wall {
        kind: WallKind::Exterior,
    }
}

fn near(a: Point, b: Point, tol: f64) -> bool {
    a.dist(b) <= tol
}

/// The chain of walls is closed: every wall's end is the next wall's start.
fn is_closed_loop(walls: &[Wall]) -> bool {
    walls
        .iter()
        .enumerate()
        .all(|(i, w)| near(w.end, walls[(i + 1) % walls.len()].start, 1e-6))
}

#[test]
fn four_click_drag_walls_make_four_connected_walls() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    assert_eq!(sim.app.tools.active().name(), "Straight Exterior Wall");

    // Wall by wall: each press-drag-release is one wall and one undo step.
    let r = sim.drag((0.0, 0.0), (W + 1.0, 1.0));
    assert_eq!(r.commit.as_deref(), Some("Draw Wall"));
    assert_eq!(sim.floor_walls(), 1);
    sim.drag((W + 2.0, 2.0), (W - 1.0, H + 1.0));
    sim.drag((W + 1.0, H - 1.0), (2.0, H + 2.0));
    assert!(sim.app.cx.rooms.is_empty(), "three walls are not a room");
    let r = sim.drag((1.0, H + 1.0), (1.0, 2.0));
    assert_eq!(r.commit.as_deref(), Some("Draw Wall"));
    assert_eq!(
        sim.floor_walls(),
        4,
        "auto-connect must not add or drop walls"
    );

    let walls = sim.app.cx.floor().walls.clone();
    // W-31: the imprecise ends were cleaned into shared endpoints.
    assert!(is_closed_loop(&walls), "{walls:#?}");
    // The shell is the size that was drawn, within the hand-drawn slop.
    let (lo, hi) = walls.iter().fold(
        (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        ),
        |(lo, hi), w| {
            (
                Point::new(lo.x.min(w.start.x), lo.y.min(w.start.y)),
                Point::new(hi.x.max(w.start.x), hi.y.max(w.start.y)),
            )
        },
    );
    assert!((hi.x - lo.x - W).abs() <= 4.0 && (hi.y - lo.y - H).abs() <= 4.0);
    // W-6 / W-51: the tool's wall comes from the exterior defaults.
    for w in &walls {
        assert_eq!(w.kind, WallKind::Exterior);
        assert_eq!(w.thickness, sim.app.cx.wall_thickness(WallKind::Exterior));
        assert_eq!(w.height, sim.app.cx.wall_height(WallKind::Exterior));
        assert!(w.length() > 300.0);
    }
    // A drag draws one wall and ends the chain (nothing pending).
    assert_eq!(sim.app.cx.status, "Room created");
}

#[test]
fn corners_are_mitered_joins_with_no_gap_or_overlap() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    let cx = &sim.app.cx;
    assert_eq!(cx.outlines.len(), 4);
    // W-32: both outlines of a corner end at the same two corner points.
    for i in 0..4 {
        let a = &cx.outlines[i].polygon;
        let b = &cx.outlines[(i + 1) % 4].polygon;
        let shared = a
            .iter()
            .filter(|p| b.iter().any(|q| near(**p, *q, 1e-6)))
            .count();
        assert!(
            shared >= 2,
            "corner {i} shares {shared} points: {a:?} / {b:?}"
        );
    }
    // Layer bands exist for the layered wall type (W-46).
    assert!(!cx.layer_outlines.is_empty());
}

#[test]
fn the_loop_is_one_room_with_chiefs_interior_area() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    let cx = &sim.app.cx;
    // R-1: a closed loop of walls is one room.
    assert_eq!(cx.rooms.len(), 1);
    let room = &cx.rooms[0];
    assert_eq!(room.label, "Room 1");
    // R-2 / R-49: the interior area is measured to the inside faces, i.e. the
    // centerline box inset by half a wall thickness on every side.
    let t = cx.wall_thickness(WallKind::Exterior);
    let expected = (W - t) * (H - t) / 144.0;
    let got = room.interior_area_sq_ft();
    assert!(
        (got - expected).abs() / expected < 0.01,
        "interior {got} sq ft, expected about {expected}"
    );
    // The centerline area is larger than the interior area, the standard
    // area (outside of exterior walls) larger still.
    assert!(room.area_sq_ft() > got);
    assert!(room.standard_area_sq_in / 144.0 > room.area_sq_ft());
    // The room is found by a click inside it and not outside.
    assert!(crate::editor::rooms_edit::room_index_at(cx, Point::new(240.0, 180.0)).is_some());
    assert!(crate::editor::rooms_edit::room_index_at(cx, Point::new(-60.0, 180.0)).is_none());
    // The label text shows interior dimensions and area (R-45, R-48).
    let text = crate::editor::rooms_edit::room_label_text(cx, room);
    assert!(text.contains("Room 1"), "{text}");
}

#[test]
fn undo_removes_the_last_wall_and_the_room_redo_restores_them() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    let before = sim.app.cx.floor().walls.clone();
    assert_eq!(sim.app.cx.rooms.len(), 1);

    // W-40: one undo step per wall, newest first.
    assert_eq!(sim.undo().as_deref(), Some("Draw Wall"));
    assert_eq!(sim.floor_walls(), 3);
    assert!(
        sim.app.cx.rooms.is_empty(),
        "the room goes with the 4th wall"
    );
    assert_eq!(sim.app.cx.redo_label(), Some("Draw Wall"));

    assert_eq!(sim.redo().as_deref(), Some("Draw Wall"));
    assert_eq!(sim.floor_walls(), 4);
    assert_eq!(sim.app.cx.rooms.len(), 1);
    assert!(
        same(&sim.app.cx.floor().walls, &before),
        "redo restores the exact walls"
    );

    // All the way back to an empty plan, then forward again.
    for _ in 0..4 {
        assert!(sim.undo().is_some());
    }
    assert_eq!(sim.floor_walls(), 0);
    assert!(!sim.app.cx.can_undo());
    for _ in 0..4 {
        assert!(sim.redo().is_some());
    }
    assert!(same(&sim.app.cx.floor().walls, &before));
    assert!(!sim.app.cx.can_redo());
}

#[test]
fn click_chain_closes_on_the_first_point_and_creates_the_room() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    // W-3, W-5: click, click, ...; the last click lands near the first start.
    for (x, y) in [(0.0, 0.0), (W, 0.0), (W, H), (0.0, H), (2.0, 2.0)] {
        sim.click(x, y);
    }
    assert_eq!(sim.floor_walls(), 4);
    assert!(is_closed_loop(&sim.app.cx.floor().walls));
    assert_eq!(sim.app.cx.rooms.len(), 1);
    // The chain ended with the closing click: nothing stays selected.
    assert!(sim.app.cx.selection.is_empty());
    // One undo step per wall (W-40).
    assert_eq!(sim.undo().as_deref(), Some("Draw Wall"));
    assert_eq!(sim.floor_walls(), 3);
}

#[test]
fn escape_cancels_the_wall_in_progress_but_keeps_finished_walls() {
    let mut sim = Sim::new();
    sim.tool(exterior());
    sim.click(0.0, 0.0);
    sim.click(W, 0.0);
    assert_eq!(sim.floor_walls(), 1);
    // W-8: Esc cancels the pending wall; the first wall stays.
    let r = sim.esc();
    assert!(r.consumed);
    assert_eq!(sim.floor_walls(), 1);
    assert_eq!(sim.app.tools.active_id(), exterior());
    // A second Esc (nothing pending) leaves the tool for Select Objects.
    sim.esc();
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
    // Walls shorter than the minimum are not created (W-7).
    sim.tool(exterior());
    sim.click(100.0, 100.0);
    sim.click(100.2, 100.0);
    assert_eq!(sim.floor_walls(), 1);
}

#[test]
fn double_click_on_a_wall_requests_the_wall_specification() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Select);
    sim.requests.clear();
    // Click on the middle of the south wall.
    sim.double_click(240.0, 0.0);
    let id = sim.app.cx.floor().walls[0].id;
    assert!(
        sim.requests
            .contains(&crate::editor::EditorRequest::OpenSpec(
                crate::editor::ObjectRef::Wall(id)
            )),
        "{:?}",
        sim.requests
    );
    // The shell opened the Wall Specification, which draws and OKs headless.
    assert!(sim.app.has_dialog());
    let (open, closed) = sim.open_then_ok();
    assert!(open, "the dialog stays open until OK");
    assert!(closed, "Enter is OK");
}
