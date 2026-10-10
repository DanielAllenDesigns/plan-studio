//! Scenario 53: material packages (Lightbeans). A synthetic store-only zip
//! with tiny PNG maps and a `productmetadata.txt` is imported through File >
//! Import > Material Package, dropped on the window, painted on a wall and
//! undone; the painted wall carries the package's maps to the 3D view and the
//! ray tracer, and the PBR maps preference switches them off.

use super::{draw_shell, Sim};
use crate::editor::ObjectRef;
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::toolbar::Action;
use crate::tools::materials::package::tests::{synthetic_zip, Sandbox};
use crate::tools::materials::package::{self, IMPORT_PACKAGE};
use crate::tools::materials::{self, spec, PainterMode};
use eframe::egui;
use plan_core::object_materials::WHOLE_OBJECT;
use plan_materials::pbr::{self, PbrSet, PBR_AO, PBR_NORMAL, PBR_ROUGH};
use plan_materials::MapKind;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.tool(crate::tools::ToolId::Select);
    sim.app.cx.selection.clear();
    sim
}

fn paint_wall(sim: &mut Sim, wall: plan_core::Id, name: &str) {
    materials::set_painter_mode(PainterMode::Paint);
    materials::set_paint_options(
        plan_materials::PaintMode::Object,
        plan_materials::PaintScope::AllSurfaces,
        false,
    );
    materials::set_active(Some(name.to_string()));
    let floor = sim.app.cx.floor;
    assert!(materials::paint_object(
        &mut sim.app.cx,
        floor,
        ObjectRef::Wall(wall)
    ));
    materials::set_painter_mode(PainterMode::Off);
}

