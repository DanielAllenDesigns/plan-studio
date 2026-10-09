//! Scaled floor-plan sheet: border, title block, walls, openings, dimensions,
//! CAD items and room labels.

use super::{LineJoin, PdfColor, PdfDoc, Scale, SheetSize};
use crate::schedule::room_name;
use plan_core::text_styles::TextStyles;
use plan_core::{
    wall_outlines, CadItem, DimFormat, Dimension, DimensionKind, Floor, LayerSet, LineStyle,
    Opening, OpeningKind, Point, Project, Room, Wall,
};
use std::f64::consts::TAU;

/// Drawing-sheet background of Daniel's Chief layout preference (RGB).
pub const CHIEF_SHEET_BACKGROUND: (u8, u8, u8) = (249, 248, 244);

/// Which area a room label prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomAreaBasis {
    /// Area inside the finished wall surfaces (Chief's Interior Area).
    Interior,
    /// Area of the wall-centerline polygon.
    Centerline,
}

/// Options for [`plan_sheet_with`].
#[derive(Debug, Clone, PartialEq)]
pub struct PlanSheetOptions {
    /// Text format for dimension labels.
    pub dim_format: DimFormat,
    /// Page background colour; `None` leaves the paper white.
    /// [`CHIEF_SHEET_BACKGROUND`] matches Chief's layout preference.
    pub background: Option<(u8, u8, u8)>,
    /// Draw each layer at its `line_weight` (1/100 mm); otherwise 0.7 pt.
    pub layer_weights: bool,
    /// Multiplier applied to layer line weights.
    pub weight_scale: f64,
    /// Draw dimensions and CAD items in their layer colour instead of black.
    pub layer_colors: bool,
    /// Area printed under room names.
    pub room_area: RoomAreaBasis,
    /// The dimension set's text style (empty: "Dimension Text Style"); a
    /// dimension's own style wins.
    pub dim_text_style: String,
    /// The dimension set holds its text size on paper at any scale.
    pub dim_printed_size: bool,
}

impl Default for PlanSheetOptions {
    fn default() -> Self {
        Self {
            dim_format: DimFormat::default(),
            background: None,
            layer_weights: true,
            weight_scale: 1.0,
            layer_colors: false,
            room_area: RoomAreaBasis::Interior,
            dim_text_style: String::new(),
            dim_printed_size: false,
        }
    }
}

impl PlanSheetOptions {
    /// The defaults plus the Chief off-white sheet background.
    pub fn chief_layout() -> Self {
        Self {
            background: Some(CHIEF_SHEET_BACKGROUND),
            ..Self::default()
        }
    }
}

/// Text for the title block strip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleBlock {
    pub project_name: String,
    pub sheet_title: String,
    pub sheet_number: String,
    pub date: String,
    pub designer: String,
}

/// Result of [`plan_sheet`].
#[derive(Debug, Clone)]
pub struct PlanSheetResult {
    /// The finished one-page PDF.
    pub pdf: Vec<u8>,
    /// The scale actually drawn (the requested one, or the largest smaller
    /// one that fits).
    pub scale_used: Scale,
    /// `true` when the plan fit at the *requested* scale. When `false`,
    /// `scale_used` is smaller than requested; if even 1/8" = 1'-0" overflows
    /// the sheet the plan is still drawn at 1/8" and runs past the margins.
    pub fitted: bool,
}

/// Plan-space bounding box of all wall footprints: `(min, max)` in inches.
fn plan_bounds(f: &Floor) -> (Point, Point) {
    let mut it = f.walls.iter().flat_map(|w| w.footprint());
    let Some(first) = it.next() else {
        return (Point::ZERO, Point::ZERO);
    };
    it.fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    })
}

/// Shorten `s` until it fits `max_w` points at `size`.
fn fit_text(s: &str, size: f64, max_w: f64) -> String {
    let mut t = s.to_string();
    while !t.is_empty() && PdfDoc::text_width(&t, size) > max_w {
        t.pop();
    }
    t
}

/// Sheets from 24" wide up get the larger margins and type.
fn is_large(sheet: SheetSize) -> bool {
    sheet.inches().0 >= 24.0
}

/// Page-space layout shared by the drawing routines.
struct Layout {
    margin: f64,
    title_h: f64,
    title_w: f64,
    sheet_w: f64,
    sheet_h: f64,
    pad: f64,
}

impl Layout {
    fn new(sheet: SheetSize) -> Self {
        let (w, h) = sheet.points();
        let large = is_large(sheet);
        Layout {
            margin: if large { 36.0 } else { 18.0 },
            title_h: (h * 0.06).clamp(40.0, 72.0),
            title_w: (w * 0.45).max(300.0).min(w * 0.8),
            sheet_w: w,
            sheet_h: h,
            pad: if large { 24.0 } else { 12.0 },
        }
    }

    /// Space available to the plan: `(x, y, w, h)` in points.
    fn plan_area(&self) -> (f64, f64, f64, f64) {
        (
            self.margin + self.pad,
            self.margin + self.title_h + self.pad,
            self.sheet_w - 2.0 * (self.margin + self.pad),
            self.sheet_h - 2.0 * self.margin - self.title_h - 2.0 * self.pad,
        )
    }
}

/// Print one floor of a project as a scaled PDF sheet with the default
/// [`PlanSheetOptions`], except that room labels keep the wall-centerline
/// area this function has always printed.
///
/// The plan is centred in the area above the title block. If it does not fit
/// at `scale`, successively smaller scales are tried; see [`PlanSheetResult`]
/// for how the outcome is reported. `floor` is an index into
/// `project.floors`; an out-of-range index prints an empty sheet.
pub fn plan_sheet(
    project: &Project,
    floor: usize,
    rooms: &[Room],
    sheet: SheetSize,
    scale: Scale,
    title_block: &TitleBlock,
) -> PlanSheetResult {
    let opts = PlanSheetOptions {
        room_area: RoomAreaBasis::Centerline,
        ..PlanSheetOptions::default()
    };
    plan_sheet_with(project, floor, rooms, sheet, scale, title_block, &opts)
}

