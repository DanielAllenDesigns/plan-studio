//! Slab and foundation tools: Slab, Slab with Footing, Slab Hole, Slab Hole
//! with Footing, Square Pad and Round Pier from the Slab flyout, and Hole in
//! Floor Platform and Hole in Ceiling Platform from the Floor flyout
//! (`docs/chief-x18-subtools.md`).
//!
//! One tool object with a flavor per flyout entry ([`FoundationVariant`]):
//!
//! * the polygon flavors work like Chief's Polyline: click the corners, a
//!   double-click (or Enter, or a click on the first corner) closes the
//!   shape. Dragging from the first click draws a rectangle instead, like
//!   Chief's Rectangular Polyline;
//! * Square Pad and Round Pier place one object per click;
//! * Ctrl/Cmd-click picks an existing foundation object, Ctrl/Cmd-drag
//!   moves it, Delete (or Backspace) deletes the selected or hovered one, and
//!   a double-click on one outside a drawing opens its specification dialog.
//!
//! Corners snap through the editor's snap engine. The objects live in the
//! floor's `FoundationLayer` through `editor::foundation_view`; every change
//! is one undo step.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::foundation::{Draft, FoundationDialog};
use crate::dialogs::Outcome;
use crate::editor::foundation_view as fv;
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Key, Stroke};
use plan_core::foundation::{
    outline_area, rect_outline, FoundationLayer, FoundationRef, Pad, Pier, PlatformKind, DATA_LAYER,
};
use plan_core::geometry::Point;
use std::cell::RefCell;

/// A drag shorter than this many screen pixels is a click.
const DRAG_PX: f64 = 6.0;

/// The flavors of the foundation tool, one per flyout entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FoundationVariant {
    Slab,
    SlabFooting,
    SlabHole,
    SlabHoleFooting,
    SquarePad,
    RoundPier,
    FloorHole,
    CeilingHole,
}

/// What the clicks of a flavor draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Draw {
    Polygon,
    Spot,
}

impl FoundationVariant {
    pub const ALL: [FoundationVariant; 8] = [
        FoundationVariant::Slab,
        FoundationVariant::SlabFooting,
        FoundationVariant::SlabHole,
        FoundationVariant::SlabHoleFooting,
        FoundationVariant::SquarePad,
        FoundationVariant::RoundPier,
        FoundationVariant::FloorHole,
        FoundationVariant::CeilingHole,
    ];

    /// Chief's name of the tool.
    pub fn name(self) -> &'static str {
        match self {
            FoundationVariant::Slab => "Slab",
            FoundationVariant::SlabFooting => "Slab with Footing",
            FoundationVariant::SlabHole => "Slab Hole",
            FoundationVariant::SlabHoleFooting => "Slab Hole with Footing",
            FoundationVariant::SquarePad => "Square Pad",
            FoundationVariant::RoundPier => "Round Pier",
            FoundationVariant::FloorHole => "Hole in Floor Platform",
            FoundationVariant::CeilingHole => "Hole in Ceiling Platform",
        }
    }

    fn draw(self) -> Draw {
        match self {
            FoundationVariant::SquarePad | FoundationVariant::RoundPier => Draw::Spot,
            _ => Draw::Polygon,
        }
    }
}

/// A move in progress (Ctrl/Cmd-drag).
struct MoveState {
    target: FoundationRef,
    start: Point,
    /// The layer as it was when the drag began; every step moves from it.
    base: FoundationLayer,
    moved: bool,
}

pub struct FoundationTool {
    variant: FoundationVariant,
    points: Vec<Point>,
    hover: Option<Point>,
    /// Where the button went down on an empty drawing (a possible rectangle).
    press: Option<Point>,
    /// The rectangle being dragged: first corner, current corner.
    rect: Option<(Point, Point)>,
    moving: Option<MoveState>,
    dialog: RefCell<Option<FoundationDialog>>,
    applied: RefCell<Option<Draft>>,
}

