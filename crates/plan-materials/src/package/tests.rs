use super::*;
use plan_library::image::png;

fn png_of(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    png::encode_rgba(&Rgba8Image::filled(w, h, rgba))
}

fn lightbeans_zip(meta: &str) -> Vec<u8> {
    let albedo = png_of(8, 8, [180, 90, 60, 255]);
    let normal = png_of(8, 8, [128, 128, 255, 255]);
    let rough = png_of(8, 8, [200, 200, 200, 255]);
    let height = png_of(8, 8, [100, 100, 100, 255]);
    let ao = png_of(4, 4, [255, 255, 255, 255]);
    store_zip(&[
        ("Brick_Red/", b""),
        ("Brick_Red/productmetadata.txt", meta.as_bytes()),
        ("Brick_Red/Brick_Red_BaseColor.png", &albedo),
        ("Brick_Red/Brick_Red_Normal.png", &normal),
        ("Brick_Red/Brick_Red_Roughness.png", &rough),
        ("Brick_Red/Brick_Red_Displacement.png", &height),
        ("Brick_Red/Brick_Red_AO.png", &ao),
        ("Brick_Red/preview.jpg", b"not a map"),
        ("__MACOSX/Brick_Red/._Brick_Red_Normal.png", b"junk"),
    ])
}

#[test]
fn map_names_are_matched_case_insensitively_by_substring() {
    use MapKind::*;
    let k = |n: &str| classify(n).map(|(k, _)| k);
    assert_eq!(k("Tile_BaseColor.jpg"), Some(Albedo));
    assert_eq!(k("tile_BASE_COLOR.PNG"), Some(Albedo));
    assert_eq!(k("tile-diffuse.jpeg"), Some(Albedo));
    assert_eq!(k("tile_albedo_4k.png"), Some(Albedo));
    assert_eq!(k("tile_Normal.png"), Some(Normal));
    assert_eq!(k("tile_nrm.png"), Some(Normal));
    assert_eq!(k("tile_nor_gl.png"), Some(Normal));
    assert_eq!(k("tile_Roughness.jpg"), Some(Roughness));
    assert_eq!(k("tile_metalness.png"), Some(Metallic));
    assert_eq!(k("tile_Height.png"), Some(Height));
    assert_eq!(k("tile_displacement.png"), Some(Height));
    assert_eq!(k("tile_bump.png"), Some(Height));
    assert_eq!(k("tile_AO.png"), Some(Ao));
    assert_eq!(k("tile_AmbientOcclusion.png"), Some(Ao));
    assert_eq!(k("tile_Opacity.png"), Some(Opacity));
    assert_eq!(k("tile_alpha.png"), Some(Opacity));
    // Not maps.
    assert_eq!(k("preview.jpg"), None);
    assert_eq!(k("tile_glossiness.png"), None);
    assert_eq!(k("tile_normal.exr"), None);
    assert_eq!(k("productmetadata.txt"), None);
    // The product's name does not fool it: the word nearest the end wins.
    assert_eq!(k("Metal_Roof_Normal_GL_2K.png"), Some(Normal));
    assert_eq!(k("Metal_Roof_BaseColor.jpg"), Some(Albedo));
    assert_eq!(k("Brick_Displacement_4K.png"), Some(Height));
    assert_eq!(k("BrickBaseColor.png"), Some(Albedo));
    assert_eq!(k("Wood_Floor_Ambient_Occlusion.png"), Some(Ao));
    // DirectX normals are told apart.
    assert_eq!(classify("tile_Normal_DX.png"), Some((Normal, true)));
    assert_eq!(classify("tile_Normal_GL.png"), Some((Normal, false)));
}

