//! The Fill Style panel and dialogs (CAD-69, CAD-73..CAD-77, CAD-72; manual
//! pp. 221-226).
//!
//! One [`panel`] edits a `plan_core::fill_styles::FillStyle` wherever a Fill
//! Style panel appears: the pattern Type (Use Layer, the system patterns, a
//! library pattern), the scale or width and height, the row offset, the
//! horizontal and vertical offsets, the angle, the line weight, the colour
//! source (single, layer or background) with transparency, a gradient, the
//! pattern background, Add to Library and a preview with a width choice.
//!
//! * **Fill Style Specification** ([`open_named`]): a named fill style for the
//!   library.
//! * **Fill Style** for the selection ([`open_for_selection`]): paints the
//!   style on the selected closed CAD shapes, slabs, rooms and walls.
//! * [`read_fill`] / [`apply_fill`]: what the Fill Style Eyedropper reads and
//!   the Fill Style Painter writes (`tools/painters.rs` calls them).
//! * [`paint_fill`]: draws a fill; plan, previews and the poché share it.

use super::{row, section, Outcome};
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{
    self, Align, Align2, Color32, Key, Layout, Modifiers, Pos2, RichText, Shape, Stroke, Vec2,
};
use plan_core::cad::CadItem;
use plan_core::fill_styles::{
    alpha, fill_geometry, ColorSource, FillStyle, FillTarget, Gradient, GradientKind, PatternType,
    SystemPattern,
};
use plan_core::geometry::Point;
use plan_core::patterns::CustomPattern;
use std::cell::RefCell;

/// The Fill Style dialog for the selected objects.
pub const APPLY: &str = "fillstyle.apply";
/// A new named fill style for the library (Fill Style Specification).
pub const NEW_NAMED: &str = "fillstyle.new";
/// View > Poché: turns poché on or off in the active view.
pub const POCHE: &str = "view.poche";

thread_local! {
    static HOST: RefCell<Option<FillDialog>> = const { RefCell::new(None) };
    /// Poché was on in the view drawn last (the menu's check mark).
    static POCHE_SHOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The plan drawing notes whether poché is on in its view.
pub fn note_poche(on: bool) {
    POCHE_SHOWN.with(|p| p.set(on));
}

/// Is poché on in the view that was drawn last?
pub fn poche_shown() -> bool {
    POCHE_SHOWN.with(std::cell::Cell::get)
}

/// View > Poché: flips the switch of the active plan view. One undo step.
pub fn toggle_poche(cx: &mut EditorContext) -> bool {
    let view = cx.project.active_plan_view.clone();
    let on = !cx
        .project
        .styles
        .poche
        .is_on(&view, plan_core::fill_styles::PocheView::Plan);
    cx.begin_change("Poch\u{e9}");
    cx.project.styles.poche.set(&view, on);
    cx.mark_dirty();
    note_poche(on);
    cx.status = format!("Poch\u{e9} {} in {view}", if on { "on" } else { "off" });
    on
}

const PREVIEW_WIDTHS: [f64; 6] = [12.0, 24.0, 48.0, 96.0, 192.0, 384.0];

// ---------------------------------------------------------------------------
// Painting
// ---------------------------------------------------------------------------

fn color32(c: [u8; 3], a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a)
}

/// Line width on screen (px) of a pattern line of `weight` hundredths of a
/// millimetre; `k` is the view's line-weight factor (1.0 off).
pub fn line_px(weight: u32, k: f32) -> f32 {
    ((weight as f32 / 25.0) * k).clamp(0.6, 8.0)
}

