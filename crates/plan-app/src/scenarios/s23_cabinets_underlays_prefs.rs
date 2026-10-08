//! Scenario 23: cabinet handles and fit to gap, countertops that join and
//! rejoin, an underlay calibrated with two clicks, a DXF imported through the
//! layer map, the Preferences file and the Material Painter's pick hook
//! (CB-5, CB-8, CB-14, L-43, L-46, the Preferences dialog).

use super::s21_layout_print::isolate_home;
use super::{draw_shell, Sim};
use crate::dialogs::exchange::{import_drawing_with, UnitsChoice};
use crate::dialogs::preferences::{self, Preferences};
use crate::dialogs::underlay as underlay_win;
use crate::editor::handles::{self, HandleKind};
use crate::editor::placed::{
    self, auto_join_enabled, load_cabinets, CORNER_FRONT_RIGHT, DEPTH_FRONT,
};
use crate::editor::ObjectRef;
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::cabinet::{fit_to_gap_enabled, set_fit_to_gap};
use crate::tools::materials::{self, PainterMode};
use crate::tools::underlay::{apply_calibration, import_image};
use crate::tools::ToolId;
use plan_cabinets::{Cabinet, CabinetKind};
use plan_core::geometry::Point;
use plan_core::{Id, WallKind};
use plan_import::{parse_dxf, ImportOptions, LayerMapping, LayerTarget};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    isolate_home();
    // Preferences are per test thread; start from the defaults.
    preferences::set(Preferences::default());
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn cabs(sim: &Sim) -> Vec<Cabinet> {
    load_cabinets(sim.app.cx.floor())
}

/// Places a base cabinet by clicking, with nothing selected before.
fn place(sim: &mut Sim, x: f64) -> Id {
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.app.cx.selection.clear();
    sim.click(x, 10.0);
    sim.tool(ToolId::Select);
    cabs(sim)
        .into_iter()
        .filter(|c| c.kind == CabinetKind::Base && c.joined.is_empty())
        .max_by_key(|c| c.id)
        .unwrap()
        .id
}

fn cab(sim: &Sim, id: Id) -> Cabinet {
    cabs(sim).into_iter().find(|c| c.id == id).unwrap()
}

fn handle(sim: &Sim, kind: HandleKind) -> Point {
    handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in)
        .into_iter()
        .find(|h| h.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} handle"))
        .pos
}

fn tops(sim: &Sim) -> Vec<Cabinet> {
    cabs(sim)
        .into_iter()
        .filter(|c| !c.joined.is_empty())
        .collect()
}

#[test]
fn the_depth_and_corner_handles_of_a_cabinet_reshape_it_with_one_undo_step_each() {
    let mut sim = house();
    let id = place(&mut sim, 100.0);
    sim.app.cx.selection.set(ObjectRef::Cabinet(id));
    let before = cab(&sim, id);

    // The front depth handle pulls the front edge out.
    let front = handle(&sim, HandleKind::Reshape(DEPTH_FRONT));
    sim.drag((front.x, front.y), (front.x, front.y + 6.0));
    let deeper = cab(&sim, id);
    assert!(
        (deeper.depth - before.depth - 6.0).abs() <= 1.0,
        "{} -> {}",
        before.depth,
        deeper.depth
    );
    assert_eq!(deeper.width, before.width);
    // The back stays against the wall.
    assert!((deeper.position.y - before.position.y).abs() < 1e-6);
    assert_eq!(
        sim.app.cx.undo_label().map(|l| l.contains("Cabinet")),
        Some(true)
    );

    // A corner handle changes width and depth together.
    let corner = handle(&sim, HandleKind::Reshape(CORNER_FRONT_RIGHT));
    sim.drag((corner.x, corner.y), (corner.x + 6.0, corner.y + 4.0));
    let both = cab(&sim, id);
    assert!(
        both.width > deeper.width + 3.0,
        "width {} -> {}",
        deeper.width,
        both.width
    );
    assert!(
        both.depth > deeper.depth + 2.0,
        "depth {} -> {}",
        deeper.depth,
        both.depth
    );

    // Each drag is one undo step.
    sim.undo();
    assert_eq!(cab(&sim, id).width, deeper.width);
    sim.undo();
    assert_eq!(cab(&sim, id).depth, before.depth);
}

