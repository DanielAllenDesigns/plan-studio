//! File > Print > Drawing Sheet Setup, Scale to Fit, Center Sheet, Clear
//! Printer Info and Customize Sheet Sizes; the Drawing Sheet on the plan
//! (manual pp. 1425-1433).
//!
//! Chief keeps a Drawing Sheet Setup for each kind of view (plan, cross
//! section / elevation, CAD Detail, layout, Materials List); a new view starts
//! from the plan's. They are stored in the plan (`Project::print_setup`).
//! The plan's setup is what the on-screen Drawing Sheet and the printed-size
//! text styles follow (`EditorContext::sheet`), and its Drawing Scale is the
//! default scale of Print and Send to Layout ([`default_scale`]).
//!
//! The Print dialog reads the active view's setup, the Drawing Sheet's
//! footprint and what is on screen through [`context`], which the canvas
//! refreshes every frame ([`sync`]).
//!
//! On screen the Drawing Sheet is an object: drag its border to move it
//! (Center Sheet and the drag keep the sheet's place in `Floor::sheet_center`
//! and never move a coordinate), drag a corner to resize it. A blue border
//! marks the printable area inside the Drawing Margins.

use super::layout::{SheetSizes, SheetSizesDialog};
use super::print::{list_printers, Printers};
use super::{row, section, Outcome};
use crate::editor::camera::Camera;
use crate::editor::sheet as esheet;
use crate::editor::EditorContext;
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Color32, Pos2, Stroke};
use plan_core::drawing_sheet::{
    DrawingScale, LenUnit, LineWeightSetup, SheetDims, ViewSheet, ViewType,
};
use plan_core::Point;
use plan_docs::{Scale, SheetSize};
use plan_layout::SheetChoice;
use std::cell::RefCell;

/// File > Print > Drawing Sheet Setup.
pub const OPEN: &str = "print.drawing_sheet_setup";
/// File > Print > Scale to Fit.
pub const SCALE_TO_FIT: &str = "print.scale_to_fit";
/// File > Print > Center Sheet.
pub const CENTER_SHEET: &str = "print.center_sheet";
/// File > Print > Clear Printer Info.
pub const CLEAR_PRINTER: &str = "print.clear_printer_info";
/// File > Print > Customize Sheet Sizes.
pub const CUSTOMIZE: &str = "print.customize_sheet_sizes";

/// Is `id` one of this module's commands?
pub fn is_command(id: &str) -> bool {
    matches!(
        id,
        OPEN | SCALE_TO_FIT | CENTER_SHEET | CLEAR_PRINTER | CUSTOMIZE
    )
}

// ------------------------------------------------------------ the view --

/// What the Print dialog and the menus need to know about the active view.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintViewContext {
    /// The kind of view that is active.
    pub view: ViewType,
    /// Its Drawing Sheet Setup (its own, else the plan's).
    pub sheet: ViewSheet,
    pub floor: usize,
    /// Is View > Drawing Sheet on (Print Source starts at Drawing Sheet)?
    pub sheet_shown: bool,
    /// What the Drawing Sheet covers on the plan: `[x0, y0, x1, y1]` plan inches.
    pub sheet_window: [f64; 4],
    /// What is on screen, plan inches.
    pub view_window: Option<[f64; 4]>,
    /// The walls and dimensions of the floor, plan inches.
    pub plan_extent: Option<(Point, Point)>,
    /// Is View > Watermark on in the active view?
    pub watermark_on: bool,
    /// Does the plan's watermark have anything to show?
    pub watermark_ready: bool,
    /// The layout's Drawing Sheet Setup (printer, line weights): the Print
    /// dialog of a layout reads it, whatever view is on screen.
    pub layout_sheet: ViewSheet,
    /// Has the canvas filled this in (false: the defaults of a fresh run)?
    pub synced: bool,
}

impl Default for PrintViewContext {
    fn default() -> Self {
        let sheet = ViewSheet::default();
        let scale = scale_of(&sheet);
        Self {
            view: ViewType::Plan,
            sheet_window: plan_layout::drawing_sheet_window(Point::ZERO, sheet.inches(), scale),
            sheet,
            floor: 0,
            sheet_shown: false,
            view_window: None,
            plan_extent: None,
            watermark_on: false,
            watermark_ready: false,
            layout_sheet: ViewSheet {
                scale: plan_core::drawing_sheet::DrawingScale::layout(),
                ..ViewSheet::default()
            },
            synced: false,
        }
    }
}

/// The plan Scale a view setup's Drawing Scale draws.
pub fn scale_of(vs: &ViewSheet) -> Scale {
    Scale::from_inches_per_foot(vs.scale.inches_per_foot())
}

thread_local! {
    static CONTEXT: RefCell<PrintViewContext> = RefCell::new(PrintViewContext::default());
    static APPLIED: RefCell<Option<(ViewType, ViewSheet)>> = const { RefCell::new(None) };
    static DIALOGS: RefCell<Dialogs> = RefCell::new(Dialogs::default());
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
}

/// The active view's print context, as the canvas last saw it.
pub fn context() -> PrintViewContext {
    CONTEXT.with(|c| c.borrow().clone())
}

/// Replaces the context (tests, scenarios).
pub fn set_context(c: PrintViewContext) {
    CONTEXT.with(|s| *s.borrow_mut() = c);
}

/// The kind of view the editor shows: the layout when its window is up, a CAD
/// Detail on a detail floor, else the plan.
pub fn current_view(cx: &EditorContext) -> ViewType {
    if crate::shell::layout_window::is_active() {
        ViewType::Layout
    } else if cx.floor().is_cad_detail() {
        ViewType::CadDetail
    } else {
        ViewType::Plan
    }
}

