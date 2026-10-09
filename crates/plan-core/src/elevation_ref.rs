//! Elevation references (reference manual "Elevation References", p. 21,
//! parity rows R-89 and R-90): what the height of a cabinet, a device, a
//! symbol, an opening, a soffit or a slab is measured from.
//!
//! An object's height field keeps its meaning as the distance typed in its
//! dialog. [`ElevationRef`] says what that distance is measured from
//! ([`ElevationBase`]) and to which edge of the object ([`ElevationEdge`]).
//! The default reference, From Floor to Bottom, is what every height field
//! has always meant, so an object with no stored reference does not move; the
//! app only runs the resolver for objects that chose something else.
//!
//! [`resolve`] gives the absolute elevation (inches above Z = 0, the scale
//! `Floor::elevation` is on) of the surface a reference measures from at one
//! plan point; [`ElevationRef::bottom`] turns the typed distance into the
//! bottom of the object. Terrain and roof surfaces are stored by crates that
//! depend on this one, so the caller supplies them through [`Surfaces`].
//!
//! # Where each reference measures from
//!
//! * Absolute: Z = 0.
//! * From Floor: the top of the floor structure at the point. In a room that
//!   is the floor's elevation plus the room's Floor Height offset (a split
//!   level or a garage drop; an absolute offset counts from Z = 0); outside
//!   every room it is the floor's own elevation, the default Floor Height of
//!   the level.
//! * From Finished Floor: From Floor plus the floor finish of the room (the
//!   floor's default finish outside a room).
//! * From Terrain: the terrain surface at the point; with no terrain, From
//!   Floor.
//! * From Ceiling: the ceiling surface at the point: the room's floor plus its
//!   own ceiling height, else the floor's default ceiling height.
//! * From Roof: the roof surface at the point; with no roof, From Ceiling.

use crate::geometry::Point;
use crate::model::Project;
use crate::rooms::{detect_rooms, Room};
use serde::{Deserialize, Serialize};

/// What a height is measured from (the six references of the manual).
pub use crate::arch_block::ElevationRef as ElevationBase;

/// To which edge of the object the typed distance reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ElevationEdge {
    /// The bottom of the object is this far from the reference.
    #[default]
    ToBottom,
    /// The top of the object is this far from the reference.
    ToTop,
}

impl ElevationEdge {
    pub const ALL: [ElevationEdge; 2] = [ElevationEdge::ToBottom, ElevationEdge::ToTop];

    pub fn name(self) -> &'static str {
        match self {
            ElevationEdge::ToBottom => "To Bottom",
            ElevationEdge::ToTop => "To Top",
        }
    }
}

/// The reference of one object's height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ElevationRef {
    pub base: ElevationBase,
    pub edge: ElevationEdge,
}

impl ElevationRef {
    pub fn new(base: ElevationBase, edge: ElevationEdge) -> Self {
        Self { base, edge }
    }

    /// The reference every height field had before references existed: From
    /// Floor, to the bottom. The app leaves such an object untouched.
    pub fn is_legacy(&self) -> bool {
        *self == Self::default()
    }

    /// The absolute elevation of the bottom of an object `height` tall whose
    /// typed distance is `value`, with the reference surface at `datum`.
    pub fn bottom(&self, datum: f64, value: f64, height: f64) -> f64 {
        match self.edge {
            ElevationEdge::ToBottom => datum + value,
            ElevationEdge::ToTop => datum + value - height,
        }
    }

    /// The typed distance that puts the bottom at `bottom` (the inverse of
    /// [`ElevationRef::bottom`]), for converting a value when the reference
    /// changes and the object must not move.
    pub fn value_for(&self, datum: f64, bottom: f64, height: f64) -> f64 {
        match self.edge {
            ElevationEdge::ToBottom => bottom - datum,
            ElevationEdge::ToTop => bottom + height - datum,
        }
    }
}

