//! Cut and fill, wall cuts, contour labels, editable outlines, site symbols.

use plan_core::geometry::{point_in_polygon, segment_intersection};
use plan_core::Point;

use crate::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)]
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

const WALL: [(f64, f64); 2] = [(100.0, 480.0), (1100.0, 480.0)];

fn walled(retain: f64, cut: bool) -> Terrain {
    let mut t = sloped();
    let mut w = TerrainWall::new(
        WallKind::Wall,
        vec![pt(WALL[0].0, WALL[0].1), pt(WALL[1].0, WALL[1].1)],
        false,
    );
    w.retain = retain;
    w.cut = cut;
    t.walls.push(w);
    t
}

/// How many contour segments cross the wall's centerline.
fn crossings(surface: &TerrainSurface) -> usize {
    let (a, b) = (pt(WALL[0].0, WALL[0].1), pt(WALL[1].0, WALL[1].1));
    contours(surface, 12.0)
        .iter()
        .flat_map(|c| c.polylines.iter())
        .flat_map(|line| line.windows(2))
        .filter(|s| segment_intersection(s[0], s[1], a, b).is_some())
        .count()
}

#[test]
fn a_terrain_wall_cuts_the_surface_and_the_grades_differ_across_it() {
    let t = walled(24.0, true);
    let s = build_terrain(&t);
    let plain = build_terrain(&sloped());
    let base = elevation_at(&plain, pt(600.0, 490.0)).unwrap();
    // No surface under the wall itself.
    assert_eq!(elevation_at(&s, pt(600.0, 480.0)), None);
    // North of the wall (the left of the path) keeps its grade...
    let left = elevation_at(&s, pt(600.0, 490.0)).unwrap();
    assert!((left - base).abs() < 1.0, "retained side {left} vs {base}");
    // ...south of it (the right) is cut 24" at the wall and slopes back at 1:4.
    let right = elevation_at(&s, pt(600.0, 470.0)).unwrap();
    assert!(
        (left - right - 22.5).abs() < 1.5,
        "grade step across the wall: {}",
        left - right
    );
    let near = elevation_at(&s, pt(600.0, 300.0)).unwrap();
    let far = elevation_at(&plain, pt(600.0, 300.0)).unwrap();
    assert!(
        (far - near).abs() < 1.0,
        "reach of 96 inches has faded by 180 inches out: {near} vs {far}"
    );
    // Past the end of the wall the two sides join again.
    let end_l = elevation_at(&s, pt(1190.0, 500.0)).unwrap();
    let end_r = elevation_at(&s, pt(1190.0, 460.0)).unwrap();
    assert!((end_l - end_r).abs() < 8.0, "{}", end_l - end_r);
}

#[test]
fn contours_stop_at_a_terrain_wall() {
    let free = build_terrain(&sloped());
    assert!(
        crossings(&free) > 0,
        "without the wall the contours run on across the line"
    );
    for retain in [0.0, 24.0] {
        let t = walled(retain, true);
        let s = build_terrain(&t);
        assert_eq!(crossings(&s), 0, "retain {retain}");
        // And they are still there on both sides.
        let cs = contours(&s, 12.0);
        let pts: Vec<Point> = cs.iter().flat_map(|c| c.polylines.concat()).collect();
        assert!(pts
            .iter()
            .any(|p| p.y > 490.0 && p.x > 200.0 && p.x < 1000.0));
        assert!(pts
            .iter()
            .any(|p| p.y < 470.0 && p.x > 200.0 && p.x < 1000.0));
    }
}

#[test]
fn a_wall_that_does_not_cut_leaves_the_surface_whole() {
    let s = build_terrain(&walled(24.0, false));
    assert!(elevation_at(&s, pt(600.0, 480.0)).is_some());
    let up = elevation_at(&s, pt(600.0, 490.0)).unwrap();
    let down = elevation_at(&s, pt(600.0, 470.0)).unwrap();
    assert!((up - down).abs() < 2.0, "no cut, no step");
}

#[test]
fn smoothing_keeps_the_faces_of_the_cut() {
    let mut t = walled(24.0, true);
    t.smoothing = 6;
    let s = build_terrain(&t);
    let left = elevation_at(&s, pt(600.0, 484.5)).unwrap();
    let right = elevation_at(&s, pt(600.0, 475.5)).unwrap();
    assert!(
        left - right > 20.0,
        "step after smoothing: {}",
        left - right
    );
}

