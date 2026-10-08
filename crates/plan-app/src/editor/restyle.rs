//! View > Color and View > Line Weights.
//!
//! The plan is drawn once in the palette's colors; these passes then restyle
//! the shapes a range of the painter's layer received: [`monochrome`] turns
//! every color to its gray, [`scale_widths`] multiplies stroke widths (by the
//! layer's line weight). Both work on plain [`Shape`] values, so they are
//! tested without a window.

use eframe::egui::{
    self,
    epaint::{ColorMode, PathStroke},
    layers::ShapeIdx,
    Color32, Shape, Stroke,
};

/// The reference line weight (hundredths of a millimeter): 0.25 mm draws at
/// the base width.
pub const BASE_WEIGHT: u32 = 25;

/// Stroke width multiplier of a layer's line weight (Line Weights on).
pub fn weight_factor(line_weight: u32) -> f32 {
    (line_weight as f32 / BASE_WEIGHT as f32).clamp(0.5, 4.0)
}

/// The gray of `c` (Rec. 709 luma), keeping its alpha.
pub fn gray(c: Color32) -> Color32 {
    let l = 0.2126 * f32::from(c.r()) + 0.7152 * f32::from(c.g()) + 0.0722 * f32::from(c.b());
    let l = l.round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(l, l, l, c.a())
}

fn gray_stroke(s: &mut Stroke) {
    s.color = gray(s.color);
}

fn gray_path_stroke(s: &mut PathStroke) {
    if let ColorMode::Solid(c) = &mut s.color {
        *c = gray(*c);
    }
}

/// Turns every solid color of `shape` to its gray.
pub fn monochrome(shape: &mut Shape) {
    match shape {
        Shape::Vec(v) => v.iter_mut().for_each(monochrome),
        Shape::Circle(c) => {
            c.fill = gray(c.fill);
            gray_stroke(&mut c.stroke);
        }
        Shape::Ellipse(e) => {
            e.fill = gray(e.fill);
            gray_stroke(&mut e.stroke);
        }
        Shape::LineSegment { stroke, .. } => gray_stroke(stroke),
        Shape::Path(p) => {
            p.fill = gray(p.fill);
            gray_path_stroke(&mut p.stroke);
        }
        Shape::Rect(r) => {
            r.fill = gray(r.fill);
            gray_stroke(&mut r.stroke);
        }
        Shape::Text(t) => {
            t.fallback_color = gray(t.fallback_color);
            t.override_text_color = Some(gray(t.override_text_color.unwrap_or(t.fallback_color)));
        }
        Shape::QuadraticBezier(b) => {
            b.fill = gray(b.fill);
            gray_path_stroke(&mut b.stroke);
        }
        Shape::CubicBezier(b) => {
            b.fill = gray(b.fill);
            gray_path_stroke(&mut b.stroke);
        }
        Shape::Noop | Shape::Mesh(_) | Shape::Callback(_) => {}
    }
}

/// A line or text color as printed in black and white: anything that is not
/// near-white prints black.
pub fn bw_line(c: Color32) -> Color32 {
    let l = gray(c);
    let v = if l.r() >= 235 { 255 } else { 0 };
    Color32::from_rgba_unmultiplied(v, v, v, c.a())
}

/// A fill as printed in black and white: only the dark ones print black.
pub fn bw_fill(c: Color32) -> Color32 {
    let l = gray(c);
    let v = if l.r() >= 110 { 255 } else { 0 };
    Color32::from_rgba_unmultiplied(v, v, v, c.a())
}

