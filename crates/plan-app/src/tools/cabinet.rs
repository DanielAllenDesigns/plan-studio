//! Cabinet tools (CB-1..CB-19 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! Variants: Base, Wall, Full Height, Soffit, Shelf and Partition (CB-1);
//! Base, Wall and Full Height Fillers (CB-19); Corner Base and Wall cabinets
//! (diagonal or pie-cut, set in the specification) and Blind Base and Wall
//! cabinets; and the polygon tools Custom Countertop, Custom Backsplash and
//! Custom Counter Hole (CB-15). Behavior:
//!
//! * a click places a cabinet with its back against the nearest wall within
//!   12", rotated to the wall and flush to its face; away from walls it is
//!   free at the click with the tool's angle (CB-2, CB-3);
//! * a cabinet placed or moved next to another slides to butt against it and
//!   aligns its back line (CB-4); back to back it forms an island, and a
//!   perpendicular run is pushed out of the one it meets (a peninsula);
//! * a filler takes the width of the gap it is clicked into, between a wall
//!   and a cabinet or between two cabinets (CB-19);
//! * a corner cabinet clicked near the inside corner of two walls turns to
//!   the corner and sits in it, legs along both walls;
//! * a blind cabinet turns its hidden end toward the nearest perpendicular
//!   wall;
//! * click-drag sets the width in 3" steps (CB-3, implemented as the width of
//!   one cabinet);
//! * the polygon tools collect corners with clicks (or one drag for a
//!   rectangle); Enter, double-click or clicking the first corner finishes,
//!   Backspace removes the last corner and Esc cancels. A counter hole is cut
//!   out of the countertop it lies in;
//! * G joins the tops of the selected (or all) base cabinets into custom
//!   countertops (Generate Countertop);
//! * a placed cabinet becomes the selection (Shift-click toggles one; the
//!   Select tool picks the rest via `placed::hit_placed`): handles are Move,
//!   Resize width (both ends,
//!   3" steps, the cabinet grows from the dragged side) and Rotate (CB-8,
//!   CB-9); dragging keeps the rotation until it bumps a wall, where it
//!   re-rotates (Ctrl suspends that);
//! * double-click or Enter opens the Cabinet Specification; the Edit toolbar
//!   offers Open Object, Delete, Copy and Reverse Door Swing.
//!
//! The variant comes from `ToolId::CabinetVariant(..)` (flyout entries and
//! hotkeys), or with Tab while the tool is active.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::handles::{self, hit_handle, Handle, HandleKind};
use crate::editor::placed::{
    self, add_cabinet, cabinet_by_id, hit_cabinet, load_cabinets, placed_handles, replace_cabinet,
    same_angle, PlacedRef,
};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Key, Pos2};
use plan_cabinets::{
    fit_between, wall_polygon, BlindSide, Cabinet, CabinetKind, CornerSpec, FaceLayout, HandleStyle,
};
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::{Floor, Id};
use std::f64::consts::FRAC_PI_2;

/// A click within this distance of a wall face places the cabinet on it, in.
pub const WALL_REACH: f64 = 12.0;
/// A corner cabinet snaps into a wall corner this close to the click, in.
pub const CORNER_REACH: f64 = 30.0;
/// Click-drag and resize steps, inches (CB-3, CB-8).
pub const WIDTH_STEP: f64 = 3.0;
/// Smallest cabinet width, inches.
const MIN_WIDTH: f64 = 3.0;
/// Pixels the pointer must travel before a press becomes a drag.
const DRAG_THRESHOLD_PX: f32 = 3.0;
/// Back-to-back and perpendicular snapping reach, inches.
const ISLAND_REACH: f64 = 6.0;
/// A blind cabinet hides its end this close to a perpendicular wall, in.
const BLIND_REACH: f64 = 30.0;

/// The Cabinet flyout, in order; Tab cycles through it. Custom Counter Hole
/// is a tool-only kind that cuts a hole instead of placing a cabinet.
pub const KINDS: [CabinetKind; 16] = [
    CabinetKind::Base,
    CabinetKind::Wall,
    CabinetKind::FullHeight,
    CabinetKind::Soffit,
    CabinetKind::Shelf,
    CabinetKind::Partition,
    CabinetKind::BaseFiller,
    CabinetKind::WallFiller,
    CabinetKind::FullHeightFiller,
    CabinetKind::CornerBase,
    CabinetKind::CornerWall,
    CabinetKind::BlindBase,
    CabinetKind::BlindWall,
    CabinetKind::CustomCountertop,
    CabinetKind::CustomBacksplash,
    CabinetKind::CounterHole,
];

/// Kinds drawn as a polygon or path instead of placed with one click.
pub fn is_polygon(kind: CabinetKind) -> bool {
    matches!(
        kind,
        CabinetKind::CustomCountertop | CabinetKind::CustomBacksplash | CabinetKind::CounterHole
    )
}

pub fn kind_name(kind: CabinetKind) -> &'static str {
    kind.name()
}

fn handle_style(name: &str) -> HandleStyle {
    match name.to_lowercase().as_str() {
        "none" => HandleStyle::None,
        "pull" => HandleStyle::Pull,
        _ => HandleStyle::Knob,
    }
}

