//! The Tray Ceiling Polyline tool (Build > Roof > Tray Ceiling Polyline; manual
//! pp. 457-458, R-111) and the tray ceiling commands of the Edit toolbar.
//!
//! # The tool
//!
//! Click inside a room and a tray ceiling polyline that follows the room is
//! made (Make Tray Ceiling in Room with the default Width and Depth). Click
//! and drag to draw a rectangular polyline instead. The tray is a closed CAD
//! polyline on the "Ceiling Planes" layer plus a
//! [`plan_core::tray::TrayCeiling`] record (see `plan_core::tray`), so it
//! selects, moves, stretches and deletes like any CAD object; one tray is one
//! undo step. Esc drops a pending drag.
//!
//! # Edit toolbar
//!
//! [`edit_actions`] and [`run_command`] are the buttons and what they do:
//! with a tray selected, Tray Ceiling Specification, Make Nested Tray Ceiling
//! and Explode Tray Ceiling; with a room selected, Make Tray Ceiling in
//! Room, Make Coffered Ceiling and Turn Off Ceiling / Turn On Ceiling (the
//! room's Flat Ceiling Over This Room); with a closed CAD polyline selected,
//! Convert Polyline to Tray Ceiling. The functions behind them
//! ([`make_in_room`], [`make_nested`], [`make_coffered`], [`explode`],
//! [`convert_polyline`], [`set_flat_ceiling`]) each make exactly one undo
//! step.
//!
//! [`draw_trays`] paints the dropped outer ceiling, the step edge and the
//! Caution symbol in plan views; the tool draws it itself while it is active.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::roof_view::{self, CeilingRecord};
use crate::editor::rooms_edit;
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Shape, Stroke};
use plan_core::cad::{CadItem, CadObject};
use plan_core::geometry::Point;
use plan_core::layers::Layer;
use plan_core::tray::{self, Caution, TrayCeiling, TrayGeom};
use plan_core::Id;

/// Custom command ids of the tray ceiling buttons.
pub mod cmd {
    pub const SPEC: &str = "tray.spec";
    pub const MAKE_IN_ROOM: &str = "tray.make_in_room";
    pub const NESTED: &str = "tray.nested";
    pub const COFFERED: &str = "tray.coffered";
    pub const EXPLODE: &str = "tray.explode";
    pub const CONVERT: &str = "tray.convert";
    pub const CEILING_OFF: &str = "tray.ceiling_off";
    pub const CEILING_ON: &str = "tray.ceiling_on";
}

/// A dragged rectangle smaller than this (inches, each way) is a stray click.
const MIN_RECT: f64 = 12.0;
/// A press and release closer than this (inches) is a click.
const CLICK_RADIUS: f64 = 6.0;
/// Target size of a coffer cell, and the beam between cells, inches.
const COFFER_CELL: f64 = 36.0;
const COFFER_BEAM: f64 = 8.0;
const COFFER_DEPTH: f64 = 6.0;

// ---------------------------------------------------------------------------
// Looking trays up
// ---------------------------------------------------------------------------

/// The trays of the active floor worked out against its rooms, with heights
/// above the floor datum.
pub fn geoms(cx: &EditorContext) -> Vec<TrayGeom> {
    let f = cx.floor();
    if f.trays.is_empty() {
        return Vec::new();
    }
    tray::resolve(f, &tray::room_ceilings(f, &cx.rooms))
}

/// The tray the selection is exactly one of.
pub fn selected_tray(cx: &EditorContext) -> Option<Id> {
    match cx.selection.single() {
        Some(ObjectRef::Cad(id)) if cx.floor().is_tray(id) => Some(id),
        _ => None,
    }
}

/// The closed CAD polyline (not yet a tray) the selection is exactly one of.
fn selected_polyline(cx: &EditorContext) -> Option<Id> {
    let Some(ObjectRef::Cad(id)) = cx.selection.single() else {
        return None;
    };
    let f = cx.floor();
    f.cad
        .iter()
        .any(|c| {
            c.id == id
                && matches!(&c.item, CadItem::Polyline { points, closed: true } if points.len() >= 3)
        })
        .then_some(id)
        .filter(|id| !f.is_tray(*id))
}

