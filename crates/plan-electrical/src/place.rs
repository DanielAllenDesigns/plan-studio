//! Placing devices: by hand on walls or free, and automatically.

use crate::defaults::ElectricalDefaults;
use crate::device::{Device, DeviceKind, COUNTER_OUTLET_HEIGHT, OUTLET_HEIGHT, SWITCH_HEIGHT};
use plan_core::geometry::{dist_to_segment, point_in_polygon};
use plan_core::rooms::{ElectricalRules, OutletPlacement};
use plan_core::{Floor, Opening, OpeningKind, Point, Room, Wall};
use serde::{Deserialize, Serialize};

/// Which face of a wall: left of the start-to-end direction, or right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallSide {
    Left,
    Right,
}

impl WallSide {
    /// `+1.0` for left (along [`Wall::normal`]), `-1.0` for right.
    pub fn sign(self) -> f64 {
        match self {
            WallSide::Left => 1.0,
            WallSide::Right => -1.0,
        }
    }
}

/// What a room is used for; drives which outlets Auto Place adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoomFunction {
    Living,
    Kitchen,
    Dining,
    Bedroom,
    Bath,
    Laundry,
    Garage,
    Hall,
    Other,
}

impl RoomFunction {
    /// Rooms where receptacles are GFCI protected when the option is on.
    pub fn is_wet(self) -> bool {
        matches!(
            self,
            RoomFunction::Kitchen | RoomFunction::Bath | RoomFunction::Laundry
        )
    }
}

/// Place a wall-mounted device on a wall face.
///
/// The device sits on the face half a wall thickness from the centerline on
/// `side`, faces away from the wall, and gets its default height. The returned
/// device has id `0`; use [`ElectricalLayer::add`](crate::ElectricalLayer::add).
pub fn place_on_wall(kind: DeviceKind, wall: &Wall, offset_along: f64, side: WallSide) -> Device {
    let normal = wall.normal() * side.sign();
    Device {
        id: 0,
        kind,
        position: wall.point_at(offset_along) + normal * (wall.thickness * 0.5),
        angle: normal.angle(),
        height: kind.default_height(),
        wall_id: Some(wall.id),
        circuit: None,
        label: String::new(),
        switched_by: Vec::new(),
        finish: String::new(),
        hide_label: false,
    }
}

/// Place a free-standing or ceiling device at `pos` (angle 0, default height).
pub fn place_free(kind: DeviceKind, pos: Point) -> Device {
    Device {
        id: 0,
        kind,
        position: pos,
        angle: 0.0,
        height: kind.default_height(),
        wall_id: None,
        circuit: None,
        label: String::new(),
        switched_by: Vec::new(),
        finish: String::new(),
        hide_label: false,
    }
}

/// How far past a wall face the probe point for [`face_is_exterior`] lies, inches.
const PROBE_BEYOND_FACE: f64 = 4.0;

/// Is the face of `wall` on `side` outdoors at `offset` along the wall?
///
/// A face is outdoors when the point just past it lies in an exterior room
/// (`is_exterior_room`: a deck, balcony or court), or when it lies in no room
/// at all and the wall is an exterior wall. The tools place a weatherproof
/// outlet, switch or wall light there (manual p. 693).
pub fn face_is_exterior(
    wall: &Wall,
    side: WallSide,
    offset: f64,
    rooms: &[Room],
    is_exterior_room: &dyn Fn(&Room) -> bool,
) -> bool {
    let normal = wall.normal() * side.sign();
    let probe = wall.point_at(offset.clamp(0.0, wall.length()))
        + normal * (wall.thickness * 0.5 + PROBE_BEYOND_FACE);
    let here: Vec<&Room> = rooms.iter().filter(|r| r.contains(probe)).collect();
    if here.is_empty() {
        wall.kind == plan_core::WallKind::Exterior
    } else {
        here.iter().all(|r| is_exterior_room(r))
    }
}

/// The kind a tool places at a click: the `tool` kind indoors, its
/// [`outdoor`](DeviceKind::outdoor) counterpart where `exterior`.
pub fn kind_for_setting(tool: DeviceKind, exterior: bool) -> DeviceKind {
    if exterior {
        tool.outdoor()
    } else {
        tool
    }
}

