//! Floor commands for Chief parity (`docs/parity/rooms-floors.md`, R-55..R-68):
//! build/insert/delete/exchange floors, build a foundation, floor naming.
//!
//! Floors are stored bottom to top in `Project::floors`. A foundation floor
//! (if any) is index 0 and an attic (if any) is last. Structural commands
//! keep the cameras' floor indices and the floor elevations consistent.

use crate::foundation::{Footing, FoundationLayer, Pier, Slab};
use crate::geometry::Point;
use crate::model::{Floor, Id, Project, RoomName, Wall, WallKind};
use crate::rooms::{apply_function_defaults, detect_rooms, function_defaults};
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
/// Width of a grade beam (a foundation wall on piers), inches.
pub const GRADE_BEAM_WIDTH: f64 = 12.0;
/// Height of a grade beam, inches (R-62).
pub const GRADE_BEAM_HEIGHT: f64 = 18.0;
/// Height of the piers under grade beams, inches.
pub const PIER_HEIGHT: f64 = 24.0;
/// Spacing of piers along an exterior wall, inches.
pub const PIER_SPACING: f64 = 96.0;
/// Width and depth of the footing under foundation walls, inches (R-61).
pub const WALL_FOOTING: (f64, f64) = (16.0, 8.0);
/// A foundation whose clear height (wall height less the floor platform
/// above) reaches this is a basement; a lower one is a crawl space, inches.
pub const BASEMENT_MIN_CLEAR_HEIGHT: f64 = 72.0;
/// Height of the walls of an attic floor, inches (the roof cuts them).
pub const ATTIC_WALL_HEIGHT: f64 = 48.0;
/// Ceiling height of an attic floor, inches.
pub const ATTIC_CEILING_HEIGHT: f64 = 96.0;

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
    /// Walls with Footings: foundation walls of the given height under the
    /// exterior walls (a basement or crawl space when the walls are tall).
    StemWall { height: f64 },
    /// Monolithic Slab: a slab with a thickened edge (a stem wall of
    /// [`FoundationOptions::edge_height`]) under the exterior walls.
    MonolithicSlab,
    /// Grade Beams on Piers: round piers at the corners and along the
    /// exterior walls, with grade beams spanning them.
    Pier,
}

/// What Build Foundation puts into the rooms of the foundation floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FoundationRooms {
    /// A basement when the clear height reaches
    /// [`BASEMENT_MIN_CLEAR_HEIGHT`], else a crawl space (Walls with
    /// Footings only).
    #[default]
    Auto,
    Basement,
    CrawlSpace,
    /// Leave the rooms alone.
    None,
}

/// The choices of the Build Foundation dialog (R-61, R-62).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoundationOptions {
    pub kind: FoundationKind,
    /// Footing under the foundation walls (Walls with Footings), inches;
    /// a zero width builds none.
    pub footing_width: f64,
    pub footing_depth: f64,
    /// Thickness of the slab of a Monolithic Slab, inches.
    pub slab_thickness: f64,
    /// Stem wall height of a Monolithic Slab, or the height of a grade
    /// beam, inches.
    pub edge_height: f64,
    /// Height of the piers under grade beams, inches.
    pub pier_height: f64,
    /// Spacing of piers along the exterior walls, inches.
    pub pier_spacing: f64,
    pub rooms: FoundationRooms,
}

impl FoundationOptions {
    /// The starting choices for `kind`.
    pub fn new(kind: FoundationKind) -> Self {
        Self {
            kind,
            footing_width: WALL_FOOTING.0,
            footing_depth: WALL_FOOTING.1,
            slab_thickness: crate::rooms::SLAB_FLOOR_THICKNESS,
            edge_height: match kind {
                FoundationKind::Pier => GRADE_BEAM_HEIGHT,
                _ => SLAB_EDGE_HEIGHT,
            },
            pier_height: PIER_HEIGHT,
            pier_spacing: PIER_SPACING,
            rooms: FoundationRooms::Auto,
        }
    }

    /// Height of the foundation floor: the walls (and the platform zone
    /// under the first floor's walls) from the bottom to the first floor.
    pub fn total_height(&self) -> f64 {
        match self.kind {
            FoundationKind::StemWall { height } => height.max(1.0),
            FoundationKind::MonolithicSlab => self.edge_height.max(1.0),
            FoundationKind::Pier => self.pier_height.max(1.0) + self.edge_height.max(1.0),
        }
    }
}

