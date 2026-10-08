//! End-to-end behaviour: lit room, determinism, PNG structure.

use plan_3d::{Material, Mesh, Scene, Vertex};
use plan_core::{Point, Project, WallKind};
use plan_render::{
    encode_png, render_to_file, Camera, Environment, Image, RenderSettings, Renderer, Sun,
    Technique, ToneMap,
};
use std::time::Instant;

/// Room extents, inches.
const SIZE: f32 = 240.0;
const HEIGHT: f32 = 96.0;

fn quad(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], material: Material) -> Mesh {
    let normal = [0.0, 1.0, 0.0];
    let vertices = [a, b, c, d]
        .iter()
        .map(|&position| Vertex {
            position,
            normal,
            uv: [0.0, 0.0],
        })
        .collect();
    Mesh {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        object_id: None,
    }
}

/// A 20 ft square room; the wall at z = 0 holds a 80 x 50 in window.
fn lit_room() -> Scene {
    let (s, h) = (SIZE, HEIGHT);
    let (wx0, wx1, wy0, wy1) = (80.0, 160.0, 30.0, 80.0);
    let wall = Material::WallInterior;
    let meshes = vec![
        quad(
            [0., 0., 0.],
            [s, 0., 0.],
            [s, 0., s],
            [0., 0., s],
            Material::Floor,
        ),
        quad(
            [0., h, 0.],
            [0., h, s],
            [s, h, s],
            [s, h, 0.],
            Material::Ceiling,
        ),
        // Window wall (z = 0): four pieces around the opening, glass inside it.
        quad([0., 0., 0.], [wx0, 0., 0.], [wx0, h, 0.], [0., h, 0.], wall),
        quad([wx1, 0., 0.], [s, 0., 0.], [s, h, 0.], [wx1, h, 0.], wall),
        quad(
            [wx0, 0., 0.],
            [wx1, 0., 0.],
            [wx1, wy0, 0.],
            [wx0, wy0, 0.],
            wall,
        ),
        quad(
            [wx0, wy1, 0.],
            [wx1, wy1, 0.],
            [wx1, h, 0.],
            [wx0, h, 0.],
            wall,
        ),
        quad(
            [wx0, wy0, 0.],
            [wx1, wy0, 0.],
            [wx1, wy1, 0.],
            [wx0, wy1, 0.],
            Material::WindowGlass,
        ),
        // Remaining walls.
        quad([0., 0., s], [s, 0., s], [s, h, s], [0., h, s], wall),
        quad([0., 0., 0.], [0., 0., s], [0., h, s], [0., h, 0.], wall),
        quad([s, 0., 0.], [s, h, 0.], [s, h, s], [s, 0., s], wall),
    ];
    Scene { meshes }
}

/// Looks straight down from just under the ceiling; image-up is toward the window.
fn overhead_camera() -> Camera {
    Camera {
        eye: [120.0, 90.0, 120.0],
        target: [120.0, 0.0, 120.0],
        up: [0.0, 0.0, -1.0],
        fov_deg: 70.0,
        aperture: 0.0,
        focus_dist: 0.0,
    }
}

fn sunny_env() -> Environment {
    Environment {
        // Sun north of the window wall (scene -Z), 30 degrees up: the beam
        // enters the glass and lands 50..140 inches into the room.
        sun: Some(Sun::from_azimuth_altitude(0.0, 30.0)),
        ..Environment::default()
    }
}

fn tiny_settings() -> RenderSettings {
    RenderSettings {
        width: 32,
        height: 24,
        samples: 48,
        threads: 0,
        ..RenderSettings::default()
    }
}

