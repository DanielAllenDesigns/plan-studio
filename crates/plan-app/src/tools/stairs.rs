//! Stair tools (CB-22..CB-34 in `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! One tool object serves the whole Build > Stairs flyout; the variant is
//! chosen by `ToolId::StairsVariant(kind)` and read in `set_variant`:
//! Draw Stairs, Straight Stairs, L-Shaped, U-Shaped, Curve to Left/Right
//! (winders), Landing and Draw Ramp.
//!
//! * press-drag-release draws a stair: the drag direction is the direction of
//!   travel and the drag length the run; the number of risers is solved from
//!   the floor-to-floor rise (CB-23, CB-24). A plain click places a default
//!   stair pointing up the screen.
//! * a selected stair shows Move / Rotate / Run / Width handles (CB-28) that
//!   work while this tool is active; `stairs_view` offers the same logic to the
//!   Select tool.
//! * when the top of a new stair lands in a room of the floor above the status
//!   bar offers Auto Stairwell (CB-29, `stairs_view::auto_stairwell`).
//! * Tab flips the turn of L, U and curved stairs; Esc cancels; Delete removes
//!   the selected stairs.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::snap::snap_to_grid;
use crate::editor::stairs_view::{self as view, StairHandleKind, StairKind, StairObj};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, ObjectRef};
use eframe::egui::{self, Pos2, Shape, Stroke};
use plan_core::geometry::Point;
use plan_core::Id;
use plan_stairs::Turn;

/// Pixels the pointer must travel between press and release for a drag.
const DRAG_PX: f32 = 4.0;
/// Drags shorter than this (inches) are clicks.
const MIN_DRAG: f64 = 6.0;

struct Press {
    screen: Pos2,
    start: Point,
}

/// A handle drag of an existing stair.
struct EditDrag {
    id: Id,
    kind: StairHandleKind,
    orig: StairObj,
    start: Point,
}

pub struct StairsTool {
    kind: StairKind,
    turn: Turn,
    press: Option<Press>,
    hover: Option<Point>,
    edit: Option<EditDrag>,
}

impl Default for StairsTool {
    fn default() -> Self {
        Self::new(StairKind::Draw)
    }
}

impl StairsTool {
    pub fn new(kind: StairKind) -> Self {
        Self {
            kind,
            turn: Turn::Left,
            press: None,
            hover: None,
            edit: None,
        }
    }

    pub fn kind(&self) -> StairKind {
        self.kind
    }

    fn snap(&self, cx: &EditorContext, p: &PointerEvent, origin: Option<Point>) -> Point {
        cx.snap_at(p.world, origin, p.modifiers.alt, &[]).point
    }

    /// The origin for the angle snap of the drag end: a landing is dragged
    /// as a rectangle, so its diagonal gets no angle snap.
    fn angle_origin(&self, start: Point) -> Option<Point> {
        (self.kind != StairKind::Landing).then_some(start)
    }

    /// A sentence for the status bar and the live readout.
    fn describe(cx: &EditorContext, o: &StairObj) -> String {
        if let Some(d) = o.x.landing_depth {
            return format!(
                "Landing: {} x {}",
                cx.fmt_dim(o.stair.params.width),
                cx.fmt_dim(d)
            );
        }
        let sol = o.solution();
        if o.is_ramp() {
            let slope = sol.total_run / o.stair.params.total_rise.max(1.0);
            return format!(
                "Ramp: run {}, rise {}, slope 1:{slope:.1}",
                cx.fmt_dim(sol.total_run),
                cx.fmt_dim(o.stair.params.total_rise)
            );
        }
        format!(
            "Stairs: {} risers at {}, {} treads at {}, run {}",
            sol.risers,
            cx.fmt_dim(sol.riser_height),
            sol.treads,
            cx.fmt_dim(sol.tread_depth),
            cx.fmt_dim(sol.total_run)
        )
    }