/// [`plan_sheet`] with explicit [`PlanSheetOptions`].
///
/// Walls print as gray fills with black outlines, openings clear the wall,
/// the floor's dimensions and CAD items are drawn, and layer visibility and
/// line weights come from `project.layers`.
pub fn plan_sheet_with(
    project: &Project,
    floor: usize,
    rooms: &[Room],
    sheet: SheetSize,
    scale: Scale,
    title_block: &TitleBlock,
    opts: &PlanSheetOptions,
) -> PlanSheetResult {
    let empty = Floor::new("", 0.0);
    let f = project.floors.get(floor).unwrap_or(&empty);
    let lay = Layout::new(sheet);
    let (lo, hi) = plan_bounds(f);
    let (plan_w, plan_h) = (hi.x - lo.x, hi.y - lo.y);
    let (ax, ay, aw, ah) = lay.plan_area();
    let fits = |s: Scale| {
        let k = s.points_per_inch();
        plan_w * k <= aw && plan_h * k <= ah
    };

    let mut used = scale;
    while !fits(used) {
        match used.smaller() {
            Some(s) => used = s,
            None => break,
        }
    }
    let fitted = used == scale;

    let k = used.points_per_inch();
    // Centre the plan bounding box in the plan area.
    let ox = ax + (aw - plan_w * k) * 0.5;
    let oy = ay + (ah - plan_h * k) * 0.5;
    let tp = |p: Point| (ox + (p.x - lo.x) * k, oy + (p.y - lo.y) * k);

    let mut doc = PdfDoc::new(lay.sheet_w, lay.sheet_h);
    if let Some((r, g, b)) = opts.background {
        doc.fill_page(PdfColor::Rgb(r, g, b));
    }
    draw_border(&mut doc, &lay);
    draw_title_block(&mut doc, &lay, title_block, used);

    // Scale note, bottom-left of the sheet.
    let note_size = (lay.title_h * 0.26).clamp(8.0, 14.0);
    let mut note = used.label().to_string();
    if !fitted {
        note.push_str("  (reduced to fit sheet)");
    }
    doc.set_gray(0.0);
    doc.text(lay.margin + 12.0, lay.margin + 12.0, note_size, &note);

    let ctx = Ctx {
        layers: &project.layers,
        styles: &project.text_styles,
        opts,
        k,
        ipf: used.inches_per_foot(),
        large: is_large(sheet),
    };
    draw_walls(&mut doc, f, &tp, &ctx);
    for o in &f.openings {
        let layer = match o.kind {
            OpeningKind::Door => "Doors",
            OpeningKind::Window => "Windows",
        };
        if let Some(w) = f.wall(o.wall_id) {
            if ctx.layers.is_visible(layer) && ctx.layers.is_visible(&w.layer) {
                let over = f
                    .openings_on(w.id)
                    .any(|v| plan_core::openings::stands_over(o, v));
                draw_opening(&mut doc, w, o, &tp, &ctx, ctx.pen(layer), over);
                // Casing in plan: one loop around a mulled unit, none of its
                // own for a window over a door.
                if let Some(unit) = project.casing_unit(floor, o.id) {
                    doc.set_rgb_stroke(0, 0, 0);
                    let ext = plan_core::exterior_sign(w, &[]);
                    for part in plan_core::opening_symbol::casing_parts(w, o, unit, ext) {
                        let pts: Vec<(f64, f64)> = part.points.iter().map(|q| tp(*q)).collect();
                        for (i, a) in pts.iter().enumerate() {
                            let b = pts[(i + 1) % pts.len()];
                            doc.line(a.0, a.1, b.0, b.1, ctx.pen(layer) * 0.5);
                        }
                    }
                }
            }
        }
    }
    draw_cad(&mut doc, f, &tp, &ctx);
    draw_dimensions(&mut doc, f, &tp, &ctx);
    if ctx.layers.is_visible("Room Labels") {
        draw_room_labels(&mut doc, f, rooms, &tp, &ctx);
    }

    PlanSheetResult {
        pdf: doc.finish(),
        scale_used: used,
        fitted,
    }
}

/// Per-sheet drawing context.
struct Ctx<'a> {
    layers: &'a LayerSet,
    styles: &'a TextStyles,
    opts: &'a PlanSheetOptions,
    /// Points of paper per inch of plan.
    k: f64,
    /// Paper inches per foot of plan (the scale the sheet is printed at).
    ipf: f64,
    large: bool,
}

impl Ctx<'_> {
    /// The size in points of a dimension's number: its own text style, else
    /// the set's, else "Dimension Text Style", at the sheet's scale (a style
    /// or a set that holds a printed size prints that size on paper).
    fn dim_text_pt(&self, d: &Dimension) -> f64 {
        let name = d
            .text_style
            .as_deref()
            .filter(|n| !n.is_empty())
            .or(Some(self.opts.dim_text_style.as_str()).filter(|n| !n.is_empty()))
            .unwrap_or("Dimension Text Style");
        match self.styles.resolve(name) {
            Some(st) => (st.plan_height_at(self.ipf, self.opts.dim_printed_size) * self.k).max(2.0),
            None if self.large => 8.0,
            None => 6.5,
        }
    }

    /// Plotted pen width of a layer in points (`line_weight` is 1/100 mm).
    fn pen(&self, layer: &str) -> f64 {
        if !self.opts.layer_weights {
            return 0.7;
        }
        let hundredths = self.layers.get(layer).map_or(25, |l| l.line_weight);
        (f64::from(hundredths) / 100.0 * 72.0 / 25.4 * self.opts.weight_scale).max(0.1)
    }

    /// Select stroke and fill colours for `layer` (black unless
    /// `layer_colors` is set and the layer exists).
    fn set_colors(&self, doc: &mut PdfDoc, layer: &str) {
        let [r, g, b] = match (self.opts.layer_colors, self.layers.get(layer)) {
            (true, Some(l)) => l.color,
            _ => [0, 0, 0],
        };
        doc.set_rgb_stroke(r, g, b);
        doc.set_rgb_fill(r, g, b);
    }
}

fn draw_border(doc: &mut PdfDoc, lay: &Layout) {
    doc.rect(
        lay.margin,
        lay.margin,
        lay.sheet_w - 2.0 * lay.margin,
        lay.sheet_h - 2.0 * lay.margin,
        2.0,
    );
}

/// Chief-like title block: two rows of labelled boxes along the bottom-right.
fn draw_title_block(doc: &mut PdfDoc, lay: &Layout, tb: &TitleBlock, scale: Scale) {
    let x0 = lay.sheet_w - lay.margin - lay.title_w;
    let y0 = lay.margin;
    let row_h = lay.title_h * 0.5;
    let label_size = (row_h * 0.22).clamp(4.5, 7.0);
    let value_size = (row_h * 0.38).clamp(7.0, 12.0);

    let sheet_text = match (tb.sheet_number.is_empty(), tb.sheet_title.is_empty()) {
        (false, false) => format!("{}  {}", tb.sheet_number, tb.sheet_title),
        (false, true) => tb.sheet_number.clone(),
        _ => tb.sheet_title.clone(),
    };
    // (label, value, x fraction start, x fraction end), top row then bottom row.
    let top = [
        ("PROJECT", tb.project_name.as_str(), 0.0, 0.55),
        ("SHEET", sheet_text.as_str(), 0.55, 1.0),
    ];
    let bottom = [
        ("SCALE", scale.label(), 0.0, 0.34),
        ("DATE", tb.date.as_str(), 0.34, 0.62),
        ("DRAWN BY", tb.designer.as_str(), 0.62, 1.0),
    ];
    doc.rect(x0, y0, lay.title_w, lay.title_h, 1.5);
    for (row, y) in [(&top[..], y0 + row_h), (&bottom[..], y0)] {
        for (label, value, f0, f1) in row {
            let bx = x0 + f0 * lay.title_w;
            let bw = (f1 - f0) * lay.title_w;
            doc.rect(bx, y, bw, row_h, 0.75);
            doc.set_gray(0.35);
            doc.text(bx + 3.0, y + row_h - label_size - 2.0, label_size, label);
            doc.set_gray(0.0);
            let v = fit_text(value, value_size, bw - 8.0);
            doc.text(bx + 4.0, y + 4.0, value_size, &v);
        }
    }
}

