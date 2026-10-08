//! The plan model. Chief Architect's core idea is that the *plan is the model*:
//! walls, openings, floors and later cabinets/roofs/stairs live here once, and
//! the 2D plan, 3D view, elevations and schedules are all generated from it.

use crate::cad::{CadItem, CadObject};
use crate::dimension::Dimension;
use crate::geometry::{point_in_polygon, Point};
use crate::layers::LayerSet;
use crate::rooms::Room;
use serde::{Deserialize, Serialize};

pub type Id = u64;

/// Default exterior wall: 2x6 framing + sheathing + drywall ≈ 6 1/2".
pub const DEFAULT_EXTERIOR_THICKNESS: f64 = 6.5;
/// Default interior wall: 2x4 framing + 1/2" drywall both sides = 4 1/2".
pub const DEFAULT_INTERIOR_THICKNESS: f64 = 4.5;
/// Default ceiling height 9'-1 1/8" (matches a 9' wall with 1 1/8" subfloor stack).
pub const DEFAULT_CEILING_HEIGHT: f64 = 109.125;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallKind {
    Exterior,
    Interior,
}

/// Layer new walls are placed on.
pub const DEFAULT_WALL_LAYER: &str = "Walls, Normal";

fn default_wall_layer() -> String {
    DEFAULT_WALL_LAYER.to_string()
}

/// Which end of a wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallEnd {
    Start,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wall {
    pub id: Id,
    /// Centerline start, inches.
    pub start: Point,
    /// Centerline end, inches.
    pub end: Point,
    pub thickness: f64,
    pub height: f64,
    pub kind: WallKind,
    /// Layer name; see [`LayerSet`]. Defaults to "Walls, Normal".
    #[serde(default = "default_wall_layer")]
    pub layer: String,
}

