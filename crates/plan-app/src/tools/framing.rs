//! Framing tools: the General, Floor/Ceiling and Roof Framing flyouts.
//!
//! One tool object with a flavor per flyout entry ([`FramingVariant`]):
//!
//! * lumber members (General Framing, Blocking, Joist, Joist Blocking,
//!   Floor/Ceiling Beam and Truss, Rafter, Roof Beam, Roof Blocking, Roof
//!   Purlin, Roof Truss, Girder Truss) are drawn start to end: press at the
//!   start and drag to the end, or click the start and then the end (Esc
//!   cancels). Both ends snap like walls;
//! * Post and Post with Footing, and the Framing Reference Marker, are placed
//!   with one click;
//! * Joist Direction, Roof Truss Direction and Bearing Line are lines drawn
//!   like members; Truss Base is a closed polyline (click the corners, Enter,
//!   a double-click or a click on the first corner closes it).
//!
//! Hold Shift or Cmd and click to pick a placed framing object (Shift on a
//! picked one drops it from the selection); drag to move the selection.
//! Delete or Backspace removes the picked objects, or the one under the
//! pointer. A double-click on a member opens the Framing Member
//! Specification. Every change is one undo step.
//!
//! The objects live in `Floor.framing` through `editor::framing_view`; Build
//! Framing honors the direction lines, bearing lines, markers and truss bases.
//! Selection is kept in `framing_view` (`ObjectRef` has no framing variant).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::framing::FramingMemberDialog;
use crate::dialogs::Outcome;
use crate::editor::framing_view::{self, Record, MIN_MEMBER};
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Color32, Key, Stroke};
use plan_core::geometry::Point;
use plan_framing::{
    BearingLine, FramingMember, JoistDirectionLine, ManualMemberKind, ReferenceMarker,
    RoofTrussDirection, TrussBase,
};
use std::cell::RefCell;

/// A click this close to the first corner closes a Truss Base, inches.
const CLOSE_TOL: f64 = 8.0;

/// The flavors of the framing tool, one per flyout entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FramingVariant {
    General,
    Post,
    PostWithFooting,
    Blocking,
    ReferenceMarker,
    Joist,
    JoistBlocking,
    JoistDirection,
    FloorCeilingBeam,
    FloorCeilingTruss,
    BearingLine,
    Rafter,
    RoofBeam,
    RoofBlocking,
    RoofPurlin,
    RoofTruss,
    GirderTruss,
    RoofTrussDirection,
    TrussBase,
}

/// What the clicks of a flavor draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Draw {
    Line,
    Spot,
    Polyline,
}

impl FramingVariant {
    pub const ALL: [FramingVariant; 19] = [
        FramingVariant::General,
        FramingVariant::Post,
        FramingVariant::PostWithFooting,
        FramingVariant::Blocking,
        FramingVariant::ReferenceMarker,
        FramingVariant::Joist,
        FramingVariant::JoistBlocking,
        FramingVariant::JoistDirection,
        FramingVariant::FloorCeilingBeam,
        FramingVariant::FloorCeilingTruss,
        FramingVariant::BearingLine,
        FramingVariant::Rafter,
        FramingVariant::RoofBeam,
        FramingVariant::RoofBlocking,
        FramingVariant::RoofPurlin,
        FramingVariant::RoofTruss,
        FramingVariant::GirderTruss,
        FramingVariant::RoofTrussDirection,
        FramingVariant::TrussBase,
    ];

