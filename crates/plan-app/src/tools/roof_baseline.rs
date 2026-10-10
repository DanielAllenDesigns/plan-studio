//! Roof Baseline Polylines, and what Build Roof does around them (manual pp.
//! 826 to 829, 850 to 852; RF-62, RF-68, RF-70, RF-71, RF-125..RF-127).
//!
//! # Roof Baseline Polylines
//!
//! A roof baseline polyline is a closed CAD polyline (a [`CadItem::Polyline`]
//! on the layer [`LAYER`], so it selects, moves, stretches and deletes like
//! any CAD object and its corners can be added and moved with the polyline
//! handles) plus a [`RoofBaseline`] record kept in `Floor.roofs` as
//! `{"kind": "baseline", "id": <polyline id>, ...}`: the Baseline Height and the
//! roof directive of every edge. A record whose polyline is gone is ignored.
//! `roof_view` leaves the record alone (it passes through as an unknown kind).
//!
//! * **Make Roof Baseline Polylines** (a Build Roof switch, or
//!   [`make_polylines`]) deletes the roof and makes one polyline per roof
//!   (one per height) along the outside of the exterior walls, carrying the
//!   walls' roof directives ([`make_from_walls`]).
//! * The tool ([`RoofBaselineTool`]) draws a polyline by hand: click the
//!   corners, then click the first corner or press Enter.
//! * **Use Existing Roof Baselines** (a Build Roof switch) builds the planes
//!   from the polylines instead of the walls ([`baseline_planes`]).
//!
//! The directive of each edge shows along it ([`draw_baselines`]): V, G, K or
//! L and the pitch, or `(vert)` when no plane slopes toward the edge.
//!
//! # Rebuilding around kept planes
//!
//! [`retained`] says which planes a rebuild keeps (the Retain Manually Drawn
//! and Retain Edited Automatic Roof Planes switches) and
//! [`drop_replaced_records`] drops a new plane where a kept one stands.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::roof_view::{self, RoofPlaneRecord, RoofSettings};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Stroke};
use plan_core::cad::{CadItem, CadObject};
use plan_core::geometry::{dist_to_segment, polygon_area, Point};
use plan_core::layers::Layer;
use plan_core::{Floor, Id, Project, WallKind};
use plan_roof::{
    build_baseline_roof, directive_text, BaselineEdge, BuildSwitches, JoinLock, RoofBaseline,
};
use serde_json::{json, Value};

/// The layer roof baseline polylines are drawn on.
pub const LAYER: &str = "Roofs, Baseline Polylines";
/// `kind` of the record in `Floor.roofs`.
const KIND: &str = "baseline";
/// A click this close to the first corner (screen pixels) closes the polyline.
const CLOSE_PX: f64 = 10.0;
/// Corners closer than this (inches) are one corner.
const SAME_POINT: f64 = 1.0;

/// Custom command ids of the Edit toolbar buttons.
pub mod cmd {
    pub const SPEC: &str = "roof_baseline.spec";
    pub const BUILD: &str = "roof_baseline.build";
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

/// One roof baseline polyline of a floor.
#[derive(Clone, Debug, PartialEq)]
pub struct Baseline {
    /// Id of the CAD polyline (and of the record).
    pub id: Id,
    /// Corners, counter-clockwise.
    pub points: Vec<Point>,
    pub spec: RoofBaseline,
}

impl Baseline {
    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.points.len()
    }

    /// Edge `i` as a segment.
    pub fn edge(&self, i: usize) -> (Point, Point) {
        let n = self.points.len();
        (self.points[i % n], self.points[(i + 1) % n])
    }
}

fn is_record(v: &Value) -> bool {
    v.get("kind").and_then(Value::as_str) == Some(KIND)
}

