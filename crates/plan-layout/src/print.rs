//! Printing: a layout (or one plan view) onto paper.
//!
//! [`print_layout_pdf`] prints the layout's pages through the same drawing
//! path as [`crate::render_pdf`], but onto a paper size of your choice: the
//! sheet is scaled (fit to page, 100%, a percentage), centred on the paper or
//! cut into overlapping tiles for a small printer, and drawn in colour,
//! grayscale or black and white, with or without line weights. Every printed
//! sheet gets a PDF bookmark.
//!
//! [`print_plan_view_pdf`] prints one floor plan at a drawing scale (1/4" =
//! 1', 1:1, a custom ratio, or the largest that fits) the same way.
//!
//! Paper sizes are inches. Copies are not part of a PDF; they are the
//! printer's business (see the application's Print dialog).
//!
//! Round 16 (manual pp. 1423-1444) adds: Fit to Paper at a percentage of the
//! paper (Chief's default 95), Check Plots (a sheet printed at a fraction of
//! its size on smaller paper), per-edge printing margins, the part of a plan
//! a print covers (the Drawing Sheet or what is on screen), advanced line
//! weights (a line weight scale, one weight for all lines, exact weights),
//! the Watermark, the information messages under the Print dialog's preview,
//! Scale to Fit, and the program-wide Customize Sheet Sizes file.

use crate::canvas::{emit, Canvas, Pen, Prim, BLACK};
use crate::extent::{frame_for, plan_bounds, source_size_in, Frame, SceneSource};
use crate::model::{BoxSource, CustomSheetSize, Layout, LayoutBox, SheetChoice};
use crate::render::{box_prims, draw_page, LayoutRenderContext};
use plan_core::watermark::{place_marks, PlacedMark, WatermarkKind, WatermarkSpec};
use plan_core::{Point, Project};
use plan_docs::pdf::FontSpec;
use plan_docs::{PdfColor, PdfColorMode, PdfDoc, Scale, SheetSize, CHIEF_SHEET_BACKGROUND};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// The paper in the printer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaperSize {
    /// A standard sheet (stored landscape; see [`PrintOptions::landscape`]).
    Standard(SheetSize),
    /// A custom size, inches.
    Custom { width_in: f64, height_in: f64 },
}

impl PaperSize {
    /// `(width, height)` in inches, landscape (wide) or portrait (tall).
    pub fn inches(self, landscape: bool) -> (f64, f64) {
        let (a, b) = match self {
            PaperSize::Standard(s) => s.inches(),
            PaperSize::Custom {
                width_in,
                height_in,
            } => (width_in, height_in),
        };
        let (long, short) = (a.max(b), a.min(b));
        if landscape {
            (long, short)
        } else {
            (short, long)
        }
    }

    /// Name for the dialog.
    pub fn label(self) -> String {
        match self {
            PaperSize::Standard(s) => s.label().to_string(),
            PaperSize::Custom {
                width_in,
                height_in,
            } => format!("Custom ({width_in:.1} x {height_in:.1})"),
        }
    }
}

/// How the sheet is scaled onto the paper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrintScale {
    /// The whole sheet on one page (with tiling on: 100%).
    Fit,
    /// Full size: a layout sheet at 100%; a plan view at 1:1.
    Actual,
    /// A layout sheet at this percentage.
    Percent(f64),
    /// A plan view at an architectural / engineering scale (1/4" = 1'...).
    /// A layout sheet is printed at 100%: its boxes carry their own scales.
    Drawing(Scale),
    /// A plan view at 1:n. A layout sheet is printed at 100%.
    Ratio(f64),
    /// Fit to Paper with the percentage of the printable area to fill (Chief's
    /// default is [`DEFAULT_FIT_PERCENT`]): the largest scale that puts the
    /// whole view on one page, then that share of it.
    FitPercent(f64),
    /// Check Plot: the view printed at `fraction` of its Drawing Scale, the
    /// line weights shrinking with it. `scale` is the view's drawing scale (a
    /// layout sheet has none: it is printed at `fraction` of its size).
    CheckPlot { scale: Scale, fraction: f64 },
}

/// Fit to Paper fills this percentage of the paper by default (manual
/// p. 1442); the value is global and kept between sessions.
pub const DEFAULT_FIT_PERCENT: f64 = 95.0;

/// The scale fractions the Check Plot list offers, with their labels.
pub const CHECK_PLOT_FRACTIONS: [(f64, &str); 6] = [
    (0.75, "3/4"),
    (2.0 / 3.0, "2/3"),
    (0.5, "1/2"),
    (1.0 / 3.0, "1/3"),
    (0.25, "1/4"),
    (0.125, "1/8"),
];

/// How line weights print.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenSetup {
    /// Multiplier on every pen weight made for Chief's default line weight
    /// scale (1 = 1/100 mm): `plan_core::drawing_sheet::LineWeightSetup::factor`.
    pub scale_factor: f64,
    /// Use 1 for all line weights: every line prints one thickness
    /// (`LineWeightSetup::SINGLE_WEIGHT_PT`), whatever the scale.
    pub single_weight: bool,
    /// Exact weights: a pen prints at its own thickness even when the sheet is
    /// printed smaller or larger than its size (not for a Check Plot).
    pub exact: bool,
}

impl Default for PenSetup {
    fn default() -> Self {
        Self {
            scale_factor: 1.0,
            single_weight: false,
            exact: false,
        }
    }
}

/// Colour handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PrintColor {
    #[default]
    Color,
    Grayscale,
    BlackWhite,
}

impl From<PrintColor> for PdfColorMode {
    fn from(c: PrintColor) -> Self {
        match c {
            PrintColor::Color => PdfColorMode::Color,
            PrintColor::Grayscale => PdfColorMode::Grayscale,
            PrintColor::BlackWhite => PdfColorMode::BlackWhite,
        }
    }
}

/// The Print dialog's answers.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintOptions {
    pub paper: PaperSize,
    /// Paper turned wide (true) or tall (false).
    pub landscape: bool,
    pub scale: PrintScale,
    /// Cut a sheet bigger than the paper into overlapping tiles.
    pub tiling: bool,
    /// Overlap of neighbouring tiles, inches.
    pub overlap_in: f64,
    /// Unprintable margin on every side of the paper, inches.
    pub margin_in: f64,
    pub color: PrintColor,
    /// Print each pen's weight (off: every line is a 0.5 pt hairline).
    pub line_weights: bool,
    /// Printed pages `(from, to)`, 1-based and inclusive; `None` is all.
    pub range: Option<(usize, usize)>,
    /// Copies the printer should make (a PDF holds one set).
    pub copies: u32,
    /// Collate the copies (whole sets in turn, not each page in a group).
    pub collate: bool,
    /// Printing margins `[top, bottom, left, right]` of the paper; `None` is
    /// [`margin_in`](Self::margin_in) on every side.
    pub edge_margins_in: Option<[f64; 4]>,
    /// Drawing Margins of the sheet `[top, bottom, left, right]`: where the
    /// Watermark's "Use Drawing Sheet Margin" starts (a layout's sheets use
    /// the layout's own margin).
    pub sheet_margins_in: [f64; 4],
    /// Plan views: the part of the plan to print, `[x0, y0, x1, y1]` plan
    /// inches: the Drawing Sheet's footprint or what is on screen. `None`
    /// prints the whole plan with its margin.
    pub plan_window: Option<[f64; 4]>,
    /// Include Watermark: the plan's Watermark settings are laid over every
    /// printed sheet.
    pub watermark: bool,
    /// How line weights print (line weight scale, one weight, exact).
    pub pens: PenSetup,
    /// DPI of the print: what pictures made for the print (Print Image) are
    /// resolved at. Vectors do not depend on it.
    pub dpi: u32,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            paper: PaperSize::Standard(SheetSize::Letter),
            landscape: true,
            scale: PrintScale::Fit,
            tiling: false,
            overlap_in: 0.5,
            margin_in: 0.25,
            color: PrintColor::Color,
            line_weights: true,
            range: None,
            copies: 1,
            collate: false,
            edge_margins_in: None,
            sheet_margins_in: [0.0; 4],
            plan_window: None,
            watermark: false,
            pens: PenSetup::default(),
            dpi: 300,
        }
    }
}

impl PrintOptions {
    /// The printing margins `[top, bottom, left, right]`, inches.
    pub fn margins(&self) -> [f64; 4] {
        self.edge_margins_in.unwrap_or([self.margin_in; 4])
    }

    /// `(width, height)` of the printable area of the paper, inches.
    pub fn printable_in(&self) -> (f64, f64) {
        let (w, h) = self.paper.inches(self.landscape);
        let [top, bottom, left, right] = self.margins();
        ((w - left - right).max(0.5), (h - top - bottom).max(0.5))
    }

    /// The printable area of the paper, `[x0, y0, x1, y1]` inches from the
    /// paper's lower-left corner.
    pub fn printable_rect_in(&self) -> [f64; 4] {
        let [_, bottom, left, _] = self.margins();
        let (w, h) = self.printable_in();
        [left, bottom, left + w, bottom + h]
    }
}

/// The tiles a scaled sheet needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileGrid {
    pub cols: usize,
    pub rows: usize,
    /// Printed scale of the sheet: 1 prints it at its own size.
    pub scale: f64,
}

impl TileGrid {
    /// Pages the sheet takes.
    pub fn count(&self) -> usize {
        self.cols * self.rows
    }
}

/// Tiles needed to print a `sheet_in` sheet at `scale` on an area of
/// `printable_in`: the first tile covers one printable area, every further tile
/// adds the printable size less `overlap_in`.
pub fn tile_grid(
    sheet_in: (f64, f64),
    scale: f64,
    printable_in: (f64, f64),
    overlap_in: f64,
) -> TileGrid {
    let count = |extent: f64, printable: f64| -> usize {
        let step = (printable - overlap_in.clamp(0.0, printable * 0.5)).max(0.1);
        if extent <= printable + 1e-6 {
            1
        } else {
            1 + ((extent - printable) / step - 1e-9).ceil() as usize
        }
    };
    TileGrid {
        cols: count(sheet_in.0 * scale, printable_in.0),
        rows: count(sheet_in.1 * scale, printable_in.1),
        scale,
    }
}

/// One sheet's drawing, ready to be put on paper.
struct Sheet {
    w_in: f64,
    h_in: f64,
    prims: Vec<Prim>,
    background: Option<PdfColor>,
    bookmark: String,
    /// The watermark laid over the sheet, when Include Watermark is on.
    watermark: Option<WmLayer>,
}

/// The factor the sheet is printed at: 1 prints it at its own size.
///
/// A plan view is drawn at its drawing scale first, so only the Percent,
/// Fit and Check Plot choices shrink it (a plan view prints with
/// [`PrintScale::Actual`] or a Check Plot).
pub fn print_scale_factor(opts: &PrintOptions, sheet_in: (f64, f64)) -> f64 {
    let (pw, ph) = opts.printable_in();
    let fit = (pw / sheet_in.0).min(ph / sheet_in.1);
    match opts.scale {
        PrintScale::Fit | PrintScale::FitPercent(_) if opts.tiling => 1.0,
        PrintScale::Fit => fit,
        PrintScale::FitPercent(p) => fit * (p / 100.0).clamp(0.01, 1.0),
        PrintScale::Percent(p) => (p / 100.0).max(0.01),
        PrintScale::CheckPlot { fraction, .. } => fraction.clamp(0.01, 1.0),
        PrintScale::Actual | PrintScale::Drawing(_) | PrintScale::Ratio(_) => 1.0,
    }
}