fn pt_list(pts: &[Point], tp: &impl Fn(Point) -> (f64, f64)) -> Vec<(f64, f64)> {
    pts.iter().map(|p| tp(*p)).collect()
}

/// Walls as a gray union with a black outline.
///
/// Pass 1 strokes every joined outline with a double-width black line; pass 2
/// fills every outline gray on top. Edges shared between joined walls are
/// covered by the neighbouring fill, leaving one clean outline around the
/// union without computing mitred outlines.
fn draw_walls(doc: &mut PdfDoc, f: &Floor, tp: &impl Fn(Point) -> (f64, f64), ctx: &Ctx) {
    let outlines = wall_outlines(&f.walls, 0.5);
    let shown: Vec<(&Wall, Vec<(f64, f64)>)> = f
        .walls
        .iter()
        .zip(&outlines)
        .filter(|(w, _)| ctx.layers.is_visible(&w.layer))
        .map(|(w, o)| (w, pt_list(&o.polygon, tp)))
        .collect();
    doc.set_rgb_stroke(0, 0, 0);
    doc.set_line_join(LineJoin::Miter);
    for (w, poly) in &shown {
        doc.polygon(poly, None, Some(2.0 * ctx.pen(&w.layer)));
    }
    for (_, poly) in &shown {
        doc.polygon(poly, Some(PdfColor::Rgb(204, 204, 204)), None);
    }
}

/// Clear the wall under an opening and draw its plan symbol: jambs, the
/// leaf and swing (or sliding panels, pocket, track, projecting unit) or the
/// window lines, the same parts the plan view draws.
fn draw_opening(
    doc: &mut PdfDoc,
    w: &Wall,
    o: &Opening,
    tp: &impl Fn(Point) -> (f64, f64),
    ctx: &Ctx,
    pen: f64,
    stands_over: bool,
) {
    use plan_core::opening_symbol::{plan_symbol_in, PartKind};
    let k = ctx.k;
    let half = w.thickness * 0.5;
    // Without rooms the outside is the wall's left face (right for interior
    // walls), like the plan view without detected rooms. A window over a door
    // (a transom) is drawn dashed and leaves the door's clearing alone.
    let sym = plan_symbol_in(w, o, plan_core::exterior_sign(w, &[]), stands_over);
    // The gap overshoots the faces a little so the outline strokes vanish; a
    // niche only clears the band it is cut into.
    let g = half + 1.5 / k;
    let hi = if sym.cut.1 >= half - 1e-9 {
        g
    } else {
        sym.cut.1
    };
    let lo = if sym.cut.0 <= -half + 1e-9 {
        -g
    } else {
        sym.cut.0
    };
    if !stands_over {
        // Along the arc on a curved wall.
        for quad in w.band_quads(sym.span.0, sym.span.1, lo, hi) {
            doc.polygon(&pt_list(&quad, tp), Some(PdfColor::WHITE), None);
        }
    }
    doc.set_rgb_stroke(0, 0, 0);
    for part in &sym.parts {
        let mut pts: Vec<(f64, f64)> = part.points.iter().map(|q| tp(*q)).collect();
        if pts.len() < 2 {
            continue;
        }
        if part.closed {
            pts.push(pts[0]);
        }
        let width = match part.kind {
            PartKind::Jamb | PartKind::Frame | PartKind::Leaf | PartKind::Sill => pen,
            PartKind::Swing => pen * 0.6,
            PartKind::Glass | PartKind::Arrow | PartKind::Threshold | PartKind::Indicator => {
                pen * 0.5
            }
            PartKind::Hidden | PartKind::Track => pen * 0.5,
        };
        let dashed = matches!(part.kind, PartKind::Hidden | PartKind::Track);
        if dashed {
            doc.set_dash(&[4.0 * width.max(0.5), 2.0 * width.max(0.5)], 0.0);
        }
        for seg in pts.windows(2) {
            doc.line(seg[0].0, seg[0].1, seg[1].0, seg[1].1, width);
        }
        if dashed {
            doc.set_dash_solid();
        }
    }
}

/// Dash pattern for a layer line style.
fn dash_for(style: LineStyle, pen: f64) -> Option<Vec<f64>> {
    let u = pen.max(0.5);
    match style {
        LineStyle::Solid => None,
        LineStyle::Dashed => Some(vec![8.0 * u, 4.0 * u]),
        LineStyle::Dotted => Some(vec![u, 3.0 * u]),
        LineStyle::DashDot => Some(vec![8.0 * u, 3.0 * u, u, 3.0 * u]),
    }
}

/// CAD lines, arcs, circles, polylines and text on visible layers.
fn draw_cad(doc: &mut PdfDoc, f: &Floor, tp: &impl Fn(Point) -> (f64, f64), ctx: &Ctx) {
    let attrs = f.cad_attr_map();
    // In drawing-group order (Edit > Drawing Group).
    for o in f.cad_draw_order() {
        if !ctx.layers.is_visible(&o.layer) {
            continue;
        }
        let pen = ctx.pen(&o.layer);
        ctx.set_colors(doc, &o.layer);
        doc.set_line_width(pen);
        let dash = ctx
            .layers
            .get(&o.layer)
            .and_then(|l| dash_for(l.line_style, pen));
        match &dash {
            Some(d) => doc.set_dash(d, 0.0),
            None => doc.set_dash_solid(),
        }
        match &o.item {
            CadItem::Line { a, b } => {
                let (pa, pb) = (tp(*a), tp(*b));
                doc.line(pa.0, pa.1, pb.0, pb.1, pen);
            }
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let sweep = (end_angle - start_angle).rem_euclid(TAU);
                let c = tp(*center);
                let a0 = start_angle.to_degrees();
                doc.arc(c.0, c.1, radius * ctx.k, a0, a0 + sweep.to_degrees());
            }
            CadItem::Circle { center, radius } => {
                let c = tp(*center);
                doc.arc(c.0, c.1, radius * ctx.k, 0.0, 360.0);
            }
            CadItem::Polyline { points, closed } => {
                doc.polyline(&pt_list(points, tp), *closed, pen);
            }
            CadItem::Text {
                pos,
                text,
                height,
                angle,
            } => {
                // The plan height the text is drawn at: a printed-size style
                // holds its size on paper at the sheet's scale.
                let drawn = ctx.styles.drawn_height(
                    ctx.layers,
                    &o.layer,
                    attrs.get(&o.id).and_then(|a| a.text_style.as_deref()),
                    *height,
                    ctx.ipf,
                );
                let size = (drawn * ctx.k).max(2.0);
                let (x, y) = tp(*pos);
                // A text box: fill, frame, wrapped and aligned lines.
                let boxed = attrs.get(&o.id).and_then(|a| {
                    let item = CadItem::Text {
                        pos: *pos,
                        text: text.clone(),
                        height: drawn,
                        angle: *angle,
                    };
                    plan_core::text_box::placed(&item, a)
                });
                if let Some(pb) = boxed {
                    draw_text_box(doc, &pb, tp, ctx.k, pen);
                } else if angle.rem_euclid(TAU).abs() < 1e-9 {
                    doc.text(x, y, size, text);
                } else {
                    doc.text_rotated(x, y, size, angle.to_degrees(), text);
                }
            }
        }
    }
    doc.set_dash_solid();
    doc.set_rgb_stroke(0, 0, 0);
    doc.set_rgb_fill(0, 0, 0);
}

