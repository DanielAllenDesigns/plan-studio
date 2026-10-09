//! The Construction Line tool (CAD > Line > Construction Line; manual pp.
//! 83-86, CAD-62..CAD-64) and the Edit Reference Document Offset tool
//! (Tools > Floor/Reference Display, manual p. 91, LAY-44).
//!
//! # Construction Line
//!
//! Click and drag, or click two points, to draw a guide line. The points
//! snap like any CAD point (and to other construction lines, which are very
//! long snap segments, see `editor::ref_overlay`). The line is a CAD line on
//! the "Construction Lines" layer in drawing group 21 with a
//! [`plan_core::construction::ConstructionLine`] record made from the plan's
//! defaults; it is infinite in plan unless the defaults say otherwise. The
//! tool stays active for the next line; Esc drops a pending first point.
//! One line is one undo step.
//!
//! # Edit Reference Document Offset
//!
//! Shows a marquee around the plan another Reference Display row refers to,
//! with a Move handle (drag the marquee, or its center) that changes the X
//! and Y offset and a Rotate handle that changes the Angle about the
//! marquee's center. With several referenced plans a small dialog asks
//! which to edit. One drag is one undo step; Esc or Enter leaves the tool.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::ref_overlay;
use crate::editor::{Camera, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Align2, Key, Pos2, Shape, Stroke};
use plan_core::construction::{self, ReferenceSource};
use plan_core::geometry::{point_in_polygon, Point};
use std::cell::RefCell;

/// A line shorter than this (inches) is a stray click.
const MIN_LENGTH: f64 = 1.0;

// ---------------------------------------------------------------------------
// Construction Line tool
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ConstructionLineTool {
    anchor: Option<Point>,
    hover: Option<Point>,
}

/// Draws a construction line from `a` to `b` on the active floor from the
/// plan's defaults. One undo step. Returns the id of its CAD line.
pub fn add_line(cx: &mut EditorContext, a: Point, b: Point) -> Option<plan_core::Id> {
    if a.dist(b) < MIN_LENGTH {
        return None;
    }
    cx.begin_change("Construction Line");
    let id = cx.project.add_construction_line(cx.floor, a, b, None);
    match id {
        Some(id) => {
            cx.mark_dirty();
            cx.selection.set(ObjectRef::Cad(id));
            Some(id)
        }
        None => {
            cx.cancel_change();
            None
        }
    }
}

impl ConstructionLineTool {
    fn finish(&mut self, cx: &mut EditorContext, b: Point) -> ToolResult {
        let Some(a) = self.anchor else {
            return ToolResult::ignored();
        };
        match add_line(cx, a, b) {
            Some(_) => {
                self.anchor = None;
                self.hover = None;
                cx.status = "Construction line drawn".into();
                ToolResult::committed("Construction Line")
            }
            None => {
                cx.status = "Draw farther to make the line".into();
                ToolResult::consumed()
            }
        }
    }

    fn snap(&self, cx: &EditorContext, p: &PointerEvent) -> Point {
        cx.snap_at(p.world, self.anchor, p.modifiers.alt, &[]).point
    }
}

impl Tool for ConstructionLineTool {
    fn id(&self) -> ToolId {
        ToolId::ConstructionLine
    }

