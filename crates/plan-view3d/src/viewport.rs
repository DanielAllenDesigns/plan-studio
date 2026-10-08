//! The egui widget: input handling plus the paint callback that drives [`gpu`](crate::gpu).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use eframe::egui::{self, Key, PointerButton, Sense};
use eframe::egui_glow;
use eframe::glow;
use plan_3d::{Bounds, Mesh, Scene};
use plan_materials::textures::TextureStore;

use crate::camera::{Camera, CameraMode};
use crate::gpu::{FrameParams, GpuScene};
use crate::quality::{nearest_lights, Look, ViewLight, ViewSettings, MAX_POINT_LIGHTS};
use crate::texturing::{self, ImageTexture, SurfaceTexture};
use crate::walkthrough::Walkthrough;

/// Orbit sensitivity, radians per dragged pixel.
const ORBIT_RADIANS_PER_PX: f32 = 0.01;
/// Look-around sensitivity, radians per dragged pixel.
const TURN_RADIANS_PER_PX: f32 = 0.005;
/// Zoom per scrolled point (exponent applied to the distance).
const SCROLL_ZOOM: f32 = 0.002;
/// First-person walking speed, inches per second (10 ft/s).
const WALK_SPEED: f32 = 120.0;
/// Walking speed multiplier while Shift is held.
const RUN_FACTOR: f32 = 3.0;

/// Light rig used by the Lambert + ambient shader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    /// Direction *toward* the key light in scene space (need not be unit
    /// length). The default is upper-left-front: -X, +Y, +Z.
    pub key_dir: [f32; 3],
    /// Constant ambient term, 0..1.
    pub ambient: f32,
    /// Key light intensity, 0..1. A fill light from the opposite side adds
    /// 35% of this.
    pub key: f32,
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting {
            key_dir: [-0.4, 0.8, 0.5],
            ambient: 0.45,
            key: 0.65,
        }
    }
}

/// A 3D viewport widget for a [`plan_3d::Scene`].
///
/// Needs an eframe app running on the glow backend with a depth buffer
/// (`NativeOptions { depth_buffer: 24, .. }`).
pub struct Viewport3d {
    /// The camera; edit directly or through [`Viewport3d::set_mode`].
    pub camera: Camera,
    /// Background color, gamma-encoded RGBA in 0..1.
    pub background: [f32; 4],
    /// Draw dark feature edges over the shaded surfaces.
    pub show_edges: bool,
    /// Lighting parameters.
    pub lighting: Lighting,
    /// Paint textured materials and pictures with their bitmaps. The Standard
    /// technique wants this on; flat and line techniques turn it off.
    pub textures_enabled: bool,
    /// The look of the interactive view (set from the rendering technique).
    pub look: Look,
    /// Shadows, ambient occlusion, quality and exposure.
    pub settings: ViewSettings,
    /// A picture drawn behind the model in place of the sky gradient (the
    /// Backdrop tab of a camera); only the looks that draw a sky show it.
    pub backdrop: Option<Arc<crate::backdrop::BackdropImage>>,
    /// Point lights of the plan; the nearest few light the view.
    lights: Vec<ViewLight>,
    gpu: Arc<Mutex<GpuScene>>,
    pending: Arc<Mutex<Option<Arc<Scene>>>>,
    /// The scene last handed over, kept to re-upload after a lost GL context.
    last_scene: Option<Arc<Scene>>,
    pending_pictures: Arc<Mutex<Option<Vec<ImageTexture>>>>,
    last_pictures: Vec<ImageTexture>,
    pending_overlay: Arc<Mutex<Option<Arc<Vec<Mesh>>>>>,
    last_overlay: Arc<Vec<Mesh>>,
    pending_surface: Arc<Mutex<Option<Vec<SurfaceTexture>>>>,
    last_surface: Vec<SurfaceTexture>,
    /// While true a drag does not move the camera (the caller is using the
    /// drag for something else, such as moving an object); scrolling still
    /// zooms.
    pub drag_locked: bool,
    store: Arc<TextureStore>,
    context_lost: Arc<AtomicBool>,
    /// Material textures the last frame still had to upload.
    pending_textures: Arc<AtomicUsize>,
    prefetching: Arc<AtomicUsize>,
    bounds: Option<Bounds>,
    aspect: f32,
}

