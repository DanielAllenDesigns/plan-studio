//! Straight wall tools (W-1..W-20 in `docs/parity/walls.md`).
//!
//! * click, click, ... draws a chain: each click after the first ends a wall
//!   and starts the next at the same point (W-3);
//! * press-drag-release draws exactly one wall and ends the chain (W-4);
//! * snaps (W-11..W-14), in priority order: the start of the chain's first
//!   wall (clicking it closes the loop and ends the chain, W-5), another
//!   wall's endpoint, intersection, midpoint, perpendicular foot, centerline,
//!   the axes through the chain's first point, alignment with the previous
//!   wall (collinear or perpendicular), the 15 degree angle and the grid;
//!   Alt suspends the angle snap;
//! * every wall is connected on commit (`editor::connect::auto_connect`,
//!   W-31..W-45): corners close exactly, a wall ending near another wall's
//!   centerline becomes a T that splits the through wall, crossing walls are
//!   cut, overlapping duplicates merge;
//! * Esc cancels the wall being drawn (W-8).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::connect;
use crate::editor::ops::{make_wall, JOIN_TOL};
use crate::editor::snap::SnapKind;
use crate::editor::{render, Camera, EditorContext, ObjectRef, SnapResult};
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align2, FontId, Pos2, Shape, Stroke, Vec2};
use plan_core::geometry::Point;
use plan_core::{detect_rooms, Id, WallKind};

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
    /// Start of the first wall of the chain (clicking it closes the loop).
    chain_first: Option<Point>,
    /// Walls drawn in the chain so far.
    chain_walls: usize,
    /// Unit direction of the previous wall of the chain.
    last_dir: Option<Point>,
}

impl Default for WallTool {
    fn default() -> Self {
        Self {
            kind: WallKind::Exterior,
            pending: None,
            press: None,
            hover: None,
            chain_first: None,
            chain_walls: 0,
            last_dir: None,
        }
    }
}

impl WallTool {
    /// The start point of the wall in progress.
    pub fn pending_start(&self) -> Option<Point> {
        self.pending
    }

    /// Forgets the chain (it closed, was cancelled or the tool left).
    fn end_chain(&mut self) {
        self.pending = None;
        self.chain_first = None;
        self.chain_walls = 0;
        self.last_dir = None;
    }

    /// The snapped end point for the pointer, and whether it closes the loop
    /// (W-5, W-11..W-14).
    fn snap(&self, cx: &EditorContext, p: &PointerEvent) -> (SnapResult, bool) {
        let raw = p.world;
        let alt = p.modifiers.alt;
        let tol = cx.snap_tol();
        // Closing the loop on the first wall's start point.
        if let (Some(first), false) = (self.chain_first, alt) {
            if self.chain_walls >= 2 && raw.dist(first) <= tol {
                let r = SnapResult {
                    point: first,
                    kind: SnapKind::Endpoint,
                    source: None,
                };
                return (r, true);
            }
        }
        // Object snaps: endpoints, intersections, midpoints, perpendicular
        // feet and wall centerlines.
        let base = cx.snap_at(raw, self.pending, alt, &[]);
        let Some(start) = self.pending else {
            return (base, false);
        };
        if alt || base.kind.is_object_snap() {
            return (base, false);
        }
        let grid = cx.defaults.grid.snap;
        let on_grid = |v: f64| {
            if grid > 0.0 {
                (v / grid).round() * grid
            } else {
                v
            }
        };
        // The axes through the chain's first point.
        if let Some(first) = self.chain_first.filter(|f| f.dist(start) > JOIN_TOL) {
            let dx = (raw.x - first.x).abs();
            let dy = (raw.y - first.y).abs();
            let hit = match (dx <= tol, dy <= tol) {
                (true, true) if dx <= dy => Some(Point::new(first.x, on_grid(raw.y))),
                (true, true) => Some(Point::new(on_grid(raw.x), first.y)),
                (true, false) => Some(Point::new(first.x, on_grid(raw.y))),
                (false, true) => Some(Point::new(on_grid(raw.x), first.y)),
                _ => None,
            };
            if let Some(point) = hit.filter(|q| q.dist(start) > JOIN_TOL) {
                let r = SnapResult {
                    point,
                    kind: SnapKind::Angle,
                    source: None,
                };
                return (r, false);
            }
        }
        // Collinear with, or perpendicular to, the previous wall.
        if let Some(d) = self.last_dir {
            let v = raw - start;
            let mut best: Option<(f64, Point, SnapKind)> = None;
            for (dir, kind) in [(d, SnapKind::Angle), (d.perp(), SnapKind::Perpendicular)] {
                let along = v.dot(dir);
                let off = v.dot(dir.perp()).abs();
                if off <= tol && along.abs() > tol && best.is_none_or(|b| off < b.0) {
                    best = Some((off, start + dir * on_grid(along), kind));
                }
            }
            if let Some((_, point, kind)) = best {
                if point.dist(start) > JOIN_TOL {
                    let r = SnapResult {
                        point,
                        kind,
                        source: None,
                    };
                    return (r, false);
                }
            }
        }
        (base, false)
    }

