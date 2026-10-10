//! Scenario 50 (round 15): 3D views and cameras, the second half. Light sets
//! chosen in Adjust Lights, in 3D > Lighting and per camera; a walkthrough
//! recorded as a Motion-JPEG movie; Export 360 Panorama; Undo Zoom in 3D; an
//! orthographic view saved as a camera; Floors Displayed picked per floor;
//! ground and fog from the Backdrop tab; and the path-traced Final View with
//! Stop (C-31, C-41, C-42, C-44, C-53, C-62, C-65, C-70, C-71, C-77).

use super::{draw_shell, Sim};
use crate::shell::view3d_panel::{
    build_view_scene, dispatch, show, Outbox, View3dCommand, View3dState, ViewRequest, ViewScope,
};
use crate::tools::camera::CameraVariant as V;
use crate::tools::ToolId;
use eframe::egui;
use plan_core::camera::PlanLight;
use plan_core::camera_view::{
    FloorsDisplayed, Fog, GroundKind, OrthoKind, RecordFormat, WalkRecord,
};
use plan_core::geometry::Point;
use plan_core::{CameraKind, CameraObject};
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

/// One headless frame of the 3D panel at clock `time`.
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

/// Closes the open dialog with Enter: two frames to lay it out, then Enter.
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

fn cmd(sim: &mut Sim, st: &mut View3dState, c: View3dCommand) {
    dispatch(c, &mut sim.app.cx, &mut sim.app.tools, st);
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("plan_s50_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs frames until `done` or the time is up.
fn until(
    ctx: &egui::Context,
    sim: &mut Sim,
    st: &mut View3dState,
    what: &str,
    done: &dyn Fn(&Sim, &View3dState) -> bool,
) {
    let started = Instant::now();
    let mut t = 100.0;
    while !done(sim, st) {
        t += 0.1;
        frame(ctx, sim, st, false, t);
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ----- light sets -----

#[test]
fn light_sets_choose_which_lights_shine_for_the_plan_and_for_one_camera() {
    let mut sim = two_storey();
    let a = sim
        .app
        .cx
        .project
        .add_light(0, PlanLight::new(Point::new(100.0, 100.0), 84.0))
        .unwrap();
    let b = sim
        .app
        .cx
        .project
        .add_light(0, PlanLight::new(Point::new(300.0, 200.0), 84.0))
        .unwrap();
    assert_eq!(
        crate::dialogs::camera::render_lights(&sim.app.cx.project).len(),
        2
    );
    let (mut st, _inbox, ctx) = state();
    cmd(&mut sim, &mut st, View3dCommand::Mode(CameraMode::Orbit));
    cmd(&mut sim, &mut st, View3dCommand::AdjustLights);
    frames(&ctx, &mut sim, &mut st, 2);
    {
        let d = st.adjust_lights_mut().expect("Adjust Lights is open");
        // A set made now holds the lights that are on now: both.
        assert!(d.add_set("Evening"));
        assert!(!d.add_set("evening"), "names are unique");
        assert_eq!(d.sets()[0].on.len(), 2);
        d.sets_mut()[0].on = vec![a];
        assert!(d.add_set("Movie"));
        d.use_set(Some(0));
        // Movie keeps only the light b.
        d.sets_mut()[1].on = vec![b];
    }
    // Nothing is stored until OK.
    assert!(sim.app.cx.project.lighting.sets.is_empty());
    ok(&ctx, &mut sim, &mut st);
    assert!(st.adjust_lights_mut().is_none(), "OK closed the dialog");
    let l = &sim.app.cx.project.lighting;
    assert_eq!(l.sets.len(), 2);
    assert_eq!(l.active_set.as_deref(), Some("Evening"));
    // The plan uses Evening: one light. A camera that names Movie gets the other.
    let project = &sim.app.cx.project;
    let evening = crate::dialogs::camera::shining_lights(project, None);
    assert_eq!(evening.iter().map(|l| l.id).collect::<Vec<_>>(), vec![a]);
    let movie = crate::dialogs::camera::shining_lights(project, Some("Movie"));
    assert_eq!(movie.iter().map(|l| l.id).collect::<Vec<_>>(), vec![b]);
    assert_eq!(
        crate::dialogs::camera::render_lights_in(project, Some("Movie")).len(),
        1
    );
    // The viewport shows the lights of the plan's set; a camera with its own
    // set shows that one.
    let mut cam = CameraObject::new(CameraKind::PerspectiveOverview, Point::ZERO, 0.0, "Den", 0);
    cam.view.light_set = Some("Movie".into());
    let id = sim.app.cx.project.add_camera(cam);
    st.show_camera(&sim.app.cx.project, id);
    frames(&ctx, &mut sim, &mut st, 2);
    assert_eq!(st.viewport.as_ref().unwrap().point_light_count(), 1);
    // The whole dialog was one undo step.
    sim.undo();
    assert!(sim.app.cx.project.lighting.sets.is_empty());
    assert_eq!(
        crate::dialogs::camera::render_lights(&sim.app.cx.project).len(),
        2
    );
    sim.redo();
    assert_eq!(sim.app.cx.project.lighting.sets.len(), 2);
    // The sets survive a save.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.lighting.sets.len(), 2);
    assert_eq!(back.lighting.active_set.as_deref(), Some("Evening"));
}

#[test]
fn a_deleted_light_leaves_every_light_set() {
    let mut sim = two_storey();
    let a = sim
        .app
        .cx
        .project
        .add_light(0, PlanLight::new(Point::new(100.0, 100.0), 84.0))
        .unwrap();
    sim.app.cx.project.lighting.add_set("Day", vec![a], vec![]);
    let (mut st, _inbox, ctx) = state();
    cmd(&mut sim, &mut st, View3dCommand::AdjustLights);
    frames(&ctx, &mut sim, &mut st, 2);
    st.adjust_lights_mut().unwrap().lights_mut().clear();
    ok(&ctx, &mut sim, &mut st);
    assert!(sim.app.cx.project.lights().is_empty());
    assert!(sim
        .app
        .cx
        .project
        .lighting
        .set("Day")
        .unwrap()
        .on
        .is_empty());
}

// ----- the movie -----

#[test]
fn a_walkthrough_records_a_motion_jpeg_movie_that_decodes() {
    let mut sim = two_storey();
    let mut cam = CameraObject::walkthrough(
        vec![Point::new(60.0, 60.0), Point::new(180.0, 60.0)],
        66.0,
        "Hall Walk",
        0,
    );
    cam.walk_speed = 120.0; // one second
    cam.view.walk = WalkRecord {
        fps: 4.0,
        width: 64,
        height: 48,
        samples: 1,
        format: RecordFormat::Video,
        quality: 80,
    };
    let id = sim.app.cx.project.add_camera(cam);
    let (mut st, _inbox, ctx) = state();
    // The dialog offers the format and starts from the camera's settings.
    cmd(&mut sim, &mut st, View3dCommand::Mode(CameraMode::Orbit));
    st.show_camera(&sim.app.cx.project, id);
    cmd(&mut sim, &mut st, View3dCommand::RecordWalkthrough);
    frames(&ctx, &mut sim, &mut st, 2);
    assert_eq!(
        st.record_dialog_mut().unwrap().settings.format,
        RecordFormat::Video
    );
    st.record_dialog_mut().unwrap().settings.format = RecordFormat::Both;
    let dir = temp_dir("movie");
    st.start_recording(&mut sim.app.cx, id, dir.clone());
    until(&ctx, &mut sim, &mut st, "the recording", &|sim, _| {
        sim.app.cx.status.starts_with("Recorded")
    });
    // This camera's own format (Video) was used: one movie, no frames.
    let movie = dir.join("Hall_Walk.avi");
    assert!(
        sim.app.cx.status.contains("Hall_Walk.avi"),
        "{}",
        sim.app.cx.status
    );
    let bytes = std::fs::read(&movie).unwrap();
    let info = plan_render::read_avi(&bytes).unwrap();
    assert_eq!((info.width, info.height), (64, 48));
    assert_eq!(info.frames.len(), 4);
    assert_eq!(info.fps, 4.0);
    for i in 0..4 {
        let f = plan_library::image::jpeg::decode(info.frame(&bytes, i).unwrap()).unwrap();
        assert_eq!((f.width, f.height), (64, 48));
    }
    assert!(!dir.join("frame_0001.png").exists());
    let _ = std::fs::remove_dir_all(&dir);
    // Both writes the movie and the numbered frames.
    sim.app
        .cx
        .project
        .update_camera(id, |c| c.view.walk.format = RecordFormat::Both);
    sim.app.cx.status.clear();
    let dir = temp_dir("both");
    st.start_recording(&mut sim.app.cx, id, dir.clone());
    until(
        &ctx,
        &mut sim,
        &mut st,
        "the second recording",
        &|sim, _| sim.app.cx.status.starts_with("Recorded"),
    );
    assert!(dir.join("Hall_Walk.avi").exists());
    assert!(dir.join("frame_0004.png").exists());
    assert!(dir.join("make_video.sh").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

// ----- 360 panorama -----

#[test]
fn export_360_panorama_writes_a_png_and_a_viewer_page_from_a_full_camera() {
    let mut sim = two_storey();
    let (mut st, inbox, ctx) = state();
    // An overview has no eye to look around from.
    cmd(&mut sim, &mut st, View3dCommand::Mode(CameraMode::Orbit));
    frames(&ctx, &mut sim, &mut st, 2);
    cmd(&mut sim, &mut st, View3dCommand::ExportPanorama);
    assert!(st.panorama_dialog_mut().is_none());
    assert!(
        sim.app.cx.status.contains("Full Camera"),
        "{}",
        sim.app.cx.status
    );
    // A Full Camera inside the shell.
    sim.tool(ToolId::CameraVariant(V::FullCamera));
    sim.drag((200.0, 150.0), (200.0, 250.0));
    let cam_id = sim.app.cx.project.cameras[0].id;
    inbox.post(ViewRequest::ShowCamera(cam_id));
    frames(&ctx, &mut sim, &mut st, 3);
    cmd(&mut sim, &mut st, View3dCommand::ExportPanorama);
    frames(&ctx, &mut sim, &mut st, 2);
    let dir = temp_dir("pano");
    {
        let d = st
            .panorama_dialog_mut()
            .expect("the Export 360 Panorama dialog");
        d.width = 0; // 1024 x 512
        d.samples = 1;
        d.path = dir.join("great room").display().to_string();
    }
    // Enter exports.
    frame(&ctx, &mut sim, &mut st, true, 0.6);
    assert!(st.panorama_running(), "{}", sim.app.cx.status);
    until(&ctx, &mut sim, &mut st, "the panorama", &|sim, _| {
        sim.app.cx.status.starts_with("Saved the panorama")
    });
    let png = std::fs::read(dir.join("great room.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    // 1024 x 512, twice as wide as high.
    let (w, h) = (
        u32::from_be_bytes(png[16..20].try_into().unwrap()),
        u32::from_be_bytes(png[20..24].try_into().unwrap()),
    );
    assert_eq!((w, h), (1024, 512));
    let html = std::fs::read_to_string(dir.join("great room.html")).unwrap();
    assert!(html.contains("data:image/jpeg;base64,") && html.contains("webgl"));
    assert!(html.contains("<title>"), "titled");
    assert!(!st.panorama_running());
    let _ = std::fs::remove_dir_all(&dir);
}

// ----- Undo Zoom -----

#[test]
fn undo_zoom_steps_back_through_zooms_and_pans_in_the_3d_view() {
    let mut sim = two_storey();
    let (mut st, _inbox, ctx) = state();
    cmd(&mut sim, &mut st, View3dCommand::Mode(CameraMode::Orbit));
    frames(&ctx, &mut sim, &mut st, 6);
    st.zoom.clear();
    frames(&ctx, &mut sim, &mut st, 2);
    let d0 = st.viewport.as_ref().unwrap().camera.distance;
    // Two zooms with the camera still in between.
    st.viewport.as_mut().unwrap().camera.distance = d0 * 0.7;
    frames(&ctx, &mut sim, &mut st, 3);
    st.viewport.as_mut().unwrap().camera.distance = d0 * 0.4;
    frames(&ctx, &mut sim, &mut st, 3);
    assert_eq!(st.zoom.steps(), 2);
    cmd(&mut sim, &mut st, View3dCommand::UndoZoom);
    assert!((st.viewport.as_ref().unwrap().camera.distance - d0 * 0.7).abs() < 0.01);
    frames(&ctx, &mut sim, &mut st, 3);
    assert_eq!(st.zoom.steps(), 1, "undoing is not itself a step");
    cmd(&mut sim, &mut st, View3dCommand::UndoZoom);
    assert!((st.viewport.as_ref().unwrap().camera.distance - d0).abs() < 0.01);
    frames(&ctx, &mut sim, &mut st, 3);
    cmd(&mut sim, &mut st, View3dCommand::UndoZoom);
    assert!(
        sim.app.cx.status.contains("Nothing to undo"),
        "{}",
        sim.app.cx.status
    );
    // A new view starts with no history.
    st.viewport.as_mut().unwrap().camera.distance = d0 * 0.5;
    frames(&ctx, &mut sim, &mut st, 3);
    assert_eq!(st.zoom.steps(), 1);
    cmd(
        &mut sim,
        &mut st,
        View3dCommand::Mode(CameraMode::DollHouse),
    );
    frames(&ctx, &mut sim, &mut st, 3);
    assert_eq!(st.zoom.steps(), 0);
}

// ----- orthographic Save Camera -----

#[test]
fn an_orthographic_view_is_saved_as_a_camera_and_restored() {
    let mut sim = two_storey();
    let (mut st, inbox, ctx) = state();
    cmd(
        &mut sim,
        &mut st,
        View3dCommand::Mode(CameraMode::ElevationLeft),
    );
    frames(&ctx, &mut sim, &mut st, 3);
    {
        let cam = &mut st.viewport.as_mut().unwrap().camera;
        cam.target[2] += 40.0;
        cam.ortho_half_height = 222.0;
    }
    frames(&ctx, &mut sim, &mut st, 2);
    let want = st.viewport.as_ref().unwrap().camera.clone();
    cmd(&mut sim, &mut st, View3dCommand::SaveCamera);
    let saved = sim
        .app
        .cx
        .project
        .cameras
        .last()
        .expect("a saved camera")
        .clone();
    assert_eq!(saved.kind, CameraKind::Orthographic);
    let o = saved.view.ortho.expect("the orthographic view");
    assert_eq!(o.kind, OrthoKind::ElevationLeft);
    assert_eq!(o.half_height, 222.0);
    assert!(!saved.view.show_in_plan);
    // Another view, then Restore.
    cmd(&mut sim, &mut st, View3dCommand::Mode(CameraMode::Orbit));
    frames(&ctx, &mut sim, &mut st, 3);
    inbox.post(ViewRequest::ShowCamera(saved.id));
    frames(&ctx, &mut sim, &mut st, 3);
    let got = &st.viewport.as_ref().unwrap().camera;
    assert_eq!(got.mode, CameraMode::ElevationLeft);
    assert_eq!(got.ortho_half_height, 222.0);
    assert!((got.target[2] - want.target[2]).abs() < 0.01);
    assert_eq!(got.yaw, want.yaw);
    // One undo step removes it.
    sim.undo();
    assert!(sim.app.cx.project.camera(saved.id).is_none());
}

// ----- Floors Displayed, ground and fog -----

#[test]
fn floors_displayed_can_pick_a_range_of_floors() {
    let mut sim = two_storey();
    let mut cam = CameraObject::new(CameraKind::PerspectiveOverview, Point::ZERO, 0.0, "Up", 0);
    cam.view.floors = FloorsDisplayed::Picked { from: 1, to: 1 };
    let id = sim.app.cx.project.add_camera(cam);
    let (mut st, inbox, ctx) = state();
    st.show_camera(&sim.app.cx.project, id);
    frames(&ctx, &mut sim, &mut st, 2);
    let f1 = sim.app.cx.project.floors[1].elevation;
    assert_eq!(
        st.scope_floor, None,
        "the top floor is picked: nothing above to leave out"
    );
    let limit = st.clip_below.expect("the floor below is cut away");
    assert!(limit < f1 && limit > f1 - 30.0, "{limit} under {f1}");
    let all = build_view_scene(&sim.app.cx.project, &ViewScope::default());
    let seen = build_view_scene(
        &sim.app.cx.project,
        &ViewScope {
            clip_below: st.clip_below,
            ..ViewScope::default()
        },
    );
    assert!(seen.triangle_count() < all.triangle_count());
    assert!(seen.triangle_count() > 0);
    // Only the ground floor: the upper one is not built.
    sim.app.cx.project.update_camera(id, |c| {
        c.view.floors = FloorsDisplayed::Picked { from: 0, to: 0 }
    });
    st.show_camera(&sim.app.cx.project, id);
    assert_eq!((st.scope_floor, st.clip_below), (Some(0), None));
    // Open the Camera Specification and pick floors there, OK saves them.
    sim.app
        .cx
        .project
        .update_camera(id, |c| c.view.floors = FloorsDisplayed::All);
    inbox.post(ViewRequest::OpenCameraSpec(id));
    frames(&ctx, &mut sim, &mut st, 2);
    st.camera_dialog_mut().unwrap().draft_mut().view.floors =
        FloorsDisplayed::Picked { from: 0, to: 1 };
    ok(&ctx, &mut sim, &mut st);
    assert_eq!(
        sim.app.cx.project.camera(id).unwrap().view.floors,
        FloorsDisplayed::Picked { from: 0, to: 1 }
    );
}

#[test]
fn ground_and_fog_from_the_backdrop_tab_reach_the_viewport() {
    let mut sim = two_storey();
    let mut cam = CameraObject::new(CameraKind::PerspectiveOverview, Point::ZERO, 0.0, "Hazy", 0);
    cam.view.backdrop.ground = GroundKind::Color;
    cam.view.backdrop.ground_color = [0, 255, 0];
    cam.view.backdrop.fog = Fog {
        on: true,
        distance_ft: 100.0,
        color: Some([255, 255, 255]),
    };
    let id = sim.app.cx.project.add_camera(cam);
    let (mut st, _inbox, ctx) = state();
    st.show_camera(&sim.app.cx.project, id);
    frames(&ctx, &mut sim, &mut st, 2);
    let vp = st.viewport.as_ref().unwrap();
    assert_eq!(vp.ground, plan_view3d::Ground::Solid([0.0, 1.0, 0.0]));
    assert!((vp.fog.density - 1.0 / 1200.0).abs() < 1e-9);
    assert_eq!(vp.fog.color, Some([1.0, 1.0, 1.0]));
    // A view with neither draws the defaults again.
    sim.app.cx.project.update_camera(id, |c| {
        c.view.backdrop.ground = GroundKind::Default;
        c.view.backdrop.fog.on = false;
    });
    st.show_camera(&sim.app.cx.project, id);
    frames(&ctx, &mut sim, &mut st, 2);
    let vp = st.viewport.as_ref().unwrap();
    assert_eq!(vp.ground, plan_view3d::Ground::Fade);
    assert!(!vp.fog.is_on());
}

// ----- Final View through the path tracer -----

#[test]
fn the_path_traced_final_view_waits_for_a_still_camera_refines_and_stops() {
    let mut sim = two_storey();
    sim.app.cx.selection.clear();
    let (mut st, _inbox, ctx) = state();
    cmd(&mut sim, &mut st, View3dCommand::Mode(CameraMode::Orbit));
    frames(&ctx, &mut sim, &mut st, 3);
    // Off: nothing is traced however long the camera rests.
    for i in 0..12 {
        frame(&ctx, &mut sim, &mut st, false, 10.0 + i as f64 * 0.2);
    }
    assert!(!st.final_view.is_showing());
    cmd(&mut sim, &mut st, View3dCommand::FinalViewToggle);
    assert!(st.final_view.enabled);
    // The camera must be still for half a second first.
    frame(&ctx, &mut sim, &mut st, false, 20.0);
    frame(&ctx, &mut sim, &mut st, false, 20.2);
    assert!(!st.final_view.is_running() && !st.final_view.is_showing());
    frame(&ctx, &mut sim, &mut st, false, 20.7);
    until(&ctx, &mut sim, &mut st, "the first pass", &|_, st| {
        st.final_view.is_showing()
    });
    assert!(st.final_view.is_running() || st.final_view.progress().0 > 0);
    // Stop keeps the picture and ends the render.
    st.final_view.stop();
    until(&ctx, &mut sim, &mut st, "the render to end", &|_, st| {
        !st.final_view.is_running()
    });
    assert!(st.final_view.is_showing(), "the picture stays");
    let (done, total) = st.final_view.progress();
    assert!(done > 0 && done <= total);
    // Moving the camera drops the picture and returns to the live view.
    st.viewport.as_mut().unwrap().camera.orbit(0.4, 0.0);
    frames(&ctx, &mut sim, &mut st, 2);
    assert!(!st.final_view.is_showing());
    // Preview quality never traces.
    cmd(
        &mut sim,
        &mut st,
        View3dCommand::Quality(plan_core::camera_view::ViewQuality::Preview),
    );
    for i in 0..10 {
        frame(&ctx, &mut sim, &mut st, false, 40.0 + i as f64 * 0.2);
    }
    assert!(!st.final_view.is_showing());
}

#[test]
fn refresh_and_rebuild_run_from_the_3d_menu_commands() {
    let mut sim = two_storey();
    let (mut st, _inbox, ctx) = state();
    cmd(&mut sim, &mut st, View3dCommand::Mode(CameraMode::Orbit));
    frames(&ctx, &mut sim, &mut st, 2);
    assert!(!st.scene_dirty);
    cmd(&mut sim, &mut st, View3dCommand::Refresh);
    assert!(st.scene_dirty);
    assert_eq!(sim.app.cx.status, "Refreshed the 3D view");
    frames(&ctx, &mut sim, &mut st, 1);
    assert!(!st.scene_dirty);
    cmd(&mut sim, &mut st, View3dCommand::Rebuild);
    assert!(st.scene_dirty);
}
