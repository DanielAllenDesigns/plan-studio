use plan_core::geometry::{point_in_polygon, polygon_area};
use plan_core::Point;

use crate::delaunay::triangulate;
use crate::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)]
}

fn total_area(s: &TerrainSurface) -> f64 {
    s.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| s.plan_point(i));
            polygon_area(&[a, b, c])
        })
        .sum()
}

/// A tilted plane over 0..1200 x 0..960 with z = x / 10, two triangles per cell.
fn tilted_plane() -> TerrainSurface {
    let (nx, ny, step) = (11usize, 9usize, 120.0);
    let mut vertices = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let x = i as f64 * step;
            vertices.push([x, x / 10.0, j as f64 * step]);
        }
    }
    let mut triangles = Vec::new();
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let a = (j * nx + i) as u32;
            let (b, c, d) = (a + 1, a + nx as u32, a + nx as u32 + 1);
            triangles.push([a, b, d]);
            triangles.push([a, d, c]);
        }
    }
    TerrainSurface::new(vertices, triangles, HeightGrid::default())
}

fn hill_terrain(smoothing: u32) -> Terrain {
    Terrain {
        modifiers: vec![Modifier {
            kind: ModifierKind::Hill,
            polygon: rect(360.0, 240.0, 840.0, 720.0),
            height: 36.0,
        }],
        smoothing,
        ..Terrain::default()
    }
}

#[test]
fn flat_perimeter_without_data_is_level_and_has_no_contours() {
    let s = build_terrain(&Terrain::default());
    assert!(!s.triangles.is_empty());
    assert!(s.vertices.iter().all(|v| v[1] == 0.0));
    assert!(s.grid.z.iter().all(|&z| z == 0.0));
    assert!(contours(&s, 12.0).is_empty());
    // The triangles tile the whole 100' x 80' lot.
    assert!((total_area(&s) - 1200.0 * 960.0).abs() < 1e-3);
    assert_eq!(elevation_at(&s, pt(600.0, 480.0)), Some(0.0));
    assert_eq!(elevation_at(&s, pt(-5.0, 480.0)), None);
}

#[test]
fn two_points_give_monotonic_elevation_between_them() {
    let t = Terrain {
        elevation_points: vec![
            ElevationPoint {
                pos: pt(0.0, 480.0),
                z: 0.0,
            },
            ElevationPoint {
                pos: pt(1200.0, 480.0),
                z: 120.0,
            },
        ],
        ..Terrain::default()
    };
    let s = build_terrain(&t);
    let mut prev = f64::NEG_INFINITY;
    for k in 0..=120 {
        let z = elevation_at(&s, pt(k as f64 * 10.0, 480.0)).expect("on the lot");
        assert!(z >= prev - 1e-6, "not monotonic at step {k}: {z} < {prev}");
        prev = z;
    }
    assert!(elevation_at(&s, pt(0.0, 480.0)).unwrap().abs() < 1e-6);
    assert!((elevation_at(&s, pt(1200.0, 480.0)).unwrap() - 120.0).abs() < 1e-6);
}

#[test]
fn hill_raises_centroid_and_leaves_boundary_alone() {
    let t = hill_terrain(0);
    let s = build_terrain(&t);
    let top = elevation_at(&s, pt(600.0, 480.0)).unwrap();
    assert!((top - 36.0).abs() < 1.0, "peak {top}");
    let lot = &t.perimeter;
    for p in lot
        .iter()
        .copied()
        .chain([pt(600.0, 0.0), pt(1200.0, 480.0), pt(0.0, 480.0)])
    {
        assert!(elevation_at(&s, p).unwrap().abs() < 1.0, "lot edge {p:?}");
    }
    for p in &t.modifiers[0].polygon {
        assert!(elevation_at(&s, *p).unwrap().abs() < 1.0, "hill edge {p:?}");
    }
    // A valley mirrors it.
    let mut valley = t.clone();
    valley.modifiers[0].kind = ModifierKind::Valley;
    let v = elevation_at(&build_terrain(&valley), pt(600.0, 480.0)).unwrap();
    assert!((v + 36.0).abs() < 1.0);
}

