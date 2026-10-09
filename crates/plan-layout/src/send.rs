//! Send to Layout and the default construction set.

use crate::boxops::{fit_check, largest_scale_that_fits, scale_for_ipf, FitCheck, SendOptions};
use crate::boxview::{CameraLink, ScaleMode};
use crate::extent::{
    frame_for, size_of, source_size_in, view_frame_in, view_size_in, Frame, SceneSource,
};
use crate::model::{BoxSource, Layout, LayoutBox, ScheduleKind, LABEL_GAP_IN};
use crate::render::LayoutRenderContext;
use crate::titleblock::TitleBlockTemplate;
use plan_3d::{build_scene_with, Scene, SceneOptions};
use plan_core::{CadItem, CadObject, Id, Point, Project};
use plan_docs::{Scale, SheetSize};
use plan_elevation::{SectionCut, ViewDir};

/// Space kept between packed boxes, paper inches.
const GUTTER_IN: f64 = 0.25;

/// A box footprint: the rectangle plus the label strip below it.
#[derive(Clone, Copy)]
struct Foot {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Foot {
    fn of(b: &LayoutBox) -> Foot {
        let [x0, y0, x1, y1] = b.bounds_in();
        Foot {
            x0,
            y0: y0 - if b.label.is_some() { LABEL_GAP_IN } else { 0.0 },
            x1,
            y1,
        }
    }

