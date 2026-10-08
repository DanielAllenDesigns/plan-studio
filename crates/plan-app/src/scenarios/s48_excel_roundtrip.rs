//! Scenario 48: custom properties and the Excel round trip (beyond Chief,
//! modelled on ArchiCAD's "Exchange Property Data with Excel").
//!
//! 1. The Property Manager (Tools menu): add, change and delete a property,
//!    each one undo step.
//! 2. The Properties tab of the specification dialogs: the shared dialog
//!    frame adds it once a kind has a property; OK applies the dialog and the
//!    tab as one undo step; Cancel changes nothing.
//! 3. Custom properties as schedule columns.
//! 4. Export for Editing, edit the cells in the workbook's XML the way Excel
//!    would, import: the review dialog's checkboxes, one undo step, the count
//!    in the status bar; a deleted object and a bad value are noted, not
//!    applied; CSV works the same.
//! 5. The workbook file watch: "Workbook changed - import?" appears when the
//!    file is newer than the last import.

use super::{flatten, Sim};
use crate::dialogs::import_review::ImportReview;
use crate::dialogs::property_manager as pm;
use crate::editor::{schedule_view, ObjectRef};
use crate::toolbar::Action;
use crate::ActiveDialog;
use eframe::egui;
use plan_cabinets::Cabinet;
use plan_core::props::{PropDef, PropKey, PropKind, PropType};
use plan_core::schedules::{Schedule, ScheduleKind};
use plan_core::{OpeningKind, PlacedSymbol, Point, WallKind};
use plan_docs::props_exchange::NoteKind;
use plan_docs::xlsx_read::read_zip_entries;
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

struct Ids {
    door1: u64,
    door2: u64,
    window: u64,
    cabinet: u64,
    symbol: u64,
}

/// A 20' x 15' box with two doors and a window, a cabinet, an electrical
/// device and a toilet.
fn house() -> (Sim, Ids) {
    pm::reset();
    let mut sim = Sim::new();
    let cx = sim.cx();
    let c = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 180.0),
        Point::new(0.0, 180.0),
    ];
    let walls: Vec<u64> = (0..4)
        .map(|i| {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior)
        })
        .collect();
    let door1 = cx
        .project
        .add_opening(0, walls[0], 60.0, OpeningKind::Door)
        .unwrap();
    let door2 = cx
        .project
        .add_opening(0, walls[0], 180.0, OpeningKind::Door)
        .unwrap();
    let window = cx
        .project
        .add_opening(0, walls[2], 120.0, OpeningKind::Window)
        .unwrap();
    let mut cab = Cabinet::base(24.0);
    cab.id = cx.project.alloc_id();
    cab.position = Point::new(20.0, 20.0);
    let cabinet = cab.id;
    cx.project.floors[0].set_cabinets(&[cab]).unwrap();
    let mut layer = plan_electrical::ElectricalLayer::default();
    layer.add(plan_electrical::Device {
        id: 1,
        kind: plan_electrical::DeviceKind::all()[0],
        position: Point::new(50.0, 3.0),
        angle: 0.0,
        height: 16.0,
        wall_id: None,
        circuit: Some(3),
        label: "A".into(),
        switched_by: Vec::new(),
        finish: String::new(),
        hide_label: false,
    });
    cx.project.floors[0].electrical = Some(serde_json::to_value(&layer).unwrap());
    let symbol = cx.project.add_symbol(
        0,
        PlacedSymbol::new(
            "core.plumbing.toilet_elongated",
            Point::new(100.0, 100.0),
            20.0,
            30.0,
            30.0,
        ),
    );
    cx.mark_dirty();
    cx.refresh();
    (
        sim,
        Ids {
            door1,
            door2,
            window,
            cabinet,
            symbol,
        },
    )
}

fn rating() -> PropDef {
    let mut d = PropDef::new(PropKind::Door, "Fire Rating", PropType::List);
    d.options = vec!["None".into(), "20 min".into(), "45 min".into()];
    d.default = "None".into();
    d.show_in_schedule = true;
    d
}

