//! Layout rendering: boxes, title blocks and the PDF.
//!
//! Every box is first turned into a list of [`Prim`]s in PDF points (strokes,
//! fills, text), clipped in software when the box asks for it, and only then
//! written through [`PdfDoc`]. The same list backs [`render_box_lines`], which
//! lets tests inspect exactly what a box would draw.

use crate::clip::{clip_polygon, clip_segment, contains, Pt, Rect};
use crate::extent::{
    frame_for, schedule_for, table_metrics, Frame, SceneSource, LINE_SPACING, ROW_H_PT,
    TABLE_TEXT_PT, TABLE_TITLE_H_PT,
};
use crate::model::{BoxSource, Layout, LayoutBox, LayoutPage, BOTTOM_STRIP_IN};
use crate::titleblock::{MacroContext, TitleBlockStyle};
use plan_3d::Scene;
use plan_core::geometry::point_in_polygon;
use plan_core::{
    detect_rooms, wall_outlines, CadItem, CadObject, DimFormat, DimensionKind, Floor, LayerSet,
    Opening, OpeningKind, Point, Project, Room, Wall,
};
use plan_docs::PdfDoc;
use plan_elevation::{elevation, section, Drawing, EdgeKind, Line2, LineWeight, Options};
use std::f64::consts::TAU;

/// Everything a layout needs from the rest of the model when it is drawn.
pub struct LayoutRenderContext<'a> {
    pub project: &'a Project,
    /// Detected rooms per floor (index = floor). Missing floors are detected on demand.
    pub rooms_by_floor: Vec<Vec<Room>>,
    /// The 3D scene for elevations and sections; built with `plan_3d::build_scene` when `None`.
    pub scene: Option<&'a Scene>,
    pub macros: MacroContext,
}

impl<'a> LayoutRenderContext<'a> {
    /// A context for `project`: rooms detected on every floor, no scene, and
    /// macros holding only the project name.
    pub fn new(project: &'a Project) -> Self {
        Self {
            project,
            rooms_by_floor: project
                .floors
                .iter()
                .map(|f| detect_rooms(&f.walls, 1.0))
                .collect(),
            scene: None,
            macros: MacroContext {
                project_name: project.name.clone(),
                ..MacroContext::default()
            },
        }
    }
}

/// A drawing primitive in PDF points.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Prim {
    Stroke {
        pts: Vec<Pt>,
        closed: bool,
        width: f64,
        gray: f64,
    },
    Fill {
        pts: Vec<Pt>,
        gray: f64,
    },
    Text {
        x: f64,
        y: f64,
        size: f64,
        gray: f64,
        text: String,
    },
}

/// Collects primitives, clipping them to `clip` when set.
pub(crate) struct Canvas {
    pub prims: Vec<Prim>,
    clip: Option<Rect>,
}

impl Canvas {
    pub(crate) fn new(clip: Option<Rect>) -> Self {
        Self {
            prims: Vec::new(),
            clip,
        }
    }

