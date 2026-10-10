use super::*;
use plan_core::geometry::polygon_area;
use plan_core::{Wall, WallKind};

fn pts(v: &[(f64, f64)]) -> Vec<Point> {
    v.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

fn rect_40x24() -> Vec<Point> {
    pts(&[(0.0, 0.0), (480.0, 0.0), (480.0, 288.0), (0.0, 288.0)])
}

fn l_shape() -> Vec<Point> {
    pts(&[
        (0.0, 0.0),
        (240.0, 0.0),
        (240.0, 120.0),
        (120.0, 120.0),
        (120.0, 240.0),
        (0.0, 240.0),
    ])
}

fn t_shape() -> Vec<Point> {
    pts(&[
        (0.0, 0.0),
        (360.0, 0.0),
        (360.0, 120.0),
        (240.0, 120.0),
        (240.0, 300.0),
        (120.0, 300.0),
        (120.0, 120.0),
        (0.0, 120.0),
    ])
}

fn u_shape() -> Vec<Point> {
    pts(&[
        (0.0, 0.0),
        (360.0, 0.0),
        (360.0, 300.0),
        (240.0, 300.0),
        (240.0, 120.0),
        (120.0, 120.0),
        (120.0, 300.0),
        (0.0, 300.0),
    ])
}

fn no_overhang(n: usize) -> Vec<EdgeRoof> {
    vec![
        EdgeRoof {
            overhang: 0.0,
            ..EdgeRoof::default()
        };
        n
    ]
}

fn wall(id: u64, a: (f64, f64), b: (f64, f64)) -> Wall {
    Wall {
        id,
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        thickness: 6.5,
        height: 108.0,
        kind: WallKind::Exterior,
        layer: "Walls, Normal".to_string(),
        ..Default::default()
    }
}

fn plan_area(poly: &[Point]) -> f64 {
    polygon_area(poly).abs()
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-6)
}

/// Every plane edge other than the eave is shared with another plane.
fn assert_closed(roof: &Roof) {
    for (pi, plane) in roof.planes.iter().enumerate() {
        let n = plane.polygon3d.len();
        for j in 1..n {
            let (a, b) = (plane.polygon3d[j], plane.polygon3d[(j + 1) % n]);
            let shared = roof.planes.iter().enumerate().any(|(qi, other)| {
                qi != pi && {
                    let m = other.polygon3d.len();
                    (0..m).any(|k| {
                        let (c, d) = (other.polygon3d[k], other.polygon3d[(k + 1) % m]);
                        (near(a, c) && near(b, d)) || (near(a, d) && near(b, c))
                    })
                }
            });
            assert!(shared, "plane {pi} edge {j} ({a:?}->{b:?}) is not shared");
        }
    }
}

fn assert_tiles(roof: &Roof, want: f64, tol: f64) {
    let got: f64 = roof.planes.iter().map(RoofPlane::projected_area).sum();
    assert!(
        (got - want).abs() <= tol * want,
        "projected area {got} vs {want}"
    );
    for p in &roof.planes {
        assert!(p.normal()[1] > 0.0, "plane {} faces down", p.source_edge);
    }
}

#[test]
fn rectangle_hip_roof() {
    let eave = 108.0;
    let roof = build_roof(&rect_40x24(), &[EdgeRoof::default(); 4], eave);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 4);

    // Overhung half-width is 144 + 16 = 160 in; 8:12 rise over it.
    let expect = (144.0 + 16.0) * 8.0 / 12.0;
    for p in &roof.planes {
        assert!(
            (p.ridge_height() - expect).abs() < 1e-6,
            "{}",
            p.ridge_height()
        );
    }
    let (lo, hi) = roof.bounds().unwrap();
    assert!((lo[1] - eave).abs() < 1e-9);
    assert!((hi[1] - (eave + expect)).abs() < 1e-6);

    // Ridge: the two top vertices share Y and Z and differ in X (long axis).
    let mut ridge: Vec<[f64; 3]> = Vec::new();
    for p in &roof.planes {
        for v in &p.polygon3d {
            if (v[1] - (eave + expect)).abs() < 1e-6 && !ridge.iter().any(|r| near(*r, *v)) {
                ridge.push(*v);
            }
        }
    }
    assert_eq!(ridge.len(), 2);
    assert!((ridge[0][2] - ridge[1][2]).abs() < 1e-6);
    assert!((ridge[0][0] - ridge[1][0]).abs() > 100.0);

    let outline = 512.0 * 320.0;
    assert_tiles(&roof, outline, 0.005);
    let sloped: f64 = roof.planes.iter().map(RoofPlane::area).sum();
    assert!(sloped > outline);
    assert_closed(&roof);
}

