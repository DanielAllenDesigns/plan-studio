//! Scenario 37: materials, round 14. The Material Painter's modes and scope
//! driven by clicks in the real 3D view, the Material Specification dialog
//! (class, pattern, texture, list data) and its live preview, Materials
//! Defaults, the library browser's save to My Materials, the pattern hatch
//! and the by-surface Materials List.

use super::{draw_shell, Sim};
use crate::editor::{EditorContext, ObjectRef};
use crate::shell::view3d_panel::{build_view_scene, show, Outbox, View3dState, ViewScope};
use crate::tools::materials::{self, browser, defaults, spec, surfaces, PainterMode};
use crate::tools::ToolId;
use eframe::egui;
use plan_3d::Scene;
use plan_core::geometry::Point;
use plan_core::object_materials::WHOLE_OBJECT;
use plan_core::Id;
use plan_materials::{
    core_library, MaterialClass, MaterialDef, MaterialLibrary, PaintMode, PaintScope, PriceUnit,
    Region,
};
use plan_view3d::CameraMode;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim
}

fn frame(
    ctx: &egui::Context,
    sim: &mut Sim,
    st: &mut View3dState,
    events: Vec<egui::Event>,
    time: f64,
) {
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        events,
        time: Some(time),
        ..egui::RawInput::default()
    };
    let cx = &mut sim.app.cx;
    let _ = ctx.run(raw, |ctx| {
        if st.frame(ctx, cx) {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| show(ui, cx, st));
        }
    });
}

fn view(sim: &mut Sim) -> (View3dState, egui::Context) {
    let mut st = View3dState::with_inbox(Outbox::default());
    st.active = true;
    let ctx = egui::Context::default();
    frame(&ctx, sim, &mut st, Vec::new(), 0.0);
    st.viewport
        .as_mut()
        .unwrap()
        .set_mode(CameraMode::DollHouse);
    frame(&ctx, sim, &mut st, Vec::new(), 0.1);
    (st, ctx)
}

fn pixel(st: &View3dState, p: [f32; 3]) -> egui::Pos2 {
    let cam = &st.viewport.as_ref().unwrap().camera;
    let ndc = plan_view3d::math::transform_point(&cam.view_projection(800.0 / 600.0), p);
    egui::pos2((ndc[0] + 1.0) * 400.0, (1.0 - ndc[1]) * 300.0)
}

fn press(pos: egui::Pos2, down: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: down,
        modifiers: egui::Modifiers::NONE,
    }
}

fn click(ctx: &egui::Context, sim: &mut Sim, st: &mut View3dState, at: egui::Pos2, t: f64) {
    frame(ctx, sim, st, vec![egui::Event::PointerMoved(at)], t);
    frame(ctx, sim, st, vec![press(at, true)], t + 0.01);
    frame(ctx, sim, st, vec![press(at, false)], t + 0.02);
}

/// Clicks the pixel the scene point `p` is at.
fn click_on(ctx: &egui::Context, sim: &mut Sim, st: &mut View3dState, p: [f32; 3], t: f64) {
    let at = pixel(st, p);
    click(ctx, sim, st, at, t);
}

/// The scene point at the middle of the top of mesh(es) tagged `id`.
fn top_of(sim: &Sim, id: Id) -> [f32; 3] {
    let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for m in scene.meshes.iter().filter(|m| m.object_id == Some(id)) {
        if let Some((a, b)) = m.bounds() {
            for k in 0..3 {
                lo[k] = lo[k].min(a[k]);
                hi[k] = hi[k].max(b[k]);
            }
        }
    }
    [(lo[0] + hi[0]) / 2.0, hi[1], (lo[2] + hi[2]) / 2.0]
}

fn palette(mode: PaintMode, scope: PaintScope, active: &str) {
    materials::set_painter_mode(PainterMode::Paint);
    materials::set_paint_options(mode, scope, false);
    materials::set_active(Some(active.to_string()));
}

fn scene_of(cx: &EditorContext) -> Scene {
    build_view_scene(&cx.project, &ViewScope::default())
}

fn color_of(scene: &Scene, id: Id) -> Option<[u8; 3]> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .find_map(|m| m.color)
}

