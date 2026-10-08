//! Walls build up to the roof (RF-13..RF-16, RF-20, RF-23): gable triangles,
//! walls clipped under hips, attic walls above a lower roof, butting roofs,
//! soffit and fascia.

use plan_3d::{
    build_scene, build_scene_covered, build_scene_with, eave_detail_meshes, eave_elements,
    EaveKind, EavePlane, Material, Mesh, RoofCover, RoofDetail, Scene, SceneOptions,
};
use plan_core::defaults::RoofWallKind;
use plan_core::geometry::point_in_polygon;
use plan_core::{Floor, Id, OpeningKind, Point, Project, WallKind};
use plan_roof::{build_roof, EdgeKind, EdgeRoof, Roof, RoofPlane};
use serde_json::{json, Value};

const W: f64 = 480.0; // 40 ft
const D: f64 = 360.0; // 30 ft
const PLATE: f64 = 96.0;
const THICK: f64 = 6.0;
/// Overhang measured from the wall centerline (16" from the face + half the wall).
const OVER: f64 = 16.0 + THICK * 0.5;
const SLAB: f64 = 6.0;

fn corners() -> [Point; 4] {
    [
        Point::new(0.0, 0.0),
        Point::new(W, 0.0),
        Point::new(W, D),
        Point::new(0.0, D),
    ]
}

/// Four exterior walls on floor 0, wall `i` along footprint edge `i`.
fn house() -> (Project, [Id; 4]) {
    let mut p = Project::new("house");
    let c = corners();
    let mut ids = [0; 4];
    for i in 0..4 {
        ids[i] = p.add_wall(0, c[i], c[(i + 1) % 4], THICK, PLATE, WallKind::Exterior);
    }
    (p, ids)
}

fn edge(kind: EdgeKind) -> EdgeRoof {
    EdgeRoof {
        pitch_in_12: 8.0,
        kind,
        overhang: OVER,
    }
}

fn plane_json(plane: &RoofPlane, id: Id, caps: bool) -> Value {
    json!({
        "kind": "plane",
        "id": id,
        "polygon3d": plane.polygon3d,
        "pitch": plane.pitch_in_12,
        "baseline": [plane.baseline.0, plane.baseline.1],
        "overhang": 16.0,
        "ridge_caps": caps,
    })
}

fn store_roof(floor: &mut Floor, roof: &Roof, caps: bool) {
    floor.roofs = roof
        .planes
        .iter()
        .enumerate()
        .map(|(i, pl)| plane_json(pl, 9000 + i as u64, caps))
        .collect();
}

fn gable_roof(baseline: f64) -> Roof {
    let e = [
        edge(EdgeKind::Hip),
        edge(EdgeKind::Gable),
        edge(EdgeKind::Hip),
        edge(EdgeKind::Gable),
    ];
    build_roof(&corners(), &e, baseline)
}

fn of(scene: &Scene, id: Id) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .collect()
}

fn max_y(meshes: &[&Mesh]) -> f32 {
    meshes
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MIN, f32::max)
}

fn min_y(meshes: &[&Mesh]) -> f32 {
    meshes
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .fold(f32::MAX, f32::min)
}

