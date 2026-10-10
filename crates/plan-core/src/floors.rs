//! Floor commands for Chief parity (`docs/parity/rooms-floors.md`, R-55..R-68):
//! build/insert/delete/exchange floors, build a foundation, floor naming.
//!
//! Floors are stored bottom to top in `Project::floors`. A foundation floor
//! (if any) is index 0 and an attic (if any) is last. Structural commands
//! keep the cameras' floor indices and the floor elevations consistent.

use crate::fireplace::{Fireplace, FireplaceKind};
use crate::foundation::{
    Footing, FoundationLayer, FoundationSettings, Pad, Pier, PierShape, Slab, STEP_TOLERANCE,
};
use crate::geometry::Point;
use crate::model::{Floor, Id, Opening, OpeningKind, Project, RoomName, Wall, WallKind};
use crate::openings::OpeningStyle;
use crate::rooms::{apply_function_defaults, detect_rooms, function_defaults, Room};
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
/// The most living floors a plan can have (manual p. 762); the foundation and
/// the attic are not counted.
pub const MAX_LIVING_FLOORS: usize = 30;
/// A basement whose clear height reaches this counts toward the Living Area
/// even without finishes, inches (manual p. 749).
pub const BASEMENT_LIVING_CLEAR_HEIGHT: f64 = 48.0;
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// Height of the piers under grade beams, inches (the Piers group's
    /// Depth).
    pub pier_height: f64,
    /// Greatest distance between piers along a wall (the Piers group's
    /// Maximum Separation), inches.
    pub pier_spacing: f64,
    pub rooms: FoundationRooms,
    /// Auto Rebuild, platform hanging, S markers, garage options, piers,
    /// rebar and the rest of the Foundation Defaults dialog.
    pub settings: FoundationSettings,
}

impl Default for FoundationOptions {
    fn default() -> Self {
        Self::new(FoundationKind::StemWall { height: 36.0 })
    }
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
            settings: FoundationSettings::default(),
        }
    }

    /// Height of the foundation floor: the walls (and the platform zone
    /// under the first floor's walls) from the bottom to the first floor.
    /// This is the older measure; [`FoundationOptions::floor_height`] knows
    /// about the platform that bears on grade beams.
    pub fn total_height(&self) -> f64 {
        match self.kind {
            FoundationKind::StemWall { height } => height.max(1.0),
            FoundationKind::MonolithicSlab => self.edge_height.max(1.0),
            FoundationKind::Pier => self.pier_height.max(1.0) + self.edge_height.max(1.0),
        }
    }

    /// Distance from the bottom of the foundation to the finished level of
    /// Floor 1 (so Floor 0 sits this far below it), given the floor
    /// platform of Floor 1. Grade beams carry the platform on top of them
    /// (DECISIONS 301 as corrected), so their floor is the platform taller.
    pub fn floor_height(&self, platform: f64) -> f64 {
        match self.kind {
            FoundationKind::Pier => self.total_height() + platform.max(0.0),
            _ => self.total_height(),
        }
    }

    /// How far below the finished level of Floor 1 the tops of the stem
    /// walls stop: the platform that bears on them, or nothing when the
    /// platform hangs inside the walls (Walls with Footings only).
    pub fn top_gap(&self, platform: f64) -> f64 {
        match self.kind {
            FoundationKind::MonolithicSlab => 0.0,
            FoundationKind::StemWall { .. } if self.settings.hang_platform => 0.0,
            _ => platform.max(0.0),
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
    /// The choices the foundation floor was built with, kept so Auto Rebuild
    /// and a rebuild in place can run again with them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foundation_options: Option<FoundationOptions>,
    /// A digest of what the foundation was built from (Floor 1's walls,
    /// rooms, doors and fireplaces); Auto Rebuild compares it.
    #[serde(skip_serializing_if = "is_zero")]
    pub foundation_signature: u64,
    /// Default fill of the rooms on this floor (the Floor Defaults Fill Style
    /// panel); a room with its own fill keeps it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_fill: Option<crate::extras::RoomFill>,
    /// The layered Floor/Ceiling Structure and Finish definitions of this
    /// floor level; a slot that is not set keeps the single thickness above
    /// (see [`crate::assemblies`]).
    #[serde(skip_serializing_if = "crate::assemblies::PlatformAssemblies::is_legacy")]
    pub platform: crate::assemblies::PlatformAssemblies,
    /// A copy of the plan-wide definitions of the slots above that follow the
    /// default, kept so a floor can be read without the plan; refreshed by
    /// `sync_thicknesses`.
    #[serde(skip_serializing_if = "crate::assemblies::PlatformAssemblies::is_legacy")]
    pub platform_inherited: crate::assemblies::PlatformAssemblies,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
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
            foundation_options: None,
            foundation_signature: 0,
            room_fill: None,
            platform: crate::assemblies::PlatformAssemblies::default(),
            platform_inherited: crate::assemblies::PlatformAssemblies::default(),
        }
    }
}

