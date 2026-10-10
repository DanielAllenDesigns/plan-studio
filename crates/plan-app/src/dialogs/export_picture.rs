//! File > Export > Picture (PNG, JPEG, BMP, TIFF): the active view as a
//! picture file (coverage audit #7, L-49).
//!
//! The subject is any view: the floor plan (a layer set and a drawing
//! scale), an exterior elevation, an elevation or section camera's drawing,
//! or the live 3D view. Plan, elevation and section views are the layout
//! renderer's vector lines (`plan_layout::render_box_lines`) rasterised with
//! `plan_layout::rasterize_lines`; the 3D view is ray traced from the
//! viewport's camera (`plan_app::shell::view3d_panel::Snapshot3d`; the GL
//! viewport has no read-back, see the note on that type). The size is given
//! in pixels or as a paper size at a DPI; the background is white or
//! (PNG, TIFF, line views) transparent. The writers are in [`encode`].
//!
//! The window is opened by [`run_command`] (the shell passes what it knows
//! about the live 3D view) and drawn by [`show`] once a frame from
//! `exchange::show_all`.

mod encode;

pub use encode::{encode, Format};

use crate::editor::EditorContext;
use crate::shell::view3d_panel::{Snapshot3d, SNAPSHOT_MAX_PX, SNAPSHOT_SAMPLES};
use eframe::egui::{self, Align2};
use plan_core::geometry::Point;
use plan_core::{Id, Project};
use plan_docs::{Scale, SheetSize};
use plan_elevation::{LineWeight, ViewDir};
use plan_layout::{BoxSource, LayoutBox};
use std::cell::RefCell;
use std::path::Path;

/// Menu id of File > Export > Picture.
pub const EXPORT_PICTURE: &str = "file.export_picture";
/// Menu id of File > Import > 3D Symbol (opens the window of
/// `dialogs::symbol`).
pub const IMPORT_3D_SYMBOL: &str = "file.import_3d_symbol";

/// True for the two ids the shell hands to [`run_command`].
pub fn is_command(id: &str) -> bool {
    id == EXPORT_PICTURE || id == IMPORT_3D_SYMBOL
}

/// Longest side of any exported picture, pixels.
pub const MAX_SIDE_PX: u32 = 8000;
/// Most pixels in one picture.
pub const MAX_PIXELS: u64 = 36_000_000;
/// Margin around a picture sized by paper, inches.
const PAPER_MARGIN_IN: f64 = 0.25;

/// What the shell knows about the live views when the command runs.
#[derive(Clone, Default)]
pub struct ViewInfo {
    /// The 3D view's scene and camera, when it has a model.
    pub snapshot_3d: Option<Snapshot3d>,
    /// The 3D view is the one on screen.
    pub view_3d_active: bool,
}

/// What a picture shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Subject {
    /// The floor plan.
    Plan,
    /// An exterior elevation of the model.
    Elevation(ViewDir),
    /// The drawing of an elevation or section camera.
    Camera(Id),
    /// The live 3D view.
    ThreeD,
}

/// How the picture's size is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeBy {
    /// Width (and height) in pixels.
    Pixels,
    /// A paper size at a resolution.
    Paper,
}

/// Everything the Export Picture window collects.
#[derive(Clone, Debug)]
pub struct PictureOptions {
    pub subject: Subject,
    pub format: Format,
    pub size_by: SizeBy,
    pub width_px: u32,
    /// Pixels: the height when `auto_height` is off.
    pub height_px: u32,
    /// Pixels: the height follows the view's shape.
    pub auto_height: bool,
    pub paper: SheetSize,
    pub landscape: bool,
    pub dpi: u32,
    /// Line views, PNG and TIFF only: no white background.
    pub transparent: bool,
    pub floor: usize,
    pub layer_set: String,
    /// Drawing scale of a line view on paper; `None` fits the view.
    pub scale: Option<Scale>,
    /// Ray-trace samples per pixel of the 3D view.
    pub samples: u32,
    /// JPEG quality, 1..=100.
    pub quality: u8,
}

