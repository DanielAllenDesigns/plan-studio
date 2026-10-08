//! A minimal, dependency-free PDF 1.4 writer and the plan-sheet renderer.
//!
//! [`PdfDoc`] collects drawing operators per page (uncompressed content
//! streams; Helvetica and Helvetica-Bold font resources; optional embedded
//! `/DeviceRGB` images) and [`PdfDoc::finish`] serialises them with a correct
//! cross-reference table. Pages may have different sizes. Coordinates are PDF
//! points (1/72"), origin at the bottom-left, Y up, which matches plan space.
//!
//! [`plan_sheet`] builds on it to print a floor at an architectural scale.

mod scale;
mod sheet;

#[cfg(test)]
mod mode_tests;

pub use scale::{Scale, SheetSize};
pub use sheet::{
    plan_sheet, plan_sheet_with, PlanSheetOptions, PlanSheetResult, RoomAreaBasis, TitleBlock,
    CHIEF_SHEET_BACKGROUND,
};

use std::fmt::Write as _;

/// Helvetica advance widths (1/1000 em) for ASCII 32..=126.
const HELVETICA_WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // space .. /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // 0-9
    278, 278, 584, 584, 584, 556, 1015, // : .. @
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, // A-M
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // N-Z
    278, 278, 278, 469, 556, 333, // [ \ ] ^ _ `
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, // a-m
    556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // n-z
    334, 260, 334, 584, // { | } ~
];

/// Helvetica-Bold advance widths (1/1000 em) for ASCII 32..=126.
const HELVETICA_BOLD_WIDTHS: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278,
    278, // space .. /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // 0-9
    333, 333, 584, 584, 584, 611, 975, // : .. @
    722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, // A-M
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // N-Z
    333, 278, 333, 584, 556, 333, // [ \ ] ^ _ `
    556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, // a-m
    611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, // n-z
    389, 280, 389, 584, // { | } ~
];

/// Format a number for a content stream: up to 3 decimals, no trailing zeros.
fn num(v: f64) -> String {
    let mut s = format!("{v:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// Like [`num`] with up to 6 decimals, for matrix entries (a scale of 0.4375
/// must not become 0.438).
fn num6(v: f64) -> String {
    let mut s = format!("{v:.6}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// Map a char to its WinAnsi byte (`?` when unrepresentable).
fn winansi(c: char) -> u8 {
    match c {
        '\u{20}'..='\u{7e}' | '\u{a0}'..='\u{ff}' => c as u32 as u8,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201c}' => 0x93,
        '\u{201d}' => 0x94,
        '\u{2026}' => 0x85,
        '\u{20ac}' => 0x80,
        _ => b'?',
    }
}

/// Escape a string as a PDF literal string body (without the parentheses).
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.chars().map(winansi) {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7e => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out
}

/// A fill or stroke colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfColor {
    /// Gray level, 0 = black, 1 = white.
    Gray(f64),
    /// 8-bit RGB.
    Rgb(u8, u8, u8),
}

impl PdfColor {
    /// Opaque white.
    pub const WHITE: PdfColor = PdfColor::Gray(1.0);
    /// Opaque black.
    pub const BLACK: PdfColor = PdfColor::Gray(0.0);

    fn components(self) -> String {
        match self {
            PdfColor::Gray(g) => num(g.clamp(0.0, 1.0)),
            PdfColor::Rgb(r, g, b) => format!(
                "{} {} {}",
                num(f64::from(r) / 255.0),
                num(f64::from(g) / 255.0),
                num(f64::from(b) / 255.0)
            ),
        }
    }

    /// The non-stroking (fill) colour operator, e.g. `0.5 g` or `1 0 0 rg`.
    fn fill_op(self) -> String {
        let op = if matches!(self, PdfColor::Gray(_)) {
            "g"
        } else {
            "rg"
        };
        format!("{} {op}", self.components())
    }

    /// The stroking colour operator, e.g. `0.5 G` or `1 0 0 RG`.
    fn stroke_op(self) -> String {
        let op = if matches!(self, PdfColor::Gray(_)) {
            "G"
        } else {
            "RG"
        };
        format!("{} {op}", self.components())
    }
}

/// How colours reach the page when printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfColorMode {
    /// Colours as drawn.
    #[default]
    Color,
    /// Every colour (and image pixel) becomes its luminance gray.
    Grayscale,
    /// Ink is black, area fills are black or white, nothing is gray.
    BlackWhite,
}

impl PdfColorMode {
    /// Luminance of an 8-bit colour, 0..=1.
    fn luma(c: PdfColor) -> f64 {
        match c {
            PdfColor::Gray(g) => g.clamp(0.0, 1.0),
            PdfColor::Rgb(r, g, b) => {
                (0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)) / 255.0
            }
        }
    }

    /// The colour of strokes, text and tracked fills in this mode.
    fn ink(self, c: PdfColor) -> PdfColor {
        match self {
            PdfColorMode::Color => c,
            PdfColorMode::Grayscale => PdfColor::Gray(Self::luma(c)),
            PdfColorMode::BlackWhite => {
                PdfColor::Gray(if Self::luma(c) > 0.92 { 1.0 } else { 0.0 })
            }
        }
    }

    /// The colour of a filled area: black B&W prints keep only dark fills.
    fn area(self, c: PdfColor) -> PdfColor {
        match self {
            PdfColorMode::BlackWhite => PdfColor::Gray(if Self::luma(c) < 0.5 { 0.0 } else { 1.0 }),
            other => other.ink(c),
        }
    }
}

/// Line cap style (`J` operator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// Line join style (`j` operator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// The parts of the PDF graphics state this writer tracks.
#[derive(Debug, Clone, PartialEq)]
struct GState {
    stroke: PdfColor,
    fill: PdfColor,
    line_width: f64,
    dash: Vec<f64>,
    dash_phase: f64,
    cap: LineCap,
    join: LineJoin,
}

impl GState {
    fn initial() -> Self {
        Self {
            stroke: PdfColor::BLACK,
            fill: PdfColor::BLACK,
            line_width: 0.5,
            dash: Vec::new(),
            dash_phase: 0.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }
    }
}

/// An embedded 8-bit `/DeviceRGB` image.
#[derive(Debug, Clone)]
struct Image {
    width_px: u32,
    height_px: u32,
    rgb: Vec<u8>,
}

/// Object number of the Helvetica font.
const OBJ_FONT: usize = 3;
/// Object number of the Helvetica-Bold font.
const OBJ_FONT_BOLD: usize = 4;
/// First object number after the fixed catalog / pages / font objects.
const OBJ_FIRST_DYNAMIC: usize = 5;

/// A multi-page PDF under construction.
#[derive(Debug, Clone)]
pub struct PdfDoc {
    /// Size of the current (last) page.
    width: f64,
    height: f64,
    pages: Vec<String>,
    /// Page size per page, parallel to `pages`.
    sizes: Vec<(f64, f64)>,
    /// Indices into `images` used by each page, parallel to `pages`.
    page_images: Vec<Vec<usize>>,
    images: Vec<Image>,
    gs: GState,
    /// States saved by [`PdfDoc::save_state`] on the current page.
    stack: Vec<GState>,
    bold: bool,
    color_mode: PdfColorMode,
    /// Every stroke is drawn this wide (print with line weights off).
    uniform_line: Option<f64>,
    /// `(page index, title)` bookmarks, written as the PDF outline.
    bookmarks: Vec<(usize, String)>,
}

