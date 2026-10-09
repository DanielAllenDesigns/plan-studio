//! Round 15: Terrain Specification parity (absolute elevation, skirt,
//! smoothing, triangle count, contour presentation), retaining walls, the two
//! import assistants, elevation data options, road geometry, plant images,
//! Grow Plants, garden bed distribution, labels and schedule categories.

use plan_3d::Mesh;
use plan_core::Point;

use crate::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn square(x: f64, y: f64, side: f64) -> Vec<Point> {
    vec![
        pt(x, y),
        pt(x + side, y),
        pt(x + side, y + side),
        pt(x, y + side),
    ]
}

/// A 100' x 80' lot falling 10' from west to east.
fn falling() -> Terrain {
    let mut t = Terrain::default();
    for (x, y, z) in [
        (0.0, 0.0, 120.0),
        (0.0, 960.0, 120.0),
        (1200.0, 0.0, 0.0),
        (1200.0, 960.0, 0.0),
        (600.0, 480.0, 60.0),
    ] {
        t.elevation_points.push(ElevationPoint { pos: pt(x, y), z });
    }
    t
}

fn z_at(t: &Terrain, p: Point) -> f64 {
    elevation_at(&build_terrain(t), p).expect("on the surface")
}

fn indices(ms: &[Mesh]) -> usize {
    ms.iter().map(|m| m.indices.len()).sum()
}

// ----- Terrain Specification -----

#[test]
fn old_files_load_with_the_round_15_fields_at_their_defaults() {
    let t: Terrain = serde_json::from_str(
        r#"{"features":[{"kind":"Rectangular"}],"roads":[{"kind":"Road"}],
            "walls":[{"kind":"Wall"}],"landscape":[{"kind":"Plants"}],
            "breaks":[{"z":12.0}],"elevation_lines":[{"z":6.0}]}"#,
    )
    .unwrap();
    assert_eq!(t.absolute_elevation, AbsoluteElevation::Automatic);
    assert_eq!(t.reference_point, None);
    assert!(!t.skirt.enabled && !t.hide_under_building);
    assert_eq!(t.smoothing_level, SmoothingLevel::Passes);
    assert_eq!(t.triangle_detail, TriangleDetail::Grid);
    assert_eq!(t.contour_offset, 0.0);
    assert_eq!(t.contour_label_units, LabelUnits::FeetInches);
    assert!(t.last_build.is_none());
    assert_eq!(t.features[0].thickness, 0.0);
    assert!(t.roads[0].outline.is_empty() && t.roads[0].flare_start.is_none());
    assert_eq!(t.landscape[0].image, None);
    assert!(!t.breaks[0].follow_ground);
    // And the new values survive a round trip.
    let mut t = Terrain::default();
    t.absolute_elevation = AbsoluteElevation::ReferencePoint;
    t.reference_point = Some(pt(10.0, 20.0));
    t.skirt.enabled = true;
    t.contour_offset = 6.0;
    t.roads.push(RoadStrip {
        flare_start: Some(24.0),
        ..RoadStrip::default()
    });
    t.landscape.push(Landscape {
        image: Some(PlantImage::default()),
        distribution: Some(Distribution::default()),
        ..Landscape::default()
    });
    let mut ex = t.extras(ObjectKey::Perimeter);
    ex.label.shown = true;
    t.set_extras(ObjectKey::Perimeter, ex);
    t.elevation_points.push(ElevationPoint {
        pos: pt(1.0, 1.0),
        z: 5.0,
    });
    let mut ex = t.extras(ObjectKey::Point(0));
    ex.note = "Top of curb".into();
    t.set_extras(ObjectKey::Point(0), ex);
    let back: Terrain = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
    assert_eq!(back, t);
}

#[test]
fn automatic_elevation_leaves_the_data_where_it_is() {
    let t = falling();
    assert!((z_at(&t, pt(0.0, 0.0)) - 120.0).abs() < 1e-6);
    assert_eq!(t.effective_subfloor_distance(), 6.0);
}

#[test]
fn retaining_the_surface_at_the_reference_point_moves_the_terrain_to_the_floor() {
    let mut t = falling();
    t.absolute_elevation = AbsoluteElevation::ReferencePoint;
    t.reference_point = Some(pt(0.0, 0.0));
    t.surface_offset = -6.0;
    t.floor_one_elevation = 0.0;
    // The surface at the reference point sits 6" under the Floor 1 subfloor.
    assert!((z_at(&t, pt(0.0, 0.0)) + 6.0).abs() < 1e-6);
    // The rest of the lot keeps its relief.
    assert!((z_at(&t, pt(1200.0, 0.0)) - (0.0 - 126.0)).abs() < 1e-6);
    assert_eq!(t.effective_subfloor_distance(), 6.0);
    // Without a placed point the middle of the lot is used.
    t.reference_point = None;
    assert_eq!(t.effective_reference_point(), Some(pt(600.0, 480.0)));
    assert!((z_at(&t, pt(600.0, 480.0)) + 6.0).abs() < 1.0);
}

#[test]
fn retaining_the_surface_at_contour_zero_puts_elevation_zero_at_the_offset() {
    let mut t = falling();
    t.absolute_elevation = AbsoluteElevation::ContourZero;
    t.surface_offset = -12.0;
    t.floor_one_elevation = 24.0;
    // Raw elevation 0 (the east side) lands at floor + offset.
    assert!((z_at(&t, pt(1200.0, 0.0)) - 12.0).abs() < 1e-6);
}

#[test]
fn the_skirt_hangs_from_the_edge_flat_or_following_the_terrain() {
    let mut t = falling();
    let surface = build_terrain(&t);
    assert!(skirt_mesh(&t, &surface).is_none());
    t.skirt.enabled = true;
    t.skirt.thickness = 36.0;
    let flat = skirt_mesh(&t, &surface).expect("a skirt");
    assert!(!flat.indices.is_empty());
    let lowest = surface
        .vertices
        .iter()
        .map(|v| v[1])
        .fold(f64::INFINITY, f64::min);
    let min_y = flat
        .vertices
        .iter()
        .map(|v| v.position[1])
        .fold(f32::INFINITY, f32::min);
    assert!((f64::from(min_y) - (lowest - 36.0)).abs() < 0.01);
    t.skirt.mode = SkirtMode::FollowTerrain;
    let follow = skirt_mesh(&t, &surface).expect("a skirt");
    let ys: Vec<f32> = follow.vertices.iter().map(|v| v.position[1]).collect();
    let (lo, hi) = (
        ys.iter().copied().fold(f32::INFINITY, f32::min),
        ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    );
    assert!(hi - lo > 100.0, "it follows the falling edge");
    t.skirt.material = "Stone".into();
    assert_eq!(
        skirt_mesh(&t, &surface).unwrap().material,
        plan_3d::Material::Stone
    );
}

#[test]
fn hide_terrain_intersected_by_building_cuts_the_footprint_out() {
    let mut t = falling();
    t.building_pad = Some(BuildingPad {
        footprint: square(400.0, 300.0, 240.0),
        ..BuildingPad::default()
    });
    t.flatten_pad = false;
    let inside = pt(520.0, 420.0);
    assert!(elevation_at(&build_terrain(&t), inside).is_some());
    t.hide_under_building = true;
    let surface = build_terrain(&t);
    assert!(elevation_at(&surface, inside).is_none());
    assert!(elevation_at(&surface, pt(100.0, 100.0)).is_some());
    // The contours stop at the footprint too.
    let lines = contours_with(&surface, 12.0, 5);
    assert!(lines
        .iter()
        .flat_map(|c| c.polylines.iter().flatten())
        .all(|p| !(p.x > 410.0 && p.x < 630.0 && p.y > 310.0 && p.y < 530.0)));
}