#[test]
fn clicks_in_the_3d_view_paint_by_mode_scope_and_use_default_material() {
    let mut sim = house();
    let (mut st, ctx) = view(&mut sim);
    let walls: Vec<Id> = sim.app.cx.floor().walls.iter().map(|w| w.id).collect();
    let brick = "Brick – Red";
    let brick_color = core_library().find(brick).unwrap().color;

    // Object: one click on the north wall paints that wall only.
    let north = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .max_by(|a, b| a.start.y.total_cmp(&b.start.y))
        .unwrap()
        .id;
    palette(PaintMode::Object, PaintScope::AllSurfaces, brick);
    let top = top_of(&sim, north);
    click_on(&ctx, &mut sim, &mut st, top, 1.0);
    assert_eq!(
        sim.app.cx.project.object_material(north, WHOLE_OBJECT),
        Some(brick)
    );
    assert_eq!(
        walls
            .iter()
            .filter(|w| sim
                .app
                .cx
                .project
                .object_material(**w, WHOLE_OBJECT)
                .is_some())
            .count(),
        1
    );
    assert!(
        sim.app.cx.selection.is_empty(),
        "a painter click never selects"
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Paint Material"));
    assert_eq!(color_of(&scene_of(&sim.app.cx), north), Some(brick_color));

    // Room: a click on the floor of the room (a surface with no object)
    // paints the room: every wall, the door and window, and the room's
    // own floor, ceiling and wall covering.
    let floor_point = [(W / 2.0) as f32, 1.0, -(H / 2.0) as f32];
    palette(PaintMode::Room, PaintScope::AllSurfaces, "Drywall");
    click_on(&ctx, &mut sim, &mut st, floor_point, 2.0);
    for w in &walls {
        assert_eq!(
            sim.app.cx.project.object_material(*w, WHOLE_OBJECT),
            Some("Drywall"),
            "wall {w}"
        );
    }
    for o in &sim.app.cx.floor().openings {
        assert_eq!(
            sim.app.cx.project.object_material(o.id, WHOLE_OBJECT),
            Some("Drywall"),
            "opening {}",
            o.id
        );
    }
    let entry = &sim.app.cx.floor().room_names[0];
    assert_eq!(entry.floor_finish.as_deref(), Some("Drywall"));
    assert_eq!(entry.ceiling_finish.as_deref(), Some("Drywall"));
    assert_eq!(entry.misc.as_ref().unwrap().wall_covering, "Drywall");
    // One undo step puts the whole room back.
    assert_eq!(sim.app.cx.undo_label(), Some("Paint Material"));
    sim.undo();
    assert_eq!(
        sim.app.cx.project.object_material(north, WHOLE_OBJECT),
        Some(brick)
    );
    assert!(sim.app.cx.floor().room_names.is_empty());

    // Scope: Same Material re-paints only what shows the clicked material.
    palette(PaintMode::Floor, PaintScope::SameMaterial, "Stucco");
    let want = core_library().find("Stucco").is_some();
    let target = if want { "Stucco" } else { "Drywall" };
    materials::set_active(Some(target.to_string()));
    let top = top_of(&sim, north);
    click_on(&ctx, &mut sim, &mut st, top, 3.0);
    assert_eq!(
        sim.app.cx.project.object_material(north, WHOLE_OBJECT),
        Some(target)
    );
    for w in walls.iter().filter(|w| **w != north) {
        assert_eq!(
            sim.app.cx.project.object_material(*w, WHOLE_OBJECT),
            None,
            "the other walls showed another material"
        );
    }

    // Plan + Use Default Material: one click puts every default back.
    materials::set_paint_options(PaintMode::Plan, PaintScope::AllSurfaces, true);
    let top = top_of(&sim, north);
    click_on(&ctx, &mut sim, &mut st, top, 4.0);
    assert!(sim.app.cx.project.object_materials.is_empty());
    assert_eq!(sim.app.cx.undo_label(), Some("Use Default Material"));
    materials::set_paint_options(PaintMode::Object, PaintScope::AllSurfaces, false);
    materials::set_painter_mode(PainterMode::Off);
}

#[test]
fn the_eyedroppers_and_adjust_definition_work_on_clicks_in_the_3d_view() {
    let mut sim = house();
    let (mut st, ctx) = view(&mut sim);
    let north = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .max_by(|a, b| a.start.y.total_cmp(&b.start.y))
        .unwrap()
        .id;
    sim.app
        .cx
        .project
        .set_object_material(north, WHOLE_OBJECT, "Brick – Red");
    sim.app
        .cx
        .project
        .set_object_material(north, "Sill Plate", "Fir Framing");
    let at = pixel(&st, top_of(&sim, north));
    materials::set_active(None);
    materials::set_painter_mode(PainterMode::Eyedropper);
    click(&ctx, &mut sim, &mut st, at, 1.0);
    assert_eq!(materials::active_material().as_deref(), Some("Brick – Red"));
    // Object Eyedropper copies the whole set; it paints another wall at once.
    materials::set_painter_mode(PainterMode::ObjectEyedropper);
    click(&ctx, &mut sim, &mut st, at, 2.0);
    let other = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .map(|w| w.id)
        .find(|w| *w != north)
        .unwrap();
    materials::set_painter_mode(PainterMode::Paint);
    materials::set_paint_options(PaintMode::Object, PaintScope::AllSurfaces, false);
    let top = top_of(&sim, other);
    click_on(&ctx, &mut sim, &mut st, top, 3.0);
    assert_eq!(
        sim.app.cx.project.object_materials_of(other),
        sim.app.cx.project.object_materials_of(north)
    );
    // Adjust Material Definition opens the specification of the clicked
    // material; Cancel leaves the library as it was.
    materials::set_painter_mode(PainterMode::Adjust);
    click(&ctx, &mut sim, &mut st, at, 4.0);
    assert!(materials::spec_open());
    assert_eq!(materials::spec_name().as_deref(), Some("Brick – Red"));
    materials::close_spec();
    materials::set_painter_mode(PainterMode::Off);
}

#[test]
fn the_material_specification_drives_the_3d_view_and_both_renderers() {
    let mut sim = house();
    let wall = sim.app.cx.floor().walls[0].id;
    let path = std::env::temp_dir().join(format!("plan-s37-mats-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    materials::set_user_path_for_test(Some(path.clone()));
    materials::set_user_library_for_test(MaterialLibrary::default());

    // Material Builder > New: a glowing, metal-free lamp material.
    assert!(materials::run_command(&mut sim.app.cx, materials::BUILDER));
    assert!(materials::spec_open());
    let mut lamp = spec::MaterialSpec::new();
    {
        let d = lamp.draft_def_mut();
        d.name = "S37 Lamp".into();
        d.color = [231, 201, 91];
        d.set_class(MaterialClass::Emissive);
        d.manufacturer = "Lumen".into();
        d.supplier = "Bright Co".into();
        d.price = 12.0;
        d.unit = PriceUnit::SqFt;
    }
    lamp.set_pattern("Tile");
    lamp.draft_def_mut().pattern_scale = 2.0;
    materials::close_spec();
    let def = lamp.def().unwrap();
    materials::save_to_user_library(&def).unwrap();
    assert!(path.exists(), "saved to the user library file");
    sim.app
        .cx
        .project
        .set_object_material(wall, WHOLE_OBJECT, "S37 Lamp");
    let scene = scene_of(&sim.app.cx);
    let mesh = scene
        .meshes
        .iter()
        .find(|m| m.object_id == Some(wall) && m.color.is_some())
        .expect("the painted wall");
    assert_eq!(mesh.color, Some([231, 201, 91]));
    let surface = mesh.paint_surface().expect("the class reached the mesh");
    assert!(surface.emissive >= 0.3 && surface.metallic == 0.0);

    // The ray tracer glows: the same scene renders brighter than with a
    // General material of the same colour.
    let bright = mean_brightness(&scene);
    let mut matte_def = def.clone();
    matte_def.name = "S37 Lamp".into();
    matte_def.set_class(MaterialClass::General);
    matte_def.emissive = 0.0;
    materials::save_to_user_library(&matte_def).unwrap();
    let dull = mean_brightness(&scene_of(&sim.app.cx));
    assert!(bright > dull * 1.05, "emissive {bright} vs general {dull}");

    // The Interactive Material Editor: an open specification shows live.
    let mut live = matte_def.clone();
    live.color = [10, 20, 30];
    materials::open_spec_for(&live);
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        materials::show_windows(ctx, &mut sim.app.cx);
    });
    assert_eq!(color_of(&scene_of(&sim.app.cx), wall), Some([10, 20, 30]));
    materials::close_spec();
    assert_eq!(color_of(&scene_of(&sim.app.cx), wall), Some([231, 201, 91]));

    materials::set_user_path_for_test(None);
    materials::set_user_library_for_test(MaterialLibrary::default());
    let _ = std::fs::remove_file(&path);
}

/// Mean of the HDR image of `scene` seen from the south-east.
fn mean_brightness(scene: &Scene) -> f32 {
    use plan_render::{Camera, Environment, RenderSettings, Renderer, Technique};
    let (lo, hi) = scene.bounds().expect("a scene");
    let centre = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let reach = (hi[0] - lo[0]).max(hi[2] - lo[2]);
    let camera = Camera {
        eye: [
            centre[0] + reach,
            centre[1] + reach * 0.6,
            centre[2] + reach,
        ],
        target: centre,
        up: [0.0, 1.0, 0.0],
        fov_deg: 50.0,
        aperture: 0.0,
        focus_dist: 0.0,
    };
    let settings = RenderSettings {
        width: 24,
        height: 18,
        samples: 6,
        threads: 1,
        technique: Technique::PhysicallyBased,
        ..RenderSettings::default()
    };
    let img = Renderer::new(scene).render(&camera, &Environment::default(), &[], &settings);
    let mut sum = 0.0;
    for y in 0..18 {
        for x in 0..24 {
            let p = img.hdr_at(x, y);
            sum += p[0] + p[1] + p[2];
        }
    }
    sum / (24.0 * 18.0 * 3.0)
}

#[test]
fn materials_defaults_reach_the_3d_scene_and_undo_as_one_step() {
    let mut sim = house();
    let wall = sim.app.cx.floor().walls[0].id;
    assert_eq!(
        color_of(&scene_of(&sim.app.cx), wall),
        None,
        "nothing is overridden yet"
    );
    assert!(defaults::set(
        &mut sim.app.cx,
        "Wall",
        "Interior Wall Surface",
        Some("Color – Bone")
    ));
    let bone = core_library().find("Color – Bone").unwrap().color;
    let painted = scene_of(&sim.app.cx);
    assert!(
        sim.app
            .cx
            .floor()
            .walls
            .iter()
            .all(|w| color_of(&painted, w.id) == Some(bone)),
        "every wall's interior face takes the default"
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Materials Defaults"));
    // An object's own paint wins; undo takes the default away again.
    sim.app
        .cx
        .project
        .set_object_material(wall, "Interior Wall Surface", "Drywall");
    let drywall = core_library().find("Drywall").unwrap().color;
    assert_eq!(color_of(&scene_of(&sim.app.cx), wall), Some(drywall));
    sim.app.cx.project.clear_object_material(wall, None);
    sim.undo();
    assert_eq!(color_of(&scene_of(&sim.app.cx), wall), None);
    // The 3D view rebuilds when the defaults change.
    let before = crate::shell::view3d_panel::project_hash(&sim.app.cx.project);
    defaults::set(&mut sim.app.cx, "Door", "Door Panel", Some("Chrome"));
    assert_ne!(
        before,
        crate::shell::view3d_panel::project_hash(&sim.app.cx.project)
    );
}

#[test]
fn the_materials_list_by_surface_adds_areas_by_region_with_prices() {
    let mut sim = house();
    let mut tile = MaterialDef::new("S37 Tile", &["Flooring"], [90, 100, 110]);
    tile.manufacturer = "Acme Tile".into();
    tile.supplier = "Tile Depot".into();
    tile.price = 54.0;
    tile.unit = PriceUnit::SqYd;
    let mut user = MaterialLibrary::default();
    user.add(tile);
    materials::set_user_library_for_test(user);
    // Paint the floor of the room and one wall with it.
    let anchor = {
        let r = sim.app.cx.rooms_now()[0].clone();
        crate::editor::rooms_edit::room_anchor(&r)
    };
    sim.app.cx.project.floors[0].room_names.push({
        let mut n = plan_core::RoomName::new(anchor, "Den", "");
        n.floor_finish = Some("S37 Tile".into());
        n
    });
    let wall = sim.app.cx.floor().walls[0].id;
    sim.app
        .cx
        .project
        .set_object_material(wall, WHOLE_OBJECT, "S37 Tile");

    let room = surfaces::lines(&mut sim.app.cx, Region::Room(0));
    let floor = surfaces::lines(&mut sim.app.cx, Region::Floor);
    let plan = surfaces::lines(&mut sim.app.cx, Region::Plan);
    let tile_line = |l: &[plan_materials::MaterialQuantity]| {
        l.iter()
            .find(|q| q.name == "S37 Tile")
            .cloned()
            .expect("tile line")
    };
    let q = tile_line(&floor);
    let interior = sim.app.cx.rooms_now()[0].interior_area_sq_in / 144.0;
    assert!(q.area_sq_ft > interior, "the floor plus the painted wall");
    assert_eq!(
        (q.manufacturer.as_str(), q.supplier.as_str()),
        ("Acme Tile", "Tile Depot")
    );
    assert!(
        (q.quantity - q.area_sq_ft / 9.0).abs() < 1e-9,
        "priced per square yard"
    );
    assert!((q.cost - q.quantity * 54.0).abs() < 1e-6);
    // The room list has the tile floor and the wall; one floor equals the plan.
    assert!(tile_line(&room).area_sq_ft > 0.0);
    assert!((tile_line(&plan).area_sq_ft - q.area_sq_ft).abs() < 1e-9);
    let csv = plan_materials::to_csv(&floor);
    assert!(csv.starts_with(
        "Material,Category,Manufacturer,Supplier,Surfaces,Area (sq ft),Price,Unit,Quantity,Cost\n"
    ));
    assert!(csv.contains("S37 Tile,Flooring,Acme Tile,Tile Depot"));
    // The Materials List window has the tab and draws it.
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        crate::dialogs::materials::show(ctx, &mut sim.app.cx);
    });
    materials::set_user_library_for_test(MaterialLibrary::default());
}

