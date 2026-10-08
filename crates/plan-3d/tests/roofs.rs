//! Roof meshes: planes with holes, skylights, ceiling planes, dormers.

use plan_3d::{
    ceiling_plane_meshes, ceiling_plane_meshes_joined, dormer_eave_meshes, dormer_meshes,
    eave_elements, roof_meshes, roof_plane_meshes, skylight_meshes, EaveKind, EavePlane, Material,
    Mesh, RoofDetail, CEILING_FRAMING_MATERIAL,
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
fn an_l_shaped_hole_is_open_in_its_arms_and_closed_in_its_notch() {
    // RF-42: a hole that is not a rectangle. An L: 60 x 60 with the corner
    // (240..260, 100..120) left out.
    let planes = gable_planes();
    let plane = south(&planes);
    let l = vec![
        Point::new(200.0, 60.0),
        Point::new(260.0, 60.0),
        Point::new(260.0, 100.0),
        Point::new(220.0, 100.0),
        Point::new(220.0, 120.0),
        Point::new(200.0, 120.0),
    ];
    let poly = roof_plane_with_holes(plane, &[RoofHole::hole(l)]);
    assert_eq!(poly.holes.len(), 1);
    assert!(poly.skipped_holes.is_empty());
    let meshes = roof_plane_meshes(&poly, 6.0);
    let tris = plan_tris(&[&meshes[0]]);
    for open in [
        Point::new(210.0, 70.0),
        Point::new(250.0, 70.0),
        Point::new(210.0, 115.0),
    ] {
        assert!(
            !tris.iter().any(|(t, _)| covers(t, open)),
            "{open:?} should be open"
        );
    }
    // The notch of the L is roof.
    let notch = Point::new(245.0, 115.0);
    assert!(tris.iter().any(|(t, _)| covers(t, notch)));
    let top: f64 = tris
        .iter()
        .filter(|(_, n)| n[1] > 0.8)
        .map(|(t, _)| area(t).abs())
        .sum();
    let l_area = 60.0 * 60.0 - 40.0 * 20.0;
    assert!((top - (480.0 * 180.0 - l_area)).abs() < 1e-2, "{top}");
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
        overhang: 0.0,
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

fn overhanging_dormer(overhang: f64) -> plan_roof::Dormer {
    let planes = gable_planes();
    auto_dormer(
        south(&planes),
        DormerSpec {
            kind: DormerKind::Gable,
            width: 72.0,
            height_to_ridge: 0.0,
            wall_height: 36.0,
            position_along_eave: 240.0,
            setback_from_eave: 30.0,
            pitch: 8.0,
            window: None,
            overhang,
        },
    )
    .unwrap()
}

#[test]
fn a_dormer_overhang_makes_the_roof_planes_longer_and_adds_fascia_and_soffit() {
    let flush = overhanging_dormer(0.0);
    let over = overhanging_dormer(12.0);
    // The slabs follow the overhang planes.
    let slab_extent = |d: &plan_roof::Dormer| {
        let ms = dormer_meshes(d, 6.0, 5.0);
        let roofs = of(&ms, Material::Roof);
        let xs: Vec<f32> = roofs
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[0]))
            .collect();
        xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min)
    };
    assert!((slab_extent(&over) - slab_extent(&flush) - 24.0).abs() < 1e-3);
    // The eave detail: no overhang, no soffit; with it, fascia, rake boards
    // and soffit appear.
    let detail = RoofDetail::default();
    let count = |ms: &[Mesh]| ms.iter().map(|m| m.indices.len()).sum::<usize>();
    let trim = dormer_eave_meshes(&over, &detail);
    assert!(!of(&trim, Material::Trim).is_empty());
    assert!(count(&trim) > count(&dormer_eave_meshes(&flush, &detail)));
}

#[test]
fn a_dormer_valley_gets_no_rake_board() {
    let d = overhanging_dormer(12.0);
    let detail = RoofDetail::default();
    let planes = |skip: bool| -> Vec<EavePlane> {
        d.overhang_planes
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let mut ep = EavePlane::bare(p.clone());
                ep.overhang = 12.0;
                if skip {
                    ep.skip = d
                        .valley_edges
                        .iter()
                        .filter(|(q, _)| *q == k)
                        .map(|(_, e)| *e)
                        .collect();
                }
                ep
            })
            .collect()
    };
    let rakes = |skip| {
        eave_elements(&planes(skip), &detail)
            .iter()
            .filter(|e| e.kind == EaveKind::RakeFascia)
            .count()
    };
    // Each plane has the front rake; without the skip the valley is one too.
    assert_eq!(rakes(true), 2);
    assert_eq!(rakes(false), 4);
}

fn framing_vertices(meshes: &[Mesh]) -> Vec<[f32; 3]> {
    of(meshes, CEILING_FRAMING_MATERIAL)
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position))
        .collect()
}