#[test]
fn smoothing_levels_map_to_passes_and_linear_skips_the_grid() {
    let mut t = falling();
    t.smoothing = 7;
    assert_eq!(t.effective_smoothing(), 7);
    for (level, passes) in [
        (SmoothingLevel::Linear, 0),
        (SmoothingLevel::Low, 1),
        (SmoothingLevel::Medium, 3),
        (SmoothingLevel::High, 6),
    ] {
        t.smoothing_level = level;
        assert_eq!(t.effective_smoothing(), passes, "{level:?}");
    }
    t.smoothing_level = SmoothingLevel::Passes;
    let grid = build_terrain(&t).triangles.len();
    t.smoothing_level = SmoothingLevel::Linear;
    let linear = build_terrain(&t);
    assert!(
        linear.triangles.len() < grid / 2,
        "{} vs {grid}",
        linear.triangles.len()
    );
    // Still a surface that covers the lot and keeps the data.
    assert!((elevation_at(&linear, pt(0.0, 0.0)).unwrap() - 120.0).abs() < 1e-6);
}

#[test]
fn the_triangle_count_sets_the_detail_of_the_surface() {
    let mut t = Terrain::default();
    let count = |t: &Terrain| build_terrain(t).triangles.len();
    t.triangle_detail = TriangleDetail::Low;
    let low = count(&t);
    t.triangle_detail = TriangleDetail::Medium;
    let medium = count(&t);
    t.triangle_detail = TriangleDetail::High;
    let high = count(&t);
    assert!(low < medium && medium < high, "{low} {medium} {high}");
    assert!((600..1600).contains(&low), "{low}");
    assert!((1400..3000).contains(&medium), "{medium}");
    assert!((3000..6000).contains(&high), "{high}");
    t.triangle_detail = TriangleDetail::Custom;
    t.custom_triangles = 500;
    let custom = count(&t);
    assert!((250..800).contains(&custom), "{custom}");
    assert!(t.estimated_triangles() > 300 && t.estimated_triangles() < 800);
    // A maximum triangle size works when no count is asked for.
    t.triangle_detail = TriangleDetail::Grid;
    t.max_triangle_size = 240.0;
    assert_eq!(t.effective_grid_spacing(), 240.0);
}

// ----- contours -----

fn ramp_surface() -> TerrainSurface {
    build_terrain(&falling())
}

#[test]
fn the_contour_offset_shifts_which_elevations_get_a_line() {
    let s = ramp_surface();
    let plain = contours_opts(
        &s,
        &ContourOptions {
            interval: 24.0,
            ..ContourOptions::default()
        },
    );
    assert!(plain.iter().all(|c| (c.z % 24.0).abs() < 1e-6));
    let shifted = contours_opts(
        &s,
        &ContourOptions {
            interval: 24.0,
            offset: 6.0,
            ..ContourOptions::default()
        },
    );
    assert!(!shifted.is_empty());
    assert!(
        shifted.iter().all(|c| ((c.z - 6.0) % 24.0).abs() < 1e-6),
        "{:?}",
        shifted.iter().map(|c| c.z).collect::<Vec<_>>()
    );
    assert!(!plain
        .iter()
        .any(|c| shifted.iter().any(|d| (c.z - d.z).abs() < 1e-6)));
}

#[test]
fn two_d_smoothing_rounds_the_corners_and_keeps_loops_closed() {
    let line = vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0)];
    let smooth = smooth_line(&line, 2);
    assert!(smooth.len() > line.len());
    assert_eq!(smooth.first(), line.first());
    assert_eq!(smooth.last(), line.last());
    // The corner is cut.
    assert!(smooth.iter().all(|p| p.dist(pt(100.0, 0.0)) > 1.0));
    let ring = vec![
        pt(0.0, 0.0),
        pt(100.0, 0.0),
        pt(100.0, 100.0),
        pt(0.0, 100.0),
        pt(0.0, 0.0),
    ];
    let s = smooth_line(&ring, 2);
    assert_eq!(s.first(), s.last());
    assert!(s.len() > ring.len());
    // Through the options.
    let mut t = falling();
    t.contour_smoothing = true;
    t.contour_smooth_passes = 2;
    let opts = t.contour_options(24.0);
    assert_eq!(opts.smooth_passes, 2);
    t.contour_smoothing = false;
    assert_eq!(t.contour_options(24.0).smooth_passes, 0);
}

#[test]
fn contour_labels_take_their_units_and_red_when_negative() {
    assert_eq!(LabelUnits::DecimalFeet.format(30.0), "2.5'");
    assert_eq!(LabelUnits::DecimalFeet.format(24.0), "2'");
    assert_eq!(LabelUnits::Inches.format(30.0), "30\"");
    assert_eq!(LabelUnits::Inches.format(-7.5), "-7.5\"");
    assert_eq!(
        LabelUnits::FeetInches.format(30.0),
        plan_core::units::fmt_ft_in_frac(30.0, 2)
    );
    let mut t = Terrain::default();
    t.elevation_points = vec![
        ElevationPoint {
            pos: pt(0.0, 0.0),
            z: -60.0,
        },
        ElevationPoint {
            pos: pt(1200.0, 960.0),
            z: 60.0,
        },
    ];
    t.contour_label_major_only = false;
    t.contour_label_spacing = 0.0;
    t.contour_label_units = LabelUnits::DecimalFeet;
    t.highlight_negative = true;
    let s = build_terrain(&t);
    let cs = contours_opts(&s, &t.contour_options(24.0));
    let strokes = plan_symbols(&t, &cs);
    let texts: Vec<(&str, bool, StrokeKind)> = strokes
        .iter()
        .filter_map(|s| match s {
            Stroke::Text {
                text,
                negative,
                kind,
                ..
            } => Some((text.as_str(), *negative, *kind)),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|(t, n, _)| t.starts_with('-') && *n),
        "{texts:?}"
    );
    assert!(texts.iter().all(|(t, n, _)| *n == t.starts_with('-')));
    assert!(texts.iter().all(|(t, _, _)| t.ends_with('\'')));
    assert!(texts.iter().any(|(_, _, k)| *k == StrokeKind::MajorContour));
    assert!(texts.iter().any(|(_, _, k)| *k == StrokeKind::Contour));
    t.highlight_negative = false;
    let strokes = plan_symbols(&t, &cs);
    assert!(strokes
        .iter()
        .all(|s| !matches!(s, Stroke::Text { negative: true, .. })));
}

#[test]
fn clear_terrain_removes_only_what_the_build_generated() {
    let mut t = falling();
    t.last_build = Some(BuildStats {
        triangles: 100,
        contour_levels: 9,
    });
    t.walls.push(TerrainWall::new(
        WallKind::Wall,
        vec![pt(0.0, 0.0), pt(100.0, 0.0)],
        false,
    ));
    let before = (t.perimeter.clone(), t.elevation_points.len(), t.walls.len());
    assert!(clear_generated_only(&mut t));
    assert!(t.last_build.is_none());
    assert_eq!(
        (t.perimeter.clone(), t.elevation_points.len(), t.walls.len()),
        before
    );
    assert!(!clear_generated_only(&mut t), "nothing left to clear");
}