#[test]
fn the_library_dock_filters_to_materials_and_saves_to_my_materials() {
    let path = std::env::temp_dir().join(format!("plan-s37-dock-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    materials::set_user_path_for_test(Some(path.clone()));
    materials::set_user_library_for_test(MaterialLibrary::default());
    assert!(browser::save_copy(&materials::library(), "Drywall").unwrap());
    assert!(materials::library().find("Drywall").is_some());
    let on_disk = materials::load_user_library_at(&path);
    assert_eq!(on_disk.materials.len(), 1);
    assert!(browser::is_mine(&on_disk, "Drywall") && !browser::is_mine(&on_disk, "Glass"));
    assert!(!browser::save_copy(&materials::library(), "No Such Material").unwrap());
    // The dock draws the materials list when the switch is on.
    let mut sim = Sim::new();
    let mut dock = crate::shell::docks::DockState::default();
    materials::set_browse_materials(true);
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            crate::shell::docks::show(
                ui,
                crate::toolbar::Dock::Library,
                &mut sim.app.cx,
                &mut dock,
            );
        });
    });
    materials::set_browse_materials(false);
    materials::set_user_path_for_test(None);
    materials::set_user_library_for_test(MaterialLibrary::default());
    let _ = std::fs::remove_file(&path);
    let _ = ObjectRef::Terrain;
    let _ = Point::ZERO;
}