/// The Drawing Scale of the active view as a `Scale`: the default of the Print
/// dialog's To Scale and of Send to Layout (manual p. 1426).
pub fn default_scale(project: &plan_core::Project, view: ViewType) -> Scale {
    scale_of(&project.print_setup.sheet_for(view))
}

/// Refreshes [`context`] from the editor and the camera, and makes the
/// editor's sheet follow the plan's Drawing Sheet Setup when that changed.
/// Called every frame by the canvas.
pub fn sync(cx: &mut EditorContext, cam: &Camera) {
    let view = current_view(cx);
    let own =
        cx.project.print_setup.has_own(view) || cx.project.print_setup.has_own(ViewType::Plan);
    let vs = cx.project.print_setup.sheet_for(view);
    // The editor's sheet follows a setup the plan has, when it changes.
    if own && matches!(view, ViewType::Plan | ViewType::CadDetail) {
        let now = (view, vs.clone());
        let changed = APPLIED.with(|a| a.borrow().as_ref() != Some(&now));
        if changed {
            apply_to_sheet(&mut cx.sheet, &vs);
            APPLIED.with(|a| *a.borrow_mut() = Some(now));
        }
    }
    super::watermark::set_menu_state(super::watermark::is_on(cx));
    let center = esheet::plan_center(cx.floor());
    let paper = cx.sheet.paper_inches();
    let rect = cam.rect;
    let (a, b) = (
        cam.screen_to_world(rect.left_top()),
        cam.screen_to_world(rect.right_bottom()),
    );
    let ctx = PrintViewContext {
        view,
        sheet_shown: cx.view_flags.contains(&ViewFlag::DrawingSheet),
        sheet_window: plan_layout::drawing_sheet_window(center, paper, cx.sheet.scale),
        view_window: Some(plan_layout::window_of(a, b)),
        plan_extent: plan_layout::plan_extent(&cx.project, cx.floor),
        watermark_on: super::watermark::is_on(cx),
        watermark_ready: cx.project.print_setup.watermark.spec.has_content(),
        floor: cx.floor,
        sheet: vs,
        layout_sheet: cx.project.print_setup.sheet_for(ViewType::Layout),
        synced: true,
    };
    CONTEXT.with(|c| {
        if *c.borrow() != ctx {
            *c.borrow_mut() = ctx;
        }
    });
}

/// Makes the editor's sheet (`EditorContext::sheet`) match a setup: the
/// standard size it names (a custom size or an upright sheet keeps the nearest
/// standard size with its own dimensions) and the Drawing Scale.
pub fn apply_to_sheet(sheet: &mut esheet::SheetSetup, vs: &ViewSheet) {
    let (long, short) = (vs.sheet.long_in, vs.sheet.short_in);
    let exact = SheetSize::ALL.into_iter().find(|s| {
        s.label() == vs.sheet.name || {
            let (w, h) = s.inches();
            (w - long).abs() < 1e-6 && (h - short).abs() < 1e-6
        }
    });
    let size = exact.unwrap_or_else(|| nearest_standard(long, short));
    let (w, h) = vs.inches();
    let upright_or_custom = vs.portrait || exact.is_none();
    sheet.size = size;
    sheet.paper_cin = upright_or_custom.then(|| {
        (
            size,
            ((w * 100.0).round() as u32, (h * 100.0).round() as u32),
        )
    });
    sheet.scale = scale_of(vs);
}

/// The smallest standard size that holds a `long` x `short` sheet (the
/// largest when none does).
fn nearest_standard(long: f64, short: f64) -> SheetSize {
    SheetSize::ALL
        .into_iter()
        .filter(|s| {
            let (w, h) = s.inches();
            w + 1e-6 >= long && h + 1e-6 >= short
        })
        .min_by(|a, b| {
            let area = |s: &SheetSize| s.inches().0 * s.inches().1;
            area(a).total_cmp(&area(b))
        })
        .unwrap_or(SheetSize::ArchE)
}

/// The standard size a sheet is, if it is one.
pub fn standard_of(sheet: &SheetDims) -> Option<SheetSize> {
    SheetSize::ALL.into_iter().find(|s| {
        s.label() == sheet.name || {
            let (w, h) = s.inches();
            (w - sheet.long_in).abs() < 1e-6 && (h - sheet.short_in).abs() < 1e-6
        }
    })
}

// ------------------------------------------------------------ commands --

/// Runs a command of this module; the status text goes to the status bar.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPEN => {
            if crate::shell::layout_window::is_active() {
                cx.status = "A layout's sheet is set in Layout > Page Setup".into();
            } else {
                open(cx, current_view(cx));
            }
        }
        SCALE_TO_FIT => cx.status = scale_to_fit(cx),
        CENTER_SHEET => cx.status = center_sheet(cx),
        CLEAR_PRINTER => cx.status = clear_printer_info(cx),
        CUSTOMIZE => open_customize(),
        _ => return false,
    }
    true
}

