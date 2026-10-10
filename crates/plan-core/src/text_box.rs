//! Text boxes: the wrap width, alignment, border and background fill of a
//! text object (TXT-1, TXT-3, TXT-13, TXT-16, S-25).
//!
//! A text is a `CadItem::Text` whose position is the lower left corner of its
//! block of lines. The extras that make it a box live in
//! [`crate::cad::CadAttrs::text_box`] so the item itself (used all over the
//! code) does not change:
//!
//! * `width` wraps the text at that many plan inches (0 keeps the text on the
//!   lines the user typed);
//! * `height` is a minimum box height; the box grows past it as the text
//!   needs (Chief's text boxes auto-grow, the Resize handle changes the
//!   width), and `valign` places the lines inside a taller box;
//! * `halign` lines the text up left, center or right inside the width;
//! * `border` draws a frame `margin` inches outside the box, `background`
//!   fills it.
//!
//! Everything here is pure geometry in the text's own frame (origin at the
//! lower left corner, +x along the text, +y up) so the plan view, the Layout
//! sheets and the PDF writer lay a box out the same way. Glyph widths are the
//! plan-wide estimate [`TEXT_WIDTH_FACTOR`] of the text height per character.

use crate::cad::TEXT_WIDTH_FACTOR;
use crate::geometry::Point;
use crate::text_styles::RichRun;
use serde::{Deserialize, Serialize};

/// Line pitch as a multiple of the text height.
pub const LINE_SPACING: f64 = 1.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl HAlign {
    pub const ALL: [HAlign; 3] = [HAlign::Left, HAlign::Center, HAlign::Right];

    pub fn label(self) -> &'static str {
        match self {
            HAlign::Left => "Left",
            HAlign::Center => "Center",
            HAlign::Right => "Right",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

impl VAlign {
    pub const ALL: [VAlign; 3] = [VAlign::Top, VAlign::Middle, VAlign::Bottom];

    pub fn label(self) -> &'static str {
        match self {
            VAlign::Top => "Top",
            VAlign::Middle => "Middle",
            VAlign::Bottom => "Bottom",
        }
    }
}

/// The box settings of one text object.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextBox {
    /// Wrap width in plan inches; 0 is no wrapping.
    pub width: f64,
    /// Minimum box height in plan inches; 0 fits the text.
    pub height: f64,
    pub halign: HAlign,
    pub valign: VAlign,
    /// A frame around the text.
    pub border: bool,
    /// Plan inches between the text box and the frame.
    pub margin: f64,
    /// Frame weight, hundredths of a millimetre; 0 is the layer's.
    pub border_weight: u32,
    /// Fill behind the text (and inside the frame).
    pub background: Option<[u8; 3]>,
    /// Paragraph Options: the line pitch as a multiple of the single
    /// spacing (1 single, 1.5, 2 double).
    pub line_spacing: f64,
    /// Paragraph Options: left and right margins inside the box, and the
    /// extra indent of the first line, plan inches.
    pub para_left: f64,
    pub para_right: f64,
    pub para_indent: f64,
    /// Width of every tab column in plan inches; 0 sizes each column to its
    /// widest cell (Reset Column Widths).
    pub tab_width: f64,
    /// Rich Text sized the word-processor way (the size is the em height)
    /// rather than CAD style (the size is the height of a capital letter).
    /// Stored; the plan draws both at the CAD size (DECISIONS TM8).
    pub word_sizing: bool,
}

impl Default for TextBox {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            halign: HAlign::Left,
            valign: VAlign::Top,
            border: false,
            margin: 1.0,
            border_weight: 0,
            background: None,
            line_spacing: 1.0,
            para_left: 0.0,
            para_right: 0.0,
            para_indent: 0.0,
            tab_width: 0.0,
            word_sizing: false,
        }
    }
}

impl TextBox {
    /// Nothing differs from a plain text.
    pub fn is_plain(&self) -> bool {
        *self == TextBox::default()
    }

    /// The text has a fixed width or height (so it has box handles).
    pub fn is_boxed(&self) -> bool {
        self.width > 0.0 || self.height > 0.0
    }

    /// Does the object need the box drawing path (anything but plain left,
    /// top, unwrapped text)?
    pub fn needs_layout(&self) -> bool {
        !self.is_plain()
    }
}

