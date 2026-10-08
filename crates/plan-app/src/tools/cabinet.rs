//! Cabinet tools (CB-1..CB-9 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! Variants: Base, Wall, Full Height, Soffit, Shelf and Partition (CB-1);
//! Custom Countertop is a stub. Behavior:
//!
//! * a click places a cabinet with its back against the nearest wall within
//!   12", rotated to the wall and flush to its face; away from walls it is
//!   free at the click with the tool's angle (CB-2, CB-3);
//! * a cabinet placed or moved next to another slides to butt against it and
//!   aligns its back line (CB-4);
//! * click-drag sets the width in 3" steps (CB-3, implemented as the width of
//!   one cabinet);
//! * a placed cabinet becomes the selection (Shift-click toggles one; the
//!   Select tool picks the rest via `placed::hit_placed`): handles are Move,
//!   Resize width (both ends,
//!   3" steps, the cabinet grows from the dragged side) and Rotate (CB-8,
//!   CB-9); dragging keeps the rotation until it bumps a wall, where it
//!   re-rotates (Ctrl suspends that);
//! * double-click or Enter opens the Cabinet Specification; the Edit toolbar
//!   offers Open Object, Delete, Copy and Reverse Door Swing.
//!
//! The variant comes from `ToolId::CabinetVariant(kind)` (flyout entries and
//! hotkeys), or with Tab while the tool is active.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::handles::{self, hit_handle, Handle, HandleKind};
use crate::editor::placed::{
    self, add_cabinet, cabinet_by_id, hit_cabinet, load_cabinets, placed_handles, replace_cabinet,
    same_angle, PlacedRef,
};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Key, Pos2};
use plan_cabinets::{Cabinet, CabinetKind, FaceLayout, HandleStyle};
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::{Floor, Id};
use std::f64::consts::FRAC_PI_2;

/// A click within this distance of a wall face places the cabinet on it, in.
pub const WALL_REACH: f64 = 12.0;
/// Click-drag and resize steps, inches (CB-3, CB-8).
pub const WIDTH_STEP: f64 = 3.0;
/// Smallest cabinet width, inches.
const MIN_WIDTH: f64 = 3.0;
/// Pixels the pointer must travel before a press becomes a drag.
const DRAG_THRESHOLD_PX: f32 = 3.0;

/// The Cabinet flyout, in order.
pub const KINDS: [CabinetKind; 6] = [
    CabinetKind::Base,
    CabinetKind::Wall,
    CabinetKind::FullHeight,
    CabinetKind::Soffit,
    CabinetKind::Shelf,
    CabinetKind::Partition,
];

pub fn kind_name(kind: CabinetKind) -> &'static str {
    match kind {
        CabinetKind::Base => "Base Cabinet",
        CabinetKind::Wall => "Wall Cabinet",
        CabinetKind::FullHeight => "Full Height Cabinet",
        CabinetKind::Soffit => "Soffit",
        CabinetKind::Shelf => "Shelf",
        CabinetKind::Partition => "Partition",
    }
}

fn handle_style(name: &str) -> HandleStyle {
    match name.to_lowercase().as_str() {
        "none" => HandleStyle::None,
        "pull" => HandleStyle::Pull,
        _ => HandleStyle::Knob,
    }
}

/// A new cabinet of `kind` from the plan's cabinet defaults (CB-6, CB-20),
/// at the origin with no id.
pub fn default_cabinet(cx: &EditorContext, kind: CabinetKind) -> Cabinet {
    let d = &cx.defaults.cabinets;
    match kind {
        CabinetKind::Base => {
            let b = &d.base;
            let mut c = Cabinet::base(b.width);
            c.depth = b.depth;
            c.height = b.height;
            if let Some(t) = c.countertop.as_mut() {
                t.thickness = b.countertop_thickness;
                t.overhang_front = b.countertop_overhang;
            }
            if let Some(t) = c.toe_kick.as_mut() {
                t.height = b.toe_kick_height;
                t.depth = b.toe_kick_depth;
            }
            c.door_style.name = b.door_style.clone();
            c.door_style.handle = handle_style(&b.handle);
            c.drawer_style.name = b.drawer_style.clone();
            c.drawer_style.handle = handle_style(&b.handle);
            c.face = FaceLayout::base_default(c.face_height());
            c
        }
        CabinetKind::Wall => {
            let w = &d.wall;
            let mut c = Cabinet::wall(w.width);
            c.depth = w.depth;
            c.height = w.height;
            c.elevation = w.elevation;
            c.face = FaceLayout::wall_default(c.face_height());
            c
        }
        CabinetKind::FullHeight => {
            let f = &d.full_height;
            let mut c = Cabinet::full_height(f.width);
            c.depth = f.depth;
            c.height = f.height;
            c.face = FaceLayout::full_height_default(c.face_height());
            c
        }
        other => Cabinet::new(other, 24.0),
    }
}