#[test]
fn rectangle_with_gable_ends() {
    let mut edges = vec![EdgeRoof::default(); 4];
    edges[1].kind = EdgeKind::Gable;
    edges[3].kind = EdgeKind::Gable;
    let roof = build_roof(&rect_40x24(), &edges, 96.0);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 2);
    for p in &roof.planes {
        assert_eq!(p.polygon3d.len(), 4);
        assert!((p.ridge_height() - 160.0 * 8.0 / 12.0).abs() < 1e-6);
    }
    assert_tiles(&roof, 512.0 * 320.0, 0.005);
    let sources: Vec<usize> = roof.planes.iter().map(|p| p.source_edge).collect();
    assert_eq!(sources, vec![0, 2]);
}

#[test]
fn square_pyramid_and_shed_edge() {
    let sq = pts(&[(0.0, 0.0), (240.0, 0.0), (240.0, 240.0), (0.0, 240.0)]);
    let roof = build_roof(&sq, &no_overhang(4), 0.0);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 4);
    assert!(roof.planes.iter().all(|p| p.polygon3d.len() == 3));
    assert_tiles(&roof, 240.0 * 240.0, 1e-6);

    // Shed on the top edge: three rising planes and no plane for the shed wall.
    let mut edges = no_overhang(4);
    edges[2].kind = EdgeKind::Shed;
    let roof = build_roof(&sq, &edges, 0.0);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 3);
    assert_tiles(&roof, 240.0 * 240.0, 1e-6);
}

#[test]
fn l_shape_hip_roof() {
    let edges = no_overhang(6);
    let roof = build_roof(&l_shape(), &edges, 100.0);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 6);
    assert_tiles(&roof, plan_area(&l_shape()), 0.01);
    assert_closed(&roof);
    // Wings are 120 wide, so every ridge sits 60 in * 8/12 above the eave.
    let (_, hi) = roof.bounds().unwrap();
    assert!((hi[1] - (100.0 + 40.0)).abs() < 1e-6);
}

#[test]
fn l_shape_with_overhang_and_gable() {
    let roof = build_roof(&l_shape(), &[EdgeRoof::default(); 6], 0.0);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 6);
    assert_closed(&roof);

    let mut edges = vec![EdgeRoof::default(); 6];
    edges[1].kind = EdgeKind::Gable;
    edges[4].kind = EdgeKind::Gable;
    let roof = build_roof(&l_shape(), &edges, 0.0);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 4);
    for p in &roof.planes {
        assert!(p.normal()[1] > 0.0);
    }
}

#[test]
fn t_and_u_shapes() {
    for (shape, n) in [(t_shape(), 8), (u_shape(), 8)] {
        let roof = build_roof(&shape, &no_overhang(n), 0.0);
        assert!(!roof.approximate);
        assert_eq!(roof.planes.len(), n);
        assert_tiles(&roof, plan_area(&shape), 0.01);
        assert_closed(&roof);
    }
}

#[test]
fn footprint_from_four_walls() {
    // Drawn clockwise on purpose.
    let walls = [
        wall(1, (0.0, 0.0), (0.0, 288.0)),
        wall(2, (0.0, 288.0), (480.0, 288.0)),
        wall(3, (480.0, 288.0), (480.0, 0.0)),
        wall(4, (480.0, 0.0), (0.0, 0.0)),
    ];
    let fp = footprint_from_walls(&walls, 0.5).unwrap();
    assert_eq!(fp.len(), 4);
    assert!(polygon_area(&fp) > 0.0);
    assert!((polygon_area(&fp) - 480.0 * 288.0).abs() < 1e-6);
}