fn prop_text(sim: &Sim, key: PropKey, kind: PropKind, name: &str) -> String {
    let def = sim.app.cx.project.props.def(kind, name).unwrap();
    sim.app.cx.project.props.text(&key, def)
}

/// Every text drawn by the second of two headless frames of `f` (windows
/// lay out on the first).
fn frame_texts(sim: &mut Sim, mut f: impl FnMut(&mut Sim, &egui::Context)) -> Vec<String> {
    let ctx = sim.ctx.clone();
    let mut texts = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let out = ctx.run(input, |c| f(sim, c));
        let mut leaves = Vec::new();
        for clipped in out.shapes {
            flatten(clipped.shape, &mut leaves);
        }
        texts = leaves
            .into_iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
    }
    texts
}

// ===================================================================
// Property Manager
// ===================================================================

#[test]
fn the_tools_menu_opens_the_property_manager_and_definitions_are_undoable() {
    let (mut sim, _) = house();
    sim.action(Action::Custom(pm::OPEN));
    assert!(pm::manager_open());
    // The window draws.
    let texts = frame_texts(&mut sim, |s, ctx| {
        pm::show_all(ctx, &mut s.app.cx, None);
    });
    assert!(texts.iter().any(|t| t.contains("Property Manager")), "{texts:?}");
    pm::add_property(sim.cx(), rating()).unwrap();
    assert_eq!(sim.app.cx.undo_label(), Some("Add Property"));
    assert!(pm::add_property(sim.cx(), rating()).is_err());
    assert_eq!(sim.undo().as_deref(), Some("Add Property"));
    assert!(sim.app.cx.project.props.defs.is_empty());
    sim.redo();
    assert_eq!(sim.app.cx.project.props.defs.len(), 1);
}

// ===================================================================
// The Properties tab
// ===================================================================

#[test]
fn a_door_dialog_gets_a_properties_tab_and_ok_is_one_undo_step() {
    let (mut sim, ids) = house();
    // No properties yet: no tab, no session.
    assert!(sim.open_spec(ObjectRef::Opening(ids.door1)));
    assert!(sim.app.spec.main_props().is_none());
    sim.cancel();

    pm::add_property(sim.cx(), rating()).unwrap();
    let mut note = PropDef::new(PropKind::Door, "Install Note", PropType::Text);
    note.default = String::new();
    pm::add_property(sim.cx(), note).unwrap();
    assert!(sim.open_spec(ObjectRef::Opening(ids.door1)));
    let session = sim.app.spec.main_props().cloned().expect("the tab is armed");
    assert_eq!(session.borrow().text("Fire Rating"), Some("None"), "the default shows");
    // The shared frame draws the tab.
    let texts = frame_texts(&mut sim, |s, ctx| s.app.dialogs(ctx));
    assert!(texts.iter().any(|t| t == "Properties"), "{texts:?}");
    session.borrow_mut().set_text("Fire Rating", "45 min");
    session.borrow_mut().set_text("Install Note", "Pre-hung");
    if let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() {
        d.draft_mut().width = 42.0;
    } else {
        panic!("no door dialog");
    }
    let depth = sim.app.cx.undo_depth();
    sim.ok();
    assert!(!sim.app.has_dialog());
    assert!(sim.app.spec.main_props().is_none(), "the session ends with the dialog");
    assert_eq!(prop_text(&sim, PropKey::door(ids.door1), PropKind::Door, "Fire Rating"), "45 min");
    assert_eq!(prop_text(&sim, PropKey::door(ids.door1), PropKind::Door, "Install Note"), "Pre-hung");
    assert_eq!(prop_text(&sim, PropKey::door(ids.door2), PropKind::Door, "Fire Rating"), "None");
    let o = sim.app.cx.floor().openings.iter().find(|o| o.id == ids.door1).unwrap().clone();
    assert_eq!(o.width, 42.0);
    assert_eq!(sim.app.cx.undo_depth(), depth + 1, "dialog and tab are one step");
    assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    let o = sim.app.cx.floor().openings.iter().find(|o| o.id == ids.door1).unwrap().clone();
    assert_eq!(o.width, 36.0);
    assert_eq!(prop_text(&sim, PropKey::door(ids.door1), PropKind::Door, "Fire Rating"), "None");
    sim.redo();
    assert_eq!(prop_text(&sim, PropKey::door(ids.door1), PropKind::Door, "Fire Rating"), "45 min");
}

