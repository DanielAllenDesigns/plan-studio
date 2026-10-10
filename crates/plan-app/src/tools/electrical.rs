//! Electrical tools (CB-62..CB-67 in `docs/parity/cabinets-stairs-framing-terrain-library.md`;
//! Round 16 brief 25: E-19..E-22, E-30..E-33).
//!
//! One tool object with a flavor per flyout entry ([`ElecVariant`]): 110V,
//! quad, 220V, GFCI and floor outlets, switches (single, 3-way, 4-way,
//! dimmer), ceiling, recessed, pendant and wall lights, rope lights, ceiling
//! fans, smoke and CO detectors, thermostats, doorbells, data, phone and TV
//! jacks, the electrical panel, Electrical Connection, Auto Place Outlets and
//! Auto Place Switches.
//!
//! * The 110V, GFCI and 220V outlet tools, the Switch tool and the Light
//!   tool read the click (manual pp. 693-694, [`placement_full`]): a wall
//!   within 12" takes a wall device on the face nearest the click, at the
//!   plan's Electrical Defaults height; over a base cabinet the height is
//!   measured up from the counter (unless the cabinet holds a sink) and a
//!   110V outlet over a kitchen or bath counter is a GFCI; on the side of a
//!   cabinet or soffit the device sits 32" up the box; on an exterior wall,
//!   or in an exterior room, an outlet, switch or wall light is the
//!   weatherproof type; in the middle of a room an outlet goes on the floor
//!   (the ceiling of a garage or slab room) and a light on the ceiling;
//!   outside any room a light is a path light. The other flavors place their
//!   own kind, wall devices on the nearest wall within 12" (CB-63).
//! * The symbol each of those tools places is the plan's Default Library
//!   Object for it (Electrical Defaults); double-clicking an Electrical Tools
//!   button opens the defaults (the toolbar calls
//!   [`crate::dialogs::default_pages::electrical::request_open_for`]).
//! * Clicking an existing device selects it (the Select tool does not know
//!   devices yet): drag to move (wall devices slide along their wall, free
//!   devices move freely), Tab flips the side, Left/Right turn a free device
//!   (Shift = 90 degrees), Delete removes it and a double-click opens the
//!   Electrical Service Specification. The selected device shows the
//!   diamond-shaped Electrical Connection handle below its symbol.
//! * Electrical Connection: click a switch (or outlet), then every light it
//!   controls (Esc ends the run), or press on any device and drag to the
//!   next (or to open ground for a free spline); the splines are stored with
//!   the layer (CB-67, E-21). A selected connection shows a handle on every
//!   vertex and at both ends: drag a vertex to reshape the curve, double-click
//!   the curve to add one, drag an end off its device to detach it (it stays
//!   as a free end) or onto another to attach it. Two 3-way (or 4-way)
//!   switches clicked in turn are wired as a pair and both control the lights
//!   of either.
//! * Rope Light: click and drag a path; the rope light is a record of the
//!   layer with its Rope Light Specification, edited like an open polyline
//!   (drag a vertex, drag a midpoint handle to add one, Delete removes the
//!   selected vertex's rope).
//! * Auto Place Switches: a switch 6" past the latch jamb of every door of
//!   every room, a ceiling light for a room that has none and the connections
//!   (CB-65); a room with two doors gets a 3-way pair.
//! * Auto Place Outlets: one click places outlets for every room of the
//!   current floor from its name and type (CB-64).
//! * Set as Default (edit tool) copies the selected device, connection or
//!   rope light into the Electrical, Electrical Connection or Rope Light
//!   defaults ([`run_command`]).
//!
//! Devices live in `Floor.electrical` through `editor::site_view`.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::electrical::{DeviceDraft, ElectricalDialog};
use crate::dialogs::rope_light::{RopeDraft, RopeLightDialog};
use crate::dialogs::Outcome;
use crate::editor::site_view::{
    device_at, draw_symbol, edit_electrical, flip_side, load_electrical, slide_on_wall,
};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext, ObjectRef};
use eframe::egui::{self, Key, Pos2, Shape};
use plan_cabinets::{Cabinet, CabinetKind, CutoutKind};
use plan_core::geometry::point_in_polygon;
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::OpeningKind;
use plan_core::{Floor, Id, Room, Wall};
use plan_electrical::{
    auto_place_exterior_outlets, auto_place_outlets, auto_place_room_light, auto_place_switch,
    connect_drawn, connect_with, face_is_exterior, kind_for_setting, place_free, place_on_wall,
    AutoOutletOptions, ConnEnd, Device, DeviceKind, DeviceOptions, ElectricalDefaults,
    ElectricalLayer, HeightContext, Mount, RoomFunction, RopeLightPath, WallSide,
};
use std::cell::{Cell, RefCell};
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
/// How near a click must be to a connection or rope handle to take it, inches.
const BEND_PICK: f64 = 6.0;
/// How far from a cabinet or soffit side a click still snaps to it, inches.
const CABINET_SIDE_SNAP: f64 = 6.0;
/// How far out from a wall face the cabinet probe of the height rules looks, inches.
const COUNTER_PROBE: f64 = 6.0;

/// The flavors of the electrical tool, one per flyout entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElecVariant {
    Outlet110,
    Outlet110Quad,
    Outlet220,
    Gfci,
    OutletFloor,
    OutletWp,
    OutletDedicated,
    Light,
    RecessedLight,
    PendantLight,
    WallLight,
    RopeLight,
    Switch,
    Switch3Way,
    Switch4Way,
    SwitchDimmer,
    CeilingFan,
    SmokeDetector,
    CoDetector,
    Thermostat,
    Doorbell,
    DataJack,
    PhoneJack,
    TvJack,
    Panel,
    Connection,
    AutoOutlets,
    AutoSwitches,
}

impl ElecVariant {
    /// Every flavor, in flyout order.
    pub const ALL: [ElecVariant; 28] = [
        ElecVariant::Outlet110,
        ElecVariant::Outlet110Quad,
        ElecVariant::Outlet220,
        ElecVariant::Gfci,
        ElecVariant::OutletFloor,
        ElecVariant::OutletWp,
        ElecVariant::OutletDedicated,
        ElecVariant::Switch,
        ElecVariant::Switch3Way,
        ElecVariant::Switch4Way,
        ElecVariant::SwitchDimmer,
        ElecVariant::Light,
        ElecVariant::RecessedLight,
        ElecVariant::PendantLight,
        ElecVariant::WallLight,
        ElecVariant::RopeLight,
        ElecVariant::CeilingFan,
        ElecVariant::SmokeDetector,
        ElecVariant::CoDetector,
        ElecVariant::Thermostat,
        ElecVariant::Doorbell,
        ElecVariant::DataJack,
        ElecVariant::PhoneJack,
        ElecVariant::TvJack,
        ElecVariant::Panel,
        ElecVariant::Connection,
        ElecVariant::AutoOutlets,
        ElecVariant::AutoSwitches,
    ];

    /// The device a click places, if this flavor places one.
    pub fn kind(self) -> Option<DeviceKind> {
        Some(match self {
            ElecVariant::Outlet110 => DeviceKind::Outlet110,
            ElecVariant::Outlet110Quad => DeviceKind::Outlet110Quad,
            ElecVariant::Outlet220 => DeviceKind::Outlet220,
            ElecVariant::Gfci => DeviceKind::Gfci,
            ElecVariant::OutletFloor => DeviceKind::OutletFloor,
            ElecVariant::OutletWp => DeviceKind::OutletWp,
            ElecVariant::OutletDedicated => DeviceKind::OutletDedicated,
            ElecVariant::Light => DeviceKind::CeilingLight,
            ElecVariant::RecessedLight => DeviceKind::RecessedCan,
            ElecVariant::PendantLight => DeviceKind::PendantLight,
            ElecVariant::WallLight => DeviceKind::WallSconce,
            ElecVariant::RopeLight => DeviceKind::RopeLight {
                length: DEFAULT_ROPE,
            },
            ElecVariant::Switch => DeviceKind::Switch,
            ElecVariant::Switch3Way => DeviceKind::Switch3Way,
            ElecVariant::Switch4Way => DeviceKind::Switch4Way,
            ElecVariant::SwitchDimmer => DeviceKind::SwitchDimmer,
            ElecVariant::CeilingFan => DeviceKind::CeilingFan,
            ElecVariant::SmokeDetector => DeviceKind::SmokeDetector,
            ElecVariant::CoDetector => DeviceKind::CoDetector,
            ElecVariant::Thermostat => DeviceKind::Thermostat,
            ElecVariant::Doorbell => DeviceKind::Doorbell,
            ElecVariant::DataJack => DeviceKind::DataJack,
            ElecVariant::PhoneJack => DeviceKind::PhoneJack,
            ElecVariant::TvJack => DeviceKind::TvJack,
            ElecVariant::Panel => DeviceKind::Panel,
            ElecVariant::Connection | ElecVariant::AutoOutlets | ElecVariant::AutoSwitches => {
                return None
            }
        })
    }

    /// Chief's name of the flyout entry.
    pub fn name(self) -> &'static str {
        match self {
            ElecVariant::Light => "Light",
            ElecVariant::RecessedLight => "Recessed Light",
            ElecVariant::WallLight => "Wall Light",
            ElecVariant::Panel => "Electrical Panel",
            ElecVariant::Connection => "Electrical Connection",
            ElecVariant::AutoOutlets => "Auto Place Outlets",
            ElecVariant::AutoSwitches => "Auto Place Switches",
            v => v.kind().map_or("Electrical", |k| k.name()),
        }
    }

    /// Does this tool read the click (wall, cabinet, room, outdoors) to pick
    /// the type and height of what it places?
    pub fn is_contextual(self) -> bool {
        matches!(
            self,
            ElecVariant::Outlet110
                | ElecVariant::Gfci
                | ElecVariant::Outlet220
                | ElecVariant::Switch
                | ElecVariant::Light
                | ElecVariant::WallLight
        )
    }

    /// The contextual tool whose built-in kind is `kind`; any other kind is
    /// placed by its own flavor. Used by [`placement`].
    pub fn for_kind(kind: DeviceKind) -> ElecVariant {
        ElecVariant::ALL
            .iter()
            .copied()
            .find(|v| {
                v.kind()
                    .is_some_and(|k| std::mem::discriminant(&k) == std::mem::discriminant(&kind))
            })
            .unwrap_or(ElecVariant::Outlet110)
    }

    /// The tool whose Electrical Defaults page (tab) the double-click of its
    /// toolbar button opens.
    pub fn defaults_tab(self) -> DefaultsTab {
        match self {
            ElecVariant::Connection => DefaultsTab::Connection,
            ElecVariant::RopeLight => DefaultsTab::RopeLight,
            _ => DefaultsTab::Electrical,
        }
    }
}

/// The three parts of the Electrical defaults dialogs (manual p. 692).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DefaultsTab {
    /// Default Library Objects and Default Heights.
    #[default]
    Electrical,
    /// Electrical Connection Defaults.
    Connection,
    /// Rope Light Defaults.
    RopeLight,
}

/// The kind `v` places under the plan's Default Library Objects.
pub fn effective_kind(defaults: &ElectricalDefaults, v: ElecVariant) -> Option<DeviceKind> {
    let builtin = v.kind()?;
    Some(match builtin {
        DeviceKind::RopeLight { .. } => builtin,
        _ => defaults.object(v.name(), builtin),
    })
}

/// What a selected Electrical Tools object is, for the edit commands.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sel {
    #[default]
    None,
    Device(Id),
    Connection(usize),
    Rope(Id),
}

thread_local! {
    /// The electrical tool's selection, published for the Edit commands.
    static SELECTION: Cell<Sel> = const { Cell::new(Sel::None) };
}

/// A press on a device that may turn into a move.
struct Drag {
    id: Id,
    start: Pos2,
    moved: bool,
}

/// A gesture of the Electrical Connection tool.
#[derive(Clone, Copy, Debug)]
enum ConnDrag {
    /// A press on a device (or on its diamond handle): a click wires, a drag
    /// draws a spline to the next object.
    FromDevice {
        id: Id,
        start: Pos2,
        moved: bool,
        diamond: bool,
    },
    /// A press on open ground: a drag draws a free spline.
    FromPoint { at: Point, start: Pos2, moved: bool },
    /// A vertex of the selected connection being dragged.
    Vertex { index: usize, handle: usize },
    /// An end of the selected connection being dragged.
    End {
        index: usize,
        end: ConnEnd,
        start: Pos2,
        moved: bool,
    },
}

