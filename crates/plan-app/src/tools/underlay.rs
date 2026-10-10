//! Underlays (L-43, L-46): a picture placed under the plan for tracing.
//!
//! * [`import_image`] places a PNG or JPEG at the middle of the plan (File >
//!   Import > Underlay Image, or the Underlays window). PDF pages are not
//!   rasterized by this build: save the page as an image and import that.
//! * [`apply_calibration`]: two-point calibration. The Underlay tool collects
//!   two clicks on the picture; the Underlays window asks for the real
//!   distance between them and scales the picture about the first point.
//! * [`draw_underlays`] draws the pictures under everything else (a textured
//!   quad for PNG and baseline JPEG, a framed placeholder otherwise)
//!   at the picture's opacity, on the "Underlays" layer.
//! * The tool also drags the active underlay (Position mode).
//!
//! The window that lists, edits and calibrates underlays is
//! `dialogs::underlay`.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::underlay as win;
use crate::editor::{Camera, EditorContext};
use crate::shell::library_browser::png;
use eframe::egui::{self, Color32, Key, Pos2, Shape, Stroke, TextureHandle, TextureId};
use plan_core::geometry::Point;
use plan_core::images::{format_of, image_size, ImageFormat};
use plan_core::underlay::{Underlay, UnderlayKind, UNDERLAY_LAYER};
use plan_core::Id;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Receiver;

/// Longest side a decoded picture is kept at in video memory, pixels.
const MAX_TEXTURE_SIDE: usize = 2048;

// ----- import -----

/// Where a new underlay goes: the middle of the walls, else the origin.
pub(crate) fn plan_center(cx: &EditorContext) -> Point {
    let pts: Vec<Point> = cx
        .floor()
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .collect();
    if pts.is_empty() {
        return Point::ZERO;
    }
    let (mut lo, mut hi) = (pts[0], pts[0]);
    for p in &pts {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Point::lerp(lo, hi, 0.5)
}

/// Why a file cannot be an underlay.
pub fn import_error(path: &Path, bytes: &[u8]) -> String {
    let is_pdf = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        || bytes.starts_with(b"%PDF");
    if is_pdf {
        "That PDF holds no scanned picture (this build cannot draw vector PDF pages): print the page to a PNG and import that"
            .to_string()
    } else {
        format!("{} is not a PNG or JPEG picture", path.display())
    }
}

/// Where pictures taken out of PDFs are kept: `~/.plan-studio/underlays/`
/// (the temporary folder without a home).
fn picture_folder() -> std::path::PathBuf {
    // Tests never write into the real home.
    #[cfg(test)]
    {
        std::env::temp_dir().join(format!("plan-studio-underlays-{}", std::process::id()))
    }
    #[cfg(not(test))]
    {
        crate::paths::user_file("underlays")
            .unwrap_or_else(|| std::env::temp_dir().join("plan-studio-underlays"))
    }
}

/// Places the picture at `path` as an underlay of the active floor (one undo
/// step, calibrated later). A PDF gives its first scanned page; use
/// [`import_pdf_page`] for another. Returns its id.
pub fn import_image(cx: &mut EditorContext, path: &Path) -> Result<Id, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    if bytes.starts_with(b"%PDF") {
        return import_pdf_bytes(cx, path, &bytes, 1, &picture_folder());
    }
    let Some((w, h, _)) = image_size(&bytes).filter(|(_, _, f)| *f != ImageFormat::Other) else {
        return Err(import_error(path, &bytes));
    };
    if w == 0 || h == 0 {
        return Err(import_error(path, &bytes));
    }
    let name = path.file_stem().map_or_else(
        || "Underlay".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let abs = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned();
    Ok(add_underlay(
        cx,
        Underlay::new(name, abs, w, h, plan_center(cx)),
    ))
}

/// Places the `page`th (1-based) scanned picture of the PDF at `path`.
pub fn import_pdf_page(cx: &mut EditorContext, path: &Path, page: usize) -> Result<Id, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    import_pdf_bytes(cx, path, &bytes, page, &picture_folder())
}

/// How many pages of the PDF at `path` have a picture this build can read.
pub fn pdf_page_count(path: &Path) -> usize {
    std::fs::read(path)
        .ok()
        .and_then(|b| pdf::page_images(&b).ok())
        .map_or(0, |p| p.pages.len())
}