#[test]
fn only_the_properties_tab_changed_is_a_step_named_object_properties_and_cancel_keeps_nothing() {
    let (mut sim, ids) = house();
    pm::add_property(sim.cx(), rating()).unwrap();
    assert!(sim.open_spec(ObjectRef::Opening(ids.window)));
    assert!(sim.app.spec.main_props().is_none(), "a window has no door properties");
    sim.cancel();
    assert!(sim.open_spec(ObjectRef::Opening(ids.door2)));
    sim.app.spec.main_props().unwrap().borrow_mut().set_text("Fire Rating", "20 min");
    sim.cancel();
    assert_eq!(prop_text(&sim, PropKey::door(ids.door2), PropKind::Door, "Fire Rating"), "None");
    assert!(sim.open_spec(ObjectRef::Opening(ids.door2)));
    sim.app.spec.main_props().unwrap().borrow_mut().set_text("Fire Rating", "20 min");
    sim.ok();
    assert_eq!(sim.app.cx.undo_label(), Some("Object Properties"));
    assert_eq!(prop_text(&sim, PropKey::door(ids.door2), PropKind::Door, "Fire Rating"), "20 min");
    // An invalid entry blocks OK: the dialog stays open and nothing is stored.
    let mut gauge = PropDef::new(PropKind::Door, "Gauge", PropType::Number);
    gauge.default = "16".into();
    pm::add_property(sim.cx(), gauge).unwrap();
    assert!(sim.open_spec(ObjectRef::Opening(ids.door2)));
    sim.app.spec.main_props().unwrap().borrow_mut().set_text("Gauge", "thick");
    assert!(sim.app.spec.main_props().unwrap().borrow().error().unwrap().contains("not a number"));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame(true);
    assert!(sim.app.has_dialog(), "OK is refused while a property is invalid");
    sim.cancel();
}

#[test]
fn cabinet_symbol_and_wall_dialogs_take_the_tab_through_the_shared_frame() {
    let (mut sim, ids) = house();
    for (kind, name) in [
        (PropKind::Cabinet, "Supplier PO"),
        (PropKind::Symbol, "Model No"),
        (PropKind::Wall, "Fire Wall"),
    ] {
        pm::add_property(sim.cx(), PropDef::new(kind, name, PropType::Text)).unwrap();
    }
    // Cabinet and symbol dialogs are hosted by SpecDialogs.
    assert!(sim.open_spec(ObjectRef::Cabinet(ids.cabinet)));
    sim.app.spec.props_mut().unwrap().borrow_mut().set_text("Supplier PO", "PO-1182");
    let texts = frame_texts(&mut sim, |s, ctx| s.app.dialogs(ctx));
    assert!(texts.iter().any(|t| t == "Properties"), "{texts:?}");
    sim.ok();
    assert_eq!(prop_text(&sim, PropKey::cabinet(ids.cabinet), PropKind::Cabinet, "Supplier PO"), "PO-1182");

    assert!(sim.open_spec(ObjectRef::Symbol(ids.symbol)));
    sim.app.spec.props_mut().unwrap().borrow_mut().set_text("Model No", "K-3999");
    sim.ok();
    assert_eq!(prop_text(&sim, PropKey::symbol(ids.symbol), PropKind::Symbol, "Model No"), "K-3999");

    // A wall dialog is hosted by main.rs.
    let wall = sim.app.cx.floor().walls[0].id;
    assert!(sim.open_spec(ObjectRef::Wall(wall)));
    sim.app.spec.main_props().unwrap().borrow_mut().set_text("Fire Wall", "2 hr");
    sim.ok();
    assert_eq!(prop_text(&sim, PropKey::wall(wall), PropKind::Wall, "Fire Wall"), "2 hr");
}

