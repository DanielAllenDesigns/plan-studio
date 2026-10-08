//! Layout rendering: boxes, title blocks and the PDF.
//!
//! Every box is first turned into a list of [`Prim`]s in PDF points (see
//! `canvas`): strokes with layer colours, weights and dashes, fills, text
//! (bold, rotated) and images. A clipped box is bracketed by clip markers that
//! become a PDF clip rectangle; the same list, soft-clipped, backs
//! [`render_box_lines`], which lets tests inspect exactly what a box draws.

use crate::annot::{PageLeader, RevisionCloud};
use crate::canvas::{
    emit, gray, rotate_prims, soft_clip, soft_clip_with, text_w, Canvas, Dash, Pen, Prim, BLACK,
};
use crate::clip::{Pt, Rect};
use crate::extent::{
    frame_for, placed_schedule_table, schedule_for, table_metrics, Frame, SceneSource,
    LINE_SPACING, ROW_H_PT, TABLE_TEXT_PT, TABLE_TITLE_H_PT,
};
use crate::hatch::wall_face_hatch;
use crate::layers::{
    LayoutLayers, LAYER_BOX_BORDERS, LAYER_REVISION_CLOUDS, LAYER_TEXT, LAYER_TITLE_BLOCK,
    TITLE_BLOCK_BASE_PT,
};
use crate::model::{
    perspective_pixels, BoxSource, Layout, LayoutBox, LayoutPage, TextAlign, BOTTOM_STRIP_IN,
};
use crate::textfit::{fit_text_box, visible_line_count, whole_line_count, TextFit, PAD_PT};
use crate::titleblock::{MacroContext, TitleBlockStyle};
use plan_3d::Scene;
use plan_core::opening_symbol::{casing_parts, plan_symbol, PartKind};
use plan_core::{
    detect_rooms, wall_outlines, CadItem, CadObject, DimFormat, DimensionKind, Floor, LayerSet,
    Opening, OpeningKind, Point, Project, Room, Wall,
};
use plan_docs::{
    materials_report, materials_to_schedule, MasterList, MaterialsScope, PdfColor, PdfDoc,
    CHIEF_SHEET_BACKGROUND,
};
use plan_elevation::{
    elevation, section, Drawing, EdgeKind, Line2, LineWeight, Options, RegionKind,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::rc::Rc;

/// Produces the 2D drawing of a camera object for [`BoxSource::Camera`] boxes.
pub type CameraDrawingFn<'a> = Box<dyn Fn(plan_core::Id) -> Option<Drawing> + 'a>;

/// A rendered raster for a [`BoxSource::Perspective`] box.
#[derive(Debug, Clone, PartialEq)]
pub struct PerspectiveImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes of RGBA, rows top to bottom.
    pub rgba: Vec<u8>,
}

/// Renders the perspective view of a camera for [`BoxSource::Perspective`].
pub type PerspectiveFn<'a> = Box<dyn Fn(plan_core::Id) -> Option<PerspectiveImage> + 'a>;

/// What a perspective box asks the renderer for: the camera and the size and
/// quality of the render (from the box's size and its DPI and samples).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PerspectiveRequest {
    pub camera_id: plan_core::Id,
    pub width: u32,
    pub height: u32,
    /// Ray-trace samples per pixel.
    pub samples: u32,
}

/// Renders a perspective view at the size and quality of a request.
pub type PerspectiveRenderFn<'a> =
    Box<dyn Fn(&PerspectiveRequest) -> Option<PerspectiveImage> + 'a>;

/// Reads the picture file named by a [`BoxSource::Image`] box.
pub type PictureFn<'a> = Box<dyn Fn(&str) -> Option<PerspectiveImage> + 'a>;

/// Everything a layout needs from the rest of the model when it is drawn.
pub struct LayoutRenderContext<'a> {
    pub project: &'a Project,
    /// Detected rooms per floor (index = floor). Missing floors are detected on demand.
    pub rooms_by_floor: Vec<Vec<Room>>,
    /// The 3D scene for elevations and sections; built with `plan_3d::build_scene` when `None`.
    pub scene: Option<&'a Scene>,
    pub macros: MacroContext,
    /// Draws a camera's elevation or section for [`BoxSource::Camera`] boxes.
    /// The application supplies it (it knows the camera's render options), so
    /// this crate does not depend on it. Each camera is asked once per context.
    pub camera_drawing: Option<CameraDrawingFn<'a>>,
    /// Renders a camera's perspective view for [`BoxSource::Perspective`]
    /// boxes. The application supplies it (it owns the ray tracer and caches
    /// each image per camera hash); each camera is asked once per context.
    pub perspective_image: Option<PerspectiveFn<'a>>,
    /// Like [`perspective_image`](Self::perspective_image) but told the size
    /// and sample count to render at (a box's DPI); it wins when both are set.
    pub perspective_render: Option<PerspectiveRenderFn<'a>>,
    /// Reads picture files for [`BoxSource::Image`] boxes (this crate reads no
    /// files itself); without it such a box is a framed placeholder.
    pub picture_loader: Option<PictureFn<'a>>,
    /// Prices, waste and stock lengths for [`BoxSource::Materials`] boxes.
    pub master_list: MasterList,
    camera_cache: RefCell<HashMap<plan_core::Id, Option<Rc<Drawing>>>>,
    perspective_cache: RefCell<HashMap<PerspectiveRequest, Option<Rc<PerspectiveImage>>>>,
    /// `(sheet number, title)` of every printed page, for sheet index boxes
    /// (set by [`set_sheet_index`](Self::set_sheet_index)).
    sheet_rows: RefCell<Vec<(String, String)>>,
    picture_cache: RefCell<HashMap<String, Option<Rc<PerspectiveImage>>>>,
}

/// What a perspective box asks the renderer for, or `None` for any other
/// box: the same request printing and the screen make for it.
pub fn perspective_request(b: &LayoutBox) -> Option<PerspectiveRequest> {
    match b.source {
        BoxSource::Perspective { camera_id } => Some(request_for(b, camera_id)),
        _ => None,
    }
}