    fn overlaps(&self, o: &Foot) -> bool {
        const EPS: f64 = 1e-9;
        self.x0 < o.x1 - EPS && o.x0 < self.x1 - EPS && self.y0 < o.y1 - EPS && o.y0 < self.y1 - EPS
    }
}

/// First free position for a `w` x `h` footprint: candidates are the area's
/// top-left corner plus the right edge and bottom edge of every placed box,
/// tried top to bottom then left to right. Returns the footprint's top-left
/// corner. If nothing fits inside the area the box goes below everything
/// already placed (overflowing the sheet) so boxes never overlap.
fn pack(area: (Point, Point), placed: &[Foot], w: f64, h: f64) -> Point {
    let mut xs = vec![area.0.x];
    let mut ys = vec![area.1.y];
    for p in placed {
        xs.push(p.x1 + GUTTER_IN);
        ys.push(p.y0 - GUTTER_IN);
    }
    ys.sort_by(|a, b| b.total_cmp(a));
    xs.sort_by(f64::total_cmp);
    let free = |x: f64, y: f64| {
        let f = Foot {
            x0: x,
            y0: y - h,
            x1: x + w,
            y1: y,
        };
        !placed.iter().any(|p| f.overlaps(p))
    };
    let fits = |x: f64, y: f64| x + w <= area.1.x + 1e-9 && y - h >= area.0.y - 1e-9 && free(x, y);
    for &y in &ys {
        for &x in &xs {
            if fits(x, y) {
                return Point::new(x, y);
            }
        }
    }
    let lowest = placed
        .iter()
        .map(|p| p.y0)
        .fold(area.1.y + GUTTER_IN, f64::min);
    Point::new(area.0.x, lowest - GUTTER_IN)
}

/// `1st Floor` as `FIRST FLOOR`: the leading ordinal number spelled out
/// (first to tenth). Other names are returned upper-cased as they are.
fn spell_ordinal(name: &str) -> String {
    const WORDS: [&str; 10] = [
        "FIRST", "SECOND", "THIRD", "FOURTH", "FIFTH", "SIXTH", "SEVENTH", "EIGHTH", "NINTH",
        "TENTH",
    ];
    let upper = name.trim().to_uppercase();
    let digits: String = upper.chars().take_while(char::is_ascii_digit).collect();
    let rest = &upper[digits.len()..];
    let suffix = rest.get(..2).unwrap_or("");
    match digits.parse::<usize>() {
        Ok(n) if (1..=WORDS.len()).contains(&n) && matches!(suffix, "ST" | "ND" | "RD" | "TH") => {
            format!("{}{}", WORDS[n - 1], &rest[2..])
        }
        _ => upper,
    }
}

/// The caption of a plan box. Single-floor projects keep the floor's own name
/// (`1ST FLOOR PLAN`); multi-floor projects spell the ordinal out like Chief's
/// layout box labels (`FIRST FLOOR PLAN`, `SECOND FLOOR PLAN`).
pub fn plan_label(project: &Project, floor: usize) -> String {
    let name = project.floors.get(floor).map_or("", |f| f.name.as_str());
    let name = if project.floors.len() > 1 {
        spell_ordinal(name)
    } else {
        name.to_uppercase()
    };
    format!("{name} PLAN").trim().to_string()
}

pub(crate) fn default_label(source: &BoxSource, project: &Project) -> Option<String> {
    match source {
        BoxSource::PlanView { floor, .. } => Some(plan_label(project, *floor)),
        BoxSource::Elevation { dir } => Some(
            match dir {
                ViewDir::Front => "FRONT ELEVATION",
                ViewDir::Back => "BACK ELEVATION",
                ViewDir::Left => "LEFT ELEVATION",
                ViewDir::Right => "RIGHT ELEVATION",
                ViewDir::Top => "ROOF PLAN",
            }
            .to_string(),
        ),
        BoxSource::Section { .. } => Some("SECTION".to_string()),
        BoxSource::Camera { camera_id } => Some(project.camera(*camera_id).map_or_else(
            || "CAMERA VIEW".to_string(),
            |c| {
                let name = c.name.to_uppercase();
                // The caption carries the view number of the camera's plan
                // callout ("1 - SOUTH ELEVATION"), so the sheet and the plan
                // agree on which view is which.
                match project.callout_number(c.id) {
                    Some(n) => format!("{n} - {name}"),
                    None => name,
                }
            },
        )),
        BoxSource::CadDetail { name, .. } => Some(name.to_uppercase()),
        BoxSource::Schedule { .. }
        | BoxSource::PlacedSchedule { .. }
        | BoxSource::Image { .. }
        | BoxSource::ImageData { .. }
        | BoxSource::Text { .. }
        | BoxSource::Materials { .. }
        | BoxSource::PageTable
        | BoxSource::RevisionTable => None,
        BoxSource::SheetIndex => Some("SHEET INDEX".to_string()),
        BoxSource::Perspective { camera_id } => Some(
            project
                .camera(*camera_id)
                .map_or_else(|| "PERSPECTIVE".to_string(), |c| c.name.to_uppercase()),
        ),
    }
}

/// Place a new box for `source` on sheet number `page` and return its id.
///
/// The box is sized to the source at `scale` (see [`source_size_in`]). With
/// `at` it sits with its lower-left corner there; otherwise it goes in the
/// first free area of the page's drawing area (shelf packing, left to right
/// then top to bottom, leaving room for the caption below). A missing page is
/// created. Plan, elevation, section and detail boxes get a default caption.
///
/// `cx` supplies the project (and optionally the scene) needed to measure the
/// source.
pub fn send_to_layout(
    layout: &mut Layout,
    cx: &LayoutRenderContext,
    page: u32,
    source: BoxSource,
    scale: Scale,
    at: Option<Point>,
) -> Id {
    send_to_layout_sized(layout, cx, page, source, scale, at, None)
}

/// [`send_to_layout`] with a box size of the caller's: `size_in` is the
/// `(width, height)` in paper inches the box takes instead of the one the
/// source measures (a picture of the 3D view at the width it was asked for).
pub fn send_to_layout_sized(
    layout: &mut Layout,
    cx: &LayoutRenderContext,
    page: u32,
    source: BoxSource,
    scale: Scale,
    at: Option<Point>,
    size_in: Option<(f64, f64)>,
) -> Id {
    // A sheet index box is measured against the pages this layout has.
    cx.set_sheet_index(layout);
    let (w, h) = size_in.unwrap_or_else(|| source_size_in(&source, scale, cx));
    let label = default_label(&source, cx.project);
    let id = layout.next_box_id();
    if layout.page(page).is_none() {
        layout.add_page(page, format!("Sheet A-{page}"));
    }
    let lower_left = match at {
        Some(p) => p,
        None => {
            // A page with a sheet of its own packs into that sheet's area.
            let area = layout
                .page(page)
                .map_or_else(|| layout.drawing_area(), |p| layout.page_drawing_area(p));
            let placed: Vec<Foot> = layout
                .page(page)
                .map(|p| p.boxes.iter().map(Foot::of).collect())
                .unwrap_or_default();
            let gap = if label.is_some() { LABEL_GAP_IN } else { 0.0 };
            let top_left = pack(area, &placed, w, h + gap);
            Point::new(top_left.x, top_left.y - h)
        }
    };
    let mut b = LayoutBox::new(
        id,
        (lower_left, Point::new(lower_left.x + w, lower_left.y + h)),
        source,
        scale,
    );
    b.label = label;
    if let Some(p) = layout.page_mut(page) {
        p.boxes.push(b);
    }
    id
}

/// The scale at or below `max` (in [`Scale::ALL`] order, largest drawing
/// first) at which `source` fits the layout's drawing area, leaving room for
/// the caption below; the smallest architectural scale (1" = 20') if none
/// does. Schedules, text and images do not scale, so they get `max`.
///
/// Pass [`Scale::ThreeInch`] for the pure "largest scale that fits".
pub fn fit_largest_scale(
    layout: &Layout,
    cx: &LayoutRenderContext,
    source: &BoxSource,
    max: Scale,
) -> Scale {
    let (lo, hi) = layout.drawing_area();
    let scenes = SceneSource::for_context(cx);
    let frame = frame_for(source, cx, &scenes);
    if matches!(frame, Frame::Paper { .. }) {
        return max;
    }
    let mut s = max;
    loop {
        let (w, h) = size_of(frame, s);
        let fits = w <= hi.x - lo.x + 1e-9 && h + LABEL_GAP_IN <= hi.y - lo.y + 1e-9;
        match (fits, s.smaller_any()) {
            (true, _) | (false, None) => return s,
            (false, Some(next)) => s = next,
        }
    }
}

/// How Send to Layout scales the view (the Scaling panel of the dialog).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SendScale {
    /// The largest scale of the lists, no larger than this one, at which the
    /// view fits the page (Largest that fits).
    Largest(Scale),
    /// This scale (a site scale such as 1 in = 100 ft too).
    Named(Scale),
    /// A typed scale: paper inches per foot of the building.
    PerFoot(f64),
    /// Fit to Sheet (No Scale): about half the size of the page's drawing
    /// area; the box can be resized afterwards.
    FitToSheet,
}

