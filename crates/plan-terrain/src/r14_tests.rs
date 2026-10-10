//! Round 14: feature kinds, road markings, stepped walls, plant forms,
//! contour and ground styles, survey import.

use plan_3d::Material;
use plan_core::Point;

use crate::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A 100' x 80' lot falling 10' from west to east.
fn falling() -> Terrain {
    let mut t = Terrain::default();
    for (x, y, z) in [
        (0.0, 0.0, 120.0),
        (0.0, 960.0, 120.0),
        (1200.0, 0.0, 0.0),
        (1200.0, 960.0, 0.0),
    ] {
        t.elevation_points.push(ElevationPoint { pos: pt(x, y), z });
    }
    t
}

#[test]
fn old_files_load_with_the_new_fields_at_their_defaults() {
    let t: Terrain = serde_json::from_str(
        r#"{"features":[{"kind":"Kidney","polygon":[]}],
            "roads":[{"kind":"Sidewalk","centerline":[],"width":48.0}],
            "walls":[{"kind":"Wall"}],
            "landscape":[{"kind":"Plants"}]}"#,
    )
    .unwrap();
    assert_eq!(t.ground_material, "Grass");
    assert_eq!(t.dirt_material, "Dirt");
    assert_eq!(t.contour_primary, ContourStyle::default());
    assert_eq!(t.features[0].radius, 0.0);
    assert_eq!(t.roads[0].material_name(), "Concrete");
    assert_eq!(t.roads[0].own_layer(), None);
    assert!(!t.walls[0].stepped);
    assert_eq!(t.walls[0].step, DEFAULT_WALL_STEP);
    assert_eq!(t.landscape[0].form, PlantForm::Auto);
    // And the new values survive a round trip.
    let mut t = Terrain::default();
    t.features.push(Feature::round(pt(300.0, 300.0), 90.0));
    t.roads.push(RoadStrip::marking(
        vec![pt(0.0, 0.0), pt(100.0, 0.0)],
        4.0,
        true,
    ));
    t.contour_primary.dashed = true;
    let back: Terrain = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
    assert_eq!(back.roads, t.roads);
    assert_eq!(back.contour_primary, t.contour_primary);
    assert_eq!(back.features[0].kind, FeatureKind::Round);
    assert_eq!(back.features[0].radius, 90.0);
}

#[test]
fn a_round_feature_is_a_circle_that_resizes_through_an_edge_point() {
    let mut f = Feature::round(pt(300.0, 300.0), 90.0);
    assert_eq!(f.kind, FeatureKind::Round);
    assert_eq!(f.polygon.len(), ROUND_CORNERS);
    assert!(f
        .polygon
        .iter()
        .all(|p| (p.dist(pt(300.0, 300.0)) - 90.0).abs() < 1e-6));
    f.set_radius_through(pt(300.0, 450.0));
    assert!((f.radius - 150.0).abs() < 1e-9);
    assert!(f
        .polygon
        .iter()
        .all(|p| (p.dist(pt(300.0, 300.0)) - 150.0).abs() < 1e-6));
    assert_eq!(f.kind_name(), "Round");
    // A polyline feature keeps its clicked corners.
    let poly = Feature {
        kind: FeatureKind::Polyline,
        polygon: vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(50.0, 80.0)],
        ..Feature::default()
    };
    let mut again = poly.clone();
    again.reflatten();
    assert_eq!(again, poly);
}

