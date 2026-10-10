//! Behavioural tests for scene building and glTF export.

use plan_3d::{build_scene, gltf, Material, Mesh, Scene};
use plan_core::{OpeningKind, Point, Project, WallKind};
use serde_json::Value;

const WALL_H: f64 = 100.0;

fn ft(f: f64) -> f64 {
    f * 12.0
}

fn single_wall(kind: WallKind) -> (Project, u64) {
    let mut p = Project::new("t");
    let id = p.add_wall(0, Point::ZERO, Point::new(ft(10.0), 0.0), 4.5, WALL_H, kind);
    (p, id)
}

fn room_20x10() -> Project {
    let mut p = Project::new("room");
    let (w, h) = (ft(20.0), ft(10.0));
    let corners = [
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ];
    for i in 0..4 {
        p.add_wall(
            0,
            corners[i],
            corners[(i + 1) % 4],
            6.5,
            109.125,
            WallKind::Exterior,
        );
    }
    p
}

fn tri_normal(m: &Mesh, t: &[u32]) -> [f64; 3] {
    let p = |i: u32| m.vertices[i as usize].position.map(f64::from);
    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
    let (u, v) = (
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
    );
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// Sum of triangle areas (sq in) whose geometric normal satisfies `keep` on the Y component.
fn area_where(m: &Mesh, keep: impl Fn(f64) -> bool) -> f64 {
    m.indices
        .chunks(3)
        .map(|t| {
            let n = tri_normal(m, t);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            (len * 0.5, if len > 0.0 { n[1] / len } else { 0.0 })
        })
        .filter(|&(_, ny)| keep(ny))
        .map(|(a, _)| a)
        .sum()
}

fn meshes_of(scene: &Scene, material: Material) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|m| m.material == material)
        .collect()
}

#[test]
fn single_wall_is_a_closed_box_of_twelve_triangles() {
    let (p, id) = single_wall(WallKind::Exterior);
    let scene = build_scene(&p);
    assert_eq!(scene.triangle_count(), 12);
    assert!(scene.meshes.iter().all(|m| m.object_id == Some(id)));
    // Closed box: surface area matches 2(lw + lh + wh).
    let (l, w, h) = (ft(10.0), 4.5, WALL_H);
    let expected = 2.0 * (l * w + l * h + w * h);
    let total: f64 = scene.meshes.iter().map(|m| area_where(m, |_| true)).sum();
    assert!((total - expected).abs() < 1e-3);
    // Every triangle faces away from the box center.
    let (lo, hi) = scene.bounds().unwrap();
    let center = [0, 1, 2].map(|k| f64::from(lo[k] + hi[k]) * 0.5);
    for m in &scene.meshes {
        for t in m.indices.chunks(3) {
            let n = tri_normal(m, t);
            let c = m.vertices[t[0] as usize].position.map(f64::from);
            let out = [c[0] - center[0], c[1] - center[1], c[2] - center[2]];
            assert!(n[0] * out[0] + n[1] * out[1] + n[2] * out[2] > 0.0);
        }
    }
}

#[test]
fn wall_scene_bounds_use_x_right_y_up_z_negated_plan_y() {
    let (p, _) = single_wall(WallKind::Interior);
    let (lo, hi) = build_scene(&p).bounds().unwrap();
    assert_eq!((lo[0], hi[0]), (0.0, 120.0));
    assert_eq!((lo[1], hi[1]), (0.0, WALL_H as f32));
    assert_eq!((lo[2], hi[2]), (-2.25, 2.25));
}

