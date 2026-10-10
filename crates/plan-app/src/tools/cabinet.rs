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
    self, add_cabinet, cabinet_by_id, hit_cabinet, placed_handles, remove_cabinet, replace_cabinet,
    same_angle, PlacedRef,
};
use crate::editor::tempdim::{self, TempDims};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, EditorRequest, ObjectRef};
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Key, Pos2};
use plan_cabinets::{
    auto_fillers, fit_between, push_run, run_bounds, wall_polygon, width_for_space, Backsplash,
    BlindSide, Cabinet, CabinetKind, CabinetPreset, CornerSpec, CornerTreatment, EdgeProfile,
    FaceLayout, FillerOptions, HandleStyle, AUTO_FILLER_REACH,
};
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::{Floor, Id};
use std::f64::consts::FRAC_PI_2;

/// A click within this distance of a wall face places the cabinet on it, in.
pub const WALL_REACH: f64 = 12.0;
/// A corner cabinet snaps into a wall corner this close to the click, in.
pub const CORNER_REACH: f64 = 30.0;
/// Click-drag and resize steps when the General Cabinet Defaults ask for the
/// 3 in default increment, inches (CB-3, CB-8); see [`width_step`].
pub const WIDTH_STEP: f64 = 3.0;
/// Pixels the pointer must travel before a press becomes a drag.
const DRAG_THRESHOLD_PX: f32 = 3.0;
/// Depth resize steps, inches (CB-8).
pub const DEPTH_STEP: f64 = 1.0;
/// Smallest cabinet depth, inches.
const MIN_DEPTH: f64 = 3.0;
/// A cabinet dragged into a gap whose width is within this of its own takes
/// the gap's width (CB-5, fit to gap), inches.
pub const FIT_TOLERANCE: f64 = 2.0;
/// Back-to-back and perpendicular snapping reach, inches (3 in, manual p. 650).
const ISLAND_REACH: f64 = AUTO_FILLER_REACH;
/// A cabinet's back or front within this of its neighbour's lines up with it
/// (3 in, manual p. 650).
const ALIGN_REACH: f64 = AUTO_FILLER_REACH;
/// A blind cabinet hides its end this close to a perpendicular wall, in.
const BLIND_REACH: f64 = 30.0;
/// A resized edge dragged this close to a wall or the next cabinet snaps to
/// it (the cabinet fills the gap), inches. 3 in as in the manual (p. 650);
/// it was 4 in before round 16 (DECISIONS 53).
pub const EDGE_SNAP: f64 = AUTO_FILLER_REACH;

/// The Cabinet flyout, in order; Tab cycles through it. Custom Counter Hole
/// is a tool-only kind that cuts a hole instead of placing a cabinet.
pub const KINDS: [CabinetKind; 17] = [
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
    CabinetKind::SoffitPolygon,
];

/// Kinds drawn as a polygon or path instead of placed with one click.
pub fn is_polygon(kind: CabinetKind) -> bool {
    matches!(
        kind,
        CabinetKind::CustomCountertop
            | CabinetKind::CustomBacksplash
            | CabinetKind::CounterHole
            | CabinetKind::SoffitPolygon
    )
}

pub fn kind_name(kind: CabinetKind) -> &'static str {
    kind.name()
}

fn handle_style(name: &str) -> HandleStyle {
    HandleStyle::from_name(name).unwrap_or(HandleStyle::Knob)
}

fn edge_profile(name: &str) -> EdgeProfile {
    EdgeProfile::ALL
        .into_iter()
        .find(|e| e.name().eq_ignore_ascii_case(name))
        .unwrap_or(EdgeProfile::Square)
}

fn corner_treatment(name: &str) -> CornerTreatment {
    CornerTreatment::ALL
        .into_iter()
        .find(|c| c.name().eq_ignore_ascii_case(name))
        .unwrap_or(CornerTreatment::None)
}

/// A box-like cabinet of `kind` sized from `d`.
fn boxed(kind: CabinetKind, d: &plan_core::defaults::BoxDefaults) -> Cabinet {
    let mut c = Cabinet::new(kind, d.width);
    c.depth = d.depth;
    c.height = d.height;
    c.elevation = d.elevation;
    c
}

/// A new cabinet made from a library type (Vanity, Pantry, Tall Oven,
/// Refrigerator), sized from the plan's cabinet defaults and dressed like a
/// base or full-height cabinet (door and drawer styles, handle).
pub fn default_preset_cabinet(cx: &EditorContext, preset: CabinetPreset) -> Cabinet {
    let d = &cx.defaults.cabinets;
    let size = match preset {
        CabinetPreset::Vanity => &d.vanity,
        CabinetPreset::Pantry => &d.pantry,
        CabinetPreset::TallOven => &d.tall_oven,
        CabinetPreset::Refrigerator => &d.refrigerator,
    };
    let mut c = Cabinet::from_preset(preset, size.width);
    c.depth = size.depth;
    c.height = size.height;
    c.elevation = size.elevation;
    let b = &d.base;
    c.door_style.name = b.door_style.clone();
    c.door_style.handle = handle_style(&b.handle);
    c.drawer_style.name = b.drawer_style.clone();
    c.drawer_style.handle = handle_style(&b.handle);
    if let Some(t) = c.countertop.as_mut() {
        t.thickness = b.countertop_thickness;
        t.overhang_front = b.countertop_overhang;
        apply_countertop_defaults(t, &d.countertop);
    }
    if c.preset == Some(CabinetPreset::Vanity) {
        add_default_backsplash(&mut c, &d.backsplash);
    }
    // The fixed items of the type's face must still fit the new height.
    if c.face.resolve(c.face_height(), c.face_width()).is_err() {
        c.face = FaceLayout::base_default(c.face_height());
    }
    c
}

fn apply_countertop_defaults(
    t: &mut plan_cabinets::Countertop,
    d: &plan_core::defaults::CountertopDefaults,
) {
    t.overhang_sides = d.overhang_sides;
    t.overhang_back = d.overhang_back;
    t.edge = edge_profile(&d.edge);
    t.edge_size = d.edge_size;
    t.corner = corner_treatment(&d.corner);
    t.corner_size = d.corner_size;
}

fn add_default_backsplash(c: &mut Cabinet, d: &plan_core::defaults::BacksplashDefaults) {
    if d.enabled && c.countertop.is_some() {
        let mut b = Backsplash::new(d.height, d.thickness);
        b.full_height = d.full_height;
        c.backsplash = Some(b);
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
                apply_countertop_defaults(t, &d.countertop);
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
            add_default_backsplash(&mut c, &d.backsplash);
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
        CabinetKind::Soffit => boxed(kind, &d.soffit),
        CabinetKind::Shelf => boxed(kind, &d.shelf),
        CabinetKind::Partition => boxed(kind, &d.partition),
        other => Cabinet::new(other, 24.0),
    }
}

// ----- General Cabinet Defaults, automatic fillers, runs (round 16, brief 23) -----

/// The floor's cabinets as the tools see them: without the fillers the
/// program makes itself (they are rebuilt after every edit, see
/// [`sync_auto_fillers`], and never block, bump or push).
fn load_cabinets(floor: &Floor) -> Vec<Cabinet> {
    placed::load_cabinets(floor)
        .into_iter()
        .filter(|c| !c.auto_filler)
        .collect()
}

/// The General Cabinet Defaults of the plan, brought into their allowed range.
pub fn general(cx: &EditorContext) -> plan_core::defaults::GeneralCabinetDefaults {
    cx.defaults.cabinets.general.clamped()
}

/// The step width handles and click-drag move in: the Snap Grid unit with
/// Use Grid Snaps, else the Resize Increment (Cabinet Resizing).
pub fn width_step(cx: &EditorContext) -> f64 {
    let g = general(cx);
    if g.resize_by_grid {
        cx.snap_unit()
            .max(plan_core::defaults::GeneralCabinetDefaults::SMALLEST)
    } else {
        g.resize_increment
    }
}

/// The smallest cabinet that is placed or resized to (Minimum Cabinet Width).
pub fn min_width(cx: &EditorContext) -> f64 {
    general(cx).min_cabinet_width
}

/// The fillers asked for by the General Cabinet Defaults.
pub fn filler_options(cx: &EditorContext) -> FillerOptions {
    let g = general(cx);
    FillerOptions {
        enabled: g.create_automatic_fillers,
        angled: g.create_automatic_fillers_angled,
        reach: AUTO_FILLER_REACH,
    }
}

/// Adds the layer the module lines are drawn on after the cabinet layers
/// when the plan lacks it. Returns whether a layer was added.
pub fn ensure_module_lines_layer(layers: &mut plan_core::layers::LayerSet) -> bool {
    let name = plan_cabinets::MODULE_LINES_LAYER;
    if layers.get(name).is_some() {
        return false;
    }
    let color = layers.get("Cabinets, Base").map_or([0, 0, 0], |l| l.color);
    layers.add(plan_core::layers::Layer::new(name, color, 9))
}

/// `cabs` with every generated countertop taken apart: the cabinets that gave
/// up their slabs have them back, and the generated tops are left out.
fn without_generated_tops(cabs: &[Cabinet]) -> Vec<Cabinet> {
    let mut logical = cabs.to_vec();
    let tops: Vec<Cabinet> = logical
        .iter()
        .filter(|c| !c.joined.is_empty())
        .cloned()
        .collect();
    for t in &tops {
        plan_cabinets::release_joined_top(t, &mut logical);
    }
    logical.retain(|c| c.joined.is_empty());
    logical
}

