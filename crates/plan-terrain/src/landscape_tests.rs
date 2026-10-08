use plan_core::geometry::{point_in_polygon, polygon_area};
use plan_core::Point;

use crate::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A 100' x 80' lot rising 10' from west to east.
fn sloped() -> Terrain {
    let mut t = Terrain::default();
    for (x, y, z) in [
        (0.0, 0.0, 0.0),
        (0.0, 960.0, 0.0),
        (1200.0, 0.0, 120.0),
        (1200.0, 960.0, 120.0),
    ] {
        t.elevation_points.push(ElevationPoint { pos: pt(x, y), z });
    }
    t
}

#[test]
fn a_break_line_changes_the_surface_and_survives_smoothing() {
    let plain = build_terrain(&sloped());
    let mid = elevation_at(&plain, pt(600.0, 480.0)).unwrap();
    assert!((mid - 60.0).abs() < 1.0, "{mid}");

    let mut t = sloped();
    t.breaks.push(TerrainBreak {
        points: vec![pt(600.0, 0.0), pt(600.0, 960.0)],
        z: 120.0,
        ..TerrainBreak::default()
    });
    let broken = build_terrain(&t);
    let on = elevation_at(&broken, pt(600.0, 480.0)).unwrap();
    assert!((on - 120.0).abs() < 1.0, "on the break: {on}");
    assert!(
        on - mid > 50.0,
        "the break lifted the surface by {}",
        on - mid
    );
    // The data points keep their own heights.
    let corner = elevation_at(&broken, pt(0.0, 0.0)).unwrap();
    assert!(corner.abs() < 1.0, "{corner}");

    // Smoothing leaves the vertices on the break alone.
    t.smoothing = 6;
    let smooth = build_terrain(&t);
    for v in smooth
        .vertices
        .iter()
        .filter(|v| (v[0] - 600.0).abs() < 0.5)
    {
        assert!((v[1] - 120.0).abs() < 0.5, "break vertex moved to {}", v[1]);
    }
    assert!(smooth.vertices.iter().any(|v| (v[0] - 600.0).abs() < 0.5));
}

fn wall_heights_match_the_ground(w: &TerrainWall, surface: &TerrainSurface) -> (usize, usize) {
    let mut t = Terrain::default();
    t.walls.push(w.clone());
    let meshes = wall_meshes(&t, Some(surface));
    assert_eq!(meshes.len(), 1);
    let (mut tops, mut bottoms) = (0, 0);
    for v in &meshes[0].vertices {
        let plan = pt(f64::from(v.position[0]), -f64::from(v.position[2]));
        let g = elevation_at(surface, plan).expect("wall stays on the lot");
        let rel = f64::from(v.position[1]) - g;
        // The edges sit half a thickness off the centerline, so allow the slope there.
        if (rel - w.height).abs() < 2.0 {
            tops += 1;
        } else if (rel + w.depth).abs() < 2.0 {
            bottoms += 1;
        } else {
            panic!("vertex {rel}\" off the ground is neither top nor bottom");
        }
    }
    (tops, bottoms)
}

#[test]
fn a_retaining_wall_top_follows_the_terrain() {
    let surface = build_terrain(&sloped());
    let wall = TerrainWall::new(
        WallKind::Wall,
        vec![pt(100.0, 100.0), pt(1000.0, 100.0)],
        false,
    );
    let (tops, bottoms) = wall_heights_match_the_ground(&wall, &surface);
    assert!(
        tops >= 8 && tops == bottoms,
        "{tops} tops, {bottoms} bottoms"
    );

    // The top climbs with the slope: east end as much higher as the ground.
    let mut t = Terrain::default();
    t.walls.push(wall.clone());
    let m = &wall_meshes(&t, Some(&surface))[0];
    let top_at = |x: f32| {
        m.vertices
            .iter()
            .filter(|v| (v.position[0] - x).abs() < 1.0)
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max)
    };
    let g = |x: f64| elevation_at(&surface, pt(x, 100.0)).unwrap() as f32;
    assert!(g(1000.0) - g(100.0) > 50.0, "the lot rises to the east");
    for x in [100.0, 1000.0] {
        assert!((top_at(x as f32) - g(x) - 36.0).abs() < 1.5, "top at x={x}");
    }

    // Curbs use the same rule with their own sizes; a curved wall is its arc.
    let curb = TerrainWall::new(
        WallKind::Curb,
        arc_polyline(pt(100.0, 300.0), pt(900.0, 300.0), 120.0, 16),
        true,
    );
    assert_eq!((curb.height, curb.thickness), (6.0, 6.0));
    let (tops, bottoms) = wall_heights_match_the_ground(&curb, &surface);
    assert!(tops > 0 && tops == bottoms);
}