/// Scale to Fit: the largest Drawing Scale at which the floor's plan fits the
/// sheet inside its Drawing Margins, and the sheet re-centered on the plan.
/// One undo step. The status text.
pub fn scale_to_fit(cx: &mut EditorContext) -> String {
    let view = plan_like(current_view(cx));
    let Some(extent) = plan_layout::plan_extent(&cx.project, cx.floor) else {
        return "Nothing to fit: the floor has no walls or dimensions".into();
    };
    let mut vs = cx.project.print_setup.sheet_for(view);
    let s = plan_layout::scale_to_fit(extent, vs.inches(), vs.margins_in);
    let metric = vs.scale.paper_unit.is_metric() && vs.scale.real_unit.is_metric();
    vs.scale = DrawingScale::from_inches_per_foot(s.inches_per_foot(), metric);
    cx.begin_change("Scale to Fit");
    cx.project.print_setup.set_sheet(view, vs.clone());
    cx.floor_mut().sheet_center = Some(plan_layout::extent_center(extent));
    cx.mark_dirty();
    apply_to_sheet(&mut cx.sheet, &vs);
    APPLIED.with(|a| *a.borrow_mut() = Some((view, vs.clone())));
    format!("Scale to Fit: {} on {}", vs.scale.label(), vs.sheet.name)
}

/// Center Sheet: puts the Drawing Sheet on the middle of the floor's plan.
/// Only the sheet moves (`Floor::sheet_center`); the status text.
pub fn center_sheet(cx: &mut EditorContext) -> String {
    let Some(extent) = plan_layout::plan_extent(&cx.project, cx.floor) else {
        return "Nothing to center on: the floor has no walls or dimensions".into();
    };
    cx.begin_change("Center Sheet");
    cx.floor_mut().sheet_center = Some(plan_layout::extent_center(extent));
    cx.mark_dirty();
    "Centered the Drawing Sheet on this floor's plan".into()
}

/// Clear Printer Info: forgets the printers the Drawing Sheet Setups name
/// (for a plan or layout template that has no printer).
pub fn clear_printer_info(cx: &mut EditorContext) -> String {
    let any = cx
        .project
        .print_setup
        .views
        .values()
        .any(|v| v.printer.is_some());
    if !any {
        return "No printer information is stored with this plan".into();
    }
    cx.begin_change("Clear Printer Info");
    for v in cx.project.print_setup.views.values_mut() {
        v.printer = None;
    }
    cx.mark_dirty();
    "Cleared the printer information".into()
}

/// The plan setup stands for a view that has no setup of its own kind in
/// these commands (Scale to Fit and Center Sheet work on the plan or detail).
fn plan_like(v: ViewType) -> ViewType {
    match v {
        ViewType::CadDetail => ViewType::CadDetail,
        _ => ViewType::Plan,
    }
}

// -------------------------------------------------------------- dialogs --

#[derive(Default)]
struct Dialogs {
    setup: Option<SetupDialog>,
    sizes: Option<SheetSizesDialog>,
    printers: Option<Printers>,
}

/// Drawing Sheet Setup: edits a draft of one view type's setup.
struct SetupDialog {
    view: ViewType,
    draft: ViewSheet,
    show_sheet: bool,
}

/// Opens Drawing Sheet Setup on the setup of `view`.
pub fn open(cx: &EditorContext, view: ViewType) {
    DIALOGS.with(|d| {
        d.borrow_mut().setup = Some(SetupDialog {
            view,
            draft: cx.project.print_setup.sheet_for(view),
            show_sheet: cx.view_flags.contains(&ViewFlag::DrawingSheet),
        });
    });
}

pub fn setup_is_open() -> bool {
    DIALOGS.with(|d| d.borrow().setup.is_some())
}

/// Opens Customize Sheet Sizes on the program-wide list.
pub fn open_customize() {
    let file = plan_layout::global_sheet_sizes();
    DIALOGS.with(|d| {
        d.borrow_mut().sizes = Some(SheetSizesDialog::new(
            SheetSizes {
                custom: file.custom,
                hidden: file.hidden,
            },
            SheetSize::ArchD,
        ));
    });
}

pub fn customize_is_open() -> bool {
    DIALOGS.with(|d| d.borrow().sizes.is_some())
}

/// Draws the print dialogs that are open (Drawing Sheet Setup, Customize Sheet
/// Sizes, Watermark Defaults). Called every frame.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    // The layout window is up without the canvas: keep its sheet current.
    let layout_sheet = cx.project.print_setup.sheet_for(ViewType::Layout);
    CONTEXT.with(|c| {
        let mut c = c.borrow_mut();
        if c.layout_sheet != layout_sheet {
            c.layout_sheet = layout_sheet;
        }
    });
    show_setup(ctx, cx);
    show_sizes(ctx, cx);
    super::watermark::show(ctx, cx);
}

fn show_sizes(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = DIALOGS.with(|s| s.borrow_mut().sizes.take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => DIALOGS.with(|s| s.borrow_mut().sizes = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            let sizes = d.sizes().clone();
            let file = plan_layout::SheetSizeFile {
                version: plan_layout::SheetSizeFile::VERSION,
                custom: sizes.custom,
                hidden: sizes.hidden,
            };
            cx.status = match plan_layout::set_global_sheet_sizes(file) {
                Ok(()) => "Sheet sizes updated for every plan and layout".into(),
                Err(e) => format!("The sheet sizes could not be saved: {e}"),
            };
        }
    }
}