/// Knobs for [`auto_place_outlets`]; lengths in inches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoOutletOptions {
    /// Largest distance between outlets along a wall (12 ft: nothing is over 6 ft from one).
    pub max_spacing: f64,
    /// Wall spaces shorter than this (between corners and doors) get no outlet.
    pub min_wall_segment: f64,
    /// Minimum clearance from a door jamb to an outlet.
    pub from_opening: f64,
    /// Maximum spacing of kitchen counter outlets.
    pub kitchen_counter_spacing: f64,
    /// Kitchens, baths and laundries get GFCI receptacles.
    pub gfci_in_wet_rooms: bool,
    /// When true, garages and baths use plain wall spacing; when false they get
    /// kitchen-style counter-height outlets too.
    pub skip_garage_bath_counters: bool,
    /// Height of a general receptacle, inches (12").
    pub outlet_height: f64,
    /// Height of a kitchen counter receptacle, inches (44").
    pub counter_height: f64,
    /// Height of a weatherproof exterior receptacle, inches (18").
    pub wp_height: f64,
    /// Auto Place Outlets also adds the exterior weatherproof GFCI receptacles
    /// (NEC 210.52(E): one at the front and one at the back of the house).
    pub exterior_wp: bool,
    /// A wall space in a bath, kitchen, laundry or garage shorter than
    /// `min_wall_segment` still gets one receptacle when it is at least this
    /// long and the room would otherwise have none, inches.
    pub min_wet_segment: f64,
    /// The footprints of the plan's counter-carrying cabinets (base, corner,
    /// blind and filler cabinets, countertops), plan inches. Counter-height
    /// outlets go only on the wall spaces a run of these stands against; a
    /// room with no cabinet on its walls keeps outlets on every wall. Empty
    /// (the default) ignores cabinets; the application supplies it because
    /// this crate does not depend on `plan-cabinets`.
    #[serde(default)]
    pub counter_runs: Vec<Vec<Point>>,
    /// The large appliances standing in the plan; each gets an outlet on the
    /// wall behind it (220 V for a range or dryer, 110 V otherwise).
    #[serde(default)]
    pub appliances: Vec<ApplianceSpot>,
    /// The centres of the plan's sinks; each gets a light overhead.
    #[serde(default)]
    pub sinks: Vec<Point>,
}

/// The large appliances that need their own outlet (manual p. 447).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplianceKind {
    Range,
    Dryer,
    Refrigerator,
    Dishwasher,
}

impl ApplianceKind {
    /// The appliance a library item stands for, from its catalog id.
    pub fn from_catalog(catalog_id: &str) -> Option<Self> {
        let id = catalog_id.to_ascii_lowercase();
        [
            ("dishwasher", Self::Dishwasher),
            ("refrigerator", Self::Refrigerator),
            ("fridge", Self::Refrigerator),
            ("dryer", Self::Dryer),
            ("range", Self::Range),
            ("cooktop", Self::Range),
        ]
        .into_iter()
        .find(|(k, _)| id.contains(k))
        .map(|(_, a)| a)
    }

    /// The outlet kind behind it: 220 V for a range or dryer, 110 V otherwise.
    pub fn outlet_kind(self) -> DeviceKind {
        match self {
            Self::Range | Self::Dryer => DeviceKind::Outlet220,
            Self::Refrigerator | Self::Dishwasher => DeviceKind::Outlet110,
        }
    }
}

/// One appliance standing in the plan: its centre and footprint size, inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ApplianceSpot {
    pub kind: ApplianceKind,
    pub center: Point,
    pub depth: f64,
}

/// How far the back of an appliance may stand from the wall and still count
/// as against it, inches.
const APPLIANCE_AGAINST_WALL: f64 = 12.0;

/// Walls that carry no devices: invisible, railing, room divider and
/// generated walls.
pub fn wall_takes_devices(w: &Wall) -> bool {
    use plan_core::walls::WallClass;
    !(w.flags.invisible
        || w.flags.railing
        || w.flags.room_divider
        || w.flags.auto_generated
        || w.class.is_railing()
        || matches!(w.class, WallClass::RoomDivider | WallClass::DeckEdge))
}