#[test]
fn the_pattern_tab_scale_and_angle_change_the_plan_hatch() {
    let sim = house();
    let project = &sim.app.cx.project;
    let poly = [
        Point::new(0.0, 0.0),
        Point::new(144.0, 0.0),
        Point::new(144.0, 72.0),
        Point::new(0.0, 72.0),
    ];
    let plain = materials::hatch_strokes(project, "Brick – Red", &poly, 0.25);
    assert!(
        plain.len() > 10,
        "the library's brick pattern hatches the region"
    );
    // A user copy with a bigger, turned pattern replaces it in the hatch.
    let mut big = core_library().find("Brick – Red").unwrap().clone();
    big.pattern_scale = 2.0;
    big.pattern_angle = 30.0;
    let mut user = MaterialLibrary::default();
    user.add(big);
    materials::set_user_library_for_test(user);
    let turned = materials::hatch_strokes(project, "Brick – Red", &poly, 0.25);
    assert!(!turned.is_empty() && turned != plain);
    assert!(turned.iter().all(|(a, b)| [a, b]
        .iter()
        .all(|p| p.x >= -1e-6 && p.x <= 144.0 + 1e-6 && p.y >= -1e-6 && p.y <= 72.0 + 1e-6)));
    // Unknown materials and materials with no pattern draw nothing.
    assert!(materials::hatch_strokes(project, "No such thing", &poly, 0.25).is_empty());
    materials::set_user_library_for_test(MaterialLibrary::default());
}