impl Default for PictureOptions {
    fn default() -> Self {
        PictureOptions {
            subject: Subject::Plan,
            format: Format::Png,
            size_by: SizeBy::Pixels,
            width_px: 2400,
            height_px: 1800,
            auto_height: true,
            paper: SheetSize::Letter,
            landscape: true,
            dpi: 300,
            transparent: false,
            floor: 0,
            layer_set: "All".into(),
            scale: None,
            samples: SNAPSHOT_SAMPLES,
            quality: 90,
        }
    }
}

/// A finished picture: straight RGBA8, rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl PictureOptions {
    /// Paper size in inches for the chosen orientation.
    pub fn paper_inches(&self) -> (f64, f64) {
        let (long, short) = self.paper.inches();
        let (long, short) = (long.max(short), long.min(short));
        if self.landscape {
            (long, short)
        } else {
            (short, long)
        }
    }

    /// The canvas in pixels: `None` for the height when it follows the view.
    fn canvas(&self) -> (u32, Option<u32>) {
        match self.size_by {
            SizeBy::Pixels => (
                self.width_px,
                (!self.auto_height || self.subject == Subject::ThreeD).then_some(
                    if self.auto_height {
                        // 3D has no shape of its own: the viewport's 4:3.
                        (f64::from(self.width_px) * 0.75).round() as u32
                    } else {
                        self.height_px
                    },
                ),
            ),
            SizeBy::Paper => {
                let (w, h) = self.paper_inches();
                (
                    (w * f64::from(self.dpi)).round() as u32,
                    Some((h * f64::from(self.dpi)).round() as u32),
                )
            }
        }
    }

    /// The size the picture will have when it is known up front (the height
    /// of a line view in pixels mode follows the drawing).
    pub fn known_size(&self) -> (u32, Option<u32>) {
        self.canvas()
    }

    /// Checks the options; the error text is what the window shows.
    pub fn check(&self) -> Result<(), String> {
        let (w, h) = self.canvas();
        let side_ok = |v: u32| (16..=MAX_SIDE_PX).contains(&v);
        if !side_ok(w) || h.is_some_and(|h| !side_ok(h)) {
            return Err(format!(
                "The picture must be 16 to {MAX_SIDE_PX} pixels each way"
            ));
        }
        if u64::from(w) * u64::from(h.unwrap_or(w)) > MAX_PIXELS {
            return Err("The picture is too large; lower the size or the DPI".into());
        }
        if self.subject == Subject::ThreeD
            && (w > SNAPSHOT_MAX_PX || h.is_some_and(|h| h > SNAPSHOT_MAX_PX))
        {
            return Err(format!(
                "A 3D picture can be at most {SNAPSHOT_MAX_PX} pixels each way"
            ));
        }
        if self.size_by == SizeBy::Paper && !(36..=1200).contains(&self.dpi) {
            return Err("The resolution must be 36 to 1200 DPI".into());
        }
        Ok(())
    }
}

/// Renders the picture the options describe. `snap` is the 3D view for
/// [`Subject::ThreeD`].
pub fn render_picture(
    project: &Project,
    o: &PictureOptions,
    snap: Option<&Snapshot3d>,
) -> Result<Picture, String> {
    o.check()?;
    let (w, h) = o.canvas();
    match &o.subject {
        Subject::ThreeD => {
            let snap = snap.ok_or("The 3D view has no model to export")?;
            if snap.scene.meshes.is_empty() {
                return Err("The 3D view has no model to export".into());
            }
            let h = h.unwrap_or_else(|| (f64::from(w) * 0.75).round() as u32);
            let settings = plan_render::RenderSettings {
                width: w,
                height: h,
                samples: o.samples.clamp(1, 512),
                ..plan_render::RenderSettings::default()
            };
            let image = plan_render::Renderer::new(&snap.scene).render(
                &snap.camera,
                &plan_render::Environment::default(),
                &[],
                &settings,
            );
            Ok(Picture {
                width: image.width,
                height: image.height,
                rgba: image.rgba,
            })
        }
        subject => {
            let source = match subject {
                Subject::Plan => BoxSource::PlanView {
                    floor: o.floor.min(project.floors.len().saturating_sub(1)),
                    layer_set: o.layer_set.clone(),
                },
                Subject::Elevation(dir) => BoxSource::Elevation { dir: *dir },
                Subject::Camera(id) => {
                    if project.camera(*id).is_none() {
                        return Err("That camera no longer exists".into());
                    }
                    BoxSource::Camera { camera_id: *id }
                }
                Subject::ThreeD => unreachable!("handled above"),
            };
            render_lines(project, o, source, w, h)
        }
    }
}