#[test]
fn metadata_lines_parse_with_colons_equals_and_units() {
    let meta = parse_metadata("\u{feff}Name: Brick Red\nmanufacturer = Acme Brick\nwidth: 600 mm\nHeight=30 cm\n\nno separator\n");
    assert_eq!(meta[0], ("name".to_string(), "Brick Red".to_string()));
    assert_eq!(meta[1].0, "manufacturer");
    let (w, h) = size_from_metadata(&meta).unwrap();
    assert!((w - 600.0 / 25.4).abs() < 1e-9);
    assert!((h - 300.0 / 25.4).abs() < 1e-9);

    let size = |t: &str| size_from_metadata(&parse_metadata(t)).unwrap();
    let (w, h) = size("Size: 2 x 3 ft");
    assert!((w - 24.0).abs() < 1e-9 && (h - 36.0).abs() < 1e-9);
    let (w, h) = size("Real width (m): 0.5\nReal height (m): 0,25");
    assert!((w - 500.0 / 25.4).abs() < 1e-6 && (h - 250.0 / 25.4).abs() < 1e-6);
    // A single width makes a square tile; inches and quotes work.
    let (w, h) = size("width_in: 24");
    assert_eq!((w, h), (24.0, 24.0));
    let (w, _) = size("\"Width\": \"18 in\",");
    assert_eq!(w, 18.0);
    // No unit at all: millimetres.
    let (w, _) = size("width: 254");
    assert!((w - 10.0).abs() < 1e-9);
    // Displacement depth is not a tile size.
    assert!(size_from_metadata(&parse_metadata("Displacement height: 2 mm")).is_none());
    assert!(size_from_metadata(&parse_metadata("name: x")).is_none());
}

#[test]
fn a_synthetic_zip_reads_into_a_package() {
    let zip = lightbeans_zip(
        "Name: Brick Red\nManufacturer: Acme Brick\nWidth: 600 mm\nHeight: 300 mm\n",
    );
    let pkg = MaterialPackage::read(&zip).unwrap();
    assert_eq!(pkg.name, "Brick Red");
    assert_eq!(pkg.manufacturer, "Acme Brick");
    let (w, h) = pkg.real_size_in.unwrap();
    assert!((w - 23.622).abs() < 0.01 && (h - 11.811).abs() < 0.01);
    assert!(pkg.maps.albedo.is_some());
    assert!(pkg.maps.normal.is_some() && pkg.maps.roughness.is_some());
    assert!(pkg.maps.height.is_some() && pkg.maps.ao.is_some());
    assert!(pkg.maps.metallic.is_none() && pkg.maps.opacity.is_none());
    assert_eq!(pkg.files.len(), 5);
    // The AO map keeps its own (smaller) size.
    assert_eq!(pkg.maps.ao.as_ref().unwrap().width, 4);
    assert!(pkg.has(MapKind::Normal) && !pkg.has(MapKind::Opacity));
    assert!(pkg.warnings.is_empty(), "{:?}", pkg.warnings);
}

#[test]
fn a_zip_without_metadata_falls_back_to_the_given_name_and_warns() {
    let albedo = png_of(4, 4, [10, 20, 30, 255]);
    let zip = store_zip(&[("diffuse.png", &albedo)]);
    let pkg = MaterialPackage::read_with(&zip, "My Stone", &ReadOptions::default()).unwrap();
    assert_eq!(pkg.name, "My Stone");
    assert!(pkg.real_size_in.is_none());
    assert!(pkg.warnings.iter().any(|w| w.contains("real-world size")));
    // No albedo: refused.
    let normal = png_of(4, 4, [128, 128, 255, 255]);
    let bad = store_zip(&[("normal.png", &normal)]);
    assert!(MaterialPackage::read(&bad).unwrap_err().contains("albedo"));
    assert!(MaterialPackage::read(b"not a zip at all, really not a zip").is_err());
}

#[test]
fn big_maps_are_downsampled_for_the_gpu_but_the_file_stays_whole() {
    let albedo = png_of(64, 32, [90, 90, 90, 255]);
    let zip = store_zip(&[("Slab/albedo.png", &albedo)]);
    let pkg = MaterialPackage::read_with(&zip, "Slab", &ReadOptions { max_side: 16 }).unwrap();
    let a = pkg.maps.albedo.as_ref().unwrap();
    assert_eq!((a.width, a.height), (16, 8));
    assert_eq!(pkg.files[0].full_size, (64, 32));
    assert_eq!(pkg.files[0].bytes, albedo);
}

