//! Drawing a CAD object with its own look (`plan_core::cad::CadAttrs`): its
//! color, weight and dash style, a solid fill, arrow ends on a line or open
//! polyline, and rich text runs (bold, italic, underline, size scale, color).
//!
//! `editor/render.rs` draws every CAD object with the layer's look and does
//! not call this yet; the integration step is in `docs/integration-queue.md`
//! (one call per CAD object that has attributes).

use super::*;
use crate::editor::restyle;
use eframe::egui::Color32;
use plan_core::cad::{ArrowStyle, CadAttrs, CadObject};
use plan_core::layers::LineStyle;
use plan_core::text_styles::RichRun;

/// Hatch patterns are real lines; only solid fills are painted here.
const DASH: (f32, f32) = (8.0, 4.0);
const DOT: (f32, f32) = (1.5, 3.5);

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The outline of a shape in screen points, for dashing.
fn screen_path(cam: &Camera, item: &CadItem) -> Option<(Vec<Pos2>, bool)> {
    let sc = |p: Point| cam.world_to_screen(p);
    match item {
        CadItem::Line { a, b } => Some((vec![sc(*a), sc(*b)], false)),
        CadItem::Polyline { points, closed } => {
            Some((points.iter().map(|p| sc(*p)).collect(), *closed))
        }
        CadItem::Circle { center, radius } => Some((
            (0..64)
                .map(|i| {
                    let a = TAU * f64::from(i) / 64.0;
                    sc(Point::new(
                        center.x + radius * a.cos(),
                        center.y + radius * a.sin(),
                    ))
                })
                .collect(),
            true,
        )),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            Some((
                (0..=48)
                    .map(|i| {
                        let a = start_angle + sweep * f64::from(i) / 48.0;
                        sc(Point::new(
                            center.x + radius * a.cos(),
                            center.y + radius * a.sin(),
                        ))
                    })
                    .collect(),
                false,
            ))
        }
        CadItem::Text { .. } => None,
    }
}

/// An arrow end: `tip` is the end of the line, `from` a point along it.
pub fn draw_arrow_end(
    painter: &Painter,
    tip: Pos2,
    from: Pos2,
    style: ArrowStyle,
    size: f32,
    stroke: Stroke,
) {
    let dir = (from - tip).normalized();
    if !dir.is_finite() || style == ArrowStyle::None {
        return;
    }
    let perp = Vec2::new(-dir.y, dir.x);
    let base = tip + dir * size;
    match style {
        ArrowStyle::None => {}
        ArrowStyle::Open => {
            painter.line_segment([tip, base + perp * size * 0.3], stroke);
            painter.line_segment([tip, base - perp * size * 0.3], stroke);
        }
        ArrowStyle::Filled => {
            let _ = painter.add(Shape::convex_polygon(
                vec![tip, base + perp * size * 0.3, base - perp * size * 0.3],
                stroke.color,
                Stroke::NONE,
            ));
        }
        ArrowStyle::Tick => {
            let c = tip + dir * size * 0.2;
            let d = (dir + perp).normalized() * size * 0.5;
            painter.line_segment([c - d, c + d], stroke);
        }
        ArrowStyle::Dot => {
            painter.circle_filled(tip, size * 0.25, stroke.color);
        }
    }
}

/// Text drawn run by run, each in its own size, color and weight.
fn draw_runs(
    painter: &Painter,
    cam: &Camera,
    pos: Point,
    height: f64,
    runs: &[RichRun],
    pal: &Palette,
) {
    let base = (height * cam.px_per_in) as f32;
    let at = cam.world_to_screen(pos);
    let mut x = at.x;
    for r in runs {
        let px = (base * r.scale as f32).clamp(6.0, 200.0);
        let color = r.color.map_or(pal.text, rgb);
        let galley = painter.layout_no_wrap(r.text.clone(), FontId::proportional(px), color);
        let rect = Align2::LEFT_BOTTOM.anchor_size(Pos2::new(x, at.y), galley.size());
        if r.bold {
            painter.galley(rect.min + Vec2::new(0.7, 0.0), galley.clone(), color);
        }
        painter.galley(rect.min, galley.clone(), color);
        if r.underline {
            let y = rect.max.y - 1.0;
            painter.line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                Stroke::new(1.0_f32, color),
            );
        }
        x += galley.size().x;
    }
}