/// A line view (plan, elevation, section) on a canvas.
fn render_lines(
    project: &Project,
    o: &PictureOptions,
    source: BoxSource,
    canvas_w: u32,
    canvas_h: Option<u32>,
) -> Result<Picture, String> {
    let rcx = crate::shell::layout_window::render_context(project);
    let fixed_scale = (o.size_by == SizeBy::Paper).then_some(o.scale).flatten();
    let scale = fixed_scale.unwrap_or(Scale::QuarterInch);
    let (w_in, h_in) = plan_layout::source_size_in(&source, scale, &rcx);
    let mut b = LayoutBox::new(
        1,
        (Point::new(0.0, 0.0), Point::new(w_in, h_in)),
        source,
        scale,
    );
    b.border = false;
    b.clip = false;
    let lines = plan_layout::render_box_lines(&b, &rcx);
    if lines.is_empty() || w_in <= 0.0 || h_in <= 0.0 {
        return Err("There is nothing to draw in this view".into());
    }

    let paper = o.size_by == SizeBy::Paper;
    let margin = if paper {
        (PAPER_MARGIN_IN * f64::from(o.dpi)).round()
    } else {
        0.0
    };
    // Pixels per drawing inch, and the canvas.
    let (cw, ch, k) = match (canvas_h, fixed_scale) {
        // True scale on paper: one paper inch is `dpi` pixels.
        (Some(ch), Some(_)) => {
            let k = f64::from(o.dpi);
            if w_in * k > f64::from(canvas_w) - 2.0 * margin
                || h_in * k > f64::from(ch) - 2.0 * margin
            {
                return Err(format!(
                    "At {} the view is {:.1} x {:.1} in, which does not fit the paper; choose a smaller scale or Fit",
                    scale.label(),
                    w_in,
                    h_in
                ));
            }
            (canvas_w, ch, k)
        }
        (Some(ch), None) => {
            let k = ((f64::from(canvas_w) - 2.0 * margin) / w_in)
                .min((f64::from(ch) - 2.0 * margin) / h_in);
            (canvas_w, ch, k)
        }
        // Height follows the view.
        (None, _) => {
            let k = f64::from(canvas_w) / w_in;
            let ch = (h_in * k).round().max(16.0) as u32;
            if ch > MAX_SIDE_PX || u64::from(canvas_w) * u64::from(ch) > MAX_PIXELS {
                return Err("The picture would be too tall; lower the width".into());
            }
            (canvas_w, ch, k)
        }
    };
    if k <= 0.0 {
        return Err("The paper is too small for the margins".into());
    }
    // Pen weights: 0.25 mm thin lines on paper at the DPI; in pixels mode a
    // 1700 px wide picture has 1 px lines.
    let thin = if paper {
        (0.0098 * f64::from(o.dpi)).max(1.0)
    } else {
        (f64::from(cw) / 1700.0).clamp(1.0, 6.0)
    };
    type Seg = ((f64, f64), (f64, f64), f64);
    let raster: Vec<Seg> = lines
        .iter()
        .map(|l| {
            let px = match l.weight {
                LineWeight::Heavy => 2.0 * thin,
                _ => thin,
            };
            ((l.a.x, l.a.y), (l.b.x, l.b.y), px)
        })
        .collect();
    let content_w = ((w_in * k).round() as u32).clamp(16, MAX_SIDE_PX);
    let (iw, ih, gray) = plan_layout::rasterize_lines(&raster, (0.0, 0.0, w_in, h_in), content_w);

    let transparent = o.transparent && o.format.has_alpha();
    let bg: [u8; 4] = if transparent {
        [0, 0, 0, 0]
    } else {
        [255, 255, 255, 255]
    };
    let mut rgba = Vec::with_capacity(cw as usize * ch as usize * 4);
    for _ in 0..(cw as usize * ch as usize) {
        rgba.extend_from_slice(&bg);
    }
    let (ox, oy) = (
        (i64::from(cw) - i64::from(iw)) / 2,
        (i64::from(ch) - i64::from(ih)) / 2,
    );
    for y in 0..ih {
        let dy = i64::from(y) + oy;
        if dy < 0 || dy >= i64::from(ch) {
            continue;
        }
        for x in 0..iw {
            let dx = i64::from(x) + ox;
            if dx < 0 || dx >= i64::from(cw) {
                continue;
            }
            let v = gray[(y as usize * iw as usize + x as usize) * 4];
            let at = (dy as usize * cw as usize + dx as usize) * 4;
            if transparent {
                rgba[at..at + 4].copy_from_slice(&[0, 0, 0, 255 - v]);
            } else {
                rgba[at..at + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
    }
    Ok(Picture {
        width: cw,
        height: ch,
        rgba,
    })
}

/// The finished file's bytes with its size in pixels.
pub fn picture_bytes(
    project: &Project,
    o: &PictureOptions,
    snap: Option<&Snapshot3d>,
) -> Result<(Vec<u8>, u32, u32), String> {
    let p = render_picture(project, o, snap)?;
    let bytes = encode(o.format, p.width, p.height, &p.rgba, o.quality);
    Ok((bytes, p.width, p.height))
}

/// Renders and writes the picture to `path`; the status text.
#[cfg_attr(not(test), allow(dead_code))]
pub fn export_to_path(
    project: &Project,
    o: &PictureOptions,
    snap: Option<&Snapshot3d>,
    path: &Path,
) -> Result<String, String> {
    let (bytes, w, h) = picture_bytes(project, o, snap)?;
    std::fs::write(path, &bytes).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    Ok(format!(
        "Saved {} ({} x {} px {})",
        path.display(),
        w,
        h,
        o.format.label()
    ))
}

thread_local! {
    /// Under test the save dialog is skipped and the bytes land here.
    static LAST_EXPORT: RefCell<Option<(Vec<u8>, u32, u32)>> = const { RefCell::new(None) };
}

/// The bytes of the last picture "saved" while testing.
#[cfg(test)]
pub fn last_export() -> Option<(Vec<u8>, u32, u32)> {
    LAST_EXPORT.with(|l| l.borrow().clone())
}

/// A name for the file: the project, the subject and the extension.
pub fn default_file_name(project: &Project, o: &PictureOptions) -> String {
    let what = match &o.subject {
        Subject::Plan => project
            .floors
            .get(o.floor)
            .map_or_else(|| "Plan".to_string(), |f| f.name.clone()),
        Subject::Elevation(d) => format!("{d:?} Elevation"),
        Subject::Camera(id) => project
            .camera(*id)
            .map_or_else(|| "Camera".to_string(), |c| c.name.clone()),
        Subject::ThreeD => "3D View".into(),
    };
    format!("{} - {}.{}", project.name, what, o.format.extension())
}

// ===================================================================
// The window
// ===================================================================

struct ExportWindow {
    o: PictureOptions,
    snap: Option<Snapshot3d>,
    cameras: Vec<(Id, String)>,
    floors: Vec<String>,
    layer_sets: Vec<String>,
}

thread_local! {
    static WINDOW: RefCell<Option<ExportWindow>> = const { RefCell::new(None) };
}

/// Runs a command id of [`is_command`].
pub fn run_command(cx: &mut EditorContext, id: &str, views: ViewInfo) {
    match id {
        EXPORT_PICTURE => open(cx, views),
        IMPORT_3D_SYMBOL => super::symbol::start_import(cx),
        _ => {}
    }
}

/// Opens the Export Picture window.
pub fn open(cx: &mut EditorContext, views: ViewInfo) {
    let mut o = PictureOptions {
        floor: cx.floor.min(cx.project.floors.len().saturating_sub(1)),
        layer_set: cx.project.layer_sets.active.clone(),
        ..PictureOptions::default()
    };
    if views.view_3d_active && views.snapshot_3d.is_some() {
        o.subject = Subject::ThreeD;
    }
    let window = ExportWindow {
        o,
        snap: views.snapshot_3d,
        cameras: cx
            .project
            .cameras
            .iter()
            .filter(|c| crate::dialogs::camera::is_elevation_camera(c))
            .map(|c| (c.id, c.name.clone()))
            .collect(),
        floors: cx.project.floors.iter().map(|f| f.name.clone()).collect(),
        layer_sets: std::iter::once("All".to_string())
            .chain(cx.project.layer_sets.sets.iter().map(|s| s.name.clone()))
            .collect(),
    };
    WINDOW.with(|w| *w.borrow_mut() = Some(window));
}

/// True while the window is open.
#[cfg_attr(not(test), allow(dead_code))]
pub fn is_open() -> bool {
    WINDOW.with(|w| w.borrow().is_some())
}

/// Closes the window.
#[cfg_attr(not(test), allow(dead_code))]
pub fn close() {
    WINDOW.with(|w| *w.borrow_mut() = None);
}

/// Runs `f` on the open window's options (the shell and the tests).
#[cfg_attr(not(test), allow(dead_code))]
pub fn with_options<R>(f: impl FnOnce(&mut PictureOptions) -> R) -> Option<R> {
    WINDOW.with(|w| w.borrow_mut().as_mut().map(|w| f(&mut w.o)))
}

/// Exports with the open window's options (as its Export button does) and
/// closes it on success; false when there is no window or the export failed.
#[cfg_attr(not(test), allow(dead_code))]
pub fn accept(cx: &mut EditorContext) -> bool {
    let Some(w) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return false;
    };
    if finish(cx, &w) {
        return true;
    }
    WINDOW.with(|slot| *slot.borrow_mut() = Some(w));
    false
}

/// Draws the window; call once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut w) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return;
    };
    let keep = window(ctx, cx, &mut w);
    if keep {
        // A command run meanwhile may have opened a new one.
        WINDOW.with(|slot| {
            let mut s = slot.borrow_mut();
            if s.is_none() {
                *s = Some(w);
            }
        });
    }
}