/// Turns every color of `shape` to pure black or white: Print Preview in
/// Black and white.
pub fn black_and_white(shape: &mut Shape) {
    fn path_stroke(s: &mut PathStroke) {
        if let ColorMode::Solid(c) = &mut s.color {
            *c = bw_line(*c);
        }
    }
    fn stroke(s: &mut Stroke) {
        s.color = bw_line(s.color);
    }
    match shape {
        Shape::Vec(v) => v.iter_mut().for_each(black_and_white),
        Shape::Circle(c) => {
            c.fill = bw_fill(c.fill);
            stroke(&mut c.stroke);
        }
        Shape::Ellipse(e) => {
            e.fill = bw_fill(e.fill);
            stroke(&mut e.stroke);
        }
        Shape::LineSegment { stroke: s, .. } => stroke(s),
        Shape::Path(p) => {
            p.fill = bw_fill(p.fill);
            path_stroke(&mut p.stroke);
        }
        Shape::Rect(r) => {
            r.fill = bw_fill(r.fill);
            stroke(&mut r.stroke);
        }
        Shape::Text(t) => {
            t.fallback_color = bw_line(t.fallback_color);
            t.override_text_color =
                Some(bw_line(t.override_text_color.unwrap_or(t.fallback_color)));
        }
        Shape::QuadraticBezier(b) => {
            b.fill = bw_fill(b.fill);
            path_stroke(&mut b.stroke);
        }
        Shape::CubicBezier(b) => {
            b.fill = bw_fill(b.fill);
            path_stroke(&mut b.stroke);
        }
        Shape::Noop | Shape::Mesh(_) | Shape::Callback(_) => {}
    }
}

/// Multiplies every stroke width of `shape` by `factor` (never below a
/// half-pixel hairline).
pub fn scale_widths(shape: &mut Shape, factor: f32) {
    let w = |width: f32| {
        if width <= 0.0 {
            0.0
        } else {
            (width * factor).max(0.5)
        }
    };
    match shape {
        Shape::Vec(v) => v.iter_mut().for_each(|s| scale_widths(s, factor)),
        Shape::Circle(c) => c.stroke.width = w(c.stroke.width),
        Shape::Ellipse(e) => e.stroke.width = w(e.stroke.width),
        Shape::LineSegment { stroke, .. } => stroke.width = w(stroke.width),
        Shape::Path(p) => p.stroke.width = w(p.stroke.width),
        Shape::Rect(r) => r.stroke.width = w(r.stroke.width),
        Shape::QuadraticBezier(b) => b.stroke.width = w(b.stroke.width),
        Shape::CubicBezier(b) => b.stroke.width = w(b.stroke.width),
        Shape::Noop | Shape::Text(_) | Shape::Mesh(_) | Shape::Callback(_) => {}
    }
}

/// The index the painter's layer will give its next shape; pass it to
/// [`restyle_from`] after drawing.
pub fn mark(painter: &egui::Painter) -> ShapeIdx {
    painter
        .ctx()
        .graphics_mut(|g| g.entry(painter.layer_id()).next_idx())
}

