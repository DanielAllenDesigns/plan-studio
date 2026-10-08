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
//! * Esc cancels the wall being drawn (W-8);
//! * the flyout variants (foundation, pony, glass, glass pony, half-wall,
//!   room divider, railing, deck railing, deck edge, fencing) draw the same
//!   way and differ only in the wall they create ([`WallVariant::spec`]);
//!   the curved variants take a third click for the arc.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::connect;
use crate::editor::ops::{make_wall, JOIN_TOL};
use crate::editor::snap::SnapKind;
use crate::editor::{render, Camera, EditorContext, ObjectRef, SnapResult};
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align2, FontId, Pos2, Shape, Stroke, Vec2};
use plan_core::geometry::Point;
use plan_core::walls::MIN_WALL_THICKNESS;
use plan_core::{
    detect_rooms, FenceStyle, Id, Layer, PlanDefaults, WallClass, WallCurve, WallKind,
};

/// Pixels the pointer must travel between press and release for a drag-draw.
const DRAG_PX: f32 = 4.0;
/// Walls shorter than this are not created.
const MIN_LENGTH: f64 = 1.0;

/// Which wall the tool draws (the flyout entry).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WallStyle {
    Exterior,
    Interior,
    Foundation,
    Pony,
    Glass,
    GlassPony,
    Half,
    RoomDivider,
    Railing,
    DeckRailing,
    DeckEdge,
    Fencing,
}

/// A wall flyout entry: a style, straight or curved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WallVariant {
    pub style: WallStyle,
    pub curved: bool,
}

/// Everything a new wall of a variant is created with.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawSpec {
    pub kind: WallKind,
    pub class: WallClass,
    /// The wall type of the new wall.
    pub wall_type: String,
    /// Wall types to register in the plan with the wall.
    pub types: Vec<String>,
    pub thickness: f64,
    pub height: f64,
    pub foundation_height: f64,
}

impl WallVariant {
    pub const fn straight(style: WallStyle) -> Self {
        Self {
            style,
            curved: false,
        }
    }

    pub const fn curved(style: WallStyle) -> Self {
        Self {
            style,
            curved: true,
        }
    }

    pub fn from_kind(kind: WallKind) -> Self {
        Self::straight(match kind {
            WallKind::Exterior => WallStyle::Exterior,
            WallKind::Interior => WallStyle::Interior,
        })
    }