/// One view to send.
#[derive(Debug, Clone, PartialEq)]
pub struct SendRequest {
    pub page: u32,
    pub source: BoxSource,
    pub scale: SendScale,
    /// Lower left corner of the box; otherwise `centre`, else the first free
    /// area of the page.
    pub at: Option<Point>,
    /// The paper point the box is centred on.
    pub centre: Option<Point>,
    pub options: SendOptions,
}

/// What a send made.
#[derive(Debug, Clone, PartialEq)]
pub struct Sent {
    pub id: Id,
    /// The box, paper inches.
    pub size_in: (f64, f64),
    /// The scale as asked, paper inches per foot.
    pub ipf: f64,
    /// The view is too big for the page's drawing area at this scale (the
    /// "too big for the sheet" warning). The box is placed anyway.
    pub too_large: bool,
    /// The box against the drawing area it went into.
    pub fit: FitCheck,
}

impl Sent {
    /// The warning text for a view that does not fit, else `None`.
    pub fn warning(&self) -> Option<String> {
        self.too_large.then(|| fit_warning(&self.fit))
    }
}

/// The "view is too big for the sheet" warning of a failed [`FitCheck`].
pub fn fit_warning(fit: &FitCheck) -> String {
    format!(
        "The view is too big for the sheet at this scale: {:.1} x {:.1} in on a drawing area of {:.1} x {:.1} in",
        fit.size_in.0, fit.size_in.1, fit.area_in.0, fit.area_in.1
    )
}

