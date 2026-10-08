use super::*;
use plan_3d::{Mesh, Vertex};

fn mesh(material: Material) -> Mesh {
    let v = |x: f32| Vertex {
        position: [x, 0.0, 0.0],
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
    };
    Mesh {
        vertices: vec![v(0.0), v(1.0), v(2.0)],
        indices: vec![0, 1, 2],
        material,
        object_id: None,
    }
}

fn scene(materials: &[Material]) -> Scene {
    Scene {
        meshes: materials.iter().map(|m| mesh(*m)).collect(),
    }
}

#[test]
fn only_textured_materials_are_needed_once_each_in_scene_order() {
    let s = scene(&[
        Material::Brick,
        Material::WindowGlass,
        Material::Floor,
        Material::Brick,
        Material::Trim,
        Material::Selection,
        Material::Roof,
    ]);
    assert_eq!(
        needed_materials(&s),
        vec![Material::Brick, Material::Floor, Material::Roof]
    );
    assert!(needed_materials(&Scene::default()).is_empty());
}

#[test]
fn uploads_happen_on_demand_a_few_per_frame() {
    let needed = [
        Material::Brick,
        Material::Floor,
        Material::Roof,
        Material::Grass,
    ];
    let mut resident = HashSet::new();
    // Frame 1 uploads the first two only.
    let f1 = next_uploads(&needed, &resident);
    assert_eq!(f1, vec![Material::Brick, Material::Floor]);
    assert_eq!(pending_uploads(&needed, &resident), 4);
    resident.extend(f1);
    // Frame 2 the rest.
    let f2 = next_uploads(&needed, &resident);
    assert_eq!(f2, vec![Material::Roof, Material::Grass]);
    resident.extend(f2);
    // Steady state: nothing to do.
    assert!(next_uploads(&needed, &resident).is_empty());
    assert_eq!(pending_uploads(&needed, &resident), 0);
    // A material that first appears later is uploaded then.
    let later = [Material::Brick, Material::Stucco];
    assert_eq!(next_uploads(&later, &resident), vec![Material::Stucco]);
    // Context loss forgets every upload: they all come back.
    resident.clear();
    assert_eq!(
        next_uploads(&needed, &resident).len(),
        MAX_UPLOADS_PER_FRAME
    );
    assert_eq!(pending_uploads(&needed, &resident), 4);
}

#[test]
fn image_textures_know_when_they_are_blended_and_valid() {
    let mut rgba = vec![255u8; 4 * 4 * 4];
    let tex = |rgba: &Vec<u8>, w, h| ImageTexture {
        object_id: 7,
        key: 1,
        width: w,
        height: h,
        rgba: Arc::new(rgba.clone()),
        flip: false,
    };
    assert!(tex(&rgba, 4, 4).is_valid());
    assert!(!tex(&rgba, 4, 4).has_alpha());
    rgba[7] = 0;
    assert!(tex(&rgba, 4, 4).has_alpha());
    assert!(!tex(&rgba, 4, 3).is_valid(), "size must match the pixels");
    assert!(!tex(&Vec::new(), 0, 0).is_valid());
}

#[test]
fn the_shader_mapping_matches_the_rust_constants() {
    let glsl = glsl_planar_uv();
    assert!(glsl.contains(&format!("{FLAT_EPSILON:?}")), "{glsl}");
    assert!(glsl.contains("vec3 t = cross(b, n);"));
    assert!(glsl.contains("-n.x * n.y / hl"));
    assert!(glsl.contains("-dot(pos, b)"));
    // The projection codes agree with the Rust enum.
    assert_eq!(proj_code(Material::Floor), 1);
    assert_eq!(proj_code(Material::Brick), 0);
}

#[test]
fn the_mapping_is_continuous_and_scales_with_the_repeat_size() {
    // Walking along a wall of known length crosses length / tile repeats.
    let n = [0.0, 0.0, 1.0];
    let a = planar_uv(
        [0.0, 10.0, 0.0],
        n,
        Projection::Auto,
        [1.0 / 32.0, 1.0 / 9.0],
    );
    let b = planar_uv(
        [96.0, 10.0, 0.0],
        n,
        Projection::Auto,
        [1.0 / 32.0, 1.0 / 9.0],
    );
    assert!(
        (b[0] - a[0] - 3.0).abs() < 1e-5,
        "three 32-inch repeats in 96 inches"
    );
    // Continuity: small steps give small changes everywhere along it.
    let mut prev = a;
    for i in 1..=960 {
        let p = [i as f32 * 0.1, 10.0, 0.0];
        let uv = planar_uv(p, n, Projection::Auto, [1.0 / 32.0, 1.0 / 9.0]);
        assert!((uv[0] - prev[0]).abs() < 0.01 && (uv[1] - prev[1]).abs() < 1e-6);
        prev = uv;
    }
}