/// The title block macros of `project`: its name plus Project Information
/// (client, address, designer, date, project number, revision, revisions).
pub fn macros_for(project: &Project) -> MacroContext {
    let info = &project.info;
    let pairs = info.macro_pairs();
    let get = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k.trim_matches('%') == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    MacroContext {
        project_name: project.name.clone(),
        client: get("client"),
        address: get("address"),
        designer: get("designer"),
        date: get("date"),
        project_number: get("project.number"),
        revision: get("revision"),
        revisions: info.revisions.clone(),
        // The rest of Project Information: %company%, %client.phone%,
        // %client.email%, %drawn.by%, %checked.by%, %project.address%,
        // %client.address% and %custom.<key>%.
        extra: pairs
            .iter()
            .filter(|(k, _)| !MacroContext::is_builtin(k))
            .cloned()
            .collect(),
        ..MacroContext::default()
    }
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
            macros: macros_for(project),
            camera_drawing: None,
            perspective_image: None,
            perspective_render: None,
            picture_loader: None,
            master_list: MasterList::default(),
            camera_cache: RefCell::new(HashMap::new()),
            perspective_cache: RefCell::new(HashMap::new()),
            sheet_rows: RefCell::new(Vec::new()),
            picture_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Sets [`picture_loader`](Self::picture_loader).
    pub fn with_picture_loader(
        mut self,
        f: impl Fn(&str) -> Option<PerspectiveImage> + 'a,
    ) -> Self {
        self.picture_loader = Some(Box::new(f));
        self.picture_cache.borrow_mut().clear();
        self
    }

    /// The pixels of picture file `path` from the loader (cached).
    pub(crate) fn picture_for(&self, path: &str) -> Option<Rc<PerspectiveImage>> {
        if let Some(hit) = self.picture_cache.borrow().get(path) {
            return hit.clone();
        }
        let made = self
            .picture_loader
            .as_ref()
            .and_then(|f| f(path))
            .map(Rc::new);
        self.picture_cache
            .borrow_mut()
            .insert(path.to_string(), made.clone());
        made
    }

    /// Sets [`perspective_image`](Self::perspective_image).
    pub fn with_perspective_image(
        mut self,
        f: impl Fn(plan_core::Id) -> Option<PerspectiveImage> + 'a,
    ) -> Self {
        self.perspective_image = Some(Box::new(f));
        self.perspective_cache.borrow_mut().clear();
        self
    }

    /// Sets [`master_list`](Self::master_list).
    pub fn with_master_list(mut self, master: MasterList) -> Self {
        self.master_list = master;
        self
    }

    /// Sets [`perspective_render`](Self::perspective_render).
    pub fn with_perspective_render(
        mut self,
        f: impl Fn(&PerspectiveRequest) -> Option<PerspectiveImage> + 'a,
    ) -> Self {
        self.perspective_render = Some(Box::new(f));
        self.perspective_cache.borrow_mut().clear();
        self
    }

    /// The perspective image for `req` from the hooks (cached per request):
    /// the sized hook when set, else the camera-only one.
    pub(crate) fn perspective_for(&self, req: PerspectiveRequest) -> Option<Rc<PerspectiveImage>> {
        if let Some(hit) = self.perspective_cache.borrow().get(&req) {
            return hit.clone();
        }
        let made = match (&self.perspective_render, &self.perspective_image) {
            (Some(f), _) => f(&req),
            (None, Some(f)) => f(req.camera_id),
            (None, None) => None,
        }
        .map(Rc::new);
        self.perspective_cache
            .borrow_mut()
            .insert(req, made.clone());
        made
    }

    /// Takes the sheet index (sheet number and title of every printed page)
    /// from `layout`, for [`BoxSource::SheetIndex`] boxes.
    pub fn set_sheet_index(&self, layout: &Layout) {
        *self.sheet_rows.borrow_mut() = layout
            .content_pages()
            .iter()
            .map(|p| (p.sheet_number(), p.title.to_uppercase()))
            .collect();
    }

    /// The sheet index as a table.
    pub(crate) fn sheet_index_table(&self) -> plan_docs::Schedule {
        plan_docs::Schedule {
            title: "SHEET INDEX".to_string(),
            columns: vec!["SHEET".to_string(), "TITLE".to_string()],
            rows: self
                .sheet_rows
                .borrow()
                .iter()
                .map(|(n, t)| vec![n.clone(), t.clone()])
                .collect(),
        }
    }

    /// The Materials List table of a [`BoxSource::Materials`] box.
    pub(crate) fn materials_table(
        &self,
        floor: Option<usize>,
        category: Option<&str>,
    ) -> plan_docs::Schedule {
        let scope = match floor {
            Some(f) => MaterialsScope::Floor(f),
            None => MaterialsScope::AllFloors,
        };
        let lines = materials_report(self.project, scope, None, &self.master_list);
        let lines: Vec<_> = lines
            .into_iter()
            .filter(|l| category.is_none_or(|c| l.category == c))
            .collect();
        let title = match category {
            Some(c) => format!("MATERIALS LIST - {}", c.to_uppercase()),
            None => "MATERIALS LIST".to_string(),
        };
        materials_to_schedule(&lines, &title)
    }

    /// Sets [`camera_drawing`](Self::camera_drawing).
    pub fn with_camera_drawing(
        mut self,
        f: impl Fn(plan_core::Id) -> Option<Drawing> + 'a,
    ) -> Self {
        self.camera_drawing = Some(Box::new(f));
        self.camera_cache.borrow_mut().clear();
        self
    }

    /// The drawing of camera `id` from the hook (cached; `None` without a hook
    /// or when the hook has no drawing for it).
    pub(crate) fn camera_drawing_for(&self, id: plan_core::Id) -> Option<Rc<Drawing>> {
        if let Some(hit) = self.camera_cache.borrow().get(&id) {
            return hit.clone();
        }
        let drawn = self
            .camera_drawing
            .as_ref()
            .and_then(|f| f(id))
            .map(Rc::new);
        self.camera_cache.borrow_mut().insert(id, drawn.clone());
        drawn
    }
}

// ---------------------------------------------------------------- helpers --

/// Pen weights for elevation and section lines, points (before the box's
/// `line_weight_scale`). Heavy is 0.25 mm, Medium 0.12 mm, Light 0.06 mm.
const HEAVY_PT: f64 = 0.7;
const MEDIUM_PT: f64 = 0.35;
const LIGHT_PT: f64 = 0.18;
/// Section cut lines, points.
const CUT_PT: f64 = 1.0;
/// Material hatch stroke, points.
const HATCH_PT: f64 = 0.15;
/// Hatch colour as a fraction of the material's base colour.
const HATCH_DARKEN: f32 = 0.6;

/// 1/100 mm to points.
fn hundredths_mm_to_pt(h: f64) -> f64 {
    h / 100.0 * 72.0 / 25.4
}

/// Plotted pen of a layer: its colour, its line weight in points (`line_weight`
/// is 1/100 mm) times `scale`, and its dash style. Unknown layers are 0.25 mm
/// black solid.
fn layer_pen(layers: &LayerSet, name: &str, scale: f64) -> Pen {
    let l = layers.get(name);
    Pen {
        width: (hundredths_mm_to_pt(f64::from(l.map_or(25, |l| l.line_weight))) * scale).max(0.1),
        color: l.map_or(BLACK, |l| PdfColor::Rgb(l.color[0], l.color[1], l.color[2])),
        dash: l.map_or(Dash::Solid, |l| l.line_style.into()),
    }
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
fn draw_cad_item(cv: &mut Canvas, o: &CadObject, tp: &dyn Fn(Point) -> Pt, k: f64, pen: Pen) {
    draw_cad_item_styled(cv, o, tp, k, pen, None);
}

/// The text style of a CAD object on the page: the style its attributes
/// name, else its layer's.
fn text_style_of<'a>(
    project: &'a Project,
    o: &CadObject,
    attrs: Option<&plan_core::cad::CadAttrs>,
) -> Option<&'a plan_core::TextStyle> {
    let styles = &project.text_styles;
    match attrs
        .and_then(|a| a.text_style.as_deref())
        .filter(|n| !n.is_empty())
    {
        Some(name) => styles.resolve(name),
        None => styles.resolve_for_layer(&project.layers, &o.layer),
    }
}

