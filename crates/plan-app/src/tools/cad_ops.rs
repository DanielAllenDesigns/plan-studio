//! Round 15 CAD operations as commands over the editor: Boolean polylines,
//! Trim and Extend to a boundary, Insert Point, Multiple Copy, Drawing Group
//! and the Plan Footprint from the outer wall faces.
//!
//! They are free functions over [`EditorContext`] (the tests and scenarios
//! call them without a tool) plus a small click-driven mode for the commands
//! that need clicks. The mode lives here (a thread-local like the Select
//! tool's own) and is reached through five one-line hooks in
//! `editor/transform.rs` (`mode_pointer_down`, `mode_pointer_move`,
//! `cancel_mode`, `mode_active`, `draw_mode_overlay`); the commands run from
//! the Edit toolbar and the menus through `EditorContext::run_custom` with the
//! ids below, and leave the Select tool in charge.
//!
//! * **Polyline Union / Subtract / Intersect** (CAD-55): the selected closed
//!   polylines and circles go through `plan_core::clip`; the results are
//!   closed polylines with their arc edges fitted back (a circle that comes
//!   out whole is a circle again). Holes come out as polylines of their own,
//!   grouped with their outline. One undo step.
//! * **Trim to Boundary / Extend to Boundary**: click the boundary object (a
//!   CAD line, polyline, arc, circle or a wall), then click the objects to
//!   cut back to it or to grow up to it, any number of them, until Esc. Lines,
//!   polylines, arcs and circles work.
//! * **Insert Point**: click an edge of a polyline (an arc edge splits into
//!   two arcs on the same circle) to add a vertex there.
//! * **Multiple Copy**: copies of any selection (walls included) at an even
//!   offset, optionally turning a little each time; the dialog or a two-click
//!   drag with a tick for every copy.
//! * **Drawing Group**: Bring to Front, Send to Back and Set Drawing Group on
//!   the selection (`plan_core::drawing_group`).
//! * **Plan Footprint** (L-38): the union of the wall outlines, so the polyline
//!   runs along the outer faces of the exterior walls.

use crate::editor::selection::hit_test_cx;
use crate::editor::transform::{self as xf, TransformParams};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, ObjectRef};
use crate::tools::cad::arcs::{self, Logical};
use crate::tools::{PointerEvent, ToolId, ToolResult};
use eframe::egui::{self, Stroke};
use plan_core::cad::{extend_segment, trim_segment, CadAttrs, CadItem, PolyArc};
use plan_core::clip::{self, BoolOp, OutPoly, Ring};
use plan_core::geometry::{polygon_area, project_on_segment, Point};
use plan_core::groups::{ObjectGroup, ObjectRef as CoreRef};
use plan_core::{wall_outlines, Id, WallKind};
use std::cell::{Cell, RefCell};
use std::f64::consts::TAU;

// ----- command ids -----

pub const UNION: &str = "cadops.union";
pub const SUBTRACT: &str = "cadops.subtract";
pub const INTERSECT: &str = "cadops.intersect";
pub const TRIM_BOUNDARY: &str = "cadops.trim_boundary";
pub const EXTEND_BOUNDARY: &str = "cadops.extend_boundary";
pub const INSERT_POINT: &str = "cadops.insert_point";
/// Opens the Multiple Copy dialog.
pub const MULTIPLE_COPY: &str = "cadops.multiple_copy";
/// The two-click Multiple Copy with ticks.
pub const MULTIPLE_COPY_DRAG: &str = "cadops.multiple_copy_drag";
pub const DG_FRONT: &str = "cadops.dg_front";
pub const DG_BACK: &str = "cadops.dg_back";
/// Opens the Set Drawing Group window.
pub const DG_SET: &str = "cadops.dg_set";
/// Opens Default Settings > Drawing Groups.
pub const DG_DEFAULTS: &str = "cadops.dg_defaults";
pub const PLAN_FOOTPRINT: &str = "cadops.plan_footprint";

/// Is `id` one of the commands of this module?
pub fn is_command(id: &str) -> bool {
    id.starts_with("cadops.")
}

/// The most copies one Multiple Copy makes.
pub const MAX_COPIES: u32 = 500;
/// Degrees between the points a circle or arc is flattened to for finding
/// where it meets a boundary.
const FLAT_DEG: f64 = 2.0;
/// Angle slack when comparing positions on a circle, radians.
const ANG_EPS: f64 = 1e-7;

// ----- selection helpers -----

fn selected_cad_ids(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) | ObjectRef::Text(id) => Some(*id),
            _ => None,
        })
        .collect()
}

fn cad_item(cx: &EditorContext, id: Id) -> Option<CadItem> {
    cx.floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.item.clone())
}

fn is_closed_shape(item: &CadItem) -> bool {
    matches!(
        item,
        CadItem::Polyline { closed: true, points } if points.len() >= 3
    ) || matches!(item, CadItem::Circle { .. })
}

/// The selected CAD objects that are closed polylines or circles, in
/// selection order.
fn selected_shapes(cx: &EditorContext) -> Vec<Id> {
    selected_cad_ids(cx)
        .into_iter()
        .filter(|id| cad_item(cx, *id).is_some_and(|i| is_closed_shape(&i)))
        .collect()
}

fn arc_edges_of(cx: &EditorContext, id: Id) -> Vec<PolyArc> {
    cx.floor()
        .cad_attrs(id)
        .map(|a| arcs::valid_arcs(&cad_points(cx, id), true, &a.arc_edges))
        .unwrap_or_default()
}

fn cad_points(cx: &EditorContext, id: Id) -> Vec<Point> {
    match cad_item(cx, id) {
        Some(CadItem::Polyline { points, .. }) => points,
        _ => Vec::new(),
    }
}

/// Removes a CAD object, takes it out of its group and drops the data that
/// belonged to it.
fn remove_object(cx: &mut EditorContext, id: Id) {
    let fl = cx.floor;
    cx.project.remove_cad(fl, id);
    let me = CoreRef::Cad(id);
    let f = &mut cx.project.floors[fl];
    for g in &mut f.groups {
        g.members.retain(|m| *m != me);
    }
    f.groups.retain(|g| g.members.len() >= 2);
    f.drawing_groups.retain(|e| e.object != me);
    cx.project.prune_cad_data(fl);
}

fn set_item(cx: &mut EditorContext, id: Id, item: CadItem) {
    let fl = cx.floor;
    if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
        c.item = item;
    }
}

/// Copies the line, fill and arrow of `from` to `to` (not its arc edges or
/// text).
fn copy_look(cx: &mut EditorContext, from: &Option<CadAttrs>, to: Id, arcs: Vec<PolyArc>) {
    let fl = cx.floor;
    let mut attrs = from.clone().unwrap_or_else(|| CadAttrs::new(to));
    attrs.target = to;
    attrs.arc_edges = arcs;
    attrs.runs.clear();
    if !attrs.is_default() {
        cx.project.set_cad_attrs(fl, attrs);
    }
}

// ----- Boolean polylines -----

/// What a Boolean did, for the status line and the tests.
#[derive(Debug, Clone, PartialEq)]
pub struct BooleanReport {
    /// The new polylines (circles included), outlines first.
    pub created: Vec<Id>,
    /// How many of them are holes.
    pub holes: usize,
    /// Net area of the result, square inches.
    pub area: f64,
}

