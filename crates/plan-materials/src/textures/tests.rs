use super::*;

fn no_chief() -> TextureStore {
    TextureStore::with_dirs(Vec::new())
}

#[test]
fn without_chief_every_textured_material_falls_back_to_procedural() {
    let store = no_chief();
    let mut textured_count = 0;
    for m in Material::ALL {
        match store.material(m) {
            Some(t) => {
                textured_count += 1;
                assert_eq!(t.origin, TextureOrigin::Procedural, "{m:?}");
                assert_eq!(t.image.width, PROCEDURAL_SIZE);
                assert_eq!(
                    t.image.rgba.len(),
                    (PROCEDURAL_SIZE * PROCEDURAL_SIZE * 4) as usize
                );
                assert!(
                    t.image.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
                    "{m:?} must be opaque"
                );
                assert!(t.scale_in[0] > 0.0 && t.scale_in[1] > 0.0);
                assert!(textured(m));
            }
            None => assert!(!textured(m), "{m:?}"),
        }
    }
    assert!(textured_count >= 14, "{textured_count}");
    // Glass, trim and the selection tint stay flat.
    for m in [
        Material::Glass,
        Material::WindowGlass,
        Material::Trim,
        Material::Selection,
        Material::WallInterior,
    ] {
        assert!(store.material(m).is_none(), "{m:?}");
    }
}