fn show_setup(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = DIALOGS.with(|s| s.borrow_mut().setup.take()) else {
        return;
    };
    let error = d.draft.problem();
    let mut customize = false;
    let printers = DIALOGS.with(|s| s.borrow().printers.clone());
    let mut want_printers = false;
    let before = d.view;
    let out = super::layout::frame(ctx, "Drawing Sheet Setup", 500.0, error, |ui| {
        setup_body(ui, &mut d, &printers, &mut want_printers, &mut customize);
    });
    if d.view != before {
        // Another kind of view: its setup is what the dialog shows.
        d.draft = cx.project.print_setup.sheet_for(d.view);
    }
    if want_printers && printers.is_none() {
        DIALOGS.with(|s| s.borrow_mut().printers = Some(list_printers()));
    }
    if customize {
        open_customize();
    }
    match out {
        Outcome::Open => DIALOGS.with(|s| s.borrow_mut().setup = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            cx.begin_change("Drawing Sheet Setup");
            cx.project.print_setup.set_sheet(d.view, d.draft.clone());
            cx.mark_dirty();
            if matches!(d.view, ViewType::Plan | ViewType::CadDetail) && d.view == current_view(cx)
            {
                apply_to_sheet(&mut cx.sheet, &d.draft);
                APPLIED.with(|a| *a.borrow_mut() = Some((d.view, d.draft.clone())));
            }
            if d.show_sheet {
                cx.view_flags.insert(ViewFlag::DrawingSheet);
            } else {
                cx.view_flags.remove(&ViewFlag::DrawingSheet);
            }
            cx.status = format!(
                "Drawing Sheet Setup: {} at {}",
                d.draft.sheet.name,
                d.draft.scale.label()
            );
        }
    }
}

fn setup_body(
    ui: &mut egui::Ui,
    d: &mut SetupDialog,
    printers: &Option<Printers>,
    want_printers: &mut bool,
    customize: &mut bool,
) {
    row(ui, "View", |ui| {
        egui::ComboBox::from_id_salt("drawing_sheet_view")
            .selected_text(d.view.label())
            .show_ui(ui, |ui| {
                for v in [
                    ViewType::Plan,
                    ViewType::Elevation,
                    ViewType::CadDetail,
                    ViewType::MaterialsList,
                ] {
                    ui.selectable_value(&mut d.view, v, v.label());
                }
            });
    });
    section(ui, "Drawing Sheet");
    row(ui, "Orientation", |ui| {
        ui.radio_value(&mut d.draft.portrait, false, "Landscape");
        ui.radio_value(&mut d.draft.portrait, true, "Portrait");
    });
    row(ui, "Size", |ui| {
        let keep = standard_of(&d.draft.sheet);
        let choices = plan_layout::global_sheet_sizes().choices(keep);
        egui::ComboBox::from_id_salt("drawing_sheet_size")
            .selected_text(d.draft.sheet.name.clone())
            .width(220.0)
            .show_ui(ui, |ui| {
                for c in &choices {
                    let (w, h) = c.inches();
                    let name = match c {
                        SheetChoice::Standard(s) => s.label().to_string(),
                        SheetChoice::Custom(c) => c.label(),
                    };
                    if ui
                        .selectable_label(d.draft.sheet.name == name, &name)
                        .clicked()
                    {
                        d.draft.sheet = SheetDims::new(name, w, h);
                    }
                }
            });
        if ui.button("Customize\u{2026}").clicked() {
            *customize = true;
        }
    });
    ui.checkbox(&mut d.show_sheet, "Show Drawing Sheet in View");

    section(ui, "Drawing Scale");
    if d.view.scaled() {
        scale_row(ui, &mut d.draft.scale);
        row(ui, "Common scales", |ui| {
            egui::ComboBox::from_id_salt("drawing_sheet_presets")
                .selected_text("Choose\u{2026}")
                .show_ui(ui, |ui| {
                    for s in Scale::choices() {
                        if ui.selectable_label(false, s.label()).clicked() {
                            d.draft.scale =
                                DrawingScale::from_inches_per_foot(s.inches_per_foot(), false);
                        }
                    }
                    for n in [20, 50, 100, 200] {
                        if ui.selectable_label(false, format!("1:{n}")).clicked() {
                            d.draft.scale = DrawingScale::ratio_mm(f64::from(n));
                        }
                    }
                });
        });
        ui.weak(format!(
            "Print and Send to Layout start at {} (a layout is 1 in = 1 in).",
            d.draft.scale.label()
        ));
    } else {
        ui.weak("A Materials List does not scale.");
    }

    section(ui, "Printer for View");
    ui.checkbox(
        &mut d.draft.remember_print_settings,
        "Remember Print Settings after Printing",
    );
    row(ui, "Printer", |ui| {
        let shown = if d.draft.remember_print_settings {
            "Chosen in the Print dialog".to_string()
        } else {
            d.draft
                .printer
                .clone()
                .unwrap_or_else(|| "Default printer".to_string())
        };
        ui.label(shown);
        ui.add_enabled_ui(!d.draft.remember_print_settings, |ui| {
            egui::ComboBox::from_id_salt("drawing_sheet_printer")
                .selected_text("Choose\u{2026}")
                .show_ui(ui, |ui| {
                    *want_printers = true;
                    if ui
                        .selectable_label(d.draft.printer.is_none(), "Default printer")
                        .clicked()
                    {
                        d.draft.printer = None;
                    }
                    for n in printers
                        .as_ref()
                        .map(|p| p.names.clone())
                        .unwrap_or_default()
                    {
                        if ui
                            .selectable_label(d.draft.printer.as_deref() == Some(&n), &n)
                            .clicked()
                        {
                            d.draft.printer = Some(n);
                        }
                    }
                });
        });
    });

    section(ui, "Drawing Margins");
    if ui
        .button("Populate from Printer")
        .on_hover_text(
            "Margins a printer cannot print in: 1/4 inch on every side for a printer, none for PDF",
        )
        .clicked()
    {
        d.draft.margins_in =
            populate_margins(d.draft.printer.is_some() || !printers_none(printers));
    }
    egui::Grid::new("drawing_margins")
        .num_columns(4)
        .show(ui, |ui| {
            for (i, name) in ["Top", "Bottom", "Left", "Right"].into_iter().enumerate() {
                ui.label(name);
                ui.add(
                    egui::DragValue::new(&mut d.draft.margins_in[i])
                        .range(0.0..=100.0)
                        .speed(0.05)
                        .max_decimals(3)
                        .suffix("\""),
                );
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });

    section(ui, "Advanced Line Weights");
    ui.checkbox(
        &mut d.draft.weights.single_weight,
        "Use 1 for all line weights (Home Designer compatibility)",
    );
    ui.add_enabled_ui(!d.draft.weights.single_weight, |ui| {
        row(ui, "Line Weight Scale", |ui| {
            ui.label("1 = 1 /");
            ui.add(
                egui::DragValue::new(&mut d.draft.weights.denominator)
                    .range(1.0..=100_000.0)
                    .speed(1.0),
            );
            egui::ComboBox::from_id_salt("drawing_sheet_weight_unit")
                .selected_text(if d.draft.weights.unit_mm { "mm" } else { "in" })
                .width(60.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.draft.weights.unit_mm, true, "mm");
                    ui.selectable_value(&mut d.draft.weights.unit_mm, false, "in");
                });
        });
    });
    weight_preview(ui, &d.draft.weights);
    ui.weak("A layout and the plan views sent to it must share one Line Weight Scale.");
}

