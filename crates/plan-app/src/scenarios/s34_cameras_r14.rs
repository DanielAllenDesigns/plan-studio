//! Scenario 34 (round 14): 3D views and cameras. A Floor Camera placed from
//! the plan and reshaped by its wedge handles, the Camera Specification tabs
//! saved on the camera, Save and Restore Camera through the Project Browser's
//! requests, 3D > Lighting, a walkthrough made from a CAD polyline and
//! recorded as a numbered PNG sequence at a chosen rate, and Preview or Final
//! View (C-6, C-25, C-31, C-33, C-44, C-62, C-64, C-70, C-71).

use super::{draw_shell, Sim};
use crate::editor::ObjectRef;
use crate::shell::view3d_panel::{
    build_view_scene, dispatch, show, Outbox, View3dCommand, View3dState, ViewRequest, ViewScope,
};
use crate::tools::camera::{
    apply_wedge, hit_wedge, wedge_handles_of, CameraVariant as V, WedgeHandle,
};
use crate::tools::ToolId;
use eframe::egui;
use plan_core::camera_view::{BackdropKind, ViewQuality, WalkRecord};
use plan_core::geometry::Point;
use plan_core::{CameraKind, Id};
use plan_view3d::CameraMode;
use std::time::{Duration, Instant};

const W: f64 = 480.0;
const H: f64 = 360.0;

/// A two-storey shell: the walls are copied to the floor above.
fn two_storey() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.app.cx.project.build_new_floor(true);
    sim.app.cx.floor = 0;
    sim.app.cx.refresh();
    sim
}

