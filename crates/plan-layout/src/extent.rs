//! Source extents: how big a box's content is, in source or paper units.

use crate::model::{BoxSource, ScheduleKind};
use crate::render::LayoutRenderContext;
use plan_3d::{build_scene_with, Scene, SceneOptions};
use plan_core::{detect_rooms, Floor, Point, Project};
use plan_docs::{
    door_schedule, room_schedule, wall_schedule, window_schedule, PdfDoc, Scale, Schedule,
};
use plan_elevation::{Projection, ViewDir};
use std::cell::OnceCell;

/// Margin added around the wall bounds of a plan view, plan inches.
pub(crate) const PLAN_MARGIN_IN: f64 = 24.0;
/// Margin added around an elevation or section, drawing inches.
pub(crate) const VIEW_MARGIN_IN: f64 = 12.0;
/// Margin added around CAD detail items, detail inches.
pub(crate) const DETAIL_MARGIN_IN: f64 = 6.0;
/// Schedule row height, points (0.25").
pub(crate) const ROW_H_PT: f64 = 18.0;
/// Height of the schedule title line, points.
pub(crate) const TABLE_TITLE_H_PT: f64 = 21.6;
/// Schedule cell text size, points.
pub(crate) const TABLE_TEXT_PT: f64 = 8.0;
const CELL_PAD_PT: f64 = 10.0;
/// Placeholder size for image boxes, paper inches.
const IMAGE_SIZE_IN: (f64, f64) = (4.0, 3.0);
/// Line height as a multiple of the text size.
pub(crate) const LINE_SPACING: f64 = 1.25;

/// The scene for elevations and sections: the caller's, one the context's
/// builder makes on demand, or the plan's own opening display (casing, jambs,
/// sills; `SceneOptions::for_project`) built here.
pub(crate) struct SceneSource<'a> {
    given: Option<&'a Scene>,
    builder: Option<&'a dyn Fn(&Project) -> Scene>,
    built: OnceCell<Scene>,
}

impl<'a> SceneSource<'a> {
    #[cfg(test)]
    pub(crate) fn new(given: Option<&'a Scene>) -> Self {
        Self {
            given,
            builder: None,
            built: OnceCell::new(),
        }
    }

    /// The scene source of a render context: its scene, else its scene
    /// builder, else the built-in one.
    pub(crate) fn for_context(cx: &'a LayoutRenderContext<'_>) -> Self {
        Self {
            given: cx.scene,
            builder: cx.scene_builder.as_deref(),
            built: OnceCell::new(),
        }
    }

    pub(crate) fn get(&self, project: &Project) -> &Scene {
        match self.given {
            Some(s) => s,
            None => self.built.get_or_init(|| match self.builder {
                Some(f) => f(project),
                None => build_scene_with(project, &SceneOptions::for_project(project)),
            }),
        }
    }
}

/// Where a source's content lives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Frame {
    /// Source-space rectangle (inches of the building), drawn at the box scale.
    Scaled { lo: Point, hi: Point },
    /// A paper-space size, independent of the box scale.
    Paper { w_in: f64, h_in: f64 },
}

fn pad(lo: Point, hi: Point, m: f64) -> Frame {
    Frame::Scaled {
        lo: Point::new(lo.x - m, lo.y - m),
        hi: Point::new(hi.x + m, hi.y + m),
    }
}

