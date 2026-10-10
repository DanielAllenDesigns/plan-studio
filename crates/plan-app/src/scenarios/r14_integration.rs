//! Round 14 integration scenarios: the Chief plan import (unsaved-changes
//! prompt, one-line status, report window), Tools > Checks > Plan Check
//! Settings and every row of the Tools menu, labelled stand-ins for symbols
//! with an unknown catalog item, roofs on a floor other than the active one,
//! Default Settings > Terrain and the 3D menu's camera steps.

use super::{draw_shell, Sim};
use crate::dialogs::{build_tools, default_settings_terrain, plan_check};
use crate::editor::roof_view::{self, RoofPlaneRecord, RoofSet};
use crate::editor::site_view;
use crate::files::Pending;
use crate::shell::view3d_panel::nudge::{Direction, Nudge};
use crate::shell::view3d_panel::{build_view_scene, View3dCommand, ViewScope};
use crate::toolbar::Action;
use crate::tools::ToolId;
use eframe::egui;
use plan_3d::Material;
use plan_chiefplan::import::{ImportReport, ImportResult};
use plan_core::geometry::Point;
use plan_core::{PlacedSymbol, Project, WallKind};
use plan_view3d::{CameraMode, Viewport3d};

fn draw_a_wall(sim: &mut Sim) {
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((100.0, 100.0), (300.0, 100.0));
    sim.tool(ToolId::Select);
}

// ----- 1. the Chief plan import -----

#[test]
fn import_chief_plan_asks_about_unsaved_changes_first() {
    super::s21_layout_print::isolate_home();
    let mut sim = Sim::new();
    draw_a_wall(&mut sim);
    sim.app.files.settle(&sim.app.cx, false);
    sim.action(Action::ImportChiefPlan);
    assert!(sim.app.files.modal_open(), "the Save / Don't Save prompt");
    // Cancel (Escape) leaves the plan alone and imports nothing.
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 700.0),
        )),
        events: vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };
    let _ = ctx.run(input.clone(), |ctx| sim.app.drive_files(ctx));
    assert!(!sim.app.files.modal_open(), "Escape cancels the prompt");
    assert_eq!(sim.app.cx.floor().walls.len(), 1);
    assert!(!plan_check::text_report_open());
}

#[test]
fn import_chief_plan_on_a_clean_plan_goes_straight_to_the_file_dialog() {
    let mut sim = Sim::new();
    sim.app.files.settle(&sim.app.cx, false);
    sim.action(Action::ImportChiefPlan);
    assert!(!sim.app.files.modal_open());
    assert!(sim.app.files.is_ready_for(&Pending::ImportChief));
}

#[test]
fn an_imported_plan_puts_one_line_on_the_status_bar_and_the_rest_in_a_window() {
    let mut sim = Sim::new();
    let mut report = ImportReport {
        file_name: "Maple.plan".into(),
        ..ImportReport::default()
    };
    report.warnings.push("dimension offsets assumed".into());
    report.warnings.push("floor names are positional".into());
    let mut project = Project::new("Maple");
    project.add_wall(
        0,
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    sim.app.apply_chief_import(ImportResult { project, report });
    let status = sim.app.cx.status.clone();
    assert!(status.starts_with("Imported Maple.plan:"), "{status}");
    assert!(!status.contains('\n'), "one line: {status}");
    assert!(!status.contains("assumed"), "warnings stay out of the bar");
    assert!(sim.app.path.is_none());
    assert_eq!(sim.app.cx.floor().walls.len(), 1);
    assert!(plan_check::text_report_open());
    // The window stays up and draws over several frames.
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(plan_check::text_report_open());
}

#[test]
fn a_file_that_cannot_be_read_says_so_and_keeps_the_plan() {
    let mut sim = Sim::new();
    draw_a_wall(&mut sim);
    let missing = std::env::temp_dir().join("plan-studio-r14-no-such-file.plan");
    sim.app.import_chief_plan_from(&missing);
    assert!(
        sim.app.cx.status.starts_with("Could not import"),
        "{}",
        sim.app.cx.status
    );
    assert_eq!(sim.app.cx.floor().walls.len(), 1);
}

// ----- 2. Tools menu -----

#[test]
fn plan_check_settings_opens_from_the_tools_menu() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    assert!(!build_tools::check_settings_open());
    sim.action(Action::Custom(plan_check::SETTINGS));
    assert!(build_tools::check_settings_open());
    // The window and the settings dialog draw.
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(build_tools::check_settings_open());
    // With the Plan Check window already up the same command reuses it.
    sim.action(Action::Custom(plan_check::SETTINGS));
    assert!(build_tools::check_settings_open());
}