// ----- retaining walls -----

#[test]
fn a_terrain_wall_is_five_feet_and_follows_the_ground_by_default() {
    let w = TerrainWall::new(WallKind::Wall, vec![pt(0.0, 0.0), pt(100.0, 0.0)], false);
    assert_eq!(w.height, 60.0);
    assert!(!w.stepped);
    let c = TerrainWall::new(WallKind::Curb, vec![pt(0.0, 0.0), pt(100.0, 0.0)], false);
    assert_eq!(c.height, 6.0);
}

#[test]
fn a_retaining_wall_is_a_break_plus_a_wall_sized_from_both_sides() {
    let t = falling();
    let surface = build_terrain(&t);
    // Drawn north: the west (high) side is on the left already.
    let up = retaining_wall(
        &t,
        &surface,
        vec![pt(600.0, 200.0), pt(600.0, 700.0)],
        false,
    )
    .unwrap();
    assert!(up.high > up.low);
    assert_eq!(up.wall.points[0], pt(600.0, 200.0));
    assert!((up.wall.retain - up.drop()).abs() < 1e-9);
    assert_eq!(up.wall.height, 0.0);
    assert_eq!(up.wall.depth, FOOTING);
    assert!(up.wall.cut && !up.wall.stepped);
    assert!(up.terrain_break.follow_ground);
    assert_eq!(up.terrain_break.points, up.wall.points);
    // Drawn south: the path is turned around so the high side stays on its left.
    let down = retaining_wall(
        &t,
        &surface,
        vec![pt(600.0, 700.0), pt(600.0, 200.0)],
        false,
    )
    .unwrap();
    assert_eq!(down.wall.points[0], pt(600.0, 200.0));
    assert!((down.drop() - up.drop()).abs() < 1e-6);
    // On flat ground it is a plain strip.
    let flat = Terrain::default();
    let fs = build_terrain(&flat);
    let strip =
        retaining_wall(&flat, &fs, vec![pt(100.0, 100.0), pt(500.0, 100.0)], false).unwrap();
    assert_eq!(strip.wall.retain, 0.0);
    assert_eq!(strip.drop(), 0.0);
    assert!(retaining_wall(&flat, &fs, vec![pt(1.0, 1.0)], false).is_none());
}

#[test]
fn a_retaining_wall_steps_the_built_ground_and_stands_between_the_sides() {
    let mut t = falling();
    let surface = build_terrain(&t);
    let rw = retaining_wall(
        &t,
        &surface,
        vec![pt(600.0, 100.0), pt(600.0, 860.0)],
        false,
    )
    .unwrap();
    let drop = rw.drop();
    assert!(drop > 2.0, "{drop}");
    t.breaks.push(rw.terrain_break);
    t.walls.push(rw.wall);
    let built = build_terrain(&t);
    // Just west of the wall the ground is higher than just east of it by about the wall's drop
    // plus the natural fall.
    let west = elevation_at(&built, pt(600.0 - 6.0, 480.0)).unwrap();
    let east = elevation_at(&built, pt(600.0 + 6.0, 480.0)).unwrap();
    assert!(west - east > drop * 0.7, "{west} {east} {drop}");
    // The wall mesh runs from the high ground down past the low one.
    let ms = wall_meshes(&t, Some(&built));
    assert_eq!(ms.len(), 1);
    let ys: Vec<f32> = ms[0].vertices.iter().map(|v| v.position[1]).collect();
    let (lo, hi) = (
        ys.iter().copied().fold(f32::INFINITY, f32::min),
        ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    );
    assert!(f64::from(hi - lo) > drop, "{lo} {hi}");
}

#[test]
fn a_break_transition_limits_how_far_its_elevation_reaches() {
    let mut t = Terrain::default();
    t.elevation_points.push(ElevationPoint {
        pos: pt(0.0, 0.0),
        z: 0.0,
    });
    t.elevation_points.push(ElevationPoint {
        pos: pt(1200.0, 960.0),
        z: 0.0,
    });
    t.breaks.push(TerrainBreak {
        points: vec![pt(600.0, 0.0), pt(600.0, 960.0)],
        z: 60.0,
        transition: 120.0,
        ..TerrainBreak::default()
    });
    let s = build_terrain(&t);
    assert!((elevation_at(&s, pt(600.0, 480.0)).unwrap() - 60.0).abs() < 1.0);
    assert!(elevation_at(&s, pt(100.0, 480.0)).unwrap().abs() < 1.0);
    t.breaks[0].transition = 0.0;
    let s = build_terrain(&t);
    assert!(
        elevation_at(&s, pt(100.0, 480.0)).unwrap() > 5.0,
        "no limit reaches the whole lot"
    );
}

// ----- elevation data options -----

#[test]
fn an_open_region_holds_only_its_outline_and_can_flatten_toward_the_edge() {
    let mut t = Terrain::default();
    t.elevation_points.push(ElevationPoint {
        pos: pt(600.0, 480.0),
        z: 0.0,
    });
    t.elevation_regions.push(ElevationRegion {
        polygon: square(400.0, 280.0, 400.0),
        z: 100.0,
    });
    let centre = pt(600.0, 480.0);
    let flat = z_at(&t, pt(500.0, 400.0));
    assert!(
        (flat - 100.0).abs() < 1.0,
        "a flat interior holds the elevation: {flat}"
    );
    let mut ex = t.extras(ObjectKey::Region(0));
    ex.interior_open = true;
    t.set_extras(ObjectKey::Region(0), ex.clone());
    let open_centre = z_at(&t, centre);
    assert!(
        open_centre < 60.0,
        "the interior follows the data: {open_centre}"
    );
    let edge = pt(430.0, 480.0);
    let plain = z_at(&t, edge);
    ex.tangent_to_edge = true;
    t.set_extras(ObjectKey::Region(0), ex);
    let tangent = z_at(&t, edge);
    assert!(tangent >= plain - 1e-6 && (100.0 - tangent) < (100.0 - plain) + 1e-6);
    assert!(tangent > 80.0, "{tangent}");
}

#[test]
fn extras_follow_their_object_and_reindex_when_one_is_removed() {
    let mut t = Terrain::default();
    for i in 0..3 {
        t.elevation_points.push(ElevationPoint {
            pos: pt(f64::from(i) * 10.0, 0.0),
            z: f64::from(i),
        });
        let mut ex = ObjectExtras::default();
        ex.note = format!("note {i}");
        t.set_extras(ObjectKey::Point(i as usize), ex);
    }
    assert_eq!(t.extras(ObjectKey::Point(1)).note, "note 1");
    // Remove point 1 the way the editor does.
    t.elevation_points.remove(1);
    t.forget_extras_of_removed(ObjectKey::Point(1));
    assert_eq!(t.extras(ObjectKey::Point(0)).note, "note 0");
    assert_eq!(t.extras(ObjectKey::Point(1)).note, "note 2");
    assert_eq!(t.extras(ObjectKey::Point(2)), ObjectExtras::default());
    // Extras of an object that does not exist are not stored.
    t.set_extras(
        ObjectKey::Point(9),
        ObjectExtras {
            note: "x".into(),
            ..ObjectExtras::default()
        },
    );
    assert!(t.side_extras.len() <= 2);
    // Objects that carry their extras keep them where they are.
    t.walls.push(TerrainWall::default());
    let mut ex = t.extras(ObjectKey::Wall(0));
    ex.info.supplier = "Acme Block".into();
    t.set_extras(ObjectKey::Wall(0), ex);
    assert_eq!(t.walls[0].extras.info.supplier, "Acme Block");
}

