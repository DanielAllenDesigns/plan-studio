//! Scenario 67: integration pass 2, area A (plan-core, 3D, walls, rooms, roofs,
//! openings). The shared opening placement rules reach the editor's move, the
//! Bay Roof options reach the dialog and the 3D unit, and the terrain itself
//! selected offers its perimeter's corners.

use super::{draw_shell, Sim};
use crate::editor::site_view::{self, load_terrain};
use crate::editor::{ops, ObjectRef};
use crate::tools::terrain::TerrainVariant as V;
use crate::tools::{KeyEvent, ToolId};
use crate::ActiveDialog;
use eframe::egui::{self, Key};
use plan_3d::{build_scene, Material};
use plan_core::geometry::Point;
use plan_core::openings::BayRoofKind;
use plan_core::{Id, OpeningStyle, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn place(sim: &mut Sim, tool: ToolId, x: f64) -> Id {
    sim.tool(tool);
    sim.click(x, 0.5);
    sim.esc();
    sim.app
        .cx
        .floor()
        .openings
        .iter()
        .max_by_key(|o| o.id)
        .map(|o| o.id)
        .expect("placed")
}

#[test]
fn the_terrain_selected_offers_the_corners_of_its_perimeter() {
    let mut sim = house();
    sim.tool(ToolId::TerrainVariant(V::Perimeter));
    for (x, y) in [
        (-300.0, -300.0),
        (780.0, -300.0),
        (780.0, 660.0),
        (-300.0, 660.0),
    ] {
        sim.click(x, y);
    }
    sim.key(KeyEvent::key(Key::Enter));
    let p0 = load_terrain(&sim.app.cx.project)
        .expect("a terrain")
        .terrain
        .perimeter
        .clone();
    assert_eq!(p0.len(), 4);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Terrain);
    let handles = crate::editor::handles::handles_for(&sim.app.cx, 1.0);
    assert_eq!(handles.len(), 4);
    assert!(handles
        .iter()
        .all(|h| h.target == ObjectRef::TerrainObject(site_view::TerrainHit::Perimeter)));
    for (h, p) in handles.iter().zip(&p0) {
        assert!(h.pos.dist(*p) < 1e-6);
    }
    // Dragging a corner reshapes the perimeter in one undo step.
    let corner = p0[2];
    sim.drag((corner.x, corner.y), (corner.x + 60.0, corner.y + 40.0));
    let p1 = load_terrain(&sim.app.cx.project)
        .expect("a terrain")
        .terrain
        .perimeter
        .clone();
    assert!(p1[2].dist(Point::new(corner.x + 60.0, corner.y + 40.0)) < 1e-6);
    sim.undo();
    let back = load_terrain(&sim.app.cx.project).unwrap().terrain.perimeter;
    assert_eq!(back, p0);
}

#[test]
fn an_opening_moved_by_the_editor_keeps_clear_of_a_meeting_wall() {
    let mut sim = house();
    // A partition meeting the south wall's inside at x = 240.
    let south = sim.app.cx.floor().walls[0].id;
    sim.app.cx.project.add_wall(
        0,
        Point::new(240.0, 0.0),
        Point::new(240.0, 200.0),
        4.5,
        96.0,
        WallKind::Interior,
    );
    let win = place(&mut sim, ToolId::Window, 100.0);
    let width = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == win)
        .unwrap()
        .width;
    // Onto the partition: refused. Beside it with the clearance: allowed.
    assert!(!ops::place_opening_at(
        &mut sim.app.cx.project,
        0,
        win,
        south,
        240.0
    ));
    // The jamb stops where its casing (3 3/4") meets the partition (Round 16).
    let clear = 240.0 - 2.25 - 3.75 - width * 0.5;
    assert!(ops::place_opening_at(
        &mut sim.app.cx.project,
        0,
        win,
        south,
        clear
    ));
    assert!(!ops::place_opening_at(
        &mut sim.app.cx.project,
        0,
        win,
        south,
        clear + 1.0
    ));
}