impl FloorSettings {
    /// These settings without the foundation record, for a floor that is not
    /// the foundation (new floors copy their source's settings).
    pub fn without_foundation(&self) -> FloorSettings {
        FloorSettings {
            foundation: None,
            foundation_options: None,
            foundation_signature: 0,
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
    /// Move the roof planes on the highest floor up with the new floor
    /// (manual p. 764). Only roof records that hold planes move.
    pub move_roof_up: bool,
    /// Step the floor and ceiling heights of the new floor so the ceilings
    /// (and, below, the floors) of the existing floor keep their heights.
    pub step_elevations: bool,
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

/// Is `v` one of the records of a floor's roof slot (planes, settings,
/// ceilings, dormers, faces)?
fn is_roof_record(v: &serde_json::Value) -> bool {
    v.get("kind").and_then(serde_json::Value::as_str).is_some()
}

/// Do the roof records hold any roof plane?
fn roof_has_planes(roofs: &[serde_json::Value]) -> bool {
    roofs
        .iter()
        .any(|v| v.get("kind").and_then(serde_json::Value::as_str) == Some("plane"))
}

/// Raises every roof record by `dy` inches: the vertices of planes, ceiling
/// planes and the faces under Dutch gables are scene elevations.
fn raise_roofs(roofs: &mut [serde_json::Value], dy: f64) {
    if dy.abs() < 1e-9 {
        return;
    }
    for rec in roofs.iter_mut() {
        let Some(poly) = rec
            .get_mut("polygon3d")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        for v in poly.iter_mut() {
            if let Some(y) = v.get_mut(1) {
                if let Some(h) = y.as_f64() {
                    *y = serde_json::json!(h + dy);
                }
            }
        }
    }
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
            .rposition(|f| f.kind == FloorKind::Normal && !f.is_cad_detail())
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

    /// How many living floors the plan has (the foundation, the attic and
    /// CAD details are not counted).
    pub fn living_floor_count(&self) -> usize {
        self.floors
            .iter()
            .filter(|f| f.kind == FloorKind::Normal && !f.is_cad_detail())
            .count()
    }

    /// Why a floor cannot be built or inserted with `opts`, in words; `None`
    /// when it can (R-59, R-121: not above the attic, not below the
    /// foundation, at most [`MAX_LIVING_FLOORS`] living floors).
    pub fn new_floor_blocker(&self, opts: &NewFloorOptions) -> Option<String> {
        let src = opts
            .source
            .filter(|&i| i < self.floors.len())
            .or_else(|| self.top_normal_floor());
        match (opts.place, src) {
            (FloorPlacement::Above, Some(i)) if self.floors[i].kind == FloorKind::Attic => {
                return Some("Cannot build a floor above the attic".into());
            }
            (FloorPlacement::Below, Some(i)) if self.floors[i].kind == FloorKind::Foundation => {
                return Some("Cannot build a floor below the foundation".into());
            }
            _ => {}
        }
        (self.living_floor_count() >= MAX_LIVING_FLOORS)
            .then(|| format!("A plan can have at most {MAX_LIVING_FLOORS} living floors"))
    }

    /// Build New Floor with the dialog's full set of choices (R-59): derive
    /// the exterior walls, every wall or nothing from the source floor, copy
    /// its rooms and slab data, take the heights from the options or the
    /// source floor, step the new floor's elevations to match the existing
    /// floor and move the highest floor's roof up. Returns the new floor's
    /// index, or `None` when the placement is impossible (above the attic,
    /// below the foundation, past [`MAX_LIVING_FLOORS`]).
    pub fn build_new_floor_with(&mut self, opts: &NewFloorOptions) -> Option<usize> {
        if self.new_floor_blocker(opts).is_some() {
            return None;
        }
        let src = opts
            .source
            .filter(|&i| i < self.floors.len())
            .or_else(|| self.top_normal_floor());
        let at = match (opts.place, src) {
            (FloorPlacement::Above, Some(i)) => i + 1,
            (FloorPlacement::Below, Some(i)) => i,
            (_, None) => self.floors.len(),
        };
        let was_auto = self.auto_named();
        // The roof planes of the highest floor wait out of the floor while
        // the stack changes; they land on the new floor (above) or stay on
        // their floor (below), raised by what the stack grew.
        let top = self.top_normal_floor();
        let top_elevation = top.map(|t| self.floors[t].elevation);
        let carried = if opts.move_roof_up && (opts.place == FloorPlacement::Below || src == top) {
            top.map(|t| self.floors[t].roofs.clone())
                .filter(|r| roof_has_planes(r))
        } else {
            None
        };
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
            if opts.step_elevations {
                self.step_new_floor_to(i, opts.place, &mut floor);
            }
        }
        // Adding a floor above a split level resets the ceiling heights of the
        // rooms whose floors are raised or lowered to the default, unless the
        // step option keeps the ceilings (manual p. 772).
        if let (FloorPlacement::Above, Some(i), false) = (opts.place, src, opts.step_elevations) {
            if src == top {
                for n in &mut self.floors[i].room_names {
                    if n.floor_height_offset != 0.0 {
                        n.ceiling_height = None;
                    }
                }
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
        if let (Some(mut roofs), Some(t0), Some(t)) = (carried, top_elevation, top) {
            // `t` is the old index of the highest floor.
            let new_top = if at <= t { t + 1 } else { t };
            let rise = match opts.place {
                FloorPlacement::Above => self.floors[at].elevation - t0,
                FloorPlacement::Below => self.floors[new_top].elevation - t0,
            };
            raise_roofs(&mut roofs, rise);
            match opts.place {
                FloorPlacement::Above => {
                    self.floors[new_top].roofs.retain(|v| !is_roof_record(v));
                    self.floors[at].roofs = roofs;
                }
                FloorPlacement::Below => self.floors[new_top].roofs = roofs,
            }
        }
        Some(at)
    }

    /// Step Floor/Ceiling Elevations to Match Existing Floor (manual pp.
    /// 764-765): the rooms of the new floor `target`, which is built above
    /// or below floor `src`, get the floor (above) or ceiling (below)
    /// heights that keep the ceilings (or floors) of `src`'s rooms where they
    /// are.
    fn step_new_floor_to(&self, src: usize, place: FloorPlacement, target: &mut Floor) {
        let s = &self.floors[src];
        let from = detect_rooms(&s.walls, 0.5);
        let into = detect_rooms(&target.walls, 0.5);
        for room in &from {
            let Some(entry) = room.name_entry(&s.room_names) else {
                continue;
            };
            let at = room.interior_point();
            let Some(there) = into.iter().find(|r| r.contains(at)) else {
                continue;
            };
            // How far the room's ceiling (above) or floor (below) is from
            // the floor's default.
            let (raise, ceiling) = match place {
                FloorPlacement::Above => (
                    entry.floor_height_offset + entry.ceiling_height.unwrap_or(s.ceiling_height)
                        - s.ceiling_height,
                    None,
                ),
                FloorPlacement::Below => {
                    (0.0, Some(target.ceiling_height + entry.floor_height_offset))
                }
            };
            if raise.abs() < 0.01
                && ceiling.is_none_or(|c| (c - target.ceiling_height).abs() < 0.01)
            {
                continue;
            }
            let anchor = there.interior_point();
            let slot = target
                .room_names
                .iter()
                .position(|n| there.contains(n.anchor));
            let n = match slot {
                Some(i) => &mut target.room_names[i],
                None => {
                    target.room_names.push(RoomName::new(
                        anchor,
                        entry.name.clone(),
                        entry.room_type.clone(),
                    ));
                    target.room_names.last_mut().expect("just pushed")
                }
            };
            // Several rooms below one room above: it clears the tallest.
            n.floor_height_offset = n.floor_height_offset.max(raise);
            if let Some(c) = ceiling {
                n.ceiling_height = Some(n.ceiling_height.map_or(c, |old| old.min(c)));
            }
        }
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
        // Floor 0 stays while Auto Rebuild Foundation is on (manual p. 766).
        if self.floors[idx].kind == FloorKind::Foundation && self.foundation_auto_rebuild() {
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

    /// Does the plan have a foundation floor whose Auto Rebuild Foundation is
    /// on? Floor 0 then cannot be deleted or edited by hand (manual p. 745).
    pub fn foundation_auto_rebuild(&self) -> bool {
        self.floors
            .first()
            .filter(|f| f.kind == FloorKind::Foundation)
            .and_then(|f| f.settings.foundation_options)
            .is_some_and(|o| o.settings.auto_rebuild)
    }

    /// Build Foundation (R-61, R-62) with the room types `Garage` and `Slab`
    /// taken as the slab rooms (see [`Project::build_foundation_classified`]
    /// for plans whose Room Types have other names for those functions).
    pub fn build_foundation_with(&mut self, opts: &FoundationOptions) -> usize {
        self.build_foundation_classified(opts, &is_slab_room_type)
    }

    /// Build Foundation (R-61, R-62): create (or rebuild in place) the
    /// Foundation floor at index 0 under the first normal floor, as the
    /// manual describes on pp. 741-749.
    ///
    /// * Walls under every exterior wall of a room that has Build Foundation
    ///   Below (never a railing or invisible wall), under interior walls
    ///   that ask for Create Wall/Footing Below or bear the floor above, and
    ///   under walls that separate a slab room from the rest or rooms of
    ///   different floor heights. Their tops and bottoms follow the floor
    ///   heights and stem wall heights of the rooms they bound, so a plan
    ///   with several floor heights gets a stepped foundation.
    /// * Walls with Footings: the stem wall height runs from the footing
    ///   top to the underside of Floor 1's platform, which bears on top
    ///   (`settings.hang_platform` builds the walls up to the platform's
    ///   top instead). A basement or crawl space room is made inside: a
    ///   clear height of 72 in gets a slab, 48 in counts as living area,
    ///   less is a crawl space.
    /// * Monolithic Slab: thickened edges under the exterior walls and a
    ///   slab in every room; garage and slab rooms are lowered and curbed.
    /// * Grade Beams on Piers: piers (round or square) at the wall ends and
    ///   no further apart than the maximum separation, grade beams on them.
    /// * Garage and slab rooms (and every room with Slab at Top of Stem
    ///   Wall) get stem walls and a slab of their own; a door in their walls
    ///   leaves a cutout in the stem wall or curb as wide as its rough
    ///   opening plus Add for Concrete Cutout.
    /// * Masonry fireplaces on Floor 1 get a fireplace foundation (Walls
    ///   with Footings and Grade Beams on Piers).
    ///
    /// `is_slab_type` tells which Room Types have the Garage or Slab
    /// function. Returns the foundation floor's index (always 0). Existing
    /// floors move up by one when a foundation is newly created.
    pub fn build_foundation_classified(
        &mut self,
        opts: &FoundationOptions,
        is_slab_type: &dyn Fn(&str) -> bool,
    ) -> usize {
        let has_foundation = self
            .floors
            .first()
            .is_some_and(|f| f.kind == FloorKind::Foundation);
        let first_normal = self
            .floors
            .iter()
            .position(|f| f.kind != FloorKind::Foundation)
            .unwrap_or(0);
        let old_kind = has_foundation
            .then(|| self.floors[0].settings.foundation_options)
            .flatten()
            .map(|o| o.kind);
        let platform = self.floors[first_normal].settings.floor_structure_thickness;
        let s = opts.settings;
        let mono = matches!(opts.kind, FoundationKind::MonolithicSlab);
        let beams = matches!(opts.kind, FoundationKind::Pier);

        // The Monolithic Slab boxes a monolithic build ticked are not the
        // user's choice once another type is built (nor in a rebuild of it).
        let stale_flags = old_kind == Some(FoundationKind::MonolithicSlab);
        // Garage and slab rooms start lowered (manual p. 748: the default
        // drop when the room's floor height is still 0).
        if s.garage_floor {
            let drop = if mono {
                s.lower_garage_floor
            } else {
                s.garage_drop(platform)
            };
            for n in &mut self.floors[first_normal].room_names {
                let flagged = !mono && !stale_flags && n.monolithic_slab.is_some();
                if n.floor_height_offset == 0.0
                    && (flagged || is_slab_type(&n.room_type))
                    && n.options.build_foundation_below
                {
                    n.floor_height_offset = -drop;
                }
            }
        }
        let rooms = source_rooms(&self.floors[first_normal], opts, is_slab_type, stale_flags);

        // ----- the walls -----
        // The Minimum Height of the dialog is applied to the stem wall height
        // when it becomes `height` (`FoundationSpec::to_kind`); here it keeps
        // the walls under lowered rooms and garages from getting shorter.
        let floor_h = opts.floor_height(platform).max(1.0);
        let gap = opts.top_gap(platform);
        let mut planned = plan_walls(&self.floors[first_normal], &rooms, opts, platform, floor_h);
        // A bay, box or bow window on Floor 1 gets foundation wall sections
        // under its sections, as high as the foundation under its wall.
        let room_list: Vec<Room> = rooms.iter().map(|r| r.room.clone()).collect();
        let unit_walls = bay_walls(&self.floors[first_normal], &room_list, &planned);
        planned.extend(unit_walls);
        if beams {
            // Grade beams stand on piers at one height.
            for w in &mut planned {
                w.top = -gap;
                w.bottom = -gap - opts.edge_height.max(1.0);
            }
        }
        let l0 = if beams {
            -floor_h
        } else {
            planned.iter().map(|w| w.bottom).fold(-floor_h, f64::min)
        };
        let height = -l0;
        // Clear height of the rooms below Floor 1's platform before a slab.
        let clear_nos = -l0 - gap;

        // Rebuilding keeps what was drawn on the foundation floor
        // (dimensions, CAD, symbols, hand-drawn walls and their openings,
        // renamed rooms): only the walls and objects the build makes are
        // replaced.
        let mut floor = if has_foundation {
            let mut old = self.floors[0].clone();
            let made: Vec<Id> = old
                .walls
                .iter()
                .filter(|w| w.flags.foundation)
                .map(|w| w.id)
                .collect();
            old.walls.retain(|w| !w.flags.foundation);
            old.openings.retain(|o| !made.contains(&o.wall_id));
            old.room_names.retain(|n| {
                !(n.name == n.room_type
                    && matches!(n.room_type.as_str(), "Basement" | "Crawl Space" | "Slab"))
            });
            let bases: Vec<Id> = old
                .fireplaces
                .iter()
                .filter(|f| f.base_of.is_some())
                .map(|f| f.id)
                .collect();
            old.symbols.retain(|y| !bases.contains(&y.id));
            old.fireplaces.retain(|f| f.base_of.is_none());
            old.elevation = l0;
            old
        } else {
            Floor::new("Foundation", l0)
        };
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
        floor.settings.foundation_options = Some(*opts);
        // The first floor's platform is the ceiling of the rooms below.
        floor.settings.ceiling_structure_thickness = platform;
        let thickness = if beams {
            GRADE_BEAM_WIDTH
        } else {
            FOUNDATION_WALL_THICKNESS
        };
        let mut made_walls: Vec<Id> = Vec::new();
        for pw in &planned {
            let id = self.alloc_id();
            let mut wall = Wall {
                id,
                thickness,
                height: pw.top - pw.bottom,
                bottom_offset: pw.bottom - l0,
                flags: WallFlags {
                    foundation: true,
                    ..WallFlags::default()
                },
                wall_type: Some(FOUNDATION_WALL_TYPE.to_string()),
                curve: pw.curve,
                ..Wall::new(
                    pw.start,
                    pw.end,
                    thickness,
                    pw.top - pw.bottom,
                    WallKind::Exterior,
                )
            };
            // Stepped stem walls get vertical footings; the thickened edge
            // of a monolithic slab a chamfer (manual p. 747).
            if mono {
                wall.spec.foundation.slab_footing = true;
                wall.spec.foundation.chamfer_monolithic = true;
                wall.spec.foundation.chamfer_width = s.chamfer_width;
                wall.spec.foundation.chamfer_height = s.chamfer_height;
            }
            wall.spec.foundation.create_below = pw.create_below;
            floor.walls.push(wall);
            made_walls.push(id);
        }
        if s.vertical_step_footings && !mono {
            let steps = crate::foundation::step_markers(&floor);
            for w in &mut floor.walls {
                let stepped = steps.iter().any(|m| {
                    m.tall_wall == w.id && (m.at.dist(w.start) < 0.5 || m.at.dist(w.end) < 0.5)
                });
                if stepped && footed {
                    w.spec.foundation.footing = true;
                    w.spec.foundation.vertical_footing = true;
                    w.spec.foundation.footing_width = opts.footing_width;
                    w.spec.foundation.footing_height = opts.footing_depth;
                }
            }
        }

        // ----- doors in garage and slab walls: cutouts in the stem wall -----
        if s.garage_floor && rooms.iter().any(|r| r.slab) {
            let src = &self.floors[first_normal];
            let mut cuts: Vec<Opening> = Vec::new();
            for (pw, &fid) in planned.iter().zip(&made_walls).filter(|(w, _)| w.slab_edge) {
                let Some(fw) = floor.walls.iter().find(|w| w.id == fid) else {
                    continue;
                };
                // The door belongs to the piece of the wall it stands in.
                let here = pw.offset0..=pw.offset0 + pw.len;
                for op in src.openings.iter().filter(|o| {
                    o.wall_id == pw.source
                        && o.kind == OpeningKind::Door
                        && here.contains(&o.center_offset)
                }) {
                    let width = crate::foundation::garage_cut_width(op);
                    let mut cut = Opening::new(
                        fid,
                        op.center_offset - pw.offset0,
                        OpeningKind::Door,
                        width,
                        fw.height.max(1.0),
                        0.0,
                    );
                    cut.style = OpeningStyle::Doorway;
                    cut.label_override = Some("Cutout".into());
                    cuts.push(cut);
                }
            }
            for mut cut in cuts {
                cut.id = self.alloc_id();
                floor.openings.push(cut);
            }
        }

        // ----- piers, slabs -----
        let mut layer = FoundationLayer::default();
        if beams {
            let mut spots: Vec<Point> = Vec::new();
            let mut add = |p: Point| {
                if !spots.iter().any(|c| c.dist(p) < 1.0) {
                    spots.push(p);
                }
            };
            for pw in &planned {
                for p in crate::foundation::pier_positions(pw.start, pw.end, opts.pier_spacing) {
                    add(p);
                }
            }
            let depth = opts.pier_height.max(1.0);
            let width = s.pier_width.max(1.0);
            for c in spots {
                let id = self.alloc_id();
                match s.pier_shape {
                    PierShape::Round => {
                        let mut pier = Pier::new(id, c);
                        pier.diameter = width;
                        pier.height = depth;
                        pier.elevation = depth;
                        pier.footing = Some(Footing {
                            width: 24.0,
                            depth: 12.0,
                        });
                        layer.piers.push(pier);
                    }
                    PierShape::Square => {
                        let mut pad = Pad::new(id, c);
                        pad.size = width;
                        pad.thickness = depth;
                        pad.elevation = depth;
                        layer.pads.push(pad);
                    }
                }
            }
        }
        // Floor 0 as the walls leave it, to find the rooms of the slabs.
        let below = detect_rooms(&floor.walls, 0.5);
        let mut slab_under: Vec<bool> = vec![false; below.len()];
        let src_room_of = |r: &Room| -> Option<&SourceRoom> {
            let at = r.interior_point();
            rooms.iter().find(|sr| sr.room.contains(at))
        };
        for (i, r0) in below.iter().enumerate() {
            let sr = src_room_of(r0);
            let slab_room = s.garage_floor && sr.is_some_and(|x| x.slab);
            let outline = if r0.inner_polygon.len() >= 3 {
                r0.inner_polygon.clone()
            } else {
                r0.polygon.clone()
            };
            if outline.len() < 3 {
                continue;
            }
            let offset = sr.map_or(0.0, |x| x.offset);
            let top = if mono {
                // The slab's top meets the underside of the first floor's
                // slab (1 in under its finished floor); a garage is lowered.
                Some(offset.min(0.0) - l0 - 1.0)
            } else if slab_room {
                Some(offset - l0)
            } else if s.slab_at_stem_top && matches!(opts.kind, FoundationKind::StemWall { .. }) {
                Some(offset - opts.top_gap(platform) - l0)
            } else {
                None
            };
            if let Some(top) = top {
                let id = self.alloc_id();
                let mut slab = Slab::new(id, outline);
                slab.thickness = opts.slab_thickness.max(1.0);
                slab.top_elevation = top;
                layer.slabs.push(slab);
                // A monolithic slab leaves its house rooms unnamed; a garage
                // or slab room under it supplies the floor above.
                slab_under[i] = !mono || slab_room;
            }
        }
        layer.store(&mut floor);

        // ----- fireplace foundations -----
        if !mono {
            let src = &self.floors[first_normal];
            let bases: Vec<(PlacedSymbol, Fireplace)> = src
                .fireplace_symbols()
                .into_iter()
                .filter(|(_, fp)| fp.kind == FireplaceKind::Masonry)
                .map(|(sym, fp)| (sym.clone(), fp))
                .collect();
            for (sym, fp) in bases {
                let id = self.alloc_id();
                let (base_sym, base_fp) = fireplace_base(&sym, &fp, id, clear_nos.max(12.0));
                floor.symbols.push(base_sym);
                floor.fireplaces.push(base_fp);
            }
        }

        // ----- rooms of the foundation floor (R-18) -----
        let slab_t = opts.slab_thickness.max(1.0);
        let tier: Option<(&str, bool)> = if matches!(opts.kind, FoundationKind::StemWall { .. }) {
            let slab_ok = clear_nos - slab_t >= BASEMENT_MIN_CLEAR_HEIGHT;
            match opts.rooms {
                FoundationRooms::None => None,
                FoundationRooms::Basement => Some(("Basement", slab_ok)),
                FoundationRooms::CrawlSpace => Some(("Crawl Space", false)),
                FoundationRooms::Auto if slab_ok => Some(("Basement", true)),
                FoundationRooms::Auto if clear_nos >= BASEMENT_LIVING_CLEAR_HEIGHT => {
                    Some(("Basement", false))
                }
                FoundationRooms::Auto => Some(("Crawl Space", false)),
            }
        } else {
            None
        };
        name_foundation_rooms(&mut floor, &below, &slab_under, tier);

        if has_foundation {
            self.floors[0] = floor;
        } else {
            self.floors.insert(0, floor);
            self.shift_cameras(0, 1);
        }
        self.restack_floors();

        // ----- Floor 1 takes its floor from the foundation -----
        let src_idx = first_normal + usize::from(!has_foundation);
        let slab_flags: Vec<(Point, bool)> = rooms
            .iter()
            .filter_map(|r| r.anchor.map(|a| (a, r.slab)))
            .collect();
        let was_mono = old_kind == Some(FoundationKind::MonolithicSlab);
        for n in &mut self.floors[src_idx].room_names {
            let slab_room = slab_flags
                .iter()
                .any(|(p, slab)| *slab && p.dist(n.anchor) < 1e-6);
            let on_slab = if mono {
                true
            } else {
                (s.garage_floor && slab_room)
                    || (s.slab_at_stem_top && matches!(opts.kind, FoundationKind::StemWall { .. }))
            };
            if on_slab {
                n.options.floor_from_foundation = true;
                if mono {
                    n.monolithic_slab.get_or_insert(crate::rooms::RoomSlab {
                        thickness: opts.slab_thickness,
                        stem_height: opts.edge_height,
                    });
                }
            }
            if was_mono && !mono {
                // The boxes a monolithic build ticked come off again (p. 738).
                n.monolithic_slab = None;
                if !on_slab {
                    n.options.floor_from_foundation = false;
                }
            }
        }
        let sig = foundation_signature(&self.floors[src_idx], opts);
        self.floors[0].settings.foundation_signature = sig;
        0
    }

    /// Auto Rebuild Foundation: when the foundation was built with Auto
    /// Rebuild on and what Floor 1 offers it has changed since, build it
    /// again with the same choices. Returns whether it was rebuilt.
    pub fn auto_rebuild_foundation(&mut self, is_slab_type: &dyn Fn(&str) -> bool) -> bool {
        let Some(f0) = self
            .floors
            .first()
            .filter(|f| f.kind == FloorKind::Foundation)
        else {
            return false;
        };
        let Some(opts) = f0
            .settings
            .foundation_options
            .filter(|o| o.settings.auto_rebuild)
        else {
            return false;
        };
        let Some(src) = self.floors.iter().find(|f| f.kind == FloorKind::Normal) else {
            return false;
        };
        if foundation_signature(src, &opts) == f0.settings.foundation_signature {
            return false;
        }
        self.build_foundation_classified(&opts, is_slab_type);
        true
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
        // The attic goes above the building, ahead of any CAD details.
        let idx = self
            .floors
            .iter()
            .position(Floor::is_cad_detail)
            .unwrap_or(self.floors.len());
        self.floors.insert(idx, floor);
        self.shift_cameras(idx, 1);
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
        // Rooms cannot be created on the Attic floor (manual p. 773): no
        // Attic room is made inside the attic walls, and the entries an
        // earlier version made are dropped.
        let floor = &mut self.floors[attic];
        let before = floor.room_names.len();
        floor.room_names.clear();
        changed |= floor.room_names.len() != before;
        changed
    }

    /// The warning that shows when walls or other objects are drawn on the
    /// Attic floor (manual p. 773), `None` for other floors and for an attic
    /// floor holding only the attic walls Build Roof makes.
    pub fn attic_floor_warning(&self, idx: usize) -> Option<&'static str> {
        let f = self
            .floors
            .get(idx)
            .filter(|f| f.kind == FloorKind::Attic)?;
        let drawn = f.walls.iter().any(|w| !w.flags.attic)
            || !f.symbols.is_empty()
            || !f.cabinets.is_empty()
            || !f.stairs.is_empty()
            || !f.cad.is_empty()
            || !f.dimensions.is_empty()
            || !f.framing.is_empty();
        drawn.then_some(
            "The Attic floor is not meant to be a living area: rooms cannot be created on it. \
             Build an attic loft or bonus room on a numbered floor instead.",
        )
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

/// The room types the builder treats as slab rooms when the caller has no
/// Room Types list to ask: Garage and Slab.
pub fn is_slab_room_type(type_name: &str) -> bool {
    matches!(type_name, "Garage" | "Slab")
}

/// A room of the first floor as the foundation builder sees it.
struct SourceRoom {
    room: Room,
    /// Garage or Slab type, or flagged Monolithic Slab Foundation: the room
    /// gets a slab of its own with stem walls or curbs.
    slab: bool,
    /// Build Foundation Below is off.
    no_foundation: bool,
    /// The room's floor height above the floor's datum.
    offset: f64,
    /// The room's own Stem Wall Height.
    stem: Option<f64>,
    /// The anchor of the room's name entry, if it has one.
    anchor: Option<Point>,
}

fn source_rooms(
    floor: &Floor,
    opts: &FoundationOptions,
    is_slab_type: &dyn Fn(&str) -> bool,
    stale_flags: bool,
) -> Vec<SourceRoom> {
    let mono = matches!(opts.kind, FoundationKind::MonolithicSlab);
    detect_rooms(&floor.walls, 0.5)
        .into_iter()
        .map(|room| {
            let e = room.name_entry(&floor.room_names);
            let slab = opts.settings.garage_floor
                && e.is_some_and(|n| {
                    is_slab_type(&n.room_type)
                        || (!mono && !stale_flags && n.monolithic_slab.is_some())
                });
            SourceRoom {
                slab,
                no_foundation: e.is_some_and(|n| !n.options.build_foundation_below),
                offset: e.map_or(0.0, |n| n.floor_height_offset),
                stem: e.and_then(|n| n.stem_wall_height).filter(|h| *h > 0.5),
                anchor: e.map(|n| n.anchor),
                room,
            }
        })
        .collect()
}

/// A foundation wall the builder will make: where, how high it stands and
/// which wall of Floor 1 it is under.
struct PlannedWall {
    source: Id,
    /// Distance along the source wall to `start`, and the piece's length:
    /// a wall is cut where interior walls end on it, so the heights can
    /// step there (manual p. 746).
    offset0: f64,
    len: f64,
    start: Point,
    end: Point,
    curve: Option<crate::walls::WallCurve>,
    /// Top and bottom relative to Floor 1's finished level, inches.
    top: f64,
    bottom: f64,
    /// The wall bounds a slab room (it gets cutouts for doors).
    slab_edge: bool,
    create_below: bool,
}

/// Which walls get a foundation and how tall (manual pp. 742, 746, 748).
fn plan_walls(
    floor: &Floor,
    rooms: &[SourceRoom],
    opts: &FoundationOptions,
    platform: f64,
    floor_h: f64,
) -> Vec<PlannedWall> {
    let mut out: Vec<PlannedWall> = Vec::new();
    for w in &floor.walls {
        if w.flags.foundation || w.flags.attic || w.length() < 1.0 {
            continue;
        }
        let mut bounds = vec![0.0];
        bounds.extend(t_junctions(floor, w));
        bounds.push(w.path_length());
        let mut pieces: Vec<PlannedWall> = Vec::new();
        for pair in bounds.windows(2) {
            let (d0, d1) = (pair[0], pair[1]);
            let (start, end) = if bounds.len() == 2 {
                (w.start, w.end)
            } else {
                let len = w.length();
                (
                    Point::lerp(w.start, w.end, d0 / len),
                    Point::lerp(w.start, w.end, d1 / len),
                )
            };
            pieces.extend(plan_piece(
                w,
                (d0, d1),
                (start, end),
                rooms,
                opts,
                platform,
                floor_h,
            ));
        }
        // Neighbouring pieces that came out alike are one wall again.
        for p in pieces {
            match out.last_mut() {
                Some(a)
                    if a.source == p.source
                        && (a.offset0 + a.len - p.offset0).abs() < 1e-6
                        && (a.top - p.top).abs() < 1e-6
                        && (a.bottom - p.bottom).abs() < 1e-6
                        && a.slab_edge == p.slab_edge
                        && a.create_below == p.create_below =>
                {
                    a.end = p.end;
                    a.len += p.len;
                }
                _ => out.push(p),
            }
        }
    }
    out
}

/// The foundation walls under the bay, box and bow windows of `floor`: one
/// per section of the unit's outer face, as high as the foundation wall under
/// the unit's wall (manual p. 634: the foundation is built under the unit on
/// Floor 1). Nothing is planned where the wall itself has no foundation.
fn bay_walls(floor: &Floor, rooms: &[Room], planned: &[PlannedWall]) -> Vec<PlannedWall> {
    let exterior = |w: &Wall| crate::openings::exterior_sign(w, rooms);
    let mut out = Vec::new();
    for (id, wall_id, pts) in floor.bay_foundation_outlines(&exterior) {
        let Some(center) = floor
            .openings
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.center_offset)
        else {
            continue;
        };
        let Some(host) = planned.iter().find(|p| {
            p.source == wall_id && p.offset0 - 1e-6 <= center && center <= p.offset0 + p.len + 1e-6
        }) else {
            continue;
        };
        for pair in pts.windows(2) {
            let len = pair[0].dist(pair[1]);
            if len < 1.0 {
                continue;
            }
            out.push(PlannedWall {
                source: wall_id,
                offset0: 0.0,
                len,
                start: pair[0],
                end: pair[1],
                curve: None,
                top: host.top,
                bottom: host.bottom,
                slab_edge: false,
                create_below: false,
            });
        }
    }
    out
}

/// Distances along the straight wall `w` at which other walls of the floor
/// end on it (a T-junction), so the foundation under it can change at the
/// room boundary there. Curved walls are not cut.
fn t_junctions(floor: &Floor, w: &Wall) -> Vec<f64> {
    if w.curve.is_some() {
        return Vec::new();
    }
    let len = w.length();
    let dir = w.direction();
    let mut out: Vec<f64> = Vec::new();
    for o in &floor.walls {
        if o.id == w.id || o.flags.foundation || o.flags.attic || o.flags.invisible {
            continue;
        }
        for p in [o.start, o.end] {
            let rel = p.sub(w.start);
            let d = rel.dot(dir);
            let off = rel.cross(dir).abs();
            if off <= w.thickness * 0.5 + 1.0
                && d > 1.0
                && d < len - 1.0
                && !out.iter().any(|x| (x - d).abs() < 1.0)
            {
                out.push(d);
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// The foundation under the piece `span` (distances along `w`) of wall `w`
/// that runs from `ends.0` to `ends.1`, or `None` when it gets none.
fn plan_piece(
    w: &Wall,
    span: (f64, f64),
    ends: (Point, Point),
    rooms: &[SourceRoom],
    opts: &FoundationOptions,
    platform: f64,
    floor_h: f64,
) -> Option<PlannedWall> {
    use crate::walls::WallClass;
    let s = opts.settings;
    let mono = matches!(opts.kind, FoundationKind::MonolithicSlab);
    let gap = opts.top_gap(platform);
    let natural = -floor_h;
    {
        let hidden = w.flags.invisible
            || w.flags.railing
            || w.flags.room_divider
            || matches!(
                w.class,
                WallClass::Railing
                    | WallClass::DeckRailing
                    | WallClass::DeckEdge
                    | WallClass::Fencing { .. }
                    | WallClass::RoomDivider
            );
        let along = (span.0 + span.1) * 0.5;
        let mid = w.point_along(along);
        let reach = w.thickness * 0.5 + 1.5;
        let n = w.normal_along(along);
        let mut adj: Vec<usize> = [1.0, -1.0]
            .iter()
            .filter_map(|sign| {
                rooms
                    .iter()
                    .position(|r| r.room.contains(mid + n * (sign * reach)))
            })
            .collect();
        adj.dedup();
        let create_below = w.spec.foundation.create_below;
        let all_declined = !adj.is_empty() && adj.iter().all(|&i| rooms[i].no_foundation);
        let wanted = if w.kind == WallKind::Exterior && !hidden {
            !all_declined || create_below
        } else {
            let two = adj.len() == 2;
            let separates_slab = two && rooms[adj[0]].slab != rooms[adj[1]].slab;
            let steps = two
                && ((rooms[adj[0]].offset - rooms[adj[1]].offset).abs() > STEP_TOLERANCE
                    || rooms[adj[0]].stem != rooms[adj[1]].stem);
            let bearing = w.spec.structure.bearing_wall && !hidden;
            (create_below || bearing || separates_slab || steps) && (!all_declined || create_below)
        };
        if !wanted {
            return None;
        }
        let (mut top, mut bottom) = (f64::NEG_INFINITY, f64::INFINITY);
        for &i in &adj {
            let r = &rooms[i];
            let t = if mono {
                if r.slab {
                    0.0
                } else {
                    r.offset.max(0.0)
                }
            } else if r.slab {
                r.offset + s.garage_floor_to_stem_top
            } else {
                r.offset - gap
            };
            let min_h = if r.slab {
                s.min_garage_height
            } else {
                s.min_height
            };
            let b = match r.stem {
                Some(h) => t - h,
                None if mono => natural.min(t - opts.edge_height.max(1.0)),
                // A room at the floor's own height keeps the height asked
                // for; a lowered one or a slab room is never shorter than
                // the minimum.
                None if r.offset == 0.0 && !r.slab => natural,
                None => natural.min(t - min_h),
            };
            top = top.max(t);
            bottom = bottom.min(b);
        }
        if adj.is_empty() {
            top = if mono { 0.0 } else { -gap };
            bottom = natural;
        }
        Some(PlannedWall {
            source: w.id,
            offset0: span.0,
            len: span.1 - span.0,
            start: ends.0,
            end: ends.1,
            curve: w.curve,
            top,
            bottom,
            slab_edge: adj.iter().any(|&i| rooms[i].slab),
            create_below,
        })
    }
}

/// The block under a masonry fireplace on Floor 1: the same footprint,
/// material and place in the wall, no firebox, hearth, mantel or chimney,
/// `height` tall (manual p. 758).
fn fireplace_base(
    sym: &PlacedSymbol,
    fp: &Fireplace,
    id: Id,
    height: f64,
) -> (PlacedSymbol, Fireplace) {
    let mut s = sym.clone();
    s.id = id;
    s.label = "Fireplace Foundation".into();
    s.options.clear();
    s.distribution = None;
    s.owner = None;
    let mut f = Fireplace::new(id, FireplaceKind::Masonry);
    f.name = "Fireplace Foundation".into();
    f.in_wall = fp.in_wall;
    f.no_firebox = true;
    f.base_of = Some(sym.id);
    f.elevation = 0.0;
    f.height = Some(height);
    f.hearth.enabled = false;
    f.mantel.enabled = false;
    f.chimney.enabled = false;
    f.materials = fp.materials.clone();
    f.suppress_dimensions = true;
    f.fit_to(s.width, s.depth);
    (s, f)
}

/// Names the rooms of the foundation floor `floor` (found as `below`): a
/// Slab room under each slab (it supplies the floor above), else the
/// `tier` (a Basement, with a slab or without, or a Crawl Space), with that
/// function's platform defaults.
fn name_foundation_rooms(
    floor: &mut Floor,
    below: &[Room],
    slab_under: &[bool],
    tier: Option<(&str, bool)>,
) {
    for (i, room) in below.iter().enumerate() {
        if room.name_entry(&floor.room_names).is_some() {
            continue;
        }
        let (type_name, with_slab) = if slab_under.get(i).copied().unwrap_or(false) {
            ("Slab", false)
        } else if let Some(t) = tier {
            t
        } else {
            continue;
        };
        let mut n = RoomName::new(room.interior_point(), type_name, type_name);
        let d = function_defaults(type_name, type_name);
        apply_function_defaults(&mut n, &d, 0.0);
        let bare = type_name == "Slab" || (type_name == "Basement" && !with_slab);
        if bare {
            // The slab object (or nothing) is the floor and the platform
            // above is the ceiling; a basement of 48 in has no finishes.
            n.has_floor = false;
            n.has_ceiling = false;
            n.floor_height_offset = 0.0;
            if let Some(m) = n.misc.as_mut() {
                m.floor_structure.clear();
            }
        }
        if type_name == "Slab" {
            n.options.supplies_floor_above = true;
        }
        floor.room_names.push(n);
    }
}

/// A digest of what Build Foundation reads from the first floor `floor`,
/// for Auto Rebuild. It leaves out what the build writes back (the garage
/// drop is in it as the floor heights are).
pub fn foundation_signature(floor: &Floor, opts: &FoundationOptions) -> u64 {
    struct Fnv(u64);
    impl Fnv {
        fn bytes(&mut self, b: &[u8]) {
            for x in b {
                self.0 ^= u64::from(*x);
                self.0 = self.0.wrapping_mul(0x100_0000_01b3);
            }
        }
        fn f(&mut self, v: f64) {
            self.bytes(&((v * 100.0).round() as i64).to_le_bytes());
        }
        fn p(&mut self, p: Point) {
            self.f(p.x);
            self.f(p.y);
        }
        fn u(&mut self, v: u64) {
            self.bytes(&v.to_le_bytes());
        }
        fn s(&mut self, v: &str) {
            self.bytes(v.as_bytes());
            self.bytes(&[0]);
        }
    }
    let mono = matches!(opts.kind, FoundationKind::MonolithicSlab);
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    h.f(floor.settings.floor_structure_thickness);
    for w in &floor.walls {
        if w.flags.foundation || w.flags.attic {
            continue;
        }
        h.p(w.start);
        h.p(w.end);
        h.u(u64::from(w.kind == WallKind::Exterior));
        h.u(u64::from(w.flags.invisible)
            | u64::from(w.flags.railing) << 1
            | u64::from(w.flags.room_divider) << 2
            | u64::from(w.spec.foundation.create_below) << 3
            | u64::from(w.spec.structure.bearing_wall) << 4);
        h.f(w.curve.map_or(0.0, |c| c.bulge));
    }
    for n in &floor.room_names {
        h.p(n.anchor);
        h.s(&n.room_type);
        h.f(n.floor_height_offset);
        h.f(n.stem_wall_height.unwrap_or(-1.0));
        h.u(u64::from(n.options.build_foundation_below));
        if !mono {
            h.u(u64::from(n.monolithic_slab.is_some()));
        }
    }
    for o in floor
        .openings
        .iter()
        .filter(|o| o.kind == OpeningKind::Door)
    {
        h.u(o.wall_id);
        h.f(o.center_offset);
        h.f(o.width);
        h.f(crate::foundation::garage_cut_width(o));
    }
    for (sym, fp) in floor.fireplace_symbols() {
        h.p(sym.position);
        h.f(sym.width);
        h.f(sym.depth);
        h.f(sym.angle);
        h.u(fp.kind as u64);
        h.u(u64::from(fp.in_wall));
    }
    h.0
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
    fn rebuilding_the_foundation_keeps_what_was_drawn_on_it() {
        let mut p = house();
        p.build_foundation(FoundationKind::StemWall { height: 36.0 });
        // A partition drawn on the foundation floor, a renamed room and a
        // note survive a rebuild; the foundation walls are made again.
        let part = p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            4.5,
            36.0,
            WallKind::Interior,
        );
        let kept = p.floors[0].room_names.len();
        let mut mine = RoomName::new(Point::new(10.0, 10.0), "Wine Cellar", "Basement");
        mine.floor_height_offset = 2.0;
        p.floors[0].room_names.push(mine);
        let old_ids: Vec<Id> = p.floors[0]
            .walls
            .iter()
            .filter(|w| w.flags.foundation)
            .map(|w| w.id)
            .collect();
        assert_eq!(
            p.build_foundation(FoundationKind::StemWall { height: 48.0 }),
            0
        );
        assert_eq!(p.floors.len(), 2, "rebuilt in place");
        let f = &p.floors[0];
        assert!(f.walls.iter().any(|w| w.id == part), "the partition stays");
        let foundation: Vec<_> = f.walls.iter().filter(|w| w.flags.foundation).collect();
        assert_eq!(foundation.len(), 4);
        // The platform bears on the walls: they stop 10 1/4 in under the
        // first floor's finished level.
        assert!(foundation
            .iter()
            .all(|w| w.height == 48.0 - FLOOR_PLATFORM_THICKNESS && !old_ids.contains(&w.id)));
        assert_eq!(f.elevation, -48.0);
        assert!(f.room_names.iter().any(|n| n.name == "Wine Cellar"));
        assert!(f.room_names.len() >= kept, "{} rooms", f.room_names.len());
    }

    #[test]
    fn build_foundation_walls_a_bay_window_on_floor_1_unless_it_is_raised() {
        use crate::openings::bay::{BayUnit, RaisedFloor};
        let mut p = house();
        let wall = p.floors[0].walls[0].id;
        let id = p.add_opening(0, wall, 60.0, OpeningKind::Window).unwrap();
        {
            let o = p.floors[0]
                .openings
                .iter_mut()
                .find(|o| o.id == id)
                .unwrap();
            o.style = OpeningStyle::BayWindow;
            o.width = 50.0;
            o.extras.spec.bay = BayUnit::for_style(OpeningStyle::BayWindow);
            // A bench seat raises the unit from the ground: nothing under it.
            o.extras.spec.bay.raised_floor = Some(RaisedFloor::default());
        }
        p.build_foundation(FoundationKind::StemWall { height: 36.0 });
        let before = p.floors[0].walls.len();
        p.floors[1]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .extras
            .spec
            .bay
            .raised_floor = None;
        p.build_foundation(FoundationKind::StemWall { height: 36.0 });
        // Three more walls, one per section, standing outside the first wall.
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), before + 3);
        let out = f
            .walls
            .iter()
            .filter(|w| w.start.y < -5.0 || w.end.y < -5.0)
            .count();
        assert_eq!(out, 3);
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
            assert_eq!(w.height, 36.0 - FLOOR_PLATFORM_THICKNESS);
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
        // The platform bears on the beams, so the floor is a platform taller.
        assert_eq!(f.elevation, -(50.0 + FLOOR_PLATFORM_THICKNESS));
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
    fn the_attic_floor_gets_attic_walls_and_no_rooms() {
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
        // Rooms cannot be created on the Attic floor (manual p. 773).
        assert!(f.room_names.is_empty());
        assert!(
            p.attic_floor_warning(a).is_none(),
            "attic walls alone are fine"
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
