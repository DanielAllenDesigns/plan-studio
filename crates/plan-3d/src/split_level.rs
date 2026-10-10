//! Split-level floors in 3D (R-86): a riser where two rooms of one floor
//! meet at different heights and no solid wall stands on the step.
//!
//! The rooms' platforms already sit at their own Floor Height
//! (`slab::room_levels`); the riser closes the open side of the raised one
//! so its edge reads as a step, from the lower floor up to the raised one.

use crate::builder::MeshSet;
use crate::fireplace::add_prism;
use crate::mesh::{Material, Mesh};
use plan_core::split_level::{level_steps, LevelStep};
use plan_core::{detect_rooms, Floor, Project, Room};

/// How far the riser reaches into the raised room, inches.
const RISER_THICKNESS: f64 = 1.5;

/// The riser of one step, as a mesh at scene elevation of `floor`.
pub fn riser_mesh(floor: &Floor, rooms: &[Room], step: &LevelStep) -> Option<Mesh> {
    if step.solid_wall || step.length() < 1.0 || step.rise() < 0.5 {
        return None;
    }
    let high = rooms.get(step.high_room)?;
    let n = step.toward_high(high.centroid);
    let strip = vec![
        step.a,
        step.b,
        step.b + n * RISER_THICKNESS,
        step.a + n * RISER_THICKNESS,
    ];
    let mut set = MeshSet::default();
    add_prism(
        set.material(Material::Floor),
        &strip,
        floor.elevation + step.low,
        floor.elevation + step.high,
    );
    set.finish(None).into_iter().next()
}

/// The risers of every split-level step of the project.
pub fn riser_meshes(project: &Project) -> Vec<Mesh> {
    let mut out = Vec::new();
    for floor in &project.floors {
        // Only floors where a room has its own height can have a step.
        if floor
            .room_names
            .iter()
            .all(|n| n.floor_height_offset.abs() < 0.5 && n.misc.is_none())
        {
            continue;
        }
        let rooms = detect_rooms(&floor.walls, 0.5);
        for step in level_steps(floor, &rooms) {
            out.extend(riser_mesh(floor, &rooms, &step));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{Point, RoomName, Wall, WallClass, WallKind};

    fn project(rise: f64, divider: bool) -> Project {
        let mut p = Project::new("split");
        let f = &mut p.floors[0];
        let corners = [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)];
        let mut id = 1;
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let mut w = Wall::new(
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
        let mut mid = Wall::new(
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            4.5,
            109.0,
            WallKind::Interior,
        );
        mid.id = id;
        if divider {
            mid.class = WallClass::RoomDivider;
            mid.flags.room_divider = true;
        }
        f.walls.push(mid);
        f.room_names
            .push(RoomName::new(Point::new(60.0, 60.0), "Low", "Living"));
        let mut high = RoomName::new(Point::new(180.0, 60.0), "High", "Living");
        high.floor_height_offset = rise;
        f.room_names.push(high);
        p
    }

    #[test]
    fn a_room_divider_between_two_levels_gets_a_riser() {
        let p = project(24.0, true);
        let meshes = riser_meshes(&p);
        assert_eq!(meshes.len(), 1);
        let (lo, hi) = meshes[0].bounds().unwrap();
        // 24" tall (from the low finished floor to the high one), 120" long,
        // 1.5" deep on the high side of x = 120.
        assert!(
            (f64::from(hi[1] - lo[1]) - 24.0).abs() < 1e-3,
            "{lo:?} {hi:?}"
        );
        assert!((f64::from(lo[0]) - 120.0).abs() < 1e-3);
        assert!((f64::from(hi[0]) - 121.5).abs() < 1e-3);
        assert!((f64::from(hi[2] - lo[2]) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn a_solid_wall_hides_the_riser() {
        let p = project(24.0, false);
        assert!(riser_meshes(&p).is_empty());
    }

    #[test]
    fn rooms_at_one_height_have_no_riser() {
        let p = project(0.0, true);
        assert!(riser_meshes(&p).is_empty());
    }
}
