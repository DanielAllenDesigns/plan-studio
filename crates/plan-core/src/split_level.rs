//! Split-level floors (R-86): rooms of one floor at different heights.
//!
//! Chief builds a split level from rooms with their own Floor Height (the
//! Structure tab of the Room Specification, R-23) divided by walls or Room
//! Dividers. This module finds where two rooms meet at different heights
//! (a [`LevelStep`]), which is what the plan marks, what the 3D view fills
//! with a riser where no wall stands, and where steps or stairs go.

use crate::geometry::{dist_to_segment, Point};
use crate::model::{Floor, Id};
use crate::rooms::Room;
use crate::walls::WallClass;

/// Differences smaller than this are not a step, inches.
pub const MIN_STEP: f64 = 0.5;
/// Greatest riser height (IRC R311.7.5.1: 7 3/4"), inches.
pub const MAX_RISER: f64 = 7.75;
/// Two room edges closer than this and parallel are one shared edge, inches.
const SHARE_TOL: f64 = 1.0;

/// Where two rooms of different floor heights meet.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelStep {
    /// The shared edge, along the walls' centerline.
    pub a: Point,
    pub b: Point,
    /// Index (in the room list the steps were found in) of the lower room.
    pub low_room: usize,
    pub high_room: usize,
    /// Finished floor height of each room above the floor datum, inches.
    pub low: f64,
    pub high: f64,
    /// A wall runs along the step: its id, and whether it is a solid wall
    /// (not a railing, divider or invisible one) that hides the riser.
    pub wall: Option<Id>,
    pub solid_wall: bool,
}

impl LevelStep {
    /// The height of the step, inches.
    pub fn rise(&self) -> f64 {
        self.high - self.low
    }

    pub fn length(&self) -> f64 {
        self.a.dist(self.b)
    }

    pub fn mid(&self) -> Point {
        Point::lerp(self.a, self.b, 0.5)
    }

    /// Unit vector across the step pointing from the low room into the high
    /// room (needs the high room's interior point).
    pub fn toward_high(&self, high_interior: Point) -> Point {
        let n = self.b.sub(self.a).normalized().perp();
        if high_interior.sub(self.mid()).dot(n) >= 0.0 {
            n
        } else {
            -n
        }
    }

    /// Risers needed between the two levels (7 3/4" at most each).
    pub fn risers(&self) -> u32 {
        risers_for(self.rise())
    }
}

/// Risers needed to climb `rise` inches.
pub fn risers_for(rise: f64) -> u32 {
    if rise < MIN_STEP {
        0
    } else {
        (rise / MAX_RISER).ceil() as u32
    }
}

/// Finished floor height of `room` above the floor datum: its Floor Height
/// plus its floor finish.
pub fn room_level(floor: &Floor, room: &Room) -> f64 {
    let named = room.name_entry(&floor.room_names);
    let offset = named.map_or(0.0, |n| n.floor_height_offset);
    let finish = named
        .and_then(|n| n.misc.as_ref())
        .map_or(floor.settings.floor_finish_thickness, |m| {
            m.floor_finish_thickness
        });
    offset + finish.max(0.0)
}

/// The steps of `floor`: every stretch where two of `rooms` share an edge
/// and their floors differ by more than [`MIN_STEP`]. A room with no floor
/// (open below) or on a deck is compared like any other.
pub fn level_steps(floor: &Floor, rooms: &[Room]) -> Vec<LevelStep> {
    let levels: Vec<f64> = rooms.iter().map(|r| room_level(floor, r)).collect();
    let mut out = Vec::new();
    for i in 0..rooms.len() {
        for j in (i + 1)..rooms.len() {
            if (levels[i] - levels[j]).abs() < MIN_STEP {
                continue;
            }
            for (a, b) in shared_edges(&rooms[i].polygon, &rooms[j].polygon) {
                let (low_room, high_room) = if levels[i] < levels[j] {
                    (i, j)
                } else {
                    (j, i)
                };
                let (wall, solid_wall) = wall_along(floor, a, b);
                out.push(LevelStep {
                    a,
                    b,
                    low_room,
                    high_room,
                    low: levels[low_room],
                    high: levels[high_room],
                    wall,
                    solid_wall,
                });
            }
        }
    }
    out
}

/// The stretches where two polygons have a collinear edge in common.
pub fn shared_edges(p: &[Point], q: &[Point]) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    let (n, m) = (p.len(), q.len());
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        let len = a.dist(b);
        if len < 2.0 {
            continue;
        }
        let dir = b.sub(a).normalized();
        for k in 0..m {
            let (c, d) = (q[k], q[(k + 1) % m]);
            if d.sub(c).normalized().cross(dir).abs() > 0.02 {
                continue;
            }
            if dist_to_segment(c, a, b).min(dist_to_segment(d, a, b)) > SHARE_TOL
                && dist_to_segment(a, c, d).min(dist_to_segment(b, c, d)) > SHARE_TOL
            {
                continue;
            }
            // Distance of the other edge's line from this one.
            if c.sub(a).cross(dir).abs() > SHARE_TOL {
                continue;
            }
            let (tc, td) = (c.sub(a).dot(dir), d.sub(a).dot(dir));
            let (t0, t1) = (tc.min(td).max(0.0), tc.max(td).min(len));
            if t1 - t0 > 2.0 {
                out.push((a + dir * t0, a + dir * t1));
            }
        }
    }
    out
}

