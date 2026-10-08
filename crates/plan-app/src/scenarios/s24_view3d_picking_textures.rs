//! Scenario 24: the 3D view as a second way to edit. Clicks that select,
//! open and delete, a terrain object picked in 3D, the Textures toggle and
//! billboards that turn to face the eye (C-43, the Textures toolbar toggle,
//! Create Billboard Image).

use super::{draw_shell, Sim};
use crate::editor::{site_view, EditorRequest, ObjectRef};
use crate::shell::view3d_panel::{
    build_view_scene, show, technique_shows_textures, Outbox, View3dState, ViewScope,
};
use crate::tools::ToolId;
use eframe::egui;
use plan_3d::images::image_mesh;
use plan_3d::{Material, Scene};
use plan_core::geometry::Point;
use plan_core::{Id, ImageSpec, PlacedSymbol, WallKind};
use plan_materials::textures::TextureStore;
use plan_materials::RenderingTechnique;
use plan_view3d::{CameraMode, Viewport3d};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    // The Doll House hides ceilings and roofs, so a click reaches the walls.
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

fn key(k: egui::Key) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: Some(k),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
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

#[test]
fn a_click_selects_a_double_click_opens_and_delete_removes_a_wall_in_3d() {
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
    let at = pixel(&st, top_of(&sim, north));
    assert!(sim.app.cx.selection.is_empty());
    click(&ctx, &mut sim, &mut st, at, 1.0);
    assert_eq!(sim.app.cx.selection.single(), Some(ObjectRef::Wall(north)));
    // The plan shows the same selection when the user goes back to it.
    assert!(sim.app.cx.selection.items.contains(&ObjectRef::Wall(north)));

    // A double click asks for the specification; the shell opens it.
    sim.app.cx.requests.clear();
    click(&ctx, &mut sim, &mut st, at, 2.0);
    click(&ctx, &mut sim, &mut st, at, 2.1);
    assert!(
        sim.app
            .cx
            .requests
            .iter()
            .any(|r| matches!(r, EditorRequest::OpenSpec(ObjectRef::Wall(i)) if *i == north)),
        "{:?}",
        sim.app.cx.requests
    );
    sim.app.process_requests();
    assert!(sim.app.has_dialog(), "the Wall Specification opens");
    sim.cancel();

    // Delete in the 3D view deletes the selection as one undo step.
    sim.app.cx.selection.set(ObjectRef::Wall(north));
    frame(&ctx, &mut sim, &mut st, vec![key(egui::Key::Delete)], 3.0);
    assert!(sim.app.cx.floor().wall(north).is_none());
    frame(&ctx, &mut sim, &mut st, Vec::new(), 3.1);
    let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    assert!(scene.meshes.iter().all(|m| m.object_id != Some(north)));
    assert_eq!(sim.undo().as_deref().map(str::is_empty), Some(false));
    assert!(sim.app.cx.floor().wall(north).is_some());
    // A click on empty sky clears the selection.
    click(&ctx, &mut sim, &mut st, egui::pos2(4.0, 4.0), 4.0);
    assert!(sim.app.cx.selection.is_empty());
}

#[test]
fn a_door_and_a_window_are_picked_by_their_own_meshes() {
    let mut sim = house();
    let (mut st, ctx) = view(&mut sim);
    let f = sim.app.cx.floor();
    let door = f
        .openings
        .iter()
        .find(|o| o.kind == plan_core::OpeningKind::Door)
        .unwrap()
        .id;
    let window = f
        .openings
        .iter()
        .find(|o| o.kind == plan_core::OpeningKind::Window)
        .unwrap()
        .id;
    // Look at the south wall from outside, level with the openings.
    st.viewport
        .as_mut()
        .unwrap()
        .set_mode(CameraMode::ElevationFront);
    frame(&ctx, &mut sim, &mut st, Vec::new(), 0.5);
    for (i, (id, want)) in [
        (door, ObjectRef::Opening(door)),
        (window, ObjectRef::Opening(window)),
    ]
    .into_iter()
    .enumerate()
    {
        let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
        let (lo, hi) = scene
            .meshes
            .iter()
            .filter(|m| {
                m.object_id == Some(id)
                    && matches!(m.material, Material::DoorPanel | Material::WindowGlass)
            })
            .filter_map(|m| m.bounds())
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), (a, b)| {
                (
                    [lo[0].min(a[0]), lo[1].min(a[1]), lo[2].min(a[2])],
                    [hi[0].max(b[0]), hi[1].max(b[1]), hi[2].max(b[2])],
                )
            });
        let mid = [
            (lo[0] + hi[0]) / 2.0,
            (lo[1] + hi[1]) / 2.0,
            (lo[2] + hi[2]) / 2.0,
        ];
        let at = pixel(&st, mid);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let hit = st.object_at(&sim.app.cx.project, 0, rect, at);
        assert_eq!(hit, Some((0, want)), "pick {i} at {at:?}");
        click(&ctx, &mut sim, &mut st, at, 1.0 + i as f64);
        assert_eq!(sim.app.cx.selection.single(), Some(want));
    }
}