// ----- wall placement and bumping -----

struct WallHit {
    dir: Point,
    normal: Point,
    start: Point,
    thickness: f64,
    side: f64,
    /// Distance of the anchor's projection from the wall start.
    along: f64,
}

/// The wall whose face is nearest `anchor` within `reach` (CB-3: near a
/// corner, the wall the cursor is closer to).
fn nearest_wall(cx: &EditorContext, anchor: Point, reach: f64) -> Option<WallHit> {
    let mut best: Option<(f64, WallHit)> = None;
    for w in cx
        .floor()
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && w.length() > 1e-9 && cx.layers().is_visible(&w.layer))
    {
        let d = (dist_to_segment(anchor, w.start, w.end) - w.thickness * 0.5).max(0.0);
        if d > reach || best.as_ref().is_some_and(|(bd, _)| d >= *bd) {
            continue;
        }
        let (t, q) = project_on_segment(anchor, w.start, w.end);
        let side = if anchor.sub(q).dot(w.normal()) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        best = Some((
            d,
            WallHit {
                dir: w.direction(),
                normal: w.normal(),
                start: w.start,
                thickness: w.thickness,
                side,
                along: t * w.length(),
            },
        ));
    }
    best.map(|(_, h)| h)
}

fn vertical_overlap(a: &Cabinet, b: &Cabinet) -> bool {
    a.elevation.max(b.elevation) < (a.elevation + a.height).min(b.elevation + b.height) - 0.5
}

/// Slides `cab` along its width axis to butt against the cabinets it overlaps
/// (same angle, overlapping heights and depths) and aligns its back line with
/// the neighbor's when they are within 6" (CB-4).
pub fn bump(others: &[Cabinet], cab: &mut Cabinet, exclude: Id) {
    let u = Point::new(cab.angle.cos(), cab.angle.sin());
    let v = u.perp();
    let mut bumped_into: Option<Cabinet> = None;
    for _ in 0..8 {
        let s0 = cab.position.dot(u);
        let s1 = s0 + cab.width;
        let t0 = cab.position.dot(v);
        let t1 = t0 + cab.depth;
        let mut shifted = false;
        for o in others.iter().filter(|o| {
            o.id != exclude && same_angle(o.angle, cab.angle) && vertical_overlap(o, cab)
        }) {
            let (os0, ot0) = (o.position.dot(u), o.position.dot(v));
            if t1.min(ot0 + o.depth) - t0.max(ot0) <= 0.5 {
                continue;
            }
            if s1.min(os0 + o.width) - s0.max(os0) <= 0.01 {
                continue;
            }
            let right = os0 + o.width - s0;
            let left = os0 - s1;
            let shift = if right.abs() <= left.abs() {
                right
            } else {
                left
            };
            cab.position = cab.position + u * shift;
            bumped_into = Some(o.clone());
            shifted = true;
            break;
        }
        if !shifted {
            break;
        }
    }
    if let Some(o) = bumped_into {
        let dt = o.position.dot(v) - cab.position.dot(v);
        if dt.abs() <= 6.0 && dt.abs() > 1e-9 {
            cab.position = cab.position + v * dt;
        }
    }
}

fn snap_to(v: f64, unit: f64, alt: bool) -> f64 {
    if alt {
        v
    } else {
        (v / unit).round() * unit
    }
}