impl Default for Viewport3d {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn to_color32(c: [f32; 4]) -> egui::Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Color32::from_rgba_unmultiplied(b(c[0]), b(c[1]), b(c[2]), b(c[3]))
}

/// Intersect two pixel rectangles `(x, y, w, h)`; returns `None` when empty.
fn intersect(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
    let x0 = a.0.max(b.0);
    let y0 = a.1.max(b.1);
    let x1 = (a.0 + a.2).min(b.0 + b.2);
    let y1 = (a.1 + a.3).min(b.1 + b.3);
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1 - x0, y1 - y0))
}

impl Viewport3d {
    /// A viewport with an empty scene and a light sky-blue background.
    pub fn new() -> Self {
        Self::with_texture_store(TextureStore::shared())
    }

    /// A viewport that looks material textures up in `store` (tests and
    /// tools without Chief's files pass a store with no folders).
    pub fn with_texture_store(store: Arc<TextureStore>) -> Self {
        let gpu = GpuScene::with_store(Arc::clone(&store));
        Viewport3d {
            camera: Camera::default(),
            background: [0.85, 0.89, 0.94, 1.0],
            show_edges: true,
            lighting: Lighting::default(),
            textures_enabled: true,
            look: Look::Standard,
            settings: ViewSettings::default(),
            backdrop: None,
            lights: Vec::new(),
            prefetching: Arc::clone(&gpu.prefetching),
            gpu: Arc::new(Mutex::new(gpu)),
            pending: Arc::new(Mutex::new(None)),
            last_scene: None,
            pending_pictures: Arc::new(Mutex::new(None)),
            last_pictures: Vec::new(),
            pending_overlay: Arc::new(Mutex::new(None)),
            last_overlay: Arc::new(Vec::new()),
            pending_surface: Arc::new(Mutex::new(None)),
            last_surface: Vec::new(),
            drag_locked: false,
            store,
            context_lost: Arc::new(AtomicBool::new(false)),
            pending_textures: Arc::new(AtomicUsize::new(0)),
            bounds: None,
            aspect: 1.0,
        }
    }

    /// Upload `scene` right away (needs the GL context, e.g. eframe's
    /// `CreationContext::gl`) and frame the camera on it.
    pub fn set_scene(&mut self, gl: &glow::Context, scene: &Scene) {
        *lock(&self.pending) = None;
        lock(&self.gpu).upload(gl, scene);
        self.last_scene = Some(Arc::new(scene.clone()));
        self.prefetch_textures(scene);
        self.bounds = scene.bounds();
        self.fit_view();
    }

    /// Queue `scene` to be uploaded on the next paint, for callers that have
    /// no GL context at hand. Also frames the camera on it.
    pub fn queue_scene(&mut self, scene: &Scene) {
        self.bounds = scene.bounds();
        let scene = Arc::new(scene.clone());
        self.prefetch_textures(&scene);
        *lock(&self.pending) = Some(Arc::clone(&scene));
        self.last_scene = Some(scene);
        self.fit_view();
    }

