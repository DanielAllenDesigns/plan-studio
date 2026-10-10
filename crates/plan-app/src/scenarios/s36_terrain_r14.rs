//! Scenario 36: terrain, round 14. Every terrain object is placed with its
//! tool, edited (specification dialog, nudge, vertex handles), seen in the 3D
//! scene and undone: polyline and round features, road markings on a crowned
//! road, stepped retaining walls, plants built as cones from the Plant
//! Chooser, the Terrain Specification's Materials and Contours, survey
//! import, the Cut and Fill Report, and a specification for each of the
//! elevation points, regions, modifiers, holes and perimeter.

use super::{draw_shell, shape_colors, Sim};
use crate::editor::site_view::{self, load_terrain, TerrainHit};
use crate::editor::ObjectRef;
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::tools::terrain::TerrainVariant as V;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::{self, Key};
use plan_core::geometry::Point;
use plan_terrain::{
    elevation_at, import_points, FeatureKind, ImportUnit, LandscapeKind, PlantForm, RoadKind,
    ROUND_CORNERS,
};

const W: f64 = 480.0;
const H: f64 = 360.0;

/// A house with a terrain perimeter around it.
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

/// Clicks the points of a polyline or polygon tool and ends it with Enter.
fn draw(sim: &mut Sim, v: V, pts: &[(f64, f64)]) {
    sim.tool(ToolId::TerrainVariant(v));
    for (x, y) in pts {
        sim.click(*x, *y);
    }
    sim.key(KeyEvent::key(Key::Enter));
}

/// Types a value into the tool's inline field (empty takes the default).
fn typed(sim: &mut Sim, text: &str) {
    if !text.is_empty() {
        sim.key(KeyEvent::text(text));
    }
    sim.key(KeyEvent::key(Key::Enter));
}

fn scene(sim: &Sim) -> plan_3d::Scene {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
}

fn build(sim: &mut Sim) {
    sim.tool(ToolId::TerrainVariant(V::Build));
    sim.click(0.0, 0.0);
}

/// A lot that falls 10' from west to east.
fn slope(sim: &mut Sim) {
    site_view::edit_terrain(&mut sim.app.cx, "Survey", |r| {
        for (x, y, z) in [
            (-300.0, -300.0, 120.0),
            (-300.0, 660.0, 120.0),
            (780.0, -300.0, 0.0),
            (780.0, 660.0, 0.0),
        ] {
            r.terrain
                .elevation_points
                .push(plan_terrain::ElevationPoint {
                    pos: Point::new(x, y),
                    z,
                });
        }
    });
}

