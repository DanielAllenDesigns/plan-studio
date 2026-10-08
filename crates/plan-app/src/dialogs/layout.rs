//! The layout dialogs (docs/parity/documentation-layout.md, L-2, L-4, L-7):
//! Send to Layout, Layout Box Specification, Page Setup, the Layout Page Table
//! and Print. (Project Information is the schedules builder's dialog,
//! `dialogs::project_info`.)
//!
//! Each dialog edits a draft and hands the result back to
//! `shell::layout_window`, which applies it to the project's layout.

use super::{row, section, Outcome, ERROR_RED};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText, Ui};
use plan_core::{Id, Point};
use plan_docs::{Scale, SheetSize};
use plan_layout::{BoxSource, LayoutBox};

/// Shared dialog frame: a centered window with an OK / Cancel row. Enter is
/// OK (unless a text field has focus) and Esc is Cancel.
pub(super) fn frame(
    ctx: &egui::Context,
    title: &str,
    width: f32,
    error: Option<&str>,
    body: impl FnOnce(&mut Ui),
) -> Outcome {
    let mut outcome = Outcome::Open;
    let mut open = true;
    egui::Window::new(title)
        .id(egui::Id::new(("layout_dialog", title)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(width);
            body(ui);
            ui.add_space(8.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ok = egui::Button::new(RichText::new("   OK   ").strong());
                if ui.add_enabled(error.is_none(), ok).clicked() {
                    outcome = Outcome::Ok;
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancel;
                }
                if let Some(e) = error {
                    ui.colored_label(ERROR_RED, e);
                }
            });
        });
    if !open {
        outcome = Outcome::Cancel;
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        outcome = Outcome::Cancel;
    } else if error.is_none()
        && !ctx.wants_keyboard_input()
        && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
    {
        outcome = Outcome::Ok;
    }
    outcome
}

/// A paper-inch drag field.
fn inches(ui: &mut Ui, v: &mut f64) {
    ui.add(
        egui::DragValue::new(v)
            .speed(0.05)
            .max_decimals(3)
            .suffix("\""),
    );
}

fn scale_combo(ui: &mut Ui, salt: &str, current: &mut Scale) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            for s in Scale::ALL {
                ui.selectable_value(current, s, s.label());
            }
            if !Scale::ALL.contains(current) {
                let c = *current;
                ui.selectable_value(current, c, c.label());
            }
        });
}

/// `(sheet number, title)` of every page, for the page drop-downs.
pub type PageList = Vec<(u32, String)>;

fn page_label(p: &(u32, String)) -> String {
    format!("A-{}  {}", p.0, p.1)
}

// ---------------------------------------------------------------- send --

/// What Send to Layout sends.
#[derive(Clone, Debug, PartialEq)]
pub enum SendSource {
    /// The current plan view: a floor and the layer set it shows.
    Plan { floor: usize, layer_set: String },
    /// An elevation or section camera.
    Camera { id: Id, name: String },
    /// A perspective (full camera) view, ray traced at low samples.
    Perspective { id: Id, name: String },
}

/// Which page receives the box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageChoice {
    Existing(u32),
    /// A new page after the last one.
    New,
}

/// Where on the page the box goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The first free area of the drawing area.
    FirstFree,
    /// Centered in the drawing area.
    Centered,
    /// Wherever the next click on the page lands.
    Click,
}

/// The Send to Layout answers.
#[derive(Clone, Debug, PartialEq)]
pub struct SendSpec {
    pub source: SendSource,
    pub page: PageChoice,
    /// `None` is "largest that fits".
    pub scale: Option<Scale>,
    pub placement: Placement,
}

/// Send to Layout (L-2).
pub struct SendDialog {
    spec: SendSpec,
    pages: PageList,
    floors: Vec<String>,
    layer_sets: Vec<String>,
}

impl SendDialog {
    pub fn new(
        source: SendSource,
        pages: PageList,
        current_page: Option<u32>,
        floors: Vec<String>,
        layer_sets: Vec<String>,
    ) -> Self {
        let page = current_page
            .filter(|n| pages.iter().any(|p| p.0 == *n))
            .or_else(|| pages.first().map(|p| p.0))
            .map_or(PageChoice::New, PageChoice::Existing);
        Self {
            spec: SendSpec {
                source,
                page,
                scale: None,
                placement: Placement::FirstFree,
            },
            pages,
            floors,
            layer_sets,
        }
    }

