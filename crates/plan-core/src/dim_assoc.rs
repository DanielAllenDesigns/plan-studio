//! Associative dimensions: a manual dimension whose end sits on a wall keeps
//! that end tied to the wall, so it follows when the wall is moved, stretched
//! or reshaped.
//!
//! An end is tied with a [`DimAnchor`]: the wall, where along it (its start,
//! its end or a fraction of its length) and how far to the side of the
//! centerline. The point is recomputed from the wall by
//! [`Floor::sync_dimension_anchors`], which the editor runs whenever the plan
//! changes. Each anchor remembers the point it last resolved to: if the
//! dimension's end has been moved by hand since (it no longer matches), the
//! tie is dropped instead of dragging the end back.

use crate::dimension::{Dimension, DimensionKind};
use crate::geometry::Point;
use crate::model::{Floor, Wall};
use crate::Id;
use serde::{Deserialize, Serialize};

/// Where along its wall an anchor sits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DimAttach {
    /// At the wall's start.
    Start,
    /// At the wall's end.
    End,
    /// This fraction (0..1) of the way from the start to the end.
    Along(f64),
}

/// One tied end of a dimension.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimAnchor {
    pub wall: Id,
    pub at: DimAttach,
    /// Distance to the left of the wall's centerline (negative: right).
    pub side: f64,
    /// The point the anchor last resolved to.
    pub last: Point,
}

impl DimAnchor {
    /// The point this anchor names on `wall` now.
    pub fn resolve(&self, wall: &Wall) -> Point {
        let base = match self.at {
            DimAttach::Start => wall.start,
            DimAttach::End => wall.end,
            DimAttach::Along(t) => Point::lerp(wall.start, wall.end, t),
        };
        base.add(wall.normal().scale(self.side))
    }
}

/// Ends closer than this to a wall point are tied to it, inches.
pub const ATTACH_TOL: f64 = 0.5;

/// The anchor for `p` on `wall`, when `p` lies on the wall's centerline, or
/// on one of its faces, at its start, its end or within its length.
fn anchor_for(wall: &Wall, p: Point) -> Option<DimAnchor> {
    let len = wall.length();
    if len < 1e-6 {
        return None;
    }
    let (dir, normal) = (wall.direction(), wall.normal());
    let rel = p.sub(wall.start);
    let along = rel.dot(dir);
    let side = rel.dot(normal);
    let half = wall.thickness * 0.5;
    // On the centerline or on either face.
    let on_line = side.abs() <= ATTACH_TOL
        || (side - half).abs() <= ATTACH_TOL
        || (side + half).abs() <= ATTACH_TOL;
    if !on_line || along < -ATTACH_TOL || along > len + ATTACH_TOL {
        return None;
    }
    // Snap the side to the exact line it is on.
    let side = [0.0, half, -half]
        .into_iter()
        .find(|s| (side - s).abs() <= ATTACH_TOL)
        .unwrap_or(side);
    let at = if along.abs() <= ATTACH_TOL {
        DimAttach::Start
    } else if (along - len).abs() <= ATTACH_TOL {
        DimAttach::End
    } else {
        DimAttach::Along(along / len)
    };
    let mut a = DimAnchor {
        wall: wall.id,
        at,
        side,
        last: p,
    };
    a.last = a.resolve(wall);
    Some(a)
}

impl Floor {
    /// Ties the ends of manual dimension `id` to the walls they lie on (an
    /// end that lies on no wall stays free). Walls are tried in order, so
    /// the first one at an end where several meet wins. Returns how many
    /// ends were tied.
    pub fn attach_dimension(&mut self, id: Id) -> usize {
        let Some(i) = self.dimensions.iter().position(|d| d.id == id) else {
            return 0;
        };
        // A typed text (and a baseline string's computed one) would go
        // stale as the ends move.
        if self.dimensions[i].kind != DimensionKind::Manual
            || self.dimensions[i].text_override.is_some()
        {
            return 0;
        }
        let ends = [self.dimensions[i].start, self.dimensions[i].end];
        let mut anchors = [None, None];
        for (k, p) in ends.into_iter().enumerate() {
            anchors[k] = self.walls.iter().find_map(|w| anchor_for(w, p));
        }
        let n = anchors.iter().flatten().count();
        self.dimensions[i].anchors = anchors;
        n
    }

    /// Moves the tied ends of every dimension to where their walls are now.
    /// A tie to a wall that no longer exists, or to an end that was moved by
    /// hand, is dropped. Returns whether anything changed.
    pub fn sync_dimension_anchors(&mut self) -> bool {
        let walls = &self.walls;
        let mut changed = false;
        for d in &mut self.dimensions {
            if d.anchors.iter().all(Option::is_none) {
                continue;
            }
            changed |= sync_dimension(d, walls);
        }
        changed
    }
}