// ---------------------------------------------------------------------------
// Commands (one undo step each)
// ---------------------------------------------------------------------------

/// Draws a rectangular tray ceiling polyline from `a` to `b`. One undo step.
pub fn draw_rect(cx: &mut EditorContext, a: Point, b: Point) -> Option<Id> {
    let (lo, hi) = (
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Point::new(a.x.max(b.x), a.y.max(b.y)),
    );
    if hi.x - lo.x < MIN_RECT || hi.y - lo.y < MIN_RECT {
        return None;
    }
    let outline = [lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)];
    cx.begin_change("Tray Ceiling Polyline");
    let fl = cx.floor;
    match cx.project.add_tray(fl, &outline, TrayCeiling::default()) {
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

/// Make Tray Ceiling in Room: a tray following room `room` (an index into
/// `cx.rooms`), inset by `spec.width`. One undo step; the tray is selected.
pub fn make_in_room(cx: &mut EditorContext, room: usize, spec: TrayCeiling) -> Option<Id> {
    let r = cx.rooms.get(room)?;
    let outline = if r.inner_polygon.len() >= 3 {
        r.inner_polygon.clone()
    } else {
        r.polygon.clone()
    };
    cx.begin_change("Make Tray Ceiling in Room");
    let fl = cx.floor;
    match cx.project.make_tray_in_room(fl, &outline, spec) {
        Some(id) => {
            cx.mark_dirty();
            cx.refresh();
            cx.selection.set(ObjectRef::Cad(id));
            cx.status =
                "Tray ceiling made. Open it to set the depth, moldings and rope lights".into();
            Some(id)
        }
        None => {
            cx.cancel_change();
            cx.status = "The room is too small for that Width".into();
            None
        }
    }
}

/// Make Nested Tray Ceiling: a smaller tray following the perimeter of tray
/// `parent`. One undo step.
pub fn make_nested(cx: &mut EditorContext, parent: Id, spec: TrayCeiling) -> Option<Id> {
    cx.begin_change("Make Nested Tray Ceiling");
    let fl = cx.floor;
    match cx.project.make_nested_tray(fl, parent, spec) {
        Some(id) => {
            cx.mark_dirty();
            cx.refresh();
            cx.selection.set(ObjectRef::Cad(id));
            Some(id)
        }
        None => {
            cx.cancel_change();
            cx.status = "The tray is too small for a nested tray of that Width".into();
            None
        }
    }
}

/// Make Coffered Ceiling in room `room`: a grid of recessed tray polylines
/// (cells of about 3 ft with 8 in beams). One undo step. Returns the ids.
pub fn make_coffered(cx: &mut EditorContext, room: usize) -> Vec<Id> {
    let Some(r) = cx.rooms.get(room) else {
        return Vec::new();
    };
    let outline = if r.inner_polygon.len() >= 3 {
        r.inner_polygon.clone()
    } else {
        r.polygon.clone()
    };
    let (lo, hi) = plan_core::foundation::bounds(&outline);
    let fit =
        |len: f64| (((len - COFFER_BEAM) / (COFFER_CELL + COFFER_BEAM)).floor() as usize).max(1);
    let (cols, rows) = (fit(hi.x - lo.x), fit(hi.y - lo.y));
    let spec = TrayCeiling {
        depth: COFFER_DEPTH,
        ..TrayCeiling::default()
    };
    cx.begin_change("Make Coffered Ceiling");
    let fl = cx.floor;
    let ids = cx
        .project
        .make_coffered_ceiling(fl, &outline, cols, rows, COFFER_BEAM, &spec);
    if ids.is_empty() {
        cx.cancel_change();
        cx.status = "The room is too small for a coffered ceiling".into();
        return ids;
    }
    cx.mark_dirty();
    cx.refresh();
    cx.selection.clear();
    cx.status = format!("Coffered ceiling: {} coffers", ids.len());
    ids
}

/// Convert Polyline: the closed CAD polyline `id` becomes a tray ceiling.
/// One undo step.
pub fn convert_polyline(cx: &mut EditorContext, id: Id) -> bool {
    cx.begin_change("Convert Polyline to Tray Ceiling");
    let fl = cx.floor;
    if cx
        .project
        .convert_polyline_to_tray(fl, id, TrayCeiling::default())
    {
        cx.mark_dirty();
        cx.refresh();
        true
    } else {
        cx.cancel_change();
        cx.status = "Select a closed polyline".into();
        false
    }
}

/// Turn Off Ceiling (`flat` false: a cathedral ceiling that follows the roof)
/// or Turn On Ceiling for room `room`. One undo step.
pub fn set_flat_ceiling(cx: &mut EditorContext, room: usize, flat: bool) -> bool {
    let Some(r) = cx.rooms.get(room).cloned() else {
        return false;
    };
    let anchor = rooms_edit::room_anchor(&r);
    let current = rooms_edit::name_entry(cx, &r).is_none_or(|n| n.flat_ceiling);
    if current == flat {
        return false;
    }
    cx.begin_change(if flat {
        "Turn On Ceiling"
    } else {
        "Turn Off Ceiling"
    });
    let fl = cx.floor;
    let rooms = cx.rooms.clone();
    cx.project.set_flat_ceiling(fl, anchor, flat, &rooms);
    cx.mark_dirty();
    cx.refresh();
    true
}

/// Explode Tray Ceiling: the tray becomes its parts - ceiling planes (the
/// outer ceiling, the raised inner ceiling of a recessed tray, one sloped
/// plane per side), the polyline as a plain CAD polyline, and molding and rope
/// light runs as polylines on their layers. Not available while the tray has
/// a Caution. One undo step; the parts cannot be put back together.
pub fn explode(cx: &mut EditorContext, id: Id) -> bool {
    let all = geoms(cx);
    let Some(g) = all.iter().find(|g| g.id == id).cloned() else {
        return false;
    };
    if !g.ok() {
        cx.status = "A tray ceiling with a Caution symbol cannot be exploded".into();
        return false;
    }
    let Some(rec) = cx.floor().tray(id).cloned() else {
        return false;
    };
    let fl = cx.floor;
    cx.begin_change("Explode Tray Ceiling");
    let elev = cx.floor().elevation;
    let thickness = rec.structure_thickness().max(0.5);
    let planes = plan_roof::tray_ceiling_planes(
        &g.inner,
        &g.outer,
        elev + g.h_outer,
        elev + g.h_inner,
        rec.recess,
        rec.pitch.unwrap_or(0.0),
        g.run,
        thickness,
    );
    let mut set = roof_view::load(&cx.project.floors[fl]);
    for p in &planes {
        let new_id = cx.project.alloc_id();
        set.ceilings
            .push(CeilingRecord::from_plane(new_id, p, false));
    }
    roof_view::store(&mut cx.project, fl, &mut set);
    // The hole stays as a plain polyline; the record goes.
    cx.project.floors[fl].trays.remove(id);
    if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
        c.layer = plan_core::cad::DEFAULT_CAD_LAYER.to_string();
    }
    // Molding and rope light runs become polylines of their own.
    let mut runs: Vec<(Vec<Point>, &'static str)> = Vec::new();
    for (_, _, outline, _) in tray::molding_runs(&g, &rec) {
        runs.push((outline, plan_core::details::MOLDING_LAYER));
    }
    for r in tray::rope_light_paths(&g, &rec) {
        runs.push((r.points, "Rope Lights"));
    }
    for (points, layer) in runs {
        if cx.project.layers.get(layer).is_none() {
            cx.project.layers.add(Layer::new(layer, [120, 80, 40], 18));
        }
        let new_id = cx.project.alloc_id();
        cx.project.floors[fl].cad.push(CadObject {
            id: new_id,
            layer: layer.to_string(),
            item: CadItem::Polyline {
                points,
                closed: true,
            },
        });
    }
    cx.mark_dirty();
    cx.refresh();
    cx.selection.set(ObjectRef::Cad(id));
    cx.status = "Tray ceiling exploded into its parts".into();
    true
}

// ---------------------------------------------------------------------------
// Edit toolbar
// ---------------------------------------------------------------------------

fn button(id: &'static str, label: &'static str, enabled: bool) -> EditAction {
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled,
    }
}