fn scale_factor(opts: &PrintOptions, sheet: (f64, f64)) -> f64 {
    print_scale_factor(opts, sheet)
}

/// Where one paper page shows a sheet: the scale and the shift that put the
/// sheet's window on the printable area.
struct Tile {
    col: usize,
    row: usize,
    cols: usize,
    rows: usize,
    /// 1-based number of this tile and how many the sheet takes.
    no: usize,
    total: usize,
    /// Printed scale of the sheet.
    scale: f64,
    /// Translation of the scaled sheet on the paper, points.
    e: f64,
    f: f64,
}

/// The paper pages `sheet` takes under `opts`: one, or a grid of overlapping
/// tiles when tiling is on.
fn tiles_of(sheet: &Sheet, opts: &PrintOptions) -> Vec<Tile> {
    let [_, bottom, left, _] = opts.margins();
    let (print_w, print_h) = opts.printable_in();
    let s = scale_factor(opts, (sheet.w_in, sheet.h_in));
    let (big_w, big_h) = (sheet.w_in * s, sheet.h_in * s);
    let grid = if opts.tiling {
        tile_grid(
            (sheet.w_in, sheet.h_in),
            s,
            (print_w, print_h),
            opts.overlap_in,
        )
    } else {
        TileGrid {
            cols: 1,
            rows: 1,
            scale: s,
        }
    };
    let overlap = opts.overlap_in.clamp(0.0, print_w.min(print_h) * 0.5);
    let n = grid.count();
    let mut out = Vec::with_capacity(n);
    let mut no = 0;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            no += 1;
            // The window of the printed sheet shown on this page, inches
            // from its left edge and from its top edge.
            let (x0, y_top) = if opts.tiling {
                (
                    col as f64 * (print_w - overlap),
                    row as f64 * (print_h - overlap),
                )
            } else {
                // Centred (or, when larger than the paper, cut around its centre).
                ((big_w - print_w) * 0.5, (big_h - print_h) * 0.5)
            };
            out.push(Tile {
                col,
                row,
                cols: grid.cols,
                rows: grid.rows,
                no,
                total: n,
                scale: s,
                e: (left - x0) * 72.0,
                // Content y runs up from the bottom of the printed sheet.
                f: (bottom - (big_h - y_top - print_h)) * 72.0,
            });
        }
    }
    out
}

/// The thickness every line prints at when the print uses one for all: the
/// hairline of "Print line weights" off, or the single weight, points on the
/// paper.
fn uniform_width_pt(opts: &PrintOptions) -> Option<f64> {
    if !opts.line_weights {
        Some(0.5)
    } else if opts.pens.single_weight {
        Some(plan_core::drawing_sheet::LineWeightSetup::SINGLE_WEIGHT_PT)
    } else {
        None
    }
}

/// `prims` with every pen weight multiplied by `k`.
fn scale_pen_widths(prims: &[Prim], k: f64) -> Vec<Prim> {
    prims
        .iter()
        .map(|p| match p {
            Prim::Stroke { pts, closed, pen } => Prim::Stroke {
                pts: pts.clone(),
                closed: *closed,
                pen: pen.scaled(k),
            },
            other => other.clone(),
        })
        .collect()
}

/// The sheet's drawing as it prints at factor `s`: with exact line weights a
/// pen keeps its own thickness on the paper (the scaling of the sheet is
/// taken out of its width); a Check Plot shrinks lines with the drawing.
fn printed_prims<'a>(sheet: &'a Sheet, opts: &PrintOptions, s: f64) -> Cow<'a, [Prim]> {
    let check_plot = matches!(opts.scale, PrintScale::CheckPlot { .. });
    if opts.pens.exact && !check_plot && (s - 1.0).abs() > 1e-9 {
        Cow::Owned(scale_pen_widths(&sheet.prims, 1.0 / s))
    } else {
        Cow::Borrowed(&sheet.prims)
    }
}

/// Puts `sheets` on paper pages of `opts`, one page or one tile grid each.
fn paginate(sheets: &[Sheet], opts: &PrintOptions) -> Vec<u8> {
    let (pw, ph) = opts.paper.inches(opts.landscape);
    let mut doc = PdfDoc::new(pw * 72.0, ph * 72.0);
    doc.set_color_mode(opts.color.into());
    let [x0, y0, _, _] = opts.printable_rect_in();
    let (print_w, print_h) = opts.printable_in();
    let mut first_page = true;
    for sheet in sheets {
        for tile in tiles_of(sheet, opts) {
            if !first_page {
                doc.new_page();
            }
            first_page = false;
            if tile.no == 1 {
                doc.add_bookmark(&sheet.bookmark);
            }
            let s = tile.scale;
            doc.set_uniform_line_width(None);
            doc.save_state();
            doc.clip_rect(x0 * 72.0, y0 * 72.0, print_w * 72.0, print_h * 72.0);
            doc.transform([s, 0.0, 0.0, s, tile.e, tile.f]);
            if let Some(w) = uniform_width_pt(opts) {
                doc.set_uniform_line_width(Some(w / s));
            }
            if let Some(bg) = sheet.background {
                doc.fill_rect(0.0, 0.0, sheet.w_in * 72.0, sheet.h_in * 72.0, bg);
            }
            emit(&mut doc, &printed_prims(sheet, opts, s));
            if let Some(wm) = &sheet.watermark {
                emit_watermark(&mut doc, wm, sheet);
            }
            doc.restore_state();
            doc.set_uniform_line_width(None);
            if opts.tiling && tile.total > 1 {
                tile_marks(
                    &mut doc,
                    opts,
                    (tile.col, tile.row),
                    (tile.cols, tile.rows),
                    tile.no,
                    tile.total,
                );
            }
        }
    }
    doc.finish()
}

// --------------------------------------------------------------- watermark --

/// What one watermark mark shows.
#[derive(Debug, Clone)]
enum WmKind {
    Text {
        text: String,
        size_pt: f64,
        color: PdfColor,
        font: Option<Arc<FontSpec>>,
    },
    Image {
        px: (u32, u32),
        rgba: Arc<Vec<u8>>,
    },
}

/// One mark on a sheet: points from the sheet's lower-left corner.
#[derive(Debug, Clone)]
struct WmMark {
    /// The centre.
    cx: f64,
    cy: f64,
    /// The size before turning.
    w: f64,
    h: f64,
    /// Counter-clockwise radians about the centre.
    angle: f64,
    kind: WmKind,
}

/// The watermark laid over one sheet.
#[derive(Debug, Clone)]
struct WmLayer {
    marks: Vec<WmMark>,
    /// Opacity, 0 to 1.
    alpha: f64,
}

impl WmMark {
    /// Where a text mark starts (its baseline's left end), points.
    fn origin(&self) -> (f64, f64) {
        PlacedMark {
            cx: self.cx,
            cy: self.cy,
            angle_deg: self.angle.to_degrees(),
            w: self.w,
            h: self.h,
        }
        .origin()
    }
}

impl WmLayer {
    /// The marks as the preview draws them on a page where the sheet is
    /// printed at `(s, e, f)`: scale, then the shift in points.
    fn preview_marks(&self, mode: PdfColorMode, (s, e, f): (f64, f64, f64)) -> Vec<PreviewMark> {
        self.marks
            .iter()
            .map(|m| {
                let (kind, color) = match &m.kind {
                    WmKind::Text {
                        text,
                        size_pt,
                        color,
                        ..
                    } => (
                        PreviewMarkKind::Text {
                            text: text.clone(),
                            size_pt: size_pt * s,
                        },
                        preview_rgb(mode, *color, false),
                    ),
                    WmKind::Image { px, rgba } => {
                        let mut out = rgba.as_ref().clone();
                        if mode != PdfColorMode::Color {
                            for p in out.as_chunks_mut::<4>().0 {
                                let v = mode.pixel([p[0], p[1], p[2]]);
                                p[..3].copy_from_slice(&v);
                            }
                        }
                        (PreviewMarkKind::Image { px: *px, rgba: out }, [0, 0, 0])
                    }
                };
                PreviewMark {
                    kind,
                    cx: m.cx * s + e,
                    cy: m.cy * s + f,
                    w: m.w * s,
                    h: m.h * s,
                    angle: m.angle,
                    color,
                    alpha: self.alpha,
                }
            })
            .collect()
    }
}

/// The watermark for a sheet of `size` inches when Include Watermark is on
/// and the plan has a usable one.
fn watermark_for(
    opts: &PrintOptions,
    cx: &LayoutRenderContext,
    size: (f64, f64),
    margins: [f64; 4],
) -> Option<WmLayer> {
    if !opts.watermark {
        return None;
    }
    watermark_layer(&cx.project.print_setup.watermark.spec, size, margins, cx)
}

/// The marks of `spec` on a sheet of `sheet_in` inches with the Drawing
/// Sheet's `margins`. `None` when the mark has nothing to show (no text, a
/// picture that cannot be read).
fn watermark_layer(
    spec: &WatermarkSpec,
    sheet_in: (f64, f64),
    margins: [f64; 4],
    cx: &LayoutRenderContext,
) -> Option<WmLayer> {
    if spec.problem().is_some() {
        return None;
    }
    // The mark at its own size, inches, and what it is made of.
    let (own, proto) = match spec.kind {
        WatermarkKind::Text => {
            let text = spec.text.lines().collect::<Vec<_>>().join(" ");
            let size = spec.font_size_pt();
            let font = (!spec.font.trim().is_empty())
                .then(|| Arc::new(FontSpec::new(spec.font.trim(), false, false)));
            let width_pt = font
                .as_ref()
                .and_then(|f| plan_docs::pdf::text_width_in(f, &text, size))
                .unwrap_or_else(|| PdfDoc::text_width(&text, size));
            let color = PdfColor::Rgb(spec.color[0], spec.color[1], spec.color[2]);
            (
                (width_pt / 72.0, spec.print_size_in),
                WmKind::Text {
                    text,
                    size_pt: size,
                    color,
                    font,
                },
            )
        }
        WatermarkKind::Image => {
            let img = cx.picture_for(spec.image_path.trim())?;
            if img.width == 0 || img.height == 0 {
                return None;
            }
            let area = plan_core::watermark::mark_area(spec, sheet_in, margins);
            let w = ((area[2] - area[0]) * spec.image_ratio).max(0.05);
            let h = w * f64::from(img.height) / f64::from(img.width);
            (
                (w, h),
                WmKind::Image {
                    px: (img.width, img.height),
                    rgba: Arc::new(img.rgba.clone()),
                },
            )
        }
    };
    let marks = place_marks(spec, sheet_in, margins, own)
        .into_iter()
        .map(|m| {
            let k = m.w / own.0.max(1e-9);
            let kind = match &proto {
                WmKind::Text {
                    text,
                    size_pt,
                    color,
                    font,
                } => WmKind::Text {
                    text: text.clone(),
                    size_pt: size_pt * k,
                    color: *color,
                    font: font.clone(),
                },
                other => other.clone(),
            };
            WmMark {
                cx: m.cx * 72.0,
                cy: m.cy * 72.0,
                w: m.w * 72.0,
                h: m.h * 72.0,
                angle: m.angle_deg.to_radians(),
                kind,
            }
        })
        .collect();
    Some(WmLayer {
        marks,
        alpha: spec.alpha(),
    })
}