/// An edit of the selected rope light.
#[derive(Clone, Copy, Debug)]
struct RopeDrag {
    id: Id,
    vertex: usize,
    moved: bool,
}

pub struct ElectricalTool {
    variant: ElecVariant,
    hover: Option<Point>,
    selected: Option<Id>,
    /// The selected connection (index in the layer).
    sel_conn: Option<usize>,
    /// The selected rope light.
    sel_rope: Option<Id>,
    drag: Option<Drag>,
    /// Rope light: where the press landed and where the pointer is now.
    rope: Option<(Point, Point)>,
    rope_drag: Option<RopeDrag>,
    /// Electrical Connection: the switch picked first.
    connect_from: Option<Id>,
    conn: Option<ConnDrag>,
    /// A connection vertex drag has moved (its undo step is open).
    vertex_moved: bool,
    /// The specification dialog, drawn from `draw_overlay` (which only has `&self`).
    dialog: RefCell<Option<ElectricalDialog>>,
    /// An OK from the dialog, applied at the next tool event.
    applied: RefCell<Option<DeviceDraft>>,
    /// The Rope Light Specification and its OK.
    rope_dialog: RefCell<Option<RopeLightDialog>>,
    rope_applied: RefCell<Option<RopeDraft>>,
}

impl Default for ElectricalTool {
    fn default() -> Self {
        Self {
            variant: ElecVariant::Outlet110,
            hover: None,
            selected: None,
            sel_conn: None,
            sel_rope: None,
            drag: None,
            rope: None,
            rope_drag: None,
            connect_from: None,
            conn: None,
            vertex_moved: false,
            dialog: RefCell::new(None),
            applied: RefCell::new(None),
            rope_dialog: RefCell::new(None),
            rope_applied: RefCell::new(None),
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
    let (wall, offset, side) = wall_hit(cx, p)?;
    Some(place_on_wall(kind, wall, offset, side))
}

/// The wall nearest `p` within reach, the snapped offset along it and the
/// face `p` is on.
fn wall_hit(cx: &EditorContext, p: Point) -> Option<(&Wall, f64, WallSide)> {
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
    Some((wall, offset.clamp(0.0, wall.length()), side))
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

/// A device a click creates and the options it starts with (mounting, host).
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub device: Device,
    pub options: DeviceOptions,
}

/// The function and type names of `room` (for Room Type rules).
fn room_names(cx: &EditorContext, room: &Room) -> (String, String) {
    let ty = room
        .name_entry(&cx.floor().room_names)
        .map(|n| n.room_type.clone())
        .unwrap_or_default();
    let function = cx
        .defaults
        .room_type(&ty)
        .map_or_else(|| ty.clone(), |t| t.function.clone());
    (function, ty)
}

/// The electrical rules of the room type of `room` (manual p. 447).
fn room_rules(cx: &EditorContext, room: &Room) -> plan_core::rooms::ElectricalRules {
    let (f, t) = room_names(cx, room);
    plan_core::rooms::electrical_rules(&f, &t)
}

/// Is `room` an exterior room (a deck, balcony or court)?
fn room_is_exterior(cx: &EditorContext, room: &Room) -> bool {
    room_rules(cx, room).weatherproof
}

/// The room that holds `p`, if any.
fn room_at(cx: &EditorContext, p: Point) -> Option<&Room> {
    cx.rooms.iter().find(|r| r.contains(p))
}

/// The base cabinets and their kinds that carry a counter.
fn is_base(c: &Cabinet) -> bool {
    matches!(
        c.kind,
        CabinetKind::Base
            | CabinetKind::CornerBase
            | CabinetKind::BlindBase
            | CabinetKind::BaseFiller
    )
}

/// Does the cabinet hold a sink (a sink face or a sink hole in its top)?
fn has_sink(c: &Cabinet) -> bool {
    c.face.has_appliance("Sink") || c.cutouts.iter().any(|k| k.kind == CutoutKind::Sink)
}

/// The cabinet or soffit side nearest `p` within the snap distance: the
/// cabinet, the point on its side and the side's outward direction. Sides
/// that stand against a wall face are not offered (the wall is).
fn cabinet_side_hit(
    cabs: &[Cabinet],
    walls: &[Wall],
    p: Point,
    reach: f64,
) -> Option<(usize, Point, Point, f64)> {
    let mut best: Option<(usize, Point, Point, f64)> = None;
    for (ci, c) in cabs.iter().enumerate() {
        if matches!(
            c.kind,
            CabinetKind::Shelf
                | CabinetKind::CustomCountertop
                | CabinetKind::CustomBacksplash
                | CabinetKind::CounterHole
        ) {
            continue;
        }
        let poly = c.footprint();
        let n = poly.len();
        if n < 3 {
            continue;
        }
        let centroid = Point::new(
            poly.iter().map(|q| q.x).sum::<f64>() / n as f64,
            poly.iter().map(|q| q.y).sum::<f64>() / n as f64,
        );
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            if a.dist(b) < 3.0 {
                continue;
            }
            let mid = Point::lerp(a, b, 0.5);
            let against_wall = walls
                .iter()
                .any(|w| dist_to_segment(mid, w.start, w.end) <= w.thickness * 0.5 + 1.0);
            if against_wall {
                continue;
            }
            let d = dist_to_segment(p, a, b);
            if d > reach || best.as_ref().is_some_and(|x| d >= x.3) {
                continue;
            }
            let mut out = b.sub(a).perp().normalized();
            if out.dot(mid.sub(centroid)) < 0.0 {
                out = -out;
            }
            let (_, on_edge) = project_on_segment(p, a, b);
            best = Some((ci, on_edge, out, d));
        }
    }
    best
}

/// The wall device `kind` on `wall` at `offset` on `side` with the height
/// rules of its place: above a base cabinet it sits up from the counter,
/// except over a sink (the plain Outlet height).
fn placed_on_wall(
    cx: &EditorContext,
    defaults: &ElectricalDefaults,
    tool: ElecVariant,
    wall: &Wall,
    offset: f64,
    side: WallSide,
) -> Placed {
    let exterior = face_is_exterior(wall, side, offset, &cx.rooms, &|r| room_is_exterior(cx, r));
    let wall_light = matches!(tool, ElecVariant::WallLight | ElecVariant::Light);
    let mut kind = if wall_light {
        if exterior {
            defaults.object("Wall Light (Exterior)", DeviceKind::WallLightExterior)
        } else {
            defaults.object("Wall Light", DeviceKind::WallSconce)
        }
    } else {
        kind_for_setting(
            effective_kind(defaults, tool).unwrap_or(DeviceKind::Outlet110),
            exterior,
        )
    };
    // The cabinet standing against this face of the wall, if any.
    let normal = wall.normal() * side.sign();
    let probe = wall.point_at(offset) + normal * (wall.thickness * 0.5 + COUNTER_PROBE);
    let cabs = crate::editor::placed::load_cabinets(cx.floor());
    let counter = cabs
        .iter()
        .find(|c| is_base(c) && c.elevation < 6.0 && point_in_polygon(probe, &c.footprint()));
    let mut ctx = HeightContext::Wall;
    if let Some(c) = counter {
        if !has_sink(c) && kind.height_group().is_some() {
            ctx = HeightContext::AboveCounter {
                counter_top: c.elevation + c.height,
            };
            // A 110V outlet over the counter of a kitchen or bath is a GFCI.
            let wet = room_at(cx, probe).is_some_and(|r| room_rules(cx, r).gfci_over_base_cabinets);
            if wet && matches!(kind, DeviceKind::Outlet110 | DeviceKind::Outlet110Quad) {
                kind = DeviceKind::Gfci;
            }
        }
    }
    let mut dev = place_on_wall(kind, wall, offset, side);
    dev.height = defaults.height_for(kind, ctx);
    Placed {
        device: dev,
        options: DeviceOptions::default(),
    }
}

/// The device a click at `world` (`snapped` for free devices) creates with
/// the tool `tool`, and the options it starts with (E-30). The contextual
/// tools read the click ([`ElecVariant::is_contextual`]); the others place
/// their own kind.
pub fn placement_full(
    cx: &EditorContext,
    tool: ElecVariant,
    world: Point,
    snapped: Point,
) -> Result<Placed, &'static str> {
    let defaults = ElectricalDefaults::load(&cx.project);
    let kind = effective_kind(&defaults, tool).ok_or("This tool places no device")?;
    if !tool.is_contextual() {
        let mut dev = if kind.is_wall_mounted() {
            wall_placement(cx, kind, world).ok_or("Click on a wall to place this device")?
        } else if kind.is_ceiling() {
            ceiling_placement(cx, kind, snapped)
        } else {
            place_free(kind, snapped)
        };
        // The plan's Electrical Defaults decide the mounting height (12" / 48" / ...).
        defaults.apply(&mut dev);
        return Ok(Placed {
            device: dev,
            options: DeviceOptions::default(),
        });
    }
    // The contextual tools: a wall, the side of a cabinet or soffit, or open floor.
    let wall = wall_hit(cx, world);
    let cabs = crate::editor::placed::load_cabinets(cx.floor());
    let reach = CABINET_SIDE_SNAP.max(cx.pick_tol());
    let cab = cabinet_side_hit(&cabs, &cx.floor().walls, world, reach);
    let wall_face_dist = wall
        .as_ref()
        .map(|(w, _, _)| (dist_to_segment(world, w.start, w.end) - w.thickness * 0.5).max(0.0));
    // A cabinet side wins over a wall only when it is nearer.
    if let Some((ci, at, out, d)) = cab {
        if wall_face_dist.is_none_or(|w| d < w) {
            return Ok(placed_on_cabinet(&defaults, tool, &cabs[ci], at, out));
        }
    }
    if let Some((w, offset, side)) = wall {
        return Ok(placed_on_wall(cx, &defaults, tool, w, offset, side));
    }
    placed_in_the_open(cx, &defaults, tool, kind, &cabs, snapped)
}

/// A wall-type device on the side of a cabinet or soffit: along the box
/// edge-on, `On Cabinet Side` up from its bottom (kept on the box).
fn placed_on_cabinet(
    defaults: &ElectricalDefaults,
    tool: ElecVariant,
    cab: &Cabinet,
    at: Point,
    out: Point,
) -> Placed {
    let wall_light = matches!(tool, ElecVariant::WallLight | ElecVariant::Light);
    let kind = if wall_light {
        defaults.object("Wall Light", DeviceKind::WallSconce)
    } else {
        effective_kind(defaults, tool).unwrap_or(DeviceKind::Outlet110)
    };
    let mut dev = place_free(kind, at);
    dev.angle = out.angle();
    dev.height = defaults.height_for(
        kind,
        HeightContext::CabinetSide {
            bottom: cab.elevation,
            top: cab.elevation + cab.height,
        },
    );
    let options = DeviceOptions {
        mount: Mount::CabinetSide,
        host: Some(cab.id),
        ..DeviceOptions::default()
    };
    Placed {
        device: dev,
        options,
    }
}