/// A text box on paper (TXT-1, TXT-16): the background fill, the frame and
/// the lines, wrapped and aligned as [`plan_core::text_box`] lays them out
/// with Helvetica's widths. `k` is points of paper per plan inch.
fn draw_text_box(
    doc: &mut PdfDoc,
    pb: &plan_core::text_box::PlacedBox,
    tp: &impl Fn(Point) -> (f64, f64),
    k: f64,
    pen: f64,
) {
    let k = k.max(1e-9);
    let width = |r: &plan_core::text_styles::RichRun, h: f64| {
        (if r.bold {
            PdfDoc::text_width_bold(&r.text, h * k)
        } else {
            PdfDoc::text_width(&r.text, h * k)
        }) / k
    };
    let draw = pb.draw_plan(&width);
    if let Some((quad, c)) = draw.fill {
        let pts: Vec<(f64, f64)> = quad.iter().map(|q| tp(*q)).collect();
        doc.polygon(&pts, Some(PdfColor::Rgb(c[0], c[1], c[2])), None);
    }
    if let Some(quad) = draw.border {
        let pts: Vec<(f64, f64)> = quad.iter().map(|q| tp(*q)).collect();
        doc.polygon(&pts, None, Some(pen));
    }
    for r in &draw.runs {
        let (x, y) = tp(r.at);
        let size = (r.height * k).max(2.0);
        doc.set_font_bold(r.run.bold);
        if pb.angle.rem_euclid(TAU).abs() < 1e-9 {
            doc.text(x, y, size, &r.run.text);
        } else {
            doc.text_rotated(x, y, size, pb.angle.to_degrees(), &r.run.text);
        }
    }
    doc.set_font_bold(false);
}

/// Extension lines, dimension line, 45 degree ticks and the label. Labels on
/// non-horizontal dimensions are rotated to run along the line, reading
/// bottom to top for vertical ones.
fn draw_dimension(
    doc: &mut PdfDoc,
    d: &Dimension,
    tp: &impl Fn(Point) -> (f64, f64),
    fmt: &DimFormat,
    pen: f64,
    text_pt: f64,
) {
    const TICK: f64 = 2.5;
    // Points of paper per inch of plan, for the sizes a dimension sets itself.
    let k = {
        let (o, e) = (tp(Point::ZERO), tp(Point::new(1.0, 0.0)));
        (e.0 - o.0).hypot(e.1 - o.1)
    };
    let look = &d.look;
    let own_ext = look.ext_gap.is_some() || look.ext_past.is_some() || look.ext_length.is_some();
    // Extension lines switched off per point stay off on paper.
    for ((m, e), hidden) in d.extension_lines().into_iter().zip(d.hide_ext) {
        if hidden {
            continue;
        }
        let seg = if own_ext {
            plan_core::dimension::extension_segment(
                m,
                e,
                look.ext_gap.unwrap_or(0.0),
                look.ext_past.unwrap_or(0.0),
                look.ext_length,
            )
        } else {
            Some((m, e))
        };
        if let Some((a, b)) = seg {
            let (pa, pb) = (tp(a), tp(b));
            doc.line(pa.0, pa.1, pb.0, pb.1, pen * 0.6);
        }
    }
    let (a, b) = d.line_points();
    let (pa, pb) = (tp(a), tp(b));
    doc.line(pa.0, pa.1, pb.0, pb.1, pen);
    draw_dimension_ends(doc, pa, pb, look, k, TICK, pen);
    let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
    if dx.hypot(dy) < 1e-6 {
        return;
    }
    // Read left to right or bottom to top: angle in (-90, 90].
    let mut ang = dy.atan2(dx).to_degrees();
    if ang > 90.0 + 1e-9 {
        ang -= 180.0;
    } else if ang <= -90.0 + 1e-9 {
        ang += 180.0;
    }
    let label = d.label(fmt);
    let w = PdfDoc::text_width(&label, text_pt);
    let (sin, cos) = ang.to_radians().sin_cos();
    let mid = ((pa.0 + pb.0) * 0.5, (pa.1 + pb.1) * 0.5);
    // Centre along the line, then lift the baseline off it.
    let lift = text_pt * 0.3;
    let x = mid.0 - cos * w * 0.5 - sin * lift;
    let y = mid.1 - sin * w * 0.5 + cos * lift;
    if ang.abs() < 1e-6 {
        doc.text(x, y, text_pt, &label);
    } else {
        doc.text_rotated(x, y, text_pt, ang, &label);
    }
}

