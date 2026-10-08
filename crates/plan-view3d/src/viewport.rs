//! The egui widget: input handling plus the paint callback that drives [`gpu`](crate::gpu).

use std::sync::{Arc, Mutex, MutexGuard};

use eframe::egui::{self, Key, PointerButton, Sense};
use eframe::egui_glow;
use eframe::glow;
use plan_3d::{Bounds, Scene};

use crate::camera::{Camera, CameraMode};
use crate::gpu::{FrameParams, GpuScene};
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
    gpu: Arc<Mutex<GpuScene>>,
    pending: Arc<Mutex<Option<Scene>>>,
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
        Viewport3d {
            camera: Camera::default(),
            background: [0.85, 0.89, 0.94, 1.0],
            show_edges: true,
            lighting: Lighting::default(),
            gpu: Arc::new(Mutex::new(GpuScene::default())),
            pending: Arc::new(Mutex::new(None)),
            bounds: None,
            aspect: 1.0,
        }
    }

    /// Upload `scene` right away (needs the GL context, e.g. eframe's
    /// `CreationContext::gl`) and frame the camera on it.
    pub fn set_scene(&mut self, gl: &glow::Context, scene: &Scene) {
        *lock(&self.pending) = None;
        lock(&self.gpu).upload(gl, scene);
        self.bounds = scene.bounds();
        self.fit_view();
    }

    /// Queue `scene` to be uploaded on the next paint, for callers that have
    /// no GL context at hand. Also frames the camera on it.
    pub fn queue_scene(&mut self, scene: &Scene) {
        self.bounds = scene.bounds();
        *lock(&self.pending) = Some(scene.clone());
        self.fit_view();
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
            view_proj: self.camera.view_projection(self.aspect),
            eye: self.camera.eye(),
            view_dir: self.camera.forward(),
            ortho: self.camera.mode.is_orthographic(),
            lighting: self.lighting,
            show_edges: self.show_edges,
            hide_ceiling_roof: self.camera.mode.hides_ceiling_and_roof(),
        };
        let gpu = Arc::clone(&self.gpu);
        let pending = Arc::clone(&self.pending);
        let callback = egui_glow::CallbackFn::new(move |info, painter| {
            let gl = painter.gl().as_ref();
            let mut gpu = lock(&gpu);
            match lock(&pending).take() {
                Some(scene) => gpu.upload(gl, &scene),
                None => gpu.ensure_program(gl),
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
                gpu.paint(gl, &frame, scissor);
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

        if pan_drag || (primary && mode.is_orthographic()) {
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