    pub fn spec(&self) -> &SendSpec {
        &self.spec
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let Self {
            spec,
            pages,
            floors,
            layer_sets,
        } = self;
        frame(ctx, "Send to Layout", 380.0, None, |ui| {
            section(ui, "View");
            match &mut spec.source {
                SendSource::Plan { floor, layer_set } => {
                    row(ui, "Floor plan", |ui| {
                        let shown = floors.get(*floor).cloned().unwrap_or_default();
                        egui::ComboBox::from_id_salt("send_floor")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (i, n) in floors.iter().enumerate() {
                                    ui.selectable_value(floor, i, n);
                                }
                            });
                    });
                    row(ui, "Layer set", |ui| {
                        egui::ComboBox::from_id_salt("send_layer_set")
                            .selected_text(layer_set.clone())
                            .show_ui(ui, |ui| {
                                for n in layer_sets.iter() {
                                    ui.selectable_value(layer_set, n.clone(), n);
                                }
                            });
                    });
                }
                SendSource::Camera { name, .. } => {
                    row(ui, "Camera view", |ui| ui.label(name.as_str()));
                }
                SendSource::Perspective { name, .. } => {
                    row(ui, "Perspective view", |ui| ui.label(name.as_str()));
                    ui.weak("Rendered at low quality for the page; Update Views renders it again.");
                }
            }
            section(ui, "Layout page");
            row(ui, "Page", |ui| {
                let shown = match spec.page {
                    PageChoice::Existing(n) => pages
                        .iter()
                        .find(|p| p.0 == n)
                        .map_or_else(|| format!("A-{n}"), page_label),
                    PageChoice::New => "New page".to_string(),
                };
                egui::ComboBox::from_id_salt("send_page")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for p in pages.iter() {
                            ui.selectable_value(
                                &mut spec.page,
                                PageChoice::Existing(p.0),
                                page_label(p),
                            );
                        }
                        ui.selectable_value(&mut spec.page, PageChoice::New, "New page");
                    });
            });
            row(ui, "Scale", |ui| {
                let shown = spec
                    .scale
                    .map_or("Largest that fits".to_string(), |s| s.label().to_string());
                egui::ComboBox::from_id_salt("send_scale")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut spec.scale, None, "Largest that fits");
                        for s in Scale::ALL {
                            ui.selectable_value(&mut spec.scale, Some(s), s.label());
                        }
                    });
            });
            row(ui, "Position", |ui| {
                ui.radio_value(&mut spec.placement, Placement::FirstFree, "First free area");
                ui.radio_value(&mut spec.placement, Placement::Centered, "Centered");
                ui.radio_value(&mut spec.placement, Placement::Click, "Click on page");
            });
        })
    }
}

// ----------------------------------------------------------------- box --

/// The tabs of the Layout Box Specification.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoxTab {
    General,
    Source,
    LineStyle,
}

/// The Layout Box Specification answers: the edited box and its page.
#[derive(Clone, Debug, PartialEq)]
pub struct BoxSpec {
    pub layout_box: LayoutBox,
    pub page: u32,
}

/// Layout Box Specification (L-4).
pub struct BoxSpecDialog {
    draft: LayoutBox,
    page: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label: String,
    pages: PageList,
    floors: Vec<String>,
    layer_sets: Vec<String>,
    tab: BoxTab,
}

impl BoxSpecDialog {
    pub fn new(
        b: &LayoutBox,
        page: u32,
        pages: PageList,
        floors: Vec<String>,
        layer_sets: Vec<String>,
    ) -> Self {
        let (a, c) = b.rect_in;
        Self {
            draft: b.clone(),
            page,
            x: a.x.min(c.x),
            y: a.y.min(c.y),
            w: (c.x - a.x).abs(),
            h: (c.y - a.y).abs(),
            label: b.label.clone().unwrap_or_default(),
            pages,
            floors,
            layer_sets,
            tab: BoxTab::General,
        }
    }

    /// The box with the dialog's fields written back.
    pub fn result(&self) -> BoxSpec {
        let mut b = self.draft.clone();
        b.rect_in = (
            Point::new(self.x, self.y),
            Point::new(self.x + self.w, self.y + self.h),
        );
        b.label = (!self.label.trim().is_empty()).then(|| self.label.clone());
        BoxSpec {
            layout_box: b,
            page: self.page,
        }
    }