// ----- labels and schedules -----

#[test]
fn labels_are_custom_or_automatic_and_points_carry_a_note() {
    let mut t = falling();
    t.walls.push(TerrainWall::new(
        WallKind::Wall,
        vec![pt(100.0, 100.0), pt(500.0, 100.0)],
        false,
    ));
    assert!(label_spots(&t).is_empty());
    let mut ex = t.extras(ObjectKey::Wall(0));
    ex.label.shown = true;
    t.set_extras(ObjectKey::Wall(0), ex.clone());
    let spots = label_spots(&t);
    assert_eq!(spots.len(), 1);
    assert!(spots[0].text.starts_with("Terrain Wall"));
    assert_eq!(spots[0].at, pt(300.0, 100.0));
    ex.label.text = "Garden wall".into();
    ex.label.offset = pt(0.0, 12.0);
    t.set_extras(ObjectKey::Wall(0), ex);
    let spots = label_spots(&t);
    assert_eq!(spots[0].text, "Garden wall");
    assert_eq!(spots[0].at, pt(300.0, 112.0));
    // The elevation point note replaces the elevation macro.
    let mut ex = t.extras(ObjectKey::Point(0));
    ex.note = "TW %elevation%".into();
    ex.marker_radius = 5.0;
    t.set_extras(ObjectKey::Point(0), ex);
    let note = label_spots(&t).into_iter().find(|s| s.note).unwrap();
    assert_eq!(
        note.text,
        format!("TW {}", plan_core::units::fmt_ft_in_frac(120.0, 2))
    );
    assert!(note.at.x > t.elevation_points[0].pos.x);
    assert_eq!(label_strokes(&t).len(), 2);
    assert_eq!(LAYER_TERRAIN_LABELS, "Terrain Labels");
}

#[test]
fn schedule_categories_follow_the_manual_and_can_be_reassigned() {
    let mut t = Terrain::default();
    let line = vec![pt(0.0, 0.0), pt(240.0, 0.0)];
    for kind in [
        RoadKind::Road,
        RoadKind::Driveway,
        RoadKind::Sidewalk,
        RoadKind::Marking,
    ] {
        t.roads.push(RoadStrip {
            kind,
            centerline: line.clone(),
            width: 48.0,
            ..RoadStrip::default()
        });
    }
    t.roads.push(RoadStrip {
        kind: RoadKind::Median,
        outline: square(0.0, 0.0, 60.0),
        ..RoadStrip::default()
    });
    t.walls
        .push(TerrainWall::new(WallKind::Curb, line.clone(), false));
    t.features.push(Feature {
        polygon: square(0.0, 0.0, 100.0),
        ..Feature::default()
    });
    t.landscape.push(Landscape::new(
        LandscapeKind::GardenBed,
        ShapeKind::Polyline,
        square(0.0, 0.0, 100.0),
    ));
    t.landscape.push(Landscape::new(
        LandscapeKind::Plants,
        ShapeKind::Polyline,
        line,
    ));
    t.elevation_points.push(ElevationPoint {
        pos: pt(1.0, 1.0),
        z: 0.0,
    });
    let cat_of = |t: &Terrain, k| category_of(t, k);
    let cat = |k| cat_of(&t, k);
    assert_eq!(
        cat(ObjectKey::Perimeter),
        Some(ScheduleCategory::TerrainPerimeter)
    );
    assert_eq!(cat(ObjectKey::Road(0)), Some(ScheduleCategory::Roads));
    assert_eq!(cat(ObjectKey::Road(1)), Some(ScheduleCategory::Driveways));
    assert_eq!(
        cat(ObjectKey::Road(2)),
        Some(ScheduleCategory::TerrainPaths)
    );
    assert_eq!(
        cat(ObjectKey::Road(3)),
        Some(ScheduleCategory::RoadMarkings)
    );
    assert_eq!(cat(ObjectKey::Road(4)), Some(ScheduleCategory::Medians));
    assert_eq!(
        cat(ObjectKey::Wall(0)),
        Some(ScheduleCategory::TerrainPaths)
    );
    assert_eq!(
        cat(ObjectKey::Feature(0)),
        Some(ScheduleCategory::TerrainFeatures)
    );
    assert_eq!(
        cat(ObjectKey::Landscape(0)),
        Some(ScheduleCategory::TerrainFeatures)
    );
    assert_eq!(
        cat(ObjectKey::Landscape(1)),
        None,
        "plants have their own schedule"
    );
    assert_eq!(cat(ObjectKey::Point(0)), None);
    // Reassign the sidewalk to Driveways on its Schedule panel.
    let mut ex = t.extras(ObjectKey::Road(2));
    ex.schedule_category = "driveways".into();
    t.set_extras(ObjectKey::Road(2), ex);
    assert_eq!(
        cat_of(&t, ObjectKey::Road(2)),
        Some(ScheduleCategory::Driveways)
    );
    let rows = terrain_schedule(&t);
    assert!(rows.windows(2).all(|w| w[0].category <= w[1].category));
    let drives: Vec<&ScheduleRow> = rows
        .iter()
        .filter(|r| r.category == ScheduleCategory::Driveways)
        .collect();
    assert_eq!(drives.len(), 2);
    assert!(drives[0].name.starts_with("Driveway") && drives[1].name.starts_with("Sidewalk"));
    assert!((drives[0].area - 240.0 * 48.0).abs() < 1e-6);
    assert_eq!(
        ScheduleCategory::from_name(" Terrain Paths "),
        Some(ScheduleCategory::TerrainPaths)
    );
    assert_eq!(ScheduleCategory::from_name("nope"), None);
}

// ----- import assistants -----

const POINTS: &str = "1,100,200,5\n2,300,400,7.5\n3,500,600,9\n";

#[test]
fn the_import_assistant_reads_every_column_order() {
    let layout = |order| TextLayout {
        order,
        ..TextLayout::default()
    };
    // Detected: four numbers are a point number and X Y Z, three are X Y Z.
    let (p, _) = read_columns(POINTS, &TextLayout::default());
    assert_eq!(
        (p[0].x, p[0].y, p[0].z, p[0].number.as_str()),
        (100.0, 200.0, 5.0, "1")
    );
    let (p, _) = read_columns("100 200 5", &TextLayout::default());
    assert_eq!((p[0].x, p[0].y, p[0].z), (100.0, 200.0, 5.0));
    let (p, skipped) = read_columns(POINTS, &layout(ColumnOrder::NXyz));
    assert_eq!(skipped, 0);
    assert_eq!((p[1].x, p[1].y, p[1].z), (300.0, 400.0, 7.5));
    assert_eq!(p[2].number, "3");
    // YXZ swaps the first two coordinates.
    let (p, _) = read_columns(POINTS, &layout(ColumnOrder::NYxz));
    assert_eq!((p[0].x, p[0].y), (200.0, 100.0));
    let (p, _) = read_columns("100 200 5\n300 400 7", &layout(ColumnOrder::Xyz));
    assert_eq!(p.len(), 2);
    let (p, _) = read_columns("200;100;5", &layout(ColumnOrder::Yxz));
    assert_eq!((p[0].x, p[0].y, p[0].z), (100.0, 200.0, 5.0));
    // A description is the rest of the line.
    let (p, _) = read_columns(
        "A1,10,20,3,top of wall\nA2,30,40,4,corner of lot",
        &layout(ColumnOrder::NXyzDescription),
    );
    assert_eq!(p[0].description, "top of wall");
    assert_eq!(p[1].number, "A2");
    // Header lines are skipped without being counted; bad rows are.
    let text = "ID,X,Y,Z\n1,10,20,3\n2,oops,40,4\n";
    let (p, skipped) = read_columns(text, &layout(ColumnOrder::NXyz));
    assert_eq!((p.len(), skipped), (1, 1));
    let (p, _) = read_columns(
        text,
        &TextLayout {
            order: ColumnOrder::NXyz,
            skip_lines: 1,
            ..TextLayout::default()
        },
    );
    assert_eq!(p.len(), 1);
    // A fixed delimiter.
    let (p, _) = read_columns(
        "10 20 3",
        &TextLayout {
            delimiter: Delimiter::Comma,
            ..TextLayout::default()
        },
    );
    assert!(p.is_empty());
}

