use super::*;
use plan_library::image::png;

#[test]
fn tangent_frames_are_orthonormal_right_handed_and_follow_planar_uv() {
    use crate::textures::planar_uv;
    let normals = [
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.6, 0.3, 0.74],
        [-0.5, 0.5, 0.7],
        [0.0, 0.7, -0.7],
    ];
    for n in normals {
        let nn = norm3(n);
        let (r, u) = tangent_frame(n, Projection::Auto);
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        assert!(dot(r, u).abs() < 1e-5 && dot(r, nn).abs() < 1e-5 && dot(u, nn).abs() < 1e-5);
        assert!((dot(r, r) - 1.0).abs() < 1e-5 && (dot(u, u) - 1.0).abs() < 1e-5);
        let cross = [
            r[1] * u[2] - r[2] * u[1],
            r[2] * u[0] - r[0] * u[2],
            r[0] * u[1] - r[1] * u[0],
        ];
        assert!(dot(cross, nn) > 0.99, "{n:?} right x up = normal");
        // Moving along `right` grows u; moving up the image shrinks v.
        let p0 = [3.0, 4.0, 5.0];
        let step = |d: [f32; 3]| [p0[0] + d[0], p0[1] + d[1], p0[2] + d[2]];
        let uv0 = planar_uv(p0, n, Projection::Auto, [1.0, 1.0]);
        let ur = planar_uv(step(r), n, Projection::Auto, [1.0, 1.0]);
        let uu = planar_uv(step(u), n, Projection::Auto, [1.0, 1.0]);
        assert!(
            (ur[0] - uv0[0] - 1.0).abs() < 1e-4 && (ur[1] - uv0[1]).abs() < 1e-4,
            "{n:?}"
        );
        assert!(
            (uu[1] - uv0[1] + 1.0).abs() < 1e-4 && (uu[0] - uv0[0]).abs() < 1e-4,
            "{n:?}"
        );
    }
    // Ground projection: u along +x, the image rises toward -z.
    let (r, u) = tangent_frame([0.0, 1.0, 0.0], Projection::GroundXz);
    assert_eq!((r, u), ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]));
    let uv0 = planar_uv([0.0; 3], [0.0, 1.0, 0.0], Projection::GroundXz, [1.0, 1.0]);
    let uz = planar_uv(
        [0.0, 0.0, -1.0],
        [0.0, 1.0, 0.0],
        Projection::GroundXz,
        [1.0, 1.0],
    );
    assert!((uz[1] - uv0[1] + 1.0).abs() < 1e-6);
    // A flat face falls back to the ground frame; a zero normal does not panic.
    assert_eq!(
        tangent_frame([0.0, 1.0, 0.0], Projection::Auto).0,
        [1.0, 0.0, 0.0]
    );
    let _ = tangent_frame([0.0; 3], Projection::Auto);
}