    fn error(&self) -> Option<&'static str> {
        (self.w < 0.05 || self.h < 0.05).then_some("The box must be at least 0.05\" square")
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        frame(ctx, "Layout Box Specification", 420.0, error, |ui| {
            ui.horizontal(|ui| {
                for (t, name) in [
                    (BoxTab::General, "General"),
                    (BoxTab::Source, "Source"),
                    (BoxTab::LineStyle, "Line Style"),
                ] {
                    ui.selectable_value(&mut self.tab, t, name);
                }
            });
            ui.separator();
            match self.tab {
                BoxTab::General => self.general(ui),
                BoxTab::Source => self.source(ui),
                BoxTab::LineStyle => self.line_style(ui),
            }
        })
    }

    fn general(&mut self, ui: &mut Ui) {
        let b = &mut self.draft;
        row(ui, "Label", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.label).desired_width(220.0));
        });
        row(ui, "Scale", |ui| scale_combo(ui, "box_scale", &mut b.scale));
        row(ui, "Page", |ui| {
            let shown = self
                .pages
                .iter()
                .find(|p| p.0 == self.page)
                .map_or_else(|| format!("A-{}", self.page), page_label);
            egui::ComboBox::from_id_salt("box_page")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for p in &self.pages {
                        ui.selectable_value(&mut self.page, p.0, page_label(p));
                    }
                });
        });
        section(ui, "Position and size");
        row(ui, "Left / Bottom", |ui| {
            inches(ui, &mut self.x);
            inches(ui, &mut self.y);
        });
        row(ui, "Width / Height", |ui| {
            inches(ui, &mut self.w);
            inches(ui, &mut self.h);
        });
        row(ui, "Rotation", |ui| {
            // The content turns counter-clockwise in quarter turns.
            let mut turns = b.quarter_turns();
            egui::ComboBox::from_id_salt("box_rotation")
                .selected_text(format!("{}\u{b0}", u32::from(turns) * 90))
                .show_ui(ui, |ui| {
                    for t in 0..4u8 {
                        ui.selectable_value(&mut turns, t, format!("{}\u{b0}", u32::from(t) * 90));
                    }
                });
            b.rotation_deg = f64::from(turns) * 90.0;
        });
        ui.checkbox(&mut b.border, "Draw border");
        ui.checkbox(&mut b.clip, "Clip content to the box");
    }

    fn source(&mut self, ui: &mut Ui) {
        match &mut self.draft.source {
            BoxSource::PlanView { floor, layer_set } => {
                row(ui, "Floor plan", |ui| {
                    let shown = self.floors.get(*floor).cloned().unwrap_or_default();
                    egui::ComboBox::from_id_salt("box_floor")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for (i, n) in self.floors.iter().enumerate() {
                                ui.selectable_value(floor, i, n);
                            }
                        });
                });
                row(ui, "Layer set", |ui| {
                    egui::ComboBox::from_id_salt("box_layer_set")
                        .selected_text(layer_set.clone())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(layer_set, "All".to_string(), "All");
                            for n in &self.layer_sets {
                                ui.selectable_value(layer_set, n.clone(), n);
                            }
                        });
                });
                ui.weak("\"All\" ignores layer visibility.");
            }
            BoxSource::Text {
                text,
                height_pt,
                align,
                bold,
            } => {
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(4)
                        .desired_width(380.0),
                );
                row(ui, "Text height", |ui| {
                    ui.add(
                        egui::DragValue::new(height_pt)
                            .speed(0.25)
                            .range(2.0..=200.0)
                            .suffix(" pt"),
                    );
                });
                row(ui, "Alignment", |ui| align_buttons(ui, align));
                row(ui, "Text fit", |ui| {
                    fit_buttons(ui, &mut self.draft.text_fit)
                });
                ui.checkbox(bold, "Bold");
                ui.weak(fit_help(self.draft.text_fit));
            }
            BoxSource::Perspective { camera_id } => {
                ui.label(format!("Perspective view of camera {camera_id}"));
                let b = &mut self.draft;
                row(ui, "Resolution", |ui| {
                    // 0 is the default (80 dpi: a 6" x 4.5" box renders at 480 x 360).
                    let mut dpi = b.effective_dpi();
                    ui.add(
                        egui::DragValue::new(&mut dpi)
                            .range(20..=600)
                            .suffix(" dpi"),
                    );
                    b.dpi = if dpi == plan_layout::DEFAULT_PERSPECTIVE_DPI {
                        0
                    } else {
                        dpi
                    };
                });
                row(ui, "Quality", |ui| {
                    let mut samples = b.effective_samples();
                    ui.add(
                        egui::DragValue::new(&mut samples)
                            .range(1..=512)
                            .suffix(" samples"),
                    );
                    b.samples = if samples == plan_layout::DEFAULT_PERSPECTIVE_SAMPLES {
                        0
                    } else {
                        samples
                    };
                });
                let (w, h) = (self.w, self.h);
                let (px_w, px_h) = plan_layout::perspective_pixels(w, h, b.dpi);
                ui.weak(format!(
                    "Renders {px_w} x {px_h} pixels; Update Views renders it again."
                ));
            }
            BoxSource::Materials { floor, category } => {
                row(ui, "Floors", |ui| {
                    let shown = floor
                        .and_then(|f| self.floors.get(f).cloned())
                        .unwrap_or_else(|| "All floors".into());
                    egui::ComboBox::from_id_salt("box_materials_floor")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(floor, None, "All floors");
                            for (i, n) in self.floors.iter().enumerate() {
                                ui.selectable_value(floor, Some(i), n);
                            }
                        });
                });
                row(ui, "Category", |ui| {
                    egui::ComboBox::from_id_salt("box_materials_category")
                        .selected_text(category.clone().unwrap_or_else(|| "All categories".into()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(category, None, "All categories");
                            for c in plan_docs::MATERIAL_CATEGORIES {
                                ui.selectable_value(category, Some(c.to_string()), c);
                            }
                        });
                });
                ui.weak("The Materials List follows the plan; prices come from the Master List.");
            }
            BoxSource::Camera { camera_id } => {
                ui.label(format!("Camera view {camera_id}"));
            }
            BoxSource::PlacedSchedule { floor, id } => {
                let name = self.floors.get(*floor).cloned().unwrap_or_default();
                ui.label(format!("Schedule {id} placed on {name}"));
                ui.weak("The table follows the plan: columns, sort, grouping and totals come from the schedule.");
            }
            other => {
                ui.label(source_name(other));
            }
        }
    }

    fn line_style(&mut self, ui: &mut Ui) {
        let b = &mut self.draft;
        row(ui, "Line weight scaling", |ui| {
            ui.add(
                egui::DragValue::new(&mut b.line_weight_scale)
                    .speed(0.01)
                    .range(0.1..=5.0)
                    .max_decimals(2)
                    .suffix(" x"),
            );
        });
        ui.checkbox(&mut b.hatch_materials, "Material hatches (elevations)");
        ui.weak("Pen colors, weights and dashes come from each layer.");
    }
}

