//! Straight wall tools (W-1..W-20 in `docs/parity/walls.md`).
//!
//! * click, click, ... draws a chain: each click after the first ends a wall
//!   and starts the next at the same point (W-3); a double click, Esc or
//!   choosing another tool ends it, a right click does not;
//! * press-drag-release draws one wall from the press point to the release
//!   point and the chain goes on from the release point (W-4); a later drag
//!   draws a wall of its own, from where it was pressed;
//! * snaps (W-11..W-14), in priority order: the start of the chain's first
//!   wall (clicking it closes the loop and ends the chain, W-5), another
//!   wall's endpoint, intersection, midpoint, perpendicular foot, centerline,
//!   the axes through the chain's first point, alignment with the previous
//!   wall (collinear or perpendicular), the 15 degree angle and the grid;
//!   Ctrl/Cmd suspends every snap (S-74, W-17, DT2) and Shift holds the angle snap
//!   increment (W-18, `editing.angle_snap_deg`) even where angle snaps are off;
//! * dashed guides show when the pointer lines up with a wall end, midpoint or
//!   face corner, or sits on a 45 degree direction from the start, and the
//!   point snaps to them (W-14, `snap::align_to_guides`); Shift holds the
//!   angle instead;
//! * the Spacebar reverses the layers of the walls being drawn, so the
//!   exterior goes to the other side (verify in Chief, DECISIONS);
//! * after the first click, typing digits fills the length (feet-inches), Tab
//!   switches to the angle (degrees counter-clockwise from east) and Enter
//!   draws the wall from the start point at exactly that length and angle
//!   (W-15, W-16); the readout shows both live, and the ghost shows the
//!   corner it makes with the wall before it (and with a wall it ends on)
//!   re-solved as the length and angle change;
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
use crate::editor::snap::{self, Guide, GuideKind, SnapKind};
use crate::editor::typed_input::{angle_deg, TypedField, TypedKey};
use crate::editor::{render, tempdim, Camera, EditorContext, ObjectRef, SnapResult};
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align2, FontId, Pos2, Shape, Stroke, Vec2};
use plan_core::geometry::Point;
use plan_core::walls::MIN_WALL_THICKNESS;
use plan_core::{
    detect_rooms, FenceStyle, Id, Layer, PlanDefaults, Wall, WallClass, WallCurve, WallKind,
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

/// The bulge of the arc through `start`, `end` and `through`: the distance
/// from the chord's middle to the arc's apex, positive on the left of
/// start-to-end. A point on the chord's line gives 0; the sweep is held to
/// what a wall can have (just under 340 degrees).
fn bulge_through(start: Point, end: Point, through: Point) -> f64 {
    let chord = start.dist(end);
    if chord < 1e-9 {
        return 0.0;
    }
    let along = (end - start).normalized();
    let mid = Point::lerp(start, end, 0.5);
    let (x, h) = (
        (through - mid).dot(along),
        (through - mid).dot(along.perp()),
    );
    if h.abs() < 1e-9 {
        return 0.0;
    }
    // The circle through the chord's ends and the point has its center on
    // the chord's bisector, `k` over the chord.
    let half = chord * 0.5;
    let k = (x * x + h * h - half * half) / (2.0 * h);
    let apex = k + half.hypot(k) * h.signum();
    apex.clamp(-5.5 * chord, 5.5 * chord)
}

struct Press {
    screen: Pos2,
    /// The press placed the first point of a new chain.
    started_chain: bool,
    /// Where a drag from this press starts its wall: the pointer snapped as a
    /// first point (nothing pending), so a drag is a wall of its own.
    free_start: Point,
}

/// The state of the chain a snap is measured against.
#[derive(Clone, Copy, Default)]
struct Chain {
    first: Option<Point>,
    walls: usize,
    last_dir: Option<Point>,
}

/// The ghost wall as it joins its neighbours (live while a length and angle
/// are typed or the pointer moves): the ghost's own outline and the outlines
/// of the walls it meets, re-solved with it.
#[derive(Clone, Debug, Default)]
pub struct JoinPreview {
    pub ghost: Vec<Point>,
    pub neighbours: Vec<Vec<Point>>,
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
    /// The pointer's snapped point; typed values replace its length or angle.
    live_to: Option<Point>,
    /// Alignment guides of the pointer (W-14).
    guides: Vec<Guide>,
    /// Spacebar: the walls drawn have their layers reversed.
    flipped: bool,
    /// The ghost joined with its neighbours.
    joined: Option<JoinPreview>,
    /// Where the wall of a press-drag in progress starts.
    drag_from: Option<Point>,
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
            live_to: None,
            guides: Vec::new(),
            flipped: false,
            joined: None,
            drag_from: None,
        }
    }
}

impl WallTool {
    /// The start point of the wall in progress.
    pub fn pending_start(&self) -> Option<Point> {
        self.pending
    }

    #[cfg(test)]
    /// The ghost wall joined with the walls it meets, as last solved.
    pub fn join_preview_now(&self) -> Option<&JoinPreview> {
        self.joined.as_ref()
    }