// ===================================================================
// Schedule columns
// ===================================================================

#[test]
fn a_custom_property_is_a_schedule_column_and_follows_the_tab() {
    let (mut sim, ids) = house();
    pm::add_property(sim.cx(), rating()).unwrap();
    let sid = schedule_view::add(sim.cx(), ScheduleKind::Door, Point::new(0.0, -60.0));
    let def = schedule_view::find(&sim.app.cx, sid).unwrap();
    let t = schedule_view::table_for(&sim.app.cx, &def, 0);
    assert_eq!(t.columns.last().unwrap(), "Fire Rating", "flagged: shown");
    assert!(t.rows.iter().all(|r| r.last().unwrap() == "None"));
    // Set it in the dialog; the table follows.
    assert!(sim.open_spec(ObjectRef::Opening(ids.door1)));
    sim.app.spec.main_props().unwrap().borrow_mut().set_text("Fire Rating", "45 min");
    sim.ok();
    let t = schedule_view::table_for(&sim.app.cx, &def, 0);
    let last: Vec<&str> = t.rows.iter().map(|r| r.last().unwrap().as_str()).collect();
    assert_eq!(last, ["45 min", "None"]);
    // The placed table is wider by the new column and sorts by it.
    let mut sorted = def.clone();
    sorted.sort.field = "prop:Fire Rating".into();
    sorted.sort.descending = true;
    let t = schedule_view::table_for(&sim.app.cx, &sorted, 0);
    assert_eq!(t.rows[0].last().unwrap(), "45 min");
    // Un-flag it: gone from a schedule that has no column for it.
    sim.cx().begin_change("Change Property");
    sim.cx().project.props.defs[0].show_in_schedule = false;
    let t = schedule_view::table_for(&sim.app.cx, &def, 0);
    assert!(!t.columns.contains(&"Fire Rating".to_string()));
}

// ===================================================================
// Excel round trip
// ===================================================================

/// Replaces the cell `at` (`C2`) of worksheet `sheet` (1-based) in the
/// workbook with `text`, the way Excel would store a typed string.
fn edit_cell(book: &[u8], sheet: usize, at: &str, text: &str) -> Vec<u8> {
    let mut parts: BTreeMap<String, Vec<u8>> = read_zip_entries(book).unwrap();
    let name = format!("xl/worksheets/sheet{sheet}.xml");
    let xml = String::from_utf8(parts[&name].clone()).unwrap();
    let start = xml.find(&format!("<c r=\"{at}\"")).unwrap_or_else(|| panic!("no cell {at}"));
    let open_end = start + xml[start..].find('>').unwrap();
    let end = if xml[..open_end].ends_with('/') {
        open_end + 1
    } else {
        open_end + xml[open_end..].find("</c>").unwrap() + 4
    };
    let cell = format!(
        "<c r=\"{at}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
        text.replace('&', "&amp;").replace('<', "&lt;")
    );
    let new = format!("{}{}{}", &xml[..start], cell, &xml[end..]);
    parts.insert(name, new.into_bytes());
    let list: Vec<(String, Vec<u8>)> = parts.into_iter().collect();
    plan_library::archive::write_zip(&list)
}

/// A door schedule over the whole plan with every column shown.
fn door_schedule() -> (usize, Schedule) {
    let mut s = Schedule::new(ScheduleKind::Door, Point::ZERO);
    s.floor_scope = plan_core::schedules::FloorScope::All;
    for c in &mut s.columns {
        c.visible = true;
    }
    (0, s)
}