fn printers_none(p: &Option<Printers>) -> bool {
    p.as_ref().is_none_or(|p| p.names.is_empty())
}

/// Populate from Printer: CUPS gives no portable imageable area, so a printer
/// (or the default one) gets 1/4 inch each side and "no printer" none.
pub fn populate_margins(has_printer: bool) -> [f64; 4] {
    if has_printer {
        [0.25; 4]
    } else {
        [0.0; 4]
    }
}

/// The two-part Drawing Scale: `[paper] [unit] = [real] [unit]`.
fn scale_row(ui: &mut egui::Ui, s: &mut DrawingScale) {
    row(ui, "Scale", |ui| {
        ui.add(
            egui::DragValue::new(&mut s.paper)
                .range(0.001..=1000.0)
                .speed(0.01)
                .max_decimals(4),
        );
        unit_combo(ui, "drawing_scale_paper_unit", &mut s.paper_unit);
        ui.label("=");
        ui.add(
            egui::DragValue::new(&mut s.real)
                .range(0.001..=100_000.0)
                .speed(0.1)
                .max_decimals(3),
        );
        unit_combo(ui, "drawing_scale_real_unit", &mut s.real_unit);
    });
}

fn unit_combo(ui: &mut egui::Ui, salt: &str, unit: &mut LenUnit) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(unit.label())
        .width(54.0)
        .show_ui(ui, |ui| {
            for u in LenUnit::ALL {
                ui.selectable_value(unit, u, u.label());
            }
        });
}

/// Preview of Line Weight: sample lines for weights 5, 10, 25 and 50 at the
/// line weight scale, drawn at 96 pixels to the inch.
fn weight_preview(ui: &mut egui::Ui, w: &LineWeightSetup) {
    ui.horizontal(|ui| {
        ui.label("Preview");
        for n in [5.0, 10.0, 25.0, 50.0] {
            let mm = if w.single_weight {
                LineWeightSetup::SINGLE_WEIGHT_PT / 72.0 * 25.4
            } else {
                n * w.weight_mm()
            };
            let px = (mm / 25.4 * 96.0) as f32;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(50.0, 16.0), egui::Sense::hover());
            ui.painter().line_segment(
                [rect.left_center(), rect.right_center()],
                Stroke::new(px.clamp(0.25, 12.0), ui.visuals().text_color()),
            );
            ui.weak(if w.single_weight {
                "1".to_string()
            } else {
                format!("{n:.0}")
            });
        }
    });
}

// ------------------------------------------------------------- on screen --

/// The blue border (printable area) and the watermark over the plan, drawn
/// after the plan: the Drawing Sheet itself is drawn with the plan
/// (`editor::render`).
pub fn paint_overlays(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let geo = sheet_geometry(cx);
    if let Some(spec) = super::watermark::active_spec(cx) {
        super::watermark::paint(painter, cam, &spec, &geo);
    }
    let sheet_on = cx.view_flags.contains(&ViewFlag::DrawingSheet)
        || cx.view_flags.contains(&ViewFlag::PrintPreview);
    if sheet_on {
        paint_printable_border(painter, cam, &geo);
        if cx.view_flags.contains(&ViewFlag::DrawingSheet) {
            paint_handles(painter, cam, &geo);
        }
    }
}

/// Where the active floor's Drawing Sheet is.
pub fn sheet_geometry(cx: &EditorContext) -> super::watermark::SheetGeometry {
    let (lo, _) = cx.sheet.rect_around(esheet::plan_center(cx.floor()));
    let view = plan_like(current_view(cx));
    let margins = cx.project.print_setup.sheet_for(view).margins_in;
    super::watermark::SheetGeometry {
        lo,
        paper_in: cx.sheet.paper_inches(),
        plan_per_paper: 12.0 / cx.sheet.scale.inches_per_foot(),
        margins,
    }
}

const BLUE: Color32 = Color32::from_rgb(0x3b, 0x7d, 0xd8);

