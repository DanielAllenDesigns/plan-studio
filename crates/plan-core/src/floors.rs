//! Floor commands for Chief parity (`docs/parity/rooms-floors.md`, R-55..R-68):
//! build/insert/delete/exchange floors, build a foundation, floor naming.
//!
//! Floors are stored bottom to top in `Project::floors`. A foundation floor
//! (if any) is index 0 and an attic (if any) is last. Structural commands
//! keep the cameras' floor indices and the floor elevations consistent.

use crate::geometry::Point;
use crate::model::{Floor, Id, Project, Wall, WallKind};
use crate::symbols::PlacedSymbol;
use crate::walls::WallFlags;
use serde::{Deserialize, Serialize};

/// Floor platform thickness added to a floor's ceiling height to get the
/// floor-to-floor rise (R-57, R-71), inches.
pub const FLOOR_PLATFORM_THICKNESS: f64 = 10.25;
/// Thickness of a "Foundation-8" wall, inches.
pub const FOUNDATION_WALL_THICKNESS: f64 = 8.0;
/// Name of the default foundation wall type.
pub const FOUNDATION_WALL_TYPE: &str = "Foundation-8";
/// Height of the thickened edge standing in for a monolithic slab, inches.
pub const SLAB_EDGE_HEIGHT: f64 = 12.0;
/// Size of a pier pad, inches.
pub const PIER_SIZE: f64 = 12.0;

/// What a floor is (R-55).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FloorKind {
    Foundation,
    #[default]
    Normal,
    Attic,
}

/// Foundation types (R-61, R-62).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FoundationKind {
    /// Stem walls of the given height under the exterior walls.
    StemWall { height: f64 },
    /// Slab on grade: a thickened edge of [`SLAB_EDGE_HEIGHT`] under the exterior walls.
    MonolithicSlab,
    /// Square pier pads (symbols) at the exterior wall corners; no walls.
    Pier,
}

/// "1st Floor", "2nd Floor", "3rd Floor", "4th Floor", ... for `n >= 1`.
pub fn ordinal_floor_name(n: usize) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix} Floor")
}

impl Project {
    /// The automatic name of floor `idx` given the current stack:
    /// "Foundation", "Attic" or the ordinal among the normal floors.
    pub fn auto_floor_name(&self, idx: usize) -> String {
        match self.floors[idx].kind {
            FloorKind::Foundation => "Foundation".to_string(),
            FloorKind::Attic => "Attic".to_string(),
            FloorKind::Normal => {
                let n = self.floors[..=idx]
                    .iter()
                    .filter(|f| f.kind == FloorKind::Normal)
                    .count();
                ordinal_floor_name(n)
            }
        }
    }

    /// Which floors still carry their automatic name (custom names are kept
    /// by [`Project::renumber_floors`]).
    fn auto_named(&self) -> Vec<bool> {
        (0..self.floors.len())
            .map(|i| self.floors[i].name == self.auto_floor_name(i))
            .collect()
    }

    /// Give the floors that had automatic names their new automatic name.
    fn renumber_floors(&mut self, was_auto: &[bool], order: &[usize]) {
        // `order[new_index]` is the old index of that floor, if it had one.
        for (i, &old) in order.iter().enumerate() {
            if old != usize::MAX && was_auto.get(old).copied().unwrap_or(false) {
                self.floors[i].name = self.auto_floor_name(i);
            }
        }
    }

    /// Recompute floor elevations from the first normal floor (whose
    /// elevation is kept): each floor sits one ceiling height plus the
    /// platform thickness above the one below; a foundation floor hangs below
    /// by its own height (R-71).
    pub fn restack_floors(&mut self) {
        let Some(base) = self
            .floors
            .iter()
            .position(|f| f.kind != FloorKind::Foundation)
        else {
            return;
        };
        for i in (0..base).rev() {
            self.floors[i].elevation = self.floors[i + 1].elevation - self.floors[i].ceiling_height;
        }
        for i in base + 1..self.floors.len() {
            self.floors[i].elevation = self.floors[i - 1].elevation
                + self.floors[i - 1].ceiling_height
                + FLOOR_PLATFORM_THICKNESS;
        }
    }

    /// Index of the highest normal floor.
    fn top_normal_floor(&self) -> Option<usize> {
        self.floors
            .iter()
            .rposition(|f| f.kind == FloorKind::Normal)
    }

    /// Shift camera floor indices at or above `from` by `delta`.
    fn shift_cameras(&mut self, from: usize, delta: isize) {
        for c in self.cameras.iter_mut().filter(|c| c.floor >= from) {
            c.floor = (c.floor as isize + delta).max(0) as usize;
        }
    }