/// A new cabinet of `kind` from the plan's cabinet defaults (CB-6, CB-20),
/// at the origin with no id. Fillers take the matching cabinet's depth and
/// height; corner and blind cabinets take their sizes from the defaults.
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
        CabinetKind::BaseFiller | CabinetKind::WallFiller | CabinetKind::FullHeightFiller => {
            let mut c = default_cabinet(
                cx,
                match kind {
                    CabinetKind::BaseFiller => CabinetKind::Base,
                    CabinetKind::WallFiller => CabinetKind::Wall,
                    _ => CabinetKind::FullHeight,
                },
            );
            c.kind = kind;
            c.width = d.filler_width;
            c.framed = false;
            c.face = FaceLayout::filler_panel();
            c
        }
        CabinetKind::CornerBase => {
            let mut c = default_cabinet(cx, CabinetKind::Base);
            c.kind = kind;
            c.corner = Some(CornerSpec {
                arm_depth: c.depth,
                ..CornerSpec::default()
            });
            c.width = d.corner_base_leg;
            c.depth = d.corner_base_leg;
            c
        }
        CabinetKind::CornerWall => {
            let mut c = default_cabinet(cx, CabinetKind::Wall);
            c.kind = kind;
            c.corner = Some(CornerSpec {
                arm_depth: c.depth,
                ..CornerSpec::default()
            });
            c.width = d.corner_wall_leg;
            c.depth = d.corner_wall_leg;
            c
        }
        CabinetKind::BlindBase => {
            let mut c = default_cabinet(cx, CabinetKind::Base);
            c.kind = kind;
            c.width = d.blind_base_width;
            c.blind = Some(plan_cabinets::BlindSpec {
                side: BlindSide::Left,
                blind_width: d.blind_hidden_width,
            });
            c
        }
        CabinetKind::BlindWall => {
            let mut c = default_cabinet(cx, CabinetKind::Wall);
            c.kind = kind;
            c.width = d.blind_base_width * 0.75;
            c.blind = Some(plan_cabinets::BlindSpec {
                side: BlindSide::Left,
                blind_width: d.blind_hidden_width * 0.8,
            });
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

fn visible_walls(cx: &EditorContext) -> impl Iterator<Item = &plan_core::Wall> {
    cx.floor()
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && w.length() > 1e-9 && cx.layers().is_visible(&w.layer))
}

/// The wall whose face is nearest `anchor` within `reach` (CB-3: near a
/// corner, the wall the cursor is closer to).
fn nearest_wall(cx: &EditorContext, anchor: Point, reach: f64) -> Option<WallHit> {
    let mut best: Option<(f64, WallHit)> = None;
    for w in visible_walls(cx) {
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

/// The plan rectangles of the floor's visible walls (fillers measure their
/// gap against them).
fn wall_polys(cx: &EditorContext) -> Vec<Vec<Point>> {
    visible_walls(cx)
        .map(|w| wall_polygon(w.start, w.end, w.thickness))
        .collect()
}

fn vertical_overlap(a: &Cabinet, b: &Cabinet) -> bool {
    a.elevation.max(b.elevation) < (a.elevation + a.height).min(b.elevation + b.height) - 0.5
}

/// Do the two angles differ by a quarter turn?
fn perpendicular(a: f64, b: f64) -> bool {
    same_angle(a - b, FRAC_PI_2) || same_angle(a - b, -FRAC_PI_2)
}

/// Slides `cab` along its width axis to butt against the cabinets it overlaps
/// (same angle, overlapping heights and depths) and aligns its back line with
/// the neighbor's when they are within 6" (CB-4). Failing that, a cabinet back
/// to back with another (angles half a turn apart, backs within 6") joins it
/// as an island, and one that runs into a cabinet at a right angle is pushed
/// out along the shorter way (a peninsula).
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
            o.id != exclude
                && !o.kind.is_custom()
                && same_angle(o.angle, cab.angle)
                && vertical_overlap(o, cab)
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
        return;
    }
    island_and_peninsula(others, cab, exclude, u, v);
}