/// One line of a laid-out box, in the text's frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLine {
    pub runs: Vec<RichRun>,
    /// Left edge of the line.
    pub x: f64,
    /// Bottom of the line.
    pub y: f64,
    /// Estimated width of the line.
    pub width: f64,
    /// Line pitch.
    pub height: f64,
    /// The first line of the text (it takes the paragraph indent).
    pub first: bool,
    /// `x` is the final position of a tab column cell; renderers use it as
    /// it is instead of aligning the line.
    pub fixed: bool,
}

impl PlacedLine {
    pub fn plain(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// A laid-out text box in the text's frame (lower left corner at the origin).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxLayout {
    pub width: f64,
    pub height: f64,
    pub lines: Vec<PlacedLine>,
    /// Height of the lines alone (the box is at least this tall).
    pub content_height: f64,
}

fn run_width(r: &RichRun, base: f64) -> f64 {
    r.text.chars().count() as f64 * base * r.scale * TEXT_WIDTH_FACTOR
}

fn runs_width(runs: &[RichRun], base: f64) -> f64 {
    runs.iter().map(|r| run_width(r, base)).sum()
}

fn line_pitch(runs: &[RichRun], base: f64) -> f64 {
    let scale = runs
        .iter()
        .filter(|r| !r.text.is_empty())
        .map(|r| r.scale)
        .fold(1.0_f64, f64::max);
    base * scale * LINE_SPACING
}

/// Removes trailing spaces from the last run of a line.
fn trim_end(runs: &mut Vec<RichRun>) {
    while let Some(last) = runs.last_mut() {
        let t = last.text.trim_end_matches(' ').len();
        last.text.truncate(t);
        if last.text.is_empty() && runs.len() > 1 {
            runs.pop();
        } else {
            break;
        }
    }
}

/// The runs of `text` broken into lines: at every newline and, when
/// `width` is positive, at the last space that keeps a line within `width`
/// (a word longer than `width` is split). `base` is the text height.
pub fn wrap_runs(runs: &[RichRun], base: f64, width: f64) -> Vec<Vec<RichRun>> {
    let mut lines: Vec<Vec<RichRun>> = vec![Vec::new()];
    let mut cur_w = 0.0;
    for run in runs {
        for (i, seg) in run.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Vec::new());
                cur_w = 0.0;
            }
            for word in seg.split_inclusive(' ') {
                let mut word = word.to_string();
                loop {
                    let visible = word.trim_end_matches(' ');
                    let piece = RichRun {
                        text: word.clone(),
                        ..run.clone()
                    };
                    let w_visible =
                        visible.chars().count() as f64 * base * run.scale * TEXT_WIDTH_FACTOR;
                    let w_full = run_width(&piece, base);
                    let empty_line = lines.last().is_none_or(|l| l.is_empty());
                    if width <= 0.0 || cur_w + w_visible <= width + 1e-9 {
                        push_piece(&mut lines, piece);
                        cur_w += w_full;
                        break;
                    }
                    if !empty_line {
                        // Break before this word.
                        if let Some(l) = lines.last_mut() {
                            trim_end(l);
                        }
                        lines.push(Vec::new());
                        cur_w = 0.0;
                        continue;
                    }
                    // A word wider than the box: split it.
                    let per = base * run.scale * TEXT_WIDTH_FACTOR;
                    let fit = ((width / per).floor() as usize).max(1);
                    let chars: Vec<char> = word.chars().collect();
                    if chars.len() <= fit {
                        push_piece(&mut lines, piece);
                        cur_w += w_full;
                        break;
                    }
                    let head: String = chars[..fit].iter().collect();
                    let tail: String = chars[fit..].iter().collect();
                    push_piece(
                        &mut lines,
                        RichRun {
                            text: head,
                            ..run.clone()
                        },
                    );
                    lines.push(Vec::new());
                    cur_w = 0.0;
                    word = tail;
                }
            }
        }
    }
    for l in &mut lines {
        trim_end(l);
        merge_runs(l);
    }
    lines
}

