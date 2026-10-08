//! Area lights with next-event estimation, the guided denoiser, depth of
//! field, the block preview and the analytic sky, end to end.

use plan_3d::{Material, Mesh, Scene, Vertex};
use plan_render::{
    AreaLight, Camera, Environment, Image, RenderSettings, Renderer, SkyModel, Sun, Technique,
};

fn quad(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], material: Material) -> Mesh {
    Mesh {
        vertices: [a, b, c, d]
            .iter()
            .map(|&position| Vertex {
                position,
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        object_id: None,
    }
}

/// A 240 x 240 inch floor.
fn floor() -> Scene {
    Scene {
        meshes: vec![quad(
            [0., 0., 0.],
            [0., 0., 240.],
            [240., 0., 240.],
            [240., 0., 0.],
            Material::Floor,
        )],
    }
}

fn dark_env() -> Environment {
    Environment {
        sky_color_zenith: [0.0; 3],
        sky_color_horizon: [0.0; 3],
        ground_color: [0.0; 3],
        sun: None,
        ..Environment::default()
    }
}

fn looking_down() -> Camera {
    Camera {
        eye: [120.0, 100.0, 120.0],
        target: [120.0, 0.0, 120.0],
        up: [0.0, 0.0, -1.0],
        fov_deg: 60.0,
        aperture: 0.0,
        focus_dist: 0.0,
    }
}

/// A 24 x 24 inch panel 60 inches over the middle of the floor.
fn panel() -> AreaLight {
    AreaLight::ceiling_panel([120.0, 60.0, 120.0], 24.0, 24.0, [6.0; 3])
}

fn settings(samples: u32, next_event: bool) -> RenderSettings {
    RenderSettings {
        width: 24,
        height: 18,
        samples,
        max_bounces: 1,
        threads: 0,
        technique: Technique::PhysicallyBased,
        next_event,
        ..RenderSettings::default()
    }
}

fn mse(a: &Image, b: &Image) -> f32 {
    let n = a.hdr.len() as f32;
    a.hdr
        .iter()
        .zip(&b.hdr)
        .map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>() / 3.0)
        .sum::<f32>()
        / n
}

#[test]
fn next_event_estimation_cuts_the_noise_of_a_small_area_light() {
    let renderer = Renderer::new(&floor());
    let env = dark_env();
    let lights = (&[][..], &[panel()][..]);
    let cam = looking_down();
    let reference = renderer.render_with_areas(&cam, &env, lights, &settings(1024, true));
    let mean = reference.hdr.iter().map(|p| p[0]).sum::<f32>() / reference.hdr.len() as f32;
    assert!(mean > 0.05, "the panel lights the floor: {mean}");

    let with = renderer.render_with_areas(&cam, &env, lights, &settings(8, true));
    let without = renderer.render_with_areas(&cam, &env, lights, &settings(8, false));
    let (e_with, e_without) = (mse(&with, &reference), mse(&without, &reference));
    eprintln!("mse vs reference: nee {e_with:.5}, bsdf-only {e_without:.5}");
    assert!(
        e_with * 4.0 < e_without,
        "NEE should be far less noisy: {e_with} vs {e_without}"
    );
}

#[test]
fn both_ways_of_lighting_converge_to_the_same_image() {
    let renderer = Renderer::new(&floor());
    let lights = (&[][..], &[panel()][..]);
    let cam = looking_down();
    let a = renderer.render_with_areas(&cam, &dark_env(), lights, &settings(2048, true));
    let b = renderer.render_with_areas(&cam, &dark_env(), lights, &settings(2048, false));
    let mean = |i: &Image| i.hdr.iter().map(|p| p[0]).sum::<f32>() / i.hdr.len() as f32;
    let (ma, mb) = (mean(&a), mean(&b));
    assert!(
        (ma - mb).abs() < 0.12 * ma,
        "unbiased either way: {ma} vs {mb}"
    );
}

#[test]
fn a_camera_sees_the_panel_itself() {
    let renderer = Renderer::new(&floor());
    let cam = Camera {
        eye: [120.0, 10.0, 120.0],
        target: [120.0, 60.0, 120.0],
        up: [0.0, 0.0, -1.0],
        fov_deg: 40.0,
        aperture: 0.0,
        focus_dist: 0.0,
    };
    let img = renderer.render_with_areas(&cam, &dark_env(), (&[], &[panel()]), &settings(1, true));
    let centre = img.hdr_at(12, 9);
    assert!((centre[0] - 6.0).abs() < 0.01, "{centre:?}");
}

#[test]
fn the_guided_denoiser_moves_a_noisy_render_toward_the_converged_one() {
    let renderer = Renderer::new(&floor());
    let env = dark_env();
    let lights = (&[][..], &[panel()][..]);
    let cam = looking_down();
    let reference = renderer.render_with_areas(&cam, &env, lights, &settings(1024, true));
    let raw = renderer.render_with_areas(&cam, &env, lights, &settings(1, true));
    let denoised = renderer.render_with_areas(
        &cam,
        &env,
        lights,
        &RenderSettings {
            denoise: true,
            ..settings(1, true)
        },
    );
    let (before, after) = (mse(&raw, &reference), mse(&denoised, &reference));
    eprintln!("denoise: mse {before:.5} -> {after:.5}");
    assert!(after < before * 0.7, "{before} -> {after}");
}

#[test]
fn the_block_preview_arrives_first_and_is_blocky() {
    let renderer = Renderer::new(&floor());
    let cam = looking_down();
    let s = RenderSettings {
        preview_blocks: true,
        ..settings(2, true)
    };
    let mut seen: Vec<(u32, Vec<[f32; 3]>)> = Vec::new();
    let mut cb = |img: &Image, done: u32| {
        seen.push((done, img.hdr.clone()));
        true
    };
    let done = renderer.render_progressive(&cam, &Environment::default(), &[], &s, &mut cb);
    assert_eq!(seen[0].0, 0, "the preview reports zero samples");
    let w = s.width as usize;
    let p = &seen[0].1;
    assert_eq!(p[0], p[w + 1]);
    assert_eq!(p[0], p[3 * w + 3]);
    assert!(seen.len() >= 2);
    // The finished image is the same with or without the preview.
    let plain = renderer.render(&cam, &Environment::default(), &[], &s);
    assert_eq!(done.rgba, plain.rgba);
}

#[test]
fn depth_of_field_blurs_what_is_off_the_focus_distance() {
    // A floor seen at a slant: with a wide lens and the focus on the image
    // centre, the nearer floor is blurrier than the pinhole version.
    let mut meshes = floor().meshes;
    // A thin bright strip near the camera.
    meshes.push(quad(
        [100.0, 1.0, 60.0],
        [100.0, 1.0, 64.0],
        [140.0, 1.0, 64.0],
        [140.0, 1.0, 60.0],
        Material::WallExterior,
    ));
    let renderer = Renderer::new(&Scene { meshes });
    let cam = Camera {
        eye: [120.0, 30.0, 20.0],
        target: [120.0, 0.0, 120.0],
        up: [0.0, 1.0, 0.0],
        fov_deg: 50.0,
        aperture: 0.0,
        focus_dist: 0.0,
    };
    let s = RenderSettings {
        width: 40,
        height: 30,
        samples: 64,
        ..RenderSettings::default()
    };
    let env = Environment::default();
    let sharp = renderer.render(&cam, &env, &[], &s);
    let blurred = renderer.render(
        &Camera {
            aperture: 8.0,
            focus_dist: 0.0,
            ..cam
        },
        &env,
        &[],
        &s,
    );
    // Total variation along image columns: blur lowers it.
    let tv = |i: &Image| {
        let mut sum = 0.0;
        for y in 1..i.height {
            for x in 0..i.width {
                sum += (i.hdr_at(x, y)[0] - i.hdr_at(x, y - 1)[0]).abs();
            }
        }
        sum
    };
    assert_ne!(sharp.rgba, blurred.rgba);
    assert!(
        tv(&blurred) < tv(&sharp),
        "{} vs {}",
        tv(&blurred),
        tv(&sharp)
    );
}

#[test]
fn the_analytic_sky_renders_a_bright_sun_side_and_a_blue_zenith() {
    let renderer = Renderer::new(&Scene { meshes: Vec::new() });
    let env = Environment {
        sun: Some(Sun::from_azimuth_altitude(90.0, 40.0)),
        sky_model: SkyModel::Preetham { turbidity: 2.5 },
        ..Environment::default()
    };
    let look = |az_deg: f32| {
        let (s, c) = az_deg.to_radians().sin_cos();
        Camera {
            eye: [0.0; 3],
            target: [s, 0.45, -c],
            up: [0.0, 1.0, 0.0],
            fov_deg: 10.0,
            aperture: 0.0,
            focus_dist: 0.0,
        }
    };
    let s = RenderSettings {
        width: 4,
        height: 4,
        samples: 1,
        ..RenderSettings::default()
    };
    // 90 degrees is east (the sun's side); 270 the opposite horizon.
    let toward = renderer.render(&look(90.0), &env, &[], &s).hdr_at(2, 2);
    let away = renderer.render(&look(270.0), &env, &[], &s).hdr_at(2, 2);
    assert!(toward[1] > away[1] * 1.5, "{toward:?} vs {away:?}");
    let up = Camera {
        target: [0.0, 1.0, 0.001],
        ..look(0.0)
    };
    let z = renderer.render(&up, &env, &[], &s).hdr_at(2, 2);
    assert!(z[2] > z[0], "blue zenith {z:?}");
}