#[test]
fn footprint_ignores_interior_walls_and_tjunctions() {
    let walls = [
        wall(1, (0.0, 0.0), (240.0, 0.0)),
        wall(2, (240.0, 0.0), (480.0, 0.0)),
        wall(3, (480.0, 0.0), (480.0, 288.0)),
        wall(4, (480.0, 288.0), (0.0, 288.0)),
        wall(5, (0.0, 288.0), (0.0, 0.0)),
        wall(6, (240.0, 0.0), (240.0, 288.0)), // partition, tiles two rooms
        wall(7, (240.0, 100.0), (400.0, 100.0)), // partition ending on a partition
        wall(8, (500.0, 10.0), (600.0, 10.0)), // free-standing wall, ignored
    ];
    let fp = footprint_from_walls(&walls, 0.5).unwrap();
    assert_eq!(fp.len(), 4);
    assert!((polygon_area(&fp) - 480.0 * 288.0).abs() < 1e-6);
    assert!(footprint_from_walls(&walls[7..], 0.5).is_none());
    assert!(footprint_from_walls(&[], 0.5).is_none());
}

#[test]
fn footprint_of_l_shaped_walls_builds_a_roof() {
    let l = l_shape();
    let walls: Vec<Wall> = (0..l.len())
        .map(|i| {
            let (a, b) = (l[i], l[(i + 1) % l.len()]);
            wall(i as u64, (a.x, a.y), (b.x, b.y))
        })
        .collect();
    let fp = footprint_from_walls(&walls, 0.5).unwrap();
    assert_eq!(fp.len(), 6);
    let roof = build_roof(&fp, &[EdgeRoof::default(); 6], 108.0);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 6);
}

#[test]
fn differing_pitches_close_the_roof() {
    for (shape, n) in [(rect_40x24(), 4), (l_shape(), 6), (t_shape(), 8)] {
        for over in [0.0, 16.0] {
            for odd in 0..n {
                let mut edges = vec![
                    EdgeRoof {
                        overhang: over,
                        ..EdgeRoof::default()
                    };
                    n
                ];
                edges[odd].pitch_in_12 = 4.0;
                let roof = build_roof(&shape, &edges, 0.0);
                assert!(!roof.approximate, "shape {n} odd {odd} over {over}");
                assert_eq!(roof.planes.len(), n);
                for p in &roof.planes {
                    assert!(p.normal()[1] > 0.0);
                }
                let outline: f64 = roof.planes.iter().map(RoofPlane::projected_area).sum();
                assert!(outline > plan_area(&shape) - 1e-6);
                assert_closed(&roof);
            }
        }
    }
}

#[test]
fn each_plane_has_its_own_pitch() {
    let mut edges = vec![EdgeRoof::default(); 4];
    edges[0].pitch_in_12 = 4.0;
    let roof = build_roof(&rect_40x24(), &edges, 0.0);
    for p in &roof.planes {
        // Slope of the plane: rise over horizontal distance from its eave line.
        let n = p.normal();
        let horiz = (n[0] * n[0] + n[2] * n[2]).sqrt();
        let rise_per_12 = 12.0 * horiz / n[1];
        assert!((rise_per_12 - p.pitch_in_12).abs() < 1e-6, "{rise_per_12}");
    }
}

#[test]
fn clockwise_input_is_accepted() {
    let mut cw = l_shape();
    cw.reverse();
    let roof = build_roof(&cw, &no_overhang(6), 0.0);
    assert!(!roof.approximate);
    assert_eq!(roof.planes.len(), 6);
    assert_tiles(&roof, plan_area(&l_shape()), 0.01);
}

#[test]
fn degenerate_inputs_give_empty_roofs() {
    assert!(build_roof(&[], &[], 0.0).planes.is_empty());
    let line = pts(&[(0.0, 0.0), (100.0, 0.0), (200.0, 0.0)]);
    assert!(build_roof(&line, &[], 0.0).planes.is_empty());
    let all_gable = vec![
        EdgeRoof {
            kind: EdgeKind::Gable,
            ..EdgeRoof::default()
        };
        4
    ];
    let roof = build_roof(&rect_40x24(), &all_gable, 0.0);
    assert!(roof.planes.is_empty() && !roof.approximate);
    assert!(roof.bounds().is_none());
}

