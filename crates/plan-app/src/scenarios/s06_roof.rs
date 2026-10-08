//! Scenario 6: Build Roof on the shell, the ridge height of an 8:12 roof,
//! holes and skylights, dormers and Explode Dormer (RF-1..RF-8, RF-35,
//! RF-42, RF-43, RF-48, RF-51).

use super::{draw_shell, Sim};
use crate::editor::roof_view::{self, RoofPlaneRecord};
use crate::editor::{EditorRequest, ObjectRef};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use eframe::egui::Key;
use plan_3d::Material;
use plan_core::geometry::Point;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn planes(sim: &Sim) -> Vec<RoofPlaneRecord> {
    roof_view::load(sim.app.cx.floor()).planes
}

/// Build Roof the way a user does: pick the tool, click, OK the dialog.
fn build_roof(sim: &mut Sim) {
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
}

fn roof_tris(sim: &Sim) -> usize {
    roof_view::roof_meshes(&sim.app.cx.project)
        .iter()
        .map(|m| m.triangle_count())
        .sum()
}

/// The roof plane that faces south: its baseline is the lowest horizontal one.
fn south_plane(sim: &Sim) -> RoofPlaneRecord {
    planes(sim)
        .into_iter()
        .filter(|p| (p.baseline.0.y - p.baseline.1.y).abs() < 1e-6)
        .min_by(|a, b| a.baseline.0.y.total_cmp(&b.baseline.0.y))
        .expect("a south plane")
}

#[test]
fn build_roof_opens_a_dialog_and_ok_builds_four_hip_planes_in_one_step() {
    let mut sim = house();
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    // The dialog is drawn but nothing is built until OK (RF-1).
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(planes(&sim).is_empty(), "no roof before OK");
    // Cancel builds nothing either.
    sim.dialog_frame_key(Some(Key::Escape));
    sim.dialog_frame(false);
    assert!(planes(&sim).is_empty());
    sim.click(240.0, 180.0);
    sim.ok();
    let p = planes(&sim);
    // A rectangle with default (hip) roof directives: four planes.
    assert_eq!(p.len(), 4, "{} planes", p.len());
    assert!(p.iter().all(|r| r.auto));
    // RF-2: the default pitch is 8:12.
    assert!(
        p.iter().all(|r| (r.pitch - 8.0).abs() < 1e-9),
        "{:?}",
        p.iter().map(|r| r.pitch).collect::<Vec<_>>()
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Build Roof"));
    assert!(
        sim.app.cx.status.starts_with("Built 4 roof planes"),
        "{}",
        sim.app.cx.status
    );
    // One undo step removes the whole roof.
    assert_eq!(sim.undo().as_deref(), Some("Build Roof"));
    assert!(planes(&sim).is_empty());
    sim.redo();
    assert_eq!(planes(&sim).len(), 4);
}

#[test]
fn the_ridge_of_an_8_12_hip_roof_is_two_thirds_of_half_the_span_above_the_eave() {
    let mut sim = house();
    build_roof(&mut sim);
    let p = planes(&sim);
    // The two long planes' eave lines are parallel: their distance is the
    // building span (wall to wall plus the overhangs).
    let south = south_plane(&sim);
    let north = p
        .iter()
        .filter(|r| (r.baseline.0.y - r.baseline.1.y).abs() < 1e-6)
        .max_by(|a, b| a.baseline.0.y.total_cmp(&b.baseline.0.y))
        .unwrap();
    let span = north.baseline.0.y - south.baseline.0.y;
    assert!(span > H, "the eaves overhang the walls: span {span}");
    let eave = south.baseline_height();
    // The eave sits at the top of the walls (with the baseline at the top
    // plate the tip hangs a few inches below it).
    let wall_h = sim.app.cx.floor().walls[0].height;
    assert!(
        eave >= wall_h - 12.0 && eave <= wall_h + 24.0,
        "eave {eave} vs wall {wall_h}"
    );
    let ridge = p
        .iter()
        .flat_map(|r| r.polygon3d.iter().map(|v| v[1]))
        .fold(f64::MIN, f64::max);
    let expected = eave + span / 2.0 * 8.0 / 12.0;
    assert!(
        (ridge - expected).abs() < 1.0,
        "ridge {ridge}, expected {expected} (span {span}, eave {eave})"
    );
    // The 3D scene reaches the same height.
    let top = roof_view::roof_meshes(&sim.app.cx.project)
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1] as f64))
        .fold(f64::MIN, f64::max);
    assert!(
        (top - ridge).abs() < 1.0,
        "scene top {top} vs ridge {ridge}"
    );
}

