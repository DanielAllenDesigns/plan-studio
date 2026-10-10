//! The 3D panel's round 15 additions: Undo Zoom, Refresh, Export 360
//! Panorama, the path-traced Final View and saving an orthographic view as a
//! camera. They are methods of [`View3dState`] kept apart from the 6000-line
//! panel file.

use super::{
    final_view, render_camera, technique_shows_textures, view_settings, View3dState, ViewScope,
};
use crate::dialogs::panorama::{PanoramaJob, PanoramaOutcome, PanoramaRequest};
use crate::editor::EditorContext;
use eframe::egui;
use plan_core::camera_view::{OrthoKind, OrthoView, ViewQuality};
use plan_view3d::{Camera, CameraMode};

/// The orthographic mode a saved view restores.
pub fn ortho_mode(kind: OrthoKind) -> CameraMode {
    match kind {
        OrthoKind::ElevationFront => CameraMode::ElevationFront,
        OrthoKind::ElevationBack => CameraMode::ElevationBack,
        OrthoKind::ElevationLeft => CameraMode::ElevationLeft,
        OrthoKind::ElevationRight => CameraMode::ElevationRight,
        OrthoKind::PlanOverhead => CameraMode::PlanOverhead,
    }
}

/// The saved-view kind of an orthographic mode; `None` for the perspective
/// ones.
pub fn ortho_kind(mode: CameraMode) -> Option<OrthoKind> {
    Some(match mode {
        CameraMode::ElevationFront => OrthoKind::ElevationFront,
        CameraMode::ElevationBack => OrthoKind::ElevationBack,
        CameraMode::ElevationLeft => OrthoKind::ElevationLeft,
        CameraMode::ElevationRight => OrthoKind::ElevationRight,
        CameraMode::PlanOverhead => OrthoKind::PlanOverhead,
        CameraMode::Orbit | CameraMode::DollHouse | CameraMode::FullCamera => return None,
    })
}

/// The saved form of the orthographic view `cam` shows.
pub fn ortho_view_of(cam: &Camera) -> Option<OrthoView> {
    Some(OrthoView {
        kind: ortho_kind(cam.mode)?,
        target: cam.target.map(f64::from),
        half_height: f64::from(cam.ortho_half_height),
    })
}

/// Aims `cam` at a saved orthographic view.
pub fn apply_ortho(cam: &mut Camera, view: &OrthoView) {
    cam.set_mode(ortho_mode(view.kind));
    cam.target = view.target.map(|v| v as f32);
    cam.ortho_half_height = (view.half_height as f32).max(1.0);
}

/// A key for everything the Final View picture depends on.
fn final_key(st: &View3dState, cam: &Camera, size: egui::Vec2, lighting_key: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let r = render_camera(cam);
    for v in r.eye.iter().chain(&r.target).chain(&[r.fov_deg]) {
        v.to_bits().hash(&mut h);
    }
    (size.x.round() as u32, size.y.round() as u32).hash(&mut h);
    st.last_project_hash.hash(&mut h);
    st.technique.label().hash(&mut h);
    st.textures_on.hash(&mut h);
    lighting_key.hash(&mut h);
    h.finish()
}

impl View3dState {
    /// Undo Zoom (C-42): back to the camera before the last zoom, pan or
    /// move.
    pub fn undo_zoom(&mut self, cx: &mut EditorContext) {
        let Some(vp) = self.viewport.as_mut().filter(|_| self.active) else {
            cx.status = "Open a 3D view, then use Undo Zoom".into();
            return;
        };
        match self.zoom.undo() {
            Some(cam) => {
                vp.camera = cam;
                self.mode = vp.camera.mode;
                self.final_view.clear();
                cx.status = "Undo Zoom".into();
            }
            None => cx.status = "Nothing to undo in the 3D view".into(),
        }
    }

    /// Refresh: redraws the view from the plan, including the vector
    /// drawing of an elevation.
    pub fn refresh(&mut self) {
        self.vector.invalidate();
        self.rebuild();
        self.final_view.clear();
    }

