//! Camera dialogs (`docs/parity/3d-views-cameras.md`): the Camera
//! Specification (C-30) and the Ray Trace dialog (C-51, C-52, C-63).
//!
//! The Camera Specification edits a cloned [`CameraObject`] on the shared
//! dialog frame. Fields the model has no storage for (show in plan, shadows,
//! lock) are drawn disabled; the per-camera rendering technique is kept for
//! the session by the 3D panel ([`CameraExtras`]).
//!
//! [`RayTraceDialog`] is a small state machine around
//! `plan_render::Renderer::render_progressive` running on a background
//! thread: Idle -> Running -> Done / Cancelled / Failed. Everything except the
//! egui drawing is plain Rust and unit tested.

use super::{dis_check, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke};
use plan_3d::Scene;
use plan_core::camera::DEFAULT_CONE_LENGTH;
use plan_core::geometry::Point;
use plan_core::{CameraKind, CameraObject, Id};
use plan_materials::{RenderingTechnique, SunSettings};
use plan_render::{Environment, Image, RenderSettings, Renderer, Sun, Technique};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

const TABS: &[Tab] = &[
    Tab {
        name: "General",
        enabled: true,
    },
    Tab {
        name: "Options",
        enabled: true,
    },
    Tab {
        name: "Rendering",
        enabled: true,
    },
];

/// Smallest and largest angle of view Chief accepts (C-7).
pub const MIN_FOV_DEG: f64 = 5.0;
pub const MAX_FOV_DEG: f64 = 170.0;

/// Per-camera settings the model cannot store yet; kept per session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraExtras {
    pub technique: RenderingTechnique,
}

impl Default for CameraExtras {
    fn default() -> Self {
        Self {
            technique: RenderingTechnique::Standard,
        }
    }
}

/// The Camera Specification dialog (C-30).
pub struct CameraDialog {
    frame: SpecDialog,
    draft: CameraObject,
    extras: CameraExtras,
    floor_name: String,
    fields: Fields,
}

impl CameraDialog {
    pub fn new(camera: &CameraObject, floor_name: &str, extras: CameraExtras) -> Self {
        Self {
            frame: SpecDialog::new("Camera Specification", "camera"),
            draft: camera.clone(),
            extras,
            floor_name: floor_name.to_string(),
            fields: Fields::default(),
        }
    }

    pub fn id(&self) -> Id {
        self.draft.id
    }

    pub fn draft(&self) -> &CameraObject {
        &self.draft
    }

    pub fn extras(&self) -> CameraExtras {
        self.extras
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        // The frame borrows `self` as the page provider, so lift it out.
        let mut frame = std::mem::replace(&mut self.frame, SpecDialog::new("", "camera_tmp"));
        let outcome = frame.show(ctx, self);
        self.frame = frame;
        outcome
    }

    fn is_section(&self) -> bool {
        matches!(self.draft.kind, CameraKind::CrossSection { .. })
    }

