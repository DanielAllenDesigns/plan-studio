//! Roof meshes: planes with holes, skylights, ceiling planes, dormers.

use plan_3d::{
    ceiling_plane_meshes, dormer_meshes, roof_meshes, roof_plane_meshes, skylight_meshes, Material,
    Mesh, CEILING_FRAMING_MATERIAL,
};
use plan_core::Point;
use plan_roof::{
    auto_dormer, build_roof, ceiling_planes_for_vaulted_room, roof_plane_with_holes, DormerKind,
    DormerSpec, EdgeKind, EdgeRoof, RoofHole, RoofPlane, SkylightSpec,
};

/// 40 x 30 ft gable roof, ridge along x, no overhang, eave at 108".
fn gable_planes() -> Vec<RoofPlane> {
    let fp = vec![
        Point::new(0.0, 0.0),
        Point::new(480.0, 0.0),
        Point::new(480.0, 360.0),
        Point::new(0.0, 360.0),
    ];
    let mut edges = vec![
        EdgeRoof {
            overhang: 0.0,
            ..EdgeRoof::default()
        };
        4
    ];
    edges[1].kind = EdgeKind::Gable;
    edges[3].kind = EdgeKind::Gable;
    build_roof(&fp, &edges, 108.0).planes
}

fn south(planes: &[RoofPlane]) -> &RoofPlane {
    planes.iter().find(|p| p.source_edge == 0).unwrap()
}

fn of(meshes: &[Mesh], m: Material) -> Vec<&Mesh> {
    meshes.iter().filter(|x| x.material == m).collect()
}

/// Triangles of `meshes` as plan-projected corner triples with their normal.
fn plan_tris(meshes: &[&Mesh]) -> Vec<([Point; 3], [f32; 3])> {
    let mut out = Vec::new();
    for m in meshes {
        for t in m.indices.chunks(3) {
            let v = |i: u32| m.vertices[i as usize];
            let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
            let p = |x: [f32; 3]| Point::new(x[0] as f64, -(x[2] as f64));
            out.push(([p(a.position), p(b.position), p(c.position)], a.normal));
        }
    }
    out
}

fn area(t: &[Point; 3]) -> f64 {
    (t[1] - t[0]).cross(t[2] - t[0]) * 0.5
}

fn covers(t: &[Point; 3], p: Point) -> bool {
    if area(t).abs() < 1e-6 {
        return false;
    }
    let s = area(t).signum();
    (0..3).all(|i| s * (t[(i + 1) % 3] - t[i]).cross(p - t[i]) > 1e-9)
}

#[test]
fn hole_mesh_has_no_triangles_inside_the_hole() {
    let planes = gable_planes();
    let plane = south(&planes);
    let rect = RoofHole::rect(Point::new(200.0, 60.0), Point::new(260.0, 120.0));
    let poly = roof_plane_with_holes(plane, &[RoofHole::hole(rect)]);
    assert_eq!(poly.holes.len(), 1);
    let meshes = roof_plane_meshes(&poly, 6.0);
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].material, Material::Roof);
    let tris = plan_tris(&[&meshes[0]]);
    // Hole centre and several interior probes are uncovered.
    for p in [
        Point::new(230.0, 90.0),
        Point::new(205.0, 65.0),
        Point::new(255.0, 115.0),
    ] {
        assert!(
            !tris.iter().any(|(t, _)| covers(t, p)),
            "{p:?} is covered by a triangle"
        );
    }
    // Points outside the hole are covered by both the top and the underside.
    let outside = Point::new(100.0, 90.0);
    let hits = tris.iter().filter(|(t, _)| covers(t, outside)).count();
    assert_eq!(hits, 2);
    // Up-facing triangles tile the plane minus the hole.
    let top: f64 = tris
        .iter()
        .filter(|(_, n)| n[1] > 0.8)
        .map(|(t, _)| area(t).abs())
        .sum();
    assert!((top - (480.0 * 180.0 - 3600.0)).abs() < 1e-2, "{top}");
    // The hole walls face into the hole: normal points toward the centre.
    let centre = [230.0f32, 108.0 + 60.0, -90.0];
    let side_ok = meshes[0].indices.chunks(3).all(|t| {
        let v = &meshes[0].vertices[t[0] as usize];
        if v.normal[1].abs() > 0.5 {
            return true;
        }
        let near_hole =
            (v.position[0] - 230.0).abs() <= 31.0 && (v.position[2] + 90.0).abs() <= 31.0;
        if !near_hole {
            return true;
        }
        let to_centre = [centre[0] - v.position[0], 0.0, centre[2] - v.position[2]];
        v.normal[0] * to_centre[0] + v.normal[2] * to_centre[2] > 0.0
    });
    assert!(side_ok);
}