/// [`draw_cad_item`] with the text style of the object and the box's paper
/// scale in inches per foot: text in a printed-size style is as tall on the
/// page as the style says whatever the scale (L-15, TXT-2).
fn draw_cad_item_styled(
    cv: &mut Canvas,
    o: &CadObject,
    tp: &dyn Fn(Point) -> Pt,
    k: f64,
    pen: Pen,
    style: Option<(&plan_core::TextStyle, f64)>,
) {
    match &o.item {
        CadItem::Line { a, b } => cv.line(tp(*a), tp(*b), pen),
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
            cv.stroke(&pts, false, pen);
        }
        CadItem::Circle { center, radius } => {
            let mut pts: Vec<Pt> = arc_points(*center, *radius, 0.0, TAU)
                .into_iter()
                .map(tp)
                .collect();
            pts.pop();
            cv.stroke(&pts, true, pen);
        }
        CadItem::Polyline { points, closed } => {
            let pts: Vec<Pt> = points.iter().map(|&p| tp(p)).collect();
            cv.stroke(&pts, *closed, pen);
        }
        CadItem::Text {
            pos,
            text,
            height,
            angle,
        } => {
            let plan_h = match style {
                Some((st, ipf)) => st.text_height(*height, ipf),
                None => *height,
            };
            cv.text_full(tp(*pos), plan_h * k, pen.color, false, *angle, text)
        }
    }
}

// ------------------------------------------------------------- plan view --

/// An opening in its wall, drawn from `plan_core::opening_symbol::plan_symbol`
/// (the same geometry as the plan view): the wall is cleared across the
/// opening, then jambs, the leaf and swing arc, window lines, pocket and track
/// lines, the projecting outline of a bay, bow or box window and so on get the
/// pen their part kind calls for. `exterior` is the wall side of the outside
/// (see [`plan_core::exterior_sign`]).
fn draw_opening(
    cv: &mut Canvas,
    w: &Wall,
    o: &Opening,
    exterior: f64,
    tp: &dyn Fn(Point) -> Pt,
    k: f64,
    pen: Pen,
) {
    let sym = plan_symbol(w, o, exterior);
    let (n, half) = (w.normal(), w.thickness * 0.5);
    // The gap overshoots the faces a little so the outline strokes vanish.
    let over = half + 1.5 / k;
    let (pa, pb) = (w.point_at(o.start_offset()), w.point_at(o.end_offset()));
    let lo = if sym.cut.0 <= -half + 1e-9 {
        -over
    } else {
        sym.cut.0
    };
    let hi = if sym.cut.1 >= half - 1e-9 {
        over
    } else {
        sym.cut.1
    };
    cv.fill(
        &[pa + n * hi, pb + n * hi, pb + n * lo, pa + n * lo].map(tp),
        gray(1.0),
    );
    for part in &sym.parts {
        let pts: Vec<Pt> = part.points.iter().map(|&p| tp(p)).collect();
        let dashed = |p: Pen| Pen {
            dash: Dash::Dashed,
            ..p
        };
        let pen = match part.kind {
            PartKind::Jamb | PartKind::Frame => pen.solid(),
            PartKind::Leaf | PartKind::Arrow => pen.scaled(0.8).solid(),
            PartKind::Swing => pen.scaled(0.6),
            PartKind::Glass => pen.scaled(0.5).solid(),
            PartKind::Hidden => dashed(pen.scaled(0.5)),
            PartKind::Track => dashed(pen.scaled(0.6)),
        };
        cv.stroke(&pts, part.closed, pen);
    }
}

fn room_name(f: &Floor, room: &Room) -> String {
    room.name_entry(&f.room_names)
        .map_or_else(|| room.label.clone(), |n| n.name.clone())
}

/// The size of a dimension's number on the page, points: its text style
/// (the dimension's own, else "Dimension Text Style") at the box's paper
/// scale, `k` points per source inch. A printed-size style is the same on
/// paper at any scale; a character-height style scales with the box.
fn dimension_text_pt(project: &Project, d: &plan_core::Dimension, k: f64) -> f64 {
    let name = d
        .text_style
        .as_deref()
        .filter(|n| !n.is_empty())
        .unwrap_or("Dimension Text Style");
    match project.text_styles.resolve(name) {
        Some(st) => st.plan_height_at(k / 6.0, false) * k,
        None => 7.0,
    }
}

fn draw_dimension(
    cv: &mut Canvas,
    d: &plan_core::Dimension,
    tp: &dyn Fn(Point) -> Pt,
    pen: Pen,
    text_pt: f64,
) {
    const TICK: f64 = 3.0;
    let text_pt = text_pt.clamp(3.0, 72.0);
    for (a, b) in d.visible_extension_lines() {
        cv.line(tp(a), tp(b), pen.scaled(0.6).solid());
    }
    let (a, b) = d.line_points();
    let (pa, pb) = (tp(a), tp(b));
    cv.line(pa, pb, pen);
    for p in [pa, pb] {
        cv.line(
            (p.0 - TICK, p.1 - TICK),
            (p.0 + TICK, p.1 + TICK),
            pen.scaled(1.5).solid(),
        );
    }
    let label = d.label(&DimFormat::default());
    let mid = ((pa.0 + pb.0) * 0.5, (pa.1 + pb.1) * 0.5);
    if (pb.0 - pa.0).abs() >= (pb.1 - pa.1).abs() {
        cv.text_centered(mid.0, mid.1 + 2.0, text_pt, pen.color, false, &label);
    } else {
        // Vertical dimensions read bottom to top, centred beside the line
        // (the glyphs rise to the left of the baseline).
        let w = text_w(&label, text_pt, false);
        cv.text_full(
            (mid.0 + 2.0 + text_pt * 0.75, mid.1 - w * 0.5),
            text_pt,
            pen.color,
            false,
            std::f64::consts::FRAC_PI_2,
            &label,
        );
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
    let pen = |name: &str| layer_pen(layers, name, lws);

    // Walls: layer-coloured outline stroke first, gray fill on top (shared
    // edges vanish).
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
        cv.stroke(&poly(o), true, pen(&w.layer).scaled(2.0));
    }
    for (_, o) in &shown {
        cv.fill(&poly(o), gray(0.8));
    }

    let fallback;
    let rooms: &[Room] = match cx.rooms_by_floor.get(floor) {
        Some(r) => r,
        None => {
            fallback = detect_rooms(&f.walls, 1.0);
            &fallback
        }
    };
    for o in &f.openings {
        let layer = match o.kind {
            OpeningKind::Door => "Doors",
            OpeningKind::Window => "Windows",
        };
        let Some(w) = f.wall(o.wall_id) else { continue };
        if show(layer) && show(&w.layer) {
            let exterior = plan_core::exterior_sign(w, rooms);
            draw_opening(cv, w, o, exterior, tp, k, pen(layer));
            // Casing drawn in plan (a mulled unit shares one loop around its span).
            let unit = o
                .mull_group
                .and_then(|_| cx.project.unit_span(floor, o.id));
            for part in casing_parts(w, o, unit, exterior) {
                let pts: Vec<Pt> = part.points.iter().map(|&p| tp(p)).collect();
                cv.stroke(&pts, true, pen(layer).scaled(0.6).solid());
            }
        }
    }

    if show("Room Labels") {
        let color = pen("Room Labels").color;
        for r in rooms {
            let (x, y) = tp(r.centroid);
            cv.text_centered(x, y + 1.5, 8.0, color, true, &room_name(f, r));
            cv.text_centered(
                x,
                y - 8.0,
                6.5,
                color,
                false,
                &format!("{:.0} SF", r.area_sq_ft()),
            );
        }
    }

    for d in &f.dimensions {
        let layer = match d.kind {
            DimensionKind::Manual => "Dimensions, Manual",
            DimensionKind::AutoExterior => "Dimensions, Automatic",
            DimensionKind::Temporary => continue,
        };
        if show(layer) {
            let text_pt = dimension_text_pt(cx.project, d, k);
            draw_dimension(cv, d, tp, pen(layer), text_pt);
        }
    }

    let attrs = f.cad_attr_map();
    for o in &f.cad {
        if show(&o.layer) {
            let style = text_style_of(cx.project, o, attrs.get(&o.id)).map(|st| (st, k / 6.0));
            draw_cad_item_styled(cv, o, tp, k, pen(&o.layer), style);
        }
    }
}