/// Settles `cab` (width, depth, angle already set) around `anchor`: against
/// the nearest wall within `reach` (rotated, flush, centered on the anchor's
/// projection and snapped to the grid unit), else free around `free_center`;
/// then bumped against its neighbors. Returns whether it is on a wall.
#[allow(clippy::too_many_arguments)]
pub fn settle(
    cx: &EditorContext,
    cab: &mut Cabinet,
    anchor: Point,
    free_center: Point,
    reach: f64,
    use_walls: bool,
    alt: bool,
    exclude: Id,
) -> bool {
    let unit = cx.snap_unit();
    let hit = if use_walls {
        nearest_wall(cx, anchor, reach)
    } else {
        None
    };
    let on_wall = hit.is_some();
    match hit {
        Some(h) => {
            let dir_u = if h.side > 0.0 { h.dir } else { h.dir * -1.0 };
            cab.angle = dir_u.angle();
            let back_at = |s: f64| h.start + h.dir * s + h.normal * (h.side * h.thickness * 0.5);
            // The cabinet's left edge in the width direction (u) is its
            // back-left corner.
            let left = if h.side > 0.0 {
                snap_to(h.along - cab.width * 0.5, unit, alt)
            } else {
                snap_to(h.along + cab.width * 0.5, unit, alt)
            };
            cab.position = back_at(left);
        }
        None => {
            let u = Point::new(cab.angle.cos(), cab.angle.sin());
            cab.position = free_center - u * (cab.width * 0.5) - u.perp() * (cab.depth * 0.5);
        }
    }
    bump(&load_cabinets(cx.floor()), cab, exclude);
    on_wall
}

// ----- the tool -----

/// A click that may become a width drag.
struct Press {
    start: Point,
    screen: Pos2,
    angle: f64,
    cab: Cabinet,
    dragged: bool,
}

/// A handle drag of a selected cabinet.
struct EditDrag {
    op: HandleKind,
    original: Cabinet,
    start: Point,
    screen: Pos2,
    begun: bool,
}

pub struct CabinetTool {
    kind: CabinetKind,
    ghost: Option<Cabinet>,
    press: Option<Press>,
    edit: Option<EditDrag>,
}

impl Default for CabinetTool {
    fn default() -> Self {
        Self {
            kind: CabinetKind::Base,
            ghost: None,
            press: None,
            edit: None,
        }
    }
}

impl CabinetTool {
    pub fn kind(&self) -> CabinetKind {
        self.kind
    }

    pub fn set_kind(&mut self, kind: CabinetKind) {
        self.kind = kind;
        self.ghost = None;
    }

    /// A fresh cabinet of the active variant placed for a click at `p`.
    fn placed_at(&self, cx: &EditorContext, p: &PointerEvent) -> Cabinet {
        let mut cab = default_cabinet(cx, self.kind);
        settle(
            cx,
            &mut cab,
            p.world,
            p.snapped,
            WALL_REACH,
            true,
            p.modifiers.alt,
            0,
        );
        cab
    }

    fn selected_cabinet(cx: &EditorContext) -> Option<Id> {
        match cx.selection.single()? {
            ObjectRef::Cabinet(id) => Some(id),
            _ => None,
        }
    }

    fn handles(cx: &EditorContext) -> Vec<Handle> {
        Self::selected_cabinet(cx)
            .map(|id| placed_handles(cx.floor(), PlacedRef::Cabinet(id), cx.px_per_in))
            .unwrap_or_default()
    }

    fn cancel(&mut self, cx: &mut EditorContext) -> bool {
        let mut did = self.press.take().is_some();
        if let Some(e) = self.edit.take() {
            did = true;
            if e.begun {
                let fl = cx.floor;
                replace_cabinet(&mut cx.project, fl, &e.original);
                cx.cancel_change();
                cx.mark_dirty();
            }
        }
        self.ghost = None;
        cx.readout = None;
        did
    }

    fn open_spec(cx: &mut EditorContext, id: Id) {
        cx.selection.set(ObjectRef::Cabinet(id));
        cx.requests
            .push(EditorRequest::OpenSpec(ObjectRef::Cabinet(id)));
    }
}