/// The printable area of the sheet, inside the Drawing Margins `[top, bottom,
/// left, right]`.
fn paint_printable_border(
    painter: &egui::Painter,
    cam: &Camera,
    g: &super::watermark::SheetGeometry,
) {
    let [t, b, l, r] = g.margins;
    if g.margins.iter().all(|m| *m <= 0.0) {
        return;
    }
    let (w, h) = g.paper_in;
    let corners = [(l, b), (w - r, b), (w - r, h - t), (l, h - t)]
        .map(|(x, y)| cam.world_to_screen(g.world(x, y)));
    painter.add(egui::Shape::closed_line(
        corners.to_vec(),
        Stroke::new(1.0_f32, BLUE),
    ));
}

fn paint_handles(painter: &egui::Painter, cam: &Camera, g: &super::watermark::SheetGeometry) {
    for p in handle_points(cam, g) {
        let r = egui::Rect::from_center_size(p, egui::vec2(8.0, 8.0));
        painter.rect_filled(r, 1.0_f32, Color32::WHITE);
        painter.rect_stroke(
            r,
            1.0_f32,
            Stroke::new(1.0_f32, BLUE),
            egui::StrokeKind::Middle,
        );
    }
}

/// The four corner handles of the sheet, on screen.
fn handle_points(cam: &Camera, g: &super::watermark::SheetGeometry) -> [Pos2; 4] {
    let (w, h) = g.paper_in;
    [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(x, y)| cam.world_to_screen(g.world(x, y)))
}

/// Which corner (index into [`handle_points`]) is under `p`, if any.
pub fn handle_at(cam: &Camera, g: &super::watermark::SheetGeometry, p: Pos2) -> Option<usize> {
    handle_points(cam, g)
        .iter()
        .position(|h| (h.x - p.x).abs() <= 7.0 && (h.y - p.y).abs() <= 7.0)
}

/// Is `p` within `tol` pixels of the sheet's border?
pub fn on_border(cam: &Camera, g: &super::watermark::SheetGeometry, p: Pos2, tol: f32) -> bool {
    let pts = handle_points(cam, g);
    (0..4).any(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % 4]);
        let ab = b - a;
        let len2 = ab.length_sq().max(1e-6);
        let t = (((p - a).dot(ab)) / len2).clamp(0.0, 1.0);
        (a + ab * t - p).length() <= tol
    })
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    /// Moving the sheet: the pointer's world position at the start and the
    /// sheet's centre then.
    Move { grab: Point, center: Point },
    /// Resizing from a corner: the opposite corner stays.
    Resize { anchor: Point },
}

/// Pointer handling for the Drawing Sheet object while View > Drawing Sheet is
/// on and the Select tool is active: a drag on the border moves the sheet, a
/// drag on a corner handle resizes it; one undo step each. Returns true when
/// the sheet took the pointer (the tools must not see it).
pub fn pointer(
    ctx: &egui::Context,
    resp: &egui::Response,
    cx: &mut EditorContext,
    cam: &Camera,
    select_tool: bool,
) -> bool {
    let shown = cx.view_flags.contains(&ViewFlag::DrawingSheet);
    let (pressed, down, pos) = ctx.input(|i| {
        (
            i.pointer.button_pressed(egui::PointerButton::Primary),
            i.pointer.button_down(egui::PointerButton::Primary),
            i.pointer.latest_pos(),
        )
    });
    let active = DRAG.with(|d| *d.borrow());
    if let Some(drag) = active {
        if !down {
            DRAG.with(|d| *d.borrow_mut() = None);
            cx.mark_dirty();
            return true;
        }
        let Some(pos) = pos else { return true };
        let now = cam.screen_to_world(pos);
        drag_to(cx, drag, now);
        ctx.request_repaint();
        return true;
    }
    if !(shown && select_tool && pressed && resp.hovered()) {
        return false;
    }
    let Some(pos) = pos else { return false };
    let g = sheet_geometry(cx);
    let world = cam.screen_to_world(pos);
    if let Some(i) = handle_at(cam, &g, pos) {
        // The opposite corner of the one grabbed.
        let (w, h) = g.paper_in;
        let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
        let (ax, ay) = corners[(i + 2) % 4];
        cx.begin_change("Resize Drawing Sheet");
        DRAG.with(|d| {
            *d.borrow_mut() = Some(Drag::Resize {
                anchor: g.world(ax, ay),
            })
        });
        return true;
    }
    if on_border(cam, &g, pos, 6.0) {
        let (lo, hi) = cx.sheet.rect_around(esheet::plan_center(cx.floor()));
        let center = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        cx.begin_change("Move Drawing Sheet");
        DRAG.with(|d| {
            *d.borrow_mut() = Some(Drag::Move {
                grab: world,
                center,
            })
        });
        return true;
    }
    false
}

fn drag_to(cx: &mut EditorContext, drag: Drag, now: Point) {
    match drag {
        Drag::Move { grab, center } => {
            cx.floor_mut().sheet_center = Some(Point::new(
                center.x + now.x - grab.x,
                center.y + now.y - grab.y,
            ));
        }
        Drag::Resize { anchor } => {
            let k = 12.0 / cx.sheet.scale.inches_per_foot();
            let w = ((now.x - anchor.x).abs() / k).max(2.0);
            let h = ((now.y - anchor.y).abs() / k).max(2.0);
            let view = plan_like(current_view(cx));
            let mut vs = cx.project.print_setup.sheet_for(view);
            vs.sheet = SheetDims::new(
                format!("Custom ({} x {})", short(w.min(h)), short(w.max(h))),
                w,
                h,
            );
            vs.portrait = h > w;
            cx.project.print_setup.set_sheet(view, vs.clone());
            apply_to_sheet(&mut cx.sheet, &vs);
            APPLIED.with(|a| *a.borrow_mut() = Some((view, vs)));
            cx.floor_mut().sheet_center = Some(Point::new(
                (now.x + anchor.x) * 0.5,
                (now.y + anchor.y) * 0.5,
            ));
        }
    }
}

