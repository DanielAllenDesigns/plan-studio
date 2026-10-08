//! Electrical tools (CB-62..CB-67 in `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! One tool object with a flavor per flyout entry ([`ElecVariant`]): 110V,
//! 220V and GFCI outlets, lights, rope lights, switches, ceiling fans, smoke
//! detectors, Electrical Connection and Auto Place Outlets.
//!
//! * Wall devices snap to the nearest wall within 12" and sit on the face
//!   nearest the cursor at the kind's default height (CB-63); the status bar
//!   shows `Height: 12"`.
//! * Ceiling devices go where you click and snap to a room's center within 12".
//! * Clicking an existing device selects it (the Select tool does not know
//!   devices yet): drag to move (wall devices slide along their wall, free
//!   devices move freely), Tab flips the side, Left/Right turn a free device
//!   (Shift = 90 degrees), Delete removes it and a double-click opens the
//!   Electrical Service Specification.
//! * Electrical Connection: click a switch (or outlet), then the light; the
//!   dashed arc is stored with the layer (CB-67).
//! * Auto Place Outlets: one click places outlets for every room of the
//!   current floor from its name and type (CB-64).
//!
//! Devices live in `Floor.electrical` through `editor::site_view`.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::electrical::{DeviceDraft, ElectricalDialog};
use crate::dialogs::Outcome;
use crate::editor::site_view::{
    arc_points, device_at, draw_symbol, edit_electrical, flip_side, load_electrical, slide_on_wall,
};
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Key, Pos2, Shape};
use plan_core::geometry::{dist_to_segment, point_in_polygon, project_on_segment, Point};
use plan_core::{Floor, Id};
use plan_electrical::{
    auto_place_outlets, auto_place_room_light, connect_in, place_free, place_on_wall,
    AutoOutletOptions, Device, DeviceKind, RoomFunction, WallSide,
};
use std::cell::RefCell;
use std::f64::consts::{FRAC_PI_2, PI};

/// How far from a wall a click still snaps to it, inches.
const WALL_SNAP: f64 = 12.0;
/// How near a room's center a ceiling click snaps to it, inches.
const ROOM_CENTER_SNAP: f64 = 12.0;
/// Pointer travel before a press on a device becomes a move, pixels.
const DRAG_PX: f32 = 4.0;
/// Rope lights shorter than this are not placed by a drag, inches.
const MIN_ROPE: f64 = 6.0;
/// Length of a rope light placed by a plain click, inches.
const DEFAULT_ROPE: f64 = 96.0;
/// Degrees a Left/Right key turns a free device.
const TURN_STEP: f64 = PI / 12.0;
/// Outlets closer than this to an existing one of the same kind are skipped, inches.
const DUPLICATE_DIST: f64 = 2.0;

/// The flavors of the electrical tool, one per flyout entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElecVariant {
    Outlet110,
    Outlet220,
    Gfci,
    Light,
    RopeLight,
    Switch,
    Switch3Way,
    CeilingFan,
    SmokeDetector,
    Connection,
    AutoOutlets,
}

impl ElecVariant {
    /// The device a click places, if this flavor places one.
    pub fn kind(self) -> Option<DeviceKind> {
        Some(match self {
            ElecVariant::Outlet110 => DeviceKind::Outlet110,
            ElecVariant::Outlet220 => DeviceKind::Outlet220,
            ElecVariant::Gfci => DeviceKind::Gfci,
            ElecVariant::Light => DeviceKind::CeilingLight,
            ElecVariant::RopeLight => DeviceKind::RopeLight {
                length: DEFAULT_ROPE,
            },
            ElecVariant::Switch => DeviceKind::Switch,
            ElecVariant::Switch3Way => DeviceKind::Switch3Way,
            ElecVariant::CeilingFan => DeviceKind::CeilingFan,
            ElecVariant::SmokeDetector => DeviceKind::SmokeDetector,
            ElecVariant::Connection | ElecVariant::AutoOutlets => return None,
        })
    }

    /// Chief's name of the flyout entry.
    pub fn name(self) -> &'static str {
        match self {
            ElecVariant::Outlet110 => "110V Outlet",
            ElecVariant::Outlet220 => "220V Outlet",
            ElecVariant::Gfci => "GFCI Outlet",
            ElecVariant::Light => "Light",
            ElecVariant::RopeLight => "Rope Light",
            ElecVariant::Switch => "Switch",
            ElecVariant::Switch3Way => "3-Way Switch",
            ElecVariant::CeilingFan => "Ceiling Fan",
            ElecVariant::SmokeDetector => "Smoke Detector",
            ElecVariant::Connection => "Electrical Connection",
            ElecVariant::AutoOutlets => "Auto Place Outlets",
        }
    }
}

/// A press on a device that may turn into a move.
struct Drag {
    id: Id,
    start: Pos2,
    moved: bool,
}

pub struct ElectricalTool {
    variant: ElecVariant,
    hover: Option<Point>,
    selected: Option<Id>,
    drag: Option<Drag>,
    /// Rope light: where the press landed and where the pointer is now.
    rope: Option<(Point, Point)>,
    /// Electrical Connection: the switch picked first.
    connect_from: Option<Id>,
    /// The specification dialog, drawn from `draw_overlay` (which only has `&self`).
    dialog: RefCell<Option<ElectricalDialog>>,
    /// An OK from the dialog, applied at the next tool event.
    applied: RefCell<Option<DeviceDraft>>,
}