/// Polyline Union, Subtract or Intersect on the selected closed polylines and
/// circles (at least two). For Subtract the first selected is the shape that
/// stays and the others are cut out of it. One undo step.
pub fn boolean_selected(cx: &mut EditorContext, op: BoolOp) -> Result<BooleanReport, String> {
    let ids = selected_shapes(cx);
    if ids.len() < 2 {
        return Err(format!(
            "Polyline {}: select two or more closed polylines or circles",
            op.name()
        ));
    }
    for id in &ids {
        if !cx.check_unlocked(ObjectRef::Cad(*id)) {
            return Err(cx.status.clone());
        }
    }
    let mut rings: Vec<Ring> = Vec::new();
    for id in &ids {
        let item = cad_item(cx, *id).ok_or("A selected object is gone")?;
        let arcs = arc_edges_of(cx, *id);
        rings.push(Ring::from_item(&item, &arcs).ok_or("A selected shape has no area")?);
    }
    let result = clip::boolean_all(op, &rings);
    let total_in: f64 = rings.iter().map(|r| r.area()).sum();
    let area = clip::region_area(&result);
    // Shapes that neither overlap nor touch change nothing.
    if op == BoolOp::Union && result.len() == rings.len() && (area - total_in).abs() < 1e-6 {
        return Err("Polyline Union: the shapes do not overlap or touch".into());
    }
    if op == BoolOp::Intersect && result.is_empty() {
        return Err("Polyline Intersect: the shapes do not overlap".into());
    }
    let polys: Vec<OutPoly> = clip::region_polys(&result);
    let subject = ids[0];
    let layer = cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == subject)
        .map(|c| c.layer.clone())
        .unwrap_or_else(|| plan_core::cad::DEFAULT_CAD_LAYER.to_string());
    let look = cx.floor().cad_attrs(subject);
    let label = format!("Polyline {}", op.name());
    cx.begin_change(&label);
    let fl = cx.floor;
    for id in &ids {
        remove_object(cx, *id);
    }
    let mut created = Vec::new();
    let mut holes = 0;
    for poly in &polys {
        let id = cx.project.add_cad(fl, layer.clone(), poly.item());
        copy_look(cx, &look, id, poly.arcs.clone());
        if poly.hole {
            holes += 1;
        }
        created.push(id);
    }
    // An outline and its holes move and select together.
    if holes > 0 && created.len() > 1 {
        let gid = cx.project.alloc_id();
        cx.project.floors[fl].groups.push(ObjectGroup {
            id: gid,
            members: created.iter().map(|i| CoreRef::Cad(*i)).collect(),
        });
    }
    cx.selection.items = created.iter().map(|i| ObjectRef::Cad(*i)).collect();
    cx.mark_dirty();
    cx.status = if created.is_empty() {
        format!("{label}: nothing is left")
    } else {
        format!(
            "{label}: {} polyline{}{}, {:.1} sq ft",
            created.len(),
            if created.len() == 1 { "" } else { "s" },
            if holes > 0 {
                format!(" ({holes} hole{})", if holes == 1 { "" } else { "s" })
            } else {
                String::new()
            },
            area / 144.0
        )
    };
    Ok(BooleanReport {
        created,
        holes,
        area,
    })
}

// ----- circle geometry for Trim and Extend -----

/// The straight pieces of `item`, arcs and circles flattened finely, for use
/// as a boundary.
pub fn flatten(item: &CadItem) -> Vec<(Point, Point)> {
    let sample = |c: Point, r: f64, a0: f64, sweep: f64| -> Vec<(Point, Point)> {
        let n = ((sweep.abs().to_degrees() / FLAT_DEG).ceil() as usize).max(2);
        let at = |k: usize| {
            let a = a0 + sweep * k as f64 / n as f64;
            Point::new(c.x + r * a.cos(), c.y + r * a.sin())
        };
        (0..n).map(|k| (at(k), at(k + 1))).collect()
    };
    match item {
        CadItem::Line { a, b } => vec![(*a, *b)],
        CadItem::Polyline { points, closed } => {
            let n = points.len();
            let edges = if *closed { n } else { n.saturating_sub(1) };
            (0..edges)
                .map(|i| (points[i], points[(i + 1) % n]))
                .collect()
        }
        CadItem::Circle { center, radius } => sample(*center, *radius, 0.0, TAU),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => sample(
            *center,
            *radius,
            *start_angle,
            (end_angle - start_angle).rem_euclid(TAU),
        ),
        CadItem::Text { .. } => Vec::new(),
    }
}

/// The angles, in `[0, TAU)`, where the circle meets the segments.
pub fn circle_cuts(center: Point, radius: f64, segs: &[(Point, Point)]) -> Vec<f64> {
    let mut out = Vec::new();
    for (a, b) in segs {
        let d = b.sub(*a);
        let f = a.sub(center);
        let qa = d.dot(d);
        if qa < 1e-18 {
            continue;
        }
        let qb = 2.0 * f.dot(d);
        let qc = f.dot(f) - radius * radius;
        let disc = qb * qb - 4.0 * qa * qc;
        if disc < 0.0 {
            continue;
        }
        let sq = disc.sqrt();
        for t in [(-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa)] {
            if (0.0..=1.0).contains(&t) {
                let p = a.add(d.scale(t));
                out.push(p.sub(center).angle().rem_euclid(TAU));
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < ANG_EPS);
    out
}

/// What is left of the arc `(start, sweep)` after the part under `pick` is
/// cut back to the nearest `cuts` on either side: up to two arcs as
/// `(start, sweep)`. `None` when no cut falls inside the arc.
pub fn trim_arc(start: f64, sweep: f64, pick: f64, cuts: &[f64]) -> Option<Vec<(f64, f64)>> {
    let off = |a: f64| (a - start).rem_euclid(TAU);
    let mut inside: Vec<f64> = cuts
        .iter()
        .map(|c| off(*c))
        .filter(|o| *o > ANG_EPS && *o < sweep - ANG_EPS)
        .collect();
    inside.sort_by(f64::total_cmp);
    if inside.is_empty() {
        return None;
    }
    let mut po = off(pick);
    if po > sweep {
        // Off the arc: the nearer end.
        po = if po - sweep < TAU - po { sweep } else { 0.0 };
    }
    let lo = inside
        .iter()
        .copied()
        .filter(|o| *o <= po)
        .fold(0.0, f64::max);
    let hi = inside
        .iter()
        .copied()
        .filter(|o| *o >= po)
        .fold(sweep, f64::min);
    let mut out = Vec::new();
    if lo > ANG_EPS {
        out.push((start, lo));
    }
    if sweep - hi > ANG_EPS {
        out.push(((start + hi).rem_euclid(TAU), sweep - hi));
    }
    Some(out)
}

/// What is left of a whole circle after the part under `pick` is cut back to
/// the nearest `cuts` on either side: one arc `(start, sweep)`. `None` for
/// fewer than two cuts.
pub fn trim_circle(pick: f64, cuts: &[f64]) -> Option<(f64, f64)> {
    if cuts.len() < 2 {
        return None;
    }
    let mut c: Vec<f64> = cuts.iter().map(|a| a.rem_euclid(TAU)).collect();
    c.sort_by(f64::total_cmp);
    let pick = pick.rem_euclid(TAU);
    let lo = c
        .iter()
        .copied()
        .filter(|a| *a <= pick)
        .fold(None, |m: Option<f64>, a| Some(m.map_or(a, |x| x.max(a))));
    let hi = c
        .iter()
        .copied()
        .filter(|a| *a >= pick)
        .fold(None, |m: Option<f64>, a| Some(m.map_or(a, |x| x.min(a))));
    let lo = lo.unwrap_or_else(|| c[c.len() - 1] - TAU);
    let hi = hi.unwrap_or_else(|| c[0] + TAU);
    let removed = hi - lo;
    Some((hi.rem_euclid(TAU), TAU - removed))
}

/// The arc `(start, sweep)` grown at the end nearer `pick` until it meets the
/// next of `cuts`; `None` when nothing lies beyond that end.
pub fn extend_arc(start: f64, sweep: f64, pick: f64, cuts: &[f64]) -> Option<(f64, f64)> {
    let gap = TAU - sweep;
    if gap < ANG_EPS * 10.0 {
        return None;
    }
    // Which end is nearer, by the angle along the circle.
    let d_start = (pick - start)
        .rem_euclid(TAU)
        .min((start - pick).rem_euclid(TAU));
    let end = start + sweep;
    let d_end = (pick - end)
        .rem_euclid(TAU)
        .min((end - pick).rem_euclid(TAU));
    if d_end <= d_start {
        let nearest = cuts
            .iter()
            .map(|c| (c - end).rem_euclid(TAU))
            .filter(|o| *o > ANG_EPS && *o < gap - ANG_EPS)
            .fold(f64::MAX, f64::min);
        (nearest < f64::MAX).then_some((start, sweep + nearest))
    } else {
        let nearest = cuts
            .iter()
            .map(|c| (start - c).rem_euclid(TAU))
            .filter(|o| *o > ANG_EPS && *o < gap - ANG_EPS)
            .fold(f64::MAX, f64::min);
        (nearest < f64::MAX).then_some(((start - nearest).rem_euclid(TAU), sweep + nearest))
    }
}

fn arc_item(center: Point, radius: f64, start: f64, sweep: f64) -> CadItem {
    CadItem::Arc {
        center,
        radius,
        start_angle: start.rem_euclid(TAU),
        end_angle: (start + sweep).rem_euclid(TAU),
    }
}

/// The pieces of `item` left after the part under `pick` is trimmed back to
/// `cutters`.
pub fn trim_item(
    item: &CadItem,
    pick: Point,
    cutters: &[(Point, Point)],
) -> Result<Vec<CadItem>, &'static str> {
    const NONE: &str = "No boundary crosses there";
    match item {
        CadItem::Line { a, b } => trim_segment((*a, *b), pick, cutters)
            .map(|v| v.into_iter().map(|(a, b)| CadItem::Line { a, b }).collect())
            .ok_or(NONE),
        CadItem::Polyline { points, closed } => {
            crate::tools::cad::trim_polyline(points, *closed, pick, cutters)
                .map(|v| {
                    v.into_iter()
                        .map(|points| CadItem::Polyline {
                            points,
                            closed: false,
                        })
                        .collect()
                })
                .ok_or(NONE)
        }
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            let cuts = circle_cuts(*center, *radius, cutters);
            let pick_a = pick.sub(*center).angle();
            trim_arc(*start_angle, sweep, pick_a, &cuts)
                .map(|v| {
                    v.into_iter()
                        .map(|(s, w)| arc_item(*center, *radius, s, w))
                        .collect()
                })
                .ok_or(NONE)
        }
        CadItem::Circle { center, radius } => {
            let cuts = circle_cuts(*center, *radius, cutters);
            trim_circle(pick.sub(*center).angle(), &cuts)
                .map(|(s, w)| vec![arc_item(*center, *radius, s, w)])
                .ok_or("A circle needs a boundary that crosses it twice")
        }
        CadItem::Text { .. } => Err("Text cannot be trimmed"),
    }
}