// ----- pads: cut and fill -----

/// The pad of the task: 20' x 20', its top 24" below the mean ground of a
/// 1:4 slope (the ground rises 1 for every 4 across).
#[test]
fn a_twenty_foot_pad_cut_24_inches_into_a_one_in_four_slope() {
    let ground = |p: Point| 100.0 + (p.x - 600.0) / 4.0;
    let poly = rect(480.0, 360.0, 720.0, 600.0);
    let top = 100.0 - 24.0;
    let v = pad_volumes(&ground, &poly, top, 2.0, 900.0, 3.0);
    let cy = |cubic_inches: f64| cubic_inches / 46_656.0;
    // Under the pad: the ground is 24" above the top at the middle, from 6"
    // below it (west edge) to 54" above (east edge).
    let cut = cy(v.pad_cut);
    let fill = cy(v.pad_fill);
    assert!((cut - 30.0).abs() < 0.1, "pad cut {cut} cy");
    assert!(
        (fill - 17_280.0 / 46_656.0).abs() < 0.02,
        "pad fill {fill} cy"
    );
    // The sides add more: a cut slope uphill and fill downhill.
    assert!(cy(v.slope_cut) > 5.0, "side cut {}", cy(v.slope_cut));
    assert!(cy(v.slope_fill) > 0.0, "side fill {}", cy(v.slope_fill));
    // A steeper slope ratio needs less of both.
    let steep = pad_volumes(&ground, &poly, top, 1.0, 900.0, 3.0);
    assert!(steep.slope_cut < v.slope_cut);
}

#[test]
fn the_report_counts_a_raised_pad_in_cubic_yards() {
    let mut t = Terrain::default();
    t.features.push(Feature {
        kind: FeatureKind::Rectangular,
        polygon: rect(480.0, 360.0, 720.0, 600.0),
        height: 12.0,
        pad: true,
        material: "Concrete".into(),
        ..Feature::default()
    });
    let report = cut_fill_report(&t);
    assert_eq!(report.items.len(), 1);
    let item = &report.items[0];
    assert_eq!(item.source, PadSource::Feature(0));
    assert!(item.name.starts_with("Rectangular Feature 1"));
    assert!((item.area_sq_ft - 400.0).abs() < 1e-6);
    // 20' x 20' x 12" of fill under the pad.
    assert!(
        (item.pad_fill_cy - 14.815).abs() < 0.05,
        "{}",
        item.pad_fill_cy
    );
    assert_eq!(item.pad_cut_cy, 0.0);
    // The sides (1:2) fill the ring up to the pad as well.
    assert!(item.slope_fill_cy > 3.0 && item.slope_fill_cy < 12.0);
    assert!((report.fill_cy() - item.fill_cy()).abs() < 1e-9);
    assert!(report.net_cy() < 0.0, "soil to bring in");

    // Cutting in instead: the same volume as cut.
    t.features[0].height = -12.0;
    let cut = cut_fill_report(&t);
    assert!((cut.items[0].pad_cut_cy - 14.815).abs() < 0.05);
    assert!(cut.net_cy() > 0.0);
    // No pad, no report.
    t.features[0].pad = false;
    assert!(cut_fill_report(&t).is_empty());
}

#[test]
fn a_pad_feature_grades_the_built_surface() {
    let mut t = Terrain::default();
    t.features.push(Feature {
        polygon: rect(480.0, 360.0, 720.0, 600.0),
        height: 12.0,
        pad: true,
        ..Feature::default()
    });
    let s = build_terrain(&t);
    let at = |x: f64, y: f64| elevation_at(&s, pt(x, y)).unwrap();
    assert!((at(600.0, 480.0) - 12.0).abs() < 0.1, "flat top");
    assert!((at(500.0, 380.0) - 12.0).abs() < 0.1);
    // 1:2 sides: 10" out the pad is 5" lower; 24" out it meets the ground.
    assert!((at(470.0, 480.0) - 7.0).abs() < 1.0, "{}", at(470.0, 480.0));
    assert!(at(440.0, 480.0).abs() < 1.0);
    assert!(at(100.0, 100.0).abs() < 1e-6);
    // The cut/fill is part of the build: ground elsewhere is untouched.
    let plain = build_terrain(&Terrain::default());
    assert_eq!(
        plain.vertices.iter().filter(|v| v[1].abs() > 0.5).count(),
        0
    );
    assert!(s.vertices.iter().any(|v| v[1] > 11.0));
    // Smoothing leaves the pad flat.
    t.smoothing = 5;
    let smooth = build_terrain(&t);
    assert!((elevation_at(&smooth, pt(600.0, 480.0)).unwrap() - 12.0).abs() < 0.1);
}