/// A solid area as a triangle mesh (outer ring minus holes).
pub fn solid_mesh(
    to_screen: &dyn Fn(Point) -> Pos2,
    outer: &[Point],
    holes: &[Vec<Point>],
    color: Color32,
) -> Option<egui::Mesh> {
    let tris = plan_3d::triangulate::ear_clip_with_holes(outer, holes);
    if tris.is_empty() {
        return None;
    }
    let all: Vec<Point> = std::iter::once(outer)
        .chain(holes.iter().map(Vec::as_slice))
        .flatten()
        .copied()
        .collect();
    let mut mesh = egui::Mesh::default();
    for t in tris {
        let base = mesh.vertices.len() as u32;
        for &i in &t {
            let Some(p) = all.get(i) else { return None };
            mesh.colored_vertex(to_screen(*p), color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
    }
    Some(mesh)
}

/// A gradient area: each triangle is cut into a grid and every corner takes
/// the gradient's colour there.
pub fn gradient_mesh(
    to_screen: &dyn Fn(Point) -> Pos2,
    outer: &[Point],
    holes: &[Vec<Point>],
    g: &Gradient,
    layer_alpha: f64,
) -> Option<egui::Mesh> {
    let tris = plan_3d::triangulate::ear_clip_with_holes(outer, holes);
    if tris.is_empty() {
        return None;
    }
    let all: Vec<Point> = std::iter::once(outer)
        .chain(holes.iter().map(Vec::as_slice))
        .flatten()
        .copied()
        .collect();
    let (mut lo, mut hi) = (all[0], all[0]);
    for p in &all {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    const N: usize = 8;
    let mut mesh = egui::Mesh::default();
    let color_at = |p: Point| -> Color32 {
        let (c, tr) = g.sample(g.position_at(p, lo, hi));
        color32(c, alpha(1.0 - (1.0 - tr) * layer_alpha))
    };
    for t in tris {
        let (Some(a), Some(b), Some(c)) = (all.get(t[0]), all.get(t[1]), all.get(t[2])) else {
            continue;
        };
        // Barycentric grid of N x N small triangles.
        let pt = |i: usize, j: usize| -> Point {
            let (u, v) = (i as f64 / N as f64, j as f64 / N as f64);
            Point::new(
                a.x + (b.x - a.x) * u + (c.x - a.x) * v,
                a.y + (b.y - a.y) * u + (c.y - a.y) * v,
            )
        };
        let mut index = std::collections::HashMap::new();
        let mut vert = |mesh: &mut egui::Mesh, i: usize, j: usize| -> u32 {
            *index.entry((i, j)).or_insert_with(|| {
                let p = pt(i, j);
                mesh.colored_vertex(to_screen(p), color_at(p));
                (mesh.vertices.len() - 1) as u32
            })
        };
        for i in 0..N {
            for j in 0..(N - i) {
                let (p0, p1, p2) = (
                    vert(&mut mesh, i, j),
                    vert(&mut mesh, i + 1, j),
                    vert(&mut mesh, i, j + 1),
                );
                mesh.add_triangle(p0, p1, p2);
                if i + j + 1 < N {
                    let p3 = vert(&mut mesh, i + 1, j + 1);
                    mesh.add_triangle(p1, p3, p2);
                }
            }
        }
    }
    Some(mesh)
}

/// Draws `style` over `outer` minus `holes`. `to_screen` maps plan points to
/// screen points; `px_per_in` is the view's zoom; `layer_rgb` and `bg_rgb`
/// resolve the Use Layer and Use Background colours; `weight_k` is the
/// line-weight factor of the view.
#[allow(clippy::too_many_arguments)]
pub fn paint_fill(
    painter: &egui::Painter,
    to_screen: &dyn Fn(Point) -> Pos2,
    px_per_in: f32,
    outer: &[Point],
    holes: &[Vec<Point>],
    style: &FillStyle,
    patterns: &[CustomPattern],
    layer_rgb: [u8; 3],
    bg_rgb: [u8; 3],
    weight_k: f32,
) {
    if outer.len() < 3 {
        return;
    }
    let ink = style.color.resolve(layer_rgb, bg_rgb);
    let a = alpha(style.transparency);
    // The pattern background goes under the lines.
    if let Some(bg) = &style.background {
        if !style.is_solid() {
            if let Some(g) = &bg.gradient {
                if let Some(m) = gradient_mesh(to_screen, outer, holes, g, 1.0 - bg.transparency) {
                    painter.add(Shape::mesh(m));
                }
            } else if let Some(m) = solid_mesh(
                to_screen,
                outer,
                holes,
                color32(bg.color.resolve(layer_rgb, bg_rgb), alpha(bg.transparency)),
            ) {
                painter.add(Shape::mesh(m));
            }
        }
    }
    // Patterns finer than a couple of pixels read as a tint.
    let cell = style.width.min(style.height).max(0.01) as f32 * px_per_in;
    let fine = !style.is_solid() && cell < 2.0 && !matches!(style.pattern, PatternType::UseLayer);
    if style.is_solid() || fine {
        let tint = if fine { a / 2 } else { a };
        let m = match &style.gradient {
            Some(g) => gradient_mesh(to_screen, outer, holes, g, f64::from(tint) / 255.0),
            None => solid_mesh(to_screen, outer, holes, color32(ink, tint)),
        };
        if let Some(m) = m {
            painter.add(Shape::mesh(m));
        }
        return;
    }
    let geom = fill_geometry(style, outer, holes, patterns);
    if geom.solid {
        return;
    }
    let width = line_px(style.line_weight, weight_k);
    let (lo, hi) = outer.iter().fold((outer[0], outer[0]), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    });
    let color_for = |p: Point| -> Color32 {
        match &style.gradient {
            Some(g) => {
                let (c, tr) = g.sample(g.position_at(p, lo, hi));
                color32(c, alpha(1.0 - (1.0 - tr) * (1.0 - style.transparency)))
            }
            None => color32(ink, a),
        }
    };
    let mut shapes = Vec::with_capacity(geom.lines.len() + geom.dots.len());
    for (p, q) in &geom.lines {
        shapes.push(Shape::line_segment(
            [to_screen(*p), to_screen(*q)],
            Stroke::new(width, color_for(Point::lerp(*p, *q, 0.5))),
        ));
    }
    for d in &geom.dots {
        shapes.push(Shape::circle_filled(
            to_screen(*d),
            (width * 0.9).max(0.8),
            color_for(*d),
        ));
    }
    painter.extend(shapes);
}

// ---------------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------------

/// What happened in the panel this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelOut {
    pub changed: bool,
    /// The Add to Library button was clicked.
    pub add_to_library: bool,
}

fn type_label(t: &PatternType) -> String {
    t.label()
}

fn color_source(ui: &mut egui::Ui, salt: &str, src: &mut ColorSource, layer: [u8; 3]) -> bool {
    let mut changed = false;
    let mut rgb = match src {
        ColorSource::Single(c) => *c,
        ColorSource::Layer => layer,
        ColorSource::Background => [255, 255, 255],
    };
    ui.horizontal(|ui| {
        let single = matches!(src, ColorSource::Single(_));
        if ui.radio(single, "Single Color").clicked() && !single {
            *src = ColorSource::Single(rgb);
            changed = true;
        }
        // Clicking the colour bar chooses Custom Color automatically.
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            *src = ColorSource::Single(rgb);
            changed = true;
        }
        if ui
            .radio(matches!(src, ColorSource::Layer), "Use Layer Color")
            .clicked()
        {
            *src = ColorSource::Layer;
            changed = true;
        }
        if ui
            .radio(
                matches!(src, ColorSource::Background),
                "Use Background Color",
            )
            .clicked()
        {
            *src = ColorSource::Background;
            changed = true;
        }
    });
    let _ = salt;
    changed
}