fn mean_luma(img: &Image, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> f32 {
    let mut sum = 0.0;
    let mut n = 0.0;
    for y in ys {
        for x in xs.clone() {
            let [r, g, b] = img.hdr_at(x, y);
            sum += 0.2126 * r + 0.7152 * g + 0.0722 * b;
            n += 1.0;
        }
    }
    sum / n
}

#[test]
fn sunlit_room_is_finite_fast_and_brighter_near_the_window() {
    let renderer = Renderer::new(&lit_room());
    let start = Instant::now();
    let img = renderer.render(&overhead_camera(), &sunny_env(), &[], &tiny_settings());
    let elapsed = start.elapsed();
    eprintln!("32x24 @48spp lit room: {elapsed:?}");
    assert!(elapsed.as_secs_f32() < 5.0, "took {elapsed:?}");
    assert_eq!((img.width, img.height), (32, 24));
    assert_eq!(img.rgba.len(), 32 * 24 * 4);
    assert!(img
        .hdr
        .iter()
        .all(|p| p.iter().all(|c| c.is_finite() && *c >= 0.0)));

    // Rows 3..10 under the window (sun patch), bottom-left corner far from it.
    let near = mean_luma(&img, 10..22, 3..10);
    let far = mean_luma(&img, 0..6, 20..24);
    eprintln!("near {near:.4} far {far:.4}");
    assert!(near > far * 2.0, "near {near} not brighter than far {far}");
    assert!(far > 0.0);
}

#[test]
fn same_seed_is_byte_identical_across_thread_counts() {
    let renderer = Renderer::new(&lit_room());
    let render = |threads, seed| {
        let s = RenderSettings {
            threads,
            seed,
            samples: 8,
            ..tiny_settings()
        };
        renderer.render(&overhead_camera(), &sunny_env(), &[], &s)
    };
    let a = render(1, 11);
    let b = render(1, 11);
    let c = render(4, 11);
    assert_eq!(a.rgba, b.rgba);
    assert_eq!(a.rgba, c.rgba);
    assert_eq!(a.hdr, c.hdr);
    assert_ne!(a.rgba, render(1, 12).rgba);
}

#[test]
fn progressive_matches_one_shot_and_can_stop_early() {
    let renderer = Renderer::new(&lit_room());
    let settings = RenderSettings {
        samples: 10,
        denoise: true,
        ..tiny_settings()
    };
    let oneshot = renderer.render(&overhead_camera(), &sunny_env(), &[], &settings);
    let mut seen = Vec::new();
    let last = renderer.render_progressive(
        &overhead_camera(),
        &sunny_env(),
        &[],
        &settings,
        &mut |_, done| {
            seen.push(done);
            true
        },
    );
    assert_eq!(seen, vec![1, 2, 4, 8, 10]);
    assert_eq!(last, oneshot);

    let mut calls = 0;
    let partial = renderer.render_progressive(
        &overhead_camera(),
        &sunny_env(),
        &[],
        &settings,
        &mut |_, _| {
            calls += 1;
            calls < 2
        },
    );
    assert_eq!(calls, 2);
    assert_ne!(partial, oneshot);
}

#[test]
fn clay_and_ambient_techniques_render() {
    let renderer = Renderer::new(&lit_room());
    for technique in [Technique::Clay, Technique::Ambient] {
        let s = RenderSettings {
            technique,
            samples: 8,
            tone_map: ToneMap::Reinhard,
            ..tiny_settings()
        };
        let img = renderer.render(&overhead_camera(), &sunny_env(), &[], &s);
        assert!(img.hdr.iter().all(|p| p.iter().all(|c| c.is_finite())));
        let luma = mean_luma(&img, 0..32, 0..24);
        assert!(luma > 0.05 && luma <= 1.0 + 1e-4, "{technique:?}: {luma}");
    }
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// Independent bitwise CRC-32.
fn crc32(data: &[u8]) -> u32 {
    let mut c = !0_u32;
    for &byte in data {
        c ^= u32::from(byte);
        for _ in 0..8 {
            c = if c & 1 == 1 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// Independent modular Adler-32.
fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u64, 0_u64);
    for &x in data {
        a = (a + u64::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    ((b << 16) | a) as u32
}

#[test]
fn png_has_valid_structure_checksums_and_pixels() {
    // Larger than one stored block so block chaining is exercised.
    let (w, h) = (120_u32, 150_u32);
    let hdr = (0..w * h).map(|i| [(i % 7) as f32 * 0.2; 3]).collect();
    let img = Image::from_hdr(w, h, hdr, ToneMap::Linear, 1.0);
    let png = encode_png(&img);
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let (mut pos, mut kinds, mut idat) = (8, Vec::new(), Vec::new());
    while pos < png.len() {
        let len = be32(&png[pos..]) as usize;
        let body = &png[pos + 4..pos + 8 + len];
        assert_eq!(
            be32(&png[pos + 8 + len..]),
            crc32(body),
            "CRC of {:?}",
            &body[..4]
        );
        kinds.push(String::from_utf8_lossy(&body[..4]).into_owned());
        if &body[..4] == b"IHDR" {
            assert_eq!(be32(&body[4..]), w);
            assert_eq!(be32(&body[8..]), h);
            assert_eq!(&body[12..17], &[8, 6, 0, 0, 0]);
        }
        if &body[..4] == b"IDAT" {
            idat.extend_from_slice(&body[4..]);
        }
        pos += 12 + len;
    }
    assert_eq!(pos, png.len());
    assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);

    // Inflate the stored blocks and check zlib framing, Adler-32 and pixels.
    assert_eq!(&idat[..2], &[0x78, 0x01]);
    assert_eq!(((u32::from(idat[0]) << 8) | u32::from(idat[1])) % 31, 0);
    let (mut raw, mut p) = (Vec::new(), 2);
    loop {
        let (bfinal, btype) = (idat[p] & 1, idat[p] >> 1);
        assert_eq!(btype, 0, "stored block expected");
        let len = usize::from(u16::from_le_bytes([idat[p + 1], idat[p + 2]]));
        let nlen = u16::from_le_bytes([idat[p + 3], idat[p + 4]]);
        assert_eq!(nlen, !(len as u16));
        raw.extend_from_slice(&idat[p + 5..p + 5 + len]);
        p += 5 + len;
        if bfinal == 1 {
            break;
        }
    }
    assert_eq!(be32(&idat[p..]), adler32(&raw));
    assert_eq!(p + 4, idat.len());
    let stride = w as usize * 4 + 1;
    assert_eq!(raw.len(), stride * h as usize);
    for (y, row) in raw.chunks(stride).enumerate() {
        assert_eq!(row[0], 0);
        assert_eq!(
            &row[1..],
            &img.rgba[y * w as usize * 4..(y + 1) * w as usize * 4]
        );
    }
}

#[test]
fn render_to_file_writes_a_png_from_a_project() {
    let mut project = Project::new("room");
    let side = 144.0;
    let corners = [
        Point::new(0.0, 0.0),
        Point::new(side, 0.0),
        Point::new(side, side),
        Point::new(0.0, side),
    ];
    for i in 0..4 {
        project.add_wall(
            0,
            corners[i],
            corners[(i + 1) % 4],
            6.5,
            96.0,
            WallKind::Exterior,
        );
    }
    let cam = Camera::from_plan(Point::new(72.0, 20.0), 90.0, 60.0, 70.0);
    let settings = RenderSettings {
        width: 16,
        height: 12,
        samples: 4,
        ..RenderSettings::default()
    };
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("plan_render_room.png");
    render_to_file(&project, &cam, &settings, &path).expect("write png");
    let bytes = std::fs::read(&path).expect("read png");
    assert_eq!(&bytes[1..4], b"PNG");
    assert!(bytes.len() > 16 * 12 * 4);
}