fn island_and_peninsula(others: &[Cabinet], cab: &mut Cabinet, exclude: Id, u: Point, v: Point) {
    for o in others
        .iter()
        .filter(|o| o.id != exclude && !o.kind.is_custom() && vertical_overlap(o, cab))
    {
        let (s0, s1) = (cab.position.dot(u), cab.position.dot(u) + cab.width);
        if same_angle(o.angle, cab.angle + std::f64::consts::PI) {
            // Facing away: its width runs back along -u and its depth along -v.
            let os1 = o.position.dot(u);
            let os0 = os1 - o.width;
            if s1.min(os1) - s0.max(os0) <= 0.5 {
                continue;
            }
            let (t_cab, t_o) = (cab.position.dot(v), o.position.dot(v));
            if (t_cab - t_o).abs() <= ISLAND_REACH {
                cab.position = cab.position + v * (t_o - t_cab);
                return;
            }
        } else if perpendicular(o.angle, cab.angle) {
            let pts = o.footprint();
            let (us, vs): (Vec<f64>, Vec<f64>) = pts.iter().map(|p| (p.dot(u), p.dot(v))).unzip();
            let (os0, os1) = (
                us.iter().copied().fold(f64::MAX, f64::min),
                us.iter().copied().fold(f64::MIN, f64::max),
            );
            let (ot0, ot1) = (
                vs.iter().copied().fold(f64::MAX, f64::min),
                vs.iter().copied().fold(f64::MIN, f64::max),
            );
            let (t0, t1) = (cab.position.dot(v), cab.position.dot(v) + cab.depth);
            if s1.min(os1) - s0.max(os0) <= 0.5 || t1.min(ot1) - t0.max(ot0) <= 0.5 {
                continue;
            }
            let moves = [(u, os1 - s0), (u, os0 - s1), (v, ot1 - t0), (v, ot0 - t1)];
            if let Some((axis, shift)) = moves
                .into_iter()
                .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            {
                cab.position = cab.position + axis * shift;
                return;
            }
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

/// Places `cab` (width, depth, angle already set) around `anchor`: against
/// the nearest wall within `reach` (rotated, flush, centered on the anchor's
/// projection and snapped to the grid unit), else free around `free_center`.
/// Returns whether it is on a wall.
#[allow(clippy::too_many_arguments)]
fn place_flush(
    cx: &EditorContext,
    cab: &mut Cabinet,
    anchor: Point,
    free_center: Point,
    reach: f64,
    use_walls: bool,
    alt: bool,
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
    on_wall
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
    let on_wall = place_flush(cx, cab, anchor, free_center, reach, use_walls, alt);
    bump(&load_cabinets(cx.floor()), cab, exclude);
    on_wall
}

/// Settles a filler: against the wall like any cabinet, then sized to the
/// gap it sits in (between a wall and a cabinet, or two cabinets). Without a
/// bounded gap of 12" or less it keeps its width and bumps like a cabinet.
/// Returns the gap width when it fitted.
#[allow(clippy::too_many_arguments)]
pub fn settle_filler(
    cx: &EditorContext,
    cab: &mut Cabinet,
    anchor: Point,
    free_center: Point,
    reach: f64,
    use_walls: bool,
    alt: bool,
    exclude: Id,
) -> Option<f64> {
    let others = load_cabinets(cx.floor());
    if !follow_neighbor(&others, cab, anchor, reach, exclude, cx.snap_unit(), alt) {
        place_flush(cx, cab, anchor, free_center, reach, use_walls, alt);
    }
    let fit = fit_between(cab, &others, &wall_polys(cx));
    if fit.is_none() {
        bump(&others, cab, exclude);
    }
    fit
}

/// Lines a filler up with the run of the cabinet nearest `anchor` (within
/// `reach`): same angle, same back line, centered on the anchor's position
/// along the run. A filler clicked into the gap of a run follows the run,
/// not whichever wall happens to be closest. False when no cabinet is near.
fn follow_neighbor(
    others: &[Cabinet],
    cab: &mut Cabinet,
    anchor: Point,
    reach: f64,
    exclude: Id,
    unit: f64,
    alt: bool,
) -> bool {
    let near = others
        .iter()
        .filter(|o| o.id != exclude && !o.kind.is_custom() && vertical_overlap(o, cab))
        .map(|o| {
            let ring = o.footprint();
            let d = (0..ring.len())
                .map(|i| dist_to_segment(anchor, ring[i], ring[(i + 1) % ring.len()]))
                .fold(f64::MAX, f64::min);
            (d, o)
        })
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, n)) = near else {
        return false;
    };
    cab.angle = n.angle;
    let u = Point::new(n.angle.cos(), n.angle.sin());
    let v = u.perp();
    let s = snap_to(anchor.dot(u) - cab.width * 0.5, unit, alt);
    cab.position = u * s + v * n.position.dot(v);
    true
}

/// The inside corner of two perpendicular walls nearest `anchor` (within
/// [`CORNER_REACH`]): the corner point where their room-side faces meet, and
/// the direction along which a corner cabinet's back (its width, `u`) runs so
/// that its other leg (`u` turned a quarter counter-clockwise) runs along the
/// second wall.
fn corner_at(cx: &EditorContext, anchor: Point) -> Option<(Point, Point)> {
    let walls: Vec<&plan_core::Wall> = visible_walls(cx).collect();
    let mut best: Option<(f64, Point, Point)> = None;
    for (i, wi) in walls.iter().enumerate() {
        for wj in &walls[i + 1..] {
            let (di, dj) = (wi.direction(), wj.direction());
            if di.dot(dj).abs() > 0.02 {
                continue;
            }
            // The face line of each wall on the anchor's side.
            let face = |w: &plan_core::Wall| {
                let (_, q) = project_on_segment(anchor, w.start, w.end);
                let side = if anchor.sub(q).dot(w.normal()) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                w.start + w.normal() * (side * w.thickness * 0.5)
            };
            let (pi, pj) = (face(wi), face(wj));
            let t = pj.sub(pi).cross(dj) / di.cross(dj);
            let c = pi + di * t;
            let near = |w: &plan_core::Wall| {
                dist_to_segment(c, w.start, w.end) <= (wi.thickness.hypot(wj.thickness)) * 0.5 + 0.5
            };
            let dist = anchor.dist(c);
            if dist > CORNER_REACH || !near(wi) || !near(wj) {
                continue;
            }
            let away = |d: Point| {
                if anchor.sub(c).dot(d) >= 0.0 {
                    d
                } else {
                    d * -1.0
                }
            };
            let (ai, aj) = (away(di), away(dj));
            let u = if ai.cross(aj) > 0.0 { ai } else { aj };
            if best.as_ref().is_none_or(|b| dist < b.0) {
                best = Some((dist, c, u));
            }
        }
    }
    best.map(|(_, c, u)| (c, u))
}

/// Turns a blind cabinet's hidden end toward the nearest perpendicular wall
/// within [`BLIND_REACH`] (the neighbouring run takes the hidden part).
fn orient_blind(cx: &EditorContext, cab: &mut Cabinet) {
    let Some(blind) = cab.blind.as_mut() else {
        return;
    };
    let u = Point::new(cab.angle.cos(), cab.angle.sin());
    let Ok((left, right)) = plan_cabinets::free_span(
        &wall_polys(cx),
        cab.position,
        u,
        u.perp(),
        (0.05, cab.depth - 0.05),
        cab.width / 2.0,
    ) else {
        return;
    };
    let gap_left = left.map(|l| -l);
    let gap_right = right.map(|r| r - cab.width);
    let pick = match (gap_left, gap_right) {
        (Some(l), Some(r)) => Some(if l <= r {
            BlindSide::Left
        } else {
            BlindSide::Right
        }),
        (Some(_), None) => Some(BlindSide::Left),
        (None, Some(_)) => Some(BlindSide::Right),
        (None, None) => None,
    };
    let near = [gap_left, gap_right]
        .into_iter()
        .flatten()
        .fold(f64::MAX, f64::min)
        <= BLIND_REACH;
    if let (Some(side), true) = (pick, near) {
        blind.side = side;
    }
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

/// A polygon or path being drawn (Custom Countertop, Backsplash, Counter Hole).
#[derive(Default)]
struct PolyDraw {
    points: Vec<Point>,
    cursor: Option<Point>,
    /// Where the button went down, until it comes up.
    press: Option<(Point, Pos2)>,
    /// Far corner of a rectangle being dragged out.
    rect_to: Option<Point>,
}

impl PolyDraw {
    fn active(&self) -> bool {
        !self.points.is_empty() || self.press.is_some() || self.rect_to.is_some()
    }
}

pub struct CabinetTool {
    kind: CabinetKind,
    ghost: Option<Cabinet>,
    press: Option<Press>,
    edit: Option<EditDrag>,
    poly: PolyDraw,
}

impl Default for CabinetTool {
    fn default() -> Self {
        Self {
            kind: CabinetKind::Base,
            ghost: None,
            press: None,
            edit: None,
            poly: PolyDraw::default(),
        }
    }
}

/// Kinds whose click-drag sets a width in 3" steps.
fn supports_width_drag(kind: CabinetKind) -> bool {
    !(kind.is_filler() || kind.is_corner() || kind.is_blind() || kind.is_custom())
}

impl CabinetTool {
    /// The cabinet kind being placed.
    pub fn kind(&self) -> CabinetKind {
        self.kind
    }

    pub fn set_kind(&mut self, kind: CabinetKind) {
        self.kind = kind;
        self.ghost = None;
        self.poly = PolyDraw::default();
    }

    /// A fresh cabinet of the active variant placed for a click at `p`.
    fn placed_at(&self, cx: &EditorContext, p: &PointerEvent) -> Cabinet {
        let kind = self.kind();
        let mut cab = default_cabinet(cx, kind);
        let alt = p.modifiers.alt;
        if kind.is_filler() {
            settle_filler(cx, &mut cab, p.world, p.snapped, WALL_REACH, true, alt, 0);
            return cab;
        }
        if kind.is_corner() {
            if let Some((corner, u)) = corner_at(cx, p.world) {
                cab.angle = u.angle();
                cab.position = corner;
                return cab;
            }
        }
        settle(cx, &mut cab, p.world, p.snapped, WALL_REACH, true, alt, 0);
        if kind.is_blind() {
            orient_blind(cx, &mut cab);
        }
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
        let mut did = self.press.take().is_some() || self.poly.active();
        self.poly = PolyDraw::default();
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

    // ----- polygon tools -----

    /// Thickness and top height of a new custom countertop, from the base
    /// cabinet defaults.
    fn top_sizes(cx: &EditorContext) -> (f64, f64) {
        let b = &cx.defaults.cabinets.base;
        (b.countertop_thickness, b.height)
    }

    /// Finishes the polygon (or path) with the points collected so far.
    fn finish_poly(&mut self, cx: &mut EditorContext) -> ToolResult {
        let mut pts = std::mem::take(&mut self.poly.points);
        self.poly = PolyDraw::default();
        while pts.len() > 1 && pts[pts.len() - 1].dist(pts[pts.len() - 2]) < 0.01 {
            pts.pop();
        }
        if pts.len() > 2 && pts[0].dist(pts[pts.len() - 1]) < 0.01 {
            pts.pop();
        }
        let (thickness, top) = Self::top_sizes(cx);
        let fl = cx.floor;
        match self.kind {
            CabinetKind::CustomCountertop => {
                let Some(cab) = Cabinet::custom_countertop(&pts, thickness, top) else {
                    cx.status = "A countertop needs three corners".into();
                    return ToolResult::consumed();
                };
                cx.begin_change("Place Custom Countertop");
                match add_cabinet(&mut cx.project, fl, cab) {
                    Some(id) => {
                        cx.selection.set(ObjectRef::Cabinet(id));
                        cx.mark_dirty();
                        cx.status.clear();
                        ToolResult::committed("Place Custom Countertop")
                    }
                    None => {
                        cx.cancel_change();
                        cx.status = "The plan's cabinets could not be read".into();
                        ToolResult::consumed()
                    }
                }
            }
            CabinetKind::CustomBacksplash => {
                let Some(cab) = Cabinet::custom_backsplash(&pts, 4.0, 0.5, top) else {
                    cx.status = "A backsplash needs two points".into();
                    return ToolResult::consumed();
                };
                cx.begin_change("Place Custom Backsplash");
                match add_cabinet(&mut cx.project, fl, cab) {
                    Some(id) => {
                        cx.selection.set(ObjectRef::Cabinet(id));
                        cx.mark_dirty();
                        cx.status.clear();
                        ToolResult::committed("Place Custom Backsplash")
                    }
                    None => {
                        cx.cancel_change();
                        cx.status = "The plan's cabinets could not be read".into();
                        ToolResult::consumed()
                    }
                }
            }
            _ => {
                cx.begin_change("Place Counter Hole");
                match placed::add_counter_hole(cx, &pts) {
                    Some(id) => {
                        cx.selection.set(ObjectRef::Cabinet(id));
                        cx.mark_dirty();
                        cx.status.clear();
                        ToolResult::committed("Place Counter Hole")
                    }
                    None => {
                        cx.cancel_change();
                        cx.status = "Draw the hole inside a countertop".into();
                        ToolResult::consumed()
                    }
                }
            }
        }
    }

    fn poly_down(&mut self, p: &PointerEvent) -> ToolResult {
        self.poly.press = Some((p.snapped, p.screen));
        self.poly.cursor = Some(p.snapped);
        ToolResult::consumed()
    }

    fn poly_move(&mut self, p: &PointerEvent) -> ToolResult {
        self.poly.cursor = Some(p.snapped);
        if let (true, Some((_, screen))) = (p.down, self.poly.press) {
            let rect_ok = self.kind != CabinetKind::CustomBacksplash;
            if rect_ok
                && self.poly.points.is_empty()
                && (p.screen - screen).length() >= DRAG_THRESHOLD_PX
            {
                self.poly.rect_to = Some(p.snapped);
            }
        }
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn poly_up(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some((start, _)) = self.poly.press.take() else {
            return ToolResult::ignored();
        };
        if self.poly.rect_to.take().is_some() {
            let end = p.snapped;
            if (end.x - start.x).abs() < 0.5 || (end.y - start.y).abs() < 0.5 {
                return ToolResult::consumed();
            }
            self.poly.points = vec![
                start,
                Point::new(end.x, start.y),
                end,
                Point::new(start.x, end.y),
            ];
            return self.finish_poly(cx);
        }
        let closes = self.poly.points.len() >= 3
            && start.dist(self.poly.points[0]) <= cx.pick_tol().max(1.0);
        if closes {
            return self.finish_poly(cx);
        }
        if self
            .poly
            .points
            .last()
            .is_none_or(|last| last.dist(start) > 0.01)
        {
            self.poly.points.push(start);
        }
        cx.status = self.hint();
        ToolResult::consumed()
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
            if orig.kind.is_custom() {
                // Free-form tops move as they are: no wall or neighbour snapping.
                c.position = Point::new(
                    snap_to(c.position.x, unit, alt),
                    snap_to(c.position.y, unit, alt),
                );
            } else if orig.kind.is_filler() {
                settle_filler(
                    cx,
                    &mut c,
                    anchor,
                    center,
                    WALL_REACH + orig.depth * 0.5,
                    !ctrl,
                    alt,
                    orig.id,
                );
            } else {
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
        self.kind.name()
    }

    fn hint(&self) -> String {
        match self.kind {
            CabinetKind::CounterHole => {
                "Custom Counter Hole: click the corners of the hole inside a countertop (or drag a rectangle), Enter finishes".to_string()
            }
            CabinetKind::CustomCountertop => {
                "Custom Countertop: click the corners (or drag a rectangle), Enter or the first corner finishes, Backspace removes a corner".to_string()
            }
            CabinetKind::CustomBacksplash => {
                "Custom Backsplash: click the points of its path, Enter finishes, Backspace removes a point".to_string()
            }
            k if k.is_filler() => {
                "Click into a gap to fill it between the wall and a cabinet; Tab changes the cabinet type".to_string()
            }
            k if k.is_corner() => {
                "Click near an inside corner to fit the corner cabinet; Tab changes the cabinet type".to_string()
            }
            k => format!(
                "{}: click to place, drag to set the width; Tab changes the cabinet type",
                k.name()
            ),
        }
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
        if is_polygon(self.kind) {
            return self.poly_move(&p);
        }
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
            let kind = self.kind;
            if let Some(pr) = &mut self.press {
                let u = Point::new(pr.angle.cos(), pr.angle.sin());
                let du = p.world.sub(pr.start).dot(u);
                let draggable = supports_width_drag(kind);
                if draggable && (pr.dragged || (p.screen - pr.screen).length() >= DRAG_THRESHOLD_PX)
                {
                    pr.dragged = true;
                    let width = (du.abs() / WIDTH_STEP).round().max(1.0) * WIDTH_STEP;
                    let sign = if du >= 0.0 { 1.0 } else { -1.0 };
                    let center = pr.start + u * (sign * width * 0.5);
                    let mut cab = default_cabinet(cx, kind);
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
        if is_polygon(self.kind) {
            return self.poly_down(&p);
        }
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
            let probe = default_cabinet(cx, self.kind());
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

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if is_polygon(self.kind) {
            return self.poly_up(cx, &p);
        }
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
        let label = format!("Place {}", self.kind.name());
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
        if is_polygon(self.kind) && !self.poly.points.is_empty() {
            return self.finish_poly(cx);
        }
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
        if is_polygon(self.kind) && !self.poly.points.is_empty() {
            if k.is(Key::Enter) {
                return self.finish_poly(cx);
            }
            if k.is(Key::Backspace) || k.is(Key::Delete) {
                self.poly.points.pop();
                return ToolResult::consumed();
            }
        }
        if k.is(Key::Tab) && self.press.is_none() && self.edit.is_none() && !self.poly.active() {
            let i = KINDS.iter().position(|x| *x == self.kind).unwrap_or(0);
            self.set_kind(KINDS[(i + 1) % KINDS.len()]);
            cx.status = self.hint();
            return ToolResult::consumed();
        }
        if k.is(Key::G) && !k.modifiers.any() {
            return if placed::generate_countertops(cx) > 0 {
                ToolResult::committed("Generate Countertop")
            } else {
                ToolResult::consumed()
            };
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
        if is_polygon(self.kind) {
            self.draw_poly(cx, painter, cam);
        } else {
            handles::draw(&Self::handles(cx), painter, cam, pal);
        }
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

impl CabinetTool {
    /// How many corners of a polygon in progress are placed.
    #[cfg(test)]
    fn draw_poly_points(&self) -> usize {
        self.poly.points.len()
    }

    fn draw_poly(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let stroke = egui::Stroke::new(1.5_f32, cx.palette.ghost_stroke);
        let pts: Vec<Pos2> =
            if let (Some((start, _)), Some(to)) = (self.poly.press, self.poly.rect_to) {
                [
                    start,
                    Point::new(to.x, start.y),
                    to,
                    Point::new(start.x, to.y),
                    start,
                ]
                .iter()
                .map(|q| cam.world_to_screen(*q))
                .collect()
            } else {
                self.poly
                    .points
                    .iter()
                    .chain(self.poly.cursor.iter())
                    .map(|q| cam.world_to_screen(*q))
                    .collect()
            };
        if pts.len() >= 2 {
            painter.add(egui::Shape::line(pts.clone(), stroke));
        }
        for q in self.poly.points.iter().map(|q| cam.world_to_screen(*q)) {
            painter.circle_filled(q, 3.0, cx.palette.ghost_stroke);
        }
    }
}

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

    // ----- fillers, corners, blind cabinets, islands, polygon tools -----

    fn put_kind(cx: &mut EditorContext, kind: CabinetKind, x: f64, y: f64, angle: f64) -> Id {
        let cab = default_cabinet(cx, kind);
        put(cx, cab, x, y, angle)
    }

    fn put(cx: &mut EditorContext, mut cab: Cabinet, x: f64, y: f64, angle: f64) -> Id {
        cab.position = Point::new(x, y);
        cab.angle = angle;
        add_cabinet(&mut cx.project, 0, cab).unwrap()
    }

    fn vertical_wall(cx: &mut EditorContext, x: f64) {
        cx.project.add_wall(
            0,
            Point::new(x, 0.0),
            Point::new(x, 200.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
    }

    fn click_clear(t: &mut CabinetTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        cx.selection.clear();
        click(t, cx, x, y)
    }

    #[test]
    fn the_cabinet_flyout_has_no_unimplemented_entries() {
        let fly = crate::toolbar::cabinet();
        let mut kinds = Vec::new();
        for e in &fly.entries {
            match e.action {
                crate::toolbar::Action::SetTool(ToolId::CabinetVariant(k)) => kinds.push(k),
                ref other => panic!("{} is not a tool entry: {other:?}", e.name),
            }
        }
        for k in KINDS {
            assert!(kinds.contains(&k), "{k:?} is missing from the flyout");
        }
        let names: Vec<&str> = fly.entries.iter().map(|e| e.name).collect();
        for want in [
            "Base Filler",
            "Wall Filler",
            "Full Height Filler",
            "Custom Countertop",
            "Custom Backsplash",
            "Custom Counter Hole",
            "Corner Base Cabinet",
            "Blind Wall Cabinet",
        ] {
            assert!(names.contains(&want), "{want}");
        }
    }

    #[test]
    fn tab_walks_every_kind_and_every_kind_places_or_draws() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        let mut seen = vec![t.kind()];
        for _ in 1..KINDS.len() {
            assert!(t.key(&mut cx, KeyEvent::key(Key::Tab)).consumed);
            seen.push(t.kind());
        }
        assert_eq!(seen, KINDS);
        assert!(t.key(&mut cx, KeyEvent::key(Key::Tab)).consumed);
        assert_eq!(t.kind(), CabinetKind::Base, "wraps around");
        for k in KINDS {
            t.set_kind(k);
            assert!(!t.name().is_empty() && !t.hint().is_empty(), "{k:?}");
            if is_polygon(k) {
                continue;
            }
            let before = cabs(&cx).len();
            click_clear(&mut t, &mut cx, 150.0, 10.0);
            let list = cabs(&cx);
            assert_eq!(list.len(), before + 1, "{k:?} placed nothing");
            let c = list.last().unwrap();
            assert_eq!(c.kind, k);
            // Every placed kind draws and meshes.
            assert!(!plan_cabinets::plan_symbol(c).is_empty());
            assert!(!plan_cabinets::meshes(c).is_empty(), "{k:?} has no 3D");
        }
    }

    #[test]
    fn a_filler_fills_the_gap_between_a_cabinet_and_a_wall() {
        let mut cx = setup();
        // Base 48..72 against the wall; a side wall whose face is at x = 77.
        put_kind(&mut cx, CabinetKind::Base, 48.0, 3.0, 0.0);
        vertical_wall(&mut cx, 80.0);
        let mut t = CabinetTool::default();
        t.set_kind(CabinetKind::BaseFiller);
        click_clear(&mut t, &mut cx, 74.0, 10.0);
        let f = cabs(&cx).into_iter().find(|c| c.kind.is_filler()).unwrap();
        assert!((f.width - 5.0).abs() < 1e-9, "{}", f.width);
        assert!((f.position.x - 72.0).abs() < 1e-9 && (f.position.y - 3.0).abs() < 1e-9);
        assert_eq!((f.depth, f.height), (24.0, 36.0));
        assert_eq!(cx.undo_label(), Some("Place Base Filler"));
    }

    #[test]
    fn a_filler_between_two_cabinets_takes_the_gap_and_keeps_default_width_without_one() {
        let mut cx = setup();
        put_kind(&mut cx, CabinetKind::Base, 48.0, 3.0, 0.0);
        put_kind(&mut cx, CabinetKind::Base, 80.0, 3.0, 0.0);
        let mut t = CabinetTool::default();
        t.set_kind(CabinetKind::BaseFiller);
        click_clear(&mut t, &mut cx, 75.0, 10.0);
        let f = cabs(&cx).into_iter().find(|c| c.kind.is_filler()).unwrap();
        assert!((f.width - 8.0).abs() < 1e-9 && (f.position.x - 72.0).abs() < 1e-9);
        // No bounded gap: the default 3" filler, bumped like any cabinet.
        click_clear(&mut t, &mut cx, 180.0, 10.0);
        let g = cabs(&cx)
            .into_iter()
            .rev()
            .find(|c| c.kind.is_filler())
            .unwrap();
        assert_eq!(g.width, 3.0);
        // Wall fillers fit between wall cabinets, ignoring the bases below.
        t.set_kind(CabinetKind::WallFiller);
        put_kind(&mut cx, CabinetKind::Wall, 90.0, 3.0, 0.0);
        click_clear(&mut t, &mut cx, 75.0, 10.0);
        let w = cabs(&cx)
            .into_iter()
            .rev()
            .find(|c| c.kind == CabinetKind::WallFiller)
            .unwrap();
        assert_eq!((w.depth, w.elevation), (12.0, 54.0));
    }

    #[test]
    fn a_corner_cabinet_sits_in_the_inside_corner_of_two_walls() {
        let mut cx = setup();
        vertical_wall(&mut cx, 0.0);
        let mut t = CabinetTool::default();
        t.set_kind(CabinetKind::CornerBase);
        click_clear(&mut t, &mut cx, 20.0, 20.0);
        let c = &cabs(&cx)[0];
        assert!(same_angle(c.angle, 0.0));
        // Faces of the two walls meet at (3, 3).
        assert!(
            c.position.dist(Point::new(3.0, 3.0)) < 1e-9,
            "{:?}",
            c.position
        );
        assert_eq!((c.width, c.depth), (36.0, 36.0));
        let fp = c.footprint();
        assert!((plan_core::geometry::polygon_area(&fp) - 1224.0).abs() < 1e-6);
        assert!(fp.iter().all(|p| p.x >= 3.0 - 1e-9 && p.y >= 3.0 - 1e-9));
        // The other end of the same wall: the corner turns a quarter.
        vertical_wall(&mut cx, 240.0);
        click_clear(&mut t, &mut cx, 225.0, 20.0);
        let c2 = &cabs(&cx)[1];
        assert!(same_angle(c2.angle, FRAC_PI_2), "{}", c2.angle);
        assert!(
            c2.position.dist(Point::new(237.0, 3.0)) < 1e-9,
            "{:?}",
            c2.position
        );
        let fp = c2.footprint();
        assert!(
            fp.iter().all(|p| p.x <= 237.0 + 1e-9 && p.y >= 3.0 - 1e-9),
            "{fp:?}"
        );
        // A wall corner at the same spot hangs at 54" with 24" legs.
        t.set_kind(CabinetKind::CornerWall);
        click_clear(&mut t, &mut cx, 20.0, 20.0);
        let w = cabs(&cx)
            .into_iter()
            .find(|c| c.kind == CabinetKind::CornerWall)
            .unwrap();
        assert_eq!((w.width, w.elevation), (24.0, 54.0));
        assert!(w.position.dist(Point::new(3.0, 3.0)) < 1e-9);
        // Far from any corner a corner cabinet just stands on the wall.
        click_clear(&mut t, &mut cx, 140.0, 10.0);
        let far = cabs(&cx).into_iter().last().unwrap();
        assert!((far.position.y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_blind_cabinet_hides_the_end_nearest_a_perpendicular_wall() {
        let mut cx = setup();
        vertical_wall(&mut cx, 0.0);
        vertical_wall(&mut cx, 260.0);
        let mut t = CabinetTool::default();
        t.set_kind(CabinetKind::BlindBase);
        click_clear(&mut t, &mut cx, 33.0, 10.0);
        let left = cabs(&cx).into_iter().next().unwrap();
        assert_eq!(left.blind.unwrap().side, BlindSide::Left);
        assert_eq!((left.width, left.blind.unwrap().blind_width), (48.0, 15.0));
        click_clear(&mut t, &mut cx, 230.0, 10.0);
        let right = cabs(&cx).into_iter().last().unwrap();
        assert_eq!(right.blind.unwrap().side, BlindSide::Right);
    }

    #[test]
    fn back_to_back_cabinets_form_an_island_and_runs_push_out_of_each_other() {
        let mut cx = setup();
        let a = default_cabinet(&cx, CabinetKind::Base);
        // An island: a cabinet facing the other way, backs 3" apart, snaps flush.
        let first = put(&mut cx, a.clone(), 50.0, 100.0, 0.0);
        let mut b = a.clone();
        b.position = Point::new(74.0, 103.0);
        b.angle = std::f64::consts::PI;
        bump(&load_cabinets(cx.floor()), &mut b, 0);
        assert!((b.position.y - 100.0).abs() < 1e-9, "{:?}", b.position);
        assert!((b.position.x - 74.0).abs() < 1e-9);
        // Too far apart: left alone.
        let mut far = a.clone();
        far.position = Point::new(74.0, 112.0);
        far.angle = std::f64::consts::PI;
        bump(&load_cabinets(cx.floor()), &mut far, 0);
        assert!((far.position.y - 112.0).abs() < 1e-9);
        let _ = first;
        // A peninsula: a run that overlaps another at a right angle is pushed
        // out along the shorter way, to butt against its front.
        let mut run = a.clone();
        run.width = 48.0;
        put(&mut cx, run, 50.0, 3.0, 0.0);
        let mut pen = a.clone();
        pen.position = Point::new(90.0, 10.0);
        pen.angle = FRAC_PI_2;
        bump(&load_cabinets(cx.floor()), &mut pen, 0);
        assert!((pen.position.y - 27.0).abs() < 1e-9, "{:?}", pen.position);
        assert!((pen.position.x - 90.0).abs() < 1e-9);
    }

    #[test]
    fn custom_countertop_by_clicks_by_dragging_and_with_backspace() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        t.set_kind(CabinetKind::CustomCountertop);
        for (x, y) in [(12.0, 96.0), (72.0, 96.0), (72.0, 120.0), (12.0, 120.0)] {
            click_clear(&mut t, &mut cx, x, y);
        }
        assert!(cabs(&cx).is_empty(), "still drawing");
        assert!(t.draw_poly_points() == 4);
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Place Custom Countertop"));
        let top = &cabs(&cx)[0];
        assert_eq!(top.kind, CabinetKind::CustomCountertop);
        assert!((top.countertop_volume() - 60.0 * 24.0 * 1.5).abs() < 1e-6);
        assert_eq!(top.elevation + top.height, 36.0);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Cabinet(top.id)));
        assert_eq!(cx.undo().as_deref(), Some("Place Custom Countertop"));
        assert!(cabs(&cx).is_empty());

        // Clicking the first corner again closes the shape.
        for (x, y) in [(12.0, 96.0), (72.0, 96.0), (72.0, 120.0), (12.0, 96.0)] {
            click_clear(&mut t, &mut cx, x, y);
        }
        assert_eq!(cabs(&cx).len(), 1);
        assert_eq!(cabs(&cx)[0].custom.as_ref().unwrap().outline.len(), 3);
        cx.undo();

        // A drag draws a rectangle.
        cx.selection.clear();
        let down = PointerEvent::at(&cx, Point::new(12.0, 96.0));
        t.pointer_move(&mut cx, down);
        t.pointer_down(&mut cx, down.with_down(true));
        let mut mv = PointerEvent::at(&cx, Point::new(72.0, 120.0)).with_down(true);
        mv.screen = Pos2::new(200.0, 80.0);
        t.pointer_move(&mut cx, mv);
        let r = t.pointer_up(&mut cx, mv);
        assert!(r.commit.is_some());
        let rect = &cabs(&cx)[0];
        assert_eq!((rect.width, rect.depth), (60.0, 24.0));
        cx.undo();

        // Backspace drops a corner; fewer than three corners make nothing.
        for (x, y) in [(12.0, 96.0), (72.0, 96.0), (72.0, 120.0)] {
            click_clear(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::key(Key::Backspace));
        assert_eq!(t.draw_poly_points(), 2);
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(cabs(&cx).is_empty());
        assert!(cx.status.contains("three corners"));
        // Esc cancels a shape in progress.
        click_clear(&mut t, &mut cx, 12.0, 96.0);
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert_eq!(t.draw_poly_points(), 0);
    }

    #[test]
    fn custom_backsplash_follows_a_path() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        t.set_kind(CabinetKind::CustomBacksplash);
        click_clear(&mut t, &mut cx, 12.0, 96.0);
        click_clear(&mut t, &mut cx, 72.0, 96.0);
        // Dragging does not make a rectangle for a path.
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Place Custom Backsplash"));
        let bs = &cabs(&cx)[0];
        assert_eq!(bs.kind, CabinetKind::CustomBacksplash);
        assert_eq!((bs.height, bs.elevation), (4.0, 36.0));
        assert!((plan_cabinets::ring_area(&bs.footprint()) - 60.0 * 0.5).abs() < 1e-6);
    }

    #[test]
    fn counter_hole_is_cut_in_the_countertop_it_lies_in() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click_clear(&mut t, &mut cx, 150.0, 100.0); // a free base cabinet
        let id = cabs(&cx)[0].id;
        let center = cabs(&cx)[0].to_plan(Point::new(12.0, 12.0));
        t.set_kind(CabinetKind::CounterHole);
        // A hole off any countertop is refused with a hint.
        for (x, y) in [(300.0, 150.0), (306.0, 150.0), (306.0, 156.0)] {
            click_clear(&mut t, &mut cx, x, y);
        }
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(cabs(&cx)[0].cutouts.is_empty());
        assert!(cx.status.contains("inside a countertop"), "{}", cx.status);
        // Inside: a rectangle by drag.
        let a = center + Point::new(-6.0, -6.0);
        let b = center + Point::new(6.0, 6.0);
        cx.selection.clear();
        let down = PointerEvent::at(&cx, a);
        t.pointer_move(&mut cx, down);
        t.pointer_down(&mut cx, down.with_down(true));
        let mut mv = PointerEvent::at(&cx, b).with_down(true);
        mv.screen = Pos2::new(100.0, 100.0);
        t.pointer_move(&mut cx, mv);
        let r = t.pointer_up(&mut cx, mv);
        assert_eq!(r.commit.as_deref(), Some("Place Counter Hole"));
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!(c.cutouts.len(), 1);
        assert!(c.countertop_volume() < Cabinet::base(24.0).countertop_volume());
        assert_eq!(cx.undo().as_deref(), Some("Place Counter Hole"));
        assert!(cabinet_by_id(cx.floor(), id).unwrap().cutouts.is_empty());
    }

    #[test]
    fn g_generates_the_countertop_over_adjacent_bases() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click_clear(&mut t, &mut cx, 60.0, 10.0);
        click_clear(&mut t, &mut cx, 80.0, 10.0);
        click_clear(&mut t, &mut cx, 100.0, 10.0);
        assert_eq!(cabs(&cx).len(), 3);
        // With one cabinet selected only that one's top is made (a
        // selection limits the cabinets joined) ...
        let r = t.key(&mut cx, KeyEvent::key(Key::G));
        assert_eq!(r.commit.as_deref(), Some("Generate Countertop"));
        assert_eq!(cabs(&cx).len(), 4);
        cx.undo();
        // ... with nothing selected, all adjacent bases join.
        cx.selection.clear();
        let r = t.key(&mut cx, KeyEvent::key(Key::G));
        assert_eq!(r.commit.as_deref(), Some("Generate Countertop"));
        let list = cabs(&cx);
        let tops: Vec<&Cabinet> = list
            .iter()
            .filter(|c| c.kind == CabinetKind::CustomCountertop)
            .collect();
        assert_eq!(tops.len(), 1);
        // Three 24" bases side by side: 72" plus the front overhang, one slab.
        let xs: Vec<(f64, f64)> = list
            .iter()
            .filter(|c| c.kind == CabinetKind::Base)
            .map(|c| (c.position.x, c.width))
            .collect();
        assert!(
            (tops[0].countertop_volume() - 72.0 * 25.0 * 1.5).abs() < 1e-6,
            "{} {xs:?}",
            tops[0].countertop_volume()
        );
        assert!(list
            .iter()
            .filter(|c| c.kind == CabinetKind::Base)
            .all(|c| c.countertop.is_none()));
        // Nothing left to join: no commit.
        assert!(t.key(&mut cx, KeyEvent::key(Key::G)).commit.is_none());
    }

    #[test]
    fn custom_countertops_neither_block_nor_bump_cabinets() {
        let mut cx = setup();
        let ring = [
            Point::new(40.0, 3.0),
            Point::new(100.0, 3.0),
            Point::new(100.0, 28.0),
            Point::new(40.0, 28.0),
        ];
        let top = Cabinet::custom_countertop(&ring, 1.5, 36.0).unwrap();
        add_cabinet(&mut cx.project, 0, top).unwrap();
        let mut t = CabinetTool::default();
        click_clear(&mut t, &mut cx, 60.0, 10.0);
        let base = cabs(&cx)
            .into_iter()
            .find(|c| c.kind == CabinetKind::Base)
            .unwrap();
        // Not slid aside: it sits where it was clicked.
        assert!((base.position.x - 48.0).abs() < 1e-9, "{:?}", base.position);
    }
}