fn gradient_editor(ui: &mut egui::Ui, g: &mut Gradient) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Type");
        egui::ComboBox::from_id_salt("fill_gradient_kind")
            .selected_text(match g.kind {
                GradientKind::Linear => "Linear",
                GradientKind::Radial => "Radial",
            })
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(&mut g.kind, GradientKind::Linear, "Linear")
                    .changed();
                changed |= ui
                    .selectable_value(&mut g.kind, GradientKind::Radial, "Radial")
                    .changed();
            });
        if g.kind == GradientKind::Linear {
            changed |= ui
                .add(
                    egui::DragValue::new(&mut g.angle_deg)
                        .speed(1.0)
                        .suffix("\u{b0}"),
                )
                .changed();
        }
    });
    let mut remove = None;
    let can_remove = g.points.len() >= 3;
    for (i, p) in g.points.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            changed |= ui.color_edit_button_srgb(&mut p.color).changed();
            let mut tr = (p.transparency * 100.0) as f32;
            if ui
                .add(egui::Slider::new(&mut tr, 0.0..=100.0).text("Transparency %"))
                .changed()
            {
                p.transparency = f64::from(tr) / 100.0;
                changed = true;
            }
            let mut pos = (p.position * 100.0) as f32;
            if ui
                .add(egui::Slider::new(&mut pos, 0.0..=100.0).text("Position %"))
                .changed()
            {
                p.position = f64::from(pos) / 100.0;
                changed = true;
            }
            if ui
                .add_enabled(can_remove, egui::Button::new("Remove Color"))
                .clicked()
            {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        g.remove_color(i);
        changed = true;
    }
    ui.horizontal(|ui| {
        if ui.button("Add Color").clicked() {
            g.add_color();
            changed = true;
        }
        if ui.button("Reset").clicked() {
            g.reset();
            changed = true;
        }
    });
    changed
}