/// The outlet behind `spot`: on the nearest wall run of the room it stands
/// against, level with the appliance.
fn appliance_outlet(
    runs: &[Run<'_>],
    spot: &ApplianceSpot,
    opts: &AutoOutletOptions,
) -> Option<Device> {
    let mut best: Option<(f64, &Run<'_>, f64)> = None;
    for run in runs {
        let d = spot.center.sub(run.wall.start);
        let t = d.dot(run.wall.direction());
        let into = d.dot(run.wall.normal()) * run.side.sign() - run.wall.thickness * 0.5;
        let gap = into - spot.depth * 0.5;
        if t < run.lo - 6.0 || t > run.hi + 6.0 || gap > APPLIANCE_AGAINST_WALL || into < 0.0 {
            continue;
        }
        if best.is_none_or(|(g, _, _)| gap < g) {
            best = Some((gap, run, t.clamp(run.lo, run.hi)));
        }
    }
    let (_, run, t) = best?;
    let mut dev = place_on_wall(spot.kind.outlet_kind(), run.wall, t, run.side);
    dev.height = opts.outlet_height;
    Some(dev)
}

/// How far from a wall's face a cabinet may stand and still count as against
/// it, inches.
const COUNTER_AGAINST_WALL: f64 = 6.0;
/// The deepest a counter run reaches from the wall face (a 25.5" base cabinet
/// plus overhang), inches; a bigger polygon is an island, not a run.
const COUNTER_MAX_DEPTH: f64 = 40.0;
/// The shortest stretch of counter that gets an outlet, inches.
const COUNTER_MIN_RUN: f64 = 12.0;

/// The stretches of `wall` (offsets along it) a run of `counters` stands
/// against: each polygon with a side within [`COUNTER_AGAINST_WALL`] of the
/// wall face and no deeper than [`COUNTER_MAX_DEPTH`], by its extent along
/// the wall.
fn counter_intervals(wall: &Wall, counters: &[Vec<Point>]) -> Vec<(f64, f64)> {
    let dir = wall.direction();
    let normal = dir.perp();
    let half = wall.thickness * 0.5;
    let mut out: Vec<(f64, f64)> = Vec::new();
    for poly in counters {
        if poly.len() < 3 {
            continue;
        }
        let (mut near, mut far) = (f64::MAX, 0.0_f64);
        let (mut t0, mut t1) = (f64::MAX, f64::MIN);
        for v in poly {
            let d = v.sub(wall.start);
            let off = d.dot(normal).abs() - half;
            near = near.min(off);
            far = far.max(off);
            let t = d.dot(dir);
            t0 = t0.min(t);
            t1 = t1.max(t);
        }
        if near <= COUNTER_AGAINST_WALL
            && far <= COUNTER_MAX_DEPTH
            && t1 > 0.0
            && t0 < wall.length()
        {
            out.push((t0.max(0.0), t1.min(wall.length())));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Join neighbours that touch.
    let mut joined: Vec<(f64, f64)> = Vec::new();
    for (a, b) in out {
        match joined.last_mut() {
            Some(last) if a <= last.1 + 1.0 => last.1 = last.1.max(b),
            _ => joined.push((a, b)),
        }
    }
    joined
}

impl AutoOutletOptions {
    /// The defaults with the heights of the plan's [`ElectricalDefaults`].
    pub fn with_defaults(defaults: &ElectricalDefaults) -> Self {
        Self {
            outlet_height: defaults.height(DeviceKind::Outlet110),
            counter_height: defaults.counter_height(),
            wp_height: defaults.height(DeviceKind::OutletWp),
            ..Self::default()
        }
    }
}

impl Default for AutoOutletOptions {
    fn default() -> Self {
        Self {
            max_spacing: 144.0,
            min_wall_segment: 24.0,
            from_opening: 6.0,
            kitchen_counter_spacing: 48.0,
            gfci_in_wet_rooms: true,
            skip_garage_bath_counters: true,
            outlet_height: OUTLET_HEIGHT,
            counter_height: COUNTER_OUTLET_HEIGHT,
            wp_height: crate::device::WP_OUTLET_HEIGHT,
            exterior_wp: true,
            min_wet_segment: 12.0,
            counter_runs: Vec::new(),
            appliances: Vec::new(),
            sinks: Vec::new(),
        }
    }
}

/// Tolerance when matching room polygon edges to wall centerlines, inches.
const EDGE_TOLERANCE: f64 = 1.5;
/// Overhead-door width that counts as a garage bay, inches.
const BAY_DOOR_WIDTH: f64 = 84.0;
/// Fallback bay footprint (10 ft x 20 ft) when no overhead doors are found, sq in.
const BAY_AREA: f64 = 28_800.0;

/// One stretch of wall inside a room: offsets run along `wall`, interior faces only.
struct Run<'a> {
    wall: &'a Wall,
    side: WallSide,
    lo: f64,
    hi: f64,
}

/// Unbroken wall space and the outlet offsets placed on it.
struct Span<'a> {
    wall: &'a Wall,
    side: WallSide,
    a: f64,
    b: f64,
    at: Vec<f64>,
    kind: DeviceKind,
    height: f64,
}

impl Span<'_> {
    /// Widest gap between neighbours (or span ends) as `(width, midpoint)`.
    fn widest_gap(&self) -> (f64, f64) {
        let mut stops = vec![self.a];
        stops.extend(&self.at);
        stops.push(self.b);
        stops
            .windows(2)
            .map(|w| (w[1] - w[0], (w[0] + w[1]) * 0.5))
            .max_by(|x, y| x.0.total_cmp(&y.0))
            .unwrap_or((0.0, self.a))
    }
}

/// Wall (by index) whose centerline carries the room edge `p`-`q`, if any.
fn match_wall(walls: &[Wall], p: Point, q: Point) -> Option<usize> {
    walls
        .iter()
        .enumerate()
        .filter_map(|(i, w)| {
            let (dp, dq) = (
                dist_to_segment(p, w.start, w.end),
                dist_to_segment(q, w.start, w.end),
            );
            (dp <= EDGE_TOLERANCE && dq <= EDGE_TOLERANCE).then_some((i, dp + dq))
        })
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(i, _)| i)
}