/// One headless frame of the 3D panel; `enter` presses Enter (OK in a dialog).
fn frame(ctx: &egui::Context, sim: &mut Sim, st: &mut View3dState, enter: bool, time: f64) {
    let mut events = Vec::new();
    if enter {
        events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
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

fn frames(ctx: &egui::Context, sim: &mut Sim, st: &mut View3dState, n: usize) {
    for i in 0..n {
        frame(ctx, sim, st, false, i as f64 * 0.1);
    }
}

/// Closes the open dialog with OK the way the other scenarios do: two frames
/// to lay it out, then Enter.
fn ok(ctx: &egui::Context, sim: &mut Sim, st: &mut View3dState) {
    frames(ctx, sim, st, 2);
    frame(ctx, sim, st, true, 0.5);
    frames(ctx, sim, st, 2);
}

fn state() -> (View3dState, Outbox, egui::Context) {
    let inbox = Outbox::default();
    (
        View3dState::with_inbox(inbox.clone()),
        inbox,
        egui::Context::default(),
    )
}

fn camera_ids(sim: &Sim) -> Vec<Id> {
    sim.app.cx.project.cameras.iter().map(|c| c.id).collect()
}

#[test]
fn a_floor_camera_is_reshaped_by_its_wedge_handles_in_one_undo_step_each() {
    let mut sim = two_storey();
    sim.tool(ToolId::CameraVariant(V::FloorCamera));
    sim.drag((120.0, 100.0), (120.0, 200.0));
    let id = sim.app.cx.project.cameras[0].id;
    let c = sim.app.cx.project.camera(id).unwrap().clone();
    assert_eq!(c.kind, CameraKind::FloorCamera);
    assert_eq!(c.floor, 0);
    // Pick the camera up again with the Camera tool and drag a cone corner.
    sim.tool(ToolId::CameraVariant(V::FullCamera));
    sim.click(120.0, 100.0);
    let corner = wedge_handles_of(&c)[0].1;
    assert_eq!(hit_wedge(&c, corner, 6.0), Some(WedgeHandle::FovLeft));
    let before = sim.app.cx.project.camera(id).unwrap().fov_deg;
    sim.drag((corner.x, corner.y), (120.0 - 200.0, 100.0 + 200.0));
    let widened = sim.app.cx.project.camera(id).unwrap().clone();
    assert!(
        widened.fov_deg > before + 20.0,
        "{} -> {}",
        before,
        widened.fov_deg
    );
    assert_eq!(widened.position, c.position);
    sim.undo();
    assert_eq!(sim.app.cx.project.camera(id).unwrap().fov_deg, before);
    // The tilt diamond tilts the view; undo puts it level again.
    let diamond = wedge_handles_of(&c)
        .into_iter()
        .find(|(h, _)| *h == WedgeHandle::Tilt)
        .unwrap()
        .1;
    sim.drag((diamond.x, diamond.y), (diamond.x, diamond.y + 40.0));
    let tilted = sim.app.cx.project.camera(id).unwrap().view.tilt_deg;
    assert!(tilted > 20.0, "{tilted}");
    sim.undo();
    assert_eq!(sim.app.cx.project.camera(id).unwrap().view.tilt_deg, 0.0);
}

#[test]
fn a_locked_floor_camera_cannot_be_dragged_from_the_plan() {
    let mut sim = two_storey();
    sim.tool(ToolId::CameraVariant(V::FloorCamera));
    sim.drag((120.0, 100.0), (120.0, 200.0));
    let id = sim.app.cx.project.cameras[0].id;
    sim.app
        .cx
        .project
        .update_camera(id, |c| c.view.locked = true);
    let before = sim.app.cx.project.camera(id).unwrap().clone();
    sim.tool(ToolId::CameraVariant(V::FullCamera));
    sim.click(120.0, 100.0);
    sim.drag((120.0, 100.0), (300.0, 300.0));
    let c = sim.app.cx.project.camera(id).unwrap();
    assert_eq!(c.position, before.position);
    let mut copy = before.clone();
    apply_wedge(&mut copy, WedgeHandle::FovLeft, Point::new(900.0, 900.0));
    assert_eq!(copy, before);
}

#[test]
fn the_floor_camera_shows_one_floor_clipped_at_its_ceiling_in_the_3d_view() {
    let mut sim = two_storey();
    sim.tool(ToolId::CameraVariant(V::FloorCamera));
    sim.drag((120.0, 100.0), (120.0, 200.0));
    let id = sim.app.cx.project.cameras[0].id;
    let (mut st, _inbox, ctx) = state();
    st.show_camera(&sim.app.cx.project, id);
    frames(&ctx, &mut sim, &mut st, 2);
    assert!(st.active);
    assert_eq!(st.mode, CameraMode::FullCamera);
    assert_eq!(st.scope_floor, Some(0));
    let f = &sim.app.cx.project.floors[0];
    let ceiling = f.elevation + f.ceiling_height;
    let clip = st.clip_above.expect("clipped at the ceiling");
    assert!(clip > ceiling && clip < ceiling + 2.0);
    let all = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let seen = build_view_scene(
        &sim.app.cx.project,
        &ViewScope {
            floor: st.scope_floor,
            clip_above: st.clip_above,
            ..ViewScope::default()
        },
    );
    assert!(seen.triangle_count() < all.triangle_count());
    // Switching to a plain Full Camera view shows everything again.
    st.open_mode(CameraMode::Orbit, None);
    assert_eq!((st.scope_floor, st.clip_above), (None, None));
}

#[test]
fn the_camera_specification_saves_its_tabs_on_the_camera() {
    let mut sim = two_storey();
    sim.tool(ToolId::CameraVariant(V::FullCamera));
    sim.drag((120.0, 100.0), (120.0, 200.0));
    let id = sim.app.cx.project.cameras[0].id;
    let (mut st, inbox, ctx) = state();
    inbox.post(ViewRequest::OpenCameraSpec(id));
    frames(&ctx, &mut sim, &mut st, 2);
    {
        let d = st
            .camera_dialog_mut()
            .expect("the Camera Specification is open");
        let cam = d.draft_mut();
        cam.view.tilt_deg = 12.0;
        cam.view.technique = Some("Clay".into());
        cam.view.quality = ViewQuality::Preview;
        cam.view.backdrop.kind = BackdropKind::Color;
        cam.view.backdrop.color = [10, 20, 200];
        cam.view.label.text = "Entry view".into();
        cam.view.label.show_in_plan = true;
        cam.view.label.show_in_view = true;
        cam.view.locked = true;
        cam.view.ambient = Some(0.8);
    }
    ok(&ctx, &mut sim, &mut st);
    assert!(st.camera_dialog_mut().is_none(), "OK closed the dialog");
    let c = sim.app.cx.project.camera(id).unwrap().clone();
    assert_eq!(c.view.tilt_deg, 12.0);
    assert_eq!(c.view.technique.as_deref(), Some("Clay"));
    assert_eq!(c.view.label.text, "Entry view");
    assert!(c.view.locked);
    // It is one undo step, and the saved plan carries it.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.camera(id).unwrap().view, c.view);
    sim.undo();
    assert_eq!(sim.app.cx.project.camera(id).unwrap().view.tilt_deg, 0.0);
    sim.redo();
    // Showing the camera brings its technique, tilt, quality and backdrop.
    st.show_camera(&sim.app.cx.project, id);
    frames(&ctx, &mut sim, &mut st, 2);
    assert_eq!(st.technique, plan_materials::RenderingTechnique::Clay);
    let vp = st.viewport.as_ref().unwrap();
    assert!((vp.camera.tilt_deg() - 12.0).abs() < 0.01);
    assert!(!vp.settings.shadows, "Preview");
    assert_eq!(vp.background[2], 200.0 / 255.0);
    assert!((vp.lighting.ambient - 0.8).abs() < 1e-6);
    // A locked camera also refuses the Camera Specification's neighbours.
    let before = sim.app.cx.project.camera(id).unwrap().clone();
    sim.tool(ToolId::CameraVariant(V::FullCamera));
    sim.click(120.0, 100.0);
    sim.drag((120.0, 100.0), (200.0, 260.0));
    assert_eq!(
        sim.app.cx.project.camera(id).unwrap().position,
        before.position
    );
}

#[test]
fn save_camera_then_restore_it_from_the_project_browser() {
    let mut sim = two_storey();
    let (mut st, inbox, ctx) = state();
    st.open_mode(CameraMode::DollHouse, None);
    frames(&ctx, &mut sim, &mut st, 2);
    st.viewport.as_mut().unwrap().camera.orbit(0.9, 0.05);
    let pose = st.viewport.as_ref().unwrap().camera.pose();
    // The Project Browser's Save Camera button posts this request.
    inbox.post(ViewRequest::SaveCamera);
    frames(&ctx, &mut sim, &mut st, 1);
    let ids = camera_ids(&sim);
    assert_eq!(ids.len(), 1);
    let saved = sim.app.cx.project.camera(ids[0]).unwrap().clone();
    assert_eq!(saved.kind, CameraKind::DollHouse);
    assert!(saved.view.pose.is_some() && !saved.view.show_in_plan);
    // It is listed in the browser, renamed and deleted like any camera.
    assert!(crate::shell::docks::rename_camera(
        &mut sim.app.cx,
        saved.id,
        "Dollhouse from the SW"
    ));
    // Another view, then Restore (double-click in the browser).
    st.open_mode(CameraMode::Orbit, None);
    frames(&ctx, &mut sim, &mut st, 2);
    inbox.post(ViewRequest::ShowCamera(saved.id));
    frames(&ctx, &mut sim, &mut st, 2);
    assert_eq!(st.active_camera, Some(saved.id));
    let (e, t) = st.viewport.as_ref().unwrap().camera.pose();
    for i in 0..3 {
        assert!((e[i] - pose.0[i]).abs() < 0.1, "eye {i}");
        assert!((t[i] - pose.1[i]).abs() < 0.1, "target {i}");
    }
    // Delete is one undo step.
    assert!(crate::shell::docks::delete_camera(
        &mut sim.app.cx,
        saved.id
    ));
    assert!(sim.app.cx.project.cameras.is_empty());
    sim.undo();
    assert_eq!(sim.app.cx.project.cameras.len(), 1);
}

#[test]
fn the_lighting_dialog_sets_the_sun_and_switches_the_interior_lights() {
    let mut sim = two_storey();
    let light = sim
        .app
        .cx
        .project
        .add_light(
            0,
            plan_core::camera::PlanLight::new(Point::new(100.0, 100.0), 84.0),
        )
        .unwrap();
    let (mut st, _inbox, ctx) = state();
    dispatch(
        View3dCommand::Mode(CameraMode::Orbit),
        &mut sim.app.cx,
        &mut sim.app.tools,
        &mut st,
    );
    dispatch(
        View3dCommand::Lighting,
        &mut sim.app.cx,
        &mut sim.app.tools,
        &mut st,
    );
    frames(&ctx, &mut sim, &mut st, 2);
    {
        let d = st
            .lighting_dialog_mut()
            .expect("the Lighting dialog is open");
        d.draft_mut().sun_azimuth_deg = 90.0;
        d.draft_mut().sun_altitude_deg = 15.0;
        d.draft_mut().interior_lights = false;
        d.lights_mut()[0].intensity = 2.0;
    }
    // Nothing reaches the model until OK.
    assert!(sim.app.cx.project.lighting.interior_lights);
    ok(&ctx, &mut sim, &mut st);
    assert!(st.lighting_dialog_mut().is_none(), "OK closed the dialog");
    assert_eq!(sim.app.cx.project.lighting.sun_azimuth_deg, 90.0);
    assert!(!sim.app.cx.project.lighting.interior_lights);
    assert_eq!(sim.app.cx.project.light(light).unwrap().intensity, 2.0);
    frames(&ctx, &mut sim, &mut st, 2);
    let rig = st.viewport.as_ref().unwrap().lighting;
    assert!(
        rig.key_dir[0] > 0.9,
        "the sun moved east: {:?}",
        rig.key_dir
    );
    assert!(
        crate::dialogs::camera::render_lights(&sim.app.cx.project).is_empty(),
        "the interior lights are off"
    );
    assert!(sim.app.cx.project.light(light).unwrap().enabled);
    // The whole dialog is one undo step.
    sim.undo();
    assert_eq!(
        crate::dialogs::camera::render_lights(&sim.app.cx.project).len(),
        1
    );
    assert_eq!(sim.app.cx.project.lighting.sun_azimuth_deg, 225.0);
    assert_eq!(sim.app.cx.project.light(light).unwrap().intensity, 1.0);
}

#[test]
fn a_walkthrough_from_a_cad_polyline_plays_scrubs_and_records_at_the_chosen_rate() {
    let mut sim = two_storey();
    let pts = vec![
        Point::new(60.0, 60.0),
        Point::new(300.0, 60.0),
        Point::new(300.0, 200.0),
    ];
    let cad = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        plan_core::CadItem::Polyline {
            points: pts.clone(),
            closed: false,
        },
    );
    sim.app.cx.selection.set(ObjectRef::Cad(cad));
    let (mut st, _inbox, ctx) = state();
    dispatch(
        View3dCommand::WalkFromCad,
        &mut sim.app.cx,
        &mut sim.app.tools,
        &mut st,
    );
    let id = sim.app.cx.project.cameras[0].id;
    assert_eq!(sim.app.cx.project.cameras[0].path, pts);
    // Key frames: a three second hold at the corner, looking up a little.
    sim.app.cx.project.update_camera(id, |c| {
        c.walk_speed = 240.0;
        c.path_nodes[1].hold_s = 1.0;
        c.path_nodes[1].tilt_deg = 10.0;
        c.view.walk = WalkRecord {
            fps: 2.0,
            width: 48,
            height: 32,
            samples: 1,
        };
    });
    let duration = sim.app.cx.project.camera(id).unwrap().walk_duration_s();
    assert!((duration - (380.0 / 240.0 + 1.0)).abs() < 1e-9);
    // Scrub to the corner: the camera stands at the key frame and has tilted.
    frames(&ctx, &mut sim, &mut st, 2);
    st.show_camera(&sim.app.cx.project, id);
    frames(&ctx, &mut sim, &mut st, 2);
    st.walk.as_mut().unwrap().t_s = 240.0 / 240.0 + 0.5;
    let held = crate::shell::view3d_panel::walk_view(
        &sim.app.cx.project,
        sim.app.cx.project.camera(id).unwrap(),
        1.5,
    );
    assert!((held.0[0] - 300.0).abs() < 1e-3);
    // Record at 2 fps into a folder of our own.
    let dir = std::env::temp_dir().join(format!("plan_s34_walk_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    st.start_recording(&mut sim.app.cx, id, dir.clone());
    let started = Instant::now();
    while !sim.app.cx.status.starts_with("Recorded") {
        frames(&ctx, &mut sim, &mut st, 1);
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "recording hung"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let frames_written = (duration * 2.0).round() as usize;
    assert!(sim
        .app
        .cx
        .status
        .contains(&format!("{frames_written} frames")));
    let pngs = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".png"))
        .count();
    assert_eq!(pngs, frames_written);
    assert!(dir.join("frame_0001.png").exists());
    let _ = std::fs::remove_dir_all(&dir);
    // The walkthrough path is one undo step away.
    assert_eq!(sim.app.cx.project.cameras.len(), 1);
}

#[test]
fn preview_and_final_view_rebuild_and_the_picture_backdrop() {
    let mut sim = two_storey();
    let (mut st, _inbox, ctx) = state();
    dispatch(
        View3dCommand::Mode(CameraMode::Orbit),
        &mut sim.app.cx,
        &mut sim.app.tools,
        &mut st,
    );
    frames(&ctx, &mut sim, &mut st, 2);
    assert!(
        st.viewport.as_ref().unwrap().settings.shadows,
        "Final View is the default"
    );
    dispatch(
        View3dCommand::Quality(ViewQuality::Preview),
        &mut sim.app.cx,
        &mut sim.app.tools,
        &mut st,
    );
    frames(&ctx, &mut sim, &mut st, 1);
    let s = st.viewport.as_ref().unwrap().settings;
    assert!(!s.shadows && !s.ambient_occlusion);
    dispatch(
        View3dCommand::Quality(ViewQuality::Final),
        &mut sim.app.cx,
        &mut sim.app.tools,
        &mut st,
    );
    frames(&ctx, &mut sim, &mut st, 1);
    assert!(st.viewport.as_ref().unwrap().settings.shadows);
    // Rebuild 3D throws the scene away and builds it again.
    dispatch(
        View3dCommand::Rebuild,
        &mut sim.app.cx,
        &mut sim.app.tools,
        &mut st,
    );
    assert!(st.scene_dirty);
    frames(&ctx, &mut sim, &mut st, 1);
    assert!(!st.scene_dirty);
    // A backdrop picture read from a file on disk (Chief's Backdrops folder
    // at run time; here a temporary one).
    let dir = std::env::temp_dir().join(format!("plan_s34_backdrop_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let png = plan_render::encode_png(&plan_render::Image {
        width: 6,
        height: 3,
        rgba: (0..18).flat_map(|_| [200, 120, 40, 255]).collect(),
        hdr: Vec::new(),
    });
    std::fs::write(dir.join("dusk.png"), png).unwrap();
    let mut cam =
        plan_core::CameraObject::new(CameraKind::PerspectiveOverview, Point::ZERO, 0.0, "Dusk", 0);
    cam.view.backdrop.kind = BackdropKind::Image;
    cam.view.backdrop.image = dir.join("dusk.png").display().to_string();
    let id = sim.app.cx.project.add_camera(cam);
    st.show_camera(&sim.app.cx.project, id);
    frames(&ctx, &mut sim, &mut st, 2);
    let img = st.viewport.as_ref().unwrap().backdrop.clone();
    assert_eq!(img.map(|i| (i.width, i.height)), Some((6, 3)));
    let _ = std::fs::remove_dir_all(&dir);
}