/// Applies `f` to every shape the painter's layer received since `from`.
pub fn restyle_from(painter: &egui::Painter, from: ShapeIdx, f: impl Fn(&mut Shape)) {
    painter.ctx().graphics_mut(|g| {
        let list = g.entry(painter.layer_id());
        let end = list.next_idx().0;
        for i in from.0..end {
            list.mutate_shape(ShapeIdx(i), |cs| f(&mut cs.shape));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect};

    #[test]
    fn gray_keeps_alpha_and_equalizes_channels() {
        let g = gray(Color32::from_rgba_unmultiplied(200, 40, 40, 128));
        assert_eq!((g.r(), g.a()), (g.g(), 128));
        assert_eq!(g.g(), g.b());
        assert_eq!(gray(Color32::WHITE), Color32::WHITE);
        assert_eq!(gray(Color32::BLACK), Color32::BLACK);
    }

    #[test]
    fn monochrome_grays_strokes_fills_and_nested_shapes() {
        let red = Color32::from_rgb(255, 0, 0);
        let mut s = Shape::Vec(vec![
            Shape::line_segment([Pos2::ZERO, Pos2::new(1.0, 1.0)], Stroke::new(1.0_f32, red)),
            Shape::rect_filled(
                Rect::from_min_max(Pos2::ZERO, Pos2::new(2.0, 2.0)),
                0.0,
                red,
            ),
            Shape::circle_stroke(Pos2::ZERO, 3.0, Stroke::new(1.0_f32, red)),
        ]);
        monochrome(&mut s);
        let Shape::Vec(v) = &s else { unreachable!() };
        let Shape::LineSegment { stroke, .. } = &v[0] else {
            unreachable!()
        };
        assert_eq!(stroke.color, gray(red));
        let Shape::Rect(r) = &v[1] else {
            unreachable!()
        };
        assert_eq!(r.fill, gray(red));
        let Shape::Circle(c) = &v[2] else {
            unreachable!()
        };
        assert_eq!(c.stroke.color, gray(red));
    }

    #[test]
    fn weights_scale_stroke_widths() {
        assert_eq!(weight_factor(25), 1.0);
        assert_eq!(weight_factor(50), 2.0);
        assert_eq!(weight_factor(13), 0.52);
        assert_eq!(weight_factor(1), 0.5);
        assert_eq!(weight_factor(1000), 4.0);
        let mut s = Shape::line_segment(
            [Pos2::ZERO, Pos2::new(1.0, 0.0)],
            Stroke::new(1.5_f32, Color32::RED),
        );
        scale_widths(&mut s, 2.0);
        let Shape::LineSegment { stroke, .. } = &s else {
            unreachable!()
        };
        assert_eq!(stroke.width, 3.0);
        // A stroke-less shape stays stroke-less.
        let mut r = Shape::rect_filled(
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            0.0,
            Color32::RED,
        );
        scale_widths(&mut r, 3.0);
        let Shape::Rect(r) = &r else { unreachable!() };
        assert_eq!(r.stroke.width, 0.0);
    }

    #[test]
    fn restyle_from_touches_only_shapes_after_the_mark() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let painter = ui.painter().clone();
                painter.line_segment(
                    [Pos2::ZERO, Pos2::new(1.0, 0.0)],
                    Stroke::new(1.0_f32, Color32::RED),
                );
                let m = mark(&painter);
                painter.line_segment(
                    [Pos2::ZERO, Pos2::new(1.0, 0.0)],
                    Stroke::new(1.0_f32, Color32::RED),
                );
                restyle_from(&painter, m, monochrome);
                let mut seen = Vec::new();
                painter.for_each_shape(|cs| {
                    if let Shape::LineSegment { stroke, .. } = &cs.shape {
                        seen.push(stroke.color);
                    }
                });
                assert_eq!(seen, vec![Color32::RED, gray(Color32::RED)]);
            });
        });
    }

    #[test]
    fn black_and_white_prints_dark_things_black_and_light_things_white() {
        let mut s = Shape::Vec(vec![
            Shape::line_segment(
                [Pos2::ZERO, Pos2::new(1.0, 1.0)],
                Stroke::new(1.0_f32, Color32::from_rgb(200, 40, 40)),
            ),
            Shape::rect_filled(
                Rect::from_min_max(Pos2::ZERO, Pos2::new(2.0, 2.0)),
                0.0,
                Color32::from_rgb(204, 204, 204),
            ),
            Shape::rect_filled(
                Rect::from_min_max(Pos2::ZERO, Pos2::new(2.0, 2.0)),
                0.0,
                Color32::from_rgb(30, 30, 30),
            ),
            Shape::circle_stroke(Pos2::ZERO, 3.0, Stroke::new(1.0_f32, Color32::WHITE)),
        ]);
        black_and_white(&mut s);
        let Shape::Vec(v) = &s else { unreachable!() };
        let Shape::LineSegment { stroke, .. } = &v[0] else {
            unreachable!()
        };
        assert_eq!(stroke.color, Color32::BLACK);
        let fill = |i: usize| match &v[i] {
            Shape::Rect(r) => r.fill,
            _ => unreachable!(),
        };
        assert_eq!(fill(1), Color32::WHITE, "a light fill prints white");
        assert_eq!(fill(2), Color32::BLACK);
        let Shape::Circle(c) = &v[3] else {
            unreachable!()
        };
        assert_eq!(c.stroke.color, Color32::WHITE);
        // Alpha survives.
        assert_eq!(
            bw_line(Color32::from_rgba_unmultiplied(9, 9, 9, 77)).a(),
            77
        );
    }
}