/// The surfaces other crates own: the terrain and the roof.
pub trait Surfaces {
    /// Absolute elevation of the terrain surface at `at` on level `floor`.
    fn terrain_at(&self, floor: usize, at: Point) -> Option<f64>;
    /// Absolute elevation of the underside of the roof at `at` over `floor`.
    fn roof_at(&self, floor: usize, at: Point) -> Option<f64>;
}

/// No terrain and no roof.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSurfaces;

impl Surfaces for NoSurfaces {
    fn terrain_at(&self, _floor: usize, _at: Point) -> Option<f64> {
        None
    }
    fn roof_at(&self, _floor: usize, _at: Point) -> Option<f64> {
        None
    }
}

/// The reference surfaces of one level, with its rooms found once.
pub struct Resolver<'a> {
    project: &'a Project,
    floor: usize,
    rooms: Vec<Room>,
    surfaces: &'a dyn Surfaces,
}

impl<'a> Resolver<'a> {
    /// A resolver for level `floor` (clamped to the last level).
    pub fn new(project: &'a Project, floor: usize, surfaces: &'a dyn Surfaces) -> Self {
        let floor = floor.min(project.floors.len().saturating_sub(1));
        let rooms = project
            .floors
            .get(floor)
            .map(|f| detect_rooms(&f.walls, 0.5))
            .unwrap_or_default();
        Self {
            project,
            floor,
            rooms,
            surfaces,
        }
    }

    fn room_at(&self, at: Point) -> Option<&Room> {
        self.rooms.iter().find(|r| r.contains(at))
    }

    /// The top of the floor structure at `at`.
    fn floor_top(&self, at: Point) -> f64 {
        let Some(f) = self.project.floors.get(self.floor) else {
            return 0.0;
        };
        let named = self.room_at(at).and_then(|r| r.name_entry(&f.room_names));
        let abs = named
            .and_then(|n| n.misc.as_ref())
            .is_some_and(|m| m.floor_height_absolute);
        let offset = named.map_or(0.0, |n| n.floor_height_offset);
        if abs {
            // Measured from Z = 0 rather than from the level.
            offset
        } else {
            f.elevation + offset
        }
    }

    /// The surface of the finished floor at `at`.
    fn finished_floor(&self, at: Point) -> f64 {
        let Some(f) = self.project.floors.get(self.floor) else {
            return 0.0;
        };
        let named = self.room_at(at).and_then(|r| r.name_entry(&f.room_names));
        let finish = named
            .and_then(|n| n.misc.as_ref())
            .map_or(f.settings.floor_finish_thickness, |m| {
                m.floor_finish_thickness
            });
        self.floor_top(at) + finish.max(0.0)
    }

    /// The ceiling surface at `at`.
    fn ceiling(&self, at: Point) -> f64 {
        let Some(f) = self.project.floors.get(self.floor) else {
            return 0.0;
        };
        let named = self.room_at(at).and_then(|r| r.name_entry(&f.room_names));
        let height = named
            .and_then(|n| n.ceiling_height)
            .unwrap_or(f.ceiling_height);
        let absolute = named
            .and_then(|n| n.misc.as_ref())
            .is_some_and(|m| m.ceiling_height_absolute);
        if absolute {
            height
        } else {
            self.floor_top(at) + height
        }
    }

    /// The absolute elevation of what `base` measures from at `at`.
    pub fn datum(&self, at: Point, base: ElevationBase) -> f64 {
        match base {
            ElevationBase::Absolute => 0.0,
            ElevationBase::FromFloor => self.floor_top(at),
            ElevationBase::FromFinishedFloor => self.finished_floor(at),
            ElevationBase::FromTerrain => self
                .surfaces
                .terrain_at(self.floor, at)
                .unwrap_or_else(|| self.floor_top(at)),
            ElevationBase::FromCeiling => self.ceiling(at),
            ElevationBase::FromRoof => self
                .surfaces
                .roof_at(self.floor, at)
                .unwrap_or_else(|| self.ceiling(at)),
        }
    }
}

