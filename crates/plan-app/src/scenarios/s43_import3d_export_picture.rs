//! Scenario 43: Import 3D Symbol (STL, 3DS, COLLADA, SketchUp's message) and
//! Export Picture (coverage audit #26, #7).
//!
//! An STL of a tall cabinet in millimeters, Z up, is imported through the
//! dialog (units and axis come from the file's size and format), lands in the
//! User Catalog with the right 3D bounding box, and the Library tool places
//! it in the plan with its 3D model. 3DS and COLLADA fixtures are built in
//! the test; a `.skp` file says how to export instead. Export Picture saves
//! the plan, an elevation and the 3D view as PNG, JPEG, BMP and TIFF at a
//! pixel size or a paper size and DPI.

use super::{draw_shell, Sim};
use crate::dialogs::export_picture::{self as ep, Format, Subject};
use crate::tools::library::user::{self as store, tests_support};
use crate::tools::library::{active_item, clear_active_item};
use crate::tools::ToolId;
use plan_import::{Facing, UpAxis};
use plan_library::{ItemKind, Placement};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ps-s43-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Triangles of a box `w x d x h` from the origin, outward facing.
fn box_tris(w: f32, d: f32, h: f32) -> Vec<[[f32; 3]; 3]> {
    let v = [
        [0.0, 0.0, 0.0],
        [w, 0.0, 0.0],
        [w, d, 0.0],
        [0.0, d, 0.0],
        [0.0, 0.0, h],
        [w, 0.0, h],
        [w, d, h],
        [0.0, d, h],
    ];
    let quads: [[usize; 4]; 6] = [
        [3, 2, 1, 0],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ];
    quads
        .iter()
        .flat_map(|q| [[v[q[0]], v[q[1]], v[q[2]]], [v[q[0]], v[q[2]], v[q[3]]]])
        .collect()
}

