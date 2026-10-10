//! Gable/Roof Line objects (manual pp. 861 to 864; RF-44, RF-133..RF-136).
//!
//! A Gable/Roof Line is a line drawn exactly parallel to an exterior wall and
//! within ten feet of its Main Layer. Its length is the width of a gable at
//! the wall; its two planes have the line's own pitch and overhang. The line
//! is kept in the plan, as a record of `Floor.roofs`
//! (`{"kind": "gable_line", "id", "a", "b", "pitch", "overhang", ...}`, which
//! `roof_view` passes through untouched), until it is deleted. Build Roof
//! turns every line into a gable ([`apply_stored`]); the next Build Roof
//! makes them again, so the lines outlive any number of rebuilds.
//!
//! * [`draw_line`] adds a line between two points (one undo step, "Gable/Roof
//!   Line"); a line that is not parallel to a wall, too short or more than
//!   ten feet from the wall is refused with Chief's reason.
//! * **Gable Over Door/Window** ([`gable_over_openings`], an Edit toolbar
//!   button for selected exterior doors and windows) draws a line 12 inches
//!   past each side of each opening; openings within 30 inches of each other
//!   share one line.
//! * **Delete Gable Over Opening** ([`delete_over_openings`]) removes the
//!   lines made for the selected openings.
//! * The **Gable Line Specification** ([`set_spec`], dialog in
//!   `dialogs::roof_trim`) edits the pitch and overhang of the two planes and
//!   the line's look.
//!
//! Where the line goes into the roof is decided by `plan_roof::apply_gable_lines`:
//! the gable planes run from the line into the roof until their ridge meets
//! the roof surface, and the old roof keeps whatever is higher.

// The entry points are called from the roof, menu and edit-toolbar owners'
// files once the hooks in docs/integration-queue.md are in; until then
// only the scenario tests use them.
#![allow(dead_code)]

use crate::editor::roof_view::{self, RoofPlaneRecord, RoofSettings};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, ObjectRef};
use eframe::egui::{self, Color32, Stroke};
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::layers::{Layer, LineStyle};
use plan_core::{Floor, Id, OpeningKind, Project, Side, WallKind};
use plan_roof::{
    apply_gable_lines, check_gable_line, gable_lines_over_openings, GableLine, GableLineProblem,
    OpeningSpan, PlaneOrigin, WallFace,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The layer gable lines are drawn on.
pub const LAYER: &str = "Roof Planes, Gable Lines";
/// `kind` of the record in `Floor.roofs`.
const KIND: &str = "gable_line";
/// A click this close to a line (screen pixels) picks it.
const PICK_PX: f64 = 6.0;

/// Custom command ids of the Edit toolbar buttons.
pub mod cmd {
    pub const OVER_OPENING: &str = "gable_line.over_opening";
    pub const DELETE_OVER_OPENING: &str = "gable_line.delete_over_opening";
    pub const SPEC: &str = "gable_line.spec";
}

/// One stored Gable/Roof Line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GableLineRecord {
    pub id: Id,
    pub a: Point,
    pub b: Point,
    /// Rise per 12 of the two planes.
    pub pitch: f64,
    /// Overhang of the two planes past the line, inches.
    pub overhang: f64,
    /// The doors and windows Gable Over Door/Window made this line for.
    pub openings: Vec<Id>,
    /// Line panel / Line Style panel: dash, colour and weight.
    pub dash: Option<LineStyle>,
    pub color: Option<[u8; 3]>,
    pub weight: Option<u32>,
    /// Arrow panel: arrowheads at the ends.
    pub arrow_start: bool,
    pub arrow_end: bool,
}

impl Default for GableLineRecord {
    fn default() -> Self {
        let d = GableLine::default();
        Self {
            id: 0,
            a: Point::ZERO,
            b: Point::ZERO,
            pitch: d.pitch,
            overhang: d.overhang,
            openings: Vec::new(),
            dash: None,
            color: None,
            weight: None,
            arrow_start: false,
            arrow_end: false,
        }
    }
}