#[test]
fn the_building_pad_sits_below_the_first_floor_by_the_terrain_to_first_floor_distance() {
    let mut t = sloped();
    t.subfloor_height_above_terrain = 6.0;
    t.building_pad = Some(BuildingPad {
        footprint: rect(500.0, 380.0, 700.0, 580.0),
        first_floor: Some(60.0),
        ..BuildingPad::default()
    });
    let s = build_terrain(&t);
    let under = elevation_at(&s, pt(600.0, 480.0)).unwrap();
    assert!((under - 54.0).abs() < 0.1, "first floor 60 less 6: {under}");
    let report = cut_fill_report(&t);
    assert_eq!(report.items[0].source, PadSource::Building);
    assert_eq!(report.items[0].name, "Building Pad");
    assert!((report.items[0].top - 54.0).abs() < 1e-9);
    // Turned off, the terrain is untouched.
    t.flatten_pad = false;
    assert!(cut_fill_report(&t).is_empty());
    let free = build_terrain(&t);
    assert!((elevation_at(&free, pt(600.0, 480.0)).unwrap() - 60.0).abs() < 2.0);
    // Without a first floor the pad levels at the mean ground.
    t.flatten_pad = true;
    t.building_pad.as_mut().unwrap().first_floor = None;
    let balanced = cut_fill_report(&t);
    let i = &balanced.items[0];
    assert!((i.pad_cut_cy - i.pad_fill_cy).abs() < 0.2 * i.pad_cut_cy.max(1.0));
}

// ----- contour labels -----

#[test]
fn major_contours_are_labeled_every_n_feet_and_read_upright() {
    let mut t = Terrain::default();
    t.modifiers.push(Modifier {
        kind: ModifierKind::Hill,
        polygon: rect(200.0, 150.0, 1000.0, 810.0),
        height: 150.0,
    });
    let surface = build_terrain(&t);
    let cs = contours_with(&surface, 12.0, 5);
    let majors: Vec<_> = cs.iter().filter(|c| c.major).collect();
    assert!(majors.len() >= 2, "levels 60 and 120");
    let strokes = plan_symbols(&t, &cs);
    let texts: Vec<(&str, f64)> = strokes
        .iter()
        .filter_map(|s| match s {
            Stroke::Text { text, angle, .. } => Some((text.as_str(), *angle)),
            _ => None,
        })
        .collect();
    // Default: one label per 40' of a major contour; none on the minor ones.
    let expected: usize = majors
        .iter()
        .flat_map(|c| c.polylines.iter())
        .map(|l| contour_label_spots(l, t.contour_label_spacing, 40.0).len())
        .sum();
    assert_eq!(texts.len(), expected);
    assert!(expected > majors.len(), "long loops carry several labels");
    assert!(texts.iter().all(|(t, _)| *t == "5'-0\"" || *t == "10'-0\""));
    let half_turn = std::f64::consts::FRAC_PI_2 + 1e-9;
    assert!(texts.iter().all(|(_, a)| a.abs() <= half_turn), "readable");
    // A longer interval gives fewer labels, and labeling every contour more.
    let mut sparse = t.clone();
    sparse.contour_label_spacing = 1200.0;
    let n = |t: &Terrain| {
        plan_symbols(t, &cs)
            .iter()
            .filter(|s| matches!(s, Stroke::Text { .. }))
            .count()
    };
    assert!(n(&sparse) < n(&t));
    let mut all = t.clone();
    all.contour_label_major_only = false;
    assert!(n(&all) > n(&t));
    // Labels sit on their line.
    for s in &strokes {
        if let Stroke::Text { at, .. } = s {
            let near = cs
                .iter()
                .filter(|c| c.major)
                .flat_map(|c| c.polylines.iter())
                .any(|l| {
                    l.windows(2)
                        .any(|w| plan_core::geometry::dist_to_segment(*at, w[0], w[1]) < 0.5)
                });
            assert!(near, "label off its contour at {at:?}");
        }
    }
}