fn binary_stl(tris: &[[[f32; 3]; 3]]) -> Vec<u8> {
    let mut out = vec![0u8; 80];
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for t in tris {
        out.extend_from_slice(&[0u8; 12]);
        for p in t {
            for c in p {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        out.extend_from_slice(&[0u8; 2]);
    }
    out
}

fn chunk(id: u16, body: &[u8]) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    v.extend_from_slice(&((body.len() + 6) as u32).to_le_bytes());
    v.extend_from_slice(body);
    v
}

fn cstr(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

/// A 3DS of one object "Table" (a `w x d x h` box) with one material "Top"
/// on every face: diffuse color and an `oak_top.jpg` texture.
fn box_3ds(w: f32, d: f32, h: f32) -> Vec<u8> {
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut faces: Vec<[u16; 3]> = Vec::new();
    for t in box_tris(w, d, h) {
        let mut f = [0u16; 3];
        for (k, p) in t.iter().enumerate() {
            let i = verts.iter().position(|q| q == p).unwrap_or_else(|| {
                verts.push(*p);
                verts.len() - 1
            });
            f[k] = i as u16;
        }
        faces.push(f);
    }
    let mut vbody = (verts.len() as u16).to_le_bytes().to_vec();
    for p in &verts {
        for c in p {
            vbody.extend_from_slice(&c.to_le_bytes());
        }
    }
    let mut mbody = cstr("Top");
    mbody.extend_from_slice(&(faces.len() as u16).to_le_bytes());
    for i in 0..faces.len() as u16 {
        mbody.extend_from_slice(&i.to_le_bytes());
    }
    let mut fbody = (faces.len() as u16).to_le_bytes().to_vec();
    for f in &faces {
        for i in f {
            fbody.extend_from_slice(&i.to_le_bytes());
        }
        fbody.extend_from_slice(&0u16.to_le_bytes());
    }
    fbody.extend(chunk(0x4130, &mbody));
    let mut tri = chunk(0x4110, &vbody);
    tri.extend(chunk(0x4120, &fbody));
    let mut obj = cstr("Table");
    obj.extend(chunk(0x4100, &tri));
    let mut mat = chunk(0xA000, &cstr("Top"));
    mat.extend(chunk(0xA020, &chunk(0x0011, &[150, 110, 60])));
    mat.extend(chunk(0xA200, &chunk(0xA300, &cstr("oak_top.jpg"))));
    let mut editor = chunk(0xAFFF, &mat);
    editor.extend(chunk(0x4000, &obj));
    chunk(0x4D4D, &chunk(0x3D3D, &editor))
}

/// A COLLADA box in centimeters, Z up, with a red effect.
fn box_dae(w: f64, d: f64, h: f64) -> String {
    let v = [
        [0.0, 0.0, 0.0],
        [w, 0.0, 0.0],
        [w, d, 0.0],
        [0.0, d, 0.0],
        [0.0, 0.0, h],
        [w, 0.0, h],
        [w, d, h],
        [0.0, d, h],
    ];
    let pos: Vec<String> = v.iter().flatten().map(|c| c.to_string()).collect();
    format!(
        r##"<?xml version="1.0"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
 <asset><unit meter="0.01" name="centimeter"/><up_axis>Z_UP</up_axis></asset>
 <library_effects><effect id="e"><profile_COMMON><technique sid="c"><phong><diffuse><color>0.8 0.1 0.1 1</color></diffuse></phong></technique></profile_COMMON></effect></library_effects>
 <library_materials><material id="m" name="Red"><instance_effect url="#e"/></material></library_materials>
 <library_geometries><geometry id="g" name="Box"><mesh>
  <source id="p"><float_array id="pa" count="24">{}</float_array><technique_common><accessor source="#pa" count="8" stride="3"/></technique_common></source>
  <vertices id="v"><input semantic="POSITION" source="#p"/></vertices>
  <polylist material="sym" count="6"><input semantic="VERTEX" source="#v" offset="0"/><vcount>4 4 4 4 4 4</vcount>
   <p>3 2 1 0 4 5 6 7 0 1 5 4 1 2 6 5 2 3 7 6 3 0 4 7</p></polylist>
 </mesh></geometry></library_geometries>
 <library_visual_scenes><visual_scene id="s"><node><translate>10 0 0</translate><instance_geometry url="#g"><bind_material><technique_common>
  <instance_material symbol="sym" target="#m"/></technique_common></bind_material></instance_geometry></node></visual_scene></library_visual_scenes>
 <scene><instance_visual_scene url="#s"/></scene>
</COLLADA>"##,
        pos.join(" ")
    )
}

/// A fresh isolated User Catalog and the folder for the files.
fn setup(tag: &str) -> (Sim, PathBuf) {
    let dir = temp_dir(tag);
    tests_support::fresh(false);
    clear_active_item();
    (Sim::new(), dir)
}

#[test]
fn an_stl_in_millimeters_is_imported_and_placed() {
    let (mut sim, dir) = setup("stl");
    // 600 x 600 x 2000 mm, Z up.
    let path = dir.join("tall_cabinet.stl");
    std::fs::write(&path, binary_stl(&box_tris(600.0, 600.0, 2000.0))).unwrap();

    // The command asks for a file (skipped under test); the dialog opens for
    // the path.
    sim.action(crate::toolbar::Action::Custom(
        crate::dialogs::export_picture::IMPORT_3D_SYMBOL,
    ));
    assert!(!crate::dialogs::symbol::import_is_open());
    crate::dialogs::symbol::open_path(sim.cx(), &path);
    assert!(crate::dialogs::symbol::import_is_open());
    sim.dialog_frame(false);
    sim.dialog_frame(false);

    // Units and axis were set from the file.
    let (unit, up, name) = crate::dialogs::symbol::with_import(|s| {
        (
            s.suggestion().unit_name.clone(),
            s.options.up_axis,
            s.name.clone(),
        )
    })
    .unwrap();
    assert_eq!(unit, "Millimeters");
    assert_eq!(up, UpAxis::Z);
    assert_eq!(name, "tall cabinet");
    let size = crate::dialogs::symbol::with_import(|s| s.natural_size().unwrap()).unwrap();
    assert!((size[0] - 23.62).abs() < 0.01, "width {size:?}");
    assert!((size[1] - 23.62).abs() < 0.01, "depth {size:?}");
    assert!((size[2] - 78.74).abs() < 0.01, "height {size:?}");

    crate::dialogs::symbol::with_import(|s| {
        s.name = "Tall Cabinet".into();
        s.placement = Some(Placement::FreeStanding);
    });
    assert!(crate::dialogs::symbol::accept_import(sim.cx()));
    assert!(!crate::dialogs::symbol::import_is_open());

    // In the User Catalog with the model and a plan symbol.
    let id = active_item().expect("the new symbol is the Library tool's item");
    let item = store::item(&id).expect("in the user catalog");
    assert_eq!((item.name.as_str(), item.kind), ("Tall Cabinet", ItemKind::Model));
    assert!((item.width - 23.62).abs() < 0.01 && (item.depth - 23.62).abs() < 0.01);
    assert!((item.height - 78.74).abs() < 0.01);
    assert!(!item.symbol.is_empty());
    assert!(store::model_of(&item).is_some());

    // The request switched to the Library tool; a click places the symbol.
    sim.app.process_requests();
    assert_eq!(sim.app.tools.active_id(), ToolId::Library);
    let r = sim.click(150.0, 150.0);
    assert_eq!(r.commit.as_deref(), Some("Place Symbol"));
    let sym = sim.cx().floor().symbols[0].clone();
    assert_eq!(sym.catalog_id, id);
    assert!((sym.height - 78.74).abs() < 0.01);
    // Its 3D model stands 78.74 in tall.
    let meshes = store::placed_meshes(&sym, 0.0).expect("model meshes");
    let top = meshes
        .iter()
        .filter_map(|m| m.bounds())
        .map(|(_, hi)| hi[1])
        .fold(0.0_f32, f32::max);
    assert!((top - 78.74).abs() < 0.02, "top {top}");

    // One undo removes the placed symbol.
    sim.undo();
    assert!(sim.cx().floor().symbols.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn facing_size_and_units_reshape_the_symbol() {
    let (mut sim, dir) = setup("shape");
    // Ambiguous size: 30 x 10 x 20 raw units, Y up OBJ; the user picks.
    let path = dir.join("bench.stl");
    std::fs::write(&path, binary_stl(&box_tris(30.0, 10.0, 20.0))).unwrap();
    crate::dialogs::symbol::open_path(sim.cx(), &path);
    crate::dialogs::symbol::with_import(|s| {
        s.set_unit(0); // inches
        s.set_up_axis(UpAxis::Z);
    });
    let n = crate::dialogs::symbol::with_import(|s| s.natural_size().unwrap()).unwrap();
    assert!((n[0] - 30.0).abs() < 1e-3 && (n[1] - 10.0).abs() < 1e-3 && (n[2] - 20.0).abs() < 1e-3);
    // Facing right swaps width and depth.
    crate::dialogs::symbol::with_import(|s| s.set_facing(Facing::Right));
    let n = crate::dialogs::symbol::with_import(|s| s.natural_size().unwrap()).unwrap();
    assert!((n[0] - 10.0).abs() < 1e-3 && (n[1] - 30.0).abs() < 1e-3, "{n:?}");
    // Resize proportionally from the height: 40 in high is 2x.
    crate::dialogs::symbol::with_import(|s| {
        s.set_facing(Facing::Front);
        s.resize = true;
        s.set_side(crate::dialogs::symbol::Side::Height, 40.0);
    });
    let shaped = crate::dialogs::symbol::with_import(|s| s.shaped().and_then(|m| m.extent())).unwrap().unwrap();
    assert!((shaped[0] - 60.0).abs() < 0.01 && (shaped[1] - 40.0).abs() < 0.01);
    crate::dialogs::symbol::with_import(|s| {
        s.name = "Bench".into();
        s.elevation = 18.0;
    });
    assert!(crate::dialogs::symbol::accept_import(sim.cx()));
    let item = store::item(&active_item().unwrap()).unwrap();
    assert!((item.width - 60.0).abs() < 0.01 && (item.height - 40.0).abs() < 0.01);
    assert_eq!(item.elevation, 18.0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_3ds_keeps_its_object_and_maps_the_texture_to_a_plan_material() {
    let (mut sim, dir) = setup("tds");
    let path = dir.join("oak_table.3ds");
    std::fs::write(&path, box_3ds(1200.0, 800.0, 750.0)).unwrap();
    crate::dialogs::symbol::open_path(sim.cx(), &path);
    let (unit, up) = crate::dialogs::symbol::with_import(|s| {
        (s.suggestion().unit_name.clone(), s.options.up_axis)
    })
    .unwrap();
    assert_eq!((unit.as_str(), up), ("Millimeters", UpAxis::Z));
    let matches = crate::dialogs::symbol::with_import(|s| s.material_matches().to_vec()).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].texture.as_deref(), Some("oak_top.jpg"));
    assert!(matches[0].applied, "the texture finds an oak material");
    assert!(matches[0]
        .plan_material
        .as_deref()
        .is_some_and(|m| m.to_ascii_lowercase().contains("oak")));
    // Drawn without panicking, then imported with the mapped color.
    sim.dialog_frame(false);
    assert!(crate::dialogs::symbol::accept_import(sim.cx()));
    let item = store::item(&active_item().unwrap()).unwrap();
    let model = store::model_of(&item).unwrap();
    assert_eq!(model.parts[0].color, matches[0].color);
    assert!((item.width - 47.24).abs() < 0.01, "{}", item.width);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_collada_file_brings_its_own_units_axis_and_transform() {
    let (mut sim, dir) = setup("dae");
    let path = dir.join("red_box.dae");
    // 50 x 40 x 90 cm, moved 10 cm along x by its node.
    std::fs::write(&path, box_dae(50.0, 40.0, 90.0)).unwrap();
    crate::dialogs::symbol::open_path(sim.cx(), &path);
    let (unit, up, from_file) = crate::dialogs::symbol::with_import(|s| {
        (
            s.suggestion().unit_name.clone(),
            s.options.up_axis,
            s.suggestion().from_file,
        )
    })
    .unwrap();
    assert_eq!((unit.as_str(), up, from_file), ("Centimeters", UpAxis::Z, true));
    let n = crate::dialogs::symbol::with_import(|s| s.natural_size().unwrap()).unwrap();
    assert!((n[0] - 19.685).abs() < 0.01 && (n[1] - 15.748).abs() < 0.01 && (n[2] - 35.433).abs() < 0.01, "{n:?}");
    assert!(crate::dialogs::symbol::accept_import(sim.cx()));
    let item = store::item(&active_item().unwrap()).unwrap();
    assert!((item.height - 35.433).abs() < 0.01);
    let model = store::model_of(&item).unwrap();
    assert_eq!(model.parts[0].color, Some([204, 26, 26]));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sketchup_files_say_how_to_export() {
    let (mut sim, dir) = setup("skp");
    let path = dir.join("house.skp");
    std::fs::write(&path, b"SketchUp Model\x00\x01").unwrap();
    crate::dialogs::symbol::open_path(sim.cx(), &path);
    assert!(
        sim.cx().status.starts_with("Export from SketchUp as COLLADA (.dae) or OBJ"),
        "{}",
        sim.cx().status
    );
    // The dialog draws with the message and refuses to import.
    sim.dialog_frame(false);
    assert!(!crate::dialogs::symbol::accept_import(sim.cx()));
    assert!(crate::dialogs::symbol::import_is_open(), "stays open");
    crate::dialogs::symbol::close_import();
    assert!(active_item().is_none());
    let _ = std::fs::remove_dir_all(dir);
}

// ----- Export Picture -----

fn decoded(bytes: &[u8]) -> plan_library::image::Rgba8Image {
    plan_library::image::decode(bytes).expect("decodes")
}

#[test]
fn export_picture_saves_the_plan_in_each_format_and_size() {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.action(crate::toolbar::Action::Custom(ep::EXPORT_PICTURE));
    assert!(ep::is_open());
    sim.dialog_frame(false);
    sim.dialog_frame(false);

    // PNG at 800 px: the plan is 4:3 and has ink.
    ep::with_options(|o| {
        o.width_px = 800;
        o.format = Format::Png;
    });
    assert!(ep::accept(sim.cx()));
    assert!(!ep::is_open());
    let (png, w, h) = ep::last_export().unwrap();
    assert_eq!(w, 800);
    assert!(h > 400 && h < 800, "{h}");
    let img = decoded(&png);
    assert_eq!((img.width, img.height), (w, h));
    assert!(img.rgba.chunks(4).any(|p| p[0] < 100), "walls are inked");
    assert_eq!(&img.rgba[..4], &[255, 255, 255, 255]);
    assert!(sim.cx().status.contains("PNG"), "{}", sim.cx().status);

    // JPEG on Letter paper at 100 DPI, portrait.
    sim.action(crate::toolbar::Action::Custom(ep::EXPORT_PICTURE));
    ep::with_options(|o| {
        o.size_by = ep::SizeBy::Paper;
        o.dpi = 100;
        o.landscape = false;
        o.format = Format::Jpeg;
    });
    assert!(ep::accept(sim.cx()));
    let (jpg, w, h) = ep::last_export().unwrap();
    assert_eq!((w, h), (850, 1100));
    let img = decoded(&jpg);
    assert_eq!((img.width, img.height), (850, 1100));

    // Transparent PNG: the corner has no alpha.
    sim.action(crate::toolbar::Action::Custom(ep::EXPORT_PICTURE));
    ep::with_options(|o| {
        o.transparent = true;
        o.width_px = 400;
    });
    assert!(ep::accept(sim.cx()));
    let (png, _, _) = ep::last_export().unwrap();
    assert_eq!(decoded(&png).rgba[3], 0);

    // BMP and TIFF at the same size.
    for (f, magic) in [(Format::Bmp, &b"BM"[..]), (Format::Tiff, &b"II"[..])] {
        sim.action(crate::toolbar::Action::Custom(ep::EXPORT_PICTURE));
        ep::with_options(|o| {
            o.format = f;
            o.width_px = 300;
        });
        assert!(ep::accept(sim.cx()));
        let (b, _, _) = ep::last_export().unwrap();
        assert_eq!(&b[..2], magic);
    }
}

#[test]
fn export_picture_covers_elevations_and_refuses_an_empty_view() {
    let mut sim = Sim::new();
    // Nothing drawn: the plan has nothing to export.
    sim.action(crate::toolbar::Action::Custom(ep::EXPORT_PICTURE));
    assert!(!ep::accept(sim.cx()));
    assert!(sim.cx().status.contains("nothing to draw"), "{}", sim.cx().status);
    assert!(ep::is_open(), "the window stays for another try");
    ep::close();

    draw_shell(&mut sim, 480.0, 360.0);
    sim.action(crate::toolbar::Action::Custom(ep::EXPORT_PICTURE));
    ep::with_options(|o| {
        o.subject = Subject::Elevation(plan_elevation::ViewDir::Front);
        o.width_px = 600;
    });
    assert!(ep::accept(sim.cx()));
    let (png, w, _) = ep::last_export().unwrap();
    assert_eq!(w, 600);
    assert!(decoded(&png).rgba.chunks(4).any(|p| p[0] < 100));

    // 3D with no model open in the viewport: not offered, so a request for it
    // is an error rather than a crash.
    sim.action(crate::toolbar::Action::Custom(ep::EXPORT_PICTURE));
    ep::with_options(|o| o.subject = Subject::ThreeD);
    assert!(!ep::accept(sim.cx()));
    ep::close();
}
