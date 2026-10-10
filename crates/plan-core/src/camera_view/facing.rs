//! Hide Camera-Facing Exterior Walls (manual pp. 1189, 1205; C-151).
//!
//! From outside a house, the walls between the camera and the rooms hide what
//! a dollhouse-style overview wants to show. With the option on, every
//! exterior wall whose outer face looks toward the camera is left out of the
//! view, with the doors and windows in it; Interior walls of an attic floor
//! go too. The option does nothing when the camera is inside the building
//! (inside one of its rooms).

use crate::floors::FloorKind;
use crate::geometry::Point;
use crate::model::{Id, Project, WallKind};
use std::collections::HashSet;

/// Is `eye` outside every room of the plan?
pub fn camera_is_outside(project: &Project, eye: Point) -> bool {
    !project.floors.iter().any(|f| {
        crate::rooms::detect_rooms(&f.walls, 1.0)
            .iter()
            .any(|r| r.contains(eye))
    })
}

/// Ids of the objects to leave out for a camera at `eye`: the exterior walls
/// facing it, their openings, and the interior walls of attic floors. Empty
/// when the camera is inside the building.
pub fn facing_exterior(project: &Project, eye: Point) -> HashSet<Id> {
    let mut hidden = HashSet::new();
    if !camera_is_outside(project, eye) {
        return hidden;
    }
    for f in &project.floors {
        let mut walls = HashSet::new();
        for w in &f.walls {
            if w.kind == WallKind::Exterior {
                let out = w.normal() * w.exterior_side.sign();
                let mid = Point::lerp(w.start, w.end, 0.5);
                if (eye - mid).dot(out) > 0.0 {
                    walls.insert(w.id);
                }
            } else if f.kind == FloorKind::Attic {
                walls.insert(w.id);
            }
        }
        for o in &f.openings {
            if walls.contains(&o.wall_id) {
                hidden.insert(o.id);
            }
        }
        hidden.extend(walls);
    }
    hidden
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::Project;

    fn box_house() -> Project {
        let mut p = Project::new("box");
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            let id = p.add_wall(0, pts[i], pts[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
            // Counter-clockwise: the outside is on the right.
            if let Some(w) = p.floors[0].walls.iter_mut().find(|w| w.id == id) {
                w.exterior_side = crate::walls::Side::Right;
            }
        }
        p
    }

    #[test]
    fn only_the_walls_facing_an_outside_camera_are_hidden() {
        let p = box_house();
        // South of the house: the wall along y = 0 (id 1) faces the camera.
        let ids: Vec<Id> = p.floors[0].walls.iter().map(|w| w.id).collect();
        let hidden = facing_exterior(&p, Point::new(120.0, -300.0));
        assert!(hidden.contains(&ids[0]));
        assert!(!hidden.contains(&ids[2]), "the far wall stays");
        // From the south-east corner two walls face the camera.
        let corner = facing_exterior(&p, Point::new(500.0, -300.0));
        assert!(corner.contains(&ids[0]) && corner.contains(&ids[1]));
        assert!(!corner.contains(&ids[3]));
    }

    #[test]
    fn a_camera_inside_a_room_hides_nothing() {
        let p = box_house();
        assert!(!camera_is_outside(&p, Point::new(120.0, 90.0)));
        assert!(facing_exterior(&p, Point::new(120.0, 90.0)).is_empty());
    }
}