/// What a box shows, for the Source tab and the status line.
pub fn source_name(s: &BoxSource) -> String {
    match s {
        BoxSource::PlanView { .. } => "Floor plan".into(),
        BoxSource::Elevation { .. } => "Elevation".into(),
        BoxSource::Section { .. } => "Section".into(),
        BoxSource::Camera { .. } => "Camera view".into(),
        BoxSource::Schedule { kind } => format!("{kind:?} schedule"),
        BoxSource::PlacedSchedule { .. } => "Placed schedule".into(),
        BoxSource::CadDetail { name, .. } => format!("CAD detail: {name}"),
        BoxSource::Image { path } => format!("Image: {path}"),
        BoxSource::ImageData { width, height, .. } => format!("Image {width} x {height}"),
        BoxSource::Text { .. } => "Text".into(),
        BoxSource::Perspective { .. } => "Perspective view".into(),
        BoxSource::SheetIndex => "Sheet index".into(),
        BoxSource::Materials { category, .. } => match category {
            Some(c) => format!("Materials List: {c}"),
            None => "Materials List".into(),
        },
    }
}

/// Wrap / Shrink to fit / As typed buttons for a text box.
fn fit_buttons(ui: &mut Ui, fit: &mut plan_layout::TextFit) {
    use plan_layout::TextFit as F;
    for (f, name) in [
        (F::Wrap, "Wrap"),
        (F::Shrink, "Shrink to fit"),
        (F::Off, "As typed"),
    ] {
        ui.selectable_value(fit, f, name);
    }
}