/// The labels of the rows `fn tools_menu` draws, from the source.
fn tools_menu_labels() -> Vec<String> {
    let src = include_str!("../menus.rs");
    let start = src.find("fn tools_menu").expect("tools_menu");
    let end = src[start..].find("fn view_menu").expect("view_menu") + start;
    let body = &src[start..end];
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find("live(") {
        rest = &rest[i + 5..];
        let Some(q) = rest.find('"') else { break };
        // The label is the first literal after `ui,`; skip rows whose label
        // is an expression or a loop variable (the generated plan view and
        // schedule rows, the Calculators).
        let before = &rest[..q];
        if before.trim() != "ui,"
            || before.contains(')')
            || before.contains('&')
            || before.contains("::")
        {
            continue;
        }
        let lit = &rest[q + 1..];
        let Some(e) = lit.find('"') else { break };
        out.push(lit[..e].replace("\\u{2026}", "\u{2026}"));
    }
    out
}

/// One row of the Tools menu and what it sends.
fn tools_rows() -> Vec<(&'static str, Action)> {
    use crate::dialogs::{app_info, layer_sets, plan_views};
    vec![
        ("Display Options\u{2026}", Action::OpenLayerDisplay),
        (
            "Calculate Materials for All Floors",
            Action::Custom(crate::dialogs::materials_list::cmd::ALL),
        ),
        (
            "Calculate Materials From Selection",
            Action::Custom(crate::dialogs::materials_list::cmd::SELECTION),
        ),
        (
            "Calculate Materials in Room",
            Action::Custom(crate::dialogs::materials_list::cmd::ROOM),
        ),
        (
            "Materials List Polyline",
            Action::SetTool(ToolId::MaterialsPolyline),
        ),
        (
            "Materials List Polyline Defaults\u{2026}",
            Action::Custom(crate::dialogs::materials_list::cmd::POLYLINE_DEFAULTS),
        ),
        (
            "Master List",
            Action::Custom(crate::dialogs::materials_list::cmd::MASTER),
        ),
        (
            "Save Active View",
            Action::Custom(crate::dialogs::materials_list::cmd::SAVE),
        ),
        (
            "Save Active View As\u{2026}",
            Action::Custom(crate::dialogs::materials_list::cmd::SAVE_AS),
        ),
        (
            "Materials List Management\u{2026}",
            Action::Custom(crate::dialogs::materials_list::cmd::MANAGE),
        ),
        (
            "Generate a Report",
            Action::Custom(crate::dialogs::materials_list::cmd::REPORT),
        ),
        ("Open Materials List\u{2026}", Action::MaterialsList),
        (
            "Edit Active View\u{2026}",
            Action::Custom(crate::dialogs::materials_list::cmd::EDIT_VIEW),
        ),
        (
            "Update From Master List",
            Action::Custom(crate::dialogs::materials_list::cmd::UPDATE_FROM),
        ),
        (
            "Update To Master List",
            Action::Custom(crate::dialogs::materials_list::cmd::UPDATE_TO),
        ),
        (
            "Export Materials List\u{2026}",
            Action::Custom(crate::dialogs::materials_list::cmd::EXPORT),
        ),
        (
            "Print Materials List\u{2026}",
            Action::Custom(crate::dialogs::materials_list::cmd::PRINT),
        ),
        (
            "Layer Set Management\u{2026}",
            Action::Custom(layer_sets::OPEN),
        ),
        (
            "Active Layers by Tool\u{2026}",
            Action::Custom(layer_sets::ACTIVE_LAYERS),
        ),
        (
            "Layer Set Defaults\u{2026}",
            Action::Custom(layer_sets::DEFAULTS),
        ),
        (
            "Floor/Reference Display\u{2026}",
            Action::ReferenceDisplayOptions,
        ),
        (
            "Underlays\u{2026}",
            Action::Custom(crate::tools::underlay::MANAGE),
        ),
        (
            "Plan View Specification\u{2026}",
            Action::Custom(plan_views::OPEN),
        ),
        ("Save Plan View", Action::Custom(plan_views::SAVE)),
        ("Reset Plan View", Action::Custom(plan_views::RESET)),
        ("Add Template Plan Views", Action::Custom(plan_views::SEED)),
        (
            "Add Starter Plan Views",
            Action::Custom(plan_views::STARTER),
        ),
        (
            "New Saved Plan View\u{2026}",
            Action::Custom(plan_views::NEW_SAVED),
        ),
        (
            "Save Active View As\u{2026}",
            Action::Custom(plan_views::SAVE_AS),
        ),
        (
            "Active Defaults\u{2026}",
            Action::Custom(crate::dialogs::default_sets::ACTIVE_DEFAULTS),
        ),
        (
            "Default Sets\u{2026}",
            Action::Custom(crate::dialogs::default_sets::DEFAULT_SETS),
        ),
        (
            "Rotate Plan View\u{2026}",
            Action::Custom(crate::shell::view_commands::ROTATE_DIALOG),
        ),
        (
            "Reverse Plan",
            Action::Custom(crate::shell::view_commands::REVERSE_PLAN),
        ),
        ("Plan Check", Action::PlanCheck),
        (
            "Plan Check Settings\u{2026}",
            Action::Custom(plan_check::SETTINGS),
        ),
        (
            "Check While Drawing",
            Action::Custom(plan_check::CHECK_LIVE),
        ),
        (
            "Apply Code Minimums to Defaults",
            Action::Custom(plan_check::APPLY_DEFAULTS),
        ),
        ("Door/Window Check", Action::DoorWindowCheck),
        (
            "Kitchen and Bath Report",
            Action::Custom(plan_check::NKBA_REPORT),
        ),
        ("Plan Footprint", Action::PlanFootprint),
        (
            "Header/Beam\u{2026}",
            Action::Custom(crate::dialogs::calculators::HEADER),
        ),
        (
            "Joist Span\u{2026}",
            Action::Custom(crate::dialogs::calculators::JOIST),
        ),
        (
            "Rafter Span\u{2026}",
            Action::Custom(crate::dialogs::calculators::RAFTER),
        ),
        (
            "Stair\u{2026}",
            Action::Custom(crate::dialogs::calculators::STAIR),
        ),
        (
            "Deck Beam/Joist\u{2026}",
            Action::Custom(crate::dialogs::calculators::DECK),
        ),
        (
            "Customize Toolbars\u{2026}",
            Action::Custom(app_info::CUSTOMIZE_TOOLBARS),
        ),
        ("Customize Hotkeys\u{2026}", Action::OpenHotkeyDialog),
        ("Space Planning Assistant\u{2026}", Action::SpacePlanning),
        ("Door Schedule", Action::DoorSchedule),
        ("Window Schedule", Action::WindowSchedule),
        ("Room Schedule", Action::RoomSchedule),
        ("Wall Schedule", Action::WallSchedule),
        (
            "Renumber Door Schedule",
            Action::Custom(crate::editor::opening_edit::RENUMBER_DOORS),
        ),
        (
            "Renumber Window Schedule",
            Action::Custom(crate::editor::opening_edit::RENUMBER_WINDOWS),
        ),
        (
            "Manage Custom Schedule Categories\u{2026}",
            Action::Custom(crate::editor::schedule_view::cmd::MANAGE_CATEGORIES),
        ),
        (
            "Framing Takeoff\u{2026}",
            Action::Framing(crate::toolbar::FramingCommand::Takeoff),
        ),
        ("Materials List\u{2026}", Action::MaterialsList),
        (
            "Object Painter Modes\u{2026}",
            Action::Custom(crate::tools::painters::MODES),
        ),
        (
            "Spell Check\u{2026}",
            Action::Custom(crate::dialogs::spell_check::OPEN),
        ),
        (
            "Property Manager\u{2026}",
            Action::Custom(crate::dialogs::property_manager::OPEN),
        ),
        ("Project Information\u{2026}", Action::ProjectInfo),
        (
            "Color Chooser\u{2026}",
            Action::Custom(app_info::COLOR_CHOOSER),
        ),
        ("New Plan View", Action::Custom(app_info::NEW_PLAN_VIEW)),
    ]
}