/// A click away from walls and cabinet sides: outlets go on the floor (the
/// ceiling of a garage or slab room, weatherproof outdoors), lights on the
/// ceiling (the bottom of a soffit) or, outside any room, a path light; a
/// switch needs a wall.
fn placed_in_the_open(
    cx: &EditorContext,
    _defaults: &ElectricalDefaults,
    tool: ElecVariant,
    kind: DeviceKind,
    cabs: &[Cabinet],
    snapped: Point,
) -> Result<Placed, &'static str> {
    let room = room_at(cx, snapped);
    let exterior_room = room.is_some_and(|r| room_is_exterior(cx, r));
    match tool {
        ElecVariant::Outlet110 | ElecVariant::Gfci | ElecVariant::Outlet220 => {
            let function = room.map(|r| room_names(cx, r).0).unwrap_or_default();
            let mut options = DeviceOptions::default();
            let (kind, height) = if room.is_none() || exterior_room {
                // Outdoors, on a deck or porch floor: weatherproof.
                options.mount = Mount::Floor;
                (kind_for_setting(kind, true), 0.0)
            } else if matches!(function.as_str(), "Garage" | "Slab")
                && tool == ElecVariant::Outlet110
            {
                options.mount = Mount::Ceiling;
                (kind, DeviceKind::CeilingLight.default_height())
            } else if tool == ElecVariant::Outlet220 {
                options.mount = Mount::Floor;
                (kind, 0.0)
            } else {
                options.mount = Mount::Floor;
                (DeviceKind::OutletFloor, 0.0)
            };
            let mut dev = place_free(kind, snapped);
            dev.height = height;
            Ok(Placed {
                device: dev,
                options,
            })
        }
        ElecVariant::Light => {
            if room.is_none() {
                let pk = DeviceKind::PathLight;
                let mut dev = place_free(pk, snapped);
                dev.height = pk.default_height();
                return Ok(Placed {
                    device: dev,
                    options: DeviceOptions::default(),
                });
            }
            let mut p = ceiling_placement(cx, kind, snapped);
            // Inside a soffit: the fixture hangs from the soffit's bottom.
            if let Some(s) = cabs.iter().find(|c| {
                c.kind == CabinetKind::Soffit && point_in_polygon(snapped, &c.footprint())
            }) {
                p.height = s.elevation;
            }
            Ok(Placed {
                device: p,
                options: DeviceOptions {
                    mount: Mount::Ceiling,
                    ..DeviceOptions::default()
                },
            })
        }
        _ => Err("Click on a wall to place this device"),
    }
}

/// The device a click at `world` (`snapped` for free devices) creates. `kind`
/// picks the tool: the built-in kind of a contextual tool (110V, GFCI, 220V
/// outlet, Switch, Light, Wall Light) reads the click; any other kind is
/// placed as itself.
pub fn placement(
    cx: &EditorContext,
    kind: DeviceKind,
    world: Point,
    snapped: Point,
) -> Result<Device, &'static str> {
    placement_full(cx, ElecVariant::for_kind(kind), world, snapped).map(|p| p.device)
}

/// A rope light along `points` with the Rope Light Defaults of the plan; a
/// path that starts under a wall cabinet hangs from its bottom.
pub fn rope_light_path(cx: &EditorContext, points: Vec<Point>) -> RopeLightPath {
    let mut spec = ElectricalDefaults::load(&cx.project).rope;
    if let Some(h) = under_wall_cabinet(cx, &points) {
        spec.reference = plan_electrical::RopeReference::Floor;
        spec.height = h;
    }
    RopeLightPath::new(points, spec)
}

/// The bottom of the wall cabinet the start of `points` lies under, if any.
pub fn under_wall_cabinet(cx: &EditorContext, points: &[Point]) -> Option<f64> {
    let first = points.first()?;
    let probe = match points.get(1) {
        Some(b) => Point::lerp(*first, *b, 0.5),
        None => *first,
    };
    crate::editor::placed::load_cabinets(cx.floor())
        .iter()
        .find(|c| {
            matches!(
                c.kind,
                CabinetKind::Wall | CabinetKind::CornerWall | CabinetKind::BlindWall
            ) && c.elevation > 12.0
                && (point_in_polygon(probe, &c.footprint())
                    || point_in_polygon(*first, &c.footprint()))
        })
        .map(|c| c.elevation)
}

/// The rope lights that tray ceilings ask for (`plan_core::tray::rope_light_paths`),
/// as closed rope light paths with the plan's Rope Light Defaults. They are
/// computed from the trays, not stored: moving the tray moves its rope light.
pub fn tray_rope_lights(cx: &EditorContext) -> Vec<RopeLightPath> {
    let spec = ElectricalDefaults::load(&cx.project).rope;
    super::tray_ceiling::geoms(cx)
        .iter()
        .filter_map(|g| cx.floor().tray(g.id).map(|rec| (g, rec)))
        .flat_map(|(g, rec)| plan_core::tray::rope_light_paths(g, rec))
        .map(|r| RopeLightPath::from_tray_path(r.tray, &r.name, &r.points, r.elevation, &spec))
        .collect()
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

/// The footprints of the floor's counter-carrying cabinets (base, corner and
/// blind base, fillers, anything with a countertop): where kitchen counter
/// outlets belong.
pub fn counter_runs(floor: &plan_core::Floor) -> Vec<Vec<plan_core::Point>> {
    use plan_cabinets::CabinetKind as K;
    crate::editor::placed::load_cabinets(floor)
        .iter()
        .filter(|c| {
            c.countertop.is_some()
                || matches!(
                    c.kind,
                    K::Base | K::CornerBase | K::BlindBase | K::BaseFiller
                )
        })
        .map(plan_cabinets::Cabinet::footprint)
        .collect()
}

/// Auto Place Outlets for every room of the current floor (CB-64). Returns the
/// number of outlets added; outlets already in place are not duplicated.
pub fn auto_place_floor_outlets(cx: &mut EditorContext) -> usize {
    cx.refresh();
    let rooms = cx.rooms.clone();
    let types: Vec<(String, RoomFunction)> = rooms
        .iter()
        .map(|r| {
            let (name, ty) = r
                .name_entry(&cx.floor().room_names)
                .map_or((r.label.clone(), String::new()), |n| {
                    (n.name.clone(), n.room_type.clone())
                });
            (r.label.clone(), room_function(&name, &ty))
        })
        .collect();
    let mut opts = AutoOutletOptions::with_defaults(&ElectricalDefaults::load(&cx.project));
    // Spacing from the plan's code minimums (NEC 210.52).
    crate::editor::code::outlet_options(&crate::editor::code::code_minimums(cx), &mut opts);
    // Counter outlets follow the base cabinets standing against the walls.
    opts.counter_runs = counter_runs(cx.floor());
    let mut placed = auto_place_outlets(cx.floor(), &rooms, &types, &opts);
    if opts.exterior_wp {
        // NEC 210.52(E): weatherproof GFCI receptacles outside, front and back.
        placed.extend(auto_place_exterior_outlets(cx.floor(), &rooms, &opts));
    }
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
    let fresh_exterior = fresh.iter().filter(|d| d.kind.is_weatherproof()).count();
    edit_electrical(cx, "Auto Place Outlets", |layer, _| {
        layer.add_all(fresh);
    });
    let exterior = fresh_exterior;
    cx.status = if exterior > 0 {
        format!("Auto Place Outlets: placed {n} outlets ({exterior} weatherproof outside)")
    } else {
        format!("Auto Place Outlets: placed {n} outlets")
    };
    n
}

/// 3-way and 4-way switches wire to each other as a pair.
fn is_traveler(k: DeviceKind) -> bool {
    matches!(k, DeviceKind::Switch3Way | DeviceKind::Switch4Way)
}

/// Connects `from` (a switch or outlet) to `to` with a dashed arc (CB-67). `to`
/// is a light or an outlet, or another 3-way / 4-way switch when `from` is one
/// (a pair: both then control the same lights).
pub fn connect_devices(cx: &mut EditorContext, from: Id, to: Id) -> Result<(), &'static str> {
    let layer = load_electrical(cx.floor());
    let (Some(a), Some(b)) = (layer.device(from), layer.device(to)) else {
        return Err("That device is gone");
    };
    if !(a.kind.is_switch() || a.kind.is_outlet()) {
        return Err("Start the connection at a switch or an outlet");
    }
    if from == to {
        return Err("Click the light or outlet the switch controls");
    }
    if b.kind.is_switch() && !(is_traveler(a.kind) && is_traveler(b.kind)) {
        return Err("Click the light or outlet the switch controls");
    }
    if layer
        .connections
        .iter()
        .any(|c| (c.from == from && c.to == to) || (c.from == to && c.to == from))
    {
        return Err("Those two are already connected");
    }
    let defaults = ElectricalDefaults::load(&cx.project).connection;
    edit_electrical(cx, "Electrical Connection", |layer, floor| {
        connect_with(layer, from, to, &floor.walls, &defaults);
    });
    Ok(())
}

/// Tolerance when matching a door to the edge of a room, inches.
const DOOR_ON_ROOM: f64 = 12.0;

/// Auto Place Switches (CB-65): for every room, a switch on the room side of
/// each of its doors, 6" past the latch jamb at 48"; a ceiling light at the
/// room's center when the room has none; and the connections. A room with two
/// doors gets a 3-way pair wired together, one with more also 4-way switches
/// in between. Doors that already have a switch are skipped. Returns the
/// number of switches added; the whole run is one undo step.
pub fn auto_place_floor_switches(cx: &mut EditorContext) -> usize {
    cx.refresh();
    let rooms = cx.rooms.clone();
    let floor = cx.floor().clone();
    let existing = load_electrical(&floor);
    let defaults = ElectricalDefaults::load(&cx.project);
    let conn = defaults.connection.clone();
    let mut added: Vec<(usize, Vec<Device>)> = Vec::new(); // room index, new switches
    for (ri, room) in rooms.iter().enumerate() {
        let mut here: Vec<Device> = Vec::new();
        for o in floor
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door)
        {
            let Some(wall) = floor.wall(o.wall_id) else {
                continue;
            };
            let mid = wall.point_at((o.start_offset() + o.end_offset()) * 0.5);
            let n = room.polygon.len();
            let on_edge = (0..n).any(|i| {
                dist_to_segment(mid, room.polygon[i], room.polygon[(i + 1) % n]) <= DOOR_ON_ROOM
            });
            if !on_edge {
                continue;
            }
            let mut d = auto_place_switch(room, o, wall);
            defaults.apply(&mut d);
            let taken = existing
                .devices
                .iter()
                .chain(here.iter())
                .any(|e| e.kind.is_switch() && e.position.dist(d.position) < DUPLICATE_DIST);
            if !taken {
                here.push(d);
            }
        }
        if !here.is_empty() {
            added.push((ri, here));
        }
    }
    if added.is_empty() {
        cx.status = if rooms.is_empty() {
            "Auto Place Switches: no rooms found (close the walls of a room first)".into()
        } else {
            "Auto Place Switches: every door already has its switch".into()
        };
        return 0;
    }
    let mut count = 0;
    edit_electrical(cx, "Auto Place Switches", |layer, floor| {
        for (ri, mut switches) in added {
            let room = &rooms[ri];
            let last = switches.len() - 1;
            if last > 0 {
                for (i, s) in switches.iter_mut().enumerate() {
                    s.kind = if i == 0 || i == last {
                        DeviceKind::Switch3Way
                    } else {
                        DeviceKind::Switch4Way
                    };
                }
            }
            count += switches.len();
            let ids = layer.add_all(switches);
            // The room's lights: those already in it, or a new ceiling light.
            let mut lights: Vec<Id> = layer
                .devices
                .iter()
                .filter(|d| d.kind.is_light() && d.wall_id.is_none())
                .filter(|d| point_in_polygon(d.position, &room.polygon))
                .map(|d| d.id)
                .collect();
            if lights.is_empty() {
                lights.push(layer.add(auto_place_room_light(room)));
            }
            for w in ids.windows(2) {
                connect_with(layer, w[0], w[1], &floor.walls, &conn);
            }
            for l in lights {
                connect_with(layer, ids[0], l, &floor.walls, &conn);
            }
        }
    });
    cx.status = format!("Auto Place Switches: placed {count} switches");
    count
}

// ----- the tool -----

/// Where the diamond-shaped Electrical Connection handle of a selected device
/// sits: a little below its symbol.
fn diamond_at(cx: &EditorContext, d: &Device) -> Point {
    d.position + Point::new(0.0, -(cx.pick_tol() * 2.0).max(8.0))
}

/// How far along a spline its end handle sits from an attached object, so a
/// press on the object itself still starts a new spline, inches.
fn end_inset(cx: &EditorContext) -> f64 {
    (cx.pick_tol() * 1.5).max(6.0)
}

/// The point `d` inches along the polyline `path` (the end when it is shorter).
fn along(path: &[Point], d: f64) -> Point {
    let mut left = d;
    for w in path.windows(2) {
        let len = w[0].dist(w[1]);
        if left <= len && len > 1e-9 {
            return Point::lerp(w[0], w[1], left / len);
        }
        left -= len;
    }
    path.last().copied().unwrap_or(Point::ZERO)
}

