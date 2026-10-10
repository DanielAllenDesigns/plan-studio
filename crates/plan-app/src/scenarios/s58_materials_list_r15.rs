//! Scenario 58: the Materials List, round 15. The Tools > Materials List
//! commands (all floors, selection, room, Materials List Polyline), the
//! 21 columns and their cells, saved lists and Reports in the Project
//! Browser and the Management dialog, Find Object, the exports, and the
//! Components and Object Information panels of the object dialogs.

use super::{draw_shell, Sim};
use crate::dialogs::materials_list as ml;
use crate::dialogs::object_info::{Field, InfoSession};
use crate::editor::{EditorContext, ObjectRef};
use crate::shell::docks::{browser_nodes, BrowserItem, BrowserNode};
use crate::shell::layout_window::use_memory_master_list;
use crate::toolbar::Action;
use crate::tools::ToolId;
use plan_core::materials_data::{ListKind, ListScope, MlColumn};
use plan_core::Id;
use plan_docs::materials::export::{ExportFormat, ExportOptions, ThirdParty};
use plan_docs::materials::list::{self, UnitsMode};
use plan_docs::MasterList;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    // Never read or write the user's Master List file.
    use_memory_master_list(MasterList::without_waste());
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim.app.cx.refresh();
    sim
}

fn frames(sim: &mut Sim) {
    sim.dialog_frame(false);
    sim.dialog_frame(false);
}

fn command(sim: &mut Sim, id: &'static str) {
    sim.action(Action::Custom(id));
    frames(sim);
}

fn shown() -> Vec<list::ListLine> {
    ml::with_state(|st| st.shown.clone())
}

fn item<'a>(lines: &'a [list::ListLine], text: &str) -> &'a list::ListLine {
    lines
        .iter()
        .find(|l| l.line.item.starts_with(text))
        .unwrap_or_else(|| panic!("no row {text}"))
}

fn walls(sim: &Sim) -> Vec<Id> {
    sim.wall_ids()
}

#[test]
fn the_menu_opens_a_list_of_all_floors_with_the_default_columns() {
    let mut sim = house();
    command(&mut sim, ml::cmd::ALL);
    let (open, scope, kind) = ml::with_state(|st| (st.open, st.spec.scope.clone(), st.spec.kind));
    assert!(open);
    assert_eq!(scope, ListScope::AllFloors);
    assert_eq!(kind, ListKind::Live);
    let lines = shown();
    assert!(lines.iter().any(|l| l.line.category == "Framing"));
    assert!(lines.iter().any(|l| l.line.category == "Doors"));
    assert!(lines.iter().any(|l| l.line.category == "Windows"));
    // The Materials List window draws its 21-column set: six shown at first.
    let cols = ml::with_state(|st| st.spec.visible_columns().len());
    assert_eq!(cols, 6);
    assert_eq!(ml::with_state(|st| st.spec.columns.len()), 21);
    // Every row remembers its objects.
    assert!(lines
        .iter()
        .filter(|l| l.line.category == "Windows")
        .all(|l| !l.sources.is_empty()));
}

#[test]
fn calculate_from_selection_counts_only_the_selected_objects() {
    let mut sim = house();
    let w = walls(&sim)[0];
    sim.app.cx.selection.set(ObjectRef::Wall(w));
    // The Edit toolbar offers the button.
    let labels: Vec<&str> = sim
        .app
        .cx
        .common_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect();
    assert!(
        labels.contains(&"Calculate Materials From Selection"),
        "{labels:?}"
    );
    command(&mut sim, ml::cmd::SELECTION);
    let (scope, name) = ml::with_state(|st| (st.spec.scope.clone(), st.spec.name.clone()));
    assert_eq!(name, "Selection");
    match scope {
        ListScope::Selection(a) => {
            assert_eq!(a.len(), 1);
            assert_eq!(a[0].key, format!("wall:{w}"));
        }
        other => panic!("{other:?}"),
    }
    let lines = shown();
    assert!(lines.iter().any(|l| l.line.item.contains("Stud")));
    assert!(!lines
        .iter()
        .any(|l| l.line.category == "Windows" || l.line.category == "Doors"));
    // With nothing selected the command says so and keeps the old list.
    sim.app.cx.selection.clear();
    sim.action(Action::Custom(ml::cmd::SELECTION));
    assert!(
        sim.app.cx.status.contains("Select"),
        "{}",
        sim.app.cx.status
    );
}