fn col_letter(sim: &mut Sim, set: &[(usize, Schedule)], header: &str) -> char {
    let book = pm::build_workbook(sim.cx(), set, "");
    let sheets = plan_docs::xlsx_read::read_xlsx(&book).unwrap();
    let i = sheets[0].rows[0].iter().position(|h| h == header).unwrap();
    (b'A' + i as u8) as char
}

fn review() -> ImportReview {
    pm::take_review().expect("the review dialog is open")
}

#[test]
fn export_edit_import_applies_the_checked_changes_as_one_undo_step() {
    let (mut sim, ids) = house();
    pm::add_property(sim.cx(), rating()).unwrap();
    let set = vec![door_schedule(), (0, Schedule::new(ScheduleKind::Cabinet, Point::ZERO))];
    let book = pm::build_workbook(sim.cx(), &set, "/plans/house.plan");
    let mfr = col_letter(&mut sim, &set[..1], "Manufacturer");
    let fire = col_letter(&mut sim, &set[..1], "Fire Rating");
    let book = edit_cell(&book, 1, &format!("{mfr}2"), "Therma-Tru");
    let book = edit_cell(&book, 1, &format!("{fire}3"), "45 min");
    let label = col_letter(&mut sim, &set[1..], "Label");
    let book = edit_cell(&book, 2, &format!("{label}2"), "Sink Base");

    pm::begin_import(sim.cx(), &book, "House.xlsx", false).unwrap();
    // The review dialog draws.
    let mut r = review();
    let texts = frame_texts(&mut sim, |_, ctx| {
        let _ = r.show(ctx);
    });
    assert!(texts.iter().any(|t| t.contains("Import Property Data")), "{texts:?}");
    assert_eq!(r.plan().changes.len(), 3, "{:#?}", r.plan().changes);
    assert_eq!(r.accepted(), 3);
    // Uncheck the cabinet label: it stays as it is.
    let cab = r.plan().changes.iter().position(|c| c.field == "label").unwrap();
    r.set_accept(cab, false);
    let depth = sim.app.cx.undo_depth();
    let report = r.apply(sim.cx());
    assert_eq!(report.applied, 2, "{:?}", report.failed);
    assert_eq!(sim.app.cx.undo_depth(), depth + 1, "one undo step");
    assert_eq!(sim.app.cx.undo_label(), Some("Import Property Data"));
    assert_eq!(sim.app.cx.status, "Imported 2 changes from House.xlsx");
    let o = sim.app.cx.floor().openings.iter().find(|o| o.id == ids.door1).unwrap().clone();
    assert_eq!(o.extras.spec.schedule.manufacturer, "Therma-Tru");
    assert_eq!(prop_text(&sim, PropKey::door(ids.door2), PropKind::Door, "Fire Rating"), "45 min");
    assert_eq!(sim.app.cx.floor().cabinets[0]["label"], "");
    // Undo puts all of it back; redo does it again.
    assert_eq!(sim.undo().as_deref(), Some("Import Property Data"));
    let o = sim.app.cx.floor().openings.iter().find(|o| o.id == ids.door1).unwrap().clone();
    assert_eq!(o.extras.spec.schedule.manufacturer, "");
    assert_eq!(prop_text(&sim, PropKey::door(ids.door2), PropKind::Door, "Fire Rating"), "None");
    sim.redo();
    assert_eq!(prop_text(&sim, PropKey::door(ids.door2), PropKind::Door, "Fire Rating"), "45 min");
    // Nothing checked: no step is left behind.
    pm::begin_import(sim.cx(), &edit_cell(&book, 1, &format!("{mfr}3"), "Other"), "House.xlsx", false).unwrap();
    let mut r = review();
    r.select_all(false);
    let depth = sim.app.cx.undo_depth();
    assert_eq!(r.apply(sim.cx()).applied, 0);
    assert_eq!(sim.app.cx.undo_depth(), depth);
}