/// The end marks of a dimension line: the 45 degree tick unless the
/// dimension sets an arrow, dot or none (the Arrow tab).
fn draw_dimension_ends(
    doc: &mut PdfDoc,
    pa: (f64, f64),
    pb: (f64, f64),
    look: &plan_core::dimension::DimOverrides,
    k: f64,
    tick: f64,
    pen: f64,
) {
    use plan_core::dimension::DimArrow;
    let mark = look.arrow.unwrap_or(DimArrow::Tick);
    let size = look
        .arrow_size
        .map_or(tick * 2.0, |s| s * k)
        .clamp(3.0, 24.0);
    let filled = look.arrow_filled.unwrap_or(true);
    let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
    let len = dx.hypot(dy);
    let (ux, uy) = if len > 1e-9 {
        (dx / len, dy / len)
    } else {
        (1.0, 0.0)
    };
    match mark {
        DimArrow::None => {}
        DimArrow::Tick => {
            for p in [pa, pb] {
                doc.line(p.0 - tick, p.1 - tick, p.0 + tick, p.1 + tick, pen * 1.5);
            }
        }
        DimArrow::Slash => {
            // A long drafting slash, steeper than the tick.
            let (sx, sy) = (size * 0.35, size * 0.6);
            for p in [pa, pb] {
                doc.line(p.0 - sx, p.1 - sy, p.0 + sx, p.1 + sy, pen * 1.5);
            }
        }
        DimArrow::Dot => {
            let r = size * 0.25;
            for p in [pa, pb] {
                let ring: Vec<(f64, f64)> = (0..12)
                    .map(|i| {
                        let t = TAU * f64::from(i) / 12.0;
                        (p.0 + r * t.cos(), p.1 + r * t.sin())
                    })
                    .collect();
                if filled {
                    doc.polygon(&ring, Some(PdfColor::Rgb(0, 0, 0)), None);
                } else {
                    doc.polygon(&ring, None, Some(pen));
                }
            }
        }
        DimArrow::Arrow => {
            let half = size * 0.3;
            for (tip, sx, sy) in [(pa, ux, uy), (pb, -ux, -uy)] {
                let base = (tip.0 + sx * size, tip.1 + sy * size);
                let (px, py) = (-sy * half, sx * half);
                let head = [tip, (base.0 + px, base.1 + py), (base.0 - px, base.1 - py)];
                if filled {
                    doc.polygon(&head, Some(PdfColor::Rgb(0, 0, 0)), None);
                } else {
                    doc.line(head[0].0, head[0].1, head[1].0, head[1].1, pen);
                    doc.line(head[0].0, head[0].1, head[2].0, head[2].1, pen);
                }
            }
        }
    }
}

fn draw_dimensions(doc: &mut PdfDoc, f: &Floor, tp: &impl Fn(Point) -> (f64, f64), ctx: &Ctx) {
    for d in &f.dimensions {
        let layer = match d.kind {
            DimensionKind::Manual => "Dimensions, Manual",
            DimensionKind::AutoExterior => "Dimensions, Automatic",
            DimensionKind::Temporary => continue,
        };
        if !ctx.layers.is_visible(layer) {
            continue;
        }
        ctx.set_colors(doc, layer);
        let text_pt = ctx.dim_text_pt(d);
        draw_dimension(doc, d, tp, &ctx.opts.dim_format, ctx.pen(layer), text_pt);
    }
    doc.set_rgb_stroke(0, 0, 0);
    doc.set_rgb_fill(0, 0, 0);
}

