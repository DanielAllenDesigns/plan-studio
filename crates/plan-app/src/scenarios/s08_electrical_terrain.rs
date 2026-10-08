//! Scenario 8: outlets, switches and lights on the shell, and the terrain
//! perimeter and elevation line around it; both tools own a specification
//! dialog (CB-51..CB-53, CB-62..CB-67).

use super::{draw_shell, Sim};
use crate::editor::site_view::{self, load_electrical, load_terrain};
use crate::editor::ObjectRef;
use crate::tools::electrical::ElecVariant;
use crate::tools::terrain::TerrainVariant;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::Key;
use plan_electrical::DeviceKind;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn devices(sim: &Sim) -> Vec<plan_electrical::Device> {
    load_electrical(sim.app.cx.floor()).devices
}

fn elec(sim: &mut Sim, v: ElecVariant) {
    sim.tool(ToolId::ElectricalVariant(v));
}

#[test]
fn outlets_and_switches_mount_on_the_wall_face_nearest_the_click() {
    let mut sim = house();
    let south = sim.app.cx.floor().walls[0].clone();
    elec(&mut sim, ElecVariant::Outlet110);
    assert_eq!(sim.app.tools.active().name(), "110V Outlet");
    let r = sim.click(100.0, 8.0);
    assert!(r.commit.is_some(), "{r:?} {}", sim.app.cx.status);
    let d = devices(&sim)[0].clone();
    assert!(matches!(d.kind, DeviceKind::Outlet110));
    assert_eq!(d.wall_id, Some(south.id));
    // On the inside face of the south wall (the click was on the room side).
    assert!(
        (d.position.y - south.thickness / 2.0).abs() < 0.5,
        "{:?}",
        d.position
    );
    assert!((d.position.x - 100.0).abs() <= 1.0);
    // CB-63: faces into the room (+y) at the kind's default height.
    assert!(
        (d.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
        "angle {}",
        d.angle
    );
    assert_eq!(d.height, 12.0);

    elec(&mut sim, ElecVariant::Switch);
    sim.click(30.0, 8.0);
    let sw = devices(&sim)[1].clone();
    assert!(matches!(sw.kind, DeviceKind::Switch));
    assert_eq!(sw.height, 48.0);

    // A click away from every wall places no wall device and says why.
    let n = devices(&sim).len();
    sim.click(240.0, 180.0);
    assert_eq!(devices(&sim).len(), n);
    assert!(sim.app.cx.status.contains("wall"), "{}", sim.app.cx.status);

    // Undo takes them off again, one at a time.
    assert_eq!(sim.undo().as_deref().map(|s| s.is_empty()), Some(false));
    assert_eq!(devices(&sim).len(), 1);
}

#[test]
fn a_light_snaps_to_the_room_center_and_a_switch_is_connected_to_it() {
    let mut sim = house();
    elec(&mut sim, ElecVariant::Light);
    sim.click(244.0, 176.0);
    let light = devices(&sim)[0].clone();
    // The ceiling light sits at the room's center (within 12").
    let room = sim.app.cx.rooms[0].clone();
    assert!(
        light.position.dist(room.centroid) < 12.0,
        "{:?} vs {:?}",
        light.position,
        room.centroid
    );

    elec(&mut sim, ElecVariant::Switch);
    sim.click(30.0, 8.0);
    let sw = devices(&sim)[1].clone();

    // Electrical Connection: switch first, then the light (CB-67).
    elec(&mut sim, ElecVariant::Connection);
    sim.click(sw.position.x, sw.position.y);
    sim.click(light.position.x, light.position.y);
    let layer = load_electrical(sim.app.cx.floor());
    assert_eq!(layer.connections.len(), 1, "{}", sim.app.cx.status);
    assert_eq!(layer.connections[0].from, sw.id);
    assert_eq!(layer.connections[0].to, light.id);
}

#[test]
fn auto_place_outlets_fills_the_rooms_and_does_not_double_up() {
    let mut sim = house();
    elec(&mut sim, ElecVariant::AutoOutlets);
    sim.click(240.0, 180.0);
    let n = devices(&sim).len();
    assert!(n >= 4, "{n} outlets: {}", sim.app.cx.status);
    assert!(devices(&sim).iter().all(|d| d.kind.is_outlet()));
    assert!(sim.app.cx.status.starts_with("Auto Place Outlets"));
    // Running it again adds nothing (outlets already in place are not duplicated).
    sim.click(240.0, 180.0);
    assert_eq!(devices(&sim).len(), n);
    assert_eq!(sim.undo().as_deref(), Some("Auto Place Outlets"));
    assert!(devices(&sim).is_empty());
}

#[test]
fn double_click_on_a_device_opens_the_service_specification_and_ok_applies_it() {
    let mut sim = house();
    elec(&mut sim, ElecVariant::Outlet110);
    sim.click(100.0, 8.0);
    let d = devices(&sim)[0].clone();
    let steps = sim.app.cx.undo_label().map(String::from);
    sim.double_click(d.position.x, d.position.y);
    // The dialog is up: the canvas ignores clicks (no second outlet).
    sim.click(200.0, 8.0);
    assert_eq!(
        devices(&sim).len(),
        1,
        "the canvas is blocked by the dialog"
    );
    // OK applies the draft as one undo step.
    sim.ok();
    sim.click(150.0, 150.0); // any tool event applies the OK
    assert_eq!(
        sim.app.cx.undo_label(),
        Some("Electrical Service Specification"),
        "was {steps:?}"
    );
    assert_eq!(devices(&sim).len(), 1);
    sim.undo();
    assert_eq!(sim.app.cx.undo_label().map(String::from), steps);
}

#[test]
fn the_select_tool_finds_a_device_and_delete_removes_it() {
    let mut sim = house();
    elec(&mut sim, ElecVariant::Outlet110);
    sim.click(100.0, 8.0);
    let d = devices(&sim)[0].clone();
    sim.tool(ToolId::Select);
    sim.click(d.position.x, d.position.y);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Device(d.id)));
    sim.key(KeyEvent::key(Key::Delete));
    assert!(devices(&sim).is_empty());
    sim.undo();
    assert_eq!(devices(&sim).len(), 1);
}