    fn place(&mut self, cx: &mut EditorContext, a: Point, b: Option<Point>) -> ToolResult {
        let obj = view::build(&cx.project, cx.floor, self.kind, self.turn, a, b);
        cx.begin_change(self.kind.name());
        let id = view::add(&mut cx.project, cx.floor, obj);
        cx.selection.set(ObjectRef::Stair(id));
        cx.mark_dirty();
        cx.readout = None;
        if let Some(o) = view::find(cx.floor(), id) {
            let mut s = Self::describe(cx, &o);
            if !o.is_landing() {
                if let Some(w) = o.solution().warnings.first() {
                    s.push_str(&format!(" ({w})"));
                }
            }
            if view::lands_in_room_above(&cx.project, cx.floor, &o) {
                s.push_str(
                    ". The stair lands in a room on the floor above: Auto Stairwell (Edit toolbar) cuts the stairwell",
                );
            }
            cx.status = s;
        }
        ToolResult::committed(self.kind.name())
    }

    fn update_readout(&self, cx: &mut EditorContext, a: Point, b: Point) {
        let o = view::build(&cx.project, cx.floor, self.kind, self.turn, a, Some(b));
        cx.readout = Some(Self::describe(cx, &o));
    }

    /// Applies the current pointer position to the handle drag.
    fn drag_edit(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(e) = &self.edit else {
            return ToolResult::ignored();
        };
        let to = match e.kind {
            StairHandleKind::Rotate | StairHandleKind::Run => {
                self.snap(cx, p, Some(e.orig.bottom_center()))
            }
            _ => p.world,
        };
        let mut n = view::drag_handle(&e.orig, e.kind, e.start, to);
        if e.kind == StairHandleKind::Move && !p.modifiers.alt {
            n.stair.origin = snap_to_grid(n.stair.origin, cx.snap_unit());
        }
        let (id, fl) = (e.id, cx.floor);
        view::update(&mut cx.project, fl, id, |o| *o = n.clone());
        cx.mark_dirty();
        cx.readout = Some(Self::describe(cx, &n));
        ToolResult::consumed()
    }
}

impl Tool for StairsTool {
    fn id(&self) -> ToolId {
        ToolId::Stairs
    }

