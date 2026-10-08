//! Stair tools (CB-22..CB-34 in `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! One tool object serves the whole Build > Stairs flyout; the variant is
//! chosen by `ToolId::StairsVariant(kind)` and read in `set_variant`:
//! Draw Stairs, Click Stairs, Straight Stairs, L-Shaped, U-Shaped, Curve to
//! Left/Right (winders), Curved Stairs, Landing and Draw Ramp.
//!
//! * press-drag-release draws a stair: the drag direction is the direction of
//!   travel and the drag length the run; the number of risers is solved from
//!   the floor-to-floor rise (CB-23, CB-24). A plain click places a default
//!   stair pointing up the screen.
//! * Click Stairs: one click places a straight stair of default length
//!   (the solved number of 10" treads) pointing the way the pointer was
//!   heading when it arrived.
//! * Curved Stairs: press at the centre, drag to the walking radius; the
//!   stair starts at the drag end and turns left (Tab flips it to the right).
//! * Landing: drag a rectangle, or click the corners of a polygon and
//!   double-click the last (Enter also ends it, Esc cancels, Backspace drops
//!   the last corner); a double-click on its own places a 3' square. A landing
//!   that touches a stair section takes its height, and a section that starts
//!   on it begins there (CB-27, `stairs_view::connect`).
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
    /// Direction the pointer was last moving (Click Stairs).
    heading: Option<f64>,
    /// Corners of the polygon landing being drawn.
    corners: Vec<Point>,
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
            heading: None,
            corners: Vec::new(),
        }
    }

    pub fn kind(&self) -> StairKind {
        self.kind
    }

    /// Corners of the polygon landing in progress (tests).
    pub fn corners(&self) -> &[Point] {
        &self.corners
    }

    /// The drag end a click stands for: Click Stairs reach their default run
    /// toward the way the pointer was heading.
    fn click_end(&self, cx: &EditorContext, a: Point) -> Option<Point> {
        (self.kind == StairKind::Click).then(|| {
            let dir = self.heading.unwrap_or(std::f64::consts::FRAC_PI_2);
            let run = view::click_run(&cx.project, cx.floor);
            Point::new(a.x + dir.cos() * run, a.y + dir.sin() * run)
        })
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
        if let Some(d) = o.landing_depth() {
            if o.is_polygon_landing() {
                return format!(
                    "Landing: {} corners, {} sq ft",
                    o.stair.params.outline.len(),
                    (view::footprint_area(o) / 144.0).round()
                );
            }
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
        let name = if o.is_curved() {
            "Curved stairs"
        } else {
            "Stairs"
        };
        format!(
            "{name}: {} risers at {}, {} treads at {}, run {}",
            sol.risers,
            cx.fmt_dim(sol.riser_height),
            sol.treads,
            cx.fmt_dim(sol.tread_depth),
            cx.fmt_dim(sol.total_run)
        )
    }

    fn place(&mut self, cx: &mut EditorContext, a: Point, b: Option<Point>) -> ToolResult {
        let b = b.or_else(|| self.click_end(cx, a));
        let obj = view::build(&cx.project, cx.floor, self.kind, self.turn, a, b);
        self.add_object(cx, obj, self.kind.name())
    }

    /// Adds `obj` as one undo step named `label`, joins it to the landings
    /// and stairs it touches and tells the status bar what it is.
    fn add_object(&mut self, cx: &mut EditorContext, obj: StairObj, label: &str) -> ToolResult {
        cx.begin_change(label);
        let id = view::add(&mut cx.project, cx.floor, obj);
        let joined = view::connect(&mut cx.project, cx.floor, id);
        cx.selection.set(ObjectRef::Stair(id));
        cx.mark_dirty();
        cx.readout = None;
        if let Some(o) = view::find(cx.floor(), id) {
            let mut s = Self::describe(cx, &o);
            if joined > 0 {
                s.push_str(&format!(
                    " (joined {joined} {})",
                    if joined == 1 { "section" } else { "sections" }
                ));
            }
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
        ToolResult::committed(label)
    }

    /// Closes the polygon landing in progress.
    fn finish_landing(&mut self, cx: &mut EditorContext) -> ToolResult {
        let mut pts = std::mem::take(&mut self.corners);
        // The double-click's first press already added the last corner.
        while pts.len() > 1 && pts[pts.len() - 1].dist(pts[pts.len() - 2]) < 0.5 {
            pts.pop();
        }
        cx.readout = None;
        let area = plan_core::geometry::polygon_area(&pts).abs();
        if pts.len() < 3 || area < 1.0 {
            cx.status = "Landing: needs three corners that enclose an area".into();
            return ToolResult::consumed();
        }
        let obj = view::build_polygon_landing(&cx.project, cx.floor, &pts);
        self.add_object(cx, obj, "Landing")
    }

    /// A click of the Landing tool: another corner (a click on the first
    /// corner closes the polygon).
    fn landing_click(&mut self, cx: &mut EditorContext, at: Point) -> ToolResult {
        if self.corners.len() >= 3 && at.dist(self.corners[0]) <= cx.pick_tol() {
            return self.finish_landing(cx);
        }
        if self.corners.last().is_some_and(|l| l.dist(at) < 0.5) {
            return ToolResult::consumed();
        }
        self.corners.push(at);
        cx.status = format!(
            "Landing: {} corner(s); double-click the last corner (or Enter) to finish",
            self.corners.len()
        );
        ToolResult::consumed()
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
        self.corners.clear();
        self.heading = None;
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
            if let Some(prev) = self.hover {
                let d = p.snapped - prev;
                if d.length() >= 2.0 {
                    self.heading = Some(d.angle());
                }
            }
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
                let fl = cx.floor;
                view::connect(&mut cx.project, fl, e.id);
                ToolResult::committed(view::drag_label(e.kind))
            };
        }
        let Some(press) = self.press.take() else {
            return ToolResult::ignored();
        };
        let end = self.snap(cx, &p, self.angle_origin(press.start));
        let dragged =
            (p.screen - press.screen).length() >= DRAG_PX && press.start.dist(end) >= MIN_DRAG;
        if self.kind == StairKind::Landing && (!dragged || !self.corners.is_empty()) {
            return self.landing_click(cx, press.start);
        }
        self.place(cx, press.start, dragged.then_some(end))
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.kind != StairKind::Landing {
            return ToolResult::ignored();
        }
        let at = cx.snap_at(p.world, None, p.modifiers.alt, &[]).point;
        match self.corners.len() {
            0 | 1 => {
                // A double-click on its own places a default square landing.
                let a = self.corners.first().copied().unwrap_or(at);
                self.corners.clear();
                self.press = None;
                self.place(cx, a, None)
            }
            2 => {
                if self.corners.last().is_some_and(|l| l.dist(at) >= 0.5) {
                    self.corners.push(at);
                }
                self.finish_landing(cx)
            }
            _ => self.finish_landing(cx),
        }
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
            if !self.corners.is_empty() {
                self.corners.clear();
                cx.readout = None;
                return ToolResult::consumed();
            }
            if self.press.take().is_some() {
                cx.readout = None;
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(egui::Key::Enter) && !self.corners.is_empty() {
            return self.finish_landing(cx);
        }
        if k.is(egui::Key::Backspace) && !self.corners.is_empty() {
            self.corners.pop();
            return ToolResult::consumed();
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
                    | StairKind::Curved
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
        if !self.corners.is_empty() {
            // The polygon landing so far, with the next edge to the pointer.
            let mut pts: Vec<Pos2> = self
                .corners
                .iter()
                .map(|p| cam.world_to_screen(*p))
                .collect();
            pts.push(cam.world_to_screen(h));
            painter.add(Shape::line(pts, Stroke::new(1.5_f32, pal.ghost_stroke)));
            return;
        }
        let (a, b) = match &self.press {
            Some(press) => (
                press.start,
                Some(h).filter(|h| press.start.dist(*h) >= MIN_DRAG),
            ),
            None => (h, None),
        };
        let b = b.or_else(|| self.click_end(cx, a));
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
    fn the_treads_beyond_the_break_line_are_drawn_hidden_on_the_stairs_own_floor() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let mut o = only_stair(&cx);
        let hidden = view::hidden_strokes(&o);
        assert!(!hidden.is_empty());
        // Lines only, all past the break, none of them in the solid symbol.
        let solid = view::symbol_strokes(&o);
        for s in &hidden {
            let PlanStroke::Line(a, b) = s else {
                panic!("only treads: {s:?}");
            };
            assert!(a.x.min(b.x) >= 100.0 - 1e-9);
            assert!(!solid.contains(s));
        }
        // They are what the floor above shows of the same run.
        let up = view::upper_strokes(&o);
        assert!(hidden.iter().all(|s| up.contains(s)));
        // No break line, no hidden part; a landing never has one.
        o.x.break_line = false;
        assert!(view::hidden_strokes(&o).is_empty());
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
        // The room is the footprint grown by the 0.3" margin that keeps the
        // hole strictly inside it.
        let grown = (150.0 + 0.6) * (36.0 + 0.6);
        assert!(
            (rooms[0].area_sq_in - grown).abs() < 1e-6,
            "{}",
            rooms[0].area_sq_in
        );
        assert!(rooms[0].area_sq_in > view::footprint_area(&o));
        assert_eq!(above.room_names.len(), 1);
        assert_eq!(above.room_names[0].name, "Stairwell");
        assert!(!above.room_names[0].has_floor, "open below");
        // A second Auto Stairwell is refused; undo removes the walls.
        assert!(!view::run_command(&mut cx, StairCommand::AutoStairwell));
        assert_eq!(cx.project.floors[1].walls.len(), 4);
        assert_eq!(cx.undo().as_deref(), Some("Auto Stairwell"));
        assert!(cx.project.floors[1].walls.is_empty());
    }

    #[test]
    fn a_guard_railing_runs_around_the_opening_except_where_the_stair_arrives() {
        let mut cx = new_cx();
        add_floor_above(&mut cx);
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let id = only_stair(&cx).id();
        assert!(view::run_command(&mut cx, StairCommand::AutoStairwell));
        let rails = |cx: &EditorContext| {
            cx.project.floors[1]
                .walls
                .iter()
                .filter(|w| w.flags.railing)
                .cloned()
                .collect::<Vec<_>>()
        };
        assert!(rails(&cx).is_empty(), "off until asked for");
        // Switching the guard on (the dialog's checkbox) adds three railings:
        // both long sides and the foot of the stair, not the arrival end.
        let mut o = only_stair(&cx);
        o.x.stairwell_guard = true;
        assert!(view::apply_edit(&mut cx, &o));
        let r = rails(&cx);
        assert_eq!(r.len(), 3, "{r:?}");
        assert!(r.iter().all(|w| w.height == view::GUARD_HEIGHT));
        // The stair runs along +x and arrives at x = 150: no railing there.
        assert!(r.iter().all(|w| !(w.start.x > 149.0 && w.end.x > 149.0)));
        // The rails are remembered with the stair and follow it when it moves.
        let o = only_stair(&cx);
        assert_eq!(o.x.guard_walls.len(), 3);
        view::update(&mut cx.project, 0, id, |o| {
            o.stair.origin = o.stair.origin + Point::new(0.0, 60.0);
        });
        let moved = rails(&cx);
        assert_eq!(moved.len(), 3, "rebuilt, not piled up");
        assert!(moved.iter().all(|w| w.start.y > 30.0));
        // Off again removes them; so does deleting the stair.
        let mut o = only_stair(&cx);
        o.x.stairwell_guard = false;
        view::apply_edit(&mut cx, &o);
        assert!(rails(&cx).is_empty());
        let mut o = only_stair(&cx);
        o.x.stairwell_guard = true;
        view::apply_edit(&mut cx, &o);
        assert_eq!(rails(&cx).len(), 3);
        cx.selection.set(ObjectRef::Stair(id));
        view::delete_selected(&mut cx);
        assert!(rails(&cx).is_empty());
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
        assert_eq!(o.landing_depth(), Some(72.0));
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
        assert_eq!(o.stair.params.left_side, plan_stairs::SideKind::Railing);
        assert_eq!(o.stair.params.right_side, plan_stairs::SideKind::Railing);
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

    fn click(t: &mut StairsTool, cx: &mut EditorContext, x: f64, y: f64) {
        let p = pe(cx, x, y);
        t.pointer_move(cx, p);
        t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
    }

    fn all(cx: &EditorContext) -> Vec<StairObj> {
        view::load(cx.floor())
    }

    #[test]
    fn click_stairs_places_a_default_stair_toward_the_pointer() {
        let mut cx = new_cx();
        let mut t = StairsTool::new(StairKind::Click);
        // The pointer comes in from the left, heading east, and clicks.
        for x in [0.0, 24.0, 48.0, 72.0] {
            let p = pe(&cx, x, 40.0);
            t.pointer_move(&mut cx, p);
        }
        click(&mut t, &mut cx, 96.0, 40.0);
        let o = only_stair(&cx);
        assert!(o.stair.direction.abs() < 1e-6, "{}", o.stair.direction);
        assert!(o.bottom_center().dist(Point::new(96.0, 40.0)) < 1e-9);
        // The default length is the solved number of 10" treads: 15 treads.
        let (top, _) = plan_stairs::top_point(&o.stair);
        assert!((top.x - 96.0 - 150.0).abs() < 1e-6, "{top:?}");
        assert!((top.y - 40.0).abs() < 1e-6);
        assert_eq!(o.stair.params.tread_depth, 10.0);
        assert_eq!(cx.undo().as_deref(), Some("Click Stairs"));
        // Without a heading it points up the screen.
        let mut t2 = StairsTool::new(StairKind::Click);
        click(&mut t2, &mut cx, 0.0, 0.0);
        assert!((only_stair(&cx).stair.direction - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn curved_stairs_take_a_centre_and_a_radius() {
        let mut cx = new_cx();
        let mut t = StairsTool::new(StairKind::Curved);
        // Centre (200, 200), walking radius 60 straight below it.
        drag(&mut t, &mut cx, (200.0, 200.0), (200.0, 140.0));
        let o = only_stair(&cx);
        assert_eq!(
            o.stair.params.shape,
            StairShape::Curved { inner_radius: 42.0 }
        );
        assert_eq!(o.stair.params.turn, Turn::Left);
        let c = plan_stairs::curve_center(&o.stair).unwrap();
        assert!(c.dist(Point::new(200.0, 200.0)) < 1e-6, "{c:?}");
        // The stair starts on the walking line, heading east, and bends up.
        assert!(o.bottom_center().dist(Point::new(200.0, 140.0)) < 1e-6);
        assert!(o.stair.direction.abs() < 1e-9);
        let (top, z) = plan_stairs::top_point(&o.stair);
        assert!((top.dist(c) - 60.0).abs() < 1e-6);
        assert!((z - o.stair.params.total_rise).abs() < 1e-6);
        assert!(cx.status.starts_with("Curved stairs"), "{}", cx.status);
        // Tab flips the turn: the next stair bends the other way.
        cx.selection.clear();
        t.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        drag(&mut t, &mut cx, (500.0, 200.0), (500.0, 140.0));
        let r = view::load(cx.floor()).pop().unwrap();
        assert_eq!(r.stair.params.turn, Turn::Right);
        let rc = plan_stairs::curve_center(&r.stair).unwrap();
        assert!(rc.dist(Point::new(500.0, 200.0)) < 1e-6, "{rc:?}");
        // The Run handle turns the stair further round.
        let h = view::handles(&r, cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == StairHandleKind::Run)
            .unwrap();
        assert!(h.pos.dist(plan_stairs::top_point(&r.stair).0) < 1e-6);
    }

    #[test]
    fn a_landing_is_clicked_out_as_a_polygon() {
        let mut cx = new_cx();
        let mut t = StairsTool::new(StairKind::Landing);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 120.0, 0.0);
        assert_eq!(t.corners().len(), 2);
        assert!(all(&cx).is_empty(), "nothing placed before the last corner");
        // click, click, double-click: the double-click's first press is the third corner.
        click(&mut t, &mut cx, 120.0, 80.0);
        let p = pe(&cx, 120.0, 80.0);
        let r = t.double_click(&mut cx, p);
        assert_eq!(r.commit.as_deref(), Some("Landing"));
        let o = only_stair(&cx);
        assert!(o.is_landing() && o.is_polygon_landing());
        assert_eq!(o.stair.params.outline.len(), 3);
        assert!((view::footprint_area(&o) - 120.0 * 80.0 / 2.0).abs() < 1e-6);
        assert!(t.corners().is_empty());
        assert!(cx.status.starts_with("Landing: 3 corners"), "{}", cx.status);
        // Picking works inside the triangle and not outside it.
        assert_eq!(
            view::pick(cx.floor(), Point::new(100.0, 20.0), 1.0),
            Some(o.id())
        );
        assert_eq!(view::pick(cx.floor(), Point::new(10.0, 70.0), 1.0), None);
        // One undo step.
        assert_eq!(cx.undo().as_deref(), Some("Landing"));
        assert!(all(&cx).is_empty());
    }

    #[test]
    fn a_landing_polygon_can_close_on_its_first_corner_or_on_enter_or_cancel() {
        let mut cx = new_cx();
        let mut t = StairsTool::new(StairKind::Landing);
        for (x, y) in [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)] {
            click(&mut t, &mut cx, x, y);
        }
        click(&mut t, &mut cx, 1.0, 1.0);
        let o = only_stair(&cx);
        assert_eq!(o.stair.params.outline.len(), 4);
        assert!((view::footprint_area(&o) - 10_000.0).abs() < 1e-6);
        // Enter ends a polygon, Backspace drops a corner, Esc cancels.
        for (x, y) in [(300.0, 0.0), (400.0, 0.0), (400.0, 90.0), (350.0, 150.0)] {
            click(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
        assert_eq!(t.corners().len(), 3);
        let r = t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Landing"));
        assert_eq!(all(&cx).len(), 2);
        click(&mut t, &mut cx, 0.0, 300.0);
        t.key(&mut cx, KeyEvent::escape());
        assert!(t.corners().is_empty());
        assert_eq!(all(&cx).len(), 2);
        // Two corners that do not enclose anything place nothing.
        click(&mut t, &mut cx, 0.0, 300.0);
        click(&mut t, &mut cx, 50.0, 300.0);
        let p = pe(&cx, 50.0, 300.0);
        t.double_click(&mut cx, p);
        assert_eq!(all(&cx).len(), 2);
        // A double-click on its own places a 3' square.
        let p = pe(&cx, 700.0, 700.0);
        t.double_click(&mut cx, p);
        let sq = view::load(cx.floor()).pop().unwrap();
        assert!((view::footprint_area(&sq) - 36.0 * 36.0).abs() < 1e-6);
    }

    #[test]
    fn landings_join_the_stair_sections_they_touch() {
        let mut cx = new_cx();
        cx.project.floors.push(Floor::new("2nd Floor", 120.0));
        let mut t = StairsTool::default();
        // Section A: the lower half of the flight, up to a 60" landing.
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let a = only_stair(&cx);
        assert!((a.stair.params.total_rise - 120.0).abs() < 1e-9);
        let id = a.id();
        view::update(&mut cx.project, 0, id, |o| view::set_total_rise(o, 60.0));
        let a = view::find(cx.floor(), id).unwrap();
        assert_eq!(a.solution().risers, 8);
        let (top, _) = plan_stairs::top_point(&a.stair);
        assert!((top.x - 150.0).abs() < 1e-6, "the run is kept: {top:?}");
        // The landing is dragged against its top: it takes the height of the stair.
        let mut l = StairsTool::new(StairKind::Landing);
        drag(&mut l, &mut cx, (150.0, -30.0), (210.0, 30.0));
        let landing = view::load(cx.floor())
            .into_iter()
            .find(StairObj::is_landing)
            .unwrap();
        assert!((landing.landing_height() - 60.0).abs() < 1e-9);
        assert!(cx.status.contains("joined 1 section"), "{}", cx.status);
        // Section B starts on the landing and rises the rest of the way.
        let mut b = StairsTool::default();
        drag(&mut b, &mut cx, (190.0, 0.0), (190.0, 150.0));
        let b = view::load(cx.floor())
            .into_iter()
            .find(|o| !o.is_landing() && o.id() != id)
            .unwrap();
        assert!((b.stair.base - 60.0).abs() < 1e-9);
        assert_eq!(b.stair.floor_elevation, 0.0, "the floor it stands on");
        assert!((b.stair.params.total_rise - 60.0).abs() < 1e-9);
        assert_eq!(b.solution().risers, 8);
        assert!((b.top_height() - 60.0 - 60.0).abs() < 1e-9);
        // Its top is the next floor.
        let (_, z) = plan_stairs::top_point(&b.stair);
        assert!((z - 120.0).abs() < 1e-9);
        // The landing placed first and the stair second joins the same way.
        let mut cx2 = new_cx();
        cx2.project.floors.push(Floor::new("2nd Floor", 120.0));
        let mut l2 = StairsTool::new(StairKind::Landing);
        drag(&mut l2, &mut cx2, (150.0, -30.0), (210.0, 30.0));
        let mut up = StairsTool::default();
        drag(&mut up, &mut cx2, (190.0, 0.0), (190.0, 150.0));
        let started_on_landing = view::load(cx2.floor())
            .into_iter()
            .find(|o| !o.is_landing())
            .unwrap();
        // The landing was at the floor (height 0): nothing to lift.
        assert!(started_on_landing.stair.base.abs() < 1e-9);
    }

    #[test]
    fn auto_stairwell_cuts_a_hole_in_the_upper_floor_platform() {
        use plan_core::foundation::FoundationLayer;
        let mut cx = new_cx();
        add_floor_above(&mut cx);
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let o = only_stair(&cx);
        assert!(view::run_command(&mut cx, StairCommand::AutoStairwell));
        let layer = FoundationLayer::load(&cx.project.floors[1]);
        assert_eq!(layer.platform_holes.len(), 1);
        let hole = &layer.platform_holes[0];
        assert_eq!(hole.kind, plan_core::foundation::PlatformKind::Floor);
        assert_eq!(hole.owner, Some(o.id()));
        // The hole is the footprint of the stair (grown by 1/20" so the faces
        // of the cut stand just outside it).
        let fp = view::footprint_area(&o);
        assert!(
            hole.area() > fp && (hole.area() - fp) / fp < 0.005,
            "{}",
            hole.area()
        );
        assert!((hole.area() - (150.1 * 36.1)).abs() < 1e-6);
        // Nothing is cut on the stair's own floor.
        assert!(FoundationLayer::load(&cx.project.floors[0])
            .platform_holes
            .is_empty());
        // Undo takes the hole and the walls away; redo brings them back.
        assert_eq!(cx.undo().as_deref(), Some("Auto Stairwell"));
        assert!(FoundationLayer::load(&cx.project.floors[1])
            .platform_holes
            .is_empty());
        assert!(cx.project.floors[1].walls.is_empty());
        assert_eq!(cx.redo().as_deref(), Some("Auto Stairwell"));
        assert_eq!(
            FoundationLayer::load(&cx.project.floors[1])
                .platform_holes
                .len(),
            1
        );
        // Moving the stair moves the hole and the walls with it.
        let h = view::handles(&only_stair(&cx), cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == StairHandleKind::Move)
            .unwrap()
            .pos;
        let (a, b) = (pe(&cx, h.x, h.y), pe(&cx, h.x + 60.0, h.y + 24.0));
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        let moved = only_stair(&cx);
        let layer = FoundationLayer::load(&cx.project.floors[1]);
        let (lo, _) = plan_core::foundation::bounds(&layer.platform_holes[0].outline);
        let (mlo, _) = plan_core::foundation::bounds(&moved.footprint());
        assert!(lo.dist(mlo) < 0.1, "{lo:?} vs {mlo:?}");
        assert!(mlo.dist(Point::new(0.0, -18.0)) > 1.0, "the stair did move");
        assert_eq!(cx.project.floors[1].walls.len(), 4);
        // The divider walls moved too: they stand 0.3" outside the footprint.
        let (wlo, _) = plan_core::foundation::bounds(
            &cx.project.floors[1]
                .walls
                .iter()
                .map(|w| w.start)
                .collect::<Vec<_>>(),
        );
        assert!(wlo.dist(mlo) < 0.5, "{wlo:?} vs {mlo:?}");
        // Deleting the stair removes the hole; undo restores it.
        let r = t.key(&mut cx, KeyEvent::key(egui::Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete Stairs"));
        assert!(FoundationLayer::load(&cx.project.floors[1])
            .platform_holes
            .is_empty());
        assert!(cx.project.floors[1].walls.is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Delete Stairs"));
        assert_eq!(
            FoundationLayer::load(&cx.project.floors[1])
                .platform_holes
                .len(),
            1
        );
    }

    #[test]
    fn a_curved_stairwell_hole_follows_the_curved_footprint() {
        use plan_core::foundation::FoundationLayer;
        let mut cx = new_cx();
        add_floor_above(&mut cx);
        let mut t = StairsTool::new(StairKind::Curved);
        drag(&mut t, &mut cx, (200.0, 200.0), (200.0, 140.0));
        let o = only_stair(&cx);
        assert!(view::run_command(&mut cx, StairCommand::AutoStairwell));
        let hole = FoundationLayer::load(&cx.project.floors[1]).platform_holes[0].clone();
        assert!(hole.outline.len() > 8, "an arc is many corners");
        assert!(hole.area() >= view::footprint_area(&o));
        assert!((hole.area() - view::footprint_area(&o)) / view::footprint_area(&o) < 0.01);
        // As many divider walls as outline edges: still a closed ring.
        assert_eq!(cx.project.floors[1].walls.len(), hole.outline.len());
    }

    #[test]
    fn heights_and_locks_drive_the_riser_count() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let mut o = only_stair(&cx);
        // 121 1/4" with 7 1/2" target: 16 risers, 15 treads of 10".
        assert_eq!((o.solution().risers, o.solution().treads), (16, 15));
        assert!((o.top_height() - 121.25).abs() < 1e-9 && o.bottom_height().abs() < 1e-9);

        // Unlocked: a lower top height gives fewer risers; the run is kept.
        view::set_top_height(&mut o, 100.0);
        assert_eq!(o.solution().risers, 13);
        assert!((o.solution().treads as f64 * o.stair.params.tread_depth - 150.0).abs() < 1e-6);
        assert!((o.solution().riser_height - 100.0 / 13.0).abs() < 1e-9);

        // Number of treads locked: the count stays, the riser height follows.
        let mut locked = o.clone();
        locked.x.lock_count = true;
        view::set_top_height(&mut locked, 90.0);
        assert_eq!(locked.solution().risers, 13);
        assert!((locked.solution().riser_height - 90.0 / 13.0).abs() < 1e-9);

        // Riser height locked: the riser stays near the target and the count follows.
        let mut rl = o.clone();
        rl.x.lock_riser = true;
        let target = rl.stair.params.riser_height_target;
        view::set_top_height(&mut rl, 60.0);
        assert!((rl.stair.params.riser_height_target - target).abs() < 1e-9);
        assert_eq!(rl.solution().risers, (60.0 / target).round() as u32);

        // Tread depth locked: changing the count leaves the tread alone.
        let mut tl = o.clone();
        tl.x.lock_tread = true;
        view::set_risers(&mut tl, 14);
        assert_eq!(tl.solution().risers, 14);
        assert_eq!(tl.stair.params.tread_depth, o.stair.params.tread_depth);
        // Unlocked it keeps the run.
        view::set_risers(&mut o, 14);
        assert!((13.0 * o.stair.params.tread_depth - 150.0).abs() < 1e-6);
        // Fewer risers than the 7 3/4" maximum allows are refused by the solver.
        let mut few = o.clone();
        view::set_risers(&mut few, 8);
        assert_eq!(few.solution().risers, 13);

        // A section starting 30" up: bottom and top height, fitted to the story.
        view::set_bottom_height(&mut o, 30.0);
        assert!((o.stair.base - 30.0).abs() < 1e-9);
        assert!((o.top_height() - 100.0).abs() < 1e-9, "the top stays");
        assert!(view::fit_to_story(&mut o));
        assert!((o.top_height() - 121.25).abs() < 1e-9);
    }

    #[test]
    fn the_riser_count_comes_from_the_floor_to_floor_height() {
        let mut cx = new_cx();
        cx.project.floors.push(Floor::new("2nd Floor", 109.125));
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let mut o = only_stair(&cx);
        assert!((o.stair.params.total_rise - 109.125).abs() < 1e-9);
        assert_eq!(o.x.story_rise, 109.125);
        // With the 7 3/4" maximum as the target: 15 risers.
        o.stair.params.riser_height_target = plan_stairs::MAX_RISER;
        let sol = o.solution();
        assert_eq!((sol.risers, sol.treads), (15, 14));
        assert!(sol.riser_height <= plan_stairs::MAX_RISER);
    }

    #[test]
    fn a_ramp_over_30_inches_can_be_set_and_gets_landings() {
        let mut cx = new_cx();
        let mut t = StairsTool::new(StairKind::Ramp);
        drag(&mut t, &mut cx, (0.0, 300.0), (360.0, 300.0));
        let mut o = only_stair(&cx);
        view::set_total_rise(&mut o, 60.0);
        assert_eq!(o.solution().landings, 1);
        assert!((o.solution().total_run - 780.0).abs() < 1e-6);
        // The Run handle sets the length of one run.
        view::set_run(&mut o, 300.0);
        match o.stair.params.shape {
            StairShape::Ramp { slope_1_in } => assert!((slope_1_in - 10.0).abs() < 1e-9),
            s => panic!("{s:?}"),
        }
        assert!(!o.solution().code_ok, "1:10 is too steep");
        // The section view draws both runs and the landing.
        let pts = view::elevation_points(&o);
        assert_eq!(pts.len(), 1 + 2 + 1);
    }

    #[test]
    fn old_files_load_a_landing_and_the_railing_flags() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let id = only_stair(&cx).id();
        // A landing as the previous version stored it: a straight stair plus x.landing_depth.
        let mut v = cx.floor().stairs[0].clone();
        v["x"]["landing_depth"] = serde_json::json!(54.0);
        v["x"]["railing_left"] = serde_json::json!(true);
        cx.floor_mut().stairs[0] = v;
        let o = view::find(cx.floor(), id).unwrap();
        assert!(o.is_landing());
        assert_eq!(o.landing_depth(), Some(54.0));
        assert_eq!(o.stair.params.left_side, plan_stairs::SideKind::Railing);
    }

    #[test]
    fn scene_meshes_cover_stairs_landings_and_railings() {
        let mut cx = new_cx();
        let mut t = StairsTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (150.0, 0.0));
        let plain = view::scene_meshes(cx.floor()).len();
        // 15 treads, 16 risers, 2 stringers.
        assert_eq!(plain, 15 + 16 + 2);
        assert!(view::run_command(&mut cx, StairCommand::MakeRailing));
        let railed = view::scene_meshes(cx.floor()).len();
        assert!(railed > plain + 30, "{railed} vs {plain}");
        let mut l = StairsTool::new(StairKind::Landing);
        drag(&mut l, &mut cx, (150.0, -30.0), (210.0, 30.0));
        assert_eq!(view::scene_meshes(cx.floor()).len(), railed + 1);
        assert!(view::scene_meshes(cx.floor())
            .iter()
            .all(|m| m.object_id.is_some()));
    }

    #[test]
    fn a_polygon_landing_has_a_move_handle_and_a_handle_per_corner() {
        let mut cx = new_cx();
        let mut t = StairsTool::new(StairKind::Landing);
        for (x, y) in [(0.0, 0.0), (120.0, 0.0), (120.0, 80.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let p = pe(&cx, 120.0, 80.0);
        t.double_click(&mut cx, p);
        let o = only_stair(&cx);
        let hs = view::handles(&o, cx.px_per_in);
        // The body moves it; each corner of the outline reshapes it.
        assert_eq!(hs.len(), 4);
        assert_eq!(hs[0].kind, StairHandleKind::Move);
        assert_eq!(
            hs[1..].iter().map(|h| h.kind).collect::<Vec<_>>(),
            (0..3).map(StairHandleKind::Corner).collect::<Vec<_>>()
        );
        assert!(hs[3].pos.dist(Point::new(120.0, 80.0)) < 1e-9);
        let a = pe(&cx, hs[0].pos.x, hs[0].pos.y);
        let b = pe(&cx, hs[0].pos.x + 48.0, hs[0].pos.y + 24.0);
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        let m = only_stair(&cx);
        assert!(m.stair.params.outline[0].dist(Point::new(48.0, 24.0)) < 1e-6);
        assert!((view::footprint_area(&m) - view::footprint_area(&o)).abs() < 1e-6);
        assert_eq!(cx.undo().as_deref(), Some("Move Stairs"));
    }

    #[test]
    fn dragging_a_landing_corner_reshapes_the_polygon_in_one_undo_step() {
        let mut cx = new_cx();
        let mut t = StairsTool::new(StairKind::Landing);
        for (x, y) in [(0.0, 0.0), (120.0, 0.0), (120.0, 80.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let p = pe(&cx, 120.0, 80.0);
        t.double_click(&mut cx, p);
        let before = only_stair(&cx);
        let area = view::footprint_area(&before);
        let hs = view::handles(&before, cx.px_per_in);
        let corner = hs
            .iter()
            .find(|h| h.kind == StairHandleKind::Corner(2))
            .unwrap();
        let a = pe(&cx, corner.pos.x, corner.pos.y);
        let b = pe(&cx, 120.0, 160.0);
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        let after = only_stair(&cx);
        assert!(after.stair.params.outline[2].dist(Point::new(120.0, 160.0)) < 1e-6);
        assert_eq!(
            after.stair.params.outline[0],
            before.stair.params.outline[0]
        );
        assert!(view::footprint_area(&after) > area + 1.0);
        // Depth and width follow the outline's extents.
        assert!((after.stair.params.width - 160.0).abs() < 1e-6);
        assert_eq!(after.landing_depth(), Some(120.0));
        assert_eq!(cx.undo().as_deref(), Some("Reshape Landing"));
        assert!(only_stair(&cx).stair.params.outline[2].dist(Point::new(120.0, 80.0)) < 1e-6);
    }

    #[test]
    fn the_l_and_u_drags_make_their_landing_without_a_landing_object() {
        let mut cx = new_cx();
        let mut l = StairsTool::new(StairKind::LShaped);
        drag(&mut l, &mut cx, (0.0, 0.0), (70.0, 0.0));
        let o = only_stair(&cx);
        assert_eq!(o.solution().landings, 1);
        assert_eq!(
            view::load(cx.floor()).len(),
            1,
            "the landing is part of the stair"
        );
        let parts = plan_stairs::tagged_meshes(&o.stair);
        assert!(parts
            .iter()
            .any(|(p, _)| *p == plan_stairs::StairPart::Landing));
    }
}