impl Default for ElectricalTool {
    fn default() -> Self {
        Self {
            variant: ElecVariant::Outlet110,
            hover: None,
            selected: None,
            drag: None,
            rope: None,
            connect_from: None,
            dialog: RefCell::new(None),
            applied: RefCell::new(None),
        }
    }
}

// ----- placement (pure helpers) -----

/// `12"`-style height text for the status bar.
pub fn height_text(h: f64) -> String {
    if (h - h.round()).abs() < 1e-9 {
        format!("Height: {}\"", h.round())
    } else {
        format!("Height: {h:.1}\"")
    }
}

/// The wall device a click at `p` places: on the nearest wall within 12" (at
/// least the pick distance), on the face nearest `p`, at the kind's default
/// height. `None` when no wall is near.
pub fn wall_placement(cx: &EditorContext, kind: DeviceKind, p: Point) -> Option<Device> {
    let reach = WALL_SNAP.max(cx.pick_tol());
    let wall = cx
        .floor()
        .walls
        .iter()
        .filter(|w| cx.layers().is_visible(&w.layer) && w.length() > 1e-6)
        .map(|w| (w, dist_to_segment(p, w.start, w.end)))
        .filter(|(_, d)| *d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w)?;
    let (t, on_wall) = project_on_segment(p, wall.start, wall.end);
    let unit = cx.snap_unit();
    let offset = ((t * wall.length()) / unit).round() * unit;
    let side = if p.sub(on_wall).dot(wall.normal()) >= 0.0 {
        WallSide::Left
    } else {
        WallSide::Right
    };
    Some(place_on_wall(
        kind,
        wall,
        offset.clamp(0.0, wall.length()),
        side,
    ))
}

/// The ceiling device a click places: at `p`, or at a room's center when that
/// is within 12".
pub fn ceiling_placement(cx: &EditorContext, kind: DeviceKind, p: Point) -> Device {
    let center = cx
        .rooms
        .iter()
        .map(|r| auto_place_room_light(r).position)
        .filter(|c| c.dist(p) <= ROOM_CENTER_SNAP)
        .min_by(|a, b| a.dist(p).total_cmp(&b.dist(p)));
    place_free(kind, center.unwrap_or(p))
}

/// The device a click at `world` (`snapped` for free devices) creates.
pub fn placement(
    cx: &EditorContext,
    kind: DeviceKind,
    world: Point,
    snapped: Point,
) -> Result<Device, &'static str> {
    if kind.is_wall_mounted() {
        wall_placement(cx, kind, world).ok_or("Click on a wall to place this device")
    } else if kind.is_ceiling() {
        Ok(ceiling_placement(cx, kind, snapped))
    } else {
        Ok(place_free(kind, snapped))
    }
}

/// A rope light from `a` to `b`: its local +Y axis runs along the strip.
pub fn rope_light(a: Point, b: Point) -> Device {
    let d = b.sub(a);
    let mut dev = place_free(DeviceKind::RopeLight { length: d.length() }, a);
    dev.angle = d.angle() - FRAC_PI_2;
    dev
}

/// The function of a room from its name and type text.
pub fn room_function(name: &str, room_type: &str) -> RoomFunction {
    let text = format!("{name} {room_type}").to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| text.contains(w));
    if has(&["kitchen"]) {
        RoomFunction::Kitchen
    } else if has(&["bath", "powder", "toilet", "wc", "lavatory"]) {
        RoomFunction::Bath
    } else if has(&["laundry", "utility", "mud"]) {
        RoomFunction::Laundry
    } else if has(&["garage"]) {
        RoomFunction::Garage
    } else if has(&["bed", "suite", "nursery"]) {
        RoomFunction::Bedroom
    } else if has(&["dining", "nook", "breakfast"]) {
        RoomFunction::Dining
    } else if has(&["hall", "entry", "foyer", "corridor", "stair"]) {
        RoomFunction::Hall
    } else if has(&[
        "living", "family", "great", "den", "lounge", "study", "office",
    ]) {
        RoomFunction::Living
    } else {
        RoomFunction::Other
    }
}

/// Auto Place Outlets for every room of the current floor (CB-64). Returns the
/// number of outlets added; outlets already in place are not duplicated.
pub fn auto_place_floor_outlets(cx: &mut EditorContext) -> usize {
    cx.refresh();
    let rooms = cx.rooms.clone();
    let types: Vec<(String, RoomFunction)> = rooms
        .iter()
        .map(|r| {
            let (name, ty) = cx
                .floor()
                .room_names
                .iter()
                .find(|n| point_in_polygon(n.anchor, &r.polygon))
                .map_or((r.label.clone(), String::new()), |n| {
                    (n.name.clone(), n.room_type.clone())
                });
            (r.label.clone(), room_function(&name, &ty))
        })
        .collect();
    let placed = auto_place_outlets(cx.floor(), &rooms, &types, &AutoOutletOptions::default());
    let existing = load_electrical(cx.floor());
    let fresh: Vec<Device> = placed
        .into_iter()
        .filter(|d| {
            !existing
                .devices
                .iter()
                .any(|e| e.kind == d.kind && e.position.dist(d.position) < DUPLICATE_DIST)
        })
        .collect();
    if fresh.is_empty() {
        cx.status = if rooms.is_empty() {
            "Auto Place Outlets: no rooms found (close the walls of a room first)".into()
        } else {
            "Auto Place Outlets: every room already has its outlets".into()
        };
        return 0;
    }
    let n = fresh.len();
    edit_electrical(cx, "Auto Place Outlets", |layer, _| {
        layer.add_all(fresh);
    });
    cx.status = format!("Auto Place Outlets: placed {n} outlets");
    n
}