/// Draws the watermark over the sheet being printed (the sheet's own
/// coordinates are in force), cut off at the sheet's edge.
fn emit_watermark(doc: &mut PdfDoc, wm: &WmLayer, sheet: &Sheet) {
    doc.save_state();
    doc.clip_rect(0.0, 0.0, sheet.w_in * 72.0, sheet.h_in * 72.0);
    doc.set_alpha(wm.alpha);
    for m in &wm.marks {
        match &m.kind {
            WmKind::Text {
                text,
                size_pt,
                color,
                font,
            } => {
                let (x, y) = m.origin();
                emit(
                    doc,
                    &[Prim::Text {
                        x,
                        y,
                        size: *size_pt,
                        color: *color,
                        bold: false,
                        angle: m.angle,
                        text: text.clone(),
                        font: font.clone(),
                    }],
                );
            }
            WmKind::Image { px, rgba } => {
                doc.image_rgba_placed(m.cx, m.cy, m.w, m.h, m.angle, px.0, px.1, rgba);
            }
        }
    }
    doc.restore_state();
}

// ----------------------------------------------------------------- preview --

/// One thing on a [`PreviewPage`]: points on the paper, y up from the
/// paper's bottom-left corner, colours already turned into the print's colour
/// mode (grayscale or black and white) and widths already scaled to the
/// printed size, so the preview is a picture of the page as it prints.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewItem {
    Fill {
        pts: Vec<(f64, f64)>,
        color: [u8; 3],
    },
    Stroke {
        pts: Vec<(f64, f64)>,
        closed: bool,
        /// Pen width on the paper, points.
        width_pt: f64,
        color: [u8; 3],
        /// On/off lengths on the paper, points; empty for solid.
        dash: Vec<f64>,
    },
    Text {
        x: f64,
        y: f64,
        size_pt: f64,
        color: [u8; 3],
        bold: bool,
        /// Counter-clockwise radians about `(x, y)`.
        angle: f64,
        text: String,
    },
    /// RGBA pixels (`px.0 * px.1 * 4`, rows top to bottom) in `rect`
    /// (`[x0, y0, x1, y1]`).
    Image {
        rect: [f64; 4],
        px: (u32, u32),
        rgba: Vec<u8>,
    },
    /// Clip what follows to the rectangle, until the matching `ClipEnd`.
    ClipBegin([f64; 4]),
    ClipEnd,
    /// A watermark mark, laid over what was drawn before it.
    Mark(PreviewMark),
}

/// What a watermark mark shows in the preview.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewMarkKind {
    Text {
        text: String,
        /// Font size on the paper, points.
        size_pt: f64,
    },
    /// RGBA pixels (`px.0 * px.1 * 4`, rows top to bottom).
    Image { px: (u32, u32), rgba: Vec<u8> },
}

/// One watermark mark as the preview draws it: points on the paper (y up),
/// the centre and the size before turning, the turn, the colour in the
/// print's colour mode and the opacity.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewMark {
    pub kind: PreviewMarkKind,
    pub cx: f64,
    pub cy: f64,
    pub w: f64,
    pub h: f64,
    /// Counter-clockwise radians about the centre.
    pub angle: f64,
    pub color: [u8; 3],
    pub alpha: f64,
}

/// One paper page of a print as it will come out: the sheet scaled and
/// placed on the paper (or one tile of it), in the chosen colour mode, with
/// the pen weights the print uses.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewPage {
    /// The paper, `(width, height)` in inches.
    pub paper_in: (f64, f64),
    /// The printable area, `[x0, y0, x1, y1]` points on the paper.
    pub printable_pt: [f64; 4],
    pub items: Vec<PreviewItem>,
    /// `A-1 PLAN`, with `TILE 2 OF 4` when the sheet is cut into tiles.
    pub caption: String,
    /// The sheet's scale on the paper (1 = its own size).
    pub scale: f64,
    pub color: PrintColor,
}

fn preview_rgb(mode: PdfColorMode, c: PdfColor, area: bool) -> [u8; 3] {
    (if area { mode.area(c) } else { mode.ink(c) }).rgb8()
}

fn preview_of(sheet: &Sheet, tile: &Tile, opts: &PrintOptions) -> PreviewPage {
    let mode: PdfColorMode = opts.color.into();
    let (pw, ph) = opts.paper.inches(opts.landscape);
    let [px0, py0, px1, py1] = opts.printable_rect_in();
    let printable = [px0 * 72.0, py0 * 72.0, px1 * 72.0, py1 * 72.0];
    let (s, e, f) = (tile.scale, tile.e, tile.f);
    let at = |p: &(f64, f64)| (p.0 * s + e, p.1 * s + f);
    let rect_at = |r: &[f64; 4]| {
        let (a, b) = (at(&(r[0], r[1])), at(&(r[2], r[3])));
        [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
    };
    let mut items = vec![PreviewItem::ClipBegin(printable)];
    if let Some(bg) = sheet.background {
        let r = rect_at(&[0.0, 0.0, sheet.w_in * 72.0, sheet.h_in * 72.0]);
        items.push(PreviewItem::Fill {
            pts: vec![(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])],
            color: preview_rgb(mode, bg, true),
        });
    }
    let prims = printed_prims(sheet, opts, s);
    for p in prims.iter() {
        match p {
            Prim::Stroke { pts, closed, pen } => items.push(PreviewItem::Stroke {
                pts: pts.iter().map(at).collect(),
                closed: *closed,
                // Without line weights every pen prints as a 0.5 pt hairline;
                // with one weight for all lines, as that weight.
                width_pt: uniform_width_pt(opts).unwrap_or(pen.width * s),
                color: preview_rgb(mode, pen.color, false),
                dash: pen.dash.pattern().iter().map(|d| d * s).collect(),
            }),
            Prim::Fill { pts, color } => items.push(PreviewItem::Fill {
                pts: pts.iter().map(at).collect(),
                color: preview_rgb(mode, *color, true),
            }),
            Prim::Text {
                x,
                y,
                size,
                color,
                bold,
                angle,
                text,
                ..
            } => {
                let (x, y) = at(&(*x, *y));
                items.push(PreviewItem::Text {
                    x,
                    y,
                    size_pt: size * s,
                    color: preview_rgb(mode, *color, false),
                    bold: *bold,
                    angle: *angle,
                    text: text.clone(),
                });
            }
            Prim::Image { rect, px, rgba } => {
                let mut out = rgba.clone();
                if mode != PdfColorMode::Color {
                    for p in out.as_chunks_mut::<4>().0 {
                        let v = mode.pixel([p[0], p[1], p[2]]);
                        p[..3].copy_from_slice(&v);
                    }
                }
                items.push(PreviewItem::Image {
                    rect: rect_at(rect),
                    px: *px,
                    rgba: out,
                });
            }
            Prim::ClipBegin(r) => items.push(PreviewItem::ClipBegin(rect_at(r))),
            Prim::ClipEnd => items.push(PreviewItem::ClipEnd),
        }
    }
    if let Some(wm) = &sheet.watermark {
        // The marks are cut off at the sheet's edge like the drawing.
        items.push(PreviewItem::ClipBegin(rect_at(&[
            0.0,
            0.0,
            sheet.w_in * 72.0,
            sheet.h_in * 72.0,
        ])));
        items.extend(
            wm.preview_marks(mode, (s, e, f))
                .into_iter()
                .map(PreviewItem::Mark),
        );
        items.push(PreviewItem::ClipEnd);
    }
    items.push(PreviewItem::ClipEnd);
    let caption = if tile.total > 1 {
        format!("{}  TILE {} OF {}", sheet.bookmark, tile.no, tile.total)
    } else {
        sheet.bookmark.clone()
    };
    PreviewPage {
        paper_in: (pw, ph),
        printable_pt: printable,
        items,
        caption,
        scale: s,
        color: opts.color,
    }
}

fn preview_pages(sheets: &[Sheet], opts: &PrintOptions) -> Vec<PreviewPage> {
    sheets
        .iter()
        .flat_map(|sheet| {
            tiles_of(sheet, opts)
                .into_iter()
                .map(move |t| preview_of(sheet, &t, opts))
        })
        .collect()
}

/// Print Preview of the layout under `opts`: one [`PreviewPage`] per paper
/// page (a sheet cut into tiles takes several), drawn the way
/// [`print_layout_pdf`] prints it: the chosen colour mode, the pen weights
/// (or hairlines when line weights are off), hatches at the box scales, the
/// page placed and scaled on the paper.
pub fn layout_print_preview(
    layout: &Layout,
    cx: &LayoutRenderContext,
    opts: &PrintOptions,
) -> Vec<PreviewPage> {
    preview_pages(&layout_sheets(layout, cx, opts), opts)
}

/// Corner marks of the printable area and the tile's place in the grid.
fn tile_marks(
    doc: &mut PdfDoc,
    opts: &PrintOptions,
    (col, row): (usize, usize),
    (cols, rows): (usize, usize),
    no: usize,
    total: usize,
) {
    let [x0, y0, x1, y1] = opts.printable_rect_in().map(|v| v * 72.0);
    let smallest = opts.margins().into_iter().fold(f64::MAX, f64::min) * 72.0;
    let arm = (smallest * 0.8).max(6.0);
    doc.set_gray(0.0);
    for (x, y, dx, dy) in [
        (x0, y0, 1.0, 1.0),
        (x1, y0, -1.0, 1.0),
        (x0, y1, 1.0, -1.0),
        (x1, y1, -1.0, -1.0),
    ] {
        doc.line(x, y, x + dx * arm, y, 0.4);
        doc.line(x, y, x, y + dy * arm, 0.4);
    }
    doc.text(
        x0 + arm + 4.0,
        (y0 * 0.35).max(2.0),
        6.0,
        &format!(
            "TILE {no} OF {total}  (COLUMN {} OF {cols}, ROW {} OF {rows})",
            col + 1,
            row + 1
        ),
    );
}

fn selected<T>(items: Vec<T>, range: Option<(usize, usize)>) -> Vec<T> {
    match range {
        None => items,
        Some((from, to)) => items
            .into_iter()
            .enumerate()
            .filter(|(i, _)| (from..=to).contains(&(i + 1)))
            .map(|(_, v)| v)
            .collect(),
    }
}

/// Prints the layout's pages (template pages are not printed) with `opts`.
/// The PDF has one bookmark per sheet, `A-1 TITLE`.
pub fn print_layout_pdf(layout: &Layout, cx: &LayoutRenderContext, opts: &PrintOptions) -> Vec<u8> {
    paginate(&layout_sheets(layout, cx, opts), opts)
}

/// The sheets of the layout's printed pages in `opts.range`, each at its own
/// size (a page can have a sheet of its own in Page Specification).
fn layout_sheets(layout: &Layout, cx: &LayoutRenderContext, opts: &PrintOptions) -> Vec<Sheet> {
    let (w_in, h_in) = layout.sheet_inches();
    let scenes = SceneSource::for_context(cx);
    let background = layout.page_background.then_some(PdfColor::Rgb(
        CHIEF_SHEET_BACKGROUND.0,
        CHIEF_SHEET_BACKGROUND.1,
        CHIEF_SHEET_BACKGROUND.2,
    ));
    let pages = layout.content_pages();
    let indices = selected((0..pages.len()).collect::<Vec<_>>(), opts.range);
    // The layout's Drawing Margin is its border inset, all round.
    let margins = [layout.margins_in; 4];
    let watermark = |size: (f64, f64)| watermark_for(opts, cx, size, margins);
    let mut sheets = Vec::new();
    for index in indices {
        let mut cv = Canvas::new();
        draw_page(&mut cv, layout, &pages, index, cx, &scenes);
        let (w_in, h_in) = layout.page_sheet_inches(pages[index]);
        let mut prims = cv.prims;
        apply_pen_factor(&mut prims, opts.pens.scale_factor);
        sheets.push(Sheet {
            w_in,
            h_in,
            prims,
            background,
            bookmark: format!("{} {}", pages[index].sheet_number(), pages[index].title),
            watermark: watermark((w_in, h_in)),
        });
    }
    if sheets.is_empty() {
        sheets.push(Sheet {
            w_in,
            h_in,
            prims: Vec::new(),
            background,
            bookmark: layout.name.clone(),
            watermark: watermark((w_in, h_in)),
        });
    }
    sheets
}