impl GableLineRecord {
    pub fn line(&self) -> GableLine {
        GableLine::new(self.a, self.b, self.pitch, self.overhang)
    }

    fn to_json(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(m) = &mut v {
            m.insert("kind".into(), json!(KIND));
        }
        v
    }
}

fn is_record(v: &Value) -> bool {
    v.get("kind").and_then(Value::as_str) == Some(KIND)
}

/// The Gable/Roof Lines of `floor`.
pub fn lines(floor: &Floor) -> Vec<GableLineRecord> {
    floor
        .roofs
        .iter()
        .filter(|v| is_record(v))
        .filter_map(|v| serde_json::from_value::<GableLineRecord>(v.clone()).ok())
        .filter(|r| r.id != 0)
        .collect()
}

/// The line `id` of `floor`.
pub fn line(floor: &Floor, id: Id) -> Option<GableLineRecord> {
    lines(floor).into_iter().find(|r| r.id == id)
}

fn put(floor: &mut Floor, rec: &GableLineRecord) {
    floor
        .roofs
        .retain(|v| !(is_record(v) && v.get("id").and_then(Value::as_u64) == Some(rec.id)));
    floor.roofs.push(rec.to_json());
}

fn ensure_layer(project: &mut Project) {
    if project.layers.get(LAYER).is_none() {
        project.layers.add(Layer::new(LAYER, [150, 90, 40], 18));
    }
}

/// Pitch and overhang a new line starts with: the Build Roof settings of
/// the floor (8:12 and 16 inches before a roof was built).
pub fn default_pitch_overhang(floor: &Floor) -> (f64, f64) {
    let s = roof_view::load(floor)
        .settings
        .unwrap_or_else(RoofSettings::fallback);
    (s.pitch, s.overhang)
}

// ---------------------------------------------------------------------------
// Walls and openings
// ---------------------------------------------------------------------------

/// The exterior walls of `floor` as [`WallFace`]s.
pub fn wall_faces(floor: &Floor) -> Vec<WallFace> {
    floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior && w.length() > 1.0)
        .map(|w| WallFace {
            start: w.start,
            end: w.end,
            thickness: w.thickness,
        })
        .collect()
}

/// Why a line from `a` to `b` is not a valid Gable/Roof Line, if it is not.
pub fn problem(floor: &Floor, a: Point, b: Point) -> Option<GableLineProblem> {
    let probe = GableLine::new(a, b, 8.0, 16.0);
    check_gable_line(&probe, &wall_faces(floor)).err()
}

/// The exterior doors and windows among the selected objects.
fn selected_openings(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Opening(id) => Some(*id),
            _ => None,
        })
        .filter(|id| span_of(cx.floor(), *id).is_some())
        .collect()
}