fn import_pdf_bytes(
    cx: &mut EditorContext,
    path: &Path,
    bytes: &[u8],
    page: usize,
    folder: &Path,
) -> Result<Id, String> {
    let found = pdf::page_images(bytes).map_err(|e| {
        if e.starts_with("That file") {
            import_error(path, bytes)
        } else {
            e
        }
    })?;
    let pictures = &found.pages;
    // The page asked for, else the page at that place in the list, else the
    // last one.
    let pic = pictures
        .iter()
        .find(|p| p.page == page)
        .or_else(|| pictures.get(page.saturating_sub(1)))
        .or(pictures.last())
        .ok_or_else(|| import_error(path, bytes))?;
    let n = pic.page;
    let stem = path
        .file_stem()
        .map_or_else(|| "plan".to_string(), |s| s.to_string_lossy().into_owned());
    std::fs::create_dir_all(folder)
        .map_err(|e| format!("Could not make {}: {e}", folder.display()))?;
    // Named by the content too, so two PDFs of one name do not share a file.
    let sum = pic
        .data
        .bytes()
        .iter()
        .fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(*b)));
    let file = folder.join(format!("{stem}-p{n}-{sum:08x}.{}", pic.data.extension()));
    std::fs::write(&file, pic.data.bytes())
        .map_err(|e| format!("Could not write {}: {e}", file.display()))?;
    let mut u = Underlay::new(
        if found.page_count.max(pictures.len()) > 1 {
            format!("{stem} p{n}")
        } else {
            stem
        },
        file.to_string_lossy().into_owned(),
        pic.width,
        pic.height,
        plan_center(cx),
    );
    u.kind = UnderlayKind::Pdf;
    u.page = n as u32;
    Ok(add_underlay(cx, u))
}

fn add_underlay(cx: &mut EditorContext, u: Underlay) -> Id {
    cx.begin_change("Import Underlay");
    let fl = cx.floor;
    let id = cx.project.add_underlay(fl, u);
    cx.mark_dirty();
    win::set_active(Some(id));
    id
}

// ----- editing -----

fn locked(cx: &EditorContext, u: &Underlay) -> bool {
    u.locked || cx.layers().is_locked(UNDERLAY_LAYER)
}

/// Two-point calibration as one undo step: the picture is scaled about `a` so
/// that the points `a` and `b` (plan points on the picture) are `real` inches
/// apart. False (changing nothing) for a locked or missing underlay, equal
/// points or a non-positive distance.
pub fn apply_calibration(cx: &mut EditorContext, id: Id, a: Point, b: Point, real: f64) -> bool {
    let fl = cx.floor;
    let Some(u) = cx.floor().underlay(id) else {
        return false;
    };
    if locked(cx, u) {
        cx.status = "That underlay is locked".into();
        return false;
    }
    let mut trial = u.clone();
    if !trial.calibrate(a, b, real) {
        cx.status = "Calibration needs two different points and a distance".into();
        return false;
    }
    cx.begin_change("Calibrate Underlay");
    if let Some(slot) = cx.project.floors[fl].underlay_mut(id) {
        *slot = trial;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Underlay scaled: those points are now {} apart",
        cx.fmt_dim(real)
    );
    true
}

/// Sets the picture's real width (its height follows the pixel aspect), as
/// one undo step; for pictures that cannot be shown (JPEG) or whose width is
/// known. The picture keeps its centre.
pub fn set_width(cx: &mut EditorContext, id: Id, width: f64) -> bool {
    let fl = cx.floor;
    let Some(u) = cx.floor().underlay(id) else {
        return false;
    };
    if locked(cx, u) || width < 0.01 || !width.is_finite() {
        return false;
    }
    let mut n = u.clone();
    // Keep the middle of the picture where it is, with the rotation.
    let mid_before = n.pixel_to_plan(
        f64::from(n.pixel_width) * 0.5,
        f64::from(n.pixel_height) * 0.5,
    );
    n.inches_per_pixel = width / f64::from(n.pixel_width.max(1));
    let mid_after = n.pixel_to_plan(
        f64::from(n.pixel_width) * 0.5,
        f64::from(n.pixel_height) * 0.5,
    );
    n.translate(mid_before.sub(mid_after));
    n.calibrated = true;
    cx.begin_change("Set Underlay Width");
    if let Some(slot) = cx.project.floors[fl].underlay_mut(id) {
        *slot = n;
    }
    cx.mark_dirty();
    true
}