// ----------------------------------------------------- elevation, tables --

/// Material hatch under the elevation lines: base colour darkened.
fn draw_hatch(
    cv: &mut Canvas,
    strokes: &[crate::hatch::HatchStroke],
    tp: &dyn Fn(Point) -> Pt,
    lws: f64,
) {
    for h in strokes {
        let c = h.material.color();
        let ch = |v: f32| (v * HATCH_DARKEN * 255.0).round().clamp(0.0, 255.0) as u8;
        let pen = Pen {
            width: HATCH_PT * lws,
            color: PdfColor::Rgb(ch(c[0]), ch(c[1]), ch(c[2])),
            dash: Dash::Solid,
        };
        cv.line(tp(h.a), tp(h.b), pen);
    }
}

/// Poche (section cut) fill, gray level.
const CUT_FILL_GRAY: f64 = 0.55;
/// Cast shadow fill, gray level.
const SHADOW_FILL_GRAY: f64 = 0.85;
/// Annotation text sizes, points.
const NOTE_PT: f64 = 7.0;
const TITLE_PT: f64 = 11.0;

/// Whether an annotation is the drawing title ("FRONT ELEVATION"...).
fn is_title_text(t: &str) -> bool {
    t.chars().any(char::is_alphabetic)
        && t == t.to_uppercase()
        && ["ELEVATION", "SECTION", "PLAN", "VIEW"]
            .iter()
            .any(|w| t.contains(w))
}

/// Paint a drawing: Cut regions solid poche gray, Shadow regions light gray,
/// both under the line strokes (Face regions stay unfilled; their hatch lines
/// are in `lines`), then the strokes, then the texts at their anchors (bold
/// for the title).
fn draw_drawing(cv: &mut Canvas, d: &Drawing, tp: &dyn Fn(Point) -> Pt, lws: f64) {
    for (kind, g) in [
        (RegionKind::Cut, CUT_FILL_GRAY),
        (RegionKind::Shadow, SHADOW_FILL_GRAY),
    ] {
        for r in d.regions.iter().filter(|r| r.kind == kind) {
            let pts: Vec<Pt> = r.polygon.iter().map(|&p| tp(p)).collect();
            cv.fill(&pts, gray(g));
        }
    }
    for l in &d.lines {
        let width = match (l.kind, l.weight) {
            (EdgeKind::Cut, _) => CUT_PT,
            (_, LineWeight::Heavy) => HEAVY_PT,
            (_, LineWeight::Medium) => MEDIUM_PT,
            (_, LineWeight::Light) => LIGHT_PT,
        } * lws;
        let mut pen = Pen::new(width);
        if l.kind == EdgeKind::Hidden {
            pen.dash = Dash::Hidden;
        }
        cv.line(tp(l.a), tp(l.b), pen);
    }
    for (at, text) in &d.texts {
        let (x, y) = tp(*at);
        if is_title_text(text) {
            cv.bold(x, y, TITLE_PT, text);
        } else {
            cv.text(x, y, NOTE_PT, BLACK, text);
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
        cv.line((x, y), (x + m.width, y), Pen::new(w));
    }
    let bottom = t0 - ROW_H_PT * n_rows as f64;
    let mut cx = x;
    for (c, cw) in m.cols.iter().enumerate() {
        cv.line((cx, t0), (cx, bottom), Pen::new(0.4));
        let hy = t0 - ROW_H_PT + 6.0;
        cv.bold(cx + 4.0, hy, TABLE_TEXT_PT, &s.columns[c]);
        for (r, row) in s.rows.iter().enumerate() {
            if let Some(cell) = row.get(c) {
                let y = t0 - ROW_H_PT * (r + 2) as f64 + 6.0;
                cv.text(cx + 4.0, y, TABLE_TEXT_PT, BLACK, cell);
            }
        }
        cx += cw;
    }
    cv.line((cx, t0), (cx, bottom), Pen::new(1.0));
    cv.line((x, t0), (x, bottom), Pen::new(1.0));
}

// ------------------------------------------------------------------ boxes --

fn box_rect_pt(b: &LayoutBox) -> Rect {
    b.bounds_in().map(|v| v * 72.0)
}

/// The prims for a box: its content, bracketed by clip markers when `clip` is
/// set (the PDF clip rectangle), then its border.
pub(crate) fn box_prims(
    b: &LayoutBox,
    cx: &LayoutRenderContext,
    scenes: &SceneSource,
    layers: &LayoutLayers,
) -> Vec<Prim> {
    let real = box_rect_pt(b);
    // A box with turned content lays it out in the box turned back: the
    // same centre, width and height swapped for a quarter turn.
    let turns = b.quarter_turns();
    let centre = ((real[0] + real[2]) * 0.5, (real[1] + real[3]) * 0.5);
    let rect = if turns % 2 == 1 {
        let (hw, hh) = ((real[3] - real[1]) * 0.5, (real[2] - real[0]) * 0.5);
        [centre.0 - hw, centre.1 - hh, centre.0 + hw, centre.1 + hh]
    } else {
        real
    };
    let mut cv = Canvas::new();
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
            let ipf = b.scale.inches_per_foot();
            match &b.source {
                BoxSource::PlanView { floor, layer_set } => {
                    draw_plan(&mut cv, cx, *floor, layer_set, &tp, k, lws);
                }
                BoxSource::Elevation { dir } => {
                    let scene = scenes.get(cx.project);
                    if b.hatch_materials {
                        draw_hatch(&mut cv, &wall_face_hatch(scene, *dir, None, ipf), &tp, lws);
                    }
                    draw_drawing(&mut cv, &elevation(scene, *dir, &opts), &tp, lws);
                }
                BoxSource::Section { cut } => {
                    let scene = scenes.get(cx.project);
                    if b.hatch_materials {
                        let h = wall_face_hatch(scene, cut.plane_normal, Some(cut.offset), ipf);
                        draw_hatch(&mut cv, &h, &tp, lws);
                    }
                    draw_drawing(&mut cv, &section(scene, *cut, &opts), &tp, lws);
                }
                BoxSource::Camera { camera_id } => {
                    if let Some(d) = cx.camera_drawing_for(*camera_id) {
                        draw_drawing(&mut cv, &d, &tp, lws);
                    }
                }
                BoxSource::CadDetail { items, .. } => {
                    let layers = &cx.project.layers;
                    for o in items {
                        let style = text_style_of(cx.project, o, None).map(|st| (st, ipf));
                        let pen = layer_pen(layers, &o.layer, lws);
                        draw_cad_item_styled(&mut cv, o, &tp, k, pen, style);
                    }
                }
                _ => {}
            }
        }
        Frame::Paper { .. } => match &b.source {
            BoxSource::Schedule { kind } => {
                draw_table(&mut cv, &schedule_for(*kind, cx), rect[0], rect[3]);
            }
            BoxSource::PlacedSchedule { floor, id } => {
                match placed_schedule_table(cx, *floor, *id) {
                    Some(t) => draw_table(&mut cv, &t, rect[0], rect[3]),
                    None => placeholder(&mut cv, rect, lws, "SCHEDULE: not found"),
                }
            }
            BoxSource::Materials { floor, category } => {
                let t = cx.materials_table(*floor, category.as_deref());
                draw_table(&mut cv, &t, rect[0], rect[3]);
            }
            BoxSource::SheetIndex => {
                draw_table(&mut cv, &cx.sheet_index_table(), rect[0], rect[3]);
            }
            BoxSource::Perspective { camera_id } => {
                match cx.perspective_for(request_for(b, *camera_id)) {
                    Some(img) => draw_pixels(&mut cv, rect, lws, &img, "PERSPECTIVE"),
                    None => placeholder(
                        &mut cv,
                        rect,
                        lws,
                        &format!("PERSPECTIVE: camera {camera_id} not rendered"),
                    ),
                }
            }
            BoxSource::Text {
                text,
                height_pt,
                align,
                bold,
            } if layers.is_visible(LAYER_TEXT) => {
                let fitted = fit_text_box(text, *height_pt, *bold, b.text_fit, bw, bh);
                let size = fitted.size_pt;
                // A line that starts below the box is dropped; one that only
                // runs past the bottom is cut by the box's clip. Without a
                // clip only lines that fit whole are drawn.
                let shown = if b.clip {
                    visible_line_count(fitted.lines.len(), size, bh)
                } else {
                    whole_line_count(fitted.lines.len(), size, bh)
                };
                let shown = if b.text_fit == TextFit::Off {
                    fitted.lines.len()
                } else {
                    shown
                };
                let color = pdf_color(layers.color(LAYER_TEXT));
                for (i, line) in fitted.lines.iter().take(shown).enumerate() {
                    let y = rect[3] - size * LINE_SPACING * (i as f64 + 0.8);
                    let w = text_w(line, size, *bold);
                    let x = match align {
                        TextAlign::Left => rect[0] + PAD_PT,
                        TextAlign::Center => (rect[0] + rect[2] - w) * 0.5,
                        TextAlign::Right => rect[2] - PAD_PT - w,
                    };
                    cv.text_full((x, y), size, color, *bold, 0.0, line);
                }
            }
            BoxSource::Text { .. } => {}
            BoxSource::Image { path } => match cx.picture_for(path) {
                Some(img) => draw_pixels(&mut cv, rect, lws, &img, "IMAGE"),
                None => placeholder(&mut cv, rect, lws, &format!("IMAGE: {path}")),
            },
            BoxSource::ImageData {
                width,
                height,
                rgba,
            } => {
                let expect = u64::from(*width) * u64::from(*height) * 4;
                if *width == 0 || *height == 0 || expect != rgba.len() as u64 {
                    placeholder(&mut cv, rect, lws, "IMAGE: invalid pixel data");
                } else {
                    let (iw, ih) = (f64::from(*width), f64::from(*height));
                    let s = (bw / iw).min(bh / ih);
                    let (dw, dh) = (iw * s, ih * s);
                    let (x, y) = (rect[0] + (bw - dw) * 0.5, rect[1] + (bh - dh) * 0.5);
                    cv.prims.push(Prim::Image {
                        rect: [x, y, x + dw, y + dh],
                        px: (*width, *height),
                        rgba: rgba.clone(),
                    });
                }
            }
            _ => {}
        },
    }

    // Turn the content, then clip it to the box and frame it.
    rotate_prims(&mut cv.prims, centre, turns);
    let mut out = Vec::with_capacity(cv.prims.len() + 8);
    if b.clip {
        out.push(Prim::ClipBegin(real));
    }
    out.append(&mut cv.prims);
    if b.clip {
        out.push(Prim::ClipEnd);
    }
    if b.border && layers.is_visible(LAYER_BOX_BORDERS) {
        let mut frame = Canvas::new();
        let pen = Pen {
            color: pdf_color(layers.color(LAYER_BOX_BORDERS)),
            ..Pen::new(layers.weight_pt(LAYER_BOX_BORDERS) * lws)
        };
        frame.rect(real[0], real[1], real[2], real[3], pen);
        out.append(&mut frame.prims);
    }
    out
}

