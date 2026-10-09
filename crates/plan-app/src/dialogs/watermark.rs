//! Watermark: View > Watermark and the Watermark Defaults dialog (Edit >
//! Default Settings > Watermark, or Define in the Print dialog), manual pp.
//! 1437-1440.
//!
//! The mark is the plan's (`Project::print_setup.watermark`): its settings and
//! the views it is switched on in. It shows on the plan behind the Drawing
//! Sheet, in Print Preview and in the printed PDF when Include Watermark is
//! ticked (`plan_layout::PrintOptions::watermark`). While the Defaults dialog
//! is open the view behind it shows the mark being edited, updating as the
//! answers change (or when Update is pressed).
//!
//! Placement is `plan_core::watermark::place_marks`, the same for the screen,
//! the preview and the PDF.

use super::layout::frame;
use super::{row, section, Outcome};
use crate::editor::camera::Camera;
use crate::editor::EditorContext;
use eframe::egui::{self, Color32, Pos2};
use plan_core::watermark::{
    place_marks, plan_key, PlacedMark, WatermarkKind, WatermarkLayout, WatermarkSpec, DETAIL_KEY,
    MAX_PRINT_SIZE_IN, MIN_PRINT_SIZE_IN,
};
use plan_core::Point;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// View > Watermark.
pub const TOGGLE: &str = "view.watermark";
/// Edit > Default Settings > Watermark.
pub const DEFAULTS: &str = "defaults.watermark";

/// Is `id` one of this module's commands?
pub fn is_command(id: &str) -> bool {
    matches!(id, TOGGLE | DEFAULTS)
}

/// Runs a command of this module; the status text goes to the status bar.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        TOGGLE => cx.status = toggle(cx),
        DEFAULTS => open(cx),
        _ => return false,
    }
    true
}

struct Dialog {
    draft: WatermarkSpec,
    /// What the view behind the dialog shows (the draft when it updates
    /// automatically, else what Update last took).
    shown: WatermarkSpec,
}

thread_local! {
    static DIALOG: RefCell<Option<Dialog>> = const { RefCell::new(None) };
    static TEXTURES: RefCell<HashMap<String, Option<egui::TextureHandle>>> = RefCell::new(HashMap::new());
    static ON_NOW: Cell<bool> = const { Cell::new(false) };
    static REQUEST: Cell<bool> = const { Cell::new(false) };
}

/// Asks for the Watermark Defaults dialog to open (the Print dialog's Define
/// button; the dialog opens on the next frame, when the plan is at hand).
pub fn request_open() {
    REQUEST.with(|r| r.set(true));
}

/// The key View > Watermark switches in the active view: the saved plan view,
/// or the CAD Detail.
pub fn view_key(cx: &EditorContext) -> String {
    if cx.floor().is_cad_detail() {
        DETAIL_KEY.to_string()
    } else {
        plan_key(&cx.project.active_plan_view)
    }
}

/// Is the watermark on in the active view?
pub fn is_on(cx: &EditorContext) -> bool {
    cx.project.print_setup.watermark.is_on(&view_key(cx))
}

/// Records whether the watermark is on in the active view, for the menu's
/// check mark (the menu does not see the plan).
pub fn set_menu_state(on: bool) {
    ON_NOW.with(|c| c.set(on));
}

/// View > Watermark's check mark.
pub fn menu_checked() -> bool {
    ON_NOW.with(Cell::get)
}

/// View > Watermark: switches the watermark in the active view as one undo
/// step; the status text.
pub fn toggle(cx: &mut EditorContext) -> String {
    let key = view_key(cx);
    cx.begin_change("Watermark");
    let on = cx.project.print_setup.watermark.toggle(&key);
    cx.mark_dirty();
    set_menu_state(on);
    if on && !cx.project.print_setup.watermark.spec.has_content() {
        "Watermark on, but it has nothing to show: set it in Edit > Default Settings > Watermark"
            .to_string()
    } else if on {
        "Watermark on in this view".to_string()
    } else {
        "Watermark off in this view".to_string()
    }
}