#[test]
fn wall_with_a_door_gets_holes_reveals_and_a_panel() {
    let (mut p, id) = single_wall(WallKind::Interior);
    let door = p.add_opening(0, id, ft(5.0), OpeningKind::Door).unwrap();
    let scene = build_scene(&p);
    for m in &scene.meshes {
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
    }
    // Wall: 3 rects per face x2, 3 reveals, 4 caps/top/bottom = 13 quads.
    let wall_tris: usize = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .map(Mesh::triangle_count)
        .sum();
    assert_eq!(wall_tris, 26);
    assert!(wall_tris > 12);
    assert_eq!(meshes_of(&scene, Material::DoorPanel).len(), 1);
    assert_eq!(
        meshes_of(&scene, Material::DoorPanel)[0].object_id,
        Some(door)
    );
    // Surface area: box minus both faces' holes plus the three reveals.
    let area: f64 = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .map(|m| area_where(m, |_| true))
        .sum();
    let (l, t, h, dw, dh) = (120.0, 4.5, WALL_H, 36.0, 80.0);
    let expected = 2.0 * (l * t + l * h + t * h) - 2.0 * dw * dh + 2.0 * t * dh + dw * t;
    assert!((area - expected).abs() < 1e-3, "{area} vs {expected}");
}

#[test]
fn window_adds_frame_and_translucent_glass() {
    let (mut p, id) = single_wall(WallKind::Interior);
    p.add_opening(0, id, ft(5.0), OpeningKind::Window).unwrap();
    let scene = build_scene(&p);
    assert_eq!(meshes_of(&scene, Material::WindowFrame).len(), 1);
    assert_eq!(meshes_of(&scene, Material::WindowGlass).len(), 1);
    assert!(Material::WindowGlass.color()[3] < 1.0);
    // A window hole leaves a closed ring of reveals: 2 jambs + head + sill.
    let wall_tris: usize = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .map(Mesh::triangle_count)
        .sum();
    assert_eq!(wall_tris, (4 * 2 + 4 + 4) * 2);
}

#[test]
fn four_walls_yield_one_floor_and_one_ceiling_of_200_sq_ft() {
    let scene = build_scene(&room_20x10());
    let floors = meshes_of(&scene, Material::Floor);
    let ceilings = meshes_of(&scene, Material::Ceiling);
    assert_eq!((floors.len(), ceilings.len()), (1, 1));
    let floor_sqft = area_where(floors[0], |ny| ny > 0.5) / 144.0;
    let ceiling_sqft = area_where(ceilings[0], |ny| ny < -0.5) / 144.0;
    assert!((floor_sqft - 200.0).abs() < 1.0, "{floor_sqft}");
    assert!((ceiling_sqft - 200.0).abs() < 1.0, "{ceiling_sqft}");
    // Slab heights: finished floor 0.75" above the elevation, ceiling at 109 1/8".
    let (_, floor_hi) = floors[0].bounds().unwrap();
    assert!((floor_hi[1] - 0.75).abs() < 1e-5);
    let (ceil_lo, _) = ceilings[0].bounds().unwrap();
    assert!((ceil_lo[1] - 109.125).abs() < 1e-4);
}

#[test]
fn exterior_walls_get_siding_outside_and_drywall_inside() {
    let scene = build_scene(&room_20x10());
    assert!(!meshes_of(&scene, Material::WallExterior).is_empty());
    assert!(!meshes_of(&scene, Material::WallInterior).is_empty());
    // The south wall's interior face points into the room (+plan y = -z).
    let south = scene.meshes.iter().find(|m| {
        m.material == Material::WallInterior && m.vertices.iter().all(|v| v.position[2].abs() < 4.0)
    });
    let south = south.expect("south wall interior mesh");
    assert!(south.vertices.iter().any(|v| v.normal[2] < -0.9));
}

#[test]
fn upper_floor_is_offset_by_its_elevation() {
    let mut p = room_20x10();
    p.floors.push(plan_core::Floor::new("2nd Floor", 120.0));
    let mut upper = p.floors[0].clone();
    upper.elevation = 120.0;
    p.floors[1] = upper;
    let scene = build_scene(&p);
    let (_, hi) = scene.bounds().unwrap();
    // The top is the ceiling platform: the 0.625" ceiling finish (R-27) and
    // the 1" platform sit above the finished ceiling.
    assert!((hi[1] - (120.0 + 109.125 + 0.625 + 1.0)).abs() < 1e-3);
    assert_eq!(meshes_of(&scene, Material::Floor).len(), 2);
}