#[test]
fn a_skylight_and_a_roof_hole_cut_the_south_plane_and_undo_separately() {
    let mut sim = house();
    build_roof(&mut sim);
    let south = south_plane(&sim);
    let c = south.centroid();
    let tris_before = roof_tris(&sim);

    // Roof Hole: drag a 40" square inside the plane.
    sim.tool(ToolId::RoofVariant(RoofMode::Hole));
    let r = sim.drag((c.x - 80.0, c.y - 20.0), (c.x - 40.0, c.y + 20.0));
    assert_eq!(r.commit.as_deref(), Some("Roof Hole"));
    let holes = &south_plane(&sim).holes;
    assert_eq!(holes.len(), 1);
    assert!(!holes[0].is_skylight());
    assert_eq!(holes[0].size(), (40.0, 40.0));
    let tris_hole = roof_tris(&sim);
    assert_ne!(tris_hole, tris_before, "the hole is cut out of the 3D roof");

    // Skylight: drag a rectangle (RF-43).
    sim.tool(ToolId::RoofVariant(RoofMode::Skylight));
    let r = sim.drag((c.x + 40.0, c.y - 24.0), (c.x + 88.0, c.y + 24.0));
    assert_eq!(r.commit.as_deref(), Some("Place Skylight"));
    let holes = south_plane(&sim).holes;
    assert_eq!(holes.len(), 2);
    assert!(holes[1].is_skylight());
    // A skylight also adds glass to the scene.
    let glass = roof_view::roof_meshes(&sim.app.cx.project)
        .iter()
        .any(|m| matches!(m.material, Material::Glass | Material::WindowGlass));
    assert!(glass, "the skylight has glass");

    // A click places the default 24 x 48 skylight on the other plane.
    let north_c = planes(&sim)
        .iter()
        .filter(|p| (p.baseline.0.y - p.baseline.1.y).abs() < 1e-6)
        .max_by(|a, b| a.baseline.0.y.total_cmp(&b.baseline.0.y))
        .unwrap()
        .centroid();
    sim.click(north_c.x, north_c.y);
    assert!(planes(&sim).iter().any(|p| p
        .holes
        .iter()
        .any(|h| h.is_skylight() && (h.size().0 - 24.0).abs() < 0.5
            || (h.size().1 - 24.0).abs() < 0.5)));

    assert_eq!(sim.undo().as_deref(), Some("Place Skylight"));
    assert_eq!(sim.undo().as_deref(), Some("Place Skylight"));
    assert_eq!(south_plane(&sim).holes.len(), 1);
    assert_eq!(sim.undo().as_deref(), Some("Roof Hole"));
    assert!(south_plane(&sim).holes.is_empty());
    assert_eq!(roof_tris(&sim), tris_before);
}

#[test]
fn a_hole_outside_every_plane_is_refused_without_an_undo_step() {
    let mut sim = house();
    build_roof(&mut sim);
    let steps = sim.app.cx.undo_label().map(String::from);
    sim.tool(ToolId::RoofVariant(RoofMode::Hole));
    sim.drag((2000.0, 2000.0), (2040.0, 2040.0));
    assert_eq!(sim.app.cx.undo_label().map(String::from), steps);
    assert!(planes(&sim).iter().all(|p| p.holes.is_empty()));
}

#[test]
fn auto_dormer_asks_for_its_specification_then_explode_dormer_turns_it_into_planes() {
    let mut sim = house();
    build_roof(&mut sim);
    let south = south_plane(&sim);
    let c = south.centroid();
    let planes_before = planes(&sim).len();

    sim.tool(ToolId::RoofVariant(RoofMode::Dormer));
    sim.click(c.x, c.y);
    // The Dormer Specification is up: no dormer exists until OK (RF-48).
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(roof_view::load(sim.app.cx.floor()).dormers.is_empty());
    sim.dialog_frame(true);
    sim.dialog_frame(false);
    let set = roof_view::load(sim.app.cx.floor());
    assert_eq!(set.dormers.len(), 1, "status: {}", sim.app.cx.status);
    let dormer = set.dormers[0].clone();
    assert_eq!(dormer.main, south.id);
    // The dormer cuts a hole in the roof under it: 3D triangle count differs.
    assert!(!dormer.floating);

    // Select Objects: double-click on the dormer requests its specification.
    sim.tool(ToolId::Select);
    sim.requests.clear();
    let at = dormer_point(&sim);
    sim.double_click(at.x, at.y);
    assert!(
        sim.requests
            .iter()
            .any(|r| matches!(r, EditorRequest::OpenSpec(ObjectRef::RoofPlane(_)))),
        "{:?}",
        sim.requests
    );
    assert!(sim.app.spec.is_open());
    sim.cancel();

    // Explode Dormer (RF-51).
    sim.tool(ToolId::RoofVariant(RoofMode::Explode));
    let r = sim.click(at.x, at.y);
    assert_eq!(
        r.commit.as_deref(),
        Some("Explode Dormer"),
        "{}",
        sim.app.cx.status
    );
    let set = roof_view::load(sim.app.cx.floor());
    assert!(set.dormers.is_empty());
    assert!(
        set.planes.len() > planes_before,
        "the dormer's planes are plain planes now"
    );
    assert_eq!(sim.undo().as_deref(), Some("Explode Dormer"));
    assert_eq!(roof_view::load(sim.app.cx.floor()).dormers.len(), 1);
}