    /// The Final View of this frame: starts, refines or drops the
    /// path-traced picture and paints it over `rect`. Called after the live
    /// view is drawn.
    pub fn final_view_frame(&mut self, ui: &mut egui::Ui, cx: &EditorContext, rect: egui::Rect) {
        let Some(vp) = self.viewport.as_ref() else {
            return;
        };
        let cam = vp.camera.clone();
        let shown = self.active_camera.and_then(|id| cx.project.camera(id));
        let view = shown.map(|c| c.view.clone());
        let playing = self.walk.is_some_and(|w| w.playing);
        let allowed = self.final_view.enabled
            && self.quality == ViewQuality::Final
            && !cam.mode.is_orthographic()
            && !self.scene_empty
            && !playing
            && self.obj_drag.is_none()
            && cx.selection.is_empty();
        let lighting_key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            format!("{:?}", cx.project.lighting).hash(&mut h);
            view.as_ref()
                .map(|v| (v.light_set.clone(), v.sun_intensity.map(f64::to_bits)))
                .hash(&mut h);
            h.finish()
        };
        let key = allowed.then(|| final_key(self, &cam, rect.size(), lighting_key));
        let (technique, textures_on) = (self.technique, self.textures_on);
        let (scene, project) = (&self.base_scene, &cx.project);
        let make = || {
            let (render_technique, style) = view_settings::render_look(technique);
            let (width, height) = final_view::trace_size((rect.width(), rect.height()));
            Some(final_view::Request {
                scene: scene.clone(),
                camera: render_camera(&cam),
                env: final_view::environment_for(&project.lighting, view.as_ref()),
                lights: crate::dialogs::camera::render_lights_in(
                    project,
                    view.as_ref().and_then(|v| v.light_set.as_deref()),
                ),
                settings: plan_render::RenderSettings {
                    width,
                    height,
                    samples: final_view::FINAL_SAMPLES,
                    technique: render_technique,
                    denoise: true,
                    textures: textures_on && technique_shows_textures(technique),
                    ..plan_render::RenderSettings::default()
                },
                style,
            })
        };
        let ctx = ui.ctx().clone();
        self.final_view.update(&ctx, key, make);
        self.final_view.paint(ui, rect);
    }

    // ----- Export 360 Panorama -----

    /// 3D > Export > 360 Panorama: opens the dialog for the Full Camera view
    /// on screen.
    pub fn panorama_dialog(&mut self, cx: &mut EditorContext) {
        if self.panorama_job.is_some() {
            cx.status = "A panorama is already being rendered".into();
            return;
        }
        let in_full_camera = self.active
            && self
                .viewport
                .as_ref()
                .is_some_and(|v| v.camera.mode == CameraMode::FullCamera);
        if !in_full_camera {
            cx.status =
                "Open a Full Camera view (or a Floor Camera), then use Export 360 Panorama".into();
            return;
        }
        let name = self
            .active_camera
            .and_then(|id| cx.project.camera(id))
            .map_or_else(|| "Full Camera".to_string(), |c| c.name.clone());
        let path = default_panorama_path(&name);
        self.panorama_dialog = Some(crate::dialogs::panorama::PanoramaDialog::new(&name, path));
    }

    /// The panorama request for the Full Camera view on screen.
    pub fn panorama_request(
        &self,
        cx: &EditorContext,
        d: &crate::dialogs::panorama::PanoramaDialog,
    ) -> Option<PanoramaRequest> {
        let cam = &self.viewport.as_ref()?.camera;
        if cam.mode != CameraMode::FullCamera {
            return None;
        }
        let shown = self.active_camera.and_then(|id| cx.project.camera(id));
        let view = shown.map(|c| &c.view);
        let (render_technique, style) = view_settings::render_look(self.technique);
        let heading = (f64::from(cam.yaw) + std::f64::consts::FRAC_PI_2)
            .to_degrees()
            .rem_euclid(360.0);
        Some(PanoramaRequest {
            scene: view_scene_for_panorama(self, &cx.project),
            position: plan_core::Point::new(
                f64::from(cam.position[0]),
                -f64::from(cam.position[2]),
            ),
            eye_y: f64::from(cam.position[1]),
            heading_deg: heading,
            env: final_view::environment_for(&cx.project.lighting, view),
            lights: crate::dialogs::camera::render_lights_in(
                &cx.project,
                view.and_then(|v| v.light_set.as_deref()),
            ),
            style,
            technique: render_technique,
            width: d.picture_width(),
            samples: d.samples,
            png: d.png_path(),
            title: shown.map_or_else(|| "360 panorama".to_string(), |c| c.name.clone()),
        })
    }

    /// The Export 360 Panorama dialog and the progress window of its render.
    pub(super) fn panorama_windows(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if let Some(mut d) = self.panorama_dialog.take() {
            match d.show(ctx) {
                PanoramaOutcome::Open => self.panorama_dialog = Some(d),
                PanoramaOutcome::Cancel => {}
                PanoramaOutcome::Export => match self.panorama_request(cx, &d) {
                    Some(req) => {
                        let job = PanoramaJob::start(req);
                        cx.status = format!("Rendering the panorama to {}", job.png.display());
                        self.panorama_job = Some(job);
                    }
                    None => cx.status = "The Full Camera view is gone; open it again".into(),
                },
            }
        }
        let Some(job) = &self.panorama_job else {
            return;
        };
        if let Some(result) = job.finished() {
            cx.status = match result {
                Ok(files) => format!(
                    "Saved the panorama {} and its web page {}",
                    files.png.display(),
                    files.html.display()
                ),
                Err(e) => format!("Panorama failed: {e}"),
            };
            self.panorama_job = None;
            return;
        }
        let (done, total) = (job.done(), job.total);
        let mut stop = false;
        egui::Window::new("Export 360 Panorama")
            .id(egui::Id::new("export_panorama_progress"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let frac = if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                };
                ui.add(egui::ProgressBar::new(frac).text(format!("{done} / {total} samples")));
                ui.weak(format!("Writing {}", job.png.display()));
                stop = ui.button("Stop and save").clicked();
            });
        if stop {
            job.stop();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

/// The scene of the view on screen with its scope, pictures included.
fn view_scene_for_panorama(st: &View3dState, project: &plan_core::Project) -> plan_3d::Scene {
    super::build_view_scene(
        project,
        &ViewScope {
            no_images: false,
            ..st.scope()
        },
    )
}

/// The file Export 360 Panorama proposes: the camera's name in Documents.
pub fn default_panorama_path(camera_name: &str) -> String {
    let clean: String = camera_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let file = format!("{} panorama.png", clean.trim());
    crate::paths::home_dir()
        .map(|h| h.join("Documents").join(&file))
        .map_or(file.clone(), |p| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orthographic_modes_and_saved_kinds_map_both_ways() {
        for k in OrthoKind::ALL {
            assert_eq!(ortho_kind(ortho_mode(k)), Some(k));
            assert!(ortho_mode(k).is_orthographic());
        }
        for m in [
            CameraMode::Orbit,
            CameraMode::DollHouse,
            CameraMode::FullCamera,
        ] {
            assert_eq!(ortho_kind(m), None);
        }
    }

    #[test]
    fn an_orthographic_view_is_saved_and_restored_exactly() {
        let mut cam = Camera::default();
        cam.set_mode(CameraMode::ElevationLeft);
        cam.target = [120.0, 48.0, -60.0];
        cam.ortho_half_height = 333.0;
        let saved = ortho_view_of(&cam).expect("orthographic");
        assert_eq!(saved.kind, OrthoKind::ElevationLeft);
        assert_eq!(saved.half_height, 333.0);
        let mut other = Camera::default();
        apply_ortho(&mut other, &saved);
        assert_eq!(other.mode, CameraMode::ElevationLeft);
        assert_eq!(other.target, [120.0, 48.0, -60.0]);
        assert_eq!(other.ortho_half_height, 333.0);
        assert_eq!(other.yaw, cam.yaw);
        // A perspective camera has no orthographic form.
        assert!(ortho_view_of(&Camera::default()).is_none());
    }

    #[test]
    fn the_panorama_file_is_named_for_the_camera() {
        let p = default_panorama_path("Great Room / 2");
        assert!(p.ends_with("Great Room - 2 panorama.png"), "{p}");
    }
}

/// The plan's watermark laid over a view whose camera has Show Watermark on
/// (C-146): the text tiled `marks_per_row` by `marks_per_column`, turned by
/// the watermark's angle. A picture watermark is drawn on printed sheets
/// only.
pub fn paint_watermark(
    painter: &eframe::egui::Painter,
    rect: eframe::egui::Rect,
    spec: &plan_core::watermark::WatermarkSpec,
) {
    use eframe::egui;
    use plan_core::watermark::WatermarkKind;
    if spec.kind != WatermarkKind::Text || !spec.has_content() {
        return;
    }
    let alpha = (spec.alpha() * 255.0).round() as u8;
    let color = egui::Color32::from_rgba_unmultiplied(spec.color[0], spec.color[1], spec.color[2], alpha);
    let size = (rect.height() / 12.0).clamp(14.0, 90.0);
    let (cols, rows) = (spec.marks_per_row.max(1), spec.marks_per_column.max(1));
    let painter = painter.with_clip_rect(rect);
    for r in 0..rows {
        for c in 0..cols {
            let at = egui::pos2(
                rect.left() + rect.width() * (c as f32 + 0.5) / cols as f32,
                rect.top() + rect.height() * (r as f32 + 0.5) / rows as f32,
            );
            let galley =
                painter.layout_no_wrap(spec.text.clone(), egui::FontId::proportional(size), color);
            let half = galley.size() * 0.5;
            // Turn about the centre of the mark; the angle is counter-clockwise.
            let (sin, cos) = (-(spec.angle_deg as f32).to_radians()).sin_cos();
            let corner = at - egui::vec2(half.x * cos - half.y * sin, half.x * sin + half.y * cos);
            painter.add(
                egui::epaint::TextShape::new(corner, galley, color)
                    .with_angle(-(spec.angle_deg as f32).to_radians()),
            );
        }
    }
}