fn pdf_color(c: [u8; 3]) -> PdfColor {
    if c == [0, 0, 0] {
        BLACK
    } else {
        PdfColor::Rgb(c[0], c[1], c[2])
    }
}

/// What a perspective box asks the renderer for: its camera at the pixel size
/// its rectangle and DPI make (a quarter-turned box is rendered upright).
pub(crate) fn request_for(b: &LayoutBox, camera_id: plan_core::Id) -> PerspectiveRequest {
    let (mut w_in, mut h_in) = b.size_in();
    if b.quarter_turns() % 2 == 1 {
        std::mem::swap(&mut w_in, &mut h_in);
    }
    let (width, height) = perspective_pixels(w_in, h_in, b.dpi);
    PerspectiveRequest {
        camera_id,
        width,
        height,
        samples: b.effective_samples(),
    }
}

/// Pixels scaled to fit `rect` and centred, or a placeholder when the data
/// does not match its size.
fn draw_pixels(cv: &mut Canvas, rect: Rect, lws: f64, img: &PerspectiveImage, what: &str) {
    let (bw, bh) = (rect[2] - rect[0], rect[3] - rect[1]);
    let expect = u64::from(img.width) * u64::from(img.height) * 4;
    if img.width == 0 || img.height == 0 || expect != img.rgba.len() as u64 {
        placeholder(cv, rect, lws, &format!("{what}: invalid image"));
        return;
    }
    let (iw, ih) = (f64::from(img.width), f64::from(img.height));
    let s = (bw / iw).min(bh / ih);
    let (dw, dh) = (iw * s, ih * s);
    let (x, y) = (rect[0] + (bw - dw) * 0.5, rect[1] + (bh - dh) * 0.5);
    cv.prims.push(Prim::Image {
        rect: [x, y, x + dw, y + dh],
        px: (img.width, img.height),
        rgba: img.rgba.clone(),
    });
}

/// A crossed frame with a caption, for images that cannot be drawn.
fn placeholder(cv: &mut Canvas, rect: Rect, lws: f64, caption: &str) {
    let (x0, y0, x1, y1) = (rect[0], rect[1], rect[2], rect[3]);
    let pen = Pen::gray(0.35 * lws, 0.6);
    cv.line((x0, y0), (x1, y1), pen);
    cv.line((x0, y1), (x1, y0), pen);
    cv.text(x0 + 4.0, y1 - 12.0, 8.0, gray(0.35), caption);
}

fn is_scaled(source: &BoxSource) -> bool {
    matches!(
        source,
        BoxSource::PlanView { .. }
            | BoxSource::Elevation { .. }
            | BoxSource::Section { .. }
            | BoxSource::Camera { .. }
            | BoxSource::CadDetail { .. }
    )
}