/// `prims` with the line weight scale applied to every pen.
fn apply_pen_factor(prims: &mut [Prim], factor: f64) {
    if (factor - 1.0).abs() < 1e-9 || factor <= 0.0 {
        return;
    }
    for p in prims {
        if let Prim::Stroke { pen, .. } = p {
            *pen = pen.scaled(factor);
        }
    }
}

/// `prims` moved by `(dx, dy)` points.
fn translate_prims(prims: &mut [Prim], dx: f64, dy: f64) {
    for p in prims {
        match p {
            Prim::Stroke { pts, .. } | Prim::Fill { pts, .. } => {
                for q in pts.iter_mut() {
                    *q = (q.0 + dx, q.1 + dy);
                }
            }
            Prim::Text { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Prim::Image { rect, .. } | Prim::ClipBegin(rect) => {
                rect[0] += dx;
                rect[1] += dy;
                rect[2] += dx;
                rect[3] += dy;
            }
            Prim::ClipEnd => {}
        }
    }
}

/// Size in paper inches of the part of a plan view that prints at `scale`:
/// the window of `opts` or the whole view with its margin.
fn view_size_at(
    source: &BoxSource,
    scale: Scale,
    cx: &LayoutRenderContext,
    opts: &PrintOptions,
) -> (f64, f64) {
    match opts.plan_window {
        Some([x0, y0, x1, y1]) => {
            let k = scale.inches_per_foot() / 12.0;
            ((x1 - x0).abs() * k, (y1 - y0).abs() * k)
        }
        None => source_size_in(source, scale, cx),
    }
}

/// The largest drawing scale at which the view fits `fraction` of the
/// printable area on one page.
fn largest_fitting_scale(
    cx: &LayoutRenderContext,
    source: &BoxSource,
    opts: &PrintOptions,
    fraction: f64,
) -> Scale {
    let (pw, ph) = opts.printable_in();
    let (pw, ph) = (pw * fraction, ph * fraction);
    let mut s = Scale::ThreeInch;
    loop {
        let (w, h) = view_size_at(source, s, cx, opts);
        if w <= pw + 1e-9 && h <= ph + 1e-9 {
            return s;
        }
        match s.smaller_any() {
            Some(next) => s = next,
            None => return s,
        }
    }
}

/// The drawing scale a plan view prints at under `opts`: the explicit scale,
/// else the largest that fits the printable area (1/4" = 1' when tiling).
pub fn plan_print_scale(
    cx: &LayoutRenderContext,
    source: &BoxSource,
    opts: &PrintOptions,
) -> Scale {
    match opts.scale {
        PrintScale::Drawing(s) => s,
        PrintScale::CheckPlot { scale, .. } => scale,
        PrintScale::Ratio(n) => Scale::Ratio(n.round().clamp(1.0, 10_000.0) as u32),
        PrintScale::Actual => Scale::Ratio(1),
        PrintScale::Percent(p) => {
            Scale::Ratio((100.0 / p.max(0.1)).round().clamp(1.0, 10_000.0) as u32)
        }
        PrintScale::Fit | PrintScale::FitPercent(_) if opts.tiling => Scale::QuarterInch,
        PrintScale::Fit => largest_fitting_scale(cx, source, opts, 1.0),
        PrintScale::FitPercent(p) => {
            largest_fitting_scale(cx, source, opts, (p / 100.0).clamp(0.01, 1.0))
        }
    }
}

/// The sheet of floor `floor` at its print scale, and the options to place
/// it with (a plan view prints at its drawing scale: the sheet is not scaled
/// again, except by a Check Plot).
///
/// With `opts.plan_window` the sheet is that part of the plan (the Drawing
/// Sheet's footprint, or what is on screen) at the drawing scale; the rest of
/// the plan is cut off at its edge.
fn plan_view_sheet(
    cx: &LayoutRenderContext,
    floor: usize,
    layer_set: &str,
    title: &str,
    opts: &PrintOptions,
) -> (Sheet, PrintOptions, Scale) {
    let source = BoxSource::PlanView {
        floor,
        layer_set: layer_set.to_string(),
    };
    let scale = plan_print_scale(cx, &source, opts);
    let (w, h) = source_size_in(&source, scale, cx);
    let note_h = 0.3;
    let mut b = LayoutBox::new(
        1,
        (Point::new(0.0, note_h), Point::new(w, h + note_h)),
        source.clone(),
        scale,
    );
    b.border = false;
    b.clip = false;
    let scenes = SceneSource::for_context(cx);
    let mut prims = box_prims(&b, cx, &scenes, &crate::layers::LayoutLayers::default());
    apply_pen_factor(&mut prims, opts.pens.scale_factor);
    let (mut sheet_w, mut sheet_h) = (w, h);
    if let (Some([wx0, wy0, wx1, wy1]), Frame::Scaled { lo, .. }) =
        (opts.plan_window, frame_for(&source, cx, &scenes))
    {
        // Plan inches to sheet points: the box put the plan's frame at the
        // sheet's lower-left (above the note strip); move the window there.
        let k = scale.points_per_inch();
        let (x0, y0) = (wx0.min(wx1), wy0.min(wy1));
        sheet_w = (wx1 - wx0).abs() * k / 72.0;
        sheet_h = (wy1 - wy0).abs() * k / 72.0;
        translate_prims(&mut prims, (lo.x - x0) * k, (lo.y - y0) * k);
        let mut clipped = Vec::with_capacity(prims.len() + 2);
        clipped.push(Prim::ClipBegin([
            0.0,
            note_h * 72.0,
            sheet_w * 72.0,
            (sheet_h + note_h) * 72.0,
        ]));
        clipped.append(&mut prims);
        clipped.push(Prim::ClipEnd);
        prims = clipped;
    }
    let mut cv = Canvas::new();
    cv.text(
        4.0,
        6.0,
        8.0,
        BLACK,
        &format!("{title}   SCALE: {}", scale.label()),
    );
    cv.line((0.0, 1.0), (sheet_w * 72.0, 1.0), Pen::gray(0.25, 0.7));
    prims.extend(cv.prims);
    let size = (sheet_w, sheet_h + note_h);
    let sheet = Sheet {
        w_in: size.0,
        h_in: size.1,
        prims,
        background: None,
        bookmark: title.to_string(),
        watermark: watermark_for(opts, cx, size, opts.sheet_margins_in),
    };
    let mut o = opts.clone();
    o.scale = match opts.scale {
        PrintScale::CheckPlot { fraction, .. } => PrintScale::CheckPlot { scale, fraction },
        _ => PrintScale::Actual,
    };
    (sheet, o, scale)
}

/// Prints floor `floor` of the plan at a drawing scale. Returns the PDF and
/// the scale used. The printed sheet is the plan (walls, openings, dimensions,
/// CAD) with its margin; the scale is printed under it.
pub fn print_plan_view_pdf(
    cx: &LayoutRenderContext,
    floor: usize,
    layer_set: &str,
    title: &str,
    opts: &PrintOptions,
) -> (Vec<u8>, Scale) {
    let (sheet, o, scale) = plan_view_sheet(cx, floor, layer_set, title, opts);
    (paginate(&[sheet], &o), scale)
}

/// Print Preview of floor `floor` under `opts`: the page as
/// [`print_plan_view_pdf`] prints it, in the chosen colour mode and pen
/// weights, and the scale used.
pub fn plan_view_print_preview(
    cx: &LayoutRenderContext,
    floor: usize,
    layer_set: &str,
    title: &str,
    opts: &PrintOptions,
) -> (Vec<PreviewPage>, Scale) {
    let (sheet, o, scale) = plan_view_sheet(cx, floor, layer_set, title, opts);
    (preview_pages(&[sheet], &o), scale)
}

/// A copy of `layout` whose perspective boxes render at `dpi` dots per inch
/// and `samples` samples per pixel (`0` leaves a box's own setting): Print
/// Model's quality override for a whole layout.
pub fn with_perspective_quality(layout: &Layout, dpi: u32, samples: u32) -> Layout {
    let mut out = layout.clone();
    for b in out.pages.iter_mut().flat_map(|p| p.boxes.iter_mut()) {
        if matches!(b.source, BoxSource::Perspective { .. }) {
            if dpi > 0 {
                b.dpi = dpi;
            }
            if samples > 0 {
                b.samples = samples;
            }
        }
    }
    out
}

/// Print Model: the perspective view of camera `camera_id`, ray traced at
/// `dpi` dots per inch and `samples` per pixel, filling the printable area of
/// the paper in `opts` (a caption `title` under it). The render is capped at
/// [`crate::MAX_PERSPECTIVE_PIXELS`], so a large sheet at a high DPI prints at
/// the highest resolution the budget allows.
pub fn print_model_pdf(
    cx: &LayoutRenderContext,
    camera_id: plan_core::Id,
    dpi: u32,
    samples: u32,
    title: &str,
    opts: &PrintOptions,
) -> Vec<u8> {
    let (pw, ph) = opts.printable_in();
    let note_h = 0.3;
    let (w, h) = (pw, (ph - note_h).max(1.0));
    let mut b = LayoutBox::new(
        1,
        (Point::new(0.0, note_h), Point::new(w, h + note_h)),
        BoxSource::Perspective { camera_id },
        Scale::QuarterInch,
    );
    b.border = false;
    b.clip = true;
    b.dpi = dpi;
    b.samples = samples;
    let scenes = SceneSource::for_context(cx);
    let mut prims = box_prims(&b, cx, &scenes, &crate::layers::LayoutLayers::default());
    let mut cv = Canvas::new();
    let (px_w, px_h) = crate::perspective_pixels(w, h, dpi);
    cv.text(
        4.0,
        6.0,
        8.0,
        BLACK,
        &format!("{title}   {px_w} x {px_h} px, {} dpi", b.effective_dpi()),
    );
    prims.extend(cv.prims);
    let sheet = Sheet {
        w_in: w,
        h_in: h + note_h,
        prims,
        background: None,
        bookmark: title.to_string(),
        watermark: watermark_for(opts, cx, (w, h + note_h), opts.sheet_margins_in),
    };
    let mut o = opts.clone();
    o.scale = PrintScale::Actual;
    paginate(&[sheet], &o)
}

/// One segment to rasterise: start, end, width in pixels.
pub type RasterLine = ((f64, f64), (f64, f64), f64);

/// Rasterises line segments onto a white RGBA image: `lines` are in inches of
/// the drawing, `(x0, y0, x1, y1)` the area shown, `width_px` the image width
/// (the height follows the area's aspect). Strokes are black, one pixel wide
/// (two for heavy lines), drawn with a supersampled distance test. Returns
/// `(width, height, rgba)`.
pub fn rasterize_lines(
    lines: &[RasterLine],
    area: (f64, f64, f64, f64),
    width_px: u32,
) -> (u32, u32, Vec<u8>) {
    let (x0, y0, x1, y1) = area;
    let aw = (x1 - x0).max(1e-6);
    let ah = (y1 - y0).max(1e-6);
    let w = width_px.clamp(16, 8000);
    let h = ((f64::from(w) * ah / aw).round() as u32).clamp(16, 8000);
    let k = f64::from(w) / aw;
    let mut cover = vec![0.0_f32; (w * h) as usize];
    for &((ax, ay), (bx, by), weight) in lines {
        let (ax, ay) = ((ax - x0) * k, (y1 - ay) * k);
        let (bx, by) = ((bx - x0) * k, (y1 - by) * k);
        let half = (weight * 0.5).max(0.5) as f32;
        let (dx, dy) = (bx - ax, by - ay);
        let len = dx.hypot(dy);
        let steps = (len * 2.0).ceil().max(1.0) as usize;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let (px, py) = (ax + dx * t, ay + dy * t);
            let r = f64::from(half);
            let (lo_x, hi_x) = (
                (px - r).floor().max(0.0) as u32,
                (px + r).ceil().min(f64::from(w) - 1.0) as u32,
            );
            let (lo_y, hi_y) = (
                (py - r).floor().max(0.0) as u32,
                (py + r).ceil().min(f64::from(h) - 1.0) as u32,
            );
            for yy in lo_y..=hi_y {
                for xx in lo_x..=hi_x {
                    let d = ((f64::from(xx) + 0.5 - px).hypot(f64::from(yy) + 0.5 - py)) as f32;
                    let c = (half + 0.5 - d).clamp(0.0, 1.0);
                    let cell = &mut cover[(yy * w + xx) as usize];
                    if c > *cell {
                        *cell = c;
                    }
                }
            }
        }
    }
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for c in cover {
        let v = (255.0 * (1.0 - c)).round() as u8;
        rgba.extend_from_slice(&[v, v, v, 255]);
    }
    (w, h, rgba)
}