/// `item` grown at the end nearer `pick` up to the first of `bounds`.
pub fn extend_item(
    item: &CadItem,
    pick: Point,
    bounds: &[(Point, Point)],
) -> Result<CadItem, &'static str> {
    const NONE: &str = "Nothing lies in the way to extend to";
    match item {
        CadItem::Line { a, b } => extend_segment((*a, *b), pick, bounds)
            .map(|(a, b)| CadItem::Line { a, b })
            .ok_or(NONE),
        CadItem::Polyline {
            points,
            closed: false,
        } if points.len() >= 2 => {
            let n = points.len();
            let to_first = points[0].dist(pick) < points[n - 1].dist(pick);
            let seg = if to_first {
                (points[1], points[0])
            } else {
                (points[n - 2], points[n - 1])
            };
            let (_, tip) = extend_segment(seg, pick, bounds).ok_or(NONE)?;
            let mut pts = points.clone();
            if to_first {
                pts[0] = tip;
            } else {
                pts[n - 1] = tip;
            }
            Ok(CadItem::Polyline {
                points: pts,
                closed: false,
            })
        }
        CadItem::Polyline { .. } => Err("A closed polyline has no end to extend"),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            let cuts = circle_cuts(*center, *radius, bounds);
            extend_arc(*start_angle, sweep, pick.sub(*center).angle(), &cuts)
                .map(|(s, w)| arc_item(*center, *radius, s, w))
                .ok_or(NONE)
        }
        CadItem::Circle { .. } => Err("A circle has no end to extend"),
        CadItem::Text { .. } => Err("Text cannot be extended"),
    }
}

// ----- Trim and Extend to a boundary -----

/// Trims or extends the CAD object `id` against `boundary` at `pick`: one
/// undo step named `label`. Returns the pieces now standing in place of the
/// object.
pub fn trim_extend_object(
    cx: &mut EditorContext,
    id: Id,
    pick: Point,
    boundary: &[(Point, Point)],
    extend: bool,
    label: &str,
) -> Result<Vec<Id>, String> {
    let item = cad_item(cx, id).ok_or("That object is gone")?;
    if !cx.check_unlocked(ObjectRef::Cad(id)) {
        return Err(cx.status.clone());
    }
    let pieces = if extend {
        vec![extend_item(&item, pick, boundary)?]
    } else {
        trim_item(&item, pick, boundary)?
    };
    let layer = cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.layer.clone())
        .unwrap_or_default();
    let look = cx.floor().cad_attrs(id);
    cx.begin_change(label);
    let fl = cx.floor;
    let mut kept = Vec::new();
    let mut it = pieces.into_iter();
    match it.next() {
        Some(first) => {
            // Arc edges described the old shape; keep the look only.
            set_item(cx, id, first);
            cx.project.edit_cad_attrs(fl, id, |a| a.arc_edges.clear());
            kept.push(id);
        }
        None => remove_object(cx, id),
    }
    for rest in it {
        let nid = cx.project.add_cad(fl, layer.clone(), rest);
        copy_look(cx, &look, nid, Vec::new());
        kept.push(nid);
    }
    cx.mark_dirty();
    Ok(kept)
}

/// The pieces Trim Line leaves of an arc or circle (the CAD tool's own Trim
/// Line mode calls this): `None`, with the reason in the status bar, when no
/// cutter crosses it.
pub fn trim_for_tool(
    cx: &mut EditorContext,
    item: &CadItem,
    pick: Point,
    cutters: &[(Point, Point)],
) -> Option<Vec<CadItem>> {
    match trim_item(item, pick, cutters) {
        Ok(v) => Some(v),
        Err(e) => {
            cx.status = format!("Trim Line: {e}");
            None
        }
    }
}

/// Extend Line on an object that is not a line (an arc or an open polyline):
/// grows the end nearer `pick` to the first other CAD object in the way.
pub fn extend_for_tool(cx: &mut EditorContext, id: Id, pick: Point) -> ToolResult {
    let bounds: Vec<(Point, Point)> = cx
        .floor()
        .cad
        .iter()
        .filter(|c| c.id != id && cx.layers().is_visible(&c.layer))
        .flat_map(|c| flatten(&c.item))
        .collect();
    match trim_extend_object(cx, id, pick, &bounds, true, "Extend Line") {
        Ok(_) => {
            cx.status.clear();
            ToolResult::committed("Extend Line")
        }
        Err(e) => {
            cx.status = format!("Extend Line: {e}");
            ToolResult::consumed()
        }
    }
}

/// The boundary segments an object stands for: a CAD object's outline or a
/// wall's centerline.
fn boundary_of(cx: &EditorContext, o: ObjectRef) -> Option<Vec<(Point, Point)>> {
    match o {
        ObjectRef::Cad(id) => {
            let segs = flatten(&cad_item(cx, id)?);
            (!segs.is_empty()).then_some(segs)
        }
        ObjectRef::Wall(id) => cx.floor().wall(id).map(|w| vec![(w.start, w.end)]),
        _ => None,
    }
}

