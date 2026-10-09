//! File > Print: the Print dialog for a layout or a plan view, Print Image, and
//! the delivery of the finished PDF (a file, the system printer or a viewer).
//!
//! The dialog follows Chief's Print View dialog (manual pp. 1440-1443):
//! Destination (printer or PDF, DPI), Paper (size, orientation, source, Match
//! Print Source), Print Range, Print Source (the Drawing Sheet or the current
//! view), Drawing Scale (Fit to Paper at a global percentage, To Scale, Check
//! Plot at a fraction), Options (copies and collate, Include Watermark, print
//! in colour) and the information messages. It edits
//! [`plan_layout::PrintOptions`]; `shell::layout_window` builds the PDF from
//! the answers and [`deliver`] gets it to its destination.
//!
//! Settings are remembered per kind of view (plan, cross section / elevation,
//! CAD Detail, layout, Materials List) in `~/.plan-studio/printsettings.json`,
//! for every plan, unless the view's Drawing Sheet Setup turns Remember Print
//! Settings after Printing off. Copies and the page range are never kept. The
//! Fit to Paper percentage is global (default 95).
//!
//! What the dialog needs to know about the active view (its Drawing Sheet
//! Setup, the Drawing Sheet's footprint, what is on screen) it reads from
//! [`drawing_sheet::context`], which the canvas keeps current.
//!
//! PDF is the portable path and works everywhere. The system printer uses
//! CUPS' `lp` (macOS and Linux), to the printer picked from the list that
//! `lpstat -p` gives (the default printer otherwise); Windows saves the PDF
//! instead. "Open in
//! viewer" uses `open -a Preview` on macOS, `xdg-open` on Linux and `start` on
//! Windows.

use super::drawing_sheet::{self, scale_of, PrintViewContext};
use super::layout::frame;
use super::{row, section, Outcome};
use eframe::egui::{self, Ui};
use plan_core::drawing_sheet::ViewType;
use plan_docs::{Scale, SheetSize};
use plan_layout::{
    tile_grid, PaperSize, PenSetup, PrintColor, PrintOptions, PrintScale, TileGrid,
    CHECK_PLOT_FRACTIONS, DEFAULT_FIT_PERCENT,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

/// Where the print goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destination {
    /// A PDF file the user names.
    Pdf,
    /// The system's default printer.
    Printer,
    /// A temporary PDF opened in the system viewer (Preview on macOS).
    Viewer,
}

/// What is printed.
#[derive(Clone, Debug, PartialEq)]
pub enum PrintTarget {
    /// The layout's printed pages, each `sheet_in` inches.
    Layout {
        pages: usize,
        sheet_in: (f64, f64),
        name: String,
    },
    /// One floor of the plan at a drawing scale.
    PlanView {
        floor: usize,
        layer_set: String,
        title: String,
    },
}

/// Print Source (plan views): the whole Drawing Sheet, or the part of the view
/// on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintSource {
    DrawingSheet,
    CurrentView,
}

/// The Drawing Scale choices.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum ScaleChoice {
    /// Fit to Paper, filling the global percentage of the paper.
    FitPercent,
    /// To Scale: the Drawing Sheet Setup's scale (a layout sheet at its size).
    ToScale,
    /// Check Plot at a fraction of the scale.
    CheckPlot,
    /// Fit to the whole paper.
    Fit,
    Actual,
    Percent,
    Drawing(Scale),
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum PaperChoice {
    Standard(SheetSize),
    Custom,
    /// "Match Print Source": the view's Drawing Sheet size and orientation.
    MatchSource,
}

/// The paper sources the Paper section offers, with the CUPS `InputSlot`
/// each asks for (`None`: the printer chooses).
pub const PAPER_SOURCES: [(&str, Option<&str>); 4] = [
    ("Automatic", None),
    ("Tray 1", Some("Tray1")),
    ("Tray 2", Some("Tray2")),
    ("Manual feed", Some("Manual")),
];

/// The DPIs the Destination section offers.
pub const DPI_CHOICES: [u32; 5] = [72, 150, 300, 600, 1200];

/// Everything the dialog remembers between uses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    dest: Destination,
    dpi: u32,
    paper: PaperChoice,
    custom_w: f64,
    custom_h: f64,
    landscape: bool,
    /// Index into [`PAPER_SOURCES`].
    paper_source: usize,
    scale: ScaleChoice,
    percent: f64,
    ratio: f64,
    /// Check Plot: the fraction of the scale.
    check_plot: f64,
    tiling: bool,
    overlap: f64,
    margin: f64,
    /// Plan views: the paper's margins are the sheet's Drawing Margins.
    use_sheet_margins: bool,
    color: PrintColor,
    line_weights: bool,
    /// Exact line weights: a sheet printed smaller or larger keeps each pen's
    /// own thickness.
    exact_weights: bool,
    copies: u32,
    collate: bool,
    /// The printer picked from the list; `None` is the system default.
    printer: Option<String>,
    /// Layout prints: render perspective boxes at this DPI (`0` = as each box says).
    persp_dpi: u32,
    /// Layout prints: ray-trace samples for perspective boxes (`0` = as each box says).
    persp_samples: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dest: Destination::Pdf,
            dpi: 300,
            paper: PaperChoice::Standard(SheetSize::Letter),
            custom_w: 13.0,
            custom_h: 19.0,
            landscape: true,
            paper_source: 0,
            scale: ScaleChoice::FitPercent,
            percent: 100.0,
            ratio: 48.0,
            check_plot: 0.5,
            tiling: false,
            overlap: 0.5,
            margin: 0.25,
            use_sheet_margins: true,
            color: PrintColor::Color,
            line_weights: true,
            exact_weights: false,
            copies: 1,
            collate: true,
            printer: None,
            persp_dpi: 0,
            persp_samples: 0,
        }
    }
}

// ------------------------------------------------------ remembered settings --

/// The print settings kept for every plan: one set per kind of view and the
/// Fit to Paper percentage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct PrintStore {
    /// Fit to Paper fills this percentage of the paper (`None`: 95).
    fit_percent: Option<f64>,
    views: BTreeMap<String, Settings>,
}

thread_local! {
    static STORE: RefCell<Option<PrintStore>> = const { RefCell::new(None) };
    /// What the last accepted Print dialog asked of the printer beyond the PDF.
    static EXTRAS: RefCell<DeliveryExtras> = RefCell::new(DeliveryExtras::default());
}

/// What the printer is asked besides the PDF: collating copies and a paper
/// source (CUPS `lp -o` options).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeliveryExtras {
    pub collate: bool,
    /// The CUPS `InputSlot` value; `None` leaves the choice to the printer.
    pub input_slot: Option<String>,
}

/// `~/.plan-studio/printsettings.json`; none under test, so a test run never
/// reads or writes the user's folder.
fn store_path() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(
        std::path::PathBuf::from(home)
            .join(".plan-studio")
            .join("printsettings.json"),
    )
}

fn with_store<R>(f: impl FnOnce(&mut PrintStore) -> R) -> R {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        let store = s.get_or_insert_with(|| {
            store_path()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default()
        });
        f(store)
    })
}

fn save_store() {
    let Some(path) = store_path() else { return };
    let Some(text) = STORE.with(|s| {
        s.borrow()
            .as_ref()
            .and_then(|st| serde_json::to_string_pretty(st).ok())
    }) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, text);
}

/// The Fit to Paper percentage, global to every view in every file.
pub fn fit_percent() -> f64 {
    with_store(|s| s.fit_percent).unwrap_or(DEFAULT_FIT_PERCENT)
}

/// Sets the Fit to Paper percentage (1 to 100).
pub fn set_fit_percent(p: f64) {
    with_store(|s| s.fit_percent = Some(p.clamp(1.0, 100.0)));
}

/// The settings last accepted for a kind of view.
fn recall(key: &str) -> Option<Settings> {
    with_store(|s| s.views.get(key).cloned())
}

/// Keeps the settings of an accepted print for the next one of that kind.
/// Copies are never kept.
fn remember(key: &str, s: &Settings) {
    let mut keep = s.clone();
    keep.copies = 1;
    with_store(|st| {
        st.views.insert(key.to_string(), keep);
    });
    save_store();
}

/// Forgets every remembered setting and the Fit to Paper percentage (tests;
/// a fresh start).
pub fn forget_remembered_settings() {
    STORE.with(|s| *s.borrow_mut() = Some(PrintStore::default()));
    save_store();
}

/// The extras the last accepted print asked of the printer.
pub fn delivery_extras() -> DeliveryExtras {
    EXTRAS.with(|e| e.borrow().clone())
}

// -------------------------------------------------------------- printers --

/// The printers the system knows (`lpstat -p`) and its default (`lpstat -d`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Printers {
    pub names: Vec<String>,
    pub default: Option<String>,
}

/// The printer names in the output of `lpstat -p`: lines of the form
/// `printer NAME is idle.  enabled since ...` (also `... disabled since`,
/// `... now printing ...`).
pub fn parse_lpstat_printers(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("printer") {
            continue;
        }
        if let Some(name) = words.next() {
            if !out.iter().any(|n| n == name) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// The default printer in the output of `lpstat -d`
/// (`system default destination: NAME`); `None` for "no system default destination".
pub fn parse_lpstat_default(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.trim()
            .strip_prefix("system default destination:")
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
    })
}

/// Asks CUPS for the printer list. Empty where there is no `lpstat` (Windows,
/// a machine without CUPS) and in tests.
pub fn list_printers() -> Printers {
    if cfg!(test) || cfg!(target_os = "windows") {
        return Printers::default();
    }
    let run = |arg: &str| {
        std::process::Command::new("lpstat")
            .arg(arg)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    Printers {
        names: parse_lpstat_printers(&run("-p")),
        default: parse_lpstat_default(&run("-d")),
    }
}

/// The printer drop-down: the system default, then every printer found.
fn printer_picker(ui: &mut Ui, current: &mut Option<String>, printers: &Printers) {
    let default_label = match &printers.default {
        Some(d) => format!("Default printer ({d})"),
        None => "Default printer".to_string(),
    };
    let shown = current.clone().unwrap_or_else(|| default_label.clone());
    egui::ComboBox::from_id_salt("print_printer")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(current, None, default_label);
            for n in &printers.names {
                ui.selectable_value(current, Some(n.clone()), n);
            }
        });
    if printers.names.is_empty() {
        ui.weak("No printers found");
    }
}

/// One paper in the Size list.
#[derive(Clone, Debug, PartialEq)]
enum PaperEntry {
    Standard(SheetSize),
    /// A custom sheet size (a name and its two sides, long first).
    Named(String, (f64, f64)),
}

impl PaperEntry {
    fn label(&self) -> String {
        match self {
            PaperEntry::Standard(z) => z.label().to_string(),
            PaperEntry::Named(n, _) => n.clone(),
        }
    }

    fn inches(&self) -> (f64, f64) {
        match self {
            PaperEntry::Standard(z) => z.inches(),
            PaperEntry::Named(_, i) => *i,
        }
    }
}