/// The topmost visible underlay under `p`.
pub fn hit_underlay(cx: &EditorContext, p: Point) -> Option<Id> {
    if !cx.layers().is_visible(UNDERLAY_LAYER) {
        return None;
    }
    cx.floor()
        .underlays
        .iter()
        .rev()
        .find(|u| u.visible && u.contains(p))
        .map(|u| u.id)
}

// ----- drawing -----

mod inflate;
mod jpeg;
pub mod pdf;
pub mod trace;

/// Where an underlay picture is on its way to the screen.
enum Slot {
    /// Being read and decoded on a thread.
    Loading(Receiver<Result<egui::ColorImage, String>>),
    Ready(TextureHandle),
    /// Could not be shown, and why.
    Failed(String),
}

thread_local! {
    static TEXTURES: RefCell<HashMap<String, Slot>> = RefCell::new(HashMap::new());
}

/// What the plan can draw of an underlay's picture.
#[derive(Clone, Debug, PartialEq)]
pub enum Picture {
    Ready(TextureId),
    Loading,
    /// The picture cannot be shown; the text says why.
    Unavailable(String),
}

/// Reads and decodes a PNG or JPEG into an egui image of at most
/// [`MAX_TEXTURE_SIDE`] pixels on the long side.
pub fn load_picture(path: &str) -> Result<egui::ColorImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {path}: {e}"))?;
    let img = match format_of(path) {
        ImageFormat::Png => png::decode(&bytes)?,
        // Our baseline decoder first; progressive and CMYK files (and the
        // JPEGs of scanned PDFs) go to the shared decoder.
        ImageFormat::Jpeg => jpeg::decode(&bytes).or_else(|first| {
            plan_library::image::jpeg::decode(&bytes)
                .map(|i| png::Rgba {
                    width: i.width as usize,
                    height: i.height as usize,
                    pixels: i.rgba,
                })
                .map_err(|_| first)
        })?,
        ImageFormat::Other => return Err("not a PNG or JPEG picture".to_string()),
    };
    Ok(img.downscaled(MAX_TEXTURE_SIDE).to_color_image())
}

/// The egui texture of an underlay picture: decoded once per path on a
/// thread (a survey scan is large), then kept. `ctx` is asked to repaint
/// while it loads.
pub fn picture(ctx: &egui::Context, u: &Underlay) -> Picture {
    TEXTURES.with(|t| {
        let mut t = t.borrow_mut();
        let slot = t.entry(u.path.clone()).or_insert_with(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            let path = u.path.clone();
            std::thread::spawn(move || {
                let _ = tx.send(load_picture(&path));
            });
            Slot::Loading(rx)
        });
        if let Slot::Loading(rx) = slot {
            match rx.try_recv() {
                Ok(Ok(img)) => {
                    *slot = Slot::Ready(ctx.load_texture(
                        format!("underlay:{}", u.path),
                        img,
                        egui::TextureOptions::LINEAR,
                    ));
                }
                Ok(Err(e)) => *slot = Slot::Failed(e),
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    *slot = Slot::Failed("the picture could not be read".into());
                }
            }
        }
        match slot {
            Slot::Ready(h) => Picture::Ready(h.id()),
            Slot::Loading(_) => Picture::Loading,
            Slot::Failed(e) => Picture::Unavailable(e.clone()),
        }
    })
}

/// Draws the floor's underlays, bottom of the list first.
pub fn draw_underlays(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if cx.floor().underlays.is_empty() || !cx.layers().is_visible(UNDERLAY_LAYER) {
        return;
    }
    let ink = cx.palette.text;
    for u in cx.floor().underlays.iter().filter(|u| u.visible) {
        let corners = u.corners();
        let pts: Vec<Pos2> = corners.iter().map(|p| cam.world_to_screen(*p)).collect();
        let alpha = (u.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        let pic = picture(painter.ctx(), u);
        match pic {
            Picture::Ready(id) => {
                let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
                let mut mesh = egui::Mesh::with_texture(id);
                for (p, uv) in pts.iter().zip(uv) {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: *p,
                        uv: Pos2::new(uv[0], uv[1]),
                        color: Color32::from_white_alpha(alpha),
                    });
                }
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                painter.add(Shape::mesh(mesh));
                painter.add(Shape::closed_line(
                    pts,
                    Stroke::new(0.8_f32, ink.gamma_multiply(0.35)),
                ));
            }
            ref other => {
                // Still loading, or the picture cannot be shown (a PDF, a
                // progressive JPEG, a missing file): a framed placeholder
                // that still moves, scales and calibrates.
                painter.add(Shape::convex_polygon(
                    pts.clone(),
                    Color32::from_rgba_unmultiplied(110, 130, 160, alpha / 4),
                    Stroke::new(1.2_f32, ink.gamma_multiply(0.6)),
                ));
                painter.add(Shape::line(vec![pts[0], pts[2]], Stroke::new(0.6_f32, ink)));
                painter.add(Shape::line(vec![pts[1], pts[3]], Stroke::new(0.6_f32, ink)));
                painter.text(
                    (pts[0] + pts[2].to_vec2()) * 0.5,
                    egui::Align2::CENTER_CENTER,
                    match other {
                        Picture::Loading => format!("{} (loading...)", u.name),
                        Picture::Unavailable(why) => format!("{} ({why})", u.name),
                        Picture::Ready(_) => u.name.clone(),
                    },
                    egui::FontId::proportional(12.0),
                    ink,
                );
            }
        }
    }
}