#[test]
fn smoothing_flattens_the_peak_but_not_the_boundary() {
    let sharp = elevation_at(&build_terrain(&hill_terrain(0)), pt(600.0, 480.0)).unwrap();
    let soft = build_terrain(&hill_terrain(3));
    let peak = elevation_at(&soft, pt(600.0, 480.0)).unwrap();
    assert!(peak < sharp - 1.0 && peak > 0.0);
    assert_eq!(elevation_at(&soft, pt(0.0, 0.0)), Some(0.0));
}

#[test]
fn raised_and_flat_regions_shift_inside_only() {
    let region = rect(360.0, 240.0, 840.0, 720.0);
    let raised = Terrain {
        modifiers: vec![Modifier {
            kind: ModifierKind::RaisedRegion,
            polygon: region.clone(),
            height: 24.0,
        }],
        ..Terrain::default()
    };
    let s = build_terrain(&raised);
    assert!((elevation_at(&s, pt(600.0, 480.0)).unwrap() - 24.0).abs() < 1e-6);
    assert!(elevation_at(&s, pt(100.0, 100.0)).unwrap().abs() < 1e-6);

    // Sloped ground (0 at x=0, 120 at x=1200) levelled to its mean inside the region.
    let flat = Terrain {
        elevation_points: vec![
            ElevationPoint {
                pos: pt(0.0, 480.0),
                z: 0.0,
            },
            ElevationPoint {
                pos: pt(1200.0, 480.0),
                z: 120.0,
            },
        ],
        modifiers: vec![Modifier {
            kind: ModifierKind::FlatRegion,
            polygon: region,
            height: 0.0,
        }],
        ..Terrain::default()
    };
    let s = build_terrain(&flat);
    let a = elevation_at(&s, pt(540.0, 480.0)).unwrap();
    let b = elevation_at(&s, pt(660.0, 480.0)).unwrap();
    assert!((a - b).abs() < 1e-6, "pad is level: {a} vs {b}");
}

#[test]
fn hole_feature_removes_triangles_under_a_footprint() {
    let mut t = Terrain::default();
    let footprint = rect(420.0, 330.0, 780.0, 630.0);
    auto_hole_for_building(&mut t, &footprint);
    let hole = &t.features[0];
    assert_eq!(hole.kind, FeatureKind::Hole);
    let min_x = hole
        .polygon
        .iter()
        .map(|p| p.x)
        .fold(f64::INFINITY, f64::min);
    assert!((min_x - 408.0).abs() < 1e-9, "offset outward by 12 inches");
    let s = build_terrain(&t);
    assert_eq!(elevation_at(&s, pt(600.0, 480.0)), None);
    assert_eq!(elevation_at(&s, pt(500.0, 400.0)), None);
    assert!(elevation_at(&s, pt(200.0, 200.0)).is_some());
    assert!(total_area(&s) < 1200.0 * 960.0 - 300.0 * 300.0);
}

#[test]
fn contours_of_a_tilted_plane_are_parallel_lines() {
    let s = tilted_plane();
    let cs = contours(&s, 12.0);
    // z runs 0..120 over the span, so 120 / 12 = 10 contours (level 0 only touches the edge).
    assert_eq!(cs.len(), 10);
    for c in &cs {
        assert_eq!(c.polylines.len(), 1);
        let x = c.z * 10.0;
        let line = &c.polylines[0];
        assert!(line.iter().all(|p| (p.x - x).abs() < 1e-6));
        let ys: Vec<f64> = line.iter().map(|p| p.y).collect();
        let (lo, hi) = (
            ys.iter().copied().fold(f64::INFINITY, f64::min),
            ys.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        );
        assert!((lo - 0.0).abs() < 1e-6 && (hi - 960.0).abs() < 1e-6);
    }
    let majors: Vec<f64> = cs.iter().filter(|c| c.major).map(|c| c.z).collect();
    assert_eq!(majors, vec![60.0, 120.0]);
}

