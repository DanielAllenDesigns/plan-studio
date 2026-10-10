//! The look a CAD object carries beyond its layer (`plan_core::cad::CadAttrs`)
//! on a layout page: its fill, colour, weight, dash and the arrow ends of a
//! line, so callouts, markers, notes and arrows print as they show in the
//! plan.

use crate::canvas::{Canvas, Dash, Pen};
use crate::clip::Pt;
use plan_core::cad::{ArrowStyle, CadAttrs, CadItem, CadObject};
use plan_core::{LineStyle, Point};
use plan_docs::PdfColor;
use std::f64::consts::TAU;

/// 1/100 mm to points.
fn hundredths_mm_to_pt(h: f64) -> f64 {
    h / 100.0 * 72.0 / 25.4
}

/// Plan inches of an arrow head whose attributes give no size.
const DEFAULT_ARROW_IN: f64 = 4.0;

/// The pen for `o` with the colour, weight and dash of `a` over `base`.
pub(crate) fn pen_for(a: &CadAttrs, base: Pen) -> Pen {
    let mut pen = base;
    if let Some(c) = a.color {
        pen.color = PdfColor::Rgb(c[0], c[1], c[2]);
    }
    if let Some(w) = a.weight {
        pen.width = hundredths_mm_to_pt(f64::from(w)).max(0.1);
    }
    if let Some(d) = a.dash {
        pen.dash = match d {
            LineStyle::Solid => Dash::Solid,
            LineStyle::Dashed => Dash::Dashed,
            LineStyle::Dotted => Dash::Dotted,
            LineStyle::DashDot => Dash::LongShort,
        };
    }
    pen
}

/// Draws the solid fill of a closed shape under its outline.
pub(crate) fn draw_fill(cv: &mut Canvas, o: &CadObject, a: &CadAttrs, tp: &dyn Fn(Point) -> Pt) {
    let Some(f) = a.fill.as_ref().filter(|f| f.pattern.is_empty()) else {
        return;
    };
    // The paper is white under the fill: transparency mixes toward it.
    let alpha = f64::from(f.opacity) / 255.0;
    let mix = |c: u8| (255.0 + (f64::from(c) - 255.0) * alpha).round() as u8;
    let color = PdfColor::Rgb(mix(f.color[0]), mix(f.color[1]), mix(f.color[2]));
    let pts: Vec<Pt> = match &o.item {
        CadItem::Polyline {
            points,
            closed: true,
        } => points.iter().map(|&p| tp(p)).collect(),
        CadItem::Circle { center, radius } => (0..48)
            .map(|i| {
                let t = TAU * f64::from(i) / 48.0;
                tp(*center + Point::new(t.cos(), t.sin()) * *radius)
            })
            .collect(),
        _ => return,
    };
    cv.fill(&pts, color);
}

/// Draws the arrow ends of a line or an open polyline.
pub(crate) fn draw_arrows(
    cv: &mut Canvas,
    o: &CadObject,
    a: &CadAttrs,
    tp: &dyn Fn(Point) -> Pt,
    pen: Pen,
) {
    if a.arrow_start == ArrowStyle::None && a.arrow_end == ArrowStyle::None {
        return;
    }
    let pts: Vec<Point> = match &o.item {
        CadItem::Line { a, b } => vec![*a, *b],
        CadItem::Polyline {
            points,
            closed: false,
        } => points.clone(),
        _ => return,
    };
    if pts.len() < 2 {
        return;
    }
    let size = if a.arrow_size > 0.0 {
        a.arrow_size
    } else {
        DEFAULT_ARROW_IN
    };
    let n = pts.len();
    for (style, tip, from) in [
        (a.arrow_start, pts[0], pts[1]),
        (a.arrow_end, pts[n - 1], pts[n - 2]),
    ] {
        if style == ArrowStyle::None {
            continue;
        }
        let d = tip.sub(from);
        if d.length() < 1e-9 {
            continue;
        }
        let d = d.normalized();
        let n = d.perp();
        match style {
            ArrowStyle::Dot => {
                let ring: Vec<Pt> = (0..16)
                    .map(|i| {
                        let t = TAU * f64::from(i) / 16.0;
                        tp(tip + Point::new(t.cos(), t.sin()) * (size * 0.3))
                    })
                    .collect();
                cv.fill(&ring, pen.color);
            }
            ArrowStyle::Tick => {
                cv.line(
                    tp(tip + n * (size * 0.5) - d * (size * 0.5)),
                    tp(tip - n * (size * 0.5) + d * (size * 0.5)),
                    pen,
                );
            }
            ArrowStyle::Open | ArrowStyle::Filled | ArrowStyle::None => {
                let back = tip - d * size;
                let head = [
                    tp(tip),
                    tp(back + n * (size * 0.3)),
                    tp(back - n * (size * 0.3)),
                ];
                if style == ArrowStyle::Filled {
                    cv.fill(&head, pen.color);
                } else {
                    cv.stroke(&[head[1], head[0], head[2]], false, pen);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Prim;
    use plan_core::cad::FillAttr;

    fn tp(p: Point) -> Pt {
        (p.x, p.y)
    }

    #[test]
    fn a_filled_shape_a_dash_and_an_arrow_reach_the_page() {
        let o = CadObject {
            id: 1,
            layer: "Text".into(),
            item: CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(10.0, 0.0),
                    Point::new(10.0, 10.0),
                ],
                closed: true,
            },
        };
        let a = CadAttrs {
            fill: Some(FillAttr {
                color: [10, 20, 30],
                opacity: 255,
                ..FillAttr::default()
            }),
            dash: Some(LineStyle::Dashed),
            color: Some([200, 0, 0]),
            ..CadAttrs::default()
        };
        let mut cv = Canvas::new();
        draw_fill(&mut cv, &o, &a, &tp);
        assert!(matches!(
            cv.prims[0],
            Prim::Fill {
                color: PdfColor::Rgb(10, 20, 30),
                ..
            }
        ));
        let pen = pen_for(&a, Pen::new(0.5));
        assert_eq!(pen.dash, Dash::Dashed);
        assert_eq!(pen.color, PdfColor::Rgb(200, 0, 0));
        // An arrow end of an open line is a filled head.
        let line = CadObject {
            id: 2,
            layer: "Text".into(),
            item: CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(20.0, 0.0),
            },
        };
        let arrow = CadAttrs {
            arrow_start: ArrowStyle::Filled,
            arrow_size: 4.0,
            ..CadAttrs::default()
        };
        let mut cv = Canvas::new();
        draw_arrows(&mut cv, &line, &arrow, &tp, pen);
        let Prim::Fill { pts, .. } = &cv.prims[0] else {
            panic!("a filled head")
        };
        // The tip at the start, the base 4 back along the line.
        assert_eq!(pts[0], (0.0, 0.0));
        assert!((pts[1].0 - 4.0).abs() < 1e-9);
    }
}