#[test]
fn calculate_in_room_has_the_rooms_finishes_and_its_openings() {
    let mut sim = house();
    sim.app.cx.refresh();
    assert!(!sim.app.cx.rooms.is_empty(), "the shell closes a room");
    crate::editor::rooms_edit::select_room(&mut sim.app.cx, 0);
    // The room's own edit toolbar button.
    let labels: Vec<&str> = crate::dialogs::materials_list::edit_buttons(&sim.app.cx)
        .iter()
        .map(|a| a.label)
        .collect();
    assert!(
        labels.contains(&"Calculate Materials in Room"),
        "{labels:?}"
    );
    command(&mut sim, ml::cmd::ROOM);
    let lines = shown();
    assert!(lines.iter().any(|l| l.line.item.starts_with("Flooring")));
    assert!(lines
        .iter()
        .any(|l| l.line.item.starts_with("Ceiling drywall")));
    assert!(lines
        .iter()
        .any(|l| l.line.item == "Wall drywall 1/2\" 4x8 sheet"));
    assert!(lines.iter().any(|l| l.line.category == "Windows"));
    // The walls around the room are not counted.
    assert!(!lines.iter().any(|l| l.line.category == "Framing"));
}

#[test]
fn the_polyline_tool_draws_an_area_and_calculates_from_it() {
    let mut sim = house();
    sim.tool(ToolId::MaterialsPolyline);
    // Drag a rectangle over the left half of the house.
    sim.drag((-30.0, -30.0), (250.0, 400.0));
    let n = sim.app.cx.project.materials.polylines.len();
    assert_eq!(n, 1);
    let cad = sim.app.cx.project.materials.polylines[0].cad_id;
    assert!(sim.app.cx.selection.items.contains(&ObjectRef::Cad(cad)));
    // Back in Select, the polyline offers Calculate Materials List.
    let labels: Vec<&str> = sim
        .app
        .cx
        .common_edit_actions()
        .iter()
        .map(|a| a.label)
        .collect();
    assert!(labels.contains(&"Calculate Materials List"), "{labels:?}");
    command(&mut sim, ml::cmd::POLYLINE_CALC);
    assert_eq!(
        ml::with_state(|st| st.spec.scope.clone()),
        ListScope::Polyline(cad)
    );
    let lines = shown();
    // By centre: the left wall and the two long walls (their middles are inside).
    let studs = item(&lines, "2x6 Stud");
    assert_eq!(studs.sources.len(), 3);
    // The right wall is outside; so is the window at x = 300 (centre outside).
    assert!(!lines.iter().any(|l| l.line.category == "Windows"));
    // One undo step removes the polyline and its record.
    assert_eq!(sim.undo().as_deref(), Some("Materials List Polyline"));
    assert!(sim.app.cx.project.materials.polylines.is_empty());
}

#[test]
fn opening_the_polyline_gives_the_included_floors_and_categories_grid() {
    let mut sim = house();
    sim.tool(ToolId::MaterialsPolyline);
    sim.drag((-30.0, -30.0), (600.0, 400.0));
    let cad = sim.app.cx.project.materials.polylines[0].cad_id;
    sim.tool(ToolId::Select);
    // The polyline's own dialog is not a spec dialog of the shell.
    sim.open_spec(ObjectRef::Cad(cad));
    frames(&mut sim);
    assert!(ml::with_state(|st| st.extras.polyline.is_some()));
    // Switch Windows off on floor 1 in the record (what the grid's check box does).
    sim.app
        .cx
        .project
        .materials
        .polyline_mut(cad)
        .unwrap()
        .spec
        .toggle("Windows", 0);
    command(&mut sim, ml::cmd::POLYLINE_CALC);
    let lines = shown();
    assert!(!lines.iter().any(|l| l.line.category == "Windows"));
    assert!(lines.iter().any(|l| l.line.category == "Doors"));
}

