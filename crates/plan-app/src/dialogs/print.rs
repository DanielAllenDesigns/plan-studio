//! File > Print: the Print dialog for a layout or a plan view, Print Image, and
//! the delivery of the finished PDF (a file, the system printer or a viewer).
//!
//! The dialog edits [`plan_layout::PrintOptions`]: destination, paper size and
//! orientation, scale (fit to page, 1:1, a drawing scale, a custom ratio or a
//! percentage), tiling with an overlap, margins, colour / grayscale / black
//! and white, line weights, page range and copies. `shell::layout_window`
//! builds the PDF from the answers; [`deliver`] gets it to its destination.
//!
//! PDF is the portable path and works everywhere. The system printer uses
//! CUPS' `lp` (macOS and Linux), to the printer picked from the list that
//! `lpstat -p` gives (the default printer otherwise); Windows saves the PDF
//! instead. "Open in
//! viewer" uses `open -a Preview` on macOS, `xdg-open` on Linux and `start` on
//! Windows.

use super::layout::frame;
use super::{row, section, Outcome};
use eframe::egui::{self, Ui};
use plan_docs::{Scale, SheetSize};
use plan_layout::{tile_grid, PaperSize, PrintColor, PrintOptions, PrintScale, TileGrid};
use std::cell::RefCell;
use std::path::Path;

/// Where the print goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum ScaleChoice {
    Fit,
    Actual,
    Percent,
    Drawing(Scale),
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PaperChoice {
    Standard(SheetSize),
    Custom,
}

/// Everything the dialog remembers between uses.
#[derive(Clone, Debug, PartialEq)]
struct Settings {
    dest: Destination,
    paper: PaperChoice,
    custom_w: f64,
    custom_h: f64,
    landscape: bool,
    scale: ScaleChoice,
    percent: f64,
    ratio: f64,
    tiling: bool,
    overlap: f64,
    margin: f64,
    color: PrintColor,
    line_weights: bool,
    copies: u32,
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
            paper: PaperChoice::Standard(SheetSize::Letter),
            custom_w: 13.0,
            custom_h: 19.0,
            landscape: true,
            scale: ScaleChoice::Fit,
            percent: 100.0,
            ratio: 48.0,
            tiling: false,
            overlap: 0.5,
            margin: 0.25,
            color: PrintColor::Color,
            line_weights: true,
            copies: 1,
            printer: None,
            persp_dpi: 0,
            persp_samples: 0,
        }
    }
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

thread_local! {
    static LAST: RefCell<Option<Settings>> = const { RefCell::new(None) };
}

/// The Print dialog.
pub struct PrintDialog {
    target: PrintTarget,
    s: Settings,
    all: bool,
    from: usize,
    to: usize,
    /// "Print Preview" was pressed: the owner shows the sheet at the chosen
    /// paper and scale, then clears this.
    pub preview_requested: bool,
    /// The printers, read when "System printer" is first chosen.
    printers: Option<Printers>,
}

impl PrintDialog {
    fn with(target: PrintTarget, pages: usize) -> Self {
        let mut s = LAST.with(|l| l.borrow().clone()).unwrap_or_default();
        if matches!(target, PrintTarget::Layout { .. })
            && matches!(s.scale, ScaleChoice::Drawing(_) | ScaleChoice::Custom)
        {
            s.scale = ScaleChoice::Fit;
        }
        Self {
            target,
            s,
            all: true,
            from: 1,
            to: pages.max(1),
            preview_requested: false,
            printers: None,
        }
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
        (!self.all).then_some((self.from, self.to))
    }

    pub fn copies(&self) -> u32 {
        self.s.copies.max(1)
    }

    /// Picks the colour mode as the radio buttons would (tests).
    #[cfg(test)]
    pub fn set_color(&mut self, color: PrintColor) {
        self.s.color = color;
    }

    /// The print options the dialog describes.
    pub fn options(&self) -> PrintOptions {
        let s = &self.s;
        PrintOptions {
            paper: match s.paper {
                PaperChoice::Standard(z) => PaperSize::Standard(z),
                PaperChoice::Custom => PaperSize::Custom {
                    width_in: s.custom_w,
                    height_in: s.custom_h,
                },
            },
            landscape: s.landscape,
            scale: match s.scale {
                ScaleChoice::Fit => PrintScale::Fit,
                ScaleChoice::Actual => PrintScale::Actual,
                ScaleChoice::Percent => PrintScale::Percent(s.percent),
                ScaleChoice::Drawing(d) => PrintScale::Drawing(d),
                ScaleChoice::Custom => PrintScale::Ratio(s.ratio),
            },
            tiling: s.tiling,
            overlap_in: s.overlap,
            margin_in: s.margin,
            color: s.color,
            line_weights: s.line_weights,
            range: self.range(),
            copies: s.copies.max(1),
        }
    }