    fn kind_label(&self) -> &'static str {
        match self.draft.kind {
            CameraKind::FullCamera => "Full Camera",
            CameraKind::PerspectiveOverview => "Perspective Overview",
            CameraKind::DollHouse => "Doll House View",
            CameraKind::CrossSection { back_clip: None } => "Cross Section/Elevation",
            CameraKind::CrossSection { .. } => "Back-Clipped Cross Section",
            CameraKind::WallElevation => "Wall Elevation",
            CameraKind::Orthographic => "Orthographic",
        }
    }

    fn general(&mut self, ui: &mut egui::Ui) {
        section(ui, "General");
        row(ui, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.draft.name).desired_width(200.0))
        });
        row(ui, "Camera Type", |ui| ui.label(self.kind_label()));
        row(ui, "Floor", |ui| ui.label(&self.floor_name));
        let section_cam = self.is_section();
        section(
            ui,
            if section_cam {
                "Cut Line"
            } else {
                "Camera Position"
            },
        );
        let f = &mut self.fields;
        let d = &mut self.draft;
        f.length_row(
            ui,
            if section_cam {
                "Center X"
            } else {
                "Position X"
            },
            "x",
            &mut d.position.x,
        );
        f.length_row(
            ui,
            if section_cam {
                "Center Y"
            } else {
                "Position Y"
            },
            "y",
            &mut d.position.y,
        );
        f.degrees_row(ui, "View Direction", "deg_dir", &mut d.direction_deg);
        if section_cam {
            f.length_row(ui, "Section Length", "width", &mut d.fov_deg);
        } else {
            f.length_row(ui, "Height Above Floor", "eye", &mut d.eye_height);
            f.degrees_row(ui, "Angle of View", "deg_fov", &mut d.fov_deg);
        }
    }

    fn options(&mut self, ui: &mut egui::Ui) {
        section(ui, "Clipping");
        let d = &mut self.draft;
        if let CameraKind::CrossSection { back_clip } = &mut d.kind {
            let mut limited = back_clip.is_some();
            if ui.checkbox(&mut limited, "Back-clip the section").changed() {
                *back_clip = limited.then_some(120.0);
            }
            if let Some(v) = back_clip {
                self.fields.length_row(ui, "Back Clip Distance", "back", v);
            }
        } else {
            let mut limited = d.clip_distance.is_some();
            if ui.checkbox(&mut limited, "Limit view distance").changed() {
                d.clip_distance = limited.then_some(DEFAULT_CONE_LENGTH);
            }
            if let Some(v) = &mut d.clip_distance {
                self.fields.length_row(ui, "Far Clip Distance", "clip", v);
            }
        }
        section(ui, "Display");
        dis_check(ui, "Show camera in plan", true);
        dis_check(ui, "Locked camera", false);
    }

    fn rendering(&mut self, ui: &mut egui::Ui) {
        section(ui, "Rendering");
        row(ui, "Technique", |ui| {
            egui::ComboBox::from_id_salt("camera_technique")
                .selected_text(self.extras.technique.label())
                .show_ui(ui, |ui| {
                    for t in RenderingTechnique::ALL {
                        ui.selectable_value(&mut self.extras.technique, t, t.label());
                    }
                });
        });
        dis_check(ui, "Cast shadows", true);
        ui.weak("The technique is kept for this session.");
    }
}

impl SpecPages for CameraDialog {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.draft.name.trim().is_empty() {
            return Some("Enter a camera name".into());
        }
        if !self.is_section() && !(MIN_FOV_DEG..=MAX_FOV_DEG).contains(&self.draft.fov_deg) {
            return Some(format!(
                "Angle of view must be {MIN_FOV_DEG:.0}\u{B0} to {MAX_FOV_DEG:.0}\u{B0}"
            ));
        }
        if self.fields.any_invalid() {
            return Some("Fix the highlighted fields".into());
        }
        None
    }

    fn page(&mut self, ui: &mut egui::Ui, tab: usize) {
        match tab {
            0 => self.general(ui),
            1 => self.options(ui),
            _ => self.rendering(ui),
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let ink = Stroke::new(1.2_f32, Color32::from_rgb(0x2F, 0x6C, 0xB3));
        let mut pts: Vec<Point> = if self.is_section() {
            let (a, b) = crate::tools::camera::section_line(&self.draft);
            vec![a, b, self.draft.position + self.draft.direction() * 36.0]
        } else {
            self.draft.symbol_points()
        };
        if pts.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (pts[0], pts[0]);
        for p in &pts {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let span = (hi.x - lo.x).max(hi.y - lo.y).max(1.0);
        let scale = f64::from(rect.width().min(rect.height())) / span * 0.9;
        let mid = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        let to_screen = |p: Point| {
            Pos2::new(
                rect.center().x + ((p.x - mid.x) * scale) as f32,
                rect.center().y - ((p.y - mid.y) * scale) as f32,
            )
        };
        let s: Vec<Pos2> = pts.drain(..).map(to_screen).collect();
        if self.is_section() {
            painter.line_segment([s[0], s[1]], ink);
            painter.line_segment([s[2], to_screen(self.draft.position)], ink);
        } else {
            painter.add(egui::Shape::convex_polygon(
                s[3..6].to_vec(),
                Color32::from_rgba_unmultiplied(0x2F, 0x6C, 0xB3, 40),
                ink,
            ));
            painter.add(egui::Shape::convex_polygon(
                s[0..3].to_vec(),
                Color32::from_rgb(0x2F, 0x6C, 0xB3),
                Stroke::NONE,
            ));
        }
        painter.text(
            rect.center_bottom(),
            Align2::CENTER_BOTTOM,
            &self.draft.name,
            egui::FontId::proportional(11.0),
            Color32::from_gray(0x2B),
        );
    }
}

// ----- ray tracing (C-51, C-52, C-63) -----

/// Image size presets of the Ray Trace dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SizePreset {
    P1280x960,
    P1920x1080,
}