/// The Edit toolbar buttons for the current selection (see the module docs).
/// `extra_edit_actions` lists them for every tool (docs/integration-queue.md).
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    if let Some(id) = selected_tray(cx) {
        let ok = geoms(cx).iter().any(|g| g.id == id && g.ok());
        v.push(button(cmd::SPEC, "Tray Ceiling Specification", true));
        v.push(button(cmd::NESTED, "Make Nested Tray Ceiling", true));
        v.push(button(cmd::EXPLODE, "Explode Tray Ceiling", ok));
    } else if let Some(id) = selected_polyline(cx) {
        let _ = id;
        v.push(button(
            cmd::CONVERT,
            "Convert Polyline to Tray Ceiling",
            true,
        ));
    }
    if let Some(room) = rooms_edit::selected_room(cx) {
        let flat = cx
            .rooms
            .get(room)
            .and_then(|r| rooms_edit::name_entry(cx, r))
            .is_none_or(|n| n.flat_ceiling);
        v.push(button(cmd::MAKE_IN_ROOM, "Make Tray Ceiling in Room", flat));
        v.push(button(cmd::COFFERED, "Make Coffered Ceiling", flat));
        if flat {
            v.push(button(cmd::CEILING_OFF, "Turn Off Ceiling", true));
        } else {
            v.push(button(cmd::CEILING_ON, "Turn On Ceiling", true));
        }
    }
    v
}