#[test]
fn a_terrain_wall_is_picked_in_3d_selected_and_deleted_as_a_terrain_edit() {
    let mut sim = house();
    let mut rec = site_view::TerrainRecord::new();
    rec.terrain.walls.push(plan_terrain::TerrainWall::new(
        plan_terrain::WallKind::Wall,
        vec![Point::new(0.0, -300.0), Point::new(300.0, -300.0)],
        false,
    ));
    site_view::save_terrain(&mut sim.app.cx.project, &rec);
    let (mut st, ctx) = view(&mut sim);
    let id = site_view::hit_mesh_id(site_view::TerrainHit::Wall(0));
    let scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let mesh = scene
        .meshes
        .iter()
        .find(|m| m.object_id == Some(id))
        .expect("the terrain wall is in the scene");
    let (lo, hi) = mesh.bounds().unwrap();
    let top = pixel(&st, [(lo[0] + hi[0]) / 2.0, hi[1], (lo[2] + hi[2]) / 2.0]);
    click(&ctx, &mut sim, &mut st, top, 1.0);
    assert_eq!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::TerrainObject(site_view::TerrainHit::Wall(0)))
    );
    frame(&ctx, &mut sim, &mut st, vec![key(egui::Key::Delete)], 2.0);
    assert!(site_view::load_terrain(&sim.app.cx.project)
        .unwrap()
        .terrain
        .walls
        .is_empty());
    sim.undo();
    assert_eq!(
        site_view::load_terrain(&sim.app.cx.project)
            .unwrap()
            .terrain
            .walls
            .len(),
        1
    );
}

/// Waits (up to a few seconds) for the texture prefetch thread to finish.
fn settle_store(store: &TextureStore, want: usize) -> usize {
    let t0 = Instant::now();
    while store.cached_count() < want && t0.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    // Let a still-running prefetch finish before counting.
    std::thread::sleep(Duration::from_millis(150));
    store.cached_count()
}

#[test]
fn the_textures_toggle_decides_what_the_viewport_loads_and_paints() {
    let mut sim = house();
    let scene: Scene = build_view_scene(&sim.app.cx.project, &ViewScope::default());

    // Headless: no GL, only the texture store. With Textures on, queueing a
    // scene prefetches the bitmaps its materials need ...
    let on_store = Arc::new(TextureStore::with_dirs(Vec::new()));
    let mut on = Viewport3d::with_texture_store(Arc::clone(&on_store));
    on.textures_enabled = true;
    on.queue_scene(&scene);
    let loaded = settle_store(&on_store, 1);
    assert!(loaded > 0, "nothing was prefetched with Textures on");
    // ... with Textures off, it loads none.
    let off_store = Arc::new(TextureStore::with_dirs(Vec::new()));
    let mut off = Viewport3d::with_texture_store(Arc::clone(&off_store));
    off.textures_enabled = false;
    off.queue_scene(&scene);
    assert_eq!(
        settle_store(&off_store, 1),
        0,
        "Textures off still decoded bitmaps"
    );

    // The shell sets the viewport flag from the toggle and the technique.
    let (mut st, ctx) = view(&mut sim);
    assert!(st.textures_on);
    assert!(st.viewport.as_ref().unwrap().textures_enabled);
    st.textures_on = false;
    frame(&ctx, &mut sim, &mut st, Vec::new(), 1.0);
    assert!(!st.viewport.as_ref().unwrap().textures_enabled);
    st.textures_on = true;
    frame(&ctx, &mut sim, &mut st, Vec::new(), 1.1);
    assert!(st.viewport.as_ref().unwrap().textures_enabled);
    // Flat techniques ignore the toggle.
    for t in RenderingTechnique::ALL {
        let flat = !technique_shows_textures(t);
        st.technique = t;
        frame(&ctx, &mut sim, &mut st, Vec::new(), 2.0);
        assert_eq!(
            st.viewport.as_ref().unwrap().textures_enabled,
            !flat,
            "{t:?}"
        );
    }
    // The signature includes the Textures toggle (integration pass, parity C-55:
    // toggling Textures refreshes the view), so it changes with it.
    st.technique = RenderingTechnique::Standard;
    let sig = st.signature(&sim.app.cx.project);
    st.textures_on = false;
    assert_ne!(st.signature(&sim.app.cx.project), sig);
}

