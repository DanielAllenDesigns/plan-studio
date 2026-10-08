//! End-to-end check of the texture path against a real (offscreen) OpenGL
//! context, macOS only. Run with
//! `cargo test -p plan-view3d headless -- --ignored --nocapture`.

use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::Arc;

use eframe::glow::{self, HasContext as _};
use plan_3d::{Material, Mesh, Scene, Vertex};
use plan_materials::textures::TextureStore;

use super::super::{FrameParams, GpuScene};
use crate::camera::Camera;
use crate::quality::{Look, Quality, ViewLight, ViewSettings};
use crate::texturing::ImageTexture;
use crate::Lighting;

#[link(name = "OpenGL", kind = "framework")]
unsafe extern "C" {
    fn CGLChoosePixelFormat(
        attribs: *const c_int,
        pix: *mut *mut c_void,
        npix: *mut c_int,
    ) -> c_int;
    fn CGLCreateContext(pix: *mut c_void, share: *mut c_void, ctx: *mut *mut c_void) -> c_int;
    fn CGLSetCurrentContext(ctx: *mut c_void) -> c_int;
}
unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}

const W: i32 = 96;
const H: i32 = 96;

fn context() -> Option<glow::Context> {
    // kCGLPFAOpenGLProfile = 99, kCGLOGLPVersion_GL3_Core = 0x3200, kCGLPFAAccelerated = 73
    let attribs: [c_int; 5] = [99, 0x3200, 73, 0, 0];
    unsafe {
        let mut pix = std::ptr::null_mut();
        let mut n = 0;
        if CGLChoosePixelFormat(attribs.as_ptr(), &mut pix, &mut n) != 0 || pix.is_null() {
            return None;
        }
        let mut ctx = std::ptr::null_mut();
        if CGLCreateContext(pix, std::ptr::null_mut(), &mut ctx) != 0 || ctx.is_null() {
            return None;
        }
        CGLSetCurrentContext(ctx);
        let lib = CString::new("/System/Library/Frameworks/OpenGL.framework/OpenGL").unwrap();
        let handle = dlopen(lib.as_ptr(), 1);
        Some(glow::Context::from_loader_function_cstr(|name| {
            dlsym(handle, name.as_ptr()) as *const _
        }))
    }
}

fn quad(
    corners: [[f32; 3]; 4],
    normal: [f32; 3],
    material: Material,
    object_id: Option<u64>,
) -> Mesh {
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    Mesh {
        vertices: corners
            .iter()
            .zip(uvs)
            .map(|(p, uv)| Vertex {
                position: *p,
                normal,
                uv,
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        object_id,
    }
}

/// What one test frame looks like.
#[derive(Clone)]
struct Setup {
    look: Look,
    settings: ViewSettings,
    lighting: Lighting,
    textures: bool,
    show_edges: bool,
    camera: Option<Camera>,
    lights: Vec<ViewLight>,
    /// Side of the square picture, pixels.
    size: i32,
}

impl Setup {
    /// The original texture test: flat light, no shadows, direct drawing.
    fn flat(textures: bool) -> Setup {
        Setup {
            look: Look::Standard,
            settings: ViewSettings {
                shadows: false,
                ambient_occlusion: false,
                quality: Quality::Low,
                exposure: 1.0,
            },
            lighting: Lighting {
                ambient: 1.0,
                key: 0.0,
                ..Lighting::default()
            },
            textures,
            show_edges: false,
            camera: None,
            lights: Vec::new(),
            size: W,
        }
    }

    /// The default sun and sky with the given look and settings.
    fn lit(look: Look, settings: ViewSettings) -> Setup {
        Setup {
            look,
            settings,
            lighting: Lighting::default(),
            textures: false,
            show_edges: false,
            camera: None,
            lights: Vec::new(),
            size: W,
        }
    }
}

/// Renders `scene` and returns the RGBA pixels, bottom row first. Without a
/// camera in `setup` it looks from the front (+Z).
fn render(gl: &glow::Context, gpu: &mut GpuScene, scene: &Scene, setup: &Setup) -> Vec<u8> {
    let (w, h) = (setup.size, setup.size);
    unsafe {
        let fbo = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        let color = gl.create_renderbuffer().unwrap();
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(color));
        gl.renderbuffer_storage(glow::RENDERBUFFER, glow::RGBA8, w, h);
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::RENDERBUFFER,
            Some(color),
        );
        let depth = gl.create_renderbuffer().unwrap();
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
        gl.renderbuffer_storage(glow::RENDERBUFFER, glow::DEPTH_COMPONENT24, w, h);
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::DEPTH_ATTACHMENT,
            glow::RENDERBUFFER,
            Some(depth),
        );
        assert_eq!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER),
            glow::FRAMEBUFFER_COMPLETE
        );
        gl.viewport(0, 0, w, h);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.disable(glow::SCISSOR_TEST);
        gl.clear(glow::COLOR_BUFFER_BIT);

        gpu.upload(gl, scene);
        let bounds = scene.bounds().unwrap();
        let cam = setup.camera.clone().unwrap_or_else(|| {
            let mut cam = Camera::default();
            cam.fit_to_bounds_with_aspect(bounds.0, bounds.1, 1.0);
            cam
        });
        let frame = FrameParams {
            lighting: setup.lighting,
            show_edges: setup.show_edges,
            textures: setup.textures,
            look: setup.look,
            settings: setup.settings,
            lights: setup.lights.clone(),
            bounds: Some(bounds),
            target_fbo: Some(fbo),
            ..FrameParams::for_camera(&cam, 1.0, (0, 0, w, h))
        };
        // The first frames upload textures (a couple per frame).
        for _ in 0..4 {
            gpu.paint(gl, &frame, (0, 0, w, h));
        }
        assert_eq!(gl.get_error(), 0, "GL error after painting");
        assert!(gpu.notes().is_empty(), "passes failed: {:?}", gpu.notes());
        let mut px = vec![0u8; (w * h * 4) as usize];
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.read_pixels(
            0,
            0,
            w,
            h,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut px)),
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.delete_framebuffer(fbo);
        gl.delete_renderbuffer(color);
        gl.delete_renderbuffer(depth);
        px
    }
}

