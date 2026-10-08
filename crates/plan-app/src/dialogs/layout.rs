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
fn frame(
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
            ui.add_enabled(false, egui::DragValue::new(&mut 0.0_f64).suffix("\u{b0}"))
                .on_disabled_hover_text("The layout model has no box rotation yet");
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
            BoxSource::Text { text, height_pt } => {
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
            }
            BoxSource::Camera { camera_id } => {
                ui.label(format!("Camera view {camera_id}"));
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
        BoxSource::CadDetail { name, .. } => format!("CAD detail: {name}"),
        BoxSource::Image { path } => format!("Image: {path}"),
        BoxSource::ImageData { width, height, .. } => format!("Image {width} x {height}"),
        BoxSource::Text { .. } => "Text".into(),
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
                ui.add_enabled(false, egui::RadioButton::new(true, "Landscape"));
                ui.add_enabled(false, egui::RadioButton::new(false, "Portrait"))
                    .on_disabled_hover_text("Sheets are landscape in the layout model");
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

/// Print: all pages or a range of printed pages (1-based, inclusive).
pub struct PrintDialog {
    pages: usize,
    all: bool,
    from: usize,
    to: usize,
}

impl PrintDialog {
    pub fn new(printed_pages: usize) -> Self {
        Self {
            pages: printed_pages,
            all: true,
            from: 1,
            to: printed_pages.max(1),
        }
    }

    /// `None` prints every page.
    pub fn range(&self) -> Option<(usize, usize)> {
        (!self.all).then_some((self.from, self.to))
    }

    fn error(&self) -> Option<&'static str> {
        if self.pages == 0 {
            Some("The layout has no pages to print")
        } else if !self.all && (self.from < 1 || self.to < self.from || self.to > self.pages) {
            Some("Enter a page range inside the layout")
        } else {
            None
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let pages = self.pages;
        let (all, from, to) = (&mut self.all, &mut self.from, &mut self.to);
        frame(ctx, "Print Layout", 340.0, error, |ui| {
            ui.label(format!("The layout has {pages} printed page(s)."));
            ui.radio_value(all, true, "All pages");
            ui.horizontal(|ui| {
                ui.radio_value(all, false, "Pages");
                ui.add_enabled(!*all, egui::DragValue::new(from).range(1..=pages.max(1)));
                ui.label("to");
                ui.add_enabled(!*all, egui::DragValue::new(to).range(1..=pages.max(1)));
            });
            ui.weak("The pages are saved as a PDF.");
        })
    }
}

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
    fn print_range_is_validated() {
        let mut d = PrintDialog::new(3);
        assert_eq!(d.range(), None);
        assert!(d.error().is_none());
        d.all = false;
        d.from = 2;
        d.to = 5;
        assert!(d.error().is_some());
        d.to = 3;
        assert_eq!(d.range(), Some((2, 3)));
        assert!(PrintDialog::new(0).error().is_some());
    }

    #[test]
    fn page_setup_rejects_margins_wider_than_the_sheet() {
        let mut d = PageSetupDialog::new(PageSetup {
            sheet: SheetSize::Letter,
            margins_in: 0.5,
            page_background: true,
            edge_line_weight: 18,
            sheet_index: false,
        });
        assert!(d.error().is_none());
        d.setup.margins_in = 5.0;
        assert!(d.error().is_some());
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
        let mut pr = PrintDialog::new(2);
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
