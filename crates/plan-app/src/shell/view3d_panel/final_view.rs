//! Final View through the path tracer, in the 3D panel (C-53).
//!
//! With 3D > Final View (Path Traced) on and the quality set to Final View,
//! the panel waits for the camera to stop moving, then ray traces the view in
//! passes of growing size and paints each pass over the OpenGL picture. The
//! image sharpens until the sample count is reached or the user presses Stop;
//! moving the camera, editing the plan or changing the technique throws the
//! picture away and returns to the live view.
//!
//! The decision of when to start and stop is [`Settler`], plain Rust over a
//! clock and a key (a hash of everything the picture depends on), so it is
//! tested without a window. The render runs on a worker thread
//! ([`spawn`]) that publishes the latest pass and honours a cancel flag.

use eframe::egui;
use plan_3d::Scene;
use plan_core::camera_view::{CameraView, Lighting};
use plan_render::{Environment, Image, PointLight, RenderSettings, Renderer, Style, Sun};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// How long the camera must be still before the render starts, seconds.
pub const SETTLE_SECONDS: f64 = 0.5;
/// Widest picture traced, pixels; the picture is stretched over the view.
pub const MAX_TRACE_WIDTH: u32 = 960;
/// Samples per pixel of a Final View.
pub const FINAL_SAMPLES: u32 = 64;

/// The sun of the plan's lighting (3D > Lighting) for the path tracer, scaled
/// by the camera's own sun strength when it has one. A sun at or below the
/// horizon, or switched off, leaves the sky alone.
pub fn environment_for(lighting: &Lighting, view: Option<&CameraView>) -> Environment {
    let strength = view.map_or(lighting.sun_intensity, |v| v.sun_in(lighting));
    let strength = strength.clamp(0.0, 2.0) as f32;
    let sun = (lighting.sun_altitude_deg > 0.0 && strength > 0.0).then(|| {
        let mut sun = Sun::from_azimuth_altitude(
            lighting.sun_azimuth_deg as f32,
            lighting.sun_altitude_deg as f32,
        );
        sun.intensity *= strength;
        sun
    });
    Environment {
        sun,
        ..Environment::default()
    }
}

/// What the settler tells the panel to do this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing to do (Final View is off, or the user stopped this picture).
    Idle,
    /// The view just changed or has not been still long enough.
    Wait,
    /// Start the render now.
    Start,
    /// Let the running or finished render stand.
    Keep,
    /// The view changed: stop the render and drop its picture.
    Cancel,
}

/// Decides when a Final View render starts and when it is dropped.
#[derive(Debug, Default)]
pub struct Settler {
    key: Option<u64>,
    since: f64,
    started: Option<u64>,
    stopped: Option<u64>,
}

impl Settler {
    /// One frame: `key` identifies what is on screen (`None` when no Final
    /// View may run: off, Preview quality, an orthographic view...), `now` is
    /// the clock in seconds and `has_render` says whether a render or its
    /// picture exists.
    pub fn step(&mut self, now: f64, key: Option<u64>, has_render: bool) -> Step {
        let Some(key) = key else {
            *self = Settler::default();
            return if has_render { Step::Cancel } else { Step::Idle };
        };
        if self.key != Some(key) {
            *self = Settler {
                key: Some(key),
                since: now,
                ..Settler::default()
            };
            return if has_render { Step::Cancel } else { Step::Wait };
        }
        if self.started == Some(key) {
            return Step::Keep;
        }
        if self.stopped == Some(key) {
            return Step::Idle;
        }
        if now - self.since >= SETTLE_SECONDS {
            self.started = Some(key);
            Step::Start
        } else {
            Step::Wait
        }
    }

    /// The user pressed Stop: keep this picture and do not restart it.
    pub fn stop(&mut self) {
        self.stopped = self.key;
    }

    /// Seconds until the render may start, for scheduling a repaint.
    pub fn wait_left(&self, now: f64) -> f64 {
        (SETTLE_SECONDS - (now - self.since)).max(0.0)
    }
}

/// One published pass.
#[derive(Clone, Debug)]
pub struct Pass {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub samples: u32,
}