    #[cfg(test)]
    /// The alignment guides the pointer is on.
    pub fn active_guides(&self) -> &[Guide] {
        &self.guides
    }

    #[cfg(test)]
    /// Whether the Spacebar reversed the layers of the walls being drawn.
    pub fn layers_reversed(&self) -> bool {
        self.flipped
    }

    /// Forgets the chain (it closed, was cancelled or the tool left).
    fn end_chain(&mut self, cx: &mut EditorContext) {
        self.arc = None;
        self.pending = None;
        self.chain_first = None;
        self.chain_walls = 0;
        self.last_dir = None;
        self.live_to = None;
        self.guides.clear();
        self.flipped = false;
        self.joined = None;
        self.drag_from = None;
        cx.typed_input.disarm();
    }

    /// The ghost wall `start`..`end` joined with the walls it meets: the
    /// walls with an end at either point and the wall it ends on (a T). The
    /// outlines are solved with the real join rules, so the corner it makes
    /// with the previous wall shows as it will be built (W-16).
    fn join_preview(&self, cx: &EditorContext, start: Point, end: Point) -> Option<JoinPreview> {
        if self.variant.curved || start.dist(end) < MIN_LENGTH {
            return None;
        }
        let tol = 0.5;
        let spec = self.variant.spec(&cx.defaults);
        let mut walls: Vec<Wall> = Vec::new();
        let mut touching: Vec<Id> = Vec::new();
        for w in &cx.floor().walls {
            let ends = [w.start, w.end];
            let at_end = ends
                .iter()
                .any(|e| e.dist(start) <= tol || e.dist(end) <= tol);
            let on_body = !w.is_curved()
                && [start, end]
                    .iter()
                    .any(|p| plan_core::geometry::dist_to_segment(*p, w.start, w.end) <= tol);
            if at_end || on_body {
                walls.push(w.clone());
                if at_end {
                    touching.push(w.id);
                }
            }
        }
        let mut ghost = make_wall(Id::MAX, start, end, spec.thickness, spec.height, spec.kind);
        if self.flipped {
            ghost.exterior_side = ghost.exterior_side.opposite();
        }
        walls.push(ghost);
        let outlines = plan_core::wall_outlines(&walls, tol);
        let mut out = JoinPreview::default();
        for o in outlines {
            if o.wall_id == Id::MAX {
                out.ghost = o.polygon;
            } else if touching.contains(&o.wall_id) {
                out.neighbours.push(o.polygon);
            }
        }
        (out.ghost.len() >= 3).then_some(out)
    }

    /// The end point the typed length and angle give, when anything was typed.
    fn typed_end(&self, cx: &EditorContext) -> Option<Point> {
        let start = self.pending?;
        if !cx.typed_input.has_text() {
            return None;
        }
        let toward = self.live_to.unwrap_or(start + Point::new(1.0, 0.0));
        tempdim::typed_point(cx, start, toward)
    }

    /// Redraws the ghost and the readout after the typed text changed.
    fn refresh_typed(&mut self, cx: &mut EditorContext) {
        if self.arc.is_some() {
            return;
        }
        let to = self.typed_end(cx).or(self.live_to);
        if let Some(to) = to {
            self.hover = Some(to);
            self.update_readout(cx, to);
            self.joined = self.pending.and_then(|s| self.join_preview(cx, s, to));
        }
    }

    /// Enter with typed text: the wall from the start at the typed length and
    /// angle (W-16).
    fn commit_typed(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(start) = self.pending else {
            return ToolResult::consumed();
        };
        let end = self.typed_end(cx).unwrap_or(start);
        if start.dist(end) < MIN_LENGTH {
            cx.status = "Type a length of at least 1\"".into();
            return ToolResult::consumed();
        }
        cx.typed_input.clear();
        if self.variant.curved {
            return self.start_arc(cx, start, end, false, false);
        }
        let Some((_, next, room)) = self.create(cx, start, end, None) else {
            return ToolResult::consumed();
        };
        self.advance_chain(cx, start, end, next, room, false, None);
        ToolResult::committed("Draw Wall")
    }

    /// The chain a snap is measured against.
    fn chain(&self) -> Chain {
        Chain {
            first: self.chain_first,
            walls: self.chain_walls,
            last_dir: self.last_dir,
        }
    }

    /// The snapped end point for the pointer, and whether it closes the loop
    /// (W-5, W-11..W-14).
    fn snap(&self, cx: &EditorContext, p: &PointerEvent) -> (SnapResult, bool) {
        self.snap_from(cx, p, self.pending, self.chain())
    }

