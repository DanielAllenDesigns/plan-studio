//! Scenario 92b: terrain and site, Round 16 brief 33. A sloped lot with a
//! flattened building pad: Clear Terrain removes only the generated surface
//! (the perimeter, the survey data, the pad and the features stay) and a
//! rebuild brings it back; a retaining wall along a break; labels on the plot
//! plan; the terrain schedule counts; the click-once Elevation Region and
//! modifier squares; the perimeter's Fill Style. Every action is one undo step.

use super::{draw_shell, Sim};
use crate::editor::site_view::{self, load_terrain};
use crate::toolbar::{Action, TerrainCommand};
use crate::tools::terrain::TerrainVariant as V;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::Key;
use plan_core::geometry::Point;
use plan_terrain::{BuildingPad, ElevationPoint, FillStyle, ScheduleCategory};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn lot() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
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
    sim
}

fn rec(sim: &Sim) -> site_view::TerrainRecord {
    load_terrain(&sim.app.cx.project).expect("a terrain")
}

/// West edge 10 ft high, east edge level with the street, plus a flattened
/// pad (first floor at 5 ft) under a 20 ft square footprint.
fn survey_with_pad(sim: &mut Sim) {
    site_view::edit_terrain(&mut sim.app.cx, "Survey", |r| {
        for (x, y, z) in [
            (-300.0, -300.0, 120.0),
            (-300.0, 660.0, 120.0),
            (780.0, -300.0, 0.0),
            (780.0, 660.0, 0.0),
        ] {
            r.terrain.elevation_points.push(ElevationPoint {
                pos: Point::new(x, y),
                z,
            });
        }
        r.terrain.flatten_pad = true;
        r.terrain.building_pad = Some(BuildingPad {
            footprint: vec![
                Point::new(100.0, 100.0),
                Point::new(340.0, 100.0),
                Point::new(340.0, 340.0),
                Point::new(100.0, 340.0),
            ],
            margin: 24.0,
            first_floor: Some(60.0),
            ..BuildingPad::default()
        });
    });
}

fn build(sim: &mut Sim) {
    sim.tool(ToolId::TerrainVariant(V::Build));
    sim.click(0.0, 0.0);
}

fn surface(sim: &Sim) -> plan_terrain::TerrainSurface {
    site_view::terrain_view(&sim.app.cx.project)
        .and_then(|v| v.surface.clone())
        .expect("a built surface")
}

fn height(sim: &Sim, x: f64, y: f64) -> f64 {
    plan_terrain::elevation_at(&surface(sim), Point::new(x, y)).expect("inside the surface")
}

#[test]
fn clear_terrain_keeps_the_pad_and_the_data_and_a_rebuild_brings_the_surface_back() {
    let mut sim = lot();
    survey_with_pad(&mut sim);
    build(&mut sim);
    assert_eq!(sim.app.cx.undo_label(), Some("Build Terrain"));
    // The pad is level under the footprint while the lot falls away east.
    let (a, b) = (height(&sim, 140.0, 140.0), height(&sim, 300.0, 300.0));
    assert!((a - b).abs() < 1.0, "pad is level: {a} {b}");
    assert!(height(&sim, -250.0, 0.0) > height(&sim, 700.0, 0.0) + 60.0);

    sim.action(Action::Terrain(TerrainCommand::Clear));
    assert_eq!(sim.app.cx.undo_label(), Some("Clear Terrain"));
    let r = rec(&sim);
    assert!(
        !r.built && r.terrain.last_build.is_none(),
        "surface is gone"
    );
    assert_eq!(r.terrain.perimeter.len(), 4);
    assert_eq!(r.terrain.elevation_points.len(), 4);
    assert!(r.terrain.building_pad.is_some() && r.terrain.flatten_pad);

    // Rebuilding from what stayed gives the same pad.
    build(&mut sim);
    assert!(rec(&sim).built);
    assert!((height(&sim, 140.0, 140.0) - a).abs() < 1.0);

    // Clear is exactly one undo step.
    sim.action(Action::Terrain(TerrainCommand::Clear));
    assert_eq!(sim.undo().as_deref(), Some("Clear Terrain"));
    assert!(rec(&sim).built);
}

