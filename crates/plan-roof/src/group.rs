//! Roof Groups (manual p. 826; R-114, RF-9).
//!
//! Automatic roofing tries to join every roof plane into one integrated
//! system, so each part of the structure influences the roof over the rest.
//! A room can be given a Roof Group number in its Room Specification; the
//! program then treats the rooms of each non-default group as a separate
//! building and builds its roof apart from the others, so the groups cannot
//! influence one another.
//!
//! Group 0 is the default group. Rooms left in it keep Plan Studio's own
//! rule for a plan that stands at several heights: rooms at one plate height
//! share a roof, rooms at different plate heights each get a roof at their
//! own plate (a one-story garage beside a two-story house; DECISIONS 118).
//! A room in any other group goes with the other rooms of that group, whatever
//! its plate height, and the group is roofed as one integrated system at the
//! tallest plate of its rooms.

/// One room as the roof builder sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupedRoom {
    /// The room's Roof Group number (0 is the default group).
    pub group: u32,
    /// The plate height beside the room (its lowest exterior wall), inches;
    /// `None` for a room with no exterior wall, which goes with the tallest.
    pub level: Option<f64>,
}

/// The rooms that share one roof.
#[derive(Debug, Clone, PartialEq)]
pub struct RoofAssignment {
    /// The Roof Group the rooms belong to.
    pub group: u32,
    /// Plate height of the roof, inches.
    pub level: f64,
    /// Indices into the room list, in the order given.
    pub rooms: Vec<usize>,
}

/// Splits `rooms` into the sets that are roofed together, tallest roof first
/// (ties by group number). `tallest` is the plate height given to a room with
/// no level of its own; levels within `tolerance` of each other are one level.
///
/// With every room in group 0 this is the plate-height grouping that wings
/// have always used.
pub fn assign_roof_groups(
    rooms: &[GroupedRoom],
    tallest: f64,
    tolerance: f64,
) -> Vec<RoofAssignment> {
    let level_of = |r: &GroupedRoom| r.level.unwrap_or(tallest);
    let mut out: Vec<RoofAssignment> = Vec::new();
    // Group 0: one assignment per plate height.
    for (i, r) in rooms.iter().enumerate().filter(|(_, r)| r.group == 0) {
        let l = level_of(r);
        match out
            .iter_mut()
            .find(|a| a.group == 0 && (a.level - l).abs() < tolerance)
        {
            Some(a) => a.rooms.push(i),
            None => out.push(RoofAssignment {
                group: 0,
                level: l,
                rooms: vec![i],
            }),
        }
    }
    // Every other group: one assignment at the tallest plate of its rooms.
    for (i, r) in rooms.iter().enumerate().filter(|(_, r)| r.group != 0) {
        let l = level_of(r);
        match out.iter_mut().find(|a| a.group == r.group) {
            Some(a) => {
                a.rooms.push(i);
                a.level = a.level.max(l);
            }
            None => out.push(RoofAssignment {
                group: r.group,
                level: l,
                rooms: vec![i],
            }),
        }
    }
    out.sort_by(|a, b| b.level.total_cmp(&a.level).then(a.group.cmp(&b.group)));
    out
}

/// Do the rooms belong to more than one roof group, so that the plan is
/// roofed as separate buildings whatever its plate heights?
pub fn has_groups(rooms: &[GroupedRoom]) -> bool {
    rooms.iter().any(|r| r.group != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_roof_with_specs, EdgeRoofSpec, RoofPlane};
    use plan_core::Point;

    fn room(group: u32, level: f64) -> GroupedRoom {
        GroupedRoom {
            group,
            level: Some(level),
        }
    }

    #[test]
    fn rooms_in_group_zero_keep_the_plate_height_rule() {
        let rooms = [room(0, 96.0), room(0, 108.0), room(0, 96.0), room(0, 108.0)];
        let a = assign_roof_groups(&rooms, 108.0, 1.0);
        assert_eq!(a.len(), 2);
        assert_eq!((a[0].level, a[0].rooms.clone()), (108.0, vec![1, 3]));
        assert_eq!((a[1].level, a[1].rooms.clone()), (96.0, vec![0, 2]));
        assert!(!has_groups(&rooms));
    }

    #[test]
    fn a_group_ignores_the_plate_heights_of_its_rooms() {
        // The garage is group 1 although its two rooms stand at different
        // plates; the house rooms stay in group 0 at one plate.
        let rooms = [
            room(0, 108.0),
            room(1, 96.0),
            room(0, 108.0),
            room(1, 108.0),
        ];
        let a = assign_roof_groups(&rooms, 108.0, 1.0);
        assert_eq!(a.len(), 2);
        let g1 = a.iter().find(|x| x.group == 1).unwrap();
        assert_eq!(g1.rooms, vec![1, 3]);
        assert_eq!(g1.level, 108.0, "the tallest plate of the group");
        let g0 = a.iter().find(|x| x.group == 0).unwrap();
        assert_eq!(g0.rooms, vec![0, 2]);
        assert!(has_groups(&rooms));
    }

    #[test]
    fn two_groups_at_one_plate_are_two_buildings() {
        let rooms = [room(0, 96.0), room(2, 96.0), room(3, 96.0)];
        let a = assign_roof_groups(&rooms, 96.0, 1.0);
        assert_eq!(a.len(), 3);
        // Equal levels sort by group number.
        assert_eq!(a.iter().map(|x| x.group).collect::<Vec<_>>(), vec![0, 2, 3]);
    }

    #[test]
    fn a_room_without_a_wall_goes_with_the_tallest() {
        let rooms = [
            room(0, 96.0),
            GroupedRoom {
                group: 0,
                level: None,
            },
        ];
        let a = assign_roof_groups(&rooms, 120.0, 1.0);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].rooms, vec![1]);
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    fn top(planes: &[RoofPlane]) -> f64 {
        planes
            .iter()
            .flat_map(|p| p.polygon3d.iter())
            .fold(f64::MIN, |m, v| m.max(v[1]))
    }

    /// Roof groups keep one building's roof from shaping the other's: the
    /// two buildings roofed apart keep the ridge each would have alone, while
    /// one roof over both bounding footprints would put a single ridge across
    /// the span.
    #[test]
    fn separate_groups_do_not_influence_each_others_roofs() {
        let house = rect(0.0, 0.0, 480.0, 288.0);
        let garage = rect(600.0, 0.0, 840.0, 240.0);
        let specs = vec![
            EdgeRoofSpec {
                overhang: 0.0,
                ..EdgeRoofSpec::default()
            };
            4
        ];
        let house_alone = build_roof_with_specs(&house, &specs, 100.0);
        let garage_alone = build_roof_with_specs(&garage, &specs, 100.0);
        // Alone, the 288" deep house rises to 144 / 12 * 8 = 96" over the
        // eave and the 240" deep garage to 80".
        assert!((top(&house_alone.planes) - 196.0).abs() < 1e-6);
        assert!((top(&garage_alone.planes) - 180.0).abs() < 1e-6);
        // Joined into one footprint (a single outline around both), the
        // garage's roof is no longer its own.
        let both = rect(0.0, 0.0, 840.0, 288.0);
        let joined = build_roof_with_specs(&both, &specs, 100.0);
        assert!((top(&joined.planes) - 196.0).abs() < 1e-6);
        let garage_top_in_joined = joined
            .planes
            .iter()
            .filter(|p| p.plan_polygon().iter().any(|q| q.x > 700.0))
            .map(|p| top(std::slice::from_ref(p)))
            .fold(f64::MIN, f64::max);
        assert!(garage_top_in_joined > top(&garage_alone.planes) + 1.0);
    }
}