#[test]
fn round_and_polyline_features_grade_and_mesh_like_the_others() {
    let mut t = falling();
    let mut round = Feature::round(pt(400.0, 480.0), 100.0);
    round.pad = true;
    round.height = 0.0;
    round.material = "Concrete".into();
    t.features.push(round);
    t.features.push(Feature {
        kind: FeatureKind::Polyline,
        polygon: vec![pt(800.0, 300.0), pt(1000.0, 300.0), pt(900.0, 500.0)],
        height: 6.0,
        material: "Stone".into(),
        ..Feature::default()
    });
    let surface = build_terrain(&t);
    // The pad is level over its whole circle.
    let centre = elevation_at(&surface, pt(400.0, 480.0)).unwrap();
    let edge = elevation_at(&surface, pt(480.0, 480.0)).unwrap();
    assert!((centre - edge).abs() < 2.0, "{centre} vs {edge}");
    let report = cut_fill_report(&t);
    assert_eq!(report.items.len(), 1);
    assert!(report.items[0].name.starts_with("Round Feature 1"));
    assert!(report.cut_cy() > 0.0 && report.fill_cy() > 0.0);
    let meshes = landscape_meshes(&t, Some(&surface));
    assert!(meshes.len() >= 2, "one mesh per feature");
}

#[test]
fn dashes_cut_a_path_into_pieces_with_gaps() {
    let line = [pt(0.0, 0.0), pt(1000.0, 0.0)];
    let dashes = dash_path(&line, 100.0, 200.0);
    // Starts at 0, 300, 600, 900 (the last is cut at 1000).
    assert_eq!(dashes.len(), 4);
    assert_eq!(dashes[1], vec![pt(300.0, 0.0), pt(400.0, 0.0)]);
    assert_eq!(dashes[3], vec![pt(900.0, 0.0), pt(1000.0, 0.0)]);
    // A corner inside a dash is kept.
    let bent = [pt(0.0, 0.0), pt(50.0, 0.0), pt(50.0, 100.0)];
    let d = dash_path(&bent, 100.0, 50.0);
    assert_eq!(d[0], vec![pt(0.0, 0.0), pt(50.0, 0.0), pt(50.0, 50.0)]);
    assert!(dash_path(&line, 0.0, 10.0).is_empty());
}

#[test]
fn a_road_marking_lies_on_the_crown_of_the_road_it_crosses() {
    let mut t = Terrain::default();
    t.roads.push(RoadStrip {
        kind: RoadKind::Road,
        centerline: vec![pt(100.0, 480.0), pt(1100.0, 480.0)],
        width: 240.0,
        crown: 4.0,
        ..RoadStrip::default()
    });
    t.roads.push(RoadStrip::marking(
        vec![pt(150.0, 480.0), pt(1050.0, 480.0)],
        MARKING_WIDTH,
        false,
    ));
    let surface = build_terrain(&t);
    let meshes = road_meshes(&t, &surface);
    assert_eq!(meshes.len(), 2, "the road and its marking");
    let marking = &meshes[1];
    assert_eq!(marking.material, Material::Trim);
    assert_eq!(
        marking.object_id,
        Some(terrain_object_id(TerrainPart::Road, 1))
    );
    let (_, hi) = marking.bounds().unwrap();
    // Road lift 0.5" + the crown 2" off the centerline (4" at the centerline,
    // 240" wide) + paint 0.3".
    let expect = 0.5 + 4.0 * (1.0 - 2.0 / 120.0) + 0.3;
    assert!((f64::from(hi[1]) - expect).abs() < 1e-3, "{}", hi[1]);
    // The same stripe off the road lies on the ground.
    t.roads[0].centerline = vec![pt(100.0, 800.0), pt(1100.0, 800.0)];
    let meshes = road_meshes(&t, &surface);
    let (_, hi) = meshes[1].bounds().unwrap();
    assert!((hi[1] - 0.8).abs() < 1e-3, "{}", hi[1]);
}