    /// Chief's name (the flyout entry).
    pub fn name(self) -> &'static str {
        use FramingVariant as V;
        match self {
            V::General => "General Framing",
            V::Post => "Post",
            V::PostWithFooting => "Post with Footing",
            V::Blocking => "Blocking",
            V::ReferenceMarker => "Framing Reference Marker",
            V::Joist => "Joist",
            V::JoistBlocking => "Joist Blocking",
            V::JoistDirection => "Joist Direction",
            V::FloorCeilingBeam => "Floor/Ceiling Beam",
            V::FloorCeilingTruss => "Floor/Ceiling Truss",
            V::BearingLine => "Bearing Line",
            V::Rafter => "Rafter",
            V::RoofBeam => "Roof Beam",
            V::RoofBlocking => "Roof Blocking",
            V::RoofPurlin => "Roof Purlin",
            V::RoofTruss => "Roof Truss",
            V::GirderTruss => "Girder Truss",
            V::RoofTrussDirection => "Roof Truss Direction",
            V::TrussBase => "Truss Base",
        }
    }

    fn draw(self) -> Draw {
        use FramingVariant as V;
        match self {
            V::Post | V::PostWithFooting | V::ReferenceMarker => Draw::Spot,
            V::TrussBase => Draw::Polyline,
            _ => Draw::Line,
        }
    }

    /// The member type a lumber flavor places.
    pub fn member_kind(self) -> Option<ManualMemberKind> {
        use FramingVariant as V;
        use ManualMemberKind as K;
        Some(match self {
            V::General => K::GeneralFraming,
            V::Post => K::Post,
            V::PostWithFooting => K::PostWithFooting,
            V::Blocking => K::Blocking,
            V::Joist => K::Joist,
            V::JoistBlocking => K::JoistBlocking,
            V::FloorCeilingBeam => K::FloorCeilingBeam,
            V::FloorCeilingTruss => K::FloorCeilingTruss,
            V::Rafter => K::Rafter,
            V::RoofBeam => K::RoofBeam,
            V::RoofBlocking => K::RoofBlocking,
            V::RoofPurlin => K::RoofPurlin,
            V::RoofTruss => K::RoofTruss,
            V::GirderTruss => K::GirderTruss,
            V::ReferenceMarker
            | V::JoistDirection
            | V::BearingLine
            | V::RoofTrussDirection
            | V::TrussBase => return None,
        })
    }
}

pub struct FramingTool {
    variant: FramingVariant,
    /// The start of a line being drawn (kept after a plain click until the
    /// second click).
    start: Option<Point>,
    /// The primary button is down on the start of a line.
    dragging: bool,
    hover: Option<Point>,
    /// The corners of a Truss Base being drawn.
    poly: Vec<Point>,
    /// The previous pointer position of a move drag.
    moving: Option<Point>,
    dialog: RefCell<Option<FramingMemberDialog>>,
    applied: RefCell<Option<plan_framing::FramingMember>>,
}

impl Default for FramingTool {
    fn default() -> Self {
        Self {
            variant: FramingVariant::Joist,
            start: None,
            dragging: false,
            hover: None,
            poly: Vec::new(),
            moving: None,
            dialog: RefCell::new(None),
            applied: RefCell::new(None),
        }
    }
}

impl FramingTool {
    fn reset(&mut self, cx: &mut EditorContext) {
        self.start = None;
        self.dragging = false;
        self.poly.clear();
        self.moving = None;
        cx.readout = None;
    }

    fn dialog_open(&self) -> bool {
        self.dialog.borrow().is_some()
    }

    /// Applies an OK in the Framing Member Specification.
    fn flush(&mut self, cx: &mut EditorContext) -> Option<ToolResult> {
        let draft = self.applied.borrow_mut().take()?;
        if framing_view::apply_edit(cx, draft) {
            cx.status = "Framing member updated".into();
            return Some(ToolResult::committed("Framing Member Specification"));
        }
        Some(ToolResult::consumed())
    }

    fn end_point(&self, cx: &EditorContext, p: &PointerEvent) -> Point {
        match self.start {
            Some(a) => cx.snap_at(p.world, Some(a), p.modifiers.alt, &[]).point,
            None => p.snapped,
        }
    }

    fn update_readout(&self, cx: &mut EditorContext) {
        cx.readout = match (self.start, self.hover) {
            (Some(a), Some(h)) => Some(format!("Length: {}", cx.fmt_dim(a.dist(h)))),
            _ => None,
        };
    }