fn fit_help(fit: plan_layout::TextFit) -> &'static str {
    match fit {
        plan_layout::TextFit::Wrap => "Wraps at the box width; text past the bottom is clipped.",
        plan_layout::TextFit::Shrink => "Wraps, then makes the type smaller until it all fits.",
        plan_layout::TextFit::Off => "One line per line break, no wrapping.",
    }
}

/// Left / Center / Right buttons for a text box.
fn align_buttons(ui: &mut Ui, align: &mut plan_layout::TextAlign) {
    use plan_layout::TextAlign as A;
    for (a, name) in [
        (A::Left, "Left"),
        (A::Center, "Center"),
        (A::Right, "Right"),
    ] {
        ui.selectable_value(align, a, name);
    }
}

// ------------------------------------------------------------ text editing --

/// The answers of the Text Box dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct TextBoxSpec {
    pub id: Id,
    pub text: String,
    pub height_pt: f64,
    pub align: plan_layout::TextAlign,
    pub bold: bool,
    pub fit: plan_layout::TextFit,
}

/// Editing a text box (double-click it in the layout view): the text, its
/// height in points, bold and alignment.
pub struct TextBoxDialog {
    spec: TextBoxSpec,
}

impl TextBoxDialog {
    /// `None` when `b` is not a text box.
    pub fn new(b: &LayoutBox) -> Option<Self> {
        let BoxSource::Text {
            text,
            height_pt,
            align,
            bold,
        } = &b.source
        else {
            return None;
        };
        Some(Self {
            spec: TextBoxSpec {
                id: b.id,
                text: text.clone(),
                height_pt: *height_pt,
                align: *align,
                bold: *bold,
                fit: b.text_fit,
            },
        })
    }

    pub fn spec(&self) -> &TextBoxSpec {
        &self.spec
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let spec = &mut self.spec;
        frame(ctx, "Text Box", 380.0, None, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut spec.text)
                    .desired_rows(5)
                    .desired_width(360.0),
            );
            row(ui, "Text height", |ui| {
                ui.add(
                    egui::DragValue::new(&mut spec.height_pt)
                        .speed(0.25)
                        .range(2.0..=200.0)
                        .suffix(" pt"),
                );
            });
            row(ui, "Alignment", |ui| align_buttons(ui, &mut spec.align));
            row(ui, "Text fit", |ui| fit_buttons(ui, &mut spec.fit));
            ui.checkbox(&mut spec.bold, "Bold");
            ui.weak(fit_help(spec.fit));
        })
    }
}

/// A line of text on the page (layout CAD): its position, words and height.
#[derive(Clone, Debug, PartialEq)]
pub struct CadTextSpec {
    /// The CAD object being edited; `None` adds new text.
    pub id: Option<Id>,
    pub pos: Point,
    pub text: String,
    /// Text height, paper inches.
    pub height_in: f64,
}

/// The layout Text tool's prompt (and double-click editing of page text).
pub struct CadTextDialog {
    spec: CadTextSpec,
}

impl CadTextDialog {
    pub fn new(spec: CadTextSpec) -> Self {
        Self { spec }
    }

    pub fn spec(&self) -> &CadTextSpec {
        &self.spec
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let empty = self.spec.text.trim().is_empty();
        let spec = &mut self.spec;
        frame(
            ctx,
            "Page Text",
            340.0,
            empty.then_some("Enter some text"),
            |ui| {
                ui.add(egui::TextEdit::singleline(&mut spec.text).desired_width(320.0));
                row(ui, "Text height", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut spec.height_in)
                            .speed(0.01)
                            .range(0.04..=3.0)
                            .max_decimals(3)
                            .suffix("\""),
                    );
                });
                ui.weak("Text macros such as %sheet.number% work here.");
            },
        )
    }
}

/// A leader on the page: where the arrow points, where the text sits, the words.
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderSpec {
    /// The leader being edited; `None` adds a new one.
    pub id: Option<Id>,
    pub tip: Point,
    pub elbow: Point,
    pub text: String,
    /// Text height, paper inches.
    pub height_in: f64,
    pub arrow: bool,
}

/// The Leader tool's prompt (and double-click editing of a leader).
pub struct LeaderDialog {
    spec: LeaderSpec,
}

impl LeaderDialog {
    pub fn new(spec: LeaderSpec) -> Self {
        Self { spec }
    }