/// Brightness spread over the middle of the picture, where the wall is
/// (the sky gradient behind it is a different matter).
fn spread(px: &[u8]) -> f32 {
    let (lo, hi) = ((W * 2 / 5) as usize, (W * 3 / 5) as usize);
    let l: Vec<f32> = (lo..hi)
        .flat_map(|y| (lo..hi).map(move |x| (y * W as usize + x) * 4))
        .map(|o| {
            0.2126 * f32::from(px[o])
                + 0.7152 * f32::from(px[o + 1])
                + 0.0722 * f32::from(px[o + 2])
        })
        .collect();
    let mean = l.iter().sum::<f32>() / l.len().max(1) as f32;
    (l.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / l.len().max(1) as f32).sqrt()
}

#[test]
#[ignore = "needs an OpenGL context (macOS CGL)"]
fn textured_walls_and_pictures_render_through_real_gl() {
    let Some(gl) = context() else {
        eprintln!("no OpenGL context available; skipping");
        return;
    };
    let store = Arc::new(TextureStore::with_dirs(Vec::new()));
    let mut gpu = GpuScene::with_store(store);

    // A 96 inch brick wall facing +Z.
    let wall = quad(
        [
            [0.0, 0.0, 0.0],
            [96.0, 0.0, 0.0],
            [96.0, 96.0, 0.0],
            [0.0, 96.0, 0.0],
        ],
        [0.0, 0.0, 1.0],
        Material::Brick,
        None,
    );
    let scene = Scene { meshes: vec![wall] };
    let flat = render(&gl, &mut gpu, &scene, &Setup::flat(false));
    assert_eq!(
        gpu.material_texture_count(),
        0,
        "no upload while textures are off"
    );
    let tex = render(&gl, &mut gpu, &scene, &Setup::flat(true));
    assert_eq!(gpu.material_texture_count(), 1, "uploaded on demand");
    assert_eq!(gpu.pending_uploads(), 0);
    if let Some(dir) = std::env::var_os("PLAN_GL_DUMP") {
        // Bottom row first in memory: write the rows top-down.
        let mut ppm = format!("P6\n{W} {H}\n255\n").into_bytes();
        for y in (0..H as usize).rev() {
            for x in 0..W as usize {
                ppm.extend_from_slice(&tex[(y * W as usize + x) * 4..][..3]);
            }
        }
        let _ = std::fs::write(std::path::Path::new(&dir).join("wall.ppm"), ppm);
    }
    let (flat_spread, tex_spread) = (spread(&flat), spread(&tex));
    eprintln!("spread flat {flat_spread:.2} textured {tex_spread:.2}");
    assert!(flat_spread < 1.5, "flat wall is uniform ({flat_spread})");
    assert!(tex_spread > 4.0, "brick wall shows bricks ({tex_spread})");

    // Context loss: everything is forgotten and comes back on demand.
    gpu.forget_context();
    assert_eq!(gpu.material_texture_count(), 0);
    let again = render(&gl, &mut gpu, &scene, &Setup::flat(true));
    assert_eq!(gpu.material_texture_count(), 1);
    assert_eq!(again, tex, "the same picture after a re-upload");

    // A picture quad: bottom-left red, bottom-right green, top-left blue,
    // top-right white (the image's top row is first in memory).
    let rgba: Vec<u8> = vec![
        0, 0, 255, 255, 255, 255, 255, 255, // top row: blue, white
        255, 0, 0, 255, 0, 255, 0, 255, // bottom row: red, green
    ];
    let pic = quad(
        [
            [0.0, 0.0, 0.0],
            [96.0, 0.0, 0.0],
            [96.0, 96.0, 0.0],
            [0.0, 96.0, 0.0],
        ],
        [0.0, 0.0, 1.0],
        Material::Trim,
        Some(9),
    );
    let scene = Scene { meshes: vec![pic] };
    gpu.set_pictures(
        &gl,
        &[ImageTexture {
            object_id: 9,
            key: 1,
            width: 2,
            height: 2,
            rgba: Arc::new(rgba),
            flip: false,
        }],
    );
    let px = render(&gl, &mut gpu, &scene, &Setup::flat(true));
    assert_eq!(gpu.picture_texture_count(), 1);
    let at = |fx: f32, fy_from_top: f32| {
        let (x, y) = (
            (fx * W as f32) as i32,
            ((1.0 - fy_from_top) * H as f32) as i32,
        );
        let o = ((y * W + x) * 4) as usize;
        [px[o], px[o + 1], px[o + 2]]
    };
    let dominant = |c: [u8; 3]| match (c[0] > 160, c[1] > 160, c[2] > 160) {
        (true, false, false) => 'R',
        (false, true, false) => 'G',
        (false, false, true) => 'B',
        (true, true, true) => 'W',
        _ => '?',
    };
    // The quad spans the middle of the framebuffer; sample inside each quarter.
    let quarters = [
        (0.38, 0.62, 'B'), // top-left
        (0.62, 0.62, 'W'), // top-right
        (0.38, 0.38, 'R'), // bottom-left
        (0.62, 0.38, 'G'), // bottom-right
    ];
    for (fx, fy, want) in quarters {
        // fy is measured from the bottom here; convert to from-top.
        let got = dominant(at(fx, 1.0 - fy));
        assert_eq!(got, want, "pixel at ({fx},{fy}): {:?}", at(fx, 1.0 - fy));
    }

    gpu.destroy(&gl);
    assert_eq!(gpu.picture_texture_count(), 0);
}