    /// Stores the finished line from `a` to `b`.
    fn finish_line(&mut self, cx: &mut EditorContext, a: Point, b: Point) -> ToolResult {
        self.start = None;
        self.dragging = false;
        cx.readout = None;
        let v = self.variant;
        let label = format!("Place {}", v.name());
        let id = if let Some(kind) = v.member_kind() {
            let m = framing_view::new_member(cx.floor(), kind, a, b);
            framing_view::add_record(cx, &label, |id| Record::Manual(FramingMember { id, ..m }))
        } else {
            match v {
                FramingVariant::JoistDirection => {
                    framing_view::add_record(cx, &label, |id| Record::JoistDirection {
                        id,
                        dir: JoistDirectionLine::new((a, b), framing_view::default_spacing(false)),
                    })
                }
                FramingVariant::RoofTrussDirection => {
                    framing_view::add_record(cx, &label, |id| Record::TrussDirection {
                        id,
                        dir: RoofTrussDirection::new((a, b), framing_view::default_spacing(true)),
                    })
                }
                _ => framing_view::add_record(cx, &label, |id| Record::BearingLine {
                    id,
                    line: BearingLine { line: (a, b) },
                }),
            }
        };
        framing_view::select(vec![id]);
        cx.status = format!("{} placed ({})", v.name(), cx.fmt_dim(a.dist(b)));
        ToolResult::committed(&label)
    }

    /// Stores the Truss Base being drawn.
    fn finish_base(&mut self, cx: &mut EditorContext) -> ToolResult {
        let pts = std::mem::take(&mut self.poly);
        cx.readout = None;
        if pts.len() < 3 {
            cx.status = "A truss base needs at least three corners".into();
            return ToolResult::consumed();
        }
        let elevation = framing_view::top_plate(cx.floor());
        let label = "Place Truss Base";
        let id = framing_view::add_record(cx, label, |id| Record::TrussBase {
            id,
            base: TrussBase::new(pts, elevation),
        });
        framing_view::select(vec![id]);
        cx.status = "Truss Base placed".into();
        ToolResult::committed(label)
    }

    fn place_spot(&mut self, cx: &mut EditorContext, at: Point) -> ToolResult {
        let v = self.variant;
        let label = format!("Place {}", v.name());
        let id = if let Some(kind) = v.member_kind() {
            let m = framing_view::new_member(cx.floor(), kind, at, at);
            framing_view::add_record(cx, &label, |id| Record::Manual(FramingMember { id, ..m }))
        } else {
            framing_view::add_record(cx, &label, |id| Record::Marker {
                id,
                marker: ReferenceMarker {
                    point: at,
                    angle: 0.0,
                },
            })
        };
        framing_view::select(vec![id]);
        cx.status = format!("{} placed", v.name());
        ToolResult::committed(&label)
    }

    fn delete_picked(&mut self, cx: &mut EditorContext) -> ToolResult {
        let mut ids = framing_view::selected();
        if ids.is_empty() {
            if let Some(h) = self.hover {
                ids.extend(framing_view::pick(cx.floor(), h, cx.pick_tol()));
            }
        }
        let n = framing_view::delete_records(cx, &ids);
        if n == 0 {
            return ToolResult::ignored();
        }
        cx.status = format!(
            "Deleted {n} framing object{}",
            if n == 1 { "" } else { "s" }
        );
        ToolResult::committed("Delete Framing")
    }

    /// Opens the specification of the member under `at`.
    fn open_spec(&mut self, cx: &mut EditorContext, at: Point) -> ToolResult {
        let Some(id) = framing_view::pick(cx.floor(), at, cx.pick_tol()) else {
            return ToolResult::consumed();
        };
        framing_view::select(vec![id]);
        match framing_view::find(cx.floor(), id) {
            Some(Record::Manual(m) | Record::Built(m)) => {
                let layers = cx.project.layers.layers.iter().map(|l| l.name.clone());
                *self.dialog.borrow_mut() = Some(FramingMemberDialog::new(&m, layers.collect()));
            }
            Some(r) => {
                cx.status = format!("{} (drag with Shift to move, Delete removes it)", r.name())
            }
            None => {}
        }
        ToolResult::consumed()
    }
}

impl Tool for FramingTool {
    fn id(&self) -> ToolId {
        ToolId::FramingVariant(self.variant)
    }