    /// Chief's name for the flyout entry.
    pub fn name(self) -> &'static str {
        use WallStyle::*;
        match (self.style, self.curved) {
            (Exterior, false) => "Straight Exterior Wall",
            (Exterior, true) => "Curved Exterior Wall",
            (Interior, false) => "Straight Interior Wall",
            (Interior, true) => "Curved Interior Wall",
            (Foundation, false) => "Straight Foundation Wall",
            (Foundation, true) => "Curved Foundation Wall",
            (Pony, false) => "Straight Pony Wall",
            (Pony, true) => "Curved Pony Wall",
            (Glass, _) => "Straight Glass Wall",
            (GlassPony, _) => "Straight Glass Pony Wall",
            (Half, false) => "Straight Half-Wall",
            (Half, true) => "Curved Half-Wall",
            (RoomDivider, _) => "Room Divider",
            (Railing, false) => "Straight Railing",
            (Railing, true) => "Curved Railing",
            (DeckRailing, false) => "Straight Deck Railing",
            (DeckRailing, true) => "Curved Deck Railing",
            (DeckEdge, false) => "Straight Deck Edge",
            (DeckEdge, true) => "Curved Deck Edge",
            (Fencing, false) => "Straight Fencing",
            (Fencing, true) => "Curved Fencing",
        }
    }

    /// The wall this variant draws, from the plan defaults.
    pub fn spec(self, d: &PlanDefaults) -> DrawSpec {
        use WallStyle::*;
        let v = &d.wall_variants;
        let thick =
            |name: &str, fallback: f64| d.wall_type(name).map_or(fallback, |t| t.thickness());
        let plain = |kind: WallKind, class: WallClass, wall_type: &str, height: f64| DrawSpec {
            kind,
            class,
            thickness: thick(
                wall_type,
                match kind {
                    WallKind::Exterior => d.exterior_thickness(),
                    WallKind::Interior => d.interior_thickness(),
                },
            ),
            types: vec![wall_type.to_string()],
            wall_type: wall_type.to_string(),
            height,
            foundation_height: d.foundation_wall.height,
        };
        let ext = &d.exterior_wall;
        let int = &d.interior_wall;
        match self.style {
            Exterior => plain(
                WallKind::Exterior,
                WallClass::Standard,
                &ext.wall_type,
                ext.height,
            ),
            Interior => plain(
                WallKind::Interior,
                WallClass::Standard,
                &int.wall_type,
                int.height,
            ),
            Foundation => plain(
                WallKind::Exterior,
                WallClass::Foundation,
                &d.foundation_wall.wall_type,
                d.foundation_wall.height,
            ),
            Pony => {
                let mut s = plain(
                    WallKind::Exterior,
                    WallClass::Pony {
                        upper_type: v.pony_upper_type.clone(),
                        lower_type: v.pony_lower_type.clone(),
                        split_height: v.pony_split_height,
                        upper_sets_plan_display: false,
                    },
                    &v.pony_upper_type,
                    ext.height,
                );
                s.types.push(v.pony_lower_type.clone());
                s
            }
            Glass => plain(
                WallKind::Exterior,
                WallClass::Glass,
                &v.glass_type,
                ext.height,
            ),
            GlassPony => {
                let mut s = plain(
                    WallKind::Exterior,
                    WallClass::GlassPony {
                        lower_type: v.pony_lower_type.clone(),
                        split_height: v.pony_split_height,
                    },
                    &v.glass_type,
                    ext.height,
                );
                s.types.push(v.pony_lower_type.clone());
                s
            }
            Half => plain(
                WallKind::Interior,
                WallClass::HalfWall {
                    height: v.half_wall_height,
                },
                &int.wall_type,
                v.half_wall_height,
            ),
            RoomDivider => DrawSpec {
                thickness: MIN_WALL_THICKNESS,
                types: Vec::new(),
                wall_type: String::new(),
                ..plain(WallKind::Interior, WallClass::RoomDivider, "", int.height)
            },
            Railing => plain(
                WallKind::Interior,
                WallClass::Railing,
                &v.railing_type,
                v.railing_height,
            ),
            DeckRailing => plain(
                WallKind::Exterior,
                WallClass::DeckRailing,
                &v.deck_railing_type,
                v.railing_height,
            ),
            DeckEdge => plain(
                WallKind::Exterior,
                WallClass::DeckEdge,
                &v.deck_edge_type,
                v.deck_edge_height,
            ),
            Fencing => plain(
                WallKind::Exterior,
                WallClass::Fencing {
                    style: FenceStyle::Picket,
                },
                &v.fencing_type,
                v.fencing_height,
            ),
        }
    }

    fn tool_id(self) -> ToolId {
        match (self.style, self.curved) {
            (WallStyle::Exterior, false) => ToolId::Wall {
                kind: WallKind::Exterior,
            },
            (WallStyle::Interior, false) => ToolId::Wall {
                kind: WallKind::Interior,
            },
            _ => ToolId::WallVariant(self),
        }
    }
}

/// The layer new walls of `class` go on, with its plan color and weight.
fn class_layer(class: &WallClass) -> Option<Layer> {
    let name = class.default_layer()?;
    Some(match class {
        WallClass::RoomDivider => Layer::new(name, [150, 150, 150], 13),
        WallClass::Fencing { .. } => Layer::new(name, [90, 130, 60], 18),
        _ => Layer::new(name, [150, 100, 50], 25),
    })
}

/// The arc of a curved wall waiting for its third click.
#[derive(Clone, Copy)]
struct PendingArc {
    start: Point,
    end: Point,
    /// A press-drag-release wall: the chain ends with it.
    single: bool,
    /// The end closes the loop.
    closing: bool,
}

struct Press {
    screen: Pos2,
    /// The press placed the first point of a new chain.
    started_chain: bool,
}