/// Draws `c` with the look in `attrs` (the layer's where it has none).
pub fn draw_cad_styled(
    cx: &EditorContext,
    painter: &Painter,
    cam: &Camera,
    c: &CadObject,
    attrs: Option<&CadAttrs>,
) {
    let pal = &cx.palette;
    let default = CadAttrs::default();
    let a = attrs.unwrap_or(&default);
    let layer = cx.layers().get(&c.layer);
    let color = a
        .color
        .map_or_else(|| layer.map_or(pal.text, |_| pal.text), rgb);
    let weight = a
        .weight
        .or_else(|| layer.map(|l| l.line_weight))
        .unwrap_or(restyle::BASE_WEIGHT);
    let k = if cx.view_flags.contains(&ViewFlag::LineWeights) {
        restyle::weight_factor(weight)
    } else {
        1.0
    };
    let stroke = Stroke::new(k, color);
    // A solid fill goes under the outline.
    if let Some(f) = a.fill.as_ref().filter(|f| f.pattern.is_empty()) {
        let fill = Color32::from_rgba_unmultiplied(f.color[0], f.color[1], f.color[2], f.opacity);
        match &c.item {
            CadItem::Polyline {
                points,
                closed: true,
            } => {
                let pts = points.iter().map(|p| cam.world_to_screen(*p)).collect();
                painter.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
            }
            CadItem::Circle { center, radius } => {
                painter.circle_filled(
                    cam.world_to_screen(*center),
                    (*radius * cam.px_per_in) as f32,
                    fill,
                );
            }
            _ => {}
        }
    }
    match &c.item {
        CadItem::Text {
            pos, text, height, ..
        } => {
            if a.runs.is_empty() {
                render::draw_cad(painter, cam, &c.item, stroke, pal);
            } else {
                draw_runs(painter, cam, *pos, *height, &a.runs, pal);
            }
            let _ = text;
        }
        item => match (a.dash, screen_path(cam, item)) {
            (Some(style), Some((pts, closed))) if style != LineStyle::Solid => {
                let mut pts = pts;
                if closed {
                    if let Some(first) = pts.first().copied() {
                        pts.push(first);
                    }
                }
                let (dash, gap) = match style {
                    LineStyle::Dotted => DOT,
                    LineStyle::DashDot => (DASH.0, DOT.1 + 2.0),
                    _ => DASH,
                };
                painter.extend(Shape::dashed_line(
                    &pts,
                    stroke,
                    dash * k.max(1.0),
                    gap * k.max(1.0),
                ));
            }
            _ => render::draw_cad(painter, cam, item, stroke, pal),
        },
    }
    // Arrow ends of a line or an open polyline.
    if a.arrow_start != ArrowStyle::None || a.arrow_end != ArrowStyle::None {
        let size = (if a.arrow_size > 0.0 {
            a.arrow_size
        } else {
            6.0
        } * cam.px_per_in) as f32;
        let ends: Option<(Pos2, Pos2, Pos2, Pos2)> = match &c.item {
            CadItem::Line { a: p, b: q } => {
                let (s, e) = (cam.world_to_screen(*p), cam.world_to_screen(*q));
                Some((s, e, e, s))
            }
            CadItem::Polyline {
                points,
                closed: false,
            } if points.len() >= 2 => {
                let n = points.len();
                let sc = |i: usize| cam.world_to_screen(points[i]);
                Some((sc(0), sc(1), sc(n - 1), sc(n - 2)))
            }
            _ => None,
        };
        if let Some((s_tip, s_from, e_tip, e_from)) = ends {
            draw_arrow_end(painter, s_tip, s_from, a.arrow_start, size, stroke);
            draw_arrow_end(painter, e_tip, e_from, a.arrow_end, size, stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::cad::{FillAttr, CAD_DATA_LAYER};

    #[test]
    fn every_look_draws_without_panicking() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.view_flags.insert(ViewFlag::LineWeights);
        let line = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(120.0, 0.0),
            },
        );
        let sq = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Polyline {
                points: vec![
                    Point::ZERO,
                    Point::new(60.0, 0.0),
                    Point::new(60.0, 60.0),
                    Point::new(0.0, 60.0),
                ],
                closed: true,
            },
        );
        let circle = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Circle {
                center: Point::new(200.0, 0.0),
                radius: 30.0,
            },
        );
        let arc = cx.project.add_cad(
            0,
            CAD_LAYER,
            CadItem::Arc {
                center: Point::ZERO,
                radius: 10.0,
                start_angle: 0.0,
                end_angle: 2.0,
            },
        );
        let text = cx.project.add_cad(
            0,
            "Text",
            CadItem::Text {
                pos: Point::new(0.0, 100.0),
                text: "Bold and big".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        let all = |cx: &mut EditorContext, id: Id, edit: &dyn Fn(&mut CadAttrs)| {
            cx.project.edit_cad_attrs(0, id, |a| edit(a));
        };
        all(&mut cx, line, &|a| {
            a.color = Some([200, 0, 0]);
            a.weight = Some(70);
            a.dash = Some(LineStyle::Dashed);
            a.arrow_start = ArrowStyle::Tick;
            a.arrow_end = ArrowStyle::Filled;
        });
        all(&mut cx, sq, &|a| {
            a.dash = Some(LineStyle::DashDot);
            a.fill = Some(FillAttr::default());
        });
        all(&mut cx, circle, &|a| {
            a.dash = Some(LineStyle::Dotted);
            a.fill = Some(FillAttr::default());
        });
        all(&mut cx, arc, &|a| a.dash = Some(LineStyle::Dashed));
        all(&mut cx, text, &|a| {
            a.runs = vec![
                RichRun::bold("Bold"),
                RichRun::plain(" and "),
                RichRun {
                    underline: true,
                    scale: 1.5,
                    color: Some([0, 0, 200]),
                    ..RichRun::plain("big")
                },
            ];
        });
        for style in ArrowStyle::ALL {
            all(&mut cx, line, &|a| a.arrow_end = style);
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, painter) =
                        ui.allocate_painter(Vec2::new(800.0, 600.0), egui::Sense::hover());
                    let mut cam = Camera::default_view();
                    cam.rect = painter.clip_rect();
                    let attrs = cx.floor().cad_attr_map();
                    for c in cx.floor().cad.iter().filter(|c| c.layer != CAD_DATA_LAYER) {
                        draw_cad_styled(&cx, &painter, &cam, c, attrs.get(&c.id));
                    }
                    // Objects without a look draw too.
                    let plain = CadObject {
                        id: 999,
                        layer: CAD_LAYER.into(),
                        item: CadItem::Line {
                            a: Point::ZERO,
                            b: Point::new(5.0, 5.0),
                        },
                    };
                    draw_cad_styled(&cx, &painter, &cam, &plain, None);
                });
            });
        }
    }
}
