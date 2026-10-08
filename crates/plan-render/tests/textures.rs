//! Textured albedo in the physically based technique.

use std::sync::Arc;

use plan_3d::{Material, Mesh, Scene, Vertex};
use plan_materials::textures::TextureStore;
use plan_render::{Camera, Environment, Image, RenderSettings, Renderer, Technique};

/// A 96 x 96 inch wall at z = 0 facing +Z, in `material`.
fn wall(material: Material) -> Scene {
    let normal = [0.0, 0.0, 1.0];
    let v = |x: f32, y: f32| Vertex {
        position: [x, y, 0.0],
        normal,
        uv: [0.0, 0.0],
    };
    Scene {
        meshes: vec![Mesh {
            vertices: vec![v(0.0, 0.0), v(96.0, 0.0), v(96.0, 96.0), v(0.0, 96.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            material,
            object_id: None,
            color: None,
        }],
    }
}

fn camera() -> Camera {
    Camera {
        eye: [48.0, 48.0, 60.0],
        target: [48.0, 48.0, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_deg: 50.0,
        aperture: 0.0,
        focus_dist: 0.0,
    }
}

fn settings(textures: bool) -> RenderSettings {
    RenderSettings {
        width: 48,
        height: 48,
        samples: 12,
        threads: 1,
        technique: Technique::PhysicallyBased,
        textures,
        ..RenderSettings::default()
    }
}

fn luma_stats(img: &Image) -> (f32, f32) {
    let mut v = Vec::new();
    for y in 8..40 {
        for x in 8..40 {
            let [r, g, b] = img.hdr_at(x, y);
            v.push(0.2126 * r + 0.7152 * g + 0.0722 * b);
        }
    }
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    let var = v.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / v.len() as f32;
    (mean, var.sqrt() / mean.max(1e-6))
}

fn render(material: Material, textures: bool) -> Image {
    let store = Arc::new(TextureStore::with_dirs(Vec::new()));
    let renderer = Renderer::new(&wall(material)).with_texture_store(store);
    renderer.render(&camera(), &Environment::default(), &[], &settings(textures))
}

#[test]
fn a_brick_wall_renders_with_varying_albedo() {
    let flat = render(Material::Brick, false);
    let textured = render(Material::Brick, true);
    let (flat_mean, flat_spread) = luma_stats(&flat);
    let (tex_mean, tex_spread) = luma_stats(&textured);
    // The flat wall is uniform (only a little sample noise); the brick wall
    // shows courses and mortar.
    assert!(flat_spread < 0.08, "flat spread {flat_spread}");
    assert!(tex_spread > 0.15, "textured spread {tex_spread}");
    assert!(tex_spread > flat_spread * 2.5);
    // Brightness is matched to the flat albedo: the exposure does not jump.
    assert!(
        (tex_mean / flat_mean - 1.0).abs() < 0.45,
        "flat {flat_mean} vs textured {tex_mean}"
    );
}

#[test]
fn textures_follow_the_repeat_scale() {
    // The tile is 32 x 9 inches: a 96 inch wall holds three repeats across,
    // so the center column pattern is not uniform across the width either.
    let img = render(Material::Brick, true);
    let row: Vec<f32> = (8..40)
        .map(|x| {
            let [r, g, b] = img.hdr_at(x, 24);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        })
        .collect();
    let mean = row.iter().sum::<f32>() / row.len() as f32;
    assert!(row.iter().any(|l| (l - mean).abs() > 0.1 * mean));
}

#[test]
fn untextured_materials_and_other_techniques_stay_flat() {
    // Interior paint has no texture: same image either way.
    let on = render(Material::WallInterior, true);
    let off = render(Material::WallInterior, false);
    assert_eq!(on.hdr_at(20, 20), off.hdr_at(20, 20));
    // Clay ignores colors and textures alike.
    let store = Arc::new(TextureStore::with_dirs(Vec::new()));
    let renderer = Renderer::new(&wall(Material::Brick)).with_texture_store(store);
    let clay = RenderSettings {
        technique: Technique::Clay,
        ..settings(true)
    };
    let img = renderer.render(&camera(), &Environment::default(), &[], &clay);
    let (_, spread) = luma_stats(&img);
    assert!(spread < 0.08, "{spread}");
}

#[test]
fn textured_renders_are_deterministic() {
    let a = render(Material::Stucco, true);
    let b = render(Material::Stucco, true);
    assert_eq!(a.hdr_at(17, 23), b.hdr_at(17, 23));
    assert_eq!(a.hdr_at(30, 5), b.hdr_at(30, 5));
}