/// The Fill Style panel. `patterns` are the library patterns the Type list
/// offers; `preview_width` is the plan inches across the preview.
pub fn panel(
    ui: &mut egui::Ui,
    salt: &str,
    style: &mut FillStyle,
    patterns: &[CustomPattern],
    layer_rgb: [u8; 3],
    preview_width: &mut f64,
    show_add_to_library: bool,
) -> PanelOut {
    let mut out = PanelOut::default();
    section(ui, "Pattern");
    row(ui, "Type", |ui| {
        egui::ComboBox::from_id_salt(("fill_type", salt))
            .selected_text(type_label(&style.pattern))
            .width(170.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(style.pattern == PatternType::UseLayer, "Use Layer")
                    .clicked()
                {
                    style.pattern = PatternType::UseLayer;
                    out.changed = true;
                }
                for sp in SystemPattern::ALL {
                    let t = PatternType::System(sp);
                    if ui
                        .selectable_label(style.pattern == t, sp.label())
                        .clicked()
                    {
                        style.pattern = t;
                        out.changed = true;
                    }
                }
                if !patterns.is_empty() {
                    ui.separator();
                }
                for p in patterns {
                    let t = PatternType::Library(p.name.clone());
                    if ui.selectable_label(style.pattern == t, &p.name).clicked() {
                        style.pattern = t;
                        out.changed = true;
                    }
                }
            });
    });
    let drag = |ui: &mut egui::Ui, v: &mut f64, speed: f64, lo: f64, hi: f64| -> bool {
        ui.add(egui::DragValue::new(v).speed(speed).range(lo..=hi))
            .changed()
    };
    match style.pattern.clone() {
        PatternType::UseLayer => {
            ui.weak("The object uses the fill style of its layer.");
        }
        PatternType::System(SystemPattern::Solid) => {}
        PatternType::System(sp) => {
            section(ui, "Scale");
            if sp.has_row_offset() {
                row(ui, "Row Offset", |ui| {
                    let mut pct = style.row_offset * 100.0;
                    if drag(ui, &mut pct, 1.0, 0.0, 100.0) {
                        style.row_offset = pct / 100.0;
                        out.changed = true;
                    }
                    ui.label("%");
                });
            }
            row(
                ui,
                if sp.width_is_spacing() {
                    "Spacing"
                } else {
                    "Width"
                },
                |ui| {
                    out.changed |= drag(ui, &mut style.width, 0.25, 0.05, 2000.0);
                },
            );
            if sp.has_height() {
                row(ui, "Height", |ui| {
                    out.changed |= drag(ui, &mut style.height, 0.25, 0.05, 2000.0);
                });
            }
        }
        PatternType::Library(_) => {
            section(ui, "Scale");
            row(ui, "X Scale", |ui| {
                out.changed |= drag(ui, &mut style.x_scale, 0.05, 0.01, 100.0);
            });
            row(ui, "Y Scale", |ui| {
                out.changed |= drag(ui, &mut style.y_scale, 0.05, 0.01, 100.0);
            });
        }
    }
    if !matches!(
        style.pattern,
        PatternType::UseLayer | PatternType::System(SystemPattern::Solid)
    ) {
        section(ui, "Offsets and Angle");
        row(ui, "Horizontal Offset", |ui| {
            out.changed |= drag(ui, &mut style.h_offset, 0.25, -2000.0, 2000.0);
        });
        row(ui, "Vertical Offset", |ui| {
            out.changed |= drag(ui, &mut style.v_offset, 0.25, -2000.0, 2000.0);
        });
        row(ui, "Angle", |ui| {
            out.changed |= drag(ui, &mut style.angle_deg, 1.0, -360.0, 360.0);
        });
        section(ui, "Pattern Appearance");
        row(ui, "Line Weight", |ui| {
            let mut w = f64::from(style.line_weight);
            if drag(ui, &mut w, 1.0, 1.0, 300.0) {
                style.line_weight = w as u32;
                out.changed = true;
            }
            ui.label("(1/100 mm)");
        });
    } else if style.is_solid() {
        section(ui, "Pattern Appearance");
    }
    if style.pattern != PatternType::UseLayer {
        let mut gradient_on = style.gradient.is_some();
        ui.horizontal(|ui| {
            if ui.radio(!gradient_on, "Single Color").clicked() && gradient_on {
                style.gradient = None;
                gradient_on = false;
                out.changed = true;
            }
            if ui.radio(gradient_on, "Gradient").clicked() && !gradient_on {
                style.gradient = Some(Gradient::default());
                out.changed = true;
            }
        });
        if let Some(g) = style.gradient.as_mut() {
            out.changed |= gradient_editor(ui, g);
        } else {
            out.changed |= color_source(ui, salt, &mut style.color, layer_rgb);
        }
        row(ui, "Transparency", |ui| {
            let mut t = (style.transparency * 100.0) as f32;
            if ui
                .add(egui::Slider::new(&mut t, 0.0..=100.0).suffix("%"))
                .changed()
            {
                style.transparency = f64::from(t) / 100.0;
                out.changed = true;
            }
        });
        if !style.is_solid() {
            section(ui, "Background");
            let mut on = style.background.is_some();
            if ui.checkbox(&mut on, "Background").changed() {
                style.background = on.then(plan_core::fill_styles::Background::default);
                out.changed = true;
            }
            if let Some(bg) = style.background.as_mut() {
                out.changed |= color_source(ui, "bg", &mut bg.color, layer_rgb);
                row(ui, "Transparency", |ui| {
                    let mut t = (bg.transparency * 100.0) as f32;
                    if ui
                        .add(egui::Slider::new(&mut t, 0.0..=100.0).suffix("%"))
                        .changed()
                    {
                        bg.transparency = f64::from(t) / 100.0;
                        out.changed = true;
                    }
                });
            }
        }
    }
    if show_add_to_library {
        ui.add_space(4.0);
        if ui.button("Add to Library").clicked() {
            out.add_to_library = true;
        }
    }
    section(ui, "Preview");
    row(ui, "Preview Width", |ui| {
        egui::ComboBox::from_id_salt(("fill_preview_width", salt))
            .selected_text(plan_core::units::fmt_ft_in(*preview_width))
            .show_ui(ui, |ui| {
                for w in PREVIEW_WIDTHS {
                    ui.selectable_value(preview_width, w, plan_core::units::fmt_ft_in(w));
                }
            });
    });
    preview(ui, style, patterns, layer_rgb, *preview_width);
    out
}