/// These rows open a native file dialog, so the headless run leaves them out.
const NEEDS_NATIVE_DIALOG: &[&str] = &[
    "Kitchen and Bath Report to Excel\u{2026}",
    "Create Construction Set\u{2026}",
    "Export Property Data (XLSX)\u{2026}",
    "Import Property Data (XLSX)\u{2026}",
];

#[test]
fn every_tools_menu_row_is_tested_and_runs() {
    // Some rows change a preference that is saved under $HOME.
    super::s21_layout_print::isolate_home();
    let rows = tools_rows();
    // Every row the menu draws is in the table (or needs a native dialog).
    for label in tools_menu_labels() {
        assert!(
            rows.iter().any(|(l, _)| *l == label) || NEEDS_NATIVE_DIALOG.contains(&label.as_str()),
            "Tools menu row {label:?} has no entry in the test table"
        );
    }
    // Each row sends an action the application handles: none reaches the
    // "Not yet implemented" fallback, none panics, and the windows it opens
    // draw.
    for (label, action) in rows {
        let mut sim = Sim::new();
        draw_shell(&mut sim, 480.0, 360.0);
        sim.app.cx.status.clear();
        sim.action(action);
        sim.dialog_frame(false);
        sim.dialog_frame(false);
        assert!(
            !sim.app.cx.status.contains("Not yet implemented"),
            "{label}: {}",
            sim.app.cx.status
        );
    }
}