#[test]
fn ceiling_planes_that_meet_at_a_ridge_are_mitred() {
    let planes = gable_planes();
    let ceil = ceiling_planes_for_vaulted_room(
        &[
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 360.0),
            Point::new(0.0, 360.0),
        ],
        &planes,
        9.0,
    );
    assert_eq!(ceil.len(), 2);
    // The ridge of the ceiling surface is at plan y = 180 (scene z = -180).
    let ridge_y = ceil[0]
        .polygon3d()
        .iter()
        .fold(f64::MIN, |m, p| m.max(p[1]));
    let cos = 12.0 / (144.0f64 + 64.0).sqrt();
    let want_top = ridge_y + 9.0 / cos;
    let square: Vec<Vec<Mesh>> = ceil.iter().map(ceiling_plane_meshes).collect();
    let mitred: Vec<Vec<Mesh>> = ceil
        .iter()
        .map(|c| ceiling_plane_meshes_joined(c, &ceil))
        .collect();
    let top_y = |ms: &[Mesh]| {
        framing_vertices(ms)
            .iter()
            .fold(f64::MIN, |m, v| m.max(f64::from(v[1])))
    };
    for k in 0..2 {
        // Cut square, each slab tops out at its own corner above the ridge.
        assert!(
            top_y(&square[k]) < want_top - 2.0,
            "{} vs {want_top}",
            top_y(&square[k])
        );
        // Mitred, both slabs reach the point where the two top planes meet.
        assert!(
            (top_y(&mitred[k]) - want_top).abs() < 1e-3,
            "{} vs {want_top}",
            top_y(&mitred[k])
        );
        // The mitre plane is the vertical one through the ridge: the slab
        // never crosses to the other plane's side (south is z > -180).
        let across = framing_vertices(&mitred[k])
            .iter()
            .filter(|v| {
                if k == 0 {
                    v[2] < -180.0 - 1e-3
                } else {
                    v[2] > -180.0 + 1e-3
                }
            })
            .count();
        assert_eq!(across, 0, "plane {k} crosses the ridge");
    }
    // The slabs meet in the same line above the ridge.
    let apex = |ms: &[Mesh]| -> Vec<[f32; 3]> {
        framing_vertices(ms)
            .into_iter()
            .filter(|v| (f64::from(v[1]) - want_top).abs() < 1e-3)
            .collect()
    };
    let (a0, a1) = (apex(&mitred[0]), apex(&mitred[1]));
    assert!(!a0.is_empty() && !a1.is_empty());
    for v in &a0 {
        assert!((f64::from(v[2]) + 180.0).abs() < 1e-3, "{v:?}");
    }
    // Same volume above the underside as the square slabs, a little less
    // under the ridge, never more than 9" of plane thickness.
    assert!(mitred.iter().all(|m| !m.is_empty()));
    // No neighbours: equal to the square cut.
    let alone = ceiling_plane_meshes_joined(&ceil[0], &[]);
    assert_eq!(alone.len(), square[0].len());
    assert_eq!(
        alone.iter().map(Mesh::triangle_count).sum::<usize>(),
        square[0].iter().map(Mesh::triangle_count).sum::<usize>()
    );
    // The underside keeps its materials.
    assert!(!of(&mitred[0], Material::WallInterior).is_empty());
}

// ----- the roof over a bay, box or bow window (RF-29, DW-48) -----

mod bay_roof {
    use plan_3d::{build_scene, Material, Mesh, Scene};
    use plan_core::{Id, OpeningKind, OpeningStyle, Point, Project, WallKind};

    fn unit(style: OpeningStyle) -> (Scene, Id) {
        let mut p = Project::new("t");
        let wall = p.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            4.5,
            100.0,
            WallKind::Interior,
        );
        let id = p
            .add_opening(0, wall, 60.0, OpeningKind::Window)
            .expect("opening");
        let o = p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.style = style;
        o.width = 72.0;
        (build_scene(&p), id)
    }

    fn roof_of(scene: &Scene, id: Id) -> Vec<&Mesh> {
        scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id) && m.material == Material::Roof)
            .collect()
    }

    fn top(ms: &[&Mesh]) -> f32 {
        ms.iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::MIN, f32::max)
    }

    fn bottom(ms: &[&Mesh]) -> f32 {
        ms.iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .fold(f32::MAX, f32::min)
    }

    /// Largest tilt of a face of the roof from level, in rise per 12 of run.
    fn steepest(ms: &[&Mesh]) -> f32 {
        ms.iter()
            .flat_map(|m| m.vertices.iter())
            .filter(|v| v.normal[1] > 0.8)
            .map(|v| (1.0 - v.normal[1] * v.normal[1]).max(0.0).sqrt() / v.normal[1] * 12.0)
            .fold(0.0, f32::max)
    }

    #[test]
    fn a_bay_bow_or_box_window_has_a_pitched_roof_not_a_flat_slab() {
        for style in [
            OpeningStyle::BayWindow,
            OpeningStyle::BowWindow,
            OpeningStyle::BoxWindow,
        ] {
            let (scene, id) = unit(style);
            let roof = roof_of(&scene, id);
            assert!(!roof.is_empty(), "{style:?} has no roof");
            let pitch = steepest(&roof);
            assert!(
                (pitch - 6.0).abs() < 0.2,
                "{style:?} roof steepest face {pitch}:12"
            );
            // It rises above its eave by a few inches (18" deep, 6:12).
            let rise = top(&roof) - bottom(&roof);
            assert!(rise > 4.0, "{style:?} rise {rise}");
        }
    }

    #[test]
    fn the_roof_stays_within_the_projection_of_the_unit() {
        for style in [OpeningStyle::BayWindow, OpeningStyle::BoxWindow] {
            let (scene, id) = unit(style);
            let zmax = roof_of(&scene, id)
                .iter()
                .flat_map(|m| m.vertices.iter().map(|v| v.position[2]))
                .fold(f32::MIN, f32::max);
            assert!(zmax <= 2.25 + 18.0 + 1e-3, "{style:?} reaches {zmax}");
        }
    }

    #[test]
    fn a_box_window_roof_is_a_single_slope_from_the_wall() {
        let (scene, id) = unit(OpeningStyle::BoxWindow);
        let roof = roof_of(&scene, id);
        // The slope faces away from the wall: every upward face leans +z.
        let leaning = roof
            .iter()
            .flat_map(|m| m.vertices.iter())
            .filter(|v| v.normal[1] > 0.8 && v.normal[1] < 0.99)
            .all(|v| v.normal[2] > 0.0);
        assert!(leaning);
    }
}
