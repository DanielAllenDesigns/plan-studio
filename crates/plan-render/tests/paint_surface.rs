//! The surface a painted mesh got from its material (class, roughness,
//! metalness, transparency, emissive) reaches the ray tracer.

use std::sync::Arc;

use plan_3d::surface::{register, PaintSurface};
use plan_3d::{Material, Mesh, Scene, Vertex};
use plan_materials::textures::TextureStore;
use plan_render::{Camera, Environment, Image, RenderSettings, Renderer, Technique};

/// A 96 x 96 inch wall at z = 0 facing +Z.
fn wall(object: u64, color: [u8; 3]) -> Scene {
    let v = |x: f32, y: f32| Vertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        uv: [0.0, 0.0],
    };
    Scene {
        meshes: vec![Mesh {
            vertices: vec![v(0.0, 0.0), v(96.0, 0.0), v(96.0, 96.0), v(0.0, 96.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: Material::WallInterior,
            object_id: Some(object),
            color: Some(color),
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
        samples: 16,
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

fn paint(object: u64, color: [u8; 3], surface: PaintSurface) {
    register(object, Material::WallInterior, color, Some(surface));
}

const MATTE: PaintSurface = PaintSurface {
    roughness: 1.0,
    metallic: 0.0,
    transparency: 0.0,
    emissive: 0.0,
};

#[test]
fn an_emissive_material_glows() {
    let color = [200, 120, 40];
    paint(9_300_001, color, MATTE);
    let plain = centre(&render(&wall(9_300_001, color), Technique::PhysicallyBased));
    paint(
        9_300_002,
        color,
        PaintSurface {
            emissive: 1.0,
            ..MATTE
        },
    );
    let lit = centre(&render(&wall(9_300_002, color), Technique::PhysicallyBased));
    assert!(lit[0] > 1.8 * plain[0], "plain {plain:?}, emissive {lit:?}");
    // It keeps its colour: orange stays orange.
    assert!(lit[0] > lit[1] && lit[1] > lit[2], "{lit:?}");
}

#[test]
fn a_fully_transparent_material_shows_what_is_behind_it() {
    let color = [220, 20, 20];
    paint(
        9_300_003,
        color,
        PaintSurface {
            transparency: 1.0,
            ..MATTE
        },
    );
    let clear = centre(&render(&wall(9_300_003, color), Technique::PhysicallyBased));
    let none = centre(&render(
        &Scene { meshes: Vec::new() },
        Technique::PhysicallyBased,
    ));
    paint(9_300_004, color, MATTE);
    let red = centre(&render(&wall(9_300_004, color), Technique::PhysicallyBased));
    assert!(red[0] > 3.0 * red[1], "the opaque wall is red: {red:?}");
    for k in 0..3 {
        assert!(
            (clear[k] - none[k]).abs() < 0.05 * none[k].max(0.1),
            "{clear:?} vs {none:?}"
        );
    }
    // Half transparent lets some of the sky through: less red than opaque.
    paint(
        9_300_005,
        color,
        PaintSurface {
            transparency: 0.5,
            ..MATTE
        },
    );
    let half = centre(&render(&wall(9_300_005, color), Technique::PhysicallyBased));
    assert!(half[1] > red[1], "{half:?} {red:?}");
}

#[test]
fn metalness_and_roughness_change_the_shading() {
    let color = [180, 180, 190];
    paint(9_300_006, color, MATTE);
    let matte = centre(&render(&wall(9_300_006, color), Technique::PhysicallyBased));
    paint(
        9_300_007,
        color,
        PaintSurface {
            roughness: 0.05,
            metallic: 1.0,
            ..MATTE
        },
    );
    let mirror = centre(&render(&wall(9_300_007, color), Technique::PhysicallyBased));
    assert!(
        (matte[0] - mirror[0]).abs() > 0.05 * matte[0],
        "{matte:?} {mirror:?}"
    );
}

#[test]
fn clay_ignores_the_painted_surface() {
    let color = [10, 200, 30];
    paint(9_300_008, color, MATTE);
    let a = centre(&render(&wall(9_300_008, color), Technique::Clay));
    paint(
        9_300_009,
        color,
        PaintSurface {
            emissive: 1.0,
            transparency: 1.0,
            ..MATTE
        },
    );
    let b = centre(&render(&wall(9_300_009, color), Technique::Clay));
    assert_eq!(a, b);
}