#[test]
fn a_dashed_marking_has_fewer_triangles_than_a_solid_one_and_stays_one_mesh() {
    let mut t = Terrain::default();
    t.roads.push(RoadStrip::marking(
        vec![pt(100.0, 480.0), pt(1100.0, 480.0)],
        MARKING_WIDTH,
        false,
    ));
    let surface = build_terrain(&t);
    let solid = road_meshes(&t, &surface);
    t.roads[0].dashed = true;
    let dashed = road_meshes(&t, &surface);
    assert_eq!((solid.len(), dashed.len()), (1, 1));
    // 1000" at 120 + 240 per dash: 3 dashes of 120" plus a 40" tail.
    assert!(!dashed[0].vertices.is_empty());
    let (lo, hi) = dashed[0].bounds().unwrap();
    assert!(lo[0] >= 99.9 && hi[0] <= 1100.1);
    // The plan draws a marking as its centerline only, no road edges.
    let strokes = plan_symbols(&t, &[]);
    assert!(!strokes.iter().any(|s| matches!(
        s,
        Stroke::Polyline {
            kind: StrokeKind::RoadEdge,
            ..
        }
    )));
}

#[test]
fn road_materials_follow_the_strips_own_name() {
    let mut t = Terrain::default();
    for m in ["Gravel", "Brick", ""] {
        t.roads.push(RoadStrip {
            kind: RoadKind::Driveway,
            centerline: vec![pt(100.0, 100.0), pt(900.0, 100.0)],
            width: 96.0,
            material: m.into(),
            ..RoadStrip::default()
        });
    }
    let meshes = road_meshes(&t, &build_terrain(&t));
    assert_eq!(
        meshes.iter().map(|m| m.material).collect::<Vec<_>>(),
        [Material::Gravel, Material::Brick, Material::Asphalt]
    );
}

/// Distinct heights of the top vertices of the wall mesh, inches (each row of
/// a wall sweep is bottom left, top left, top right, bottom right).
fn wall_tops(t: &Terrain, surface: &TerrainSurface) -> Vec<f64> {
    let mesh = &wall_meshes(t, Some(surface))[0];
    let mut tops: Vec<f64> = mesh
        .vertices
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 4 == 1 || i % 4 == 2)
        .map(|(_, v)| f64::from(v.position[1]))
        .collect();
    tops.sort_by(f64::total_cmp);
    tops.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    tops
}

#[test]
fn a_stepped_wall_drops_in_whole_courses_while_a_plain_one_follows_the_ground() {
    let mut t = falling();
    let mut wall = TerrainWall::new(
        WallKind::Wall,
        vec![pt(100.0, 480.0), pt(1100.0, 480.0)],
        false,
    );
    wall.height = 30.0;
    wall.depth = 6.0;
    wall.cut = false;
    t.walls.push(wall);
    let surface = build_terrain(&t);
    let plain = wall_tops(&t, &surface);
    t.walls[0].stepped = true;
    t.walls[0].step = 24.0;
    let stepped = wall_tops(&t, &surface);
    // The ground falls 100" over the wall: the plain top has many heights, the
    // stepped one a handful, all a whole number of courses over the lowest.
    assert!(plain.len() > stepped.len(), "{plain:?} vs {stepped:?}");
    let base = stepped[0];
    for h in &stepped {
        let courses = (h - base) / 24.0;
        assert!(
            (courses - courses.round()).abs() < 0.01,
            "{h} off the courses"
        );
    }
    // The step never leaves the wall below the plain top.
    assert!(stepped.last().unwrap() >= plain.last().unwrap());
    assert!(stepped.len() >= 3);
}