    fn name(&self) -> &'static str {
        "Construction Line"
    }

    fn hint(&self) -> String {
        "Construction Line: click and drag, or click two points, to draw a guide line".into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.anchor = None;
        self.hover = None;
        cx.status = self.hint();
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.anchor = None;
        self.hover = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.hover = Some(self.snap(cx, &p));
        ToolResult::consumed()
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let at = self.snap(cx, &p);
        if self.anchor.is_some() {
            return self.finish(cx, at);
        }
        self.anchor = Some(at);
        self.hover = Some(at);
        cx.status = "Construction Line: click the second point".into();
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        // A drag ends the line where the button is let go; a plain click
        // keeps the first point for the second click.
        let Some(a) = self.anchor else {
            return ToolResult::ignored();
        };
        let at = self.snap(cx, &p);
        if a.dist(at) >= MIN_LENGTH.max(3.0 * cx.snap_tol()) {
            return self.finish(cx, at);
        }
        ToolResult::consumed()
    }

    fn key(&mut self, _cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Escape) && self.anchor.take().is_some() {
            self.hover = None;
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let (Some(a), Some(b)) = (self.anchor, self.hover) else {
            return;
        };
        if a.dist(b) < 1e-6 {
            return;
        }
        let stroke = Stroke::new(1.2_f32, cx.palette.selection);
        // The line as it will be: across the whole view when the defaults
        // make it infinite.
        let (pa, pb) = (cam.world_to_screen(a), cam.world_to_screen(b));
        let far = if cx.project.construction.defaults.infinite_plan {
            construction::clip_line_to_rect(
                Point::new(f64::from(pa.x), f64::from(pa.y)),
                Point::new(f64::from(pb.x), f64::from(pb.y)),
                Point::new(f64::from(cam.rect.min.x), f64::from(cam.rect.min.y)),
                Point::new(f64::from(cam.rect.max.x), f64::from(cam.rect.max.y)),
            )
            .map(|(s, e)| {
                (
                    Pos2::new(s.x as f32, s.y as f32),
                    Pos2::new(e.x as f32, e.y as f32),
                )
            })
        } else {
            Some((pa, pb))
        };
        if let Some((s, e)) = far {
            painter.extend(Shape::dashed_line(&[s, e], stroke, 8.0, 5.0));
        }
        painter.line_segment([pa, pb], Stroke::new(2.0_f32, cx.palette.selection));
        painter.circle_filled(pa, 3.0, cx.palette.selection);
        painter.circle_filled(pb, 3.0, cx.palette.selection);
    }
}

// ---------------------------------------------------------------------------
// Edit Reference Document Offset
// ---------------------------------------------------------------------------

/// The row of the table the pick dialog chose, set by `frame`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Pick {
    /// Indices into the table's rows that refer to another plan.
    rows: Vec<usize>,
    chosen: usize,
}

thread_local! {
    static PICK: RefCell<Option<Pick>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    /// Moving: the last pointer position.
    Move { last: Point, moved: bool },
    /// Rotating about `pivot`: the row's angle and the pointer's angle at
    /// the start (degrees).
    Rotate {
        pivot: Point,
        angle0: f64,
        pointer0: f64,
        moved: bool,
    },
}

#[derive(Default)]
pub struct ReferenceOffsetTool {
    /// The table row being edited.
    row: Option<usize>,
    drag: Option<Drag>,
}

/// The four corners of the marquee around the plan row `row` refers to, in
/// plan coordinates (the other plan's walls, turned and moved by the row's
/// angle and offset). `None` when the row's file cannot be read or has no
/// walls on the floor it shows.
pub fn marquee(cx: &EditorContext, row: usize) -> Option<[Point; 4]> {
    let r = cx.project.reference_table.rows.get(row)?;
    let ReferenceSource::File(path) = &r.source else {
        return None;
    };
    let plan = ref_overlay::other_plan(path)?;
    let floor = r.floor.resolve(cx.floor, plan.floors.len(), true)?;
    let mut pts = plan.floors[floor]
        .walls
        .iter()
        .filter(|w| !w.flags.invisible)
        .flat_map(|w| w.footprint());
    let first = pts.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in pts {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Some([lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)].map(|p| r.to_world(p)))
}

/// The Move handle (the marquee's center) and the Rotate handle (past the
/// middle of the marquee's top edge, `reach` inches out).
pub fn handles(m: &[Point; 4], reach: f64) -> (Point, Point) {
    let center = Point::lerp(m[0], m[2], 0.5);
    let top = Point::lerp(m[3], m[2], 0.5);
    let out = top.sub(center);
    let dir = if out.length() < 1e-9 {
        Point::new(0.0, 1.0)
    } else {
        out.normalized()
    };
    (center, top.add(dir.scale(reach)))
}

