//! The Trim flyout (Corner Boards, Auto Place Corner Boards, Quoins, Auto
//! Place Quoins, Molding Line, Molding Polyline), Floor Material Region, the
//! wall flyout's Wall Material Region, Wall Hatching and Slab Footing, Polygon
//! Shaped Deck and the 3D Solid flyout (3D Solid, Face, Cone, Cylinder,
//! Pyramid, Sphere) (`docs/chief-x18-subtools.md`).
//!
//! One tool object with a flavor per flyout entry ([`DetailsVariant`]):
//!
//! * the auto flavors build the trim on every convex exterior corner of the
//!   floor with one click; the manual flavors place it on the corner clicked;
//! * Molding Line takes two clicks (or a drag); Molding Polyline takes a click
//!   per corner and a double-click (or Enter) to finish;
//! * the polygon flavors (Floor Material Region, Polygon Shaped Deck, Slab
//!   Footing, 3D Solid, Pyramid, Face) work like Chief's Polyline: click the
//!   corners, a double-click, Enter or a click on the first corner closes the
//!   shape; dragging from the first click draws a rectangle (a 3D Solid
//!   dragged this way is a box);
//! * Cone, Cylinder and Sphere take the center and then the radius (a second
//!   click, or drag from the center);
//! * Wall Material Region: click a wall, or press on it and drag along it for
//!   part of its length; the heights (bottom 0, top the wall height) and the
//!   face are edited in the dialog that opens (the simple alternative to a
//!   mini elevation popup);
//! * Wall Hatching: click a wall; the dialog opens to pick the pattern;
//! * a new solid, wall region or hatch opens its dialog so the height (or
//!   range) can be set right away.
//!
//! Ctrl/Cmd-click picks an existing object, Ctrl/Cmd-drag moves it, Delete (or
//! Backspace) deletes the selected or hovered one, and a double-click on one
//! outside a drawing opens its specification dialog. Every change is one undo
//! step. The objects live in the floor's `DetailsLayer`
//! (`editor::details_view`).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::details::{DetailsDialog, Draft};
use crate::dialogs::Outcome;
use crate::editor::details_view as dv;
use crate::editor::foundation_view as fv;
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Key, Stroke};
use plan_core::details::{
    centroid, circle_points, DetailRef, DetailsLayer, ExteriorCorner, MoldingProfile, SolidKind,
};
use plan_core::foundation::{outline_area, rect_outline, FoundationRef};
use plan_core::geometry::{project_on_segment, Point};
use plan_core::LineStyle;
use std::cell::RefCell;

/// A drag shorter than this many screen pixels is a click.
const DRAG_PX: f64 = 6.0;
/// Height of a new 3D solid, inches (set in its dialog).
pub const DEFAULT_SOLID_HEIGHT: f64 = 48.0;
/// A radius under this is a stray second click, inches.
const MIN_RADIUS: f64 = 1.0;

/// The flavors of the details tool, one per flyout entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetailsVariant {
    CornerBoards,
    AutoCornerBoards,
    Quoins,
    AutoQuoins,
    MoldingLine,
    MoldingPolyline,
    FloorMaterialRegion,
    WallMaterialRegion,
    WallHatching,
    PolygonDeck,
    SlabFooting,
    Solid3d,
    Face,
    Cone,
    Cylinder,
    Pyramid,
    Sphere,
}

/// What the clicks of a flavor do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Draw {
    /// One click builds trim on every exterior corner.
    Auto,
    /// One click places trim on the corner clicked.
    Corner,
    Line,
    Polyline,
    Polygon,
    /// Center, then radius.
    Radius,
    /// A wall, optionally a range along it.
    Wall,
    /// A wall, then the dialog.
    Hatch,
}

impl DetailsVariant {
    pub const ALL: [DetailsVariant; 17] = [
        DetailsVariant::CornerBoards,
        DetailsVariant::AutoCornerBoards,
        DetailsVariant::Quoins,
        DetailsVariant::AutoQuoins,
        DetailsVariant::MoldingLine,
        DetailsVariant::MoldingPolyline,
        DetailsVariant::FloorMaterialRegion,
        DetailsVariant::WallMaterialRegion,
        DetailsVariant::WallHatching,
        DetailsVariant::PolygonDeck,
        DetailsVariant::SlabFooting,
        DetailsVariant::Solid3d,
        DetailsVariant::Face,
        DetailsVariant::Cone,
        DetailsVariant::Cylinder,
        DetailsVariant::Pyramid,
        DetailsVariant::Sphere,
    ];

    /// Chief's name of the tool.
    pub fn name(self) -> &'static str {
        match self {
            DetailsVariant::CornerBoards => "Corner Boards",
            DetailsVariant::AutoCornerBoards => "Auto Place Corner Boards",
            DetailsVariant::Quoins => "Quoins",
            DetailsVariant::AutoQuoins => "Auto Place Quoins",
            DetailsVariant::MoldingLine => "Molding Line",
            DetailsVariant::MoldingPolyline => "Molding Polyline",
            DetailsVariant::FloorMaterialRegion => "Floor Material Region",
            DetailsVariant::WallMaterialRegion => "Wall Material Region",
            DetailsVariant::WallHatching => "Wall Hatching",
            DetailsVariant::PolygonDeck => "Polygon Shaped Deck",
            DetailsVariant::SlabFooting => "Slab Footing",
            DetailsVariant::Solid3d => "3D Solid",
            DetailsVariant::Face => "Face",
            DetailsVariant::Cone => "Cone",
            DetailsVariant::Cylinder => "Cylinder",
            DetailsVariant::Pyramid => "Pyramid",
            DetailsVariant::Sphere => "Sphere",
        }
    }

    fn draw(self) -> Draw {
        match self {
            DetailsVariant::AutoCornerBoards | DetailsVariant::AutoQuoins => Draw::Auto,
            DetailsVariant::CornerBoards | DetailsVariant::Quoins => Draw::Corner,
            DetailsVariant::MoldingLine => Draw::Line,
            DetailsVariant::MoldingPolyline => Draw::Polyline,
            DetailsVariant::Cone | DetailsVariant::Cylinder | DetailsVariant::Sphere => {
                Draw::Radius
            }
            DetailsVariant::WallMaterialRegion => Draw::Wall,
            DetailsVariant::WallHatching => Draw::Hatch,
            _ => Draw::Polygon,
        }
    }
}