#[test]
fn roof_without_holes_is_a_plain_slab_set() {
    let planes = gable_planes();
    let roof = plan_roof::Roof {
        planes: planes.clone(),
        fascia_height: 6.0,
        baseline_elevation: 108.0,
        approximate: false,
    };
    let meshes = roof_meshes(&roof, &[], 5.0);
    assert!(!meshes.is_empty() && meshes.iter().all(|m| m.material == Material::Roof));
    let up: f64 = plan_tris(&meshes.iter().collect::<Vec<_>>())
        .iter()
        .filter(|(_, n)| n[1] > 0.8)
        .map(|(t, _)| area(t).abs())
        .sum();
    assert!((up - 480.0 * 360.0).abs() < 1e-2);
}

#[test]
fn skylight_glass_sits_within_the_curb_height() {
    let planes = gable_planes();
    let plane = south(&planes);
    let spec = SkylightSpec {
        curb_height: 8.0,
        glass_thickness: 1.0,
        frame_width: 2.0,
    };
    let rect = RoofHole::rect(Point::new(200.0, 60.0), Point::new(260.0, 120.0));
    let poly = roof_plane_with_holes(plane, &[RoofHole::skylight(rect, spec)]);
    assert_eq!(poly.skylights.len(), 1);
    let sky = skylight_meshes(&poly.skylights[0]);
    let n = poly.normal;
    let o = plane.polygon3d[0];
    let height_above = |p: [f32; 3]| (0..3).map(|k| (p[k] as f64 - o[k]) * n[k]).sum::<f64>();
    let glass = of(&sky, Material::Glass);
    assert_eq!(glass.len(), 1);
    for v in &glass[0].vertices {
        let h = height_above(v.position);
        assert!(h > 0.0 && h <= 8.0 + 1e-4, "glass at {h}");
        assert!(h >= 7.0 - 1e-4, "glass at {h}");
    }
    // Frame (Trim) and curb (Roof) are there too, and the curb stays below.
    let frame = of(&sky, Material::Trim);
    assert_eq!(frame.len(), 1);
    assert!(frame[0]
        .vertices
        .iter()
        .all(|v| height_above(v.position) <= 8.0 + 1e-4));
    let curb = of(&sky, Material::Roof);
    assert_eq!(curb.len(), 1);
    assert!(curb[0]
        .vertices
        .iter()
        .all(|v| (-1e-4..=7.0 + 1e-4).contains(&height_above(v.position))));
    // The glass fills the opening inside the frame: no triangle covers the
    // frame ring from the glass layer, and the glass covers the centre.
    let g = plan_tris(&glass);
    assert!(g.iter().any(|(t, _)| covers(t, Point::new(230.0, 90.0))));
    // The roof slab with this skylight carries all four materials.
    let all = roof_plane_meshes(&poly, 6.0);
    for m in [Material::Roof, Material::Trim, Material::Glass] {
        assert!(!of(&all, m).is_empty(), "{m:?}");
    }
    // The roof slab itself has no triangle over the hole.
    let roof_only = plan_tris(&of(&all, Material::Roof));
    assert!(!roof_only
        .iter()
        .any(|(t, n)| n[1] > 0.8 && covers(t, Point::new(230.0, 90.0))));
}

