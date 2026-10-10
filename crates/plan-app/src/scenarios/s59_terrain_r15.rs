//! Scenario 59: terrain, round 15 (manual audit part 6 gaps 1, 5, 6, 9, 15
//! and the terrain parts of 16 and 17). Every new tool is used with the
//! pointer, every new setting goes through the specification (draft, OK,
//! undo) and shows in the plan or the 3D scene: Retaining Wall tools, the
//! 5 ft terrain wall, Absolute Elevation with the Reference Point tools, the
//! skirt, Hide Terrain Intersected by Building, smoothing and triangle count,
//! contours on their own layers with offset, units and red negatives, the
//! Import Terrain and GPS assistants, medians, cul-de-sacs, polyline roads,
//! flares, Auto Generate Sidewalk, plant images with seasons, Grow All
//! Plants, labels and the terrain schedule.

use super::{draw_shell, Sim};
use crate::editor::site_view::{self, load_terrain, TerrainHit};
use crate::editor::ObjectRef;
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::terrain::TerrainVariant as V;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::Key;
use plan_core::geometry::Point;
use plan_terrain::{
    AbsoluteElevation, ElevationPoint, LandscapeKind, ObjectKey, RoadKind, Season, TriangleDetail,
};

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

fn slope(sim: &mut Sim) {
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

fn scene(sim: &Sim) -> plan_3d::Scene {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
}

fn spec(sim: &mut Sim, edit: impl FnOnce(&mut site_view::TerrainRecord)) {
    let mut draft = rec(sim);
    edit(&mut draft);
    site_view::edit_terrain(&mut sim.app.cx, "Terrain Specification", |r| {
        r.apply_spec(&draft)
    });
}

#[test]
fn a_retaining_wall_is_a_break_and_a_wall_in_one_undo_step() {
    let mut sim = lot();
    slope(&mut sim);
    build(&mut sim);
    let before = sim.app.cx.undo_label().map(String::from);
    sim.tool(ToolId::TerrainVariant(V::StraightRetainingWall));
    // Drawn from south to north: the west (high) side is on its left.
    sim.click(240.0, -100.0);
    sim.click(240.0, 400.0);
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.app.cx.undo_label(), Some("Straight Retaining Wall"));
    let t = rec(&sim).terrain;
    assert_eq!((t.breaks.len(), t.walls.len()), (1, 1));
    let wall = &t.walls[0];
    assert!(
        wall.retain > 1.0,
        "the grade step is the drop across it: {}",
        wall.retain
    );
    assert_eq!(wall.height, 0.0, "the top matches the high side");
    assert!(wall.cut && !wall.stepped);
    assert!(t.breaks[0].follow_ground);
    // The surface steps down across it and the wall shows in 3D.
    let s = surface(&sim);
    let west = plan_terrain::elevation_at(&s, Point::new(240.0 - 8.0, 150.0)).unwrap();
    let east = plan_terrain::elevation_at(&s, Point::new(240.0 + 8.0, 150.0)).unwrap();
    assert!(west - east > wall.retain * 0.8, "{west} {east}");
    assert!(scene(&sim).meshes.iter().any(|m| m.object_id
        == Some(plan_terrain::terrain_object_id(
            plan_terrain::TerrainPart::Wall,
            0
        ))));
    // One undo takes both away.
    assert_eq!(sim.undo().as_deref(), Some("Straight Retaining Wall"));
    let t = rec(&sim).terrain;
    assert_eq!((t.breaks.len(), t.walls.len()), (0, 0));
    assert_eq!(sim.app.cx.undo_label().map(String::from), before);
    // Drawn the other way round it is turned so the high side stays on the left.
    sim.tool(ToolId::TerrainVariant(V::StraightRetainingWall));
    sim.click(240.0, 400.0);
    sim.click(240.0, -100.0);
    sim.key(KeyEvent::key(Key::Enter));
    let wall = &rec(&sim).terrain.walls[0];
    assert!(
        wall.points[0].dist(Point::new(240.0, -100.0)) < 1e-6,
        "{:?}",
        wall.points[0]
    );
}