#[test]
fn a_saved_list_follows_the_plan_when_opened_and_survives_a_file_round_trip() {
    let mut sim = house();
    command(&mut sim, ml::cmd::ALL);
    let before = shown()
        .iter()
        .filter(|l| l.line.category == "Windows")
        .count();
    let name = {
        let cx = &mut sim.app.cx;
        ml::with_state(|st| ml::save_list(cx, st, Some("Takeoff")))
    };
    assert_eq!(name, "Takeoff");
    // Saving is one undo step.
    assert_eq!(sim.undo().as_deref(), Some("Save Materials List"));
    assert!(sim.app.cx.project.materials.list("Takeoff").is_none());
    sim.redo();
    assert!(sim.app.cx.project.materials.list("Takeoff").is_some());
    // It is a row of the Project Browser.
    let rows = browser_nodes(sim.cx())
        .into_iter()
        .find(|(n, _)| *n == BrowserNode::MaterialsLists)
        .unwrap()
        .1;
    assert!(rows
        .iter()
        .any(|e| e.item == BrowserItem::MaterialsList("Takeoff".into())));
    // Add a second window, then open the saved list: it is calculated again.
    sim.tool(ToolId::Window);
    sim.click(60.0, 0.0);
    sim.tool(ToolId::Select);
    assert!(ml::open_from_browser(&sim.app.cx, "Takeoff"));
    frames(&mut sim);
    let after = shown()
        .iter()
        .filter(|l| l.line.category == "Windows")
        .map(|l| l.line.quantity as usize)
        .sum::<usize>();
    assert!(after >= 2, "{before} -> {after}");
    // The file keeps the list (and an old file without the slot still loads).
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert!(back.materials.list("Takeoff").is_some());
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value.as_object_mut().unwrap().remove("materials");
    let old: plan_core::Project = serde_json::from_value(value).unwrap();
    assert!(old.materials.is_empty());
}

#[test]
fn management_copies_renames_and_deletes_with_undo() {
    let mut sim = house();
    command(&mut sim, ml::cmd::ALL);
    {
        let cx = &mut sim.app.cx;
        ml::with_state(|st| ml::save_list(cx, st, Some("A")));
    }
    let cx = &mut sim.app.cx;
    assert_eq!(ml::copy_saved(cx, "A", "A").as_deref(), Some("A 2"));
    assert!(ml::rename_saved(cx, "A 2", "Kitchen"));
    assert!(!ml::rename_saved(cx, "Kitchen", "A"), "the name is taken");
    assert_eq!(cx.project.materials.lists.len(), 2);
    assert!(ml::delete_saved(cx, "Kitchen"));
    assert_eq!(cx.project.materials.lists.len(), 1);
    assert_eq!(cx.undo().as_deref(), Some("Delete Materials List"));
    assert_eq!(cx.project.materials.lists.len(), 2);
    // The Management window draws.
    ml::run_command(&mut sim.app.cx, ml::cmd::MANAGE);
    frames(&mut sim);
}

#[test]
fn a_report_is_frozen_editable_and_saved_with_its_rows() {
    let mut sim = house();
    command(&mut sim, ml::cmd::ALL);
    command(&mut sim, ml::cmd::REPORT);
    let (kind, rows) = ml::with_state(|st| (st.spec.kind, st.rows.len()));
    assert_eq!(kind, ListKind::Report);
    assert!(rows > 3);
    frames(&mut sim);
    // The plan changes; the report does not.
    let n_before = shown().len();
    sim.tool(ToolId::Window);
    sim.click(60.0, 0.0);
    sim.tool(ToolId::Select);
    frames(&mut sim);
    assert_eq!(shown().len(), n_before);
    // Type a price into a row of the report.
    let lines = shown();
    let i = lines
        .iter()
        .position(|l| l.line.category == "Doors")
        .unwrap();
    {
        let cx = &mut sim.app.cx;
        ml::with_state(|st| {
            let e = ml::CellEdit {
                line: i,
                col: MlColumn::Price,
                text: "$250".into(),
            };
            ml::apply_edit(cx, st, &lines, &e);
        });
    }
    frames(&mut sim);
    assert_eq!(shown()[i].line.unit_price, Some(250.0));
    // Save it; the rows go into the plan.
    let cx = &mut sim.app.cx;
    ml::with_state(|st| ml::save_list(cx, st, Some("Frozen")));
    let saved = sim.app.cx.project.materials.list("Frozen").unwrap();
    assert_eq!(saved.spec.kind, ListKind::Report);
    assert_eq!(saved.rows[i].price, Some(250.0));
}

#[test]
fn typing_in_a_cell_of_a_live_list_writes_the_object_and_is_one_undo_step() {
    let mut sim = house();
    command(&mut sim, ml::cmd::ALL);
    let lines = shown();
    let i = lines
        .iter()
        .position(|l| l.line.category == "Windows")
        .unwrap();
    {
        let cx = &mut sim.app.cx;
        ml::with_state(|st| {
            let e = ml::CellEdit {
                line: i,
                col: MlColumn::Supplier,
                text: "Pella".into(),
            };
            ml::apply_edit(cx, st, &lines, &e);
        });
    }
    frames(&mut sim);
    assert_eq!(shown()[i].supplier, "Pella");
    assert!(sim
        .app
        .cx
        .project
        .materials
        .objects
        .keys()
        .any(|k| k.starts_with("window:")));
    assert_eq!(sim.undo().as_deref(), Some("Materials List Edit"));
    frames(&mut sim);
    assert_eq!(shown()[i].supplier, "");
}