fn draw_room_labels(
    doc: &mut PdfDoc,
    f: &Floor,
    rooms: &[Room],
    tp: &impl Fn(Point) -> (f64, f64),
    ctx: &Ctx,
) {
    let size = if ctx.large { 10.0 } else { 7.0 };
    doc.set_gray(0.0);
    for r in rooms {
        let (cx, cy) = tp(r.centroid);
        let sq_ft = match ctx.opts.room_area {
            RoomAreaBasis::Interior if r.interior_area_sq_in > 0.0 => r.interior_area_sq_ft(),
            _ => r.area_sq_ft(),
        };
        doc.set_font_bold(true);
        doc.text_centered(cx, cy + size * 0.2, size, &room_name(f, r));
        doc.set_font_bold(false);
        let area = format!("{sq_ft:.0} SF");
        doc.text_centered(cx, cy - size * 1.0, size * 0.85, &area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::tests::{check_xref, count};
    use crate::test_support::{house, rect_walls};
    use plan_core::{detect_rooms, WallKind};

    fn tb() -> TitleBlock {
        TitleBlock {
            project_name: "Smith Residence".into(),
            sheet_title: "First Floor Plan".into(),
            sheet_number: "A-101".into(),
            date: "2026-10-07".into(),
            designer: "DAD".into(),
        }
    }

    fn text_of(pdf: &[u8]) -> String {
        pdf.iter().map(|&b| b as char).collect()
    }

    #[test]
    fn house_fits_arch_d_quarter_inch() {
        let p = house();
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        let r = plan_sheet(&p, 0, &rooms, SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(r.fitted);
        assert_eq!(r.scale_used, Scale::QuarterInch);
        assert!(r.pdf.starts_with(b"%PDF-1.4"));
        assert!(r.pdf.ends_with(b"%%EOF"));
        assert_eq!(count(&r.pdf, "/Type /Page"), 1);
        check_xref(&r.pdf);
        let t = text_of(&r.pdf);
        for label in ["PROJECT", "SHEET", "SCALE", "DATE", "DRAWN BY"] {
            assert!(t.contains(label), "missing {label}");
        }
        assert!(t.contains("Smith Residence"));
        assert!(t.contains("1/4\" = 1'-0\""));
        assert!(t.contains("/MediaBox [0 0 2592 1728]"));
        // Room label and area, drawn at the centroid: 40' x 30' centerline.
        assert!(t.contains("1200 SF"));
    }

    #[test]
    fn oversize_plan_steps_down_scale() {
        let mut p = plan_core::Project::new("big");
        // 100' x 60' overflows Letter at every scale, so it lands on 1/8".
        rect_walls(&mut p, 1200.0, 720.0, 6.5, WallKind::Exterior);
        let r = plan_sheet(
            &p,
            0,
            &[],
            SheetSize::Letter,
            Scale::QuarterInch,
            &TitleBlock::default(),
        );
        assert!(!r.fitted);
        assert_eq!(r.scale_used, Scale::EighthInch);
        assert!(text_of(&r.pdf).contains("reduced to fit sheet"));
        check_xref(&r.pdf);
    }

    #[test]
    fn scale_math() {
        assert!((Scale::QuarterInch.points_per_inch() - 1.5).abs() < 1e-12);
        assert!((Scale::EighthInch.points_per_inch() - 0.75).abs() < 1e-12);
        assert_eq!(Scale::EighthInch.smaller(), None);
        assert_eq!(Scale::HalfInch.smaller(), Some(Scale::QuarterInch));
        assert_eq!(SheetSize::ArchD.points(), (2592.0, 1728.0));
    }

    #[test]
    fn empty_or_bad_floor_still_produces_pdf() {
        let p = plan_core::Project::new("empty");
        let r = plan_sheet(
            &p,
            5,
            &[],
            SheetSize::Tabloid,
            Scale::HalfInch,
            &TitleBlock::default(),
        );
        assert!(r.fitted);
        check_xref(&r.pdf);
    }

    use crate::pdf::tests::{assert_balanced, check_objects, content_streams, operators};
    use plan_core::{CadObject, Dimension};

    fn with_dim(p: &mut plan_core::Project, a: Point, b: Point, offset: f64) {
        p.floors[0]
            .dimensions
            .push(Dimension::new(900, DimensionKind::Manual, a, b, offset));
    }

    /// Every `a b c d e f Tm` in the page content as `[a, b, c, d, e, f]`.
    fn tm_matrices(pdf: &[u8]) -> Vec<[f64; 6]> {
        let mut out = Vec::new();
        for s in content_streams(pdf) {
            let ops = operators(&s);
            for (i, op) in ops.iter().enumerate() {
                if op == "Tm" {
                    let mut m = [0.0; 6];
                    for (j, v) in m.iter_mut().enumerate() {
                        *v = ops[i - 6 + j].parse().unwrap();
                    }
                    out.push(m);
                }
            }
        }
        out
    }

    #[test]
    fn vertical_dimension_uses_rotated_text() {
        let mut p = house();
        // 360" vertical dimension left of the house, line pushed 24" outward.
        with_dim(&mut p, Point::new(0.0, 0.0), Point::new(0.0, 360.0), -24.0);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        let r = plan_sheet(&p, 0, &rooms, SheetSize::ArchD, Scale::QuarterInch, &tb());
        check_xref(&r.pdf);
        check_objects(&r.pdf);
        let t = text_of(&r.pdf);
        assert!(t.contains("(30'-0\\\")") || t.contains("(30'-0\\\" )") || t.contains("30'-0"));
        let tms = tm_matrices(&r.pdf);
        assert_eq!(tms.len(), 1, "one rotated label");
        let m = tms[0];
        // 90 degrees: [cos sin -sin cos] = [0 1 -1 0], so not the identity.
        assert!(m[0].abs() < 1e-3 && (m[1] - 1.0).abs() < 1e-3);
        assert!((m[2] + 1.0).abs() < 1e-3 && m[3].abs() < 1e-3);
        for s in content_streams(&r.pdf) {
            assert_balanced(&s);
        }
    }

    #[test]
    fn horizontal_dimension_stays_unrotated_and_uses_format() {
        let mut p = house();
        with_dim(&mut p, Point::new(0.0, 0.0), Point::new(480.0, 0.0), -24.0);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        let r = plan_sheet(&p, 0, &rooms, SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(tm_matrices(&r.pdf).is_empty());
        assert!(text_of(&r.pdf).contains("40'-0"));
        let opts = PlanSheetOptions {
            dim_format: DimFormat::metric_mm(),
            ..PlanSheetOptions::default()
        };
        let r = plan_sheet_with(
            &p,
            0,
            &rooms,
            SheetSize::ArchD,
            Scale::QuarterInch,
            &tb(),
            &opts,
        );
        assert!(text_of(&r.pdf).contains("(12192)"));
    }

    #[test]
    fn sloped_dimension_label_follows_the_line() {
        let mut p = house();
        with_dim(&mut p, Point::new(0.0, 0.0), Point::new(480.0, 360.0), 12.0);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        let r = plan_sheet(&p, 0, &rooms, SheetSize::ArchD, Scale::QuarterInch, &tb());
        let m = tm_matrices(&r.pdf)[0];
        // atan(360/480) = 36.87 degrees.
        assert!((m[0] - 0.8).abs() < 1e-2 && (m[1] - 0.6).abs() < 1e-2);
    }

    #[test]
    fn walls_are_rgb_gray_with_black_outline() {
        let p = house();
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        let t = text_of(&r.pdf);
        assert!(t.contains("0 0 0 RG"));
        assert!(t.contains("0.8 0.8 0.8 rg"), "gray wall fill");
        // The opening gap clears the wall in white.
        assert!(t.contains("q 1 g "));
        assert!(t.contains(" f Q"));
    }

    #[test]
    fn layer_weights_and_visibility_are_honoured() {
        let mut p = house();
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        // "Walls, Normal" is 0.5 mm = 1.417 pt; outline pass strokes at twice that.
        assert!(text_of(&r.pdf).contains("q 2.835 w "));
        p.layers.get_mut("Walls, Normal").unwrap().line_weight = 100;
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(text_of(&r.pdf).contains("q 5.669 w "));
        let flat = PlanSheetOptions {
            layer_weights: false,
            ..PlanSheetOptions::default()
        };
        let r = plan_sheet_with(
            &p,
            0,
            &[],
            SheetSize::ArchD,
            Scale::QuarterInch,
            &tb(),
            &flat,
        );
        assert!(text_of(&r.pdf).contains("q 1.4 w "));
        // Hidden layers disappear: no fills at all without walls or openings.
        p.layers.set_display("Walls, Normal", false);
        p.layers.set_display("Doors", false);
        p.layers.set_display("Windows", false);
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(!text_of(&r.pdf).contains("0.8 0.8 0.8 rg"));
    }

    #[test]
    fn room_labels_use_interior_area_and_bold_name() {
        let mut p = house();
        p.set_room_name(0, Point::new(100.0, 100.0), "Great Room", "Great Room", &[]);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        let r0 = &rooms[0];
        assert!(r0.interior_area_sq_in > 0.0);
        assert!(r0.interior_area_sq_ft() < r0.area_sq_ft());
        let interior = format!("({:.0} SF)", r0.interior_area_sq_ft());
        let centerline = format!("({:.0} SF)", r0.area_sq_ft());
        let r = plan_sheet_with(
            &p,
            0,
            &rooms,
            SheetSize::ArchD,
            Scale::QuarterInch,
            &tb(),
            &PlanSheetOptions::default(),
        );
        let t = text_of(&r.pdf);
        assert!(
            t.contains(&interior) && !t.contains(&centerline),
            "{interior}"
        );
        assert!(t.contains("BT /F2 "), "room name is bold");
        // The legacy entry point keeps the centerline area.
        let legacy = plan_sheet(&p, 0, &rooms, SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(text_of(&legacy.pdf).contains(&centerline));
        // Hiding the Room Labels layer drops the labels.
        p.layers.set_display("Room Labels", false);
        let r = plan_sheet(&p, 0, &rooms, SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(!text_of(&r.pdf).contains("Great Room"));
    }

    #[test]
    fn cad_items_are_drawn() {
        let mut p = house();
        let items = [
            CadItem::Line {
                a: Point::new(0.0, -40.0),
                b: Point::new(100.0, -40.0),
            },
            CadItem::Arc {
                center: Point::new(200.0, -40.0),
                radius: 20.0,
                start_angle: 0.0,
                end_angle: std::f64::consts::PI,
            },
            CadItem::Circle {
                center: Point::new(300.0, -40.0),
                radius: 10.0,
            },
            CadItem::Polyline {
                points: vec![
                    Point::new(0.0, -60.0),
                    Point::new(50.0, -60.0),
                    Point::new(50.0, -80.0),
                ],
                closed: true,
            },
            CadItem::Text {
                pos: Point::new(10.0, -100.0),
                text: "NOTE ONE".into(),
                height: 4.0,
                angle: 0.0,
            },
            CadItem::Text {
                pos: Point::new(10.0, -100.0),
                text: "NOTE TWO".into(),
                height: 4.0,
                angle: std::f64::consts::FRAC_PI_2,
            },
        ];
        for (i, item) in items.into_iter().enumerate() {
            p.floors[0].cad.push(CadObject {
                id: 500 + i as u64,
                layer: "CAD, Default".into(),
                item,
            });
        }
        // A dashed layer draws with a dash pattern, then resets to solid.
        p.layers.get_mut("CAD, Default").unwrap().line_style = LineStyle::Dashed;
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        check_xref(&r.pdf);
        check_objects(&r.pdf);
        let t = text_of(&r.pdf);
        // 2 quarter-circle beziers per half arc is 2, circle is 4.
        assert!(t.matches(" c ").count() >= 2 + 4);
        assert!(t.contains("(NOTE ONE) Tj"));
        assert!(t.contains("(NOTE TWO) Tj"));
        assert!(t.contains("] 0 d"), "dash pattern set");
        assert!(t.trim_end().contains("[] 0 d"));
        assert_eq!(tm_matrices(&r.pdf).len(), 1, "only the angled text rotates");
        // Hidden CAD layer: nothing drawn.
        p.layers.set_display("CAD, Default", false);
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(!text_of(&r.pdf).contains("NOTE ONE"));
    }

    #[test]
    fn cad_draws_in_drawing_group_order() {
        let mut p = house();
        for (i, t) in ["FIRSTTEXT", "SECONDTEXT"].iter().enumerate() {
            p.floors[0].cad.push(CadObject {
                id: 800 + i as u64,
                layer: "CAD, Default".into(),
                item: CadItem::Text {
                    pos: Point::new(10.0, -100.0 - 20.0 * i as f64),
                    text: (*t).into(),
                    height: 4.0,
                    angle: 0.0,
                },
            });
        }
        let order = |p: &plan_core::Project| {
            let r = plan_sheet(p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
            let t = text_of(&r.pdf);
            (
                t.find("(FIRSTTEXT) Tj").expect("first drawn"),
                t.find("(SECONDTEXT) Tj").expect("second drawn"),
            )
        };
        let (a, b) = order(&p);
        assert!(a < b, "drawn in the order they were made");
        // Bring the first to the front: it now draws last.
        p.drawing_group_to_front(0, &[plan_core::ObjectRef::Cad(800)]);
        let (a, b) = order(&p);
        assert!(b < a, "the first text is in front of the second now");
    }

    /// The font size in points a string is drawn at (`BT /F size Tf ... (text) Tj`).
    fn size_of(pdf: &[u8], text: &str) -> f64 {
        let t = text_of(pdf);
        let at = t.find(&format!("({text}) Tj")).expect("text drawn");
        let bt = t[..at].rfind("BT /").expect("BT");
        t[bt..at]
            .split_whitespace()
            .nth(2)
            .and_then(|v| v.parse().ok())
            .expect("size")
    }

    #[test]
    fn printed_size_text_prints_the_same_size_at_any_sheet_scale() {
        let mut p = house();
        p.floors[0].cad.push(CadObject {
            id: 700,
            layer: "Text".into(),
            item: CadItem::Text {
                pos: Point::new(10.0, -100.0),
                text: "KITCHEN".into(),
                height: 6.0,
                angle: 0.0,
            },
        });
        with_dim(&mut p, Point::new(0.0, 0.0), Point::new(480.0, 0.0), -24.0);
        let size = |p: &plan_core::Project, scale, opts: &PlanSheetOptions| {
            let r = plan_sheet_with(p, 0, &[], SheetSize::ArchD, scale, &tb(), opts);
            (size_of(&r.pdf, "KITCHEN"), size_of(&r.pdf, "40'-0\""))
        };
        let opts = PlanSheetOptions::default();
        // Character height: the plan height prints larger at a bigger scale.
        let (t4, d4) = size(&p, Scale::QuarterInch, &opts);
        let (t2, d2) = size(&p, Scale::HalfInch, &opts);
        assert!(
            (t4 - 9.0).abs() < 1e-6 && (t2 - 18.0).abs() < 1e-6,
            "{t4} {t2}"
        );
        assert!(
            (d4 - 6.75).abs() < 1e-6 && (d2 - 13.5).abs() < 1e-6,
            "{d4} {d2}"
        );
        // Printed size 1/8" = 9 pt, and a printed-size dimension set.
        let i = p
            .text_styles
            .styles
            .iter()
            .position(|s| s.name == "Default Text Style")
            .unwrap();
        p.text_styles.styles[i].use_printed_size(true);
        let held = PlanSheetOptions {
            dim_printed_size: true,
            ..PlanSheetOptions::default()
        };
        for scale in [Scale::QuarterInch, Scale::HalfInch, Scale::EighthInch] {
            let (t, d) = size(&p, scale, &held);
            assert!((t - 9.0).abs() < 1e-6, "{scale:?}: text {t}");
            // The Dimension Text Style's 4.5" at 1/4" scale is 3/32" on
            // paper: 6.75 pt, held at every scale.
            assert!((d - 6.75).abs() < 1e-6, "{scale:?}: dimension {d}");
        }
    }

    #[test]
    fn hidden_extension_lines_are_not_drawn() {
        let mut p = house();
        with_dim(&mut p, Point::new(0.0, 0.0), Point::new(480.0, 0.0), -24.0);
        let lines = |p: &plan_core::Project| {
            let r = plan_sheet(p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
            text_of(&r.pdf).matches(" l S Q").count()
        };
        let all = lines(&p);
        p.floors[0].dimensions.last_mut().unwrap().hide_ext = [true, true];
        let none = lines(&p);
        assert!(none < all, "{none} vs {all}");
        assert_eq!(all - none, 2);
    }

    #[test]
    fn layer_colours_are_optional() {
        let mut p = house();
        with_dim(&mut p, Point::new(0.0, 0.0), Point::new(480.0, 0.0), -24.0);
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(!text_of(&r.pdf).contains("0.706 0 0 RG"));
        let opts = PlanSheetOptions {
            layer_colors: true,
            ..PlanSheetOptions::default()
        };
        let r = plan_sheet_with(
            &p,
            0,
            &[],
            SheetSize::ArchD,
            Scale::QuarterInch,
            &tb(),
            &opts,
        );
        // "Dimensions, Manual" is [180, 0, 0].
        assert!(text_of(&r.pdf).contains("0.706 0 0 RG"));
    }

    #[test]
    fn chief_background_colour_option() {
        let p = house();
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(!text_of(&r.pdf).contains(" rg 0 0 2592 1728 re f"));
        let r = plan_sheet_with(
            &p,
            0,
            &[],
            SheetSize::ArchD,
            Scale::QuarterInch,
            &tb(),
            &PlanSheetOptions::chief_layout(),
        );
        let t = text_of(&r.pdf);
        assert!(t.contains("q 0.976 0.973 0.957 rg 0 0 2592 1728 re f Q"));
        assert_eq!(CHIEF_SHEET_BACKGROUND, (249, 248, 244));
        check_xref(&r.pdf);
    }

    #[test]
    fn new_scales_and_sheets_fit_and_label() {
        let p = house();
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        for (size, scale) in [
            (SheetSize::IsoA1, Scale::Ratio(50)),
            (SheetSize::ArchE, Scale::ThreeQuarterInch),
            (SheetSize::ArchB, Scale::OneInchEq10Ft),
            (SheetSize::AnsiC, Scale::Ratio(100)),
        ] {
            let r = plan_sheet(&p, 0, &rooms, size, scale, &tb());
            assert!(r.fitted, "{} at {}", size.label(), scale.label());
            assert_eq!(r.scale_used, scale);
            assert!(text_of(&r.pdf).contains(&format!("({})", scale.label())));
            check_xref(&r.pdf);
        }
        // Too big for the sheet: steps down through the ratio ladder.
        let r = plan_sheet(&p, 0, &rooms, SheetSize::IsoA4, Scale::Ratio(20), &tb());
        assert!(!r.fitted);
        assert!(matches!(r.scale_used, Scale::Ratio(n) if n > 20));
    }

    #[test]
    fn casing_in_plan_prints_once_per_unit_and_never_for_a_transom() {
        let mut p = house();
        assert!(!p.floors[0].openings.is_empty());
        let lines = |p: &plan_core::Project| {
            let r = plan_sheet(p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
            text_of(&r.pdf).matches(" l S Q").count()
        };
        let plain = lines(&p);
        for o in &mut p.floors[0].openings {
            o.extras.spec.casing_in_plan = true;
        }
        let cased = lines(&p);
        // Two rectangles (four edges each) beside each jamb pair, on each face.
        assert_eq!(cased, plain + p.floors[0].openings.len() * 4 * 4);
        // A transom over a door adds its own jambs and head lines but no
        // casing: the door's loop is the unit's.
        let door = p.floors[0]
            .openings
            .iter()
            .find(|o| o.kind == plan_core::OpeningKind::Door)
            .unwrap()
            .id;
        let t = p.add_transom(0, door, 12.0).unwrap();
        p.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == t)
            .unwrap()
            .extras
            .spec
            .casing_in_plan = true;
        let with_transom = lines(&p);
        let transom_lines = {
            let mut q = p.clone();
            q.floors[0]
                .openings
                .iter_mut()
                .for_each(|o| o.extras.spec.casing_in_plan = false);
            lines(&q) - plain
        };
        assert_eq!(with_transom, transom_lines + cased);
    }
    #[test]
    fn a_text_box_prints_wrapped_lines_a_frame_and_a_fill() {
        use plan_core::text_box::TextBox;
        let mut p = house();
        p.floors[0].cad.push(CadObject {
            id: 800,
            layer: "CAD, Default".into(),
            item: CadItem::Text {
                pos: Point::new(10.0, -100.0),
                text: "alpha beta gamma delta".into(),
                height: 4.0,
                angle: 0.0,
            },
        });
        let plain = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(text_of(&plain.pdf).contains("(alpha beta gamma delta) Tj"));
        // 11 glyphs wide: two lines.
        let boxed = TextBox {
            width: 4.0 * 0.6 * 11.0,
            ..TextBox::default()
        };
        p.floors[0].cad_attrs.push(plan_core::cad::CadAttrs {
            text_box: boxed,
            ..plan_core::cad::CadAttrs::new(800)
        });
        let r = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        check_xref(&r.pdf);
        check_objects(&r.pdf);
        let t = text_of(&r.pdf);
        assert!(t.contains("(alpha beta) Tj"), "first line");
        assert!(t.contains("(gamma delta) Tj"), "wrapped line");
        assert!(!t.contains("(alpha beta gamma delta) Tj"));
        // A frame and a fill add drawing operators.
        p.floors[0].cad_attrs[0].text_box = TextBox {
            border: true,
            background: Some([250, 240, 200]),
            ..boxed
        };
        let framed = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        check_xref(&framed.pdf);
        assert!(text_of(&framed.pdf).len() > t.len());
        // Right-aligned lines start further right than left-aligned ones.
        let x_of = |pdf: &[u8], s: &str| -> f64 {
            let t = text_of(pdf);
            let at = t.find(&format!("({s}) Tj")).unwrap();
            let td = t[..at].rfind(" Td").unwrap();
            t[..td]
                .split_whitespace()
                .rev()
                .nth(1)
                .and_then(|v| v.parse().ok())
                .unwrap()
        };
        let left_x = x_of(&r.pdf, "alpha beta");
        p.floors[0].cad_attrs[0].text_box.halign = plan_core::text_box::HAlign::Right;
        let right = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(x_of(&right.pdf, "alpha beta") > left_x);
    }

    #[test]
    fn a_dimension_that_sets_its_own_arrow_prints_it() {
        let mut p = house();
        let id = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(0.0, -200.0),
                Point::new(120.0, -200.0),
                12.0,
            ),
        );
        let base = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        let mut with = |arrow: plan_core::dimension::DimArrow, filled: bool| {
            let d = p.floors[0]
                .dimensions
                .iter_mut()
                .find(|d| d.id == id)
                .unwrap();
            d.look.arrow = Some(arrow);
            d.look.arrow_filled = Some(filled);
            plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb())
        };
        let arrow = with(plan_core::dimension::DimArrow::Arrow, true);
        let dot = with(plan_core::dimension::DimArrow::Dot, false);
        let none = with(plan_core::dimension::DimArrow::None, true);
        for r in [&arrow, &dot, &none] {
            check_xref(&r.pdf);
            check_objects(&r.pdf);
        }
        assert_ne!(base.pdf, arrow.pdf);
        assert_ne!(arrow.pdf, dot.pdf);
        assert_ne!(none.pdf, base.pdf);
        // No ticks: fewer line operators than the tick version.
        assert!(text_of(&none.pdf).len() < text_of(&base.pdf).len());
        // The dimension's own extension line length prints too.
        let d = p.floors[0]
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap();
        d.look = plan_core::dimension::DimOverrides {
            ext_gap: Some(2.0),
            ext_past: Some(3.0),
            ext_length: Some(6.0),
            ..Default::default()
        };
        let ext = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert_ne!(ext.pdf, base.pdf);
        // A format of its own changes the number.
        let d = p.floors[0]
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap();
        d.look = plan_core::dimension::DimOverrides {
            units: Some(plan_core::units::LengthUnit::Inches),
            ..Default::default()
        };
        let inches = plan_sheet(&p, 0, &[], SheetSize::ArchD, Scale::QuarterInch, &tb());
        assert!(text_of(&inches.pdf).contains("(120\") Tj"));
        assert!(text_of(&base.pdf).contains("(10'-0\") Tj"));
    }
}