/// The Print dialog.
pub struct PrintDialog {
    target: PrintTarget,
    /// The kind of view: its remembered settings and its Drawing Sheet Setup.
    view: ViewType,
    /// The active view as the canvas last saw it.
    ctx: PrintViewContext,
    s: Settings,
    source: PrintSource,
    include_watermark: bool,
    /// The Fit to Paper percentage (global).
    fit_pct: f64,
    all: bool,
    /// Print Range: only the page that is open (when [`with_current_page`] told).
    ///
    /// [`with_current_page`]: Self::with_current_page
    current_only: bool,
    from: usize,
    to: usize,
    /// The printed page (1-based among the printed pages) that is open.
    current_page: Option<usize>,
    /// "Print Preview" was pressed: the owner shows the sheet at the chosen
    /// paper and scale, then clears this.
    pub preview_requested: bool,
    /// The printers, read when "System printer" is first chosen.
    printers: Option<Printers>,
    /// The layout's own sizes (Customize Sheet Sizes), offered as paper.
    custom_papers: Vec<(String, (f64, f64))>,
    /// The Check Plot fraction the paper was last matched to.
    matched_check: Option<f64>,
}

impl PrintDialog {
    fn with(target: PrintTarget, pages: usize) -> Self {
        let ctx = drawing_sheet::context();
        let layout = matches!(target, PrintTarget::Layout { .. });
        let view = if layout {
            ViewType::Layout
        } else {
            match ctx.view {
                ViewType::Layout | ViewType::MaterialsList => ViewType::Plan,
                v => v,
            }
        };
        let setup = if layout {
            &ctx.layout_sheet
        } else {
            &ctx.sheet
        };
        let remembered = if setup.remember_print_settings {
            recall(view.key())
        } else {
            None
        };
        let fresh = remembered.is_none();
        let mut s = remembered.unwrap_or_default();
        if layout && matches!(s.scale, ScaleChoice::Drawing(_) | ScaleChoice::Custom) {
            s.scale = ScaleChoice::FitPercent;
        }
        if fresh && ctx.synced {
            // The first print of this kind of view starts from its Drawing
            // Sheet Setup: its sheet, and its scale when the sheet is shown.
            s.paper = PaperChoice::MatchSource;
            s.scale = if ctx.sheet_shown && !layout {
                ScaleChoice::ToScale
            } else {
                ScaleChoice::FitPercent
            };
            if !layout {
                s.margin = setup.margins_in.into_iter().fold(0.0, f64::max);
            }
        }
        if !setup.remember_print_settings {
            if let Some(p) = &setup.printer {
                s.printer = Some(p.clone());
            }
        }
        let source = if ctx.sheet_shown {
            PrintSource::DrawingSheet
        } else {
            PrintSource::CurrentView
        };
        Self {
            target,
            view,
            include_watermark: ctx.watermark_on && ctx.watermark_ready,
            ctx,
            s,
            source,
            fit_pct: fit_percent(),
            all: true,
            current_only: false,
            from: 1,
            to: pages.max(1),
            current_page: None,
            preview_requested: false,
            printers: None,
            custom_papers: Vec::new(),
            matched_check: None,
        }
    }

    /// A dialog around given settings, for Print Model, which prints through
    /// the same options.
    fn bare(s: Settings) -> Self {
        Self {
            target: PrintTarget::PlanView {
                floor: 0,
                layer_set: String::new(),
                title: String::new(),
            },
            view: ViewType::Plan,
            ctx: PrintViewContext::default(),
            s,
            source: PrintSource::CurrentView,
            include_watermark: false,
            fit_pct: DEFAULT_FIT_PERCENT,
            all: true,
            current_only: false,
            from: 1,
            to: 1,
            current_page: None,
            preview_requested: false,
            printers: None,
            custom_papers: Vec::new(),
            matched_check: None,
        }
    }

    /// Offers the layout's custom sheet sizes (`name`, long and short side in
    /// inches) in the paper list besides the program-wide ones; picking one
    /// fills in a custom paper.
    pub fn with_custom_papers(mut self, papers: Vec<(String, (f64, f64))>) -> Self {
        self.custom_papers = papers;
        self
    }

    /// Tells the dialog which printed page of the layout is open (1-based),
    /// which Print Range's Current Sheet prints.
    pub fn with_current_page(mut self, ordinal: usize) -> Self {
        self.current_page = Some(ordinal.max(1));
        self
    }

    /// The printer picked for a system-printer print; `None` is the default.
    pub fn printer(&self) -> Option<&str> {
        self.s.printer.as_deref()
    }

    /// Layout prints: the DPI perspective boxes are rendered at (`0` = each box's own).
    pub fn perspective_dpi(&self) -> u32 {
        self.s.persp_dpi
    }

    /// Layout prints: ray-trace samples for perspective boxes (`0` = each box's own).
    pub fn perspective_samples(&self) -> u32 {
        self.s.persp_samples
    }

    /// Print the layout (`printed_pages` pages of `sheet_in` inches).
    pub fn for_layout(printed_pages: usize, sheet_in: (f64, f64), name: &str) -> Self {
        Self::with(
            PrintTarget::Layout {
                pages: printed_pages,
                sheet_in,
                name: name.to_string(),
            },
            printed_pages,
        )
    }

    /// Print floor `floor` of the plan.
    pub fn for_plan(floor: usize, layer_set: &str, title: &str) -> Self {
        Self::with(
            PrintTarget::PlanView {
                floor,
                layer_set: layer_set.to_string(),
                title: title.to_string(),
            },
            1,
        )
    }

    /// Layout pages only: a quick constructor for tests and the old call site.
    #[cfg(test)]
    pub fn new(printed_pages: usize) -> Self {
        Self::for_layout(printed_pages, (36.0, 24.0), "Layout")
    }

    pub fn target(&self) -> &PrintTarget {
        &self.target
    }

    pub fn destination(&self) -> Destination {
        self.s.dest
    }

    /// Printed pages `(from, to)` for a layout; `None` prints every page.
    pub fn range(&self) -> Option<(usize, usize)> {
        match (self.current_only, self.current_page) {
            (true, Some(n)) => Some((n, n)),
            _ => (!self.all).then_some((self.from, self.to)),
        }
    }

    pub fn copies(&self) -> u32 {
        self.s.copies.max(1)
    }

    /// Print Source (plan views).
    pub fn source(&self) -> PrintSource {
        self.source
    }

    /// Include Watermark.
    pub fn includes_watermark(&self) -> bool {
        self.include_watermark
    }

    /// Picks the colour mode as the radio buttons would (tests).
    #[cfg(test)]
    pub fn set_color(&mut self, color: PrintColor) {
        self.s.color = color;
    }

    fn is_layout(&self) -> bool {
        matches!(self.target, PrintTarget::Layout { .. })
    }

    /// The Drawing Sheet Setup the print starts from.
    fn setup(&self) -> &plan_core::drawing_sheet::ViewSheet {
        if self.is_layout() {
            &self.ctx.layout_sheet
        } else {
            &self.ctx.sheet
        }
    }

    /// The sheet Match Print Source stands for, as drawn `(width, height)`.
    fn match_sheet_in(&self) -> (f64, f64) {
        match &self.target {
            PrintTarget::Layout { sheet_in, .. } => *sheet_in,
            PrintTarget::PlanView { .. } => self.ctx.sheet.inches(),
        }
    }

    /// The paper and its orientation the options print on.
    fn paper_and_orientation(&self) -> (PaperSize, bool) {
        let s = &self.s;
        match s.paper {
            PaperChoice::Standard(z) => (PaperSize::Standard(z), s.landscape),
            PaperChoice::Custom => (
                PaperSize::Custom {
                    width_in: s.custom_w,
                    height_in: s.custom_h,
                },
                s.landscape,
            ),
            PaperChoice::MatchSource => {
                let (w, h) = self.match_sheet_in();
                let (long, short) = (w.max(h), w.min(h));
                let std = SheetSize::ALL.into_iter().find(|z| {
                    let (zl, zs) = z.inches();
                    (zl - long).abs() < 1e-6 && (zs - short).abs() < 1e-6
                });
                let paper = match std {
                    Some(z) => PaperSize::Standard(z),
                    None => PaperSize::Custom {
                        width_in: long,
                        height_in: short,
                    },
                };
                (paper, w >= h)
            }
        }
    }

    /// The part of the plan the print covers, plan inches; `None` prints the
    /// whole plan (and does when the canvas has not told the dialog where the
    /// sheet is).
    fn plan_window(&self) -> Option<[f64; 4]> {
        if self.is_layout() || !self.ctx.synced {
            return None;
        }
        match self.source {
            PrintSource::DrawingSheet => Some(self.ctx.sheet_window),
            PrintSource::CurrentView => self.ctx.view_window,
        }
    }

    /// Size in plan inches of what a plan print covers.
    fn plan_window_dims(&self) -> (f64, f64) {
        match self.plan_window() {
            Some(w) => ((w[2] - w[0]).abs(), (w[3] - w[1]).abs()),
            None => match self.ctx.plan_extent {
                // The plan's own margin is 24 inches on every side.
                Some((lo, hi)) => ((hi.x - lo.x).abs() + 48.0, (hi.y - lo.y).abs() + 48.0),
                None => (120.0, 120.0),
            },
        }
    }

    /// The largest drawing scale at which a plan print fits `fraction` of the
    /// printable area.
    fn fit_scale(&self, fraction: f64) -> Scale {
        let o = self.options();
        let (pw, ph) = o.printable_in();
        let (w, h) = self.plan_window_dims();
        let mut s = Scale::ThreeInch;
        loop {
            let k = s.inches_per_foot() / 12.0;
            if w * k <= pw * fraction + 1e-9 && (h * k + 0.3) <= ph * fraction + 1e-9 {
                return s;
            }
            match s.smaller_any() {
                Some(n) => s = n,
                None => return s,
            }
        }
    }

    /// The drawing scale a plan view prints at under the choices made.
    pub fn plan_scale(&self) -> Scale {
        match self.s.scale {
            ScaleChoice::ToScale | ScaleChoice::CheckPlot => scale_of(&self.ctx.sheet),
            ScaleChoice::Drawing(sc) => sc,
            ScaleChoice::Actual => Scale::Ratio(1),
            ScaleChoice::Percent => Scale::Ratio(
                (100.0 / self.s.percent.max(0.1))
                    .round()
                    .clamp(1.0, 10_000.0) as u32,
            ),
            ScaleChoice::Custom => Scale::Ratio(self.s.ratio.round().clamp(1.0, 10_000.0) as u32),
            ScaleChoice::FitPercent if self.s.tiling => Scale::QuarterInch,
            ScaleChoice::Fit if self.s.tiling => Scale::QuarterInch,
            ScaleChoice::FitPercent => self.fit_scale((self.fit_pct / 100.0).clamp(0.01, 1.0)),
            ScaleChoice::Fit => self.fit_scale(1.0),
        }
    }

    /// The sheet as it comes off at its own size, inches: a layout sheet, or
    /// the part of the plan at the drawing scale (with the note strip).
    fn sheet_in(&self) -> (f64, f64) {
        match &self.target {
            PrintTarget::Layout { sheet_in, .. } => *sheet_in,
            PrintTarget::PlanView { .. } => {
                let k = self.plan_scale().inches_per_foot() / 12.0;
                let (w, h) = self.plan_window_dims();
                (w * k, h * k + 0.3)
            }
        }
    }