/// Joins neighbouring runs that are formatted alike (the words of a plain
/// text come out as one run per line).
fn merge_runs(line: &mut Vec<RichRun>) {
    let mut out: Vec<RichRun> = Vec::with_capacity(line.len());
    for r in line.drain(..) {
        match out.last_mut() {
            Some(last)
                if last.bold == r.bold
                    && last.italic == r.italic
                    && last.underline == r.underline
                    && (last.scale - r.scale).abs() < 1e-9
                    && last.color == r.color
                    && last.font == r.font
                    && last.strike == r.strike
                    && last.upper == r.upper
                    && last.link == r.link =>
            {
                last.text.push_str(&r.text);
            }
            _ => out.push(r),
        }
    }
    *line = out;
}

fn push_piece(lines: &mut [Vec<RichRun>], piece: RichRun) {
    if let Some(l) = lines.last_mut() {
        l.push(piece);
    }
}

/// The cells of a line: its runs cut at every tab character.
fn split_cells(line: &[RichRun]) -> Vec<Vec<RichRun>> {
    let mut cells: Vec<Vec<RichRun>> = vec![Vec::new()];
    for r in line {
        for (i, part) in r.text.split('\t').enumerate() {
            if i > 0 {
                cells.push(Vec::new());
            }
            if !part.is_empty() {
                if let Some(c) = cells.last_mut() {
                    c.push(RichRun {
                        text: part.to_string(),
                        ..r.clone()
                    });
                }
            }
        }
    }
    cells
}

/// Lays `runs` out in the box `tb` for text of height `base`.
///
/// Paragraph Options: the line pitch follows `tb.line_spacing`; the left and
/// right margins narrow the area the lines wrap and align in; the first line
/// is indented. A text with tab characters is set in columns (one per tab
/// stop, as wide as the widest cell, or `tb.tab_width`); such text is not
/// wrapped.
pub fn layout_runs(runs: &[RichRun], base: f64, tb: &TextBox) -> BoxLayout {
    let tabbed = runs.iter().any(|r| r.text.contains('\t'));
    let (ml, mr) = (tb.para_left.max(0.0), tb.para_right.max(0.0));
    let wrap_w = if tb.width > 0.0 && !tabbed {
        (tb.width - ml - mr).max(base * TEXT_WIDTH_FACTOR)
    } else {
        0.0
    };
    let lines = wrap_runs(runs, base, wrap_w);
    // Uppercase runs show in capitals (Edit Bar).
    let lines: Vec<Vec<RichRun>> = lines
        .into_iter()
        .map(|l| {
            l.into_iter()
                .map(|mut r| {
                    if r.upper {
                        r.text = r.text.to_uppercase();
                    }
                    r
                })
                .collect()
        })
        .collect();
    let spacing = if tb.line_spacing > 0.0 {
        tb.line_spacing
    } else {
        1.0
    };
    let pitches: Vec<f64> = lines
        .iter()
        .map(|l| line_pitch(l, base) * spacing)
        .collect();
    let content: f64 = pitches.iter().sum();
    // Tab columns: the cells of every line and the width of each column.
    let cells: Vec<Vec<Vec<RichRun>>> = if tabbed {
        lines.iter().map(|l| split_cells(l)).collect()
    } else {
        Vec::new()
    };
    let gap = base * TEXT_WIDTH_FACTOR * 2.0;
    let mut col_w: Vec<f64> = Vec::new();
    for row in &cells {
        for (k, c) in row.iter().enumerate() {
            if col_w.len() <= k {
                col_w.push(0.0);
            }
            col_w[k] = col_w[k].max(runs_width(c, base));
        }
    }
    for w in &mut col_w {
        *w = if tb.tab_width > 0.0 {
            tb.tab_width
        } else {
            *w + gap
        };
    }
    let col_x = |k: usize| ml + col_w.iter().take(k).sum::<f64>();
    let widths: Vec<f64> = if tabbed {
        cells
            .iter()
            .map(|row| {
                let k = row.len().saturating_sub(1);
                col_x(k) - ml + row.last().map_or(0.0, |c| runs_width(c, base))
            })
            .collect()
    } else {
        lines.iter().map(|l| runs_width(l, base)).collect()
    };
    let indent = tb.para_indent.max(0.0);
    let block_w = if tb.width > 0.0 {
        tb.width
    } else {
        widths
            .iter()
            .enumerate()
            .map(|(i, w)| ml + w + mr + if i == 0 { indent } else { 0.0 })
            .fold(0.0, f64::max)
    };
    let block_h = tb.height.max(content);
    let free = block_h - content;
    let top_off = match tb.valign {
        VAlign::Top => 0.0,
        VAlign::Middle => free * 0.5,
        VAlign::Bottom => free,
    };
    let avail = (block_w - ml - mr).max(0.0);
    let mut y_top = block_h - top_off;
    let mut placed = Vec::with_capacity(lines.len());
    for (i, (runs, pitch)) in lines.into_iter().zip(pitches).enumerate() {
        let y = y_top - pitch;
        y_top -= pitch;
        if tabbed {
            for (k, cell) in cells[i].iter().enumerate() {
                placed.push(PlacedLine {
                    x: col_x(k) + if i == 0 && k == 0 { indent } else { 0.0 },
                    y,
                    width: runs_width(cell, base),
                    height: pitch,
                    runs: cell.clone(),
                    first: i == 0,
                    fixed: true,
                });
            }
            continue;
        }
        let w = widths[i];
        let x = match tb.halign {
            HAlign::Left => ml + if i == 0 { indent } else { 0.0 },
            HAlign::Center => ml + (avail - w) * 0.5,
            HAlign::Right => ml + avail - w,
        };
        placed.push(PlacedLine {
            runs,
            x,
            y,
            width: w,
            height: pitch,
            first: i == 0,
            fixed: false,
        });
    }
    BoxLayout {
        width: block_w,
        height: block_h,
        lines: placed,
        content_height: content,
    }
}

