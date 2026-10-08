//! The plan model. Chief Architect's core idea is that the *plan is the model*:
//! walls, openings, floors and later cabinets/roofs/stairs live here once, and
//! the 2D plan, 3D view, elevations and schedules are all generated from it.

use crate::geometry::Point;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Floor {
    pub name: String,
    /// Finished-floor elevation relative to the first floor, inches.
    pub elevation: f64,
    pub ceiling_height: f64,
    pub walls: Vec<Wall>,
    pub openings: Vec<Opening>,
}

impl Floor {
    pub fn new(name: impl Into<String>, elevation: f64) -> Self {
        Self {
            name: name.into(),
            elevation,
            ceiling_height: DEFAULT_CEILING_HEIGHT,
            walls: Vec::new(),
            openings: Vec::new(),
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
}

/// Minimum clear distance between an opening jamb and a wall end or another opening.
const OPENING_MARGIN: f64 = 2.0;

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            floors: vec![Floor::new("1st Floor", 0.0)],
            next_id: 1,
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
        });
        id
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
}