/// Wall footprints and dimension lines of a floor: `(min, max)` in plan inches.
pub(crate) fn plan_bounds(f: &Floor) -> Option<(Point, Point)> {
    let wall_pts = f.walls.iter().flat_map(|w| w.footprint());
    let dim_pts = f.dimensions.iter().flat_map(|d| {
        let (a, b) = d.line_points();
        [d.start, d.end, a, b]
    });
    let mut it = wall_pts.chain(dim_pts);
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

fn view_frame(dir: ViewDir, project: &Project, scenes: &SceneSource) -> Frame {
    match scenes.get(project).bounds() {
        Some(bounds) => {
            let (lo, hi) = Projection::for_view(dir, bounds).extent();
            pad(lo, hi, VIEW_MARGIN_IN)
        }
        None => pad(Point::ZERO, Point::new(120.0, 120.0), 0.0),
    }
}

/// Column widths and overall size of a schedule table, points.
pub(crate) struct TableMetrics {
    pub cols: Vec<f64>,
    pub width: f64,
    pub height: f64,
}

pub(crate) fn table_metrics(s: &Schedule) -> TableMetrics {
    let cols: Vec<f64> = (0..s.columns.len())
        .map(|c| {
            let header = PdfDoc::text_width(&s.columns[c], TABLE_TEXT_PT) + 1.0;
            s.rows
                .iter()
                .filter_map(|r| r.get(c))
                .map(|t| PdfDoc::text_width(t, TABLE_TEXT_PT))
                .fold(header, f64::max)
                + CELL_PAD_PT
        })
        .collect();
    TableMetrics {
        width: cols.iter().sum(),
        height: TABLE_TITLE_H_PT + ROW_H_PT * (s.rows.len() + 1) as f64,
        cols,
    }
}

/// The plan's own Schedule Specification for a standard schedule box: the
/// first placed schedule of the same kind (door, window, room or wall) and
/// the floor it sits on.
fn spec_for(
    kind: ScheduleKind,
    project: &Project,
) -> Option<(usize, plan_core::schedules::Schedule)> {
    use plan_core::schedules::{ScheduleKind as K, ScheduleLayer};
    let want = match kind {
        ScheduleKind::Door => K::Door,
        ScheduleKind::Window => K::Window,
        ScheduleKind::Room => K::Room,
        ScheduleKind::Wall => K::Wall,
    };
    project.floors.iter().enumerate().find_map(|(i, f)| {
        ScheduleLayer::load(f)
            .schedules
            .into_iter()
            .find(|s| s.kind == want)
            .map(|s| (i, s))
    })
}

/// One table for `kind` covering every floor. With several floors the number
/// column is prefixed with the floor number (`2-D01`).
///
/// When the plan has a placed schedule of that kind, the table is built from
/// its Schedule Specification (the columns shown and their order, headings,
/// sort, grouping, filter and totals), over all floors, so the schedule on
/// the sheet reads the way the one in the plan does.
pub(crate) fn schedule_for(kind: ScheduleKind, cx: &LayoutRenderContext) -> Schedule {
    if let Some((floor, mut def)) = spec_for(kind, cx.project) {
        let legacy = standard_schedule_for(kind, cx);
        def.floor_scope = plan_core::schedules::FloorScope::All;
        let rooms = cx.rooms_by_floor.get(floor).map(|r| (floor, r.as_slice()));
        let mut t = plan_docs::schedule_kinds::table(cx.project, &def, floor, rooms);
        if def.title.trim().is_empty() {
            t.title = legacy.title;
        }
        return t;
    }
    standard_schedule_for(kind, cx)
}

/// The standard columns of `kind`, every floor in one table.
fn standard_schedule_for(kind: ScheduleKind, cx: &LayoutRenderContext) -> Schedule {
    let floors = cx.project.floors.len();
    let mut out: Option<Schedule> = None;
    for floor in 0..floors {
        let mut s = match kind {
            ScheduleKind::Door => door_schedule(cx.project, floor),
            ScheduleKind::Window => window_schedule(cx.project, floor),
            ScheduleKind::Wall => wall_schedule(cx.project, floor),
            ScheduleKind::Room => match cx.rooms_by_floor.get(floor) {
                Some(rooms) => room_schedule(cx.project, floor, rooms),
                None => {
                    let rooms = detect_rooms(&cx.project.floors[floor].walls, 1.0);
                    room_schedule(cx.project, floor, &rooms)
                }
            },
        };
        if floors > 1 {
            for row in &mut s.rows {
                if let Some(n) = row.first_mut() {
                    *n = format!("{}-{n}", floor + 1);
                }
            }
        }
        match &mut out {
            None => out = Some(s),
            Some(acc) => acc.rows.append(&mut s.rows),
        }
    }
    out.unwrap_or_else(|| match kind {
        ScheduleKind::Door => door_schedule(cx.project, 0),
        ScheduleKind::Window => window_schedule(cx.project, 0),
        ScheduleKind::Wall => wall_schedule(cx.project, 0),
        ScheduleKind::Room => room_schedule(cx.project, 0, &[]),
    })
}

/// The table a schedule, placed schedule, Materials List or sheet index box
/// shows (for Export CSV / Excel); `None` for every other box.
pub fn box_table(b: &crate::model::LayoutBox, cx: &LayoutRenderContext) -> Option<Schedule> {
    match &b.source {
        BoxSource::Schedule { kind } => Some(schedule_for(*kind, cx)),
        BoxSource::PlacedSchedule { floor, id } => placed_schedule_table(cx, *floor, *id),
        BoxSource::Materials { floor, category } => {
            Some(cx.materials_table(*floor, category.as_deref()))
        }
        BoxSource::SheetIndex => Some(cx.sheet_index_table()),
        BoxSource::PageTable => Some(cx.page_table()),
        BoxSource::RevisionTable => Some(cx.revision_table()),
        _ => None,
    }
}

/// The table of the placed schedule `id` on `floor`, as the plan shows it;
/// `None` when the plan has no such schedule.
pub(crate) fn placed_schedule_table(
    cx: &LayoutRenderContext,
    floor: usize,
    id: plan_core::Id,
) -> Option<Schedule> {
    let f = cx.project.floors.get(floor)?;
    let layer = plan_core::schedules::ScheduleLayer::load(f);
    let def = layer.find(id)?;
    let rooms = cx.rooms_by_floor.get(floor).map(|r| (floor, r.as_slice()));
    Some(plan_docs::schedule_kinds::table(
        cx.project, def, floor, rooms,
    ))
}

/// Lines of a text source and the paper size they need, inches.
pub(crate) fn text_size_in(text: &str, height_pt: f64) -> (f64, f64) {
    let lines: Vec<&str> = text.lines().collect();
    let w = lines
        .iter()
        .map(|l| PdfDoc::text_width(l, height_pt))
        .fold(0.0, f64::max);
    let h = lines.len().max(1) as f64 * height_pt * LINE_SPACING;
    (w / 72.0 + 0.1, h / 72.0 + 0.05)
}

pub(crate) fn frame_for(
    source: &BoxSource,
    cx: &LayoutRenderContext,
    scenes: &SceneSource,
) -> Frame {
    match source {
        BoxSource::PlanView { floor, .. } => {
            match cx.project.floors.get(*floor).and_then(plan_bounds) {
                Some((lo, hi)) => pad(lo, hi, PLAN_MARGIN_IN),
                None => pad(Point::ZERO, Point::new(120.0, 120.0), 0.0),
            }
        }
        BoxSource::Elevation { dir } => view_frame(*dir, cx.project, scenes),
        BoxSource::Camera { camera_id } => match cx.camera_drawing_for(*camera_id) {
            Some(d) if !d.lines.is_empty() => {
                let (lo, hi) = d.bounds;
                pad(lo, hi, VIEW_MARGIN_IN)
            }
            _ => pad(Point::ZERO, Point::new(120.0, 120.0), 0.0),
        },
        BoxSource::Section { cut } => view_frame(cut.plane_normal, cx.project, scenes),
        BoxSource::Schedule { kind } => {
            let m = table_metrics(&schedule_for(*kind, cx));
            Frame::Paper {
                w_in: m.width / 72.0,
                h_in: m.height / 72.0,
            }
        }
        BoxSource::PlacedSchedule { floor, id } => match placed_schedule_table(cx, *floor, *id) {
            Some(t) => {
                let m = table_metrics(&t);
                Frame::Paper {
                    w_in: m.width / 72.0,
                    h_in: m.height / 72.0,
                }
            }
            None => Frame::Paper {
                w_in: IMAGE_SIZE_IN.0,
                h_in: 1.0,
            },
        },
        BoxSource::CadDetail { items, .. } => {
            let mut it = items.iter().map(|o| o.bounds());
            match it.next() {
                Some(first) => {
                    let (lo, hi) = it.fold(first, |(lo, hi), (a, b)| {
                        (
                            Point::new(lo.x.min(a.x), lo.y.min(a.y)),
                            Point::new(hi.x.max(b.x), hi.y.max(b.y)),
                        )
                    });
                    pad(lo, hi, DETAIL_MARGIN_IN)
                }
                None => pad(Point::ZERO, Point::new(12.0, 12.0), 0.0),
            }
        }
        BoxSource::Image { path } => match cx.picture_for(path) {
            Some(img) if img.width > 0 && img.height > 0 => Frame::Paper {
                w_in: IMAGE_SIZE_IN.0,
                h_in: (IMAGE_SIZE_IN.0 * f64::from(img.height) / f64::from(img.width)).min(6.0),
            },
            _ => Frame::Paper {
                w_in: IMAGE_SIZE_IN.0,
                h_in: IMAGE_SIZE_IN.1,
            },
        },
        // 4" wide at the image's aspect ratio (at most 6" tall).
        BoxSource::ImageData { width, height, .. } => {
            if *width == 0 || *height == 0 {
                Frame::Paper {
                    w_in: IMAGE_SIZE_IN.0,
                    h_in: IMAGE_SIZE_IN.1,
                }
            } else {
                let h = (IMAGE_SIZE_IN.0 * f64::from(*height) / f64::from(*width)).min(6.0);
                Frame::Paper {
                    w_in: IMAGE_SIZE_IN.0,
                    h_in: h,
                }
            }
        }
        BoxSource::Text {
            text, height_pt, ..
        } => {
            let (w_in, h_in) = text_size_in(text, *height_pt);
            Frame::Paper { w_in, h_in }
        }
        BoxSource::Materials { floor, category } => {
            let m = table_metrics(&cx.materials_table(*floor, category.as_deref()));
            Frame::Paper {
                w_in: m.width / 72.0,
                h_in: m.height / 72.0,
            }
        }
        BoxSource::SheetIndex => {
            let m = table_metrics(&cx.sheet_index_table());
            Frame::Paper {
                w_in: (m.width / 72.0).max(3.0),
                h_in: m.height / 72.0,
            }
        }
        BoxSource::PageTable | BoxSource::RevisionTable => {
            let t = if matches!(source, BoxSource::PageTable) {
                cx.page_table()
            } else {
                cx.revision_table()
            };
            let m = table_metrics(&t);
            Frame::Paper {
                w_in: (m.width / 72.0).max(3.0),
                h_in: m.height / 72.0,
            }
        }
        // A 6" x 4.5" image of the 4:3 render.
        BoxSource::Perspective { .. } => Frame::Paper {
            w_in: 6.0,
            h_in: 4.5,
        },
    }
}

/// Paper size `(width, height)` in inches that `source` needs at `scale`.
///
/// Plan views are the wall bounds (plus dimension lines) with a 24" margin;
/// elevations and sections the projected model with a 12" margin; schedules
/// the text-measured table (0.25" rows); text its measured lines.
pub fn source_size_in(source: &BoxSource, scale: Scale, cx: &LayoutRenderContext) -> (f64, f64) {
    let scenes = SceneSource::for_context(cx);
    size_of(frame_for(source, cx, &scenes), scale)
}

pub(crate) fn size_of(frame: Frame, scale: Scale) -> (f64, f64) {
    match frame {
        Frame::Scaled { lo, hi } => {
            let k = scale.inches_per_foot() / 12.0;
            ((hi.x - lo.x) * k, (hi.y - lo.y) * k)
        }
        Frame::Paper { w_in, h_in } => (w_in, h_in),
    }
}
