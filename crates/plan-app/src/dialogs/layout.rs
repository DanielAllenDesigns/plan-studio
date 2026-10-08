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
use plan_layout::{BoxSource, CustomSheetSize, LayoutBox, SheetChoice};

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

/// Which layout file receives the box (L-2): the plan can hold several, one
/// of them open at a time.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum LayoutTarget {
    /// The layout that is open.
    #[default]
    Current,
    /// One of the plan's other layout files, by name.
    Existing(String),
    /// A new layout file with this name.
    New(String),
}

/// A picture of the 3D view as it is, sent instead of the camera view: the
/// size of the box on the page and the quality it is traced at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotSpec {
    /// Width of the box, paper inches (the height follows the view).
    pub width_in: f64,
    pub dpi: u32,
    pub samples: u32,
}

impl Default for SnapshotSpec {
    fn default() -> Self {
        Self {
            width_in: 6.0,
            dpi: plan_layout::DEFAULT_PERSPECTIVE_DPI,
            samples: plan_layout::DEFAULT_PERSPECTIVE_SAMPLES,
        }
    }
}

/// Send to Layout (L-2).
pub struct SendDialog {
    spec: SendSpec,
    pages: PageList,
    floors: Vec<String>,
    layer_sets: Vec<String>,
    /// Names of the plan's layout files, the open one first.
    layouts: Vec<String>,
    target: LayoutTarget,
    /// The 3D view can be sent as a picture.
    snapshot_offered: bool,
    snapshot: Option<SnapshotSpec>,
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
            layouts: Vec::new(),
            target: LayoutTarget::Current,
            snapshot_offered: false,
            snapshot: None,
        }
    }

    /// Offers the plan's layout files (the open one first) to send to.
    pub fn with_layouts(mut self, names: Vec<String>) -> Self {
        self.layouts = names;
        self
    }

    /// Offers the 3D view as a picture; `chosen` starts with it picked.
    pub fn with_snapshot(mut self, chosen: bool) -> Self {
        self.snapshot_offered = true;
        self.snapshot = chosen.then(SnapshotSpec::default);
        self
    }

    pub fn spec(&self) -> &SendSpec {
        &self.spec
    }

    /// The layout file the box goes to.
    pub fn target(&self) -> &LayoutTarget {
        &self.target
    }

    /// Send the 3D view as a picture with these settings, if chosen.
    pub fn snapshot(&self) -> Option<SnapshotSpec> {
        self.snapshot
    }

    /// Why the answers cannot be used, if they cannot.
    fn error(&self) -> Option<&'static str> {
        match &self.target {
            LayoutTarget::New(n) if n.trim().is_empty() => Some("Name the new layout file"),
            LayoutTarget::New(n)
                if self
                    .layouts
                    .iter()
                    .any(|l| l.trim().eq_ignore_ascii_case(n.trim())) =>
            {
                Some("A layout file with that name exists")
            }
            _ => None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let Self {
            spec,
            pages,
            floors,
            layer_sets,
            layouts,
            target,
            snapshot_offered,
            snapshot,
        } = self;
        frame(ctx, "Send to Layout", 380.0, error, |ui| {
            if !layouts.is_empty() {
                section(ui, "Layout file");
                row(ui, "Send to", |ui| {
                    let shown = match target {
                        LayoutTarget::Current => layouts
                            .first()
                            .map_or("The open layout".to_string(), |n| format!("{n} (open)")),
                        LayoutTarget::Existing(n) => n.clone(),
                        LayoutTarget::New(_) => "New layout file".to_string(),
                    };
                    egui::ComboBox::from_id_salt("send_layout_file")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for (i, n) in layouts.iter().enumerate() {
                                let value = if i == 0 {
                                    LayoutTarget::Current
                                } else {
                                    LayoutTarget::Existing(n.clone())
                                };
                                let label = if i == 0 {
                                    format!("{n} (open)")
                                } else {
                                    n.clone()
                                };
                                ui.selectable_value(target, value, label);
                            }
                            if ui
                                .selectable_label(
                                    matches!(target, LayoutTarget::New(_)),
                                    "New layout file...",
                                )
                                .clicked()
                                && !matches!(target, LayoutTarget::New(_))
                            {
                                *target = LayoutTarget::New(String::new());
                            }
                        });
                });
                if let LayoutTarget::New(name) = target {
                    row(ui, "Name", |ui| {
                        ui.add(egui::TextEdit::singleline(name).desired_width(220.0));
                    });
                }
                if !matches!(target, LayoutTarget::Current) {
                    ui.weak("The page list below is the open layout's; a new page is added in the chosen file.");
                }
            }
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
            if *snapshot_offered {
                let mut picture = snapshot.is_some();
                if ui
                    .checkbox(&mut picture, "Send a picture of the 3D view as it is now")
                    .on_hover_text(
                        "A frozen picture of the view on screen (its camera and everything it shows), embedded in the PDF",
                    )
                    .changed()
                {
                    *snapshot = picture.then(SnapshotSpec::default);
                }
                if let Some(sn) = snapshot {
                    row(ui, "Width", |ui| {
                        ui.add(
                            egui::DragValue::new(&mut sn.width_in)
                                .range(1.0..=60.0)
                                .speed(0.1)
                                .suffix("\""),
                        );
                    });
                    row(ui, "Resolution", |ui| {
                        ui.add(
                            egui::DragValue::new(&mut sn.dpi)
                                .range(20..=600)
                                .suffix(" dpi"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut sn.samples)
                                .range(1..=512)
                                .suffix(" samples"),
                        );
                    });
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
    /// Bend points between the tip and the elbow (a multi-segment leader).
    pub bends: Vec<Point>,
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
    /// The layout's sheet: a standard size or one of its custom sizes.
    pub sheet: SheetChoice,
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
    /// The sizes the list offers (Customize Sheet Sizes decides which).
    choices: Vec<SheetChoice>,
}

impl PageSetupDialog {
    pub fn new(setup: PageSetup) -> Self {
        let mut choices: Vec<SheetChoice> = SheetSize::ALL
            .into_iter()
            .map(SheetChoice::Standard)
            .collect();
        if !choices.contains(&setup.sheet) {
            choices.push(setup.sheet.clone());
        }
        Self { setup, choices }
    }

    /// The sizes to offer (`Layout::size_choices`).
    pub fn with_choices(mut self, choices: Vec<SheetChoice>) -> Self {
        let current = self.setup.sheet.clone();
        self.choices = choices;
        if !self.choices.contains(&current) {
            self.choices.insert(0, current);
        }
        self
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
        let choices = &self.choices;
        let s = &mut self.setup;
        frame(ctx, "Page Setup", 380.0, error, |ui| {
            section(ui, "Sheet");
            row(ui, "Sheet size", |ui| {
                egui::ComboBox::from_id_salt("setup_sheet")
                    .selected_text(s.sheet.label())
                    .show_ui(ui, |ui| {
                        for z in choices {
                            ui.selectable_value(&mut s.sheet, z.clone(), z.label());
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

// ------------------------------------------------- page specification --

/// The Page Specification answers (L-7): one page's own settings.
#[derive(Clone, Debug, PartialEq)]
pub struct PageSpec {
    /// Sheet number: the page is `A-{number}`.
    pub number: u32,
    pub title: String,
    /// A Page Template page is not printed; its content repeats on the
    /// others.
    pub template_page: bool,
    /// The page's own sheet; `None` follows the layout.
    pub sheet: Option<SheetChoice>,
    /// The page's own sheet turned upright.
    pub portrait: bool,
    /// The page prints without the border and title block.
    pub no_title_block: bool,
}

impl PageSpec {
    /// The settings of `page` in `layout`.
    pub fn of(page: &plan_layout::LayoutPage, layout: &plan_layout::Layout) -> Self {
        let (sheet, portrait) = match page.size_override_in {
            None => (None, false),
            Some((w, h)) => {
                let portrait = h > w;
                let (long, short) = (w.max(h), w.min(h));
                let known = layout.size_choices().into_iter().find(|c| {
                    let (cl, cs) = c.inches();
                    (cl - long).abs() < 1e-6 && (cs - short).abs() < 1e-6
                });
                let choice = known.unwrap_or_else(|| {
                    SheetChoice::Custom(CustomSheetSize::new("This page", long, short))
                });
                (Some(choice), portrait)
            }
        };
        Self {
            number: page.number,
            title: page.title.clone(),
            template_page: page.template_page,
            sheet,
            portrait,
            no_title_block: page.no_title_block,
        }
    }
}

/// Page Specification: title, sheet number, the Page Template flag, the
/// page's own sheet size and whether it carries the title block.
pub struct PageSpecDialog {
    spec: PageSpec,
    /// Sheet numbers of the other pages.
    taken: Vec<u32>,
    choices: Vec<SheetChoice>,
    layout_sheet: String,
}

impl PageSpecDialog {
    /// `taken` are the other pages' numbers, `choices` the sizes on offer
    /// and `layout_sheet` the name of the layout's own sheet.
    pub fn new(
        spec: PageSpec,
        taken: Vec<u32>,
        mut choices: Vec<SheetChoice>,
        layout_sheet: &str,
    ) -> Self {
        if let Some(c) = &spec.sheet {
            if !choices.contains(c) {
                choices.insert(0, c.clone());
            }
        }
        Self {
            spec,
            taken,
            choices,
            layout_sheet: layout_sheet.to_string(),
        }
    }

    pub fn spec(&self) -> &PageSpec {
        &self.spec
    }

    fn error(&self) -> Option<&'static str> {
        if self.taken.contains(&self.spec.number) {
            Some("Another page already has that sheet number")
        } else if self.spec.title.trim().is_empty() {
            Some("Give the page a title")
        } else {
            None
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let Self {
            spec,
            choices,
            layout_sheet,
            ..
        } = self;
        frame(ctx, "Page Specification", 400.0, error, |ui| {
            section(ui, "Page");
            row(ui, "Title", |ui| {
                ui.add(egui::TextEdit::singleline(&mut spec.title).desired_width(240.0));
            });
            row(ui, "Sheet number", |ui| {
                ui.label("A-");
                ui.add(egui::DragValue::new(&mut spec.number).range(0..=999));
            });
            ui.checkbox(
                &mut spec.template_page,
                "Page Template (not printed; its content repeats on every page)",
            );
            ui.checkbox(
                &mut spec.no_title_block,
                "No border or title block on this page",
            );
            section(ui, "Sheet");
            row(ui, "Sheet size", |ui| {
                let shown = match &spec.sheet {
                    None => format!("Same as the layout ({layout_sheet})"),
                    Some(c) => c.label(),
                };
                egui::ComboBox::from_id_salt("page_spec_sheet")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut spec.sheet,
                            None,
                            format!("Same as the layout ({layout_sheet})"),
                        );
                        for c in choices.iter() {
                            ui.selectable_value(&mut spec.sheet, Some(c.clone()), c.label());
                        }
                    });
            });
            ui.add_enabled_ui(spec.sheet.is_some(), |ui| {
                row(ui, "Orientation", |ui| {
                    ui.radio_value(&mut spec.portrait, false, "Landscape");
                    ui.radio_value(&mut spec.portrait, true, "Portrait");
                });
            });
            ui.weak(
                "A page with its own sheet prints at that size; its boxes pack into that sheet.",
            );
        })
    }
}

// ------------------------------------------------ customize sheet sizes --

/// The Customize Sheet Sizes answers: sizes added and standard sizes left
/// out of the lists.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SheetSizes {
    pub custom: Vec<CustomSheetSize>,
    pub hidden: Vec<SheetSize>,
}

/// Customize Sheet Sizes (L-8): add named sizes, and choose which standard
/// sizes the Page Setup and Page Specification lists show.
pub struct SheetSizesDialog {
    draft: SheetSizes,
    /// The layout's sheet: its standard size stays in the list.
    in_use: SheetSize,
}

impl SheetSizesDialog {
    pub fn new(sizes: SheetSizes, in_use: SheetSize) -> Self {
        Self {
            draft: sizes,
            in_use,
        }
    }

    pub fn sizes(&self) -> &SheetSizes {
        &self.draft
    }

    fn error(&self) -> Option<String> {
        for (i, c) in self.draft.custom.iter().enumerate() {
            if let Some(p) = c.problem() {
                return Some(format!("Size {}: {p}", i + 1));
            }
            let name = c.name.trim();
            if self
                .draft
                .custom
                .iter()
                .skip(i + 1)
                .any(|o| o.name.trim().eq_ignore_ascii_case(name))
                || SheetSize::ALL
                    .iter()
                    .any(|s| s.label().eq_ignore_ascii_case(name))
            {
                return Some(format!("There are two sizes called {name}"));
            }
        }
        None
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let in_use = self.in_use;
        let d = &mut self.draft;
        frame(
            ctx,
            "Customize Sheet Sizes",
            460.0,
            error.as_deref(),
            |ui| {
                section(ui, "Standard sizes");
                ui.horizontal(|ui| {
                    if ui
                        .button("Daniel's sizes (ARCH)")
                        .on_hover_text("Show the ARCH sizes only, 18 x 24 first")
                        .clicked()
                    {
                        d.hidden = SheetSize::ALL
                            .into_iter()
                            .filter(|s| !plan_layout::DANIEL_SIZES.contains(s))
                            .collect();
                    }
                    if ui.button("Show all").clicked() {
                        d.hidden.clear();
                    }
                });
                egui::Grid::new("sheet_sizes_standard")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for z in SheetSize::ALL {
                            let mut shown = !d.hidden.contains(&z);
                            let locked = z == in_use;
                            let was = shown;
                            ui.add_enabled(!locked, egui::Checkbox::new(&mut shown, z.label()));
                            let (w, h) = z.inches();
                            ui.weak(format!("{:.1} x {:.1} in", h, w));
                            ui.end_row();
                            if shown != was {
                                if shown {
                                    d.hidden.retain(|s| *s != z);
                                } else {
                                    d.hidden.push(z);
                                }
                            }
                        }
                    });
                section(ui, "Custom sizes");
                let mut remove = None;
                egui::Grid::new("sheet_sizes_custom")
                    .num_columns(4)
                    .show(ui, |ui| {
                        for (i, c) in d.custom.iter_mut().enumerate() {
                            ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(140.0));
                            ui.add(
                                egui::DragValue::new(&mut c.width_in)
                                    .range(0.0..=400.0)
                                    .speed(0.1)
                                    .suffix("\""),
                            );
                            ui.add(
                                egui::DragValue::new(&mut c.height_in)
                                    .range(0.0..=400.0)
                                    .speed(0.1)
                                    .suffix("\""),
                            );
                            if ui.small_button("Delete").clicked() {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                if let Some(i) = remove {
                    d.custom.remove(i);
                }
                if ui.button("Add size").clicked() {
                    let n = d.custom.len() + 1;
                    d.custom
                        .push(CustomSheetSize::new(format!("Custom {n}"), 24.0, 36.0));
                }
                ui.weak("The layout's own sheet always stays in the list.");
            },
        )
    }
}

// ------------------------------------------------------------- copy box --

/// Copy Box to Page: which page receives copies of the selected boxes.
pub struct CopyBoxDialog {
    pages: PageList,
    /// Sheet number of the receiving page.
    to: u32,
    count: usize,
}

impl CopyBoxDialog {
    /// `current` is the page the boxes are on (the first choice is the page
    /// after it, else the same page).
    pub fn new(pages: PageList, current: u32, count: usize) -> Self {
        let to = pages
            .iter()
            .map(|p| p.0)
            .find(|n| *n > current)
            .unwrap_or(current);
        Self { pages, to, count }
    }

    /// Sheet number of the receiving page.
    pub fn to(&self) -> u32 {
        self.to
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let Self { pages, to, count } = self;
        frame(ctx, "Copy Layout Box to Page", 360.0, None, |ui| {
            ui.label(if *count == 1 {
                "Copy the selected box to:".to_string()
            } else {
                format!("Copy the {count} selected boxes to:")
            });
            row(ui, "Page", |ui| {
                let shown = pages
                    .iter()
                    .find(|p| p.0 == *to)
                    .map_or_else(|| format!("A-{to}"), page_label);
                egui::ComboBox::from_id_salt("copy_box_page")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for p in pages.iter() {
                            ui.selectable_value(to, p.0, page_label(p));
                        }
                    });
            });
            ui.weak(
                "The copies keep their place on the new page; on the same page they land offset.",
            );
        })
    }
}

// ------------------------------------------------------- new layout file --

/// A one-line name prompt: New Layout File.
pub struct NameDialog {
    title: String,
    prompt: String,
    name: String,
    taken: Vec<String>,
}

impl NameDialog {
    pub fn new(title: &str, prompt: &str, name: &str, taken: Vec<String>) -> Self {
        Self {
            title: title.to_string(),
            prompt: prompt.to_string(),
            name: name.to_string(),
            taken,
        }
    }

    pub fn name(&self) -> &str {
        self.name.trim()
    }

    fn error(&self) -> Option<&'static str> {
        let n = self.name.trim();
        if n.is_empty() {
            Some("Enter a name")
        } else if self.taken.iter().any(|t| t.trim().eq_ignore_ascii_case(n)) {
            Some("A layout file with that name exists")
        } else {
            None
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let Self {
            title,
            prompt,
            name,
            ..
        } = self;
        frame(ctx, title, 360.0, error, |ui| {
            row(ui, prompt, |ui| {
                ui.add(egui::TextEdit::singleline(name).desired_width(220.0));
            });
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

// ----------------------------------------------------- layout templates --

/// `~/.plan-studio/templates`, where layout templates are kept.
pub fn layout_templates_dir() -> Option<std::path::PathBuf> {
    #[cfg(test)]
    if let Some(d) = TEMPLATE_DIR_OVERRIDE.with(|d| d.borrow().clone()) {
        return Some(d);
    }
    crate::paths::user_file("templates")
}

#[cfg(test)]
thread_local! {
    /// Tests keep their templates here instead of in the home folder.
    static TEMPLATE_DIR_OVERRIDE: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// Points the template folder at `dir` for the rest of this test (tests).
#[cfg(test)]
pub fn set_layout_templates_dir_for_tests(dir: Option<std::path::PathBuf>) {
    TEMPLATE_DIR_OVERRIDE.with(|d| *d.borrow_mut() = dir);
}

/// Writes `template` to `dir` as `<name>.layout.json`, creating the folder,
/// and returns the file. A template of the same name is replaced; when the
/// template is the default for its sheet size, the other templates of that
/// size stop being it.
pub fn save_layout_template(
    dir: &std::path::Path,
    template: &plan_layout::LayoutTemplate,
) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    if template.default_for_sheet {
        for mut other in list_layout_templates(dir) {
            if other.sheet() == template.sheet()
                && other.default_for_sheet
                && other.name != template.name
            {
                other.default_for_sheet = false;
                write_template_file(dir, &other)?;
            }
        }
    }
    write_template_file(dir, template)
}

fn write_template_file(
    dir: &std::path::Path,
    template: &plan_layout::LayoutTemplate,
) -> Result<std::path::PathBuf, String> {
    let file = dir.join(format!(
        "{}{}",
        plan_layout::template_file_stem(&template.name),
        plan_layout::TEMPLATE_EXTENSION
    ));
    std::fs::write(&file, template.to_json()?)
        .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    Ok(file)
}

/// The layout templates in `dir` (unreadable files are skipped), by name.
pub fn list_layout_templates(dir: &std::path::Path) -> Vec<plan_layout::LayoutTemplate> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<plan_layout::LayoutTemplate> = read
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .ends_with(plan_layout::TEMPLATE_EXTENSION)
        })
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| plan_layout::LayoutTemplate::from_json(&t).ok())
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// The template new layouts of `sheet` start from: the one marked as the
/// default for that size.
pub fn default_layout_template(
    dir: &std::path::Path,
    sheet: SheetSize,
) -> Option<plan_layout::LayoutTemplate> {
    list_layout_templates(dir)
        .into_iter()
        .find(|t| t.default_for_sheet && t.sheet() == sheet)
}

/// What the template window does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateMode {
    /// Save As Template: name the template and store the layout under it.
    Save,
    /// Apply Template: replace the layout with a saved template.
    Apply,
}

/// The Save As Template and Apply Template window.
pub struct LayoutTemplateDialog {
    mode: TemplateMode,
    pub name: String,
    pub default_for_sheet: bool,
    existing: Vec<plan_layout::LayoutTemplate>,
    selected: Option<usize>,
}

impl LayoutTemplateDialog {
    /// Save As Template for a layout called `layout_name`.
    pub fn save(layout_name: &str, existing: Vec<plan_layout::LayoutTemplate>) -> Self {
        Self {
            mode: TemplateMode::Save,
            name: layout_name.to_string(),
            default_for_sheet: false,
            existing,
            selected: None,
        }
    }

    /// Apply one of `existing`.
    pub fn apply(existing: Vec<plan_layout::LayoutTemplate>) -> Self {
        let selected = (!existing.is_empty()).then_some(0);
        Self {
            mode: TemplateMode::Apply,
            name: String::new(),
            default_for_sheet: false,
            existing,
            selected,
        }
    }

    pub fn mode(&self) -> TemplateMode {
        self.mode
    }

    /// The template picked to apply.
    pub fn picked(&self) -> Option<&plan_layout::LayoutTemplate> {
        self.selected.and_then(|i| self.existing.get(i))
    }

    /// Does saving under the typed name replace a template?
    pub fn replaces(&self) -> bool {
        let n = self.name.trim().to_lowercase();
        self.existing.iter().any(|t| t.name.to_lowercase() == n)
    }

    fn error(&self) -> Option<&'static str> {
        match self.mode {
            TemplateMode::Save if self.name.trim().is_empty() => Some("Name the template"),
            TemplateMode::Apply if self.picked().is_none() => {
                Some("There are no saved layout templates")
            }
            _ => None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let title = match self.mode {
            TemplateMode::Save => "Save Layout As Template",
            TemplateMode::Apply => "Apply Layout Template",
        };
        let (mode, replaces) = (self.mode, self.replaces());
        let (name, default_for_sheet, existing, selected) = (
            &mut self.name,
            &mut self.default_for_sheet,
            &self.existing,
            &mut self.selected,
        );
        frame(ctx, title, 360.0, error, |ui| {
            if mode == TemplateMode::Save {
                row(ui, "Name", |ui| {
                    ui.add(egui::TextEdit::singleline(name).desired_width(240.0));
                });
                ui.checkbox(
                    default_for_sheet,
                    "Start new layouts of this sheet size from it",
                );
                if replaces {
                    ui.weak("A template with this name is replaced.");
                }
                ui.weak("Boxes of cameras and placed schedules are left out.");
            }
            if !existing.is_empty() {
                ui.separator();
                ui.label(if mode == TemplateMode::Save {
                    "Saved templates"
                } else {
                    "Choose a template"
                });
                for (i, t) in existing.iter().enumerate() {
                    let text = format!(
                        "{}   ({}, {} pages{})",
                        t.name,
                        t.sheet().label(),
                        t.layout.pages.len(),
                        if t.default_for_sheet { ", default" } else { "" }
                    );
                    if ui.selectable_label(*selected == Some(i), text).clicked() {
                        *selected = Some(i);
                        if mode == TemplateMode::Save {
                            *name = t.name.clone();
                        }
                    }
                }
            }
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
            sheet: SheetChoice::Standard(SheetSize::Letter),
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
            bends: Vec::new(),
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
    fn page_spec_reads_a_pages_own_sheet_and_validates() {
        let mut l = plan_layout::Layout::new("t", SheetSize::ArchC);
        l.add_page(1, "Plan").size_override_in = Some((36.0, 24.0));
        l.add_page(2, "Detail").size_override_in = Some((24.0, 36.0));
        l.add_page(3, "Odd").size_override_in = Some((30.0, 20.0));
        l.add_page(4, "Plain");
        let spec = |i: usize| PageSpec::of(&l.pages[i], &l);
        assert_eq!(spec(0).sheet, Some(SheetChoice::Standard(SheetSize::ArchD)));
        assert!(!spec(0).portrait);
        assert!(spec(1).portrait, "taller than wide is portrait");
        assert_eq!(spec(1).sheet, Some(SheetChoice::Standard(SheetSize::ArchD)));
        match spec(2).sheet {
            Some(SheetChoice::Custom(c)) => assert_eq!(c.inches(), (30.0, 20.0)),
            other => panic!("{other:?}"),
        }
        assert_eq!(spec(3).sheet, None);
        let mut d = PageSpecDialog::new(spec(3), vec![1, 2, 3], l.size_choices(), "ARCH C");
        assert!(d.error().is_none());
        d.spec.number = 2;
        assert_eq!(
            d.error(),
            Some("Another page already has that sheet number")
        );
        d.spec.number = 9;
        d.spec.title = "  ".into();
        assert!(d.error().is_some());
        // A size the list does not offer is added so the combo can show it.
        let odd = PageSpecDialog::new(spec(2), vec![], l.size_choices(), "ARCH C");
        assert!(odd
            .choices
            .iter()
            .any(|c| matches!(c, SheetChoice::Custom(_))));
    }

    #[test]
    fn sheet_size_and_name_dialogs_validate() {
        let mut d = SheetSizesDialog::new(SheetSizes::default(), SheetSize::ArchC);
        assert!(d.error().is_none());
        d.draft
            .custom
            .push(CustomSheetSize::new("Poster", 30.0, 40.0));
        assert!(d.error().is_none());
        d.draft
            .custom
            .push(CustomSheetSize::new("poster", 24.0, 36.0));
        assert!(d.error().unwrap().contains("two sizes"));
        d.draft.custom.pop();
        d.draft
            .custom
            .push(CustomSheetSize::new("ARCH C (18 x 24)", 10.0, 10.0));
        assert!(d.error().is_some(), "a standard size's name is taken");
        d.draft.custom.pop();
        d.draft.custom.push(CustomSheetSize::new("Tiny", 1.0, 10.0));
        assert!(d.error().unwrap().contains("Size 2"));
        let mut n = NameDialog::new("New Layout File", "Name", "Set A", vec!["Set A".into()]);
        assert!(n.error().is_some());
        n.name = " Set B ".into();
        assert!(n.error().is_none());
        assert_eq!(n.name(), "Set B");
        n.name.clear();
        assert_eq!(n.error(), Some("Enter a name"));
        // Copy Box starts on the page after the current one.
        let pages = vec![
            (1, "A".to_string()),
            (2, "B".to_string()),
            (5, "C".to_string()),
        ];
        assert_eq!(CopyBoxDialog::new(pages.clone(), 1, 2).to(), 2);
        assert_eq!(CopyBoxDialog::new(pages.clone(), 2, 1).to(), 5);
        assert_eq!(
            CopyBoxDialog::new(pages, 5, 1).to(),
            5,
            "the last page: itself"
        );
    }

    #[test]
    fn the_send_dialog_offers_layout_files_and_the_3d_picture() {
        let plan = SendSource::Plan {
            floor: 0,
            layer_set: "Default Set".into(),
        };
        let mut d = SendDialog::new(plan, vec![(1, "A".into())], Some(1), vec![], vec![])
            .with_layouts(vec!["Open".into(), "Parked".into()])
            .with_snapshot(true);
        assert_eq!(d.target(), &LayoutTarget::Current);
        assert_eq!(d.snapshot(), Some(SnapshotSpec::default()));
        d.target = LayoutTarget::New("  ".into());
        assert!(d.error().is_some());
        d.target = LayoutTarget::New("parked".into());
        assert_eq!(d.error(), Some("A layout file with that name exists"));
        d.target = LayoutTarget::New("Permit Set".into());
        assert!(d.error().is_none());
        d.target = LayoutTarget::Existing("Parked".into());
        assert!(d.error().is_none());
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |c| {
                d.show(c);
            });
        }
        let off = SendDialog::new(
            SendSource::Plan {
                floor: 0,
                layer_set: String::new(),
            },
            vec![],
            None,
            vec![],
            vec![],
        )
        .with_snapshot(false);
        assert_eq!(off.snapshot(), None);
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
            sheet: SheetChoice::Standard(SheetSize::ArchC),
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
        let mut pr = PrintDialog::for_layout(2, (36.0, 24.0), "Layout")
            .with_custom_papers(vec![("Poster (30 x 40)".into(), (40.0, 30.0))]);
        dialogs.push(Box::new(move |c| {
            pr.show(c);
        }));
        let mut spec = PageSpecDialog::new(
            PageSpec {
                number: 1,
                title: "Plan".into(),
                template_page: false,
                sheet: Some(SheetChoice::Standard(SheetSize::ArchD)),
                portrait: false,
                no_title_block: false,
            },
            vec![2],
            vec![SheetChoice::Standard(SheetSize::ArchC)],
            "ARCH C",
        );
        dialogs.push(Box::new(move |c| {
            spec.show(c);
        }));
        let mut sizes = SheetSizesDialog::new(
            SheetSizes {
                custom: vec![CustomSheetSize::new("Poster", 30.0, 40.0)],
                hidden: vec![SheetSize::IsoA0],
            },
            SheetSize::ArchC,
        );
        dialogs.push(Box::new(move |c| {
            sizes.show(c);
        }));
        let mut copy = CopyBoxDialog::new(vec![(1, "A".into()), (2, "B".into())], 1, 2);
        dialogs.push(Box::new(move |c| {
            copy.show(c);
        }));
        let mut name = NameDialog::new("New Layout File", "Name", "Set B", vec![]);
        dialogs.push(Box::new(move |c| {
            name.show(c);
        }));
        for _ in 0..2 {
            for d in &mut dialogs {
                let _ = ctx.run(egui::RawInput::default(), |c| d(c));
            }
        }
    }

    #[test]
    fn layout_templates_save_list_and_default_per_sheet() {
        let dir = std::env::temp_dir().join(format!(
            "plan-studio-layout-templates-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(list_layout_templates(&dir).is_empty());
        let mut layout = plan_layout::Layout::new("Smith", SheetSize::ArchC);
        layout.add_page(1, "Plan");
        let a = plan_layout::LayoutTemplate::new("Presentation", &layout, true);
        let file = save_layout_template(&dir, &a).unwrap();
        assert!(file.ends_with("Presentation.layout.json"));
        let mut b = plan_layout::LayoutTemplate::new("Working", &layout, true);
        b.layout.add_page(2, "Elevations");
        save_layout_template(&dir, &b).unwrap();
        // Another sheet size keeps its own default.
        let other = plan_layout::Layout::new("Small", SheetSize::ArchB);
        save_layout_template(
            &dir,
            &plan_layout::LayoutTemplate::new("Small", &other, true),
        )
        .unwrap();
        let all = list_layout_templates(&dir);
        assert_eq!(
            all.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["Presentation", "Small", "Working"]
        );
        // Only one default per sheet size: the newest.
        let defaults: Vec<&str> = all
            .iter()
            .filter(|t| t.default_for_sheet)
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(defaults, ["Small", "Working"]);
        let d = default_layout_template(&dir, SheetSize::ArchC).unwrap();
        assert_eq!(d.name, "Working");
        assert_eq!(d.layout.pages.len(), 2);
        assert_eq!(
            default_layout_template(&dir, SheetSize::ArchB)
                .unwrap()
                .name,
            "Small"
        );
        assert!(default_layout_template(&dir, SheetSize::ArchD).is_none());
        // Re-saving a name replaces the file; junk files are skipped.
        std::fs::write(dir.join("junk.layout.json"), "nope").unwrap();
        std::fs::write(dir.join("readme.txt"), "x").unwrap();
        save_layout_template(
            &dir,
            &plan_layout::LayoutTemplate::new("Working", &layout, false),
        )
        .unwrap();
        let all = list_layout_templates(&dir);
        assert_eq!(all.len(), 3);
        assert_eq!(all[2].layout.pages.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_template_dialog_validates_and_picks() {
        let layout = plan_layout::Layout::new("Smith", SheetSize::ArchC);
        let t = plan_layout::LayoutTemplate::new("Presentation", &layout, false);
        let mut save = LayoutTemplateDialog::save("Smith Layout", vec![t.clone()]);
        assert!(save.error().is_none());
        assert!(!save.replaces());
        save.name = "presentation".into();
        assert!(save.replaces(), "names compare without case");
        save.name = "  ".into();
        assert!(save.error().is_some());
        let apply = LayoutTemplateDialog::apply(vec![t]);
        assert_eq!(apply.picked().unwrap().name, "Presentation");
        assert!(LayoutTemplateDialog::apply(vec![]).error().is_some());
        // Both windows draw.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let mut d = LayoutTemplateDialog::save("x", vec![]);
            d.show(ctx);
            let mut d = LayoutTemplateDialog::apply(vec![]);
            d.show(ctx);
        });
    }
}