/// The roof baseline polylines of `floor`: the closed CAD polylines that have
/// a record. The record's edges are brought to the polyline's corner count.
pub fn baselines(floor: &Floor) -> Vec<Baseline> {
    let mut out = Vec::new();
    for v in floor.roofs.iter().filter(|v| is_record(v)) {
        let Some(id) = v.get("id").and_then(Value::as_u64) else {
            continue;
        };
        let Ok(mut spec) = serde_json::from_value::<RoofBaseline>(v.clone()) else {
            continue;
        };
        let points = floor.cad.iter().find_map(|c| match &c.item {
            CadItem::Polyline {
                points,
                closed: true,
            } if c.id == id && points.len() >= 3 => Some(points.clone()),
            _ => None,
        });
        if let Some(points) = points {
            spec.fit(points.len());
            out.push(Baseline { id, points, spec });
        }
    }
    out
}

/// The baseline polyline `id` of `floor`.
pub fn baseline(floor: &Floor, id: Id) -> Option<Baseline> {
    baselines(floor).into_iter().find(|b| b.id == id)
}

/// Is CAD object `id` a roof baseline polyline?
pub fn is_baseline(floor: &Floor, id: Id) -> bool {
    baseline(floor, id).is_some()
}

fn put_record(floor: &mut Floor, id: Id, spec: &RoofBaseline) {
    floor
        .roofs
        .retain(|v| !(is_record(v) && v.get("id").and_then(Value::as_u64) == Some(id)));
    if let Ok(Value::Object(mut m)) = serde_json::to_value(spec) {
        m.insert("kind".into(), json!(KIND));
        m.insert("id".into(), json!(id));
        floor.roofs.push(Value::Object(m));
    }
}

fn ensure_layer(project: &mut Project) {
    if project.layers.get(LAYER).is_none() {
        project.layers.add(Layer::new(LAYER, [150, 90, 40], 18));
    }
}

/// Adds a baseline polyline over `points` (either winding) to floor `fi`.
/// `spec.edges` follow `points`. `None` for fewer than three corners, a
/// polygon with no area or one that crosses itself.
pub fn add_polyline(
    project: &mut Project,
    fi: usize,
    points: &[Point],
    mut spec: RoofBaseline,
) -> Option<Id> {
    let n = points.len();
    if fi >= project.floors.len()
        || n < 3
        || polygon_area(points).abs() < 36.0
        || roof_view::polygon_self_intersects(points)
    {
        return None;
    }
    spec.fit(n);
    let mut pts = points.to_vec();
    if polygon_area(&pts) < 0.0 {
        // Reversed, edge k is the old edge (n - 2 - k) mod n.
        pts.reverse();
        let old = spec.edges.clone();
        spec.edges = (0..n).map(|k| old[(2 * n - 2 - k) % n].clone()).collect();
    }
    ensure_layer(project);
    let id = project.alloc_id();
    let f = &mut project.floors[fi];
    f.cad.push(CadObject {
        id,
        layer: LAYER.to_string(),
        item: CadItem::Polyline {
            points: pts,
            closed: true,
        },
    });
    put_record(f, id, &spec);
    Some(id)
}

/// Removes every roof baseline polyline of floor `fi` (the polylines and the
/// records, and records whose polyline is gone). Returns how many polylines.
pub fn remove_all(project: &mut Project, fi: usize) -> usize {
    let Some(f) = project.floors.get_mut(fi) else {
        return 0;
    };
    let ids: Vec<Id> = baselines(f).iter().map(|b| b.id).collect();
    f.cad.retain(|c| !ids.contains(&c.id));
    f.roofs.retain(|v| !is_record(v));
    ids.len()
}

/// Replaces the specification of polyline `id` of floor `fi`.
pub fn set_spec(project: &mut Project, fi: usize, id: Id, mut spec: RoofBaseline) -> bool {
    let Some(b) = project.floors.get(fi).and_then(|f| baseline(f, id)) else {
        return false;
    };
    spec.fit(b.points.len());
    put_record(&mut project.floors[fi], id, &spec);
    true
}

// ---------------------------------------------------------------------------
// Make Roof Baseline Polylines / Use Existing Roof Baselines
// ---------------------------------------------------------------------------

