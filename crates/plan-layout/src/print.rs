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

use crate::canvas::{emit, Canvas, Pen, Prim, BLACK};
use crate::extent::{source_size_in, SceneSource};
use crate::model::{BoxSource, Layout, LayoutBox};
use crate::render::{box_prims, draw_page, LayoutRenderContext};
use plan_core::Point;
use plan_docs::{PdfColor, PdfColorMode, PdfDoc, Scale, SheetSize, CHIEF_SHEET_BACKGROUND};

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
}

/// Colour handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
        }
    }
}

impl PrintOptions {
    /// `(width, height)` of the printable area of the paper, inches.
    pub fn printable_in(&self) -> (f64, f64) {
        let (w, h) = self.paper.inches(self.landscape);
        (
            (w - 2.0 * self.margin_in).max(0.5),
            (h - 2.0 * self.margin_in).max(0.5),
        )
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
}

fn scale_factor(opts: &PrintOptions, sheet: (f64, f64)) -> f64 {
    let (pw, ph) = opts.printable_in();
    match opts.scale {
        PrintScale::Fit if opts.tiling => 1.0,
        PrintScale::Fit => (pw / sheet.0).min(ph / sheet.1),
        PrintScale::Percent(p) => (p / 100.0).max(0.01),
        PrintScale::Actual | PrintScale::Drawing(_) | PrintScale::Ratio(_) => 1.0,
    }
}

/// Puts `sheets` on paper pages of `opts`, one page or one tile grid each.
fn paginate(sheets: &[Sheet], opts: &PrintOptions) -> Vec<u8> {
    let (pw, ph) = opts.paper.inches(opts.landscape);
    let mut doc = PdfDoc::new(pw * 72.0, ph * 72.0);
    doc.set_color_mode(opts.color.into());
    let m = opts.margin_in;
    let (print_w, print_h) = opts.printable_in();
    let mut first_page = true;
    for sheet in sheets {
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
        let mut tile_no = 0;
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                tile_no += 1;
                if !first_page {
                    doc.new_page();
                }
                first_page = false;
                if tile_no == 1 {
                    doc.add_bookmark(&sheet.bookmark);
                }
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
                let e = (m - x0) * 72.0;
                // Content y runs up from the bottom of the printed sheet.
                let f = (m - (big_h - y_top - print_h)) * 72.0;
                doc.set_uniform_line_width(None);
                doc.save_state();
                doc.clip_rect(m * 72.0, m * 72.0, print_w * 72.0, print_h * 72.0);
                doc.transform([s, 0.0, 0.0, s, e, f]);
                if !opts.line_weights {
                    doc.set_uniform_line_width(Some(0.5 / s));
                }
                if let Some(bg) = sheet.background {
                    doc.fill_rect(0.0, 0.0, sheet.w_in * 72.0, sheet.h_in * 72.0, bg);
                }
                emit(&mut doc, &sheet.prims);
                doc.restore_state();
                doc.set_uniform_line_width(None);
                if opts.tiling && n > 1 {
                    tile_marks(
                        &mut doc,
                        opts,
                        (col, row),
                        (grid.cols, grid.rows),
                        tile_no,
                        n,
                    );
                }
            }
        }
    }
    doc.finish()
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
    let m = opts.margin_in * 72.0;
    let (pw, ph) = opts.paper.inches(opts.landscape);
    let (x1, y1) = (pw * 72.0 - m, ph * 72.0 - m);
    let arm = (m * 0.8).max(6.0);
    doc.set_gray(0.0);
    for (x, y, dx, dy) in [
        (m, m, 1.0, 1.0),
        (x1, m, -1.0, 1.0),
        (m, y1, 1.0, -1.0),
        (x1, y1, -1.0, -1.0),
    ] {
        doc.line(x, y, x + dx * arm, y, 0.4);
        doc.line(x, y, x, y + dy * arm, 0.4);
    }
    doc.text(
        m + arm + 4.0,
        (m * 0.35).max(2.0),
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
    let (w_in, h_in) = layout.sheet_inches();
    let scenes = SceneSource::new(cx.scene);
    let background = layout.page_background.then_some(PdfColor::Rgb(
        CHIEF_SHEET_BACKGROUND.0,
        CHIEF_SHEET_BACKGROUND.1,
        CHIEF_SHEET_BACKGROUND.2,
    ));
    let pages = layout.content_pages();
    let indices = selected((0..pages.len()).collect::<Vec<_>>(), opts.range);
    let mut sheets = Vec::new();
    for index in indices {
        let mut cv = Canvas::new();
        draw_page(&mut cv, layout, &pages, index, cx, &scenes);
        sheets.push(Sheet {
            w_in,
            h_in,
            prims: cv.prims,
            background,
            bookmark: format!("{} {}", pages[index].sheet_number(), pages[index].title),
        });
    }
    if sheets.is_empty() {
        sheets.push(Sheet {
            w_in,
            h_in,
            prims: Vec::new(),
            background,
            bookmark: layout.name.clone(),
        });
    }
    paginate(&sheets, opts)
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
        PrintScale::Ratio(n) => Scale::Ratio(n.round().clamp(1.0, 10_000.0) as u32),
        PrintScale::Actual => Scale::Ratio(1),
        PrintScale::Percent(p) => {
            Scale::Ratio((100.0 / p.max(0.1)).round().clamp(1.0, 10_000.0) as u32)
        }
        PrintScale::Fit if opts.tiling => Scale::QuarterInch,
        PrintScale::Fit => {
            let (pw, ph) = opts.printable_in();
            let mut s = Scale::ThreeInch;
            loop {
                let (w, h) = source_size_in(source, s, cx);
                if w <= pw + 1e-9 && h <= ph + 1e-9 {
                    return s;
                }
                match s.smaller_any() {
                    Some(next) => s = next,
                    None => return s,
                }
            }
        }
    }
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
        source,
        scale,
    );
    b.border = false;
    b.clip = false;
    let scenes = SceneSource::new(cx.scene);
    let mut prims = box_prims(&b, cx, &scenes, &crate::layers::LayoutLayers::default());
    let mut cv = Canvas::new();
    cv.text(
        4.0,
        6.0,
        8.0,
        BLACK,
        &format!("{title}   SCALE: {}", scale.label()),
    );
    cv.line((0.0, 1.0), (w * 72.0, 1.0), Pen::gray(0.25, 0.7));
    prims.extend(cv.prims);
    let sheet = Sheet {
        w_in: w,
        h_in: h + note_h,
        prims,
        background: None,
        bookmark: title.to_string(),
    };
    // A plan view prints at its drawing scale: the sheet is not scaled again.
    let mut o = opts.clone();
    o.scale = PrintScale::Actual;
    (paginate(&[sheet], &o), scale)
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
    let scenes = SceneSource::new(cx.scene);
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