/// The preview swatch: `width` plan inches across.
pub fn preview(
    ui: &mut egui::Ui,
    style: &FillStyle,
    patterns: &[CustomPattern],
    layer_rgb: [u8; 3],
    width: f64,
) {
    let size = Vec2::new(210.0, 130.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bg = Color32::from_rgb(0xEC, 0xEA, 0xE3);
    painter.rect_filled(rect, 2.0, bg);
    let k = f64::from(rect.width()) / width.max(1.0);
    let h = f64::from(rect.height()) / k;
    let outer = vec![
        Point::new(0.0, 0.0),
        Point::new(width, 0.0),
        Point::new(width, h),
        Point::new(0.0, h),
    ];
    let to_screen = |p: Point| {
        Pos2::new(
            rect.left() + (p.x * k) as f32,
            rect.bottom() - (p.y * k) as f32,
        )
    };
    let style = if style.pattern == PatternType::UseLayer {
        FillStyle::solid(layer_rgb)
    } else {
        style.clone()
    };
    paint_fill(
        &painter,
        &to_screen,
        k as f32,
        &outer,
        &[],
        &style,
        patterns,
        layer_rgb,
        [0xEC, 0xEA, 0xE3],
        1.0,
    );
}

// ---------------------------------------------------------------------------
// The dialog
// ---------------------------------------------------------------------------

/// The Fill Style dialog: a named library fill style, or the style for the
/// selected objects.
pub struct FillDialog {
    /// The Fill Style Name (named mode only).
    pub name: String,
    pub named: bool,
    pub style: FillStyle,
    pub targets: Vec<FillTarget>,
    pub preview_width: f64,
    patterns: Vec<CustomPattern>,
    layer_rgb: [u8; 3],
    /// Add to Library clicked: save the style under `name` in the User Catalog.
    pub add_to_library: bool,
}

impl FillDialog {
    pub fn error(&self) -> Option<String> {
        (self.named && self.name.trim().is_empty()).then(|| "A fill style needs a name".to_string())
    }

    fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.error();
        let title = if self.named {
            "Fill Style Specification"
        } else {
            "Fill Style"
        };
        egui::Window::new(title)
            .id(egui::Id::new("fill_style_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(520.0);
                if self.named {
                    row(ui, "Fill Style Name", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(220.0));
                    });
                } else {
                    ui.weak(format!("{} object(s) selected", self.targets.len()));
                }
                let out = panel(
                    ui,
                    "dialog",
                    &mut self.style,
                    &self.patterns,
                    self.layer_rgb,
                    &mut self.preview_width,
                    !self.named,
                );
                if out.add_to_library {
                    self.add_to_library = true;
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if let Some(e) = &error {
                        ui.colored_label(Color32::from_rgb(0xFF, 0x7B, 0x7B), e);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                error.is_none(),
                                egui::Button::new(RichText::new("   OK   ").strong()),
                            )
                            .clicked()
                        {
                            outcome = Outcome::Ok;
                        }
                        if ui.button("Cancel").clicked() {
                            outcome = Outcome::Cancel;
                        }
                    });
                });
            });
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }
}