/// Connects `from` (a switch or outlet) to `to` with a dashed arc (CB-67).
pub fn connect_devices(cx: &mut EditorContext, from: Id, to: Id) -> Result<(), &'static str> {
    let layer = load_electrical(cx.floor());
    let (Some(a), Some(b)) = (layer.device(from), layer.device(to)) else {
        return Err("That device is gone");
    };
    if !(a.kind.is_switch() || a.kind.is_outlet()) {
        return Err("Start the connection at a switch or an outlet");
    }
    if from == to || b.kind.is_switch() {
        return Err("Click the light or outlet the switch controls");
    }
    if layer
        .connections
        .iter()
        .any(|c| c.from == from && c.to == to)
    {
        return Err("Those two are already connected");
    }
    edit_electrical(cx, "Electrical Connection", |layer, floor| {
        connect_in(layer, from, to, &floor.walls);
    });
    Ok(())
}

// ----- the tool -----

impl ElectricalTool {
    pub fn variant(&self) -> ElecVariant {
        self.variant
    }

    pub fn selected(&self) -> Option<Id> {
        self.selected
    }

    fn reset_transient(&mut self) {
        self.drag = None;
        self.rope = None;
        self.connect_from = None;
    }

    /// The dialog is open: the canvas is not in use.
    fn dialog_open(&self) -> bool {
        self.dialog.borrow().is_some()
    }

    fn selected_device(&self, cx: &EditorContext) -> Option<Device> {
        let id = self.selected?;
        load_electrical(cx.floor()).device(id).cloned()
    }

    /// Applies a dialog OK.
    fn apply_draft(&mut self, cx: &mut EditorContext, draft: &DeviceDraft) -> ToolResult {
        if load_electrical(cx.floor()).device(draft.id).is_none() {
            return ToolResult::consumed();
        }
        edit_electrical(cx, "Electrical Service Specification", |layer, _| {
            if let Some(d) = layer.device_mut(draft.id) {
                draft.apply(d);
            }
        });
        ToolResult::committed("Electrical Service Specification")
    }

    /// Applies an OK waiting from the dialog.
    fn flush(&mut self, cx: &mut EditorContext) -> Option<ToolResult> {
        let draft = self.applied.borrow_mut().take()?;
        Some(self.apply_draft(cx, &draft))
    }

    fn place_at(
        &mut self,
        cx: &mut EditorContext,
        kind: DeviceKind,
        p: &PointerEvent,
    ) -> ToolResult {
        cx.refresh();
        match placement(cx, kind, p.world, p.snapped) {
            Ok(dev) => {
                let label = format!("Place {}", kind.name());
                let mut id = 0;
                edit_electrical(cx, &label, |layer, _| id = layer.add(dev));
                self.selected = Some(id);
                cx.status.clear();
                ToolResult::committed(&label)
            }
            Err(msg) => {
                cx.status = msg.into();
                ToolResult::consumed()
            }
        }
    }

    fn place_rope(&mut self, cx: &mut EditorContext, a: Point, b: Point) -> ToolResult {
        let dev = if a.dist(b) >= MIN_ROPE {
            rope_light(a, b)
        } else {
            let mut d = place_free(
                DeviceKind::RopeLight {
                    length: DEFAULT_ROPE,
                },
                a,
            );
            d.angle = -FRAC_PI_2; // strip along +X
            d
        };
        let mut id = 0;
        edit_electrical(cx, "Place Rope Light", |layer, _| id = layer.add(dev));
        self.selected = Some(id);
        ToolResult::committed("Place Rope Light")
    }

    fn connection_click(&mut self, cx: &mut EditorContext, hit: Option<Id>) -> ToolResult {
        let Some(hit) = hit else {
            cx.status = match self.connect_from {
                Some(_) => "Click the light or outlet the switch controls".into(),
                None => "Click a switch to start the connection".into(),
            };
            return ToolResult::consumed();
        };
        let Some(from) = self.connect_from else {
            let layer = load_electrical(cx.floor());
            match layer.device(hit) {
                Some(d) if d.kind.is_switch() || d.kind.is_outlet() => {
                    self.connect_from = Some(hit);
                    self.selected = Some(hit);
                    cx.status = "Now click the light it controls".into();
                }
                _ => cx.status = "Start the connection at a switch or an outlet".into(),
            }
            return ToolResult::consumed();
        };
        match connect_devices(cx, from, hit) {
            Ok(()) => {
                self.connect_from = None;
                cx.status.clear();
                ToolResult::committed("Electrical Connection")
            }
            Err(msg) => {
                cx.status = msg.into();
                ToolResult::consumed()
            }
        }
    }