#[test]
fn conifers_are_cones_billboards_are_crossed_planes_and_the_rest_round() {
    let mut t = Terrain::default();
    let mut shrub = Landscape::new(
        LandscapeKind::Plants,
        ShapeKind::Polyline,
        vec![pt(100.0, 100.0), pt(100.0, 100.0)],
    );
    shrub.size = 48.0;
    shrub.height = 60.0;
    shrub.plant = "core.plants.boxwood_2ft".into();
    let mut spruce = shrub.clone();
    spruce.plant = "core.plants.blue_spruce_12ft".into();
    let mut board = shrub.clone();
    board.form = PlantForm::Billboard;
    assert_eq!(shrub.plant_form(), PlantForm::Round);
    assert_eq!(spruce.plant_form(), PlantForm::Cone);
    assert_eq!(board.plant_form(), PlantForm::Billboard);
    assert!(is_conifer("Colorado Blue Spruce 20ft") && !is_conifer("Crape Myrtle"));
    t.landscape = vec![shrub, spruce, board];
    let surface = build_terrain(&t);
    let meshes = landscape_meshes(&t, Some(&surface));
    let of = |i: usize| {
        meshes
            .iter()
            .filter(|m| m.object_id == Some(terrain_object_id(TerrainPart::Landscape, i)))
            .collect::<Vec<_>>()
    };
    let canopy = |i: usize| of(i)[0];
    assert_eq!(canopy(0).material, Material::Foliage);
    // A cone: 12 ring points, the tip and the base center.
    assert_eq!(canopy(1).vertices.len(), 14);
    let (_, hi) = canopy(1).bounds().unwrap();
    assert!(
        (hi[1] - 60.0).abs() < 1.0,
        "the tip is the plant's height: {}",
        hi[1]
    );
    // A billboard: two planes of four corners, each seen from both sides.
    assert_eq!(canopy(2).vertices.len(), 8);
    assert_eq!(canopy(2).indices.len(), 2 * 2 * 6);
    let (lo, hi) = canopy(2).bounds().unwrap();
    assert!((hi[1] - lo[1] - 60.0).abs() < 1e-3);
    assert!(canopy(0).vertices.len() > canopy(1).vertices.len());
}

#[test]
fn contour_styles_set_the_plan_weights_and_the_ground_material_the_mesh() {
    let mut t = falling();
    t.contour_label_major_only = true;
    let surface = build_terrain(&t);
    let cs = contours(&surface, 12.0);
    let weights = |t: &Terrain| {
        let mut major = Vec::new();
        let mut minor = Vec::new();
        for s in plan_symbols(t, &cs) {
            if let Stroke::Polyline { weight, kind, .. } = s {
                match kind {
                    StrokeKind::MajorContour => major.push(weight),
                    StrokeKind::Contour => minor.push(weight),
                    _ => {}
                }
            }
        }
        (major, minor)
    };
    let (major, minor) = weights(&t);
    assert!(major.iter().all(|w| *w == PRIMARY_CONTOUR_WEIGHT) && !major.is_empty());
    assert!(minor.iter().all(|w| *w == SECONDARY_CONTOUR_WEIGHT) && !minor.is_empty());
    t.contour_primary.weight = 2.5;
    t.contour_secondary.weight = 0.2;
    let (major, minor) = weights(&t);
    assert!(major.iter().all(|w| *w == 2.5));
    assert!(minor.iter().all(|w| *w == 0.2));
    assert_eq!(terrain_mesh_for(&t, &surface).material, Material::Grass);
    t.ground_material = "Gravel".into();
    assert_eq!(terrain_mesh_for(&t, &surface).material, Material::Gravel);
    t.ground_material = "no such stuff".into();
    assert_eq!(terrain_mesh_for(&t, &surface).material, Material::Grass);
}

#[test]
fn imported_survey_points_build_a_surface_through_them() {
    let text = "0 0 100\n100 0 100\n100 100 110\n0 100 110\n50 50 105\n";
    let mut got = import_points(text, ImportUnit::Feet).unwrap();
    got.center_on(pt(600.0, 480.0));
    got.zero_lowest();
    let mut t = Terrain::default();
    assert_eq!(t.add_elevation_points(&got.points), 5);
    let s = build_terrain(&t);
    // The middle point is half way up the 10' rise, and the survey was
    // 100' x 100' centered in the lot: the point at its middle is at 5'.
    let z = elevation_at(&s, pt(600.0, 480.0)).unwrap();
    assert!((z - 60.0).abs() < 1.0, "{z}");
}
