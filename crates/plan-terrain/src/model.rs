//! Data model: the editable [`Terrain`] and the built [`TerrainSurface`].

use std::sync::OnceLock;

use plan_core::Point;
use serde::{Deserialize, Serialize};

use crate::query::TriIndex;

/// A spot height (Elevation Point tool).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElevationPoint {
    pub pos: Point,
    /// Elevation, inches.
    pub z: f64,
}

/// A polyline at one height (Elevation Line / Elevation Spline once flattened).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElevationLine {
    pub points: Vec<Point>,
    pub z: f64,
}

/// A polygon held at one height (Elevation Region).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElevationRegion {
    pub polygon: Vec<Point>,
    pub z: f64,
}

/// The terrain modifier tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierKind {
    /// Smooth bump added to the surface.
    Hill,
    /// Smooth dip subtracted from the surface.
    Valley,
    /// Shifts the surface up inside the polygon.
    RaisedRegion,
    /// Shifts the surface down inside the polygon.
    LoweredRegion,
    /// Flat Region (Cut/Fill): levels the polygon to its mean elevation.
    FlatRegion,
}

/// A polygonal modifier. `height` is a magnitude in inches (ignored by `FlatRegion`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    pub kind: ModifierKind,
    pub polygon: Vec<Point>,
    pub height: f64,
}

/// The terrain feature tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureKind {
    Rectangular,
    Kidney,
    Spline,
    /// Cuts the surface away (Terrain Hole, Make Terrain Hole Around Building).
    Hole,
}

/// A feature overlay. Only `Hole` changes the surface; the others are drawn in plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub kind: FeatureKind,
    pub polygon: Vec<Point>,
    pub material: String,
}

/// Road-like strip types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoadKind {
    Road,
    Driveway,
    Sidewalk,
}

/// A road, driveway or sidewalk: a polyline strip draped on the terrain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoadStrip {
    pub kind: RoadKind,
    pub centerline: Vec<Point>,
    /// Full strip width, inches.
    pub width: f64,
    pub curb: bool,
}

/// Everything the user draws for the terrain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Terrain {
    /// Terrain Perimeter, a closed polygon (the closing point is implicit).
    pub perimeter: Vec<Point>,
    pub elevation_points: Vec<ElevationPoint>,
    pub elevation_lines: Vec<ElevationLine>,
    pub elevation_regions: Vec<ElevationRegion>,
    pub modifiers: Vec<Modifier>,
    pub features: Vec<Feature>,
    pub roads: Vec<RoadStrip>,
    /// Chief default 6": how far the subfloor sits above the terrain at the building.
    pub subfloor_height_above_terrain: f64,
    /// Elevation of the building pad, inches.
    pub building_pad_elevation: f64,
    /// Laplacian smoothing iterations applied to interior vertices (default 0).
    pub smoothing: u32,
    /// Spacing of the sample grid, inches (default 120").
    pub grid_spacing: f64,
}

impl Default for Terrain {
    /// A flat 100' x 80' lot at elevation 0 with no data.
    fn default() -> Self {
        Terrain {
            perimeter: vec![
                Point::new(0.0, 0.0),
                Point::new(1200.0, 0.0),
                Point::new(1200.0, 960.0),
                Point::new(0.0, 960.0),
            ],
            elevation_points: Vec::new(),
            elevation_lines: Vec::new(),
            elevation_regions: Vec::new(),
            modifiers: Vec::new(),
            features: Vec::new(),
            roads: Vec::new(),
            subfloor_height_above_terrain: 6.0,
            building_pad_elevation: 0.0,
            smoothing: 0,
            grid_spacing: 120.0,
        }
    }
}

impl Terrain {
    /// Finished floor elevation: the building pad plus the subfloor height above terrain.
    pub fn finished_floor_elevation(&self) -> f64 {
        self.building_pad_elevation + self.subfloor_height_above_terrain
    }
}

/// The regular sample grid the surface was built from (row-major, `z[j * nx + i]`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HeightGrid {
    /// Plan position of node `(0, 0)`.
    pub origin: Point,
    pub spacing: f64,
    pub nx: usize,
    pub ny: usize,
    /// Elevation (after modifiers) at every node, including nodes outside the perimeter.
    pub z: Vec<f64>,
}

impl HeightGrid {
    /// Elevation at node `(i, j)`, or `None` when out of range.
    pub fn at(&self, i: usize, j: usize) -> Option<f64> {
        (i < self.nx && j < self.ny)
            .then(|| self.z.get(j * self.nx + i).copied())
            .flatten()
    }

    /// Plan position of node `(i, j)`.
    pub fn node(&self, i: usize, j: usize) -> Point {
        Point::new(
            self.origin.x + i as f64 * self.spacing,
            self.origin.y + j as f64 * self.spacing,
        )
    }
}

/// A triangulated terrain surface.
///
/// `vertices` are `[plan x, elevation, plan y]`; `triangles` index them and are
/// counter-clockwise in plan (so they face up). The point-location index used by
/// [`crate::elevation_at`] is built lazily on first query, so do not mutate
/// `vertices`/`triangles` after querying.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerrainSurface {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub grid: HeightGrid,
    #[serde(skip)]
    index: OnceLock<TriIndex>,
}

impl TerrainSurface {
    /// Assemble a surface from its parts.
    pub fn new(vertices: Vec<[f64; 3]>, triangles: Vec<[u32; 3]>, grid: HeightGrid) -> Self {
        TerrainSurface {
            vertices,
            triangles,
            grid,
            index: OnceLock::new(),
        }
    }

    /// Plan position of vertex `i`.
    pub fn plan_point(&self, i: u32) -> Point {
        let v = self.vertices[i as usize];
        Point::new(v[0], v[2])
    }

    pub(crate) fn tri_index(&self) -> &TriIndex {
        self.index
            .get_or_init(|| TriIndex::build(&self.vertices, &self.triangles))
    }
}