    /// The print options the dialog describes.
    pub fn options(&self) -> PrintOptions {
        let s = &self.s;
        let layout = self.is_layout();
        let (paper, landscape) = self.paper_and_orientation();
        let setup = self.setup();
        let sheet_scale = scale_of(&self.ctx.sheet);
        let scale = match s.scale {
            ScaleChoice::FitPercent => PrintScale::FitPercent(self.fit_pct),
            ScaleChoice::ToScale if layout => PrintScale::Actual,
            ScaleChoice::ToScale => PrintScale::Drawing(sheet_scale),
            ScaleChoice::CheckPlot => PrintScale::CheckPlot {
                scale: sheet_scale,
                fraction: s.check_plot,
            },
            ScaleChoice::Fit => PrintScale::Fit,
            ScaleChoice::Actual => PrintScale::Actual,
            ScaleChoice::Percent => PrintScale::Percent(s.percent),
            ScaleChoice::Drawing(d) => PrintScale::Drawing(d),
            ScaleChoice::Custom => PrintScale::Ratio(s.ratio),
        };
        let sheet_margins = if layout { [0.0; 4] } else { setup.margins_in };
        PrintOptions {
            paper,
            landscape,
            scale,
            tiling: s.tiling,
            overlap_in: s.overlap,
            margin_in: s.margin,
            color: s.color,
            line_weights: s.line_weights,
            range: self.range(),
            copies: s.copies.max(1),
            collate: s.collate && s.copies > 1,
            edge_margins_in: (!layout && s.use_sheet_margins && self.ctx.synced)
                .then_some(setup.margins_in),
            sheet_margins_in: sheet_margins,
            plan_window: self.plan_window(),
            watermark: self.include_watermark && self.ctx.watermark_ready,
            pens: PenSetup {
                scale_factor: setup.weights.factor(),
                single_weight: setup.weights.single_weight,
                exact: s.exact_weights,
            },
            dpi: s.dpi,
        }
    }

    /// The options as the information messages read them: a plan view has
    /// been drawn at its scale already, so only a Check Plot shrinks it.
    fn info_options(&self) -> PrintOptions {
        let mut o = self.options();
        if !self.is_layout() && !matches!(o.scale, PrintScale::CheckPlot { .. }) {
            o.scale = PrintScale::Actual;
        }
        o
    }

    /// The tiles one sheet needs under the current options.
    pub fn tiles(&self) -> Option<TileGrid> {
        let o = self.info_options();
        let sheet = self.sheet_in();
        let (pw, ph) = o.printable_in();
        let scale = plan_layout::print_scale_factor(&o, sheet);
        Some(if o.tiling {
            tile_grid(sheet, scale, (pw, ph), o.overlap_in)
        } else {
            TileGrid {
                cols: 1,
                rows: 1,
                scale,
            }
        })
    }

    /// The information messages under the preview: sheet and paper sizes, the
    /// scale the print comes out at and what may go wrong.
    pub fn info(&self) -> Vec<String> {
        let mut out = plan_layout::print_info(self.sheet_in(), &self.info_options(), None);
        if self.include_watermark && !self.ctx.watermark_ready {
            out.push("The watermark has nothing to show: define it first.".to_string());
        }
        if self.s.dest == Destination::Printer && cfg!(target_os = "windows") {
            out.push("Printing to a system printer is not available on Windows yet.".to_string());
        }
        out
    }

    fn error(&self) -> Option<&'static str> {
        let s = &self.s;
        if let PrintTarget::Layout { pages, .. } = &self.target {
            if *pages == 0 {
                return Some("The layout has no pages to print");
            }
            if !self.all
                && !self.current_only
                && (self.from < 1 || self.to < self.from || self.to > *pages)
            {
                return Some("Enter a page range inside the layout");
            }
        }
        if s.paper == PaperChoice::Custom && (s.custom_w < 2.0 || s.custom_h < 2.0) {
            return Some("Custom paper must be at least 2 inches each way");
        }
        if s.margin < 0.0 || s.margin > 2.0 {
            return Some("Margins must be between 0 and 2 inches");
        }
        if s.tiling && s.overlap < 0.0 {
            return Some("The tile overlap cannot be negative");
        }
        if s.scale == ScaleChoice::Custom && s.ratio < 1.0 {
            return Some("The scale ratio must be at least 1:1");
        }
        if s.scale == ScaleChoice::Percent && s.percent < 1.0 {
            return Some("The print percentage must be at least 1%");
        }
        if !(1.0..=100.0).contains(&self.fit_pct) {
            return Some("Fit to Paper takes 1 to 100 percent of the paper");
        }
        if s.scale == ScaleChoice::CheckPlot && !(0.05..=1.0).contains(&s.check_plot) {
            return Some("A check plot is 5 to 100 percent of the scale");
        }
        None
    }

    /// One line on what will come out of the printer.
    pub fn summary(&self) -> String {
        match (&self.target, self.tiles()) {
            (PrintTarget::Layout { pages, .. }, Some(g)) => {
                let n = match self.range() {
                    Some((a, b)) => b + 1 - a,
                    None => *pages,
                };
                format!(
                    "{n} sheet(s) at {:.0}%: {} paper page(s){}",
                    g.scale * 100.0,
                    n * g.count(),
                    if g.count() > 1 {
                        format!(" ({} x {} tiles per sheet)", g.cols, g.rows)
                    } else {
                        String::new()
                    }
                )
            }
            (_, Some(g)) => {
                let scale = self.plan_scale();
                format!(
                    "The plan prints at {}{}: {} paper page(s)",
                    scale.label(),
                    if g.scale < 0.999 {
                        format!(" reduced to {:.0}%", g.scale * 100.0)
                    } else {
                        String::new()
                    },
                    g.count()
                )
            }
            _ => String::new(),
        }
    }

    /// The papers the Size list offers: the standard sizes Customize Sheet
    /// Sizes leaves in, then the program-wide and the layout's custom ones.
    fn paper_entries(&self) -> Vec<PaperEntry> {
        let keep = match self.s.paper {
            PaperChoice::Standard(z) => Some(z),
            _ => None,
        };
        let sizes = plan_layout::global_sheet_sizes();
        let mut out: Vec<PaperEntry> = SheetSize::ALL
            .into_iter()
            .filter(|z| !sizes.hidden.contains(z) || Some(*z) == keep)
            .map(PaperEntry::Standard)
            .collect();
        for (n, i) in plan_layout::global_custom_papers()
            .into_iter()
            .chain(self.custom_papers.iter().cloned())
        {
            if !out.iter().any(|e| e.label() == n) {
                out.push(PaperEntry::Named(n, i));
            }
        }
        out
    }

    /// Check Plot: the paper adjusts to the reduced sheet (the smallest that
    /// holds it with the margins). Returns whether a paper was found.
    fn match_check_plot_paper(&mut self) -> bool {
        let sheet = self.sheet_in();
        let entries = self.paper_entries();
        let papers: Vec<(String, (f64, f64))> =
            entries.iter().map(|e| (e.label(), e.inches())).collect();
        let o = self.options();
        let Some(i) =
            plan_layout::paper_for_check_plot(sheet, self.s.check_plot, o.margins(), &papers)
        else {
            return false;
        };
        self.s.landscape = sheet.0 >= sheet.1;
        match &entries[i] {
            PaperEntry::Standard(z) => self.s.paper = PaperChoice::Standard(*z),
            PaperEntry::Named(_, (long, short)) => {
                self.s.paper = PaperChoice::Custom;
                self.s.custom_w = *long;
                self.s.custom_h = *short;
            }
        }
        self.matched_check = Some(self.s.check_plot);
        true
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let title = match &self.target {
            PrintTarget::Layout { name, .. } => format!("Print: {name}"),
            PrintTarget::PlanView { title, .. } => format!("Print: {title}"),
        };
        if self.s.dest == Destination::Printer && self.printers.is_none() {
            self.printers = Some(list_printers());
        }
        // A Check Plot adjusts the paper whenever its fraction changes.
        if self.s.scale == ScaleChoice::CheckPlot && self.matched_check != Some(self.s.check_plot) {
            self.match_check_plot_paper();
        }
        if self.s.scale != ScaleChoice::CheckPlot {
            self.matched_check = None;
        }
        let summary = self.summary();
        let info = self.info();
        let this = &mut *self;
        let out = frame(ctx, &title, 460.0, error, |ui| {
            this.body(ui, &summary, &info)
        });
        if out == Outcome::Ok {
            self.accepted();
        }
        out
    }

    /// The dialog was accepted: keep the settings for the next print of this
    /// kind of view and what the printer is asked beyond the PDF.
    fn accepted(&self) {
        if self.setup().remember_print_settings {
            remember(self.view.key(), &self.s);
        }
        set_fit_percent(self.fit_pct);
        save_store();
        let slot = PAPER_SOURCES
            .get(self.s.paper_source)
            .and_then(|p| p.1)
            .map(str::to_string);
        EXTRAS.with(|e| {
            *e.borrow_mut() = DeliveryExtras {
                collate: self.s.collate && self.s.copies > 1,
                input_slot: slot,
            }
        });
    }

    fn body(&mut self, ui: &mut Ui, summary: &str, info: &[String]) {
        let layout = self.is_layout();
        let pages = match &self.target {
            PrintTarget::Layout { pages, .. } => *pages,
            PrintTarget::PlanView { .. } => 1,
        };

        section(ui, "Destination");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.s.dest, Destination::Pdf, "PDF file");
            ui.radio_value(&mut self.s.dest, Destination::Printer, "System printer");
            ui.radio_value(&mut self.s.dest, Destination::Viewer, "Open in viewer");
        });
        if self.s.dest == Destination::Printer {
            row(ui, "Printer", |ui| {
                let list = self.printers.clone().unwrap_or_default();
                printer_picker(ui, &mut self.s.printer, &list);
                if ui.small_button("Refresh").clicked() {
                    self.printers = Some(list_printers());
                }
            });
        }
        row(ui, "DPI", |ui| {
            egui::ComboBox::from_id_salt("print_dpi")
                .selected_text(format!("{}", self.s.dpi))
                .width(80.0)
                .show_ui(ui, |ui| {
                    for d in DPI_CHOICES {
                        ui.selectable_value(&mut self.s.dpi, d, format!("{d}"));
                    }
                });
        });

        section(ui, "Paper");
        let entries = self.paper_entries();
        row(ui, "Size", |ui| self.paper_combo(ui, &entries));
        if self.s.paper == PaperChoice::Custom {
            row(ui, "Width / Height", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.s.custom_w)
                        .suffix("\"")
                        .speed(0.1),
                );
                ui.add(
                    egui::DragValue::new(&mut self.s.custom_h)
                        .suffix("\"")
                        .speed(0.1),
                );
            });
        }
        if self.s.paper != PaperChoice::MatchSource {
            row(ui, "Orientation", |ui| {
                ui.radio_value(&mut self.s.landscape, true, "Landscape");
                ui.radio_value(&mut self.s.landscape, false, "Portrait");
            });
        } else {
            let (paper, landscape) = self.paper_and_orientation();
            ui.weak(format!(
                "Match Print Source: {} {}",
                paper.label(),
                if landscape { "landscape" } else { "portrait" }
            ));
        }
        row(ui, "Source", |ui| {
            let shown = PAPER_SOURCES
                .get(self.s.paper_source)
                .map_or("Automatic", |p| p.0);
            egui::ComboBox::from_id_salt("print_paper_source")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (i, (label, _)) in PAPER_SOURCES.iter().enumerate() {
                        ui.selectable_value(&mut self.s.paper_source, i, *label);
                    }
                });
        });
        if !layout {
            ui.checkbox(
                &mut self.s.use_sheet_margins,
                "Margins are the Drawing Margins of the sheet",
            );
        }
        if layout || !self.s.use_sheet_margins {
            row(ui, "Margin", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.s.margin)
                        .suffix("\"")
                        .speed(0.05)
                        .max_decimals(2),
                );
            });
        }

        if layout {
            section(ui, "Print Range");
            if ui
                .radio_value(&mut self.all, true, format!("All {pages} page(s)"))
                .clicked()
            {
                self.current_only = false;
            }
            if self.current_page.is_some() {
                let mut cur = self.current_only;
                if ui.radio_value(&mut cur, true, "Current Sheet").clicked() {
                    self.current_only = true;
                    self.all = false;
                }
            }
            ui.horizontal(|ui| {
                if ui.radio_value(&mut self.all, false, "Sheets").clicked() {
                    self.current_only = false;
                }
                let on = !self.all && !self.current_only;
                ui.add_enabled(
                    on,
                    egui::DragValue::new(&mut self.from).range(1..=pages.max(1)),
                );
                ui.label("to");
                ui.add_enabled(
                    on,
                    egui::DragValue::new(&mut self.to).range(1..=pages.max(1)),
                );
            });
        } else {
            section(ui, "Print Source");
            ui.radio_value(
                &mut self.source,
                PrintSource::DrawingSheet,
                "Drawing Sheet: the whole sheet, even zoomed in",
            );
            ui.radio_value(
                &mut self.source,
                PrintSource::CurrentView,
                "Current View: what is on screen",
            );
        }

        section(ui, "Drawing Scale");
        self.scale_section(ui, layout);
        row(ui, "Tiling", |ui| {
            ui.checkbox(&mut self.s.tiling, "Print across several pages");
        });
        if self.s.tiling {
            row(ui, "Overlap", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.s.overlap)
                        .suffix("\"")
                        .speed(0.05)
                        .max_decimals(2),
                );
            });
        }

        section(ui, "Options");
        row(ui, "Copies", |ui| {
            ui.add(egui::DragValue::new(&mut self.s.copies).range(1..=99));
            ui.add_enabled(
                self.s.copies >= 2,
                egui::Checkbox::new(&mut self.s.collate, "Collate"),
            );
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.include_watermark, "Include Watermark");
            if ui.small_button("Define\u{2026}").clicked() {
                super::watermark::request_open();
            }
        });
        row(ui, "Print in Color", |ui| {
            ui.radio_value(&mut self.s.color, PrintColor::Color, "Color");
            ui.radio_value(&mut self.s.color, PrintColor::Grayscale, "Grayscale");
            ui.radio_value(&mut self.s.color, PrintColor::BlackWhite, "Black and white");
        });
        ui.checkbox(&mut self.s.line_weights, "Print line weights");
        ui.add_enabled(
            self.s.line_weights,
            egui::Checkbox::new(&mut self.s.exact_weights, "Exact line weights"),
        )
        .on_hover_text("A sheet printed smaller or larger keeps each pen's own thickness");
        if layout {
            row(ui, "Perspective views", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.s.persp_dpi)
                        .range(0..=600)
                        .speed(1.0)
                        .custom_formatter(|v, _| {
                            if v < 1.0 {
                                "as set".to_string()
                            } else {
                                format!("{v:.0} dpi")
                            }
                        }),
                )
                .on_hover_text("Render perspective boxes at this DPI (0 keeps each box's own)");
                ui.add(
                    egui::DragValue::new(&mut self.s.persp_samples)
                        .range(0..=256)
                        .custom_formatter(|v, _| {
                            if v < 1.0 {
                                "as set".to_string()
                            } else {
                                format!("{v:.0} samples")
                            }
                        }),
                );
            });
        }

        section(ui, "Advanced Options");
        if ui
            .button("Open System Print Dialog")
            .on_hover_text(
                "Opens the PDF in the viewer, where its own Print command is the system dialog",
            )
            .clicked()
        {
            self.s.dest = Destination::Viewer;
        }

        section(ui, "Preview");
        ui.weak(summary);
        if ui
            .button("Print Preview")
            .on_hover_text("Show the pages as they will print")
            .clicked()
        {
            self.preview_requested = true;
        }
        if !info.is_empty() {
            section(ui, "Information");
            for m in info {
                ui.weak(m);
            }
        }
    }

    /// Drawing Scale: Fit to Paper, To Scale, Check Plot, and the older
    /// choices under More scales.
    fn scale_section(&mut self, ui: &mut Ui, layout: bool) {
        let to_scale_label = if layout {
            "To Scale: each sheet at its own size".to_string()
        } else {
            format!("To Scale: {}", scale_of(&self.ctx.sheet).label())
        };
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.s.scale, ScaleChoice::FitPercent, "Fit to Paper");
            ui.add_enabled(
                self.s.scale == ScaleChoice::FitPercent,
                egui::DragValue::new(&mut self.fit_pct)
                    .range(1.0..=100.0)
                    .speed(0.5)
                    .max_decimals(0)
                    .suffix("%"),
            )
            .on_hover_text("Of the paper; global to every view in every file");
        });
        ui.radio_value(&mut self.s.scale, ScaleChoice::ToScale, to_scale_label);
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.s.scale, ScaleChoice::CheckPlot, "Check Plot at");
            ui.add_enabled_ui(self.s.scale == ScaleChoice::CheckPlot, |ui| {
                egui::ComboBox::from_id_salt("print_check_plot")
                    .selected_text(plan_layout::check_plot_label(self.s.check_plot))
                    .width(70.0)
                    .show_ui(ui, |ui| {
                        for (f, label) in CHECK_PLOT_FRACTIONS {
                            ui.selectable_value(&mut self.s.check_plot, f, label);
                        }
                    });
            });
        });
        let other = !matches!(
            self.s.scale,
            ScaleChoice::FitPercent | ScaleChoice::ToScale | ScaleChoice::CheckPlot
        );
        ui.horizontal(|ui| {
            if ui.radio(other, "Other").clicked() && !other {
                self.s.scale = ScaleChoice::Fit;
            }
            ui.add_enabled_ui(other, |ui| scale_combo(ui, &mut self.s.scale, layout));
        });
        match self.s.scale {
            ScaleChoice::Percent => {
                row(ui, "Percent", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.s.percent)
                            .range(1.0..=800.0)
                            .suffix("%"),
                    );
                });
            }
            ScaleChoice::Custom => {
                row(ui, "Ratio 1 :", |ui| {
                    ui.add(egui::DragValue::new(&mut self.s.ratio).range(1.0..=10_000.0));
                });
            }
            _ => {}
        }
    }

    fn paper_combo(&mut self, ui: &mut Ui, entries: &[PaperEntry]) {
        let shown = match self.s.paper {
            PaperChoice::Standard(z) => z.label().to_string(),
            PaperChoice::MatchSource => "Match Print Source".to_string(),
            PaperChoice::Custom => entries
                .iter()
                .find(|e| {
                    let (a, b) = e.inches();
                    (a.max(b) - self.s.custom_w.max(self.s.custom_h)).abs() < 1e-6
                        && (a.min(b) - self.s.custom_w.min(self.s.custom_h)).abs() < 1e-6
                })
                .map_or_else(|| "Custom".to_string(), PaperEntry::label),
        };
        egui::ComboBox::from_id_salt("print_paper")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.s.paper,
                    PaperChoice::MatchSource,
                    "Match Print Source",
                );
                for e in entries {
                    match e {
                        PaperEntry::Standard(z) => {
                            ui.selectable_value(
                                &mut self.s.paper,
                                PaperChoice::Standard(*z),
                                z.label(),
                            );
                        }
                        PaperEntry::Named(name, (a, b)) => {
                            if ui.selectable_label(false, name).clicked() {
                                self.s.paper = PaperChoice::Custom;
                                self.s.custom_w = a.max(*b);
                                self.s.custom_h = a.min(*b);
                            }
                        }
                    }
                }
                ui.selectable_value(&mut self.s.paper, PaperChoice::Custom, "Custom size");
            });
    }
}