#[test]
fn bounding_box_fallback_is_flagged() {
    let prep = prepare(&l_shape(), &no_overhang(6)).unwrap();
    let planes = build_bbox(&prep, 0.0);
    assert_eq!(planes.len(), 4);
    let roof = Roof {
        planes,
        fascia_height: DEFAULT_FASCIA_HEIGHT,
        baseline_elevation: 0.0,
        approximate: true,
    };
    assert_tiles(&roof, 240.0 * 240.0, 1e-6);
}

#[test]
fn unresolvable_pitch_mix_falls_back_to_uniform_pitch() {
    // A faster plane overtakes a parallel neighbour across a collapsing step:
    // the exact skeleton does not support this, so pitches are levelled.
    let pitches = [12.0, 4.0, 8.0, 12.0, 4.0, 8.0];
    let edges: Vec<EdgeRoof> = pitches
        .iter()
        .map(|&p| EdgeRoof {
            pitch_in_12: p,
            ..EdgeRoof::default()
        })
        .collect();
    let roof = build_roof(&l_shape(), &edges, 0.0);
    assert!(roof.approximate);
    assert_eq!(roof.planes.len(), 6);
    let uniform = roof.planes[0].pitch_in_12;
    assert!(roof.planes.iter().all(|p| p.pitch_in_12 == uniform));
    assert!((4.0..=12.0).contains(&uniform));
    assert_tiles(&roof, 272.0 * 152.0 + 152.0 * 120.0, 0.001);
    assert_closed(&roof);
}

// ---- Round 16 brief 18: eave alignment and plane heights (manual pp. 829-844)

fn mixed_specs(over: [f64; 2]) -> Vec<EdgeRoofSpec> {
    // South and north at 12:12, east and west at 6:12.
    let mk = |pitch: f64, overhang: f64| EdgeRoofSpec {
        pitch,
        overhang,
        ..EdgeRoofSpec::default()
    };
    vec![
        mk(12.0, over[0]),
        mk(6.0, over[1]),
        mk(12.0, over[0]),
        mk(6.0, over[1]),
    ]
}

fn wall_heights(roof: &Roof) -> Vec<f64> {
    // Height of each plane over the midpoint of its footprint edge.
    let fp = rect_40x24();
    roof.planes
        .iter()
        .map(|pl| {
            let (a, b) = (fp[pl.source_edge], fp[(pl.source_edge + 1) % 4]);
            let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            pl.height_at(mid).expect("plane has a height")
        })
        .collect()
}

#[test]
fn aligned_overhangs_make_wings_of_different_pitch_meet_at_one_eave_line() {
    let h = HeightSettings::default();
    let pitches = [Some(12.0), Some(6.0), Some(12.0), Some(6.0)];
    let ind = independent_edges(&pitches);
    assert!(ind.iter().all(|i| !i), "mixed pitches meet at the hips");
    let over: Vec<f64> = pitches
        .iter()
        .map(|p| h.eave_overhang(8.0, 16.0, p.unwrap(), 16.0, false))
        .collect();
    // A steeper plane overhangs less, a flatter one more; the fascia drop
    // (overhang x slope) is the default plane's 16 x 8/12.
    assert!((over[0] - 16.0 * 8.0 / 12.0).abs() < 1e-9);
    assert!((over[1] - 16.0 * 8.0 / 6.0).abs() < 1e-9);
    let roof = build_roof_with_specs(&rect_40x24(), &mixed_specs([over[0], over[1]]), 100.0);
    let heights = wall_heights(&roof);
    let drop = 16.0 * 8.0 / 12.0;
    for z in &heights {
        assert!((z - (100.0 + drop)).abs() < 1e-6, "wall height {z}");
    }
}

#[test]
fn equal_overhangs_leave_the_wall_heights_apart_without_alignment() {
    let roof = build_roof_with_specs(&rect_40x24(), &mixed_specs([16.0, 16.0]), 100.0);
    let heights = wall_heights(&roof);
    assert!((heights[0] - 116.0).abs() < 1e-6);
    assert!((heights[1] - 108.0).abs() < 1e-6);
}