/// Make Roof Baseline Polylines for the build floor `fi` and the wings under
/// it: deletes the baselines there and makes one polyline per roof along the
/// outside of the exterior walls, with the walls' roof directives. Returns how
/// many were made. Used by `roof_view::rebuild` for the Build Roof switch.
pub fn make_from_walls(
    project: &mut Project,
    fi: usize,
    s: &RoofSettings,
) -> Result<usize, String> {
    let sources = roof_view::baseline_sources(project, fi, s)?;
    for g in 0..=fi {
        remove_all(project, g);
    }
    let mut made = 0;
    for src in &sources {
        if let Some((outline, spec)) =
            plan_roof::baseline_from_footprint(&src.fp, &src.specs, &src.half, src.height)
        {
            if add_polyline(project, src.floor, &outline, spec).is_some() {
                made += 1;
            }
        }
    }
    if made == 0 {
        return Err("The exterior walls do not enclose an area".to_string());
    }
    Ok(made)
}

/// The planes (automatic, ids not yet given), whether plan-roof had to
/// approximate, and the Dutch gable faces of the roof built from the baseline
/// polylines of floor `g`. Empty when the floor has none.
#[allow(clippy::type_complexity)]
pub fn planes_on_floor(
    project: &Project,
    g: usize,
    s: &RoofSettings,
) -> Result<(Vec<RoofPlaneRecord>, bool, Vec<Vec<[f64; 3]>>), String> {
    let floor = &project.floors[g];
    let mut planes = Vec::new();
    let mut approximate = false;
    let mut faces = Vec::new();
    for b in baselines(floor) {
        let elevation = floor.elevation + b.spec.height;
        let (roof, f) = build_baseline_roof(&b.points, &b.spec, elevation);
        approximate |= roof.approximate;
        faces.extend(f);
        let n = b.points.len();
        for pl in roof.planes {
            let k = pl.source_edge % n;
            let mut r = RoofPlaneRecord::new(0, pl.polygon3d, pl.pitch_in_12, pl.baseline);
            r.auto = true;
            r.source = Some(b.edge(k));
            r.overhang = b.spec.edges.get(k).map_or(0.0, |e| e.overhang);
            r.material = s.material.clone();
            planes.push(r);
        }
    }
    Ok((planes, approximate, faces))
}

/// The roof of the build floor `fi` from its roof baseline polylines, with
/// ids. An error when the floor has none.
#[allow(clippy::type_complexity)]
pub fn baseline_planes(
    project: &mut Project,
    fi: usize,
    s: &RoofSettings,
) -> Result<(Vec<RoofPlaneRecord>, bool, Vec<Vec<[f64; 3]>>), String> {
    let (mut planes, approximate, faces) = planes_on_floor(project, fi, s)?;
    if planes.is_empty() {
        return Err(format!(
            "There are no Roof Baseline Polylines on {}: use Make Roof Baseline Polylines first",
            project.floors[fi].name
        ));
    }
    for r in &mut planes {
        r.id = project.alloc_id();
    }
    Ok((planes, approximate, faces))
}

// ---------------------------------------------------------------------------
// Rebuilding around kept planes
// ---------------------------------------------------------------------------

/// Does a rebuild keep plane `p`? Automatic planes are replaced; a plane the
/// user edited (it still has its source edge) stays with Retain Edited
/// Automatic Roof Planes; a plane drawn by hand stays with Retain Manually
/// Drawn Roof Planes.
pub fn retained(p: &RoofPlaneRecord, sw: &BuildSwitches) -> bool {
    if p.auto {
        return false;
    }
    if p.source.is_some() {
        sw.retain_edited
    } else {
        sw.retain_manual
    }
}

/// `new` without the planes a plane of `kept` stands in for: coplanar with it
/// and overlapping by at least half the area of either.
pub fn drop_replaced_records(
    kept: &[RoofPlaneRecord],
    new: Vec<RoofPlaneRecord>,
) -> Vec<RoofPlaneRecord> {
    if kept.is_empty() {
        return new;
    }
    let kept: Vec<_> = kept.iter().map(|r| r.to_roof_plane(0)).collect();
    new.into_iter()
        .filter(|n| {
            let p = n.to_roof_plane(0);
            !kept
                .iter()
                .any(|k| plan_roof::retained_plane_replaces(k, &p))
        })
        .collect()
}