/// Collapse a room polygon into maximal runs along single walls, with the
/// face-to-face extent of each (corners inset by the neighbouring wall's half thickness).
fn room_runs<'a>(floor: &'a Floor, room: &Room) -> Vec<Run<'a>> {
    struct Raw {
        wall: Option<usize>,
        p: Point,
        q: Point,
    }
    let n = room.polygon.len();
    let mut raw: Vec<Raw> = Vec::new();
    for i in 0..n {
        let (p, q) = (room.polygon[i], room.polygon[(i + 1) % n]);
        let wall = match_wall(&floor.walls, p, q);
        match raw.last_mut() {
            Some(last) if last.wall.is_some() && last.wall == wall => last.q = q,
            _ => raw.push(Raw { wall, p, q }),
        }
    }
    if raw.len() > 1 && raw[0].wall.is_some() && raw[0].wall == raw[raw.len() - 1].wall {
        let first = raw.remove(0);
        if let Some(last) = raw.last_mut() {
            last.q = first.q;
        }
    }
    let half = |r: &Raw| r.wall.map_or(0.0, |i| floor.walls[i].thickness * 0.5);
    let m = raw.len();
    let mut runs = Vec::new();
    for (i, r) in raw.iter().enumerate() {
        let Some(wi) = r.wall else { continue };
        let wall = &floor.walls[wi];
        if !wall_takes_devices(wall) {
            continue;
        }
        let dir = wall.direction();
        let forward = r.q.sub(r.p).dot(dir) >= 0.0;
        let (inset_start, inset_end) = (half(&raw[(i + m - 1) % m]), half(&raw[(i + 1) % m]));
        let (tp, tq) = (r.p.sub(wall.start).dot(dir), r.q.sub(wall.start).dot(dir));
        let (lo, hi) = if forward {
            (tp + inset_start, tq - inset_end)
        } else {
            (tq + inset_end, tp - inset_start)
        };
        if hi > lo {
            runs.push(Run {
                wall,
                side: if forward {
                    WallSide::Left
                } else {
                    WallSide::Right
                },
                lo,
                hi,
            });
        }
    }
    runs
}

