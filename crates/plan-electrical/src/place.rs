//! Placing devices: by hand on walls or free, and automatically.

use crate::device::{Device, DeviceKind, COUNTER_OUTLET_HEIGHT, OUTLET_HEIGHT, SWITCH_HEIGHT};
use plan_core::geometry::{dist_to_segment, point_in_polygon};
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
    let mut out = Vec::new();
    for room in rooms {
        let function = room_types
            .iter()
            .find(|(name, _)| *name == room.label)
            .map_or(RoomFunction::Other, |(_, f)| *f);
        let runs = room_runs(floor, room);

        let counters = match function {
            RoomFunction::Kitchen => true,
            RoomFunction::Bath | RoomFunction::Garage => !opts.skip_garage_bath_counters,
            _ => false,
        };
        let gfci =
            function == RoomFunction::Garage || (function.is_wet() && opts.gfci_in_wet_rooms);
        let (kind, height, max_gap) = match (counters, gfci) {
            (true, true) => (
                DeviceKind::Gfci,
                COUNTER_OUTLET_HEIGHT,
                opts.kitchen_counter_spacing,
            ),
            (true, false) => (
                DeviceKind::Outlet110,
                COUNTER_OUTLET_HEIGHT,
                opts.kitchen_counter_spacing,
            ),
            (false, true) => (DeviceKind::Gfci, OUTLET_HEIGHT, opts.max_spacing),
            (false, false) => (DeviceKind::Outlet110, OUTLET_HEIGHT, opts.max_spacing),
        };

        let mut spans: Vec<Span> = Vec::new();
        for run in &runs {
            for (a, b) in wall_spaces(floor, run.wall, run.lo, run.hi, opts.min_wall_segment) {
                spans.push(Span {
                    wall: run.wall,
                    side: run.side,
                    a,
                    b,
                    at: spread(a, b, max_gap, opts.from_opening),
                });
            }
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

        for span in &spans {
            for &t in &span.at {
                let mut d = place_on_wall(kind, span.wall, t, span.side);
                d.height = height;
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