/// A move in progress (Ctrl/Cmd-drag).
struct MoveState {
    target: DetailRef,
    start: Point,
    /// The layer as it was when the drag began; every step moves from it.
    base: DetailsLayer,
    moved: bool,
}

pub struct DetailsTool {
    variant: DetailsVariant,
    points: Vec<Point>,
    hover: Option<Point>,
    /// Where the button went down on an empty drawing (a possible drag).
    press: Option<Point>,
    /// The rectangle being dragged: first corner, current corner.
    rect: Option<(Point, Point)>,
    /// The wall pressed by Wall Material Region, with the pointer position.
    wall_press: Option<dv::WallHit>,
    wall_drag_u: Option<f64>,
    hover_corner: Option<ExteriorCorner>,
    moving: Option<MoveState>,
    dialog: RefCell<Option<DetailsDialog>>,
    applied: RefCell<Option<Draft>>,
}

impl Default for DetailsTool {
    fn default() -> Self {
        Self {
            variant: DetailsVariant::CornerBoards,
            points: Vec::new(),
            hover: None,
            press: None,
            rect: None,
            wall_press: None,
            wall_drag_u: None,
            hover_corner: None,
            moving: None,
            dialog: RefCell::new(None),
            applied: RefCell::new(None),
        }
    }
}

impl DetailsTool {
    pub fn variant(&self) -> DetailsVariant {
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
        self.wall_press = None;
        self.wall_drag_u = None;
        cx.readout = None;
    }

    /// Applies an OK from the specification dialog, as one undo step.
    fn flush(&mut self, cx: &mut EditorContext) -> Option<ToolResult> {
        let draft = self.applied.borrow_mut().take()?;
        let label = draft.title();
        let mut found = false;
        dv::edit(cx, label, |l| found = draft.apply(l));
        if !found {
            cx.cancel_change();
            cx.status = "The object is gone".into();
            return Some(ToolResult::consumed());
        }
        Some(ToolResult::committed(label))
    }