#[test]
fn find_object_goes_to_the_floor_selects_and_centres_the_view() {
    let mut sim = house();
    command(&mut sim, ml::cmd::ALL);
    let w = walls(&sim)[1];
    let mut cam = sim.app.camera;
    cam.center = plan_core::Point::new(-5000.0, -5000.0);
    let ok = ml::find_object(&mut sim.app.cx, Some(&mut cam), 0, &format!("wall:{w}"));
    assert!(ok);
    assert!(sim.app.cx.selection.items.contains(&ObjectRef::Wall(w)));
    assert!(cam.center.x > -1000.0, "the view moved to the wall");
    assert!(!ml::find_object(
        &mut sim.app.cx,
        Some(&mut cam),
        0,
        "wall:99999"
    ));
    // A row's first object is what a double-click finds.
    let lines = shown();
    let studs = item(&lines, "2x6 Stud");
    assert!(!list::find_targets(studs).is_empty());
    // A device key and a room key map to their objects.
    assert!(matches!(
        ml::object_of_key(&sim.app.cx, 0, "door:7"),
        Some(ObjectRef::Opening(7))
    ));
    sim.app.cx.refresh();
    let key = list::describe_key("room:0:1,1");
    assert_eq!(key, "Room 1,1");
}

#[test]
fn exports_and_print_work_from_the_window() {
    let mut sim = house();
    command(&mut sim, ml::cmd::ALL);
    let cx = &sim.app.cx;
    for f in ExportFormat::ALL {
        let opts = ExportOptions {
            format: f,
            ..ExportOptions::default()
        };
        let bytes = ml::with_state(|st| ml::export_bytes(cx, st, &opts));
        assert!(bytes.len() > 100, "{f:?}");
    }
    let bt = ExportOptions {
        third_party: ThirdParty::BuilderTrend,
        units: UnitsMode::None,
        ..ExportOptions::default()
    };
    let text = String::from_utf8(ml::with_state(|st| ml::export_bytes(cx, st, &bt))).unwrap();
    assert!(text.starts_with("Cost Code,"));
    let msg = ml::with_state(|st| ml::print_list(cx, st, crate::dialogs::print::Destination::Pdf));
    assert_eq!(msg, "Print skipped in tests");
    // The other commands run without a window to spare.
    for id in [
        ml::cmd::EDIT_VIEW,
        ml::cmd::EXPORT,
        ml::cmd::POLYLINE_DEFAULTS,
        ml::cmd::MASTER,
    ] {
        command(&mut sim, id);
    }
    assert!(ml::with_state(|st| st.spec_dialog.is_some()));
}

#[test]
fn update_to_master_list_keeps_a_price_for_the_next_plan() {
    let mut sim = house();
    command(&mut sim, ml::cmd::ALL);
    let lines = shown();
    let i = lines
        .iter()
        .position(|l| l.line.category == "Doors")
        .unwrap();
    {
        let cx = &mut sim.app.cx;
        ml::with_state(|st| {
            let e = ml::CellEdit {
                line: i,
                col: MlColumn::Price,
                text: "180".into(),
            };
            ml::apply_edit(cx, st, &lines, &e);
            st.selected.insert(i);
        });
    }
    frames(&mut sim);
    let cx = &mut sim.app.cx;
    ml::with_state(|st| ml::update_to_master(cx, st));
    let master = crate::shell::layout_window::load_master_list();
    assert!(master
        .items
        .iter()
        .any(|m| m.category == "Doors" && m.unit_price == 180.0));
    // A fresh plan with the same door is priced from the Master List.
    let mut other = Sim::new();
    draw_shell(&mut other, W, H);
    other.tool(ToolId::Door);
    other.click(120.0, 0.0);
    other.tool(ToolId::Select);
    command(&mut other, ml::cmd::ALL);
    let priced = shown();
    let door = priced.iter().find(|l| l.line.category == "Doors").unwrap();
    assert_eq!(door.line.unit_price, Some(180.0));
}

// ---------------------------------------------- Components and Object Information --