#[test]
fn the_import_filter_limits_ranges_and_thins_evenly() {
    let (raw, _) = read_columns(
        POINTS,
        &TextLayout {
            order: ColumnOrder::NXyz,
            ..TextLayout::default()
        },
    );
    let r = ranges_of(&raw).unwrap();
    assert_eq!((r.count, r.x, r.z), (3, (100.0, 500.0), (5.0, 9.0)));
    let kept = filter_points(
        &raw,
        &RangeFilter {
            x: Some((200.0, 600.0)),
            ..RangeFilter::default()
        },
    );
    assert_eq!(kept.len(), 2);
    let kept = filter_points(
        &raw,
        &RangeFilter {
            z: Some((6.0, 8.0)),
            y: Some((0.0, 500.0)),
            ..RangeFilter::default()
        },
    );
    assert_eq!(kept.len(), 1);
    // A 20 x 20 grid of points reduced to 25 stays spread over the area.
    let mut grid = Vec::new();
    for i in 0..20 {
        for j in 0..20 {
            grid.push(RawPoint {
                x: f64::from(i) * 10.0,
                y: f64::from(j) * 10.0,
                z: 0.0,
                number: String::new(),
                description: String::new(),
            });
        }
    }
    let thin = filter_points(
        &grid,
        &RangeFilter {
            max_points: Some(25),
            ..RangeFilter::default()
        },
    );
    assert!(thin.len() <= 25 && thin.len() >= 12, "{}", thin.len());
    let r = ranges_of(&thin).unwrap();
    assert!(
        r.x.1 - r.x.0 > 120.0 && r.y.1 - r.y.0 > 120.0,
        "spread over the area: {r:?}"
    );
    assert_eq!(
        filter_points(
            &grid,
            &RangeFilter {
                max_points: Some(1000),
                ..RangeFilter::default()
            }
        )
        .len(),
        400
    );
    assert!(MANY_POINTS >= 1000);
}

#[test]
fn the_import_scale_step_converts_units_maps_the_origin_relieves_and_rotates() {
    let (raw, _) = read_columns("10 20 5", &TextLayout::default());
    let feet = scale_points(&raw, &ScaleOptions::uniform(ImportUnit::Feet));
    assert_eq!((feet[0].pos, feet[0].z), (pt(120.0, 240.0), 60.0));
    // Units per axis.
    let mixed = scale_points(
        &raw,
        &ScaleOptions {
            unit_x: ImportUnit::Feet,
            unit_y: ImportUnit::Inches,
            unit_z: ImportUnit::Meters,
            ..ScaleOptions::default()
        },
    );
    assert_eq!(mixed[0].pos, pt(120.0, 20.0));
    assert!((mixed[0].z - 5.0 / 0.0254).abs() < 1e-6);
    // A file point becomes the plan origin.
    let moved = scale_points(
        &raw,
        &ScaleOptions {
            map_to_origin: Some((10.0, 20.0)),
            ..ScaleOptions::default()
        },
    );
    assert_eq!(moved[0].pos, pt(0.0, 0.0));
    // Relief scale.
    let relief = scale_points(
        &raw,
        &ScaleOptions {
            relief_scale: 2.0,
            ..ScaleOptions::default()
        },
    );
    assert_eq!(relief[0].z, 120.0);
    // Rotate north counterclockwise by 90 degrees.
    let (raw, _) = read_columns("10 0 0", &TextLayout::default());
    let turned = scale_points(
        &raw,
        &ScaleOptions {
            rotate_ccw: 90.0,
            ..ScaleOptions::default()
        },
    );
    assert!(turned[0].pos.x.abs() < 1e-9 && (turned[0].pos.y - 120.0).abs() < 1e-9);
    // The whole assistant.
    let job = TerrainImport {
        layout: TextLayout {
            order: ColumnOrder::NYxz,
            ..TextLayout::default()
        },
        filter: RangeFilter {
            z: Some((6.0, 10.0)),
            ..RangeFilter::default()
        },
        scale: ScaleOptions::uniform(ImportUnit::Inches),
    };
    let got = import_terrain_text(POINTS, &job).unwrap();
    assert_eq!(got.points.len(), 2);
    assert_eq!(got.points[0].pos, pt(400.0, 300.0));
    assert!(import_terrain_text("nothing here", &job).is_err());
    let none = TerrainImport {
        filter: RangeFilter {
            z: Some((50.0, 60.0)),
            ..RangeFilter::default()
        },
        ..job
    };
    assert!(import_terrain_text(POINTS, &none).is_err());
}

#[test]
fn an_import_creates_the_perimeter_around_the_data_when_there_is_none() {
    let mut t = Terrain {
        perimeter: Vec::new(),
        ..Terrain::default()
    };
    let pts = vec![
        ElevationPoint {
            pos: pt(100.0, 100.0),
            z: 1.0,
        },
        ElevationPoint {
            pos: pt(900.0, 700.0),
            z: 2.0,
        },
    ];
    assert_eq!(t.import_elevation_points(&pts, true), 2);
    assert_eq!(t.perimeter.len(), 4);
    let (lo, hi) = (t.perimeter[0], t.perimeter[2]);
    assert!(lo.x < 100.0 && lo.y < 100.0 && hi.x > 900.0 && hi.y > 700.0);
    // An existing perimeter is left alone.
    let before = t.perimeter.clone();
    t.import_elevation_points(
        &[ElevationPoint {
            pos: pt(5000.0, 5000.0),
            z: 3.0,
        }],
        true,
    );
    assert_eq!(t.perimeter, before);
    assert!(perimeter_around(&[], 10.0).is_empty());
}

const GPX: &str = r#"<?xml version="1.0"?>
<gpx version="1.1" creator="t">
 <wpt lat="40.0000" lon="-75.0000"><ele>100</ele><name>Corner A</name></wpt>
 <wpt lat="40.0005" lon="-75.0000"><ele>101.5</ele><name>Corner B</name></wpt>
 <wpt lat="40.0005" lon="-74.9995"><name>No elevation</name></wpt>
 <rte><rtept lat="41.0" lon="-75.0"><ele>5</ele></rtept><rtept lat="41.1" lon="-75.0"/></rte>
 <trk><trkseg>
  <trkpt lat="40.0000" lon="-75.0000"/>
  <trkpt lat="40.0010" lon="-75.0000"/>
  <trkpt lat="40.0010" lon="-74.9990"/>
  <trkpt lat="40.0000" lon="-74.9990"/>
 </trkseg></trk>