#[test]
fn the_bay_roof_options_reach_the_dialog_and_the_3d_unit() {
    let mut sim = house();
    let id = place(&mut sim, ToolId::Window, 240.0);
    for o in &mut sim.app.cx.project.floors[0].openings {
        if o.id == id {
            o.style = OpeningStyle::BayWindow;
            o.width = 72.0;
        }
    }
    assert!(sim.open_spec(ObjectRef::Opening(id)));
    let ctx = egui::Context::default();
    {
        let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() else {
            panic!("no opening dialog");
        };
        assert!(d.draw_tab_for_test(&ctx, "Options"));
        let roof = &mut d.draft_mut().extras.spec.bay_roof;
        roof.kind = BayRoofKind::None;
        d.draw_tab_for_test(&ctx, "Options");
    }
    sim.ok();
    let stored = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == id)
        .unwrap()
        .extras
        .spec
        .bay_roof;
    assert_eq!(stored.kind, BayRoofKind::None);
    let roof_meshes = |sim: &Sim| {
        build_scene(&sim.app.cx.project)
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id) && m.material == Material::Roof)
            .count()
    };
    assert_eq!(roof_meshes(&sim), 0);
    // Back to the default: the hip returns.
    for o in &mut sim.app.cx.project.floors[0].openings {
        if o.id == id {
            o.extras.spec.bay_roof.kind = BayRoofKind::Default;
        }
    }
    assert!(roof_meshes(&sim) > 0);
}

#[test]
fn a_select_double_click_on_a_fireplace_opens_the_fireplace_specification() {
    use crate::editor::fireplace_view as fv;
    use crate::tools::fireplace::FireplaceMode as M;
    let mut sim = house();
    sim.tool(ToolId::FireplaceVariant(M::Masonry));
    assert!(sim.click(240.0, 30.0).commit.is_some());
    let id = match sim.app.cx.selection.single() {
        Some(ObjectRef::Symbol(id)) => id,
        other => panic!("no fireplace selected: {other:?}"),
    };
    let (sym, _) = fv::load(sim.app.cx.floor(), id).unwrap();
    sim.tool(ToolId::Select);
    let r = sim.double_click(sym.position.x, sym.position.y + 12.0);
    assert!(r.consumed);
    // The request to switch tools is handled on the next pass of the shell.
    sim.app.process_requests();
    // The Fireplace tool took over to host its dialog; Esc closes it and the
    // Select tool is back.
    assert_eq!(sim.app.tools.active().name(), "Fireplace");
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame_key(Some(egui::Key::Escape));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
}

#[test]
fn a_window_placed_into_a_bedroom_starts_from_the_egress_window() {
    let placed = |room_type: &str| {
        let mut sim = house();
        sim.app.cx.defaults.code.bedroom_window.width = 44.0;
        sim.app.cx.defaults.code.bedroom_window.height = 78.0;
        sim.app.cx.defaults.code.bedroom_window.sill_height = 20.0;
        sim.app
            .cx
            .floor_mut()
            .room_names
            .push(plan_core::RoomName::new(
                Point::new(240.0, 180.0),
                room_type,
                room_type,
            ));
        sim.app.cx.mark_dirty();
        sim.app.cx.refresh();
        sim.tool(ToolId::Window);
        assert!(sim.click(240.0, 0.0).commit.is_some());
        sim.app
            .cx
            .floor()
            .openings
            .iter()
            .find(|o| o.kind == plan_core::OpeningKind::Window)
            .cloned()
            .expect("a window")
    };
    let bed = placed("Bedroom");
    assert!(bed.width >= 44.0 && bed.height >= 78.0 && bed.sill_height <= 20.0);
    let kitchen = placed("Kitchen");
    assert!(kitchen.sill_height > 20.0, "{}", kitchen.sill_height);
}

#[test]
fn the_room_types_button_opens_the_default_settings_list() {
    let mut sim = house();
    assert!(sim.app.lists.is_none());
    crate::dialogs::room::press_room_types_for_test();
    sim.app.app_commands(&egui::Context::default());
    assert!(matches!(
        sim.app.lists,
        Some(crate::dialogs::DefaultsList::RoomTypes(_))
    ));
}

#[test]
fn build_foundation_starts_from_the_code_footing() {
    let mut sim = house();
    sim.app.cx.defaults.code.footing_width = 20.0;
    sim.app.cx.defaults.code.footing_thickness = 10.0;
    let spec = crate::editor::rooms_edit::FoundationSpec::from_defaults(&sim.app.cx.defaults);
    assert_eq!((spec.footing_width, spec.footing_depth), (20.0, 10.0));
    let o = spec.to_options();
    assert_eq!((o.footing_width, o.footing_depth), (20.0, 10.0));
}