impl PdfDoc {
    /// A new document whose pages are `width_pt` x `height_pt`, with one
    /// blank page already started. Initial state: black, 0.5 pt lines.
    pub fn new(width_pt: f64, height_pt: f64) -> Self {
        let mut doc = Self {
            width: width_pt,
            height: height_pt,
            pages: vec![String::new()],
            sizes: vec![(width_pt, height_pt)],
            page_images: vec![Vec::new()],
            images: Vec::new(),
            gs: GState::initial(),
            stack: Vec::new(),
            bold: false,
            color_mode: PdfColorMode::Color,
            uniform_line: None,
            bookmarks: Vec::new(),
        };
        doc.emit_state();
        doc
    }

    /// Number of pages so far.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Size in points of the current page.
    pub fn page_size(&self) -> (f64, f64) {
        (self.width, self.height)
    }

    /// Print in colour, grayscale or black and white from here on. Set it
    /// before drawing: colours already written stay as they were.
    pub fn set_color_mode(&mut self, mode: PdfColorMode) {
        self.color_mode = mode;
        let (s, f) = (self.gs.stroke, self.gs.fill);
        self.set_stroke_color(s);
        self.set_fill_color(f);
    }

    /// The colour mode.
    pub fn color_mode(&self) -> PdfColorMode {
        self.color_mode
    }

    /// Draw every stroke `width_pt` wide (`None` restores each stroke's own
    /// width): "Print line weights" off. Fill outlines and hatches follow.
    pub fn set_uniform_line_width(&mut self, width_pt: Option<f64>) {
        self.uniform_line = width_pt.map(|w| w.max(0.0));
        let w = self.gs.line_width;
        self.set_line_width(w);
    }

    /// A stroke width after the uniform-width override.
    fn lw(&self, width_pt: f64) -> f64 {
        self.uniform_line.unwrap_or(width_pt)
    }

    /// Concatenate a matrix `[a b c d e f]` to the current transformation
    /// (`cm`). Call between [`PdfDoc::save_state`] and [`PdfDoc::restore_state`]
    /// to bound it; later drawing is scaled, rotated or moved by it.
    pub fn transform(&mut self, m: [f64; 6]) {
        let s = format!(
            "{} {} {} {} {} {} cm\n",
            num6(m[0]),
            num6(m[1]),
            num6(m[2]),
            num6(m[3]),
            num6(m[4]),
            num6(m[5])
        );
        self.emit(&s);
    }

    /// Add a bookmark (PDF outline entry) to the current page.
    pub fn add_bookmark(&mut self, title: &str) {
        let page = self.pages.len() - 1;
        self.bookmarks.push((page, title.to_string()));
    }

    /// The bookmarks added so far, `(page index, title)`.
    pub fn bookmarks(&self) -> &[(usize, String)] {
        &self.bookmarks
    }

    fn emit(&mut self, s: &str) {
        if let Some(p) = self.pages.last_mut() {
            p.push_str(s);
        }
    }

    /// Write the tracked state at the top of a fresh page.
    fn emit_state(&mut self) {
        let g = self.gs.clone();
        let mut s = format!(
            "{} {} {} w\n",
            g.fill.fill_op(),
            g.stroke.stroke_op(),
            num(g.line_width)
        );
        if g.cap != LineCap::Butt {
            let _ = writeln!(s, "{} J", g.cap as u8);
        }
        if g.join != LineJoin::Miter {
            let _ = writeln!(s, "{} j", g.join as u8);
        }
        if !g.dash.is_empty() {
            s.push_str(&Self::dash_op(&g.dash, g.dash_phase));
        }
        self.emit(&s);
    }

    /// Close any states left open on the page being finished.
    fn close_states(&mut self) {
        while let Some(prev) = self.stack.pop() {
            self.emit("Q\n");
            self.gs = prev;
        }
    }

    /// Start a new page of the same size as the current one, carrying over
    /// the current colours, line width, dash, cap and join. States left open
    /// with [`PdfDoc::save_state`] are closed first.
    pub fn new_page(&mut self) {
        let (w, h) = (self.width, self.height);
        self.new_page_sized(w, h);
    }

    /// Start a new page of `width_pt` x `height_pt` points (pages of one
    /// document may differ in size), carrying over the drawing state.
    pub fn new_page_sized(&mut self, width_pt: f64, height_pt: f64) {
        self.close_states();
        self.width = width_pt;
        self.height = height_pt;
        self.pages.push(String::new());
        self.sizes.push((width_pt, height_pt));
        self.page_images.push(Vec::new());
        self.emit_state();
    }

    // ------------------------------------------------------------ state --

    /// Set the fill and stroke colour (0 = black, 1 = white).
    pub fn set_gray(&mut self, g: f64) {
        let c = self.color_mode.ink(PdfColor::Gray(g.clamp(0.0, 1.0)));
        self.gs.fill = c;
        self.gs.stroke = c;
        let s = format!("{} {}\n", c.fill_op(), c.stroke_op());
        self.emit(&s);
    }

    /// Set the stroke colour to 8-bit RGB.
    pub fn set_rgb_stroke(&mut self, r: u8, g: u8, b: u8) {
        let c = self.color_mode.ink(PdfColor::Rgb(r, g, b));
        self.gs.stroke = c;
        let s = format!("{}\n", c.stroke_op());
        self.emit(&s);
    }

    /// Set the fill colour (also used by text) to 8-bit RGB.
    pub fn set_rgb_fill(&mut self, r: u8, g: u8, b: u8) {
        let c = self.color_mode.ink(PdfColor::Rgb(r, g, b));
        self.gs.fill = c;
        let s = format!("{}\n", c.fill_op());
        self.emit(&s);
    }

    /// Set the stroke colour from a [`PdfColor`].
    pub fn set_stroke_color(&mut self, c: PdfColor) {
        let c = self.color_mode.ink(c);
        self.gs.stroke = c;
        let s = format!("{}\n", c.stroke_op());
        self.emit(&s);
    }

    /// Set the fill colour from a [`PdfColor`].
    pub fn set_fill_color(&mut self, c: PdfColor) {
        let c = self.color_mode.ink(c);
        self.gs.fill = c;
        let s = format!("{}\n", c.fill_op());
        self.emit(&s);
    }

    /// Set the line width used by [`PdfDoc::arc`] (other strokes pass their own).
    pub fn set_line_width(&mut self, width_pt: f64) {
        self.gs.line_width = self.lw(width_pt.max(0.0));
        let s = format!("{} w\n", num(self.gs.line_width));
        self.emit(&s);
    }

    /// Set the line cap style.
    pub fn set_line_cap(&mut self, cap: LineCap) {
        self.gs.cap = cap;
        let s = format!("{} J\n", cap as u8);
        self.emit(&s);
    }

    /// Set the line join style.
    pub fn set_line_join(&mut self, join: LineJoin) {
        self.gs.join = join;
        let s = format!("{} j\n", join as u8);
        self.emit(&s);
    }