    /// Build New Floor (R-59): add a floor above the highest normal floor
    /// (below the attic, if any). With `copy_exterior_walls`, the exterior
    /// walls of the floor below are copied with their doors and windows and
    /// fresh ids. Returns the new floor's index.
    pub fn build_new_floor(&mut self, copy_exterior_walls: bool) -> usize {
        let src = self.top_normal_floor();
        let at = src.map_or(self.floors.len(), |i| i + 1);
        let was_auto = self.auto_named();
        let ceiling = src.map_or(crate::model::DEFAULT_CEILING_HEIGHT, |i| {
            self.floors[i].ceiling_height
        });
        let mut floor = Floor::new("", 0.0);
        floor.ceiling_height = ceiling;
        if copy_exterior_walls {
            if let Some(i) = src {
                self.copy_exterior_into(i, &mut floor);
            }
        }
        self.floors.insert(at, floor);
        self.shift_cameras(at, 1);
        let order: Vec<usize> = (0..self.floors.len())
            .map(|n| match n.cmp(&at) {
                std::cmp::Ordering::Less => n,
                std::cmp::Ordering::Equal => usize::MAX,
                std::cmp::Ordering::Greater => n - 1,
            })
            .collect();
        self.floors[at].name = self.auto_floor_name(at);
        self.renumber_floors(&was_auto, &order);
        self.restack_floors();
        at
    }