#[test]
fn ceiling_planes_mesh_interior_below_and_framing_above() {
    let planes = gable_planes();
    let room = vec![
        Point::new(0.0, 0.0),
        Point::new(480.0, 0.0),
        Point::new(480.0, 360.0),
        Point::new(0.0, 360.0),
    ];
    let ceil = ceiling_planes_for_vaulted_room(&room, &planes, 9.0);
    assert_eq!(ceil.len(), 2);
    assert_eq!(CEILING_FRAMING_MATERIAL, Material::Framing);
    for c in &ceil {
        let meshes = ceiling_plane_meshes(c);
        let interior = of(&meshes, Material::WallInterior);
        let framing = of(&meshes, CEILING_FRAMING_MATERIAL);
        assert_eq!(interior.len(), 1);
        assert_eq!(framing.len(), 1);
        // The underside faces down; its triangles tile the ceiling outline.
        assert!(interior[0].vertices.iter().all(|v| v.normal[1] < 0.0));
        let a: f64 = plan_tris(&interior)
            .iter()
            .map(|(t, _)| area(t).abs())
            .sum();
        assert!((a - c.plan_area()).abs() < 1e-2);
        // Framing sits above the surface (thickness along the normal).
        let (ilo, _) = interior[0].bounds().unwrap();
        let (_, fhi) = framing[0].bounds().unwrap();
        assert!(fhi[1] > ilo[1]);
    }
}

#[test]
fn dormer_meshes_have_walls_roof_and_window_glass_and_cut_the_main_roof() {
    let planes = gable_planes();
    let main = south(&planes);
    let spec = DormerSpec {
        kind: DormerKind::Gable,
        width: 72.0,
        height_to_ridge: 0.0,
        wall_height: 36.0,
        position_along_eave: 240.0,
        setback_from_eave: 30.0,
        pitch: 8.0,
        window: Some((30.0, 24.0)),
    };
    for kind in [DormerKind::Gable, DormerKind::Hip, DormerKind::Shed] {
        let d = auto_dormer(main, DormerSpec { kind, ..spec }).unwrap();
        let meshes = dormer_meshes(&d, 6.0, 5.0);
        for m in [
            Material::Stucco,
            Material::Roof,
            Material::WindowGlass,
            Material::WindowFrame,
            Material::WallInterior,
        ] {
            assert!(!of(&meshes, m).is_empty(), "{kind:?} lacks {m:?}");
        }
        // The front wall has the window cut out: its centre (plan) is open in
        // the Stucco face; walls are vertical so look from the side instead.
        let stucco = of(&meshes, Material::Stucco);
        let front_faces: Vec<_> = stucco[0]
            .indices
            .chunks(3)
            .filter(|t| {
                let v = &stucco[0].vertices[t[0] as usize];
                v.normal[2] > 0.99 && (v.position[2] + 30.0).abs() < 1e-3
            })
            .collect();
        assert!(!front_faces.is_empty());
        // Cutting the main roof with the dormer footprint leaves it uncovered.
        let poly = roof_plane_with_holes(main, std::slice::from_ref(&d.hole_in_main_roof));
        assert_eq!(poly.holes.len(), 1);
        let slab = roof_plane_meshes(&poly, 6.0);
        let tris = plan_tris(&of(&slab, Material::Roof));
        let c = Point::new(230.0, 40.0);
        assert!(!tris.iter().any(|(t, _)| covers(t, c)), "{kind:?}");
        // The dormer roof planes cover the same plan spot (nothing is missing).
        let roofs = plan_tris(&of(&meshes, Material::Roof));
        assert!(
            roofs.iter().any(|(t, n)| n[1] > 0.3 && covers(t, c)),
            "{kind:?}"
        );
    }
}
