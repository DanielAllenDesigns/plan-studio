//! The Fireplace tools (CB-87): Fireplace, Fireplace in Wall, Prefab
//! Fireplace and Chimney.
//!
//! One click places one fireplace as one undo step. Near a wall the
//! fireplace turns its back to the wall on the pointer's side (flush against
//! the face; *in the wall* its body reaches through the wall, which is cut in
//! 3D); elsewhere it stands free, `R` turning it a quarter. Ctrl/Cmd-click
//! selects a fireplace; a double-click on one opens its Fireplace
//! Specification (General, Hearth, Mantel, Chimney, Materials, Label, Layer).
//! The Edit toolbar's *Fireplace Specification* button (the Select tool) hands
//! the dialog to this tool too, see `editor::fireplace_view::request_open`.
//! Esc returns to Select Objects.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::fireplace::FireplaceDialog;
use crate::dialogs::Outcome;
use crate::editor::fireplace_view::{self as fv, Placement};
use crate::editor::selection::ObjectRef;
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Key, Pos2, Shape, Stroke};
use plan_core::fireplace::{
    body_poly, chimney_poly, hearth_poly, Fireplace, FireplaceKind, FIREPLACE_LAYER,
};
use plan_core::geometry::Point;
use plan_core::{Id, PlacedSymbol};
use std::cell::RefCell;

/// The flavors of the fireplace tool, one per flyout entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FireplaceMode {
    Masonry,
    InWall,
    Prefab,
    Chimney,
}

impl FireplaceMode {
    pub const ALL: [FireplaceMode; 4] = [
        FireplaceMode::Masonry,
        FireplaceMode::InWall,
        FireplaceMode::Prefab,
        FireplaceMode::Chimney,
    ];

    /// Chief's name of the tool.
    pub fn name(self) -> &'static str {
        match self {
            FireplaceMode::Masonry => "Fireplace",
            FireplaceMode::InWall => "Fireplace in Wall",
            FireplaceMode::Prefab => "Prefab Fireplace",
            FireplaceMode::Chimney => "Chimney",
        }
    }

    pub fn kind(self) -> FireplaceKind {
        match self {
            FireplaceMode::Masonry | FireplaceMode::InWall => FireplaceKind::Masonry,
            FireplaceMode::Prefab => FireplaceKind::Prefab,
            FireplaceMode::Chimney => FireplaceKind::ChimneyOnly,
        }
    }

    pub fn in_wall(self) -> bool {
        self == FireplaceMode::InWall
    }
}

pub struct FireplaceTool {
    mode: FireplaceMode,
    /// Where a click would place the fireplace.
    ghost: Option<Placement>,
    /// Angle of a free fireplace, degrees.
    free_angle: f64,
    /// The last pointer position over the plan.
    last_pointer: Option<Point>,
    dialog: RefCell<Option<FireplaceDialog>>,
    /// The edited specification, waiting to be written to the plan.
    applied: RefCell<Option<(PlacedSymbol, Fireplace)>>,
    /// The dialog was opened from the Select tool: go back to it afterwards.
    return_to_select: RefCell<bool>,
}

impl Default for FireplaceTool {
    fn default() -> Self {
        Self {
            mode: FireplaceMode::Masonry,
            ghost: None,
            free_angle: 0.0,
            last_pointer: None,
            dialog: RefCell::new(None),
            applied: RefCell::new(None),
            return_to_select: RefCell::new(false),
        }
    }
}

impl FireplaceTool {
    pub fn mode(&self) -> FireplaceMode {
        self.mode
    }

    pub fn ghost(&self) -> Option<Placement> {
        self.ghost
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog.borrow().is_some()
    }

    /// Opens the specification of fireplace `id`.
    pub fn open_spec(&self, cx: &EditorContext, id: Id) -> bool {
        let Some((sym, fp)) = fv::load(cx.floor(), id) else {
            return false;
        };
        let layers: Vec<String> = cx.layers().layers.iter().map(|l| l.name.clone()).collect();
        *self.dialog.borrow_mut() = Some(FireplaceDialog::new(sym, fp, layers));
        true
    }