fn scale_combo(ui: &mut Ui, current: &mut ScaleChoice, layout: bool) {
    let shown = match current {
        ScaleChoice::Fit => "Fit to the whole paper".to_string(),
        ScaleChoice::Actual if layout => "100% (actual size)".to_string(),
        ScaleChoice::Actual => "1:1 (full size)".to_string(),
        ScaleChoice::Percent => "Percentage".to_string(),
        ScaleChoice::Drawing(s) => s.label().to_string(),
        ScaleChoice::Custom => "Custom ratio".to_string(),
        _ => "Choose\u{2026}".to_string(),
    };
    egui::ComboBox::from_id_salt("print_scale")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(current, ScaleChoice::Fit, "Fit to the whole paper");
            if layout {
                ui.selectable_value(current, ScaleChoice::Actual, "100% (actual size)");
                ui.selectable_value(current, ScaleChoice::Percent, "Percentage");
            } else {
                ui.selectable_value(current, ScaleChoice::Actual, "1:1 (full size)");
                for s in Scale::choices() {
                    ui.selectable_value(current, ScaleChoice::Drawing(s), s.label());
                }
                ui.selectable_value(current, ScaleChoice::Custom, "Custom ratio");
            }
        });
}

// ----------------------------------------------------------- Print Preview --

/// Print Preview (L-19): the pages as they will print, drawn from the same
/// primitives the PDF is made of, in the chosen colour mode, with the pen
/// weights (or hairlines) of the print, the sheet scaled and placed on the
/// paper, hatches at their scale and pictures in place.
pub struct PrintPreviewDialog {
    title: String,
    note: String,
    pages: Vec<plan_layout::PreviewPage>,
    index: usize,
    /// Screen pixels per paper inch; `None` fits the page in the window.
    zoom: Option<f32>,
    textures: std::collections::HashMap<(usize, usize), egui::TextureHandle>,
}