/// A door or window on an exterior wall as the geometry sees it.
fn span_of(floor: &Floor, opening: Id) -> Option<OpeningSpan> {
    let o = floor.openings.iter().find(|o| o.id == opening)?;
    if !matches!(o.kind, OpeningKind::Door | OpeningKind::Window) {
        return None;
    }
    let w = floor
        .wall(o.wall_id)
        .filter(|w| w.kind == WallKind::Exterior)?;
    let outward = match w.exterior_side {
        Side::Left => w.normal(),
        Side::Right => w.normal().scale(-1.0),
    };
    Some(OpeningSpan {
        wall_start: w.start,
        wall_end: w.end,
        offset: o.center_offset,
        width: o.width,
        outward,
        face: w.thickness * 0.5,
    })
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Draws a Gable/Roof Line from `a` to `b` with the floor's default pitch and
/// overhang. One undo step. Err carries Chief's reason.
pub fn draw_line(cx: &mut EditorContext, a: Point, b: Point) -> Result<Id, String> {
    if let Some(p) = problem(cx.floor(), a, b) {
        cx.status = p.message().to_string();
        return Err(p.message().to_string());
    }
    let (pitch, overhang) = default_pitch_overhang(cx.floor());
    cx.begin_change("Gable/Roof Line");
    ensure_layer(&mut cx.project);
    let id = cx.project.alloc_id();
    let rec = GableLineRecord {
        id,
        a,
        b,
        pitch,
        overhang,
        ..Default::default()
    };
    let fl = cx.floor;
    put(&mut cx.project.floors[fl], &rec);
    cx.selection.items.clear();
    cx.mark_dirty();
    cx.status = "Gable/Roof Line drawn: it becomes a gable at the next Build Roof".into();
    Ok(id)
}

/// Gable Over Door/Window for the selected exterior doors and windows: a line
/// 12 inches past each side of each opening, openings within 30 inches
/// sharing one. An opening that already has a line gets a fresh one. Returns
/// the number of lines made. One undo step.
pub fn gable_over_openings(cx: &mut EditorContext) -> usize {
    let ids = selected_openings(cx);
    if ids.is_empty() {
        cx.status = "Select doors or windows on an exterior wall first".into();
        return 0;
    }
    let spans: Vec<OpeningSpan> = ids
        .iter()
        .filter_map(|id| span_of(cx.floor(), *id))
        .collect();
    let (pitch, overhang) = default_pitch_overhang(cx.floor());
    let made = gable_lines_over_openings(&spans, pitch, overhang);
    if made.is_empty() {
        return 0;
    }
    cx.begin_change("Gable Over Door/Window");
    ensure_layer(&mut cx.project);
    let fl = cx.floor;
    // Older lines of these openings give way to the new ones.
    remove_for(&mut cx.project.floors[fl], &ids);
    for l in &made {
        let id = cx.project.alloc_id();
        let tagged = ids
            .iter()
            .copied()
            .filter(|oid| covers(cx.floor(), l, *oid))
            .collect();
        let rec = GableLineRecord {
            id,
            a: l.a,
            b: l.b,
            pitch: l.pitch,
            overhang: l.overhang,
            openings: tagged,
            ..Default::default()
        };
        put(&mut cx.project.floors[fl], &rec);
    }
    cx.mark_dirty();
    cx.status = format!("{} gable line(s) over the selected openings", made.len());
    made.len()
}

/// Does `l` run along the face of the wall that carries `opening`, over it?
fn covers(floor: &Floor, l: &GableLine, opening: Id) -> bool {
    let Some(s) = span_of(floor, opening) else {
        return false;
    };
    let dir = s.wall_end.sub(s.wall_start).normalized();
    let centre = s.wall_start.add(dir.scale(s.offset));
    let face = centre.add(s.outward.scale(s.face));
    dist_to_segment(face, l.a, l.b) < 1.0
}

fn remove_for(floor: &mut Floor, openings: &[Id]) -> usize {
    let before = floor.roofs.len();
    let drop: Vec<Id> = lines(floor)
        .into_iter()
        .filter(|r| r.openings.iter().any(|o| openings.contains(o)))
        .map(|r| r.id)
        .collect();
    floor.roofs.retain(|v| {
        !(is_record(v)
            && v.get("id")
                .and_then(Value::as_u64)
                .is_some_and(|i| drop.contains(&i)))
    });
    before - floor.roofs.len()
}

/// Delete Gable Over Opening: removes the lines made for the selected
/// openings. Returns how many were removed. One undo step.
pub fn delete_over_openings(cx: &mut EditorContext) -> usize {
    let ids: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Opening(id) => Some(*id),
            _ => None,
        })
        .collect();
    if !has_line_for(cx.floor(), &ids) {
        cx.status = "No gable over the selected openings".into();
        return 0;
    }
    cx.begin_change("Delete Gable Over Opening");
    let fl = cx.floor;
    let n = remove_for(&mut cx.project.floors[fl], &ids);
    cx.mark_dirty();
    cx.status = "Gable over opening deleted".into();
    n
}

fn has_line_for(floor: &Floor, openings: &[Id]) -> bool {
    lines(floor)
        .iter()
        .any(|r| r.openings.iter().any(|o| openings.contains(o)))
}