/// Lays out plain `text` (or `runs` when it has any) in the box `tb`.
pub fn layout(text: &str, runs: &[RichRun], base: f64, tb: &TextBox) -> BoxLayout {
    if runs.is_empty() {
        layout_runs(&[RichRun::plain(text)], base, tb)
    } else {
        layout_runs(runs, base, tb)
    }
}

impl BoxLayout {
    /// A point of the text's frame in plan coordinates for a text at `pos`
    /// turned `angle` radians.
    pub fn to_plan(pos: Point, angle: f64, local: Point) -> Point {
        let (s, c) = angle.sin_cos();
        Point::new(
            pos.x + local.x * c - local.y * s,
            pos.y + local.x * s + local.y * c,
        )
    }

    /// The four corners of the box grown by `grow` on every side, in plan
    /// coordinates, counter-clockwise from the lower left.
    pub fn corners(&self, pos: Point, angle: f64, grow: f64) -> [Point; 4] {
        let l = |x: f64, y: f64| Self::to_plan(pos, angle, Point::new(x, y));
        [
            l(-grow, -grow),
            l(self.width + grow, -grow),
            l(self.width + grow, self.height + grow),
            l(-grow, self.height + grow),
        ]
    }

    /// Axis-aligned plan bounds of the box grown by `grow`.
    pub fn bounds(&self, pos: Point, angle: f64, grow: f64) -> (Point, Point) {
        let c = self.corners(pos, angle, grow);
        let mut lo = c[0];
        let mut hi = c[0];
        for p in &c[1..] {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        (lo, hi)
    }

    /// Distance from plan point `p` to the box (0 inside), for a text at
    /// `pos` turned `angle` and grown by `grow`.
    pub fn distance(&self, pos: Point, angle: f64, grow: f64, p: Point) -> f64 {
        // Into the text's frame.
        let (s, c) = angle.sin_cos();
        let d = p.sub(pos);
        let local = Point::new(d.x * c + d.y * s, -d.x * s + d.y * c);
        let dx = (-grow - local.x)
            .max(0.0)
            .max(local.x - (self.width + grow));
        let dy = (-grow - local.y)
            .max(0.0)
            .max(local.y - (self.height + grow));
        dx.hypot(dy)
    }

    /// The right-edge midpoint (the width handle) in plan coordinates.
    pub fn width_handle(&self, pos: Point, angle: f64) -> Point {
        Self::to_plan(pos, angle, Point::new(self.width, self.height * 0.5))
    }

    /// The top-edge midpoint (the height handle).
    pub fn height_handle(&self, pos: Point, angle: f64) -> Point {
        Self::to_plan(pos, angle, Point::new(self.width * 0.5, self.height))
    }

    /// The upper right corner (both at once).
    pub fn corner_handle(&self, pos: Point, angle: f64) -> Point {
        Self::to_plan(pos, angle, Point::new(self.width, self.height))
    }
}

/// A text object with its box laid out where it stands in the plan.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedBox {
    pub pos: Point,
    pub angle: f64,
    /// The text height the lines were laid out for.
    pub text_height: f64,
    pub tb: TextBox,
    pub layout: BoxLayout,
}