</gpx>"#;

#[test]
fn gps_way_points_carry_elevation_track_points_do_not_and_routes_are_ignored() {
    let pts = parse_gpx_points(GPX).unwrap();
    assert_eq!(pts.iter().filter(|p| p.kind == GpsKind::Way).count(), 3);
    assert_eq!(pts.iter().filter(|p| p.kind == GpsKind::Route).count(), 2);
    assert_eq!(pts.iter().filter(|p| p.kind == GpsKind::Track).count(), 4);
    assert_eq!(pts[0].name, "Corner A");
    assert!(parse_gpx_points("not xml").is_err());
    let tr = GpsTransform::default();
    // Way points as elevation data; the one without <ele> is counted, the route ignored.
    let got = import_gps(GPX, GpsImportAs::ElevationData, GpsImportAs::Marker, &tr).unwrap();
    assert_eq!(got.elevation_points.len(), 2);
    assert_eq!(got.no_elevation, 1);
    assert_eq!(got.route_ignored, 2);
    assert_eq!(got.markers.len(), 4, "track points as markers");
    assert!((got.elevation_points[0].z - 100.0 / 0.0254).abs() < 1e-6);
    // The first point is the origin; north is plan y.
    assert_eq!(got.elevation_points[0].pos, pt(0.0, 0.0));
    assert!(
        got.elevation_points[1].pos.y > 1900.0 && got.elevation_points[1].pos.y < 2200.0,
        "{:?}",
        got.elevation_points[1].pos
    );
    assert!(got.summary().contains("route points ignored"));
    // Track points as a perimeter, way points as a polyline.
    let got = import_gps(GPX, GpsImportAs::Polyline, GpsImportAs::Perimeter, &tr).unwrap();
    assert_eq!(got.perimeter.len(), 4);
    assert_eq!(got.polyline.len(), 3);
    assert!(got.elevation_points.is_empty());
    // Track points cannot be elevation data.
    let only_tracks = "<gpx><trk><trkseg><trkpt lat=\"1\" lon=\"1\"/></trkseg></trk></gpx>";
    assert!(import_gps(
        only_tracks,
        GpsImportAs::Marker,
        GpsImportAs::ElevationData,
        &tr
    )
    .is_err());
    // The legacy single-form importer reads way points only.
    let legacy = import_points(GPX, ImportUnit::Auto).unwrap();
    assert_eq!(legacy.points.len(), 3);
}

#[test]
fn gps_transform_lowers_rotates_and_maps_the_origin() {
    let base = import_gps(
        GPX,
        GpsImportAs::ElevationData,
        GpsImportAs::Marker,
        &GpsTransform::default(),
    )
    .unwrap();
    let lowered = import_gps(
        GPX,
        GpsImportAs::ElevationData,
        GpsImportAs::Marker,
        &GpsTransform {
            lower_by: 100.0 / 0.0254,
            ..GpsTransform::default()
        },
    )
    .unwrap();
    assert!(lowered.elevation_points[0].z.abs() < 1e-6);
    assert!((lowered.elevation_points[1].z - 1.5 / 0.0254).abs() < 1e-6);
    let turned = import_gps(
        GPX,
        GpsImportAs::ElevationData,
        GpsImportAs::Marker,
        &GpsTransform {
            rotate_ccw: 90.0,
            ..GpsTransform::default()
        },
    )
    .unwrap();
    let (b, r) = (base.elevation_points[1].pos, turned.elevation_points[1].pos);
    assert!((r.x + b.y).abs() < 1e-6 && (r.y - b.x).abs() < 1e-6);
    let mapped = import_gps(
        GPX,
        GpsImportAs::ElevationData,
        GpsImportAs::Marker,
        &GpsTransform {
            origin: Some((40.0005, -75.0)),
            ..GpsTransform::default()
        },
    )
    .unwrap();
    assert!(mapped.elevation_points[1].pos.dist(pt(0.0, 0.0)) < 1e-6);
}

// ----- roads -----

#[test]
fn a_flare_widens_the_end_of_a_road_by_a_quarter_circle() {
    assert!((flare_offset(24.0, 0.0) - 24.0).abs() < 1e-9);
    assert!(flare_offset(24.0, 24.0).abs() < 1e-9);
    assert!(flare_offset(24.0, 30.0).abs() < 1e-9);
    let mid = flare_offset(24.0, 12.0);
    assert!((mid - (24.0 - (24.0f64 * 24.0 - 12.0 * 12.0).sqrt())).abs() < 1e-9);
    let mut road = RoadStrip {
        centerline: vec![pt(0.0, 0.0), pt(600.0, 0.0)],
        width: 240.0,
        ..RoadStrip::default()
    };
    let plain = road_polygon(&road);
    road.flare_end = Some(24.0);
    let flared = road_polygon(&road);
    assert!(flared.len() > plain.len());
    let max_y = flared.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let min_y = flared.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    assert!(
        (max_y - 144.0).abs() < 1e-6 && (min_y + 144.0).abs() < 1e-6,
        "{max_y} {min_y}"
    );
    // The start is untouched.
    assert!(flared
        .iter()
        .filter(|p| p.x < 100.0)
        .all(|p| p.y.abs() <= 120.0 + 1e-6));
    // Both ends, and the 3D strip still builds.
    road.flare_start = Some(24.0);
    let mut t = Terrain::default();
    t.roads.push(road);
    let ms = road_meshes(&t, &build_terrain(&t));
    assert!(indices(&ms) > 0);
}

#[test]
fn polyline_roads_medians_and_cul_de_sacs_are_outline_shapes() {
    let mut t = Terrain::default();
    t.roads.push(RoadStrip {
        centerline: vec![pt(0.0, 480.0), pt(900.0, 480.0)],
        width: 240.0,
        curb: true,
        ..RoadStrip::default()
    });
    let surface = build_terrain(&t);
    let base = indices(&road_meshes(&t, &surface));
    // A polyline driveway: an outline, not a centerline.
    let outline = rectangle_along(pt(300.0, 700.0), pt(300.0, 1000.0), 144.0);
    assert_eq!(outline.len(), 4);
    t.roads.push(RoadStrip {
        kind: RoadKind::Driveway,
        outline: outline.clone(),
        ..RoadStrip::default()
    });
    let (len, area) = road_length_and_area(&t.roads[1]);
    assert!((area - 300.0 * 144.0).abs() < 1.0 && (len - 2.0 * (300.0 + 144.0)).abs() < 1.0);
    assert!(indices(&road_meshes(&t, &surface)) > base);
    // A median inside the road, with the road's curb around it.
    let before = indices(&road_meshes(&t, &surface));
    t.roads.push(RoadStrip {
        kind: RoadKind::Median,
        outline: square(400.0, 440.0, 80.0),
        ..RoadStrip::default()
    });
    assert!(indices(&road_meshes(&t, &surface)) > before);
    let med = t.roads.last().unwrap();
    assert_eq!(med.material_name(), "Grass");
    // A cul-de-sac goes on the end of the road nearest the click.
    let cds = cul_de_sac_at(&t, pt(880.0, 470.0), 60.0).unwrap();
    assert_eq!(cds.kind, RoadKind::CulDeSac);
    assert_eq!(cds.center, pt(900.0, 480.0));
    assert!(cds.radius >= 120.0);
    assert_eq!(road_polygon(&cds).len(), 36);
    assert!(
        cul_de_sac_at(&t, pt(450.0, 100.0), 60.0).is_none(),
        "not near a road end"
    );
    let n = t.roads.len();
    t.roads.push(cds);
    assert_eq!(t.roads.len(), n + 1);
    assert!(indices(&road_meshes(&t, &surface)) > before);
    // Outline roads draw as closed outlines in plan.
    let strokes = plan_symbols(&t, &[]);
    let closed = strokes
        .iter()
        .filter(|s| {
            matches!(
                s,
                Stroke::Polyline {
                    closed: true,
                    kind: StrokeKind::RoadEdge,
                    ..
                }
            )
        })
        .count();
    assert_eq!(closed, 3);
}