    /// The tiles one layout sheet needs under the current options.
    pub fn tiles(&self) -> Option<TileGrid> {
        let PrintTarget::Layout { sheet_in, .. } = &self.target else {
            return None;
        };
        let o = self.options();
        let (pw, ph) = o.printable_in();
        let scale = match o.scale {
            PrintScale::Fit if o.tiling => 1.0,
            PrintScale::Fit => (pw / sheet_in.0).min(ph / sheet_in.1),
            PrintScale::Percent(p) => p / 100.0,
            _ => 1.0,
        };
        Some(if o.tiling {
            tile_grid(*sheet_in, scale, (pw, ph), o.overlap_in)
        } else {
            TileGrid {
                cols: 1,
                rows: 1,
                scale,
            }
        })
    }

    fn error(&self) -> Option<&'static str> {
        let s = &self.s;
        if let PrintTarget::Layout { pages, .. } = &self.target {
            if *pages == 0 {
                return Some("The layout has no pages to print");
            }
            if !self.all && (self.from < 1 || self.to < self.from || self.to > *pages) {
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
            _ => "The floor plan is printed at the chosen scale (tiled when it is larger \
                  than the paper)."
                .to_string(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        let title = match &self.target {
            PrintTarget::Layout { name, .. } => format!("Print: {name}"),
            PrintTarget::PlanView { title, .. } => format!("Print: {title}"),
        };
        let layout = matches!(self.target, PrintTarget::Layout { .. });
        let pages = match &self.target {
            PrintTarget::Layout { pages, .. } => *pages,
            PrintTarget::PlanView { .. } => 1,
        };
        let summary = self.summary();
        if self.s.dest == Destination::Printer && self.printers.is_none() {
            self.printers = Some(list_printers());
        }
        let Self {
            s,
            all,
            from,
            to,
            preview_requested,
            printers,
            ..
        } = self;
        let out = frame(ctx, &title, 420.0, error, |ui| {
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
                    if ui.small_button("Refresh").clicked() {
                        *printers = Some(list_printers());
                    }
                });
                row(ui, "Copies", |ui| {
                    ui.add(egui::DragValue::new(&mut s.copies).range(1..=99));
                });
            }
            section(ui, "Paper");
            row(ui, "Size", |ui| paper_combo(ui, &mut s.paper));
            if s.paper == PaperChoice::Custom {
                row(ui, "Width / Height", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut s.custom_w)
                            .suffix("\"")
                            .speed(0.1),
                    );
                    ui.add(
                        egui::DragValue::new(&mut s.custom_h)
                            .suffix("\"")
                            .speed(0.1),
                    );
                });
            }
            row(ui, "Orientation", |ui| {
                ui.radio_value(&mut s.landscape, true, "Landscape");
                ui.radio_value(&mut s.landscape, false, "Portrait");
            });
            row(ui, "Margin", |ui| {
                ui.add(
                    egui::DragValue::new(&mut s.margin)
                        .suffix("\"")
                        .speed(0.05)
                        .max_decimals(2),
                );
            });
            section(ui, "Scale");
            row(ui, "Print scale", |ui| {
                scale_combo(ui, &mut s.scale, layout)
            });
            match s.scale {
                ScaleChoice::Percent => row(ui, "Percent", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut s.percent)
                            .range(1.0..=800.0)
                            .suffix("%"),
                    );
                }),
                ScaleChoice::Custom => row(ui, "Ratio 1 :", |ui| {
                    ui.add(egui::DragValue::new(&mut s.ratio).range(1.0..=10_000.0));
                }),
                _ => {}
            }
            row(ui, "Tiling", |ui| {
                ui.checkbox(&mut s.tiling, "Tile onto several pages");
            });
            if s.tiling {
                row(ui, "Overlap", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut s.overlap)
                            .suffix("\"")
                            .speed(0.05)
                            .max_decimals(2),
                    );
                });
            }
            section(ui, "Appearance");
            row(ui, "Color", |ui| {
                ui.radio_value(&mut s.color, PrintColor::Color, "Color");
                ui.radio_value(&mut s.color, PrintColor::Grayscale, "Grayscale");
                ui.radio_value(&mut s.color, PrintColor::BlackWhite, "Black and white");
            });
            ui.checkbox(&mut s.line_weights, "Print line weights");
            if layout {
                row(ui, "Perspective views", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut s.persp_dpi)
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
                        egui::DragValue::new(&mut s.persp_samples)
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
                section(ui, "Print range");
                ui.radio_value(all, true, format!("All {pages} page(s)"));
                ui.horizontal(|ui| {
                    ui.radio_value(all, false, "Pages");
                    ui.add_enabled(!*all, egui::DragValue::new(from).range(1..=pages.max(1)));
                    ui.label("to");
                    ui.add_enabled(!*all, egui::DragValue::new(to).range(1..=pages.max(1)));
                });
            }
            ui.add_space(6.0);
            ui.weak(&summary);
            if ui
                .button("Print Preview")
                .on_hover_text("Show the sheet at this paper size and scale in the plan")
                .clicked()
            {
                *preview_requested = true;
            }
        });
        if out == Outcome::Ok {
            LAST.with(|l| *l.borrow_mut() = Some(self.s.clone()));
        }
        out
    }
}