/// The absolute elevation of what `base` measures from at plan point `at` of
/// level `floor` (inches). Builds a [`Resolver`]; use that directly to resolve
/// many points of one level.
pub fn resolve(
    project: &Project,
    floor: usize,
    at: Point,
    base: ElevationBase,
    surfaces: &dyn Surfaces,
) -> f64 {
    Resolver::new(project, floor, surfaces).datum(at, base)
}

/// The elevation of the bottom of an object above its level's own elevation:
/// the number the 3D builders add `Floor::elevation` to. `value` is the typed
/// distance and `height` the object's height.
pub fn local_bottom(
    resolver: &Resolver<'_>,
    r: ElevationRef,
    at: Point,
    value: f64,
    height: f64,
) -> f64 {
    let level = resolver
        .project
        .floors
        .get(resolver.floor)
        .map_or(0.0, |f| f.elevation);
    r.bottom(resolver.datum(at, r.base), value, height) - level
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extras::RoomMisc;
    use crate::model::RoomName;
    use crate::model::WallKind;

    /// A 20 x 12 ft house split into a 10 ft main room and a 10 ft sunken one.
    fn split_level() -> Project {
        let mut p = Project::new("split");
        let h = 96.0;
        let pts = [(0.0, 0.0), (240.0, 0.0), (240.0, 144.0), (0.0, 144.0)];
        for i in 0..4 {
            let a = pts[i];
            let b = pts[(i + 1) % 4];
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                h,
                WallKind::Exterior,
            );
        }
        // The divider at x = 120.
        p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 144.0),
            4.0,
            h,
            WallKind::Interior,
        );
        p.floors[0].elevation = 12.0;
        p.floors[0].ceiling_height = 108.0;
        let high = RoomName::new(Point::new(60.0, 72.0), "Living", "Living Room");
        let mut low = RoomName::new(Point::new(180.0, 72.0), "Sunken", "Family Room");
        low.floor_height_offset = -24.0;
        low.ceiling_height = Some(132.0);
        low.misc = Some(RoomMisc {
            floor_finish_thickness: 0.0,
            ..RoomMisc::default()
        });
        p.floors[0].room_names = vec![high, low];
        p
    }

    /// Terrain that rises one inch for every ten inches east of x = 0, over a
    /// base of -6.
    struct Slope;
    impl Surfaces for Slope {
        fn terrain_at(&self, _floor: usize, at: Point) -> Option<f64> {
            Some(-6.0 + at.x / 10.0)
        }
        fn roof_at(&self, _floor: usize, at: Point) -> Option<f64> {
            Some(150.0 + at.x * 0.25)
        }
    }

    #[test]
    fn the_default_reference_is_the_legacy_one() {
        let r = ElevationRef::default();
        assert!(r.is_legacy());
        assert_eq!(r.base, ElevationBase::FromFloor);
        assert_eq!(r.edge, ElevationEdge::ToBottom);
        assert!(
            !ElevationRef::new(ElevationBase::FromCeiling, ElevationEdge::ToBottom).is_legacy()
        );
        // Old files and empty objects read as the default.
        let back: ElevationRef = serde_json::from_str("{}").unwrap();
        assert!(back.is_legacy());
        let s = serde_json::to_string(&ElevationRef::new(
            ElevationBase::FromRoof,
            ElevationEdge::ToTop,
        ))
        .unwrap();
        let back: ElevationRef = serde_json::from_str(&s).unwrap();
        assert_eq!(back.base, ElevationBase::FromRoof);
        assert_eq!(back.edge, ElevationEdge::ToTop);
    }

    #[test]
    fn absolute_and_floor_references_on_a_split_level_floor() {
        let p = split_level();
        let high = Point::new(60.0, 72.0);
        let sunken = Point::new(180.0, 72.0);
        let nowhere = Point::new(-500.0, -500.0);
        let at = |pt, base| resolve(&p, 0, pt, base, &NoSurfaces);
        assert_eq!(at(high, ElevationBase::Absolute), 0.0);
        assert_eq!(at(high, ElevationBase::FromFloor), 12.0);
        assert_eq!(at(sunken, ElevationBase::FromFloor), -12.0);
        // Outside every room it is the level's own Floor Height.
        assert_eq!(at(nowhere, ElevationBase::FromFloor), 12.0);
        // The main room has the floor's default finish, the sunken one none.
        let finish = p.floors[0].settings.floor_finish_thickness;
        assert!((at(high, ElevationBase::FromFinishedFloor) - (12.0 + finish)).abs() < 1e-9);
        assert_eq!(at(sunken, ElevationBase::FromFinishedFloor), -12.0);
        assert!((at(nowhere, ElevationBase::FromFinishedFloor) - (12.0 + finish)).abs() < 1e-9);
    }

    #[test]
    fn ceiling_and_roof_follow_the_room_and_the_surfaces() {
        let p = split_level();
        let high = Point::new(60.0, 72.0);
        let sunken = Point::new(180.0, 72.0);
        let nowhere = Point::new(-500.0, -500.0);
        let c = |pt| resolve(&p, 0, pt, ElevationBase::FromCeiling, &NoSurfaces);
        assert_eq!(c(high), 12.0 + 108.0);
        // The sunken room's own ceiling height counts from its lower floor.
        assert_eq!(c(sunken), -12.0 + 132.0);
        assert_eq!(c(nowhere), 12.0 + 108.0);
        // With no roof, From Roof is the ceiling; with one, its surface.
        assert_eq!(
            resolve(&p, 0, high, ElevationBase::FromRoof, &NoSurfaces),
            c(high)
        );
        assert_eq!(
            resolve(&p, 0, high, ElevationBase::FromRoof, &Slope),
            150.0 + 60.0 * 0.25
        );
    }

    #[test]
    fn terrain_follows_a_slope_and_falls_back_to_the_floor() {
        let p = split_level();
        let west = Point::new(30.0, 72.0);
        let east = Point::new(210.0, 72.0);
        let t = |pt, s: &dyn Surfaces| resolve(&p, 0, pt, ElevationBase::FromTerrain, s);
        assert_eq!(t(west, &Slope), -6.0 + 3.0);
        assert_eq!(t(east, &Slope), -6.0 + 21.0);
        // No terrain: the default Floor Height of the point.
        assert_eq!(t(west, &NoSurfaces), 12.0);
        assert_eq!(t(east, &NoSurfaces), -12.0);
    }

    #[test]
    fn to_top_and_to_bottom_place_the_object_and_convert_back() {
        let bottom = ElevationRef::new(ElevationBase::FromCeiling, ElevationEdge::ToBottom);
        let top = ElevationRef::new(ElevationBase::FromCeiling, ElevationEdge::ToTop);
        // A 30 inch cabinet whose top is flush with a 120 inch ceiling.
        assert_eq!(top.bottom(120.0, 0.0, 30.0), 90.0);
        assert_eq!(bottom.bottom(120.0, -30.0, 30.0), 90.0);
        assert_eq!(top.value_for(120.0, 90.0, 30.0), 0.0);
        assert_eq!(bottom.value_for(120.0, 90.0, 30.0), -30.0);
        let p = split_level();
        let rs = Resolver::new(&p, 0, &NoSurfaces);
        // Above the level's own elevation (12): the ceiling is at 108.
        let local = local_bottom(&rs, top, Point::new(60.0, 72.0), 0.0, 30.0);
        assert_eq!(local, 12.0 + 108.0 - 30.0 - 12.0);
    }

    #[test]
    fn a_ceiling_height_edit_moves_a_ceiling_hung_object() {
        let mut p = split_level();
        let r = ElevationRef::new(ElevationBase::FromCeiling, ElevationEdge::ToTop);
        let at = Point::new(60.0, 72.0);
        let before = local_bottom(&Resolver::new(&p, 0, &NoSurfaces), r, at, 0.0, 30.0);
        p.floors[0].ceiling_height = 120.0;
        let after = local_bottom(&Resolver::new(&p, 0, &NoSurfaces), r, at, 0.0, 30.0);
        assert_eq!(after - before, 12.0);
    }
}