/// Runs a tray ceiling Edit toolbar command. False when `id` is not one.
/// (Make Tray Ceiling in Room and Make Nested Tray Ceiling open the
/// Tray Ceiling Specification with its Width box, as Chief does.)
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        cmd::SPEC => {
            if let Some(t) = selected_tray(cx) {
                crate::dialogs::tray_ceiling::open_edit(cx, t);
            }
        }
        cmd::NESTED => {
            if let Some(t) = selected_tray(cx) {
                crate::dialogs::tray_ceiling::open_make_nested(cx, t);
            }
        }
        cmd::MAKE_IN_ROOM => {
            if let Some(r) = rooms_edit::selected_room(cx) {
                crate::dialogs::tray_ceiling::open_make_in_room(cx, r);
            }
        }
        cmd::COFFERED => {
            if let Some(r) = rooms_edit::selected_room(cx) {
                make_coffered(cx, r);
            }
        }
        cmd::EXPLODE => {
            if let Some(t) = selected_tray(cx) {
                explode(cx, t);
            }
        }
        cmd::CONVERT => {
            if let Some(p) = selected_polyline(cx) {
                convert_polyline(cx, p);
            }
        }
        cmd::CEILING_OFF | cmd::CEILING_ON => {
            if let Some(r) = rooms_edit::selected_room(cx) {
                set_flat_ceiling(cx, r, id == cmd::CEILING_ON);
            }
        }
        _ => return false,
    }
    true
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

const RING_FILL: Color32 = Color32::from_rgba_premultiplied(40, 24, 40, 40);
const STEP_EDGE: Color32 = Color32::from_rgb(110, 50, 110);
const CAUTION: Color32 = Color32::from_rgb(220, 150, 0);