// ----- Insert Point -----

/// The nearest point to `p` on edge `i` of `l`, with the bulges the two edges
/// it splits into get.
fn split_edge(l: &Logical, i: usize, p: Point) -> Option<(Point, Option<f64>, Option<f64>)> {
    let n = l.pts.len();
    let (a, b) = (l.pts[i], l.pts[(i + 1) % n]);
    match l.bulge[i] {
        None => {
            let (_, q) = project_on_segment(p, a, b);
            (q.dist(a) > 1e-6 && q.dist(b) > 1e-6).then_some((q, None, None))
        }
        Some(bulge) => {
            let (c, r) = arcs::arc_center(a, b, bulge)?;
            let theta = arcs::sweep_of(bulge);
            let dir = p.sub(c).normalized();
            let q = c.add(dir.scale(r));
            // The sweep from a to q, in the arc's direction.
            let a0 = a.sub(c).angle();
            let mut s1 = (q.sub(c).angle() - a0).rem_euclid(TAU);
            if theta < 0.0 {
                s1 -= TAU;
                if s1 == -TAU {
                    s1 = 0.0;
                }
            }
            let inside = if theta >= 0.0 {
                s1 > 1e-6 && s1 < theta - 1e-6
            } else {
                s1 < -1e-6 && s1 > theta + 1e-6
            };
            inside.then_some((q, Some((s1 / 4.0).tan()), Some(((theta - s1) / 4.0).tan())))
        }
    }
}

/// Adds a vertex to polyline `id` on the edge nearest `at`: one undo step.
pub fn insert_point(cx: &mut EditorContext, id: Id, at: Point) -> Result<Point, String> {
    let l = arcs::logical_of(cx, id).ok_or("Insert Point works on polylines")?;
    if !cx.check_unlocked(ObjectRef::Cad(id)) {
        return Err(cx.status.clone());
    }
    let (i, d) = l.nearest_edge(at).ok_or("The polyline has no edges")?;
    if d > cx.pick_tol() * 4.0 {
        return Err("Click on an edge of the polyline".into());
    }
    let (q, b1, b2) = split_edge(&l, i, at).ok_or("There is already a vertex there")?;
    let mut l = l;
    l.pts.insert(i + 1, q);
    l.bulge[i] = b1;
    l.bulge.insert(i + 1, b2);
    cx.begin_change("Insert Point");
    arcs::replace_polyline(cx, id, &l);
    Ok(q)
}

// ----- Multiple Copy -----

/// The values of Multiple Copy.
#[derive(Debug, Clone, PartialEq)]
pub struct MultipleCopy {
    pub count: u32,
    /// From one copy to the next, inches.
    pub step: Point,
    /// Degrees each copy is turned further than the one before, about the
    /// center of the selection.
    pub turn_deg: f64,
}

impl Default for MultipleCopy {
    fn default() -> Self {
        Self {
            count: 3,
            step: Point::new(48.0, 0.0),
            turn_deg: 0.0,
        }
    }
}

impl MultipleCopy {
    /// Copies `distance` apart along `angle_deg` (counter-clockwise from the
    /// X axis).
    pub fn polar(count: u32, distance: f64, angle_deg: f64) -> Self {
        let a = angle_deg.to_radians();
        Self {
            count,
            step: Point::new(distance * a.cos(), distance * a.sin()),
            turn_deg: 0.0,
        }
    }

    /// `count` copies spread evenly over `total`, the offset of the last one
    /// from the original.
    pub fn spread(count: u32, total: Point) -> Self {
        Self {
            count,
            step: total.scale(1.0 / f64::from(count.max(1))),
            turn_deg: 0.0,
        }
    }
}

/// Multiple Copy of the selection (walls included): `count` copies, copy k
/// offset k steps from the original (and turned k times the turn). One undo
/// step; the copies become the selection.
pub fn multiple_copy(cx: &mut EditorContext, p: &MultipleCopy) -> Result<String, String> {
    if p.count == 0 || p.count > MAX_COPIES {
        return Err(format!("Make between 1 and {MAX_COPIES} copies"));
    }
    if p.step.length() < 1e-6 && p.turn_deg.abs() < 1e-9 {
        return Err("The copies would all land on the original".into());
    }
    let params = TransformParams {
        copies: p.count,
        move_x: p.step.x,
        move_y: p.step.y,
        rotate_deg: p.turn_deg,
        ..TransformParams::default()
    };
    let mut result = Err("Select objects to copy".to_string());
    cx.as_one_step("Multiple Copy", |cx| {
        result = xf::transform_replicate(cx, &params);
    });
    if let Ok(msg) = &result {
        cx.status = msg.clone();
    }
    result
}

thread_local! {
    static COPY_SETTINGS: RefCell<MultipleCopy> = RefCell::new(MultipleCopy::default());
}

/// The values Multiple Copy was last used with (the dialog and the drag
/// share them).
pub fn copy_settings() -> MultipleCopy {
    COPY_SETTINGS.with(|s| s.borrow().clone())
}

pub fn set_copy_settings(s: MultipleCopy) {
    COPY_SETTINGS.with(|c| *c.borrow_mut() = s);
}

// ----- Drawing Group -----

fn core_refs(cx: &EditorContext) -> Vec<CoreRef> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| o.to_group_ref())
        .collect()
}

/// Bring to Front (`front`) or Send to Back for the selection. One undo step.
pub fn drawing_group_order(cx: &mut EditorContext, front: bool) -> Result<usize, String> {
    let refs = core_refs(cx);
    if refs.is_empty() {
        return Err("Select objects first".into());
    }
    let label = if front {
        "Bring to Front"
    } else {
        "Send to Back"
    };
    cx.begin_change(label);
    let fl = cx.floor;
    let n = if front {
        cx.project.drawing_group_to_front(fl, &refs)
    } else {
        cx.project.drawing_group_to_back(fl, &refs)
    };
    if n == 0 {
        cx.cancel_change();
        return Err(format!(
            "{label}: the selection is already {}",
            if front { "in front" } else { "at the back" }
        ));
    }
    cx.mark_dirty();
    cx.status = format!("{label}: {n} object{}", if n == 1 { "" } else { "s" });
    Ok(n)
}

/// Set Drawing Group: `group` for the selection (`None` puts each object back
/// in its kind's group). One undo step.
pub fn set_drawing_group(cx: &mut EditorContext, group: Option<i32>) -> Result<usize, String> {
    let refs = core_refs(cx);
    if refs.is_empty() {
        return Err("Select objects first".into());
    }
    cx.begin_change("Set Drawing Group");
    let fl = cx.floor;
    let n = cx.project.set_drawing_groups(fl, &refs, group);
    if n == 0 {
        cx.cancel_change();
        return Err("Set Drawing Group: nothing changed".into());
    }
    cx.mark_dirty();
    cx.status = format!(
        "Set the drawing group of {n} object{}",
        if n == 1 { "" } else { "s" }
    );
    Ok(n)
}

// ----- Plan Footprint -----

