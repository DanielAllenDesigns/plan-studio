//! Drawing a laid-out schedule: the tables, their text, the object preview
//! pictures, the callout shapes and the edit handles of a selected schedule.

use super::handles::{self, Handle, HandleShape};
use super::layout::{wrap_text, Frame, Layout, Piece};
use crate::editor::Camera;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke};
use plan_core::geometry::Point;
use plan_core::schedules::{
    is_preview_field, CalloutShape, FractionFormat, FractionStyle, LabelOptions, Schedule,
    ScheduleKind, TextAlign, VAlign,
};
use plan_docs::schedule_kinds::Preview;

/// Text smaller than this many screen pixels is not drawn.
pub const MIN_TEXT_PX: f32 = 3.0;

/// The colours a schedule is drawn with.
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub body: Color32,
    pub title: Color32,
    pub head: Color32,
    pub paper: Color32,
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Screen position of local point `(x, y)`.
fn sp(cam: &Camera, f: &Frame, x: f64, y: f64) -> Pos2 {
    cam.world_to_screen(f.to_plan(x, y))
}

/// The angle text is drawn at: screen angles run clockwise.
fn text_angle(f: &Frame) -> f32 {
    -(f.angle() as f32)
}

fn rot(v: egui::Vec2, a: f32) -> egui::Vec2 {
    let (s, c) = a.sin_cos();
    egui::vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Draws `text` so that its `h` / `v` anchor falls on `at`, turned by
/// `angle` (screen radians, clockwise) about that anchor.
fn put_text(
    painter: &egui::Painter,
    text: &str,
    at: Pos2,
    h: Align2,
    font: FontId,
    color: Color32,
    angle: f32,
) {
    if text.is_empty() {
        return;
    }
    let galley = painter.layout_no_wrap(text.to_string(), font, color);
    let size = galley.size();
    let off = egui::vec2(
        match h.x() {
            egui::Align::Min => 0.0,
            egui::Align::Center => -size.x / 2.0,
            egui::Align::Max => -size.x,
        },
        match h.y() {
            egui::Align::Min => 0.0,
            egui::Align::Center => -size.y / 2.0,
            egui::Align::Max => -size.y,
        },
    );
    let pos = at + rot(off, angle);
    if angle.abs() < 1e-6 {
        painter.galley(pos, galley, color);
    } else {
        painter.add(egui::epaint::TextShape::new(pos, galley, color).with_angle(angle));
    }
}

/// Fills the local rectangle `(x0, y0, x1, y1)`.
fn fill_rect(
    painter: &egui::Painter,
    cam: &Camera,
    f: &Frame,
    r: (f64, f64, f64, f64),
    c: Color32,
) {
    let pts = vec![
        sp(cam, f, r.0, r.1),
        sp(cam, f, r.2, r.1),
        sp(cam, f, r.2, r.3),
        sp(cam, f, r.0, r.3),
    ];
    painter.add(Shape::convex_polygon(pts, c, Stroke::NONE));
}

fn line(painter: &egui::Painter, cam: &Camera, f: &Frame, a: (f64, f64), b: (f64, f64), s: Stroke) {
    painter.line_segment([sp(cam, f, a.0, a.1), sp(cam, f, b.0, b.1)], s);
}

/// The on-screen rectangle's bounding box of the schedule (for culling).
fn screen_bounds(cam: &Camera, l: &Layout, top_left: Point) -> Rect {
    let (lo, hi) = l.bounds(top_left);
    Rect::from_two_pos(
        cam.world_to_screen(Point::new(lo.x, hi.y)),
        cam.world_to_screen(Point::new(hi.x, lo.y)),
    )
}

/// Draws the schedule `s` laid out as `l`.
pub fn draw_schedule(painter: &egui::Painter, cam: &Camera, s: &Schedule, l: &Layout, col: Colors) {
    if !screen_bounds(cam, l, s.position).intersects(cam.rect) {
        return;
    }
    let px = cam.px_per_in as f32;
    let f = l.frame(s.position);
    let ang = text_angle(&f);
    let line_color = s.line_color.map_or(col.body, rgb);
    let weight = s.line_weight.max(0.2);
    let strong = Stroke::new(1.6_f32 * weight, line_color);
    let rule = Stroke::new(1.0_f32 * weight, line_color);
    let faint = Stroke::new(1.0_f32 * weight, line_color.gamma_multiply(0.35));
    let paper = if s.fill {
        s.fill_color.map_or(col.paper, rgb)
    } else {
        Color32::TRANSPARENT
    };
    let body_px = (l.metrics.body as f32 * px).clamp(1.0, 400.0);
    let title_px = (l.metrics.title as f32 * px).clamp(1.0, 400.0);
    let head_px = (l.metrics.head as f32 * px).clamp(1.0, 400.0);
    let [ml, mr, mt, mb] = s.margins;
    for piece in &l.pieces {
        let (x0, y0, x1, y1) = piece.rect();
        fill_rect(painter, cam, &f, piece.rect(), paper);
        // Title.
        if piece.title && piece.title_h > 0.0 {
            line(
                painter,
                cam,
                &f,
                (x0, y0 + piece.title_h),
                (x1, y0 + piece.title_h),
                rule,
            );
            if title_px >= MIN_TEXT_PX {
                put_text(
                    painter,
                    &l.table.title,
                    sp(cam, &f, (x0 + x1) / 2.0, y0 + piece.title_h / 2.0),
                    Align2::CENTER_CENTER,
                    FontId::proportional(title_px),
                    col.title,
                    ang,
                );
            }
        }
        // Grid lines between the cells; the rule under the headings is solid.
        let mut y = y0 + piece.title_h;
        for (r, rh) in piece.row_h.iter().enumerate() {
            y += rh;
            if r + 1 >= piece.row_h.len() {
                continue;
            }
            let heading = r + 1 == piece.head_rows;
            if heading {
                line(painter, cam, &f, (x0, y), (x1, y), rule);
            } else if s.grid_lines {
                line(painter, cam, &f, (x0, y), (x1, y), faint);
            }
        }
        let mut x = x0;
        for (c, cw) in piece.col_w.iter().enumerate() {
            x += cw;
            if c + 1 >= piece.col_w.len() {
                continue;
            }
            let heading = c + 1 == piece.head_cols;
            let (top, bottom) = (y0 + piece.title_h, y1);
            if heading {
                line(painter, cam, &f, (x, top), (x, bottom), rule);
            } else if s.grid_lines {
                line(painter, cam, &f, (x, top), (x, bottom), faint);
            }
        }
        if s.border {
            let pts = [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)];
            for w in pts.windows(2) {
                line(painter, cam, &f, w[0], w[1], strong);
            }
        }
        if body_px < MIN_TEXT_PX {
            continue;
        }
        draw_cells(
            painter,
            cam,
            s,
            l,
            piece,
            &f,
            col,
            (body_px, head_px),
            (ml, mr, mt, mb),
            ang,
        );
    }
}