impl Wall {
    pub fn length(&self) -> f64 {
        self.start.dist(self.end)
    }
    /// Unit direction from start to end.
    pub fn direction(&self) -> Point {
        self.end.sub(self.start).normalized()
    }
    /// Unit normal (left side when walking start→end).
    pub fn normal(&self) -> Point {
        self.direction().perp()
    }
    pub fn point_at(&self, dist_from_start: f64) -> Point {
        self.start.add(self.direction().scale(dist_from_start))
    }
    /// Four corners of the wall footprint (centerline ± thickness/2).
    pub fn footprint(&self) -> [Point; 4] {
        let n = self.normal().scale(self.thickness * 0.5);
        [
            self.start.add(n),
            self.end.add(n),
            self.end.sub(n),
            self.start.sub(n),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningKind {
    Door,
    Window,
}

/// A door or window hosted in a wall. Position is measured along the wall
/// centerline from `start`, like Chief's "distance from wall end" fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opening {
    pub id: Id,
    pub wall_id: Id,
    /// Distance from the wall start to the opening center, inches.
    pub center_offset: f64,
    pub width: f64,
    pub height: f64,
    /// Bottom of the opening above the floor (0 for doors).
    pub sill_height: f64,
    pub kind: OpeningKind,
    /// Doors only: hinge on the far jamb and swing to the other side.
    pub swing_flipped: bool,
}

impl Opening {
    pub fn default_door(id: Id, wall_id: Id, center_offset: f64) -> Self {
        Self {
            id,
            wall_id,
            center_offset,
            width: 36.0,
            height: 80.0,
            sill_height: 0.0,
            kind: OpeningKind::Door,
            swing_flipped: false,
        }
    }
    pub fn default_window(id: Id, wall_id: Id, center_offset: f64) -> Self {
        Self {
            id,
            wall_id,
            center_offset,
            width: 36.0,
            height: 60.0,
            sill_height: 24.0,
            kind: OpeningKind::Window,
            swing_flipped: false,
        }
    }
    pub fn start_offset(&self) -> f64 {
        self.center_offset - self.width * 0.5
    }
    pub fn end_offset(&self) -> f64 {
        self.center_offset + self.width * 0.5
    }
}

/// A room's user-assigned name. Rooms are derived from walls, so a name is
/// attached to a point inside the room instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomName {
    pub anchor: Point,
    pub name: String,
    pub room_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Floor {
    pub name: String,
    /// Finished-floor elevation relative to the first floor, inches.
    pub elevation: f64,
    pub ceiling_height: f64,
    pub walls: Vec<Wall>,
    pub openings: Vec<Opening>,
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    #[serde(default)]
    pub cad: Vec<CadObject>,
    #[serde(default)]
    pub room_names: Vec<RoomName>,
}

impl Floor {
    pub fn new(name: impl Into<String>, elevation: f64) -> Self {
        Self {
            name: name.into(),
            elevation,
            ceiling_height: DEFAULT_CEILING_HEIGHT,
            walls: Vec::new(),
            openings: Vec::new(),
            dimensions: Vec::new(),
            cad: Vec::new(),
            room_names: Vec::new(),
        }
    }
    pub fn wall(&self, id: Id) -> Option<&Wall> {
        self.walls.iter().find(|w| w.id == id)
    }
    pub fn wall_mut(&mut self, id: Id) -> Option<&mut Wall> {
        self.walls.iter_mut().find(|w| w.id == id)
    }
    pub fn openings_on(&self, wall_id: Id) -> impl Iterator<Item = &Opening> {
        self.openings.iter().filter(move |o| o.wall_id == wall_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub floors: Vec<Floor>,
    next_id: Id,
    #[serde(default = "LayerSet::default_floor_plan")]
    pub layers: LayerSet,
}

/// Minimum clear distance between an opening jamb and a wall end or another opening.
const OPENING_MARGIN: f64 = 2.0;

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            floors: vec![Floor::new("1st Floor", 0.0)],
            next_id: 1,
            layers: LayerSet::default_floor_plan(),
        }
    }

    pub fn alloc_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_wall(
        &mut self,
        floor: usize,
        start: Point,
        end: Point,
        thickness: f64,
        height: f64,
        kind: WallKind,
    ) -> Id {
        let id = self.alloc_id();
        self.floors[floor].walls.push(Wall {
            id,
            start,
            end,
            thickness,
            height,
            kind,
            layer: default_wall_layer(),
        });
        id
    }

    /// Move one end of a wall. Openings keep their distance from the wall
    /// start, so callers may want to re-validate them afterwards.
    /// Returns `false` if the wall does not exist.
    pub fn move_wall_endpoint(
        &mut self,
        floor: usize,
        id: Id,
        which_end: WallEnd,
        new_pos: Point,
    ) -> bool {
        match self.floors[floor].wall_mut(id) {
            Some(w) => {
                match which_end {
                    WallEnd::Start => w.start = new_pos,
                    WallEnd::End => w.end = new_pos,
                }
                true
            }
            None => false,
        }
    }

    /// Move a whole wall by `delta`. Returns `false` if the wall does not exist.
    pub fn translate_wall(&mut self, floor: usize, id: Id, delta: Point) -> bool {
        match self.floors[floor].wall_mut(id) {
            Some(w) => {
                w.start = w.start.add(delta);
                w.end = w.end.add(delta);
                true
            }
            None => false,
        }
    }

    /// Add a dimension (its `id` is replaced with a fresh one) and return the id.
    pub fn add_dimension(&mut self, floor: usize, mut dim: Dimension) -> Id {
        let id = self.alloc_id();
        dim.id = id;
        self.floors[floor].dimensions.push(dim);
        id
    }

    pub fn remove_dimension(&mut self, floor: usize, id: Id) {
        self.floors[floor].dimensions.retain(|d| d.id != id);
    }

    /// Add a CAD item on `layer` and return its id.
    pub fn add_cad(&mut self, floor: usize, layer: impl Into<String>, item: CadItem) -> Id {
        let id = self.alloc_id();
        self.floors[floor].cad.push(CadObject {
            id,
            layer: layer.into(),
            item,
        });
        id
    }

    pub fn remove_cad(&mut self, floor: usize, id: Id) {
        self.floors[floor].cad.retain(|c| c.id != id);
    }

    /// Name the room containing `anchor`. Any existing name whose anchor lies
    /// in the same detected room (from `rooms`) is replaced. If `anchor` is
    /// not inside any room, names anchored within 1" of it are replaced.
    pub fn set_room_name(
        &mut self,
        floor: usize,
        anchor: Point,
        name: impl Into<String>,
        room_type: impl Into<String>,
        rooms: &[Room],
    ) {
        let names = &mut self.floors[floor].room_names;
        match rooms.iter().find(|r| point_in_polygon(anchor, &r.polygon)) {
            Some(room) => names.retain(|n| !point_in_polygon(n.anchor, &room.polygon)),
            None => names.retain(|n| n.anchor.dist(anchor) > 1.0),
        }
        names.push(RoomName {
            anchor,
            name: name.into(),
            room_type: room_type.into(),
        });
    }

    /// Remove a wall and every opening hosted in it.
    pub fn remove_wall(&mut self, floor: usize, id: Id) {
        let f = &mut self.floors[floor];
        f.walls.retain(|w| w.id != id);
        f.openings.retain(|o| o.wall_id != id);
    }

    pub fn remove_opening(&mut self, floor: usize, id: Id) {
        self.floors[floor].openings.retain(|o| o.id != id);
    }

    /// Place an opening on `wall_id` centered as close to `center_offset` as
    /// fits. Returns `None` if the wall is too short or the spot overlaps
    /// another opening.
    pub fn add_opening(
        &mut self,
        floor: usize,
        wall_id: Id,
        center_offset: f64,
        kind: OpeningKind,
    ) -> Option<Id> {
        let id = self.alloc_id();
        let mut opening = match kind {
            OpeningKind::Door => Opening::default_door(id, wall_id, center_offset),
            OpeningKind::Window => Opening::default_window(id, wall_id, center_offset),
        };
        let f = &mut self.floors[floor];
        let wall_len = f.wall(wall_id)?.length();
        let half = opening.width * 0.5;
        if wall_len < opening.width + 2.0 * OPENING_MARGIN {
            return None;
        }
        opening.center_offset = opening
            .center_offset
            .clamp(half + OPENING_MARGIN, wall_len - half - OPENING_MARGIN);
        let overlaps = f.openings_on(wall_id).any(|o| {
            opening.start_offset() < o.end_offset() + OPENING_MARGIN
                && opening.end_offset() > o.start_offset() - OPENING_MARGIN
        });
        if overlaps {
            return None;
        }
        f.openings.push(opening);
        Some(id)
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openings_clamp_and_reject_overlap() {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            DEFAULT_INTERIOR_THICKNESS,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        );
        // Asked for center at 5", clamped to 18 + 2 = 20".
        let d = p.add_opening(0, w, 5.0, OpeningKind::Door).unwrap();
        let o = p.floors[0].openings.iter().find(|o| o.id == d).unwrap();
        assert!((o.center_offset - 20.0).abs() < 1e-9);
        // Overlapping second door is rejected.
        assert!(p.add_opening(0, w, 30.0, OpeningKind::Door).is_none());
        // Far enough along fits.
        assert!(p.add_opening(0, w, 90.0, OpeningKind::Door).is_some());
        // Too short a wall rejects.
        let short = p.add_wall(
            0,
            Point::new(0.0, 50.0),
            Point::new(30.0, 50.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        assert!(p.add_opening(0, short, 15.0, OpeningKind::Door).is_none());
    }

    #[test]
    fn json_round_trip() {
        let mut p = Project::new("rt");
        p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        let s = p.to_json().unwrap();
        let q = Project::from_json(&s).unwrap();
        assert_eq!(q.floors[0].walls.len(), 1);
        assert_eq!(q.name, "rt");
    }

    #[test]
    fn old_json_without_new_fields_still_loads() {
        let old = r#"{
            "name": "legacy",
            "floors": [{
                "name": "1st Floor",
                "elevation": 0.0,
                "ceiling_height": 109.125,
                "walls": [{
                    "id": 1,
                    "start": {"x": 0.0, "y": 0.0},
                    "end": {"x": 100.0, "y": 0.0},
                    "thickness": 6.5,
                    "height": 109.125,
                    "kind": "Exterior"
                }],
                "openings": []
            }],
            "next_id": 2
        }"#;
        let p = Project::from_json(old).unwrap();
        assert_eq!(p.floors[0].walls[0].layer, "Walls, Normal");
        assert!(p.floors[0].dimensions.is_empty());
        assert!(p.floors[0].cad.is_empty());
        assert!(p.floors[0].room_names.is_empty());
        assert!(p.layers.get("Doors").is_some());
        // And the new format round-trips.
        let q = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(q.layers, p.layers);
    }

    #[test]
    fn edit_helpers() {
        use crate::cad::CadItem;
        use crate::dimension::{Dimension, DimensionKind};
        let mut p = Project::new("e");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        assert!(p.move_wall_endpoint(0, w, WallEnd::End, Point::new(150.0, 0.0)));
        assert!((p.floors[0].walls[0].length() - 150.0).abs() < 1e-9);
        assert!(p.translate_wall(0, w, Point::new(10.0, 20.0)));
        assert_eq!(p.floors[0].walls[0].start, Point::new(10.0, 20.0));
        assert!(!p.translate_wall(0, 999, Point::ZERO));

        let d = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::ZERO,
                Point::new(10.0, 0.0),
                6.0,
            ),
        );
        assert_eq!(p.floors[0].dimensions[0].id, d);
        p.remove_dimension(0, d);
        assert!(p.floors[0].dimensions.is_empty());

        let c = p.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(5.0, 5.0),
            },
        );
        assert_eq!(p.floors[0].cad[0].id, c);
        p.remove_cad(0, c);
        assert!(p.floors[0].cad.is_empty());
    }

    #[test]
    fn room_names_replace_within_same_room() {
        let room = Room {
            polygon: vec![
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
                Point::new(0.0, 100.0),
            ],
            area_sq_in: 10_000.0,
            centroid: Point::new(50.0, 50.0),
            label: "Room 1".into(),
        };
        let rooms = [room];
        let mut p = Project::new("r");
        p.set_room_name(0, Point::new(10.0, 10.0), "Kitchen", "Kitchen", &rooms);
        p.set_room_name(0, Point::new(90.0, 90.0), "Pantry", "Pantry", &rooms);
        assert_eq!(p.floors[0].room_names.len(), 1);
        assert_eq!(p.floors[0].room_names[0].name, "Pantry");
        // A point outside every room adds a separate name.
        p.set_room_name(0, Point::new(500.0, 500.0), "Yard", "Other", &rooms);
        assert_eq!(p.floors[0].room_names.len(), 2);
    }
}