/// Checks a view against a page before it is sent: the box it would make at
/// `scale`, and whether the page's drawing area holds it. `labelled` leaves
/// room for the caption.
pub fn check_send(layout: &Layout, cx: &LayoutRenderContext, req: &SendRequest) -> (FitCheck, f64) {
    let (tb, ipf) = trial_box(layout, cx, req);
    let size = view_size_in(&tb.source, &tb.view, ipf, cx);
    let size = if matches!(tb.source, BoxSource::Perspective { .. }) {
        source_size_in(&tb.source, Scale::QuarterInch, cx)
    } else {
        size
    };
    (fit_check(layout, Some(req.page), size, true), ipf)
}

/// A box with the view's options applied (picture made when the camera link
/// keeps one) and the scale the request asks for, in paper inches per foot.
fn trial_box(layout: &Layout, cx: &LayoutRenderContext, req: &SendRequest) -> (LayoutBox, f64) {
    let mut tb = LayoutBox::new(
        0,
        (Point::ZERO, Point::ZERO),
        req.source.clone(),
        Scale::QuarterInch,
    );
    req.options.apply_to(&mut tb);
    if tb.has_camera_options() && tb.view.camera != CameraLink::Always {
        tb.view.art = crate::render::make_art(&tb, cx);
    }
    let frame = view_frame_in(&tb.source, &tb.view, cx);
    let labelled = true;
    let area = layout
        .page(req.page)
        .map_or_else(|| layout.drawing_area(), |p| layout.page_drawing_area(p));
    let (aw, ah) = (
        area.1.x - area.0.x,
        area.1.y - area.0.y - if labelled { LABEL_GAP_IN } else { 0.0 },
    );
    let ipf = match (req.scale, frame) {
        (SendScale::Named(s), _) | (SendScale::Largest(s), None) => s.inches_per_foot(),
        (SendScale::PerFoot(v), _) => v,
        (SendScale::Largest(max), Some(f)) => {
            largest_scale_that_fits(layout, Some(req.page), f, max, labelled).inches_per_foot()
        }
        (SendScale::FitToSheet, Some((fw, fh))) => {
            12.0 * (aw * 0.5 / fw.max(1e-6)).min(ah * 0.5 / fh.max(1e-6))
        }
        (SendScale::FitToSheet, None) => Scale::QuarterInch.inches_per_foot(),
    };
    (tb, ipf)
}

/// Sends a view to the layout with the dialog's answers: its scale (any of
/// the lists, a typed one, or Fit to Sheet), the Send Options and the Camera
/// View Options (a Plot Lines or Update on Demand view keeps its picture
/// from now on). The box is placed even when it is too big for the sheet;
/// [`Sent::too_large`] says so.
pub fn send_view(layout: &mut Layout, cx: &LayoutRenderContext, req: &SendRequest) -> Sent {
    cx.set_sheet_index(layout);
    let (mut tb, ipf) = trial_box(layout, cx, req);
    let size = if matches!(tb.source, BoxSource::Perspective { .. }) {
        source_size_in(&tb.source, Scale::QuarterInch, cx)
    } else {
        view_size_in(&tb.source, &tb.view, ipf, cx)
    };
    let label = default_label(&req.source, cx.project);
    let id = layout.next_box_id();
    if layout.page(req.page).is_none() {
        layout.add_page(req.page, format!("Sheet A-{}", req.page));
    }
    let check = fit_check(layout, Some(req.page), size, label.is_some());
    let (w, h) = size;
    let lower_left = place(layout, req, w, h, label.is_some());
    tb.id = id;
    tb.rect_in = (lower_left, Point::new(lower_left.x + w, lower_left.y + h));
    tb.label = label;
    if matches!(req.scale, SendScale::FitToSheet) {
        tb.scale = nominal_for(ipf);
        tb.view.scale_mode = ScaleMode::NoScale(ipf);
    } else {
        let (s, mode) = scale_for_ipf(ipf);
        tb.scale = s;
        tb.view.scale_mode = mode;
    }
    // The hatch of a kept picture is made for the scale the box ends up at.
    if tb.has_camera_options() && tb.view.camera != CameraLink::Always {
        tb.view.art = crate::render::make_art(&tb, cx).or(tb.view.art.take());
    }
    if let Some(p) = layout.page_mut(req.page) {
        p.boxes.push(tb);
    }
    Sent {
        id,
        size_in: size,
        ipf,
        too_large: check.too_large,
        fit: check,
    }
}