/// The wall along the stretch `a`-`b` and whether it is a solid one.
fn wall_along(floor: &Floor, a: Point, b: Point) -> (Option<Id>, bool) {
    let mid = Point::lerp(a, b, 0.5);
    let dir = b.sub(a).normalized();
    let mut found: Option<Id> = None;
    let mut solid = false;
    for w in &floor.walls {
        if w.length() < 2.0 || w.direction().cross(dir).abs() > 0.02 {
            continue;
        }
        if dist_to_segment(mid, w.start, w.end) > w.thickness * 0.5 + SHARE_TOL {
            continue;
        }
        found.get_or_insert(w.id);
        let hidden = w.flags.invisible
            || w.flags.room_divider
            || w.flags.railing
            || matches!(
                w.class,
                WallClass::RoomDivider | WallClass::Railing | WallClass::DeckRailing
            );
        if !hidden && !w.flags.half_wall {
            found = Some(w.id);
            solid = true;
        }
    }
    (found, solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, RoomName, WallKind};
    use crate::rooms::detect_rooms;
    use crate::walls::WallClass;

    /// Two 10 x 10 ft rooms side by side, the right one `rise` inches up.
    fn two_rooms(rise: f64, divider: Option<WallClass>) -> (Project, Vec<Room>) {
        let mut p = Project::new("split");
        let f = &mut p.floors[0];
        let corners = [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)];
        let mut id = 1;
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let mut w = crate::model::Wall::new(
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                4.5,
                109.0,
                WallKind::Exterior,
            );
            w.id = id;
            id += 1;
            f.walls.push(w);
        }
        let mut mid = crate::model::Wall::new(
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            4.5,
            109.0,
            WallKind::Interior,
        );
        mid.id = id;
        if let Some(c) = divider {
            mid.class = c.clone();
            if c == WallClass::RoomDivider {
                mid.flags.room_divider = true;
            }
        }
        f.walls.push(mid);
        f.room_names
            .push(RoomName::new(Point::new(60.0, 60.0), "Low", "Living"));
        let mut high = RoomName::new(Point::new(180.0, 60.0), "High", "Living");
        high.floor_height_offset = rise;
        f.room_names.push(high);
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        (p, rooms)
    }

    #[test]
    fn rooms_at_different_heights_share_a_step() {
        let (p, rooms) = two_rooms(24.0, None);
        assert_eq!(rooms.len(), 2);
        let steps = level_steps(&p.floors[0], &rooms);
        assert_eq!(steps.len(), 1);
        let s = &steps[0];
        assert!((s.rise() - 24.0).abs() < 1e-9);
        assert!((s.length() - 120.0).abs() < 1e-6);
        assert!((s.a.x - 120.0).abs() < 1e-6 && (s.b.x - 120.0).abs() < 1e-6);
        // 24" in 7 3/4" risers is 4 (a 6" riser each).
        assert_eq!(s.risers(), 4);
        assert!(s.solid_wall, "an interior wall stands on the step");
        // The high room is the one at x > 120.
        assert!(rooms[s.high_room].centroid.x > 120.0);
        assert!(rooms[s.low_room].centroid.x < 120.0);
    }

    #[test]
    fn rooms_at_one_height_have_no_step() {
        let (p, rooms) = two_rooms(0.0, None);
        assert!(level_steps(&p.floors[0], &rooms).is_empty());
        let (p, rooms) = two_rooms(0.25, None);
        assert!(level_steps(&p.floors[0], &rooms).is_empty());
    }

    #[test]
    fn a_room_divider_leaves_the_riser_showing() {
        let (p, rooms) = two_rooms(8.0, Some(WallClass::RoomDivider));
        let steps = level_steps(&p.floors[0], &rooms);
        assert_eq!(steps.len(), 1);
        assert!(!steps[0].solid_wall);
        assert!(steps[0].wall.is_some());
        assert_eq!(steps[0].risers(), 2);
    }

    #[test]
    fn the_direction_toward_the_high_room_points_across_the_step() {
        let (p, rooms) = two_rooms(12.0, None);
        let s = &level_steps(&p.floors[0], &rooms)[0];
        let n = s.toward_high(rooms[s.high_room].centroid);
        assert!((n.x - 1.0).abs() < 1e-9 && n.y.abs() < 1e-9);
    }

    #[test]
    fn risers_follow_the_code_maximum() {
        assert_eq!(risers_for(0.0), 0);
        assert_eq!(risers_for(0.4), 0);
        assert_eq!(risers_for(6.0), 1);
        assert_eq!(risers_for(7.75), 1);
        assert_eq!(risers_for(7.76), 2);
        assert_eq!(risers_for(48.0), 7);
    }

    #[test]
    fn shared_edges_need_collinear_overlap() {
        let a = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        let b = vec![
            Point::new(100.0, 40.0),
            Point::new(200.0, 40.0),
            Point::new(200.0, 140.0),
            Point::new(100.0, 140.0),
        ];
        let e = shared_edges(&a, &b);
        assert_eq!(e.len(), 1);
        assert!((e[0].0.dist(e[0].1) - 60.0).abs() < 1e-6);
        let far = vec![
            Point::new(110.0, 0.0),
            Point::new(200.0, 0.0),
            Point::new(200.0, 50.0),
            Point::new(110.0, 50.0),
        ];
        assert!(shared_edges(&a, &far).is_empty());
    }
}