    fn update_readout(&self, cx: &mut EditorContext, to: Point) {
        cx.readout = self
            .pending
            .map(|s| format!("Length: {}", cx.fmt_dim(s.dist(to))));
    }

    /// Adds the wall `start`..`end` and connects it to the plan (corners, Ts,
    /// crossings). Returns the new wall's id, the point the next wall of the
    /// chain starts from (the end after any corner adjustment), and whether
    /// the wall completed a new room.
    fn create(
        &mut self,
        cx: &mut EditorContext,
        start: Point,
        end: Point,
    ) -> Option<(Id, Point, bool)> {
        if start.dist(end) < MIN_LENGTH {
            return None;
        }
        cx.begin_change("Draw Wall");
        let fl = cx.floor;
        let rooms_before = detect_rooms(&cx.project.floors[fl].walls, 0.5).len();
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
        let reach = connect::MIN_CONNECT_DISTANCE.max(cx.wall_thickness(self.kind));
        connect::auto_connect(cx, id);
        // The next wall starts where this one really ended.
        let next = match cx.project.floors[fl].wall(id) {
            Some(w) if w.end.dist(end) <= reach => w.end,
            _ => end,
        };
        if cx.project.floors[fl].wall(id).is_some() {
            cx.selection.set(ObjectRef::Wall(id));
        }
        let rooms_after = detect_rooms(&cx.project.floors[fl].walls, 0.5).len();
        cx.mark_dirty();
        Some((id, next, rooms_after > rooms_before))
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
        self.end_chain();
        self.press = None;
        self.hover = None;
        cx.readout = None;
        cx.last_snap = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let (s, _) = self.snap(cx, &p);
        self.hover = Some(s.point);
        cx.last_snap = Some(s);
        self.update_readout(cx, s.point);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let (s, _) = self.snap(cx, &p);
        self.hover = Some(s.point);
        let started_chain = self.pending.is_none();
        if started_chain {
            self.pending = Some(s.point);
            self.chain_first = Some(s.point);
            self.chain_walls = 0;
            self.last_dir = None;
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
        let (snap, closing) = self.snap(cx, &p);
        let end = snap.point;
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
            self.end_chain();
            cx.readout = None;
            return match made {
                Some((_, _, room)) => {
                    if room {
                        cx.status = "Room created".into();
                    }
                    ToolResult::committed("Draw Wall")
                }
                None => ToolResult::consumed(),
            };
        }
        let Some((_, next, room)) = self.create(cx, start, end) else {
            return ToolResult::consumed();
        };
        self.chain_walls += 1;
        self.last_dir = Some((end - start).normalized());
        if room {
            cx.status = "Room created".into();
        }
        let first = self.chain_first;
        let closed =
            closing || first.is_some_and(|f| self.chain_walls >= 3 && next.dist(f) <= JOIN_TOL);
        if closed {
            // The loop is closed: the chain ends and nothing stays selected.
            self.end_chain();
            cx.selection.clear();
            cx.readout = None;
            cx.last_snap = None;
            self.hover = None;
        } else {
            self.pending = Some(next);
            self.update_readout(cx, next);
        }
        ToolResult::committed("Draw Wall")
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(egui::Key::Escape) && self.pending.is_some() {
            self.end_chain();
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

    /// Press-drag-release one wall from `a` to `b`.
    fn drag(t: &mut WallTool, cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) {
        let pa = PointerEvent::at(cx, Point::new(a.0, a.1));
        let pb = PointerEvent::at(cx, Point::new(b.0, b.1));
        t.pointer_down(cx, pa.with_down(true));
        t.pointer_move(cx, pb.with_down(true));
        t.pointer_up(cx, pb);
    }

    #[test]
    fn sloppy_chain_closes_on_the_start_point_with_exact_corners() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        // Each click is 2-3" off the ideal corner; the last lands near the start.
        for (x, y) in [
            (0.0, 0.0),
            (237.0, 1.0),
            (243.0, 147.0),
            (-3.0, 141.0),
            (2.0, -2.0),
        ] {
            click(&mut t, &mut cx, x, y);
        }
        assert!(t.pending_start().is_none(), "the chain ended");
        assert!(cx.selection.is_empty(), "nothing stays selected");
        assert_eq!(cx.status, "Room created");
        let walls = &cx.floor().walls;
        assert_eq!(walls.len(), 4);
        // The closing click snapped exactly onto the first point.
        assert_eq!(walls[3].end, walls[0].start);
        assert_eq!(walls[0].start, Point::new(0.0, 0.0));
        // Every corner is shared exactly by two walls.
        let ends: Vec<Point> = walls.iter().flat_map(|w| [w.start, w.end]).collect();
        for e in &ends {
            assert_eq!(ends.iter().filter(|o| *o == e).count(), 2, "corner {e:?}");
        }
        cx.refresh();
        assert_eq!(cx.rooms.len(), 1);
    }

    #[test]
    fn closing_snap_reports_an_endpoint_marker_near_the_start() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        for (x, y) in [(0.0, 0.0), (240.0, 0.0), (240.0, 144.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let p = PointerEvent::at(&cx, Point::new(3.0, 2.0));
        t.pointer_move(&mut cx, p);
        let s = cx.last_snap.unwrap();
        assert_eq!((s.kind, s.point), (SnapKind::Endpoint, Point::ZERO));
    }

    #[test]
    fn drag_drawn_rectangle_with_gaps_beyond_the_snap_distance_still_closes() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        // Gaps of 6-7" are outside the 5" snap but inside the 7 5/8" connect distance.
        drag(&mut t, &mut cx, (0.0, 0.0), (234.0, 0.0));
        drag(&mut t, &mut cx, (238.0, -5.0), (238.0, 138.0));
        drag(&mut t, &mut cx, (244.0, 142.0), (2.0, 142.0));
        drag(&mut t, &mut cx, (0.0, 148.0), (0.0, 6.0));
        let walls = &cx.floor().walls;
        assert_eq!(walls.len(), 4);
        let ends: Vec<Point> = walls.iter().flat_map(|w| [w.start, w.end]).collect();
        for e in &ends {
            assert_eq!(ends.iter().filter(|o| *o == e).count(), 2, "corner {e:?}");
        }
        cx.refresh();
        assert_eq!(cx.rooms.len(), 1);
        // Each wall was one undo step, and drawing the last one included its fixes.
        assert_eq!(cx.undo().as_deref(), Some("Draw Wall"));
        assert_eq!(cx.floor().walls.len(), 3);
    }

    #[test]
    fn a_wall_ending_near_a_centerline_makes_a_tee_and_splits_it() {
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
        // 6" above the centerline: too far for the snap, close enough to connect.
        drag(&mut t, &mut cx, (100.0, 120.0), (100.0, 6.0));
        let walls = &cx.floor().walls;
        assert_eq!(walls.len(), 3);
        assert_eq!(
            cx.floor().wall(through).unwrap().end,
            Point::new(100.0, 0.0)
        );
        let stem = walls.iter().find(|w| w.start.y == 120.0).unwrap();
        assert_eq!(stem.end, Point::new(100.0, 0.0));
    }

    #[test]
    fn a_wall_drawn_across_another_cuts_both() {
        let mut cx = new_cx();
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(200.0, 0.0),
            7.625,
            109.125,
            WallKind::Exterior,
        );
        let mut t = WallTool::default();
        drag(&mut t, &mut cx, (100.0, -50.0), (100.0, 50.0));
        assert_eq!(cx.floor().walls.len(), 4);
    }

    #[test]
    fn alignment_with_the_previous_wall_snaps_perpendicular_and_collinear() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        // A wall at 30 degrees; its perpendicular is 120 degrees.
        let a = 30f64.to_radians();
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 200.0 * a.cos(), 200.0 * a.sin());
        let start = t.pending_start().unwrap();
        let perp = Point::new(-a.sin(), a.cos());
        // Cursor 2" off the perpendicular, 100" along it.
        let raw = start + perp * 100.0 + Point::new(a.cos(), a.sin()) * 2.0;
        let p = PointerEvent::at(&cx, raw);
        t.pointer_move(&mut cx, p);
        let s = cx.last_snap.unwrap();
        assert_eq!(s.kind, SnapKind::Perpendicular);
        let v = s.point - start;
        assert!(v.dot(Point::new(a.cos(), a.sin())).abs() < 1e-6);
    }
}