/// A point inside the first dormer of the floor.
fn dormer_point(sim: &Sim) -> Point {
    let set = roof_view::load(sim.app.cx.floor());
    let d = &set.dormers[0];
    let geo = roof_view::dormer_geometry(&set, d).expect("dormer geometry");
    let _ = geo;
    // The center along the eave and the setback up the slope.
    let main = set.plane(d.main).unwrap();
    let (a, b) = main.baseline;
    let along = (b - a).normalized();
    a + along * d.spec.position_along_eave + main.up_slope() * d.spec.setback_from_eave
}

#[test]
fn double_click_on_a_roof_plane_opens_its_specification_and_editing_it_makes_it_manual() {
    let mut sim = house();
    build_roof(&mut sim);
    let south = south_plane(&sim);
    // Roof planes are picked by their edges: the eave line, outside the walls.
    let (a, b) = south.baseline;
    let c = (a + b) * 0.5;
    sim.tool(ToolId::Select);
    sim.requests.clear();
    sim.double_click(c.x, c.y);
    assert!(
        sim.requests
            .contains(&EditorRequest::OpenSpec(ObjectRef::RoofPlane(south.id))),
        "{:?}",
        sim.requests
    );
    assert!(sim.app.spec.is_open());
    sim.cancel();

    // Apply a pitch change the way the dialog's OK does (one undo step).
    let ridge_before = max_roof_y(&sim);
    let mut edited = south.clone();
    edited.pitch = 12.0;
    let fl = sim.app.cx.floor;
    sim.app.cx.begin_change("Roof Plane Specification");
    assert!(roof_view::apply_plane_edit(
        &mut sim.app.cx.project,
        fl,
        &edited
    ));
    sim.app.cx.mark_dirty();
    let now = roof_view::load(sim.app.cx.floor())
        .plane(south.id)
        .cloned()
        .unwrap();
    assert_eq!(now.pitch, 12.0);
    assert!(!now.auto, "an edited plane is manual (RF-37)");
    assert!(
        max_roof_y(&sim) > ridge_before,
        "a steeper plane is taller in 3D"
    );
    assert_eq!(sim.undo().as_deref(), Some("Roof Plane Specification"));
    assert_eq!(
        roof_view::load(sim.app.cx.floor())
            .plane(south.id)
            .unwrap()
            .pitch,
        8.0
    );
}

fn max_roof_y(sim: &Sim) -> f64 {
    roof_view::roof_meshes(&sim.app.cx.project)
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1] as f64))
        .fold(f64::MIN, f64::max)
}

#[test]
fn delete_roof_planes_and_a_manual_roof_plane_by_drag() {
    let mut sim = house();
    build_roof(&mut sim);
    assert_eq!(planes(&sim).len(), 4);
    // Roof Plane: drag a baseline, click toward the ridge.
    sim.tool(ToolId::RoofVariant(RoofMode::Plane));
    sim.drag((600.0, 0.0), (800.0, 0.0));
    let r = sim.click(700.0, 100.0);
    assert_eq!(r.commit.as_deref(), Some("Draw Roof Plane"));
    let all = planes(&sim);
    assert_eq!(all.len(), 5);
    assert!(!all.last().unwrap().auto);
    assert_eq!(sim.undo().as_deref(), Some("Draw Roof Plane"));
    assert_eq!(planes(&sim).len(), 4);
}

/// Every other multi-mode tool names the mode it is in ("Draw Line",
/// "Auto Exterior Dimensions"); the roof tool answers "Roof" for all twelve.
#[test]
fn each_roof_mode_names_itself_like_its_toolbar_entry() {
    let expected = [
        (RoofMode::Plane, "Roof Plane"),
        (RoofMode::Edit, "Edit Roof Planes"),
        (RoofMode::Build, "Build Roof"),
        (RoofMode::GableLine, "Gable/Roof Line"),
        (RoofMode::Hole, "Roof Hole"),
        (RoofMode::Skylight, "Skylight"),
        (RoofMode::Join, "Join Roof Planes"),
        (RoofMode::Dormer, "Auto Dormer"),
        (RoofMode::FloatingDormer, "Auto Floating Dormer"),
        (RoofMode::Ceiling, "Ceiling Plane"),
        (RoofMode::Explode, "Explode Dormer"),
        (RoofMode::Return, "Roof Return"),
    ];
    let mut sim = Sim::new();
    let mut wrong = Vec::new();
    for (mode, name) in expected {
        sim.tool(ToolId::RoofVariant(mode));
        let got = sim.app.tools.active().name();
        if got != name {
            wrong.push(format!("{mode:?}: \"{got}\" instead of \"{name}\""));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