/// Split `[lo, hi]` of `wall` at its doors into wall spaces at least `min_len` long.
fn wall_spaces(floor: &Floor, wall: &Wall, lo: f64, hi: f64, min_len: f64) -> Vec<(f64, f64)> {
    let mut doors: Vec<(f64, f64)> = floor
        .openings_on(wall.id)
        .filter(|o| o.kind == OpeningKind::Door)
        .map(|o| (o.start_offset(), o.end_offset()))
        .collect();
    doors.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut spaces = Vec::new();
    let mut cursor = lo;
    for (s, e) in doors {
        if e <= cursor {
            continue;
        }
        if s >= hi {
            break;
        }
        if s > cursor {
            spaces.push((cursor, s));
        }
        cursor = cursor.max(e);
    }
    if cursor < hi {
        spaces.push((cursor, hi));
    }
    spaces.retain(|(a, b)| b - a >= min_len);
    spaces
}

/// The parts of `[a, b]` outside the sorted `cover` intervals that are at
/// least `min_len` long.
fn subtract_covered(a: f64, b: f64, cover: &[(f64, f64)], min_len: f64) -> Vec<(f64, f64)> {
    let mut free = Vec::new();
    let mut cursor = a;
    for &(c0, c1) in cover {
        if c1 <= cursor {
            continue;
        }
        if c0 >= b {
            break;
        }
        if c0 > cursor {
            free.push((cursor, c0));
        }
        cursor = cursor.max(c1);
    }
    if cursor < b {
        free.push((cursor, b));
    }
    free.retain(|(x, y)| y - x >= min_len);
    free
}

/// Evenly spread the fewest outlets so none is farther than `max_gap` from
/// its neighbour (so nothing is over `max_gap / 2` from an outlet), kept
/// `clear` away from the space ends when there is room.
fn spread(a: f64, b: f64, max_gap: f64, clear: f64) -> Vec<f64> {
    let len = b - a;
    let count = (len / max_gap).ceil().max(1.0) as usize;
    (0..count)
        .map(|i| {
            let t = a + len * (2 * i + 1) as f64 / (2 * count) as f64;
            if len > 2.0 * clear {
                t.clamp(a + clear, b - clear)
            } else {
                (a + b) * 0.5
            }
        })
        .collect()
}

/// Number of vehicle bays: overhead-width doors on the room's walls, else by area.
fn garage_bays(floor: &Floor, room: &Room, runs: &[Run]) -> usize {
    let wide = |o: &&Opening| o.kind == OpeningKind::Door && o.width >= BAY_DOOR_WIDTH;
    let doors: usize = runs
        .iter()
        .enumerate()
        // A wall can appear in several runs only if it is revisited; count each once.
        .filter(|(i, r)| runs[..*i].iter().all(|p| p.wall.id != r.wall.id))
        .map(|(_, r)| floor.openings_on(r.wall.id).filter(wide).count())
        .sum();
    if doors > 0 {
        doors
    } else {
        (room.area_sq_in / BAY_AREA).round().max(1.0) as usize
    }
}

/// Automatically place receptacles around every room.
///
/// Walking each room's boundary on the interior side, walls are split at door
/// openings. Every wall space of at least `min_wall_segment` gets outlets
/// spread so that none is more than `max_spacing` from the next and nothing is
/// more than half that from one; outlets stay `from_opening` clear of jambs.
/// Kitchens get counter-height (44") outlets at most `kitchen_counter_spacing`
/// apart instead; wet rooms use GFCI when `gfci_in_wet_rooms`; garages are all
/// GFCI with at least one per bay. Rooms are matched to `room_types` by
/// [`Room::label`]; unmatched rooms are [`RoomFunction::Other`].
///
/// Returned devices carry `wall_id`, face the room, have the room label as
/// `label`, and still have id `0`; add them with [`ElectricalLayer::add_all`](crate::ElectricalLayer::add_all).
pub fn auto_place_outlets(
    floor: &Floor,
    rooms: &[Room],
    room_types: &[(String, RoomFunction)],
    opts: &AutoOutletOptions,
) -> Vec<Device> {
    auto_place_outlets_by_rules(floor, rooms, room_types, &[], opts)
}

