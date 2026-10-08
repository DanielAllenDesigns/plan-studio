//! Straight wall tools (W-1..W-20 in `docs/parity/walls.md`).
//!
//! * click, click, ... draws a chain: each click after the first ends a wall
//!   and starts the next at the same point (W-3);
//! * press-drag-release draws exactly one wall and ends the chain (W-4);
//! * endpoint, midpoint, intersection, on-wall, 15 degree angle and grid
//!   snaps (W-11); Alt suspends the angle snap;
//! * a wall started or ended in the middle of another wall splits that wall
//!   there, so the T-junction is a real connection;
//! * Esc cancels the wall being drawn (W-8).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::ops::{self, make_wall};
use crate::editor::{render, Camera, EditorContext, ObjectRef};
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align2, FontId, Pos2, Shape, Stroke, Vec2};
use plan_core::geometry::Point;
use plan_core::WallKind;

/// Pixels the pointer must travel between press and release for a drag-draw.
const DRAG_PX: f32 = 4.0;
/// Walls shorter than this are not created.
const MIN_LENGTH: f64 = 1.0;

struct Press {
    screen: Pos2,
    /// The press placed the first point of a new chain.
    started_chain: bool,
}

pub struct WallTool {
    kind: WallKind,
    pending: Option<Point>,
    press: Option<Press>,
    hover: Option<Point>,
}

impl Default for WallTool {
    fn default() -> Self {
        Self {
            kind: WallKind::Exterior,
            pending: None,
            press: None,
            hover: None,
        }
    }
}

impl WallTool {
    /// The start point of the wall in progress.
    pub fn pending_start(&self) -> Option<Point> {
        self.pending
    }

    fn snap(&self, cx: &EditorContext, p: &PointerEvent) -> crate::editor::SnapResult {
        cx.snap_at(p.world, self.pending, p.modifiers.alt, &[])
    }

    fn update_readout(&self, cx: &mut EditorContext, to: Point) {
        cx.readout = self
            .pending
            .map(|s| format!("Length: {}", cx.fmt_dim(s.dist(to))));
    }

    /// Adds the wall `start`..`end`, splitting walls it starts or ends on.
    fn create(&mut self, cx: &mut EditorContext, start: Point, end: Point) -> bool {
        if start.dist(end) < MIN_LENGTH {
            return false;
        }
        cx.begin_change("Draw Wall");
        let fl = cx.floor;
        ops::split_walls_at_point(&mut cx.project, fl, start, &[]);
        ops::split_walls_at_point(&mut cx.project, fl, end, &[]);
        let id = cx.project.add_wall(
            fl,
            start,
            end,
            cx.wall_thickness(self.kind),
            cx.wall_height(self.kind),
            self.kind,
        );
        // New walls take the default wall type of their kind (W-51).
        let ty = cx.defaults.walls_for(self.kind).wall_type.clone();
        if let Some(def) = cx.defaults.wall_type(&ty).cloned() {
            if cx.project.wall_type_def(&ty).is_none() {
                cx.project.register_wall_type(def);
            }
            if let Some(w) = cx.project.floors[fl].wall_mut(id) {
                w.wall_type = Some(ty);
            }
        }
        cx.selection.set(ObjectRef::Wall(id));
        cx.mark_dirty();
        true
    }
}

impl Tool for WallTool {
    fn id(&self) -> ToolId {
        ToolId::Wall { kind: self.kind }
    }