/// A raster image (PNG pixels) of floor `floor`'s plan view at a drawing scale:
/// walls, openings, dimensions and CAD as black lines on white, `width_px`
/// wide. Returns `(width, height, rgba)`.
pub fn plan_view_image(
    cx: &LayoutRenderContext,
    floor: usize,
    layer_set: &str,
    scale: Scale,
    width_px: u32,
) -> (u32, u32, Vec<u8>) {
    let source = BoxSource::PlanView {
        floor,
        layer_set: layer_set.to_string(),
    };
    let (w, h) = source_size_in(&source, scale, cx);
    let mut b = LayoutBox::new(1, (Point::new(0.0, 0.0), Point::new(w, h)), source, scale);
    b.border = false;
    b.clip = false;
    let lines: Vec<_> = crate::render::render_box_lines(&b, cx)
        .into_iter()
        .map(|l| {
            let px = match l.weight {
                plan_elevation::LineWeight::Heavy => 2.0,
                _ => 1.0,
            };
            ((l.a.x, l.a.y), (l.b.x, l.b.y), px)
        })
        .collect();
    rasterize_lines(&lines, (0.0, 0.0, w, h), width_px)
}

// ------------------------------------------------- Scale to Fit, Check Plot --

/// Walls and dimension lines of floor `floor`: `(min, max)` plan inches. What
/// Scale to Fit fits to the sheet and Center Sheet centers it on.
pub fn plan_extent(project: &Project, floor: usize) -> Option<(Point, Point)> {
    project.floors.get(floor).and_then(plan_bounds)
}

/// Scale to Fit (File > Print): the largest drawing scale at which a plan of
/// the given extent fits inside a sheet of `sheet_in` inches less its
/// Drawing Margins `[top, bottom, left, right]`. Architectural scales are
/// tried first, largest to smallest, then the metric ratios 1:250, 1:500,
/// 1:1000 and 1:2000.
pub fn scale_to_fit(extent: (Point, Point), sheet_in: (f64, f64), margins: [f64; 4]) -> Scale {
    let (w, h) = (
        (extent.1.x - extent.0.x).abs().max(1.0),
        (extent.1.y - extent.0.y).abs().max(1.0),
    );
    let [top, bottom, left, right] = margins;
    let (aw, ah) = (
        (sheet_in.0 - left - right).max(1.0),
        (sheet_in.1 - top - bottom).max(1.0),
    );
    let mut s = Scale::ThreeInch;
    loop {
        let k = s.inches_per_foot() / 12.0;
        if w * k <= aw + 1e-9 && h * k <= ah + 1e-9 {
            return s;
        }
        let next = s.smaller_any().or_else(|| {
            matches!(s, Scale::OneInchEq20Ft)
                .then_some(Scale::Ratio(250))
                .or_else(|| matches!(s, Scale::Ratio(n) if n >= 1000).then_some(Scale::Ratio(2000)))
        });
        match next {
            Some(n) if n != s => s = n,
            _ => return s,
        }
    }
}

/// The middle of an extent: where Center Sheet puts the Drawing Sheet.
pub fn extent_center(extent: (Point, Point)) -> Point {
    Point::new(
        (extent.0.x + extent.1.x) * 0.5,
        (extent.0.y + extent.1.y) * 0.5,
    )
}

/// The plan inches a Drawing Sheet of `sheet_in` paper inches covers at
/// `scale` when its middle is at `center`: `[x0, y0, x1, y1]`.
pub fn drawing_sheet_window(center: Point, sheet_in: (f64, f64), scale: Scale) -> [f64; 4] {
    let k = 12.0 / scale.inches_per_foot();
    let (w, h) = (sheet_in.0 * k, sheet_in.1 * k);
    [
        center.x - w * 0.5,
        center.y - h * 0.5,
        center.x + w * 0.5,
        center.y + h * 0.5,
    ]
}

/// A window `(x0, y0, x1, y1)` in plan inches from two opposite corners.
pub fn window_of(a: Point, b: Point) -> [f64; 4] {
    [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]
}

/// Check Plot: the smallest paper that holds a sheet of `sheet_in` printed at
/// `fraction` of its size with the printing `margins` `[top, bottom, left,
/// right]`, in the sheet's own orientation. `papers` are `(name, (long side,
/// short side))`; the answer is the index of the smallest by area.
pub fn paper_for_check_plot(
    sheet_in: (f64, f64),
    fraction: f64,
    margins: [f64; 4],
    papers: &[(String, (f64, f64))],
) -> Option<usize> {
    let [top, bottom, left, right] = margins;
    let landscape = sheet_in.0 >= sheet_in.1;
    let (need_w, need_h) = (
        sheet_in.0 * fraction + left + right,
        sheet_in.1 * fraction + top + bottom,
    );
    papers
        .iter()
        .enumerate()
        .filter(|(_, (_, (long, short)))| {
            let (pw, ph) = if landscape {
                (*long, *short)
            } else {
                (*short, *long)
            };
            pw + 1e-6 >= need_w && ph + 1e-6 >= need_h
        })
        .min_by(|a, b| {
            let area = |p: &(String, (f64, f64))| p.1 .0 * p.1 .1;
            area(a.1).total_cmp(&area(b.1))
        })
        .map(|(i, _)| i)
}

/// `1/2` for a Check Plot fraction on the list, else the percentage.
pub fn check_plot_label(fraction: f64) -> String {
    CHECK_PLOT_FRACTIONS
        .iter()
        .find(|(f, _)| (f - fraction).abs() < 1e-6)
        .map_or_else(
            || format!("{:.0}%", fraction * 100.0),
            |(_, l)| (*l).to_string(),
        )
}

/// The information messages under the Print dialog's preview (manual p.
/// 1443): what the sheet and paper are, how big the print comes out and why a
/// result may not be what was meant. `sheet_in` is the sheet as it prints at
/// its own size (a plan view: the part of the plan at its drawing scale);
/// `pixels` the size of a Print Image, when it is one.
pub fn print_info(
    sheet_in: (f64, f64),
    opts: &PrintOptions,
    pixels: Option<(u32, u32)>,
) -> Vec<String> {
    let mut out = Vec::new();
    let (paper_w, paper_h) = opts.paper.inches(opts.landscape);
    let (print_w, print_h) = opts.printable_in();
    let s = print_scale_factor(opts, sheet_in);
    out.push(format!(
        "The sheet is {} x {} in; the paper is {} x {} in ({} x {} in printable).",
        fmt_in(sheet_in.0),
        fmt_in(sheet_in.1),
        fmt_in(paper_w),
        fmt_in(paper_h),
        fmt_in(print_w),
        fmt_in(print_h)
    ));
    match opts.scale {
        PrintScale::CheckPlot { fraction, .. } => out.push(format!(
            "Check plot at {}: the drawing and its line weights print at {:.0}% of their size.",
            check_plot_label(fraction),
            s * 100.0
        )),
        _ if s < 0.995 => out.push(format!(
            "The sheet prints at {:.0}% of its size, so the drawing is not to scale.",
            s * 100.0
        )),
        _ if s > 1.005 => out.push(format!(
            "The sheet prints enlarged to {:.0}% of its size; check the paper.",
            s * 100.0
        )),
        _ => {}
    }
    let (big_w, big_h) = (sheet_in.0 * s, sheet_in.1 * s);
    if opts.tiling {
        let g = tile_grid(sheet_in, s, (print_w, print_h), opts.overlap_in);
        if g.count() > 1 {
            out.push(format!(
                "The sheet takes {} pages ({} x {}); cut along the crop marks.",
                g.count(),
                g.cols,
                g.rows
            ));
        }
    } else if big_w > print_w + 0.01 || big_h > print_h + 0.01 {
        out.push(
            "The sheet is larger than the printable area and will be cut off: choose Fit to \
             Paper, a Check Plot or print across several pages."
                .to_string(),
        );
    }
    if let Some((w, h)) = pixels {
        let mp = f64::from(w) * f64::from(h) / 1.0e6;
        out.push(format!(
            "The picture is {w} x {h} pixels ({mp:.1} megapixels) at {} dpi.",
            opts.dpi
        ));
        if mp > 100.0 {
            out.push("That is a very large picture; printing it may be slow.".to_string());
        }
    } else if opts.dpi > 0 && opts.dpi < 150 {
        out.push(format!(
            "Pictures in the print are resolved at {} dpi; they may look coarse.",
            opts.dpi
        ));
    }
    out
}

fn fmt_in(v: f64) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}").trim_end_matches('0').to_string()
    }
}

// ------------------------------------------------- program-wide sheet sizes --

/// Customize Sheet Sizes, program-wide (manual p. 1430: Chief keeps the data
/// in one file in the program's Data folder, `sheetSizes.sheet`): the sizes
/// added to the standard list and the standard sizes left out of it. Plan
/// Studio keeps it in `~/.plan-studio/sheetsizes.json`.
///
/// Layouts used to keep their own (DECISIONS 63): [`SheetSizeFile::adopt_layout`]
/// takes those over, so a layout from before still finds its sizes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetSizeFile {
    pub version: u32,
    pub custom: Vec<CustomSheetSize>,
    pub hidden: Vec<SheetSize>,
}

impl SheetSizeFile {
    /// The file's name inside `~/.plan-studio`.
    pub const FILE_NAME: &'static str = "sheetsizes.json";
    pub const VERSION: u32 = 1;

    /// `~/.plan-studio/sheetsizes.json` (`HOME`, else `USERPROFILE`).
    pub fn default_path() -> Option<std::path::PathBuf> {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
        Some(
            std::path::PathBuf::from(home)
                .join(".plan-studio")
                .join(Self::FILE_NAME),
        )
    }