    pub fn spec(&self) -> &LeaderSpec {
        &self.spec
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let empty = self.spec.text.trim().is_empty();
        let spec = &mut self.spec;
        frame(
            ctx,
            "Leader",
            340.0,
            empty.then_some("Enter the leader's text"),
            |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut spec.text)
                        .desired_rows(2)
                        .desired_width(320.0),
                );
                row(ui, "Text height", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut spec.height_in)
                            .speed(0.01)
                            .range(0.04..=2.0)
                            .max_decimals(3)
                            .suffix("\""),
                    );
                });
                ui.checkbox(&mut spec.arrow, "Arrowhead");
            },
        )
    }
}

/// A revision cloud on the page and the revision it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct CloudSpec {
    /// The cloud being edited; `None` adds a new one.
    pub id: Option<Id>,
    pub rect: (Point, Point),
    pub revision: String,
}

/// The Revision Cloud tool's prompt (and double-click editing of a cloud).
pub struct CloudDialog {
    spec: CloudSpec,
}

impl CloudDialog {
    pub fn new(spec: CloudSpec) -> Self {
        Self { spec }
    }

    pub fn spec(&self) -> &CloudSpec {
        &self.spec
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let spec = &mut self.spec;
        frame(ctx, "Revision Cloud", 320.0, None, |ui| {
            row(ui, "Revision", |ui| {
                ui.add(egui::TextEdit::singleline(&mut spec.revision).desired_width(80.0));
            });
            ui.weak("The mark (1, A, ...) is drawn in a triangle on the cloud; leave it empty for none.");
        })
    }
}

// -------------------------------------------------------- layer display --

/// Layer Display Options of the layout view: show or hide each layout layer
/// and set its line weight and colour.
pub struct LayoutLayersDialog {
    draft: plan_layout::LayoutLayers,
}

impl LayoutLayersDialog {
    pub fn new(layers: plan_layout::LayoutLayers) -> Self {
        Self { draft: layers }
    }

    pub fn layers(&self) -> &plan_layout::LayoutLayers {
        &self.draft
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let layers = &mut self.draft;
        frame(ctx, "Layout Layer Display Options", 420.0, None, |ui| {
            egui::Grid::new("layout_layers")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Show");
                    ui.strong("Layer");
                    ui.strong("Line weight");
                    ui.strong("Color");
                    ui.end_row();
                    for l in &mut layers.layers {
                        ui.checkbox(&mut l.display, "");
                        ui.label(&l.name);
                        ui.add(
                            egui::DragValue::new(&mut l.line_weight_pt)
                                .speed(0.05)
                                .range(plan_layout::MIN_WEIGHT_PT..=plan_layout::MAX_WEIGHT_PT)
                                .max_decimals(2)
                                .suffix(" pt"),
                        );
                        ui.color_edit_button_srgb(&mut l.color);
                        ui.end_row();
                    }
                });
            ui.weak("Box borders, page drawings, text, the title block and revision clouds each print with their layer's weight; hidden layers do not print.");
        })
    }
}

// ---------------------------------------------------------- page setup --

/// The Page Setup answers (Drawing Sheet Setup).
#[derive(Clone, Debug, PartialEq)]
pub struct PageSetup {
    pub sheet: SheetSize,
    pub margins_in: f64,
    pub page_background: bool,
    pub edge_line_weight: u32,
    pub sheet_index: bool,
    /// Portrait orientation (the sheet turned upright).
    pub portrait: bool,
}

/// Page Setup (sheet size, margins, background, edge weight).
pub struct PageSetupDialog {
    setup: PageSetup,
}

impl PageSetupDialog {
    pub fn new(setup: PageSetup) -> Self {
        Self { setup }
    }

    pub fn setup(&self) -> &PageSetup {
        &self.setup
    }