#[test]
fn a_flat_tangent_normal_leaves_the_face_normal_and_a_tilt_leans_toward_the_tangent() {
    let n = [0.0, 0.0, 1.0];
    let frame = tangent_frame(n, Projection::Auto);
    let flat = perturb_normal(n, frame, [0.0, 0.0, 1.0]);
    assert!((flat[2] - 1.0).abs() < 1e-6);
    let lean = perturb_normal(n, frame, norm3([0.5, 0.0, 1.0]));
    // Leaning toward +u moves the normal along `right`.
    let r = frame.0;
    assert!(lean[0] * r[0] + lean[1] * r[1] + lean[2] * r[2] > 0.3);
    assert!((lean.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 1e-5);
}

fn write_png(dir: &Path, name: &str, w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> String {
    let mut img = Rgba8Image::filled(w, h, [0, 0, 0, 255]);
    for y in 0..h {
        for x in 0..w {
            let o = ((y * w + x) * 4) as usize;
            img.rgba[o..o + 4].copy_from_slice(&f(x, y));
        }
    }
    let p = dir.join(name);
    std::fs::write(&p, png::encode_rgba(&img)).unwrap();
    p.display().to_string()
}

fn temp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ps-pbr-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_set_loads_packs_orm_and_samples_the_maps() {
    let dir = temp("set");
    let mut def = MaterialDef::new("Slab", &["T"], [100, 100, 100]);
    def.bump = 0.5;
    def.normal_map = Some(write_png(&dir, "n.png", 4, 4, |_, _| [255, 128, 128, 255]));
    def.roughness_map = Some(write_png(&dir, "r.png", 4, 4, |_, _| [51, 51, 51, 255]));
    def.ao_map = Some(write_png(&dir, "a.png", 2, 2, |_, _| [128, 128, 128, 255]));
    def.opacity_map = Some(write_png(&dir, "o.png", 4, 4, |x, _| {
        if x < 2 {
            [255; 4]
        } else {
            [0, 0, 0, 255]
        }
    }));
    let src = PbrSource::from_def(&def).expect("has maps");
    let set = PbrSet::load(&src, 64, false);
    assert_eq!(set.flags, PBR_NORMAL | PBR_ROUGH | PBR_AO | PBR_CUTOUT);
    let s = set.sample(0.25, 0.5);
    // Red 255 = fully +x tangent normal.
    let n = s.normal.unwrap();
    assert!(n[0] > 0.95, "{n:?}");
    assert!((s.roughness.unwrap() - 0.2).abs() < 0.02);
    assert!((s.ao.unwrap() - 0.5).abs() < 0.02);
    assert!(s.metallic.is_none());
    assert!(s.opacity.unwrap() > 0.9);
    assert!(set.sample(0.75, 0.5).opacity.unwrap() < 0.1);
    // Opacity folds into an albedo's alpha.
    let mut rgba = vec![255u8; 4 * 4 * 4];
    set.fold_opacity(&mut rgba, 4, 4);
    assert_eq!(rgba[3], 255);
    assert_eq!(rgba[3 * 4 + 3], 0);
    // Switching a map off removes it from the set (and from the key).
    let key = src.key();
    let mut off = def.clone();
    off.set_map_enabled(MapKind::Normal, false);
    let src2 = PbrSource::from_def(&off).unwrap();
    assert_ne!(src2.key(), key);
    assert_eq!(PbrSet::load(&src2, 64, false).flags & PBR_NORMAL, 0);
    // Every map off: no source at all.
    for k in MapKind::ALL {
        off.set_map_enabled(k, false);
    }
    assert!(PbrSource::from_def(&off).is_none());
    // The cache hands back the same set.
    let a = load_cached(&src, 64, false);
    let b = load_cached(&src, 64, false);
    assert!(Arc::ptr_eq(&a, &b));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_height_map_becomes_a_normal_map_and_directx_normals_flip() {
    let dir = temp("height");
    // A ramp rising to the right (bright on the right).
    let mut def = MaterialDef::new("Ramp", &["T"], [100, 100, 100]);
    def.bump = 0.5;
    def.height_map = Some(write_png(&dir, "h.png", 8, 8, |x, _| {
        let v = (x * 30) as u8;
        [v, v, v, 255]
    }));
    let set = PbrSet::load(&PbrSource::from_def(&def).unwrap(), 64, false);
    assert!(set.flags & PBR_NORMAL != 0);
    // Heights rise toward +u so the surface leans toward -u: negative x.
    let n = set.sample(0.5, 0.5).normal.unwrap();
    assert!(n[0] < -0.1 && n[2] > 0.5, "{n:?}");
    // A zero bump slider drops the derived normals.
    def.bump = 0.0;
    assert_eq!(
        PbrSet::load(&PbrSource::from_def(&def).unwrap(), 64, false).flags & PBR_NORMAL,
        0
    );

    // DirectX green is flipped on load.
    let mut dx = MaterialDef::new("Dx", &["T"], [1, 1, 1]);
    dx.bump = 0.5;
    dx.normal_map = Some(write_png(&dir, "n.png", 2, 2, |_, _| [128, 255, 128, 255]));
    let up = PbrSet::load(&PbrSource::from_def(&dx).unwrap(), 64, false)
        .sample(0.5, 0.5)
        .normal
        .unwrap();
    assert!(up[1] > 0.9);
    dx.normal_flip_y = true;
    let down = PbrSet::load(&PbrSource::from_def(&dx).unwrap(), 64, false)
        .sample(0.5, 0.5)
        .normal
        .unwrap();
    assert!(down[1] < -0.9);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_bump_slider_scales_the_normals() {
    let dir = temp("bump");
    let mut def = MaterialDef::new("Tilt", &["T"], [1, 1, 1]);
    // Normal leaning 45 degrees toward +x.
    def.normal_map = Some(write_png(&dir, "n.png", 2, 2, |_, _| [218, 128, 218, 255]));
    let at = |bump: f32| {
        let mut d = def.clone();
        d.bump = bump;
        PbrSet::load(&PbrSource::from_def(&d).unwrap(), 64, false)
            .sample(0.5, 0.5)
            .normal
            .unwrap()
    };
    let flat = at(0.0);
    let neutral = at(0.5);
    let strong = at(1.0);
    assert!(flat[0].abs() < 0.02 && flat[2] > 0.99);
    assert!((neutral[0] - 0.7).abs() < 0.05);
    assert!(strong[0] > neutral[0] + 0.1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_painted_mesh_table_finds_what_was_registered() {
    let def = {
        let mut d = MaterialDef::new("Reg", &["T"], [9, 9, 9]);
        d.normal_map = Some("/nope/n.png".into());
        d
    };
    let src = Arc::new(PbrSource::from_def(&def).unwrap());
    let id: Id = 990_001;
    assert!(lookup(Some(id), Material::Floor, Some([9, 9, 9])).is_none());
    register(id, Material::Floor, [9, 9, 9], Some(Arc::clone(&src)));
    let found = lookup(Some(id), Material::Floor, Some([9, 9, 9])).unwrap();
    assert_eq!(found.key(), src.key());
    assert!(source_by_key(src.key()).is_some());
    assert!(lookup(Some(id), Material::Floor, Some([9, 9, 8])).is_none());
    assert!(lookup(None, Material::Floor, Some([9, 9, 9])).is_none());
    register(id, Material::Floor, [9, 9, 9], None);
    assert!(lookup(Some(id), Material::Floor, Some([9, 9, 9])).is_none());
    // A missing file just yields an empty set.
    assert_eq!(PbrSet::load(&src, 64, false).flags, 0);
}