impl PlacedBox {
    /// How far the frame (if any) reaches outside the box.
    pub fn grow(&self) -> f64 {
        if self.tb.border {
            self.tb.margin.max(0.0)
        } else {
            0.0
        }
    }

    /// Plan distance from `p` to the box and its frame (0 inside).
    pub fn distance(&self, p: Point) -> f64 {
        self.layout.distance(self.pos, self.angle, self.grow(), p)
    }

    /// Axis-aligned plan bounds of the box and its frame.
    pub fn bounds(&self) -> (Point, Point) {
        self.layout.bounds(self.pos, self.angle, self.grow())
    }
}

/// The laid-out box of a text item with these attributes; `None` for other
/// items and for texts that are plain (they are drawn and hit as before).
/// The text height is the item's own (callers pass the drawn item when a
/// printed-size style changes it).
pub fn placed(item: &crate::cad::CadItem, attrs: &crate::cad::CadAttrs) -> Option<PlacedBox> {
    let crate::cad::CadItem::Text {
        pos,
        text,
        height,
        angle,
    } = item
    else {
        return None;
    };
    if !attrs.text_box.needs_layout() {
        return None;
    }
    Some(PlacedBox {
        pos: *pos,
        angle: *angle,
        text_height: *height,
        tb: attrs.text_box,
        layout: layout(text, &attrs.runs, *height, &attrs.text_box),
    })
}

/// One run of a laid-out text box, placed in the plan for a renderer that
/// measures its own glyphs (the Layout sheets and the PDF writer).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnRun {
    /// Lower left of the run in plan coordinates.
    pub at: Point,
    /// Text height of the run in plan inches (the base height times its scale).
    pub height: f64,
    pub run: RichRun,
}

/// A text box ready to draw on paper: the background, the frame and the
/// runs, all in plan coordinates (the text turns by [`PlacedBox::angle`]).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxDraw {
    /// Background quadrilateral (counter-clockwise from the lower left) and
    /// its color.
    pub fill: Option<([Point; 4], [u8; 3])>,
    /// The frame, when the box has a border.
    pub border: Option<[Point; 4]>,
    pub runs: Vec<DrawnRun>,
}

impl PlacedBox {
    /// Lays the box out for a renderer: `width_of(run, height)` is the
    /// width in plan inches of the run's text set `height` tall (so
    /// alignment follows the real glyphs); an unwrapped box is as wide as
    /// its widest line.
    pub fn draw_plan(&self, width_of: &dyn Fn(&RichRun, f64) -> f64) -> BoxDraw {
        let base = self.text_height;
        let line_widths: Vec<f64> = self
            .layout
            .lines
            .iter()
            .map(|l| l.runs.iter().map(|r| width_of(r, base * r.scale)).sum())
            .collect();
        let (ml, mr) = (self.tb.para_left.max(0.0), self.tb.para_right.max(0.0));
        let box_w = if self.tb.width > 0.0 {
            self.tb.width
        } else {
            line_widths.iter().copied().fold(0.0, f64::max) + ml + mr
        };
        let box_h = self.layout.height;
        let g = self.grow();
        let quad = |g: f64| {
            let l = |x: f64, y: f64| BoxLayout::to_plan(self.pos, self.angle, Point::new(x, y));
            [
                l(-g, -g),
                l(box_w + g, -g),
                l(box_w + g, box_h + g),
                l(-g, box_h + g),
            ]
        };
        let mut runs = Vec::new();
        let avail = (box_w - ml - mr).max(0.0);
        for (line, w) in self.layout.lines.iter().zip(&line_widths) {
            let mut x = if line.fixed {
                line.x
            } else {
                match self.tb.halign {
                    HAlign::Left => {
                        ml + if line.first {
                            self.tb.para_indent.max(0.0)
                        } else {
                            0.0
                        }
                    }
                    HAlign::Center => ml + (avail - w) * 0.5,
                    HAlign::Right => ml + avail - w,
                }
            };
            for r in line.runs.iter().filter(|r| !r.text.is_empty()) {
                let height = base * r.scale;
                runs.push(DrawnRun {
                    at: BoxLayout::to_plan(self.pos, self.angle, Point::new(x, line.y)),
                    height,
                    run: r.clone(),
                });
                x += width_of(r, height);
            }
        }
        BoxDraw {
            fill: self.tb.background.map(|k| (quad(g), k)),
            border: self.tb.border.then(|| quad(g)),
            runs,
        }
    }
}