/// Where the two end handles of connection `c` are: a free end exactly at its
/// point, an attached end `inset` inches along the curve from its object.
pub fn end_handles(
    layer: &ElectricalLayer,
    c: &plan_electrical::Connection,
    inset: f64,
) -> Option<(Point, Point)> {
    let path = layer.connection_path(c)?;
    let total: f64 = path.windows(2).map(|w| w[0].dist(w[1])).sum();
    let inset = inset.min(total * 0.4);
    let start = if c.from == 0 {
        path[0]
    } else {
        along(&path, inset)
    };
    let rev: Vec<Point> = path.iter().rev().copied().collect();
    let end = if c.to == 0 {
        rev[0]
    } else {
        along(&rev, inset)
    };
    Some((start, end))
}

impl ElectricalTool {
    pub fn variant(&self) -> ElecVariant {
        self.variant
    }

    pub fn selected(&self) -> Option<Id> {
        self.selected
    }

    /// The selected connection spline, if any.
    pub fn selected_connection(&self) -> Option<usize> {
        self.sel_conn
    }

    /// The selected rope light, if any.
    pub fn selected_rope(&self) -> Option<Id> {
        self.sel_rope
    }

    /// Publishes the selection for the Edit commands.
    fn publish(&self) {
        let sel = if let Some(r) = self.sel_rope {
            Sel::Rope(r)
        } else if let Some(c) = self.sel_conn {
            Sel::Connection(c)
        } else if let Some(d) = self.selected {
            Sel::Device(d)
        } else {
            Sel::None
        };
        SELECTION.with(|s| s.set(sel));
    }

    fn select_device(&mut self, id: Option<Id>) {
        self.selected = id;
        self.sel_conn = None;
        self.sel_rope = None;
        self.publish();
    }

    fn select_connection(&mut self, index: Option<usize>) {
        self.sel_conn = index;
        if index.is_some() {
            self.selected = None;
            self.sel_rope = None;
        }
        self.publish();
    }

    fn select_rope(&mut self, id: Option<Id>) {
        self.sel_rope = id;
        if id.is_some() {
            self.selected = None;
            self.sel_conn = None;
        }
        self.publish();
    }

    fn reset_transient(&mut self) {
        self.drag = None;
        self.rope = None;
        self.rope_drag = None;
        self.connect_from = None;
        self.conn = None;
    }

    /// The dialog is open: the canvas is not in use.
    fn dialog_open(&self) -> bool {
        self.dialog.borrow().is_some() || self.rope_dialog.borrow().is_some()
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
        edit_electrical(cx, "Electrical Service Specification", |layer, floor| {
            draft.apply_to_layer(layer, &floor.walls);
        });
        // The default heights belong to the same undo step.
        draft.store_defaults(&mut cx.project);
        ToolResult::committed("Electrical Service Specification")
    }

    /// Applies an OK waiting from the dialog.
    fn flush(&mut self, cx: &mut EditorContext) -> Option<ToolResult> {
        if let Some(draft) = self.rope_applied.borrow_mut().take() {
            if load_electrical(cx.floor()).rope(draft.id).is_none() {
                return Some(ToolResult::consumed());
            }
            edit_electrical(cx, "Rope Light Specification", |layer, _| {
                if let Some(r) = layer.rope_mut(draft.id) {
                    draft.apply(r);
                }
            });
            return Some(ToolResult::committed("Rope Light Specification"));
        }
        let draft = self.applied.borrow_mut().take()?;
        Some(self.apply_draft(cx, &draft))
    }