#[test]
fn an_untouched_workbook_has_nothing_to_import() {
    let (mut sim, _) = house();
    pm::add_property(sim.cx(), rating()).unwrap();
    let set = pm::export_set(sim.cx());
    assert_eq!(set.len(), 8, "no schedules placed: the standard set");
    let book = pm::build_workbook(sim.cx(), &set, "");
    pm::begin_import(sim.cx(), &book, "All.xlsx", false).unwrap();
    let r = review();
    assert!(r.plan().changes.is_empty(), "{:#?}", r.plan().changes);
    assert!(r.plan().notes.is_empty(), "{:#?}", r.plan().notes);
    assert!(r.plan().rows_read >= 6);
}

#[test]
fn deleted_objects_and_bad_values_are_noted_and_never_applied() {
    let (mut sim, ids) = house();
    pm::add_property(sim.cx(), rating()).unwrap();
    let set = vec![door_schedule()];
    let book = pm::build_workbook(sim.cx(), &set, "");
    let fire = col_letter(&mut sim, &set, "Fire Rating");
    let shgc = col_letter(&mut sim, &set, "SHGC");
    let mfr = col_letter(&mut sim, &set, "Manufacturer");
    let width = col_letter(&mut sim, &set, "Width");
    let book = edit_cell(&book, 1, &format!("{fire}2"), "90 min");
    let book = edit_cell(&book, 1, &format!("{shgc}2"), "2.5");
    let book = edit_cell(&book, 1, &format!("{width}2"), "9'-0\"");
    let book = edit_cell(&book, 1, &format!("{mfr}3"), "Kept");
    // The second door is deleted in the plan before the import.
    sim.cx().begin_change("Delete Door");
    sim.cx().project.remove_opening(0, ids.door2);
    sim.cx().mark_dirty();
    pm::begin_import(sim.cx(), &book, "House.xlsx", false).unwrap();
    let mut r = review();
    let plan = r.plan().clone();
    assert_eq!(plan.changes.len(), 2, "{:#?}", plan.changes);
    assert!(plan.changes.iter().all(|c| c.problem.is_some()));
    assert_eq!(r.accepted(), 0, "refused values start unchecked and stay so");
    r.set_accept(0, true);
    assert_eq!(r.accepted(), 0);
    let kinds: Vec<NoteKind> = plan.notes.iter().map(|n| n.kind).collect();
    assert!(kinds.contains(&NoteKind::ObjectDeleted), "{:#?}", plan.notes);
    assert!(kinds.contains(&NoteKind::Computed), "{:#?}", plan.notes);
    assert_eq!(r.apply(sim.cx()).applied, 0);
    let o = sim.app.cx.floor().openings.iter().find(|o| o.id == ids.door1).unwrap().clone();
    assert_eq!(o.width, 36.0, "the computed Width edit was ignored");
}