// ----- 5. unknown catalog items -----

#[test]
fn an_unknown_catalog_symbol_draws_a_labelled_box_in_plan() {
    let mut sim = Sim::new();
    let mut s = PlacedSymbol::new(
        "chief-plan.dining-chair",
        Point::new(200.0, 200.0),
        24.0,
        22.0,
        36.0,
    );
    s.label = String::new();
    sim.app.cx.project.add_symbol(0, s);
    sim.app.cx.refresh();
    let texts: Vec<String> = sim
        .plan_shapes()
        .into_iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|t| t == "Dining Chair"),
        "the box carries its name: {texts:?}"
    );
}

#[test]
fn an_unknown_catalog_symbol_is_a_block_in_3d() {
    let mut p = Project::new("T");
    p.add_symbol(
        0,
        PlacedSymbol::new("chief-plan.sofa", Point::new(10.0, 10.0), 80.0, 36.0, 30.0),
    );
    let scene = build_view_scene(&p, &ViewScope::default());
    assert!(
        scene
            .meshes
            .iter()
            .any(|m| m.material == Material::Trim && !m.vertices.is_empty()),
        "a block for the unknown item"
    );
}

// ----- 6. roofs on another floor -----

#[test]
fn imported_roof_planes_without_settings_draw_from_another_floor() {
    let mut p = Project::new("Two floors");
    let top = p.insert_floor_above(0).expect("second floor");
    // What the importer writes: plane records and no settings record.
    let ring = |z: f64| {
        vec![
            [0.0, z, 0.0],
            [240.0, z, 0.0],
            [240.0, z + 60.0, 144.0],
            [0.0, z + 60.0, 144.0],
        ]
    };
    let rec = RoofPlaneRecord::new(
        501,
        ring(120.0),
        6.0,
        (Point::new(0.0, 0.0), Point::new(240.0, 0.0)),
    );
    let mut set = RoofSet::default();
    set.planes.push(rec);
    roof_view::store(&mut p, top, &mut set);
    assert!(roof_view::load(&p.floors[top]).settings.is_none());
    // 3D: the roof is in the scene whichever floor is active.
    let scene = build_view_scene(&p, &ViewScope::default());
    assert!(scene.meshes.iter().any(|m| m.material == Material::Roof));
    // Plan: with the floor that holds the roof active it draws; with the
    // other floor active it does not draw (a roof belongs to its floor).
    let mut sim = Sim::new();
    sim.app.cx.set_project(p);
    sim.app.cx.floor = top;
    sim.app.cx.refresh();
    let with_roof = sim.plan_shapes().len();
    sim.app.cx.floor = 0;
    sim.app.cx.refresh();
    let without = sim.plan_shapes().len();
    assert!(with_roof > without, "{with_roof} vs {without}");
}

// ----- 7. Default Settings > Terrain, camera steps -----

#[test]
fn default_settings_tree_opens_the_terrain_page() {
    let mut sim = Sim::new();
    sim.action(Action::DefaultSettings);
    assert!(!default_settings_terrain::is_open());
    default_settings_terrain::request_open();
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(default_settings_terrain::is_open());
    sim.cancel();
    assert!(site_view::load_terrain(&sim.app.cx.project).is_none());
}

#[test]
fn the_3d_menu_camera_steps_move_the_viewport_camera() {
    let mut sim = Sim::new();
    // No 3D view yet: the command opens the overview instead.
    sim.action(Action::View3d(View3dCommand::Nudge(Nudge::OrbitLeft)));
    assert!(sim.app.view3d.active);
    // With a viewport up the step changes its camera.
    let mut vp = Viewport3d::new();
    vp.set_mode(CameraMode::Orbit);
    let yaw = vp.camera.yaw;
    sim.app.view3d.viewport = Some(vp);
    sim.action(Action::View3d(View3dCommand::Nudge(Nudge::OrbitRight)));
    let cam = &sim.app.view3d.viewport.as_ref().unwrap().camera;
    assert!(cam.yaw > yaw);
    sim.action(Action::View3d(View3dCommand::Nudge(Nudge::Look(
        Direction::Back,
    ))));
    let cam = &sim.app.view3d.viewport.as_ref().unwrap().camera;
    assert!((cam.yaw - Direction::Back.yaw()).abs() < 1e-6);
    // An elevation looks in a fixed direction and says so.
    sim.app
        .view3d
        .viewport
        .as_mut()
        .unwrap()
        .set_mode(CameraMode::ElevationFront);
    sim.action(Action::View3d(View3dCommand::Nudge(Nudge::Forward)));
    assert!(sim.app.cx.status.contains("fixed direction"));
}