/// The plan edges that are hips between two sections of the roof over a
/// curved wall: neighbouring planes of one pitch whose eaves turn by no more
/// than the segment angle. Show All Ridges off leaves them out.
pub fn facet_hips(planes: &[RoofPlaneRecord], segment_angle: f64) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    for a in planes.iter().filter(|p| p.auto) {
        let da = a.baseline.1.sub(a.baseline.0).normalized();
        for b in planes.iter().filter(|p| p.auto) {
            if a.id == b.id
                || a.baseline.1.dist(b.baseline.0) > 1.0
                || (a.pitch - b.pitch).abs() > 1e-6
            {
                continue;
            }
            let db = b.baseline.1.sub(b.baseline.0).normalized();
            let turn = da.cross(db).atan2(da.dot(db)).abs().to_degrees();
            if turn < 1e-3 || turn > segment_angle + 0.5 {
                continue;
            }
            let (pa, pb) = (a.plan_polygon(), b.plan_polygon());
            let na = pa.len();
            for i in 0..na {
                let e = (pa[i], pa[(i + 1) % na]);
                let touches = e.0.dist(a.baseline.1) < 1.0 || e.1.dist(a.baseline.1) < 1.0;
                let eave = e.0.dist(a.baseline.0) < 1.0 && e.1.dist(a.baseline.1) < 1.0;
                let shared = (0..pb.len()).any(|j| {
                    let f = (pb[j], pb[(j + 1) % pb.len()]);
                    (e.0.dist(f.0) < 1.0 && e.1.dist(f.1) < 1.0)
                        || (e.0.dist(f.1) < 1.0 && e.1.dist(f.0) < 1.0)
                });
                if touches && !eave && shared {
                    out.push(e);
                }
            }
        }
    }
    out
}

/// Does joining curved plane `a` need the Join Curved Roof Plane choice?
pub fn join_asks(cx: &EditorContext, a: Id) -> bool {
    roof_view::load(cx.floor())
        .plane(a)
        .is_some_and(|p| p.curved.is_some_and(|c| !c.is_straight()))
}

// ---------------------------------------------------------------------------
// Commands (one undo step each)
// ---------------------------------------------------------------------------

/// The settings Build Roof starts from: those stored on a floor, else the
/// defaults.
fn current_settings(cx: &EditorContext) -> RoofSettings {
    cx.project
        .floors
        .iter()
        .find_map(|f| roof_view::load(f).settings)
        .unwrap_or_else(|| RoofSettings::from_defaults(&cx.defaults))
}

/// The edge a hand-drawn polyline starts with: the roof's pitch and overhang.
pub fn default_edge(cx: &EditorContext) -> BaselineEdge {
    let s = current_settings(cx);
    BaselineEdge {
        pitch: s.pitch,
        overhang: s.overhang,
        ..BaselineEdge::default()
    }
}

/// The Baseline Height a hand-drawn polyline starts with: the top of the
/// tallest exterior wall of the floor (the floor's ceiling height without
/// one), plus Raise Roof Off Plate.
pub fn default_height(cx: &EditorContext) -> f64 {
    let f = cx.floor();
    let top = f
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .map(|w| w.height)
        .fold(0.0, f64::max);
    let top = if top > 0.0 { top } else { f.ceiling_height };
    top + current_settings(cx).raise_off_plate
}

/// Draws a roof baseline polyline over `points` on the active floor. One undo
/// step. The polyline is selected.
pub fn draw_polyline(cx: &mut EditorContext, points: &[Point]) -> Option<Id> {
    let spec = RoofBaseline::uniform(default_height(cx), points.len(), default_edge(cx));
    cx.begin_change("Roof Baseline Polyline");
    let fl = cx.floor;
    match add_polyline(&mut cx.project, fl, points, spec) {
        Some(id) => {
            cx.mark_dirty();
            cx.refresh();
            cx.selection.set(ObjectRef::Cad(id));
            Some(id)
        }
        None => {
            cx.cancel_change();
            None
        }
    }
}