impl PrintPreviewDialog {
    pub fn new(title: &str, note: &str, pages: Vec<plan_layout::PreviewPage>) -> Self {
        Self {
            title: title.to_string(),
            note: note.to_string(),
            pages,
            index: 0,
            zoom: None,
            textures: std::collections::HashMap::new(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn pages(&self) -> &[plan_layout::PreviewPage] {
        &self.pages
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Shows page `i` (clamped).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn go_to(&mut self, i: usize) {
        self.index = i.min(self.pages.len().saturating_sub(1));
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut open = true;
        let mut close = false;
        let n = self.pages.len();
        egui::Window::new(format!("Print Preview: {}", self.title))
            .id(egui::Id::new("print_preview_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([820.0, 640.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.index > 0, egui::Button::new("Previous"))
                        .clicked()
                    {
                        self.index -= 1;
                    }
                    ui.label(format!("Page {} of {}", self.index + 1, n.max(1)));
                    if ui
                        .add_enabled(self.index + 1 < n, egui::Button::new("Next"))
                        .clicked()
                    {
                        self.index += 1;
                    }
                    ui.separator();
                    if ui.button("Fit").clicked() {
                        self.zoom = None;
                    }
                    if ui.button("+").clicked() {
                        self.zoom = Some(self.zoom.unwrap_or(40.0) * 1.25);
                    }
                    if ui.button("-").clicked() {
                        self.zoom = Some((self.zoom.unwrap_or(40.0) / 1.25).max(4.0));
                    }
                    ui.separator();
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
                let Some(page) = self.pages.get(self.index) else {
                    ui.label("Nothing to print.");
                    return;
                };
                ui.weak(format!(
                    "{}   {}   {:.0}% on {:.1} x {:.1} in paper, {}",
                    page.caption,
                    self.note,
                    page.scale * 100.0,
                    page.paper_in.0,
                    page.paper_in.1,
                    color_note(page.color)
                ));
                let avail = ui.available_size();
                let fit = (avail.x / page.paper_in.0 as f32)
                    .min(avail.y / page.paper_in.1 as f32)
                    .max(4.0);
                let ppi = self.zoom.unwrap_or(fit);
                let size = egui::vec2(page.paper_in.0 as f32 * ppi, page.paper_in.1 as f32 * ppi);
                egui::ScrollArea::both().show(ui, |ui| {
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    paint_preview(ui, rect, ppi, page, self.index, &mut self.textures);
                });
            });
        if close || !open {
            Outcome::Cancel
        } else {
            Outcome::Open
        }
    }
}

fn color_note(c: PrintColor) -> &'static str {
    match c {
        PrintColor::Color => "in colour",
        PrintColor::Grayscale => "in grayscale",
        PrintColor::BlackWhite => "in black and white",
    }
}

fn px_color(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// Paints a preview page into `rect` (the whole paper) at `ppi` screen pixels
/// per paper inch.
fn paint_preview(
    ui: &mut Ui,
    rect: egui::Rect,
    ppi: f32,
    page: &plan_layout::PreviewPage,
    page_index: usize,
    textures: &mut std::collections::HashMap<(usize, usize), egui::TextureHandle>,
) {
    use plan_layout::PreviewItem as I;
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(150)),
        egui::StrokeKind::Inside,
    );
    let k = ppi / 72.0;
    let h_pt = page.paper_in.1 as f32 * 72.0;
    let at = |x: f64, y: f64| {
        egui::pos2(
            rect.min.x + x as f32 * k,
            rect.min.y + (h_pt - y as f32) * k,
        )
    };
    let rect_of = |r: &[f64; 4]| egui::Rect::from_two_pos(at(r[0], r[1]), at(r[2], r[3]));
    // The printable area, faint, so the margin shows.
    let pa = rect_of(&page.printable_pt);
    painter.rect_stroke(
        pa,
        0.0,
        egui::Stroke::new(0.5_f32, egui::Color32::from_gray(210)),
        egui::StrokeKind::Inside,
    );
    let mut clips: Vec<egui::Rect> = vec![rect];
    for (idx, item) in page.items.iter().enumerate() {
        let clip = *clips.last().unwrap_or(&rect);
        let p = painter.with_clip_rect(clip);
        match item {
            I::ClipBegin(r) => clips.push(clip.intersect(rect_of(r))),
            I::ClipEnd => {
                if clips.len() > 1 {
                    clips.pop();
                }
            }
            I::Fill { pts, color } => {
                if pts.len() >= 3 {
                    let pts: Vec<egui::Pos2> = pts.iter().map(|q| at(q.0, q.1)).collect();
                    p.add(egui::Shape::convex_polygon(
                        pts,
                        px_color(*color),
                        egui::Stroke::NONE,
                    ));
                }
            }
            I::Stroke {
                pts,
                closed,
                width_pt,
                color,
                dash,
            } => {
                let mut v: Vec<egui::Pos2> = pts.iter().map(|q| at(q.0, q.1)).collect();
                if *closed && v.len() > 2 {
                    v.push(v[0]);
                }
                let stroke = egui::Stroke::new((*width_pt as f32 * k).max(0.4), px_color(*color));
                if dash.len() >= 2 && dash[0] > 0.0 {
                    let on = (dash[0] as f32 * k).max(1.0);
                    let off = (dash[1] as f32 * k).max(1.0);
                    p.extend(egui::Shape::dashed_line(&v, stroke, on, off));
                } else {
                    p.add(egui::Shape::line(v, stroke));
                }
            }
            I::Text {
                x,
                y,
                size_pt,
                color,
                bold,
                angle,
                text,
            } => {
                let size = (*size_pt as f32 * k).max(1.0);
                if size < 2.0 {
                    continue;
                }
                let font = egui::FontId::proportional(size);
                let c = px_color(*color);
                let galley = p.layout_no_wrap(text.clone(), font, c);
                let base = at(*x, *y);
                let ascent = size * 0.8;
                let phi = -(*angle as f32);
                let off = egui::vec2(-ascent * phi.sin(), ascent * phi.cos());
                let mut shape = egui::epaint::TextShape::new(base - off, galley, c).with_angle(phi);
                if *bold {
                    // A second pass, a hair to the right, reads as bold.
                    let mut again = shape.clone();
                    again.pos += egui::vec2(size * 0.03, 0.0);
                    p.add(egui::Shape::Text(again));
                }
                shape.override_text_color = Some(c);
                p.add(egui::Shape::Text(shape));
            }
            I::Mark(m) => {
                // A watermark mark: centred on (cx, cy), turned about it.
                let c = at(m.cx, m.cy);
                let phi = -(m.angle as f32);
                let (sp, cp) = phi.sin_cos();
                let a = (m.alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
                let turn = |x: f32, y: f32| c + egui::vec2(x * cp - y * sp, x * sp + y * cp);
                match &m.kind {
                    plan_layout::PreviewMarkKind::Text { text, size_pt } => {
                        let size = (*size_pt as f32 * k).max(1.0);
                        if size < 2.0 {
                            continue;
                        }
                        let col = egui::Color32::from_rgba_unmultiplied(
                            m.color[0], m.color[1], m.color[2], a,
                        );
                        let galley =
                            p.layout_no_wrap(text.clone(), egui::FontId::proportional(size), col);
                        let half = galley.size() * 0.5;
                        let pos = turn(-half.x, -half.y);
                        let mut shape =
                            egui::epaint::TextShape::new(pos, galley, col).with_angle(phi);
                        shape.override_text_color = Some(col);
                        p.add(egui::Shape::Text(shape));
                    }
                    plan_layout::PreviewMarkKind::Image { px, rgba } => {
                        let tex = textures.entry((page_index, idx)).or_insert_with(|| {
                            let img = egui::ColorImage::from_rgba_unmultiplied(
                                [px.0 as usize, px.1 as usize],
                                rgba,
                            );
                            ui.ctx().load_texture(
                                format!("print_preview_mark_{page_index}_{idx}"),
                                img,
                                egui::TextureOptions::LINEAR,
                            )
                        });
                        let (hw, hh) = (m.w as f32 * k * 0.5, m.h as f32 * k * 0.5);
                        let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
                        let uv = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
                        let mut mesh = egui::Mesh::with_texture(tex.id());
                        for ((x, y), (u, v)) in corners.into_iter().zip(uv) {
                            mesh.vertices.push(egui::epaint::Vertex {
                                pos: turn(x, y),
                                uv: egui::pos2(u, v),
                                color: egui::Color32::from_white_alpha(a),
                            });
                        }
                        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
                        p.add(egui::Shape::mesh(mesh));
                    }
                }
            }
            I::Image { rect: r, px, rgba } => {
                let tex = textures.entry((page_index, idx)).or_insert_with(|| {
                    let img = egui::ColorImage::from_rgba_unmultiplied(
                        [px.0 as usize, px.1 as usize],
                        rgba,
                    );
                    ui.ctx().load_texture(
                        format!("print_preview_{page_index}_{idx}"),
                        img,
                        egui::TextureOptions::LINEAR,
                    )
                });
                p.image(
                    tex.id(),
                    rect_of(r),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
    }
}

// ----------------------------------------------------------- Print Image --

/// Print Image (manual p. 1443): the current plan view as pixels, not
/// vectors. Destination, DPI and Paper decide how many pixels: the picture can
/// be sized to the printable width of a paper at the DPI, or given a width.
pub struct ImageDialog {
    pub width_px: u32,
    /// `None` fits the plan to the image; `Some` draws it at that scale.
    pub scale: Option<Scale>,
    pub floor: usize,
    pub layer_set: String,
    /// Dots per inch of the print.
    pub dpi: u32,
    /// Size the picture to the printable width of `paper` at the DPI.
    pub fit_paper: bool,
    paper: SheetSize,
    landscape: bool,
}

/// The margin Print Image leaves on the paper, inches.
const IMAGE_MARGIN_IN: f64 = 0.25;

impl ImageDialog {
    pub fn new(floor: usize, layer_set: &str) -> Self {
        Self {
            width_px: 2048,
            scale: None,
            floor,
            layer_set: layer_set.to_string(),
            dpi: 300,
            fit_paper: false,
            paper: SheetSize::Letter,
            landscape: true,
        }
    }

    /// The pixel width the paper's printable width comes to at the DPI.
    pub fn paper_width_px(&self) -> u32 {
        let (w, h) = self.paper.inches();
        let across = if self.landscape { w } else { h };
        (((across - 2.0 * IMAGE_MARGIN_IN) * f64::from(self.dpi)).round() as u32).max(16)
    }

    /// The information messages: what the picture is on paper and what may
    /// go wrong.
    pub fn info(&self) -> Vec<String> {
        let w = self.effective_width();
        let inches = f64::from(w) / f64::from(self.dpi.max(1));
        let mut out = vec![format!(
            "The picture is {w} pixels wide: {inches:.1} inches at {} dpi.",
            self.dpi
        )];
        if self.dpi < 150 {
            out.push("Below 150 dpi lines look coarse on paper.".to_string());
        }
        if w > 8000 {
            out.push("The most the picture can be is 8000 pixels wide.".to_string());
        }
        out
    }

    fn effective_width(&self) -> u32 {
        if self.fit_paper {
            self.paper_width_px()
        } else {
            self.width_px
        }
    }

    /// Settles the width the paper and DPI ask for (OK takes `width_px`).
    pub fn settle(&mut self) {
        if self.fit_paper {
            self.width_px = self.paper_width_px().clamp(256, 8000);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.settle();
        let w = self.effective_width();
        let error =
            (!(256..=8000).contains(&w)).then_some("The image must be 256 to 8000 pixels wide");
        let info = self.info();
        let Self {
            width_px,
            scale,
            dpi,
            fit_paper,
            paper,
            landscape,
            ..
        } = self;
        let out = frame(ctx, "Print Image", 380.0, error, |ui| {
            row(ui, "DPI", |ui| {
                egui::ComboBox::from_id_salt("print_image_dpi")
                    .selected_text(format!("{dpi}"))
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for d in DPI_CHOICES {
                            ui.selectable_value(dpi, d, format!("{d}"));
                        }
                    });
            });
            ui.checkbox(fit_paper, "Size the picture to the paper");
            if *fit_paper {
                row(ui, "Paper", |ui| {
                    egui::ComboBox::from_id_salt("print_image_paper")
                        .selected_text(paper.label())
                        .show_ui(ui, |ui| {
                            for z in SheetSize::ALL {
                                ui.selectable_value(paper, z, z.label());
                            }
                        });
                });
                row(ui, "Orientation", |ui| {
                    ui.radio_value(landscape, true, "Landscape");
                    ui.radio_value(landscape, false, "Portrait");
                });
            } else {
                row(ui, "Width", |ui| {
                    ui.add(
                        egui::DragValue::new(width_px)
                            .range(64..=9000)
                            .suffix(" px"),
                    );
                });
            }
            row(ui, "Scale", |ui| {
                let shown = scale.map_or("Fit the plan".to_string(), |s| s.label().to_string());
                egui::ComboBox::from_id_salt("print_image_scale")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(scale, None, "Fit the plan");
                        for s in Scale::choices() {
                            ui.selectable_value(scale, Some(s), s.label());
                        }
                    });
            });
            ui.weak(
                "Saves the floor plan's lines as a PNG. For a rendering, use Ray Trace > Save PNG.",
            );
            section(ui, "Information");
            for m in &info {
                ui.weak(m);
            }
        });
        self.settle();
        out
    }
}

// ------------------------------------------------- Print Image of the 3D view --

/// Print Image of the 3D view: the view as it is, saved as a PNG at a chosen
/// size. The picture is ray traced from the viewport's camera (the 3D view
/// has no offscreen target to read back), so it shows the default sun and sky.
pub struct Image3dDialog {
    pub width_px: u32,
    pub height_px: u32,
    /// Ray-trace samples per pixel.
    pub samples: u32,
}

/// Largest side of a 3D Print Image, pixels.
pub const IMAGE_3D_MAX_PX: u32 = 4096;

impl Image3dDialog {
    /// Starts at the view's own aspect ratio, 1600 pixels wide.
    pub fn new(aspect: f32) -> Self {
        let width_px = 1600;
        let height_px =
            ((width_px as f32 / aspect.max(0.1)).round() as u32).clamp(64, IMAGE_3D_MAX_PX);
        Self {
            width_px,
            height_px,
            samples: 24,
        }
    }

    fn error(&self) -> Option<&'static str> {
        let ok = |v: u32| (64..=IMAGE_3D_MAX_PX).contains(&v);
        (!(ok(self.width_px) && ok(self.height_px)))
            .then_some("The image must be 64 to 4096 pixels each way")
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let Self {
            width_px,
            height_px,
            samples,
        } = self;
        frame(ctx, "Print Image: 3D View", 380.0, error, |ui| {
            row(ui, "Width / Height", |ui| {
                ui.add(
                    egui::DragValue::new(width_px)
                        .range(64..=9000)
                        .suffix(" px"),
                );
                ui.add(
                    egui::DragValue::new(height_px)
                        .range(64..=9000)
                        .suffix(" px"),
                );
            });
            row(ui, "Quality", |ui| {
                ui.add(
                    egui::DragValue::new(samples)
                        .range(1..=512)
                        .suffix(" samples"),
                );
            });
            ui.weak("Ray traced from the 3D view's camera with the default sun and sky.");
        })
    }
}

// ------------------------------------------------------------ Print Model --

/// Print Model: a perspective camera rendered at a chosen DPI onto the paper.
pub struct ModelDialog {
    pub camera: plan_core::Id,
    cameras: Vec<(plan_core::Id, String)>,
    pub dpi: u32,
    pub samples: u32,
    s: Settings,
    printers: Option<Printers>,
}

/// Highest DPI Print Model offers.
pub const MODEL_MAX_DPI: u32 = 600;

impl ModelDialog {
    /// `cameras` are the project's perspective cameras (id and name); `current`
    /// is the one the 3D view shows, if any.
    pub fn new(cameras: Vec<(plan_core::Id, String)>, current: Option<plan_core::Id>) -> Self {
        let s = recall("model").unwrap_or_default();
        let camera = current
            .filter(|c| cameras.iter().any(|(id, _)| id == c))
            .or_else(|| cameras.first().map(|(id, _)| *id))
            .unwrap_or_default();
        Self {
            camera,
            cameras,
            dpi: 150,
            samples: 16,
            s,
            printers: None,
        }
    }

    pub fn destination(&self) -> Destination {
        self.s.dest
    }

    pub fn copies(&self) -> u32 {
        self.s.copies.max(1)
    }

    pub fn printer(&self) -> Option<&str> {
        self.s.printer.as_deref()
    }

    /// The camera's name.
    pub fn camera_name(&self) -> String {
        self.cameras
            .iter()
            .find(|(id, _)| *id == self.camera)
            .map_or_else(|| "Perspective".to_string(), |(_, n)| n.clone())
    }

    /// The paper and colour options (always printed at actual size).
    pub fn options(&self) -> PrintOptions {
        PrintOptions {
            scale: PrintScale::Actual,
            range: None,
            tiling: false,
            ..PrintDialog::bare(self.s.clone()).options()
        }
    }

    fn error(&self) -> Option<&'static str> {
        if self.cameras.is_empty() {
            return Some("The plan has no perspective camera");
        }
        if !(20..=MODEL_MAX_DPI).contains(&self.dpi) {
            return Some("The DPI must be between 20 and 600");
        }
        if self.s.paper == PaperChoice::Custom && (self.s.custom_w < 2.0 || self.s.custom_h < 2.0) {
            return Some("Custom paper must be at least 2 inches each way");
        }
        None
    }

    /// The render size on the printable area, `(width, height)` pixels.
    pub fn pixels(&self) -> (u32, u32) {
        let (pw, ph) = self.options().printable_in();
        plan_layout::perspective_pixels(pw, (ph - 0.3).max(1.0), self.dpi)
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        if self.s.dest == Destination::Printer && self.printers.is_none() {
            self.printers = Some(list_printers());
        }
        let error = self.error();
        let (px_w, px_h) = self.pixels();
        let Self {
            camera,
            cameras,
            dpi,
            samples,
            s,
            printers,
        } = self;
        let out = frame(ctx, "Print Model", 400.0, error, |ui| {
            row(ui, "Camera", |ui| {
                let shown = cameras
                    .iter()
                    .find(|(id, _)| id == camera)
                    .map_or_else(String::new, |(_, n)| n.clone());
                egui::ComboBox::from_id_salt("model_camera")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for (id, name) in cameras.iter() {
                            ui.selectable_value(camera, *id, name);
                        }
                    });
            });
            row(ui, "Resolution", |ui| {
                ui.add(
                    egui::DragValue::new(dpi)
                        .range(20..=MODEL_MAX_DPI)
                        .suffix(" dpi"),
                );
            });
            row(ui, "Quality", |ui| {
                ui.add(
                    egui::DragValue::new(samples)
                        .range(1..=512)
                        .suffix(" samples"),
                );
            });
            section(ui, "Paper");
            row(ui, "Size", |ui| simple_paper_combo(ui, s));
            row(ui, "Orientation", |ui| {
                ui.radio_value(&mut s.landscape, true, "Landscape");
                ui.radio_value(&mut s.landscape, false, "Portrait");
            });
            row(ui, "Margin", |ui| {
                ui.add(egui::DragValue::new(&mut s.margin).suffix("\"").speed(0.05));
            });
            section(ui, "Destination");
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.dest, Destination::Pdf, "PDF file");
                ui.radio_value(&mut s.dest, Destination::Printer, "System printer");
                ui.radio_value(&mut s.dest, Destination::Viewer, "Open in viewer");
            });
            if s.dest == Destination::Printer {
                row(ui, "Printer", |ui| {
                    let list = printers.clone().unwrap_or_default();
                    printer_picker(ui, &mut s.printer, &list);
                });
            }
            ui.add_space(6.0);
            ui.weak(format!("Renders {px_w} x {px_h} pixels."));
        });
        if out == Outcome::Ok {
            remember("model", &self.s);
        }
        out
    }
}