/// Caption and scale note under a box.
fn label_prims(b: &LayoutBox) -> Vec<Prim> {
    let Some(label) = &b.label else {
        return Vec::new();
    };
    let r = box_rect_pt(b);
    let mut cv = Canvas::new();
    cv.bold(r[0], r[1] - 13.0, 10.0, label);
    if is_scaled(&b.source) {
        cv.text(
            r[0],
            r[1] - 23.0,
            7.0,
            gray(0.35),
            &format!("SCALE: {}", b.scale.label()),
        );
    }
    cv.prims
}

/// Every stroked segment a box draws (content plus border), in paper inches.
///
/// Honors the box's `clip` (applied in software here; the PDF uses a clip
/// rectangle), scale and `line_weight_scale` exactly like [`render_pdf`]. Pen
/// widths map to [`LineWeight`] (0.7 pt and up Heavy, 0.35 pt and up Medium,
/// otherwise Light); every segment has kind [`EdgeKind::Silhouette`]. Fills,
/// text and images are not included.
pub fn render_box_lines(b: &LayoutBox, cx: &LayoutRenderContext) -> Vec<Line2> {
    let scenes = SceneSource::new(cx.scene);
    let mut out = Vec::new();
    for p in soft_clip(box_prims(b, cx, &scenes, &LayoutLayers::default())) {
        if let Prim::Stroke { pts, closed, pen } = p {
            let width = pen.width;
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

/// A line of text a box draws, in paper inches (see [`render_box_artwork`]).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxText {
    /// Baseline-left corner.
    pub x: f64,
    pub y: f64,
    pub size_pt: f64,
    pub bold: bool,
    /// Counter-clockwise radians about `(x, y)`.
    pub angle: f64,
    /// Gray level 0 (black) to 1.
    pub gray: f32,
    pub text: String,
}

/// A raster image a box draws, placed in paper inches.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxImage {
    /// `[x_min, y_min, x_max, y_max]`.
    pub rect_in: [f64; 4],
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Everything a box draws, for an on-screen view: its strokes, texts (text
/// boxes, table cells, placeholders) and images, turned and clipped exactly
/// like the printed page. Fills are not included.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoxArtwork {
    pub lines: Vec<Line2>,
    pub texts: Vec<BoxText>,
    pub images: Vec<BoxImage>,
}

/// The strokes, texts and images of a box (see [`BoxArtwork`]).
pub fn render_box_artwork(b: &LayoutBox, cx: &LayoutRenderContext) -> BoxArtwork {
    render_box_artwork_in(b, cx, &LayoutLayers::default())
}

/// [`render_box_artwork`] with the layout's own layers: a hidden layer's
/// parts (box borders, text boxes) are left out and weights follow the layers.
pub fn render_box_artwork_in(
    b: &LayoutBox,
    cx: &LayoutRenderContext,
    layers: &LayoutLayers,
) -> BoxArtwork {
    let scenes = SceneSource::new(cx.scene);
    let mut out = BoxArtwork::default();
    for p in soft_clip_with(box_prims(b, cx, &scenes, layers), true) {
        match p {
            Prim::Stroke { pts, closed, pen } => {
                let width = pen.width;
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
                    out.lines.push(Line2 {
                        a: Point::new(a.0 / 72.0, a.1 / 72.0),
                        b: Point::new(c.0 / 72.0, c.1 / 72.0),
                        weight,
                        kind: EdgeKind::Silhouette,
                    });
                }
            }
            Prim::Text {
                x,
                y,
                size,
                color,
                bold,
                angle,
                text,
            } => {
                let gray = match color {
                    PdfColor::Gray(g) => g as f32,
                    PdfColor::Rgb(r, g, b) => {
                        ((0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b))
                            / 255.0) as f32
                    }
                };
                out.texts.push(BoxText {
                    x: x / 72.0,
                    y: y / 72.0,
                    size_pt: size,
                    bold,
                    angle,
                    gray,
                    text,
                });
            }
            Prim::Image { rect, px, rgba } => out.images.push(BoxImage {
                rect_in: rect.map(|v| v / 72.0),
                width: px.0,
                height: px.1,
                rgba,
            }),
            Prim::Fill { .. } | Prim::ClipBegin(_) | Prim::ClipEnd => {}
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
    soft_clip(box_prims(b, cx, scenes, &LayoutLayers::default()))
}

// ------------------------------------------------------------ title block --

fn draw_field(cv: &mut Canvas, x: f64, y: f64, w: f64, h: f64, label: &str, value: &str) {
    cv.rect(x, y, x + w, y + h, Pen::new(0.75));
    cv.text_full((x + 3.0, y + h - 7.5), 5.5, gray(0.35), true, 0.0, label);
    let size = (h * 0.2).clamp(8.0, 14.0);
    let max_lines = (((h - 10.0) / (size * 1.2)).floor() as usize).max(1);
    for (i, line) in wrap_text(value, size, w - 8.0)
        .into_iter()
        .take(max_lines)
        .enumerate()
    {
        let base = (y + h - 10.0 - size - i as f64 * size * 1.2).max(y + 3.0);
        cv.text(x + 4.0, base, size, BLACK, &line);
    }
}

/// Height of the REVISIONS caption row, header row and each revision row, points.
const REV_TITLE_H: f64 = 13.0;
const REV_HEAD_H: f64 = 11.0;
const REV_ROW_H: f64 = 14.0;

/// The REVISIONS table of Daniel's title block: `rows` rows, the latest
/// `rows` revisions oldest first. `(x, y)` is its lower-left corner.
fn draw_revision_table(
    cv: &mut Canvas,
    x: f64,
    y: f64,
    w: f64,
    rows: usize,
    revs: &[(String, String, String)],
) {
    let h = REV_TITLE_H + REV_HEAD_H + REV_ROW_H * rows as f64;
    let top = y + h;
    let line = Pen::new(0.5);
    cv.rect(x, y, x + w, top, Pen::new(0.75));
    cv.text_full((x + 3.0, top - 9.5), 6.0, BLACK, true, 0.0, "REVISIONS");
    let (c1, c2) = (x + 26.0, x + 26.0 + 50.0);
    let head_top = top - REV_TITLE_H;
    cv.line((x, head_top), (x + w, head_top), line);
    for (cx, label) in [(x, "REV"), (c1, "DATE"), (c2, "DESCRIPTION")] {
        cv.text_full(
            (cx + 3.0, head_top - 8.0),
            5.5,
            gray(0.35),
            true,
            0.0,
            label,
        );
    }
    let rows_top = head_top - REV_HEAD_H;
    for i in 0..rows {
        let ry = rows_top - REV_ROW_H * i as f64;
        cv.line((x, ry), (x + w, ry), line);
    }
    for cx in [c1, c2] {
        cv.line((cx, head_top), (cx, y), line);
    }
    let shown = &revs[revs.len().saturating_sub(rows)..];
    for (i, (num, date, desc)) in shown.iter().enumerate() {
        let base = rows_top - REV_ROW_H * (i + 1) as f64 + 4.0;
        cv.text(x + 3.0, base, 6.5, BLACK, &fit_text(num, 6.5, c1 - x - 5.0));
        cv.text(
            c1 + 3.0,
            base,
            6.5,
            BLACK,
            &fit_text(date, 6.5, c2 - c1 - 5.0),
        );
        cv.text(
            c2 + 3.0,
            base,
            6.5,
            BLACK,
            &fit_text(desc, 6.5, x + w - c2 - 5.0),
        );
    }
}

fn draw_title_block(cv: &mut Canvas, layout: &Layout, ctx: &MacroContext) {
    let (w_in, h_in) = layout.sheet_inches();
    let (sw, sh, m) = (w_in * 72.0, h_in * 72.0, layout.margins_in * 72.0);
    // Layout Edge: the page border at its own (thin) line weight.
    let edge = hundredths_mm_to_pt(f64::from(layout.edge_line_weight)).max(0.1);
    cv.rect(m, m, sw - m, sh - m, Pen::new(edge));
    let fields = layout.title_block.expand_macros(ctx);
    match &layout.title_block.style {
        TitleBlockStyle::RightStrip => {
            let w = layout.right_strip_in() * 72.0;
            let x0 = sw - m - w;
            cv.line((x0, m), (x0, sh - m), Pen::new(1.0));
            let rows = layout.title_block.revision_rows;
            let rev_h = if rows > 0 {
                REV_TITLE_H + REV_HEAD_H + REV_ROW_H * rows as f64
            } else {
                0.0
            };
            // Taller boxes for values that wrap; the strip is always filled.
            let weights: Vec<f64> = fields
                .iter()
                .map(|(_, v)| 0.6 + 0.4 * wrap_text(v, 11.0, w - 8.0).len().max(1) as f64)
                .collect();
            let total: f64 = weights.iter().sum();
            let avail = (sh - 2.0 * m - rev_h).max(0.0);
            let mut top = sh - m;
            for ((label, value), wt) in fields.iter().zip(&weights) {
                let h = avail * wt / total;
                draw_field(cv, x0, top - h, w, h, label, value);
                top -= h;
            }
            if rows > 0 {
                draw_revision_table(cv, x0, m, w, rows, &ctx.revisions);
            }
        }
        TitleBlockStyle::BottomStrip => {
            let h = BOTTOM_STRIP_IN * 72.0;
            cv.line((m, m + h), (sw - m, m + h), Pen::new(1.0));
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
                draw_cad_item(cv, &o, &tp, 72.0, Pen::new(0.75));
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
    labels.sort_unstable();
    labels.dedup();
    match labels.as_slice() {
        [] => "NTS".to_string(),
        [one] => (*one).to_string(),
        _ => "AS NOTED".to_string(),
    }
}

fn draw_sheet_index(cv: &mut Canvas, layout: &Layout, ctx: &MacroContext) {
    let (lo, hi) = layout.drawing_area();
    let x = lo.x * 72.0 + 36.0;
    let mut y = (hi.y * 72.0 - 4.5 * 72.0).max(lo.y * 72.0 + 40.0);
    cv.bold(x, y, 14.0, "SHEET INDEX");
    let sub = ctx.expand("%project.name%");
    if !sub.is_empty() {
        cv.text(
            x + 4.0 * 72.0 - PdfDoc::text_width(&sub, 8.0),
            y + 1.0,
            8.0,
            gray(0.35),
            &sub,
        );
    }
    y -= 8.0;
    cv.line((x, y), (x + 4.0 * 72.0, y), Pen::new(0.75));
    for p in layout.content_pages() {
        y -= 16.0;
        cv.text(x, y, 10.0, BLACK, &p.sheet_number());
        cv.text(x + 54.0, y, 10.0, BLACK, &p.title.to_uppercase());
    }
}

// -------------------------------------------------------------------- PDF --

/// Add a page's (or template page's) boxes and CAD to `cv`; CAD text may use macros.
fn draw_page_content(
    cv: &mut Canvas,
    page: &LayoutPage,
    ctx: &MacroContext,
    cx: &LayoutRenderContext,
    scenes: &SceneSource,
    layers: &LayoutLayers,
) {
    for b in &page.boxes {
        cv.prims.extend(box_prims(b, cx, scenes, layers));
        cv.prims.extend(label_prims(b));
    }
    draw_annotations(cv, page, ctx, layers);
}

/// The pen of a layout layer on the page.
fn layer_page_pen(layers: &LayoutLayers, name: &str) -> Pen {
    Pen {
        color: pdf_color(layers.color(name)),
        ..Pen::new(layers.weight_pt(name))
    }
}

/// A page's CAD, leaders and revision clouds, each on its layout layer
/// (hidden layers are skipped; text may use macros).
fn draw_annotations(cv: &mut Canvas, page: &LayoutPage, ctx: &MacroContext, layers: &LayoutLayers) {
    let tp = |p: Point| (p.x * 72.0, p.y * 72.0);
    for o in &page.cad {
        let layer = LayoutLayers::layer_of(o);
        if !layers.is_visible(layer) {
            continue;
        }
        let mut o = o.clone();
        if let CadItem::Text { text, .. } = &mut o.item {
            *text = ctx.expand(text);
        }
        draw_cad_item(cv, &o, &tp, 72.0, layer_page_pen(layers, layer));
    }
    for l in &page.leaders {
        draw_leader(cv, l, ctx, layers);
    }
    for c in &page.clouds {
        draw_cloud(cv, c, layers);
    }
}

/// A leader: the line with its landing, a filled arrowhead, and its text.
fn draw_leader(cv: &mut Canvas, l: &PageLeader, ctx: &MacroContext, layers: &LayoutLayers) {
    let tp = |p: Point| (p.x * 72.0, p.y * 72.0);
    if layers.is_visible(crate::layers::LAYER_CAD) {
        let pen = layer_page_pen(layers, crate::layers::LAYER_CAD);
        for line in l.polylines() {
            let pts: Vec<Pt> = line.iter().map(|&p| tp(p)).collect();
            cv.stroke(&pts, false, pen);
        }
        if let Some(tri) = l.arrowhead() {
            let pts: Vec<Pt> = tri.iter().map(|&p| tp(p)).collect();
            cv.fill(&pts, pen.color);
        }
    }
    if layers.is_visible(LAYER_TEXT) {
        let color = pdf_color(layers.color(LAYER_TEXT));
        let pos = l.text_pos();
        for (i, line) in ctx.expand(&l.text).lines().enumerate() {
            let at = Point::new(pos.x, pos.y - l.height_in * 1.2 * i as f64);
            cv.text_full(tp(at), l.height_in * 72.0, color, false, 0.0, line);
        }
    }
}

/// A revision cloud: the scalloped outline and the revision tag.
fn draw_cloud(cv: &mut Canvas, c: &RevisionCloud, layers: &LayoutLayers) {
    if !layers.is_visible(LAYER_REVISION_CLOUDS) {
        return;
    }
    let tp = |p: Point| (p.x * 72.0, p.y * 72.0);
    let pen = layer_page_pen(layers, LAYER_REVISION_CLOUDS);
    let pts: Vec<Pt> = c.outline().iter().map(|&p| tp(p)).collect();
    cv.stroke(&pts, true, pen);
    if let Some((tri, mid)) = c.tag() {
        let pts: Vec<Pt> = tri.iter().map(|&p| tp(p)).collect();
        cv.stroke(&pts, true, pen.scaled(0.7));
        let size = 7.0;
        let (mx, my) = tp(mid);
        cv.text_centered(mx, my - size * 0.3, size, pen.color, true, &c.revision);
    }
}

pub(crate) fn draw_page(
    cv: &mut Canvas,
    layout: &Layout,
    pages: &[&LayoutPage],
    index: usize,
    cx: &LayoutRenderContext,
    scenes: &SceneSource,
) {
    let page = pages[index];
    let mut ctx = cx.macros.clone();
    if ctx.project_name.is_empty() {
        ctx.project_name = cx.project.name.clone();
    }
    ctx.sheet_number = page.sheet_number();
    ctx.sheet_title = page.title.clone();
    ctx.scale = page_scale_label(page);
    ctx.page_count = pages.len();

    cx.set_sheet_index(layout);
    for t in layout.template_pages() {
        draw_page_content(cv, t, &ctx, cx, scenes, &layout.layers);
    }
    draw_page_content(cv, page, &ctx, cx, scenes, &layout.layers);
    if layout.layers.is_visible(LAYER_TITLE_BLOCK) {
        let mut block = Canvas::new();
        draw_title_block(&mut block, layout, &ctx);
        let k = layout.layers.weight_pt(LAYER_TITLE_BLOCK) / TITLE_BLOCK_BASE_PT;
        if (k - 1.0).abs() > 1e-9 {
            for p in &mut block.prims {
                if let Prim::Stroke { pen, .. } = p {
                    pen.width *= k;
                }
            }
        }
        cv.prims.append(&mut block.prims);
    }

    let (w_in, _) = layout.sheet_inches();
    let m = layout.margins_in * 72.0;
    if layout.layers.is_visible(LAYER_TITLE_BLOCK) {
        let text = format!("SHEET {} OF {}", index + 1, pages.len());
        cv.text_right(w_in * 72.0 - m, m * 0.4, 8.0, BLACK, &text);
    }
    if index == 0 && layout.sheet_index {
        draw_sheet_index(cv, layout, &ctx);
    }
}

/// Print the layout: one PDF page per printed layout page (template pages are
/// not printed), sized to `layout.sheet`.
///
/// Each page starts with the sheet background fill when `layout.page_background`
/// is set, then the template pages' boxes and CAD, its own boxes (clipped by
/// PDF clip rectangles, drawn with layer colours, weights and dashes), page
/// CAD, the page border at the Layout Edge weight, the title block with macros
/// expanded for that sheet, `SHEET n OF m`, and, on the first page when
/// `layout.sheet_index` is set, the sheet index. An empty layout produces one
/// blank page. Elevations and sections use `cx.scene`, or a scene built from
/// the project (once) when it is `None`.
pub fn render_pdf(layout: &Layout, cx: &LayoutRenderContext) -> Vec<u8> {
    let (w_in, h_in) = layout.sheet_inches();
    let mut doc = PdfDoc::new(w_in * 72.0, h_in * 72.0);
    let scenes = SceneSource::new(cx.scene);
    let bg = layout.page_background.then_some(PdfColor::Rgb(
        CHIEF_SHEET_BACKGROUND.0,
        CHIEF_SHEET_BACKGROUND.1,
        CHIEF_SHEET_BACKGROUND.2,
    ));
    let pages = layout.content_pages();
    if pages.is_empty() {
        if let Some(bg) = bg {
            doc.fill_page(bg);
        }
        return doc.finish();
    }
    for index in 0..pages.len() {
        if index > 0 {
            doc.new_page();
        }
        if let Some(bg) = bg {
            doc.fill_page(bg);
        }
        doc.add_bookmark(&format!(
            "{} {}",
            pages[index].sheet_number(),
            pages[index].title
        ));
        let mut cv = Canvas::new();
        draw_page(&mut cv, layout, &pages, index, cx, &scenes);
        emit(&mut doc, &cv.prims);
    }
    doc.finish()
}

#[cfg(test)]
mod region_tests {
    use super::*;
    use plan_3d::Material;
    use plan_elevation::Region;

    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        vec![
            Point::new(x, y),
            Point::new(x + s, y),
            Point::new(x + s, y + s),
            Point::new(x, y + s),
        ]
    }

    fn region(kind: RegionKind, poly: Vec<Point>) -> Region {
        Region {
            polygon: poly,
            material: Material::WallExterior,
            object_id: None,
            kind,
        }
    }

    #[test]
    fn cut_and_shadow_regions_fill_under_the_lines_and_texts_follow() {
        let mut d = Drawing::new(vec![Line2 {
            a: Point::new(0.0, 0.0),
            b: Point::new(10.0, 0.0),
            weight: LineWeight::Heavy,
            kind: EdgeKind::Silhouette,
        }]);
        d.regions = vec![
            region(RegionKind::Face, square(0.0, 0.0, 4.0)),
            region(RegionKind::Cut, square(0.0, 0.0, 5.0)),
            region(RegionKind::Shadow, square(6.0, 0.0, 2.0)),
        ];
        d.texts = vec![
            (Point::new(1.0, 1.0), "T.O. PLATE".into()),
            (Point::new(2.0, -3.0), "FRONT ELEVATION".into()),
        ];
        let mut cv = Canvas::new();
        draw_drawing(&mut cv, &d, &|p| (p.x, p.y), 1.0);
        let fills: Vec<(usize, PdfColor)> = cv
            .prims
            .iter()
            .enumerate()
            .filter_map(|(i, p)| match p {
                Prim::Fill { color, .. } => Some((i, *color)),
                _ => None,
            })
            .collect();
        assert_eq!(fills.len(), 2, "Face regions are not filled");
        assert_eq!(fills[0].1, PdfColor::Gray(0.55));
        assert_eq!(fills[1].1, PdfColor::Gray(0.85));
        let first_stroke = cv
            .prims
            .iter()
            .position(|p| matches!(p, Prim::Stroke { .. }))
            .expect("the line is stroked");
        assert!(fills.iter().all(|(i, _)| *i < first_stroke));
        let texts: Vec<(&str, bool, (f64, f64))> = cv
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text {
                    text, bold, x, y, ..
                } => Some((text.as_str(), *bold, (*x, *y))),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            vec![
                ("T.O. PLATE", false, (1.0, 1.0)),
                ("FRONT ELEVATION", true, (2.0, -3.0))
            ]
        );
    }
}

#[cfg(test)]
mod room_name_tests {
    use super::*;
    use plan_core::{detect_rooms, RoomName, WallKind};

    #[test]
    fn an_island_keeps_its_own_name() {
        let mut p = Project::new("island");
        let ring = |p: &mut Project, x0: f64, y0: f64, x1: f64, y1: f64, kind| {
            let c = [
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ];
            for i in 0..4 {
                p.add_wall(0, c[i], c[(i + 1) % 4], 4.5, 96.0, kind);
            }
        };
        ring(&mut p, 0.0, 0.0, 480.0, 360.0, WallKind::Exterior);
        ring(&mut p, 200.0, 150.0, 280.0, 210.0, WallKind::Interior);
        p.floors[0]
            .room_names
            .push(RoomName::new(Point::new(240.0, 180.0), "Pantry", "Pantry"));
        p.floors[0].room_names.push(RoomName::new(
            Point::new(60.0, 60.0),
            "Great Room",
            "Family",
        ));
        let f = &p.floors[0];
        let rooms = detect_rooms(&f.walls, 0.5);
        let mut names: Vec<String> = rooms.iter().map(|r| room_name(f, r)).collect();
        names.sort();
        assert_eq!(names, ["Great Room", "Pantry"]);
    }
}