impl ReferenceOffsetTool {
    fn reach(cx: &EditorContext) -> f64 {
        // 30 pixels at the snap tolerance of 10.
        3.0 * cx.snap_tol()
    }

    fn marquee_handles(&self, cx: &EditorContext) -> Option<([Point; 4], Point, Point)> {
        let m = marquee(cx, self.row?)?;
        let (mv, rot) = handles(&m, Self::reach(cx));
        Some((m, mv, rot))
    }

    /// Starts editing row `row` of the table.
    pub fn edit_row(&mut self, cx: &mut EditorContext, row: usize) {
        self.row = Some(row);
        cx.status =
            "Edit Reference Document Offset: drag to move, drag the round handle to rotate".into();
    }

    /// The row being edited.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn row(&self) -> Option<usize> {
        self.row
    }
}

fn angle_of(from: Point, to: Point) -> f64 {
    to.sub(from).angle().to_degrees()
}

impl Tool for ReferenceOffsetTool {
    fn id(&self) -> ToolId {
        ToolId::ReferenceOffset
    }

    fn name(&self) -> &'static str {
        "Edit Reference Document Offset"
    }

    fn hint(&self) -> String {
        "Edit Reference Document Offset: drag the marquee to move the other plan, \
         drag the round handle to rotate it"
            .into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Move
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.drag = None;
        self.row = None;
        let rows = cx.project.reference_table.file_rows();
        match rows.as_slice() {
            [] => {
                cx.status = "No other plan file is in the Reference Display".into();
                cx.requests.push(EditorRequest::SetTool(ToolId::Select));
            }
            [one] => self.edit_row(cx, *one),
            many => {
                PICK.with(|p| {
                    *p.borrow_mut() = Some(Pick {
                        rows: many.to_vec(),
                        chosen: 0,
                    })
                });
                cx.status = self.hint();
            }
        }
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.drag = None;
        self.row = None;
        PICK.with(|p| *p.borrow_mut() = None);
    }

    fn frame(&mut self, cx: &mut EditorContext, ctx: &egui::Context) {
        let Some(mut pick) = PICK.with(|p| p.borrow().clone()) else {
            return;
        };
        let mut done = None;
        egui::Window::new("Select Reference Document to Edit")
            .id(egui::Id::new("ref_offset_pick"))
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                for (i, r) in pick.rows.iter().enumerate() {
                    let name = cx.project.reference_table.rows[*r].source.label();
                    ui.radio_value(&mut pick.chosen, i, name);
                }
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        done = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
        match done {
            None => PICK.with(|p| *p.borrow_mut() = Some(pick)),
            Some(true) => {
                PICK.with(|p| *p.borrow_mut() = None);
                let row = pick.rows[pick.chosen.min(pick.rows.len() - 1)];
                self.edit_row(cx, row);
            }
            Some(false) => {
                PICK.with(|p| *p.borrow_mut() = None);
                cx.requests.push(EditorRequest::SetTool(ToolId::Select));
            }
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let Some((m, mv, rot)) = self.marquee_handles(cx) else {
            return ToolResult::ignored();
        };
        let tol = 1.5 * cx.snap_tol();
        let row = self.row.unwrap_or(0);
        let drag = if p.world.dist(rot) <= tol {
            let pivot = Point::lerp(m[0], m[2], 0.5);
            Some(Drag::Rotate {
                pivot,
                angle0: cx.project.reference_table.rows[row].angle_deg,
                pointer0: angle_of(pivot, p.world),
                moved: false,
            })
        } else if p.world.dist(mv) <= tol || point_in_polygon(p.world, &m) {
            Some(Drag::Move {
                last: p.world,
                moved: false,
            })
        } else {
            None
        };
        let Some(drag) = drag else {
            return ToolResult::consumed();
        };
        cx.begin_change("Edit Reference Document Offset");
        self.drag = Some(drag);
        ToolResult::consumed()
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let (Some(drag), Some(row)) = (self.drag, self.row) else {
            return ToolResult::ignored();
        };
        match drag {
            Drag::Move { last, .. } => {
                let delta = p.world.sub(last);
                if let Some(r) = cx.project.reference_table.rows.get_mut(row) {
                    r.move_by(delta);
                }
                self.drag = Some(Drag::Move {
                    last: p.world,
                    moved: true,
                });
            }
            Drag::Rotate {
                pivot,
                angle0,
                pointer0,
                ..
            } => {
                self.drag = Some(Drag::Rotate {
                    pivot,
                    angle0,
                    pointer0,
                    moved: true,
                });
                let mut angle = angle0 + (angle_of(pivot, p.world) - pointer0);
                if p.modifiers.shift {
                    angle = (angle / 5.0).round() * 5.0;
                }
                if let Some(r) = cx.project.reference_table.rows.get_mut(row) {
                    r.rotate_about(pivot, angle);
                }
            }
        }
        cx.mark_dirty();
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        let Some(drag) = self.drag.take() else {
            return ToolResult::ignored();
        };
        let changed = match drag {
            Drag::Move { moved, .. } => moved,
            Drag::Rotate { moved, .. } => moved,
        };
        if changed {
            ToolResult::committed("Edit Reference Document Offset")
        } else {
            cx.cancel_change();
            ToolResult::consumed()
        }
    }

    fn key(&mut self, _cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Enter) {
            return ToolResult {
                consumed: true,
                repaint: true,
                switch_to: Some(ToolId::Select),
                commit: None,
            };
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let Some((m, mv, rot)) = self.marquee_handles(cx) else {
            return;
        };
        let stroke = Stroke::new(1.5_f32, cx.palette.selection);
        let pts: Vec<Pos2> = m.iter().map(|p| cam.world_to_screen(*p)).collect();
        painter.add(Shape::closed_line(pts, stroke));
        let (cm, cr) = (cam.world_to_screen(mv), cam.world_to_screen(rot));
        let top = cam.world_to_screen(Point::lerp(m[3], m[2], 0.5));
        painter.line_segment([top, cr], stroke);
        painter.rect_filled(
            egui::Rect::from_center_size(cm, egui::vec2(10.0, 10.0)),
            1.0,
            cx.palette.selection,
        );
        painter.circle_filled(cr, 6.0, cx.palette.selection);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::construction::ReferenceRow;
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        EditorContext::new(crate::plan_defaults::embedded())
    }

    fn event(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(cx, Point::new(x, y))
    }

    fn down(t: &mut impl Tool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = event(cx, x, y);
        t.pointer_down(cx, e)
    }

    fn mv(t: &mut impl Tool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = event(cx, x, y);
        t.pointer_move(cx, e)
    }

    fn up(t: &mut impl Tool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = event(cx, x, y);
        t.pointer_up(cx, e)
    }

    #[test]
    fn two_clicks_draw_a_construction_line_in_one_undo_step() {
        let mut cx = cx();
        cx.defaults.grid.snap = 0.0;
        let mut t = ConstructionLineTool::default();
        t.activate(&mut cx);
        down(&mut t, &mut cx, 0.0, 0.0);
        up(&mut t, &mut cx, 0.0, 0.0);
        let r = down(&mut t, &mut cx, 100.0, 0.0);
        assert_eq!(r.commit.as_deref(), Some("Construction Line"));
        let f = cx.floor();
        assert_eq!(f.construction.lines.len(), 1);
        let id = f.construction.lines[0].id;
        assert!(f.is_construction_line(id));
        assert!(cx.selection.contains(ObjectRef::Cad(id)));
        assert_eq!(cx.undo_label(), Some("Construction Line"));
        cx.undo();
        assert!(cx.floor().construction.is_empty());
        assert!(cx.floor().cad.is_empty());
    }

    #[test]
    fn a_drag_draws_the_line_and_a_stray_click_does_not() {
        let mut cx = cx();
        cx.defaults.grid.snap = 0.0;
        let mut t = ConstructionLineTool::default();
        t.activate(&mut cx);
        down(&mut t, &mut cx, 10.0, 10.0);
        up(&mut t, &mut cx, 10.0, 10.0);
        assert!(
            cx.floor().construction.is_empty(),
            "one point is not a line"
        );
        up(&mut t, &mut cx, 10.4, 10.0);
        assert!(cx.floor().construction.is_empty());
        // Esc drops the first point.
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(!t.key(&mut cx, KeyEvent::escape()).consumed, "then leaves");
        // A drag.
        down(&mut t, &mut cx, 0.0, 50.0);
        mv(&mut t, &mut cx, 60.0, 50.0);
        up(&mut t, &mut cx, 200.0, 50.0);
        assert_eq!(cx.floor().construction.lines.len(), 1);
    }

    #[test]
    fn a_second_line_snaps_to_where_two_construction_lines_cross() {
        let mut cx = cx();
        cx.defaults.grid.snap = 0.0;
        cx.defaults.grid.angle_snap_deg = 0.0;
        add_line(&mut cx, Point::new(0.0, 100.0), Point::new(10.0, 100.0)).unwrap();
        add_line(&mut cx, Point::new(500.0, 0.0), Point::new(500.0, 10.0)).unwrap();
        // Both are short stubs; the crossing is far past their ends.
        let near = cx.snap_at(Point::new(498.0, 101.5), None, false, &[]);
        assert_eq!(near.kind, crate::editor::snap::SnapKind::Intersection);
        assert!(
            near.point.dist(Point::new(500.0, 100.0)) < 1e-6,
            "{:?}",
            near.point
        );
    }

    #[test]
    fn a_wall_corner_snaps_onto_the_extension_of_a_construction_line() {
        let mut cx = cx();
        cx.defaults.grid.snap = 0.0;
        cx.defaults.grid.angle_snap_deg = 0.0;
        add_line(&mut cx, Point::new(0.0, 100.0), Point::new(10.0, 100.0)).unwrap();
        let on = cx.snap_at(Point::new(700.0, 101.5), None, false, &[]);
        assert_eq!(on.kind, crate::editor::snap::SnapKind::OnObject);
        assert!((on.point.y - 100.0).abs() < 1e-6 && (on.point.x - 700.0).abs() < 1e-6);
        // Far from the line nothing snaps to it.
        let far = cx.snap_at(Point::new(700.0, 300.0), None, false, &[]);
        assert_ne!(far.kind, crate::editor::snap::SnapKind::OnObject);
    }

    // ----- the offset tool -----

    fn with_other_plan(cx: &mut EditorContext, path: &str) {
        let mut other = plan_core::Project::new("existing");
        other.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(200.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        other.add_wall(
            0,
            Point::new(200.0, 0.0),
            Point::new(200.0, 100.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        ref_overlay::preload_other_plan(path, other);
        cx.project.reference_table.rows = vec![ReferenceRow::for_file(path)];
        cx.view_flags
            .insert(crate::toolbar::ViewFlag::ReferenceDisplay);
    }

    #[test]
    fn the_marquee_wraps_the_other_plan_and_follows_its_offset() {
        let mut cx = cx();
        with_other_plan(&mut cx, "/nonexistent/ref-a.psplan");
        let m = marquee(&cx, 0).unwrap();
        // The free end of the first wall starts at x = 0; the corner at (200, 0) is
        // 3 inches either side of the centerlines.
        assert!(m[0].dist(Point::new(0.0, -3.0)) < 1e-6, "{:?}", m[0]);
        assert!(m[2].dist(Point::new(203.0, 100.0)) < 1e-6, "{:?}", m[2]);
        cx.project.reference_table.rows[0].offset = [10.0, 20.0, 0.0];
        let m = marquee(&cx, 0).unwrap();
        assert!(m[0].dist(Point::new(10.0, 17.0)) < 1e-6);
        let (mv, rot) = handles(&m, 30.0);
        assert!(mv.dist(Point::lerp(m[0], m[2], 0.5)) < 1e-6);
        assert!(rot.y > m[3].y, "the Rotate handle sits past the top edge");
        // A row of this plan has no marquee.
        cx.project.reference_table.rows = vec![ReferenceRow::default()];
        assert!(marquee(&cx, 0).is_none());
    }

    #[test]
    fn dragging_the_marquee_moves_the_other_plan_in_one_undo_step() {
        let mut cx = cx();
        with_other_plan(&mut cx, "/nonexistent/ref-b.psplan");
        let mut t = ReferenceOffsetTool::default();
        t.activate(&mut cx);
        assert_eq!(t.row(), Some(0));
        down(&mut t, &mut cx, 100.0, 50.0);
        mv(&mut t, &mut cx, 130.0, 70.0);
        mv(&mut t, &mut cx, 160.0, 90.0);
        up(&mut t, &mut cx, 160.0, 90.0);
        let o = cx.project.reference_table.rows[0].offset;
        assert!(
            (o[0] - 60.0).abs() < 1e-9 && (o[1] - 40.0).abs() < 1e-9,
            "{o:?}"
        );
        assert_eq!(cx.undo_label(), Some("Edit Reference Document Offset"));
        cx.undo();
        assert_eq!(cx.project.reference_table.rows[0].offset, [0.0; 3]);
    }

    #[test]
    fn the_rotate_handle_turns_the_other_plan_about_the_marquee_center() {
        let mut cx = cx();
        with_other_plan(&mut cx, "/nonexistent/ref-c.psplan");
        let mut t = ReferenceOffsetTool::default();
        t.activate(&mut cx);
        let m = marquee(&cx, 0).unwrap();
        let center = Point::lerp(m[0], m[2], 0.5);
        let (_, rot) = handles(&m, ReferenceOffsetTool::reach(&cx));
        down(&mut t, &mut cx, rot.x, rot.y);
        // Swing the pointer a quarter turn about the center.
        let v = rot.sub(center);
        let turned = center.add(Point::new(-v.y, v.x));
        mv(&mut t, &mut cx, turned.x, turned.y);
        up(&mut t, &mut cx, turned.x, turned.y);
        let row = &cx.project.reference_table.rows[0];
        assert!(
            (row.angle_deg.rem_euclid(360.0) - 90.0).abs() < 1e-6,
            "{}",
            row.angle_deg
        );
        // The center of the marquee did not move.
        let after = marquee(&cx, 0).unwrap();
        assert!(Point::lerp(after[0], after[2], 0.5).dist(center) < 1e-6);
        assert_eq!(cx.undo_label(), Some("Edit Reference Document Offset"));
    }

    #[test]
    fn a_click_outside_the_marquee_does_nothing_and_no_file_row_leaves_the_tool() {
        let mut cx = cx();
        with_other_plan(&mut cx, "/nonexistent/ref-d.psplan");
        let mut t = ReferenceOffsetTool::default();
        t.activate(&mut cx);
        let steps = cx.can_undo();
        down(&mut t, &mut cx, 900.0, 900.0);
        up(&mut t, &mut cx, 900.0, 900.0);
        assert_eq!(cx.can_undo(), steps);
        // A click inside without moving leaves no step either.
        down(&mut t, &mut cx, 100.0, 50.0);
        up(&mut t, &mut cx, 100.0, 50.0);
        assert_eq!(cx.can_undo(), steps);
        // Without a file row the tool asks to leave.
        cx.project.reference_table.rows = vec![ReferenceRow::default()];
        cx.requests.clear();
        t.activate(&mut cx);
        assert!(matches!(
            cx.requests.last(),
            Some(EditorRequest::SetTool(ToolId::Select))
        ));
        assert!(t.row().is_none());
    }
}