fn op_label(op: HandleKind) -> &'static str {
    match op {
        HandleKind::Rotate => "Rotate Cabinet",
        HandleKind::ResizeStart | HandleKind::ResizeEnd => "Resize Cabinet",
        _ => "Move Cabinet",
    }
}

/// The cabinet after dragging handle `op` from `start` to `p` (CB-8, CB-9).
pub fn apply_edit(
    cx: &EditorContext,
    op: HandleKind,
    orig: &Cabinet,
    start: Point,
    p: &PointerEvent,
) -> Cabinet {
    let mut c = orig.clone();
    let alt = p.modifiers.alt;
    let u = Point::new(orig.angle.cos(), orig.angle.sin());
    let local_center = Point::new(orig.width * 0.5, orig.depth * 0.5);
    match op {
        HandleKind::Move => {
            let delta = p.world.sub(start);
            let anchor = orig.to_plan(local_center) + delta;
            let unit = cx.snap_unit();
            let center = Point::new(snap_to(anchor.x, unit, alt), snap_to(anchor.y, unit, alt));
            c.position = orig.position + delta;
            let ctrl = p.modifiers.command || p.modifiers.ctrl;
            settle(
                cx,
                &mut c,
                anchor,
                center,
                WALL_REACH + orig.depth * 0.5,
                !ctrl,
                alt,
                orig.id,
            );
        }
        HandleKind::ResizeEnd => {
            let s = p.world.sub(orig.position).dot(u);
            c.width = snap_to(s, WIDTH_STEP, alt).max(MIN_WIDTH);
        }
        HandleKind::ResizeStart => {
            let s = p.world.sub(orig.position).dot(u);
            let w = snap_to(orig.width - s, WIDTH_STEP, alt).max(MIN_WIDTH);
            c.width = w;
            c.position = orig.position + u * (orig.width - w);
        }
        HandleKind::Rotate => {
            let center = orig.to_plan(local_center);
            let d = p.world.sub(center);
            if d.length() > 1e-6 {
                let step = if cx.defaults.grid.angle_snap_deg > 0.0 {
                    cx.defaults.grid.angle_snap_deg.to_radians()
                } else {
                    15.0_f64.to_radians()
                };
                c.angle = snap_to(d.angle() - FRAC_PI_2, step, alt);
                let (s, co) = c.angle.sin_cos();
                let (hx, hy) = (local_center.x, local_center.y);
                c.position =
                    Point::new(center.x - (hx * co - hy * s), center.y - (hx * s + hy * co));
            }
        }
        _ => {}
    }
    c
}

impl Tool for CabinetTool {
    fn id(&self) -> ToolId {
        ToolId::Cabinet
    }