    fn move_selected(&mut self, cx: &mut EditorContext, p: &PointerEvent) {
        let Some(id) = self.drag.as_ref().map(|d| d.id) else {
            return;
        };
        let unit = cx.snap_unit();
        let mut layer = load_electrical(cx.floor());
        let wall = layer
            .device(id)
            .and_then(|d| d.wall_id)
            .and_then(|w| cx.floor().wall(w))
            .cloned();
        let Some(dev) = layer.device_mut(id) else {
            return;
        };
        match &wall {
            Some(w) => slide_on_wall(dev, w, p.world, unit),
            None => dev.position = p.snapped,
        }
        let fl = cx.floor;
        crate::editor::site_view::save_electrical(&mut cx.project, fl, &layer);
        cx.mark_dirty();
    }

    fn delete_selected(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(id) = self.selected.take() else {
            return ToolResult::ignored();
        };
        if load_electrical(cx.floor()).device(id).is_none() {
            return ToolResult::consumed();
        }
        edit_electrical(cx, "Delete Device", |layer, _| layer.remove(id));
        ToolResult::committed("Delete Device")
    }

    fn flip_selected(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(id) = self.selected else {
            return ToolResult::ignored();
        };
        let wall = load_electrical(cx.floor())
            .device(id)
            .and_then(|d| d.wall_id)
            .and_then(|w| cx.floor().wall(w))
            .cloned();
        edit_electrical(cx, "Flip Device", |layer, _| {
            if let Some(d) = layer.device_mut(id) {
                flip_side(d, wall.as_ref());
            }
        });
        ToolResult::committed("Flip Device")
    }

    fn turn_selected(&mut self, cx: &mut EditorContext, delta: f64) -> ToolResult {
        let Some(dev) = self.selected_device(cx) else {
            return ToolResult::ignored();
        };
        if dev.wall_id.is_some() {
            cx.status = "Wall devices turn with Tab (flip side)".into();
            return ToolResult::consumed();
        }
        edit_electrical(cx, "Rotate Device", |layer, _| {
            if let Some(d) = layer.device_mut(dev.id) {
                d.angle = (d.angle + delta).rem_euclid(2.0 * PI);
            }
        });
        ToolResult::committed("Rotate Device")
    }

    fn update_readout(&self, cx: &mut EditorContext) {
        cx.readout = match (self.rope, self.variant.kind()) {
            (Some((a, b)), _) => Some(format!("Length: {}", cx.fmt_dim(a.dist(b)))),
            (None, Some(k)) => Some(height_text(k.default_height())),
            _ => None,
        };
    }
}

impl Tool for ElectricalTool {
    fn id(&self) -> ToolId {
        ToolId::ElectricalVariant(self.variant)
    }