/// Plan Footprint (L-38): a closed polyline around the outer faces of the
/// exterior walls of the floor (the union of the wall outlines, so wall joins
/// and openings do not matter), with the area written under it. Interior
/// walls are inside it and do not change it. Returns the area in square feet,
/// `None` when the floor has no walls. One undo step.
pub fn plan_footprint(cx: &mut EditorContext) -> Option<f64> {
    let (rings, area_sf) = footprint_rings(cx)?;
    let height = cx.defaults.text.height;
    cx.begin_change("Plan Footprint");
    let fl = cx.floor;
    let mut lo = Point::new(f64::MAX, f64::MAX);
    let mut made = Vec::new();
    for poly in clip::region_polys(&rings) {
        for p in &poly.points {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        }
        let id = cx
            .project
            .add_cad(fl, plan_core::cad::DEFAULT_CAD_LAYER, poly.item());
        copy_look(cx, &None, id, poly.arcs);
        made.push(id);
    }
    cx.project.add_cad(
        fl,
        plan_core::cad::DEFAULT_CAD_LAYER,
        CadItem::Text {
            pos: Point::new(lo.x, lo.y - height * 2.0),
            text: format!("Footprint: {area_sf:.0} sq ft"),
            height,
            angle: 0.0,
        },
    );
    cx.selection.items = made.iter().map(|i| ObjectRef::Cad(*i)).collect();
    cx.mark_dirty();
    cx.status = format!("Plan Footprint (outer wall faces): {area_sf:.0} sq ft");
    Some(area_sf)
}

/// The outer rings of the footprint of the active floor and their area in
/// square feet.
pub fn footprint_rings(cx: &EditorContext) -> Option<(Vec<Ring>, f64)> {
    let f = cx.floor();
    let exterior: Vec<&plan_core::Wall> = f
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .collect();
    let walls: Vec<plan_core::Wall> = if exterior.is_empty() {
        f.walls.clone()
    } else {
        exterior.into_iter().cloned().collect()
    };
    if walls.is_empty() {
        return None;
    }
    let shapes: Vec<Ring> = wall_outlines(&walls, 0.5)
        .iter()
        .filter_map(|o| Ring::from_polyline(&o.polygon, &[]))
        .collect();
    let union = clip::boolean_all(BoolOp::Union, &shapes);
    // The footprint is the outside: holes (courtyards) are not part of it.
    let outer: Vec<Ring> = union.into_iter().filter(|r| r.area() > 0.0).collect();
    if outer.is_empty() {
        return None;
    }
    let area = outer.iter().map(|r| polygon_area(&r.pts)).sum::<f64>() / 144.0;
    Some((outer, area))
}

// ----- the click-driven modes -----

#[derive(Clone, Debug)]
enum Mode {
    /// Trim or Extend to a boundary: first the boundary is clicked.
    Boundary {
        extend: bool,
        boundary: Option<Vec<(Point, Point)>>,
    },
    InsertPoint,
    /// Multiple Copy by dragging: the base point, then the last copy.
    MultiCopy {
        base: Option<Point>,
    },
}

thread_local! {
    static MODE: RefCell<Option<Mode>> = const { RefCell::new(None) };
    static CURSOR: Cell<Option<Point>> = const { Cell::new(None) };
}

fn set_mode(m: Option<Mode>) {
    MODE.with(|x| *x.borrow_mut() = m);
}

fn mode() -> Option<Mode> {
    MODE.with(|x| x.borrow().clone())
}

/// Is one of this module's modes waiting for clicks?
pub fn active() -> bool {
    MODE.with(|m| m.borrow().is_some())
}

/// Ends the mode without doing anything. True when there was one.
pub fn cancel(cx: &mut EditorContext) -> bool {
    let had = MODE.with(|m| m.borrow_mut().take()).is_some();
    if had {
        cx.status.clear();
    }
    had
}

/// The pointer moved (the ticks of a Multiple Copy drag follow it).
pub fn pointer_move(p: &PointerEvent) {
    if active() {
        CURSOR.with(|c| c.set(Some(p.snapped)));
    }
}

fn first_cad(cx: &EditorContext, at: Point, skip_text: bool) -> Option<Id> {
    hit_test_cx(cx, at, cx.pick_tol())
        .into_iter()
        .find_map(|o| match o {
            ObjectRef::Cad(id)
                if !skip_text || !matches!(cad_item(cx, id), Some(CadItem::Text { .. })) =>
            {
                Some(id)
            }
            _ => None,
        })
}

/// A click while one of the modes is on. `None`: no mode of ours.
pub fn pointer_down(cx: &mut EditorContext, p: &PointerEvent) -> Option<ToolResult> {
    let mode = mode()?;
    match mode {
        Mode::Boundary { extend, boundary } => {
            let verb = if extend { "Extend" } else { "Trim" };
            let Some(boundary) = boundary else {
                let hit = hit_test_cx(cx, p.world, cx.pick_tol())
                    .into_iter()
                    .find(|o| matches!(o, ObjectRef::Cad(_) | ObjectRef::Wall(_)));
                match hit.and_then(|o| boundary_of(cx, o)) {
                    Some(segs) => {
                        set_mode(Some(Mode::Boundary {
                            extend,
                            boundary: Some(segs),
                        }));
                        cx.status = if extend {
                            "Extend to Boundary: click the end of an object to extend".into()
                        } else {
                            "Trim to Boundary: click the part of an object to cut away".into()
                        };
                    }
                    None => {
                        cx.status = format!(
                            "{verb} to Boundary: click a line, arc, polyline, circle or wall"
                        );
                    }
                }
                return Some(ToolResult::consumed());
            };
            let Some(id) = first_cad(cx, p.world, true) else {
                cx.status = format!("{verb} to Boundary: click a CAD object");
                return Some(ToolResult::consumed());
            };
            let label = if extend {
                "Extend to Boundary"
            } else {
                "Trim to Boundary"
            };
            Some(
                match trim_extend_object(cx, id, p.world, &boundary, extend, label) {
                    Ok(_) => {
                        cx.status.clear();
                        ToolResult::committed(label)
                    }
                    Err(e) => {
                        cx.status = format!("{verb} to Boundary: {e}");
                        ToolResult::consumed()
                    }
                },
            )
        }
        Mode::InsertPoint => {
            let Some(id) = hit_test_cx(cx, p.world, cx.pick_tol() * 2.0)
                .into_iter()
                .find_map(|o| match o {
                    ObjectRef::Cad(id)
                        if matches!(cad_item(cx, id), Some(CadItem::Polyline { .. })) =>
                    {
                        Some(id)
                    }
                    _ => None,
                })
            else {
                cx.status = "Insert Point: click an edge of a polyline".into();
                return Some(ToolResult::consumed());
            };
            Some(match insert_point(cx, id, p.world) {
                Ok(_) => {
                    cx.selection.set(ObjectRef::Cad(id));
                    ToolResult::committed("Insert Point")
                }
                Err(e) => {
                    cx.status = format!("Insert Point: {e}");
                    ToolResult::consumed()
                }
            })
        }
        Mode::MultiCopy { base: None } => {
            set_mode(Some(Mode::MultiCopy {
                base: Some(p.snapped),
            }));
            let n = copy_settings().count;
            cx.status = format!("Multiple Copy: click the position of the last of {n} copies");
            Some(ToolResult::consumed())
        }
        Mode::MultiCopy { base: Some(base) } => {
            let s = copy_settings();
            set_mode(None);
            let total = p.snapped - base;
            let params = MultipleCopy {
                step: total.scale(1.0 / f64::from(s.count.max(1))),
                ..s
            };
            Some(match multiple_copy(cx, &params) {
                Ok(_) => ToolResult::committed("Multiple Copy"),
                Err(e) => {
                    cx.status = e;
                    ToolResult::consumed()
                }
            })
        }
    }
}

/// Draws what the mode shows: the drag line of a Multiple Copy with a tick at
/// every copy.
pub fn draw_overlay(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let Some(Mode::MultiCopy { base: Some(base) }) = mode() else {
        return;
    };
    let Some(to) = CURSOR.with(Cell::get).or(cx.cursor_world) else {
        return;
    };
    let n = copy_settings().count.max(1);
    let stroke = Stroke::new(1.5_f32, cx.palette.ghost_stroke);
    painter.line_segment([cam.world_to_screen(base), cam.world_to_screen(to)], stroke);
    let step = (to - base).scale(1.0 / f64::from(n));
    let along = step.normalized();
    for k in 0..=n {
        let at = cam.world_to_screen(base + step.scale(f64::from(k)));
        let side = egui::vec2(-along.y as f32, -along.x as f32) * 5.0;
        painter.line_segment([at - side, at + side], stroke);
    }
}