    fn name(&self) -> &'static str {
        kind_name(self.kind)
    }

    fn hint(&self) -> String {
        format!(
            "{}: click to place, drag to set the width; Tab changes the cabinet type",
            kind_name(self.kind)
        )
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::CabinetVariant(k) = id {
            self.set_kind(k);
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = self.hint();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.cancel(cx);
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if p.down {
            if let Some(e) = &mut self.edit {
                let moved = (p.screen - e.screen).length() >= DRAG_THRESHOLD_PX;
                if !e.begun && !moved {
                    return ToolResult::consumed();
                }
                if !e.begun {
                    cx.begin_change(op_label(e.op));
                    e.begun = true;
                }
                let next = apply_edit(cx, e.op, &e.original, e.start, &p);
                let fl = cx.floor;
                replace_cabinet(&mut cx.project, fl, &next);
                cx.mark_dirty();
                cx.readout = Some(format!("Width: {}", cx.fmt_dim(next.width)));
                return ToolResult::consumed();
            }
            if let Some(pr) = &mut self.press {
                let u = Point::new(pr.angle.cos(), pr.angle.sin());
                let du = p.world.sub(pr.start).dot(u);
                if pr.dragged || (p.screen - pr.screen).length() >= DRAG_THRESHOLD_PX {
                    pr.dragged = true;
                    let width = (du.abs() / WIDTH_STEP).round().max(1.0) * WIDTH_STEP;
                    let sign = if du >= 0.0 { 1.0 } else { -1.0 };
                    let center = pr.start + u * (sign * width * 0.5);
                    let mut cab = default_cabinet(cx, self.kind);
                    cab.width = width;
                    cab.angle = pr.angle;
                    settle(
                        cx,
                        &mut cab,
                        center,
                        center,
                        WALL_REACH,
                        true,
                        p.modifiers.alt,
                        0,
                    );
                    cx.readout = Some(format!("Width: {}", cx.fmt_dim(width)));
                    pr.cab = cab.clone();
                    self.ghost = Some(cab);
                }
                return ToolResult::consumed();
            }
        }
        self.ghost = Some(self.placed_at(cx, &p));
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let tol = cx.pick_tol();
        // A handle of the selected cabinet.
        if let (Some(id), Some(h)) = (
            Self::selected_cabinet(cx),
            hit_handle(&Self::handles(cx), p.world, tol),
        ) {
            if let Some(original) = cabinet_by_id(cx.floor(), id) {
                self.edit = Some(EditDrag {
                    op: h.kind,
                    original,
                    start: p.world,
                    screen: p.screen,
                    begun: false,
                });
                return ToolResult::consumed();
            }
        }
        // Shift-click toggles a cabinet of this variant's height range. Any
        // other click places a new cabinet, which bumps against whatever is
        // under it; a selected cabinet moves with its center handle.
        if p.modifiers.shift {
            let probe = default_cabinet(cx, self.kind);
            if let Some(id) = hit_cabinet(cx, p.world, 0.0, |c| vertical_overlap(c, &probe)) {
                cx.selection.toggle(ObjectRef::Cabinet(id));
                return ToolResult::consumed();
            }
        }
        // Otherwise a placement: committed on release (a click or a drag).
        let cab = self.placed_at(cx, &p);
        self.press = Some(Press {
            start: p.world,
            screen: p.screen,
            angle: cab.angle,
            cab: cab.clone(),
            dragged: false,
        });
        self.ghost = Some(cab);
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        if let Some(e) = self.edit.take() {
            return if e.begun {
                cx.readout = None;
                ToolResult::committed(op_label(e.op))
            } else {
                ToolResult::consumed()
            };
        }
        let Some(pr) = self.press.take() else {
            return ToolResult::ignored();
        };
        cx.readout = None;
        let label = format!("Place {}", kind_name(self.kind));
        cx.begin_change(&label);
        let fl = cx.floor;
        match add_cabinet(&mut cx.project, fl, pr.cab) {
            Some(id) => {
                cx.selection.set(ObjectRef::Cabinet(id));
                cx.mark_dirty();
                cx.status.clear();
                ToolResult::committed(&label)
            }
            None => {
                cx.cancel_change();
                cx.status = "The plan's cabinets could not be read".into();
                ToolResult::consumed()
            }
        }
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        match hit_cabinet(cx, p.world, cx.pick_tol(), |_| true) {
            Some(id) => {
                Self::open_spec(cx, id);
                ToolResult::consumed()
            }
            None => ToolResult::ignored(),
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Escape) {
            return if self.cancel(cx) {
                ToolResult::consumed()
            } else {
                ToolResult::ignored()
            };
        }
        if k.is(Key::Tab) && self.press.is_none() && self.edit.is_none() {
            let i = KINDS.iter().position(|x| *x == self.kind).unwrap_or(0);
            self.set_kind(KINDS[(i + 1) % KINDS.len()]);
            cx.status = self.hint();
            return ToolResult::consumed();
        }
        if k.is(Key::Delete) || k.is(Key::Backspace) {
            if placed::delete_placed(cx) > 0 {
                return ToolResult::committed("Delete");
            }
            return ToolResult::ignored();
        }
        if k.is(Key::Enter) {
            if let Some(id) = Self::selected_cabinet(cx) {
                Self::open_spec(cx, id);
                return ToolResult::consumed();
            }
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        if let Some(g) = &self.ghost {
            if self.edit.is_none() {
                placed::draw_cabinet(painter, cam, g, pal.ghost_stroke, false);
            }
        }
        handles::draw(&Self::handles(cx), painter, cam, pal);
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        let mut v = cx.common_edit_actions();
        if cx
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Cabinet(_)))
        {
            let mut a = EditAction::new(EditActionKind::ReverseSwing);
            a.label = "Reverse Door Swing";
            v.push(a);
        }
        v
    }
}