#[test]
fn import_copies_files_and_records_the_source_and_date() {
    let zip = lightbeans_zip("Name: Brick Red\nWidth: 12 in\nHeight: 6 in\n");
    let pkg = MaterialPackage::read(&zip).unwrap();
    let root = std::env::temp_dir().join(format!("ps-pkg-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let imp = pkg.import_to(&root, "2026-10-08").unwrap();
    assert!(imp.dir.ends_with("Brick Red"));
    for (kind, path) in &imp.files {
        assert!(path.is_file(), "{kind:?}");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with(kind.key()), "{name}");
    }
    let record = std::fs::read_to_string(imp.dir.join("package.json")).unwrap();
    assert!(record.contains("2026-10-08") && record.contains("Brick Red"));
    // Re-importing the same package overwrites the folder.
    pkg.import_to(&root, "2026-10-09").unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn from_package_sets_texture_scale_class_maker_and_map_paths() {
    let zip =
        lightbeans_zip("Name: Brick Red\nManufacturer: Acme Brick\nWidth: 12 in\nHeight: 6 in\n");
    let pkg = MaterialPackage::read(&zip).unwrap();
    let root = std::env::temp_dir().join(format!("ps-pkg-def-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let imp = pkg.import_to(&root, "2026-10-08").unwrap();
    let def = MaterialDef::from_package(&pkg, &imp, "2026-10-08");
    assert_eq!(def.name, "Brick Red");
    assert_eq!(def.class, MaterialClass::General);
    assert_eq!(def.manufacturer, "Acme Brick");
    assert_eq!(def.category, vec!["Lightbeans".to_string()]);
    assert!((def.texture_scale_in.0 - 12.0).abs() < 1e-9);
    assert!((def.texture_scale_in.1 - 6.0).abs() < 1e-9);
    assert!(def.texture_path.as_deref().unwrap().ends_with("albedo.png"));
    assert!(def.normal_map.as_deref().unwrap().ends_with("normal.png"));
    assert!(def.roughness_map.is_some() && def.height_map.is_some() && def.ao_map.is_some());
    assert!(def.metallic_map.is_none() && def.opacity_map.is_none());
    assert_eq!(def.package_imported.as_deref(), Some("2026-10-08"));
    assert_eq!(def.color, [180, 90, 60]);
    assert!(def.has_pbr_maps());
    // Per-map switches.
    assert!(def.map_enabled(MapKind::Normal));
    let mut d = def.clone();
    d.set_map_enabled(MapKind::Normal, false);
    assert!(!d.map_enabled(MapKind::Normal) && d.map_enabled(MapKind::Roughness));
    d.set_map_enabled(MapKind::Normal, true);
    assert!(d.maps_off.is_empty());
    // The definition survives JSON, and old files without the fields load.
    let json = serde_json::to_string(&def).unwrap();
    let back: MaterialDef = serde_json::from_str(&json).unwrap();
    assert_eq!(back, def);
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    for k in ["normal_map", "maps_off", "package_source", "normal_flip_y"] {
        v.as_object_mut().unwrap().remove(k);
    }
    let old: MaterialDef = serde_json::from_value(v).unwrap();
    assert!(old.normal_map.is_none() && old.maps_off.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dates_and_folder_names_are_well_formed() {
    assert_eq!(iso_date(0), "1970-01-01");
    assert_eq!(iso_date(951_782_400), "2000-02-29");
    assert_eq!(iso_date(1_791_417_600), "2026-10-08");
    assert_eq!(today().len(), 10);
    assert_eq!(safe_folder_name("A/B:C?"), "A_B_C_");
    assert_eq!(safe_folder_name("  "), "package");
    assert_eq!(
        safe_folder_name("../x"),
        "._x".trim_start_matches('.').to_string()
    );
}