/// [`auto_place_outlets`] with the electrical rules of each room's function
/// and type (manual p. 447; `plan_core::rooms::electrical_rules`), keyed by
/// [`Room::label`]; a room without an entry follows the plain rules.
///
/// * [`OutletPlacement::None`] (exterior rooms, Porches, Open Below): no outlets.
/// * [`OutletPlacement::Fewer`] (other hybrid rooms such as a garage): twice
///   the spacing between outlets; a garage still gets one per bay.
/// * `gfci_over_base_cabinets` (kitchens and baths): the wall spaces a run of
///   base cabinets stands against get GFCI outlets at the counter height.
///   A bath with a vanity keeps no outlets elsewhere.
/// * `standard_height_outlets` (kitchens): the wall spaces without cabinets
///   get plain outlets at the standard height as well.
pub fn auto_place_outlets_by_rules(
    floor: &Floor,
    rooms: &[Room],
    room_types: &[(String, RoomFunction)],
    rules: &[(String, ElectricalRules)],
    opts: &AutoOutletOptions,
) -> Vec<Device> {
    let mut out = Vec::new();
    for room in rooms {
        let rule = rules
            .iter()
            .find(|(name, _)| *name == room.label)
            .map(|(_, r)| *r);
        let known = rule.is_some();
        let rule = rule.unwrap_or_default();
        if rule.outlets == OutletPlacement::None {
            continue;
        }
        let gap_scale = if rule.outlets == OutletPlacement::Fewer {
            2.0
        } else {
            1.0
        };
        let function = room_types
            .iter()
            .find(|(name, _)| *name == room.label)
            .map_or(RoomFunction::Other, |(_, f)| *f);
        let runs = room_runs(floor, room);

        // A kitchen or bath with base cabinets on its walls takes counter
        // outlets over them whatever the options say (the room's rules).
        let cabinet_counters = rule.gfci_over_base_cabinets
            && runs
                .iter()
                .any(|r| !counter_intervals(r.wall, &opts.counter_runs).is_empty());
        let counters = cabinet_counters
            || match function {
                RoomFunction::Kitchen => true,
                RoomFunction::Bath | RoomFunction::Garage => !opts.skip_garage_bath_counters,
                _ => false,
            };
        let gfci = function == RoomFunction::Garage
            || (function.is_wet() && opts.gfci_in_wet_rooms)
            || (rule.gfci_over_base_cabinets && counters);
        let (kind, height, max_gap) = match (counters, gfci) {
            (true, true) => (
                DeviceKind::Gfci,
                opts.counter_height,
                opts.kitchen_counter_spacing,
            ),
            (true, false) => (
                DeviceKind::Outlet110,
                opts.counter_height,
                opts.kitchen_counter_spacing,
            ),
            (false, true) => (DeviceKind::Gfci, opts.outlet_height, opts.max_spacing),
            (false, false) => (DeviceKind::Outlet110, opts.outlet_height, opts.max_spacing),
        };
        let max_gap = max_gap * gap_scale;

        // Counter outlets follow the cabinets when the room's walls have
        // any: only the spaces a run of base cabinets stands against.
        let counter_cover: Vec<Vec<(f64, f64)>> = runs
            .iter()
            .map(|r| {
                if counters {
                    counter_intervals(r.wall, &opts.counter_runs)
                } else {
                    Vec::new()
                }
            })
            .collect();
        let follow_cabinets = counter_cover.iter().any(|c| !c.is_empty());
        let collect = |min_len: f64| -> Vec<Span> {
            let mut spans: Vec<Span> = Vec::new();
            for (ri, run) in runs.iter().enumerate() {
                for (a, b) in wall_spaces(floor, run.wall, run.lo, run.hi, min_len) {
                    let pieces: Vec<(f64, f64)> = if follow_cabinets {
                        counter_cover[ri]
                            .iter()
                            .map(|&(c0, c1)| (a.max(c0), b.min(c1)))
                            .filter(|(x, y)| y - x >= COUNTER_MIN_RUN)
                            .collect()
                    } else {
                        vec![(a, b)]
                    };
                    for (a, b) in pieces {
                        spans.push(Span {
                            wall: run.wall,
                            side: run.side,
                            a,
                            b,
                            at: spread(a, b, max_gap, opts.from_opening),
                            kind,
                            height,
                        });
                    }
                }
            }
            spans
        };
        let mut spans = collect(opts.min_wall_segment);
        // A small bath, kitchen, laundry or garage still gets a receptacle.
        let needs_one = gfci || counters || function == RoomFunction::Laundry;
        if spans.is_empty() && needs_one && opts.min_wet_segment < opts.min_wall_segment {
            let mut small = collect(opts.min_wet_segment);
            small.sort_by(|x, y| (y.b - y.a).total_cmp(&(x.b - x.a)));
            small.truncate(1);
            spans = small;
        }

        if function == RoomFunction::Garage {
            let bays = garage_bays(floor, room, &runs);
            // Top up to one GFCI per bay by splitting the widest remaining gap.
            while spans.iter().map(|s| s.at.len()).sum::<usize>() < bays {
                let best = spans
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (i, s.widest_gap()))
                    .max_by(|x, y| x.1 .0.total_cmp(&y.1 .0));
                match best {
                    Some((i, (gap, mid))) if gap > 2.0 * opts.from_opening => {
                        spans[i].at.push(mid);
                        spans[i].at.sort_by(f64::total_cmp);
                    }
                    _ => break,
                }
            }
        }

        // A kitchen's standard height outlets: the wall spaces the cabinets
        // leave free (only where cabinets stand, else the counter outlets run
        // all around).
        if known && rule.standard_height_outlets && follow_cabinets {
            for (ri, run) in runs.iter().enumerate() {
                for (a, b) in wall_spaces(floor, run.wall, run.lo, run.hi, opts.min_wall_segment) {
                    for (a, b) in subtract_covered(a, b, &counter_cover[ri], opts.min_wall_segment)
                    {
                        spans.push(Span {
                            wall: run.wall,
                            side: run.side,
                            a,
                            b,
                            at: spread(a, b, opts.max_spacing, opts.from_opening),
                            kind: DeviceKind::Outlet110,
                            height: opts.outlet_height,
                        });
                    }
                }
            }
        }
        for span in &spans {
            for &t in &span.at {
                let mut d = place_on_wall(span.kind, span.wall, t, span.side);
                d.height = span.height;
                d.label = room.label.clone();
                out.push(d);
            }
        }
        // 110/220 V outlets behind the range, dryer, refrigerator and
        // dishwasher standing in the room, and a light over each sink.
        for spot in &opts.appliances {
            if point_in_polygon(spot.center, &room.polygon) {
                if let Some(mut d) = appliance_outlet(&runs, spot, opts) {
                    d.label = room.label.clone();
                    out.push(d);
                }
            }
        }
        for &sink in &opts.sinks {
            if point_in_polygon(sink, &room.polygon) {
                let mut d = place_free(DeviceKind::RecessedCan, sink);
                d.label = room.label.clone();
                out.push(d);
            }
        }
    }
    out
}