#[test]
fn walls_pick_a_3d_material_by_name() {
    let mut t = Terrain::default();
    for name in ["Concrete", "Stone", "Brick"] {
        let mut w = TerrainWall::new(WallKind::Wall, vec![pt(0.0, 0.0), pt(100.0, 0.0)], false);
        w.material = name.into();
        t.walls.push(w);
    }
    let mats: Vec<_> = wall_meshes(&t, None).iter().map(|m| m.material).collect();
    use plan_3d::Material::*;
    assert_eq!(mats, vec![Concrete, Stone, Brick]);
}

#[test]
fn stepping_stones_are_one_per_spacing_of_path() {
    let path = vec![pt(0.0, 0.0), pt(150.0, 0.0), pt(150.0, 150.0)];
    assert_eq!(path_length(&path), 300.0);
    let stones = stepping_stones(&path, 30.0);
    assert_eq!(stones.len(), 10);
    // Centered in their cells: the first is half a spacing along.
    assert!((stones[0].0.x - 15.0).abs() < 1e-9 && stones[0].0.y == 0.0);
    assert!((stones[9].0.y - 135.0).abs() < 1e-9);
    // A path shorter than the spacing still gets one.
    assert_eq!(
        stepping_stones(&[pt(0.0, 0.0), pt(10.0, 0.0)], 30.0).len(),
        1
    );
    assert!(stepping_stones(&[pt(0.0, 0.0)], 30.0).is_empty());

    let mut o = Landscape::new(LandscapeKind::SteppingStones, ShapeKind::Polyline, path);
    o.spacing = 60.0;
    assert_eq!(o.stones().len(), 5);
}