    fn name(&self) -> &'static str {
        self.kind.name()
    }

    fn hint(&self) -> String {
        self.kind.hint().to_string()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::StairsVariant(k) = id {
            self.kind = k;
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = self.kind.hint().to_string();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.press = None;
        self.hover = None;
        self.edit = None;
        cx.readout = None;
        cx.last_snap = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.edit.is_some() {
            return self.drag_edit(cx, &p);
        }
        if let Some(press) = &self.press {
            let start = press.start;
            let end = self.snap(cx, &p, self.angle_origin(start));
            self.hover = Some(end);
            if start.dist(end) >= MIN_DRAG {
                self.update_readout(cx, start, end);
            }
        } else {
            self.hover = Some(p.snapped);
        }
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(o) = view::selected_stair(cx) {
            let hs = view::handles(&o, cx.px_per_in);
            if let Some(h) = view::hit_handle(&hs, p.world, cx.pick_tol()) {
                cx.begin_change(view::drag_label(h.kind));
                self.edit = Some(EditDrag {
                    id: o.id(),
                    kind: h.kind,
                    orig: o,
                    start: p.world,
                });
                return ToolResult::consumed();
            }
        }
        let start = self.snap(cx, &p, None);
        self.press = Some(Press {
            screen: p.screen,
            start,
        });
        self.hover = Some(start);
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(e) = self.edit.take() {
            cx.readout = None;
            return if view::find(cx.floor(), e.id).as_ref() == Some(&e.orig) {
                cx.cancel_change();
                ToolResult::consumed()
            } else {
                ToolResult::committed(view::drag_label(e.kind))
            };
        }
        let Some(press) = self.press.take() else {
            return ToolResult::ignored();
        };
        let end = self.snap(cx, &p, self.angle_origin(press.start));
        let dragged =
            (p.screen - press.screen).length() >= DRAG_PX && press.start.dist(end) >= MIN_DRAG;
        self.place(cx, press.start, dragged.then_some(end))
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(egui::Key::Escape) {
            if let Some(e) = self.edit.take() {
                let (id, fl, orig) = (e.id, cx.floor, e.orig);
                view::update(&mut cx.project, fl, id, |o| *o = orig);
                cx.cancel_change();
                cx.mark_dirty();
                cx.readout = None;
                return ToolResult::consumed();
            }
            if self.press.take().is_some() {
                cx.readout = None;
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(egui::Key::Delete) || k.is(egui::Key::Backspace) {
            if view::delete_selected(cx) > 0 {
                return ToolResult::committed("Delete Stairs");
            }
            return ToolResult::ignored();
        }
        if k.is(egui::Key::Tab)
            && matches!(
                self.kind,
                StairKind::LShaped
                    | StairKind::UShaped
                    | StairKind::CurveLeft
                    | StairKind::CurveRight
            )
        {
            self.turn = match self.turn {
                Turn::Left => Turn::Right,
                Turn::Right => Turn::Left,
            };
            cx.status = format!("Turn to the {:?}", self.turn).to_lowercase();
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        if self.edit.is_some() {
            return;
        }
        let Some(h) = self.hover else { return };
        let pal = &cx.palette;
        let (a, b) = match &self.press {
            Some(press) => (
                press.start,
                Some(h).filter(|h| press.start.dist(*h) >= MIN_DRAG),
            ),
            None => (h, None),
        };
        let ghost = view::build(&cx.project, cx.floor, self.kind, self.turn, a, b);
        let pts = ghost
            .footprint()
            .iter()
            .map(|p| cam.world_to_screen(*p))
            .collect();
        painter.add(Shape::closed_line(
            pts,
            Stroke::new(1.0_f32, pal.ghost_stroke),
        ));
        view::draw_strokes(
            painter,
            cam,
            &view::symbol_strokes(&ghost),
            pal.ghost_stroke,
            1.0,
            false,
        );
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        if view::selected_stair(cx).is_none() {
            return Vec::new();
        }
        let mut v = vec![
            EditAction::new(EditActionKind::OpenObject),
            EditAction::new(EditActionKind::Delete),
        ];
        v.extend(cx.extra_edit_actions());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::stairs_view::{StairCommand, StairHandleKind};
    use crate::plan_defaults;
    use plan_core::{detect_rooms, Floor, WallKind};
    use plan_stairs::{solve, StairShape, Stroke as PlanStroke};

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn pe(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(cx, Point::new(x, y))
    }

    fn drag(t: &mut StairsTool, cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) {
        let pa = pe(cx, a.0, a.1);
        let pb = pe(cx, b.0, b.1);
        t.pointer_move(cx, pa);
        t.pointer_down(cx, pa.with_down(true));
        t.pointer_move(cx, pb.with_down(true));
        t.pointer_up(cx, pb);
    }

    fn only_stair(cx: &EditorContext) -> StairObj {
        let all = view::load(cx.floor());
        assert_eq!(all.len(), 1, "expected exactly one stair");
        all.into_iter().next().unwrap()
    }

    fn add_floor_above(cx: &mut EditorContext) {
        cx.project.floors.push(Floor::new("2nd Floor", 121.25));
    }

    fn room_walls(cx: &mut EditorContext, fl: usize, lo: (f64, f64), hi: (f64, f64)) {
        let c = [
            Point::new(lo.0, lo.1),
            Point::new(hi.0, lo.1),
            Point::new(hi.0, hi.1),
            Point::new(lo.0, hi.1),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(fl, c[i], c[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
        }
    }

    #[test]
    fn drag_draws_a_stair_with_the_dragged_run_and_solved_risers() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        let sol = o.solution();
        // 109 1/8 ceiling + 12 1/8 platform = 121 1/4 rise with no floor above.
        assert!((o.stair.params.total_rise - 121.25).abs() < 1e-9);
        assert_eq!(sol.risers, 16);
        assert_eq!(sol.treads, 15);
        assert!((sol.total_run - 150.0).abs() < 1.0, "run {}", sol.total_run);
        assert!(
            o.stair.direction.abs() < 1e-9,
            "drag direction is the travel direction"
        );
        assert_eq!(o.stair.params.width, 36.0);
        // The drag starts at the centre of the bottom riser.
        assert!(o.bottom_center().dist(Point::new(0.0, 0.0)) < 1e-9);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Stair(o.id())));
        assert!(cx.status.starts_with("Stairs: 16 risers"), "{}", cx.status);
    }

    #[test]
    fn undo_removes_the_stair() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (0.0, 150.0));
        assert_eq!(view::load(cx.floor()).len(), 1);
        assert_eq!(cx.undo().as_deref(), Some("Draw Stairs"));
        assert!(view::load(cx.floor()).is_empty());
        cx.redo();
        assert_eq!(view::load(cx.floor()).len(), 1);
    }

    #[test]
    fn rise_comes_from_the_floor_above_when_there_is_one() {
        let mut cx = new_cx();
        cx.project.floors.push(Floor::new("2nd Floor", 100.0));
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (130.0, 0.0));
        let o = only_stair(&cx);
        assert!((o.stair.params.total_rise - 100.0).abs() < 1e-9);
        assert_eq!(o.solution().risers, 13);
        assert_eq!(o.stair.floor_elevation, 0.0);
    }

    #[test]
    fn a_click_places_a_default_straight_stair() {
        let mut cx = new_cx();
        let mut t = StairsTool::new(StairKind::Straight);
        let p = pe(&cx, 60.0, 40.0);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        let o = only_stair(&cx);
        assert!((o.stair.direction - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert_eq!(o.stair.params.tread_depth, 10.0);
        assert!(o.bottom_center().dist(Point::new(60.0, 40.0)) < 1e-9);
    }

    #[test]
    fn the_symbol_has_the_break_line_at_two_thirds_and_the_up_arrow() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        let strokes = view::symbol_strokes(&o);
        assert!(strokes
            .iter()
            .any(|s| matches!(s, PlanStroke::Polyline(p, false) if p.len() == 6)));
        assert!(strokes
            .iter()
            .any(|s| matches!(s, PlanStroke::Text { text, .. } if text == "UP")));
        // The cut is at 2/3 of the run: nothing is drawn beyond x = 100.
        let max_x = strokes
            .iter()
            .filter_map(|s| match s {
                PlanStroke::Line(a, b) => Some(a.x.max(b.x)),
                _ => None,
            })
            .fold(f64::MIN, f64::max);
        assert!(max_x < 101.0, "risers stop at the break line, got {max_x}");
        // Remove Breakline draws the full run.
        let mut o2 = o.clone();
        o2.x.break_line = false;
        assert!(!view::symbol_strokes(&o2)
            .iter()
            .any(|s| matches!(s, PlanStroke::Polyline(p, false) if p.len() == 6)));
    }

    #[test]
    fn the_floor_above_shows_the_top_portion_with_dn() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        let up = view::upper_strokes(&o);
        assert!(up
            .iter()
            .any(|s| matches!(s, PlanStroke::Text { text, .. } if text == "DN")));
        assert!(!up
            .iter()
            .any(|s| matches!(s, PlanStroke::Text { text, .. } if text == "UP")));
        // Risers beyond the break line are there, the ones below are not.
        let xs: Vec<f64> = up
            .iter()
            .filter_map(|s| match s {
                PlanStroke::Line(a, b) if (a.x - b.x).abs() < 1e-9 => Some(a.x),
                _ => None,
            })
            .collect();
        assert!(!xs.is_empty());
        assert!(xs.iter().all(|x| *x >= 100.0 - 1e-9), "{xs:?}");
    }

    #[test]
    fn auto_stairwell_adds_four_invisible_divider_walls_forming_the_footprint_room() {
        let mut cx = new_cx();
        add_floor_above(&mut cx);
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        let n = view::run_command(&mut cx, StairCommand::AutoStairwell);
        assert!(n, "{}", cx.status);
        let above = &cx.project.floors[1];
        assert_eq!(above.walls.len(), 4);
        assert!(above
            .walls
            .iter()
            .all(|w| w.flags.room_divider && w.flags.invisible));
        let rooms = detect_rooms(&above.walls, 0.5);
        assert_eq!(rooms.len(), 1);
        assert!((rooms[0].area_sq_in - view::footprint_area(&o)).abs() < 1e-6);
        assert!((rooms[0].area_sq_in - 150.0 * 36.0).abs() < 1e-6);
        assert_eq!(above.room_names.len(), 1);
        assert_eq!(above.room_names[0].name, "Stairwell");
        // A second Auto Stairwell is refused; undo removes the walls.
        assert!(!view::run_command(&mut cx, StairCommand::AutoStairwell));
        assert_eq!(cx.project.floors[1].walls.len(), 4);
        assert_eq!(cx.undo().as_deref(), Some("Auto Stairwell"));
        assert!(cx.project.floors[1].walls.is_empty());
    }

    #[test]
    fn auto_stairwell_needs_a_floor_above() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        assert!(!view::run_command(&mut cx, StairCommand::AutoStairwell));
        assert!(cx.status.contains("no floor above"));
        assert!(view::edit_commands(&cx)
            .iter()
            .any(|(c, on)| *c == StairCommand::AutoStairwell && !*on));
    }

    #[test]
    fn landing_in_a_room_above_offers_the_stairwell() {
        let mut cx = new_cx();
        add_floor_above(&mut cx);
        room_walls(&mut cx, 1, (-100.0, -200.0), (400.0, 200.0));
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        assert!(cx.status.contains("Auto Stairwell"), "{}", cx.status);
        // A stair that ends outside any room does not.
        let mut cx2 = new_cx();
        add_floor_above(&mut cx2);
        room_walls(&mut cx2, 1, (-100.0, -200.0), (100.0, 200.0));
        let mut t2 = StairsTool::default();
        drag(&mut t2, &mut cx2, (0.0, 0.0), (150.0, 0.0));
        assert!(!cx2.status.contains("Auto Stairwell"), "{}", cx2.status);
    }

    #[test]
    fn the_rotate_handle_changes_the_direction_about_the_bottom() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        let pivot = o.bottom_center();
        let h = view::handles(&o, cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == StairHandleKind::Rotate)
            .unwrap();
        // Behind the bottom of the run.
        assert!(h.pos.x < 0.0);
        let from = pe(&cx, h.pos.x, h.pos.y);
        let to = pe(&cx, 0.0, -80.0);
        t.pointer_down(&mut cx, from.with_down(true));
        t.pointer_move(&mut cx, to.with_down(true));
        let r = t.pointer_up(&mut cx, to);
        assert_eq!(r.commit.as_deref(), Some("Rotate Stairs"));
        let n = only_stair(&cx);
        assert!(
            (n.stair.direction - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
            "direction {}",
            n.stair.direction
        );
        assert!(n.bottom_center().dist(pivot) < 1e-6, "pivot stays");
        assert_eq!(cx.undo().as_deref(), Some("Rotate Stairs"));
        assert!(only_stair(&cx).stair.direction.abs() < 1e-9);
    }

    #[test]
    fn run_and_width_handles_resize() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        let hs = view::handles(&o, cx.px_per_in);
        let pos = |k| hs.iter().find(|h| h.kind == k).unwrap().pos;
        // Run: top end from x = 150 to x = 180 -> tread depth 12.
        let run = pos(StairHandleKind::Run);
        assert!((run.x - 150.0).abs() < 1e-9);
        let (a, b) = (pe(&cx, run.x, run.y), pe(&cx, 180.0, 0.0));
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        let n = only_stair(&cx);
        assert!((n.stair.params.tread_depth - 12.0).abs() < 1e-6);
        assert_eq!(n.solution().risers, 16, "the rise fixes the risers");
        // Width: drag the right side (y = -36 for travel along +x) out by 12".
        let o = only_stair(&cx);
        let hs = view::handles(&o, cx.px_per_in);
        let right = hs
            .iter()
            .find(|h| h.kind == StairHandleKind::WidthRight)
            .unwrap()
            .pos;
        assert!((right.y - -18.0).abs() < 1e-9, "{right:?}");
        let (a, b) = (pe(&cx, right.x, right.y), pe(&cx, right.x, -30.0));
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        let n = only_stair(&cx);
        assert!(
            (n.stair.params.width - 48.0).abs() < 1e-6,
            "{}",
            n.stair.params.width
        );
        // Left side keeps the right edge fixed.
        let left = view::handles(&n, cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == StairHandleKind::WidthLeft)
            .unwrap()
            .pos;
        let right_edge = n.stair.origin + n.right() * n.stair.params.width;
        // Dragging the left handle out (toward +y) widens the stair by 12".
        let (a, b) = (pe(&cx, left.x, left.y), pe(&cx, left.x, left.y + 12.0));
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        let m = only_stair(&cx);
        assert!((m.stair.params.width - 60.0).abs() < 1e-6);
        assert!((m.stair.origin + m.right() * m.stair.params.width).dist(right_edge) < 1e-6);
    }

    #[test]
    fn move_handle_moves_and_escape_cancels() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        let c = view::handles(&o, cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == StairHandleKind::Move)
            .unwrap()
            .pos;
        let a = pe(&cx, c.x, c.y);
        let b = pe(&cx, c.x + 40.0, c.y + 20.0);
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        assert!((only_stair(&cx).stair.origin.x - (o.stair.origin.x + 40.0)).abs() < 1e-6);
        t.key(&mut cx, KeyEvent::escape());
        assert_eq!(only_stair(&cx), o);
    }

    #[test]
    fn delete_removes_the_selected_stair_and_undoes() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let r = t.key(&mut cx, KeyEvent::key(egui::Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete Stairs"));
        assert!(view::load(cx.floor()).is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Delete Stairs"));
        assert_eq!(view::load(cx.floor()).len(), 1);
    }

    #[test]
    fn l_u_curve_ramp_and_landing_variants() {
        let mut cx = new_cx();
        let mut l = StairsTool::new(StairKind::LShaped);
        drag(&mut l, &mut cx, (0.0, 0.0), (70.0, 0.0));
        let o = view::load(cx.floor()).pop().unwrap();
        assert_eq!(
            o.stair.params.shape,
            StairShape::LShaped {
                treads_before_landing: 7
            }
        );
        assert!(solve(&o.stair.params).risers == 16);

        let mut u = StairsTool::new(StairKind::UShaped);
        u.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        drag(&mut u, &mut cx, (300.0, 0.0), (300.0, 100.0));
        let o = view::load(cx.floor()).pop().unwrap();
        assert!(matches!(o.stair.params.shape, StairShape::UShaped { .. }));
        assert_eq!(o.stair.params.turn, Turn::Right);

        let mut c = StairsTool::new(StairKind::CurveRight);
        let p = pe(&cx, 600.0, 0.0);
        c.pointer_down(&mut cx, p.with_down(true));
        c.pointer_up(&mut cx, p);
        let o = view::load(cx.floor()).pop().unwrap();
        assert_eq!(o.stair.params.shape, StairShape::Winder { winders: 3 });
        assert_eq!(o.stair.params.turn, Turn::Right);

        let mut r = StairsTool::new(StairKind::Ramp);
        drag(&mut r, &mut cx, (0.0, 300.0), (360.0, 300.0));
        let o = view::load(cx.floor()).pop().unwrap();
        match o.stair.params.shape {
            StairShape::Ramp { slope_1_in } => assert!((slope_1_in - 12.0).abs() < 1e-6),
            s => panic!("{s:?}"),
        }
        assert_eq!(o.stair.params.total_rise, 30.0);
        assert!(cx.status.starts_with("Ramp"));

        let mut g = StairsTool::new(StairKind::Landing);
        drag(&mut g, &mut cx, (100.0, 400.0), (172.0, 440.0));
        let o = view::load(cx.floor()).pop().unwrap();
        assert!(o.is_landing());
        assert_eq!(o.x.landing_depth, Some(72.0));
        assert_eq!(o.stair.params.width, 40.0);
        let fp = o.footprint();
        let (lo_x, hi_x) = fp
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (lo_y, hi_y) = fp
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        assert_eq!((lo_x, hi_x, lo_y, hi_y), (100.0, 172.0, 400.0, 440.0));
        assert_eq!(
            view::pick(cx.floor(), Point::new(130.0, 420.0), 1.0),
            Some(o.id())
        );
    }

    #[test]
    fn flare_curve_toggles_winders_and_breakline_toggles() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        assert!(view::run_command(&mut cx, StairCommand::FlareCurve));
        assert!(matches!(
            only_stair(&cx).stair.params.shape,
            StairShape::Winder { .. }
        ));
        assert!(view::run_command(&mut cx, StairCommand::FlareCurve));
        assert!(matches!(
            only_stair(&cx).stair.params.shape,
            StairShape::LShaped { .. }
        ));
        assert!(view::run_command(&mut cx, StairCommand::ToggleBreakLine));
        assert!(!only_stair(&cx).x.break_line);
        assert!(view::run_command(&mut cx, StairCommand::MakeRailing));
        let o = only_stair(&cx);
        assert!(o.x.railing_left && o.x.railing_right);
    }

    #[test]
    fn storage_keeps_extras_and_foreign_values() {
        let mut cx = new_cx();
        cx.floor_mut()
            .stairs
            .push(serde_json::json!({"not": "a stair"}));
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let mut o = view::load(cx.floor()).pop().unwrap();
        o.x.label = "Main".into();
        o.x.show_label = true;
        o.x.materials[0].1 = "Maple".into();
        assert!(view::apply_edit(&mut cx, &o));
        let back = view::load(cx.floor()).pop().unwrap();
        assert_eq!(back, o);
        assert_eq!(cx.floor().stairs.len(), 2, "the foreign value survives");
        // plan-stairs reads the same JSON.
        let plain: plan_stairs::Stair =
            serde_json::from_value(cx.floor().stairs[1].clone()).unwrap();
        assert_eq!(plain, o.stair);
    }

    #[test]
    fn the_elevation_preview_matches_the_solution() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        let pts = view::elevation_points(&o);
        let top = pts.iter().map(|p| p.1).fold(0.0, f64::max);
        assert!((top - 121.25).abs() < 1e-6);
        // 16 risers: 16 vertical segments.
        let risers = pts
            .windows(2)
            .filter(|w| (w[0].0 - w[1].0).abs() < 1e-9 && w[1].1 > w[0].1)
            .count();
        assert_eq!(risers, 16);
    }

    #[test]
    fn variant_ids_select_the_stair_tool() {
        let mut t = StairsTool::new(StairKind::Ramp);
        for k in StairKind::ALL {
            t.set_variant(ToolId::StairsVariant(k));
            assert_eq!(t.kind(), k);
        }
        // The plain id leaves the picked kind alone.
        t.set_variant(ToolId::Stairs);
        assert_eq!(t.kind(), StairKind::ALL[StairKind::ALL.len() - 1]);
    }

    #[test]
    fn edit_toolbar_offers_open_and_delete_for_a_selected_stair() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        assert!(t.edit_toolbar(&cx).is_empty());
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let kinds: Vec<_> = t.edit_toolbar(&cx).iter().map(|a| a.kind).collect();
        assert_eq!(
            kinds[..2],
            [EditActionKind::OpenObject, EditActionKind::Delete]
        );
        // The stair commands follow (Auto Stairwell, Flare/Curve, ...).
        assert!(kinds.iter().any(|k| matches!(
            k,
            EditActionKind::Custom {
                id: "stair.flare_curve",
                ..
            }
        )));

        cx.apply_edit_action(EditActionKind::OpenObject);
        assert!(cx.requests.iter().any(|r| matches!(
            r,
            crate::editor::EditorRequest::OpenSpec(ObjectRef::Stair(_))
        )));
    }
}