/// The lower left corner of a new `w` x `h` box on `req.page`: where the
/// request says, else the first free area (leaving room for a caption).
fn place(layout: &Layout, req: &SendRequest, w: f64, h: f64, labelled: bool) -> Point {
    match (req.at, req.centre) {
        (Some(p), _) => p,
        (None, Some(c)) => Point::new(c.x - w / 2.0, c.y - h / 2.0),
        (None, None) => {
            let area = layout
                .page(req.page)
                .map_or_else(|| layout.drawing_area(), |p| layout.page_drawing_area(p));
            let placed: Vec<Foot> = layout
                .page(req.page)
                .map(|p| p.boxes.iter().map(Foot::of).collect())
                .unwrap_or_default();
            let gap = if labelled { LABEL_GAP_IN } else { 0.0 };
            let top_left = pack(area, &placed, w, h + gap);
            Point::new(top_left.x, top_left.y - h)
        }
    }
}

/// Current Screen As Image: the view drawn once as a picture of the page's
/// box size and embedded (150 dots per inch), a static box that is never
/// updated, only replaced.
pub fn send_as_image(layout: &mut Layout, cx: &LayoutRenderContext, req: &SendRequest) -> Sent {
    cx.set_sheet_index(layout);
    let (tb, ipf) = trial_box(layout, cx, req);
    let size = view_size_in(&tb.source, &tb.view, ipf, cx);
    let label = default_label(&req.source, cx.project);
    let id = layout.next_box_id();
    if layout.page(req.page).is_none() {
        layout.add_page(req.page, format!("Sheet A-{}", req.page));
    }
    let check = fit_check(layout, Some(req.page), size, label.is_some());
    let (w, h) = size;
    let lower_left = place(layout, req, w, h, label.is_some());
    // Draw the view alone on a box of its own size.
    let mut draw = tb;
    draw.rect_in = (Point::ZERO, Point::new(w, h));
    let (sc, mode) = scale_for_ipf(ipf);
    draw.scale = sc;
    draw.view.scale_mode = mode;
    draw.border = false;
    draw.clip = false;
    let lines: Vec<_> = crate::render::render_box_lines(&draw, cx)
        .into_iter()
        .map(|l| {
            let px = match l.weight {
                plan_elevation::LineWeight::Heavy => 2.0,
                _ => 1.0,
            };
            ((l.a.x, l.a.y), (l.b.x, l.b.y), px)
        })
        .collect();
    let width_px = ((w * 150.0).round() as u32).clamp(64, 4000);
    let (pw, ph, rgba) = crate::print::rasterize_lines(&lines, (0.0, 0.0, w, h), width_px);
    let mut b = LayoutBox::new(
        id,
        (lower_left, Point::new(lower_left.x + w, lower_left.y + h)),
        BoxSource::ImageData {
            width: pw,
            height: ph,
            rgba,
        },
        Scale::QuarterInch,
    );
    b.label = label;
    if let Some(p) = layout.page_mut(req.page) {
        p.boxes.push(b);
    }
    Sent {
        id,
        size_in: size,
        ipf,
        too_large: check.too_large,
        fit: check,
    }
}

fn nominal_for(ipf: f64) -> Scale {
    crate::boxview::nominal_scale(ipf)
}

/// The scale [`send_to_layout_auto`] starts from: construction drawings are
/// not printed larger than 1/4" = 1'-0" unless asked for.
pub const AUTO_SCALE_CEILING: Scale = Scale::QuarterInch;

