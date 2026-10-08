//! Scenario 10: documents made from the finished house: the construction
//! set PDF, the DXF export, schedules and the materials list
//! (documentation-layout parity, DW-60, R-49).

use super::{draw_shell, Sim};
use crate::dialogs::build_tools::{
    construction_set_pdf, materials_csv, materials_for, schedule_for, SchedKind,
};
use crate::dialogs::exchange::floor_dxf;
use crate::editor::rooms_edit;
use crate::tools::ToolId;
use plan_core::geometry::Point;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.app.cx.project.name = "Maple Court Residence".into();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.click(100.0, H + 1.0);
    sim.tool(ToolId::Wall {
        kind: plan_core::WallKind::Interior,
    });
    sim.drag((240.0, 0.0), (240.0, H + 1.0));
    sim.app.cx.refresh();
    sim
}

fn pdf_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Number of page objects (`/Type /Page`, not `/Pages`).
fn pdf_pages(text: &str) -> usize {
    text.match_indices("/Type /Page")
        .filter(|(i, _)| !text[i + "/Type /Page".len()..].starts_with('s'))
        .count()
}

#[test]
fn the_construction_set_is_a_multi_page_pdf_with_the_project_name() {
    let sim = house();
    let bytes = construction_set_pdf(&sim.app.cx.project);
    assert!(bytes.starts_with(b"%PDF"));
    let text = pdf_text(&bytes);
    let pages = pdf_pages(&text);
    assert!(pages >= 4, "{pages} pages");
    assert!(
        text.to_uppercase().contains("MAPLE COURT RESIDENCE"),
        "the project name is on the sheets"
    );
    assert!(text.contains("%%EOF"));
    assert!(bytes.len() > 20_000, "{} bytes", bytes.len());
}

#[test]
fn a_second_floor_adds_a_floor_plan_sheet() {
    let mut sim = house();
    let one = pdf_pages(&pdf_text(&construction_set_pdf(&sim.app.cx.project)));
    sim.action(crate::toolbar::Action::BuildNewFloor);
    sim.ok();
    let two = pdf_pages(&pdf_text(&construction_set_pdf(&sim.app.cx.project)));
    assert!(two > one, "{one} -> {two} pages");
}

#[test]
fn the_dxf_export_has_the_wall_layer_and_the_openings() {
    let mut sim = house();
    let dxf = floor_dxf(&mut sim.app.cx);
    assert!(dxf.contains("SECTION") && dxf.contains("EOF"));
    // Walls go out on their layer, doors on Doors, windows on Windows.
    assert!(dxf.contains("Walls, Normal"), "wall layer");
    assert!(dxf.contains("Doors"), "door layer");
    assert!(dxf.contains("Windows"), "window layer");
    // R12 ASCII with polylines for the walls.
    assert!(dxf.matches("POLYLINE").count() >= 4);
}

#[test]
fn schedules_and_the_materials_list_are_not_empty() {
    let sim = house();
    for kind in [
        SchedKind::Door,
        SchedKind::Window,
        SchedKind::Room,
        SchedKind::Wall,
    ] {
        let s = schedule_for(&sim.app.cx, kind);
        assert!(!s.rows.is_empty(), "{kind:?} schedule is empty");
        let csv = s.to_csv();
        assert!(csv.lines().count() >= 2, "{csv}");
    }
    let doors = schedule_for(&sim.app.cx, SchedKind::Door);
    assert_eq!(doors.rows.len(), 1, "{:?}", doors.rows);
    // One window on the south wall, one on the north wall.
    let windows = schedule_for(&sim.app.cx, SchedKind::Window);
    assert_eq!(windows.rows.len(), 2, "{:?}", windows.rows);
    // Two rooms (the partition), five or more walls.
    let rooms = schedule_for(&sim.app.cx, SchedKind::Room);
    assert_eq!(rooms.rows.len(), 2, "{:?}", rooms.rows);
    assert!(schedule_for(&sim.app.cx, SchedKind::Wall).rows.len() >= 5);
    let lines = materials_for(&sim.app.cx);
    assert!(lines.len() >= 5, "{} material lines", lines.len());
    assert!(lines.iter().all(|l| l.quantity > 0.0));
    assert!(materials_csv(&sim.app.cx).lines().count() >= 6);
}

#[test]
fn the_schedules_follow_the_edits_and_the_room_names() {
    let mut sim = house();
    // Name the west room through the Room Specification.
    let idx = rooms_edit::room_index_at(&sim.app.cx, Point::new(100.0, 180.0)).unwrap();
    let init = rooms_edit::room_dialog_init(&sim.app.cx, idx).unwrap();
    let mut d = crate::dialogs::room::RoomDialog::new(init);
    d.set_name("Great Room");
    let draft = d.room_name().clone();
    rooms_edit::apply_room_spec(&mut sim.app.cx, idx, &draft, d.extras());
    let rooms = schedule_for(&sim.app.cx, SchedKind::Room);
    assert!(
        rooms
            .rows
            .iter()
            .any(|r| r.iter().any(|c| c == "Great Room")),
        "{:?}",
        rooms.rows
    );
    // A third door shows up in the door schedule.
    sim.tool(ToolId::Door);
    sim.click(40.0, 0.0);
    assert_eq!(schedule_for(&sim.app.cx, SchedKind::Door).rows.len(), 2);
}

/// R-49 / manual 4.1: schedules report the interior area (to the inside wall
/// surfaces), the same number as the plan label and Room Specification.
#[test]
#[ignore = "QA-03"]
fn the_room_schedule_reports_the_interior_area_like_the_plan_label() {
    let sim = house();
    let rooms = schedule_for(&sim.app.cx, SchedKind::Room);
    for (row, room) in rooms.rows.iter().zip(&sim.app.cx.rooms) {
        let reported: f64 = row[2].parse().expect("area column");
        assert!(
            (reported - room.interior_area_sq_ft()).abs() < 0.2,
            "schedule {reported} sq ft, interior {:.1} (centerline {:.1})",
            room.interior_area_sq_ft(),
            room.area_sq_ft()
        );
    }
}