impl Default for FoundationTool {
    fn default() -> Self {
        Self {
            variant: FoundationVariant::Slab,
            points: Vec::new(),
            hover: None,
            press: None,
            rect: None,
            moving: None,
            dialog: RefCell::new(None),
            applied: RefCell::new(None),
        }
    }
}

impl FoundationTool {
    pub fn variant(&self) -> FoundationVariant {
        self.variant
    }

    /// The corners of the shape being drawn.
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog.borrow().is_some()
    }

    fn reset(&mut self, cx: &mut EditorContext) {
        self.points.clear();
        self.press = None;
        self.rect = None;
        cx.readout = None;
    }

    /// Applies an OK from the specification dialog, as one undo step.
    fn flush(&mut self, cx: &mut EditorContext) -> Option<ToolResult> {
        let draft = self.applied.borrow_mut().take()?;
        let label = draft.title();
        let mut found = false;
        fv::edit(cx, label, |l| found = draft.apply(l));
        if !found {
            cx.cancel_change();
            cx.status = "The object is gone".into();
            return Some(ToolResult::consumed());
        }
        Some(ToolResult::committed(label))
    }

    /// Opens the specification dialog of `r`.
    pub fn open_spec(&self, cx: &EditorContext, r: FoundationRef) -> bool {
        let layer = fv::load(cx);
        let mut names: Vec<String> = cx
            .project
            .layers
            .layers
            .iter()
            .filter(|l| l.name != DATA_LAYER)
            .map(|l| l.name.clone())
            .collect();
        if let Some(own) = layer.layer_of(r) {
            if !names.contains(&own) {
                names.push(own);
            }
        }
        let dialog = FoundationDialog::new(&layer, r, names);
        let opened = dialog.is_some();
        *self.dialog.borrow_mut() = dialog;
        opened
    }

    fn snapped(&self, cx: &EditorContext, p: &PointerEvent) -> Point {
        cx.snap_at(p.world, self.points.last().copied(), p.modifiers.alt, &[])
            .point
    }

    fn update_readout(&self, cx: &mut EditorContext) {
        cx.readout = match (self.rect, self.points.last(), self.hover) {
            (Some((a, b)), _, _) => Some(format!(
                "{} x {}",
                cx.fmt_dim((b.x - a.x).abs()),
                cx.fmt_dim((b.y - a.y).abs())
            )),
            (None, Some(a), Some(h)) => Some(format!("Length: {}", cx.fmt_dim(a.dist(h)))),
            _ => None,
        };
    }

    /// Stores the finished outline as the object of the current flavor.
    fn create(&mut self, cx: &mut EditorContext, outline: Vec<Point>) -> ToolResult {
        let v = self.variant;
        if outline.len() < 3 || outline_area(&outline) < fv::MIN_AREA {
            cx.status = format!("{} needs at least 3 corners", v.name());
            self.reset(cx);
            return ToolResult::consumed();
        }
        let target = match v {
            FoundationVariant::Slab => FoundationRef::Slab(fv::add_slab(cx, outline, false)),
            FoundationVariant::SlabFooting => FoundationRef::Slab(fv::add_slab(cx, outline, true)),
            FoundationVariant::SlabHole => {
                FoundationRef::SlabHole(fv::add_slab_hole(cx, outline, false))
            }
            FoundationVariant::SlabHoleFooting => {
                FoundationRef::SlabHole(fv::add_slab_hole(cx, outline, true))
            }
            FoundationVariant::FloorHole => {
                FoundationRef::PlatformHole(fv::add_platform_hole(cx, outline, PlatformKind::Floor))
            }
            FoundationVariant::CeilingHole => FoundationRef::PlatformHole(fv::add_platform_hole(
                cx,
                outline,
                PlatformKind::Ceiling,
            )),
            FoundationVariant::SquarePad | FoundationVariant::RoundPier => {
                return ToolResult::consumed();
            }
        };
        fv::select(target);
        self.reset(cx);
        cx.status.clear();
        ToolResult::committed(v.name())
    }

    /// Closes the polygon being drawn.
    fn finish(&mut self, cx: &mut EditorContext) -> ToolResult {
        if self.points.len() < 3 {
            cx.status = format!("{} needs at least 3 corners", self.variant.name());
            return ToolResult::consumed();
        }
        let pts = std::mem::take(&mut self.points);
        self.create(cx, pts)
    }

    fn place_spot(&mut self, cx: &mut EditorContext, at: Point) -> ToolResult {
        let target = match self.variant {
            FoundationVariant::SquarePad => FoundationRef::Pad(fv::add_pad(cx, at)),
            _ => FoundationRef::Pier(fv::add_pier(cx, at)),
        };
        fv::select(target);
        ToolResult::committed(self.variant.name())
    }

    /// The object to delete: the selected one, else the one under the pointer.
    fn delete_target(&self, cx: &EditorContext) -> Option<FoundationRef> {
        fv::selected()
            .filter(|r| fv::exists(cx, *r))
            .or_else(|| self.hover.and_then(|h| fv::pick(cx, h, cx.pick_tol())))
    }

    fn delete_current(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(r) = self.delete_target(cx) else {
            return ToolResult::ignored();
        };
        let label = format!("Delete {}", r.name());
        fv::delete(cx, r);
        ToolResult {
            commit: Some(label),
            ..ToolResult::consumed()
        }
    }

    fn is_move_click(p: &PointerEvent) -> bool {
        p.modifiers.command || p.modifiers.ctrl || p.modifiers.mac_cmd
    }

    fn draw_progress(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let ink = cx.palette.ghost_stroke;
        let stroke = Stroke::new(1.6_f32, ink);
        if let Some((a, b)) = self.rect {
            fv::draw_outline(painter, cam, &rect_outline(a, b), true, stroke, true);
            return;
        }
        match self.variant.draw() {
            Draw::Polygon => {
                let mut shape = self.points.clone();
                shape.extend(self.hover.filter(|_| !self.points.is_empty()));
                fv::draw_outline(painter, cam, &shape, false, stroke, true);
                if self.points.len() >= 3 {
                    if let Some(h) = self.hover {
                        fv::draw_outline(
                            painter,
                            cam,
                            &[h, self.points[0]],
                            false,
                            Stroke::new(1.0_f32, ink.gamma_multiply(0.5)),
                            true,
                        );
                    }
                }
                for p in &self.points {
                    painter.circle_filled(cam.world_to_screen(*p), 3.0, ink);
                }
            }
            Draw::Spot => {
                let Some(h) = self.hover else {
                    return;
                };
                let ghost = ink.gamma_multiply(0.6);
                if self.variant == FoundationVariant::SquarePad {
                    let o = Pad::new(0, h).outline();
                    fv::draw_outline(painter, cam, &o, true, Stroke::new(1.4_f32, ghost), false);
                } else {
                    let r = Pier::new(0, h).diameter * 0.5 * cam.px_per_in;
                    painter.circle_stroke(
                        cam.world_to_screen(h),
                        r as f32,
                        Stroke::new(1.4_f32, ghost),
                    );
                }
            }
        }
    }
}