/// Total area of triangles whose geometric normal is mostly along `axis`.
fn face_area(meshes: &[&Mesh], axis: usize) -> f64 {
    let mut total = 0.0;
    for m in meshes {
        for t in m.indices.chunks(3) {
            let p = |i: u32| m.vertices[i as usize].position.map(f64::from);
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let (u, v) = (
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 0.0 && n[axis].abs() / len > 0.9 {
                total += len * 0.5;
            }
        }
    }
    total
}

#[test]
fn gable_end_walls_reach_the_ridge_and_eave_walls_stop_at_the_plate() {
    let (mut p, ids) = house();
    let roof = gable_roof(PLATE);
    assert_eq!(roof.planes.len(), 2);
    store_roof(&mut p.floors[0], &roof, false);
    for i in [1, 3] {
        p.floors[0].walls[i].roof.kind = RoofWallKind::FullGable;
    }
    let scene = build_scene(&p);
    // The ridge underside above a gable wall: ridge surface minus the slab.
    let ridge = roof
        .planes
        .iter()
        .flat_map(|pl| pl.polygon3d.iter().map(|v| v[1]))
        .fold(f64::MIN, f64::max);
    let ny = roof.planes[0].normal()[1];
    let underside = ridge - SLAB / ny;
    for i in [1, 3] {
        let top = f64::from(max_y(&of(&scene, ids[i])));
        assert!(
            (top - underside).abs() < 0.5,
            "gable wall {i} peaks at {top}, ridge underside {underside}"
        );
        // The triangle: the wall corners are lower than the peak.
        assert!(top > PLATE + 100.0);
    }
    for i in [0, 2] {
        let top = f64::from(max_y(&of(&scene, ids[i])));
        assert!((top - PLATE).abs() < 1e-3, "eave wall {i} stops at {top}");
    }
}

#[test]
fn a_gable_edge_in_the_roof_settings_raises_the_walls_along_it() {
    // Gable/Roof Line on an eave sets the edge override, not the wall's kind.
    let (mut p, ids) = house();
    store_roof(&mut p.floors[0], &gable_roof(PLATE), false);
    let c = corners();
    p.floors[0].roofs.push(json!({
        "kind": "settings",
        "edge_specs": [
            {"a": c[1], "b": c[2], "pitch": null, "overhang": null, "gable": true},
        ],
    }));
    let scene = build_scene(&p);
    assert!(max_y(&of(&scene, ids[1])) > PLATE as f32 + 100.0);
    assert!((max_y(&of(&scene, ids[3])) - PLATE as f32).abs() < 1e-3);
}

#[test]
fn a_gable_wall_has_vertices_along_the_slopes_not_only_at_the_peak() {
    let (mut p, ids) = house();
    store_roof(&mut p.floors[0], &gable_roof(PLATE), false);
    p.floors[0].walls[1].roof.kind = RoofWallKind::FullGable;
    let scene = build_scene(&p);
    let ys: Vec<f32> = of(&scene, ids[1])
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .collect();
    let peak = ys.iter().copied().fold(f32::MIN, f32::max);
    // Corners start at the underside at the wall's end, above the plate.
    let low = ys
        .iter()
        .copied()
        .filter(|y| *y > PLATE as f32 + 0.5)
        .fold(f32::MAX, f32::min);
    assert!(peak - low > 100.0, "peak {peak} corner {low}");
}

/// Every vertex of every wall under a plane lies on or below its underside.
fn assert_under_planes(scene: &Scene, roof: &Roof, ids: &[Id]) -> usize {
    let mut checked = 0;
    for &id in ids {
        for m in of(scene, id) {
            for v in &m.vertices {
                let at = Point::new(f64::from(v.position[0]), -f64::from(v.position[2]));
                for pl in &roof.planes {
                    if point_in_polygon(at, &pl.plan_polygon()) {
                        let under = pl.underside_at(at, SLAB).unwrap();
                        assert!(
                            f64::from(v.position[1]) <= under + 0.01,
                            "wall {id} vertex {:?} above the plane underside {under}",
                            v.position
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    checked
}

#[test]
fn a_hip_roof_clips_every_wall_to_the_plane_with_no_vertex_above_it() {
    let (mut p, ids) = house();
    // A roof set 36" below the top of the walls cuts all four, and a partition
    // across the building (under the slope, taller than the eaves) as well.
    let roof = build_roof(&corners(), &[edge(EdgeKind::Hip); 4], PLATE - 36.0);
    assert!(roof.planes.len() >= 4);
    store_roof(&mut p.floors[0], &roof, false);
    let part = p.add_wall(
        0,
        Point::new(240.0, 0.0),
        Point::new(240.0, D),
        4.5,
        150.0,
        WallKind::Interior,
    );
    let flat = build_scene_with(
        &p,
        &SceneOptions {
            roof_cuts_walls: false,
            ..SceneOptions::default()
        },
    );
    let scene = build_scene(&p);
    let mut all = ids.to_vec();
    all.push(part);
    let checked = assert_under_planes(&scene, &roof, &all);
    assert!(checked > 40, "only {checked} vertices were under a plane");
    // The eave walls came down from the plate.
    for id in ids {
        let (cut, boxy) = (max_y(&of(&scene, id)), max_y(&of(&flat, id)));
        assert!(
            cut < boxy - 20.0,
            "wall {id} top {cut} vs flat {boxy}: not clipped"
        );
    }
    // The partition's top is sloped: eave height at its ends, higher inside.
    let part_ys: Vec<f32> = of(&scene, part)
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .collect();
    assert!(max_y(&of(&scene, part)) > min_y(&of(&scene, part)) + 60.0);
    assert!(part_ys.iter().any(|y| *y > 100.0));
}

#[test]
fn openings_still_cut_a_clipped_wall() {
    let build = |window: bool| {
        let (mut p, ids) = house();
        store_roof(&mut p.floors[0], &gable_roof(PLATE), false);
        p.floors[0].walls[1].roof.kind = RoofWallKind::FullGable;
        if window {
            p.add_opening(0, ids[1], D * 0.5, OpeningKind::Window)
                .unwrap();
        }
        (build_scene(&p), ids[1])
    };
    let (plain, id) = build(false);
    let (with, _) = build(true);
    // The wall at x = W faces +-x: both faces lose the window's area.
    let before = face_area(&of(&plain, id), 0);
    let after = face_area(&of(&with, id), 0);
    let opening = {
        let o = plan_core::Opening::default_window(1, id, 0.0);
        o.width * o.height
    };
    assert!(
        (before - after - 2.0 * opening).abs() < 1.0,
        "face area {before} -> {after}, window {opening}"
    );
    // Its reveals were added.
    let reveals = with
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::WallInterior)
        .count();
    assert!(reveals > 0);
}

#[test]
fn walls_with_no_plane_above_keep_their_flat_top() {
    let (p, ids) = house();
    let scene = build_scene(&p);
    for id in ids {
        assert!((max_y(&of(&scene, id)) - PLATE as f32).abs() < 1e-4);
    }
    // And a project with a roof but opted out of cutting.
    let (mut q, ids) = house();
    store_roof(&mut q.floors[0], &gable_roof(PLATE), false);
    q.floors[0].walls[1].roof.kind = RoofWallKind::FullGable;
    let off = build_scene_with(
        &q,
        &SceneOptions {
            roof_cuts_walls: false,
            ..SceneOptions::default()
        },
    );
    assert!((max_y(&of(&off, ids[1])) - PLATE as f32).abs() < 1e-4);
}

#[test]
fn interior_walls_rise_to_a_vaulted_ceiling() {
    let (mut p, ids) = house();
    let _ = ids;
    // A ceiling plane over the west half rising 6:12 from y = 0.
    p.floors[0].roofs = vec![json!({
        "kind": "ceiling",
        "id": 7001,
        "outline": [
            Point::new(0.0, 0.0), Point::new(240.0, 0.0),
            Point::new(240.0, D), Point::new(0.0, D)
        ],
        "baseline": [Point::new(0.0, 0.0), Point::new(240.0, 0.0)],
        "pitch": 6.0,
        "height": 96.0,
        "thickness": 6.0,
    })];
    // The ceiling rises toward +y (left of baseline.0 -> baseline.1).
    let part = p.add_wall(
        0,
        Point::new(120.0, 0.0),
        Point::new(120.0, D),
        4.5,
        96.0,
        WallKind::Interior,
    );
    let scene = build_scene(&p);
    let top = f64::from(max_y(&of(&scene, part)));
    // 96 + 360 * 6 / 12 = 276 at the far end.
    assert!((top - 276.0).abs() < 0.5, "partition rises to {top}");
    let low = f64::from(min_y(&of(&scene, part)));
    assert!(low.abs() < 1e-3);
}

/// A shed roof rising to the north edge of a 240" square: one plane.
fn shed_roof(baseline: f64, pitch: f64) -> Roof {
    let sq = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 240.0),
        Point::new(0.0, 240.0),
    ];
    let mut e = [edge(EdgeKind::Shed); 4];
    e[0] = EdgeRoof {
        pitch_in_12: pitch,
        kind: EdgeKind::Hip,
        overhang: OVER,
    };
    build_roof(&sq, &e, baseline)
}

/// Floor 0 holds a low shed roof; a floor-1 wall at y = 240 stands taller.
fn lean_to() -> (Project, Id, Roof) {
    let mut p = Project::new("lean-to");
    p.floors.push(Floor::new("Second", 120.0));
    let roof = shed_roof(60.0, 2.0);
    assert_eq!(roof.planes.len(), 1, "one plane rising to the north");
    store_roof(&mut p.floors[0], &roof, false);
    let wall = p.add_wall(
        1,
        Point::new(-100.0, 240.0),
        Point::new(340.0, 240.0),
        THICK,
        96.0,
        WallKind::Exterior,
    );
    (p, wall, roof)
}

#[test]
fn a_lower_roof_is_trimmed_at_the_face_of_a_taller_wall() {
    let (p, wall, roof) = lean_to();
    let cover = RoofCover::from_project(&p);
    let eaves = &cover.floor(0).unwrap().eaves;
    assert_eq!(eaves.len(), 1);
    let plan_max_y = eaves[0]
        .plane
        .plan_polygon()
        .iter()
        .map(|q| q.y)
        .fold(f64::MIN, f64::max);
    // The wall is at y = 240, 6" thick: the plane ends just short of its south face.
    assert!(
        (plan_max_y - 236.5).abs() < 1e-6,
        "ends at y = {plan_max_y}"
    );
    let original_max = roof.planes[0]
        .plan_polygon()
        .iter()
        .map(|q| q.y)
        .fold(f64::MIN, f64::max);
    assert!(original_max > 240.0, "untrimmed plane overhangs the wall");
    assert_eq!(eaves[0].cuts.len(), 1, "one butt line for the flashing");
    // The taller wall is not cut by the lower roof.
    let scene = build_scene(&p);
    assert!((max_y(&of(&scene, wall)) - (120.0 + 96.0)).abs() < 1e-3);
}

#[test]
fn an_attic_wall_fills_the_gap_above_a_lower_roof() {
    let (p, _wall, _roof) = lean_to();
    let scene = build_scene(&p);
    let attic: Vec<&Mesh> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id.is_none() && m.material == Material::WallExterior)
        .collect();
    assert!(!attic.is_empty(), "no attic wall generated");
    // From the roof at the wall face up to the wall's bottom at 120".
    let face_h = 60.0 + (237.0 + OVER) / 12.0 * 2.0;
    assert!((f64::from(max_y(&attic)) - 120.0).abs() < 1e-3);
    assert!(
        (f64::from(min_y(&attic)) - face_h).abs() < 0.1,
        "attic wall starts at {} not {face_h}",
        min_y(&attic)
    );
    // Switched off, it is gone.
    let cover = RoofCover::from_project_with(
        &p,
        RoofDetail {
            auto_attic_walls: false,
            ..RoofDetail::default()
        },
    );
    let off = build_scene_covered(&p, &SceneOptions::default(), &[], &cover);
    assert!(off
        .meshes
        .iter()
        .all(|m| m.object_id.is_some() || m.material != Material::WallExterior));
}

#[test]
fn a_roof_beside_a_wall_it_does_not_clear_makes_no_attic_wall() {
    // Same lean-to but with the roof high enough to meet the wall above its bottom.
    let mut p = Project::new("high");
    p.floors.push(Floor::new("Second", 120.0));
    let roof = shed_roof(110.0, 2.0);
    store_roof(&mut p.floors[0], &roof, false);
    p.add_wall(
        1,
        Point::new(-100.0, 240.0),
        Point::new(340.0, 240.0),
        THICK,
        96.0,
        WallKind::Exterior,
    );
    let scene = build_scene(&p);
    assert!(scene
        .meshes
        .iter()
        .all(|m| m.object_id.is_some() || m.material != Material::WallExterior));
}

fn eave_planes(caps: bool) -> Vec<EavePlane> {
    gable_roof(PLATE)
        .planes
        .into_iter()
        .map(|plane| EavePlane {
            plane,
            overhang: 16.0,
            ridge_caps: caps,
            cuts: Vec::new(),
            id: None,
            opts: Default::default(),
        })
        .collect()
}

fn count(planes: &[EavePlane], detail: &RoofDetail, kind: EaveKind) -> usize {
    eave_elements(planes, detail)
        .iter()
        .filter(|e| e.kind == kind)
        .count()
}

#[test]
fn a_gable_roof_gets_fascia_soffit_and_rake_boards() {
    let planes = eave_planes(false);
    let d = RoofDetail::default();
    assert_eq!(count(&planes, &d, EaveKind::Fascia), 2, "one per eave");
    assert_eq!(count(&planes, &d, EaveKind::RakeFascia), 4, "two per plane");
    assert_eq!(count(&planes, &d, EaveKind::Soffit), 2);
    assert_eq!(count(&planes, &d, EaveKind::RakeSoffit), 4);
    assert_eq!(
        count(&planes, &d, EaveKind::Frieze),
        0,
        "frieze is optional"
    );
    assert_eq!(
        count(&planes, &d, EaveKind::RidgeCap),
        0,
        "caps are per plane"
    );
    // Boards are boxes (6 faces), soffits single faces.
    let els = eave_elements(&planes, &d);
    assert!(els
        .iter()
        .filter(|e| e.kind == EaveKind::Fascia)
        .all(|e| e.faces.len() == 6));
    assert!(els
        .iter()
        .filter(|e| e.kind == EaveKind::Soffit)
        .all(|e| e.faces.len() == 1));
    // Meshes: 12 boards of 6 faces and 6 soffits of 1 face, 2 triangles each.
    let meshes = eave_detail_meshes(&planes, &d);
    let tris: usize = meshes
        .iter()
        .filter(|m| m.material == Material::Trim)
        .map(|m| m.indices.len() / 3)
        .sum();
    assert_eq!(tris, (6 * 6 + 6) * 2);
}

#[test]
fn eave_options_switch_pieces_on_and_off() {
    let planes = eave_planes(true);
    let none = RoofDetail {
        soffit: false,
        rake_fascia: false,
        ..RoofDetail::default()
    };
    assert_eq!(count(&planes, &none, EaveKind::Soffit), 0);
    assert_eq!(count(&planes, &none, EaveKind::RakeSoffit), 0);
    assert_eq!(count(&planes, &none, EaveKind::RakeFascia), 0);
    assert_eq!(count(&planes, &none, EaveKind::Fascia), 2);
    let frieze = RoofDetail {
        frieze: true,
        ..RoofDetail::default()
    };
    assert_eq!(count(&planes, &frieze, EaveKind::Frieze), 2);
    assert_eq!(
        count(&planes, &RoofDetail::default(), EaveKind::RidgeCap),
        2
    );
    let no_caps = RoofDetail {
        ridge_caps: false,
        ..RoofDetail::default()
    };
    assert_eq!(count(&planes, &no_caps, EaveKind::RidgeCap), 0);
    // A sloped soffit rises toward the wall; a level one does not.
    let level = eave_elements(&planes, &RoofDetail::default());
    let sloped = eave_elements(
        &planes,
        &RoofDetail {
            sloped_soffit: true,
            ..RoofDetail::default()
        },
    );
    let ys = |els: &[plan_3d::EaveElement]| -> Vec<f64> {
        els.iter()
            .find(|e| e.kind == EaveKind::Soffit)
            .unwrap()
            .faces[0]
            .0
            .iter()
            .map(|v| v[1])
            .collect()
    };
    let (l, s) = (ys(&level), ys(&sloped));
    assert!(l.iter().all(|y| (*y - l[0]).abs() < 1e-9));
    assert!(s.iter().any(|y| *y > s[0] + 5.0));
}

#[test]
fn the_soffit_runs_from_the_eave_to_the_wall_face() {
    let planes = eave_planes(false);
    let els = eave_elements(&planes, &RoofDetail::default());
    let soffit = els.iter().find(|e| e.kind == EaveKind::Soffit).unwrap();
    let (quad, normal) = soffit.faces[0];
    assert!(normal[1] < -0.99, "faces down");
    // Its depth is the 16" overhang: baseline z vs the inner edge z.
    let zs: Vec<f64> = quad.iter().map(|v| v[2]).collect();
    let depth =
        zs.iter().fold(f64::MIN, |a, b| a.max(*b)) - zs.iter().fold(f64::MAX, |a, b| a.min(*b));
    assert!((depth - 16.0).abs() < 1e-6, "soffit depth {depth}");
}

#[test]
fn flashing_follows_the_butt_line() {
    let (p, _wall, _roof) = lean_to();
    let cover = RoofCover::from_project(&p);
    let planes = &cover.floor(0).unwrap().eaves;
    let d = RoofDetail::default();
    assert_eq!(count(planes, &d, EaveKind::Flashing), 1);
    let off = RoofDetail {
        flashing: false,
        ..RoofDetail::default()
    };
    assert_eq!(count(planes, &off, EaveKind::Flashing), 0);
    // The butt edge is not a rake: no fascia on it.
    assert_eq!(
        count(planes, &d, EaveKind::RakeFascia),
        2,
        "the two sides only"
    );
}