/// How a foundation floor was built; stored in the floor's settings so the
/// 3D view can draw the footings under its walls.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FoundationBuild {
    pub kind: FoundationKind,
    pub footing_width: f64,
    pub footing_depth: f64,
}

/// Per-floor defaults of the Floor Defaults dialog (R-56, R-58): the
/// platform thicknesses that set the floor-to-floor rise, the finish
/// thicknesses, and what rooms on the floor start with. Every floor owns one;
/// the plan defaults carry the one new floors start from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FloorSettings {
    /// Thickness of this floor's floor platform, inches. The floor sits this
    /// far above the ceiling of the floor below (with that floor's ceiling
    /// structure).
    pub floor_structure_thickness: f64,
    /// Thickness of this floor's ceiling platform, inches (adds to the rise
    /// to the floor above).
    pub ceiling_structure_thickness: f64,
    /// Finish on the floor platform, inches (R-27).
    pub floor_finish_thickness: f64,
    /// Finish under the ceiling platform, inches (R-27).
    pub ceiling_finish_thickness: f64,
    /// Room type a new room on this floor starts with ("" = the plan's first).
    pub default_room_type: String,
    /// Floor surface material of a new room ("" = none).
    pub floor_material: String,
    /// Ceiling surface material of a new room ("" = none).
    pub ceiling_material: String,
    /// Interior wall surface material of the rooms on this floor ("" =
    /// none); a room's own Wall Covering overrides it (R-36).
    pub wall_material: String,
    /// How this floor was built when it is a foundation floor (Build
    /// Foundation): footings and the like that the 3D view draws. Other
    /// floors ignore it.
    pub foundation: Option<FoundationBuild>,
}

impl Default for FloorSettings {
    fn default() -> Self {
        Self {
            floor_structure_thickness: FLOOR_PLATFORM_THICKNESS,
            ceiling_structure_thickness: 0.0,
            floor_finish_thickness: 0.75,
            ceiling_finish_thickness: 0.625,
            default_room_type: String::new(),
            floor_material: String::new(),
            ceiling_material: String::new(),
            wall_material: String::new(),
            foundation: None,
        }
    }
}

impl FloorSettings {
    /// These settings without the foundation record, for a floor that is not
    /// the foundation (new floors copy their source's settings).
    pub fn without_foundation(&self) -> FloorSettings {
        FloorSettings {
            foundation: None,
            ..self.clone()
        }
    }

    /// The floor height of Floor Defaults: ceiling height plus both
    /// platforms, inches.
    pub fn floor_height(&self, ceiling_height: f64) -> f64 {
        ceiling_height + self.ceiling_structure_thickness + self.floor_structure_thickness
    }
}

/// What a new floor copies from the floor it is built from (R-59).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DeriveFrom {
    /// A blank plan.
    Blank,
    /// The exterior walls with their doors and windows.
    #[default]
    ExteriorWalls,
    /// Every wall with its doors and windows.
    AllWalls,
}

/// Where Build New Floor puts the floor relative to its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FloorPlacement {
    #[default]
    Above,
    Below,
}