/// The paper list of Print Model: the standard sizes and a custom one.
fn simple_paper_combo(ui: &mut Ui, s: &mut Settings) {
    let shown = match s.paper {
        PaperChoice::Standard(z) => z.label().to_string(),
        _ => "Custom".to_string(),
    };
    egui::ComboBox::from_id_salt("model_paper")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for z in SheetSize::ALL {
                ui.selectable_value(&mut s.paper, PaperChoice::Standard(z), z.label());
            }
            ui.selectable_value(&mut s.paper, PaperChoice::Custom, "Custom size");
        });
}

// -------------------------------------------------------------- delivery --

/// The command that prints `path` on the system printer: CUPS' `lp`, to the
/// named printer (`lp -d NAME`); `None` is the default.
pub fn printer_command_to(
    path: &Path,
    copies: u32,
    printer: Option<&str>,
) -> (&'static str, Vec<String>) {
    printer_command_ext(path, copies, printer, &DeliveryExtras::default())
}

/// [`printer_command_to`] with the options of the Print dialog: collating
/// copies (`-o collate=true`) and a paper source (`-o InputSlot=...`).
pub fn printer_command_ext(
    path: &Path,
    copies: u32,
    printer: Option<&str>,
    extras: &DeliveryExtras,
) -> (&'static str, Vec<String>) {
    let mut args = Vec::new();
    if let Some(p) = printer.filter(|p| !p.is_empty()) {
        args.push("-d".to_string());
        args.push(p.to_string());
    }
    args.push("-n".to_string());
    args.push(copies.max(1).to_string());
    if extras.collate && copies > 1 {
        args.push("-o".to_string());
        args.push("collate=true".to_string());
    }
    if let Some(slot) = extras.input_slot.as_deref().filter(|s| !s.is_empty()) {
        args.push("-o".to_string());
        args.push(format!("InputSlot={slot}"));
    }
    args.push(path.to_string_lossy().into_owned());
    ("lp", args)
}

/// The command that opens `path` in the system's PDF viewer.
pub fn viewer_command(path: &Path) -> (&'static str, Vec<String>) {
    let p = path.to_string_lossy().into_owned();
    if cfg!(target_os = "macos") {
        ("open", vec!["-a".into(), "Preview".into(), p])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C".into(), "start".into(), String::new(), p])
    } else {
        ("xdg-open", vec![p])
    }
}

/// Writes `bytes` to `path`; the status text.
pub fn save_pdf(path: &Path, bytes: &[u8]) -> String {
    match std::fs::write(path, bytes) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save: {e}"),
    }
}

/// A scratch file for a print job.
fn temp_pdf(name: &str) -> std::path::PathBuf {
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    std::env::temp_dir().join(format!("plan-studio-{safe}-{}.pdf", std::process::id()))
}

/// Sends the finished PDF to `dest`; returns the status text. A PDF asks for a
/// file name. On a system without a printer command the PDF is saved instead.
pub fn deliver(bytes: &[u8], dest: Destination, copies: u32, name: &str) -> String {
    deliver_to(bytes, dest, copies, name, None)
}