fn billboard(sim: &mut Sim, color: [u8; 3]) -> Id {
    let mut spec = ImageSpec::new("tree.png", 10, 10);
    spec.color = color;
    sim.app.cx.project.add_symbol(
        0,
        PlacedSymbol::billboard(spec, Point::new(400.0, 100.0), 40.0, 72.0),
    )
}

#[test]
fn a_billboard_faces_the_eye_from_every_side_and_stays_pickable() {
    let mut sim = house();
    // A brick-coloured picture (it maps to the Brick material).
    let id = billboard(&mut sim, [158, 77, 56]);
    let sym = sim
        .app
        .cx
        .floor()
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap()
        .clone();
    let base = sim.app.cx.floor().elevation;
    // The mesh normal points at the eye whichever side the eye is on.
    for eye_plan in [
        Point::new(600.0, 300.0),
        Point::new(200.0, 300.0),
        Point::new(200.0, -100.0),
        Point::new(700.0, -100.0),
    ] {
        let eye = [eye_plan.x as f32, 60.0, -eye_plan.y as f32];
        let m = image_mesh(&sym, base, Some(eye)).unwrap();
        let n = m.vertices[0].normal;
        assert!(n[1].abs() < 1e-4, "upright: {n:?}");
        let c = m
            .bounds()
            .map(|(lo, hi)| [(lo[0] + hi[0]) / 2.0, 0.0, (lo[2] + hi[2]) / 2.0])
            .unwrap();
        let to_eye = [eye[0] - c[0], 0.0, eye[2] - c[2]];
        let len = (to_eye[0] * to_eye[0] + to_eye[2] * to_eye[2]).sqrt();
        let dot = (n[0] * to_eye[0] + n[2] * to_eye[2]) / len;
        assert!(dot > 0.99, "billboard turned away from {eye_plan:?}: {dot}");
    }

    // In the 3D view the picture is part of the overlay, so a click on it
    // selects it, before and after the camera orbits a quarter turn.
    let (mut st, _ctx) = view(&mut sim);
    st.refresh_overlay(&sim.app.cx.project, &sim.app.cx.selection);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let eye = st.viewport.as_ref().unwrap().camera.eye();
    let mesh = image_mesh(&sym, base, Some(eye)).unwrap();
    let (lo, hi) = mesh.bounds().unwrap();
    // A hair off the quad's diagonal so the ray lands inside one triangle.
    let centre = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0 + 7.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let at = pixel(&st, centre);
    let hit = st.object_at(&sim.app.cx.project, 0, rect, at);
    assert_eq!(hit, Some((0, ObjectRef::Symbol(id))), "before orbiting");
    st.viewport.as_mut().unwrap().camera.yaw += std::f32::consts::FRAC_PI_2;
    // The picture turned with the eye: its centre is where the mesh now is.
    let eye = st.viewport.as_ref().unwrap().camera.eye();
    let mesh = image_mesh(&sym, base, Some(eye)).unwrap();
    let (lo, hi) = mesh.bounds().unwrap();
    // A hair off the quad's diagonal so the ray lands inside one triangle.
    let centre = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0 + 7.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let at = pixel(&st, centre);
    let hit = st.object_at(&sim.app.cx.project, 0, rect, at);
    assert_eq!(
        hit,
        Some((0, ObjectRef::Symbol(id))),
        "after a quarter turn"
    );
    // The model was not rebuilt for it.
    let sig = st.signature(&sim.app.cx.project);
    st.ensure_scene(&sim.app.cx.project);
    assert_eq!(st.last_project_hash, sig);
    let _ = WallKind::Exterior;
}

#[test]
fn a_green_billboard_is_not_hidden_with_the_roofs_in_the_doll_house() {
    let mut sim = house();
    // The picture's average colour is nearest the Roof material, and the
    // Doll House hides every mesh of the Roof (and Ceiling) material.
    let id = billboard(&mut sim, [100, 120, 90]);
    let sym = sim
        .app
        .cx
        .floor()
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap()
        .clone();
    let base = sim.app.cx.floor().elevation;
    let (st, _ctx) = view(&mut sim);
    let eye = st.viewport.as_ref().unwrap().camera.eye();
    let mesh = image_mesh(&sym, base, Some(eye)).unwrap();
    let (lo, hi) = mesh.bounds().unwrap();
    let at = pixel(
        &st,
        [
            (lo[0] + hi[0]) / 2.0,
            (lo[1] + hi[1]) / 2.0 + 7.0,
            (lo[2] + hi[2]) / 2.0,
        ],
    );
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    assert_eq!(
        st.object_at(&sim.app.cx.project, 0, rect, at),
        Some((0, ObjectRef::Symbol(id)))
    );
}
