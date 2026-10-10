//! Export 360 Panorama (3D > Export, parity C-77): renders the Full Camera's
//! view in every direction with the path tracer and saves an equirectangular
//! PNG plus a self-contained HTML viewer beside it.
//!
//! [`PanoramaDialog`] asks for the width, the samples and the file;
//! [`PanoramaJob`] renders on a worker thread (progressive, so Stop keeps the
//! best picture so far) and writes the files when it is done.

use super::{row, section};
use eframe::egui;
use plan_3d::Scene;
use plan_render::{
    panorama_camera, panorama_settings, Camera, Environment, PanoramaFiles, PointLight,
    RenderSettings, Renderer, Style, PANORAMA_WIDTHS,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Sample counts the dialog offers.
pub const SAMPLE_CHOICES: [u32; 4] = [8, 32, 128, 512];

/// What the dialog decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanoramaOutcome {
    Open,
    Cancel,
    Export,
}

/// The Export 360 Panorama dialog.
pub struct PanoramaDialog {
    /// Index into [`PANORAMA_WIDTHS`].
    pub width: usize,
    /// Samples per pixel.
    pub samples: u32,
    /// The PNG file; the HTML viewer is written beside it.
    pub path: String,
    /// The camera the panorama is taken from, for the caption.
    pub name: String,
}

impl PanoramaDialog {
    /// A dialog proposing `path` for the camera called `name`.
    pub fn new(name: &str, path: String) -> Self {
        Self {
            width: 1,
            samples: 32,
            path,
            name: name.to_string(),
        }
    }

    /// The picture width chosen, pixels (before the size limits).
    pub fn picture_width(&self) -> u32 {
        PANORAMA_WIDTHS[self.width.min(PANORAMA_WIDTHS.len() - 1)]
    }

    /// The PNG path: the typed name with `.png`.
    pub fn png_path(&self) -> PathBuf {
        Path::new(self.path.trim()).with_extension("png")
    }

    /// Can the export start (a file is named)?
    pub fn ready(&self) -> bool {
        !self.path.trim().is_empty()
    }

    pub fn show(&mut self, ctx: &egui::Context) -> PanoramaOutcome {
        let mut outcome = PanoramaOutcome::Open;
        let mut open = true;
        egui::Window::new("Export 360 Panorama")
            .id(egui::Id::new("export_panorama_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "From {}: the whole view around the camera, as a picture and a web page.",
                    self.name
                ));
                section(ui, "Picture");
                row(ui, "Width", |ui| {
                    egui::ComboBox::from_id_salt("panorama_width")
                        .selected_text(format!(
                            "{} x {}",
                            self.picture_width(),
                            self.picture_width() / 2
                        ))
                        .show_ui(ui, |ui| {
                            for (i, w) in PANORAMA_WIDTHS.iter().enumerate() {
                                ui.selectable_value(&mut self.width, i, format!("{w} x {}", w / 2));
                            }
                        })
                });
                row(ui, "Samples per pixel", |ui| {
                    egui::ComboBox::from_id_salt("panorama_samples")
                        .selected_text(self.samples.to_string())
                        .show_ui(ui, |ui| {
                            for n in SAMPLE_CHOICES {
                                ui.selectable_value(&mut self.samples, n, n.to_string());
                            }
                        })
                });
                section(ui, "File");
                ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .hint_text("panorama.png")
                        .desired_width(340.0),
                );
                if ui.button("Choose File\u{2026}").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title("Save the 360 panorama")
                        .add_filter("PNG picture", &["png"])
                        .set_file_name("panorama.png")
                        .save_file()
                    {
                        self.path = p.display().to_string();
                    }
                }
                ui.weak("A web page with the same name is written beside the PNG. Open it in any browser to look around.");
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.ready(), egui::Button::new("Export"))
                        .clicked()
                    {
                        outcome = PanoramaOutcome::Export;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = PanoramaOutcome::Cancel;
                    }
                });
            });
        if !open {
            outcome = PanoramaOutcome::Cancel;
        }
        // Enter exports and Escape cancels unless a field is being edited.
        if outcome == PanoramaOutcome::Open && ctx.memory(|m| m.focused()).is_none() {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && self.ready() {
                outcome = PanoramaOutcome::Export;
            } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                outcome = PanoramaOutcome::Cancel;
            }
        }
        outcome
    }
}

/// What a panorama job renders.
pub struct PanoramaRequest {
    pub scene: Scene,
    /// Plan position of the eye, inches (plan Y up the page).
    pub position: plan_core::Point,
    /// Eye height above the scene origin, inches.
    pub eye_y: f64,
    /// The view direction the picture's centre looks along, degrees
    /// counter-clockwise from plan +X.
    pub heading_deg: f64,
    pub env: Environment,
    pub lights: Vec<PointLight>,
    pub style: Option<Style>,
    pub technique: plan_render::Technique,
    pub width: u32,
    pub samples: u32,
    pub png: PathBuf,
    pub title: String,
}

/// The render settings and camera of a request.
pub fn plan_request(req: &PanoramaRequest) -> (Camera, RenderSettings) {
    let base = RenderSettings {
        technique: req.technique,
        denoise: true,
        ..RenderSettings::default()
    };
    (
        panorama_camera(req.position, req.heading_deg, req.eye_y),
        panorama_settings(&base, req.width, req.samples),
    )
}

/// A panorama rendering on a worker thread.
pub struct PanoramaJob {
    done: Arc<AtomicU32>,
    pub total: u32,
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<PanoramaFiles, String>>>>,
    pub png: PathBuf,
}

