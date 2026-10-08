//! Drawing primitives in PDF points, and how they reach a [`PdfDoc`].
//!
//! A box (or page) is first turned into a list of [`Prim`]s. Clipped boxes
//! are bracketed by [`Prim::ClipBegin`] / [`Prim::ClipEnd`], which [`emit`]
//! writes as a PDF clip rectangle (`q ... re W n` / `Q`). [`soft_clip`]
//! applies the same clip in software, for callers that inspect geometry
//! ([`crate::render_box_lines`]) rather than print it.

use crate::clip::{clip_polygon, clip_segment, contains, Pt, Rect};
use plan_core::LineStyle;
use plan_docs::{PdfColor, PdfDoc};

pub(crate) const BLACK: PdfColor = PdfColor::Gray(0.0);

pub(crate) fn gray(g: f64) -> PdfColor {
    PdfColor::Gray(g)
}

/// Dash pattern of a stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dash {
    Solid,
    Dashed,
    Dotted,
    /// Dash-dot.
    LongShort,
    /// Hidden lines in elevations (short dashes).
    Hidden,
}

impl Dash {
    /// On/off lengths in points; empty for solid.
    pub(crate) fn pattern(self) -> &'static [f64] {
        match self {
            Dash::Solid => &[],
            Dash::Dashed => &[6.0, 3.0],
            Dash::Dotted => &[1.0, 3.0],
            Dash::LongShort => &[6.0, 2.0, 1.0, 2.0],
            Dash::Hidden => &[3.0, 2.0],
        }
    }
}

impl From<LineStyle> for Dash {
    fn from(s: LineStyle) -> Dash {
        match s {
            LineStyle::Solid => Dash::Solid,
            LineStyle::Dashed => Dash::Dashed,
            LineStyle::Dotted => Dash::Dotted,
            LineStyle::DashDot => Dash::LongShort,
        }
    }
}

/// Stroke width (points), colour and dash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pen {
    pub width: f64,
    pub color: PdfColor,
    pub dash: Dash,
}

impl Pen {
    pub(crate) fn new(width: f64) -> Pen {
        Pen {
            width,
            color: BLACK,
            dash: Dash::Solid,
        }
    }

    pub(crate) fn gray(width: f64, g: f64) -> Pen {
        Pen {
            color: gray(g),
            ..Pen::new(width)
        }
    }

    /// The same pen with `width * f`.
    pub(crate) fn scaled(self, f: f64) -> Pen {
        Pen {
            width: self.width * f,
            ..self
        }
    }

    pub(crate) fn solid(self) -> Pen {
        Pen {
            dash: Dash::Solid,
            ..self
        }
    }
}

/// A drawing primitive in PDF points.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Prim {
    Stroke {
        pts: Vec<Pt>,
        closed: bool,
        pen: Pen,
    },
    Fill {
        pts: Vec<Pt>,
        color: PdfColor,
    },
    Text {
        x: f64,
        y: f64,
        size: f64,
        color: PdfColor,
        bold: bool,
        /// Counter-clockwise radians about `(x, y)`.
        angle: f64,
        text: String,
    },
    /// RGBA pixels (`px.0 * px.1 * 4` bytes) scaled into `rect`.
    Image {
        rect: Rect,
        px: (u32, u32),
        rgba: Vec<u8>,
    },
    /// Start clipping to `Rect` (until the matching [`Prim::ClipEnd`]).
    ClipBegin(Rect),
    ClipEnd,
}

/// `p` turned `turns` quarter turns counter-clockwise about `c`.
fn turn_point(p: Pt, c: Pt, turns: u8) -> Pt {
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    match turns % 4 {
        0 => p,
        1 => (c.0 - dy, c.1 + dx),
        2 => (c.0 - dx, c.1 - dy),
        _ => (c.0 + dy, c.1 - dx),
    }
}