#[test]
fn a_driveway_or_sidewalk_cuts_the_curb_of_the_road_it_crosses() {
    let mut t = Terrain::default();
    t.roads.push(RoadStrip {
        centerline: vec![pt(0.0, 480.0), pt(1200.0, 480.0)],
        width: 240.0,
        curb: true,
        ..RoadStrip::default()
    });
    let surface = build_terrain(&t);
    let whole = indices(&road_meshes(&t, &surface));
    t.roads.push(RoadStrip {
        kind: RoadKind::Driveway,
        centerline: vec![pt(600.0, 480.0), pt(600.0, 100.0)],
        width: 144.0,
        ..RoadStrip::default()
    });
    let driveway_alone = {
        let mut u = Terrain::default();
        u.roads.push(t.roads[1].clone());
        indices(&road_meshes(&u, &surface))
    };
    let cut = indices(&road_meshes(&t, &surface)) - driveway_alone;
    assert!(cut < whole, "{cut} vs {whole}");
    t.roads[0].cut_curb = false;
    let kept = indices(&road_meshes(&t, &surface)) - driveway_alone;
    assert_eq!(kept, whole);
}

#[test]
fn terrain_to_top_and_thickness_lift_and_thicken_a_path() {
    let mut t = Terrain::default();
    t.roads.push(RoadStrip {
        kind: RoadKind::Sidewalk,
        centerline: vec![pt(100.0, 100.0), pt(700.0, 100.0)],
        width: 48.0,
        ..RoadStrip::default()
    });
    let surface = build_terrain(&t);
    let top = |t: &Terrain| {
        road_meshes(t, &surface)
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::NEG_INFINITY, f32::max)
    };
    let flat = top(&t);
    t.roads[0].to_top = 4.0;
    assert!((top(&t) - flat - 4.0).abs() < 0.01);
    let thin = indices(&road_meshes(&t, &surface));
    t.roads[0].thickness = 4.0;
    assert!(
        indices(&road_meshes(&t, &surface)) > thin,
        "side faces of the slab"
    );
}

#[test]
fn auto_generate_sidewalk_runs_beside_the_road_on_the_chosen_sides() {
    let mut t = Terrain::default();
    for (a, b) in [
        ((0.0, 400.0), (500.0, 400.0)),
        ((500.0, 400.0), (900.0, 700.0)),
    ] {
        t.roads.push(RoadStrip {
            centerline: vec![pt(a.0, a.1), pt(b.0, b.1)],
            width: 240.0,
            ..RoadStrip::default()
        });
    }
    t.roads.push(RoadStrip {
        centerline: vec![pt(0.0, 50.0), pt(300.0, 50.0)],
        ..RoadStrip::default()
    });
    assert_eq!(connected_roads(&t, 0), vec![0, 1]);
    assert_eq!(connected_roads(&t, 2), vec![2]);
    let both = auto_sidewalks(&t, 0, &AutoSidewalk::default());
    assert_eq!(both.len(), 4);
    assert!(both
        .iter()
        .all(|r| r.kind == RoadKind::Sidewalk && r.width == 48.0));
    let one = auto_sidewalks(
        &t,
        0,
        &AutoSidewalk {
            all_connected: false,
            right: false,
            ..AutoSidewalk::default()
        },
    );
    assert_eq!(one.len(), 1);
    // The sidewalk sits beside the road's edge: 120 + 24 off the centerline, 12 more with an offset.
    assert!(
        (one[0].centerline[0].y - (400.0 + 144.0)).abs() < 1e-6,
        "{:?}",
        one[0].centerline
    );
    let off = auto_sidewalks(
        &t,
        0,
        &AutoSidewalk {
            all_connected: false,
            right: false,
            offset: 12.0,
            ..AutoSidewalk::default()
        },
    );
    assert!((off[0].centerline[0].y - (400.0 + 156.0)).abs() < 1e-6);
    assert!(auto_sidewalks(&t, 9, &AutoSidewalk::default()).is_empty());
}

// ----- features and grass -----

#[test]
fn a_feature_with_a_thickness_is_a_shell_and_clips_what_a_lower_feature_cuts() {
    let mut t = Terrain::default();
    t.features.push(Feature {
        polygon: square(200.0, 200.0, 300.0),
        material: "Concrete".into(),
        height: 24.0,
        ..Feature::default()
    });
    let surface = build_terrain(&t);
    let low = |t: &Terrain| {
        landscape_meshes(t, Some(&surface))
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::INFINITY, f32::min)
    };
    assert!(low(&t).abs() < 1.0, "solid down to the ground");
    t.features[0].thickness = 6.0;
    assert!(
        (low(&t) - 18.0).abs() < 0.01,
        "a shell 6 in thick under the 24 in top: {}",
        low(&t)
    );
    t.features[0].height = -12.0;
    t.features[0].thickness = 6.0;
    assert!(low(&t) < -17.0, "sunk below the ground: {}", low(&t));
    // Clip overlap: the higher feature hides the part a lower one covers.
    t.features[0].height = 36.0;
    t.features[0].thickness = 0.0;
    t.features.push(Feature {
        polygon: square(300.0, 300.0, 100.0),
        height: 6.0,
        ..Feature::default()
    });
    let plain = indices(&landscape_meshes(&t, Some(&surface)));
    t.features[0].clip_overlap = true;
    let tops = |t: &Terrain| {
        landscape_meshes(t, Some(&surface))
            .iter()
            .filter(|m| m.object_id == Some(terrain_object_id(TerrainPart::Feature, 0)))
            .map(|m| m.indices.len())
            .sum::<usize>()
    };
    let clipped = tops(&t);
    t.features[0].clip_overlap = false;
    assert!(clipped != tops(&t));
    let _ = plain;
}

#[test]
fn grass_blades_sanitize_and_the_look_tints_the_region() {
    let b = GrassBlades {
        min_height: 4.0,
        max_height: 2.0,
        min_curve: 2.0,
        max_curve: -1.0,
        density: 9999.0,
        ..GrassBlades::default()
    };
    let s = b.sanitized();
    assert_eq!((s.min_height, s.max_height), (2.0, 4.0));
    assert_eq!((s.min_curve, s.max_curve), (0.0, 1.0));
    assert_eq!(s.density, 400.0);
    assert_eq!(s.mean_height(), 3.0);
    let look = GrassLook {
        colors: vec![[10, 20, 30], [30, 40, 50]],
        ..GrassLook::default()
    };
    assert_eq!(look.average_color(), [20, 30, 40]);
    let mut t = Terrain::default();
    t.landscape.push(Landscape::new(
        LandscapeKind::GrassRegion,
        ShapeKind::Polyline,
        square(100.0, 100.0, 300.0),
    ));
    let surface = build_terrain(&t);
    let plain = landscape_meshes(&t, Some(&surface));
    assert!(plain.iter().all(|m| m.color.is_none()));
    t.landscape[0].grass_look = look;
    let tinted = landscape_meshes(&t, Some(&surface));
    assert!(tinted.iter().all(|m| m.color == Some([20, 30, 40])));
}