/// A ceiling light at the room's centroid (moved onto the widest scanline
/// through the centroid when the centroid falls outside an L- or U-shaped room).
pub fn auto_place_room_light(room: &Room) -> Device {
    let mut pos = room.centroid;
    if !point_in_polygon(pos, &room.polygon) {
        let n = room.polygon.len();
        let mut xs: Vec<f64> = (0..n)
            .filter_map(|i| {
                let (a, b) = (room.polygon[i], room.polygon[(i + 1) % n]);
                ((a.y > pos.y) != (b.y > pos.y))
                    .then(|| a.x + (pos.y - a.y) * (b.x - a.x) / (b.y - a.y))
            })
            .collect();
        xs.sort_by(f64::total_cmp);
        if let Some(pair) = xs
            .windows(2)
            .step_by(2)
            .max_by(|p, q| (p[1] - p[0]).total_cmp(&(q[1] - q[0])))
        {
            pos.x = (pair[0] + pair[1]) * 0.5;
        }
    }
    let mut d = place_free(DeviceKind::CeilingLight, pos);
    d.label = room.label.clone();
    d
}

/// Distance of the light switch from the latch-side jamb, inches.
const SWITCH_JAMB_OFFSET: f64 = 6.0;

/// A light switch 48" high, 6" past the door's latch-side jamb, on the room side.
///
/// The latch is at the far jamb unless `swing_flipped` (hinge on the far
/// jamb), in which case it is at the near jamb. The room side is whichever
/// face of `wall` points into `room`.
pub fn auto_place_switch(room: &Room, door_opening: &Opening, wall: &Wall) -> Device {
    let (latch, away) = if door_opening.swing_flipped {
        (door_opening.start_offset(), -1.0)
    } else {
        (door_opening.end_offset(), 1.0)
    };
    let offset = (latch + away * SWITCH_JAMB_OFFSET).clamp(0.0, wall.length());
    let probe = |side: WallSide| {
        let p =
            wall.point_at(offset) + wall.normal() * (side.sign() * (wall.thickness * 0.5 + 2.0));
        point_in_polygon(p, &room.polygon)
    };
    let side = if probe(WallSide::Left) || !probe(WallSide::Right) {
        WallSide::Left
    } else {
        WallSide::Right
    };
    let mut d = place_on_wall(DeviceKind::Switch, wall, offset, side);
    d.height = SWITCH_HEIGHT;
    d.label = room.label.clone();
    d
}