#[test]
fn independent_planes_keep_their_overhang_unless_both_switches_are_on() {
    let mut h = HeightSettings::default();
    // One pitch all round: every plane is independent.
    let ind = independent_edges(&[Some(12.0), Some(12.0), None, Some(12.0)]);
    assert_eq!(ind, vec![true; 4]);
    assert_eq!(h.eave_overhang(8.0, 16.0, 12.0, 24.0, true), 24.0);
    h.same_height_eaves = true;
    assert!((h.eave_overhang(8.0, 16.0, 12.0, 24.0, true) - 16.0 * 8.0 / 12.0).abs() < 1e-9);
    // Same Roof Height off: overhangs are the walls' own.
    h.same_roof_height = false;
    assert_eq!(h.eave_overhang(8.0, 16.0, 12.0, 24.0, false), 24.0);
    assert!(h.eaves_at_default_height());
    assert!(!HeightSettings::default().eaves_at_default_height());
}

#[test]
fn same_height_eaves_reference_is_the_default_plane() {
    // 6" structure on a plate at 100: the default 8:12 plane with a 16"
    // overhang puts its eave tip at 100 + 6 x sqrt(1 + (2/3)^2) - 16 x 2/3.
    let tip = seated_eave_elevation(100.0, 6.0, 8.0, 16.0);
    let want = 100.0 + 6.0 * (1.0f64 + (8.0f64 / 12.0).powi(2)).sqrt() - 16.0 * 8.0 / 12.0;
    assert!((tip - want).abs() < 1e-9);
    // The first-hip rule of plate_baseline gives the same for that plane.
    let spec = EdgeRoofSpec::default();
    assert!((plate_baseline(&[spec], 100.0, 6.0) - tip).abs() < 1e-9);
}

#[test]
fn heel_height_counts_for_trusses_and_the_cut_for_rafters() {
    let mut h = HeightSettings {
        heel_height: 4.0,
        birdsmouth_cut: -3.0,
        ..HeightSettings::default()
    };
    assert_eq!(h.plate_lift(), 0.0, "automatic birdsmouth adds nothing");
    h.auto_birdsmouth = false;
    assert_eq!(h.plate_lift(), -3.0);
    h.framing = RoofFraming::Trusses;
    assert_eq!(h.plate_lift(), 4.0);
    assert!(!h.has_birdsmouth(0.0), "trusses have no birdsmouth");
    h.framing = RoofFraming::Rafters;
    assert!(h.has_birdsmouth(0.0));
    assert!(h.has_birdsmouth(-3.0));
    assert!(!h.has_birdsmouth(NO_BIRDSMOUTH_RAISE));
}

fn plane_8_12() -> PlaneHeights {
    PlaneHeights {
        baseline: 109.0,
        pitch: 8.0,
        run: 144.0,
        overhang: 16.0,
        thickness: 6.0,
        plate_top: 100.0,
        plate_width: 4.5,
        shadow_rise: 0.0,
    }
}

#[test]
fn the_four_heights_follow_the_slope() {
    let p = plane_8_12();
    assert!((p.ridge_top() - (109.0 + 96.0)).abs() < 1e-9);
    assert!((p.fascia_top() - (109.0 - 16.0 * 8.0 / 12.0)).abs() < 1e-9);
    assert!((p.height(HeightLock::TopOfPlate) - 100.0).abs() < 1e-9);
    assert!((p.vertical_depth() - 6.0 * (1.0f64 + (2.0f64 / 3.0).powi(2)).sqrt()).abs() < 1e-9);
}

#[test]
fn a_pitch_change_pivots_about_each_locked_height() {
    let p = plane_8_12();
    for lock in [
        HeightLock::RidgeTop,
        HeightLock::Baseline,
        HeightLock::FasciaTop,
    ] {
        let q = p.with_pitch(12.0, lock, true);
        assert_eq!(q.pitch, 12.0);
        assert!(
            (q.height(lock) - p.height(lock)).abs() < 1e-9,
            "{lock:?} must stay put"
        );
    }
    // Ridge locked: the baseline sinks as the plane steepens.
    assert!(p.with_pitch(12.0, HeightLock::RidgeTop, true).baseline < p.baseline);
    // Fascia locked: the baseline rises as the plane steepens.
    assert!(p.with_pitch(12.0, HeightLock::FasciaTop, true).baseline > p.baseline);
}