    /// Opens the specification dialog of `r`.
    pub fn open_spec(&self, cx: &EditorContext, r: DetailRef) -> bool {
        let layer = dv::load(cx);
        let mut names: Vec<String> = cx
            .project
            .layers
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect();
        if let Some(own) = layer.layer_of(r) {
            if !names.contains(&own) {
                names.push(own);
            }
        }
        let dialog = DetailsDialog::new(&layer, r, names);
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
            (None, Some(a), Some(h)) if self.variant.draw() == Draw::Radius => {
                Some(format!("Radius: {}", cx.fmt_dim(a.dist(h))))
            }
            (None, Some(a), Some(h)) => Some(format!("Length: {}", cx.fmt_dim(a.dist(h)))),
            _ => None,
        };
    }

    // ----- creating -----

    fn finish_ok(&mut self, cx: &mut EditorContext, r: DetailRef, open: bool) {
        dv::select(r);
        self.reset(cx);
        cx.status.clear();
        if open {
            self.open_spec(cx, r);
        }
    }

    /// Stores the finished polygon as the object of the current flavor.
    fn create_polygon(
        &mut self,
        cx: &mut EditorContext,
        outline: Vec<Point>,
        rect: bool,
    ) -> ToolResult {
        let v = self.variant;
        if outline.len() < 3 || outline_area(&outline) < dv::MIN_AREA {
            cx.status = format!("{} needs at least 3 corners", v.name());
            self.reset(cx);
            return ToolResult::consumed();
        }
        // Solids keep their outline relative to a position (the centroid).
        let at = centroid(&outline);
        let relative = |pts: &[Point]| pts.iter().map(|p| *p - at).collect::<Vec<_>>();
        match v {
            DetailsVariant::FloorMaterialRegion => {
                let id = dv::add_floor_region(cx, outline);
                self.finish_ok(cx, DetailRef::Region(id), false);
            }
            DetailsVariant::PolygonDeck => {
                let id = dv::add_deck(cx, outline);
                self.finish_ok(cx, DetailRef::Deck(id), false);
            }
            DetailsVariant::SlabFooting => {
                let id = fv::add_slab(cx, outline, true);
                fv::select(cx, FoundationRef::Slab(id));
                self.reset(cx);
                cx.status.clear();
            }
            DetailsVariant::Solid3d | DetailsVariant::Pyramid | DetailsVariant::Face => {
                let kind = match v {
                    DetailsVariant::Solid3d if rect => {
                        let (lo, hi) = plan_core::details::bounds(&outline);
                        SolidKind::Box {
                            w: hi.x - lo.x,
                            d: hi.y - lo.y,
                            h: DEFAULT_SOLID_HEIGHT,
                        }
                    }
                    DetailsVariant::Solid3d => SolidKind::PolylineSolid {
                        outline: relative(&outline),
                        h: DEFAULT_SOLID_HEIGHT,
                    },
                    DetailsVariant::Pyramid => SolidKind::Pyramid {
                        outline: relative(&outline),
                        h: DEFAULT_SOLID_HEIGHT,
                    },
                    _ => SolidKind::Face {
                        polygon: relative(&outline),
                    },
                };
                let id = dv::add_solid(cx, kind, at);
                self.finish_ok(cx, DetailRef::Solid(id), true);
            }
            _ => return ToolResult::consumed(),
        }
        ToolResult::committed(v.name())
    }

    /// Closes the polygon being drawn.
    fn finish_polygon(&mut self, cx: &mut EditorContext) -> ToolResult {
        if self.points.len() < 3 {
            cx.status = format!("{} needs at least 3 corners", self.variant.name());
            return ToolResult::consumed();
        }
        let pts = std::mem::take(&mut self.points);
        self.create_polygon(cx, pts, false)
    }

    /// Ends the molding line or polyline being drawn.
    fn finish_molding(&mut self, cx: &mut EditorContext) -> ToolResult {
        if self.points.len() < 2 {
            cx.status = format!("{} needs at least 2 points", self.variant.name());
            return ToolResult::consumed();
        }
        let pts = std::mem::take(&mut self.points);
        let id = dv::add_molding(cx, pts, MoldingProfile::Crown);
        self.finish_ok(cx, DetailRef::Molding(id), false);
        ToolResult::committed(self.variant.name())
    }

    /// Makes the round solid of the current flavor.
    fn create_round(&mut self, cx: &mut EditorContext, center: Point, radius: f64) -> ToolResult {
        let h = DEFAULT_SOLID_HEIGHT;
        let kind = match self.variant {
            DetailsVariant::Cone => SolidKind::Cone { r: radius, h },
            DetailsVariant::Cylinder => SolidKind::Cylinder { r: radius, h },
            _ => SolidKind::Sphere { r: radius },
        };
        let id = dv::add_solid(cx, kind, center);
        self.finish_ok(cx, DetailRef::Solid(id), true);
        ToolResult::committed(self.variant.name())
    }

    fn auto_place(&mut self, cx: &mut EditorContext) -> ToolResult {
        let (n, what) = match self.variant {
            DetailsVariant::AutoQuoins => (dv::auto_quoins(cx), "quoins"),
            _ => (dv::auto_corner_boards(cx), "corner boards"),
        };
        if n == 0 {
            cx.status = format!("No new exterior corners for {what}");
            return ToolResult::consumed();
        }
        cx.status = format!(
            "Placed {what} on {n} corner{}",
            if n == 1 { "" } else { "s" }
        );
        ToolResult::committed(self.variant.name())
    }

    fn place_on_corner(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let tol = cx.pick_tol() * 2.0;
        let Some(corner) = dv::corner_at(cx, p.world, tol) else {
            cx.status = "Click on a corner of the exterior walls".into();
            return ToolResult::consumed();
        };
        let layer = dv::load(cx);
        let taken = match self.variant {
            DetailsVariant::Quoins => layer
                .quoins
                .iter()
                .any(|q| q.corner.dist(corner.apex) < 1.0),
            _ => layer
                .corner_boards
                .iter()
                .any(|b| b.wall_corner.dist(corner.apex) < 1.0),
        };
        if taken {
            cx.status = format!(
                "That corner already has {}",
                self.variant.name().to_lowercase()
            );
            return ToolResult::consumed();
        }
        let r = match self.variant {
            DetailsVariant::Quoins => DetailRef::Quoin(dv::add_quoin(cx, &corner)),
            _ => DetailRef::CornerBoard(dv::add_corner_board(cx, &corner)),
        };
        dv::select(r);
        cx.status.clear();
        ToolResult::committed(self.variant.name())
    }

    /// The wall region from a press at `hit` released at `world`.
    fn create_wall_region(
        &mut self,
        cx: &mut EditorContext,
        hit: dv::WallHit,
        world: Point,
    ) -> ToolResult {
        let Some(w) = cx.floor().wall(hit.wall) else {
            return ToolResult::consumed();
        };
        let (t, _) = project_on_segment(world, w.start, w.end);
        let u2 = t * hit.length;
        let slop = DRAG_PX / cx.px_per_in.max(1e-6);
        let (u0, u1) = if (u2 - hit.u).abs() > slop {
            (hit.u.min(u2), hit.u.max(u2))
        } else {
            (0.0, hit.length)
        };
        let id = dv::add_wall_region(cx, hit.wall, hit.side, (u0, u1), (0.0, hit.height));
        self.finish_ok(cx, DetailRef::Region(id), true);
        ToolResult::committed(self.variant.name())
    }

    fn hatch_wall(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(hit) = dv::wall_at(cx, p.world, cx.pick_tol()) else {
            cx.status = "Click on a wall".into();
            return ToolResult::consumed();
        };
        let (id, added) = dv::wall_hatch(cx, hit.wall);
        self.finish_ok(cx, DetailRef::Hatch(id), true);
        if added {
            ToolResult::committed(self.variant.name())
        } else {
            ToolResult::consumed()
        }
    }

    // ----- editing existing objects -----

    /// The object to delete: the selected one, else the one under the pointer.
    fn delete_target(&self, cx: &EditorContext) -> Option<DetailRef> {
        dv::selected()
            .filter(|r| dv::exists(cx, *r))
            .or_else(|| self.hover.and_then(|h| dv::pick(cx, h, cx.pick_tol())))
    }

    fn delete_current(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(r) = self.delete_target(cx) else {
            return ToolResult::ignored();
        };
        let label = format!("Delete {}", r.name());
        dv::delete(cx, r);
        ToolResult {
            commit: Some(label),
            ..ToolResult::consumed()
        }
    }

    fn is_move_click(p: &PointerEvent) -> bool {
        p.modifiers.command || p.modifiers.ctrl || p.modifiers.mac_cmd
    }

    // ----- overlay -----

    fn draw_progress(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let ink = cx.palette.ghost_stroke;
        let stroke = Stroke::new(1.6_f32, ink);
        let line = |pts: &[Point], closed: bool, s: Stroke| {
            dv::stroke_line(painter, cam, pts, closed, s, LineStyle::Dashed);
        };
        if let Some((a, b)) = self.rect {
            line(&rect_outline(a, b), true, stroke);
            return;
        }
        if let Some(c) = self.hover_corner {
            painter.circle_stroke(
                cam.world_to_screen(c.apex),
                7.0,
                Stroke::new(2.0_f32, cx.palette.hover),
            );
        }
        match self.variant.draw() {
            Draw::Polygon | Draw::Line | Draw::Polyline => {
                let mut shape = self.points.clone();
                shape.extend(self.hover.filter(|_| !self.points.is_empty()));
                line(&shape, false, stroke);
                if self.variant.draw() == Draw::Polygon && self.points.len() >= 3 {
                    if let Some(h) = self.hover {
                        line(
                            &[h, self.points[0]],
                            false,
                            Stroke::new(1.0_f32, ink.gamma_multiply(0.5)),
                        );
                    }
                }
                for p in &self.points {
                    painter.circle_filled(cam.world_to_screen(*p), 3.0, ink);
                }
            }
            Draw::Radius => {
                if let (Some(c), Some(h)) = (self.points.first(), self.hover) {
                    let r = c.dist(h);
                    if r > MIN_RADIUS {
                        line(&circle_points(*c, r), true, stroke);
                        line(
                            &[*c, h],
                            false,
                            Stroke::new(1.0_f32, ink.gamma_multiply(0.5)),
                        );
                    }
                }
            }
            Draw::Wall | Draw::Hatch => {
                let wall = self
                    .wall_press
                    .map(|h| h.wall)
                    .or_else(|| {
                        self.hover
                            .and_then(|h| dv::wall_at(cx, h, cx.pick_tol()))
                            .map(|h| h.wall)
                    })
                    .and_then(|id| cx.floor().wall(id));
                if let Some(w) = wall {
                    let (u0, u1) = match (self.wall_press, self.wall_drag_u) {
                        (Some(h), Some(u)) => (h.u.min(u), h.u.max(u)),
                        _ => (0.0, w.length()),
                    };
                    let strip = plan_core::details::wall_strip(w, u0, u1);
                    line(&strip, true, Stroke::new(2.0_f32, cx.palette.hover));
                }
            }
            Draw::Auto | Draw::Corner => {}
        }
    }
}