    fn place_at(
        &mut self,
        cx: &mut EditorContext,
        variant: ElecVariant,
        p: &PointerEvent,
    ) -> ToolResult {
        cx.refresh();
        match placement_full(cx, variant, p.world, p.snapped) {
            Ok(Placed { device, options }) => {
                let label = format!("Place {}", device.kind.name());
                let mut id = 0;
                edit_electrical(cx, &label, |layer, _| {
                    id = layer.add(device);
                    layer.set_options(id, options);
                });
                self.select_device(Some(id));
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
        cx.refresh();
        let points = if a.dist(b) >= MIN_ROPE {
            vec![a, b]
        } else {
            vec![a, a + Point::new(DEFAULT_ROPE, 0.0)]
        };
        let rope = rope_light_path(cx, points);
        let mut id = 0;
        edit_electrical(cx, "Place Rope Light", |layer, _| id = layer.add_rope(rope));
        self.select_rope(Some(id));
        ToolResult::committed("Place Rope Light")
    }

    fn connection_click(&mut self, cx: &mut EditorContext, hit: Option<Id>) -> ToolResult {
        let Some(hit) = hit else {
            cx.status = match self.connect_from {
                Some(_) => "Click the next light or outlet, or press Esc to finish".into(),
                None => "Click a switch to start the connection".into(),
            };
            return ToolResult::consumed();
        };
        let layer = load_electrical(cx.floor());
        let start = |this: &mut Self, cx: &mut EditorContext| {
            this.connect_from = Some(hit);
            this.select_device(Some(hit));
            cx.status = "Now click each light it controls (Esc finishes)".into();
        };
        let Some(from) = self.connect_from else {
            match layer.device(hit) {
                Some(d) if d.kind.is_switch() || d.kind.is_outlet() => start(self, cx),
                _ => cx.status = "Start the connection at a switch or an outlet".into(),
            }
            return ToolResult::consumed();
        };
        // A click on another switch starts a new run, except that two 3-way /
        // 4-way switches wire together as a pair.
        let hit_switch = layer.device(hit).is_some_and(|d| d.kind.is_switch());
        let pair = hit_switch
            && layer
                .device(from)
                .zip(layer.device(hit))
                .is_some_and(|(a, b)| is_traveler(a.kind) && is_traveler(b.kind))
            && !layer
                .connections
                .iter()
                .any(|c| (c.from == from && c.to == hit) || (c.from == hit && c.to == from));
        if hit == from || (hit_switch && !pair) {
            start(self, cx);
            return ToolResult::consumed();
        }
        match connect_devices(cx, from, hit) {
            Ok(()) => {
                // The run goes on from the same switch, so every light of the
                // room can be clicked in turn.
                let multi = load_electrical(cx.floor())
                    .device(hit)
                    .is_some_and(|l| l.switched_by.len() >= 2);
                cx.status = if pair {
                    "3-way pair wired; click the light, or press Esc".into()
                } else if multi {
                    "More than one switch controls it: the switches are now 3-way (S3) / 4-way (S4). Click the next light, or press Esc".into()
                } else {
                    "Click the next light or outlet, or press Esc to finish".into()
                };
                ToolResult::committed("Electrical Connection")
            }
            Err(msg) => {
                cx.status = msg.into();
                ToolResult::consumed()
            }
        }
    }

    /// Is connection `index` one whose handles show (every connection with
    /// the Electrical Connection tool, else those of the selection)?
    fn handles_shown(&self, layer: &ElectricalLayer, index: usize) -> bool {
        let Some(c) = layer.connections.get(index) else {
            return false;
        };
        self.variant == ElecVariant::Connection
            || self.sel_conn == Some(index)
            || self.selected.is_some_and(|s| c.from == s || c.to == s)
    }

    /// A press on a handle of a connection: an end of the selected one or a
    /// vertex of any whose handles show. True when a drag started.
    fn start_connection_edit(
        &mut self,
        cx: &EditorContext,
        layer: &ElectricalLayer,
        p: &PointerEvent,
        device_hit: bool,
    ) -> bool {
        let tol = cx.pick_tol().max(BEND_PICK);
        if self.connect_from.is_none() {
            if let Some((c, ci)) = self
                .sel_conn
                .and_then(|i| layer.connections.get(i).map(|c| (c, i)))
            {
                if let Some((a, b)) = end_handles(layer, c, end_inset(cx)) {
                    for (at, end) in [(a, ConnEnd::Start), (b, ConnEnd::End)] {
                        if at.dist(p.world) <= tol {
                            self.conn = Some(ConnDrag::End {
                                index: ci,
                                end,
                                start: p.screen,
                                moved: false,
                            });
                            return true;
                        }
                    }
                }
            }
        }
        if device_hit {
            return false;
        }
        match layer.connection_handle_hit(p.world, tol, false) {
            Some((i, j)) if self.handles_shown(layer, i) => {
                self.select_connection(Some(i));
                self.conn = Some(ConnDrag::Vertex {
                    index: i,
                    handle: j,
                });
                true
            }
            _ => false,
        }
    }

    /// Moves the vertex being dragged; the first move opens the undo step.
    fn drag_vertex(&mut self, cx: &mut EditorContext, index: usize, handle: usize, p: Point) {
        if !self.vertex_moved {
            cx.begin_change("Bend Connection");
            self.vertex_moved = true;
        }
        let mut layer = load_electrical(cx.floor());
        if layer.move_handle(index, handle, p) {
            let fl = cx.floor;
            crate::editor::site_view::save_electrical(&mut cx.project, fl, &layer);
            cx.mark_dirty();
        }
    }

    /// A press on a vertex or a midpoint handle of the selected rope light.
    fn start_rope_edit(
        &mut self,
        cx: &mut EditorContext,
        layer: &ElectricalLayer,
        p: &PointerEvent,
    ) -> bool {
        let Some(id) = self.sel_rope else {
            return false;
        };
        let Some(r) = layer.rope(id) else {
            return false;
        };
        let tol = cx.pick_tol().max(BEND_PICK);
        if let Some(v) = r.vertex_at(p.world, tol) {
            self.rope_drag = Some(RopeDrag {
                id,
                vertex: v,
                moved: false,
            });
            return true;
        }
        if let Some(seg) = r.midpoint_at(p.world, tol) {
            // The midpoint handle adds a vertex and drags it.
            cx.begin_change("Edit Rope Light");
            let mut layer = layer.clone();
            let at = Point::lerp(r.segments()[seg].0, r.segments()[seg].1, 0.5);
            let v = layer.rope_mut(id).map_or(0, |r| r.insert_vertex(seg, at));
            let fl = cx.floor;
            crate::editor::site_view::save_electrical(&mut cx.project, fl, &layer);
            cx.mark_dirty();
            self.rope_drag = Some(RopeDrag {
                id,
                vertex: v,
                moved: true,
            });
            return true;
        }
        false
    }

    fn drag_rope_vertex(&mut self, cx: &mut EditorContext, p: Point) {
        let Some(d) = self.rope_drag.as_mut() else {
            return;
        };
        if !d.moved {
            cx.begin_change("Edit Rope Light");
            d.moved = true;
        }
        let (id, vertex) = (d.id, d.vertex);
        let mut layer = load_electrical(cx.floor());
        if layer.rope_mut(id).is_some_and(|r| r.move_vertex(vertex, p)) {
            let fl = cx.floor;
            crate::editor::site_view::save_electrical(&mut cx.project, fl, &layer);
            cx.mark_dirty();
        }
    }

    /// The end of a press-drag-release on the Electrical Connection tool.
    fn finish_connection_drag(
        &mut self,
        cx: &mut EditorContext,
        p: &PointerEvent,
        drag: ConnDrag,
    ) -> ToolResult {
        let layer = load_electrical(cx.floor());
        let target = device_at(&layer, p.world, cx.pick_tol());
        let defaults = ElectricalDefaults::load(&cx.project);
        let end_at = p.snapped;
        match drag {
            ConnDrag::Vertex { .. } => {
                if self.vertex_moved {
                    self.vertex_moved = false;
                    cx.mark_dirty();
                    ToolResult::committed("Bend Connection")
                } else {
                    ToolResult::consumed()
                }
            }
            ConnDrag::FromDevice {
                id, moved, diamond, ..
            } => {
                if !moved {
                    return if diamond {
                        ToolResult::consumed()
                    } else {
                        self.connection_click(cx, Some(id))
                    };
                }
                let Some(from_pos) = layer.device(id).map(|d| d.position) else {
                    return ToolResult::consumed();
                };
                let mut made: Option<usize> = None;
                match target {
                    Some(t) if t != id => {
                        let mut probe = layer.clone();
                        let walls = cx.floor().walls.clone();
                        if connect_drawn(&mut probe, id, t, &walls, &defaults.connection).is_none()
                        {
                            cx.status = "Those two are already connected".into();
                            return ToolResult::consumed();
                        }
                        edit_electrical(cx, "Electrical Connection", |layer, floor| {
                            made = connect_drawn(layer, id, t, &floor.walls, &defaults.connection);
                        });
                    }
                    Some(_) => return ToolResult::consumed(),
                    None => {
                        edit_electrical(cx, "Electrical Connection", |layer, _| {
                            let i =
                                layer.add_free_connection(from_pos, end_at, &defaults.connection);
                            layer.attach_end(i, ConnEnd::Start, id);
                            made = Some(i);
                        });
                    }
                }
                self.connect_from = None;
                self.selected = None;
                self.select_connection(made);
                ToolResult::committed("Electrical Connection")
            }
            ConnDrag::FromPoint { at, moved, .. } => {
                if !moved {
                    return self.connection_click(cx, None);
                }
                if at.dist(end_at) < 1.0 {
                    return ToolResult::consumed();
                }
                let mut made = None;
                edit_electrical(cx, "Electrical Connection", |layer, _| {
                    let i = layer.add_free_connection(at, end_at, &defaults.connection);
                    if let Some(t) = target {
                        layer.attach_end(i, ConnEnd::End, t);
                    }
                    made = Some(i);
                });
                self.select_connection(made);
                ToolResult::committed("Electrical Connection")
            }
            ConnDrag::End {
                index, end, moved, ..
            } => {
                if !moved {
                    return ToolResult::consumed();
                }
                // Dropped on a device: attach; anywhere else: a free end.
                if let Some(t) = target {
                    let mut probe = layer.clone();
                    if !probe.attach_end(index, end, t) {
                        cx.status = "That end cannot join that device".into();
                        return ToolResult::consumed();
                    }
                    edit_electrical(cx, "Edit Connection", |layer, _| {
                        layer.attach_end(index, end, t);
                    });
                } else {
                    edit_electrical(cx, "Edit Connection", |layer, _| {
                        layer.detach_end(index, end, end_at);
                    });
                }
                ToolResult::committed("Edit Connection")
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
        if let Some(id) = self.sel_rope.take() {
            self.publish();
            if load_electrical(cx.floor()).rope(id).is_none() {
                return ToolResult::consumed();
            }
            edit_electrical(cx, "Delete Rope Light", |layer, _| {
                layer.remove_rope(id);
            });
            return ToolResult::committed("Delete Rope Light");
        }
        if let Some(i) = self.sel_conn.take() {
            self.publish();
            if load_electrical(cx.floor()).connections.get(i).is_none() {
                return ToolResult::consumed();
            }
            edit_electrical(cx, "Delete Connection", |layer, _| {
                layer.remove_connection(i);
            });
            return ToolResult::committed("Delete Connection");
        }
        let Some(id) = self.selected.take() else {
            return ToolResult::ignored();
        };
        self.publish();
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
            (None, Some(DeviceKind::RopeLight { .. })) => None,
            (None, Some(_)) => {
                let defaults = ElectricalDefaults::load(&cx.project);
                // The height the next click would use: the hovered place's.
                let h = self
                    .hover
                    .and_then(|h| placement_full(cx, self.variant, h, h).ok())
                    .map(|p| p.device.height)
                    .or_else(|| {
                        effective_kind(&defaults, self.variant).map(|k| defaults.height(k))
                    });
                h.map(height_text)
            }
            _ => None,
        };
    }

    /// The Edit commands that apply to the selection of this tool.
    fn edit_actions(&self, cx: &EditorContext) -> Vec<EditAction> {
        let custom = |id: &'static str, label: &'static str| {
            EditAction::new(EditActionKind::Custom {
                id,
                label,
                icon: "",
            })
        };
        let mut v = Vec::new();
        match self.publish_value() {
            Sel::Device(id) => {
                if let Some(d) = load_electrical(cx.floor()).device(id) {
                    match d.kind {
                        DeviceKind::Outlet110 => {
                            v.push(custom(cmd::TO_GFCI, "Change to GFCI Outlet"))
                        }
                        DeviceKind::Gfci => v.push(custom(cmd::TO_110, "Change to 110V Outlet")),
                        _ => {}
                    }
                    v.push(custom(cmd::SET_DEFAULT, "Set as Default"));
                }
            }
            Sel::Connection(_) => {
                v.push(custom(cmd::RESET_CURVATURE, "Reset Curvature"));
                v.push(custom(cmd::SET_DEFAULT, "Set as Default"));
            }
            Sel::Rope(_) => v.push(custom(cmd::SET_DEFAULT, "Set as Default")),
            Sel::None => {}
        }
        v
    }

    fn publish_value(&self) -> Sel {
        if let Some(r) = self.sel_rope {
            Sel::Rope(r)
        } else if let Some(c) = self.sel_conn {
            Sel::Connection(c)
        } else if let Some(d) = self.selected {
            Sel::Device(d)
        } else {
            Sel::None
        }
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
            ElecVariant::Connection => "Electrical Connection: click a switch, then each light it \
                 controls, or drag from one object to the next (drag a handle to reshape a \
                 spline, double-click it to add a vertex)"
                .into(),
            ElecVariant::AutoSwitches => {
                "Auto Place Switches: click to place a switch at every door, with a light".into()
            }
            ElecVariant::AutoOutlets => {
                "Auto Place Outlets: click to place outlets in every room of this floor".into()
            }
            ElecVariant::RopeLight => "Rope Light: click and drag to draw its path".into(),
            v if v.is_contextual() => format!(
                "{}: click a wall, a cabinet side or the room to place it (the type and height follow the place)",
                v.name()
            ),
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
        self.select_device(None);
        *self.dialog.borrow_mut() = None;
        *self.applied.borrow_mut() = None;
        *self.rope_dialog.borrow_mut() = None;
        *self.rope_applied.borrow_mut() = None;
        cx.readout = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        self.hover = Some(p.world);
        match self.conn {
            Some(ConnDrag::Vertex { index, handle }) if p.down => {
                self.drag_vertex(cx, index, handle, p.world);
            }
            Some(
                ConnDrag::FromDevice {
                    ref mut moved,
                    start,
                    ..
                }
                | ConnDrag::FromPoint {
                    ref mut moved,
                    start,
                    ..
                }
                | ConnDrag::End {
                    ref mut moved,
                    start,
                    ..
                },
            ) if p.down && !*moved && (p.screen - start).length() >= DRAG_PX => {
                *moved = true;
            }
            _ => {}
        }
        if self.rope_drag.is_some() && p.down {
            self.drag_rope_vertex(cx, p.snapped);
        }
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
            *end = p.snapped;
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
        // The diamond handle of the selected device starts a connection.
        if let Some(d) = self.selected_device(cx) {
            if diamond_at(cx, &d).dist(p.world) <= cx.pick_tol().max(BEND_PICK) {
                self.vertex_moved = false;
                self.conn = Some(ConnDrag::FromDevice {
                    id: d.id,
                    start: p.screen,
                    moved: false,
                    diamond: true,
                });
                return ToolResult::consumed();
            }
        }
        self.vertex_moved = false;
        if self.start_connection_edit(cx, &layer, &p, hit.is_some()) {
            return ToolResult::consumed();
        }
        if hit.is_none() && self.start_rope_edit(cx, &layer, &p) {
            return ToolResult::consumed();
        }
        match self.variant {
            ElecVariant::Connection => {
                if let Some(id) = hit {
                    self.conn = Some(ConnDrag::FromDevice {
                        id,
                        start: p.screen,
                        moved: false,
                        diamond: false,
                    });
                    return ToolResult::consumed();
                }
                let tol = cx.pick_tol().max(BEND_PICK);
                if let Some(i) = layer.connection_at(p.world, tol) {
                    self.select_connection(Some(i));
                    return ToolResult::consumed();
                }
                self.conn = Some(ConnDrag::FromPoint {
                    at: p.snapped,
                    start: p.screen,
                    moved: false,
                });
                ToolResult::consumed()
            }
            ElecVariant::AutoOutlets => {
                if auto_place_floor_outlets(cx) > 0 {
                    ToolResult::committed("Auto Place Outlets")
                } else {
                    ToolResult::consumed()
                }
            }
            ElecVariant::AutoSwitches => {
                if auto_place_floor_switches(cx) > 0 {
                    ToolResult::committed("Auto Place Switches")
                } else {
                    ToolResult::consumed()
                }
            }
            _ if hit.is_some() => {
                self.select_device(hit);
                self.drag = hit.map(|id| Drag {
                    id,
                    start: p.screen,
                    moved: false,
                });
                ToolResult::consumed()
            }
            v => {
                // An existing rope light under the pointer is selected.
                let tol = cx.pick_tol().max(BEND_PICK);
                if let Some(r) = layer.rope_at(p.world, tol) {
                    self.select_rope(Some(r));
                    return ToolResult::consumed();
                }
                self.select_device(None);
                if v == ElecVariant::RopeLight {
                    self.rope = Some((p.snapped, p.snapped));
                    return ToolResult::consumed();
                }
                if v.kind().is_some() {
                    self.place_at(cx, v, &p)
                } else {
                    ToolResult::consumed()
                }
            }
        }
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some((a, _)) = self.rope.take() {
            return self.place_rope(cx, a, p.snapped);
        }
        if let Some(d) = self.rope_drag.take() {
            return if d.moved {
                cx.mark_dirty();
                ToolResult::committed("Edit Rope Light")
            } else {
                ToolResult::consumed()
            };
        }
        if let Some(drag) = self.conn.take() {
            return self.finish_connection_drag(cx, &p, drag);
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
        let tol = cx.pick_tol().max(BEND_PICK);

        if let Some(id) = device_at(&layer, p.world, cx.pick_tol()) {
            self.drag = None;
            self.rope = None;
            self.conn = None;
            self.select_device(Some(id));
            if let Some(d) = layer.device(id) {
                let defaults = ElectricalDefaults::load(&cx.project);
                *self.dialog.borrow_mut() =
                    Some(ElectricalDialog::for_device(d, &layer).with_defaults(&defaults));
            }
            return ToolResult::consumed();
        }
        // A vertex of a rope light: remove it; its path: the specification.
        if let Some(id) = self.sel_rope {
            if let Some(v) = layer.rope(id).and_then(|r| r.vertex_at(p.world, tol)) {
                self.rope_drag = None;
                let mut gone = false;
                edit_electrical(cx, "Edit Rope Light", |layer, _| {
                    gone = layer.rope_mut(id).is_some_and(|r| r.remove_vertex(v));
                });
                if gone {
                    return ToolResult::committed("Edit Rope Light");
                }
                cx.cancel_change();
            }
        }
        // A vertex of a connection: remove it; its spline: add one.
        if let Some((i, j)) = layer.connection_handle_hit(p.world, tol, false) {
            // The middle handle of a plain arc is not a vertex: a double-click
            // there adds one (below).
            if self.handles_shown(&layer, i) && !layer.connections[i].is_arc() {
                self.conn = None;
                self.vertex_moved = false;
                let mut gone = false;
                edit_electrical(cx, "Edit Connection", |layer, _| {
                    gone = layer.remove_vertex(i, j);
                });
                if gone {
                    self.select_connection(Some(i));
                    return ToolResult::committed("Edit Connection");
                }
                cx.cancel_change();
                return ToolResult::consumed();
            }
        }
        if let Some(i) = layer.connection_at(p.world, tol) {
            self.conn = None;
            let mut added = None;
            edit_electrical(cx, "Edit Connection", |layer, _| {
                added = layer.insert_vertex(i, p.world);
            });
            if added.is_some() {
                self.select_connection(Some(i));
                return ToolResult::committed("Edit Connection");
            }
            cx.cancel_change();
            return ToolResult::consumed();
        }
        if let Some(id) = layer.rope_at(p.world, tol) {
            self.drag = None;
            self.rope = None;
            self.rope_drag = None;
            self.select_rope(Some(id));
            if let Some(r) = layer.rope(id) {
                *self.rope_dialog.borrow_mut() = Some(RopeLightDialog::for_rope(r));
            }
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if let Some(r) = self.flush(cx) {
            return r;
        }
        if self.dialog_open() {
            return ToolResult::consumed();
        }
        if k.is(Key::Escape) {
            if self.connect_from.is_some()
                || self.rope.is_some()
                || self.drag.is_some()
                || self.conn.is_some()
                || self.rope_drag.is_some()
            {
                self.reset_transient();
                return ToolResult::consumed();
            }
            if self.selected.is_some() || self.sel_conn.is_some() || self.sel_rope.is_some() {
                self.select_device(None);
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

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        self.edit_actions(cx)
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        let ghost = pal.ghost_stroke;
        let layer = load_electrical(cx.floor());
        // Rubber band of a rope light.
        if let Some((a, b)) = self.rope {
            if a.dist(b) > 1.0 {
                let spec = ElectricalDefaults::load(&cx.project).rope;
                let ghost_rope = RopeLightPath::new(vec![a, b], spec);
                draw_rope(painter, cam, &ghost_rope, ghost, 1.5);
            }
        } else if let (Some(h), Some(kind)) = (self.hover, self.variant.kind()) {
            // The ghost of the device a click would place.
            if self.drag.is_none() && device_at(&layer, h, cx.pick_tol()).is_none() {
                if let DeviceKind::RopeLight { .. } = kind {
                    // A rope light is drawn by a drag: nothing to preview.
                } else if let Ok(p) = placement_full(cx, self.variant, h, h) {
                    draw_symbol(painter, cam, &p.device.symbol_world(), ghost, 1.5);
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
            // The diamond-shaped Electrical Connection handle below the symbol.
            let c = cam.world_to_screen(diamond_at(cx, d));
            let r = 5.0_f32;
            painter.add(Shape::convex_polygon(
                vec![
                    egui::pos2(c.x, c.y - r),
                    egui::pos2(c.x + r, c.y),
                    egui::pos2(c.x, c.y + r),
                    egui::pos2(c.x - r, c.y),
                ],
                pal.selection,
                egui::Stroke::new(1.0_f32, ghost),
            ));
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
        // A spline being drawn by a drag, or an end being moved.
        if let (Some(h), Some(conn)) = (self.hover, self.conn) {
            let from = match conn {
                ConnDrag::FromDevice {
                    id, moved: true, ..
                } => layer.device(id).map(|d| d.position),
                ConnDrag::FromPoint {
                    at, moved: true, ..
                } => Some(at),
                ConnDrag::End {
                    index,
                    end,
                    moved: true,
                    ..
                } => layer.connections.get(index).and_then(|c| {
                    layer.connection_ends(c).map(|(a, b)| match end {
                        ConnEnd::Start => b,
                        ConnEnd::End => a,
                    })
                }),
                _ => None,
            };
            if let Some(a) = from {
                painter.extend(Shape::dashed_line(
                    &[cam.world_to_screen(a), cam.world_to_screen(h)],
                    egui::Stroke::new(1.5_f32, ghost),
                    5.0,
                    3.0,
                ));
            }
            if let Some(d) = device_at(&layer, h, cx.pick_tol()).and_then(|id| layer.device(id)) {
                ring(d, pal.hover);
            }
        }
        if let Some(h) = self
            .hover
            .filter(|_| self.variant == ElecVariant::Connection)
        {
            if let Some(d) = device_at(&layer, h, cx.pick_tol()).and_then(|id| layer.device(id)) {
                ring(d, pal.hover);
            }
        }
        for (i, c) in layer.connections.iter().enumerate() {
            if !(self.sel_conn == Some(i)
                || self.selected.is_some_and(|s| c.from == s || c.to == s))
            {
                continue;
            }
            if let Some(pts) = layer.connection_path(c) {
                let pts: Vec<Pos2> = pts.iter().map(|q| cam.world_to_screen(*q)).collect();
                if pts.len() >= 2 {
                    painter.add(Shape::line(pts, egui::Stroke::new(2.0_f32, pal.selection)));
                }
            }
        }
        // The handles of every connection that can be edited: a square at
        // each vertex, a circle at the ends of the selected one.
        for (i, c) in layer.connections.iter().enumerate() {
            if !self.handles_shown(&layer, i) {
                continue;
            }
            let Some(h) = layer.connection_handles(c) else {
                continue;
            };
            let last = h.len() - 1;
            if self.sel_conn == Some(i) {
                if let Some((a, b)) = end_handles(&layer, c, end_inset(cx)) {
                    for q in [a, b] {
                        painter.circle(
                            cam.world_to_screen(q),
                            4.5,
                            pal.selection,
                            egui::Stroke::new(1.0_f32, ghost),
                        );
                    }
                }
            }
            for (j, q) in h.iter().enumerate() {
                let at = cam.world_to_screen(*q);
                if j == 0 || j == last {
                    continue;
                }
                let r = egui::Rect::from_center_size(at, egui::vec2(7.0, 7.0));
                painter.rect_filled(r, 0.0, pal.selection);
                painter.rect_stroke(
                    r,
                    0.0,
                    egui::Stroke::new(1.0_f32, ghost),
                    egui::StrokeKind::Middle,
                );
            }
        }
        // The selected rope light: a handle on every vertex, a small diamond
        // at every segment midpoint.
        if let Some(r) = self.sel_rope.and_then(|id| layer.rope(id)) {
            draw_rope(painter, cam, r, pal.selection, 3.0);
            for q in &r.points {
                let r = egui::Rect::from_center_size(cam.world_to_screen(*q), egui::vec2(7.0, 7.0));
                painter.rect_filled(r, 0.0, pal.selection);
                painter.rect_stroke(
                    r,
                    0.0,
                    egui::Stroke::new(1.0_f32, ghost),
                    egui::StrokeKind::Middle,
                );
            }
            for (a, b) in r.segments() {
                let c = cam.world_to_screen(Point::lerp(a, b, 0.5));
                painter.circle_stroke(c, 3.0, egui::Stroke::new(1.0_f32, pal.selection));
            }
        }
        // The Electrical Service Specification and the Rope Light Specification.
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
        let mut slot = self.rope_dialog.borrow_mut();
        let outcome = slot.as_mut().map(|d| d.show(painter.ctx()));
        match outcome {
            Some(Outcome::Ok) => {
                if let Some(d) = slot.take() {
                    *self.rope_applied.borrow_mut() = Some(d.draft().clone());
                }
            }
            Some(Outcome::Cancel) => {
                slot.take();
            }
            _ => {}
        }
    }
}

// ----- drawing the layer's connections and rope lights -----

fn line_dashes(style: plan_core::LineStyle) -> Option<(f32, f32)> {
    match style {
        plan_core::LineStyle::Solid => None,
        plan_core::LineStyle::Dashed => Some((5.0, 3.0)),
        plan_core::LineStyle::Dotted => Some((1.0, 3.0)),
        plan_core::LineStyle::DashDot => Some((8.0, 3.0)),
    }
}

/// An arrow head at `tip` pointing along `dir` (screen space).
fn arrow_head(painter: &egui::Painter, tip: Pos2, dir: egui::Vec2, stroke: egui::Stroke) {
    if dir.length() < 1e-3 {
        return;
    }
    let d = dir.normalized();
    let n = egui::vec2(-d.y, d.x);
    let base = tip - d * 9.0;
    painter.add(Shape::convex_polygon(
        vec![tip, base + n * 3.5, base - n * 3.5],
        stroke.color,
        stroke,
    ));
}

/// Draws the connection splines of `layer` in their line styles with arrows.
pub fn draw_connections(
    painter: &egui::Painter,
    cam: &Camera,
    layer: &ElectricalLayer,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.0_f32, color);
    for c in &layer.connections {
        let Some(path) = layer.connection_path(c) else {
            continue;
        };
        let pts: Vec<Pos2> = path.iter().map(|q| cam.world_to_screen(*q)).collect();
        if pts.len() < 2 {
            continue;
        }
        match line_dashes(c.style()) {
            Some((d, g)) => {
                painter.extend(Shape::dashed_line(&pts, stroke, d, g));
            }
            None => {
                painter.add(Shape::line(pts.clone(), stroke));
            }
        }
        let n = pts.len();
        if matches!(
            c.arrow,
            plan_electrical::Arrow::End | plan_electrical::Arrow::Both
        ) {
            arrow_head(painter, pts[n - 1], pts[n - 1] - pts[n - 2], stroke);
        }
        if matches!(
            c.arrow,
            plan_electrical::Arrow::Start | plan_electrical::Arrow::Both
        ) {
            arrow_head(painter, pts[0], pts[0] - pts[1], stroke);
        }
    }
}

/// Draws one rope light: its path and, when its spec asks, the light sources.
pub fn draw_rope(
    painter: &egui::Painter,
    cam: &Camera,
    rope: &RopeLightPath,
    color: egui::Color32,
    width: f32,
) {
    let mut pts: Vec<Pos2> = rope
        .points
        .iter()
        .map(|q| cam.world_to_screen(*q))
        .collect();
    if rope.closed && pts.len() > 2 {
        pts.push(pts[0]);
    }
    if pts.len() >= 2 {
        painter.add(Shape::line(pts, egui::Stroke::new(width, color)));
    }
    if rope.spec.show_lights {
        let r = ((rope.spec.light_size * 0.5 * cam.px_per_in) as f32).max(1.5);
        for l in rope.lights() {
            painter.circle_filled(cam.world_to_screen(l), r, color);
        }
    }
}

/// The wall of `floor` carrying `d`, if any.
pub fn host_wall<'a>(floor: &'a Floor, d: &Device) -> Option<&'a plan_core::Wall> {
    d.wall_id.and_then(|w| floor.wall(w))
}

// ----- Edit commands: Set as Default, Reset Curvature, Change to GFCI / 110V -----

/// The ids of the Electrical tools' edit commands.
pub mod cmd {
    pub const SET_DEFAULT: &str = "electrical.set_default";
    pub const RESET_CURVATURE: &str = "electrical.reset_curvature";
    pub const TO_GFCI: &str = "electrical.to_gfci";
    pub const TO_110: &str = "electrical.to_110";
}

/// Is `id` one of this module's Edit commands?
pub fn is_command(id: &str) -> bool {
    matches!(
        id,
        cmd::SET_DEFAULT | cmd::RESET_CURVATURE | cmd::TO_GFCI | cmd::TO_110
    )
}

/// What the Edit commands act on: the electrical tool's selection, else the
/// device the Select tool selected.
fn command_target(cx: &EditorContext) -> Sel {
    match SELECTION.with(Cell::get) {
        Sel::None => match cx.selection.single() {
            Some(ObjectRef::Device(id)) => Sel::Device(id),
            _ => Sel::None,
        },
        s => s,
    }
}

/// The height context a placed device is in: above a base cabinet, on a
/// cabinet side or plain wall (the rules the tools place by).
pub fn device_context(cx: &EditorContext, d: &Device) -> HeightContext {
    let layer = load_electrical(cx.floor());
    let cabs = crate::editor::placed::load_cabinets(cx.floor());
    let opts = layer.options_of(d.id);
    if opts.mount == Mount::CabinetSide {
        if let Some(c) = opts.host.and_then(|h| cabs.iter().find(|c| c.id == h)) {
            return HeightContext::CabinetSide {
                bottom: c.elevation,
                top: c.elevation + c.height,
            };
        }
    }
    if let Some(w) = d.wall_id.and_then(|w| cx.floor().wall(w)) {
        let n = Point::new(d.angle.cos(), d.angle.sin());
        let probe = d.position + n * COUNTER_PROBE;
        let _ = w;
        if let Some(c) = cabs
            .iter()
            .find(|c| is_base(c) && c.elevation < 6.0 && point_in_polygon(probe, &c.footprint()))
        {
            if !has_sink(c) {
                return HeightContext::AboveCounter {
                    counter_top: c.elevation + c.height,
                };
            }
        }
    }
    HeightContext::Wall
}

/// Set as Default for a device: its type and height become the defaults of
/// its tool and height group in the plan's Electrical Defaults. One undo step.
pub fn set_device_as_default(cx: &mut EditorContext, id: Id) -> bool {
    let layer = load_electrical(cx.floor());
    let Some(d) = layer.device(id).cloned() else {
        return false;
    };
    let ctx = device_context(cx, &d);
    let mut defaults = ElectricalDefaults::load(&cx.project);
    if !defaults.set_from_device(&d, ctx) {
        cx.status = "The electrical defaults already match this object".into();
        return false;
    }
    cx.begin_change("Set as Default");
    defaults.store(&mut cx.project);
    cx.mark_dirty();
    cx.status = format!("Set the electrical defaults from the {}", d.kind.name());
    true
}

/// Set as Default for a connection spline: its curvature ratio, line style,
/// arrow and label become the Electrical Connection Defaults.
pub fn set_connection_as_default(cx: &mut EditorContext, index: usize) -> bool {
    let layer = load_electrical(cx.floor());
    let Some(c) = layer.connections.get(index) else {
        return false;
    };
    let mut defaults = ElectricalDefaults::load(&cx.project);
    let before = defaults.clone();
    if let Some(r) = layer.curvature_ratio(c) {
        if c.is_arc() {
            defaults.connection.curvature_ratio = r.clamp(0.0, 0.5);
        }
    }
    defaults.connection.line_style = c.style();
    defaults.connection.arrow = c.arrow;
    defaults.connection.label = c.label.clone();
    if defaults == before {
        cx.status = "The Electrical Connection Defaults already match this spline".into();
        return false;
    }
    cx.begin_change("Set as Default");
    defaults.store(&mut cx.project);
    cx.mark_dirty();
    cx.status = "Set the Electrical Connection Defaults from the spline".into();
    true
}

/// Set as Default for a rope light: its specification becomes the Rope
/// Light Defaults.
pub fn set_rope_as_default(cx: &mut EditorContext, id: Id) -> bool {
    let layer = load_electrical(cx.floor());
    let Some(r) = layer.rope(id) else {
        return false;
    };
    let mut defaults = ElectricalDefaults::load(&cx.project);
    if defaults.rope == r.spec {
        cx.status = "The Rope Light Defaults already match this rope light".into();
        return false;
    }
    defaults.rope = r.spec.clone();
    cx.begin_change("Set as Default");
    defaults.store(&mut cx.project);
    cx.mark_dirty();
    cx.status = "Set the Rope Light Defaults from the rope light".into();
    true
}

/// Runs an Edit command of the Electrical Tools on the selection (one undo
/// step). False when `id` is not one of them.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    if !is_command(id) {
        return false;
    }
    match (id, command_target(cx)) {
        (cmd::SET_DEFAULT, Sel::Device(d)) => {
            set_device_as_default(cx, d);
        }
        (cmd::SET_DEFAULT, Sel::Connection(i)) => {
            set_connection_as_default(cx, i);
        }
        (cmd::SET_DEFAULT, Sel::Rope(r)) => {
            set_rope_as_default(cx, r);
        }
        (cmd::RESET_CURVATURE, Sel::Connection(i)) => {
            let ratio = ElectricalDefaults::load(&cx.project)
                .connection
                .curvature_ratio;
            edit_electrical(cx, "Reset Curvature", |layer, _| {
                layer.reset_curvature(i, ratio);
            });
        }
        (cmd::TO_GFCI | cmd::TO_110, Sel::Device(d)) => {
            let to = if id == cmd::TO_GFCI {
                DeviceKind::Gfci
            } else {
                DeviceKind::Outlet110
            };
            let label = if id == cmd::TO_GFCI {
                "Change to GFCI Outlet"
            } else {
                "Change to 110V Outlet"
            };
            edit_electrical(cx, label, |layer, _| {
                if let Some(dev) = layer.device_mut(d) {
                    dev.kind = to;
                }
            });
        }
        _ => {}
    }
    true
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
        let u = t.pointer_up(cx, p);
        if u.commit.is_some() {
            u
        } else {
            r
        }
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
    fn far_from_walls_an_outlet_goes_on_the_floor_and_a_switch_is_refused() {
        let (mut cx, _) = cx_with_room();
        // The middle of a room: a floor outlet (manual p. 693).
        let mut t = tool(ElecVariant::Outlet220);
        click(&mut t, &mut cx, 120.0, 72.0);
        let d = &devices(&cx)[0];
        assert_eq!(
            (d.kind, d.height, d.wall_id),
            (DeviceKind::Outlet220, 0.0, None)
        );
        let mut t = tool(ElecVariant::Outlet110);
        click(&mut t, &mut cx, 60.0, 60.0);
        assert_eq!(devices(&cx)[1].kind, DeviceKind::OutletFloor);
        // A switch needs a wall.
        let (mut cx, _) = cx_with_room();
        let mut t = tool(ElecVariant::Switch);
        click(&mut t, &mut cx, 120.0, 72.0);
        assert!(devices(&cx).is_empty());
        assert!(cx.status.contains("wall"));
        assert!(!cx.can_undo());
        // A tool that is not contextual still needs its wall.
        let mut t = tool(ElecVariant::Thermostat);
        click(&mut t, &mut cx, 120.0, 72.0);
        assert!(devices(&cx).is_empty() && cx.status.contains("wall"));
    }

    #[test]
    fn a_wall_outside_the_house_makes_the_weatherproof_types() {
        let (mut cx, ids) = cx_with_room();
        for id in ids {
            cx.project.floors[0].wall_mut(id).unwrap().kind = WallKind::Exterior;
        }
        cx.refresh();
        // Inside the south wall: plain; outside it: weatherproof.
        click(&mut tool(ElecVariant::Outlet110), &mut cx, 100.0, 5.0);
        click(&mut tool(ElecVariant::Outlet110), &mut cx, 100.0, -5.0);
        click(&mut tool(ElecVariant::Gfci), &mut cx, 140.0, -5.0);
        click(&mut tool(ElecVariant::Outlet220), &mut cx, 180.0, -5.0);
        click(&mut tool(ElecVariant::Switch), &mut cx, 60.0, -5.0);
        click(&mut tool(ElecVariant::WallLight), &mut cx, 220.0, -5.0);
        click(&mut tool(ElecVariant::WallLight), &mut cx, 220.0, 5.0);
        let kinds: Vec<_> = devices(&cx).iter().map(|d| d.kind).collect();
        assert_eq!(
            kinds,
            [
                DeviceKind::Outlet110,
                DeviceKind::OutletWp,
                DeviceKind::OutletWp,
                DeviceKind::Outlet220,
                DeviceKind::SwitchWp,
                DeviceKind::WallLightExterior,
                DeviceKind::WallSconce
            ]
        );
        // The explicit WP tool and the interior/exterior wall light slots.
        let mut defaults = ElectricalDefaults::default();
        defaults.set_object(
            "Wall Light (Exterior)",
            DeviceKind::WallLightExterior,
            DeviceKind::WallLightExterior,
        );
        assert!(defaults.objects.is_empty());
    }

    #[test]
    fn variants_map_back_to_their_tools() {
        assert_eq!(ElecVariant::for_kind(DeviceKind::Gfci), ElecVariant::Gfci);
        assert_eq!(
            ElecVariant::for_kind(DeviceKind::CeilingLight),
            ElecVariant::Light
        );
        assert_eq!(
            ElecVariant::for_kind(DeviceKind::WallSconce),
            ElecVariant::WallLight
        );
        assert_eq!(
            ElecVariant::for_kind(DeviceKind::Doorbell),
            ElecVariant::Doorbell
        );
        assert!(ElecVariant::Light.is_contextual() && !ElecVariant::Doorbell.is_contextual());
        assert_eq!(
            ElecVariant::Connection.defaults_tab(),
            DefaultsTab::Connection
        );
        assert_eq!(
            ElecVariant::RopeLight.defaults_tab(),
            DefaultsTab::RopeLight
        );
        assert_eq!(ElecVariant::Gfci.defaults_tab(), DefaultsTab::Electrical);
        // The Default Library Object replaces what the tool places.
        let mut d = ElectricalDefaults::default();
        assert_eq!(
            effective_kind(&d, ElecVariant::Light),
            Some(DeviceKind::CeilingLight)
        );
        d.set_object("Light", DeviceKind::CeilingLight, DeviceKind::PendantLight);
        assert_eq!(
            effective_kind(&d, ElecVariant::Light),
            Some(DeviceKind::PendantLight)
        );
        assert_eq!(effective_kind(&d, ElecVariant::Connection), None);
        // No tray ceilings, no tray rope lights.
        let (cx, _) = cx_with_room();
        assert!(tray_rope_lights(&cx).is_empty());
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
    fn rope_light_is_a_path_that_takes_its_length_from_the_drag() {
        let (mut cx, _) = cx_with_room();
        let mut t = tool(ElecVariant::RopeLight);
        let a = PointerEvent::at(&cx, Point::new(20.0, 100.0));
        t.pointer_down(&mut cx, a.with_down(true));
        let b = PointerEvent::at(&cx, Point::new(140.0, 100.0));
        t.pointer_move(&mut cx, b.with_down(true));
        assert!(cx.readout.as_deref().unwrap().starts_with("Length:"));
        let r = t.pointer_up(&mut cx, b);
        assert_eq!(r.commit.as_deref(), Some("Place Rope Light"));
        assert!(
            devices(&cx).is_empty(),
            "a rope light is a path, not a device"
        );
        let layer = load_electrical(cx.floor());
        assert_eq!(layer.ropes.len(), 1);
        let rope = &layer.ropes[0];
        assert_eq!(
            rope.points,
            vec![Point::new(20.0, 100.0), Point::new(140.0, 100.0)]
        );
        assert_eq!(rope.length(), 120.0);
        assert_eq!(rope.lights().len(), 20, "every 6\", centered");
        assert_eq!(t.selected_rope(), Some(rope.id));
        // A plain click places the default 8' strip along +X.
        click(&mut t, &mut cx, 40.0, 40.0);
        let rope = &load_electrical(cx.floor()).ropes[1];
        assert_eq!(rope.length(), DEFAULT_ROPE);
        assert_eq!(cx.undo().as_deref(), Some("Place Rope Light"));
        assert_eq!(cx.undo().as_deref(), Some("Place Rope Light"));
        assert!(load_electrical(cx.floor()).ropes.is_empty());
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

    // ----- Chief's full flyout, multi-light connections, 3-way pairs, bending -----

    #[test]
    fn every_flyout_variant_places_its_kind_at_its_default_height() {
        let mut placed = 0;
        for v in ElecVariant::ALL {
            let Some(kind) = v.kind() else { continue };
            if matches!(kind, DeviceKind::RopeLight { .. }) {
                continue; // a press starts the drag; covered by its own test
            }
            // A fresh room per kind: wall devices on the south wall, the others
            // away from the room's center.
            let (mut cx, _) = cx_with_room();
            let mut t = tool(v);
            let (x, y) = if kind.is_wall_mounted() {
                (60.0, 0.0)
            } else {
                (40.0, 30.0)
            };
            let r = click(&mut t, &mut cx, x, y);
            assert!(r.commit.is_some(), "{v:?} did not place: {}", cx.status);
            placed += 1;
            let ds = devices(&cx);
            assert_eq!(ds.len(), 1, "{v:?}");
            let d = &ds[0];
            assert_eq!(
                std::mem::discriminant(&d.kind),
                std::mem::discriminant(&kind)
            );
            assert_eq!(
                d.height,
                ElectricalDefaults::default().height(kind),
                "{v:?}: the Electrical Defaults heights"
            );
            assert_eq!(d.wall_id.is_some(), kind.is_wall_mounted(), "{v:?}");
            assert_eq!(
                cx.undo().as_deref(),
                Some(format!("Place {}", kind.name()).as_str())
            );
        }
        assert_eq!(placed, 24);
        // The flyout names are unique and Chief-like.
        let names: Vec<_> = ElecVariant::ALL.iter().map(|v| v.name()).collect();
        for n in &names {
            assert_eq!(names.iter().filter(|m| *m == n).count(), 1, "{n}");
        }
        assert!(names.contains(&"Quad Outlet") && names.contains(&"Auto Place Switches"));
    }

    #[test]
    fn a_connection_run_wires_several_lights_to_one_switch() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch), &mut cx, 30.0, 0.0);
        click(&mut tool(ElecVariant::Light), &mut cx, 60.0, 40.0);
        click(&mut tool(ElecVariant::RecessedLight), &mut cx, 160.0, 100.0);
        let mut t = tool(ElecVariant::Connection);
        click(&mut t, &mut cx, 30.0, 2.25);
        click(&mut t, &mut cx, 60.0, 40.0);
        assert!(
            t.connect_from.is_some(),
            "the run continues from the switch"
        );
        click(&mut t, &mut cx, 160.0, 100.0);
        let layer = load_electrical(cx.floor());
        assert_eq!(layer.connections.len(), 2);
        let sw = layer
            .devices
            .iter()
            .find(|d| d.kind.is_switch())
            .unwrap()
            .id;
        assert!(layer
            .devices
            .iter()
            .filter(|d| d.kind.is_light())
            .all(|d| d.switched_by == vec![sw]));
        // Esc ends the run.
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(t.connect_from.is_none());
        // Both arcs undo one at a time.
        assert_eq!(cx.undo().as_deref(), Some("Electrical Connection"));
        assert_eq!(load_electrical(cx.floor()).connections.len(), 1);
    }

    #[test]
    fn two_three_way_switches_wire_as_a_pair() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch3Way), &mut cx, 30.0, 0.0);
        click(&mut tool(ElecVariant::Switch3Way), &mut cx, 200.0, 0.0);
        click(&mut tool(ElecVariant::Light), &mut cx, 120.0, 72.0);
        let mut t = tool(ElecVariant::Connection);
        click(&mut t, &mut cx, 30.0, 2.25);
        click(&mut t, &mut cx, 120.0, 72.0);
        // Clicking the other 3-way switch wires the pair.
        let r = click(&mut t, &mut cx, 200.0, 2.25);
        assert_eq!(r.commit.as_deref(), Some("Electrical Connection"));
        let layer = load_electrical(cx.floor());
        let ids: Vec<Id> = layer
            .devices
            .iter()
            .filter(|d| d.kind.is_switch())
            .map(|d| d.id)
            .collect();
        assert_eq!(ids.len(), 2);
        let layer = load_electrical(cx.floor());
        assert_eq!(
            layer.connections.len(),
            2,
            "one arc to the light, one traveler"
        );
        let light = layer.devices.iter().find(|d| d.kind.is_light()).unwrap();
        let mut by = light.switched_by.clone();
        by.sort();
        let mut want = ids.clone();
        want.sort();
        assert_eq!(by, want, "both ends control the light");
        // Two plain switches do not pair.
        click(&mut tool(ElecVariant::Switch), &mut cx, 60.0, 144.0);
        click(&mut tool(ElecVariant::Switch), &mut cx, 90.0, 144.0);
        click(&mut t, &mut cx, 60.0, 141.75);
        click(&mut t, &mut cx, 90.0, 141.75);
        assert_eq!(load_electrical(cx.floor()).connections.len(), 2);
    }

    #[test]
    fn dragging_the_handle_bends_the_arc_in_one_undo_step() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch), &mut cx, 30.0, 0.0);
        click(&mut tool(ElecVariant::Light), &mut cx, 120.0, 72.0);
        let mut t = tool(ElecVariant::Connection);
        click(&mut t, &mut cx, 30.0, 2.25);
        click(&mut t, &mut cx, 120.0, 72.0);
        t.key(&mut cx, KeyEvent::escape());
        let layer = load_electrical(cx.floor());
        let handle = layer.arc_midpoint(&layer.connections[0]).unwrap();
        let before = layer.connections[0].arc_bulge;
        // Press on the handle, drag, release.
        let down = PointerEvent::at(&cx, handle);
        t.pointer_down(&mut cx, down.with_down(true));
        assert!(matches!(t.conn, Some(ConnDrag::Vertex { .. })));
        let target = handle + Point::new(0.0, 20.0);
        let mut mv = PointerEvent::at(&cx, target);
        mv.screen = down.screen + egui::vec2(0.0, -40.0);
        t.pointer_move(&mut cx, mv.with_down(true));
        let r = t.pointer_up(&mut cx, mv);
        assert_eq!(r.commit.as_deref(), Some("Bend Connection"));
        let layer = load_electrical(cx.floor());
        assert!((layer.connections[0].arc_bulge - before).abs() > 1.0);
        let m = layer.arc_midpoint(&layer.connections[0]).unwrap();
        // The arc bows to the pointer's side of the chord by the pointer's
        // distance from it.
        let (a, b) = (
            layer.device(layer.connections[0].from).unwrap().position,
            layer.device(layer.connections[0].to).unwrap().position,
        );
        let n = b.sub(a).perp().normalized();
        let want = Point::lerp(a, b, 0.5) + n * target.sub(Point::lerp(a, b, 0.5)).dot(n);
        assert!(m.dist(want) < 1e-6, "the handle follows the pointer");
        assert_eq!(cx.undo().as_deref(), Some("Bend Connection"));
        assert!((load_electrical(cx.floor()).connections[0].arc_bulge - before).abs() < 1e-9);
        // A press away from every handle bends nothing.
        let far = PointerEvent::at(&cx, Point::new(200.0, 120.0));
        t.pointer_down(&mut cx, far.with_down(true));
        assert!(!matches!(t.conn, Some(ConnDrag::Vertex { .. })));
    }

    #[test]
    fn auto_place_switches_puts_one_at_each_door_with_a_light() {
        let (mut cx, ids) = cx_with_room();
        // One door on the south wall: a plain switch and a ceiling light.
        cx.project
            .add_opening(0, ids[0], 120.0, plan_core::OpeningKind::Door)
            .unwrap();
        cx.refresh();
        let mut t = tool(ElecVariant::AutoSwitches);
        let r = click(&mut t, &mut cx, 10.0, 10.0);
        assert_eq!(r.commit.as_deref(), Some("Auto Place Switches"));
        let layer = load_electrical(cx.floor());
        let switches: Vec<_> = layer
            .devices
            .iter()
            .filter(|d| d.kind.is_switch())
            .collect();
        assert_eq!(switches.len(), 1);
        let s = switches[0];
        assert_eq!(
            (s.kind, s.height, s.wall_id),
            (DeviceKind::Switch, 48.0, Some(ids[0]))
        );
        // Beside the latch-side jamb (door spans 102..138): 6" past 138 is 144.
        assert!(
            (s.position.x - 144.0).abs() < 1e-6 || (s.position.x - 96.0).abs() < 1e-6,
            "{}",
            s.position.x
        );
        assert!(s.position.y > 0.0, "on the room side");
        let light = layer.devices.iter().find(|d| d.kind.is_light()).unwrap();
        assert_eq!(light.position, Point::new(120.0, 72.0));
        assert_eq!(layer.connections.len(), 1);
        assert_eq!(light.switched_by, vec![s.id]);
        // A second click adds nothing.
        click(&mut t, &mut cx, 10.0, 10.0);
        assert_eq!(load_electrical(cx.floor()).devices.len(), 2);
        assert!(cx.status.contains("already"));
        // Undo takes the whole run back.
        assert_eq!(cx.undo().as_deref(), Some("Auto Place Switches"));
        assert!(devices(&cx).is_empty());
    }

    #[test]
    fn a_room_with_two_doors_gets_a_three_way_pair() {
        let (mut cx, ids) = cx_with_room();
        cx.project
            .add_opening(0, ids[0], 60.0, plan_core::OpeningKind::Door)
            .unwrap();
        cx.project
            .add_opening(0, ids[2], 180.0, plan_core::OpeningKind::Door)
            .unwrap();
        cx.refresh();
        assert_eq!(auto_place_floor_switches(&mut cx), 2);
        let layer = load_electrical(cx.floor());
        let kinds: Vec<_> = layer
            .devices
            .iter()
            .filter(|d| d.kind.is_switch())
            .map(|d| d.kind)
            .collect();
        assert_eq!(kinds, [DeviceKind::Switch3Way, DeviceKind::Switch3Way]);
        let light = layer.devices.iter().find(|d| d.kind.is_light()).unwrap();
        assert_eq!(light.switched_by.len(), 2, "both doors switch the light");
        assert_eq!(layer.connections.len(), 2);
    }

    #[test]
    fn the_dialog_applies_connections_and_a_new_kind() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch), &mut cx, 30.0, 0.0);
        click(&mut tool(ElecVariant::Light), &mut cx, 120.0, 72.0);
        let mut t = tool(ElecVariant::Switch);
        let p = PointerEvent::at(&cx, Point::new(30.0, 2.25));
        assert!(t.double_click(&mut cx, p).consumed);
        let mut draft = t.dialog.borrow().as_ref().unwrap().draft().clone();
        let light = load_electrical(cx.floor())
            .devices
            .iter()
            .find(|d| d.kind.is_light())
            .unwrap()
            .id;
        draft.controls = vec![light];
        draft.kind = DeviceKind::SwitchDimmer;
        *t.applied.borrow_mut() = Some(draft);
        let r = t.pointer_move(&mut cx, p);
        assert_eq!(
            r.commit.as_deref(),
            Some("Electrical Service Specification")
        );
        let layer = load_electrical(cx.floor());
        assert_eq!(layer.connections.len(), 1);
        assert!(layer
            .devices
            .iter()
            .any(|d| d.kind == DeviceKind::SwitchDimmer));
        assert_eq!(layer.device(light).unwrap().switched_by.len(), 1);
    }

    #[test]
    fn the_dialog_stores_default_heights_in_the_same_undo_step() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Outlet110), &mut cx, 60.0, 0.0);
        let mut t = tool(ElecVariant::Outlet110);
        let p = PointerEvent::at(&cx, Point::new(60.0, 2.25));
        assert!(t.double_click(&mut cx, p).consumed);
        let mut draft = t.dialog.borrow().as_ref().unwrap().draft().clone();
        assert!(
            draft.defaults.is_some(),
            "the tool opens it with the defaults"
        );
        let mut defaults = draft.defaults.clone().unwrap();
        defaults.set_height(DeviceKind::Outlet110, 16.0);
        defaults.set_counter_height(40.0);
        draft.defaults = Some(defaults);
        draft.height = 20.0;
        *t.applied.borrow_mut() = Some(draft);
        let r = t.pointer_move(&mut cx, p);
        assert_eq!(
            r.commit.as_deref(),
            Some("Electrical Service Specification")
        );
        let stored = ElectricalDefaults::load(&cx.project);
        assert_eq!(stored.height(DeviceKind::Outlet110), 16.0);
        assert_eq!(stored.counter_height(), 40.0);
        assert_eq!(
            devices(&cx)[0].height,
            20.0,
            "this device keeps its own height"
        );
        // The next outlet uses the new default; the readout shows it.
        let mut t2 = tool(ElecVariant::Outlet110);
        let at = PointerEvent::at(&cx, Point::new(120.0, 2.25));
        t2.pointer_move(&mut cx, at);
        assert_eq!(cx.readout.as_deref(), Some("Height: 16\""));
        click(&mut t2, &mut cx, 120.0, 0.0);
        assert_eq!(devices(&cx)[1].height, 16.0);
        // One undo takes the dialog's OK away: devices and defaults together.
        cx.undo();
        cx.undo();
        assert_eq!(
            ElectricalDefaults::load(&cx.project).height(DeviceKind::Outlet110),
            12.0
        );
        assert_eq!(devices(&cx)[0].height, 12.0);
    }

    #[test]
    fn a_dialog_without_the_defaults_stores_nothing() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch), &mut cx, 30.0, 0.0);
        let layer = load_electrical(cx.floor());
        let d = layer.devices[0].clone();
        let dlg = ElectricalDialog::for_device(&d, &layer);
        let draft = dlg.draft().clone();
        assert!(draft.defaults.is_none());
        assert!(!draft.store_defaults(&mut cx.project));
        assert!(cx.project.electrical_defaults.is_none());
    }

    #[test]
    fn wiring_two_switches_to_a_light_promotes_them_to_s3() {
        let (mut cx, _) = cx_with_room();
        click(&mut tool(ElecVariant::Switch), &mut cx, 30.0, 0.0);
        click(&mut tool(ElecVariant::Switch), &mut cx, 60.0, 0.0);
        click(&mut tool(ElecVariant::Light), &mut cx, 120.0, 72.0);
        let layer = load_electrical(cx.floor());
        let sw: Vec<Id> = layer
            .devices
            .iter()
            .filter(|d| d.kind.is_switch())
            .map(|d| d.id)
            .collect();
        let light = layer.devices.iter().find(|d| d.kind.is_light()).unwrap().id;
        connect_devices(&mut cx, sw[0], light).unwrap();
        assert_eq!(
            load_electrical(cx.floor()).device(sw[0]).unwrap().kind,
            DeviceKind::Switch
        );
        connect_devices(&mut cx, sw[1], light).unwrap();
        let layer = load_electrical(cx.floor());
        assert_eq!(layer.device(sw[0]).unwrap().kind, DeviceKind::Switch3Way);
        assert_eq!(layer.device(sw[1]).unwrap().kind, DeviceKind::Switch3Way);
        cx.undo();
        let layer = load_electrical(cx.floor());
        assert_eq!(layer.device(sw[0]).unwrap().kind, DeviceKind::Switch);
        assert_eq!(layer.connections.len(), 1);
    }
}