/// Opens the Watermark Defaults dialog on the plan's watermark.
pub fn open(cx: &EditorContext) {
    let spec = cx.project.print_setup.watermark.spec.clone();
    DIALOG.with(|d| {
        *d.borrow_mut() = Some(Dialog {
            shown: spec.clone(),
            draft: spec,
        });
    });
}

pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

/// The spec the view shows: the one being edited while the dialog is open,
/// else the plan's when the watermark is on in the active view.
pub fn active_spec(cx: &EditorContext) -> Option<WatermarkSpec> {
    if let Some(s) = DIALOG.with(|d| d.borrow().as_ref().map(|d| d.shown.clone())) {
        return Some(s);
    }
    is_on(cx).then(|| cx.project.print_setup.watermark.spec.clone())
}

/// Draws the Watermark Defaults dialog when it is open.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if REQUEST.with(Cell::take) && !is_open() {
        open(cx);
    }
    let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) else {
        return;
    };
    let error = d.draft.problem();
    let mut update_now = false;
    let out = frame(ctx, "Watermark Defaults", 460.0, error, |ui| {
        body(ui, &mut d.draft, &mut update_now);
    });
    if d.draft.update_automatically || update_now {
        d.shown = d.draft.clone();
    }
    match out {
        Outcome::Open => DIALOG.with(|s| *s.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            cx.begin_change("Watermark Defaults");
            cx.project.print_setup.watermark.spec = d.draft;
            cx.mark_dirty();
            cx.status = "Watermark settings updated".into();
        }
    }
}

fn body(ui: &mut egui::Ui, d: &mut WatermarkSpec, update_now: &mut bool) {
    section(ui, "Watermark Type");
    row(ui, "Type", |ui| {
        egui::ComboBox::from_id_salt("watermark_type")
            .selected_text(d.kind.label())
            .show_ui(ui, |ui| {
                for k in WatermarkKind::ALL {
                    ui.selectable_value(&mut d.kind, k, k.label());
                }
            });
    });
    match d.kind {
        WatermarkKind::Text => {
            section(ui, "Text");
            row(ui, "Text", |ui| {
                ui.add(egui::TextEdit::singleline(&mut d.text).desired_width(240.0));
            });
            row(ui, "Color", |ui| {
                ui.color_edit_button_srgb(&mut d.color);
            });
            row(ui, "Print Size", |ui| {
                ui.add(
                    egui::DragValue::new(&mut d.print_size_in)
                        .range(MIN_PRINT_SIZE_IN..=MAX_PRINT_SIZE_IN)
                        .speed(0.02)
                        .max_decimals(2)
                        .suffix("\""),
                )
                .on_hover_text("From the baseline to the top of a capital A");
            });
            row(ui, "Font", |ui| {
                crate::fonts::font_picker(ui, "watermark", &mut d.font, "", false, false);
            });
        }
        WatermarkKind::Image => {
            section(ui, "Image");
            row(ui, "File", |ui| {
                ui.add(egui::TextEdit::singleline(&mut d.image_path).desired_width(240.0));
                if ui.button("Browse\u{2026}").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Picture", &["png", "jpg", "jpeg"])
                        .pick_file()
                    {
                        d.image_path = p.to_string_lossy().into_owned();
                    }
                }
            });
            row(ui, "", |ui| {
                if ui.button("Delete From Plan").clicked() {
                    d.image_path.clear();
                }
            });
            ui.add_enabled_ui(d.layout != WatermarkLayout::FitToSheet, |ui| {
                row(ui, "Ratio to Sheet", |ui| {
                    let mut pct = d.image_ratio * 100.0;
                    if ui
                        .add(
                            egui::DragValue::new(&mut pct)
                                .range(1.0..=100.0)
                                .suffix("%"),
                        )
                        .changed()
                    {
                        d.image_ratio = pct / 100.0;
                    }
                });
            });
        }
    }
    section(ui, "General");
    row(ui, "Layout", |ui| {
        egui::ComboBox::from_id_salt("watermark_layout")
            .selected_text(d.layout.label())
            .show_ui(ui, |ui| {
                for l in WatermarkLayout::ALL {
                    ui.selectable_value(&mut d.layout, l, l.label());
                }
            });
    });
    row(ui, "Angle", |ui| {
        ui.add(
            egui::DragValue::new(&mut d.angle_deg)
                .range(-180.0..=180.0)
                .speed(1.0)
                .suffix("\u{b0}"),
        );
    });
    row(ui, "Transparency", |ui| {
        ui.add(egui::Slider::new(&mut d.transparency, 0.0..=100.0).suffix("%"));
    });
    ui.add_enabled_ui(d.layout != WatermarkLayout::FitToSheet, |ui| {
        row(ui, "Marks Per Row", |ui| {
            ui.add(egui::DragValue::new(&mut d.marks_per_row).range(1..=40));
        });
        row(ui, "Marks Per Column", |ui| {
            ui.add(egui::DragValue::new(&mut d.marks_per_column).range(1..=40));
        });
    });
    section(ui, "Margins");
    ui.checkbox(&mut d.use_sheet_margin, "Use Drawing Sheet Margin");
    ui.add_enabled_ui(!d.use_sheet_margin, |ui| {
        for (i, name) in ["Top", "Bottom", "Left", "Right"].into_iter().enumerate() {
            row(ui, name, |ui| {
                ui.add(
                    egui::DragValue::new(&mut d.margins_in[i])
                        .range(0.0..=100.0)
                        .speed(0.05)
                        .max_decimals(2)
                        .suffix("\""),
                );
            });
        }
    });
    section(ui, "Preview");
    ui.checkbox(&mut d.update_automatically, "Update Automatically");
    ui.add_enabled_ui(!d.update_automatically, |ui| {
        if ui.button("Update").clicked() {
            *update_now = true;
        }
    });
}