/// Deletes line `id`. One undo step.
pub fn delete_line(cx: &mut EditorContext, id: Id) -> bool {
    if line(cx.floor(), id).is_none() {
        return false;
    }
    cx.begin_change("Delete Gable/Roof Line");
    let fl = cx.floor;
    cx.project.floors[fl]
        .roofs
        .retain(|v| !(is_record(v) && v.get("id").and_then(Value::as_u64) == Some(id)));
    cx.mark_dirty();
    true
}

/// Gable Line Specification OK: replaces line `rec.id` with `rec`. One undo
/// step; false when the line is gone or no longer valid.
pub fn set_spec(cx: &mut EditorContext, rec: &GableLineRecord) -> bool {
    if line(cx.floor(), rec.id).is_none() || rec.pitch <= 0.0 || rec.overhang < 0.0 {
        return false;
    }
    cx.begin_change("Gable Line Specification");
    let fl = cx.floor;
    put(&mut cx.project.floors[fl], rec);
    cx.mark_dirty();
    true
}

/// The line near `p` (within the pick distance at the current zoom).
pub fn line_near(cx: &EditorContext, p: Point) -> Option<Id> {
    let tol = PICK_PX / cx.px_per_in.max(1e-6);
    lines(cx.floor())
        .into_iter()
        .map(|r| (r.id, dist_to_segment(p, r.a, r.b)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(id, _)| id)
}

// ---------------------------------------------------------------------------
// Edit toolbar
// ---------------------------------------------------------------------------

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

/// The Edit toolbar buttons for the current selection: Gable Over Door/Window
/// with exterior doors or windows selected, Delete Gable Over Opening when
/// one of them has a gable, Open Object with a line selected through
/// [`line_near`]. `extra_edit_actions` lists them for every tool
/// (docs/integration-queue.md).
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    let ids = selected_openings(cx);
    if !ids.is_empty() {
        v.push(button(cmd::OVER_OPENING, "Gable Over Door/Window"));
        if has_line_for(cx.floor(), &ids) {
            v.push(button(
                cmd::DELETE_OVER_OPENING,
                "Delete Gable Over Opening",
            ));
        }
    }
    v
}

/// Runs an Edit toolbar command of this module. False when `id` is not one.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        cmd::OVER_OPENING => {
            gable_over_openings(cx);
        }
        cmd::DELETE_OVER_OPENING => {
            delete_over_openings(cx);
        }
        _ => return false,
    }
    true
}

// ---------------------------------------------------------------------------
// Build Roof
// ---------------------------------------------------------------------------

/// The roof planes of `planes` with the gables of `lines` added, as records.
///
/// Only automatic planes that rise from a footprint edge take part. A plane a
/// gable cuts is replaced by its remaining pieces (the first piece keeps the
/// plane's id and holes); each gable plane is a new automatic plane without a
/// source edge, like a roof return. Returns the new records and how many
/// lines changed the roof.
pub fn gabled_records(
    project: &mut Project,
    planes: Vec<RoofPlaneRecord>,
    lines: &[GableLineRecord],
    layer_of_new: &str,
    material: &str,
) -> (Vec<RoofPlaneRecord>, usize) {
    let (taking, rest): (Vec<_>, Vec<_>) = planes
        .into_iter()
        .partition(|p| p.auto && p.source.is_some());
    if taking.is_empty() || lines.is_empty() {
        let mut all = taking;
        all.extend(rest);
        return (all, 0);
    }
    let roof: Vec<_> = taking.iter().map(|r| r.to_roof_plane(0)).collect();
    let eave = taking[0].baseline_height();
    let geo: Vec<GableLine> = lines.iter().map(GableLineRecord::line).collect();
    let out = apply_gable_lines(&roof, eave, &geo);
    let used = geo.len() - out.skipped.len();
    let mut made: Vec<RoofPlaneRecord> = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    for (plane, origin) in out.planes.iter().zip(&out.origin) {
        match *origin {
            PlaneOrigin::Main(i) => {
                let mut rec = taking[i].clone();
                if seen.contains(&i) {
                    rec.id = project.alloc_id();
                    rec.holes.clear();
                } else {
                    seen.push(i);
                    let poly: Vec<Point> = plane
                        .polygon3d
                        .iter()
                        .map(|v| Point::new(v[0], -v[2]))
                        .collect();
                    rec.holes
                        .retain(|h| h.outline.iter().all(|q| point_in_polygon(*q, &poly)));
                }
                rec.polygon3d = plane.polygon3d.clone();
                rec.baseline = plane.baseline;
                made.push(rec);
            }
            PlaneOrigin::Wing(k) => {
                let id = project.alloc_id();
                let mut rec = RoofPlaneRecord::new(
                    id,
                    plane.polygon3d.clone(),
                    plane.pitch_in_12,
                    plane.baseline,
                );
                rec.auto = true;
                rec.overhang = lines[k].overhang;
                rec.layer = layer_of_new.to_string();
                rec.material = material.to_string();
                made.push(rec);
            }
        }
    }
    // Planes a gable swallowed completely are gone; the rest stay in order.
    made.extend(rest);
    (made, used)
}