impl Tool for DetailsTool {
    fn id(&self) -> ToolId {
        ToolId::DetailsVariant(self.variant)
    }

    fn name(&self) -> &'static str {
        self.variant.name()
    }

    fn hint(&self) -> String {
        let v = self.variant;
        match v.draw() {
            Draw::Auto => format!("{}: click to place on every exterior corner", v.name()),
            Draw::Corner => format!("{}: click an exterior corner", v.name()),
            Draw::Line => format!("{}: click the start and the end", v.name()),
            Draw::Polyline => format!("{}: click the corners, double-click finishes", v.name()),
            Draw::Polygon => format!(
                "{}: click the corners, double-click closes; drag for a rectangle",
                v.name()
            ),
            Draw::Radius => format!(
                "{}: click the center, then click or drag the radius",
                v.name()
            ),
            Draw::Wall => format!(
                "{}: click a wall, or drag along it for part of its length",
                v.name()
            ),
            Draw::Hatch => format!("{}: click a wall", v.name()),
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::DetailsVariant(v) = id {
            if v != self.variant {
                self.points.clear();
                self.press = None;
                self.rect = None;
                self.wall_press = None;
                self.hover_corner = None;
            }
            self.variant = v;
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.reset(cx);
        self.hover = None;
        self.hover_corner = None;
        cx.status.clear();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.reset(cx);
        self.moving = None;
        self.hover_corner = None;
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
                    dv::save(&mut cx.project, fl, &layer);
                    cx.mark_dirty();
                    m.moved = true;
                    cx.readout = Some(format!("Move: {}", cx.fmt_dim(d.length())));
                }
            }
            return ToolResult::consumed();
        }
        let draw = self.variant.draw();
        self.hover = Some(match draw {
            Draw::Corner | Draw::Wall | Draw::Hatch | Draw::Auto => p.world,
            _ if self.points.is_empty() => p.snapped,
            _ => self.snapped(cx, &p),
        });
        if draw == Draw::Corner {
            self.hover_corner = dv::corner_at(cx, p.world, cx.pick_tol() * 2.0);
        }
        if let (Some(hit), true) = (self.wall_press, p.down) {
            if let Some(w) = cx.floor().wall(hit.wall) {
                let (t, _) = project_on_segment(p.world, w.start, w.end);
                let u = t * hit.length;
                self.wall_drag_u = Some(u);
                cx.readout = Some(format!("Length: {}", cx.fmt_dim((u - hit.u).abs())));
            }
        }
        if let (Some(a), true) = (self.press, p.down) {
            let slop = DRAG_PX / cx.px_per_in.max(1e-6);
            if draw == Draw::Polygon && (self.rect.is_some() || p.world.dist(a) > slop) {
                let corner = cx.snap_at(p.world, None, p.modifiers.alt, &[]).point;
                self.rect = Some((a, corner));
            }
        }
        if self.wall_press.is_none() {
            self.update_readout(cx);
        }
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
        let draw = self.variant.draw();
        if self.points.is_empty() && draw != Draw::Auto && Self::is_move_click(&p) {
            return match dv::pick(cx, p.world, cx.pick_tol()) {
                Some(target) => {
                    dv::select(target);
                    cx.begin_change(&format!("Move {}", target.name()));
                    self.moving = Some(MoveState {
                        target,
                        start: cx.snap_at(p.world, None, false, &[]).point,
                        base: dv::load(cx),
                        moved: false,
                    });
                    ToolResult::consumed()
                }
                None => {
                    dv::clear_selection();
                    ToolResult::consumed()
                }
            };
        }
        match draw {
            Draw::Auto => self.auto_place(cx),
            Draw::Corner => self.place_on_corner(cx, &p),
            Draw::Hatch => self.hatch_wall(cx, &p),
            Draw::Wall => {
                match dv::wall_at(cx, p.world, cx.pick_tol()) {
                    Some(hit) => {
                        self.wall_press = Some(hit);
                        self.wall_drag_u = None;
                    }
                    None => cx.status = "Click on a wall".into(),
                }
                ToolResult::consumed()
            }
            Draw::Line | Draw::Polyline => {
                let pt = if self.points.is_empty() {
                    p.snapped
                } else {
                    self.snapped(cx, &p)
                };
                if self.points.is_empty() {
                    self.press = Some(pt);
                }
                if self.points.last().is_none_or(|l| l.dist(pt) > 0.5) {
                    self.points.push(pt);
                    if draw == Draw::Line && self.points.len() == 2 {
                        return self.finish_molding(cx);
                    }
                }
                self.update_readout(cx);
                ToolResult::consumed()
            }
            Draw::Radius => {
                let pt = if self.points.is_empty() {
                    p.snapped
                } else {
                    self.snapped(cx, &p)
                };
                match self.points.first().copied() {
                    None => {
                        self.points.push(pt);
                        self.press = Some(pt);
                    }
                    Some(c) => {
                        let r = c.dist(pt);
                        if r > MIN_RADIUS {
                            self.points.clear();
                            return self.create_round(cx, c, r);
                        }
                    }
                }
                self.update_readout(cx);
                ToolResult::consumed()
            }
            Draw::Polygon => {
                let pt = if self.points.is_empty() {
                    p.snapped
                } else {
                    self.snapped(cx, &p)
                };
                let closes =
                    self.points.len() >= 3 && p.world.dist(self.points[0]) <= cx.pick_tol() * 1.5;
                if closes {
                    return self.finish_polygon(cx);
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
        if let Some(hit) = self.wall_press.take() {
            self.wall_drag_u = None;
            cx.readout = None;
            return self.create_wall_region(cx, hit, p.world);
        }
        let press = self.press.take();
        if let Some((a, _)) = self.rect.take() {
            let corner = cx.snap_at(p.world, None, p.modifiers.alt, &[]).point;
            self.points.clear();
            return self.create_polygon(cx, rect_outline(a, corner), true);
        }
        // A drag from the first click ends a line or a radius.
        if let (Some(a), 1) = (press, self.points.len()) {
            let slop = DRAG_PX / cx.px_per_in.max(1e-6);
            if p.world.dist(a) > slop {
                let end = cx.snap_at(p.world, None, p.modifiers.alt, &[]).point;
                match self.variant.draw() {
                    Draw::Line => {
                        self.points.push(end);
                        return self.finish_molding(cx);
                    }
                    Draw::Radius if a.dist(end) > MIN_RADIUS => {
                        self.points.clear();
                        return self.create_round(cx, a, a.dist(end));
                    }
                    _ => {}
                }
            }
        }
        ToolResult::ignored()
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        match self.variant.draw() {
            Draw::Polygon if self.points.len() >= 2 => return self.finish_polygon(cx),
            Draw::Polyline if self.points.len() >= 2 => return self.finish_molding(cx),
            _ => {}
        }
        // The click that started the double-click left at most one stray
        // point (or placed trim): it opens the dialog instead.
        self.reset(cx);
        match dv::pick(cx, p.world, cx.pick_tol()) {
            Some(r) => {
                dv::select(r);
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
        let draw = self.variant.draw();
        if k.is(Key::Enter) {
            return match draw {
                _ if self.points.is_empty() => ToolResult::ignored(),
                Draw::Polygon => self.finish_polygon(cx),
                Draw::Polyline | Draw::Line => self.finish_molding(cx),
                _ => ToolResult::ignored(),
            };
        }
        if k.is(Key::Escape) {
            if self.points.is_empty() && self.wall_press.is_none() {
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
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn tool(v: DetailsVariant) -> DetailsTool {
        let mut t = DetailsTool::default();
        t.set_variant(ToolId::DetailsVariant(v));
        t
    }

    fn layer(cx: &EditorContext) -> DetailsLayer {
        dv::load(cx)
    }

    fn click(t: &mut DetailsTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        let r = t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
        r
    }

    fn dbl(t: &mut DetailsTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.double_click(cx, p)
    }

    fn drag(
        t: &mut DetailsTool,
        cx: &mut EditorContext,
        from: (f64, f64),
        to: (f64, f64),
    ) -> ToolResult {
        let a = PointerEvent::at(cx, Point::new(from.0, from.1));
        t.pointer_move(cx, a);
        t.pointer_down(cx, a.with_down(true));
        let b = PointerEvent::at(cx, Point::new(to.0, to.1)).with_down(true);
        t.pointer_move(cx, b);
        t.pointer_up(cx, b)
    }

    fn box_walls(cx: &mut EditorContext) {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.5, 108.0, WallKind::Exterior);
        }
        cx.mark_dirty();
    }

    fn triangle(t: &mut DetailsTool, cx: &mut EditorContext) -> ToolResult {
        click(t, cx, 480.0, 0.0);
        click(t, cx, 720.0, 0.0);
        click(t, cx, 720.0, 120.0);
        dbl(t, cx, 720.0, 120.0)
    }

    #[test]
    fn one_click_places_corner_boards_on_every_exterior_corner() {
        let mut cx = cx();
        box_walls(&mut cx);
        let mut t = tool(DetailsVariant::AutoCornerBoards);
        let r = click(&mut t, &mut cx, 120.0, 90.0);
        assert_eq!(r.commit.as_deref(), Some("Auto Place Corner Boards"));
        assert_eq!(layer(&cx).corner_boards.len(), 4);
        assert!(cx.status.contains("4 corners"));
        // A second click adds nothing and leaves no undo step.
        let r = click(&mut t, &mut cx, 120.0, 90.0);
        assert!(r.commit.is_none());
        assert_eq!(layer(&cx).corner_boards.len(), 4);
        assert_eq!(cx.undo().as_deref(), Some("Auto Place Corner Boards"));
        assert!(layer(&cx).is_empty());
    }

    #[test]
    fn auto_place_quoins_alternates_and_needs_walls() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::AutoQuoins);
        let r = click(&mut t, &mut cx, 0.0, 0.0);
        assert!(r.commit.is_none());
        assert!(cx.status.contains("No new"));
        box_walls(&mut cx);
        let r = click(&mut t, &mut cx, 0.0, 0.0);
        assert_eq!(r.commit.as_deref(), Some("Auto Place Quoins"));
        let l = layer(&cx);
        assert_eq!(l.quoins.len(), 4);
        assert!(l.quoins.iter().all(|q| q.alternating));
    }

    #[test]
    fn manual_corner_boards_and_quoins_go_on_the_clicked_corner() {
        let mut cx = cx();
        box_walls(&mut cx);
        let mut t = tool(DetailsVariant::CornerBoards);
        // Away from every corner nothing is placed.
        let r = click(&mut t, &mut cx, 120.0, 90.0);
        assert!(r.commit.is_none());
        assert!(cx.status.contains("corner"));
        let r = click(&mut t, &mut cx, 238.0, 2.0);
        assert_eq!(r.commit.as_deref(), Some("Corner Boards"));
        let l = layer(&cx);
        assert_eq!(l.corner_boards.len(), 1);
        assert!(
            l.corner_boards[0]
                .wall_corner
                .dist(Point::new(243.25, -3.25))
                < 1e-6
        );
        // The same corner twice is refused.
        let r = click(&mut t, &mut cx, 238.0, 2.0);
        assert!(r.commit.is_none());
        assert_eq!(layer(&cx).corner_boards.len(), 1);
        let mut q = tool(DetailsVariant::Quoins);
        let r = click(&mut q, &mut cx, 2.0, 2.0);
        assert_eq!(r.commit.as_deref(), Some("Quoins"));
        assert_eq!(layer(&cx).quoins.len(), 1);
        assert_eq!(cx.undo().as_deref(), Some("Quoins"));
    }

    #[test]
    fn molding_lines_and_polylines() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::MoldingLine);
        click(&mut t, &mut cx, 0.0, 0.0);
        assert_eq!(t.points().len(), 1);
        let r = click(&mut t, &mut cx, 120.0, 0.0);
        assert_eq!(r.commit.as_deref(), Some("Molding Line"));
        let m = layer(&cx).moldings[0].clone();
        assert_eq!(m.polyline.len(), 2);
        assert_eq!(m.length(), 120.0);
        assert!(t.points().is_empty());
        // Dragging also draws a line.
        let r = drag(&mut t, &mut cx, (0.0, 60.0), (96.0, 60.0));
        assert_eq!(r.commit.as_deref(), Some("Molding Line"));
        assert_eq!(layer(&cx).moldings.len(), 2);
        // A polyline takes any number of corners and a double-click.
        let mut p = tool(DetailsVariant::MoldingPolyline);
        click(&mut p, &mut cx, 0.0, 120.0);
        click(&mut p, &mut cx, 96.0, 120.0);
        click(&mut p, &mut cx, 96.0, 216.0);
        let r = dbl(&mut p, &mut cx, 96.0, 216.0);
        assert_eq!(r.commit.as_deref(), Some("Molding Polyline"));
        assert_eq!(layer(&cx).moldings[2].polyline.len(), 3);
        // Crown by default, at the top of the wall.
        assert!(layer(&cx).moldings[2].elevation > 90.0);
        assert_eq!(cx.undo().as_deref(), Some("Molding Polyline"));
        // Enter finishes too; two points are needed.
        click(&mut p, &mut cx, 0.0, 300.0);
        assert!(p.key(&mut cx, KeyEvent::key(Key::Enter)).commit.is_none());
        click(&mut p, &mut cx, 50.0, 300.0);
        assert!(p.key(&mut cx, KeyEvent::key(Key::Enter)).commit.is_some());
    }

    #[test]
    fn floor_material_region_and_deck_are_polygons() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::FloorMaterialRegion);
        let r = triangle(&mut t, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("Floor Material Region"));
        let l = layer(&cx);
        assert_eq!(l.regions.len(), 1);
        assert!(l.regions[0].is_floor());
        assert_eq!(l.regions[0].outline.len(), 3);
        assert_eq!(l.regions[0].thickness, 0.25);
        // Dragging draws a rectangle.
        let r = drag(&mut t, &mut cx, (0.0, 0.0), (96.0, 48.0));
        assert_eq!(r.commit.as_deref(), Some("Floor Material Region"));
        assert_eq!(layer(&cx).regions[1].outline.len(), 4);
        assert_eq!(layer(&cx).regions[1].area(), 96.0 * 48.0);
        let mut d = tool(DetailsVariant::PolygonDeck);
        let r = triangle(&mut d, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("Polygon Shaped Deck"));
        let deck = layer(&cx).decks[0].clone();
        assert_eq!(deck.outline.len(), 3);
        assert_eq!(deck.board_thickness, 1.5);
        assert!(!deck.railing);
        assert_eq!(cx.undo().as_deref(), Some("Polygon Shaped Deck"));
        assert!(layer(&cx).decks.is_empty());
    }

    #[test]
    fn a_polygon_needs_three_corners_and_backspace_pops() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::PolygonDeck);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 100.0, 0.0);
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(r.commit.is_none() && r.consumed);
        assert!(cx.status.contains("at least 3"));
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
    fn wall_material_region_takes_a_range_or_the_whole_wall_and_opens_its_dialog() {
        let mut cx = cx();
        box_walls(&mut cx);
        let wall = cx.floor().walls[0].id; // the south wall along +x
        let mut t = tool(DetailsVariant::WallMaterialRegion);
        // Drag along the outside of the wall: part of its length, right face.
        let r = drag(&mut t, &mut cx, (48.0, -2.0), (144.0, -2.0));
        assert_eq!(r.commit.as_deref(), Some("Wall Material Region"));
        let reg = layer(&cx).regions[0].clone();
        assert_eq!(reg.wall_id(), Some(wall));
        let (u0, u1, v0, v1) = reg.uv_bounds().unwrap();
        assert!(
            (u0 - 48.0).abs() < 1.0 && (u1 - 144.0).abs() < 1.0,
            "{u0} {u1}"
        );
        assert_eq!((v0, v1), (0.0, 108.0));
        assert_eq!(reg.side, plan_core::walls::Side::Right);
        assert!(t.dialog_open(), "the dialog sets the heights");
        *t.dialog.borrow_mut() = None;
        // A plain click covers the whole wall, on the inside (left).
        let r = drag(&mut t, &mut cx, (120.0, 2.0), (120.0, 2.0));
        assert!(r.commit.is_some());
        let whole = layer(&cx).regions[1].clone();
        assert_eq!(whole.uv_bounds(), Some((0.0, 240.0, 0.0, 108.0)));
        assert_eq!(whole.side, plan_core::walls::Side::Left);
        // Away from every wall nothing happens.
        *t.dialog.borrow_mut() = None;
        let r = click(&mut t, &mut cx, 120.0, 90.0);
        assert!(r.commit.is_none());
        assert_eq!(layer(&cx).regions.len(), 2);
    }

    #[test]
    fn wall_hatching_adds_one_hatch_per_wall_and_opens_the_dialog() {
        let mut cx = cx();
        box_walls(&mut cx);
        let mut t = tool(DetailsVariant::WallHatching);
        let r = click(&mut t, &mut cx, 120.0, 2.0);
        assert_eq!(r.commit.as_deref(), Some("Wall Hatching"));
        assert_eq!(layer(&cx).hatches.len(), 1);
        assert!(t.dialog_open());
        *t.dialog.borrow_mut() = None;
        // Clicking the same wall again edits the hatch it has.
        let r = click(&mut t, &mut cx, 60.0, 2.0);
        assert!(r.commit.is_none());
        assert_eq!(layer(&cx).hatches.len(), 1);
        assert!(t.dialog_open());
        *t.dialog.borrow_mut() = None;
        let r = click(&mut t, &mut cx, 120.0, 90.0);
        assert!(r.commit.is_none());
        assert!(cx.status.contains("wall"));
    }

    #[test]
    fn round_solids_take_a_center_and_a_radius_and_open_the_dialog() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::Sphere);
        click(&mut t, &mut cx, 100.0, 100.0);
        assert_eq!(t.points().len(), 1);
        // A stray second click on the center does nothing.
        click(&mut t, &mut cx, 100.0, 100.0);
        assert_eq!(t.points().len(), 1);
        let r = click(&mut t, &mut cx, 136.0, 100.0);
        assert_eq!(r.commit.as_deref(), Some("Sphere"));
        let s = layer(&cx).solids[0].clone();
        assert_eq!(s.position, Point::new(100.0, 100.0));
        assert!(matches!(s.kind, SolidKind::Sphere { r } if (r - 36.0).abs() < 1e-6));
        assert!(t.dialog_open());
        *t.dialog.borrow_mut() = None;
        // Dragging from the center sets the radius too.
        let mut c = tool(DetailsVariant::Cylinder);
        let r = drag(&mut c, &mut cx, (300.0, 100.0), (312.0, 100.0));
        assert_eq!(r.commit.as_deref(), Some("Cylinder"));
        assert!(
            matches!(layer(&cx).solids[1].kind, SolidKind::Cylinder { r, h } if (r - 12.0).abs() < 1e-6 && h == DEFAULT_SOLID_HEIGHT)
        );
        *c.dialog.borrow_mut() = None;
        let mut k = tool(DetailsVariant::Cone);
        click(&mut k, &mut cx, 500.0, 100.0);
        click(&mut k, &mut cx, 524.0, 100.0);
        assert!(matches!(layer(&cx).solids[2].kind, SolidKind::Cone { .. }));
        assert_eq!(cx.undo().as_deref(), Some("Cone"));
    }

    #[test]
    fn polygon_solids_are_prisms_pyramids_faces_and_dragged_boxes() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::Solid3d);
        let r = triangle(&mut t, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("3D Solid"));
        let s = layer(&cx).solids[0].clone();
        match &s.kind {
            SolidKind::PolylineSolid { outline, h } => {
                assert_eq!(outline.len(), 3);
                assert_eq!(*h, DEFAULT_SOLID_HEIGHT);
                // The outline is relative to the position, so the footprint
                // lands back on the clicked corners.
                let fp = s.footprint();
                assert!(fp.iter().any(|p| p.dist(Point::new(720.0, 120.0)) < 1e-6));
            }
            k => panic!("{k:?}"),
        }
        assert!(t.dialog_open());
        *t.dialog.borrow_mut() = None;
        // A dragged rectangle is a box.
        let r = drag(&mut t, &mut cx, (0.0, 0.0), (48.0, 24.0));
        assert_eq!(r.commit.as_deref(), Some("3D Solid"));
        assert!(
            matches!(layer(&cx).solids[1].kind, SolidKind::Box { w, d, .. } if w == 48.0 && d == 24.0)
        );
        assert_eq!(layer(&cx).solids[1].position, Point::new(24.0, 12.0));
        *t.dialog.borrow_mut() = None;
        let mut p = tool(DetailsVariant::Pyramid);
        triangle(&mut p, &mut cx);
        assert!(matches!(
            layer(&cx).solids[2].kind,
            SolidKind::Pyramid { .. }
        ));
        *p.dialog.borrow_mut() = None;
        let mut f = tool(DetailsVariant::Face);
        triangle(&mut f, &mut cx);
        assert!(matches!(layer(&cx).solids[3].kind, SolidKind::Face { .. }));
    }

    #[test]
    fn slab_footing_makes_a_slab_with_a_footing() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::SlabFooting);
        let r = triangle(&mut t, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("Slab Footing"));
        let f = fv::load(&cx);
        assert_eq!(f.slabs.len(), 1);
        assert!(f.slabs[0].footing.is_some());
        assert!(layer(&cx).is_empty(), "it is a foundation object");
        assert_eq!(cx.undo().as_deref(), Some("Slab with Footing"));
    }

    #[test]
    fn ctrl_drag_moves_and_delete_removes_with_undo() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::PolygonDeck);
        triangle(&mut t, &mut cx);
        let id = layer(&cx).decks[0].id;
        let ctrl = Modifiers::COMMAND;
        let down = PointerEvent::at(&cx, Point::new(600.0, 30.0))
            .with_down(true)
            .with_modifiers(ctrl);
        t.pointer_down(&mut cx, down);
        for x in [620.0, 660.0] {
            let m = PointerEvent::at(&cx, Point::new(x, 70.0))
                .with_down(true)
                .with_modifiers(ctrl);
            t.pointer_move(&mut cx, m);
        }
        let up = PointerEvent::at(&cx, Point::new(660.0, 70.0)).with_modifiers(ctrl);
        let r = t.pointer_up(&mut cx, up);
        assert_eq!(r.commit.as_deref(), Some("Move Deck"));
        assert_eq!(
            layer(&cx).deck(id).unwrap().outline[0],
            Point::new(540.0, 40.0)
        );
        assert_eq!(cx.undo().as_deref(), Some("Move Deck"));
        assert_eq!(
            layer(&cx).deck(id).unwrap().outline[0],
            Point::new(480.0, 0.0)
        );
        // A Ctrl-click that does not move leaves no undo step.
        let before = cx.undo_label().map(str::to_string);
        let down = PointerEvent::at(&cx, Point::new(600.0, 30.0)).with_modifiers(ctrl);
        t.pointer_down(&mut cx, down);
        t.pointer_up(&mut cx, down);
        assert_eq!(cx.undo_label().map(str::to_string), before);
        // Delete removes the selected object.
        assert_eq!(dv::selected(), Some(DetailRef::Deck(id)));
        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete Deck"));
        assert!(layer(&cx).is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Delete Deck"));
        assert_eq!(layer(&cx).decks.len(), 1);
    }

    #[test]
    fn a_double_click_opens_the_dialog_and_ok_applies_with_undo() {
        let mut cx = cx();
        let mut t = tool(DetailsVariant::PolygonDeck);
        triangle(&mut t, &mut cx);
        let id = layer(&cx).decks[0].id;
        // The press of the double-click starts a stray corner; the
        // double-click then opens the dialog instead of a shape.
        click(&mut t, &mut cx, 700.0, 20.0);
        dbl(&mut t, &mut cx, 700.0, 20.0);
        assert!(t.dialog_open());
        assert!(t.points().is_empty());
        let mut d = t.dialog.borrow_mut().take().unwrap();
        if let Draft::Deck(deck) = d.draft_mut() {
            deck.elevation = 30.0;
            deck.railing = true;
        }
        *t.applied.borrow_mut() = Some(d.draft().clone());
        let r = t.flush(&mut cx).expect("an applied draft");
        assert_eq!(r.commit.as_deref(), Some("Deck Specification"));
        let deck = layer(&cx).deck(id).unwrap().clone();
        assert_eq!((deck.elevation, deck.railing), (30.0, true));
        assert_eq!(cx.undo().as_deref(), Some("Deck Specification"));
        assert!(!layer(&cx).deck(id).unwrap().railing);
        // A double-click on empty ground opens nothing.
        dbl(&mut t, &mut cx, 5000.0, 5000.0);
        assert!(!t.dialog_open());
    }

    #[test]
    fn every_listed_flyout_entry_is_live_and_selects_its_variant() {
        let mut stubs = Vec::new();
        let mut live = 0;
        for f in [
            toolbar::trim(),
            toolbar::solid_3d(),
            toolbar::straight_wall(),
            toolbar::railing_deck(),
            toolbar::floor(),
        ] {
            for e in &f.entries {
                if let Action::NotImplemented(name) = e.action {
                    stubs.push(name);
                }
                if let Some(v) = DetailsVariant::ALL.iter().find(|v| v.name() == e.name) {
                    assert_eq!(
                        e.action,
                        Action::SetTool(ToolId::DetailsVariant(*v)),
                        "{}",
                        e.name
                    );
                    live += 1;
                }
            }
        }
        assert_eq!(live, DetailsVariant::ALL.len());
        for v in DetailsVariant::ALL {
            assert!(!stubs.contains(&v.name()), "{} is still a stub", v.name());
        }
        // The one flyout entry left is the 3D Solid Feature.
        assert!(stubs.contains(&"3D Solid Feature"));
    }

    #[test]
    fn the_tool_set_selects_each_variant_by_id() {
        let mut cx = cx();
        let mut set = crate::tools::ToolSet::new();
        for v in DetailsVariant::ALL {
            let id = ToolId::DetailsVariant(v);
            set.set_active(&mut cx, id);
            assert_eq!(set.active().name(), v.name());
            assert_eq!(set.active_id(), id);
            assert!(set.active().hint().starts_with(v.name()));
        }
    }
}