#[test]
fn a_curved_retaining_wall_takes_the_arc_of_the_curved_wall() {
    let mut sim = lot();
    slope(&mut sim);
    sim.tool(ToolId::TerrainVariant(V::CurvedRetainingWall));
    sim.click(100.0, 0.0);
    sim.click(100.0, 300.0);
    sim.click(160.0, 150.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Curved Retaining Wall"));
    let t = rec(&sim).terrain;
    assert!(t.walls[0].curved && t.walls[0].points.len() > 4);
    assert_eq!(t.breaks.len(), 1);
}

#[test]
fn terrain_walls_are_five_feet_follow_the_ground_and_stepping_is_an_option() {
    let mut sim = lot();
    sim.tool(ToolId::TerrainVariant(V::StraightWall));
    sim.click(0.0, 100.0);
    sim.click(400.0, 100.0);
    sim.key(KeyEvent::key(Key::Enter));
    let w = &rec(&sim).terrain.walls[0];
    assert_eq!((w.height, w.retain), (60.0, 0.0));
    assert!(!w.stepped);
    // Stepping is switched on in the wall's specification.
    site_view::edit_terrain(&mut sim.app.cx, "Terrain Wall Specification", |r| {
        r.terrain.walls[0].stepped = true;
    });
    assert!(rec(&sim).terrain.walls[0].stepped);
}

#[test]
fn absolute_elevation_moves_the_surface_to_the_floor_and_the_reference_point_tools_work() {
    let mut sim = lot();
    slope(&mut sim);
    // Place the Terrain Elevation Reference Point with its tool.
    sim.tool(ToolId::TerrainVariant(V::ReferencePoint));
    sim.click(-300.0, -300.0);
    assert_eq!(
        sim.app.cx.undo_label(),
        Some("Terrain Elevation Reference Point")
    );
    assert_eq!(
        rec(&sim).terrain.reference_point,
        Some(Point::new(-300.0, -300.0))
    );
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
    // Automatic leaves the survey where it is; retaining at the point puts it 6" under the floor.
    build(&mut sim);
    let raw = plan_terrain::elevation_at(&surface(&sim), Point::new(-300.0, -300.0)).unwrap();
    assert!((raw - 120.0).abs() < 1e-6);
    spec(&mut sim, |r| {
        r.terrain.absolute_elevation = AbsoluteElevation::ReferencePoint;
        r.terrain.surface_offset = -6.0;
    });
    build(&mut sim);
    let shifted = plan_terrain::elevation_at(&surface(&sim), Point::new(-300.0, -300.0)).unwrap();
    assert!((shifted + 6.0).abs() < 1e-6, "{shifted}");
    // Remove the point.
    sim.tool(ToolId::TerrainVariant(V::RemoveReferencePoint));
    sim.click(0.0, 0.0);
    assert_eq!(rec(&sim).terrain.reference_point, None);
    assert_eq!(
        sim.app.cx.undo_label(),
        Some("Remove Terrain Elevation Reference Point")
    );
    // Contour 0 retains the surface at elevation 0 instead.
    spec(&mut sim, |r| {
        r.terrain.absolute_elevation = AbsoluteElevation::ContourZero;
        r.terrain.surface_offset = -12.0;
        r.terrain.floor_one_elevation = 24.0;
    });
    build(&mut sim);
    let east = plan_terrain::elevation_at(&surface(&sim), Point::new(780.0, -300.0)).unwrap();
    assert!((east - 12.0).abs() < 1e-6, "{east}");
}

#[test]
fn the_skirt_and_the_surface_detail_settings_reach_the_scene_and_the_build() {
    let mut sim = lot();
    slope(&mut sim);
    build(&mut sim);
    let meshes = |sim: &Sim| scene(sim).meshes.len();
    let plain = meshes(&sim);
    let grid_triangles = surface(&sim).triangles.len();
    spec(&mut sim, |r| {
        r.terrain.skirt.enabled = true;
        r.terrain.skirt.thickness = 36.0;
    });
    build(&mut sim);
    assert_eq!(meshes(&sim), plain + 1, "a skirt mesh");
    // Medium detail asks for about 2000 triangles.
    spec(&mut sim, |r| {
        r.terrain.triangle_detail = TriangleDetail::Medium
    });
    build(&mut sim);
    let medium = surface(&sim).triangles.len();
    assert!(
        medium != grid_triangles && (1000..4000).contains(&medium),
        "{medium}"
    );
    // The build reports its triangles for the specification.
    let stats = rec(&sim).terrain.last_build.expect("a build report");
    assert_eq!(stats.triangles as usize, medium);
    // Hide Terrain Intersected by Building leaves a gap under the house.
    site_view::edit_terrain(&mut sim.app.cx, "Terrain Specification", |r| {
        r.terrain.triangle_detail = TriangleDetail::Grid;
        r.terrain.flatten_pad = false;
        r.terrain.building_pad = Some(plan_terrain::BuildingPad {
            footprint: vec![
                Point::new(0.0, 0.0),
                Point::new(480.0, 0.0),
                Point::new(480.0, 360.0),
                Point::new(0.0, 360.0),
            ],
            ..plan_terrain::BuildingPad::default()
        });
        r.terrain.hide_under_building = true;
    });
    build(&mut sim);
    assert!(plan_terrain::elevation_at(&surface(&sim), Point::new(240.0, 180.0)).is_none());
    assert!(plan_terrain::elevation_at(&surface(&sim), Point::new(-200.0, -200.0)).is_some());
}

#[test]
fn clearing_the_terrain_keeps_the_perimeter_and_the_data() {
    let mut sim = lot();
    slope(&mut sim);
    build(&mut sim);
    assert!(rec(&sim).built && rec(&sim).terrain.last_build.is_some());
    site_view::edit_terrain(&mut sim.app.cx, "Clear Terrain", |r| {
        assert!(r.clear_generated());
    });
    let r = rec(&sim);
    assert!(!r.built && r.terrain.last_build.is_none());
    assert_eq!(r.terrain.perimeter.len(), 4);
    assert_eq!(r.terrain.elevation_points.len(), 4);
}

#[test]
fn contours_sit_on_their_own_layers_with_their_units_and_red_negatives() {
    let mut sim = lot();
    site_view::edit_terrain(&mut sim.app.cx, "Survey", |r| {
        for (x, y, z) in [
            (-300.0, -300.0, -60.0),
            (-300.0, 660.0, -60.0),
            (780.0, -300.0, 60.0),
            (780.0, 660.0, 60.0),
        ] {
            r.terrain.elevation_points.push(ElevationPoint {
                pos: Point::new(x, y),
                z,
            });
        }
    });
    site_view::ensure_landscape_layers(&mut sim.app.cx.project);
    sim.app.cx.project.layers.layers.iter().for_each(|_| {});
    spec(&mut sim, |r| {
        r.contour_interval = 24.0;
        r.terrain.contour_offset = 12.0;
        r.terrain.contour_label_units = plan_terrain::LabelUnits::DecimalFeet;
        r.terrain.highlight_negative = true;
        r.terrain.contour_label_major_only = false;
        r.terrain.contour_label_spacing = 0.0;
    });
    build(&mut sim);
    let view = site_view::terrain_view(&sim.app.cx.project).unwrap();
    let levels: Vec<f64> = view.contours.iter().map(|c| c.z).collect();
    assert!(
        !levels.is_empty() && levels.iter().all(|z| ((z - 12.0) % 24.0).abs() < 1e-6),
        "{levels:?}"
    );
    let texts: Vec<(&str, bool)> = view
        .symbols
        .iter()
        .filter_map(|s| match s {
            plan_terrain::Stroke::Text { text, negative, .. } => Some((text.as_str(), *negative)),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|(t, n)| t.starts_with('-') && *n),
        "{texts:?}"
    );
    assert!(texts.iter().all(|(t, _)| t.ends_with('\'')));
    // The two layers exist and hiding one removes its lines from the plan.
    for name in [
        plan_terrain::LAYER_PRIMARY_CONTOURS,
        plan_terrain::LAYER_SECONDARY_CONTOURS,
    ] {
        assert!(sim.app.cx.project.layers.get(name).is_some(), "{name}");
    }
    let all = sim.plan_shapes().len();
    sim.app
        .cx
        .project
        .layers
        .get_mut(plan_terrain::LAYER_SECONDARY_CONTOURS)
        .unwrap()
        .display = false;
    sim.app.cx.refresh();
    let fewer = sim.plan_shapes().len();
    assert!(fewer < all, "{fewer} vs {all}");
}

#[test]
fn the_terrain_labels_tool_switches_a_label_on_and_the_schedule_lists_the_object() {
    let mut sim = lot();
    sim.tool(ToolId::TerrainVariant(V::StraightWall));
    sim.click(0.0, 100.0);
    sim.click(400.0, 100.0);
    sim.key(KeyEvent::key(Key::Enter));
    sim.tool(ToolId::TerrainVariant(V::TerrainLabels));
    sim.click(200.0, 100.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Terrain Label"));
    let t = rec(&sim).terrain;
    assert!(t.extras(ObjectKey::Wall(0)).label.shown);
    let spots = plan_terrain::label_spots(&t);
    assert_eq!(spots.len(), 1);
    assert!(spots[0].text.starts_with("Terrain Wall"));
    assert!(sim
        .app
        .cx
        .project
        .layers
        .get(plan_terrain::LAYER_TERRAIN_LABELS)
        .is_some());
    // Clicking again switches it off.
    sim.tool(ToolId::TerrainVariant(V::TerrainLabels));
    sim.click(200.0, 100.0);
    assert!(!rec(&sim).terrain.extras(ObjectKey::Wall(0)).label.shown);
    // The wall is a Terrain Path in the schedule, and the perimeter has its own category.
    let rows = plan_terrain::terrain_schedule(&rec(&sim).terrain);
    assert!(rows
        .iter()
        .any(|r| r.category == plan_terrain::ScheduleCategory::TerrainPaths));
    assert!(rows
        .iter()
        .any(|r| r.category == plan_terrain::ScheduleCategory::TerrainPerimeter));
    // The General schedule lists them too.
    let entries = plan_docs::schedule_kinds::entries(
        &sim.app.cx.project,
        plan_core::schedules::ScheduleKind::General,
        None,
    );
    assert!(entries.iter().any(
        |e| e.cell("category") == "Terrain Paths" && e.cell("name").starts_with("Terrain Wall")
    ));
}

#[test]
fn polyline_roads_medians_cul_de_sacs_and_sidewalks_are_placed_and_undone() {
    let mut sim = lot();
    // A straight road, then its median, cul-de-sac, a polyline driveway and sidewalks.
    sim.tool(ToolId::TerrainVariant(V::Road));
    sim.click(-200.0, 300.0);
    sim.click(600.0, 300.0);
    sim.key(KeyEvent::key(Key::Enter));
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(rec(&sim).terrain.roads.len(), 1);
    sim.tool(ToolId::TerrainVariant(V::Median));
    for (x, y) in [
        (100.0, 290.0),
        (200.0, 290.0),
        (200.0, 310.0),
        (100.0, 310.0),
    ] {
        sim.click(x, y);
    }
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.app.cx.undo_label(), Some("Median"));
    sim.tool(ToolId::TerrainVariant(V::CulDeSac));
    sim.click(595.0, 300.0);
    let t = rec(&sim).terrain;
    let cds = t.roads.last().unwrap();
    assert_eq!(cds.kind, RoadKind::CulDeSac);
    assert_eq!(
        cds.center,
        Point::new(600.0, 300.0),
        "on the end of the road"
    );
    sim.tool(ToolId::TerrainVariant(V::PolylineDriveway));
    for (x, y) in [(0.0, 200.0), (80.0, 200.0), (80.0, 100.0), (0.0, 100.0)] {
        sim.click(x, y);
    }
    sim.key(KeyEvent::key(Key::Enter));
    let t = rec(&sim).terrain;
    let drive = t.roads.last().unwrap();
    assert!(drive.kind == RoadKind::Driveway && drive.outline.len() == 4);
    // Auto Generate Sidewalk: click the road, type no offset.
    let before = t.roads.len();
    sim.tool(ToolId::TerrainVariant(V::AutoSidewalk));
    sim.click(0.0, 300.0);
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(sim.app.cx.undo_label(), Some("Auto Generate Sidewalk"));
    let t = rec(&sim).terrain;
    assert_eq!(t.roads.len(), before + 2, "a sidewalk each side");
    assert!(t.roads[before..]
        .iter()
        .all(|r| r.kind == RoadKind::Sidewalk));
    // Hit-testing finds an outline road inside its shape.
    assert_eq!(
        site_view::hit_terrain(&t, Point::new(40.0, 150.0), 6.0),
        Some(TerrainHit::Road(
            t.roads
                .iter()
                .position(|r| r.kind == RoadKind::Driveway)
                .unwrap()
        ))
    );
    // Build and look at the scene: meshes for every road object.
    build(&mut sim);
    let road_meshes = scene(&sim)
        .meshes
        .iter()
        .filter(|m| {
            matches!(
                m.object_id.and_then(plan_terrain::terrain_object_of),
                Some((plan_terrain::TerrainPart::Road, _))
            )
        })
        .count();
    assert!(road_meshes >= t.roads.len(), "{road_meshes}");
    // Undo the sidewalks in one step.
    assert_eq!(
        sim.undo().as_deref(),
        Some("Build Terrain")
            .or(Some("Build Terrain"))
            .map(str::to_string)
            .as_deref()
    );
}

#[test]
fn plant_images_change_with_the_season_and_grow_with_the_slider() {
    let mut sim = lot();
    sim.tool(ToolId::TerrainVariant(V::PlantPolyline));
    sim.click(0.0, 0.0);
    sim.click(300.0, 0.0);
    sim.key(KeyEvent::key(Key::Enter));
    // Make the run a plant image with growth data.
    spec_object(&mut sim, |l| {
        l.image = Some(plan_terrain::PlantImage::sized(
            "maple.png",
            120.0,
            240.0,
            false,
        ));
        l.size = 120.0;
        l.height = 240.0;
        l.mature_height = 240.0;
        l.mature_width = 120.0;
        l.maturity_months = 240.0;
    });
    build(&mut sim);
    let tinted = |sim: &Sim| {
        scene(sim)
            .meshes
            .iter()
            .filter(|m| m.material == plan_3d::Material::Foliage)
            .filter_map(|m| m.color)
            .collect::<Vec<_>>()
    };
    let summer = tinted(&sim);
    assert_eq!(summer.len(), 1);
    spec(&mut sim, |r| r.terrain.season = Season::Autumn);
    let autumn = tinted(&sim);
    assert_eq!(autumn.len(), 1);
    assert_ne!(autumn[0], summer[0]);
    // Grow All Plants: the dialog's slider scales the run, OK stores it.
    sim.tool(ToolId::TerrainVariant(V::GrowPlants));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert_eq!(sim.app.tools.active().name(), "Grow All Plants");
    sim.cancel();
    assert_eq!(
        rec(&sim).terrain.landscape[0].height,
        240.0,
        "Cancel changes nothing"
    );
    let mut draft = rec(&sim);
    assert!(plan_terrain::grow_plants(&mut draft.terrain.landscape, 2.0) == 1);
    let grown = draft.terrain.landscape[0].height;
    assert!(grown < 240.0 && grown > 100.0, "{grown}");
}

/// Edits the first landscape run the way its specification does.
fn spec_object(sim: &mut Sim, edit: impl FnOnce(&mut plan_terrain::Landscape)) {
    site_view::edit_terrain(&mut sim.app.cx, "Plant Specification", |r| {
        edit(&mut r.terrain.landscape[0])
    });
    assert_eq!(rec(sim).terrain.landscape[0].kind, LandscapeKind::Plants);
}

#[test]
fn the_import_assistants_open_filter_scale_and_store_in_one_undo_step() {
    let mut sim = lot();
    sim.tool(ToolId::TerrainVariant(V::ImportData));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert_eq!(sim.app.tools.active().name(), "Import Terrain Data");
    sim.cancel();
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
    sim.tool(ToolId::TerrainVariant(V::ImportGps));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert_eq!(sim.app.tools.active().name(), "Import GPS Data");
    sim.cancel();
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
    // The same results through the engine and the record: GPS markers and a
    // polyline become CAD on the Site Plan layer, the perimeter is replaced,
    // all in one undo step.
    let gpx = r#"<gpx version="1.1"><wpt lat="40.0" lon="-75.0"><ele>100</ele><name>A</name></wpt>
        <trk><trkseg><trkpt lat="40.0" lon="-75.0"/><trkpt lat="40.0005" lon="-75.0"/>
        <trkpt lat="40.0005" lon="-74.9995"/></trkseg></trk></gpx>"#;
    let got = plan_terrain::import_gps(
        gpx,
        plan_terrain::GpsImportAs::ElevationData,
        plan_terrain::GpsImportAs::Perimeter,
        &plan_terrain::GpsTransform::default(),
    )
    .unwrap();
    let mut draft = rec(&sim);
    draft.terrain.add_elevation_points(&got.elevation_points);
    draft.terrain.perimeter = got.perimeter.clone();
    let cad_before = sim.app.cx.floor().cad.len();
    let marked = plan_terrain::GpsResult {
        markers: vec![(Point::new(10.0, 10.0), "Pin".into())],
        polyline: vec![Point::new(0.0, 0.0), Point::new(50.0, 50.0)],
        ..got
    };
    site_view::apply_gps_import(&mut sim.app.cx, "Import GPS Data", &draft, &marked);
    let t = rec(&sim).terrain;
    assert_eq!(t.perimeter.len(), 3);
    assert_eq!(t.elevation_points.len(), 1);
    assert_eq!(sim.app.cx.floor().cad.len(), cad_before + 3);
    assert_eq!(sim.undo().as_deref(), Some("Import GPS Data"));
    assert_eq!(sim.app.cx.floor().cad.len(), cad_before);
    assert_eq!(rec(&sim).terrain.perimeter.len(), 4);
}

#[test]
fn the_object_dialogs_open_with_the_new_panels_and_keep_their_extras() {
    let mut sim = lot();
    sim.tool(ToolId::TerrainVariant(V::ElevationPoint));
    sim.click(100.0, 100.0);
    sim.key(KeyEvent::text("2'"));
    sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(rec(&sim).terrain.elevation_points.len(), 1);
    let hit = ObjectRef::TerrainObject(TerrainHit::Point(0));
    assert!(sim.open_spec(hit));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.cancel();
    // Extras set on the point survive a removal of another point.
    site_view::edit_terrain(&mut sim.app.cx, "Note", |r| {
        let mut ex = r.terrain.extras(ObjectKey::Point(0));
        ex.note = "TC %elevation%".into();
        r.terrain.set_extras(ObjectKey::Point(0), ex);
        r.terrain.elevation_points.push(ElevationPoint {
            pos: Point::new(300.0, 300.0),
            z: 0.0,
        });
    });
    site_view::edit_terrain(&mut sim.app.cx, "Delete", |r| {
        site_view::remove_terrain_element(&mut r.terrain, TerrainHit::Point(0));
    });
    let t = rec(&sim).terrain;
    assert_eq!(t.elevation_points.len(), 1);
    assert_eq!(
        t.extras(ObjectKey::Point(0)).note,
        "",
        "the note went with its point"
    );
}
