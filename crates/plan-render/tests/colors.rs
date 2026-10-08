//! `Mesh.color`: a mesh's own colour is the albedo the ray tracer shades with.

use std::sync::Arc;

use plan_3d::{Material, Mesh, Scene, Vertex};
use plan_materials::textures::TextureStore;
use plan_render::{Camera, Environment, Image, RenderSettings, Renderer, Technique};

/// A 96 x 96 inch wall at z = 0 facing +Z.
fn wall(material: Material, color: Option<[u8; 3]>) -> Scene {
    let v = |x: f32, y: f32| Vertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        uv: [0.0, 0.0],
    };
    Scene {
        meshes: vec![Mesh {
            vertices: vec![v(0.0, 0.0), v(96.0, 0.0), v(96.0, 96.0), v(0.0, 96.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            material,
            object_id: None,
            color,
        }],
    }
}

fn render(scene: &Scene, technique: Technique) -> Image {
    let store = Arc::new(TextureStore::with_dirs(Vec::new()));
    let camera = Camera {
        eye: [48.0, 48.0, 60.0],
        target: [48.0, 48.0, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_deg: 50.0,
        aperture: 0.0,
        focus_dist: 0.0,
    };
    let settings = RenderSettings {
        width: 32,
        height: 32,
        samples: 8,
        threads: 1,
        technique,
        textures: true,
        ..RenderSettings::default()
    };
    Renderer::new(scene).with_texture_store(store).render(
        &camera,
        &Environment::default(),
        &[],
        &settings,
    )
}

fn centre(img: &Image) -> [f32; 3] {
    img.hdr_at(16, 16)
}

#[test]
fn a_mesh_colour_replaces_the_materials_albedo() {
    // Interior paint is an off-white; paint the mesh pure red.
    let plain = centre(&render(
        &wall(Material::WallInterior, None),
        Technique::PhysicallyBased,
    ));
    let red = centre(&render(
        &wall(Material::WallInterior, Some([220, 20, 20])),
        Technique::PhysicallyBased,
    ));
    assert!(
        plain[0] > 0.0 && (plain[0] - plain[2]).abs() < 0.2 * plain[0],
        "{plain:?}"
    );
    assert!(red[0] > 3.0 * red[1] && red[0] > 3.0 * red[2], "{red:?}");
}

#[test]
fn the_colour_shows_in_proportion_and_a_textured_material_goes_flat() {
    // The same mesh colour gives the same image whatever textured material
    // carries it (the colour is the albedo, not a tint of the bitmap).
    let on_brick = centre(&render(
        &wall(Material::Brick, Some([40, 160, 40])),
        Technique::PhysicallyBased,
    ));
    let on_paint = centre(&render(
        &wall(Material::WallInterior, Some([40, 160, 40])),
        Technique::PhysicallyBased,
    ));
    assert!(on_brick[1] > 2.0 * on_brick[0], "{on_brick:?}");
    for k in 0..3 {
        assert!(
            (on_brick[k] - on_paint[k]).abs() < 0.2 * on_paint[k].max(0.02),
            "{on_brick:?} vs {on_paint:?}"
        );
    }
}

#[test]
fn clay_ignores_the_mesh_colour_like_it_ignores_every_colour() {
    let plain = centre(&render(
        &wall(Material::WallInterior, None),
        Technique::Clay,
    ));
    let red = centre(&render(
        &wall(Material::WallInterior, Some([220, 20, 20])),
        Technique::Clay,
    ));
    assert_eq!(plain, red);
}