    fn name(&self) -> &'static str {
        match self.kind {
            WallKind::Exterior => "Straight Exterior Wall",
            WallKind::Interior => "Straight Interior Wall",
        }
    }

    fn hint(&self) -> String {
        "Wall: click to place points; Alt disables angle snap; Esc/right-click ends".into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::Wall { kind } = id {
            self.kind = kind;
        }
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.pending = None;
        self.press = None;
        self.hover = None;
        cx.readout = None;
        cx.last_snap = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let s = self.snap(cx, &p);
        self.hover = Some(s.point);
        cx.last_snap = Some(s);
        self.update_readout(cx, s.point);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let s = self.snap(cx, &p);
        self.hover = Some(s.point);
        let started_chain = self.pending.is_none();
        if started_chain {
            self.pending = Some(s.point);
        }
        self.press = Some(Press {
            screen: p.screen,
            started_chain,
        });
        self.update_readout(cx, s.point);
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let Some(press) = self.press.take() else {
            return ToolResult::ignored();
        };
        let end = self.snap(cx, &p).point;
        let Some(start) = self.pending else {
            return ToolResult::ignored();
        };
        if press.started_chain {
            let dragged = (p.screen - press.screen).length() >= DRAG_PX;
            if !dragged {
                return ToolResult::consumed();
            }
            // Press-drag-release: one wall, then the chain ends.
            let made = self.create(cx, start, end);
            self.pending = None;
            cx.readout = None;
            return if made {
                ToolResult::committed("Draw Wall")
            } else {
                ToolResult::consumed()
            };
        }
        if self.create(cx, start, end) {
            self.pending = Some(end);
            self.update_readout(cx, end);
            ToolResult::committed("Draw Wall")
        } else {
            ToolResult::consumed()
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(egui::Key::Escape) && self.pending.is_some() {
            self.pending = None;
            self.press = None;
            cx.readout = None;
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        let Some(to) = self.hover else { return };
        if let Some(start) = self.pending {
            let len = start.dist(to);
            if len > 0.01 {
                let ghost = make_wall(
                    0,
                    start,
                    to,
                    cx.wall_thickness(self.kind),
                    cx.wall_height(self.kind),
                    self.kind,
                );
                let pts = ghost
                    .footprint()
                    .iter()
                    .map(|p| cam.world_to_screen(*p))
                    .collect();
                painter.add(Shape::convex_polygon(
                    pts,
                    pal.ghost_fill,
                    Stroke::new(1.0_f32, pal.ghost_stroke),
                ));
                if cx.view_flags.contains(&ViewFlag::TemporaryDimensions) {
                    let mid = Point::lerp(start, to, 0.5)
                        .add(ghost.normal().scale(ghost.thickness * 0.5));
                    painter.text(
                        cam.world_to_screen(mid)
                            + Vec2::new(0.0, -8.0) * ghost.normal().y.signum() as f32,
                        Align2::CENTER_CENTER,
                        cx.fmt_dim(len),
                        FontId::proportional(13.0),
                        pal.dimension_text,
                    );
                }
            }
        }
        if let Some(s) = cx.last_snap {
            render::draw_snap_marker(painter, cam, &s, pal.ghost_stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use eframe::egui::Modifiers;

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn click(t: &mut WallTool, cx: &mut EditorContext, x: f64, y: f64) {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
    }

    #[test]
    fn click_click_draws_a_closed_room() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        for (x, y) in [
            (0.0, 0.0),
            (240.0, 0.0),
            (240.0, 144.0),
            (0.0, 144.0),
            (0.0, 0.0),
        ] {
            click(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::escape());
        assert!(t.pending_start().is_none());
        assert_eq!(cx.floor().walls.len(), 4);
        cx.refresh();
        assert_eq!(cx.rooms.len(), 1);
        // One undo step per wall.
        assert_eq!(cx.undo().as_deref(), Some("Draw Wall"));
        assert_eq!(cx.floor().walls.len(), 3);
    }

    #[test]
    fn drag_draws_one_wall_and_releases() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        let a = PointerEvent::at(&cx, Point::new(0.0, 0.0));
        let b = PointerEvent::at(&cx, Point::new(120.0, 0.0));
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        assert_eq!(cx.floor().walls.len(), 1);
        assert!(t.pending_start().is_none());
    }

    #[test]
    fn angle_snap_unless_alt() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        click(&mut t, &mut cx, 0.0, 0.0);
        let raw = Point::new(100.0, 4.0);
        let p = PointerEvent::at(&cx, raw);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        assert!(cx.floor().walls[0].end.y.abs() < 1e-9);
        let mut cx2 = new_cx();
        let mut t2 = WallTool::default();
        click(&mut t2, &mut cx2, 0.0, 0.0);
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        let p = PointerEvent::at(&cx2, raw).with_modifiers(alt);
        t2.pointer_down(&mut cx2, p.with_down(true));
        t2.pointer_up(&mut cx2, p);
        assert_eq!(cx2.floor().walls[0].end, Point::new(100.0, 4.0));
    }

    #[test]
    fn drawing_onto_a_wall_splits_it() {
        let mut cx = new_cx();
        let through = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            7.625,
            109.125,
            WallKind::Exterior,
        );
        let mut t = WallTool::default();
        t.set_variant(ToolId::Wall {
            kind: WallKind::Interior,
        });
        // Start 3" off the wall: on-wall snap pulls the start onto it.
        click(&mut t, &mut cx, 100.0, 3.0);
        click(&mut t, &mut cx, 100.0, 120.0);
        let walls = &cx.floor().walls;
        assert_eq!(walls.len(), 3);
        let first = cx.floor().wall(through).unwrap();
        assert_eq!(first.end, Point::new(100.0, 0.0));
        let tail = walls
            .iter()
            .find(|w| w.start == Point::new(100.0, 0.0) && w.end.x == 240.0);
        assert!(tail.is_some());
        let stem = walls.iter().find(|w| w.kind == WallKind::Interior).unwrap();
        assert_eq!(stem.start, Point::new(100.0, 0.0));
    }
}
