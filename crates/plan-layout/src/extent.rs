//! Source extents: how big a box's content is, in source or paper units.

use crate::model::{BoxSource, ScheduleKind};
use crate::render::LayoutRenderContext;
use plan_3d::{build_scene, Scene};
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

/// The scene for elevations and sections: the caller's, or one built on demand.
pub(crate) struct SceneSource<'a> {
    given: Option<&'a Scene>,
    built: OnceCell<Scene>,
}

impl<'a> SceneSource<'a> {
    pub(crate) fn new(given: Option<&'a Scene>) -> Self {
        Self {
            given,
            built: OnceCell::new(),
        }
    }

    pub(crate) fn get(&self, project: &Project) -> &Scene {
        match self.given {
            Some(s) => s,
            None => self.built.get_or_init(|| build_scene(project)),
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

/// One table for `kind` covering every floor. With several floors the number
/// column is prefixed with the floor number (`2-D01`).
pub(crate) fn schedule_for(kind: ScheduleKind, cx: &LayoutRenderContext) -> Schedule {
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
        BoxSource::Section { cut } => view_frame(cut.plane_normal, cx.project, scenes),
        BoxSource::Schedule { kind } => {
            let m = table_metrics(&schedule_for(*kind, cx));
            Frame::Paper {
                w_in: m.width / 72.0,
                h_in: m.height / 72.0,
            }
        }
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
        BoxSource::Image { .. } => Frame::Paper {
            w_in: IMAGE_SIZE_IN.0,
            h_in: IMAGE_SIZE_IN.1,
        },
        BoxSource::Text { text, height_pt } => {
            let (w_in, h_in) = text_size_in(text, *height_pt);
            Frame::Paper { w_in, h_in }
        }
    }
}

/// Paper size `(width, height)` in inches that `source` needs at `scale`.
///
/// Plan views are the wall bounds (plus dimension lines) with a 24" margin;
/// elevations and sections the projected model with a 12" margin; schedules
/// the text-measured table (0.25" rows); text its measured lines.
pub fn source_size_in(source: &BoxSource, scale: Scale, cx: &LayoutRenderContext) -> (f64, f64) {
    let scenes = SceneSource::new(cx.scene);
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