#[test]
fn an_imported_package_paints_a_wall_with_its_maps_and_undoes() {
    let sb = Sandbox::new("s53");
    let zip = sb.write_zip("limestone.zip", "Limestone Ashlar");
    let mut sim = house();

    // File > Import > Material Package... (the file box is preset).
    package::set_next_pick_for_test(Some(vec![zip.clone()]));
    sim.action(Action::Custom(IMPORT_PACKAGE));
    assert!(
        sim.app.cx.status.contains("Imported Limestone Ashlar"),
        "{}",
        sim.app.cx.status
    );
    let def = materials::library()
        .find("Limestone Ashlar")
        .cloned()
        .unwrap();
    assert_eq!(def.class, plan_materials::MaterialClass::General);
    assert_eq!(def.manufacturer, "Acme Stone");
    assert!(def.normal_map.is_some() && def.roughness_map.is_some() && def.ao_map.is_some());
    // The tile size comes from the metadata: 600 x 300 mm.
    assert!((def.texture_scale_in.0 - 23.622).abs() < 0.01);
    assert!((def.texture_scale_in.1 - 11.811).abs() < 0.01);
    // The plan stays valid after the download is deleted.
    std::fs::remove_file(&zip).unwrap();
    assert!(std::path::Path::new(def.texture_path.as_deref().unwrap()).is_file());

    // Paint a wall with it: one undo step.
    let wall = sim.app.cx.floor().walls[0].id;
    paint_wall(&mut sim, wall, "Limestone Ashlar");
    assert_eq!(
        sim.app.cx.project.object_material(wall, WHOLE_OBJECT),
        Some("Limestone Ashlar")
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Paint Material"));

    // The 3D scene carries the bitmap and the maps for that wall.
    let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let mesh = scene
        .meshes
        .iter()
        .find(|m| m.object_id == Some(wall) && m.color.is_some())
        .expect("the painted wall has a colour");
    assert_eq!(mesh.color, Some(def.color));
    let store = plan_materials::textures::TextureStore::new();
    let list = materials::painted_textures(&sim.app.cx.project, &scene, &store);
    let tex = list
        .iter()
        .find(|t| t.object_id == wall)
        .expect("a bitmap for the wall");
    let set = tex.pbr.as_ref().expect("the package's maps");
    assert_eq!(
        set.flags & (PBR_NORMAL | PBR_ROUGH | PBR_AO),
        PBR_NORMAL | PBR_ROUGH | PBR_AO
    );
    assert!(set.normal.is_some() && set.orm.is_some());
    // The ray tracer finds the same maps for the mesh.
    let src =
        pbr::lookup(Some(wall), mesh.material, mesh.color).expect("registered for the renderers");
    assert!(PbrSet::load(&src, 64, true).albedo.is_some());

    // Undo takes the paint away again.
    sim.undo();
    assert!(sim
        .app
        .cx
        .project
        .object_material(wall, WHOLE_OBJECT)
        .is_none());
    let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    assert!(materials::painted_textures(&sim.app.cx.project, &scene, &store).is_empty());
    sim.redo();
    assert_eq!(
        sim.app.cx.project.object_material(wall, WHOLE_OBJECT),
        Some("Limestone Ashlar")
    );
}

#[test]
fn the_pbr_maps_preference_and_the_texture_tab_switches_turn_maps_off() {
    let _sb = Sandbox::new("s53b");
    let mut sim = house();
    let zip = _sb.write_zip("slate.zip", "Slate Roofing");
    let mut cx = crate::editor::EditorContext::new(crate::plan_defaults::embedded());
    package::import_zips(&mut cx, &[zip]);
    let wall = sim.app.cx.floor().walls[0].id;
    paint_wall(&mut sim, wall, "Slate Roofing");
    let store = plan_materials::textures::TextureStore::new();
    let maps = |sim: &Sim| {
        let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
        let list = materials::painted_textures(&sim.app.cx.project, &scene, &store);
        list.into_iter()
            .find(|t| t.object_id == wall)
            .map(|t| t.pbr.is_some())
    };
    assert_eq!(maps(&sim), Some(true));

    // Preferences > Render > PBR maps off: the albedo only.
    crate::dialogs::preferences::pages::update(|p| p.render.pbr_maps = false);
    assert_eq!(maps(&sim), Some(false));
    let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let mesh = scene
        .meshes
        .iter()
        .find(|m| m.object_id == Some(wall) && m.color.is_some())
        .unwrap();
    assert!(pbr::lookup(Some(wall), mesh.material, mesh.color).is_none());
    crate::dialogs::preferences::pages::update(|p| p.render.pbr_maps = true);
    assert_eq!(maps(&sim), Some(true));

    // The Material Specification's Texture tab: per-map switches.
    let def = materials::library().find("Slate Roofing").cloned().unwrap();
    let mut draft = spec::MaterialSpec::from_def(&def);
    for k in [MapKind::Normal, MapKind::Roughness, MapKind::Ao] {
        draft.draft_def_mut().set_map_enabled(k, false);
    }
    let edited = draft.def().unwrap();
    assert!(!edited.map_enabled(MapKind::Normal) && !edited.map_enabled(MapKind::Ao));
    assert!(
        pbr::PbrSource::from_def(&edited).is_none(),
        "every map is off"
    );
    // The tab draws on the package.
    let ctx = egui::Context::default();
    let mut draft = spec::MaterialSpec::from_def(&def);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        draft.show(ctx);
    });
}

#[test]
fn a_zip_dropped_on_the_window_is_imported() {
    let sb = Sandbox::new("s53c");
    let mut sim = house();
    let zip = sb.write_zip("dropped.zip", "Dropped Marble");
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        dropped_files: vec![egui::DroppedFile {
            path: Some(zip),
            ..Default::default()
        }],
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| sim.app.drive_files(ctx));
    assert!(
        sim.app.cx.status.contains("Imported Dropped Marble"),
        "{}",
        sim.app.cx.status
    );
    assert!(materials::library().find("Dropped Marble").is_some());
    // The synthetic archive itself is what the tests promise.
    assert!(!synthetic_zip("x").is_empty());
}