#[test]
fn all_normals_are_unit_length_and_indices_in_range() {
    let mut p = room_20x10();
    let wall = p.floors[0].walls[0].id;
    p.add_opening(0, wall, ft(10.0), OpeningKind::Door).unwrap();
    let wall = p.floors[0].walls[2].id;
    p.add_opening(0, wall, ft(10.0), OpeningKind::Window)
        .unwrap();
    let scene = build_scene(&p);
    assert!(scene.triangle_count() > 100);
    for m in &scene.meshes {
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        for v in &m.vertices {
            let n = v.normal.map(f64::from);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5);
        }
    }
}

#[test]
fn gltf_json_has_matching_accessors_and_materials() {
    let mut p = room_20x10();
    let wall = p.floors[0].walls[0].id;
    p.add_opening(0, wall, ft(10.0), OpeningKind::Window)
        .unwrap();
    let scene = build_scene(&p);
    let (json, bin) = gltf::export_gltf(&scene);
    let doc: Value = serde_json::from_str(&json).expect("valid JSON");

    assert_eq!(doc["asset"]["version"], "2.0");
    assert_eq!(
        doc["buffers"][0]["byteLength"].as_u64().unwrap() as usize,
        bin.len()
    );
    assert_eq!(
        doc["materials"].as_array().unwrap().len(),
        Material::ALL.len()
    );
    let glass = &doc["materials"][Material::WindowGlass.index()];
    assert_eq!(glass["alphaMode"], "BLEND");

    let nodes = doc["nodes"].as_array().unwrap();
    let meshes = doc["meshes"].as_array().unwrap();
    assert_eq!(nodes.len(), scene.meshes.len());
    assert_eq!(meshes.len(), scene.meshes.len());
    assert_eq!(
        doc["scenes"][0]["nodes"].as_array().unwrap().len(),
        nodes.len()
    );

    let accessors = doc["accessors"].as_array().unwrap();
    for (mesh, source) in meshes.iter().zip(&scene.meshes) {
        let prim = &mesh["primitives"][0];
        let count = |v: &Value| {
            accessors[v.as_u64().unwrap() as usize]["count"]
                .as_u64()
                .unwrap() as usize
        };
        for key in ["POSITION", "NORMAL", "TEXCOORD_0"] {
            assert_eq!(count(&prim["attributes"][key]), source.vertices.len());
        }
        assert_eq!(count(&prim["indices"]), source.indices.len());
        assert_eq!(
            prim["material"].as_u64().unwrap() as usize,
            source.material.index()
        );
    }
    // Every bufferView lies inside the buffer and is 4-byte aligned.
    for view in doc["bufferViews"].as_array().unwrap() {
        let (off, len) = (
            view["byteOffset"].as_u64().unwrap(),
            view["byteLength"].as_u64().unwrap(),
        );
        assert!(off + len <= bin.len() as u64);
        assert_eq!(off % 4, 0);
    }
}

#[test]
fn write_gltf_files_writes_both_files_with_matching_uri() {
    let (p, _) = single_wall(WallKind::Interior);
    let scene = build_scene(&p);
    let dir = std::env::temp_dir().join(format!("plan3d-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = dir.join("house.v1");
    gltf::write_gltf_files(&scene, &base).unwrap();
    let json = std::fs::read_to_string(dir.join("house.v1.gltf")).unwrap();
    let bin = std::fs::read(dir.join("house.v1.bin")).unwrap();
    let doc: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["buffers"][0]["uri"], "house.v1.bin");
    assert_eq!(
        doc["buffers"][0]["byteLength"].as_u64().unwrap() as usize,
        bin.len()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn empty_project_exports_valid_empty_gltf() {
    let mut p = Project::new("empty");
    p.floors.clear();
    let (json, bin) = gltf::export_gltf(&build_scene(&p));
    let doc: Value = serde_json::from_str(&json).unwrap();
    assert!(bin.is_empty());
    assert!(doc.get("buffers").is_none());
}