#[test]
fn a_chief_file_wins_over_the_fallback_and_names_its_tile_size() {
    let dir = std::env::temp_dir().join(format!("plan_tex_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The decoder sniffs content, so a PNG under a ".jpg" name is fine.
    let img = Rgba8Image::filled(8, 4, [200, 30, 20, 255]);
    std::fs::write(dir.join("Brick(36).jpg"), image::png::encode_rgba(&img)).unwrap();
    let store = TextureStore::with_dirs(vec![dir.clone()]);
    let t = store.material(Material::Brick).expect("brick texture");
    assert_eq!(t.origin, TextureOrigin::File(dir.join("Brick(36).jpg")));
    assert_eq!((t.image.width, t.image.height), (8, 4));
    assert_eq!(t.scale_in, [36.0, 36.0]);
    // Other materials still fall back.
    assert_eq!(
        store.material(Material::Grass).unwrap().origin,
        TextureOrigin::Procedural
    );
    // A broken file falls back too.
    std::fs::write(dir.join("Stucco(48).jpg"), b"not an image").unwrap();
    assert_eq!(
        store.material(Material::Stucco).unwrap().origin,
        TextureOrigin::Procedural
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tile_size_in_the_file_name() {
    assert_eq!(tile_inches_from_name("Brick(36).jpg"), Some(36.0));
    assert_eq!(tile_inches_from_name("Water1(60).JPG"), Some(60.0));
    assert_eq!(tile_inches_from_name("Gravel.jpg"), None);
    assert_eq!(tile_inches_from_name("Odd(2).jpg"), None);
    assert_eq!(tile_inches_from_name("Odd(abc).jpg"), None);
}

fn rows_gap(t: &TextureImage, a: u32, b: u32) -> f64 {
    let w = t.image.width;
    (0..w)
        .map(|x| {
            let (p, q) = (t.image.pixel(x, a), t.image.pixel(x, b));
            (0..3)
                .map(|k| (i32::from(p[k]) - i32::from(q[k])).abs() as f64)
                .sum::<f64>()
                / 3.0
        })
        .sum::<f64>()
        / f64::from(w)
}

fn cols_gap(t: &TextureImage, a: u32, b: u32) -> f64 {
    let h = t.image.height;
    (0..h)
        .map(|y| {
            let (p, q) = (t.image.pixel(a, y), t.image.pixel(b, y));
            (0..3)
                .map(|k| (i32::from(p[k]) - i32::from(q[k])).abs() as f64)
                .sum::<f64>()
                / 3.0
        })
        .sum::<f64>()
        / f64::from(h)
}

#[test]
fn procedural_textures_tile_seamlessly() {
    for m in Material::ALL {
        let Some(t) = procedural(m) else { continue };
        let n = t.image.height;
        // The wrap-around seam must look like any other pair of neighbours.
        let interior_rows: f64 =
            (0..n - 1).map(|r| rows_gap(&t, r, r + 1)).sum::<f64>() / f64::from(n - 1);
        let interior_cols: f64 =
            (0..n - 1).map(|c| cols_gap(&t, c, c + 1)).sum::<f64>() / f64::from(n - 1);
        let seam_rows = rows_gap(&t, n - 1, 0);
        let seam_cols = cols_gap(&t, n - 1, 0);
        assert!(
            seam_rows <= interior_rows * 2.5 + 2.0,
            "{m:?}: row seam {seam_rows:.2} vs interior {interior_rows:.2}"
        );
        assert!(
            seam_cols <= interior_cols * 2.5 + 2.0,
            "{m:?}: column seam {seam_cols:.2} vs interior {interior_cols:.2}"
        );
    }
}

#[test]
fn procedural_textures_actually_vary() {
    // A brick wall must not be a flat color.
    let t = procedural(Material::Brick).unwrap();
    let first = t.image.pixel(0, 0);
    let varied = t
        .image
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| (0..3).any(|k| (i32::from(p[k]) - i32::from(first[k])).abs() > 12))
        .count();
    assert!(varied > t.image.rgba.len() / 4 / 4, "{varied}");
}

#[test]
fn cache_evicts_least_recently_used_by_bytes() {
    let one = (PROCEDURAL_SIZE * PROCEDURAL_SIZE * 4) as usize;
    let store = TextureStore::with_dirs(Vec::new()).with_capacity(one * 2 + one / 2);
    let k = TextureStore::material_key;
    store.material(Material::Brick).unwrap();
    store.material(Material::Stucco).unwrap();
    assert_eq!(store.cached_count(), 2);
    // Touch Brick so Stucco is the oldest, then load a third.
    store.material(Material::Brick).unwrap();
    store.material(Material::Concrete).unwrap();
    assert_eq!(store.cached_count(), 2);
    assert!(store.cached_bytes() <= one * 2 + one / 2);
    assert!(store.is_cached(&k(Material::Brick)));
    assert!(store.is_cached(&k(Material::Concrete)));
    assert!(!store.is_cached(&k(Material::Stucco)));
    // An evicted texture comes back on demand.
    assert!(store.material(Material::Stucco).is_some());
    store.clear();
    assert_eq!(store.cached_bytes(), 0);
}

#[test]
fn definition_textures_use_the_material_scale() {
    let lib = crate::core_library();
    let def = lib.find("Brick – Red").expect("core brick");
    let store = no_chief();
    let t = store.definition(def);
    assert_eq!(t.scale_in, [32.0, 9.0]);
    assert_eq!(t.image.width, crate::DEFAULT_TEXTURE_SIZE);
    // Cached: the same Arc comes back.
    assert!(Arc::ptr_eq(&t, &store.definition(def)));
}

#[test]
fn bilinear_sampling_wraps_and_blends() {
    let mut img = Rgba8Image::filled(2, 2, [0, 0, 0, 255]);
    img.rgba[0..4].copy_from_slice(&[255, 255, 255, 255]); // (0,0) white
    let t = TextureImage::new(img, TextureOrigin::Procedural, [12.0, 12.0]);
    // Texel centers.
    let c00 = t.sample(0.25, 0.25);
    assert!((c00[0] - 1.0).abs() < 1e-5);
    let c10 = t.sample(0.75, 0.25);
    assert!(c10[0].abs() < 1e-5);
    // Halfway between centers: linear-light mean of white and black.
    let mid = t.sample(0.5, 0.25);
    assert!((mid[0] - 0.5).abs() < 1e-3, "{mid:?}");
    // Wraps in both directions.
    assert_eq!(t.sample(0.25, 0.25), t.sample(3.25, -2.75));
    assert!((t.average[0] - 0.25).abs() < 1e-3);
}

// ---- planar_uv --------------------------------------------------------------

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Two orthonormal in-plane directions of a plane with normal `n`.
fn plane_axes(n: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let helper = if n[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let a = norm(cross(helper, n));
    (a, cross(n, a))
}

#[test]
fn planar_uv_is_isometric_in_every_wall_and_roof_plane() {
    let inv = [1.0 / 24.0, 1.0 / 12.0];
    let normals = [
        norm([0.0, 0.0, 1.0]),
        norm([1.0, 0.0, 0.0]),
        norm([1.0, 0.0, 1.0]),
        norm([-0.3, 0.0, 0.8]),
        norm([0.0, 0.6, 0.8]),  // 37 degree roof
        norm([0.7, 0.5, -0.2]), // hip
        norm([0.0, -0.6, 0.8]), // soffit
    ];
    for n in normals {
        let (ax, ay) = plane_axes(n);
        let p0 = [37.0, 54.0, -81.0];
        let uv0 = planar_uv(p0, n, Projection::Auto, inv);
        // Every in-plane step of d inches moves the texture by d / scale in
        // (u: 24", v: 12"): no stretching, no skew.
        for (axis, d) in [
            (ax, 10.0f32),
            (ay, 10.0),
            (norm([ax[0] + ay[0], ax[1] + ay[1], ax[2] + ay[2]]), 14.0),
        ] {
            let p1 = [
                p0[0] + axis[0] * d,
                p0[1] + axis[1] * d,
                p0[2] + axis[2] * d,
            ];
            let uv1 = planar_uv(p1, n, Projection::Auto, inv);
            let du = (uv1[0] - uv0[0]) * 24.0;
            let dv = (uv1[1] - uv0[1]) * 12.0;
            assert!(
                (du * du + dv * dv).sqrt() - d < 1e-2 && (du * du + dv * dv).sqrt() - d > -1e-2,
                "{n:?}: {du} {dv} vs {d}"
            );
        }
        // Normal displacement does not move the mapping.
        let off = [p0[0] + n[0] * 5.0, p0[1] + n[1] * 5.0, p0[2] + n[2] * 5.0];
        let uv2 = planar_uv(off, n, Projection::Auto, inv);
        assert!(
            (uv2[0] - uv0[0]).abs() < 1e-4 && (uv2[1] - uv0[1]).abs() < 1e-4,
            "{n:?}"
        );
    }
}

#[test]
fn planar_uv_orientation_and_repeat_scale() {
    let inv = [1.0 / 48.0, 1.0 / 48.0];
    // A wall facing +Z (viewer at +Z looking toward -Z): u grows to the
    // viewer's right (+X); v grows downward so the image top points up.
    let n = [0.0, 0.0, 1.0];
    let a = planar_uv([0.0, 0.0, 0.0], n, Projection::Auto, inv);
    let right = planar_uv([48.0, 0.0, 0.0], n, Projection::Auto, inv);
    let up = planar_uv([0.0, 48.0, 0.0], n, Projection::Auto, inv);
    assert!((right[0] - a[0] - 1.0).abs() < 1e-5 && (right[1] - a[1]).abs() < 1e-5);
    assert!((up[1] - a[1] + 1.0).abs() < 1e-5 && (up[0] - a[0]).abs() < 1e-5);
    // The wall facing -Z (seen from the other side) is not mirrored.
    let n = [0.0, 0.0, -1.0];
    let a = planar_uv([0.0, 0.0, 0.0], n, Projection::Auto, inv);
    let right = planar_uv([-48.0, 0.0, 0.0], n, Projection::Auto, inv);
    assert!((right[0] - a[0] - 1.0).abs() < 1e-5);
    // Floors and ground use plan XZ at the repeat scale.
    let f = planar_uv([96.0, 5.0, -48.0], [0.0, 1.0, 0.0], Projection::Auto, inv);
    assert!((f[0] - 2.0).abs() < 1e-5 && (f[1] + 1.0).abs() < 1e-5);
    let g = planar_uv(
        [96.0, 5.0, -48.0],
        [0.3, 0.9, 0.1],
        Projection::GroundXz,
        inv,
    );
    assert_eq!(f, g);
    // A roof panel sloping toward +Z: moving up the slope decreases v.
    let n = norm([0.0, 0.6, 0.8]);
    let a = planar_uv([0.0, 0.0, 0.0], n, Projection::Auto, inv);
    let upslope = [0.0, 0.8, -0.6];
    let b = planar_uv(
        [upslope[0] * 48.0, upslope[1] * 48.0, upslope[2] * 48.0],
        n,
        Projection::Auto,
        inv,
    );
    assert!(
        (b[1] - a[1] + 1.0).abs() < 1e-4 && (b[0] - a[0]).abs() < 1e-4,
        "{a:?} {b:?}"
    );
}

#[test]
fn planar_uv_is_continuous_across_a_face_and_between_neighbouring_normals() {
    let inv = [1.0 / 36.0, 1.0 / 36.0];
    // Across a face: tiny steps give tiny UV steps (no jump).
    for n in [
        norm([0.2, 0.0, 1.0]),
        norm([0.0, 0.5, 0.86]),
        [0.0, 1.0, 0.0],
    ] {
        let mut prev = planar_uv([0.0, 0.0, 0.0], n, Projection::Auto, inv);
        let (ax, ay) = plane_axes(n);
        for i in 1..=200 {
            let s = i as f32 * 0.5;
            let p = [
                ax[0] * s + ay[0] * s * 0.3,
                ax[1] * s + ay[1] * s * 0.3,
                ax[2] * s + ay[2] * s * 0.3,
            ];
            let uv = planar_uv(p, n, Projection::Auto, inv);
            assert!(
                (uv[0] - prev[0]).abs() < 0.03 && (uv[1] - prev[1]).abs() < 0.03,
                "{n:?} step {i}"
            );
            prev = uv;
        }
    }
    // Slightly different normals on a wall or roof give nearly the same UV.
    let p = [100.0, 60.0, -40.0];
    let base = norm([0.4, 0.3, 0.86]);
    let uv0 = planar_uv(p, base, Projection::Auto, inv);
    let uv1 = planar_uv(p, norm([0.41, 0.3, 0.86]), Projection::Auto, inv);
    assert!((uv0[0] - uv1[0]).abs() < 0.05 && (uv0[1] - uv1[1]).abs() < 0.05);
    // Degenerate normals are safe.
    let z = planar_uv(p, [0.0, 0.0, 0.0], Projection::Auto, inv);
    assert!(z[0].is_finite() && z[1].is_finite());
    assert_eq!(sub(p, p), [0.0, 0.0, 0.0]);
}

#[test]
fn ground_materials_project_flat() {
    for m in [
        Material::Floor,
        Material::Ceiling,
        Material::Grass,
        Material::Water,
        Material::Asphalt,
    ] {
        assert_eq!(projection(m), Projection::GroundXz, "{m:?}");
    }
    for m in [
        Material::Brick,
        Material::Roof,
        Material::Siding,
        Material::Stucco,
    ] {
        assert_eq!(projection(m), Projection::Auto, "{m:?}");
    }
}