    /// Reads the file; a missing or damaged one is an empty list.
    pub fn load_from(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<SheetSizeFile>(&t).ok())
            .unwrap_or_default()
    }

    /// Writes the file, making its folder first.
    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut copy = self.clone();
        copy.version = Self::VERSION;
        let text = serde_json::to_string_pretty(&copy)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }

    /// The sizes the lists offer: the standard ones that are not hidden, then
    /// the custom ones. `keep` (the size in use) is never left out.
    pub fn choices(&self, keep: Option<SheetSize>) -> Vec<SheetChoice> {
        let mut out: Vec<SheetChoice> = SheetSize::ALL
            .into_iter()
            .filter(|s| !self.hidden.contains(s) || Some(*s) == keep)
            .map(SheetChoice::Standard)
            .collect();
        out.extend(self.custom.iter().cloned().map(SheetChoice::Custom));
        out
    }

    /// Takes over the custom sizes a layout kept for itself (same name and
    /// sides: already here). Returns whether the list grew.
    pub fn adopt_layout(&mut self, layout: &Layout) -> bool {
        self.adopt(&layout.custom_sizes)
    }

    /// Adds the sizes not already here (same name, ignoring case).
    pub fn adopt(&mut self, sizes: &[CustomSheetSize]) -> bool {
        let mut grew = false;
        for c in sizes {
            if c.problem().is_some() {
                continue;
            }
            let known = self
                .custom
                .iter()
                .any(|k| k.name.trim().eq_ignore_ascii_case(c.name.trim()));
            if !known {
                self.custom.push(c.clone());
                grew = true;
            }
        }
        grew
    }
}

struct SizeStore {
    file: SheetSizeFile,
    /// Where changes are written; `None` keeps them in memory.
    path: Option<std::path::PathBuf>,
}

thread_local! {
    static SIZES: std::cell::RefCell<Option<SizeStore>> = const { std::cell::RefCell::new(None) };
}

fn with_store<R>(f: impl FnOnce(&mut SizeStore) -> R) -> R {
    SIZES.with(|s| {
        let mut s = s.borrow_mut();
        let store = s.get_or_insert_with(|| {
            let path = SheetSizeFile::default_path();
            SizeStore {
                file: path
                    .as_deref()
                    .map(SheetSizeFile::load_from)
                    .unwrap_or_default(),
                path,
            }
        });
        f(store)
    })
}

/// The program-wide sheet sizes (read from `~/.plan-studio/sheetsizes.json`
/// on first use).
pub fn global_sheet_sizes() -> SheetSizeFile {
    with_store(|s| s.file.clone())
}

/// Makes `file` the program-wide sheet sizes and writes it, unless the store
/// was made in-memory ([`use_sheet_sizes`] with no path). Returns the error
/// text when the write failed.
pub fn set_global_sheet_sizes(file: SheetSizeFile) -> Result<(), String> {
    with_store(|s| {
        s.file = file;
        match &s.path {
            Some(p) => s.file.save_to(p).map_err(|e| e.to_string()),
            None => Ok(()),
        }
    })
}

/// Uses `file` as the program-wide sheet sizes, written to `path` when it is
/// given (tests pass `None`: nothing touches the user's folder). Replaces
/// what was loaded on this thread.
pub fn use_sheet_sizes(file: SheetSizeFile, path: Option<std::path::PathBuf>) {
    SIZES.with(|s| *s.borrow_mut() = Some(SizeStore { file, path }));
}

/// Takes over the custom sizes of `layout` into the program-wide list and
/// writes it when it grew. Returns whether it grew.
pub fn adopt_layout_sheet_sizes(layout: &Layout) -> bool {
    with_store(|s| {
        let grew = s.file.adopt_layout(layout);
        if grew {
            if let Some(p) = &s.path {
                let _ = s.file.save_to(p);
            }
        }
        grew
    })
}

/// The program-wide custom sizes as `(label, (long, short))` for the Print
/// dialog's paper list.
pub fn global_custom_papers() -> Vec<(String, (f64, f64))> {
    global_sheet_sizes()
        .custom
        .iter()
        .map(|c| (c.label(), c.inches()))
        .collect()
}