/// Custom Countertop (CB-15) arrives with the polyline tools.
pub const CUSTOM_COUNTERTOP_NOTE: &str = "Custom Countertop: not yet implemented";

/// Is the floor's cabinet list readable? (A foreign entry would make edits
/// refuse rather than drop it.)
pub fn cabinets_readable(floor: &Floor) -> bool {
    floor.cabinets_as::<Cabinet>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;
    use std::f64::consts::PI;

    fn setup() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        cx
    }

    fn click(t: &mut CabinetTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        let r = t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
        r
    }

    fn cabs(cx: &EditorContext) -> Vec<Cabinet> {
        load_cabinets(cx.floor())
    }

    #[test]
    fn placing_near_a_wall_rotates_and_sits_flush_to_the_face() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        // 10" above the wall centerline: within 12" of the face (face y = 3).
        click(&mut t, &mut cx, 60.0, 10.0);
        let c = &cabs(&cx)[0];
        assert!(same_angle(c.angle, 0.0));
        // Back on the wall's upper face, centered on the click.
        assert!((c.position.y - 3.0).abs() < 1e-9, "{:?}", c.position);
        assert!((c.position.x - 48.0).abs() < 1e-9);
        assert_eq!((c.width, c.depth, c.height), (24.0, 24.0, 36.0));
        // Below the wall the cabinet turns around and its front faces -y.
        click(&mut t, &mut cx, 60.0, -10.0);
        let c2 = &cabs(&cx)[1];
        assert!(same_angle(c2.angle, PI), "{}", c2.angle);
        assert!((c2.position.y + 3.0).abs() < 1e-9);
        let corners = c2.corners();
        assert!(corners.iter().all(|q| q.y <= -3.0 + 1e-9), "{corners:?}");
        assert!(corners.iter().any(|q| (q.y + 27.0).abs() < 1e-9));
        // The click selected/placed object is the new cabinet.
        assert_eq!(cx.selection.single(), Some(ObjectRef::Cabinet(c2.id)));
    }

    #[test]
    fn far_from_walls_the_cabinet_is_free_with_angle_zero() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 120.0, 100.0);
        let c = &cabs(&cx)[0];
        assert_eq!(c.angle, 0.0);
        let center = c.to_plan(Point::new(12.0, 12.0));
        let snapped = PointerEvent::at(&cx, Point::new(120.0, 100.0)).snapped;
        assert!(center.dist(snapped) < 1e-9);
    }

    #[test]
    fn two_clicks_on_the_same_wall_bump_together() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0);
        click(&mut t, &mut cx, 65.0, 10.0);
        let list = cabs(&cx);
        assert_eq!(list.len(), 2);
        // The second wants x = 53..77, overlaps 48..72 and slides to 72.
        assert!(
            (list[1].position.x - 72.0).abs() < 1e-9,
            "{:?}",
            list[1].position
        );
        assert!((list[1].position.y - list[0].position.y).abs() < 1e-9);
        let gap = list[1].position.x - (list[0].position.x + list[0].width);
        assert!(gap.abs() < 1e-9);
    }

    #[test]
    fn wall_cabinets_do_not_bump_into_base_cabinets() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0);
        t.set_kind(CabinetKind::Wall);
        cx.selection.clear(); // otherwise the click would land on the move handle
        click(&mut t, &mut cx, 60.0, 10.0);
        let list = cabs(&cx);
        assert_eq!(list.len(), 2);
        assert!((list[1].position.x - 48.0).abs() < 1e-9);
        assert_eq!((list[1].depth, list[1].elevation), (12.0, 54.0));
    }

    #[test]
    fn drag_sets_the_width_in_three_inch_steps() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        let start = Point::new(60.0, 10.0);
        let down = PointerEvent::at(&cx, start);
        t.pointer_move(&mut cx, down);
        t.pointer_down(&mut cx, down.with_down(true));
        // Drag 41" along the wall: rounds to 42".
        let mut mv = PointerEvent::at(&cx, Point::new(101.0, 10.0)).with_down(true);
        mv.screen = Pos2::new(300.0, 0.0);
        t.pointer_move(&mut cx, mv);
        let r = t.pointer_up(&mut cx, mv);
        assert!(r.commit.is_some());
        let c = &cabs(&cx)[0];
        assert_eq!(c.width, 42.0);
        // The left edge is at the press point, flush to the wall.
        assert!((c.position.x - 60.0).abs() < 1e-9, "{:?}", c.position);
        assert!((c.position.y - 3.0).abs() < 1e-9);
        // Dragging the other way grows to the left.
        let down = PointerEvent::at(&cx, Point::new(200.0, 10.0));
        t.pointer_move(&mut cx, down);
        t.pointer_down(&mut cx, down.with_down(true));
        let mut mv = PointerEvent::at(&cx, Point::new(179.0, 10.0)).with_down(true);
        mv.screen = Pos2::new(-300.0, 0.0);
        t.pointer_move(&mut cx, mv);
        t.pointer_up(&mut cx, mv);
        let c = &cabs(&cx)[1];
        assert_eq!(c.width, 21.0);
        assert!((c.position.x - 179.0).abs() < 1e-9, "{:?}", c.position);
    }

    #[test]
    fn undo_removes_the_placed_cabinet() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        let r = click(&mut t, &mut cx, 60.0, 10.0);
        assert_eq!(r.commit, None); // the press itself commits nothing
        assert_eq!(cabs(&cx).len(), 1);
        assert_eq!(cx.undo_label(), Some("Place Base Cabinet"));
        assert_eq!(cx.undo().as_deref(), Some("Place Base Cabinet"));
        assert!(cabs(&cx).is_empty());
        cx.redo();
        assert_eq!(cabs(&cx).len(), 1);
    }

    #[test]
    fn resize_handles_snap_to_three_inches_and_grow_from_the_dragged_side() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0);
        let orig = cabs(&cx)[0].clone();
        let id = orig.id;
        let drag = |t: &mut CabinetTool, cx: &mut EditorContext, kind: HandleKind, dx: f64| {
            let h = placed_handles(cx.floor(), PlacedRef::Cabinet(id), cx.px_per_in)
                .into_iter()
                .find(|h| h.kind == kind)
                .unwrap();
            let mut down = PointerEvent::at(cx, h.pos).with_down(true);
            down.screen = Pos2::new(0.0, 0.0);
            t.pointer_down(cx, down);
            let mut mv = PointerEvent::at(cx, h.pos + Point::new(dx, 0.0)).with_down(true);
            mv.screen = Pos2::new(50.0, 0.0);
            t.pointer_move(cx, mv);
            t.pointer_up(cx, mv)
        };
        let r = drag(&mut t, &mut cx, HandleKind::ResizeEnd, 7.0);
        assert_eq!(r.commit.as_deref(), Some("Resize Cabinet"));
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!(c.width, 30.0); // 24 + 7 -> 31 -> nearest 3" step
        assert_eq!(c.position, orig.position);
        // Left handle: the right edge stays put.
        let right_edge = c.position.x + c.width;
        drag(&mut t, &mut cx, HandleKind::ResizeStart, -9.0);
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!(c.width, 39.0);
        assert!((c.position.x + c.width - right_edge).abs() < 1e-9);
        assert_eq!(cx.undo().as_deref(), Some("Resize Cabinet"));
        assert_eq!(cabinet_by_id(cx.floor(), id).unwrap().width, 30.0);
    }

    #[test]
    fn dragging_a_cabinet_keeps_rotation_until_it_bumps_a_wall() {
        let mut cx = setup();
        cx.project.add_wall(
            0,
            Point::new(0.0, 200.0),
            Point::new(240.0, 200.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0); // on the lower wall, angle 0
        let id = cabs(&cx)[0].id;
        let center = cabs(&cx)[0].to_plan(Point::new(12.0, 12.0));
        let mut down = PointerEvent::at(&cx, center).with_down(true);
        down.screen = Pos2::new(0.0, 0.0);
        t.pointer_down(&mut cx, down);
        // Move well into the room, then up against the upper wall.
        let mut mv = PointerEvent::at(&cx, Point::new(60.0, 110.0)).with_down(true);
        mv.screen = Pos2::new(0.0, 100.0);
        t.pointer_move(&mut cx, mv);
        let mid = cabinet_by_id(cx.floor(), id).unwrap();
        assert!(same_angle(mid.angle, 0.0), "keeps its angle in open floor");
        let mut mv = PointerEvent::at(&cx, Point::new(60.0, 185.0)).with_down(true);
        mv.screen = Pos2::new(0.0, 200.0);
        t.pointer_move(&mut cx, mv);
        let top = cabinet_by_id(cx.floor(), id).unwrap();
        assert!(
            same_angle(top.angle, PI),
            "re-rotates to the upper wall: {}",
            top.angle
        );
        assert!((top.position.y - 197.0).abs() < 1e-9, "{:?}", top.position);
        t.pointer_up(&mut cx, mv);
        assert_eq!(cx.undo().as_deref(), Some("Move Cabinet"));
    }

    #[test]
    fn rotate_handle_turns_the_cabinet_about_its_center() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 120.0, 100.0);
        let orig = cabs(&cx)[0].clone();
        let center = orig.to_plan(Point::new(12.0, 12.0));
        let h = placed_handles(cx.floor(), PlacedRef::Cabinet(orig.id), cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == HandleKind::Rotate)
            .unwrap();
        let mut down = PointerEvent::at(&cx, h.pos).with_down(true);
        down.screen = Pos2::new(0.0, 0.0);
        t.pointer_down(&mut cx, down);
        let mut mv = PointerEvent::at(&cx, center + Point::new(-40.0, 0.0)).with_down(true);
        mv.screen = Pos2::new(60.0, 60.0);
        t.pointer_move(&mut cx, mv);
        t.pointer_up(&mut cx, mv);
        let c = cabinet_by_id(cx.floor(), orig.id).unwrap();
        // Front handle dragged to -x: the front now faces -x (angle 90 degrees).
        assert!(same_angle(c.angle, FRAC_PI_2), "{}", c.angle);
        assert!(c.to_plan(Point::new(12.0, 12.0)).dist(center) < 1e-9);
    }

    #[test]
    fn double_click_tab_edit_toolbar_and_delete() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0);
        let id = cabs(&cx)[0].id;
        let ev = PointerEvent::at(&cx, Point::new(60.0, 15.0));
        t.double_click(&mut cx, ev);
        assert!(cx
            .requests
            .contains(&EditorRequest::OpenSpec(ObjectRef::Cabinet(id))));
        let labels: Vec<&str> = t.edit_toolbar(&cx).iter().map(|a| a.label).collect();
        for want in ["Open Object", "Delete Objects", "Reverse Door Swing"] {
            assert!(labels.contains(&want), "{labels:?}");
        }
        assert!(t.key(&mut cx, KeyEvent::key(Key::Tab)).consumed);
        assert_eq!(t.kind(), CabinetKind::Wall);
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete"));
        assert!(cabs(&cx).is_empty());
    }

    #[test]
    fn requested_kind_applies_on_set_variant() {
        let mut t = CabinetTool::default();
        t.set_variant(ToolId::CabinetVariant(CabinetKind::Partition));
        assert_eq!(t.kind(), CabinetKind::Partition);
        assert_eq!(t.name(), "Partition");
        t.set_variant(ToolId::Cabinet);
        assert_eq!(t.kind(), CabinetKind::Partition);
    }

    #[test]
    fn defaults_come_from_the_plan_defaults() {
        let mut cx = setup();
        cx.defaults.cabinets.base.width = 30.0;
        cx.defaults.cabinets.wall.elevation = 60.0;
        assert_eq!(default_cabinet(&cx, CabinetKind::Base).width, 30.0);
        assert_eq!(default_cabinet(&cx, CabinetKind::Wall).elevation, 60.0);
        assert_eq!(default_cabinet(&cx, CabinetKind::FullHeight).height, 84.0);
        assert!(cabinets_readable(cx.floor()));
    }
}