    /// Decode the textures `scene` needs on a background thread, so the first
    /// frame that shows them only has to upload (a 2048 pixel JPEG takes
    /// about 100 ms to decode).
    fn prefetch_textures(&self, scene: &Scene) {
        let missing: Vec<_> = texturing::needed_materials(scene)
            .into_iter()
            .filter(|m| !self.store.is_cached(&TextureStore::material_key(*m)))
            .collect();
        if missing.is_empty() || !self.textures_enabled {
            return;
        }
        let (store, flag) = (Arc::clone(&self.store), Arc::clone(&self.prefetching));
        flag.fetch_add(1, Ordering::SeqCst);
        let spawned = std::thread::Builder::new()
            .name("texture-prefetch".into())
            .spawn(move || {
                for m in missing {
                    let _ = store.material(m);
                }
                flag.fetch_sub(1, Ordering::SeqCst);
            });
        if spawned.is_err() {
            self.prefetching.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Set the pictures to draw on their quads (see [`ImageTexture`]); the
    /// list replaces the previous one. Uploads happen on the next paint and
    /// only for keys not already on the GPU, so calling this every time the
    /// pictures may have changed is cheap.
    pub fn set_image_textures(&mut self, images: Vec<ImageTexture>) {
        self.last_pictures = images.clone();
        *lock(&self.pending_pictures) = Some(images);
    }

    /// Set the overlay: meshes drawn after, and over, the cached scene (the
    /// selection tint, the hover tint, billboards that turn with the camera).
    /// The list replaces the previous one. Unlike [`Viewport3d::queue_scene`]
    /// it does not touch the cached scene, the camera framing or the shadow
    /// map, and its meshes cast no shadows, so it is cheap to call whenever
    /// the selection or the pointer changes. A new scene clears the overlay
    /// on the GPU; the last overlay is put back with it.
    pub fn set_overlay(&mut self, meshes: Vec<Mesh>) {
        let meshes = Arc::new(meshes);
        self.last_overlay = Arc::clone(&meshes);
        *lock(&self.pending_overlay) = Some(meshes);
    }

    /// The meshes of the overlay last set.
    pub fn overlay(&self) -> &[Mesh] {
        &self.last_overlay
    }

    /// Meshes in the scene last handed over (not counting the overlay).
    pub fn scene_mesh_count(&self) -> usize {
        self.last_scene.as_ref().map_or(0, |s| s.meshes.len())
    }

    /// Set the bitmaps of painted materials (see [`SurfaceTexture`]); the
    /// list replaces the previous one. Uploaded on the next paint, only for
    /// keys not already resident.
    pub fn set_surface_textures(&mut self, textures: Vec<SurfaceTexture>) {
        self.last_surface = textures.clone();
        *lock(&self.pending_surface) = Some(textures);
    }

    /// Painted-material bitmaps resident on the GPU.
    pub fn uploaded_surface_textures(&self) -> usize {
        lock(&self.gpu).surface_texture_count()
    }

    /// Overlay meshes resident on the GPU.
    pub fn uploaded_overlay_meshes(&self) -> usize {
        lock(&self.gpu).overlay_count()
    }

    /// Tell the viewport the OpenGL context was replaced or lost: every GL
    /// object is forgotten (not deleted) and the last scene, pictures and
    /// material textures are uploaded again on the next paint. A replaced
    /// context is also noticed on its own.
    pub fn context_lost(&mut self) {
        self.context_lost.store(true, Ordering::SeqCst);
    }

    /// The store material textures come from.
    pub fn texture_store(&self) -> Arc<TextureStore> {
        Arc::clone(&self.store)
    }

    /// Material textures resident on the GPU.
    pub fn uploaded_material_textures(&self) -> usize {
        lock(&self.gpu).material_texture_count()
    }

    /// Picture textures resident on the GPU.
    pub fn uploaded_picture_textures(&self) -> usize {
        lock(&self.gpu).picture_texture_count()
    }

    /// Re-frame the camera on the current scene for the current mode.
    pub fn fit_view(&mut self) {
        if let Some((min, max)) = self.bounds {
            self.camera.fit_to_bounds_with_aspect(min, max, self.aspect);
        }
    }

    /// Switch view mode. Orthographic views are re-fitted to the scene;
    /// entering `FullCamera` drops the eye at eye height over the orbit target.
    pub fn set_mode(&mut self, mode: CameraMode) {
        let was_full = self.camera.mode == CameraMode::FullCamera;
        let refit = mode.is_orthographic() || self.camera.mode.is_orthographic();
        self.camera.set_mode(mode);
        if refit {
            self.fit_view();
        }
        if mode == CameraMode::FullCamera && !was_full {
            let floor = self.bounds.map_or(0.0, |(min, _)| min[1]);
            let t = self.camera.target;
            self.camera.position = [t[0], floor + self.camera.eye_height, t[2]];
        }
    }

    /// Light the view with the plan's point lights (the renderer's list); the
    /// nearest few to the eye are evaluated per pixel.
    pub fn set_point_lights(&mut self, lights: &[plan_render::PointLight]) {
        self.lights = lights
            .iter()
            .map(|l| ViewLight {
                position: l.position,
                // Same units as the ray tracer: albedo / pi * intensity / d^2.
                color: l
                    .color
                    .map(|c| c * l.intensity.max(0.0) / std::f32::consts::PI),
            })
            .collect();
    }

    /// Why a shadow, occlusion or composite pass is off (shader compile
    /// failures, targets the driver cannot make). Empty when all is well.
    pub fn render_notes(&self) -> Vec<String> {
        lock(&self.gpu).notes().to_vec()
    }

    /// Point lights currently set.
    pub fn point_light_count(&self) -> usize {
        self.lights.len()
    }

    /// Replace the camera wholesale (for example with a saved view or a
    /// walkthrough pose). The scene framing is left untouched.
    pub fn set_camera(&mut self, camera: &Camera) {
        self.camera = camera.clone();
    }

    /// Show the walkthrough's camera at time `t` seconds (Walkthrough Preview).
    pub fn play_walkthrough(&mut self, walkthrough: &Walkthrough, t: f64) {
        self.camera = walkthrough.camera_at(t);
    }

    /// Scene bounds from the last `set_scene` / `queue_scene`, if any.
    pub fn bounds(&self) -> Option<Bounds> {
        self.bounds
    }

    /// Why OpenGL setup failed (shader compile error, GL too old...), if it did.
    pub fn gl_error(&self) -> Option<String> {
        lock(&self.gpu).error().map(str::to_owned)
    }

    /// Number of meshes currently uploaded to the GPU.
    pub fn uploaded_mesh_count(&self) -> usize {
        lock(&self.gpu).mesh_count()
    }

    /// Free all GL objects. Call from `eframe::App::on_exit` with
    /// `frame.gl()`, or before replacing the viewport.
    pub fn destroy(&mut self, gl: &glow::Context) {
        *lock(&self.pending) = None;
        *lock(&self.pending_pictures) = None;
        *lock(&self.pending_overlay) = None;
        *lock(&self.pending_surface) = None;
        lock(&self.gpu).destroy(gl);
    }

    /// Allocate `size` in `ui`, process mouse/keyboard input and paint the
    /// scene. Returns the widget's response.
    ///
    /// Controls: left-drag orbits; right/middle-drag (or Shift+left-drag)
    /// pans; scroll dollies. In `FullCamera`, left-drag turns, W/A/S/D or the
    /// arrow keys walk (Shift runs) and scroll moves forward. In the
    /// orthographic modes any drag pans and scroll zooms.
    pub fn ui(&mut self, ui: &mut egui::Ui, size: egui::Vec2) -> egui::Response {
        let size = egui::vec2(size.x.max(1.0), size.y.max(1.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let aspect = rect.width() / rect.height();
        if (aspect - self.aspect).abs() > 1e-3 {
            self.aspect = aspect;
            if self.camera.mode.is_orthographic() {
                self.fit_view_keep_target();
            }
        }
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        self.handle_input(ui, &response, rect.height());

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, to_color32(self.background));

        let frame = FrameParams {
            lighting: self.lighting,
            show_edges: self.show_edges,
            textures: self.textures_enabled,
            look: self.look,
            settings: self.settings,
            background: self.background,
            backdrop: self.backdrop.clone(),
            lights: nearest_lights(&self.lights, self.camera.eye(), MAX_POINT_LIGHTS),
            bounds: self.bounds,
            pixels_per_point: ui.ctx().pixels_per_point(),
            ..FrameParams::for_camera(&self.camera, self.aspect, (0, 0, 1, 1))
        };
        // Textures still decoding or waiting for their upload: keep painting.
        if self.textures_enabled
            && (self.prefetching.load(Ordering::Relaxed) > 0
                || self.pending_textures.load(Ordering::Relaxed) > 0)
        {
            ui.ctx().request_repaint();
        }
        let gpu = Arc::clone(&self.gpu);
        let pending = Arc::clone(&self.pending);
        let pending_pictures = Arc::clone(&self.pending_pictures);
        let pending_overlay = Arc::clone(&self.pending_overlay);
        let pending_surface = Arc::clone(&self.pending_surface);
        let lost = Arc::clone(&self.context_lost);
        let (last_scene, last_pictures) = (self.last_scene.clone(), self.last_pictures.clone());
        let (last_overlay, last_surface) =
            (Arc::clone(&self.last_overlay), self.last_surface.clone());
        let pending_textures = Arc::clone(&self.pending_textures);
        let callback = egui_glow::CallbackFn::new(move |info, painter| {
            let gl = painter.gl().as_ref();
            let mut gpu = lock(&gpu);
            let mut scene = lock(&pending).take();
            let mut pictures = lock(&pending_pictures).take();
            let mut surface = lock(&pending_surface).take();
            let mut overlay = lock(&pending_overlay).take();
            if lost.swap(false, Ordering::SeqCst) || gpu.context_was_replaced(gl) {
                // The context's objects are gone: start over from the last
                // scene, pictures, painted bitmaps and overlay.
                gpu.forget_context();
                scene = scene.or_else(|| last_scene.clone());
                pictures = pictures.or_else(|| Some(last_pictures.clone()));
                surface = surface.or_else(|| Some(last_surface.clone()));
            }
            match scene {
                Some(scene) => {
                    gpu.upload(gl, &scene);
                    // The upload cleared the overlay: put the last one back.
                    overlay = overlay.or_else(|| Some(Arc::clone(&last_overlay)));
                }
                None => gpu.ensure_program(gl),
            }
            if let Some(list) = pictures {
                gpu.set_pictures(gl, &list);
            }
            if let Some(list) = surface {
                gpu.set_surface_textures(gl, &list);
            }
            if let Some(meshes) = overlay {
                gpu.set_overlay(gl, &meshes);
            }
            let vp = info.viewport_in_pixels();
            let clip = info.clip_rect_in_pixels();
            let scissor = intersect(
                (vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px),
                (
                    clip.left_px,
                    clip.from_bottom_px,
                    clip.width_px,
                    clip.height_px,
                ),
            );
            if let Some(scissor) = scissor {
                let frame = FrameParams {
                    viewport: (vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px),
                    target_fbo: painter.intermediate_fbo(),
                    ..frame.clone()
                };
                gpu.paint(gl, &frame, scissor);
                pending_textures.store(gpu.pending_uploads(), Ordering::Relaxed);
            }
        });
        painter.add(egui::PaintCallback {
            rect,
            callback: Arc::new(callback),
        });
        response
    }

    /// Resize an orthographic view for a new aspect ratio without moving the
    /// target the user may have panned to.
    fn fit_view_keep_target(&mut self) {
        let target = self.camera.target;
        self.fit_view();
        if self.bounds.is_some() {
            self.camera.target = target;
        }
    }

    fn handle_input(&mut self, ui: &egui::Ui, response: &egui::Response, height_px: f32) {
        let shift = ui.input(|i| i.modifiers.shift);
        let delta = response.drag_delta();
        let primary = response.dragged_by(PointerButton::Primary);
        let pan_drag = response.dragged_by(PointerButton::Secondary)
            || response.dragged_by(PointerButton::Middle)
            || (primary && shift);
        let mode = self.camera.mode;

        if self.drag_locked {
            // The drag belongs to the caller.
        } else if pan_drag || (primary && mode.is_orthographic()) {
            self.camera.pan(delta.x, delta.y, height_px);
        } else if primary {
            if mode == CameraMode::FullCamera {
                self.camera.turn(
                    -delta.x * TURN_RADIANS_PER_PX,
                    -delta.y * TURN_RADIANS_PER_PX,
                );
            } else {
                self.camera.orbit(
                    -delta.x * ORBIT_RADIANS_PER_PX,
                    delta.y * ORBIT_RADIANS_PER_PX,
                );
            }
        }

        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.dolly(scroll * SCROLL_ZOOM);
            }
        }

        if mode == CameraMode::FullCamera && (response.hovered() || response.has_focus()) {
            let (dt, forward, strafe, run) = ui.input(|i| {
                let axis = |pos: &[Key], neg: &[Key]| {
                    let down = |keys: &[Key]| keys.iter().any(|k| i.key_down(*k));
                    f32::from(down(pos)) - f32::from(down(neg))
                };
                (
                    i.stable_dt.min(0.1),
                    axis(&[Key::W, Key::ArrowUp], &[Key::S, Key::ArrowDown]),
                    axis(&[Key::D, Key::ArrowRight], &[Key::A, Key::ArrowLeft]),
                    i.modifiers.shift,
                )
            });
            if forward != 0.0 || strafe != 0.0 {
                let step = WALK_SPEED * if run { RUN_FACTOR } else { 1.0 } * dt;
                self.camera.walk(forward * step, strafe * step);
                ui.ctx().request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scissor_intersection() {
        assert_eq!(
            intersect((0, 0, 100, 100), (50, 50, 100, 100)),
            Some((50, 50, 50, 50))
        );
        assert_eq!(intersect((0, 0, 10, 10), (20, 20, 5, 5)), None);
    }

    #[test]
    fn default_lighting_is_upper_left_front() {
        let l = Lighting::default();
        assert!(l.key_dir[0] < 0.0 && l.key_dir[1] > 0.0 && l.key_dir[2] > 0.0);
    }

    #[test]
    fn a_new_viewport_has_sensible_shading_defaults() {
        let vp = Viewport3d::new();
        assert!(vp.settings.shadows && vp.settings.ambient_occlusion);
        assert_eq!(vp.settings.quality, crate::Quality::Medium);
        assert_eq!(vp.look, Look::Standard);
        assert!(vp.render_notes().is_empty());
    }

    #[test]
    fn plan_lights_become_shader_lights_in_the_ray_tracers_units() {
        let mut vp = Viewport3d::new();
        vp.set_point_lights(&[
            plan_render::PointLight {
                position: [10.0, 80.0, -20.0],
                intensity: 12_000.0 * std::f32::consts::PI,
                color: [1.0, 0.5, 0.25],
                radius: 2.0,
            },
            plan_render::PointLight {
                position: [0.0; 3],
                intensity: -5.0,
                color: [1.0; 3],
                radius: 0.0,
            },
        ]);
        assert_eq!(vp.point_light_count(), 2);
        let l = &vp.lights;
        assert_eq!(l[0].position, [10.0, 80.0, -20.0]);
        assert!((l[0].color[0] - 12_000.0).abs() < 1.0);
        assert!((l[0].color[1] - 6_000.0).abs() < 1.0);
        assert_eq!(l[1].color, [0.0; 3], "negative intensity gives no light");
    }

    #[test]
    fn queued_scene_frames_the_camera_without_gl() {
        use plan_3d::{Material, Mesh, Vertex};
        let v = |x, y, z| Vertex {
            position: [x, y, z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        };
        let scene = Scene {
            meshes: vec![Mesh {
                vertices: vec![v(0.0, 0.0, 0.0), v(240.0, 0.0, 0.0), v(240.0, 0.0, -120.0)],
                indices: vec![0, 1, 2],
                material: Material::Floor,
                object_id: None,
                color: None,
            }],
        };
        let mut vp = Viewport3d::new();
        vp.queue_scene(&scene);
        assert_eq!(vp.bounds(), scene.bounds());
        assert!((vp.camera.target[0] - 120.0).abs() < 1e-3);
        assert!(vp.camera.distance > 0.0);
        vp.set_mode(CameraMode::FullCamera);
        assert!((vp.camera.position[1] - 66.0).abs() < 1e-3);
        vp.set_mode(CameraMode::ElevationFront);
        assert!(vp.camera.ortho_half_height > 0.0);
    }
}
