//! The Schedule tool (Schedule flyout): click to place a schedule table in the
//! plan, one flavor per kind (`Door Schedule`, `Cabinet Schedule`, ... and
//! `Create Schedule` for a General one).
//!
//! * A click on empty plan places a new schedule with its upper-left corner at
//!   the snapped point and selects it. Placing is one undo step.
//! * A click on an existing schedule selects it; dragging moves it.
//! * A double-click on a schedule opens its Schedule Specification.
//! * Delete (or Backspace) deletes the selected schedule.
//!
//! A schedule is `ObjectRef::Schedule`, so the selection is `cx.selection`,
//! shared with Select Objects (which also picks, moves, deletes and opens one).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::build_tools;
use crate::editor::schedule_view as sv;
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Key, Stroke, StrokeKind};
use plan_core::geometry::Point;
use plan_core::schedules::{Schedule, ScheduleKind};
use plan_core::Id;

/// The kinds the Schedule flyout lists, in order, then the General one.
pub const FLYOUT_KINDS: [ScheduleKind; 10] = [
    ScheduleKind::Door,
    ScheduleKind::Window,
    ScheduleKind::Room,
    ScheduleKind::Cabinet,
    ScheduleKind::Electrical,
    ScheduleKind::Framing,
    ScheduleKind::Plant,
    ScheduleKind::Fixture,
    ScheduleKind::Furniture,
    ScheduleKind::General,
];

/// The flyout entry name of a kind ("Create Schedule" for the General one).
pub fn entry_name(kind: ScheduleKind) -> &'static str {
    match kind {
        ScheduleKind::General => "Create Schedule",
        k => k.title(),
    }
}

/// A move in progress: the schedule, where the pointer went down and where
/// the table's corner was.
struct MoveState {
    id: Id,
    start: Point,
    corner: Point,
    moved: bool,
}

pub struct ScheduleTool {
    kind: ScheduleKind,
    hover: Option<Point>,
    moving: Option<MoveState>,
    /// Size of the ghost table shown under the pointer, plan inches.
    ghost: Option<(f64, f64)>,
}

impl Default for ScheduleTool {
    fn default() -> Self {
        Self {
            kind: ScheduleKind::Door,
            hover: None,
            moving: None,
            ghost: None,
        }
    }
}

impl ScheduleTool {
    fn ghost_size(&self, cx: &EditorContext) -> (f64, f64) {
        let def = Schedule::new(self.kind, Point::ZERO);
        let l = sv::layout_of(cx, &def, cx.floor);
        (l.width, l.height)
    }
}

impl Tool for ScheduleTool {
    fn id(&self) -> ToolId {
        ToolId::ScheduleVariant(self.kind)
    }