#[test]
fn delaunay_of_square_with_center_has_four_triangles() {
    let pts = [
        pt(0.0, 0.0),
        pt(10.0, 0.0),
        pt(10.0, 10.0),
        pt(0.0, 10.0),
        pt(5.0, 5.0),
    ];
    let tris = triangulate(&pts);
    assert_eq!(tris.len(), 4);
    assert!(tris.iter().all(|t| t.contains(&4)));
    let area: f64 = tris.iter().map(|t| polygon_area(&t.map(|i| pts[i]))).sum();
    assert!(
        (area - 100.0).abs() < 1e-9,
        "all counter-clockwise, covering the square"
    );
}

#[test]
fn delaunay_circumcircles_are_empty() {
    // Deterministic scatter (LCG) so the test needs no rand crate.
    let mut seed = 12345u64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };
    let pts: Vec<Point> = (0..60)
        .map(|_| pt(next() * 1000.0, next() * 700.0))
        .collect();
    let tris = triangulate(&pts);
    assert!(tris.len() >= 60);
    for t in &tris {
        let [a, b, c] = t.map(|i| pts[i]);
        let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
        let ux = ((a.x * a.x + a.y * a.y) * (b.y - c.y)
            + (b.x * b.x + b.y * b.y) * (c.y - a.y)
            + (c.x * c.x + c.y * c.y) * (a.y - b.y))
            / d;
        let uy = ((a.x * a.x + a.y * a.y) * (c.x - b.x)
            + (b.x * b.x + b.y * b.y) * (a.x - c.x)
            + (c.x * c.x + c.y * c.y) * (b.x - a.x))
            / d;
        let centre = pt(ux, uy);
        let r = centre.dist(a);
        for (i, p) in pts.iter().enumerate() {
            if !t.contains(&i) {
                assert!(centre.dist(*p) >= r - 1e-6, "point {i} inside circumcircle");
            }
        }
    }
}

#[test]
fn chained_contours_have_no_duplicate_consecutive_points() {
    let s = build_terrain(&hill_terrain(0));
    let cs = contours(&s, 12.0);
    assert!(cs.len() >= 2);
    for c in &cs {
        for line in &c.polylines {
            assert!(line.len() >= 2);
            assert!(line.windows(2).all(|w| w[0].dist(w[1]) > 0.005));
        }
    }
    // The mid-height contour of a hill is a closed loop.
    let mid = cs.iter().find(|c| (c.z - 12.0).abs() < 1e-9).unwrap();
    let ring = &mid.polylines[0];
    assert_eq!(ring.first(), ring.last());
    assert!(point_in_polygon(pt(600.0, 480.0), ring));
}

#[test]
fn terrain_mesh_faces_up_and_uses_scene_frame() {
    let s = build_terrain(&Terrain::default());
    let m = terrain_mesh(&s);
    assert_eq!(m.triangle_count(), s.triangles.len());
    assert!(m.vertices.iter().all(|v| v.normal[1] > 0.999));
    let (lo, hi) = m.bounds().unwrap();
    assert_eq!((lo[2], hi[2]), (-960.0, 0.0), "Z = -plan y");
    let v = m.vertices.iter().find(|v| v.position[0] == 1200.0).unwrap();
    assert_eq!(v.uv[0], 100.0, "uv in feet");
}