/// Roof Baseline Specification OK: replaces the specification of polyline
/// `id`. One undo step.
pub fn apply_spec(cx: &mut EditorContext, fi: usize, id: Id, spec: RoofBaseline) -> bool {
    if cx
        .project
        .floors
        .get(fi)
        .and_then(|f| baseline(f, id))
        .is_none()
    {
        return false;
    }
    cx.begin_change("Roof Baseline Specification");
    let ok = set_spec(&mut cx.project, fi, id, spec);
    if ok {
        cx.mark_dirty();
    } else {
        cx.cancel_change();
    }
    ok
}

/// Make Roof Baseline Polylines the way the Build Roof dialog does with the
/// switch checked: deletes the roof and makes the polylines. One undo step.
/// The status line tells how many.
pub fn make_polylines(cx: &mut EditorContext) -> bool {
    let mut s = current_settings(cx);
    let fi = roof_view::build_floor(&cx.project, s.ignore_top_floor, cx.floor);
    s.build_planes = false;
    s.switches.make_baselines = true;
    cx.begin_change("Make Roof Baseline Polylines");
    match roof_view::rebuild(&mut cx.project, fi, s, false) {
        Ok(_) => {
            let n: usize = (0..=fi)
                .map(|g| baselines(&cx.project.floors[g]).len())
                .sum();
            cx.mark_dirty();
            cx.refresh();
            cx.status = format!(
                "Made {n} roof baseline polyline{}",
                if n == 1 { "" } else { "s" }
            );
            true
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = format!("Make Roof Baseline Polylines: {e}");
            false
        }
    }
}

/// Build Roof Planes with Use Existing Roof Baselines. One undo step.
pub fn build_from_baselines(cx: &mut EditorContext) -> bool {
    let mut s = current_settings(cx);
    let fi = roof_view::build_floor(&cx.project, s.ignore_top_floor, cx.floor);
    s.build_planes = true;
    s.switches.make_baselines = false;
    s.switches.use_existing_baselines = true;
    cx.begin_change("Build Roof");
    match roof_view::rebuild(&mut cx.project, fi, s, false) {
        Ok(rep) => {
            cx.mark_dirty();
            cx.refresh();
            cx.status = format!(
                "Built {} roof plane{} from the roof baselines",
                rep.planes,
                if rep.planes == 1 { "" } else { "s" }
            );
            true
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = format!("Build Roof: {e}");
            false
        }
    }
}

/// Turns roof plane `id` of the active floor into a curved plane (or, with
/// `None`, straightens it). One undo step; the plane becomes edited.
pub fn set_curved(cx: &mut EditorContext, id: Id, curved: Option<plan_roof::CurvedSpec>) -> bool {
    let fl = cx.floor;
    let mut set = roof_view::load(cx.floor());
    let Some(rec) = set.plane_mut(id) else {
        return false;
    };
    cx.begin_change(if curved.is_some() {
        "Curved Roof Plane"
    } else {
        "Straighten Roof Plane"
    });
    rec.curved = curved.map(|c| c.retarget(rec.pitch));
    rec.auto = false;
    roof_view::store(&mut cx.project, fl, &mut set);
    cx.mark_dirty();
    true
}