fn subject_label(o: &ExportWindow, s: &Subject) -> String {
    match s {
        Subject::Plan => "Floor Plan".into(),
        Subject::Elevation(d) => format!("{d:?} Elevation"),
        Subject::Camera(id) => o
            .cameras
            .iter()
            .find(|(c, _)| c == id)
            .map_or_else(|| "Camera".into(), |(_, n)| n.clone()),
        Subject::ThreeD => "3D View".into(),
    }
}

/// Accepted: asks where to save (or records the bytes when testing).
fn finish(cx: &mut EditorContext, w: &ExportWindow) -> bool {
    let o = &w.o;
    let (bytes, pw, ph) = match picture_bytes(&cx.project, o, w.snap.as_ref()) {
        Ok(r) => r,
        Err(e) => {
            cx.status = format!("Export Picture: {e}");
            return false;
        }
    };
    if cfg!(test) {
        LAST_EXPORT.with(|l| *l.borrow_mut() = Some((bytes.clone(), pw, ph)));
        cx.status = format!(
            "Exported {} bytes of {} ({pw} x {ph} px)",
            bytes.len(),
            o.format.label()
        );
        return true;
    }
    let name = default_file_name(&cx.project, o);
    let Some(path) = rfd::FileDialog::new()
        .set_title("Export Picture")
        .set_file_name(name)
        .add_filter(o.format.label(), &[o.format.extension()])
        .save_file()
    else {
        cx.status = "Export Picture cancelled".into();
        return false;
    };
    cx.status = match std::fs::write(&path, &bytes) {
        Ok(()) => format!(
            "Saved {} ({pw} x {ph} px {})",
            path.display(),
            o.format.label()
        ),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    };
    true
}