    pub(crate) fn stroke(&mut self, pts: &[Pt], closed: bool, width: f64, gray: f64) {
        if pts.len() < 2 {
            return;
        }
        let Some(r) = self.clip else {
            self.prims.push(Prim::Stroke {
                pts: pts.to_vec(),
                closed,
                width,
                gray,
            });
            return;
        };
        if pts.iter().all(|&p| contains(r, p)) {
            self.prims.push(Prim::Stroke {
                pts: pts.to_vec(),
                closed,
                width,
                gray,
            });
            return;
        }
        let n = if closed { pts.len() } else { pts.len() - 1 };
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if let Some((a, b)) = clip_segment(r, a, b) {
                self.prims.push(Prim::Stroke {
                    pts: vec![a, b],
                    closed: false,
                    width,
                    gray,
                });
            }
        }
    }

    pub(crate) fn line(&mut self, a: Pt, b: Pt, width: f64, gray: f64) {
        self.stroke(&[a, b], false, width, gray);
    }

    /// A dashed line (3 pt on, 2 pt off).
    pub(crate) fn dashed(&mut self, a: Pt, b: Pt, width: f64, gray: f64) {
        const ON: f64 = 3.0;
        const OFF: f64 = 2.0;
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        if len <= ON {
            self.line(a, b, width, gray);
            return;
        }
        let at = |d: f64| (a.0 + (b.0 - a.0) * d / len, a.1 + (b.1 - a.1) * d / len);
        let mut d = 0.0;
        while d < len {
            self.line(at(d), at((d + ON).min(len)), width, gray);
            d += ON + OFF;
        }
    }

    pub(crate) fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, width: f64) {
        self.stroke(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true, width, 0.0);
    }

    pub(crate) fn fill(&mut self, pts: &[Pt], gray: f64) {
        if pts.len() < 3 {
            return;
        }
        let pts = match self.clip {
            Some(r) => clip_polygon(r, pts),
            None => pts.to_vec(),
        };
        if pts.len() >= 3 {
            self.prims.push(Prim::Fill { pts, gray });
        }
    }

    /// Text at its baseline-left. Clipped text is dropped unless it fits whole.
    pub(crate) fn text(&mut self, x: f64, y: f64, size: f64, gray: f64, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(r) = self.clip {
            let w = PdfDoc::text_width(text, size);
            if !(contains(r, (x, y - size * 0.25)) && contains(r, (x + w, y + size))) {
                return;
            }
        }
        self.prims.push(Prim::Text {
            x,
            y,
            size,
            gray,
            text: text.to_string(),
        });
    }

    pub(crate) fn text_centered(&mut self, cx: f64, y: f64, size: f64, gray: f64, text: &str) {
        let w = PdfDoc::text_width(text, size);
        self.text(cx - w * 0.5, y, size, gray, text);
    }

    pub(crate) fn text_right(&mut self, rx: f64, y: f64, size: f64, gray: f64, text: &str) {
        let w = PdfDoc::text_width(text, size);
        self.text(rx - w, y, size, gray, text);
    }

    /// Heavier-looking text: drawn twice, 0.3 pt apart.
    pub(crate) fn bold(&mut self, x: f64, y: f64, size: f64, text: &str) {
        self.text(x, y, size, 0.0, text);
        self.text(x + 0.3, y, size, 0.0, text);
    }
}

// ---------------------------------------------------------------- helpers --

/// Plotted pen width of a layer in points (`line_weight` is 1/100 mm).
fn pen_pt(layers: &LayerSet, name: &str, scale: f64) -> f64 {
    let hundredths = layers.get(name).map_or(25, |l| l.line_weight);
    (f64::from(hundredths) / 100.0 * 72.0 / 25.4 * scale).max(0.1)
}

