//! Per-run context: rooms with their types, and openings with the rooms on
//! either side of them.

use plan_core::geometry::point_in_polygon;
use plan_core::{Floor, Opening, OpeningKind, Point, Room, Wall, WallKind};
use plan_stairs::Stair;

use crate::CheckOptions;

/// How far from a wall centerline to sample the room on each side.
const SIDE_PROBE: f64 = 3.0;

/// An opening with its wall, its centre and the rooms it connects.
pub(crate) struct OpInfo<'a> {
    pub op: &'a Opening,
    pub wall: Option<&'a Wall>,
    /// Centre of the opening on the wall centerline.
    pub center: Option<Point>,
    /// Room on the wall's left (+normal) side.
    pub left: Option<usize>,
    /// Room on the wall's right (-normal) side.
    pub right: Option<usize>,
}

impl OpInfo<'_> {
    /// Whether the opening touches room `i`.
    pub fn touches(&self, i: usize) -> bool {
        self.left == Some(i) || self.right == Some(i)
    }

    /// The room on the other side of room `i` (`None` outside or unknown).
    pub fn other_side(&self, i: usize) -> Option<usize> {
        if self.left == Some(i) {
            self.right
        } else if self.right == Some(i) {
            self.left
        } else {
            None
        }
    }

    /// True when the opening leads outdoors: an exterior wall, or no room on
    /// one side.
    pub fn exterior(&self) -> bool {
        self.wall.is_some_and(|w| w.kind == WallKind::Exterior)
            || (self.left.is_some() != self.right.is_some())
    }

    /// Room on the side the door swings toward.
    pub fn swing_room(&self) -> Option<usize> {
        if self.op.swing_flipped {
            self.right
        } else {
            self.left
        }
    }
}

/// Everything the rules read.
pub(crate) struct Ctx<'a> {
    pub floor: &'a Floor,
    pub rooms: &'a [Room],
    pub stairs: &'a [Stair],
    pub opts: &'a CheckOptions,
    /// Lower-case type of each room; empty when the room is unnamed.
    pub types: Vec<String>,
    pub ops: Vec<OpInfo<'a>>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        floor: &'a Floor,
        rooms: &'a [Room],
        room_types: &[(usize, String)],
        stairs: &'a [Stair],
        opts: &'a CheckOptions,
    ) -> Self {
        let types = rooms
            .iter()
            .enumerate()
            .map(|(i, r)| room_type(floor, i, r, room_types))
            .collect();
        let ops = floor
            .openings
            .iter()
            .map(|op| op_info(floor, rooms, op))
            .collect();
        Self {
            floor,
            rooms,
            stairs,
            opts,
            types,
            ops,
        }
    }

    /// True for the grade (ground) floor, where the smaller egress area applies.
    pub fn grade_floor(&self) -> bool {
        self.floor.elevation <= 0.0
    }

    pub fn is_bedroom(&self, i: usize) -> bool {
        is_bedroom(&self.types[i])
    }

    pub fn is_bath(&self, i: usize) -> bool {
        is_bath(&self.types[i])
    }

    pub fn is_garage(&self, i: usize) -> bool {
        self.types[i].contains("garage")
    }

    /// Display name of a room for messages.
    pub fn name(&self, i: usize) -> String {
        if self.types[i].is_empty() {
            self.rooms[i].label.clone()
        } else {
            self.floor
                .room_names
                .iter()
                .find(|n| point_in_polygon(n.anchor, &self.rooms[i].polygon))
                .map(|n| n.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| self.types[i].clone())
        }
    }

    /// Openings of `kind` that bring daylight or exit into room `i`.
    pub fn exterior_openings(&self, i: usize, kind: OpeningKind) -> Vec<&OpInfo<'a>> {
        self.ops
            .iter()
            .filter(|o| o.op.kind == kind && o.touches(i) && o.exterior())
            .collect()
    }
}

/// Type of room `i`: the explicit list first, then a room name anchored
/// inside it, then a non-default label.
fn room_type(floor: &Floor, i: usize, room: &Room, given: &[(usize, String)]) -> String {
    let explicit = given
        .iter()
        .find(|(idx, t)| *idx == i && !t.trim().is_empty())
        .map(|(_, t)| t.clone());
    let named = || {
        floor
            .room_names
            .iter()
            .find(|n| point_in_polygon(n.anchor, &room.polygon))
            .map(|n| {
                if n.room_type.trim().is_empty() {
                    n.name.clone()
                } else {
                    n.room_type.clone()
                }
            })
            .filter(|t| !t.trim().is_empty())
    };
    let label = || {
        let l = room.label.trim();
        (!l.is_empty() && !l.starts_with("Room ")).then(|| l.to_string())
    };
    explicit
        .or_else(named)
        .or_else(label)
        .unwrap_or_default()
        .to_lowercase()
}

fn op_info<'a>(floor: &'a Floor, rooms: &[Room], op: &'a Opening) -> OpInfo<'a> {
    let wall = floor.wall(op.wall_id);
    let center = wall.map(|w| w.point_at(op.center_offset.clamp(0.0, w.length())));
    let (mut left, mut right) = (None, None);
    if let (Some(w), Some(c)) = (wall, center) {
        let n = w.normal().scale(SIDE_PROBE);
        let find = |p: Point| rooms.iter().position(|r| point_in_polygon(p, &r.polygon));
        left = find(c.add(n));
        right = find(c.sub(n));
    }
    OpInfo {
        op,
        wall,
        center,
        left,
        right,
    }
}

pub(crate) fn is_bath(t: &str) -> bool {
    ["bath", "powder", "toilet", "lavatory", "restroom", "wc"]
        .iter()
        .any(|k| t.contains(k))
}

pub(crate) fn is_bedroom(t: &str) -> bool {
    !is_bath(t) && ["bed", "master", "nursery"].iter().any(|k| t.contains(k))
}

pub(crate) fn is_habitable(t: &str) -> bool {
    is_bedroom(t)
        || [
            "living", "dining", "kitchen", "family", "office", "den", "study", "great", "nook",
            "library", "media", "playroom", "game", "bonus", "loft", "sunroom", "sun room",
        ]
        .iter()
        .any(|k| t.contains(k))
}

pub(crate) fn is_hall(t: &str) -> bool {
    ["hall", "corridor", "passage"]
        .iter()
        .any(|k| t.contains(k))
}