fn sync_dimension(d: &mut Dimension, walls: &[Wall]) -> bool {
    let mut changed = false;
    for k in 0..2 {
        let Some(a) = d.anchors[k] else { continue };
        let current = if k == 0 { d.start } else { d.end };
        let Some(wall) = walls.iter().find(|w| w.id == a.wall) else {
            d.anchors[k] = None;
            changed = true;
            continue;
        };
        // The end was moved by hand since the last sync: let go.
        if current.dist(a.last) > 1e-6 {
            d.anchors[k] = None;
            changed = true;
            continue;
        }
        let target = a.resolve(wall);
        if target.dist(current) > 1e-9 {
            if k == 0 {
                d.start = target;
            } else {
                d.end = target;
            }
            d.anchors[k] = Some(DimAnchor { last: target, ..a });
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, WallKind};

    fn house() -> Project {
        let mut p = Project::new("t");
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 144.0),
            Point::new(0.0, 144.0),
        ];
        for i in 0..4 {
            p.add_wall(0, pts[i], pts[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        p
    }

    fn dim(p: &mut Project, a: Point, b: Point) -> Id {
        let id = p.add_dimension(0, Dimension::new(0, DimensionKind::Manual, a, b, 24.0));
        p.floors[0].attach_dimension(id);
        id
    }

    fn get(p: &Project, id: Id) -> &Dimension {
        p.floors[0].dimensions.iter().find(|d| d.id == id).unwrap()
    }

    #[test]
    fn a_dimension_between_wall_corners_follows_a_moved_wall() {
        let mut p = house();
        // Outer faces of the south wall's two ends: x 0..240 at y = -3.
        let id = dim(&mut p, Point::new(0.0, -3.0), Point::new(240.0, -3.0));
        let a = get(&p, id).anchors;
        assert!(a[0].is_some() && a[1].is_some());
        assert_eq!(a[0].unwrap().at, DimAttach::Start);
        assert_eq!(a[1].unwrap().at, DimAttach::End);
        // Nothing moved: nothing changes.
        assert!(!p.floors[0].sync_dimension_anchors());
        // Stretch the east end of the south wall by 60".
        let south = p.floors[0].walls[0].id;
        p.floors[0].wall_mut(south).unwrap().end = Point::new(300.0, 0.0);
        assert!(p.floors[0].sync_dimension_anchors());
        let d = get(&p, id);
        assert_eq!(d.start, Point::new(0.0, -3.0));
        assert!(d.end.dist(Point::new(300.0, -3.0)) < 1e-9, "{:?}", d.end);
        assert!((d.length() - 300.0).abs() < 1e-9);
        // Move the whole wall up: both ends follow.
        let w = p.floors[0].wall_mut(south).unwrap();
        w.start = Point::new(0.0, 10.0);
        w.end = Point::new(300.0, 10.0);
        p.floors[0].sync_dimension_anchors();
        let d = get(&p, id);
        assert!(d.start.dist(Point::new(0.0, 7.0)) < 1e-9);
        assert!(d.end.dist(Point::new(300.0, 7.0)) < 1e-9);
        // Idempotent.
        assert!(!p.floors[0].sync_dimension_anchors());
    }

    #[test]
    fn a_point_along_a_wall_keeps_its_fraction() {
        let mut p = house();
        let id = dim(&mut p, Point::new(60.0, 0.0), Point::new(240.0, 144.0));
        let a = get(&p, id).anchors;
        assert_eq!(a[0].unwrap().at, DimAttach::Along(0.25));
        let south = p.floors[0].walls[0].id;
        p.floors[0].wall_mut(south).unwrap().end = Point::new(480.0, 0.0);
        p.floors[0].sync_dimension_anchors();
        assert!(get(&p, id).start.dist(Point::new(120.0, 0.0)) < 1e-9);
    }

    #[test]
    fn a_hand_moved_end_and_a_deleted_wall_let_go() {
        let mut p = house();
        let id = dim(&mut p, Point::new(0.0, 0.0), Point::new(240.0, 0.0));
        // The user drags the start by hand.
        p.floors[0]
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap()
            .start = Point::new(10.0, 5.0);
        let south = p.floors[0].walls[0].id;
        p.floors[0].wall_mut(south).unwrap().end = Point::new(300.0, 0.0);
        p.floors[0].sync_dimension_anchors();
        let d = get(&p, id);
        assert_eq!(d.start, Point::new(10.0, 5.0), "not dragged back");
        assert!(d.anchors[0].is_none());
        assert!(d.anchors[1].is_some());
        assert!(d.end.dist(Point::new(300.0, 0.0)) < 1e-9);
        // The wall goes away: the end stays where it was and is free.
        p.floors[0].walls.retain(|w| w.id != south);
        p.floors[0].sync_dimension_anchors();
        let d = get(&p, id);
        assert!(d.anchors.iter().all(Option::is_none));
        assert!(d.end.dist(Point::new(300.0, 0.0)) < 1e-9);
    }

    #[test]
    fn free_ends_and_other_kinds_are_not_tied() {
        let mut p = house();
        let id = dim(&mut p, Point::new(500.0, 500.0), Point::new(600.0, 500.0));
        assert!(get(&p, id).anchors.iter().all(Option::is_none));
        let auto = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::AutoExterior,
                Point::new(0.0, 0.0),
                Point::new(240.0, 0.0),
                24.0,
            ),
        );
        assert_eq!(p.floors[0].attach_dimension(auto), 0);
        // Old files without the field load.
        let json = serde_json::to_string(get(&p, id)).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("anchors");
        let back: Dimension = serde_json::from_value(v).unwrap();
        assert!(back.anchors.iter().all(Option::is_none));
    }

    #[test]
    fn anchors_survive_the_json() {
        let mut p = house();
        let id = dim(&mut p, Point::new(0.0, 0.0), Point::new(240.0, 0.0));
        let json = serde_json::to_string(get(&p, id)).unwrap();
        let back: Dimension = serde_json::from_str(&json).unwrap();
        assert_eq!(&back, get(&p, id));
    }
}