/// The scale Scale Images gives every picture of a column: the widest and
/// tallest picture of the column fill the cell, the others keep their size
/// against them.
fn shared_scale(l: &Layout, field: &str, inner_w: f64, inner_h: f64) -> f64 {
    let dims = |p: &Preview| {
        if field == "symbol_2d" {
            (p.width, p.depth)
        } else {
            (p.width, p.height)
        }
    };
    let (mw, mh) = l
        .previews
        .iter()
        .flatten()
        .map(dims)
        .fold((1.0_f64, 1.0_f64), |a, b| (a.0.max(b.0), a.1.max(b.1)));
    (inner_w / mw).min(inner_h / mh)
}

#[allow(clippy::too_many_arguments)]
fn draw_cells(
    painter: &egui::Painter,
    cam: &Camera,
    s: &Schedule,
    l: &Layout,
    piece: &Piece,
    f: &Frame,
    col: Colors,
    (body_px, head_px): (f32, f32),
    (ml, mr, mt, mb): (f64, f64, f64, f64),
    ang: f32,
) {
    for (r, row) in piece.grid.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            let (cx0, cy0) = piece.cell_origin(r, c);
            let (cw, ch) = (
                piece.col_w.get(c).copied().unwrap_or(0.0),
                piece.row_h.get(r).copied().unwrap_or(0.0),
            );
            let (attr, obj) = piece.cell_ref(r, c);
            let field = attr.and_then(|a| l.fields.get(a)).map(String::as_str);
            let heading = if piece.swapped {
                c < piece.head_cols
            } else {
                r < piece.head_rows
            };
            // A picture column.
            if let (Some(field), Some(obj)) = (field, obj) {
                if is_preview_field(field) {
                    if let Some(Some(p)) = l.previews.get(obj) {
                        let inner = (cx0 + ml, cy0 + mt, cx0 + cw - mr, cy0 + ch - mb);
                        let scale = s
                            .previews
                            .scale_images
                            .then(|| shared_scale(l, field, inner.2 - inner.0, inner.3 - inner.1));
                        draw_preview(painter, cam, f, field, p, inner, scale, s, col);
                        continue;
                    }
                }
            }
            let (font_px, color) = if heading {
                (head_px, col.head)
            } else {
                (body_px, col.body)
            };
            let align = if heading && !piece.swapped {
                // Headings follow the column.
                attr.and_then(|a| l.aligns.get(a).copied().flatten())
                    .unwrap_or(s.h_align)
            } else {
                attr.and_then(|a| l.aligns.get(a).copied().flatten())
                    .unwrap_or(s.h_align)
            };
            let h_char = if heading {
                l.metrics.head
            } else {
                l.metrics.body
            };
            let lines = if text.is_empty() {
                Vec::new()
            } else {
                wrap_text(text, (cw - ml - mr).max(h_char), h_char)
            };
            let line_h = super::layout::LINE_H * h_char;
            // A fraction set diagonally or vertically (Fraction Format).
            if lines.len() == 1
                && !heading
                && s.fraction.style != FractionStyle::Horizontal
                && field.is_some_and(|f| s.kind.num_kind(f).is_some())
            {
                if let Some(parts) = split_fraction(&lines[0]) {
                    let y_mid = match s.v_align {
                        VAlign::Top => cy0 + mt + line_h / 2.0,
                        VAlign::Middle => cy0 + ch / 2.0,
                        VAlign::Bottom => cy0 + ch - mb - line_h / 2.0,
                    };
                    draw_fraction(
                        painter,
                        cam,
                        f,
                        &parts,
                        (cx0 + ml, cx0 + cw - mr, y_mid),
                        (font_px, h_char),
                        s.fraction,
                        align,
                        color,
                        ang,
                    );
                    continue;
                }
            }
            let block = lines.len() as f64 * line_h;
            let top = match s.v_align {
                VAlign::Top => cy0 + mt,
                VAlign::Middle => cy0 + (ch - block) / 2.0,
                VAlign::Bottom => cy0 + ch - mb - block,
            };
            for (i, t) in lines.iter().enumerate() {
                let y = top + (i as f64 + 0.5) * line_h;
                if align == TextAlign::Justify && lines.len() == 1 && t.contains(' ') && !heading {
                    justify_line(
                        painter,
                        cam,
                        f,
                        t,
                        cx0 + ml,
                        cx0 + cw - mr,
                        y,
                        font_px,
                        color,
                        ang,
                    );
                    continue;
                }
                let (x, anchor) = match align {
                    TextAlign::Left | TextAlign::Justify => (cx0 + ml, Align2::LEFT_CENTER),
                    TextAlign::Center => (cx0 + cw / 2.0, Align2::CENTER_CENTER),
                    TextAlign::Right => (cx0 + cw - mr, Align2::RIGHT_CENTER),
                };
                put_text(
                    painter,
                    t,
                    sp(cam, f, x, y),
                    anchor,
                    FontId::proportional(font_px),
                    color,
                    ang,
                );
            }
        }
    }
}

