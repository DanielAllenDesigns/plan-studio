//! Schedule categories of terrain and road objects (manual pp. 1321, 1351).
//!
//! The Terrain Perimeter, Driveways, Medians, Roads and Road Markings have
//! categories of their own; Sidewalks, Terrain Walls and Curbs are Terrain
//! Paths; features, modifiers, garden beds, grass and water features and
//! stepping stones are Terrain Features. Any object can be moved to another
//! category on its Schedule panel ([`crate::ObjectExtras::schedule_category`]).

use plan_core::geometry::polygon_area;

use crate::landscape::{path_length, LandscapeKind};
use crate::model::{RoadKind, Terrain};
use crate::spec::ObjectKey;

/// A category of the terrain schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScheduleCategory {
    TerrainPerimeter,
    Driveways,
    Medians,
    Roads,
    RoadMarkings,
    TerrainPaths,
    TerrainFeatures,
}

impl ScheduleCategory {
    pub const ALL: [ScheduleCategory; 7] = [
        ScheduleCategory::TerrainPerimeter,
        ScheduleCategory::Driveways,
        ScheduleCategory::Medians,
        ScheduleCategory::Roads,
        ScheduleCategory::RoadMarkings,
        ScheduleCategory::TerrainPaths,
        ScheduleCategory::TerrainFeatures,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ScheduleCategory::TerrainPerimeter => "Terrain Perimeter",
            ScheduleCategory::Driveways => "Driveways",
            ScheduleCategory::Medians => "Medians",
            ScheduleCategory::Roads => "Roads",
            ScheduleCategory::RoadMarkings => "Road Markings",
            ScheduleCategory::TerrainPaths => "Terrain Paths",
            ScheduleCategory::TerrainFeatures => "Terrain Features",
        }
    }

    /// The category a name (case-insensitive) stands for.
    pub fn from_name(name: &str) -> Option<ScheduleCategory> {
        let n = name.trim();
        ScheduleCategory::ALL
            .into_iter()
            .find(|c| c.name().eq_ignore_ascii_case(n))
    }
}

/// The category an object belongs to until its Schedule panel says otherwise
/// (`None` for objects that are not scheduled: elevation data, breaks, plants
/// and sprinklers, which have their own schedules).
pub fn default_category(t: &Terrain, key: ObjectKey) -> Option<ScheduleCategory> {
    Some(match key {
        ObjectKey::Perimeter => ScheduleCategory::TerrainPerimeter,
        ObjectKey::Point(_) | ObjectKey::Line(_) | ObjectKey::Region(_) | ObjectKey::Break(_) => {
            return None
        }
        ObjectKey::Modifier(_) | ObjectKey::Feature(_) => ScheduleCategory::TerrainFeatures,
        ObjectKey::Wall(_) => ScheduleCategory::TerrainPaths,
        ObjectKey::Landscape(i) => match t.landscape.get(i)?.kind {
            LandscapeKind::GardenBed
            | LandscapeKind::GrassRegion
            | LandscapeKind::WaterFeature
            | LandscapeKind::SteppingStones => ScheduleCategory::TerrainFeatures,
            LandscapeKind::Plants | LandscapeKind::Sprinklers | LandscapeKind::SprinklerLine => {
                return None
            }
        },
        ObjectKey::Road(i) => match t.roads.get(i)?.kind {
            RoadKind::Road | RoadKind::CulDeSac => ScheduleCategory::Roads,
            RoadKind::Driveway => ScheduleCategory::Driveways,
            RoadKind::Sidewalk => ScheduleCategory::TerrainPaths,
            RoadKind::Marking => ScheduleCategory::RoadMarkings,
            RoadKind::Median => ScheduleCategory::Medians,
        },
    })
}

/// The category of `key` as scheduled: the one chosen on its Schedule panel,
/// else the default.
pub fn category_of(t: &Terrain, key: ObjectKey) -> Option<ScheduleCategory> {
    let own = t.extras(key).schedule_category;
    if !own.trim().is_empty() {
        if let Some(c) = ScheduleCategory::from_name(&own) {
            return Some(c);
        }
    }
    default_category(t, key)
}

/// One row of the terrain schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleRow {
    pub key: ObjectKey,
    pub category: ScheduleCategory,
    /// The object's name, numbered within its kind ("Driveway 2").
    pub name: String,
    /// Custom label text, or the automatic label.
    pub label: String,
    /// Length of the object's path or outline, inches.
    pub length: f64,
    /// Area it covers, square inches (0 for a path without width).
    pub area: f64,
    pub material: String,
}

fn closed_len(poly: &[plan_core::Point]) -> f64 {
    if poly.len() < 2 {
        return 0.0;
    }
    path_length(poly) + poly[poly.len() - 1].dist(poly[0])
}

/// The terrain objects in schedule order (by category, then drawing order).
pub fn terrain_schedule(t: &Terrain) -> Vec<ScheduleRow> {
    let mut rows = Vec::new();
    let mut counts: std::collections::HashMap<&'static str, usize> = Default::default();
    for key in t.object_keys() {
        let Some(category) = category_of(t, key) else {
            continue;
        };
        let (kind_name, length, area, material): (&'static str, f64, f64, String) = match key {
            ObjectKey::Perimeter => (
                "Terrain Perimeter",
                closed_len(&t.perimeter),
                polygon_area(&t.perimeter).abs(),
                t.ground_material.clone(),
            ),
            ObjectKey::Modifier(i) => {
                let m = &t.modifiers[i];
                (
                    "Terrain Modifier",
                    closed_len(&m.polygon),
                    polygon_area(&m.polygon).abs(),
                    String::new(),
                )
            }
            ObjectKey::Feature(i) => {
                let f = &t.features[i];
                (
                    "Terrain Feature",
                    closed_len(&f.polygon),
                    polygon_area(&f.polygon).abs(),
                    f.material.clone(),
                )
            }
            ObjectKey::Wall(i) => {
                let w = &t.walls[i];
                (
                    match w.kind {
                        crate::landscape::WallKind::Wall => "Terrain Wall",
                        crate::landscape::WallKind::Curb => "Terrain Curb",
                    },
                    path_length(&w.points),
                    path_length(&w.points) * w.thickness,
                    w.material.clone(),
                )
            }
            ObjectKey::Landscape(i) => {
                let l = &t.landscape[i];
                let region = l.is_region();
                (
                    l.name(),
                    if region {
                        closed_len(&l.points)
                    } else {
                        path_length(&l.points)
                    },
                    if region {
                        polygon_area(&l.points).abs()
                    } else {
                        0.0
                    },
                    l.material.clone(),
                )
            }
            ObjectKey::Road(i) => {
                let r = &t.roads[i];
                let (length, area) = crate::roads::road_length_and_area(r);
                (r.kind.name(), length, area, r.material_name().to_string())
            }
            _ => continue,
        };
        let n = counts.entry(kind_name).or_default();
        *n += 1;
        rows.push(ScheduleRow {
            key,
            category,
            name: format!("{kind_name} {n}"),
            label: crate::labels::label_text(t, key),
            length,
            area,
            material,
        });
    }
    rows.sort_by_key(|r| r.category);
    rows
}