/// Brings the automatic fillers up to date with the cabinets: the fillers
/// that Create Automatic Fillers asks for between cabinets and walls within
/// 3 in (see [`plan_cabinets::auto_fillers`]) are made, moved, resized or
/// removed to match. A filler that is still right keeps its id and is not
/// written. Part of the undo step the caller has begun; returns how many
/// fillers were added, replaced or removed.
pub fn sync_auto_fillers(cx: &mut EditorContext) -> usize {
    let fl = cx.floor;
    // Blind ends and exposed ends first (brief 24): the fillers and the
    // generated tops below read them.
    let special = crate::editor::cabinet_edit::sync_special(cx);
    let stored = placed::load_cabinets(cx.floor());
    if !stored.iter().any(|c| c.kind != CabinetKind::CounterHole) {
        return special;
    }
    // The cabinets as they are without any generated countertop on them (a
    // joined cabinet gave up its slab): fillers copy the cabinets' own tops.
    let logical = without_generated_tops(&stored)
        .into_iter()
        .filter(|c| !c.auto_filler)
        .collect::<Vec<_>>();
    let want = auto_fillers(&logical, &wall_polys(cx), &filler_options(cx));
    let have: Vec<&Cabinet> = stored.iter().filter(|c| c.auto_filler).collect();
    let mut claimed: Vec<Id> = Vec::new();
    let mut changes = special;
    let mut pending: Vec<Cabinet> = Vec::new();
    for w in want {
        // The same filler already stands: nothing to write.
        let same = have.iter().find(|h| {
            !claimed.contains(&h.id) && {
                let mut x = w.clone();
                x.id = h.id;
                x == ***h
            }
        });
        if let Some(h) = same {
            claimed.push(h.id);
            continue;
        }
        pending.push(w);
    }
    for mut w in pending {
        // A filler that moved a little keeps its id.
        let near = have.iter().find(|h| {
            !claimed.contains(&h.id)
                && h.kind == w.kind
                && h.to_plan(Point::ZERO).dist(w.to_plan(Point::ZERO)) < 6.0
        });
        match near {
            Some(h) => {
                claimed.push(h.id);
                w.id = h.id;
                if replace_cabinet(&mut cx.project, fl, &w) {
                    changes += 1;
                }
            }
            None => {
                if add_cabinet(&mut cx.project, fl, w).is_some() {
                    changes += 1;
                }
            }
        }
    }
    for h in have.iter().filter(|h| !claimed.contains(&h.id)) {
        if remove_cabinet(&mut cx.project, fl, h.id) {
            changes += 1;
        }
    }
    // The fillers just made or removed change which ends are mated.
    changes += crate::editor::cabinet_edit::sync_special(cx);
    if ensure_module_lines_layer(&mut cx.project.layers) {
        changes += 1;
    }
    if changes > 0 {
        let project = &cx.project;
        cx.selection.items.retain(|o| o.exists_in(project, fl));
        cx.mark_dirty();
    }
    changes
}

/// The free stretch around `cab` along its width axis when walls or cabinets
/// bound it on both sides: `(left, right)` offsets from its position.
fn bounded_space(cx: &EditorContext, cab: &Cabinet) -> Option<(f64, f64)> {
    let others = load_cabinets(cx.floor());
    match run_bounds(cab, &others, &wall_polys(cx)) {
        Some((Some(l), Some(r))) => Some((l, r)),
        _ => None,
    }
}

/// A cabinet too wide for the space it is placed in (walls and cabinets on
/// both sides) takes the largest multiple of the Resize Increment that fits
/// (a 24 in cabinet in a 20 in space with a 3 in increment becomes 18 in),
/// never below the Minimum Cabinet Width (manual p. 651). False when the
/// space is narrower than the Minimum Cabinet Width: no cabinet is placed.
fn fit_narrow_space(cx: &EditorContext, cab: &mut Cabinet) -> bool {
    if !fills_gaps(cab.kind) {
        return true;
    }
    let Some((l, r)) = bounded_space(cx, cab) else {
        return true;
    };
    let space = r - l;
    if space >= cab.width - 1e-9 {
        return true;
    }
    match width_for_space(cab.width, space, width_step(cx), min_width(cx)) {
        Some(w) => {
            let u = Point::new(cab.angle.cos(), cab.angle.sin());
            cab.position = cab.position + u * l;
            cab.width = w;
            true
        }
        None => false,
    }
}