/// State shared with the worker thread.
pub struct Shared {
    pub total: u32,
    done: AtomicU32,
    cancel: AtomicBool,
    finished: AtomicBool,
    latest: Mutex<Option<Pass>>,
}

impl Shared {
    /// Samples finished so far.
    #[cfg(test)]
    pub fn done(&self) -> u32 {
        self.done.load(Ordering::Relaxed)
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The newest pass not yet taken.
    pub fn take_latest(&self) -> Option<Pass> {
        self.latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

/// Everything the worker renders.
pub struct Request {
    pub scene: Scene,
    pub camera: plan_render::Camera,
    pub env: Environment,
    pub lights: Vec<PointLight>,
    pub settings: RenderSettings,
    pub style: Option<Style>,
}

/// Starts rendering `req` on a worker thread.
pub fn spawn(req: Request) -> Arc<Shared> {
    let shared = Arc::new(Shared {
        total: req.settings.samples.max(1),
        done: AtomicU32::new(0),
        cancel: AtomicBool::new(false),
        finished: AtomicBool::new(false),
        latest: Mutex::new(None),
    });
    let worker = Arc::clone(&shared);
    let spawned = std::thread::Builder::new()
        .name("final-view".into())
        .spawn(move || {
            let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let renderer = Renderer::new(&req.scene);
                renderer.render_progressive(
                    &req.camera,
                    &req.env,
                    &req.lights,
                    &req.settings,
                    &mut |img: &Image, samples: u32| {
                        let rgba = match req.style {
                            Some(s) => plan_render::stylize(&img.rgba, img.width, img.height, s),
                            None => img.rgba.clone(),
                        };
                        *worker.latest.lock().unwrap_or_else(PoisonError::into_inner) =
                            Some(Pass {
                                width: img.width,
                                height: img.height,
                                rgba,
                                samples,
                            });
                        worker.done.store(samples, Ordering::Relaxed);
                        !worker.cancel.load(Ordering::Relaxed)
                    },
                );
            }));
            // A panic leaves the live view in place; nothing to report.
            let _ = run;
            worker.finished.store(true, Ordering::Relaxed);
        });
    if spawned.is_err() {
        shared.finished.store(true, Ordering::Relaxed);
    }
    shared
}

/// The size to trace for a view of `view_px` pixels: at most
/// [`MAX_TRACE_WIDTH`] wide, the same shape, never smaller than 16 pixels.
pub fn trace_size(view_px: (f32, f32)) -> (u32, u32) {
    let (w, h) = (view_px.0.max(1.0), view_px.1.max(1.0));
    let scale = (MAX_TRACE_WIDTH as f32 / w).min(1.0);
    (
        ((w * scale).round() as u32).max(16),
        ((h * scale).round() as u32).max(16),
    )
}

/// The Final View of the panel.
#[derive(Default)]
pub struct FinalView {
    /// 3D > Final View (Path Traced).
    pub enabled: bool,
    settler: Settler,
    job: Option<Arc<Shared>>,
    texture: Option<egui::TextureHandle>,
    samples: u32,
}

impl FinalView {
    /// Is a picture (finished or being refined) over the live view?
    #[cfg(test)]
    pub fn is_showing(&self) -> bool {
        self.texture.is_some()
    }

    /// Is the worker still refining?
    pub fn is_running(&self) -> bool {
        self.job.as_ref().is_some_and(|j| !j.is_finished())
    }

    /// `(samples done, samples total)` of the picture shown.
    pub fn progress(&self) -> (u32, u32) {
        (
            self.samples,
            self.job.as_ref().map_or(self.samples, |j| j.total),
        )
    }

    /// Stops refining and keeps the picture.
    pub fn stop(&mut self) {
        if let Some(j) = &self.job {
            j.cancel();
        }
        self.settler.stop();
    }

    /// Throws the picture and the render away.
    pub fn clear(&mut self) {
        if let Some(j) = self.job.take() {
            j.cancel();
        }
        self.texture = None;
        self.samples = 0;
    }