    /// The dialog being shown (tests drive it).
    pub fn dialog_mut(&self) -> std::cell::RefMut<'_, Option<FireplaceDialog>> {
        self.dialog.borrow_mut()
    }

    /// Writes an accepted dialog and opens a requested one.
    fn flush(&mut self, cx: &mut EditorContext) -> Option<ToolResult> {
        if let Some(id) = fv::take_open() {
            if self.open_spec(cx, id) {
                *self.return_to_select.borrow_mut() = true;
            }
        }
        let (sym, fp) = self.applied.borrow_mut().take()?;
        let result = if fv::apply(cx, &sym, &fp) {
            ToolResult::committed("Fireplace Specification")
        } else {
            cx.status = "The fireplace is gone".into();
            ToolResult::consumed()
        };
        Some(result)
    }

    /// After a dialog opened from the Select tool closes, goes back to it.
    fn leave_when_closed(&self, cx: &mut EditorContext) {
        if !self.dialog_open()
            && self.applied.borrow().is_none()
            && self.return_to_select.replace(false)
        {
            cx.requests
                .push(crate::editor::EditorRequest::SetTool(ToolId::Select));
        }
    }

    fn update_ghost(&mut self, cx: &EditorContext, at: Point) {
        self.last_pointer = Some(at);
        self.ghost = Some(fv::placement_at(
            cx.floor(),
            at,
            self.mode.kind(),
            self.mode.in_wall(),
            self.free_angle,
        ));
    }

    /// A one-line description of the ghost for the status bar.
    fn ghost_text(&self) -> String {
        match self.ghost {
            Some(Placement {
                wall: Some(_),
                in_wall: true,
                ..
            }) => "Built into the wall: the wall is cut where the body stands".into(),
            Some(Placement { wall: Some(_), .. }) => "Against the wall".into(),
            Some(_) => "Free standing: R turns it".into(),
            None => String::new(),
        }
    }
}

impl Tool for FireplaceTool {
    fn id(&self) -> ToolId {
        ToolId::FireplaceVariant(self.mode)
    }