// ----- shadows, occlusion, looks -----

fn slab_scene() -> Scene {
    let floor = quad(
        [
            [-100.0, 0.0, -100.0],
            [100.0, 0.0, -100.0],
            [100.0, 0.0, 100.0],
            [-100.0, 0.0, 100.0],
        ],
        [0.0, 1.0, 0.0],
        Material::Concrete,
        None,
    );
    let slab = quad(
        [
            [-30.0, 60.0, -30.0],
            [30.0, 60.0, -30.0],
            [30.0, 60.0, 30.0],
            [-30.0, 60.0, 30.0],
        ],
        [0.0, 1.0, 0.0],
        Material::WallInterior,
        None,
    );
    let wall = quad(
        [
            [-100.0, 0.0, -100.0],
            [100.0, 0.0, -100.0],
            [100.0, 100.0, -100.0],
            [-100.0, 100.0, -100.0],
        ],
        [0.0, 0.0, 1.0],
        Material::WallInterior,
        None,
    );
    Scene {
        meshes: vec![floor, slab, wall],
    }
}

/// Brightness of the pixel where the world point `p` lands.
fn lum_at(px: &[u8], cam: &Camera, size: i32, p: [f32; 3]) -> f32 {
    let n = crate::math::transform_point(&cam.view_projection(1.0), p);
    let x = (((n[0] * 0.5 + 0.5) * size as f32) as i32).clamp(0, size - 1);
    let y = (((n[1] * 0.5 + 0.5) * size as f32) as i32).clamp(0, size - 1);
    let o = ((y * size + x) * 4) as usize;
    0.2126 * f32::from(px[o]) + 0.7152 * f32::from(px[o + 1]) + 0.0722 * f32::from(px[o + 2])
}

