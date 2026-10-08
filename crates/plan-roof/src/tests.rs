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