#[test]
fn plants_and_sprinkler_heads_run_from_end_to_end() {
    let path = vec![pt(0.0, 0.0), pt(360.0, 0.0)];
    let mut plants = Landscape::new(LandscapeKind::Plants, ShapeKind::Polyline, path.clone());
    plants.spacing = 36.0;
    let pos = plants.plant_positions();
    assert_eq!(pos.len(), 11);
    assert_eq!((pos[0], pos[10]), (pt(0.0, 0.0), pt(360.0, 0.0)));

    let mut sprinklers = Landscape::new(LandscapeKind::Sprinklers, ShapeKind::Polyline, path);
    sprinklers.spacing = 120.0;
    let heads = sprinklers.heads();
    assert_eq!(heads.len(), 4);
    // The spray faces the left of the path (plan +y).
    assert!((heads[0].1 - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn a_kidney_is_a_notched_blob_between_its_ends() {
    let (a, b, c) = (pt(0.0, 0.0), pt(200.0, 0.0), pt(100.0, 60.0));
    let k = kidney_outline(a, b, c).unwrap();
    let area = polygon_area(&k).abs();
    let ellipse = std::f64::consts::PI * 100.0 * 60.0;
    assert!(
        area < ellipse && area > ellipse * 0.5,
        "{area} vs {ellipse}"
    );
    // The long axis is the two clicks; the width is on the clicked side.
    let hi = k.iter().map(|p| p.y).fold(f64::MIN, f64::max);
    assert!(
        (hi - 60.0).abs() < 1.0,
        "the click sets the width on its side: {hi}"
    );
    assert!(k.iter().all(|p| p.x >= -1.0 && p.x <= 201.0));
    assert!(point_in_polygon(pt(100.0, 30.0), &k));
    // The notch faces away from the click: the middle of the other side is
    // nearer the axis than the parts either side of it.
    let low = |x: f64| {
        k.iter()
            .filter(|p| (p.x - x).abs() < 8.0)
            .map(|p| p.y)
            .fold(f64::MAX, f64::min)
    };
    assert!(
        low(100.0) > low(55.0) && low(100.0) > low(145.0),
        "{} {}",
        low(100.0),
        low(55.0)
    );
    assert!(kidney_outline(a, a, c).is_none());
    assert!(kidney_outline(a, b, pt(100.0, 0.0)).is_none());
}

#[test]
fn rectangles_and_splines_close_cleanly() {
    let r = rectangle_outline(pt(10.0, 20.0), pt(110.0, 80.0));
    assert_eq!(polygon_area(&r).abs(), 6000.0);
    let s = closed_spline(&[
        pt(0.0, 0.0),
        pt(100.0, 0.0),
        pt(100.0, 100.0),
        pt(0.0, 100.0),
    ]);
    assert!(s.len() > 4 && polygon_area(&s).abs() > 8000.0);
    let o = open_spline(&[pt(0.0, 0.0), pt(100.0, 50.0), pt(200.0, 0.0)]);
    assert_eq!((o[0], *o.last().unwrap()), (pt(0.0, 0.0), pt(200.0, 0.0)));
}

fn finite(m: &plan_3d::Mesh) -> bool {
    m.vertices
        .iter()
        .all(|v| v.position.iter().all(|c| c.is_finite()))
}

#[test]
fn every_object_kind_makes_meshes_draped_on_the_ground() {
    let surface = build_terrain(&sloped());
    let ring = vec![
        pt(200.0, 200.0),
        pt(500.0, 200.0),
        pt(500.0, 400.0),
        pt(200.0, 400.0),
    ];
    let path = vec![pt(100.0, 600.0), pt(700.0, 600.0)];
    let mut t = Terrain::default();
    t.features.push(Feature {
        kind: FeatureKind::Rectangular,
        polygon: ring.clone(),
        material: "Stone".into(),
        height: 6.0,
        ..Feature::default()
    });
    let kinds = [
        (LandscapeKind::GardenBed, ring.clone()),
        (LandscapeKind::GrassRegion, ring.clone()),
        (LandscapeKind::WaterFeature, ring.clone()),
        (LandscapeKind::SteppingStones, path.clone()),
        (LandscapeKind::Plants, path.clone()),
        (LandscapeKind::Sprinklers, path.clone()),
    ];
    for (k, pts) in kinds {
        let mut one = t.clone();
        one.landscape
            .push(Landscape::new(k, ShapeKind::Polyline, pts));
        let n_feature = landscape_meshes(&t, Some(&surface)).len();
        let meshes = landscape_meshes(&one, Some(&surface));
        assert!(meshes.len() > n_feature, "{k:?} made no mesh");
        assert!(
            meshes.iter().all(|m| !m.indices.is_empty() && finite(m)),
            "{k:?}"
        );
    }
    // Without a built surface everything sits on flat ground.
    assert!(!landscape_meshes(&t, None).is_empty());
}

#[test]
fn grass_and_beds_lie_on_the_ground_and_water_sits_below_grade() {
    let surface = build_terrain(&sloped());
    let ring = vec![
        pt(200.0, 200.0),
        pt(500.0, 200.0),
        pt(500.0, 400.0),
        pt(200.0, 400.0),
    ];
    let mut t = Terrain::default();
    t.landscape.push(Landscape::new(
        LandscapeKind::GardenBed,
        ShapeKind::Polyline,
        ring.clone(),
    ));
    t.landscape[0].edging = false;
    let bed = &landscape_meshes(&t, Some(&surface))[0];
    for v in &bed.vertices {
        let g = elevation_at(
            &surface,
            pt(f64::from(v.position[0]), -f64::from(v.position[2])),
        )
        .unwrap();
        assert!(
            (f64::from(v.position[1]) - g - 3.0).abs() < 0.2,
            "mulch 3\" deep"
        );
    }

    let mut t = Terrain::default();
    t.landscape.push(Landscape::new(
        LandscapeKind::WaterFeature,
        ShapeKind::Polyline,
        ring,
    ));
    let rim = elevation_at(&surface, pt(200.0, 200.0)).unwrap();
    let water = landscape_meshes(&t, Some(&surface))
        .into_iter()
        .find(|m| m.material == plan_3d::Material::WindowGlass)
        .unwrap();
    let level = f64::from(water.vertices[0].position[1]);
    assert!(
        level < rim,
        "the water level {level} is below the grade {rim}"
    );
    assert!(water
        .vertices
        .iter()
        .all(|v| (f64::from(v.position[1]) - level).abs() < 0.01));
}

#[test]
fn plan_items_sit_on_the_chief_layers() {
    let ring = vec![
        pt(0.0, 0.0),
        pt(200.0, 0.0),
        pt(200.0, 100.0),
        pt(0.0, 100.0),
    ];
    let path = vec![pt(0.0, 300.0), pt(300.0, 300.0)];
    let mut t = Terrain::default();
    t.features.push(Feature {
        polygon: ring.clone(),
        material: "Concrete".into(),
        ..Feature::default()
    });
    t.features.push(Feature {
        kind: FeatureKind::Hole,
        polygon: ring.clone(),
        ..Feature::default()
    });
    t.landscape.push(Landscape::new(
        LandscapeKind::GardenBed,
        ShapeKind::Polyline,
        ring,
    ));
    t.landscape.push(Landscape::new(
        LandscapeKind::Plants,
        ShapeKind::Polyline,
        path.clone(),
    ));
    t.landscape.push(Landscape::new(
        LandscapeKind::Sprinklers,
        ShapeKind::Polyline,
        path,
    ));
    let items = landscape_plan(&t);
    let on = |layer: &str| items.iter().filter(|i| i.layer == layer).count();
    assert!(on("Terrain, Features") >= 2, "outline and label");
    assert!(
        on("Landscaping, Garden Beds") > 3,
        "outline, edging, hatch, label"
    );
    assert!(on("Plants") >= 2 * 9);
    assert!(on("Sprinklers") >= 2 * 3);
    // The hole is not part of the landscape drawing (plan_symbols draws it).
    let hole = plan_symbols(&t, &[]);
    assert_eq!(
        hole.iter()
            .filter(|s| matches!(
                s,
                Stroke::Polyline {
                    kind: StrokeKind::Feature,
                    ..
                }
            ))
            .count(),
        1
    );
    // A style layer moves the object.
    t.landscape[0].style.layer = "My Beds".into();
    assert!(landscape_plan(&t).iter().any(|i| i.layer == "My Beds"));
}

#[test]
fn hatching_is_clipped_to_the_outline() {
    let sq = vec![
        pt(0.0, 0.0),
        pt(120.0, 0.0),
        pt(120.0, 120.0),
        pt(0.0, 120.0),
    ];
    let h = hatch_segments(&sq, 12.0, 0.0);
    assert_eq!(h.len(), 9);
    assert!(h
        .iter()
        .all(|[a, b]| (a.y - b.y).abs() < 1e-9 && (a.dist(*b) - 120.0).abs() < 1e-9));
    let diag = hatch_segments(&sq, 12.0, std::f64::consts::FRAC_PI_4);
    assert!(diag.len() > 10);
    assert!(diag.iter().all(|[a, b]| {
        let m = Point::lerp(*a, *b, 0.5);
        point_in_polygon(m, &sq)
    }));
    // A concave outline gets two runs on a scanline through both arms.
    let u = vec![
        pt(0.0, 0.0),
        pt(90.0, 0.0),
        pt(90.0, 90.0),
        pt(60.0, 90.0),
        pt(60.0, 30.0),
        pt(30.0, 30.0),
        pt(30.0, 90.0),
        pt(0.0, 90.0),
    ];
    let runs = hatch_segments(&u, 10.0, 0.0)
        .iter()
        .filter(|[a, _]| (a.y - 60.0).abs() < 1e-9)
        .count();
    assert_eq!(runs, 2);
}

#[test]
fn old_files_without_the_new_fields_still_load() {
    let old = r#"{"perimeter":[{"x":0.0,"y":0.0},{"x":10.0,"y":0.0},{"x":10.0,"y":10.0}],
        "features":[{"kind":"Rectangular","polygon":[{"x":0.0,"y":0.0},{"x":5.0,"y":0.0},{"x":5.0,"y":5.0}],"material":"Stone"}]}"#;
    let t: Terrain = serde_json::from_str(old).unwrap();
    assert!(t.breaks.is_empty() && t.walls.is_empty() && t.landscape.is_empty());
    assert_eq!(t.features[0].material, "Stone");
    assert_eq!(t.features[0].height, 0.0);
    assert_eq!(t.features[0].style, ObjectStyle::default());

    let mut t = Terrain::default();
    t.walls.push(TerrainWall::default());
    t.breaks.push(TerrainBreak::default());
    t.landscape.push(Landscape::new(
        LandscapeKind::Sprinklers,
        ShapeKind::Spline,
        vec![pt(1.0, 2.0)],
    ));
    let back: Terrain = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
    assert_eq!(back, t);
}
