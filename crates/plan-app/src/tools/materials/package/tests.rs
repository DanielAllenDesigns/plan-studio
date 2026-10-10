use super::*;
use crate::tools::materials::{self as mat};
use plan_library::image::{png, Rgba8Image};
use plan_materials::package::store_zip;
use plan_materials::MapKind;

fn png_of(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    png::encode_rgba(&Rgba8Image::filled(w, h, rgba))
}

/// A synthetic Lightbeans-style zip (store-only entries, tiny PNG maps).
pub(crate) fn synthetic_zip(name: &str) -> Vec<u8> {
    let meta = format!("Name: {name}\nManufacturer: Acme Stone\nWidth: 600 mm\nHeight: 300 mm\n");
    let albedo = png_of(8, 8, [150, 140, 130, 255]);
    let normal = png_of(8, 8, [128, 128, 255, 255]);
    let rough = png_of(8, 8, [180, 180, 180, 255]);
    let ao = png_of(8, 8, [220, 220, 220, 255]);
    store_zip(&[
        ("productmetadata.txt", meta.as_bytes()),
        (&format!("{name}_BaseColor.png"), &albedo),
        (&format!("{name}_Normal.png"), &normal),
        (&format!("{name}_Roughness.png"), &rough),
        (&format!("{name}_AO.png"), &ao),
    ])
}

/// A fresh sandbox: textures folder, user library file and Downloads folder
/// under one temp dir, none of them the real ones.
pub(crate) struct Sandbox {
    pub root: PathBuf,
    pub downloads: PathBuf,
}