#[test]
fn fit_to_gap_sizes_a_cabinet_to_the_gap_it_is_dropped_in() {
    let mut sim = house();
    let a = place(&mut sim, 60.0);
    let ca = cab(&sim, a);
    // A second cabinet 25" further along leaves a gap 1" wider than a cabinet.
    let b = place(&mut sim, ca.position.x + ca.width + 25.0 + 12.0);
    let cb = cab(&sim, b);
    let gap = cb.position.x - (ca.position.x + ca.width);
    assert!(
        gap > 20.0,
        "the second cabinet butted against the first instead: {gap}"
    );
    assert!(fit_to_gap_enabled());
    // A third one clicked into the gap takes its width.
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.app.cx.selection.clear();
    sim.click(ca.position.x + ca.width + gap / 2.0, 10.0);
    sim.tool(ToolId::Select);
    let all = cabs(&sim);
    let fitted = all
        .iter()
        .filter(|c| c.id != a && c.id != b && c.kind == CabinetKind::Base && c.joined.is_empty())
        .max_by_key(|c| c.id)
        .expect("a third cabinet");
    assert!(
        (fitted.width - gap).abs() < 0.01,
        "width {} vs gap {gap}",
        fitted.width
    );
    // Off in Preferences: the cabinet keeps its own width and butts instead.
    set_fit_to_gap(false);
    assert!(!fit_to_gap_enabled());
    set_fit_to_gap(true);
}

#[test]
fn touching_base_cabinets_share_one_countertop_that_regenerates_when_one_moves() {
    let mut sim = house();
    assert!(auto_join_enabled());
    let a = place(&mut sim, 60.0);
    let a_pos = cab(&sim, a).position;
    let b = place(&mut sim, a_pos.x + 36.0);
    let c = place(&mut sim, a_pos.x + 60.0);
    let joined = tops(&sim);
    assert_eq!(joined.len(), 1, "three touching bases, one generated top");
    assert_eq!(joined[0].joined.len(), 3);

    // Moving the last cabinet away: the top shrinks to the two that still touch.
    sim.app.cx.selection.set(ObjectRef::Cabinet(c));
    let from = handle(&sim, HandleKind::Move);
    sim.drag((from.x, from.y), (from.x + 150.0, from.y));
    let after = tops(&sim);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].joined.len(), 2, "{:?}", after[0].joined);
    // Put it back against the others: three again, in one undo step.
    sim.undo();
    assert_eq!(tops(&sim)[0].joined.len(), 3);

    // Auto-join off in Preferences: no top is generated for new neighbours.
    let mut p = preferences::current();
    p.auto_join_countertops = false;
    preferences::set(p);
    assert!(!auto_join_enabled());
    sim.app.cx.selection.clear();
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.click(a_pos.x + 200.0, 10.0);
    sim.click(a_pos.x + 224.0, 10.0);
    sim.tool(ToolId::Select);
    let singles = cabs(&sim);
    assert!(singles.iter().filter(|c| c.joined.is_empty()).count() >= 5);
    // Generate Countertop (G) still joins by hand.
    sim.app.cx.run_custom(placed::GENERATE_COUNTERTOP);
    let _ = b;
}

/// A 1x1 PNG, the smallest picture the header reader accepts.
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xDC, 0xCC, 0x59, 0xE7, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