#[test]
fn top_of_plate_lock_pivots_about_the_inside_edge_with_the_automatic_cut() {
    // Start seated the automatic way: underside on the plate's inside edge.
    let mut p = plane_8_12();
    p.baseline = p.plate_top + p.vertical_depth() - p.plate_width * 8.0 / 12.0;
    assert!((p.birdsmouth_depth() - 4.5 * 8.0 / 12.0).abs() < 1e-9);
    assert!((p.birdsmouth_seat() - 4.5).abs() < 1e-9);
    let q = p.with_pitch(12.0, HeightLock::TopOfPlate, true);
    // Still seated on the inside edge: the seat is the plate width, the
    // cut deepens with the pitch.
    assert!((q.birdsmouth_seat() - 4.5).abs() < 1e-9);
    assert!((q.birdsmouth_depth() - 4.5).abs() < 1e-9);
    assert_eq!(q.plate_top, p.plate_top);
}

#[test]
fn top_of_plate_lock_pivots_about_the_outside_edge_without_it() {
    let p = plane_8_12();
    let q = p.with_pitch(12.0, HeightLock::TopOfPlate, false);
    // The underside stays at the same height on the baseline, so the
    // birdsmouth depth is unchanged.
    assert!((q.underside_at_baseline() - p.underside_at_baseline()).abs() < 1e-9);
    assert!((q.birdsmouth_depth() - p.birdsmouth_depth()).abs() < 1e-9);
}

#[test]
fn raising_a_locked_pitch_plane_one_inch_shallows_the_birdsmouth_by_one_inch() {
    let p = plane_8_12();
    let q = p.with_height(HeightLock::Baseline, p.baseline + 1.0);
    assert!((p.birdsmouth_depth() - q.birdsmouth_depth() - 1.0).abs() < 1e-9);
    let r = p.with_height(HeightLock::RidgeTop, p.ridge_top() + 1.0);
    assert!((r.baseline - (p.baseline + 1.0)).abs() < 1e-9);
    let f = p.with_height(HeightLock::FasciaTop, p.fascia_top() - 2.0);
    assert!((f.baseline - (p.baseline - 2.0)).abs() < 1e-9);
}

#[test]
fn birdsmouth_cut_and_seat_convert_at_the_pitch() {
    assert!((birdsmouth_cut_for_seat(6.0, 8.0) - 4.0).abs() < 1e-9);
    assert!((birdsmouth_seat_for_cut(4.0, 8.0) - 6.0).abs() < 1e-9);
    assert_eq!(birdsmouth_seat_for_cut(4.0, 0.0), 0.0);
}

#[test]
fn projected_and_actual_edge_lengths_convert_at_the_pitch() {
    // 12:12 is 45 degrees: the sloped edge is sqrt(2) longer in plan terms.
    assert!((actual_length(100.0, 12.0) - 100.0 * 2.0f64.sqrt()).abs() < 1e-9);
    assert!((projected_length(actual_length(87.5, 8.0), 8.0) - 87.5).abs() < 1e-9);
}

// ---- Round 16 brief 18b: placing planes, edge entry, In From Baseline ----

/// A plane rising from the baseline `(x0, y0) -> (x1, y0)` toward +y at
/// `pitch`, `depth` inches deep, its eave at elevation `z`.
fn shed_plane(x0: f64, x1: f64, y0: f64, depth: f64, pitch: f64, z: f64) -> RoofPlane {
    let rise = depth * pitch / 12.0;
    RoofPlane {
        polygon3d: vec![
            [x0, z, -y0],
            [x1, z, -y0],
            [x1, z + rise, -(y0 + depth)],
            [x0, z + rise, -(y0 + depth)],
        ],
        pitch_in_12: pitch,
        baseline: (Point::new(x0, y0), Point::new(x1, y0)),
        source_edge: 0,
    }
}

#[test]
fn move_to_be_coplanar_raises_a_parallel_plane_into_the_other() {
    // A low plane in front of a taller one at the same 6:12 slope: its
    // baseline sits 12" below the plane of the back plane.
    let back = shed_plane(0.0, 240.0, 100.0, 120.0, 6.0, 120.0);
    let front = shed_plane(0.0, 240.0, 40.0, 60.0, 6.0, 100.0);
    // The back plane at y = 40 is 100 + (40 - 100) * 0.5 = 90.
    let shift = coplanar_shift(&front, &back).expect("coplanar");
    assert!(
        (shift - (90.0 - 100.0 - (120.0 - 120.0))).abs() < 1e-9,
        "{shift}"
    );
    // Moved, the front baseline lies in the back plane.
    let mut moved = front.clone();
    for v in &mut moved.polygon3d {
        v[1] += shift;
    }
    let want = back.height_at(Point::new(0.0, 40.0)).unwrap();
    assert!((moved.polygon3d[0][1] - want).abs() < 1e-9);
}