    /// One frame. `key` is `None` unless a Final View may run now; `make`
    /// builds the render request when one is due.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        key: Option<u64>,
        make: impl FnOnce() -> Option<Request>,
    ) {
        let now = ctx.input(|i| i.time);
        let key = key.filter(|_| self.enabled);
        match self
            .settler
            .step(now, key, self.job.is_some() || self.texture.is_some())
        {
            Step::Idle | Step::Keep => {}
            Step::Wait => ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                self.settler.wait_left(now) + 0.02,
            )),
            Step::Cancel => self.clear(),
            Step::Start => match make() {
                Some(req) => {
                    self.clear();
                    self.job = Some(spawn(req));
                }
                None => self.settler.stop(),
            },
        }
        if let Some(job) = &self.job {
            if let Some(pass) = job.take_latest() {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [pass.width as usize, pass.height as usize],
                    &pass.rgba,
                );
                self.samples = pass.samples;
                match &mut self.texture {
                    Some(t) => t.set(image, egui::TextureOptions::LINEAR),
                    None => {
                        self.texture = Some(ctx.load_texture(
                            "final_view",
                            image,
                            egui::TextureOptions::LINEAR,
                        ))
                    }
                }
            }
            if job.is_finished() {
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(60));
            }
        }
    }

    /// Paints the picture over `rect` with its progress line and Stop
    /// button. Returns whether Stop was pressed.
    pub fn paint(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> bool {
        let Some(tex) = &self.texture else {
            return false;
        };
        ui.painter().image(
            tex.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        let (done, total) = self.progress();
        let running = self.is_running();
        let mut stop = false;
        egui::Area::new(egui::Id::new("final_view_bar"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.right_bottom() - egui::vec2(260.0, 40.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if running {
                            ui.label(format!("Final View {done} / {total} samples"));
                            stop = ui.button("Stop").clicked();
                        } else {
                            ui.label(format!("Final View, {done} samples"));
                        }
                    });
                });
            });
        if stop {
            self.stop();
        }
        stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_3d::{Material, Mesh, Vertex};

    #[test]
    fn the_render_waits_for_a_still_camera_then_starts_once() {
        let mut s = Settler::default();
        assert_eq!(s.step(0.0, Some(1), false), Step::Wait);
        assert_eq!(s.step(0.3, Some(1), false), Step::Wait);
        assert_eq!(s.step(0.6, Some(1), false), Step::Start);
        // Running or done, the picture stands.
        assert_eq!(s.step(0.7, Some(1), true), Step::Keep);
        assert_eq!(s.step(9.0, Some(1), true), Step::Keep);
    }

    #[test]
    fn moving_the_camera_drops_the_picture_and_starts_the_wait_again() {
        let mut s = Settler::default();
        s.step(0.0, Some(1), false);
        assert_eq!(s.step(1.0, Some(1), false), Step::Start);
        assert_eq!(s.step(1.1, Some(2), true), Step::Cancel);
        assert_eq!(s.step(1.2, Some(2), false), Step::Wait);
        assert_eq!(s.step(1.7, Some(2), false), Step::Start);
        // Quick successive changes never start.
        let mut s = Settler::default();
        for (i, t) in [0.0, 0.2, 0.4, 0.6, 0.8].into_iter().enumerate() {
            assert_ne!(s.step(t, Some(10 + i as u64), false), Step::Start);
        }
    }

    #[test]
    fn stop_keeps_the_picture_and_does_not_restart_it() {
        let mut s = Settler::default();
        s.step(0.0, Some(7), false);
        assert_eq!(s.step(1.0, Some(7), false), Step::Start);
        s.stop();
        assert_eq!(
            s.step(1.1, Some(7), true),
            Step::Keep,
            "the started render stands"
        );
        // After the picture is dropped (cleared) and the same view comes back
        // it is the same key: a stopped key stays idle.
        let mut s = Settler::default();
        s.step(0.0, Some(8), false);
        s.stop();
        assert_eq!(s.step(5.0, Some(8), false), Step::Idle);
        // A different view is a fresh start.
        assert_eq!(s.step(5.1, Some(9), false), Step::Wait);
    }

    #[test]
    fn no_key_means_no_final_view_and_cancels_what_runs() {
        let mut s = Settler::default();
        assert_eq!(s.step(0.0, None, false), Step::Idle);
        s.step(0.0, Some(1), false);
        assert_eq!(s.step(1.0, None, true), Step::Cancel);
        assert_eq!(
            s.step(1.1, Some(1), false),
            Step::Wait,
            "waits again from scratch"
        );
    }

    #[test]
    fn the_trace_size_keeps_the_shape_and_caps_the_width() {
        assert_eq!(trace_size((1920.0, 1080.0)), (960, 540));
        assert_eq!(trace_size((640.0, 480.0)), (640, 480));
        assert_eq!(trace_size((0.0, 0.0)), (16, 16));
    }

    #[test]
    fn the_sun_follows_the_plan_lighting_and_the_camera() {
        let mut l = Lighting::default();
        l.sun_azimuth_deg = 90.0; // due east
        l.sun_altitude_deg = 30.0;
        let sun = environment_for(&l, None)
            .sun
            .expect("a sun above the horizon");
        assert!(sun.direction[0] > 0.8 && sun.direction[1] > 0.4);
        let mut v = CameraView::default();
        v.sun_intensity = Some(0.0);
        assert!(
            environment_for(&l, Some(&v)).sun.is_none(),
            "the camera turned the sun off"
        );
        l.sun_altitude_deg = -5.0;
        assert!(environment_for(&l, None).sun.is_none(), "below the horizon");
        let half = {
            l.sun_altitude_deg = 45.0;
            v.sun_intensity = Some(0.5);
            environment_for(&l, Some(&v)).sun.unwrap().intensity
        };
        assert!(half < environment_for(&l, None).sun.unwrap().intensity);
    }

    /// A 20 in square floor under the camera.
    fn floor_scene() -> Scene {
        let h = 20.0_f32;
        let v = |x: f32, z: f32| Vertex {
            position: [x, 0.0, z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        };
        Scene {
            meshes: vec![Mesh {
                vertices: vec![v(-h, -h), v(h, -h), v(h, h), v(-h, h)],
                indices: vec![0, 2, 1, 0, 3, 2],
                material: Material::Floor,
                object_id: None,
                color: None,
            }],
        }
    }

    fn request(samples: u32, style: Option<Style>) -> Request {
        Request {
            scene: floor_scene(),
            camera: plan_render::Camera {
                eye: [0.0, 40.0, 0.0],
                target: [0.0, 0.0, 0.0],
                up: [0.0, 0.0, -1.0],
                fov_deg: 60.0,
                aperture: 0.0,
                focus_dist: 0.0,
            },
            env: environment_for(&Lighting::default(), None),
            lights: Vec::new(),
            settings: RenderSettings {
                width: 32,
                height: 24,
                samples,
                threads: 2,
                ..RenderSettings::default()
            },
            style,
        }
    }

    fn wait(job: &Shared) {
        for _ in 0..3000 {
            if job.is_finished() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the render did not finish");
    }

    #[test]
    fn the_worker_refines_to_the_sample_count_and_publishes_the_picture() {
        let job = spawn(request(4, None));
        wait(&job);
        assert_eq!(job.done(), 4);
        assert_eq!(job.total, 4);
        let pass = job.take_latest().expect("the last pass");
        assert_eq!((pass.width, pass.height, pass.samples), (32, 24, 4));
        assert_eq!(pass.rgba.len(), 32 * 24 * 4);
        assert!(job.take_latest().is_none(), "a pass is taken once");
    }

    #[test]
    fn stop_ends_the_render_early_and_keeps_the_picture_so_far() {
        let job = spawn(request(4096, None));
        // Let the first pass land, then press Stop.
        for _ in 0..3000 {
            if job.done() > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        job.cancel();
        wait(&job);
        assert!(job.done() < 4096, "stopped at {}", job.done());
        assert!(job.take_latest().is_some());
    }

    #[test]
    fn a_technique_look_changes_the_published_picture() {
        let plain = spawn(request(2, None));
        let line = spawn(request(2, Some(Style::LineDrawing)));
        wait(&plain);
        wait(&line);
        let (a, b) = (plain.take_latest().unwrap(), line.take_latest().unwrap());
        assert_ne!(a.rgba, b.rgba);
        // Line Drawing is white where nothing has an edge.
        assert!(b.rgba.chunks(4).filter(|p| p[0] == 255).count() > 100);
    }
}