/// A single line spread across `x0..x1` by widening the spaces.
#[allow(clippy::too_many_arguments)]
fn justify_line(
    painter: &egui::Painter,
    cam: &Camera,
    f: &Frame,
    text: &str,
    x0: f64,
    x1: f64,
    y: f64,
    font_px: f32,
    color: Color32,
    ang: f32,
) {
    let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
    if words.len() < 2 {
        put_text(
            painter,
            text,
            sp(cam, f, x0, y),
            Align2::LEFT_CENTER,
            FontId::proportional(font_px),
            color,
            ang,
        );
        return;
    }
    let px = cam.px_per_in as f32;
    let widths: Vec<f64> = words
        .iter()
        .map(|w| {
            painter
                .layout_no_wrap((*w).to_string(), FontId::proportional(font_px), color)
                .size()
                .x as f64
                / px as f64
        })
        .collect();
    let total: f64 = widths.iter().sum();
    let gap = ((x1 - x0) - total).max(0.0) / (words.len() - 1) as f64;
    let mut x = x0;
    for (w, wd) in words.iter().zip(&widths) {
        put_text(
            painter,
            w,
            sp(cam, f, x, y),
            Align2::LEFT_CENTER,
            FontId::proportional(font_px),
            color,
            ang,
        );
        x += wd + gap;
    }
}