#[test]
fn every_electrical_flavor_activates_with_a_name_and_hint() {
    let mut sim = house();
    for v in [
        ElecVariant::Outlet110,
        ElecVariant::Outlet220,
        ElecVariant::Gfci,
        ElecVariant::Light,
        ElecVariant::RopeLight,
        ElecVariant::Switch,
        ElecVariant::Switch3Way,
        ElecVariant::CeilingFan,
        ElecVariant::SmokeDetector,
        ElecVariant::Connection,
        ElecVariant::AutoOutlets,
    ] {
        elec(&mut sim, v);
        assert_eq!(sim.app.tools.active().name(), v.name());
        assert!(!sim.app.tools.active().hint().is_empty(), "{v:?}");
    }
}

fn perimeter(sim: &mut Sim) {
    sim.tool(ToolId::TerrainVariant(TerrainVariant::Perimeter));
    for (x, y) in [
        (-300.0, -300.0),
        (780.0, -300.0),
        (780.0, 660.0),
        (-300.0, 660.0),
    ] {
        sim.click(x, y);
    }
    sim.key(KeyEvent::key(Key::Enter));
}

#[test]
fn terrain_perimeter_closes_with_enter_and_there_is_only_one() {
    let mut sim = house();
    perimeter(&mut sim);
    let rec = load_terrain(&sim.app.cx.project).expect("terrain stored");
    assert_eq!(rec.terrain.perimeter.len(), 4);
    assert!(rec.has_perimeter());
    assert_eq!(sim.app.cx.undo_label(), Some("Terrain Perimeter"));
    // A second perimeter is refused (CB-51).
    let steps = sim.app.cx.undo_label().map(String::from);
    sim.click(50.0, 50.0);
    assert!(
        sim.app.cx.status.contains("already"),
        "{}",
        sim.app.cx.status
    );
    assert_eq!(sim.app.cx.undo_label().map(String::from), steps);
    // Undo removes it.
    sim.undo();
    assert!(load_terrain(&sim.app.cx.project).is_none_or(|r| !r.has_perimeter()));
}

#[test]
fn an_elevation_line_raises_the_terrain_and_build_terrain_shows_it() {
    let mut sim = house();
    perimeter(&mut sim);
    sim.tool(ToolId::TerrainVariant(TerrainVariant::ElevationLine));
    sim.click(-300.0, 0.0);
    sim.click(780.0, 0.0);
    sim.key(KeyEvent::key(Key::Enter));
    // Ask for the value: type 36" and accept.
    sim.key(KeyEvent::text("36"));
    sim.key(KeyEvent::key(Key::Enter));
    let rec = load_terrain(&sim.app.cx.project).unwrap();
    assert_eq!(
        rec.terrain.elevation_lines.len(),
        1,
        "{}",
        sim.app.cx.status
    );
    let el = &rec.terrain.elevation_lines[0];
    assert_eq!(el.z, 36.0, "{el:?}");
    // Build Terrain: one click builds the surface.
    sim.tool(ToolId::TerrainVariant(TerrainVariant::Build));
    sim.click(0.0, 0.0);
    // On the line the surface is at the line's elevation.
    let z = site_view::terrain_elevation_at(
        &sim.app.cx.project,
        plan_core::geometry::Point::new(100.0, 0.0),
    );
    assert!(z.is_some_and(|z| (z - 36.0).abs() < 1.0), "{z:?}");
    // Elevation lines undo.
    let mut undone = 0;
    while sim.app.cx.can_undo() && undone < 10 {
        if sim.undo().as_deref() == Some("Elevation Line") {
            break;
        }
        undone += 1;
    }
    let after = load_terrain(&sim.app.cx.project).unwrap();
    assert!(after.terrain.elevation_lines.is_empty());
}

#[test]
fn double_click_with_the_terrain_tool_opens_the_terrain_specification() {
    let mut sim = house();
    perimeter(&mut sim);
    sim.tool(ToolId::TerrainVariant(TerrainVariant::ElevationPoint));
    sim.double_click(10.0, 10.0);
    // The dialog blocks the canvas: a click starts no elevation point.
    sim.click(40.0, 40.0);
    assert!(sim.app.cx.temp.editing.is_none());
    sim.ok();
    sim.click(900.0, 900.0); // the next tool event applies the OK
    assert_eq!(sim.app.cx.undo_label(), Some("Terrain Specification"));
}

#[test]
fn every_terrain_flavor_activates_with_a_name_and_hint() {
    let mut sim = house();
    for v in [
        TerrainVariant::Perimeter,
        TerrainVariant::ElevationPoint,
        TerrainVariant::ElevationLine,
        TerrainVariant::ElevationRegion,
        TerrainVariant::Hill,
        TerrainVariant::Valley,
        TerrainVariant::Raised,
        TerrainVariant::Lowered,
        TerrainVariant::Flat,
        TerrainVariant::Road,
        TerrainVariant::Driveway,
        TerrainVariant::Sidewalk,
        TerrainVariant::Hole,
        TerrainVariant::Build,
    ] {
        sim.tool(ToolId::TerrainVariant(v));
        assert_eq!(sim.app.tools.active().name(), v.name());
        assert!(!sim.app.tools.active().hint().is_empty(), "{v:?}");
        sim.esc();
    }
}