// ------------------------------------------------------------ on screen --

/// The picture of a picture watermark as an egui texture (loaded once per
/// file name; `None` when it cannot be read).
fn texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    TEXTURES.with(|t| {
        t.borrow_mut()
            .entry(path.to_string())
            .or_insert_with(|| {
                let img = crate::shell::layout_window::load_picture(path)?;
                let ci = egui::ColorImage::from_rgba_unmultiplied(
                    [img.width as usize, img.height as usize],
                    &img.rgba,
                );
                Some(ctx.load_texture(
                    format!("watermark_{path}"),
                    ci,
                    egui::TextureOptions::LINEAR,
                ))
            })
            .clone()
    })
}

/// Where the Drawing Sheet is on the ground: its lower-left corner, its size
/// on paper, the plan inches in each paper inch and its Drawing Margins.
pub struct SheetGeometry {
    pub lo: Point,
    pub paper_in: (f64, f64),
    pub plan_per_paper: f64,
    pub margins: [f64; 4],
}

impl SheetGeometry {
    /// A paper point of the sheet (inches from its lower-left corner) on the ground.
    pub fn world(&self, x: f64, y: f64) -> Point {
        Point::new(
            self.lo.x + x * self.plan_per_paper,
            self.lo.y + y * self.plan_per_paper,
        )
    }
}

