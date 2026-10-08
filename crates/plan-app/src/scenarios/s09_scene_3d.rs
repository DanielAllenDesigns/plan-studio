//! Scenario 9: the 3D scene of the finished house: walls, openings (door
//! panel, window glass), platforms, slab, roof, and the project hash that
//! decides when the 3D view is rebuilt.

use super::{draw_shell, Sim};
use crate::editor::stairs_view::StairKind;
use crate::shell::view3d_panel::{build_view_scene, project_hash, ViewScope};
use crate::toolbar::Action;
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use plan_3d::{Material, Scene};
use plan_cabinets::CabinetKind;

const W: f64 = 480.0;
const H: f64 = 360.0;

/// Shell with a door, a window, a slab and a roof.
fn finished_house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::FoundationVariant(
        crate::tools::foundation::FoundationVariant::Slab,
    ));
    sim.drag((-10.0, -10.0), (W + 12.0, H + 12.0));
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    sim.app.cx.selection.clear();
    sim
}

fn scene(sim: &Sim) -> Scene {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
}

fn count(scene: &Scene, m: Material) -> usize {
    scene.meshes.iter().filter(|x| x.material == m).count()
}

fn hash(sim: &Sim) -> u64 {
    project_hash(&sim.app.cx.project)
}

#[test]
fn the_scene_has_walls_openings_platforms_slab_and_roof() {
    let sim = finished_house();
    let s = scene(&sim);
    let f = sim.app.cx.floor();
    // Every wall made at least one mesh tagged with its id.
    for w in &f.walls {
        assert!(
            s.meshes.iter().any(|m| m.object_id == Some(w.id)),
            "wall {} has no mesh",
            w.id
        );
    }
    // Openings: the window has glass, the door a panel.
    let window = f
        .openings
        .iter()
        .find(|o| o.kind == plan_core::OpeningKind::Window)
        .unwrap();
    let door = f
        .openings
        .iter()
        .find(|o| o.kind == plan_core::OpeningKind::Door)
        .unwrap();
    assert!(
        s.meshes
            .iter()
            .any(|m| m.material == Material::WindowGlass && m.object_id == Some(window.id)),
        "window glass"
    );
    assert!(
        s.meshes
            .iter()
            .any(|m| m.material == Material::DoorPanel && m.object_id == Some(door.id)),
        "door panel"
    );
    // Platforms, slab and roof.
    assert!(count(&s, Material::Floor) >= 1, "floor platform");
    assert!(count(&s, Material::Ceiling) >= 1, "ceiling platform");
    assert!(count(&s, Material::Concrete) >= 1, "foundation slab");
    assert!(
        count(&s, Material::Roof) >= 4,
        "roof planes: {}",
        count(&s, Material::Roof)
    );
    assert!(s.triangle_count() > 200, "{} triangles", s.triangle_count());
    // The overall size matches the house (the roof overhangs a little).
    let (lo, hi) = s.bounds().expect("bounds");
    let width = (hi[0] - lo[0]) as f64;
    let depth = (hi[2] - lo[2]) as f64;
    assert!(width > W && width < W + 120.0, "width {width}");
    assert!(depth > H && depth < H + 120.0, "depth {depth}");
    // Roof ridge is above the wall tops.
    let wall_h = f.walls[0].height as f32;
    assert!(hi[1] > wall_h + 40.0, "top {} vs wall {wall_h}", hi[1]);
}