impl Tool for FoundationTool {
    fn id(&self) -> ToolId {
        ToolId::FoundationVariant(self.variant)
    }

    fn name(&self) -> &'static str {
        self.variant.name()
    }

    fn hint(&self) -> String {
        let v = self.variant;
        match v.draw() {
            Draw::Spot => format!("{}: click to place it", v.name()),
            Draw::Polygon => format!(
                "{}: click the corners, double-click closes; drag for a rectangle",
                v.name()
            ),
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::FoundationVariant(v) = id {
            if v != self.variant {
                self.points.clear();
                self.press = None;
                self.rect = None;
            }
            self.variant = v;
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.reset(cx);
        self.hover = None;
        cx.status.clear();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.reset(cx);
        self.moving = None;
        *self.dialog.borrow_mut() = None;
        *self.applied.borrow_mut() = None;
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        let _ = self.flush(cx);
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if let Some(m) = &mut self.moving {
            if p.down {
                let to = cx.snap_at(p.world, None, p.modifiers.alt, &[]).point;
                let d = to - m.start;
                if d.length() > 1e-6 {
                    let mut layer = m.base.clone();
                    layer.translate(m.target, d);
                    let fl = cx.floor;
                    fv::save(&mut cx.project, fl, &layer);
                    cx.mark_dirty();
                    m.moved = true;
                    cx.readout = Some(format!("Move: {}", cx.fmt_dim(d.length())));
                }
            }
            return ToolResult::consumed();
        }
        self.hover = Some(if self.points.is_empty() {
            p.snapped
        } else {
            self.snapped(cx, &p)
        });
        if let (Some(a), true) = (self.press, p.down) {
            let slop = DRAG_PX / cx.px_per_in.max(1e-6);
            if self.rect.is_some() || p.world.dist(a) > slop {
                let corner = cx.snap_at(p.world, None, p.modifiers.alt, &[]).point;
                self.rect = Some((a, corner));
            }
        }
        self.update_readout(cx);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        if self.points.is_empty() && Self::is_move_click(&p) {
            return match fv::pick(cx, p.world, cx.pick_tol()) {
                Some(target) => {
                    fv::select(target);
                    cx.begin_change(&format!("Move {}", target.name()));
                    self.moving = Some(MoveState {
                        target,
                        start: cx.snap_at(p.world, None, false, &[]).point,
                        base: fv::load(cx),
                        moved: false,
                    });
                    ToolResult::consumed()
                }
                None => {
                    fv::clear_selection();
                    ToolResult::consumed()
                }
            };
        }
        match self.variant.draw() {
            Draw::Spot => self.place_spot(cx, p.snapped),
            Draw::Polygon => {
                let pt = if self.points.is_empty() {
                    p.snapped
                } else {
                    self.snapped(cx, &p)
                };
                let closes =
                    self.points.len() >= 3 && p.world.dist(self.points[0]) <= cx.pick_tol() * 1.5;
                if closes {
                    return self.finish(cx);
                }
                if self.points.is_empty() {
                    self.press = Some(pt);
                }
                if self.points.last().is_none_or(|l| l.dist(pt) > 0.5) {
                    self.points.push(pt);
                }
                self.update_readout(cx);
                ToolResult::consumed()
            }
        }
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(m) = self.moving.take() {
            cx.readout = None;
            if !m.moved {
                cx.cancel_change();
                return ToolResult::consumed();
            }
            return ToolResult::committed(&format!("Move {}", m.target.name()));
        }
        self.press = None;
        if let Some((a, _)) = self.rect.take() {
            let corner = cx.snap_at(p.world, None, p.modifiers.alt, &[]).point;
            self.points.clear();
            return self.create(cx, rect_outline(a, corner));
        }
        ToolResult::ignored()
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        if self.variant.draw() == Draw::Polygon && self.points.len() >= 2 {
            return self.finish(cx);
        }
        // The click that started the double-click left at most one stray
        // corner (or a placed pad or pier): it opens the dialog instead.
        self.reset(cx);
        match fv::pick(cx, p.world, cx.pick_tol()) {
            Some(r) => {
                fv::select(r);
                self.open_spec(cx, r);
                ToolResult::consumed()
            }
            None => ToolResult::consumed(),
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        if k.is(Key::Enter) {
            return if self.points.is_empty() {
                ToolResult::ignored()
            } else {
                self.finish(cx)
            };
        }
        if k.is(Key::Escape) {
            if self.points.is_empty() {
                return ToolResult::ignored();
            }
            self.reset(cx);
            return ToolResult::consumed();
        }
        if k.is(Key::Backspace) || k.is(Key::Delete) {
            if self.points.pop().is_some() {
                self.update_readout(cx);
                return ToolResult::consumed();
            }
            return self.delete_current(cx);
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        self.draw_progress(cx, painter, cam);
        // The specification dialog.
        let mut slot = self.dialog.borrow_mut();
        let outcome = slot.as_mut().map(|d| d.show(painter.ctx()));
        match outcome {
            Some(Outcome::Ok) => {
                if let Some(d) = slot.take() {
                    *self.applied.borrow_mut() = Some(d.draft().clone());
                }
            }
            Some(Outcome::Cancel) => {
                slot.take();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::toolbar::{self, Action};
    use eframe::egui::Modifiers;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn tool(v: FoundationVariant) -> FoundationTool {
        let mut t = FoundationTool::default();
        t.set_variant(ToolId::FoundationVariant(v));
        t
    }

    fn click(t: &mut FoundationTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        let r = t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
        r
    }

    fn layer(cx: &EditorContext) -> FoundationLayer {
        fv::load(cx)
    }

    fn draw_triangle(t: &mut FoundationTool, cx: &mut EditorContext) -> ToolResult {
        click(t, cx, 0.0, 0.0);
        click(t, cx, 240.0, 0.0);
        click(t, cx, 240.0, 120.0);
        let p = PointerEvent::at(cx, Point::new(240.0, 120.0));
        t.double_click(cx, p)
    }

    #[test]
    fn click_click_double_click_makes_a_slab_with_three_corners() {
        let mut cx = cx();
        let mut t = tool(FoundationVariant::Slab);
        let r = draw_triangle(&mut t, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("Slab"));
        let l = layer(&cx);
        assert_eq!(l.slabs.len(), 1);
        assert!(l.slabs[0].outline.len() >= 3);
        assert_eq!(l.slabs[0].outline.len(), 3);
        assert!(l.slabs[0].footing.is_none());
        assert_eq!(l.slabs[0].thickness, 4.0);
        assert!(t.points().is_empty());
        assert_eq!(fv::selected(), Some(FoundationRef::Slab(l.slabs[0].id)));
        // The real shell sends a second press for the double-click too.
        assert_eq!(cx.undo().as_deref(), Some("Slab"));
        assert!(layer(&cx).is_empty());
        assert_eq!(cx.redo().as_deref(), Some("Slab"));
        assert_eq!(layer(&cx).slabs.len(), 1);
    }

    #[test]
    fn enter_and_a_click_on_the_first_corner_close_the_shape() {
        let mut cx = cx();
        let mut t = tool(FoundationVariant::SlabFooting);
        for (x, y) in [(0.0, 0.0), (240.0, 0.0), (240.0, 180.0), (0.0, 180.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Slab with Footing"));
        let s = layer(&cx).slabs[0].clone();
        assert_eq!(s.outline.len(), 4);
        let f = s.footing.expect("Slab with Footing");
        assert_eq!((f.width, f.depth), (16.0, 8.0));

        let mut t = tool(FoundationVariant::SlabHole);
        for (x, y) in [(50.0, 50.0), (90.0, 50.0), (90.0, 90.0)] {
            click(&mut t, &mut cx, x, y);
        }
        let r = click(&mut t, &mut cx, 51.0, 51.0);
        assert!(r.commit.is_some());
        let l = layer(&cx);
        assert_eq!(l.holes.len(), 1);
        assert!(!l.holes[0].with_footing);
        // The hole sits inside the slab, so the slab's net area shrinks.
        let slab = &l.slabs[0];
        assert_eq!(l.holes_in(slab).len(), 1);
    }

    #[test]
    fn too_few_corners_do_not_close_and_backspace_pops() {
        let mut cx = cx();
        let mut t = tool(FoundationVariant::Slab);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 100.0, 0.0);
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(r.commit.is_none() && r.consumed);
        assert!(cx.status.contains("at least 3"));
        assert_eq!(t.points().len(), 2);
        t.key(&mut cx, KeyEvent::key(Key::Backspace));
        assert_eq!(t.points().len(), 1);
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(t.points().is_empty());
        assert!(
            !t.key(&mut cx, KeyEvent::escape()).consumed,
            "Esc leaves the tool"
        );
        assert!(layer(&cx).is_empty());
    }

    #[test]
    fn dragging_draws_a_rectangle() {
        let mut cx = cx();
        let mut t = tool(FoundationVariant::Slab);
        let a = PointerEvent::at(&cx, Point::new(0.0, 0.0));
        t.pointer_move(&mut cx, a);
        t.pointer_down(&mut cx, a.with_down(true));
        let b = PointerEvent::at(&cx, Point::new(300.0, 200.0)).with_down(true);
        t.pointer_move(&mut cx, b);
        assert!(cx.readout.as_deref().unwrap().contains(" x "));
        let r = t.pointer_up(&mut cx, b);
        assert_eq!(r.commit.as_deref(), Some("Slab"));
        let s = &layer(&cx).slabs[0];
        assert_eq!(s.outline.len(), 4);
        assert!((s.gross_area() - 300.0 * 200.0).abs() < 1e-6);
        assert!(t.points().is_empty());
    }

    #[test]
    fn pads_and_piers_place_with_one_click() {
        let mut cx = cx();
        let mut t = tool(FoundationVariant::SquarePad);
        let r = click(&mut t, &mut cx, 100.0, 100.0);
        assert_eq!(r.commit.as_deref(), Some("Square Pad"));
        let pad = layer(&cx).pads[0].clone();
        assert_eq!((pad.size, pad.thickness), (24.0, 12.0));
        assert_eq!(pad.center, Point::new(100.0, 100.0));
        let mut t = tool(FoundationVariant::RoundPier);
        click(&mut t, &mut cx, 200.0, 100.0);
        click(&mut t, &mut cx, 300.0, 100.0);
        let l = layer(&cx);
        assert_eq!(l.piers.len(), 2);
        assert_eq!((l.piers[0].diameter, l.piers[0].height), (12.0, 36.0));
        assert!(l.piers[0].footing.is_none());
        assert_eq!(cx.undo().as_deref(), Some("Round Pier"));
        assert_eq!(layer(&cx).piers.len(), 1);
    }

    #[test]
    fn platform_hole_tools_store_the_right_kind() {
        let mut cx = cx();
        for (v, kind) in [
            (FoundationVariant::FloorHole, PlatformKind::Floor),
            (FoundationVariant::CeilingHole, PlatformKind::Ceiling),
        ] {
            let mut t = tool(v);
            draw_triangle(&mut t, &mut cx);
            let l = layer(&cx);
            assert_eq!(l.platform_holes.last().unwrap().kind, kind);
        }
        let l = layer(&cx);
        assert_eq!(l.platform_hole_outlines(PlatformKind::Floor).len(), 1);
        assert_eq!(l.platform_hole_outlines(PlatformKind::Ceiling).len(), 1);
        assert_eq!(
            l.layer_of(FoundationRef::PlatformHole(l.platform_holes[1].id))
                .as_deref(),
            Some("Ceilings, Holes")
        );
    }

    #[test]
    fn ctrl_drag_moves_and_delete_removes_with_undo() {
        let mut cx = cx();
        let mut t = tool(FoundationVariant::SquarePad);
        click(&mut t, &mut cx, 100.0, 100.0);
        let id = layer(&cx).pads[0].id;
        let ctrl = Modifiers::COMMAND;
        let down = PointerEvent::at(&cx, Point::new(100.0, 100.0))
            .with_down(true)
            .with_modifiers(ctrl);
        t.pointer_down(&mut cx, down);
        for x in [120.0, 160.0] {
            let m = PointerEvent::at(&cx, Point::new(x, 140.0))
                .with_down(true)
                .with_modifiers(ctrl);
            t.pointer_move(&mut cx, m);
        }
        let up = PointerEvent::at(&cx, Point::new(160.0, 140.0)).with_modifiers(ctrl);
        let r = t.pointer_up(&mut cx, up);
        assert_eq!(r.commit.as_deref(), Some("Move Square Pad"));
        assert_eq!(layer(&cx).pad(id).unwrap().center, Point::new(160.0, 140.0));
        assert_eq!(cx.undo().as_deref(), Some("Move Square Pad"));
        assert_eq!(layer(&cx).pad(id).unwrap().center, Point::new(100.0, 100.0));
        // A Ctrl-click that does not move leaves no undo step.
        let before = cx.undo_label().map(str::to_string);
        let down = PointerEvent::at(&cx, Point::new(100.0, 100.0)).with_modifiers(ctrl);
        t.pointer_down(&mut cx, down);
        t.pointer_up(&mut cx, down);
        assert_eq!(cx.undo_label().map(str::to_string), before);
        // Delete removes the selected object.
        assert_eq!(fv::selected(), Some(FoundationRef::Pad(id)));
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete Square Pad"));
        assert!(layer(&cx).is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Delete Square Pad"));
        assert_eq!(layer(&cx).pads.len(), 1);
    }

    #[test]
    fn a_double_click_on_an_object_opens_its_dialog_and_ok_applies_with_undo() {
        let mut cx = cx();
        let mut t = tool(FoundationVariant::Slab);
        draw_triangle(&mut t, &mut cx);
        let id = layer(&cx).slabs[0].id;
        // The press of the double-click starts a stray corner; the
        // double-click then opens the dialog instead of a shape.
        click(&mut t, &mut cx, 200.0, 20.0);
        let p = PointerEvent::at(&cx, Point::new(200.0, 20.0));
        t.double_click(&mut cx, p);
        assert!(t.dialog_open());
        assert!(t.points().is_empty());
        // Simulate OK with a changed draft.
        let mut d = t.dialog.borrow_mut().take().unwrap();
        if let Draft::Slab(s) = d.draft_mut() {
            s.thickness = 6.0;
            s.top_elevation = 2.0;
        }
        *t.applied.borrow_mut() = Some(d.draft().clone());
        let r = t.frame_for_test(&mut cx);
        assert_eq!(r.commit.as_deref(), Some("Slab Specification"));
        let s = layer(&cx).slab(id).unwrap().clone();
        assert_eq!((s.thickness, s.top_elevation), (6.0, 2.0));
        assert_eq!(cx.undo().as_deref(), Some("Slab Specification"));
        assert_eq!(layer(&cx).slab(id).unwrap().thickness, 4.0);
        // A double-click on empty ground opens nothing.
        let p = PointerEvent::at(&cx, Point::new(900.0, 900.0));
        t.double_click(&mut cx, p);
        assert!(!t.dialog_open());
    }

    impl FoundationTool {
        fn frame_for_test(&mut self, cx: &mut EditorContext) -> ToolResult {
            self.flush(cx).expect("an applied draft")
        }
    }

    #[test]
    fn the_flyout_entries_are_live_except_floor_material_region() {
        let mut names = Vec::new();
        for f in [toolbar::slab(), toolbar::floor()] {
            for e in &f.entries {
                if let Action::NotImplemented(name) = e.action {
                    names.push(name);
                }
            }
        }
        assert!(names.contains(&"Floor Material Region"));
        for v in FoundationVariant::ALL {
            assert!(!names.contains(&v.name()), "{} is still a stub", v.name());
        }
        // Each entry selects the matching variant.
        for f in [toolbar::slab(), toolbar::floor()] {
            for e in &f.entries {
                if let Some(v) = FoundationVariant::ALL.iter().find(|v| v.name() == e.name) {
                    assert_eq!(e.action, Action::SetTool(ToolId::FoundationVariant(*v)));
                }
            }
        }
        assert!(toolbar::slab().entries.len() == 6);
    }

    #[test]
    fn the_tool_set_selects_each_variant_by_id() {
        let mut cx = cx();
        let mut set = crate::tools::ToolSet::new();
        for v in FoundationVariant::ALL {
            let id = ToolId::FoundationVariant(v);
            set.set_active(&mut cx, id);
            assert_eq!(set.active().name(), v.name());
            assert_eq!(set.active_id(), id);
            assert!(set.active().hint().starts_with(v.name()));
        }
    }
}