    fn name(&self) -> &'static str {
        self.variant.name()
    }

    fn hint(&self) -> String {
        let v = self.variant;
        match v.draw() {
            Draw::Spot => format!("{}: click to place it", v.name()),
            Draw::Polyline => "Truss Base: click the corners, Enter closes the shape".to_string(),
            Draw::Line => format!(
                "{}: drag from the start to the end, or click twice",
                v.name()
            ),
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::FramingVariant(v) = id {
            if v != self.variant {
                self.start = None;
                self.dragging = false;
                self.poly.clear();
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
        *self.dialog.borrow_mut() = None;
        *self.applied.borrow_mut() = None;
        framing_view::select(Vec::new());
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        let _ = self.flush(cx);
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        // Shift or Cmd picks; a drag moves the pick.
        if p.modifiers.shift || p.modifiers.command {
            let hit = framing_view::pick(cx.floor(), p.world, cx.pick_tol());
            let mut sel = framing_view::selected();
            match hit {
                Some(id) if p.modifiers.shift && sel.contains(&id) => sel.retain(|i| *i != id),
                Some(id) if p.modifiers.shift => sel.push(id),
                Some(id) => sel = vec![id],
                None => sel.clear(),
            }
            self.moving = hit.map(|_| p.snapped);
            framing_view::select(sel);
            return ToolResult::consumed();
        }
        match self.variant.draw() {
            Draw::Spot => self.place_spot(cx, p.snapped),
            Draw::Polyline => {
                let pt = self.end_point_poly(cx, &p);
                if self.poly.len() >= 3 && self.poly[0].dist(pt) <= CLOSE_TOL {
                    return self.finish_base(cx);
                }
                if self.poly.last().is_none_or(|l| l.dist(pt) > 0.5) {
                    self.poly.push(pt);
                }
                ToolResult::consumed()
            }
            Draw::Line => {
                if let Some(a) = self.start {
                    // Second click of a click-click line.
                    let b = self.end_point(cx, &p);
                    if a.dist(b) >= MIN_MEMBER {
                        return self.finish_line(cx, a, b);
                    }
                    return ToolResult::consumed();
                }
                self.start = Some(p.snapped);
                self.dragging = true;
                self.hover = Some(p.snapped);
                ToolResult::consumed()
            }
        }
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if let Some(last) = self.moving {
            let ids = framing_view::selected();
            let to = p.snapped;
            if framing_view::move_records(cx, &ids, to - last) > 0 {
                self.moving = Some(to);
            }
            return ToolResult::consumed();
        }
        self.hover = Some(match self.variant.draw() {
            Draw::Line if self.start.is_some() => self.end_point(cx, &p),
            Draw::Polyline if !self.poly.is_empty() => self.end_point_poly(cx, &p),
            _ => p.snapped,
        });
        self.update_readout(cx);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.moving.take().is_some() {
            cx.end_merge();
            cx.status = "Moved framing".into();
            return ToolResult::committed("Move Framing");
        }
        if !self.dragging {
            return ToolResult::ignored();
        }
        self.dragging = false;
        let Some(a) = self.start else {
            return ToolResult::ignored();
        };
        let b = self.end_point(cx, &p);
        if a.dist(b) >= MIN_MEMBER {
            return self.finish_line(cx, a, b);
        }
        // A plain click: the start waits for the second click.
        self.hover = Some(a);
        ToolResult::consumed()
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        if self.variant.draw() == Draw::Polyline && self.poly.len() >= 3 {
            return self.finish_base(cx);
        }
        // The first click of the double-click may have started a line.
        self.start = None;
        self.dragging = false;
        self.open_spec(cx, p.world)
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        if k.is(Key::Enter) && !self.poly.is_empty() {
            return self.finish_base(cx);
        }
        if k.is(Key::Escape) {
            if self.start.is_some() || !self.poly.is_empty() {
                self.reset(cx);
                return ToolResult::consumed();
            }
            if !framing_view::selected().is_empty() {
                framing_view::select(Vec::new());
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(Key::Backspace) || k.is(Key::Delete) {
            if self.poly.pop().is_some() {
                return ToolResult::consumed();
            }
            return self.delete_picked(cx);
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        let ghost = Stroke::new(1.5_f32, pal.ghost_stroke);
        let v = self.variant;
        match (v.draw(), self.start, self.hover) {
            (Draw::Line, Some(a), Some(b)) if a.dist(b) > 0.5 => {
                if let Some(kind) = v.member_kind() {
                    let m = framing_view::new_member(cx.floor(), kind, a, b);
                    framing_view::paint_member(painter, cam, &m, pal.ghost_stroke, ghost);
                } else {
                    painter.line_segment([cam.world_to_screen(a), cam.world_to_screen(b)], ghost);
                }
            }
            (Draw::Spot, _, Some(h)) => {
                if let Some(kind) = v.member_kind() {
                    let m = framing_view::new_member(cx.floor(), kind, h, h);
                    framing_view::paint_member(painter, cam, &m, pal.ghost_stroke, ghost);
                } else {
                    painter.circle_stroke(cam.world_to_screen(h), 6.0, ghost);
                }
            }
            (Draw::Polyline, _, hover) => {
                let mut pts: Vec<egui::Pos2> =
                    self.poly.iter().map(|p| cam.world_to_screen(*p)).collect();
                pts.extend(
                    hover
                        .filter(|_| !self.poly.is_empty())
                        .map(|h| cam.world_to_screen(h)),
                );
                if pts.len() >= 2 {
                    painter.add(egui::Shape::line(pts, ghost));
                }
            }
            _ => {}
        }
        for p in &self.poly {
            painter.circle_filled(cam.world_to_screen(*p), 3.0, Color32::from_gray(0x60));
        }
        if let Some(a) = self.start {
            painter.circle_filled(cam.world_to_screen(a), 3.0, pal.selection);
        }
        // The Framing Member Specification.
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

impl FramingTool {
    fn end_point_poly(&self, cx: &EditorContext, p: &PointerEvent) -> Point {
        match self.poly.last() {
            Some(a) => cx.snap_at(p.world, Some(*a), p.modifiers.alt, &[]).point,
            None => p.snapped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::toolbar::{self, Action};
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
        }
        cx.refresh();
        cx
    }

    fn tool(v: FramingVariant) -> FramingTool {
        let mut t = FramingTool::default();
        t.set_variant(ToolId::FramingVariant(v));
        t
    }

    fn ev(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(cx, Point::new(x, y))
    }

    /// Press at `a`, drag to `b`, release.
    fn drag(t: &mut FramingTool, cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) {
        let e = ev(cx, a.0, a.1);
        t.pointer_down(cx, e);
        let e = ev(cx, b.0, b.1).with_down(true);
        t.pointer_move(cx, e);
        let e = ev(cx, b.0, b.1);
        t.pointer_up(cx, e);
    }

    fn click(t: &mut FramingTool, cx: &mut EditorContext, x: f64, y: f64) {
        let e = ev(cx, x, y);
        t.pointer_down(cx, e);
        let e = ev(cx, x, y);
        t.pointer_up(cx, e);
    }

    fn records(cx: &EditorContext) -> Vec<Record> {
        framing_view::load_records(cx.floor())
    }

    #[test]
    fn every_framing_flyout_entry_activates_a_tool() {
        let mut cx = cx();
        let mut set = crate::tools::ToolSet::new();
        let mut seen = 0;
        for f in [
            toolbar::general_framing(),
            toolbar::floor_ceiling_framing(),
            toolbar::roof_framing(),
        ] {
            for e in &f.entries {
                assert!(
                    !matches!(e.action, Action::NotImplemented(_)),
                    "{} is still a stub",
                    e.name
                );
                if let Some(v) = FramingVariant::ALL.iter().find(|v| v.name() == e.name) {
                    assert_eq!(e.action, Action::SetTool(ToolId::FramingVariant(*v)));
                    set.set_active(&mut cx, ToolId::FramingVariant(*v));
                    assert_eq!(set.active().name(), e.name);
                    assert_eq!(set.active_id(), ToolId::FramingVariant(*v));
                    assert!(!set.active().hint().is_empty());
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, 19, "all 19 variants sit in the three flyouts");
    }

    #[test]
    fn drawing_a_joist_stores_a_member_and_undo_removes_it() {
        let mut cx = cx();
        let mut t = tool(FramingVariant::Joist);
        drag(&mut t, &mut cx, (20.0, 40.0), (200.0, 40.0));
        let recs = records(&cx);
        assert_eq!(recs.len(), 1);
        let Record::Manual(m) = &recs[0] else {
            panic!("a manual member")
        };
        assert_eq!(m.kind, ManualMemberKind::Joist);
        assert!((m.plan_length() - 180.0).abs() < 1e-6, "{m:?}");
        assert_eq!(m.layer_name, "Framing, Floor Joists");
        assert!(m.id > 0);
        // It sits under the subfloor.
        assert!((m.elevation_bottom + m.depth + 0.75).abs() < 1e-9);
        assert!(cx.layers().is_visible("Framing, Floor Joists"));
        assert_eq!(cx.undo_label(), Some("Place Joist"));
        assert!(framing_view::selected().contains(&m.id));
        cx.undo();
        assert!(records(&cx).is_empty());
        cx.redo();
        assert_eq!(records(&cx).len(), 1);
    }

    #[test]
    fn click_click_and_short_clicks_for_lines_and_esc_cancels() {
        let mut cx = cx();
        let mut t = tool(FramingVariant::Rafter);
        click(&mut t, &mut cx, 20.0, 40.0);
        assert!(t.start.is_some(), "the start waits for the second click");
        assert!(records(&cx).is_empty());
        click(&mut t, &mut cx, 20.0, 160.0);
        let recs = records(&cx);
        assert_eq!(recs.len(), 1);
        let Record::Manual(m) = &recs[0] else {
            panic!()
        };
        assert_eq!(m.kind, ManualMemberKind::Rafter);
        assert!((m.rise - 60.0).abs() < 1e-6, "6:12 over 120\"");
        // Esc drops a pending start.
        click(&mut t, &mut cx, 60.0, 60.0);
        assert!(t.start.is_some());
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(t.start.is_none());
        // The next Esc drops the selection, and an idle Esc leaves the tool.
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(framing_view::selected().is_empty());
        assert!(
            !t.key(&mut cx, KeyEvent::escape()).consumed,
            "idle Esc leaves the tool"
        );
        assert_eq!(records(&cx).len(), 1);
    }

    #[test]
    fn posts_markers_and_layout_lines_are_placed() {
        let mut cx = cx();
        let mut post = tool(FramingVariant::PostWithFooting);
        click(&mut post, &mut cx, 100.0, 100.0);
        let mut marker = tool(FramingVariant::ReferenceMarker);
        click(&mut marker, &mut cx, 50.0, 50.0);
        let mut dir = tool(FramingVariant::JoistDirection);
        drag(&mut dir, &mut cx, (10.0, 20.0), (10.0, 100.0));
        let mut bearing = tool(FramingVariant::BearingLine);
        drag(&mut bearing, &mut cx, (120.0, 5.0), (120.0, 180.0));
        let mut tdir = tool(FramingVariant::RoofTrussDirection);
        drag(&mut tdir, &mut cx, (30.0, 20.0), (130.0, 20.0));
        let recs = records(&cx);
        assert_eq!(recs.len(), 5);
        assert!(
            matches!(&recs[0], Record::Manual(m) if m.kind == ManualMemberKind::PostWithFooting
            && m.start == m.end && m.footing().is_some())
        );
        assert!(matches!(&recs[1], Record::Marker { .. }));
        assert!(matches!(&recs[2], Record::JoistDirection { dir, .. }
            if (dir.spacing - 16.0).abs() < 1e-9 && (dir.angle - 90.0).abs() < 1e-6));
        assert!(matches!(&recs[3], Record::BearingLine { .. }));
        assert!(matches!(&recs[4], Record::TrussDirection { dir, .. }
            if (dir.spacing - 24.0).abs() < 1e-9));
    }

    #[test]
    fn a_truss_base_closes_on_enter_or_the_first_corner() {
        let mut cx = cx();
        let mut t = tool(FramingVariant::TrussBase);
        for (x, y) in [(0.0, 0.0), (240.0, 0.0), (240.0, 192.0)] {
            click(&mut t, &mut cx, x, y);
        }
        assert!(records(&cx).is_empty());
        assert!(t.key(&mut cx, KeyEvent::key(Key::Enter)).consumed);
        let recs = records(&cx);
        let Record::TrussBase { base, .. } = &recs[0] else {
            panic!()
        };
        assert_eq!(base.points.len(), 3);
        assert!((base.elevation - 109.125).abs() < 1e-9);
        for (x, y) in [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (1.0, 1.0)] {
            click(&mut t, &mut cx, x, y);
        }
        assert_eq!(
            records(&cx).len(),
            2,
            "a click on the first corner closes it"
        );
        // Too few corners store nothing.
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 50.0, 0.0);
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(records(&cx).len(), 2);
    }

    #[test]
    fn shift_picks_drags_and_delete_removes_with_undo() {
        let mut cx = cx();
        let mut t = tool(FramingVariant::Joist);
        drag(&mut t, &mut cx, (20.0, 40.0), (200.0, 40.0));
        let id = records(&cx)[0].id();
        framing_view::select(Vec::new());
        // Shift-click on the joist picks it, a drag moves it.
        let shift = egui::Modifiers::SHIFT;
        let e = ev(&cx, 100.0, 40.0).with_modifiers(shift);
        t.pointer_down(&mut cx, e);
        assert_eq!(framing_view::selected(), vec![id]);
        let e = ev(&cx, 100.0, 70.0).with_modifiers(shift).with_down(true);
        t.pointer_move(&mut cx, e);
        let e = ev(&cx, 100.0, 70.0);
        t.pointer_up(&mut cx, e);
        let Record::Manual(m) = &records(&cx)[0] else {
            panic!()
        };
        assert!(
            (m.start.y - 70.0).abs() < 1e-6 && (m.end.y - 70.0).abs() < 1e-6,
            "{m:?}"
        );
        assert_eq!(cx.undo_label(), Some("Move Framing"));
        // Delete removes the picked joist; undo brings it back.
        assert!(t.key(&mut cx, KeyEvent::key(Key::Delete)).consumed);
        assert!(records(&cx).is_empty());
        assert_eq!(cx.undo_label(), Some("Delete Framing"));
        cx.undo();
        assert_eq!(records(&cx).len(), 1);
    }

    #[test]
    fn double_click_opens_the_spec_and_an_ok_applies_it() {
        let mut cx = cx();
        let mut t = tool(FramingVariant::Joist);
        drag(&mut t, &mut cx, (20.0, 40.0), (200.0, 40.0));
        let e = ev(&cx, 100.0, 40.0);
        t.double_click(&mut cx, e);
        assert!(t.dialog_open());
        {
            let mut slot = t.dialog.borrow_mut();
            let d = slot.as_mut().unwrap();
            d.set_lumber(plan_framing::LumberSize::TWO_BY_TWELVE);
            d.set_length(150.0);
        }
        // Stand in for the OK click.
        let draft = t.dialog.borrow_mut().take().unwrap().draft().clone();
        *t.applied.borrow_mut() = Some(draft);
        let ctx = egui::Context::default();
        t.frame(&mut cx, &ctx);
        let Record::Manual(m) = &records(&cx)[0] else {
            panic!()
        };
        assert_eq!(m.lumber, plan_framing::LumberSize::TWO_BY_TWELVE);
        assert!((m.plan_length() - 150.0).abs() < 1e-6);
        assert_eq!(cx.undo_label(), Some("Framing Member Specification"));
    }

    #[test]
    fn the_tool_draws_its_overlay_and_the_plan_draws_manual_framing() {
        let mut cx = cx();
        let mut t = tool(FramingVariant::Joist);
        drag(&mut t, &mut cx, (20.0, 40.0), (200.0, 40.0));
        let mut post = tool(FramingVariant::PostWithFooting);
        click(&mut post, &mut cx, 100.0, 100.0);
        let mut dir = tool(FramingVariant::JoistDirection);
        drag(&mut dir, &mut cx, (10.0, 20.0), (10.0, 100.0));
        let mut base = tool(FramingVariant::TrussBase);
        click(&mut base, &mut cx, 0.0, 0.0);
        click(&mut base, &mut cx, 100.0, 0.0);
        let e = ev(&cx, 100.0, 100.0);
        base.pointer_move(&mut cx, e);
        let egui_ctx = egui::Context::default();
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(egui::vec2(400.0, 300.0), egui::Sense::hover());
                let mut cam = Camera::default_view();
                cam.rect = painter.clip_rect();
                framing_view::draw(&cx, &painter, &cam);
                base.draw_overlay(&cx, &painter, &cam);
                t.draw_overlay(&cx, &painter, &cam);
                post.draw_overlay(&cx, &painter, &cam);
            });
        });
    }
}