    /// [`WallTool::snap`] measured from `origin` against `chain`: a drag
    /// measures from its press point and starts no chain of its own.
    fn snap_from(
        &self,
        cx: &EditorContext,
        p: &PointerEvent,
        origin: Option<Point>,
        chain: Chain,
    ) -> (SnapResult, bool) {
        let raw = p.world;
        let alt = p.overrides();
        let tol = cx.snap_tol();
        // Closing the loop on the first wall's start point.
        if let (Some(first), false) = (chain.first, alt) {
            if chain.walls >= 2 && raw.dist(first) <= tol {
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
        let base = cx.snap_at(raw, origin, alt, &[]);
        let Some(start) = origin else {
            return (
                self.aligned(cx, base, raw, None, p.modifiers.shift || alt),
                false,
            );
        };
        // Shift restricts the angle to 90 or 45 degrees (W-18, manual p. 193),
        // over the object snaps.
        if p.modifiers.shift && !alt {
            let set = snap::restrictive_angles(&cx.defaults.editing);
            let held = snap::angle_snap_list(start, raw, cx.defaults.grid.snap, &set);
            if let Some(point) = held {
                let r = SnapResult {
                    point,
                    kind: SnapKind::Angle,
                    source: None,
                };
                return (r, false);
            }
        }
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
        if let Some(first) = chain.first.filter(|f| f.dist(start) > JOIN_TOL) {
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
        if let Some(d) = chain.last_dir {
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
        (self.aligned(cx, base, raw, Some(start), false), false)
    }

    /// `base` pulled onto the alignment guides (W-14): the pointer lined up
    /// with a wall end, midpoint or face corner in x or y, or on a 45 degree
    /// direction from `origin`. Object snaps and the Shift angle hold win
    /// (`hold`: Shift or Ctrl/Cmd is down).
    fn aligned(
        &self,
        cx: &EditorContext,
        base: SnapResult,
        raw: Point,
        origin: Option<Point>,
        hold: bool,
    ) -> SnapResult {
        if hold
            || base.kind.is_object_snap()
            || !snap::SnapSettings::from_editing(&cx.defaults.editing).object_snaps
        {
            return base;
        }
        let anchors = snap::alignment_anchors(cx.floor(), cx.layers(), &[]);
        let tol = cx.snap_tol();
        let Some(point) = snap::align_to_guides(&anchors, origin, raw, tol, cx.defaults.grid.snap)
        else {
            return base;
        };
        if origin.is_some_and(|o| point.dist(o) <= JOIN_TOL) {
            return base;
        }
        let aligned = snap::guides_through(&anchors, origin, point, 0.05)
            .iter()
            .any(|g| g.kind == GuideKind::Alignment);
        SnapResult {
            point,
            kind: if aligned {
                SnapKind::Extension
            } else {
                SnapKind::Angle
            },
            source: None,
        }
    }

    /// The guides the point `to` is on (empty with Ctrl/Cmd or Shift held).
    fn guides_at(
        &self,
        cx: &EditorContext,
        from: Option<Point>,
        to: Point,
        p: &PointerEvent,
    ) -> Vec<Guide> {
        if p.overrides() || p.modifiers.shift {
            return Vec::new();
        }
        let anchors = snap::alignment_anchors(cx.floor(), cx.layers(), &[]);
        snap::guides_through(&anchors, from, to, 0.05)
    }

    fn update_readout(&self, cx: &mut EditorContext, to: Point) {
        self.update_readout_from(cx, self.pending, to);
    }

    fn update_readout_from(&self, cx: &mut EditorContext, from: Option<Point>, to: Point) {
        cx.readout = from.map(|s| {
            let ti = &cx.typed_input;
            let (len, ang) = (
                cx.fmt_dim(s.dist(to)),
                format!("{:.1}\u{b0}", angle_deg(s, to)),
            );
            // The field being typed shows what was typed, with a caret.
            let len = match (ti.field(), ti.length_text()) {
                (TypedField::Length, t) if ti.has_text() => format!("{t}|"),
                _ => len,
            };
            let ang = match (ti.field(), ti.angle_text()) {
                (TypedField::Angle, t) if ti.has_text() => format!("{t}|"),
                _ => ang,
            };
            format!("Length: {len}   Angle: {ang}")
        });
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
        // Standard walls go on the active layer of the exterior or interior
        // wall tool (Tools > Layer Settings); classes with a layer of their
        // own (railing, fence, divider...) keep it.
        let tool_layer = cx.project.layers.tool_layer(match spec.kind {
            WallKind::Exterior => "walls_exterior",
            WallKind::Interior => "walls_interior",
        });
        if let Some(w) = cx.project.floors[fl].wall_mut(id) {
            if typed {
                w.wall_type = Some(spec.wall_type.clone());
            }
            w.curve = curve;
            if self.flipped {
                w.exterior_side = w.exterior_side.opposite();
            }
            w.foundation_height = spec.foundation_height;
            match spec.class.default_layer() {
                Some(layer) => w.layer = layer.to_string(),
                None if !tool_layer.is_empty() => w.layer = tool_layer,
                None => {}
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
    /// the arc passes through the pointer (W-64), its apex's distance from the
    /// chord, positive on the left, on the grid.
    fn arc_bulge(&self, cx: &EditorContext, arc: PendingArc, world: Point) -> f64 {
        let unit = cx.snap_unit();
        (bulge_through(arc.start, arc.end, world) / unit).round() * unit
    }

    /// The status line of an arc being set: its radius, arc length and chord.
    fn arc_readout(&self, cx: &EditorContext, arc: PendingArc, bulge: f64) -> String {
        let chord = arc.start.dist(arc.end);
        let curve = WallCurve { bulge };
        match curve.radius(chord) {
            Some(r) => format!(
                "Radius: {}   Arc: {}   Chord: {}",
                cx.fmt_dim(r),
                cx.fmt_dim(curve.arc_length(arc.start, arc.end)),
                cx.fmt_dim(chord)
            ),
            None => format!("Straight   Chord: {}", cx.fmt_dim(chord)),
        }
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
            self.end_chain(cx);
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
            self.end_chain(cx);
            cx.selection.clear();
            cx.readout = None;
            cx.last_snap = None;
            self.hover = None;
        } else {
            self.pending = Some(next);
            self.live_to = Some(next);
            cx.typed_input.clear();
            self.update_readout(cx, next);
        }
    }
}

impl Tool for WallTool {
    fn id(&self) -> ToolId {
        self.variant.tool_id()
    }

    /// After the first click: the wall's start, so Tab or Enter can ask for
    /// the end location (manual p. 196).
    fn coordinate_origin(&self, cx: &EditorContext) -> Option<Point> {
        if cx.typed_input.is_armed() {
            self.pending
        } else {
            None
        }
    }

    fn name(&self) -> &'static str {
        self.variant.name()
    }

    fn hint(&self) -> String {
        "Wall: click to place points; type a length, Tab, an angle, Enter; Shift holds the angle to 90 or 45 degrees; Ctrl/Cmd disables snaps; Tab or Enter with nothing typed asks for coordinates; Spacebar reverses the layers; double-click or Esc ends"
            .into()
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
        self.end_chain(cx);
        self.press = None;
        self.hover = None;
        cx.readout = None;
        cx.last_snap = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(arc) = self.arc {
            self.hover = Some(p.world);
            let bulge = self.arc_bulge(cx, arc, p.world);
            cx.readout = Some(self.arc_readout(cx, arc, bulge));
            return ToolResult {
                repaint: true,
                ..ToolResult::default()
            };
        }
        // A press that moved far enough is a drag: its wall starts where it
        // was pressed (W-4), whatever chain was going.
        let dragging = self
            .press
            .as_ref()
            .filter(|pr| p.down && (p.screen - pr.screen).length() >= DRAG_PX)
            .map(|pr| pr.free_start);
        self.drag_from = dragging.filter(|f| Some(*f) != self.pending);
        let (origin, chain) = match self.drag_from {
            Some(f) => (Some(f), Chain::default()),
            None => (self.pending, self.chain()),
        };
        let (s, _) = self.snap_from(cx, &p, origin, chain);
        self.live_to = Some(s.point);
        let to = self.typed_end(cx).unwrap_or(s.point);
        self.hover = Some(to);
        cx.last_snap = Some(s);
        self.guides = self.guides_at(cx, origin, to, &p);
        self.update_readout_from(cx, origin, to);
        self.joined = origin.and_then(|o| self.join_preview(cx, o, to));
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
            cx.typed_input.arm();
        }
        // A click ends whatever was being typed.
        cx.typed_input.clear();
        // The point a drag from here would start at: the pointer as a first
        // point, nothing pending.
        let free_start = if started_chain {
            s.point
        } else {
            let (f, _) = self.snap_from(cx, &p, None, Chain::default());
            f.point
        };
        self.press = Some(Press {
            screen: p.screen,
            started_chain,
            free_start,
        });
        self.update_readout(cx, s.point);
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let Some(press) = self.press.take() else {
            return ToolResult::ignored();
        };
        self.drag_from = None;
        let dragged = (p.screen - press.screen).length() >= DRAG_PX;
        if dragged {
            // Press-drag-release: one wall from the press point to the
            // release point; the chain goes on from its end (W-4).
            let start = press.free_start;
            let (snap, _) = self.snap_from(cx, &p, Some(start), Chain::default());
            let end = snap.point;
            self.pending = Some(start);
            self.chain_first = Some(start);
            self.chain_walls = 0;
            self.last_dir = None;
            if self.variant.curved {
                return self.start_arc(cx, start, end, false, false);
            }
            let Some((_, next, room)) = self.create(cx, start, end, None) else {
                // Too short: the drag drew nothing and the chain is as before.
                return ToolResult::consumed();
            };
            self.advance_chain(cx, start, end, next, room, false, None);
            return ToolResult::committed("Draw Wall");
        }
        let (snap, closing) = self.snap(cx, &p);
        let end = snap.point;
        let Some(start) = self.pending else {
            return ToolResult::ignored();
        };
        if press.started_chain {
            return ToolResult::consumed();
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

    /// A double click ends the chain; the wall of its first click is drawn
    /// and the second adds none (W-3).
    fn double_click(&mut self, cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        if self.pending.is_none() {
            return ToolResult::ignored();
        }
        self.end_chain(cx);
        self.press = None;
        cx.readout = None;
        ToolResult::consumed()
    }

    /// A right click keeps the chain going (W-3): it only drops what was
    /// typed.
    fn secondary_click(&mut self, cx: &mut EditorContext) -> ToolResult {
        if self.pending.is_some() && self.arc.is_none() {
            cx.typed_input.clear();
            self.refresh_typed(cx);
        }
        ToolResult::consumed()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        // The Spacebar reverses the layers of the walls being drawn.
        if self.pending.is_some() && k.text.as_deref() == Some(" ") && !cx.typed_input.has_text() {
            self.flipped = !self.flipped;
            cx.status = if self.flipped {
                "Layers reversed: the exterior is on the other side (Spacebar flips it back)"
            } else {
                "Layers back to the usual side"
            }
            .into();
            if let Some(to) = self.live_to {
                self.joined = self.pending.and_then(|s| self.join_preview(cx, s, to));
            }
            return ToolResult::consumed();
        }
        if self.pending.is_some() && self.arc.is_none() {
            match cx.typed_input.handle(k.key, k.text.as_deref()) {
                TypedKey::Edited | TypedKey::Cancelled => {
                    self.refresh_typed(cx);
                    return ToolResult::consumed();
                }
                TypedKey::Commit => return self.commit_typed(cx),
                TypedKey::Ignored => {}
            }
            // Backspace edits the typed text, never the wall just drawn.
            if k.is(egui::Key::Backspace) {
                return ToolResult::consumed();
            }
        }
        if k.is(egui::Key::Escape) && self.pending.is_some() {
            self.end_chain(cx);
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
        // Alignment guides (W-14): dashed lines from the wall point or the
        // start to the pointer.
        for g in &self.guides {
            let stroke = match g.kind {
                GuideKind::Alignment => Stroke::new(1.0_f32, pal.ghost_stroke),
                GuideKind::Direction => Stroke::new(1.0_f32, pal.ghost_stroke.gamma_multiply(0.7)),
            };
            let pts = [cam.world_to_screen(g.from), cam.world_to_screen(g.to)];
            painter.extend(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
        }
        if let Some(start) = self.drag_from.or(self.pending) {
            snap::draw_angle_rays(painter, cam, cx, start);
            let len = start.dist(to);
            if len > 0.01 {
                let mut ghost = make_wall(0, start, to, spec.thickness, spec.height, spec.kind);
                if self.flipped {
                    ghost.exterior_side = ghost.exterior_side.opposite();
                }
                // The ghost as it joins its neighbours, and those neighbours
                // re-solved with it.
                match &self.joined {
                    Some(j) => {
                        let pts: Vec<Pos2> =
                            j.ghost.iter().map(|p| cam.world_to_screen(*p)).collect();
                        let stroke = Stroke::new(1.0_f32, pal.ghost_stroke);
                        if pts.len() == 4 {
                            painter.add(Shape::convex_polygon(pts, pal.ghost_fill, stroke));
                        } else {
                            painter.add(Shape::closed_line(pts, stroke));
                        }
                        for n in &j.neighbours {
                            let pts: Vec<Pos2> =
                                n.iter().map(|p| cam.world_to_screen(*p)).collect();
                            painter.add(Shape::closed_line(
                                pts,
                                Stroke::new(1.0_f32, pal.ghost_stroke.gamma_multiply(0.8)),
                            ));
                        }
                    }
                    None => {
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
                    }
                }
                // The exterior face is drawn heavier, so Spacebar's reversal
                // shows.
                let n = ghost
                    .normal()
                    .scale(ghost.thickness * 0.5 * ghost.exterior_side.sign());
                painter.line_segment(
                    [cam.world_to_screen(start + n), cam.world_to_screen(to + n)],
                    Stroke::new(3.0_f32, pal.ghost_stroke),
                );
                if cx.view_flags.contains(&ViewFlag::TemporaryDimensions) {
                    let mid = Point::lerp(start, to, 0.5)
                        .add(ghost.normal().scale(ghost.thickness * 0.5));
                    let ti = &cx.typed_input;
                    let typing = ti.has_text();
                    let len_text = match ti.field() {
                        TypedField::Length if typing => format!("{}|", ti.length_text()),
                        _ => cx.fmt_dim(len),
                    };
                    painter.text(
                        cam.world_to_screen(mid)
                            + Vec2::new(0.0, -8.0) * ghost.normal().y.signum() as f32,
                        Align2::CENTER_CENTER,
                        len_text,
                        FontId::proportional(13.0),
                        pal.dimension_text,
                    );
                    // The angle readout rides beside the start point (W-15).
                    let ang_text = match ti.field() {
                        TypedField::Angle if typing => format!("{}|", ti.angle_text()),
                        _ => format!("{:.1}\u{b0}", angle_deg(start, to)),
                    };
                    painter.text(
                        cam.world_to_screen(start) + Vec2::new(14.0, -14.0),
                        Align2::LEFT_BOTTOM,
                        ang_text,
                        FontId::proportional(12.0),
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
    fn drag_draws_one_wall_and_the_chain_goes_on_from_its_end() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        let a = PointerEvent::at(&cx, Point::new(0.0, 0.0));
        let b = PointerEvent::at(&cx, Point::new(120.0, 0.0));
        t.pointer_down(&mut cx, a.with_down(true));
        t.pointer_move(&mut cx, b.with_down(true));
        t.pointer_up(&mut cx, b);
        assert_eq!(cx.floor().walls.len(), 1);
        // W-4: the next wall starts at the release point; Esc ends the chain.
        assert_eq!(t.pending_start(), Some(Point::new(120.0, 0.0)));
        click(&mut t, &mut cx, 120.0, 96.0);
        assert_eq!(cx.floor().walls.len(), 2);
        assert_eq!(cx.floor().walls[1].start, Point::new(120.0, 0.0));
        t.key(&mut cx, KeyEvent::escape());
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
            ctrl: true,
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
        // The alignment guides would pull these ends onto each other's lines;
        // this test is about the connect distance alone.
        cx.defaults.editing.object_snaps = false;
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
        let end = cx.floor().wall(through).unwrap().end;
        assert!(end.dist(Point::new(100.0, 0.0)) < 1e-9, "{end:?}");
        let stem = walls.iter().find(|w| w.start.y == 120.0).unwrap();
        assert!(
            stem.end.dist(Point::new(100.0, 0.0)) < 1e-9,
            "{:?}",
            stem.end
        );
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
    fn walls_go_on_the_active_layer_of_their_tool() {
        use plan_core::Layer;
        let mut cx = new_cx();
        cx.project
            .layers
            .add(Layer::new("Walls, Exterior", [0, 0, 0], 50));
        cx.project
            .layers
            .add(Layer::new("Walls, Partitions", [0, 0, 0], 25));
        assert!(cx
            .project
            .layers
            .set_tool_layer("walls_exterior", "Walls, Exterior"));
        assert!(cx
            .project
            .layers
            .set_tool_layer("walls_interior", "Walls, Partitions"));
        for (style, want) in [
            (WallStyle::Exterior, "Walls, Exterior"),
            (WallStyle::Interior, "Walls, Partitions"),
        ] {
            let mut t = variant_tool(style, false);
            let before = cx.floor().walls.len();
            drag(
                &mut t,
                &mut cx,
                (0.0, 1000.0 * before as f64),
                (120.0, 1000.0 * before as f64),
            );
            assert_eq!(cx.floor().walls.last().unwrap().layer, want, "{style:?}");
        }
        // A class with a layer of its own keeps it, and a tool back on its
        // default layer draws on "Walls, Normal".
        let mut t = variant_tool(WallStyle::Fencing, false);
        drag(&mut t, &mut cx, (0.0, 5000.0), (120.0, 5000.0));
        assert_eq!(cx.floor().walls.last().unwrap().layer, "Fencing");
        assert!(cx
            .project
            .layers
            .set_tool_layer("walls_exterior", "Walls, Normal"));
        let mut t = variant_tool(WallStyle::Exterior, false);
        drag(&mut t, &mut cx, (0.0, 7000.0), (120.0, 7000.0));
        assert_eq!(cx.floor().walls.last().unwrap().layer, "Walls, Normal");
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
    fn the_arc_passes_through_the_third_point() {
        // The apex is where the pointer is when it is over the middle ...
        let (a, b) = (Point::new(0.0, 0.0), Point::new(120.0, 0.0));
        assert!((bulge_through(a, b, Point::new(60.0, 30.0)) - 30.0).abs() < 1e-9);
        assert!((bulge_through(a, b, Point::new(60.0, -20.0)) + 20.0).abs() < 1e-9);
        // ... and off to one side the arc still goes through it.
        let p = Point::new(90.0, 25.0);
        let bulge = bulge_through(a, b, p);
        let w = WallCurve { bulge };
        let (c, r) = w.arc_center_radius(a, b).unwrap();
        assert!((p.dist(c) - r).abs() < 1e-9, "{p:?} is off the arc");
        assert!(
            bulge > 25.0,
            "an off-center point means a deeper arc: {bulge}"
        );
        // On the chord's line it is straight; far outside it is held in range.
        assert_eq!(bulge_through(a, b, Point::new(30.0, 0.0)), 0.0);
        assert!(bulge_through(a, b, Point::new(60.0, 1.0e-3)).abs() <= 5.5 * 120.0);
        // The held bulge is still an arc a wall can have.
        let far = WallCurve {
            bulge: bulge_through(a, b, Point::new(60.0, 1.0e-3)),
        };
        assert!(far.sweep_abs(120.0) < 340.0_f64.to_radians());
    }

    #[test]
    fn the_arc_readout_gives_radius_arc_length_and_chord() {
        let mut cx = new_cx();
        let mut t = variant_tool(WallStyle::Exterior, true);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 240.0, 0.0);
        // A semicircle: 120" radius, 188.5" of arc, the 240" chord.
        let top = PointerEvent::at(&cx, Point::new(120.0, 120.0));
        t.pointer_move(&mut cx, top);
        let text = cx.readout.clone().unwrap();
        assert!(
            text.contains("Radius") && text.contains("Arc") && text.contains("Chord"),
            "{text}"
        );
        assert!(text.contains(&cx.fmt_dim(120.0)), "{text}");
        assert!(
            text.contains(&cx.fmt_dim(std::f64::consts::PI * 120.0)),
            "{text}"
        );
        assert!(text.contains(&cx.fmt_dim(240.0)), "{text}");
        // The pointer on the chord's line reads straight.
        let flat = PointerEvent::at(&cx, Point::new(120.0, 0.0));
        t.pointer_move(&mut cx, flat);
        assert!(cx.readout.clone().unwrap().starts_with("Straight"));
    }

    #[test]
    fn a_dragged_curved_wall_is_one_wall_then_the_chain_goes_on() {
        let mut cx = new_cx();
        let mut t = variant_tool(WallStyle::Exterior, true);
        drag(&mut t, &mut cx, (0.0, 0.0), (120.0, 0.0));
        click(&mut t, &mut cx, 60.0, -20.0);
        let w = &cx.floor().walls[0];
        assert_eq!(w.curve, Some(WallCurve { bulge: -20.0 }));
        // The chain goes on from the arc's end until Esc.
        assert_eq!(t.pending_start(), Some(Point::new(120.0, 0.0)));
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

    fn key_text(t: &mut WallTool, cx: &mut EditorContext, s: &str) {
        t.key(cx, KeyEvent::text(s));
    }

    #[test]
    fn typed_length_and_angle_draw_exactly_that_wall() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        assert!(!cx.typed_input.is_armed());
        click(&mut t, &mut cx, 0.0, 0.0);
        // The shell forwards typed text only while the input is armed.
        assert!(cx.typed_input.is_armed());
        key_text(&mut t, &mut cx, "12'");
        assert!(cx.readout.as_deref().unwrap().contains("12'|"));
        t.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        key_text(&mut t, &mut cx, "90");
        let readout = cx.readout.clone().unwrap();
        assert!(readout.contains("Angle: 90|"), "{readout}");
        let r = t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Draw Wall"));
        let w = &cx.floor().walls[0];
        assert_eq!(
            (w.start, w.end),
            (Point::new(0.0, 0.0), Point::new(0.0, 144.0))
        );
        // The chain goes on from the new end; the typed text is gone.
        assert_eq!(t.pending_start(), Some(Point::new(0.0, 144.0)));
        assert!(!cx.typed_input.has_text() && cx.typed_input.is_armed());
        key_text(&mut t, &mut cx, "6'");
        t.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        key_text(&mut t, &mut cx, "0");
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(cx.floor().walls.len(), 2);
        assert_eq!(cx.floor().walls[1].end, Point::new(72.0, 144.0));
        // Undo takes one wall at a time.
        assert_eq!(cx.undo().as_deref(), Some("Draw Wall"));
        assert_eq!(cx.floor().walls.len(), 1);
    }

    #[test]
    fn typed_length_alone_follows_the_pointer_direction() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        click(&mut t, &mut cx, 0.0, 0.0);
        let p = PointerEvent::at(&cx, Point::new(0.0, 60.0));
        t.pointer_move(&mut cx, p);
        key_text(&mut t, &mut cx, "10'");
        // Moving the pointer keeps the typed length and turns the wall.
        let p = PointerEvent::at(&cx, Point::new(-60.0, 0.0));
        t.pointer_move(&mut cx, p);
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(cx.floor().walls[0].end, Point::new(-120.0, 0.0));
        // Esc drops typed text first, then the chain.
        key_text(&mut t, &mut cx, "5");
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(t.pending_start().is_some() && !cx.typed_input.has_text());
        t.key(&mut cx, KeyEvent::escape());
        assert!(t.pending_start().is_none() && !cx.typed_input.is_armed());
    }

    #[test]
    fn typed_enter_with_nothing_typed_or_too_short_draws_nothing() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        click(&mut t, &mut cx, 0.0, 0.0);
        let r = t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert!(!r.consumed && cx.floor().walls.is_empty());
        key_text(&mut t, &mut cx, "0");
        let r = t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert!(r.consumed && r.commit.is_none() && cx.floor().walls.is_empty());
        assert!(cx.status.contains("at least"));
    }

    #[test]
    fn shift_holds_the_angle_to_90_or_45_even_with_angle_snaps_off() {
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        let draw = |shift_held: bool| {
            let mut cx = new_cx();
            cx.defaults.editing.angle_snaps = false;
            cx.defaults.editing.restrictive_angle_deg = 45.0;
            let mut t = WallTool::default();
            click(&mut t, &mut cx, 0.0, 0.0);
            let mut p = PointerEvent::at(&cx, Point::new(100.0, 60.0));
            if shift_held {
                p = p.with_modifiers(shift);
            }
            t.pointer_down(&mut cx, p.with_down(true));
            t.pointer_up(&mut cx, p);
            cx.floor().walls[0].end
        };
        assert_eq!(draw(false), Point::new(100.0, 60.0));
        let held = draw(true);
        assert!((held.x - held.y).abs() < 1e-9 && held.x > 50.0, "{held:?}");
        // 90 degrees by default: 12 degrees falls to the horizontal.
        let mut cx = new_cx();
        cx.defaults.editing.angle_snaps = false;
        let mut t = WallTool::default();
        click(&mut t, &mut cx, 0.0, 0.0);
        let p = PointerEvent::at(&cx, Point::new(100.0, 22.0)).with_modifiers(shift);
        t.pointer_down(&mut cx, p.with_down(true));
        t.pointer_up(&mut cx, p);
        let e = cx.floor().walls[0].end;
        let deg = angle_deg(Point::ZERO, e);
        assert!(deg.abs() < 1e-6, "{deg}");
    }

    #[test]
    fn ctrl_suspends_every_snap_while_drawing() {
        let alt = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let start_with = |alt_held: bool| {
            let mut cx = new_cx();
            cx.project.add_wall(
                0,
                Point::new(0.0, 0.0),
                Point::new(120.0, 0.0),
                7.625,
                109.0,
                WallKind::Exterior,
            );
            let mut t = WallTool::default();
            let mut p = PointerEvent::at(&cx, Point::new(121.3, 2.2));
            if alt_held {
                p = p.with_modifiers(alt);
            }
            t.pointer_move(&mut cx, p);
            t.pointer_down(&mut cx, p.with_down(true));
            t.pointer_up(&mut cx, p);
            t.pending_start().unwrap()
        };
        assert_eq!(start_with(false), Point::new(120.0, 0.0));
        assert_eq!(start_with(true), Point::new(121.3, 2.2));
    }

    fn existing_wall(cx: &mut EditorContext) {
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            7.625,
            109.0,
            WallKind::Exterior,
        );
    }

    #[test]
    fn dashed_guides_pull_the_pointer_onto_a_wall_end_and_the_45_degree_line() {
        let mut cx = new_cx();
        existing_wall(&mut cx);
        let mut t = WallTool::default();
        // The first point: 3" off the vertical through the wall's end.
        click(&mut t, &mut cx, 243.0, 150.0);
        let start = t.pending_start().unwrap();
        assert!((start.x - 240.0).abs() < 1e-9, "{start:?}");
        // Pointer on the horizontal through that start: snaps to the angle,
        // with a guide when it lines up with the wall's other end.
        let p = PointerEvent::at(&cx, Point::new(-2.0, 152.0));
        t.pointer_move(&mut cx, p);
        let s = cx.last_snap.unwrap();
        assert!((s.point.x).abs() < 1e-9 || s.point.y == 150.0, "{s:?}");
        assert!(!t.active_guides().is_empty(), "a guide is drawn");
        // 45 degrees from the start, 2" off.
        let p = PointerEvent::at(&cx, Point::new(240.0 - 100.0, 150.0 + 102.0));
        t.pointer_move(&mut cx, p);
        let s = cx.last_snap.unwrap();
        let v = s.point - start;
        assert!((v.x.abs() - v.y.abs()).abs() < 1e-6, "{v:?}");
        assert!(t
            .active_guides()
            .iter()
            .any(|g| g.kind == GuideKind::Direction));
        // Shift holds the angle increment instead: no guides.
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        let p = PointerEvent::at(&cx, Point::new(140.0, 252.0)).with_modifiers(shift);
        t.pointer_move(&mut cx, p);
        assert!(t.active_guides().is_empty());
    }

    #[test]
    fn a_guide_through_the_midpoint_of_a_wall_pulls_the_start_onto_its_line() {
        let mut cx = new_cx();
        existing_wall(&mut cx);
        let mut t = WallTool::default();
        click(&mut t, &mut cx, 118.0, 200.0);
        assert!((t.pending_start().unwrap().x - 120.0).abs() < 1e-9);
    }

    #[test]
    fn typing_a_length_and_angle_re_solves_the_join_with_the_previous_wall() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 120.0, 0.0);
        key_text(&mut t, &mut cx, "8'");
        t.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        key_text(&mut t, &mut cx, "90");
        let j = t.join_preview_now().expect("the ghost is joined");
        assert!(j.ghost.len() >= 3);
        // The wall before it is in the preview, mitered with the ghost.
        assert_eq!(j.neighbours.len(), 1);
        let first = j.neighbours[0].clone();
        // Changing the angle re-solves the corner live.
        t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
        t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
        key_text(&mut t, &mut cx, "45");
        let j2 = t.join_preview_now().expect("still joined");
        assert_ne!(j2.neighbours[0], first);
        // Nothing was drawn until Enter.
        assert_eq!(cx.floor().walls.len(), 1);
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(cx.floor().walls.len(), 2);
        let w = &cx.floor().walls[1];
        assert!((w.length() - 96.0).abs() < 1e-6);
        assert!((angle_deg(w.start, w.end) - 45.0).abs() < 1e-6);
    }

    #[test]
    fn the_spacebar_reverses_the_layers_of_the_next_walls_and_esc_ends_the_chain() {
        let mut cx = new_cx();
        let mut t = WallTool::default();
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 120.0, 0.0);
        let plain = cx.floor().walls[0].exterior_side;
        t.key(&mut cx, KeyEvent::text(" "));
        assert!(t.layers_reversed());
        click(&mut t, &mut cx, 120.0, 96.0);
        assert_eq!(cx.floor().walls[1].exterior_side, plain.opposite());
        // The chain goes on from the last end until Esc.
        assert_eq!(t.pending_start(), Some(Point::new(120.0, 96.0)));
        t.key(&mut cx, KeyEvent::escape());
        assert!(t.pending_start().is_none());
        assert!(!t.layers_reversed());
        assert_eq!(cx.floor().walls.len(), 2);
    }
}