#[test]
fn polyline_and_round_features_are_placed_resized_and_undone() {
    let mut sim = lot();
    let steps = sim.app.cx.undo_label().map(String::from);
    draw(
        &mut sim,
        V::PolylineFeature,
        &[(40.0, 40.0), (200.0, 40.0), (200.0, 120.0), (60.0, 160.0)],
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Polyline Feature"));
    sim.tool(ToolId::TerrainVariant(V::RoundFeature));
    sim.click(600.0, 100.0);
    sim.click(660.0, 100.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Round Feature"));
    let t = rec(&sim).terrain;
    assert_eq!(t.features.len(), 2);
    assert_eq!(t.features[0].kind, FeatureKind::Polyline);
    assert_eq!(t.features[0].polygon.len(), 4);
    let round = &t.features[1];
    assert_eq!(round.kind, FeatureKind::Round);
    assert_eq!(
        (round.center, round.radius),
        (Point::new(600.0, 100.0), 60.0)
    );
    // Both are graded pads that own a height and a material.
    assert!(t.features.iter().all(|f| f.pad && !f.material.is_empty()));

    // The handles of a round feature resize it: any vertex sets the radius.
    let hit = TerrainHit::Feature(1);
    assert_eq!(site_view::hit_points(&t, hit).len(), ROUND_CORNERS);
    site_view::edit_terrain(&mut sim.app.cx, "Reshape Terrain Element", |r| {
        site_view::move_terrain_vertex(&mut r.terrain, hit, 0, Point::new(600.0 + 90.0, 100.0));
    });
    let round = rec(&sim).terrain.features[1].clone();
    assert!((round.radius - 90.0).abs() < 1e-9);
    assert!(round
        .polygon
        .iter()
        .all(|p| (p.dist(round.center) - 90.0).abs() < 1e-6));
    // A move carries the center along.
    site_view::edit_terrain(&mut sim.app.cx, "Move Terrain Element", |r| {
        site_view::move_terrain_element(&mut r.terrain, hit, Point::new(10.0, 0.0));
    });
    assert_eq!(
        rec(&sim).terrain.features[1].center,
        Point::new(610.0, 100.0)
    );

    // Both draw in 3D after Build Terrain: one mesh each, with their ids.
    build(&mut sim);
    let ids: Vec<u64> = scene(&sim)
        .meshes
        .iter()
        .filter_map(|m| m.object_id)
        .filter_map(plan_terrain::terrain_object_of)
        .filter(|(part, _)| *part == plan_terrain::TerrainPart::Feature)
        .map(|(_, i)| i as u64)
        .collect();
    assert!(ids.contains(&0) && ids.contains(&1), "{ids:?}");

    // Undo takes the objects away in reverse order.
    let mut labels = Vec::new();
    while let Some(l) = sim.undo() {
        labels.push(l);
        if sim.app.cx.undo_label().map(String::from) == steps {
            break;
        }
    }
    assert!(labels.contains(&"Round Feature".to_string()), "{labels:?}");
    assert!(labels.contains(&"Polyline Feature".to_string()));
    assert!(rec(&sim).terrain.features.is_empty());
}

#[test]
fn road_markings_lie_on_the_ground_and_on_the_crown_of_a_road() {
    let mut sim = lot();
    // A 20' road along y = 500 with the default crown.
    draw(&mut sim, V::Road, &[(-250.0, 500.0), (700.0, 500.0)]);
    typed(&mut sim, "20'");
    let road = rec(&sim).terrain.roads[0].clone();
    assert_eq!((road.kind, road.width), (RoadKind::Road, 240.0));
    assert!(road.crown > 0.0);
    // A center stripe along it and a stripe across the lawn, both 4" wide.
    draw(&mut sim, V::RoadMarking, &[(-200.0, 500.0), (650.0, 500.0)]);
    typed(&mut sim, "");
    draw(&mut sim, V::RoadMarking, &[(0.0, 250.0), (400.0, 250.0)]);
    typed(&mut sim, "");
    let t = rec(&sim).terrain;
    assert_eq!(t.roads.len(), 3);
    assert!(t.roads[1..]
        .iter()
        .all(|r| r.kind == RoadKind::Marking && r.width == plan_terrain::MARKING_WIDTH));
    // The first stripe is dashed in the specification.
    site_view::edit_terrain(&mut sim.app.cx, "Road Marking Specification", |r| {
        r.terrain.roads[1].dashed = true;
    });
    build(&mut sim);
    let sc = scene(&sim);
    let of = |i: usize| {
        sc.meshes
            .iter()
            .filter(|m| {
                m.object_id
                    == Some(plan_terrain::terrain_object_id(
                        plan_terrain::TerrainPart::Road,
                        i,
                    ))
            })
            .collect::<Vec<_>>()
    };
    let top = |i: usize| of(i)[0].bounds().unwrap().1[1];
    // Painted white, on top of the crown (road lift 0.5" + crown + 0.3").
    assert_eq!(of(1)[0].material, plan_3d::Material::Trim);
    let crown = f64::from(top(1));
    assert!(
        crown > f64::from(top(0)) - 0.5 && crown >= road.crown,
        "stripe {crown} vs road {}",
        top(0)
    );
    // The stripe on the lawn hugs the (flat) ground: 0.5" + 0.3".
    assert!((f64::from(top(2)) - 0.8).abs() < 0.05, "{}", top(2));
    // The plan draws a marking as a line, in traffic yellow, and the road edges.
    let shapes = sim.plan_shapes();
    assert!(!shapes.is_empty());
}

#[test]
fn stepped_retaining_walls_drop_in_courses_in_the_3d_scene() {
    let mut sim = lot();
    slope(&mut sim);
    draw(
        &mut sim,
        V::StraightWall,
        &[(-200.0, 450.0), (700.0, 450.0)],
    );
    let w = rec(&sim).terrain.walls[0].clone();
    assert!(
        !w.stepped,
        "terrain walls follow the ground until stepping is chosen"
    );
    site_view::edit_terrain(&mut sim.app.cx, "Terrain Wall Specification", |r| {
        r.terrain.walls[0].stepped = true;
    });
    build(&mut sim);
    let tops = |sim: &Sim| {
        let sc = scene(sim);
        let wall = sc
            .meshes
            .iter()
            .find(|m| {
                m.object_id
                    == Some(plan_terrain::terrain_object_id(
                        plan_terrain::TerrainPart::Wall,
                        0,
                    ))
            })
            .expect("the wall's mesh");
        let mut tops: Vec<f64> = wall
            .vertices
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 4 == 1 || i % 4 == 2)
            .map(|(_, v)| f64::from(v.position[1]))
            .collect();
        tops.sort_by(f64::total_cmp);
        tops.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        tops
    };
    let stepped = tops(&sim);
    site_view::edit_terrain(&mut sim.app.cx, "Terrain Wall Specification", |r| {
        r.terrain.walls[0].stepped = false;
    });
    let plain = tops(&sim);
    assert!(
        plain.len() > stepped.len(),
        "the plain top follows the ground ({}), the stepped top has {} levels",
        plain.len(),
        stepped.len()
    );
    // The levels are whole courses apart.
    for h in &stepped {
        let n = (h - stepped[0]) / w.step;
        assert!((n - n.round()).abs() < 0.01, "{h}");
    }
}

#[test]
fn a_plant_run_chosen_from_the_library_is_built_as_cones_in_3d() {
    let mut sim = lot();
    draw(&mut sim, V::PlantPolyline, &[(40.0, 450.0), (440.0, 450.0)]);
    let run = rec(&sim).terrain.landscape[0].clone();
    assert_eq!(run.kind, LandscapeKind::Plants);
    assert_eq!(run.plant_form(), PlantForm::Round, "boxwood is a shrub");
    // The chooser lists the library by category.
    let cats = crate::tools::terrain::plant_categories();
    assert!(cats.iter().any(|c| c == "Trees > Evergreen"), "{cats:?}");
    let pine = crate::tools::terrain::plants_in("Trees > Evergreen", "pine")
        .into_iter()
        .next()
        .expect("a pine in the Evergreen category");
    site_view::edit_terrain(&mut sim.app.cx, "Plant Specification", |r| {
        crate::tools::terrain::apply_plant(&mut r.terrain.landscape[0], pine);
    });
    build(&mut sim);
    let run = rec(&sim).terrain.landscape[0].clone();
    assert_eq!(run.plant_form(), PlantForm::Cone);
    let n = run.plant_positions().len();
    let sc = scene(&sim);
    let canopy = sc
        .meshes
        .iter()
        .find(|m| {
            m.material == plan_3d::Material::Foliage
                && m.object_id
                    == Some(plan_terrain::terrain_object_id(
                        plan_terrain::TerrainPart::Landscape,
                        0,
                    ))
        })
        .expect("the canopies");
    // A cone is 14 vertices (12 around the base, the tip and the base center).
    assert_eq!(canopy.vertices.len(), 14 * n);
    // A billboard is two crossed planes.
    site_view::edit_terrain(&mut sim.app.cx, "Plant Specification", |r| {
        r.terrain.landscape[0].form = PlantForm::Billboard;
    });
    let sc = scene(&sim);
    let board = sc
        .meshes
        .iter()
        .find(|m| m.material == plan_3d::Material::Foliage)
        .unwrap();
    assert_eq!(board.vertices.len(), 8 * n);
}

#[test]
fn every_terrain_object_opens_its_own_specification_and_ok_is_one_undo_step() {
    let mut sim = lot();
    // One of everything, well apart.
    sim.tool(ToolId::TerrainVariant(V::ElevationPoint));
    sim.click(-250.0, -250.0);
    typed(&mut sim, "24");
    draw(
        &mut sim,
        V::ElevationLine,
        &[(-250.0, -200.0), (-100.0, -200.0)],
    );
    typed(&mut sim, "12");
    draw(
        &mut sim,
        V::ElevationRegion,
        &[(-250.0, -150.0), (-100.0, -150.0), (-100.0, -80.0)],
    );
    typed(&mut sim, "30");
    draw(
        &mut sim,
        V::Hill,
        &[(500.0, -250.0), (700.0, -250.0), (700.0, -150.0)],
    );
    typed(&mut sim, "");
    draw(
        &mut sim,
        V::Flat,
        &[(500.0, -100.0), (700.0, -100.0), (700.0, 0.0)],
    );
    draw(&mut sim, V::Break, &[(500.0, 50.0), (700.0, 50.0)]);
    typed(&mut sim, "6");
    draw(&mut sim, V::StraightWall, &[(500.0, 100.0), (700.0, 100.0)]);
    draw(&mut sim, V::StraightCurb, &[(500.0, 150.0), (700.0, 150.0)]);
    draw(
        &mut sim,
        V::PolylineFeature,
        &[(500.0, 200.0), (700.0, 200.0), (700.0, 260.0)],
    );
    draw(
        &mut sim,
        V::Hole,
        &[(500.0, 300.0), (700.0, 300.0), (700.0, 360.0)],
    );
    draw(&mut sim, V::Road, &[(-250.0, 560.0), (300.0, 560.0)]);
    typed(&mut sim, "8'");
    draw(&mut sim, V::RoadMarking, &[(-250.0, 640.0), (300.0, 640.0)]);
    typed(&mut sim, "");
    draw(
        &mut sim,
        V::BedPolyline,
        &[(-250.0, 100.0), (-100.0, 100.0), (-100.0, 200.0)],
    );
    draw(
        &mut sim,
        V::PlantPolyline,
        &[(-250.0, 250.0), (-100.0, 250.0)],
    );
    draw(
        &mut sim,
        V::SprinklerPolyline,
        &[(400.0, 600.0), (700.0, 600.0)],
    );
    draw(
        &mut sim,
        V::StonePolyline,
        &[(-250.0, 350.0), (-100.0, 350.0)],
    );
    draw(
        &mut sim,
        V::WaterPolyline,
        &[(-250.0, 400.0), (-100.0, 400.0), (-100.0, 480.0)],
    );
    draw(
        &mut sim,
        V::GrassPolyline,
        &[(400.0, 400.0), (700.0, 400.0), (700.0, 500.0)],
    );
    let t = rec(&sim).terrain;
    let hits = site_view::all_hits(&t);
    assert!(hits.len() >= 18, "{} elements", hits.len());

    sim.tool(ToolId::TerrainVariant(V::ElevationPoint));
    let mut opened = 0;
    for hit in hits {
        let t = rec(&sim).terrain;
        let Some(obj) = site_view::object_at(&t, hit) else {
            panic!("{hit:?} has no specification of its own");
        };
        let at = site_view::hit_points(&t, hit)[0];
        assert_eq!(
            site_view::hit_terrain(&t, at, sim.app.cx.pick_tol()),
            Some(hit),
            "{} is picked at its first vertex",
            obj.title()
        );
        let before = sim.app.cx.undo_label().map(String::from);
        sim.double_click(at.x, at.y);
        // The dialog blocks the canvas.
        let n = rec(&sim).terrain.elevation_points.len();
        sim.click(at.x + 3.0, at.y + 3.0);
        assert_eq!(
            rec(&sim).terrain.elevation_points.len(),
            n,
            "{}",
            obj.title()
        );
        sim.ok();
        assert_eq!(
            sim.app.cx.undo_label(),
            Some(obj.title()),
            "{hit:?}: was {before:?}"
        );
        opened += 1;
    }
    assert!(opened >= 18);

    // The perimeter opens the Terrain Specification.
    let before = sim.app.cx.undo_label().map(String::from);
    sim.double_click(-300.0, 100.0);
    sim.ok();
    assert_eq!(sim.app.cx.undo_label(), Some("Terrain Specification"));
    assert_ne!(sim.app.cx.undo_label().map(String::from), before);
}

#[test]
fn the_perimeter_is_edited_by_its_corner_handles_and_by_nudging() {
    let mut sim = lot();
    let p0 = rec(&sim).terrain.perimeter.clone();
    assert_eq!(p0.len(), 4);
    // Select Objects: the perimeter is the Terrain object, its corners handles.
    sim.tool(ToolId::Select);
    sim.app
        .cx
        .selection
        .set(ObjectRef::TerrainObject(TerrainHit::Perimeter));
    let handles = crate::editor::handles::handles_for(&sim.app.cx, 1.0);
    assert!(handles.len() >= 4, "{} handles", handles.len());
    let corner = p0[2];
    sim.drag((corner.x, corner.y), (corner.x + 60.0, corner.y + 40.0));
    let p1 = rec(&sim).terrain.perimeter.clone();
    assert!(
        p1[2].dist(Point::new(corner.x + 60.0, corner.y + 40.0)) < 1e-6,
        "{p1:?}"
    );
    assert_eq!(&p1[..2], &p0[..2]);
    assert_eq!(sim.app.cx.undo_label(), Some("Reshape Terrain Element"));
    sim.undo();
    assert_eq!(rec(&sim).terrain.perimeter, p0);
}

#[test]
fn materials_and_contour_styles_reach_the_plan_and_the_3d_surface() {
    let mut sim = lot();
    slope(&mut sim);
    build(&mut sim);
    let ground = |sim: &Sim| {
        let n = site_view::terrain_view(&sim.app.cx.project)
            .and_then(|v| v.surface.as_ref().map(|s| s.vertices.len()))
            .expect("a built surface");
        scene(sim)
            .meshes
            .iter()
            .find(|m| m.object_id.is_none() && m.vertices.len() == n)
            .map(|m| m.material)
    };
    assert_eq!(ground(&sim), Some(plan_3d::Material::Grass));
    // The Terrain Specification's Materials and Contours pages store through apply_spec.
    let mut draft = rec(&sim);
    draft.terrain.ground_material = "Gravel".into();
    draft.terrain.contour_primary.color = Some([1, 2, 3]);
    draft.terrain.contour_primary.dashed = true;
    draft.terrain.contour_secondary.color = Some([4, 5, 6]);
    site_view::edit_terrain(&mut sim.app.cx, "Terrain Specification", |r| {
        r.apply_spec(&draft)
    });
    assert_eq!(ground(&sim), Some(plan_3d::Material::Gravel));
    let shapes = sim.plan_shapes();
    let has = |c: [u8; 3]| {
        shapes.iter().any(|s| {
            shape_colors(s)
                .iter()
                .any(|k| (k.r(), k.g(), k.b()) == (c[0], c[1], c[2]))
        })
    };
    assert!(has([1, 2, 3]), "primary contours in their own color");
    assert!(has([4, 5, 6]), "secondary contours in their own color");
    // One undo step restores the plan's colors and the grass.
    assert_eq!(sim.undo().as_deref(), Some("Terrain Specification"));
    assert_eq!(ground(&sim), Some(plan_3d::Material::Grass));
    let _ = egui::Color32::RED;
}

#[test]
fn survey_points_import_into_the_terrain_and_shape_the_surface() {
    let mut sim = lot();
    let dxf = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n2\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nPOINT\n10\n0.0\n20\n0.0\n30\n100.0\n\
0\nPOINT\n10\n90.0\n20\n0.0\n30\n110.0\n\
0\nPOINT\n10\n90.0\n20\n70.0\n30\n110.0\n\
0\nPOINT\n10\n0.0\n20\n70.0\n30\n100.0\n\
0\nENDSEC\n0\nEOF\n";
    let mut got = import_points(dxf, ImportUnit::Auto).unwrap();
    assert_eq!(got.points.len(), 4);
    // Feet: the survey is 90' x 70', centered on the lot's middle.
    got.center_on(Point::new(240.0, 180.0));
    got.zero_lowest();
    site_view::edit_terrain(&mut sim.app.cx, "Terrain Specification", |r| {
        r.terrain.add_elevation_points(&got.points);
    });
    build(&mut sim);
    let surface = site_view::terrain_view(&sim.app.cx.project)
        .and_then(|v| v.surface.clone())
        .expect("a built surface");
    let west = elevation_at(&surface, Point::new(240.0 - 540.0 + 5.0, 180.0)).unwrap();
    let east = elevation_at(&surface, Point::new(240.0 + 540.0 - 5.0, 180.0)).unwrap();
    assert!(
        east - west > 60.0,
        "the survey rises toward the east: {west} -> {east}"
    );
    // The same points through GPX text.
    let gpx = "<gpx><wpt lat=\"33.0\" lon=\"-84.0\"><ele>100</ele></wpt>\
<wpt lat=\"33.0005\" lon=\"-84.0\"><ele>104</ele></wpt></gpx>";
    let g = import_points(gpx, ImportUnit::Auto).unwrap();
    assert_eq!(g.points.len(), 2);
    assert!((g.points[1].z - g.points[0].z - 4.0 / 0.0254).abs() < 1e-6);
}

#[test]
fn the_import_and_report_commands_open_their_dialogs_and_return_to_select() {
    let mut sim = lot();
    // A graded pad for the report.
    sim.tool(ToolId::TerrainVariant(V::RectFeature));
    sim.drag((100.0, 100.0), (260.0, 220.0));
    assert_eq!(rec(&sim).terrain.features.len(), 1);
    let steps = sim.app.cx.undo_label().map(String::from);
    for v in [V::CutFillReport, V::ImportData] {
        sim.tool(ToolId::TerrainVariant(v));
        assert_eq!(sim.app.tools.active().name(), v.name());
        sim.dialog_frame(false);
        sim.dialog_frame(false);
        assert!(sim.app.tools.active_id() == ToolId::TerrainVariant(v));
        // Click on the canvas: the dialog is up, nothing is drawn.
        sim.click(50.0, 50.0);
        sim.ok();
        assert_eq!(
            sim.app.tools.active_id(),
            ToolId::Select,
            "{} goes back to Select",
            v.name()
        );
        // Neither stores anything when nothing was imported.
        assert_eq!(sim.app.cx.undo_label().map(String::from), steps, "{v:?}");
    }
    // Cancel returns to Select too.
    sim.tool(ToolId::TerrainVariant(V::ImportData));
    sim.dialog_frame(false);
    sim.cancel();
    assert_eq!(sim.app.tools.active_id(), ToolId::Select);
    let report = plan_terrain::cut_fill_report(&rec(&sim).terrain);
    assert_eq!(report.items.len(), 1);
    assert!(report.to_csv().lines().count() == 3);
}