    fn name(&self) -> &'static str {
        self.variant.name()
    }

    fn hint(&self) -> String {
        match self.variant {
            ElecVariant::Connection => {
                "Electrical Connection: click a switch, then click the light it controls".into()
            }
            ElecVariant::AutoOutlets => {
                "Auto Place Outlets: click to place outlets in every room of this floor".into()
            }
            ElecVariant::RopeLight => "Rope Light: click and drag to set its length".into(),
            v => match v.kind() {
                Some(k) if k.is_wall_mounted() => {
                    format!("{}: click on a wall to place it", v.name())
                }
                _ => format!("{}: click to place it", v.name()),
            },
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::ElectricalVariant(v) = id {
            if v != self.variant {
                self.reset_transient();
            }
            self.variant = v;
        }
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.reset_transient();
        self.hover = None;
        cx.status.clear();
        self.update_readout(cx);
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.reset_transient();
        self.hover = None;
        self.selected = None;
        *self.dialog.borrow_mut() = None;
        *self.applied.borrow_mut() = None;
        cx.readout = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        self.hover = Some(p.world);
        if let Some(drag) = &mut self.drag {
            if p.down && !drag.moved && (p.screen - drag.start).length() >= DRAG_PX {
                drag.moved = true;
                cx.begin_change("Move Device");
            }
            if p.down && drag.moved {
                self.move_selected(cx, &p);
            }
        }
        if let (Some((_, end)), true) = (&mut self.rope, p.down) {
            *end = p.world;
        }
        self.update_readout(cx);
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
        let layer = load_electrical(cx.floor());
        let hit = device_at(&layer, p.world, cx.pick_tol());
        match self.variant {
            ElecVariant::Connection => self.connection_click(cx, hit),
            ElecVariant::AutoOutlets => {
                if auto_place_floor_outlets(cx) > 0 {
                    ToolResult::committed("Auto Place Outlets")
                } else {
                    ToolResult::consumed()
                }
            }
            _ if hit.is_some() => {
                self.selected = hit;
                self.drag = hit.map(|id| Drag {
                    id,
                    start: p.screen,
                    moved: false,
                });
                ToolResult::consumed()
            }
            ElecVariant::RopeLight => {
                self.selected = None;
                self.rope = Some((p.snapped, p.snapped));
                ToolResult::consumed()
            }
            v => {
                self.selected = None;
                match v.kind() {
                    Some(kind) => self.place_at(cx, kind, &p),
                    None => ToolResult::consumed(),
                }
            }
        }
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some((a, _)) = self.rope.take() {
            return self.place_rope(cx, a, p.snapped);
        }
        match self.drag.take() {
            Some(d) if d.moved => {
                cx.mark_dirty();
                ToolResult::committed("Move Device")
            }
            _ => ToolResult::ignored(),
        }
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        // An OK in the Electrical Service Specification applies right away.
        let _ = self.flush(cx);
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let layer = load_electrical(cx.floor());

        let Some(id) = device_at(&layer, p.world, cx.pick_tol()) else {
            return ToolResult::ignored();
        };
        self.drag = None;
        self.rope = None;
        self.selected = Some(id);
        if let Some(d) = layer.device(id) {
            *self.dialog.borrow_mut() = Some(ElectricalDialog::for_device(d, &layer));
        }
        ToolResult::consumed()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        if k.is(Key::Escape) {
            if self.connect_from.is_some() || self.rope.is_some() || self.drag.is_some() {
                self.reset_transient();
                return ToolResult::consumed();
            }
            if self.selected.take().is_some() {
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(Key::Delete) || k.is(Key::Backspace) {
            return self.delete_selected(cx);
        }
        if k.is(Key::Tab) {
            return self.flip_selected(cx);
        }
        let step = if k.modifiers.shift {
            FRAC_PI_2
        } else {
            TURN_STEP
        };
        if k.is(Key::ArrowLeft) {
            return self.turn_selected(cx, step);
        }
        if k.is(Key::ArrowRight) {
            return self.turn_selected(cx, -step);
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        let ghost = pal.ghost_stroke;
        let layer = load_electrical(cx.floor());
        // Rubber band of a rope light.
        if let Some((a, b)) = self.rope {
            if a.dist(b) > 1.0 {
                draw_symbol(painter, cam, &rope_light(a, b).symbol_world(), ghost, 1.5);
            }
        } else if let (Some(h), Some(kind)) = (self.hover, self.variant.kind()) {
            // The ghost of the device a click would place.
            if self.drag.is_none() && device_at(&layer, h, cx.pick_tol()).is_none() {
                let ghost_dev = match kind {
                    DeviceKind::RopeLight { .. } => Some(place_free(kind, h)),
                    _ => placement(cx, kind, h, h).ok(),
                };
                if let Some(d) = ghost_dev {
                    draw_symbol(painter, cam, &d.symbol_world(), ghost, 1.5);
                }
            }
        }
        // Selection and the connection in progress.
        let ring = |d: &Device, color: egui::Color32| {
            let r = (8.0 * cam.px_per_in as f32).max(8.0);
            painter.circle_stroke(
                cam.world_to_screen(d.position),
                r,
                egui::Stroke::new(2.0_f32, color),
            );
        };
        if let Some(d) = self.selected.and_then(|id| layer.device(id)) {
            ring(d, pal.selection);
        }
        if let (Some(from), Some(h)) = (
            self.connect_from.and_then(|id| layer.device(id)),
            self.hover,
        ) {
            let a = cam.world_to_screen(from.position);
            let b = cam.world_to_screen(h);
            painter.extend(Shape::dashed_line(
                &[a, b],
                egui::Stroke::new(1.5_f32, ghost),
                5.0,
                3.0,
            ));
        }
        if let Some(h) = self
            .hover
            .filter(|_| self.variant == ElecVariant::Connection)
        {
            if let Some(d) = device_at(&layer, h, cx.pick_tol()).and_then(|id| layer.device(id)) {
                ring(d, pal.hover);
            }
        }
        for c in layer
            .connections
            .iter()
            .filter(|c| self.selected.is_some_and(|s| c.from == s || c.to == s))
        {
            if let Some(arc) = layer.connection_arc(c) {
                let pts = arc_points(cam, &arc);
                if pts.len() >= 2 {
                    painter.add(Shape::line(pts, egui::Stroke::new(2.0_f32, pal.selection)));
                }
            }
        }
        // The Electrical Service Specification.
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

/// The wall of `floor` carrying `d`, if any.
pub fn host_wall<'a>(floor: &'a Floor, d: &Device) -> Option<&'a plan_core::Wall> {
    d.wall_id.and_then(|w| floor.wall(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{Project, WallKind};

    fn cx_with_room() -> (EditorContext, Vec<Id>) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 144.0),
            Point::new(0.0, 144.0),
        ];
        let ids = (0..4)
            .map(|i| {
                cx.project
                    .add_wall(0, c[i], c[(i + 1) % 4], 4.5, 96.0, WallKind::Interior)
            })
            .collect();
        cx.refresh();
        (cx, ids)
    }

    fn tool(v: ElecVariant) -> ElectricalTool {
        let mut t = ElectricalTool::default();
        t.set_variant(ToolId::ElectricalVariant(v));
        t
    }

    fn click(t: &mut ElectricalTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        let r = t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
        r
    }

    fn devices(cx: &EditorContext) -> Vec<Device> {
        load_electrical(cx.floor()).devices
    }

    #[test]
    fn outlet_near_a_wall_gets_the_wall_id_and_the_nearest_face() {
        let (mut cx, ids) = cx_with_room();
        let mut t = tool(ElecVariant::Outlet110);
        let r = click(&mut t, &mut cx, 100.3, 5.0);
        assert_eq!(r.commit.as_deref(), Some("Place 110V Outlet"));
        let d = &devices(&cx)[0];
        assert_eq!(d.wall_id, Some(ids[0]));
        assert!((d.position.x - 100.0).abs() < 1e-9, "snapped to 1\"");
        assert!((d.position.y - 2.25).abs() < 1e-9, "left face of the wall");
        assert!((d.angle - FRAC_PI_2).abs() < 1e-9, "faces into the room");
        assert_eq!(d.height, 12.0);
        assert_eq!(cx.readout.as_deref(), Some("Height: 12\""));

        // A click on the other side lands on the other face.
        click(&mut t, &mut cx, 160.0, -6.0);
        let d = &devices(&cx)[1];
        assert!((d.position.y + 2.25).abs() < 1e-9);
        assert!((d.angle + FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn far_from_walls_an_outlet_is_refused() {
        let (mut cx, _) = cx_with_room();
        let mut t = tool(ElecVariant::Outlet220);
        click(&mut t, &mut cx, 120.0, 72.0);
        assert!(devices(&cx).is_empty());
        assert!(cx.status.contains("wall"));
        assert!(!cx.can_undo());
    }

    #[test]
    fn switch_default_height_and_status() {
        let (mut cx, _) = cx_with_room();
        let mut t = tool(ElecVariant::Switch);
        click(&mut t, &mut cx, 30.0, 144.0);
        let d = &devices(&cx)[0];
        assert_eq!(d.kind, DeviceKind::Switch);
        assert_eq!(d.height, 48.0);
        assert_eq!(cx.readout.as_deref(), Some("Height: 48\""));
    }

    #[test]
    fn lights_snap_to_the_room_center_within_a_foot() {
        let (mut cx, _) = cx_with_room();
        let mut t = tool(ElecVariant::Light);
        click(&mut t, &mut cx, 126.0, 77.0);
        click(&mut t, &mut cx, 40.0, 30.0);
        let ds = devices(&cx);
        assert_eq!(ds[0].position, Point::new(120.0, 72.0));
        assert_eq!(ds[1].position, Point::new(40.0, 30.0));
        assert_eq!(ds[0].wall_id, None);
        assert_eq!(ds[0].height, DeviceKind::CeilingLight.default_height());
    }

    #[test]
    fn connection_creates_an_arc_record() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch), &mut cx, 30.0, 0.0);
        click(&mut tool(ElecVariant::Light), &mut cx, 120.0, 72.0);
        let mut t = tool(ElecVariant::Connection);
        click(&mut t, &mut cx, 30.0, 2.25);
        assert!(t.connect_from.is_some());
        let r = click(&mut t, &mut cx, 120.0, 72.0);
        assert_eq!(r.commit.as_deref(), Some("Electrical Connection"));
        let layer = load_electrical(cx.floor());
        assert_eq!(layer.connections.len(), 1);
        let c = &layer.connections[0];
        assert!(layer.connection_arc(c).is_some());
        assert!(c.arc_bulge.abs() >= 6.0);
        let light = layer.devices.iter().find(|d| d.kind.is_light()).unwrap();
        assert_eq!(light.switched_by, vec![c.from]);
        // Undo removes the arc, and a repeat is refused.
        assert_eq!(cx.undo().as_deref(), Some("Electrical Connection"));
        assert!(load_electrical(cx.floor()).connections.is_empty());
    }

    #[test]
    fn connection_needs_a_switch_first_and_refuses_duplicates() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch), &mut cx, 30.0, 0.0);
        click(&mut tool(ElecVariant::Light), &mut cx, 120.0, 72.0);
        let mut t = tool(ElecVariant::Connection);
        click(&mut t, &mut cx, 120.0, 72.0);
        assert!(t.connect_from.is_none());
        assert!(cx.status.contains("switch"));
        click(&mut t, &mut cx, 30.0, 2.25);
        click(&mut t, &mut cx, 120.0, 72.0);
        click(&mut t, &mut cx, 30.0, 2.25);
        click(&mut t, &mut cx, 120.0, 72.0);
        assert!(cx.status.contains("already"));
        assert_eq!(load_electrical(cx.floor()).connections.len(), 1);
    }

    #[test]
    fn auto_place_on_a_20_by_12_room_keeps_outlets_within_12_feet() {
        let (mut cx, ids) = cx_with_room();
        let mut t = tool(ElecVariant::AutoOutlets);
        let r = click(&mut t, &mut cx, 10.0, 10.0);
        assert_eq!(r.commit.as_deref(), Some("Auto Place Outlets"));
        let ds = devices(&cx);
        assert!(ds.len() >= 6, "{} outlets", ds.len());
        for id in ids {
            let wall = cx.floor().wall(id).unwrap().clone();
            let mut offs: Vec<f64> = ds
                .iter()
                .filter(|d| d.wall_id == Some(id))
                .map(|d| project_on_segment(d.position, wall.start, wall.end).0 * wall.length())
                .collect();
            offs.sort_by(f64::total_cmp);
            assert!(!offs.is_empty(), "wall {id} has no outlet");
            for w in offs.windows(2) {
                assert!(
                    w[1] - w[0] <= 144.0 + 1e-6,
                    "gap {} on wall {id}",
                    w[1] - w[0]
                );
            }
            assert!(offs[0] <= 144.0 && wall.length() - offs[offs.len() - 1] <= 144.0);
        }
        // A second click adds nothing.
        let n = ds.len();
        click(&mut t, &mut cx, 10.0, 10.0);
        assert_eq!(devices(&cx).len(), n);
        assert!(cx.status.contains("already"));
        // Undo removes them all.
        cx.undo();
        assert!(devices(&cx).is_empty());
    }

    #[test]
    fn room_names_pick_the_outlet_rules() {
        assert_eq!(room_function("Kitchen", ""), RoomFunction::Kitchen);
        assert_eq!(room_function("Master Bath", ""), RoomFunction::Bath);
        assert_eq!(room_function("Room 1", "Bedroom"), RoomFunction::Bedroom);
        assert_eq!(room_function("Room 2", ""), RoomFunction::Other);
        // A kitchen in the plan gets counter outlets.
        let (mut cx, _) = cx_with_room();
        let anchor = Point::new(120.0, 72.0);
        cx.project.floors[0]
            .room_names
            .push(plan_core::model::RoomName::new(
                anchor, "Kitchen", "Kitchen",
            ));
        auto_place_floor_outlets(&mut cx);
        assert!(devices(&cx).iter().any(|d| d.kind == DeviceKind::Gfci));
    }

    #[test]
    fn rope_light_takes_its_length_from_the_drag() {
        let (mut cx, _) = cx_with_room();
        let mut t = tool(ElecVariant::RopeLight);
        let a = PointerEvent::at(&cx, Point::new(20.0, 100.0));
        t.pointer_down(&mut cx, a.with_down(true));
        let b = PointerEvent::at(&cx, Point::new(140.0, 100.0));
        t.pointer_move(&mut cx, b.with_down(true));
        assert!(cx.readout.as_deref().unwrap().starts_with("Length:"));
        let r = t.pointer_up(&mut cx, b);
        assert_eq!(r.commit.as_deref(), Some("Place Rope Light"));
        let d = &devices(&cx)[0];
        assert_eq!(d.kind, DeviceKind::RopeLight { length: 120.0 });
        let ends = d.symbol_world();
        match &ends[0] {
            plan_electrical::Stroke::Line { a, b } => {
                assert!(a.dist(Point::new(20.0, 100.0)) < 1e-6);
                assert!(b.dist(Point::new(140.0, 100.0)) < 1e-6);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn dragging_slides_a_wall_device_and_moves_a_free_one() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Outlet110), &mut cx, 100.0, 0.0);
        click(&mut tool(ElecVariant::SmokeDetector), &mut cx, 40.0, 40.0);
        let mut t = tool(ElecVariant::Outlet110);
        // Drag the outlet along the wall (the pointer wanders off the wall).
        let down = PointerEvent::at(&cx, Point::new(100.0, 2.25));
        t.pointer_down(&mut cx, down.with_down(true));
        assert!(t.selected().is_some());
        let mut mv = PointerEvent::at(&cx, Point::new(180.4, 30.0));
        mv.screen = down.screen + egui::vec2(80.0, -20.0);
        t.pointer_move(&mut cx, mv.with_down(true));
        let r = t.pointer_up(&mut cx, mv);
        assert_eq!(r.commit.as_deref(), Some("Move Device"));
        let d = devices(&cx)
            .into_iter()
            .find(|d| d.kind.is_outlet())
            .unwrap();
        assert!((d.position.x - 180.0).abs() < 1e-9 && (d.position.y - 2.25).abs() < 1e-9);
        // One undo step puts it back.
        assert_eq!(cx.undo().as_deref(), Some("Move Device"));
        let d = devices(&cx)
            .into_iter()
            .find(|d| d.kind.is_outlet())
            .unwrap();
        assert!((d.position.x - 100.0).abs() < 1e-9);

        // The free device moves anywhere.
        let down = PointerEvent::at(&cx, Point::new(40.0, 40.0));
        t.pointer_down(&mut cx, down.with_down(true));
        let mut mv = PointerEvent::at(&cx, Point::new(60.0, 90.0));
        mv.screen = down.screen + egui::vec2(40.0, -100.0);
        t.pointer_move(&mut cx, mv.with_down(true));
        t.pointer_up(&mut cx, mv);
        let d = devices(&cx)
            .into_iter()
            .find(|d| d.kind == DeviceKind::SmokeDetector)
            .unwrap();
        assert_eq!(d.position, Point::new(60.0, 90.0));
    }

    #[test]
    fn a_press_without_travel_only_selects() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Outlet110), &mut cx, 100.0, 0.0);
        let before = cx.can_undo();
        let mut t = tool(ElecVariant::Outlet110);
        click(&mut t, &mut cx, 101.0, 2.0);
        assert!(t.selected().is_some());
        assert_eq!(devices(&cx).len(), 1, "no second outlet");
        assert_eq!(cx.can_undo(), before);
    }

    #[test]
    fn keys_flip_turn_and_delete_the_selection() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Outlet110), &mut cx, 100.0, 0.0);
        click(&mut tool(ElecVariant::SmokeDetector), &mut cx, 40.0, 40.0);
        let mut t = tool(ElecVariant::Outlet110);

        click(&mut t, &mut cx, 100.0, 2.25);
        let r = t.key(&mut cx, KeyEvent::key(Key::Tab));
        assert_eq!(r.commit.as_deref(), Some("Flip Device"));
        let o = devices(&cx)
            .into_iter()
            .find(|d| d.kind.is_outlet())
            .unwrap();
        assert!((o.position.y + 2.25).abs() < 1e-9, "other face");
        let r = t.key(&mut cx, KeyEvent::key(Key::ArrowLeft));
        assert!(r.commit.is_none(), "wall devices do not turn");

        click(&mut t, &mut cx, 40.0, 40.0);
        let r = t.key(&mut cx, KeyEvent::key(Key::ArrowLeft));
        assert_eq!(r.commit.as_deref(), Some("Rotate Device"));
        let s = devices(&cx)
            .into_iter()
            .find(|d| d.kind == DeviceKind::SmokeDetector)
            .unwrap();
        assert!((s.angle - TURN_STEP).abs() < 1e-9);

        let r = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(r.commit.as_deref(), Some("Delete Device"));
        assert_eq!(devices(&cx).len(), 1);
        cx.undo();
        assert_eq!(devices(&cx).len(), 2);
        assert!(!t.key(&mut cx, KeyEvent::escape()).consumed || t.selected().is_none());
    }

    #[test]
    fn double_click_opens_the_specification_and_ok_applies_it() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Light), &mut cx, 120.0, 72.0);
        let mut t = tool(ElecVariant::Light);
        let p = PointerEvent::at(&cx, Point::new(121.0, 72.0));
        assert!(t.double_click(&mut cx, p).consumed);
        assert!(t.dialog_open());
        // While the dialog is open the canvas ignores clicks.
        let n = cx.can_undo();
        t.pointer_down(&mut cx, p.with_down(true));
        assert_eq!(devices(&cx).len(), 1);
        assert_eq!(cx.can_undo(), n);
        let mut draft = t.dialog.borrow().as_ref().unwrap().draft().clone();
        draft.height = 90.0;
        draft.label = "Hall".into();
        draft.circuit = Some(3);
        *t.applied.borrow_mut() = Some(draft);
        let r = t.pointer_move(&mut cx, p);
        assert_eq!(
            r.commit.as_deref(),
            Some("Electrical Service Specification")
        );
        let d = &devices(&cx)[0];
        assert_eq!(
            (d.height, d.label.as_str(), d.circuit),
            (90.0, "Hall", Some(3))
        );
    }

    #[test]
    fn devices_survive_save_and_load_and_undo_removes_them() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Gfci), &mut cx, 60.0, 0.0);
        click(&mut tool(ElecVariant::CeilingFan), &mut cx, 100.0, 60.0);
        let project = Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        let back = crate::editor::site_view::load_electrical(&project.floors[0]);
        assert_eq!(back.devices.len(), 2);
        assert_eq!(back.devices[0].kind, DeviceKind::Gfci);
        assert_eq!(cx.undo().as_deref(), Some("Place Ceiling Fan"));
        assert_eq!(cx.undo().as_deref(), Some("Place GFCI Outlet"));
        assert!(devices(&cx).is_empty());
    }

    #[test]
    fn the_variant_id_round_trips_through_the_tool_set() {
        use crate::tools::ToolSet;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut set = ToolSet::new();
        set.set_active(&mut cx, ToolId::ElectricalVariant(ElecVariant::Switch3Way));
        assert_eq!(
            set.active_id(),
            ToolId::ElectricalVariant(ElecVariant::Switch3Way)
        );
        assert_eq!(cx.readout.as_deref(), Some("Height: 48\""));
        set.set_active(&mut cx, ToolId::ElectricalVariant(ElecVariant::Connection));
        assert_eq!(set.active().name(), "Electrical Connection");
        set.set_active(&mut cx, ToolId::Select);
        assert_eq!(set.active_id(), ToolId::Select);
    }

    #[test]
    fn overlay_and_dialog_draw_without_panicking() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch), &mut cx, 30.0, 0.0);
        click(&mut tool(ElecVariant::Light), &mut cx, 120.0, 72.0);
        let mut t = tool(ElecVariant::Connection);
        click(&mut t, &mut cx, 30.0, 2.25);
        let ev = PointerEvent::at(&cx, Point::new(80.0, 50.0));
        t.pointer_move(&mut cx, ev);
        let ev = PointerEvent::at(&cx, Point::new(30.0, 2.25));
        t.double_click(&mut cx, ev);
        let egui_ctx = egui::Context::default();
        for variant in [
            ElecVariant::Connection,
            ElecVariant::RopeLight,
            ElecVariant::Gfci,
        ] {
            t.set_variant(ToolId::ElectricalVariant(variant));
            let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, painter) =
                        ui.allocate_painter(egui::Vec2::new(800.0, 600.0), egui::Sense::hover());
                    let mut cam = Camera::default_view();
                    cam.rect = painter.clip_rect();
                    t.draw_overlay(&cx, &painter, &cam);
                });
            });
        }
    }
}