/// [`send_to_layout`] with an optional scale. With `Some(scale)` it behaves
/// exactly like [`send_to_layout`]; with `None` it picks the largest scale of
/// [`Scale::ALL`], no larger than [`AUTO_SCALE_CEILING`] (1/4"), at which the
/// source fits the page's drawing area (see [`fit_largest_scale`]). A 40' x 30'
/// plan lands at 1/4" on an Arch D sheet and 1/8" on Letter.
pub fn send_to_layout_auto(
    layout: &mut Layout,
    cx: &LayoutRenderContext,
    page: u32,
    source: BoxSource,
    scale: Option<Scale>,
    at: Option<Point>,
) -> Id {
    let scale = scale.unwrap_or_else(|| fit_largest_scale(layout, cx, &source, AUTO_SCALE_CEILING));
    send_to_layout(layout, cx, page, source, scale, at)
}

/// Adds a "Materials List" page numbered `number` with one table box per
/// category that has rows (Foundation, Framing, Roofing, ...) priced from the
/// context's master list, packed onto the sheet. False (and no page) when the
/// plan has no materials.
pub fn add_materials_page(layout: &mut Layout, cx: &LayoutRenderContext, number: u32) -> bool {
    let lines = plan_docs::materials_report(
        cx.project,
        plan_docs::MaterialsScope::AllFloors,
        None,
        &cx.master_list,
    );
    let categories: Vec<&str> = plan_docs::MATERIAL_CATEGORIES
        .iter()
        .copied()
        .filter(|c| lines.iter().any(|l| l.category == *c))
        .collect();
    if categories.is_empty() {
        return false;
    }
    layout.add_page(number, "Materials List");
    for c in categories {
        send_to_layout(
            layout,
            cx,
            number,
            BoxSource::Materials {
                floor: None,
                category: Some(c.to_string()),
            },
            Scale::QuarterInch,
            None,
        );
    }
    true
}

/// Sends the 2D view of camera `camera_id` to page `page` as a
/// [`BoxSource::Camera`] box (the "Send to Layout" of a 3D view). With `scale`
/// `None` the largest scale that fits is used, as in [`send_to_layout_auto`].
/// `None` is returned when the project has no such camera. The drawing itself
/// comes from the context's `camera_drawing` hook when the box is drawn.
pub fn send_camera_to_layout(
    layout: &mut Layout,
    cx: &LayoutRenderContext,
    page: u32,
    camera_id: Id,
    scale: Option<Scale>,
) -> Option<Id> {
    cx.project.camera(camera_id)?;
    Some(send_to_layout_auto(
        layout,
        cx,
        page,
        BoxSource::Camera { camera_id },
        scale,
        None,
    ))
}

fn page_text(layout: &mut Layout, page: u32, text: &str, height_in: f64, x: f64, y: f64) {
    if let Some(p) = layout.page_mut(page) {
        let id = p.cad.iter().map(|o| o.id).max().map_or(1, |m| m + 1);
        p.cad.push(CadObject {
            id,
            layer: plan_core::cad::DEFAULT_CAD_LAYER.to_string(),
            item: CadItem::Text {
                pos: Point::new(x, y),
                text: text.to_string(),
                height: height_in,
                angle: 0.0,
            },
        });
    }
}

/// Daniel's construction set on 18x24 (Arch C landscape): his presentation
/// title block ([`TitleBlockTemplate::from_daniel_18x24`]), Chief's layout
/// background and Layout Edge border, and Chief-style box labels (floors spelled
/// out as `FIRST FLOOR PLAN` when the project has several) with the scale note
/// under each box.
///
/// Sheets, numbered `A-0`, `A-1`, ... in his order (see
/// [`append_construction_set`]): Cover (project title and the sheet index
/// table), Site, Floor Plans (one per floor), Elevations (Front and Back, then
/// Left and Right), Sections, Details, Schedules (door, window and room, then
/// the Materials List) and a framing plan placeholder.
///
/// `floors` is how many floors get a plan sheet (clamped to the project's
/// floor count). Views that do not fit a sheet at 1/4" step down to the next
/// smaller scale. Rooms are detected for every floor and the 3D scene is built
/// once.
pub fn default_construction_set(project: &Project, floors: usize) -> Layout {
    default_construction_set_with(project, floors, &plan_docs::MasterList::default())
}