#[test]
fn coplanar_moves_refuse_planes_that_cannot_be_one() {
    let a = shed_plane(0.0, 240.0, 0.0, 100.0, 6.0, 100.0);
    let steeper = shed_plane(0.0, 240.0, 200.0, 100.0, 9.0, 100.0);
    assert_eq!(
        coplanar_shift(&a, &steeper),
        Err(PlacementError::DifferentPitch)
    );
    // Baselines at an angle.
    let mut turned = shed_plane(0.0, 240.0, 200.0, 100.0, 6.0, 100.0);
    turned.baseline.1 = Point::new(240.0, 260.0);
    assert_eq!(
        coplanar_shift(&a, &turned),
        Err(PlacementError::NotParallel)
    );
    // A plane that rises the other way (baseline at the back, polygon in front).
    let mut away = shed_plane(0.0, 240.0, 300.0, 100.0, 6.0, 100.0);
    for v in &mut away.polygon3d {
        v[2] = -(600.0 - (-v[2]));
    }
    away.baseline = (Point::new(0.0, 300.0), Point::new(240.0, 300.0));
    assert_eq!(coplanar_shift(&a, &away), Err(PlacementError::FacingApart));
}

#[test]
fn an_edge_meets_another_plane_at_the_intersection_point() {
    let slope = shed_plane(0.0, 240.0, 0.0, 200.0, 6.0, 100.0);
    // A level edge never meets a level plane at another height.
    let flat = RoofPlane {
        polygon3d: vec![
            [300.0, 130.0, 0.0],
            [400.0, 130.0, 0.0],
            [400.0, 130.0, -50.0],
            [300.0, 130.0, -50.0],
        ],
        pitch_in_12: 0.0,
        baseline: (Point::new(300.0, 0.0), Point::new(400.0, 0.0)),
        source_edge: 0,
    };
    assert!(edge_plane_point(&slope, 0, &flat).is_none(), "parallel");
    // Edge 1 of the slope goes (240,100,0) -> (240,200,-200): it climbs 100
    // over 200 of plan; meets elevation 130 at plan y = 60.
    let level = RoofPlane {
        polygon3d: vec![
            [0.0, 130.0, 0.0],
            [10.0, 130.0, 0.0],
            [10.0, 130.0, -10.0],
            [0.0, 130.0, -10.0],
        ],
        pitch_in_12: 0.0,
        baseline: (Point::new(0.0, 0.0), Point::new(10.0, 0.0)),
        source_edge: 0,
    };
    let q = edge_plane_point(&slope, 1, &level).expect("meets");
    assert!((q[1] - 130.0).abs() < 1e-9);
    assert!((-q[2] - 60.0).abs() < 1e-9, "plan y {}", -q[2]);
    assert!((q[0] - 240.0).abs() < 1e-9);
}

#[test]
fn a_baseline_over_a_plane_takes_the_wall_top_or_the_planes_height() {
    let under = shed_plane(0.0, 240.0, 0.0, 200.0, 6.0, 100.0);
    let at = Point::new(100.0, 80.0);
    assert!(baseline_lies_on(&under, at));
    assert!(!baseline_lies_on(&under, Point::new(100.0, 400.0)));
    let wall_top = 96.0;
    assert_eq!(
        baseline_height_over(BaselineOver::WallTop, wall_top, &under, at),
        96.0
    );
    assert!(
        (baseline_height_over(BaselineOver::ExistingPlane, wall_top, &under, at) - 140.0).abs()
            < 1e-9
    );
}