/// The text's frame coordinates of plan point `p` for a text at `pos` turned
/// `angle`.
pub fn to_local(pos: Point, angle: f64, p: Point) -> Point {
    let (s, c) = angle.sin_cos();
    let d = p.sub(pos);
    Point::new(d.x * c + d.y * s, -d.x * s + d.y * c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h() -> f64 {
        10.0
    }

    fn per_char() -> f64 {
        h() * TEXT_WIDTH_FACTOR
    }

    fn words(lines: &[Vec<RichRun>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.iter().map(|r| r.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn unwrapped_text_keeps_the_typed_lines() {
        let l = wrap_runs(&[RichRun::plain("one two\nthree")], h(), 0.0);
        assert_eq!(words(&l), ["one two", "three"]);
    }

    #[test]
    fn text_wraps_at_the_last_space_that_fits() {
        // 7 characters fit.
        let w = per_char() * 7.0;
        let l = wrap_runs(&[RichRun::plain("aaa bbb ccc dd")], h(), w);
        assert_eq!(words(&l), ["aaa bbb", "ccc dd"]);
        // A wider box holds more on a line.
        let l = wrap_runs(&[RichRun::plain("aaa bbb ccc dd")], h(), per_char() * 20.0);
        assert_eq!(words(&l), ["aaa bbb ccc dd"]);
    }

    #[test]
    fn a_word_wider_than_the_box_is_split() {
        let l = wrap_runs(&[RichRun::plain("abcdefghij")], h(), per_char() * 4.0);
        assert_eq!(words(&l), ["abcd", "efgh", "ij"]);
    }

    #[test]
    fn formats_survive_a_wrap() {
        let runs = [RichRun::bold("big words "), RichRun::plain("then plain")];
        let l = wrap_runs(&runs, h(), per_char() * 10.0);
        assert_eq!(words(&l), ["big words", "then plain"]);
        assert!(l[0][0].bold && !l[1][0].bold);
    }

    #[test]
    fn the_box_grows_in_height_with_the_text_and_keeps_its_width() {
        let tb = TextBox {
            width: per_char() * 7.0,
            ..TextBox::default()
        };
        let one = layout("aaa", &[], h(), &tb);
        let two = layout("aaa bbb ccc dd", &[], h(), &tb);
        assert_eq!(one.width, tb.width);
        assert_eq!(two.width, tb.width);
        assert!((one.height - h() * LINE_SPACING).abs() < 1e-9);
        assert!((two.height - 2.0 * h() * LINE_SPACING).abs() < 1e-9);
        // The first line is on top.
        assert!(two.lines[0].y > two.lines[1].y);
        assert!((two.lines[1].y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn alignment_places_lines_inside_the_box() {
        let mut tb = TextBox {
            width: 100.0,
            height: 60.0,
            ..TextBox::default()
        };
        let pitch = h() * LINE_SPACING;
        let lw = per_char() * 3.0;
        let at = |tb: &TextBox| layout("abc", &[], h(), tb).lines[0].clone();
        let l = at(&tb);
        assert!((l.x - 0.0).abs() < 1e-9 && (l.y - (60.0 - pitch)).abs() < 1e-9);
        tb.halign = HAlign::Center;
        tb.valign = VAlign::Middle;
        let l = at(&tb);
        assert!((l.x - (100.0 - lw) * 0.5).abs() < 1e-9);
        assert!((l.y - (60.0 - pitch) * 0.5).abs() < 1e-9);
        tb.halign = HAlign::Right;
        tb.valign = VAlign::Bottom;
        let l = at(&tb);
        assert!((l.x - (100.0 - lw)).abs() < 1e-9 && l.y.abs() < 1e-9);
    }

    #[test]
    fn a_taller_minimum_height_holds_but_the_text_can_push_past_it() {
        let tb = TextBox {
            width: per_char() * 3.0,
            height: 50.0,
            ..TextBox::default()
        };
        assert_eq!(layout("ab", &[], h(), &tb).height, 50.0);
        let big = layout("aaa bbb ccc ddd eee fff ggg", &[], h(), &tb);
        assert!(big.height > 50.0);
    }

    #[test]
    fn box_geometry_follows_the_angle() {
        let tb = TextBox {
            width: 100.0,
            height: 20.0,
            ..TextBox::default()
        };
        let lay = layout("x", &[], 5.0, &tb);
        let pos = Point::new(10.0, 10.0);
        let hnd = lay.width_handle(pos, 0.0);
        assert!(hnd.dist(Point::new(110.0, 20.0)) < 1e-9);
        let turned = lay.width_handle(pos, std::f64::consts::FRAC_PI_2);
        assert!(turned.dist(Point::new(0.0, 110.0)) < 1e-9, "{turned:?}");
        assert!(lay.distance(pos, 0.0, 0.0, Point::new(50.0, 15.0)) == 0.0);
        assert!((lay.distance(pos, 0.0, 0.0, Point::new(120.0, 15.0)) - 10.0).abs() < 1e-9);
        // The frame grows the hit area.
        assert!(lay.distance(pos, 0.0, 12.0, Point::new(120.0, 15.0)) == 0.0);
        let (lo, hi) = lay.bounds(pos, 0.0, 1.0);
        assert!(lo.dist(Point::new(9.0, 9.0)) < 1e-9 && hi.dist(Point::new(111.0, 31.0)) < 1e-9);
        let l = to_local(pos, std::f64::consts::FRAC_PI_2, Point::new(0.0, 110.0));
        assert!(l.dist(Point::new(100.0, 10.0)) < 1e-9, "{l:?}");
    }

    #[test]
    fn the_default_box_is_plain_and_serializes_compactly() {
        assert!(TextBox::default().is_plain());
        assert!(!TextBox::default().needs_layout());
        let tb = TextBox {
            width: 50.0,
            border: true,
            ..TextBox::default()
        };
        assert!(tb.is_boxed() && !tb.is_plain());
        let back: TextBox = serde_json::from_str(&serde_json::to_string(&tb).unwrap()).unwrap();
        assert_eq!(back, tb);
        let old: TextBox = serde_json::from_str("{}").unwrap();
        assert!(old.is_plain());
    }
    #[test]
    fn paragraph_options_set_the_pitch_margins_and_indent() {
        let pitch = h() * LINE_SPACING;
        let base = layout("a\nb", &[], h(), &TextBox::default());
        assert!((base.height - 2.0 * pitch).abs() < 1e-9);
        let one_half = TextBox {
            line_spacing: 1.5,
            ..TextBox::default()
        };
        let l = layout("a\nb", &[], h(), &one_half);
        assert!((l.height - 3.0 * pitch).abs() < 1e-9);
        assert!((l.lines[0].y - l.lines[1].y - 1.5 * pitch).abs() < 1e-9);
        // Margins narrow the wrap width and push the lines in.
        let w = per_char() * 10.0;
        let m = TextBox {
            width: w,
            para_left: per_char() * 2.0,
            para_right: per_char() * 1.0,
            para_indent: per_char(),
            ..TextBox::default()
        };
        // 7 characters fit between the margins.
        let l = layout("aaa bbb ccc", &[], h(), &m);
        assert_eq!(l.lines.len(), 2, "{:?}", l.lines);
        assert!((l.lines[0].x - per_char() * 3.0).abs() < 1e-9);
        assert!((l.lines[1].x - per_char() * 2.0).abs() < 1e-9);
        assert!(l.lines[0].first && !l.lines[1].first);
        // Right alignment stops at the right margin.
        let r = TextBox {
            halign: HAlign::Right,
            ..m
        };
        let l = layout("ab", &[], h(), &r);
        assert!((l.lines[0].x + per_char() * 2.0 + per_char() * 1.0 - w).abs() < 1e-9);
        // The drawn runs follow the margins too.
        let attrs = crate::cad::CadAttrs {
            text_box: m,
            ..crate::cad::CadAttrs::new(1)
        };
        let item = crate::cad::CadItem::Text {
            pos: Point::new(0.0, 0.0),
            text: "ab".into(),
            height: 10.0,
            angle: 0.0,
        };
        let d = placed(&item, &attrs)
            .unwrap()
            .draw_plan(&|r, h| r.text.chars().count() as f64 * h * 0.6);
        assert!((d.runs[0].at.x - per_char() * 3.0).abs() < 1e-9);
        // Plain boxes stay plain.
        assert!(TextBox::default().is_plain());
        assert!(!one_half.is_plain());
    }

    #[test]
    fn tab_characters_make_columns_that_reset_to_the_widest_cell() {
        let text = "Item\tQty\nWindow\t4\nDoor\t1";
        let l = layout(text, &[], h(), &TextBox::default());
        // Two cells per row, in two columns.
        assert_eq!(l.lines.len(), 6);
        let col2: Vec<f64> = l
            .lines
            .iter()
            .filter(|p| p.fixed && p.x > 0.0)
            .map(|p| p.x)
            .collect();
        assert_eq!(col2.len(), 3);
        // The second column starts after the widest first cell ("Window").
        let want = per_char() * 6.0 + per_char() * 2.0;
        assert!(
            col2.iter().all(|x| (x - want).abs() < 1e-9),
            "{col2:?} {want}"
        );
        // Rows share a baseline.
        assert!((l.lines[0].y - l.lines[1].y).abs() < 1e-9);
        // A fixed column width wins; 0 resets to auto.
        let fixed = TextBox {
            tab_width: 100.0,
            ..TextBox::default()
        };
        let l2 = layout(text, &[], h(), &fixed);
        assert!(l2
            .lines
            .iter()
            .filter(|p| p.x > 0.0)
            .all(|p| (p.x - 100.0).abs() < 1e-9));
        let reset = TextBox {
            tab_width: 0.0,
            ..fixed
        };
        assert_eq!(layout(text, &[], h(), &reset).lines, l.lines);
        // Tabbed text is not wrapped.
        let narrow = TextBox {
            width: per_char() * 3.0,
            ..TextBox::default()
        };
        assert_eq!(layout(text, &[], h(), &narrow).lines.len(), 6);
    }

    #[test]
    fn a_box_draws_fill_frame_and_aligned_runs() {
        let tb = TextBox {
            width: 100.0,
            halign: HAlign::Right,
            border: true,
            margin: 2.0,
            background: Some([250, 240, 200]),
            ..TextBox::default()
        };
        let attrs = crate::cad::CadAttrs {
            text_box: tb,
            ..crate::cad::CadAttrs::new(1)
        };
        let item = crate::cad::CadItem::Text {
            pos: Point::new(10.0, 10.0),
            text: "abcd".into(),
            height: 10.0,
            angle: 0.0,
        };
        let pb = placed(&item, &attrs).unwrap();
        // Glyphs 5 wide each: the line is 20 wide and ends at the box edge.
        let d = pb.draw_plan(&|r, h| r.text.chars().count() as f64 * h * 0.5);
        assert_eq!(d.runs.len(), 1);
        assert!(
            (d.runs[0].at.x - (10.0 + 80.0)).abs() < 1e-9,
            "{:?}",
            d.runs[0]
        );
        let frame = d.border.unwrap();
        assert!(frame[0].dist(Point::new(8.0, 8.0)) < 1e-9);
        assert!((frame[2].x - 112.0).abs() < 1e-9);
        assert_eq!(d.fill.unwrap().1, [250, 240, 200]);
        // No border, no frame; an unwrapped box is as wide as its text.
        let plain = TextBox {
            halign: HAlign::Center,
            ..TextBox::default()
        };
        let a2 = crate::cad::CadAttrs {
            text_box: plain,
            ..crate::cad::CadAttrs::new(1)
        };
        let d = placed(&item, &a2)
            .unwrap()
            .draw_plan(&|r, h| r.text.chars().count() as f64 * h * 0.5);
        assert!(d.border.is_none() && d.fill.is_none());
        assert!((d.runs[0].at.x - 10.0).abs() < 1e-9);
    }
}