/// Applies an accepted draft as one undo step.
pub fn apply(cx: &mut EditorContext, d: &FillDialog) {
    if d.named {
        cx.begin_change("Fill Style Specification");
        let name = d.name.trim().to_string();
        let s = &mut cx.project.styles;
        match s.fill_styles.iter_mut().find(|f| f.name == name) {
            Some(f) => f.style = d.style.clone(),
            None => s.fill_styles.push(plan_core::fill_styles::NamedFill {
                name: name.clone(),
                style: d.style.clone(),
            }),
        }
        s.add_fill_to_library(&name, d.style.clone());
        cx.status = format!("Fill style {name} saved in the library");
    } else {
        cx.begin_change("Fill Style");
        for t in &d.targets {
            cx.project
                .styles
                .apply_fill(t.clone(), Some(d.style.clone()));
        }
        if d.add_to_library {
            let name = format!("Fill {}", cx.project.styles.user_fills.len() + 1);
            cx.project
                .styles
                .add_fill_to_library(&name, d.style.clone());
        }
        cx.status = format!("Fill style applied to {} object(s)", d.targets.len());
    }
    cx.mark_dirty();
}

/// A fill style the Fill Style Specification dialog starts from.
pub fn open_named(cx: &EditorContext) {
    let d = FillDialog {
        name: "New Fill Style".into(),
        named: true,
        style: FillStyle::hatch(45.0, 6.0, [0, 0, 0]),
        targets: Vec::new(),
        preview_width: 48.0,
        patterns: cx.project.styles.all_patterns(),
        layer_rgb: [0, 0, 0],
        add_to_library: false,
    };
    HOST.with(|h| *h.borrow_mut() = Some(d));
}

/// The targets of the selected objects (closed CAD shapes, slabs, rooms,
/// walls).
pub fn selection_targets(cx: &EditorContext) -> Vec<FillTarget> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| target_of(cx, *o))
        .collect()
}

/// Opens the Fill Style dialog for the selected objects; false when nothing
/// selected takes a fill.
pub fn open_for_selection(cx: &mut EditorContext) -> bool {
    let targets = selection_targets(cx);
    let Some(first) = targets.first() else {
        cx.status = "Select a closed CAD shape, slab, room or wall to fill".into();
        return false;
    };
    let style = cx
        .project
        .styles
        .fill_for(first)
        .cloned()
        .or_else(|| legacy_fill(cx, first))
        .unwrap_or_else(|| FillStyle::hatch(45.0, 6.0, [0, 0, 0]));
    let d = FillDialog {
        name: String::new(),
        named: false,
        style,
        targets,
        preview_width: 48.0,
        patterns: cx.project.styles.all_patterns(),
        layer_rgb: [0, 0, 0],
        add_to_library: false,
    };
    HOST.with(|h| *h.borrow_mut() = Some(d));
    true
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut FillDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(d) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    if d.error().is_some() {
        return false;
    }
    apply(cx, &d);
    true
}