    fn name(&self) -> &'static str {
        self.mode.name()
    }

    fn hint(&self) -> String {
        match self.mode {
            FireplaceMode::Chimney => {
                "Click to place a chimney. Ctrl-click selects, double-click opens its specification"
                    .into()
            }
            FireplaceMode::InWall => {
                "Click near a wall to build a fireplace into it. Double-click opens its specification"
                    .into()
            }
            _ => "Click to place a fireplace; near a wall it goes against the wall. R turns a free one. Double-click opens its specification".into(),
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::FireplaceVariant(m) = id {
            self.mode = m;
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.ghost = None;
        cx.status.clear();
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.ghost = None;
        *self.dialog.borrow_mut() = None;
        *self.applied.borrow_mut() = None;
        *self.return_to_select.borrow_mut() = false;
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        let _ = self.flush(cx);
        self.leave_when_closed(cx);
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() {
            return ToolResult::ignored();
        }
        self.update_ghost(cx, p.world);
        cx.readout = Some(self.ghost_text());
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
        if p.button != egui::PointerButton::Primary {
            return ToolResult::ignored();
        }
        // Ctrl/Cmd-click picks a fireplace that is already there.
        if p.modifiers.command {
            let tol = 6.0 / cx.px_per_in.max(1e-6);
            return match fv::hit(cx.floor(), p.world, tol) {
                Some(id) => {
                    cx.selection.set(ObjectRef::Symbol(id));
                    ToolResult::consumed()
                }
                None => ToolResult::ignored(),
            };
        }
        let pl = fv::placement_at(
            cx.floor(),
            p.world,
            self.mode.kind(),
            self.mode.in_wall(),
            self.free_angle,
        );
        let label = if self.mode == FireplaceMode::Chimney {
            "Place Chimney"
        } else {
            "Place Fireplace"
        };
        fv::place(cx, self.mode.kind(), pl);
        ToolResult::committed(label)
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let tol = 6.0 / cx.px_per_in.max(1e-6);
        match fv::hit(cx.floor(), p.world, tol) {
            Some(id) if self.open_spec(cx, id) => {
                cx.selection.set(ObjectRef::Symbol(id));
                ToolResult::consumed()
            }
            _ => ToolResult::ignored(),
        }
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if self.dialog_open() {
            return ToolResult::ignored();
        }
        if k.is(Key::Escape) {
            return ToolResult {
                consumed: true,
                repaint: true,
                switch_to: Some(ToolId::Select),
                commit: None,
            };
        }
        if k.is(Key::R)
            || k.text
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("r"))
        {
            self.free_angle = (self.free_angle + 90.0).rem_euclid(360.0);
            if let Some(at) = self.last_pointer {
                self.update_ghost(cx, at);
            }
            cx.readout = Some(format!("Turned to {:.0} degrees", self.free_angle));
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        if let Some(pl) = self.ghost.filter(|_| !self.dialog_open()) {
            let kind = self.mode.kind();
            let (w, d) = Fireplace::default_size(kind);
            let mut sym = PlacedSymbol::new(kind.catalog_id(), pl.position, w, d, 96.0);
            sym.angle = pl.angle;
            sym.layer = FIREPLACE_LAYER.to_string();
            let mut fp = Fireplace::new(0, kind);
            fp.fit_to(w, d);
            let pal = &cx.palette;
            let stroke = Stroke::new(1.5_f32, pal.ghost_stroke);
            let outline = |poly: &[Point], fill: bool| {
                let pts: Vec<Pos2> = poly.iter().map(|q| cam.world_to_screen(*q)).collect();
                if pts.len() >= 3 {
                    if fill {
                        painter.add(Shape::convex_polygon(pts.clone(), pal.ghost_fill, stroke));
                    } else {
                        painter.add(Shape::closed_line(pts, stroke));
                    }
                }
            };
            outline(&body_poly(&sym), true);
            outline(&hearth_poly(&fp, &sym), false);
            if fp.chimney.enabled && kind != FireplaceKind::ChimneyOnly {
                outline(&chimney_poly(&fp, &sym), false);
            }
        }
        // The specification dialog.
        let mut slot = self.dialog.borrow_mut();
        let outcome = slot.as_mut().map(|d| d.show(painter.ctx()));
        match outcome {
            Some(Outcome::Ok) => {
                if let Some(d) = slot.take() {
                    *self.applied.borrow_mut() = Some((d.symbol().clone(), d.fireplace().clone()));
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
    use eframe::egui::Modifiers;
    use plan_core::{Wall, WallKind};

    fn cx() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut w = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(400.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        w.id = cx.project.alloc_id();
        cx.floor_mut().walls.push(w);
        cx
    }

    fn tool(m: FireplaceMode) -> FireplaceTool {
        let mut t = FireplaceTool::default();
        t.set_variant(ToolId::FireplaceVariant(m));
        t
    }

    fn click(t: &mut FireplaceTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        t.pointer_down(cx, p.with_down(true))
    }

    #[test]
    fn each_mode_has_chiefs_name_a_kind_and_a_hint() {
        let names: Vec<&str> = FireplaceMode::ALL.iter().map(|m| m.name()).collect();
        assert_eq!(
            names,
            [
                "Fireplace",
                "Fireplace in Wall",
                "Prefab Fireplace",
                "Chimney"
            ]
        );
        for m in FireplaceMode::ALL {
            let t = tool(m);
            assert_eq!(t.id(), ToolId::FireplaceVariant(m));
            assert_eq!(t.name(), m.name());
            assert!(!t.hint().is_empty());
        }
        assert!(FireplaceMode::InWall.in_wall());
        assert_eq!(FireplaceMode::Prefab.kind(), FireplaceKind::Prefab);
        assert_eq!(FireplaceMode::Chimney.kind(), FireplaceKind::ChimneyOnly);
    }

    #[test]
    fn hovering_near_a_wall_shows_a_ghost_against_it_and_a_click_places_it() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::Masonry);
        let p = PointerEvent::at(&cx, Point::new(200.0, 30.0));
        let r = t.pointer_move(&mut cx, p);
        assert!(r.repaint);
        let ghost = t.ghost().unwrap();
        assert!(ghost.wall.is_some() && !ghost.in_wall);
        assert!(cx
            .readout
            .as_deref()
            .is_some_and(|s| s.contains("Against the wall")));
        let r = click(&mut t, &mut cx, 200.0, 30.0);
        assert_eq!(r.commit.as_deref(), Some("Place Fireplace"));
        assert_eq!(cx.floor().fireplace_symbols().len(), 1);
        // A second click is a second fireplace.
        click(&mut t, &mut cx, 300.0, 30.0);
        assert_eq!(cx.floor().fireplace_symbols().len(), 2);
        cx.undo();
        assert_eq!(cx.floor().fireplace_symbols().len(), 1);
    }

    #[test]
    fn built_into_the_wall_and_chimney_modes_commit_with_their_own_words() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::InWall);
        let r = click(&mut t, &mut cx, 200.0, 30.0);
        assert_eq!(r.commit.as_deref(), Some("Place Fireplace"));
        let (_, fp) = cx.floor().fireplace_symbols().remove(0);
        assert!(fp.in_wall);
        assert!(cx.readout.is_some());
        let mut c = tool(FireplaceMode::Chimney);
        let r = click(&mut c, &mut cx, 300.0, 200.0);
        assert_eq!(r.commit.as_deref(), Some("Place Chimney"));
        assert!(cx.status.contains("chimney"));
    }

    #[test]
    fn r_turns_a_free_fireplace_a_quarter_but_not_one_on_a_wall() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::Masonry);
        let p = PointerEvent::at(&cx, Point::new(200.0, 250.0));
        t.pointer_move(&mut cx, p);
        assert_eq!(t.ghost().unwrap().angle, 0.0);
        assert!(t.key(&mut cx, KeyEvent::text("r")).consumed);
        assert_eq!(t.ghost().unwrap().angle, 90.0);
        assert!(t.key(&mut cx, KeyEvent::key(Key::R)).consumed);
        assert_eq!(t.ghost().unwrap().angle, 180.0);
        // Near the wall the angle comes from the wall.
        let p = PointerEvent::at(&cx, Point::new(200.0, 30.0));
        t.pointer_move(&mut cx, p);
        assert!(t.ghost().unwrap().angle.abs() < 1e-9);
        let r = click(&mut t, &mut cx, 200.0, 250.0);
        assert!(r.commit.is_some());
        let sym = cx.floor().fireplace_symbols().remove(0).0.clone();
        assert_eq!(sym.angle, 180.0);
    }

    #[test]
    fn escape_goes_back_to_select_objects() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::Masonry);
        let r = t.key(&mut cx, KeyEvent::escape());
        assert!(r.consumed);
        assert_eq!(r.switch_to, Some(ToolId::Select));
    }

    #[test]
    fn ctrl_click_picks_an_existing_fireplace_without_placing_another() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::Masonry);
        click(&mut t, &mut cx, 200.0, 30.0);
        let sym = cx.floor().fireplace_symbols().remove(0).0.clone();
        cx.selection.clear();
        let at = Point::new(sym.position.x, sym.position.y + 10.0);
        let p = PointerEvent::at(&cx, at)
            .with_down(true)
            .with_modifiers(Modifiers::COMMAND);
        let r = t.pointer_down(&mut cx, p);
        assert!(r.consumed && r.commit.is_none());
        assert_eq!(cx.selection.single(), Some(ObjectRef::Symbol(sym.id)));
        assert_eq!(cx.floor().fireplace_symbols().len(), 1);
        // Ctrl-click on nothing does nothing.
        let p = PointerEvent::at(&cx, Point::new(350.0, 300.0))
            .with_down(true)
            .with_modifiers(Modifiers::COMMAND);
        assert!(!t.pointer_down(&mut cx, p).consumed);
    }

    #[test]
    fn a_double_click_opens_the_dialog_and_ok_writes_the_edit_as_one_step() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::Masonry);
        click(&mut t, &mut cx, 200.0, 30.0);
        let sym = cx.floor().fireplace_symbols().remove(0).0.clone();
        let at = Point::new(sym.position.x, sym.position.y + 10.0);
        let p = PointerEvent::at(&cx, at).with_down(true);
        assert!(t.double_click(&mut cx, p).consumed);
        assert!(t.dialog_open());
        {
            let mut slot = t.dialog_mut();
            let d = slot.as_mut().unwrap();
            d.fireplace_mut().name = "Great Room Fireplace".into();
            d.fireplace_mut().hearth.projection = 20.0;
            d.symbol_mut().width = 84.0;
        }
        // OK: the shell's next frame writes it.
        let d = t.dialog_mut().take().unwrap();
        *t.applied.borrow_mut() = Some((d.symbol().clone(), d.fireplace().clone()));
        let ctx = egui::Context::default();
        t.frame(&mut cx, &ctx);
        let (sym2, fp2) = fv::load(cx.floor(), sym.id).unwrap();
        assert_eq!(fp2.name, "Great Room Fireplace");
        assert_eq!(fp2.hearth.projection, 20.0);
        assert_eq!(sym2.width, 84.0);
        assert_eq!(sym2.label, "Great Room Fireplace");
        cx.undo();
        let (sym3, fp3) = fv::load(cx.floor(), sym.id).unwrap();
        assert_eq!((fp3.name.as_str(), sym3.width), ("Fireplace", 72.0));
        // A double-click on nothing is left alone.
        let p = PointerEvent::at(&cx, Point::new(350.0, 300.0)).with_down(true);
        assert!(!t.double_click(&mut cx, p).consumed);
    }

    #[test]
    fn a_request_from_the_edit_toolbar_opens_the_dialog_and_cancel_returns_to_select() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::Masonry);
        click(&mut t, &mut cx, 200.0, 30.0);
        let sym = cx.floor().fireplace_symbols().remove(0).0.clone();
        fv::request_open(sym.id);
        let ctx = egui::Context::default();
        t.frame(&mut cx, &ctx);
        assert!(t.dialog_open());
        assert!(cx.requests.is_empty());
        t.dialog_mut().take();
        t.frame(&mut cx, &ctx);
        assert!(cx
            .requests
            .contains(&crate::editor::EditorRequest::SetTool(ToolId::Select)));
        // A request for something that is not a fireplace opens nothing.
        fv::request_open(9999);
        t.frame(&mut cx, &ctx);
        assert!(!t.dialog_open());
    }

    #[test]
    fn leaving_the_tool_drops_the_ghost_and_the_dialog() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::Masonry);
        click(&mut t, &mut cx, 200.0, 30.0);
        let sym = cx.floor().fireplace_symbols().remove(0).0.clone();
        assert!(t.open_spec(&cx, sym.id));
        t.deactivate(&mut cx);
        assert!(t.ghost().is_none() && !t.dialog_open());
    }

    #[test]
    fn the_ghost_and_the_dialog_draw() {
        let mut cx = cx();
        let mut t = tool(FireplaceMode::Prefab);
        let p = PointerEvent::at(&cx, Point::new(200.0, 30.0));
        t.pointer_move(&mut cx, p);
        click(&mut t, &mut cx, 200.0, 30.0);
        let sym = cx.floor().fireplace_symbols().remove(0).0.clone();
        let p = PointerEvent::at(&cx, Point::new(250.0, 40.0));
        t.pointer_move(&mut cx, p);
        t.open_spec(&cx, sym.id);
        let ctx = egui::Context::default();
        let cam = Camera::default_view();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let painter = ui.painter().clone();
                    t.draw_overlay(&cx, &painter, &cam);
                });
            });
        }
        assert!(t.dialog_open());
    }
}