#[test]
fn typed_edge_lengths_read_as_projected_or_actual() {
    // A hip-like edge: 120 in plan, 90 of rise, so 150 along the slope.
    let a = [0.0, 0.0, 0.0];
    let b = [120.0, 90.0, 0.0];
    let (plan, actual) = edge_length(a, b);
    assert!((plan - 120.0).abs() < 1e-9 && (actual - 150.0).abs() < 1e-9);
    assert_eq!(
        plan_length_for_entry(a, b, 100.0, LengthEntry::Projected),
        100.0
    );
    assert!((plan_length_for_entry(a, b, 100.0, LengthEntry::Actual) - 80.0).abs() < 1e-9);
    // A level edge reads the same both ways.
    let c = [60.0, 0.0, 0.0];
    assert_eq!(plan_length_for_entry(a, c, 42.0, LengthEntry::Actual), 42.0);
    let (p, s) = perimeter(&shed_plane(0.0, 240.0, 0.0, 100.0, 12.0, 0.0).polygon3d);
    assert!((p - 680.0).abs() < 1e-9);
    assert!(s > p);
}

#[test]
fn in_from_baseline_and_starts_at_height_are_two_views_of_one_number() {
    // 108" wall, 6:12 lower pitch: 48" in from the baseline is 24" higher.
    let h = start_height_for_in_from_baseline(108.0, 6.0, 48.0);
    assert!((h - 132.0).abs() < 1e-9);
    assert!((in_from_baseline_for_start_height(108.0, 6.0, h) - 48.0).abs() < 1e-9);
    assert_eq!(in_from_baseline_for_start_height(108.0, 6.0, 100.0), 0.0);
    assert_eq!(in_from_baseline_for_start_height(108.0, 0.0, 200.0), 0.0);
}

#[test]
fn make_parallel_and_perpendicular_turn_an_edge_the_short_way() {
    let east = Point::new(1.0, 0.0);
    let tilted = Point::new(1.0, 0.1);
    let t = turn_to_align(tilted, east, false);
    assert!((t + 0.1f64.atan()).abs() < 1e-9, "{t}");
    let u = turn_to_align(tilted, east, true);
    assert!(
        (u - (std::f64::consts::FRAC_PI_2 - 0.1f64.atan())).abs() < 1e-9,
        "{u}"
    );
    // Already the other way round: no more than a quarter turn.
    let back = turn_to_align(Point::new(-1.0, 0.0), east, false);
    assert!(back.abs() < 1e-9 || (back.abs() - std::f64::consts::PI).abs() > 1e-9);
    assert!(turn_to_align(Point::new(0.3, 1.0), east, false).abs() <= std::f64::consts::FRAC_PI_2);
}

#[test]
fn a_baseline_snaps_to_the_outside_of_a_parallel_wall() {
    let wall = WallSurface {
        a: Point::new(0.0, 0.0),
        b: Point::new(480.0, 0.0),
    };
    // 5" off the surface and a degree out of parallel: both ends land on it.
    let (a, b) = snap_to_wall_surface(Point::new(10.0, 5.0), Point::new(300.0, 10.0), &[wall]);
    assert!(a.y.abs() < 1e-9 && b.y.abs() < 1e-9);
    assert!((a.x - 10.0).abs() < 1e-9 && (b.x - 300.0).abs() < 1e-9);
    // Too far, or not parallel: left alone.
    let far = (Point::new(10.0, 40.0), Point::new(300.0, 40.0));
    assert_eq!(snap_to_wall_surface(far.0, far.1, &[wall]), far);
    let skew = (Point::new(10.0, 2.0), Point::new(300.0, 100.0));
    assert_eq!(snap_to_wall_surface(skew.0, skew.1, &[wall]), skew);
}

#[test]
fn the_shadow_board_top_rides_on_the_fascia_and_pivots_with_it() {
    let mut h = plane_8_12();
    h.shadow_rise = 1.5;
    assert!((h.shadow_board_top() - (h.fascia_top() + 1.5)).abs() < 1e-9);
    // Typing a shadow board top moves the plane with its pitch kept.
    let up = h.with_height(HeightLock::ShadowBoardTop, h.shadow_board_top() + 6.0);
    assert!((up.shadow_board_top() - (h.shadow_board_top() + 6.0)).abs() < 1e-9);
    assert!((up.pitch - h.pitch).abs() < 1e-9);
    // Locked, a pitch change leaves the shadow board top where it was.
    let steeper = h.with_pitch(12.0, HeightLock::ShadowBoardTop, true);
    assert!((steeper.shadow_board_top() - h.shadow_board_top()).abs() < 1e-9);
}