/// Shows the open dialog once a frame and applies its OK.
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => apply(cx, &d),
    }
}

// ---------------------------------------------------------------------------
// Painter and Eyedropper
// ---------------------------------------------------------------------------

/// The slabs of the active floor.
pub fn slabs(cx: &EditorContext) -> Vec<plan_core::foundation::Slab> {
    cx.floor()
        .foundation_as::<plan_core::foundation::FoundationLayer>()
        .ok()
        .flatten()
        .map(|f| f.slabs)
        .unwrap_or_default()
}

/// Is the CAD item a closed shape that takes a fill?
fn closed_shape(item: &CadItem) -> bool {
    matches!(
        item,
        CadItem::Polyline { closed: true, .. } | CadItem::Circle { .. }
    )
}

/// The fill target of `o`, when it takes a fill.
pub fn target_of(cx: &EditorContext, o: ObjectRef) -> Option<FillTarget> {
    match o {
        ObjectRef::Cad(id) => cx
            .floor()
            .cad
            .iter()
            .find(|c| c.id == id)
            .filter(|c| closed_shape(&c.item))
            .map(|_| FillTarget::Cad(id)),
        ObjectRef::Foundation(id) => slabs(cx)
            .iter()
            .find(|s| s.id == id)
            .map(|_| FillTarget::Slab(id)),
        ObjectRef::Room(i) => cx
            .rooms
            .get(i)
            .map(|r| FillTarget::Room(crate::editor::rooms_edit::room_anchor(r))),
        ObjectRef::Wall(id) => {
            let w = cx.floor().wall(id)?;
            let name = w.wall_type.clone()?;
            let ty = cx
                .project
                .wall_types
                .iter()
                .chain(cx.defaults.wall_types.iter())
                .find(|t| t.name == name)?;
            let idx = ty.layers.iter().position(|l| l.is_main).unwrap_or(0);
            Some(FillTarget::WallLayer {
                wall_type: name,
                index: idx,
            })
        }
        _ => None,
    }
}

/// The fill an object has in its own fields (what the plan drew before a
/// shared style was assigned), as a shared style.
fn legacy_fill(cx: &EditorContext, t: &FillTarget) -> Option<FillStyle> {
    match t {
        FillTarget::Cad(id) => cx
            .floor()
            .cad_attrs(*id)
            .and_then(|a| a.fill)
            .and_then(|f| FillStyle::from_fill_attr(&f)),
        FillTarget::Slab(id) => {
            let all = slabs(cx);
            let s = all.iter().find(|s| s.id == *id)?;
            FillStyle::from_legacy(&s.fill_pattern, s.fill_color, 1.0)
        }
        _ => None,
    }
}

/// Fill Style Eyedropper: the fill style `o` has, assigned or its own.
pub fn read_fill(cx: &EditorContext, o: ObjectRef) -> Option<FillStyle> {
    let t = target_of(cx, o)?;
    cx.project
        .styles
        .read_fill(&t)
        .or_else(|| legacy_fill(cx, &t))
}

/// Fill Style Painter: paints `style` on `o`. The caller opens the undo step
/// (`cx.begin_change`) so a stroke over several objects is one step.
pub fn apply_fill(cx: &mut EditorContext, o: ObjectRef, style: &FillStyle) -> bool {
    let Some(t) = target_of(cx, o) else {
        return false;
    };
    cx.project.styles.apply_fill(t, Some(style.clone()));
    cx.mark_dirty();
    true
}