// ----- the tool -----

/// What a drag is moving.
struct Drag {
    id: Id,
    start: Point,
    origin: Point,
    begun: bool,
}

/// The Underlay tool: Position mode drags the active underlay (a click on
/// another picture makes it the active one); Calibrate mode takes the two
/// clicks of the two-point calibration.
#[derive(Default)]
pub struct UnderlayTool {
    drag: Option<Drag>,
}

impl Tool for UnderlayTool {
    fn id(&self) -> ToolId {
        ToolId::Underlay
    }

    fn name(&self) -> &'static str {
        "Underlay"
    }

    fn hint(&self) -> String {
        match win::calibration() {
            Some(c) if c.align && c.a.is_none() => {
                "Rotate to Align: click the first end of a line that should be level".into()
            }
            Some(c) if c.align => "Rotate to Align: click the other end of the line".into(),
            Some(c) if c.a.is_none() => {
                "Point to Point Resize: click the first of two points on the picture".into()
            }
            Some(c) if c.b.is_none() => "Point to Point Resize: click the second point".into(),
            Some(_) => {
                "Point to Point Resize: type the real distance in the Underlays window".into()
            }
            None => "Underlay: drag the picture to move it; click another to pick it; Esc returns"
                .into(),
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        if win::calibration().is_some() {
            egui::CursorIcon::Crosshair
        } else {
            egui::CursorIcon::Move
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.drag = None;
        cx.status = self.hint();
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.drag = None;
        win::cancel_calibration();
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if win::calibration().is_some() {
            win::push_calibration_point(p.world);
            // Rotate to Align needs no distance: the second click does it.
            if let Some(c) = win::calibration().filter(|c| c.align) {
                if let (Some(a), Some(b)) = (c.a, c.b) {
                    if trace::align_underlay(cx, c.id, a, b) {
                        win::cancel_calibration();
                        return ToolResult::committed("Rotate Underlay to Align");
                    }
                    win::cancel_calibration();
                    return ToolResult::consumed();
                }
            }
            cx.status = self.hint();
            return ToolResult::consumed();
        }
        let active = win::active().and_then(|id| cx.floor().underlay(id));
        let id = match active {
            Some(u) if u.visible && u.contains(p.world) => Some(u.id),
            _ => hit_underlay(cx, p.world),
        };
        let Some(id) = id else {
            return ToolResult::ignored();
        };
        win::set_active(Some(id));
        if let Some(u) = cx.floor().underlay(id) {
            if locked(cx, u) {
                cx.status = "That underlay is locked".into();
                return ToolResult::consumed();
            }
            self.drag = Some(Drag {
                id,
                start: p.world,
                origin: u.origin,
                begun: false,
            });
        }
        ToolResult::consumed()
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let Some(d) = &mut self.drag else {
            return ToolResult::ignored();
        };
        if !p.down {
            return ToolResult::ignored();
        }
        if !d.begun {
            cx.begin_change("Move Underlay");
            d.begun = true;
        }
        let fl = cx.floor;
        let delta = p.world.sub(d.start);
        if let Some(u) = cx.project.floors[fl].underlay_mut(d.id) {
            u.origin = d.origin.add(delta);
        }
        cx.mark_dirty();
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, _cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        match self.drag.take() {
            Some(d) if d.begun => ToolResult::committed("Move Underlay"),
            Some(_) => ToolResult::consumed(),
            None => ToolResult::ignored(),
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Escape) {
            if let Some(d) = self.drag.take() {
                if d.begun {
                    let fl = cx.floor;
                    if let Some(u) = cx.project.floors[fl].underlay_mut(d.id) {
                        u.origin = d.origin;
                    }
                    cx.cancel_change();
                    cx.mark_dirty();
                }
                return ToolResult::consumed();
            }
            if win::calibration().is_some() {
                win::cancel_calibration();
                cx.status = "Calibration cancelled".into();
                return ToolResult::consumed();
            }
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let accent = cx.palette.selection;
        if let Some(u) = win::active().and_then(|id| cx.floor().underlay(id)) {
            let pts: Vec<Pos2> = u
                .corners()
                .iter()
                .map(|p| cam.world_to_screen(*p))
                .collect();
            painter.add(Shape::closed_line(pts, Stroke::new(1.5_f32, accent)));
        }
        if let Some(c) = win::calibration() {
            let mark = |p: Point, label: &str| {
                let s = cam.world_to_screen(p);
                painter.line_segment(
                    [s - egui::vec2(8.0, 0.0), s + egui::vec2(8.0, 0.0)],
                    Stroke::new(1.5_f32, accent),
                );
                painter.line_segment(
                    [s - egui::vec2(0.0, 8.0), s + egui::vec2(0.0, 8.0)],
                    Stroke::new(1.5_f32, accent),
                );
                painter.text(
                    s + egui::vec2(10.0, -10.0),
                    egui::Align2::LEFT_BOTTOM,
                    label,
                    egui::FontId::proportional(13.0),
                    accent,
                );
            };
            if let Some(a) = c.a {
                mark(a, "A");
            }
            if let Some(b) = c.b {
                mark(b, "B");
            }
            if let (Some(a), Some(b)) = (c.a, c.b) {
                painter.add(Shape::dashed_line(
                    &[cam.world_to_screen(a), cam.world_to_screen(b)],
                    Stroke::new(1.2_f32, accent),
                    6.0,
                    4.0,
                ));
            }
        }
    }
}

// ----- commands -----

/// Command id: File > Import > Underlay Image...
pub const IMPORT: &str = "underlay.import";
/// Command id: Tools > Underlays... (the window).
pub const MANAGE: &str = "underlay.manage";

/// Runs an underlay command by id (`EditorContext::run_custom`); false when
/// the id is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        IMPORT => {
            win::pick_and_import(cx);
            true
        }
        MANAGE => {
            win::open();
            true
        }
        // The other picture and detail commands share this door.
        _ => super::images::run_command(cx, id) || super::details::run_command(cx, id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    /// A 1x1 white PNG, the smallest valid file the header reader accepts.
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8,
        0xFF, 0xFF, 0x3F, 0x00, 0x05, 0xFE, 0x02, 0xFE, 0xDC, 0xCC, 0x59, 0xE7, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    fn scratch(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("plan-studio-underlay-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn importing_a_png_adds_an_underlay_layer_and_one_undo_step() {
        let mut cx = cx();
        let path = scratch("survey.png", PNG_1X1);
        let id = import_image(&mut cx, &path).unwrap();
        let u = cx.floor().underlay(id).unwrap();
        assert_eq!((u.pixel_width, u.pixel_height), (1, 1));
        assert_eq!(u.name, "survey");
        assert!(!u.calibrated);
        assert!(cx.project.layers.get(UNDERLAY_LAYER).is_some());
        assert_eq!(cx.undo_label(), Some("Import Underlay"));
        cx.undo();
        assert!(cx.floor().underlays.is_empty());
    }

    #[test]
    fn importing_a_pdf_or_junk_explains_itself() {
        let mut cx = cx();
        let pdf = scratch("plan.pdf", b"%PDF-1.4\n");
        let err = import_image(&mut cx, &pdf).unwrap_err();
        assert!(err.contains("PDF") && err.contains("scanned"), "{err}");
        let junk = scratch("x.png", b"not a picture at all");
        assert!(import_image(&mut cx, &junk)
            .unwrap_err()
            .contains("not a PNG"));
        assert!(cx.floor().underlays.is_empty());
        assert!(cx.undo_label().is_none());
    }

    #[test]
    fn calibration_scales_about_the_first_point_in_one_undo_step() {
        let mut cx = cx();
        let id = import_image(&mut cx, &scratch("s.png", PNG_1X1)).unwrap();
        // The 1x1 picture is 480" across; click two points 120" apart on it
        // that really are 240" apart.
        let (a, b) = {
            let u = cx.floor().underlay(id).unwrap();
            (u.pixel_to_plan(0.25, 0.5), u.pixel_to_plan(0.5, 0.5))
        };
        assert!((a.dist(b) - 120.0).abs() < 1e-9);
        assert!(apply_calibration(&mut cx, id, a, b, 240.0));
        let u = cx.floor().underlay(id).unwrap();
        assert!(u.calibrated);
        assert!((u.pixel_to_plan(0.25, 0.5).dist(u.pixel_to_plan(0.5, 0.5)) - 240.0).abs() < 1e-9);
        assert!(u.pixel_to_plan(0.25, 0.5).dist(a) < 1e-9);
        assert_eq!(cx.undo_label(), Some("Calibrate Underlay"));
        cx.undo();
        assert!((cx.floor().underlay(id).unwrap().size().0 - 480.0).abs() < 1e-9);
    }

    #[test]
    fn locked_underlays_refuse_calibration_and_width() {
        let mut cx = cx();
        let id = import_image(&mut cx, &scratch("l.png", PNG_1X1)).unwrap();
        let fl = cx.floor;
        cx.project.floors[fl].underlay_mut(id).unwrap().locked = true;
        let before = cx.floor().underlay(id).unwrap().clone();
        assert!(!apply_calibration(
            &mut cx,
            id,
            Point::ZERO,
            Point::new(10.0, 0.0),
            20.0
        ));
        assert!(!set_width(&mut cx, id, 100.0));
        assert_eq!(*cx.floor().underlay(id).unwrap(), before);
    }

    #[test]
    fn set_width_keeps_the_middle_and_the_aspect() {
        let mut cx = cx();
        let id = import_image(&mut cx, &scratch("w.png", PNG_1X1)).unwrap();
        let mid = |cx: &EditorContext| cx.floor().underlay(id).unwrap().pixel_to_plan(0.5, 0.5);
        let before = mid(&cx);
        assert!(set_width(&mut cx, id, 960.0));
        assert!(mid(&cx).dist(before) < 1e-9);
        assert!((cx.floor().underlay(id).unwrap().size().0 - 960.0).abs() < 1e-9);
    }

    #[test]
    fn the_tool_moves_the_picture_and_calibrates_by_two_clicks() {
        let mut cx = cx();
        let id = import_image(&mut cx, &scratch("t.png", PNG_1X1)).unwrap();
        let mut tool = UnderlayTool::default();
        tool.activate(&mut cx);
        let origin = cx.floor().underlay(id).unwrap().origin;
        let inside = origin.add(Point::new(100.0, 100.0));
        let down = PointerEvent::at(&cx, inside);
        assert!(tool.pointer_down(&mut cx, down).consumed);
        let moved = PointerEvent::at(&cx, inside.add(Point::new(30.0, -10.0))).with_down(true);
        tool.pointer_move(&mut cx, moved);
        let up = PointerEvent::at(&cx, inside.add(Point::new(30.0, -10.0)));
        assert_eq!(
            tool.pointer_up(&mut cx, up).commit.as_deref(),
            Some("Move Underlay")
        );
        let now = cx.floor().underlay(id).unwrap().origin;
        assert!((now.x - origin.x - 30.0).abs() < 1e-9 && (now.y - origin.y + 10.0).abs() < 1e-9);
        // Calibrate mode takes the next two clicks as the points.
        win::begin_calibration(id);
        let a = now.add(Point::new(50.0, 50.0));
        let b = now.add(Point::new(150.0, 50.0));
        let (ea, eb) = (PointerEvent::at(&cx, a), PointerEvent::at(&cx, b));
        tool.pointer_down(&mut cx, ea);
        tool.pointer_down(&mut cx, eb);
        let c = win::calibration().unwrap();
        assert_eq!((c.a, c.b), (Some(a), Some(b)));
        win::cancel_calibration();
    }

    fn fixture(name: &str) -> String {
        format!(
            "{}/src/tools/underlay/testdata/{name}",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn wait_for(ctx: &egui::Context, u: &Underlay) -> Picture {
        for _ in 0..500 {
            match picture(ctx, u) {
                Picture::Loading => std::thread::sleep(std::time::Duration::from_millis(10)),
                done => return done,
            }
        }
        panic!("the picture never finished loading");
    }

    #[test]
    fn png_and_jpeg_pictures_load_on_a_thread_and_a_bad_one_says_why() {
        let ctx = egui::Context::default();
        for (name, w, h) in [
            ("pixels.png", 3, 2),
            ("rgb444.jpg", 40, 24),
            ("gray.jpg", 21, 17),
        ] {
            let u = Underlay::new(name, fixture(name), w, h, Point::ZERO);
            // The first call only starts the work.
            assert!(
                !matches!(picture(&ctx, &u), Picture::Unavailable(_)),
                "{name}"
            );
            assert!(matches!(wait_for(&ctx, &u), Picture::Ready(_)), "{name}");
        }
        let img = load_picture(&fixture("pixels.png")).unwrap();
        assert_eq!(img.size, [3, 2]);
        // Progressive JPEG goes to the shared decoder.
        let progressive = Underlay::new("p", fixture("progressive.jpg"), 32, 32, Point::ZERO);
        assert!(matches!(wait_for(&ctx, &progressive), Picture::Ready(_)));
        // A damaged JPEG says why.
        let junk = scratch("junk.jpg", &[0xFF, 0xD8, 0xFF, 0xD9]);
        let broken = Underlay::new("j", junk.to_string_lossy(), 8, 8, Point::ZERO);
        assert!(matches!(wait_for(&ctx, &broken), Picture::Unavailable(_)));
        let missing = Underlay::new("m", "/no/such/file.png", 10, 10, Point::ZERO);
        assert!(matches!(wait_for(&ctx, &missing), Picture::Unavailable(_)));
        let mut pdf = Underlay::new("d", "/x/plan.pdf", 10, 10, Point::ZERO);
        pdf.kind = UnderlayKind::Pdf;
        assert!(matches!(wait_for(&ctx, &pdf), Picture::Unavailable(_)));
    }

    #[test]
    fn underlays_draw_headlessly_loading_ready_and_unavailable() {
        let mut cx = cx();
        let ok = import_image(&mut cx, Path::new(&fixture("pixels.png"))).unwrap();
        let bad = import_image(&mut cx, &scratch("never-decoded.png", PNG_1X1)).unwrap();
        assert_ne!(ok, bad);
        let ctx = egui::Context::default();
        let cam = Camera::default_view();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    draw_underlays(&cx, ui.painter(), &cam);
                });
            });
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }

    #[test]
    fn a_scanned_pdf_gives_its_pages_as_underlays() {
        let mut cx = cx();
        let pdf = scratch("survey-scan.pdf", &pdf::tests::sample_pdf());
        assert_eq!(pdf_page_count(&pdf), 2);
        let folder =
            std::env::temp_dir().join(format!("plan-studio-pdfpics-{}", std::process::id()));
        let bytes = std::fs::read(&pdf).unwrap();
        let first = import_pdf_bytes(&mut cx, &pdf, &bytes, 1, &folder).unwrap();
        let second = import_pdf_bytes(&mut cx, &pdf, &bytes, 2, &folder).unwrap();
        let (a, b) = (
            cx.floor().underlay(first).unwrap().clone(),
            cx.floor().underlay(second).unwrap().clone(),
        );
        assert_eq!(
            (a.kind, a.page, a.name.as_str()),
            (UnderlayKind::Pdf, 1, "survey-scan p1")
        );
        assert_eq!((b.page, b.pixel_width, b.pixel_height), (2, 21, 17));
        assert_eq!((a.pixel_width, a.pixel_height), (40, 24));
        // The picture was taken out of the PDF into a JPEG file the plan
        // points at, and it decodes.
        assert!(a.path.ends_with(".jpg") && Path::new(&a.path).exists());
        assert!(load_picture(&a.path).is_ok());
        // A page past the end gives the last one.
        let last = import_pdf_bytes(&mut cx, &pdf, &bytes, 9, &folder).unwrap();
        assert_eq!(cx.floor().underlay(last).unwrap().page, 2);
        // The plan opens again with the same underlays.
        let back = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(back.floors[0].underlays.len(), 3);
        assert_eq!(back.floors[0].underlays[0].kind, UnderlayKind::Pdf);
        // Through the plain image door a PDF gives its first page.
        let via = import_image(&mut cx, &pdf).unwrap();
        assert_eq!(cx.floor().underlay(via).unwrap().page, 1);
    }
}