#[test]
fn label_spots_follow_the_line_and_flip_to_stay_upright() {
    // A line running west: the text would be upside down, so it turns around.
    let line = [pt(1000.0, 0.0), pt(0.0, 0.0)];
    let spots = contour_label_spots(&line, 400.0, 40.0);
    assert_eq!(spots.len(), 2);
    assert!(spots.iter().all(|(_, a)| a.abs() < 1e-9));
    let up = contour_label_spots(&[pt(0.0, 0.0), pt(0.0, 500.0)], 0.0, 40.0);
    assert_eq!(up.len(), 1);
    assert!((up[0].1 - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
    // A stub too short for its text gets none.
    assert!(contour_label_spots(&[pt(0.0, 0.0), pt(10.0, 0.0)], 480.0, 40.0).is_empty());
}

// ----- editable outlines -----

#[test]
fn a_kidney_is_a_spline_through_control_points_that_can_be_edited() {
    let (a, b, c) = (pt(0.0, 0.0), pt(300.0, 0.0), pt(150.0, 100.0));
    let control = kidney_control_points(a, b, c).unwrap();
    assert!(control.len() >= 8);
    let outline = closed_spline(&control);
    assert_eq!(outline.len(), control.len() * 8);
    for (i, p) in control.iter().enumerate() {
        assert!(outline[i * 8].dist(*p) < 1e-9, "passes through control {i}");
    }
    // The spline stays near the old fixed blob.
    let blob = kidney_outline(a, b, c).unwrap();
    let (lo, hi) = (-1.0, 301.0);
    assert!(outline.iter().all(|p| p.x >= lo && p.x <= hi));
    assert!(blob.len() == 40 && outline.iter().any(|p| p.y > 50.0));

    // Round trip through a Landscape object: edit a control point, re-flatten.
    let mut bed = Landscape::new(LandscapeKind::GardenBed, ShapeKind::Kidney, outline.clone());
    bed.control = control.clone();
    let json = serde_json::to_string(&bed).unwrap();
    let mut back: Landscape = serde_json::from_str(&json).unwrap();
    assert_eq!(
        (back.control.len(), back.points.len()),
        (control.len(), outline.len())
    );
    assert!(back
        .control
        .iter()
        .zip(&control)
        .all(|(a, b)| a.dist(*b) < 1e-9));
    back.control[3] = back.control[3].add(pt(0.0, 40.0));
    back.reflatten();
    assert_eq!(back.points.len(), outline.len());
    assert!(back.points[3 * 8].dist(back.control[3]) < 1e-9);
    assert!(
        back.points[3 * 8].dist(outline[3 * 8]) > 39.0,
        "outline moved"
    );
    assert_ne!(back.points, outline);
    // A clicked polyline (no control points) is left alone by reflatten.
    let mut plain = Landscape::new(
        LandscapeKind::GardenBed,
        ShapeKind::Polyline,
        rect(0.0, 0.0, 10.0, 10.0),
    );
    let before = plain.points.clone();
    plain.reflatten();
    assert_eq!(plain.points, before);
}

#[test]
fn spline_tension_reshapes_an_elevation_spline() {
    let control = vec![
        pt(0.0, 0.0),
        pt(100.0, 80.0),
        pt(200.0, 0.0),
        pt(300.0, 80.0),
    ];
    let mut line = ElevationLine::spline(control.clone(), 36.0, 0.5, 8);
    assert_eq!(
        line.points,
        flatten_spline(&control, false, 8),
        "0.5 is Catmull-Rom"
    );
    let bulge = |l: &ElevationLine| {
        // How far the middle of the first span strays from its chord.
        let mid = l.points[4];
        plan_core::geometry::dist_to_segment(mid, control[0], control[1])
    };
    let normal = bulge(&line);
    line.tension = 0.0;
    line.reflatten(8);
    assert!(bulge(&line) < 1e-9, "tension 0 is straight segments");
    assert_eq!(line.points.len(), 3 * 8 + 1);
    line.tension = 1.0;
    line.reflatten(8);
    assert!(bulge(&line) > normal, "a loose spline swings wider");
    // The control points and tension survive a save.
    let back: ElevationLine = serde_json::from_str(&serde_json::to_string(&line).unwrap()).unwrap();
    assert_eq!(back, line);
    // Old files (points and z only) still load.
    let old: ElevationLine = serde_json::from_str("{\"points\":[],\"z\":12.0}").unwrap();
    assert_eq!((old.z, old.tension, old.control.len()), (12.0, 0.5, 0));
}

// ----- water, roads, progress -----

#[test]
fn water_ripples_stay_inside_the_outline() {
    let pond = kidney_outline(pt(0.0, 0.0), pt(300.0, 0.0), pt(150.0, 100.0)).unwrap();
    let rows = ripple_lines(&pond, 24.0, 2.0, 24.0);
    assert!(rows.len() >= 3, "{} ripple rows", rows.len());
    assert!(rows.iter().flatten().all(|p| point_in_polygon(*p, &pond)));
    assert!(rows.iter().all(|r| r.len() >= 2));
    // The rows really wave.
    let wavy = rows.iter().any(|r| {
        let (lo, hi) = r.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
        hi - lo > 2.0
    });
    assert!(wavy);

    // A water feature draws the ripple fill and its depth in plan.
    let mut t = Terrain::default();
    let mut water = Landscape::new(LandscapeKind::WaterFeature, ShapeKind::Spline, pond);
    water.depth = 30.0;
    t.landscape.push(water);
    let items = landscape_plan(&t);
    let waves = items
        .iter()
        .filter(|i| matches!(&i.shape, PlanShape::Polyline { closed: false, weight, .. } if *weight < 0.5))
        .count();
    assert!(waves >= 3);
    assert!(items.iter().any(
        |i| matches!(&i.shape, PlanShape::Text { text, .. } if text.contains("Depth 2'-6\""))
    ));
    // Asking for no fill takes the pattern away.
    t.landscape[0].style.fill = FillStyle::None;
    assert!(!landscape_plan(&t)
        .iter()
        .any(|i| matches!(&i.shape, PlanShape::Fill { .. })));
}

#[test]
fn a_crowned_road_is_higher_along_its_centerline() {
    let mut t = Terrain::default();
    t.roads.push(RoadStrip {
        kind: RoadKind::Road,
        centerline: vec![pt(100.0, 480.0), pt(1100.0, 480.0)],
        width: 240.0,
        curb: true,
        crown: 4.0,
        curb_height: 8.0,
    });
    let s = build_terrain(&t);
    let meshes = road_meshes(&t, &s);
    assert_eq!(meshes.len(), 2);
    let (lo, hi) = meshes[0].bounds().unwrap();
    assert!((lo[1] - 0.5).abs() < 1e-3, "edges stay on the ground");
    assert!((hi[1] - 4.5).abs() < 1e-3, "centerline crowned 4 inches");
    let (_, curb_top) = meshes[1].bounds().unwrap();
    assert!((curb_top[1] - 8.5).abs() < 1e-3, "8-inch curb");
    // Without a crown the strip is flat as before.
    t.roads[0].crown = 0.0;
    let flat = road_meshes(&t, &s);
    let (lo, hi) = flat[0].bounds().unwrap();
    assert_eq!(lo[1], hi[1]);
}

#[test]
fn build_terrain_reports_its_progress() {
    let mut stages = Vec::new();
    let s =
        build_terrain_with_progress(&walled(24.0, true), &mut |stage, f| stages.push((stage, f)));
    assert!(!s.triangles.is_empty());
    let names: Vec<BuildStage> = stages.iter().map(|s| s.0).collect();
    assert_eq!(
        names,
        [
            BuildStage::Sampling,
            BuildStage::Triangulating,
            BuildStage::Clipping,
            BuildStage::Smoothing,
            BuildStage::Done
        ]
    );
    assert!(stages.windows(2).all(|w| w[0].1 <= w[1].1));
    assert_eq!(stages.last().unwrap().1, 1.0);
    assert_eq!(BuildStage::Triangulating.label(), "Triangulating");
}

#[test]
fn subdivision_refines_the_sample_grid() {
    let coarse = build_terrain(&Terrain::default());
    let t = Terrain {
        subdivision: 2,
        ..Terrain::default()
    };
    let fine = build_terrain(&t);
    assert!(fine.grid.spacing < coarse.grid.spacing);
    assert!(fine.triangles.len() > coarse.triangles.len());
}

#[test]
fn contour_major_every_is_a_setting() {
    let s = build_terrain(&sloped());
    let majors = |n: u32| {
        contours_with(&s, 12.0, n)
            .iter()
            .filter(|c| c.major)
            .count()
    };
    assert!(majors(2) > majors(5));
    assert_eq!(
        contours_with(&s, 12.0, 0)
            .iter()
            .filter(|c| c.major)
            .count(),
        majors(5),
        "0 falls back to every fifth"
    );
}

// ----- site symbols -----

#[test]
fn the_north_angle_turns_a_compass_azimuth_into_a_plan_direction() {
    // North up the page: the sun due south shines from the bottom of the plan.
    assert_eq!(plan_azimuth(180.0, 0.0), 180.0);
    // Plan rotated so north points right (east on the page): south is left.
    assert_eq!(plan_azimuth(180.0, 90.0), 270.0);
    assert_eq!(plan_azimuth(350.0, 20.0), 10.0);
    assert!((true_azimuth(plan_azimuth(123.0, 77.0), 77.0) - 123.0).abs() < 1e-9);
    // North of a 90-degree plan is plan east.
    let v = north_vector(90.0);
    assert!((v.x - 1.0).abs() < 1e-9 && v.y.abs() < 1e-9);
    assert!((north_angle_toward(pt(0.0, 0.0), pt(10.0, 0.0)) - 90.0).abs() < 1e-9);
    assert!((north_angle_toward(pt(0.0, 0.0), pt(0.0, -5.0)) - 180.0).abs() < 1e-9);
    // Labels: a wall facing plan-up looks true north-west on a plan turned 45 degrees.
    let facing = azimuth_of_plan_vector(pt(0.0, 1.0), 45.0).unwrap();
    assert_eq!(facing_label(facing), "NW");
    assert_eq!(facing_label(0.0), "N");
    assert_eq!(facing_label(359.0), "N");
    assert_eq!(facing_label(181.0), "S");
    assert!(azimuth_of_plan_vector(Point::ZERO, 0.0).is_none());
}

#[test]
fn the_north_pointer_arrow_points_along_the_north_angle() {
    use plan_core::cad::CadItem;
    for angle in [0.0, 90.0, 215.0] {
        let items = north_pointer_items(pt(500.0, 500.0), 24.0, angle);
        assert!(matches!(items[0], CadItem::Circle { radius, .. } if radius == 24.0));
        let CadItem::Polyline { points, closed } = &items[1] else {
            panic!("the arrow head");
        };
        assert!(*closed);
        let tip = points[0];
        let toward = tip.sub(pt(500.0, 500.0));
        let n = north_vector(angle);
        assert!((toward.x - 24.0 * n.x).abs() < 1e-9 && (toward.y - 24.0 * n.y).abs() < 1e-9);
        assert!(items
            .iter()
            .any(|i| matches!(i, CadItem::Text { text, .. } if text == "N")));
    }
}

#[test]
fn a_scale_bar_has_ticks_and_length_labels() {
    use plan_core::cad::CadItem;
    let items = scale_bar_items(pt(0.0, 0.0), 0.0, 480.0, 4);
    let labels: Vec<&str> = items
        .iter()
        .filter_map(|i| match i {
            CadItem::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, ["0", "10'-0\"", "20'-0\"", "30'-0\"", "40'-0\""]);
    // The outline spans the length.
    let CadItem::Polyline { points, .. } = &items[0] else {
        panic!("outline")
    };
    assert!((points[1].x - 480.0).abs() < 1e-9);
    // Three inner ticks.
    let ticks = items
        .iter()
        .filter(|i| matches!(i, CadItem::Line { a, b } if (a.x - b.x).abs() < 1e-9 && a.y != b.y))
        .count();
    assert_eq!(ticks, 3);
}

// ----- old files -----

#[test]
fn files_without_the_new_fields_still_load() {
    let f: Feature = serde_json::from_str("{\"kind\":\"Rectangular\",\"polygon\":[]}").unwrap();
    assert!(!f.pad && f.slope_ratio == 2.0 && f.control.is_empty());
    let w: TerrainWall = serde_json::from_str("{\"points\":[]}").unwrap();
    assert!(w.cut && w.retain == 0.0);
    let r: RoadStrip =
        serde_json::from_str("{\"kind\":\"Road\",\"centerline\":[],\"width\":240.0,\"curb\":true}")
            .unwrap();
    assert!(r.curb && r.crown == 0.0 && r.curb_height == 6.0);
    let t: Terrain = serde_json::from_str("{\"smoothing\":2}").unwrap();
    assert!(t.building_pad.is_none() && t.flatten_pad && t.north_angle == 0.0);
    assert_eq!((t.contour_major_every, t.subdivision), (5, 1));
    assert!(t.contour_label_major_only && t.contour_label_spacing == 480.0);
}