    /// Copy the exterior walls (and their openings) of floor `src` into
    /// `target` with fresh ids.
    fn copy_exterior_into(&mut self, src: usize, target: &mut Floor) {
        let walls: Vec<Wall> = self.floors[src]
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior && !w.flags.foundation)
            .cloned()
            .collect();
        for mut w in walls {
            let old = w.id;
            w.id = self.alloc_id();
            let hosted: Vec<_> = self.floors[src]
                .openings
                .iter()
                .filter(|o| o.wall_id == old)
                .cloned()
                .collect();
            for mut o in hosted {
                o.id = self.alloc_id();
                o.wall_id = w.id;
                target.openings.push(o);
            }
            target.walls.push(w);
        }
    }

    /// Insert an empty floor directly above floor `idx` (R-60) and renumber.
    /// Returns the new floor's index (`idx + 1`), or `None` if `idx` is out of
    /// range or is the attic (nothing goes above it).
    pub fn insert_floor_above(&mut self, idx: usize) -> Option<usize> {
        if idx >= self.floors.len() || self.floors[idx].kind == FloorKind::Attic {
            return None;
        }
        let at = idx + 1;
        let was_auto = self.auto_named();
        let mut floor = Floor::new("", 0.0);
        floor.ceiling_height = self.floors[idx].ceiling_height;
        self.floors.insert(at, floor);
        self.shift_cameras(at, 1);
        let order: Vec<usize> = (0..self.floors.len())
            .map(|n| match n.cmp(&at) {
                std::cmp::Ordering::Less => n,
                std::cmp::Ordering::Equal => usize::MAX,
                std::cmp::Ordering::Greater => n - 1,
            })
            .collect();
        self.floors[at].name = self.auto_floor_name(at);
        self.renumber_floors(&was_auto, &order);
        self.restack_floors();
        Some(at)
    }

    /// Delete a floor (R-60, R-63) with its cameras. Refuses to delete the
    /// last remaining floor or an out-of-range index.
    pub fn delete_floor(&mut self, idx: usize) -> bool {
        if self.floors.len() <= 1 || idx >= self.floors.len() {
            return false;
        }
        let was_auto = self.auto_named();
        self.floors.remove(idx);
        self.cameras.retain(|c| c.floor != idx);
        self.shift_cameras(idx + 1, -1);
        let order: Vec<usize> = (0..self.floors.len())
            .map(|n| if n < idx { n } else { n + 1 })
            .collect();
        self.renumber_floors(&was_auto, &order);
        self.restack_floors();
        true
    }

    /// Exchange With Floor Above/Below (R-64): swap the contents of floors
    /// `a` and `b` (walls, openings, dimensions, CAD, room names, symbols,
    /// cabinets, stairs, groups, ceiling height and cameras). Names, kinds and
    /// numbers stay with their positions. Returns `false` for equal or
    /// out-of-range indices.
    pub fn exchange_floors(&mut self, a: usize, b: usize) -> bool {
        let n = self.floors.len();
        if a == b || a >= n || b >= n {
            return false;
        }
        let (lo, hi) = (a.min(b), a.max(b));
        let (left, right) = self.floors.split_at_mut(hi);
        let (fa, fb) = (&mut left[lo], &mut right[0]);
        std::mem::swap(&mut fa.walls, &mut fb.walls);
        std::mem::swap(&mut fa.openings, &mut fb.openings);
        std::mem::swap(&mut fa.dimensions, &mut fb.dimensions);
        std::mem::swap(&mut fa.cad, &mut fb.cad);
        std::mem::swap(&mut fa.room_names, &mut fb.room_names);
        std::mem::swap(&mut fa.symbols, &mut fb.symbols);
        std::mem::swap(&mut fa.cabinets, &mut fb.cabinets);
        std::mem::swap(&mut fa.stairs, &mut fb.stairs);
        std::mem::swap(&mut fa.groups, &mut fb.groups);
        std::mem::swap(&mut fa.ceiling_height, &mut fb.ceiling_height);
        for c in self.cameras.iter_mut() {
            if c.floor == a {
                c.floor = b;
            } else if c.floor == b {
                c.floor = a;
            }
        }
        self.restack_floors();
        true
    }

    /// Build Foundation (R-61, R-62): create (or rebuild) the Foundation
    /// floor at index 0 with foundation walls under the exterior walls of the
    /// first normal floor. Foundation walls share the exterior walls'
    /// centerlines so corners keep joining. Returns the foundation floor's
    /// index (always 0). Existing floors move up by one when a foundation is
    /// newly created.
    pub fn build_foundation(&mut self, kind: FoundationKind) -> usize {
        let has_foundation = self
            .floors
            .first()
            .is_some_and(|f| f.kind == FloorKind::Foundation);
        let first_normal = self
            .floors
            .iter()
            .position(|f| f.kind != FloorKind::Foundation)
            .unwrap_or(0);
        let exterior: Vec<Wall> = self.floors[first_normal]
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior && !w.flags.foundation && !w.flags.invisible)
            .cloned()
            .collect();
        let height = match kind {
            FoundationKind::StemWall { height } => height.max(1.0),
            FoundationKind::MonolithicSlab => SLAB_EDGE_HEIGHT,
            FoundationKind::Pier => 24.0,
        };
        let mut floor = Floor::new("Foundation", -height);
        floor.kind = FloorKind::Foundation;
        floor.ceiling_height = height;
        if !matches!(kind, FoundationKind::Pier) {
            for src in &exterior {
                let id = self.alloc_id();
                floor.walls.push(Wall {
                    id,
                    thickness: FOUNDATION_WALL_THICKNESS,
                    height,
                    flags: WallFlags {
                        foundation: true,
                        ..WallFlags::default()
                    },
                    wall_type: Some(FOUNDATION_WALL_TYPE.to_string()),
                    curve: src.curve,
                    ..Wall::new(
                        src.start,
                        src.end,
                        FOUNDATION_WALL_THICKNESS,
                        height,
                        WallKind::Exterior,
                    )
                });
            }
        } else {
            let mut corners: Vec<Point> = Vec::new();
            for w in &exterior {
                for p in [w.start, w.end] {
                    if !corners.iter().any(|c| c.dist(p) < 1.0) {
                        corners.push(p);
                    }
                }
            }
            for p in corners {
                let id = self.alloc_id();
                let mut s = PlacedSymbol::new("Square Pier", p, PIER_SIZE, PIER_SIZE, height);
                s.id = id;
                floor.symbols.push(s);
            }
        }
        if has_foundation {
            self.floors[0] = floor;
        } else {
            self.floors.insert(0, floor);
            self.shift_cameras(0, 1);
        }
        self.restack_floors();
        0
    }

    /// Ids of the foundation walls on floor `idx`.
    pub fn foundation_wall_ids(&self, idx: usize) -> Vec<Id> {
        self.floors[idx]
            .walls
            .iter()
            .filter(|w| w.flags.foundation)
            .map(|w| w.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{CameraKind, CameraObject};
    use crate::model::{OpeningKind, DEFAULT_CEILING_HEIGHT};

    fn house() -> Project {
        let mut p = Project::new("h");
        let k = WallKind::Exterior;
        let pts = [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)];
        let mut first = 0;
        for i in 0..4 {
            let a = pts[i];
            let b = pts[(i + 1) % 4];
            let id = p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.5,
                DEFAULT_CEILING_HEIGHT,
                k,
            );
            if i == 0 {
                first = id;
            }
        }
        p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            4.5,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        );
        p.add_opening(0, first, 100.0, OpeningKind::Door).unwrap();
        p
    }

    #[test]
    fn naming() {
        assert_eq!(ordinal_floor_name(1), "1st Floor");
        assert_eq!(ordinal_floor_name(2), "2nd Floor");
        assert_eq!(ordinal_floor_name(3), "3rd Floor");
        assert_eq!(ordinal_floor_name(4), "4th Floor");
        assert_eq!(ordinal_floor_name(11), "11th Floor");
        assert_eq!(ordinal_floor_name(22), "22nd Floor");
    }

    #[test]
    fn build_new_floor_copies_exterior_walls() {
        let mut p = house();
        let up = p.build_new_floor(true);
        assert_eq!(up, 1);
        let f = &p.floors[1];
        assert_eq!(f.name, "2nd Floor");
        assert_eq!(f.walls.len(), 4, "interior wall must not be copied");
        assert!(f.walls.iter().all(|w| w.kind == WallKind::Exterior));
        assert_eq!(f.openings.len(), 1);
        // Fresh ids; openings point at the copied wall.
        let ids0: Vec<Id> = p.floors[0].walls.iter().map(|w| w.id).collect();
        assert!(f.walls.iter().all(|w| !ids0.contains(&w.id)));
        assert!(f.openings.iter().all(|o| f.wall(o.wall_id).is_some()));
        assert!((f.elevation - (DEFAULT_CEILING_HEIGHT + FLOOR_PLATFORM_THICKNESS)).abs() < 1e-9);
        // Empty floor option.
        let third = p.build_new_floor(false);
        assert_eq!(third, 2);
        assert!(p.floors[2].walls.is_empty());
        assert_eq!(p.floors[2].name, "3rd Floor");
    }

    #[test]
    fn insert_delete_exchange() {
        let mut p = house();
        p.build_new_floor(false);
        p.floors[1].name = "Loft".into();
        let cam = CameraObject::new(CameraKind::FullCamera, Point::new(5.0, 5.0), 0.0, "c", 1);
        let cid = p.add_camera(cam);
        // Insert above the first floor: the loft moves to index 2, name kept.
        assert_eq!(p.insert_floor_above(0), Some(1));
        assert_eq!(p.floors[1].name, "2nd Floor");
        assert_eq!(p.floors[2].name, "Loft");
        assert_eq!(p.cameras.iter().find(|c| c.id == cid).unwrap().floor, 2);
        // Exchange moves contents (and cameras) but not names.
        assert!(p.exchange_floors(0, 1));
        assert!(p.floors[0].walls.is_empty());
        assert_eq!(p.floors[1].walls.len(), 5);
        assert_eq!(p.floors[0].name, "1st Floor");
        assert!(!p.exchange_floors(1, 1) && !p.exchange_floors(0, 9));
        // Delete: the camera on the deleted floor goes, later ones shift down.
        assert!(p.delete_floor(1));
        assert_eq!(p.floors.len(), 2);
        assert_eq!(p.cameras.iter().find(|c| c.id == cid).unwrap().floor, 1);
        assert!(p.delete_floor(1));
        assert!(p.cameras.is_empty());
        // The last floor cannot be deleted.
        assert!(!p.delete_floor(0));
        assert!(!p.delete_floor(5));
        assert_eq!(p.floors.len(), 1);
    }

    #[test]
    fn build_foundation_under_exterior_walls_only() {
        let mut p = house();
        p.floors[0].name = "1st Floor".into();
        let cid = p.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::ZERO,
            0.0,
            "c",
            0,
        ));
        let idx = p.build_foundation(FoundationKind::StemWall { height: 36.0 });
        assert_eq!(idx, 0);
        assert_eq!(p.floors.len(), 2);
        let f = &p.floors[0];
        assert_eq!(f.name, "Foundation");
        assert_eq!(f.kind, FloorKind::Foundation);
        assert_eq!(f.walls.len(), 4);
        for w in &f.walls {
            assert!(w.flags.foundation);
            assert_eq!(w.wall_type.as_deref(), Some("Foundation-8"));
            assert_eq!(w.height, 36.0);
            assert_eq!(w.thickness, 8.0);
        }
        // Same centerlines as the exterior walls above; the partition has none.
        for w in p.floors[1]
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior)
        {
            assert!(f
                .walls
                .iter()
                .any(|fw| fw.start == w.start && fw.end == w.end));
        }
        assert_eq!(p.floors[1].name, "1st Floor");
        assert_eq!(p.floors[0].elevation, -36.0);
        assert_eq!(p.floors[1].elevation, 0.0);
        assert_eq!(p.cameras.iter().find(|c| c.id == cid).unwrap().floor, 1);
        // Rebuilding replaces rather than stacking another foundation.
        p.build_foundation(FoundationKind::MonolithicSlab);
        assert_eq!(p.floors.len(), 2);
        assert_eq!(p.floors[0].walls[0].height, SLAB_EDGE_HEIGHT);
        // Floors above a foundation are numbered from 1st.
        let up = p.build_new_floor(true);
        assert_eq!(p.floors[up].name, "2nd Floor");
        assert_eq!(p.foundation_wall_ids(0).len(), 4);
        // Piers: pads at the corners, no walls.
        p.build_foundation(FoundationKind::Pier);
        assert!(p.floors[0].walls.is_empty());
        assert_eq!(p.floors[0].symbols.len(), 4);
    }
}