#[test]
fn the_window_glass_follows_the_window_dialog() {
    let mut sim = finished_house();
    let before = scene(&sim);
    let window = sim
        .app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.kind == plan_core::OpeningKind::Window)
        .unwrap()
        .clone();
    let glass_width = |s: &Scene| {
        s.meshes
            .iter()
            .filter(|m| m.material == Material::WindowGlass && m.object_id == Some(window.id))
            .filter_map(|m| m.bounds())
            .map(|(lo, hi)| (hi[0] - lo[0]) as f64)
            .fold(0.0_f64, f64::max)
    };
    let narrow = glass_width(&before);
    // Widen the window through the opening dialog.
    sim.tool(ToolId::Select);
    sim.double_click(300.0, 0.0);
    if let Some(crate::ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() {
        d.draft_mut().width = 60.0;
    } else {
        panic!("no window dialog");
    }
    sim.ok();
    let wide = glass_width(&scene(&sim));
    assert!(wide > narrow + 10.0, "glass {narrow} -> {wide}");
    assert_ne!(
        hash(&sim),
        project_hash(&{
            let mut p = sim.app.cx.project.clone();
            p.floors[0]
                .openings
                .iter_mut()
                .for_each(|o| o.width = window.width);
            p
        })
    );
}

#[test]
fn the_project_hash_changes_with_every_edit_that_shapes_the_3d_model_and_undo_restores_it() {
    let mut sim = Sim::new();
    let mut seen = vec![hash(&sim)];
    let step = |sim: &mut Sim, what: &str, seen: &mut Vec<u64>| {
        let h = hash(sim);
        assert_ne!(Some(&h), seen.last(), "{what} did not change the hash");
        seen.push(h);
    };
    draw_shell(&mut sim, W, H);
    step(&mut sim, "shell", &mut seen);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    step(&mut sim, "door", &mut seen);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    step(&mut sim, "window", &mut seen);
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    step(&mut sim, "Build Roof", &mut seen);
    // Undo walks the same hashes back.
    sim.undo(); // roof
    assert_eq!(hash(&sim), seen[seen.len() - 2], "undo roof");
    sim.undo(); // window
    assert_eq!(hash(&sim), seen[seen.len() - 3], "undo window");
    sim.redo();
    sim.redo();
    assert_eq!(hash(&sim), *seen.last().unwrap(), "redo");
    // A second floor changes the hash too.
    sim.action(Action::BuildNewFloor);
    sim.ok();
    step(&mut sim, "Build New Floor", &mut seen);
    // Selecting and zooming do not.
    let h = hash(&sim);
    sim.app.cx.selection.clear();
    sim.action(Action::ZoomIn);
    assert_eq!(hash(&sim), h);
}

#[test]
fn the_floor_overview_scope_leaves_out_the_floors_above() {
    let mut sim = finished_house();
    sim.action(Action::BuildNewFloor);
    sim.ok();
    let all = scene(&sim);
    let first_only = build_view_scene(
        &sim.app.cx.project,
        &ViewScope {
            floor: Some(0),
            ..ViewScope::default()
        },
    );
    assert!(first_only.triangle_count() < all.triangle_count());
    let top = |s: &Scene| s.bounds().unwrap().1[1];
    assert!(top(&first_only) < top(&all));
}

/// The scene should show placed cabinets (parity 3d-views-cameras "3D scene
/// contents"): a cabinet on the south wall adds meshes and changes the hash.
#[test]
fn placed_cabinets_appear_in_the_3d_scene_and_change_the_hash() {
    let mut sim = finished_house();
    let tris = scene(&sim).triangle_count();
    let h = hash(&sim);
    sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
    sim.click(200.0, 10.0);
    assert_eq!(
        crate::editor::placed::load_cabinets(sim.app.cx.floor()).len(),
        1
    );
    assert!(
        scene(&sim).triangle_count() > tris,
        "cabinet adds no triangles ({tris} before and after)"
    );
    assert_ne!(
        hash(&sim),
        h,
        "the 3D view would not rebuild after placing a cabinet"
    );
}

/// Same for stairs.
#[test]
fn stairs_appear_in_the_3d_scene_and_change_the_hash() {
    let mut sim = finished_house();
    let tris = scene(&sim).triangle_count();
    let h = hash(&sim);
    sim.tool(ToolId::StairsVariant(StairKind::Draw));
    sim.drag((100.0, 100.0), (250.0, 100.0));
    assert!(
        scene(&sim).triangle_count() > tris,
        "stairs add no triangles ({tris} before and after)"
    );
    assert_ne!(
        hash(&sim),
        h,
        "the 3D view would not rebuild after drawing a stair"
    );
}