/// Weatherproof GFCI receptacles on the outside of the house (NEC 210.52(E)):
/// one at the front and one at the back, each on the exterior face of an
/// exterior wall, in its widest stretch clear of doors, at `opts.wp_height`.
///
/// The front is the longest exterior wall; the back is the longest exterior
/// wall that faces the opposite way (the other side of the house). With no
/// opposite wall only the front gets one. The exterior face is the side of
/// the wall that lies outside every room. Returned devices have id `0`.
pub fn auto_place_exterior_outlets(
    floor: &Floor,
    rooms: &[Room],
    opts: &AutoOutletOptions,
) -> Vec<Device> {
    // (wall index, exterior side, outward normal, widest wall space)
    let mut cands: Vec<(usize, WallSide, Point, (f64, f64))> = Vec::new();
    for (i, w) in floor.walls.iter().enumerate() {
        if w.kind != plan_core::WallKind::Exterior || w.length() < 48.0 || w.curve.is_some() {
            continue;
        }
        let probe = |side: WallSide| {
            let p = w.point_at(w.length() * 0.5)
                + w.normal() * (side.sign() * (w.thickness * 0.5 + 6.0));
            rooms.iter().any(|r| point_in_polygon(p, &r.polygon))
        };
        let side = match (probe(WallSide::Left), probe(WallSide::Right)) {
            (true, false) => WallSide::Right,
            (false, true) => WallSide::Left,
            (false, false) => match w.exterior_side {
                plan_core::Side::Left => WallSide::Left,
                plan_core::Side::Right => WallSide::Right,
            },
            // Rooms on both sides: not on the outside of the house.
            (true, true) => continue,
        };
        let spaces = wall_spaces(floor, w, 0.0, w.length(), opts.min_wall_segment);
        let Some(best) = spaces
            .into_iter()
            .max_by(|x, y| (x.1 - x.0).total_cmp(&(y.1 - y.0)))
        else {
            continue;
        };
        cands.push((i, side, w.normal() * side.sign(), best));
    }
    cands.sort_by(|x, y| {
        floor.walls[y.0]
            .length()
            .total_cmp(&floor.walls[x.0].length())
            .then(floor.walls[x.0].id.cmp(&floor.walls[y.0].id))
    });
    let mut chosen: Vec<usize> = Vec::new();
    if let Some(front) = cands.first() {
        chosen.push(0);
        if let Some(back) = cands.iter().position(|c| c.2.dot(front.2) < -0.5) {
            chosen.push(back);
        }
    }
    chosen
        .into_iter()
        .map(|ci| {
            let (wi, side, _, (a, b)) = cands[ci];
            let w = &floor.walls[wi];
            let mut d = place_on_wall(DeviceKind::OutletWp, w, (a + b) * 0.5, side);
            d.height = opts.wp_height;
            d.label = "Exterior".into();
            d
        })
        .collect()
}