    fn error(&self) -> Option<&'static str> {
        let (w, h) = self.setup.sheet.inches();
        (self.setup.margins_in * 2.0 >= w.min(h)).then_some("The margins leave no room")
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let s = &mut self.setup;
        frame(ctx, "Page Setup", 380.0, error, |ui| {
            section(ui, "Sheet");
            row(ui, "Sheet size", |ui| {
                egui::ComboBox::from_id_salt("setup_sheet")
                    .selected_text(s.sheet.label())
                    .show_ui(ui, |ui| {
                        for z in SheetSize::ALL {
                            ui.selectable_value(&mut s.sheet, z, z.label());
                        }
                    });
            });
            row(ui, "Orientation", |ui| {
                ui.radio_value(&mut s.portrait, false, "Landscape");
                ui.radio_value(&mut s.portrait, true, "Portrait");
            });
            row(ui, "Margins", |ui| inches(ui, &mut s.margins_in));
            section(ui, "Page");
            ui.checkbox(&mut s.page_background, "Layout background (warm off-white)");
            row(ui, "Edge line weight", |ui| {
                ui.add(
                    egui::DragValue::new(&mut s.edge_line_weight)
                        .range(1..=100)
                        .suffix(" /100 mm"),
                );
            });
            ui.checkbox(&mut s.sheet_index, "Sheet index on the first page");
        })
    }
}

// ---------------------------------------------------------- page table --

/// One row of the Layout Page Table.
#[derive(Clone, Debug, PartialEq)]
pub struct PageRow {
    pub number: u32,
    pub title: String,
    pub template_page: bool,
}

/// Layout Page Table: titles and the template flag of every page.
pub struct PageTableDialog {
    rows: Vec<PageRow>,
}

impl PageTableDialog {
    pub fn new(rows: Vec<PageRow>) -> Self {
        Self { rows }
    }

    pub fn rows(&self) -> &[PageRow] {
        &self.rows
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let rows = &mut self.rows;
        frame(ctx, "Layout Page Table", 440.0, None, |ui| {
            egui::Grid::new("page_table").striped(true).show(ui, |ui| {
                ui.strong("Sheet");
                ui.strong("Title");
                ui.strong("Page Template");
                ui.end_row();
                for r in rows.iter_mut() {
                    ui.label(format!("A-{}", r.number));
                    ui.add(egui::TextEdit::singleline(&mut r.title).desired_width(240.0));
                    ui.checkbox(&mut r.template_page, "");
                    ui.end_row();
                }
            });
            ui.weak("A template page is not printed; its boxes repeat on every page.");
        })
    }
}

// --------------------------------------------------------------- print --

pub use super::print::PrintDialog;

#[cfg(test)]
mod tests {
    use super::*;
    use plan_layout::{BoxSource, LayoutBox};

    fn a_box() -> LayoutBox {
        let mut b = LayoutBox::new(
            3,
            (Point::new(1.0, 2.0), Point::new(7.0, 6.0)),
            BoxSource::PlanView {
                floor: 0,
                layer_set: "Default Set".into(),
            },
            Scale::QuarterInch,
        );
        b.label = Some("FIRST FLOOR PLAN".into());
        b
    }

    #[test]
    fn box_dialog_round_trips_the_box() {
        let b = a_box();
        let d = BoxSpecDialog::new(&b, 1, vec![(1, "One".into())], vec!["1st".into()], vec![]);
        assert_eq!(d.result().layout_box, b);
        assert_eq!(d.result().page, 1);
    }

    #[test]
    fn box_dialog_writes_back_fields() {
        let mut d = BoxSpecDialog::new(&a_box(), 1, vec![], vec![], vec![]);
        d.x = 2.0;
        d.w = 3.0;
        d.label = "  ".into();
        let r = d.result().layout_box;
        assert_eq!(r.rect_in, (Point::new(2.0, 2.0), Point::new(5.0, 6.0)));
        assert_eq!(r.label, None);
        d.h = 0.0;
        assert!(d.error().is_some());
    }

    #[test]
    fn send_dialog_defaults_to_the_current_page_and_auto_scale() {
        let pages = vec![(1, "A".to_string()), (2, "B".to_string())];
        let d = SendDialog::new(
            SendSource::Plan {
                floor: 0,
                layer_set: "Default Set".into(),
            },
            pages.clone(),
            Some(2),
            vec!["1st Floor".into()],
            vec!["Default Set".into()],
        );
        assert_eq!(d.spec().page, PageChoice::Existing(2));
        assert_eq!(d.spec().scale, None);
        assert_eq!(d.spec().placement, Placement::FirstFree);
        let none = SendDialog::new(
            SendSource::Camera {
                id: 1,
                name: "Front".into(),
            },
            vec![],
            None,
            vec![],
            vec![],
        );
        assert_eq!(none.spec().page, PageChoice::New);
    }