#[test]
fn csv_edited_in_a_text_editor_imports_the_same_way() {
    let (mut sim, ids) = house();
    pm::add_property(sim.cx(), rating()).unwrap();
    let (floor, def) = door_schedule();
    sim.app.cx.refresh();
    let csv = plan_docs::props_exchange::export_csv(
        &sim.app.cx.project,
        &plan_docs::props_exchange::ExportSchedule { def: &def, home_floor: floor },
        None,
        true,
    );
    assert!(csv.starts_with("PlanStudio ID,Mark,"), "{csv}");
    let mut rows = plan_docs::xlsx_read::read_csv(&csv);
    let mfr = rows[0].iter().position(|h| h == "Manufacturer").unwrap();
    rows[1][mfr] = "CSV, Inc.".into();
    let edited: String = rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| {
                    if c.contains([',', '"']) {
                        format!("\"{}\"", c.replace('"', "\"\""))
                    } else {
                        c.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    pm::begin_import(sim.cx(), edited.as_bytes(), "doors.csv", true).unwrap();
    let r = review();
    assert_eq!(r.plan().changes.len(), 1, "{:#?}", r.plan().changes);
    assert_eq!(r.apply(sim.cx()).applied, 1);
    let o = sim.app.cx.floor().openings.iter().find(|o| o.id == ids.door1).unwrap().clone();
    assert_eq!(o.extras.spec.schedule.manufacturer, "CSV, Inc.");
    assert!(pm::begin_import(sim.cx(), b"Mark,Width\nD01,3'-0\"\n", "bad.csv", true).is_err());
}

#[test]
fn a_new_workbook_can_be_written_to_a_file_and_read_back_through_the_file_path() {
    let (mut sim, ids) = house();
    pm::add_property(sim.cx(), rating()).unwrap();
    let dir = std::env::temp_dir().join(format!("ps_s48_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Doors.xlsx");
    let set = vec![door_schedule()];
    let status = pm::export_to_path(sim.cx(), &set, &path, None);
    assert!(status.starts_with("Saved "), "{status}");
    // Edit the file on disk, then import it from there.
    let book = std::fs::read(&path).unwrap();
    let mfr = col_letter(&mut sim, &set, "Manufacturer");
    std::fs::write(&path, edit_cell(&book, 1, &format!("{mfr}2"), "On Disk Co")).unwrap();
    pm::import_path(sim.cx(), &path);
    let r = review();
    assert_eq!(r.plan().changes.len(), 1);
    assert_eq!(r.apply(sim.cx()).applied, 1);
    let o = sim.app.cx.floor().openings.iter().find(|o| o.id == ids.door1).unwrap().clone();
    assert_eq!(o.extras.spec.schedule.manufacturer, "On Disk Co");
    // A missing file is reported, not a panic.
    pm::import_path(sim.cx(), &dir.join("nope.xlsx"));
    assert!(sim.app.cx.status.starts_with("Could not read"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ===================================================================
// The file watch
// ===================================================================

#[test]
fn the_status_bar_offers_a_workbook_that_changed_after_the_export() {
    let (mut sim, _) = house();
    let dir = std::env::temp_dir().join(format!("ps_s48w_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Watched.xlsx");
    std::fs::write(&path, b"x").unwrap();
    let exported = SystemTime::now() - Duration::from_secs(3600);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(exported).unwrap();
    drop(file);
    pm::arm_watch(&path, None, Some(exported));
    // Unchanged since the export: nothing is offered.
    let _ = frame_texts(&mut sim, |s, ctx| pm::show_all(ctx, &mut s.app.cx, None));
    assert!(pm::offered().is_none());
    // Saved again in Excel: the next poll offers it, with its buttons.
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(SystemTime::now()).unwrap();
    drop(file);
    pm::arm_watch(&path, None, Some(exported));
    let texts = frame_texts(&mut sim, |s, ctx| pm::show_all(ctx, &mut s.app.cx, None));
    assert!(pm::offered().is_some());
    assert!(
        texts.iter().any(|t| t.contains("Workbook changed")),
        "{texts:?}"
    );
    // Another plan being open ends the watch.
    let _ = frame_texts(&mut sim, |s, ctx| {
        pm::show_all(ctx, &mut s.app.cx, Some(std::path::Path::new("/other.plan")))
    });
    assert!(pm::offered().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn schedule_context_menu_and_dialog_buttons_reach_the_exchange() {
    let (mut sim, _) = house();
    let sid = schedule_view::add(sim.cx(), ScheduleKind::Door, Point::new(0.0, -60.0));
    schedule_view::select(sim.cx(), sid);
    let entries = sim.app.cx.context_entries(&[], false);
    let labels: Vec<&str> = entries.iter().map(|e| e.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.starts_with("Export for Editing")), "{labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("Import Property Data")), "{labels:?}");
    // The entries run as ordinary commands.
    let e = entries.iter().find(|e| e.label.starts_with("Import")).unwrap();
    assert_eq!(e.action, Action::Custom(pm::IMPORT));
    // Nothing else selected: the entries are not offered.
    sim.cx().selection.clear();
    let entries = sim.app.cx.context_entries(&[], false);
    assert!(!entries.iter().any(|e| e.label.starts_with("Export for Editing")));
}