    fn name(&self) -> &'static str {
        entry_name(self.kind)
    }

    fn hint(&self) -> String {
        format!(
            "{}: click to place it; double-click a schedule for its specification",
            entry_name(self.kind)
        )
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::ScheduleVariant(k) = id {
            self.kind = k;
            self.ghost = None;
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.hover = None;
        self.moving = None;
        self.ghost = None;
        cx.status = self.hint();
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.hover = None;
        self.moving = None;
        self.ghost = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(m) = &mut self.moving {
            if p.down {
                let to = Point::new(
                    m.corner.x + (p.world.x - m.start.x),
                    m.corner.y + (p.world.y - m.start.y),
                );
                if (to - m.corner).length() > 1e-6 {
                    m.moved = true;
                }
                // Preview without an undo step; the drop commits it.
                let id = m.id;
                let fl = cx.floor;
                let mut layer = sv::load(cx);
                if let Some(s) = layer.find_mut(id) {
                    s.position = to;
                }
                layer.store(&mut cx.project.floors[fl]);
                return ToolResult::consumed();
            }
        }
        self.hover = Some(cx.snap_at(p.world, None, p.modifiers.alt, &[]).point);
        if self.ghost.is_none() {
            self.ghost = Some(self.ghost_size(cx));
        }
        ToolResult::consumed()
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(id) = sv::pick(cx, p.world) {
            sv::select(cx, id);
            if let Some(s) = sv::find(cx, id) {
                // The drag moves it; the undo step is taken here, before the
                // first preview change, and dropped on release if nothing moved.
                cx.begin_change("Move Schedule");
                self.moving = Some(MoveState {
                    id,
                    start: p.world,
                    corner: s.position,
                    moved: false,
                });
            }
            return ToolResult::consumed();
        }
        sv::clear_selection(cx);
        let at = cx.snap_at(p.world, None, p.modifiers.alt, &[]).point;
        let id = sv::add(cx, self.kind, at);
        sv::select(cx, id);
        cx.status = format!("{} placed", entry_name(self.kind));
        ToolResult::committed(&format!("Place {}", self.kind.title()))
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        let Some(m) = self.moving.take() else {
            return ToolResult::ignored();
        };
        if !m.moved {
            cx.cancel_change();
            return ToolResult::consumed();
        }
        cx.mark_dirty();
        ToolResult::committed("Move Schedule")
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        // The first click of the double-click may have started a move.
        if let Some(m) = self.moving.take() {
            if !m.moved {
                cx.cancel_change();
            }
        }
        match sv::pick(cx, p.world) {
            Some(id) => {
                sv::select(cx, id);
                build_tools::open_schedule_spec(cx.floor, id);
                ToolResult::consumed()
            }
            None => ToolResult::ignored(),
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Delete) || k.is(Key::Backspace) {
            if let Some(id) = sv::selected(cx) {
                if sv::delete(cx, id) {
                    return ToolResult::committed("Delete Schedule");
                }
            }
            return ToolResult::ignored();
        }
        if k.is(Key::Enter) {
            if let Some(id) = sv::selected(cx).filter(|id| sv::find(cx, *id).is_some()) {
                build_tools::open_schedule_spec(cx.floor, id);
                return ToolResult::consumed();
            }
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let (Some(at), Some((w, h))) = (self.hover, self.ghost) else {
            return;
        };
        if self.moving.is_some() {
            return;
        }
        let a = cam.world_to_screen(at);
        let b = cam.world_to_screen(Point::new(at.x + w, at.y - h));
        let r = egui::Rect::from_two_pos(a, b);
        painter.rect_filled(r, 0.0, cx.palette.ghost_fill);
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(1.6_f32, cx.palette.ghost_stroke),
            StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::schedules::ScheduleLayer;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn ev(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(cx, Point::new(x, y))
    }

    #[test]
    fn a_click_places_a_schedule_of_the_flavor_and_undo_removes_it() {
        let mut cx = cx();
        sv::clear_selection(&mut cx);
        let mut t = ScheduleTool::default();
        t.set_variant(ToolId::ScheduleVariant(ScheduleKind::Cabinet));
        assert_eq!(t.name(), "Cabinet Schedule");
        t.activate(&mut cx);
        let ev1 = ev(&cx, 100.0, -50.0);
        let r = t.pointer_down(&mut cx, ev1);
        assert!(r.commit.is_some());
        let layer = ScheduleLayer::load(cx.floor());
        assert_eq!(layer.schedules.len(), 1);
        assert_eq!(layer.schedules[0].kind, ScheduleKind::Cabinet);
        assert_eq!(sv::selected(&cx), Some(layer.schedules[0].id));
        assert_eq!(cx.undo_label(), Some("Place Cabinet Schedule"));
        cx.undo();
        assert!(ScheduleLayer::load(cx.floor()).is_empty());
    }

    #[test]
    fn create_schedule_places_a_general_one() {
        let mut cx = cx();
        let mut t = ScheduleTool::default();
        t.set_variant(ToolId::ScheduleVariant(ScheduleKind::General));
        assert_eq!(t.name(), "Create Schedule");
        let ev2 = ev(&cx, 0.0, 0.0);
        t.pointer_down(&mut cx, ev2);
        assert_eq!(
            ScheduleLayer::load(cx.floor()).schedules[0].kind,
            ScheduleKind::General
        );
    }

    #[test]
    fn clicking_a_schedule_selects_it_and_dragging_moves_it_as_one_step() {
        let mut cx = cx();
        let mut t = ScheduleTool::default();
        let ev3 = ev(&cx, 0.0, 0.0);
        t.pointer_down(&mut cx, ev3);
        let id = sv::selected(&cx).unwrap();
        let def = sv::find(&cx, id).unwrap();
        let l = sv::layout_of(&cx, &def, 0);
        let (lo, hi) = l.bounds(def.position);
        let inside = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        sv::clear_selection(&mut cx);
        let ev4 = PointerEvent::at(&cx, inside);
        let r = t.pointer_down(&mut cx, ev4);
        assert!(r.consumed && r.commit.is_none());
        assert_eq!(sv::selected(&cx), Some(id));
        assert_eq!(
            ScheduleLayer::load(cx.floor()).schedules.len(),
            1,
            "a click on a schedule does not place another"
        );
        let drag =
            PointerEvent::at(&cx, Point::new(inside.x + 30.0, inside.y + 40.0)).with_down(true);
        t.pointer_move(&mut cx, drag);
        let ev5 = ev(&cx, inside.x + 30.0, inside.y + 40.0);
        let r = t.pointer_up(&mut cx, ev5);
        assert_eq!(r.commit.as_deref(), Some("Move Schedule"));
        let moved = sv::find(&cx, id).unwrap();
        assert!((moved.position.x - (def.position.x + 30.0)).abs() < 1e-6);
        assert!((moved.position.y - (def.position.y + 40.0)).abs() < 1e-6);
        assert_eq!(cx.undo().as_deref(), Some("Move Schedule"));
        assert_eq!(sv::find(&cx, id).unwrap().position, def.position);
    }

    #[test]
    fn a_click_without_a_drag_leaves_no_undo_step() {
        let mut cx = cx();
        let mut t = ScheduleTool::default();
        let ev6 = ev(&cx, 0.0, 0.0);
        t.pointer_down(&mut cx, ev6);
        let id = sv::selected(&cx).unwrap();
        let def = sv::find(&cx, id).unwrap();
        let l = sv::layout_of(&cx, &def, 0);
        let (lo, hi) = l.bounds(def.position);
        let inside = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
        let ev7 = PointerEvent::at(&cx, inside);
        t.pointer_down(&mut cx, ev7);
        let ev8 = PointerEvent::at(&cx, inside);
        t.pointer_up(&mut cx, ev8);
        assert_eq!(cx.undo_label(), Some("Place Door Schedule"));
    }

    #[test]
    fn delete_removes_the_selected_schedule() {
        let mut cx = cx();
        let mut t = ScheduleTool::default();
        let ev9 = ev(&cx, 0.0, 0.0);
        t.pointer_down(&mut cx, ev9);
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete Schedule"));
        assert!(ScheduleLayer::load(cx.floor()).is_empty());
        assert!(!t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
    }

    #[test]
    fn every_flyout_kind_names_itself() {
        for k in FLYOUT_KINDS {
            assert!(!entry_name(k).is_empty());
        }
        assert_eq!(FLYOUT_KINDS.len(), 10);
    }
}
