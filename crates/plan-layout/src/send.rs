//! Send to Layout and the default construction set.

use crate::extent::{frame_for, size_of, source_size_in, Frame, SceneSource};
use crate::model::{BoxSource, Layout, LayoutBox, ScheduleKind, LABEL_GAP_IN};
use crate::render::LayoutRenderContext;
use crate::titleblock::TitleBlockTemplate;
use plan_3d::build_scene;
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

fn default_label(source: &BoxSource, project: &Project) -> Option<String> {
    match source {
        BoxSource::PlanView { floor, .. } => {
            let name = project.floors.get(*floor).map_or("", |f| f.name.as_str());
            Some(format!("{name} PLAN").to_uppercase())
        }
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
        BoxSource::CadDetail { name, .. } => Some(name.to_uppercase()),
        BoxSource::Schedule { .. } | BoxSource::Image { .. } | BoxSource::Text { .. } => None,
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
    let (w, h) = source_size_in(&source, scale, cx);
    let label = default_label(&source, cx.project);
    let id = layout.next_box_id();
    if layout.page(page).is_none() {
        layout.add_page(page, format!("Sheet A-{page}"));
    }
    let lower_left = match at {
        Some(p) => p,
        None => {
            let area = layout.drawing_area();
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

/// The largest scale at or below `want` at which `source` fits the layout's
/// drawing area (the smallest scale if none does).
fn fit_scale(layout: &Layout, cx: &LayoutRenderContext, source: &BoxSource, want: Scale) -> Scale {
    let (lo, hi) = layout.drawing_area();
    let scenes = SceneSource::new(cx.scene);
    let frame = frame_for(source, cx, &scenes);
    if matches!(frame, Frame::Paper { .. }) {
        return want;
    }
    let mut s = want;
    loop {
        let (w, h) = size_of(frame, s);
        let fits = w <= hi.x - lo.x && h + LABEL_GAP_IN <= hi.y - lo.y;
        match (fits, s.smaller()) {
            (true, _) | (false, None) => return s,
            (false, Some(next)) => s = next,
        }
    }
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

/// Chief-like construction set on 18x24 (Arch C landscape).
///
/// Sheets, numbered `A-0`, `A-1`, ... in order:
/// cover (project title and sheet index); one floor plan per floor at 1/4";
/// elevations (Front and Back, then Left and Right, two per sheet);
/// one longitudinal section through the middle of the building; door,
/// window and room schedules; and a framing plan placeholder.
///
/// `floors` is how many floors get a plan sheet (clamped to the project's
/// floor count). Views that do not fit a sheet at 1/4" step down to the next
/// smaller scale. Rooms are detected for every floor and the 3D scene is built
/// once.
pub fn default_construction_set(project: &Project, floors: usize) -> Layout {
    let mut layout = Layout::new(
        format!("{} Construction Set", project.name),
        SheetSize::ArchC,
    );
    layout.title_block = TitleBlockTemplate::presentation_18x24();
    layout.sheet_index = true;

    let scene = build_scene(project);
    let mut cx = LayoutRenderContext::new(project);
    cx.scene = Some(&scene);
    let cx = &cx;
    let area = layout.drawing_area();
    let quarter = Scale::QuarterInch;
    let mut number = 0_u32;

    // Cover.
    layout.add_page(number, "Cover");
    let title = BoxSource::Text {
        text: project.name.to_uppercase(),
        height_pt: 36.0,
    };
    let cover = send_to_layout(&mut layout, cx, number, title, quarter, None);
    if let Some(b) = layout
        .page_mut(number)
        .and_then(|p| p.boxes.iter_mut().find(|b| b.id == cover))
    {
        b.border = false;
    }
    page_text(
        &mut layout,
        number,
        "CONSTRUCTION DOCUMENTS",
        0.3,
        area.0.x + 0.1,
        area.1.y - 1.4,
    );

    // Floor plans.
    for floor in 0..floors.min(project.floors.len()) {
        number += 1;
        layout.add_page(number, format!("{} Plan", project.floors[floor].name));
        let source = BoxSource::PlanView {
            floor,
            layer_set: project.layers.name.clone(),
        };
        let scale = fit_scale(&layout, cx, &source, quarter);
        send_to_layout(&mut layout, cx, number, source, scale, None);
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
            let scale = fit_scale(&layout, cx, &source, quarter);
            send_to_layout(&mut layout, cx, number, source, scale, None);
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
    let scale = fit_scale(&layout, cx, &source, quarter);
    send_to_layout(&mut layout, cx, number, source, scale, None);

    // Schedules.
    number += 1;
    layout.add_page(number, "Schedules");
    for kind in [ScheduleKind::Door, ScheduleKind::Window, ScheduleKind::Room] {
        send_to_layout(
            &mut layout,
            cx,
            number,
            BoxSource::Schedule { kind },
            quarter,
            None,
        );
    }

    // Framing plan placeholder.
    number += 1;
    layout.add_page(number, "Framing Plan");
    let text = BoxSource::Text {
        text: "FRAMING PLAN\nTO BE DEVELOPED".to_string(),
        height_pt: 18.0,
    };
    send_to_layout(&mut layout, cx, number, text, quarter, None);

    layout
}