fn paper_combo(ui: &mut Ui, current: &mut PaperChoice) {
    let shown = match current {
        PaperChoice::Standard(z) => z.label().to_string(),
        PaperChoice::Custom => "Custom".to_string(),
    };
    egui::ComboBox::from_id_salt("print_paper")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for z in SheetSize::ALL {
                ui.selectable_value(current, PaperChoice::Standard(z), z.label());
            }
            ui.selectable_value(current, PaperChoice::Custom, "Custom size");
        });
}

fn scale_combo(ui: &mut Ui, current: &mut ScaleChoice, layout: bool) {
    let shown = match current {
        ScaleChoice::Fit => "Fit to page".to_string(),
        ScaleChoice::Actual if layout => "100% (actual size)".to_string(),
        ScaleChoice::Actual => "1:1 (full size)".to_string(),
        ScaleChoice::Percent => "Percentage".to_string(),
        ScaleChoice::Drawing(s) => s.label().to_string(),
        ScaleChoice::Custom => "Custom ratio".to_string(),
    };
    egui::ComboBox::from_id_salt("print_scale")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(current, ScaleChoice::Fit, "Fit to page");
            if layout {
                ui.selectable_value(current, ScaleChoice::Actual, "100% (actual size)");
                ui.selectable_value(current, ScaleChoice::Percent, "Percentage");
            } else {
                ui.selectable_value(current, ScaleChoice::Actual, "1:1 (full size)");
                for s in Scale::ALL {
                    ui.selectable_value(current, ScaleChoice::Drawing(s), s.label());
                }
                ui.selectable_value(current, ScaleChoice::Custom, "Custom ratio");
            }
        });
}

// ----------------------------------------------------------- Print Image --

/// Print Image: the current plan view as a PNG.
pub struct ImageDialog {
    pub width_px: u32,
    /// `None` fits the plan to the image; `Some` draws it at that scale.
    pub scale: Option<Scale>,
    pub floor: usize,
    pub layer_set: String,
}

impl ImageDialog {
    pub fn new(floor: usize, layer_set: &str) -> Self {
        Self {
            width_px: 2048,
            scale: None,
            floor,
            layer_set: layer_set.to_string(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = (!(256..=8000).contains(&self.width_px))
            .then_some("The image must be 256 to 8000 pixels wide");
        let Self {
            width_px, scale, ..
        } = self;
        frame(ctx, "Print Image", 360.0, error, |ui| {
            row(ui, "Width", |ui| {
                ui.add(
                    egui::DragValue::new(width_px)
                        .range(64..=9000)
                        .suffix(" px"),
                );
            });
            row(ui, "Scale", |ui| {
                let shown = scale.map_or("Fit the plan".to_string(), |s| s.label().to_string());
                egui::ComboBox::from_id_salt("print_image_scale")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(scale, None, "Fit the plan");
                        for s in Scale::ALL {
                            ui.selectable_value(scale, Some(s), s.label());
                        }
                    });
            });
            ui.weak(
                "Saves the floor plan's lines as a PNG. For a rendering, use Ray Trace > Save PNG.",
            );
        })
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
        let s = LAST.with(|l| l.borrow().clone()).unwrap_or_default();
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
            ..PrintDialog {
                target: PrintTarget::PlanView {
                    floor: 0,
                    layer_set: String::new(),
                    title: String::new(),
                },
                s: self.s.clone(),
                all: true,
                from: 1,
                to: 1,
                preview_requested: false,
                printers: None,
            }
            .options()
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
            row(ui, "Size", |ui| paper_combo(ui, &mut s.paper));
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
            LAST.with(|l| *l.borrow_mut() = Some(self.s.clone()));
        }
        out
    }
}

// -------------------------------------------------------------- delivery --

/// The command that prints `path` on the system printer: CUPS' `lp`, to the
/// named printer (`lp -d NAME`); `None` is the default.
pub fn printer_command_to(
    path: &Path,
    copies: u32,
    printer: Option<&str>,
) -> (&'static str, Vec<String>) {
    let mut args = Vec::new();
    if let Some(p) = printer.filter(|p| !p.is_empty()) {
        args.push("-d".to_string());
        args.push(p.to_string());
    }
    args.push("-n".to_string());
    args.push(copies.max(1).to_string());
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
                printer_command_to(&path, copies, printer)
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
        assert!(d.tiles().is_none());
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
}