/// [`deliver`] to a chosen printer (`None` is the system default).
pub fn deliver_to(
    bytes: &[u8],
    dest: Destination,
    copies: u32,
    name: &str,
    printer: Option<&str>,
) -> String {
    if cfg!(test) {
        return "Print skipped in tests".into();
    }
    match dest {
        Destination::Pdf => {
            let Some(path) = rfd::FileDialog::new()
                .set_file_name(format!("{name}.pdf"))
                .add_filter("PDF", &["pdf"])
                .save_file()
            else {
                return "Print cancelled".into();
            };
            save_pdf(&path, bytes)
        }
        Destination::Printer | Destination::Viewer => {
            if cfg!(target_os = "windows") && dest == Destination::Printer {
                return "Printing to a system printer is not available on Windows yet; \
                        use PDF file and print that"
                    .into();
            }
            let path = temp_pdf(name);
            if let Err(e) = std::fs::write(&path, bytes) {
                return format!("Could not write the print file: {e}");
            }
            let (prog, args) = if dest == Destination::Printer {
                printer_command_ext(&path, copies, printer, &delivery_extras())
            } else {
                viewer_command(&path)
            };
            match std::process::Command::new(prog).args(&args).status() {
                Ok(st) if st.success() => {
                    if dest == Destination::Printer {
                        match printer {
                            Some(p) => format!("Sent {copies} copy(ies) to {p}"),
                            None => format!("Sent {copies} copy(ies) to the printer"),
                        }
                    } else {
                        "Opened the print in the viewer".into()
                    }
                }
                Ok(st) => format!("{prog} failed ({st}); the PDF is at {}", path.display()),
                Err(e) => format!(
                    "Could not run {prog}: {e}; the PDF is at {}",
                    path.display()
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn options_follow_the_dialog() {
        let mut d = PrintDialog::for_layout(4, (36.0, 24.0), "Layout");
        d.s.paper = PaperChoice::Standard(SheetSize::Tabloid);
        d.s.landscape = false;
        d.s.scale = ScaleChoice::Percent;
        d.s.percent = 50.0;
        d.s.tiling = true;
        d.s.color = PrintColor::Grayscale;
        d.s.line_weights = false;
        let o = d.options();
        assert_eq!(o.paper, PaperSize::Standard(SheetSize::Tabloid));
        assert!(!o.landscape && o.tiling && !o.line_weights);
        assert_eq!(o.scale, PrintScale::Percent(50.0));
        assert_eq!(o.color, PrintColor::Grayscale);
        // 36 x 24 at 50% on Tabloid portrait (10.5 x 16.5 printable): 2 x 1.
        let g = d.tiles().unwrap();
        assert_eq!((g.cols, g.rows), (2, 1));
        assert!(d.summary().contains("8 paper page(s)"), "{}", d.summary());
    }

    #[test]
    fn letter_portrait_at_half_size_is_six_tiles_per_arch_d_sheet() {
        let mut d = PrintDialog::for_layout(1, (36.0, 24.0), "Layout");
        d.s.landscape = false;
        d.s.scale = ScaleChoice::Percent;
        d.s.percent = 50.0;
        d.s.tiling = true;
        assert_eq!(d.tiles().unwrap().count(), 6);
    }

    #[test]
    fn plan_view_dialogs_offer_drawing_scales() {
        let mut d = PrintDialog::for_plan(0, "Default Set", "1ST FLOOR PLAN");
        d.s.scale = ScaleChoice::Drawing(Scale::QuarterInch);
        assert_eq!(d.options().scale, PrintScale::Drawing(Scale::QuarterInch));
        d.s.scale = ScaleChoice::Custom;
        d.s.ratio = 0.5;
        assert!(d.error().is_some());
    }

    #[test]
    fn delivery_commands() {
        let (p, a) = printer_command_to(Path::new("/tmp/x.pdf"), 3, None);
        assert_eq!(
            (p, a.as_slice()),
            ("lp", ["-n", "3", "/tmp/x.pdf"].map(String::from).as_slice())
        );
        let (p, a) = viewer_command(Path::new("/tmp/x.pdf"));
        if cfg!(target_os = "macos") {
            assert_eq!(p, "open");
            assert_eq!(a[..2], ["-a".to_string(), "Preview".to_string()]);
        }
        let dir = std::env::temp_dir().join(format!("ps-print-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.pdf");
        assert!(save_pdf(&f, b"%PDF").starts_with("Saved "));
        assert_eq!(std::fs::read(&f).unwrap(), b"%PDF");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn lpstat_output_gives_the_printer_list_and_default() {
        let out = "printer Brother_HL_L2350DW is idle.  enabled since Tue Oct  6 09:12:00 2026\n\
                   printer HP_LaserJet now printing HP_LaserJet-12.  enabled since Mon Oct  5 08:00:00 2026\n\
                   \tconnecting to printer\n\
                   printer Plotter_36 disabled since Mon Oct  5 08:00:00 2026 -\n\
                   \treason unknown\n\
                   printer Brother_HL_L2350DW is idle.  enabled\n";
        assert_eq!(
            parse_lpstat_printers(out),
            ["Brother_HL_L2350DW", "HP_LaserJet", "Plotter_36"]
        );
        assert!(parse_lpstat_printers("lpstat: No destinations added.\n").is_empty());
        assert_eq!(
            parse_lpstat_default("system default destination: Brother_HL_L2350DW\n").as_deref(),
            Some("Brother_HL_L2350DW")
        );
        assert_eq!(
            parse_lpstat_default("no system default destination\n"),
            None
        );
    }

    #[test]
    fn a_picked_printer_goes_to_lp_with_d() {
        let (p, a) = printer_command_to(Path::new("/tmp/x.pdf"), 2, Some("HP_LaserJet"));
        assert_eq!(p, "lp");
        assert_eq!(
            a,
            ["-d", "HP_LaserJet", "-n", "2", "/tmp/x.pdf"].map(String::from)
        );
        let (_, a) = printer_command_to(Path::new("/tmp/x.pdf"), 1, None);
        assert_eq!(a, ["-n", "1", "/tmp/x.pdf"].map(String::from));
        let (_, a) = printer_command_to(Path::new("/tmp/x.pdf"), 1, Some(""));
        assert_eq!(a.len(), 3);
        let mut d = PrintDialog::new(1);
        assert_eq!(d.printer(), None);
        d.s.printer = Some("HP_LaserJet".into());
        assert_eq!(d.printer(), Some("HP_LaserJet"));
    }

    #[test]
    fn the_print_dialog_carries_the_perspective_quality() {
        let mut d = PrintDialog::new(2);
        assert_eq!((d.perspective_dpi(), d.perspective_samples()), (0, 0));
        d.s.persp_dpi = 200;
        d.s.persp_samples = 32;
        assert_eq!((d.perspective_dpi(), d.perspective_samples()), (200, 32));
    }

    #[test]
    fn print_model_and_3d_image_dialogs_validate_and_describe_the_render() {
        let cams = vec![(3, "Front Corner".to_string()), (7, "Rear".to_string())];
        let mut m = ModelDialog::new(cams.clone(), Some(7));
        assert_eq!(m.camera, 7);
        assert_eq!(m.camera_name(), "Rear");
        assert!(m.error().is_none());
        // Letter landscape: 10.5" x 8" printable, less the 0.3" caption, at 150 dpi.
        assert_eq!(m.pixels(), (1575, 1155));
        m.dpi = 10;
        assert!(m.error().is_some());
        assert!(ModelDialog::new(Vec::new(), None).error().is_some());
        let o = m.options();
        assert_eq!(o.scale, PrintScale::Actual);
        assert!(!o.tiling && o.range.is_none());
        let mut i = Image3dDialog::new(1.5);
        assert_eq!((i.width_px, i.height_px), (1600, 1067));
        assert!(i.error().is_none());
        i.width_px = 9000;
        assert!(i.error().is_some());
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            m.show(ctx);
            i.show(ctx);
        });
    }

    #[test]
    fn the_print_preview_window_pages_zooms_and_draws_every_item() {
        use plan_layout::{PreviewItem as I, PreviewPage};
        let page = |caption: &str| PreviewPage {
            paper_in: (11.0, 8.5),
            printable_pt: [18.0, 18.0, 774.0, 594.0],
            items: vec![
                I::ClipBegin([18.0, 18.0, 774.0, 594.0]),
                I::Fill {
                    pts: vec![(20.0, 20.0), (200.0, 20.0), (200.0, 120.0), (20.0, 120.0)],
                    color: [200, 200, 200],
                },
                I::Stroke {
                    pts: vec![(30.0, 30.0), (190.0, 30.0), (190.0, 110.0)],
                    closed: true,
                    width_pt: 1.5,
                    color: [0, 0, 0],
                    dash: vec![],
                },
                I::Stroke {
                    pts: vec![(30.0, 200.0), (300.0, 200.0)],
                    closed: false,
                    width_pt: 0.5,
                    color: [90, 90, 90],
                    dash: vec![6.0, 3.0],
                },
                I::Text {
                    x: 40.0,
                    y: 300.0,
                    size_pt: 12.0,
                    color: [0, 0, 0],
                    bold: true,
                    angle: 0.0,
                    text: "FIRST FLOOR PLAN".into(),
                },
                I::Text {
                    x: 400.0,
                    y: 300.0,
                    size_pt: 10.0,
                    color: [0, 0, 0],
                    bold: false,
                    angle: std::f64::consts::FRAC_PI_2,
                    text: "TURNED".into(),
                },
                I::Image {
                    rect: [400.0, 400.0, 480.0, 460.0],
                    px: (2, 2),
                    rgba: vec![255; 16],
                },
                I::ClipEnd,
            ],
            caption: caption.into(),
            scale: 0.5,
            color: PrintColor::Grayscale,
        };
        let mut d =
            PrintPreviewDialog::new("Layout", "SCALE: 1/4\"", vec![page("A-1"), page("A-2")]);
        assert_eq!((d.pages().len(), d.index()), (2, 0));
        d.go_to(9);
        assert_eq!(d.index(), 1, "clamped to the last page");
        d.go_to(0);
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let mut out = Outcome::Open;
            let _ = ctx.run(egui::RawInput::default(), |c| out = d.show(c));
            assert_eq!(out, Outcome::Open);
        }
        d.zoom = Some(60.0);
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |c| {
                d.show(c);
            });
        }
        // No pages: the window says so instead of drawing.
        let mut none = PrintPreviewDialog::new("Empty", "", vec![]);
        let _ = ctx.run(egui::RawInput::default(), |c| {
            assert_eq!(none.show(c), Outcome::Open);
        });
    }

    #[test]
    fn the_paper_list_offers_the_layouts_custom_sizes() {
        let mut d = PrintDialog::for_layout(1, (40.0, 30.0), "Layout")
            .with_custom_papers(vec![("Poster (30 x 40)".into(), (40.0, 30.0))]);
        assert_eq!(d.custom_papers.len(), 1);
        // Picking it fills in a custom paper of that size.
        let (name, (a, b)) = d.custom_papers[0].clone();
        assert_eq!(name, "Poster (30 x 40)");
        d.s.paper = PaperChoice::Custom;
        d.s.custom_w = a;
        d.s.custom_h = b;
        assert_eq!(d.options().paper.inches(true), (40.0, 30.0));
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |c| {
                d.show(c);
            });
        }
    }

    #[test]
    fn dialogs_draw_headless() {
        let ctx = egui::Context::default();
        let mut a = PrintDialog::for_layout(2, (36.0, 24.0), "L");
        a.s.tiling = true;
        a.s.dest = Destination::Printer;
        a.s.paper = PaperChoice::Custom;
        let mut b = PrintDialog::for_plan(0, "Default Set", "PLAN");
        b.s.scale = ScaleChoice::Custom;
        let mut c = ImageDialog::new(0, "Default Set");
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                a.show(ctx);
                b.show(ctx);
                c.show(ctx);
            });
        }
    }

    // ----------------------------------------------------------- round 16 --

    fn synced_context(f: impl FnOnce(&mut PrintViewContext)) {
        let mut c = PrintViewContext {
            synced: true,
            ..PrintViewContext::default()
        };
        f(&mut c);
        drawing_sheet::set_context(c);
    }

    #[test]
    fn fit_to_paper_defaults_to_95_percent_and_the_percentage_is_global() {
        forget_remembered_settings();
        assert_eq!(fit_percent(), 95.0);
        let d = PrintDialog::for_layout(1, (36.0, 24.0), "L");
        assert_eq!(d.options().scale, PrintScale::FitPercent(95.0));
        set_fit_percent(80.0);
        let d = PrintDialog::for_plan(0, "Default Set", "PLAN");
        assert_eq!(d.options().scale, PrintScale::FitPercent(80.0));
        set_fit_percent(500.0);
        assert_eq!(fit_percent(), 100.0, "it takes 1 to 100 percent");
        forget_remembered_settings();
    }

    #[test]
    fn settings_are_remembered_per_view_type_and_copies_reset() {
        forget_remembered_settings();
        synced_context(|_| {});
        let mut plan = PrintDialog::for_plan(0, "Default Set", "PLAN");
        plan.s.color = PrintColor::Grayscale;
        plan.s.copies = 3;
        plan.s.collate = false;
        plan.s.dest = Destination::Viewer;
        plan.accepted();
        let plan2 = PrintDialog::for_plan(0, "Default Set", "PLAN");
        assert_eq!(plan2.s.color, PrintColor::Grayscale);
        assert_eq!(plan2.s.dest, Destination::Viewer);
        assert_eq!(plan2.s.copies, 1, "copies always reset");
        assert!(!plan2.s.collate);
        // A layout has its own settings.
        let lay = PrintDialog::for_layout(2, (36.0, 24.0), "L");
        assert_eq!(lay.s.color, PrintColor::Color);
        assert_eq!(lay.s.dest, Destination::Pdf);
        // The range is not kept either.
        let mut lay = lay;
        lay.all = false;
        lay.from = 2;
        lay.accepted();
        assert!(PrintDialog::for_layout(2, (36.0, 24.0), "L").all);
        // Remember Print Settings after Printing off: nothing is kept or applied.
        synced_context(|c| c.sheet.remember_print_settings = false);
        let mut plan3 = PrintDialog::for_plan(0, "Default Set", "PLAN");
        assert_eq!(
            plan3.s.color,
            PrintColor::Color,
            "the remembered set is not used"
        );
        plan3.s.color = PrintColor::BlackWhite;
        plan3.accepted();
        synced_context(|_| {});
        assert_eq!(
            PrintDialog::for_plan(0, "Default Set", "PLAN").s.color,
            PrintColor::Grayscale,
            "and it kept nothing new"
        );
        forget_remembered_settings();
    }

    #[test]
    fn the_first_print_of_a_view_starts_from_its_drawing_sheet_setup() {
        forget_remembered_settings();
        synced_context(|c| {
            c.sheet.sheet =
                plan_core::drawing_sheet::SheetDims::new("ARCH C (18 x 24)", 24.0, 18.0);
            c.sheet.scale = plan_core::drawing_sheet::DrawingScale::per_foot(0.125);
            c.sheet.margins_in = [0.5, 0.5, 0.25, 0.25];
            c.sheet_shown = true;
        });
        let d = PrintDialog::for_plan(0, "Default Set", "PLAN");
        // Match Print Source: the Drawing Sheet's size and orientation.
        assert_eq!(d.s.paper, PaperChoice::MatchSource);
        let o = d.options();
        assert_eq!(o.paper, PaperSize::Standard(SheetSize::ArchC));
        assert!(o.landscape);
        // Print Source follows the Drawing Sheet toggle, the scale is To Scale.
        assert_eq!(d.source(), PrintSource::DrawingSheet);
        assert_eq!(o.scale, PrintScale::Drawing(Scale::EighthInch));
        // The paper's margins are the sheet's Drawing Margins, top bottom left right.
        assert_eq!(o.edge_margins_in, Some([0.5, 0.5, 0.25, 0.25]));
        assert_eq!(o.printable_in(), (23.5, 17.0));
        // Not shown: Current View.
        synced_context(|c| c.sheet_shown = false);
        assert_eq!(
            PrintDialog::for_plan(0, "Default Set", "PLAN").source(),
            PrintSource::CurrentView
        );
        // A portrait sheet turns the paper.
        synced_context(|c| c.sheet.portrait = true);
        let o = PrintDialog::for_plan(0, "Default Set", "PLAN").options();
        assert!(!o.landscape);
        assert_eq!(o.paper.inches(o.landscape), (18.0, 24.0));
        forget_remembered_settings();
    }

    #[test]
    fn print_source_chooses_the_part_of_the_plan() {
        forget_remembered_settings();
        synced_context(|c| {
            c.sheet_window = [0.0, 0.0, 1728.0, 1152.0];
            c.view_window = Some([100.0, 100.0, 500.0, 400.0]);
            c.sheet_shown = true;
        });
        let mut d = PrintDialog::for_plan(0, "Default Set", "PLAN");
        assert_eq!(d.options().plan_window, Some([0.0, 0.0, 1728.0, 1152.0]));
        d.source = PrintSource::CurrentView;
        assert_eq!(d.options().plan_window, Some([100.0, 100.0, 500.0, 400.0]));
        // A layout never prints a window of the plan.
        assert_eq!(
            PrintDialog::for_layout(1, (36.0, 24.0), "L")
                .options()
                .plan_window,
            None
        );
        // Without a canvas that told the dialog, the whole plan prints.
        drawing_sheet::set_context(PrintViewContext::default());
        assert_eq!(
            PrintDialog::for_plan(0, "Default Set", "PLAN")
                .options()
                .plan_window,
            None
        );
        forget_remembered_settings();
    }

    #[test]
    fn a_check_plot_matches_the_paper_to_the_reduced_sheet() {
        forget_remembered_settings();
        let mut d = PrintDialog::for_layout(1, (36.0, 24.0), "L");
        d.s.margin = 0.0;
        d.s.scale = ScaleChoice::CheckPlot;
        d.s.check_plot = 0.5;
        assert!(d.match_check_plot_paper());
        assert_eq!(d.s.paper, PaperChoice::Standard(SheetSize::ArchB));
        assert!(d.s.landscape);
        let o = d.options();
        assert_eq!(
            o.scale,
            PrintScale::CheckPlot {
                scale: Scale::QuarterInch,
                fraction: 0.5
            }
        );
        assert_eq!(plan_layout::print_scale_factor(&o, (36.0, 24.0)), 0.5);
        // One page, no tiling needed.
        assert_eq!(d.tiles().unwrap().count(), 1);
        // A quarter plot goes onto Letter.
        d.s.check_plot = 0.25;
        d.s.margin = 0.25;
        assert!(d.match_check_plot_paper());
        assert_eq!(d.s.paper, PaperChoice::Standard(SheetSize::Letter));
        // The information says what it is.
        assert!(
            d.info().iter().any(|m| m.contains("Check plot at 1/4")),
            "{:?}",
            d.info()
        );
        forget_remembered_settings();
    }

    #[test]
    fn collate_and_the_paper_source_reach_the_printer() {
        let extras = DeliveryExtras {
            collate: true,
            input_slot: Some("Tray2".into()),
        };
        let (p, a) = printer_command_ext(Path::new("/tmp/x.pdf"), 3, Some("HP"), &extras);
        assert_eq!(p, "lp");
        assert_eq!(
            a,
            [
                "-d",
                "HP",
                "-n",
                "3",
                "-o",
                "collate=true",
                "-o",
                "InputSlot=Tray2",
                "/tmp/x.pdf"
            ]
            .map(String::from)
        );
        // One copy needs no collating.
        let (_, a) = printer_command_ext(Path::new("/tmp/x.pdf"), 1, None, &extras);
        assert!(!a.contains(&"collate=true".to_string()));
        // The dialog sets them when it is accepted.
        forget_remembered_settings();
        let mut d = PrintDialog::for_plan(0, "Default Set", "PLAN");
        d.s.copies = 2;
        d.s.collate = true;
        d.s.paper_source = 1;
        assert!(d.options().collate);
        d.accepted();
        assert_eq!(
            delivery_extras(),
            DeliveryExtras {
                collate: true,
                input_slot: Some("Tray1".into())
            }
        );
        d.s.copies = 1;
        assert!(!d.options().collate);
        forget_remembered_settings();
    }

    #[test]
    fn include_watermark_needs_something_to_show() {
        forget_remembered_settings();
        synced_context(|c| {
            c.watermark_on = true;
            c.watermark_ready = false;
        });
        let d = PrintDialog::for_plan(0, "Default Set", "PLAN");
        assert!(
            !d.includes_watermark(),
            "the view has it on, but it is empty"
        );
        synced_context(|c| {
            c.watermark_on = true;
            c.watermark_ready = true;
        });
        let mut d = PrintDialog::for_plan(0, "Default Set", "PLAN");
        assert!(d.includes_watermark());
        assert!(d.options().watermark);
        d.include_watermark = false;
        assert!(!d.options().watermark);
        // Ticked with nothing to show: a message, and no watermark in the options.
        synced_context(|c| {
            c.watermark_on = false;
            c.watermark_ready = false;
        });
        let mut d = PrintDialog::for_plan(0, "Default Set", "PLAN");
        d.include_watermark = true;
        assert!(!d.options().watermark);
        assert!(d.info().iter().any(|m| m.contains("watermark")));
        forget_remembered_settings();
    }

    #[test]
    fn a_plan_print_reports_its_sheet_pages_and_scale() {
        forget_remembered_settings();
        // ARCH D at 1/4" = 1' shown, printed on Letter at 95%.
        synced_context(|c| c.sheet_shown = true);
        let mut d = PrintDialog::for_plan(0, "Default Set", "PLAN");
        d.s.paper = PaperChoice::Standard(SheetSize::Letter);
        d.s.scale = ScaleChoice::ToScale;
        d.s.margin = 0.25;
        d.s.use_sheet_margins = false;
        // At its own scale a 36 x 24 sheet does not fit Letter: it says so.
        assert!(
            d.info().iter().any(|m| m.contains("cut off")),
            "{:?}",
            d.info()
        );
        d.s.tiling = true;
        let g = d.tiles().unwrap();
        assert!(g.count() > 1);
        assert!(d.summary().contains("paper page"), "{}", d.summary());
        assert!(d.info().iter().any(|m| m.contains("crop marks")));
        forget_remembered_settings();
    }

    #[test]
    fn print_image_sizes_the_picture_to_the_paper_at_the_dpi() {
        let mut i = ImageDialog::new(0, "All");
        assert!(!i.fit_paper);
        i.fit_paper = true;
        // Letter landscape, 1/4" margins: 10.5" printable at 300 dpi.
        assert_eq!(i.paper_width_px(), 3150);
        i.dpi = 150;
        assert_eq!(i.paper_width_px(), 1575);
        i.landscape = false;
        assert_eq!(
            i.paper_width_px(),
            1125,
            "portrait Letter is 8.5 - 0.5 = 8 inches wide"
        );
        i.settle();
        assert_eq!(i.width_px, 1125);
        assert!(i.info()[0].contains("1125 pixels wide") && i.info()[0].contains("7.5 inches"));
        i.dpi = 72;
        assert!(i.info().iter().any(|m| m.contains("coarse")));
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |c| {
                i.show(c);
            });
        }
    }

    #[test]
    fn the_whole_dialog_draws_with_every_option() {
        forget_remembered_settings();
        synced_context(|c| {
            c.sheet_shown = true;
            c.watermark_ready = true;
            c.plan_extent = Some((
                plan_core::Point::new(0.0, 0.0),
                plan_core::Point::new(720.0, 480.0),
            ));
        });
        let ctx = egui::Context::default();
        let mut plan = PrintDialog::for_plan(0, "Default Set", "PLAN").with_current_page(1);
        let mut lay = PrintDialog::for_layout(3, (36.0, 24.0), "L").with_current_page(2);
        lay.current_only = true;
        lay.all = false;
        assert_eq!(lay.range(), Some((2, 2)));
        for choice in [
            ScaleChoice::FitPercent,
            ScaleChoice::ToScale,
            ScaleChoice::CheckPlot,
            ScaleChoice::Fit,
            ScaleChoice::Percent,
            ScaleChoice::Custom,
            ScaleChoice::Drawing(Scale::QuarterInch),
        ] {
            plan.s.scale = choice;
            lay.s.scale = choice;
            for _ in 0..2 {
                let _ = ctx.run(egui::RawInput::default(), |c| {
                    plan.show(c);
                    lay.show(c);
                });
            }
        }
        plan.s.dest = Destination::Printer;
        plan.s.copies = 3;
        let _ = ctx.run(egui::RawInput::default(), |c| {
            plan.show(c);
        });
        forget_remembered_settings();
    }
}