/// With `PLAN_GL_DUMP=<dir>`, writes `<name>.ppm` (top row first).
fn dump(name: &str, px: &[u8], size: i32) {
    let Some(dir) = std::env::var_os("PLAN_GL_DUMP") else {
        return;
    };
    let mut ppm = format!("P6\n{size} {size}\n255\n").into_bytes();
    for y in (0..size as usize).rev() {
        for x in 0..size as usize {
            ppm.extend_from_slice(&px[(y * size as usize + x) * 4..][..3]);
        }
    }
    let _ = std::fs::write(std::path::Path::new(&dir).join(format!("{name}.ppm")), ppm);
}

fn dark_pixels(px: &[u8], below: f32) -> usize {
    px.as_chunks::<4>()
        .0
        .iter()
        .filter(|p| {
            0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]) < below
        })
        .count()
}

fn settings(shadows: bool, ao: bool, quality: Quality) -> ViewSettings {
    ViewSettings {
        shadows,
        ambient_occlusion: ao,
        quality,
        exposure: 1.0,
    }
}

#[test]
#[ignore = "needs an OpenGL context (macOS CGL)"]
fn shadows_occlusion_and_looks_render_through_real_gl() {
    let Some(gl) = context() else {
        eprintln!("no OpenGL context available; skipping");
        return;
    };
    let store = Arc::new(TextureStore::with_dirs(Vec::new()));
    let mut gpu = GpuScene::with_store(store);
    let scene = slab_scene();
    let size = 192;
    // A view from the front and above.
    let (lo, hi) = scene.bounds().unwrap();
    let mut cam = Camera::default();
    cam.fit_to_bounds_with_aspect(lo, hi, 1.0);
    let go = |look: Look, s: ViewSettings, lights: Vec<ViewLight>, gpu: &mut GpuScene| {
        let mut setup = Setup::lit(look, s);
        setup.camera = Some(cam.clone());
        setup.size = size;
        setup.lights = lights;
        let px = render(&gl, gpu, &scene, &setup);
        let tag = format!(
            "{look:?}_{}{}{:?}",
            u8::from(s.shadows),
            u8::from(s.ambient_occlusion),
            s.quality
        );
        dump(&tag, &px, size);
        px
    };
    // The sun is the default key light: its shadow of the slab lands +30 in x
    // and -37.5 in z of the slab's footprint.
    let shadowed = [30.0, 0.0, -37.5];
    let lit = [-45.0, 0.0, 45.0];
    let corner = [0.0, 0.0, -96.0]; // floor against the wall
    let open_floor = [0.0, 0.0, 70.0];

    let plain = go(
        Look::Standard,
        settings(false, false, Quality::Low),
        vec![],
        &mut gpu,
    );
    let (p_sh, p_lit) = (
        lum_at(&plain, &cam, size, shadowed),
        lum_at(&plain, &cam, size, lit),
    );
    eprintln!("no shadows: shadowed spot {p_sh:.1}, lit spot {p_lit:.1}");
    assert!(
        (p_sh - p_lit).abs() < 0.12 * p_lit,
        "floor is even without shadows"
    );

    for quality in [Quality::Low, Quality::Medium, Quality::High] {
        let img = go(
            Look::Standard,
            settings(true, false, quality),
            vec![],
            &mut gpu,
        );
        let (s, l) = (
            lum_at(&img, &cam, size, shadowed),
            lum_at(&img, &cam, size, lit),
        );
        eprintln!("shadows {quality:?}: shadowed {s:.1}, lit {l:.1}");
        assert!(
            s < 0.85 * l,
            "{quality:?}: the slab's shadow darkens the floor ({s} vs {l})"
        );
        assert!(
            l > 0.9 * p_lit,
            "{quality:?}: lit floor keeps its light ({l} vs {p_lit})"
        );
    }

    // Ambient occlusion darkens the floor/wall corner relative to open floor.
    let off = go(
        Look::Standard,
        settings(false, false, Quality::Medium),
        vec![],
        &mut gpu,
    );
    let on = go(
        Look::Standard,
        settings(false, true, Quality::Medium),
        vec![],
        &mut gpu,
    );
    let ratio = |img: &[u8]| lum_at(img, &cam, size, corner) / lum_at(img, &cam, size, open_floor);
    let (r_off, r_on) = (ratio(&off), ratio(&on));
    eprintln!("corner / open floor: ao off {r_off:.3}, on {r_on:.3}");
    assert!(
        r_on < r_off * 0.97,
        "occlusion darkens the corner ({r_on} vs {r_off})"
    );
    // ... and under the slab.
    let under = [0.0, 0.0, 0.0];
    let (u_off, u_on) = (
        lum_at(&off, &cam, size, under),
        lum_at(&on, &cam, size, under),
    );
    assert!(u_on < u_off, "under the slab {u_on} vs {u_off}");

    // Technical illustration draws silhouette lines in the composite pass.
    let flat = go(
        Look::Standard,
        settings(false, false, Quality::Medium),
        vec![],
        &mut gpu,
    );
    let tech = go(
        Look::Technical,
        settings(false, false, Quality::Medium),
        vec![],
        &mut gpu,
    );
    let (d_flat, d_tech) = (dark_pixels(&flat, 40.0), dark_pixels(&tech, 40.0));
    eprintln!("dark pixels: standard {d_flat}, technical {d_tech}");
    assert!(d_tech > d_flat + 40, "edge lines add dark pixels");

    // Every look renders without GL errors and differs from the plain one.
    for look in [
        Look::Physical,
        Look::Clay,
        Look::GlassHouse,
        Look::Watercolor,
        Look::Duotone,
        Look::Flat,
    ] {
        let img = go(
            look,
            settings(true, true, Quality::Medium),
            vec![],
            &mut gpu,
        );
        assert_ne!(img, flat, "{look:?} differs from Standard");
    }

    // A point light brightens the floor under it.
    let bulb = ViewLight {
        position: [60.0, 30.0, 60.0],
        color: [1.0e4 / std::f32::consts::PI; 3],
    };
    let dark_room = go(
        Look::Standard,
        settings(false, false, Quality::Low),
        vec![],
        &mut gpu,
    );
    let with_bulb = go(
        Look::Standard,
        settings(false, false, Quality::Low),
        vec![bulb],
        &mut gpu,
    );
    let under_bulb = [60.0, 0.0, 60.0];
    assert!(
        lum_at(&with_bulb, &cam, size, under_bulb)
            > lum_at(&dark_room, &cam, size, under_bulb) + 3.0,
        "the bulb lights the floor"
    );

    // Frame cost of each configuration (informational).
    for (name, look, s) in [
        (
            "direct, no effects",
            Look::Standard,
            settings(false, false, Quality::Low),
        ),
        (
            "shadows only",
            Look::Standard,
            settings(true, false, Quality::Low),
        ),
        (
            "medium",
            Look::Standard,
            settings(true, true, Quality::Medium),
        ),
        ("high", Look::Standard, settings(true, true, Quality::High)),
        (
            "watercolor",
            Look::Watercolor,
            settings(true, true, Quality::Medium),
        ),
        (
            "technical",
            Look::Technical,
            settings(true, true, Quality::Medium),
        ),
    ] {
        let mut setup = Setup::lit(look, s);
        setup.camera = Some(cam.clone());
        setup.size = size;
        let start = std::time::Instant::now();
        for _ in 0..20 {
            render(&gl, &mut gpu, &scene, &setup);
        }
        eprintln!(
            "{name}: {:.1} ms per frame (includes upload and readback)",
            start.elapsed().as_secs_f32() * 50.0
        );
    }

    // Context loss: everything comes back, including the passes.
    gpu.forget_context();
    let again = go(
        Look::Standard,
        settings(true, true, Quality::Medium),
        vec![],
        &mut gpu,
    );
    assert!(lum_at(&again, &cam, size, shadowed) < lum_at(&again, &cam, size, lit));
    gpu.destroy(&gl);
}