/// Join Curved Roof Plane OK: joins edge `edge` of plane `a` to plane `b`
/// keeping what `lock` says. One undo step.
pub fn join_curved(
    cx: &mut EditorContext,
    fi: usize,
    a: Id,
    edge: usize,
    b: Id,
    lock: JoinLock,
) -> bool {
    cx.begin_change("Join Roof Planes");
    match roof_view::join_planes_record_locked(&mut cx.project, fi, a, edge, b, lock) {
        Ok(()) => {
            cx.mark_dirty();
            cx.status = "Roof planes joined".into();
            true
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = format!("Join Roof Planes: {e}");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Edit toolbar
// ---------------------------------------------------------------------------

/// The baseline polyline the selection is exactly one of.
pub fn selected_baseline(cx: &EditorContext) -> Option<Id> {
    match cx.selection.single() {
        Some(ObjectRef::Cad(id)) if is_baseline(cx.floor(), id) => Some(id),
        _ => None,
    }
}

fn button(id: &'static str, label: &'static str) -> EditAction {
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled: true,
    }
}

/// The Edit toolbar buttons for the current selection: with a roof baseline
/// polyline selected, Open Object (Roof Baseline Specification) and Build
/// Roof from Baselines. `extra_edit_actions` lists them for every tool
/// (docs/integration-queue.md).
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    if selected_baseline(cx).is_some() {
        v.push(button(cmd::SPEC, "Roof Baseline Specification"));
        v.push(button(cmd::BUILD, "Build Roof from Roof Baselines"));
    }
    v
}

/// Runs an Edit toolbar command of this module. False when `id` is not one.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        cmd::SPEC => {
            if let Some(b) = selected_baseline(cx) {
                crate::dialogs::roof_baseline::open_spec(cx, b, None);
            }
        }
        cmd::BUILD => {
            build_from_baselines(cx);
        }
        _ => return false,
    }
    true
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn layer_color(cx: &EditorContext) -> Color32 {
    cx.layers()
        .get(LAYER)
        .map_or(Color32::from_rgb(150, 90, 40), |l| {
            Color32::from_rgb(l.color[0], l.color[1], l.color[2])
        })
}