fn arc_points(center: Point, r: f64, start: f64, sweep: f64) -> Vec<Point> {
    let steps = ((sweep.abs() / (TAU / 48.0)).ceil() as usize).max(4);
    (0..=steps)
        .map(|i| {
            let a = start + sweep * i as f64 / steps as f64;
            center + Point::new(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Shorten `s` until it fits `max_w` points at `size`.
fn fit_text(s: &str, size: f64, max_w: f64) -> String {
    let mut t = s.to_string();
    while !t.is_empty() && PdfDoc::text_width(&t, size) > max_w {
        t.pop();
    }
    t
}

/// Greedy word wrap to `max_w` points; overlong words are trimmed.
fn wrap_text(s: &str, size: f64, max_w: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        let word = fit_text(word, size, max_w);
        match lines.last_mut() {
            Some(last) if PdfDoc::text_width(&format!("{last} {word}"), size) <= max_w => {
                last.push(' ');
                last.push_str(&word);
            }
            _ => lines.push(word),
        }
    }
    lines
}

// ------------------------------------------------------------------- CAD --

/// Draw one CAD object through `tp` (source space to points). `k` converts
/// source inches to points for text height.
fn draw_cad_item(cv: &mut Canvas, o: &CadObject, tp: &dyn Fn(Point) -> Pt, k: f64, width: f64) {
    match &o.item {
        CadItem::Line { a, b } => cv.line(tp(*a), tp(*b), width, 0.0),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            let pts: Vec<Pt> = arc_points(*center, *radius, *start_angle, sweep)
                .into_iter()
                .map(tp)
                .collect();
            cv.stroke(&pts, false, width, 0.0);
        }
        CadItem::Circle { center, radius } => {
            let mut pts: Vec<Pt> = arc_points(*center, *radius, 0.0, TAU)
                .into_iter()
                .map(tp)
                .collect();
            pts.pop();
            cv.stroke(&pts, true, width, 0.0);
        }
        CadItem::Polyline { points, closed } => {
            let pts: Vec<Pt> = points.iter().map(|&p| tp(p)).collect();
            cv.stroke(&pts, *closed, width, 0.0);
        }
        CadItem::Text {
            pos, text, height, ..
        } => {
            let (x, y) = tp(*pos);
            cv.text(x, y, height * k, 0.0, text);
        }
    }
}

// ------------------------------------------------------------- plan view --

fn draw_opening(
    cv: &mut Canvas,
    w: &Wall,
    o: &Opening,
    tp: &dyn Fn(Point) -> Pt,
    k: f64,
    pen: f64,
) {
    let (n, half) = (w.normal(), w.thickness * 0.5);
    let (s, e) = (o.start_offset(), o.end_offset());
    // The gap overshoots the faces a little so the outline strokes vanish.
    let g = half + 1.5 / k;
    let at = |off: f64, side: f64| w.point_at(off) + n * side;
    cv.fill(&[at(s, g), at(e, g), at(e, -g), at(s, -g)].map(tp), 1.0);
    for off in [s, e] {
        cv.line(tp(at(off, half)), tp(at(off, -half)), pen, 0.0);
    }
    match o.kind {
        OpeningKind::Window => {
            for (side, width) in [(half, pen), (0.0, pen * 0.5), (-half, pen)] {
                cv.line(tp(at(s, side)), tp(at(e, side)), width, 0.0);
            }
        }
        OpeningKind::Door => {
            // Hinge at the wall-start jamb, swinging to the left.
            let (hinge_off, other_off, sign) = if o.swing_flipped {
                (e, s, -1.0)
            } else {
                (s, e, 1.0)
            };
            let swing = n * sign;
            let hinge = w.point_at(hinge_off) + swing * half;
            let closed_dir = (w.point_at(other_off) - w.point_at(hinge_off)).normalized();
            let leaf_end = hinge + swing * o.width;
            cv.line(tp(hinge), tp(leaf_end), pen * 0.8, 0.0);
            let a0 = closed_dir.angle();
            let mut sweep = swing.angle() - a0;
            while sweep > std::f64::consts::PI {
                sweep -= TAU;
            }
            while sweep <= -std::f64::consts::PI {
                sweep += TAU;
            }
            let pts: Vec<Pt> = arc_points(hinge, o.width, a0, sweep)
                .into_iter()
                .map(tp)
                .collect();
            cv.stroke(&pts, false, pen * 0.6, 0.0);
        }
    }
}

fn room_name(f: &Floor, room: &Room) -> String {
    f.room_names
        .iter()
        .find(|n| point_in_polygon(n.anchor, &room.polygon))
        .map_or_else(|| room.label.clone(), |n| n.name.clone())
}

fn draw_dimension(cv: &mut Canvas, d: &plan_core::Dimension, tp: &dyn Fn(Point) -> Pt, pen: f64) {
    const TICK: f64 = 3.0;
    const TEXT_PT: f64 = 7.0;
    for (a, b) in d.extension_lines() {
        cv.line(tp(a), tp(b), pen * 0.6, 0.0);
    }
    let (a, b) = d.line_points();
    let (pa, pb) = (tp(a), tp(b));
    cv.line(pa, pb, pen, 0.0);
    for p in [pa, pb] {
        cv.line(
            (p.0 - TICK, p.1 - TICK),
            (p.0 + TICK, p.1 + TICK),
            pen * 1.5,
            0.0,
        );
    }
    let label = d.label(&DimFormat::default());
    let mid = ((pa.0 + pb.0) * 0.5, (pa.1 + pb.1) * 0.5);
    if (pb.0 - pa.0).abs() >= (pb.1 - pa.1).abs() {
        cv.text_centered(mid.0, mid.1 + 2.0, TEXT_PT, 0.0, &label);
    } else {
        // PdfDoc cannot rotate text, so vertical dimensions read sideways-free.
        cv.text(mid.0 + 3.0, mid.1 - TEXT_PT * 0.35, TEXT_PT, 0.0, &label);
    }
}

fn draw_plan(
    cv: &mut Canvas,
    cx: &LayoutRenderContext,
    floor: usize,
    layer_set: &str,
    tp: &dyn Fn(Point) -> Pt,
    k: f64,
    lws: f64,
) {
    let Some(f) = cx.project.floors.get(floor) else {
        return;
    };
    let layers = &cx.project.layers;
    let show = |name: &str| layer_set == "All" || layers.is_visible(name);
    let pen = |name: &str| pen_pt(layers, name, lws);

    // Walls: black outline stroke first, gray fill on top (shared edges vanish).
    let outlines = wall_outlines(&f.walls, 0.5);
    let shown: Vec<_> = f
        .walls
        .iter()
        .zip(&outlines)
        .filter(|(w, _)| show(&w.layer))
        .collect();
    let poly =
        |o: &plan_core::WallOutline| -> Vec<Pt> { o.polygon.iter().map(|&p| tp(p)).collect() };
    for (w, o) in &shown {
        cv.stroke(&poly(o), true, 2.0 * pen(&w.layer), 0.0);
    }
    for (_, o) in &shown {
        cv.fill(&poly(o), 0.8);
    }

    for o in &f.openings {
        let layer = match o.kind {
            OpeningKind::Door => "Doors",
            OpeningKind::Window => "Windows",
        };
        let Some(w) = f.wall(o.wall_id) else { continue };
        if show(layer) && show(&w.layer) {
            draw_opening(cv, w, o, tp, k, pen(layer));
        }
    }

    if show("Room Labels") {
        let fallback;
        let rooms: &[Room] = match cx.rooms_by_floor.get(floor) {
            Some(r) => r,
            None => {
                fallback = detect_rooms(&f.walls, 1.0);
                &fallback
            }
        };
        for r in rooms {
            let (x, y) = tp(r.centroid);
            cv.text_centered(x, y + 1.5, 8.0, 0.0, &room_name(f, r));
            cv.text_centered(x, y - 8.0, 6.5, 0.0, &format!("{:.0} SF", r.area_sq_ft()));
        }
    }

    for d in &f.dimensions {
        let layer = match d.kind {
            DimensionKind::Manual => "Dimensions, Manual",
            DimensionKind::AutoExterior => "Dimensions, Automatic",
            DimensionKind::Temporary => continue,
        };
        if show(layer) {
            draw_dimension(cv, d, tp, pen(layer));
        }
    }

    for o in &f.cad {
        if show(&o.layer) {
            draw_cad_item(cv, o, tp, k, pen(&o.layer));
        }
    }
}

// ----------------------------------------------------- elevation, tables --

fn draw_drawing(cv: &mut Canvas, d: &Drawing, tp: &dyn Fn(Point) -> Pt, lws: f64) {
    for l in &d.lines {
        let (a, b, kind) = (l.a, l.b, l.kind);
        let width = match l.weight {
            LineWeight::Heavy => 0.7,
            LineWeight::Medium => 0.35,
            LineWeight::Light => 0.18,
        } * lws;
        if kind == EdgeKind::Hidden {
            cv.dashed(tp(a), tp(b), width, 0.0);
        } else {
            cv.line(tp(a), tp(b), width, 0.0);
        }
    }
}

/// Draw a schedule table with its top-left corner at `(x, top)`.
fn draw_table(cv: &mut Canvas, s: &plan_docs::Schedule, x: f64, top: f64) {
    let m = table_metrics(s);
    cv.bold(x + 2.0, top - TABLE_TITLE_H_PT + 6.0, 10.0, &s.title);
    let t0 = top - TABLE_TITLE_H_PT;
    let n_rows = s.rows.len() + 1;
    // Horizontal rules: top, under the header, under every row.
    for i in 0..=n_rows {
        let y = t0 - ROW_H_PT * i as f64;
        let w = if i == 0 || i == n_rows { 1.0 } else { 0.4 };
        cv.line((x, y), (x + m.width, y), w, 0.0);
    }
    let bottom = t0 - ROW_H_PT * n_rows as f64;
    let mut cx = x;
    for (c, cw) in m.cols.iter().enumerate() {
        cv.line((cx, t0), (cx, bottom), 0.4, 0.0);
        let hy = t0 - ROW_H_PT + 6.0;
        cv.bold(cx + 4.0, hy, TABLE_TEXT_PT, &s.columns[c]);
        for (r, row) in s.rows.iter().enumerate() {
            if let Some(cell) = row.get(c) {
                let y = t0 - ROW_H_PT * (r + 2) as f64 + 6.0;
                cv.text(cx + 4.0, y, TABLE_TEXT_PT, 0.0, cell);
            }
        }
        cx += cw;
    }
    cv.line((cx, t0), (cx, bottom), 1.0, 0.0);
    cv.line((x, t0), (x, bottom), 1.0, 0.0);
}

// ------------------------------------------------------------------ boxes --

fn box_rect_pt(b: &LayoutBox) -> Rect {
    b.bounds_in().map(|v| v * 72.0)
}

/// The prims for a box: its content, clipped when `clip` is set, then its border.
fn box_prims(b: &LayoutBox, cx: &LayoutRenderContext, scenes: &SceneSource) -> Vec<Prim> {
    let rect = box_rect_pt(b);
    let mut cv = Canvas::new(b.clip.then_some(rect));
    let (bw, bh) = (rect[2] - rect[0], rect[3] - rect[1]);
    let k = b.scale.points_per_inch();
    let lws = b.line_weight_scale;
    let frame = frame_for(&b.source, cx, scenes);

    match frame {
        Frame::Scaled { lo, hi } => {
            let (cw, ch) = ((hi.x - lo.x) * k, (hi.y - lo.y) * k);
            let (ox, oy) = if cw <= bw + 1e-6 && ch <= bh + 1e-6 {
                (rect[0] + (bw - cw) * 0.5, rect[1] + (bh - ch) * 0.5)
            } else {
                (rect[0], rect[3] - ch)
            };
            let tp = move |p: Point| (ox + (p.x - lo.x) * k, oy + (p.y - lo.y) * k);
            let opts = Options::default();
            match &b.source {
                BoxSource::PlanView { floor, layer_set } => {
                    draw_plan(&mut cv, cx, *floor, layer_set, &tp, k, lws);
                }
                BoxSource::Elevation { dir } => {
                    let d = elevation(scenes.get(cx.project), *dir, &opts);
                    draw_drawing(&mut cv, &d, &tp, lws);
                }
                BoxSource::Section { cut } => {
                    let d = section(scenes.get(cx.project), *cut, &opts);
                    draw_drawing(&mut cv, &d, &tp, lws);
                }
                BoxSource::CadDetail { items, .. } => {
                    let layers = &cx.project.layers;
                    for o in items {
                        draw_cad_item(&mut cv, o, &tp, k, pen_pt(layers, &o.layer, lws));
                    }
                }
                _ => {}
            }
        }
        Frame::Paper { .. } => match &b.source {
            BoxSource::Schedule { kind } => {
                draw_table(&mut cv, &schedule_for(*kind, cx), rect[0], rect[3]);
            }
            BoxSource::Text { text, height_pt } => {
                for (i, line) in text.lines().enumerate() {
                    let y = rect[3] - height_pt * LINE_SPACING * (i as f64 + 0.8);
                    cv.text(rect[0] + 3.0, y, *height_pt, 0.0, line);
                }
            }
            BoxSource::Image { path } => {
                let (x0, y0, x1, y1) = (rect[0], rect[1], rect[2], rect[3]);
                cv.line((x0, y0), (x1, y1), 0.35 * lws, 0.6);
                cv.line((x0, y1), (x1, y0), 0.35 * lws, 0.6);
                cv.text(x0 + 4.0, y1 - 12.0, 8.0, 0.35, &format!("IMAGE: {path}"));
            }
            _ => {}
        },
    }

    let mut prims = cv.prims;
    if b.border {
        let mut edge = Canvas::new(None);
        edge.rect(rect[0], rect[1], rect[2], rect[3], 0.75 * lws);
        prims.extend(edge.prims);
    }
    prims
}

fn is_scaled(source: &BoxSource) -> bool {
    matches!(
        source,
        BoxSource::PlanView { .. }
            | BoxSource::Elevation { .. }
            | BoxSource::Section { .. }
            | BoxSource::CadDetail { .. }
    )
}

/// Caption and scale note under a box.
fn label_prims(b: &LayoutBox) -> Vec<Prim> {
    let Some(label) = &b.label else {
        return Vec::new();
    };
    let r = box_rect_pt(b);
    let mut cv = Canvas::new(None);
    cv.bold(r[0], r[1] - 13.0, 10.0, label);
    if is_scaled(&b.source) {
        cv.text(
            r[0],
            r[1] - 23.0,
            7.0,
            0.35,
            &format!("SCALE: {}", b.scale.label()),
        );
    }
    cv.prims
}

/// Every stroked segment a box draws (content plus border), in paper inches.
///
/// Honors the box's `clip`, scale and `line_weight_scale` exactly like
/// [`render_pdf`]. Pen widths map to [`LineWeight`] (0.7 pt and up Heavy,
/// 0.35 pt and up Medium, otherwise Light); every segment has kind
/// [`EdgeKind::Silhouette`]. Fills and text are not included.
pub fn render_box_lines(b: &LayoutBox, cx: &LayoutRenderContext) -> Vec<Line2> {
    let scenes = SceneSource::new(cx.scene);
    let mut out = Vec::new();
    for p in box_prims(b, cx, &scenes) {
        if let Prim::Stroke {
            pts, closed, width, ..
        } = p
        {
            let weight = if width >= 0.7 - 1e-9 {
                LineWeight::Heavy
            } else if width >= 0.35 - 1e-9 {
                LineWeight::Medium
            } else {
                LineWeight::Light
            };
            let n = if closed { pts.len() } else { pts.len() - 1 };
            for i in 0..n {
                let (a, c) = (pts[i], pts[(i + 1) % pts.len()]);
                out.push(Line2 {
                    a: Point::new(a.0 / 72.0, a.1 / 72.0),
                    b: Point::new(c.0 / 72.0, c.1 / 72.0),
                    weight,
                    kind: EdgeKind::Silhouette,
                });
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn box_prims_for_test(
    b: &LayoutBox,
    cx: &LayoutRenderContext,
    scenes: &SceneSource,
) -> Vec<Prim> {
    box_prims(b, cx, scenes)
}

// ------------------------------------------------------------ title block --

fn draw_field(cv: &mut Canvas, x: f64, y: f64, w: f64, h: f64, label: &str, value: &str) {
    cv.rect(x, y, x + w, y + h, 0.75);
    cv.text(x + 3.0, y + h - 7.5, 5.5, 0.35, label);
    let size = (h * 0.2).clamp(8.0, 14.0);
    let max_lines = (((h - 10.0) / (size * 1.2)).floor() as usize).max(1);
    for (i, line) in wrap_text(value, size, w - 8.0)
        .into_iter()
        .take(max_lines)
        .enumerate()
    {
        let base = (y + h - 10.0 - size - i as f64 * size * 1.2).max(y + 3.0);
        cv.text(x + 4.0, base, size, 0.0, &line);
    }
}

fn draw_title_block(cv: &mut Canvas, layout: &Layout, ctx: &MacroContext) {
    let (w_in, h_in) = layout.sheet.inches();
    let (sw, sh, m) = (w_in * 72.0, h_in * 72.0, layout.margins_in * 72.0);
    cv.rect(m, m, sw - m, sh - m, 1.5);
    let fields = layout.title_block.expand_macros(ctx);
    match &layout.title_block.style {
        TitleBlockStyle::RightStrip => {
            let w = layout.right_strip_in() * 72.0;
            let x0 = sw - m - w;
            cv.line((x0, m), (x0, sh - m), 1.5, 0.0);
            // Taller boxes for values that wrap; the strip is always filled.
            let weights: Vec<f64> = fields
                .iter()
                .map(|(_, v)| 0.6 + 0.4 * wrap_text(v, 11.0, w - 8.0).len().max(1) as f64)
                .collect();
            let total: f64 = weights.iter().sum();
            let mut top = sh - m;
            for ((label, value), wt) in fields.iter().zip(&weights) {
                let h = (sh - 2.0 * m) * wt / total;
                draw_field(cv, x0, top - h, w, h, label, value);
                top -= h;
            }
        }
        TitleBlockStyle::BottomStrip => {
            let h = BOTTOM_STRIP_IN * 72.0;
            cv.line((m, m + h), (sw - m, m + h), 1.5, 0.0);
            let weights: Vec<f64> = fields
                .iter()
                .map(|(l, v)| PdfDoc::text_width(v, 10.0).max(PdfDoc::text_width(l, 6.0)) + 24.0)
                .collect();
            let total: f64 = weights.iter().sum();
            let mut x = m;
            for ((label, value), wt) in fields.iter().zip(&weights) {
                let w = (sw - 2.0 * m) * wt / total;
                draw_field(cv, x, m, w, h, label, value);
                x += w;
            }
        }
        TitleBlockStyle::Custom(items) => {
            let tp = |p: Point| (p.x * 72.0, p.y * 72.0);
            for o in items {
                let mut o = o.clone();
                if let CadItem::Text { text, .. } = &mut o.item {
                    *text = ctx.expand(text);
                }
                draw_cad_item(cv, &o, &tp, 72.0, 0.75);
            }
        }
    }
}

/// The scale a page announces: its boxes' common scale, `AS NOTED` when they
/// differ, `NTS` when no box is scaled.
fn page_scale_label(page: &LayoutPage) -> String {
    let mut labels: Vec<&str> = page
        .boxes
        .iter()
        .filter(|b| is_scaled(&b.source))
        .map(|b| b.scale.label())
        .collect();
    labels.dedup();
    labels.sort_unstable();
    labels.dedup();
    match labels.as_slice() {
        [] => "NTS".to_string(),
        [one] => (*one).to_string(),
        _ => "AS NOTED".to_string(),
    }
}

fn draw_sheet_index(cv: &mut Canvas, layout: &Layout) {
    let (lo, hi) = layout.drawing_area();
    let x = lo.x * 72.0 + 36.0;
    let mut y = (hi.y * 72.0 - 4.5 * 72.0).max(lo.y * 72.0 + 40.0);
    cv.bold(x, y, 14.0, "SHEET INDEX");
    y -= 8.0;
    cv.line((x, y), (x + 4.0 * 72.0, y), 0.75, 0.0);
    for p in &layout.pages {
        y -= 16.0;
        cv.text(x, y, 10.0, 0.0, &p.sheet_number());
        cv.text(x + 54.0, y, 10.0, 0.0, &p.title.to_uppercase());
    }
}

// -------------------------------------------------------------------- PDF --

fn draw_page(
    cv: &mut Canvas,
    layout: &Layout,
    index: usize,
    cx: &LayoutRenderContext,
    scenes: &SceneSource,
) {
    let page = &layout.pages[index];
    for b in &page.boxes {
        cv.prims.extend(box_prims(b, cx, scenes));
        cv.prims.extend(label_prims(b));
    }
    let tp = |p: Point| (p.x * 72.0, p.y * 72.0);
    for o in &page.cad {
        draw_cad_item(cv, o, &tp, 72.0, 0.5);
    }

    let mut ctx = cx.macros.clone();
    if ctx.project_name.is_empty() {
        ctx.project_name = cx.project.name.clone();
    }
    ctx.sheet_number = page.sheet_number();
    ctx.sheet_title = page.title.clone();
    ctx.scale = page_scale_label(page);
    draw_title_block(cv, layout, &ctx);

    let (w_in, _) = layout.sheet.inches();
    let m = layout.margins_in * 72.0;
    let text = format!("SHEET {} OF {}", index + 1, layout.pages.len());
    cv.text_right(w_in * 72.0 - m, m * 0.4, 8.0, 0.0, &text);
    if index == 0 && layout.sheet_index {
        draw_sheet_index(cv, layout);
    }
}

fn emit(doc: &mut PdfDoc, prims: &[Prim]) {
    let mut gray = f64::NAN;
    let mut set_gray = |doc: &mut PdfDoc, g: f64| {
        if g != gray {
            doc.set_gray(g);
            gray = g;
        }
    };
    for p in prims {
        match p {
            Prim::Stroke {
                pts,
                closed,
                width,
                gray: g,
            } => {
                set_gray(doc, *g);
                if pts.len() == 2 && !closed {
                    doc.line(pts[0].0, pts[0].1, pts[1].0, pts[1].1, *width);
                } else {
                    doc.polyline(pts, *closed, *width);
                }
            }
            Prim::Fill { pts, gray: g } => doc.filled_polygon(pts, *g),
            Prim::Text {
                x,
                y,
                size,
                gray: g,
                text,
            } => {
                set_gray(doc, *g);
                doc.text(*x, *y, *size, text);
            }
        }
    }
}

/// Print the layout: one PDF page per layout page, sized to `layout.sheet`.
///
/// Each page draws its boxes (content, border, label), page CAD, the title
/// block with macros expanded for that sheet, `SHEET n OF m`, and, on the
/// first page when `layout.sheet_index` is set, the sheet index. An empty
/// layout produces one blank page. Elevations and sections use `cx.scene`, or
/// a scene built from the project (once) when it is `None`.
pub fn render_pdf(layout: &Layout, cx: &LayoutRenderContext) -> Vec<u8> {
    let (w_in, h_in) = layout.sheet.inches();
    let mut doc = PdfDoc::new(w_in * 72.0, h_in * 72.0);
    let scenes = SceneSource::new(cx.scene);
    for index in 0..layout.pages.len() {
        if index > 0 {
            doc.new_page();
        }
        let mut cv = Canvas::new(None);
        draw_page(&mut cv, layout, index, cx, &scenes);
        emit(&mut doc, &cv.prims);
    }
    doc.finish()
}