/// Paints the watermark `spec` over the Drawing Sheet `g`.
pub fn paint(painter: &egui::Painter, cam: &Camera, spec: &WatermarkSpec, g: &SheetGeometry) {
    if spec.problem().is_some() {
        return;
    }
    let ctx = painter.ctx().clone();
    let px_per_paper = cam.px_per_in * g.plan_per_paper;
    // The sheet, as the marks are cut off at its edge.
    let corners = [
        g.world(0.0, 0.0),
        g.world(g.paper_in.0, 0.0),
        g.world(g.paper_in.0, g.paper_in.1),
        g.world(0.0, g.paper_in.1),
    ]
    .map(|p| cam.world_to_screen(p));
    let bounds = egui::Rect::from_points(&corners).intersect(painter.clip_rect());
    let painter = painter.with_clip_rect(bounds);
    let [r, gr, b] = spec.color;
    let a = (spec.alpha() * 255.0).round() as u8;
    let tint = Color32::from_rgba_unmultiplied(r, gr, b, a);
    let (own, galley_for): ((f64, f64), Option<(String, f64)>) = match spec.kind {
        WatermarkKind::Text => {
            let text = spec.text.lines().collect::<Vec<_>>().join(" ");
            let size_px = (spec.font_size_pt() / 72.0 * px_per_paper).clamp(2.0, 1500.0);
            let font = crate::fonts::font_id(
                &ctx,
                &crate::fonts::spec_named(&spec.font, false, false),
                size_px as f32,
            );
            let galley = painter.layout_no_wrap(text.clone(), font, tint);
            (
                (
                    f64::from(galley.size().x) / px_per_paper,
                    spec.print_size_in,
                ),
                Some((text, size_px)),
            )
        }
        WatermarkKind::Image => {
            let Some(tex) = texture(&ctx, spec.image_path.trim()) else {
                return;
            };
            let area = plan_core::watermark::mark_area(spec, g.paper_in, g.margins);
            let w = ((area[2] - area[0]) * spec.image_ratio).max(0.05);
            let sz = tex.size();
            ((w, w * sz[1] as f64 / sz[0].max(1) as f64), None)
        }
    };
    let marks = place_marks(spec, g.paper_in, g.margins, own);
    for m in marks {
        let k = m.w / own.0.max(1e-9);
        match (&galley_for, spec.kind) {
            (Some((text, size_px)), WatermarkKind::Text) => {
                paint_text(
                    &painter,
                    cam,
                    g,
                    &m,
                    text,
                    (*size_px * k) as f32,
                    spec,
                    tint,
                );
            }
            _ => paint_image(&painter, cam, g, &m, spec, tint),
        }
    }
}

/// A paper vector from the mark's centre, turned with the mark.
fn turned(m: &PlacedMark, dx: f64, dy: f64) -> (f64, f64) {
    let (s, c) = m.angle_deg.to_radians().sin_cos();
    (dx * c - dy * s, dx * s + dy * c)
}

fn mark_screen(cam: &Camera, g: &SheetGeometry, m: &PlacedMark, dx: f64, dy: f64) -> Pos2 {
    let (vx, vy) = turned(m, dx, dy);
    cam.world_to_screen(g.world(m.cx + vx, m.cy + vy))
}

#[allow(clippy::too_many_arguments)]
fn paint_text(
    painter: &egui::Painter,
    cam: &Camera,
    g: &SheetGeometry,
    m: &PlacedMark,
    text: &str,
    size_px: f32,
    spec: &WatermarkSpec,
    tint: Color32,
) {
    let size_px = size_px.clamp(2.0, 1500.0);
    let font = crate::fonts::font_id(
        painter.ctx(),
        &crate::fonts::spec_named(&spec.font, false, false),
        size_px,
    );
    let galley = painter.layout_no_wrap(text.to_string(), font, tint);
    // The baseline's direction on screen is the text's turn; the galley is
    // set so that the middle of the line sits on the mark's centre.
    let c = mark_screen(cam, g, m, 0.0, 0.0);
    let ahead = mark_screen(cam, g, m, 1.0, 0.0);
    let phi = (ahead.y - c.y).atan2(ahead.x - c.x);
    let (sp, cp) = phi.sin_cos();
    let half = galley.size() * 0.5;
    let pos = c - egui::vec2(half.x * cp - half.y * sp, half.x * sp + half.y * cp);
    let mut shape = egui::epaint::TextShape::new(pos, galley, tint).with_angle(phi);
    shape.override_text_color = Some(tint);
    painter.add(egui::Shape::Text(shape));
}