/// A busy scene: a floor with a 20 x 20 grid of 4 ft cubes (6 quads each).
fn busy_scene() -> Scene {
    let mut meshes = vec![quad(
        [
            [-600.0, 0.0, -600.0],
            [600.0, 0.0, -600.0],
            [600.0, 0.0, 600.0],
            [-600.0, 0.0, 600.0],
        ],
        [0.0, 1.0, 0.0],
        Material::Concrete,
        None,
    )];
    for i in 0..20 {
        for j in 0..20 {
            let (x, z) = (-570.0 + i as f32 * 60.0, -570.0 + j as f32 * 60.0);
            let (x1, z1, h) = (
                x + 48.0,
                z + 48.0,
                48.0 + ((i * 7 + j * 3) % 5) as f32 * 24.0,
            );
            let m = Material::Siding;
            meshes.push(quad(
                [[x, h, z], [x1, h, z], [x1, h, z1], [x, h, z1]],
                [0.0, 1.0, 0.0],
                m,
                None,
            ));
            meshes.push(quad(
                [[x, 0.0, z1], [x1, 0.0, z1], [x1, h, z1], [x, h, z1]],
                [0.0, 0.0, 1.0],
                m,
                None,
            ));
            meshes.push(quad(
                [[x, 0.0, z], [x1, 0.0, z], [x1, h, z], [x, h, z]],
                [0.0, 0.0, -1.0],
                m,
                None,
            ));
            meshes.push(quad(
                [[x1, 0.0, z], [x1, 0.0, z1], [x1, h, z1], [x1, h, z]],
                [1.0, 0.0, 0.0],
                m,
                None,
            ));
            meshes.push(quad(
                [[x, 0.0, z], [x, 0.0, z1], [x, h, z1], [x, h, z]],
                [-1.0, 0.0, 0.0],
                m,
                None,
            ));
        }
    }
    Scene { meshes }
}