impl Sandbox {
    pub fn new(tag: &str) -> Sandbox {
        let root = std::env::temp_dir().join(format!("ps-pkgapp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let downloads = root.join("Downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        set_textures_root_for_test(Some(root.join("textures")));
        set_downloads_for_test(Some(downloads.clone()));
        mat::set_user_path_for_test(Some(root.join("materials.json")));
        mat::set_user_library_for_test(plan_materials::MaterialLibrary::default());
        Sandbox { root, downloads }
    }

    pub fn write_zip(&self, file: &str, name: &str) -> PathBuf {
        let p = self.downloads.join(file);
        std::fs::write(&p, synthetic_zip(name)).unwrap();
        p
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        set_textures_root_for_test(None);
        set_downloads_for_test(None);
        mat::set_user_path_for_test(None);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn cx() -> EditorContext {
    EditorContext::new(crate::plan_defaults::embedded())
}

#[test]
fn importing_a_zip_copies_the_maps_and_saves_a_material() {
    let sb = Sandbox::new("import");
    let zip = sb.write_zip("granite.zip", "Granite Grey");
    let (def, warnings) = import_zip(&zip).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(def.name, "Granite Grey");
    assert_eq!(def.manufacturer, "Acme Stone");
    assert_eq!(def.category, vec!["Lightbeans".to_string()]);
    assert!((def.texture_scale_in.0 - 600.0 / 25.4).abs() < 1e-6);
    // The copies live under the textures folder, not next to the zip.
    let albedo = def.texture_path.clone().unwrap();
    assert!(
        albedo.starts_with(sb.root.join("textures").to_str().unwrap()),
        "{albedo}"
    );
    for k in [
        MapKind::Albedo,
        MapKind::Normal,
        MapKind::Roughness,
        MapKind::Ao,
    ] {
        assert!(
            std::path::Path::new(def.map_path(k).unwrap()).is_file(),
            "{k:?}"
        );
    }
    // Deleting the download leaves the material whole.
    std::fs::remove_file(&zip).unwrap();
    assert!(std::path::Path::new(&albedo).is_file());
    assert_eq!(def.package_source.as_deref(), Some(zip.to_str().unwrap()));
    // It is in My Materials, saved to the (test) file.
    let lib = mat::library();
    assert!(lib.find("Granite Grey").is_some());
    let text = std::fs::read_to_string(sb.root.join("materials.json")).unwrap();
    assert!(text.contains("Granite Grey") && text.contains("normal_map"));
    assert_eq!(imported_packages().len(), 1);
}

#[test]
fn many_zips_import_together_and_the_last_becomes_active() {
    let sb = Sandbox::new("many");
    let a = sb.write_zip("a.zip", "Slate A");
    let b = sb.write_zip("b.zip", "Slate B");
    let junk = sb.downloads.join("junk.zip");
    std::fs::write(&junk, b"not a zip").unwrap();
    let mut cx = cx();
    let names = import_zips(&mut cx, &[a, junk.clone(), b]);
    assert_eq!(names, vec!["Slate A".to_string(), "Slate B".to_string()]);
    assert!(cx.status.contains("could not import"), "{}", cx.status);
    assert_eq!(mat::active_material().as_deref(), Some("Slate B"));
    let mut cx2 = self::cx();
    assert!(import_zips(&mut cx2, &[junk]).is_empty());
    assert!(cx2.status.starts_with("Could not import"), "{}", cx2.status);
}

#[test]
fn the_import_command_uses_the_file_box_and_survives_cancel() {
    let sb = Sandbox::new("cmd");
    let z = sb.write_zip("c.zip", "Oak Plank");
    let mut cx = cx();
    set_next_pick_for_test(Some(vec![z]));
    cx.run_custom(IMPORT_PACKAGE);
    assert!(cx.status.contains("Imported Oak Plank"), "{}", cx.status);
    cx.run_custom(IMPORT_PACKAGE);
    assert_eq!(cx.status, "No material package chosen");
}

#[test]
fn dropped_zips_import_and_other_files_are_left_alone() {
    let sb = Sandbox::new("drop");
    let z = sb.write_zip("d.zip", "Terrazzo");
    let mut cx = cx();
    assert_eq!(
        handle_dropped(&mut cx, &[PathBuf::from("/x/plan.psplan")]),
        0
    );
    assert_eq!(
        handle_dropped(&mut cx, &[z, PathBuf::from("/x/readme.txt")]),
        1
    );
    assert!(mat::library().find("Terrazzo").is_some());
    // A zip of something else is reported, not swallowed.
    let other = sb.downloads.join("other.zip");
    std::fs::write(&other, store_zip(&[("readme.txt", b"hi")])).unwrap();
    assert_eq!(handle_dropped(&mut cx, &[other]), 1);
    assert!(
        cx.status.contains("albedo") || cx.status.contains("Could not"),
        "{}",
        cx.status
    );
}

#[test]
fn the_browse_row_opens_the_lightbeans_page() {
    let _sb = Sandbox::new("browse");
    let mut cx = cx();
    cx.run_custom(BROWSE_LIGHTBEANS);
    assert_eq!(
        opened_urls_for_test().last().map(String::as_str),
        Some(LIGHTBEANS_URL)
    );
    assert!(LIGHTBEANS_URL.starts_with("https://lightbeans.com/en/textures"));
}

#[test]
fn scanning_downloads_finds_only_new_package_zips_once() {
    let sb = Sandbox::new("scan");
    let pkg = sb.write_zip("granite.zip", "Granite");
    std::fs::write(
        sb.downloads.join("taxes.zip"),
        store_zip(&[("a.txt", b"x")]),
    )
    .unwrap();
    std::fs::write(sb.downloads.join("notes.txt"), b"hi").unwrap();
    std::fs::write(sb.downloads.join("half.zip"), b"PK\x03\x04 incomplete").unwrap();
    let mut seen = HashMap::new();
    let epoch = SystemTime::UNIX_EPOCH;
    let found = scan_downloads(&sb.downloads, epoch, &mut seen);
    assert_eq!(found, vec![pkg.clone()]);
    // Every zip was looked into once.
    assert_eq!(seen.len(), 3);
    assert!(seen[&pkg].2);
    // Nothing newer than "now + a day" is found: old zips are not new.
    let future = SystemTime::now() + Duration::from_secs(86_400);
    assert!(scan_downloads(&sb.downloads, future, &mut seen).is_empty());
    assert!(scan_downloads(&sb.downloads.join("missing"), epoch, &mut seen).is_empty());
}

#[test]
fn the_watcher_lists_new_downloads_until_imported_or_dismissed() {
    let sb = Sandbox::new("watch");
    sb.write_zip("one.zip", "Watch One");
    let two = sb.write_zip("two.zip", "Watch Two");
    // Off by default: nothing is listed.
    crate::dialogs::preferences::pages::update(|p| p.render.watch_downloads = false);
    assert!(!poll_downloads(Instant::now(), true));
    assert!(new_downloads().is_empty());
    crate::dialogs::preferences::pages::update(|p| p.render.watch_downloads = true);
    let t0 = Instant::now();
    assert!(
        poll_downloads(t0, true),
        "the first look finds the two zips"
    );
    assert_eq!(new_downloads().len(), 2);
    // Within three seconds nothing is looked at again.
    sb.write_zip("three.zip", "Watch Three");
    assert!(!poll_downloads(t0 + Duration::from_secs(1), false));
    assert_eq!(new_downloads().len(), 2);
    assert!(poll_downloads(
        t0 + POLL_EVERY + Duration::from_millis(1),
        false
    ));
    assert_eq!(new_downloads().len(), 3);
    // Importing from the group removes it; dismissing hides it for good.
    let mut cx = cx();
    import_zips(&mut cx, std::slice::from_ref(&two));
    assert_eq!(new_downloads().len(), 2);
    let one = sb.downloads.join("one.zip");
    dismiss_download(&one);
    assert_eq!(new_downloads(), vec![sb.downloads.join("three.zip")]);
    poll_downloads(t0 + Duration::from_secs(20), false);
    assert_eq!(new_downloads(), vec![sb.downloads.join("three.zip")]);
    // Switching the preference off empties the group.
    crate::dialogs::preferences::pages::update(|p| p.render.watch_downloads = false);
    poll_downloads(t0 + Duration::from_secs(30), false);
    assert!(new_downloads().is_empty());
}

#[test]
fn the_lightbeans_folder_draws_headlessly_with_packages_and_new_downloads() {
    let sb = Sandbox::new("ui");
    let z = sb.write_zip("ui.zip", "Drawn Tile");
    sb.write_zip("new.zip", "Fresh Tile");
    let mut cx = cx();
    import_zips(&mut cx, &[z]);
    crate::dialogs::preferences::pages::update(|p| p.render.watch_downloads = true);
    poll_downloads(Instant::now(), true);
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                lightbeans_folder(ui, &mut cx);
                mat::browser::panel(ui, &mut cx);
            });
        });
    }
    crate::dialogs::preferences::pages::update(|p| p.render.watch_downloads = false);
    poll_downloads(Instant::now(), true);
}