/// Turns the stored Gable/Roof Lines of floor `fi` into gables on the planes
/// Build Roof has just stored there. `roof_view::rebuild` calls this once,
/// right after storing freshly built planes (the planes are not gabled
/// twice: only automatic planes with a source edge are cut, and a rebuild
/// makes them anew). Returns the number of lines that changed the roof.
pub fn apply_stored(project: &mut Project, fi: usize) -> usize {
    let lines = self::lines(&project.floors[fi]);
    if lines.is_empty() {
        return 0;
    }
    let mut set = roof_view::load(&project.floors[fi]);
    let (layer, material) = set
        .planes
        .first()
        .map(|p| (p.layer.clone(), p.material.clone()))
        .unwrap_or_else(|| {
            (
                roof_view::LAYER_PLANES.to_string(),
                RoofSettings::fallback().material,
            )
        });
    let planes = std::mem::take(&mut set.planes);
    let (planes, used) = gabled_records(project, planes, &lines, &layer, &material);
    set.planes = planes;
    roof_view::store(project, fi, &mut set);
    used
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// Draws the Gable/Roof Lines of the active floor in the plan.
pub fn draw_lines(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let floor = cx.floor();
    if floor.roofs.is_empty() || !cx.layers().is_visible(LAYER) {
        return;
    }
    let base = cx
        .layers()
        .get(LAYER)
        .map_or(Color32::from_rgb(150, 90, 40), |l| {
            Color32::from_rgb(l.color[0], l.color[1], l.color[2])
        });
    for r in lines(floor) {
        let color = r
            .color
            .map_or(base, |c| Color32::from_rgb(c[0], c[1], c[2]));
        let stroke = Stroke::new(r.weight.map_or(1.5, |w| w as f32 * 0.5 + 0.5), color);
        let (p, q) = (cam.world_to_screen(r.a), cam.world_to_screen(r.b));
        match r.dash.unwrap_or(LineStyle::Dashed) {
            LineStyle::Solid => {
                painter.line_segment([p, q], stroke);
            }
            _ => {
                painter.add(egui::Shape::dashed_line(&[p, q], stroke, 8.0, 4.0));
            }
        }
        let dir = (q - p).normalized();
        let tip = |at: egui::Pos2, d: egui::Vec2| {
            let side = egui::vec2(-d.y, d.x);
            painter.line_segment([at, at - d * 9.0 + side * 4.0], stroke);
            painter.line_segment([at, at - d * 9.0 - side * 4.0], stroke);
        };
        if r.arrow_end {
            tip(q, dir);
        }
        if r.arrow_start {
            tip(p, -dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_round_trip_and_default_to_chiefs_stock_values() {
        let r = GableLineRecord {
            id: 7,
            a: Point::new(0.0, 0.0),
            b: Point::new(60.0, 0.0),
            openings: vec![3, 4],
            ..Default::default()
        };
        assert_eq!((r.pitch, r.overhang), (8.0, 16.0));
        let back: GableLineRecord = serde_json::from_value(r.to_json()).unwrap();
        assert_eq!(back, r);
        assert!(is_record(&r.to_json()));
    }
}