/// A wall cabinet over an appliance takes its bottom from the appliance top
/// (CB-635): the appliance bay of a base or tall cabinet, or a free-standing
/// library appliance, whose top reaches above the wall cabinet's default
/// bottom. The height shrinks to stay under the ceiling. True when it moved.
fn stack_on_appliance(cx: &EditorContext, cab: &mut Cabinet) -> bool {
    if cab.kind != CabinetKind::Wall {
        return false;
    }
    let mut tops = plan_cabinets::cabinet_appliance_tops(&load_cabinets(cx.floor()));
    for s in &cx.floor().symbols {
        if placed::appliance_of_catalog(&s.catalog_id).is_some() {
            tops.push((s.footprint().to_vec(), s.elevation + s.height));
        }
    }
    let Some(bottom) = plan_cabinets::bottom_over_appliance(cab, &tops) else {
        return false;
    };
    cab.elevation = bottom;
    let room = cx.floor().ceiling_height - bottom;
    if room >= 12.0 && cab.height > room {
        cab.height = room;
    }
    true
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
pub(crate) fn wall_polys(cx: &EditorContext) -> Vec<Vec<Point>> {
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
        if dt.abs() <= ALIGN_REACH && dt.abs() > 1e-9 {
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
            if (t_cab - t_o).abs() <= ISLAND_REACH + 1e-6 {
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
    /// The space it lands in is wide enough for a cabinet.
    room: bool,
}

/// A handle drag of a selected cabinet.
struct EditDrag {
    op: HandleKind,
    original: Cabinet,
    start: Point,
    screen: Pos2,
    begun: bool,
    /// The floor's cabinets when the drag began, kept for a Move in Push
    /// mode: every step pushes from these, so dragging back lets the
    /// pushed neighbours return.
    snapshot: Option<Vec<Cabinet>>,
    /// The cabinets the last step pushed.
    pushed: Vec<Id>,
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
    /// A library type (Vanity, Pantry, ...) placed instead of the plain kind.
    preset: Option<CabinetPreset>,
    ghost: Option<Cabinet>,
    press: Option<Press>,
    edit: Option<EditDrag>,
    poly: PolyDraw,
}

impl Default for CabinetTool {
    fn default() -> Self {
        Self {
            kind: CabinetKind::Base,
            preset: None,
            ghost: None,
            press: None,
            edit: None,
            poly: PolyDraw::default(),
        }
    }
}

/// [`CabinetTool::new_cabinet`] without the tool (the borrow is elsewhere).
fn self_new_cabinet(
    preset: Option<CabinetPreset>,
    cx: &EditorContext,
    kind: CabinetKind,
) -> Cabinet {
    match preset {
        Some(p) => default_preset_cabinet(cx, p),
        None => default_cabinet(cx, kind),
    }
}

/// Kinds that take the width of the gap they are placed or dropped into.
fn fills_gaps(kind: CabinetKind) -> bool {
    matches!(
        kind,
        CabinetKind::Base | CabinetKind::Wall | CabinetKind::FullHeight
    )
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
        self.preset = None;
        self.ghost = None;
        self.poly = PolyDraw::default();
    }

    /// The library type being placed, if any.
    pub fn preset(&self) -> Option<CabinetPreset> {
        self.preset
    }

    /// Places a library type (Vanity, Pantry, Tall Oven, Refrigerator) until
    /// another kind is chosen; `None` goes back to the plain kind.
    pub fn set_preset(&mut self, preset: Option<CabinetPreset>) {
        match preset {
            Some(p) => {
                self.kind = p.kind();
                self.preset = Some(p);
                self.ghost = None;
                self.poly = PolyDraw::default();
            }
            None => self.preset = None,
        }
    }

    /// A fresh cabinet of the active variant, at the origin.
    fn new_cabinet(&self, cx: &EditorContext) -> Cabinet {
        match self.preset {
            Some(p) => default_preset_cabinet(cx, p),
            None => default_cabinet(cx, self.kind),
        }
    }

    /// A fresh cabinet of the active variant placed for a click at `p`.
    fn placed_at(&self, cx: &EditorContext, p: &PointerEvent) -> Cabinet {
        self.placed_with_room(cx, p).0
    }

    /// [`CabinetTool::placed_at`], and whether the space it lands in is wide
    /// enough for a cabinet at all (nothing narrower than the Minimum Cabinet
    /// Width is placed).
    fn placed_with_room(&self, cx: &EditorContext, p: &PointerEvent) -> (Cabinet, bool) {
        let kind = self.kind();
        let mut cab = self.new_cabinet(cx);
        let alt = p.overrides();
        if kind.is_filler() {
            settle_filler(cx, &mut cab, p.world, p.snapped, WALL_REACH, true, alt, 0);
            return (cab, true);
        }
        if kind.is_corner() {
            if let Some((corner, u)) = corner_at(cx, p.world) {
                cab.angle = u.angle();
                cab.position = corner;
                return (cab, true);
            }
        }
        let mut room = true;
        if fills_gaps(kind) && !alt {
            // Flush to the wall; then into the space between its neighbours:
            // Chief's "fill between" (a gap within the tolerance of its width
            // is taken as it is), else a smaller cabinet, a multiple of the
            // resize increment (manual p. 651); then bumped against them.
            place_flush(cx, &mut cab, p.world, p.snapped, WALL_REACH, true, alt);
            if !fit_gap(cx, &mut cab) {
                room = fit_narrow_space(cx, &mut cab);
            }
            bump(&load_cabinets(cx.floor()), &mut cab, 0);
        } else {
            settle(cx, &mut cab, p.world, p.snapped, WALL_REACH, true, alt, 0);
        }
        if kind.is_blind() {
            orient_blind(cx, &mut cab);
        }
        stack_on_appliance(cx, &mut cab);
        (cab, room)
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
            CabinetKind::SoffitPolygon => {
                // Same size and height as the Soffit variant's box.
                let base = Cabinet::new(CabinetKind::Soffit, 12.0);
                let Some(cab) = Cabinet::soffit_polygon(&pts, base.height, base.elevation) else {
                    cx.status = "A soffit needs three corners".into();
                    return ToolResult::consumed();
                };
                cx.begin_change("Place Soffit Polygon");
                match add_cabinet(&mut cx.project, fl, cab) {
                    Some(id) => {
                        cx.selection.set(ObjectRef::Cabinet(id));
                        cx.mark_dirty();
                        cx.status.clear();
                        ToolResult::committed("Place Soffit Polygon")
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
        HandleKind::ResizeStart | HandleKind::ResizeEnd | HandleKind::Reshape(_) => {
            "Resize Cabinet"
        }
        HandleKind::Label => "Move Cabinet Label",
        _ => "Move Cabinet",
    }
}

/// What a dragged cabinet does when it meets other cabinets (Chief's
/// Bumping/Pushing setting, CB-4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BumpMode {
    /// It stops against them, butted and lined up with their back (default).
    #[default]
    Bump,
    /// It pushes the cabinets of its run along ahead of it.
    Push,
    /// It passes over them.
    Off,
}

impl BumpMode {
    pub const ALL: [BumpMode; 3] = [BumpMode::Bump, BumpMode::Push, BumpMode::Off];

    pub fn name(self) -> &'static str {
        match self {
            BumpMode::Bump => "Bump",
            BumpMode::Push => "Push",
            BumpMode::Off => "Off",
        }
    }

    /// The Edit toolbar label that shows this mode and cycles to the next.
    pub fn toolbar_label(self) -> &'static str {
        match self {
            BumpMode::Bump => "Neighbors: Bump",
            BumpMode::Push => "Neighbors: Push",
            BumpMode::Off => "Neighbors: Pass Through",
        }
    }

    pub fn next(self) -> BumpMode {
        match self {
            BumpMode::Bump => BumpMode::Push,
            BumpMode::Push => BumpMode::Off,
            BumpMode::Off => BumpMode::Bump,
        }
    }
}

/// The Edit toolbar / menu command that cycles [`BumpMode`].
pub const BUMP_MODE_COMMAND: &str = "cabinet.bump_mode";

thread_local! {
    /// Edit > Snap Settings > Bumping/Pushing (cabinets).
    static BUMP_MODE: std::cell::Cell<BumpMode> = const { std::cell::Cell::new(BumpMode::Bump) };
    /// Preferences > Architectural: fit a dragged cabinet to the gap it is in.
    static FIT_TO_GAP: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    /// A gap within this many inches of a cabinet's width is filled.
    static FIT_TOLERANCE_IN: std::cell::Cell<f64> = const { std::cell::Cell::new(FIT_TOLERANCE) };
    /// The library type the next `set_variant` places.
    static REQUESTED_PRESET: std::cell::Cell<Option<CabinetPreset>> = const { std::cell::Cell::new(None) };
}

/// Sets the Bumping/Pushing mode of dragged cabinets.
pub fn set_bump_mode(mode: BumpMode) {
    BUMP_MODE.with(|m| m.set(mode));
}

/// The Bumping/Pushing mode of dragged cabinets.
pub fn bump_mode() -> BumpMode {
    BUMP_MODE.with(std::cell::Cell::get)
}

/// Sets how far a gap may differ from a cabinet's width and still be filled
/// (Preferences > Architectural; 2" until set). Clamped to 0..=12.
pub fn set_fit_tolerance(inches: f64) {
    FIT_TOLERANCE_IN.with(|f| f.set(inches.clamp(0.0, 12.0)));
}

/// The fit-to-gap tolerance, inches.
pub fn fit_tolerance() -> f64 {
    FIT_TOLERANCE_IN.with(std::cell::Cell::get)
}

/// Asks for a library type (Vanity, Pantry, ...) to be placed by the next
/// `ToolId::CabinetVariant(preset.kind())` the tool receives; a toolbar entry
/// for the type calls this before `set_active`.
pub fn request_preset(preset: CabinetPreset) {
    REQUESTED_PRESET.with(|r| r.set(Some(preset)));
}

/// The `Action::Custom` id of the toolbar and menu entry for a library type.
pub fn preset_command(preset: CabinetPreset) -> &'static str {
    match preset {
        CabinetPreset::Vanity => "cabinet.preset.vanity",
        CabinetPreset::Pantry => "cabinet.preset.pantry",
        CabinetPreset::TallOven => "cabinet.preset.tall_oven",
        CabinetPreset::Refrigerator => "cabinet.preset.refrigerator",
    }
}

/// Runs a library-type entry (`preset_command`): asks for the type and starts
/// the Cabinet tool on the kind it is built from. False for any other id.
pub fn run_preset_command(cx: &mut EditorContext, id: &str) -> bool {
    let Some(preset) = CabinetPreset::ALL
        .into_iter()
        .find(|p| preset_command(*p) == id)
    else {
        return false;
    };
    request_preset(preset);
    cx.requests
        .push(EditorRequest::SetTool(ToolId::CabinetVariant(
            preset.kind(),
        )));
    true
}

/// Turns fit to gap on or off.
pub fn set_fit_to_gap(on: bool) {
    FIT_TO_GAP.with(|f| f.set(on));
}

/// Is fit to gap on?
pub fn fit_to_gap_enabled() -> bool {
    FIT_TO_GAP.with(std::cell::Cell::get)
}

/// Fit to gap: sizes `c` to the gap it sits in when that gap is within
/// [`FIT_TOLERANCE`] of its width. True when it changed.
pub fn fit_gap(cx: &EditorContext, c: &mut Cabinet) -> bool {
    let others = load_cabinets(cx.floor());
    fit_gap_among(cx, &others, c)
}

/// [`fit_gap`] among the given cabinets.
fn fit_gap_among(cx: &EditorContext, others: &[Cabinet], c: &mut Cabinet) -> bool {
    if !fit_to_gap_enabled() || c.kind.is_custom() || c.kind.is_filler() {
        return false;
    }
    plan_cabinets::fit_to_gap(c, others, &wall_polys(cx), fit_tolerance())
        .is_some_and(|w| w >= min_width(cx))
}

/// The result of dragging a handle: the edited cabinet and, in Push mode,
/// the neighbours it pushed out of the way (at their new places).
#[derive(Debug, Clone)]
pub struct Edited {
    pub cab: Cabinet,
    pub pushed: Vec<Cabinet>,
}

/// The cabinet after dragging handle `op` from `start` to `p` (CB-8, CB-9).
/// Dragging into neighbours bumps ([`BumpMode::Push`] counts as bump here:
/// the pushed neighbours are the Cabinet tool's to move, see
/// [`apply_edit_mode`]).
pub fn apply_edit(
    cx: &EditorContext,
    op: HandleKind,
    orig: &Cabinet,
    start: Point,
    p: &PointerEvent,
) -> Cabinet {
    let mode = match bump_mode() {
        BumpMode::Push => BumpMode::Bump,
        m => m,
    };
    apply_edit_mode(cx, mode, op, orig, start, p, None).cab
}

/// [`apply_edit`] under an explicit Bumping/Pushing `mode`. `others` are the
/// cabinets as they were when the drag began (the floor's when `None`).
pub fn apply_edit_mode(
    cx: &EditorContext,
    mode: BumpMode,
    op: HandleKind,
    orig: &Cabinet,
    start: Point,
    p: &PointerEvent,
    others: Option<&[Cabinet]>,
) -> Edited {
    let owned;
    let others: &[Cabinet] = match others {
        Some(o) => o,
        None => {
            owned = load_cabinets(cx.floor());
            &owned
        }
    };
    let mut pushed: Vec<Cabinet> = Vec::new();
    let mut c = orig.clone();
    let alt = p.overrides();
    let (step, min_w) = (width_step(cx), min_width(cx));
    let u = Point::new(orig.angle.cos(), orig.angle.sin());
    let local_center = Point::new(orig.width * 0.5, orig.depth * 0.5);
    match op {
        HandleKind::Move => {
            let delta = p.world.sub(start);
            let anchor = orig.to_plan(local_center) + delta;
            let unit = cx.snap_unit();
            let center = Point::new(snap_to(anchor.x, unit, alt), snap_to(anchor.y, unit, alt));
            c.position = orig.position + delta;
            let ctrl = p.overrides();
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
                place_flush(
                    cx,
                    &mut c,
                    anchor,
                    center,
                    WALL_REACH + orig.depth * 0.5,
                    !ctrl,
                    alt,
                );
                match mode {
                    BumpMode::Off => {}
                    BumpMode::Bump => bump(others, &mut c, orig.id),
                    BumpMode::Push => match push_run(&c, others, &wall_polys(cx)) {
                        Some(run) => {
                            pushed = run;
                            if pushed.is_empty() {
                                // Nothing of the run in the way: islands and
                                // peninsulas still line up.
                                bump(others, &mut c, orig.id);
                            }
                        }
                        // The run cannot give way (a wall): stop against it.
                        None => bump(others, &mut c, orig.id),
                    },
                }
                // Between a wall and a cabinet (or two cabinets) with a gap
                // within 2" of its width, the cabinet takes the gap (CB-5).
                if !alt
                    && pushed.is_empty()
                    && mode != BumpMode::Off
                    && !orig.kind.is_corner()
                    && !orig.kind.is_blind()
                {
                    fit_gap_among(cx, others, &mut c);
                }
            }
        }
        HandleKind::ResizeEnd => {
            let s = p.world.sub(orig.position).dot(u);
            let mut w = snap_to(s, step, alt).max(min_w);
            if let (false, Some(r)) = (alt, edge_bounds(cx, orig, others).1) {
                if (s - r).abs() <= EDGE_SNAP && r >= min_w {
                    w = r;
                }
            }
            c.width = w;
        }
        HandleKind::ResizeStart => {
            let s = p.world.sub(orig.position).dot(u);
            let mut w = snap_to(orig.width - s, step, alt).max(min_w);
            if let (false, Some(l)) = (alt, edge_bounds(cx, orig, others).0) {
                if (s - l).abs() <= EDGE_SNAP && orig.width - l >= min_w {
                    w = orig.width - l;
                }
            }
            c.width = w;
            c.position = orig.position + u * (orig.width - w);
        }
        HandleKind::Reshape(n) => {
            let v = u.perp();
            let rel = p.world.sub(orig.position);
            // Where each edge is dragged to, in the cabinet's frame.
            let (sx, sy) = (rel.dot(u), rel.dot(v));
            let drag_left = matches!(n, placed::CORNER_BACK_LEFT | placed::CORNER_FRONT_LEFT);
            let drag_right = matches!(n, placed::CORNER_BACK_RIGHT | placed::CORNER_FRONT_RIGHT);
            let drag_back = matches!(
                n,
                placed::DEPTH_BACK | placed::CORNER_BACK_LEFT | placed::CORNER_BACK_RIGHT
            );
            let drag_front = matches!(
                n,
                placed::DEPTH_FRONT | placed::CORNER_FRONT_LEFT | placed::CORNER_FRONT_RIGHT
            );
            let mut shift = Point::ZERO;
            let (wall_l, wall_r) = if alt || !(drag_left || drag_right) {
                (None, None)
            } else {
                edge_bounds(cx, orig, others)
            };
            if drag_right {
                let mut w = snap_to(sx, step, alt).max(min_w);
                if let Some(r) = wall_r.filter(|r| (sx - r).abs() <= EDGE_SNAP && *r >= min_w) {
                    w = r;
                }
                c.width = w;
            }
            if drag_left {
                let mut w = snap_to(orig.width - sx, step, alt).max(min_w);
                if let Some(l) =
                    wall_l.filter(|l| (sx - l).abs() <= EDGE_SNAP && orig.width - l >= min_w)
                {
                    w = orig.width - l;
                }
                c.width = w;
                shift = shift + u * (orig.width - w);
            }
            if drag_front {
                c.depth = snap_to(sy, DEPTH_STEP, alt).max(MIN_DEPTH);
            }
            if drag_back {
                let d = snap_to(orig.depth - sy, DEPTH_STEP, alt).max(MIN_DEPTH);
                c.depth = d;
                shift = shift + v * (orig.depth - d);
            }
            c.position = orig.position + shift;
        }
        HandleKind::Label => {
            // The label follows the pointer: the offset is the drag.
            c.label_offset = orig.label_offset + p.world.sub(start);
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
    Edited { cab: c, pushed }
}

/// Where the nearest wall or cabinet stops `orig` on each side along its
/// width: `(left, right)` offsets from its position; `None` where nothing
/// does. A resized edge snaps to them (the cabinet fills the gap).
fn edge_bounds(
    cx: &EditorContext,
    orig: &Cabinet,
    others: &[Cabinet],
) -> (Option<f64>, Option<f64>) {
    if orig.kind.is_custom() {
        return (None, None);
    }
    run_bounds(orig, others, &wall_polys(cx)).unwrap_or((None, None))
}

impl Tool for CabinetTool {
    fn id(&self) -> ToolId {
        ToolId::Cabinet
    }

    fn name(&self) -> &'static str {
        self.preset
            .map_or_else(|| self.kind.name(), CabinetPreset::name)
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
            CabinetKind::SoffitPolygon => {
                "Soffit Polygon: click the corners of the soffit (or drag a rectangle), Enter or the first corner finishes, Backspace removes a corner".to_string()
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
            // A library type requested with `request_preset` rides on the
            // variant of the kind it is built from.
            if let Some(p) = REQUESTED_PRESET.with(std::cell::Cell::take) {
                if p.kind() == k {
                    self.set_preset(Some(p));
                }
            }
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
                let mode = if e.op == HandleKind::Move {
                    bump_mode()
                } else {
                    BumpMode::Bump
                };
                let edited = apply_edit_mode(
                    cx,
                    mode,
                    e.op,
                    &e.original,
                    e.start,
                    &p,
                    e.snapshot.as_deref(),
                );
                let fl = cx.floor;
                // The neighbours the last step pushed go back first.
                if let Some(snap) = &e.snapshot {
                    for id in e.pushed.drain(..) {
                        if let Some(orig) = snap.iter().find(|c| c.id == id) {
                            replace_cabinet(&mut cx.project, fl, orig);
                        }
                    }
                }
                replace_cabinet(&mut cx.project, fl, &edited.cab);
                for q in &edited.pushed {
                    replace_cabinet(&mut cx.project, fl, q);
                }
                e.pushed = edited.pushed.iter().map(|q| q.id).collect();
                cx.mark_dirty();
                cx.readout = Some(format!("Width: {}", cx.fmt_dim(edited.cab.width)));
                return ToolResult::consumed();
            }
            let (kind, preset) = (self.kind, self.preset);
            if let Some(pr) = &mut self.press {
                let u = Point::new(pr.angle.cos(), pr.angle.sin());
                let du = p.world.sub(pr.start).dot(u);
                let draggable = supports_width_drag(kind);
                if draggable && (pr.dragged || (p.screen - pr.screen).length() >= DRAG_THRESHOLD_PX)
                {
                    pr.dragged = true;
                    let step = width_step(cx);
                    let width = ((du.abs() / step).round().max(1.0) * step).max(min_width(cx));
                    let sign = if du >= 0.0 { 1.0 } else { -1.0 };
                    let center = pr.start + u * (sign * width * 0.5);
                    let mut cab = self_new_cabinet(preset, cx, kind);
                    cab.width = width;
                    cab.angle = pr.angle;
                    settle(
                        cx,
                        &mut cab,
                        center,
                        center,
                        WALL_REACH,
                        true,
                        p.overrides(),
                        0,
                    );
                    cx.readout = Some(format!("Width: {}", cx.fmt_dim(width)));
                    pr.cab = cab.clone();
                    pr.room = true;
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
        // A temporary dimension of the selected cabinet turns into an edit
        // field (Enter applies it by moving or resizing the cabinet).
        if let Some(i) = cx.temp.hit_label(p.world, cx.px_per_in) {
            cx.temp.cancel();
            cx.temp.begin_edit(i);
            return ToolResult::consumed();
        }
        cx.temp.cancel();
        // A handle of the selected cabinet.
        if let (Some(id), Some(h)) = (
            Self::selected_cabinet(cx),
            hit_handle(&Self::handles(cx), p.world, tol),
        ) {
            if let Some(original) = cabinet_by_id(cx.floor(), id) {
                let snapshot = (h.kind == HandleKind::Move && bump_mode() == BumpMode::Push)
                    .then(|| load_cabinets(cx.floor()));
                self.edit = Some(EditDrag {
                    op: h.kind,
                    original,
                    start: p.world,
                    screen: p.screen,
                    begun: false,
                    snapshot,
                    pushed: Vec::new(),
                });
                return ToolResult::consumed();
            }
        }
        // Shift-click toggles a cabinet of this variant's height range. Any
        // other click places a new cabinet, which bumps against whatever is
        // under it; a selected cabinet moves with its center handle.
        if p.modifiers.shift {
            let probe = self.new_cabinet(cx);
            if let Some(id) = hit_cabinet(cx, p.world, 0.0, |c| vertical_overlap(c, &probe)) {
                cx.selection.toggle(ObjectRef::Cabinet(id));
                return ToolResult::consumed();
            }
        }
        // Otherwise a placement: committed on release (a click or a drag).
        let (cab, room) = self.placed_with_room(cx, &p);
        self.press = Some(Press {
            start: p.world,
            screen: p.screen,
            angle: cab.angle,
            cab: cab.clone(),
            dragged: false,
            room,
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
                placed::rejoin_if_enabled(cx);
                ToolResult::committed(op_label(e.op))
            } else {
                ToolResult::consumed()
            };
        }
        let Some(pr) = self.press.take() else {
            return ToolResult::ignored();
        };
        cx.readout = None;
        if !pr.room {
            self.ghost = None;
            cx.status = format!(
                "No cabinet placed: the space is narrower than the Minimum Cabinet Width ({})",
                cx.fmt_dim(min_width(cx))
            );
            return ToolResult::consumed();
        }
        let label = format!("Place {}", self.kind.name());
        cx.begin_change(&label);
        let fl = cx.floor;
        match add_cabinet(&mut cx.project, fl, pr.cab) {
            Some(id) => {
                cx.selection.set(ObjectRef::Cabinet(id));
                placed::rejoin_if_enabled(cx);
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
        if cx.temp.editing.is_some() {
            return Self::key_while_editing(cx, &k);
        }
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
            if k.modifiers.shift {
                // Shift+Tab walks the library types: Vanity, Pantry, Tall
                // Oven, Refrigerator, then back to the plain kinds.
                let next = match self.preset {
                    None => Some(CabinetPreset::ALL[0]),
                    Some(p) => CabinetPreset::ALL
                        .iter()
                        .position(|x| *x == p)
                        .and_then(|i| CabinetPreset::ALL.get(i + 1).copied()),
                };
                match next {
                    Some(p) => self.set_preset(Some(p)),
                    None => self.set_kind(KINDS[0]),
                }
            } else {
                let i = KINDS.iter().position(|x| *x == self.kind).unwrap_or(0);
                self.set_kind(KINDS[(i + 1) % KINDS.len()]);
            }
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
        let show_dims = cx.view_flags.contains(&ViewFlag::TemporaryDimensions);
        if let Some(g) = &self.ghost {
            if self.edit.is_none() {
                placed::draw_cabinet(painter, cam, g, pal.ghost_stroke, false);
                // The ghost's distances to the walls and neighbours on
                // either side, while it is placed or dragged out.
                if show_dims && !is_polygon(self.kind) && !g.kind.is_custom() {
                    let dims = TempDims {
                        dims: tempdim::cabinet_temp_dims(
                            cx.floor(),
                            g,
                            ObjectRef::Cabinet(0),
                            &tempdim::TempLocate::of(cx),
                        ),
                        editing: None,
                    };
                    tempdim::draw(&dims, painter, cam, pal, &cx.dim_format());
                }
            }
        }
        if show_dims && !is_polygon(self.kind) {
            tempdim::draw(&cx.temp, painter, cam, pal, &cx.dim_format());
        }
        if is_polygon(self.kind) {
            self.draw_poly(cx, painter, cam);
        } else {
            handles::draw(&Self::handles(cx), painter, cam, pal);
        }
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        let mut v = cx.common_edit_actions();
        if placed::selection_has_closed_polyline(cx) {
            v.push(EditAction::new(EditActionKind::Custom {
                id: placed::SOFFIT_FROM_POLYLINE,
                label: "Convert Polyline to Soffit",
                icon: "",
            }));
        }
        // Bumping/Pushing: shows the mode, a click goes to the next.
        v.push(EditAction::new(EditActionKind::Custom {
            id: BUMP_MODE_COMMAND,
            label: bump_mode().toolbar_label(),
            icon: "",
        }));
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
        // Set as Default copies one standard cabinet into the defaults of its
        // kind (not for special shapes).
        if let Some(c) = Self::selected_cabinet(cx).and_then(|id| cabinet_by_id(cx.floor(), id)) {
            if defaults_name(c.kind).is_some() && c.preset.is_none() && c.custom.is_none() {
                v.push(EditAction::new(EditActionKind::Custom {
                    id: SET_AS_DEFAULT_COMMAND,
                    label: "Set as Default",
                    icon: "",
                }));
            }
        }
        v
    }
}

impl CabinetTool {
    /// Typing into a temporary dimension: digits and quotes edit the value,
    /// Tab moves to the next, Enter applies, Esc leaves it.
    fn key_while_editing(cx: &mut EditorContext, k: &KeyEvent) -> ToolResult {
        if let Some(t) = &k.text {
            let ok: String = t
                .chars()
                .filter(|c| c.is_ascii_digit() || " '\"-/.".contains(*c))
                .collect();
            cx.temp.type_text(&ok);
        } else if k.is(Key::Backspace) {
            cx.temp.backspace();
        } else if k.is(Key::Tab) {
            cx.temp.next_field();
        } else if k.is(Key::Escape) {
            cx.temp.cancel();
        } else if k.is(Key::Enter) {
            match tempdim::commit_edit(cx) {
                Ok(label) => return ToolResult::committed(label),
                Err(e) => cx.status = e,
            }
        }
        ToolResult::consumed()
    }

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

// ----- Set as Default and dynamic defaults (CB-475) -----

/// Command id of the Edit-toolbar Set as Default button.
pub const SET_AS_DEFAULT_COMMAND: &str = "cabinet.set_as_default";

/// The name a cabinet kind has in Default Settings, for the kinds that have
/// a defaults dialog.
fn defaults_name(kind: CabinetKind) -> Option<&'static str> {
    Some(match kind {
        CabinetKind::Base => "Base Cabinet",
        CabinetKind::Wall => "Wall Cabinet",
        CabinetKind::FullHeight => "Full Height Cabinet",
        CabinetKind::Soffit => "Soffit",
        CabinetKind::Shelf => "Shelf",
        CabinetKind::Partition => "Partition",
        _ => return None,
    })
}

/// Set as Default: the attributes of the selected cabinet become the defaults
/// of its kind (manual p. 644; not for special shapes: only standard base,
/// wall and full height cabinets, soffits, shelves and partitions). Nothing
/// is written to the plan, so there is no undo step. True when the defaults
/// changed.
pub fn set_as_default(cx: &mut EditorContext) -> bool {
    let Some(id) = CabinetTool::selected_cabinet(cx) else {
        cx.status = "Select one cabinet to set the defaults from".into();
        return false;
    };
    let all = placed::load_cabinets(cx.floor());
    let logical = without_generated_tops(&all);
    let Some(c) = logical.into_iter().find(|c| c.id == id) else {
        return false;
    };
    let Some(name) = defaults_name(c.kind).filter(|_| c.preset.is_none() && c.custom.is_none())
    else {
        cx.status = "Set as Default is only available for standard cabinets".into();
        return false;
    };
    let d = &mut cx.defaults.cabinets;
    let boxed = |d: &mut plan_core::defaults::BoxDefaults| {
        d.width = c.width;
        d.depth = c.depth;
        d.height = c.height;
        d.elevation = c.elevation;
    };
    match c.kind {
        CabinetKind::Base => {
            d.base.width = c.width;
            d.base.depth = c.depth;
            d.base.height = c.height;
            if let Some(t) = c.countertop {
                d.base.countertop_thickness = t.thickness;
                d.base.countertop_overhang = t.overhang_front;
                d.countertop = plan_core::defaults::CountertopDefaults {
                    overhang_sides: t.overhang_sides,
                    overhang_back: t.overhang_back,
                    edge: t.edge.name().into(),
                    edge_size: t.edge_size,
                    corner: t.corner.name().into(),
                    corner_size: t.corner_size,
                };
            }
            if let Some(k) = c.toe_kick {
                d.base.toe_kick_height = k.height;
                d.base.toe_kick_depth = k.depth;
            }
            d.base.door_style = c.door_style.name.clone();
            d.base.drawer_style = c.drawer_style.name.clone();
            d.base.handle = c.door_style.handle.name().into();
            d.backsplash = match c.backsplash {
                Some(b) => plan_core::defaults::BacksplashDefaults {
                    enabled: true,
                    height: b.height,
                    thickness: b.thickness,
                    full_height: b.full_height,
                },
                None => plan_core::defaults::BacksplashDefaults {
                    enabled: false,
                    ..d.backsplash.clone()
                },
            };
        }
        CabinetKind::Wall => {
            d.wall.width = c.width;
            d.wall.depth = c.depth;
            d.wall.height = c.height;
            d.wall.elevation = c.elevation;
        }
        CabinetKind::FullHeight => {
            d.full_height.width = c.width;
            d.full_height.depth = c.depth;
            d.full_height.height = c.height;
        }
        CabinetKind::Soffit => boxed(&mut d.soffit),
        CabinetKind::Shelf => boxed(&mut d.shelf),
        CabinetKind::Partition => boxed(&mut d.partition),
        _ => {}
    }
    cx.mark_dirty();
    cx.status = format!("{name} Defaults have been updated");
    true
}

/// Dynamic cabinet defaults (manual p. 644): after the Cabinet Defaults
/// changed from `old` to the plan's current ones, every standard base cabinet
/// that still has an old default value (countertop thickness and overhangs,
/// toe kick height and depth, backsplash height and thickness) takes the new
/// one. A cabinet that was set to some other value keeps it. The cabinets that
/// gave up their slab to a generated countertop follow through the sources the
/// top remembers. One undo step; returns how many cabinets changed.
pub fn apply_dynamic_defaults(
    cx: &mut EditorContext,
    old: &plan_core::defaults::CabinetDefaults,
) -> usize {
    let new = cx.defaults.cabinets.clone();
    if &new == old {
        return 0;
    }
    cx.begin_change("Cabinet Defaults");
    let mut changed = 0;
    for fi in 0..cx.project.floors.len() {
        for mut c in placed::load_cabinets(&cx.project.floors[fi]) {
            if c.preset.is_some() || (c.custom.is_some() && c.joined.is_empty()) {
                continue;
            }
            let mut touched = false;
            if c.kind == CabinetKind::Base {
                if let Some(t) = c.countertop.as_mut() {
                    let before = t.thickness;
                    touched |= follow_top(t, old, &new);
                    c.height += t.thickness - before;
                }
                if let Some(k) = c.toe_kick.as_mut() {
                    touched |= follow(
                        &mut k.height,
                        old.base.toe_kick_height,
                        new.base.toe_kick_height,
                    );
                    touched |= follow(
                        &mut k.depth,
                        old.base.toe_kick_depth,
                        new.base.toe_kick_depth,
                    );
                }
                if let Some(b) = c.backsplash.as_mut() {
                    touched |= follow_backsplash(b, old, &new);
                }
            }
            // The slabs a generated top took from base cabinets.
            for j in &mut c.joined {
                touched |= follow_top(&mut j.countertop, old, &new);
                if let Some(b) = j.backsplash.as_mut() {
                    touched |= follow_backsplash(b, old, &new);
                }
            }
            if touched && replace_cabinet(&mut cx.project, fi, &c) {
                changed += 1;
            }
        }
    }
    if changed == 0 {
        cx.cancel_change();
        return 0;
    }
    placed::rejoin_if_enabled(cx);
    cx.mark_dirty();
    changed
}

/// `value` takes `now` when it still has the old default `was`.
fn follow(value: &mut f64, was: f64, now: f64) -> bool {
    if (*value - was).abs() < 1e-6 && (was - now).abs() >= 1e-6 {
        *value = now;
        true
    } else {
        false
    }
}

fn follow_top(
    t: &mut plan_cabinets::Countertop,
    old: &plan_core::defaults::CabinetDefaults,
    new: &plan_core::defaults::CabinetDefaults,
) -> bool {
    let mut touched = follow(
        &mut t.thickness,
        old.base.countertop_thickness,
        new.base.countertop_thickness,
    );
    touched |= follow(
        &mut t.overhang_front,
        old.base.countertop_overhang,
        new.base.countertop_overhang,
    );
    touched |= follow(
        &mut t.overhang_sides,
        old.countertop.overhang_sides,
        new.countertop.overhang_sides,
    );
    touched |= follow(
        &mut t.overhang_back,
        old.countertop.overhang_back,
        new.countertop.overhang_back,
    );
    touched
}

fn follow_backsplash(
    b: &mut Backsplash,
    old: &plan_core::defaults::CabinetDefaults,
    new: &plan_core::defaults::CabinetDefaults,
) -> bool {
    let mut touched = follow(&mut b.height, old.backsplash.height, new.backsplash.height);
    touched |= follow(
        &mut b.thickness,
        old.backsplash.thickness,
        new.backsplash.thickness,
    );
    touched
}

/// Is every record of the floor's cabinet list readable? (A foreign record is
/// kept untouched by every cabinet edit.)
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
    fn a_library_type_entry_asks_for_the_type_and_starts_the_tool() {
        let mut cx = setup();
        assert!(!run_preset_command(&mut cx, "cabinet.nope"));
        assert!(run_preset_command(
            &mut cx,
            preset_command(CabinetPreset::Pantry)
        ));
        assert!(matches!(
            cx.requests.last(),
            Some(EditorRequest::SetTool(ToolId::CabinetVariant(k))) if *k == CabinetPreset::Pantry.kind()
        ));
        let mut t = CabinetTool::default();
        t.set_variant(ToolId::CabinetVariant(CabinetPreset::Pantry.kind()));
        assert_eq!(t.preset(), Some(CabinetPreset::Pantry));
    }

    #[test]
    fn the_cabinet_flyout_has_no_unimplemented_entries() {
        let fly = crate::toolbar::cabinet();
        let mut kinds = Vec::new();
        let mut presets = Vec::new();
        for e in &fly.entries {
            match e.action {
                crate::toolbar::Action::SetTool(ToolId::CabinetVariant(k)) => kinds.push(k),
                // The library types (Vanity, Pantry, ...) start the tool through
                // `request_preset` (integration queue, cabinets round).
                crate::toolbar::Action::Custom(id) => presets.push(id),
                ref other => panic!("{} is not a tool entry: {other:?}", e.name),
            }
        }
        for p in CabinetPreset::ALL {
            assert!(presets.contains(&preset_command(p)), "{p:?} is missing");
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
    fn a_soffit_polygon_is_drawn_by_clicking_its_corners() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        t.set_kind(CabinetKind::SoffitPolygon);
        for (x, y) in [
            (0.0, 0.0),
            (60.0, 0.0),
            (60.0, 12.0),
            (12.0, 12.0),
            (12.0, 40.0),
        ] {
            click_clear(&mut t, &mut cx, x, y);
        }
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Place Soffit Polygon"));
        let list = cabs(&cx);
        assert_eq!(list.len(), 1);
        let so = &list[0];
        assert_eq!(so.kind, CabinetKind::Soffit, "stored as an ordinary soffit");
        assert_eq!(so.custom.as_ref().unwrap().outline.len(), 5);
        assert_eq!(so.elevation, 84.0);
        assert!(!plan_cabinets::meshes(so).is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Place Soffit Polygon"));
        assert!(cabs(&cx).is_empty());
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

    // ----- depth and corner handles, fit to gap, automatic countertop join -----

    /// Drags the handle `kind` of cabinet `id` to `to` through the tool.
    fn drag_handle_to(
        t: &mut CabinetTool,
        cx: &mut EditorContext,
        id: Id,
        kind: HandleKind,
        to: Point,
    ) -> ToolResult {
        let h = placed_handles(cx.floor(), PlacedRef::Cabinet(id), cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == kind)
            .unwrap_or_else(|| panic!("no {kind:?} handle"));
        let mut down = PointerEvent::at(cx, h.pos).with_down(true);
        down.screen = Pos2::new(0.0, 0.0);
        t.pointer_down(cx, down);
        let mut mv = PointerEvent::at(cx, to).with_down(true);
        mv.screen = Pos2::new(50.0, 50.0);
        t.pointer_move(cx, mv);
        t.pointer_up(cx, mv)
    }

    #[test]
    fn depth_handles_resize_from_the_front_and_the_back() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0); // 48..72, back on the wall face y = 3
        let id = cabs(&cx)[0].id;
        let front = HandleKind::Reshape(placed::DEPTH_FRONT);
        let back = HandleKind::Reshape(placed::DEPTH_BACK);
        // Front handle 6" out: the back stays on the wall.
        let r = drag_handle_to(&mut t, &mut cx, id, front, Point::new(60.0, 33.0));
        assert_eq!(r.commit.as_deref(), Some("Resize Cabinet"));
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!((c.depth, c.width), (30.0, 24.0));
        assert!((c.position.y - 3.0).abs() < 1e-9);
        // Back handle 3" in: the front edge (y = 33) stays put.
        drag_handle_to(&mut t, &mut cx, id, back, Point::new(60.0, 6.0));
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!(c.depth, 27.0);
        assert!((c.position.y - 6.0).abs() < 1e-9 && (c.position.y + c.depth - 33.0).abs() < 1e-9);
        // Never thinner than 3".
        drag_handle_to(&mut t, &mut cx, id, front, Point::new(60.0, -50.0));
        assert_eq!(cabinet_by_id(cx.floor(), id).unwrap().depth, 3.0);
        assert_eq!(cx.undo().as_deref(), Some("Resize Cabinet"));
    }

    #[test]
    fn corner_handles_change_width_and_depth_with_the_opposite_corner_fixed() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0); // 48..72 x 3..27
        let id = cabs(&cx)[0].id;
        let corner = |n| HandleKind::Reshape(n);
        // Front-right corner: the back-left corner (48, 3) stays.
        drag_handle_to(
            &mut t,
            &mut cx,
            id,
            corner(placed::CORNER_FRONT_RIGHT),
            Point::new(81.0, 33.0),
        );
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!((c.width, c.depth), (33.0, 30.0));
        assert!(c.position.dist(Point::new(48.0, 3.0)) < 1e-9);
        // Back-left corner: the front-right corner (81, 33) stays.
        drag_handle_to(
            &mut t,
            &mut cx,
            id,
            corner(placed::CORNER_BACK_LEFT),
            Point::new(39.0, 0.0),
        );
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!((c.width, c.depth), (42.0, 33.0));
        let front_right = c.to_plan(Point::new(c.width, c.depth));
        assert!(
            front_right.dist(Point::new(81.0, 33.0)) < 1e-9,
            "{front_right:?}"
        );
        // The other two corners work the same way.
        drag_handle_to(
            &mut t,
            &mut cx,
            id,
            corner(placed::CORNER_BACK_RIGHT),
            Point::new(60.0, 6.0),
        );
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert!(
            c.to_plan(Point::new(0.0, c.depth))
                .dist(Point::new(39.0, 33.0))
                < 1e-9
        );
        assert_eq!((c.width, c.depth), (21.0, 27.0));
        drag_handle_to(
            &mut t,
            &mut cx,
            id,
            corner(placed::CORNER_FRONT_LEFT),
            Point::new(45.0, 30.0),
        );
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert!(
            c.to_plan(Point::new(c.width, 0.0))
                .dist(Point::new(60.0, 6.0))
                < 1e-9
        );
    }

    #[test]
    fn dragging_a_cabinet_into_a_gap_close_to_its_width_fits_it() {
        let mut cx = setup();
        // A at 20..44 and C at 75..99, both on the wall face: a 31" gap.
        put_kind(&mut cx, CabinetKind::Base, 20.0, 3.0, 0.0);
        put_kind(&mut cx, CabinetKind::Base, 75.0, 3.0, 0.0);
        let mut b = default_cabinet(&cx, CabinetKind::Base);
        b.width = 30.0;
        b.position = Point::new(120.0, 3.0);
        let id = add_cabinet(&mut cx.project, 0, b).unwrap();
        let orig = cabinet_by_id(cx.floor(), id).unwrap();
        let start = orig.to_plan(Point::new(15.0, 12.0));
        let to = |x: f64| PointerEvent::at(&cx, Point::new(x, start.y));
        let moved = apply_edit(&cx, HandleKind::Move, &orig, start, &to(59.0));
        assert!((moved.width - 31.0).abs() < 1e-9, "{}", moved.width);
        assert!(
            (moved.position.x - 44.0).abs() < 1e-9,
            "{:?}",
            moved.position
        );
        // The preference switches it off.
        set_fit_to_gap(false);
        let plain = apply_edit(&cx, HandleKind::Move, &orig, start, &to(59.0));
        set_fit_to_gap(true);
        assert_eq!(plain.width, 30.0);
        // A 24" cabinet is 7" short of the gap: it keeps its width.
        let mut small = orig.clone();
        small.width = 24.0;
        let kept = apply_edit(&cx, HandleKind::Move, &small, start, &to(59.0));
        assert_eq!(kept.width, 24.0);
        // Resizing by a handle is not a gap fit.
        let wide = apply_edit(
            &cx,
            HandleKind::ResizeEnd,
            &orig,
            start,
            &PointerEvent::at(&cx, Point::new(160.0, 10.0)),
        );
        assert_eq!(wide.width, 39.0);
    }

    #[test]
    fn touching_base_cabinets_join_their_countertops_and_regenerate_on_a_move() {
        let mut cx = setup();
        placed::set_auto_join(true);
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0); // 48..72
        assert!(
            cabs(&cx).iter().all(|c| c.joined.is_empty()),
            "one cabinet joins nothing"
        );
        click_clear(&mut t, &mut cx, 84.0, 10.0); // butts against it: 72..96
        let list = cabs(&cx);
        let tops: Vec<&Cabinet> = list.iter().filter(|c| !c.joined.is_empty()).collect();
        assert_eq!(tops.len(), 1, "{list:?}");
        assert_eq!(tops[0].joined.len(), 2);
        let top_id = tops[0].id;
        let bases: Vec<&Cabinet> = list.iter().filter(|c| c.joined.is_empty()).collect();
        assert!(bases
            .iter()
            .all(|c| c.countertop.is_none() && c.height < 36.0));
        // Placing and joining were one undo step.
        assert_eq!(cx.undo_label(), Some("Place Base Cabinet"));
        // Move the second cabinet away with its handle: the top comes apart.
        let second = bases[1].id;
        drag_handle_to(
            &mut t,
            &mut cx,
            second,
            HandleKind::Move,
            Point::new(180.0, 18.0),
        );
        let list = cabs(&cx);
        assert!(
            list.iter().all(|c| c.joined.is_empty()),
            "no top while apart"
        );
        assert!(list
            .iter()
            .all(|c| c.countertop.is_some() && c.height == 36.0));
        // Bring it back beside the first: a top again (the old id is gone).
        drag_handle_to(
            &mut t,
            &mut cx,
            second,
            HandleKind::Move,
            Point::new(84.0, 18.0),
        );
        let list = cabs(&cx);
        let again: Vec<&Cabinet> = list.iter().filter(|c| !c.joined.is_empty()).collect();
        assert_eq!(again.len(), 1);
        assert_ne!(again[0].id, top_id);
        // Deleting a joined cabinet gives the other its slab back.
        cx.selection.set(ObjectRef::Cabinet(second));
        assert_eq!(placed::delete_placed(&mut cx), 1);
        let list = cabs(&cx);
        assert_eq!(list.len(), 1);
        assert!(list[0].countertop.is_some() && list[0].joined.is_empty());
        placed::set_auto_join(false);
    }

    // ----- placement sizing, library types, defaults, labels, temp dims -----

    #[test]
    fn a_cabinet_placed_into_a_34_inch_gap_takes_the_gap() {
        let mut cx = setup();
        // A at 20..44 and C at 78..102 on the wall face: a 34" gap.
        put_kind(&mut cx, CabinetKind::Base, 20.0, 3.0, 0.0);
        put_kind(&mut cx, CabinetKind::Base, 78.0, 3.0, 0.0);
        cx.defaults.cabinets.base.width = 36.0;
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 61.0, 10.0);
        let placed = cabs(&cx).into_iter().find(|c| c.width != 24.0).unwrap();
        assert!((placed.width - 34.0).abs() < 1e-9, "{}", placed.width);
        assert!(
            (placed.position.x - 44.0).abs() < 1e-9,
            "{:?}",
            placed.position
        );
        // A gap further off than the tolerance is left alone: 30" in a 34" gap.
        let mut cx = setup();
        put_kind(&mut cx, CabinetKind::Base, 20.0, 3.0, 0.0);
        put_kind(&mut cx, CabinetKind::Base, 78.0, 3.0, 0.0);
        cx.defaults.cabinets.base.width = 30.0;
        let t = CabinetTool::default();
        let p = PointerEvent::at(&cx, Point::new(61.0, 10.0));
        assert_eq!(t.placed_at(&cx, &p).width, 30.0);
        // A wider tolerance in the preferences closes it; Alt never fits.
        set_fit_tolerance(5.0);
        let mut alt = p;
        alt.modifiers.ctrl = true;
        assert_eq!(t.placed_at(&cx, &alt).width, 30.0);
        assert!((t.placed_at(&cx, &p).width - 34.0).abs() < 1e-9);
        set_fit_tolerance(FIT_TOLERANCE);
        // The preference off places it as it is.
        set_fit_to_gap(false);
        assert_eq!(t.placed_at(&cx, &p).width, 30.0);
        set_fit_to_gap(true);
        // Fillers, soffits and the like are not fitted this way.
        assert!(!fills_gaps(CabinetKind::Soffit) && fills_gaps(CabinetKind::FullHeight));
    }

    #[test]
    fn the_defaults_give_each_kind_its_chief_size() {
        let cx = setup();
        let b = default_cabinet(&cx, CabinetKind::Base);
        assert_eq!((b.width, b.depth, b.height), (24.0, 24.0, 36.0));
        let s = default_cabinet(&cx, CabinetKind::Soffit);
        assert_eq!((s.depth, s.height, s.elevation), (12.0, 12.0, 84.0));
        let sh = default_cabinet(&cx, CabinetKind::Shelf);
        assert_eq!((sh.depth, sh.height, sh.elevation), (12.0, 0.75, 48.0));
        let pt = default_cabinet(&cx, CabinetKind::Partition);
        assert_eq!((pt.depth, pt.height), (24.0, 36.0));
        for (p, size) in [
            (CabinetPreset::Vanity, (30.0, 21.0, 34.5)),
            (CabinetPreset::Pantry, (24.0, 24.0, 84.0)),
            (CabinetPreset::TallOven, (30.0, 24.0, 84.0)),
            (CabinetPreset::Refrigerator, (36.0, 25.0, 84.0)),
        ] {
            let c = default_preset_cabinet(&cx, p);
            assert_eq!((c.width, c.depth, c.height), size, "{p:?}");
            assert_eq!(c.preset, Some(p));
        }
        // The Cabinet Defaults page edits them.
        let mut cx = setup();
        cx.defaults.cabinets.vanity.depth = 18.0;
        cx.defaults.cabinets.soffit.elevation = 90.0;
        cx.defaults.cabinets.shelf.width = 36.0;
        assert_eq!(
            default_preset_cabinet(&cx, CabinetPreset::Vanity).depth,
            18.0
        );
        assert_eq!(default_cabinet(&cx, CabinetKind::Soffit).elevation, 90.0);
        assert_eq!(default_cabinet(&cx, CabinetKind::Shelf).width, 36.0);
    }

    #[test]
    fn countertop_and_backsplash_defaults_dress_new_base_cabinets() {
        let mut cx = setup();
        assert!(default_cabinet(&cx, CabinetKind::Base).backsplash.is_none());
        cx.defaults.cabinets.backsplash.enabled = true;
        cx.defaults.cabinets.backsplash.full_height = true;
        cx.defaults.cabinets.backsplash.height = 6.0;
        cx.defaults.cabinets.countertop.edge = "Bullnose".into();
        cx.defaults.cabinets.countertop.edge_size = 1.0;
        cx.defaults.cabinets.countertop.corner = "Rounded".into();
        cx.defaults.cabinets.countertop.overhang_sides = 0.5;
        cx.defaults.cabinets.base.handle = "Cup Pull".into();
        let c = default_cabinet(&cx, CabinetKind::Base);
        let bs = c.backsplash.unwrap();
        assert!(bs.full_height && bs.height == 6.0);
        let t = c.countertop.unwrap();
        assert_eq!(t.edge, EdgeProfile::Bullnose);
        assert_eq!((t.edge_size, t.overhang_sides), (1.0, 0.5));
        assert_eq!(t.corner, CornerTreatment::Rounded);
        assert_eq!(c.door_style.handle, HandleStyle::Cup);
        // A wall cabinet has no top to carry one.
        assert!(default_cabinet(&cx, CabinetKind::Wall).backsplash.is_none());
        // The vanity takes the backsplash too.
        assert!(default_preset_cabinet(&cx, CabinetPreset::Vanity)
            .backsplash
            .is_some());
    }

    #[test]
    fn the_tool_places_library_types_and_shift_tab_walks_them() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        t.set_preset(Some(CabinetPreset::Vanity));
        assert_eq!(t.name(), "Vanity Cabinet");
        click(&mut t, &mut cx, 60.0, 10.0);
        let v = &cabs(&cx)[0];
        assert_eq!(v.preset, Some(CabinetPreset::Vanity));
        assert_eq!((v.depth, v.height), (21.0, 34.5));
        assert_eq!(v.display_label(), "VB30");
        // Shift+Tab: Pantry, Tall Oven, Refrigerator, then the plain kinds.
        let shift_tab = || {
            let mut k = KeyEvent::key(Key::Tab);
            k.modifiers.shift = true;
            k
        };
        for want in [
            CabinetPreset::Pantry,
            CabinetPreset::TallOven,
            CabinetPreset::Refrigerator,
        ] {
            assert!(t.key(&mut cx, shift_tab()).consumed);
            assert_eq!(t.preset(), Some(want));
        }
        assert!(t.key(&mut cx, shift_tab()).consumed);
        assert_eq!((t.preset(), t.kind()), (None, KINDS[0]));
        assert!(t.key(&mut cx, shift_tab()).consumed);
        assert_eq!(t.preset(), Some(CabinetPreset::Vanity));
        // Tab goes back to plain kinds.
        t.key(&mut cx, KeyEvent::key(Key::Tab));
        assert_eq!(t.preset(), None);
        // A toolbar entry requests the type before choosing the variant.
        request_preset(CabinetPreset::Pantry);
        t.set_variant(ToolId::CabinetVariant(CabinetKind::FullHeight));
        assert_eq!(t.preset(), Some(CabinetPreset::Pantry));
        t.set_variant(ToolId::CabinetVariant(CabinetKind::FullHeight));
        assert_eq!(t.preset(), None, "the request is used once");
    }

    #[test]
    fn dragging_the_label_handle_moves_the_label_on_its_layer() {
        let mut cx = setup();
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 60.0, 10.0);
        let id = cabs(&cx)[0].id;
        assert!(
            cx.project.layers.get("Cabinets, Labels").is_some(),
            "the label layer appears with the first cabinet"
        );
        drag_handle_to(&mut t, &mut cx, id, HandleKind::Label, Point::ZERO);
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        // The drag started on the handle and ended at the origin: the label
        // moved by exactly that drag.
        let h = placed::placed_handles(cx.floor(), PlacedRef::Cabinet(id), cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == HandleKind::Label)
            .unwrap();
        assert_ne!(c.label_offset, Point::ZERO);
        assert!(h.pos.dist(Point::ZERO) < 40.0, "{:?}", h.pos);
        assert_eq!(cx.undo_label(), Some("Move Cabinet Label"));
    }

    #[test]
    fn clicking_a_temporary_dimension_edits_it_and_enter_moves_the_cabinet() {
        let mut cx = setup();
        cx.view_flags.insert(ViewFlag::TemporaryDimensions);
        vertical_wall(&mut cx, 0.0);
        vertical_wall(&mut cx, 54.0);
        let mut t = CabinetTool::default();
        click(&mut t, &mut cx, 27.0, 10.0);
        cx.refresh();
        let id = cabs(&cx)[0].id;
        let i = cx
            .temp
            .dims
            .iter()
            .position(|d| d.kind == tempdim::TempDimKind::CabinetToLeft)
            .expect("a gap to the left");
        let gap = cx.temp.dims[i].value;
        let at = cx.temp.dims[i].label_pos(cx.px_per_in);
        let p = PointerEvent::at(&cx, at);
        assert!(t.pointer_down(&mut cx, p.with_down(true)).consumed);
        assert!(cx.temp.editing.is_some());
        cx.temp.editing.as_mut().unwrap().text.clear();
        assert!(t.key(&mut cx, KeyEvent::text("6")).consumed);
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Move Cabinet"));
        let c = cabinet_by_id(cx.floor(), id).unwrap();
        assert!(gap > 6.0);
        cx.refresh();
        let left = cx
            .temp
            .dims
            .iter()
            .find(|d| d.kind == tempdim::TempDimKind::CabinetToLeft)
            .unwrap();
        assert!((left.value - 6.0).abs() < 1e-9, "{}", left.value);
        assert!(c.width == 24.0);
    }

    // ----- Bumping/Pushing, edge snapping, Convert Polyline to Soffit (round 14) -----

    /// Presses the handle `kind` of cabinet `id`, moves through `path` and
    /// releases at its end; the pointer positions are plan points.
    fn drag_through(
        t: &mut CabinetTool,
        cx: &mut EditorContext,
        id: Id,
        kind: HandleKind,
        path: &[Point],
    ) -> ToolResult {
        let h = placed_handles(cx.floor(), PlacedRef::Cabinet(id), cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == kind)
            .unwrap_or_else(|| panic!("no {kind:?} handle"));
        let mut down = PointerEvent::at(cx, h.pos).with_down(true);
        down.screen = Pos2::new(0.0, 0.0);
        t.pointer_down(cx, down);
        let mut last = down;
        for (i, p) in path.iter().enumerate() {
            let mut mv = PointerEvent::at(cx, *p).with_down(true);
            mv.screen = Pos2::new(50.0 + 10.0 * i as f32, 50.0);
            t.pointer_move(cx, mv);
            last = mv;
        }
        t.pointer_up(cx, last)
    }

    fn base_at(cx: &mut EditorContext, x: f64, width: f64) -> Id {
        let mut c = default_cabinet(cx, CabinetKind::Base);
        c.width = width;
        c.countertop = None;
        put(cx, c, x, 3.0, 0.0)
    }

    fn x_of(cx: &EditorContext, id: Id) -> f64 {
        cabinet_by_id(cx.floor(), id).unwrap().position.x
    }

    #[test]
    fn pushing_moves_the_run_ahead_of_a_dragged_cabinet_and_lets_it_back() {
        set_bump_mode(BumpMode::Push);
        placed::set_auto_join(false);
        let mut cx = setup();
        let a = base_at(&mut cx, 20.0, 24.0);
        let b = base_at(&mut cx, 44.0, 24.0);
        let d = base_at(&mut cx, 120.0, 24.0);
        cx.selection.set(ObjectRef::Cabinet(d));
        let mut t = CabinetTool::default();
        // D's centre handle moves 60" left (from 132 to 72): D ends at 60..84?
        // Drag so D's left edge lands at 60: a 60" move left.
        let y = 15.0;
        let start = cabinet_by_id(cx.floor(), d)
            .unwrap()
            .to_plan(Point::new(12.0, 12.0));
        let left60 = Point::new(start.x - 60.0, y);
        let back = Point::new(start.x, y);
        // Out and back in one drag: the pushed cabinets return.
        let r = drag_through(&mut t, &mut cx, d, HandleKind::Move, &[left60, back]);
        assert_eq!(r.commit.as_deref(), Some("Move Cabinet"));
        assert_eq!(
            (x_of(&cx, a), x_of(&cx, b), x_of(&cx, d)),
            (20.0, 44.0, 120.0)
        );
        // Out and stay: D at 60..84 pushes B (44..68) to 36..60 and A to 12..36.
        let r = drag_through(&mut t, &mut cx, d, HandleKind::Move, &[left60]);
        assert_eq!(r.commit.as_deref(), Some("Move Cabinet"));
        assert_eq!(x_of(&cx, d), 60.0);
        assert_eq!(x_of(&cx, b), 36.0);
        assert_eq!(x_of(&cx, a), 12.0);
        // One undo step puts all three back.
        assert_eq!(cx.undo().as_deref(), Some("Move Cabinet"));
        assert_eq!(
            (x_of(&cx, a), x_of(&cx, b), x_of(&cx, d)),
            (20.0, 44.0, 120.0)
        );
        set_bump_mode(BumpMode::Bump);
    }

    #[test]
    fn a_wall_that_stops_the_pushed_run_makes_the_cabinet_bump_instead() {
        set_bump_mode(BumpMode::Push);
        placed::set_auto_join(false);
        let mut cx = setup();
        vertical_wall(&mut cx, 84.0); // faces at x = 81 and 87
        let b = base_at(&mut cx, 30.0, 24.0);
        let c = base_at(&mut cx, 54.0, 24.0);
        let d = base_at(&mut cx, -80.0, 24.0);
        cx.selection.set(ObjectRef::Cabinet(d));
        let mut t = CabinetTool::default();
        let start = cabinet_by_id(cx.floor(), d)
            .unwrap()
            .to_plan(Point::new(12.0, 12.0));
        // D to 18..42 would push B and C into the wall: it stops against B.
        let to = Point::new(start.x + 98.0, 15.0);
        drag_through(&mut t, &mut cx, d, HandleKind::Move, &[to]);
        assert_eq!(x_of(&cx, b), 30.0);
        assert_eq!(x_of(&cx, c), 54.0);
        assert_eq!(x_of(&cx, d), 6.0, "butted against B");
        set_bump_mode(BumpMode::Bump);
    }

    #[test]
    fn bump_mode_stops_and_off_passes_through() {
        placed::set_auto_join(false);
        for (mode, expect) in [(BumpMode::Bump, 68.0), (BumpMode::Off, 50.0)] {
            set_bump_mode(mode);
            let mut cx = setup();
            let b = base_at(&mut cx, 44.0, 24.0);
            let d = base_at(&mut cx, 120.0, 24.0);
            cx.selection.set(ObjectRef::Cabinet(d));
            let mut t = CabinetTool::default();
            let start = cabinet_by_id(cx.floor(), d)
                .unwrap()
                .to_plan(Point::new(12.0, 12.0));
            drag_through(
                &mut t,
                &mut cx,
                d,
                HandleKind::Move,
                &[Point::new(start.x - 70.0, 15.0)],
            );
            assert_eq!(x_of(&cx, b), 44.0, "{mode:?}: B stays");
            assert_eq!(x_of(&cx, d), expect, "{mode:?}");
        }
        set_bump_mode(BumpMode::Bump);
    }

    #[test]
    fn the_edit_toolbar_cycles_the_bumping_mode() {
        set_bump_mode(BumpMode::Bump);
        let mut cx = setup();
        let t = CabinetTool::default();
        let label = |cx: &EditorContext| {
            t.edit_toolbar(cx)
                .into_iter()
                .find(|a| matches!(a.kind, EditActionKind::Custom { id, .. } if id == BUMP_MODE_COMMAND))
                .map(|a| a.label)
        };
        assert_eq!(label(&cx), Some("Neighbors: Bump"));
        cx.run_custom(BUMP_MODE_COMMAND);
        assert_eq!(bump_mode(), BumpMode::Push);
        assert_eq!(label(&cx), Some("Neighbors: Push"));
        cx.run_custom(BUMP_MODE_COMMAND);
        assert_eq!(bump_mode(), BumpMode::Off);
        cx.run_custom(BUMP_MODE_COMMAND);
        assert_eq!(bump_mode(), BumpMode::Bump);
        assert!(cx.status.contains("Bump"), "{}", cx.status);
    }

    #[test]
    fn a_resized_edge_dragged_near_a_wall_or_cabinet_snaps_to_it() {
        placed::set_auto_join(false);
        set_fit_to_gap(false);
        let mut cx = setup();
        vertical_wall(&mut cx, 65.0); // left face at x = 62
        let a = base_at(&mut cx, 20.0, 24.0);
        let mut t = CabinetTool::default();
        cx.selection.set(ObjectRef::Cabinet(a));
        // The end handle dragged to 59 (3" off the wall): 42" wide, flush.
        drag_handle_to(
            &mut t,
            &mut cx,
            a,
            HandleKind::ResizeEnd,
            Point::new(59.0, 10.0),
        );
        let c = cabinet_by_id(cx.floor(), a).unwrap();
        assert_eq!(c.width, 42.0);
        assert_eq!(c.position.x, 20.0);
        // 12" off the wall it keeps the 3" grid.
        cx.undo();
        drag_handle_to(
            &mut t,
            &mut cx,
            a,
            HandleKind::ResizeEnd,
            Point::new(50.0, 10.0),
        );
        assert_eq!(cabinet_by_id(cx.floor(), a).unwrap().width, 30.0);
        // The start handle snaps to a cabinet on its left.
        cx.undo();
        let n = base_at(&mut cx, -10.0, 24.0); // -10..14
        cx.selection.set(ObjectRef::Cabinet(a));
        drag_handle_to(
            &mut t,
            &mut cx,
            a,
            HandleKind::ResizeStart,
            Point::new(17.0, 10.0),
        );
        let c = cabinet_by_id(cx.floor(), a).unwrap();
        assert_eq!((c.position.x, c.width), (14.0, 30.0), "grew to touch N");
        assert_eq!(x_of(&cx, n), -10.0);
        // Alt turns the snap off.
        cx.undo();
        let h = placed_handles(cx.floor(), PlacedRef::Cabinet(a), cx.px_per_in)
            .into_iter()
            .find(|h| h.kind == HandleKind::ResizeEnd)
            .unwrap();
        let orig = cabinet_by_id(cx.floor(), a).unwrap();
        let mut ev = PointerEvent::at(&cx, Point::new(59.0, 10.0));
        ev.modifiers.ctrl = true;
        let free = apply_edit(&cx, HandleKind::ResizeEnd, &orig, h.pos, &ev);
        assert_eq!(free.width, 39.0);
        set_fit_to_gap(true);
    }

    #[test]
    fn a_closed_polyline_becomes_a_polygon_soffit_in_one_undo_step() {
        let mut cx = setup();
        let ring = vec![
            Point::new(0.0, 20.0),
            Point::new(48.0, 20.0),
            Point::new(48.0, 44.0),
            Point::new(24.0, 56.0),
            Point::new(0.0, 44.0),
        ];
        let poly = cx.project.add_cad(
            0,
            "CAD, Default",
            plan_core::CadItem::Polyline {
                points: ring.clone(),
                closed: true,
            },
        );
        let open = cx.project.add_cad(
            0,
            "CAD, Default",
            plan_core::CadItem::Polyline {
                points: ring.clone(),
                closed: false,
            },
        );
        // An open polyline is not offered and not converted.
        cx.selection.set(ObjectRef::Cad(open));
        assert!(!placed::selection_has_closed_polyline(&cx));
        cx.run_custom(placed::SOFFIT_FROM_POLYLINE);
        assert!(cabs(&cx).is_empty());
        assert!(cx.status.contains("closed polyline"), "{}", cx.status);
        // The closed one is.
        cx.selection.set(ObjectRef::Cad(poly));
        assert!(placed::selection_has_closed_polyline(&cx));
        let t = CabinetTool::default();
        assert!(t.edit_toolbar(&cx).iter().any(
            |a| matches!(a.kind, EditActionKind::Custom { id, .. } if id == placed::SOFFIT_FROM_POLYLINE)
        ));
        cx.run_custom(placed::SOFFIT_FROM_POLYLINE);
        let soffits = cabs(&cx);
        assert_eq!(soffits.len(), 1);
        let s = &soffits[0];
        assert_eq!(s.kind, CabinetKind::Soffit);
        let outline = &s.custom.as_ref().expect("a polygon outline").outline;
        assert_eq!(outline.len(), 5);
        let area = plan_cabinets::ring_area(outline).abs();
        assert!(
            (area - (48.0 * 24.0 + 0.5 * 48.0 * 12.0)).abs() < 1e-6,
            "{area}"
        );
        assert!(
            cx.floor().cad.iter().all(|c| c.id != poly),
            "the polyline is replaced"
        );
        assert!(cx.floor().cad.iter().any(|c| c.id == open));
        assert_eq!(cx.selection.items, vec![ObjectRef::Cabinet(s.id)]);
        assert_eq!(cx.undo_label(), Some("Convert Polyline to Soffit"));
        cx.undo();
        assert!(cabs(&cx).is_empty());
        assert!(cx.floor().cad.iter().any(|c| c.id == poly));
    }
}