    #[test]
    fn page_setup_rejects_margins_wider_than_the_sheet() {
        let mut d = PageSetupDialog::new(PageSetup {
            sheet: SheetSize::Letter,
            margins_in: 0.5,
            page_background: true,
            edge_line_weight: 18,
            sheet_index: false,
            portrait: false,
        });
        assert!(d.error().is_none());
        d.setup.margins_in = 5.0;
        assert!(d.error().is_some());
    }

    #[test]
    fn perspective_and_text_box_fields_round_trip_through_the_box_dialog() {
        let mut b = LayoutBox::new(
            4,
            (Point::new(1.0, 1.0), Point::new(7.0, 5.5)),
            BoxSource::Perspective { camera_id: 2 },
            Scale::QuarterInch,
        );
        b.dpi = 200;
        b.samples = 32;
        let d = BoxSpecDialog::new(&b, 1, vec![], vec![], vec![]);
        assert_eq!(d.result().layout_box, b);
        let mut t = LayoutBox::new(
            5,
            (Point::new(1.0, 1.0), Point::new(3.0, 2.0)),
            BoxSource::text("hi", 10.0),
            Scale::QuarterInch,
        );
        t.text_fit = plan_layout::TextFit::Shrink;
        let td = TextBoxDialog::new(&t).unwrap();
        assert_eq!(td.spec().fit, plan_layout::TextFit::Shrink);
        let ctx = egui::Context::default();
        let (mut p, mut x) = (
            BoxSpecDialog::new(&b, 1, vec![], vec![], vec![]),
            BoxSpecDialog::new(&t, 1, vec![], vec![], vec![]),
        );
        let mut td = td;
        for tab in [BoxTab::Source, BoxTab::General] {
            p.tab = tab;
            x.tab = tab;
            let _ = ctx.run(egui::RawInput::default(), |c| {
                p.show(c);
                x.show(c);
                td.show(c);
            });
        }
    }

    #[test]
    fn leader_cloud_and_layer_dialogs_validate_and_draw() {
        let mut l = LeaderDialog::new(LeaderSpec {
            id: None,
            tip: Point::new(1.0, 1.0),
            elbow: Point::new(2.0, 2.0),
            text: String::new(),
            height_in: 0.125,
            arrow: true,
        });
        let ctx = egui::Context::default();
        let mut c = CloudDialog::new(CloudSpec {
            id: None,
            rect: (Point::new(1.0, 1.0), Point::new(3.0, 3.0)),
            revision: "1".into(),
        });
        let mut y = LayoutLayersDialog::new(plan_layout::LayoutLayers::default());
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                l.show(ctx);
                c.show(ctx);
                y.show(ctx);
            });
        }
        assert_eq!(y.layers().layers.len(), 5);
        assert_eq!(c.spec().revision, "1");
    }

    #[test]
    fn every_dialog_draws_frames() {
        let ctx = egui::Context::default();
        type Draw = Box<dyn FnMut(&egui::Context)>;
        let mut dialogs: Vec<Draw> = Vec::new();
        let mut send = SendDialog::new(
            SendSource::Plan {
                floor: 0,
                layer_set: "Default Set".into(),
            },
            vec![(1, "A".into())],
            Some(1),
            vec!["1st Floor".into()],
            vec!["Default Set".into()],
        );
        dialogs.push(Box::new(move |c| {
            send.show(c);
        }));
        let mut bx = BoxSpecDialog::new(&a_box(), 1, vec![(1, "A".into())], vec![], vec![]);
        dialogs.push(Box::new(move |c| {
            bx.show(c);
        }));
        let mut ps = PageSetupDialog::new(PageSetup {
            sheet: SheetSize::ArchC,
            margins_in: 0.5,
            page_background: true,
            edge_line_weight: 18,
            sheet_index: true,
            portrait: true,
        });
        dialogs.push(Box::new(move |c| {
            ps.show(c);
        }));
        let mut pt = PageTableDialog::new(vec![PageRow {
            number: 1,
            title: "T".into(),
            template_page: false,
        }]);
        dialogs.push(Box::new(move |c| {
            pt.show(c);
        }));
        let mut pr = PrintDialog::for_layout(2, (36.0, 24.0), "Layout");
        dialogs.push(Box::new(move |c| {
            pr.show(c);
        }));
        for _ in 0..2 {
            for d in &mut dialogs {
                let _ = ctx.run(egui::RawInput::default(), |c| d(c));
            }
        }
    }
}