/// The Build New Floor dialog's choices (R-59).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewFloorOptions {
    /// The floor to derive from; `None` is the highest normal floor.
    pub source: Option<usize>,
    pub place: FloorPlacement,
    pub derive: DeriveFrom,
    /// Copy the source floor's room name entries.
    pub copy_rooms: bool,
    /// Copy the source floor's slab, pad and pier data.
    pub copy_foundation: bool,
    /// Ceiling height of the new floor; `None` follows the source floor.
    pub ceiling_height: Option<f64>,
    /// Floor Defaults of the new floor; `None` follows the source floor.
    pub settings: Option<FloorSettings>,
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
                + self.floors[i - 1].settings.ceiling_structure_thickness
                + self.floors[i].settings.floor_structure_thickness;
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
        let derive = if copy_exterior_walls {
            DeriveFrom::ExteriorWalls
        } else {
            DeriveFrom::Blank
        };
        self.build_new_floor_with(&NewFloorOptions {
            derive,
            ..NewFloorOptions::default()
        })
        .unwrap_or(0)
    }

    /// Build New Floor with the dialog's full set of choices (R-59): derive
    /// the exterior walls, every wall or nothing from the source floor, copy
    /// its rooms and slab data, and take the heights from the options or the
    /// source floor. Returns the new floor's index, or `None` when the
    /// placement is impossible (above the attic, below the foundation).
    pub fn build_new_floor_with(&mut self, opts: &NewFloorOptions) -> Option<usize> {
        let src = opts
            .source
            .filter(|&i| i < self.floors.len())
            .or_else(|| self.top_normal_floor());
        let at = match (opts.place, src) {
            (FloorPlacement::Above, Some(i)) if opts.source.is_some() => {
                if self.floors[i].kind == FloorKind::Attic {
                    return None;
                }
                i + 1
            }
            (FloorPlacement::Above, Some(i)) => i + 1,
            (FloorPlacement::Below, Some(i)) => {
                if self.floors[i].kind == FloorKind::Foundation {
                    return None;
                }
                i
            }
            (_, None) => self.floors.len(),
        };
        let was_auto = self.auto_named();
        let mut floor = Floor::new("", 0.0);
        floor.ceiling_height = opts.ceiling_height.unwrap_or_else(|| {
            src.map_or(crate::model::DEFAULT_CEILING_HEIGHT, |i| {
                self.floors[i].ceiling_height
            })
        });
        if let Some(s) = &opts.settings {
            floor.settings = s.without_foundation();
        } else if let Some(i) = src {
            floor.settings = self.floors[i].settings.without_foundation();
        }
        if let Some(i) = src {
            match opts.derive {
                DeriveFrom::Blank => {}
                DeriveFrom::ExteriorWalls => self.copy_walls_into(i, &mut floor, false),
                DeriveFrom::AllWalls => self.copy_walls_into(i, &mut floor, true),
            }
            if opts.copy_rooms {
                floor.room_names = self.floors[i].room_names.clone();
            }
            if opts.copy_foundation {
                floor.foundation = self.floors[i].foundation.clone();
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
        Some(at)
    }

    /// Copy the walls (and their openings) of floor `src` into `target` with
    /// fresh ids: the exterior walls, or every wall when `all`. Foundation
    /// walls are never copied.
    fn copy_walls_into(&mut self, src: usize, target: &mut Floor, all: bool) {
        let walls: Vec<Wall> = self.floors[src]
            .walls
            .iter()
            .filter(|w| (all || w.kind == WallKind::Exterior) && !w.flags.foundation)
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
        floor.settings = self.floors[idx].settings.without_foundation();
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

    /// Insert an empty floor directly below floor `idx` (R-60) and renumber.
    /// Returns the new floor's index (`idx`), or `None` if `idx` is out of
    /// range or is the foundation (nothing goes below it).
    pub fn insert_floor_below(&mut self, idx: usize) -> Option<usize> {
        if idx >= self.floors.len() || self.floors[idx].kind == FloorKind::Foundation {
            return None;
        }
        let was_auto = self.auto_named();
        let mut floor = Floor::new("", 0.0);
        floor.ceiling_height = self.floors[idx].ceiling_height;
        floor.settings = self.floors[idx].settings.without_foundation();
        self.floors.insert(idx, floor);
        self.shift_cameras(idx, 1);
        let order: Vec<usize> = (0..self.floors.len())
            .map(|n| match n.cmp(&idx) {
                std::cmp::Ordering::Less => n,
                std::cmp::Ordering::Equal => usize::MAX,
                std::cmp::Ordering::Greater => n - 1,
            })
            .collect();
        self.floors[idx].name = self.auto_floor_name(idx);
        self.renumber_floors(&was_auto, &order);
        self.restack_floors();
        Some(idx)
    }

    /// Floor Defaults (R-56, R-58): set floor `idx`'s ceiling height and
    /// settings. Walls that stood at the old ceiling height (the default wall
    /// top) follow the new one and every floor above moves by the change.
    /// Returns `false` for an out-of-range index.
    pub fn apply_floor_settings(
        &mut self,
        idx: usize,
        ceiling_height: f64,
        settings: FloorSettings,
    ) -> bool {
        let Some(floor) = self.floors.get_mut(idx) else {
            return false;
        };
        let old = floor.ceiling_height;
        if (old - ceiling_height).abs() > 1e-9 {
            for w in floor
                .walls
                .iter_mut()
                .filter(|w| !w.spec.structure.custom_top && (w.height - old).abs() < 0.01)
            {
                w.height = ceiling_height;
            }
        }
        floor.ceiling_height = ceiling_height;
        floor.settings = settings;
        self.restack_floors();
        true
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

    /// Build Foundation with the default choices of `kind` (R-61, R-62).
    pub fn build_foundation(&mut self, kind: FoundationKind) -> usize {
        self.build_foundation_with(&FoundationOptions::new(kind))
    }

    /// Build Foundation (R-61, R-62): create (or rebuild) the Foundation
    /// floor at index 0 under the exterior walls of the first normal floor.
    /// Foundation walls share the exterior walls' centerlines so corners
    /// keep joining.
    ///
    /// * Walls with Footings: foundation walls of the stated height with a
    ///   footing under them; a basement or crawl space room is made on the
    ///   floor (`opts.rooms`).
    /// * Monolithic Slab: a stem wall of `edge_height` under the exterior
    ///   walls and a slab of `slab_thickness` inside it.
    /// * Grade Beams on Piers: round piers at the wall ends and every
    ///   `pier_spacing` along them, with grade beams on top.
    ///
    /// Returns the foundation floor's index (always 0). Existing floors move
    /// up by one when a foundation is newly created.
    pub fn build_foundation_with(&mut self, opts: &FoundationOptions) -> usize {
        let has_foundation = self
            .floors
            .first()
            .is_some_and(|f| f.kind == FloorKind::Foundation);
        let first_normal = self
            .floors
            .iter()
            .position(|f| f.kind != FloorKind::Foundation)
            .unwrap_or(0);
        let platform = self.floors[first_normal].settings.floor_structure_thickness;
        let exterior: Vec<Wall> = self.floors[first_normal]
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior && !w.flags.foundation && !w.flags.invisible)
            .cloned()
            .collect();
        let height = opts.total_height();
        let mut floor = Floor::new("Foundation", -height);
        floor.kind = FloorKind::Foundation;
        floor.ceiling_height = height;
        let footed = matches!(opts.kind, FoundationKind::StemWall { .. })
            && opts.footing_width > 0.0
            && opts.footing_depth > 0.0;
        floor.settings.foundation = Some(FoundationBuild {
            kind: opts.kind,
            footing_width: if footed { opts.footing_width } else { 0.0 },
            footing_depth: if footed { opts.footing_depth } else { 0.0 },
        });
        // The first floor's platform is the ceiling of the rooms below.
        floor.settings.ceiling_structure_thickness = platform;
        let beams = matches!(opts.kind, FoundationKind::Pier);
        for src in &exterior {
            let id = self.alloc_id();
            let (thickness, wall_height, bottom) = if beams {
                (
                    GRADE_BEAM_WIDTH,
                    opts.edge_height.max(1.0),
                    opts.pier_height.max(1.0),
                )
            } else {
                (FOUNDATION_WALL_THICKNESS, height, 0.0)
            };
            floor.walls.push(Wall {
                id,
                thickness,
                height: wall_height,
                bottom_offset: bottom,
                flags: WallFlags {
                    foundation: true,
                    ..WallFlags::default()
                },
                wall_type: Some(FOUNDATION_WALL_TYPE.to_string()),
                curve: src.curve,
                ..Wall::new(
                    src.start,
                    src.end,
                    thickness,
                    wall_height,
                    WallKind::Exterior,
                )
            });
        }
        let mut layer = FoundationLayer::default();
        match opts.kind {
            FoundationKind::Pier => {
                let mut spots: Vec<Point> = Vec::new();
                let mut add = |p: Point| {
                    if !spots.iter().any(|c| c.dist(p) < 1.0) {
                        spots.push(p);
                    }
                };
                for w in &exterior {
                    let n = (w.length() / opts.pier_spacing.max(12.0)).ceil().max(1.0) as usize;
                    for i in 0..n {
                        add(Point::lerp(w.start, w.end, i as f64 / n as f64));
                    }
                    add(w.end);
                }
                for c in spots {
                    let id = self.alloc_id();
                    let mut pier = Pier::new(id, c);
                    pier.height = opts.pier_height.max(1.0);
                    pier.elevation = pier.height;
                    pier.footing = Some(Footing {
                        width: 24.0,
                        depth: 12.0,
                    });
                    layer.piers.push(pier);
                }
            }
            FoundationKind::MonolithicSlab => {
                let biggest = detect_rooms(&floor.walls, 0.5)
                    .into_iter()
                    .max_by(|a, b| a.interior_area_sq_in.total_cmp(&b.interior_area_sq_in));
                if let Some(room) = biggest {
                    let outline = if room.inner_polygon.len() >= 3 {
                        room.inner_polygon
                    } else {
                        room.polygon
                    };
                    let id = self.alloc_id();
                    let mut slab = Slab::new(id, outline);
                    slab.thickness = opts.slab_thickness.max(1.0);
                    // The slab's top meets the underside of the first
                    // floor's slab (1" under its finished floor).
                    slab.top_elevation = height - 1.0;
                    layer.slabs.push(slab);
                }
            }
            FoundationKind::StemWall { .. } => {}
        }
        layer.store(&mut floor);
        // Rooms of a basement or crawl space (R-18).
        if matches!(opts.kind, FoundationKind::StemWall { .. }) {
            let type_name = match opts.rooms {
                FoundationRooms::None => None,
                FoundationRooms::Basement => Some("Basement"),
                FoundationRooms::CrawlSpace => Some("Crawl Space"),
                FoundationRooms::Auto => Some(if height - platform >= BASEMENT_MIN_CLEAR_HEIGHT {
                    "Basement"
                } else {
                    "Crawl Space"
                }),
            };
            if let Some(t) = type_name {
                name_foundation_rooms(&mut floor, t);
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

    /// Build Attic Floor (R-68): add the attic floor above the highest normal
    /// floor, with attic walls on top of its exterior walls and an Attic room
    /// inside them, or bring the existing attic floor up to date. Returns the
    /// attic floor's index, or `None` when there is no floor with exterior
    /// walls to build on.
    pub fn build_attic_floor(&mut self) -> Option<usize> {
        if let Some(i) = self.floors.iter().position(|f| f.kind == FloorKind::Attic) {
            self.refresh_attic_floor();
            return Some(i);
        }
        let top = self.top_normal_floor()?;
        if !self.floors[top]
            .walls
            .iter()
            .any(|w| w.kind == WallKind::Exterior && !w.flags.invisible)
        {
            return None;
        }
        let mut floor = Floor::new("", 0.0);
        floor.kind = FloorKind::Attic;
        floor.ceiling_height = ATTIC_CEILING_HEIGHT;
        floor.settings = self.floors[top].settings.without_foundation();
        self.floors.push(floor);
        let idx = self.floors.len() - 1;
        self.floors[idx].name = self.auto_floor_name(idx);
        self.restack_floors();
        self.refresh_attic_floor();
        Some(idx)
    }

    /// Generate the attic walls and attic rooms of the attic floor from the
    /// exterior walls of the highest normal floor below it (R-68). Walls and
    /// rooms that are already right are left alone, so this is cheap to run
    /// after every Build Roof. Returns whether anything changed.
    pub fn refresh_attic_floor(&mut self) -> bool {
        let Some(attic) = self.floors.iter().position(|f| f.kind == FloorKind::Attic) else {
            return false;
        };
        let Some(top) = self.floors[..attic]
            .iter()
            .rposition(|f| f.kind == FloorKind::Normal)
        else {
            return false;
        };
        let wanted: Vec<Wall> = self.floors[top]
            .walls
            .iter()
            .filter(|w| {
                w.kind == WallKind::Exterior
                    && !w.flags.invisible
                    && !w.flags.foundation
                    && !w.flags.attic
            })
            .cloned()
            .collect();
        let same = |a: &Wall, b: &Wall| {
            a.start.dist(b.start) < 0.01 && a.end.dist(b.end) < 0.01 && a.curve == b.curve
        };
        let have: Vec<&Wall> = self.floors[attic]
            .walls
            .iter()
            .filter(|w| w.flags.attic)
            .collect();
        let mut changed =
            !(have.len() == wanted.len() && wanted.iter().all(|w| have.iter().any(|h| same(h, w))));
        if changed {
            self.floors[attic].walls.retain(|w| !w.flags.attic);
            for src in &wanted {
                let id = self.alloc_id();
                self.floors[attic].walls.push(Wall {
                    id,
                    flags: WallFlags {
                        attic: true,
                        ..WallFlags::default()
                    },
                    curve: src.curve,
                    ..Wall::new(
                        src.start,
                        src.end,
                        src.thickness,
                        ATTIC_WALL_HEIGHT,
                        WallKind::Interior,
                    )
                });
            }
        }
        // An Attic room inside the attic walls.
        let floor = &mut self.floors[attic];
        let rooms = detect_rooms(&floor.walls, 0.5);
        let before = floor.room_names.len();
        floor
            .room_names
            .retain(|n| rooms.iter().any(|r| r.contains(n.anchor)));
        changed |= floor.room_names.len() != before;
        for room in &rooms {
            if room.name_entry(&floor.room_names).is_none() {
                let mut n = RoomName::new(room.interior_point(), "Attic", "Attic");
                let d = function_defaults("Utility", "Attic");
                apply_function_defaults(&mut n, &d, floor.settings.floor_finish_thickness);
                // The roof is the attic's ceiling: a flat platform at the
                // attic's ceiling height would poke through it.
                n.has_ceiling = false;
                floor.room_names.push(n);
                changed = true;
            }
        }
        changed
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

/// Names every room of the foundation floor `floor` a room of type `type_name`
/// (Basement or Crawl Space), with that function's platform defaults.
fn name_foundation_rooms(floor: &mut Floor, type_name: &str) {
    for room in detect_rooms(&floor.walls, 0.5) {
        if room.name_entry(&floor.room_names).is_some() {
            continue;
        }
        let mut n = RoomName::new(room.interior_point(), type_name, type_name);
        let d = function_defaults(type_name, type_name);
        apply_function_defaults(&mut n, &d, 0.0);
        floor.room_names.push(n);
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
    fn derive_exterior_only_copies_only_the_exterior_walls() {
        let mut p = house();
        let up = p
            .build_new_floor_with(&NewFloorOptions {
                derive: DeriveFrom::ExteriorWalls,
                ..NewFloorOptions::default()
            })
            .unwrap();
        assert_eq!(p.floors[up].walls.len(), 4);
        assert!(p.floors[up]
            .walls
            .iter()
            .all(|w| w.kind == WallKind::Exterior));
        assert_eq!(p.floors[up].openings.len(), 1);
    }

    #[test]
    fn derive_all_walls_copies_the_partitions_and_rooms() {
        let mut p = house();
        p.floors[0].room_names.push(crate::model::RoomName::new(
            Point::new(60.0, 60.0),
            "Den",
            "Den",
        ));
        let up = p
            .build_new_floor_with(&NewFloorOptions {
                derive: DeriveFrom::AllWalls,
                copy_rooms: true,
                ..NewFloorOptions::default()
            })
            .unwrap();
        assert_eq!(p.floors[up].walls.len(), 5);
        assert!(p.floors[up]
            .walls
            .iter()
            .any(|w| w.kind == WallKind::Interior));
        assert_eq!(p.floors[up].room_names.len(), 1);
        // A blank floor copies nothing, and the rooms are only copied on ask.
        let blank = p
            .build_new_floor_with(&NewFloorOptions {
                derive: DeriveFrom::Blank,
                ..NewFloorOptions::default()
            })
            .unwrap();
        assert!(p.floors[blank].walls.is_empty() && p.floors[blank].room_names.is_empty());
    }

    #[test]
    fn a_floor_can_be_built_below_and_heights_come_from_the_options() {
        let mut p = house();
        let settings = FloorSettings {
            floor_structure_thickness: 12.0,
            ..FloorSettings::default()
        };
        let below = p
            .build_new_floor_with(&NewFloorOptions {
                source: Some(0),
                place: FloorPlacement::Below,
                derive: DeriveFrom::Blank,
                ceiling_height: Some(96.0),
                settings: Some(settings.clone()),
                ..NewFloorOptions::default()
            })
            .unwrap();
        assert_eq!(below, 0);
        assert_eq!(p.floors.len(), 2);
        assert_eq!(p.floors[0].ceiling_height, 96.0);
        assert_eq!(p.floors[0].settings, settings);
        assert_eq!(p.floors[0].name, "1st Floor");
        assert_eq!(p.floors[1].name, "2nd Floor");
        // The old first floor now sits above the new one.
        assert_eq!(p.floors[1].walls.len(), 5);
        // Nothing goes above the attic or below the foundation.
        let mut q = house();
        q.build_foundation(FoundationKind::MonolithicSlab);
        assert!(q.insert_floor_below(0).is_none());
        assert!(q
            .build_new_floor_with(&NewFloorOptions {
                source: Some(0),
                place: FloorPlacement::Below,
                ..NewFloorOptions::default()
            })
            .is_none());
        assert_eq!(q.insert_floor_below(1), Some(1));
        assert_eq!(q.floors.len(), 3);
    }

    #[test]
    fn floor_defaults_restack_the_floors_above() {
        let mut p = house();
        let up = p.build_new_floor(false);
        let before = p.floors[up].elevation;
        assert!((before - (DEFAULT_CEILING_HEIGHT + FLOOR_PLATFORM_THICKNESS)).abs() < 1e-9);
        // Lower the ceiling by 12": walls at the default top follow, the
        // floor above comes down by the same amount.
        let mut st = p.floors[0].settings.clone();
        st.ceiling_structure_thickness = 1.0;
        assert!(p.apply_floor_settings(0, DEFAULT_CEILING_HEIGHT - 12.0, st));
        assert!(p.floors[0]
            .walls
            .iter()
            .all(|w| (w.height - (DEFAULT_CEILING_HEIGHT - 12.0)).abs() < 1e-9));
        assert!((p.floors[up].elevation - (before - 12.0 + 1.0)).abs() < 1e-9);
        // The upper floor's own floor structure sets its rise.
        let mut upper = p.floors[up].settings.clone();
        upper.floor_structure_thickness = 14.0;
        p.apply_floor_settings(up, p.floors[up].ceiling_height, upper);
        assert!(
            (p.floors[up].elevation - (DEFAULT_CEILING_HEIGHT - 12.0 + 1.0 + 14.0)).abs() < 1e-9
        );
        assert!(!p.apply_floor_settings(9, 96.0, FloorSettings::default()));
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
        // Grade beams on piers: piers at the corners and along the walls,
        // a grade beam on top of each wall.
        p.build_foundation(FoundationKind::Pier);
        assert_eq!(p.floors[0].walls.len(), 4);
        let layer = FoundationLayer::load(&p.floors[0]);
        assert!(layer.piers.len() >= 4, "a pier at every corner");
    }

    fn corner_count(p: &Project) -> usize {
        FoundationLayer::load(&p.floors[0]).piers.len()
    }

    #[test]
    fn walls_with_footings_make_a_basement_or_a_crawl_space() {
        let mut p = house();
        let platform = p.floors[0].settings.floor_structure_thickness;
        // Tall walls: a basement room with a concrete slab, a ceiling under
        // the first floor's platform.
        let mut o = FoundationOptions::new(FoundationKind::StemWall { height: 108.0 });
        o.footing_width = 20.0;
        o.footing_depth = 10.0;
        p.build_foundation_with(&o);
        let f = &p.floors[0];
        assert_eq!(f.kind, FloorKind::Foundation);
        assert_eq!(f.room_names.len(), 1);
        let n = &f.room_names[0];
        assert_eq!(
            (n.name.as_str(), n.room_type.as_str()),
            ("Basement", "Basement")
        );
        assert!(n.has_floor && n.has_ceiling);
        assert_eq!(n.floor_height_offset, crate::rooms::SLAB_FLOOR_THICKNESS);
        let misc = n.misc.as_ref().unwrap();
        assert_eq!(
            crate::extras::structure_thickness(&misc.floor_structure),
            crate::rooms::SLAB_FLOOR_THICKNESS
        );
        assert_eq!(f.ceiling_height, 108.0);
        assert_eq!(f.settings.ceiling_structure_thickness, platform);
        let b = f.settings.foundation.unwrap();
        assert_eq!((b.footing_width, b.footing_depth), (20.0, 10.0));
        assert_eq!(f.elevation, -108.0);
        // Short walls: a crawl space, no floor and no ceiling of its own.
        let mut o = FoundationOptions::new(FoundationKind::StemWall { height: 36.0 });
        o.footing_width = 0.0;
        p.build_foundation_with(&o);
        let n = &p.floors[0].room_names[0];
        assert_eq!(n.room_type, "Crawl Space");
        assert!(!n.has_floor && !n.has_ceiling);
        assert_eq!(p.floors[0].settings.foundation.unwrap().footing_width, 0.0);
        // The caller can force the type or leave the rooms alone.
        let mut o = FoundationOptions::new(FoundationKind::StemWall { height: 108.0 });
        o.rooms = FoundationRooms::CrawlSpace;
        p.build_foundation_with(&o);
        assert_eq!(p.floors[0].room_names[0].room_type, "Crawl Space");
        o.rooms = FoundationRooms::None;
        p.build_foundation_with(&o);
        assert!(p.floors[0].room_names.is_empty());
        assert_eq!(p.floors.len(), 2, "rebuilding replaces the foundation");
    }

    #[test]
    fn a_monolithic_slab_has_a_stem_wall_and_a_slab_of_the_chosen_thickness() {
        let mut p = house();
        let mut o = FoundationOptions::new(FoundationKind::MonolithicSlab);
        o.slab_thickness = 6.0;
        o.edge_height = 16.0;
        p.build_foundation_with(&o);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 4);
        assert!(f.walls.iter().all(|w| w.height == 16.0));
        assert_eq!(f.elevation, -16.0);
        assert!(f.room_names.is_empty(), "a slab has no basement room");
        let layer = FoundationLayer::load(f);
        assert_eq!(layer.slabs.len(), 1);
        let slab = &layer.slabs[0];
        assert_eq!(slab.thickness, 6.0);
        // Inside the 8" stem wall of a 240" x 120" house.
        let area = crate::foundation::outline_area(&slab.outline);
        assert!(area < 240.0 * 120.0 && area > 220.0 * 100.0, "{area}");
        // The top meets the underside of the first floor's slab.
        assert!((f.elevation + slab.top_elevation + 1.0).abs() < 1e-9);
    }

    #[test]
    fn grade_beams_stand_on_piers_spaced_along_the_walls() {
        let mut p = house();
        let mut o = FoundationOptions::new(FoundationKind::Pier);
        o.pier_spacing = 60.0;
        o.pier_height = 30.0;
        o.edge_height = 20.0;
        p.build_foundation_with(&o);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 4);
        for w in &f.walls {
            assert_eq!(w.bottom_offset, 30.0, "the beam sits on the piers");
            assert_eq!(w.height, 20.0);
            assert_eq!(w.thickness, GRADE_BEAM_WIDTH);
        }
        assert_eq!(f.elevation, -50.0);
        let layer = FoundationLayer::load(f);
        // 240" walls take 4 spans of 60", 120" walls take 2: 12 piers.
        assert_eq!(layer.piers.len(), 12);
        assert_eq!(corner_count(&p), 12);
        assert!(layer.piers.iter().all(|pier| pier.height == 30.0));
        assert!(layer.piers.iter().all(|pier| pier.footing.is_some()));
        // Every corner has one.
        for c in [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)] {
            assert!(layer
                .piers
                .iter()
                .any(|pier| pier.center.dist(Point::new(c.0, c.1)) < 1.0));
        }
    }

    #[test]
    fn new_floors_do_not_inherit_the_foundation_record() {
        let mut p = house();
        p.build_foundation(FoundationKind::StemWall { height: 36.0 });
        assert!(p.floors[0].settings.foundation.is_some());
        let above = p.insert_floor_below(1).unwrap();
        assert!(p.floors[above].settings.foundation.is_none());
        let up = p.insert_floor_above(0).unwrap();
        assert!(p.floors[up].settings.foundation.is_none());
    }

    #[test]
    fn the_attic_floor_gets_attic_walls_and_an_attic_room() {
        let mut p = house();
        p.build_new_floor(true);
        let a = p.build_attic_floor().unwrap();
        assert_eq!(a, 2);
        let f = &p.floors[a];
        assert_eq!(f.kind, FloorKind::Attic);
        assert_eq!(f.name, "Attic");
        assert_eq!(f.walls.len(), 4);
        assert!(f
            .walls
            .iter()
            .all(|w| w.flags.attic && w.kind == WallKind::Interior));
        assert!(f.walls.iter().all(|w| w.height == ATTIC_WALL_HEIGHT));
        assert_eq!(f.room_names.len(), 1);
        assert_eq!(f.room_names[0].room_type, "Attic");
        assert!(
            !f.room_names[0].has_floor,
            "an attic room has no floor platform"
        );
        // Above the second floor.
        let top = &p.floors[1];
        assert!(f.elevation > top.elevation + top.ceiling_height);
        // Running it again changes nothing (ids stay).
        let ids: Vec<Id> = f.walls.iter().map(|w| w.id).collect();
        assert_eq!(p.build_attic_floor(), Some(a));
        assert!(!p.refresh_attic_floor());
        assert_eq!(
            p.floors[a].walls.iter().map(|w| w.id).collect::<Vec<_>>(),
            ids
        );
        assert_eq!(p.floors.len(), 3);
        // Moving a wall of the floor below brings the attic along.
        let w0 = p.floors[1].walls[0].id;
        p.floors[1].wall_mut(w0).unwrap().end = Point::new(260.0, 0.0);
        assert!(p.refresh_attic_floor());
        assert!(p.floors[a]
            .walls
            .iter()
            .any(|w| w.end.dist(Point::new(260.0, 0.0)) < 0.01));
        // No exterior walls, no attic.
        let mut blank = Project::new("b");
        assert!(blank.build_attic_floor().is_none());
        assert!(!blank.refresh_attic_floor());
    }
}