// ----- commands -----

fn start_mode(cx: &mut EditorContext, m: Mode, hint: &str) {
    set_mode(Some(m));
    cx.status = hint.into();
    cx.requests
        .push(crate::editor::EditorRequest::SetTool(ToolId::Select));
}

/// Runs a `cadops.*` command. False when `id` is not one of them.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    let report = |cx: &mut EditorContext, r: Result<(), String>| {
        if let Err(e) = r {
            cx.status = e;
        }
    };
    match id {
        UNION | SUBTRACT | INTERSECT => {
            let op = match id {
                UNION => BoolOp::Union,
                SUBTRACT => BoolOp::Subtract,
                _ => BoolOp::Intersect,
            };
            let r = boolean_selected(cx, op).map(|_| ());
            report(cx, r);
        }
        TRIM_BOUNDARY | EXTEND_BOUNDARY => {
            let extend = id == EXTEND_BOUNDARY;
            let hint = if extend {
                "Extend to Boundary: click the boundary object"
            } else {
                "Trim to Boundary: click the boundary object"
            };
            start_mode(
                cx,
                Mode::Boundary {
                    extend,
                    boundary: None,
                },
                hint,
            );
        }
        INSERT_POINT => start_mode(
            cx,
            Mode::InsertPoint,
            "Insert Point: click an edge of a polyline",
        ),
        MULTIPLE_COPY => {
            if cx.selection.is_empty() {
                cx.status = "Select objects to copy".into();
            } else {
                crate::dialogs::multiple_copy::open();
            }
        }
        MULTIPLE_COPY_DRAG => {
            if cx.selection.is_empty() {
                cx.status = "Select objects to copy".into();
            } else {
                start_mode(
                    cx,
                    Mode::MultiCopy { base: None },
                    "Multiple Copy: click the point to copy from",
                );
            }
        }
        DG_FRONT => {
            let r = drawing_group_order(cx, true).map(|_| ());
            report(cx, r);
        }
        DG_BACK => {
            let r = drawing_group_order(cx, false).map(|_| ());
            report(cx, r);
        }
        DG_SET => {
            if cx.selection.is_empty() {
                cx.status = "Select objects first".into();
            } else {
                crate::dialogs::drawing_groups::open_set(cx);
            }
        }
        DG_DEFAULTS => crate::dialogs::drawing_groups::open_defaults(),
        PLAN_FOOTPRINT => {
            if plan_footprint(cx).is_none() {
                cx.status = "Plan Footprint: there are no walls on this floor".into();
            }
        }
        _ => return false,
    }
    true
}

/// The Edit toolbar buttons of this module for the selection.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let button = |id: &'static str, label: &'static str| EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled: true,
    };
    let mut v = Vec::new();
    if selected_shapes(cx).len() >= 2 {
        v.push(button(UNION, "Polyline Union"));
        v.push(button(SUBTRACT, "Polyline Subtract"));
        v.push(button(INTERSECT, "Polyline Intersect"));
    }
    let drawn = selected_cad_ids(cx)
        .into_iter()
        .filter_map(|id| cad_item(cx, id))
        .filter(|i| !matches!(i, CadItem::Text { .. }))
        .collect::<Vec<_>>();
    if !drawn.is_empty() {
        v.push(button(TRIM_BOUNDARY, "Trim to Boundary"));
        v.push(button(EXTEND_BOUNDARY, "Extend to Boundary"));
    }
    if drawn.iter().any(|i| matches!(i, CadItem::Polyline { .. })) {
        v.push(button(INSERT_POINT, "Insert Point"));
    }
    if !cx.selection.is_empty() {
        v.push(button(MULTIPLE_COPY, "Multiple Copy"));
    }
    v
}