/// The directive letters and pitch along every edge of the active floor's
/// roof baseline polylines, with a tick toward the interior on each edge a
/// plane rises from. The polylines themselves are CAD objects and draw with
/// the rest of the CAD.
pub fn draw_baselines(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let floor = cx.floor();
    if floor.roofs.is_empty() || !cx.layers().is_visible(LAYER) {
        return;
    }
    let color = layer_color(cx);
    for b in baselines(floor) {
        let n = b.edge_count();
        for i in 0..n {
            let (p, q) = b.edge(i);
            let mid = Point::lerp(p, q, 0.5);
            let inward = q.sub(p).normalized().perp();
            let edge = &b.spec.edges[i];
            if edge.rises() {
                let tick = [
                    cam.world_to_screen(mid),
                    cam.world_to_screen(mid.add(inward.scale(8.0 / cam.px_per_in))),
                ];
                painter.line_segment(tick, Stroke::new(1.5_f32, color));
            }
            let at = cam.world_to_screen(mid.add(inward.scale(18.0 / cam.px_per_in)));
            painter.text(
                at,
                Align2::CENTER_CENTER,
                directive_text(edge),
                FontId::proportional(11.0),
                color,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The tool
// ---------------------------------------------------------------------------

/// Build > Roof > Roof Baseline Polyline (a Plan Studio extra: Chief draws
/// baselines from Build Roof and edits them as CAD polylines, DECISIONS RB2).
#[derive(Default)]
pub struct RoofBaselineTool {
    /// Corners clicked so far.
    pts: Vec<Point>,
    hover: Option<Point>,
}

impl RoofBaselineTool {
    /// The corners clicked so far.
    pub fn pending(&self) -> &[Point] {
        &self.pts
    }

    fn close_radius(cx: &EditorContext) -> f64 {
        CLOSE_PX / cx.px_per_in.max(1e-6)
    }

    fn finish(&mut self, cx: &mut EditorContext) -> ToolResult {
        let mut pts = std::mem::take(&mut self.pts);
        self.hover = None;
        while pts.len() > 1 && pts[pts.len() - 1].dist(pts[pts.len() - 2]) < SAME_POINT {
            pts.pop();
        }
        if pts.len() >= 2 && pts[0].dist(pts[pts.len() - 1]) < SAME_POINT {
            pts.pop();
        }
        if pts.len() < 3 {
            cx.status = "A roof baseline polyline needs at least three corners".into();
            return ToolResult::consumed();
        }
        match draw_polyline(cx, &pts) {
            Some(_) => {
                cx.status = "Roof baseline polyline drawn: open it to set its roof options".into();
                ToolResult::committed("Roof Baseline Polyline")
            }
            None => {
                cx.status = "The polyline is too small or crosses itself".into();
                ToolResult::consumed()
            }
        }
    }

    /// The baseline polyline whose edge is within `tol` of `at`.
    fn hit(cx: &EditorContext, at: Point, tol: f64) -> Option<(Id, usize)> {
        baselines(cx.floor()).into_iter().find_map(|b| {
            (0..b.edge_count()).find_map(|i| {
                let (p, q) = b.edge(i);
                (dist_to_segment(at, p, q) <= tol).then_some((b.id, i))
            })
        })
    }
}

impl Tool for RoofBaselineTool {
    fn id(&self) -> ToolId {
        ToolId::RoofBaseline
    }

    fn name(&self) -> &'static str {
        "Roof Baseline Polyline"
    }

    fn hint(&self) -> String {
        if self.pts.is_empty() {
            "Roof Baseline Polyline: click the corners of the roof; click an existing one to select it"
                .into()
        } else {
            "Click the next corner; click the first corner or press Enter to close, Esc cancels"
                .into()
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.pts.clear();
        self.hover = None;
    }

    fn pointer_move(&mut self, _cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.hover = Some(p.snapped);
        ToolResult {
            repaint: !self.pts.is_empty(),
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let at = p.snapped;
        if self.pts.is_empty() {
            // A click on an existing baseline selects it.
            let tol = cx.pick_tol();
            if let Some((id, _)) = Self::hit(cx, p.world, tol) {
                cx.selection.set(ObjectRef::Cad(id));
                cx.status = "Roof baseline polyline selected".into();
                return ToolResult::consumed();
            }
        }
        if self.pts.len() >= 3 && at.dist(self.pts[0]) <= Self::close_radius(cx) {
            return self.finish(cx);
        }
        if self.pts.last().is_some_and(|l| l.dist(at) < SAME_POINT) {
            return ToolResult::consumed();
        }
        self.pts.push(at);
        ToolResult::consumed()
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if self.pts.is_empty() {
            if let Some((id, edge)) = Self::hit(cx, p.world, cx.pick_tol()) {
                cx.selection.set(ObjectRef::Cad(id));
                crate::dialogs::roof_baseline::open_spec(cx, id, Some(edge));
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        self.finish(cx)
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Escape) {
            let had = !self.pts.is_empty();
            self.pts.clear();
            self.hover = None;
            return if had {
                ToolResult::consumed()
            } else {
                ToolResult::ignored()
            };
        }
        if k.is(Key::Enter) && !self.pts.is_empty() {
            return self.finish(cx);
        }
        if k.is(Key::Backspace) && !self.pts.is_empty() {
            self.pts.pop();
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        if self.pts.is_empty() {
            return;
        }
        let mut pts: Vec<Pos2> = self.pts.iter().map(|p| cam.world_to_screen(*p)).collect();
        if let Some(h) = self.hover {
            pts.push(cam.world_to_screen(h));
        }
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(1.5_f32, cx.palette.selection),
        ));
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        let mut v = cx.common_edit_actions();
        v.extend(edit_actions(cx));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_roof::BaselineOption;

    fn pts(v: &[(f64, f64)]) -> Vec<Point> {
        v.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    #[test]
    fn a_polyline_and_its_record_round_trip_through_the_floor() {
        let mut cx = cx();
        let id = draw_polyline(
            &mut cx,
            &pts(&[(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)]),
        )
        .unwrap();
        let b = baseline(cx.floor(), id).unwrap();
        assert_eq!(b.points.len(), 4);
        assert_eq!(b.spec.edges.len(), 4);
        let obj = cx.floor().cad.iter().find(|c| c.id == id).unwrap();
        assert_eq!(obj.layer, LAYER);
        assert!(cx.project.layers.get(LAYER).is_some());
        // One undo step takes the polyline and its record away.
        assert_eq!(cx.undo().as_deref(), Some("Roof Baseline Polyline"));
        assert!(baselines(cx.floor()).is_empty());
        assert!(cx.floor().roofs.iter().all(|v| !is_record(v)));
    }

    #[test]
    fn a_clockwise_polyline_keeps_each_directive_on_its_edge() {
        let mut cx = cx();
        let mut spec = RoofBaseline::uniform(96.0, 4, BaselineEdge::default());
        // Clockwise: (0,0) -> (0,120) -> (240,120) -> (240,0). Edge 0 is the
        // west side.
        spec.edges[0].option = BaselineOption::FullGable;
        let id = add_polyline(
            &mut cx.project,
            0,
            &pts(&[(0.0, 0.0), (0.0, 120.0), (240.0, 120.0), (240.0, 0.0)]),
            spec,
        )
        .unwrap();
        let b = baseline(cx.floor(), id).unwrap();
        assert!(polygon_area(&b.points) > 0.0);
        for i in 0..4 {
            let (p, q) = b.edge(i);
            let west = p.x == 0.0 && q.x == 0.0;
            assert_eq!(
                b.spec.edges[i].option == BaselineOption::FullGable,
                west,
                "edge {i}"
            );
        }
    }

    #[test]
    fn a_crossing_or_tiny_polyline_is_refused() {
        let mut cx = cx();
        assert!(draw_polyline(
            &mut cx,
            &pts(&[(0.0, 0.0), (100.0, 100.0), (100.0, 0.0), (0.0, 100.0)])
        )
        .is_none());
        assert!(draw_polyline(&mut cx, &pts(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0)])).is_none());
        assert!(!cx.can_undo(), "a refused polyline leaves no undo step");
    }

    #[test]
    fn editing_the_spec_is_one_undo_step() {
        let mut cx = cx();
        let id = draw_polyline(
            &mut cx,
            &pts(&[(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)]),
        )
        .unwrap();
        let mut spec = baseline(cx.floor(), id).unwrap().spec;
        spec.edges[2].against_wall = true;
        spec.height = 110.0;
        assert!(apply_spec(&mut cx, 0, id, spec));
        let b = baseline(cx.floor(), id).unwrap();
        assert!(b.spec.edges[2].against_wall && b.spec.height == 110.0);
        assert_eq!(cx.undo().as_deref(), Some("Roof Baseline Specification"));
        assert!(!baseline(cx.floor(), id).unwrap().spec.edges[2].against_wall);
    }

    #[test]
    fn retention_follows_the_switches() {
        let mut manual = RoofPlaneRecord::new(
            1,
            vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 5.0, -10.0]],
            6.0,
            (Point::ZERO, Point::new(10.0, 0.0)),
        );
        let mut edited = manual.clone();
        edited.source = Some((Point::ZERO, Point::new(10.0, 0.0)));
        let mut auto = manual.clone();
        auto.auto = true;
        manual.auto = false;
        let both = BuildSwitches::default();
        assert!(retained(&manual, &both) && retained(&edited, &both) && !retained(&auto, &both));
        let none = BuildSwitches {
            retain_manual: false,
            retain_edited: false,
            ..both.clone()
        };
        assert!(!retained(&manual, &none) && !retained(&edited, &none));
        let only_edited = BuildSwitches {
            retain_manual: false,
            ..both
        };
        assert!(!retained(&manual, &only_edited) && retained(&edited, &only_edited));
    }

    #[test]
    fn the_tool_closes_a_polyline_on_the_first_corner() {
        let mut cx = cx();
        let mut t = RoofBaselineTool::default();
        for p in [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)] {
            let ev = PointerEvent::at(&cx, Point::new(p.0, p.1));
            t.pointer_down(&mut cx, ev);
        }
        assert_eq!(t.pending().len(), 4);
        let ev = PointerEvent::at(&cx, Point::new(0.0, 0.0));
        let r = t.pointer_down(&mut cx, ev);
        assert_eq!(r.commit.as_deref(), Some("Roof Baseline Polyline"));
        assert!(t.pending().is_empty());
        assert_eq!(baselines(cx.floor()).len(), 1);
        // Esc drops a pending polyline.
        let ev = PointerEvent::at(&cx, Point::new(500.0, 500.0));
        t.pointer_down(&mut cx, ev);
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(t.pending().is_empty());
    }
}