impl PanoramaJob {
    /// Starts rendering `req`.
    pub fn start(req: PanoramaRequest) -> Self {
        let (camera, settings) = plan_request(&req);
        let total = settings.samples;
        let done = Arc::new(AtomicU32::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let result = Arc::new(Mutex::new(None));
        let (d, c, r) = (Arc::clone(&done), Arc::clone(&cancel), Arc::clone(&result));
        let png = req.png.clone();
        let spawned = std::thread::Builder::new()
            .name("panorama".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let renderer = Renderer::new(&req.scene);
                    let image = renderer.render_progressive(
                        &camera,
                        &req.env,
                        &req.lights,
                        &settings,
                        &mut |_, samples| {
                            d.store(samples, Ordering::Relaxed);
                            !c.load(Ordering::Relaxed)
                        },
                    );
                    let image = match req.style {
                        Some(s) => plan_render::Image {
                            rgba: plan_render::stylize(&image.rgba, image.width, image.height, s),
                            ..image
                        },
                        None => image,
                    };
                    plan_render::write_panorama(&image, &req.png, &req.title)
                        .map_err(|e| e.to_string())
                }))
                .unwrap_or_else(|_| Err("The renderer stopped unexpectedly".into()));
                *r.lock().unwrap_or_else(PoisonError::into_inner) = Some(outcome);
            });
        if let Err(e) = spawned {
            *result.lock().unwrap_or_else(PoisonError::into_inner) =
                Some(Err(format!("Could not start the renderer: {e}")));
        }
        Self {
            done,
            total,
            cancel,
            result,
            png,
        }
    }

    /// Samples finished.
    pub fn done(&self) -> u32 {
        self.done.load(Ordering::Relaxed)
    }

    /// Stop; the picture so far is still saved.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The files once the worker has finished.
    pub fn finished(&self) -> Option<Result<PanoramaFiles, String>> {
        self.result
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_3d::{Material, Mesh, Vertex};
    use plan_core::Point;

    #[test]
    fn the_dialog_proposes_a_png_next_to_the_name_and_needs_a_file() {
        let mut d = PanoramaDialog::new("Great Room", "/tmp/room".into());
        assert_eq!(d.png_path(), Path::new("/tmp/room.png"));
        assert_eq!(d.picture_width(), 2048);
        assert!(d.ready());
        d.path = "  ".into();
        assert!(!d.ready());
        d.width = 99;
        assert_eq!(
            d.picture_width(),
            8192,
            "a stale choice is brought into range"
        );
        d.path = "/tmp/x.jpg".into();
        assert_eq!(d.png_path(), Path::new("/tmp/x.png"));
    }

    #[test]
    fn the_dialog_returns_its_outcome_without_a_window() {
        let ctx = egui::Context::default();
        let mut d = PanoramaDialog::new("Cam", "/tmp/p".into());
        let mut seen = PanoramaOutcome::Cancel;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            seen = d.show(ctx);
        });
        assert_eq!(seen, PanoramaOutcome::Open);
    }

    fn room() -> Scene {
        let h = 100.0_f32;
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            material: Material::WallInterior,
            object_id: None,
            color: None,
        };
        let mut quad = |c: [[f32; 3]; 4], n: [f32; 3]| {
            let base = mesh.vertices.len() as u32;
            for p in c {
                mesh.vertices.push(Vertex {
                    position: p,
                    normal: n,
                    uv: [0.0, 0.0],
                });
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };
        quad(
            [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]],
            [0.0, 1.0, 0.0],
        );
        quad(
            [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]],
            [0.0, -1.0, 0.0],
        );
        quad(
            [[-h, -h, -h], [-h, h, -h], [h, h, -h], [h, -h, -h]],
            [0.0, 0.0, 1.0],
        );
        quad(
            [[h, -h, h], [h, h, h], [-h, h, h], [-h, -h, h]],
            [0.0, 0.0, -1.0],
        );
        quad(
            [[-h, -h, h], [-h, h, h], [-h, h, -h], [-h, -h, -h]],
            [1.0, 0.0, 0.0],
        );
        quad(
            [[h, -h, -h], [h, h, -h], [h, h, h], [h, -h, h]],
            [-1.0, 0.0, 0.0],
        );
        Scene { meshes: vec![mesh] }
    }

    #[test]
    fn a_job_renders_and_writes_the_png_and_the_page() {
        let dir = std::env::temp_dir().join(format!("plan_pano_job_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let req = PanoramaRequest {
            scene: room(),
            position: Point::ZERO,
            eye_y: 0.0,
            heading_deg: 90.0,
            env: Environment::default(),
            lights: Vec::new(),
            style: None,
            technique: plan_render::Technique::Clay,
            width: 64,
            samples: 2,
            png: dir.join("pano.png"),
            title: "Test".into(),
        };
        let (cam, settings) = plan_request(&req);
        assert_eq!((settings.width, settings.height), (64, 32));
        assert_eq!(cam.eye, [0.0, 0.0, 0.0]);
        let job = PanoramaJob::start(req);
        assert_eq!(job.total, 2);
        let mut result = None;
        for _ in 0..3000 {
            result = job.finished();
            if result.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let files = result.expect("finished").expect("written");
        assert_eq!(job.done(), 2);
        assert!(std::fs::read(&files.png).unwrap().starts_with(b"\x89PNG"));
        let html = std::fs::read_to_string(&files.html).unwrap();
        assert!(html.contains("<title>Test</title>"));
        assert_eq!(files.html, dir.join("pano.html"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