impl SizePreset {
    pub const ALL: [SizePreset; 2] = [SizePreset::P1280x960, SizePreset::P1920x1080];

    pub fn dims(self) -> (u32, u32) {
        match self {
            SizePreset::P1280x960 => (1280, 960),
            SizePreset::P1920x1080 => (1920, 1080),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SizePreset::P1280x960 => "1280 x 960",
            SizePreset::P1920x1080 => "1920 x 1080",
        }
    }
}

/// Samples per pixel presets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SamplesPreset {
    S64,
    S256,
    S1024,
}

impl SamplesPreset {
    pub const ALL: [SamplesPreset; 3] = [
        SamplesPreset::S64,
        SamplesPreset::S256,
        SamplesPreset::S1024,
    ];

    pub fn count(self) -> u32 {
        match self {
            SamplesPreset::S64 => 64,
            SamplesPreset::S256 => 256,
            SamplesPreset::S1024 => 1024,
        }
    }
}

/// The two techniques the ray tracer offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RtTechnique {
    PhysicallyBased,
    Clay,
}

impl RtTechnique {
    pub const ALL: [RtTechnique; 2] = [RtTechnique::PhysicallyBased, RtTechnique::Clay];

    pub fn label(self) -> &'static str {
        match self {
            RtTechnique::PhysicallyBased => "Physically Based",
            RtTechnique::Clay => "Clay",
        }
    }

    fn render_technique(self) -> Technique {
        match self {
            RtTechnique::PhysicallyBased => Technique::PhysicallyBased,
            RtTechnique::Clay => Technique::Clay,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RtPhase {
    Idle,
    Running,
    Done,
    /// Stopped early; the image so far can still be saved.
    Cancelled,
    Failed(String),
}

/// A progressive image as display bytes.
#[derive(Clone, Debug)]
struct Preview {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

struct Shared {
    done: AtomicU32,
    total: u32,
    cancel: AtomicBool,
    latest: Mutex<Option<Preview>>,
    finished: Mutex<Option<Result<Image, String>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct RayTraceDialog {
    pub open: bool,
    pub size: SizePreset,
    pub samples: SamplesPreset,
    pub technique: RtTechnique,
    /// Sun date `(month, day)`, local solar time in hours and latitude (C-63).
    pub date: (u32, u32),
    pub time_hours: f64,
    pub latitude: f64,
    /// Replaces the size preset (tests render tiny images).
    pub size_override: Option<(u32, u32)>,
    pub message: String,
    phase: RtPhase,
    shared: Option<Arc<Shared>>,
    preview: Option<Preview>,
    preview_dirty: bool,
    image: Option<Image>,
    texture: Option<egui::TextureHandle>,
}

impl Default for RayTraceDialog {
    fn default() -> Self {
        Self {
            open: false,
            size: SizePreset::P1280x960,
            samples: SamplesPreset::S64,
            technique: RtTechnique::PhysicallyBased,
            // Mid-afternoon at the summer solstice in Metro Atlanta.
            date: (6, 21),
            time_hours: 15.0,
            latitude: 33.75,
            size_override: None,
            message: String::new(),
            phase: RtPhase::Idle,
            shared: None,
            preview: None,
            preview_dirty: false,
            image: None,
            texture: None,
        }
    }
}

impl RayTraceDialog {
    pub fn phase(&self) -> &RtPhase {
        &self.phase
    }

    pub fn is_running(&self) -> bool {
        self.phase == RtPhase::Running
    }

    /// The finished (or cancelled) image.
    pub fn image(&self) -> Option<&Image> {
        self.image.as_ref()
    }

    /// `(samples done, samples total)` of the current or last render.
    pub fn progress(&self) -> (u32, u32) {
        self.shared
            .as_ref()
            .map_or((0, 0), |s| (s.done.load(Ordering::Relaxed), s.total))
    }

    /// The sun for the chosen date, time and latitude.
    pub fn sun(&self) -> SunSettings {
        SunSettings::from_date_time_location(self.date, self.time_hours, self.latitude)
    }

    /// The render settings the dialog describes (always valid: sizes and
    /// sample counts come from the presets).
    pub fn settings(&self) -> RenderSettings {
        let (width, height) = self.size_override.unwrap_or_else(|| self.size.dims());
        RenderSettings {
            width: width.max(1),
            height: height.max(1),
            samples: self.samples.count(),
            technique: self.technique.render_technique(),
            ..RenderSettings::default()
        }
    }

    /// Sky plus the sun (none when it is below the horizon).
    pub fn environment(&self) -> Environment {
        let s = self.sun();
        let sun = (s.altitude_deg > 0.0).then(|| {
            let mut sun = Sun::from_azimuth_altitude(s.azimuth_deg as f32, s.altitude_deg as f32);
            sun.intensity *= s.intensity.clamp(0.1, 1.0);
            sun.color = s.color.map(|c| f32::from(c) / 255.0);
            sun
        });
        Environment {
            sun,
            ..Environment::default()
        }
    }

    /// Starts rendering `scene` from `camera` on a background thread.
    pub fn start(&mut self, scene: Scene, camera: plan_render::Camera) {
        if self.is_running() {
            return;
        }
        let settings = self.settings();
        let env = self.environment();
        let shared = Arc::new(Shared {
            done: AtomicU32::new(0),
            total: settings.samples,
            cancel: AtomicBool::new(false),
            latest: Mutex::new(None),
            finished: Mutex::new(None),
        });
        let worker = Arc::clone(&shared);
        let spawned = std::thread::Builder::new()
            .name("ray-trace".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    let renderer = Renderer::new(&scene);
                    let mut progress = |img: &Image, done: u32| {
                        worker.done.store(done, Ordering::Relaxed);
                        *lock(&worker.latest) = Some(Preview {
                            width: img.width,
                            height: img.height,
                            rgba: img.rgba.clone(),
                        });
                        !worker.cancel.load(Ordering::Relaxed)
                    };
                    renderer.render_progressive(&camera, &env, &[], &settings, &mut progress)
                }));
                *lock(&worker.finished) =
                    Some(result.map_err(|_| "The renderer stopped unexpectedly".to_string()));
            });
        match spawned {
            Ok(_) => {
                self.shared = Some(shared);
                self.phase = RtPhase::Running;
                self.image = None;
                self.preview = None;
                self.message.clear();
            }
            Err(e) => self.phase = RtPhase::Failed(format!("Could not start the renderer: {e}")),
        }
    }

    /// Asks a running render to stop after its current pass.
    pub fn cancel(&mut self) {
        if let Some(s) = &self.shared {
            s.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Moves what the worker produced into the dialog; call every frame.
    pub fn poll(&mut self) {
        let Some(shared) = self.shared.clone() else {
            return;
        };
        if let Some(p) = lock(&shared.latest).take() {
            self.preview = Some(p);
            self.preview_dirty = true;
        }
        let finished = lock(&shared.finished).take();
        match finished {
            Some(Ok(image)) => {
                self.preview = Some(Preview {
                    width: image.width,
                    height: image.height,
                    rgba: image.rgba.clone(),
                });
                self.preview_dirty = true;
                self.image = Some(image);
                self.phase = if shared.cancel.load(Ordering::Relaxed) {
                    RtPhase::Cancelled
                } else {
                    RtPhase::Done
                };
            }
            Some(Err(e)) => self.phase = RtPhase::Failed(e),
            None => {}
        }
    }

    /// Writes the image to `path` as a PNG.
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        match &self.image {
            Some(img) => std::fs::write(path, plan_render::encode_png(img)),
            None => Err(std::io::Error::other("nothing has been rendered yet")),
        }
    }

    /// Draws the dialog. `source` supplies the scene and camera when
    /// Render is pressed (`None` if there is nothing to render).
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        source: &mut dyn FnMut() -> Option<(Scene, plan_render::Camera)>,
    ) {
        self.poll();
        if !self.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Ray Trace")
            .id(egui::Id::new("ray_trace_dialog"))
            .open(&mut open)
            .collapsible(false)
            .default_width(460.0)
            .show(ctx, |ui| self.contents(ui, ctx, source));
        self.open = open;
        if self.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        source: &mut dyn FnMut() -> Option<(Scene, plan_render::Camera)>,
    ) {
        let idle = !self.is_running();
        ui.add_enabled_ui(idle, |ui| {
            egui::Grid::new("rt_grid").num_columns(2).show(ui, |ui| {
                ui.label("Image size");
                egui::ComboBox::from_id_salt("rt_size")
                    .selected_text(self.size.label())
                    .show_ui(ui, |ui| {
                        for s in SizePreset::ALL {
                            ui.selectable_value(&mut self.size, s, s.label());
                        }
                    });
                ui.end_row();
                ui.label("Samples per pixel");
                egui::ComboBox::from_id_salt("rt_samples")
                    .selected_text(self.samples.count().to_string())
                    .show_ui(ui, |ui| {
                        for s in SamplesPreset::ALL {
                            ui.selectable_value(&mut self.samples, s, s.count().to_string());
                        }
                    });
                ui.end_row();
                ui.label("Technique");
                egui::ComboBox::from_id_salt("rt_technique")
                    .selected_text(self.technique.label())
                    .show_ui(ui, |ui| {
                        for t in RtTechnique::ALL {
                            ui.selectable_value(&mut self.technique, t, t.label());
                        }
                    });
                ui.end_row();
                ui.label("Sun date");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.date.0)
                            .range(1..=12)
                            .prefix("month "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.date.1)
                            .range(1..=31)
                            .prefix("day "),
                    );
                });
                ui.end_row();
                ui.label("Sun time");
                ui.add(
                    egui::Slider::new(&mut self.time_hours, 0.0..=24.0)
                        .suffix(" h")
                        .fixed_decimals(1),
                );
                ui.end_row();
                ui.label("Latitude");
                ui.add(
                    egui::DragValue::new(&mut self.latitude)
                        .range(-66.0..=66.0)
                        .suffix("\u{B0}"),
                );
                ui.end_row();
            });
        });
        let sun = self.sun();
        ui.weak(format!(
            "Sun: azimuth {:.0}\u{B0}, altitude {:.0}\u{B0}",
            sun.azimuth_deg, sun.altitude_deg
        ));
        ui.separator();
        ui.horizontal(|ui| {
            if self.is_running() {
                if ui.button("Cancel").clicked() {
                    self.cancel();
                }
            } else if ui.button("Render").clicked() {
                match source() {
                    Some((scene, camera)) => self.start(scene, camera),
                    None => self.message = "There is nothing to render".into(),
                }
            }
            let can_save = self.image().is_some();
            if ui
                .add_enabled(can_save, egui::Button::new("Save PNG\u{2026}"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG image", &["png"])
                    .set_file_name("render.png")
                    .save_file()
                {
                    self.message = match self.save_png(&path) {
                        Ok(()) => format!("Saved {}", path.display()),
                        Err(e) => format!("Could not save: {e}"),
                    };
                }
            }
        });
        let (done, total) = self.progress();
        match self.phase() {
            RtPhase::Idle => {}
            RtPhase::Running => {
                let frac = if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                };
                ui.add(egui::ProgressBar::new(frac).text(format!("{done} / {total} samples")));
            }
            RtPhase::Done => {
                ui.label(format!("Finished: {total} samples"));
            }
            RtPhase::Cancelled => {
                ui.label(format!("Stopped after {done} of {total} samples"));
            }
            RtPhase::Failed(e) => {
                ui.colored_label(Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
            }
        }
        if !self.message.is_empty() {
            ui.weak(&self.message);
        }
        self.show_preview(ui, ctx);
    }

    fn show_preview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(p) = &self.preview else { return };
        if self.preview_dirty || self.texture.is_none() {
            let img = egui::ColorImage::from_rgba_unmultiplied(
                [p.width as usize, p.height as usize],
                &p.rgba,
            );
            match &mut self.texture {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => {
                    self.texture =
                        Some(ctx.load_texture("ray_trace", img, egui::TextureOptions::LINEAR));
                }
            }
            self.preview_dirty = false;
        }
        if let Some(t) = &self.texture {
            let w = ui.available_width().clamp(160.0, 640.0);
            let h = w * p.height as f32 / p.width.max(1) as f32;
            ui.image((t.id(), egui::vec2(w, h)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_3d::{Material, Mesh, Vertex};

    fn tiny_scene() -> Scene {
        let v = |x, y, z| Vertex {
            position: [x, y, z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        };
        Scene {
            meshes: vec![Mesh {
                vertices: vec![
                    v(-100.0, 0.0, -100.0),
                    v(100.0, 0.0, -100.0),
                    v(100.0, 0.0, 100.0),
                    v(-100.0, 0.0, 100.0),
                ],
                indices: vec![0, 2, 1, 0, 3, 2],
                material: Material::Floor,
                object_id: None,
            }],
        }
    }

    #[test]
    fn presets_build_valid_render_settings() {
        let mut d = RayTraceDialog::default();
        let s = d.settings();
        assert_eq!((s.width, s.height, s.samples), (1280, 960, 64));
        assert_eq!(s.technique, Technique::PhysicallyBased);
        d.size = SizePreset::P1920x1080;
        d.samples = SamplesPreset::S1024;
        d.technique = RtTechnique::Clay;
        let s = d.settings();
        assert_eq!((s.width, s.height, s.samples), (1920, 1080, 1024));
        assert_eq!(s.technique, Technique::Clay);
        for size in SizePreset::ALL {
            for samples in SamplesPreset::ALL {
                d.size = size;
                d.samples = samples;
                let s = d.settings();
                assert!(s.width > 0 && s.height > 0 && s.samples > 0 && s.max_bounces > 0);
            }
        }
    }

    #[test]
    fn sun_follows_the_date_and_time() {
        let mut d = RayTraceDialog::default();
        let noon = {
            d.time_hours = 12.0;
            d.environment().sun.expect("sun is up at noon")
        };
        assert!(noon.direction[1] > 0.5);
        d.time_hours = 0.0;
        assert!(d.environment().sun.is_none(), "no sun at midnight");
        // Summer afternoon: the sun is in the west half of the sky (scene +X is east).
        d.time_hours = 16.0;
        let pm = d.environment().sun.unwrap();
        assert!(pm.direction[0] < 0.0);
    }

    #[test]
    fn render_runs_to_done_on_a_background_thread() {
        let mut d = RayTraceDialog {
            size_override: Some((16, 12)),
            ..RayTraceDialog::default()
        };
        d.samples = SamplesPreset::S64;
        assert_eq!(d.phase(), &RtPhase::Idle);
        let cam = plan_render::Camera::from_plan(Point::new(0.0, -300.0), 90.0, 120.0, 60.0);
        d.start(tiny_scene(), cam);
        assert_eq!(d.phase(), &RtPhase::Running);
        let t0 = std::time::Instant::now();
        while d.is_running() && t0.elapsed().as_secs() < 60 {
            d.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(d.phase(), &RtPhase::Done);
        assert_eq!(d.progress(), (64, 64));
        let img = d.image().expect("image");
        assert_eq!((img.width, img.height), (16, 12));
        let dir = std::env::temp_dir().join(format!("plan_rt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.png");
        d.save_png(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cancel_stops_early_and_keeps_the_image() {
        let mut d = RayTraceDialog {
            size_override: Some((64, 48)),
            samples: SamplesPreset::S1024,
            ..RayTraceDialog::default()
        };
        let cam = plan_render::Camera::from_plan(Point::new(0.0, -300.0), 90.0, 120.0, 60.0);
        d.start(tiny_scene(), cam);
        d.cancel();
        let t0 = std::time::Instant::now();
        while d.is_running() && t0.elapsed().as_secs() < 60 {
            d.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(d.phase(), &RtPhase::Cancelled);
        assert!(d.progress().0 < 1024);
        assert!(d.image().is_some());
    }

    #[test]
    fn saving_without_an_image_fails() {
        let d = RayTraceDialog::default();
        assert!(d
            .save_png(std::path::Path::new("/nonexistent/x.png"))
            .is_err());
    }

    #[test]
    fn camera_dialog_validates_name_and_fov() {
        let cam = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "Camera 1", 0);
        let mut d = CameraDialog::new(&cam, "1st Floor", CameraExtras::default());
        assert!(d.error().is_none());
        d.draft.name = "  ".into();
        assert!(d.error().is_some());
        d.draft.name = "A".into();
        d.draft.fov_deg = 3.0;
        assert!(d.error().unwrap().contains("Angle of view"));
        d.draft.fov_deg = 90.0;
        assert!(d.error().is_none());
        assert_eq!(d.id(), cam.id);
    }
}