/// [`default_construction_set`] with the Materials List page priced and
/// wasted from `master`.
pub fn default_construction_set_with(
    project: &Project,
    floors: usize,
    master: &plan_docs::MasterList,
) -> Layout {
    let mut layout = Layout::new(
        format!("{} Construction Set", project.name),
        SheetSize::ArchC,
    );
    layout.title_block = TitleBlockTemplate::from_daniel_18x24();
    layout.page_background = true;
    append_construction_set(&mut layout, project, floors, master);
    layout
}

/// The `(floor, id)` of the placed schedules whose kind the layout's own
/// door, window and room schedule boxes do not already show.
fn other_placed_schedules(project: &Project) -> Vec<(usize, Id)> {
    use plan_core::schedules::{ScheduleKind as K, ScheduleLayer};
    let mut out = Vec::new();
    for (floor, f) in project.floors.iter().enumerate() {
        for s in ScheduleLayer::load(f).schedules {
            if !matches!(s.kind, K::Door | K::Window | K::Room) {
                out.push((floor, s.id));
            }
        }
    }
    out
}

/// Adds Daniel's sheet set to `layout`, laid out for its sheet and title block:
///
/// 1. **Cover**: the project title, "CONSTRUCTION DOCUMENTS" and the sheet
///    index as a table box ([`BoxSource::SheetIndex`], kept up to date);
/// 2. **Site Plan**: the first floor's plan at the largest scale up to 1/8";
/// 3. **Floor plans**: one per floor at 1/4";
/// 4. **Elevations**: Front and Back, Left and Right;
/// 5. **Building Section**: a longitudinal section;
/// 6. **Details**: a sheet for typical details;
/// 7. **Schedules**: door, window and room schedules, the Materials List
///    pages, and a framing plan placeholder.
///
/// Pages with nothing on them are removed first (the empty first page a new
/// layout starts with); pages that have content stay, and the set's sheets are
/// numbered after them. The title block, background and margins are the
/// layout's own. Returns the number of pages added.
pub fn append_construction_set(
    layout: &mut Layout,
    project: &Project,
    floors: usize,
    master: &plan_docs::MasterList,
) -> usize {
    append_construction_set_in(layout, project, floors, master, None)
}