/// Paints the trays of the active floor in a plan view: the outer ceiling
/// shaded (so the step reads), a heavier line along the step edge, the custom
/// label at the middle and the Caution symbol on a tray that cannot generate.
pub fn draw_trays(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    for g in geoms(cx) {
        let screen = |p: Point| cam.world_to_screen(p);
        if g.ok() {
            for piece in plan_roof::subtract_polygon(&g.outer, &g.inner) {
                let pts: Vec<Pos2> = piece.iter().map(|p| screen(*p)).collect();
                painter.add(Shape::convex_polygon(pts, RING_FILL, Stroke::NONE));
            }
        }
        let mut edge: Vec<Pos2> = g.inner.iter().map(|p| screen(*p)).collect();
        if let Some(first) = edge.first().copied() {
            edge.push(first);
        }
        painter.add(Shape::line(edge, Stroke::new(1.6_f32, STEP_EDGE)));
        let c = screen(plan_core::geometry::polygon_centroid(&g.inner));
        if let Some(caution) = g.caution {
            draw_caution(painter, c, caution);
        } else if let Some(rec) = cx.floor().tray(g.id) {
            if !rec.label.trim().is_empty() {
                painter.text(
                    c,
                    Align2::CENTER_CENTER,
                    rec.label.trim(),
                    FontId::proportional(12.0),
                    STEP_EDGE,
                );
            }
        }
    }
}

fn draw_caution(painter: &egui::Painter, c: Pos2, why: Caution) {
    let r = 11.0_f32;
    painter.add(Shape::convex_polygon(
        vec![
            Pos2::new(c.x, c.y - r),
            Pos2::new(c.x + r, c.y + r * 0.8),
            Pos2::new(c.x - r, c.y + r * 0.8),
        ],
        CAUTION,
        Stroke::new(1.2_f32, Color32::BLACK),
    ));
    painter.text(
        Pos2::new(c.x, c.y + 2.0),
        Align2::CENTER_CENTER,
        "!",
        FontId::proportional(13.0),
        Color32::BLACK,
    );
    let _ = why;
}

// ---------------------------------------------------------------------------
// The tool
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct TrayCeilingTool {
    anchor: Option<Point>,
    hover: Option<Point>,
}

impl TrayCeilingTool {
    fn snap(&self, cx: &EditorContext, p: &PointerEvent) -> Point {
        cx.snap_at(p.world, self.anchor, p.modifiers.alt, &[]).point
    }
}

impl Tool for TrayCeilingTool {
    fn id(&self) -> ToolId {
        ToolId::TrayCeiling
    }

    fn name(&self) -> &'static str {
        "Tray Ceiling Polyline"
    }

    fn hint(&self) -> String {
        "Tray Ceiling Polyline: click inside a room, or click and drag a rectangle".into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.anchor = None;
        self.hover = None;
        cx.status = self.hint();
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.anchor = None;
        self.hover = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.hover = Some(self.snap(cx, &p));
        ToolResult::consumed()
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let at = self.snap(cx, &p);
        self.anchor = Some(at);
        self.hover = Some(at);
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let Some(a) = self.anchor.take() else {
            return ToolResult::ignored();
        };
        let b = self.snap(cx, &p);
        self.hover = None;
        if a.dist(b) <= CLICK_RADIUS {
            // A click: the tray follows the room around the point.
            let Some(room) = rooms_edit::room_index_at(cx, a) else {
                cx.status = "Click inside a room to make a tray ceiling".into();
                return ToolResult::consumed();
            };
            return match make_in_room(cx, room, TrayCeiling::default()) {
                Some(_) => ToolResult::committed("Make Tray Ceiling in Room"),
                None => ToolResult::consumed(),
            };
        }
        match draw_rect(cx, a, b) {
            Some(_) => {
                cx.status = "Tray ceiling polyline drawn".into();
                ToolResult::committed("Tray Ceiling Polyline")
            }
            None => {
                cx.status = "Drag a larger rectangle".into();
                ToolResult::consumed()
            }
        }
    }

    fn key(&mut self, _cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Escape) && self.anchor.take().is_some() {
            self.hover = None;
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        draw_trays(cx, painter, cam);
        let (Some(a), Some(b)) = (self.anchor, self.hover) else {
            return;
        };
        if a.dist(b) <= CLICK_RADIUS {
            return;
        }
        let rect = [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)];
        let mut pts: Vec<Pos2> = rect.iter().map(|p| cam.world_to_screen(*p)).collect();
        pts.push(pts[0]);
        painter.add(Shape::line(pts, Stroke::new(1.5_f32, cx.palette.selection)));
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        let mut v = cx.common_edit_actions();
        v.extend(edit_actions(cx));
        v
    }
}