    fn dash_op(pattern: &[f64], phase: f64) -> String {
        let items: Vec<String> = pattern.iter().map(|v| num(*v)).collect();
        format!("[{}] {} d\n", items.join(" "), num(phase))
    }

    /// Set a dash pattern (alternating on/off lengths in points) and phase.
    /// An empty pattern, or one that is all zeros or has a negative length,
    /// is the same as [`PdfDoc::set_dash_solid`].
    pub fn set_dash(&mut self, pattern: &[f64], phase: f64) {
        if pattern.iter().any(|v| *v < 0.0 || !v.is_finite()) || pattern.iter().all(|v| *v == 0.0) {
            self.set_dash_solid();
            return;
        }
        self.gs.dash = pattern.to_vec();
        self.gs.dash_phase = phase;
        let s = Self::dash_op(pattern, phase);
        self.emit(&s);
    }

    /// Back to solid lines.
    pub fn set_dash_solid(&mut self) {
        self.gs.dash.clear();
        self.gs.dash_phase = 0.0;
        self.emit("[] 0 d\n");
    }

    /// Choose Helvetica-Bold (`true`) or Helvetica (`false`) for later text
    /// and for the width calculations of [`PdfDoc::text_centered`] and
    /// [`PdfDoc::text_right`]. Survives [`PdfDoc::new_page`].
    pub fn set_font_bold(&mut self, bold: bool) {
        self.bold = bold;
    }

    /// Is the bold font selected?
    pub fn is_bold(&self) -> bool {
        self.bold
    }

    /// Save the graphics state (`q`). Pair with [`PdfDoc::restore_state`];
    /// states still open at a page break or at [`PdfDoc::finish`] are closed
    /// automatically so `q` and `Q` always balance.
    pub fn save_state(&mut self) {
        self.stack.push(self.gs.clone());
        self.emit("q\n");
    }

    /// Restore the state saved by the matching [`PdfDoc::save_state`]
    /// (`Q`). Does nothing when no state is saved.
    pub fn restore_state(&mut self) {
        if let Some(prev) = self.stack.pop() {
            self.emit("Q\n");
            self.gs = prev;
        }
    }