fn window(ctx: &egui::Context, cx: &mut EditorContext, w: &mut ExportWindow) -> bool {
    let mut open = true;
    let mut go = false;
    let mut cancel = false;
    let error = w.o.check().err();
    let three_d_ok = w.snap.is_some();
    egui::Window::new("Export Picture")
        .id(egui::Id::new("export_picture"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            egui::Grid::new("export_picture_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("View");
                    let shown = subject_label(w, &w.o.subject);
                    egui::ComboBox::from_id_salt("ep_view")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut w.o.subject, Subject::Plan, "Floor Plan");
                            for d in [ViewDir::Front, ViewDir::Back, ViewDir::Left, ViewDir::Right]
                            {
                                ui.selectable_value(
                                    &mut w.o.subject,
                                    Subject::Elevation(d),
                                    format!("{d:?} Elevation"),
                                );
                            }
                            for (id, name) in &w.cameras {
                                ui.selectable_value(
                                    &mut w.o.subject,
                                    Subject::Camera(*id),
                                    name.as_str(),
                                );
                            }
                            ui.add_enabled_ui(three_d_ok, |ui| {
                                ui.selectable_value(&mut w.o.subject, Subject::ThreeD, "3D View");
                            });
                        });
                    ui.end_row();
                    if w.o.subject == Subject::Plan {
                        ui.label("Floor");
                        egui::ComboBox::from_id_salt("ep_floor")
                            .selected_text(
                                w.floors
                                    .get(w.o.floor)
                                    .cloned()
                                    .unwrap_or_else(|| "-".into()),
                            )
                            .show_ui(ui, |ui| {
                                for (i, n) in w.floors.iter().enumerate() {
                                    ui.selectable_value(&mut w.o.floor, i, n.as_str());
                                }
                            });
                        ui.end_row();
                        ui.label("Layer set");
                        egui::ComboBox::from_id_salt("ep_layers")
                            .selected_text(w.o.layer_set.clone())
                            .show_ui(ui, |ui| {
                                for n in &w.layer_sets {
                                    ui.selectable_value(&mut w.o.layer_set, n.clone(), n.as_str());
                                }
                            });
                        ui.end_row();
                    }
                    ui.label("Size");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut w.o.size_by, SizeBy::Pixels, "Pixels");
                        ui.radio_value(&mut w.o.size_by, SizeBy::Paper, "Paper size and DPI");
                    });
                    ui.end_row();
                    match w.o.size_by {
                        SizeBy::Pixels => {
                            ui.label("Width");
                            ui.add(
                                egui::DragValue::new(&mut w.o.width_px)
                                    .range(16..=MAX_SIDE_PX)
                                    .suffix(" px"),
                            );
                            ui.end_row();
                            ui.label("Height");
                            ui.horizontal(|ui| {
                                ui.add_enabled(
                                    !w.o.auto_height,
                                    egui::DragValue::new(&mut w.o.height_px)
                                        .range(16..=MAX_SIDE_PX)
                                        .suffix(" px"),
                                );
                                ui.checkbox(&mut w.o.auto_height, "Match the view's shape");
                            });
                            ui.end_row();
                        }
                        SizeBy::Paper => {
                            ui.label("Paper");
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt("ep_paper")
                                    .selected_text(w.o.paper.label())
                                    .show_ui(ui, |ui| {
                                        for s in SheetSize::ALL {
                                            ui.selectable_value(&mut w.o.paper, s, s.label());
                                        }
                                    });
                                ui.radio_value(&mut w.o.landscape, true, "Landscape");
                                ui.radio_value(&mut w.o.landscape, false, "Portrait");
                            });
                            ui.end_row();
                            ui.label("Resolution");
                            egui::ComboBox::from_id_salt("ep_dpi")
                                .selected_text(format!("{} DPI", w.o.dpi))
                                .show_ui(ui, |ui| {
                                    for d in [72, 100, 150, 200, 300, 600] {
                                        ui.selectable_value(&mut w.o.dpi, d, format!("{d} DPI"));
                                    }
                                });
                            ui.end_row();
                            if w.o.subject != Subject::ThreeD {
                                ui.label("Scale");
                                egui::ComboBox::from_id_salt("ep_scale")
                                    .selected_text(
                                        w.o.scale.map_or("Fit to the paper", |s| s.label()),
                                    )
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut w.o.scale,
                                            None,
                                            "Fit to the paper",
                                        );
                                        for s in Scale::choices() {
                                            ui.selectable_value(&mut w.o.scale, Some(s), s.label());
                                        }
                                    });
                                ui.end_row();
                            }
                        }
                    }
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("ep_format")
                        .selected_text(w.o.format.label())
                        .show_ui(ui, |ui| {
                            for f in Format::ALL {
                                ui.selectable_value(&mut w.o.format, f, f.label());
                            }
                        });
                    ui.end_row();
                    if w.o.format == Format::Jpeg {
                        ui.label("JPEG quality");
                        ui.add(egui::Slider::new(&mut w.o.quality, 10..=100));
                        ui.end_row();
                    }
                    if w.o.subject == Subject::ThreeD {
                        ui.label("Quality");
                        ui.add(
                            egui::DragValue::new(&mut w.o.samples)
                                .range(1..=512)
                                .suffix(" samples"),
                        );
                        ui.end_row();
                    } else {
                        ui.label("Background");
                        ui.add_enabled_ui(w.o.format.has_alpha(), |ui| {
                            ui.checkbox(&mut w.o.transparent, "Transparent");
                        });
                        ui.end_row();
                    }
                });
            ui.separator();
            let (pw, ph) = w.o.known_size();
            ui.weak(match (w.o.size_by, ph) {
                (SizeBy::Paper, Some(ph)) => {
                    let (a, b) = w.o.paper_inches();
                    format!("{pw} x {ph} px ({a:.1} x {b:.1} in at {} DPI)", w.o.dpi)
                }
                (_, Some(ph)) => format!("{pw} x {ph} px"),
                (_, None) => format!("{pw} px wide; the height follows the view"),
            });
            if w.o.subject == Subject::ThreeD {
                ui.weak("Ray traced from the 3D view's camera with the default sun and sky.");
            }
            if let Some(e) = &error {
                ui.colored_label(egui::Color32::from_rgb(0xB0, 0x30, 0x30), e);
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(error.is_none(), egui::Button::new("Export\u{2026}"))
                    .clicked()
                {
                    go = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if go && finish(cx, w) {
        return false;
    }
    open && !cancel
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::WallKind;

    fn project_with_walls() -> Project {
        let mut p = Project::new("Export Test");
        for (a, b) in [
            ((0.0, 0.0), (240.0, 0.0)),
            ((240.0, 0.0), (240.0, 180.0)),
            ((240.0, 180.0), (0.0, 180.0)),
            ((0.0, 180.0), (0.0, 0.0)),
        ] {
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        p
    }

    #[test]
    fn size_checks_and_paper_math() {
        let mut o = PictureOptions::default();
        assert!(o.check().is_ok());
        assert_eq!(o.known_size(), (2400, None));
        o.width_px = 8;
        assert!(o.check().is_err());
        o.width_px = 9000;
        assert!(o.check().is_err());
        o.width_px = 2400;
        o.auto_height = false;
        o.height_px = 1800;
        assert_eq!(o.known_size(), (2400, Some(1800)));
        // Letter landscape at 300 DPI.
        o.size_by = SizeBy::Paper;
        assert_eq!(o.known_size(), (3300, Some(2550)));
        o.landscape = false;
        assert_eq!(o.known_size(), (2550, Some(3300)));
        o.dpi = 1200;
        assert!(o.check().is_err(), "too many pixels");
        o.dpi = 10;
        assert!(o.check().is_err());
        // 3D is limited to the ray tracer's side.
        let o3 = PictureOptions {
            subject: Subject::ThreeD,
            width_px: 5000,
            ..PictureOptions::default()
        };
        assert!(o3.check().is_err());
    }

    #[test]
    fn a_plan_exports_in_every_format() {
        let p = project_with_walls();
        let mut o = PictureOptions {
            width_px: 600,
            ..PictureOptions::default()
        };
        for f in Format::ALL {
            o.format = f;
            let (bytes, w, h) = picture_bytes(&p, &o, None).unwrap();
            assert_eq!(w, 600);
            assert!(h > 100, "height follows the 4:3 plan, got {h}");
            match f {
                Format::Png => assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']),
                Format::Jpeg => assert_eq!(&bytes[..2], &[0xFF, 0xD8]),
                Format::Bmp => assert_eq!(&bytes[..2], b"BM"),
                Format::Tiff => assert_eq!(&bytes[..2], b"II"),
            }
        }
    }

    #[test]
    fn the_picture_has_ink_and_a_transparent_background_option() {
        let p = project_with_walls();
        let mut o = PictureOptions {
            width_px: 400,
            ..PictureOptions::default()
        };
        let white = render_picture(&p, &o, None).unwrap();
        assert_eq!(&white.rgba[..4], &[255, 255, 255, 255], "white corner");
        assert!(
            white.rgba.chunks(4).any(|px| px[0] < 128),
            "walls are drawn"
        );
        o.transparent = true;
        let clear = render_picture(&p, &o, None).unwrap();
        assert_eq!(clear.rgba[3], 0, "transparent corner");
        assert!(clear.rgba.chunks(4).any(|px| px[3] > 128), "ink is opaque");
        // JPEG has no alpha: the option is ignored and the corner is white.
        o.format = Format::Jpeg;
        let jpg = render_picture(&p, &o, None).unwrap();
        assert_eq!(&jpg.rgba[..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn paper_size_gives_the_canvas_and_scale_must_fit() {
        let p = project_with_walls();
        let mut o = PictureOptions {
            size_by: SizeBy::Paper,
            dpi: 100,
            ..PictureOptions::default()
        };
        let pic = render_picture(&p, &o, None).unwrap();
        assert_eq!((pic.width, pic.height), (1100, 850), "Letter at 100 DPI");
        // A 20 x 15 ft house at 1/4" = 1' is 5 x 3.75 in plus its margin: fits.
        o.scale = Some(Scale::QuarterInch);
        assert!(render_picture(&p, &o, None).is_ok());
        // At 3" = 1' it is 60 in wide: too big for Letter.
        o.scale = Some(Scale::ThreeInch);
        let e = render_picture(&p, &o, None).unwrap_err();
        assert!(e.contains("does not fit"), "{e}");
    }

    #[test]
    fn elevations_and_empty_views() {
        let p = project_with_walls();
        let o = PictureOptions {
            subject: Subject::Elevation(ViewDir::Front),
            width_px: 500,
            ..PictureOptions::default()
        };
        assert!(render_picture(&p, &o, None).is_ok());
        let empty = Project::new("Empty");
        let e = render_picture(&empty, &PictureOptions::default(), None).unwrap_err();
        assert!(e.contains("nothing to draw"), "{e}");
        let three = PictureOptions {
            subject: Subject::ThreeD,
            ..PictureOptions::default()
        };
        assert!(render_picture(&p, &three, None).is_err());
        let gone = PictureOptions {
            subject: Subject::Camera(999),
            ..PictureOptions::default()
        };
        assert!(render_picture(&p, &gone, None).is_err());
    }

    #[test]
    fn the_3d_view_is_ray_traced_at_the_chosen_size() {
        let p = project_with_walls();
        let scene = crate::shell::view3d_panel::build_view_scene(
            &p,
            &crate::shell::view3d_panel::ViewScope::default(),
        );
        assert!(!scene.meshes.is_empty());
        let snap = Snapshot3d {
            scene,
            camera: plan_render::Camera {
                eye: [120.0, 200.0, 500.0],
                target: [120.0, 40.0, -90.0],
                up: [0.0, 1.0, 0.0],
                fov_deg: 50.0,
                aperture: 0.0,
                focus_dist: 0.0,
            },
        };
        let o = PictureOptions {
            subject: Subject::ThreeD,
            width_px: 64,
            samples: 2,
            ..PictureOptions::default()
        };
        let pic = render_picture(&p, &o, Some(&snap)).unwrap();
        assert_eq!((pic.width, pic.height), (64, 48));
        assert!(
            pic.rgba.chunks(4).any(|px| px[0] != pic.rgba[0]),
            "not flat"
        );
    }

    #[test]
    fn export_to_path_writes_the_file_and_names_follow_the_view() {
        let p = project_with_walls();
        let o = PictureOptions {
            format: Format::Jpeg,
            width_px: 300,
            ..PictureOptions::default()
        };
        assert_eq!(default_file_name(&p, &o), "Export Test - 1st Floor.jpg");
        let dir = std::env::temp_dir().join(format!("ps-export-picture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plan.jpg");
        let msg = export_to_path(&p, &o, None, &path).unwrap();
        assert!(msg.contains("JPEG"), "{msg}");
        let bytes = std::fs::read(&path).unwrap();
        assert!(plan_library::image::jpeg::decode(&bytes).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }
}