// ===================================================================
// Fractions
// ===================================================================

/// `text` cut around its first fraction: what comes before, the numerator,
/// the denominator and what follows (`3'-0 1/4"` gives `3'-0 `, `1`, `4`,
/// `"`).
pub fn split_fraction(text: &str) -> Option<(String, String, String, String)> {
    let b: Vec<char> = text.chars().collect();
    let slash = b.iter().position(|c| *c == '/')?;
    let mut s = slash;
    while s > 0 && b[s - 1].is_ascii_digit() {
        s -= 1;
    }
    let mut e = slash + 1;
    while e < b.len() && b[e].is_ascii_digit() {
        e += 1;
    }
    if s == slash || e == slash + 1 {
        return None;
    }
    Some((
        b[..s].iter().collect(),
        b[s..slash].iter().collect(),
        b[slash + 1..e].iter().collect(),
        b[e..].iter().collect(),
    ))
}

/// Draws a cell whose text has a fraction in the style the schedule asks for:
/// the numerator and denominator in smaller text, raised and lowered along a
/// slash (Diagonal) or stacked over a line (Vertical). `span` is the cell's
/// text area `(left, right, middle line)` in local inches.
#[allow(clippy::too_many_arguments)]
fn draw_fraction(
    painter: &egui::Painter,
    cam: &Camera,
    f: &Frame,
    parts: &(String, String, String, String),
    span: (f64, f64, f64),
    (font_px, h_char): (f32, f64),
    fmt: FractionFormat,
    align: TextAlign,
    color: Color32,
    ang: f32,
) {
    let (pre, num, den, post) = parts;
    let px = cam.px_per_in as f32;
    let small = (font_px * (fmt.text_pct as f32 / 100.0)).max(2.0);
    let width = |t: &str, size: f32| -> f64 {
        if t.is_empty() {
            return 0.0;
        }
        painter
            .layout_no_wrap(t.to_string(), FontId::proportional(size), color)
            .size()
            .x as f64
            / f64::from(px)
    };
    let (w_pre, w_num, w_den, w_post) = (
        width(pre, font_px),
        width(num, small),
        width(den, small),
        width(post, font_px),
    );
    let w_slash = width("/", small);
    let frac_w = match fmt.style {
        FractionStyle::Vertical => w_num.max(w_den) + 0.2 * h_char,
        _ => w_num + w_slash + w_den,
    };
    let total = w_pre + frac_w + w_post;
    let (left, right, y) = span;
    let x0 = match align {
        TextAlign::Left | TextAlign::Justify => left,
        TextAlign::Center => left + ((right - left) - total) / 2.0,
        TextAlign::Right => right - total,
    };
    let at = |x: f64, y: f64| sp(cam, f, x, y);
    let put = |t: &str, x: f64, y: f64, a: Align2, size: f32| {
        put_text(
            painter,
            t,
            at(x, y),
            a,
            FontId::proportional(size),
            color,
            ang,
        )
    };
    put(pre, x0, y, Align2::LEFT_CENTER, font_px);
    let fx = x0 + w_pre;
    match fmt.style {
        FractionStyle::Vertical => {
            let mid = fx + frac_w / 2.0;
            put(num, mid, y - 0.32 * h_char, Align2::CENTER_CENTER, small);
            put(den, mid, y + 0.32 * h_char, Align2::CENTER_CENTER, small);
            painter.line_segment([at(fx, y), at(fx + frac_w, y)], Stroke::new(1.0_f32, color));
        }
        _ => {
            put(num, fx, y - 0.22 * h_char, Align2::LEFT_CENTER, small);
            put("/", fx + w_num, y, Align2::LEFT_CENTER, small);
            put(
                den,
                fx + w_num + w_slash,
                y + 0.22 * h_char,
                Align2::LEFT_CENTER,
                small,
            );
        }
    }
    put(post, fx + frac_w, y, Align2::LEFT_CENTER, font_px);
}