    /// Intersect the clip region with a rectangle (`re W n`). Call between
    /// [`PdfDoc::save_state`] and [`PdfDoc::restore_state`] to bound it.
    pub fn clip_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let s = format!("{} {} {} {} re W n\n", num(x), num(y), num(w), num(h));
        self.emit(&s);
    }

    // ------------------------------------------------------------ paths --

    /// Stroke a straight line.
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width_pt: f64) {
        let s = format!(
            "q {} w {} {} m {} {} l S Q\n",
            num(self.lw(width_pt)),
            num(x1),
            num(y1),
            num(x2),
            num(y2)
        );
        self.emit(&s);
    }

    fn path(points: &[(f64, f64)]) -> String {
        let mut s = String::new();
        for (i, (x, y)) in points.iter().enumerate() {
            let op = if i == 0 { "m" } else { "l" };
            let _ = write!(s, "{} {} {op} ", num(*x), num(*y));
        }
        s
    }

    /// Stroke a polyline, optionally closed. Fewer than 2 points draws nothing.
    pub fn polyline(&mut self, points: &[(f64, f64)], closed: bool, width_pt: f64) {
        if points.len() < 2 {
            return;
        }
        let s = format!(
            "q {} w {}{} Q\n",
            num(self.lw(width_pt)),
            Self::path(points),
            if closed { "s" } else { "S" }
        );
        self.emit(&s);
    }

    /// Fill a polygon with a gray level (no outline). The current colour is
    /// unchanged afterwards.
    pub fn filled_polygon(&mut self, points: &[(f64, f64)], gray: f64) {
        if points.len() < 3 {
            return;
        }
        let g = self.color_mode.area(PdfColor::Gray(gray.clamp(0.0, 1.0)));
        let s = format!("q {} {}f Q\n", g.fill_op(), Self::path(points));
        self.emit(&s);
    }

    /// Draw a closed polygon, filled with `fill` and/or outlined with the
    /// current stroke colour at `stroke_width_pt`. Neither given, or fewer
    /// than 3 points, draws nothing. The graphics state is unchanged.
    pub fn polygon(
        &mut self,
        points: &[(f64, f64)],
        fill: Option<PdfColor>,
        stroke_width_pt: Option<f64>,
    ) {
        if points.len() < 3 || (fill.is_none() && stroke_width_pt.is_none()) {
            return;
        }
        let mut s = String::from("q ");
        if let Some(c) = fill {
            let _ = write!(s, "{} ", self.color_mode.area(c).fill_op());
        }
        if let Some(w) = stroke_width_pt {
            let _ = write!(s, "{} w ", num(self.lw(w.max(0.0))));
        }
        s.push_str(&Self::path(points));
        s.push_str(match (fill.is_some(), stroke_width_pt.is_some()) {
            (true, true) => "b",
            (true, false) => "f",
            _ => "s",
        });
        s.push_str(" Q\n");
        self.emit(&s);
    }

    /// Stroke an axis-aligned rectangle with its lower-left corner at (x, y).
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, width_pt: f64) {
        let s = format!(
            "q {} w {} {} {} {} re S Q\n",
            num(self.lw(width_pt)),
            num(x),
            num(y),
            num(w),
            num(h)
        );
        self.emit(&s);
    }

    /// Fill an axis-aligned rectangle (no outline); the state is unchanged.
    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: PdfColor) {
        let s = format!(
            "q {} {} {} {} {} re f Q\n",
            self.color_mode.area(color).fill_op(),
            num(x),
            num(y),
            num(w),
            num(h)
        );
        self.emit(&s);
    }

    /// Fill the whole current page, e.g. with a drawing-sheet background.
    /// Call it first so later drawing lands on top.
    pub fn fill_page(&mut self, color: PdfColor) {
        let (w, h) = (self.width, self.height);
        self.fill_rect(0.0, 0.0, w, h, color);
    }

    /// Stroke one cubic Bezier from `p0` to `p1` with control points `c1`
    /// and `c2`.
    pub fn curve_to(
        &mut self,
        p0: (f64, f64),
        c1: (f64, f64),
        c2: (f64, f64),
        p1: (f64, f64),
        width_pt: f64,
    ) {
        let s = format!(
            "q {} w {} {} m {} {} {} {} {} {} c S Q\n",
            num(self.lw(width_pt)),
            num(p0.0),
            num(p0.1),
            num(c1.0),
            num(c1.1),
            num(c2.0),
            num(c2.1),
            num(p1.0),
            num(p1.1)
        );
        self.emit(&s);
    }

    /// Stroke a circular arc centred at (cx, cy) from `start_deg` to
    /// `end_deg` (counter-clockwise when end > start, clockwise otherwise),
    /// approximated with cubic Bezier segments of at most 90 degrees, using
    /// the current line width.
    pub fn arc(&mut self, cx: f64, cy: f64, r: f64, start_deg: f64, end_deg: f64) {
        let sweep = end_deg - start_deg;
        if sweep == 0.0 || r <= 0.0 {
            return;
        }
        let segs = (sweep.abs() / 90.0).ceil().max(1.0) as usize;
        let step = (sweep / segs as f64).to_radians();
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let mut a = start_deg.to_radians();
        let at = |ang: f64| (cx + r * ang.cos(), cy + r * ang.sin());
        let (x0, y0) = at(a);
        let mut s = format!("{} {} m ", num(x0), num(y0));
        for _ in 0..segs {
            let b = a + step;
            let (p0x, p0y) = (a.cos(), a.sin());
            let (p3x, p3y) = (b.cos(), b.sin());
            let c1 = (cx + r * (p0x - k * p0y), cy + r * (p0y + k * p0x));
            let c2 = (cx + r * (p3x + k * p3y), cy + r * (p3y - k * p3x));
            let e = at(b);
            let _ = write!(
                s,
                "{} {} {} {} {} {} c ",
                num(c1.0),
                num(c1.1),
                num(c2.0),
                num(c2.1),
                num(e.0),
                num(e.1)
            );
            a = b;
        }
        s.push_str("S\n");
        self.emit(&s);
    }

    /// Hatch the rectangle `(x, y, w, h)` with parallel lines `spacing_pt`
    /// apart, running at `angle_deg` (0 = horizontal lines, 45 = diagonals
    /// rising to the right), each `width_pt` wide in the current stroke
    /// colour and dash. Lines are aligned to the page origin so neighbouring
    /// rectangles hatch seamlessly. Material hatches (brick, insulation...)
    /// are built from this.
    pub fn hatch_rect(
        &mut self,
        rect: (f64, f64, f64, f64),
        angle_deg: f64,
        spacing_pt: f64,
        width_pt: f64,
    ) {
        const MAX_LINES: f64 = 20_000.0;
        let (x, y, w, h) = rect;
        if w <= 0.0 || h <= 0.0 || spacing_pt <= 0.0 || !spacing_pt.is_finite() {
            return;
        }
        let (sin, cos) = angle_deg.to_radians().sin_cos();
        let (dir, nrm) = ((cos, sin), (-sin, cos));
        let corners = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
        let proj = corners.map(|c| c.0 * nrm.0 + c.1 * nrm.1);
        let dmin = proj.iter().copied().fold(f64::INFINITY, f64::min);
        let dmax = proj.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let (k0, k1) = ((dmin / spacing_pt).ceil(), (dmax / spacing_pt).floor());
        if k1 < k0 || k1 - k0 > MAX_LINES {
            return;
        }
        // Clip the infinite line  P(t) = nrm * d + dir * t  to the rectangle.
        let clip = |d: f64| -> Option<((f64, f64), (f64, f64))> {
            let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
            for (o, dv, lo, hi) in [(nrm.0 * d, dir.0, x, x + w), (nrm.1 * d, dir.1, y, y + h)] {
                if dv.abs() < 1e-12 {
                    if o < lo - 1e-9 || o > hi + 1e-9 {
                        return None;
                    }
                } else {
                    let (a, b) = ((lo - o) / dv, (hi - o) / dv);
                    t0 = t0.max(a.min(b));
                    t1 = t1.min(a.max(b));
                }
            }
            if t1 - t0 > 1e-9 {
                Some((
                    (nrm.0 * d + dir.0 * t0, nrm.1 * d + dir.1 * t0),
                    (nrm.0 * d + dir.0 * t1, nrm.1 * d + dir.1 * t1),
                ))
            } else {
                None
            }
        };
        let mut s = format!("q {} w ", num(self.lw(width_pt)));
        let mut any = false;
        let mut k = k0;
        while k <= k1 {
            if let Some((a, b)) = clip(k * spacing_pt) {
                let _ = write!(
                    s,
                    "{} {} m {} {} l ",
                    num(a.0),
                    num(a.1),
                    num(b.0),
                    num(b.1)
                );
                any = true;
            }
            k += 1.0;
        }
        if any {
            s.push_str("S Q\n");
            self.emit(&s);
        }
    }

    // ------------------------------------------------------------- text --

    fn font_name(&self) -> &'static str {
        if self.bold {
            "F2"
        } else {
            "F1"
        }
    }

    /// Draw text with its baseline-left at (x, y) in Helvetica (or
    /// Helvetica-Bold, see [`PdfDoc::set_font_bold`]), using the current fill
    /// colour. Characters are mapped to WinAnsi; parentheses and backslashes
    /// are escaped.
    pub fn text(&mut self, x: f64, y: f64, size_pt: f64, text: &str) {
        let s = format!(
            "BT /{} {} Tf {} {} Td ({}) Tj ET\n",
            self.font_name(),
            num(size_pt),
            num(x),
            num(y),
            escape_text(text)
        );
        self.emit(&s);
    }

    /// Draw text rotated `angle_deg` degrees counter-clockwise about its
    /// baseline-left origin (x, y): 90 reads bottom to top.
    pub fn text_rotated(&mut self, x: f64, y: f64, size_pt: f64, angle_deg: f64, text: &str) {
        let (sin, cos) = angle_deg.to_radians().sin_cos();
        let s = format!(
            "BT /{} {} Tf {} {} {} {} {} {} Tm ({}) Tj ET\n",
            self.font_name(),
            num(size_pt),
            num(cos),
            num(sin),
            num(-sin),
            num(cos),
            num(x),
            num(y),
            escape_text(text)
        );
        self.emit(&s);
    }

    /// Approximate width of `text` in points (Helvetica metrics for ASCII,
    /// a mid-width guess for other characters).
    pub fn text_width(text: &str, size_pt: f64) -> f64 {
        Self::width_with(&HELVETICA_WIDTHS, text, size_pt)
    }

    /// Like [`PdfDoc::text_width`] for Helvetica-Bold.
    pub fn text_width_bold(text: &str, size_pt: f64) -> f64 {
        Self::width_with(&HELVETICA_BOLD_WIDTHS, text, size_pt)
    }

    /// Width of `text` in the currently selected font.
    pub fn current_text_width(&self, text: &str, size_pt: f64) -> f64 {
        if self.bold {
            Self::text_width_bold(text, size_pt)
        } else {
            Self::text_width(text, size_pt)
        }
    }

    fn width_with(table: &[u16; 95], text: &str, size_pt: f64) -> f64 {
        let units: u32 = text
            .chars()
            .map(|c| match c {
                ' '..='~' => u32::from(table[c as usize - 32]),
                _ => 556,
            })
            .sum();
        f64::from(units) * size_pt / 1000.0
    }

    /// Draw text horizontally centred on `cx` (current font).
    pub fn text_centered(&mut self, cx: f64, y: f64, size_pt: f64, text: &str) {
        let w = self.current_text_width(text, size_pt);
        self.text(cx - w * 0.5, y, size_pt, text);
    }

    /// Draw text ending at `x_right` (current font).
    pub fn text_right(&mut self, x_right: f64, y: f64, size_pt: f64, text: &str) {
        let w = self.current_text_width(text, size_pt);
        self.text(x_right - w, y, size_pt, text);
    }

    // ----------------------------------------------------------- images --

    /// Draw an 8-bit RGB image (`rgb` is `width_px * height_px * 3` bytes,
    /// rows top to bottom) scaled into the rectangle with lower-left corner
    /// (x, y) and size `w` x `h` points. The pixels are embedded uncompressed
    /// as a `/DeviceRGB` image XObject. Returns `false` (drawing nothing)
    /// when the data length does not match the pixel dimensions.
    #[allow(clippy::too_many_arguments)]
    pub fn image_rgb(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        width_px: u32,
        height_px: u32,
        rgb: &[u8],
    ) -> bool {
        let expect = u64::from(width_px) * u64::from(height_px) * 3;
        if width_px == 0 || height_px == 0 || expect != rgb.len() as u64 {
            return false;
        }
        let idx = self.images.len();
        let rgb: Vec<u8> = match self.color_mode {
            PdfColorMode::Color => rgb.to_vec(),
            mode => rgb
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| {
                    let l = PdfColorMode::luma(PdfColor::Rgb(p[0], p[1], p[2]));
                    let v = if mode == PdfColorMode::BlackWhite {
                        if l < 0.5 {
                            0
                        } else {
                            255
                        }
                    } else {
                        (l * 255.0).round() as u8
                    };
                    [v, v, v]
                })
                .collect(),
        };
        self.images.push(Image {
            width_px,
            height_px,
            rgb,
        });
        if let Some(used) = self.page_images.last_mut() {
            used.push(idx);
        }
        let s = format!(
            "q {} 0 0 {} {} {} cm /Im{} Do Q\n",
            num(w),
            num(h),
            num(x),
            num(y),
            idx + 1
        );
        self.emit(&s);
        true
    }

    /// Draw an 8-bit RGBA image (`width_px * height_px * 4` bytes), flattened
    /// onto white, as [`PdfDoc::image_rgb`] does. This takes raw pixels, not
    /// PNG file bytes (decoding PNG needs inflate).
    #[allow(clippy::too_many_arguments)]
    pub fn image_png_rgba(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        width_px: u32,
        height_px: u32,
        rgba: &[u8],
    ) -> bool {
        let expect = u64::from(width_px) * u64::from(height_px) * 4;
        if width_px == 0 || height_px == 0 || expect != rgba.len() as u64 {
            return false;
        }
        let rgb: Vec<u8> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let a = u32::from(p[3]);
                let blend = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
                [blend(p[0]), blend(p[1]), blend(p[2])]
            })
            .collect();
        self.image_rgb(x, y, w, h, width_px, height_px, &rgb)
    }

    /// Alias of [`PdfDoc::image_png_rgba`].
    #[allow(clippy::too_many_arguments)]
    pub fn image_rgba(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        width_px: u32,
        height_px: u32,
        rgba: &[u8],
    ) -> bool {
        self.image_png_rgba(x, y, w, h, width_px, height_px, rgba)
    }

    // ----------------------------------------------------------- output --

    /// Serialise the document: header, objects, xref table and trailer.
    ///
    /// Object layout: 1 catalog, 2 page tree, 3 Helvetica, 4 Helvetica-Bold,
    /// one image XObject per embedded image, then a page object and a content
    /// stream per page, the two font descriptors and, when bookmarks were
    /// added, the outline (root, then one item per bookmark). Non-page dictionaries write `/Type/X` without a space
    /// so a search for `/Type /Page` matches only pages.
    pub fn finish(mut self) -> Vec<u8> {
        self.close_states();
        let n_pages = self.pages.len();
        let n_img = self.images.len();
        let first_page_obj = OBJ_FIRST_DYNAMIC + n_img;
        let mut objs: Vec<Vec<u8>> = Vec::new();
        objs.push(b"<< /Type/Catalog /Pages 2 0 R >>".to_vec());
        let kids: Vec<String> = (0..n_pages)
            .map(|i| format!("{} 0 R", first_page_obj + 2 * i))
            .collect();
        objs.push(
            format!(
                "<< /Kids [{}] /Count {n_pages} /Type/Pages >>",
                kids.join(" ")
            )
            .into_bytes(),
        );
        // The two standard fonts carry their metrics (FirstChar, LastChar,
        // Widths) and a font descriptor, so a viewer lays the text out the
        // same way without a substitute font; the descriptors are the last
        // objects of the file (see `descriptor_obj` below).
        let n_dynamic = n_img + 2 * n_pages;
        let descriptor_obj = |bold: bool| OBJ_FIRST_DYNAMIC + n_dynamic + usize::from(bold);
        for (bold, base, widths) in [
            (false, "Helvetica", &HELVETICA_WIDTHS),
            (true, "Helvetica-Bold", &HELVETICA_BOLD_WIDTHS),
        ] {
            let w: Vec<String> = widths.iter().map(|v| v.to_string()).collect();
            objs.push(
                format!(
                    "<< /Type/Font /Subtype/Type1 /BaseFont/{base} /Encoding/WinAnsiEncoding \
                     /FirstChar 32 /LastChar 126 /Widths [{}] /FontDescriptor {} 0 R >>",
                    w.join(" "),
                    descriptor_obj(bold)
                )
                .into_bytes(),
            );
        }
        for img in &self.images {
            let mut obj = format!(
                "<< /Type/XObject /Subtype/Image /Width {} /Height {} \
                 /ColorSpace/DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n",
                img.width_px,
                img.height_px,
                img.rgb.len()
            )
            .into_bytes();
            obj.extend_from_slice(&img.rgb);
            obj.extend_from_slice(b"endstream");
            objs.push(obj);
        }
        for (i, content) in self.pages.iter().enumerate() {
            let (w, h) = self.sizes[i];
            let mut used = self.page_images[i].clone();
            used.sort_unstable();
            used.dedup();
            let xobjects = if used.is_empty() {
                String::new()
            } else {
                let items: Vec<String> = used
                    .iter()
                    .map(|k| format!("/Im{} {} 0 R", k + 1, OBJ_FIRST_DYNAMIC + k))
                    .collect();
                format!(" /XObject << {} >>", items.join(" "))
            };
            objs.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents {} 0 R \
                     /Resources << /Font << /F1 {OBJ_FONT} 0 R /F2 {OBJ_FONT_BOLD} 0 R >>{xobjects} >> >>",
                    num(w),
                    num(h),
                    first_page_obj + 2 * i + 1
                )
                .into_bytes(),
            );
            let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            stream.extend_from_slice(content.as_bytes());
            stream.extend_from_slice(b"endstream");
            objs.push(stream);
        }
        for (bold, base) in [(false, "Helvetica"), (true, "Helvetica-Bold")] {
            objs.push(
                format!(
                    "<< /Type /FontDescriptor /FontName/{base} /Flags 32 \
                     /FontBBox [-166 -225 1000 931] /ItalicAngle 0 /Ascent 718 /Descent -207 \
                     /CapHeight 718 /StemV {} >>",
                    if bold { 140 } else { 88 }
                )
                .into_bytes(),
            );
        }
        if !self.bookmarks.is_empty() {
            // Outline: the root, then one item per bookmark, all after the
            // pages so the fixed object numbers above do not move.
            let root = objs.len() + 1;
            let n = self.bookmarks.len();
            objs.push(
                format!(
                    "<< /Type/Outlines /First {} 0 R /Last {} 0 R /Count {n} >>",
                    root + 1,
                    root + n
                )
                .into_bytes(),
            );
            for (i, (page, title)) in self.bookmarks.iter().enumerate() {
                let mut d = format!(
                    "<< /Title ({}) /Parent {root} 0 R /Dest [{} 0 R /Fit]",
                    escape_text(title),
                    first_page_obj + 2 * page
                );
                if i > 0 {
                    let _ = write!(d, " /Prev {} 0 R", root + i);
                }
                if i + 1 < n {
                    let _ = write!(d, " /Next {} 0 R", root + i + 2);
                }
                d.push_str(" >>");
                objs.push(d.into_bytes());
            }
            objs[0] = format!(
                "<< /Type/Catalog /Pages 2 0 R /Outlines {root} 0 R /PageMode/UseOutlines >>"
            )
            .into_bytes();
        }

        let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::with_capacity(objs.len());
        for (i, body) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", objs.len() + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
                objs.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn count(hay: &[u8], needle: &str) -> usize {
        let n = needle.as_bytes();
        hay.windows(n.len()).filter(|w| *w == n).count()
    }

    /// Parse the xref table and check every offset lands on "N 0 obj".
    pub(crate) fn check_xref(pdf: &[u8]) {
        // Latin-1 view so byte offsets equal char offsets.
        let text: String = pdf.iter().map(|&b| b as char).collect();
        let sx = text.rfind("startxref\n").expect("startxref");
        let xref_off: usize = text[sx + 10..]
            .lines()
            .next()
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // Char offsets above 127 take two bytes in `text`; map via a byte index.
        let bytes = pdf;
        assert!(bytes[xref_off..].starts_with(b"xref\n"));
        let tail = &text[text.rfind("xref\n0 ").unwrap()..];
        let mut lines = tail.lines().skip(1);
        let header: Vec<usize> = lines
            .next()
            .unwrap()
            .split_whitespace()
            .map(|t| t.parse().unwrap())
            .collect();
        assert_eq!(header[0], 0);
        let total = header[1];
        assert!(lines.next().unwrap().starts_with("0000000000 65535 f"));
        for obj in 1..total {
            let entry = lines.next().unwrap();
            assert_eq!(entry.len(), 19, "xref entries are 20 bytes with newline");
            assert!(entry.ends_with(" 00000 n "));
            let off: usize = entry[..10].parse().unwrap();
            let want = format!("{obj} 0 obj\n");
            assert!(
                bytes[off..].starts_with(want.as_bytes()),
                "object {obj} offset {off} wrong"
            );
        }
        assert!(text.contains(&format!("/Size {total}")));
    }

    /// Content-stream tokens with `(...)` string bodies removed, so a `Q`
    /// inside text cannot be mistaken for an operator.
    pub(crate) fn operators(content: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut depth = 0usize;
        let mut cur = String::new();
        let mut esc = false;
        for ch in content.chars() {
            if depth > 0 {
                match (esc, ch) {
                    (true, _) => esc = false,
                    (false, '\\') => esc = true,
                    (false, '(') => depth += 1,
                    (false, ')') => depth -= 1,
                    _ => {}
                }
                continue;
            }
            match ch {
                '(' => {
                    depth = 1;
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                c => cur.push(c),
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }

    /// All page content streams of a finished PDF (found by their `/Length`).
    pub(crate) fn content_streams(pdf: &[u8]) -> Vec<String> {
        let text: String = pdf.iter().map(|&b| b as char).collect();
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(i) = text[from..].find("<< /Length ") {
            let at = from + i;
            let len: usize = text[at + 11..].split(' ').next().unwrap().parse().unwrap();
            let start = at + text[at..].find("stream\n").unwrap() + 7;
            out.push(text[start..start + len].to_string());
            from = start + len;
        }
        out
    }

    /// `q` and `Q` operators are balanced (never negative, zero at the end).
    pub(crate) fn assert_balanced(content: &str) {
        let mut depth = 0i32;
        for op in operators(content) {
            match op.as_str() {
                "q" => depth += 1,
                "Q" => {
                    depth -= 1;
                    assert!(depth >= 0, "Q without q");
                }
                _ => {}
            }
        }
        assert_eq!(depth, 0, "unbalanced q/Q");
    }

    fn find(hay: &[u8], needle: &str) -> Option<usize> {
        let n = needle.as_bytes();
        hay.windows(n.len()).position(|w| w == n)
    }

    /// Every xref offset points at `N 0 obj` ... `endobj`, stream `/Length`
    /// values match the bytes, and every `N 0 R` reference resolves to an
    /// existing object.
    pub(crate) fn check_objects(pdf: &[u8]) {
        let sx = pdf.windows(10).rposition(|w| w == b"startxref\n").unwrap() + 10;
        let tail = String::from_utf8_lossy(&pdf[sx..]).into_owned();
        let xref_off: usize = tail.lines().next().unwrap().trim().parse().unwrap();
        let table = String::from_utf8_lossy(&pdf[xref_off..]).into_owned();
        let mut lines = table.lines();
        assert_eq!(lines.next(), Some("xref"));
        let total: usize = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        lines.next();
        let offs: Vec<usize> = (1..total)
            .map(|_| lines.next().unwrap()[..10].parse().unwrap())
            .collect();
        for (i, &off) in offs.iter().enumerate() {
            let end = off + find(&pdf[off..], "endobj").expect("endobj");
            let body = &pdf[off..end];
            assert!(body.starts_with(format!("{} 0 obj\n", i + 1).as_bytes()));
            if let Some(p) = find(body, "stream\n") {
                let dict = String::from_utf8_lossy(&body[..p]).into_owned();
                let l = dict.find("/Length ").expect("stream has /Length") + 8;
                let len: usize = dict[l..].split([' ', '>']).next().unwrap().parse().unwrap();
                assert!(
                    body[p + 7 + len..].starts_with(b"endstream"),
                    "object {} /Length {len} does not end at endstream",
                    i + 1
                );
            } else {
                let dict = String::from_utf8_lossy(body).into_owned();
                let toks: Vec<&str> = dict
                    .split(|c: char| c.is_whitespace() || c == '[' || c == ']')
                    .collect();
                for w in toks.windows(3) {
                    if w[2] == "R" && w[1] == "0" {
                        if let Ok(n) = w[0].parse::<usize>() {
                            assert!(
                                (1..=offs.len()).contains(&n),
                                "dangling ref {n} in object {}",
                                i + 1
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn structure_and_xref() {
        let mut d = PdfDoc::new(612.0, 792.0);
        d.line(0.0, 0.0, 100.0, 100.0, 1.0);
        d.new_page();
        d.text(10.0, 10.0, 12.0, "Hi (there) \\ ok");
        d.new_page();
        d.filled_polygon(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], 0.5);
        let pdf = d.finish();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF"));
        assert_eq!(count(&pdf, "/Type /Page"), 3);
        assert_eq!(count(&pdf, "/Type/Pages"), 1);
        assert_eq!(count(&pdf, "Hi \\(there\\) \\\\ ok"), 1);
        check_xref(&pdf);
    }

    #[test]
    fn stream_length_matches() {
        let mut d = PdfDoc::new(100.0, 100.0);
        d.polyline(&[(0.0, 0.0), (5.5, 5.25)], true, 2.0);
        let pdf = d.finish();
        let text: String = pdf.iter().map(|&b| b as char).collect();
        let l: usize = text
            .split("/Length ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let start = text.find("stream\n").unwrap() + 7;
        let end = text.find("endstream").unwrap();
        assert_eq!(end - start, l);
    }

    #[test]
    fn winansi_and_widths() {
        assert_eq!(escape_text("5\u{b0}"), "5\\260");
        assert_eq!(escape_text("\u{2019}"), "\\222");
        assert_eq!(escape_text("\u{4e2d}"), "?");
        assert!((PdfDoc::text_width("AA", 10.0) - 13.34).abs() < 1e-9);
    }

    #[test]
    fn arc_uses_bezier_segments() {
        let mut d = PdfDoc::new(100.0, 100.0);
        d.arc(50.0, 50.0, 10.0, 0.0, 90.0);
        d.arc(50.0, 50.0, 10.0, 0.0, 180.0);
        let text = d.pages[0].clone();
        assert_eq!(text.matches(" c ").count(), 3);
        assert!(text.contains("60 50 m"));
        // Quarter-circle control points: k = 0.5523 * 10.
        assert!(text.contains("60 55.523 55.523 60 50 60 c"));
    }

    fn page_text(d: &PdfDoc, i: usize) -> &str {
        &d.pages[i]
    }

    #[test]
    fn colours_dash_cap_join() {
        let mut d = PdfDoc::new(100.0, 100.0);
        d.set_rgb_stroke(255, 0, 0);
        d.set_rgb_fill(0, 0, 255);
        d.set_dash(&[4.0, 2.0], 1.0);
        d.set_line_cap(LineCap::Round);
        d.set_line_join(LineJoin::Bevel);
        d.set_dash(&[], 0.0);
        d.set_dash(&[0.0, 0.0], 0.0);
        d.set_dash_solid();
        let t = page_text(&d, 0);
        assert!(t.contains("1 0 0 RG"));
        assert!(t.contains("0 0 1 rg"));
        assert!(t.contains("[4 2] 1 d"));
        assert!(t.contains("1 J"));
        assert!(t.contains("2 j"));
        assert_eq!(t.matches("[] 0 d").count(), 3);
        // The tracked state is written at the top of the next page.
        d.set_dash(&[3.0, 3.0], 0.0);
        d.new_page();
        let p2 = page_text(&d, 1);
        assert!(p2.starts_with("0 0 1 rg 1 0 0 RG 0.5 w\n"));
        assert!(p2.contains("1 J") && p2.contains("2 j") && p2.contains("[3 3] 0 d"));
        check_xref(&d.finish());
    }

    #[test]
    fn clip_and_state_are_balanced() {
        let mut d = PdfDoc::new(200.0, 200.0);
        d.save_state();
        d.clip_rect(10.0, 10.0, 50.0, 40.0);
        d.line(0.0, 0.0, 200.0, 200.0, 1.0);
        d.save_state();
        d.set_rgb_stroke(1, 2, 3);
        d.restore_state();
        d.restore_state();
        d.restore_state(); // extra restore is ignored
        d.save_state(); // left open: closed by new_page
        d.new_page();
        d.save_state(); // left open: closed by finish
        d.text(5.0, 5.0, 8.0, "Q q (Q)");
        let pdf = d.finish();
        let streams = content_streams(&pdf);
        assert_eq!(streams.len(), 2);
        for s in &streams {
            assert_balanced(s);
        }
        assert!(streams[0].contains("10 10 50 40 re W n"));
        check_objects(&pdf);
    }

    #[test]
    fn state_restored_after_restore() {
        let mut d = PdfDoc::new(50.0, 50.0);
        d.set_rgb_fill(9, 9, 9);
        d.save_state();
        d.set_rgb_fill(200, 0, 0);
        d.restore_state();
        d.new_page();
        assert!(page_text(&d, 1).starts_with("0.035 0.035 0.035 rg"));
    }

    #[test]
    fn two_fonts_bold_and_alignment() {
        let mut d = PdfDoc::new(300.0, 100.0);
        d.text(1.0, 1.0, 10.0, "regular");
        d.set_font_bold(true);
        assert!(d.is_bold());
        d.text(1.0, 20.0, 10.0, "bold");
        d.text_right(200.0, 40.0, 10.0, "Right");
        d.text_centered(100.0, 60.0, 10.0, "Mid");
        let t = page_text(&d, 0).to_string();
        assert!(t.contains("BT /F1 10 Tf 1 1 Td (regular)"));
        assert!(t.contains("BT /F2 10 Tf 1 20 Td (bold)"));
        let wr = PdfDoc::text_width_bold("Right", 10.0);
        assert!(t.contains(&format!("{} 40 Td (Right)", num(200.0 - wr))));
        let wm = PdfDoc::text_width_bold("Mid", 10.0);
        assert!(t.contains(&format!("{} 60 Td (Mid)", num(100.0 - wm * 0.5))));
        // Bold is wider than regular and survives page breaks.
        assert!(PdfDoc::text_width_bold("Hello", 10.0) > PdfDoc::text_width("Hello", 10.0));
        d.new_page();
        assert!(d.is_bold());
        let pdf = d.finish();
        assert_eq!(count(&pdf, "/Type/Font"), 2);
        assert_eq!(count(&pdf, "/BaseFont/Helvetica-Bold"), 1);
        assert_eq!(count(&pdf, "/F1 3 0 R /F2 4 0 R"), 2);
        check_xref(&pdf);
        check_objects(&pdf);
    }

    #[test]
    fn bold_width_table_spot_checks() {
        // Helvetica-Bold AFM: space 278, A 722, W 944, i 278, m 889.
        assert!((PdfDoc::text_width_bold(" AWim", 1000.0) - 3111.0).abs() < 1e-9);
    }

    #[test]
    fn rotated_text_uses_tm() {
        let mut d = PdfDoc::new(100.0, 100.0);
        d.text_rotated(10.0, 20.0, 8.0, 90.0, "up");
        d.text_rotated(10.0, 20.0, 8.0, 30.0, "tilt");
        let t = page_text(&d, 0);
        assert!(t.contains("BT /F1 8 Tf 0 1 -1 0 10 20 Tm (up) Tj ET"));
        assert!(t.contains("0.866 0.5 -0.5 0.866 10 20 Tm (tilt)"));
    }

    #[test]
    fn polygon_fill_and_stroke_operators() {
        let tri = [(0.0, 0.0), (10.0, 0.0), (5.0, 8.0)];
        let mut d = PdfDoc::new(100.0, 100.0);
        d.polygon(&tri, Some(PdfColor::Rgb(204, 204, 204)), Some(1.5));
        d.polygon(&tri, Some(PdfColor::Gray(0.5)), None);
        d.polygon(&tri, None, Some(2.0));
        d.polygon(&tri, None, None);
        d.polygon(&tri[..2], Some(PdfColor::WHITE), None);
        let t = page_text(&d, 0);
        assert!(t.contains("q 0.8 0.8 0.8 rg 1.5 w 0 0 m 10 0 l 5 8 l b Q"));
        assert!(t.contains("q 0.5 g 0 0 m 10 0 l 5 8 l f Q"));
        assert!(t.contains("q 2 w 0 0 m 10 0 l 5 8 l s Q"));
        assert_eq!(t.matches(" Q\n").count(), 3);
    }

    #[test]
    fn curves_and_arcs() {
        let mut d = PdfDoc::new(100.0, 100.0);
        d.curve_to((0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0), 1.0);
        d.arc(50.0, 50.0, 10.0, 0.0, 360.0);
        let t = page_text(&d, 0);
        assert!(t.contains("q 1 w 0 0 m 0 10 10 10 10 0 c S Q"));
        assert_eq!(t.matches(" c ").count(), 1 + 4);
    }

    #[test]
    fn hatch_lines_stay_inside_rect() {
        let mut d = PdfDoc::new(200.0, 200.0);
        d.hatch_rect((10.0, 20.0, 50.0, 30.0), 45.0, 5.0, 0.3);
        d.hatch_rect((0.0, 0.0, 10.0, 10.0), 0.0, 5.0, 0.3);
        d.hatch_rect((0.0, 0.0, 10.0, 10.0), 0.0, 0.0, 0.3); // ignored
        d.hatch_rect((0.0, 0.0, 0.0, 10.0), 0.0, 5.0, 0.3); // ignored
        let t = page_text(&d, 0).to_string();
        let ops = operators(&t);
        // Walk the first hatch stroke and check every endpoint is in the rect.
        let first_q = ops.iter().position(|o| o == "q").unwrap();
        let first_s = ops.iter().position(|o| o == "S").unwrap();
        let mut n = 0;
        for w in ops[first_q + 3..first_s].windows(3) {
            if w[2] == "m" || w[2] == "l" {
                let (x, y): (f64, f64) = (w[0].parse().unwrap(), w[1].parse().unwrap());
                assert!(
                    (9.999..=60.001).contains(&x) && (19.999..=50.001).contains(&y),
                    "{x},{y}"
                );
                n += 1;
            }
        }
        assert!(n >= 20, "expected many hatch endpoints, got {n}");
        // Diagonals at 45 degrees: dx == dy on each segment.
        assert!(t.contains(" S Q\n"));
        // The horizontal hatch of the 10x10 square has lines at y = 0, 5, 10.
        assert!(t.contains("0 5 m 10 5 l") || t.contains("0 5 m 10 5 l "));
        assert_eq!(t.matches("q 0.3 w").count(), 2);
    }

    fn rgb_gradient(w: u32, h: u32) -> Vec<u8> {
        (0..w * h)
            .flat_map(|i| [(i % 256) as u8, (i * 7 % 256) as u8, (i * 13 % 256) as u8])
            .collect()
    }

    #[test]
    fn images_are_embedded_and_xref_holds() {
        let mut d = PdfDoc::new(612.0, 792.0);
        let px = rgb_gradient(4, 3);
        assert!(d.image_rgb(10.0, 20.0, 40.0, 30.0, 4, 3, &px));
        // RGBA: half-transparent red flattens onto white.
        let rgba = [255, 0, 0, 128, 0, 0, 0, 0];
        assert!(d.image_png_rgba(0.0, 0.0, 20.0, 10.0, 2, 1, &rgba));
        d.new_page();
        assert!(d.image_rgb(0.0, 0.0, 5.0, 5.0, 4, 3, &px));
        // Wrong lengths draw nothing.
        assert!(!d.image_rgb(0.0, 0.0, 5.0, 5.0, 4, 3, &px[..10]));
        assert!(!d.image_png_rgba(0.0, 0.0, 5.0, 5.0, 2, 1, &rgba[..7]));
        assert!(!d.image_rgb(0.0, 0.0, 5.0, 5.0, 0, 3, &[]));
        d.set_font_bold(true);
        d.text(1.0, 1.0, 9.0, "caption");
        let pdf = d.finish();
        assert_eq!(count(&pdf, "/Subtype/Image"), 3);
        assert_eq!(count(&pdf, "/ColorSpace/DeviceRGB"), 3);
        assert_eq!(count(&pdf, "/Type/Font"), 2);
        assert_eq!(count(&pdf, "/Type /Page"), 2);
        check_xref(&pdf);
        check_objects(&pdf);
        let t: String = pdf.iter().map(|&b| b as char).collect();
        // Image objects follow the fonts: 5, 6, 7; pages from 8.
        assert!(t.contains("/XObject << /Im1 5 0 R /Im2 6 0 R >>"));
        assert!(t.contains("/XObject << /Im3 7 0 R >>"));
        assert!(t.contains("q 40 0 0 30 10 20 cm /Im1 Do Q"));
        // The flattened RGBA pixel: 255,0,0 at a=128 over white is 255,127,127.
        let flat = [255u8, 127, 127, 255, 255, 255];
        assert!(pdf.windows(6).any(|w| w == flat));
        // Raw pixel bytes are present verbatim.
        assert!(pdf.windows(px.len()).any(|w| w == px.as_slice()));
        for s in content_streams(&pdf) {
            assert_balanced(&s);
        }
    }

    #[test]
    fn mixed_page_sizes() {
        let mut d = PdfDoc::new(612.0, 792.0);
        d.line(0.0, 0.0, 10.0, 10.0, 1.0);
        d.new_page_sized(1728.0, 2592.0);
        assert_eq!(d.page_size(), (1728.0, 2592.0));
        d.fill_page(PdfColor::Rgb(249, 248, 244));
        d.new_page(); // inherits the previous size
        assert_eq!(d.page_size(), (1728.0, 2592.0));
        assert_eq!(d.page_count(), 3);
        let pdf = d.finish();
        let t: String = pdf.iter().map(|&b| b as char).collect();
        assert!(t.contains("/MediaBox [0 0 612 792]"));
        assert_eq!(count(&pdf, "/MediaBox [0 0 1728 2592]"), 2);
        assert!(t.contains("0 0 1728 2592 re f"));
        assert!(t.contains("0.976 0.973 0.957 rg"));
        check_xref(&pdf);
        check_objects(&pdf);
    }

    #[test]
    fn full_kitchen_sink_validates() {
        let mut d = PdfDoc::new(300.0, 300.0);
        d.set_rgb_stroke(10, 20, 30);
        d.set_dash(&[3.0, 2.0], 0.0);
        d.save_state();
        d.clip_rect(0.0, 0.0, 100.0, 100.0);
        d.hatch_rect((0.0, 0.0, 100.0, 100.0), 135.0, 4.0, 0.25);
        d.restore_state();
        d.set_font_bold(true);
        d.text_rotated(20.0, 20.0, 9.0, 270.0, "side");
        d.image_rgb(0.0, 0.0, 10.0, 10.0, 2, 2, &[7; 12]);
        d.new_page_sized(100.0, 50.0);
        d.polygon(
            &[(0.0, 0.0), (9.0, 0.0), (9.0, 9.0)],
            Some(PdfColor::BLACK),
            Some(1.0),
        );
        let pdf = d.finish();
        check_xref(&pdf);
        check_objects(&pdf);
        for s in content_streams(&pdf) {
            assert_balanced(&s);
        }
    }
}