/// Frame time of each configuration on a busy scene at 1024 x 1024 (the
/// readback of 4 MB is included in every number). Prints the table.
#[test]
#[ignore = "needs an OpenGL context (macOS CGL)"]
fn frame_cost_on_a_busy_scene() {
    let Some(gl) = context() else {
        eprintln!("no OpenGL context available; skipping");
        return;
    };
    let store = Arc::new(TextureStore::with_dirs(Vec::new()));
    let mut gpu = GpuScene::with_store(store);
    let scene = busy_scene();
    let (lo, hi) = scene.bounds().unwrap();
    let mut cam = Camera::default();
    cam.fit_to_bounds_with_aspect(lo, hi, 1.0);
    // One offscreen window for all the timed frames.
    let (w, h) = (1024, 1024);
    let fbo = unsafe {
        let fbo = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        for (format, attachment) in [
            (glow::RGBA8, glow::COLOR_ATTACHMENT0),
            (glow::DEPTH_COMPONENT24, glow::DEPTH_ATTACHMENT),
        ] {
            let rb = gl.create_renderbuffer().unwrap();
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rb));
            gl.renderbuffer_storage(glow::RENDERBUFFER, format, w, h);
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                attachment,
                glow::RENDERBUFFER,
                Some(rb),
            );
        }
        assert_eq!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER),
            glow::FRAMEBUFFER_COMPLETE
        );
        fbo
    };
    gpu.upload(&gl, &scene);
    let mut results = Vec::new();
    for (name, look, s) in [
        (
            "direct, no effects",
            Look::Standard,
            settings(false, false, Quality::Low),
        ),
        (
            "shadows only (low)",
            Look::Standard,
            settings(true, false, Quality::Low),
        ),
        (
            "medium (shadows, AO, FXAA)",
            Look::Standard,
            settings(true, true, Quality::Medium),
        ),
        (
            "high (shadows, AO, FXAA)",
            Look::Standard,
            settings(true, true, Quality::High),
        ),
        (
            "watercolor",
            Look::Watercolor,
            settings(true, true, Quality::Medium),
        ),
        (
            "technical",
            Look::Technical,
            settings(false, false, Quality::Medium),
        ),
    ] {
        let frame = FrameParams {
            look,
            settings: s,
            bounds: Some((lo, hi)),
            textures: false,
            show_edges: true,
            target_fbo: Some(fbo),
            ..FrameParams::for_camera(&cam, 1.0, (0, 0, w, h))
        };
        // Warm up: programs, targets and the shadow map.
        gpu.paint(&gl, &frame, (0, 0, w, h));
        unsafe { gl.finish() };
        let frames = 20;
        let start = std::time::Instant::now();
        for _ in 0..frames {
            gpu.paint(&gl, &frame, (0, 0, w, h));
            unsafe { gl.finish() };
        }
        let ms = start.elapsed().as_secs_f32() * 1000.0 / frames as f32;
        assert_eq!(unsafe { gl.get_error() }, 0, "GL error while timing {name}");
        assert!(gpu.notes().is_empty(), "{:?}", gpu.notes());
        results.push((name, ms));
    }
    for (name, ms) in &results {
        eprintln!("{name}: {ms:.1} ms per frame");
    }
    assert!(results.iter().all(|(_, ms)| *ms < 1000.0));
    gpu.destroy(&gl);
}