// ===================================================================
// Object previews
// ===================================================================

/// Draws the picture of one object in the local rectangle `inner`. `scale`
/// is the shared plan-to-local scale of Scale Images; without it the picture
/// fills the cell.
#[allow(clippy::too_many_arguments)]
fn draw_preview(
    painter: &egui::Painter,
    cam: &Camera,
    f: &Frame,
    field: &str,
    p: &Preview,
    inner: (f64, f64, f64, f64),
    scale: Option<f64>,
    s: &Schedule,
    col: Colors,
) {
    let color = if s.previews.show_color {
        match s.previews.color_from {
            plan_core::schedules::ColorFrom::Schedule => s.line_color.map_or(col.body, rgb),
            plan_core::schedules::ColorFrom::Plan => preview_color(p.kind),
        }
    } else {
        col.body
    };
    let stroke = Stroke::new(1.0_f32, color);
    let (cx, cy) = ((inner.0 + inner.2) / 2.0, (inner.1 + inner.3) / 2.0);
    let (aw, ah) = ((inner.2 - inner.0).max(0.1), (inner.3 - inner.1).max(0.1));
    if field == "callout_symbol" {
        let h = (ah * 0.55).max(1.0);
        let opts = default_label_options(p.kind);
        let at = cam.world_to_screen(f.to_plan(cx, cy));
        let px = cam.px_per_in as f32;
        draw_callout_shape(
            painter,
            at,
            &p.mark,
            h as f32 * px,
            &opts,
            p.kind,
            color,
            col.paper,
            text_angle(f),
        );
        return;
    }
    // Width and height of the picture in plan inches.
    let (pw, ph) = match field {
        "symbol_2d" => (p.width.max(1.0), p.depth.max(1.0)),
        _ => (p.width.max(1.0), p.height.max(1.0)),
    };
    let k = scale
        .unwrap_or_else(|| (aw / pw).min(ah / ph))
        .min((aw / pw).min(ah / ph) * 1.0001);
    let (w, h) = (pw * k, ph * k);
    let (x0, y0, x1, y1) = (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
    let seg = |a: (f64, f64), b: (f64, f64)| line(painter, cam, f, a, b, stroke);
    let rect = |r: (f64, f64, f64, f64)| {
        seg((r.0, r.1), (r.2, r.1));
        seg((r.2, r.1), (r.2, r.3));
        seg((r.2, r.3), (r.0, r.3));
        seg((r.0, r.3), (r.0, r.1));
    };
    match (field, p.kind) {
        ("symbol_2d", ScheduleKind::Door) => {
            // Leaf and swing arc in the opening.
            seg((x0, y1), (x1, y1));
            seg((x0, y1), (x0, y0));
            let n = 12;
            let mut prev = (x1, y1);
            for i in 1..=n {
                let a = std::f64::consts::FRAC_PI_2 * i as f64 / n as f64;
                let pt = (x0 + w * a.cos(), y1 - w.min(h) * a.sin());
                seg(prev, pt);
                prev = pt;
            }
        }
        ("symbol_2d", ScheduleKind::Window) => {
            for t in [0.0, 0.5, 1.0] {
                let y = y0 + (y1 - y0) * t;
                seg((x0, y), (x1, y));
            }
        }
        ("symbol_2d", _) => {
            rect((x0, y0, x1, y1));
            if matches!(p.kind, ScheduleKind::Cabinet) {
                seg((x0, y0), (x1, y1));
            }
        }
        ("elevation_3d", ScheduleKind::Door) => {
            rect((x0, y0, x1, y1));
            let (mx, third) = ((x0 + x1) / 2.0, (y1 - y0) / 3.0);
            rect((
                x0 + w * 0.15,
                y0 + third * 0.25,
                mx - w * 0.05,
                y0 + third * 1.4,
            ));
            rect((
                mx + w * 0.05,
                y0 + third * 0.25,
                x1 - w * 0.15,
                y0 + third * 1.4,
            ));
            rect((
                x0 + w * 0.15,
                y0 + third * 1.6,
                mx - w * 0.05,
                y1 - third * 0.25,
            ));
            rect((
                mx + w * 0.05,
                y0 + third * 1.6,
                x1 - w * 0.15,
                y1 - third * 0.25,
            ));
        }
        ("elevation_3d", ScheduleKind::Window) => {
            rect((x0, y0, x1, y1));
            rect((x0 + w * 0.08, y0 + h * 0.08, x1 - w * 0.08, y1 - h * 0.08));
            seg(
                ((x0 + x1) / 2.0, y0 + h * 0.08),
                ((x0 + x1) / 2.0, y1 - h * 0.08),
            );
            seg(
                (x0 + w * 0.08, (y0 + y1) / 2.0),
                (x1 - w * 0.08, (y0 + y1) / 2.0),
            );
        }
        ("elevation_3d", _) => {
            rect((x0, y0, x1, y1));
            seg(((x0 + x1) / 2.0, y0), ((x0 + x1) / 2.0, y1));
        }
        ("perspective_3d", _) => {
            // An oblique box: the front, and the top and side set back.
            let d = (w.min(h) * 0.25).max(0.5);
            let (fx0, fy0, fx1, fy1) = (x0, y0 + d, x1 - d, y1);
            rect((fx0, fy0, fx1, fy1));
            seg((fx0, fy0), (fx0 + d, fy0 - d));
            seg((fx1, fy0), (fx1 + d, fy0 - d));
            seg((fx1, fy1), (fx1 + d, fy1 - d));
            seg((fx0 + d, fy0 - d), (fx1 + d, fy0 - d));
            seg((fx1 + d, fy0 - d), (fx1 + d, fy1 - d));
        }
        _ => rect((x0, y0, x1, y1)),
    }
}

fn preview_color(kind: ScheduleKind) -> Color32 {
    match kind {
        ScheduleKind::Door => Color32::from_rgb(160, 90, 30),
        ScheduleKind::Window => Color32::from_rgb(40, 110, 170),
        ScheduleKind::Cabinet => Color32::from_rgb(110, 80, 50),
        ScheduleKind::Plant => Color32::from_rgb(40, 130, 60),
        _ => Color32::from_rgb(90, 90, 90),
    }
}

// ===================================================================
// Callout shapes
// ===================================================================

/// The label options a callout uses when no schedule says otherwise.
pub fn default_label_options(kind: ScheduleKind) -> LabelOptions {
    let _ = kind;
    LabelOptions::default()
}

/// The shape a kind's callout takes when the schedule names none: a circle
/// for doors, a hexagon for windows, text alone for the rest.
pub fn shape_of(opts: &LabelOptions, kind: ScheduleKind) -> CalloutShape {
    opts.shape.unwrap_or(match kind {
        ScheduleKind::Door => CalloutShape::Circle,
        ScheduleKind::Window => CalloutShape::Hexagon,
        _ => CalloutShape::None,
    })
}

/// Draws a callout: `text` in the shape the options name, centred on `at`,
/// at character height `font_px` pixels.
#[allow(clippy::too_many_arguments)]
pub fn draw_callout_shape(
    painter: &egui::Painter,
    at: Pos2,
    text: &str,
    font_px: f32,
    opts: &LabelOptions,
    kind: ScheduleKind,
    ink: Color32,
    paper: Color32,
    angle: f32,
) {
    let shape = shape_of(opts, kind);
    let text_w = text.chars().count() as f32 * font_px * super::layout::CHAR_W as f32;
    let auto = (text_w * 0.5 + 0.35 * font_px).max(0.7 * font_px);
    let radius = if opts.auto_size {
        auto
    } else {
        opts.size as f32 * font_px / 4.5
    }
    .max(2.0);
    let stroke = Stroke::new(1.2_f32, ink);
    let fill = if opts.filled {
        let base = if opts.fill_by_layer {
            ink
        } else {
            rgb(opts.fill_color)
        };
        base.gamma_multiply(1.0 - f32::from(opts.transparency) / 100.0)
    } else {
        paper
    };
    let shape_angle = (-opts.shape_angle.to_radians()) as f32 + angle;
    let poly = |n: usize, start: f32, rx: f32, ry: f32| -> Vec<Pos2> {
        (0..n)
            .map(|k| {
                let a = start + std::f32::consts::TAU * k as f32 / n as f32;
                at + rot(egui::vec2(rx * a.cos(), ry * a.sin()), shape_angle)
            })
            .collect()
    };
    match shape {
        CalloutShape::None => {}
        CalloutShape::Circle => {
            painter.circle(at, radius, fill, stroke);
        }
        CalloutShape::Ellipse => {
            painter.add(Shape::convex_polygon(
                poly(32, 0.0, radius * 1.45, radius),
                fill,
                stroke,
            ));
        }
        CalloutShape::Rectangle => {
            let (rx, ry) = (radius * 1.25, radius * 0.9);
            let corners = [(-rx, -ry), (rx, -ry), (rx, ry), (-rx, ry)]
                .iter()
                .map(|(x, y)| at + rot(egui::vec2(*x, *y), shape_angle))
                .collect();
            painter.add(Shape::convex_polygon(corners, fill, stroke));
        }
        CalloutShape::Capsule => {
            // Two half circles joined.
            let (rx, ry) = (radius * 1.4, radius * 0.8);
            let mut pts = Vec::new();
            for k in 0..=12 {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 12.0;
                pts.push(
                    at + rot(
                        egui::vec2(rx - ry + ry * a.cos(), ry * a.sin()),
                        shape_angle,
                    ),
                );
            }
            for k in 0..=12 {
                let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 12.0;
                pts.push(
                    at + rot(
                        egui::vec2(-(rx - ry) + ry * a.cos(), ry * a.sin()),
                        shape_angle,
                    ),
                );
            }
            painter.add(Shape::convex_polygon(pts, fill, stroke));
        }
        s => {
            let n = s.sides().max(3);
            let start = match s {
                CalloutShape::Triangle | CalloutShape::Pentagon => -std::f32::consts::FRAC_PI_2,
                CalloutShape::Hexagon => std::f32::consts::FRAC_PI_6,
                CalloutShape::Octagon => std::f32::consts::PI / 8.0,
                _ => 0.0,
            };
            let r = radius * if n <= 4 { 1.25 } else { 1.1 };
            painter.add(Shape::convex_polygon(poly(n, start, r, r), fill, stroke));
        }
    }
    // The text angle follows the shape angle unless set apart.
    let text_deg = if opts.auto_text_angle {
        opts.shape_angle
    } else {
        opts.shape_angle + opts.text_angle
    };
    let a = (-text_deg.to_radians()) as f32 + angle;
    put_text(
        painter,
        text,
        at,
        Align2::CENTER_CENTER,
        FontId::proportional(font_px),
        ink,
        a,
    );
}

// ===================================================================
// Handles
// ===================================================================

/// Draws the edit handles of a selected schedule.
pub fn draw_handles(
    painter: &egui::Painter,
    cam: &Camera,
    handles: &[Handle],
    color: Color32,
    fill: Color32,
) {
    let stroke = Stroke::new(1.4_f32, color);
    for h in handles {
        let at = cam.world_to_screen(h.at);
        match h.shape {
            HandleShape::Circle => {
                painter.circle(at, 4.5, fill, stroke);
            }
            HandleShape::Square => {
                painter.rect_filled(Rect::from_center_size(at, egui::vec2(9.0, 9.0)), 1.0, color);
            }
            HandleShape::Triangle => {
                let r = 6.0;
                let pts = vec![
                    at + egui::vec2(0.0, r),
                    at + egui::vec2(-r, -r * 0.7),
                    at + egui::vec2(r, -r * 0.7),
                ];
                painter.add(Shape::convex_polygon(pts, color, stroke));
            }
            HandleShape::Diamond => {
                let r = 6.0;
                let pts = vec![
                    at + egui::vec2(0.0, -r),
                    at + egui::vec2(r, 0.0),
                    at + egui::vec2(0.0, r),
                    at + egui::vec2(-r, 0.0),
                ];
                painter.add(Shape::convex_polygon(pts, fill, stroke));
            }
            HandleShape::Sort(down) => {
                let r = 4.0;
                let d = if down { 1.0 } else { -1.0 };
                let pts = vec![
                    at + egui::vec2(-r, -r * d * 0.6),
                    at + egui::vec2(r, -r * d * 0.6),
                    at + egui::vec2(0.0, r * d * 0.8),
                ];
                painter.add(Shape::convex_polygon(
                    pts,
                    color.gamma_multiply(0.6),
                    stroke,
                ));
            }
        }
    }
    let _ = handles::HANDLE_PX;
}