fn short(v: f64) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx() -> EditorContext {
        EditorContext::new(crate::plan_defaults::embedded())
    }

    fn house(cx: &mut EditorContext, w_ft: f64, h_ft: f64) {
        use plan_core::WallKind;
        let (w, h) = (w_ft * 12.0, h_ft * 12.0);
        let p = [
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, p[i], p[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
    }

    #[test]
    fn scale_to_fit_sets_the_scale_and_centers_the_sheet_in_one_undo_step() {
        let mut cx = cx();
        house(&mut cx, 60.0, 40.0);
        let msg = scale_to_fit(&mut cx);
        assert!(msg.contains("1/2\" = 1'"), "{msg}");
        let vs = cx.project.print_setup.sheet_for(ViewType::Plan);
        assert_eq!(scale_of(&vs), Scale::HalfInch);
        assert_eq!(cx.sheet.scale, Scale::HalfInch);
        let c = cx.floor().sheet_center.unwrap();
        assert!(
            c.x > 300.0 && c.x < 400.0 && c.y > 200.0 && c.y < 300.0,
            "{c:?}"
        );
        assert_eq!(cx.undo_label(), Some("Scale to Fit"));
        cx.undo();
        assert!(cx.floor().sheet_center.is_none());
        assert!(!cx.project.print_setup.has_own(ViewType::Plan));
        // No walls: nothing to fit.
        let mut empty = super::tests::cx();
        assert!(scale_to_fit(&mut empty).contains("Nothing to fit"));
    }

    #[test]
    fn center_sheet_moves_only_the_sheet() {
        let mut cx = cx();
        house(&mut cx, 40.0, 30.0);
        let walls_before: Vec<_> = cx.floor().walls.iter().map(|w| (w.start, w.end)).collect();
        center_sheet(&mut cx);
        let c = cx.floor().sheet_center.unwrap();
        assert!(
            (c.x - 240.0).abs() < 1.0 && (c.y - 180.0).abs() < 1.0,
            "{c:?}"
        );
        let after: Vec<_> = cx.floor().walls.iter().map(|w| (w.start, w.end)).collect();
        assert_eq!(walls_before, after, "coordinates do not change");
        // The on-screen sheet is centered there.
        let (lo, hi) = cx.sheet.rect_around(esheet::plan_center(cx.floor()));
        assert!(((lo.x + hi.x) * 0.5 - c.x).abs() < 1e-9);
        // Each floor keeps its own.
        cx.project.floors.push(plan_core::Floor::new("2nd", 108.0));
        assert!(cx.project.floors[1].sheet_center.is_none());
    }

    #[test]
    fn the_editor_sheet_follows_the_plan_setup_including_custom_and_upright_sheets() {
        let mut sheet = esheet::SheetSetup::default();
        let mut vs = ViewSheet {
            sheet: SheetDims::new("ARCH C (18 x 24)", 24.0, 18.0),
            scale: DrawingScale::per_foot(0.125),
            ..ViewSheet::default()
        };
        apply_to_sheet(&mut sheet, &vs);
        assert_eq!(sheet.size, SheetSize::ArchC);
        assert_eq!(sheet.scale, Scale::EighthInch);
        assert!(sheet.paper_cin.is_none());
        assert_eq!(sheet.paper_inches(), (24.0, 18.0));
        vs.portrait = true;
        apply_to_sheet(&mut sheet, &vs);
        assert_eq!(sheet.paper_inches(), (18.0, 24.0));
        // 18 x 24 at 1/8" = 1' covers 144' x 192'.
        assert_eq!(sheet.world_size(), (18.0 * 96.0, 24.0 * 96.0));
        // A custom 30 x 40 sheet draws on the smallest standard size that holds it.
        vs.portrait = false;
        vs.sheet = SheetDims::new("Poster (30 x 40)", 40.0, 30.0);
        apply_to_sheet(&mut sheet, &vs);
        assert_eq!(sheet.size, SheetSize::ArchE1, "42 x 30 holds 40 x 30");
        assert_eq!(sheet.paper_inches(), (40.0, 30.0));
        assert!(sheet.caption().starts_with("40 x 30 in"));
    }

    #[test]
    fn sync_applies_a_stored_setup_but_leaves_an_untouched_plan_alone() {
        let mut cx = cx();
        let cam = Camera::default_view();
        cx.sheet.scale = Scale::EighthInch;
        sync(&mut cx, &cam);
        assert_eq!(
            cx.sheet.scale,
            Scale::EighthInch,
            "no setup stored: not touched"
        );
        let vs = ViewSheet {
            scale: DrawingScale::per_foot(0.5),
            ..ViewSheet::default()
        };
        cx.project.print_setup.set_sheet(ViewType::Plan, vs);
        sync(&mut cx, &cam);
        assert_eq!(cx.sheet.scale, Scale::HalfInch);
        // The context carries the setup, the sheet's footprint and the window on screen.
        let c = context();
        assert_eq!(c.view, ViewType::Plan);
        assert_eq!(scale_of(&c.sheet), Scale::HalfInch);
        // ARCH D at 1/2" = 1' covers 72' x 48'.
        let w = c.sheet_window;
        assert!(
            (w[2] - w[0] - 36.0 * 24.0).abs() < 1e-6 && (w[3] - w[1] - 24.0 * 24.0).abs() < 1e-6
        );
        assert!(c.view_window.is_some());
        // The Drawing Scale feeds Send to Layout's default.
        assert_eq!(default_scale(&cx.project, ViewType::Plan), Scale::HalfInch);
        assert_eq!(
            default_scale(&cx.project, ViewType::Elevation),
            Scale::HalfInch
        );
    }

    #[test]
    fn the_setup_dialog_stores_one_undo_step_and_shows_the_sheet() {
        let mut cx = cx();
        let ctx = egui::Context::default();
        open(&cx, ViewType::Plan);
        assert!(setup_is_open());
        DIALOGS.with(|d| {
            let mut d = d.borrow_mut();
            let s = d.setup.as_mut().unwrap();
            s.draft.sheet = SheetDims::new("ARCH C (18 x 24)", 24.0, 18.0);
            s.draft.scale = DrawingScale::per_foot(0.1875);
            s.draft.margins_in = [0.5, 0.5, 1.0, 0.25];
            s.show_sheet = true;
        });
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |c| show_all(c, &mut cx));
        }
        assert!(
            !cx.project.print_setup.has_own(ViewType::Plan),
            "not until OK"
        );
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(input, |c| show_all(c, &mut cx));
        assert!(!setup_is_open());
        let vs = cx.project.print_setup.sheet_for(ViewType::Plan);
        assert_eq!(vs.margins_in, [0.5, 0.5, 1.0, 0.25]);
        assert_eq!(cx.sheet.size, SheetSize::ArchC);
        assert_eq!(cx.sheet.scale, Scale::ThreeSixteenths);
        assert!(cx.view_flags.contains(&ViewFlag::DrawingSheet));
        assert_eq!(cx.undo_label(), Some("Drawing Sheet Setup"));
        cx.undo();
        assert!(!cx.project.print_setup.has_own(ViewType::Plan));
    }

    #[test]
    fn a_new_view_type_starts_from_the_plan_setup() {
        let mut cx = cx();
        let vs = ViewSheet {
            scale: DrawingScale::per_foot(0.5),
            sheet: SheetDims::new("ARCH B (12 x 18)", 18.0, 12.0),
            ..ViewSheet::default()
        };
        cx.project.print_setup.set_sheet(ViewType::Plan, vs.clone());
        open(&cx, ViewType::Elevation);
        let got = DIALOGS.with(|d| d.borrow().setup.as_ref().map(|s| s.draft.clone()));
        assert_eq!(got, Some(vs));
    }

    #[test]
    fn clearing_printer_info_forgets_every_named_printer() {
        let mut cx = cx();
        assert!(clear_printer_info(&mut cx).contains("No printer"));
        let vs = ViewSheet {
            printer: Some("HP_LaserJet".into()),
            ..ViewSheet::default()
        };
        cx.project.print_setup.set_sheet(ViewType::Plan, vs.clone());
        cx.project.print_setup.set_sheet(ViewType::Layout, vs);
        assert!(clear_printer_info(&mut cx).contains("Cleared"));
        assert!(cx
            .project
            .print_setup
            .views
            .values()
            .all(|v| v.printer.is_none()));
        assert_eq!(cx.undo_label(), Some("Clear Printer Info"));
    }

    #[test]
    fn populating_margins_and_handles() {
        assert_eq!(populate_margins(true), [0.25; 4]);
        assert_eq!(populate_margins(false), [0.0; 4]);
        let mut cam = Camera::default_view();
        cam.center = Point::new(0.0, 0.0);
        cam.px_per_in = 1.0;
        let g = super::super::watermark::SheetGeometry {
            lo: Point::new(-100.0, -100.0),
            paper_in: (10.0, 10.0),
            plan_per_paper: 20.0,
            margins: [0.25; 4],
        };
        let corner = cam.world_to_screen(g.world(10.0, 10.0));
        assert_eq!(handle_at(&cam, &g, corner), Some(2));
        let mid_top = cam.world_to_screen(g.world(5.0, 10.0));
        assert!(on_border(&cam, &g, mid_top, 3.0));
        assert!(handle_at(&cam, &g, mid_top).is_none());
        let inside = cam.world_to_screen(g.world(5.0, 5.0));
        assert!(!on_border(&cam, &g, inside, 3.0));
    }

    #[test]
    fn dragging_the_sheet_moves_it_and_resizing_makes_a_custom_size() {
        let mut cx = cx();
        house(&mut cx, 40.0, 30.0);
        let start = esheet::plan_center(cx.floor());
        cx.begin_change("Move Drawing Sheet");
        drag_to(
            &mut cx,
            Drag::Move {
                grab: Point::new(0.0, 0.0),
                center: start,
            },
            Point::new(120.0, -60.0),
        );
        let c = cx.floor().sheet_center.unwrap();
        assert!((c.x - start.x - 120.0).abs() < 1e-9 && (c.y - start.y + 60.0).abs() < 1e-9);
        // Resize: the corner opposite stays; ARCH D at 1/4" = 1' is 48 plan inches per paper inch.
        cx.begin_change("Resize Drawing Sheet");
        drag_to(
            &mut cx,
            Drag::Resize {
                anchor: Point::new(0.0, 0.0),
            },
            Point::new(48.0 * 20.0, 48.0 * 10.0),
        );
        let vs = cx.project.print_setup.sheet_for(ViewType::Plan);
        assert_eq!((vs.sheet.long_in, vs.sheet.short_in), (20.0, 10.0));
        assert!(vs.sheet.name.starts_with("Custom"));
        assert_eq!(cx.sheet.paper_inches(), (20.0, 10.0));
        let c = cx.floor().sheet_center.unwrap();
        assert_eq!((c.x, c.y), (480.0, 240.0));
    }
}