// ----- tests -----

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::cad::CadItem;
    use std::f64::consts::PI;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn rect(cx: &mut EditorContext, x: f64, y: f64, w: f64, h: f64) -> Id {
        cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Polyline {
                points: vec![
                    Point::new(x, y),
                    Point::new(x + w, y),
                    Point::new(x + w, y + h),
                    Point::new(x, y + h),
                ],
                closed: true,
            },
        )
    }

    fn select(cx: &mut EditorContext, ids: &[Id]) {
        cx.selection.items = ids.iter().map(|i| ObjectRef::Cad(*i)).collect();
    }

    fn area_of(cx: &EditorContext, id: Id) -> f64 {
        match cad_item(cx, id).unwrap() {
            CadItem::Polyline { points, .. } => polygon_area(&points).abs(),
            CadItem::Circle { radius, .. } => PI * radius * radius,
            _ => 0.0,
        }
    }

    #[test]
    fn union_of_two_overlapping_rectangles_is_one_polyline_and_one_undo_step() {
        let mut cx = cx();
        let a = rect(&mut cx, 0.0, 0.0, 100.0, 100.0);
        let b = rect(&mut cx, 50.0, 50.0, 100.0, 100.0);
        select(&mut cx, &[a, b]);
        let r = boolean_selected(&mut cx, BoolOp::Union).unwrap();
        assert_eq!(r.created.len(), 1);
        assert!((area_of(&cx, r.created[0]) - 17500.0).abs() < 1e-6);
        assert_eq!(cx.floor().cad.len(), 1, "both originals are gone");
        assert_eq!(cx.undo_label(), Some("Polyline Union"));
        cx.undo();
        assert_eq!(cx.floor().cad.len(), 2);
    }

    #[test]
    fn subtract_keeps_the_first_and_cuts_the_others_out() {
        let mut cx = cx();
        let a = rect(&mut cx, 0.0, 0.0, 100.0, 100.0);
        let b = rect(&mut cx, 25.0, 25.0, 50.0, 50.0);
        select(&mut cx, &[a, b]);
        let r = boolean_selected(&mut cx, BoolOp::Subtract).unwrap();
        assert_eq!(r.created.len(), 2, "an outline and a hole");
        assert_eq!(r.holes, 1);
        assert!((r.area - 7500.0).abs() < 1e-6);
        // The outline and its hole are grouped.
        assert_eq!(cx.floor().groups.len(), 1);
        assert_eq!(cx.floor().groups[0].members.len(), 2);
    }

    #[test]
    fn intersect_of_disjoint_shapes_says_so_and_changes_nothing() {
        let mut cx = cx();
        let a = rect(&mut cx, 0.0, 0.0, 10.0, 10.0);
        let b = rect(&mut cx, 50.0, 50.0, 10.0, 10.0);
        select(&mut cx, &[a, b]);
        let e = boolean_selected(&mut cx, BoolOp::Intersect).unwrap_err();
        assert!(e.contains("do not overlap"), "{e}");
        assert_eq!(cx.floor().cad.len(), 2);
        assert!(cx.undo_label().is_none());
        let e = boolean_selected(&mut cx, BoolOp::Union).unwrap_err();
        assert!(e.contains("do not overlap or touch"), "{e}");
    }

    #[test]
    fn a_circle_cut_by_a_rectangle_becomes_a_polyline_with_an_arc_edge() {
        let mut cx = cx();
        let c = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::ZERO,
                radius: 50.0,
            },
        );
        let b = rect(&mut cx, 0.0, -100.0, 100.0, 200.0);
        select(&mut cx, &[c, b]);
        let r = boolean_selected(&mut cx, BoolOp::Intersect).unwrap();
        assert_eq!(r.created.len(), 1);
        let id = r.created[0];
        let half = PI * 2500.0 / 2.0;
        assert!((r.area - half).abs() / half < 0.01);
        let attrs = cx.floor().cad_attrs(id).expect("arc edges are stored");
        assert_eq!(attrs.arc_edges.len(), 1);
        assert!(
            (attrs.arc_edges[0].bulge.abs() - 1.0).abs() < 1e-6,
            "a half turn"
        );
        // Union with a second whole circle on the same spot leaves a circle.
        let c2 = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(0.0, 0.0),
                radius: 50.0,
            },
        );
        let c3 = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(0.0, 0.0),
                radius: 20.0,
            },
        );
        select(&mut cx, &[c2, c3]);
        let r = boolean_selected(&mut cx, BoolOp::Union).unwrap();
        assert!(
            matches!(cad_item(&cx, r.created[0]), Some(CadItem::Circle { radius, .. }) if (radius - 50.0).abs() < 1e-6)
        );
    }

    #[test]
    fn trim_arc_removes_the_part_between_the_cuts() {
        // A quarter-turn arc from 0 to 90 degrees, cut at 30 and 60: picking
        // at 45 keeps the two ends.
        let pieces = trim_arc(0.0, PI / 2.0, PI / 4.0, &[PI / 6.0, PI / 3.0]).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!((pieces[0].0).abs() < 1e-9 && (pieces[0].1 - PI / 6.0).abs() < 1e-9);
        assert!((pieces[1].0 - PI / 3.0).abs() < 1e-9 && (pieces[1].1 - PI / 6.0).abs() < 1e-9);
        // Picking between the start and the first cut leaves one piece.
        let pieces = trim_arc(0.0, PI / 2.0, 0.1, &[PI / 6.0, PI / 3.0]).unwrap();
        assert_eq!(pieces, vec![(PI / 6.0, PI / 2.0 - PI / 6.0)]);
        // No cut inside the arc: nothing to trim to.
        assert!(trim_arc(0.0, PI / 2.0, 0.1, &[PI]).is_none());
    }

    #[test]
    fn trim_circle_keeps_the_arc_outside_the_cuts() {
        // Cuts at 90 and 270 degrees, picking at 0: the right half goes, the
        // left half stays, starting at 270 and sweeping a half turn.
        let (s, w) = trim_circle(0.0, &[PI / 2.0, 3.0 * PI / 2.0]).unwrap();
        assert!((w - PI).abs() < 1e-9);
        assert!((s - PI / 2.0).abs() < 1e-9 || (s - 3.0 * PI / 2.0).abs() < 1e-9);
        // Picking at 180 keeps the right half instead.
        let (s, w) = trim_circle(PI, &[PI / 2.0, 3.0 * PI / 2.0]).unwrap();
        assert!((w - PI).abs() < 1e-9);
        assert!((s - 3.0 * PI / 2.0).abs() < 1e-9);
        assert!(trim_circle(0.0, &[1.0]).is_none());
    }

    #[test]
    fn extend_arc_grows_the_nearer_end_to_the_next_cut() {
        // Arc 0..90 degrees, a boundary at 120 and one at -30: picking near
        // the end grows it to 120, picking near the start grows it to -30.
        let (s, w) =
            extend_arc(0.0, PI / 2.0, PI / 2.0 - 0.05, &[2.0 * PI / 3.0, -PI / 6.0]).unwrap();
        assert!(s.abs() < 1e-9 && (w - 2.0 * PI / 3.0).abs() < 1e-9);
        let (s, w) = extend_arc(0.0, PI / 2.0, 0.05, &[2.0 * PI / 3.0, -PI / 6.0]).unwrap();
        assert!((s - (TAU - PI / 6.0)).abs() < 1e-9 && (w - (PI / 2.0 + PI / 6.0)).abs() < 1e-9);
        assert!(
            extend_arc(0.0, PI / 2.0, 0.05, &[PI / 4.0]).is_none(),
            "a cut inside the arc is no extension"
        );
    }

    #[test]
    fn trim_and_extend_lines_arcs_and_circles_against_a_boundary() {
        let mut cx = cx();
        // The boundary: a vertical line at x = 50.
        let boundary = vec![(Point::new(50.0, -100.0), Point::new(50.0, 100.0))];
        // Extend: a horizontal line ending at x = 30 grows to 50.
        let l = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(30.0, 0.0),
            },
        );
        trim_extend_object(
            &mut cx,
            l,
            Point::new(28.0, 0.0),
            &boundary,
            true,
            "Extend to Boundary",
        )
        .unwrap();
        match cad_item(&cx, l).unwrap() {
            CadItem::Line { b, .. } => assert!(b.dist(Point::new(50.0, 0.0)) < 1e-9),
            o => panic!("{o:?}"),
        }
        assert_eq!(cx.undo_label(), Some("Extend to Boundary"));
        // Trim: a line crossing the boundary loses the picked side.
        let l2 = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(0.0, 20.0),
                b: Point::new(100.0, 20.0),
            },
        );
        trim_extend_object(
            &mut cx,
            l2,
            Point::new(90.0, 20.0),
            &boundary,
            false,
            "Trim to Boundary",
        )
        .unwrap();
        match cad_item(&cx, l2).unwrap() {
            CadItem::Line { a, b } => {
                assert!(
                    a.dist(Point::new(0.0, 20.0)) < 1e-9 && b.dist(Point::new(50.0, 20.0)) < 1e-9
                )
            }
            o => panic!("{o:?}"),
        }
        // Trim: an arc crossing the boundary.
        let arc = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Arc {
                center: Point::ZERO,
                radius: 100.0,
                start_angle: -0.5,
                end_angle: 0.5,
            },
        );
        let pick = Point::new(100.0 * 0.4f64.cos(), 100.0 * 0.4f64.sin());
        // The boundary x = 50 does not cross a radius-100 arc near 0 rad.
        assert!(
            trim_extend_object(&mut cx, arc, pick, &boundary, false, "Trim to Boundary").is_err()
        );
        let vert = vec![(Point::new(-200.0, 0.0), Point::new(200.0, 0.0))];
        let ids = trim_extend_object(&mut cx, arc, pick, &vert, false, "Trim to Boundary").unwrap();
        assert_eq!(ids.len(), 1);
        match cad_item(&cx, arc).unwrap() {
            CadItem::Arc {
                start_angle,
                end_angle,
                ..
            } => {
                // What is left runs from -0.5 rad up to the boundary at 0.
                assert!((start_angle - (TAU - 0.5)).abs() < 1e-6, "{start_angle}");
                assert!(end_angle.min(TAU - end_angle) < 1e-6, "{end_angle}");
            }
            o => panic!("{o:?}"),
        }
        // Trim: a circle across a line becomes an arc.
        let circ = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::new(0.0, 200.0),
                radius: 50.0,
            },
        );
        let chord = vec![(Point::new(-200.0, 200.0), Point::new(200.0, 200.0))];
        trim_extend_object(
            &mut cx,
            circ,
            Point::new(0.0, 250.0),
            &chord,
            false,
            "Trim to Boundary",
        )
        .unwrap();
        match cad_item(&cx, circ).unwrap() {
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                assert_eq!(center, Point::new(0.0, 200.0));
                assert!((radius - 50.0).abs() < 1e-9);
                // The bottom half is left: from 180 to 360 degrees.
                assert!((start_angle - PI).abs() < 1e-6 && end_angle.min(TAU - end_angle) < 1e-6);
            }
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn an_open_polyline_extends_at_its_nearer_end() {
        let item = CadItem::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(20.0, 0.0),
                Point::new(20.0, 30.0),
            ],
            closed: false,
        };
        let wall = vec![(Point::new(-50.0, 80.0), Point::new(50.0, 80.0))];
        let out = extend_item(&item, Point::new(20.0, 28.0), &wall).unwrap();
        match out {
            CadItem::Polyline { points, .. } => {
                assert_eq!(points.len(), 3);
                assert!(points[2].dist(Point::new(20.0, 80.0)) < 1e-9);
            }
            o => panic!("{o:?}"),
        }
        assert!(
            extend_item(&item, Point::new(0.0, 1.0), &wall).is_err(),
            "the first segment runs along the boundary's direction"
        );
    }

    #[test]
    fn insert_point_splits_a_straight_edge_and_an_arc_edge() {
        let mut cx = cx();
        let id = rect(&mut cx, 0.0, 0.0, 100.0, 100.0);
        let q = insert_point(&mut cx, id, Point::new(50.0, 1.0)).unwrap();
        assert!(q.dist(Point::new(50.0, 0.0)) < 1e-9);
        match cad_item(&cx, id).unwrap() {
            CadItem::Polyline { points, .. } => assert_eq!(points.len(), 5),
            o => panic!("{o:?}"),
        }
        assert_eq!(cx.undo_label(), Some("Insert Point"));
        // An arc edge: make the polyline a half disc through Logical.
        let mut l = arcs::logical_of(&cx, id).unwrap();
        // Edge 0 now runs (0,0) -> (50,0): make it an arc with a bulge.
        l.bulge[0] = Some(0.5);
        arcs::replace_polyline(&mut cx, id, &l);
        let l = arcs::logical_of(&cx, id).unwrap();
        let (c, r) = arcs::arc_center(l.pts[0], l.pts[1], 0.5).unwrap();
        let apex = {
            let theta = arcs::sweep_of(0.5);
            let a0 = l.pts[0].sub(c).angle();
            let ang = a0 + theta / 2.0;
            Point::new(c.x + r * ang.cos(), c.y + r * ang.sin())
        };
        let q = insert_point(&mut cx, id, apex).unwrap();
        assert!((q.dist(c) - r).abs() < 1e-6, "on the arc");
        let l2 = arcs::logical_of(&cx, id).unwrap();
        assert_eq!(l2.pts.len(), l.pts.len() + 1);
        let (b1, b2) = (l2.bulge[0].unwrap(), l2.bulge[1].unwrap());
        assert!((b1 - b2).abs() < 1e-6, "split in the middle: equal halves");
        assert!((arcs::sweep_of(b1) + arcs::sweep_of(b2) - arcs::sweep_of(0.5)).abs() < 1e-9);
        // Clicking away from every edge does nothing.
        assert!(insert_point(&mut cx, id, Point::new(500.0, 500.0)).is_err());
    }

    #[test]
    fn multiple_copy_makes_n_copies_of_walls_and_cad_in_one_step() {
        let mut cx = cx();
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let a = rect(&mut cx, 0.0, 20.0, 10.0, 10.0);
        let w = cx.floor().walls[0].id;
        cx.selection.items = vec![ObjectRef::Wall(w), ObjectRef::Cad(a)];
        let msg = multiple_copy(&mut cx, &MultipleCopy::polar(3, 50.0, 90.0)).unwrap();
        assert!(msg.contains("3 copies"), "{msg}");
        assert_eq!(cx.floor().walls.len(), 4);
        assert_eq!(cx.floor().cad.len(), 4);
        let mut ys: Vec<f64> = cx.floor().walls.iter().map(|w| w.start.y).collect();
        ys.sort_by(f64::total_cmp);
        assert_eq!(ys, vec![0.0, 50.0, 100.0, 150.0]);
        assert_eq!(cx.undo_label(), Some("Multiple Copy"));
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 1);
        assert_eq!(cx.floor().cad.len(), 1);
        // The last copy at a given total.
        cx.selection.items = vec![ObjectRef::Wall(w)];
        multiple_copy(&mut cx, &MultipleCopy::spread(4, Point::new(0.0, 200.0))).unwrap();
        let mut ys: Vec<f64> = cx.floor().walls.iter().map(|w| w.start.y).collect();
        ys.sort_by(f64::total_cmp);
        assert_eq!(ys, vec![0.0, 50.0, 100.0, 150.0, 200.0]);
        // Refusals.
        assert!(multiple_copy(
            &mut cx,
            &MultipleCopy {
                count: 0,
                ..MultipleCopy::default()
            }
        )
        .is_err());
        assert!(multiple_copy(
            &mut cx,
            &MultipleCopy {
                count: 2,
                step: Point::ZERO,
                turn_deg: 0.0
            }
        )
        .is_err());
        assert!(multiple_copy(
            &mut cx,
            &MultipleCopy {
                count: MAX_COPIES + 1,
                ..MultipleCopy::default()
            }
        )
        .is_err());
    }

    #[test]
    fn drawing_group_commands_order_the_selection() {
        let mut cx = cx();
        let a = rect(&mut cx, 0.0, 0.0, 10.0, 10.0);
        let b = rect(&mut cx, 5.0, 5.0, 10.0, 10.0);
        select(&mut cx, &[a]);
        assert_eq!(drawing_group_order(&mut cx, true).unwrap(), 1);
        assert_eq!(cx.undo_label(), Some("Bring to Front"));
        let order: Vec<Id> = cx.floor().cad_draw_order().iter().map(|c| c.id).collect();
        assert_eq!(order, vec![b, a]);
        assert!(
            drawing_group_order(&mut cx, true).is_err(),
            "already in front"
        );
        assert_eq!(drawing_group_order(&mut cx, false).unwrap(), 1);
        select(&mut cx, &[b]);
        assert_eq!(set_drawing_group(&mut cx, Some(5)).unwrap(), 1);
        assert_eq!(set_drawing_group(&mut cx, None).unwrap(), 1);
        assert!(set_drawing_group(&mut cx, None).is_err());
    }

    #[test]
    fn plan_footprint_runs_along_the_outer_wall_faces() {
        let mut cx = cx();
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        // An interior partition does not change the outline.
        cx.project.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 180.0),
            4.0,
            96.0,
            WallKind::Interior,
        );
        let area = plan_footprint(&mut cx).unwrap();
        // The wall centerlines enclose 240 x 180; the outer faces are 3" out.
        let want = 246.0 * 186.0 / 144.0;
        assert!((area - want).abs() < 0.5, "{area} vs {want}");
        let polys: Vec<_> = cx
            .floor()
            .cad
            .iter()
            .filter(|c| matches!(c.item, CadItem::Polyline { .. }))
            .collect();
        assert_eq!(polys.len(), 1);
        assert_eq!(cx.undo_label(), Some("Plan Footprint"));
        // No walls: nothing to do.
        let mut empty = cx_empty();
        assert!(plan_footprint(&mut empty).is_none());
    }

    fn cx_empty() -> EditorContext {
        cx()
    }

    #[test]
    fn the_edit_toolbar_offers_what_the_selection_allows() {
        let mut cx = cx();
        let a = rect(&mut cx, 0.0, 0.0, 10.0, 10.0);
        let b = rect(&mut cx, 5.0, 5.0, 10.0, 10.0);
        select(&mut cx, &[a]);
        let labels: Vec<&str> = edit_actions(&cx).iter().map(|e| e.label).collect();
        assert!(labels.contains(&"Multiple Copy") && labels.contains(&"Insert Point"));
        assert!(
            !labels.contains(&"Polyline Union"),
            "one shape has nothing to combine with"
        );
        select(&mut cx, &[a, b]);
        let labels: Vec<&str> = edit_actions(&cx).iter().map(|e| e.label).collect();
        for want in [
            "Polyline Union",
            "Polyline Subtract",
            "Polyline Intersect",
            "Trim to Boundary",
            "Extend to Boundary",
        ] {
            assert!(labels.contains(&want), "{want} in {labels:?}");
        }
        cx.selection.items.clear();
        assert!(edit_actions(&cx).is_empty());
    }
}