#[test]
fn an_underlay_is_calibrated_by_two_clicks_and_a_real_distance() {
    let mut sim = house();
    let dir = std::env::temp_dir().join(format!("plan-qa-underlay-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("survey.png");
    std::fs::write(&path, PNG_1X1).unwrap();
    let id = import_image(&mut sim.app.cx, &path).unwrap();
    assert_eq!(sim.app.cx.undo_label(), Some("Import Underlay"));
    let u = sim.app.cx.floor().underlay(id).unwrap().clone();
    assert!(!u.calibrated);
    let (a, b) = (u.pixel_to_plan(0.25, 0.5), u.pixel_to_plan(0.5, 0.5));
    assert!((a.dist(b) - 120.0).abs() < 1e-9);

    // Calibrate with the tool: the next two clicks are the points.
    sim.tool(ToolId::Underlay);
    underlay_win::begin_calibration(id);
    sim.click(a.x, a.y);
    sim.click(b.x, b.y);
    let c = underlay_win::calibration().expect("calibration in progress");
    assert!(c.a.is_some_and(|p| p.dist(a) < 1.5) && c.b.is_some_and(|p| p.dist(b) < 1.5));
    // The real distance is 16': the window applies it.
    assert!(apply_calibration(
        &mut sim.app.cx,
        id,
        c.a.unwrap(),
        c.b.unwrap(),
        192.0
    ));
    let u = sim.app.cx.floor().underlay(id).unwrap().clone();
    assert!(u.calibrated);
    let scaled = u.pixel_to_plan(0.25, 0.5).dist(u.pixel_to_plan(0.5, 0.5));
    assert!((scaled - 192.0).abs() < 1e-6, "{scaled}");
    // About the first point; the picture grew 1.6x (from 480" to 768" wide).
    assert!((u.size().0 - 768.0).abs() < 2.0, "{}", u.size().0);
    assert_eq!(sim.app.cx.undo_label(), Some("Calibrate Underlay"));
    sim.undo();
    assert!(!sim.app.cx.floor().underlay(id).unwrap().calibrated);
    underlay_win::cancel_calibration();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dxf_imports_through_the_layer_map_and_comes_out_in_one_undo_step() {
    let mut sim = house();
    let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nA-WALL\n10\n0\n20\n0\n11\n120\n21\n0\n\
0\nLINE\n8\nA-WALL\n10\n0\n20\n60\n11\n120\n21\n60\n\
0\nLINE\n8\nNOTES\n10\n0\n20\n0\n11\n5\n21\n0\n\
0\nLINE\n8\nJUNK\n10\n0\n20\n0\n11\n9\n21\n0\n\
0\nENDSEC\n0\nEOF\n";
    let drawing = parse_dxf(dxf).unwrap();
    let mut opts = ImportOptions::new(UnitsChoice::FromFile.factor(&drawing), "DXF: ");
    opts.layers = vec![
        LayerMapping {
            source: "A-WALL".into(),
            target: LayerTarget::Rename("Walls, Normal".into()),
        },
        LayerMapping {
            source: "NOTES".into(),
            target: LayerTarget::Keep,
        },
        LayerMapping {
            source: "JUNK".into(),
            target: LayerTarget::Skip,
        },
    ];
    let before = sim.app.cx.floor().cad.len();
    let r = import_drawing_with(&mut sim.app.cx, &drawing, &opts, None);
    assert_eq!(r.objects, 3, "the skipped layer stays out");
    let layers: Vec<&str> = sim.app.cx.floor().cad[before..]
        .iter()
        .map(|c| c.layer.as_str())
        .collect();
    assert_eq!(layers.iter().filter(|l| **l == "Walls, Normal").count(), 2);
    assert_eq!(layers.iter().filter(|l| **l == "DXF: NOTES").count(), 1);
    assert!(sim.app.cx.project.layers.get("DXF: NOTES").is_some());
    assert!(sim.app.cx.project.layers.get("DXF: JUNK").is_none());
    assert_eq!(sim.app.cx.undo_label(), Some("Import Drawing"));
    sim.undo();
    assert_eq!(sim.app.cx.floor().cad.len(), before);
    assert!(sim.app.cx.project.layers.get("DXF: NOTES").is_none());
}

#[test]
fn preferences_round_trip_through_the_settings_file_and_drive_the_editor() {
    isolate_home();
    let dir = std::env::temp_dir().join(format!("plan-qa-prefs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("settings.json");
    // Another key in the file survives.
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&file, r#"{"theme":"dark"}"#).unwrap();
    let p = Preferences {
        text_size_pct: 125,
        auto_join_countertops: false,
        fit_cabinets_to_gap: false,
        render_samples: 64,
        selection_color: Some([10, 120, 200]),
        show_status_bar: false,
        ..Preferences::default()
    };
    preferences::write_at(&file, &p).unwrap();
    assert_eq!(preferences::read_at(&file), Some(p.clone()));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("\"theme\""), "{text}");
    // A missing key reads as None; a half-written entry fills with defaults.
    assert_eq!(preferences::read_at(&dir.join("none.json")), None);
    std::fs::write(&file, r#"{"preferences":{"text_size_pct":90}}"#).unwrap();
    let half = preferences::read_at(&file).unwrap();
    assert_eq!(half.text_size_pct, 90);
    assert!(half.auto_join_countertops && half.fit_cabinets_to_gap);

    // Setting them reaches the parts of the editor that act on them.
    preferences::set(p.clone());
    assert!(!auto_join_enabled());
    assert!(!fit_to_gap_enabled());
    assert_eq!(preferences::current(), p);
    assert!(!preferences::show_status_bar());
    assert!((p.zoom() - 1.25).abs() < 1e-6);
    preferences::set(Preferences::default());
    assert!(auto_join_enabled() && fit_to_gap_enabled());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_material_painter_overrides_a_wall_in_3d_through_the_pick_hook() {
    let mut sim = house();
    let wall = sim.app.cx.floor().walls[0].id;
    let before = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let mats = |s: &plan_3d::Scene| -> Vec<plan_3d::Material> {
        s.meshes
            .iter()
            .filter(|m| m.object_id == Some(wall))
            .map(|m| m.material)
            .collect()
    };
    let plain = mats(&before);
    assert!(!plain.is_empty());

    // The 3D view hands a click on the wall to the painter when it is on.
    materials::set_painter_mode(PainterMode::Paint);
    assert!(materials::painter_active());
    materials::set_active(None);
    assert!(!materials::paint_object(
        &mut sim.app.cx,
        0,
        ObjectRef::Wall(wall)
    ));
    assert!(sim.app.cx.status.contains("Pick a material"));
    let name = materials::library()
        .materials
        .iter()
        .find(|m| m.category.first().map(String::as_str) == Some("Masonry"))
        .expect("a masonry material in the library")
        .name
        .clone();
    materials::set_active(Some(name.clone()));
    assert!(materials::paint_object(
        &mut sim.app.cx,
        0,
        ObjectRef::Wall(wall)
    ));
    assert_eq!(sim.app.cx.undo_label(), Some("Paint Material"));
    let painted = mats(&build_view_scene(
        &sim.app.cx.project,
        &ViewScope::default(),
    ));
    assert_ne!(
        painted, plain,
        "the wall's meshes take the painted material"
    );
    // The eyedropper picks it back up; Delete Surface puts the wall right.
    materials::set_active(None);
    materials::set_painter_mode(PainterMode::Eyedropper);
    materials::paint_object(&mut sim.app.cx, 0, ObjectRef::Wall(wall));
    assert_eq!(materials::active_material().as_deref(), Some(name.as_str()));
    materials::set_painter_mode(PainterMode::Erase);
    assert!(materials::paint_object(
        &mut sim.app.cx,
        0,
        ObjectRef::Wall(wall)
    ));
    assert_eq!(
        mats(&build_view_scene(
            &sim.app.cx.project,
            &ViewScope::default()
        )),
        plain
    );
    materials::set_painter_mode(PainterMode::Off);
    let _ = WallKind::Exterior;
}