// ----- plants -----

#[test]
fn a_plant_image_keeps_its_aspect_ratio_and_its_elevation_reference() {
    let mut img = PlantImage::sized("maple.png", 100.0, 200.0, false);
    assert_eq!(img.original_aspect, 2.0);
    img.set_width(50.0);
    assert_eq!(img.height, 100.0);
    img.set_height(300.0);
    assert_eq!(img.width, 150.0);
    img.retain_aspect = false;
    img.set_width(80.0);
    assert_eq!(img.height, 300.0);
    img.reset_aspect();
    assert_eq!(img.height, 160.0);
    img.elevation = 20.0;
    assert_eq!(img.bottom_above_ground(), 20.0);
    img.elevation_to_top = true;
    assert_eq!(img.bottom_above_ground(), 20.0 - 160.0);
}

#[test]
fn plant_images_are_billboards_tinted_by_season() {
    let mut run = Landscape::new(
        LandscapeKind::Plants,
        ShapeKind::Polyline,
        vec![pt(100.0, 100.0), pt(500.0, 100.0)],
    );
    run.image = Some(PlantImage::sized("maple.png", 120.0, 240.0, false));
    run.size = 120.0;
    run.height = 240.0;
    let mut t = Terrain::default();
    t.landscape.push(run);
    let surface = build_terrain(&t);
    let build = |t: &Terrain| landscape_meshes(t, Some(&surface));
    t.season = Season::Summer;
    let summer = build(&t);
    assert_eq!(summer.len(), 1, "one billboard mesh for the whole run");
    assert_eq!(
        summer[0].color,
        Some(
            t.landscape[0]
                .image
                .as_ref()
                .unwrap()
                .look(Season::Summer)
                .tint
        )
    );
    t.season = Season::Autumn;
    let autumn = build(&t);
    assert_ne!(autumn[0].color, summer[0].color);
    t.season = Season::Winter;
    let winter = build(&t);
    assert_eq!(
        winter[0].color,
        Some(
            t.landscape[0]
                .image
                .as_ref()
                .unwrap()
                .look(Season::Winter)
                .tint
        )
    );
    // A bare winter plant has a thinner crown.
    let width = |ms: &[Mesh]| {
        let xs: Vec<f32> = ms[0].vertices.iter().map(|v| v.position[0]).collect();
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max)
            - xs.iter().copied().fold(f32::INFINITY, f32::min)
    };
    assert!(width(&winter) < width(&summer));
    // An evergreen keeps its foliage.
    let evergreen = default_seasons(true);
    assert!(evergreen.iter().all(|s| s.foliage == 1.0));
    assert_eq!(Season::ALL.len(), 4);
}

#[test]
fn grow_plants_scales_plants_that_have_a_mature_size() {
    let mut a = Landscape::new(
        LandscapeKind::Plants,
        ShapeKind::Polyline,
        vec![pt(0.0, 0.0), pt(100.0, 0.0)],
    );
    a.mature_height = 240.0;
    a.mature_width = 180.0;
    a.maturity_months = default_age_at_maturity(240.0);
    a.start_fraction = 0.25;
    let mut b = Landscape::new(
        LandscapeKind::Plants,
        ShapeKind::Polyline,
        vec![pt(0.0, 0.0), pt(100.0, 0.0)],
    );
    b.height = 50.0;
    let mut runs = vec![a, b];
    assert_eq!(grow_plants(&mut runs, 0.0), 1);
    assert!((runs[0].height - 60.0).abs() < 1e-9 && (runs[0].size - 45.0).abs() < 1e-9);
    assert_eq!(runs[1].height, 50.0, "no growth data, no change");
    grow_plants(&mut runs, 10.0);
    let mid = runs[0].height;
    assert!(mid > 60.0 && mid < 240.0, "{mid}");
    grow_plants(&mut runs, 20.0);
    assert!((runs[0].height - 240.0).abs() < 1e-9);
    // The slider stops at 20 years and applying twice changes nothing.
    assert_eq!(grow_plants(&mut runs, 50.0), 0);
    assert_eq!(growth_fraction(0.0, 0.0, 0.5), 1.0);
    // Image sizes follow.
    runs[0].image = Some(PlantImage::default());
    grow_plants(&mut runs, 0.0);
    assert_eq!(runs[0].image.as_ref().unwrap().height, runs[0].height);
}

#[test]
fn a_garden_bed_spreads_its_plant_over_the_bed_and_draws_them() {
    let mut bed = Landscape::new(
        LandscapeKind::GardenBed,
        ShapeKind::Polyline,
        square(100.0, 100.0, 360.0),
    );
    assert!(bed.distributed_positions().is_empty());
    bed.distribution = Some(Distribution {
        plant: "plants.shrub.boxwood".into(),
        spacing: 48.0,
        margin: 12.0,
        stagger: false,
        ..Distribution::default()
    });
    let pts = bed.distributed_positions();
    assert!(pts.len() >= 40 && pts.len() <= 64, "{}", pts.len());
    assert!(pts
        .iter()
        .all(|p| p.x > 112.0 && p.x < 448.0 && p.y > 112.0 && p.y < 448.0));
    let mut staggered = bed.clone();
    staggered.distribution.as_mut().unwrap().stagger = true;
    assert!(staggered.distributed_positions().len() > 20);
    // Inside a smaller outline it fits fewer.
    let few = distribute_in(
        &square(0.0, 0.0, 100.0),
        &Distribution {
            spacing: 48.0,
            margin: 12.0,
            ..Distribution::default()
        },
    );
    assert!(few.len() < 8);
    assert!(distribute_in(&[], &Distribution::default()).is_empty());
    let mut t = Terrain::default();
    t.landscape.push(bed);
    let surface = build_terrain(&t);
    let with = landscape_meshes(&t, Some(&surface)).len();
    t.landscape[0].distribution = None;
    assert!(with > landscape_meshes(&t, Some(&surface)).len());
}

#[test]
fn a_sprinkler_line_is_a_dashed_two_d_pipe_with_no_3d() {
    let mut t = Terrain::default();
    t.landscape.push(Landscape::new(
        LandscapeKind::SprinklerLine,
        ShapeKind::Polyline,
        vec![pt(100.0, 100.0), pt(500.0, 100.0), pt(500.0, 400.0)],
    ));
    let l = &t.landscape[0];
    assert!(!l.is_region());
    assert_eq!(l.name(), "Sprinkler Line");
    assert_eq!(l.layer(), "Sprinklers");
    assert!(landscape_meshes(&t, Some(&build_terrain(&t))).is_empty());
    let items = landscape_plan(&t);
    assert!(items
        .iter()
        .any(|i| matches!(&i.shape, PlanShape::Polyline { dashed: true, closed: false, points, .. } if points.len() == 3)));
    assert_eq!(category_of(&t, ObjectKey::Landscape(0)), None);
}