fn paint_image(
    painter: &egui::Painter,
    cam: &Camera,
    g: &SheetGeometry,
    m: &PlacedMark,
    spec: &WatermarkSpec,
    tint: Color32,
) {
    let Some(tex) = texture(painter.ctx(), spec.image_path.trim()) else {
        return;
    };
    let (hw, hh) = (m.w * 0.5, m.h * 0.5);
    let pts =
        [(-hw, hh), (hw, hh), (hw, -hh), (-hw, -hh)].map(|(x, y)| mark_screen(cam, g, m, x, y));
    let uv = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    let mut mesh = egui::Mesh::with_texture(tex.id());
    let white = Color32::from_white_alpha(tint.a());
    for (p, (u, v)) in pts.iter().zip(uv) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: *p,
            uv: Pos2::new(u, v),
            color: white,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_watermark_is_one_undo_step_per_saved_plan_view() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        assert!(!is_on(&cx));
        let name = cx.project.active_plan_view.clone();
        let status = toggle(&mut cx);
        assert!(status.contains("on"), "{status}");
        assert!(is_on(&cx));
        assert!(cx.project.print_setup.watermark.is_on(&plan_key(&name)));
        assert!(menu_checked());
        assert_eq!(cx.undo_label(), Some("Watermark"));
        cx.undo();
        assert!(!is_on(&cx), "one undo step");
        cx.redo();
        assert!(is_on(&cx));
        // Another saved view has its own switch.
        cx.project
            .add_plan_view(plan_core::SavedPlanView::new("Second", "Default Set"));
        assert!(cx.project.activate_plan_view("Second"));
        assert!(!is_on(&cx));
        assert!(toggle(&mut cx).contains("on"));
        assert!(cx.project.activate_plan_view(&name));
        assert!(is_on(&cx), "the first view remembers it");
    }

    #[test]
    fn the_dialog_edits_a_copy_and_ok_stores_it() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let ctx = egui::Context::default();
        open(&cx);
        assert!(is_open());
        // The active spec while open is the draft, even with the view switch off.
        assert!(active_spec(&cx).is_some());
        DIALOG.with(|d| {
            let mut d = d.borrow_mut();
            let d = d.as_mut().unwrap();
            d.draft.text = "NOT FOR CONSTRUCTION".into();
            d.draft.layout = WatermarkLayout::Border;
        });
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |c| show(c, &mut cx));
        }
        // Not stored until OK; the live preview takes the draft.
        assert_eq!(cx.project.print_setup.watermark.spec.text, "DRAFT");
        assert_eq!(active_spec(&cx).unwrap().text, "NOT FOR CONSTRUCTION");
        // OK: press Enter.
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(input, |c| show(c, &mut cx));
        assert!(!is_open());
        assert_eq!(
            cx.project.print_setup.watermark.spec.text,
            "NOT FOR CONSTRUCTION"
        );
        assert_eq!(
            cx.project.print_setup.watermark.spec.layout,
            WatermarkLayout::Border
        );
        assert_eq!(cx.undo_label(), Some("Watermark Defaults"));
        // Off in the view and the dialog closed: nothing to show.
        assert!(active_spec(&cx).is_none());
    }

    #[test]
    fn marks_paint_in_every_layout_without_panicking() {
        let ctx = egui::Context::default();
        let cam = Camera::default_view();
        let g = SheetGeometry {
            lo: Point::new(0.0, 0.0),
            paper_in: (36.0, 24.0),
            plan_per_paper: 48.0,
            margins: [0.25; 4],
        };
        for layout in WatermarkLayout::ALL {
            for angle in [0.0, 30.0, 90.0] {
                let spec = WatermarkSpec {
                    layout,
                    angle_deg: angle,
                    ..WatermarkSpec::default()
                };
                let _ = ctx.run(egui::RawInput::default(), |c| {
                    egui::CentralPanel::default().show(c, |ui| {
                        let p = ui.painter().clone();
                        paint(&p, &cam, &spec, &g);
                    });
                });
            }
        }
    }
}