fn first_wall_session(cx: &EditorContext) -> InfoSession {
    let w = cx.floor().walls[0].id;
    InfoSession::new(cx, 0, format!("wall:{w}"))
}

#[test]
fn the_components_panel_lists_an_objects_line_items_and_overrides_them() {
    let sim = house();
    let cx = &sim.app.cx;
    let mut s = first_wall_session(cx);
    let rows = s.rows();
    assert!(rows.iter().any(|r| r.name.contains("Stud")), "{rows:?}");
    let stud = rows.iter().position(|r| r.name.contains("Stud")).unwrap();
    let base = rows[stud].count;
    // Price and markup give a Total Cost by the manual's formula.
    s.set_number(stud, Field::Price, Some(4.0));
    s.set_number(stud, Field::Markup, Some(25.0));
    s.set_number(stud, Field::Extra, Some(2.0));
    let r = &s.rows()[stud];
    let want = (base + 2.0) * 4.0 * 1.25;
    assert!((r.total().unwrap() - want).abs() < 1e-9);
    // Remove, Restore, Revert.
    s.remove_row(stud);
    assert!(s.rows()[stud].removed);
    s.restore();
    assert!(!s.rows()[stud].removed);
    s.add_row();
    assert_eq!(s.rows().len(), rows.len() + 1);
    s.revert();
    assert_eq!(s.rows().len(), rows.len());
    assert!(!s.changed());
}

#[test]
fn the_panels_open_with_the_wall_dialog_and_ok_is_one_undo_step() {
    let mut sim = house();
    let w = walls(&sim)[0];
    sim.app.cx.selection.set(ObjectRef::Wall(w));
    assert!(sim.open_spec(ObjectRef::Wall(w)));
    sim.dialog_frame(false);
    let info = sim.app.spec.main_info().cloned().expect("tabs armed");
    {
        let mut s = info.borrow_mut();
        s.draft.supplier = "Acme Lumber".into();
        s.draft.code = "STUD-1".into();
        s.draft.comment = "Delivery Monday".into();
    }
    let steps = sim.app.cx.undo_depth();
    sim.ok();
    assert!(!sim.app.has_dialog());
    let key = format!("wall:{w}");
    let stored = sim.app.cx.project.materials.info(&key).expect("stored");
    assert_eq!(stored.supplier, "Acme Lumber");
    assert_eq!(stored.code, "STUD-1");
    assert_eq!(sim.app.cx.undo_depth(), steps + 1);
    // The list shows it.
    command(&mut sim, ml::cmd::ALL);
    let lines = shown();
    let studs = lines
        .iter()
        .find(|l| l.supplier == "Acme Lumber")
        .expect("a row of its own");
    assert_eq!(studs.code, "STUD-1");
    assert_eq!(studs.comment, "Delivery Monday");
    // One undo takes the whole OK back.
    sim.undo();
    assert!(sim.app.cx.project.materials.info(&key).is_none());
}

#[test]
fn cabinets_symbols_and_electrical_get_the_panels_too() {
    let sim = house();
    let cx = &sim.app.cx;
    for o in [
        ObjectRef::Cabinet(5),
        ObjectRef::Device(3),
        ObjectRef::Stair(2),
        ObjectRef::Foundation(2),
        ObjectRef::RoofPlane(4),
    ] {
        assert!(InfoSession::for_object(cx, o).is_some(), "{o:?}");
    }
    assert_eq!(
        crate::dialogs::object_info::object_key(cx, ObjectRef::Device(3)).as_deref(),
        Some("device:0:3")
    );
    // Dimensions and CAD make no materials: no panels.
    assert!(InfoSession::for_object(cx, ObjectRef::Dimension(1)).is_none());
}

#[test]
fn an_added_line_item_reaches_the_list_and_macros_expand() {
    let mut sim = house();
    let w = walls(&sim)[0];
    let mut s = first_wall_session(&sim.app.cx);
    s.add_row();
    let last = s.rows().len() - 1;
    s.draft.added[0].description = "Corner bracket %floor%".into();
    s.set_number(last, Field::Count, Some(8.0));
    s.set_number(last, Field::Price, Some(2.5));
    assert!(s.apply(&mut sim.app.cx.project));
    command(&mut sim, ml::cmd::ALL);
    let lines = shown();
    let bracket = item(&lines, "Corner bracket");
    assert_eq!(bracket.line.item, "Corner bracket 1st Floor");
    assert_eq!(bracket.line.quantity, 8.0);
    assert_eq!(bracket.line.price, Some(20.0));
    let _ = w;
}