/// [`append_construction_set`] measuring elevations and sections on the
/// scene `scene_builder` makes (the application's, with roofs, stairs and
/// the rest of the 3D view) instead of the walls-and-openings scene built
/// here. Placed schedules of the plan that no standard schedule box covers
/// (cabinets, electrical, stairs, notes...) follow the door, window and room
/// schedules on the Schedules sheet, each as a box that follows its
/// Schedule Specification.
pub fn append_construction_set_in(
    layout: &mut Layout,
    project: &Project,
    floors: usize,
    master: &plan_docs::MasterList,
    scene_builder: Option<&dyn Fn(&Project) -> Scene>,
) -> usize {
    layout.pages.retain(|p| {
        p.template_page
            || !(p.boxes.is_empty()
                && p.cad.is_empty()
                && p.leaders.is_empty()
                && p.clouds.is_empty())
    });
    let before = layout.pages.len();
    let mut number = layout.pages.iter().map(|p| p.number + 1).max().unwrap_or(0);

    let scene = match scene_builder {
        Some(f) => f(project),
        None => build_scene_with(project, &SceneOptions::for_project(project)),
    };
    let mut cx = LayoutRenderContext::new(project);
    cx.scene = Some(&scene);
    cx.master_list = master.clone();
    let cx = &cx;
    let area = layout.drawing_area();
    let quarter = Scale::QuarterInch;
    let cover_number = number;

    // Cover (the sheet index goes on last, when every sheet exists).
    layout.add_page(number, "Cover");
    let title = BoxSource::text(project.name.to_uppercase(), 36.0);
    let cover = send_to_layout(layout, cx, number, title, quarter, None);
    if let Some(b) = layout
        .page_mut(number)
        .and_then(|p| p.boxes.iter_mut().find(|b| b.id == cover))
    {
        b.border = false;
    }
    page_text(
        layout,
        number,
        "CONSTRUCTION DOCUMENTS",
        0.3,
        area.0.x + 0.1,
        area.1.y - 1.4,
    );

    // Site plan: the first floor's footprint at a site scale.
    if !project.floors.is_empty() {
        number += 1;
        layout.add_page(number, "Site Plan");
        let source = BoxSource::PlanView {
            floor: 0,
            layer_set: project.layers.name.clone(),
        };
        let scale = fit_largest_scale(layout, cx, &source, Scale::EighthInch);
        let id = send_to_layout(layout, cx, number, source, scale, None);
        if let Some(b) = layout
            .page_mut(number)
            .and_then(|p| p.boxes.iter_mut().find(|b| b.id == id))
        {
            b.label = Some("SITE PLAN".to_string());
        }
    }

    // Floor plans.
    for floor in 0..floors.min(project.floors.len()) {
        number += 1;
        layout.add_page(number, format!("{} Plan", project.floors[floor].name));
        let source = BoxSource::PlanView {
            floor,
            layer_set: project.layers.name.clone(),
        };
        let scale = fit_largest_scale(layout, cx, &source, quarter);
        send_to_layout(layout, cx, number, source, scale, None);
    }

    // Elevations, two per sheet.
    for (title, dirs) in [
        (
            "Elevations: Front and Back",
            [ViewDir::Front, ViewDir::Back],
        ),
        (
            "Elevations: Left and Right",
            [ViewDir::Left, ViewDir::Right],
        ),
    ] {
        number += 1;
        layout.add_page(number, title);
        for dir in dirs {
            let source = BoxSource::Elevation { dir };
            let scale = fit_largest_scale(layout, cx, &source, quarter);
            send_to_layout(layout, cx, number, source, scale, None);
        }
    }

    // Longitudinal section through the middle of the building.
    number += 1;
    layout.add_page(number, "Building Section");
    let cut = scene.bounds().map_or(
        SectionCut {
            plane_normal: ViewDir::Front,
            offset: 0.0,
        },
        |(lo, hi)| {
            let (dx, dz) = (f64::from(hi[0] - lo[0]), f64::from(hi[2] - lo[2]));
            // Cut across the short side so the section runs along the long one.
            if dx >= dz {
                SectionCut {
                    plane_normal: ViewDir::Front,
                    offset: f64::from(lo[2] + hi[2]) * 0.5,
                }
            } else {
                SectionCut {
                    plane_normal: ViewDir::Left,
                    offset: f64::from(lo[0] + hi[0]) * 0.5,
                }
            }
        },
    );
    let source = BoxSource::Section { cut };
    let scale = fit_largest_scale(layout, cx, &source, quarter);
    send_to_layout(layout, cx, number, source, scale, None);

    // Details.
    number += 1;
    layout.add_page(number, "Details");
    let text = BoxSource::text("TYPICAL DETAILS\nTO BE DEVELOPED".to_string(), 18.0);
    send_to_layout(layout, cx, number, text, quarter, None);

    // Schedules.
    number += 1;
    layout.add_page(number, "Schedules");
    for kind in [ScheduleKind::Door, ScheduleKind::Window, ScheduleKind::Room] {
        send_to_layout(
            layout,
            cx,
            number,
            BoxSource::Schedule { kind },
            quarter,
            None,
        );
    }

    // Placed schedules no standard box covers: the plan's cabinet, stair,
    // electrical... schedules, as the plan shows them.
    for (floor, id) in other_placed_schedules(project) {
        send_to_layout(
            layout,
            cx,
            number,
            BoxSource::PlacedSchedule { floor, id },
            quarter,
            None,
        );
    }

    // Materials List: one table per category that has rows.
    if add_materials_page(layout, cx, number + 1) {
        number += 1;
    }

    // Framing plan placeholder.
    number += 1;
    layout.add_page(number, "Framing Plan");
    let text = BoxSource::text("FRAMING PLAN\nTO BE DEVELOPED".to_string(), 18.0);
    send_to_layout(layout, cx, number, text, quarter, None);

    // The sheet index, now that every sheet has its title.
    layout.sheet_index = false;
    cx.set_sheet_index(layout);
    send_to_layout(
        layout,
        cx,
        cover_number,
        BoxSource::SheetIndex,
        quarter,
        None,
    );

    layout.pages.len() - before
}