/// RGBA pixels (`w * h * 4`, rows top to bottom) turned counter-clockwise.
fn turn_pixels(rgba: &[u8], (w, h): (u32, u32), turns: u8) -> (Vec<u8>, (u32, u32)) {
    let (wu, hu) = (w as usize, h as usize);
    if rgba.len() != wu * hu * 4 {
        return (rgba.to_vec(), (w, h));
    }
    let (nw, nh) = if turns % 2 == 1 { (hu, wu) } else { (wu, hu) };
    let mut out = vec![0u8; rgba.len()];
    for y in 0..hu {
        for x in 0..wu {
            let (nx, ny) = match turns % 4 {
                0 => (x, y),
                1 => (y, wu - 1 - x),
                2 => (wu - 1 - x, hu - 1 - y),
                _ => (hu - 1 - y, x),
            };
            let (from, to) = ((y * wu + x) * 4, (ny * nw + nx) * 4);
            out[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    (out, (nw as u32, nh as u32))
}

/// Turns `prims` `turns` quarter turns counter-clockwise about `c` (text
/// turns with its position, images keep their pixels upright in the turned
/// frame). Used for a box whose content is rotated.
pub(crate) fn rotate_prims(prims: &mut [Prim], c: Pt, turns: u8) {
    if turns.is_multiple_of(4) {
        return;
    }
    let angle = f64::from(turns % 4) * std::f64::consts::FRAC_PI_2;
    for p in prims {
        match p {
            Prim::Stroke { pts, .. } | Prim::Fill { pts, .. } => {
                for q in pts.iter_mut() {
                    *q = turn_point(*q, c, turns);
                }
            }
            Prim::Text { x, y, angle: a, .. } => {
                let (nx, ny) = turn_point((*x, *y), c, turns);
                *x = nx;
                *y = ny;
                *a += angle;
            }
            Prim::Image { rect, px, rgba } => {
                let (turned, size) = turn_pixels(rgba, *px, turns);
                *rgba = turned;
                *px = size;
                *rect = turn_rect(*rect, c, turns);
            }
            Prim::ClipBegin(rect) => *rect = turn_rect(*rect, c, turns),
            Prim::ClipEnd => {}
        }
    }
}

fn turn_rect(r: Rect, c: Pt, turns: u8) -> Rect {
    let a = turn_point((r[0], r[1]), c, turns);
    let b = turn_point((r[2], r[3]), c, turns);
    [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
}

/// Collects primitives.
#[derive(Default)]
pub(crate) struct Canvas {
    pub prims: Vec<Prim>,
}

impl Canvas {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn stroke(&mut self, pts: &[Pt], closed: bool, pen: Pen) {
        if pts.len() >= 2 {
            self.prims.push(Prim::Stroke {
                pts: pts.to_vec(),
                closed,
                pen,
            });
        }
    }

    pub(crate) fn line(&mut self, a: Pt, b: Pt, pen: Pen) {
        self.stroke(&[a, b], false, pen);
    }

    pub(crate) fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, pen: Pen) {
        self.stroke(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true, pen);
    }

    pub(crate) fn fill(&mut self, pts: &[Pt], color: PdfColor) {
        if pts.len() >= 3 {
            self.prims.push(Prim::Fill {
                pts: pts.to_vec(),
                color,
            });
        }
    }

    /// Text with its baseline-left at `(x, y)`.
    pub(crate) fn text_full(
        &mut self,
        (x, y): Pt,
        size: f64,
        color: PdfColor,
        bold: bool,
        angle: f64,
        text: &str,
    ) {
        if !text.is_empty() {
            self.prims.push(Prim::Text {
                x,
                y,
                size,
                color,
                bold,
                angle,
                text: text.to_string(),
            });
        }
    }

    pub(crate) fn text(&mut self, x: f64, y: f64, size: f64, color: PdfColor, text: &str) {
        self.text_full((x, y), size, color, false, 0.0, text);
    }

    /// Black bold text.
    pub(crate) fn bold(&mut self, x: f64, y: f64, size: f64, text: &str) {
        self.text_full((x, y), size, BLACK, true, 0.0, text);
    }

    pub(crate) fn text_centered(
        &mut self,
        cx: f64,
        y: f64,
        size: f64,
        color: PdfColor,
        bold: bool,
        text: &str,
    ) {
        let w = text_w(text, size, bold);
        self.text_full((cx - w * 0.5, y), size, color, bold, 0.0, text);
    }

    pub(crate) fn text_right(&mut self, rx: f64, y: f64, size: f64, color: PdfColor, text: &str) {
        let w = text_w(text, size, false);
        self.text_full((rx - w, y), size, color, false, 0.0, text);
    }
}

pub(crate) fn text_w(text: &str, size: f64, bold: bool) -> f64 {
    if bold {
        PdfDoc::text_width_bold(text, size)
    } else {
        PdfDoc::text_width(text, size)
    }
}

// ----------------------------------------------------------- soft clip --

/// Corners of the (possibly rotated) text box.
fn text_corners(x: f64, y: f64, size: f64, angle: f64, w: f64) -> [Pt; 4] {
    let (s, c) = angle.sin_cos();
    let at = |dx: f64, dy: f64| (x + dx * c - dy * s, y + dx * s + dy * c);
    [
        at(0.0, -size * 0.25),
        at(w, -size * 0.25),
        at(w, size),
        at(0.0, size),
    ]
}

/// Apply every clip region in `prims` in software and drop the clip markers.
///
/// Strokes are cut at the rectangle, fills clipped, text kept only when it
/// fits whole and images when they overlap the rectangle.
pub(crate) fn soft_clip(prims: Vec<Prim>) -> Vec<Prim> {
    let mut out = Vec::with_capacity(prims.len());
    let mut clip: Option<Rect> = None;
    for p in prims {
        match (p, clip) {
            (Prim::ClipBegin(r), _) => clip = Some(r),
            (Prim::ClipEnd, _) => clip = None,
            (p, None) => out.push(p),
            (Prim::Stroke { pts, closed, pen }, Some(r)) => {
                if pts.iter().all(|&p| contains(r, p)) {
                    out.push(Prim::Stroke { pts, closed, pen });
                    continue;
                }
                let n = if closed { pts.len() } else { pts.len() - 1 };
                for i in 0..n {
                    let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                    if let Some((a, b)) = clip_segment(r, a, b) {
                        out.push(Prim::Stroke {
                            pts: vec![a, b],
                            closed: false,
                            pen,
                        });
                    }
                }
            }
            (Prim::Fill { pts, color }, Some(r)) => {
                let pts = clip_polygon(r, &pts);
                if pts.len() >= 3 {
                    out.push(Prim::Fill { pts, color });
                }
            }
            (
                Prim::Text {
                    x,
                    y,
                    size,
                    color,
                    bold,
                    angle,
                    text,
                },
                Some(r),
            ) => {
                let w = text_w(&text, size, bold);
                if text_corners(x, y, size, angle, w)
                    .iter()
                    .all(|&c| contains(r, c))
                {
                    out.push(Prim::Text {
                        x,
                        y,
                        size,
                        color,
                        bold,
                        angle,
                        text,
                    });
                }
            }
            (Prim::Image { rect, px, rgba }, Some(r)) => {
                if rect[0] < r[2] && r[0] < rect[2] && rect[1] < r[3] && r[1] < rect[3] {
                    out.push(Prim::Image { rect, px, rgba });
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------- emitting --

/// What the PDF writer's graphics state is known to hold.
#[derive(Clone, Copy, Default)]
struct Tracked {
    stroke: Option<PdfColor>,
    fill: Option<PdfColor>,
    dash: Option<Dash>,
}

/// Write `prims` to the current page of `doc`.
pub(crate) fn emit(doc: &mut PdfDoc, prims: &[Prim]) {
    let mut st = Tracked::default();
    let mut saved: Vec<Tracked> = Vec::new();
    for p in prims {
        match p {
            Prim::ClipBegin(r) => {
                doc.save_state();
                saved.push(st);
                doc.clip_rect(r[0], r[1], r[2] - r[0], r[3] - r[1]);
            }
            Prim::ClipEnd => {
                doc.restore_state();
                if let Some(prev) = saved.pop() {
                    st = prev;
                }
            }
            Prim::Stroke { pts, closed, pen } => {
                if st.stroke != Some(pen.color) {
                    doc.set_stroke_color(pen.color);
                    st.stroke = Some(pen.color);
                }
                if st.dash != Some(pen.dash) {
                    doc.set_dash(pen.dash.pattern(), 0.0);
                    st.dash = Some(pen.dash);
                }
                if pts.len() == 2 && !closed {
                    doc.line(pts[0].0, pts[0].1, pts[1].0, pts[1].1, pen.width);
                } else {
                    doc.polyline(pts, *closed, pen.width);
                }
            }
            Prim::Fill { pts, color } => doc.polygon(pts, Some(*color), None),
            Prim::Text {
                x,
                y,
                size,
                color,
                bold,
                angle,
                text,
            } => {
                if st.fill != Some(*color) {
                    doc.set_fill_color(*color);
                    st.fill = Some(*color);
                }
                doc.set_font_bold(*bold);
                if angle.abs() < 1e-9 {
                    doc.text(*x, *y, *size, text);
                } else {
                    doc.text_rotated(*x, *y, *size, angle.to_degrees(), text);
                }
            }
            Prim::Image { rect, px, rgba } => {
                doc.image_rgba(
                    rect[0],
                    rect[1],
                    rect[2] - rect[0],
                    rect[3] - rect[1],
                    px.0,
                    px.1,
                    rgba,
                );
            }
        }
    }
    doc.set_font_bold(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_clip_cuts_strokes_and_drops_markers() {
        let r = [0.0, 0.0, 10.0, 10.0];
        let prims = vec![
            Prim::ClipBegin(r),
            Prim::Stroke {
                pts: vec![(-5.0, 5.0), (15.0, 5.0)],
                closed: false,
                pen: Pen::new(1.0),
            },
            Prim::ClipEnd,
            Prim::Stroke {
                pts: vec![(-5.0, 20.0), (15.0, 20.0)],
                closed: false,
                pen: Pen::new(1.0),
            },
        ];
        let out = soft_clip(prims);
        assert_eq!(out.len(), 2);
        assert!(matches!(
            &out[0],
            Prim::Stroke { pts, .. } if pts == &vec![(0.0, 5.0), (10.0, 5.0)]
        ));
        assert!(matches!(
            &out[1],
            Prim::Stroke { pts, .. } if pts[0] == (-5.0, 20.0)
        ));
    }

    #[test]
    fn rotated_text_must_fit_whole() {
        let r = [0.0, 0.0, 10.0, 10.0];
        let t = |y: f64| Prim::Text {
            x: 5.0,
            y,
            size: 6.0,
            color: BLACK,
            bold: false,
            angle: std::f64::consts::FRAC_PI_2,
            text: "ABCDEFGH".into(),
        };
        // About 27 pt long when vertical: never fits a 10 pt box.
        assert!(soft_clip(vec![Prim::ClipBegin(r), t(0.0)]).is_empty());
    }
}