#[test]
fn a_retaining_wall_labels_and_the_schedule_count_on_the_plot_plan() {
    let mut sim = lot();
    survey_with_pad(&mut sim);
    build(&mut sim);

    sim.tool(ToolId::TerrainVariant(V::StraightRetainingWall));
    sim.click(480.0, -100.0);
    sim.click(480.0, 500.0);
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.app.cx.undo_label(), Some("Straight Retaining Wall"));
    let t = rec(&sim).terrain;
    assert_eq!((t.breaks.len(), t.walls.len()), (1, 1));
    // The wall's height comes from the ground on its two sides.
    assert!(t.walls[0].retain > 1.0, "grade step {}", t.walls[0].retain);

    // A label on the wall shows on the plot plan and is one undo step.
    let shapes = sim.plan_shapes().len();
    sim.tool(ToolId::TerrainVariant(V::TerrainLabels));
    sim.click(480.0, 200.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Terrain Label"));
    let t = rec(&sim).terrain;
    // The break and the wall share the line; the click labels one of them.
    assert!(t.object_keys().into_iter().any(|k| t.extras(k).label.shown));
    assert_eq!(plan_terrain::label_spots(&t).len(), 1);
    assert!(sim.plan_shapes().len() > shapes, "the label is drawn");

    // Schedule counts: one perimeter, the wall as a Terrain Path.
    let rows = plan_terrain::terrain_schedule(&t);
    let count = |c: ScheduleCategory| rows.iter().filter(|r| r.category == c).count();
    assert_eq!(count(ScheduleCategory::TerrainPerimeter), 1);
    assert_eq!(count(ScheduleCategory::TerrainPaths), 1);

    // Undo walks back one step at a time: label, then wall and break.
    assert_eq!(sim.undo().as_deref(), Some("Terrain Label"));
    assert_eq!(sim.undo().as_deref(), Some("Straight Retaining Wall"));
    let t = rec(&sim).terrain;
    assert_eq!((t.breaks.len(), t.walls.len()), (0, 0));
}

#[test]
fn one_click_makes_the_default_region_squares_and_the_perimeter_takes_a_fill() {
    let mut sim = lot();
    sim.tool(ToolId::TerrainVariant(V::ElevationRegion));
    sim.click(100.0, 100.0);
    sim.key(KeyEvent::key(Key::Enter));
    // The inline field asks for the elevation; Enter takes the default.
    sim.key(KeyEvent::key(Key::Enter));
    let t = rec(&sim).terrain;
    assert_eq!(t.elevation_regions.len(), 1, "one click, one region");
    let poly = &t.elevation_regions[0].polygon;
    assert_eq!(poly.len(), 4);
    let xs: Vec<f64> = poly.iter().map(|p| p.x).collect();
    let width =
        xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
    assert!((width - 96.0).abs() < 1e-6, "8 ft square: {width}");

    sim.tool(ToolId::TerrainVariant(V::Hill));
    sim.click(300.0, 300.0);
    sim.key(KeyEvent::key(Key::Enter));
    sim.key(KeyEvent::key(Key::Enter));
    let t = rec(&sim).terrain;
    assert_eq!(t.modifiers.len(), 1);
    let ys: Vec<f64> = t.modifiers[0].polygon.iter().map(|p| p.y).collect();
    let depth =
        ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min);
    assert!((depth - 120.0).abs() < 1e-6, "10 ft square: {depth}");

    // Fill Style of the perimeter draws a fill on the Terrain layer.
    let plain = plan_terrain::landscape_plan(&rec(&sim).terrain).len();
    site_view::edit_terrain(&mut sim.app.cx, "Terrain Specification", |r| {
        r.terrain.perimeter_extras.style.fill = FillStyle::Solid;
    });
    let items = plan_terrain::landscape_plan(&rec(&sim).terrain);
    assert!(items.len() > plain);
    assert!(items
        .iter()
        .any(|i| i.layer == "Terrain" && matches!(i.shape, plan_terrain::PlanShape::Fill { .. })));
}