// ---------------------------------------------------------------------- tests --

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> (Point, Point) {
        (Point::ZERO, Point::new(w, h))
    }

    #[test]
    fn check_plot_scales_the_sheet_and_its_lines_by_the_fraction() {
        let mut o = PrintOptions {
            scale: PrintScale::CheckPlot {
                scale: Scale::QuarterInch,
                fraction: 0.5,
            },
            ..PrintOptions::default()
        };
        assert_eq!(print_scale_factor(&o, (36.0, 24.0)), 0.5);
        // The fraction is clamped: a check plot never enlarges.
        o.scale = PrintScale::CheckPlot {
            scale: Scale::QuarterInch,
            fraction: 2.0,
        };
        assert_eq!(print_scale_factor(&o, (36.0, 24.0)), 1.0);
        // A view sent at 1/4" = 1' prints at 1/8" = 1' at half scale: 0.25 * 0.5.
        let s = Scale::QuarterInch.inches_per_foot() * 0.5;
        assert_eq!(Scale::from_inches_per_foot(s), Scale::EighthInch);
        assert_eq!(check_plot_label(0.5), "1/2");
        assert_eq!(check_plot_label(0.6), "60%");
    }

    #[test]
    fn fit_to_paper_fills_the_set_percentage_of_the_printable_area() {
        let o = PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            margin_in: 0.0,
            scale: PrintScale::Fit,
            ..PrintOptions::default()
        };
        // 36 x 24 on 11 x 8.5: width limits, 11 / 36.
        let full = print_scale_factor(&o, (36.0, 24.0));
        assert!((full - 11.0 / 36.0).abs() < 1e-12);
        let o95 = PrintOptions {
            scale: PrintScale::FitPercent(DEFAULT_FIT_PERCENT),
            ..o.clone()
        };
        assert!((print_scale_factor(&o95, (36.0, 24.0)) - full * 0.95).abs() < 1e-12);
        let tiled = PrintOptions {
            tiling: true,
            ..o95
        };
        assert_eq!(print_scale_factor(&tiled, (36.0, 24.0)), 1.0);
    }

    #[test]
    fn scale_to_fit_picks_the_largest_scale_that_fits_the_sheet() {
        // A 60' x 40' plan on ARCH D (36 x 24) with 1/4" margins: 35.5 x 23.5 printable.
        let plan = rect(60.0 * 12.0, 40.0 * 12.0);
        let m = [0.25; 4];
        assert_eq!(scale_to_fit(plan, (36.0, 24.0), m), Scale::HalfInch);
        // 120' x 80': 3/4" is 90 x 60, 1/2" is 60 x 40, 1/4" is 30 x 20.
        let big = rect(120.0 * 12.0, 80.0 * 12.0);
        assert_eq!(scale_to_fit(big, (36.0, 24.0), m), Scale::QuarterInch);
        // A small house on a big sheet takes a large scale.
        let small = rect(10.0 * 12.0, 8.0 * 12.0);
        assert_eq!(scale_to_fit(small, (36.0, 24.0), m), Scale::OneAndHalfInch);
        // Margins count: a plan that just fits without them does not with them.
        let tight = rect(70.0 * 12.0, 40.0 * 12.0);
        assert_eq!(scale_to_fit(tight, (36.0, 24.0), [0.0; 4]), Scale::HalfInch);
        assert_eq!(
            scale_to_fit(tight, (36.0, 24.0), [3.0; 4]),
            Scale::QuarterInch
        );
        // A site larger than 1" = 20' falls to the ratios.
        let site = rect(2000.0 * 12.0, 1000.0 * 12.0);
        assert!(matches!(scale_to_fit(site, (36.0, 24.0), m), Scale::Ratio(n) if n >= 250));
        let c = extent_center(plan);
        assert_eq!((c.x, c.y), (360.0, 240.0));
    }

    #[test]
    fn the_drawing_sheet_covers_its_footprint_around_its_centre() {
        // ARCH D at 1/4" = 1' covers 144' x 96'.
        let w = drawing_sheet_window(Point::new(100.0, 50.0), (36.0, 24.0), Scale::QuarterInch);
        assert_eq!(
            w,
            [100.0 - 864.0, 50.0 - 576.0, 100.0 + 864.0, 50.0 + 576.0]
        );
        assert_eq!(
            window_of(Point::new(5.0, 9.0), Point::new(1.0, 2.0)),
            [1.0, 2.0, 5.0, 9.0]
        );
    }

    #[test]
    fn a_check_plot_picks_the_smallest_paper_that_holds_the_reduced_sheet() {
        let papers: Vec<(String, (f64, f64))> = [
            SheetSize::ArchD,
            SheetSize::ArchC,
            SheetSize::Letter,
            SheetSize::Tabloid,
            SheetSize::ArchB,
        ]
        .into_iter()
        .map(|z| (z.label().to_string(), z.inches()))
        .collect();
        // ARCH D at 1/2 is 18 x 12: ARCH B exactly, with no margin.
        let i = paper_for_check_plot((36.0, 24.0), 0.5, [0.0; 4], &papers).unwrap();
        assert_eq!(papers[i].0, SheetSize::ArchB.label());
        // With a quarter inch margin it no longer fits ARCH B: ARCH C.
        let i = paper_for_check_plot((36.0, 24.0), 0.5, [0.25; 4], &papers).unwrap();
        assert_eq!(papers[i].0, SheetSize::ArchC.label());
        // At 1/4 it is 9 x 6: Letter holds it.
        let i = paper_for_check_plot((36.0, 24.0), 0.25, [0.25; 4], &papers).unwrap();
        assert_eq!(papers[i].0, SheetSize::Letter.label());
        // A portrait sheet needs a portrait paper: 24 x 36 at 1/2 is 12 x 18.
        let i = paper_for_check_plot((24.0, 36.0), 0.5, [0.0; 4], &papers).unwrap();
        assert_eq!(papers[i].0, SheetSize::ArchB.label());
        // Nothing holds a full ARCH D but ARCH D.
        let i = paper_for_check_plot((36.0, 24.0), 1.0, [0.0; 4], &papers).unwrap();
        assert_eq!(papers[i].0, SheetSize::ArchD.label());
        assert!(paper_for_check_plot((72.0, 48.0), 1.0, [0.0; 4], &papers).is_none());
    }

    #[test]
    fn per_edge_margins_set_the_printable_area() {
        let o = PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            margin_in: 0.25,
            ..PrintOptions::default()
        };
        assert_eq!(o.margins(), [0.25; 4]);
        assert_eq!(o.printable_in(), (10.5, 8.0));
        assert_eq!(o.printable_rect_in(), [0.25, 0.25, 10.75, 8.25]);
        // Top, bottom, left, right.
        let o = PrintOptions {
            edge_margins_in: Some([0.5, 1.0, 0.25, 0.75]),
            ..o
        };
        assert_eq!(o.printable_in(), (10.0, 7.0));
        assert_eq!(o.printable_rect_in(), [0.25, 1.0, 10.25, 8.0]);
    }

    #[test]
    fn info_messages_explain_the_result() {
        let letter = PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            margin_in: 0.0,
            scale: PrintScale::FitPercent(95.0),
            ..PrintOptions::default()
        };
        let m = print_info((36.0, 24.0), &letter, None);
        assert!(
            m[0].contains("36 x 24 in") && m[0].contains("11 x 8.5 in"),
            "{m:?}"
        );
        assert!(m.iter().any(|t| t.contains("not to scale")), "{m:?}");
        let chk = PrintOptions {
            scale: PrintScale::CheckPlot {
                scale: Scale::QuarterInch,
                fraction: 0.5,
            },
            paper: PaperSize::Standard(SheetSize::ArchB),
            ..letter.clone()
        };
        let m = print_info((36.0, 24.0), &chk, None);
        assert!(
            m.iter()
                .any(|t| t.contains("Check plot at 1/2") && t.contains("50%")),
            "{m:?}"
        );
        assert!(
            !m.iter().any(|t| t.contains("cut off")),
            "an 18 x 12 sheet fits ARCH B"
        );
        // Actual size on small paper is cut off, unless it is tiled.
        let cut = PrintOptions {
            scale: PrintScale::Actual,
            ..letter.clone()
        };
        assert!(print_info((36.0, 24.0), &cut, None)
            .iter()
            .any(|t| t.contains("cut off")));
        let tiled = PrintOptions {
            tiling: true,
            ..cut
        };
        assert!(print_info((36.0, 24.0), &tiled, None)
            .iter()
            .any(|t| t.contains("takes") && t.contains("pages")));
        let m = print_info((10.0, 8.0), &letter, Some((3000, 2400)));
        assert!(m
            .iter()
            .any(|t| t.contains("3000 x 2400 pixels") && t.contains("7.2 megapixels")));
    }

    #[test]
    fn the_sheet_size_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("ps-sheetsizes-{}", std::process::id()));
        let path = dir.join("nested").join(SheetSizeFile::FILE_NAME);
        let mut f = SheetSizeFile::default();
        f.custom.push(CustomSheetSize::new("Poster", 30.0, 40.0));
        f.hidden = vec![SheetSize::IsoA0, SheetSize::AnsiE];
        f.save_to(&path).unwrap();
        let back = SheetSizeFile::load_from(&path);
        assert_eq!(back.custom, f.custom);
        assert_eq!(back.hidden, f.hidden);
        assert_eq!(back.version, SheetSizeFile::VERSION);
        // The lists: hidden standard sizes are gone, custom ones follow.
        let all = back.choices(None);
        assert_eq!(all.len(), SheetSize::ALL.len() - 2 + 1);
        assert!(!all.contains(&SheetChoice::Standard(SheetSize::IsoA0)));
        assert!(matches!(all.last(), Some(SheetChoice::Custom(c)) if c.name == "Poster"));
        // The size in use stays even when hidden.
        assert!(back
            .choices(Some(SheetSize::IsoA0))
            .contains(&SheetChoice::Standard(SheetSize::IsoA0)));
        // A missing or damaged file is an empty list.
        assert_eq!(
            SheetSizeFile::load_from(&dir.join("nope.json")),
            SheetSizeFile::default()
        );
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(SheetSizeFile::load_from(&path), SheetSizeFile::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sizes_a_layout_kept_for_itself_are_taken_over() {
        // The JSON of a layout saved before sheet sizes were program-wide.
        let mut old = Layout::new("Old", SheetSize::ArchC);
        old.custom_sizes = vec![
            CustomSheetSize::new("Poster", 30.0, 40.0),
            CustomSheetSize::new("Banner", 12.0, 60.0),
            CustomSheetSize::new("Bad", 1.0, 10.0),
        ];
        let json = serde_json::to_string(&old).unwrap();
        let loaded: Layout = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded.custom_sizes.len(),
            3,
            "old per-layout sizes still load"
        );
        use_sheet_sizes(SheetSizeFile::default(), None);
        assert!(adopt_layout_sheet_sizes(&loaded));
        let names: Vec<_> = global_sheet_sizes()
            .custom
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(
            names,
            ["Poster", "Banner"],
            "the invalid size is left behind"
        );
        // Doing it again changes nothing, and a same-named size is not doubled.
        assert!(!adopt_layout_sheet_sizes(&loaded));
        let mut other = Layout::new("Other", SheetSize::ArchC);
        other.custom_sizes = vec![CustomSheetSize::new("poster", 10.0, 10.0)];
        assert!(!adopt_layout_sheet_sizes(&other));
        // The Print dialog's paper list.
        let papers = global_custom_papers();
        assert_eq!(papers.len(), 2);
        assert_eq!(papers[0].1, (40.0, 30.0));
    }

    #[test]
    fn set_global_sheet_sizes_writes_only_when_a_path_is_set() {
        use_sheet_sizes(SheetSizeFile::default(), None);
        let mut f = SheetSizeFile::default();
        f.custom.push(CustomSheetSize::new("Poster", 30.0, 40.0));
        assert!(set_global_sheet_sizes(f.clone()).is_ok());
        assert_eq!(global_sheet_sizes(), f);
        let path = std::env::temp_dir()
            .join(format!("ps-sheetsizes-w-{}", std::process::id()))
            .join(SheetSizeFile::FILE_NAME);
        use_sheet_sizes(SheetSizeFile::default(), Some(path.clone()));
        set_global_sheet_sizes(f.clone()).unwrap();
        assert_eq!(SheetSizeFile::load_from(&path).custom, f.custom);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
        use_sheet_sizes(SheetSizeFile::default(), None);
    }

    // ----------------------------------------------- printing, end to end --

    use crate::tests::two_room_house;
    use plan_core::watermark::{WatermarkLayout, WatermarkSpec};

    fn text_of(pdf: &[u8]) -> String {
        pdf.iter().map(|&b| b as char).collect()
    }

    fn count(pdf: &[u8], needle: &str) -> usize {
        let n = needle.as_bytes();
        pdf.windows(n.len()).filter(|w| *w == n).count()
    }

    /// A one-sheet layout, ARCH D, with a plan view on it.
    fn arch_d_layout(p: &Project) -> Layout {
        let mut l = crate::default_construction_set(p, 1);
        l.sheet = SheetSize::ArchD;
        l
    }

    fn arch_d_on_arch_d() -> PrintOptions {
        PrintOptions {
            paper: PaperSize::Standard(SheetSize::ArchD),
            landscape: true,
            scale: PrintScale::Actual,
            margin_in: 0.0,
            ..PrintOptions::default()
        }
    }

    fn marks(pages: &[PreviewPage]) -> Vec<PreviewMark> {
        pages
            .iter()
            .flat_map(|pg| pg.items.iter())
            .filter_map(|i| match i {
                PreviewItem::Mark(m) => Some(m.clone()),
                _ => None,
            })
            .collect()
    }

    fn watermarked(spec: WatermarkSpec) -> Project {
        let mut p = two_room_house();
        p.print_setup.watermark.spec = spec;
        p
    }

    #[test]
    fn the_watermark_prints_with_opacity_and_turn_only_when_included() {
        let p = watermarked(WatermarkSpec {
            layout: WatermarkLayout::Tile,
            marks_per_row: 3,
            marks_per_column: 3,
            angle_deg: 30.0,
            transparency: 70.0,
            text: "DRAFT".into(),
            ..WatermarkSpec::default()
        });
        let cx = LayoutRenderContext::new(&p);
        let l = arch_d_layout(&p);
        let pages = l.content_pages().len();
        let off = print_layout_pdf(&l, &cx, &arch_d_on_arch_d());
        assert_eq!(count(&off, "(DRAFT) Tj"), 0);
        let on = print_layout_pdf(
            &l,
            &cx,
            &PrintOptions {
                watermark: true,
                ..arch_d_on_arch_d()
            },
        );
        // Nine marks on every printed sheet, at 30% opacity, turned 30 degrees.
        assert_eq!(count(&on, "(DRAFT) Tj"), 9 * pages);
        assert!(count(&on, "/GSA30 gs") >= pages);
        assert!(text_of(&on).contains("/GSA30 << /Type/ExtGState /ca 0.3 /CA 0.3 >>"));
        assert!(text_of(&on).contains("0.866 0.5 -0.5 0.866"));
        // A watermark with no words prints nothing.
        let empty = watermarked(WatermarkSpec {
            text: " ".into(),
            ..WatermarkSpec::default()
        });
        let cx = LayoutRenderContext::new(&empty);
        let none = print_layout_pdf(
            &arch_d_layout(&empty),
            &cx,
            &PrintOptions {
                watermark: true,
                ..arch_d_on_arch_d()
            },
        );
        assert_eq!(count(&none, "/GSA"), 0);
    }

    #[test]
    fn preview_marks_sit_where_the_placement_puts_them() {
        let p = watermarked(WatermarkSpec {
            layout: WatermarkLayout::Tile,
            marks_per_row: 3,
            marks_per_column: 3,
            angle_deg: 30.0,
            use_sheet_margin: true,
            ..WatermarkSpec::default()
        });
        let cx = LayoutRenderContext::new(&p);
        let l = arch_d_layout(&p);
        let opts = PrintOptions {
            watermark: true,
            range: Some((1, 1)),
            ..arch_d_on_arch_d()
        };
        let m = marks(&layout_print_preview(&l, &cx, &opts));
        assert_eq!(m.len(), 9);
        // The area is the sheet inside the layout's margin; cells are a third of it.
        let (w, h) = l.sheet_inches();
        let mg = l.margins_in;
        let (cw, ch) = ((w - 2.0 * mg) / 3.0, (h - 2.0 * mg) / 3.0);
        assert!(
            (m[0].cx - (mg + cw * 0.5) * 72.0).abs() < 1e-6,
            "{}",
            m[0].cx
        );
        assert!((m[0].cy - (h - mg - ch * 0.5) * 72.0).abs() < 1e-6);
        assert!((m[8].cx - (w - mg - cw * 0.5) * 72.0).abs() < 1e-6);
        assert!((m[8].cy - (mg + ch * 0.5) * 72.0).abs() < 1e-6);
        assert!(m
            .iter()
            .all(|k| (k.angle - 30f64.to_radians()).abs() < 1e-12));
        assert!(m.iter().all(|k| (k.alpha - 0.3).abs() < 1e-9));
        // The text is 0.5 inch tall in a capital A: a 50 pt font.
        let size = match &m[0].kind {
            PreviewMarkKind::Text { size_pt, .. } => *size_pt,
            PreviewMarkKind::Image { .. } => panic!("text"),
        };
        assert!((size - 0.5 * 72.0 / 0.718).abs() < 1e-6);
        // Shown at half size on smaller paper, positions and sizes follow.
        let half = layout_print_preview(
            &l,
            &cx,
            &PrintOptions {
                scale: PrintScale::Percent(50.0),
                paper: PaperSize::Standard(SheetSize::ArchB),
                ..opts.clone()
            },
        );
        let h2 = marks(&half);
        assert!((h2[0].w - m[0].w * 0.5).abs() < 1e-6);
        // In grayscale the mark's colour loses its hue.
        let g = marks(&layout_print_preview(
            &l,
            &cx,
            &PrintOptions {
                color: PrintColor::Grayscale,
                ..opts
            },
        ));
        assert!(g[0].color[0] == g[0].color[1] && g[0].color[1] == g[0].color[2]);
    }

    #[test]
    fn border_and_fit_layouts_make_the_marks_they_promise() {
        let count_marks = |layout, nx, ny| {
            let p = watermarked(WatermarkSpec {
                layout,
                marks_per_row: nx,
                marks_per_column: ny,
                ..WatermarkSpec::default()
            });
            let cx = LayoutRenderContext::new(&p);
            let l = arch_d_layout(&p);
            let o = PrintOptions {
                watermark: true,
                range: Some((1, 1)),
                ..arch_d_on_arch_d()
            };
            marks(&layout_print_preview(&l, &cx, &o))
        };
        assert_eq!(count_marks(WatermarkLayout::Border, 4, 3).len(), 4 + 4 + 2);
        let fit = count_marks(WatermarkLayout::FitToSheet, 4, 3);
        assert_eq!(fit.len(), 1);
        // The one mark is grown to fill the sheet inside the margins.
        assert!(fit[0].w > 400.0, "{}", fit[0].w);
    }

    #[test]
    fn a_picture_watermark_prints_with_a_soft_mask_and_the_set_turn() {
        let p = watermarked(WatermarkSpec {
            kind: WatermarkKind::Image,
            image_path: "logo.png".into(),
            layout: WatermarkLayout::Tile,
            marks_per_row: 2,
            marks_per_column: 1,
            image_ratio: 0.2,
            angle_deg: 90.0,
            ..WatermarkSpec::default()
        });
        let cx = LayoutRenderContext::new(&p).with_picture_loader(|name| {
            (name == "logo.png").then(|| crate::PerspectiveImage {
                width: 2,
                height: 1,
                rgba: vec![255, 0, 0, 255, 0, 0, 255, 100],
            })
        });
        let l = arch_d_layout(&p);
        let o = PrintOptions {
            watermark: true,
            range: Some((1, 1)),
            ..arch_d_on_arch_d()
        };
        let pdf = print_layout_pdf(&l, &cx, &o);
        assert!(
            text_of(&pdf).contains("/SMask "),
            "the transparent pixel needs a mask"
        );
        // Two marks, turned a quarter: the x axis of the picture points up.
        let m = marks(&layout_print_preview(&l, &cx, &o));
        assert_eq!(m.len(), 2);
        assert!(matches!(
            m[0].kind,
            PreviewMarkKind::Image { px: (2, 1), .. }
        ));
        // 20% of the area's width, 2:1.
        let area_w = l.sheet_inches().0 - 2.0 * l.margins_in;
        assert!((m[0].w - area_w * 0.2 * 72.0).abs() < 1e-6);
        assert!((m[0].h - m[0].w * 0.5).abs() < 1e-6);
        // A picture that cannot be read prints no mark.
        let blind = LayoutRenderContext::new(&p);
        assert!(marks(&layout_print_preview(&l, &blind, &o)).is_empty());
    }

    fn widths(pdf: &[u8]) -> std::collections::BTreeSet<i64> {
        let t = text_of(pdf);
        let mut set = std::collections::BTreeSet::new();
        for part in t.split(" w ") {
            if let Some(tok) = part.rsplit(' ').next() {
                if let Ok(v) = tok.parse::<f64>() {
                    set.insert((v * 1000.0).round() as i64);
                }
            }
        }
        set
    }

    #[test]
    fn line_weight_scale_single_weight_and_exact_weights() {
        let p = two_room_house();
        let cx = LayoutRenderContext::new(&p);
        let l = arch_d_layout(&p);
        let base = widths(&print_layout_pdf(&l, &cx, &arch_d_on_arch_d()));
        assert!(base.len() > 2);
        // A scale of 1 = 1/50 mm doubles every weight.
        let double = widths(&print_layout_pdf(
            &l,
            &cx,
            &PrintOptions {
                pens: PenSetup {
                    scale_factor: 2.0,
                    ..PenSetup::default()
                },
                ..arch_d_on_arch_d()
            },
        ));
        let (mb, md) = (*base.iter().max().unwrap(), *double.iter().max().unwrap());
        assert!((md - 2 * mb).abs() <= 2, "{double:?} vs {base:?}");
        // One weight for all lines: every stroke is 1/300 inch.
        let single = widths(&print_layout_pdf(
            &l,
            &cx,
            &PrintOptions {
                pens: PenSetup {
                    single_weight: true,
                    ..PenSetup::default()
                },
                ..arch_d_on_arch_d()
            },
        ));
        // 0.24 pt, and the 0.5 pt a page starts with.
        assert_eq!(single, [240, 500].into_iter().collect(), "{single:?}");
        // The hairline rule still wins over the single weight.
        let hair = widths(&print_layout_pdf(
            &l,
            &cx,
            &PrintOptions {
                line_weights: false,
                pens: PenSetup {
                    single_weight: true,
                    ..PenSetup::default()
                },
                ..arch_d_on_arch_d()
            },
        ));
        assert_eq!(hair, [500].into_iter().collect(), "{hair:?}");
        // Printed at half size, pens shrink with the sheet unless the weights are exact.
        let half = PrintOptions {
            scale: PrintScale::Percent(50.0),
            ..arch_d_on_arch_d()
        };
        let shrunk = widths(&print_layout_pdf(&l, &cx, &half));
        let exact = widths(&print_layout_pdf(
            &l,
            &cx,
            &PrintOptions {
                pens: PenSetup {
                    exact: true,
                    ..PenSetup::default()
                },
                ..half.clone()
            },
        ));
        // The stream holds the width before the 0.5 transform: exact ones are twice as wide.
        assert!(
            exact.iter().max() > shrunk.iter().max(),
            "{exact:?} vs {shrunk:?}"
        );
        // A Check Plot shrinks lines with the drawing even when weights are exact.
        let check = widths(&print_layout_pdf(
            &l,
            &cx,
            &PrintOptions {
                scale: PrintScale::CheckPlot {
                    scale: Scale::QuarterInch,
                    fraction: 0.5,
                },
                pens: PenSetup {
                    exact: true,
                    ..PenSetup::default()
                },
                ..arch_d_on_arch_d()
            },
        ));
        assert_eq!(check, shrunk);
    }

    #[test]
    fn per_edge_margins_clip_the_page_to_the_printable_area() {
        let p = two_room_house();
        let cx = LayoutRenderContext::new(&p);
        let l = arch_d_layout(&p);
        let o = PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            landscape: true,
            scale: PrintScale::Fit,
            edge_margins_in: Some([0.5, 1.0, 0.25, 0.75]),
            range: Some((1, 1)),
            ..PrintOptions::default()
        };
        let pdf = print_layout_pdf(&l, &cx, &o);
        // Left 0.25, bottom 1.0 inch, 10 x 7 inches printable.
        assert!(
            text_of(&pdf).contains("18 72 720 504 re"),
            "{}",
            &text_of(&pdf)[..200.min(text_of(&pdf).len())]
        );
        // The sheet fits that area: 10 / 36 and 7 / 24 -> 0.27778.
        let k = (10.0f64 / 36.0).min(7.0 / 24.0);
        assert!(text_of(&pdf).contains(&format!("{k:.5}")[..7]), "{k}");
        // The preview carries the same printable area.
        let pages = layout_print_preview(&l, &cx, &o);
        assert_eq!(pages[0].printable_pt, [18.0, 72.0, 738.0, 576.0]);
    }

    #[test]
    fn check_plot_prints_a_large_sheet_small_on_smaller_paper() {
        let p = two_room_house();
        let cx = LayoutRenderContext::new(&p);
        let l = arch_d_layout(&p);
        let check = |paper: SheetSize, tiling: bool| {
            let pdf = print_layout_pdf(
                &l,
                &cx,
                &PrintOptions {
                    paper: PaperSize::Standard(paper),
                    landscape: true,
                    margin_in: 0.0,
                    scale: PrintScale::CheckPlot {
                        scale: Scale::QuarterInch,
                        fraction: 0.5,
                    },
                    tiling,
                    range: Some((1, 1)),
                    ..PrintOptions::default()
                },
            );
            (count(&pdf, "/Type /Page "), text_of(&pdf))
        };
        // ARCH D at 1/2 is 18 x 12: one ARCH B page.
        let (pages, t) = check(SheetSize::ArchB, false);
        assert_eq!(pages, 1);
        assert!(t.contains("/MediaBox [0 0 1296 864]"));
        assert!(t.contains("0.5 0 0 0.5"));
        // On Letter it needs four pages cut along the marks.
        let (pages, t) = check(SheetSize::Letter, true);
        assert_eq!(pages, 4);
        assert!(t.contains("/MediaBox [0 0 792 612]"));
        // Full size on ARCH D is one page of 36 x 24.
        let pdf = print_layout_pdf(&l, &cx, &arch_d_on_arch_d());
        assert!(text_of(&pdf).contains("/MediaBox [0 0 2592 1728]"));
    }

    #[test]
    fn a_plan_window_prints_only_that_part_at_the_drawing_scale() {
        let p = two_room_house();
        let cx = LayoutRenderContext::new(&p);
        let base = PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            landscape: true,
            scale: PrintScale::Drawing(Scale::QuarterInch),
            margin_in: 0.0,
            ..PrintOptions::default()
        };
        // 480 x 240 plan inches at 1/4" = 1' is 10 x 5 inches of paper.
        let windowed = PrintOptions {
            plan_window: Some([0.0, 0.0, 480.0, 240.0]),
            ..base.clone()
        };
        let (pages, scale) = plan_view_print_preview(&cx, 0, "All", "WIN", &windowed);
        assert_eq!(scale, Scale::QuarterInch);
        assert_eq!(pages.len(), 1);
        let strip = pages[0]
            .items
            .iter()
            .find_map(|i| match i {
                PreviewItem::Stroke { pts, width_pt, .. }
                    if pts.len() == 2 && (width_pt - 0.25).abs() < 1e-9 =>
                {
                    Some(pts.clone())
                }
                _ => None,
            })
            .expect("the note strip's rule");
        assert!((strip[1].0 - strip[0].0 - 720.0).abs() < 1e-6, "{strip:?}");
        // The whole plan is a bigger sheet with more in it.
        let (whole, _) = plan_view_print_preview(&cx, 0, "All", "ALL", &base);
        let strokes = |pg: &PreviewPage| {
            pg.items
                .iter()
                .filter(|i| matches!(i, PreviewItem::Stroke { .. }))
                .count()
        };
        assert!(strokes(&whole[0]) > 0);
        // Fit to Paper fits the window, not the plan.
        let fit = PrintOptions {
            scale: PrintScale::FitPercent(100.0),
            plan_window: Some([0.0, 0.0, 120.0, 60.0]),
            ..base.clone()
        };
        let (_, s) = plan_view_print_preview(&cx, 0, "All", "F", &fit);
        let whole_fit = PrintOptions {
            plan_window: None,
            ..fit
        };
        let (_, s_whole) = plan_view_print_preview(&cx, 0, "All", "F", &whole_fit);
        assert!(
            s.inches_per_foot() > s_whole.inches_per_foot(),
            "{s:?} {s_whole:?}"
        );
        // The part that is printed carries the watermark over its own sheet.
        let mut p2 = two_room_house();
        p2.print_setup.watermark.spec.layout = WatermarkLayout::FitToSheet;
        let cx2 = LayoutRenderContext::new(&p2);
        let (wm, _) = plan_view_print_preview(
            &cx2,
            0,
            "All",
            "WM",
            &PrintOptions {
                watermark: true,
                ..windowed
            },
        );
        let m = marks(&wm);
        assert_eq!(m.len(), 1);
        // Fit to Sheet: centred on the 10 x 5.3 inch sheet, which sits in the
        // middle of the 11 inch Letter page.
        assert!((m[0].cx - 396.0).abs() < 1.0, "{}", m[0].cx);
    }

    #[test]
    fn fit_to_paper_on_a_plan_leaves_the_set_share_of_the_paper() {
        let p = two_room_house();
        let cx = LayoutRenderContext::new(&p);
        let o = |fit| PrintOptions {
            paper: PaperSize::Standard(SheetSize::Letter),
            landscape: true,
            margin_in: 0.0,
            scale: fit,
            ..PrintOptions::default()
        };
        let (_, full) = print_plan_view_pdf(&cx, 0, "All", "T", &o(PrintScale::Fit));
        let (_, part) = print_plan_view_pdf(&cx, 0, "All", "T", &o(PrintScale::FitPercent(50.0)));
        assert!(
            part.inches_per_foot() < full.inches_per_foot(),
            "{part:?} {full:?}"
        );
        let (_, same) = print_plan_view_pdf(&cx, 0, "All", "T", &o(PrintScale::FitPercent(100.0)));
        assert_eq!(same, full);
    }
}