pub struct WallTool {
    variant: WallVariant,
    /// A curved wall's chord is set; the next click sets the bulge.
    arc: Option<PendingArc>,
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
            variant: WallVariant::from_kind(WallKind::Exterior),
            arc: None,
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
        self.arc = None;
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
        curve: Option<WallCurve>,
    ) -> Option<(Id, Point, bool)> {
        if start.dist(end) < MIN_LENGTH {
            return None;
        }
        cx.begin_change("Draw Wall");
        let fl = cx.floor;
        let spec = self.variant.spec(&cx.defaults);
        let rooms_before = detect_rooms(&cx.project.floors[fl].walls, 0.5).len();
        let id = cx
            .project
            .add_wall(fl, start, end, spec.thickness, spec.height, spec.kind);
        // New walls take the default wall type of their variant (W-51).
        for name in &spec.types {
            if let Some(def) = cx.defaults.wall_type(name).cloned() {
                if cx.project.wall_type_def(name).is_none() {
                    cx.project.register_wall_type(def);
                }
            }
        }
        let typed = cx.defaults.wall_type(&spec.wall_type).is_some();
        if let Some(layer) = class_layer(&spec.class) {
            cx.project.layers.add(layer);
        }
        if let Some(w) = cx.project.floors[fl].wall_mut(id) {
            if typed {
                w.wall_type = Some(spec.wall_type.clone());
            }
            w.curve = curve;
            w.foundation_height = spec.foundation_height;
            if let Some(layer) = spec.class.default_layer() {
                w.layer = layer.to_string();
            }
            if !spec.class.is_standard() {
                w.set_class(spec.class.clone());
            }
        }
        let reach = connect::MIN_CONNECT_DISTANCE.max(spec.thickness);
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

    /// The bulge for an arc whose chord is set, with the pointer at `world`:
    /// its distance from the chord, positive on the left, on the grid.
    fn arc_bulge(&self, cx: &EditorContext, arc: PendingArc, world: Point) -> f64 {
        let normal = (arc.end - arc.start).normalized().perp();
        let mid = Point::lerp(arc.start, arc.end, 0.5);
        let unit = cx.snap_unit();
        (((world - mid).dot(normal)) / unit).round() * unit
    }

    /// The chord of a curved wall is set: wait for the click that sets the arc.
    fn start_arc(
        &mut self,
        cx: &mut EditorContext,
        start: Point,
        end: Point,
        single: bool,
        closing: bool,
    ) -> ToolResult {
        if start.dist(end) < MIN_LENGTH {
            return ToolResult::consumed();
        }
        self.arc = Some(PendingArc {
            start,
            end,
            single,
            closing,
        });
        self.hover = Some(end);
        cx.readout = Some("Click to set the curve".into());
        ToolResult::consumed()
    }

    /// The click that sets a curved wall's bulge creates the wall.
    fn finish_arc(&mut self, cx: &mut EditorContext, arc: PendingArc, world: Point) -> ToolResult {
        self.arc = None;
        let bulge = self.arc_bulge(cx, arc, world);
        let curve = (bulge.abs() >= 0.5).then_some(WallCurve { bulge });
        let made = self.create(cx, arc.start, arc.end, curve);
        if arc.single {
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
        let Some((_, next, room)) = made else {
            return ToolResult::consumed();
        };
        self.advance_chain(cx, arc.start, arc.end, next, room, arc.closing, None);
        ToolResult::committed("Draw Wall")
    }

    /// Continues (or closes) the chain after a wall was added.
    #[allow(clippy::too_many_arguments)]
    fn advance_chain(
        &mut self,
        cx: &mut EditorContext,
        start: Point,
        end: Point,
        next: Point,
        room: bool,
        closing: bool,
        last_dir: Option<Point>,
    ) {
        self.chain_walls += 1;
        self.last_dir = last_dir.or(Some((end - start).normalized()));
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
    }
}

impl Tool for WallTool {
    fn id(&self) -> ToolId {
        self.variant.tool_id()
    }

    fn name(&self) -> &'static str {
        self.variant.name()
    }

    fn hint(&self) -> String {
        "Wall: click to place points; Alt disables angle snap; Esc/right-click ends".into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        match id {
            ToolId::Wall { kind } => self.variant = WallVariant::from_kind(kind),
            ToolId::WallVariant(v) => self.variant = v,
            _ => {}
        }
        // A pending arc belongs to the variant that drew its chord.
        self.arc = None;
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.end_chain();
        self.press = None;
        self.hover = None;
        cx.readout = None;
        cx.last_snap = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(arc) = self.arc {
            self.hover = Some(p.world);
            let bulge = self.arc_bulge(cx, arc, p.world);
            cx.readout = Some(format!("Bulge: {}", cx.fmt_dim(bulge.abs())));
            return ToolResult {
                repaint: true,
                ..ToolResult::default()
            };
        }
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
        if let Some(arc) = self.arc {
            return self.finish_arc(cx, arc, p.world);
        }
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
            if self.variant.curved {
                return self.start_arc(cx, start, end, true, false);
            }
            let made = self.create(cx, start, end, None);
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
        if self.variant.curved {
            return self.start_arc(cx, start, end, false, closing);
        }
        let Some((_, next, room)) = self.create(cx, start, end, None) else {
            return ToolResult::consumed();
        };
        self.advance_chain(cx, start, end, next, room, closing, None);
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
        let spec = self.variant.spec(&cx.defaults);
        if let Some(arc) = self.arc {
            let mut ghost = make_wall(
                0,
                arc.start,
                arc.end,
                spec.thickness,
                spec.height,
                spec.kind,
            );
            let bulge = self.arc_bulge(cx, arc, to);
            ghost.curve = Some(WallCurve { bulge });
            let pts = ghost
                .plan_polygon()
                .iter()
                .map(|p| cam.world_to_screen(*p))
                .collect();
            // The band of an arc is not convex: outline only.
            painter.add(Shape::closed_line(
                pts,
                Stroke::new(1.5_f32, pal.ghost_stroke),
            ));
            return;
        }
        if let Some(start) = self.pending {
            let len = start.dist(to);
            if len > 0.01 {
                let ghost = make_wall(0, start, to, spec.thickness, spec.height, spec.kind);
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

    fn variant_tool(style: WallStyle, curved: bool) -> WallTool {
        let mut t = WallTool::default();
        t.set_variant(ToolId::WallVariant(WallVariant { style, curved }));
        t
    }

    #[test]
    fn every_variant_draws_its_wall_with_the_right_class_type_and_layer() {
        use WallStyle::*;
        let cases: [(WallStyle, &str, Option<&str>, f64); 10] = [
            (Foundation, "Foundation", None, 48.0),
            (Pony, "Pony Wall", None, 109.125),
            (Glass, "Glass Wall", None, 109.125),
            (GlassPony, "Glass Pony Wall", None, 109.125),
            (Half, "Half-Wall", None, 36.0),
            (
                RoomDivider,
                "Room Divider",
                Some("Walls, Invisible"),
                109.125,
            ),
            (Railing, "Railing", None, 36.0),
            (DeckRailing, "Deck Railing", Some("Deck Railing"), 36.0),
            (DeckEdge, "Deck Edge", Some("Deck Railing"), 9.25),
            (Fencing, "Fencing", Some("Fencing"), 72.0),
        ];
        for (style, label, layer, height) in cases {
            let mut cx = new_cx();
            let mut t = variant_tool(style, false);
            drag(&mut t, &mut cx, (0.0, 0.0), (120.0, 0.0));
            let walls = &cx.floor().walls;
            assert_eq!(walls.len(), 1, "{label}");
            let w = &walls[0];
            assert_eq!(w.class.label(), label);
            assert_eq!(w.layer, layer.unwrap_or("Walls, Normal"), "{label}");
            assert!((w.height - height).abs() < 1e-9, "{label} {}", w.height);
            assert!(w.curve.is_none());
            assert_eq!(cx.undo().as_deref(), Some("Draw Wall"));
            assert!(cx.floor().walls.is_empty());
        }
    }

    #[test]
    fn variant_defaults_come_from_the_plan_defaults() {
        let d = plan_defaults::embedded();
        let pony = WallVariant::straight(WallStyle::Pony).spec(&d);
        assert_eq!(
            pony.class,
            WallClass::Pony {
                upper_type: d.exterior_wall.wall_type.clone(),
                lower_type: "Foundation-8".into(),
                split_height: 36.0,
                upper_sets_plan_display: false,
            }
        );
        let glass = WallVariant::straight(WallStyle::Glass).spec(&d);
        assert_eq!(
            (glass.wall_type.as_str(), glass.thickness),
            ("Glass-1", 1.0)
        );
        let found = WallVariant::straight(WallStyle::Foundation).spec(&d);
        assert_eq!(
            (found.wall_type.as_str(), found.thickness),
            ("Foundation-8", 8.0)
        );
        let half = WallVariant::straight(WallStyle::Half).spec(&d);
        assert_eq!(half.class, WallClass::HalfWall { height: 36.0 });
        let divider = WallVariant::straight(WallStyle::RoomDivider).spec(&d);
        assert!(divider.thickness < 0.2 && divider.wall_type.is_empty());
    }

    #[test]
    fn the_types_of_a_pony_wall_are_registered_with_the_plan() {
        let mut cx = new_cx();
        let mut t = variant_tool(WallStyle::Pony, false);
        drag(&mut t, &mut cx, (0.0, 0.0), (120.0, 0.0));
        assert!(cx.project.wall_type_def("Stucco-6").is_some());
        assert!(cx.project.wall_type_def("Foundation-8").is_some());
        assert_eq!(cx.floor().walls[0].wall_type.as_deref(), Some("Stucco-6"));
    }

    #[test]
    fn curved_variants_take_a_third_click_for_the_arc() {
        let mut cx = new_cx();
        let mut t = variant_tool(WallStyle::Foundation, true);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 120.0, 0.0);
        assert!(cx.floor().walls.is_empty(), "the chord alone makes no wall");
        // The pointer 30" to the left of the chord's middle sets the bulge.
        click(&mut t, &mut cx, 60.0, 30.0);
        let walls = &cx.floor().walls;
        assert_eq!(walls.len(), 1);
        assert_eq!(walls[0].class, WallClass::Foundation);
        assert_eq!(walls[0].curve, Some(WallCurve { bulge: 30.0 }));
        // The chain continues from the arc's end.
        assert_eq!(t.pending_start(), Some(Point::new(120.0, 0.0)));
        assert_eq!(cx.undo().as_deref(), Some("Draw Wall"));
        assert!(cx.floor().walls.is_empty());
    }

    #[test]
    fn a_dragged_curved_wall_is_one_wall_then_the_chain_ends() {
        let mut cx = new_cx();
        let mut t = variant_tool(WallStyle::Exterior, true);
        drag(&mut t, &mut cx, (0.0, 0.0), (120.0, 0.0));
        click(&mut t, &mut cx, 60.0, -20.0);
        let w = &cx.floor().walls[0];
        assert_eq!(w.curve, Some(WallCurve { bulge: -20.0 }));
        assert!(t.pending_start().is_none());
        // Esc before the third click draws nothing.
        drag(&mut t, &mut cx, (0.0, 100.0), (120.0, 100.0));
        t.key(&mut cx, KeyEvent::escape());
        assert_eq!(cx.floor().walls.len(), 1);
    }

    #[test]
    fn variant_walls_connect_with_other_walls() {
        let mut cx = new_cx();
        let mut t = variant_tool(WallStyle::Glass, false);
        drag(&mut t, &mut cx, (0.0, 0.0), (240.0, 0.0));
        let mut stem = variant_tool(WallStyle::Half, false);
        // A half-wall ending 3" off the glass wall's centerline makes a tee
        // that splits it; both halves stay glass.
        drag(&mut stem, &mut cx, (100.0, 120.0), (100.0, 3.0));
        let walls = &cx.floor().walls;
        assert_eq!(walls.len(), 3);
        assert_eq!(
            walls.iter().filter(|w| w.class == WallClass::Glass).count(),
            2
        );
        let half = walls
            .iter()
            .find(|w| matches!(w.class, WallClass::HalfWall { .. }))
            .unwrap();
        assert_eq!(half.end, Point::new(100.0, 0.0));
    }

    #[test]
    fn room_dividers_close_rooms() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        t.set_variant(ToolId::Wall {
            kind: WallKind::Interior,
        });
        for (x, y) in [(0.0, 0.0), (240.0, 0.0), (240.0, 144.0), (0.0, 144.0)] {
            click(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::escape());
        cx.refresh();
        assert_eq!(cx.rooms.len(), 0, "three walls are not a room");
        let mut d = variant_tool(WallStyle::RoomDivider, false);
        drag(&mut d, &mut cx, (0.0, 144.0), (0.0, 0.0));
        cx.refresh();
        assert_eq!(cx.rooms.len(), 1);
        assert!(cx
            .floor()
            .walls
            .iter()
            .any(|w| w.class == WallClass::RoomDivider));
    }

    #[test]
    fn the_wall_railing_deck_and_fencing_flyouts_are_live() {
        use crate::toolbar::{curved_wall, fencing, railing_deck, straight_wall, Action};
        const PENDING: [&str; 4] = [
            "Polygon Shaped Deck",
            "Slab Footing",
            "Wall Hatching",
            "Wall Material Region",
        ];
        let mut cx = new_cx();
        let mut set = crate::tools::ToolSet::new();
        for f in [straight_wall(), curved_wall(), railing_deck(), fencing()] {
            for e in &f.entries {
                match e.action {
                    Action::NotImplemented(name) => {
                        assert!(PENDING.contains(&name), "{} / {name}", f.group)
                    }
                    Action::SetTool(id) => {
                        set.set_active(&mut cx, id);
                        assert_eq!(set.active_id(), id, "{}", e.name);
                        assert_eq!(set.active().name(), e.name);
                    }
                    other => panic!("{}: {other:?}", e.name),
                }
            }
        }
    }
}