/// Run a Fill Style command by id; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        APPLY => {
            open_for_selection(cx);
        }
        NEW_NAMED => open_named(cx),
        POCHE => {
            toggle_poche(cx);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::cad::FillAttr;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn square(cx: &mut EditorContext) -> plan_core::Id {
        cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Polyline {
                points: vec![
                    Point::ZERO,
                    Point::new(96.0, 0.0),
                    Point::new(96.0, 96.0),
                    Point::new(0.0, 96.0),
                ],
                closed: true,
            },
        )
    }

    #[test]
    fn the_dialog_paints_a_fill_style_on_the_selection_in_one_undo_step() {
        let mut cx = cx();
        let a = square(&mut cx);
        let line = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(10.0, 0.0),
            },
        );
        cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(line)];
        assert_eq!(
            selection_targets(&cx),
            vec![FillTarget::Cad(a)],
            "an open line takes no fill"
        );
        assert!(open_for_selection(&mut cx));
        with_dialog(|d| {
            d.style = FillStyle::system(SystemPattern::Brick, 8.0, 2.25);
            d.style.angle_deg = 15.0;
            d.style.background = Some(plan_core::fill_styles::Background::default());
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        assert_eq!(cx.undo_label(), Some("Fill Style"));
        let s = cx.project.styles.fill_for(&FillTarget::Cad(a)).unwrap();
        assert_eq!(s.pattern, PatternType::System(SystemPattern::Brick));
        assert_eq!(s.angle_deg, 15.0);
        assert!(s.background.is_some());
        cx.undo();
        assert!(cx.project.styles.fill_for(&FillTarget::Cad(a)).is_none());
        // Nothing fillable selected: no dialog.
        cx.selection.items = vec![ObjectRef::Cad(line)];
        assert!(!open_for_selection(&mut cx));
    }

    #[test]
    fn a_named_fill_style_goes_to_the_file_and_the_user_catalog() {
        let mut cx = cx();
        open_named(&cx);
        with_dialog(|d| {
            d.name = "Wood Floor".into();
            d.style = FillStyle::system(SystemPattern::Herringbone, 6.0, 12.0);
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        assert_eq!(cx.undo_label(), Some("Fill Style Specification"));
        assert_eq!(cx.project.styles.fill_styles[0].name, "Wood Floor");
        assert_eq!(cx.project.styles.user_fills[0].name, "Wood Floor");
        // A blank name blocks OK.
        open_named(&cx);
        with_dialog(|d| d.name = " ".into()).unwrap();
        assert!(!accept_dialog(&mut cx));
        HOST.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn the_eyedropper_reads_assigned_and_old_fills_and_the_painter_writes_them() {
        let mut cx = cx();
        let a = square(&mut cx);
        let b = square(&mut cx);
        // An old CAD fill is read in the shared type.
        cx.project.edit_cad_attrs(0, a, |at| {
            at.fill = Some(FillAttr {
                pattern: "Cross Hatch".into(),
                spacing: 9.0,
                ..FillAttr::default()
            })
        });
        let got = read_fill(&cx, ObjectRef::Cad(a)).unwrap();
        assert_eq!(got.pattern, PatternType::System(SystemPattern::CrossHatch));
        assert_eq!(got.width, 9.0);
        assert!(read_fill(&cx, ObjectRef::Cad(b)).is_none());
        cx.begin_change("Fill Style Painter");
        assert!(apply_fill(&mut cx, ObjectRef::Cad(b), &got));
        assert_eq!(read_fill(&cx, ObjectRef::Cad(b)), Some(got));
        assert!(!apply_fill(
            &mut cx,
            ObjectRef::Opening(1),
            &FillStyle::default()
        ));
    }

    #[test]
    fn the_panel_the_dialog_and_every_fill_kind_draw() {
        let mut cx = cx();
        let _ = square(&mut cx);
        open_named(&cx);
        let ctx = egui::Context::default();
        let mut width = 48.0;
        let mut styles = vec![FillStyle::default(), FillStyle::solid([200, 10, 10])];
        for sp in SystemPattern::ALL {
            styles.push(FillStyle::system(sp, 6.0, 4.0));
        }
        let mut grad = FillStyle::solid([1, 2, 3]);
        grad.gradient = Some(Gradient::default());
        styles.push(grad);
        let mut bg = FillStyle::hatch(0.0, 6.0, [0; 3]);
        bg.background = Some(plan_core::fill_styles::Background::default());
        styles.push(bg);
        let mut pat = CustomPattern::new("Slash");
        pat.groups[0].tile = vec![(Point::ZERO, Point::new(6.0, 6.0))];
        pat.groups[0].width = 6.0;
        pat.groups[0].height = 6.0;
        let pats = vec![pat];
        styles.push(FillStyle::library("Slash"));
        for mut s in styles {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                host_frame(&mut cx, ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = panel(ui, "t", &mut s, &pats, [0, 0, 0], &mut width, true);
                });
            });
        }
        HOST.with(|h| *h.borrow_mut() = None);
    }
}