#[test]
fn roads_are_draped_half_an_inch_above_a_raised_lot() {
    let mut t = Terrain::default();
    t.modifiers.push(Modifier {
        kind: ModifierKind::RaisedRegion,
        polygon: rect(-100.0, -100.0, 1300.0, 1100.0),
        height: 30.0,
    });
    t.roads.push(RoadStrip {
        kind: RoadKind::Driveway,
        centerline: vec![pt(100.0, 480.0), pt(1100.0, 480.0)],
        width: 120.0,
        curb: false,
    });
    t.roads.push(RoadStrip {
        kind: RoadKind::Sidewalk,
        centerline: vec![pt(100.0, 800.0), pt(600.0, 800.0), pt(600.0, 900.0)],
        width: 48.0,
        curb: true,
    });
    let s = build_terrain(&t);
    let meshes = road_meshes(&t, &s);
    assert_eq!(meshes.len(), 3, "driveway, sidewalk, sidewalk curb");
    let (lo, hi) = meshes[0].bounds().unwrap();
    assert!((lo[1] - 30.5).abs() < 1e-3 && (hi[1] - 30.5).abs() < 1e-3);
    assert!((lo[2] + 540.0).abs() < 1e-3 && (hi[2] + 420.0).abs() < 1e-3);
    assert!(meshes[0].vertices.iter().all(|v| v.normal[1] > 0.99));
    let (_, curb_hi) = meshes[2].bounds().unwrap();
    assert!((curb_hi[1] - 36.5).abs() < 1e-3);
}

#[test]
fn plan_symbols_draw_perimeter_contours_labels_and_road_edges() {
    let mut t = hill_terrain(0);
    t.roads.push(RoadStrip {
        kind: RoadKind::Road,
        centerline: vec![pt(0.0, 100.0), pt(1200.0, 100.0)],
        width: 240.0,
        curb: true,
    });
    let cs = contours(&build_terrain(&t), 12.0);
    let strokes = plan_symbols(&t, &cs);
    let count = |k: StrokeKind| {
        strokes
            .iter()
            .filter(|s| matches!(s, Stroke::Polyline { kind, .. } if *kind == k))
            .count()
    };
    assert_eq!(count(StrokeKind::Perimeter), 1);
    assert_eq!(count(StrokeKind::RoadEdge), 2);
    assert!(count(StrokeKind::Contour) > 0);
    let labels: Vec<&str> = strokes
        .iter()
        .filter_map(|s| match s {
            Stroke::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(labels.contains(&"1'-0\""));
    let lines = strokes
        .iter()
        .filter(|s| {
            matches!(
                s,
                Stroke::Polyline {
                    kind: StrokeKind::Contour | StrokeKind::MajorContour,
                    ..
                }
            )
        })
        .count();
    assert_eq!(labels.len(), lines, "one label per contour polyline");
}

#[test]
fn spline_flattening_passes_through_control_points() {
    let ctrl = [
        pt(0.0, 0.0),
        pt(100.0, 50.0),
        pt(200.0, 0.0),
        pt(300.0, 50.0),
    ];
    let line = flatten_spline(&ctrl, false, 8);
    assert_eq!(line.len(), 3 * 8 + 1);
    assert_eq!(line[0], ctrl[0]);
    assert!(line[8].dist(ctrl[1]) < 1e-9 && line[16].dist(ctrl[2]) < 1e-9);
    assert_eq!(line.last(), ctrl.last());
}

#[test]
fn terrain_round_trips_through_json() {
    let mut t = hill_terrain(2);
    auto_hole_for_building(&mut t, &rect(0.0, 0.0, 100.0, 100.0));
    let json = serde_json::to_string(&t).unwrap();
    let back: Terrain = serde_json::from_str(&json).unwrap();
    assert_eq!(t, back);
    assert_eq!(Terrain::default().finished_floor_elevation(), 6.0);
    let partial: Terrain = serde_json::from_str("{\"smoothing\": 4}").unwrap();
    assert_eq!(partial.smoothing, 4);
    assert_eq!(partial.perimeter.len(), 4);
}

#[test]
fn invalid_perimeter_gives_an_empty_surface() {
    let t = Terrain {
        perimeter: vec![pt(0.0, 0.0), pt(10.0, 0.0)],
        ..Terrain::default()
    };
    let s = build_terrain(&t);
    assert!(s.vertices.is_empty() && s.triangles.is_empty());
    assert_eq!(elevation_at(&s, pt(1.0, 1.0)), None);
    assert!(contours(&s, 12.0).is_empty());
}
